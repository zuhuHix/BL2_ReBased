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
        // A class argument that is not an object reference is refused: reading it as None would delete every pool.
        let class = match c.arg(0)? {
            Value::Object(o) => *o,
            other => {
                return Err(super::access::error(format!(
                    "Controller.ApplyCharacterClassStartingValues: the class argument is not an object reference ({})",
                    other.kind_name()
                )));
            }
        };
        apply_class_starting_values(c.vm, controller, class)?;
        Ok(Value::None)
    })?;
    // The AI mind's class route reaches two natives no note has read yet (NATIVE_COMBAT_GROUNDWORK.md section 1.11):
    // they answer `NotYet { NotRead }` by name instead of the stub policy's refusal.
    for (path, what) in [
        ("WillowGame.WillowPawn.SetDefaultPhysics", "WillowPawn.SetDefaultPhysics (not read)"),
        ("WillowGame.WillowAIPawn.GetExpLevel", "WillowAIPawn.GetExpLevel (not read)"),
    ] {
        bind_fn(vm, path, move |_| Err(NotYet::not_read(what, RECORD_STARTING).into()))?;
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

/// `Controller.ApplyCharacterClassStartingValues(inCharacterClass)`, the `Controller` base (NATIVE_COMBAT_GROUNDWORK.md
/// section 1.10 step 1, and section 1.11: `WillowMind` has no override). Every pool of the controller's
/// `ResourcePoolManager` is deleted; then, with a class, one pool per entry of its `ResourcePools` in array order, then
/// `AccuracyPoolDefinition` into `AccuracyPool` and `OffHandAccuracyPoolDefinition` into `OffHandAccuracyPool`, each only
/// when set. The pools are made by `CreatePool` with the controller as owner. A manager that call spawns is written
/// back to `ResourcePoolManager`.
///
/// Refused, `NotYet`:
/// - a `WillowPlayerController` (or a subclass): its override ends with a script function whose name is not read
///   (section 1.10 step 2);
/// - a class whose pools (its `ResourcePools` and the two accuracy pool definitions together) name one resource twice.
///   The note does not give `CreatePool`'s reinitialise flag. After the delete no pool exists, so the flag can matter
///   only for a second pool of the same resource; that case is refused before anything changes.
///
/// A class argument that is not an object reference is a script error, not "no class" (review r1 of #412).
///
/// UNVERIFIED in the running game.
pub fn apply_class_starting_values(vm: &mut Vm, controller: ObjRef, class: Option<ObjRef>) -> Result<(), VmError> {
    let unit = "Controller.ApplyCharacterClassStartingValues";
    if let Ok(player) = vm.find_class("WillowGame.WillowPlayerController") {
        let own = vm.space.heap.get(controller)?.class;
        if vm.space.classes.is_child_of(own, player) {
            return Err(NotYet::not_read(
                "ApplyCharacterClassStartingValues on a WillowPlayerController (its override's last script call is not named)",
                RECORD_STARTING,
            )
            .into());
        }
    }
    let mut defs: Vec<Option<ObjRef>> = Vec::new();
    let mut accuracy = (None, None);
    if let Some(class) = class {
        let class = super::access::loaded(vm, class)?;
        if let Some(list) = get(vm, class, "ResourcePools")? {
            for v in list.elements().unwrap_or(&[]) {
                defs.push(object_of(v));
            }
        }
        accuracy = (
            object(vm, class, "AccuracyPoolDefinition")?,
            object(vm, class, "OffHandAccuracyPoolDefinition")?,
        );
        // Every pool the call will make, the two accuracy pools included, before anything is deleted.
        let mut resources = Vec::new();
        for def in defs.iter().flatten().chain([accuracy.0, accuracy.1].iter().flatten()) {
            let def = super::access::loaded(vm, *def)?;
            let resource = object(vm, def, "Resource")?;
            if resource.is_some() && resources.contains(&resource) {
                return Err(NotYet::not_read(
                    "ApplyCharacterClassStartingValues for a class listing one resource twice (CreatePool's reinitialise flag is not given)",
                    RECORD_STARTING,
                )
                .into());
            }
            resources.push(resource);
        }
    }
    let mut manager = object(vm, controller, "ResourcePoolManager")?;
    ow_world::pools::delete_all_pools(vm, manager)?;
    if class.is_none() {
        return Ok(());
    }
    for def in defs.into_iter().flatten() {
        let made = ow_world::pools::create_pool(vm, manager, Some(controller), Some(def), false)?;
        manager = made.manager;
        set_required(vm, controller, "ResourcePoolManager", Value::Object(manager), unit)?;
    }
    for (def, field) in [(accuracy.0, "AccuracyPool"), (accuracy.1, "OffHandAccuracyPool")] {
        let Some(def) = def else { continue };
        let made = ow_world::pools::create_pool(vm, manager, Some(controller), Some(def), false)?;
        manager = made.manager;
        set_required(vm, controller, "ResourcePoolManager", Value::Object(manager), unit)?;
        let class_id = vm.space.heap.get(controller)?.class;
        let ty = vm
            .space
            .prop(class_id, field)
            .and_then(|p| vm.space.classes.decl(p))
            .and_then(|d| match d.kind {
                PropKind::Struct(t) => Some(t),
                _ => None,
            })
            .ok_or_else(|| super::access::error(format!("{unit}: the controller has no struct property {field}")))?;
        let v = ow_world::pools::ref_value(&vm.space, ty, made.reference)?;
        set_required(vm, controller, field, v, unit)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
