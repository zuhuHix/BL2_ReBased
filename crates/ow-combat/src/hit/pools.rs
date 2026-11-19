//! Four Off resource-pool rules from NATIVE_RESOURCE_POOL_IDLE.md, implemented from its own-words note only.
//! AI-assisted (Codex/GPT-6.1 Sol). All native/gameplay behavior remains UNVERIFIED in the running game.
//! Reference retains its existing bindings and stubs. No regeneration is implemented here.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use ow_object::layout::PropKind;

use super::access::{get, object, object_of, required, set_required};
use super::{bind_fn, this};

pub const RECORD: &str = "NATIVE_RESOURCE_POOL_IDLE.md";
/// The record of the class starting values (lane FIX-MIND-ADOPT, decision 2026-11-19-26).
pub const RECORD_STARTING: &str = "NATIVE_COMBAT_GROUNDWORK.md sections 1.10 and 1.11";
/// The player controller's native override of the starting values (decision 2026-11-19-46; its rule, decision
/// 2026-11-19-57).
pub const PLAYER_STARTING_VALUES: &str = "WillowGame.WillowPlayerController.ApplyCharacterClassStartingValues";
/// The note both natives are implemented from (lane FIX-CLASS-STARTING-VALUES, decision 2026-11-19-57; the analyst was
/// lane ANALYST-CLASS-STARTING-VALUES, decision 2026-11-19-50).
pub const RECORD_CLASS_STARTING: &str = "docs/verification/NATIVE_CLASS_STARTING_VALUES.md";

fn float(vm: &mut Vm, pool: ObjRef, name: &str) -> Result<f32, VmError> {
    match required(vm, pool, name, "resource pool")? {
        Value::Float(value) => Ok(value),
        _ => Err(NotYet::not_ported(format!("resource pool float {name}"), RECORD).into()),
    }
}

/// Stamp first, then the class's native hook. The note records four empty hook implementations and leaves the
/// shield implementation unread. It gives no rule for an additional subclass's native override: refuse after
/// the stamp, even if that subclass inherits ResourcePool's script functions. Script method inheritance cannot
/// prove native virtual-hook inheritance. Exact qualified identities prevent an unrelated same-named class
/// from being treated as one of the implementations the note read.
pub fn reset_idle_timer(vm: &mut Vm, pool: ObjRef) -> Result<(), VmError> {
    let time = vm.world_time_seconds()?;
    set_required(
        vm,
        pool,
        "PoolIdleDelayStartTime",
        Value::Float(time),
        "resource pool",
    )?;
    let class = vm.space.heap.get(pool)?.class;
    let path = vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.path().to_string())
        .unwrap_or_default();
    match path.to_ascii_lowercase().as_str() {
        "engine.resourcepool" | "engine.healthresourcepool" | "engine.ammoresourcepool"
        | "engine.experienceresourcepool" => Ok(()),
        "engine.shieldresourcepool" => Err(NotYet::not_read(
            "ShieldResourcePool idle-reset hook: provider chain and final function (stamp already written)", RECORD,
        ).into()),
        _ => Err(NotYet::not_read(
            format!("{path} native idle-reset hook/override (stamp already written)"), RECORD,
        ).into()),
    }
}

/// Strict stored-float delay comparison; only a passed delay clears the stamp. No world read or regeneration.
pub fn has_idle_delay_passed(vm: &mut Vm, pool: ObjRef, current: f32) -> Result<bool, VmError> {
    let start = float(vm, pool, "PoolIdleDelayStartTime")?;
    let delay = float(vm, pool, "OnIdleRegenerationDelay")?;
    if start > 0.0 && current < start + delay {
        return Ok(false);
    }
    set_required(
        vm,
        pool,
        "PoolIdleDelayStartTime",
        Value::Float(0.0),
        "resource pool",
    )?;
    Ok(true)
}

