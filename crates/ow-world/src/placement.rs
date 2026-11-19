//! Owner: lane IMPL-SPAWN-PLACEMENT (decision 2026-11-19-79). Where a spawn is placed, in `ParityMode::Off`: the spawn's
//! spot search and its encroachment check, from `docs/verification/NATIVE_SPAWN_PLACEMENT.md` (analyst lane
//! ANALYST-SPAWN-PLACEMENT, decision 2026-11-19-77, at its final head 1a83fcbd) alone, the implementer half of the clean
//! room. **Every rule here is UNVERIFIED in the running game.**
//!
//! **The collision service** (note section 7). The search tests world geometry, which this crate does not hold. A host
//! registers a [`SpawnCollision`] as a VM service ([`install_collision`]); this crate gains no dependency for it. Each
//! of its three queries answers per query: Clear, Hit (a sweep carries its location, normal and fraction), or
//! Unsupported. An answer needs the shape, the geometry over the **whole** region the query touches (the full box;
//! for a sweep the full swept volume) in the requested collision mode, and the query's filter; anything less is
//! Unsupported, so an empty candidate set is Clear only when the whole region is covered. A world without a service answers
//! the note's `NotYet { LayerAbsent }` ("the spawn placement spot search"), never "free". No service exists in the
//! tree yet. The host's movement collision (lane H4) cannot answer box queries (it has no box shape) and holds simple
//! hulls only, so an adapter over it would answer every box query Unsupported (note section 7).
//!
//! **The search** (note sections 3 and 6), before anything is made, in this precedence:
//! 1. `bNoCollisionFail`: no search. Neither the template's `bCollideWorld` nor its `bCollideWhenPlacing` outside a
//!    client: no search. The template is the object the actor is made from (the archetype, else the class default),
//!    read alone.
//! 2. No service: `NotYet { LayerAbsent }`. Then no cylinder extent (the template's `CollisionComponent` is not a
//!    `CylinderComponent`, or none): `NotYet { NotRead }`, "the actor's bounding-cylinder rule".
//! 3. **The trace filters** (lane IMPL-SPAWN-TRACE-FILTERS, decision 2026-11-19-90; [`crate::trace_filter`]). Each
//!    query has two parts. The service answers for the geometry it holds: the level's BSP and whatever else it loaded,
//!    under the note's filters. This crate answers for the VM's live actors: for each kind of query, the first live
//!    actor primitive the filter tests ([`crate::trace_filter::tested`]). The world holds no geometry for an actor's
//!    primitives, and their skip flag is unresolved, so a tested one makes that kind of query Unsupported. A box test
//!    the service reports blocked is blocked whatever the actors (bit 512, the BSP first).
//! 4. The steps of section 3 (the first box test, the zero-extent failure, the slice step, the corner nudges, the
//!    point sweep, the final sweep), each query asked of both parts. The first Unsupported query is
//!    `NotYet { NotRead }` naming it (and, for an actor part, the actor). A failed search makes the spawn return None,
//!    nothing made; a successful one gives the location.
//!
//! **After `PreBeginPlay`** (note section 6), only after play began and when the actor was not destroyed, decided on
//! the live actor's `bCollideActors`. These run after the actor is made, so a `NotYet` is terminal and prefixed
//! "stopped after partial writes", its kind kept:
//! - `bNoCollisionFail` false: the flag false passes the check (the port's Off answer, not a reading: the spawn routine
//!   calls it whatever the flag, open question 1, UNVERIFIED); true with no service `NotYet { LayerAbsent }`, true
//!   with one `NotYet { NotRead }` (the check's rule);
//! - `bNoCollisionFail` true and the flag true: the relocation step, `NotYet { NotRead }`.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::actors::{actors_of, flag, float, is_destroyed, object, objects};
use crate::trace_filter::{self, Candidate, Query};

/// Where these rules are written down.
pub const RECORD: &str = "NATIVE_SPAWN_PLACEMENT.md";

