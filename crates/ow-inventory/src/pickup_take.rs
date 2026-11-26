//! Owner: lane IMPL-UPDATEAMMOCOUNTS. Link 6's take after affordability, the natives of `ow-inventory`'s side, in
//! `ParityMode::Off`: `WillowPlayerController.UpdateAmmoCounts`, `WillowPickup.GetSaveRotation`,
//! `WillowInventory.IsDLCRequirementMet`, `WillowWeapon.GetResourcePoolContext` and
//! `AttributeExpression.EvaluateExpressions`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md sections 3.1, 3.2, 3.6,
//! 3.8 and 4.2 alone (lane ANALYST-UPDATEAMMOCOUNTS, #785, landed at 0195ce94); this lane did not read the executable. **Every
//! rule is UNVERIFIED in the running game.** Reference binds nothing (the stub policy answers).
//!
//! **`UpdateAmmoCounts(bSilent)`** (3.1). The preflight writes and loads nothing:
//! - no world globals object: nothing;
//! - the globals definition resident (`globals::globals_definition_resident`'s refusals);
//! - the eight definition fields declared as objects on its class, and the eight caches as floats on the controller's,
//!   else `VmError::Script`;
//! - each set definition resident, else [`AMMO_NOT_LOADED`] (`NotRead`);
//! - all eight lookups' own refusal checked against the state before the call (the hub's correction on #785): the
//!   lookup refuses before it writes only for a controller whose pawn is a vehicle (`pools::controller_pool`), so that
//!   is checked once here, [`VEHICLE_POOL`] (`NotRead`). A later lookup's refusal (a pool definition's load, a
//!   creation) comes after a write and carries the prefix.
//!
//! The commit, in [`AMMO`]'s order, per set definition: the controller's pool through
//! `pools::controller_pool(.., create = true)` (the lookup with creation, NATIVE_COMBAT_GROUNDWORK.md 1.5: a missing
//! pool is made); its `CurrentValue` through the resolved handle, else 0; when it differs from the cache, the script
//! event `ScriptAnnounceAmmoGain(kind, gain)` when it is larger and `bSilent` is false (gain = (value − cache)
//! truncated to an int), then the cache := the value. A refusal after the first write (a creation, a cache, the event)
//! is marked "stopped after partial writes" (#518's counters).
//!
//! **`GetSaveRotation()`**, by NATIVE_PICKUP_SAVE_ROTATION.md section 5 (the hub's correction on #785, which replaces
//! #785's section 3.2), reads only: `Physics` compared with `PHYS_RigidBody`, the value found in the declared
//! `EPhysics` enum by name; not rigid body: the actor's `Rotation`; a None `CollisionComponent`: the actor's
//! `Rotation`; rigid body with a `CollisionComponent`: [`BODY_POSE`] (`LayerAbsent`, the physics layer), the note's
//! default. **The hub's at-rest ruling** (on #785, extending #574): in a world whose set-up turned on
//! [`AtRestBodyPose`] (the slice's pickup world, as `PhysXLevel` is a harness setting), that case answers the actor's
//! `Rotation`, with the stand-in line [`AT_REST_LINE`], UNVERIFIED. Every other world, Reference included, keeps the
//! `LayerAbsent`. Malformed `Physics`, `CollisionComponent` or `Rotation`: `VmError::Script`.
//!
//! **`IsDLCRequirementMet(Other)`** (3.6), for a receiver whose first native class is `WillowGame.WillowWeapon`: no
//! `DefinitionData.BalanceDefinition`, or a balance whose `DlcItemSet` is None: true. `DefinitionData` (a struct),
//! its `BalanceDefinition` and the balance's `DlcItemSet` (objects) must each be declared, else `VmError::Script`. A set item set is
//! [`DLC_ITEM_SET`] (`NotRead`); a balance the store has not loaded is [`BALANCE_NOT_LOADED`] (`NotRead`); any other
//! first native class is [`OTHER_INVENTORY`] (`NotRead`). Reads only.
//!
//! **`GetResourcePoolContext()`** (3.8): the weapon's `Instigator`'s `Controller`, else that `Instigator`'s
//! `DrivenVehicle`'s `Controller`; no `Instigator`: None. Each field declared as an object, else `VmError::Script`.
//!
//! **`EvaluateExpressions(Mode, ContextSource, Expressions, OptionalOverrideContextSource)`** (4.2): `ow-attr`'s rule
//! (NATIVE_ATTRIBUTES.md section 11): mode 0 And, 1 Or, an empty list true. A mode outside 0..=1 is `VmError::Script`.

