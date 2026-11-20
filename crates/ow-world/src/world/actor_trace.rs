//! Owner: lane IMPL-ACTOR-TRACE (decision 2026-11-19-245). `Actor.Trace` in `ParityMode::Off`, the one shared
//! implementation both binding sites use (the Phaselock pack set, `ow_skills::action_skill::Pack`, and the go-down
//! worlds, `ow_combat::go_down`). AI-assisted (Claude Code).
//!
//! Coded from `docs/verification/NATIVE_ACTOR_TRACE.md` alone, the note as accepted at 860e5492 (analyst lane
//! ANALYST-ACTOR-TRACE, PR #621; clean room: no native code or analysis output was opened). **Every rule is UNVERIFIED
//! in the running game.** Off only: the binding sites check the mode, and Reference keeps the stub policy and the
//! go-down refusal it had.
//!
//! **What it answers.** Only a miss it can prove: None, with `HitLocation` and `HitNormal` both zero. Everything else
//! is a typed refusal (note section 3), in this order, the first failure being the answer:
//! 1. **T1:** no receiver is a `VmError::Script`; a receiver the store has not loaded, or a stand-in with no slots, is
//!    `NotRead`, "Actor.Trace: the actor <name> is not loaded (a load is a write)".
//! 2. **The arguments:** `TraceEnd`, a supplied `TraceStart` and a supplied `Extent` are structs of the reflected type
//!    `Core.Object.Vector` ([`VECTOR`]; another struct type is refused even with float X, Y and Z), `bTraceActors` a bool, `ExtraTraceFlags` an int or a byte, `bTraceBulletListeners` a bool and
//!    `BulletListenerSource` an object. Another shape is a `VmError::Script`. Omitted ones take the note's defaults:
//!    the receiver's `Location`, its `bCollideActors`, and a zero extent.
//! 3. **T2, an unread option** ([`option_refusal`]): a `HitInfo` argument, `bTraceBulletListeners` set, a set
//!    `BulletListenerSource`, or an `ExtraTraceFlags` bit other than 8.
//! 4. **T3, the source actor:** the receiver's class is `WillowGame.WillowAIPawn` or a child of it that is not one of
//!    the recorded native descendants ([`NATIVE_DESCENDANTS`], from NATIVE_IS_STAGGERED.md); any other class is
//!    `NotRead`, "Actor.Trace: the trace source of <class> (its virtual was not read)". Classes are looked up with the
//!    class table's `by_path` and `is_child_of` only; a class absent from the table is "not this class".
//! 5. **T4:** no collision service is `LayerAbsent`, "Actor.Trace: the world's collision" (go_down's text and layer).
//!    **Then the query's values** ([`BAD_QUERY`], `NotRead`): an endpoint or an extent that is not finite, or a
//!    negative extent, is refused before the service is asked. The game's handling was not read; this is the port's
//!    policy, UNVERIFIED.
//! 6. **The out destinations:** the function declares `HitLocation` and `HitNormal` as `out`, both hold a
//!    `Core.Object.Vector`, and the zero answers are prepared; then each **actual** argument is checked with
//!    `NativeCall::check_out`, which confirms it is `out` and that the write through its alias would be accepted,
//!    writing nothing. Both pass before the first `set_out`, so nothing can fail between the two writes.
//! 7. **The level part** (the service covers the level's BSP only): `point_sweep` for a zero extent, else
//!    `box_sweep`. `Unsupported` is `NotRead`, "Actor.Trace: the level sweep"; a `Hit` is **T6**, `NotRead`, "Actor.Trace:
//!    the actor answered for a level hit (not read)"; `Clear` goes on.
//! 8. **The actor part**, run whenever the flag word ([`trace_flags`]) reaches the collision hash, which both values of
//!    `bTraceActors` do. Every live actor's primitive components (its `CollisionComponent`, then its `Components`), in
//!    the actor list's order, except the source actor's own. For each primitive: it and its owner
//!    ([`trace_filter::owner_of`]) must be resident (else the T1 refusal naming the object); then **the hash
//!    admission's necessary condition** (NATIVE_ACTOR_TRACE.md section 5, lane IMPL-TRACE-HASH-ADMISSION): the
//!    primitive's `CollideActors`, then its owner's `bCollideActors`, each declared as a bool (else
//!    `VmError::Script` naming the class), and either clear leaves the primitive out with nothing more read (its bounds
//!    unasked, so meeting bounds are a miss too), because the collision hash never admits it (UNVERIFIED). Not when
//!    the walked actor or the owner is a terrain: the walk goes on as before. `BulletListener` is not read; missing bounds
//!    (`actor_bounds` None) is **T5**, `LayerAbsent`; bounds that are not finite and ordered are `NotRead`
//!    ([`BAD_BOUNDS`]; they cannot prove a miss, the port's policy, UNVERIFIED); bounds disjoint from the swept box
//!    leave the walk with no filter field read past the two collide flags; bounds that meet it are **T7**, `NotRead`, naming the first unread rule: the terrain handling
//!    for a terrain actor, otherwise the owner filters with a source actor ([`SOURCE_FILTERS`]). The note puts the hash
//!    admission third; it is never reached, since no non-terrain owner gets past the unread source-aware filter.
//!
//! **Nothing is written, loaded or drawn** before the two `set_out` calls of a proven miss, which are the native's
//! only writes. Every read is of a resident object through lookup-only names (`ObjectSpace::prop`, which never
//! interns). The source actor's own primitives are left out of the walk: UNVERIFIED (the note's open question 1).

