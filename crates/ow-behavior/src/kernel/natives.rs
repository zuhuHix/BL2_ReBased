//! Owner: lane FIX-KERNEL-NATIVES. The behavior kernel's script-facing natives (`GearboxFramework.BehaviorKernel.*`
//! and `WillowPawn.GetBehaviorConsumerHandle`), bound by one pack, with the shared state the crates above use instead
//! of binding these natives themselves. AI-assisted (Claude Code). Decision 2026-11-16-50.
//!
//! **One owner.** [`Pack`] is the only binder of the keys below. `ow-mission` and `ow-combat` (and `ow-skills`, once
//! W6-SKILLS lands) register it from their own registration and never bind these keys: registering the pack again is
//! a no-op, and a key another pack already bound is refused (`duplicate native registration`), so no registration
//! order decides behaviour. A key whose function the package set does not declare is skipped and bound by a later
//! registration once it is declared.
//!
//! **The output-link collector** ([`collect`]). `ActivateBehaviorOutputLink(KernelInfo, OutputLinkId)` appends the
//! id to the innermost running script behavior's list, in call order, duplicates kept; with no behavior running it
//! keeps nothing (NATIVE_MISSION_DISPATCH.md A2, the C++ `mission_script.cpp` and `combat_death.cpp`). A crate that
//! runs a behavior's `ApplyBehaviorToContext` wraps the run in [`collect`] and gets the ids back.
//!
//! **The consumer table.** Every consumer handle's `PID` has at most one owner ([`Owner`]): a crate that hands out
//! handles claims each one ([`claim`], [`allocate`]); the kernel owns the handles it gives out itself
//! (`RegisterBehaviorConsumer`, Off only). The natives that take a consumer handle ([`ROUTED`]) go to the owner of
//! its `PID`:
//! - a crate's handle runs that crate's body for the native ([`route`]); a crate that gave no body for it gets the
//!   VM's unbound-native behaviour (the stub policy: the `UNIMPLEMENTED <key>` line and the zero value, or
//!   `UnboundNative`). On a VM holding that crate alone this is what the call got before the pack existed (the
//!   crate did not bind the native); where another crate bound it, the call used to reach that crate's body;
//! - a kernel handle runs the kernel's own rule, below;
//! - a handle nobody owns (-1, the invalid handle, among them):
//!   - **Off**: a consumer with no process: false, and nothing fires or changes (NATIVE_BEHAVIOR_CONTEXT.md, "kernel
//!     entry points given -1"; NATIVE_BEHAVIOR_POPULATION.md section C, "only continues if [the process] exists");
//!   - **Reference**: what the C++ does, which is the body of the one crate that binds the native in the C++
//!     ([`fallback`]: `ow-mission` for `IsBehaviorSequenceEnabled` and `ActivateBehaviorEventFromScript`, `ow-combat`
//!     for `BroadcastBehaviorEventFromScript`, whose C++ body logs for every `PID`), else the stub policy (the C++
//!     binds nothing there). Two crates' fallbacks for one native have no C++ counterpart: `NotYet { NotPorted }`.
//!
//! **`WillowPawn.GetBehaviorConsumerHandle`.** Off: the record's plain read of the object's `ConsumerHandle`, for
//! every object. Reference: routed like the others, by the `PID` in the object's own `ConsumerHandle` (the C++ mission
//! bridge reads the director's and stubs every other pawn; the C++ combat pack reads every pawn's).
//!
//! | Native | Rule | Modes | Status |
//! |---|---|---|---|
//! | `ActivateBehaviorOutputLink` | the collector | both | UNVERIFIED (NATIVE_MISSION_DISPATCH.md A2) |
//! | `WillowPawn.GetBehaviorConsumerHandle` | Off: a plain read of the object's `ConsumerHandle`; Reference: routed | both | UNVERIFIED (NATIVE_BEHAVIOR_CONTEXT.md) |
//! | `IsBehaviorSequenceEnabled` | routed; kernel: the registered provider's enabled bit, else false | both | UNVERIFIED (NATIVE_MARCUS_USE_CHAIN.md) |
//! | `ActivateBehaviorEventFromScript` | Off: a None provider fires nothing; routed; kernel: fired on the registered provider, an omitted filter -1 | both | UNVERIFIED (NATIVE_MARCUS_USE_CHAIN.md) |
//! | `BroadcastBehaviorEventFromScript` | routed; kernel: fired on every provider of the consumer, in registration order | both | UNVERIFIED (COMBAT_DEATH.md) |
//! | `RegisterBehaviorConsumer` | None: handle -1; else a new process at the lowest free non-negative `PID` | Off | UNVERIFIED; the allocator is not read |
//! | `IntializeBehaviorProviderForConsumer` | routed; kernel: decode, pass 1, refuse pass 2 | Off | UNVERIFIED (NATIVE_BEHAVIOR_POPULATION.md section C) |
//! | `ChangeBehaviorSequenceActivationStatus` | routed; kernel: Enable / Disable / Toggle through the provider | Off | UNVERIFIED (section C) |
//! | `BeginNondeterministicProviderRegistration` | nothing a script sees; the handle is not written | Off | UNVERIFIED (NATIVE_BEHAVIOR_PROVIDERS.md 2, 4) |
//! | `ForceUnregisterBehaviorConsumer` | kernel: the process goes (its providers, so their threads and pending work, with it) and its `PID` is kept out of the allocator; a crate's handle `NotYet`; the handle always comes back -1 | Off | UNVERIFIED (NATIVE_BEHAVIOR_PROVIDERS.md 3, 4) |
//!
//! **Reference mode** binds only what the C++ reference binds (the first five rows); the last five stay unbound
//! there, as in the C++, so the VM's stub policy answers them. The other kernel natives (`Publish*OutputVariable`,
//! `ProcessReplicated*`, ...) are bound in neither mode: their rules are not read, so the stub policy answers them
//! (decision 2026-11-16-50 lists them).
//!
//! **What is fitted, not read.** The `PID` the kernel gives out: the record reads "a non-negative slot index of the
//! process table" and "probably 0" for the first; the lowest free index from 0 is this pack's choice, UNVERIFIED (an
//! analyst question). A consumer registered twice, a provider registered twice on one consumer, a None provider and
//! a provider with a `CustomEnableCondition` (pass 2 links the condition, which the condition's owner does) are
//! `NotYet`, never guessed.
//!
//! **Kernel providers.** A provider the kernel registers keeps its consumer ([`Provider::consumer`]) and runs its
//! behaviors through the VM's handler registry. [`with_provider`] lends one out (for its owner to read its errors or
//! set handlers). A native that reaches a provider that is lent out (a behavior of it is running) is `NotYet`: the
//! re-entrant call is not modelled.

