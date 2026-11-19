//! Owner: lane W5-DIALOG. The bounded `InterpActor` lifecycle adapter: the port of `src/mover.*`. AI-assisted
//! (Claude Code).
//!
//! **What it is.** Motion comes from the installed Matinee data in the host; this runs the placed mover's installed
//! script notifications (`InterpActor.InterpolationStarted` / `InterpolationFinished`) and the timers they set, on a
//! VM of its own (the Core natives, step limit 20000, unbound natives logged as `UNIMPLEMENTED ...` and answered with
//! zero, as the C++ `Runtime` does, or refused, as the caller chooses). It is a tool for checking that the installed script runs, not a model of the
//! game's movers: nothing here was seen in the running game, so every rule below is **UNVERIFIED** as a game rule
//! (`docs/verification/SANCTUARY_MOVER_PROTOTYPE.md` holds what was observed).
//!
//! **The binding** ([`Mover::new`]), in the C++ order, each failure a [`MoverError`] with the C++ text:
//! 1. the actor export is loaded as a placed actor (26-byte property prefix) and the action export as an ordinary
//!    object (4 bytes), from the definition store (the one object of that path, not a copy: decision
//!    2026-11-15-23). The C++ size check of the prefix ("object property prefix outside export") runs first, with
//!    the prefix the C++ passes; a prefix the store derives differently is the store's `PrefixMismatch`;
//! 2. the actor's class must be exactly `Engine.InterpActor` and the action's exactly `Engine.SeqAct_Interp`;
//! 3. the two notifications must be script functions taking `InterpAction` (an object) first, and
//!    `InterpolationStarted` `GroupInst` (an object) second, with exactly two and one parameters; each one's bytecode
//!    is decoded right after its check (`ow_script::decode::decode`, as the C++ decodes it here), so a body that does
//!    not decode fails the binding with the decoder's text;
//! 4. the action must declare `bReversePlayback` and the actor `bShouldSaveForCheckpoint`;
//! 5. a non-null `AmbientSoundComponent` must resolve to an export before it is loaded (the C++ `resourceIndex > 0`:
//!    a reference to an export, or an import that resolves to one), else "unresolved mover ambient component"; it is
//!    then loaded by the store (the C++ instantiates it at 8 bytes; the store derives the prefix, 8 for an
//!    `Engine.ActorComponent` subclass);
//! 6. `Engine.Actor.SetTimer`, `Engine.Actor.ClearTimer` and `Engine.AudioComponent.Stop` must be natives, and are
//!    bound to this mover (below);
//! 7. loading must leave no diagnostic in the VM log ("mover object loading emitted a diagnostic: ..."). The store's
//!    own `defs: no holder for <path>` lines have no C++ counterpart (the C++ resource stand-ins are silent) and are
//!    kept apart in [`Mover::store_notes`]; so are those of every later lifecycle call, which therefore neither fail
//!    the call nor stop [`Mover::advance`]'s timer loop.
//!
//! **The bound natives**, each refusing a call outside this binding:
//! - `SetTimer(Rate, bLoop, Name, Obj)` on the actor only (an `Obj` other than the actor is refused): a rate that is
//!   not finite, negative or above 3600 s, or a name the actor's class has no method for, is refused; rate 0 clears;
//!   otherwise the timer `Name` (lower-cased, default `Timer`) is due at clock + rate;
//! - `ClearTimer(Name, Obj)`: the same target rule, removes the timer;
//! - `AudioComponent.Stop` on the actor's ambient component only: stopping an idle component is valid; one with a
//!   `SoundCue` is refused ("mover sound playback is unsupported"): there is no audio backend here.
//!
//! **A lifecycle call** ([`Mover::notify`], [`Mover::advance`]) clears the log and saves the actor's and the
//! action's properties, the timers and the clock; on any failure (an error, or a log line the call left, which is
//! reported as the error) all of them are restored. [`Mover::advance`] runs every due timer in due order (a tie goes
//! to the first name in byte order), a looping timer re-armed at its due time plus its rate, at most 32 callbacks
//! per call; it stops after a callback that logged.
//!
//! **The Kismet entry points** ([`Mover::remote_event`] and the rest) run the installed sequence that owns the
//! action: its path is the action path up to the last `.` ("mover action path has no owning sequence" without one).
//! The executor is lane W5-KISMET's ([`crate::kismet`], decision 2026-11-15-31). It runs over a decode context of
//! its own (a package store on the VM store's cooked root, a reflection index, an object space and a decode state),
//! built at the first sequence call, as the C++ builds its `Kismet` lazily; its handlers only record requests, so they
//! need no VM. Only this mover's Matinee action is bound: its `Play` and `Reverse` inputs request motion (+1 / -1);
//! every other `Engine.SequenceAction` input (and another input of the action) is listed at the host boundary as
//! `<Class>:<op> <- <input>`, not run. One call ([`Dispatch`]) clears the per-call state, runs its entry, then every
//! impulse due; what the C++ throws in it is reported in [`Dispatch::errors`], followed by the decode log lines of the
//! call as `runtime: <line>`. The sequence's objects are the decode context's, not the VM's (the C++ shares one
//! runtime): the actor and the action the notifications run on are the VM's, and the executor reads only its own.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_defs::store::DefError;
use ow_object::decode::{DecodeState, ValueCx};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{FuncId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::CodeIndex;
use ow_object::value::Value;
use ow_package::store::PackageStore;
use ow_vm::api::{NativeCall, NativeFn, StubPolicy, Vm};
use ow_vm::error::VmError;

use crate::kismet::{Kismet, KismetError};

/// The record of what was observed of movers.
pub const RECORD: &str = "SANCTUARY_MOVER_PROTOTYPE.md";

/// The VM step budget of one call (C++ `runtime.stepLimit = 20000`).
pub const STEP_LIMIT: u64 = 20_000;

/// The largest timer rate the binding accepts, in seconds.
pub const MAX_TIMER_RATE: f64 = 3600.0;

/// The largest time step [`Mover::advance`] accepts, in seconds.
pub const MAX_ADVANCE: f64 = 60.0;

/// The most timer callbacks one [`Mover::advance`] runs.
pub const MAX_CALLBACKS: usize = 32;

/// Why binding a mover or one of its calls failed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MoverError {
    /// A check of this adapter failed; the text is the C++ `RuntimeError` text.
    Binding(String),
    /// The VM failed.
    Vm(VmError),
    /// The definition store failed to load an object of the binding.
    Def(DefError),
    /// The call left a line in the VM log (the C++ throws the first line); the text is that line.
    Logged(String),
    /// The Kismet executor failed (`crate::kismet::KismetError`, whose text is the C++ text); kept as that text,
    /// because the executor's error is neither `Clone` nor `PartialEq`.
    Kismet(String),
    /// Something this port does not have yet.
    NotYet(NotYet),
}