pub(super) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.add_service(ow_world::pools::ResetIdleTimerOff(reset_idle_timer))?;
    bind_fn(vm, "Engine.ResourcePoolManager.ShutDown", |c| {
        let manager = this(c, "ResourcePoolManager.ShutDown")?;
        // The existing body preserves authority, slot order, script dispatch and partial effects on error.
        ow_world::pools::delete_all_pools(c.vm, Some(manager))?;
        Ok(Value::None)
    })?;
    bind_fn(vm, "Engine.Controller.ApplyCharacterClassStartingValues", |c| {
        let controller = this(c, "Controller.ApplyCharacterClassStartingValues")?;
        let class = class_argument(c.arg(0)?, "Controller.ApplyCharacterClassStartingValues")?;
        apply_class_starting_values(c.vm, controller, class)?;
        Ok(Value::None)
    })?;
    // The player controller's own override is native too (its declaration carries no script; lane FIX-PLAYER-CLASS-B,
    // decision 2026-11-19-46), NATIVE_CLASS_STARTING_VALUES.md section 2 (decision 2026-11-19-57).
    bind_fn(vm, PLAYER_STARTING_VALUES, |c| {
        let controller = this(c, "WillowPlayerController.ApplyCharacterClassStartingValues")?;
        let class = class_argument(c.arg(0)?, "WillowPlayerController.ApplyCharacterClassStartingValues")?;
        apply_class_starting_values(c.vm, controller, class)?;
        Ok(Value::None)
    })?;
    // Two natives of the AI mind's class route (NATIVE_MIND_ROUTE.md sections 1 and 2; decision 2026-11-19-32).
    bind_fn(vm, "WillowGame.WillowPawn.SetDefaultPhysics", |c| {
        let pawn = this(c, "WillowPawn.SetDefaultPhysics")?;
        set_default_physics(c.vm, pawn)?;
        Ok(Value::None)
    })?;
    for path in ["WillowGame.WillowPawn.GetExpLevel", "WillowGame.WillowAIPawn.GetExpLevel"] {
        bind_fn(vm, path, |c| {
            let pawn = this(c, "WillowPawn.GetExpLevel")?;
            get_exp_level(c.vm, pawn)
        })?;
    }
    bind_fn(vm, "Engine.ResourcePool.GetTimeSeconds", |c| {
        this(c, "ResourcePool.GetTimeSeconds")?;
        Ok(Value::Float(c.vm.world_time_seconds()?))
    })?;
    bind_fn(vm, "Engine.ResourcePool.HasIdleDelayPassed", |c| {
        let pool = this(c, "ResourcePool.HasIdleDelayPassed")?;
        let Value::Float(current) = c.arg(0)? else {
            return Err(NotYet::not_ported("HasIdleDelayPassed float CurrentTime", RECORD).into());
        };
        let current = *current;
        Ok(Value::Bool(has_idle_delay_passed(c.vm, pool, current)?))
    })
}

/// The class argument of either native. One that is not an object reference is a script error, never "no class"
/// (which would delete every pool; review r1 of #412).
fn class_argument(v: &Value, unit: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(*o),
        other => Err(super::access::error(format!(
            "{unit}: the class argument is not an object reference ({})",
            other.kind_name()
        ))),
    }
}

