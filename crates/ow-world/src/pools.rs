//! Owner: lane W5-POOLS. Resource pools: the pool natives, the pool manager natives, pool references, the pawn's
//! health glue, and the world's answer to [`EngineQuery::ResourcePool`]. AI-assisted (Claude Code).
//!
//! Every rule comes from the records, read from the executable and **UNVERIFIED in the running game** unless a line
//! says otherwise: NATIVE_COMBAT_GROUNDWORK.md sections 1 and 2 (references, the manager, pawn health),
//! NATIVE_LEVEL_UP_ATTRIBUTES.md section 4 (`GetMaxValue`, `SetCurrentValue`, `UpdateLastValues`, `PoolIsNowFull`)
//! and NATIVE_ATTRIBUTES.md section 8 (`ClearAttributeModifierStacks`). The C++ reference is `src/combat.cpp` (the
//! pool natives, `CreatePool`, `DeletePoolByReference`, the pawn natives) and `src/mission_script.cpp` (the rebase).
//! The pool's own script (`InitializeAuthoritative`, `CalculateBaseValues`, `ApplyUpgrades`,
//! `UpdateCurrentValueOnExtremaChange`, `Reinitialize`, `ShutDown`) is the installed one and runs on the VM.
//!
//! **What the C++ does not have and this module adds**, each from the record: `DeletePoolAtIndex`, `DeleteAllPools`,
//! `DeletePool`, `CreateResourcePoolManagerIfNecessary`, both `GetResourcePoolForResourceDefinition` forms,
//! `HasResourcePoolForResourceDefinition`, `InternalCreatePoolAtIndex`, `GetCurrentValue`,
//! `ClearAttributeModifierStacks`, and the manager's resource-to-slot map ([`PoolMaps`]). The map is a VM service
//! rather than the manager's `ResourcePoolMap` property, because the object model holds a map property as
//! `Value::None` (decision 2026-11-15-06). Creation records an entry, deletion removes it, lookups read it.
//!
//! **Off timer dispatch:** `ResetIdleTimer` delegates to [`ResetIdleTimerOff`], installed by ow-combat from
//! NATIVE_RESOURCE_POOL_IDLE.md. Without that handler it refuses; Reference keeps the C++ stamp unchanged.
//! The "current value changed" hook `SetCurrentValue` ends with is taken as empty only for the classes of [`KNOWN_HOOK_CLASSES`]
//! (an allow-list), so a pool of any other class is `NotYet` before anything is written (the C++ skips every hook).
//!
//! **Creation refusal in Off mode:** an unsupported idle hook can refuse inside `InitializeAuthoritative`.
//! The refusal comes from inside the pool's
//! script, so it is not decided before anything changes. What the natives changed is undone: [`create_pool_at_index`]
//! empties the slot again, [`create_pool`] restores `NextPoolGUID` and destroys a manager it spawned, and
//! [`controller_pool`] restores the controller's `ResourcePoolManager`; no map entry is made. The new pool object is
//! left unreferenced, and what the script itself wrote before the refusal is **not** undone.
//!
//! Always `NotYet`: a `CreatePoolAtIndex` without a manager, a vehicle's pool provider in the controller lookup, a
//! `DeletePool` whose manager is not the reference's, and every resource owner resolver but the controller one in the
//! engine query.
//!
//! **Identity.** Two definitions or resources are "the same object" when they are the same `ObjRef`: the definition
//! store gives one object per export (decision 2026-11-15-16), where the C++ compares loaded copies by path.

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::api::{AttrError, EngineQuery, WorldParts};
use ow_core::{DetMap, NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use crate::actors::{
    self, Actors, ROLE_AUTHORITY, SpawnRequest, arg_bool, arg_object, byte, field, flag, float,
    object, prop_of, set_field,
};
use crate::query::{QueryFn, QueryKind, WorldQuery};

/// The record of the references, the manager and pawn health.
pub const RECORD: &str = "NATIVE_COMBAT_GROUNDWORK.md sections 1-2";
/// The record of the pool natives.
pub const RECORD_POOL: &str = "NATIVE_LEVEL_UP_ATTRIBUTES.md section 4";

/// Off's timer handler is owned by ow-combat. Keeping this seam here preserves world's Reference binding and
/// avoids a dependency from the world to combat. No callback runs while its service cell is borrowed.
#[derive(Debug, Clone, Copy)]
pub struct ResetIdleTimerOff(pub fn(&mut Vm, ObjRef) -> Result<(), VmError>);

/// `ResourcePoolManager.ResourcePools[16]` (NATIVE_COMBAT_GROUNDWORK.md section 1.1).
pub const POOL_SLOTS: u8 = 16;
/// The slot and GUID bytes of the invalid reference (section 1.1).
pub const INVALID_BYTE: u8 = 255;
/// `UpdateLastValues` clears "created and not modified" when current moved by more than this (record section 4).
pub const LAST_VALUE_EPSILON: f32 = 0.0001;
/// `Pawn.SetHealth`: a value from 0 up to (not including) this becomes exactly 0 (section 2).
pub const HEALTH_FLOOR: f32 = 1.0;

/// The manager class.
pub const MANAGER_CLASS: &str = "Engine.ResourcePoolManager";

/// The pool classes (exact class names) whose "current value changed" hook `SetCurrentValue` takes as empty; every
/// other class is `NotYet` in Off mode (an allow-list). `ResourcePool`: read, empty (NATIVE_LEVEL_UP_ATTRIBUTES.md
/// section 4). `HealthResourcePool` (the installed health pool's class) and `ShieldResourcePool` (the shield pool's):
/// **UNVERIFIED, fitted**. The record names no hook for them, and "no hook" is inferred from that silence (decision
/// 2026-11-15-29). The record names the experience and ammo pools' hooks as unread, so those two are refused.
pub const KNOWN_HOOK_CLASSES: [&str; 3] =
    ["ResourcePool", "HealthResourcePool", "ShieldResourcePool"];

/// The attributes `ClearAttributeModifierStacks` empties (NATIVE_ATTRIBUTES.md section 8).
pub const CLEARED_STACKS: [&str; 7] = [
    "MinValue",
    "MaxValue",
    "ConsumptionRate",
    "ActiveRegenerationRate",
    "OnIdleRegenerationRate",
    "OnIdleRegenerationDelay",
    "PassiveRegenerationRate",
];

// ---------------------------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------------------------

fn rule_error(rule: &str, message: String) -> VmError {
    VmError::Script {
        function: rule.to_string(),
        message,
    }
}

/// Whether a class in `o`'s class chain (its own included) is named `name`, any case. Classes are matched by name,
/// not package, as `ow_attr::effects::owner_notification` does.
pub(crate) fn is_a(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<bool, VmError> {
    let mut cursor = Some(space.heap.get(o)?.class);
    while let Some(c) = cursor {
        let Some(layout) = space.classes.get_layout(c) else {
            break;
        };
        if layout.path().name().eq_ignore_ascii_case(name) {
            return Ok(true);
        }
        cursor = layout.super_class();
    }
    Ok(false)
}

/// The object path of `o` (its outer chain, outermost first, joined by dots), for messages. The walk stops after 64
/// outers, a Rust-only bound against an outer cycle.
pub(crate) fn object_path(space: &ObjectSpace, o: ObjRef) -> String {
    let mut parts = Vec::new();
    let mut cursor = Some(o);
    while let Some(c) = cursor {
        if parts.len() == 64 {
            break;
        }
        match space.heap.get(c) {
            Ok(x) => {
                parts.push(space.names.text(x.name).into_owned());
                cursor = x.outer;
            }
            Err(_) => break,
        }
    }
    parts.reverse();
    parts.join(".")
}

/// Loads `o` when it is one of the definition store's unloaded handles.
pub(crate) fn loaded(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        let done = vm.defs.ensure_loaded(&mut vm.space, o);
        vm.log.extend(vm.defs.take_log());
        done.map_err(VmError::Def)?;
    }
    Ok(())
}

/// `o.name` as an object, after loading `o`.
fn data_object(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    loaded(vm, o)?;
    object(&vm.space, o, name)
}

fn is_authority(space: &ObjectSpace, actor: ObjRef) -> Result<bool, VmError> {
    Ok(byte(space, actor, "Role")? == ROLE_AUTHORITY)
}

/// The world time a pool stamps: `WorldInfo.TimeSeconds`, or 0 with no world (record section 4).
fn world_time(vm: &Vm) -> Result<f32, VmError> {
    let wi = match vm.service::<Actors>() {
        Some(a) => a
            .try_borrow()
            .map_err(|_| VmError::ServiceBorrowed("Actors"))?
            .world_info(),
        None => None,
    };
    match wi {
        Some(wi) => float(&vm.space, wi, "TimeSeconds"),
        None => Ok(0.0),
    }
}

fn this(c: &NativeCall<'_>, native: &str) -> Result<ObjRef, VmError> {
    c.this
        .ok_or_else(|| rule_error(native, "called without an object".to_string()))
}

fn bad(native: &str, index: usize, reason: &'static str) -> VmError {
    VmError::BadArgument {
        function: native.to_string(),
        index,
        reason,
    }
}

fn arg_float(c: &NativeCall<'_>, native: &str, i: usize) -> Result<f32, VmError> {
    match c.arg(i)? {
        Value::Float(f) => Ok(*f),
        _ => Err(bad(native, i, "expected a float")),
    }
}

fn arg_int(c: &NativeCall<'_>, native: &str, i: usize) -> Result<i32, VmError> {
    match c.arg(i)? {
        Value::Int(n) => Ok(*n),
        _ => Err(bad(native, i, "expected an int")),
    }
}

fn arg_byte(c: &NativeCall<'_>, native: &str, i: usize) -> Result<u8, VmError> {
    match c.arg(i)? {
        Value::Byte(b) => Ok(*b),
        _ => Err(bad(native, i, "expected a byte")),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Pool references (section 1.1, 1.2)
// ---------------------------------------------------------------------------------------------------------------

/// A `ResourcePoolReference` as plain fields: the manager, the slot, the generation byte and the cached pool.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolRef {
    pub manager: Option<ObjRef>,
    pub index: u8,
    pub guid: u8,
    pub data: Option<ObjRef>,
}

impl PoolRef {
    /// The invalid reference: no manager, index 255, GUID 255, no cached pool.
    pub const INVALID: PoolRef = PoolRef {
        manager: None,
        index: INVALID_BYTE,
        guid: INVALID_BYTE,
        data: None,
    };

    pub fn new(manager: Option<ObjRef>, index: u8, guid: u8, data: Option<ObjRef>) -> Self {
        PoolRef {
            manager,
            index,
            guid,
            data,
        }
    }
}

const REF_FIELDS: [&str; 4] = ["PoolManager", "PoolIndexInManager", "PoolGUID", "Data"];

fn ref_field_index(space: &ObjectSpace, ty: StructId, name: &str) -> Result<usize, VmError> {
    let missing = || {
        rule_error(
            "ResourcePoolReference",
            format!("the pool reference struct has no field {name} ({RECORD})"),
        )
    };
    let n = space.names.find(name).ok_or_else(missing)?;
    let layout = space.classes.struct_layout(ty).ok_or_else(missing)?;
    let (entry, _) = layout.field(n).ok_or_else(missing)?;
    usize::try_from(entry).map_err(|_| missing())
}

/// Reads a reference struct value.
pub fn read_ref(space: &ObjectSpace, v: &Value) -> Result<PoolRef, VmError> {
    let s = v.as_struct().ok_or_else(|| {
        rule_error(
            "ResourcePoolReference",
            format!("expected a pool reference struct, got {}", v.kind_name()),
        )
    })?;
    let mut parts = [Value::None, Value::None, Value::None, Value::None];
    for (slot, name) in parts.iter_mut().zip(REF_FIELDS) {
        let i = ref_field_index(space, s.ty, name)?;
        *slot = s.fields.get(i).cloned().ok_or_else(|| {
            rule_error(
                "ResourcePoolReference",
                format!("the pool reference value lacks field {name}"),
            )
        })?;
    }
    let obj = |v: &Value, name: &str| match v {
        Value::Object(o) => Ok(*o),
        other => Err(rule_error(
            "ResourcePoolReference",
            format!("{name} holds {}, expected an object", other.kind_name()),
        )),
    };
    let byte_of = |v: &Value, name: &str| match v {
        Value::Byte(b) => Ok(*b),
        other => Err(rule_error(
            "ResourcePoolReference",
            format!("{name} holds {}, expected a byte", other.kind_name()),
        )),
    };
    Ok(PoolRef {
        manager: obj(&parts[0], REF_FIELDS[0])?,
        index: byte_of(&parts[1], REF_FIELDS[1])?,
        guid: byte_of(&parts[2], REF_FIELDS[2])?,
        data: obj(&parts[3], REF_FIELDS[3])?,
    })
}

/// A reference struct value of type `ty` holding `r` (the struct's other fields at their defaults).
pub fn ref_value(space: &ObjectSpace, ty: StructId, r: PoolRef) -> Result<Value, VmError> {
    let mut v = space
        .classes
        .struct_defaults(ty)
        .map_err(|e| VmError::from(ow_object::heap::HeapError::Layout(e)))?;
    let values = [
        Value::Object(r.manager),
        Value::Byte(r.index),
        Value::Byte(r.guid),
        Value::Object(r.data),
    ];
    let mut indices = [0usize; 4];
    for (slot, name) in indices.iter_mut().zip(REF_FIELDS) {
        *slot = ref_field_index(space, ty, name)?;
    }
    let s = v.struct_mut().ok_or_else(|| {
        rule_error(
            "ResourcePoolReference",
            "the reference type's default is not a struct".to_string(),
        )
    })?;
    for (i, value) in indices.into_iter().zip(values) {
        let f = s.fields.get_mut(i).ok_or_else(|| {
            rule_error(
                "ResourcePoolReference",
                "the reference type's default lacks a field".to_string(),
            )
        })?;
        *f = value;
    }
    Ok(v)
}

/// The struct type a declaration holds (a reference parameter, result or property).
fn struct_of(kind: &PropKind, what: &str) -> Result<StructId, VmError> {
    match kind {
        PropKind::Struct(s) => Ok(*s),
        other => Err(rule_error(
            "ResourcePoolReference",
            format!(
                "{what} is a {}, not a pool reference struct",
                other.kind_name()
            ),
        )),
    }
}

/// The reference struct type of the running native's result.
fn result_struct(c: &NativeCall<'_>, native: &str) -> Result<StructId, VmError> {
    let decl =
        c.vm.function(c.function)
            .and_then(|d| d.result.clone())
            .ok_or_else(|| rule_error(native, "declared without a result".to_string()))?;
    struct_of(&decl.kind, native)
}

/// The reference struct type of property `name` of `o`.
fn prop_struct(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<StructId, VmError> {
    let p = prop_of(space, o, name)?;
    let decl = space
        .classes
        .decl(p)
        .ok_or_else(|| rule_error(name, "a property without a declaration".to_string()))?;
    struct_of(&decl.kind, name)
}

// ---------------------------------------------------------------------------------------------------------------
// The manager's slots and map
// ---------------------------------------------------------------------------------------------------------------

/// The pool in slot `index` (below [`POOL_SLOTS`]) of `manager`. The class must declare `ResourcePools` with 16
/// elements, as the game's does.
pub fn slot(space: &ObjectSpace, manager: ObjRef, index: u8) -> Result<Option<ObjRef>, VmError> {
    let p = prop_of(space, manager, "ResourcePools")?;
    let dim = space.classes.decl(p).map(|d| d.array_dim).unwrap_or(0);
    if dim != i32::from(POOL_SLOTS) {
        return Err(rule_error(
            "ResourcePools",
            format!(
                "{}.ResourcePools has {dim} elements, the record's manager has {POOL_SLOTS} ({RECORD})",
                ow_attr::eval::class_path(space, manager)
            ),
        ));
    }
    if index >= POOL_SLOTS {
        return Ok(None);
    }
    match space.read(manager, PropAddr::element(p, u32::from(index)))? {
        Value::Object(o) => Ok(*o),
        other => Err(rule_error(
            "ResourcePools",
            format!("a slot holds {}, expected an object", other.kind_name()),
        )),
    }
}

fn set_slot(
    space: &mut ObjectSpace,
    manager: ObjRef,
    index: u8,
    pool: Option<ObjRef>,
) -> Result<(), VmError> {
    let p = prop_of(space, manager, "ResourcePools")?;
    space.write(
        manager,
        PropAddr::element(p, u32::from(index)),
        Value::Object(pool),
    )?;
    Ok(())
}

/// Every pool of `manager`, in slot order, empty slots skipped.
pub fn pools_of(space: &ObjectSpace, manager: ObjRef) -> Result<Vec<(u8, ObjRef)>, VmError> {
    let mut out = Vec::new();
    for i in 0..POOL_SLOTS {
        if let Some(p) = slot(space, manager, i)? {
            out.push((i, p));
        }
    }
    Ok(out)
}

/// The manager's resource-to-slot map (`ResourcePoolMap`, module documentation). A VM service.
#[derive(Debug, Default)]
pub struct PoolMaps {
    maps: DetMap<ObjRef, DetMap<ObjRef, u8>>,
}

impl PoolMaps {
    pub fn new() -> Self {
        Self::default()
    }

    /// The slot `manager`'s map gives `resource`.
    pub fn slot(&self, manager: ObjRef, resource: ObjRef) -> Option<u8> {
        self.maps
            .get(&manager)
            .and_then(|m| m.get(&resource).copied())
    }

    fn set(&mut self, manager: ObjRef, resource: ObjRef, index: u8) {
        self.maps
            .entry(manager)
            .or_default()
            .insert(resource, index);
    }

    fn remove(&mut self, manager: ObjRef, resource: ObjRef) {
        if let Some(m) = self.maps.get_mut(&manager) {
            m.remove(&resource);
        }
    }
}

fn maps_of(vm: &Vm) -> Result<Rc<RefCell<PoolMaps>>, VmError> {
    vm.service::<PoolMaps>().ok_or_else(|| {
        NotYet::layer_absent(
            "the pool managers' resource map",
            "ow-world pools (pools::Pack, registered by World::new)",
            RECORD,
        )
        .into()
    })
}

fn map_slot(vm: &Vm, manager: ObjRef, resource: ObjRef) -> Result<Option<u8>, VmError> {
    let maps = maps_of(vm)?;
    let m = maps
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("PoolMaps"))?;
    Ok(m.slot(manager, resource))
}

fn map_edit(vm: &Vm, f: impl FnOnce(&mut PoolMaps)) -> Result<(), VmError> {
    let maps = maps_of(vm)?;
    let mut m = maps
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("PoolMaps"))?;
    f(&mut m);
    Ok(())
}

/// Section 1.2: `r` is valid exactly when its manager's slot holds a pool whose `PoolGUID` equals the reference's.
/// Returns the reference with its cache set (valid) or cleared (not valid), and the pool.
pub fn resolve(space: &ObjectSpace, r: PoolRef) -> Result<(PoolRef, Option<ObjRef>), VmError> {
    let pool = match r.manager {
        Some(m) if r.index < POOL_SLOTS => match slot(space, m, r.index)? {
            Some(p) if byte(space, p, "PoolGUID")? == r.guid => Some(p),
            _ => None,
        },
        _ => None,
    };
    Ok((PoolRef { data: pool, ..r }, pool))
}

/// The manager's GUID counter: the value before the increment, which wraps as a byte (section 1.3).
fn next_guid(space: &mut ObjectSpace, manager: ObjRef) -> Result<u8, VmError> {
    let guid = byte(space, manager, "NextPoolGUID")?;
    set_field(
        space,
        manager,
        "NextPoolGUID",
        Value::Byte(guid.wrapping_add(1)),
    )?;
    Ok(guid)
}

// ---------------------------------------------------------------------------------------------------------------
// The pool natives (NATIVE_LEVEL_UP_ATTRIBUTES.md section 4)
// ---------------------------------------------------------------------------------------------------------------

/// Whether the pool's resource only takes whole numbers (`ResourceDefinition.bIntegerOnlyUpdates`); false with no
/// definition or resource, as the C++.
fn integer_only(vm: &mut Vm, pool: ObjRef) -> Result<bool, VmError> {
    let Some(def) = object(&vm.space, pool, "Definition")? else {
        return Ok(false);
    };
    let Some(resource) = data_object(vm, def, "Resource")? else {
        return Ok(false);
    };
    loaded(vm, resource)?;
    flag(&vm.space, resource, "bIntegerOnlyUpdates")
}

/// What [`set_current`] would store for `value`, without writing anything.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stored {
    /// The new `CurrentValue`.
    pub current: f32,
    /// The new `RateRemainder` of an integer-only resource; `None` for any other (the field is not written).
    pub remainder: Option<f32>,
    /// The minimum the clamp used.
    pub minimum: f32,
    /// The maximum the clamp used (truncated toward zero for an integer-only resource).
    pub maximum: f32,
}

