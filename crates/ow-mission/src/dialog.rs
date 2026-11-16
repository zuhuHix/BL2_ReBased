//! Owner: lane W5-DIALOG. `Behavior_TriggerDialogEvent` and the dialog it triggers: the port of `src/dialog.*`.
//! AI-assisted (Claude Code).
//!
//! Every rule comes from `docs/verification/NATIVE_DIALOG.md`, `NATIVE_DIALOG_GROUPS.md`,
//! `NATIVE_BEHAVIOR_CONTEXT.md` and `NATIVE_BEHAVIOR_POPULATION.md`, read from native code and **UNVERIFIED in the
//! running game**, except where a rule says it was read from the live game (the Marcus fallback in
//! [`DialogSystem`]'s talker choice, `FIRE_MISSION_GOLDEN_TRACE.md`). Porting changes nothing about what is known.
//!
//! **The behavior** ([`DialogSystem::behavior`]): its first run selects Out (id 0) and becomes latent for one kernel
//! wake; the next run triggers the dialog; while a line is live the behavior polls every 0.1 s and selects Finished
//! (id 1) on the first poll where the event's talk act is no longer live (at once when no line started).
//! `bForcePlayImmediate` triggers in the first run and selects Finished, then Out. An application to an empty context
//! list does not run (no Out, no Finished, no wait) and is listed in [`DialogSystem::not_run`]. It runs in lane
//! W5-BEHAVIOR's kernel (`ow_behavior`, decision 2026-11-15-30): [`handler`] is its `HandlerFn`, which
//! [`register`] adds to the VM's behavior registry under [`BEHAVIOR_CLASS`]; it reads the kernel's run record
//! (`Provider::run_info`: the initial run, the linked outputs, the wait, and the per-thread state, which holds a
//! [`DialogBehaviorState`]) and the behavior's Context link (`Provider::linked_objects`). With no Context link the
//! application is one null entry, the mission, as the C++ asks the provider with no own object.
//!
//! **The dialog**: the group's last enabled `DialogEvents` entry for the tag; its inline talk act (`OutputAction`)
//! or, with none, the act the group's link table points to (a `TalkActs` template or a node by `NodeID`); the talker
//! (the instigator, or a random `TalkData` entry resolved by exact name tag to a registered talker, or an echo caller
//! when none is registered); the priority arbitration (index in the dialog globals' `Priorities`, the tracked
//! mission's floor) before the line starts; then a live line that ends when the audio does (+ `OutputDelay`). Nodes
//! off the Fire route (talker variables, chance, compare and random-branch nodes, an object-parameter switch whose
//! chosen output is linked, sound-effect events, other output links on a talk act) are
//! [`DialogError::NotImplemented`], the C++ "dialog: not implemented: ..." error.
//!
//! **The audio**: a host that can play a line sets a line player ([`DialogSystem::set_line_player`],
//! [`DialogSystem::set_timed_line_player`]); without one no line starts (the Talk does nothing without an audio
//! device), so the behavior finishes on the run that triggers the dialog. [`DialogSystem::set_test_line_length`] is a
//! player for tests: every line starts and ends after a fixed time.
//!
//! **Where the Rust port differs from the C++ one** (representation, not game rules; decision 2026-11-15-32):
//! - Objects come from the definition store, one object per path (decision 2026-11-15-23), not a copy per
//!   `path@package` as the C++ `load` makes. The group identity of the parent search is that object; the C++ key
//!   `path@package` differs only where two packages export the same path, which the store's identity policy makes
//!   one object.
//! - An object path is the path of the export (or import) the object came from in its package, the C++ `pathOf`.
//! - The random `TalkData` pick: under `ParityMode::Reference` the C++ `std::mt19937` seeded 12345 (`rng() % n`), a
//!   reference stand-in (architecture section 6.1); under `ParityMode::Off` the world's `CrtStream::Dialog`
//!   (`rand15 % n`). The mode is the VM's (`Vm::parity`), read at each pick. How the game picks is not read
//!   (UNVERIFIED).
//! - A chain of Trigger acts deeper than [`MAX_TRIGGER_DEPTH`] is an error (the C++ has no bound: package data could
//!   recurse until the stack overflows).
//! - A link entry that lacks `LinkNumber` or `ToNodeID` is [`DialogError::Malformed`] (the C++ reads through a null
//!   pointer there).
//!
//! One `DialogSystem` works with one VM: every call takes it, and the objects it keeps are that VM's.

use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use ow_behavior::kernel::{BehaviorId, Provider};
use ow_behavior::provider::BehaviorError;
use ow_behavior::registry::{self, Handler};

use ow_core::ParityMode;
use ow_core::rand::{CrtStream, Rand15};
use ow_defs::store::DefError;
use ow_object::heap::{HeapError, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::PackageId;
use ow_package::package::PackageError;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The records of these rules.
pub const RECORD: &str = "NATIVE_DIALOG.md, NATIVE_DIALOG_GROUPS.md, NATIVE_BEHAVIOR_CONTEXT.md";

/// The dialog globals' path in the mission's package.
pub const GLOBALS_PATH: &str = "GD_Globals.Dialog.DialogGlobals";

/// How deep a chain of Trigger acts may go (a Rust-only bound, module documentation).
pub const MAX_TRIGGER_DEPTH: u32 = 16;

/// How many `ParentGroup` links [`DialogSystem`] follows to a root group (the C++ bound).
const ROOT_DEPTH: usize = 8;

/// The poll interval of a live line, in seconds (NATIVE_DIALOG.md).
pub const POLL: f64 = 0.1;

/// The latency of the first run, in seconds: one kernel wake.
pub const FIRST_WAKE: f64 = 0.001;

/// Why a dialog call failed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DialogError {
    /// A node off the modelled route (the C++ "dialog: not implemented: <what>").
    NotImplemented(String),
    /// A reference that loads nothing (the C++ "dialog: unresolved object reference[ <path>]").
    Unresolved(Option<String>),
    /// Data the C++ reads without a check (module documentation).
    Malformed(String),
    Vm(VmError),
    Def(DefError),
    Package(PackageError),
}

impl DialogError {
    /// The text the C++ `what()` gives (the Rust-only errors their own text).
    pub fn cpp_text(&self) -> String {
        match self {
            DialogError::NotImplemented(what) => format!("dialog: not implemented: {what}"),
            DialogError::Unresolved(None) => "dialog: unresolved object reference".to_string(),
            DialogError::Unresolved(Some(path)) => {
                format!("dialog: unresolved object reference {path}")
            }
            DialogError::Malformed(what) => format!("dialog: {what}"),
            DialogError::Vm(VmError::Script { message, .. }) => message.clone(),
            other => other.to_string(),
        }
    }
}

impl fmt::Display for DialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogError::Vm(e) => write!(f, "dialog: {e}"),
            DialogError::Def(e) => write!(f, "dialog: {e}"),
            DialogError::Package(e) => write!(f, "dialog: {e}"),
            other => f.write_str(&other.cpp_text()),
        }
    }
}

impl std::error::Error for DialogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DialogError::Vm(e) => Some(e),
            DialogError::Def(e) => Some(e),
            DialogError::Package(e) => Some(e),
            _ => None,
        }
    }
}

impl From<VmError> for DialogError {
    fn from(e: VmError) -> Self {
        DialogError::Vm(e)
    }
}

impl From<DefError> for DialogError {
    fn from(e: DefError) -> Self {
        DialogError::Def(e)
    }
}

