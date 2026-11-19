//! Owner: lane IMPL-SPAWN-TRACE-FILTERS (decision 2026-11-19-90). The spawn spot search's trace filters, in
//! `ParityMode::Off`: which primitives a box test or a sweep tests, and which hit a sweep returns. Coded from
//! `docs/verification/NATIVE_SPAWN_PLACEMENT.md` section 7 (analyst lane ANALYST-SPAWN-TRACE-FILTERS, decision
//! 2026-11-19-84) alone, the implementer half of the clean room. AI-assisted (Claude Code). **Every rule here is
//! UNVERIFIED in the running game.**
//!
//! **The flag sets** (decimal, as the note gives them): every box test passes [`BOX_TEST`] (8902), with [`COMPLEX`]
//! added in template mode for a `bCollideComplex` template; every sweep passes [`SWEEP`] (8326), and the sweep routine
//! adds [`ONE_RESULT`]. The spawn's queries pass no source actor.
//!
//! **Who is tested** ([`tested`]): a primitive component that blocks the query's kind (`BlockNonZeroExtent` for a box
//! test of a non-zero extent and for a box sweep, `BlockZeroExtent` for a zero-extent box test and for a point sweep),
//! on an owner whose class row ([`Row`]) accepts the flag set. A box test always walks the actor collision hash; a
//! sweep walks it only under the mask [`SWEEP_HASH_MASK`]. A class whose filter the note did not read takes the
//! nearest read ancestor's row (UNVERIFIED: such a class may override the filter).
//!
//! **The note's Off policy for its unread points:** a tested candidate whose primitive skip flag is set makes the query
//! Unsupported. The flag's name is unresolved, so a candidate whose flag cannot be read counts as one that may have it
//! ([`Candidate::Tested`] carries no flag value; the caller answers Unsupported). The further per-component test is
//! taken as passing, and a terrain-specific effect of bit 8192 is ignored.
//!
//! **Which hit a sweep returns** ([`choose_sweep_hit`]): the BSP hit; the actor hits up to the BSP hit plus 5 units
//! along the sweep (the whole sweep without one), the smallest fraction kept; then the smaller fraction of the two
//! (the note's Off reading of an unread comparator). Exact ties with different locations or normals are Unsupported.

use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef};
use ow_vm::error::VmError;

use crate::actors::flag;

/// Bit 1: pawns.
pub const PAWNS: u32 = 1;
/// Bit 2: movers.
pub const MOVERS: u32 = 2;
/// Bit 4: the level's own BSP geometry, tested first.
pub const LEVEL_BSP: u32 = 4;
/// Bit 8: volumes.
pub const VOLUMES: u32 = 8;
/// Bit 16: other actors (non-world-geometry actors), and stationary pawns.
pub const OTHER_ACTORS: u32 = 16;
/// Bit 32: projectile targets only.
pub const PROJ_TARGETS: u32 = 32;
/// Bit 64: blocking only, through a test on the query's source actor.
pub const BLOCKING_ONLY: u32 = 64;
/// Bit 128: world-geometry actors.
pub const WORLD_GEOMETRY: u32 = 128;
/// Bit 512: stop at the first hit.
pub const STOP_AT_FIRST: u32 = 512;
/// Bit 1024: one result, added by the sweep routine.
pub const ONE_RESULT: u32 = 1024;
/// Bit 8192: terrain (the sweep routine's entry mask only).
pub const TERRAIN: u32 = 8192;
/// Bit 131072: complex (per-polygon) collision.
pub const COMPLEX: u32 = 131072;

/// The flag set of every box test.
pub const BOX_TEST: u32 = 8902;
/// The flag set of every sweep, before the sweep routine adds [`ONE_RESULT`].
pub const SWEEP: u32 = 8326;
/// A sweep walks the actor collision hash only when one of these bits is set (1, 2, 8, 16, 128, 8192).
pub const SWEEP_HASH_MASK: u32 = 8347;
/// How far past the BSP hit a sweep still walks the actors, in units along the sweep.
pub const PAST_BSP: f32 = 5.0;

