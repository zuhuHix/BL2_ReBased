//! Owner: lane IMPL-INITIALIZE-BEHAVIOR-PROVIDERS. `WillowPawn.InitializeBehaviorProviders()` in `ParityMode::Off`,
//! bound by the kernel pack ([`super::Pack`]). AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 2 alone (#740
//! with its review r1, lane ANALYST-INITIALIZE-BEHAVIOR-PROVIDERS, decision 2026-11-25-75); this lane did not read the
//! executable. **Every rule is UNVERIFIED in the running game.** Reference binds nothing (the stub policy answers).
//!
//! **The preflight** reads only: no write, no store load, no class-code load, nothing interned. In order, the first
//! refusal met is the answer:
//! 1. a receiver of `WillowGame.WillowAIPawn` or a subclass (by name, up the loaded class chain): [`AI_PAWN`]
//!    (`NotRead`);
//! 2. the declarations, else `VmError::Script` naming the field: `ConsumerHandle` a struct with an int `PID`;
//!    `BodyClass` an object; `InstanceDataState` a struct whose `Data` is an array of a struct whose `ComponentData`
//!    is a struct with an object `HitRegionDefinition`; `ReplicatedBehaviorConsumerState` a struct with a bool
//!    `bNeedToApplyThisState`. A receiver the store has not loaded is then [`RECEIVER_NOT_LOADED`] (`NotRead`);
//! 3. `ConsumerHandle.PID` not exactly -1: nothing, and no write;
//! 4. no kernel service: the pack's `NotYet { LayerAbsent }`, "the behavior kernel natives";
//! 5. a kernel process already holding the pawn as its consumer: `RegisterBehaviorConsumer`'s own `NotRead`;
//! 6. the list, with a `BodyClass` (none without one), in the note's order:
//!    - (a) `DefaultHitRegion`'s definition, only when it is set, `HitRegionList` is not empty and holds it
//!      (**UNVERIFIED, medium**: the membership test was read by fit);
//!    - (b) each non-None `HitRegionList` entry's definition, then its `OverrideDeathDef`'s when that is set;
//!    - (c) each non-None `ScrapedStances` entry's `BehaviorProviderDefinition` field (**UNVERIFIED, by fit**: the
//!      stance's interface method was not read);
//!    - (d) each `InstanceDataState.Data` entry whose `ComponentData.HitRegionDefinition` is set and is a
//!      `WillowGame.BodyHitRegionDefinition` (by name, up the loaded class chain): its definition;
//!    - (e) `DeathDef`'s definition; (f) `PlayerEventDef`'s definition.
//!
//!    "Its definition" is the object's `BehaviorProviderDefinition` field. Every object read must be resident, else
//!    [`NOT_LOADED`] (`NotRead`). Then the kernel's gate: a None definition is dropped; a definition outside the
//!    provider classes is the decoder's `VmError::Script`; one whose `BehaviorSequences` is empty is dropped;
//! 7. per kept definition, in list order: a repeat (its stock path met earlier in the list, as the kernel rule
//!    compares) is the kernel rule's `NotRead`; then its first sequence with a `CustomEnableCondition` (a plain read
//!    of the field) is the kernel rule's pass-2 `LayerAbsent`, with the same text and layer;
//! 8. `ReplicatedBehaviorConsumerState.bNeedToApplyThisState` set: [`APPLY_REPLICATED`] (`NotRead`).
//!
//! **The commit:** register the pawn by `RegisterBehaviorConsumer`'s allocator (owner `Kernel`) and write its `PID`
//! into `ConsumerHandle`; then each kept definition, in order, by the kernel rule of
//! `IntializeBehaviorProviderForConsumer` (decode, which loads, then pass 1). The note's step 3 does nothing (the flag
//! is clear by preflight 8). A refusal after the registration is marked "stopped after partial writes" by #518's
//! counters, its kind and record kept; none before it is.

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::{
    INVALID_PID, Owner, Process, borrow, claim_in, conditioned, free_pid, handle_pid, kernel, provider_into,
    provider_twice, registered_twice, to_vm, with_pid,
};
use crate::provider::{self, PROVIDER_CLASSES, class_path_of, stock_path};

/// The native's path.
pub const INITIALIZE_BEHAVIOR_PROVIDERS: &str = "WillowGame.WillowPawn.InitializeBehaviorProviders";
/// The record of the rule.
pub const RECORD_PAWN_PROVIDERS: &str = "docs/verification/NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md (section 2)";
/// An AI pawn receiver.
pub const AI_PAWN: &str = "WillowPawn.InitializeBehaviorProviders on a WillowAIPawn: the AI pawn's own body \
                           (NATIVE_BEHAVIOR_POPULATION.md section F)";