/// The note's names.
pub const SPOT_SEARCH: &str = "the spawn placement spot search";
pub const BOUNDING_CYLINDER: &str = "the actor's bounding-cylinder rule";
pub const ENCROACHMENT: &str = "the post-spawn encroachment check";
pub const ENCROACHMENT_RULE: &str = "the post-spawn encroachment check's rule";
pub const RELOCATION: &str = "the post-spawn relocation step";
/// The mark of a refusal after the actor was made.
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

const LAYER: &str = "collision (the world's collision service, a SpawnCollision)";

/// A box test's collision mode (note section 3, "Terms").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Complex (per-polygon) collision when the template has `bCollideComplex`.
    Template { complex: bool },
    /// Simple collision always.
    Simple,
}

/// A box test's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Test {
    Clear,
    Blocked,
    Unsupported,
}

/// A sweep's answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sweep {
    Clear,
    Hit { location: [f32; 3], normal: [f32; 3], fraction: f32 },
    Unsupported,
}

/// The world's collision service (module documentation). Extents are half sizes.
pub trait SpawnCollision {
    /// Whether an axis-aligned box of `extent` at `at` overlaps world geometry.
    fn box_test(&self, extent: [f32; 3], at: [f32; 3], mode: Mode) -> Result<Test, VmError>;
    /// A box of `extent` swept from `start` to `end`: the first hit.
    fn box_sweep(&self, extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError>;
    /// A point swept from `start` to `end`: the first hit.
    fn point_sweep(&self, start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError>;
}

/// The collision service of a world that loaded no level (a harness stand-in, as slice link 9's own world is,
/// decision 2026-11-16-42): it holds no BSP and no other geometry, so every query is clear. The live VM actors are
/// still the crate's part of each query (module documentation, step 3), so a tested actor primitive still makes the
/// query Unsupported. Not a stand-in for a loaded map: a world with a level must install a service that holds it.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoLevel;

impl SpawnCollision for NoLevel {
    fn box_test(&self, _extent: [f32; 3], _at: [f32; 3], _mode: Mode) -> Result<Test, VmError> {
        Ok(Test::Clear)
    }
    fn box_sweep(&self, _extent: [f32; 3], _start: [f32; 3], _end: [f32; 3]) -> Result<Sweep, VmError> {
        Ok(Sweep::Clear)
    }
    fn point_sweep(&self, _start: [f32; 3], _end: [f32; 3]) -> Result<Sweep, VmError> {
        Ok(Sweep::Clear)
    }
}

/// The VM service holding the world's collision service.
pub struct CollisionService(pub Rc<dyn SpawnCollision>);

/// Registers `model` as the world's collision service. A second one is refused.
pub fn install_collision(vm: &mut Vm, model: Rc<dyn SpawnCollision>) -> Result<(), VmError> {
    if vm.service::<CollisionService>().is_some() {
        return Err(VmError::Registration {
            key: "the world's collision service".to_string(),
            reason: "a collision service is already installed",
        });
    }
    vm.add_service(CollisionService(model))?;
    Ok(())
}

/// The world's collision service: `Ok(None)` when none is registered; a registered one that is borrowed is
/// `VmError::ServiceBorrowed`, not an absent service (review r1 of #463, item 3).
pub fn collision_model(vm: &Vm) -> Result<Option<Rc<dyn SpawnCollision>>, VmError> {
    let Some(s) = vm.service::<CollisionService>() else {
        return Ok(None);
    };
    let model = s.try_borrow().map_err(|_| VmError::ServiceBorrowed("CollisionService"))?.0.clone();
    Ok(Some(model))
}

/// Whether the spot search runs (module documentation, step 1), on the template's flags alone.
pub fn search_runs(space: &ObjectSpace, template: ObjRef, no_collision_fail: bool, client: bool) -> Result<bool, VmError> {
    if no_collision_fail {
        return Ok(false);
    }
    Ok(flag(space, template, "bCollideWorld")? || (flag(space, template, "bCollideWhenPlacing")? && !client))
}

/// The template's extent (note section 2): (radius, radius, half height) of its cylinder component.
pub fn extent(space: &ObjectSpace, template: ObjRef) -> Result<[f32; 3], VmError> {
    let not_read = || -> VmError { NotYet::not_read(BOUNDING_CYLINDER, RECORD).into() };
    let Some(component) = object(space, template, "CollisionComponent")? else {
        return Err(not_read());
    };
    let Some(cylinder) = space.classes.by_path("Engine.CylinderComponent") else {
        return Err(not_read());
    };
    if !space.classes.is_child_of(space.heap.get(component)?.class, cylinder) {
        return Err(not_read());
    }
    let radius = float(space, component, "CollisionRadius")?;
    let height = float(space, component, "CollisionHeight")?;
    Ok([radius, radius, height])
}

fn location_error(why: String) -> VmError {
    VmError::Script { message: format!("the spawn location {why}"), function: "Actor.Spawn".to_string() }
}

/// The field indices of X, Y and Z in a `Vector` struct value.
fn xyz(space: &ObjectSpace, v: &Value) -> Result<[usize; 3], VmError> {
    let s = v.as_struct().ok_or_else(|| location_error(format!("is a {}, not a vector", v.kind_name())))?;
    let layout = space.classes.struct_layout(s.ty).ok_or_else(|| location_error("has no struct layout".to_string()))?;
    let mut out = [0; 3];
    for (i, n) in ["X", "Y", "Z"].iter().enumerate() {
        let id = space.names.find(n).ok_or_else(|| location_error(format!("has no field {n}")))?;
        let (entry, _) = layout.field(id).ok_or_else(|| location_error(format!("has no field {n}")))?;
        out[i] = usize::try_from(entry).map_err(|_| location_error(format!("field {n} is out of range")))?;
    }
    Ok(out)
}

fn vector(space: &ObjectSpace, v: &Value) -> Result<[f32; 3], VmError> {
    let at = xyz(space, v)?;
    let s = v.as_struct().expect("checked by xyz");
    let mut out = [0.0; 3];
    for (k, i) in at.iter().enumerate() {
        out[k] = match s.fields.get(*i) {
            Some(Value::Float(f)) => *f,
            Some(other) => return Err(location_error(format!("holds a {}", other.kind_name()))),
            None => return Err(location_error("lacks a field".to_string())),
        };
    }
    Ok(out)
}

fn with_vector(space: &ObjectSpace, v: &Value, p: [f32; 3]) -> Result<Value, VmError> {
    let at = xyz(space, v)?;
    let mut out = v.clone();
    let s = out.struct_mut().ok_or_else(|| location_error("is not a vector".to_string()))?;
    for (k, i) in at.iter().enumerate() {
        *s.fields.get_mut(*i).ok_or_else(|| location_error("lacks a field".to_string()))? = Value::Float(p[k]);
    }
    Ok(out)
}

fn unsupported(query: &str) -> VmError {
    NotYet::not_read(
        format!("{SPOT_SEARCH}: {query}, which the world's collision service does not support"),
        RECORD,
    )
    .into()
}

fn test_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Template { .. } => "an axis-aligned box test in template mode",
        Mode::Simple => "an axis-aligned box test in simple mode",
    }
}

