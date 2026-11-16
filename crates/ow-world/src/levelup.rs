//! Owner: lane W5-POOLS. The level-up rebase: `RecalculateAttributeInitializedState` on a controller
//! (NATIVE_LEVEL_UP_ATTRIBUTES.md sections 1 and 2). AI-assisted (Claude Code).
//!
//! **Confirmed in game 2026-10-07 (lane L1, two local captures, REALGAME_GROUND_TRUTH.md):** the player controller's
//! call rewrites the health pool's base and effective maximum and leaves current health untouched; nothing else of the
//! controller, pawn, replication info or pools changed inside it. The rest is read from the executable and
//! **UNVERIFIED**.
//!
//! **What the call does here**, in the record's order (the controller-class part, then the player-controller part):
//! 1. *Class-level pawn attributes* (only with a pawn that is not a vehicle and a `CharacterClass`): the encumbrance
//!    resistance value and twelve damage-modifier attributes from the class's init data. The record does not name the
//!    pawn properties these land in, so in `ParityMode::Off` this step is `NotYet { NotRead }` whenever it applies.
//! 2. *Pool rebase, always* (even with no pawn): `ResourcePoolManager.RecalculateBaseValues` on the controller's
//!    `ResourcePoolManager`, which runs the installed script `ResourcePool.CalculateBaseValues(true)` on every pool
//!    in slot order (`crate::pools::recalculate_base_values`). The script sets each pool's base maximum from its
//!    definition with the pool's `AssociatedProvider` as context, keeps the modifier stack, and moves current only
//!    when the pool was full at its last `UpdateLastValues` (the follow rule, script).
//! 3. *Status-effect resistances* (a `WillowPlayerController` with a `MyWillowPawn` that has a `StatusEffectComp`,
//!    and a `WillowCharacterClassDefinition`): ten attributes of the status component; `NotYet { NotRead }` in Off mode
//!    when it applies, as step 1.
//!
//! In `ParityMode::Reference` steps 1 and 3 are skipped, as the C++ does (`src/mission_script.cpp`: "not modelled";
//! for Maya's class the data are the constants 0 and 1). A stand-in pinned by no tracked test. Every refusal is
//! decided before the rebase, so a `NotYet` changes nothing.
//!
//! **Not read:** the `WillowMind` override (`NotYet`) and the `WillowVehicle` thunk (its own native, not registered
//! here). No change notification is sent and no script event fires (record section 1).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use crate::actors::object;
use crate::pools::{self, is_a};

/// The record.
pub const RECORD: &str = "NATIVE_LEVEL_UP_ATTRIBUTES.md sections 1-2";

/// The two script-callable keys the installed packages declare (one shared thunk calls the class's virtual).
pub const NATIVE_KEYS: [&str; 2] = [
    "Controller.RecalculateAttributeInitializedState()",
    "WillowPlayerController.RecalculateAttributeInitializedState()",
];

/// What [`recalculate_attribute_initialized_state`] ran.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recalculated {
    /// Step 1 applied and was skipped (Reference mode only).
    pub class_part_skipped: bool,
    /// Step 3 applied and was skipped (Reference mode only).
    pub status_part_skipped: bool,
    /// The manager whose pools were rebased (None: no manager, nothing rebased).
    pub manager: Option<ObjRef>,
}

/// `RecalculateAttributeInitializedState()` on `controller` (module documentation).
pub fn recalculate_attribute_initialized_state(
    vm: &mut Vm,
    controller: ObjRef,
) -> Result<Recalculated, VmError> {
    let space = &vm.space;
    if is_a(space, controller, "WillowMind")? {
        return Err(NotYet::not_read(
            "the WillowMind override of RecalculateAttributeInitializedState",
            RECORD,
        )
        .into());
    }
    let class_def = object(space, controller, "CharacterClass")?;
    let pawn = object(space, controller, "Pawn")?;
    let class_part = match (pawn, class_def) {
        (Some(p), Some(_)) => !is_a(space, p, "Vehicle")?,
        _ => false,
    };
    let status_part = if is_a(space, controller, "WillowPlayerController")? {
        let my_pawn = object(space, controller, "MyWillowPawn")?;
        let component = match my_pawn {
            Some(p) => object(space, p, "StatusEffectComp")?,
            None => None,
        };
        let willow_class = match class_def {
            Some(c) => is_a(space, c, "WillowCharacterClassDefinition")?,
            None => false,
        };
        component.is_some() && willow_class
    } else {
        false
    };
    if vm.parity == ParityMode::Off {
        if class_part {
            return Err(NotYet::not_read(
                "the class-level pawn attributes of RecalculateAttributeInitializedState (encumbrance resistance \
                 and twelve damage modifiers: the pawn properties they land in are not named)",
                RECORD,
            )
            .into());
        }
        if status_part {
            return Err(NotYet::not_read(
                "the ten status-effect resistances RecalculateAttributeInitializedState re-initialises (the \
                 component properties they land in are not named)",
                RECORD,
            )
            .into());
        }
    }
    let manager = object(&vm.space, controller, "ResourcePoolManager")?;
    pools::recalculate_base_values(vm, manager)?;
    Ok(Recalculated {
        class_part_skipped: class_part,
        status_part_skipped: status_part,
        manager,
    })
}

/// The two `RecalculateAttributeInitializedState` keys as a [`NativePack`]. `World::new` registers it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        for key in NATIVE_KEYS {
            vm.register_native(
                key,
                Rc::new(move |c: &mut NativeCall<'_>| {
                    let controller = c.this.ok_or_else(|| VmError::Script {
                        function: key.to_string(),
                        message: "called without a controller".to_string(),
                    })?;
                    recalculate_attribute_initialized_state(c.vm, controller)?;
                    Ok(Value::None)
                }),
            )?;
        }
        Ok(())
    }
}
