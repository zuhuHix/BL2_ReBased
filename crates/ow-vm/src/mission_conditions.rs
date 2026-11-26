//! Owner: lane IMPL-BEHAVIOR-KERNEL-PASS2 (decision 2026-11-26-33). The mission-condition service the behavior
//! kernel's pass 2 reads (docs/verification/NATIVE_BEHAVIOR_KERNEL_PASS2.md sections 3 and 4, the hub's ruling 1 on
//! review of #771). AI-assisted (Claude Code).
//!
//! **Why here.** `ow-mission` depends on `ow-behavior`, never the reverse, so the kernel cannot call the mission
//! bridge. Both depend on this crate. The bridge implements [`MissionConditionAnswers`]; the harness installs it as
//! the VM service [`MissionConditions`]; the kernel reads it. A world without the service has no answers: the
//! kernel's pass 2 refuses with `NotYet { LayerAbsent }` where it needs one.
//!
//! **Two duties** (the note's preflight step 8 and commit step B):
//! - the answers, read-only: a `BehaviorSequenceEnableByMission` condition's verdict now, and whether a mission is
//!   Complete;
//! - the list of kernel-registered conditions by mission, **separate from any script observers** (the hub's ruling 2):
//!   the bridge's script notification never walks it. A store cannot fail (it is not a heap write).
//!
//! **Every rule behind it is UNVERIFIED in the running game.**

use std::rc::Rc;

use ow_object::ids::ObjRef;

use crate::api::Vm;
use crate::error::VmError;

/// The answers the kernel's pass 2 asks for (the note's section 2.4), read-only.
pub trait MissionConditionAnswers {
    /// `condition`'s verdict from the tracker's current state (NATIVE_BEHAVIOR_KERNEL_PASS2.md section 2.4). Reads
    /// only.
    fn verdict(&self, vm: &Vm, condition: ObjRef) -> Result<bool, VmError>;
    /// Whether `mission`'s status is Complete (no record: NotStarted, so false). Reads only.
    fn mission_complete(&self, vm: &Vm, mission: ObjRef) -> Result<bool, VmError>;
}

/// The VM service: the answers and the kernel-registered conditions by mission, in registration order.
pub struct MissionConditions {
    answers: Rc<dyn MissionConditionAnswers>,
    /// (the mission's stock path, the condition), the one identity the verdict uses too (review of #775, item 4).
    stored: Vec<(String, ObjRef)>,
}

impl std::fmt::Debug for MissionConditions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MissionConditions")
            .field("stored", &self.stored)
            .finish()
    }
}

impl MissionConditions {
    /// The service over `answers`, with nothing stored.
    pub fn new(answers: Rc<dyn MissionConditionAnswers>) -> Self {
        MissionConditions {
            answers,
            stored: Vec::new(),
        }
    }

    /// The answers.
    pub fn answers(&self) -> Rc<dyn MissionConditionAnswers> {
        self.answers.clone()
    }

    /// Stores `condition` under the mission whose stock path is `mission` (commit step B). A pair stored already is
    /// kept once (a port choice: the tracker's own duplicate handling was not read; a second copy would only re-apply
    /// the same verdict).
    pub fn store(&mut self, mission: &str, condition: ObjRef) {
        if !self.stored.iter().any(|(m, c)| m == mission && *c == condition) {
            self.stored.push((mission.to_string(), condition));
        }
    }

    /// The conditions stored under the mission whose stock path is `mission`, in registration order.
    pub fn stored_under(&self, mission: &str) -> Vec<ObjRef> {
        self.stored.iter().filter(|(m, _)| m == mission).map(|(_, c)| *c).collect()
    }
}

/// The installed service's answers, if any (the cell is not held after the call).
pub fn answers(vm: &Vm) -> Result<Option<Rc<dyn MissionConditionAnswers>>, VmError> {
    match vm.service::<MissionConditions>() {
        None => Ok(None),
        Some(cell) => {
            let s = cell
                .try_borrow()
                .map_err(|_| VmError::ServiceBorrowed("MissionConditions"))?;
            Ok(Some(s.answers()))
        }
    }
}

/// Installs `answers` as the service, or replaces the answers of an installed one (its stored list kept).
pub fn install(vm: &mut Vm, answers: Rc<dyn MissionConditionAnswers>) -> Result<(), VmError> {
    match vm.service::<MissionConditions>() {
        Some(cell) => {
            cell.try_borrow_mut()
                .map_err(|_| VmError::ServiceBorrowed("MissionConditions"))?
                .answers = answers;
            Ok(())
        }
        None => vm.add_service(MissionConditions::new(answers)).map(|_| ()),
    }
}