/// An object of the list the store has not loaded.
pub const NOT_LOADED: &str = "InitializeBehaviorProviders: a definition the store has not loaded (a load is a write)";
/// A receiver the store has not loaded.
pub const RECEIVER_NOT_LOADED: &str =
    "InitializeBehaviorProviders: a receiver the store has not loaded (a load is a write)";
/// The replicated state to apply.
pub const APPLY_REPLICATED: &str = "InitializeBehaviorProviders: applying the replicated behavior consumer state";

const AI_PAWN_CLASS: &str = "WillowGame.WillowAIPawn";
const BODY_HIT_REGION: &str = "WillowGame.BodyHitRegionDefinition";
const WHAT: &str = "WillowPawn.InitializeBehaviorProviders";

fn bad(message: String) -> VmError {
    VmError::Script { function: WHAT.to_string(), message }
}

fn class_path(vm: &Vm, c: ClassId) -> String {
    vm.space.classes.get_layout(c).map_or_else(|| format!("{c:?}"), |l| l.path().as_str().to_string())
}

/// Whether `class` or one of its loaded super classes has the path `path` (case-insensitive). Loads nothing.
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

/// `o` resident: not a store handle still unloaded or failed, and its slots built.
fn resident(vm: &Vm, o: ObjRef, refusal: &'static str) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(refusal, RECORD_PAWN_PROVIDERS).into());
    }
    Ok(())
}

/// Property `name` of `class` with its declared kind, else a data error naming it.
fn declared(vm: &Vm, class: ClassId, name: &str) -> Result<(PropId, PropKind), VmError> {
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("{} declares no {name}", class_path(vm, class))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(format!("{name} has no declaration")))?;
    Ok((p, kind))
}

/// The declared kind of field `name` of struct `ty`.
fn field_kind(vm: &Vm, ty: StructId, name: &str) -> Option<PropKind> {
    let id = vm.space.names.find(name)?;
    vm.space.classes.struct_layout(ty)?.field(id).map(|(_, d)| d.kind.clone())
}

/// `kind` as a struct whose field `field` has a kind `fits` accepts; else a data error naming `owner.field`.
fn struct_with(
    vm: &Vm,
    kind: &PropKind,
    owner: &str,
    field: &str,
    fits: impl Fn(&PropKind) -> bool,
) -> Result<PropKind, VmError> {
    let PropKind::Struct(ty) = kind else {
        return Err(bad(format!("{owner} is declared as {}, not a struct", kind.kind_name())));
    };
    match field_kind(vm, *ty, field) {
        Some(k) if fits(&k) => Ok(k),
        Some(k) => Err(bad(format!("{owner}.{field} is declared as {}", k.kind_name()))),
        None => Err(bad(format!("{owner} declares no {field}"))),
    }
}

/// The resident `o`'s object field `name` (declared as an object, else a data error).
fn object_field(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let (p, kind) = declared(vm, class, name)?;
    if !matches!(kind, PropKind::Object { .. }) {
        return Err(bad(format!("{name} is declared as {}, not an object", kind.kind_name())));
    }
    match vm.space.read(o, PropAddr::scalar(p))? {
        Value::Object(x) => Ok(*x),
        other => Err(bad(format!("{name} holds {}, not an object", other.kind_name()))),
    }
}

/// The resident `o`'s array field `name`, its elements (declared as an array, else a data error).
fn array_field(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<Value>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let (p, kind) = declared(vm, class, name)?;
    if !matches!(kind, PropKind::Array(_)) {
        return Err(bad(format!("{name} is declared as {}, not an array", kind.kind_name())));
    }
    let v = vm.space.read(o, PropAddr::scalar(p))?;
    v.elements().map(<[Value]>::to_vec).ok_or_else(|| bad(format!("{name} holds {}, not an array", v.kind_name())))
}

/// The resident `o`'s array field `name` as objects (an element that is no object is a data error).
fn objects_field(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<Option<ObjRef>>, VmError> {
    array_field(vm, o, name)?
        .into_iter()
        .map(|v| match v {
            Value::Object(x) => Ok(x),
            other => Err(bad(format!("a {name} entry holds {}, not an object", other.kind_name()))),
        })
        .collect()
}

/// Field `name` of the struct value `v`, else a data error naming `owner.name`.
fn struct_value<'v>(vm: &Vm, v: &'v Value, owner: &str, name: &str) -> Result<&'v Value, VmError> {
    provider::field(&vm.space, v, name).ok_or_else(|| bad(format!("{owner} holds no {name}")))
}

