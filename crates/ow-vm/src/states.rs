//! Owner: lane W5-STATES. AI-assisted (Claude Code). Object states: the State export body, the per-object state
//! frame, `GotoState`, `PushState` / `PopState`, `IsInState`, `Enable` / `Disable`, label jumps and the state-code
//! step, as `docs/verification/NATIVE_ENGINE_CORE.md` ("Object state machine", "Probe events", "State-code step")
//! describes them. Latent actions (`Sleep` and its poll) are [`crate::latent`]'s.
//!
//! **Two modes** (architecture 6.1, R3). Under `ParityMode::Reference` the C++ approximation stays where
//! `natives_core.rs` has it (a state name and the two events, a label logged and not run, the 8-level nesting
//! bound); state code is not run ([`process_state`] does nothing) and `PushState`, `PopState` and `Sleep` stay
//! unbound, as in the C++. Otherwise (`Off`) this module is the state machine, and `natives_core.rs` hands
//! `GotoState`, `IsInState`, `Enable` and `Disable` to it.
//!
//! **The state frame.** Each object gets one on first use (when the engine makes one for an object is not read;
//! UNVERIFIED): the active state (a State export, or none for the class itself), the state code position, the
//! latent action, the probe mask and the push stack. The object's `state` field in the heap holds the active
//! state's name, kept in step, so the interpreter's dispatch by state sees it.
//!
//! **States and their parents.** A state name resolves to the State export of the most derived class of the
//! object's chain that declares it; its parent is the State export its super reference names. Labels are looked up
//! in the state, then its parents; events (`BeginState`, `EndState`, `PushedState`, ...) in the state's functions,
//! then its parents', then the class chain (the record's function lookup). The interpreter's own dispatch
//! (`Vm::find_method`, W3-INTERP) looks a state up by name in each class of the chain and does not follow a parent
//! state of another name; that gap is W3-INTERP's (decision 2026-11-15-27, open items).
//!
//! **Probes.** The record names ten of the 32 probe events (ids 300 to 331); those ten are probes here. The 22
//! other names of the public UE3 table for those ids are `NotYet { NotRead }` where a probe answer is needed (this
//! executable's names for them are not read); any other name is not a probe. The mask of a state frame is the set
//! of probes the class chain or the active state chain defines a function for, as the record describes it (the
//! serialized probe mask is read but not used).
//!
//! **State code** runs through the interpreter (`interp::declare_state_code`): one frame with no locals per run,
//! started by [`process_state`] as a top-level call, so each run has the VM's step budget. A label in the running
//! state's code is a jump inside the run; a label in a parent state's code, a state change and a latent call end
//! the run, and [`process_state`] continues at the new position (at most five restarts on a state change per step,
//! the record's bound, and at most [`MAX_RUNS_PER_STEP`] runs in all, a Rust-only bound).
//!
//! Status: every rule here is the record's reading of the executable (Ghidra, lane G3) or, where marked, this
//! port's; none is confirmed in the running game (UNVERIFIED).

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ow_core::{FixedHashMap, NotYet, ParityMode};
use ow_object::ids::{ClassId, FuncId, NameId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::PackageId;
use ow_package::package::Package;
use ow_package::reader::{ReadError, Reader};
use ow_package::tables;
use ow_script::function::FunctionInfo;
use ow_script::ops::op;

use crate::api::{NativeCall, Vm};
use crate::error::VmError;
use crate::latent::LatentAction;

/// The record this module follows.
const RECORD: &str = "NATIVE_ENGINE_CORE.md (Object state machine)";

// ---------------------------------------------------------------------------------------------------------------
// The State export body
// ---------------------------------------------------------------------------------------------------------------

/// The end-of-script token a non-empty state script ends in (`ow_script::ops::op::END_OF_SCRIPT`).
const END_OF_SCRIPT: u8 = op::END_OF_SCRIPT;

/// The largest count the function map may hold (the class body's `count()` bound, `ow_object::class_body::MAX_COUNT`).
const MAX_COUNT: i32 = ow_object::class_body::MAX_COUNT;

/// A label table offset of all ones: the state has no label table (the public UE3 `MAXWORD` sentinel; checked on the
/// installed packages, see [`read_state_body`]).
pub const NO_LABEL_TABLE: u16 = 0xFFFF;

/// The part of a State export body the state machine uses.
///
/// The layout: the net index and an empty tagged-property block (only the `None` terminator) of a plain object,
/// then the struct part and the state part of the Class export body (`ow_object::class_body`, the C++
/// `decodeClassBody`, checked on every Class export of the nine code packages), up to the end of the state part,
/// where a State export ends. That a State export carries exactly these parts is a fit, not a recovered layout: it
/// was checked structurally on the installed packages (decision 2026-11-15-27: every State export of the
/// nine code packages ends exactly at its serial size, its script decodes, its label table offset is one past a
/// `LabelTable` token and every label lands on a statement). What the fields mean in the running game is UNVERIFIED.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBody {
    /// The parent state (the export table's super reference, which the body repeats), 0 for none.
    pub super_ref: i32,
    /// `ScriptBytecodeSize`: the in-memory size of the state code.
    pub memory_size: u32,
    /// Offset of the state code inside the package data.
    pub script_offset: usize,
    /// The stored size of the state code (0: the state has no code).
    pub script_size: usize,
    /// The serialized probe mask (32 bits). Not used: the probe mask here is computed from the functions a class
    /// and a state define (NATIVE_ENGINE_CORE.md, "Probe events"), which is what the record describes.
    pub probe_mask: u32,
    /// One past the in-memory offset of the state code's `LabelTable` token (where its entries start),
    /// [`NO_LABEL_TABLE`] for none.
    pub label_table: u16,
    /// The state flags. Which bit marks an `auto` state, and which a state that runs for non-authority actors, is
    /// not read (NotYet where needed).
    pub flags: u16,
}

/// Why a State export body did not read.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateBodyError {
    /// The export index is outside the export table.
    IndexOutOfRange(i32),
    /// The export table gives a negative offset or size.
    NegativeOffsetOrSize,
    /// The tagged properties before the struct part are not empty (they are not walked).
    TaggedProperties,
    /// The body's super reference differs from the export table's.
    SuperDiffers,
    /// A negative script size.
    NegativeScriptSize,
    /// A non-empty script that does not end in `EndOfScript`.
    ScriptNotTerminated,
    /// The function map's count is outside `0..=MAX_COUNT`.
    CountOutOfRange,
    /// Bytes left after the state part.
    TrailingBytes,
    /// A read past the export or a bad name or reference.
    Read(ReadError),
}

impl fmt::Display for StateBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateBodyError::IndexOutOfRange(i) => write!(f, "state export index {i} out of range"),
            StateBodyError::NegativeOffsetOrSize => f.write_str("state export has a negative offset or size"),
            StateBodyError::TaggedProperties => f.write_str("state body has tagged properties"),
            StateBodyError::SuperDiffers => f.write_str("state body super reference differs from the export table"),
            StateBodyError::NegativeScriptSize => f.write_str("state body has a negative script size"),
            StateBodyError::ScriptNotTerminated => f.write_str("state body script does not end in EndOfScript"),
            StateBodyError::CountOutOfRange => f.write_str("state body count out of range"),
            StateBodyError::TrailingBytes => f.write_str("state body has bytes after the state part"),
            StateBodyError::Read(_) => f.write_str("state body does not read"),
        }
    }
}

impl std::error::Error for StateBodyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StateBodyError::Read(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ReadError> for StateBodyError {
    fn from(e: ReadError) -> Self {
        StateBodyError::Read(e)
    }
}

/// Reads the body of State export `index` of `package`: the struct part and the state part of the class body layout
/// (see [`StateBody`]), with every check the class body reader makes for those parts, and nothing after them.
pub fn read_state_body(package: &Package, index: i32) -> Result<StateBody, StateBodyError> {
    read_body(package, index, true)
}

/// Reads the struct part and the state part of Class export `index`: the layout `ow_object::class_body` reads (the
/// C++ `decodeClassBody`, checked on every Class export of the nine code packages), stopping after the state flags,
/// before the function map and the class part. `flags` are the class's own state flags.
pub fn read_class_state_part(package: &Package, index: i32) -> Result<StateBody, StateBodyError> {
    read_body(package, index, false)
}

/// `state_export`: a State export (a plain object's net index and an empty tagged-property block first, the
/// function map and the end checked); otherwise a Class export (the class body's first word first, nothing read
/// after the state flags).
fn read_body(package: &Package, index: i32, state_export: bool) -> Result<StateBody, StateBodyError> {
    let exported = usize::try_from(index)
        .ok()
        .filter(|&i| i >= 1)
        .and_then(|i| package.exports.get(i - 1))
        .ok_or(StateBodyError::IndexOutOfRange(index))?;
    let (Ok(start), Ok(size)) = (usize::try_from(exported.offset), usize::try_from(exported.size)) else {
        return Err(StateBodyError::NegativeOffsetOrSize);
    };
    let limit = start.checked_add(size).ok_or(ReadError::Truncated)?;
    let mut r = Reader::new(&package.data).with_limit(limit);
    r.set_pos(start);
    let imports = i32::try_from(package.imports.len()).map_err(|_| ReadError::InvalidReference)?;
    let exports = i32::try_from(package.exports.len()).map_err(|_| ReadError::InvalidReference)?;
    let reference = |r: &mut Reader<'_>| r.reference(imports, exports);

    // Base object part: the net index (the class body's first word), then the tagged properties, which must be
    // empty (only the `None` terminator). A State export with tagged properties is refused, not walked.
    r.i32()?; // net index (the class body's "unknown reference"), not range checked
    if state_export && tables::read_name(&mut r, &package.names)? != "None" {
        return Err(StateBodyError::TaggedProperties);
    }
    // Struct part (as `ow_object::class_body`).
    reference(&mut r)?; // next field of the field chain
    let super_ref = reference(&mut r)?;
    if super_ref != exported.super_ {
        return Err(StateBodyError::SuperDiffers);
    }
    reference(&mut r)?; // script-text slot
    reference(&mut r)?; // first child
    reference(&mut r)?; // C++-text slot
    r.i32()?; // source line
    r.i32()?; // text position
    let memory_size = r.i32()?;
    let script_size = r.i32()?;
    let (Ok(memory_size), Ok(script_size)) = (u32::try_from(memory_size), usize::try_from(script_size)) else {
        return Err(StateBodyError::NegativeScriptSize);
    };
    r.require(script_size)?;
    let script_offset = r.pos();
    if script_size != 0 && r.byte(script_offset + script_size - 1)? != END_OF_SCRIPT {
        return Err(StateBodyError::ScriptNotTerminated);
    }
    r.skip(script_size)?;

    // State part.
    let probe_mask = r.u32()?;
    let label_table = u16_le(&mut r)?;
    let flags = u16_le(&mut r)?;
    if !state_export {
        return Ok(StateBody {
            super_ref,
            memory_size,
            script_offset,
            script_size,
            probe_mask,
            label_table,
            flags,
        });
    }
    let count = r.i32()?;
    if !(0..=MAX_COUNT).contains(&count) {
        return Err(StateBodyError::CountOutOfRange);
    }
    for _ in 0..count {
        // function map: (name, function export)
        tables::read_name(&mut r, &package.names)?;
        reference(&mut r)?;
    }
    if r.pos() != r.limit() {
        return Err(StateBodyError::TrailingBytes);
    }
    Ok(StateBody {
        super_ref,
        memory_size,
        script_offset,
        script_size,
        probe_mask,
        label_table,
        flags,
    })
}

fn u16_le(r: &mut Reader<'_>) -> Result<u16, ReadError> {
    let b = r.take(2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

// ---------------------------------------------------------------------------------------------------------------
// Probes
// ---------------------------------------------------------------------------------------------------------------

/// The probe events the record names, with their name ids (NATIVE_ENGINE_CORE.md, "Conventions used below"). A
/// probe's bit in the mask is its id minus 300 (names 300 to 331 are the probes).
const PROBES: [(&str, u32); 10] = [
    ("Destroyed", 300),
    ("GainedChild", 301),
    ("LostChild", 302),
    ("Touch", 306),
    ("UnTouch", 307),
    ("BeginState", 309),
    ("EndState", 310),
    ("Tick", 317),
    ("PreBeginPlay", 328),
    ("PostBeginPlay", 329),
];

/// The public UE3 names of the 22 probe ids the record does not name (303-305, 308, 311-316, 318-327, 330, 331).
/// Whether this executable gives those ids these names is not read, so a probe answer for one of them is `NotYet`.
const UNREAD_PROBES: [&str; 22] = [
    "HitWall",
    "Falling",
    "Landed",
    "Bump",
    "BaseChange",
    "Attach",
    "Detach",
    "EncroachingOn",
    "EncroachedBy",
    "MayFall",
    "SeePlayer",
    "EnemyNotVisible",
    "HearNoise",
    "UpdateEyeHeight",
    "SeeMonster",
    "SpecialHandling",
    "BotDesireability",
    "NotifyBump",
    "NotifyLanded",
    "NotifyHitWall",
    "UnusedProbe",
    "All",
];

const BEGIN_STATE_BIT: u32 = 1 << (309 - 300);
const END_STATE_BIT: u32 = 1 << (310 - 300);

/// What a name is to the probe mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Probe {
    Bit(u32),
    Unread,
    Other,
}

fn probe(name: &str) -> Probe {
    if let Some(&(_, id)) = PROBES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        return Probe::Bit(1 << (id - 300));
    }
    if UNREAD_PROBES.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Probe::Unread;
    }
    Probe::Other
}

