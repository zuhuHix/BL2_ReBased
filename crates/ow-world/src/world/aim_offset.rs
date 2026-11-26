//! Owner: lane IMPL-AIMOFFSET-SET-PROFILE. `AnimNodeAimOffset.SetActiveProfileByName(name)` and
//! `SetActiveProfileByIndex(int)` in `ParityMode::Off`: link 1's stop in `maya:8` after `GetWeaponHoldDef`
//! (`WillowPawn.SetAimOffsetProfile` from `UpdateAnimSets`), and links 3, 4, 5, 13 and 14's. One rule for both keys;
//! `WillowAnimNodeAimOffset` keeps both bodies, so the keys cover it. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_AIMOFFSET_SET_PROFILE.md section 3 alone (lane
//! ANALYST-LINK1-AFTER-HOLD, #730 at its review r1 fix bff35b46); this lane did not read the executable. **Every rule
//! is UNVERIFIED in the running game.** Reference keeps the stub policy.
//!
//! **Preflight, then the writes;** load-free (no load, no class-code load, nothing interned):
//! 1. **the receiver** is resident ([`NODE_NOT_LOADED`], `NotRead`) and an `Engine.AnimNodeAimOffset` or below it
//!    (found load-free), declaring `TemplateNode` and `SkelComponent` (object references), `Profiles` (an array of a
//!    struct), `CurrentProfileIndex` (an int), and `RequiredBones` and `AimCpntIndexLUT` (arrays of bytes); otherwise
//!    `VmError::Script` naming the field. All are checked before anything else, so the write is all or nothing;
//! 2. **the list's owner** is `TemplateNode` when set, else the receiver. A `TemplateNode` the store has not loaded is
//!    [`TEMPLATE_NOT_LOADED`] (`NotRead`), before any field of it is read. The owner declares `Profiles` too;
//! 3. **the profiles** (lane IMPL-AIMOFFSET-PROFILES, the hub's ruling on the interim rule): on the list's owner,
//!    - a non-empty `Profiles` is the slot's values; each element is a struct with a `ProfileName` (a name), else
//!      `VmError::Script`;
//!    - an empty one the store marked skipped (`DefStore::is_skipped`) is the profiles layer's names
//!      (crate::world::aim_profiles, the binary elements read from the owner's export); its refusal is
//!      [`PROFILES`]-led `LayerAbsent` on [`PROFILES_LAYER`];
//!    - an empty one on a loaded definition that is not marked skipped **and whose own export tags `Profiles`** was
//!      decoded: an empty list, so "no match";
//!    - an empty one on a loaded definition whose export does not tag it carries the value it was built over (an
//!      archetype's, whose skip mark the build does not carry, or the class default's), which the store does not
//!      report: the reader's "no Profiles tag" `LayerAbsent` (review r1 of #737, the hub's ruling);
//!    - an empty one on any other owner (a run-time object) keeps the interim rule: [`PROFILES`] (`LayerAbsent`).
//!
//!    A profile name with no entry in the name table cannot be the asked name (nothing is interned);
//! 4. **the index:** by name, the first element whose `ProfileName` is the name (by id), and no match is nothing; by
//!    index, the argument, and negative or not below the list's length is nothing. The same index as
//!    `CurrentProfileIndex` is nothing;
//! 5. **the rebuild** (lane IMPL-AIMOFFSET-REBUILD, from docs/verification/NATIVE_AIMOFFSET_REBUILD.md section 3 alone,
//!    #741 with its r1 fix; this lane did not read the executable; UNVERIFIED), planned in the preflight, write-free
//!    and load-free, in this order, the first refusal the answer:
//!    1. the gate: with no `SkelComponent`, or a resident one with `SkeletalMesh` None, both arrays are written empty; a
//!       `SkelComponent` the store has not loaded is [`COMPONENT_NOT_LOADED`] (`NotRead`);
//!    2. the mesh's reference skeleton (`skeletal::reference_skeleton`), its own refusals standing (no source or a mesh
//!       not loaded: `LayerAbsent`; a skeleton that fails its checks, repeated names included: `VmError::Script`);
//!    3. more than 256 bones: [`PAST_A_BYTE`] (`NotRead`);
//!    4. the current profile's components' bone names: from a decoded `Profiles`, its entry's `AimComponents`, each a
//!       struct with a name `BoneName` (else `VmError::Script`); from the profiles layer, the source's text names. More
//!       than 255 components: [`PAST_A_BYTE`] (`NotRead`);
//!    5. the match, per component in order, interning nothing: a layer name through `ReferenceSkeleton::bone_index`
//!       (case-insensitive text), a decoded name id by each bone's `names.find`; no bone, or a `None` name, skips the
//!       component. A match appends the bone to `RequiredBones` and sets its `AimCpntIndexLUT` entry (one 255 per bone
//!       before) to the component's position;
//!    6. `RequiredBones` sorted ascending, then the parent pass: each missing ancestor goes directly before the first
//!       bone that needs it (its own ancestors before it in turn), a parent already in the list is not added again,
//!       and a root bone (`parent` None) needs nothing. So bones 3 and 4, with 1 and 3 children of the root and 4 a
//!       child of 1, give 0, 3, 1, 4.
//!
//! **Writes** (only on a change): `CurrentProfileIndex` := the index, then `RequiredBones`, then `AimCpntIndexLUT`, as
//! byte arrays. Every refusal comes before the first write. Nothing else: not the tree, the component or the pawn.

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
pub const COMPONENT_NOT_LOADED: &str = "AnimNodeAimOffset: the skeletal component not loaded at set-up (a load is a write)";
/// The profiles the store skipped.
pub const PROFILES: &str = "AnimNodeAimOffset: the profiles (a binary-serialized struct array)";
/// The layer [`PROFILES`] names.
pub const PROFILES_LAYER: &str = "a reader for AnimNodeAimOffset.Profiles";
/// The rebuild with a mesh, before lane IMPL-AIMOFFSET-REBUILD (kept for reference; no longer answered).
pub const REBUILD: &str = "AnimNodeAimOffset: the required-bones rebuild's sort and parent pass";
/// The rebuild's byte limits (more than 256 bones, or more than 255 components).
pub const PAST_A_BYTE: &str = "AnimNodeAimOffset: a required-bones rebuild past a byte";
/// The rebuild's record.
pub const RECORD_REBUILD: &str = "docs/verification/NATIVE_AIMOFFSET_REBUILD.md";
/// The table's "none" entry.
pub const LUT_NONE: u8 = 255;
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

