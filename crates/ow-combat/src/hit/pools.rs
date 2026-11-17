//! Four Off resource-pool rules from NATIVE_RESOURCE_POOL_IDLE.md, implemented from its own-words note only.
//! AI-assisted (Codex/GPT-6.1 Sol). All native/gameplay behavior remains UNVERIFIED in the running game.
//! Reference retains its existing bindings and stubs. No regeneration is implemented here.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::access::{required, set_required};
use super::{bind_fn, this};

pub const RECORD: &str = "NATIVE_RESOURCE_POOL_IDLE.md";

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

#[cfg(test)]
mod tests;
