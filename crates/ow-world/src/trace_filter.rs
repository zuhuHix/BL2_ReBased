//! Owner: lane IMPL-SPAWN-TRACE-FILTERS (decision 2026-11-19-90). The spawn spot search's trace filters, in
//! `ParityMode::Off`: which primitives a box test or a sweep tests, and which hit a sweep returns. Coded from
//! `docs/verification/NATIVE_SPAWN_PLACEMENT.md` section 7 (analyst lane ANALYST-SPAWN-TRACE-FILTERS, decision
//! 2026-11-19-84, at its review-r3 head) alone, the implementer half of the clean room. AI-assisted (Claude Code).
//! **Every rule here is UNVERIFIED in the running game.**
//!
//! **The flag sets** (decimal, as the note gives them): every box test passes [`BOX_TEST`] (8902), with [`COMPLEX`]
//! added in template mode for a `bCollideComplex` template; every sweep passes [`SWEEP`] (8326), and the sweep routine
//! adds [`ONE_RESULT`]. The spawn's queries pass no source actor.
//!
//! **Who is a candidate** ([`tested`]): a primitive component that blocks the query's kind (`BlockZeroExtent` for a
//! box test of extent (0, 0, 0) and for a point sweep, `BlockNonZeroExtent` otherwise), on an owner whose class row
//! ([`Row`]) accepts the flag set. A box test always walks the actor collision hash; a sweep walks it only under the
//! mask [`SWEEP_HASH_MASK`]. Only the classes the note read have a row ([`row_of`]); no row is inherited.
//!
//! **The note's Off policy for what is unread:** wherever an unread rule decides the answer, the query is
//! Unsupported, naming the rule ([`Unread`]). So:
//! - every candidate is Unsupported, because the further per-component test runs for it and was not read (a
//!   candidate's skip flag, also unresolved, is covered by the same answer);
//! - a blocking primitive whose owner's class has no read row is Unsupported;
//! - a `Terrain` actor's primitive is Unsupported (the terrain bit's handling was not read).
//!
//! **Which hit a sweep returns** ([`choose_sweep_hit`]): the BSP hit, or the actors' hit when there is no BSP hit.
//! Actors are walked to the BSP hit plus 5 units along the sweep, capped at the end point (the whole sweep without a
//! BSP hit). A BSP hit together with an actor hit is Unsupported (the comparator between them was not read). Among the
//! actors every hit at the minimum fraction is kept, and a tie whose locations or normals differ is Unsupported.

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

/// The note's names for its unread rules.
pub const PER_COMPONENT_TEST: &str = "the trace filter's per-component test";
pub const TERRAIN_HANDLING: &str = "the trace filter's terrain handling";
pub const BSP_ACTOR_COMPARATOR: &str = "the trace filter's comparator between a BSP hit and an actor hit";
pub const TIE_ORDER: &str = "the trace filter's order among tied actor hits";

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
            Query::BoxTest { zero_extent: true, .. } | Query::PointSweep => "BlockZeroExtent",
            Query::BoxTest { zero_extent: false, .. } | Query::BoxSweep => "BlockNonZeroExtent",
        }
    }
}

/// The owner actor's filter, by class (note section 7, step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `Actor`, `StaticMeshActor`, `Brush` and `Terrain`.
    Actor,
    Volume,
    /// `InterpActor` and `KActor`.
    Mover,
    Pawn,
}

/// The classes whose filter was read. Only these have a row.
const ROWS: [(&str, Row); 8] = [
    ("Engine.Actor", Row::Actor),
    ("Engine.StaticMeshActor", Row::Actor),
    ("Engine.Brush", Row::Actor),
    ("Engine.Terrain", Row::Actor),
    ("Engine.Volume", Row::Volume),
    ("Engine.InterpActor", Row::Mover),
    ("Engine.KActor", Row::Mover),
    ("Engine.Pawn", Row::Pawn),
];

