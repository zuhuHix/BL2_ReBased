//! Owner: lane W7-AI (part 1). One AI tick of a component, in the recorded order (NATIVE_AI_MELEE.md, "AIComponent:
//! cadence"; live, L8 section 3.1). AI-assisted (Claude Code).
//!
//! **Where the tick sits.** The mind ticks its component once per mind tick, when the mind may tick at all
//! (`WillowMind.CanTickAI`, not read here: the caller decides). The mind is a controller, so it ticks in the tick
//! group its class gives it (`Actor.TickGroup`, read once by `ow_world::world::World::tick`); the call from the
//! world's actor tick into [`tick`] is a seam of `ow-world` that part 1 does not add (decision 2026-11-16-59).
//!
//! **Inside one tick** (G25; L8: `LastTargetUpdateTime` equalled `LastStateUpdateTime` in 2,433 of 2,433 frames,
//! spaced at least 0.1999 s with `UpdateRate` 0.2):
//! 1. the first tick initialises the component (the action tree's reset: part 2);
//! 2. on hold, nothing more, and the running action is stopped (part 2);
//! 3. the target update, when `now >= LastStateUpdateTime + UpdateRate` (the target update tests the **state**
//!    update's time): the fall after `ForgetTime`, the removal pass, `FindTargets`, `CalcTargetInfo` on every record
//!    that was already in the list, `ChooseBestTarget`; then `LastTargetUpdateTime = now`;
//! 4. the running action's update and the tree's timed failures (part 2);
//! 5. the state update under the same test (the tree's selection: part 2); then `LastStateUpdateTime = now`.
//!
//! [`TickReport::pending`] names the tree's steps; [`super::actions::tick_brain`] runs them around this tick (part 2,
//! decision 2026-11-16-66). `tick` alone, without a tree, leaves them pending: not run and not an error.

use ow_object::ids::ObjRef;
use ow_vm::error::VmError;

use super::Senses;
use super::iterators::calc_target_info;
use super::record::Component;
use super::targets::{Effect, choose_best, find_targets, remove_stale};
use super::threat::{forget, no_parent};

/// What one tick did.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TickReport {
    /// Whether the target update ran.
    pub target_update: bool,
    /// Whether the state update's time was due (its selection is part 2).
    pub state_update: bool,
    /// Targets added to and removed from the list.
    pub added: Vec<ObjRef>,
    pub removed: Vec<ObjRef>,
    pub effects: Vec<Effect>,
    /// The steps of later AI lanes this tick reached.
    pub pending: Vec<&'static str>,
}

/// The target update (step 3 of the module documentation).
pub fn target_update(c: &mut Component, senses: &mut dyn Senses, now: f32, report: &mut TickReport) -> Result<(), VmError> {
    no_parent(c)?;
    forget(c, now);
    report.removed = remove_stale(c, senses)?;
    let before = c.records.len();
    let added = find_targets(c, senses, now)?;
    report.added = added.iter().map(|&i| c.records[i].target).collect();
    for i in 0..before {
        calc_target_info(c, senses, now, i)?;
    }
    choose_best(c, senses, &mut report.effects)
}

/// One AI tick of the component at world time `now` (module documentation).
pub fn tick(c: &mut Component, senses: &mut dyn Senses, now: f32) -> Result<TickReport, VmError> {
    let mut report = TickReport::default();
    if !c.initialised {
        c.initialised = true;
        report.pending.push("the action tree's first-run reset (AI part 2)");
    }
    if c.on_hold {
        report.pending.push("stopping the running action while on hold (AI part 2)");
        return Ok(report);
    }
    let due = now >= c.last_state_update + c.settings.update_rate;
    if due {
        target_update(c, senses, now, &mut report)?;
        c.last_target_update = now;
        report.target_update = true;
    }
    report.pending.push("the running action's update and the tree's timed failures (AI part 2)");
    if due {
        c.last_state_update = now;
        report.state_update = true;
        report.pending.push("the action tree's state update (AI part 2)");
    }
    Ok(report)
}
