//! Owner: lane IMPL-AIMOFFSET-SET-PROFILE. `AnimNodeAimOffset.SetActiveProfileByName(name)` and
//! `SetActiveProfileByIndex(int)` in `ParityMode::Off`: link 1's stop in `maya:8` after `GetWeaponHoldDef`
//! (`WillowPawn.SetAimOffsetProfile` from `UpdateAnimSets`), and links 3, 4, 5, 13 and 14's. One rule for both keys;
//! `WillowAnimNodeAimOffset` keeps both bodies, so the keys cover it. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_AIMOFFSET_SET_PROFILE.md section 3 alone (lane
//! ANALYST-LINK1-AFTER-HOLD, branch docs/native-link1-after-hold-aimoffset-profile at 46a470ec, not yet reviewed);
//! this lane did not read the executable. **Every rule is UNVERIFIED in the running game.** Reference keeps the stub
//! policy.
//!
//! **Preflight, then the writes;** load-free (no load, no class-code load, nothing interned):
//! 1. **the receiver** is resident ([`NODE_NOT_LOADED`], `NotRead`) and an `Engine.AnimNodeAimOffset` or below it (found load-free), declaring
//!    `TemplateNode` (an object reference), `Profiles` (an array of a struct) and `CurrentProfileIndex` (an int);
//!    otherwise `VmError::Script` naming the field;
//! 2. **the list's owner** is `TemplateNode` when set, else the receiver. A `TemplateNode` the store has not loaded is
//!    [`TEMPLATE_NOT_LOADED`] (`NotRead`), before any field of it is read. The owner declares `Profiles` too;
//! 3. **the profiles.** When the store skipped the owner's `Profiles` tag (`DefStore::skipped_property`: the port has
//!    no reader for the binary-serialized elements), [`PROFILES`] (`LayerAbsent`, [`PROFILES_LAYER`]). A `Profiles`
//!    that was read, empty included, answers by the rules below. Each element read is a struct with a `ProfileName`
//!    (a name), else `VmError::Script`;
//! 4. **the index:** by name, the first element whose `ProfileName` is the name (by id), and no match is nothing; by
//!    index, the argument, and negative or not below the list's length is nothing. The same index as
//!    `CurrentProfileIndex` is nothing;
//! 5. **the rebuild,** planned before the write: with no `SkelComponent`, or none with a `SkeletalMesh`, it leaves
//!    `RequiredBones` and `AimCpntIndexLUT` empty (the routine's own gate); both must be declared arrays, else
//!    `VmError::Script`. With a mesh, [`REBUILD`] (`NotRead`: the sort and parent pass are not read), before any write.
//!    A `SkelComponent` the store has not loaded is [`COMPONENT_NOT_LOADED`] (`NotRead`).
//!
//! **Writes** (only on a change with no mesh): `CurrentProfileIndex` := the index, then `RequiredBones` and
//! `AimCpntIndexLUT` := empty where they are not empty already. Nothing else: not the tree, the component or the pawn.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_AIMOFFSET_SET_PROFILE.md";
/// The two native keys.
pub const BY_NAME: &str = "AnimNodeAimOffset.SetActiveProfileByName(name)";
pub const BY_INDEX: &str = "AnimNodeAimOffset.SetActiveProfileByIndex(int)";
/// A `TemplateNode` the store has not loaded.
pub const TEMPLATE_NOT_LOADED: &str =
    "SetActiveProfileByName: the aim-offset template node not loaded at set-up (a load is a write)";
/// A receiver the store has not loaded.
pub const NODE_NOT_LOADED: &str =
    "SetActiveProfileByName: an aim-offset node not loaded at set-up (a load is a write)";
/// A `SkelComponent` the store has not loaded.
pub const COMPONENT_NOT_LOADED: &str =
    "SetActiveProfileByName: the aim-offset node's SkelComponent not loaded at set-up (a load is a write)";
