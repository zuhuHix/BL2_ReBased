//! Owner: lane IMPL-REFRESH-BALANCE-DATA (decision 2026-11-19-NNN, number pending).
//! `WillowPlayerController.RefreshBalanceDataFromMissionCompletion(Mission)` in `ParityMode::Off`. AI-assisted (Claude
//! Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_REFRESH_BALANCE_DATA_FROM_MISSION_COMPLETION.md` alone
//! (analyst lane ANALYST-REFRESH-BALANCE-DATA, #559, accepted at be877676) and the hub's ruling on its residency
//! question; this lane did not read the executable. **Every rule is UNVERIFIED in the running
//! game.** Off only; Reference keeps the C++ stub.
//!
//! **The rule** ([`plan`], the note's section 5), reading only and loading nothing; the caller writes the plan:
//! 1. **The receiver:** a `WillowPlayerController`, else `VmError::Script`; a strict subclass is [`SUBCLASS`],
//!    `NotYet { NotRead }` (as #554 answers it). The playthrough index is the bridge's current playthrough (the
//!    caller passes it). A negative index is [`NEGATIVE`], `NotYet { NotRead }`.
//! 2. **The mission:** an object reference to a `MissionDefinition`; None or another class is `VmError::Script`; an
//!    unloaded handle is `NotYet { LayerAbsent }` (the store's load).
//! 3. **The globals definition** (the caller passes the bridge's answer to `GetGlobalsDefinition`): None answers false
//!    and writes nothing; an unloaded handle is `NotYet { LayerAbsent }`.
//! 4. **The table:** the mission's `DlcExpansion.RegionBalanceData` when `DlcExpansion` is set (an unloaded handle is
//!    `NotYet { LayerAbsent }`), else the globals definition's. The playthrough index picks an entry **by position**
//!    (`PlayThroughNumber` is not compared); an index past the table answers false, with no clamp.
//! 5. **The search**, in array order: every `BalanceDefinitions` entry (None is `VmError::Script`; an unloaded handle is
//!    [`unloaded_balance`], `NotYet { LayerAbsent }`, the hub's ruling: the harness preloads them), every
//!    `BalanceByRegion` record, every `MissionOverrides` record. An override whose `Mission` is the completed mission
//!    (object identity, the handle never resolved) sets the answer to true and adds one reset of the record's `Region`.
//!    Every matching override adds one, duplicates included; the search does not stop at a match.
//! 6. **The plan:** each reset, in order, removes the first `RegionGameStages` record whose `RegionDef` is that region
//!    and whose `PlaythroughIdx` is the current playthrough, against the array as the earlier removals leave it. No
//!    such record: nothing. A record that is not the `RegionGameStageData` shape is `VmError::Script`.
//!
//! **The write** (the caller's, [`crate::bridge`]): `RegionGameStages` once, only when a record was removed. Nothing
//! refuses after it. The refusals all come before it, so they are unmarked here; the turn-in's caller
//! (`UpdateMissionStatus`, which wrote first) marks them.

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_REFRESH_BALANCE_DATA_FROM_MISSION_COMPLETION.md";
/// The function named in `VmError::Script` errors.
pub const UNIT: &str = "WillowPlayerController.RefreshBalanceDataFromMissionCompletion";
pub const CONTROLLER_CLASS: &str = "WillowGame.WillowPlayerController";
pub const MISSION_CLASS: &str = "WillowGame.MissionDefinition";
/// Step 1's `NotRead`s.
pub const SUBCLASS: &str = "RefreshBalanceDataFromMissionCompletion on a controller of a WillowPlayerController subclass \
                            (whether it overrides the current-playthrough virtual is not read)";
pub const NEGATIVE: &str = "RefreshBalanceDataFromMissionCompletion: a negative playthrough index";
/// The layer of every unloaded-handle refusal: the native loads nothing.
pub const BALANCE_LAYER: &str = "the definition store's load of a region balance definition (the native loads nothing)";
pub const LOAD_LAYER: &str = "the definition store's load (the native loads nothing)";

/// Step 5's `LayerAbsent` text for an unloaded balance definition.
pub fn unloaded_balance(path: &str) -> String {
    format!("RefreshBalanceDataFromMissionCompletion: the region balance definition {path}")
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

fn is_a(vm: &Vm, class: ClassId, path: &str) -> bool {
    vm.space.classes.by_path(path).is_some_and(|p| vm.space.classes.is_child_of(class, p))
}

fn object_of(v: &Value, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(script(format!("{what} holds {}, not an object", other.kind_name()))),
    }
}

fn prop(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn field(vm: &Vm, v: &Value, name: &str) -> Result<Value, VmError> {
    let missing = || script(format!("a record without the field {name}"));
    let s = v.as_struct().ok_or_else(missing)?;
    let id = vm.space.names.find(name).ok_or_else(missing)?;
    let (index, _) = vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(id)).ok_or_else(missing)?;
    s.fields.get(usize::try_from(index).map_err(|_| missing())?).cloned().ok_or_else(missing)
}