use ow_core::NotYet;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::actors::{actors_of, is_destroyed, object, objects};
use crate::placement::{Bounds, Sweep, collision_model};
use crate::trace_filter::{
    BLOCKING_ONLY, LEVEL_BSP, MOVERS, OTHER_ACTORS, PAWNS, PROJ_TARGETS, SWEEP_HASH_MASK, TERRAIN, TERRAIN_HANDLING,
    VOLUMES, WORLD_GEOMETRY, owner_of,
};

/// Where these rules are written down.
pub const RECORD: &str = "NATIVE_ACTOR_TRACE.md";
/// The function path both binding sites bind.
pub const PATH: &str = "Engine.Actor.Trace";
/// T4's text and layer (go_down's).
pub const NO_COLLISION: &str = "Actor.Trace: the world's collision";
pub const COLLISION_LAYER: &str = "the world's collision";
/// The level part's refusals.
pub const LEVEL_SWEEP: &str = "Actor.Trace: the level sweep";
pub const LEVEL_HIT: &str = "Actor.Trace: the actor answered for a level hit (not read)";
/// T5's text and layer (the pickup targeting's layer text).
pub const NO_BOUNDS: &str = "Actor.Trace: the colliding components' bounds";
pub const BOUNDS_LAYER: &str = "component bounds (the headless world has none)";
/// T7's rule for a primitive that is not a terrain's.
pub const SOURCE_FILTERS: &str = "the owner filters with a source actor";
/// The class whose source virtual was read (T3), and its recorded native descendants, which refuse.
pub const SOURCE_CLASS: &str = "WillowGame.WillowAIPawn";
pub const NATIVE_DESCENDANTS: [&str; 2] = ["WillowGame.WillowAICranePawn", "WillowGame.DeployableTurretActor"];
/// The reflected type every vector argument must have (review r1 of #633, item 3).
pub const VECTOR: &str = "Core.Object.Vector";
/// The refusal for a query whose endpoints or extent are not finite, or whose extent is negative (review r1 of #633,
/// item 2). The game's handling of such a query was not read; refusing it is the port's policy, UNVERIFIED.
pub const BAD_QUERY: &str = "Actor.Trace: a query with a non-finite endpoint or extent, or a negative extent (its handling was not read)";
/// The refusal for primitive bounds that are not finite and ordered (review r1 of #633, item 2): they cannot prove
/// that the primitive is clear of the swept box.
pub const BAD_BOUNDS: &str = "Actor.Trace: a primitive's bounds that are not finite and ordered (they cannot prove a miss)";
/// The one `ExtraTraceFlags` bit whose effect was read: the blocking bit.
pub const EXTRA_BLOCKING: i64 = 8;

