//! Owner: lane IMPL-BEHAVIOR-KERNEL-PASS2 (decision 2026-11-26-33). Pass 2 of `IntializeBehaviorProviderForConsumer`
//! in `ParityMode::Off`: a sequence with a `CustomEnableCondition` is linked, registered and reacted to at once.
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_BEHAVIOR_KERNEL_PASS2.md section 4 alone (#771 with its
//! review r1, lane ANALYST-BEHAVIOR-KERNEL-PASS2) and the hub's rulings; this lane did not read the executable. **Every
//! rule is UNVERIFIED in the running game.**
//!
//! **The plan** ([`plan`]) is the note's preflight, write-free and load-free, nothing interned. For each sequence of
//! the definition's `BehaviorSequences` with a `CustomEnableCondition`, in order, the first refusal met is the answer:
//! 1. the condition resident, else [`CONDITION_NOT_LOADED`] (`NotRead`);
//! 2. its class a `WillowGame.BehaviorSequenceEnableByMission` (up the loaded class chain), else `NotRead` naming the
//!    class ([`other_condition`]);
//! 3. its fields declared with their kinds, else `VmError::Script` naming the field;
//! 4. the consumer's world (its `WorldInfo`) a client (`NetMode` `NM_Client`, the kernel's `is_client`): that sequence
//!    has no pass 2;
//! 5. `bInstanced` set: no pass 2;
//! 6. the tracker walk, `WorldInfo.GRI` as a `WillowGame.WillowGameReplicationInfo`, then its `MissionTracker`. Any
//!    link missing: a link and nothing more;
//! 7. with a tracker: its `Role` not `ROLE_Authority` (by its enum), or `LinkedMission` None, a link and nothing more.
//!    A tracker the store has not loaded is [`TRACKER_NOT_LOADED`]. `LinkedObjective` set and not resident is
//!    [`OBJECTIVE_NOT_LOADED`]; its `bRememberItemsWithinObjective` set is [`PER_INSTANCE`];
//! 8. the verdict and the store decision, from the [`MissionConditions`] service: none installed is
//!    `NotYet { LayerAbsent }` ([`NO_ANSWERS`]). A Complete mission must be resident ([`MISSION_NOT_LOADED`]) and
//!    declare `bRepeatable` as a bool (`VmError::Script`); stored unless Complete and not repeatable;
//! 9. `bCreateWaypoint` set and the verdict would change any record in the condition's list once this one is linked
//!    (this consumer's new record, whose state is its `bEnabledOnSpawn`, and every record linked before, by any
//!    consumer, the list being shared): [`WAYPOINT`] (`NotRead`).
//!
//! **The commit** ([`commit`]), after pass 1, in sequence order: A, the link (the kernel's own list keyed by the
//! condition, native state); C, the verdict applied to every record of that list (the
//! `ChangeBehaviorSequenceActivationStatus` rule, events only on a change); B, the store in the service (cannot fail).
//! A refusal from an event's behaviors comes after writes; the callers mark it.
//!
//! **Later notifications** ([`reevaluate_conditions`], the hub's ruling 2): the bridge, after its own script observers,
//! asks for every kernel-registered condition stored under a mission to be re-evaluated through the same verdict and
//! applied the same way. What cannot be answered is a typed refusal.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_vm::mission_conditions::{self, MissionConditions};

use super::{borrow, kernel, lend, to_vm};
use crate::provider;

/// The record of the rule.
pub const RECORD_PASS2: &str = "docs/verification/NATIVE_BEHAVIOR_KERNEL_PASS2.md (section 4)";
/// The condition class read.
pub const CONDITION_CLASS: &str = "WillowGame.BehaviorSequenceEnableByMission";
/// The tracker's replication-info class.
const GRI_CLASS: &str = "WillowGame.WillowGameReplicationInfo";

pub const CONDITION_NOT_LOADED: &str = "pass 2 of IntializeBehaviorProviderForConsumer: \
    a CustomEnableCondition the store has not loaded (a load is a write)";
pub const TRACKER_NOT_LOADED: &str = "pass 2 of IntializeBehaviorProviderForConsumer: \
    a mission tracker the store has not loaded (a load is a write)";
pub const OBJECTIVE_NOT_LOADED: &str = "pass 2 of IntializeBehaviorProviderForConsumer: \
    a LinkedObjective the store has not loaded (a load is a write)";
pub const PER_INSTANCE: &str =
    "pass 2 of IntializeBehaviorProviderForConsumer: per-instance objectives (the key is not read)";
pub const MISSION_NOT_LOADED: &str = "pass 2 of IntializeBehaviorProviderForConsumer: \
    a Complete LinkedMission the store has not loaded (a load is a write)";
pub const NO_ANSWERS: &str =
    "pass 2 of IntializeBehaviorProviderForConsumer: a mission condition's verdict";
pub const NO_ANSWERS_LAYER: &str = "the mission tracker's answers";
pub const WAYPOINT: &str = "pass 2 of IntializeBehaviorProviderForConsumer: the waypoint hooks";

/// A condition class other than the one read.
pub fn other_condition(class: &str) -> String {
    format!(
        "pass 2 of IntializeBehaviorProviderForConsumer: a CustomEnableCondition of class {class}"
    )
}

/// One state record: sequence `seq` of provider `path` on kernel process `pid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Record {
    pub pid: i32,
    pub path: String,
    pub seq: usize,
}

