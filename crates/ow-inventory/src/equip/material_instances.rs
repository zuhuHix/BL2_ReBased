//! Owner: lane IMPL-INITIALIZE-BEHAVIOR-PROVIDERS. The material natives the player part of
//! `ApplyBodyClassStartingValues` reaches, in `ParityMode::Off`: `MeshComponent.GetMaterial`,
//! `MaterialInstance.SetParent`, `MaterialInstance.SetVectorParameterValue` and
//! `MeshComponent.GetInstancedMaterialInstanceConstant`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 3 alone (#740
//! with its review r1, lane ANALYST-INITIALIZE-BEHAVIOR-PROVIDERS, decision 2026-11-25-75); this lane did not read the
//! executable. **Every rule is UNVERIFIED in the running game.** Reference binds nothing (the stub policy answers).
//! "First native class" is `material_natives::first_native` (the fitted native-bit walk); the class tests below are
//! by name up the loaded class chain, so none loads class code. Every preflight reads only: no write, no store load,
//! no class-code load, nothing interned. An object a rule must read that the store has not loaded is a `NotRead`
//! ([`not_loaded`]); a field missing or of the wrong kind is a `VmError::Script` naming it.
//!
//! **`GetMaterial(ElementIndex)`** ([`get_material`]), for a receiver whose first native class is
//! `Engine.SkeletalMeshComponent` or `GearboxFramework.GearboxSkeletalMeshComponent` (any other: [`get_material_for`],
//! `NotRead`). With `Materials` (an array of objects) and `SkeletalMesh` (an object) declared: the component's own
//! entry at the index when it is not None; else, with a mesh, the mesh's `Materials` entry there when not None; else
//! None. A negative index is None after the declarations, with no further read. The mesh is read (so must be resident,
//! its `Materials` declared) only when the own list gives nothing. Nothing is written.
//!
//! **`SetParent(NewParent)`** ([`set_parent`]), for a receiver whose first native class is
//! `Engine.MaterialInstanceConstant` (any other: [`set_parent_for`]): `Parent` (declared an object that can hold the
//! argument) is written only when it differs. The material update after it has no counterpart here (no render state).
//!
//! **`SetVectorParameterValue(ParameterName, Value)`** ([`set_vector_parameter_value`]), for the same receivers (any
//! other: [`set_vector_for`]): `VectorParameterValues` must be an array of a struct with a name `ParameterName`, a
//! struct `ParameterValue` with float `R`, `G`, `B` and `A`, and a struct `ExpressionGUID`. The first entry whose
//! `ParameterName` equals the argument (as names compare, number included) gets the four floats when any differs (an
//! exact f32 compare, so a NaN always writes); with no such entry, one is appended (the struct's zero value: a zero
//! `ExpressionGUID`) with the name and the four floats. One write of the array, or none.
//!
//! **`GetInstancedMaterialInstanceConstant(ElementIndex)`** ([`get_instanced`]):
//! - **preflight:** [`get_material`]'s rule and refusals; a material that is a `MaterialInstanceConstant` (or a
//!   subclass) whose `Outer` is the component is the answer, with nothing written. Otherwise the creation branch:
//!   `Engine.MaterialInstanceConstant` ready with no load (`globals::ready_class`, and its default object built),
//!   else [`instance_class_not_ready`] (`NotRead`); `SetParent`'s declarations on that class; then `SetMaterial`'s
//!   preflight for the index (its reattach refusals, `REATTACH_OWNER` and `REATTACH_SCENE`), before anything is made;
//! - **commit:** one object of that class as the VM's script `new` makes it (the class default's values, the
//!   component as `Outer`, the VM's generated name: the object and the name it interns count as writes; **its flags
//!   were not decoded**); `SetParent` with the material asked again; `SetMaterial`'s commit; the answer is the new
//!   instance. A negative index still makes it, and `SetMaterial` does nothing. A refusal after the object is made is
//!   marked "stopped after partial writes" by #518's counters and the class-code loads (none is expected).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;
use ow_world::world::component_setters::{self, Slots};

use super::bad;
use super::material_natives::{MaterialPlan, Setting, class_path, commit_set_material, first_native, plan_set_material};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md (section 3)";
/// The natives' keys.
pub const GET_MATERIAL: &str = "MeshComponent.GetMaterial(int)";
pub const SET_PARENT: &str = "MaterialInstance.SetParent(object)";
pub const SET_VECTOR_PARAMETER_VALUE: &str = "MaterialInstance.SetVectorParameterValue(name,linearcolor)";
pub const GET_INSTANCED: &str = "MeshComponent.GetInstancedMaterialInstanceConstant(int)";