/// The resident `o`'s `BehaviorProviderDefinition`.
fn definition_of(vm: &Vm, o: ObjRef) -> Result<Option<ObjRef>, VmError> {
    resident(vm, o, NOT_LOADED)?;
    object_field(vm, o, "BehaviorProviderDefinition")
}

/// What the commit does, every check passed.
struct Plan {
    handle: Value,
    handle_prop: PropId,
    definitions: Vec<ObjRef>,
}

/// The declarations of preflight step 2; answers the props of `ConsumerHandle`, `BodyClass`, `InstanceDataState` and
/// `ReplicatedBehaviorConsumerState`.
fn declarations(vm: &Vm, class: ClassId) -> Result<[PropId; 4], VmError> {
    let (handle, kind) = declared(vm, class, "ConsumerHandle")?;
    struct_with(vm, &kind, "ConsumerHandle", "PID", |k| matches!(k, PropKind::Int))?;
    let (body, kind) = declared(vm, class, "BodyClass")?;
    if !matches!(kind, PropKind::Object { .. }) {
        return Err(bad(format!("BodyClass is declared as {}, not an object", kind.kind_name())));
    }
    let (instance, kind) = declared(vm, class, "InstanceDataState")?;
    let data = struct_with(vm, &kind, "InstanceDataState", "Data", |k| matches!(k, PropKind::Array(_)))?;
    let PropKind::Array(entry) = data else {
        unreachable!("checked as an array")
    };
    let component = struct_with(vm, &entry, "InstanceDataState.Data", "ComponentData", |k| {
        matches!(k, PropKind::Struct(_))
    })?;
    struct_with(vm, &component, "InstanceDataState.Data.ComponentData", "HitRegionDefinition", |k| {
        matches!(k, PropKind::Object { .. })
    })?;
    let (replicated, kind) = declared(vm, class, "ReplicatedBehaviorConsumerState")?;
    struct_with(vm, &kind, "ReplicatedBehaviorConsumerState", "bNeedToApplyThisState", |k| {
        matches!(k, PropKind::Bool)
    })?;
    Ok([handle, body, instance, replicated])
}

/// The provider definitions of preflight step 6, before the gate (None entries kept: the gate drops them).
fn listed(vm: &Vm, body: ObjRef, instance: &Value) -> Result<Vec<Option<ObjRef>>, VmError> {
    resident(vm, body, NOT_LOADED)?;
    let mut list = Vec::new();
    let default = object_field(vm, body, "DefaultHitRegion")?;
    let regions = objects_field(vm, body, "HitRegionList")?;
    // (a)
    if let Some(d) = default
        && !regions.is_empty()
        && regions.contains(&Some(d))
    {
        list.push(definition_of(vm, d)?);
    }
    // (b)
    for region in regions.iter().flatten() {
        list.push(definition_of(vm, *region)?);
        if let Some(death) = object_field(vm, *region, "OverrideDeathDef")? {
            list.push(definition_of(vm, death)?);
        }
    }
    // (c)
    for stance in objects_field(vm, body, "ScrapedStances")?.into_iter().flatten() {
        list.push(definition_of(vm, stance)?);
    }
    // (d)
    let data = struct_value(vm, instance, "InstanceDataState", "Data")?;
    let entries = data.elements().ok_or_else(|| bad(format!("InstanceDataState.Data holds {}", data.kind_name())))?;
    for entry in entries {
        let component = struct_value(vm, entry, "InstanceDataState.Data", "ComponentData")?;
        let region = match struct_value(vm, component, "InstanceDataState.Data.ComponentData", "HitRegionDefinition")? {
            Value::Object(o) => *o,
            other => return Err(bad(format!("HitRegionDefinition holds {}, not an object", other.kind_name()))),
        };
        let Some(region) = region else { continue };
        resident(vm, region, NOT_LOADED)?;
        if is_named(vm, vm.space.heap.get(region)?.class, BODY_HIT_REGION) {
            list.push(definition_of(vm, region)?);
        }
    }
    // (e), (f)
    for name in ["DeathDef", "PlayerEventDef"] {
        if let Some(o) = object_field(vm, body, name)? {
            list.push(definition_of(vm, o)?);
        }
    }
    Ok(list)
}

/// The sequences of the resident definition `d` (the kernel's gate reads their count).
fn sequences(vm: &Vm, d: ObjRef) -> Result<Vec<Value>, VmError> {
    let class = class_path_of(vm, d).map_err(to_vm)?;
    if !PROVIDER_CLASSES.contains(&class.as_str()) {
        let path = stock_path(vm, d).map_err(to_vm)?;
        // The decoder's own refusal (`provider::decode`), raised here before any write.
        return Err(bad(format!("not a behavior provider: {path} ({class})")));
    }
    array_field(vm, d, "BehaviorSequences")
}