use std::cell::RefCell;
use std::rc::Rc;

use ow_core::{DetMap, DetSet, NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, NativePack, Vm};
use ow_vm::error::VmError;

use crate::kernel::Provider;
use crate::provider::{self, BehaviorError, stock_path};

#[cfg(test)]
mod tests;

/// The decision that records this pack.
pub const RECORD: &str =
    "docs/decisions/2026-11/2026-11-16-50-rust-behavior-kernel-natives-one-pack-the-shared-collector.md";
/// The records of the rules.
pub const RECORD_OUTPUT_LINK: &str = "docs/verification/NATIVE_MISSION_DISPATCH.md (A2)";
pub const RECORD_HANDLE: &str = "docs/verification/NATIVE_BEHAVIOR_CONTEXT.md (GetBehaviorConsumerHandle)";
pub const RECORD_USE_CHAIN: &str = "docs/verification/NATIVE_MARCUS_USE_CHAIN.md";
pub const RECORD_SECTION_C: &str = "docs/verification/NATIVE_BEHAVIOR_POPULATION.md (section C)";

pub const OUTPUT_LINK: &str = "GearboxFramework.BehaviorKernel.ActivateBehaviorOutputLink";
pub const CONSUMER_HANDLE: &str = "WillowGame.WillowPawn.GetBehaviorConsumerHandle";
pub const IS_SEQUENCE_ENABLED: &str = "GearboxFramework.BehaviorKernel.IsBehaviorSequenceEnabled";
pub const EVENT_FROM_SCRIPT: &str = "GearboxFramework.BehaviorKernel.ActivateBehaviorEventFromScript";
pub const BROADCAST_FROM_SCRIPT: &str =
    "GearboxFramework.BehaviorKernel.BroadcastBehaviorEventFromScript";
pub const REGISTER_CONSUMER: &str = "GearboxFramework.BehaviorKernel.RegisterBehaviorConsumer";
pub const INITIALIZE_PROVIDER: &str =
    "GearboxFramework.BehaviorKernel.IntializeBehaviorProviderForConsumer";
pub const CHANGE_SEQUENCE: &str =
    "GearboxFramework.BehaviorKernel.ChangeBehaviorSequenceActivationStatus";
pub const BEGIN_NONDETERMINISTIC: &str =
    "GearboxFramework.BehaviorKernel.BeginNondeterministicProviderRegistration";
pub const FORCE_UNREGISTER: &str = "GearboxFramework.BehaviorKernel.ForceUnregisterBehaviorConsumer";
pub const RECORD_PROVIDERS: &str = "docs/verification/NATIVE_BEHAVIOR_PROVIDERS.md";

/// The natives that take a consumer handle first and go to its owner (module documentation).
pub const ROUTED: [&str; 5] = [
    IS_SEQUENCE_ENABLED,
    EVENT_FROM_SCRIPT,
    BROADCAST_FROM_SCRIPT,
    INITIALIZE_PROVIDER,
    CHANGE_SEQUENCE,
];

/// The natives a crate may give a body for ([`route`], [`fallback`]).
pub const ROUTABLE: [&str; 6] = [
    IS_SEQUENCE_ENABLED,
    EVENT_FROM_SCRIPT,
    BROADCAST_FROM_SCRIPT,
    INITIALIZE_PROVIDER,
    CHANGE_SEQUENCE,
    CONSUMER_HANDLE,
];

/// The invalid consumer handle's `PID` (NATIVE_BEHAVIOR_CONTEXT.md: the constructor's value, all bits set).
pub const INVALID_PID: i32 = -1;

