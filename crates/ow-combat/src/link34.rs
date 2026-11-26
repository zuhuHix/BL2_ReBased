//! Owner: lane IMPL-LINK34-AFTER-MIND (decision 2026-11-26-NNN). Five natives links 3 and 4 meet after the mind, in
//! `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK34_AFTER_MIND.md sections 4 to 8 (#793 as landed)
//! alone; this lane did not read the executable. **Every rule is UNVERIFIED in the running game.** Reference binds
//! nothing. Each is read-only: nothing is written, loaded or drawn; each field read with its declared kind, else
//! `VmError::Script`; each object read resident, else `NotRead`.
//!
//! - **`GameInfo.IsFriendlyFire(Victim, InstigatingTargetable)`** (section 4): the arguments declared as an object and
//!   an interface; a `WillowCoopGameInfo` receiver is [`COOP_FRIENDLY_FIRE`] (`NotRead`: four parts not named); any
//!   other game info false.
//! - **`Pawn.InGodMode()`** and the controller keys (section 5): the pawn's `Controller`, else its `DrivenVehicle`'s
//!   `Controller`, else false. A controller: a `WillowMind` with the VM-wide AI-god flag set, true; then `bGodMode` or
//!   `bNonCheatGodMode`; then on a `GearboxMind`: its own `Pawn` with `bCanBeInjured`, false; else its `AIComponent`'s
//!   god hold list non-empty. `WillowMind.GetAIGod` / `SetAIGod` read and write the flag (initially false).
//! - **`DamageTypeDefinition.IsHealingDamageType()`** (section 6): a `WillowDamageTypeDefinition` whose `DamageType`
//!   is the enum's `DAMAGE_TYPE_Healing`; the base class false.
//! - **`WillowPawn.AdjustMomentumForSurfaceType(Magnitude, InstigatedBy, DamageType, HitInfo)`** (section 7): no
//!   damage type, the magnitude; else the magnitude times the damage type's momentum scale for the victim's surface,
//!   the scale's init data evaluated with `InstigatedBy` as the context source (its definition and resolvers resident
//!   first, else `NotRead`). The surface and the scale are in [`surface`] and [`momentum_scale`].
//! - **`Pawn.IsVulnerableToFriendlyFire()`** (section 8): a `WillowPlayerPawn` whose
//!   `FriendlyFireDamageAcceptedBaseValue` is above 0.0001 (UNVERIFIED: the note's layout reading; with the known gap
//!   the read would be `FriendlyFireDamageAccepted`); any other pawn false.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::hit::bind_fn;

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINK34_AFTER_MIND.md";
pub const NOT_LOADED: &str = "an object the store has not loaded (a load is a write)";
pub const COOP_FRIENDLY_FIRE: &str = "WillowCoopGameInfo.IsFriendlyFire: four parts are not named";
pub const GOD_NOT_LOADED: &str = "Pawn.InGodMode: an object the store has not loaded";
pub const IS_FRIENDLY_FIRE: [&str; 2] =
    ["Engine.GameInfo.IsFriendlyFire", "WillowGame.WillowCoopGameInfo.IsFriendlyFire"];
pub const IN_GOD_MODE: [&str; 4] = [
    "Engine.Pawn.InGodMode",
    "Engine.Controller.InGodMode",
    "GearboxFramework.GearboxMind.InGodMode",
    "WillowGame.WillowMind.InGodMode",
];
pub const GET_AI_GOD: &str = "WillowGame.WillowMind.GetAIGod";
pub const SET_AI_GOD: &str = "WillowGame.WillowMind.SetAIGod";
pub const IS_HEALING: [&str; 2] =
    ["Engine.DamageTypeDefinition.IsHealingDamageType", "WillowGame.WillowDamageTypeDefinition.IsHealingDamageType"];
pub const ADJUST_MOMENTUM: &str = "WillowGame.WillowPawn.AdjustMomentumForSurfaceType";
pub const IS_VULNERABLE: [&str; 2] =
    ["Engine.Pawn.IsVulnerableToFriendlyFire", "WillowGame.WillowPlayerPawn.IsVulnerableToFriendlyFire"];

fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: what.to_string(), message: message.into() }
}

/// `o`'s `name` with its declared kind.
fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("{} declares no {name}", path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} undeclared")))?;
    Ok((vm.space.read(o, PropAddr::scalar(p))?.clone(), kind))
}

fn path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).ok().and_then(|x| vm.space.classes.get_layout(x.class)).map_or_else(String::new, |l| l.path().to_string())
}

fn object(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name, what)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(what, format!("{name} is not declared as an object"))),
    }
}

