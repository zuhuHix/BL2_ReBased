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
//! **Who is a candidate** ([`tested`]; lane IMPL-LINK9-TRACE-FILTER, from NATIVE_SPAWN_TRACE_FILTER.md section 7,
//! PR #502). Each read test only rejects, so a primitive any one of them rejects is [`Candidate::NotTested`], in any
//! order (note section 3). In the walkers' order:
//! 1. **the skip flag**, read as `BulletListener` on the live primitive (the note's inferred name, UNVERIFIED): set,
//!    not tested, except in a sweep that sets bit 30 ([`SKIP_IGNORED`]), which the spawn's sweeps never do;
//! 2. **the owner** ([`owner_of`]): the component's own `Owner` field when its class declares it and it is set,
//!    otherwise the walked actor, the one whose `CollisionComponent` or `Components` holds the primitive (note
//!    section 7, step 2, review r1 of #502). The game's attach step sets that field; the port's spawn never does, so
//!    the walked actor stands for it and a walked primitive always has an owner (UNVERIFIED). A `bAttached` that reads
//!    clear is no answer on its own (note section 7, step 5): it is not read;
//! 3. the bounds against the query's region: the caller's walk (`crate::placement`), before any flag is read;
//! 4. **the blocking flag** for the query's kind (`BlockZeroExtent` for a box test of extent (0, 0, 0) and for a point
//!    sweep, `BlockNonZeroExtent` otherwise): clear, not tested;
//! 5. **the per-component test** (note section 2): the live primitive's `CollideActors`, and the live owner's
//!    `bCollideActors`, both set; otherwise not tested;
//! 6. **the owner's filter** by its exact class's row ([`Row`]; [`row_of`]): a row that rejects, not tested. The four
//!    controller classes (`Engine.Controller`, `Engine.PlayerController`, `GameFramework.GamePlayerController`,
//!    `WillowGame.WillowPlayerController`) run the
//!    `Actor` row (note section 1). No row is inherited.
//!
//! **What stays unread**, naming the rule ([`Unread`]):
//! - a `Terrain` actor's primitive (the terrain bit's handling), ahead of every test above, blocking or not;
//!
//! and, only when every read test above passes (note section 7, step 3):
//! - an owner whose exact class has no read row;
//! - a primitive every read test accepts: the collision hash's admission beyond the per-component test (the scene
//!   condition and the removal rules, note section 5), [`HASH_ADMISSION`] under the note's record.
//!
//! So a primitive is never answered "tested": the most the port can say is "not tested" or Unsupported.
//!
//! **The touch's line check** ([`Query::PointLine`]; lane IMPL-TOUCH-PICKUP-TRACE, from NATIVE_TOUCH_PICKUP_TRACE.md
//! sections 2 and 4, UNVERIFIED): a point line through the world's single-line routine with the flags [`TOUCH_TRACE`]
//! (plus [`ONE_RESULT`]) and a **source actor** ([`Source`]). Two steps differ from the spawn's queries:
//! - **the source actor's exclusion**, after the per-component test and before the owner's row: a primitive whose
//!   owner is the source actor, or in its `Base` chain, is not tested. The chain is read only when a candidate reaches
//!   this step, from resident objects through lookup-only names. A link the store has not loaded is
//!   [`Unread::BaseChainLink`]; the walk stops at None and at a link already seen (a cycle); a class that declares no
//!   `Base` is a data error;
//! - **a `bPawnsOnly` volume** is [`Unread::SourceVolumeTest`] (the source actor's test, not read) instead of rejected.
//!
//! A primitive every read test accepts is still [`Unread::HashAdmission`]. Only when a test installs the hash stand-in
//! ([`Source::admission_established`]) does it become [`Unread::PrimitiveLineTest`], the primitive's own line test.
//!
//! **Which hit a sweep returns** ([`choose_sweep_hit`]): the BSP hit, or the actors' hit when there is no BSP hit.
//! Actors are walked to the BSP hit plus 5 units along the sweep, capped at the end point (the whole sweep without a
//! BSP hit). A BSP hit together with an actor hit is Unsupported (the comparator between them was not read). Among the
//! actors every hit at the minimum fraction is kept, and a tie whose locations or normals differ is Unsupported.

use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef};
use ow_vm::error::VmError;