/// Who answers the natives for one consumer handle.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// A process the kernel made (`RegisterBehaviorConsumer`).
    Kernel,
    /// A crate that hands out its own handles (`"ow-mission"`, `"ow-combat"`).
    Crate(&'static str),
    /// A kernel process that `ForceUnregisterBehaviorConsumer` released. No consumer answers for it (a handle nobody
    /// owns), and its `PID` stays out of the allocator: when the game hands a released `PID` out again is not read
    /// (NATIVE_BEHAVIOR_PROVIDERS.md, open question 2).
    Released,
}

/// A consumer the kernel registered: its object and its providers in registration order (path, provider; the
/// provider is `None` while it is lent out).
struct Process {
    consumer: ObjRef,
    providers: Vec<(String, Option<Provider>)>,
}

/// The pack's VM service (module documentation).
#[derive(Default)]
pub struct Kernel {
    /// The keys this pack bound.
    bound: DetSet<String>,
    /// One list per running script behavior, innermost last.
    frames: Vec<Vec<i32>>,
    owners: DetMap<i32, Owner>,
    /// (owner crate, native path) -> the owner's body.
    routes: DetMap<(&'static str, &'static str), NativeFn>,
    /// native path -> the crates' Reference bodies for a handle nobody owns, by owner.
    fallbacks: DetMap<&'static str, DetMap<&'static str, NativeFn>>,
    processes: DetMap<i32, Process>,
}