impl MoverError {
    /// The text the C++ `what()` gives for this failure: a script error's own message, a check's or a log line's
    /// text, anything else its text and causes.
    pub fn cpp_text(&self) -> String {
        match self {
            MoverError::Binding(text) | MoverError::Logged(text) | MoverError::Kismet(text) => {
                text.clone()
            }
            MoverError::Vm(VmError::Script { message, .. }) => message.clone(),
            MoverError::Vm(VmError::Def(e)) | MoverError::Def(e) => chain(e),
            MoverError::Vm(e) => chain(e),
            MoverError::NotYet(n) => n.to_string(),
        }
    }

    /// Whether this is a `NotYet` of the port (the CLI's exit code 3).
    pub fn is_not_yet(&self) -> bool {
        matches!(
            self,
            MoverError::NotYet(_) | MoverError::Vm(VmError::NotYet(_))
        )
    }
}

fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

impl fmt::Display for MoverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoverError::Binding(text) => write!(f, "mover: {text}"),
            MoverError::Kismet(text) => write!(f, "mover: kismet: {text}"),
            MoverError::Vm(e) => write!(f, "mover: {e}"),
            MoverError::Def(e) => write!(f, "mover: {e}"),
            MoverError::Logged(line) => write!(f, "mover: the call logged {line:?}"),
            MoverError::NotYet(n) => write!(f, "mover: {n}"),
        }
    }
}

impl std::error::Error for MoverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MoverError::Vm(e) => Some(e),
            MoverError::Def(e) => Some(e),
            MoverError::NotYet(n) => Some(n),
            MoverError::Binding(_) | MoverError::Logged(_) | MoverError::Kismet(_) => None,
        }
    }
}

impl From<KismetError> for MoverError {
    fn from(e: KismetError) -> Self {
        MoverError::Kismet(e.to_string())
    }
}

impl From<VmError> for MoverError {
    fn from(e: VmError) -> Self {
        match e {
            VmError::NotYet(n) => MoverError::NotYet(n),
            other => MoverError::Vm(other),
        }
    }
}

impl From<DefError> for MoverError {
    fn from(e: DefError) -> Self {
        match e {
            DefError::NotYet(n) => MoverError::NotYet(n),
            other => MoverError::Def(other),
        }
    }
}

impl From<ow_object::heap::HeapError> for MoverError {
    fn from(e: ow_object::heap::HeapError) -> Self {
        MoverError::Vm(VmError::Heap(e))
    }
}

fn binding(text: impl Into<String>) -> MoverError {
    MoverError::Binding(text.into())
}

/// What one lifecycle call did (C++ `Mover::Result`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct MoverResult {
    /// The VM steps the call used (for [`Mover::advance`], the sum over its callbacks; after a failure, the failing
    /// call's own).
    pub steps: u64,
    /// The actor's `bShouldSaveForCheckpoint` after the call (false after a failure, as the C++ leaves it).
    pub checkpoint: bool,
    /// Why the call failed (`None`: it succeeded). Everything it changed is restored.
    pub error: Option<MoverError>,
}

/// A world-acting op of the sequence the host may execute (C++ `Mover::Request`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub class: String,
    pub op: String,
    pub input: String,
}

/// What one sequence call did (C++ `Mover::Dispatch`).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dispatch {
    pub matched: usize,
    /// +1 play, -1 reverse, 0 none.
    pub motion: i32,
    pub trace: Vec<String>,
    pub host_boundary: Vec<String>,
    pub errors: Vec<String>,
    pub requests: Vec<Request>,
    pub entered: Vec<String>,
}

