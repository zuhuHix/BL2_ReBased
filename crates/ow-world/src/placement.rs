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
//! 2. No service: `NotYet { LayerAbsent }`. Then the extent ([`extent`]; lane IMPL-LINK9-EXTENT, from
//!    NATIVE_SPAWN_EXTENT.md): a cylinder `CollisionComponent`, else the template's exact class's bounding-cylinder
//!    rule (the four pawn classes' and `Actor`'s; any other class `NotYet { NotRead }`). `Actor`'s rule reads the
//!    template's primitive `Components` entries, which the spawn loads first ([`load_extent_components`], a store
//!    write: every later `NotYet` up to the actor's insertion is marked "stopped after partial writes: ").
//! 3. **The trace filters** (lane IMPL-SPAWN-TRACE-FILTERS, decision 2026-11-19-90; [`crate::trace_filter`]). Each
//!    query has two parts, kept apart:
//!    - **the level part**, asked of the service first: its answers cover the level's BSP only (bit 4). Every other
//!      piece of geometry, a map-authored static mesh included, belongs to an actor and goes through the actor
//!      filters (review r2 of #477);
//!    - **the actor walk**, done by this crate over the live VM actors, and only when the query reaches it: a box
//!      test the level blocks is blocked (bit 512, the BSP first), so no actor is read for it. The walk covers the
//!      query's region: the box, or for a sweep the swept box from the start to the BSP hit plus 5 units, capped at
//!      the end point (the whole sweep without a BSP hit). A primitive under an unread rule
//!      ([`crate::trace_filter::tested`]) is taken out of the walk only when the service reports its bounds
//!      ([`SpawnCollision::actor_bounds`]) and they miss the region; with no bounds, or bounds that meet it, the
//!      query is Unsupported, naming the rule. Missing data on a live primitive is a named refusal too.
//!    - A sweep's result is [`crate::trace_filter::choose_sweep_hit`] over the level's hit and the walk's actor hits,
//!      rescaled to the whole sweep. Under the rules read so far the walk refuses before it could record an actor
//!      hit, so only a level-only hit or no hit is answered.
//! 4. The steps of section 3 (the first box test, the zero-extent failure, the slice step, the corner nudges, the
//!    point sweep, the final sweep), each query asked of both parts. The first Unsupported query is
//!    `NotYet { NotRead }` naming it (and, for an actor part, the rule and the actor). A failed search makes the spawn return None,
//!    nothing made; a successful one gives the location.
//!
//! **After `PreBeginPlay`** (note section 6), only after play began and when the actor was not destroyed, decided on
//! the live actor's `bCollideActors`. These run after the actor is made, so a `NotYet` is terminal, marked "stopped
//! after partial writes" by the spawn's post-insertion boundary, its kind kept:
//! - `bNoCollisionFail` false: the flag false passes the check with no write. That is a reading of the check's gate
//!   (NATIVE_ENCROACHMENT_CHECK.md section 2, step 1), no longer the port's choice. The flag true runs the check
//!   ([`encroachment`], lane IMPL-ENCROACHMENT-CHECK): no service `NotYet { LayerAbsent }`, then the note's attach
//!   step, location step, shape, level part and overlap walk, each refusing with `NotYet { NotRead }`;
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
pub const ENCROACHMENT: &str = "the post-spawn encroachment check";
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

/// An axis-aligned box, by its corners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    /// Whether the two boxes meet (touching counts).
    pub fn meets(&self, other: &Bounds) -> bool {
        (0..3).all(|k| self.min[k] <= other.max[k] && other.min[k] <= self.max[k])
    }
}

