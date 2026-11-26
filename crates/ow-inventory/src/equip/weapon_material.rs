//! Owner: lane IMPL-BUILDWEAPONMATERIAL. `WillowWeapon.BuildWeaponMaterial(Weapon, Mesh, WeaponData)` in
//! `ParityMode::Off`, link 8's next stop after #678. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_WEAPON_VISUALS.md section 1.1 alone (#698 at its review r1
//! head 5797b5a, lane ANALYST-BUILDWEAPONMATERIAL, decision 2026-11-25-32); this lane did not read
//! the executable. **Every rule is UNVERIFIED in the running game.** Reference binds nothing (the stub policy).
//!
//! **The preflight**, write-free and loaded-only (by name lookup on resident objects; a field of the wrong kind is a
//! `VmError::Script` naming it; a non-resident object it must read is `NotYet { NotRead }`, [`NOT_LOADED`]). In order:
//! 1. `WeaponData.WeaponTypeDefinition` None: the answer is None. Nothing more is read;
//! 2. (the note's 1a) no weapon passed: `NotYet { NotRead }` ([`NO_WEAPON`]): the default outer is not read;
//!    `InitMeshes` always passes `Self`;
//! 3. **the parent:** the Material part's `Material`, when both are set (each resident). Otherwise, with a mesh
//!    component passed, `NotYet { NotRead }` ([`MESH_MATERIAL`]); with neither, None;
//! 4. **the class:** the type's `MaterialClass`. None is `NotYet { NotRead }` ([`NO_CLASS`]). The class and
//!    `Engine.MaterialInstanceConstant` must each be ready with no load (a defined layout and loaded code, as
//!    `globals::ready_class` asks) and the class's default object already resident (found by a load-free scan of the
//!    heap for its default origin), since the construction copies it; otherwise `NotYet { NotRead }` naming the class
//!    ([`class_not_ready`]). A class outside the
//!    `MaterialInstanceConstant` family is `NotYet { NotRead }` ([`outside_family`]). The instance's `Parent` must be
//!    declared as an object reference that can hold the parent (else `VmError::Script`);
//! 5. **the parts' parameters:** for each part slot in `WeaponDefinitionData`'s order without the two name parts
//!    ([`crate::equip::WEAPON_PART_FIELDS`]), skipping empty slots: the part's `MaterialVectorParameterValues` must be an
//!    array of a struct with `ParameterName` and `ParameterValue`, and any entry at all is `NotYet { NotRead }`
//!    ([`VECTOR_SETTER`]): the setter's rule was not read, so no `VectorParameterValues` is guessed.
//!
//! **The commit:** one object of `MaterialClass` through the VM's own object creation (as a script `new` does: the
//! class default's values, the weapon as outer, the VM's generated name); its `Parent` = the parent of step 3
//! (**UNVERIFIED by name**: the note assumes the parent setter's only script-visible effect is the `Parent` field; its
//! render work is not ported); the answer is the instance. The preflight settles every
//! check, so no refusal is expected after the creation; one would be marked "stopped after partial writes" by #518's
//! counters and the class-code loads (#685).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::bad;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_WEAPON_VISUALS.md";
/// The native's key.
pub const KEY: &str = "WillowWeapon.BuildWeaponMaterial(object,object,weapondefinitiondata)";
/// The material family the note saw.
pub const MATERIAL_INSTANCE_CONSTANT: &str = "Engine.MaterialInstanceConstant";
const WHAT: &str = "WillowWeapon.BuildWeaponMaterial";

/// A read on an object the store has not loaded.
pub const NOT_LOADED: &str = "BuildWeaponMaterial: an object the store has not loaded (a load is a write)";
/// No weapon passed (the default outer).
pub const NO_WEAPON: &str = "BuildWeaponMaterial: the default outer (used when no weapon is passed) is not read";
/// The mesh component's material 0.
pub const MESH_MATERIAL: &str = "BuildWeaponMaterial: the mesh component's material 0 (its material lookup is not read)";
/// No `MaterialClass`.
pub const NO_CLASS: &str = "BuildWeaponMaterial: a weapon type with no MaterialClass (the game constructs it unchecked)";
/// Any part vector parameter.
pub const VECTOR_SETTER: &str = "BuildWeaponMaterial: the material instance's vector-parameter setter (not read)";