/// The value rule of [`set_current`], shared so a caller can test the stored value before it writes (seam for
/// W6-SKILLS' cooldown drain, review r1 of PR #342): `None` when the pool is not authoritative (nothing is stored);
/// otherwise an integer-only resource truncates the value and the maximum toward zero and keeps the value's fraction
/// as the remainder, and current is the value clamped to minimum..maximum, the minimum winning when it exceeds the
/// maximum (the C++ comparison form, so a NaN value gives the minimum).
pub fn stored_value(vm: &mut Vm, pool: ObjRef, value: f32) -> Result<Option<Stored>, VmError> {
    if !flag(&vm.space, pool, "bIsAuthoritative")? {
        return Ok(None);
    }
    let mut maximum = float(&vm.space, pool, "MaxValue")?;
    let minimum = float(&vm.space, pool, "MinValue")?;
    let mut value = value;
    let mut remainder = None;
    if integer_only(vm, pool)? {
        maximum = maximum.trunc();
        let whole = value.trunc();
        remainder = Some(value - whole);
        value = whole;
    }
    // The C++ form `std::max(minimum, std::min(value, maximum))`, written out so a NaN value gives the minimum as it
    // does there (`f32::min`/`max` would drop the NaN and give the maximum).
    let low = if maximum < value { maximum } else { value };
    let current = if minimum < low { low } else { minimum };
    Ok(Some(Stored {
        current,
        remainder,
        minimum,
        maximum,
    }))
}