impl std::fmt::Debug for Kernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kernel")
            .field("bound", &self.bound)
            .field("frames", &self.frames)
            .field("owners", &self.owners)
            .field("routes", &self.routes.keys().collect::<Vec<_>>())
            .field(
                "fallbacks",
                &self
                    .fallbacks
                    .iter()
                    .map(|(n, o)| (*n, o.keys().collect::<Vec<_>>()))
                    .collect::<Vec<_>>(),
            )
            .field("processes", &self.processes.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Binds the kernel natives (module documentation). Registering it again binds only keys declared since.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        if vm.service::<Kernel>().is_none() {
            vm.add_service(Kernel::default())?;
        }
        bind(vm, OUTPUT_LINK, Rc::new(output_link))?;
        let handle: NativeFn = match vm.parity {
            ParityMode::Off => Rc::new(consumer_handle),
            ParityMode::Reference => Rc::new(consumer_handle_reference),
        };
        bind(vm, CONSUMER_HANDLE, handle)?;
        for path in [IS_SEQUENCE_ENABLED, EVENT_FROM_SCRIPT, BROADCAST_FROM_SCRIPT] {
            bind(vm, path, routed(path))?;
        }
        if vm.parity == ParityMode::Off {
            bind(vm, REGISTER_CONSUMER, Rc::new(register_consumer))?;
            for path in [INITIALIZE_PROVIDER, CHANGE_SEQUENCE] {
                bind(vm, path, routed(path))?;
            }
            bind(vm, BEGIN_NONDETERMINISTIC, Rc::new(begin_nondeterministic))?;
            bind(vm, FORCE_UNREGISTER, Rc::new(force_unregister))?;
            // Running behavior lists outside the kernel (NATIVE_RUN_BEHAVIORS.md; decision 2026-11-19-54).
            super::run::register(vm)?;
            // The behavior-variable setters (NATIVE_SET_BEHAVIOR_VARIABLE.md; decision 2026-11-19-159).
            super::variables::register(vm)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The API of the crates above
// ---------------------------------------------------------------------------------------------------------------

/// Runs `run` as one script behavior and returns its result with the output links its script selected
/// (`ActivateBehaviorOutputLink`, in call order, duplicates kept). Runs nest: an inner behavior's links are its own.
/// `NotYet { LayerAbsent }` without [`Pack`].
pub fn collect<T>(vm: &mut Vm, run: impl FnOnce(&mut Vm) -> T) -> Result<(T, Vec<i32>), VmError> {
    let k = kernel(vm)?;
    borrow(&k)?.frames.push(Vec::new());
    let value = run(vm);
    let ids = borrow(&k)?.frames.pop().unwrap_or_default();
    Ok((value, ids))
}

/// Gives `owner`'s body for the routed native `native` ([`ROUTABLE`]), run for the handles `owner` holds. Registers
/// [`Pack`] first. A second body for the same owner and native is refused.
pub fn route(vm: &mut Vm, owner: &'static str, native: &'static str, body: NativeFn) -> Result<(), VmError> {
    routable(native)?;
    Pack.register(vm)?;
    let k = kernel(vm)?;
    let mut k = borrow(&k)?;
    if k.routes.contains_key(&(owner, native)) {
        return Err(VmError::Registration {
            key: native.to_string(),
            reason: "this owner already routes this kernel native",
        });
    }
    k.routes.insert((owner, native), body);
    Ok(())
}

/// Gives `owner`'s Reference body for `native` ([`ROUTABLE`]) on a handle nobody owns: the crate binds this native in
/// the C++ reference (module documentation). Off never runs it. Registers [`Pack`] first. A second fallback of the same
/// owner for one native is refused.
pub fn fallback(
    vm: &mut Vm,
    owner: &'static str,
    native: &'static str,
    body: NativeFn,
) -> Result<(), VmError> {
    routable(native)?;
    Pack.register(vm)?;
    let k = kernel(vm)?;
    let mut k = borrow(&k)?;
    let bodies = k.fallbacks.entry(native).or_default();
    if bodies.contains_key(owner) {
        return Err(VmError::Registration {
            key: native.to_string(),
            reason: "this owner already gave a fallback for this kernel native",
        });
    }
    bodies.insert(owner, body);
    Ok(())
}

fn routable(native: &str) -> Result<(), VmError> {
    if ROUTABLE.contains(&native) {
        Ok(())
    } else {
        Err(VmError::Registration {
            key: native.to_string(),
            reason: "not a routed kernel native",
        })
    }
}

/// Claims `pid` for `owner`. Claiming a `PID` one already holds is a no-op; a `PID` another owner holds, and the
/// invalid `PID`, are refused. Registers [`Pack`] first.
pub fn claim(vm: &mut Vm, pid: i32, owner: &'static str) -> Result<(), VmError> {
    Pack.register(vm)?;
    let k = kernel(vm)?;
    let mut k = borrow(&k)?;
    claim_in(&mut k, pid, Owner::Crate(owner))
}

/// Claims the lowest `PID` from `from` up that no one holds, for `owner`, and returns it. Registers [`Pack`] first.
pub fn allocate(vm: &mut Vm, owner: &'static str, from: i32) -> Result<i32, VmError> {
    Pack.register(vm)?;
    let k = kernel(vm)?;
    let mut k = borrow(&k)?;
    let pid = free_pid(&k, from)?;
    claim_in(&mut k, pid, Owner::Crate(owner))?;
    Ok(pid)
}

/// The owner of `pid`, if any (without [`Pack`]: none).
pub fn owner_of(vm: &Vm, pid: i32) -> Result<Option<Owner>, VmError> {
    match vm.service::<Kernel>() {
        Some(k) => Ok(borrow(&k)?.owners.get(&pid).copied()),
        None => Ok(None),
    }
}

/// The `PID` of the kernel process of `consumer`, if the kernel registered it.
pub fn process_of(vm: &Vm, consumer: ObjRef) -> Result<Option<i32>, VmError> {
    match vm.service::<Kernel>() {
        Some(k) => Ok(borrow(&k)?
            .processes
            .iter()
            .find(|(_, p)| p.consumer == consumer)
            .map(|(pid, _)| *pid)),
        None => Ok(None),
    }
}

/// The stock paths of the providers the kernel registered on process `pid`, in registration order (lane
/// FIX-PHASELOCK-CAST part B); `None` when the kernel holds no process `pid`, or without [`Pack`].
pub fn provider_paths(vm: &Vm, pid: i32) -> Result<Option<Vec<String>>, VmError> {
    match vm.service::<Kernel>() {
        Some(k) => Ok(borrow(&k)?
            .processes
            .get(&pid)
            .map(|p| p.providers.iter().map(|(q, _)| q.clone()).collect())),
        None => Ok(None),
    }
}

/// The `PID` of a `BehaviorConsumerHandle` struct value; `None` for anything else (lane FIX-PHASELOCK-CAST part B).
pub fn consumer_pid(vm: &Vm, handle: &Value) -> Option<i32> {
    handle_pid(vm, handle)
}

/// Lends out the provider at `path` that the kernel registered on process `pid`, runs `f` on it and puts it back.
/// `None` when the process has no such provider; `NotYet` when it is already lent out.
pub fn with_provider<T>(
    vm: &mut Vm,
    pid: i32,
    path: &str,
    f: impl FnOnce(&mut Vm, &mut Provider) -> T,
) -> Result<Option<T>, VmError> {
    let k = kernel(vm)?;
    let slot = borrow(&k)?
        .processes
        .get(&pid)
        .and_then(|p| p.providers.iter().position(|(q, _)| q == path));
    match slot {
        Some(slot) => lend(vm, pid, slot, |vm, p| Ok(f(vm, p))).map(Some),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

// ActivateBehaviorOutputLink(KernelInfo, OutputLinkId): appended to the innermost running behavior's list.
fn output_link(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    // An output link id is an int; the low 32 bits, as the C++ `int(integer())`.
    let id = integer(c.arg(1)?) as i32;
    let k = kernel(c.vm)?;
    if let Some(top) = borrow(&k)?.frames.last_mut() {
        top.push(id);
    }
    Ok(Value::None)
}

// IBehaviorConsumer.GetBehaviorConsumerHandle on the pawn: its own ConsumerHandle, whatever it holds.
fn consumer_handle(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(me) = c.this else {
        return Err(error("GetBehaviorConsumerHandle called without an object".to_string()));
    };
    let class = c.vm.space.heap.get(me)?.class;
    let Some(p) = c.vm.space.prop(class, "ConsumerHandle") else {
        return Err(error(format!(
            "{} has no property ConsumerHandle",
            provider::class_path_of(c.vm, me).map_err(to_vm)?
        )));
    };
    Ok(c.vm.space.read(me, PropAddr::scalar(p))?.clone())
}

// GetBehaviorConsumerHandle in Reference: routed by the PID of the object's own ConsumerHandle (module documentation).
fn consumer_handle_reference(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pid = match c.this {
        Some(me) => {
            let class = c.vm.space.heap.get(me)?.class;
            match c.vm.space.prop(class, "ConsumerHandle") {
                Some(p) => handle_pid(c.vm, c.vm.space.read(me, PropAddr::scalar(p))?),
                None => None,
            }
        }
        None => None,
    };
    dispatch(c, CONSUMER_HANDLE, pid)
}

// RegisterBehaviorConsumer(Consumer) -> BehaviorConsumerHandle (Off only).
fn register_consumer(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let consumer = match c.arg(0)? {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    };
    let pid = match consumer {
        None => INVALID_PID,
        Some(o) => {
            let k = kernel(c.vm)?;
            let mut k = borrow(&k)?;
            if k.processes.values().any(|p| p.consumer == o) {
                return Err(NotYet::not_read(
                    "RegisterBehaviorConsumer of a consumer the kernel already registered",
                    RECORD_HANDLE,
                )
                .into());
            }
            let pid = free_pid(&k, 0)?;
            claim_in(&mut k, pid, Owner::Kernel)?;
            k.processes.insert(
                pid,
                Process {
                    consumer: o,
                    providers: Vec::new(),
                },
            );
            pid
        }
    };
    handle_value(c, pid)
}

// BeginNondeterministicProviderRegistration(out ConsumerHandle), Off only (NATIVE_BEHAVIOR_PROVIDERS.md sections 2
// and 4): the game caps the consumer's replication slot counter, which nothing in this port reads (no slot numbers, no
// replication), so nothing changes and the handle is not written, for any PID.
fn begin_nondeterministic(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Ok(Value::None)
}

// ForceUnregisterBehaviorConsumer(out ConsumerHandle), Off only (NATIVE_BEHAVIOR_PROVIDERS.md sections 3 and 4).
// A kernel process is unregistered: it leaves the process table with its providers, so their running threads and
// pending work never run again, and its PID is marked released (kept out of the allocator, no consumer answers for
// it). A crate's handle is `NotYet` (that crate's teardown is not ported here). Then the caller's handle reads -1, in
// every case that does not refuse: unknown, -1 and released PIDs only get the write.
fn force_unregister(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let handle = c.arg(0)?.clone();
    let pid = handle_pid(c.vm, &handle);
    let k = kernel(c.vm)?;
    {
        let mut k = borrow(&k)?;
        match pid.and_then(|p| k.owners.get(&p).copied().map(|o| (p, o))) {
            Some((p, Owner::Kernel)) => {
                k.processes.remove(&p);
                k.owners.insert(p, Owner::Released);
            }
            Some((p, Owner::Crate(name))) => {
                return Err(NotYet::not_ported(
                    format!("ForceUnregisterBehaviorConsumer on consumer PID {p}, which {name} owns (its teardown)"),
                    RECORD_PROVIDERS,
                )
                .into());
            }
            _ => {}
        }
    }
    let invalid = with_pid(c.vm, handle, INVALID_PID)?;
    c.set_out(0, invalid)?;
    Ok(Value::None)
}

/// A routed native's body (module documentation).
fn routed(native: &'static str) -> NativeFn {
    Rc::new(move |c: &mut NativeCall<'_>| {
        // Off: a None provider fires nothing, whoever owns the handle (NATIVE_MARCUS_USE_CHAIN.md). Reference leaves
        // it to the body the call reaches, as the C++ does (the mission bridge's body checks it first).
        if c.vm.parity == ParityMode::Off
            && native == EVENT_FROM_SCRIPT
            && provider_arg(c)?.is_none()
        {
            return Ok(Value::None);
        }
        let pid = handle_pid(c.vm, c.arg(0)?);
        dispatch(c, native, pid)
    })
}

/// Runs `native` for the handle `pid` (module documentation, "The consumer table").
fn dispatch(c: &mut NativeCall<'_>, native: &'static str, pid: Option<i32>) -> Result<Value, VmError> {
    let k = kernel(c.vm)?;
    let owner = match pid {
        Some(p) => borrow(&k)?.owners.get(&p).copied(),
        None => None,
    };
    match (owner, pid) {
        (Some(Owner::Crate(name)), _) => {
            let body = borrow(&k)?.routes.get(&(name, native)).cloned();
            match body {
                Some(body) => body(c),
                None => unbound(c),
            }
        }
        (Some(Owner::Kernel), Some(pid)) => kernel_rule(c, native, pid),
        _ => match c.vm.parity {
            ParityMode::Off => Ok(unowned(native)),
            ParityMode::Reference => {
                let bodies: Vec<NativeFn> = borrow(&k)?
                    .fallbacks
                    .get(native)
                    .map(|b| b.values().cloned().collect())
                    .unwrap_or_default();
                match bodies.as_slice() {
                    [] => unbound(c),
                    [body] => body(c),
                    _ => Err(NotYet::not_ported(
                        format!(
                            "{native} on a handle nobody owns, with two crates' Reference bodies (no C++ VM holds both)"
                        ),
                        RECORD,
                    )
                    .into()),
                }
            }
        },
    }
}

/// What a routed native answers for a handle no one owns: false, nothing else.
fn unowned(native: &str) -> Value {
    if native == IS_SEQUENCE_ENABLED {
        Value::Bool(false)
    } else {
        Value::None
    }
}

fn kernel_rule(c: &mut NativeCall<'_>, native: &'static str, pid: i32) -> Result<Value, VmError> {
    match native {
        IS_SEQUENCE_ENABLED => {
            let name = text(c.vm, c.arg(2)?);
            let Some(slot) = registered(c, pid)? else {
                return Ok(Value::Bool(false));
            };
            lend(c.vm, pid, slot, |_, p| Ok(Value::Bool(p.sequence_enabled(&name))))
        }
        EVENT_FROM_SCRIPT => {
            let name = text(c.vm, c.arg(2)?);
            let filter = if c.has(3) {
                integer(c.arg(3)?) as i32
            } else {
                -1
            };
            if c.has(4) {
                return Err(NotYet::not_read(
                    "ActivateBehaviorEventFromScript with Parameters (how they reach the event is not read)",
                    RECORD_USE_CHAIN,
                )
                .into());
            }
            if let Some(slot) = registered(c, pid)? {
                lend(c.vm, pid, slot, |vm, p| {
                    p.fire_event(vm, &name, &DetMap::new(), filter, &[])
                        .map_err(to_vm)
                })?;
            }
            Ok(Value::None)
        }
        BROADCAST_FROM_SCRIPT => {
            let name = text(c.vm, c.arg(1)?);
            let count = {
                let k = kernel(c.vm)?;
                let k = borrow(&k)?;
                k.processes.get(&pid).map_or(0, |p| p.providers.len())
            };
            for slot in 0..count {
                lend(c.vm, pid, slot, |vm, p| {
                    p.fire_event(vm, &name, &DetMap::new(), -1, &[])
                        .map_err(to_vm)
                })?;
            }
            Ok(Value::None)
        }
        INITIALIZE_PROVIDER => initialize_provider(c, pid),
        CHANGE_SEQUENCE => change_sequence(c, pid),
        _ => Err(error(format!("{native} is not a routed kernel native"))),
    }
}

// IntializeBehaviorProviderForConsumer(ConsumerHandle, ProviderDefinition), on a kernel process (section C).
fn initialize_provider(c: &mut NativeCall<'_>, pid: i32) -> Result<Value, VmError> {
    let Some(definition) = provider_arg(c)? else {
        return Err(NotYet::not_read(
            "IntializeBehaviorProviderForConsumer with a None provider",
            RECORD_SECTION_C,
        )
        .into());
    };
    let path = stock_path(c.vm, definition).map_err(to_vm)?;
    let k = kernel(c.vm)?;
    let (consumer, known) = {
        let k = borrow(&k)?;
        let Some(process) = k.processes.get(&pid) else {
            return Ok(Value::None);
        };
        (
            process.consumer,
            process.providers.iter().any(|(q, _)| *q == path),
        )
    };
    if known {
        return Err(NotYet::not_read(
            "a provider registered twice on one consumer",
            RECORD_SECTION_C,
        )
        .into());
    }
    let mut p = Provider::new(c.vm, definition).map_err(to_vm)?;
    if let Some(s) = p.data().sequences.iter().find(|s| s.condition.is_some()) {
        return Err(NotYet::layer_absent(
            format!(
                "pass 2 of IntializeBehaviorProviderForConsumer: sequence {} of {path} has a CustomEnableCondition",
                s.name
            ),
            "the condition's owner (the mission tracker's observers)",
            RECORD_SECTION_C,
        )
        .into());
    }
    p.set_consumer(Some(consumer));
    // Pass 1 runs with the provider in its slot, lent out, so a behavior that reaches its own provider is refused
    // (module documentation) instead of finding it missing.
    if let Some(process) = borrow(&k)?.processes.get_mut(&pid) {
        process.providers.push((path.clone(), None));
    }
    let slot = borrow(&k)?
        .processes
        .get(&pid)
        .map_or(0, |p| p.providers.len().saturating_sub(1));
    let result = p.register_consumer(c.vm).map_err(to_vm);
    let mut k = borrow(&k)?;
    if let Some(process) = k.processes.get_mut(&pid) {
        match &result {
            // Registered: the provider goes into its slot.
            Ok(()) => {
                if let Some(entry) = process.providers.get_mut(slot) {
                    entry.1 = Some(p);
                }
            }
            // A failed pass 1 registers nothing: the slot goes, so the call can be made again.
            Err(_) => {
                if process
                    .providers
                    .get(slot)
                    .is_some_and(|(q, held)| held.is_none() && *q == path)
                {
                    process.providers.remove(slot);
                }
            }
        }
    }
    result.map(|()| Value::None)
}

// ChangeBehaviorSequenceActivationStatus(ConsumerHandle, Provider, SequenceName, Action), on a kernel process.
fn change_sequence(c: &mut NativeCall<'_>, pid: i32) -> Result<Value, VmError> {
    if provider_arg(c)?.is_none() {
        return Err(NotYet::not_read(
            "ChangeBehaviorSequenceActivationStatus with a None provider (the lookup is not read)",
            RECORD_SECTION_C,
        )
        .into());
    }
    let name = text(c.vm, c.arg(2)?);
    let action = action_name(c)?;
    let Some(slot) = registered(c, pid)? else {
        return Err(NotYet::not_read(
            "ChangeBehaviorSequenceActivationStatus for a provider the consumer has not registered",
            RECORD_SECTION_C,
        )
        .into());
    };
    lend(c.vm, pid, slot, |vm, p| {
        let enable = match action.as_str() {
            "change_enable" => true,
            "change_disable" => false,
            "change_toggle" => !p.sequence_enabled(&name),
            other => {
                return Err(error(format!(
                    "ChangeBehaviorSequenceActivationStatus: unknown action {other}"
                )));
            }
        };
        p.set_sequence_enabled(vm, &name, enable).map_err(to_vm)?;
        Ok(Value::None)
    })
}

/// The action argument's enum value name (lower case), named from the declaration's enum, never by position.
fn action_name(c: &NativeCall<'_>) -> Result<String, VmError> {
    let value = integer(c.arg(3)?);
    let names = c
        .vm
        .function(c.function)
        .and_then(|d| d.params.get(3))
        .and_then(|p| match &p.kind {
            PropKind::Byte { enum_: Some(e) } => c.vm.space.classes.enum_layout(*e),
            _ => None,
        })
        .map(|layout| layout.values().to_vec());
    let Some(names) = names else {
        return Err(NotYet::not_read(
            "the action of ChangeBehaviorSequenceActivationStatus: its declaration names no enum",
            RECORD_SECTION_C,
        )
        .into());
    };
    let name = usize::try_from(value).ok().and_then(|i| names.get(i)).copied();
    match name {
        Some(n) => Ok(c.vm.space.names.text(n).to_ascii_lowercase()),
        None => Err(error(format!(
            "ChangeBehaviorSequenceActivationStatus: action {value} is outside its enum"
        ))),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------------------------

fn kernel(vm: &Vm) -> Result<Rc<RefCell<Kernel>>, VmError> {
    vm.service::<Kernel>().ok_or_else(|| {
        NotYet::layer_absent(
            "the behavior kernel natives",
            "ow-behavior (kernel::natives::Pack)",
            RECORD,
        )
        .into()
    })
}

fn borrow(k: &Rc<RefCell<Kernel>>) -> Result<std::cell::RefMut<'_, Kernel>, VmError> {
    k.try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_behavior::kernel::natives::Kernel"))
}

/// Binds `f` to the native at `path` when the package set declares it native and this pack has not bound it yet.
pub(crate) fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), VmError> {
    let Ok(id) = vm.find_function(path) else {
        return Ok(());
    };
    // The VM's own package-qualified identity (case folded), so first-wins here and the VM agree (review r1 of #519,
    // item 3).
    let Some(key) = vm.dispatch_key(id) else {
        return Ok(());
    };
    let k = kernel(vm)?;
    if borrow(&k)?.bound.contains(&key) {
        return Ok(());
    }
    // Another pack's binding of the function is refused here: the kernel natives have one owner.
    vm.register_native_for(id, f)?;
    borrow(&k)?.bound.insert(key);
    Ok(())
}

fn claim_in(k: &mut Kernel, pid: i32, owner: Owner) -> Result<(), VmError> {
    if pid == INVALID_PID {
        return Err(VmError::Registration {
            key: format!("consumer PID {pid}"),
            reason: "the invalid consumer handle cannot be claimed",
        });
    }
    match k.owners.get(&pid) {
        Some(o) if *o == owner => Ok(()),
        Some(_) => Err(VmError::Registration {
            key: format!("consumer PID {pid}"),
            reason: "another owner holds this consumer handle",
        }),
        None => {
            k.owners.insert(pid, owner);
            Ok(())
        }
    }
}

fn free_pid(k: &Kernel, from: i32) -> Result<i32, VmError> {
    let mut pid = from.max(0);
    while k.owners.contains_key(&pid) {
        pid = pid.checked_add(1).ok_or(VmError::Registration {
            key: "consumer PID".to_string(),
            reason: "no free consumer handle",
        })?;
    }
    Ok(pid)
}

/// The `PID` of a consumer handle struct, `None` for anything else.
fn handle_pid(vm: &Vm, handle: &Value) -> Option<i32> {
    match provider::field(&vm.space, handle, "PID")? {
        Value::Int(i) => Some(*i),
        Value::Byte(b) => Some(i32::from(*b)),
        _ => None,
    }
}

/// `handle`, a consumer handle struct, with its `PID` field set to `pid`.
fn with_pid(vm: &Vm, mut handle: Value, pid: i32) -> Result<Value, VmError> {
    let index = vm.space.names.find("PID").and_then(|id| {
        let ty = handle.as_struct()?.ty;
        vm.space.classes.struct_layout(ty)?.field(id).map(|(i, _)| i)
    });
    let slot = index
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| handle.struct_mut()?.fields.get_mut(i));
    match slot {
        Some(slot) => *slot = Value::Int(pid),
        None => return Err(error("a consumer handle that is no struct with a PID".to_string())),
    }
    Ok(handle)
}

/// The handle the native returns: its declared result type's default, with `PID` set.
fn handle_value(c: &NativeCall<'_>, pid: i32) -> Result<Value, VmError> {
    let kind = c
        .vm
        .function(c.function)
        .and_then(|d| d.result.as_ref())
        .map(|r| r.kind.clone())
        .ok_or_else(|| error("RegisterBehaviorConsumer declares no result".to_string()))?;
    let mut handle = c
        .vm
        .space
        .classes
        .default_value(&kind)
        .map_err(ow_object::heap::HeapError::from)?;
    let index = c.vm.space.names.find("PID").and_then(|id| {
        let ty = handle.as_struct()?.ty;
        c.vm.space.classes.struct_layout(ty)?.field(id).map(|(i, _)| i)
    });
    let slot = index
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| handle.struct_mut()?.fields.get_mut(i));
    match slot {
        Some(slot) => *slot = Value::Int(pid),
        None => {
            return Err(error(
                "RegisterBehaviorConsumer's result is no struct with a PID".to_string(),
            ));
        }
    }
    Ok(handle)
}

/// The provider definition argument (index 1), loaded; `None` for a None object.
fn provider_arg(c: &mut NativeCall<'_>) -> Result<Option<ObjRef>, VmError> {
    if c.arg_count() < 2 {
        return Ok(None);
    }
    let o = match c.arg(1)? {
        Value::Object(o) => *o,
        _ => None,
    };
    if let Some(o) = o
        && c.vm.defs.is_unloaded(&c.vm.space, o)
    {
        let r = c.vm.defs.ensure_loaded(&mut c.vm.space, o);
        let log = c.vm.defs.take_log();
        c.vm.log.extend(log);
        r.map_err(VmError::Def)?;
    }
    Ok(o)
}

/// The slot of the provider argument among process `pid`'s providers, by stock path (as the C++ compares).
fn registered(c: &mut NativeCall<'_>, pid: i32) -> Result<Option<usize>, VmError> {
    let Some(definition) = provider_arg(c)? else {
        return Ok(None);
    };
    let path = stock_path(c.vm, definition).map_err(to_vm)?;
    let k = kernel(c.vm)?;
    let k = borrow(&k)?;
    Ok(k.processes
        .get(&pid)
        .and_then(|p| p.providers.iter().position(|(q, _)| *q == path)))
}

/// Runs `f` on provider `slot` of process `pid`, taken out of its slot, and puts it back (dropped if the process
/// went away meanwhile).
fn lend<T>(
    vm: &mut Vm,
    pid: i32,
    slot: usize,
    f: impl FnOnce(&mut Vm, &mut Provider) -> Result<T, VmError>,
) -> Result<T, VmError> {
    let k = kernel(vm)?;
    let taken = borrow(&k)?
        .processes
        .get_mut(&pid)
        .and_then(|p| p.providers.get_mut(slot))
        .and_then(|(_, p)| p.take());
    let Some(mut p) = taken else {
        return Err(NotYet::not_read(
            "a kernel native reaching a provider whose behavior is running (re-entrant use is not modelled)",
            RECORD,
        )
        .into());
    };
    let result = f(vm, &mut p);
    if let Some(entry) = borrow(&k)?
        .processes
        .get_mut(&pid)
        .and_then(|q| q.providers.get_mut(slot))
    {
        entry.1 = Some(p);
    }
    result
}

/// The VM's unbound-native behaviour for this call (the stub policy, `Vm::stub`): what the call got before the pack.
fn unbound(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let decl = c.vm.function(c.function);
    let key = decl
        .and_then(|d| d.native_key.clone())
        .unwrap_or_default();
    let result = decl.and_then(|d| d.result.as_ref()).map(|r| r.kind.clone());
    if !c.vm.stub_policy.allows_stub(&key) {
        return Err(VmError::UnboundNative(key));
    }
    c.vm.log.push(format!("UNIMPLEMENTED {key}"));
    match result {
        Some(kind) => Ok(c
            .vm
            .space
            .classes
            .default_value(&kind)
            .map_err(ow_object::heap::HeapError::from)?),
        None => Ok(Value::None),
    }
}

/// The C++ `Value::integer()` of the arguments read here.
fn integer(v: &Value) -> i64 {
    match v {
        Value::Int(i) => i64::from(*i),
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        Value::Float(f) => i64::from(ow_core::num::trunc_i64_wrap_i32(f64::from(*f))),
        _ => 0,
    }
}

/// A name or string argument's text (anything else: empty).
fn text(vm: &Vm, v: &Value) -> String {
    match v {
        Value::Name(n) => vm.space.names.text(*n).into_owned(),
        Value::Str(s) => s.to_string(),
        _ => String::new(),
    }
}

fn error(message: String) -> VmError {
    VmError::Script {
        function: "behavior kernel".to_string(),
        message,
    }
}

fn to_vm(e: BehaviorError) -> VmError {
    match e {
        BehaviorError::Vm(v) => v,
        other => error(other.to_string()),
    }
}