const GET_WHAT: &str = "MeshComponent.GetMaterial";
const PARENT_WHAT: &str = "MaterialInstance.SetParent";
const VECTOR_WHAT: &str = "MaterialInstance.SetVectorParameterValue";
const INSTANCED_WHAT: &str = "MeshComponent.GetInstancedMaterialInstanceConstant";

/// The two component classes whose `GetMaterial` the note read.
pub const SKELETAL_COMPONENTS: [&str; 2] =
    ["Engine.SkeletalMeshComponent", "GearboxFramework.GearboxSkeletalMeshComponent"];
/// The material instance class whose setters the note read.
pub const MATERIAL_INSTANCE_CONSTANT: &str = "Engine.MaterialInstanceConstant";

/// A read on an object the store has not loaded, by native.
pub fn not_loaded(what: &str) -> String {
    format!("{what}: an object the store has not loaded (a load is a write)")
}

/// `GetMaterial` on a receiver of another first native class.
pub fn get_material_for(class: &str) -> String {
    format!("MeshComponent.GetMaterial for {class}")
}

/// `SetParent` on a receiver of another first native class.
pub fn set_parent_for(class: &str) -> String {
    format!("MaterialInstance.SetParent for {class}")
}

/// `SetVectorParameterValue` on a receiver of another first native class.
pub fn set_vector_for(class: &str) -> String {
    format!("MaterialInstance.SetVectorParameterValue for {class}")
}

/// The instance class not ready with no load.
pub fn instance_class_not_ready() -> String {
    format!("GetInstancedMaterialInstanceConstant: the class {MATERIAL_INSTANCE_CONSTANT}, not loaded (a class load is a write)")
}

/// `o` resident: not a store handle still unloaded or failed, and its slots built.
fn resident(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(not_loaded(what), RECORD).into());
    }
    Ok(())
}

/// Whether `class` or a loaded super class has the path `path` (case-insensitive). Loads nothing.
fn is_named(vm: &Vm, class: ClassId, path: &str) -> bool {
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        let Some(layout) = vm.space.classes.get_layout(c) else {
            return false;
        };
        if layout.path().as_str().eq_ignore_ascii_case(path) {
            return true;
        }
        cursor = layout.super_class();
    }
    false
}

/// The resident `o`'s first native class, else its own class path (for a refusal's text).
fn native_of(vm: &Vm, o: ObjRef) -> Result<(Option<String>, String), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let native = first_native(vm, class);
    let named = native.clone().unwrap_or_else(|| class_path(vm, class));
    Ok((native, named))
}

fn is_one_of(native: Option<&str>, paths: &[&str]) -> bool {
    native.is_some_and(|n| paths.iter().any(|p| n.eq_ignore_ascii_case(p)))
}

/// The resident `o`'s array of objects `name` (declared so, else a data error).
fn objects(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Vec<Option<ObjRef>>, VmError> {
    match component_setters::declared(vm, o, name, what)? {
        PropKind::Array(inner) if matches!(*inner, PropKind::Object { .. }) => {}
        other => return Err(bad(what, format!("{name} is declared as {}, not an array of objects", other.kind_name()))),
    }
    let v = Slots::of(o).required(vm, name, what)?;
    let items = v.elements().ok_or_else(|| bad(what, format!("{name} holds {}, not an array", v.kind_name())))?;
    items
        .iter()
        .map(|i| match i {
            Value::Object(x) => Ok(*x),
            other => Err(bad(what, format!("a {name} entry holds {}, not an object", other.kind_name()))),
        })
        .collect()
}

/// The resident `o`'s object `name` (declared so, else a data error).
fn object(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match component_setters::declared(vm, o, name, what)? {
        PropKind::Object { .. } => {}
        other => return Err(bad(what, format!("{name} is declared as {}, not an object", other.kind_name()))),
    }
    match Slots::of(o).required(vm, name, what)? {
        Value::Object(x) => Ok(x),
        other => Err(bad(what, format!("{name} holds {}, not an object", other.kind_name()))),
    }
}

// ---- GetMaterial ---------------------------------------------------------------------------------------------------

/// `GetMaterial`'s rule (module documentation), read only.
pub fn get_material(vm: &Vm, component: ObjRef, index: i32) -> Result<Option<ObjRef>, VmError> {
    resident(vm, component, GET_WHAT)?;
    let (native, named) = native_of(vm, component)?;
    if !is_one_of(native.as_deref(), &SKELETAL_COMPONENTS) {
        return Err(NotYet::not_read(get_material_for(&named), RECORD).into());
    }
    let own = objects(vm, component, "Materials", GET_WHAT)?;
    let mesh = object(vm, component, "SkeletalMesh", GET_WHAT)?;
    let Ok(i) = usize::try_from(index) else {
        return Ok(None);
    };
    if let Some(Some(m)) = own.get(i) {
        return Ok(Some(*m));
    }
    let Some(mesh) = mesh else {
        return Ok(None);
    };
    resident(vm, mesh, GET_WHAT)?;
    Ok(objects(vm, mesh, "Materials", GET_WHAT)?.get(i).copied().flatten())
}

fn get_material_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(GET_WHAT, "called without a component"))?;
    let index = int_arg(c, GET_WHAT)?;
    Ok(Value::Object(get_material(c.vm, component, index)?))
}