fn unread_probe(name: &str) -> VmError {
    NotYet::not_read(
        format!("whether {name} is a probe event of this executable (a probe id the record does not name)"),
        RECORD,
    )
    .into()
}

// ---------------------------------------------------------------------------------------------------------------
// The state machine's data (a VM service)
// ---------------------------------------------------------------------------------------------------------------

/// How many restarts on a state change one state-code step makes (the record: "at most five").
pub const MAX_STATE_RESTARTS: u32 = 5;

/// How many runs of state code one step makes in all (a Rust-only bound): a label in a parent state's code also
/// ends a run, so two states jumping to each other's labels with no latent call between would never stop.
pub const MAX_RUNS_PER_STEP: u32 = 64;

/// How deep the parent chain of a state may go (a Rust-only bound, which also stops a cycle).
pub const MAX_STATE_DEPTH: u32 = 64;

/// One state: the State export behind it (the package's filed id and the export index).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StateKey {
    package: PackageId,
    export: i32,
}

/// What was read about one state (cached per State export).
#[derive(Debug)]
struct StateCode {
    key: StateKey,
    /// The state's name, as its package spells it.
    name: NameId,
    /// `Package.Class.State`, for messages.
    path: String,
    /// The class that declares it.
    owner: ClassId,
    parent: Option<StateKey>,
    /// Label -> statement index of the state's code, in table order.
    labels: Vec<(NameId, usize)>,
    /// Lower-cased names of the functions the state itself declares.
    functions: Vec<String>,
    /// The state's code as a function the interpreter runs; `None` when the state has no code.
    code: Option<FuncId>,
}

/// A state code position: the state whose code runs, and the statement to run next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CodePos {
    state: StateKey,
    pc: usize,
}

/// One entry of the push stack.
#[derive(Debug, Clone)]
struct Pushed {
    node: Option<StateKey>,
    code: Option<CodePos>,
}

/// One object's state frame (the record's "state storage").
#[derive(Debug, Clone)]
struct StateFrame {
    /// The active state; `None` is the class itself (no state).
    node: Option<StateKey>,
    code: Option<CodePos>,
    latent: Option<LatentAction>,
    /// Bit `id - 300` per probe event (only the ten the record names).
    probes: u32,
    stack: Vec<Pushed>,
    /// An `EndState` is being sent (the record's re-entrancy guard).
    in_end_state: bool,
    /// State changes so far (switches, pushes, pops): how "script changed state again" is seen.
    changes: u64,
    /// The code position was set (a label, a switch, a push or a pop) since the running state code last looked.
    moved: bool,
}

/// Why a run of state code ended before its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Moved,
    Latent,
}

#[derive(Debug)]
struct Running {
    object: ObjRef,
    state: StateKey,
    stop: Option<Stop>,
    /// A `PushState` ran on the object during this run (the position it saved may sit inside a loop).
    pushed: bool,
}

/// The state frames, the states read so far and the state code running (a VM service, added on first use).
#[derive(Debug, Default)]
pub(crate) struct StateMachine {
    frames: FixedHashMap<ObjRef, StateFrame>,
    states: FixedHashMap<StateKey, Rc<StateCode>>,
    running: Vec<Running>,
}

fn machine(vm: &mut Vm) -> Result<Rc<RefCell<StateMachine>>, VmError> {
    match vm.service::<StateMachine>() {
        Some(m) => Ok(m),
        None => vm.add_service(StateMachine::default()),
    }
}

/// Runs `f` on the state machine, borrowed only for `f` (never across a script call).
fn with<R>(vm: &mut Vm, f: impl FnOnce(&mut StateMachine) -> R) -> Result<R, VmError> {
    let m = machine(vm)?;
    let mut guard = m
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_vm::states::StateMachine"))?;
    Ok(f(&mut guard))
}

fn internal(message: impl Into<String>) -> VmError {
    VmError::Script {
        function: "ow_vm::states".to_string(),
        message: message.into(),
    }
}

fn decode_error(path: &str, e: &dyn std::error::Error) -> VmError {
    let mut text = format!("state {path}: {e}");
    let mut cause = e.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    VmError::Decode(text)
}

fn decode_text(path: &str, why: &str) -> VmError {
    VmError::Decode(format!("state {path}: {why}"))
}

fn begin(vm: &mut Vm) -> NameId {
    vm.space.names.intern("Begin")
}

fn is_auto(vm: &Vm, name: NameId) -> bool {
    vm.space.names.text(name).eq_ignore_ascii_case("Auto")
}

fn class_text(vm: &Vm, class: ClassId) -> String {
    vm.space
        .classes
        .get_layout(class)
        .map_or("?", |l| l.path().name())
        .to_string()
}

/// The class and its super classes, most derived first (stops at a repeat).
fn chain(vm: &Vm, class: ClassId) -> Vec<ClassId> {
    let mut out = Vec::new();
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        if out.contains(&c) {
            break;
        }
        out.push(c);
        cursor = vm.space.classes.get_layout(c).and_then(|l| l.super_class());
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Reading states
// ---------------------------------------------------------------------------------------------------------------

/// The State export the name `state` resolves to for an object of `class`: that of the most derived class of the
/// chain that declares it; `None` when no class of the chain does.
fn node_for(vm: &mut Vm, class: ClassId, state: NameId) -> Result<Option<StateKey>, VmError> {
    if state == NameId::NONE || !vm.class_declares_state(class, state)? {
        return Ok(None);
    }
    let lower = vm.space.names.text(state).to_ascii_lowercase();
    for c in chain(vm, class) {
        let found = vm.defs.code().key_of(c).and_then(|k| {
            let info = vm.defs.code().class(k);
            info.states().get(&lower).map(|s| StateKey {
                package: info.package,
                export: s.export,
            })
        });
        if let Some(key) = found {
            load_state(vm, key, c, 0)?;
            return Ok(Some(key));
        }
    }
    Ok(None)
}

fn loaded(vm: &mut Vm, key: StateKey) -> Result<Rc<StateCode>, VmError> {
    with(vm, |m| m.states.get(&key).cloned())?
        .ok_or_else(|| internal("a state used before it was read"))
}

/// Reads State export `key` (declared by `owner`) and its parents, once: the body, the label table, the state's
/// own functions, and its code declared to the interpreter.
fn load_state(
    vm: &mut Vm,
    key: StateKey,
    owner: ClassId,
    depth: u32,
) -> Result<Rc<StateCode>, VmError> {
    if let Some(done) = with(vm, |m| m.states.get(&key).cloned())? {
        return Ok(done);
    }
    let pkg = vm.defs.packages().shared(key.package);
    let Some(export) = usize::try_from(key.export)
        .ok()
        .filter(|&i| i >= 1)
        .and_then(|i| pkg.exports.get(i - 1))
    else {
        return Err(internal(format!(
            "state export {} is not in {}",
            key.export, pkg.name
        )));
    };
    let export_name = export.name.clone();
    let path = match pkg.path_of(key.export) {
        Ok(p) => format!("{}.{p}", pkg.name),
        Err(e) => return Err(decode_error(&export_name, &e)),
    };
    if depth >= MAX_STATE_DEPTH {
        return Err(decode_text(
            &path,
            &format!("its parent states nest deeper than {MAX_STATE_DEPTH} (or form a cycle)"),
        ));
    }
    let body = read_state_body(&pkg, key.export).map_err(|e| decode_error(&path, &e))?;
    let functions: Vec<String> = vm
        .defs
        .code()
        .key_of(owner)
        .and_then(|k| {
            let info = vm.defs.code().class(k);
            (info.package == key.package)
                .then(|| info.states().values().find(|s| s.export == key.export))
                .flatten()
                .map(|s| s.functions.keys().cloned().collect())
        })
        .unwrap_or_default();
    let parent = if body.super_ref == 0 {
        None
    } else {
        let (p, i, outer) = {
            let (code, store) = vm.defs.code_mut();
            let target = code
                .resolve_ref(store, key.package, body.super_ref)
                .filter(|&(p, i)| code.class_name(store, p, i) == "State");
            let Some((p, i)) = target else {
                return Err(decode_text(
                    &path,
                    "its parent does not resolve to a State export",
                ));
            };
            let outer = usize::try_from(i)
                .ok()
                .and_then(|i| store.get(p).exports.get(i.wrapping_sub(1)))
                .map_or(0, |e| e.outer);
            (p, i, outer)
        };
        let Some(parent_owner) = crate::interp::class_at(vm, p, outer, "state owner")? else {
            return Err(decode_text(&path, "its parent state's class does not load"));
        };
        let parent = StateKey { package: p, export: i };
        load_state(vm, parent, parent_owner, depth + 1)?;
        Some(parent)
    };
    let mut labels = Vec::new();
    let mut code = None;
    if body.script_size != 0 {
        let info = FunctionInfo {
            index: key.export,
            name: export_name.clone(),
            memory_size: body.memory_size,
            script_offset: body.script_offset,
            script_size: body.script_size,
            ..FunctionInfo::default()
        };
        let decoded =
            ow_script::decode::decode(&pkg, &info).map_err(|e| decode_error(&path, &e))?;
        if body.label_table != NO_LABEL_TABLE {
            let table = u32::from(body.label_table)
                .checked_sub(1)
                .and_then(|at| decoded.statement_at.get(&at))
                .and_then(|&i| decoded.statements.get(i))
                .filter(|s| s.op == op::LABEL_TABLE)
                .ok_or_else(|| {
                    decode_text(
                        &path,
                        "its label table offset is not one past a LabelTable token",
                    )
                })?;
            for (label, offset) in &table.labels {
                let at = u32::try_from(*offset)
                    .ok()
                    .and_then(|o| decoded.statement_at.get(&o))
                    .copied()
                    .ok_or_else(|| {
                        decode_text(&path, &format!("its label {label} does not land on a statement"))
                    })?;
                labels.push((vm.space.names.intern(label), at));
            }
        }
        let function_path = format!("{path}.<state code>");
        code = Some(crate::interp::declare_state_code(
            vm,
            key.package,
            owner,
            &function_path,
            info,
        )?);
    }
    let state = Rc::new(StateCode {
        key,
        name: vm.space.names.intern(&export_name),
        path,
        owner,
        parent,
        labels,
        functions,
        code,
    });
    with(vm, |m| m.states.insert(key, state.clone()))?;
    Ok(state)
}

/// The state and its parents, the state first (all read when the state was).
fn state_chain(vm: &mut Vm, node: Option<StateKey>) -> Result<Vec<Rc<StateCode>>, VmError> {
    let mut out: Vec<Rc<StateCode>> = Vec::new();
    let mut cursor = node;
    while let Some(key) = cursor {
        if out.iter().any(|s| s.key == key) || out.len() >= MAX_STATE_DEPTH as usize {
            break;
        }
        let s = loaded(vm, key)?;
        cursor = s.parent;
        out.push(s);
    }
    Ok(out)
}

/// The probes the class chain defines a function for.
fn class_probes(vm: &mut Vm, class: ClassId) -> Result<u32, VmError> {
    let mut mask = 0;
    for (name, id) in PROBES {
        if vm.find_method(class, name, None)?.is_some() {
            mask |= 1 << (id - 300);
        }
    }
    Ok(mask)
}

/// The probe mask of an object of `class` in `node`: the probes the class chain or the state chain defines.
fn mask_for(vm: &mut Vm, class: ClassId, node: Option<StateKey>) -> Result<u32, VmError> {
    let mut mask = class_probes(vm, class)?;
    for s in state_chain(vm, node)? {
        for (name, id) in PROBES {
            if s.functions.iter().any(|f| f.eq_ignore_ascii_case(name)) {
                mask |= 1 << (id - 300);
            }
        }
    }
    Ok(mask)
}

// ---------------------------------------------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------------------------------------------

fn ensure_frame(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    if with(vm, |m| m.frames.contains_key(&o))? {
        return Ok(());
    }
    let (class, named) = {
        let object = vm.space.heap.get(o)?;
        (object.class, object.state)
    };
    // A state name set before the frame existed (not by this module) is resolved; an undeclared one is dropped.
    let node = match named {
        Some(n) => node_for(vm, class, n)?,
        None => None,
    };
    let probes = mask_for(vm, class, node)?;
    let name = match node {
        Some(k) => Some(loaded(vm, k)?.name),
        None => None,
    };
    vm.space.heap.get_mut(o)?.state = name;
    with(vm, |m| {
        m.frames.entry(o).or_insert(StateFrame {
            node,
            code: None,
            latent: None,
            probes,
            stack: Vec::new(),
            in_end_state: false,
            changes: 0,
            moved: false,
        });
    })
}

/// Runs `f` on the object's frame, made first if it has none.
fn frame<R>(vm: &mut Vm, o: ObjRef, f: impl FnOnce(&mut StateFrame) -> R) -> Result<R, VmError> {
    ensure_frame(vm, o)?;
    with(vm, |m| m.frames.get_mut(&o).map(f))?.ok_or_else(|| internal("a state frame vanished"))
}

/// Makes `node` the active state with code position `code`: the latent action cleared, the probe mask rebuilt, the
/// heap's state name set, a state change counted.
fn set_node(
    vm: &mut Vm,
    o: ObjRef,
    node: Option<StateKey>,
    code: Option<CodePos>,
) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let probes = mask_for(vm, class, node)?;
    let name = match node {
        Some(k) => Some(loaded(vm, k)?.name),
        None => None,
    };
    frame(vm, o, |fr| {
        fr.node = node;
        fr.code = code;
        fr.latent = None;
        fr.probes = probes;
        fr.changes += 1;
        fr.moved = true;
    })?;
    vm.space.heap.get_mut(o)?.state = name;
    Ok(())
}