/// The parameters, by index.
const HIT_LOCATION: usize = 0;
const HIT_NORMAL: usize = 1;
const TRACE_END: usize = 2;
const TRACE_START: usize = 3;
const TRACE_ACTORS: usize = 4;
const EXTENT: usize = 5;
const HIT_INFO: usize = 6;
const EXTRA_FLAGS: usize = 7;
const BULLET_LISTENERS: usize = 8;
const LISTENER_SOURCE: usize = 9;

fn script(message: String) -> VmError {
    VmError::Script { function: "Actor.Trace".to_string(), message }
}

/// The world flag word for `trace_actors` and `extra` (note section 1). Bit 8 of `extra` swaps the projectile-targets
/// bit for blocking-only; the other extra bits are refused before this is asked (T2).
pub fn trace_flags(trace_actors: bool, extra: i64) -> u32 {
    let base = MOVERS | LEVEL_BSP | WORLD_GEOMETRY | TERRAIN;
    if !trace_actors {
        return base;
    }
    let narrowing = if extra & EXTRA_BLOCKING != 0 { BLOCKING_ONLY } else { PROJ_TARGETS };
    base | PAWNS | VOLUMES | OTHER_ACTORS | narrowing
}

/// T2's refusal for `option`.
pub fn option_refusal(option: &str) -> VmError {
    NotYet::not_read(format!("Actor.Trace: {option} (its effect on the query was not read)"), RECORD).into()
}

/// The T1 refusal naming `o`, unless it is resident: neither a handle the store has not loaded nor a stand-in with no
/// slots. Never a load.
fn resident(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    let space = &vm.space;
    let obj = space.heap.get(o)?;
    let full = u32::try_from(obj.slots.len()).ok() == space.classes.get_layout(obj.class).map(|l| l.slot_count());
    if vm.defs.is_unloaded(space, o) || !full {
        let name = space.names.text(obj.name);
        return Err(NotYet::not_read(format!("Actor.Trace: {what} {name} is not loaded (a load is a write)"), RECORD).into());
    }
    Ok(())
}

/// The X, Y and Z of a `Core.Object.Vector` struct value, by field name (lookup only). A struct of any other
/// reflected type is a `VmError::Script`, even one with float X, Y and Z (review r1 of #633, item 3).
fn vector(space: &ObjectSpace, v: &Value, what: &str) -> Result<[f32; 3], VmError> {
    let s = v.as_struct().ok_or_else(|| script(format!("{what} is a {}, not a vector", v.kind_name())))?;
    if space.classes.struct_by_path(VECTOR) != Some(s.ty) {
        return Err(script(format!("{what} is a struct of another type, not a {VECTOR}")));
    }
    let layout = space.classes.struct_layout(s.ty).ok_or_else(|| script(format!("{what} has no struct layout")))?;
    let mut out = [0.0; 3];
    for (k, n) in ["X", "Y", "Z"].iter().enumerate() {
        let field = space.names.find(n).and_then(|id| layout.field(id)).map(|(entry, _)| entry);
        let at = field.and_then(|e| usize::try_from(e).ok()).ok_or_else(|| script(format!("{what} has no field {n}")))?;
        out[k] = match s.fields.get(at) {
            Some(Value::Float(f)) => *f,
            _ => return Err(script(format!("{what}.{n} is not a float"))),
        };
    }
    Ok(out)
}