fn int_arg(c: &NativeCall<'_>, what: &str) -> Result<i32, VmError> {
    match c.arg(0)? {
        Value::Int(i) => Ok(*i),
        other => Err(bad(what, format!("ElementIndex holds {}, not an int", other.kind_name()))),
    }
}

// ---- SetParent -----------------------------------------------------------------------------------------------------

/// `Parent` of `class`, declared an object that can hold `parent`; else a data error.
fn parent_prop(vm: &Vm, class: ClassId, parent: Option<ObjRef>) -> Result<PropId, VmError> {
    let p = vm
        .space
        .prop(class, "Parent")
        .ok_or_else(|| bad(PARENT_WHAT, format!("{} declares no Parent", class_path(vm, class))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(PARENT_WHAT, "Parent has no declaration"))?;
    if !matches!(kind, PropKind::Object { .. }) {
        return Err(bad(PARENT_WHAT, format!("Parent is declared as {}, not an object", kind.kind_name())));
    }
    vm.space
        .heap
        .check_value(&vm.space.classes, &kind, &Value::Object(parent))
        .map_err(|e| bad(PARENT_WHAT, format!("Parent cannot hold the new parent: {e}")))?;
    Ok(p)
}

/// `SetParent`'s preflight: the prop to write, or `None` when `Parent` already holds `parent`.
fn plan_set_parent(vm: &Vm, instance: ObjRef, parent: Option<ObjRef>) -> Result<Option<PropId>, VmError> {
    resident(vm, instance, PARENT_WHAT)?;
    let (native, named) = native_of(vm, instance)?;
    if !is_one_of(native.as_deref(), &[MATERIAL_INSTANCE_CONSTANT]) {
        return Err(NotYet::not_read(set_parent_for(&named), RECORD).into());
    }
    let class = vm.space.heap.get(instance)?.class;
    let p = parent_prop(vm, class, parent)?;
    Ok((*vm.space.read(instance, PropAddr::scalar(p))? != Value::Object(parent)).then_some(p))
}

/// `SetParent`'s rule (module documentation).
pub fn set_parent(vm: &mut Vm, instance: ObjRef, parent: Option<ObjRef>) -> Result<(), VmError> {
    if let Some(p) = plan_set_parent(vm, instance, parent)? {
        vm.space.write(instance, PropAddr::scalar(p), Value::Object(parent))?;
    }
    Ok(())
}

fn set_parent_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let instance = c.this.ok_or_else(|| bad(PARENT_WHAT, "called without a material instance"))?;
    let parent = match c.arg(0)? {
        Value::Object(o) | Value::Interface(o) => *o,
        Value::None => None,
        other => return Err(bad(PARENT_WHAT, format!("NewParent holds {}, not an object", other.kind_name()))),
    };
    set_parent(c.vm, instance, parent)?;
    Ok(Value::None)
}

// ---- SetVectorParameterValue ---------------------------------------------------------------------------------------

const COLOR: [&str; 4] = ["R", "G", "B", "A"];

/// The entry index of field `name` of struct `ty`, with its declared kind.
fn struct_field(vm: &Vm, ty: StructId, name: &str) -> Option<(usize, PropKind)> {
    let id = vm.space.names.find(name)?;
    let (entry, d) = vm.space.classes.struct_layout(ty)?.field(id)?;
    Some((entry as usize, d.kind.clone()))
}

/// The entry struct's layout: (struct, `ParameterName`'s entry, `ParameterValue`'s entry, its colour struct and the
/// four colour entries).
struct VectorLayout {
    entry: StructId,
    name: usize,
    value: usize,
    color: [usize; 4],
}