fn boolean(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<bool, VmError> {
    match read(vm, o, name, what)? {
        (Value::Bool(b), PropKind::Bool) => Ok(b),
        _ => Err(bad(what, format!("{name} is not declared as a bool"))),
    }
}

fn float(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<f32, VmError> {
    match read(vm, o, name, what)? {
        (Value::Float(f), PropKind::Float | PropKind::FloatAttribute) => Ok(f),
        _ => Err(bad(what, format!("{name} is not declared as a float"))),
    }
}

fn byte(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<u8, VmError> {
    match read(vm, o, name, what)? {
        (Value::Byte(b), PropKind::Byte { .. }) => Ok(b),
        _ => Err(bad(what, format!("{name} is not declared as a byte"))),
    }
}

fn resident(vm: &Vm, o: ObjRef, refusal: &str) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(refusal.to_string(), RECORD).into());
    }
    Ok(())
}

fn is_a(vm: &Vm, o: ObjRef, wanted: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(l) = vm.space.classes.get_layout(c) else { return Ok(false) };
        if l.path().as_str().eq_ignore_ascii_case(wanted) {
            return Ok(true);
        }
        cursor = l.super_class();
    }
    Ok(false)
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without a receiver"))
}

// --- IsFriendlyFire -------------------------------------------------------------------------------------------------

/// `GameInfo.IsFriendlyFire` (module documentation).
pub fn is_friendly_fire(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "GameInfo.IsFriendlyFire";
    let game = this(c, WHAT)?;
    let params = c.vm.function(c.function).map(|d| d.params.clone()).unwrap_or_default();
    let declared = matches!(
        params.as_slice(),
        [a, b, ..] if matches!(a.kind, PropKind::Object { .. }) && matches!(b.kind, PropKind::Interface { .. })
    );
    if !declared {
        return Err(bad(WHAT, "the parameters are not declared as an object and an interface"));
    }
    if is_a(c.vm, game, "WillowGame.WillowCoopGameInfo")? {
        return Err(NotYet::not_read(COOP_FRIENDLY_FIRE, RECORD).into());
    }
    Ok(Value::Bool(false))
}

// --- InGodMode ------------------------------------------------------------------------------------------------------

/// The controller's god-mode rule (module documentation). Reads only.
pub fn controller_god(vm: &Vm, controller: ObjRef) -> Result<bool, VmError> {
    const WHAT: &str = "Controller.InGodMode";
    resident(vm, controller, GOD_NOT_LOADED)?;
    if is_a(vm, controller, "WillowGame.WillowMind")? && crate::hit::with_state(vm, |s| s.ai_god())? {
        return Ok(true);
    }
    if boolean(vm, controller, "bGodMode", WHAT)? || boolean(vm, controller, "bNonCheatGodMode", WHAT)? {
        return Ok(true);
    }
    if !is_a(vm, controller, "GearboxFramework.GearboxMind")? {
        return Ok(false);
    }
    if let Some(pawn) = object(vm, controller, "Pawn", WHAT)? {
        resident(vm, pawn, GOD_NOT_LOADED)?;
        if boolean(vm, pawn, "bCanBeInjured", WHAT)? {
            return Ok(false);
        }
    }
    let Some(component) = object(vm, controller, "AIComponent", WHAT)? else {
        return Ok(false);
    };
    resident(vm, component, GOD_NOT_LOADED)?;
    match read(vm, component, "GodHolds", WHAT)? {
        (v, PropKind::Array(_)) => Ok(v.elements().is_some_and(|e| !e.is_empty())),
        _ => Err(bad(WHAT, "GodHolds is not declared as an array")),
    }
}

/// `Pawn.InGodMode()` (module documentation). Reads only.
pub fn pawn_god(vm: &Vm, pawn: ObjRef) -> Result<bool, VmError> {
    const WHAT: &str = "Pawn.InGodMode";
    resident(vm, pawn, GOD_NOT_LOADED)?;
    if let Some(controller) = object(vm, pawn, "Controller", WHAT)? {
        return controller_god(vm, controller);
    }
    if let Some(vehicle) = object(vm, pawn, "DrivenVehicle", WHAT)? {
        resident(vm, vehicle, GOD_NOT_LOADED)?;
        if let Some(controller) = object(vm, vehicle, "Controller", WHAT)? {
            return controller_god(vm, controller);
        }
    }
    Ok(false)
}

fn in_god_mode(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c, "InGodMode")?;
    let god = if is_a(c.vm, me, "Engine.Controller")? { controller_god(c.vm, me)? } else { pawn_god(c.vm, me)? };
    Ok(Value::Bool(god))
}

