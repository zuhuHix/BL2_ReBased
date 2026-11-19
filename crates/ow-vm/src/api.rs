//! Owner: lane W2-VMAPI. The VM contract of architecture section 3.8: [`Vm`], [`NativeCall`], [`NativeFn`],
//! [`NativePack`], [`StubPolicy`], and the [`WorldHooks`] trait (globals, game info, engine queries) that this lane
//! freezes and `ow-world` implements, held as a service `Rc<RefCell<dyn WorldHooks>>`.
//!
//! **What is here.** A VM that owns everything a native needs (architecture 3.9): the object space, the definition
//! store, the attribute system as `Rc`, the game RNG streams, the attribute global slots, the simulation clock and the
//! parity mode, which the caller passes in as a value (library code never reads `OW_PARITY`). It keeps a function
//! table, registers natives, iterators and script overrides, holds services, and calls a native directly. `Vm` itself
//! implements [`ow_attr::api::AttrWorld`], so the attribute system evaluates against the running world through
//! `EvalCx::new(&mut vm)`.
//!
//! **The interpreter** is [`crate::interp`] (W3-INTERP): [`Vm::run_frame`] runs one bytecode frame there, and
//! [`Vm::find_function`] / [`Vm::find_class`] load functions and classes from the code packages of the definition
//! store (`DefStore::code_mut`) when they are not declared yet. A script function declared by hand
//! ([`Vm::declare_function`], no package export behind it) has no bytecode: calling it without an override is still
//! `NotYet { NotPorted }`. With a store that holds no packages at all, an undeclared path is `NotYet { LayerAbsent }`
//! as before. `instantiate_export` waits for `ow_object::prefix` (W3-DECODE). The typed let of an attribute (opcode
//! 0x5F) converts the right-hand side with the VM's own assignment rule (`interp.cpp:511-522`) **before** it calls
//! `ow_attr::let_base`, which refuses a right-hand side of another kind.
//!
//! **Single-threaded on purpose** (decision D18): the VM holds `Rc` values and is `!Send`; a compile-time check
//! below stops the build if that changes.
//!
//! **The borrow rule for natives.** A native reaches services through `call.vm.service::<T>()`, borrows the cell for
//! a short scope and never across [`NativeCall::call_script`] (checked: any call while a service is borrowed is
//! [`VmError::ServiceBorrowed`]). To use the attribute system from a native, clone the `Rc` out first, then lend
//! the whole VM: `let attrs = call.vm.attrs.clone(); attrs.value_of(a, Some(s), None, &mut EvalCx::new(call.vm))`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, AttrWorld, EngineQuery, GlobalSlots, NumForms, WorldParts};
use ow_core::rand::{CrtStream, GameRng, Rand15};
use ow_core::sim::SimClock;
use ow_core::{DetMap, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{HeapError, ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropId};
use ow_object::layout::{LayoutError, PropertyDecl};
use ow_object::value::Value;

use crate::error::VmError;
use crate::services::Services;

// ---------------------------------------------------------------------------------------------------------------
// The upward hook
// ---------------------------------------------------------------------------------------------------------------

/// The one upward hook the VM's [`AttrWorld`] implementation calls; frozen by W2-VMAPI, implemented by `ow-world`.
///
/// Held as a service (`Rc<RefCell<dyn WorldHooks>>`, [`Vm::set_world_hooks`]): the VM clones the `Rc` out of its
/// service map, borrows it, and passes its own parts, so no borrow of the map is live. The hooks get the object
/// space and the store, not the VM, so they cannot run script. Balance state (region stages, playthrough) is not
/// part of this trait: it is a separate service whose type lives in `ow-world` (it names `ow-balance` types, which
/// `ow-vm` may not).
///
/// The running game always has a globals object and a game info object (`progression.cpp:84,87`), so both answer an
/// object or an error, never "none".
pub trait WorldHooks {
    /// The engine globals object (`AttrWorld::globals`).
    fn globals(&mut self, w: WorldParts<'_>) -> Result<ObjRef, AttrError>;
    /// The game info object (`AttrWorld::game_info`).
    fn game_info(&mut self, w: WorldParts<'_>) -> Result<ObjRef, AttrError>;
    /// An engine query of a context resolver (`AttrWorld::engine_query`). `Ok(None)`: ran, found nothing.
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
        w: WorldParts<'_>,
    ) -> Result<Option<ObjRef>, AttrError>;
}

/// The represented engine world's first WorldInfo, or an explicitly absent world (`Ok(None)`).
/// An unrepresented world must return an error. Installed by the world owner; a VM without this query cannot
/// infer absence from a missing service or from its simulation clock. UNVERIFIED: NATIVE_RESOURCE_POOL_IDLE.md.
#[derive(Debug, Clone, Copy)]
pub struct WorldTimeQuery(pub fn(&Vm) -> Result<Option<ObjRef>, VmError>);

// ---------------------------------------------------------------------------------------------------------------
// Policies and limits
// ---------------------------------------------------------------------------------------------------------------

/// What a call of something with no implementation does. Each command chooses one (architecture 3.8, "Stub policy
/// per command"):
///
/// | Command | Policy |
/// |---|---|
/// | `--native`, `--native-args`, `--native-selftest` | `LogAndZero` (`natives_test.py:16-20`, status `unimplemented`) |
/// | `--run`, `--run-batch` | `LogAndZero` (`vm_test.py:818`, an iterator native with nothing bound) |
/// | `--native-census`, `--vm-sweep` | `LogAndZero` (`vm_test.py:886-889`, a stub counted once) |
/// | `--behavior-run`, `--kismet-run`, `--mission-run`, `--slice-run`, `--combat-run`, `--inventory-move`, `--mover-event`, every `ow-host` world | `Refuse { allow }`, the allow list with a reason per entry kept by the command |
///
/// The logged zero result of the exploration commands is their declared behaviour (the Phase 2 harness), not a
/// stand-in for a game rule. [`Vm::new`] starts with `Refuse` and an empty allow list, so a VM nobody configured fails
/// loudly on the first unbound native.
///
/// **Every stub site goes through the policy**, not only unbound natives. Each consults [`StubPolicy::allows_stub`]
/// with the key below, logs the C++ text when allowed, and is [`VmError::UnboundNative`] (carrying the key) under
/// `Refuse` otherwise. The interpreter (W3-INTERP) calls `Vm::stub_site(key, text)` for the five sites that are its:
///
/// | Site (C++) | Key | Logged text |
/// |---|---|---|
/// | unbound native (`interp.cpp:692`, here in `Vm::call`) | the native key `Class.Name(types)` | `UNIMPLEMENTED <key>` |
/// | function name not found on the object (`interp.cpp:634`) | `<Class>.<Name>` | `UNIMPLEMENTED function <Class>.<Name>` |
/// | native index with no function (`interp.cpp:663`) | `native index <n>` (decimal) | `UNIMPLEMENTED native index <n>` |
/// | primitive cast not handled (`interp.cpp:217`) | `primitive cast 0x<hh>` (two lower-case hex digits) | `UNIMPLEMENTED primitive cast 0x<hh>` |
/// | `DynArraySort` (`interp.cpp:467`) | `DynArraySort` | `UNIMPLEMENTED DynArraySort` |
/// | `foreach` over an iterator native with no iterator bound (`interp.cpp:866`) | the iterator native's key | `UNIMPLEMENTED native iterator (foreach over an engine iterator)` |
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StubPolicy {
    /// Log the site's text, leave the out arguments as they are and return the zero value (the return type's, or None
    /// for a function with no return value).
    LogAndZero,
    /// A stub site is [`VmError::UnboundNative`], except the keys in `allow`, which are stubbed as under
    /// `LogAndZero`.
    Refuse { allow: BTreeSet<String> },
}

impl StubPolicy {
    /// `Refuse` with nothing allowed.
    pub fn refuse_all() -> Self {
        StubPolicy::Refuse {
            allow: BTreeSet::new(),
        }
    }

    /// Whether the stub site `key` may run as a logged zero-result stub.
    pub fn allows_stub(&self, key: &str) -> bool {
        match self {
            StubPolicy::LogAndZero => true,
            StubPolicy::Refuse { allow } => allow.contains(key),
        }
    }
}

/// The VM's run limits.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Executed steps per top-level call, as the C++ `Runtime::stepLimit` (5,000,000). Every call counts one step;
    /// the interpreter (W3-INTERP) adds one per executed expression through [`Vm::tick`], as the C++ `Interp::tick`.
    pub step_limit: u64,
    /// Nested bytecode frames, as the C++ `MAX_CALL_DEPTH` (200). The counter lives in the `Vm`, so it carries
    /// through a native's call back into script, as the C++ interpreter's `depth` does. Natives and script functions
    /// a binding overrides are not counted (C++ checks the depth only before making a bytecode frame).
    pub max_call_depth: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            step_limit: 5_000_000,
            max_call_depth: 200,
        }
    }
}

