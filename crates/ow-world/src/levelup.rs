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
//! In `ParityMode::Reference` steps 1 and 3 are skipped and their conditions are not read (no `CharacterClass`,
//! `Pawn` or `MyWillowPawn`), as the C++ does (`src/mission_script.cpp`: "not modelled"; for Maya's class the data
//! are the constants 0 and 1). A stand-in pinned by `tests/mission_script_test.py` (scenarios S7 and S7b, `--slice-run`)
//! (W6 lanes). Every refusal is decided before the rebase, so a `NotYet` changes nothing.
//!
//! **The `WillowMind` override** (NATIVE_MIND_ROUTE.md section 3, Off only; Reference has no stand-in and stays
//! `NotYet`): its own parts first ([`mind_override`]), then the shared controller part above.
//!
//! **Not read:** the `WillowVehicle` thunk (its own native, not registered
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
    /// Steps 1 and 3 were not run and their conditions not read (Reference mode, as the C++).
    pub class_parts_skipped: bool,
    /// The manager whose pools were rebased (None: no manager, nothing rebased).
    pub manager: Option<ObjRef>,
}

/// `RecalculateAttributeInitializedState()` on `controller` (module documentation).
pub fn recalculate_attribute_initialized_state(
    vm: &mut Vm,
    controller: ObjRef,
) -> Result<Recalculated, VmError> {
    if is_a(&vm.space, controller, "WillowMind")? {
        if vm.parity == ParityMode::Reference {
            return Err(NotYet::not_read(
                "the WillowMind override of RecalculateAttributeInitializedState (no C++ stand-in)",
                RECORD,
            )
            .into());
        }
        mind_override(vm, controller)?;
    }
    let skipped = vm.parity == ParityMode::Reference;
    if !skipped {
        refuse_class_parts(&vm.space, controller)?;
    }
    let manager = object(&vm.space, controller, "ResourcePoolManager")?;
    pools::recalculate_base_values(vm, manager)?;
    Ok(Recalculated {
        class_parts_skipped: skipped,
        manager,
    })
}

/// The record of the mind's override (decision 2026-11-19-32).
pub const RECORD_MIND: &str = "NATIVE_MIND_ROUTE.md section 3";

/// `WillowMind`'s override, the parts before the shared controller part (NATIVE_MIND_ROUTE.md section 3), Off only.
/// Every refusal is decided before anything changes:
/// - **the mind's own part** (with both `AIClass` and `MyWillowPawn`) sets the pawn's `CommerceMarkup` and ten
///   status-effect resistances from evaluations of the class's init data. The note does not name the context those are
///   evaluated with, so it is `NotYet { NotRead }`;
/// - **the skill refresh** (the world's game info a `WillowGameInfo`): its skill manager, through the script native
///   `GetSkillManager` (made lazily, NATIVE_SKILL_MANAGER.md). Unless the manager's `bUpdatingSkills` is set, every
///   `ActiveSkills` entry whose `SkillInstigator` is this mind is flagged and gets one call on the skill that is not
///   read: a matching entry is `NotYet { NotRead }`; with none, the flag set and cleared again leaves nothing changed.
///
/// The shared controller part (steps 1 and 2 of the module documentation) then runs as for any controller. The note
/// counts 13 writes in it where NATIVE_LEVEL_UP_ATTRIBUTES.md lists 12 (not reconciled); its class-level step stays
/// `NotYet` either way.
fn mind_override(vm: &mut Vm, mind: ObjRef) -> Result<(), VmError> {
    let class = object(&vm.space, mind, "AIClass")?;
    let pawn = object(&vm.space, mind, "MyWillowPawn")?;
    if class.is_some() && pawn.is_some() {
        return Err(NotYet::not_read(
            "the WillowMind part of RecalculateAttributeInitializedState (CommerceMarkup and ten status-effect \
             resistances from the AI class's init data: the context they are evaluated with is not named)",
            RECORD_MIND,
        )
        .into());
    }
    let world_info = object(&vm.space, mind, "WorldInfo")?;
    let game = match world_info {
        Some(w) => object(&vm.space, w, "Game")?,
        None => None,
    };
    let Some(game) = game.filter(|g| is_a(&vm.space, *g, "WillowGameInfo").unwrap_or(false)) else {
        return Ok(());
    };
    let f = vm.find_function("WillowGame.WillowGameInfo.GetSkillManager")?;
    let manager = match vm.call(f, Some(game), Vec::new())?.ret {
        Value::Object(o) => o,
        _ => None,
    };
    let Some(manager) = manager else { return Ok(()) };
    if matches!(crate::actors::field(&vm.space, manager, "bUpdatingSkills")?, Value::Bool(true)) {
        return Ok(());
    }
    let skills = crate::actors::objects(&vm.space, manager, "ActiveSkills")?;
    for skill in skills.into_iter().flatten() {
        if object(&vm.space, skill, "SkillInstigator")? == Some(mind) {
            return Err(NotYet::not_read(
                "the skill refresh of the WillowMind RecalculateAttributeInitializedState on a skill of this mind \
                 (the call on the skill is not read)",
                RECORD_MIND,
            )
            .into());
        }
    }
    Ok(())
}

/// Off mode: `NotYet` when step 1 or step 3 applies (module documentation). Reads `CharacterClass`, `Pawn` and, on a
/// `WillowPlayerController`, `MyWillowPawn` and its `StatusEffectComp`.
fn refuse_class_parts(
    space: &ow_object::heap::ObjectSpace,
    controller: ObjRef,
) -> Result<(), VmError> {
    let class_def = object(space, controller, "CharacterClass")?;
    let pawn = object(space, controller, "Pawn")?;
    let class_part = match (pawn, class_def) {
        (Some(p), Some(_)) => !is_a(space, p, "Vehicle")?,
        _ => false,
    };
    if class_part {
        return Err(NotYet::not_read(
            "the class-level pawn attributes of RecalculateAttributeInitializedState (encumbrance resistance \
             and twelve damage modifiers: the pawn properties they land in are not named)",
            RECORD,
        )
        .into());
    }
    if is_a(space, controller, "WillowPlayerController")? {
        let component = match object(space, controller, "MyWillowPawn")? {
            Some(p) => object(space, p, "StatusEffectComp")?,
            None => None,
        };
        let willow_class = match class_def {
            Some(c) => is_a(space, c, "WillowCharacterClassDefinition")?,
            None => false,
        };
        if component.is_some() && willow_class {
            return Err(NotYet::not_read(
                "the ten status-effect resistances RecalculateAttributeInitializedState re-initialises (the \
                 component properties they land in are not named)",
                RECORD,
            )
            .into());
        }
    }
    Ok(())
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