/// A variable linked to an op's variable link (C++ `Mover::Variable`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub object: String,
}

#[derive(Debug, Clone, PartialEq)]
struct Timer {
    due: f64,
    looping: bool,
    rate: f64,
    name: String,
}

/// The timer due at `clock` with the earliest due time; a tie goes to the first name in byte order (C++: the first
/// in `std::map` order with a strictly smaller due time wins).
fn next_due(timers: &BTreeMap<String, Timer>, clock: f64) -> Option<&Timer> {
    let mut pick: Option<&Timer> = None;
    for t in timers.values() {
        if t.due <= clock && pick.is_none_or(|p| t.due < p.due) {
            pick = Some(t);
        }
    }
    pick
}

/// The state the bound natives share with the mover.
#[derive(Debug, Clone, Default)]
struct Shared {
    /// Keyed by the lower-cased timer name (C++ `std::map`, byte order).
    timers: BTreeMap<String, Timer>,
    clock: f64,
}

/// One bound mover (module documentation).
pub struct Mover {
    vm: Vm,
    actor: ObjRef,
    action: ObjRef,
    started: FuncId,
    finished: FuncId,
    shared: Rc<RefCell<Shared>>,
    diagnostics: Vec<String>,
    store_notes: Vec<String>,
    package: String,
    action_path: String,
    sequence: Option<Box<Sequence>>,
    record: Rc<RefCell<Record>>,
}

/// What the bound handlers record during one sequence call.
#[derive(Debug, Default)]
struct Record {
    motion: i32,
    host_boundary: Vec<String>,
    requests: Vec<Request>,
}

/// The executor's decode context and the executor, once built.
struct Sequence {
    store: PackageStore,
    code: CodeIndex,
    space: ObjectSpace,
    state: DecodeState,
    kismet: Option<Kismet>,
}

impl fmt::Debug for Mover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mover")
            .field("actor", &self.actor)
            .field("action", &self.action)
            .field("action_path", &self.action_path)
            .finish_non_exhaustive()
    }
}

/// The prefix of the definition store's own log lines, which the C++ has no counterpart for.
const STORE_NOTE: &str = "defs: no holder for ";

fn is_store_note(line: &str) -> bool {
    line.starts_with(STORE_NOTE)
}

/// A VM log split into the lines the C++ would have logged and the store's notes.
fn split_notes(log: Vec<String>) -> (Vec<String>, Vec<String>) {
    log.into_iter().partition(|line| !is_store_note(line))
}

/// Decodes script function `f`'s bytecode (its export, found by its path, read and decoded as the interpreter does)
/// and reports the decoder's error, the C++ `script::decode` text.
fn decode_code(vm: &mut Vm, f: FuncId) -> Result<(), MoverError> {
    let path = vm
        .function(f)
        .map(|d| d.path.as_str().to_string())
        .unwrap_or_default();
    let Some((package_name, _)) = path.split_once('.') else {
        return Err(binding(format!("function not found: {path}")));
    };
    let (code, store) = vm.defs.code_mut();
    let package = code
        .package(store, package_name)
        .map_err(|e| binding(chain(&e)))?;
    let index = code.find_export(store, package, &path);
    if index <= 0 {
        return Err(binding(format!("function not found: {path}")));
    }
    let pkg = store.shared(package);
    let info =
        ow_script::function::read_function(&pkg, index).map_err(|e| binding(e.to_string()))?;
    ow_script::decode::decode(&pkg, &info).map_err(|e| binding(e.to_string()))?;
    Ok(())
}

/// A world-acting op the host may execute, listed (C++ `Impl::boundary`).
fn boundary(record: &mut Record, class: &str, op: &str, desc: String) {
    record.host_boundary.push(format!("{class}:{op} <- {desc}"));
    record.requests.push(Request {
        class: class.to_string(),
        op: op.to_string(),
        input: desc,
    });
}

/// The C++ size check of an export's property prefix (`Runtime::applyTaggedDefaults`).
fn check_prefix(
    vm: &Vm,
    package: ow_package::PackageId,
    index: i32,
    prefix: PropertyPrefix,
) -> Result<(), MoverError> {
    let pkg = vm.defs.packages().get(package);
    let entry = pkg.object(index).map_err(|e| binding(chain(&e)))?;
    let size = i64::from(entry.size);
    let bytes = i64::try_from(prefix.bytes()).unwrap_or(i64::MAX);
    if size < 0 || bytes > size || size - bytes < 8 {
        return Err(binding("object property prefix outside export"));
    }
    Ok(())
}

fn load_export(
    vm: &mut Vm,
    package: &str,
    path: &str,
    prefix: PropertyPrefix,
) -> Result<ObjRef, MoverError> {
    let (code, store) = vm.defs.code_mut();
    let id = code
        .package(store, package)
        .map_err(|e| binding(chain(&e)))?;
    let index = code.find_export(store, id, path);
    if index <= 0 {
        return Err(binding("unsupported exported object request"));
    }
    check_prefix(vm, id, index, prefix)?;
    let loaded = vm.defs.load_export(&mut vm.space, id, index, prefix);
    vm.log.extend(vm.defs.take_log());
    Ok(loaded?)
}

