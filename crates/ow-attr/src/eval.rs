//! Owner: lane W3-ATTREVAL. The chain walks and the `AttributeInitializationData` evaluator: the bodies of the
//! [`AttributeSystem`] methods `resolve_context`, `resolve_contexts`, `value_of`, `base_value_of`,
//! `value_from_context`, `base_value_from_context` and `evaluate_init` (`registry.rs` delegates here in one line;
//! architecture section 3.5), and the readers of definition data the value resolvers share.
//!
//! The rules are NATIVE_ATTRIBUTES.md section 1 and NATIVE_PROGRESSION.md section 1, read from native code and
//! **UNVERIFIED in the running game**. Two numbers that run through the evaluator are confirmed live
//! (REALGAME_GROUND_TRUTH.md, 2026-10-02): the level curve and the maximum health base (`math.rs` checks both, and
//! the tests here run them through definitions).
//!
//! # Reading definitions
//!
//! The readers below (`ensure_loaded`, `class_path`, `property`, `required`, `field`, `number`, `flag`, `object`,
//! `elements`, `init_data`) are public so the crates above (`ow-balance`) read definition data the same way instead of
//! keeping a copy (hub grant, W3-BALANCE).
//!
//! Every property read goes through [`property`], which first makes sure the object is loaded: an unloaded
//! definition handle (`Origin::Resource`, no slots: what `ow-defs` hands out for a referenced definition) is loaded
//! through the world's `DefStore`; any other live object is read as it is. A world without a store answers
//! `NotYet { LayerAbsent("ow-defs") }` for an unloaded handle, never a guessed value.
//!
//! # Where the two C++ evaluators disagree
//!
//! `progression.cpp` (`AttributeEvaluator`) and `weapon_gen.cpp` (`AttributeValue`) are each a subset; this file
//! follows the native reading and, where the reading is silent, names the oracle it follows:
//!
//! - the power: `cx.forms.pow` (architecture 5.3; both forms give the same bits on every fixture, see the tests);
//! - `RoundingMode` 1 (`ATTRROUNDING_IntRound`): `floor(x + 0.5)` as `weapon_gen.cpp` `Round()`
//!   ([`ow_core::num::round_mode`]); `progression.cpp` `restricted()` rounds halves away from zero instead. They
//!   differ only on negative halves (`-2.5`, and the sign of a zero from `-0.3`); no fixture has one. UNVERIFIED;
//!   reported as an open item for architecture 5.3;
//! - a definition with neither `ValueFormula` nor `ConditionalInitialization` enabled leaves the base as it is (no
//!   `BaseValueMode` step), as `weapon_gen.cpp` does; a random variance is still added. `progression.cpp` refuses such
//!   a definition unless it is random, and then combines `f = 0` by the mode, which differs only for a non-zero base
//!   with mode 0. UNVERIFIED;
//! - an attribute operand of a definition (a formula term, a bound, a conditional's value) is evaluated with the same
//!   source and override as the data that names it.
//!
//! # Cycles
//!
//! Data drives the recursion (a formula term names an attribute whose resolver evaluates more data), so every nested
//! step enters a guard: at most [`MAX_DEPTH`] steps deep, and a step already in progress with the same key (the same
//! definition with the same sources, the same attribute on the same context) is a cycle. Both are
//! [`AttrError::BadData`]. Depth alone does not bound the work: a sub-definition named by all four formula terms (or
//! by both clamp bounds) is evaluated once per naming, so a chain of them costs about 4^n steps well under the depth
//! cap. So the guard also counts the steps one outermost step takes (every entry while the stack is not empty, the
//! count reset when an entry finds it empty), and refuses the step past [`MAX_WORK`] as `BadData`. Both budgets are
//! this project's refusals, not rules of the game. The guard is a per-thread stack because resolvers re-enter through
//! the public [`AttributeSystem`] methods, which carry no depth; the simulation is single-threaded (architecture
//! D18), and the guard pops on every return path.

use std::cell::RefCell;

use ow_core::NotYet;
use ow_core::num::{RoundingMode, round_mode};
use ow_core::rand::CrtStream;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::value::Value;

use crate::api::{AttrError, AttrValue, EvalCx, ExprMode, InitData, WorldParts};
use crate::math::{self, BaseValueMode};
use crate::registry::AttributeSystem;

/// The record the chain rules come from.
pub(crate) const CHAINS: &str = "NATIVE_ATTRIBUTES.md section 1";

/// How deep one evaluation may nest (definitions, attributes and context chains together) before it is refused as
/// bad data. The shipped data nests a few levels; the budget is a guard, not a rule of the game.
pub const MAX_DEPTH: usize = 64;

/// How many guarded steps one outermost step may take (module documentation, "Cycles"). A shipped evaluation takes
/// tens; the budget is a guard against data that names one sub-definition many times, not a rule of the game.
pub const MAX_WORK: usize = 10_000;

/// The class whose definitions expand to several contexts (`ResolveContexts`); its expansion is not read.
const MULTI_CONTEXT_CLASS: &str = "Engine.AttributeDefinitionMultiContext";

// ---------------------------------------------------------------------------------------------------------------
// The cycle guard
// ---------------------------------------------------------------------------------------------------------------

/// One evaluation step in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// An `AttributeInitializationDefinition` evaluated for these sources.
    Definition {
        definition: ObjRef,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
    },
    /// An attribute's context chain run on a source.
    Context {
        attr: ObjRef,
        source: Option<ObjRef>,
    },
    /// An attribute's value chain run on a context (the value or the base fold).
    Value {
        attr: ObjRef,
        context: ObjRef,
        base: bool,
    },
}

/// The steps in progress and the work the current outermost step has taken.
struct Active {
    steps: Vec<Step>,
    work: usize,
}

thread_local! {
    static ACTIVE: RefCell<Active> = const { RefCell::new(Active { steps: Vec::new(), work: 0 }) };
}

/// A step on the per-thread stack; popped when dropped, on every return path.
struct Guard;

