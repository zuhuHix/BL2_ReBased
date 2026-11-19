//! Owner: lane IMPL-ENCROACHMENT-CHECK. The spawn's post-spawn encroachment check in `ParityMode::Off`, from
//! NATIVE_ENCROACHMENT_CHECK.md (analyst lane ANALYST-ENCROACHMENT-CHECK, PR #490) alone, section 3, the implementer
//! half of the clean room. **Every rule here is UNVERIFIED in the running game.** AI-assisted (Claude Code).
//!
//! The spawn calls [`check`] on the live actor after `PreBeginPlay`, only when `bNoCollisionFail` is false and the
//! live actor's `bCollideActors` is set (the gate, note section 3 step 2: clear passes with no write, a reading). The
//! check comes after the actor was made, so it returns its refusals **unmarked**: the spawn's post-insertion boundary
//! adds "stopped after partial writes: " and keeps the kind and record. In the note's order, before any write of its
//! own:
//! 1. no collision service: `NotYet { LayerAbsent }`, "the post-spawn encroachment check" (main's answer, kept);
//! 2. **the attach step**: any primitive of the live actor (its `CollisionComponent`, or an entry of `Components`)
//!    with `CollideActors` set is `NotYet { NotRead }` ([`ATTACH_STEP`]). The port does not model the per-component
//!    attach step the game runs before the check, so an unattached primitive is never taken to pass. A primitive the
//!    store has not loaded cannot show its flag, so it refuses the same way;
//! 3. **the location step**: the spawn's passed location and rotation against the live actor's `Location` and
//!    `Rotation` after `PreBeginPlay`. Any difference is `NotYet { NotRead }` ([`MOVE_STEP`]), whatever the
//!    `Physics` value (the note's open question 3);
//! 4. **the shape**: the live actor's bounding cylinder ([`super::extent`]), with its own refusals;
//! 5. **the level part**: the service's box test with that extent at the location (in the spawn's template mode,
//!    `bCollideComplex` of the live actor; the note names no mode, UNVERIFIED). Clear goes on; Blocked or Unsupported
//!    is `NotYet { NotRead }` ([`LEVEL_PART`]);
//! 6. **the candidates**: with no colliding primitive of the live actor, the overlap set is empty and the check passes
//!    with no write. With step 2 refusing every primitive with `CollideActors`, that is the only answer reached today.
//!    Once the attach step is read, the port walks the live actors itself ([`walk`], as the spot search's actor walk
//!    does): the actor itself, a destroyed or pending-delete actor and an actor based on the new one are skipped; a
//!    primitive whose bounds the service does not report is `NotYet { NotRead }` ([`OVERLAP_QUERY`]); bounds that miss
//!    the region leave the walk; bounds that meet it are `NotYet { NotRead }` ([`PER_CLASS_TEST`]).

use ow_core::NotYet;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::{Bounds, ENCROACHMENT, LAYER, Mode, RECORD, SpawnCollision, Test, collision_model, extent, name, vector};
use crate::actors::{actors_of, field, flag, is_destroyed, object, objects};

/// The record of these rules.
pub const RECORD_ENCROACHMENT: &str = "NATIVE_ENCROACHMENT_CHECK.md";
/// The note's names (section 3).
pub const ATTACH_STEP: &str = "the encroachment check: the new actor's per-component attach step (which primitives it attaches)";
pub const MOVE_STEP: &str = "the encroachment check's move to the spawn location";
pub const LEVEL_PART: &str = "the encroachment check: whether its query tests the level's BSP";
pub const OVERLAP_QUERY: &str = "the encroachment check's overlap query";
pub const PER_CLASS_TEST: &str = "the encroachment check's per-class blocking test";

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD_ENCROACHMENT).into()
}

/// `actor`'s primitive components: its `CollisionComponent`, then each new non-None entry of `Components`, by class.
/// A field `actor`'s class does not declare is left out when `tolerant`, else a `VmError::Script` naming it.
fn primitives(space: &ObjectSpace, actor: ObjRef, tolerant: bool) -> Result<Vec<ObjRef>, VmError> {
    let Some(primitive) = space.classes.by_path("Engine.PrimitiveComponent") else {
        return Ok(Vec::new());
    };
    let class = space.heap.get(actor)?.class;
    let mut parts: Vec<ObjRef> = Vec::new();
    if !tolerant || space.prop(class, "CollisionComponent").is_some() {
        parts.extend(object(space, actor, "CollisionComponent")?);
    }
    if !tolerant || space.prop(class, "Components").is_some() {
        for c in objects(space, actor, "Components")?.into_iter().flatten() {
            if !parts.contains(&c) {
                parts.push(c);
            }
        }
    }
    let mut out = Vec::new();
    for p in parts {
        if space.classes.is_child_of(space.heap.get(p)?.class, primitive) {
            out.push(p);
        }
    }
    Ok(out)
}