/// The first live actor primitive each kind of query tests (module documentation, step 3), as "actor's component";
/// `None` when the kind tests none.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ActorScan {
    box_test: Option<String>,
    zero_box_test: Option<String>,
    box_sweep: Option<String>,
    point_sweep: Option<String>,
}

impl ActorScan {
    fn of(&self, q: Query) -> Option<&String> {
        match q {
            Query::BoxTest { zero_extent: false, .. } => self.box_test.as_ref(),
            Query::BoxTest { zero_extent: true, .. } => self.zero_box_test.as_ref(),
            Query::BoxSweep => self.box_sweep.as_ref(),
            Query::PointSweep => self.point_sweep.as_ref(),
        }
    }
}

fn name(space: &ObjectSpace, o: ObjRef) -> Result<String, VmError> {
    Ok(space.names.text(space.heap.get(o)?.name).into_owned())
}

/// The live actors' primitives, scanned once per search (nothing changes the actors while it runs). An actor's
/// primitives are its `CollisionComponent` and the primitive components in its `Components`, each once; a class
/// that declares neither field has none.
fn scan_actors(vm: &Vm) -> Result<ActorScan, VmError> {
    let space = &vm.space;
    let mut scan = ActorScan::default();
    let Some(primitive) = space.classes.by_path("Engine.PrimitiveComponent") else {
        return Ok(scan);
    };
    let list = actors_of(vm)?.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
    for actor in list {
        if is_destroyed(space, actor)? {
            continue;
        }
        let class = space.heap.get(actor)?.class;
        let mut parts: Vec<ObjRef> = Vec::new();
        if space.prop(class, "CollisionComponent").is_some() {
            parts.extend(object(space, actor, "CollisionComponent")?);
        }
        if space.prop(class, "Components").is_some() {
            for c in objects(space, actor, "Components")?.into_iter().flatten() {
                if !parts.contains(&c) {
                    parts.push(c);
                }
            }
        }
        for part in parts {
            if !space.classes.is_child_of(space.heap.get(part)?.class, primitive) {
                continue;
            }
            for (kind, slot) in [
                (Query::BoxTest { zero_extent: false, complex: false }, &mut scan.box_test),
                (Query::BoxTest { zero_extent: true, complex: false }, &mut scan.zero_box_test),
                (Query::BoxSweep, &mut scan.box_sweep),
                (Query::PointSweep, &mut scan.point_sweep),
            ] {
                if slot.is_none() && trace_filter::tested(space, part, actor, kind)? == Candidate::Tested {
                    *slot = Some(format!("{}'s {}", name(space, actor)?, name(space, part)?));
                }
            }
        }
    }
    Ok(scan)
}