/// The world's collision service (module documentation). Extents are half sizes. The three queries answer for the
/// **level's BSP only** (bit 4, the first pass): `Blocked` means the BSP blocks the box, and a sweep's `Hit` is the
/// BSP's first hit. Geometry owned by an actor, a map-authored static mesh's hull included, must not enter these
/// answers: it goes through this crate's actor walk and the trace filters, and the service may only report its
/// bounds ([`SpawnCollision::actor_bounds`]).
pub trait SpawnCollision {
    /// Whether an axis-aligned box of `extent` at `at` overlaps the level's BSP.
    fn box_test(&self, extent: [f32; 3], at: [f32; 3], mode: Mode) -> Result<Test, VmError>;
    /// A box of `extent` swept from `start` to `end`: the BSP's first hit.
    fn box_sweep(&self, extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError>;
    /// A point swept from `start` to `end`: the BSP's first hit.
    fn point_sweep(&self, start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError>;
    /// The world-space bounds of a live VM actor's primitive component, when the service holds them; `None` when it
    /// does not (the default). Used only to take a primitive out of a query's actor walk.
    fn actor_bounds(&self, _space: &ObjectSpace, _primitive: ObjRef) -> Result<Option<Bounds>, VmError> {
        Ok(None)
    }
}

/// The collision service of a world that loaded no level (a harness stand-in, as slice link 9's own world is,
/// decision 2026-11-16-42): it holds no BSP and no other geometry, so every level part is clear. It holds no actor
/// bounds either, so the crate's actor walk (module documentation, step 3) refuses on any live primitive under an
/// unread rule. Not a stand-in for a loaded map: a world with a level must install a service that holds it.
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

/// The classes whose bounding-cylinder rule is `Pawn`'s (NATIVE_SPAWN_EXTENT.md section 3).
pub const PAWN_RULE_CLASSES: [&str; 4] = ["Engine.Pawn", "WillowGame.WillowPawn", "WillowGame.WillowAIPawn", "WillowGame.WillowPlayerPawn"];
/// The class whose bounding-cylinder rule is `Actor`'s own (NATIVE_SPAWN_EXTENT.md section 2).
pub const ACTOR_RULE_CLASS: &str = "Engine.Actor";
/// The record of the bounding-cylinder rule (lane IMPL-LINK9-EXTENT).
pub const EXTENT_RECORD: &str = "NATIVE_SPAWN_EXTENT.md";
/// The prefix of the bounding-cylinder rule's `NotYet`s.
pub const BOUNDING_CYLINDER_OF: &str = "the bounding-cylinder rule of";

/// Whether `component` is a `CylinderComponent` (or a subclass), from its class alone (a lazy handle carries it).
fn is_cylinder(space: &ObjectSpace, component: ObjRef) -> Result<bool, VmError> {
    let Some(cylinder) = space.classes.by_path("Engine.CylinderComponent") else {
        return Ok(false);
    };
    Ok(space.classes.is_child_of(space.heap.get(component)?.class, cylinder))
}

/// Whether `component` is a primitive component, from its class alone (NATIVE_SPAWN_EXTENT.md section 5, step 5.2).
fn is_primitive(space: &ObjectSpace, component: ObjRef) -> Result<bool, VmError> {
    let Some(primitive) = space.classes.by_path("Engine.PrimitiveComponent") else {
        return Ok(false);
    };
    Ok(space.classes.is_child_of(space.heap.get(component)?.class, primitive))
}

/// The class path of `o`, for the rule's texts.
fn class_path(space: &ObjectSpace, o: ObjRef) -> Result<String, VmError> {
    let class = space.heap.get(o)?.class;
    Ok(space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_else(|| format!("{class:?}")))
}

/// Which rule answers the template's extent (NATIVE_SPAWN_EXTENT.md section 5, steps 1 to 4), decided from object
/// handles and classes alone: nothing is loaded for it.
enum Rule {
    /// A cylinder component whose radius and height are the extent (section 5 steps 1 and 2).
    Cylinder(ObjRef),
    /// `Actor`'s rule (section 2), for a template of the class named.
    Actor(String),
    /// A class whose rule was not read.
    NotRead(String),
}

fn rule(space: &ObjectSpace, template: ObjRef) -> Result<Rule, VmError> {
    let collision = object(space, template, "CollisionComponent")?;
    if let Some(c) = collision
        && is_cylinder(space, c)?
    {
        return Ok(Rule::Cylinder(c));
    }
    let class = space.heap.get(template)?.class;
    let path = class_path(space, template)?;
    let exactly = |p: &str| space.classes.by_path(p) == Some(class);
    if PAWN_RULE_CLASSES.iter().any(|p| exactly(p)) {
        if let Some(c) = object(space, template, "CylinderComponent")?
            && Some(c) != collision
            && is_cylinder(space, c)?
        {
            return Ok(Rule::Cylinder(c));
        }
        return Ok(Rule::Actor(path));
    }
    if exactly(ACTOR_RULE_CLASS) {
        return Ok(Rule::Actor(path));
    }
    Ok(Rule::NotRead(path))
}

/// The extent's loads (NATIVE_SPAWN_EXTENT.md section 5, step 5.3), which the spawn runs before the spot search: when
/// the template's extent is `Actor`'s rule, each primitive entry of its `Components` that the store has not loaded is
/// loaded, as the template's own definition (the same reading as the instancing's second pass). None entries and
/// non-primitive entries are skipped and load nothing. A load is a store write, so the answer says whether one ran:
/// the spawn then marks every `NotYet` after it "stopped after partial writes: " (step 5.3). A fresh load attempt
/// counts as a write even when it fails; a retry of a handle whose load already failed does not. A failing load's
/// `NotYet` is marked here when this call wrote (that attempt or an earlier one). A world without a collision service loads nothing: the spot search refuses it first.
pub fn load_extent_components(vm: &mut Vm, template: ObjRef) -> Result<bool, VmError> {
    if collision_model(vm)?.is_none() {
        return Ok(false);
    }
    if !matches!(rule(&vm.space, template)?, Rule::Actor(_)) {
        return Ok(false);
    }
    let mut wrote = false;
    for c in objects(&vm.space, template, "Components")?.into_iter().flatten() {
        if !is_primitive(&vm.space, c)? || !vm.defs.is_unloaded(&vm.space, c) {
            continue;
        }
        // A fresh attempt is a write even when it fails: the store may read dependencies or mark the handle Failed
        // before it answers (review r1 of #498, item 1, as the spawn counts its archetype load). A retry of a Failed
        // handle returns the same error and writes nothing.
        wrote |= !vm.defs.is_failed(&vm.space, c);
        let result = vm.defs.ensure_loaded(&mut vm.space, c);
        vm.log.extend(vm.defs.take_log());
        if let Err(e) = result {
            let e = VmError::from(e);
            return Err(if wrote { crate::actors::after_writes(e) } else { e });
        }
    }
    Ok(wrote)
}

/// (radius, radius, half height) of a cylinder component.
fn cylinder_extent(vm: &Vm, component: ObjRef, class: &str) -> Result<[f32; 3], VmError> {
    let space = &vm.space;
    if vm.defs.is_unloaded(space, component) {
        // The note loads the `Components` entries of `Actor`'s rule only (section 5, step 5.3).
        return Err(NotYet::not_read(
            format!("{BOUNDING_CYLINDER_OF} {class}: a cylinder component the store has not loaded"),
            EXTENT_RECORD,
        )
        .into());
    }
    let radius = float(space, component, "CollisionRadius")?;
    let height = float(space, component, "CollisionHeight")?;
    Ok([radius, radius, height])
}

/// The template's extent (NATIVE_SPAWN_PLACEMENT.md section 2 and NATIVE_SPAWN_EXTENT.md section 5, lane
/// IMPL-LINK9-EXTENT): (radius, radius, half height). It reads only: the loads it needs ran before it
/// ([`load_extent_components`]).
/// 1. A `CylinderComponent` as the template's `CollisionComponent`: its radius and height.
/// 2. Otherwise the template's class's bounding-cylinder rule, chosen by its **exact** class:
///    - `Pawn`, `WillowPawn`, `WillowAIPawn` and `WillowPlayerPawn` ([`PAWN_RULE_CLASSES`]): a `CylinderComponent` that
///      is set, is not the `CollisionComponent` and is a cylinder component gives its radius and height; otherwise
///      `Actor`'s rule;
///    - `Actor` ([`ACTOR_RULE_CLASS`]): [`actor_rule`];
///    - any other class: `NotYet { NotRead }`, "the bounding-cylinder rule of <class>" (the body was read for these five
///      classes only, and no other class may be assumed to keep its parent's).
///
/// A field the classes do not declare is a `VmError::Script` naming it (the actor field reads of [`crate::actors`]).
pub fn extent(vm: &Vm, template: ObjRef) -> Result<[f32; 3], VmError> {
    let class = class_path(&vm.space, template)?;
    match rule(&vm.space, template)? {
        Rule::Cylinder(c) => cylinder_extent(vm, c, &class),
        Rule::Actor(path) => actor_rule(vm, template, &path),
        Rule::NotRead(path) => Err(NotYet::not_read(format!("{BOUNDING_CYLINDER_OF} {path}"), EXTENT_RECORD).into()),
    }
}

/// `Actor`'s rule (NATIVE_SPAWN_EXTENT.md section 2, in section 5's step 5 order), for each entry of the template's
/// `Components`:
/// 1. None: skipped;
/// 2. not a primitive component (from its class): skipped;
/// 3. loaded before the spot search ([`load_extent_components`]); one still unloaded here is a `VmError::Script`
///    (the caller skipped the loads), and a loaded stand-in with no slots is `NotYet { NotRead }`;
/// 4. `bAttached` clear: skipped;
/// 5. `CollideActors` clear: skipped;
/// 6. the skip flag, the trace filters' unresolved bit ([`crate::trace_filter::SKIP_FLAG`]): the port cannot read it,
///    so an entry that passed steps 2 to 5 is `NotYet { NotRead }` naming it, and no box is built from an unread flag
///    (UNVERIFIED);
/// 7. `Bounds`: not reached while step 6 refuses; [`box_cylinder`] is the box's cylinder for when it is read.
///
/// No qualifying entry gives (0, 0, 0). For the installed class defaults every loaded entry is expected unattached,
/// so none reaches step 6 (UNVERIFIED, note section 4).
fn actor_rule(vm: &Vm, template: ObjRef, class: &str) -> Result<[f32; 3], VmError> {
    let space = &vm.space;
    for c in objects(space, template, "Components")?.into_iter().flatten() {
        if !is_primitive(space, c)? {
            continue;
        }
        if vm.defs.is_unloaded(space, c) {
            return Err(VmError::Script {
                message: format!(
                    "{BOUNDING_CYLINDER_OF} {class}: the Components entry {} was not loaded before the spot search \
                     (placement::load_extent_components)",
                    class_path(space, c)?
                ),
                function: "Actor.Spawn".to_string(),
            });
        }
        let o = space.heap.get(c)?;
        if u32::try_from(o.slots.len()).ok() != space.classes.get_layout(o.class).map(|l| l.slot_count()) {
            return Err(NotYet::not_read(
                format!(
                    "{BOUNDING_CYLINDER_OF} {class}: a Components entry whose properties are not loaded (a stand-in \
                     with no slots): {}",
                    class_path(space, c)?
                ),
                EXTENT_RECORD,
            )
            .into());
        }
        if !flag(space, c, "bAttached")? || !flag(space, c, "CollideActors")? {
            continue;
        }
        return Err(NotYet::not_read(
            format!("{BOUNDING_CYLINDER_OF} {class}: {} on an attached colliding component", trace_filter::SKIP_FLAG),
            EXTENT_RECORD,
        )
        .into());
    }
    Ok(box_cylinder(&[]))
}

/// The cylinder of the union of `boxes`, each (origin, box extent): radius √((width/2)² + (depth/2)²), half height
/// half the height; no box gives (0, 0, 0) (NATIVE_SPAWN_EXTENT.md section 2, step 3).
pub fn box_cylinder(boxes: &[([f32; 3], [f32; 3])]) -> [f32; 3] {
    let Some(&(o, e)) = boxes.first() else {
        return [0.0; 3];
    };
    let (mut lo, mut hi) = ([o[0] - e[0], o[1] - e[1], o[2] - e[2]], [o[0] + e[0], o[1] + e[1], o[2] + e[2]]);
    for &(o, e) in &boxes[1..] {
        for k in 0..3 {
            lo[k] = lo[k].min(o[k] - e[k]);
            hi[k] = hi[k].max(o[k] + e[k]);
        }
    }
    let (w, d, h) = ((hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0, (hi[2] - lo[2]) / 2.0);
    // Square root through ow_core::num (rule N3); the sum is formed in single precision, the root taken in double.
    let r = ow_core::num::sqrt_f64(f64::from(w * w + d * d)) as f32;
    [r, r, h]
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

fn name(space: &ObjectSpace, o: ObjRef) -> Result<String, VmError> {
    Ok(space.names.text(space.heap.get(o)?.name).into_owned())
}

/// A refusal naming the query and why.
fn refusal(query: &str, why: &str) -> VmError {
    NotYet::not_read(format!("{SPOT_SEARCH}: {query}: {why}"), RECORD).into()
}

/// Missing data on a live primitive or its actor (a property not loaded) becomes a named refusal; any other error
/// passes through.
fn data_or_refusal<T>(r: Result<T, VmError>, query: &str, what: &str) -> Result<T, VmError> {
    match r {
        Err(VmError::Heap(ow_object::heap::HeapError::SlotMissing { .. })) => Err(refusal(
            query,
            &format!("the data of the live actor primitive {what}, which is not loaded"),
        )),
        other => other,
    }
}

/// The box `extent` swept from `start` to `end`, as bounds.
fn swept(extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Bounds {
    let mut b = Bounds { min: [0.0; 3], max: [0.0; 3] };
    for k in 0..3 {
        b.min[k] = start[k].min(end[k]) - extent[k];
        b.max[k] = start[k].max(end[k]) + extent[k];
    }
    b
}

/// The length of a sweep from `start` to `end`, in units.
fn sweep_length(start: [f32; 3], end: [f32; 3]) -> f32 {
    let d = [0, 1, 2].map(|k| f64::from(end[k] - start[k]));
    ow_core::num::sqrt_f64(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f32
}

/// How far a sweep walks the actors, as a fraction of the whole sweep (module documentation, step 3): the BSP hit
/// plus 5 units, capped at 1; the whole sweep without a BSP hit.
fn actor_reach(bsp: Option<f32>, start: [f32; 3], end: [f32; 3]) -> f32 {
    let length = sweep_length(start, end);
    match bsp {
        Some(f) if length > 0.0 => (f + trace_filter::PAST_BSP / length).min(1.0),
        Some(f) => f,
        None => 1.0,
    }
}

/// The search's queries: the level part from the service, then the actor walk (module documentation, step 3).
struct Queries<'a> {
    model: &'a dyn SpawnCollision,
    /// The VM whose live actors are walked; `None` walks none (the unit tests below).
    vm: Option<&'a Vm>,
}

impl Queries<'_> {
    /// The actor walk of `kind` over `region`: Ok when no live primitive under an unread rule meets it. It records
    /// no actor hit: every primitive the walk could hit refuses first (module documentation, step 3).
    fn walk(&self, kind: Query, region: Bounds, query: &str) -> Result<Vec<trace_filter::Hit>, VmError> {
        let Some(vm) = self.vm else {
            return Ok(Vec::new());
        };
        let space = &vm.space;
        let Some(primitive) = space.classes.by_path("Engine.PrimitiveComponent") else {
            return Ok(Vec::new());
        };
        let list = actors_of(vm)?.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
        for actor in list {
            let who = name(space, actor)?;
            if data_or_refusal(is_destroyed(space, actor), query, &who)? {
                continue;
            }
            let class = space.heap.get(actor)?.class;
            let mut parts: Vec<ObjRef> = Vec::new();
            if space.prop(class, "CollisionComponent").is_some() {
                parts.extend(data_or_refusal(object(space, actor, "CollisionComponent"), query, &who)?);
            }
            if space.prop(class, "Components").is_some() {
                for c in data_or_refusal(objects(space, actor, "Components"), query, &who)?.into_iter().flatten() {
                    if !parts.contains(&c) {
                        parts.push(c);
                    }
                }
            }
            for part in parts {
                if !space.classes.is_child_of(space.heap.get(part)?.class, primitive) {
                    continue;
                }
                let what = format!("{who}'s {}", name(space, part)?);
                // Bounds first (review r2 of #477): a primitive whose service bounds miss the region leaves the walk
                // before any of its filter data is read, so its unloaded flags cannot refuse the query.
                let bounds = self.model.actor_bounds(space, part)?;
                if bounds.is_some_and(|b| !b.meets(&region)) {
                    continue;
                }
                let Candidate::Unsupported(rule) =
                    data_or_refusal(trace_filter::tested(space, part, actor, kind), query, &what)?
                else {
                    continue;
                };
                let held = if bounds.is_some() { "" } else { ", whose bounds the collision service does not hold" };
                return Err(NotYet::not_read(
                    format!("{SPOT_SEARCH}: {query}: {} (the live actor primitive {what}{held})", rule.rule()),
                    rule.record(),
                )
                .into());
            }
        }
        Ok(Vec::new())
    }

    fn free(&self, extent: [f32; 3], at: [f32; 3], mode: Mode) -> Result<bool, VmError> {
        let complex = matches!(mode, Mode::Template { complex: true });
        let kind = Query::BoxTest { zero_extent: extent == [0.0; 3], complex };
        match self.model.box_test(extent, at, mode)? {
            // Bit 512: the level's block ends the test before any actor is read.
            Test::Blocked => Ok(false),
            Test::Unsupported => Err(unsupported(test_name(mode))),
            Test::Clear => self.walk(kind, swept(extent, at, at), test_name(mode)).map(|_| true),
        }
    }

    /// A sweep of `kind`: the level's hit, the actor walk over the capped segment, then the hit choice.
    fn sweep(&self, kind: Query, extent: [f32; 3], start: [f32; 3], end: [f32; 3], query: &str)
        -> Result<Option<trace_filter::Hit>, VmError> {
        let level = match kind {
            Query::PointSweep => self.model.point_sweep(start, end)?,
            _ => self.model.box_sweep(extent, start, end)?,
        };
        let bsp = match level {
            Sweep::Unsupported => return Err(unsupported(query)),
            Sweep::Clear => None,
            Sweep::Hit { location, normal, fraction } => Some(trace_filter::Hit { location, normal, fraction }),
        };
        let reach = actor_reach(bsp.map(|h| h.fraction), start, end);
        let stop = [0, 1, 2].map(|k| start[k] + (end[k] - start[k]) * reach);
        let segment_hits = self.walk(kind, swept(extent, start, stop), query)?;
        // Actor fractions along the walked segment, rescaled to the whole sweep.
        let actors: Vec<trace_filter::Hit> =
            segment_hits.into_iter().map(|h| trace_filter::Hit { fraction: h.fraction * reach, ..h }).collect();
        let length = sweep_length(start, end);
        trace_filter::choose_sweep_hit(bsp, &actors, length).map_err(|why| refusal(query, why.rule()))
    }

    fn box_sweep(&self, extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Result<Option<([f32; 3], [f32; 3], f32)>, VmError> {
        Ok(self.sweep(Query::BoxSweep, extent, start, end, "a box sweep")?.map(|h| (h.location, h.normal, h.fraction)))
    }

    fn point_hits(&self, start: [f32; 3], end: [f32; 3]) -> Result<bool, VmError> {
        Ok(self.sweep(Query::PointSweep, [0.0; 3], start, end, "a point sweep")?.is_some())
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

/// One box sweep of the search (the level part, then the actor walk over the capped segment), for the crate's
/// tests: the search reaches a sweep only behind a scripted first test.
#[cfg(test)]
pub(crate) fn box_sweep_in(
    vm: &Vm,
    extent: [f32; 3],
    start: [f32; 3],
    end: [f32; 3],
) -> Result<Option<([f32; 3], [f32; 3], f32)>, VmError> {
    let model = collision_model(vm)?.expect("a test installs a service");
    Queries { model: model.as_ref(), vm: Some(vm) }.box_sweep(extent, start, end)
}

/// The spot search for a spawn of `template` at `location` (module documentation, steps 2 and 3): the location to
/// make the actor at, or `None` when the search fails. Reads only; it runs before anything is made.
pub fn spot_search(vm: &Vm, template: ObjRef, location: &Value) -> Result<Option<Value>, VmError> {
    let space = &vm.space;
    let Some(model) = collision_model(vm)? else {
        return Err(NotYet::layer_absent(SPOT_SEARCH, LAYER, RECORD).into());
    };
    let e = extent(vm, template)?;
    let complex = flag(space, template, "bCollideComplex")?;
    let l0 = vector(space, location)?;
    let q = Queries { model: model.as_ref(), vm: Some(vm) };
    match search(&q, e, l0, complex)? {
        None => Ok(None),
        Some(l) if l == l0 => Ok(Some(location.clone())),
        Some(l) => Ok(Some(with_vector(space, location, l)?)),
    }
}

/// The step after `PreBeginPlay` for a live actor that collides with actors (module documentation): the relocation
/// step's `NotYet { NotRead }` for a `bNoCollisionFail` spawn, else the encroachment check
/// ([`encroachment::check`], NATIVE_ENCROACHMENT_CHECK.md) at the spawn's `location` and `rotation`. Unmarked: the
/// spawn's post-insertion boundary marks every refusal after the actor was made.
pub fn after_pre_begin_play(
    vm: &Vm,
    actor: ObjRef,
    no_collision_fail: bool,
    location: &Value,
    rotation: &Value,
) -> Result<(), VmError> {
    if no_collision_fail {
        return Err(NotYet::not_read(RELOCATION, RECORD).into());
    }
    encroachment::check(vm, actor, location, rotation)
}

/// The post-spawn encroachment check (lane IMPL-ENCROACHMENT-CHECK).
#[path = "placement_encroachment.rs"]
pub mod encroachment;

/// The search's steps against a scripted service with the filter taken as read: these steps are not reached in Off
/// (module documentation), so they are tested here directly. Invented geometry only.
#[cfg(test)]
#[path = "placement_extent_tests.rs"]
mod extent_tests;

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

    #[test]
    fn a_sweep_walks_the_actors_to_five_units_past_the_bsp_hit_capped_at_the_end() {
        // A 100-unit sweep: the BSP at 0.5 reaches 0.55; at 0.98 it is capped at 1; with none the whole sweep.
        let (a, b) = ([0.0, 0.0, 0.0], [100.0, 0.0, 0.0]);
        assert!((actor_reach(Some(0.5), a, b) - 0.55).abs() < 1e-6);
        assert_eq!(actor_reach(Some(0.98), a, b), 1.0);
        assert_eq!(actor_reach(None, a, b), 1.0);
        // A zero-length sweep walks no further than its BSP hit.
        assert_eq!(actor_reach(Some(0.0), a, a), 0.0);
        // The walked region is the swept box to that point, and an actor box beyond it is missed.
        let region = swept([1.0, 1.0, 1.0], a, [55.0, 0.0, 0.0]);
        assert_eq!(region, Bounds { min: [-1.0, -1.0, -1.0], max: [56.0, 1.0, 1.0] });
        assert!(!Bounds { min: [60.0, -1.0, -1.0], max: [62.0, 1.0, 1.0] }.meets(&region));
        assert!(Bounds { min: [56.0, -1.0, -1.0], max: [62.0, 1.0, 1.0] }.meets(&region));
    }

    #[test]
    fn a_sweep_with_a_level_hit_and_no_walk_hits_returns_the_level_hit() {
        let s = script(vec![], vec![Sweep::Hit { location: [1.0, 2.0, 3.0], normal: [0.0, 0.0, 1.0], fraction: 0.25 }], Test::Clear);
        let q = Queries { model: &s, vm: None };
        assert_eq!(q.box_sweep(E, L0, [L0[0], L0[1], L0[2] - 40.0]).unwrap(), Some(([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], 0.25)));
        assert_eq!(q.box_sweep(E, L0, [L0[0], L0[1], L0[2] - 40.0]).unwrap(), None);
    }

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
        let q = Queries { model: &s, vm: None };
        assert_eq!(search(&q, E, L0, true).unwrap(), Some(L0));
        assert_eq!(s.asked.borrow()[0], format!("test {E:?} {L0:?} {:?}", Mode::Template { complex: true }));
    }

    #[test]
    fn an_unsupported_query_is_named() {
        let s = script(vec![], vec![], Test::Unsupported);
        let q = Queries { model: &s, vm: None };
        assert!(not_read(search(&q, E, L0, false).unwrap_err()).contains("an axis-aligned box test in template mode"));
    }

    #[test]
    fn a_blocked_zero_extent_fails() {
        let s = script(vec![], vec![], Test::Blocked);
        let q = Queries { model: &s, vm: None };
        assert_eq!(search(&q, [0.0; 3], L0, false).unwrap(), None);
    }

    #[test]
    fn the_slice_step_raises_the_box_off_a_near_floor_and_lowers_it_onto_a_far_one() {
        // t = 0.25: raised by Ez (1 - 2t) + 1, a clear simple test succeeds with no upward sweep.
        let floor = Sweep::Hit { location: [0.0; 3], normal: [0.0, 0.0, 1.0], fraction: 0.25 };
        let s = script(vec![Test::Blocked, Test::Clear, Test::Clear], vec![floor], Test::Blocked);
        let q = Queries { model: &s, vm: None };
        assert_eq!(search(&q, E, L0, false).unwrap(), Some([L0[0], L0[1], L0[2] + 20.0 * 0.5 + 1.0]));
        assert_eq!(s.asked.borrow().len(), 4);
        // t = 0.75: lowered by Ez (2t - 1), raised by 1, then the common tail's test and its upward sweep.
        let floor = Sweep::Hit { location: [0.0; 3], normal: [0.0, 0.0, 1.0], fraction: 0.75 };
        let s = script(vec![Test::Blocked, Test::Clear, Test::Clear], vec![floor], Test::Blocked);
        let q = Queries { model: &s, vm: None };
        let l = [L0[0], L0[1], L0[2] - 20.0 * 0.5 + 1.0];
        assert_eq!(search(&q, E, L0, false).unwrap(), Some(l));
        assert_eq!(*s.asked.borrow().last().unwrap(), format!("sweep {E:?} {l:?} {:?}", [l[0], l[1], l[2] + 20.0]));
    }

    #[test]
    fn a_failed_slice_that_was_not_blocked_fails_the_search() {
        let s = script(vec![Test::Blocked, Test::Clear], vec![], Test::Blocked);
        let q = Queries { model: &s, vm: None };
        assert_eq!(search(&q, E, L0, false).unwrap(), None);
    }

    #[test]
    fn one_free_corner_doubles_the_move() {
        let tests = vec![Test::Blocked, Test::Blocked, Test::Clear, Test::Blocked, Test::Blocked, Test::Blocked, Test::Clear];
        let s = script(tests, vec![], Test::Blocked);
        let q = Queries { model: &s, vm: None };
        // In the code's own order, in single precision: one move of 1.1 x the displacement, then doubled.
        let one = |c: f32| c + 1.1 * (-1.0 * 0.55 * 10.0);
        let twice = |c: f32| 2.0 * one(c) - c;
        assert_eq!(search(&q, E, L0, false).unwrap(), Some([twice(L0[0]), twice(L0[1]), L0[2]]));
        assert!(s.asked.borrow()[6].starts_with("point"), "{:?}", s.asked.borrow());
    }
}