/// How many objects may be answering one `QueryInterface` question at once ([`Vm::query_interface`]).
pub const MAX_INTERFACE_DEPTH: usize = 64;

/// The call census, off unless [`Vm::census`] is `Some` (the C++ `countCalls` counters, `vm.hpp`). Keys are function
/// paths; counters accumulate until the caller clears them. `DetMap` because the C++ counters are `std::map` and
/// the census output prints them in key order.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallCensus {
    /// Script functions entered (a script function a binding overrides counts here, as in C++).
    pub script_calls: DetMap<String, u64>,
    /// Natives that ran an implementation.
    pub native_calls: DetMap<String, u64>,
    /// Natives with no implementation that ran as a logged stub (or were refused).
    pub stub_calls: DetMap<String, u64>,
    /// Script function -> context expressions whose object was None (filled by the interpreter, W3-INTERP).
    pub none_contexts: DetMap<String, u64>,
}

// ---------------------------------------------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------------------------------------------

/// One function the VM can call, as the loader (W3-INTERP) or a test declares it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    /// `Package.Class.Function` (C++ `Function::path`); unique in one VM, compared case-insensitively.
    pub path: ObjectPath,
    /// The class that declares it (or owns the state it is declared in).
    pub owner: Option<ClassId>,
    /// The registry key `Class.Name(type,type)` for a native (C++ `Function::nativeKey`, built by the loader exactly
    /// as `vm.cpp:600-606` does); `None` for a script function.
    pub native_key: Option<String>,
    /// `static` (C++ `FUNC_Static`).
    pub is_static: bool,
    /// Parameters in declaration order, return value excluded; `PropertyDecl::is_out` / `is_optional` hold.
    pub params: Vec<PropertyDecl>,
    /// The return value, if any.
    pub result: Option<PropertyDecl>,
}

impl FunctionDecl {
    /// A script function (no native key).
    pub fn script(path: ObjectPath) -> Self {
        FunctionDecl {
            path,
            owner: None,
            native_key: None,
            is_static: false,
            params: Vec::new(),
            result: None,
        }
    }

    /// A native function with its registry key.
    pub fn native(path: ObjectPath, key: impl Into<String>) -> Self {
        FunctionDecl {
            native_key: Some(key.into()),
            ..FunctionDecl::script(path)
        }
    }

    pub fn with_owner(mut self, owner: ClassId) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn with_static(mut self, is_static: bool) -> Self {
        self.is_static = is_static;
        self
    }

    pub fn with_params(mut self, params: Vec<PropertyDecl>) -> Self {
        self.params = params;
        self
    }

    pub fn with_result(mut self, result: PropertyDecl) -> Self {
        self.result = Some(result);
        self
    }

    /// Whether it is native (has a registry key).
    pub fn is_native(&self) -> bool {
        self.native_key.is_some()
    }

    /// The registry key of a native, the path of a script function: what the stub log line names.
    pub fn display_key(&self) -> &str {
        self.native_key.as_deref().unwrap_or(self.path.as_str())
    }
}

struct Function {
    decl: FunctionDecl,
    /// A host binding that runs instead of the bytecode (`override_script`).
    binding: Option<NativeFn>,
}

// ---------------------------------------------------------------------------------------------------------------
// Natives
// ---------------------------------------------------------------------------------------------------------------

/// A native implementation: reads its arguments from the call, may write `out` arguments and returns the result
/// (`Value::None` for a function with no return value).
pub type NativeFn = Rc<dyn Fn(&mut NativeCall<'_>) -> Result<Value, VmError>>;

/// A class's own answer to `Object.QueryInterface` (the engine asks the object's class for it; lane
/// IMPL-PAWN-AI-INTERFACE, decision 2026-11-19-62): called with the asked object and the interface class, `Ok(None)`
/// for no answer. Registered per class with [`Vm::register_interface_rule`]; an object of a class with none (and no
/// parent with one) answers by [`Vm::plain_interface`].
pub type InterfaceRuleFn = Rc<dyn Fn(&mut Vm, ObjRef, ClassId) -> Result<Option<ObjRef>, VmError>>;

/// What an engine iterator native (`Actor.AllActors`, `Actor.DynamicActors`) gives a `foreach`: the values the loop
/// visits, in order, each assigned to the call's out parameter `out_param` (C++ `IteratorResult`).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IteratorResult {
    pub out_param: usize,
    pub items: Vec<Value>,
}

impl IteratorResult {
    pub fn new(out_param: usize, items: Vec<Value>) -> Self {
        IteratorResult { out_param, items }
    }
}

/// An iterator native (same key form as a native).
pub type IteratorFn = Rc<dyn Fn(&mut NativeCall<'_>) -> Result<IteratorResult, VmError>>;

/// A change-notification override for one class (NATIVE_ATTRIBUTES.md section 4: `ResourcePool` clamps
/// `CurrentValue`, `Skill` flags a refresh on `Grade`). It gets the whole VM, so it may run script, and the calling
/// evaluation's forms, for a nested `EvalCx::new(vm).with_forms(forms)`.
pub type AttrNotifyFn = Rc<dyn Fn(&mut Vm, ObjRef, PropId, NumForms) -> Result<(), AttrError>>;

/// One argument of a native call.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    /// The value as the call was bound. For an out argument from script, the live value is the caller's storage
    /// ([`NativeCall::arg`] reads it); this field holds what the native last wrote through [`NativeCall::set_out`].
    pub value: Value,
    /// An `out` parameter: the native writes it with [`NativeCall::set_out`]. From script, the write goes to the
    /// caller's storage at once (the C++ alias); from the host, the caller sees it in [`CallResult::outs`].
    pub out: bool,
    /// False for an omitted optional parameter (its value is `Value::None`, as the C++ leaves it).
    pub supplied: bool,
    /// The caller's storage an out argument from script aliases (the C++ `NativeCall::Arg::alias`), set by the
    /// interpreter (W3-INTERP): a script callee and [`NativeCall::arg`] read it live, and [`NativeCall::set_out`]
    /// writes through it at once, so script a native calls back into sees the write. `None` for a call from the host.
    /// A copy of the argument kept past its call (an `Arg` is `Clone`) still names the caller's frame: once that frame
    /// has returned, reading or writing through it is a typed stale-frame `VmError::Script`, never a read of or a
    /// write into the frame that took its place.
    pub(crate) alias: Option<crate::interp::Alias>,
}