fn actor_unsupported(query: &str, actor: &str) -> VmError {
    NotYet::not_read(
        format!(
            "{SPOT_SEARCH}: {query} that tests the live actor primitive {actor}, whose geometry and skip flag the \
             world does not hold"
        ),
        RECORD,
    )
    .into()
}

/// The search's queries: the service's part, then the live actors' (module documentation, step 3). An Unsupported
/// part becomes its named `NotYet`.
struct Queries<'a> {
    model: &'a dyn SpawnCollision,
    actors: ActorScan,
}

impl Queries<'_> {
    fn actors(&self, q: Query, name: &str) -> Result<(), VmError> {
        match self.actors.of(q) {
            Some(actor) => Err(actor_unsupported(name, actor)),
            None => Ok(()),
        }
    }

    fn free(&self, extent: [f32; 3], at: [f32; 3], mode: Mode) -> Result<bool, VmError> {
        let complex = matches!(mode, Mode::Template { complex: true });
        let q = Query::BoxTest { zero_extent: extent == [0.0; 3], complex };
        match self.model.box_test(extent, at, mode)? {
            Test::Blocked => Ok(false),
            Test::Unsupported => Err(unsupported(test_name(mode))),
            Test::Clear => self.actors(q, test_name(mode)).map(|()| true),
        }
    }

    fn box_sweep(&self, extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Result<Option<([f32; 3], [f32; 3], f32)>, VmError> {
        match self.model.box_sweep(extent, start, end)? {
            Sweep::Unsupported => Err(unsupported("a box sweep")),
            hit => {
                self.actors(Query::BoxSweep, "a box sweep")?;
                Ok(match hit {
                    Sweep::Hit { location, normal, fraction } => Some((location, normal, fraction)),
                    _ => None,
                })
            }
        }
    }

    fn point_hits(&self, start: [f32; 3], end: [f32; 3]) -> Result<bool, VmError> {
        match self.model.point_sweep(start, end)? {
            Sweep::Unsupported => Err(unsupported("a point sweep")),
            hit => {
                self.actors(Query::PointSweep, "a point sweep")?;
                Ok(matches!(hit, Sweep::Hit { .. }))
            }
        }
    }
}