/// What pass 2 does for one conditioned sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Step {
    /// A client world or `bInstanced`: nothing.
    Nothing,
    /// Linked, no register (no tracker, a tracker without authority, or no `LinkedMission`).
    Link,
    /// Linked, registered under `mission` (stored when `store`), the verdict applied.
    Register {
        /// The mission's stock path (one identity with the verdict's, review of #775, item 4).
        mission: String,
        verdict: bool,
        store: bool,
    },
}

/// One conditioned sequence's plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Planned {
    pub seq: usize,
    pub condition: ObjRef,
    pub step: Step,
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD_PASS2).into()
}

fn script(message: String) -> VmError {
    VmError::Script {
        function: "IntializeBehaviorProviderForConsumer".to_string(),
        message,
    }
}

fn is_named(vm: &Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(layout) = vm.space.classes.get_layout(c) else {
            return Ok(false);
        };
        if layout.path().as_str().eq_ignore_ascii_case(path) {
            return Ok(true);
        }
        cursor = layout.super_class();
    }
    Ok(false)
}

fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm
        .space
        .classes
        .get_layout(x.class)
        .is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short))
}

/// `o`'s `name`, declared with a kind `ok` accepts, else `VmError::Script` naming it.
fn declared(
    vm: &Vm,
    o: ObjRef,
    name: &str,
    want: &str,
    ok: impl Fn(&PropKind) -> bool,
) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| {
        script(format!(
            "{} declares no {name}",
            provider::class_path_of(vm, o).unwrap_or_default()
        ))
    })?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone());
    if !kind.as_ref().is_some_and(&ok) {
        return Err(script(format!("{name} is not declared as {want}")));
    }
    Ok(vm
        .space
        .read(o, ow_object::ids::PropAddr::scalar(p))?
        .clone())
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match declared(vm, o, name, "an object", |k| {
        matches!(k, PropKind::Object { .. })
    })? {
        Value::Object(x) => Ok(x),
        other => Err(script(format!(
            "{name} holds {}, not an object",
            other.kind_name()
        ))),
    }
}

fn boolean(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match declared(vm, o, name, "a bool", |k| matches!(k, PropKind::Bool))? {
        Value::Bool(b) => Ok(b),
        other => Err(script(format!(
            "{name} holds {}, not a bool",
            other.kind_name()
        ))),
    }
}

/// The condition's fields, declared with their kinds (preflight step 3).
fn declarations(vm: &Vm, condition: ObjRef) -> Result<(), VmError> {
    object(vm, condition, "LinkedMission")?;
    object(vm, condition, "LinkedObjective")?;
    for name in ["bInstanced", "bIsObjectiveSpecific", "bCreateWaypoint"] {
        boolean(vm, condition, name)?;
    }
    for name in ["MissionStatesToLinkTo", "ObjectiveStatesToLinkTo"] {
        declared(vm, condition, name, "a struct", |k| {
            matches!(k, PropKind::Struct(_))
        })?;
    }
    declared(
        vm,
        condition,
        "ObjectiveSetRestrictions",
        "an array of objects",
        |k| matches!(k, PropKind::Array(inner) if matches!(**inner, PropKind::Object { .. })),
    )?;
    Ok(())
}

