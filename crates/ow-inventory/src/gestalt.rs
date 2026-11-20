//! Owner: lane IMPL-GESTALT-MESH-LAYER. AI-assisted (Claude Code, Opus 5.5). `WillowWeapon.BuildCompositeMesh` and
//! `SkeletalMeshComponent.InitGestaltMeshDataFromOther` in Off, coded from the two analyst notes alone:
//! NATIVE_BUILD_COMPOSITE_MESH.md (#588, at ce0ec917) and NATIVE_INIT_GESTALT_MESH_DATA.md (#591, at 792ba72c). This
//! lane read neither native (clean room). Every rule is the notes' reading of the executable, **UNVERIFIED** in the
//! running game.
//!
//! **`BuildCompositeMesh(C, D)`** (NATIVE_BUILD_COMPOSITE_MESH.md section 4). The preflight runs in the note's order,
//! and the first failure is the answer:
//! 1. `D.WeaponTypeDefinition` (an object field, else a data error). None answers None, and nothing else is checked,
//!    the component included.
//! 2. The type must be resident, else [`NOT_LOADED`] (`NotRead`). Its `GestaltMesh` must be an object field.
//! 3. The path choice. When `GestaltMesh` is set, that definition must be resident ([`NOT_LOADED`]) and its
//!    `GestaltSkeletalMesh` an object field. Both set: the gestalt path. Otherwise: the non-gestalt path.
//! 4. The part walk: `D`'s slots in `AllParts` order ([`WEAPON_ALL_PARTS`] without the two name parts), empty slots
//!    skipped. Each part must be resident ([`NOT_LOADED`]). Then come `bIsGestaltMode` (a bool) and
//!    `NongestaltSkeletalMesh` (an object), and on the gestalt path also `GestaltModeSkeletalMeshName` (a name) and
//!    `AdditionalGestaltModeSkeletalMeshNames` (a static array of names). A part not in gestalt mode is named by its
//!    `NongestaltSkeletalMesh` handle's object name. The heap holds that name for an unloaded handle too, so it is read
//!    without loading the mesh.
//! 5. The component `C`, on the gestalt path only:
//!    - None is a data error;
//!    - so is a class that is not `Engine.SkeletalMeshComponent`, or that declares no struct `GestaltData`;
//!    - an unloaded handle is [`COMPONENT_NOT_LOADED`] (`NotRead`).
//!
//! **Its answers:**
//! - the gestalt path refuses `LayerAbsent(`[`GESTALT_LAYER`]`)` ([`GESTALT_DATA`]);
//! - the non-gestalt path answers None for zero meshes and that mesh for one. Two or more refuse
//!   `LayerAbsent(`[`MERGE_LAYER`]`)` ([`MERGE`]).
//!
//! **`InitGestaltMeshDataFromOther(From)`** (NATIVE_INIT_GESTALT_MESH_DATA.md section 5). The preflight, in order:
//! 1. the receiver must be resident ([`RECEIVER_NOT_LOADED`], `NotRead`) and declare a struct `GestaltData` (else a
//!    data error);
//! 2. the argument:
//!    - None is a data error;
//!    - so is one that is not a `SkeletalMeshComponent` or declares no struct `GestaltData`;
//!    - an unloaded handle is [`ARGUMENT_NOT_LOADED`] (`NotRead`).
//!
//! 3. the receiver being the argument is a self-assignment: success, with nothing written, even while the layer is
//!    absent (UNVERIFIED: the notes did not read the self-copy separately);
//! 4. both components' `GestaltData` slots must be held whole: one complete value, read as it stands, with no decode
//!    and no load ([`held_whole`]). A component whose slots the store has not filled (a loaded stand-in with no slots)
//!    is [`NOT_DECODED`] (`NotRead`). A slot of the wrong shape is the same `LayerAbsent` as the answer.
//!
//! Any other pair then refuses `LayerAbsent(`[`GESTALT_LAYER`]`)` ([`INIT_GESTALT_DATA`]), the same layer as the
//! fill. With the layer, the copy would be one atomic value replacement (a single heap write of the whole container),
//! after checks 1 to 4.
//!
//! **Not built here: the gestalt data layer itself** (the component's `GestaltData` value and the fill that writes it).
//! The fill needs several things the notes did not read:
//! - which apply step writes `PartMeshNames`;
//! - how the socket mappings are filled;
//! - how the reference-pose bounds grow per kept part;
//! - which declared member each write lands in. The notes match that only by position (UNVERIFIED).
//!
//! Filling it now would mean inventing those, so both natives stop at the refusal. The copy alone (a whole-value
//! assignment, the second note's section 3) would copy a container that nothing fills, so it waits for the fill.
//!
//! **Write-free by construction.** Both answers take `&Vm`. They cannot write, load (a lazy load is a write) or draw.
//! Every refusal comes before either native's first write, so none carries the "stopped after partial writes: "
//! prefix inside the native. The caller's write tracking marks it when earlier steps wrote.
//!
//! **Only in `ParityMode::Off`.** The C++ reference binds neither native, so Reference keeps the VM's stub policy.
//! `WillowItem`'s twin of `BuildCompositeMesh` is not bound here.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::equip::natives::WEAPON_ALL_PARTS;
use crate::equip::{bad, field, object_of};