/// One of the spawn's queries, as the filter sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    /// A box test; `zero_extent` for an extent of (0, 0, 0); `complex` for a template-mode test of a
    /// `bCollideComplex` template.
    BoxTest { zero_extent: bool, complex: bool },
    BoxSweep,
    PointSweep,
}

impl Query {
    /// The flag set the query passes.
    pub fn flags(self) -> u32 {
        match self {
            Query::BoxTest { complex, .. } => BOX_TEST | if complex { COMPLEX } else { 0 },
            Query::BoxSweep | Query::PointSweep => SWEEP | ONE_RESULT,
        }
    }

    /// Whether the query walks the actor collision hash.
    pub fn walks_actors(self) -> bool {
        match self {
            Query::BoxTest { .. } => true,
            Query::BoxSweep | Query::PointSweep => self.flags() & SWEEP_HASH_MASK != 0,
        }
    }

    /// The primitive flag a candidate needs to block this query's kind.
    pub fn blocking_flag(self) -> &'static str {
        match self {
            Query::BoxTest { zero_extent: false, .. } | Query::BoxSweep => "BlockNonZeroExtent",
            Query::BoxTest { zero_extent: true, .. } | Query::PointSweep => "BlockZeroExtent",
        }
    }
}

/// The owner actor's filter, by class (note section 7, step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `Actor`, and the classes that keep its rule (`StaticMeshActor`, `Brush`, `Terrain`, and any class without a
    /// nearer read ancestor).
    Actor,
    Volume,
    /// `InterpActor` and `KActor`.
    Mover,
    Pawn,
}

/// The classes whose filter was read, nearest first when walking up from a class.
const ROWS: [(&str, Row); 4] = [
    ("Engine.Pawn", Row::Pawn),
    ("Engine.Volume", Row::Volume),
    ("Engine.InterpActor", Row::Mover),
    ("Engine.KActor", Row::Mover),
];

/// The row of `class`: the nearest read ancestor's (itself included), else `Actor`'s.
pub fn row_of(space: &ObjectSpace, class: ClassId) -> Row {
    let mut at = Some(class);
    while let Some(c) = at {
        for (path, row) in ROWS {
            if space.classes.by_path(path) == Some(c) {
                return row;
            }
        }
        at = space.classes.get_layout(c).and_then(|l| l.super_class());
    }
    Row::Actor
}

/// The owner's flags a row reads. A flag a row does not read is not read from the object.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Owner {
    pub world_geometry: bool,
    pub pawns_only: bool,
    pub ignore_all_traces: bool,
    pub stationary: bool,
    pub proj_target: bool,
    pub block_actors: bool,
}

/// The narrowing (note section 7, step 3), with no source actor: bit 32 accepts a projectile target or a blocking
/// actor with a blocking primitive; else bit 64 needs the source actor, so it rejects; else it accepts.
fn narrowed(o: &Owner, primitive_blocks_actors: bool, flags: u32) -> bool {
    if flags & PROJ_TARGETS != 0 {
        o.proj_target || (o.block_actors && primitive_blocks_actors)
    } else {
        flags & BLOCKING_ONLY == 0
    }
}

/// Whether an owner of `row` with flags `o` is tested by a query passing `flags` with no source actor.
pub fn owner_tested(row: Row, o: &Owner, primitive_blocks_actors: bool, flags: u32) -> bool {
    match row {
        Row::Actor => {
            (o.world_geometry && flags & WORLD_GEOMETRY != 0)
                || (flags & OTHER_ACTORS != 0 && narrowed(o, primitive_blocks_actors, flags))
        }
        Row::Volume => {
            // A `bPawnsOnly` volume needs a test on the source actor; with none it is rejected.
            !o.pawns_only
                && ((o.world_geometry && flags & WORLD_GEOMETRY != 0)
                    || (flags & VOLUMES != 0 && narrowed(o, primitive_blocks_actors, flags)))
        }
        Row::Mover => flags & MOVERS != 0 && narrowed(o, primitive_blocks_actors, flags),
        Row::Pawn => {
            !o.ignore_all_traces
                && (flags & PAWNS != 0 || (o.stationary && flags & OTHER_ACTORS != 0))
                && flags & BLOCKING_ONLY == 0
        }
    }
}