/// The profiles the store skipped.
pub const PROFILES: &str = "AnimNodeAimOffset: the profiles (a binary-serialized struct array)";
/// The layer [`PROFILES`] names.
pub const PROFILES_LAYER: &str = "a reader for AnimNodeAimOffset.Profiles";
/// The rebuild with a mesh.
pub const REBUILD: &str = "AnimNodeAimOffset: the required-bones rebuild's sort and parent pass";
const AIM_OFFSET: &str = "Engine.AnimNodeAimOffset";
const WHAT: &str = "AnimNodeAimOffset.SetActiveProfile";

fn script(message: String) -> VmError {
    VmError::Script { function: WHAT.to_string(), message }
}

/// Whether `o` is resident: neither a handle the store has not loaded (or failed) nor a slotless stand-in.
fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless))
}

/// `o`'s field `name`, whose declaration must satisfy `ok` (else `VmError::Script` naming it), with its property.
fn field<'a>(
    vm: &'a Vm,
    o: ObjRef,
    name: &str,
    kind: &str,
    ok: fn(&PropKind) -> bool,
) -> Result<(ow_object::ids::PropId, &'a Value), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let path = || crate::world::pawn_meshes::handle_path(vm, o);
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{}'s class declares no {name}", path())))?;
    if !vm.space.classes.decl(p).is_some_and(|d| ok(&d.kind)) {
        return Err(script(format!("{}'s class declares {name} as something other than {kind}", path())));
    }
    Ok((p, vm.space.read(o, PropAddr::scalar(p))?))
}

fn is_object(k: &PropKind) -> bool {
    matches!(k, PropKind::Object { .. })
}

fn is_array(k: &PropKind) -> bool {
    matches!(k, PropKind::Array(_))
}

fn is_struct_array(k: &PropKind) -> bool {
    matches!(k, PropKind::Array(inner) if matches!(**inner, PropKind::Struct(_)))
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match field(vm, o, name, "an object reference", is_object)?.1 {
        Value::Object(x) => Ok(*x),
        other => Err(script(format!("{name} holds {}, not an object", other.kind_name()))),
    }
}

/// The profile names of `owner`'s `Profiles`, or [`PROFILES`] when the store skipped them.
fn profile_names(vm: &Vm, owner: ObjRef) -> Result<Vec<NameId>, VmError> {
    let (prop, list) = field(vm, owner, "Profiles", "an array of a struct", is_struct_array)?;
    if vm.defs.skipped_property(&vm.space, owner, prop) {
        return Err(NotYet::layer_absent(PROFILES, PROFILES_LAYER, RECORD).into());
    }
    let items = list.elements().ok_or_else(|| script(format!("Profiles holds {}, not an array", list.kind_name())))?;
    let key = vm.space.names.find("ProfileName");
    items
        .iter()
        .map(|item| {
            let s = item.as_struct().ok_or_else(|| script(format!("a profile is a {}, not a struct", item.kind_name())))?;
            let at = key
                .and_then(|k| vm.space.classes.struct_layout(s.ty)?.field(k))
                .ok_or_else(|| script("a profile has no ProfileName".to_string()))?;
            match s.fields.get(at.0 as usize) {
                Some(Value::Name(n)) => Ok(*n),
                Some(other) => Err(script(format!("ProfileName holds {}, not a name", other.kind_name()))),
                None => Err(script("a profile has no ProfileName".to_string())),
            }
        })
        .collect()
}

/// What the call asks for.
enum Ask {
    Name(NameId),
    Index(i32),
}