fn changes(vm: &mut Vm, o: ObjRef) -> Result<u64, VmError> {
    frame(vm, o, |fr| fr.changes)
}

/// The function `name` as an event sent to `o` sees it: the active state's own functions, then its parents', then
/// the class chain (the record's function lookup). This is the Off-mode lookup over this module's state frame;
/// [`send_world_event`] also gives the Reference one.
pub fn find_event(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<FuncId>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let node = frame(vm, o, |fr| fr.node)?;
    let lower = name.to_ascii_lowercase();
    for s in state_chain(vm, node)? {
        if s.functions.contains(&lower)
            && let Some(f) = vm.find_method(s.owner, name, Some(s.name))?
        {
            return Ok(Some(f));
        }
    }
    vm.find_method(class, name, None)
}

/// Sends event `name` to `o` with `args` for the world (the destroy sequence, the actor tick); whether it was sent.
/// Outside `ParityMode::Reference` it goes through [`find_event`] and, for a probe event, the frame's mask (a probe
/// id the record does not name is `NotYet`). Under Reference it is the C++ lookup by the heap's state name, with no
/// mask. A missing event is silent (`false`).
pub fn send_world_event(
    vm: &mut Vm,
    o: ObjRef,
    name: &str,
    args: Vec<Value>,
) -> Result<bool, VmError> {
    if vm.parity == ParityMode::Reference {
        let (class, state) = {
            let object = vm.space.heap.get(o)?;
            (object.class, object.state)
        };
        let Some(f) = vm.find_method(class, name, state)? else {
            return Ok(false);
        };
        vm.call(f, Some(o), args)?;
        return Ok(true);
    }
    let gate = match probe(name) {
        Probe::Bit(bit) => Some(bit),
        Probe::Unread => return Err(unread_probe(name)),
        Probe::Other => None,
    };
    send_event(vm, o, name, args, gate)
}

/// Sends event `name` to `o` with `args`, if the object defines it and, for a probe event (`gate`), its frame's mask
/// has the probe's bit; whether it was sent. A missing event is silent.
fn send_event(
    vm: &mut Vm,
    o: ObjRef,
    name: &str,
    args: Vec<Value>,
    gate: Option<u32>,
) -> Result<bool, VmError> {
    if let Some(bit) = gate
        && frame(vm, o, |fr| fr.probes & bit)? == 0
    {
        return Ok(false);
    }
    let Some(f) = find_event(vm, o, name)? else {
        return Ok(false);
    };
    vm.call(f, Some(o), args)?;
    Ok(true)
}

/// The position of label `label` in the code of `node` or of its parents.
fn find_label(
    vm: &mut Vm,
    node: Option<StateKey>,
    label: NameId,
) -> Result<Option<CodePos>, VmError> {
    for s in state_chain(vm, node)? {
        if let Some(&(_, pc)) = s.labels.iter().find(|(n, _)| *n == label) {
            return Ok(Some(CodePos { state: s.key, pc }));
        }
    }
    Ok(None)
}

/// Applies a `GotoState` or `PushState` label: state code resumes there on the next step. A label not found leaves
/// no code to run (silent for the default `Begin`, logged with the record's text for an explicit one; that the code
/// position is cleared is this port's reading, UNVERIFIED). The latent action is cleared (UNVERIFIED).
fn apply_label(
    vm: &mut Vm,
    o: ObjRef,
    label: NameId,
    explicit: bool,
    what: &str,
) -> Result<(), VmError> {
    let node = frame(vm, o, |fr| fr.node)?;
    let found = find_label(vm, node, label)?;
    frame(vm, o, |fr| {
        fr.code = found;
        fr.latent = None;
        fr.moved = true;
    })?;
    if found.is_none() && explicit {
        let state = match node {
            Some(k) => {
                let name = loaded(vm, k)?.name;
                vm.space.names.text(name).into_owned()
            }
            None => "None".to_string(),
        };
        let label = vm.space.names.text(label).into_owned();
        vm.log
            .push(format!("{what} ({state} {label}): Label not found"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// GotoState, PushState, PopState
// ---------------------------------------------------------------------------------------------------------------

/// What the state change routine reports (the record's 0, 1 and 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// 0: the new state is none.
    NoState,
    /// 1: changed.
    Done,
    /// 2: script changed state again during `EndState` or `BeginState`; that change stands.
    Superseded,
}

/// `Object.GotoState(NewState, Label, bForceEvents, bKeepStack)` outside `ParityMode::Reference`
/// (NATIVE_ENGINE_CORE.md, "Object.GotoState"); `natives_core.rs` calls it.
pub(crate) fn goto_state_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(this) = c.this else {
        return Ok(Value::None);
    };
    let requested = if c.has(0) {
        Some(name_arg(c, 0)?)
    } else {
        None
    };
    let label = if c.has(1) {
        name_arg(c, 1)?
    } else {
        NameId::NONE
    };
    let force = c.has(2) && c.arg(2)?.truth(&c.vm.space.names);
    let keep = c.has(3) && c.arg(3)?.truth(&c.vm.space.names);
    goto_state(c.vm, this, requested, label, force, keep)?;
    Ok(Value::None)
}

/// `GotoState` on `o`: `requested` `None` is an omitted `NewState` (stay in the current state); `label`
/// `NameId::NONE` is the default `Begin`.
fn goto_state(
    vm: &mut Vm,
    o: ObjRef,
    requested: Option<NameId>,
    label: NameId,
    force: bool,
    keep_stack: bool,
) -> Result<(), VmError> {
    ensure_frame(vm, o)?;
    let current = vm.space.heap.get(o)?.state.unwrap_or(NameId::NONE);
    let target = requested.unwrap_or(current);
    let explicit = label != NameId::NONE;
    let label = if explicit { label } else { begin(vm) };
    // The same state, not forced: no events, only the label.
    if target == current && !force {
        return apply_label(vm, o, label, explicit, "GotoState");
    }
    match change_state(vm, o, target, force, keep_stack)? {
        Change::Done => apply_label(vm, o, label, explicit, "GotoState"),
        Change::Superseded => Ok(()),
        Change::NoState => {
            // None and Auto are silent; another name was not found. The wording is this port's: the record says
            // only "a not-found style warning".
            if target != NameId::NONE && !is_auto(vm, target) {
                let class = vm.space.heap.get(o)?.class;
                let text = vm.space.names.text(target).into_owned();
                let line = format!(
                    "GotoState ({text}): state not found in {}",
                    class_text(vm, class)
                );
                vm.log.push(line);
            }
            Ok(())
        }
    }
}

/// What `Auto` resolves to for an object of `class` (the record: the class itself if it is flagged auto, else the
/// first state flagged auto, else no state). Which state flag bit means auto is not read, but auto is a bit of the
/// state flags (this port's reading, UNVERIFIED), so when the class's own state flags and those of every state its
/// chain declares are all 0 the answer is no state, whatever the bit is (129 of the 166 installed states have flags
/// 0, decision 2026-11-15-27). Otherwise, or when the class body does not read, `NotYet { NotRead }`.
fn auto_state(vm: &mut Vm, class: ClassId) -> Result<Option<StateKey>, VmError> {
    let not_read = |why: &str| -> VmError {
        NotYet::not_read(
            format!("GotoState('Auto'): which state flag bit marks an auto state ({why})"),
            RECORD,
        )
        .into()
    };
    // Loads the chain's code, so its states are listed.
    let auto = vm.space.names.intern("Auto");
    vm.class_declares_state(class, auto)?;
    let Some((package, export)) = vm.defs.code().key_of(class).map(|k| {
        let info = vm.defs.code().class(k);
        (info.package, info.export)
    }) else {
        return Err(not_read("the class has no package export"));
    };
    let pkg = vm.defs.packages().shared(package);
    match read_class_state_part(&pkg, export) {
        Ok(body) if body.flags == 0 => {}
        Ok(_) => return Err(not_read("the class's own state flags are not 0")),
        Err(_) => return Err(not_read("the class body's state flags do not read")),
    }
    for c in chain(vm, class) {
        let states: Vec<(PackageId, i32)> = vm
            .defs
            .code()
            .key_of(c)
            .map(|k| {
                let info = vm.defs.code().class(k);
                info.states()
                    .values()
                    .map(|s| (info.package, s.export))
                    .collect()
            })
            .unwrap_or_default();
        for (package, export) in states {
            let pkg = vm.defs.packages().shared(package);
            match read_state_body(&pkg, export) {
                Ok(body) if body.flags == 0 => {}
                Ok(_) => {
                    return Err(not_read(
                        "a state of the class chain has state flags that are not 0",
                    ));
                }
                Err(e) => return Err(decode_error(&format!("{}:{export}", pkg.name), &e)),
            }
        }
    }
    Ok(None)
}

/// The state change routine (the record's steps 1 to 7; every object has a frame here, so step 1 never fails).
fn change_state(
    vm: &mut Vm,
    o: ObjRef,
    target: NameId,
    force: bool,
    keep_stack: bool,
) -> Result<Change, VmError> {
    // 2. Resolve the target. An unknown name falls back to the class (no state) and the name to None.
    let class = vm.space.heap.get(o)?.class;
    let new_node = if is_auto(vm, target) {
        auto_state(vm, class)?
    } else {
        node_for(vm, class, target)?
    };
    let new_name = match new_node {
        Some(k) => loaded(vm, k)?.name,
        None => NameId::NONE,
    };
    // 3. Unless bKeepStack, every pushed state is popped (PoppedState each, when there is an active state).
    if !keep_stack {
        while !frame(vm, o, |fr| fr.stack.is_empty())? {
            pop_one(vm, o, true)?;
        }
    }
    // 4. EndState(new name) on the old state, probe-gated, not from inside another EndState.
    let old_name = vm.space.heap.get(o)?.state.unwrap_or(NameId::NONE);
    let before = changes(vm, o)?;
    let in_end = frame(vm, o, |fr| fr.in_end_state)?;
    if (force || (old_name != NameId::NONE && new_name != old_name)) && !in_end {
        frame(vm, o, |fr| fr.in_end_state = true)?;
        let sent = send_event(
            vm,
            o,
            "EndState",
            vec![Value::Name(new_name)],
            Some(END_STATE_BIT),
        );
        frame(vm, o, |fr| fr.in_end_state = false)?;
        sent?;
        if changes(vm, o)? != before {
            return Ok(Change::Superseded);
        }
    }
    // 5. The switch.
    set_node(vm, o, new_node, None)?;
    let switched = changes(vm, o)?;
    // 6. BeginState(old name), unless not forced and the new state is none.
    if !force && new_name == NameId::NONE {
        return Ok(Change::NoState);
    }
    if new_name != old_name || force {
        send_event(
            vm,
            o,
            "BeginState",
            vec![Value::Name(old_name)],
            Some(BEGIN_STATE_BIT),
        )?;
        if changes(vm, o)? != switched {
            return Ok(Change::Superseded);
        }
    }
    // 7.
    Ok(if new_name == NameId::NONE {
        Change::NoState
    } else {
        Change::Done
    })
}

/// Pops the top pushed state: `PoppedState()` to the leaving state, the entry restored (state and code position),
/// then `ContinuedState()` to the resumed one. Inside `GotoState`'s clear (`in_goto`) `PoppedState` is sent only
/// when there is an active state and `ContinuedState` is not sent; that the clear restores each entry as the pop
/// routine does is this port's reading of the record's "popping every pushed state" (UNVERIFIED).
fn pop_one(vm: &mut Vm, o: ObjRef, in_goto: bool) -> Result<(), VmError> {
    let node = frame(vm, o, |fr| fr.node)?;
    if !in_goto || node.is_some() {
        send_event(vm, o, "PoppedState", Vec::new(), None)?;
    }
    // The event may itself have popped.
    let Some(top) = frame(vm, o, |fr| fr.stack.pop())? else {
        return Ok(());
    };
    set_node(vm, o, top.node, top.code)?;
    if !in_goto {
        send_event(vm, o, "ContinuedState", Vec::new(), None)?;
    }
    Ok(())
}

/// `Object.PushState(NewState, NewLabel)` (outside Reference only; Reference keeps it unbound, as the C++).
fn push_state_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(this) = c.this else {
        return Ok(Value::None);
    };
    let target = name_arg(c, 0)?;
    let label = if c.has(1) {
        name_arg(c, 1)?
    } else {
        NameId::NONE
    };
    push_state(c.vm, this, target, label)?;
    Ok(Value::None)
}

fn push_state(vm: &mut Vm, o: ObjRef, target: NameId, label: NameId) -> Result<(), VmError> {
    ensure_frame(vm, o)?;
    let class = vm.space.heap.get(o)?.class;
    let Some(node) = node_for(vm, class, target)? else {
        // Own wording: the record says only that a resolvable state is needed.
        let text = vm.space.names.text(target).into_owned();
        let line = format!("PushState ({text}): state not found in {}", class_text(vm, class));
        vm.log.push(line);
        return Ok(());
    };
    let (current, on_stack) = frame(vm, o, |fr| {
        (fr.node, fr.stack.iter().any(|p| p.node == Some(node)))
    })?;
    // The record: "if the new state is not the current one: ... then jump to the label". That a push of the current
    // state does nothing at all, the label jump included, is this port's reading of that sentence (UNVERIFIED).
    if on_stack || current == Some(node) {
        return Ok(());
    }
    send_event(vm, o, "PausedState", Vec::new(), None)?;
    with(vm, |m| {
        if let Some(r) = m.running.last_mut()
            && r.object == o
        {
            r.pushed = true;
        }
    })?;
    // When state code runs the PushState, its position is already the statement after it.
    frame(vm, o, |fr| {
        let entry = Pushed {
            node: fr.node,
            code: fr.code,
        };
        fr.stack.push(entry);
    })?;
    set_node(vm, o, Some(node), None)?;
    send_event(vm, o, "PushedState", Vec::new(), None)?;
    let explicit = label != NameId::NONE;
    let label = if explicit { label } else { begin(vm) };
    apply_label(vm, o, label, explicit, "PushState")
}

/// `Object.PopState(bPopAll)` (outside Reference only).
fn pop_state_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(this) = c.this else {
        return Ok(Value::None);
    };
    let all = c.has(0) && c.arg(0)?.truth(&c.vm.space.names);
    loop {
        if frame(c.vm, this, |fr| fr.stack.is_empty())? {
            break;
        }
        pop_one(c.vm, this, false)?;
        if !all {
            break;
        }
    }
    Ok(Value::None)
}

