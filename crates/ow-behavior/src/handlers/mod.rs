//! Owner: lane W5-BEHAVIOR. The behavior handlers that need only the world, and [`Pack`], which registers them into
//! the VM's handler registry (`crate::registry`). AI-assisted (Claude Code).
//!
//! | Class | Rule | Status |
//! |---|---|---|
//! | `WillowGame.Behavior_CompareObject` | [`compare_object`] | read from the installed script; UNVERIFIED in the game |
//!
//! Every other class is registered by the crate that owns it (`ow-combat`, `ow-skills`, `ow-mission`) or answers
//! `NotYet`.

use ow_vm::api::{NativePack, Vm};
use ow_vm::error::VmError;

use crate::kernel::{BehaviorId, Provider};
use crate::provider::BehaviorError;
use crate::registry::{self, Handler};

/// `WillowGame.Behavior_CompareObject`.
pub const COMPARE_OBJECT: &str = "WillowGame.Behavior_CompareObject";

/// `ECompareObjectOutputLinkIds`: `OUTPUT_Same`, `OUTPUT_Different`.
pub const OUTPUT_SAME: i32 = 0;
pub const OUTPUT_DIFFERENT: i32 = 1;

/// Registers the world-only handlers into the VM's registry.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        registry::register(vm, COMPARE_OBJECT, Handler::run(compare_object))
    }
}

/// `Behavior_CompareObject.ApplyBehaviorToContext`, read from its installed script (BEHAVIOR_DATA_DECODE.md,
/// "What CompareObject does with them"): `ObjectA == ObjectB` (`Object.EqualEqual_ObjectObject`, Core native 114)
/// selects output 0 (`OUTPUT_Same`), otherwise output 1 (`OUTPUT_Different`). The objects compare by stock path
/// (`""` is None), as the C++ does. An input whose value is unknown selects nothing (the error is in the provider's
/// `errors`). Status: UNVERIFIED in the running game.
pub fn compare_object(
    vm: &mut Vm,
    p: &mut Provider,
    id: BehaviorId,
    _event: &str,
) -> Result<Vec<i32>, BehaviorError> {
    let a = p.object_input(vm, id, "ObjectA")?;
    let b = p.object_input(vm, id, "ObjectB")?;
    match (a, b) {
        (Some(a), Some(b)) => Ok(vec![if a == b {
            OUTPUT_SAME
        } else {
            OUTPUT_DIFFERENT
        }]),
        _ => Ok(Vec::new()),
    }
}