impl Arg {
    pub fn new(value: Value, out: bool, supplied: bool) -> Self {
        Arg {
            value,
            out,
            supplied,
            alias: None,
        }
    }

    /// An out argument aliasing the caller's storage (W3-INTERP).
    pub(crate) fn aliasing(value: Value, alias: crate::interp::Alias) -> Self {
        Arg {
            value,
            out: true,
            supplied: true,
            alias: Some(alias),
        }
    }
}

/// What a call returns: the result and every argument as it stands after the call (out parameters updated), as the
/// C++ `Runtime::call(.., outs)`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct CallResult {
    pub ret: Value,
    pub outs: Vec<Value>,
}

/// One native call as the implementation sees it. Built by the VM (and by the interpreter, through
/// [`NativeCall::new`]). The arguments are read with [`NativeCall::arg`] and out arguments written only with
/// [`NativeCall::set_out`], so an out argument from script always reads and writes the caller's storage.
#[non_exhaustive]
pub struct NativeCall<'a> {
    pub vm: &'a mut Vm,
    pub function: FuncId,
    pub this: Option<ObjRef>,
    pub(crate) args: Vec<Arg>,
}

impl<'a> NativeCall<'a> {
    pub fn new(vm: &'a mut Vm, function: FuncId, this: Option<ObjRef>, args: Vec<Arg>) -> Self {
        NativeCall {
            vm,
            function,
            this,
            args,
        }
    }

    /// Argument `i`: for an out argument from script, the caller's storage as it is now (the C++ read through the
    /// alias), so a write through another argument naming the same variable, or by script the native called back
    /// into, shows. An index past the declared parameters is [`VmError::BadArgument`] (a native's bug, reported, not
    /// a panic).
    pub fn arg(&self, i: usize) -> Result<&Value, VmError> {
        match self.args.get(i) {
            Some(Arg {
                alias: Some(alias), ..
            }) => crate::interp::alias_value(self.vm, alias),
            Some(a) => Ok(&a.value),
            None => Err(self.bad_arg(i, "no such argument")),
        }
    }

    /// How many arguments the call has (one per declared parameter).
    pub fn arg_count(&self) -> usize {
        self.args.len()
    }

    /// Writes `out` argument `i`, the only way to write one. From script, the value goes to the caller's storage at
    /// once (the C++ write through the alias); from the host, the caller sees it in [`CallResult::outs`]. Writing a
    /// parameter that is not `out` is refused.
    pub fn set_out(&mut self, i: usize, v: Value) -> Result<(), VmError> {
        match self.args.get_mut(i) {
            Some(a) if a.out => {
                let alias = a.alias.clone();
                a.value = v.clone();
                // An out argument from script writes through to the caller's storage at once, as the C++ alias.
                if let Some(alias) = alias {
                    crate::interp::write_alias(self.vm, &alias, v)?;
                }
                Ok(())
            }
            Some(_) => Err(self.bad_arg(i, "not an out parameter")),
            None => Err(self.bad_arg(i, "no such argument")),
        }
    }

    /// Whether argument `i` exists and was supplied (C++ `NativeCall::has`).
    pub fn has(&self, i: usize) -> bool {
        self.args.get(i).is_some_and(|a| a.supplied)
    }

    /// Calls a function from inside this native, sharing the running step budget and call depth (as C++
    /// `callFromNative`). No service may be borrowed across it ([`VmError::ServiceBorrowed`]).
    pub fn call_script(
        &mut self,
        f: FuncId,
        this: Option<ObjRef>,
        args: Vec<Value>,
    ) -> Result<CallResult, VmError> {
        self.vm.call(f, this, args)
    }

    fn bad_arg(&self, index: usize, reason: &'static str) -> VmError {
        VmError::BadArgument {
            function: self.vm.function_path(self.function),
            index,
            reason,
        }
    }
}

/// Every crate above the VM exports one (`pub struct Pack; impl NativePack for Pack`), registering its natives,
/// iterators, overrides, notification overrides and services.
pub trait NativePack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError>;
}

// ---------------------------------------------------------------------------------------------------------------
// The VM
// ---------------------------------------------------------------------------------------------------------------

/// The script VM and everything a native needs (architecture 3.8, 3.9).
pub struct Vm {
    pub space: ObjectSpace,
    pub defs: DefStore,
    /// Immutable; clone the `Rc` out before lending the VM to an [`ow_attr::api::EvalCx`].
    pub attrs: Rc<AttributeSystem>,
    pub rng: GameRng,
    /// `GlobalAttributeValueResolver`'s two slots. The world owns them: the game keeps one process-wide pair
    /// (`AttrWorld::global_slots`).
    pub global_slots: GlobalSlots,
    pub clock: SimClock,
    /// The mode the binary read once and passed in; never read from the environment here.
    pub parity: ParityMode,
    /// `Log()` output and every `UNIMPLEMENTED ...` stub line (C++ `Runtime::log`).
    pub log: Vec<String>,
    pub limits: Limits,
    pub stub_policy: StubPolicy,
    /// The call census, off when `None`.
    pub census: Option<CallCensus>,
    /// Call order, off when `None`: every call appends its function path as it starts (C++ `callTrace`).
    pub trace: Option<Vec<String>>,
    functions: Vec<Function>,
    function_by_path: DetMap<ObjectPath, FuncId>,
    natives: DetMap<String, NativeFn>,
    iterators: DetMap<String, IteratorFn>,
    notify: DetMap<ObjectPath, AttrNotifyFn>,
    /// Per-class `QueryInterface` answers ([`InterfaceRuleFn`]).
    interface_rules: DetMap<ClassId, InterfaceRuleFn>,
    /// The (object, interface) questions being answered, outermost first ([`Vm::query_interface`]).
    interface_asking: Vec<(ObjRef, ClassId)>,
    services: Services,
    /// The next number per class for an object made without a name (`<Class>_<n>`).
    unique_names: DetMap<ClassId, u32>,
    /// Steps used by the running top-level call.
    steps: u64,
    /// Calls in progress (0: the next call is a top-level call).
    active_calls: u32,
    /// Bytecode frames in progress, across natives (checked against `Limits::max_call_depth`).
    script_depth: u32,
    /// What the interpreter loaded and keeps per VM (lane W3-INTERP): the functions of the code packages, their
    /// bytecode, the classes whose functions and states are loaded, the native index, the active script frames.
    pub(crate) code: crate::interp::CodeState,
}