/// `ApplyCharacterClassStartingValues(inCharacterClass)` on any controller, Off, from NATIVE_CLASS_STARTING_VALUES.md
/// alone (clean room; decision 2026-11-19-57). A `WillowPlayerController` (or a subclass) gets the override of section
/// 2, every other controller (AI minds included) the `Controller` base of section 1. Both natives' bindings come here,
/// so a call through the base on a player controller runs the override, as a native virtual does.
///
/// **The base (section 1):** every pool of the controller's `ResourcePoolManager` is deleted, always. Then, with a
/// class, one pool per entry of its `ResourcePools` in array order, then `AccuracyPoolDefinition` into `AccuracyPool`
/// and `OffHandAccuracyPoolDefinition` into `OffHandAccuracyPool`, each only when set. The pools are made by
/// `CreatePool` with the controller as owner (the lowest free slot); a manager that call spawns is written back to
/// `ResourcePoolManager`. No role check, and nothing else changes.
///
/// **The override (section 2):** the base first, with the same class. Then only on the authority (`Role` is
/// `ROLE_Authority`): with the controller's own `PlayerClass` (not the argument), pools for its
/// `ExperiencePoolDefinition`, `SkillCooldownPoolDefinition` and `MeleeSkillCooldownPoolDefinition`, in that order and
/// each only when set, stored as `ExpPool`, `SkillCooldownPool` and `MeleeSkillCooldownPool`; then, with or without a
/// `PlayerClass`, the controller's script event `OnPlayerClassChange()`, run through the VM.
///
/// **References left as they were:** no stored pool reference is cleared. With no class, an unset definition, a
/// controller that is not the authority or no `PlayerClass`, the field keeps what it held. Its pool was deleted, so it
/// reads as not valid by the manager's GUID rule (whether a new pool in the same slot can carry the same GUID is the
/// note's open question 2, not decided here).
///
/// Everything is read and checked before the delete: the class fields, `Role`, `PlayerClass`, the struct types of the
/// reference fields to be written, and that the controller has `OnPlayerClassChange` when it will be called. Refused,
/// `NotYet { NotRead }`, before anything changes: pools that name one resource twice (the base's and the override's
/// together). The note does not give `CreatePool`'s reinitialise flag; after the delete no pool exists, so the flag can
/// matter only for a second pool of the same resource.
///
/// Open question 1 (whether the argument and `PlayerClass` can differ) does not change the rule: each step reads what
/// the note says it reads. UNVERIFIED in the running game.
pub fn apply_class_starting_values(vm: &mut Vm, controller: ObjRef, class: Option<ObjRef>) -> Result<(), VmError> {
    let player = match vm.find_class("WillowGame.WillowPlayerController") {
        Ok(player) => {
            let own = vm.space.heap.get(controller)?.class;
            vm.space.classes.is_child_of(own, player)
        }
        Err(_) => false,
    };
    let unit = if player {
        "WillowPlayerController.ApplyCharacterClassStartingValues"
    } else {
        "Controller.ApplyCharacterClassStartingValues"
    };
    // The pools to make, in order, each with the controller field that stores its reference (none for `ResourcePools`).
    let mut plan: Vec<(ObjRef, Option<&'static str>)> = Vec::new();
    if let Some(class) = class {
        let class = super::access::loaded(vm, class)?;
        if let Some(list) = get(vm, class, "ResourcePools")? {
            let Some(list) = list.elements() else {
                return Err(super::access::error(format!("{unit}: the class's ResourcePools is not an array")));
            };
            plan.extend(list.iter().filter_map(object_of).map(|d| (d, None)));
        }
        for (def, field) in
            [("AccuracyPoolDefinition", "AccuracyPool"), ("OffHandAccuracyPoolDefinition", "OffHandAccuracyPool")]
        {
            if let Some(d) = object(vm, class, def)? {
                plan.push((d, Some(field)));
            }
        }
    }
    let mut event = false;
    if player && is_authority_strict(vm, controller, unit)? {
        let player_class = match required(vm, controller, "PlayerClass", unit)? {
            Value::Object(o) => o,
            other => {
                return Err(super::access::error(format!(
                    "{unit}: PlayerClass holds {}, not an object",
                    other.kind_name()
                )));
            }
        };
        if let Some(pc) = player_class {
            let pc = super::access::loaded(vm, pc)?;
            for (def, field) in [
                ("ExperiencePoolDefinition", "ExpPool"),
                ("SkillCooldownPoolDefinition", "SkillCooldownPool"),
                ("MeleeSkillCooldownPoolDefinition", "MeleeSkillCooldownPool"),
            ] {
                match required(vm, pc, def, unit)? {
                    Value::Object(Some(d)) => plan.push((d, Some(field))),
                    Value::Object(None) => {}
                    other => {
                        return Err(super::access::error(format!(
                            "{unit}: the PlayerClass's {def} holds {}, not an object",
                            other.kind_name()
                        )));
                    }
                }
            }
        }
        let own = vm.space.heap.get(controller)?.class;
        let state = vm.space.heap.get(controller)?.state;
        if vm.find_method(own, "OnPlayerClassChange", state)?.is_none() {
            return Err(super::access::error(format!("{unit}: the controller has no OnPlayerClassChange")));
        }
        event = true;
    }
    // Every pool the call will make, before anything is deleted: one resource twice is not read.
    let mut resources = Vec::new();
    let mut types = Vec::new();
    for &(def, field) in &plan {
        let def = super::access::loaded(vm, def)?;
        let resource = object(vm, def, "Resource")?;
        if resource.is_some() && resources.contains(&resource) {
            return Err(NotYet::not_read(
                "ApplyCharacterClassStartingValues for pools that name one resource twice (CreatePool's reinitialise flag is not given)",
                RECORD_CLASS_STARTING,
            )
            .into());
        }
        resources.push(resource);
        types.push(match field {
            Some(field) => Some(reference_type(vm, controller, field, unit)?),
            None => None,
        });
    }
    let mut manager = object(vm, controller, "ResourcePoolManager")?;
    ow_world::pools::delete_all_pools(vm, manager)?;
    for ((def, field), ty) in plan.into_iter().zip(types) {
        let made = ow_world::pools::create_pool(vm, manager, Some(controller), Some(def), false)?;
        manager = made.manager;
        set_required(vm, controller, "ResourcePoolManager", Value::Object(manager), unit)?;
        if let (Some(field), Some(ty)) = (field, ty) {
            let v = ow_world::pools::ref_value(&vm.space, ty, made.reference)?;
            set_required(vm, controller, field, v, unit)?;
        }
    }
    if event {
        super::call_method(vm, controller, "OnPlayerClassChange", vec![])?;
    }
    Ok(())
}

/// Whether `controller`'s `Role` is `ROLE_Authority`; a controller without a byte `Role` is bad data.
fn is_authority_strict(vm: &mut Vm, controller: ObjRef, unit: &str) -> Result<bool, VmError> {
    let authority = super::with_state(vm, |s| s.authority)?;
    match required(vm, controller, "Role", unit)? {
        Value::Byte(r) => Ok(i64::from(r) == i64::from(authority)),
        other => Err(super::access::error(format!("{unit}: Role holds {}, not a byte", other.kind_name()))),
    }
}

/// The struct type of the controller's pool reference field `field`.
fn reference_type(vm: &Vm, controller: ObjRef, field: &str, unit: &str) -> Result<ow_object::ids::StructId, VmError> {
    let class_id = vm.space.heap.get(controller)?.class;
    vm.space
        .prop(class_id, field)
        .and_then(|p| vm.space.classes.decl(p))
        .and_then(|d| match d.kind {
            PropKind::Struct(t) => Some(t),
            _ => None,
        })
        .ok_or_else(|| super::access::error(format!("{unit}: the controller has no struct property {field}")))
}

/// The record of the route natives (decision 2026-11-19-32).
pub const RECORD_ROUTE: &str = "NATIVE_MIND_ROUTE.md";

/// `WillowPawn.SetDefaultPhysics()` (NATIVE_MIND_ROUTE.md section 1), Off. The gates first. Nothing changes when:
/// - the pawn is gone (`bDeleteMe` or `bIsDead`);
/// - it has no `SMComponent`;
/// - that component's `CurrentSMData.SMD` is a `WillowGame.SpecialMove_Spawned` (a pawn in its spawn move keeps its
///   physics); with a special move set, a world whose classes lack `SpecialMove_Spawned` is an error.
///
/// The gate on the pawn's `ObjectArchetype` being a `Pawn` always passes for a pawn here: its archetype is its template
/// or its class default object, both pawns (UNVERIFIED; the port does not model `ObjectArchetype`).
///
/// Past the gates the native sets the default physics, copies `bCollideWorld` and, unless two pawn queries both answer
/// true, the collision flags from the archetype. Those two queries are not named (the note's "Not read"), and an AI
/// pawn's default physics also hangs on an unnamed navigation-handle query in its last case. No branch is picked: this
/// is `NotYet { NotRead }`, decided before anything changes. UNVERIFIED in the running game.
pub fn set_default_physics(vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
    let flag = |vm: &mut Vm, name: &str| -> Result<bool, VmError> {
        Ok(matches!(get(vm, pawn, name)?, Some(Value::Bool(true))))
    };
    if flag(vm, "bDeleteMe")? || flag(vm, "bIsDead")? {
        return Ok(());
    }
    let Some(component) = object(vm, pawn, "SMComponent")? else {
        return Ok(());
    };
    let current = get(vm, component, "CurrentSMData")?;
    let smd = current
        .as_ref()
        .and_then(|v| ow_combat_field(vm, v, "SMD"))
        .and_then(|v| object_of(&v));
    if let Some(smd) = smd {
        // `SpecialMove_Spawned` is a `WillowGame` class (research/native_by_class.json). A failed lookup is an error,
        // never a gate that silently passes (review r1 of #418).
        let spawned = vm.find_class("WillowGame.SpecialMove_Spawned")?;
        let own = vm.space.heap.get(smd)?.class;
        if vm.space.classes.is_child_of(own, spawned) {
            return Ok(());
        }
    }
    Err(NotYet::not_read(
        "WillowPawn.SetDefaultPhysics past its gates: the two pawn queries that decide whether the collision flags are \
         copied, and the navigation-handle query of an AI pawn's default physics, are not named (NATIVE_MIND_ROUTE.md \
         section 1, not read)",
        RECORD_ROUTE,
    )
    .into())
}

/// A field of a struct value by name.
fn ow_combat_field(vm: &Vm, v: &Value, name: &str) -> Option<Value> {
    let s = v.as_struct()?;
    let id = vm.space.names.find(name)?;
    let (index, _) = vm.space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(usize::try_from(index).ok()?).cloned()
}

/// `GetExpLevel()` on a pawn (NATIVE_MIND_ROUTE.md section 2), Off: on a `WillowAIPawn`, the pawn's own `IBalancedActor`
/// answer, its `ExpLevel` int as stored. Another pawn's `IBalancedActor` answer is not read: `NotYet { NotRead }`.
/// UNVERIFIED in the running game.
pub fn get_exp_level(vm: &mut Vm, pawn: ObjRef) -> Result<Value, VmError> {
    let ai = vm.find_class("WillowGame.WillowAIPawn")?;
    let own = vm.space.heap.get(pawn)?.class;
    if !vm.space.classes.is_child_of(own, ai) {
        return Err(NotYet::not_read(
            "GetExpLevel on a pawn that is not a WillowAIPawn (its IBalancedActor answer is not read)",
            RECORD_ROUTE,
        )
        .into());
    }
    match get(vm, pawn, "ExpLevel")? {
        Some(v @ Value::Int(_)) => Ok(v),
        _ => Err(super::access::error("WillowAIPawn.GetExpLevel: the pawn has no int ExpLevel")),
    }
}

#[cfg(test)]
mod tests;
