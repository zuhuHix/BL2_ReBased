//! Owner: lane W3-INTERP. The UnrealScript interpreter core (architecture section 8, lane W3-INTERP): a port of
//! `src/interp.cpp` and of the run half of `src/vm.cpp` (`functionAt`, `findMethod`, `findFunction`, `findClass`,
//! `buildNativeIndex`, `nativeByIndex`). AI-assisted (Claude Code).
//!
//! **What runs here.** One bytecode frame per script call ([`run_frame`], reached through `Vm::call` →
//! `Vm::run_bytecode`, which keeps the depth bookkeeping): locals, parameters and out parameters, the optional-parameter
//! prologue, `Return`, `Jump`/`JumpIfNot`, `Switch`/`Case`, `Context` and `ClassContext`, the primitive, dynamic and
//! meta casts, the constants, virtual, final, global and super calls, numbered natives (`&&`/`||` short-circuit as
//! the C++ does), `New`, `Assert`, the Gearbox hidden typed temporaries (opcodes 0x4C-0x50), the attribute read 0x5E
//! and the typed let 0x5F (through `ow_attr::registry::let_base`), the step budget (one step per statement, per
//! expression and per call, as the C++ `tick`), the census of `Context` expressions whose object was None, and the
//! `<- caller` trace of the depth-limit message. Out parameters alias the caller's storage live (`eval.rs`). A state
//! variable reads as an instance variable and a `GotoLabel` is stepped over, as the C++ does (latent state code is
//! W5-STATES'; both UNVERIFIED in the game).
//!
//! **Aggregates, `foreach`, delegates and interfaces** (W4-INTERP2, decision 2026-11-15-20): struct members, array
//! elements and the dynamic-array operations (`aggregate.rs`, places inside an aggregate are `eval::Place::In`),
//! `foreach` over dynamic arrays and engine iterators (`iter.rs`), delegates (`delegate.rs`), and the interface
//! context and casts through `Vm::implements`. A static array read as a whole is the array of its elements, and a
//! callee's static-array parameter takes such an array one element a slot (decision 2026-11-15-28); assigning one
//! whole, passing it `out` aliased, and a static-array parameter given a value of another shape stay `NotYet`.
//!
//! **State code** (W5-STATES, `crate::states`): a state's code runs as a frame with no locals, from the position the
//! object's state frame holds; a `GotoLabel` and a latent call hand control back to `crate::states`. Under
//! `ParityMode::Reference` state code is not run (the C++), and a `GotoLabel` in a function is stepped over.
//!
//! **Loading** (`load.rs`). Functions and classes come from the code packages of the definition store, through
//! `DefStore::code_mut` (its package store and reflection index, so one `CodeIndex` defines every layout). Loading a
//! class declares every function it and its states declare, with parameters typed by
//! `CodeIndex::child_declarations` and the native key built as `vm.cpp:600-606` builds it; the bytecode is decoded
//! on a function's first call, as the C++ does. Class default objects (a call with no object, `Class.static`,
//! `default.` reads, `New`) come from `DefStore::class_default`, built on first use by the definition store.
//!
//! **Stack depth.** The interpreter recurses (expression, call, frame), the decoder recurses up to 256 levels inside
//! a call, and calls nest up to `Limits::max_call_depth` (200) frames. Expressions nest at most [`MAX_NESTING`]
//! levels across all active frames (a typed error past it, a Rust-only bound), so the stack a call needs is bounded.
//! That bound needs more than a 1 MiB main thread in a debug build: a caller runs the VM on a thread of at least
//! [`STACK_SIZE`] bytes ([`on_vm_stack`]), as `ow-cli` does for every VM command and the deep unit tests do. Decoding
//! stays lazy (inside the first call), as the C++.
//!
//! **Values.** The C++ keeps one dynamically typed bag per object; here properties live in typed heap slots
//! (decision 2026-11-15-02), so a few representation rules are added at a heap write, after the C++ assignment rule
//! (`interp.cpp:511-522`): a `None` value or the `None` literal stored into a class, interface or delegate property
//! becomes that kind's null, an object reference stored into an interface property becomes an interface reference
//! (and back), and a `None` value stored into any other property becomes the property's zero (the C++ stores the
//! `None` kind there, which reads as 0 everywhere a number is read). A class stored into an object property declared
//! as a class's ancestor (`Core.Object` .. `Core.Class`) is kept (a class is an object in UnrealScript), and an object
//! of an engine-native stand-in class that the heap refuses for a typed object property is `NotYet`, since its real
//! chain is unknown (decision 2026-11-15-08). Locals stay untyped, as in the C++. String
//! constants: an ANSI constant's bytes are read one character each (Latin-1, the UE3 widening of an ANSI character),
//! a Unicode constant's UTF-16 units are decoded with lone surrogates as U+FFFD (the `Value::Str` limit, decision
//! 2026-11-15-07). For ASCII both equal the C++ text; past ASCII the C++ keeps the raw bytes.
//!
//! Status: a port of the C++ reading. Every rule here is UNVERIFIED in the running game unless its line says
//! otherwise; the C++ labels are kept where it has them.

