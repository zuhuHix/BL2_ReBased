//! Owner: lane W5-KISMET. The Kismet (UE3 visual script) executor: sequence ops, the impulse stack and the pure-logic
//! ops (port of `src/kismet.*`).
//!
//! **What is data and what is native.** The sequence graph is installed data: every `SequenceOp` of a sequence is
//! instantiated from its package export ([`ow_object::cdo::instantiate_export`], 4-byte prefix), so links, link
//! descriptions, variables and property values come from the package, not from this file. What this file supplies is
//! the native part of op execution: impulse propagation, delays, and a handful of pure-logic ops (`SequenceEvent`
//! entered through an input, `SeqAct_ActivateRemoteEvent`, `SeqAct_Gate`, `SeqAct_SetBool`, `SeqCond_CompareBool`,
//! `SeqAct_Delay`). Ops that act on the world are host bindings ([`Kismet::handle`]); an op whose class chain has no
//! handler is reported in [`Kismet::errors`] and nothing downstream of it runs. Silent success is never produced for
//! an unhandled op.
//!
//! **Objects.** References decoded inside an op are the decoder's stand-ins ([`Origin::Resource`], the C++
//! `Runtime::resource`): a link lands on an op of this sequence when its stand-in names an export of this sequence's
//! package that is one of its ops. Variables (`SeqVar_*`) linked from `VariableLinks` are instantiated on first use and
//! kept for the life of the executor, one object per export.
//!
//! **Scheduling** follows `docs/verification/NATIVE_MISSION_DISPATCH.md` A4 (read from native code; UNVERIFIED in the
//! game): queued ops wait on a stack (the most recently queued runs first); the links of one output are counted in
//! link order and their ops pushed last to first, so the first link's target runs next (depth-first in link order);
//! an op already queued keeps its place; a link's delay is the target input's `ActivateDelay` plus the output's; a
//! disabled input receives nothing; an input hit twice runs its op twice; an entered event fires all of its outputs;
//! at most [`EXECUTION_LIMIT`] ops run per [`Kismet::run`] call (one frame). Not modelled, as in the C++ (A4 records each):
//! an op handles one input per run (the game sees all inputs with an impulse at once); event `MaxTriggerCount` /
//! `ReTriggerDelay` checks; latent ops other than `Delay`; the native and script `Activated` / `Deactivated` events
//! and `UpdateOp`; `bSupportsMultipleActivations`; an event that is already active queuing its activation in its
//! parent sequence; the player and network activation checks; and an op that is still active and not latent not
//! propagating its outputs.
//!
//! **The pure-logic ops** follow the public UE3 sequence model, as the C++ does; they are UNVERIFIED against the
//! original game until a paired trace exists (`docs/verification/SANCTUARY_RPG_KISMET.md`).
//!
//! **Differences from the C++.**
//! - A property the C++ dereferences without a check (`bOpen`, `AutoCloseCount`, `CurrentCloseCount`, `bResult`,
//!   `DefaultValue`, `Duration`) crashes the C++ when the op's class does not declare it. Here that is an error line
//!   (`<op>: <Class> without <Property>`) and the op does nothing more. The installed classes declare them all.
//! - The C++ bag stores whatever kind a write gives; a slot keeps its declared kind. A number written here (a count,
//!   a bool variable's value) is written in the slot's own kind, so a later read gives the number the C++ reads.
//! - A float read as an integer (the C++ `Value::integer()`) that is NaN or outside the 64-bit range is undefined in
//!   the C++; here it is an error line naming the op.
//! - A count that would pass `i32::MAX` (`TriggerCount`, `ActivateCount`, `CurrentCloseCount`) wraps in the C++; here
//!   it is an error line (the count stays).
//! - Names compare by text, as the C++ compares strings. The name table keeps the first spelling of a name it met
//!   (`ow_object::names`), so a package that spells one name two ways in different cases compares the first spelling.
//!
//! Status: parity with the C++ executor on invented packages (`tests/kismet_test.py`). Nothing here is confirmed in
//! the running game (UNVERIFIED).

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::rc::Rc;

use ow_object::cdo::instantiate_export;
use ow_object::decode::{DecodeError, ValueCx};
use ow_object::heap::{HeapError, ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::{ClassKey, ReflectError};
use ow_object::value::Value;
use ow_package::PackageId;
use ow_package::package::PackageError;

#[cfg(test)]
mod tests;

/// Ops run per [`Kismet::run`] call (one frame) before the run stops with "kismet execution limit exceeded" (the C++
/// `executionLimit`).
pub const EXECUTION_LIMIT: usize = 1000;

/// The input index of an entered event: when it runs, it fires all of its outputs (the C++ `kEventInput`).
pub const EVENT_INPUT: i32 = -1;

/// The prefix an op's and a variable's tags start after (the C++ `instantiateExport(package, index, 4)`).
const OP_PREFIX: usize = 4;

/// Why the executor could not go on. Data problems the C++ reports as lines (an unsupported op, an unresolved link)
/// are [`Kismet::errors`] entries, not these; these are what the C++ throws.
#[derive(Debug)]
#[non_exhaustive]
pub enum KismetError {
    /// No export of the package at the sequence path (the C++ "kismet sequence not found: <path>").
    SequenceNotFound(String),
    /// A class could not be found or loaded (`Engine.SequenceOp` and the event classes are looked up by path).
    Reflect(ReflectError),
    /// An op or a variable could not be instantiated.
    Decode(DecodeError),
    /// A path could not be formed from the package tables.
    Package(PackageError),
    /// A heap read or write was refused.
    Heap(HeapError),
}

impl fmt::Display for KismetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KismetError::SequenceNotFound(path) => write!(f, "kismet sequence not found: {path}"),
            // Transparent wrappers: the wrapped error's text is this one's (the C++ rethrows the same exception).
            KismetError::Reflect(e) => e.fmt(f),
            KismetError::Decode(e) => e.fmt(f),
            KismetError::Package(e) => e.fmt(f),
            KismetError::Heap(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for KismetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // The wrapped error's text is this one's, so its cause is this one's cause.
            KismetError::SequenceNotFound(_) => None,
            KismetError::Reflect(e) => e.source(),
            KismetError::Decode(e) => e.source(),
            KismetError::Package(e) => e.source(),
            KismetError::Heap(e) => e.source(),
        }
    }
}