/// The row of `class` when its filter was read; `None` for any other class (a subclass of a read class included,
/// since it may override the filter).
pub fn row_of(space: &ObjectSpace, class: ClassId) -> Option<Row> {
    ROWS.into_iter().find(|(path, _)| space.classes.by_path(path) == Some(class)).map(|(_, row)| row)
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

/// An unread rule that decides a query's answer (module documentation): the query is Unsupported, naming it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// A candidate primitive: the further per-component test (and the skip flag) decide it.
    PerComponentTest,
    /// A blocking primitive whose owner's class has no read row; the class's path.
    OwnerClass(String),
    /// A primitive of a `Terrain` actor.
    TerrainHandling,
}

impl Unread {
    /// The note's name for the rule.
    pub fn rule(&self) -> String {
        match self {
            Unread::PerComponentTest => PER_COMPONENT_TEST.to_string(),
            Unread::OwnerClass(class) => format!("the trace filter of {class}"),
            Unread::TerrainHandling => TERRAIN_HANDLING.to_string(),
        }
    }
}

/// A primitive's standing in one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    /// The query does not test it.
    NotTested,
    /// The query's answer depends on an unread rule for it: Unsupported.
    Unsupported(Unread),
}

/// The standing of `primitive`, owned by the actor `owner`, in `query` (module documentation).
pub fn tested(space: &ObjectSpace, primitive: ObjRef, owner: ObjRef, query: Query) -> Result<Candidate, VmError> {
    if !query.walks_actors() {
        return Ok(Candidate::NotTested);
    }
    let class = space.heap.get(owner)?.class;
    if let Some(terrain) = space.classes.by_path("Engine.Terrain") {
        if space.classes.is_child_of(class, terrain) {
            return Ok(Candidate::Unsupported(Unread::TerrainHandling));
        }
    }
    if !flag(space, primitive, query.blocking_flag())? {
        return Ok(Candidate::NotTested);
    }
    let Some(row) = row_of(space, class) else {
        return Ok(Candidate::Unsupported(Unread::OwnerClass(ow_attr::eval::class_path(space, owner))));
    };
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
    Ok(if owner_tested(row, &o, primitive_blocks, flags) {
        Candidate::Unsupported(Unread::PerComponentTest)
    } else {
        Candidate::NotTested
    })
}

/// One sweep hit: where, the surface normal, and the fraction of the whole sweep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub location: [f32; 3],
    pub normal: [f32; 3],
    pub fraction: f32,
}

/// Why [`choose_sweep_hit`] cannot answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadChoice {
    /// A BSP hit and an actor hit: their comparator was not read ([`BSP_ACTOR_COMPARATOR`]).
    Comparator,
    /// Actor hits tied at the minimum with different locations or normals ([`TIE_ORDER`]).
    TieOrder,
}

impl UnreadChoice {
    /// The note's name for the rule.
    pub fn rule(self) -> &'static str {
        match self {
            UnreadChoice::Comparator => BSP_ACTOR_COMPARATOR,
            UnreadChoice::TieOrder => TIE_ORDER,
        }
    }
}

/// The hit a sweep of `length` units returns (module documentation), from the BSP's hit and the actor hits, each
/// fraction of the whole sweep. Actor hits beyond the BSP hit plus [`PAST_BSP`] units (capped at the end point) are
/// not walked.
pub fn choose_sweep_hit(bsp: Option<Hit>, actors: &[Hit], length: f32) -> Result<Option<Hit>, UnreadChoice> {
    let reach = match bsp {
        Some(b) if length > 0.0 => (b.fraction + PAST_BSP / length).min(1.0),
        Some(b) => b.fraction,
        None => 1.0,
    };
    let walked: Vec<Hit> = actors.iter().copied().filter(|h| h.fraction <= reach).collect();
    if let Some(b) = bsp {
        return if walked.is_empty() { Ok(Some(b)) } else { Err(UnreadChoice::Comparator) };
    }
    let Some(min) = walked.iter().map(|h| h.fraction).reduce(f32::min) else {
        return Ok(None);
    };
    let mut tied = walked.into_iter().filter(|h| h.fraction == min);
    let first = tied.next().expect("the minimum is one of the hits");
    if tied.any(|h| h.location != first.location || h.normal != first.normal) {
        Err(UnreadChoice::TieOrder)
    } else {
        Ok(Some(first))
    }
}

#[cfg(test)]
mod tests;
