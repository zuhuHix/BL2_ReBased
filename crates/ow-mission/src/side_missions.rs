//! Owner: lane IMPL-ALL-EXPANSION-SIDE-MISSIONS (decision 2026-11-19-NNN, number pending).
//! `MissionTracker.AllExpansionSideMissionsComplete(Controller, Expansion)` in `ParityMode::Off`. AI-assisted (Claude
//! Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_ALL_EXPANSION_SIDE_MISSIONS_COMPLETE.md` alone (analyst
//! lane ANALYST-LINK12-EXPANSION-SIDE-MISSIONS, #550 at 690ae348) and the hub's ruling on #550's review; this lane did
//! not read the executable. **Every rule is UNVERIFIED in the running game.** Off only; Reference keeps the C++ stub.
//!
//! **The rule, in the note's order** (section 5), reading only and loading nothing:
//! 1. **Arguments:** each an object reference or None, else `VmError::Script`; a set expansion that is not a
//!    `WillowGame.DownloadableExpansionDefinition` is `VmError::Script`.
//! 2. **A None controller:** false.
//! 3. **The controller:** not a `WillowPlayerController`: `VmError::Script`. A strict subclass: [`SUBCLASS`],
//!    `NotYet { NotRead }` (whether it overrides the current-playthrough virtual is not read). Otherwise the index is
//!    the bridge's current playthrough (the caller passes it).
//! 4. **The index** outside the controller's `MissionPlaythroughs`: false.
//! 5. **The tracker's list** (the caller's step, `crate::bridge`): the set-up build's refusal replayed, then
//!    `NotYet { LayerAbsent }` while no list was built, as `GetAllMissions` answers.
//! 6. **The walk** ([`walk`]), over `crate::mission_list::missions` in its stand-in order (UNVERIFIED), stopping at
//!    the first failure: a None mission is skipped; an unloaded handle is [`unloaded_mission`], `NotYet { LayerAbsent }`;
//!    a mission counts when it is not `bPlotCritical`, its `DlcExpansion` is the given expansion (identity; None matches
//!    None), its `MarketingUnlock` is None and it is not `bRepeatable`; a counted mission's first entry in that
//!    playthrough's `MissionList` (by identity) must exist with `Status` `MS_Complete` (by name in the enum the
//!    record's `Status` field declares), else false.
//! 7. **All passed** (none counted included): true.

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_ALL_EXPANSION_SIDE_MISSIONS_COMPLETE.md";
/// The function named in `VmError::Script` errors.
pub const UNIT: &str = "MissionTracker.AllExpansionSideMissionsComplete";
pub const CONTROLLER_CLASS: &str = "WillowGame.WillowPlayerController";
pub const EXPANSION_CLASS: &str = "WillowGame.DownloadableExpansionDefinition";
/// Step 5's `LayerAbsent`, in `GetAllMissions`'s style (the same layer).
pub const LIST: &str = "MissionTracker.AllExpansionSideMissionsComplete (every record of the tracker's MissionList)";
pub const LIST_LAYER: &str = "the tracker's full mission list (not built in this world)";
/// Step 3's `NotRead`.
pub const SUBCLASS: &str = "MissionTracker.AllExpansionSideMissionsComplete with a controller of a WillowPlayerController \
                            subclass (whether it overrides the current-playthrough virtual is not read)";
/// Step 6's layer for an unloaded mission handle.
pub const LOAD_LAYER: &str = "the definition store's load of a MissionList mission (the walk loads nothing)";
/// The status the walk asks for, by name.
pub const COMPLETE: &str = "MS_Complete";

/// Step 6's `LayerAbsent` text for an unloaded mission.
pub fn unloaded_mission(path: &str) -> String {
    format!("MissionTracker.AllExpansionSideMissionsComplete: the unloaded MissionList mission {path}")
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn object_arg(v: &Value, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(script(format!("{what} holds {}, not an object", other.kind_name()))),
    }
}

fn is_a(vm: &Vm, class: ClassId, path: &str) -> bool {
    vm.space.classes.by_path(path).is_some_and(|p| vm.space.classes.is_child_of(class, p))
}

/// Property `name` of `o`, or bad data.
fn prop(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_path(vm, class))))?;
    Ok(vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?.clone())
}

/// Field `name` of the struct value `v`, or bad data.
fn field(vm: &Vm, v: &Value, name: &str) -> Result<Value, VmError> {
    let missing = || script(format!("a record without the field {name}"));
    let s = v.as_struct().ok_or_else(missing)?;
    let id = vm.space.names.find(name).ok_or_else(missing)?;
    let (index, _) = vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(id)).ok_or_else(missing)?;
    s.fields.get(usize::try_from(index).map_err(|_| missing())?).cloned().ok_or_else(missing)
}

fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

fn flag(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match prop(vm, o, name)? {
        Value::Bool(b) => Ok(b),
        other => Err(script(format!("{name} holds {}, not a bool", other.kind_name()))),
    }
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    object_arg(&prop(vm, o, name)?, name)
}

/// What steps 1 to 4 decide: an answer, or a walk over the controller's playthrough.
#[derive(Debug)]
pub enum Step {
    Answer(bool),
    Walk(Walk),
}

/// The walk's inputs: the expansion and the playthrough's `MissionList` records.
#[derive(Debug)]
pub struct Walk {
    expansion: Option<ObjRef>,
    entries: Vec<Value>,
}

/// Steps 1 to 4 (module documentation). `playthrough` is the current playthrough index.
pub fn prepare(vm: &Vm, controller: &Value, expansion: &Value, playthrough: impl FnOnce(&Vm) -> Result<i32, VmError>) -> Result<Step, VmError> {
    let controller = object_arg(controller, "the controller")?;
    let expansion = object_arg(expansion, "the expansion")?;
    if let Some(x) = expansion {
        let class = vm.space.heap.get(x)?.class;
        if !is_a(vm, class, EXPANSION_CLASS) {
            return Err(script(format!("{} is not a DownloadableExpansionDefinition", class_path(vm, class))));
        }
    }
    let Some(controller) = controller else {
        return Ok(Step::Answer(false));
    };
    let class = vm.space.heap.get(controller)?.class;
    let Some(base) = vm.space.classes.by_path(CONTROLLER_CLASS) else {
        return Err(script(format!("{} is not a WillowPlayerController", class_path(vm, class))));
    };
    if !vm.space.classes.is_child_of(class, base) {
        return Err(script(format!("{} is not a WillowPlayerController", class_path(vm, class))));
    }
    if class != base {
        return Err(NotYet::not_read(SUBCLASS, RECORD).into());
    }
    let index = playthrough(vm)?;
    let all = prop(vm, controller, "MissionPlaythroughs")?;
    let all = all.elements().ok_or_else(|| script("MissionPlaythroughs is not an array".to_string()))?;
    let Some(record) = usize::try_from(index).ok().and_then(|i| all.get(i)) else {
        return Ok(Step::Answer(false));
    };
    let list = field(vm, record, "MissionList")?;
    let entries = list.elements().ok_or_else(|| script("MissionList is not an array".to_string()))?.to_vec();
    Ok(Step::Walk(Walk { expansion, entries }))
}

/// Step 6 and 7 (module documentation) over the tracker's `missions` (each record's `MissionDef`, in order).
pub fn walk(vm: &Vm, w: &Walk, missions: &[Value]) -> Result<bool, VmError> {
    for m in missions {
        let Some(def) = object_arg(m, "a MissionList record's MissionDef")? else {
            continue;
        };
        if vm.defs.is_unloaded(&vm.space, def) {
            let path = handle_path(vm, def);
            return Err(NotYet::layer_absent(unloaded_mission(&path), LOAD_LAYER, RECORD).into());
        }
        let counted = !flag(vm, def, "bPlotCritical")?
            && object(vm, def, "DlcExpansion")? == w.expansion
            && object(vm, def, "MarketingUnlock")?.is_none()
            && !flag(vm, def, "bRepeatable")?;
        if !counted {
            continue;
        }
        let mut entry = None;
        for e in &w.entries {
            if object_arg(&field(vm, e, "MissionDef")?, "MissionDef")? == Some(def) {
                entry = Some(e);
                break;
            }
        }
        let Some(entry) = entry else {
            return Ok(false);
        };
        if field(vm, entry, "Status")? != Value::Byte(complete_value(vm, entry)?) {
            return Ok(false);
        }
    }
    Ok(true)
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

/// `MS_Complete`'s value in the enum the entry's `Status` field declares.
fn complete_value(vm: &Vm, entry: &Value) -> Result<u8, VmError> {
    let missing = || script(format!("the record's Status declares no enum with {COMPLETE}"));
    let s = entry.as_struct().ok_or_else(missing)?;
    let id = vm.space.names.find("Status").ok_or_else(missing)?;
    let (_, decl) = vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(id)).ok_or_else(missing)?;
    let PropKind::Byte { enum_: Some(e) } = &decl.kind else {
        return Err(missing());
    };
    let layout = vm.space.classes.enum_layout(*e).ok_or_else(missing)?;
    let i = layout.values().iter().position(|v| vm.space.names.text(*v) == COMPLETE).ok_or_else(missing)?;
    u8::try_from(i).map_err(|_| missing())
}

#[cfg(test)]
#[path = "side_missions_tests.rs"]
mod tests;