use crate::actors::{flag, object};

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
/// Bit 134217728: read by one game-class filter override only (NATIVE_TOUCH_PICKUP_TRACE.md section 3).
pub const TOUCH_ONLY_BIT: u32 = 134_217_728;
/// The flag set of `DroppedPickup.TouchPickupTrace` (bits 2, 4, 128, 512, 8192 and [`TOUCH_ONLY_BIT`]), before the
/// single-line routine adds [`ONE_RESULT`].
pub const TOUCH_TRACE: u32 = 134_226_566;
/// The record of the touch's line check.
pub const RECORD_TOUCH: &str = "NATIVE_TOUCH_PICKUP_TRACE.md";
/// The touch note's names for its unread rules.
pub const SOURCE_VOLUME_TEST: &str = "the source actor's volume test";
pub const BASE_CHAIN_LINK: &str = "a Base-chain link the store has not loaded";
pub const PRIMITIVE_LINE_TEST: &str = "the primitive's line test";

/// The flag set of every box test.
pub const BOX_TEST: u32 = 8902;
/// The flag set of every sweep, before the sweep routine adds [`ONE_RESULT`].
pub const SWEEP: u32 = 8326;
/// A sweep walks the actor collision hash only when one of these bits is set (1, 2, 8, 16, 128, 8192).
pub const SWEEP_HASH_MASK: u32 = 8347;
/// How far past the BSP hit a sweep still walks the actors, in units along the sweep.
pub const PAST_BSP: f32 = 5.0;

/// The note's names for its unread rules.
pub const TERRAIN_HANDLING: &str = "the trace filter's terrain handling";
pub const BSP_ACTOR_COMPARATOR: &str = "the trace filter's comparator between a BSP hit and an actor hit";
pub const TIE_ORDER: &str = "the trace filter's order among tied actor hits";
pub const SKIP_FLAG: &str = "the trace filter's skip flag (bit 10 of a primitive's collision flag word, unresolved)";
/// The skip flag's field as NATIVE_SPAWN_TRACE_FILTER.md section 4 infers it (UNVERIFIED), read by [`tested`]. The
/// spawn extent's rule (`crate::placement`) still answers its own `NotYet` with [`SKIP_FLAG`]: NATIVE_SPAWN_EXTENT.md
/// leaves the flag unread there.
pub const SKIP_FLAG_FIELD: &str = "BulletListener";
/// Bit 30: a sweep that sets it ignores the skip flag. The spawn's queries never set it.
pub const SKIP_IGNORED: u32 = 1 << 30;
/// The record of the per-component test, the controller rows, the skip flag's reading and the hash admission.
pub const RECORD_TRACE_FILTER: &str = "NATIVE_SPAWN_TRACE_FILTER.md";
/// The rule left unread for a primitive every read test accepts (NATIVE_SPAWN_TRACE_FILTER.md section 7, step 5).
pub const HASH_ADMISSION: &str = "the collision hash's admission beyond the per-component test";

/// One of the spawn's queries, as the filter sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    /// A box test; `zero_extent` for an extent of (0, 0, 0); `complex` for a template-mode test of a
    /// `bCollideComplex` template.
    BoxTest { zero_extent: bool, complex: bool },
    BoxSweep,
    PointSweep,
    /// The touch's point line (zero extent), with a source actor (module documentation).
    PointLine,
}

impl Query {
    /// The flag set the query passes.
    pub fn flags(self) -> u32 {
        match self {
            Query::BoxTest { complex, .. } => BOX_TEST | if complex { COMPLEX } else { 0 },
            Query::BoxSweep | Query::PointSweep => SWEEP | ONE_RESULT,
            Query::PointLine => TOUCH_TRACE | ONE_RESULT,
        }
    }

    /// Whether the query walks the actor collision hash.
    pub fn walks_actors(self) -> bool {
        match self {
            Query::BoxTest { .. } => true,
            Query::BoxSweep | Query::PointSweep | Query::PointLine => self.flags() & SWEEP_HASH_MASK != 0,
        }
    }