/// The name of the first of `sequences` with a `CustomEnableCondition` (a plain read), as the decoder names it.
fn first_conditioned(vm: &Vm, sequences: &[Value]) -> Option<String> {
    sequences.iter().find_map(|s| match provider::field(&vm.space, s, "CustomEnableCondition") {
        Some(Value::Object(Some(_))) => Some(match provider::field(&vm.space, s, "BehaviorSequenceName") {
            Some(Value::Name(n)) => vm.space.names.text(*n).into_owned(),
            Some(Value::Str(t)) => t.to_string(),
            _ => String::new(),
        }),
        _ => None,
    })
}

/// The preflight (module documentation): `Ok(None)` for nothing to do. Reads only.
fn preflight(vm: &Vm, pawn: ObjRef) -> Result<Option<Plan>, VmError> {
    let class = vm.space.heap.get(pawn)?.class;
    // 1.
    if is_named(vm, class, AI_PAWN_CLASS) {
        return Err(NotYet::not_read(AI_PAWN, RECORD_PAWN_PROVIDERS).into());
    }
    // 2.
    let [handle_prop, body_prop, instance_prop, replicated_prop] = declarations(vm, class)?;
    resident(vm, pawn, RECEIVER_NOT_LOADED)?;
    // 3.
    let handle = vm.space.read(pawn, PropAddr::scalar(handle_prop))?.clone();
    if handle_pid(vm, &handle) != Some(INVALID_PID) {
        return Ok(None);
    }
    // 4, 5.
    let k = kernel(vm)?;
    if borrow(&k)?.processes.values().any(|p| p.consumer == pawn) {
        return Err(registered_twice());
    }
    // 6.
    let body = match vm.space.read(pawn, PropAddr::scalar(body_prop))? {
        Value::Object(o) => *o,
        other => return Err(bad(format!("BodyClass holds {}, not an object", other.kind_name()))),
    };
    let listed = match body {
        Some(body) => {
            let instance = vm.space.read(pawn, PropAddr::scalar(instance_prop))?.clone();
            listed(vm, body, &instance)?
        }
        None => Vec::new(),
    };
    let mut kept = Vec::new();
    for d in listed.into_iter().flatten() {
        resident(vm, d, NOT_LOADED)?;
        let seqs = sequences(vm, d)?;
        if !seqs.is_empty() {
            kept.push((d, seqs));
        }
    }
    // 7.
    let mut paths: Vec<String> = Vec::with_capacity(kept.len());
    for (d, seqs) in &kept {
        let path = stock_path(vm, *d).map_err(to_vm)?;
        if paths.contains(&path) {
            return Err(provider_twice());
        }
        if let Some(name) = first_conditioned(vm, seqs) {
            return Err(conditioned(&name, &path));
        }
        paths.push(path);
    }
    // 8.
    let replicated = vm.space.read(pawn, PropAddr::scalar(replicated_prop))?;
    match struct_value(vm, replicated, "ReplicatedBehaviorConsumerState", "bNeedToApplyThisState")? {
        Value::Bool(false) => {}
        Value::Bool(true) => return Err(NotYet::not_read(APPLY_REPLICATED, RECORD_PAWN_PROVIDERS).into()),
        other => return Err(bad(format!("bNeedToApplyThisState holds {}, not a bool", other.kind_name()))),
    }
    Ok(Some(Plan { handle, handle_prop, definitions: kept.into_iter().map(|(d, _)| d).collect() }))
}

/// #518's three counters.
fn marks(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

/// `WillowPawn.InitializeBehaviorProviders()` in Off (module documentation).
pub(super) fn native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pawn = c.this.ok_or_else(|| bad("called without a pawn".to_string()))?;
    let Some(plan) = preflight(c.vm, pawn)? else {
        return Ok(Value::None);
    };
    let before = marks(c.vm);
    match commit(c.vm, pawn, plan) {
        Err(VmError::NotYet(n)) if marks(c.vm) != before => Err(n.after_partial_writes().into()),
        other => other,
    }
}

fn commit(vm: &mut Vm, pawn: ObjRef, plan: Plan) -> Result<Value, VmError> {
    let k = kernel(vm)?;
    let pid = {
        let mut k = borrow(&k)?;
        let pid = free_pid(&k, 0)?;
        claim_in(&mut k, pid, Owner::Kernel)?;
        k.processes.insert(pid, Process { consumer: pawn, providers: Vec::new() });
        pid
    };
    let handle = with_pid(vm, plan.handle, pid)?;
    vm.space.write(pawn, PropAddr::scalar(plan.handle_prop), handle)?;
    for d in plan.definitions {
        provider_into(vm, pid, d)?;
    }
    Ok(Value::None)
}