/// The refusal of a class outside the `MaterialInstanceConstant` family.
pub fn outside_family(class: &str) -> String {
    format!("BuildWeaponMaterial: a MaterialClass outside the MaterialInstanceConstant family ({class}; only that family was seen)")
}

/// The refusal of a class that is not ready with no load.
pub fn class_not_ready(class: &str) -> String {
    format!("BuildWeaponMaterial: the class {class}, not loaded (a class load is a write)")
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    Ok(())
}

fn class_path(vm: &Vm, c: ClassId) -> String {
    vm.space.classes.get_layout(c).map(|l| l.path().to_string()).unwrap_or_else(|| format!("{c:?}"))
}

/// Struct field `name` of `data` as an object reference; a missing or non-object field is a data error.
fn data_object(vm: &Vm, data: &Value, name: &str) -> Result<Option<ObjRef>, VmError> {
    match super::field(vm, data, name)? {
        Value::Object(o) => Ok(o),
        other => Err(bad(WHAT, format!("WeaponData.{name} holds {}, not an object", other.kind_name()))),
    }
}

/// Field `name` of the resident `o`, by name, with its declared kind.
fn object_field(vm: &Vm, o: ObjRef, name: &str) -> Result<(PropKind, Value), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(WHAT, format!("{} declares no {name}", class_path(vm, class))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(WHAT, format!("{name} has no declaration")))?;
    Ok((kind, vm.space.read(o, PropAddr::scalar(p))?.clone()))
}

fn object_of(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match object_field(vm, o, name)? {
        (PropKind::Object { .. }, Value::Object(x)) => Ok(x),
        (k, _) => Err(bad(WHAT, format!("{name} is declared as {}, not an object", k.kind_name()))),
    }
}

/// Whether `c` is ready with no load: a defined layout, loaded code, and (for a class the commit constructs) its
/// default object already built in the heap. The store exposes no load-free query of its class-default cache (the
/// decoder's `class_defaults`, which the commit's `class_default` reads), so the heap is scanned for an object of
/// `Origin::Default` of exactly `c`. The two agree: outside tests, only `ow_object::cdo`'s default builder allocates
/// `Origin::Default`, and it records each object it builds in that cache (review r1 of #702).
fn ready(vm: &Vm, c: ClassId, needs_default: bool) -> Result<(), VmError> {
    let defined = vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined());
    let built = !needs_default || default_built(vm, c);
    if defined && vm.class_code_loaded(c) && built {
        return Ok(());
    }
    Err(NotYet::not_read(class_not_ready(&class_path(vm, c)), RECORD).into())
}

/// Whether the default object of exactly `c` is already built in the heap (an object of `Origin::Default`), by a
/// load-free scan ([`ready`]). `super::material_instances` asks it too.
pub(super) fn default_built(vm: &Vm, c: ClassId) -> bool {
    vm.space
        .heap
        .handles()
        .into_iter()
        .any(|o| vm.space.heap.get(o).is_ok_and(|x| x.class == c && matches!(x.origin, Origin::Default { .. })))
}

/// What the commit does, every check passed.
struct Plan {
    class: ClassId,
    weapon: ObjRef,
    parent: Option<ObjRef>,
    parent_prop: PropId,
}