mod aggregate;
mod attr;
mod convert;
mod delegate;
mod eval;
mod iter;
mod load;

#[cfg(test)]
mod tests;

/// Whether script function `f` can run here: its loader record has no failure and its script decodes (the call's own
/// preparation, `eval::prepare`). `Ok(false)`: no package export is behind it (declared by hand), so no bytecode.
pub(crate) fn prepare_script(vm: &mut crate::api::Vm, f: ow_object::ids::FuncId) -> Result<bool, crate::error::VmError> {
    if load::code_of(vm, f).is_none() {
        return Ok(false);
    }
    eval::prepare(vm, f)?;
    Ok(true)
}

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ow_core::FixedHashMap;
use ow_object::ids::{ClassId, FuncId};
use ow_object::layout::PropertyDecl;
use ow_package::PackageId;
use ow_script::decode::Code;
use ow_script::function::FunctionInfo;

pub use convert::{describe, describe_for_string};
pub use load::instantiate_from_defaults;

pub(crate) use eval::{Alias, Place, alias_value, check_alias_write, check_entry, is_a_named, is_child, prepare, run_frame, write_alias};
pub(crate) use load::{
    class_at, declare_state_code, find_class, find_class_if_present, find_function, find_function_if_any, is_state_code,
};

/// The stack a thread running the VM needs (module documentation, "Stack depth"). Measured locally on one shape:
/// `Limits::max_call_depth` (200) frames, each a statement nesting 252 expression levels that go eval -> native ->
/// call (the `the_worst_case_nesting_fits_the_vm_stack` unit test): 278 MB against 512 MiB in a debug build, about
/// 5.5 KB a level (65 MB, 1.3 KB a level, in a release build). A second shape adds a function decoded first at that
/// depth (`a_decode_at_depth_fits_the_vm_stack`). UNVERIFIED as a bound for every dispatch path: another path through
/// the interpreter may cost more stack a level.
pub const STACK_SIZE: usize = 512 * 1024 * 1024;

/// How deep expressions (`eval` and `lval`) nest across all active frames before a call is refused with a typed
/// error (a Rust-only bound): 200 frames of 256 levels each, the decoder's own nesting bound, fit under it.
pub const MAX_NESTING: u32 = 200 * 257;

/// The value slots one frame may hold (a Rust-only refusal, checked before the frame is allocated). The installed
/// code packages need at most 257 (16,323 functions with children, measured locally).
pub const MAX_FRAME_SLOTS: usize = 4096;

/// The value slots all active frames together may hold (a Rust-only refusal, checked with [`MAX_FRAME_SLOTS`]).
pub const MAX_LIVE_SLOTS: usize = 1 << 20;

/// Runs `f` on a new thread with [`STACK_SIZE`] bytes of stack and returns its result: the way to run a VM call
/// from a thread whose stack is smaller (a `Vm` is not `Send`, so `f` builds or owns the VM it uses). A panic in `f`
/// is handed back as [`VmThreadError::Panicked`] (not re-raised), and a thread that cannot be started as
/// [`VmThreadError::Spawn`].
pub fn on_vm_stack<T: Send>(f: impl FnOnce() -> T + Send) -> Result<T, VmThreadError> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("ow-vm".into())
            .stack_size(STACK_SIZE)
            .spawn_scoped(scope, f)
            .map_err(VmThreadError::Spawn)?;
        handle.join().map_err(|_| VmThreadError::Panicked)
    })
}

/// Why [`on_vm_stack`] has no result.
#[non_exhaustive]
#[derive(Debug)]
pub enum VmThreadError {
    /// The thread could not be started.
    Spawn(std::io::Error),
    /// The closure panicked on the VM thread.
    Panicked,
}