fn vector_layout(vm: &Vm, instance: ObjRef) -> Result<VectorLayout, VmError> {
    let kind = component_setters::declared(vm, instance, "VectorParameterValues", VECTOR_WHAT)?;
    let entry = match kind {
        PropKind::Array(inner) => match *inner {
            PropKind::Struct(ty) => ty,
            other => return Err(bad(VECTOR_WHAT, format!("VectorParameterValues holds {} entries, not structs", other.kind_name()))),
        },
        other => return Err(bad(VECTOR_WHAT, format!("VectorParameterValues is declared as {}, not an array", other.kind_name()))),
    };
    let field = |name: &str, fits: &dyn Fn(&PropKind) -> bool| -> Result<(usize, PropKind), VmError> {
        match struct_field(vm, entry, name) {
            Some((i, k)) if fits(&k) => Ok((i, k)),
            Some((_, k)) => Err(bad(VECTOR_WHAT, format!("VectorParameterValue.{name} is declared as {}", k.kind_name()))),
            None => Err(bad(VECTOR_WHAT, format!("VectorParameterValue declares no {name}"))),
        }
    };
    let (name, _) = field("ParameterName", &|k| matches!(k, PropKind::Name))?;
    let (value, color_kind) = field("ParameterValue", &|k| matches!(k, PropKind::Struct(_)))?;
    field("ExpressionGUID", &|k| matches!(k, PropKind::Struct(_)))?;
    let PropKind::Struct(color_ty) = color_kind else { unreachable!("checked as a struct") };
    let mut color = [0; 4];
    for (slot, c) in color.iter_mut().zip(COLOR) {
        *slot = match struct_field(vm, color_ty, c) {
            Some((i, PropKind::Float)) => i,
            _ => return Err(bad(VECTOR_WHAT, format!("VectorParameterValue.ParameterValue has no float {c}"))),
        };
    }
    Ok(VectorLayout { entry, name, value, color })
}

/// The four floats of a `LinearColor` argument.
fn color_arg(vm: &Vm, v: &Value) -> Result<[f32; 4], VmError> {
    let mut out = [0.0; 4];
    for (slot, c) in out.iter_mut().zip(COLOR) {
        *slot = match ow_attr::eval::field(&vm.space, v, c) {
            Ok(Value::Float(f)) => *f,
            _ => return Err(bad(VECTOR_WHAT, format!("Value holds no float {c}"))),
        };
    }
    Ok(out)
}

/// The array to write, or `None` for no write (module documentation). Reads only.
fn plan_set_vector(vm: &Vm, instance: ObjRef, name: &Value, color: [f32; 4]) -> Result<Option<Vec<Value>>, VmError> {
    resident(vm, instance, VECTOR_WHAT)?;
    let (native, named) = native_of(vm, instance)?;
    if !is_one_of(native.as_deref(), &[MATERIAL_INSTANCE_CONSTANT]) {
        return Err(NotYet::not_read(set_vector_for(&named), RECORD).into());
    }
    let l = vector_layout(vm, instance)?;
    let list = Slots::of(instance).required(vm, "VectorParameterValues", VECTOR_WHAT)?;
    let mut entries =
        list.elements().map(<[Value]>::to_vec).ok_or_else(|| bad(VECTOR_WHAT, "VectorParameterValues holds no array"))?;
    let set_color = |entry: &mut Value| -> Result<bool, VmError> {
        let value = entry
            .struct_mut()
            .and_then(|s| s.fields.get_mut(l.value))
            .and_then(Value::struct_mut)
            .ok_or_else(|| bad(VECTOR_WHAT, "a VectorParameterValues entry holds no ParameterValue struct"))?;
        let mut changed = false;
        for (&at, &want) in l.color.iter().zip(&color) {
            let slot = value.fields.get_mut(at).ok_or_else(|| bad(VECTOR_WHAT, "ParameterValue is short"))?;
            // An exact compare: a NaN never equals, so it always writes.
            if *slot != Value::Float(want) || want.is_nan() {
                changed = true;
            }
        }
        if changed {
            for (&at, &want) in l.color.iter().zip(&color) {
                value.fields[at] = Value::Float(want);
            }
        }
        Ok(changed)
    };
    let found = entries.iter().position(|e| e.as_struct().and_then(|s| s.fields.get(l.name)) == Some(name));
    match found {
        Some(i) => Ok(set_color(&mut entries[i])?.then_some(entries)),
        None => {
            let mut fresh = vm
                .space
                .classes
                .struct_layout(l.entry)
                .map(|layout| layout.zero().clone())
                .ok_or_else(|| bad(VECTOR_WHAT, "VectorParameterValue has no layout"))?;
            let slot = fresh.struct_mut().and_then(|s| s.fields.get_mut(l.name)).ok_or_else(|| bad(VECTOR_WHAT, "ParameterName"))?;
            *slot = name.clone();
            set_color(&mut fresh)?;
            entries.push(fresh);
            Ok(Some(entries))
        }
    }
}