/// `ResourcePool.SetCurrentValue(Value)` (record section 4, UNVERIFIED): nothing unless the pool is authoritative;
/// an integer-only resource truncates the value and the maximum toward zero and keeps the value's fraction in
/// `RateRemainder`; current becomes the value clamped to minimum..maximum, the minimum winning when it exceeds the
/// maximum ([`stored_value`]); `PoolIdleDelayStartTime` becomes the world time; then the "current value changed" hook
/// (module documentation: `NotYet` for a subclass in Off mode, checked before anything is written).
pub fn set_current(vm: &mut Vm, pool: ObjRef, value: f32) -> Result<(), VmError> {
    if !flag(&vm.space, pool, "bIsAuthoritative")? {
        return Ok(());
    }
    current_hook_refusal(vm, pool)?;
    let Some(stored) = stored_value(vm, pool, value)? else {
        return Ok(());
    };
    if let Some(r) = stored.remainder {
        set_field(&mut vm.space, pool, "RateRemainder", Value::Float(r))?;
    }
    set_field(
        &mut vm.space,
        pool,
        "CurrentValue",
        Value::Float(stored.current),
    )?;
    let t = world_time(vm)?;
    set_field(
        &mut vm.space,
        pool,
        "PoolIdleDelayStartTime",
        Value::Float(t),
    )?;
    Ok(())
}

/// Off mode: `NotYet` for an authoritative pool whose class is not one of the [`KNOWN_HOOK_CLASSES`] (the hook
/// `SetCurrentValue` ends with). Reference mode skips every hook, as the C++.
fn current_hook_refusal(vm: &Vm, pool: ObjRef) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off || !flag(&vm.space, pool, "bIsAuthoritative")? {
        return Ok(());
    }
    let class = vm.space.heap.get(pool)?.class;
    let name = vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.path().name().to_string())
        .unwrap_or_default();
    if KNOWN_HOOK_CLASSES
        .iter()
        .any(|k| k.eq_ignore_ascii_case(&name))
    {
        return Ok(());
    }
    Err(NotYet::not_read(
        format!("the \"current value changed\" hook of pool class {name} (not read)"),
        RECORD_POOL,
    )
    .into())
}