    /// The primitive flag a candidate needs to block this query's kind.
    pub fn blocking_flag(self) -> &'static str {
        match self {
            Query::BoxTest { zero_extent: true, .. } | Query::PointSweep | Query::PointLine => "BlockZeroExtent",
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

/// The classes whose filter was read. Only these have a row. The four controller classes run `Actor`'s body
/// (NATIVE_SPAWN_TRACE_FILTER.md section 1).
const ROWS: [(&str, Row); 12] = [
    ("Engine.Actor", Row::Actor),
    ("Engine.StaticMeshActor", Row::Actor),
    ("Engine.Brush", Row::Actor),
    ("Engine.Terrain", Row::Actor),
    ("Engine.Volume", Row::Volume),
    ("Engine.InterpActor", Row::Mover),
    ("Engine.KActor", Row::Mover),
    ("Engine.Pawn", Row::Pawn),
    ("Engine.Controller", Row::Actor),
    ("Engine.PlayerController", Row::Actor),
    // The installed packages hold it in GameFramework (decision 2026-11-15-23 census; review r1 of #514).
    ("GameFramework.GamePlayerController", Row::Actor),
    ("WillowGame.WillowPlayerController", Row::Actor),
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
    /// A primitive whose owner's exact class has no read row; the class's path.
    OwnerClass(String),
    /// A primitive of a `Terrain` actor.
    TerrainHandling,
    /// A primitive every read test accepts: whether the game's collision hash holds it.
    HashAdmission,
    /// A `bPawnsOnly` volume in a query with a source actor: the source actor's test.
    SourceVolumeTest,
    /// A link of the source actor's `Base` chain the store has not loaded, needed to decide a candidate.
    BaseChainLink,
    /// A primitive every read test accepts, once the hash admits it (a test stand-in only): its own line test.
    PrimitiveLineTest,
}

impl Unread {
    /// The note's name for the rule.
    pub fn rule(&self) -> String {
        match self {
            Unread::OwnerClass(class) => format!("the trace filter of {class}"),
            Unread::TerrainHandling => TERRAIN_HANDLING.to_string(),
            Unread::HashAdmission => HASH_ADMISSION.to_string(),
            Unread::SourceVolumeTest => SOURCE_VOLUME_TEST.to_string(),
            Unread::BaseChainLink => BASE_CHAIN_LINK.to_string(),
            Unread::PrimitiveLineTest => PRIMITIVE_LINE_TEST.to_string(),
        }
    }

    /// The record that leaves the rule unread.
    pub fn record(&self) -> &'static str {
        match self {
            Unread::OwnerClass(_) | Unread::TerrainHandling => "NATIVE_SPAWN_PLACEMENT.md",
            Unread::HashAdmission => RECORD_TRACE_FILTER,
            Unread::SourceVolumeTest | Unread::BaseChainLink | Unread::PrimitiveLineTest => RECORD_TOUCH,
        }
    }
}

/// A primitive's standing in one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    /// The query does not test it: a read test rejects it (module documentation), or the query does not walk the
    /// actors.
    NotTested,
    /// The query's answer depends on an unread rule for it: Unsupported.
    Unsupported(Unread),
}

/// The owner the filter reads for `primitive`, met in the walk of the actor `walked` (module documentation, step 2):
/// the component's `Owner` field when its class declares it and it is set, otherwise `walked`.
pub fn owner_of(space: &ObjectSpace, primitive: ObjRef, walked: ObjRef) -> Result<ObjRef, VmError> {
    let class = space.heap.get(primitive)?.class;
    if space.prop(class, "Owner").is_some()
        && let Some(owner) = object(space, primitive, "Owner")?
    {
        return Ok(owner);
    }
    Ok(walked)
}

/// A query's source actor (module documentation, the touch's line check).
pub struct Source<'a> {
    /// The source actor (the pickup).
    pub actor: ObjRef,
    /// Whether an object is a handle the store has not loaded (a link whose `Base` cannot be read).
    pub unloaded: &'a dyn Fn(ObjRef) -> bool,
    /// A test stand-in for the hash step: when set, the hash's admission counts as established (UNVERIFIED).
    pub admission_established: bool,
}

/// Where `owner` stands against the source actor's chain.
enum Chain {
    Excluded,
    Outside,
    Unloaded,
}