/// The preflight (module documentation): `Ok(None)` for the answer None. Reads only.
fn preflight(vm: &Vm, weapon: Option<ObjRef>, mesh: Option<ObjRef>, data: &Value) -> Result<Option<Plan>, VmError> {
    // 1.
    let Some(weapon_type) = data_object(vm, data, "WeaponTypeDefinition")? else {
        return Ok(None);
    };
    // 2.
    let Some(weapon) = weapon else {
        return Err(NotYet::not_read(NO_WEAPON, RECORD).into());
    };
    resident(vm, weapon_type)?;
    // 3. The parent.
    let material = match data_object(vm, data, "MaterialPartDefinition")? {
        Some(part) => {
            resident(vm, part)?;
            object_of(vm, part, "Material")?
        }
        None => None,
    };
    let parent = match (material, mesh) {
        (Some(m), _) => {
            resident(vm, m)?;
            Some(m)
        }
        (None, Some(_)) => return Err(NotYet::not_read(MESH_MATERIAL, RECORD).into()),
        (None, None) => None,
    };
    // 4. The class.
    let class = match object_field(vm, weapon_type, "MaterialClass")? {
        (PropKind::Class { .. }, Value::Class(c)) => c,
        (k, _) => return Err(bad(WHAT, format!("MaterialClass is declared as {}, not a class", k.kind_name()))),
    };
    let Some(class) = class else {
        return Err(NotYet::not_read(NO_CLASS, RECORD).into());
    };
    let family = crate::globals::ready_class(vm, MATERIAL_INSTANCE_CONSTANT)
        .map_err(|_| VmError::from(NotYet::not_read(class_not_ready(MATERIAL_INSTANCE_CONSTANT), RECORD)))?;
    // The class's readiness first (a declared-only class has no super chain to test; review r1 of #702), then the
    // family, then its default object.
    ready(vm, class, false)?;
    if !vm.space.classes.is_child_of(class, family) {
        return Err(NotYet::not_read(outside_family(&class_path(vm, class)), RECORD).into());
    }
    ready(vm, class, true)?;
    let parent_prop = vm
        .space
        .prop(class, "Parent")
        .ok_or_else(|| bad(WHAT, format!("{} declares no Parent", class_path(vm, class))))?;
    let kind = vm.space.classes.decl(parent_prop).map(|d| d.kind.clone()).ok_or_else(|| bad(WHAT, "Parent has no declaration"))?;
    if !matches!(kind, PropKind::Object { .. }) {
        return Err(bad(WHAT, format!("Parent is declared as {}, not an object", kind.kind_name())));
    }
    vm.space
        .heap
        .check_value(&vm.space.classes, &kind, &Value::Object(parent))
        .map_err(|e| bad(WHAT, format!("Parent cannot hold the parent material: {e}")))?;
    // 5. The parts' vector parameters.
    for slot in crate::equip::WEAPON_PART_FIELDS {
        let Some(part) = data_object(vm, data, slot)? else { continue };
        resident(vm, part)?;
        let (kind, value) = object_field(vm, part, "MaterialVectorParameterValues")?;
        let fits = match &kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(ty) => vm.space.classes.struct_layout(ty).is_some_and(|l| {
                    ["ParameterName", "ParameterValue"].iter().all(|n| vm.space.names.find(n).is_some_and(|id| l.field(id).is_some()))
                }),
                _ => false,
            },
            _ => false,
        };
        if !fits {
            return Err(bad(WHAT, "MaterialVectorParameterValues is not declared as an array of VectorParameterValue"));
        }
        if value.elements().is_some_and(|e| !e.is_empty()) {
            return Err(NotYet::not_read(VECTOR_SETTER, RECORD).into());
        }
    }
    Ok(Some(Plan { class, weapon, parent, parent_prop }))
}

/// #518's counters and the class-code loads (#685).
fn marks(vm: &Vm) -> (u64, u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads())
}

fn native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let object = |v: &Value, name: &str| -> Result<Option<ObjRef>, VmError> {
        match v {
            Value::Object(o) => Ok(*o),
            Value::None => Ok(None),
            other => Err(bad(WHAT, format!("{name} is {}, not an object", other.kind_name()))),
        }
    };
    let weapon = object(c.arg(0)?, "Weapon")?;
    let mesh = object(c.arg(1)?, "Mesh")?;
    let data = c.arg(2)?.clone();
    let Some(plan) = preflight(c.vm, weapon, mesh, &data)? else {
        return Ok(Value::Object(None));
    };
    let entry = marks(c.vm);
    match commit(c.vm, &plan) {
        Err(e) if marks(c.vm) != entry => Err(ow_world::pools::after_partial_writes(e)),
        other => other,
    }
}

fn commit(vm: &mut Vm, plan: &Plan) -> Result<Value, VmError> {
    let instance = ow_vm::interp::instantiate_from_defaults(vm, plan.class)?;
    vm.space.heap.get_mut(instance)?.outer = Some(plan.weapon);
    vm.space.heap.note_write();
    vm.space.write(instance, PropAddr::scalar(plan.parent_prop), Value::Object(plan.parent))?;
    Ok(Value::Object(Some(instance)))
}

/// Binds the rule in Off, keep-existing; Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off || vm.has_native(KEY) {
        return Ok(());
    }
    vm.register_native(KEY, Rc::new(native))
}

#[cfg(test)]
#[path = "weapon_material_tests.rs"]
mod tests;