// ---------------------------------------------------------------------------------------------------------------
// IsInState, Enable, Disable, probes
// ---------------------------------------------------------------------------------------------------------------

/// `Object.IsInState(TestState, bTestStateStack)` outside Reference: the active state or one of its parents, or
/// with `bTestStateStack` a pushed state. The frame is made first, as the other state natives do.
pub(crate) fn is_in_state_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(this) = c.this else {
        return Ok(Value::Bool(false));
    };
    let test = name_arg(c, 0)?;
    let stack = c.has(1) && c.arg(1)?.truth(&c.vm.space.names);
    let vm = &mut *c.vm;
    // As GotoState, PushState, Enable and Disable do: a state name set before the frame existed is resolved first,
    // so IsInState and GetStateName agree whatever the order of the calls.
    ensure_frame(vm, this)?;
    let Some((node, pushed)) = with(vm, |m| {
        m.frames.get(&this).map(|fr| {
            let pushed: Vec<StateKey> = fr.stack.iter().filter_map(|p| p.node).collect();
            (fr.node, pushed)
        })
    })?
    else {
        return Ok(Value::Bool(false));
    };
    if state_chain(vm, node)?.iter().any(|s| s.name == test) {
        return Ok(Value::Bool(true));
    }
    if stack {
        for key in pushed {
            if loaded(vm, key)?.name == test {
                return Ok(Value::Bool(true));
            }
        }
    }
    Ok(Value::Bool(false))
}

/// `Object.Enable(ProbeFunc)` (`on`) and `Object.Disable(ProbeFunc)` outside Reference: set or clear the probe's
/// bit; `Enable` only when the class or the active state defines the function. Another name is logged with the
/// record's text; a probe id the record does not name is `NotYet`.
pub(crate) fn enable_native(c: &mut NativeCall<'_>, on: bool) -> Result<Value, VmError> {
    let name = name_arg(c, 0)?;
    let text = c.vm.space.names.text(name).into_owned();
    let Some(this) = c.this else {
        return Ok(Value::None);
    };
    match probe(&text) {
        Probe::Unread => Err(unread_probe(&text)),
        Probe::Other => {
            let what = if on { "Enable" } else { "Disable" };
            c.vm
                .log
                .push(format!("{what}: '{text}' is not a probe function"));
            Ok(Value::None)
        }
        Probe::Bit(bit) => {
            let vm = &mut *c.vm;
            if on {
                let class = vm.space.heap.get(this)?.class;
                let node = frame(vm, this, |fr| fr.node)?;
                let implemented = mask_for(vm, class, node)?;
                frame(vm, this, |fr| fr.probes |= bit & implemented)?;
            } else {
                frame(vm, this, |fr| fr.probes &= !bit)?;
            }
            Ok(Value::None)
        }
    }
}

/// Whether the engine sends event `name` to `object` now (the record's probe rule, for the world's events such as
/// `Tick` and `Touch`): a probe only when the object has no state frame or its mask has the bit; any other event
/// always. Under `ParityMode::Reference` always (the C++ has no probe mask). A probe id the record does not name is
/// `NotYet`.
pub fn probe_enabled(vm: &mut Vm, object: ObjRef, name: &str) -> Result<bool, VmError> {
    if vm.parity == ParityMode::Reference {
        return Ok(true);
    }
    match probe(name) {
        Probe::Other => Ok(true),
        Probe::Unread => Err(unread_probe(name)),
        Probe::Bit(bit) => {
            let bits = with(vm, |m| m.frames.get(&object).map(|fr| fr.probes & bit))?;
            Ok(bits.is_none_or(|b| b != 0))
        }
    }
}

/// Drops `object`'s state frame. Every path that frees objects should call it (the world's destroy, and the sweep's
/// `release_unreferenced`): a frame kept for a freed handle is never read again, but it holds memory.
pub fn forget(vm: &mut Vm, object: ObjRef) -> Result<(), VmError> {
    with(vm, |m| {
        m.frames.remove(&object);
    })
}

/// A name argument: a name, the text of a string (interned), or none for an omitted one.
fn name_arg(c: &mut NativeCall<'_>, i: usize) -> Result<NameId, VmError> {
    let value = c.arg(i)?.clone();
    match value {
        Value::Name(n) => Ok(n),
        Value::Str(s) => Ok(c.vm.space.names.intern(&s)),
        Value::None => Ok(NameId::NONE),
        _ => Err(VmError::BadArgument {
            function: c.vm.function_path(c.function),
            index: i,
            reason: "expected a name",
        }),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// State code
// ---------------------------------------------------------------------------------------------------------------

/// One state-code step of `object` with the tick's `delta` (the record's "State-code step"; the world calls it as
/// step 2 of the actor tick). Nothing without a frame or code, or when the object is destroyed. A pending latent
/// action is polled first and, still pending, ends the step. Then state code runs until it ends, a latent call
/// yields, or the state changes more than [`MAX_STATE_RESTARTS`] times. An actor that is not the authority (`Role`
/// is not 3) is `NotYet`: which state flag lets a state run there is not read. Under `ParityMode::Reference`
/// nothing runs (the C++ runs no state code).
///
/// Each run is a call through `Vm::call`: from the top level it gets a fresh step budget, and it shows in a call
/// trace and the census as `<state path>.<state code>`.
pub fn process_state(vm: &mut Vm, object: ObjRef, delta: f32) -> Result<(), VmError> {
    if vm.parity == ParityMode::Reference || vm.space.heap.get(object)?.destroyed {
        return Ok(());
    }
    let Some(code) = with(vm, |m| m.frames.get(&object).map(|fr| fr.code))? else {
        return Ok(());
    };
    if code.is_none() {
        return Ok(());
    }
    authority(vm, object)?;
    if crate::latent::poll(vm, object, delta)? {
        return Ok(());
    }
    let mut restarts = 0;
    for _ in 0..MAX_RUNS_PER_STEP {
        if vm.space.heap.get(object)?.destroyed {
            return Ok(());
        }
        let next = with(vm, |m| {
            m.frames.get_mut(&object).and_then(|fr| {
                fr.moved = false;
                fr.code.map(|c| (c, fr.changes))
            })
        })?;
        let Some((pos, before)) = next else {
            return Ok(());
        };
        let Some(function) = loaded(vm, pos.state)?.code else {
            frame(vm, object, |fr| fr.code = None)?;
            return Ok(());
        };
        with(vm, |m| {
            m.running.push(Running {
                object,
                state: pos.state,
                stop: None,
                pushed: false,
            })
        })?;
        let ran = vm.call(function, Some(object), Vec::new());
        let stop = with(vm, |m| m.running.pop().and_then(|r| r.stop))?;
        ran?;
        match stop {
            Some(Stop::Latent) => return Ok(()),
            Some(Stop::Moved) => {
                if changes(vm, object)? != before {
                    if restarts == MAX_STATE_RESTARTS {
                        return Ok(());
                    }
                    restarts += 1;
                }
            }
            None => {
                frame(vm, object, |fr| fr.code = None)?;
                return Ok(());
            }
        }
    }
    let at = with(vm, |m| m.frames.get(&object).and_then(|fr| fr.code))?;
    let state = match at {
        Some(pos) => loaded(vm, pos.state)?.path.clone(),
        None => "?".to_string(),
    };
    Err(internal(format!(
        "the state code of {state} ran {MAX_RUNS_PER_STEP} times in one step without a latent call (a Rust-only \
         bound)"
    )))
}

/// The record's authority condition: an actor (an object with a `Role`) must be the authority (3) for its state
/// code to run; the state flag that lets a state run elsewhere is not read.
fn authority(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let Some(role) = vm.space.prop(class, "Role") else {
        return Ok(());
    };
    match vm.space.read(o, PropAddr::scalar(role))? {
        Value::Byte(3) => Ok(()),
        _ => Err(NotYet::not_read(
            "state code of an actor that is not the authority: which state flag lets a state run there",
            RECORD,
        )
        .into()),
    }
}

/// The interpreter's seam: the statement `o`'s state code starts at (its frame's code position).
pub(crate) fn resume_at(vm: &mut Vm, o: ObjRef) -> Result<usize, VmError> {
    with(vm, |m| {
        let running = m.running.last().filter(|r| r.object == o)?;
        let pos = m.frames.get(&o)?.code?;
        (pos.state == running.state).then_some(pos.pc)
    })?
    .ok_or_else(|| internal("state code started without a code position"))
}

/// The interpreter's seam: statement `pc` of `o`'s state code is about to run, so its code position becomes the next
/// statement (where a latent call resumes, and what a `PushState` saves).
pub(crate) fn at_statement(vm: &mut Vm, o: ObjRef, pc: usize) -> Result<(), VmError> {
    with(vm, |m| {
        let Some(state) = m.running.last().filter(|r| r.object == o).map(|r| r.state) else {
            return;
        };
        if let Some(fr) = m.frames.get_mut(&o) {
            fr.code = Some(CodePos { state, pc: pc + 1 });
        }
    })
}

/// The interpreter's seam, after each statement of `o`'s state code: whether the run ends here (the code position
/// was set, the object was destroyed, or a latent action is pending). A latent call or a `PushState` inside a
/// `foreach` (`no_loop` false) is `NotYet`: how the loop survives the wait or the push is not read.
pub(crate) fn yields(vm: &mut Vm, o: ObjRef, no_loop: bool) -> Result<bool, VmError> {
    let pushed = with(vm, |m| {
        m.running.last().is_some_and(|r| r.object == o && r.pushed)
    })?;
    if pushed && !no_loop {
        return Err(NotYet::not_read(
            "a PushState inside a foreach of state code: how the loop survives until the PopState",
            RECORD,
        )
        .into());
    }
    let destroyed = vm.space.heap.get(o).map_or(true, |x| x.destroyed);
    let (moved, latent) = with(vm, |m| {
        m.frames
            .get(&o)
            .map_or((true, false), |fr| (fr.moved, fr.latent.is_some()))
    })?;
    let stop = if destroyed || moved {
        Stop::Moved
    } else if latent {
        if !no_loop {
            return Err(NotYet::not_read(
                "a latent call inside a foreach of state code: how the loop survives the wait",
                RECORD,
            )
            .into());
        }
        Stop::Latent
    } else {
        return Ok(false);
    };
    with(vm, |m| {
        if let Some(r) = m.running.last_mut() {
            r.stop = Some(stop);
        }
    })?;
    Ok(true)
}

/// The interpreter's seam for a `GotoLabel` in `o`'s state code: the label is looked up in the active state and its
/// parents. In the running state's code it is a jump (`Some(statement)`); in another state's code the run ends and
/// the step continues there (`None`). A label not found is logged (own wording, after the record's `GotoState`
/// text) and leaves no code to run (UNVERIFIED).
pub(crate) fn goto_label(vm: &mut Vm, o: ObjRef, label: &Value) -> Result<Option<usize>, VmError> {
    let label = match label {
        Value::Name(n) => *n,
        Value::Str(s) => {
            let s = s.clone();
            vm.space.names.intern(&s)
        }
        _ => return Err(internal("a GotoLabel whose label is not a name")),
    };
    let running = with(vm, |m| {
        m.running.last().filter(|r| r.object == o).map(|r| r.state)
    })?
    .ok_or_else(|| internal("a GotoLabel outside running state code"))?;
    let node = frame(vm, o, |fr| fr.node)?;
    let found = find_label(vm, node, label)?;
    if let Some(pos) = found
        && pos.state == running
    {
        frame(vm, o, |fr| fr.code = Some(pos))?;
        return Ok(Some(pos.pc));
    }
    if found.is_none() {
        let text = vm.space.names.text(label).into_owned();
        vm.log.push(format!("GotoLabel ({text}): Label not found"));
    }
    frame(vm, o, |fr| {
        fr.code = found;
        fr.moved = true;
    })?;
    with(vm, |m| {
        if let Some(r) = m.running.last_mut() {
            r.stop = Some(Stop::Moved);
        }
    })?;
    Ok(None)
}

/// Whether `o`'s state code is the code running now (for latent natives).
pub(crate) fn running_code_of(vm: &mut Vm, o: ObjRef) -> Result<bool, VmError> {
    with(vm, |m| m.running.last().is_some_and(|r| r.object == o))
}

/// `o`'s pending latent action (none without a state frame).
pub fn latent_of(vm: &mut Vm, o: ObjRef) -> Result<Option<LatentAction>, VmError> {
    with(vm, |m| m.frames.get(&o).and_then(|fr| fr.latent))
}

/// Whether `o` waits on a latent action (its state code does not continue until the poll clears it).
pub fn latent_pending(vm: &mut Vm, o: ObjRef) -> Result<bool, VmError> {
    Ok(latent_of(vm, o)?.is_some())
}

/// Sets `o`'s latent action.
pub(crate) fn set_latent(
    vm: &mut Vm,
    o: ObjRef,
    action: Option<LatentAction>,
) -> Result<(), VmError> {
    frame(vm, o, |fr| fr.latent = action)
}

/// Registers the state natives the C++ does not bind (`PushState`, `PopState`), outside `ParityMode::Reference`
/// only (under Reference they stay unbound, as in the C++). `natives_core::register` calls it.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity == ParityMode::Reference {
        return Ok(());
    }
    vm.register_native("Object.PushState(name,name)", Rc::new(push_state_native))?;
    vm.register_native("Object.PopState(bool)", Rc::new(pop_state_native))
}