impl From<HeapError> for DialogError {
    fn from(e: HeapError) -> Self {
        DialogError::Vm(VmError::Heap(e))
    }
}

impl From<PackageError> for DialogError {
    fn from(e: PackageError) -> Self {
        DialogError::Package(e)
    }
}

fn not_implemented(what: impl Into<String>) -> DialogError {
    DialogError::NotImplemented(what.into())
}

/// One line the dialog chose (C++ `DialogSystem::Line`). Paths are object paths ("" = none); `talker` is the
/// talker's name tag.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    pub id: i32,
    pub event_tag: String,
    pub group: String,
    pub talk_act: String,
    pub ak_event: String,
    pub talker: String,
    /// The talker is an echo caller, not a registered pawn.
    pub echo: bool,
    pub output_delay: f64,
}

/// `WillowPawn.GetDialogGroups` and the dialog name tag of a pawn's body class (C++ `PawnDialog`).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PawnDialog {
    pub groups: Vec<Value>,
    pub name_tag: String,
}

/// A handle on one event data object: its pool index and the use count it was taken at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    index: i32,
    use_count: i32,
}

impl Default for Handle {
    fn default() -> Self {
        Handle {
            index: -1,
            use_count: 0,
        }
    }
}

/// What a `Behavior_TriggerDialogEvent` application keeps from its first run to its resumed runs (the kernel's
/// per-thread state): the event data it triggered and the talker of its context object (-1: none, the mission).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogBehaviorState {
    handle: Handle,
    instigator: i32,
}

impl Default for DialogBehaviorState {
    fn default() -> Self {
        DialogBehaviorState {
            handle: Handle::default(),
            instigator: -1,
        }
    }
}

/// The objects behavior `id` is applied to: those of its Context-type variable link, else one null entry (the C++
/// `provider.contexts(behavior, nullptr)`: the mission, which is no actor).
fn context_objects(provider: &Provider, id: BehaviorId) -> Vec<Option<ObjRef>> {
    match provider
        .behavior(id)
        .variables
        .iter()
        .find(|l| l.link_type == "BVARLINK_Context")
    {
        Some(link) => provider
            .linked_objects(id, link)
            .into_iter()
            .map(Some)
            .collect(),
        None => vec![None],
    }
}

/// The dialog state the kernel keeps for this behavior's thread; a state of another type is an error.
fn state_of(provider: &mut Provider) -> Result<Option<DialogBehaviorState>, DialogError> {
    match &provider.run_info().state {
        None => Ok(None),
        Some(state) => state
            .downcast_ref::<DialogBehaviorState>()
            .copied()
            .map(Some)
            .ok_or_else(|| {
                DialogError::Malformed("the behavior's state is not a dialog state".to_string())
            }),
    }
}

fn set_state(provider: &mut Provider, state: Option<DialogBehaviorState>) {
    provider.run_info().state = state.map(|s| Rc::new(s) as Rc<dyn Any>);
}

/// The behavior class the dialog runs (its handler's registry key).
pub const BEHAVIOR_CLASS: &str = "GearboxFramework.Behavior_TriggerDialogEvent";

/// The kernel handler of [`BEHAVIOR_CLASS`] over `dialog` ([`DialogSystem::behavior`]; a dialog error becomes the
/// kernel's error, the C++ exception through the kernel).
pub fn handler(dialog: Rc<RefCell<DialogSystem>>) -> Handler {
    Handler::run(
        move |vm: &mut Vm, provider: &mut Provider, id: BehaviorId, _event: &str| {
            let mut dialog = dialog.try_borrow_mut().map_err(|_| {
                BehaviorError::Data("dialog: the dialog system is already in use".to_string())
            })?;
            Ok(dialog.behavior(vm, provider, id)?)
        },
    )
}

/// Registers [`handler`] for [`BEHAVIOR_CLASS`] in the VM's behavior registry (a second registration is refused).
///
/// For a VM with one dialog system (the CLI, tests): every provider of the VM then talks through `dialog`. A mission
/// tracker that keeps its own dialog system uses [`handler`] on its own provider instead (`Provider::handle`), with
/// the same `Rc` it gives the dialog component's `TriggerEvent` native, so the dialog state is not split in two.
pub fn register(vm: &mut Vm, dialog: Rc<RefCell<DialogSystem>>) -> Result<(), VmError> {
    registry::register(vm, BEHAVIOR_CLASS, handler(dialog))
}

