//! Owner: lane IMPL-LINK1-TAIL-SETTERS. The collision setters of `SetupSkeletalMeshPhysics` in `ParityMode::Off`:
//! `PrimitiveComponent.SetRBChannel`, `SetBlockRigidBody`, `SetActorCollision`, `SetTraceBlocking` and
//! `SetNotifyRigidBodyCollision`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK1_TAIL_SETTERS.md sections 2.D to 2.H alone (lane
//! ANALYST-LINK1-TAIL, #734 with its review r1, on main); this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference keeps what it has (the stub policy, and `ow-combat` `death.rs`'s
//! `SetRBChannel` no-op, which Off skips).
//!
//! **Physics bodies** (2.D, 2.E, 2.H): on a `SkeletalMeshComponent` its `PhysicsAssetInstance` set; on any other
//! component its `BodyInstance` set. The field is checked by its declaration (an object reference, else
//! `VmError::Script`) and read as a handle, never followed. With bodies, the physics part is [`RB_FILTER`]
//! (`LayerAbsent`, [`RB_LAYER`]) before any write; with none, the game's own gate makes it nothing.
//! - `SetRBChannel(Channel)`: `RBChannel` (a byte) := the channel;
//! - `SetBlockRigidBody(b)`: `BlockRigidBody` := b;
//! - `SetNotifyRigidBodyCollision(b)`: `bNotifyRigidBodyCollision` := b.
//!
//! **`SetTraceBlocking(z, nz)`** (2.G): `BlockZeroExtent` := z, `BlockNonZeroExtent` := nz; nothing else.
//!
//! **`SetActorCollision(collide, block, optional always)`** (2.F), all or nothing by a preflight:
//! `AlwaysCheckCollision` := `always` (false when omitted); when `CollideActors` differs from `collide`,
//! `CollideActors` := it and the deferred reattach (`bNeedsReattach` := true; the owner rule's `REATTACH_OWNER`, or with
//! no owner `REATTACH_SCENE` for a non-zero `Scene`, refused first); a change with `collide` and `always` set and an
//! `Owner` also reaches an unread owner step ([`OWNER_STEP`], `NotRead`); then `BlockActors` := `block`.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::component_setters::{REATTACH_FIELDS, Slots, Want, bad, bool_of, declared, object_of, reattach_check, schemas, write};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINK1_TAIL_SETTERS.md";
/// The native keys.
pub const SET_RB_CHANNEL: &str = "PrimitiveComponent.SetRBChannel(byte)";
pub const SET_BLOCK_RIGID_BODY: &str = "PrimitiveComponent.SetBlockRigidBody(bool)";
pub const SET_ACTOR_COLLISION: &str = "PrimitiveComponent.SetActorCollision(bool,bool,bool)";
pub const SET_TRACE_BLOCKING: &str = "PrimitiveComponent.SetTraceBlocking(bool,bool)";
pub const SET_NOTIFY_RIGID_BODY_COLLISION: &str = "PrimitiveComponent.SetNotifyRigidBodyCollision(bool)";
/// The physics bodies' part of 2.D, 2.E and 2.H.
pub const RB_FILTER: &str = "the rigid-body collision filter update";
/// The layer [`RB_FILTER`] names.
pub const RB_LAYER: &str = "a physics layer (rigid bodies)";
/// 2.F's unread owner step.
pub const OWNER_STEP: &str =
    "PrimitiveComponent.SetActorCollision: the owner step when CollideActors and AlwaysCheckCollision are set";
const SKELETAL: &str = "Engine.SkeletalMeshComponent";

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    Slots::of(o).required(vm, name, "PrimitiveComponent")
}

fn component(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without a component"))
}

fn bool_arg(c: &NativeCall<'_>, i: usize, what: &str) -> Result<bool, VmError> {
    if !c.has(i) {
        return Ok(false);
    }
    bool_of(c.arg(i)?, &format!("argument {i}"), what)
}

/// Whether `o` has physics bodies (module documentation), read as a handle.
fn bodies_present(vm: &Vm, o: ObjRef, what: &str) -> Result<bool, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let skeletal = vm.space.classes.by_path(SKELETAL).is_some_and(|s| vm.space.classes.is_child_of(class, s));
    let field = if skeletal { "PhysicsAssetInstance" } else { "BodyInstance" };
    schemas(vm, o, &[(field, Want::Object)], what)?;
    Ok(object_of(&read(vm, o, field)?, what)?.is_some())
}

/// 2.D, 2.E and 2.H: `field` := `v` (declared as `kind`), refused first when the component has physics bodies.
fn body_setter(vm: &mut Vm, o: ObjRef, field: &str, kind: Want, v: Value, what: &str) -> Result<(), VmError> {
    schemas(vm, o, &[(field, kind)], what)?;
    if bodies_present(vm, o, what)? {
        return Err(NotYet::layer_absent(RB_FILTER, RB_LAYER, RECORD).into());
    }
    write(vm, o, field, v, what)
}