/// The encroachment check on the live `actor` the spawn placed at `location` with `rotation` (module documentation).
/// `Ok(())` is no encroachment, with nothing written. Its refusals are unmarked: the spawn marks them.
pub fn check(vm: &Vm, actor: ObjRef, location: &Value, rotation: &Value) -> Result<(), VmError> {
    let space = &vm.space;
    // Step 3.1: no service.
    let Some(model) = collision_model(vm)? else {
        return Err(NotYet::layer_absent(ENCROACHMENT, LAYER, RECORD).into());
    };
    // Step 3.2: the attach step, before any `bAttached` read.
    let parts = primitives(space, actor, false)?;
    let mut colliding = Vec::new();
    for &p in &parts {
        if vm.defs.is_unloaded(space, p) {
            return Err(not_read(format!("{ATTACH_STEP}: {}, which the store has not loaded", name(space, p)?)));
        }
        if flag(space, p, "CollideActors")? {
            colliding.push(p);
        }
    }
    if let Some(&p) = colliding.first() {
        return Err(not_read(format!("{ATTACH_STEP}: {}", name(space, p)?)));
    }
    // Step 3.3: the location step.
    if field(space, actor, "Location")? != *location || field(space, actor, "Rotation")? != *rotation {
        return Err(not_read(MOVE_STEP));
    }
    // Step 3.4: the shape, the live actor's own.
    let e = extent(vm, actor)?;
    // Step 3.5: the level part.
    let at = vector(space, location)?;
    let complex = flag(space, actor, "bCollideComplex")?;
    if model.box_test(e, at, Mode::Template { complex })? != Test::Clear {
        return Err(not_read(LEVEL_PART));
    }
    // Step 3.6.1: no colliding primitive, an empty overlap set.
    if colliding.is_empty() {
        return Ok(());
    }
    let region = Bounds { min: [at[0] - e[0], at[1] - e[1], at[2] - e[2]], max: [at[0] + e[0], at[1] + e[1], at[2] + e[2]] };
    walk(vm, model.as_ref(), actor, region)
}

/// Whether `other` is based on `actor`, directly or through its base chain (bounded at 256 links).
fn based_on(space: &ObjectSpace, other: ObjRef, actor: ObjRef) -> Result<bool, VmError> {
    let mut cursor = other;
    for _ in 0..256 {
        let class = space.heap.get(cursor)?.class;
        if space.prop(class, "Base").is_none() {
            return Ok(false);
        }
        match object(space, cursor, "Base")? {
            Some(b) if b == actor => return Ok(true),
            Some(b) => cursor = b,
            None => return Ok(false),
        }
    }
    Ok(false)
}

/// The port's own overlap walk over the live actors for `actor`'s check over `region` (module documentation, step
/// 6): `Ok(())` when nothing overlaps. Reached only once the attach step is read; tested directly.
pub(crate) fn walk(vm: &Vm, model: &dyn SpawnCollision, actor: ObjRef, region: Bounds) -> Result<(), VmError> {
    let space = &vm.space;
    let list = actors_of(vm)?.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
    for other in list {
        if other == actor || is_destroyed(space, other)? {
            continue;
        }
        let class = space.heap.get(other)?.class;
        if space.prop(class, "bPendingDelete").is_some() && flag(space, other, "bPendingDelete")? {
            continue;
        }
        if based_on(space, other, actor)? {
            continue;
        }
        let who = name(space, other)?;
        for part in primitives(space, other, true)? {
            match model.actor_bounds(space, part)? {
                Some(b) if !b.meets(&region) => continue,
                Some(_) => {
                    return Err(not_read(format!("{PER_CLASS_TEST} (the live actor primitive {who}'s {})", name(space, part)?)));
                }
                None => {
                    return Err(not_read(format!(
                        "{OVERLAP_QUERY} (the live actor primitive {who}'s {}, whose bounds the collision service does \
                         not hold)",
                        name(space, part)?
                    )));
                }
            }
        }
    }
    Ok(())
}