/// `ResourcePool.UpdateLastValues()` (record section 4): clears `bCreatedAndNotModified` when current moved from the
/// recorded current by more than 0.0001, then records current, minimum and maximum.
pub fn update_last_values(space: &mut ObjectSpace, pool: ObjRef) -> Result<(), VmError> {
    let current = float(space, pool, "CurrentValue")?;
    if (current - float(space, pool, "LastCurrentValue")?).abs() > LAST_VALUE_EPSILON {
        set_field(space, pool, "bCreatedAndNotModified", Value::Bool(false))?;
    }
    let (min, max) = (
        float(space, pool, "MinValue")?,
        float(space, pool, "MaxValue")?,
    );
    set_field(space, pool, "LastCurrentValue", Value::Float(current))?;
    set_field(space, pool, "LastMinValue", Value::Float(min))?;
    set_field(space, pool, "LastMaxValue", Value::Float(max))?;
    Ok(())
}

/// `ResourcePool.PoolIsNowFull()` (record section 4): sets `bHasPoolBeenFullSinceLastBeingDepleted` when the
/// minimum is below the maximum.
pub fn pool_is_now_full(space: &mut ObjectSpace, pool: ObjRef) -> Result<(), VmError> {
    if float(space, pool, "MinValue")? < float(space, pool, "MaxValue")? {
        set_field(
            space,
            pool,
            "bHasPoolBeenFullSinceLastBeingDepleted",
            Value::Bool(true),
        )?;
    }
    Ok(())
}