/// `SetVectorParameterValue(ParameterName, Value)` in Off (module documentation).
pub fn set_vector_parameter_value(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let instance = c.this.ok_or_else(|| bad(VECTOR_WHAT, "called without a material instance"))?;
    let name = match c.arg(0)? {
        v @ Value::Name(_) => v.clone(),
        other => return Err(bad(VECTOR_WHAT, format!("ParameterName holds {}, not a name", other.kind_name()))),
    };
    let color = color_arg(c.vm, c.arg(1)?)?;
    if let Some(entries) = plan_set_vector(c.vm, instance, &name, color)? {
        component_setters::write(c.vm, instance, "VectorParameterValues", Value::array(entries), VECTOR_WHAT)?;
    }
    Ok(Value::None)
}

// ---- GetInstancedMaterialInstanceConstant --------------------------------------------------------------------------

/// What the creation branch does, every check passed.
struct Creation {
    class: ClassId,
    set: MaterialPlan,
}

/// The preflight (module documentation): `Ok(Err(m))` when `m` is the answer with no write.
fn plan_instanced(vm: &Vm, component: ObjRef, index: i32) -> Result<Result<Creation, ObjRef>, VmError> {
    let material = get_material(vm, component, index)?;
    if let Some(m) = material {
        resident(vm, m, INSTANCED_WHAT)?;
        let x = vm.space.heap.get(m)?;
        if is_named(vm, x.class, MATERIAL_INSTANCE_CONSTANT) && x.outer == Some(component) {
            return Ok(Err(m));
        }
    }
    let class = crate::globals::ready_class(vm, MATERIAL_INSTANCE_CONSTANT)
        .ok()
        .filter(|&c| super::weapon_material::default_built(vm, c))
        .ok_or_else(|| VmError::from(NotYet::not_read(instance_class_not_ready(), RECORD)))?;
    parent_prop(vm, class, material)?;
    let set = plan_set_material(vm, component, index, Setting::NotMadeYet)?;
    Ok(Ok(Creation { class, set }))
}

/// #518's counters and the class-code loads (#685).
fn marks(vm: &Vm) -> (u64, u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads())
}

fn create(vm: &mut Vm, component: ObjRef, index: i32, plan: Creation) -> Result<ObjRef, VmError> {
    let instance = ow_vm::interp::instantiate_from_defaults(vm, plan.class)?;
    vm.space.heap.get_mut(instance)?.outer = Some(component);
    vm.space.heap.note_write();
    // SetParent(GetMaterial(ElementIndex)): the material asked again.
    let parent = get_material(vm, component, index)?;
    set_parent(vm, instance, parent)?;
    if let MaterialPlan::Write { mut entries, index } = plan.set {
        entries[index] = Value::Object(Some(instance));
        commit_set_material(vm, component, entries)?;
    }
    Ok(instance)
}

/// `GetInstancedMaterialInstanceConstant(ElementIndex)` in Off (module documentation).
pub fn get_instanced(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(INSTANCED_WHAT, "called without a component"))?;
    let index = int_arg(c, INSTANCED_WHAT)?;
    let plan = match plan_instanced(c.vm, component, index)? {
        Err(owned) => return Ok(Value::Object(Some(owned))),
        Ok(plan) => plan,
    };
    let before = marks(c.vm);
    match create(c.vm, component, index, plan) {
        Ok(instance) => Ok(Value::Object(Some(instance))),
        Err(e) if marks(c.vm) != before => Err(ow_world::pools::after_partial_writes(e)),
        Err(e) => Err(e),
    }
}

/// Binds the four natives in Off, keep-existing: a key already bound is kept. Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, fn(&mut NativeCall<'_>) -> Result<Value, VmError>); 4] = [
        (GET_MATERIAL, get_material_native),
        (SET_PARENT, set_parent_native),
        (SET_VECTOR_PARAMETER_VALUE, set_vector_parameter_value),
        (GET_INSTANCED, get_instanced),
    ];
    for (key, f) in natives {
        if !vm.has_native(key) {
            vm.register_native(key, Rc::new(f))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "material_instances_tests.rs"]
mod tests;
