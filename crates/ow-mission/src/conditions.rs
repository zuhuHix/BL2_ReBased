//! Owner: lane IMPL-BEHAVIOR-KERNEL-PASS2 (decision 2026-11-26-33). The one `BehaviorSequenceEnableByMission` verdict
//! (docs/verification/NATIVE_BEHAVIOR_KERNEL_PASS2.md section 2.4, the hub's rulings 1 and 3 on review of #771), and
//! the bridge's answers to the behavior kernel's pass 2 (`ow_vm::mission_conditions`). AI-assisted (Claude Code).
//!
//! **One verdict** ([`verdict`]), from a [`MissionSystem`]'s current state. **Every rule is UNVERIFIED in the running
//! game.**
//! - not objective-specific: the linked mission's status against `MissionStatesToLinkTo`. The system's own mission has
//!   its status; **every other mission is NotStarted** (no record), as the bridge's `GetMissionStatus` answers;
//! - objective-specific: `LinkedObjective` None is false. Otherwise the objective's state (the system's objective
//!   rules for its own mission, NotStarted for every other) against `ObjectiveStatesToLinkTo`. An objective outside its
//!   mission is false, reported to the caller ([`Verdict::outside`]);
//! - a true verdict with a non-empty `ObjectiveSetRestrictions` stays true only while one listed set of the system's
//!   own mission is active.
//!
//! **The Fire slice's difference** (the hub's ruling 3): its old verdict counted a mission in its completed set as
//! Complete; this one, like `GetMissionStatus`, has only the system's own mission. The slice now asks this verdict.
//!
//! **The answers** ([`BridgeAnswers`]): the verdict above for the bridge's mission, and "is this mission Complete"
//! (only the bridge's own can be). The bridge installs them in Off ([`install`]); the kernel reads them.

use std::rc::Rc;

use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_vm::mission_conditions::MissionConditionAnswers;

use crate::tracker::{MissionError, MissionSystem, Status, field, prop, ref_path};

/// The status field names of `MissionStatesToLinkTo`, by status index.
const STATUS_FIELDS: [&str; 4] = ["bNotStarted", "bActive", "bReadyToTurnIn", "bComplete"];

/// A verdict, with the objective path when the condition names an objective outside its mission (then false).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub value: bool,
    pub outside: Option<String>,
}

/// `condition`'s verdict from `m`'s current state (module documentation). Reads only.
pub fn verdict(vm: &Vm, m: &MissionSystem, condition: ObjRef) -> Result<Verdict, MissionError> {
    let linked = ref_path(vm, prop(&vm.space, condition, "LinkedMission")?.as_ref())?;
    let own = linked == m.path();
    let truth = |vm: &Vm, v: Option<&Value>| v.is_some_and(|v| v.truth(&vm.space.names));
    let specific = truth(
        vm,
        prop(&vm.space, condition, "bIsObjectiveSpecific")?.as_ref(),
    );
    let mut value = if !specific {
        let state = if own {
            STATUS_FIELDS[usize::from(m.status().index())]
        } else {
            "bNotStarted"
        };
        let states = prop(&vm.space, condition, "MissionStatesToLinkTo")?;
        truth(vm, states.as_ref().and_then(|v| field(&vm.space, v, state)))
    } else {
        let objective = ref_path(vm, prop(&vm.space, condition, "LinkedObjective")?.as_ref())?;
        if objective.is_empty() {
            return Ok(Verdict {
                value: false,
                outside: None,
            });
        }
        let state = if own {
            m.objective_state(&objective)
        } else {
            "NotStarted"
        };
        if state.is_empty() {
            return Ok(Verdict {
                value: false,
                outside: Some(objective),
            });
        }
        let states = prop(&vm.space, condition, "ObjectiveStatesToLinkTo")?;
        let bit = format!("b{state}");
        truth(vm, states.as_ref().and_then(|v| field(&vm.space, v, &bit)))
    };
    if value
        && let Some(Value::Array(list)) = prop(&vm.space, condition, "ObjectiveSetRestrictions")?
        && !list.is_empty()
    {
        value = false;
        if own {
            for set in list.iter() {
                if m.set_active(&ref_path(vm, Some(set))?) {
                    value = true;
                    break;
                }
            }
        }
    }
    Ok(Verdict {
        value,
        outside: None,
    })
}

/// A mission error as the VM's error: a VM error as it is, any other as a script error naming the call.
fn to_vm(e: MissionError) -> VmError {
    match e {
        MissionError::Vm(v) => v,
        other => VmError::Script {
            function: "the mission condition's verdict".to_string(),
            message: other.to_string(),
        },
    }
}

/// The bridge's answers (module documentation).
pub struct BridgeAnswers {
    mission: MissionSystem,
}

impl MissionConditionAnswers for BridgeAnswers {
    fn verdict(&self, vm: &Vm, condition: ObjRef) -> Result<bool, VmError> {
        verdict(vm, &self.mission, condition)
            .map(|v| v.value)
            .map_err(to_vm)
    }

    fn mission_complete(&self, vm: &Vm, mission: ObjRef) -> Result<bool, VmError> {
        let own = mission == self.mission.definition()
            || ow_behavior::provider::stock_path(vm, mission).map_err(|e| to_vm(e.into()))?
                == self.mission.path();
        Ok(own && self.mission.status() == Status::Complete)
    }
}

/// Installs the bridge's answers for `mission` as the VM's mission-condition service (Off; the bridge calls it).
pub fn install(vm: &mut Vm, mission: &MissionSystem) -> Result<(), VmError> {
    ow_vm::mission_conditions::install(
        vm,
        Rc::new(BridgeAnswers {
            mission: mission.clone(),
        }),
    )
}