/// The records the two natives follow.
pub const BUILD_RECORD: &str = "docs/verification/NATIVE_BUILD_COMPOSITE_MESH.md";
pub const INIT_RECORD: &str = "docs/verification/NATIVE_INIT_GESTALT_MESH_DATA.md";

/// The native keys [`register`] binds in Off.
pub const BUILD_COMPOSITE_MESH: &str = "WillowWeapon.BuildCompositeMesh(object,weapondefinitiondata)";
pub const INIT_GESTALT_MESH_DATA: &str = "SkeletalMeshComponent.InitGestaltMeshDataFromOther(object)";

/// The absent layer both natives name (the notes' shared layer name).
pub const GESTALT_LAYER: &str = "a gestalt component data layer (a skeletal mesh component's GestaltData)";
/// The absent layer of the non-gestalt path with two or more meshes.
pub const MERGE_LAYER: &str = "a skeletal mesh merge";

/// The named `NotYet`s.
pub const GESTALT_DATA: &str = "BuildCompositeMesh: the gestalt mesh data of the component";
pub const MERGE: &str = "BuildCompositeMesh: merging the parts' meshes";
pub const NOT_LOADED: &str = "BuildCompositeMesh: a definition it reads is not loaded (a load is a write)";
pub const COMPONENT_NOT_LOADED: &str = "BuildCompositeMesh: the component is an unloaded handle (a load is a write)";
pub const INIT_GESTALT_DATA: &str = "InitGestaltMeshDataFromOther: the gestalt mesh data of the component";
pub const RECEIVER_NOT_LOADED: &str =
    "InitGestaltMeshDataFromOther: the component is an unloaded handle (a load is a write)";
pub const ARGUMENT_NOT_LOADED: &str =
    "InitGestaltMeshDataFromOther: ComponentToCopyFrom is an unloaded handle (a load is a write)";
pub const NOT_DECODED: &str =
    "InitGestaltMeshDataFromOther: a component whose properties the store has not decoded (a decode is a write)";

/// The component class both natives require.
pub const COMPONENT_CLASS: &str = "Engine.SkeletalMeshComponent";

const BUILD: &str = "WillowWeapon.BuildCompositeMesh";
const INIT: &str = "SkeletalMeshComponent.InitGestaltMeshDataFromOther";

/// The path `BuildCompositeMesh` takes for definition data whose type is set (the note's section 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositePlan {
    /// The gestalt path: the gestalt definition, its shared mesh, and the part names it would apply, in walk order.
    Gestalt { definition: ObjRef, mesh: ObjRef, names: Vec<NameId> },
    /// The non-gestalt path: the parts' non-gestalt meshes, in walk order.
    Meshes(Vec<ObjRef>),
}

/// The declared kind and dimension of `o`'s property `name`, else a data error naming the field.
fn declared(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(ow_object::ids::PropId, PropKind, i32), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    let d = vm.space.classes.decl(p).ok_or_else(|| bad(what, format!("{name} has no declaration")))?;
    Ok((p, d.kind.clone(), d.array_dim))
}