/// `PrimitiveComponent.SetRBChannel(byte Channel)` (2.D).
pub fn set_rb_channel(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "PrimitiveComponent.SetRBChannel";
    let o = component(c, WHAT)?;
    let channel = match c.arg(0)? {
        Value::Byte(b) => *b,
        other => return Err(bad(WHAT, format!("Channel holds {}, not a byte", other.kind_name()))),
    };
    if !matches!(declared(c.vm, o, "RBChannel", WHAT)?, PropKind::Byte { .. }) {
        return Err(bad(WHAT, "RBChannel is not declared as a byte"));
    }
    if bodies_present(c.vm, o, WHAT)? {
        return Err(NotYet::layer_absent(RB_FILTER, RB_LAYER, RECORD).into());
    }
    write(c.vm, o, "RBChannel", Value::Byte(channel), WHAT)?;
    Ok(Value::None)
}

/// `PrimitiveComponent.SetBlockRigidBody(bool)` (2.E).
pub fn set_block_rigid_body(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "PrimitiveComponent.SetBlockRigidBody";
    let o = component(c, WHAT)?;
    let b = bool_arg(c, 0, WHAT)?;
    body_setter(c.vm, o, "BlockRigidBody", Want::Bool, Value::Bool(b), WHAT)?;
    Ok(Value::None)
}

/// `PrimitiveComponent.SetNotifyRigidBodyCollision(bool)` (2.H).
pub fn set_notify_rigid_body_collision(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "PrimitiveComponent.SetNotifyRigidBodyCollision";
    let o = component(c, WHAT)?;
    let b = bool_arg(c, 0, WHAT)?;
    body_setter(c.vm, o, "bNotifyRigidBodyCollision", Want::Bool, Value::Bool(b), WHAT)?;
    Ok(Value::None)
}

/// `PrimitiveComponent.SetTraceBlocking(bool, bool)` (2.G).
pub fn set_trace_blocking(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "PrimitiveComponent.SetTraceBlocking";
    let o = component(c, WHAT)?;
    let (z, nz) = (bool_arg(c, 0, WHAT)?, bool_arg(c, 1, WHAT)?);
    schemas(c.vm, o, &[("BlockZeroExtent", Want::Bool), ("BlockNonZeroExtent", Want::Bool)], WHAT)?;
    write(c.vm, o, "BlockZeroExtent", Value::Bool(z), WHAT)?;
    write(c.vm, o, "BlockNonZeroExtent", Value::Bool(nz), WHAT).map_err(super::component_setters::after_writes)?;
    Ok(Value::None)
}

/// `PrimitiveComponent.SetActorCollision(bool, bool, optional bool)` (2.F).
pub fn set_actor_collision(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "PrimitiveComponent.SetActorCollision";
    let o = component(c, WHAT)?;
    let (collide, block, always) = (bool_arg(c, 0, WHAT)?, bool_arg(c, 1, WHAT)?, bool_arg(c, 2, WHAT)?);
    // Preflight (all or nothing).
    schemas(
        c.vm,
        o,
        &[("CollideActors", Want::Bool), ("BlockActors", Want::Bool), ("AlwaysCheckCollision", Want::Bool)],
        WHAT,
    )?;
    let change = bool_of(&read(c.vm, o, "CollideActors")?, "CollideActors", WHAT)? != collide;
    if change {
        schemas(c.vm, o, &REATTACH_FIELDS, WHAT)?;
        reattach_check(c.vm, Slots::of(o), WHAT)?;
        let owner = object_of(&read(c.vm, o, "Owner")?, WHAT)?;
        if collide && always && owner.is_some() {
            return Err(NotYet::not_read(OWNER_STEP, RECORD).into());
        }
    }
    // Writes.
    write(c.vm, o, "AlwaysCheckCollision", Value::Bool(always), WHAT)?;
    let rest = (|| {
        if change {
            write(c.vm, o, "CollideActors", Value::Bool(collide), WHAT)?;
            write(c.vm, o, "bNeedsReattach", Value::Bool(true), WHAT)?;
        }
        write(c.vm, o, "BlockActors", Value::Bool(block), WHAT)
    })();
    rest.map_err(super::component_setters::after_writes)?;
    Ok(Value::None)
}

/// Binds the five keys (Off only; the caller checks the mode). A key bound first keeps its binding; `death.rs` skips
/// its `SetRBChannel` no-op in Off.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    for (key, f) in [
        (SET_RB_CHANNEL, set_rb_channel as fn(&mut NativeCall<'_>) -> Result<Value, VmError>),
        (SET_BLOCK_RIGID_BODY, set_block_rigid_body),
        (SET_ACTOR_COLLISION, set_actor_collision),
        (SET_TRACE_BLOCKING, set_trace_blocking),
        (SET_NOTIFY_RIGID_BODY_COLLISION, set_notify_rigid_body_collision),
    ] {
        if !vm.has_native(key) {
            vm.register_native(key, Rc::new(f))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "collision_setters_tests.rs"]
mod tests;