impl std::fmt::Display for VmThreadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmThreadError::Spawn(e) => write!(f, "cannot start the VM thread: {e}"),
            VmThreadError::Panicked => f.write_str("the VM thread panicked"),
        }
    }
}

impl std::error::Error for VmThreadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VmThreadError::Spawn(e) => Some(e),
            VmThreadError::Panicked => None,
        }
    }
}

/// What the interpreter keeps per VM: the loaded functions and classes, the native index and the active frames.
#[derive(Debug, Default)]
pub(crate) struct CodeState {
    /// Script functions running, innermost last (the C++ `Interp::stack`), for the depth-limit message.
    pub(crate) stack: Vec<FuncId>,
    /// Function export (filed package id, index) -> its function.
    functions: FixedHashMap<(PackageId, i32), FuncId>,
    /// What the loader read for each function it declared.
    code: FixedHashMap<FuncId, Rc<FuncCode>>,
    /// Classes whose functions and states are loaded.
    classes: FixedHashMap<ClassId, Rc<ClassCode>>,
    /// Native index -> the native (or why it could not be loaded), built on first use over the code packages.
    natives: Option<Rc<FixedHashMap<u32, Result<FuncId, String>>>>,
    /// (filed package, outer export) -> the typed child declarations of that export, for property references.
    children: FixedHashMap<(PackageId, i32), Children>,
    /// The storage of the active frames, innermost last (`eval::Frame` holds its index).
    frames: Vec<FrameStore>,
    /// The value slots the active frames hold together (bounded by [`MAX_LIVE_SLOTS`]).
    live_slots: usize,
    /// Expression levels (`eval`, `lval`) in progress across the active frames (bounded by [`MAX_NESTING`]).
    nesting: u32,
    /// The serial number of the last frame pushed (every frame gets the next one; none is reused).
    frame_serial: u64,
}

impl CodeState {
    /// Whether `class`'s functions and states are loaded (a read).
    pub(crate) fn has_class(&self, class: ClassId) -> bool {
        self.classes.contains_key(&class)
    }
}

/// One active frame's storage.
#[derive(Debug, Default)]
struct FrameStore {
    /// The serial number the VM gave this frame (`CodeState::frame_serial`).
    serial: u64,
    /// Every declaration's elements, at the slots the loader gave them.
    slots: Vec<ow_object::value::Value>,
    /// Local-variable references to no declaration of the function (the C++ makes such a slot on first use).
    extra: FixedHashMap<i32, ow_object::value::Value>,
    /// The hidden typed temporaries 4C-50, by (opcode, operand).
    typed: BTreeMap<(u8, i32), ow_object::value::Value>,
}

/// The typed child declarations of one export, or why they did not type.
type Children = Result<Rc<Vec<PropertyDecl>>, String>;

/// One function the loader declared from a package export.
#[derive(Debug)]
pub(crate) struct FuncCode {
    /// The filed id of the package that holds it.
    package: PackageId,
    /// `Package.Outer.Name`.
    path: String,
    info: FunctionInfo,
    owner: Option<ClassId>,
    /// Every child declaration (parameters, locals, the return value) with its first frame slot, in declaration
    /// order.
    locals: Vec<Local>,
    /// The child export index of each entry of `locals`.
    by_export: FixedHashMap<i32, usize>,
    /// The `locals` entry of each parameter, in order.
    params: Vec<usize>,
    /// The `locals` entry of the return value.
    result: Option<usize>,
    /// Frame slots: one per element of every declaration.
    slot_count: usize,
    /// Why the function cannot run (its header did not read, or a declaration did not type).
    failure: Option<String>,
    /// The decoded script, decoded on the first call (the C++ `Function::code` / `decodeFailed`).
    script: RefCell<Option<Result<Rc<Code>, String>>>,
    /// A state's code (W5-STATES, `crate::states`), not a function: it resumes where the object's state frame says,
    /// follows labels and stops at a latent call.
    state_code: bool,
}

#[derive(Debug)]
struct Local {
    decl: PropertyDecl,
    slot: usize,
}

/// The functions and states one class declares (not its super classes'), by lower-cased name.
#[derive(Debug, Default, Clone)]
struct ClassCode {
    functions: BTreeMap<String, FuncId>,
    states: BTreeMap<String, BTreeMap<String, FuncId>>,
}