// --- IsHealingDamageType --------------------------------------------------------------------------------------------

/// `DamageTypeDefinition.IsHealingDamageType()` (module documentation).
pub fn is_healing(vm: &Vm, definition: ObjRef) -> Result<bool, VmError> {
    const WHAT: &str = "DamageTypeDefinition.IsHealingDamageType";
    if !is_a(vm, definition, "WillowGame.WillowDamageTypeDefinition")? {
        return Ok(false);
    }
    resident(vm, definition, NOT_LOADED)?;
    let (v, kind) = read(vm, definition, "DamageType", WHAT)?;
    let (Value::Byte(b), PropKind::Byte { enum_ }) = (v, kind) else {
        return Err(bad(WHAT, "DamageType is not declared as a byte"));
    };
    let healing = match enum_ {
        Some(e) => vm
            .space
            .classes
            .enum_layout(e)
            .and_then(|l| l.values().iter().position(|n| vm.space.names.text(*n).eq_ignore_ascii_case("DAMAGE_TYPE_Healing")))
            .ok_or_else(|| bad(WHAT, "DamageType's enum has no DAMAGE_TYPE_Healing"))?,
        None => 6,
    };
    Ok(usize::from(b) == healing)
}

// --- AdjustMomentumForSurfaceType -----------------------------------------------------------------------------------

const MOMENTUM: &str = "WillowPawn.AdjustMomentumForSurfaceType";

fn name_in(v: &Value, vm: &Vm, field: &str) -> Option<NameId> {
    let s = v.as_struct()?;
    let id = vm.space.names.find(field)?;
    let (i, _) = vm.space.classes.struct_layout(s.ty)?.field(id)?;
    match s.fields.get(usize::try_from(i).ok()?)? {
        Value::Name(n) => Some(*n),
        _ => None,
    }
}

/// The victim's damage surface for a hit (section 7): 3 for a live shield pool with value; else the hit region's
/// physical material property's `DamageSurfaceType` (a `WillowPhysicalMaterialProperty`), else the region's
/// `DefaultDamageSurfaceType`; no region, or no helper or body class, 0. Reads only.
pub fn surface(vm: &Vm, pawn: ObjRef, hit: &Value) -> Result<u8, VmError> {
    resident(vm, pawn, NOT_LOADED)?;
    let (reference, _) = read(vm, pawn, "ShieldArmor", MOMENTUM)?;
    if let Ok(r) = ow_world::pools::read_ref(&vm.space, &reference)
        && let (_, Some(pool)) = ow_world::pools::resolve(&vm.space, r)?
    {
        resident(vm, pool, NOT_LOADED)?;
        if float(vm, pool, "CurrentValue", MOMENTUM)? > 0.0 {
            return Ok(3);
        }
    }
    let helper = object(vm, pawn, "MyHitRegionHelper", MOMENTUM)?;
    let body = object(vm, pawn, "BodyClass", MOMENTUM)?;
    let (Some(_), Some(body)) = (helper, body) else { return Ok(0) };
    resident(vm, body, NOT_LOADED)?;
    let bone = name_in(hit, vm, "BoneName");
    let mut region = None;
    if let (Value::Array(list), PropKind::Array(_)) = read(vm, body, "HitRegionList", MOMENTUM)? {
        for entry in list.iter() {
            let Value::Object(Some(r)) = entry else { continue };
            resident(vm, *r, NOT_LOADED)?;
            let bones = match read(vm, *r, "RegionBoneNames", MOMENTUM)? {
                (Value::Array(b), PropKind::Array(_)) => b,
                _ => return Err(bad(MOMENTUM, "RegionBoneNames is not declared as an array")),
            };
            if bone.is_some_and(|n| bones.iter().any(|v| *v == Value::Name(n))) {
                region = Some(*r);
                break;
            }
        }
    } else {
        return Err(bad(MOMENTUM, "HitRegionList is not declared as an array"));
    }
    let region = match region {
        Some(r) => Some(r),
        None => object(vm, body, "DefaultHitRegion", MOMENTUM)?,
    };
    let Some(region) = region else { return Ok(0) };
    resident(vm, region, NOT_LOADED)?;
    if let Some(material) = object(vm, region, "PhysicalMaterial", MOMENTUM)? {
        resident(vm, material, NOT_LOADED)?;
        if let Some(property) = object(vm, material, "PhysicalMaterialProperty", MOMENTUM)?
            && is_a(vm, property, "WillowGame.WillowPhysicalMaterialProperty")?
        {
            resident(vm, property, NOT_LOADED)?;
            return byte(vm, property, "DamageSurfaceType", MOMENTUM);
        }
    }
    byte(vm, region, "DefaultDamageSurfaceType", MOMENTUM)
}