use std::rc::Rc;

use ow_attr::api::{EvalCx, ExprMode};
use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::attach_mesh::pickup_rb::WriteCounters;
use crate::equip::{bad, field, object_of, read};

/// The record of the rules.
pub const RECORD: &str = "NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md";
/// The native keys.
pub const UPDATE_AMMO_COUNTS: &str = "WillowPlayerController.UpdateAmmoCounts(bool)";
pub const GET_SAVE_ROTATION: &str = "WillowPickup.GetSaveRotation()";
pub const IS_DLC_REQUIREMENT_MET: &str = "WillowInventory.IsDLCRequirementMet(object)";
pub const GET_RESOURCE_POOL_CONTEXT: &str = "WillowWeapon.GetResourcePoolContext()";
pub const EVALUATE_EXPRESSIONS: &str = "AttributeExpression.EvaluateExpressions(byte,object,array,object)";
/// The refusals.
pub const AMMO_NOT_LOADED: &str = "UpdateAmmoCounts: an ammo definition the store has not loaded (a load is a write)";
pub const DLC_ITEM_SET: &str = "IsDLCRequirementMet: the DLC item set's own check";
pub const BALANCE_NOT_LOADED: &str = "IsDLCRequirementMet: a balance definition the store has not loaded (a load is a write)";
pub const OTHER_INVENTORY: &str = "IsDLCRequirementMet: the DLC check of";
pub const VEHICLE_POOL: &str =
    "the pool lookup of a vehicle that provides pools (Controller.GetResourcePoolForResourceDefinition)";
pub const BODY_POSE: &str = "WillowPickup.GetSaveRotation: a rigid-body pickup's body pose";
pub const PHYSICS_LAYER: &str = "a physics layer (rigid bodies)";
/// The eight ammo kinds, in the native's order: (the definition field on `GlobalsDefinition`, the cache field on
/// `WillowPlayerController`, the event's kind).
pub const AMMO: [(&str, &str, i32); 8] = [
    ("AmmoDefinition_RepeaterPistol", "LastKnownAmmoCount_RepeaterPistol", 3),
    ("AmmoDefinition_RevolverPistol", "LastKnownAmmoCount_RevolverPistol", 2),
    ("AmmoDefinition_SMG", "LastKnownAmmoCount_SMG", 1),
    ("AmmoDefinition_Shotgun", "LastKnownAmmoCount_Shotgun", 4),
    ("AmmoDefinition_CombatRifle", "LastKnownAmmoCount_CombatRifle", 0),
    ("AmmoDefinition_SniperRifle", "LastKnownAmmoCount_SniperRifle", 5),
    ("AmmoDefinition_RocketLauncher", "LastKnownAmmoCount_RocketLauncher", 6),
    ("GrenadeAmmoDefinition", "LastKnownAmmoCount_Grenades", 7),
];

fn unloaded(vm: &Vm, o: ObjRef) -> bool {
    ow_world::touch_trace::is_unloaded(vm, o) || vm.defs.is_failed(&vm.space, o)
}

/// `name` on `o`'s class declared as `want`, else a data error.
fn declared(vm: &Vm, o: ObjRef, name: &str, want: fn(&PropKind) -> bool, kind: &str, what: &str) -> Result<(), VmError> {
    match ow_world::world::component_setters::declared(vm, o, name, what) {
        Ok(k) if want(&k) => Ok(()),
        Ok(k) => Err(bad(what, format!("{name} is declared as {}, not {kind}", k.kind_name()))),
        Err(e) => Err(e),
    }
}

fn object_field(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    declared(vm, o, name, |k| matches!(k, PropKind::Object { .. }), "an object", what)?;
    match read(vm, o, name)? {
        Some(v) => object_of(&v, what),
        None => Ok(None),
    }
}