/// What the slice step reported (note section 3, step 3).
enum Slice {
    Success([f32; 3]),
    /// Failed; `blocked` when the slice's own first test was not free.
    Failed { blocked: bool },
}

/// The slice step at `l` (note section 3, step 3).
fn slice(q: &Queries<'_>, e: [f32; 3], mut l: [f32; 3]) -> Result<Slice, VmError> {
    let s = [e[0], e[1], 1.0];
    if !q.free(s, l, Mode::Simple)? {
        return Ok(Slice::Failed { blocked: true });
    }
    let floor = q.box_sweep(s, l, [l[0], l[1], l[2] - 2.0 * e[2]])?;
    let normal = match floor {
        Some((_, n, t)) if t <= 0.5 => {
            l[2] += e[2] * (1.0 - 2.0 * t) + 1.0;
            if q.free(e, l, Mode::Simple)? {
                return Ok(Slice::Success(l));
            }
            Some(n)
        }
        Some((_, n, t)) => {
            l[2] = l[2] - e[2] * (2.0 * t - 1.0) + 1.0;
            Some(n)
        }
        None => {
            l[2] -= e[2];
            None
        }
    };
    // The common tail.
    if q.free(e, l, Mode::Simple)? {
        if let Some((hit, _, _)) = q.box_sweep(e, l, [l[0], l[1], l[2] + e[2]])? {
            l = hit;
        }
        return Ok(Slice::Success(l));
    }
    // With no floor hit there is no normal: the move is zero and the test repeats.
    let n = normal.unwrap_or([0.0; 3]);
    l[0] += e[0] * n[0];
    l[1] += e[0] * n[1];
    if q.free(e, l, Mode::Simple)? {
        Ok(Slice::Success(l))
    } else {
        Ok(Slice::Failed { blocked: false })
    }
}

/// The search after its first test failed (note section 3, steps 2 and 4): `None` when it fails.
fn search_rest(q: &Queries<'_>, e: [f32; 3], l0: [f32; 3], complex: bool) -> Result<Option<[f32; 3]>, VmError> {
    let template = Mode::Template { complex };
    if e == [0.0; 3] {
        return Ok(None);
    }
    match slice(q, e, l0)? {
        Slice::Success(l) => return Ok(Some(l)),
        Slice::Failed { blocked: false } => return Ok(None),
        Slice::Failed { blocked: true } => {}
    }
    // The corner nudges.
    let mut l = l0;
    let mut counted = 0;
    for (sx, sy) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let d = [sx * 0.55 * e[0], sy * 0.55 * e[1], 0.0];
        let at = [l0[0] + d[0], l0[1] + d[1], l0[2]];
        if q.free([0.5 * e[0], 0.5 * e[1], 1.0], at, template)? && counted < 2 {
            for k in 0..3 {
                l[k] += 1.1 * d[k];
            }
            counted += 1;
        }
    }
    match counted {
        0 => return Ok(None),
        1 => {
            for k in 0..3 {
                l[k] = 2.0 * l[k] - l0[k];
            }
        }
        _ => {}
    }
    if q.point_hits(l0, l)? {
        return Ok(None);
    }
    if !q.free(e, l, template)? {
        match slice(q, e, l)? {
            Slice::Success(moved) => l = moved,
            Slice::Failed { .. } => return Ok(None),
        }
    }
    let end = [l0[0] + 0.2 * (l0[0] - l[0]), l0[1] + 0.2 * (l0[1] - l[1]), l0[2] + 0.2 * (l0[2] - l[2])];
    match q.box_sweep(e, l, end)? {
        None => Ok(Some(l)),
        Some((hit, _, _)) => Ok(Some(hit)),
    }
}