fn is_byte_array(k: &PropKind) -> bool {
    matches!(k, PropKind::Array(inner) if matches!(**inner, PropKind::Byte { .. }))
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

/// A component's bone name, as the profile list gives it.
#[derive(Debug, Clone)]
enum Bone {
    /// A decoded `BoneName` (its id).
    Id(NameId),
    /// The profiles layer's text (`Name_N` for a numbered name).
    Text(String),
}

/// Where the profile list came from: the slot's decoded values, or the profiles layer's read.
enum Profiles {
    Decoded(Vec<Option<NameId>>),
    Layer(Rc<Vec<crate::world::aim_profiles::SourceProfile>>),
}

impl Profiles {
    fn names(&self, vm: &Vm) -> Vec<Option<NameId>> {
        match self {
            Profiles::Decoded(n) => n.clone(),
            Profiles::Layer(l) => l.iter().map(|p| vm.space.names.find(&p.name)).collect(),
        }
    }
}

/// The bone names of profile `index`'s components (step 5.4).
fn component_bones(vm: &Vm, owner: ObjRef, profiles: &Profiles, index: usize) -> Result<Vec<Bone>, VmError> {
    if let Profiles::Layer(l) = profiles {
        let p = l.get(index).ok_or_else(|| script(format!("no profile at {index}")))?;
        return Ok(p.bones.iter().map(|b| Bone::Text(b.clone())).collect());
    }
    let (_, list) = field(vm, owner, "Profiles", "an array of a struct", is_struct_array)?;
    let item = list.elements().and_then(|e| e.get(index)).ok_or_else(|| script(format!("no profile at {index}")))?;
    let s = item.as_struct().ok_or_else(|| script(format!("a profile is a {}, not a struct", item.kind_name())))?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| script("a profile with no layout".to_string()))?;
    let (at, decl) = vm
        .space
        .names
        .find("AimComponents")
        .and_then(|k| layout.field(k))
        .ok_or_else(|| script("a profile has no AimComponents".to_string()))?;
    if !is_struct_array(&decl.kind) {
        return Err(script("AimComponents is declared as something other than an array of a struct".to_string()));
    }
    let components = s.fields.get(at as usize).and_then(Value::elements).ok_or_else(|| script("AimComponents holds no array".to_string()))?;
    let key = vm.space.names.find("BoneName");
    let mut out = Vec::with_capacity(components.len());
    for c in components {
        let cs = c.as_struct().ok_or_else(|| script(format!("an aim component is a {}, not a struct", c.kind_name())))?;
        let cl = vm.space.classes.struct_layout(cs.ty).ok_or_else(|| script("an aim component with no layout".to_string()))?;
        let (bi, bd) = key.and_then(|k| cl.field(k)).ok_or_else(|| script("an aim component has no BoneName".to_string()))?;
        if !matches!(bd.kind, PropKind::Name) {
            return Err(script("an aim component's BoneName is not declared as a name".to_string()));
        }
        match cs.fields.get(bi as usize) {
            Some(Value::Name(n)) => out.push(Bone::Id(*n)),
            other => return Err(script(format!("BoneName holds {other:?}, not a name"))),
        }
    }
    Ok(out)
}