/// `v` (a `Vector` struct) with X, Y and Z zero: the out answer of a miss.
fn zeroed(space: &ObjectSpace, v: &Value, what: &str) -> Result<Value, VmError> {
    vector(space, v, what)?;
    let mut out = v.clone();
    let ty = out.as_struct().expect("checked by vector").ty;
    let layout = space.classes.struct_layout(ty).expect("checked by vector");
    let at: Vec<usize> = ["X", "Y", "Z"]
        .iter()
        .filter_map(|n| space.names.find(n).and_then(|id| layout.field(id)).and_then(|(e, _)| usize::try_from(e).ok()))
        .collect();
    let s = out.struct_mut().expect("checked by vector");
    for i in at {
        s.fields[i] = Value::Float(0.0);
    }
    Ok(out)
}

/// The receiver's own field `name` (resident, lookup-only name); a field its class does not declare is a
/// `VmError::Script`.
fn own_field(space: &ObjectSpace, this: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = space.heap.get(this)?.class;
    let p = space
        .prop(class, name)
        .ok_or_else(|| script(format!("{} declares no {name}", ow_attr::eval::class_path(space, this))))?;
    Ok(space.read(this, PropAddr::scalar(p))?.clone())
}

/// The query the preflight settled.
struct Query {
    start: [f32; 3],
    end: [f32; 3],
    extent: [f32; 3],
    flags: u32,
}

/// The native (module documentation).
pub fn trace(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let this = c.this.ok_or_else(|| script("called with no actor".to_string()))?;
    let (query, outs) = preflight(c, this)?;
    answer(c.vm, this, &query)?;
    let [location, normal] = outs;
    c.set_out(HIT_LOCATION, location)?;
    c.set_out(HIT_NORMAL, normal)?;
    Ok(Value::Object(None))
}