/// The momentum scale's init data for `surface` (section 7): `Some(data)` for a `WillowDamageTypeDefinition` and
/// surfaces 1 to 3, else the constant (1.0 for another surface, 0.0 for the base class). Its definitions checked
/// resident with their chains' resolvers. Loads nothing.
fn momentum_scale(vm: &mut Vm, definition: ObjRef, surface: u8) -> Result<Result<ow_attr::api::InitData, f32>, VmError> {
    if !is_a(vm, definition, "WillowGame.WillowDamageTypeDefinition")? {
        return Ok(Err(0.0));
    }
    resident(vm, definition, NOT_LOADED)?;
    let field = match surface {
        1 => "FleshMomentumModifier",
        2 => "ArmorMomentumModifier",
        3 => "ShieldMomentumModifier",
        _ => return Ok(Err(1.0)),
    };
    let (v, kind) = read(vm, definition, field, MOMENTUM)?;
    if !matches!(kind, PropKind::Struct(_)) {
        return Err(bad(MOMENTUM, format!("{field} is not declared as a struct")));
    }
    let data = ow_attr::eval::init_data(&vm.space, &v)?;
    if let Some(attr) = data.base_value_attribute {
        resident(vm, attr, NOT_LOADED)?;
        let attrs = vm.attrs.clone();
        attrs.preflight_read_resident(attr, &mut ow_attr::api::EvalCx::new(vm))?;
    }
    if let Some(def) = data.initialization_definition {
        resident(vm, def, NOT_LOADED)?;
    }
    Ok(Ok(data))
}

fn adjust_momentum(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pawn = this(c, MOMENTUM)?;
    let magnitude = match c.arg(0)? {
        Value::Float(f) => *f,
        other => return Err(bad(MOMENTUM, format!("Magnitude is {}", other.kind_name()))),
    };
    let instigator = crate::hit::arg_object(c, 1)?;
    let Some(definition) = crate::hit::arg_object(c, 2)? else {
        return Ok(Value::Float(magnitude));
    };
    let hit = if c.arg_count() > 3 { c.arg(3)?.clone() } else { Value::None };
    let s = surface(c.vm, pawn, &hit)?;
    let scale = match momentum_scale(c.vm, definition, s)? {
        Err(constant) => constant,
        Ok(data) => {
            let attrs = c.vm.attrs.clone();
            attrs.evaluate_init(&data, instigator, None, &mut ow_attr::api::EvalCx::new(c.vm))?
        }
    };
    Ok(Value::Float(magnitude * scale))
}

// --- IsVulnerableToFriendlyFire -------------------------------------------------------------------------------------

/// `Pawn.IsVulnerableToFriendlyFire()` (module documentation).
pub fn is_vulnerable(vm: &Vm, pawn: ObjRef) -> Result<bool, VmError> {
    if !is_a(vm, pawn, "WillowGame.WillowPlayerPawn")? {
        return Ok(false);
    }
    resident(vm, pawn, NOT_LOADED)?;
    Ok(float(vm, pawn, "FriendlyFireDamageAcceptedBaseValue", "Pawn.IsVulnerableToFriendlyFire")? > 0.0001)
}

/// Binds the natives in Off (keep-existing).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    for key in IS_FRIENDLY_FIRE {
        bind_fn(vm, key, is_friendly_fire)?;
    }
    for key in IN_GOD_MODE {
        bind_fn(vm, key, in_god_mode)?;
    }
    bind_fn(vm, GET_AI_GOD, |c| Ok(Value::Bool(crate::hit::with_state(c.vm, |s| s.ai_god())?)))?;
    bind_fn(vm, SET_AI_GOD, |c| {
        let on = match c.arg(0)? {
            Value::Bool(b) => *b,
            other => return Err(bad(SET_AI_GOD, format!("the flag is {}", other.kind_name()))),
        };
        crate::hit::with_state(c.vm, |s| s.set_ai_god(on))?;
        Ok(Value::None)
    })?;
    for key in IS_HEALING {
        bind_fn(vm, key, |c| {
            let me = this(c, "IsHealingDamageType")?;
            Ok(Value::Bool(is_healing(c.vm, me)?))
        })?;
    }
    bind_fn(vm, ADJUST_MOMENTUM, adjust_momentum)?;
    for key in IS_VULNERABLE {
        bind_fn(vm, key, |c| {
            let me = this(c, "IsVulnerableToFriendlyFire")?;
            Ok(Value::Bool(is_vulnerable(c.vm, me)?))
        })?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "link34_tests.rs"]
mod tests;