/// `Vm` stays `!Send` (decision D18). Ambiguous impls make this fail to compile if `Vm` ever becomes `Send`.
const _: fn() = || {
    trait AmbiguousIfSend<A> {
        fn some_item() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    struct IsSend;
    impl<T: ?Sized + Send> AmbiguousIfSend<IsSend> for T {}
    let _ = <Vm as AmbiguousIfSend<_>>::some_item;
};

impl std::fmt::Debug for Vm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vm")
            .field("parity", &self.parity)
            .field("functions", &self.functions.len())
            .field("natives", &self.natives.len())
            .field("stub_policy", &self.stub_policy)
            .field("services", &self.services)
            .finish_non_exhaustive()
    }
}

impl Vm {
    /// A VM over `space`, `defs` and `attrs`. The RNG is `GameRng::from_world_seed(0, mode)` (another seed:
    /// [`Vm::with_world_seed`]), the clock is at 0, the global slots are 0, the stub policy refuses every stub site,
    /// census and trace are off. `mode` is the value the binary read once (`parity_mode()` in `main`); tests pass
    /// either mode.
    pub fn new(
        space: ObjectSpace,
        defs: DefStore,
        attrs: Rc<AttributeSystem>,
        mode: ParityMode,
    ) -> Self {
        Vm {
            space,
            defs,
            attrs,
            rng: GameRng::from_world_seed(0, mode),
            global_slots: GlobalSlots::default(),
            clock: SimClock::new_empty(),
            parity: mode,
            log: Vec::new(),
            limits: Limits::default(),
            stub_policy: StubPolicy::refuse_all(),
            census: None,
            trace: None,
            functions: Vec::new(),
            function_by_path: DetMap::new(),
            natives: DetMap::new(),
            iterators: DetMap::new(),
            notify: DetMap::new(),
            interface_rules: DetMap::new(),
            interface_asking: Vec::new(),
            services: Services::new(),
            unique_names: DetMap::new(),
            steps: 0,
            active_calls: 0,
            script_depth: 0,
            code: crate::interp::CodeState::default(),
        }
    }

    /// Reseeds the RNG streams from `seed`, in the VM's own parity mode. That the streams' mode matches
    /// [`Vm::parity`] (no `Off` VM on the Reference stand-in streams, nor the reverse) is checked on every call in
    /// debug builds; `rng` and `parity` are public fields, so this constructor is the safe way to reseed.
    pub fn with_world_seed(mut self, seed: u64) -> Self {
        self.rng = GameRng::from_world_seed(seed, self.parity);
        self
    }

    pub fn with_stub_policy(mut self, policy: StubPolicy) -> Self {
        self.stub_policy = policy;
        self
    }

    // ---- classes, functions, objects -------------------------------------------------------------------------

    /// The class at `path` (`Package.Class`, C++ `Runtime::findClass`): one the class table has, or one loaded from
    /// the code packages of the definition store, with its functions and states (W3-INTERP, [`crate::interp`]).
    /// A path no package holds is the C++ error (`class not found: <path>`, the store's text for a missing
    /// package); with a store that holds no packages at all, `NotYet { LayerAbsent }`.
    pub fn find_class(&mut self, path: &str) -> Result<ClassId, VmError> {
        crate::interp::find_class(self, path)
    }

    /// [`Vm::find_class`], with a class no installed package holds (or a VM with no code layer) as `None`; any other
    /// lookup failure is kept as the error (lane FIX-RUN-BEHAVIORS, review r1 of #439).
    pub fn find_class_if_present(&mut self, path: &str) -> Result<Option<ClassId>, VmError> {
        crate::interp::find_class_if_present(self, path)
    }

    /// Adds a function to the table and returns its id. A path declared twice is refused.
    pub fn declare_function(&mut self, decl: FunctionDecl) -> Result<FuncId, VmError> {
        if self.function_by_path.contains_key(&decl.path) {
            return Err(VmError::Registration {
                key: decl.path.as_str().to_string(),
                reason: "a function with this path is already declared",
            });
        }
        let raw = u32::try_from(self.functions.len()).map_err(|_| VmError::Registration {
            key: decl.path.as_str().to_string(),
            reason: "the function table is full",
        })?;
        let id = FuncId::new(raw);
        self.function_by_path.insert(decl.path.clone(), id);
        self.functions.push(Function {
            decl,
            binding: None,
        });
        Ok(id)
    }

    /// The function at `path` (`Package.Class.Function`, C++ `Runtime::findFunction`): a declared one, or one loaded
    /// from the code packages of the definition store with its class (W3-INTERP, [`crate::interp`]). A path no
    /// package holds is the C++ error (`function not found: <path>`, `not a function: <path>`); with a store that
    /// holds no packages at all, `NotYet { LayerAbsent }`.
    pub fn find_function(&mut self, path: &str) -> Result<FuncId, VmError> {
        if let Some(found) = self.declared_function(path) {
            return Ok(found);
        }
        crate::interp::find_function(self, path)
    }

    /// The declared function at `path`, if any (paths compare case-insensitively).
    pub(crate) fn declared_function(&self, path: &str) -> Option<FuncId> {
        ObjectPath::parse(path)
            .ok()
            .and_then(|p| self.function_by_path.get(&p).copied())
    }

    /// The declaration of `f`, if this VM made it.
    pub fn function(&self, f: FuncId) -> Option<&FunctionDecl> {
        self.functions.get(f.index()).map(|x| &x.decl)
    }

    pub(crate) fn function_path(&self, f: FuncId) -> String {
        match self.function(f) {
            Some(d) => d.path.as_str().to_string(),
            None => format!("{f:?}"),
        }
    }

    /// A new runtime object of class `c` (no outer), starting from the class's default slots.
    ///
    /// An empty `name`:
    /// - `ParityMode::Off`: `<Class>_<n>` with a per-class counter starting at 0, the form UE3's
    ///   `MakeUniqueObjectName` gives an unnamed object. Status: UNVERIFIED for this executable: the exact numbering
    ///   (per-class counter, no check against an existing object of that name) is not read from it.
    /// - `ParityMode::Reference`: `<Class>_0` for every unnamed object, as the C++ VM names them (`vm.cpp:1083`;
    ///   several per class in `combat.cpp:325`, `combat_hit.cpp:173`, `combat_loot.cpp:435`, `EX_New`
    ///   `interp.cpp:472`). A Reference-only stand-in (architecture section 6.1, owner W7-RETIRE).
    ///
    /// Both give `<Class>_0` for the first unnamed object of a class (`vm_test.py:937-938`).
    pub fn instantiate(&mut self, c: ClassId, name: &str) -> Result<ObjRef, VmError> {
        if !name.is_empty() {
            return Ok(self.space.alloc(c, name, None, Origin::Runtime)?);
        }
        let layout = self
            .space
            .classes
            .get_layout(c)
            .ok_or(HeapError::Layout(LayoutError::UnknownClass(c)))?;
        let next = match self.parity {
            ParityMode::Off => self.unique_names.get(&c).copied().unwrap_or(0),
            ParityMode::Reference => 0,
        };
        let generated = format!("{}_{next}", layout.path().name());
        let object = self.space.alloc(c, &generated, None, Origin::Runtime)?;
        if self.parity == ParityMode::Off {
            self.unique_names.insert(c, next.wrapping_add(1));
        }
        Ok(object)
    }

    // ---- registration ----------------------------------------------------------------------------------------