/// Whether `owner` is the source actor or in its `Base` chain (module documentation). Reads resident objects only,
/// through lookup-only names; bounded by the links seen.
fn chain_of(space: &ObjectSpace, s: &Source<'_>, owner: ObjRef) -> Result<Chain, VmError> {
    if owner == s.actor {
        return Ok(Chain::Excluded);
    }
    let mut seen = vec![s.actor];
    let mut at = s.actor;
    loop {
        let class = space.heap.get(at)?.class;
        if space.prop(class, "Base").is_none() {
            return Err(VmError::Script {
                function: "the touch trace's Base chain".to_string(),
                message: format!("{} declares no Base", ow_attr::eval::class_path(space, at)),
            });
        }
        match object(space, at, "Base")? {
            None => return Ok(Chain::Outside),
            Some(next) if seen.contains(&next) => return Ok(Chain::Outside),
            Some(next) if next == owner => return Ok(Chain::Excluded),
            Some(next) if (s.unloaded)(next) => return Ok(Chain::Unloaded),
            Some(next) => {
                seen.push(next);
                at = next;
            }
        }
    }
}

/// The standing of `primitive`, met in the walk of the actor `walked`, in `query` (module documentation): its owner
/// ([`owner_of`]), then the read tests in the walkers' order, each only rejecting, then the unread rules. Reads the
/// live objects only. A query with no source actor ([`tested_from`] with `None`).
pub fn tested(space: &ObjectSpace, primitive: ObjRef, walked: ObjRef, query: Query) -> Result<Candidate, VmError> {
    tested_from(space, primitive, walked, query, None)
}

/// [`tested`] for a query that may pass a source actor (module documentation, the touch's line check).
pub fn tested_from(
    space: &ObjectSpace,
    primitive: ObjRef,
    walked: ObjRef,
    query: Query,
    source: Option<&Source<'_>>,
) -> Result<Candidate, VmError> {
    if !query.walks_actors() {
        return Ok(Candidate::NotTested);
    }
    // The terrain bit's handling is unread and comes ahead of every other test (NATIVE_SPAWN_PLACEMENT.md, kept by
    // the hub for IMPL-LINK9-TRACE-FILTER): a `Terrain` actor's primitive is Unsupported, blocking or not. The walked
    // actor is checked first, before any owner is resolved, then the resolved owner (review r1 of #514, item 2): a
    // terrain on either side refuses, so a component owner can never discard a walked terrain's primitive.
    let is_terrain = |o: ObjRef| -> Result<bool, VmError> {
        let class = space.heap.get(o)?.class;
        Ok(space.classes.by_path("Engine.Terrain").is_some_and(|t| space.classes.is_child_of(class, t)))
    };
    if is_terrain(walked)? {
        return Ok(Candidate::Unsupported(Unread::TerrainHandling));
    }
    let owner = owner_of(space, primitive, walked)?;
    if is_terrain(owner)? {
        return Ok(Candidate::Unsupported(Unread::TerrainHandling));
    }
    let class = space.heap.get(owner)?.class;
    // The skip flag (note section 4); a sweep with bit 30 ignores it.
    let ignores_skip =
        matches!(query, Query::BoxSweep | Query::PointSweep | Query::PointLine) && query.flags() & SKIP_IGNORED != 0;
    if !ignores_skip && flag(space, primitive, SKIP_FLAG_FIELD)? {
        return Ok(Candidate::NotTested);
    }
    if !flag(space, primitive, query.blocking_flag())? {
        return Ok(Candidate::NotTested);
    }
    // The per-component test (note section 2), on the live primitive and its live owner.
    if !flag(space, primitive, "CollideActors")? || !flag(space, owner, "bCollideActors")? {
        return Ok(Candidate::NotTested);
    }
    // The source actor's exclusion (module documentation), read only for a candidate that reaches it.
    if let Some(s) = source {
        match chain_of(space, s, owner)? {
            Chain::Excluded => return Ok(Candidate::NotTested),
            Chain::Unloaded => return Ok(Candidate::Unsupported(Unread::BaseChainLink)),
            Chain::Outside => {}
        }
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
                if o.pawns_only && source.is_some() {
                    return Ok(Candidate::Unsupported(Unread::SourceVolumeTest));
                }
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
    if !owner_tested(row, &o, primitive_blocks, flags) {
        return Ok(Candidate::NotTested);
    }
    if source.is_some_and(|s| s.admission_established) {
        return Ok(Candidate::Unsupported(Unread::PrimitiveLineTest));
    }
    Ok(Candidate::Unsupported(Unread::HashAdmission))
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