fn float_field(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<f32, VmError> {
    match read(vm, o, name)? {
        Some(Value::Float(x)) => Ok(x),
        other => Err(bad(what, format!("{name} holds {other:?}, not a float"))),
    }
}

/// `UpdateAmmoCounts(bSilent)` (module documentation).
fn update_ammo_counts(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let controller = c.this.ok_or_else(|| bad(AMMO_WHAT, "called without a controller"))?;
    let silent = match c.arg(0)? {
        Value::Bool(b) => *b,
        other => return Err(bad(AMMO_WHAT, format!("bSilent holds {}, not a bool", other.kind_name()))),
    };
    let globals = match crate::globals::world_globals(c.vm) {
        Ok(g) => g,
        Err(VmError::NotYet(n)) if matches!(n.missing, ow_core::Missing::LayerAbsent(_)) => return Ok(Value::None),
        Err(e) => return Err(e),
    };
    let definition = crate::globals::globals_definition_resident(c.vm, globals, AMMO_WHAT)?;
    update_ammo(c.vm, controller, definition, silent, &mut pool_value)?;
    Ok(Value::None)
}

const AMMO_WHAT: &str = "WillowPlayerController.UpdateAmmoCounts";

/// The value of `controller`'s pool for `ammo` by the lookup with creation (module documentation): its `CurrentValue`
/// through the resolved handle, else 0.
fn pool_value(vm: &mut Vm, controller: ObjRef, ammo: ObjRef) -> Result<f32, VmError> {
    let r = ow_world::pools::controller_pool(vm, controller, Some(ammo), true)?;
    let (_, pool) = ow_world::pools::resolve(&vm.space, r)?;
    match pool {
        Some(p) => float_field(vm, p, "CurrentValue", AMMO_WHAT),
        None => Ok(0.0),
    }
}

/// The pool lookup [`update_ammo`] uses: (the controller, the ammo definition) to the value.
pub(crate) type PoolValue<'a> = dyn FnMut(&mut Vm, ObjRef, ObjRef) -> Result<f32, VmError> + 'a;

/// `UpdateAmmoCounts`' rule on `controller` with the globals definition `definition` (module documentation); the
/// pool lookup is `lookup` (the native passes the real one; the tests a toy).
pub(crate) fn update_ammo(
    vm: &mut Vm,
    controller: ObjRef,
    definition: ObjRef,
    silent: bool,
    lookup: &mut PoolValue<'_>,
) -> Result<(), VmError> {
    let counters = WriteCounters::of(vm);
    let planned = ammo_plan(vm, controller, definition)?;
    ammo_commit(vm, controller, planned, silent, lookup, counters)
}

/// [`update_ammo`]'s preflight (module documentation): reads only.
fn ammo_plan(vm: &Vm, controller: ObjRef, definition: ObjRef) -> Result<Vec<(ObjRef, &'static str, i32)>, VmError> {
    const WHAT: &str = AMMO_WHAT;
    let mut planned: Vec<(ObjRef, &'static str, i32)> = Vec::new();
    for (def_field, cache, kind) in AMMO {
        let ammo = object_field(vm, definition, def_field, WHAT)?;
        declared(vm, controller, cache, |k| matches!(k, PropKind::Float), "a float", WHAT)?;
        if let Some(a) = ammo {
            if unloaded(vm, a) {
                return Err(NotYet::not_read(AMMO_NOT_LOADED, RECORD).into());
            }
            planned.push((a, cache, kind));
        }
    }
    // The lookups' own refusal before any write: a controller whose pawn is a vehicle.
    if !planned.is_empty()
        && let Some(Value::Object(Some(pawn))) = read(vm, controller, "Pawn")?
    {
        let class = vm.space.heap.get(pawn)?.class;
        if vm.space.classes.by_path("Engine.Vehicle").is_some_and(|v| vm.space.classes.is_child_of(class, v)) {
            return Err(NotYet::not_read(VEHICLE_POOL, ow_world::pools::RECORD).into());
        }
    }
    Ok(planned)
}

/// [`update_ammo`]'s commit, after its preflight ([`ammo_plan`]).
fn ammo_commit(
    vm: &mut Vm,
    controller: ObjRef,
    planned: Vec<(ObjRef, &'static str, i32)>,
    silent: bool,
    lookup: &mut PoolValue<'_>,
    counters: WriteCounters,
) -> Result<(), VmError> {
    const WHAT: &str = AMMO_WHAT;
    let mark = |vm: &Vm, e: VmError| counters.mark(vm, e);
    for (ammo, cache, kind) in planned {
        let value = lookup(vm, controller, ammo).map_err(|e| mark(vm, e))?;
        let old = float_field(vm, controller, cache, WHAT).map_err(|e| mark(vm, e))?;
        if value == old {
            continue;
        }
        if value > old && !silent {
            // Truncated toward zero, as the note gives it (an f32 cast saturates at the int range).
            let gain = (value - old) as i32;
            ow_world::actors::send_event(vm, controller, "ScriptAnnounceAmmoGain", vec![Value::Int(kind), Value::Int(gain)])
                .map_err(|e| mark(vm, e))?;
        }
        ow_world::world::component_setters::write(vm, controller, cache, Value::Float(value), WHAT)
            .map_err(|e| mark(vm, e))?;
    }
    Ok(())
}

/// `GetSaveRotation()` (module documentation). Reads only.
fn get_save_rotation(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPickup.GetSaveRotation";
    let pickup = c.this.ok_or_else(|| bad(WHAT, "called without a pickup"))?;
    let vm: &Vm = c.vm;
    declared(vm, pickup, "Rotation", |k| matches!(k, PropKind::Struct(_)), "a struct", WHAT)?;
    let rotation = read(vm, pickup, "Rotation")?.ok_or_else(|| bad(WHAT, "the pickup holds no Rotation"))?;
    let rigid = enum_value(vm, pickup, "Physics", "PHYS_RigidBody", WHAT)?;
    let physics = match read(vm, pickup, "Physics")? {
        Some(Value::Byte(b)) => b,
        other => return Err(bad(WHAT, format!("Physics holds {other:?}, not a byte"))),
    };
    if physics != rigid || object_field(vm, pickup, "CollisionComponent", WHAT)?.is_none() {
        return Ok(rotation);
    }
    if vm.service::<AtRestBodyPose>().is_some() {
        c.vm.log.push(AT_REST_LINE.to_string());
        return Ok(rotation);
    }
    Err(NotYet::layer_absent(BODY_POSE, PHYSICS_LAYER, RECORD_SAVE_ROTATION).into())
}

/// GetSaveRotation's record.
pub const RECORD_SAVE_ROTATION: &str = "NATIVE_PICKUP_SAVE_ROTATION.md";

/// The harness setting of the hub's at-rest ruling for `GetSaveRotation` (a VM service; its presence turns it on).
#[derive(Debug, Clone, Copy, Default)]
pub struct AtRestBodyPose;

/// The stand-in line `GetSaveRotation` logs under the ruling.
pub const AT_REST_LINE: &str = "harness input: GetSaveRotation: a rigid-body pickup at rest, its body pose taken as \
     the actor's Rotation (hub ruling; UNVERIFIED)";

/// Turns the at-rest ruling on for `vm`'s world (world set-up; Off only, else a registration refusal). Turning it on
/// twice is a registration refusal too.
pub fn enable_at_rest_body_pose(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Err(VmError::Registration {
            key: "AtRestBodyPose".to_string(),
            reason: "the at-rest ruling is an Off harness setting; Reference keeps the LayerAbsent",
        });
    }
    vm.add_service(AtRestBodyPose).map(|_| ())
}

/// The value of enum value `name` in the enum `field` of `o`'s class declares, by name (a lookup only), else a data
/// error.
fn enum_value(vm: &Vm, o: ObjRef, field: &str, name: &str, what: &str) -> Result<u8, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let missing = || bad(what, format!("{field} declares no enum with {name}"));
    let p = vm.space.prop(class, field).ok_or_else(missing)?;
    let Some(PropKind::Byte { enum_: Some(e) }) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
        return Err(missing());
    };
    let layout = vm.space.classes.enum_layout(e).ok_or_else(missing)?;
    let i = layout.values().iter().position(|v| vm.space.names.text(*v) == name).ok_or_else(missing)?;
    u8::try_from(i).map_err(|_| missing())
}