/// The rebuild's two lists (steps 5.2 to 5.6), on `mesh`'s skeleton, for `bones` in component order.
fn rebuild_lists(vm: &Vm, mesh: ObjRef, bones: impl FnOnce() -> Result<Vec<Bone>, VmError>) -> Result<(Vec<u8>, Vec<u8>), VmError> {
    let skeleton = crate::world::skeletal::reference_skeleton(vm, mesh)?;
    let count = skeleton.len();
    if count > 256 {
        return Err(NotYet::not_read(PAST_A_BYTE, RECORD_REBUILD).into());
    }
    let bones = bones()?;
    if bones.len() > 255 {
        return Err(NotYet::not_read(PAST_A_BYTE, RECORD_REBUILD).into());
    }
    let mut lut = vec![LUT_NONE; count];
    let mut required: Vec<usize> = Vec::new();
    for (i, bone) in bones.iter().enumerate() {
        let found = match bone {
            Bone::Id(id) if *id == NameId::NONE => None,
            Bone::Id(id) => skeleton.bones.iter().position(|b| vm.space.names.find(&b.name) == Some(*id)),
            Bone::Text(t) if t.eq_ignore_ascii_case("None") => None,
            Bone::Text(t) => skeleton.bone_index(t),
        };
        if let Some(b) = found {
            required.push(b);
            lut[b] = u8::try_from(i).map_err(|_| NotYet::not_read(PAST_A_BYTE, RECORD_REBUILD))?;
        }
    }
    required.sort_unstable();
    let parents: Vec<Option<usize>> = skeleton.bones.iter().map(|b| b.parent).collect();
    let required = with_parents(&required, &parents);
    let bytes = required
        .into_iter()
        .map(|b| u8::try_from(b).map_err(|_| VmError::from(NotYet::not_read(PAST_A_BYTE, RECORD_REBUILD))))
        .collect::<Result<Vec<u8>, VmError>>()?;
    Ok((bytes, lut))
}

/// The parent pass (step 5.6) over `list`, with `parents[b]` each bone's parent (`None` for a root).
pub fn with_parents(list: &[usize], parents: &[Option<usize>]) -> Vec<usize> {
    let mut out = list.to_vec();
    let mut i = 0;
    while i < out.len() {
        let b = out[i];
        if let Some(Some(parent)) = parents.get(b)
            && !out.contains(parent)
        {
            // The missing parent goes directly before the bone; the loop then checks the parent itself.
            out.insert(i, *parent);
            continue;
        }
        i += 1;
    }
    out
}