    /// Binds the native implementation `f` to `key` (`Class.Name(type,type)`). A key registered twice is refused
    /// (C++ "duplicate native registration"). Functions declared before or after with this key use it.
    pub fn register_native(&mut self, key: &str, f: NativeFn) -> Result<(), VmError> {
        if self.natives.contains_key(key) {
            return Err(VmError::Registration {
                key: key.to_string(),
                reason: "duplicate native registration",
            });
        }
        self.natives.insert(key.to_string(), f);
        Ok(())
    }

    /// Whether a native implementation is registered for `key`.
    pub fn has_native(&self, key: &str) -> bool {
        self.natives.contains_key(key)
    }

    /// Whether `f` could be called here, checked without running it (lane IMPL-IS-FRIENDLY, review r3 of #449): a
    /// native needs a registered implementation, whatever the stub policy; a script function needs a package export
    /// whose script prepares. The error says why it cannot.
    pub fn prepare_call(&mut self, f: FuncId) -> Result<(), VmError> {
        let Some(d) = self.function(f) else {
            return Err(VmError::Script {
                function: format!("{f:?}"),
                message: "no such function in this VM".to_string(),
            });
        };
        if let Some(key) = d.native_key.clone() {
            return if self.has_native(&key) { Ok(()) } else { Err(VmError::UnboundNative(key)) };
        }
        let path = d.path.as_str().to_string();
        if crate::interp::prepare_script(self, f)? {
            Ok(())
        } else {
            Err(VmError::Script {
                function: path,
                message: "declared by hand, with no package export and so no bytecode".to_string(),
            })
        }
    }

    /// Binds an iterator native to `key`. A key registered twice is refused (C++ "duplicate iterator registration").
    pub fn register_iterator(&mut self, key: &str, f: IteratorFn) -> Result<(), VmError> {
        if self.iterators.contains_key(key) {
            return Err(VmError::Registration {
                key: key.to_string(),
                reason: "duplicate iterator registration",
            });
        }
        self.iterators.insert(key.to_string(), f);
        Ok(())
    }

    /// The iterator bound to native `f`'s key, if any (C++ `iteratorFor`). The interpreter's `foreach` uses it.
    pub fn iterator_for(&self, f: FuncId) -> Option<IteratorFn> {
        let key = self.function(f)?.native_key.as_deref()?;
        self.iterators.get(key).cloned()
    }

    /// Whether `class` implements the interface class `iface` (C++ `Runtime::implements`, lane W4-INTERP2): `iface`
    /// is an interface class and the interface table of `class` or of a super class names `iface` or a class derived
    /// from it (`CodeIndex::implements`, NATIVE_CLASS_SERIAL_LAYOUT.md, UNVERIFIED in the running game). A chain whose
    /// bodies did not all decode (synthetic packages) falls back to the structural rule, as the C++ does. A class
    /// with no package export behind it (one defined by hand) implements nothing. Being a child of `iface` is not
    /// asked here: the callers (`QueryInterface`, the casts) ask the class table first, as the C++ does.
    ///
    /// **Callers apply the stand-in check.** This is a plain `bool`. When the answer is "no" and `class`'s chain may
    /// run through an engine-native stand-in class (decision 2026-11-15-08), the "no" is not proven: a caller asks
    /// `interp::is_child(vm, class, iface)?` after a "no" and so answers `NotYet` there, as `DynamicCast`,
    /// `InterfaceCast`, `QueryInterface` and the iterators' InterfaceClass filter do. `interp::is_child` is
    /// crate-internal, so a caller outside `ow-vm` cannot follow this rule yet and must not treat a "no" as settled;
    /// a public checked form is open (decision 2026-11-15-20).
    pub fn implements(&mut self, class: ClassId, iface: ClassId) -> bool {
        let (code, store) = self.defs.code_mut();
        match (code.key_of(class), code.key_of(iface)) {
            (Some(c), Some(i)) => code.implements(store, c, i),
            _ => false,
        }
    }

    /// [`Vm::implements`] with the stand-in check applied, for callers outside `ow-vm` (the public checked form
    /// decision 2026-11-15-20 left open; lane FIX-RUN-BEHAVIORS, review r1 of #439): a "no" is asked again through
    /// the class chain, which is `NotYet` when that chain runs through an engine-native stand-in class, as the casts
    /// answer. So `Ok(false)` is a proven "no".
    pub fn implements_checked(&mut self, class: ClassId, iface: ClassId) -> Result<bool, VmError> {
        if self.implements(class, iface) {
            return Ok(true);
        }
        crate::interp::is_child(self, class, iface)
    }

    /// Binds `class`'s own `QueryInterface` answer (an [`InterfaceRuleFn`]); its subclasses use it too unless one has
    /// its own. A class registered twice is refused, like a native key.
    pub fn register_interface_rule(&mut self, class: ClassId, f: InterfaceRuleFn) -> Result<(), VmError> {
        if self.interface_rules.contains_key(&class) {
            return Err(VmError::Registration {
                key: self.space.classes.layout(class).path().to_string(),
                reason: "duplicate interface rule registration",
            });
        }
        self.interface_rules.insert(class, f);
        Ok(())
    }

    /// Whether `class` itself (not a parent) has a `QueryInterface` answer registered.
    pub fn has_interface_rule(&self, class: ClassId) -> bool {
        self.interface_rules.contains_key(&class)
    }

    /// The `QueryInterface` answer `class` uses: its own, else its nearest parent's, else `None` (the plain answer).
    pub fn interface_rule_for(&self, class: ClassId) -> Option<InterfaceRuleFn> {
        let mut cursor = Some(class);
        while let Some(c) = cursor {
            if let Some(f) = self.interface_rules.get(&c) {
                return Some(f.clone());
            }
            cursor = self.space.classes.layout(c).super_class();
        }
        None
    }

    /// Whether the question (`this`, `iface`) is being answered further up the running chain.
    pub fn interface_asked(&self, this: ObjRef, iface: ClassId) -> bool {
        self.interface_asking.contains(&(this, iface))
    }

    /// The plain `Object` answer to `QueryInterface(iface)` (NATIVE_CONTROLLER_HELPERS.md, UNVERIFIED): `this` when its
    /// class is `iface` or derives from it, or implements it ([`Vm::implements`], parents' tables included); else
    /// none. A "no" through an engine-native stand-in class is `NotYet` (decision 2026-11-15-08), as the casts answer
    /// it.
    pub fn plain_interface(&mut self, this: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
        let own = self.space.heap.get(this)?.class;
        let yes = self.space.classes.is_child_of(own, iface) || self.implements(own, iface);
        if !yes {
            crate::interp::is_child(self, own, iface)?;
        }
        Ok(yes.then_some(this))
    }