impl From<ReflectError> for KismetError {
    fn from(e: ReflectError) -> Self {
        KismetError::Reflect(e)
    }
}

impl From<DecodeError> for KismetError {
    fn from(e: DecodeError) -> Self {
        KismetError::Decode(e)
    }
}

impl From<PackageError> for KismetError {
    fn from(e: PackageError) -> Self {
        KismetError::Package(e)
    }
}

impl From<HeapError> for KismetError {
    fn from(e: HeapError) -> Self {
        KismetError::Heap(e)
    }
}

/// One op of the sequence: its instantiated object, its export index in the sequence's package, its class, and its
/// class path (`Package.Class`), name and path as the package spells them.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Op {
    pub object: ObjRef,
    pub index: i32,
    pub class: ClassKey,
    pub cls: String,
    pub name: String,
    pub path: String,
}

/// A host binding or a logic op: called with the executor, the decode context, the op's position in
/// [`Kismet::ops`] and the activated input's index ([`EVENT_INPUT`] never reaches a handler). Handlers read the
/// input's `LinkDesc` through [`Kismet::input_desc`] and fire outputs through [`Kismet::fire`].
///
/// **Not frozen for world hosting.** The handler gets a decode context (`ValueCx`), as the C++ executor works over the
/// decode `Runtime`; a handler of a world-hosted executor needs the VM (`&mut Vm`), which this type cannot reach. The
/// shape changes when the first world host (W5-DIALOG's mover, the host world) binds handlers (decision 2026-11-15-31).
pub type Handler = Rc<dyn Fn(&mut Kismet, &mut ValueCx<'_>, usize, i32) -> Result<(), KismetError>>;

/// Called with the name of a remote event no op of this sequence consumes (other sequences, mission systems).
pub type RemoteSink = Rc<dyn Fn(&str)>;

/// A delayed impulse, or a `Delay` completion (`finish`: fire output `input` of the op).
#[derive(Debug, Clone, Copy)]
struct Impulse {
    due: f64,
    order: u64,
    op: i32,
    input: i32,
    finish: bool,
}

/// Census of a loaded graph ([`Kismet::link_stats`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct LinkStats {
    /// Output links of every op.
    pub outputs: usize,
    /// Links of those outputs.
    pub links: usize,
    /// Links whose `LinkedOp` is null or not an op of this sequence.
    pub unresolved: usize,
    /// Variable links of every op.
    pub variable_links: usize,
}

/// The executor of one sequence (module documentation).
pub struct Kismet {
    package: PackageId,
    ops: Vec<Op>,
    by_index: BTreeMap<i32, usize>,
    /// By lower-cased class path.
    handlers: BTreeMap<String, Handler>,
    variables: BTreeMap<i32, ObjRef>,
    /// Delayed impulses and `Delay` completions.
    queue: Vec<Impulse>,
    /// Queued ops by export index, top = last.
    stack: Vec<i32>,
    /// Pending input activations per op.
    inputs: BTreeMap<i32, VecDeque<i32>>,
    /// Links fired by the op running now, applied after it.
    collected: Vec<(i32, i32)>,
    now: f64,
    order: u64,
    /// Remote events no op of this sequence consumes.
    pub remote_sink: Option<RemoteSink>,
    /// One line per executed impulse, in order.
    pub trace: Vec<String>,
    /// Unsupported ops, unresolved links, the runaway guard.
    pub errors: Vec<String>,
    /// Ops run (and `Delay` completions) over the life of this executor.
    pub executed: usize,
    /// Ops per [`Kismet::run`] call.
    pub execution_limit: usize,
}

impl fmt::Debug for Kismet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kismet")
            .field("package", &self.package)
            .field("ops", &self.ops.len())
            .field("now", &self.now)
            .field("executed", &self.executed)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Values, read as the C++ `Value` reads them
// ---------------------------------------------------------------------------------------------------------------

/// The C++ `Value::number()`: a float as itself, every other kind as its integer field (0 for kinds without one).
fn number(v: &Value) -> f64 {
    match v {
        Value::Float(f) => f64::from(*f),
        Value::Int(i) => f64::from(*i),
        Value::Byte(b) => f64::from(*b),
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// The C++ `Value::integer()`: a float truncated, every other kind as its integer field (0 for kinds without one).
/// `None` for a float the C++ cast leaves undefined (NaN, or outside the 64-bit range).
fn integer(v: &Value) -> Option<i64> {
    match v {
        Value::Float(f) => {
            let x = f64::from(*f).trunc();
            // N5: the range check makes the cast exact; 2^63 itself is outside.
            (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0)
                .contains(&x)
                .then_some(x as i64)
        }
        Value::Int(i) => Some(i64::from(*i)),
        Value::Byte(b) => Some(i64::from(*b)),
        Value::Bool(b) => Some(i64::from(*b)),
        _ => Some(0),
    }
}

/// The C++ `Value::s` of a string or a name (empty for other kinds).
fn text(space: &ObjectSpace, v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        Value::Name(n) => space
            .names
            .get(*n)
            .map(|t| t.into_owned())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// `value` with a number written in its own kind (module documentation); `None` where that kind cannot hold it.
fn same_kind(slot: &Value, n: i64) -> Option<Value> {
    Some(match slot {
        Value::Int(_) => Value::Int(i32::try_from(n).ok()?),
        Value::Byte(_) => Value::Byte(u8::try_from(n).ok()?),
        Value::Bool(_) => Value::Bool(n != 0),
        // N5: an i64 count to a float rounds to the nearest float, as the C++ int-to-float read of it would.
        Value::Float(_) => Value::Float(n as f32),
        _ => return None,
    })
}

/// Field `name` (any case) of a struct value; `None` for another kind or an absent field (the C++ `field`).
fn field<'v>(space: &ObjectSpace, value: &'v Value, name: &str) -> Option<&'v Value> {
    let s = value.as_struct()?;
    let id = space.names.find(name)?;
    let (entry, _) = space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(entry as usize)
}

/// The elements of an array value, or none.
fn elements(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::elements).unwrap_or(&[])
}