fn array(v: Value, what: &str) -> Result<Vec<Value>, VmError> {
    v.elements().map(<[Value]>::to_vec).ok_or_else(|| script(format!("{what} is not an array")))
}

/// The store's object path of an unloaded handle (its export table entry; nothing is loaded).
fn handle_path(vm: &Vm, o: ObjRef) -> String {
    if let Ok(x) = vm.space.heap.get(o)
        && let ow_object::heap::Origin::Export { package, index } | ow_object::heap::Origin::Resource { package, index } = x.origin
        && let Ok(path) = ow_defs::identity::full_path(vm.defs.packages().get(package), index)
    {
        return path;
    }
    format!("{o:?}")
}

fn loaded(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        let text = format!("RefreshBalanceDataFromMissionCompletion: the unloaded {what} {}", handle_path(vm, o));
        return Err(NotYet::layer_absent(text, LOAD_LAYER, RECORD).into());
    }
    Ok(())
}

/// What the native does: its answer, and the new `RegionGameStages` when a record is removed.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub matched: bool,
    pub stages: Option<Value>,
}

impl Outcome {
    fn nothing() -> Self {
        Outcome { matched: false, stages: None }
    }
}

/// Steps 1 to 6 (module documentation). Reads only. `definition` is the bridge's globals definition, `playthrough`
/// its current playthrough index.
pub fn plan(vm: &Vm, controller: ObjRef, mission: &Value, definition: Option<ObjRef>, playthrough: i32) -> Result<Outcome, VmError> {
    // 1. The receiver and the index.
    let class = vm.space.heap.get(controller)?.class;
    let Some(base) = vm.space.classes.by_path(CONTROLLER_CLASS).filter(|&b| vm.space.classes.is_child_of(class, b)) else {
        return Err(script(format!("{} is not a WillowPlayerController", class_path(vm, class))));
    };
    if class != base {
        return Err(NotYet::not_read(SUBCLASS, RECORD).into());
    }
    if playthrough < 0 {
        return Err(NotYet::not_read(NEGATIVE, RECORD).into());
    }
    // 2. The mission.
    let Some(mission) = object_of(mission, "the mission")? else {
        return Err(script("called with a None mission".to_string()));
    };
    let mission_class = vm.space.heap.get(mission)?.class;
    if !is_a(vm, mission_class, MISSION_CLASS) {
        return Err(script(format!("{} is not a MissionDefinition", class_path(vm, mission_class))));
    }
    loaded(vm, mission, "mission")?;
    // 3. The globals definition.
    let Some(definition) = definition else {
        return Ok(Outcome::nothing());
    };
    loaded(vm, definition, "globals definition")?;
    // 4. The table and the entry, by position.
    let table_owner = match object_of(&prop(vm, mission, "DlcExpansion")?, "DlcExpansion")? {
        Some(x) => {
            loaded(vm, x, "expansion")?;
            x
        }
        None => definition,
    };
    let table = array(prop(vm, table_owner, "RegionBalanceData")?, "RegionBalanceData")?;
    let Some(entry) = usize::try_from(playthrough).ok().and_then(|i| table.get(i)) else {
        return Ok(Outcome::nothing());
    };
    // 5. The search: one reset per matching override, duplicates included.
    let mut resets: Vec<Option<ObjRef>> = Vec::new();
    for b in array(field(vm, entry, "BalanceDefinitions")?, "BalanceDefinitions")? {
        let Some(balance) = object_of(&b, "a BalanceDefinitions entry")? else {
            return Err(script("a None BalanceDefinitions entry".to_string()));
        };
        if vm.defs.is_unloaded(&vm.space, balance) {
            return Err(NotYet::layer_absent(unloaded_balance(&handle_path(vm, balance)), BALANCE_LAYER, RECORD).into());
        }
        for region in array(prop(vm, balance, "BalanceByRegion")?, "BalanceByRegion")? {
            let region_def = object_of(&field(vm, &region, "Region")?, "Region")?;
            for o in array(field(vm, &region, "MissionOverrides")?, "MissionOverrides")? {
                if object_of(&field(vm, &o, "Mission")?, "an override's Mission")? == Some(mission) {
                    resets.push(region_def);
                }
            }
        }
    }
    let matched = !resets.is_empty();
    // 6. The plan, one removal per reset against the array as the earlier removals leave it.
    let mut stages = array(prop(vm, controller, "RegionGameStages")?, "RegionGameStages")?;
    let mut removed = false;
    for region in resets {
        let mut hit = None;
        for (i, record) in stages.iter().enumerate() {
            let def = object_of(&field(vm, record, "RegionDef")?, "RegionDef")?;
            let idx = match field(vm, record, "PlaythroughIdx")? {
                Value::Int(n) => n,
                other => return Err(script(format!("PlaythroughIdx holds {}, not an int", other.kind_name()))),
            };
            if def == region && idx == playthrough {
                hit = Some(i);
                break;
            }
        }
        if let Some(i) = hit {
            stages.remove(i);
            removed = true;
        }
    }
    Ok(Outcome { matched, stages: removed.then(|| Value::array(stages)) })
}

#[cfg(test)]
#[path = "balance_refresh_tests.rs"]
mod tests;