/// Whether `actor`'s `Role` is `ROLE_Authority`, by that value's place in the enum `Role` is declared with.
fn is_authority(vm: &Vm, actor: ObjRef) -> Result<bool, VmError> {
    let class = vm.space.heap.get(actor)?.class;
    let p = vm
        .space
        .prop(class, "Role")
        .ok_or_else(|| script("the mission tracker declares no Role".to_string()))?;
    let Some(PropKind::Byte { enum_: Some(e) }) = vm.space.classes.decl(p).map(|d| d.kind.clone())
    else {
        return Err(not_read(
            "pass 2 of IntializeBehaviorProviderForConsumer: a Role not declared with its enum",
        ));
    };
    let authority = vm
        .space
        .classes
        .enum_layout(e)
        .and_then(|l| {
            l.values().iter().position(|v| {
                vm.space
                    .names
                    .text(*v)
                    .eq_ignore_ascii_case("ROLE_Authority")
            })
        })
        .and_then(|i| u8::try_from(i).ok());
    let Some(authority) = authority else {
        return Err(not_read(
            "pass 2 of IntializeBehaviorProviderForConsumer: a Role enum with no ROLE_Authority",
        ));
    };
    match vm.space.read(actor, ow_object::ids::PropAddr::scalar(p))? {
        Value::Byte(r) => Ok(*r == authority),
        other => Err(script(format!(
            "Role holds {}, not a byte",
            other.kind_name()
        ))),
    }
}

/// `(client, tracker)` of `consumer`'s world (preflight steps 4 and 6): `client` for a client world (then no tracker
/// is read); the tracker `None` for a missing link.
fn tracker(vm: &Vm, consumer: ObjRef) -> Result<(bool, Option<ObjRef>), VmError> {
    let Some(world) = object(vm, consumer, "WorldInfo")? else {
        return Err(not_read(
            "pass 2 of IntializeBehaviorProviderForConsumer: a consumer with no WorldInfo",
        ));
    };
    if super::super::run::is_client(vm, world, "IntializeBehaviorProviderForConsumer")? {
        return Ok((true, None));
    }
    let Some(gri) = object(vm, world, "GRI")? else {
        return Ok((false, None));
    };
    if !is_named(vm, gri, GRI_CLASS)? {
        return Ok((false, None));
    }
    Ok((false, object(vm, gri, "MissionTracker")?))
}

/// The verdict of `condition` and the objective checks before it (preflight steps 7 and 8, and a re-evaluation).
fn verdict(vm: &Vm, condition: ObjRef) -> Result<bool, VmError> {
    if let Some(objective) = object(vm, condition, "LinkedObjective")? {
        if !resident(vm, objective)? {
            return Err(not_read(OBJECTIVE_NOT_LOADED));
        }
        if boolean(vm, objective, "bRememberItemsWithinObjective")? {
            return Err(not_read(PER_INSTANCE));
        }
    }
    let Some(answers) = mission_conditions::answers(vm)? else {
        return Err(NotYet::layer_absent(NO_ANSWERS, NO_ANSWERS_LAYER, RECORD_PASS2).into());
    };
    answers.verdict(vm, condition)
}

/// The store decision for `mission` (preflight step 8).
fn store_decision(vm: &Vm, mission: ObjRef) -> Result<bool, VmError> {
    let Some(answers) = mission_conditions::answers(vm)? else {
        return Err(NotYet::layer_absent(NO_ANSWERS, NO_ANSWERS_LAYER, RECORD_PASS2).into());
    };
    if !answers.mission_complete(vm, mission)? {
        return Ok(true);
    }
    if !resident(vm, mission)? {
        return Err(not_read(MISSION_NOT_LOADED));
    }
    Ok(boolean(vm, mission, "bRepeatable")?)
}

/// The current state of `record`, read from its provider (None: lent out or gone).
fn record_enabled(vm: &Vm, record: &Record) -> Result<Option<bool>, VmError> {
    let k = kernel(vm)?;
    let k = borrow(&k)?;
    Ok(k.processes
        .get(&record.pid)
        .and_then(|p| {
            p.providers
                .iter()
                .find(|(q, _)| *q == record.path)
                .and_then(|(_, held)| held.as_ref())
        })
        .map(|p| p.index_enabled(record.seq)))
}

