//! Owner: lane IMPL-GETFIRINGMODEDEFINITION. `WillowWeapon.GetFiringModeDefinition()` in `ParityMode::Off`, link 8's
//! stop after `OnCreate` (`InitializeInternal`), and the rule `ow-combat`'s shot helper (`hit::shot::firing_mode`)
//! shares. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_WEAPON_GET_FIRING_MODE.md sections 2 and 3 alone (lane
//! ANALYST-GETFIRINGMODEDEFINITION, #731 with its review r1, on main); this lane did not read the executable. **Every
//! rule is UNVERIFIED in the running game**, except the barrel and type steps, which lane L7 observed
//! (PROJECTILE_FIRING_PATTERNS_LIVE.md section 3). Reference keeps the stub policy.
//!
//! **The rule** ([`rule`]), in order, stopping where it stops:
//! 1. the barrel part set, with its `CustomFiringModeDefinition` set: that mode;
//! 2. else the elemental part set, with its `CustomFiringModeDefinition` set: that mode;
//! 3. else the weapon type set: its `DefaultFiringModeDefinition`, None included;
//! 4. else None.
//!
//! A part that is None is skipped. Each object is "reached" only when the rule reads it, through the caller's step:
//! the native's step is load-free ([`resident`]: an unloaded store handle, a failed one, or a slotless stand-in is
//! `NotYet { NotRead }` naming it), while the shot path's step loads it. Each object reached must declare its mode
//! field as an object reference, else `VmError::Script` naming it. The mode is returned as the field holds it (a
//! store handle is not loaded). Nothing is written and nothing is cached.
//!
//! **The native** reads the weapon's `DefinitionData`, which its class must declare as a
//! `WillowGame.WillowWeaponTypes.WeaponDefinitionData` holding `BarrelPartDefinition`, `ElementalPartDefinition` and
//! `WeaponTypeDefinition` as objects (else `VmError::Script`), and nothing else of the weapon.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::bad;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_WEAPON_GET_FIRING_MODE.md";
/// The native key.
pub const KEY: &str = "WillowWeapon.GetFiringModeDefinition()";
const WHAT: &str = "WillowWeapon.GetFiringModeDefinition";
const DEFINITION_DATA: &str = "WillowGame.WillowWeaponTypes.WeaponDefinitionData";

/// An object the rule reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Barrel,
    Elemental,
    WeaponType,
}

impl Part {
    /// The object's name in a refusal.
    pub fn label(self) -> &'static str {
        match self {
            Part::Barrel => "barrel part",
            Part::Elemental => "elemental part",
            Part::WeaponType => "weapon type",
        }
    }

    /// The mode field the rule reads on it.
    pub fn field(self) -> &'static str {
        match self {
            Part::Barrel | Part::Elemental => "CustomFiringModeDefinition",
            Part::WeaponType => "DefaultFiringModeDefinition",
        }
    }
}

/// `o`'s mode field for `part`, declared as an object reference (else `VmError::Script` naming it).
fn mode_of(vm: &Vm, part: Part, o: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let name = part.field();
    let class = vm.space.heap.get(o)?.class;
    let path = || ow_world::world::pawn_meshes::handle_path(vm, o);
    let p = vm.space.prop(class, name).ok_or_else(|| bad(WHAT, format!("the {} {} declares no {name}", part.label(), path())))?;
    if !vm.space.classes.decl(p).is_some_and(|d| matches!(d.kind, PropKind::Object { .. })) {
        return Err(bad(WHAT, format!("the {} {} declares {name} as something other than an object", part.label(), path())));
    }
    match vm.space.read(o, PropAddr::scalar(p))? {
        Value::Object(m) => Ok(*m),
        other => Err(bad(WHAT, format!("{name} holds {}, not an object", other.kind_name()))),
    }
}