/// `ResourcePool.ClearAttributeModifierStacks()` (NATIVE_ATTRIBUTES.md section 8, confidence medium): empties the
/// modifier stacks of the seven [`CLEARED_STACKS`] attributes; no value is recomputed and `RegenerationDisabled` is
/// left alone.
pub fn clear_modifier_stacks(space: &mut ObjectSpace, pool: ObjRef) -> Result<(), VmError> {
    for name in CLEARED_STACKS {
        set_field(
            space,
            pool,
            &format!("{name}ModifierStack"),
            Value::array(Vec::new()),
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The manager (section 1.3 to 1.6)
// ---------------------------------------------------------------------------------------------------------------

/// Whether the world exists for a manager spawn: the actor list with a `WorldInfo`.
fn world_exists(vm: &Vm) -> Result<bool, VmError> {
    Ok(match vm.service::<Actors>() {
        Some(a) => a
            .try_borrow()
            .map_err(|_| VmError::ServiceBorrowed("Actors"))?
            .world_info()
            .is_some(),
        None => false,
    })
}

/// `CreateResourcePoolManagerIfNecessary(out Mgr, ActorOwner)` (section 1.5): `mgr` as it is when set; otherwise,
/// when `owner` has authority and a world exists, a `ResourcePoolManager` spawned with `owner` as its owner (None
/// when the spawn refuses or makes another class).
pub fn ensure_manager(
    vm: &mut Vm,
    mgr: Option<ObjRef>,
    owner: Option<ObjRef>,
) -> Result<Option<ObjRef>, VmError> {
    if mgr.is_some() {
        return Ok(mgr);
    }
    let Some(owner) = owner else { return Ok(None) };
    if !is_authority(&vm.space, owner)? || !world_exists(vm)? {
        return Ok(None);
    }
    let class = vm.find_class(MANAGER_CLASS)?;
    let made = actors::spawn(
        vm,
        owner,
        &SpawnRequest::new(Some(class)).with_owner(Some(owner)),
    )?;
    match made {
        Some(m)
            if vm
                .space
                .classes
                .is_child_of(vm.space.heap.get(m)?.class, class) =>
        {
            Ok(Some(m))
        }
        _ => Ok(None),
    }
}

/// The pool class of a definition: its resource's `DefaultResourcePoolClass`. None when the definition names no
/// resource or the resource no class, as the C++ (`poolClassFor`).
fn pool_class(vm: &mut Vm, def: ObjRef) -> Result<Option<ClassId>, VmError> {
    let Some(resource) = data_object(vm, def, "Resource")? else {
        return Ok(None);
    };
    loaded(vm, resource)?;
    match field(&vm.space, resource, "DefaultResourcePoolClass")? {
        Value::Class(c) => Ok(c),
        other => Err(rule_error(
            "DefaultResourcePoolClass",
            format!("holds {}, expected a class", other.kind_name()),
        )),
    }
}

/// `InternalCreatePoolAtIndex` (section 1.4): with a definition, an index below 16 and an empty slot, a pool of the
/// resource's pool class in the slot, its `PoolGUID` set; an authoritative manager then runs the pool's script
/// `InitializeAuthoritative(definition, manager.Owner)`, any other only sets `Definition`; the map records
/// resource -> slot. None for every refusal.
pub fn create_pool_at_index(
    vm: &mut Vm,
    manager: ObjRef,
    def: Option<ObjRef>,
    index: i32,
    guid: u8,
) -> Result<Option<ObjRef>, VmError> {
    let Some(def) = def else { return Ok(None) };
    let Ok(index) = u8::try_from(index) else {
        return Ok(None);
    };
    if index >= POOL_SLOTS || slot(&vm.space, manager, index)?.is_some() {
        return Ok(None);
    }
    let Some(class) = pool_class(vm, def)? else {
        return Ok(None);
    };
    let pool = ow_vm::interp::instantiate_from_defaults(vm, class)?;
    // Everything that can fail without changing the manager is looked up before the slot is written.
    let init = if is_authority(&vm.space, manager)? {
        let owner = object(&vm.space, manager, "Owner")?;
        Some((find_script(vm, pool, "InitializeAuthoritative")?, owner))
    } else {
        None
    };
    let resource = data_object(vm, def, "Resource")?;
    maps_of(vm)?;
    set_slot(&mut vm.space, manager, index, Some(pool))?;
    let tail = fill_new_pool(vm, manager, pool, def, index, guid, init, resource);
    if let Err(e) = tail {
        // A refusal after the slot was written (in Off mode, `ResetIdleTimer` inside the installed
        // `InitializeAuthoritative`): the slot is emptied again, so the manager is as it was (the caller restores
        // `NextPoolGUID`) and no map entry is made. The new pool object is left unreferenced, and what the script
        // itself wrote before the refusal is not undone.
        set_slot(&mut vm.space, manager, index, None)?;
        return Err(e);
    }
    Ok(Some(pool))
}

/// The tail of [`create_pool_at_index`] after the slot is written: the GUID, the start state (the script, or only the
/// definition), the map entry.
#[allow(clippy::too_many_arguments)]
fn fill_new_pool(
    vm: &mut Vm,
    manager: ObjRef,
    pool: ObjRef,
    def: ObjRef,
    index: u8,
    guid: u8,
    init: Option<(ow_object::ids::FuncId, Option<ObjRef>)>,
    resource: Option<ObjRef>,
) -> Result<(), VmError> {
    set_field(&mut vm.space, pool, "PoolGUID", Value::Byte(guid))?;
    match init {
        Some((f, owner)) => {
            actors::call_with_zeros(
                vm,
                f,
                Some(pool),
                vec![Value::Object(Some(def)), Value::Object(owner)],
            )?;
        }
        None => set_field(&mut vm.space, pool, "Definition", Value::Object(Some(def)))?,
    }
    if let Some(resource) = resource {
        map_edit(vm, |m| m.set(manager, resource, index))?;
    }
    Ok(())
}

/// The function `name` the pool's class sees; a class that has none is a typed error (the game's pool classes all
/// declare the ones the natives call).
fn find_script(vm: &mut Vm, o: ObjRef, name: &str) -> Result<ow_object::ids::FuncId, VmError> {
    let (class, state) = {
        let x = vm.space.heap.get(o)?;
        (x.class, x.state)
    };
    vm.find_method(class, name, state)?.ok_or_else(|| {
        rule_error(
            name,
            format!(
                "{} has no function {name} ({RECORD})",
                ow_attr::eval::class_path(&vm.space, o)
            ),
        )
    })
}

/// What [`create_pool`] did, for its caller and the tests.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Created {
    /// The manager after the call (made when needed); write it back to the caller's `Mgr`.
    pub manager: Option<ObjRef>,
    pub reference: PoolRef,
}

/// `ResourcePoolManager.CreatePool(out Mgr, ActorOwner, PoolDefinition, bReinitializeExistingPool)` (section 1.3):
/// the outcomes in the record's order. A full manager logs "Too many ResourcePools" (the game's on-screen debug
/// message) and gives the invalid reference.
pub fn create_pool(
    vm: &mut Vm,
    mgr: Option<ObjRef>,
    owner: Option<ObjRef>,
    def: Option<ObjRef>,
    reinitialize: bool,
) -> Result<Created, VmError> {
    let invalid = |manager| Created {
        manager,
        reference: PoolRef::INVALID,
    };
    let Some(def) = def else {
        return Ok(invalid(mgr));
    };
    let manager = ensure_manager(vm, mgr, owner)?;
    let Some(m) = manager else {
        return Ok(invalid(None));
    };
    let result = create_in(vm, m, def, reinitialize);
    if result.is_err() && mgr.is_none() {
        // A manager this call spawned does not outlive a refused creation.
        actors::destroy(vm, m)?;
    }
    result
}

/// The body of [`create_pool`] once the manager `m` exists.
fn create_in(vm: &mut Vm, m: ObjRef, def: ObjRef, reinitialize: bool) -> Result<Created, VmError> {
    let manager = Some(m);
    let invalid = |manager| Created {
        manager,
        reference: PoolRef::INVALID,
    };
    if !is_authority(&vm.space, m)? {
        return Ok(invalid(manager));
    }
    let resource = data_object(vm, def, "Resource")?;
    let (mut matched, mut empty) = (None, None);
    for i in 0..POOL_SLOTS {
        let Some(pool) = slot(&vm.space, m, i)? else {
            empty = empty.or(Some(i));
            continue;
        };
        let Some(used) = object(&vm.space, pool, "Definition")? else {
            continue;
        };
        let used_resource = data_object(vm, used, "Resource")?;
        if used == def || (resource.is_some() && used_resource == resource) {
            matched = Some((i, pool));
            break;
        }
    }
    if let Some((i, pool)) = matched {
        if !reinitialize {
            return Ok(invalid(manager));
        }
        // The pool's script Reinitialize (read as that name, medium confidence); a class without it is skipped as
        // the C++ does. The new GUID is not written into the pool, so this reference does not validate against it
        // (a quirk of the native code, medium confidence).
        let (class, state) = {
            let x = vm.space.heap.get(pool)?;
            (x.class, x.state)
        };
        if let Some(f) = vm.find_method(class, "Reinitialize", state)? {
            actors::call_with_zeros(vm, f, Some(pool), Vec::new())?;
        }
        let guid = next_guid(&mut vm.space, m)?;
        return Ok(Created {
            manager,
            reference: PoolRef::new(manager, i, guid, None),
        });
    }
    let Some(i) = empty else {
        // The game's on-screen message names the owner and the definition (section 1.3 step 7); the C++ logs the
        // definition's object path.
        let owner_path = match object(&vm.space, m, "Owner")? {
            Some(o) => object_path(&vm.space, o),
            None => "None".to_string(),
        };
        let line = format!(
            "Too many ResourcePools: {} (owner {owner_path})",
            object_path(&vm.space, def)
        );
        vm.log.push(line);
        return Ok(invalid(manager));
    };
    let guid = next_guid(&mut vm.space, m)?;
    let made = match create_pool_at_index(vm, m, Some(def), i32::from(i), guid) {
        Ok(made) => made,
        Err(e) => {
            set_field(&mut vm.space, m, "NextPoolGUID", Value::Byte(guid))?;
            return Err(e);
        }
    };
    let reference = match made {
        Some(pool) => PoolRef::new(manager, i, guid, Some(pool)),
        None => PoolRef::INVALID,
    };
    Ok(Created { manager, reference })
}

/// `DeletePoolAtIndex(Index)` (section 1.6): for a used slot below 16, the pool's resource leaves the map, the pool's
/// script `ShutDown` runs (a class without one is skipped, as the C++), and the slot is emptied. The network-update
/// mark is not modelled (no replication).
pub fn delete_pool_at_index(vm: &mut Vm, manager: ObjRef, index: i32) -> Result<(), VmError> {
    let Ok(index) = u8::try_from(index) else {
        return Ok(());
    };
    if index >= POOL_SLOTS {
        return Ok(());
    }
    let Some(pool) = slot(&vm.space, manager, index)? else {
        return Ok(());
    };
    if let Some(def) = object(&vm.space, pool, "Definition")?
        && let Some(resource) = data_object(vm, def, "Resource")?
    {
        map_edit(vm, |m| m.remove(manager, resource))?;
    }
    let (class, state) = {
        let x = vm.space.heap.get(pool)?;
        (x.class, x.state)
    };
    if let Some(f) = vm.find_method(class, "ShutDown", state)? {
        actors::call_with_zeros(vm, f, Some(pool), Vec::new())?;
    }
    set_slot(&mut vm.space, manager, index, None)
}

/// `DeleteAllPools(Mgr)` (section 1.6): with an authoritative manager, every slot 0 to 15 in order.
pub fn delete_all_pools(vm: &mut Vm, manager: Option<ObjRef>) -> Result<(), VmError> {
    let Some(m) = manager else { return Ok(()) };
    if !is_authority(&vm.space, m)? {
        return Ok(());
    }
    for i in 0..POOL_SLOTS {
        delete_pool_at_index(vm, m, i32::from(i))?;
    }
    Ok(())
}

/// The shared body of `DeletePoolByReference` and `DeletePool` (section 1.6): with an authoritative manager and a
/// reference that resolves to a pool with a definition and a resource, the slot the map gives for that resource is
/// deleted. Returns the resolved reference (its cache refreshed).
fn delete_by(vm: &mut Vm, r: PoolRef) -> Result<PoolRef, VmError> {
    let Some(m) = r.manager else { return Ok(r) };
    if !is_authority(&vm.space, m)? {
        return Ok(r);
    }
    let (r, pool) = resolve(&vm.space, r)?;
    let Some(pool) = pool else { return Ok(r) };
    let Some(def) = object(&vm.space, pool, "Definition")? else {
        return Ok(r);
    };
    let Some(resource) = data_object(vm, def, "Resource")? else {
        return Ok(r);
    };
    if let Some(i) = map_slot(vm, m, resource)? {
        delete_pool_at_index(vm, m, i32::from(i))?;
    }
    Ok(r)
}

/// `GetResourcePoolForResourceDefinition(out Mgr, ActorOwner, Resource)` (section 1.5): the manager made if
/// necessary; the map's slot for the resource gives manager, slot, the slot pool's GUID (255 for an empty slot) and
/// the cache by the resolve rule; otherwise the invalid reference. No creation.
pub fn pool_for_resource(
    vm: &mut Vm,
    mgr: Option<ObjRef>,
    owner: Option<ObjRef>,
    resource: Option<ObjRef>,
) -> Result<Created, VmError> {
    let manager = ensure_manager(vm, mgr, owner)?;
    let reference = match (manager, resource) {
        (Some(m), Some(res)) => match map_slot(vm, m, res)? {
            Some(i) if i < POOL_SLOTS => {
                let guid = match slot(&vm.space, m, i)? {
                    Some(p) => byte(&vm.space, p, "PoolGUID")?,
                    None => INVALID_BYTE,
                };
                resolve(&vm.space, PoolRef::new(Some(m), i, guid, None))?.0
            }
            _ => PoolRef::INVALID,
        },
        _ => PoolRef::INVALID,
    };
    Ok(Created { manager, reference })
}

/// `ResourcePoolManager.RecalculateBaseValues(Mgr)` (NATIVE_LEVEL_UP_ATTRIBUTES.md section 2, UNVERIFIED; observed
/// through the health pool): every slot that holds a pool, in slot order, runs the pool's script function
/// `CalculateBaseValues` with its argument true (looked up by name). A None manager is skipped.
pub fn recalculate_base_values(vm: &mut Vm, manager: Option<ObjRef>) -> Result<(), VmError> {
    let Some(m) = manager else { return Ok(()) };
    for (_, pool) in pools_of(&vm.space, m)? {
        let f = find_script(vm, pool, "CalculateBaseValues")?;
        vm.call(f, Some(pool), vec![Value::Bool(true)])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The pawn's health (section 2, section 1.8)
// ---------------------------------------------------------------------------------------------------------------

/// `pawn.HealthPool` resolved on a copy (the pawn's cache is not refreshed): the pool, if valid.
pub fn health_pool(space: &ObjectSpace, pawn: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let r = read_ref(space, &field(space, pawn, "HealthPool")?)?;
    Ok(resolve(space, r)?.1)
}

/// `pawn.HealthPool` resolved and its cache written back to the pawn.
fn health_pool_cached(space: &mut ObjectSpace, pawn: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let v = field(space, pawn, "HealthPool")?;
    let ty = struct_of_value(&v)?;
    let (r, pool) = resolve(space, read_ref(space, &v)?)?;
    let v = ref_value(space, ty, r)?;
    set_field(space, pawn, "HealthPool", v)?;
    Ok(pool)
}

fn struct_of_value(v: &Value) -> Result<StructId, VmError> {
    v.as_struct().map(|s| s.ty).ok_or_else(|| {
        rule_error(
            "ResourcePoolReference",
            format!("expected a pool reference struct, got {}", v.kind_name()),
        )
    })
}

/// `Pawn.GetHealth()`: the pool's `CurrentValue` when `HealthPool` resolves (on a copy), else `HealthVar`. No clamp.
pub fn get_health(space: &ObjectSpace, pawn: ObjRef) -> Result<f32, VmError> {
    match health_pool(space, pawn)? {
        Some(pool) => float(space, pool, "CurrentValue"),
        None => float(space, pawn, "HealthVar"),
    }
}

/// `Pawn.SetHealth(v)`: a value from 0 up to (not including) 1 becomes exactly 0, a negative one is kept; then the
/// pool's `SetCurrentValue` when the pawn's own `HealthPool` resolves (and is cached), else `HealthVar`.
pub fn set_health(vm: &mut Vm, pawn: ObjRef, v: f32) -> Result<(), VmError> {
    let v = if (0.0..HEALTH_FLOOR).contains(&v) {
        0.0
    } else {
        v
    };
    // The hook refusal of `set_current` is decided before the pawn's cache is written.
    if let Some(pool) = health_pool(&vm.space, pawn)? {
        current_hook_refusal(vm, pool)?;
    }
    match health_pool_cached(&mut vm.space, pawn)? {
        Some(pool) => set_current(vm, pool, v),
        None => set_field(&mut vm.space, pawn, "HealthVar", Value::Float(v)),
    }
}

/// `Pawn.GetMaxHealth()` (no argument): the pool's effective `MaxValue` when `HealthPool` resolves, else
/// `HealthMaxVar`.
pub fn get_max_health(space: &ObjectSpace, pawn: ObjRef) -> Result<f32, VmError> {
    match health_pool(space, pawn)? {
        Some(pool) => float(space, pool, "MaxValue"),
        None => float(space, pawn, "HealthMaxVar"),
    }
}

/// `Pawn.SetHealthResourcePoolDefinition(out Mgr, Definition)` (section 1.8): only for a pawn with authority. A pawn
/// whose `HealthPool` does not resolve gets `CreatePool(Mgr, pawn.Controller, Definition, reinit true)`; then, with a
/// valid pool, `HealthMaxVar` becomes the pool's effective maximum. Returns the manager for the caller's `Mgr`.
pub fn set_health_pool_definition(
    vm: &mut Vm,
    pawn: ObjRef,
    mgr: Option<ObjRef>,
    def: Option<ObjRef>,
) -> Result<Option<ObjRef>, VmError> {
    if !is_authority(&vm.space, pawn)? {
        return Ok(mgr);
    }
    let mut manager = mgr;
    if health_pool_cached(&mut vm.space, pawn)?.is_none() {
        let controller = object(&vm.space, pawn, "Controller")?;
        let made = create_pool(vm, mgr, controller, def, true)?;
        manager = made.manager;
        let ty = prop_struct(&vm.space, pawn, "HealthPool")?;
        let v = ref_value(&vm.space, ty, made.reference)?;
        set_field(&mut vm.space, pawn, "HealthPool", v)?;
    }
    if let Some(pool) = health_pool_cached(&mut vm.space, pawn)? {
        let max = float(&vm.space, pool, "MaxValue")?;
        set_field(&mut vm.space, pawn, "HealthMaxVar", Value::Float(max))?;
    }
    Ok(manager)
}

// ---------------------------------------------------------------------------------------------------------------
// The engine query (NATIVE_ATTRIBUTES.md section 5)
// ---------------------------------------------------------------------------------------------------------------

const RECORD_QUERY: &str = "NATIVE_ATTRIBUTES.md section 5 (ResourcePoolAttributeContextResolver)";

/// The answer to [`EngineQuery::ResourcePool`] (NATIVE_ATTRIBUTES.md section 5, confidence medium): the resource's
/// own owner resolver (`ResourceContextResolver`) finds the owner from the source, then the owner's pool for the
/// resource is the slot its manager's map gives. Only the controller resolver is read here, with the record's rule
/// on any source (`ow_attr::context_resolvers::controller_of`: a controller is itself, an actor with an
/// `Instigator` the instigator's controller, a pawn its own or its vehicle's); a source with no controller has no
/// pool (`Ok(None)`). Any other resolver class is `NotYet`, and so is an owner without `ResourcePoolManager`. A None
/// manager or no pool for the resource is `Ok(None)`.
///
/// The record does not name the property. `ResourceContextResolver` is the `ResourceDefinition` property checked
/// locally against the installed `Engine.upk`: all 31 installed resources hold a `ControllerAttributeContextResolver`
/// there (decision 2026-11-15-29). A None resolver giving `Ok(None)` is UNVERIFIED (fitted: no installed resource has
/// one). The controller rule is `ow-attr`'s, called directly; dispatching any resolver class through the
/// `AttributeSystem` registry is an open contract follow-up.
fn query_pool(
    maps: &RefCell<PoolMaps>,
    resource: ObjRef,
    source: ObjRef,
    w: WorldParts<'_>,
) -> Result<Option<ObjRef>, AttrError> {
    let WorldParts {
        space, mut defs, ..
    } = w;
    let mut load = |space: &mut ObjectSpace, o: ObjRef| -> Result<(), AttrError> {
        if let Some(d) = defs.as_deref_mut()
            && d.is_unloaded(space, o)
        {
            d.ensure_loaded(space, o)
                .map_err(|e| AttrError::World(format!("loading a resource definition: {e}")))?;
        }
        Ok(())
    };
    load(space, resource)?;
    let Some(resolver) = object(space, resource, "ResourceContextResolver")? else {
        return Ok(None);
    };
    load(space, resolver)?;
    let owner = if is_a(space, resolver, "ControllerAttributeContextResolver")? {
        // The record's controller rule on any source (`ow-attr`'s resolver; a weapon reaches here through
        // `WeaponResourcePoolAttributeContextResolver`, lane FIX-EQUIP-NATIVES). No controller is no pool.
        match ow_attr::context_resolvers::controller_of(space, source)? {
            Some(owner) => owner,
            None => return Ok(None),
        }
    } else {
        return Err(NotYet::not_read(
            format!(
                "the resource owner resolver {} on a {} source",
                ow_attr::eval::class_path(space, resolver),
                ow_attr::eval::class_path(space, source)
            ),
            RECORD_QUERY,
        )
        .into());
    };
    let class = space.heap.get(owner).map_err(VmError::from)?.class;
    if space.prop(class, "ResourcePoolManager").is_none() {
        return Err(NotYet::not_read(
            format!(
                "the pools of an owner without a manager field ({})",
                ow_attr::eval::class_path(space, owner)
            ),
            RECORD_QUERY,
        )
        .into());
    }
    let Some(manager) = object(space, owner, "ResourcePoolManager")? else {
        return Ok(None);
    };
    let found = maps
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("PoolMaps"))?
        .slot(manager, resource);
    match found {
        Some(i) => Ok(slot(space, manager, i)?),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------------------------------------------

const GET_MAX_VALUE: &str = "ResourcePool.GetMaxValue(bool)";
const GET_CURRENT_VALUE: &str = "ResourcePool.GetCurrentValue()";
const SET_CURRENT_VALUE: &str = "ResourcePool.SetCurrentValue(float)";
const UPDATE_LAST_VALUES: &str = "ResourcePool.UpdateLastValues()";
const POOL_IS_NOW_FULL: &str = "ResourcePool.PoolIsNowFull()";
const RESET_IDLE_TIMER: &str = "ResourcePool.ResetIdleTimer()";
const CLEAR_STACKS: &str = "ResourcePool.ClearAttributeModifierStacks()";
const CREATE_POOL: &str = "ResourcePoolManager.CreatePool(object,object,object,bool)";
const CREATE_POOL_AT_INDEX: &str =
    "ResourcePoolManager.CreatePoolAtIndex(object,object,object,int,byte)";
const INTERNAL_CREATE: &str = "ResourcePoolManager.InternalCreatePoolAtIndex(object,int,byte)";
const DELETE_BY_REFERENCE: &str =
    "ResourcePoolManager.DeletePoolByReference(resourcepoolreference)";
const DELETE_POOL: &str = "ResourcePoolManager.DeletePool(object,resourcepoolreference)";
const DELETE_AT_INDEX: &str = "ResourcePoolManager.DeletePoolAtIndex(int)";
const DELETE_ALL: &str = "ResourcePoolManager.DeleteAllPools(object)";
const HAS_POOL: &str = "ResourcePoolManager.HasResourcePoolForResourceDefinition(object)";
const GET_POOL: &str =
    "ResourcePoolManager.GetResourcePoolForResourceDefinition(object,object,object)";
const ENSURE_MANAGER: &str =
    "ResourcePoolManager.CreateResourcePoolManagerIfNecessary(object,object)";
const RECALCULATE: &str = "ResourcePoolManager.RecalculateBaseValues(object)";
const CONTROLLER_GET_POOL: &str = "Controller.GetResourcePoolForResourceDefinition(object,bool)";
const IS_VALID: &str = "Actor.IsResourcePoolValid(resourcepoolreference)";
const CURRENT_OR_DEFAULT: &str =
    "Actor.GetResourcePoolCurrentValueOrDefault(resourcepoolreference,float)";
const SET_HEALTH_DEFINITION: &str = "Pawn.SetHealthResourcePoolDefinition(object,object)";
const GET_HEALTH: &str = "Pawn.GetHealth()";
const SET_HEALTH: &str = "Pawn.SetHealth(float)";
const GET_MAX_HEALTH: &str = "Pawn.GetMaxHealth()";
const SET_MAX_HEALTH: &str = "Pawn.SetMaxHealth(float)";

/// Every native key this module registers, as the loader derives them from the installed `Engine.upk`
/// (checked locally; decision 2026-11-15-29).
pub const NATIVE_KEYS: [&str; 26] = [
    GET_MAX_VALUE,
    GET_CURRENT_VALUE,
    SET_CURRENT_VALUE,
    UPDATE_LAST_VALUES,
    POOL_IS_NOW_FULL,
    RESET_IDLE_TIMER,
    CLEAR_STACKS,
    CREATE_POOL,
    CREATE_POOL_AT_INDEX,
    INTERNAL_CREATE,
    DELETE_BY_REFERENCE,
    DELETE_POOL,
    DELETE_AT_INDEX,
    DELETE_ALL,
    HAS_POOL,
    GET_POOL,
    ENSURE_MANAGER,
    RECALCULATE,
    CONTROLLER_GET_POOL,
    IS_VALID,
    CURRENT_OR_DEFAULT,
    SET_HEALTH_DEFINITION,
    GET_HEALTH,
    SET_HEALTH,
    GET_MAX_HEALTH,
    SET_MAX_HEALTH,
];

type Body = fn(&mut NativeCall<'_>) -> Result<Value, VmError>;

fn none() -> Value {
    Value::None
}

/// The reference argument `i` and its struct type.
fn arg_ref(c: &NativeCall<'_>, native: &str, i: usize) -> Result<(PoolRef, StructId), VmError> {
    let v = c.arg(i)?;
    let ty = v
        .as_struct()
        .map(|s| s.ty)
        .ok_or_else(|| bad(native, i, "expected a pool reference struct"))?;
    Ok((read_ref(&c.vm.space, v)?, ty))
}

fn natives() -> [(&'static str, Body); 26] {
    [
        (GET_MAX_VALUE, |c| {
            let pool = this(c, GET_MAX_VALUE)?;
            let base = arg_bool(c, GET_MAX_VALUE, 0)?;
            let name = if base {
                "MaxValueBaseValue"
            } else {
                "MaxValue"
            };
            Ok(Value::Float(float(&c.vm.space, pool, name)?))
        }),
        (GET_CURRENT_VALUE, |c| {
            let pool = this(c, GET_CURRENT_VALUE)?;
            Ok(Value::Float(float(&c.vm.space, pool, "CurrentValue")?))
        }),
        (SET_CURRENT_VALUE, |c| {
            let pool = this(c, SET_CURRENT_VALUE)?;
            let v = arg_float(c, SET_CURRENT_VALUE, 0)?;
            set_current(c.vm, pool, v)?;
            Ok(none())
        }),
        (UPDATE_LAST_VALUES, |c| {
            let pool = this(c, UPDATE_LAST_VALUES)?;
            update_last_values(&mut c.vm.space, pool)?;
            Ok(none())
        }),
        (POOL_IS_NOW_FULL, |c| {
            let pool = this(c, POOL_IS_NOW_FULL)?;
            pool_is_now_full(&mut c.vm.space, pool)?;
            Ok(none())
        }),
        (RESET_IDLE_TIMER, |c| {
            let pool = this(c, RESET_IDLE_TIMER)?;
            if c.vm.parity == ParityMode::Off {
                let handler = c.vm.service::<ResetIdleTimerOff>().ok_or_else(|| NotYet::not_read(
                    "ResourcePool.ResetIdleTimer: Off handler not installed",
                    "NATIVE_RESOURCE_POOL_IDLE.md",
                ))?;
                let handler = handler.try_borrow().map_err(|_| VmError::ServiceBorrowed("ResetIdleTimerOff"))?.0;
                handler(c.vm, pool)?;
                return Ok(none());
            }
            let t = world_time(c.vm)?;
            set_field(
                &mut c.vm.space,
                pool,
                "PoolIdleDelayStartTime",
                Value::Float(t),
            )?;
            Ok(none())
        }),
        (CLEAR_STACKS, |c| {
            let pool = this(c, CLEAR_STACKS)?;
            clear_modifier_stacks(&mut c.vm.space, pool)?;
            Ok(none())
        }),
        (CREATE_POOL, |c| {
            let ty = result_struct(c, CREATE_POOL)?;
            let mgr = arg_object(c, CREATE_POOL, 0)?;
            let owner = arg_object(c, CREATE_POOL, 1)?;
            let def = arg_object(c, CREATE_POOL, 2)?;
            let reinit = arg_bool(c, CREATE_POOL, 3)?;
            let made = create_pool(c.vm, mgr, owner, def, reinit)?;
            c.set_out(0, Value::Object(made.manager))?;
            ref_value(&c.vm.space, ty, made.reference)
        }),
        (CREATE_POOL_AT_INDEX, |c| {
            let Some(m) = arg_object(c, CREATE_POOL_AT_INDEX, 0)? else {
                return Err(NotYet::not_read(
                    "ResourcePoolManager.CreatePoolAtIndex without a manager (whether it makes one is not read)",
                    RECORD,
                )
                .into());
            };
            let def = arg_object(c, CREATE_POOL_AT_INDEX, 2)?;
            let index = arg_int(c, CREATE_POOL_AT_INDEX, 3)?;
            let guid = arg_byte(c, CREATE_POOL_AT_INDEX, 4)?;
            Ok(Value::Object(create_pool_at_index(
                c.vm, m, def, index, guid,
            )?))
        }),
        (INTERNAL_CREATE, |c| {
            let m = this(c, INTERNAL_CREATE)?;
            let def = arg_object(c, INTERNAL_CREATE, 0)?;
            let index = arg_int(c, INTERNAL_CREATE, 1)?;
            let guid = arg_byte(c, INTERNAL_CREATE, 2)?;
            Ok(Value::Object(create_pool_at_index(
                c.vm, m, def, index, guid,
            )?))
        }),
        (DELETE_BY_REFERENCE, |c| {
            let (r, ty) = arg_ref(c, DELETE_BY_REFERENCE, 0)?;
            delete_by(c.vm, r)?;
            let v = ref_value(&c.vm.space, ty, PoolRef::INVALID)?;
            c.set_out(0, v)?;
            Ok(none())
        }),
        (DELETE_POOL, |c| {
            let mgr = arg_object(c, DELETE_POOL, 0)?;
            let (r, _) = arg_ref(c, DELETE_POOL, 1)?;
            if mgr != r.manager {
                return Err(NotYet::not_read(
                    "ResourcePoolManager.DeletePool with a manager other than the reference's",
                    RECORD,
                )
                .into());
            }
            delete_by(c.vm, r)?;
            Ok(none())
        }),
        (DELETE_AT_INDEX, |c| {
            let m = this(c, DELETE_AT_INDEX)?;
            let i = arg_int(c, DELETE_AT_INDEX, 0)?;
            delete_pool_at_index(c.vm, m, i)?;
            Ok(none())
        }),
        (DELETE_ALL, |c| {
            let m = arg_object(c, DELETE_ALL, 0)?;
            delete_all_pools(c.vm, m)?;
            Ok(none())
        }),
        (HAS_POOL, |c| {
            let m = this(c, HAS_POOL)?;
            let res = arg_object(c, HAS_POOL, 0)?;
            let found = pool_for_resource(c.vm, Some(m), None, res)?;
            Ok(Value::Bool(found.reference.data.is_some()))
        }),
        (GET_POOL, |c| {
            let ty = result_struct(c, GET_POOL)?;
            let mgr = arg_object(c, GET_POOL, 0)?;
            let owner = arg_object(c, GET_POOL, 1)?;
            let res = arg_object(c, GET_POOL, 2)?;
            let found = pool_for_resource(c.vm, mgr, owner, res)?;
            c.set_out(0, Value::Object(found.manager))?;
            ref_value(&c.vm.space, ty, found.reference)
        }),
        (ENSURE_MANAGER, |c| {
            let mgr = arg_object(c, ENSURE_MANAGER, 0)?;
            let owner = arg_object(c, ENSURE_MANAGER, 1)?;
            let m = ensure_manager(c.vm, mgr, owner)?;
            c.set_out(0, Value::Object(m))?;
            Ok(none())
        }),
        (RECALCULATE, |c| {
            let m = arg_object(c, RECALCULATE, 0)?;
            recalculate_base_values(c.vm, m)?;
            Ok(none())
        }),
        (CONTROLLER_GET_POOL, |c| {
            let ty = result_struct(c, CONTROLLER_GET_POOL)?;
            let controller = this(c, CONTROLLER_GET_POOL)?;
            let res = arg_object(c, CONTROLLER_GET_POOL, 0)?;
            // bCreatePoolIfOneDoesntExist: true when omitted (the record's default).
            let create = if c.has(1) {
                arg_bool(c, CONTROLLER_GET_POOL, 1)?
            } else {
                true
            };
            let r = controller_pool(c.vm, controller, res, create)?;
            ref_value(&c.vm.space, ty, r)
        }),
        (IS_VALID, |c| {
            let (r, ty) = arg_ref(c, IS_VALID, 0)?;
            let (r, pool) = resolve(&c.vm.space, r)?;
            let v = ref_value(&c.vm.space, ty, r)?;
            c.set_out(0, v)?;
            Ok(Value::Bool(pool.is_some()))
        }),
        (CURRENT_OR_DEFAULT, |c| {
            let (r, ty) = arg_ref(c, CURRENT_OR_DEFAULT, 0)?;
            let default = arg_float(c, CURRENT_OR_DEFAULT, 1)?;
            let (r, pool) = resolve(&c.vm.space, r)?;
            let v = ref_value(&c.vm.space, ty, r)?;
            c.set_out(0, v)?;
            match pool {
                Some(p) => Ok(Value::Float(float(&c.vm.space, p, "CurrentValue")?)),
                None => Ok(Value::Float(default)),
            }
        }),
        (SET_HEALTH_DEFINITION, |c| {
            let pawn = this(c, SET_HEALTH_DEFINITION)?;
            let mgr = arg_object(c, SET_HEALTH_DEFINITION, 0)?;
            let def = arg_object(c, SET_HEALTH_DEFINITION, 1)?;
            let m = set_health_pool_definition(c.vm, pawn, mgr, def)?;
            c.set_out(0, Value::Object(m))?;
            Ok(none())
        }),
        (GET_HEALTH, |c| {
            let pawn = this(c, GET_HEALTH)?;
            Ok(Value::Float(get_health(&c.vm.space, pawn)?))
        }),
        (SET_HEALTH, |c| {
            let pawn = this(c, SET_HEALTH)?;
            let v = arg_float(c, SET_HEALTH, 0)?;
            set_health(c.vm, pawn, v)?;
            Ok(none())
        }),
        (GET_MAX_HEALTH, |c| {
            let pawn = this(c, GET_MAX_HEALTH)?;
            Ok(Value::Float(get_max_health(&c.vm.space, pawn)?))
        }),
        (SET_MAX_HEALTH, |c| {
            let pawn = this(c, SET_MAX_HEALTH)?;
            let v = arg_float(c, SET_MAX_HEALTH, 0)?;
            set_field(&mut c.vm.space, pawn, "HealthMaxVar", Value::Float(v))?;
            Ok(none())
        }),
    ]
}

/// `Controller.GetResourcePoolForResourceDefinition(Resource, bCreatePoolIfOneDoesntExist)` (section 1.5,
/// medium-high): with a resource, and a manager or `create`, the controller's manager is made if necessary and the
/// map lookup tried; a controller whose pawn is a vehicle is `NotYet` (the vehicle's pool provider is not read);
/// still nothing and `create`: `CreatePool(manager, controller, Resource.DefaultResourcePoolDefinition, false)`.
pub fn controller_pool(
    vm: &mut Vm,
    controller: ObjRef,
    resource: Option<ObjRef>,
    create: bool,
) -> Result<PoolRef, VmError> {
    let Some(res) = resource else {
        return Ok(PoolRef::INVALID);
    };
    let mgr = object(&vm.space, controller, "ResourcePoolManager")?;
    if mgr.is_none() && !create {
        return Ok(PoolRef::INVALID);
    }
    // A controller driving a vehicle: the vehicle's pool provider is not read. Decided before a manager is made, so the
    // refusal changes nothing; an existing manager's pool is still found first, as the record orders it.
    if let Some(pawn) = object(&vm.space, controller, "Pawn")?
        && is_a(&vm.space, pawn, "Vehicle")?
    {
        if let Some(m) = mgr {
            let found = pool_for_resource(vm, Some(m), None, Some(res))?;
            if found.reference.data.is_some() {
                return Ok(found.reference);
            }
        }
        return Err(NotYet::not_read(
            "the pool lookup of a vehicle that provides pools (Controller.GetResourcePoolForResourceDefinition)",
            RECORD,
        )
        .into());
    }
    let found = pool_for_resource(vm, mgr, Some(controller), Some(res))?;
    let spawned = if mgr.is_none() { found.manager } else { None };
    let result = if found.reference.data.is_some() || !create {
        Ok(found)
    } else {
        data_object(vm, res, "DefaultResourcePoolDefinition")
            .and_then(|def| create_pool(vm, found.manager, Some(controller), def, false))
    };
    match result {
        Ok(made) => {
            // The controller's field is written only once the call succeeded.
            set_field(
                &mut vm.space,
                controller,
                "ResourcePoolManager",
                Value::Object(made.manager),
            )?;
            Ok(made.reference)
        }
        Err(e) => {
            // A refused creation leaves the controller as it was: a manager this call spawned is destroyed and the
            // field keeps its old value (it was never written).
            if let Some(m) = spawned {
                actors::destroy(vm, m)?;
            }
            Err(e)
        }
    }
}

/// The pool natives, the manager natives, the two actor natives and the pawn's health natives as a [`NativePack`],
/// plus the [`PoolMaps`] service and the [`QueryKind::ResourcePool`] handler. `World::new` registers it; it needs
/// the world's [`WorldQuery`] service.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        let query = vm
            .service::<WorldQuery>()
            .ok_or_else(|| VmError::Registration {
                key: "pools::Pack".to_string(),
                reason: "needs the world query service (World::new installs it first)",
            })?;
        let maps = vm.add_service(PoolMaps::new())?;
        if vm.service::<ow_vm::api::WorldTimeQuery>().is_none() {
            vm.add_service(ow_vm::api::WorldTimeQuery(|vm| {
                let actors = actors::actors_of(vm)?;
                let world = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info();
                // World::new represents an existing world. Before its first actor is placed, its time is unknown.
                world.map(Some).ok_or_else(|| NotYet::not_ported(
                    "the existing world's first WorldInfo actor", "NATIVE_RESOURCE_POOL_IDLE.md",
                ).into())
            }))?;
        }
        for (key, body) in natives() {
            vm.register_native(key, Rc::new(body))?;
        }
        let handler: QueryFn = Rc::new(move |q, source, w| match q {
            EngineQuery::ResourcePool { resource } => query_pool(&maps, resource, source, w),
            other => Err(NotYet::not_ported(
                format!("engine query {other:?} sent to the resource-pool handler"),
                RECORD_QUERY,
            )
            .into()),
        });
        query
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?
            .register(QueryKind::ResourcePool, handler)
    }
}

#[cfg(test)]
mod tests;