    /// `this`'s answer to `QueryInterface(iface)`, as the `Object.QueryInterface` native gives it: its class's
    /// registered answer ([`Vm::interface_rule_for`]), else [`Vm::plain_interface`]. A question already being answered
    /// further up the same chain gets none at once, which ends a cycle (NATIVE_PAWN_AI_INTERFACE.md, section 2, "A
    /// question asked twice", UNVERIFIED); the asker takes the none as its step's answer. A question reaching an object
    /// while [`MAX_INTERFACE_DEPTH`] distinct objects are already answering it is an error, whichever answer that
    /// object would give (review r1 of #456, item 6; r2, item 3): no rule read so far hands a question on that far.
    pub fn query_interface(&mut self, this: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
        if self.interface_asked(this, iface) {
            return Ok(None);
        }
        // The cap comes before either answer (review r2 of #456, item 3): a plain-table object at the end of a full
        // chain is refused too.
        if self.interface_asking.len() >= MAX_INTERFACE_DEPTH {
            return Err(VmError::Script {
                function: "Object.QueryInterface".to_string(),
                message: format!("the question went through more than {MAX_INTERFACE_DEPTH} answering objects"),
            });
        }
        let class = self.space.heap.get(this)?.class;
        let Some(rule) = self.interface_rule_for(class) else {
            return self.plain_interface(this, iface);
        };
        self.interface_asking.push((this, iface));
        let answer = rule(self, this, iface);
        self.interface_asking.pop();
        answer
    }

    /// Replaces a **script** function with a host binding that runs instead of its bytecode (C++ `overrideScript`,
    /// for a call that only presents something, and architecture R2's offline rules reached through script). The
    /// function must be declared and not native, and is overridden once.
    pub fn override_script(&mut self, path: &str, f: NativeFn) -> Result<(), VmError> {
        let refused = |reason| VmError::Registration {
            key: path.to_string(),
            reason,
        };
        let id = ObjectPath::parse(path)
            .ok()
            .and_then(|p| self.function_by_path.get(&p).copied())
            .ok_or(refused("overrideScript needs a script function"))?;
        let function = &mut self.functions[id.index()];
        if function.decl.is_native() {
            return Err(refused("overrideScript needs a script function"));
        }
        if function.binding.is_some() {
            return Err(refused("the script function is already overridden"));
        }
        function.binding = Some(f);
        Ok(())
    }

    /// Binds the attribute change-notification override of the class at `class_path` (and its subclasses that have
    /// none of their own). Registered by the pack that owns the class; a class registered twice is refused.
    pub fn register_attribute_notify(
        &mut self,
        class_path: &str,
        f: AttrNotifyFn,
    ) -> Result<(), VmError> {
        let refused = |reason| VmError::Registration {
            key: class_path.to_string(),
            reason,
        };
        let path = ObjectPath::parse(class_path).map_err(|_| refused("not an object path"))?;
        if self.notify.contains_key(&path) {
            return Err(refused("the class already has a notification override"));
        }
        self.notify.insert(path, f);
        Ok(())
    }

    // ---- services --------------------------------------------------------------------------------------------