/// The search's steps from the first test (note section 3): the location, or `None` when the search fails.
fn search(q: &Queries<'_>, e: [f32; 3], l0: [f32; 3], complex: bool) -> Result<Option<[f32; 3]>, VmError> {
    if q.free(e, l0, Mode::Template { complex })? {
        return Ok(Some(l0));
    }
    search_rest(q, e, l0, complex)
}

/// The spot search for a spawn of `template` at `location` (module documentation, steps 2 and 3): the location to
/// make the actor at, or `None` when the search fails. Reads only; it runs before anything is made.
pub fn spot_search(vm: &Vm, template: ObjRef, location: &Value) -> Result<Option<Value>, VmError> {
    let space = &vm.space;
    let Some(model) = collision_model(vm)? else {
        return Err(NotYet::layer_absent(SPOT_SEARCH, LAYER, RECORD).into());
    };
    let e = extent(space, template)?;
    let complex = flag(space, template, "bCollideComplex")?;
    let l0 = vector(space, location)?;
    let q = Queries { model: model.as_ref(), actors: scan_actors(vm)? };
    match search(&q, e, l0, complex)? {
        None => Ok(None),
        Some(l) if l == l0 => Ok(Some(location.clone())),
        Some(l) => Ok(Some(with_vector(space, location, l)?)),
    }
}

/// The `NotYet` after `PreBeginPlay` for a live actor that collides with actors (module documentation): the
/// relocation step's for a `bNoCollisionFail` spawn, else the encroachment check's. Prefixed: the actor was made.
pub fn after_pre_begin_play(vm: &Vm, no_collision_fail: bool) -> VmError {
    let n = if no_collision_fail {
        NotYet::not_read(RELOCATION, RECORD)
    } else {
        match collision_model(vm) {
            Ok(None) => NotYet::layer_absent(ENCROACHMENT, LAYER, RECORD),
            Ok(Some(_)) => NotYet::not_read(ENCROACHMENT_RULE, RECORD),
            Err(e) => return e,
        }
    };
    NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into()
}