/// The preflight (module documentation, steps 1 to 5): the writes to make, `None` for nothing.
fn plan(vm: &Vm, node: ObjRef, ask: Ask) -> Result<Option<(i32, Vec<&'static str>)>, VmError> {
    if !resident(vm, node)? {
        return Err(NotYet::not_read(NODE_NOT_LOADED, RECORD).into());
    }
    let class = vm.space.heap.get(node)?.class;
    let is_node = vm.space.classes.by_path(AIM_OFFSET).is_some_and(|a| vm.space.classes.is_child_of(class, a));
    if !is_node {
        let path = crate::world::pawn_meshes::handle_path(vm, node);
        return Err(script(format!("{path} is not an {AIM_OFFSET}")));
    }
    let template = object(vm, node, "TemplateNode")?;
    field(vm, node, "Profiles", "an array of a struct", is_struct_array)?;
    let current = match field(vm, node, "CurrentProfileIndex", "an int", |k| matches!(k, PropKind::Int))?.1 {
        Value::Int(i) => *i,
        other => return Err(script(format!("CurrentProfileIndex holds {}, not an int", other.kind_name()))),
    };
    let owner = match template {
        Some(t) if !resident(vm, t)? => return Err(NotYet::not_read(TEMPLATE_NOT_LOADED, RECORD).into()),
        Some(t) => t,
        None => node,
    };
    let names = profile_names(vm, owner)?;
    let index = match ask {
        Ask::Name(n) => match names.iter().position(|&x| x == n) {
            Some(i) => i32::try_from(i).map_err(|_| script("a profile list past i32".to_string()))?,
            None => return Ok(None),
        },
        Ask::Index(i) => {
            if i < 0 || usize::try_from(i).map_or(true, |u| u >= names.len()) {
                return Ok(None);
            }
            i
        }
    };
    if index == current {
        return Ok(None);
    }
    // The rebuild (step 5).
    let mesh = match object(vm, node, "SkelComponent")? {
        None => None,
        Some(c) if !resident(vm, c)? => return Err(NotYet::not_read(COMPONENT_NOT_LOADED, RECORD).into()),
        Some(c) => object(vm, c, "SkeletalMesh")?,
    };
    if mesh.is_some() {
        return Err(NotYet::not_read(REBUILD, RECORD).into());
    }
    let mut clear = Vec::new();
    for name in ["RequiredBones", "AimCpntIndexLUT"] {
        let (_, v) = field(vm, node, name, "an array", is_array)?;
        if v.elements().is_some_and(|e| !e.is_empty()) {
            clear.push(name);
        }
    }
    Ok(Some((index, clear)))
}

fn write(vm: &mut Vm, node: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(node)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("no {name}")))?;
    vm.space.write(node, PropAddr::scalar(p), v)?;
    Ok(())
}

fn run(c: &mut NativeCall<'_>, ask: Ask) -> Result<Value, VmError> {
    let node = c.this.ok_or_else(|| script("called without an aim-offset node".to_string()))?;
    if let Some((index, clear)) = plan(c.vm, node, ask)? {
        write(c.vm, node, "CurrentProfileIndex", Value::Int(index))?;
        for name in clear {
            write(c.vm, node, name, Value::array(Vec::new()))?;
        }
    }
    Ok(Value::None)
}

/// `SetActiveProfileByName(ProfileName)` (module documentation).
pub fn set_active_profile_by_name(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let name = match c.arg(0)? {
        Value::Name(n) => *n,
        other => return Err(script(format!("ProfileName holds {}, not a name", other.kind_name()))),
    };
    run(c, Ask::Name(name))
}

/// `SetActiveProfileByIndex(ProfileIndex)` (module documentation).
pub fn set_active_profile_by_index(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let index = match c.arg(0)? {
        Value::Int(i) => *i,
        other => return Err(script(format!("ProfileIndex holds {}, not an int", other.kind_name()))),
    };
    run(c, Ask::Index(index))
}

/// Binds both keys (Off only; the caller checks the mode). A key another pack bound first keeps its binding.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if !vm.has_native(BY_NAME) {
        vm.register_native(BY_NAME, Rc::new(set_active_profile_by_name))?;
    }
    if !vm.has_native(BY_INDEX) {
        vm.register_native(BY_INDEX, Rc::new(set_active_profile_by_index))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "aim_offset_tests.rs"]
mod tests;