/// `IsDLCRequirementMet(Other)` (module documentation). Reads only.
fn is_dlc_requirement_met(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowInventory.IsDLCRequirementMet";
    let item = c.this.ok_or_else(|| bad(WHAT, "called without an item"))?;
    let vm: &Vm = c.vm;
    let class = vm.space.heap.get(item)?.class;
    let native = ow_world::world::render_hooks::first_native(vm, class);
    if !native.as_deref().is_some_and(|n| n.eq_ignore_ascii_case("WillowGame.WillowWeapon")) {
        let named = native.unwrap_or_else(|| ow_attr::eval::class_path(&vm.space, item));
        return Err(NotYet::not_read(format!("{OTHER_INVENTORY} {named}"), RECORD).into());
    }
    let data_kind = ow_world::world::component_setters::declared(vm, item, "DefinitionData", WHAT)?;
    let PropKind::Struct(ty) = data_kind else {
        return Err(bad(WHAT, format!("DefinitionData is declared as {}, not a struct", data_kind.kind_name())));
    };
    let member = vm.space.names.find("BalanceDefinition").and_then(|n| vm.space.classes.struct_layout(ty)?.field(n));
    if !matches!(member, Some((_, d)) if matches!(d.kind, PropKind::Object { .. })) {
        return Err(bad(WHAT, "DefinitionData declares no BalanceDefinition object"));
    }
    let data = read(vm, item, "DefinitionData")?.ok_or_else(|| bad(WHAT, "the weapon holds no DefinitionData"))?;
    let Some(balance) = object_of(&field(vm, &data, "BalanceDefinition")?, WHAT)? else {
        return Ok(Value::Bool(true));
    };
    if unloaded(vm, balance) {
        return Err(NotYet::not_read(BALANCE_NOT_LOADED, RECORD).into());
    }
    if object_field(vm, balance, "DlcItemSet", WHAT)?.is_some() {
        return Err(NotYet::not_read(DLC_ITEM_SET, RECORD).into());
    }
    Ok(Value::Bool(true))
}