/// The search's steps against a scripted service with the filter taken as read: these steps are not reached in Off
/// (module documentation), so they are tested here directly. Invented geometry only.
#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    struct Script {
        tests: RefCell<Vec<Test>>,
        sweeps: RefCell<Vec<Sweep>>,
        rest: Test,
        asked: RefCell<Vec<String>>,
    }

    impl SpawnCollision for Script {
        fn box_test(&self, extent: [f32; 3], at: [f32; 3], mode: Mode) -> Result<Test, VmError> {
            self.asked.borrow_mut().push(format!("test {extent:?} {at:?} {mode:?}"));
            let mut q = self.tests.borrow_mut();
            Ok(if q.is_empty() { self.rest } else { q.remove(0) })
        }
        fn box_sweep(&self, extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError> {
            self.asked.borrow_mut().push(format!("sweep {extent:?} {start:?} {end:?}"));
            let mut q = self.sweeps.borrow_mut();
            Ok(if q.is_empty() { Sweep::Clear } else { q.remove(0) })
        }
        fn point_sweep(&self, start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError> {
            self.asked.borrow_mut().push(format!("point {start:?} {end:?}"));
            Ok(Sweep::Clear)
        }
    }

    fn script(tests: Vec<Test>, sweeps: Vec<Sweep>, rest: Test) -> Script {
        Script { tests: RefCell::new(tests), sweeps: RefCell::new(sweeps), rest, asked: RefCell::new(Vec::new()) }
    }

    const E: [f32; 3] = [10.0, 10.0, 20.0];
    const L0: [f32; 3] = [100.0, 200.0, 300.0];

    fn not_read(e: VmError) -> String {
        match e {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, ow_core::Missing::NotRead);
                n.what.into_owned()
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_clear_first_test_keeps_the_location() {
        let s = script(vec![], vec![], Test::Clear);
        let q = Queries { model: &s, actors: ActorScan::default() };
        assert_eq!(search(&q, E, L0, true).unwrap(), Some(L0));
        assert_eq!(s.asked.borrow()[0], format!("test {E:?} {L0:?} {:?}", Mode::Template { complex: true }));
    }

    #[test]
    fn an_unsupported_query_is_named() {
        let s = script(vec![], vec![], Test::Unsupported);
        let q = Queries { model: &s, actors: ActorScan::default() };
        assert!(not_read(search(&q, E, L0, false).unwrap_err()).contains("an axis-aligned box test in template mode"));
    }

    #[test]
    fn a_blocked_zero_extent_fails() {
        let s = script(vec![], vec![], Test::Blocked);
        let q = Queries { model: &s, actors: ActorScan::default() };
        assert_eq!(search(&q, [0.0; 3], L0, false).unwrap(), None);
    }

    #[test]
    fn the_slice_step_raises_the_box_off_a_near_floor_and_lowers_it_onto_a_far_one() {
        // t = 0.25: raised by Ez (1 - 2t) + 1, a clear simple test succeeds with no upward sweep.
        let floor = Sweep::Hit { location: [0.0; 3], normal: [0.0, 0.0, 1.0], fraction: 0.25 };
        let s = script(vec![Test::Blocked, Test::Clear, Test::Clear], vec![floor], Test::Blocked);
        let q = Queries { model: &s, actors: ActorScan::default() };
        assert_eq!(search(&q, E, L0, false).unwrap(), Some([L0[0], L0[1], L0[2] + 20.0 * 0.5 + 1.0]));
        assert_eq!(s.asked.borrow().len(), 4);
        // t = 0.75: lowered by Ez (2t - 1), raised by 1, then the common tail's test and its upward sweep.
        let floor = Sweep::Hit { location: [0.0; 3], normal: [0.0, 0.0, 1.0], fraction: 0.75 };
        let s = script(vec![Test::Blocked, Test::Clear, Test::Clear], vec![floor], Test::Blocked);
        let q = Queries { model: &s, actors: ActorScan::default() };
        let l = [L0[0], L0[1], L0[2] - 20.0 * 0.5 + 1.0];
        assert_eq!(search(&q, E, L0, false).unwrap(), Some(l));
        assert_eq!(*s.asked.borrow().last().unwrap(), format!("sweep {E:?} {l:?} {:?}", [l[0], l[1], l[2] + 20.0]));
    }

    #[test]
    fn a_failed_slice_that_was_not_blocked_fails_the_search() {
        let s = script(vec![Test::Blocked, Test::Clear], vec![], Test::Blocked);
        let q = Queries { model: &s, actors: ActorScan::default() };
        assert_eq!(search(&q, E, L0, false).unwrap(), None);
    }

    #[test]
    fn one_free_corner_doubles_the_move() {
        let tests = vec![Test::Blocked, Test::Blocked, Test::Clear, Test::Blocked, Test::Blocked, Test::Blocked, Test::Clear];
        let s = script(tests, vec![], Test::Blocked);
        let q = Queries { model: &s, actors: ActorScan::default() };
        // In the code's own order, in single precision: one move of 1.1 x the displacement, then doubled.
        let one = |c: f32| c + 1.1 * (-1.0 * 0.55 * 10.0);
        let twice = |c: f32| 2.0 * one(c) - c;
        assert_eq!(search(&q, E, L0, false).unwrap(), Some([twice(L0[0]), twice(L0[1]), L0[2]]));
        assert!(s.asked.borrow()[6].starts_with("point"), "{:?}", s.asked.borrow());
    }
}