/// Steps 1 to 6 of the module documentation: the query and the two prepared out answers. Reads only.
fn preflight(c: &NativeCall<'_>, this: ObjRef) -> Result<(Query, [Value; 2]), VmError> {
    let vm: &Vm = c.vm;
    let space = &vm.space;
    resident(vm, this, "the actor")?;
    // Step 2, the arguments' shapes.
    let supplied = |i: usize| c.has(i);
    let end = vector(space, c.arg(TRACE_END)?, "TraceEnd")?;
    let start = if supplied(TRACE_START) { Some(vector(space, c.arg(TRACE_START)?, "TraceStart")?) } else { None };
    let trace_actors = if supplied(TRACE_ACTORS) {
        match c.arg(TRACE_ACTORS)? {
            Value::Bool(b) => Some(*b),
            other => return Err(script(format!("bTraceActors is a {}, not a bool", other.kind_name()))),
        }
    } else {
        None
    };
    let extent = if supplied(EXTENT) { vector(space, c.arg(EXTENT)?, "Extent")? } else { [0.0; 3] };
    let extra = if supplied(EXTRA_FLAGS) {
        match c.arg(EXTRA_FLAGS)? {
            Value::Int(i) => i64::from(*i),
            Value::Byte(b) => i64::from(*b),
            other => return Err(script(format!("ExtraTraceFlags is a {}, not an int", other.kind_name()))),
        }
    } else {
        0
    };
    let listeners = if supplied(BULLET_LISTENERS) {
        match c.arg(BULLET_LISTENERS)? {
            Value::Bool(b) => *b,
            other => return Err(script(format!("bTraceBulletListeners is a {}, not a bool", other.kind_name()))),
        }
    } else {
        false
    };
    let listener_source = if supplied(LISTENER_SOURCE) {
        match c.arg(LISTENER_SOURCE)? {
            Value::Object(o) => *o,
            other => return Err(script(format!("BulletListenerSource is a {}, not an object", other.kind_name()))),
        }
    } else {
        None
    };
    // Step 3, T2.
    if supplied(HIT_INFO) {
        return Err(option_refusal("a HitInfo argument"));
    }
    if listeners {
        return Err(option_refusal("bTraceBulletListeners set"));
    }
    if listener_source.is_some() {
        return Err(option_refusal("a BulletListenerSource"));
    }
    if extra & !EXTRA_BLOCKING != 0 {
        return Err(option_refusal(&format!("ExtraTraceFlags {extra} (a bit other than {EXTRA_BLOCKING})")));
    }
    // Step 4, T3.
    let class = space.heap.get(this)?.class;
    let child_of = |p: &str| space.classes.by_path(p).is_some_and(|k| space.classes.is_child_of(class, k));
    if !child_of(SOURCE_CLASS) || NATIVE_DESCENDANTS.iter().any(|p| child_of(p)) {
        return Err(NotYet::not_read(
            format!("Actor.Trace: the trace source of {} (its virtual was not read)", ow_attr::eval::class_path(space, this)),
            RECORD,
        )
        .into());
    }
    // Step 5, T4.
    if collision_model(vm)?.is_none() {
        return Err(NotYet::layer_absent(NO_COLLISION, COLLISION_LAYER, RECORD).into());
    }
    // The defaults (note section 1), read from the resident receiver.
    let start = match start {
        Some(s) => s,
        None => vector(space, &own_field(space, this, "Location")?, "Location")?,
    };
    let trace_actors = match trace_actors {
        Some(b) => b,
        None => match own_field(space, this, "bCollideActors")? {
            Value::Bool(b) => b,
            other => return Err(script(format!("bCollideActors is a {}, not a bool", other.kind_name()))),
        },
    };
    // A query the port cannot reason about (review r1 of #633, item 2): a NaN or an infinity would make the box
    // arithmetic below unreliable, and a negative extent inverts the swept box. Refused before the service is asked.
    if !start.iter().chain(&end).chain(&extent).all(|v| v.is_finite()) || extent.iter().any(|&e| e < 0.0) {
        return Err(NotYet::not_read(BAD_QUERY, RECORD).into());
    }
    // Step 6, the out destinations, before any write.
    let outs = vm.function(c.function).map(|f| [HIT_LOCATION, HIT_NORMAL].iter().all(|&i| f.params.get(i).is_some_and(|p| p.is_out())));
    if c.arg_count() <= HIT_NORMAL || outs != Some(true) {
        return Err(script("the function does not declare HitLocation and HitNormal as out".to_string()));
    }
    let location = zeroed(space, c.arg(HIT_LOCATION)?, "HitLocation")?;
    let normal = zeroed(space, c.arg(HIT_NORMAL)?, "HitNormal")?;
    // The actual arguments, not only the declaration (review r1 of #633, item 1): each is out, and the write through
    // its alias would be accepted. Both are checked before either is written, so the two `set_out` calls cannot fail
    // between them.
    // Both writes are vectors, which change no array and no object reference, so the first cannot invalidate the
    // second's destination, even when both alias one place (review r2 of #633; `check_out` checks the current
    // destination only).
    c.check_out(HIT_LOCATION, &location)?;
    c.check_out(HIT_NORMAL, &normal)?;
    Ok((Query { start, end, extent, flags: trace_flags(trace_actors, extra) }, [location, normal]))
}

/// The box `extent` swept from `start` to `end`, as bounds.
fn swept(q: &Query) -> Bounds {
    let mut b = Bounds { min: [0.0; 3], max: [0.0; 3] };
    for k in 0..3 {
        b.min[k] = q.start[k].min(q.end[k]) - q.extent[k];
        b.max[k] = q.start[k].max(q.end[k]) + q.extent[k];
    }
    b
}