/// `GetResourcePoolContext()` (module documentation). Reads only.
fn get_resource_pool_context(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowWeapon.GetResourcePoolContext";
    let weapon = c.this.ok_or_else(|| bad(WHAT, "called without a weapon"))?;
    let vm: &Vm = c.vm;
    let Some(instigator) = object_field(vm, weapon, "Instigator", WHAT)? else {
        return Ok(Value::Object(None));
    };
    if let Some(controller) = object_field(vm, instigator, "Controller", WHAT)? {
        return Ok(Value::Object(Some(controller)));
    }
    let Some(vehicle) = object_field(vm, instigator, "DrivenVehicle", WHAT)? else {
        return Ok(Value::Object(None));
    };
    Ok(Value::Object(object_field(vm, vehicle, "Controller", WHAT)?))
}

/// `EvaluateExpressions(Mode, ContextSource, Expressions, OptionalOverrideContextSource)` (module documentation).
fn evaluate_expressions(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "AttributeExpression.EvaluateExpressions";
    let mode = match c.arg(0)? {
        Value::Byte(b) => ExprMode::from_raw(i64::from(*b)).ok_or_else(|| bad(WHAT, "Mode outside its enum"))?,
        other => return Err(bad(WHAT, format!("Mode holds {}, not a byte", other.kind_name()))),
    };
    let source = object_of(c.arg(1)?, WHAT)?;
    let rows = match c.arg(2)? {
        Value::Array(rows) => rows.clone(),
        other => return Err(bad(WHAT, format!("Expressions holds {}, not an array", other.kind_name()))),
    };
    let override_source = if c.arg_count() > 3 { object_of(c.arg(3)?, WHAT)? } else { None };
    Ok(Value::Bool(expressions(c.vm, mode, source, &rows, override_source)?))
}

/// `EvaluateExpressions`' rule over the script rows `rows` (module documentation).
pub(crate) fn expressions(
    vm: &mut Vm,
    mode: ExprMode,
    source: Option<ObjRef>,
    rows: &[Value],
    override_source: Option<ObjRef>,
) -> Result<bool, VmError> {
    const WHAT: &str = "AttributeExpression.EvaluateExpressions";
    let exprs = rows
        .iter()
        .map(|r| crate::equip::name_parts::expr_data_for(vm, r, WHAT))
        .collect::<Result<Vec<_>, _>>()?;
    let sys = vm.attrs.clone();
    Ok(sys.expressions_hold(&exprs, mode, source, override_source, &mut EvalCx::new(vm))?)
}

/// Binds the five keys in Off (the caller checks the mode). No pack bound them before (checked by grep; the
/// `UpdateAmmoCounts` `NotRead` of `pickup_cost` is replaced), so a duplicate is a registration error.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    let natives: [(&str, NativeFn); 5] = [
        (UPDATE_AMMO_COUNTS, Rc::new(update_ammo_counts)),
        (GET_SAVE_ROTATION, Rc::new(get_save_rotation)),
        (IS_DLC_REQUIREMENT_MET, Rc::new(is_dlc_requirement_met)),
        (GET_RESOURCE_POOL_CONTEXT, Rc::new(get_resource_pool_context)),
        (EVALUATE_EXPRESSIONS, Rc::new(evaluate_expressions)),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "pickup_take_tests.rs"]
mod tests;