/// A primitive's standing in one query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidate {
    /// The query does not test it.
    NotTested,
    /// The query tests it. Its skip flag is unresolved and the further per-component test is taken as passing
    /// (module documentation): the caller must hold its geometry and know its skip flag to answer, else Unsupported.
    Tested,
}

/// The standing of `primitive`, owned by the actor `owner`, in `query` (module documentation).
pub fn tested(space: &ObjectSpace, primitive: ObjRef, owner: ObjRef, query: Query) -> Result<Candidate, VmError> {
    if !query.walks_actors() || !flag(space, primitive, query.blocking_flag())? {
        return Ok(Candidate::NotTested);
    }
    let row = row_of(space, space.heap.get(owner)?.class);
    let read = |name: &str| flag(space, owner, name);
    let mut o = Owner::default();
    match row {
        Row::Actor | Row::Volume => {
            o.world_geometry = read("bWorldGeometry")?;
            if row == Row::Volume {
                o.pawns_only = read("bPawnsOnly")?;
            }
        }
        Row::Pawn => {
            o.ignore_all_traces = read("bIgnoreAllTraces")?;
            o.stationary = read("bStationary")?;
        }
        Row::Mover => {}
    }
    let flags = query.flags();
    let primitive_blocks = if row != Row::Pawn && flags & PROJ_TARGETS != 0 {
        o.proj_target = read("bProjTarget")?;
        o.block_actors = read("bBlockActors")?;
        flag(space, primitive, "BlockActors")?
    } else {
        false
    };
    Ok(if owner_tested(row, &o, primitive_blocks, flags) { Candidate::Tested } else { Candidate::NotTested })
}

/// One sweep hit: where, the surface normal, and the fraction of the whole sweep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub location: [f32; 3],
    pub normal: [f32; 3],
    pub fraction: f32,
}

/// Why [`choose_sweep_hit`] cannot answer: an exact tie with different locations or normals (the order is unread).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnreadTieOrder;

/// The smallest-fraction hit of `hits`; an exact tie answers only when the tied hits agree.
fn smallest(hits: impl Iterator<Item = Hit>) -> Result<Option<Hit>, UnreadTieOrder> {
    let mut best: Option<Hit> = None;
    let mut tied_apart = false;
    for h in hits {
        match best {
            None => best = Some(h),
            Some(b) if h.fraction < b.fraction => {
                best = Some(h);
                tied_apart = false;
            }
            Some(b) if h.fraction == b.fraction && (h.location != b.location || h.normal != b.normal) => {
                tied_apart = true;
            }
            Some(_) => {}
        }
    }
    if tied_apart { Err(UnreadTieOrder) } else { Ok(best) }
}

/// The hit a sweep of `length` units returns (module documentation), from the BSP's hit and the actor hits, each
/// fraction of the whole sweep. Actor hits beyond the BSP hit plus [`PAST_BSP`] units are not walked.
pub fn choose_sweep_hit(bsp: Option<Hit>, actors: &[Hit], length: f32) -> Result<Option<Hit>, UnreadTieOrder> {
    let reach = match bsp {
        Some(b) if length > 0.0 => b.fraction + PAST_BSP / length,
        Some(_) => 0.0,
        None => f32::INFINITY,
    };
    let actor = smallest(actors.iter().copied().filter(|h| h.fraction <= reach))?;
    smallest(bsp.into_iter().chain(actor))
}

#[cfg(test)]
mod tests;
