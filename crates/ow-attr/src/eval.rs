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

/// Makes sure `obj` can be read (module documentation). Only an object with `Origin::Resource` can be an unloaded
/// definition handle (a load gives it `Origin::Export`), so only such an object goes to the world's store, whose
/// [`DefStore::ensure_loaded`](ow_defs::DefStore::ensure_loaded) loads it, or leaves it as it is when it is a
/// stand-in that stays one. Every other live object (run-time objects, loaded definitions, class defaults) is read as
/// it is. A world without a store answers `NotYet { LayerAbsent("ow-defs") }` for a slotless resource handle.
pub fn ensure_loaded(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<(), AttrError> {
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
        Some(defs) => Ok(defs.ensure_loaded(space, obj)?),
        None if slotless => Err(NotYet::layer_absent(
            format!("loading the unloaded definition {obj:?}"),
            "ow-defs",
            CHAINS,
        )
        .into()),
        None => Ok(()),
    }
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

/// [`AttributeSystem::resolve_context`]: the `ContextResolverChain` left to right, each resolver receiving what the
/// previous returned (the first receives `source`); a None entry is skipped; a resolver answering None ends the walk
/// with None; an empty chain (or one of None entries only) is None. Status: UNVERIFIED (NATIVE_ATTRIBUTES.md
/// section 1, high confidence).
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
fn value_context(
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

/// The class of a designer attribute (the C++ reads its `BaseValue` as its value; [`fold`]).
const DESIGNER_DEFINITION: &str = "WillowGame.DesignerAttributeDefinition";

/// The value chain on a resolved context: one fold, each resolver receiving the previous result (0.0 first); the
/// last result is the value; an empty chain is 0.0. A None entry in a value chain is not read (the record says what
/// a None entry does only for add and remove), so it is `NotYet { NotRead }`.
fn fold(
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
    let reference = cx
        .world
        .parts()
        .defs
        .is_some_and(|d| d.mode() == ow_core::ParityMode::Reference);
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

    let formula = required(cx, definition, "ValueFormula")?;
    let conditional = required(cx, definition, "ConditionalInitialization")?;
    let variance = required(cx, definition, "RandomVariance")?;
    let range = required(cx, definition, "RangeRestriction")?;
    let mode = byte("BaseValueMode", &required(cx, definition, "BaseValueMode")?)?;
    let rounding = byte("RoundingMode", &required(cx, definition, "RoundingMode")?)?;
    let bad = |what: &str, raw: u8| AttrError::BadData {
        reason: format!("{what} {raw} is outside the enum").into(),
    };
    let mode =
        BaseValueMode::from_raw(i64::from(mode)).ok_or_else(|| bad("BaseValueMode", mode))?;
    let rounding =
        RoundingMode::from_value(rounding).ok_or_else(|| bad("RoundingMode", rounding))?;

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
fn random_between(
    low: f32,
    high: f32,
    integer: bool,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
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