/// Steps 7 and 8: Ok when the trace is a provable miss.
fn answer(vm: &Vm, source: ObjRef, q: &Query) -> Result<(), VmError> {
    let model = collision_model(vm)?.ok_or_else(|| NotYet::layer_absent(NO_COLLISION, COLLISION_LAYER, RECORD))?;
    let level = if q.extent == [0.0; 3] {
        model.point_sweep(q.start, q.end)?
    } else {
        model.box_sweep(q.extent, q.start, q.end)?
    };
    match level {
        Sweep::Unsupported => return Err(NotYet::not_read(LEVEL_SWEEP, RECORD).into()),
        Sweep::Hit { .. } => return Err(NotYet::not_read(LEVEL_HIT, RECORD).into()),
        Sweep::Clear => {}
    }
    if q.flags & SWEEP_HASH_MASK == 0 {
        return Ok(());
    }
    let space = &vm.space;
    let Some(primitive) = space.classes.by_path("Engine.PrimitiveComponent") else {
        return Ok(());
    };
    let terrain = space.classes.by_path("Engine.Terrain");
    let is_terrain = |o: ObjRef| -> Result<bool, VmError> {
        Ok(terrain.is_some_and(|t| space.heap.get(o).is_ok_and(|x| space.classes.is_child_of(x.class, t))))
    };
    let region = swept(q);
    let list = actors_of(vm)?.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
    for actor in list {
        if actor == source {
            continue;
        }
        resident(vm, actor, "the actor")?;
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
            resident(vm, part, "the component")?;
            let owner = owner_of(space, part, actor)?;
            resident(vm, owner, "the actor")?;
            if owner == source {
                continue;
            }
            // NATIVE_ACTOR_TRACE.md section 5: a primitive the collision hash cannot admit is met by no query. Its own
            // `CollideActors` first, then its owner's `bCollideActors`; either clear leaves it out, unread further.
            // Not for terrain: when the walked actor or the owner is a terrain, the walk goes on as before (T7 names
            // the terrain handling).
            let terrain_part = is_terrain(actor)? || is_terrain(owner)?;
            if !terrain_part
                && (!collide_flag(space, part, "CollideActors")? || !collide_flag(space, owner, "bCollideActors")?)
            {
                continue;
            }
            let Some(bounds) = model.actor_bounds(space, part)? else {
                return Err(NotYet::layer_absent(NO_BOUNDS, BOUNDS_LAYER, RECORD).into());
            };
            // Only finite, ordered bounds can prove a primitive clear (review r1 of #633, item 2): a NaN makes every
            // comparison false, and an inverted box would miss everything.
            let sound = (0..3).all(|k| bounds.min[k].is_finite() && bounds.max[k].is_finite() && bounds.min[k] <= bounds.max[k]);
            if !sound {
                return Err(NotYet::not_read(BAD_BOUNDS, RECORD).into());
            }
            if !bounds.meets(&region) {
                continue;
            }
            let rule = if terrain_part { TERRAIN_HANDLING } else { SOURCE_FILTERS };
            let who = space.names.text(space.heap.get(actor)?.name);
            let what = space.names.text(space.heap.get(part)?.name);
            return Err(NotYet::not_read(format!("Actor.Trace: {rule} (the live actor primitive {who}'s {what})"), RECORD).into());
        }
    }
    Ok(())
}

/// The bool `name` on `o` (NATIVE_ACTOR_TRACE.md section 5): its class must declare it as a bool, else a
/// `VmError::Script` naming the class. Read-only, through lookup-only names.
fn collide_flag(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<bool, VmError> {
    let class = space.heap.get(o)?.class;
    let path = || space.classes.layout(class).path().to_string();
    let Some(p) = space.prop(class, name) else {
        return Err(script(format!("{} declares no {name}", path())));
    };
    if !matches!(space.classes.decl(p).map(|d| &d.kind), Some(ow_object::layout::PropKind::Bool)) {
        return Err(script(format!("{} declares {name} as something other than a bool", path())));
    }
    match space.read(o, ow_object::ids::PropAddr::scalar(p))? {
        Value::Bool(b) => Ok(*b),
        other => Err(script(format!("{}.{name} holds {}, not a bool", path(), other.kind_name()))),
    }
}

#[cfg(test)]
#[path = "actor_trace_tests.rs"]
mod tests;