    /// The service of type `T`, cloned out (holding it borrows nothing). See the borrow rule in the module doc.
    pub fn service<T: 'static>(&self) -> Option<Rc<RefCell<T>>> {
        self.services.get::<T>()
    }

    /// Adds the service of type `T` and returns its cell; a second service of one type is refused.
    pub fn add_service<T: 'static>(&mut self, t: T) -> Result<Rc<RefCell<T>>, VmError> {
        self.services.add(t)
    }

    /// Sets the [`WorldHooks`] (`ow-world` does this once; a second set is refused).
    pub fn set_world_hooks(&mut self, hooks: Rc<RefCell<dyn WorldHooks>>) -> Result<(), VmError> {
        self.services.set_hooks(hooks)
    }

    /// The world hooks, if set.
    pub fn world_hooks(&self) -> Option<Rc<RefCell<dyn WorldHooks>>> {
        self.services.hooks()
    }

    /// Reads the represented World's stored float TimeSeconds; zero only for an explicitly absent world.
    /// UNVERIFIED in game (NATIVE_RESOURCE_POOL_IDLE.md); this does not advance or substitute the VM clock.
    pub fn world_time_seconds(&self) -> Result<f32, VmError> {
        let query = self.service::<WorldTimeQuery>().ok_or_else(|| {
            NotYet::layer_absent("the engine world-time query", "the world owner", "NATIVE_RESOURCE_POOL_IDLE.md")
        })?;
        let query = query.try_borrow().map_err(|_| VmError::ServiceBorrowed("WorldTimeQuery"))?.0;
        let Some(world) = query(self)? else { return Ok(0.0) };
        let class = self.space.heap.get(world)?.class;
        if let Some(prop) = self.space.prop(class, "TimeSeconds")
            && let Value::Float(time) = self.space.read(world, ow_object::ids::PropAddr::scalar(prop))?
        {
            return Ok(*time);
        }
        Err(NotYet::not_ported("the represented World's float TimeSeconds", "NATIVE_RESOURCE_POOL_IDLE.md").into())
    }

    // ---- calls -----------------------------------------------------------------------------------------------

    /// Calls `f` with `args` (fewer than the parameters: the rest are omitted, `Value::None` and not supplied; more:
    /// refused, C++ "too many arguments for"). Refused while any service is borrowed.
    ///
    /// From outside any call this is a top-level call with a fresh step budget (C++ `Runtime::call`). From inside a
    /// native it **shares** the running step budget and call depth, which is C++ `callFromNative`. This differs
    /// from C++ `Runtime::call` made inside a native, which starts a fresh `Interp` (new budget, depth 0;
    /// `interp.cpp:915-917`): the Rust VM has one entry point and gives it the `callFromNative` meaning.
    ///
    /// A native runs its implementation, or the stub policy decides. A script function runs its override binding, or
    /// its bytecode on the interpreter ([`crate::interp`]).
    ///
    /// **Stack.** Running script recurses (expression, call, frame, up to `Limits::max_call_depth` frames and
    /// [`crate::interp::MAX_NESTING`] expression levels). A caller must run the VM on a thread with at least
    /// [`crate::interp::STACK_SIZE`] bytes of stack, which covers the worst case in a debug build;
    /// [`crate::interp::on_vm_stack`] runs a closure on such a thread. Past the nesting bound a call is a typed
    /// [`VmError::Script`], never a stack overflow.
    pub fn call(
        &mut self,
        f: FuncId,
        this: Option<ObjRef>,
        args: Vec<Value>,
    ) -> Result<CallResult, VmError> {
        debug_assert_eq!(
            self.rng.mode(),
            self.parity,
            "the RNG streams' parity mode differs from the VM's"
        );
        self.services.check_none_borrowed()?;
        if self.active_calls == 0 {
            debug_assert_eq!(self.script_depth, 0, "a bytecode frame outlived its call");
            self.steps = 0;
        }
        let bound = self.bind_args(f, args)?;
        self.active_calls += 1;
        let result = self.invoke(f, this, bound);
        self.active_calls -= 1;
        result
    }

    /// [`Vm::call`] with arguments the interpreter bound itself (W3-INTERP): an omitted parameter may sit before a
    /// supplied one (`EmptyParmValue`), which a plain value list cannot say. `args` has one entry per parameter.
    pub(crate) fn call_bound(
        &mut self,
        f: FuncId,
        this: Option<ObjRef>,
        args: Vec<Arg>,
    ) -> Result<CallResult, VmError> {
        self.services.check_none_borrowed()?;
        if self.active_calls == 0 {
            debug_assert_eq!(self.script_depth, 0, "a bytecode frame outlived its call");
            self.steps = 0;
        }
        self.active_calls += 1;
        let result = self.invoke(f, this, args);
        self.active_calls -= 1;
        result
    }

    /// The steps the running (or the last) top-level call used, as the C++ `Runtime::steps` the `--run` output
    /// prints. A new top-level call starts it at 0.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// Whether [`Vm::call`] of `f` with `args` would be refused at the call's entry, without making the call or
    /// changing anything: the same argument binding, and for a script function the same entry checks
    /// (`interp::check_entry`, shared with the call). Lane FIX-RUN-BEHAVIORS, review r2 of #439: a walk checks
    /// every call before it makes the first.
    pub fn check_call(&mut self, f: FuncId, args: Vec<Value>) -> Result<(), VmError> {
        let bound = self.bind_args(f, args)?;
        crate::interp::check_entry(self, f, &bound)
    }

    fn bind_args(&self, f: FuncId, mut args: Vec<Value>) -> Result<Vec<Arg>, VmError> {
        let decl = self.function(f).ok_or_else(|| VmError::Script {
            function: format!("{f:?}"),
            message: "no such function in this VM".to_string(),
        })?;
        if args.len() > decl.params.len() {
            return Err(VmError::Script {
                function: decl.path.as_str().to_string(),
                message: format!(
                    "too many arguments ({} for {} parameters)",
                    args.len(),
                    decl.params.len()
                ),
            });
        }
        let given = args.len();
        args.resize(decl.params.len(), Value::None);
        Ok(args
            .into_iter()
            .zip(&decl.params)
            .enumerate()
            .map(|(i, (value, p))| Arg::new(value, p.is_out(), i < given))
            .collect())
    }

    /// One executed step against the running call's budget (C++ `Interp::tick`): every call takes one here, and the
    /// interpreter (W3-INTERP) one per executed expression.
    pub(crate) fn tick(&mut self) -> Result<(), VmError> {
        self.steps += 1;
        if self.steps > self.limits.step_limit {
            return Err(VmError::StepLimit);
        }
        Ok(())
    }

    /// A stub site of the table on [`StubPolicy`]: `Ok` (and `text` logged) when the policy allows `key`, else
    /// [`VmError::UnboundNative`] with `key`. The caller supplies the zero result.
    pub(crate) fn stub_site(&mut self, key: &str, text: String) -> Result<(), VmError> {
        if !self.stub_policy.allows_stub(key) {
            return Err(VmError::UnboundNative(key.to_string()));
        }
        self.log.push(text);
        Ok(())
    }

    /// One call (C++ `Interp::invoke`): a step, the trace and census entries, then the native, the stub policy or the
    /// script branch.
    fn invoke(
        &mut self,
        f: FuncId,
        this: Option<ObjRef>,
        args: Vec<Arg>,
    ) -> Result<CallResult, VmError> {
        self.tick()?;
        let function = &self.functions[f.index()];
        let path = function.decl.path.as_str().to_string();
        let native = function
            .decl
            .native_key
            .as_ref()
            .map(|key| (key.clone(), self.natives.get(key).cloned()));
        let binding = function.binding.clone();
        if let Some(trace) = &mut self.trace {
            trace.push(path.clone());
        }
        if let Some(census) = &mut self.census {
            let counter = match &native {
                None => &mut census.script_calls,
                Some((_, Some(_))) => &mut census.native_calls,
                Some((_, None)) => &mut census.stub_calls,
            };
            *counter.entry(path.clone()).or_insert(0) += 1;
        }
        match native {
            Some((_, Some(imp))) => self.run_native(&imp, f, this, args),
            Some((key, None)) => self.stub(f, &key, args),
            None => match binding {
                Some(imp) => self.run_native(&imp, f, this, args),
                None => self.run_bytecode(f, this, args),
            },
        }
    }

    /// The bytecode branch: only the depth bookkeeping, so the counter unwinds on every path out of the frame,
    /// errors included. The depth is counted in the `Vm` so that it carries through natives, and checked before a
    /// frame is made, as C++ (`interp.cpp:711`). The frame itself is [`Vm::run_frame`].
    fn run_bytecode(
        &mut self,
        f: FuncId,
        this: Option<ObjRef>,
        args: Vec<Arg>,
    ) -> Result<CallResult, VmError> {
        // The C++ `invoke` reports a function that cannot run, and decodes, before it checks the depth.
        crate::interp::prepare(self, f)?;
        if self.script_depth >= self.limits.max_call_depth {
            let path = self.function_path(f);
            // The C++ names the four innermost active script functions, oldest first (`interp.cpp:711-713`).
            let active = &self.code.stack;
            let mut trace = String::new();
            for caller in &active[active.len().saturating_sub(4)..] {
                trace.push_str(" <- ");
                trace.push_str(&self.function_path(*caller));
            }
            return Err(VmError::Script {
                message: format!("call depth limit reached in {path}{trace}"),
                function: path,
            });
        }
        self.script_depth += 1;
        let result = self.run_frame(f, this, args);
        self.script_depth -= 1;
        result
    }

    /// Runs one bytecode frame of script function `f` ([`crate::interp::run_frame`], lane W3-INTERP).
    fn run_frame(
        &mut self,
        f: FuncId,
        this: Option<ObjRef>,
        args: Vec<Arg>,
    ) -> Result<CallResult, VmError> {
        crate::interp::run_frame(self, f, this, args)
    }

    fn run_native(
        &mut self,
        imp: &NativeFn,
        f: FuncId,
        this: Option<ObjRef>,
        args: Vec<Arg>,
    ) -> Result<CallResult, VmError> {
        let mut call = NativeCall::new(self, f, this, args);
        let ret = imp(&mut call)?;
        // Every argument as it stands now (an aliased out argument read from the caller's storage).
        let outs = (0..call.args.len())
            .map(|i| call.arg(i).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CallResult { ret, outs })
    }

    /// An unbound native under the stub policy: refused, or logged with the return type's zero value and the out
    /// arguments as they were (C++ `interp.cpp:692-695`).
    fn stub(&mut self, f: FuncId, key: &str, args: Vec<Arg>) -> Result<CallResult, VmError> {
        self.stub_site(key, format!("UNIMPLEMENTED {key}"))?;
        let ret = match &self.functions[f.index()].decl.result {
            Some(r) => self
                .space
                .classes
                .default_value(&r.kind)
                .map_err(HeapError::from)?,
            None => Value::None,
        };
        Ok(CallResult {
            ret,
            outs: args.into_iter().map(|a| a.value).collect(),
        })
    }

    // ---- attribute notification ------------------------------------------------------------------------------

    /// The override for `owner`'s class: the nearest class in its super chain that has one.
    fn notify_override(&self, owner: ObjRef) -> Result<Option<AttrNotifyFn>, AttrError> {
        let mut class = Some(self.space.heap.get(owner)?.class);
        while let Some(c) = class {
            let layout = self
                .space
                .classes
                .get_layout(c)
                .ok_or(LayoutError::UnknownClass(c))?;
            if let Some(f) = self.notify.get(layout.path()) {
                return Ok(Some(f.clone()));
            }
            class = layout.super_class();
        }
        Ok(None)
    }

    /// Runs `f` on the world hooks with this VM's parts: `NotYet { LayerAbsent("ow-world") }` without hooks, an
    /// `AttrError::World` (not a panic) when the hooks are already borrowed.
    fn with_hooks<R>(
        &mut self,
        what: &'static str,
        f: impl FnOnce(&mut dyn WorldHooks, WorldParts<'_>) -> Result<R, AttrError>,
    ) -> Result<R, AttrError> {
        let hooks = self.services.hooks().ok_or_else(|| {
            AttrError::from(NotYet::layer_absent(
                what,
                "ow-world",
                "architecture.md section 3.9 (WorldHooks)",
            ))
        })?;
        let mut hooks = hooks.try_borrow_mut().map_err(|_| {
            AttrError::World(format!("{what}: the world hooks are already borrowed"))
        })?;
        f(&mut *hooks, self.parts())
    }
}