// ---------------------------------------------------------------------------------------------------------------
// Tests: the record's rules on an invented "Core" package (AGENTS.md rule 9) whose State exports carry real bodies
// (code, label tables, a parent), built with `ow_package::testkit`.
// ---------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use ow_attr::AttributeSystem;
    use ow_core::Missing;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use ow_package::testkit::script::{Asm, CPF_OPT, FunctionSpec, Param, ScriptToy};
    use ow_package::testkit::reflect::{ClassBodySpec, class_body};
    use ow_package::testkit::{Export, ints};

    use super::*;
    use crate::api::StubPolicy;
    use crate::natives_core;

    const FUNC_FINAL: u32 = 0x1;
    const FUNC_DEFINED: u32 = 0x2;
    const FUNC_NATIVE: u32 = 0x400;
    const FUNC_PUBLIC: u32 = 0x20000;
    const VIRTUAL: u32 = FUNC_DEFINED | FUNC_PUBLIC;
    const NATIVE: u32 = FUNC_NATIVE | FUNC_FINAL | FUNC_PUBLIC;
    const OPERATOR: u32 = FUNC_NATIVE | 0x1000 | 0x23000;

    fn param(kind: &str, name: &str, flags: u64) -> Param {
        Param {
            flags,
            ..Param::new(kind, name)
        }
    }

    /// The properties the events write.
    #[derive(Clone, Copy)]
    struct Ids {
        trace: i32,
        arg: i32,
    }

    /// `Trace = Trace $ text`.
    fn append(a: &mut Asm, trace: i32, text: &str) {
        let mut constant = vec![0x1F];
        constant.extend_from_slice(text.as_bytes());
        constant.push(0);
        a.stmt().raw(&[0x0F]).instance(trace).native(112, |a| {
            a.instance(trace).raw(&constant);
        });
    }

    /// A call of `function` whose single argument is the name `name` (index from `PackageBuilder::name`).
    fn call_with_name(a: &mut Asm, function: i32, name: i32) {
        a.stmt().final_function(function, |a| {
            a.raw(&[0x21]).i32(name).i32(0);
        });
    }

    /// The label table (at the end, after the code) and `EndOfScript`; returns the body and the label table
    /// offset (one past the `LabelTable` token).
    fn finish(t: &mut ScriptToy, mut a: Asm, labels: &[(&str, u32)]) -> (Asm, u16) {
        let at = a.here();
        a.stmt().raw(&[0x0C]);
        for &(name, offset) in labels {
            let f = t.p.fname(name, 0);
            a.raw(&f).i32(i32::try_from(offset).unwrap());
        }
        let none = t.none;
        a.raw(&none).i32(0);
        a.end();
        (a, u16::try_from(at + 1).unwrap())
    }

    /// A State export of `owner` with parent `parent` (0 for none) and code `code` (a body and its label table
    /// offset), in the layout [`read_state_body`] reads.
    fn state(t: &mut ScriptToy, owner: i32, name: &str, parent: i32, code: Option<&(Asm, u16)>) -> i32 {
        state_full(t, owner, name, parent, code, 0)
    }

    /// [`state`] with state flags `flags`.
    fn state_full(
        t: &mut ScriptToy,
        owner: i32,
        name: &str,
        parent: i32,
        code: Option<&(Asm, u16)>,
        flags: u16,
    ) -> i32 {
        let none = t.none;
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&none);
        payload.extend_from_slice(&ints(&[0, parent, 0, 0, 0, 0, 0]));
        let (script, memory, label) = match code {
            Some((a, label)) => (a.bytes.clone(), a.memory_size(), *label),
            None => (Vec::new(), 0, NO_LABEL_TABLE),
        };
        payload.extend_from_slice(&ints(&[
            i32::try_from(memory).unwrap(),
            i32::try_from(script.len()).unwrap(),
        ]));
        payload.extend_from_slice(&script);
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&label.to_le_bytes());
        payload.extend_from_slice(&flags.to_le_bytes());
        payload.extend_from_slice(&ints(&[0]));
        let mut export = Export::new(t.import("State"), name, payload);
        export.outer = owner;
        export.super_ = parent;
        t.p.add_export_with(export)
    }

    /// A State export of `owner` with no code and state flags `flags`.
    fn state_flags(t: &mut ScriptToy, owner: i32, name: &str, flags: u16) -> i32 {
        state_full(t, owner, name, 0, None, flags)
    }

    /// An event of `owner` that appends `text` to `Trace` (and, with `keep`, stores its name argument in `Arg`).
    fn event(t: &mut ScriptToy, ids: Ids, owner: i32, name: &str, text: &str, arg: Option<&str>, keep: bool) {
        let params: Vec<Param> = arg.iter().map(|a| param("Name", a, 0)).collect();
        t.function(owner, name, &params, None, &[], &FunctionSpec::new(VIRTUAL, name), |f| {
            let mut a = Asm::new();
            append(&mut a, ids.trace, text);
            if keep && let Some(p) = arg {
                a.stmt().raw(&[0x0F]).instance(ids.arg).local(f.get(p));
            }
            a.stmt().return_nothing();
            a.end();
            Some(a)
        });
    }

    fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<&str>, number: u16) -> i32 {
        let mut s = FunctionSpec::new(NATIVE, name);
        s.native = number;
        t.function(owner, name, params, result.map(|r| (r, 0)), &[], &s, |_| None).function
    }

    /// The invented package: `Object` (natives, empty `BeginState` / `EndState`), `Actor` (`LatentFloat`,
    /// `Sleep`), `Mach` (states A to Q), `Remote` (an actor with a `Role`).
    fn package() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "Function",
            "State",
            "StrProperty",
            "NameProperty",
            "FloatProperty",
            "BoolProperty",
            "ByteProperty",
            "IntProperty",
            "ArrayProperty",
        ]);
        let object = t.class("Object", 0);
        let mut concat = FunctionSpec::new(OPERATOR, "$");
        concat.native = 112;
        t.function(
            object,
            "Concat_StrStr",
            &[param("Str", "A", 0), param("Str", "B", 0)],
            Some(("Str", 0)),
            &[],
            &concat,
            |_| None,
        );
        let goto = native(
            &mut t,
            object,
            "GotoState",
            &[
                param("Name", "NewState", CPF_OPT),
                param("Name", "Label", CPF_OPT),
                param("Bool", "bForceEvents", CPF_OPT),
                param("Bool", "bKeepStack", CPF_OPT),
            ],
            None,
            113,
        );
        let push = native(
            &mut t,
            object,
            "PushState",
            &[param("Name", "NewState", 0), param("Name", "NewLabel", CPF_OPT)],
            None,
            0,
        );
        let pop = native(&mut t, object, "PopState", &[param("Bool", "bPopAll", CPF_OPT)], None, 0);
        native(
            &mut t,
            object,
            "IsInState",
            &[param("Name", "TestState", 0), param("Bool", "bTestStateStack", CPF_OPT)],
            Some("Bool"),
            0,
        );
        native(&mut t, object, "Enable", &[param("Name", "ProbeFunc", 0)], None, 0);
        native(&mut t, object, "Disable", &[param("Name", "ProbeFunc", 0)], None, 0);
        native(&mut t, object, "GetStateName", &[], Some("Name"), 0);
        for (name, arg) in [("BeginState", "PreviousStateName"), ("EndState", "NextStateName")] {
            t.function(
                object,
                name,
                &[param("Name", arg, 0)],
                None,
                &[],
                &FunctionSpec::new(VIRTUAL, name),
                |_| {
                    let mut a = Asm::new();
                    a.stmt().return_nothing();
                    a.end();
                    Some(a)
                },
            );
        }
        let actor = t.class("Actor", object);
        t.prop("Float", actor, "LatentFloat", 0, 0);
        let sleep = native(&mut t, actor, "Sleep", &[param("Float", "Seconds", 0)], None, 256);
        let mach = t.class("Mach", actor);
        let trace = t.prop("Str", mach, "Trace", 0, 0);
        let arg = t.prop("Name", mach, "Arg", 0, 0);
        // Items: array<int>, its inner property adopted after the array exists; Item: the loop variable.
        let inner = t.prop("Int", 0, "ItemsInner", 0, 0);
        let items = t.prop("Array", mach, "Items", 0, inner);
        t.p.set_outer(inner, items);
        let item = t.prop("Int", mach, "Item", 0, 0);
        let ids = Ids { trace, arg };
        for (name, text) in [
            ("PausedState", "Pa;"),
            ("PushedState", "Pu;"),
            ("PoppedState", "Po;"),
            ("ContinuedState", "Co;"),
        ] {
            event(&mut t, ids, mach, name, text, None, false);
        }

        // A: Begin: "1"; Sleep(Seconds); "2"; goto 'Next'; "x"; Stop. Next: "3"; Stop.
        let next = t.p.name("Next");
        let code_a = {
            let mut a = Asm::new();
            let a_begin = a.here();
            append(&mut a, trace, "1");
            a.stmt().final_function(sleep, |a| {
                a.float_const(0.375);
            });
            append(&mut a, trace, "2");
            a.stmt().raw(&[0x0D, 0x21]).i32(next).i32(0);
            append(&mut a, trace, "x");
            a.stmt().raw(&[0x08]);
            let a_next = a.here();
            append(&mut a, trace, "3");
            a.stmt().raw(&[0x08]);
            finish(&mut t, a, &[("Begin", a_begin), ("Next", a_next)])
        };
        let state_a = state(&mut t, mach, "A", 0, Some(&code_a));
        event(&mut t, ids, state_a, "BeginState", "bA;", Some("PreviousStateName"), true);
        event(&mut t, ids, state_a, "EndState", "eA;", Some("NextStateName"), true);
        t.function(
            state_a,
            "Tick",
            &[param("Float", "DeltaTime", 0)],
            None,
            &[],
            &FunctionSpec::new(VIRTUAL, "Tick"),
            |_| {
                let mut a = Asm::new();
                a.stmt().return_nothing();
                a.end();
                Some(a)
            },
        );

        // B extends A: Begin: "B"; goto 'Next' (A's label); Stop. No events of its own.
        let code_b = {
            let mut a = Asm::new();
            let begin = a.here();
            append(&mut a, trace, "B");
            a.stmt().raw(&[0x0D, 0x21]).i32(next).i32(0);
            a.stmt().raw(&[0x08]);
            finish(&mut t, a, &[("Begin", begin)])
        };
        state(&mut t, mach, "B", state_a, Some(&code_b));

        // C: BeginState goes to A. D: EndState goes to B. Both without code.
        let (name_a, name_b) = (t.p.name("A"), t.p.name("B"));
        let state_c = state(&mut t, mach, "C", 0, None);
        t.function(
            state_c,
            "BeginState",
            &[param("Name", "PreviousStateName", 0)],
            None,
            &[],
            &FunctionSpec::new(VIRTUAL, "BeginState"),
            |_| {
                let mut a = Asm::new();
                append(&mut a, trace, "bC;");
                call_with_name(&mut a, goto, name_a);
                a.stmt().return_nothing();
                a.end();
                Some(a)
            },
        );
        let state_d = state(&mut t, mach, "D", 0, None);
        event(&mut t, ids, state_d, "BeginState", "bD;", Some("PreviousStateName"), false);
        t.function(
            state_d,
            "EndState",
            &[param("Name", "NextStateName", 0)],
            None,
            &[],
            &FunctionSpec::new(VIRTUAL, "EndState"),
            |_| {
                let mut a = Asm::new();
                append(&mut a, trace, "eD;");
                call_with_name(&mut a, goto, name_b);
                a.stmt().return_nothing();
                a.end();
                Some(a)
            },
        );

        // L1 / L2: each one's code goes to the other.
        let (name_l1, name_l2) = (t.p.name("L1"), t.p.name("L2"));
        for (name, text, other) in [("L1", "1", name_l2), ("L2", "2", name_l1)] {
            let code = {
                let mut a = Asm::new();
                let begin = a.here();
                append(&mut a, trace, text);
                call_with_name(&mut a, goto, other);
                a.stmt().raw(&[0x08]);
                finish(&mut t, a, &[("Begin", begin)])
            };
            state(&mut t, mach, name, 0, Some(&code));
        }

        // P: Begin: "p"; PushState('Q'); "r"; Stop. Q: Begin: "q"; PopState(); "never"; Stop.
        let name_q = t.p.name("Q");
        let code_p = {
            let mut a = Asm::new();
            let begin = a.here();
            append(&mut a, trace, "p");
            call_with_name(&mut a, push, name_q);
            append(&mut a, trace, "r");
            a.stmt().raw(&[0x08]);
            finish(&mut t, a, &[("Begin", begin)])
        };
        state(&mut t, mach, "P", 0, Some(&code_p));
        let code_q = {
            let mut a = Asm::new();
            let begin = a.here();
            append(&mut a, trace, "q");
            a.stmt().final_function(pop, |_| {});
            append(&mut a, trace, "never");
            a.stmt().raw(&[0x08]);
            finish(&mut t, a, &[("Begin", begin)])
        };
        state(&mut t, mach, "Q", 0, Some(&code_q));

        // R: Begin: goto 'Missing'; "never"; Stop.
        let missing = t.p.name("Missing");
        let code_r = {
            let mut a = Asm::new();
            let begin = a.here();
            a.stmt().raw(&[0x0D, 0x21]).i32(missing).i32(0);
            append(&mut a, trace, "never");
            a.stmt().raw(&[0x08]);
            finish(&mut t, a, &[("Begin", begin)])
        };
        state(&mut t, mach, "R", 0, Some(&code_r));

        // F: Begin: foreach Items(Item) { PushState('Q'); } Stop.
        let code_f = {
            let mut a = Asm::new();
            let begin = a.here();
            a.stmt().raw(&[0x58]).instance(items).instance(item).raw(&[0]);
            a.instance(item);
            let at = a.bytes.len();
            a.w(0);
            call_with_name(&mut a, push, name_q);
            a.stmt().raw(&[0x31]);
            let end = u16::try_from(a.here()).unwrap();
            a.stmt().raw(&[0x30]);
            a.bytes[at..at + 2].copy_from_slice(&end.to_le_bytes());
            a.stmt().raw(&[0x08]);
            finish(&mut t, a, &[("Begin", begin)])
        };
        state(&mut t, mach, "F", 0, Some(&code_f));

        // X / Y: each one's BeginState goes to the other (no end).
        let (name_x, name_y) = (t.p.name("X"), t.p.name("Y"));
        for (name, other) in [("X", name_y), ("Y", name_x)] {
            let s = state(&mut t, mach, name, 0, None);
            t.function(
                s,
                "BeginState",
                &[param("Name", "PreviousStateName", 0)],
                None,
                &[],
                &FunctionSpec::new(VIRTUAL, "BeginState"),
                |_| {
                    let mut a = Asm::new();
                    call_with_name(&mut a, goto, other);
                    a.stmt().return_nothing();
                    a.end();
                    Some(a)
                },
            );
        }

        // Plain and Flagged extend Mach with real class bodies whose own state flags are 0; Flagged also declares
        // a state Z with flags 2.
        for name in ["Plain", "Flagged"] {
            let mut body = class_body(&mut t.p, &ClassBodySpec::new(mach, 0, 0));
            // The state flags follow ten words, the (empty) script, the probe mask and the label table offset.
            body[46..48].copy_from_slice(&0u16.to_le_bytes());
            let mut export = Export::new(t.import("Class"), name, body);
            export.super_ = mach;
            let class = t.p.add_export_with(export);
            if name == "Flagged" {
                state_flags(&mut t, class, "Z", 2);
            }
        }

        let remote = t.class("Remote", mach);
        t.prop("Byte", remote, "Role", 0, 0);
        t.build()
    }

    struct Run {
        vm: Vm,
        dir: PathBuf,
    }

    impl Drop for Run {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn start_on(bytes: &[u8], mode: ParityMode) -> Run {
        let dir = std::env::temp_dir().join(format!(
            "ow-vm-states-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Core.upk", bytes)],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            mode,
        )
        .with_stub_policy(StubPolicy::refuse_all());
        natives_core::register(&mut vm).unwrap();
        Run { vm, dir }
    }

    fn start(mode: ParityMode) -> Run {
        start_on(&package(), mode)
    }

    /// A quarter second: exact in binary, so the latent arithmetic below is exact.
    const DT: f32 = 0.25;

    impl Run {
        fn object(&mut self, class: &str) -> ObjRef {
            let c = self.vm.find_class(class).unwrap();
            self.vm.instantiate(c, "").unwrap()
        }

        fn mach(&mut self) -> ObjRef {
            self.object("Core.Mach")
        }

        fn name(&mut self, text: &str) -> Value {
            Value::Name(self.vm.space.names.intern(text))
        }

        fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
            let f = self.vm.find_function(path)?;
            Ok(self.vm.call(f, Some(this), args)?.ret)
        }

        fn goto(&mut self, this: ObjRef, args: &[&str]) {
            let args: Vec<Value> = args.iter().map(|a| self.name(a)).collect();
            self.call("Core.Object.GotoState", this, args).unwrap();
        }

        fn goto_with(&mut self, this: ObjRef, state: &str, force: bool, keep: bool) {
            let state = self.name(state);
            let args = vec![state, Value::None, Value::Bool(force), Value::Bool(keep)];
            self.call("Core.Object.GotoState", this, args).unwrap();
        }

        fn prop(&self, o: ObjRef, name: &str) -> Value {
            let class = self.vm.space.heap.get(o).unwrap().class;
            let prop = self.vm.space.prop(class, name).unwrap();
            self.vm.space.read(o, PropAddr::scalar(prop)).unwrap().clone()
        }

        fn trace(&self, o: ObjRef) -> String {
            match self.prop(o, "Trace") {
                Value::Str(s) => s.to_string(),
                other => panic!("Trace is {other:?}"),
            }
        }

        fn state(&self, o: ObjRef) -> String {
            match self.vm.space.heap.get(o).unwrap().state {
                Some(n) => self.vm.space.names.text(n).into_owned(),
                None => "None".to_string(),
            }
        }

        fn in_state(&mut self, o: ObjRef, state: &str, stack: bool) -> bool {
            let state = self.name(state);
            let v = self
                .call("Core.Object.IsInState", o, vec![state, Value::Bool(stack)])
                .unwrap();
            v == Value::Bool(true)
        }

        fn step(&mut self, o: ObjRef) {
            process_state(&mut self.vm, o, DT).unwrap();
        }

        fn arg(&self, o: ObjRef) -> String {
            match self.prop(o, "Arg") {
                Value::Name(n) => self.vm.space.names.text(n).into_owned(),
                other => panic!("Arg is {other:?}"),
            }
        }
    }

    fn not_yet(r: Result<impl fmt::Debug, VmError>) -> ow_core::NotYet {
        match r {
            Err(VmError::NotYet(n)) => n,
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    // ---- the State export body -----------------------------------------------------------------------------

    #[test]
    fn a_state_body_reads_its_code_label_table_and_parent() {
        let pkg = Package::from_decoded(PathBuf::new(), "Core".into(), package()).unwrap();
        let mut seen = 0;
        for i in 1..=pkg.exports.len() as i32 {
            let e = &pkg.exports[i as usize - 1];
            let is_state = e.class < 0 && pkg.imports[(-e.class - 1) as usize].name == "State";
            if is_state && e.name == "B" {
                let body = read_state_body(&pkg, i).unwrap();
                assert_ne!(body.super_ref, 0);
                assert!(body.script_size > 0);
                assert_ne!(body.label_table, NO_LABEL_TABLE);
                assert_eq!(pkg.data[body.script_offset + body.script_size - 1], op::END_OF_SCRIPT);
                seen += 1;
            }
        }
        assert_eq!(seen, 1);
    }

    /// A one-export package whose State export has `payload`.
    fn one_state(payload: Vec<u8>, super_: i32) -> Package {
        let mut t = ScriptToy::with_imports(&["Class", "State"]);
        let class = t.class("Holder", 0);
        let mut export = Export::new(t.import("State"), "S", payload);
        export.outer = class;
        export.super_ = super_;
        t.p.add_export_with(export);
        Package::from_decoded(PathBuf::new(), "Core".into(), t.build()).unwrap()
    }

    fn body(tagged: bool, super_: i32, script: &[u8], trailing: &[u8]) -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&["Class", "State"]);
        let none = t.none;
        let other = t.p.fname("Other", 0);
        let mut payload = ints(&[0]);
        payload.extend_from_slice(if tagged { &other } else { &none });
        payload.extend_from_slice(&ints(&[0, super_, 0, 0, 0, 0, 0]));
        let len = i32::try_from(script.len()).unwrap();
        payload.extend_from_slice(&ints(&[len, len]));
        payload.extend_from_slice(script);
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&NO_LABEL_TABLE.to_le_bytes());
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&ints(&[0]));
        payload.extend_from_slice(trailing);
        payload
    }

    #[test]
    fn a_state_body_is_checked_as_the_class_body_parts_are() {
        // The names "Other" and "None" sit at different indices in the two builders' tables, so each package is
        // built with its own (the body builder's table is not the package's); only the shape is asserted here.
        let ok = one_state(body(false, 0, &[0x08, 0x53], &[]), 0);
        let found = (1..=ok.exports.len() as i32).find(|&i| ok.exports[i as usize - 1].name == "S").unwrap();
        let read = read_state_body(&ok, found).unwrap();
        assert_eq!((read.script_size, read.memory_size, read.label_table), (2, 2, NO_LABEL_TABLE));
        assert_eq!(
            read_state_body(&one_state(body(false, 0, &[0x08, 0x53], &[0]), 0), found),
            Err(StateBodyError::TrailingBytes)
        );
        assert_eq!(
            read_state_body(&one_state(body(false, 0, &[0x08, 0x08], &[]), 0), found),
            Err(StateBodyError::ScriptNotTerminated)
        );
        assert_eq!(
            read_state_body(&one_state(body(false, 1, &[0x08, 0x53], &[]), 0), found),
            Err(StateBodyError::SuperDiffers)
        );
        assert_eq!(read_state_body(&ok, 99), Err(StateBodyError::IndexOutOfRange(99)));
        let truncated = {
            let mut p = body(false, 0, &[0x08, 0x53], &[]);
            p.truncate(p.len() - 2);
            one_state(p, 0)
        };
        let e = read_state_body(&truncated, found).unwrap_err();
        assert!(matches!(e, StateBodyError::Read(_)), "{e:?}");
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn a_state_body_with_tagged_properties_is_refused() {
        // The tagged-property name must be None: build the package so that "Other" is a valid name in it.
        let mut t = ScriptToy::with_imports(&["Class", "State"]);
        let class = t.class("Holder", 0);
        let other = t.p.fname("Other", 0);
        let none = t.none;
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&other);
        payload.extend_from_slice(&none);
        payload.extend_from_slice(&ints(&[0, 0, 0, 0, 0, 0, 0, 2, 2]));
        payload.extend_from_slice(&[0x08, 0x53]);
        payload.extend_from_slice(&[0; 8]);
        payload.extend_from_slice(&ints(&[0]));
        let mut export = Export::new(t.import("State"), "S", payload);
        export.outer = class;
        let index = t.p.add_export_with(export);
        let pkg = Package::from_decoded(PathBuf::new(), "Core".into(), t.build()).unwrap();
        assert_eq!(read_state_body(&pkg, index), Err(StateBodyError::TaggedProperties));
    }

    // ---- GotoState ------------------------------------------------------------------------------------------

    #[test]
    fn goto_state_ends_the_old_state_switches_then_begins_the_new_one() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["A"]);
        assert_eq!(r.trace(m), "bA;");
        assert_eq!(r.arg(m), "None", "BeginState gets the previous state");
        assert_eq!(r.state(m), "A");
        let got = r.call("Core.Object.GetStateName", m, vec![]).unwrap();
        assert_eq!(got, r.name("A"));
        // The same state, not forced: no events.
        r.goto(m, &["A"]);
        assert_eq!(r.trace(m), "bA;");
        // A -> B: EndState(B) on A, then BeginState(A) on B, which B inherits from its parent A.
        r.goto(m, &["B"]);
        assert_eq!(r.trace(m), "bA;eA;bA;");
        assert_eq!(r.arg(m), "A");
        assert!(r.in_state(m, "B", false));
        assert!(r.in_state(m, "A", false), "a child state is in its parent");
        assert!(!r.in_state(m, "P", false));
        // Forced: both events although the state stays B.
        r.goto_with(m, "B", true, false);
        assert_eq!(r.trace(m), "bA;eA;bA;eA;bA;");
        // To None: EndState(None) only; no state afterwards.
        r.goto(m, &["None"]);
        assert_eq!(r.trace(m), "bA;eA;bA;eA;bA;eA;");
        assert_eq!(r.arg(m), "None");
        assert_eq!(r.state(m), "None");
        assert!(r.vm.log.is_empty(), "{:?}", r.vm.log);
    }

    #[test]
    fn an_unknown_state_ends_the_old_one_and_leaves_no_state() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["A"]);
        r.goto(m, &["Nope"]);
        assert_eq!(r.trace(m), "bA;eA;");
        assert_eq!(r.arg(m), "None", "EndState gets None: the unknown name is reset");
        assert_eq!(r.state(m), "None");
        assert_eq!(r.vm.log, vec!["GotoState (Nope): state not found in Mach".to_string()]);
    }

    #[test]
    fn a_state_change_inside_begin_state_or_end_state_wins() {
        let mut r = start(ParityMode::Off);
        // BeginState of C goes to A: C ends once (the class-level EndState), A begins; the outer change stops.
        let n = r.mach();
        r.goto(n, &["C"]);
        assert_eq!(r.trace(n), "bC;bA;");
        assert_eq!(r.state(n), "A");
        assert_eq!(r.arg(n), "C");
        // EndState of D goes to B; that change stands, the outer one to A is dropped.
        let d = r.mach();
        r.goto(d, &["D"]);
        r.goto(d, &["A"]);
        assert_eq!(r.trace(d), "bD;eD;bA;");
        assert_eq!(r.state(d), "B");
        assert_eq!(r.arg(d), "D");
    }

    #[test]
    fn goto_state_auto_is_no_state_when_every_candidate_state_flag_is_0() {
        let mut r = start(ParityMode::Off);
        let o = r.object("Core.Plain");
        r.goto(o, &["A"]);
        r.goto(o, &["Auto"]);
        assert_eq!(r.trace(o), "bA;eA;", "EndState(None) on A, no BeginState");
        assert_eq!(r.arg(o), "None");
        assert_eq!(r.state(o), "None");
        assert!(r.vm.log.is_empty(), "Auto is silent: {:?}", r.vm.log);
    }

    #[test]
    fn goto_state_auto_is_not_read_yet_when_a_candidate_state_flag_is_set() {
        let mut r = start(ParityMode::Off);
        let o = r.object("Core.Flagged");
        let auto = r.name("Auto");
        let f = r.vm.find_function("Core.Object.GotoState").unwrap();
        assert_eq!(not_yet(r.vm.call(f, Some(o), vec![auto])).missing, Missing::NotRead);
    }

    #[test]
    fn goto_state_auto_is_not_read_yet_when_the_class_body_does_not_read() {
        // Mach has the testkit's 16-byte stand-in class body.
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        let auto = r.name("Auto");
        let f = r.vm.find_function("Core.Object.GotoState").unwrap();
        assert_eq!(not_yet(r.vm.call(f, Some(m), vec![auto])).missing, Missing::NotRead);
    }

    #[test]
    fn an_explicit_label_not_found_is_logged_and_the_default_begin_is_silent() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["C"]);
        assert!(r.vm.log.is_empty(), "C has no code and no Begin: silent");
        r.goto(m, &["A", "Nowhere"]);
        assert_eq!(r.vm.log, vec!["GotoState (A Nowhere): Label not found".to_string()]);
        // No code to run.
        let before = r.trace(m);
        r.step(m);
        assert_eq!(r.trace(m), before);
    }

    // ---- state code, labels and Sleep -----------------------------------------------------------------------

    #[test]
    fn state_code_runs_from_begin_sleeps_and_follows_labels() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["A"]);
        assert_eq!(r.trace(m), "bA;", "state code runs on the next step, not in GotoState");
        // Step 1: "1", then Sleep(0.375) yields.
        r.step(m);
        assert_eq!(r.trace(m), "bA;1");
        assert_eq!(r.prop(m, "LatentFloat"), Value::Float(0.375));
        // Step 2: 0.375 - 0.25 = 0.125, exactly half the delta: not below it, still sleeping.
        r.step(m);
        assert_eq!(r.prop(m, "LatentFloat"), Value::Float(0.125));
        assert_eq!(r.trace(m), "bA;1");
        // Step 3: -0.125 is below half the delta: the sleep ends and the code continues in this step: "2", the
        // jump to Next ("x" skipped), "3", Stop.
        r.step(m);
        assert_eq!(r.prop(m, "LatentFloat"), Value::Float(-0.125));
        assert_eq!(r.trace(m), "bA;123");
        // The code ran out: further steps do nothing.
        r.step(m);
        assert_eq!(r.trace(m), "bA;123");
        // GotoState to the same state restarts its code at Begin (no events).
        r.goto(m, &["A"]);
        r.step(m);
        assert_eq!(r.trace(m), "bA;1231");
        // An explicit label of the same state.
        r.goto(m, &["A", "Next"]);
        r.step(m);
        assert_eq!(r.trace(m), "bA;12313", "the label also ends the pending sleep");
    }

    #[test]
    fn a_label_in_a_parent_state_continues_in_the_parent_code_in_the_same_step() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["B"]);
        r.step(m);
        assert_eq!(r.trace(m), "bA;B3", "B's Begin, then A's Next (B has no Next)");
        assert_eq!(r.state(m), "B", "a label does not change the state");
    }

    #[test]
    fn a_label_not_found_by_goto_label_stops_the_code() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["R"]);
        r.step(m);
        assert_eq!(r.trace(m), "");
        assert_eq!(r.vm.log, vec!["GotoLabel (Missing): Label not found".to_string()]);
        r.step(m);
        assert_eq!(r.trace(m), "");
    }

    #[test]
    fn a_state_change_in_state_code_restarts_at_most_five_times_a_step() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["L1"]);
        r.step(m);
        assert_eq!(r.trace(m), "121212", "one run and five restarts");
        assert_eq!(r.state(m), "L1", "the sixth change happened; its code waits for the next step");
        r.step(m);
        assert_eq!(r.trace(m), "121212121212");
    }

    #[test]
    fn sleep_is_refused_outside_state_code() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        let f = r.vm.find_function("Core.Actor.Sleep").unwrap();
        let e = r.vm.call(f, Some(m), vec![Value::Float(1.0)]).unwrap_err();
        assert!(matches!(e, VmError::Script { .. }), "{e:?}");
    }

    #[test]
    fn a_destroyed_object_or_one_without_code_does_nothing() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.step(m);
        r.goto(m, &["A"]);
        r.vm.space.heap.get_mut(m).unwrap().destroyed = true;
        r.step(m);
        assert_eq!(r.trace(m), "bA;");
    }

    #[test]
    fn state_code_of_an_actor_that_is_not_the_authority_is_not_read_yet() {
        let mut r = start(ParityMode::Off);
        let o = r.object("Core.Remote");
        r.goto(o, &["A"]);
        assert_eq!(not_yet(process_state(&mut r.vm, o, DT)).missing, Missing::NotRead);
        let role = r.vm.space.prop(r.vm.space.heap.get(o).unwrap().class, "Role").unwrap();
        r.vm.space.write(o, PropAddr::scalar(role), Value::Byte(3)).unwrap();
        r.step(o);
        assert_eq!(r.trace(o), "bA;1");
    }

    #[test]
    fn is_in_state_and_get_state_name_agree_on_a_state_set_before_the_frame() {
        let mut r = start(ParityMode::Off);
        // A declared state written into the heap before any state native ran: IsInState resolves it first.
        let m = r.mach();
        let a = r.vm.space.names.intern("A");
        r.vm.space.heap.get_mut(m).unwrap().state = Some(a);
        assert!(r.in_state(m, "A", false));
        let got = r.call("Core.Object.GetStateName", m, vec![]).unwrap();
        assert_eq!(got, r.name("A"));
        // An undeclared one is dropped by the frame: IsInState and GetStateName then both say no state.
        let n = r.mach();
        let nope = r.vm.space.names.intern("Nope");
        r.vm.space.heap.get_mut(n).unwrap().state = Some(nope);
        assert!(!r.in_state(n, "Nope", false));
        let got = r.call("Core.Object.GetStateName", n, vec![]).unwrap();
        assert_eq!(got, Value::Name(NameId::NONE));
    }

    #[test]
    fn state_changes_that_never_settle_give_the_typed_depth_error() {
        // X's BeginState goes to Y, Y's to X: the call depth limit, on the VM's stack-sized thread.
        let message = crate::interp::on_vm_stack(|| {
            let mut r = start(ParityMode::Off);
            let m = r.mach();
            let x = r.name("X");
            let f = r.vm.find_function("Core.Object.GotoState").unwrap();
            match r.vm.call(f, Some(m), vec![x]) {
                Err(VmError::Script { message, .. }) => message,
                other => format!("unexpected: {other:?}"),
            }
        })
        .unwrap();
        assert!(message.starts_with("call depth limit reached"), "{message}");
    }

    #[test]
    fn a_push_state_inside_a_foreach_of_state_code_is_not_read_yet() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        let class = r.vm.space.heap.get(m).unwrap().class;
        let items = r.vm.space.prop(class, "Items").unwrap();
        r.vm.space
            .write(m, PropAddr::scalar(items), Value::array(vec![Value::Int(1)]))
            .unwrap();
        r.goto(m, &["F"]);
        assert_eq!(not_yet(process_state(&mut r.vm, m, DT)).missing, Missing::NotRead);
    }

    #[test]
    fn the_world_functions_send_events_and_report_latent_actions() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        assert!(!latent_pending(&mut r.vm, m).unwrap());
        r.goto(m, &["A"]);
        r.step(m);
        assert!(latent_pending(&mut r.vm, m).unwrap());
        assert_eq!(latent_of(&mut r.vm, m).unwrap(), Some(LatentAction::Sleep));
        // A state event through the state chain, a missing one, a gated probe, a probe the record does not name.
        assert!(find_event(&mut r.vm, m, "EndState").unwrap().is_some());
        let b = r.name("B");
        assert!(send_world_event(&mut r.vm, m, "PausedState", vec![]).unwrap());
        assert!(!send_world_event(&mut r.vm, m, "NoSuchEvent", vec![]).unwrap());
        let end = r.name("EndState");
        r.call("Core.Object.Disable", m, vec![end]).unwrap();
        assert!(!send_world_event(&mut r.vm, m, "EndState", vec![b]).unwrap());
        assert_eq!(r.trace(m), "bA;1Pa;");
        assert_eq!(
            not_yet(send_world_event(&mut r.vm, m, "Bump", vec![])).missing,
            Missing::NotRead
        );
        forget(&mut r.vm, m).unwrap();
        assert!(!latent_pending(&mut r.vm, m).unwrap());
    }

    // ---- the installed game (local, ignored) ----------------------------------------------------------------

    type Counts = std::collections::BTreeMap<String, u64>;

    fn bump(n: &mut Counts, key: &str) {
        *n.entry(key.to_string()).or_insert(0) += 1;
    }

    /// Counts how a serialized probe mask (restricted to the ten named probes) compares with the probe functions an
    /// export declares: all of them (`own.0`), and only those with a script (`own.1`).
    fn compare_mask(n: &mut Counts, kind: &str, serialized: u32, own: (u32, u32)) {
        let known: u32 = PROBES.iter().map(|&(_, id)| 1u32 << (id - 300)).sum();
        let s = serialized & known;
        bump(n, &format!("{kind}: compared"));
        if serialized & !known != 0 {
            bump(n, &format!("{kind}: serialized bits outside the ten named probes"));
        }
        if s == own.0 {
            bump(n, &format!("{kind}: serialized == declared probe functions"));
        }
        if s == own.1 {
            bump(n, &format!("{kind}: serialized == declared probe functions with a body"));
        }
        if s != own.0 && s != own.1 {
            bump(n, &format!("{kind}: serialized differs from both"));
        }
        if own.0 != own.1 {
            bump(n, &format!("{kind}: declares a probe function without a body"));
        }
    }

    /// The structural check behind the fitted State layout, and the serialized probe masks against the probe
    /// functions each state and class declares, on the installed code packages (`OW_COOKED`, else the default Steam
    /// folder). Prints counts only and writes nothing. Run with
    /// `cargo test -p ow-vm --lib -- --ignored --nocapture installed`.
    #[test]
    #[ignore = "reads the installed game"]
    fn installed_state_exports_meet_the_fitted_layout() {
        use ow_script::function::{is_function_export, read_function};
        let cooked = std::env::var("OW_COOKED").unwrap_or_else(|_| {
            "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole".to_string()
        });
        let mut n = Counts::new();
        for package in ow_object::reflect::CODE_PACKAGES {
            let path = std::path::Path::new(&cooked).join(format!("{package}.upk"));
            let pkg = Package::load(&path).unwrap();
            let class_of = |i: i32| -> String {
                match pkg.exports[i as usize - 1].class {
                    0 => "Class".to_string(),
                    c if c < 0 => pkg.imports[(-c - 1) as usize].name.clone(),
                    c => pkg.exports[c as usize - 1].name.clone(),
                }
            };
            // The probe bits of the functions `owner` declares (all of them, and those with a script).
            let declared = |owner: i32| -> (u32, u32) {
                let (mut all, mut with_body) = (0, 0);
                for i in 1..=pkg.exports.len() as i32 {
                    let e = &pkg.exports[i as usize - 1];
                    if e.outer != owner || !is_function_export(&pkg, i) {
                        continue;
                    }
                    if let Probe::Bit(bit) = probe(&e.name) {
                        all |= bit;
                        if read_function(&pkg, i).is_ok_and(|f| f.has_script()) {
                            with_body |= bit;
                        }
                    }
                }
                (all, with_body)
            };
            for i in 1..=pkg.exports.len() as i32 {
                match class_of(i).as_str() {
                    "State" => {
                        bump(&mut n, "state: exports");
                        let Ok(body) = read_state_body(&pkg, i) else {
                            bump(&mut n, "state: body does not read");
                            continue;
                        };
                        bump(&mut n, "state: body ends at the exact size");
                        compare_mask(&mut n, "state probe mask", body.probe_mask, declared(i));
                        if body.script_size == 0 {
                            continue;
                        }
                        let info = FunctionInfo {
                            index: i,
                            memory_size: body.memory_size,
                            script_offset: body.script_offset,
                            script_size: body.script_size,
                            ..FunctionInfo::default()
                        };
                        let Ok(code) = ow_script::decode::decode(&pkg, &info) else {
                            bump(&mut n, "state: code does not decode");
                            continue;
                        };
                        bump(&mut n, "state: code decodes");
                        if body.label_table == NO_LABEL_TABLE {
                            bump(&mut n, "state: no label table");
                            continue;
                        }
                        let table = u32::from(body.label_table)
                            .checked_sub(1)
                            .and_then(|at| code.statement_at.get(&at))
                            .and_then(|&s| code.statements.get(s))
                            .filter(|s| s.op == op::LABEL_TABLE);
                        let Some(table) = table else {
                            bump(&mut n, "state: label table offset misses");
                            continue;
                        };
                        bump(&mut n, "state: label table offset one past a LabelTable token");
                        for (_, offset) in &table.labels {
                            let lands = u32::try_from(*offset)
                                .is_ok_and(|o| code.statement_at.contains_key(&o));
                            bump(
                                &mut n,
                                if lands {
                                    "state: label on a statement"
                                } else {
                                    "state: label off a statement"
                                },
                            );
                        }
                    }
                    "Class" => match read_class_state_part(&pkg, i) {
                        Ok(body) => {
                            compare_mask(&mut n, "class probe mask", body.probe_mask, declared(i))
                        }
                        Err(_) => bump(&mut n, "class: state part does not read"),
                    },
                    _ => {}
                }
            }
        }
        for (k, v) in &n {
            println!("{k}: {v}");
        }
    }

    // ---- PushState and PopState -----------------------------------------------------------------------------

    #[test]
    fn push_state_pauses_and_pop_state_resumes_where_the_code_was() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["P"]);
        // P: "p", PushState('Q') (Paused, Pushed), restart in Q: "q", PopState() (Popped, Continued), restart in P
        // after the PushState: "r", Stop.
        r.step(m);
        assert_eq!(r.trace(m), "pPa;Pu;qPo;Co;r");
        assert_eq!(r.state(m), "P");
    }

    #[test]
    fn the_push_stack_counts_for_is_in_state_and_goto_state_empties_it() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["P"]);
        let (q, p) = (r.name("Q"), r.name("P"));
        r.call("Core.Object.PushState", m, vec![q.clone()]).unwrap();
        assert_eq!(r.trace(m), "Pa;Pu;");
        assert_eq!(r.state(m), "Q");
        assert!(!r.in_state(m, "P", false));
        assert!(r.in_state(m, "P", true));
        // A state already on the stack, or the current one, is not pushed again.
        r.call("Core.Object.PushState", m, vec![q]).unwrap();
        r.call("Core.Object.PushState", m, vec![p]).unwrap();
        assert_eq!(r.trace(m), "Pa;Pu;");
        // bKeepStack keeps it.
        r.goto_with(m, "A", false, true);
        assert!(r.in_state(m, "P", true));
        // Without it, every pushed state is popped (PoppedState, no ContinuedState) before EndState.
        r.goto(m, &["B"]);
        assert!(!r.in_state(m, "P", true));
        assert_eq!(r.trace(m), "Pa;Pu;bA;Po;bA;");
        assert_eq!(r.state(m), "B");
    }

    #[test]
    fn pop_state_with_pop_all_empties_the_stack() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        r.goto(m, &["C"]);
        let (p, q) = (r.name("P"), r.name("Q"));
        r.call("Core.Object.PushState", m, vec![p]).unwrap();
        r.call("Core.Object.PushState", m, vec![q]).unwrap();
        assert_eq!(r.state(m), "Q");
        r.call("Core.Object.PopState", m, vec![Value::Bool(true)]).unwrap();
        assert_eq!(r.state(m), "A", "C's BeginState went to A before the pushes");
        assert!(r.trace(m).ends_with("Po;Co;Po;Co;"), "{}", r.trace(m));
        // Nothing pushed: nothing happens.
        r.call("Core.Object.PopState", m, vec![]).unwrap();
        assert_eq!(r.state(m), "A");
    }

    // ---- Enable, Disable and the probe mask -----------------------------------------------------------------

    #[test]
    fn disable_gates_a_probe_until_the_next_switch_and_enable_needs_a_definition() {
        let mut r = start(ParityMode::Off);
        let m = r.mach();
        // No frame yet: every event is sent.
        assert!(probe_enabled(&mut r.vm, m, "Tick").unwrap());
        r.goto(m, &["A"]);
        assert!(probe_enabled(&mut r.vm, m, "Tick").unwrap(), "A defines Tick");
        let tick = r.name("Tick");
        r.call("Core.Object.Disable", m, vec![tick.clone()]).unwrap();
        assert!(!probe_enabled(&mut r.vm, m, "Tick").unwrap());
        r.call("Core.Object.Enable", m, vec![tick.clone()]).unwrap();
        assert!(probe_enabled(&mut r.vm, m, "Tick").unwrap());
        // EndState disabled: A -> B sends no EndState; the switch rebuilds the mask, so BeginState is sent.
        let end = r.name("EndState");
        r.call("Core.Object.Disable", m, vec![end]).unwrap();
        r.goto(m, &["B"]);
        assert_eq!(r.trace(m), "bA;bA;");
        // In P nothing defines Tick: Enable cannot set it.
        r.goto(m, &["P"]);
        r.call("Core.Object.Enable", m, vec![tick]).unwrap();
        assert!(!probe_enabled(&mut r.vm, m, "Tick").unwrap());
        // Not a probe: logged with the record's text; any other event is always sent.
        let foo = r.name("Foo");
        r.call("Core.Object.Enable", m, vec![foo]).unwrap();
        assert_eq!(r.vm.log, vec!["Enable: 'Foo' is not a probe function".to_string()]);
        assert!(probe_enabled(&mut r.vm, m, "Foo").unwrap());
        // A probe id the record does not name.
        let bump = r.name("Bump");
        let f = r.vm.find_function("Core.Object.Disable").unwrap();
        assert_eq!(not_yet(r.vm.call(f, Some(m), vec![bump])).missing, Missing::NotRead);
        assert_eq!(not_yet(probe_enabled(&mut r.vm, m, "Bump")).missing, Missing::NotRead);
    }

    // ---- Reference mode keeps the C++ stand-in --------------------------------------------------------------

    #[test]
    fn under_reference_no_state_code_runs_and_the_new_natives_stay_unbound() {
        let mut r = start(ParityMode::Reference);
        let m = r.mach();
        r.goto(m, &["A"]);
        assert_eq!(r.trace(m), "bA;");
        process_state(&mut r.vm, m, DT).unwrap();
        assert_eq!(r.trace(m), "bA;", "the C++ runs no state code");
        let q = r.name("Q");
        let f = r.vm.find_function("Core.Object.PushState").unwrap();
        assert!(matches!(r.vm.call(f, Some(m), vec![q]), Err(VmError::UnboundNative(_))));
        let f = r.vm.find_function("Core.Actor.Sleep").unwrap();
        assert!(matches!(
            r.vm.call(f, Some(m), vec![Value::Float(1.0)]),
            Err(VmError::UnboundNative(_))
        ));
        assert!(probe_enabled(&mut r.vm, m, "Bump").unwrap(), "no probe mask in the C++");
    }
}