/// The waypoint test (preflight step 9, and a re-evaluation): `known` the states of the condition's records.
fn waypoint(vm: &Vm, condition: ObjRef, verdict: bool, known: &[bool]) -> Result<(), VmError> {
    if boolean(vm, condition, "bCreateWaypoint")? && known.iter().any(|&state| state != verdict) {
        return Err(not_read(WAYPOINT));
    }
    Ok(())
}

/// The states of the records already in `condition`'s list; a record whose provider is lent out is refused.
fn linked_states(vm: &Vm, condition: ObjRef) -> Result<Vec<bool>, VmError> {
    let records = {
        let k = kernel(vm)?;
        let k = borrow(&k)?;
        k.condition_links
            .get(&condition)
            .cloned()
            .unwrap_or_default()
    };
    let mut out = Vec::with_capacity(records.len());
    for r in &records {
        match record_enabled(vm, r)? {
            Some(state) => out.push(state),
            None => {
                return Err(not_read(
                    "pass 2 of IntializeBehaviorProviderForConsumer: a linked record whose provider is running",
                ));
            }
        }
    }
    Ok(out)
}

/// The plan of `definition`'s pass 2 on `consumer` (module documentation), read only. `definition` must be resident
/// (its raw `BehaviorSequences` are read).
pub(super) fn plan(vm: &Vm, consumer: ObjRef, definition: ObjRef) -> Result<Vec<Planned>, VmError> {
    let class = vm.space.heap.get(definition)?.class;
    let sequences = match vm.space.prop(class, "BehaviorSequences") {
        Some(p) => match vm
            .space
            .read(definition, ow_object::ids::PropAddr::scalar(p))?
        {
            Value::Array(items) => items.as_ref().clone(),
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    let mut out = Vec::new();
    // Records planned earlier in this pass, per condition, with their states after pass 1.
    let mut planned: Vec<(ObjRef, bool)> = Vec::new();
    let mut world: Option<(bool, Option<ObjRef>)> = None;
    for (seq, item) in sequences.iter().enumerate() {
        let Some(Value::Object(Some(condition))) =
            provider::field(&vm.space, item, "CustomEnableCondition")
        else {
            continue;
        };
        let condition = *condition;
        // 1, 2, 3.
        if !resident(vm, condition)? {
            return Err(not_read(CONDITION_NOT_LOADED));
        }
        if !is_named(vm, condition, CONDITION_CLASS)? {
            return Err(not_read(other_condition(
                &provider::class_path_of(vm, condition).unwrap_or_default(),
            )));
        }
        declarations(vm, condition)?;
        // 4.
        let (client, tracker_ref) = match world {
            Some(w) => w,
            None => {
                let w = tracker(vm, consumer)?;
                world = Some(w);
                w
            }
        };
        if client || boolean(vm, condition, "bInstanced")? {
            out.push(Planned {
                seq,
                condition,
                step: Step::Nothing,
            });
            continue;
        }
        let on_spawn = provider::field(&vm.space, item, "bEnabledOnSpawn")
            .is_none_or(|v| v.truth(&vm.space.names));
        // 6, 7.
        let link = |planned: &mut Vec<(ObjRef, bool)>, out: &mut Vec<Planned>| {
            planned.push((condition, on_spawn));
            out.push(Planned {
                seq,
                condition,
                step: Step::Link,
            });
        };
        let Some(tracker) = tracker_ref else {
            link(&mut planned, &mut out);
            continue;
        };
        if !resident(vm, tracker)? {
            return Err(not_read(TRACKER_NOT_LOADED));
        }
        let mission = object(vm, condition, "LinkedMission")?;
        let Some(mission) = mission.filter(|_| is_authority(vm, tracker).unwrap_or(false)) else {
            // Role's own refusals stand when there is a mission to register.
            if mission.is_some() {
                is_authority(vm, tracker)?;
            }
            link(&mut planned, &mut out);
            continue;
        };
        // 7 (the objective), 8.
        let verdict = verdict(vm, condition)?;
        let store = store_decision(vm, mission)?;
        // Review of #775, item 5: commit step B cannot fail, so the service's borrow is checked here.
        if store
            && let Some(cell) = vm.service::<MissionConditions>()
            && cell.try_borrow_mut().is_err()
        {
            return Err(VmError::ServiceBorrowed("MissionConditions"));
        }
        let mission_path = mission_key(vm, mission)?;
        // 9: the records linked before (any consumer), the ones planned earlier in this pass, and this one.
        let mut known = linked_states(vm, condition)?;
        known.extend(
            planned
                .iter()
                .filter(|(c, _)| *c == condition)
                .map(|(_, s)| *s),
        );
        known.push(on_spawn);
        waypoint(vm, condition, verdict, &known)?;
        planned.push((condition, verdict));
        out.push(Planned {
            seq,
            condition,
            step: Step::Register {
                mission: mission_path,
                verdict,
                store,
            },
        });
    }
    Ok(out)
}

/// The service's identity of `mission` (review of #775, item 4): its stock path, the identity the verdict compares. A
/// run-time object, which has none (no mission is one in the game), is keyed by its own name, marked so it meets no
/// package path.
pub fn mission_key(vm: &Vm, mission: ObjRef) -> Result<String, VmError> {
    let path = provider::stock_path(vm, mission).map_err(to_vm)?;
    if !path.is_empty() {
        return Ok(path);
    }
    let name = vm.space.heap.get(mission)?.name;
    Ok(format!("(run time) {}", vm.space.names.text(name)))
}

/// Applies `verdict` to every record in `condition`'s list (commit step C), in link order.
fn react(vm: &mut Vm, condition: ObjRef, verdict: bool) -> Result<(), VmError> {
    let records = {
        let k = kernel(vm)?;
        let k = borrow(&k)?;
        k.condition_links
            .get(&condition)
            .cloned()
            .unwrap_or_default()
    };
    for r in records {
        let slot = {
            let k = kernel(vm)?;
            let k = borrow(&k)?;
            k.processes
                .get(&r.pid)
                .and_then(|p| p.providers.iter().position(|(q, _)| *q == r.path))
        };
        let Some(slot) = slot else { continue };
        lend(vm, r.pid, slot, |vm, p| {
            p.set_index_enabled(vm, r.seq, verdict).map_err(to_vm)
        })?;
    }
    Ok(())
}

/// The commit of `planned` for provider `path` on process `pid` (module documentation), after pass 1.
pub(super) fn commit(
    vm: &mut Vm,
    pid: i32,
    path: &str,
    planned: &[Planned],
) -> Result<(), VmError> {
    for step in planned {
        if step.step == Step::Nothing {
            continue;
        }
        // A.
        {
            let k = kernel(vm)?;
            let mut k = borrow(&k)?;
            let record = Record {
                pid,
                path: path.to_string(),
                seq: step.seq,
            };
            let list = k.condition_links.entry(step.condition).or_default();
            if !list.contains(&record) {
                list.push(record);
            }
        }
        let Step::Register {
            mission,
            verdict,
            store,
        } = &step.step
        else {
            continue;
        };
        // C, then B (its borrow was checked in the plan; review of #775, item 5).
        react(vm, step.condition, *verdict)?;
        if *store && let Some(cell) = vm.service::<MissionConditions>() {
            cell.try_borrow_mut()
                .map_err(|_| VmError::ServiceBorrowed("MissionConditions"))?
                .store(mission, step.condition);
        }
    }
    Ok(())
}

/// Re-evaluates every kernel-registered condition stored under the mission whose stock path is `mission`, in
/// registration order (module
/// documentation): its verdict again from the service, then applied to every record of its list. An unloaded object, a
/// per-instance objective or a waypoint condition whose records would change is a typed refusal. Nothing without the
/// service. The caller (the bridge, after its own observers) marks a refusal as after partial writes.
pub fn reevaluate_conditions(vm: &mut Vm, mission: &str) -> Result<(), VmError> {
    let stored = match vm.service::<MissionConditions>() {
        None => return Ok(()),
        Some(cell) => cell
            .try_borrow()
            .map_err(|_| VmError::ServiceBorrowed("MissionConditions"))?
            .stored_under(mission),
    };
    for condition in stored {
        if !resident(vm, condition)? {
            return Err(not_read(CONDITION_NOT_LOADED));
        }
        let verdict = verdict(vm, condition)?;
        let known = linked_states(vm, condition)?;
        waypoint(vm, condition, verdict, &known)?;
        react(vm, condition, verdict)?;
    }
    Ok(())
}