/// The running world as the attribute system sees it.
impl AttrWorld for Vm {
    fn parts(&mut self) -> WorldParts<'_> {
        WorldParts::new(&mut self.space, Some(&mut self.defs))
    }

    /// Through the [`WorldHooks`]; without them `NotYet { LayerAbsent("ow-world") }`.
    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        self.with_hooks("the engine globals object", |h, w| h.globals(w))
    }

    /// Through the [`WorldHooks`] (always an object or an error); without them `NotYet { LayerAbsent("ow-world") }`.
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        self.with_hooks("the game info object", |h, w| h.game_info(w).map(Some))
    }

    /// Through the [`WorldHooks`]; without them `NotYet { LayerAbsent("ow-world") }`.
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        self.with_hooks("an attribute engine query", |h, w| {
            h.engine_query(q, source, w)
        })
    }

    /// Runs the resolver class's script `GetAttributeContext` (virtual lookup on the resolver's class) with
    /// `Attribute` and `AttributeContextSource` bound by parameter name, compared without case as the name table
    /// keeps the first spelling it interned, any other parameter its zero value (lane FIX-DESIGNER-RESOLVER, decision
    /// 2026-11-16-62). A function that lacks either name is `NotYet { NotRead }`: the engine binds by position, so an
    /// override with other names is valid script, but this binding is by name and does not guess the order. What the script reaches is the VM's as for any call: an unbound
    /// or unread native stops it (its `NotYet`, or the stub policy's answer). A class with no such function, or whose
    /// function is native, is `NotYet { NotRead }`: no script rule to run. An answer that is not an object reference
    /// is bad data.
    fn script_context(
        &mut self,
        resolver: ObjRef,
        attribute: ObjRef,
        source: Option<ObjRef>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let class = self.space.heap.get(resolver)?.class;
        let class_path = self
            .space
            .classes
            .get_layout(class)
            .map(|l| l.path().as_str().to_string())
            .unwrap_or_default();
        let function = self.find_method(class, "GetAttributeContext", None)?;
        let Some(function) = function.filter(|f| self.function(*f).is_some_and(|d| !d.is_native())) else {
            return Err(NotYet::not_read(
                format!("{class_path}.GetAttributeContext: no script function to run"),
                "NATIVE_ATTRIBUTES.md section 5",
            )
            .into());
        };
        let params = self.function(function).map(|d| d.params.clone()).unwrap_or_default();
        // Both arguments are bound by name, without case (review r2 of PR #360); a function that lacks either name is
        // refused rather than given a zero (review r1).
        let named = |vm: &Vm, d: &PropertyDecl, wanted: &str| vm.space.names.text(d.name).eq_ignore_ascii_case(wanted);
        for wanted in ["Attribute", "AttributeContextSource"] {
            if !params.iter().any(|d| named(self, d, wanted)) {
                return Err(NotYet::not_read(
                    format!(
                        "{class_path}.GetAttributeContext has no parameter {wanted}: the parameters are bound by name"
                    ),
                    "NATIVE_ATTRIBUTES.md section 5",
                )
                .into());
            }
        }
        let args = params
            .iter()
            .map(|d| {
                if named(self, d, "Attribute") {
                    Value::Object(Some(attribute))
                } else if named(self, d, "AttributeContextSource") {
                    Value::Object(source)
                } else {
                    self.space.classes.zero_value(&d.kind)
                }
            })
            .collect();
        match self.call(function, Some(resolver), args)?.ret {
            Value::Object(o) | Value::Interface(o) => Ok(o),
            other => Err(AttrError::BadData {
                reason: format!(
                    "{class_path}.GetAttributeContext returned a {}, not an object",
                    other.kind_name()
                )
                .into(),
            }),
        }
    }

    /// The VM's clock: the running world's `TimeSeconds` (decision 2026-11-16-80).
    fn time_seconds(&mut self) -> Result<Option<f32>, AttrError> {
        Ok(Some(self.clock.time_seconds()))
    }

    /// Calls `GetCurrencyOnHand(form)` on `info` (virtual lookup on its loaded class code, in its state, as
    /// `ow_combat::hit::call_method` does), so the answer is whatever the VM has
    /// bound for it (`ow_combat::down` binds the native; unbound, the stub policy's answer or its `NotYet`). A class
    /// with no such function is `NotYet { NotRead }`; an answer that is not an int is bad data (decision
    /// 2026-11-16-80).
    fn currency_on_hand(&mut self, info: ObjRef, form: u8) -> Result<i32, AttrError> {
        // The bound combat native still uses the Reference zero stand-in for keys. Off cannot
        // answer until the owner controller's key count is represented (UNVERIFIED in game).
        if self.parity != ParityMode::Reference && form == 3 {
            return Err(NotYet::not_read(
                "GetCurrencyOnHand: the owner controller's golden-key count is not represented",
                "NATIVE_CONTROLLER_HELPERS.md (GetCurrencyOnHand)",
            )
            .into());
        }
        let (class, state) = {
            let o = self.space.heap.get(info)?;
            (o.class, o.state)
        };
        let Some(function) = self.find_method(class, "GetCurrencyOnHand", state)? else {
            return Err(NotYet::not_read(
                "GetCurrencyOnHand: the replication info's class has no such function",
                "NATIVE_ATTR_RESOLVERS.md section 2",
            )
            .into());
        };
        match self.call(function, Some(info), vec![Value::Byte(form)])?.ret {
            Value::Int(n) => Ok(n),
            other => Err(AttrError::BadData {
                reason: format!("GetCurrencyOnHand returned a {}, not an int", other.kind_name()).into(),
            }),
        }
    }

    /// The VM's own pair ([`Vm::global_slots`]).
    fn global_slots(&mut self) -> &mut GlobalSlots {
        &mut self.global_slots
    }

    /// Dispatches the owner class's notification override (NATIVE_ATTRIBUTES.md section 4), found by walking the
    /// owner's class chain; the override gets the whole VM (no heap borrow is held while it runs) and `forms`, and may
    /// run script. With no override in the chain nothing happens: the record reads the base implementation as doing
    /// nothing. Status: UNVERIFIED. Owners whose override the record leaves open (weapon `ClipSize`, inventory
    /// manager `WeaponReadyMax`, interactive object `MaxHealth`, game info `ShopTimerRate`) are the owning pack's to
    /// register, as a `NotYet` override until read.
    fn attribute_changed(
        &mut self,
        owner: ObjRef,
        value_prop: PropId,
        forms: NumForms,
    ) -> Result<(), AttrError> {
        match self.notify_override(owner)? {
            Some(f) => f(self, owner, value_prop, forms),
            None => Ok(()),
        }
    }

    /// The VM's `GameRng` C-`rand()` stream `s` (the VM always has one).
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Ok(self.rng.crt(s))
    }
}

#[cfg(test)]
mod tests;
