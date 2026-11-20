//! Owner: lane IMPL-TOUCH-PICKUP-TRACE. AI-assisted (Claude Code, Opus 5.5). The line check under
//! `DroppedPickup.TouchPickupTrace` in `ParityMode::Off`, coded from `docs/verification/NATIVE_TOUCH_PICKUP_TRACE.md`
//! section 4 (#624, accepted at 6dbdea1e) alone: the implementer half of the clean room. Analyst lane
//! ANALYST-LINKS-6-7-PAST-TOUCH read the native; this lane did not. **Every rule here is UNVERIFIED in the running
//! game.** The native's own checks (the receiver, its class, the arguments, an unloaded receiver) are
//! `ow_inventory::pickup`'s; [`touch_line`] is steps 5 to 9:
//!
//! 5. No collision service: `NotYet { LayerAbsent }`, [`TOUCH_TRACE`], the layer the spawn placement refuses on.
//! 6. A zero-length segment: `NotYet { NotRead }`, [`ZERO_LENGTH`] (the routines' handling was not read).
//! 7. **The level part**, the service's `point_sweep` from `EndLoc` (the pickup's touch point) to `StartLoc` (the
//!    toucher). The note corrects the earlier reading of the direction: the parameter names are the script's, and the
//!    line runs the other way. A hit answers false (blocked; bit 512, so no actor is read). `Unsupported` is
//!    `NotYet { NotRead }` naming the level part. Clear goes on.
//! 8. **The actor walk** over the live actors, with the whole segment's box as the region and zero extent, under
//!    [`Query::PointLine`]: the flags 134226566 (the single-line routine adds 1024), with the pickup as the source
//!    actor ([`crate::trace_filter`], "The touch's line check"). Every unread point keeps its named refusal. The hash
//!    admission comes first; only with the test stand-in [`AdmissionStandIn`] installed is the primitive's line test
//!    named instead.
//! 9. Otherwise true: a clear touch.
//!
//! Nothing is written, loaded or drawn: the walk reads resident objects through lookup-only names.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::placement::{LAYER, Sweep, actor_walk, collision_model, swept};
use crate::trace_filter::{Query, RECORD_TOUCH, Source};

/// The note's names.
pub const TOUCH_TRACE: &str = "the pickup touch trace";
pub const ZERO_LENGTH: &str = "a zero-length touch trace";
pub const LEVEL_PART: &str =
    "the pickup touch trace: the level part (a point line from the touch point to the toucher), which the world's \
     collision service does not support";

/// A test stand-in for the collision hash's admission (NATIVE_TOUCH_PICKUP_TRACE.md section 4, step 8: "a later
/// reading of the hash", UNVERIFIED). Installed as a VM service only by tests; with it, a primitive every read test
/// accepts is refused naming its own line test instead of the hash admission. No harness or host installs it.
#[derive(Debug, Default, Clone, Copy)]
pub struct AdmissionStandIn;

/// Whether `o` is a handle the store has not loaded, or a slotless stand-in whose class has slots.
pub fn is_unloaded(vm: &Vm, o: ObjRef) -> bool {
    let Ok(x) = vm.space.heap.get(o) else {
        return false;
    };
    let stand_in = x.slots.is_empty() && vm.space.classes.get_layout(x.class).is_some_and(|l| l.slot_count() > 0);
    vm.defs.is_unloaded(&vm.space, o) || stand_in
}

/// Steps 5 to 9 of the touch's line check for the pickup `source`, from `start_loc` (the toucher) and `end_loc` (the
/// pickup's touch point), as the script names them (module documentation). True for a clear touch.
pub fn touch_line(vm: &Vm, source: ObjRef, start_loc: [f32; 3], end_loc: [f32; 3]) -> Result<bool, VmError> {
    let Some(model) = collision_model(vm)? else {
        return Err(NotYet::layer_absent(TOUCH_TRACE, LAYER, RECORD_TOUCH).into());
    };
    if start_loc == end_loc {
        return Err(NotYet::not_read(ZERO_LENGTH, RECORD_TOUCH).into());
    }
    // The line runs from the touch point to the toucher.
    match model.point_sweep(end_loc, start_loc)? {
        Sweep::Hit { .. } => return Ok(false),
        Sweep::Unsupported => return Err(NotYet::not_read(LEVEL_PART, RECORD_TOUCH).into()),
        Sweep::Clear => {}
    }
    let unloaded = |o: ObjRef| is_unloaded(vm, o);
    let s = Source { actor: source, unloaded: &unloaded, admission_established: vm.service::<AdmissionStandIn>().is_some() };
    actor_walk(vm, &*model, Query::PointLine, swept([0.0; 3], end_loc, start_loc), Some(&s), TOUCH_TRACE, RECORD_TOUCH)?;
    Ok(true)
}