/// The profile names of `owner`'s `Profiles` (module documentation, step 3), each as its name-table id (`None` for a
/// name the table does not hold, which matches no asked name).
fn profile_names(vm: &Vm, owner: ObjRef) -> Result<Profiles, VmError> {
    let (prop, list) = field(vm, owner, "Profiles", "an array of a struct", is_struct_array)?;
    let items = list.elements().ok_or_else(|| script(format!("Profiles holds {}, not an array", list.kind_name())))?;
    if items.is_empty() {
        if vm.defs.is_skipped(&vm.space, owner, prop) {
            return Ok(Profiles::Layer(crate::world::aim_profiles::profiles(vm, owner)?));
        }
        if vm.defs.is_loaded(&vm.space, owner) {
            if crate::world::aim_profiles::has_own_profiles_tag(vm, owner)? {
                return Ok(Profiles::Decoded(Vec::new()));
            }
            return Err(crate::world::aim_profiles::no_profiles_tag());
        }
        return Err(NotYet::layer_absent(PROFILES, PROFILES_LAYER, RECORD).into());
    }
    let key = vm.space.names.find("ProfileName");
    let names: Result<Vec<Option<NameId>>, VmError> = items
        .iter()
        .map(|item| {
            let s = item.as_struct().ok_or_else(|| script(format!("a profile is a {}, not a struct", item.kind_name())))?;
            let at = key
                .and_then(|k| vm.space.classes.struct_layout(s.ty)?.field(k))
                .ok_or_else(|| script("a profile has no ProfileName".to_string()))?;
            match s.fields.get(at.0 as usize) {
                Some(Value::Name(n)) => Ok(Some(*n)),
                Some(other) => Err(script(format!("ProfileName holds {}, not a name", other.kind_name()))),
                None => Err(script("a profile has no ProfileName".to_string())),
            }
        })
        .collect();
    Ok(Profiles::Decoded(names?))
}

/// What the call asks for.
enum Ask {
    Name(NameId),
    Index(i32),
}

/// The writes a change makes: `CurrentProfileIndex`, `RequiredBones`, `AimCpntIndexLUT`.
type Writes = (i32, Vec<u8>, Vec<u8>);

/// The preflight (module documentation, steps 1 to 5): the writes to make, `None` for nothing.
fn plan(vm: &Vm, node: ObjRef, ask: Ask) -> Result<Option<Writes>, VmError> {
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
    let component = object(vm, node, "SkelComponent")?;
    field(vm, node, "Profiles", "an array of a struct", is_struct_array)?;
    for name in ["RequiredBones", "AimCpntIndexLUT"] {
        field(vm, node, name, "an array of bytes", is_byte_array)?;
    }
    let current = match field(vm, node, "CurrentProfileIndex", "an int", |k| matches!(k, PropKind::Int))?.1 {
        Value::Int(i) => *i,
        other => return Err(script(format!("CurrentProfileIndex holds {}, not an int", other.kind_name()))),
    };
    let owner = match template {
        Some(t) if !resident(vm, t)? => return Err(NotYet::not_read(TEMPLATE_NOT_LOADED, RECORD).into()),
        Some(t) => t,
        None => node,
    };
    let profiles = profile_names(vm, owner)?;
    let names = profiles.names(vm);
    let index = match ask {
        Ask::Name(n) => match names.iter().position(|&x| x == Some(n)) {
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
    let mesh = match component {
        None => None,
        Some(c) if !resident(vm, c)? => return Err(NotYet::not_read(COMPONENT_NOT_LOADED, RECORD).into()),
        Some(c) => object(vm, c, "SkeletalMesh")?,
    };
    let Some(mesh) = mesh else {
        return Ok(Some((index, Vec::new(), Vec::new())));
    };
    let at = usize::try_from(index).map_err(|_| script("a negative profile index".to_string()))?;
    let (required, lut) = rebuild_lists(vm, mesh, || component_bones(vm, owner, &profiles, at))?;
    Ok(Some((index, required, lut)))
}

fn write(vm: &mut Vm, node: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(node)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("no {name}")))?;
    vm.space.write(node, PropAddr::scalar(p), v)?;
    Ok(())
}

fn run(c: &mut NativeCall<'_>, ask: Ask) -> Result<Value, VmError> {
    let node = c.this.ok_or_else(|| script("called without an aim-offset node".to_string()))?;
    if let Some((index, required, lut)) = plan(c.vm, node, ask)? {
        let bytes = |v: Vec<u8>| Value::array(v.into_iter().map(Value::Byte).collect());
        write(c.vm, node, "CurrentProfileIndex", Value::Int(index))?;
        write(c.vm, node, "RequiredBones", bytes(required))?;
        write(c.vm, node, "AimCpntIndexLUT", bytes(lut))?;
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

#[cfg(test)]
#[path = "aim_offset_rebuild_tests.rs"]
mod rebuild_tests;