/// The kernel's view of a dialog failure: VM, store and package errors keep their kinds; the dialog's own errors
/// carry their C++ text.
impl From<DialogError> for BehaviorError {
    fn from(e: DialogError) -> Self {
        match e {
            DialogError::Vm(e) => BehaviorError::Vm(e),
            DialogError::Def(e) => BehaviorError::Vm(e.into()),
            DialogError::Package(e) => BehaviorError::Package(e),
            other => BehaviorError::Data(other.cpp_text()),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct EventData {
    use_count: i32,
    live: bool,
    playing: bool,
    line_id: i32,
    finish_time: f64,
    output_delay: f64,
    test_end: f64,
    /// The timed player's audio end on the dialog clock (-1: none).
    audio_end: f64,
    root_group: String,
    talker: i32,
}

impl EventData {
    fn new(use_count: i32) -> Self {
        EventData {
            use_count,
            test_end: -1.0,
            audio_end: -1.0,
            talker: -1,
            ..EventData::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TalkEntry {
    name_tag: String,
    ak_event: String,
}

#[derive(Debug, Clone, Default)]
struct Act {
    path: String,
    talk: Vec<TalkEntry>,
    output_delay: f64,
    instigator_talker: bool,
    /// `bEnableNoMatch`.
    no_match: bool,
    /// The node on output 1 (a Trigger act); `None` when not linked.
    no_match_node: Option<Value>,
}

#[derive(Debug, Clone, Default)]
struct Talker {
    name_tag: String,
    echo: bool,
    live_data: i32,
    live_index: i32,
    groups: Vec<Value>,
    actor: Option<ObjRef>,
}

impl Talker {
    fn new(name_tag: &str, echo: bool, actor: Option<ObjRef>) -> Self {
        Talker {
            name_tag: name_tag.to_string(),
            echo,
            live_data: -1,
            live_index: 0,
            groups: Vec::new(),
            actor,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Tag {
    path: String,
    echo: bool,
    no_override_same: bool,
    group_event: bool,
    sound_effect: bool,
    once_per_session: bool,
    multiplayer_only: bool,
    priority: i32,
}

enum Player {
    None,
    Plain(Box<dyn FnMut(&Line) -> bool>),
    Timed(Box<dyn FnMut(&Line) -> f64>),
}

/// `std::mt19937` (the C++ dialog's generator, seeded 12345): a `ParityMode::Reference` stand-in.
#[derive(Clone)]
struct Mt19937 {
    mt: [u32; 624],
    index: usize,
}

impl Mt19937 {
    fn new(seed: u32) -> Self {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            let prev = mt[i - 1];
            // i < 624, so the index fits u32.
            mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        Mt19937 { mt, index: 624 }
    }

    fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            for i in 0..624 {
                let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % 624] & 0x7FFF_FFFF);
                let mut v = self.mt[(i + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    v ^= 0x9908_B0DF;
                }
                self.mt[i] = v;
            }
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^ (y >> 18)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Reading objects
// ---------------------------------------------------------------------------------------------------------------

/// The package and reference an object came from (C++ `resourcePackage` / `resourceIndex`).
fn source(vm: &Vm, o: ObjRef) -> Result<Option<(PackageId, i32)>, DialogError> {
    Ok(match vm.space.heap.get(o)?.origin {
        Origin::Export { package, index } | Origin::Resource { package, index } => {
            Some((package, index))
        }
        _ => None,
    })
}

/// The object of an object value that came from a package (C++ `v.o && v.o->resourcePackage`).
fn object(vm: &Vm, v: &Value) -> Result<Option<ObjRef>, DialogError> {
    match v {
        Value::Object(Some(o)) if source(vm, *o)?.is_some() => Ok(Some(*o)),
        _ => Ok(None),
    }
}

/// The export `o` is, or resolves to (C++ `resourcePackage` / `resourceIndex` after `resolveRef`): an import the
/// store has not loaded yet is resolved as the C++ resolves it (`CodeIndex::resolve_ref`). `None` for an object that
/// came from no package; an import that resolves to nothing stays the import (a reference that is not positive).
fn resolved(vm: &mut Vm, o: ObjRef) -> Result<Option<(PackageId, i32)>, DialogError> {
    let Some((package, index)) = source(vm, o)? else {
        return Ok(None);
    };
    if index > 0 {
        return Ok(Some((package, index)));
    }
    let (code, store) = vm.defs.code_mut();
    Ok(Some(
        code.resolve_ref(store, package, index)
            .unwrap_or((package, index)),
    ))
}

/// The object path of an object value (C++ `pathOf`): "" for anything that is not an object from a package.
fn path_of(vm: &mut Vm, v: &Value) -> Result<String, DialogError> {
    let Value::Object(Some(o)) = v else {
        return Ok(String::new());
    };
    match resolved(vm, *o)? {
        Some((package, index)) => Ok(vm.defs.packages().get(package).path_of(index)?),
        None => Ok(String::new()),
    }
}

/// [`path_of`] of an optional value.
fn path_opt(vm: &mut Vm, v: &Option<Value>) -> Result<String, DialogError> {
    match v {
        Some(v) => path_of(vm, v),
        None => Ok(String::new()),
    }
}

/// Property `name` of the loaded object `o` (element 0); `None` when its class declares none (C++ `property()`).
fn prop(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<Value>, DialogError> {
    let class = vm.space.heap.get(o)?.class;
    match vm.space.prop(class, name) {
        Some(p) => Ok(Some(vm.space.read(o, PropAddr::scalar(p))?.clone())),
        None => Ok(None),
    }
}

/// Field `name` of a struct value (C++ `Value::field`); `None` for a value that is not a struct or has no such field.
fn field(vm: &Vm, v: &Value, name: &str) -> Option<Value> {
    let s = v.as_struct()?;
    let layout = vm.space.classes.struct_layout(s.ty)?;
    let (first, _) = layout.field(vm.space.names.find(name)?)?;
    s.fields.get(usize::try_from(first).ok()?).cloned()
}

/// The elements of an optional array value (C++ `elements()`: none for anything else).
fn elements(v: &Option<Value>) -> Vec<Value> {
    v.as_ref()
        .and_then(Value::elements)
        .map(<[Value]>::to_vec)
        .unwrap_or_default()
}

fn flag(vm: &Vm, v: &Option<Value>) -> bool {
    v.as_ref().is_some_and(|v| v.truth(&vm.space.names))
}

fn number(v: &Option<Value>) -> f64 {
    match v {
        Some(Value::Float(f)) => f64::from(*f),
        Some(Value::Int(i)) => f64::from(*i),
        Some(Value::Byte(b)) => f64::from(*b),
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

fn integer(v: &Option<Value>) -> Option<i64> {
    match v {
        Some(Value::Int(i)) => Some(i64::from(*i)),
        Some(Value::Byte(b)) => Some(i64::from(*b)),
        Some(Value::Bool(b)) => Some(i64::from(*b)),
        _ => None,
    }
}

/// Whether `o`'s class or a super class is named `name` (short name; C++ `cls->name`). Bounded by the class count.
fn is_a(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, DialogError> {
    let mut class = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = class {
        let Some(layout) = vm.space.classes.get_layout(c) else {
            break;
        };
        if layout.path().name() == name {
            return Ok(true);
        }
        steps += 1;
        if steps > vm.space.classes.len() {
            break;
        }
        class = layout.super_class();
    }
    Ok(false)
}

fn class_path(vm: &Vm, o: ObjRef) -> Result<String, DialogError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.path().as_str().to_string())
        .unwrap_or_default())
}

// ---------------------------------------------------------------------------------------------------------------
// The dialog system
// ---------------------------------------------------------------------------------------------------------------

/// What [`DialogSystem::on_line`] is called with: the line and its outcome.
pub type LineSink = Box<dyn FnMut(&Line, &str)>;

/// The dialog components and manager of one mission package (module documentation).
pub struct DialogSystem {
    /// Called for every line chosen (a talk act with an audio event and a talker, or without a talker) with what
    /// became of it: "started", "not playing", "no talker", "no audio device", "blocked by priority" or
    /// "no audio event".
    pub on_line: Option<LineSink>,
    /// Called with a line's id when the dialog stops it while its audio still plays (the talker's next line, a group
    /// silence). Not called for a line whose audio had already ended.
    pub on_line_stopped: Option<Box<dyn FnMut(i32)>>,
    package: Option<PackageId>,
    player: Player,
    test_length: f64,
    now: f64,
    next_line: i32,
    data: Vec<EventData>,
    talkers: Vec<Talker>,
    /// Root group path -> effective priority index of its current event.
    group_index: BTreeMap<String, i32>,
    globals: Option<ObjRef>,
    globals_loaded: bool,
    not_run: Vec<String>,
    search: Vec<String>,
    /// `bOncePerSession` tags already played.
    played: BTreeSet<String>,
    priorities: Vec<String>,
    tracked_group: String,
    tracked_plot: bool,
    start_index: i32,
    side_floor: i32,
    plot_floor: i32,
    mt: Mt19937,
    /// The current event context's instigator: a registered talker (a pawn), -1 for the mission.
    instigator: i32,
    /// Set by `talk` when a line started during the current component trigger.
    line_started: bool,
    trigger_depth: u32,
}

impl fmt::Debug for DialogSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DialogSystem")
            .field("package", &self.package)
            .field("now", &self.now)
            .field("talkers", &self.talkers.len())
            .finish_non_exhaustive()
    }
}

impl DialogSystem {
    /// The dialog of the mission package `package` (`None`: no package, so no dialog globals).
    pub fn new(package: Option<PackageId>) -> Self {
        DialogSystem {
            on_line: None,
            on_line_stopped: None,
            package,
            player: Player::None,
            test_length: -1.0,
            now: 0.0,
            next_line: 1,
            data: Vec::new(),
            talkers: Vec::new(),
            group_index: BTreeMap::new(),
            globals: None,
            globals_loaded: false,
            not_run: Vec::new(),
            search: Vec::new(),
            played: BTreeSet::new(),
            priorities: Vec::new(),
            tracked_group: String::new(),
            tracked_plot: false,
            start_index: 0,
            side_floor: 0,
            plot_floor: 0,
            mt: Mt19937::new(12345),
            instigator: -1,
            line_started: false,
            trigger_depth: 0,
        }
    }

    /// A host that plays lines: `start` is told when a line starts and returns whether it plays; the host reports
    /// the end with [`DialogSystem::line_ended`].
    pub fn set_line_player(&mut self, start: impl FnMut(&Line) -> bool + 'static) {
        self.player = Player::Plain(Box::new(start));
        self.test_length = -1.0;
    }

    /// A player that knows each line's length: `start` returns it in seconds (negative: not played). The dialog's
    /// clock ends the audio at its exact time, so the finish time is that end plus the act's `OutputDelay` however
    /// long the update that reaches it is.
    pub fn set_timed_line_player(&mut self, start: impl FnMut(&Line) -> f64 + 'static) {
        self.player = Player::Timed(Box::new(start));
        self.test_length = -1.0;
    }

    /// A player for tests: every line starts and ends `seconds` later.
    pub fn set_test_line_length(&mut self, seconds: f64) {
        self.test_length = seconds;
        self.player = Player::None;
    }

    /// The behaviors applied to an empty context list, which did not run (NATIVE_BEHAVIOR_CONTEXT.md, UNVERIFIED),
    /// by name, in order.
    pub fn not_run(&self) -> &[String] {
        &self.not_run
    }

    /// The groups the searches of the last [`DialogSystem::trigger_on_component`] looked at, in order (None and
    /// skipped template groups are not listed).
    pub fn last_search(&self) -> &[String] {
        &self.search
    }

    /// The tracked mission's dialog group and whether it is plot-critical ("" = no tracked mission): the priority
    /// floor.
    pub fn set_tracked_mission(&mut self, group_path: &str, plot_critical: bool) {
        self.tracked_group = group_path.to_string();
        self.tracked_plot = plot_critical;
    }

    /// The dialog clock, in seconds.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// The object path of an object value ("" for anything else); C++ `DialogSystem::objectPath`.
    pub fn object_path(vm: &mut Vm, value: &Value) -> Result<String, DialogError> {
        path_of(vm, value)
    }

    /// The object `reference` names, loaded (C++ `load`): an import no holder has is looked up by its path among the
    /// dialog package's exports.
    fn load(&mut self, vm: &mut Vm, reference: &Value) -> Result<ObjRef, DialogError> {
        let Some(o) = object(vm, reference)? else {
            return Err(DialogError::Unresolved(None));
        };
        let loaded = vm.defs.ensure_loaded(&mut vm.space, o);
        vm.log.extend(vm.defs.take_log());
        loaded?;
        if matches!(vm.space.heap.get(o)?.origin, Origin::Export { .. }) {
            return Ok(o);
        }
        let path = path_of(vm, reference)?;
        if let Some(package) = self.package {
            let (code, store) = vm.defs.code_mut();
            let index = code.find_export(store, package, &path);
            if index > 0 {
                let found = vm.defs.resolve_ref(&mut vm.space, package, index);
                vm.log.extend(vm.defs.take_log());
                if let Some(found) = found? {
                    return Ok(found);
                }
            }
        }
        Err(DialogError::Unresolved(Some(path)))
    }

    /// The dialog globals: `Priorities` (a lower index is more important) and the tracked-mission floors. Without
    /// them every priority index is -1 (the manager's answer for "no globals").
    fn load_globals(&mut self, vm: &mut Vm) -> Result<(), DialogError> {
        if self.globals_loaded {
            return Ok(());
        }
        self.globals_loaded = true;
        let Some(package) = self.package else {
            return Ok(());
        };
        let (code, store) = vm.defs.code_mut();
        let index = code.find_export(store, package, GLOBALS_PATH);
        if index <= 0 {
            return Ok(());
        }
        let found = vm.defs.resolve_ref(&mut vm.space, package, index);
        vm.log.extend(vm.defs.take_log());
        let Some(globals) = found? else {
            return Ok(());
        };
        self.globals = Some(globals);
        for priority in elements(&prop(vm, globals, "Priorities")?) {
            self.priorities.push(path_of(vm, &priority)?);
        }
        let start = prop(vm, globals, "ActiveMissionMinPriorityStart")?;
        let start = path_opt(vm, &start)?;
        let side = prop(vm, globals, "ActiveSideMissionMinPriority")?;
        let side = path_opt(vm, &side)?;
        let plot = prop(vm, globals, "ActivePlotMissionMinPriority")?;
        let plot = path_opt(vm, &plot)?;
        self.start_index = self.index_of(vm, &start)?;
        self.side_floor = self.index_of(vm, &side)?;
        self.plot_floor = self.index_of(vm, &plot)?;
        Ok(())
    }

    fn index_of(&mut self, vm: &mut Vm, priority: &str) -> Result<i32, DialogError> {
        self.load_globals(vm)?;
        if self.priorities.is_empty() {
            return Ok(-1);
        }
        if priority.is_empty() {
            return Ok(i32::MAX);
        }
        Ok(self
            .priorities
            .iter()
            .position(|p| p == priority)
            .map_or(i32::MAX, |i| i32::try_from(i).unwrap_or(i32::MAX)))
    }

    fn tag_of(&mut self, vm: &mut Vm, reference: &Value) -> Result<Tag, DialogError> {
        let o = self.load(vm, reference)?;
        let priority = prop(vm, o, "Priority")?;
        let priority = path_opt(vm, &priority)?;
        let read = |vm: &Vm, name| prop(vm, o, name).map(|v| flag(vm, &v));
        Ok(Tag {
            path: path_of(vm, reference)?,
            priority: self.index_of(vm, &priority)?,
            echo: read(vm, "bIsEchoEvent")?,
            no_override_same: read(vm, "bDoesNotOverrideSamePriority")?,
            group_event: read(vm, "bGroupEvent")?,
            sound_effect: read(vm, "bSoundEffect")?,
            once_per_session: read(vm, "bOncePerSession")?,
            multiplayer_only: read(vm, "bMultiplayerOnly")?,
        })
    }

    /// A tag is invalid when it is once-per-session and was played, or multiplayer-only with fewer than two players
    /// (one here).
    fn tag_valid(&self, tag: &Tag) -> bool {
        if tag.once_per_session && self.played.contains(&tag.path) {
            return false;
        }
        !tag.multiplayer_only
    }

    /// The group state is keyed by the root group (the `ParentGroup` chain, at most 8 links).
    fn root_group_of(&mut self, vm: &mut Vm, group: &Value) -> Result<String, DialogError> {
        let mut current = group.clone();
        for _ in 0..ROOT_DEPTH {
            let o = self.load(vm, &current)?;
            match prop(vm, o, "ParentGroup")? {
                Some(parent) if object(vm, &parent)?.is_some() => current = parent,
                _ => break,
            }
        }
        path_of(vm, &current)
    }

    /// A pawn that can talk (its name tag); without one the talker is an echo caller. With `actor` the talker is that
    /// object: a behavior whose context object is it speaks as its instigator (a talker registered earlier under the
    /// same name tag without an actor takes the actor).
    pub fn register_talker(&mut self, name_tag: &str, actor: Option<ObjRef>) {
        for talker in &mut self.talkers {
            if talker.echo || talker.name_tag != name_tag {
                continue;
            }
            if actor.is_some() && talker.actor.is_none() {
                talker.actor = actor;
            }
            return;
        }
        self.talkers.push(Talker::new(name_tag, false, actor));
    }

    /// `WillowPawn.GetDialogGroups` and the lazily filled dialog name tag (NATIVE_DIALOG_GROUPS.md, UNVERIFIED),
    /// recomputed on every call from a pawn's body class: no body class gives an empty list and no name tag; else
    /// the body class's `DialogGroups`, then, only when its `bNPCDialog` is set, the name tag's
    /// `DlcExpansion.NPCDialogGroups` (if any) and the dialog globals' `NPCDialogGroups`, then (always, when the
    /// globals exist) the globals' `DefaultTemplateGroup` as one entry. No de-duplication; entries are kept as they
    /// are (None included). The name tag is `BodyClass.DialogName` ("" when none).
    pub fn pawn_dialog(
        &mut self,
        vm: &mut Vm,
        body_class: &Value,
    ) -> Result<PawnDialog, DialogError> {
        let mut result = PawnDialog::default();
        let Some(o) = object(vm, body_class)? else {
            return Ok(result);
        };
        // An import that resolves to no export (C++ `resourceIndex <= 0`): no body class.
        if resolved(vm, o)?.is_some_and(|(_, index)| index <= 0) {
            return Ok(result);
        }
        let body = self.load(vm, body_class)?;
        let name = prop(vm, body, "DialogName")?;
        result.name_tag = path_opt(vm, &name)?;
        result
            .groups
            .extend(elements(&prop(vm, body, "DialogGroups")?));
        self.load_globals(vm)?;
        if flag(vm, &prop(vm, body, "bNPCDialog")?) {
            if let Some(name) = &name
                && object(vm, name)?.is_some()
            {
                let tag = self.load(vm, name)?;
                if let Some(expansion) = prop(vm, tag, "DlcExpansion")?
                    && object(vm, &expansion)?.is_some()
                {
                    let dlc = self.load(vm, &expansion)?;
                    result
                        .groups
                        .extend(elements(&prop(vm, dlc, "NPCDialogGroups")?));
                }
            }
            if let Some(globals) = self.globals {
                result
                    .groups
                    .extend(elements(&prop(vm, globals, "NPCDialogGroups")?));
            }
        }
        if let Some(globals) = self.globals {
            // One entry, even when the field is None.
            let default = prop(vm, globals, "DefaultTemplateGroup")?.unwrap_or(Value::None);
            result.groups.push(default);
        }
        Ok(result)
    }

    /// GetMatchingEvent (no group given; NATIVE_DIALOG_GROUPS.md, UNVERIFIED): the talker's groups in order; the first
    /// with an enabled entry for the tag wins; a group without one appends its `ParentGroup` to the end of the search
    /// unless it is already in it. Template groups (`GearboxDialogTemplateGroup`) are skipped, adding no parent,
    /// unless `allow_templates`. `None` when no group has one.
    fn matching_group(
        &mut self,
        vm: &mut Vm,
        groups: &[Value],
        tag: &Tag,
        allow_templates: bool,
    ) -> Result<Option<Value>, DialogError> {
        let mut queue: Vec<Value> = groups.to_vec();
        let mut seen: BTreeSet<Option<ObjRef>> = BTreeSet::new();
        for group in &queue {
            seen.insert(object(vm, group)?);
        }
        let mut i = 0;
        while i < queue.len() {
            let entry = queue[i].clone();
            i += 1;
            if object(vm, &entry)?.is_none() {
                continue;
            }
            let group = self.load(vm, &entry)?;
            if is_a(vm, group, "GearboxDialogTemplateGroup")? && !allow_templates {
                continue;
            }
            self.search.push(path_of(vm, &entry)?);
            for event in elements(&prop(vm, group, "DialogEvents")?) {
                let event_tag = field(vm, &event, "Tag");
                if path_opt(vm, &event_tag)? == tag.path && flag(vm, &field(vm, &event, "bEnabled"))
                {
                    return Ok(Some(entry));
                }
            }
            if let Some(parent) = prop(vm, group, "ParentGroup")?
                && let Some(p) = object(vm, &parent)?
                && seen.insert(Some(p))
            {
                queue.push(parent);
            }
        }
        Ok(None)
    }

    /// A pawn's dialog component (NATIVE_DIALOG.md "Component TriggerEvent / GetMatchingEvent", UNVERIFIED): the
    /// speaker is registered as a talker with its own dialog groups, and `tag` is triggered on it with the speaker as
    /// the event's instigator. The first group with an enabled event for the tag runs it (template groups are searched
    /// here, where no event data is reused); a Talk act with no entry for the speaker and `bEnableNoMatch` takes its
    /// output 1 (a Trigger act, which fires its DialogEvent on the instigator's own groups with the same event data).
    /// Returns whether a line started.
    pub fn trigger_on_component(
        &mut self,
        vm: &mut Vm,
        speaker_name_tag: &str,
        groups: &[Value],
        tag: &Value,
    ) -> Result<bool, DialogError> {
        let tag = self.tag_of(vm, tag)?;
        if !self.tag_valid(&tag) {
            return Ok(false);
        }
        self.register_talker(speaker_name_tag, None);
        let speaker = self
            .talkers
            .iter()
            .rposition(|t| !t.echo && t.name_tag == speaker_name_tag)
            .ok_or_else(|| DialogError::Malformed("the speaker was not registered".to_string()))?;
        self.talkers[speaker].groups = groups.to_vec();
        self.search.clear();
        // A fresh trigger (no event data reused): template groups are searched.
        let Some(group) = self.matching_group(vm, groups, &tag, true)? else {
            return Ok(false);
        };
        let saved = self.instigator;
        // TriggerEvent: Instigator := the component's owner.
        self.instigator = i32::try_from(speaker).unwrap_or(-1);
        self.line_started = false;
        let ran = self.run(vm, Handle::default(), &group, &tag);
        self.instigator = saved;
        ran?;
        Ok(self.line_started)
    }

    /// The talker for a name tag: the first registered pawn with exactly that tag; failing that (echo events only)
    /// the echo caller for it, created when absent. -1 when there is none.
    fn resolve_talker(&mut self, name_tag: &str, echo: bool) -> i32 {
        let index = |i: usize| i32::try_from(i).unwrap_or(-1);
        if let Some(i) = self
            .talkers
            .iter()
            .position(|t| !t.echo && t.name_tag == name_tag)
        {
            return index(i);
        }
        if !echo {
            return -1;
        }
        if let Some(i) = self
            .talkers
            .iter()
            .position(|t| t.echo && t.name_tag == name_tag)
        {
            return index(i);
        }
        self.talkers.push(Talker::new(name_tag, true, None));
        index(self.talkers.len() - 1)
    }

    fn stop_talking(&mut self, index: usize) {
        let (live, playing, line, talker, root) = {
            let d = &self.data[index];
            (d.live, d.playing, d.line_id, d.talker, d.root_group.clone())
        };
        if live
            && playing
            && let Some(stopped) = &mut self.on_line_stopped
        {
            stopped(line);
        }
        if let Some(t) = usize::try_from(talker)
            .ok()
            .and_then(|t| self.talkers.get_mut(t))
            && usize::try_from(t.live_data).ok() == Some(index)
        {
            t.live_data = -1;
        }
        self.group_index.remove(&root);
        let d = &mut self.data[index];
        d.live = false;
        d.playing = false;
        d.talker = -1;
    }

    /// The host says line `line_id`'s audio ended.
    pub fn line_ended(&mut self, line_id: i32) {
        let now = self.now;
        for d in &mut self.data {
            if d.live && d.playing && d.line_id == line_id {
                d.playing = false;
                d.finish_time = now + d.output_delay;
            }
        }
    }

    /// The dialog components' per-frame update (NATIVE_DIALOG.md "Line end"): a finished audio sets the finish time
    /// (the act's `OutputDelay` later); a live act with no playing audio and a reached finish time ends. A live act
    /// that never had audio ends on the next update.
    pub fn tick(&mut self, seconds: f64) {
        self.now += seconds;
        for i in 0..self.data.len() {
            let (live, playing, test_end, line) = {
                let d = &self.data[i];
                (d.live, d.playing, d.test_end, d.line_id)
            };
            if live && playing && test_end >= 0.0 && self.now >= test_end {
                self.line_ended(line);
            }
            let now = self.now;
            let d = &mut self.data[i];
            if d.live && d.playing && d.audio_end >= 0.0 && now >= d.audio_end {
                // The timed player's line: finish from the real end.
                d.playing = false;
                d.finish_time = d.audio_end + d.output_delay;
            }
            if d.live && !d.playing && now >= d.finish_time {
                self.stop_talking(i);
            }
        }
    }

    /// [`DialogSystem::tick`] up to the dialog time `time` (nothing when it is not later than now).
    pub fn advance_to(&mut self, time: f64) {
        if time > self.now {
            self.tick(time - self.now);
        }
    }

    fn talk_entries(vm: &mut Vm, list: &Option<Value>) -> Result<Vec<TalkEntry>, DialogError> {
        let mut entries = Vec::new();
        for e in elements(list) {
            let name_tag = field(vm, &e, "NameTag");
            let ak_event = field(vm, &e, "TalkAkEvent");
            entries.push(TalkEntry {
                name_tag: path_opt(vm, &name_tag)?,
                ak_event: path_opt(vm, &ak_event)?,
            });
        }
        Ok(entries)
    }

    /// An act from a talk node object. Only plain talk acts are supported.
    fn act_of(vm: &mut Vm, node: ObjRef) -> Result<Act, DialogError> {
        if !is_a(vm, node, "GearboxDialogAct_Talk")? {
            return Err(not_implemented(format!(
                "dialog node class {}",
                class_path(vm, node)?
            )));
        }
        let mut act = Act::default();
        let no_match = flag(vm, &prop(vm, node, "bEnableNoMatch")?);
        // Output 1 is the no-match output (no talker and bEnableNoMatch); another output with a link is not modelled.
        for (i, link) in elements(&prop(vm, node, "OutputLinks")?).iter().enumerate() {
            let targets = elements(&field(vm, link, "Links"));
            if let Some(first) = targets.first() {
                if i != 1 || !no_match {
                    return Err(not_implemented("an output link on a talk act"));
                }
                act.no_match_node = Some(first.clone());
            }
        }
        act.no_match = no_match;
        if let Some(variable) = prop(vm, node, "TalkerVariable")?
            && matches!(variable, Value::Object(Some(_)))
        {
            return Err(not_implemented("a talker variable"));
        }
        act.path = match source(vm, node)? {
            Some((package, index)) => vm.defs.packages().get(package).path_of(index)?,
            None => vm
                .space
                .names
                .text(vm.space.heap.get(node)?.name)
                .into_owned(),
        };
        if let Some(delay) = prop(vm, node, "OutputDelay")? {
            act.output_delay = number(&Some(delay));
        }
        act.instigator_talker = flag(vm, &prop(vm, node, "bInstigatorTalker")?);
        let talk = prop(vm, node, "TalkData")?;
        act.talk = Self::talk_entries(vm, &talk)?;
        Ok(act)
    }

    /// Act_ObjectParameterSwitch.Activate (NATIVE_DIALOG.md, UNVERIFIED): output i for every `Outputs` entry equal to
    /// the event data's ObjectParameter (None on every route here) and, with none, the last output. Only an output
    /// without a link is modelled.
    fn object_parameter_switch(vm: &Vm, node: ObjRef) -> Result<(), DialogError> {
        let outputs = elements(&prop(vm, node, "Outputs")?);
        let links = elements(&prop(vm, node, "OutputLinks")?);
        let mut chosen: Vec<usize> = outputs
            .iter()
            .enumerate()
            .filter(|(_, o)| !matches!(o, Value::Object(Some(_))))
            .map(|(i, _)| i)
            .collect();
        if chosen.is_empty() && !links.is_empty() {
            chosen.push(links.len() - 1);
        }
        for i in chosen {
            if let Some(link) = links.get(i)
                && !elements(&field(vm, link, "Links")).is_empty()
            {
                return Err(not_implemented(
                    "an output link on an object parameter switch",
                ));
            }
        }
        Ok(())
    }

    /// FindEvent: the last enabled `DialogEvents` entry for the tag; its act is the inline `OutputAction` or, with
    /// none, the target of the group's link table for (the entry's 1-based id, link 0): a `TalkActs` template (ids
    /// after the events) or a node by `NodeID`. `None`: no enabled entry; `Some(None)`: an event with no act.
    fn find_act(
        &mut self,
        vm: &mut Vm,
        group_ref: &Value,
        tag: &Tag,
    ) -> Result<Option<Option<Act>>, DialogError> {
        let group = self.load(vm, group_ref)?;
        let events = elements(&prop(vm, group, "DialogEvents")?);
        let mut found = 0usize;
        for (i, entry) in events.iter().enumerate() {
            let entry_tag = field(vm, entry, "Tag");
            if path_opt(vm, &entry_tag)? == tag.path && flag(vm, &field(vm, entry, "bEnabled")) {
                found = i + 1;
            }
        }
        if found == 0 {
            return Ok(None);
        }
        let entry = &events[found - 1];
        if let Some(action) = field(vm, entry, "OutputAction")
            && matches!(action, Value::Object(Some(_)))
        {
            let node = self.load(vm, &action)?;
            if is_a(vm, node, "GearboxDialogAct_ObjectParameterSwitch")? {
                Self::object_parameter_switch(vm, node)?;
                return Ok(Some(None));
            }
            return Ok(Some(Some(Self::act_of(vm, node)?)));
        }
        let Some(links) = prop(vm, group, "OutputLinksToStructs")? else {
            return Ok(Some(None));
        };
        let found = i64::try_from(found).unwrap_or(i64::MAX);
        let event_count = i64::try_from(events.len()).unwrap_or(i64::MAX);
        for link in elements(&Some(links)) {
            let Some(from) = field(vm, &link, "FromNodeID") else {
                continue;
            };
            // As the C++ reads it: LinkNumber only once FromNodeID matched.
            if integer(&Some(from)) != Some(found) {
                continue;
            }
            let link_number = integer(&field(vm, &link, "LinkNumber"))
                .ok_or_else(|| DialogError::Malformed("a link without LinkNumber".to_string()))?;
            if link_number != 0 {
                continue;
            }
            let to = integer(&field(vm, &link, "ToNodeID"))
                .ok_or_else(|| DialogError::Malformed("a link without ToNodeID".to_string()))?;
            let templates = elements(&prop(vm, group, "TalkActs")?);
            let slot = to - event_count;
            if slot > 0
                && let Some(template) = usize::try_from(slot - 1)
                    .ok()
                    .and_then(|s| templates.get(s))
                    .cloned()
            {
                let template = &template;
                if matches!(
                    field(vm, template, "TalkerVariable"),
                    Some(Value::Object(Some(_)))
                ) {
                    return Err(not_implemented("a talker variable"));
                }
                if matches!(
                    field(vm, template, "OutputAction"),
                    Some(Value::Object(Some(_)))
                ) {
                    return Err(not_implemented("an output action on a template talk act"));
                }
                let output_delay = number(&field(vm, template, "OutputDelay"));
                let instigator_talker = flag(vm, &field(vm, template, "bInstigatorTalker"));
                let talk = field(vm, template, "TalkData");
                let act = Act {
                    path: format!("{}.TalkActs[{}]", path_of(vm, group_ref)?, slot - 1),
                    output_delay,
                    instigator_talker,
                    talk: Self::talk_entries(vm, &talk)?,
                    ..Act::default()
                };
                return Ok(Some(Some(act)));
            }
            for node in elements(&prop(vm, group, "Nodes")?) {
                let o = self.load(vm, &node)?;
                if integer(&prop(vm, o, "NodeID")?) == Some(to) {
                    return Ok(Some(Some(Self::act_of(vm, o)?)));
                }
            }
            return Err(not_implemented(format!("a link to node {to}")));
        }
        // An event with nothing linked: the chain ends.
        Ok(Some(None))
    }

    fn trigger(&mut self, vm: &mut Vm, group: &Value, tag: &Value) -> Result<Handle, DialogError> {
        let tag = self.tag_of(vm, tag)?;
        if !self.tag_valid(&tag) {
            return Ok(Handle::default());
        }
        if !matches!(group, Value::Object(Some(_))) {
            return Err(not_implemented(
                "a dialog event without a group (talker-owned events)",
            ));
        }
        self.run(vm, Handle::default(), group, &tag)
    }

    /// TriggerGroupEvent from the found event on: the event data (`reuse` keeps its identity, else a pooled one),
    /// then the act chain.
    fn run(
        &mut self,
        vm: &mut Vm,
        reuse: Handle,
        group: &Value,
        tag: &Tag,
    ) -> Result<Handle, DialogError> {
        if tag.sound_effect {
            return Err(not_implemented("sound-effect dialog events"));
        }
        let Some(act) = self.find_act(vm, group, tag)? else {
            return Ok(reuse);
        };
        let mut handle = reuse;
        if handle.index < 0 {
            // The first pooled event data that is not live, reused (its use count rises), or a new one.
            let slot = self
                .data
                .iter()
                .position(|d| !d.live)
                .unwrap_or(self.data.len());
            if slot == self.data.len() {
                self.data.push(EventData::new(0));
            }
            let use_count = self.data[slot].use_count.wrapping_add(1);
            self.data[slot] = EventData::new(use_count);
            handle = Handle {
                index: i32::try_from(slot).map_err(|_| {
                    DialogError::Malformed("the event data pool is full".to_string())
                })?,
                use_count,
            };
        }
        let Some(act) = act else {
            return Ok(handle);
        };
        let group_path = path_of(vm, group)?;
        let root = self.root_group_of(vm, group)?;
        self.talk(vm, handle, &act, tag, &group_path, &root)?;
        Ok(handle)
    }

    /// Output 1 of a talk act (no talker, `bEnableNoMatch`): the linked node. Only a Trigger act is modelled.
    fn follow_no_match(
        &mut self,
        vm: &mut Vm,
        handle: Handle,
        act: &Act,
    ) -> Result<(), DialogError> {
        let Some(node) = act
            .no_match_node
            .as_ref()
            .filter(|n| matches!(n, Value::Object(Some(_))))
        else {
            return Ok(());
        };
        let node = self.load(vm, node)?;
        if !is_a(vm, node, "WillowDialogAct_Trigger")? {
            return Err(not_implemented(format!(
                "a no-match output into {}",
                class_path(vm, node)?
            )));
        }
        if self.trigger_depth >= MAX_TRIGGER_DEPTH {
            return Err(DialogError::Malformed(format!(
                "a chain of more than {MAX_TRIGGER_DEPTH} trigger acts"
            )));
        }
        self.trigger_depth += 1;
        let ran = self.run_trigger(vm, handle, node);
        self.trigger_depth -= 1;
        ran
    }

    /// Act_Trigger.Activate (NATIVE_DIALOG.md): the talker variable's talkers (the event's Instigator variable here)
    /// that can talk the DialogEvent; the first is chosen and the DialogEvent is triggered on it with the same event
    /// data. The act's own output 0 has no link in the stock data and is not followed.
    fn run_trigger(
        &mut self,
        vm: &mut Vm,
        handle: Handle,
        node: ObjRef,
    ) -> Result<(), DialogError> {
        let Some(event) =
            prop(vm, node, "DialogEvent")?.filter(|e| matches!(e, Value::Object(Some(_))))
        else {
            return Ok(());
        };
        for link in elements(&prop(vm, node, "OutputLinks")?) {
            if !elements(&field(vm, &link, "Links")).is_empty() {
                return Err(not_implemented("an output link on a trigger act"));
            }
        }
        let mut talkers = Vec::new();
        for link in elements(&prop(vm, node, "VariableLinks")?) {
            for target in elements(&field(vm, &link, "Links")) {
                if !matches!(target, Value::Object(Some(_))) {
                    continue;
                }
                let variable = self.load(vm, &target)?;
                if !is_a(vm, variable, "GearboxDialogVar_Instigator")? {
                    return Err(not_implemented(format!(
                        "a dialog variable of class {}",
                        class_path(vm, variable)?
                    )));
                }
                if self.instigator >= 0 {
                    talkers.push(self.instigator);
                }
            }
        }
        let tag = self.tag_of(vm, &event)?;
        if !self.tag_valid(&tag) {
            return Ok(());
        }
        for talker in talkers {
            let groups = usize::try_from(talker)
                .ok()
                .and_then(|t| self.talkers.get(t))
                .map(|t| t.groups.clone())
                .unwrap_or_default();
            // Reused event data: no template groups.
            let Some(group) = self.matching_group(vm, &groups, &tag, false)? else {
                continue;
            };
            self.run(vm, handle, &group, &tag)?;
            return Ok(());
        }
        Ok(())
    }

    fn report(&mut self, line: &Line, outcome: &str) {
        if let Some(on_line) = &mut self.on_line {
            on_line(line, outcome);
        }
    }

    /// The random `TalkData` pick (module documentation).
    fn pick(&mut self, vm: &mut Vm, count: usize) -> usize {
        let draw = match vm.parity {
            ParityMode::Reference => self.mt.next_u32() as usize,
            ParityMode::Off => usize::from(vm.rng.crt(CrtStream::Dialog).rand15()),
        };
        draw % count
    }

    /// The talk act's Activate and GearboxDialogComponent.Talk: a pass-through for an act without audio; the talker;
    /// the gates (audio device, priority); then the live state, the group silence, the interruption of the talker's
    /// own line and the start of the audio.
    fn talk(
        &mut self,
        vm: &mut Vm,
        handle: Handle,
        act: &Act,
        tag: &Tag,
        group_path: &str,
        root: &str,
    ) -> Result<(), DialogError> {
        if !act.talk.iter().any(|e| !e.ak_event.is_empty()) {
            return Ok(()); // no audio event: output 0 at once
        }
        let mut line = Line {
            id: self.next_line,
            event_tag: tag.path.clone(),
            group: group_path.to_string(),
            talk_act: act.path.clone(),
            output_delay: act.output_delay,
            ..Line::default()
        };
        self.next_line = self.next_line.wrapping_add(1);
        // The talker: the event's instigator when the act says so and it has an entry for its own name tag (exact
        // tag; the ParentTag ancestors are not modelled); otherwise a random TalkData entry by its name tag. With no
        // talker and bEnableNoMatch the act takes its output 1. For an act that names the instigator, the random
        // entry when the context is the mission is a fallback read from the live game, UNVERIFIED as a native rule
        // (FIRE_MISSION_GOLDEN_TRACE.md, line 03b).
        let instigator = usize::try_from(self.instigator)
            .ok()
            .filter(|&i| i < self.talkers.len());
        let mut talker = -1;
        let chosen: TalkEntry;
        if act.instigator_talker
            && let Some(i) = instigator
        {
            let own = self.talkers[i].name_tag.clone();
            match act.talk.iter().find(|c| c.name_tag == own) {
                Some(entry) => {
                    chosen = entry.clone();
                    talker = self.instigator;
                }
                None if act.no_match => return self.follow_no_match(vm, handle, act),
                None => chosen = act.talk[0].clone(),
            }
        } else {
            let at = self.pick(vm, act.talk.len());
            chosen = act.talk[at].clone();
            talker = self.resolve_talker(&chosen.name_tag, tag.echo);
        }
        line.ak_event = chosen.ak_event.clone();
        line.talker = chosen.name_tag.clone();
        let Some(t) = usize::try_from(talker).ok() else {
            self.report(&line, "no talker");
            return Ok(());
        };
        line.echo = self.talkers[t].echo;
        if matches!(self.player, Player::None) && self.test_length < 0.0 {
            // Talk does nothing without an audio device.
            self.report(&line, "no audio device");
            return Ok(());
        }
        // Priority arbitration: a lower index is more important; the tracked mission's group is floored.
        let mut index = tag.priority;
        if !self.tracked_group.is_empty()
            && group_path == self.tracked_group
            && index > self.start_index
        {
            index = index.min(if self.tracked_plot {
                self.plot_floor
            } else {
                self.side_floor
            });
        }
        let may_override = tag.echo && !tag.no_override_same;
        let blocks = |existing: i32| {
            if may_override {
                existing < index
            } else {
                existing <= index
            }
        };
        let group_blocks = self.group_index.get(root).is_some_and(|&e| blocks(e));
        let speaker_blocks = {
            let speaker = &self.talkers[t];
            speaker.live_data >= 0 && blocks(speaker.live_index)
        };
        if group_blocks || speaker_blocks {
            self.report(&line, "blocked by priority");
            return Ok(());
        }
        let slot = usize::try_from(handle.index)
            .ok()
            .filter(|&s| s < self.data.len())
            .ok_or_else(|| DialogError::Malformed("a talk act without event data".to_string()))?;
        {
            let d = &mut self.data[slot];
            d.live = true;
            d.talker = talker;
            d.root_group = root.to_string();
            d.output_delay = act.output_delay;
            d.finish_time = 0.0;
            d.line_id = line.id;
        }
        if tag.group_event || tag.echo {
            // The group is silenced, then this event is the group's current one.
            for i in 0..self.data.len() {
                if i != slot && self.data[i].live && self.data[i].root_group == root {
                    self.stop_talking(i);
                }
            }
            self.group_index.insert(root.to_string(), index);
        }
        let previous = self.talkers[t].live_data;
        if previous >= 0 && previous != handle.index {
            // Interrupts its own line.
            if let Ok(p) = usize::try_from(previous)
                && p < self.data.len()
            {
                self.stop_talking(p);
            }
        }
        self.talkers[t].live_data = handle.index;
        self.talkers[t].live_index = index;
        if chosen.ak_event.is_empty() {
            // Live until the next update ends it.
            self.report(&line, "no audio event");
            return Ok(());
        }
        let now = self.now;
        let (playing, audio_end) = match &mut self.player {
            Player::Timed(start) => {
                let length = start(&line);
                // A negative, NaN or infinite length (or one whose end overflows the clock) is not played.
                let ok = length.is_finite() && length >= 0.0 && (now + length).is_finite();
                (ok, if ok { now + length } else { -1.0 })
            }
            Player::Plain(start) => (start(&line), -1.0),
            Player::None => (true, -1.0),
        };
        {
            let d = &mut self.data[slot];
            d.playing = playing;
            d.audio_end = audio_end;
            if self.test_length >= 0.0 {
                d.test_end = now + self.test_length;
            }
        }
        if tag.once_per_session {
            self.played.insert(tag.path.clone());
        }
        self.line_started = true;
        self.report(&line, if playing { "started" } else { "not playing" });
        Ok(())
    }

    fn active(&self, handle: Handle) -> bool {
        usize::try_from(handle.index)
            .ok()
            .and_then(|i| self.data.get(i))
            .is_some_and(|d| d.use_count == handle.use_count && d.live)
    }

    /// `Behavior_TriggerDialogEvent.ApplyBehaviorToContext` (NATIVE_DIALOG.md; module documentation): the output
    /// ids behavior `id` of `provider` selects in this run, in order (0 Out, 1 Finished).
    pub fn behavior(
        &mut self,
        vm: &mut Vm,
        provider: &mut Provider,
        id: BehaviorId,
    ) -> Result<Vec<i32>, DialogError> {
        let behavior = provider.behavior(id).object;
        let tag = prop(vm, behavior, "EventTag")?;
        let group = prop(vm, behavior, "Group")?;
        let immediate = flag(vm, &prop(vm, behavior, "bForcePlayImmediate")?);
        let initial = provider.run_info().initial_run;
        let mut ids = Vec::new();
        if initial {
            // The objects the kernel resolves for the Context link (with none, the mission: one null entry). An empty
            // list means the behavior does not run (NATIVE_BEHAVIOR_CONTEXT.md, UNVERIFIED).
            let contexts = context_objects(provider, id);
            let Some(first) = contexts.first() else {
                self.not_run.push(provider.behavior(id).name.clone());
                return Ok(ids);
            };
            let mut fresh = DialogBehaviorState::default();
            if let Some(first) = first {
                for (i, t) in self.talkers.iter().enumerate() {
                    if t.actor == Some(*first) {
                        fresh.instigator = i32::try_from(i).unwrap_or(-1);
                    }
                }
            }
            set_state(provider, Some(fresh));
        }
        let mut state = state_of(provider)?.unwrap_or_default();
        match &tag {
            Some(tag @ Value::Object(Some(_))) => {
                if initial && !immediate {
                    provider.run_info().wait = FIRST_WAKE; // latent for one kernel wake; Out is selected below
                } else {
                    if state.handle.index < 0 || immediate {
                        let saved = self.instigator;
                        // TriggerGroupEvent: Instigator := the behavior's context object.
                        self.instigator = state.instigator;
                        let triggered =
                            self.trigger(vm, &group.clone().unwrap_or(Value::None), tag);
                        self.instigator = saved;
                        state.handle = triggered?;
                        set_state(provider, Some(state));
                    }
                    if state.handle.index < 0
                        || !self.active(state.handle)
                        || immediate
                        || !provider.run_info().has_linked_outputs
                    {
                        ids.push(1); // Finished
                    } else {
                        provider.run_info().wait = POLL; // poll while the line is live
                    }
                    if immediate {
                        set_state(provider, None);
                    }
                }
            }
            _ => ids.push(1),
        }
        if initial {
            ids.push(0); // Out, at the end of the first run
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests;