/// `o`'s scalar property `name`.
fn scalar(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    let (p, _, _) = declared(vm, o, name, what)?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn object_field(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    object_of(&scalar(vm, o, name, what)?, &format!("{what}: {name}"))
}

fn bool_field(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<bool, VmError> {
    match scalar(vm, o, name, what)? {
        Value::Bool(b) => Ok(b),
        other => Err(bad(what, format!("{name} holds {}, not a bool", other.kind_name()))),
    }
}

fn name_value(v: &Value, name: &str, what: &str) -> Result<NameId, VmError> {
    match v {
        Value::Name(n) => Ok(*n),
        other => Err(bad(what, format!("{name} holds {}, not a name", other.kind_name()))),
    }
}

/// Every element of `o`'s static name array `name` (declared as names, else a data error).
fn names_field(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Vec<NameId>, VmError> {
    let (p, kind, dim) = declared(vm, o, name, what)?;
    if kind != PropKind::Name {
        return Err(bad(what, format!("{name} is declared as {kind:?}, not names")));
    }
    let dim = u32::try_from(dim).map_err(|_| bad(what, format!("{name} has a negative dimension")))?;
    (0..dim).map(|i| name_value(vm.space.read(o, PropAddr::element(p, i))?, name, what)).collect()
}

/// `o` must be resident, else `NotYet { NotRead }` with `text` and `record`.
fn resident(vm: &Vm, o: ObjRef, text: &'static str, record: &'static str) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        return Err(NotYet::not_read(text, record).into());
    }
    Ok(())
}

/// `o`'s class is a `SkeletalMeshComponent`, else a data error.
fn is_component(vm: &Vm, o: ObjRef, role: &str, what: &str) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let ok = vm.space.classes.by_path(COMPONENT_CLASS).is_some_and(|c| vm.space.classes.is_child_of(class, c));
    if ok {
        Ok(())
    } else {
        Err(bad(what, format!("{role} is a {}, not a {COMPONENT_CLASS}", ow_attr::eval::class_path(&vm.space, o))))
    }
}

/// `o`'s class declares `GestaltData` as a struct, else a data error.
fn has_gestalt_data(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    match declared(vm, o, "GestaltData", what)? {
        (_, PropKind::Struct(_), 1) => Ok(()),
        (_, kind, dim) => Err(bad(what, format!("GestaltData is declared as {kind:?} (dimension {dim}), not a struct"))),
    }
}

/// Whether `v` is a complete value of `kind`, nested structs and arrays included (the same variant, a struct value of
/// the declared struct with every entry of its layout).
fn complete(vm: &Vm, v: &Value, kind: &PropKind, dim: i32) -> bool {
    match (kind, v) {
        (PropKind::Struct(ty), Value::Struct(sv)) => {
            let Some(layout) = vm.space.classes.struct_layout(*ty) else { return false };
            sv.ty == *ty
                && u32::try_from(sv.fields.len()).ok() == Some(layout.slot_count())
                && layout.fields().all(|(at, d)| {
                    (0..d.array_dim.max(1)).all(|i| {
                        let i = at as usize + usize::try_from(i).unwrap_or(0);
                        sv.fields.get(i).is_some_and(|f| complete(vm, f, &d.kind, 1))
                    })
                })
        }
        (PropKind::Array(inner), Value::Array(items)) => items.iter().all(|x| complete(vm, x, inner, 1)),
        (PropKind::Int, Value::Int(_))
        | (PropKind::Float, Value::Float(_))
        | (PropKind::Bool, Value::Bool(_))
        | (PropKind::Byte { .. }, Value::Byte(_))
        | (PropKind::Str, Value::Str(_))
        | (PropKind::Name, Value::Name(_))
        | (PropKind::Object { .. }, Value::Object(_))
        | (PropKind::Interface { .. }, Value::Interface(_) | Value::Object(_))
        | (PropKind::Class { .. }, Value::Class(_)) => dim == 1,
        _ => false,
    }
}

/// Check 4 of the copy: `o`'s `GestaltData` slot is held whole, read as it stands (module documentation).
pub fn held_whole(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let filled = vm.space.classes.get_layout(x.class).map(|l| l.slot_count());
    if u32::try_from(x.slots.len()).ok() != filled {
        return Err(NotYet::not_read(NOT_DECODED, INIT_RECORD).into());
    }
    let (p, kind, dim) = declared(vm, o, "GestaltData", INIT)?;
    if complete(vm, vm.space.read(o, PropAddr::scalar(p))?, &kind, dim) {
        Ok(())
    } else {
        Err(NotYet::layer_absent(INIT_GESTALT_DATA, GESTALT_LAYER, INIT_RECORD).into())
    }
}