/// Property `name` (any case) of object `o`, or `None` where its class does not declare it (the C++
/// `Runtime::property` returning null).
pub fn property(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<Value>, KismetError> {
    let class = space.heap.get(o)?.class;
    let Some(p) = space.prop(class, name) else {
        return Ok(None);
    };
    Ok(Some(space.read(o, PropAddr::scalar(p))?.clone()))
}

/// Writes property `name` of `o`; `false` where its class does not declare it.
fn set_property(
    space: &mut ObjectSpace,
    o: ObjRef,
    name: &str,
    v: Value,
) -> Result<bool, KismetError> {
    let class = space.heap.get(o)?.class;
    let Some(p) = space.prop(class, name) else {
        return Ok(false);
    };
    space.write(o, PropAddr::scalar(p), v)?;
    Ok(true)
}

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// The package and export a reference names: a decoder stand-in's ([`Origin::Resource`], what the C++ executor and
/// this one meet over a decode context), or an instantiated export's ([`Origin::Export`], what a definition store's
/// holder resolves a reference to, so a world-hosted executor sees its links land). `None` for other objects.
fn resource(space: &ObjectSpace, v: &Value) -> Option<(PackageId, i32)> {
    let Value::Object(Some(o)) = v else {
        return None;
    };
    match space.heap.get(*o).ok()?.origin {
        Origin::Resource { package, index } | Origin::Export { package, index } => {
            Some((package, index))
        }
        _ => None,
    }
}

impl Kismet {
    /// The executor of the sequence at `sequence_path` in `package` (a filed id, as [`ow_object::reflect::CodeIndex`]
    /// hands out): every export whose outer is the sequence and whose class derives from `Engine.SequenceOp` is
    /// instantiated, in export order. Exports whose class cannot be loaded are skipped, as the C++ catches them; a
    /// sequence with no ops is valid. The logic ops are registered.
    pub fn new(
        cx: &mut ValueCx<'_>,
        package: PackageId,
        sequence_path: &str,
    ) -> Result<Kismet, KismetError> {
        let sequence = cx.code.find_export(cx.store, package, sequence_path);
        if sequence <= 0 {
            return Err(KismetError::SequenceNotFound(sequence_path.to_string()));
        }
        let op_class = cx.code.find_class(cx.store, "Engine.SequenceOp")?;
        let prefix =
            PropertyPrefix::from_bytes(OP_PREFIX).ok_or(DecodeError::UnsupportedRequest)?;
        let pkg = cx.store.shared(package);
        let mut kismet = Kismet {
            package,
            ops: Vec::new(),
            by_index: BTreeMap::new(),
            handlers: BTreeMap::new(),
            variables: BTreeMap::new(),
            queue: Vec::new(),
            stack: Vec::new(),
            inputs: BTreeMap::new(),
            collected: Vec::new(),
            now: 0.0,
            order: 0,
            remote_sink: None,
            trace: Vec::new(),
            errors: Vec::new(),
            executed: 0,
            execution_limit: EXECUTION_LIMIT,
        };
        for (i, exported) in pkg.exports.iter().enumerate() {
            if exported.outer != sequence {
                continue;
            }
            let index = i32::try_from(i + 1).map_err(|_| DecodeError::UnsupportedRequest)?;
            let class = match cx.code.class_at(cx.store, package, exported.class) {
                Ok(Some(class)) => class,
                Ok(None) | Err(_) => continue,
            };
            if !cx.code.is_child_of(class, op_class) {
                continue;
            }
            let object = instantiate_export(cx, package, index, prefix)?.object;
            kismet.by_index.insert(index, kismet.ops.len());
            kismet.ops.push(Op {
                object,
                index,
                class,
                cls: cx.code.class(class).path.clone(),
                name: exported.name.clone(),
                path: pkg.path_of(index)?,
            });
        }
        kismet.register_logic_ops();
        Ok(kismet)
    }

    /// The ops, in export order.
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// The first op named `name` (exactly), as a position in [`Kismet::ops`].
    pub fn find(&self, name: &str) -> Option<usize> {
        self.ops.iter().position(|op| op.name == name)
    }

    /// The op of export `index`, as a position in [`Kismet::ops`].
    pub fn find_by_index(&self, index: i32) -> Option<usize> {
        self.by_index.get(&index).copied()
    }

    /// The sequence's package.
    pub fn package(&self) -> PackageId {
        self.package
    }

    /// The executor's clock, in seconds.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// Whether nothing is queued, delayed or collected.
    pub fn idle(&self) -> bool {
        self.queue.is_empty() && self.stack.is_empty() && self.collected.is_empty()
    }

    /// Binds `handler` to the class at `class_path` (any case) and its subclasses; a nearer class's handler wins.
    pub fn handle(&mut self, class_path: &str, handler: Handler) {
        self.handlers.insert(lower(class_path), handler);
    }

    fn handler_for(&self, cx: &ValueCx<'_>, class: ClassKey) -> Option<Handler> {
        cx.code
            .super_chain(class)
            .find_map(|c| self.handlers.get(&lower(&cx.code.class(c).path)))
            .cloned()
    }

    // -----------------------------------------------------------------------------------------------------------
    // Property access
    // -----------------------------------------------------------------------------------------------------------

    /// Property `name` of op `op`, or `None` where its class does not declare it.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn prop(
        &self,
        cx: &ValueCx<'_>,
        op: usize,
        name: &str,
    ) -> Result<Option<Value>, KismetError> {
        property(cx.space, self.ops[op].object, name)
    }

    fn links(&self, cx: &ValueCx<'_>, op: usize, name: &str) -> Result<Vec<Value>, KismetError> {
        Ok(match self.prop(cx, op, name)? {
            Some(Value::Array(a)) => a.to_vec(),
            _ => Vec::new(),
        })
    }

    /// The `LinkDesc` of input `input` of op `op`, empty when there is none.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn input_desc(
        &self,
        cx: &ValueCx<'_>,
        op: usize,
        input: i32,
    ) -> Result<String, KismetError> {
        Ok(self
            .input_link(cx, op, input)?
            .and_then(|link| field(cx.space, &link, "LinkDesc").map(|d| text(cx.space, d)))
            .unwrap_or_default())
    }

    fn input_link(
        &self,
        cx: &ValueCx<'_>,
        op: usize,
        input: i32,
    ) -> Result<Option<Value>, KismetError> {
        let links = self.links(cx, op, "InputLinks")?;
        Ok(usize::try_from(input)
            .ok()
            .and_then(|i| links.get(i))
            .cloned())
    }

    fn input_disabled(&self, cx: &ValueCx<'_>, op: usize, input: i32) -> Result<bool, KismetError> {
        Ok(self
            .input_link(cx, op, input)?
            .and_then(|link| field(cx.space, &link, "bDisabled").map(|d| d.truth(&cx.space.names)))
            .unwrap_or(false))
    }

    fn input_delay(&self, cx: &ValueCx<'_>, op: usize, input: i32) -> Result<f64, KismetError> {
        Ok(self
            .input_link(cx, op, input)?
            .and_then(|link| field(cx.space, &link, "ActivateDelay").map(number))
            .unwrap_or(0.0))
    }

    /// The index of the output whose `LinkDesc` is `desc` (any case), or `None`.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn output_index(
        &self,
        cx: &ValueCx<'_>,
        op: usize,
        desc: &str,
    ) -> Result<Option<i32>, KismetError> {
        let wanted = lower(desc);
        for (i, link) in self.links(cx, op, "OutputLinks")?.iter().enumerate() {
            if field(cx.space, link, "LinkDesc")
                .is_some_and(|d| lower(&text(cx.space, d)) == wanted)
            {
                return Ok(Some(
                    i32::try_from(i).map_err(|_| DecodeError::UnsupportedRequest)?,
                ));
            }
        }
        Ok(None)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Firing and impulses
    // -----------------------------------------------------------------------------------------------------------

    /// Fires the output whose `LinkDesc` is `desc`; an error line when the op has none.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn fire_named(
        &mut self,
        cx: &mut ValueCx<'_>,
        op: usize,
        desc: &str,
    ) -> Result<(), KismetError> {
        match self.output_index(cx, op, desc)? {
            Some(index) => self.fire(cx, op, index),
            None => {
                let line = format!("{}: no output link named '{desc}'", self.ops[op].name);
                self.errors.push(line);
                Ok(())
            }
        }
    }

    /// Fires every link of output `output`: a disabled output is traced and fires nothing; a link whose delay (the
    /// target input's plus the output's) is above zero is queued for later, the others are collected and applied
    /// after the running op. A link that does not land on an op of this sequence is an error line; a null
    /// `LinkedOp` or a link without `InputLinkIdx` is skipped.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn fire(
        &mut self,
        cx: &mut ValueCx<'_>,
        op: usize,
        output: i32,
    ) -> Result<(), KismetError> {
        let outputs = self.links(cx, op, "OutputLinks")?;
        let name = self.ops[op].name.clone();
        let Some(link) = usize::try_from(output).ok().and_then(|i| outputs.get(i)) else {
            self.errors
                .push(format!("{name}: output index out of range"));
            return Ok(());
        };
        if field(cx.space, link, "bDisabled").is_some_and(|d| d.truth(&cx.space.names)) {
            self.trace.push(format!("{name} output {output} disabled"));
            return Ok(());
        }
        let output_delay = field(cx.space, link, "ActivateDelay").map_or(0.0, number);
        let targets = field(cx.space, link, "Links");
        let count = targets.and_then(Value::elements).map_or(0, <[Value]>::len);
        self.trace
            .push(format!("{name} output {output} -> {count} link(s)"));
        let targets = elements(targets).to_vec();
        for target in &targets {
            let (Some(linked), Some(input_index)) = (
                field(cx.space, target, "LinkedOp"),
                field(cx.space, target, "InputLinkIdx"),
            ) else {
                continue;
            };
            let Value::Object(Some(linked_object)) = linked else {
                continue;
            };
            let destination = match resource(cx.space, linked) {
                Some((package, index)) if package == self.package => self.find_by_index(index),
                _ => None,
            };
            let Some(destination) = destination else {
                let target_name = self.object_name(cx, *linked_object)?;
                self.errors.push(format!(
                    "{name}: linked op is outside this sequence: {target_name}"
                ));
                continue;
            };
            let Some(input) = integer(input_index) else {
                self.errors
                    .push(format!("{name}: InputLinkIdx is not an integer"));
                continue;
            };
            // The C++ `int(...)` of the 64-bit integer keeps its low 32 bits.
            let input = input as i32;
            // Delay = target input's ActivateDelay + output's (NATIVE_MISSION_DISPATCH.md A4 step 5, UNVERIFIED).
            let total = self.input_delay(cx, destination, input)? + output_delay;
            let index = self.ops[destination].index;
            if total > 0.0 {
                self.push_delayed(self.now + total, index, input, false);
            } else {
                self.collected.push((index, input));
            }
        }
        Ok(())
    }

    /// A stand-in's name as its package spells it (the C++ `resource->name`), else its heap name.
    fn object_name(&self, cx: &ValueCx<'_>, o: ObjRef) -> Result<String, KismetError> {
        let object = cx.space.heap.get(o)?;
        if let Origin::Resource { package, index } | Origin::Export { package, index } =
            object.origin
            && let Ok(entry) = cx.store.get(package).object(index)
        {
            return Ok(entry.name.clone());
        }
        Ok(cx
            .space
            .names
            .get(object.name)
            .map(|t| t.into_owned())
            .unwrap_or_else(|| "?".to_string()))
    }

    fn push_delayed(&mut self, due: f64, op: i32, input: i32, finish: bool) {
        let order = self.order;
        self.order += 1;
        self.queue.push(Impulse {
            due,
            order,
            op,
            input,
            finish,
        });
    }

    /// An input activation: counted on the op (a second hit before it runs makes it run twice) and the op queued
    /// unless it already is (A4 steps 1, 6, 7; UNVERIFIED). A disabled input receives nothing.
    fn impulse(&mut self, cx: &ValueCx<'_>, index: i32, input: i32) -> Result<(), KismetError> {
        let Some(op) = self.find_by_index(index) else {
            return Ok(());
        };
        if input != EVENT_INPUT && self.input_disabled(cx, op, input)? {
            self.trace
                .push(format!("{} input {input} disabled", self.ops[op].name));
            return Ok(());
        }
        self.inputs.entry(index).or_default().push_back(input);
        if !self.stack.contains(&index) {
            self.stack.push(index);
        }
        Ok(())
    }

    /// The links fired by one op: inputs counted in link order, the ops pushed last to first so that the first
    /// link's target is on top (depth-first in link order).
    fn apply_collected(&mut self, cx: &ValueCx<'_>) -> Result<(), KismetError> {
        let links = std::mem::take(&mut self.collected);
        for &(index, input) in &links {
            let Some(op) = self.find_by_index(index) else {
                continue;
            };
            if self.input_disabled(cx, op, input)? {
                self.trace
                    .push(format!("{} input {input} disabled", self.ops[op].name));
                continue;
            }
            self.inputs.entry(index).or_default().push_back(input);
        }
        for &(index, _) in links.iter().rev() {
            if self.inputs.entry(index).or_default().is_empty() {
                continue;
            }
            if !self.stack.contains(&index) {
                self.stack.push(index);
            }
        }
        Ok(())
    }

    /// An impulse on input `input` of op `op`, after `delay` seconds plus the input's `ActivateDelay`.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn activate(
        &mut self,
        cx: &ValueCx<'_>,
        op: usize,
        input: i32,
        delay: f64,
    ) -> Result<(), KismetError> {
        let total = delay + self.input_delay(cx, op, input)?;
        let index = self.ops[op].index;
        if total > 0.0 {
            self.push_delayed(self.now + total, index, input, false);
            Ok(())
        } else {
            self.impulse(cx, index, input)
        }
    }

    /// Enters event op `op`: traced, its `TriggerCount` counted, and queued to fire all of its outputs (disabled ones
    /// excepted) when it runs (A4, UNVERIFIED).
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn activate_event(&mut self, cx: &mut ValueCx<'_>, op: usize) -> Result<(), KismetError> {
        self.trace.push(format!("event {}", self.ops[op].name));
        self.count(cx, op, "TriggerCount")?;
        let index = self.ops[op].index;
        self.impulse(cx, index, EVENT_INPUT)
    }

    /// Adds one to the op's count property `name`, where its class declares it.
    fn count(&mut self, cx: &mut ValueCx<'_>, op: usize, name: &str) -> Result<(), KismetError> {
        let object = self.ops[op].object;
        let Some(current) = property(cx.space, object, name)? else {
            return Ok(());
        };
        let next = integer(&current)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| same_kind(&current, n));
        match next {
            Some(v) => {
                set_property(cx.space, object, name, v)?;
            }
            None => {
                let line = format!("{}: {name} cannot count further", self.ops[op].name);
                self.errors.push(line);
            }
        }
        Ok(())
    }

    fn is_child_of(&self, cx: &ValueCx<'_>, op: usize, base: ClassKey) -> bool {
        cx.code.is_child_of(self.ops[op].class, base)
    }

    /// Enters every `Engine.SeqEvent_RemoteEvent` op (that class exactly: mission remote events are another family,
    /// [`Kismet::mission_remote_event`]) whose `EventName` is `name`, in op order; with no match, the remote sink is
    /// told. Returns the number entered.
    pub fn remote_event(&mut self, cx: &mut ValueCx<'_>, name: &str) -> Result<usize, KismetError> {
        let remote = cx
            .code
            .find_class(cx.store, "Engine.SeqEvent_RemoteEvent")?;
        let mut matched = 0;
        for op in 0..self.ops.len() {
            if !self.is_child_of(cx, op, remote) {
                continue;
            }
            if self.ops[op].cls != "Engine.SeqEvent_RemoteEvent" {
                continue;
            }
            if self
                .prop(cx, op, "EventName")?
                .is_some_and(|v| text(cx.space, &v) == name)
            {
                self.activate_event(cx, op)?;
                matched += 1;
            }
        }
        if matched == 0
            && let Some(sink) = self.remote_sink.clone()
        {
            sink(name);
        }
        Ok(matched)
    }

    /// The path of the object a reference names, in the package that holds it (an unresolved import's path in the
    /// referencing package).
    fn reference_path(&self, cx: &ValueCx<'_>, v: &Value) -> Result<Option<String>, KismetError> {
        let Some((package, index)) = resource(cx.space, v) else {
            return Ok(None);
        };
        Ok(Some(cx.store.get(package).path_of(index)?))
    }

    /// Enters every `WillowGame.WillowSeqEvent_MissionRemoteEvent` op whose `EventName` is `name` and whose
    /// `AssociatedMissionDefinition` is the object at `mission_path` (its path in its own package). Returns the number
    /// entered.
    pub fn mission_remote_event(
        &mut self,
        cx: &mut ValueCx<'_>,
        mission_path: &str,
        name: &str,
    ) -> Result<usize, KismetError> {
        let mission = cx
            .code
            .find_class(cx.store, "WillowGame.WillowSeqEvent_MissionRemoteEvent")?;
        let mut matched = 0;
        for op in 0..self.ops.len() {
            if !self.is_child_of(cx, op, mission) {
                continue;
            }
            let (Some(event_name), Some(associated)) = (
                self.prop(cx, op, "EventName")?,
                self.prop(cx, op, "AssociatedMissionDefinition")?,
            ) else {
                continue;
            };
            if text(cx.space, &event_name) != name {
                continue;
            }
            if self.reference_path(cx, &associated)?.as_deref() != Some(mission_path) {
                continue;
            }
            self.activate_event(cx, op)?;
            matched += 1;
        }
        Ok(matched)
    }

    /// The `Engine.SequenceEvent` ops whose `Originator` is the object at `object_path` (a placed population den or
    /// spawn point, for example). Query only: the caller enters them with [`Kismet::activate_event`].
    pub fn events_for_originator(
        &mut self,
        cx: &mut ValueCx<'_>,
        object_path: &str,
    ) -> Result<Vec<usize>, KismetError> {
        let event = cx.code.find_class(cx.store, "Engine.SequenceEvent")?;
        let mut result = Vec::new();
        for op in 0..self.ops.len() {
            if !self.is_child_of(cx, op, event) {
                continue;
            }
            let Some(originator) = self.prop(cx, op, "Originator")? else {
                continue;
            };
            if self.reference_path(cx, &originator)?.as_deref() == Some(object_path) {
                result.push(op);
            }
        }
        Ok(result)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Running
    // -----------------------------------------------------------------------------------------------------------

    /// The due impulse that runs next: the earliest, ties in activation order (the C++ `std::min_element` with its
    /// comparator, so a NaN time compares as the C++ compares it).
    fn next_due(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, x) in self.queue.iter().enumerate() {
            match best {
                None => best = Some(i),
                Some(b) => {
                    let y = &self.queue[b];
                    let less = if x.due != y.due {
                        x.due < y.due
                    } else {
                        x.order < y.order
                    };
                    if less {
                        best = Some(i);
                    }
                }
            }
        }
        best
    }

    /// Runs every queued op and every due impulse, at most [`Kismet::execution_limit`] ops (past it: an error line,
    /// and the stack, the pending inputs and the collected links are dropped; delayed impulses stay).
    pub fn run(&mut self, cx: &mut ValueCx<'_>) -> Result<(), KismetError> {
        let mut steps = 0usize;
        // Links fired from outside run() (a host-fired output).
        self.apply_collected(cx)?;
        loop {
            while let Some(next) = self.next_due() {
                if self.queue[next].due > self.now {
                    break;
                }
                let due = self.queue.remove(next);
                let Some(op) = self.find_by_index(due.op) else {
                    continue;
                };
                if due.finish {
                    self.executed += 1;
                    self.trace.push(format!("{} finished", self.ops[op].name));
                    self.fire(cx, op, due.input)?;
                    self.apply_collected(cx)?;
                } else {
                    self.impulse(cx, due.op, due.input)?;
                }
            }
            let Some(&index) = self.stack.last() else {
                return Ok(());
            };
            steps += 1;
            if steps > self.execution_limit {
                self.errors
                    .push("kismet execution limit exceeded".to_string());
                self.stack.clear();
                self.inputs.clear();
                self.collected.clear();
                return Ok(());
            }
            self.stack.pop();
            let Some(input) = self.inputs.entry(index).or_default().pop_front() else {
                continue;
            };
            self.executed += 1;
            if let Some(op) = self.find_by_index(index) {
                if input == EVENT_INPUT {
                    let count = self.links(cx, op, "OutputLinks")?.len();
                    for output in 0..count {
                        let output =
                            i32::try_from(output).map_err(|_| DecodeError::UnsupportedRequest)?;
                        self.fire(cx, op, output)?;
                    }
                } else {
                    self.execute(cx, op, input)?;
                }
            }
            // A remaining activation queues the op again before its links are pushed on top (A4 steps 6-7).
            if self.inputs.get(&index).is_some_and(|p| !p.is_empty())
                && !self.stack.contains(&index)
            {
                self.stack.push(index);
            }
            self.apply_collected(cx)?;
        }
    }

    /// Advances the clock by `seconds` and runs what is due ([`Kismet::run`]).
    pub fn tick(&mut self, cx: &mut ValueCx<'_>, seconds: f64) -> Result<(), KismetError> {
        self.now += seconds;
        self.run(cx)
    }

    fn execute(&mut self, cx: &mut ValueCx<'_>, op: usize, input: i32) -> Result<(), KismetError> {
        self.count(cx, op, "ActivateCount")?;
        let desc = self.input_desc(cx, op, input)?;
        self.trace.push(format!("{} <- {desc}", self.ops[op].name));
        match self.handler_for(cx, self.ops[op].class) {
            Some(handler) => handler(self, cx, op, input),
            None => {
                let line = format!(
                    "unsupported op class {} ({})",
                    self.ops[op].cls, self.ops[op].name
                );
                self.errors.push(line);
                Ok(())
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Variables
    // -----------------------------------------------------------------------------------------------------------

    fn variable_object(
        &mut self,
        cx: &mut ValueCx<'_>,
        reference: &Value,
    ) -> Result<Option<ObjRef>, KismetError> {
        let Some((package, index)) = resource(cx.space, reference) else {
            return Ok(None);
        };
        if package != self.package {
            return Ok(None);
        }
        if let Some(&o) = self.variables.get(&index) {
            return Ok(Some(o));
        }
        let prefix =
            PropertyPrefix::from_bytes(OP_PREFIX).ok_or(DecodeError::UnsupportedRequest)?;
        let object = instantiate_export(cx, package, index, prefix)?.object;
        self.variables.insert(index, object);
        Ok(Some(object))
    }

    /// The variables linked to op `op` through its variable links whose `LinkDesc` is `desc` (any case), in link
    /// order; references outside this package are left out.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn variables(
        &mut self,
        cx: &mut ValueCx<'_>,
        op: usize,
        desc: &str,
    ) -> Result<Vec<ObjRef>, KismetError> {
        let wanted = lower(desc);
        let mut result = Vec::new();
        for link in self.links(cx, op, "VariableLinks")? {
            if field(cx.space, &link, "LinkDesc")
                .is_none_or(|d| lower(&text(cx.space, d)) != wanted)
            {
                continue;
            }
            let Some(linked) =
                field(cx.space, &link, "LinkedVariables").filter(|v| v.elements().is_some())
            else {
                continue;
            };
            for reference in elements(Some(linked)).to_vec() {
                if let Some(o) = self.variable_object(cx, &reference)? {
                    result.push(o);
                }
            }
        }
        Ok(result)
    }

    /// The value property of variable `i` of link `desc`: the first of `bValue`, `IntValue`, `FloatValue` its class
    /// declares (bool, int and float variables), with the variable; `None` when there is no such variable or value.
    ///
    /// # Panics
    /// If `op` is not a position in [`Kismet::ops`] (a caller bug).
    pub fn variable_value(
        &mut self,
        cx: &mut ValueCx<'_>,
        op: usize,
        desc: &str,
        i: usize,
    ) -> Result<Option<(ObjRef, &'static str, Value)>, KismetError> {
        let vars = self.variables(cx, op, desc)?;
        let Some(&var) = vars.get(i) else {
            return Ok(None);
        };
        for name in ["bValue", "IntValue", "FloatValue"] {
            if let Some(v) = property(cx.space, var, name)? {
                return Ok(Some((var, name, v)));
            }
        }
        Ok(None)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Census
    // -----------------------------------------------------------------------------------------------------------

    /// Counts the loaded graph's output links, their links, the links that do not land on an op of this sequence,
    /// and the variable links.
    pub fn link_stats(&self, cx: &ValueCx<'_>) -> Result<LinkStats, KismetError> {
        let mut stats = LinkStats::default();
        for op in 0..self.ops.len() {
            if let Some(v @ Value::Array(_)) = self.prop(cx, op, "VariableLinks")? {
                stats.variable_links += elements(Some(&v)).len();
            }
            let Some(outputs @ Value::Array(_)) = self.prop(cx, op, "OutputLinks")? else {
                continue;
            };
            for output in elements(Some(&outputs)) {
                stats.outputs += 1;
                for target in elements(field(cx.space, output, "Links")) {
                    stats.links += 1;
                    let lands = field(cx.space, target, "LinkedOp")
                        .and_then(|linked| resource(cx.space, linked))
                        .is_some_and(|(package, index)| {
                            package == self.package && self.find_by_index(index).is_some()
                        });
                    if !lands {
                        stats.unresolved += 1;
                    }
                }
            }
        }
        Ok(stats)
    }

    // -----------------------------------------------------------------------------------------------------------
    // The logic ops
    // -----------------------------------------------------------------------------------------------------------

    /// A property a logic op cannot work without (module documentation, "Differences from the C++").
    fn required(
        &mut self,
        cx: &ValueCx<'_>,
        op: usize,
        name: &str,
    ) -> Result<Option<Value>, KismetError> {
        let value = self.prop(cx, op, name)?;
        if value.is_none() {
            let line = format!("{}: {} without {name}", self.ops[op].name, self.ops[op].cls);
            self.errors.push(line);
        }
        Ok(value)
    }

    /// Writes a number into the op's property in the property's own kind (module documentation).
    fn set_number(
        &mut self,
        cx: &mut ValueCx<'_>,
        object: ObjRef,
        current: &Value,
        name: &str,
        n: i64,
        op: usize,
    ) -> Result<bool, KismetError> {
        match same_kind(current, n) {
            Some(v) => {
                set_property(cx.space, object, name, v)?;
                Ok(true)
            }
            None => {
                let line = format!("{}: {name} cannot hold {n}", self.ops[op].name);
                self.errors.push(line);
                Ok(false)
            }
        }
    }

    fn register_logic_ops(&mut self) {
        // An impulse on an event's input fires its first output (events entered remotely fire all of theirs).
        self.handle(
            "Engine.SequenceEvent",
            Rc::new(|k, cx, op, _| k.fire(cx, op, 0)),
        );
        self.handle(
            "Engine.SeqAct_ActivateRemoteEvent",
            Rc::new(activate_remote_event),
        );
        self.handle("Engine.SeqAct_Gate", Rc::new(gate));
        self.handle("Engine.SeqAct_SetBool", Rc::new(set_bool));
        self.handle("Engine.SeqCond_CompareBool", Rc::new(compare_bool));
        self.handle("Engine.SeqAct_Delay", Rc::new(delay));
    }
}

/// `SeqAct_ActivateRemoteEvent`: enters the remote events named by its `EventName` (an error line when it has none),
/// then fires `Out`.
fn activate_remote_event(
    k: &mut Kismet,
    cx: &mut ValueCx<'_>,
    op: usize,
    _: i32,
) -> Result<(), KismetError> {
    let name = k
        .prop(cx, op, "EventName")?
        .map(|v| text(cx.space, &v))
        .unwrap_or_default();
    if name.is_empty() {
        let line = format!("{}: remote event without a name", k.ops[op].name);
        k.errors.push(line);
        return Ok(());
    }
    let line = format!("{} remote '{name}'", k.ops[op].name);
    k.trace.push(line);
    k.remote_event(cx, &name)?;
    k.fire_named(cx, op, "Out")
}

/// `SeqAct_Gate`: `Open`, `Close` and `Toggle` set `bOpen`; `In` fires `Out` when open, and with an `AutoCloseCount`
/// above zero closes after that many passes (UNVERIFIED against the game). Any other input is an error line.
fn gate(k: &mut Kismet, cx: &mut ValueCx<'_>, op: usize, input: i32) -> Result<(), KismetError> {
    let desc = lower(&k.input_desc(cx, op, input)?);
    let object = k.ops[op].object;
    let Some(open) = k.required(cx, op, "bOpen")? else {
        return Ok(());
    };
    let is_open = open.truth(&cx.space.names);
    match desc.as_str() {
        "open" => k.set_number(cx, object, &open, "bOpen", 1, op).map(drop),
        "close" => k.set_number(cx, object, &open, "bOpen", 0, op).map(drop),
        "toggle" => k
            .set_number(cx, object, &open, "bOpen", i64::from(!is_open), op)
            .map(drop),
        "in" => {
            if !is_open {
                return Ok(());
            }
            k.fire_named(cx, op, "Out")?;
            let Some(limit) = k.required(cx, op, "AutoCloseCount")? else {
                return Ok(());
            };
            let Some(limit) = integer(&limit) else {
                let line = format!("{}: AutoCloseCount is not an integer", k.ops[op].name);
                k.errors.push(line);
                return Ok(());
            };
            // The C++ `int(...)` of the 64-bit integer keeps its low 32 bits.
            let limit = limit as i32;
            if limit <= 0 {
                return Ok(());
            }
            let Some(count) = k.required(cx, op, "CurrentCloseCount")? else {
                return Ok(());
            };
            let Some(next) = integer(&count).and_then(|n| n.checked_add(1)) else {
                let line = format!("{}: CurrentCloseCount cannot count further", k.ops[op].name);
                k.errors.push(line);
                return Ok(());
            };
            if !k.set_number(cx, object, &count, "CurrentCloseCount", next, op)? {
                return Ok(());
            }
            if next >= i64::from(limit) {
                k.set_number(cx, object, &open, "bOpen", 0, op)?;
                k.set_number(cx, object, &count, "CurrentCloseCount", 0, op)?;
            }
            Ok(())
        }
        _ => {
            let line = format!("{}: unsupported gate input '{desc}'", k.ops[op].name);
            k.errors.push(line);
            Ok(())
        }
    }
}

/// `SeqAct_SetBool`: the linked `Value` variable when there is one (non-zero is true), else the op's own
/// `DefaultValue`, written to every `Target` variable; then fires `Out`. No `Target` is an error line.
fn set_bool(k: &mut Kismet, cx: &mut ValueCx<'_>, op: usize, _: i32) -> Result<(), KismetError> {
    let Some(default) = k.required(cx, op, "DefaultValue")? else {
        return Ok(());
    };
    let mut value = default.truth(&cx.space.names);
    if let Some((_, _, linked)) = k.variable_value(cx, op, "Value", 0)? {
        let Some(n) = integer(&linked) else {
            let line = format!("{}: Value is not an integer", k.ops[op].name);
            k.errors.push(line);
            return Ok(());
        };
        value = n != 0;
    }
    let targets = k.variables(cx, op, "Target")?;
    if targets.is_empty() {
        let line = format!("{}: SetBool without a Target", k.ops[op].name);
        k.errors.push(line);
        return Ok(());
    }
    for i in 0..targets.len() {
        if let Some((var, name, slot)) = k.variable_value(cx, op, "Target", i)? {
            k.set_number(cx, var, &slot, name, i64::from(value), op)?;
        }
    }
    k.fire_named(cx, op, "Out")
}

/// `SeqCond_CompareBool`: the linked `Bool` variable (non-zero is true) sets `bResult` and fires `True` or `False`.
/// No `Bool` variable is an error line.
fn compare_bool(
    k: &mut Kismet,
    cx: &mut ValueCx<'_>,
    op: usize,
    _: i32,
) -> Result<(), KismetError> {
    let Some((_, _, value)) = k.variable_value(cx, op, "Bool", 0)? else {
        let line = format!("{}: CompareBool without a Bool variable", k.ops[op].name);
        k.errors.push(line);
        return Ok(());
    };
    let Some(n) = integer(&value) else {
        let line = format!("{}: Bool is not an integer", k.ops[op].name);
        k.errors.push(line);
        return Ok(());
    };
    let result = n != 0;
    let object = k.ops[op].object;
    let Some(current) = k.required(cx, op, "bResult")? else {
        return Ok(());
    };
    if !k.set_number(cx, object, &current, "bResult", i64::from(result), op)? {
        return Ok(());
    }
    k.fire_named(cx, op, if result { "True" } else { "False" })
}

/// `SeqAct_Delay`: `Start` queues a firing of `Finished` after the linked `Duration` variable's value, else the op's
/// own `Duration` (a zero duration completes in the same run). Any other input, and no `Finished` output, is an error
/// line.
fn delay(k: &mut Kismet, cx: &mut ValueCx<'_>, op: usize, input: i32) -> Result<(), KismetError> {
    let desc = lower(&k.input_desc(cx, op, input)?);
    if desc != "start" {
        let line = format!("{}: delay input '{desc}' is unsupported", k.ops[op].name);
        k.errors.push(line);
        return Ok(());
    }
    let Some(own) = k.required(cx, op, "Duration")? else {
        return Ok(());
    };
    let mut duration = number(&own);
    if let Some((_, _, linked)) = k.variable_value(cx, op, "Duration", 0)? {
        duration = number(&linked);
    }
    let Some(finished) = k.output_index(cx, op, "Finished")? else {
        let line = format!("{}: delay without a Finished output", k.ops[op].name);
        k.errors.push(line);
        return Ok(());
    };
    let index = k.ops[op].index;
    k.push_delayed(k.now + duration, index, finished, true);
    Ok(())
}