impl Guard {
    fn enter(step: Step) -> Result<Guard, AttrError> {
        let entered = ACTIVE.try_with(|active| {
            let mut active = active.try_borrow_mut().map_err(|_| AttrError::BadData {
                reason: "the attribute evaluation guard is busy".into(),
            })?;
            if active.steps.is_empty() {
                active.work = 0;
            }
            active.work += 1;
            if active.work > MAX_WORK {
                return Err(AttrError::BadData {
                    reason: format!("attribute evaluation took more than {MAX_WORK} steps").into(),
                });
            }
            if active.steps.len() >= MAX_DEPTH {
                return Err(AttrError::BadData {
                    reason: format!("attribute evaluation nested deeper than {MAX_DEPTH} steps")
                        .into(),
                });
            }
            if active.steps.contains(&step) {
                return Err(AttrError::BadData {
                    reason: format!("attribute evaluation cycle at {step:?}").into(),
                });
            }
            active.steps.push(step);
            Ok(())
        });
        match entered {
            Ok(Ok(())) => Ok(Guard),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AttrError::BadData {
                reason: "the attribute evaluation guard is gone (thread ending)".into(),
            }),
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = ACTIVE.try_with(|active| {
            if let Ok(mut active) = active.try_borrow_mut() {
                active.steps.pop();
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Reading objects and values
// ---------------------------------------------------------------------------------------------------------------

thread_local! {
    /// The unit of the innermost [`load_free`] scope, or None outside one.
    static LOAD_FREE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// Runs `f` with definition loads refused (lane IMPL-ADD-ATTRIBUTE-MODIFIER, NATIVE_ADD_ATTRIBUTE_MODIFIER.md section
/// 3, step 8). Inside, an unloaded definition handle answers `NotYet { NotRead }` naming `unit` and loads nothing, on
/// the two load paths that check the scope ([`load_free_refusal`]): this module's [`ensure_loaded`] and the context
/// resolvers' own `ensure_loaded` (review r1 of #616). The scope ends when `f` returns, also on an error; scopes nest,
/// and the outer scope is back when an inner one ends.
///
/// **What it does not cover** (review r1 of #616): the black-market upgrade's `load_for_write` (a write path, which
/// loads on purpose) and the interface class lookups (`find_class`) of the balanced and inventory context resolvers,
/// which can load a class package. A probe that reaches either is not made load-free by this scope. The add's probe
/// (`effects::check_modifier_chain` with `ProbeOp::Add`) reaches neither: it resolves no context and writes nothing.
pub fn load_free<T>(unit: &'static str, f: impl FnOnce() -> T) -> T {
    let before = LOAD_FREE.with(|l| l.replace(Some(unit)));
    struct Restore(Option<&'static str>);
    impl Drop for Restore {
        fn drop(&mut self) {
            LOAD_FREE.with(|l| l.set(self.0));
        }
    }
    let _restore = Restore(before);
    f()
}

/// The load-free scope's check, shared by every load path that honours it ([`load_free`]): inside a scope, an unloaded
/// handle `obj` is `NotYet { NotRead }`, "<unit>: <role> <name> is an unloaded handle (a load is a write)"; otherwise,
/// or for any other object, nothing.
pub(crate) fn load_free_refusal(
    space: &ObjectSpace,
    defs: &ow_defs::DefStore,
    obj: ObjRef,
    role: &str,
) -> Result<(), AttrError> {
    if let Some(unit) = LOAD_FREE.with(|l| l.get())
        && defs.is_unloaded(space, obj)
    {
        let name = space.names.text(space.heap.get(obj)?.name).into_owned();
        return Err(NotYet::not_read(
            format!("{unit}: {role} {name} is an unloaded handle (a load is a write)"),
            "NATIVE_ADD_ATTRIBUTE_MODIFIER.md",
        )
        .into());
    }
    Ok(())
}

/// Whether a [`load_free`] scope is open, and its unit (for tests of the scope).
pub fn load_free_unit() -> Option<&'static str> {
    LOAD_FREE.with(|l| l.get())
}

/// Makes sure `obj` can be read (module documentation). Only an object with `Origin::Resource` can be an unloaded
/// definition handle (a load gives it `Origin::Export`), so only such an object goes to the world's store, whose
/// [`DefStore::ensure_loaded`](ow_defs::DefStore::ensure_loaded) loads it, or leaves it as it is when it is a
/// stand-in that stays one. Every other live object (run-time objects, loaded definitions, class defaults) is read as
/// it is. A world without a store answers `NotYet { LayerAbsent("ow-defs") }` for a slotless resource handle.
pub fn ensure_loaded(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<(), AttrError> {
    ensure_loaded_as(cx, obj, "a default value's handle")
}

/// [`ensure_loaded`] with the `role` a load-free scope's refusal names (review of #791, item 3: a chain entry is named
/// as the walk's resolver, not as a default value's handle).
fn ensure_loaded_as(cx: &mut EvalCx<'_>, obj: ObjRef, role: &str) -> Result<(), AttrError> {
    let WorldParts { space, defs, .. } = cx.world.parts();
    let (resource, slotless) = {
        let o = space.heap.get(obj)?;
        (
            matches!(o.origin, Origin::Resource { .. }),
            o.slots.is_empty(),
        )
    };
    if !resource {
        return Ok(());
    }
    match defs {
        Some(defs) => {
            // Inside a load-free scope ([`load_free`]) an unloaded handle refuses instead of loading (a load is a write).
            load_free_refusal(space, defs, obj, role)?;
            defs.ensure_loaded(space, obj)?;
            slotless_refusal(space, true, obj)
        }
        None if slotless => slotless_refusal(space, false, obj),
        None => Ok(()),
    }
}

/// The refusals [`ensure_loaded`] gives a slotless resource object, without loading anything: with a store, a handle
/// the store leaves a stand-in (an object inside a class default object, for one) holds no field although its class
/// has some, so reading it would be a heap error and it is `NotYet { NotRead }` naming the object (lane
/// FIX-PHASELOCK-CHAIN, decision 2026-11-16-69); without a store, any slotless object is `NotYet { LayerAbsent }`
/// for `ow-defs`. Any other object passes. Shared with resident-only readers (review r2 of #584, item 2).
pub(crate) fn slotless_refusal(space: &ObjectSpace, with_store: bool, obj: ObjRef) -> Result<(), AttrError> {
    let o = space.heap.get(obj)?;
    if !matches!(o.origin, Origin::Resource { .. }) || !o.slots.is_empty() {
        return Ok(());
    }
    if !with_store {
        return Err(NotYet::layer_absent(format!("loading the unloaded definition {obj:?}"), "ow-defs", CHAINS).into());
    }
    let fields = space.classes.get_layout(o.class).map_or(0, |l| l.slot_count());
    if fields > 0 {
        let path = format!(
            "{} {}",
            space.classes.get_layout(o.class).map_or_else(String::new, |l| l.path().as_str().to_string()),
            space.names.text(o.name)
        );
        return Err(NotYet::not_read(
            format!(
                "the reference ({path}) stays a stand-in the definition store does not load (an object inside a \
                 class default object is one), so its fields cannot be read"
            ),
            CHAINS,
        )
        .into());
    }
    Ok(())
}

/// The class path of `obj`, for messages.
pub fn class_path(space: &ObjectSpace, obj: ObjRef) -> String {
    match space.heap.get(obj) {
        Ok(o) => match space.classes.get_layout(o.class) {
            Some(l) => l.path().as_str().to_string(),
            None => format!("{:?}", o.class),
        },
        Err(_) => format!("{obj:?}"),
    }
}


/// The heap's committed-write count and the store's load-transition count (the hub's tracker design, review r2 of
/// #523 and #518). RNG draws note themselves at the draw (no draw counter here).
fn counters(cx: &mut EvalCx<'_>) -> (u64, u64) {
    let WorldParts { space, defs, .. } = cx.world.parts();
    (space.heap.committed_writes(), defs.as_ref().map_or(0, |d| d.load_transitions()))
}

/// One write tracker for a whole Off operation (review r2 of #523, the hub's ruling, in #518's shape: a definition
/// load, a committed heap change or an RNG draw is a write). The outermost scope opens a heap change watch
/// records the heap's committed-write count and the store's load-transition count and starts a fresh write count;
/// nested scopes share it. When the outermost scope closes, a moved counter is a write; a draw notes itself
/// ([`random_between`], the random resolver), and a resolver that writes notes itself too. A `NotYet` leaving the scope after a write is prefixed once, kind kept; any other error,
/// and everything under Reference, is unchanged.
pub(crate) fn tracked<T>(
    cx: &mut EvalCx<'_>,
    op: impl FnOnce(&mut EvalCx<'_>) -> Result<T, AttrError>,
) -> Result<T, AttrError> {
    if parity(cx) != ow_core::ParityMode::Off {
        return op(cx);
    }
    let start = cx.outermost().then(|| counters(cx));
    cx.enter_fold();
    let result = op(cx);
    cx.leave_fold();
    if let Some(start) = start
        && counters(cx) != start
    {
        cx.note_writes();
    }
    match result {
        Err(e) if cx.wrote() => Err(after_partial_writes(e)),
        other => other,
    }
}

/// The full path of `obj` for messages, by its outers only (no load): names joined with `.`, and `:` before an object
/// whose outer is not a package but whose outer's outer is (the subobject separator, as `Object.PathName` writes it).
/// An object that is not live is its handle's debug text.
pub fn object_path(space: &ObjectSpace, obj: ObjRef) -> String {
    let mut links = Vec::new();
    let mut at = Some(obj);
    while let Some(o) = at {
        let Ok(x) = space.heap.get(o) else {
            return format!("{obj:?}");
        };
        let package = space.classes.get_layout(x.class).is_some_and(|l| l.path().as_str() == "Core.Package");
        links.push((space.names.text(x.name).into_owned(), package));
        if links.len() > 64 {
            break;
        }
        at = x.outer;
    }
    links.reverse();
    let mut out = String::new();
    for (i, (name, _)) in links.iter().enumerate() {
        if i > 0 {
            let sub = !links[i - 1].1 && i >= 2 && links[i - 2].1;
            out.push(if sub { ':' } else { '.' });
        }
        out.push_str(name);
    }
    out
}

/// Property `name` of `obj` (element 0), loaded first; `Ok(None)` when the object's class (its whole super chain)
/// has no property of that name.
pub fn property(cx: &mut EvalCx<'_>, obj: ObjRef, name: &str) -> Result<Option<Value>, AttrError> {
    ensure_loaded(cx, obj)?;
    let space = cx.world.parts().space;
    let class = space.heap.get(obj)?.class;
    let Some(p) = space.prop(class, name) else {
        return Ok(None);
    };
    Ok(Some(space.read(obj, PropAddr::scalar(p))?.clone()))
}

/// Property `name` of `obj`, which the class must have (`BadData` naming both otherwise).
pub fn required(cx: &mut EvalCx<'_>, obj: ObjRef, name: &str) -> Result<Value, AttrError> {
    match property(cx, obj, name)? {
        Some(v) => Ok(v),
        None => {
            let class = class_path(cx.world.parts().space, obj);
            Err(AttrError::BadData {
                reason: format!("{class} has no property {name}").into(),
            })
        }
    }
}

/// Field `name` of the struct value `v`.
pub fn field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Result<&'v Value, AttrError> {
    let bad = |reason: String| AttrError::BadData {
        reason: reason.into(),
    };
    let s = v.as_struct().ok_or_else(|| {
        bad(format!(
            "expected a struct with field {name}, found {}",
            v.kind_name()
        ))
    })?;
    let layout = space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| bad(format!("struct {:?} has no layout", s.ty)))?;
    let entry = space
        .names
        .find(name)
        .and_then(|n| layout.field(n))
        .map(|(entry, _)| entry)
        .ok_or_else(|| {
            bad(format!(
                "struct {} has no field {name}",
                layout.path().as_str()
            ))
        })?;
    usize::try_from(entry)
        .ok()
        .and_then(|i| s.fields.get(i))
        .ok_or_else(|| {
            bad(format!(
                "struct {} value lacks field {name}",
                layout.path().as_str()
            ))
        })
}

fn wrong(what: &str, v: &Value) -> AttrError {
    AttrError::BadData {
        reason: format!(
            "{what}: expected a number, a flag, a name or an object, found {}",
            v.kind_name()
        )
        .into(),
    }
}

/// A stored number as a float: a float as it is, an int or a byte converted (NATIVE_ATTRIBUTES.md section 3).
pub fn number(what: &str, v: &Value) -> Result<f32, AttrError> {
    match v {
        Value::Float(f) => Ok(*f),
        // int -> float rounds to nearest, as the C++ float(int) and the native's conversion.
        Value::Int(i) => Ok(*i as f32),
        Value::Byte(b) => Ok(f32::from(*b)),
        other => Err(wrong(what, other)),
    }
}

pub fn flag(what: &str, v: &Value) -> Result<bool, AttrError> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(wrong(what, other)),
    }
}

pub(crate) fn byte(what: &str, v: &Value) -> Result<u8, AttrError> {
    match v {
        Value::Byte(b) => Ok(*b),
        other => Err(wrong(what, other)),
    }
}

pub(crate) fn name(what: &str, v: &Value) -> Result<NameId, AttrError> {
    match v {
        Value::Name(n) => Ok(*n),
        other => Err(wrong(what, other)),
    }
}

/// An object reference; `Value::None` reads as no object.
pub fn object(what: &str, v: &Value) -> Result<Option<ObjRef>, AttrError> {
    match v {
        Value::Object(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(wrong(what, other)),
    }
}

/// The elements of an array value.
pub fn elements<'v>(what: &str, v: &'v Value) -> Result<&'v [Value], AttrError> {
    v.elements().ok_or_else(|| AttrError::BadData {
        reason: format!("{what}: expected an array, found {}", v.kind_name()).into(),
    })
}

/// An `AttributeInitializationData` struct value as [`InitData`].
pub fn init_data(space: &ObjectSpace, v: &Value) -> Result<InitData, AttrError> {
    Ok(InitData {
        base_value_constant: number("BaseValueConstant", field(space, v, "BaseValueConstant")?)?,
        base_value_attribute: object("BaseValueAttribute", field(space, v, "BaseValueAttribute")?)?,
        initialization_definition: object(
            "InitializationDefinition",
            field(space, v, "InitializationDefinition")?,
        )?,
        base_value_scale_constant: number(
            "BaseValueScaleConstant",
            field(space, v, "BaseValueScaleConstant")?,
        )?,
    })
}

/// Evaluates the `AttributeInitializationData` struct value `v` (the field `name` of `holder`, for messages).
pub(crate) fn evaluate_value(
    sys: &AttributeSystem,
    v: &Value,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let data = init_data(cx.world.parts().space, v)?;
    evaluate_init(sys, &data, source, override_source, cx)
}

/// Field `name` of the struct `v`, cloned (so no borrow of the space outlives the read).
fn field_of(cx: &mut EvalCx<'_>, v: &Value, name: &str) -> Result<Value, AttrError> {
    Ok(field(cx.world.parts().space, v, name)?.clone())
}

/// Evaluates field `name` of the struct `v` as `AttributeInitializationData`.
fn evaluate_field(
    sys: &AttributeSystem,
    v: &Value,
    name: &str,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let data = field_of(cx, v, name)?;
    evaluate_value(sys, &data, source, override_source, cx)
}

/// `ConditionalInitializationExpressions` (the `ConditionalInitialization` of a definition, the `ValueExpressions`
/// of a conditional resolver): the `BaseValueIfTrue` of the first `ConditionalExpressionList` entry whose
/// `Expressions` all hold, else `DefaultBaseValue`, evaluated with the same sources (NATIVE_PROGRESSION.md section
/// 1; NATIVE_ATTRIBUTES.md section 1: "the same 'all' evaluation as `EvaluateExpressions` in And mode"). The caller
/// decides whether `bEnabled` is consulted. Status: UNVERIFIED.
pub(crate) fn conditional_value(
    sys: &AttributeSystem,
    conditional: &Value,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let list = field_of(cx, conditional, "ConditionalExpressionList")?;
    for entry in elements("ConditionalExpressionList", &list)? {
        let exprs = {
            let space = cx.world.parts().space;
            crate::expr::read_expressions(space, field(space, entry, "Expressions")?)?
        };
        if crate::expr::expressions_hold(sys, &exprs, ExprMode::And, source, override_source, cx)? {
            return evaluate_field(sys, entry, "BaseValueIfTrue", source, override_source, cx);
        }
    }
    evaluate_field(
        sys,
        conditional,
        "DefaultBaseValue",
        source,
        override_source,
        cx,
    )
}

/// Whether `attr` is an `AttributeDefinitionMultiContext` (or a subclass), whose context expansion is not read.
fn is_multi_context(space: &ObjectSpace, attr: ObjRef) -> Result<bool, AttrError> {
    let class = space.heap.get(attr)?.class;
    Ok(match space.classes.by_path(MULTI_CONTEXT_CLASS) {
        Some(multi) => space.classes.is_child_of(class, multi),
        None => false,
    })
}

fn multi_context_not_read(what: &str) -> AttrError {
    NotYet::not_read(
        format!(
            "{what} of an AttributeDefinitionMultiContext (its MultiContextResolver expansion)"
        ),
        "NATIVE_ATTRIBUTES.md sections 1 and Not read yet",
    )
    .into()
}

// ---------------------------------------------------------------------------------------------------------------
// The chains
// ---------------------------------------------------------------------------------------------------------------

/// A chain entry loaded before the walk reads or dispatches it (lane IMPL-RESOLVER-CHAIN-LOADS; NATIVE_TARGETABLE_
/// RESOLVER.md section 5.1, the hub's ruling on #787), **in `ParityMode::Off` only**: Reference keeps main's
/// behaviour, loading nothing here (review of #791, item 1). The port's store is lazy, so a resident definition's
/// chain can still hold unloaded handles; the game's package load has them resident with it, by fit (UNVERIFIED). The
/// load is [`ensure_loaded`]'s, so a load-free scope refuses it as before (`NotRead` naming the entry by its role).
/// A load that fails leaves the handle in place, and the walk's residency check ([`resident`]) answers the store's
/// `BadData` for it here, where the entry is reached.
fn load_entry(cx: &mut EvalCx<'_>, entry: ObjRef, what: &str) -> Result<(), AttrError> {
    if parity(cx) != ow_core::ParityMode::Off {
        return Ok(());
    }
    match ensure_loaded_as(cx, entry, what) {
        Ok(()) => Ok(()),
        Err(e) => {
            let WorldParts { space, defs, .. } = cx.world.parts();
            let failed = defs.as_deref().is_some_and(|d| d.is_failed(space, entry));
            if failed {
                resident(cx, entry, what)?;
            }
            Err(e)
        }
    }
}

/// [`AttributeSystem::resolve_context`]: the `ContextResolverChain` left to right, each resolver receiving what the
/// previous returned (the first receives `source`); a None entry is skipped; a resolver answering None ends the walk
/// with None; an empty chain (or one of None entries only) is None. Each entry is loaded before it is dispatched
/// ([`load_entry`]). Status: UNVERIFIED (NATIVE_ATTRIBUTES.md section 1, high confidence).
pub(crate) fn resolve_context(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<ObjRef>, AttrError> {
    let _guard = Guard::enter(Step::Context { attr, source })?;
    let chain = required(cx, attr, "ContextResolverChain")?;
    let mut current = source;
    let mut answered = false;
    for entry in elements("ContextResolverChain", &chain)? {
        let Some(resolver) = object("ContextResolverChain entry", entry)? else {
            continue;
        };
        load_entry(cx, resolver, "a context resolver")?;
        let r = sys.context_resolver(cx.world.parts().space, resolver)?;
        match r.resolve(sys, resolver, attr, current, cx)? {
            Some(context) => current = Some(context),
            None => return Ok(None),
        }
        answered = true;
    }
    Ok(if answered { current } else { None })
}

/// [`AttributeSystem::resolve_contexts`]: an `AttributeDefinition` gives its one context when it resolves; an
/// `AttributeDefinitionMultiContext` is `NotYet { NotRead }` (its expansion is not read).
pub(crate) fn resolve_contexts(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Vec<ObjRef>, AttrError> {
    ensure_loaded(cx, attr)?;
    if is_multi_context(cx.world.parts().space, attr)? {
        return Err(multi_context_not_read("ResolveContexts"));
    }
    Ok(resolve_context(sys, attr, source, cx)?
        .into_iter()
        .collect())
}

/// The context `GetValue` uses: the override source's when the override is given and resolves, else the source's.
/// [`AttributeSystem::preflight_read`]: what a read can refuse before it evaluates anything (review of #442). The
/// definition is loaded, and must not be multi-context. Its chain entries are not loaded here (the hub's ruling on
/// review of #791, item 2): the class and availability checks run on the handles, so a refusal here comes before any
/// load or write. The walks load each entry ([`load_entry`], Off), after this preflight: a read through `GetValue`
/// loads the definition and each of its N unloaded chain entries, 1+N loads, and #518's `load_transitions` rises by as
/// many (it was 1); a refusal from those walks follows the loads and is prefixed by the counters.
/// Every resolver of its context chain must be a class this library implements. Every entry of its value chain must be
/// set and a class it implements. Nothing is resolved or evaluated, so no generator is drawn and no resolver writes. A
/// refusal from deeper in a read (a nested attribute, a resolver's own data) can only be found by evaluating, and is
/// not covered here.
pub(crate) fn preflight_read(sys: &AttributeSystem, attr: ObjRef, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
    ensure_loaded(cx, attr)?;
    if is_multi_context(cx.world.parts().space, attr)? {
        return Err(multi_context_not_read("GetValue"));
    }
    let chain = required(cx, attr, "ContextResolverChain")?;
    for entry in elements("ContextResolverChain", &chain)? {
        if let Some(resolver) = object("ContextResolverChain entry", entry)? {
            sys.context_resolver(cx.world.parts().space, resolver)?;
        }
    }
    let chain = required(cx, attr, "ValueResolverChain")?;
    for entry in elements("ValueResolverChain", &chain)? {
        let Some(resolver) = object("ValueResolverChain entry", entry)? else {
            return Err(NotYet::not_read("a None entry in a ValueResolverChain on a read", CHAINS).into());
        };
        sys.value_resolver(cx.world.parts().space, resolver)?.available(resolver, cx)?;
    }
    Ok(())
}

/// [`AttributeSystem::preflight_read_resident`]: [`preflight_read`]'s checks, loading nothing (review r1 of #573, item
/// 2; under #518 a lazy load is a write). The definition and every resolver object of its two chains must be
/// resident: a handle the definition store has not loaded is `NotYet { NotRead }` naming it, and one whose load failed
/// is `BadData`. Then, as [`preflight_read`]: not multi-context, every context resolver class implemented, every value
/// chain entry set and of an implemented class available in this mode, asked through
/// [`crate::api::ValueResolver::availability`] with shared references only (review r2 of #573). Nothing is loaded,
/// resolved or evaluated, and no resolver code that gets the mutable context runs.
pub(crate) fn preflight_read_resident(sys: &AttributeSystem, attr: ObjRef, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
    resident(cx, attr, "the attribute definition")?;
    if is_multi_context(cx.world.parts().space, attr)? {
        return Err(multi_context_not_read("GetValue"));
    }
    let chain = resident_field(cx, attr, "ContextResolverChain")?;
    for entry in elements("ContextResolverChain", &chain)? {
        if let Some(resolver) = object("ContextResolverChain entry", entry)? {
            resident(cx, resolver, "a context resolver")?;
            sys.context_resolver(cx.world.parts().space, resolver)?;
        }
    }
    let chain = resident_field(cx, attr, "ValueResolverChain")?;
    for entry in elements("ValueResolverChain", &chain)? {
        let Some(resolver) = object("ValueResolverChain entry", entry)? else {
            return Err(NotYet::not_read("a None entry in a ValueResolverChain on a read", CHAINS).into());
        };
        // The dispatch first: a class this library does not implement is refused by its name, resident or not.
        let r = sys.value_resolver(cx.world.parts().space, resolver)?;
        // Then the mode availability, as metadata with shared references only: the resolver's own `available`, which
        // gets the mutable context, is never called here (review r2 of #573). It comes before the residency, so a
        // class one mode does not implement is refused by its name, resident or not, as an unregistered class is
        // (review r2 of #584, item 1).
        let WorldParts { space, defs, .. } = cx.world.parts();
        let mode = defs.as_ref().map_or(ow_core::ParityMode::Off, |d| d.mode());
        r.availability(space, resolver, mode)?;
        resident(cx, resolver, "a value resolver")?;
    }
    Ok(())
}

/// `obj` as a read may use it without a load: `NotYet { NotRead }` for a handle the store has not loaded, `BadData`
/// for one whose load failed. Loads nothing.
fn resident(cx: &mut EvalCx<'_>, obj: ObjRef, what: &str) -> Result<(), AttrError> {
    let WorldParts { space, defs, .. } = cx.world.parts();
    if let Some(defs) = defs {
        let name = || format!("{} {}", class_path(space, obj), space.heap.get(obj).map_or_else(|_| "?".into(), |o| space.names.text(o.name).into_owned()));
        if defs.is_failed(space, obj) {
            return Err(AttrError::BadData { reason: format!("{what} ({}) failed to load", name()).into() });
        }
        if defs.is_unloaded(space, obj) {
            return Err(NotYet::not_read(format!("{what} ({}) is not loaded (a load is a write)", name()), CHAINS).into());
        }
    }
    Ok(())
}

/// Property `name` of the resident `obj`, read without a load (`BadData` when its class has none).
pub(crate) fn resident_field(cx: &mut EvalCx<'_>, obj: ObjRef, name: &str) -> Result<Value, AttrError> {
    let space = cx.world.parts().space;
    let class = space.heap.get(obj)?.class;
    match space.prop(class, name) {
        Some(p) => Ok(space.read(obj, PropAddr::scalar(p))?.clone()),
        None => Err(AttrError::BadData { reason: format!("{} has no property {name}", class_path(space, obj)).into() }),
    }
}

/// The mark of a refusal after committed writes (the hub's ruling, the same text the script-native wrappers use).
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

/// A `NotYet` met after a resolver of this evaluation committed writes is terminal: its text is prefixed with
/// [`PARTIAL_WRITES`] at the very start (once), its kind and record kept. Any other error is returned unchanged.
pub fn after_partial_writes(e: AttrError) -> AttrError {
    match e {
        AttrError::NotYet(n) if !n.what.starts_with(PARTIAL_WRITES) => {
            NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into()
        }
        other => other,
    }
}

pub(crate) fn value_context(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<ObjRef>, AttrError> {
    ensure_loaded(cx, attr)?;
    if is_multi_context(cx.world.parts().space, attr)? {
        return Err(multi_context_not_read("GetValue"));
    }
    if let Some(o) = override_source
        && let Some(context) = resolve_context(sys, attr, Some(o), cx)?
    {
        return Ok(Some(context));
    }
    resolve_context(sys, attr, source, cx)
}

fn read_of(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    base: bool,
    cx: &mut EvalCx<'_>,
) -> Result<Option<AttrValue>, AttrError> {
    match value_context(sys, attr, source, override_source, cx)? {
        None => Ok(None),
        Some(context) => Ok(Some(AttrValue {
            context,
            value: fold(sys, attr, context, base, cx)?,
        })),
    }
}

/// [`AttributeSystem::value_of`]: `GetValue`, the override tried first. Status: UNVERIFIED (NATIVE_ATTRIBUTES.md
/// section 1).
pub(crate) fn value_of(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<AttrValue>, AttrError> {
    read_of(sys, attr, source, override_source, false, cx)
}

/// [`AttributeSystem::base_value_of`]: `GetBaseValue`, each resolver's base-value method.
pub(crate) fn base_value_of(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<AttrValue>, AttrError> {
    read_of(sys, attr, source, override_source, true, cx)
}

/// The world's parity mode: its store's. A world without a store runs the Rust rules (`Off`). The one lookup the
/// resolvers share (lane FIX-DESIGNER-RESOLVER, review r1 of PR #360).
pub(crate) fn parity(cx: &mut EvalCx<'_>) -> ow_core::ParityMode {
    cx.world
        .parts()
        .defs
        .map_or(ow_core::ParityMode::Off, |d| d.mode())
}

/// The class of a designer attribute (the C++ reads its `BaseValue` as its value; [`fold`]).
const DESIGNER_DEFINITION: &str = "WillowGame.DesignerAttributeDefinition";

/// The value chain on a resolved context: one fold, each resolver receiving the previous result (0.0 first); the
/// last result is the value; an empty chain is 0.0. A None entry in a value chain is not read (the record says what
/// a None entry does only for add and remove), so it is `NotYet { NotRead }`.
///
/// Every exit goes through the partial-write rule (review r3 of #458, item 1): once a resolver of the current outer
/// evaluation committed writes, any `NotYet` leaving the fold is terminal and prefixed, its kind kept.
fn fold(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: ObjRef,
    base: bool,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    if parity(cx) == ow_core::ParityMode::Off {
        return tracked(cx, |cx| fold_chain(sys, attr, context, base, cx));
    }
    cx.enter_fold();
    let result = fold_chain(sys, attr, context, base, cx);
    cx.leave_fold();
    match result {
        Err(e) if cx.wrote() => Err(after_partial_writes(e)),
        other => other,
    }
}

fn fold_chain(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: ObjRef,
    base: bool,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let _guard = Guard::enter(Step::Value {
        attr,
        context,
        base,
    })?;
    // The C++ stand-in, `ParityMode::Reference` only (lane FIX-DESIGNER-RESOLVER, decision 2026-11-16-62): a designer
    // attribute is its own `BaseValue`, evaluated on the context (`src/progression.cpp:117`; the game's designers' code
    // sets it at run time). Off reads the value chain on the designer object the context resolver answered.
    let reference = parity(cx) == ow_core::ParityMode::Reference;
    if reference && class_path(cx.world.parts().space, attr) == DESIGNER_DEFINITION {
        let data = required(cx, attr, "BaseValue")?;
        let data = init_data(cx.world.parts().space, &data)?;
        return evaluate_init(sys, &data, Some(context), None, cx);
    }
    let chain = required(cx, attr, "ValueResolverChain")?;
    let mut previous = 0.0f32;
    for entry in elements("ValueResolverChain", &chain)? {
        let Some(resolver) = object("ValueResolverChain entry", entry)? else {
            return Err(
                NotYet::not_read("a None entry in a ValueResolverChain on a read", CHAINS).into(),
            );
        };
        // Loaded before its dispatch (`load_entry`, lane IMPL-RESOLVER-CHAIN-LOADS).
        load_entry(cx, resolver, "a value resolver")?;
        // A refusal after a resolver of this evaluation committed writes is terminal and prefixed (review r1 of #458,
        // item 4): `fold` marks it on the way out, whichever API the read came through.
        let r = sys.value_resolver(cx.world.parts().space, resolver)?;
        previous = if base {
            r.base_value(sys, resolver, context, previous, cx)?
        } else {
            r.value(sys, resolver, context, previous, cx)?
        };
    }
    Ok(previous)
}

/// [`AttributeSystem::value_from_context`]: `GetValueFromContext`, 0.0 for a None context.
pub(crate) fn value_from_context(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    match context {
        None => Ok(0.0),
        Some(c) => fold(sys, attr, c, false, cx),
    }
}

/// [`AttributeSystem::base_value_from_context`]: `GetBaseValueFromContext`, 0.0 for a None context.
pub(crate) fn base_value_from_context(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    match context {
        None => Ok(0.0),
        Some(c) => fold(sys, attr, c, true, cx),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The evaluator
// ---------------------------------------------------------------------------------------------------------------

/// [`AttributeSystem::evaluate_init`]: `EvaluateInitializationData` (NATIVE_PROGRESSION.md section 1, re-read in
/// NATIVE_ATTRIBUTES.md section 1), in this order:
///
/// 1. the base: `BaseValueConstant`, replaced by `BaseValueAttribute`'s value when that attribute resolves (the
///    override tried first);
/// 2. with an `InitializationDefinition`: its `ValueFormula` (`Multiplier * (Level ^ Power + Offset)`, the terms in
///    that order) or else its `ConditionalInitialization` gives `f`, combined with the base by `BaseValueMode`; then
///    an enabled `RandomVariance` is added;
/// 3. times `BaseValueScaleConstant`;
/// 4. with a definition: `RangeRestriction` minimum, then maximum (each when enabled), then `RoundingMode`.
///
/// Single-precision arithmetic, one operation at a time (architecture 5.1 N4). Status: UNVERIFIED; the module
/// documentation lists where the two C++ evaluators disagree and which form this follows.
pub(crate) fn evaluate_init(
    sys: &AttributeSystem,
    data: &InitData,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let mut base = data.base_value_constant;
    if let Some(attr) = data.base_value_attribute
        && let Some(v) = value_of(sys, attr, source, override_source, cx)?
    {
        base = v.value;
    }
    let Some(definition) = data.initialization_definition else {
        return Ok(base * data.base_value_scale_constant);
    };
    let _guard = Guard::enter(Step::Definition {
        definition,
        source,
        override_source,
    })?;

    let [formula, conditional, variance, range] = definition_parts(cx, definition)?;
    let (mode, rounding) = definition_modes(cx, definition)?;

    let enabled = |cx: &mut EvalCx<'_>, v: &Value, name: &str| -> Result<bool, AttrError> {
        flag(name, &field_of(cx, v, name)?)
    };
    let f = if enabled(cx, &formula, "bEnabled")? {
        let mut terms = [0.0f32; 4];
        for (term, name) in terms
            .iter_mut()
            .zip(["Multiplier", "Level", "Power", "Offset"])
        {
            *term = evaluate_field(sys, &formula, name, source, override_source, cx)?;
        }
        let [m, l, p, o] = terms;
        Some(math::formula(m, l, p, o, cx.forms.pow))
    } else if enabled(cx, &conditional, "bEnabled")? {
        Some(conditional_value(
            sys,
            &conditional,
            source,
            override_source,
            cx,
        )?)
    } else {
        None
    };
    let mut value = match f {
        Some(f) => math::combine_base(mode, base, f),
        None => base,
    };
    if enabled(cx, &variance, "bEnabled")? {
        let low = evaluate_field(sys, &variance, "LowerBound", source, override_source, cx)?;
        let high = evaluate_field(sys, &variance, "UpperBound", source, override_source, cx)?;
        let integer = enabled(cx, &variance, "bUseIntegerRandomization")?;
        value += random_between(low, high, integer, cx)?;
    }
    value *= data.base_value_scale_constant;

    let min = if enabled(cx, &range, "bEnableMinValueRestriction")? {
        Some(evaluate_field(
            sys,
            &range,
            "MinValue",
            source,
            override_source,
            cx,
        )?)
    } else {
        None
    };
    let max = if enabled(cx, &range, "bEnableMaxValueRestriction")? {
        Some(evaluate_field(
            sys,
            &range,
            "MaxValue",
            source,
            override_source,
            cx,
        )?)
    } else {
        None
    };
    Ok(round_mode(math::restrict(value, min, max), rounding))
}

/// The four struct parts of the initialization definition `definition`, in [`evaluate_init`]'s order: `ValueFormula`,
/// `ConditionalInitialization`, `RandomVariance` and `RangeRestriction`. Reads only.
fn definition_parts(cx: &mut EvalCx<'_>, definition: ObjRef) -> Result<[Value; 4], AttrError> {
    Ok([
        required(cx, definition, "ValueFormula")?,
        required(cx, definition, "ConditionalInitialization")?,
        required(cx, definition, "RandomVariance")?,
        required(cx, definition, "RangeRestriction")?,
    ])
}

/// The initialization definition's `BaseValueMode` and `RoundingMode`; one outside its enum is a data error. Reads
/// only.
fn definition_modes(cx: &mut EvalCx<'_>, definition: ObjRef) -> Result<(BaseValueMode, RoundingMode), AttrError> {
    let mode = byte("BaseValueMode", &required(cx, definition, "BaseValueMode")?)?;
    let rounding = byte("RoundingMode", &required(cx, definition, "RoundingMode")?)?;
    let bad = |what: &str, raw: u8| AttrError::BadData {
        reason: format!("{what} {raw} is outside the enum").into(),
    };
    let mode =
        BaseValueMode::from_raw(i64::from(mode)).ok_or_else(|| bad("BaseValueMode", mode))?;
    let rounding =
        RoundingMode::from_value(rounding).ok_or_else(|| bad("RoundingMode", rounding))?;
    Ok((mode, rounding))
}

/// What evaluating the `AttributeInitializationData` value `v` can refuse before it evaluates anything (lane
/// FIX-483-FOLLOWUP, F1; deepened in review r1 of #518): every field the evaluation can reach, checked without
/// evaluating it, so no resolver writes, no noun entry is made and no generator is drawn:
/// - the struct's four fields, as [`init_data`] reads them;
/// - a `BaseValueAttribute`: [`preflight_read`] (its definition loaded, its chains' classes known);
/// - an `InitializationDefinition`: loaded, its four parts present, its two modes inside their enums, and then, as
///   [`evaluate_init`] would reach them, the enabled formula's four terms, or else the enabled conditional's every
///   entry (each expression's two attribute operands through [`preflight_read`], its `BaseValueIfTrue`) and its
///   `DefaultBaseValue`; the enabled variance's two bounds and its integer flag; the enabled range limits. Each term
///   is itself initialization data, checked the same way.
///
/// A conditional entry is checked whether or not an earlier one would hold, since which one holds is only known by
/// evaluating, except where it is known without evaluating: an entry with no expressions always holds, so the entries
/// after it and `DefaultBaseValue` are never reached and are not checked (review r2 of #518, item 5).
///
/// Each definition level enters the evaluator's own guard ([`MAX_DEPTH`] deep, [`MAX_WORK`] steps, a definition already
/// in progress refused as a cycle), so a long or cyclic chain is the same `BadData` the evaluation would give, not a
/// stack overflow; a definition already checked whole is not checked again (review r2 of #518, item 1).
/// What stays out of reach of a check: a resolver's own data read only when it runs (a context resolver's answer, a
/// value resolver's property). A definition load is not a write under NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md's
/// ruling; a caller that counts loads as writes (the hub's ruling in review r1 of #518) tracks them itself.
pub(crate) fn preflight_init(sys: &AttributeSystem, v: &Value, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
    let mut done = ow_core::DetSet::new();
    preflight_init_in(sys, v, cx, &mut done)
}

fn preflight_init_in(
    sys: &AttributeSystem,
    v: &Value,
    cx: &mut EvalCx<'_>,
    seen: &mut ow_core::DetSet<ObjRef>,
) -> Result<(), AttrError> {
    let data = init_data(cx.world.parts().space, v)?;
    if let Some(attr) = data.base_value_attribute {
        preflight_read(sys, attr, cx)?;
    }
    let Some(definition) = data.initialization_definition else {
        return Ok(());
    };
    if seen.contains(&definition) {
        return Ok(());
    }
    // The evaluator's depth, work and cycle budgets, held while this definition's terms are checked.
    let _guard = Guard::enter(Step::Definition {
        definition,
        source: None,
        override_source: None,
    })?;
    ensure_loaded(cx, definition)?;
    let [formula, conditional, variance, range] = definition_parts(cx, definition)?;
    definition_modes(cx, definition)?;
    let enabled = |cx: &mut EvalCx<'_>, v: &Value, name: &str| -> Result<bool, AttrError> {
        flag(name, &field_of(cx, v, name)?)
    };
    let term = |cx: &mut EvalCx<'_>, v: &Value, name: &str, seen: &mut ow_core::DetSet<ObjRef>| -> Result<(), AttrError> {
        let t = field_of(cx, v, name)?;
        preflight_init_in(sys, &t, cx, seen)
    };
    if enabled(cx, &formula, "bEnabled")? {
        for name in ["Multiplier", "Level", "Power", "Offset"] {
            term(cx, &formula, name, seen)?;
        }
    } else if enabled(cx, &conditional, "bEnabled")? {
        let list = field_of(cx, &conditional, "ConditionalExpressionList")?;
        for entry in elements("ConditionalExpressionList", &list)? {
            let exprs = {
                let space = cx.world.parts().space;
                crate::expr::read_expressions(space, field(space, entry, "Expressions")?)?
            };
            for e in &exprs {
                for attr in [e.attribute_operand1, e.attribute_operand2].into_iter().flatten() {
                    preflight_read(sys, attr, cx)?;
                }
            }
            term(cx, entry, "BaseValueIfTrue", seen)?;
            if exprs.is_empty() {
                // This entry always holds: nothing after it is reached.
                seen.insert(definition);
                return range_and_variance(cx, &variance, &range, seen, &term);
            }
        }
        term(cx, &conditional, "DefaultBaseValue", seen)?;
    }
    range_and_variance(cx, &variance, &range, seen, &term)?;
    seen.insert(definition);
    Ok(())
}

/// The check of one initialization-data term of [`preflight_init_in`].
type Term<'a> = dyn Fn(&mut EvalCx<'_>, &Value, &str, &mut ow_core::DetSet<ObjRef>) -> Result<(), AttrError> + 'a;

/// The variance bounds and range limits of [`preflight_init_in`], each when enabled.
fn range_and_variance(
    cx: &mut EvalCx<'_>,
    variance: &Value,
    range: &Value,
    seen: &mut ow_core::DetSet<ObjRef>,
    term: &Term<'_>,
) -> Result<(), AttrError> {
    let enabled = |cx: &mut EvalCx<'_>, v: &Value, name: &str| -> Result<bool, AttrError> {
        flag(name, &field_of(cx, v, name)?)
    };
    if enabled(cx, variance, "bEnabled")? {
        term(cx, variance, "LowerBound", seen)?;
        term(cx, variance, "UpperBound", seen)?;
        enabled(cx, variance, "bUseIntegerRandomization")?;
    }
    if enabled(cx, range, "bEnableMinValueRestriction")? {
        term(cx, range, "MinValue", seen)?;
    }
    if enabled(cx, range, "bEnableMaxValueRestriction")? {
        term(cx, range, "MaxValue", seen)?;
    }
    Ok(())
}

/// A random amount from `low` to `high`, drawn from `AttrWorld::rng(CrtStream::AttributeVariance)`: one `rand15`
/// draw `d = r / 32768` (so `0 <= d < 1`); a real amount is `low + f32(d) * (high - low)`; an integer amount is
/// `ceil(low) + floor(d * span)` with `span = floor(high) - ceil(low) + 1` (both ends reachable), or `ceil(low)`
/// when the span is below 1. The bounds are evaluated before the draw.
///
/// Status: UNVERIFIED. The record reads "a random amount between `LowerBound` and `UpperBound` is added,
/// integer-uniform when `bUseIntegerRandomization`"; the mapping of the draw onto the span is `progression.cpp`
/// `randomBetween`'s (the only oracle; the native generator's mapping is unread), used in both parity modes as a
/// reading, like the CrtRand script-stream convention. The live item game-stage offset reaching both ends fits the
/// inclusive integer reading.
pub(crate) fn random_between(
    low: f32,
    high: f32,
    integer: bool,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    // A draw is a write in Off (review r2 of #523, the hub's ruling).
    if parity(cx) == ow_core::ParityMode::Off {
        cx.note_writes();
    }
    let draw = f64::from(cx.world.rng(CrtStream::AttributeVariance)?.rand15()) / 32768.0;
    if !integer {
        // progression.cpp: low + float(draw) * (high - low), single precision.
        return Ok(low + (draw as f32) * (high - low));
    }
    // progression.cpp: the span and the pick in double, the result rounded to single.
    let first = f64::from(low).ceil();
    let span = f64::from(high).floor() - first + 1.0;
    if span < 1.0 {
        return Ok(first as f32);
    }
    Ok((first + (draw * span).floor()) as f32)
}

// ---------------------------------------------------------------------------------------------------------------
// The SetBaseValue preflight (lane IMPL-SET-BASE-VALUE, review r1 of #572)
// ---------------------------------------------------------------------------------------------------------------

/// The refusal for a definition handle the preflight would have to load (a load is a write; #518).
pub const PREFLIGHT_UNLOADED: &str = "a definition the SetBaseValue preflight reaches is not loaded (a load is a write)";

/// Whether `obj` is one of the store's unloaded handles (never loads it).
fn is_unloaded_handle(cx: &mut EvalCx<'_>, obj: ObjRef) -> bool {
    let WorldParts { space, defs, .. } = cx.world.parts();
    defs.is_some_and(|d| d.is_unloaded(space, obj))
}

fn refuse_unloaded(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<(), AttrError> {
    if is_unloaded_handle(cx, obj) {
        let path = class_path(cx.world.parts().space, obj);
        return Err(NotYet::not_read(format!("{PREFLIGHT_UNLOADED}: an object of class {path}"), CHAINS).into());
    }
    Ok(())
}

/// How deep the preflight follows nested initialisation data before it refuses (a port bound, not read).
const PREFLIGHT_DEPTH: usize = 32;

/// [`AttributeSystem::preflight_set_base_value`]: what `SetBaseValue(dest, data, ...)` can refuse before anything is
/// evaluated or written, loading nothing:
/// - every definition the call reaches must already be loaded: the destination, its resolvers, and the definitions
///   and attributes the initialisation data names, followed into nested data, conditional expressions included;
/// - the destination must not be multi-context (the refusal `set_base` would give), and its context and value
///   resolver classes must be implemented (the value chain is checked up to its first None entry, where the set
///   stops);
/// - every attribute the evaluation reads must pass the read preflight's class checks (no multi-context, implemented
///   resolver classes, no None value-chain entry).
///
/// Nothing is resolved, evaluated or drawn. A refusal from a resolver's own data can only be found by running it.
pub(crate) fn preflight_set_base_value(
    sys: &AttributeSystem,
    dest: ObjRef,
    data: &InitData,
    cx: &mut EvalCx<'_>,
) -> Result<(), AttrError> {
    let mut refuse = |cx: &mut EvalCx<'_>, obj: ObjRef| refuse_unloaded(cx, obj).map(|()| true);
    let mut walk = SetBaseWalk { visit: &mut refuse, checks: true };
    check_definition(sys, dest, true, cx, &mut walk)?;
    check_init(sys, data, 0, cx, &mut walk)
}

/// The walk of [`preflight_set_base_value`] with `visit` in place of its refusal of an unloaded handle (decision
/// 2026-11-26-NNN, NATIVE_SPAWN_SETBASEVALUE.md section 3, step 5): the same objects in the same order, so a preload
/// built on it cannot drift from the preflight. `visit` sees each object the preflight would check before its fields
/// are read (it may load it) and answers whether its fields can be read; `false` leaves that object's dependants.
/// The preflight's class checks (multi-context, implemented resolver classes, a None read-chain entry, the depth
/// bound) are not made: what the walk passes over is left to the preflight's own refusal. A field the preflight
/// requires that is missing or of the wrong kind is the same error here.
pub(crate) fn walk_set_base_value(
    sys: &AttributeSystem,
    dest: Option<ObjRef>,
    data: &InitData,
    cx: &mut EvalCx<'_>,
    visit: &mut dyn FnMut(&mut EvalCx<'_>, ObjRef) -> Result<bool, AttrError>,
) -> Result<(), AttrError> {
    let mut walk = SetBaseWalk { visit, checks: false };
    if let Some(dest) = dest {
        check_definition(sys, dest, true, cx, &mut walk)?;
    }
    check_init(sys, data, 0, cx, &mut walk)
}

/// What [`check_definition`] and [`check_init`] do at each object: `visit` it, and (`checks`) make the preflight's class
/// checks.
struct SetBaseWalk<'f> {
    visit: &'f mut dyn FnMut(&mut EvalCx<'_>, ObjRef) -> Result<bool, AttrError>,
    checks: bool,
}

/// One attribute definition: loaded, not multi-context, its resolvers loaded and implemented. `set` is the set-base
/// form (the value chain stops at its first None entry); otherwise the read form (a None entry refuses).
fn check_definition(
    sys: &AttributeSystem,
    attr: ObjRef,
    set: bool,
    cx: &mut EvalCx<'_>,
    w: &mut SetBaseWalk<'_>,
) -> Result<(), AttrError> {
    if !(w.visit)(cx, attr)? {
        return Ok(());
    }
    if w.checks && is_multi_context(cx.world.parts().space, attr)? {
        return Err(multi_context_not_read(if set { "ResolveContexts" } else { "GetValue" }));
    }
    let chain = required(cx, attr, "ContextResolverChain")?;
    for entry in elements("ContextResolverChain", &chain)? {
        if let Some(resolver) = object("ContextResolverChain entry", entry)?
            && (w.visit)(cx, resolver)?
            && w.checks
        {
            sys.context_resolver(cx.world.parts().space, resolver)?;
        }
    }
    let chain = required(cx, attr, "ValueResolverChain")?;
    for entry in elements("ValueResolverChain", &chain)? {
        let Some(resolver) = object("ValueResolverChain entry", entry)? else {
            if set || !w.checks {
                break;
            }
            return Err(NotYet::not_read("a None entry in a ValueResolverChain on a read", CHAINS).into());
        };
        if (w.visit)(cx, resolver)? && w.checks {
            sys.value_resolver(cx.world.parts().space, resolver)?;
        }
    }
    Ok(())
}

/// An initialisation data value: its attribute, and its definition followed into the nested data it evaluates.
fn check_init(
    sys: &AttributeSystem,
    data: &InitData,
    depth: usize,
    cx: &mut EvalCx<'_>,
    w: &mut SetBaseWalk<'_>,
) -> Result<(), AttrError> {
    if depth > PREFLIGHT_DEPTH {
        if !w.checks {
            return Ok(());
        }
        return Err(AttrError::BadData {
            reason: format!("initialisation data nested deeper than {PREFLIGHT_DEPTH} (the SetBaseValue preflight)").into(),
        });
    }
    if let Some(attr) = data.base_value_attribute {
        check_definition(sys, attr, false, cx, w)?;
    }
    let Some(definition) = data.initialization_definition else {
        return Ok(());
    };
    if !(w.visit)(cx, definition)? {
        return Ok(());
    }
    let nested = |cx: &mut EvalCx<'_>, v: &Value, name: &str| -> Result<InitData, AttrError> {
        let space = cx.world.parts().space;
        init_data(space, field(space, v, name)?)
    };
    let enabled = |cx: &mut EvalCx<'_>, v: &Value, name: &str| -> Result<bool, AttrError> {
        flag(name, &field_of(cx, v, name)?)
    };
    let mut parts: Vec<InitData> = Vec::new();
    let formula = required(cx, definition, "ValueFormula")?;
    let conditional = required(cx, definition, "ConditionalInitialization")?;
    if enabled(cx, &formula, "bEnabled")? {
        for name in ["Multiplier", "Level", "Power", "Offset"] {
            parts.push(nested(cx, &formula, name)?);
        }
    } else if enabled(cx, &conditional, "bEnabled")? {
        // Only the branches the evaluation can reach (review r2 of #572): an entry with no expressions always holds,
        // so the scan stops there and neither a later entry nor the default is reached. An expression's second
        // operand is checked only when its first is set.
        let list = field_of(cx, &conditional, "ConditionalExpressionList")?;
        let mut always = false;
        for entry in elements("ConditionalExpressionList", &list)? {
            let exprs = {
                let space = cx.world.parts().space;
                crate::expr::read_expressions(space, field(space, entry, "Expressions")?)?
            };
            for e in &exprs {
                if let Some(first) = e.attribute_operand1 {
                    check_definition(sys, first, false, cx, w)?;
                    if let Some(second) = e.attribute_operand2 {
                        check_definition(sys, second, false, cx, w)?;
                    }
                }
            }
            parts.push(nested(cx, entry, "BaseValueIfTrue")?);
            if exprs.is_empty() {
                always = true;
                break;
            }
        }
        if !always {
            parts.push(nested(cx, &conditional, "DefaultBaseValue")?);
        }
    }
    let variance = required(cx, definition, "RandomVariance")?;
    if enabled(cx, &variance, "bEnabled")? {
        parts.push(nested(cx, &variance, "LowerBound")?);
        parts.push(nested(cx, &variance, "UpperBound")?);
    }
    let range = required(cx, definition, "RangeRestriction")?;
    if enabled(cx, &range, "bEnableMinValueRestriction")? {
        parts.push(nested(cx, &range, "MinValue")?);
    }
    if enabled(cx, &range, "bEnableMaxValueRestriction")? {
        parts.push(nested(cx, &range, "MaxValue")?);
    }
    for p in &parts {
        check_init(sys, p, depth + 1, cx, w)?;
    }
    Ok(())
}

#[cfg(test)]
mod load_free_tests {
    use super::{load_free, load_free_unit};

    /// Review r1 of #616, item 4: the scope is closed again after an `Err`, and after a nested scope ends the outer one
    /// is back.
    #[test]
    fn the_scope_is_restored_after_an_error_and_after_nesting() {
        assert_eq!(load_free_unit(), None);
        let outer: Result<(), &str> = load_free("outer", || {
            assert_eq!(load_free_unit(), Some("outer"));
            let inner: Result<(), &str> = load_free("inner", || {
                assert_eq!(load_free_unit(), Some("inner"));
                Err("inner refused")
            });
            assert!(inner.is_err());
            assert_eq!(load_free_unit(), Some("outer"), "the outer scope is back");
            Err("outer refused")
        });
        assert!(outer.is_err());
        assert_eq!(load_free_unit(), None, "closed after an error");
    }
}