/// The plan of `BuildCompositeMesh` for definition data `data`: steps 1 to 4 of the module documentation. `None` when
/// `D.WeaponTypeDefinition` is None. It reads only resident objects and writes nothing.
pub fn composite_plan(vm: &Vm, data: &Value) -> Result<Option<CompositePlan>, VmError> {
    let Some(ty) = object_of(&field(vm, data, "WeaponTypeDefinition")?, "WeaponTypeDefinition")? else {
        return Ok(None);
    };
    resident(vm, ty, NOT_LOADED, BUILD_RECORD)?;
    let gestalt = match object_field(vm, ty, "GestaltMesh", BUILD)? {
        Some(g) => {
            resident(vm, g, NOT_LOADED, BUILD_RECORD)?;
            object_field(vm, g, "GestaltSkeletalMesh", BUILD)?.map(|mesh| (g, mesh))
        }
        None => None,
    };
    let mut names = Vec::new();
    let mut meshes = Vec::new();
    for slot in &WEAPON_ALL_PARTS[..9] {
        let Some(part) = object_of(&field(vm, data, slot)?, slot)? else {
            continue;
        };
        resident(vm, part, NOT_LOADED, BUILD_RECORD)?;
        let gestalt_mode = bool_field(vm, part, "bIsGestaltMode", BUILD)?;
        let mesh = object_field(vm, part, "NongestaltSkeletalMesh", BUILD)?;
        if gestalt.is_some() {
            let own = name_value(&scalar(vm, part, "GestaltModeSkeletalMeshName", BUILD)?, "GestaltModeSkeletalMeshName", BUILD)?;
            let extra = names_field(vm, part, "AdditionalGestaltModeSkeletalMeshNames", BUILD)?;
            let name = if gestalt_mode {
                own
            } else {
                match mesh {
                    // The handle's own name: held by the heap for an unloaded handle too (no load).
                    Some(m) => vm.space.heap.get(m)?.name,
                    None => NameId::NONE,
                }
            };
            if name != NameId::NONE {
                names.push(name);
                names.extend(extra.into_iter().filter(|n| *n != NameId::NONE));
            }
        } else if let (false, Some(m)) = (gestalt_mode, mesh) {
            meshes.push(m);
        }
    }
    Ok(Some(match gestalt {
        Some((definition, mesh)) => CompositePlan::Gestalt { definition, mesh, names },
        None => CompositePlan::Meshes(meshes),
    }))
}

/// `BuildCompositeMesh(component, data)` in Off (module documentation): the answered mesh, or a refusal.
pub fn build_composite_mesh(vm: &Vm, component: Option<ObjRef>, data: &Value) -> Result<Option<ObjRef>, VmError> {
    match composite_plan(vm, data)? {
        None => Ok(None),
        Some(CompositePlan::Gestalt { .. }) => {
            let c = component.ok_or_else(|| bad(BUILD, "BuildCompositeMesh: the component is None"))?;
            is_component(vm, c, "the component", BUILD)?;
            has_gestalt_data(vm, c, BUILD)?;
            resident(vm, c, COMPONENT_NOT_LOADED, BUILD_RECORD)?;
            Err(NotYet::layer_absent(GESTALT_DATA, GESTALT_LAYER, BUILD_RECORD).into())
        }
        Some(CompositePlan::Meshes(m)) => match m.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            _ => Err(NotYet::layer_absent(MERGE, MERGE_LAYER, BUILD_RECORD).into()),
        },
    }
}

/// `InitGestaltMeshDataFromOther(from)` on `this` in Off (module documentation): success only for a self-copy.
pub fn init_gestalt_mesh_data(vm: &Vm, this: ObjRef, from: Option<ObjRef>) -> Result<(), VmError> {
    resident(vm, this, RECEIVER_NOT_LOADED, INIT_RECORD)?;
    has_gestalt_data(vm, this, INIT)?;
    let from = from.ok_or_else(|| bad(INIT, "InitGestaltMeshDataFromOther: ComponentToCopyFrom is None"))?;
    is_component(vm, from, "ComponentToCopyFrom", INIT)?;
    has_gestalt_data(vm, from, INIT)?;
    resident(vm, from, ARGUMENT_NOT_LOADED, INIT_RECORD)?;
    if from == this {
        return Ok(());
    }
    held_whole(vm, this)?;
    held_whole(vm, from)?;
    Err(NotYet::layer_absent(INIT_GESTALT_DATA, GESTALT_LAYER, INIT_RECORD).into())
}

fn build_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = object_of(c.arg(0)?, BUILD)?;
    let data = c.arg(1)?.clone();
    Ok(Value::Object(build_composite_mesh(c.vm, component, &data)?))
}

fn init_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let this = c.this.ok_or_else(|| bad(INIT, "called without a component"))?;
    let from = object_of(c.arg(0)?, INIT)?;
    init_gestalt_mesh_data(c.vm, this, from)?;
    Ok(Value::None)
}

/// Binds both natives in `ParityMode::Off`; Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.register_native(BUILD_COMPOSITE_MESH, Rc::new(build_native))?;
    vm.register_native(INIT_GESTALT_MESH_DATA, Rc::new(init_native))
}

#[cfg(test)]
#[path = "gestalt_tests.rs"]
mod tests;