fn class_of(vm: &Vm, o: ObjRef) -> Result<ow_object::ids::ClassId, MoverError> {
    Ok(vm.space.heap.get(o)?.class)
}

fn declares(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<PropAddr>, MoverError> {
    let class = class_of(vm, o)?;
    Ok(vm.space.prop(class, name).map(PropAddr::scalar))
}

fn param_is(vm: &Vm, f: FuncId, index: usize, name: &str) -> bool {
    vm.function(f)
        .and_then(|d| d.params.get(index))
        .is_some_and(|p| {
            matches!(p.kind, PropKind::Object { .. }) && vm.space.names.text(p.name) == name
        })
}

/// A number argument as the C++ `Value::number()` reads it (0 for a value that is not a number).
fn number(v: &Value) -> f64 {
    match v {
        Value::Float(f) => f64::from(*f),
        Value::Int(i) => f64::from(*i),
        Value::Byte(b) => f64::from(*b),
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// The lower-cased timer name of argument `i` (C++ `timerName(c.has(i) ? c.in(i).s : "Timer")`).
fn timer_name(c: &NativeCall<'_>, i: usize) -> Result<String, VmError> {
    if !c.has(i) {
        return Ok("timer".to_string());
    }
    Ok(match c.arg(i)? {
        Value::Name(n) => c.vm.space.names.text(*n).to_ascii_lowercase(),
        Value::Str(s) => s.to_ascii_lowercase(),
        _ => String::new(),
    })
}

/// Whether argument `i` names an object other than `actor` (C++ `c.has(i) && c.in(i).o && c.in(i).o != actor`).
fn other_target(c: &NativeCall<'_>, i: usize, actor: ObjRef) -> Result<bool, VmError> {
    if !c.has(i) {
        return Ok(false);
    }
    Ok(matches!(c.arg(i)?, Value::Object(Some(o)) if *o != actor))
}

fn refuse(function: &str, message: &str) -> VmError {
    VmError::Script {
        function: function.to_string(),
        message: message.to_string(),
    }
}

impl Mover {
    /// Binds the mover `actor` and its Matinee action `action` (export paths in `package`) over the cooked folder
    /// `cooked` (module documentation, "The binding"). `mode` is the run's parity mode; `policy` the VM's stub policy
    /// (the C++ `Runtime` logs an unbound native and answers zero, which is `StubPolicy::LogAndZero`; a gameplay
    /// command refuses one, architecture section 3.8, "Stub policy per command").
    pub fn new(
        cooked: PathBuf,
        package: &str,
        actor: &str,
        action: &str,
        mode: ParityMode,
        policy: StubPolicy,
    ) -> Result<Mover, MoverError> {
        let defs = DefStore::open(cooked, IdentityPolicy::FirstInScanOrder, mode)?;
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            mode,
        )
        .with_stub_policy(policy);
        ow_vm::natives_core::register(&mut vm)?;
        vm.limits.step_limit = STEP_LIMIT;
        Self::bind(vm, package, actor, action)
    }

    /// [`Mover::new`] on a VM the caller built (its natives registered; the step limit is set here).
    pub fn bind(
        mut vm: Vm,
        package: &str,
        actor_path: &str,
        action_path: &str,
    ) -> Result<Mover, MoverError> {
        vm.limits.step_limit = STEP_LIMIT;
        let actor = load_export(&mut vm, package, actor_path, PropertyPrefix::PlacedActor)?;
        let action = load_export(&mut vm, package, action_path, PropertyPrefix::Ordinary)?;
        let interp_actor = vm.find_class("Engine.InterpActor")?;
        let interp_action = vm.find_class("Engine.SeqAct_Interp")?;
        if class_of(&vm, actor)? != interp_actor || class_of(&vm, action)? != interp_action {
            return Err(binding("mover binding has unsupported classes"));
        }
        let started = vm.find_function("Engine.InterpActor.InterpolationStarted")?;
        let finished = vm.find_function("Engine.InterpActor.InterpolationFinished")?;
        for (f, count) in [(started, 2), (finished, 1)] {
            let decl = vm.function(f);
            let shape = decl.is_some_and(|d| !d.is_native() && d.params.len() == count);
            if !shape || !param_is(&vm, f, 0, "InterpAction") {
                return Err(binding("unsupported mover notification signature"));
            }
            // The C++ decodes the notification's code here, so a body that does not decode fails the binding.
            decode_code(&mut vm, f)?;
        }
        if !param_is(&vm, started, 1, "GroupInst") {
            return Err(binding("unsupported mover group parameter"));
        }
        if declares(&vm, action, "bReversePlayback")?.is_none()
            || declares(&vm, actor, "bShouldSaveForCheckpoint")?.is_none()
        {
            return Err(binding("missing mover lifecycle properties"));
        }
        let audio = Self::ambient(&mut vm, actor)?;
        let shared = Rc::new(RefCell::new(Shared::default()));
        Self::bind_natives(&mut vm, actor, audio, &shared)?;
        let (diagnostics, store_notes) = split_notes(std::mem::take(&mut vm.log));
        if let Some(first) = diagnostics.first() {
            return Err(binding(format!(
                "mover object loading emitted a diagnostic: {first}"
            )));
        }
        Ok(Mover {
            vm,
            actor,
            action,
            started,
            finished,
            shared,
            diagnostics,
            store_notes,
            package: package.to_string(),
            action_path: action_path.to_string(),
            sequence: None,
            record: Rc::new(RefCell::new(Record::default())),
        })
    }

    /// The actor's ambient sound component, loaded (`None` when the actor has no such property or it is null).
    fn ambient(vm: &mut Vm, actor: ObjRef) -> Result<Option<ObjRef>, MoverError> {
        let Some(addr) = declares(vm, actor, "AmbientSoundComponent")? else {
            return Ok(None);
        };
        let Value::Object(Some(resource)) = vm.space.read(actor, addr)?.clone() else {
            return Ok(None);
        };
        // The C++ check before it instantiates: the reference resolves to an export (`resourceIndex > 0`). A
        // reference to an export of a package is one; an import is one when it resolves to an export (the C++
        // `resolveRef`), and an import that resolves to nothing, or an object from no package, is refused.
        let resolves = match vm.space.heap.get(resource)?.origin {
            Origin::Export { .. } => true,
            Origin::Resource { index, .. } if index > 0 => true,
            Origin::Resource { package, index } => {
                let (code, store) = vm.defs.code_mut();
                code.resolve_ref(store, package, index)
                    .is_some_and(|(_, i)| i > 0)
            }
            _ => false,
        };
        if !resolves {
            return Err(binding("unresolved mover ambient component"));
        }
        let loaded = vm.defs.load(&mut vm.space, resource);
        vm.log.extend(vm.defs.take_log());
        match loaded {
            Ok(audio) if matches!(vm.space.heap.get(audio)?.origin, Origin::Export { .. }) => {
                Ok(Some(audio))
            }
            Ok(_) | Err(DefError::NotADefinition(_)) => {
                Err(binding("unresolved mover ambient component"))
            }
            Err(e) => Err(e.into()),
        }
    }

    fn bind_natives(
        vm: &mut Vm,
        actor: ObjRef,
        audio: Option<ObjRef>,
        shared: &Rc<RefCell<Shared>>,
    ) -> Result<(), MoverError> {
        let bind = |vm: &mut Vm, path: &str, f: NativeFn| -> Result<(), MoverError> {
            let id = vm.find_function(path)?;
            if !vm.function(id).is_some_and(|d| d.is_native()) {
                return Err(binding(format!("expected native: {path}")));
            }
            vm.register_native_for(id, f)?;
            Ok(())
        };
        let timers = Rc::clone(shared);
        bind(
            vm,
            "Engine.Actor.ClearTimer",
            Rc::new(move |c: &mut NativeCall<'_>| {
                if c.this != Some(actor) || other_target(c, 1, actor)? {
                    return Err(refuse(
                        "Engine.Actor.ClearTimer",
                        "mover ClearTimer target is outside binding",
                    ));
                }
                let name = timer_name(c, 0)?;
                timers
                    .try_borrow_mut()
                    .map_err(|_| refuse("Engine.Actor.ClearTimer", "mover timers are busy"))?
                    .timers
                    .remove(&name);
                Ok(Value::None)
            }),
        )?;
        let timers = Rc::clone(shared);
        bind(
            vm,
            "Engine.Actor.SetTimer",
            Rc::new(move |c: &mut NativeCall<'_>| {
                const KEY: &str = "Engine.Actor.SetTimer";
                if c.this != Some(actor) || other_target(c, 3, actor)? {
                    return Err(refuse(KEY, "mover SetTimer target is outside binding"));
                }
                let rate = number(c.arg(0)?);
                let looping = c.has(1) && c.arg(1)?.truth(&c.vm.space.names);
                let name = timer_name(c, 2)?;
                let class = c.vm.space.heap.get(actor)?.class;
                // NaN is outside the range too.
                if !(0.0..=MAX_TIMER_RATE).contains(&rate)
                    || c.vm.find_method(class, &name, None)?.is_none()
                {
                    return Err(refuse(KEY, "unsupported mover timer"));
                }
                let mut state = timers
                    .try_borrow_mut()
                    .map_err(|_| refuse(KEY, "mover timers are busy"))?;
                if rate == 0.0 {
                    state.timers.remove(&name);
                } else {
                    let due = state.clock + rate;
                    state.timers.insert(
                        name.clone(),
                        Timer {
                            due,
                            looping,
                            rate,
                            name,
                        },
                    );
                }
                Ok(Value::None)
            }),
        )?;
        bind(
            vm,
            "Engine.AudioComponent.Stop",
            Rc::new(move |c: &mut NativeCall<'_>| {
                const KEY: &str = "Engine.AudioComponent.Stop";
                let Some(audio) = audio.filter(|a| c.this == Some(*a)) else {
                    return Err(refuse(KEY, "mover audio target outside binding"));
                };
                let class = c.vm.space.heap.get(audio)?.class;
                if let Some(p) = c.vm.space.prop(class, "SoundCue")
                    && matches!(
                        c.vm.space.read(audio, PropAddr::scalar(p))?,
                        Value::Object(Some(_))
                    )
                {
                    return Err(refuse(KEY, "mover sound playback is unsupported"));
                }
                Ok(Value::None)
            }),
        )?;
        Ok(())
    }

    /// The diagnostics loading left (always empty: a diagnostic fails [`Mover::new`]; kept for the C++ output).
    pub fn loading_diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// The definition store's `defs: no holder for <path>` lines from loading (no C++ counterpart).
    pub fn store_notes(&self) -> &[String] {
        &self.store_notes
    }

    /// The VM, for reading what the calls did.
    pub fn vm(&self) -> &Vm {
        &self.vm
    }

    /// The bound actor.
    pub fn actor(&self) -> ObjRef {
        self.actor
    }

    /// The timers now: (lower-cased name, due time on the mover's clock), in name order.
    pub fn timers(&self) -> Vec<(String, f64)> {
        self.shared
            .borrow()
            .timers
            .values()
            .map(|t| (t.name.clone(), t.due))
            .collect()
    }

    /// The mover's clock, in seconds.
    pub fn clock(&self) -> f64 {
        self.shared.borrow().clock
    }

    /// One lifecycle call: clears the log, runs `operation` (which returns the steps it used), and on any failure
    /// restores the actor, the action, the timers and the clock (C++ `Impl::result`).
    fn result(
        &mut self,
        operation: impl FnOnce(&mut Mover, &mut u64) -> Result<(), MoverError>,
    ) -> MoverResult {
        let actor_slots = self.vm.space.heap.get(self.actor).map(|o| o.slots.clone());
        let action_slots = self.vm.space.heap.get(self.action).map(|o| o.slots.clone());
        let saved = self.shared.borrow().clone();
        self.vm.log.clear();
        let mut steps = 0;
        let outcome = (|| {
            operation(self, &mut steps)?;
            // The store's own `defs: no holder` lines are notes, not the C++ log (module documentation).
            let (lines, notes) = split_notes(std::mem::take(&mut self.vm.log));
            self.store_notes.extend(notes);
            if let Some(first) = lines.first() {
                return Err(MoverError::Logged(first.clone()));
            }
            let addr = declares(&self.vm, self.actor, "bShouldSaveForCheckpoint")?
                .ok_or_else(|| binding("missing mover lifecycle properties"))?;
            Ok(self
                .vm
                .space
                .read(self.actor, addr)?
                .truth(&self.vm.space.names))
        })();
        match outcome {
            Ok(checkpoint) => MoverResult {
                steps,
                checkpoint,
                error: None,
            },
            Err(error) => {
                for (o, slots) in [(self.actor, actor_slots), (self.action, action_slots)] {
                    if let (Ok(slots), Ok(object)) = (slots, self.vm.space.heap.get_mut(o)) {
                        object.slots = slots;
                    }
                }
                *self.shared.borrow_mut() = saved;
                MoverResult {
                    steps,
                    checkpoint: false,
                    error: Some(error),
                }
            }
        }
    }

    /// `InterpolationStarted(action, None)` (`finished` false) or `InterpolationFinished(action)` (true), with the
    /// action's `bReversePlayback` set to `reverse` first (C++ `Mover::notify`).
    pub fn notify(&mut self, finished: bool, reverse: bool) -> MoverResult {
        self.result(|m, steps| {
            let addr = declares(&m.vm, m.action, "bReversePlayback")?
                .ok_or_else(|| binding("missing mover lifecycle properties"))?;
            m.vm.space.write(m.action, addr, Value::Bool(reverse))?;
            let mut args = vec![Value::Object(Some(m.action))];
            if !finished {
                args.push(Value::Object(None)); // the unused GroupInst of this bounded notification
            }
            let f = if finished { m.finished } else { m.started };
            let called = m.vm.call(f, Some(m.actor), args);
            *steps = m.vm.steps();
            called.map(|_| ()).map_err(MoverError::from)
        })
    }

    /// Advances the mover's clock by `seconds` (finite, 0 to 60) and runs the timers due (module documentation).
    pub fn advance(&mut self, seconds: f64) -> MoverResult {
        self.result(|m, steps| {
            if !seconds.is_finite() || !(0.0..=MAX_ADVANCE).contains(&seconds) {
                return Err(binding("invalid mover timer delta"));
            }
            m.shared.borrow_mut().clock += seconds;
            let mut calls = 0;
            let mut sum = 0u64;
            loop {
                let next = {
                    let mut state = m.shared.borrow_mut();
                    let Some(timer) = next_due(&state.timers, state.clock).cloned() else {
                        break;
                    };
                    calls += 1;
                    if calls > MAX_CALLBACKS {
                        return Err(binding("mover timer callback limit exceeded"));
                    }
                    state.timers.remove(&timer.name);
                    if timer.looping {
                        state.timers.insert(
                            timer.name.clone(),
                            Timer {
                                due: timer.due + timer.rate,
                                ..timer.clone()
                            },
                        );
                    }
                    timer
                };
                let class = m.vm.space.heap.get(m.actor)?.class;
                let state = m.vm.space.heap.get(m.actor)?.state;
                let Some(f) = m.vm.find_method(class, &next.name, state)? else {
                    let class_name =
                        m.vm.space
                            .classes
                            .get_layout(class)
                            .map(|l| l.path().name().to_string())
                            .unwrap_or_default();
                    return Err(binding(format!(
                        "no function {} on {class_name}",
                        next.name
                    )));
                };
                let called = m.vm.call(f, Some(m.actor), Vec::new());
                *steps = m.vm.steps();
                called?;
                sum = sum.checked_add(m.vm.steps()).ok_or(VmError::StepLimit)?;
                if m.vm.log.iter().any(|line| !is_store_note(line)) {
                    break;
                }
            }
            *steps = sum;
            Ok(())
        })
    }

    /// The owning sequence's path and the action's name (C++ `Impl::sequence`).
    fn split_action(&self) -> Result<(String, String), MoverError> {
        let dot = self
            .action_path
            .rfind('.')
            .ok_or_else(|| binding("mover action path has no owning sequence"))?;
        Ok((
            self.action_path[..dot].to_string(),
            self.action_path[dot + 1..].to_string(),
        ))
    }

    /// Builds the executor at the first call (a failure is retried at the next, as the C++ leaves its `Kismet`
    /// unset), with this mover's handlers.
    fn ensure_sequence(&mut self) -> Result<(), MoverError> {
        let (sequence_path, action_name) = self.split_action()?;
        if self.sequence.is_none() {
            let root = self.vm.defs.packages().root().to_path_buf();
            let store = PackageStore::open(root).map_err(|e| binding(chain(&e)))?;
            self.sequence = Some(Box::new(Sequence {
                store,
                code: CodeIndex::new(),
                space: ObjectSpace::new(),
                state: DecodeState::new(),
                kismet: None,
            }));
        }
        let mode = self.vm.parity;
        let Some(seq) = self.sequence.as_deref_mut() else {
            return Ok(());
        };
        if seq.kismet.is_some() {
            return Ok(());
        }
        let mut cx = ValueCx::new(
            &mut seq.store,
            &mut seq.code,
            &mut seq.space,
            &mut seq.state,
            mode,
        );
        let package = cx
            .code
            .package(cx.store, &self.package)
            .map_err(|e| binding(chain(&e)))?;
        let mut kismet = Kismet::new(&mut cx, package, &sequence_path)?;
        let record = Rc::clone(&self.record);
        // Only this mover's Matinee action is bound; everything else is reported at the host boundary.
        kismet.handle(
            "Engine.SeqAct_Interp",
            Rc::new(
                move |k: &mut Kismet, cx: &mut ValueCx<'_>, op: usize, input: i32| {
                    let desc = k.input_desc(cx, op, input)?;
                    let mut r = record.borrow_mut();
                    if k.ops()[op].name == action_name && (desc == "Play" || desc == "Reverse") {
                        r.motion = if desc == "Play" { 1 } else { -1 };
                    } else {
                        boundary(&mut r, &k.ops()[op].cls, &k.ops()[op].name, desc);
                    }
                    Ok(())
                },
            ),
        );
        let record = Rc::clone(&self.record);
        kismet.handle(
            "Engine.SequenceAction",
            Rc::new(
                move |k: &mut Kismet, cx: &mut ValueCx<'_>, op: usize, input: i32| {
                    let desc = k.input_desc(cx, op, input)?;
                    boundary(
                        &mut record.borrow_mut(),
                        &k.ops()[op].cls,
                        &k.ops()[op].name,
                        desc,
                    );
                    Ok(())
                },
            ),
        );
        seq.kismet = Some(kismet);
        Ok(())
    }

    /// One sequence call (C++ `Impl::dispatch`): the per-call state cleared, `entry`, then every impulse due now.
    fn dispatch(
        &mut self,
        entry: impl FnOnce(&mut Kismet, &mut ValueCx<'_>, &mut Dispatch) -> Result<(), MoverError>,
    ) -> Dispatch {
        let mut out = Dispatch::default();
        self.vm.log.clear();
        *self.record.borrow_mut() = Record::default();
        if let Some(seq) = self.sequence.as_deref_mut() {
            seq.state.log.clear();
        }
        let mode = self.vm.parity;
        let ran = self.ensure_sequence().and_then(|()| {
            let seq = self
                .sequence
                .as_deref_mut()
                .ok_or_else(|| binding("mover sequence missing"))?;
            let kismet = seq
                .kismet
                .as_mut()
                .ok_or_else(|| binding("mover sequence missing"))?;
            let mut cx = ValueCx::new(
                &mut seq.store,
                &mut seq.code,
                &mut seq.space,
                &mut seq.state,
                mode,
            );
            kismet.trace.clear();
            kismet.errors.clear();
            entry(kismet, &mut cx, &mut out)?;
            kismet.run(&mut cx)?;
            out.trace = kismet.trace.clone();
            out.errors = kismet.errors.clone();
            Ok(())
        });
        if let Err(e) = ran {
            out.errors.push(e.cpp_text());
        }
        if let Some(seq) = self.sequence.as_deref() {
            out.errors
                .extend(seq.state.log.iter().map(|line| format!("runtime: {line}")));
        }
        let record = std::mem::take(&mut *self.record.borrow_mut());
        out.motion = record.motion;
        out.host_boundary = record.host_boundary;
        out.requests = record.requests;
        out
    }

    /// A remote event through the action's installed Kismet sequence (C++ `Mover::remoteEvent`).
    pub fn remote_event(&mut self, name: &str) -> Dispatch {
        self.dispatch(|k, cx, out| {
            out.matched = k.remote_event(cx, name)?;
            Ok(())
        })
    }

    /// A mission behavior's remote event: the `WillowSeqEvent_MissionRemoteEvent` ops bound to that mission (C++
    /// `Mover::missionEvent`).
    pub fn mission_event(&mut self, mission: &str, name: &str) -> Dispatch {
        self.dispatch(|k, cx, out| {
            out.matched = k.mission_remote_event(cx, mission, name)?;
            Ok(())
        })
    }

    /// Reports the end of the host-driven motion to the sequence: fires the action's "Completed" or "Reversed"
    /// (C++ `Mover::motionFinished`).
    pub fn motion_finished(&mut self, reverse: bool) -> Dispatch {
        let action = self.split_action().map(|(_, a)| a).unwrap_or_default();
        self.dispatch(|k, cx, out| {
            if let Some(op) = k.find(&action) {
                k.fire_named(cx, op, if reverse { "Reversed" } else { "Completed" })?;
                out.matched = 1;
            }
            Ok(())
        })
    }

    /// Enters one event op by name (C++ `Mover::sequenceEvent`); a name that is no `Engine.SequenceEvent` op is an
    /// error line.
    pub fn sequence_event(&mut self, name: &str) -> Dispatch {
        self.dispatch(|k, cx, out| {
            let event = cx
                .code
                .find_class(cx.store, "Engine.SequenceEvent")
                .map_err(|e| binding(e.to_string()))?;
            match k.find(name) {
                Some(op) if cx.code.is_child_of(k.ops()[op].class, event) => {
                    k.activate_event(cx, op)?;
                    out.matched = 1;
                    out.entered.push(k.ops()[op].name.clone());
                }
                _ => k.errors.push(format!("no sequence event named {name}")),
            }
            Ok(())
        })
    }

    /// Enters every event whose `Originator` is the object at `object_path` (C++ `Mover::originatorEvent`).
    pub fn originator_event(&mut self, object_path: &str) -> Dispatch {
        self.dispatch(|k, cx, out| {
            for op in k.events_for_originator(cx, object_path)? {
                k.activate_event(cx, op)?;
                out.matched += 1;
                out.entered.push(k.ops()[op].name.clone());
            }
            Ok(())
        })
    }

    /// Fires output `desc` of op `name` (an op the host executed; C++ `Mover::output`). An op with no such output
    /// does nothing; a name that is no op is an error line.
    pub fn output(&mut self, name: &str, desc: &str) -> Dispatch {
        self.dispatch(|k, cx, out| {
            let Some(op) = k.find(name) else {
                k.errors.push(format!("no sequence op named {name}"));
                return Ok(());
            };
            if k.output_index(cx, op, desc)?.is_none() {
                return Ok(());
            }
            k.fire_named(cx, op, desc)?;
            out.matched = 1;
            Ok(())
        })
    }

    /// Advances sequence time by `seconds` (finite, 0 to 60) and runs what is due (C++ `Mover::advanceSequence`).
    pub fn advance_sequence(&mut self, seconds: f64) -> Dispatch {
        self.dispatch(|k, cx, _| {
            if !seconds.is_finite() || !(0.0..=MAX_ADVANCE).contains(&seconds) {
                return Err(binding("invalid sequence time delta"));
            }
            k.tick(cx, seconds)?;
            Ok(())
        })
    }

    /// The variables linked to op `name`'s variable link `desc`: each variable's name and, for an object variable,
    /// the path of the object its `ObjValue` names (C++ `Mover::variables`). The C++ does not catch here.
    pub fn variables(&mut self, name: &str, desc: &str) -> Result<Vec<Variable>, MoverError> {
        self.ensure_sequence()?;
        let mode = self.vm.parity;
        let Some(seq) = self.sequence.as_deref_mut() else {
            return Ok(Vec::new());
        };
        let Some(kismet) = seq.kismet.as_mut() else {
            return Ok(Vec::new());
        };
        let mut cx = ValueCx::new(
            &mut seq.store,
            &mut seq.code,
            &mut seq.space,
            &mut seq.state,
            mode,
        );
        let Some(op) = kismet.find(name) else {
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        for variable in kismet.variables(&mut cx, op, desc)? {
            let object = cx.space.heap.get(variable)?;
            let mut entry = Variable {
                name: cx.space.names.text(object.name).into_owned(),
                object: String::new(),
            };
            if let Some(Value::Object(Some(o))) =
                crate::kismet::property(cx.space, variable, "ObjValue")?
                && let Origin::Export { package, index } | Origin::Resource { package, index } =
                    cx.space.heap.get(o)?.origin
            {
                entry.object = cx
                    .store
                    .get(package)
                    .path_of(index)
                    .map_err(|e| binding(chain(&e)))?;
            }
            result.push(entry);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