/// The rule (module documentation) over the three objects; `reach` is called on each object when the rule reads it,
/// and answers the object to read (or refuses).
pub fn rule(
    vm: &mut Vm,
    barrel: Option<ObjRef>,
    elemental: Option<ObjRef>,
    weapon_type: Option<ObjRef>,
    reach: &mut dyn FnMut(&mut Vm, Part, ObjRef) -> Result<ObjRef, VmError>,
) -> Result<Option<ObjRef>, VmError> {
    for (part, o) in [(Part::Barrel, barrel), (Part::Elemental, elemental)] {
        if let Some(o) = o {
            let o = reach(vm, part, o)?;
            if let Some(mode) = mode_of(vm, part, o)? {
                return Ok(Some(mode));
            }
        }
    }
    match weapon_type {
        Some(t) => {
            let t = reach(vm, Part::WeaponType, t)?;
            mode_of(vm, Part::WeaponType, t)
        }
        None => Ok(None),
    }
}

/// The native's load-free step: `o` as it is when resident, else `NotYet { NotRead }` naming it (module
/// documentation). Never a load, never a retry of a failed one.
pub fn resident(vm: &mut Vm, part: Part, o: ObjRef) -> Result<ObjRef, VmError> {
    let path = ow_world::world::pawn_meshes::handle_path(vm, o);
    if vm.defs.is_failed(&vm.space, o) {
        return Err(NotYet::not_read(format!("{WHAT}: the {} {path}, whose load failed", part.label()), RECORD).into());
    }
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || slotless {
        return Err(NotYet::not_read(
            format!("{WHAT}: the {} {path}, which the store has not loaded (a load is a write)", part.label()),
            RECORD,
        )
        .into());
    }
    Ok(o)
}

/// The weapon's three definition-data objects (module documentation, "The native").
fn definition_data(vm: &Vm, weapon: ObjRef) -> Result<[Option<ObjRef>; 3], VmError> {
    let class = vm.space.heap.get(weapon)?.class;
    let path = || ow_attr::eval::class_path(&vm.space, weapon);
    let p = vm.space.prop(class, "DefinitionData").ok_or_else(|| bad(WHAT, format!("{} declares no DefinitionData", path())))?;
    let expected = vm.space.classes.struct_by_path(DEFINITION_DATA);
    let ty = match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Struct(ty)) if Some(*ty) == expected => *ty,
        _ => return Err(bad(WHAT, format!("{} declares DefinitionData as something other than a {DEFINITION_DATA}", path()))),
    };
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(WHAT, "DefinitionData has no struct layout"))?;
    let data = vm.space.read(weapon, PropAddr::scalar(p))?;
    let s = data.as_struct().ok_or_else(|| bad(WHAT, format!("DefinitionData holds {}, not a struct", data.kind_name())))?;
    let mut out = [None; 3];
    for (slot, name) in out.iter_mut().zip(["BarrelPartDefinition", "ElementalPartDefinition", "WeaponTypeDefinition"]) {
        let (index, decl) = vm
            .space
            .names
            .find(name)
            .and_then(|n| layout.field(n))
            .ok_or_else(|| bad(WHAT, format!("DefinitionData has no {name}")))?;
        if !matches!(decl.kind, PropKind::Object { .. }) {
            return Err(bad(WHAT, format!("DefinitionData's {name} is not an object reference")));
        }
        *slot = match s.fields.get(index as usize) {
            Some(Value::Object(o)) => *o,
            Some(other) => return Err(bad(WHAT, format!("{name} holds {}, not an object", other.kind_name()))),
            None => return Err(bad(WHAT, format!("DefinitionData has no {name}"))),
        };
    }
    Ok(out)
}

/// `WillowWeapon.GetFiringModeDefinition()` in Off (module documentation).
pub fn get_firing_mode_definition(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let weapon = c.this.ok_or_else(|| bad(WHAT, "called without a weapon"))?;
    let [barrel, elemental, weapon_type] = definition_data(c.vm, weapon)?;
    Ok(Value::Object(rule(c.vm, barrel, elemental, weapon_type, &mut resident)?))
}

/// Binds the key in `ParityMode::Off` (a key bound first keeps its binding); Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity == ParityMode::Off && !vm.has_native(KEY) {
        vm.register_native(KEY, Rc::new(get_firing_mode_definition))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "firing_mode_tests.rs"]
mod tests;
