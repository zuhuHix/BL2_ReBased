//! Owner: lane W2-ATTRAPI. [`AttributeSystem`]: the resolver registry and the evaluator entry points (architecture
//! section 3.5).
//!
//! The system is immutable once built and shared as `Rc<AttributeSystem>`. A caller clones the `Rc`, then lends its
//! world, so no borrow of the system's owner is live while the world is lent:
//!
//! ```ignore
//! let attrs = vm.attrs.clone();
//! let v = attrs.evaluate_init(&data, source, None, &mut EvalCx::new(&mut vm))?;
//! ```
//!
//! Every entry point is a one-line delegate into the file of the lane that owns the rule (architecture section 2.3):
//! `eval.rs` and `expr.rs` (W3-ATTREVAL), `effects.rs` and `storage.rs` (W3-ATTREFFECTS); [`AttributeSystem::standard`]
//! calls `value_resolvers::register` (W3-ATTREVAL) and `context_resolvers::register` (W3-ATTRCTX). Those lanes fill
//! their own files and do not edit this one.

use std::fmt;

use ow_core::{DetMap, NotYet, ObjectPath};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef};
use ow_object::layout::AttrCompanions;
use ow_object::value::Value;

use crate::api::{
    AddModifierOutcome, Applied, AttrError, AttrValue, ContextResolver, EffectData, EvalCx,
    ExprData, ExprMode, InitData, RemoveModifierOutcome, ValueResolver,
};
use crate::stack::CombineForm;
use crate::{context_resolvers, effects, eval, expr, storage, value_resolvers};

/// The resolver registry and the evaluator entry points. Resolvers are keyed by their **exact** class path
/// (case-insensitive, `:` equal to `.`): a subclass is not served by its parent's entry, because an unread subclass
/// may override the virtual, and an unread class is `NotYet { NotRead }`, never the parent's answer.
#[derive(Default)]
pub struct AttributeSystem {
    value: DetMap<ObjectPath, Box<dyn ValueResolver>>,
    context: DetMap<ObjectPath, Box<dyn ContextResolver>>,
}

impl fmt::Debug for AttributeSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttributeSystem")
            .field("value_resolvers", &self.value.keys().collect::<Vec<_>>())
            .field(
                "context_resolvers",
                &self.context.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

fn class_key(class_path: &str) -> Result<ObjectPath, AttrError> {
    ObjectPath::parse(class_path).map_err(|e| AttrError::BadData {
        reason: format!("resolver class path {class_path:?}: {e}").into(),
    })
}

/// The class path of object `o`, for the registry lookup.
fn class_path_of(space: &ObjectSpace, o: ObjRef) -> Result<&ObjectPath, AttrError> {
    let class = space.heap.get(o)?.class;
    match space.classes.get_layout(class) {
        Some(layout) => Ok(layout.path()),
        None => Err(AttrError::BadData {
            reason: format!("resolver object {o:?} has a class the table did not make").into(),
        }),
    }
}

impl AttributeSystem {
    /// No resolver registered: every lookup is `NotYet { NotRead }`.
    pub fn new_empty() -> Self {
        Self::default()
    }

    /// Every resolver class the notes read (NATIVE_ATTRIBUTES.md sections 5 and 6), registered by
    /// `value_resolvers::register` and `context_resolvers::register`. None yet.
    pub fn standard() -> Result<Self, AttrError> {
        let sys = value_resolvers::register(Self::new_empty())?;
        context_resolvers::register(sys)
    }

    /// Registers the value resolver for class `class_path`. A path registered twice is refused.
    pub fn with_value_resolver(
        mut self,
        class_path: &str,
        resolver: impl ValueResolver + 'static,
    ) -> Result<Self, AttrError> {
        let key = class_key(class_path)?;
        if self.value.contains_key(&key) {
            return Err(AttrError::DuplicateResolver {
                class: class_path.to_string(),
            });
        }
        self.value.insert(key, Box::new(resolver));
        Ok(self)
    }

    /// Registers the context resolver for class `class_path`. A path registered twice is refused.
    pub fn with_context_resolver(
        mut self,
        class_path: &str,
        resolver: impl ContextResolver + 'static,
    ) -> Result<Self, AttrError> {
        let key = class_key(class_path)?;
        if self.context.contains_key(&key) {
            return Err(AttrError::DuplicateResolver {
                class: class_path.to_string(),
            });
        }
        self.context.insert(key, Box::new(resolver));
        Ok(self)
    }

    /// The value resolver for the class of `resolver`, or `NotYet { NotRead }` naming that class.
    pub fn value_resolver(
        &self,
        space: &ObjectSpace,
        resolver: ObjRef,
    ) -> Result<&dyn ValueResolver, AttrError> {
        let path = class_path_of(space, resolver)?;
        match self.value.get(path) {
            Some(r) => Ok(r.as_ref()),
            None => Err(NotYet::not_read(
                format!("value resolver class {}", path.as_str()),
                "NATIVE_ATTRIBUTES.md section 6",
            )
            .into()),
        }
    }

    /// The context resolver for the class of `resolver`, or `NotYet { NotRead }` naming that class.
    pub fn context_resolver(
        &self,
        space: &ObjectSpace,
        resolver: ObjRef,
    ) -> Result<&dyn ContextResolver, AttrError> {
        let path = class_path_of(space, resolver)?;
        match self.context.get(path) {
            Some(r) => Ok(r.as_ref()),
            None => Err(NotYet::not_read(
                format!("context resolver class {}", path.as_str()),
                "NATIVE_ATTRIBUTES.md section 5",
            )
            .into()),
        }
    }

    /// `AttributeDefinitionBase.ResolveContext(ContextSource)`: the attribute definition `attr`'s context chain run
    /// on `source` (NATIVE_ATTRIBUTES.md section 1). `Ok(None)`: the chain did not resolve (an empty chain included).
    pub fn resolve_context(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        eval::resolve_context(self, attr, source, cx)
    }

    /// `AttributeDefinitionBase.ResolveContexts(ContextSource, out ResolvedContexts)`: the definition's list of
    /// contexts (one at most for an `AttributeDefinition`; the multi-context expansion otherwise).
    pub fn resolve_contexts(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Vec<ObjRef>, AttrError> {
        eval::resolve_contexts(self, attr, source, cx)
    }

    /// `AttributeDefinition.GetValue(ContextSource, out ResolvedContext, OptionalOverrideContextSource)`: the
    /// override source is tried first, then `source`. `Ok(None)`: no context resolved (the native then returns 0.0
    /// and a None context; `StaticGetAttributeValueOrDefault` returns its default).
    pub fn value_of(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<AttrValue>, AttrError> {
        eval::value_of(self, attr, source, override_source, cx)
    }

    /// The context step of `GetValue` alone: the override source first when given, then `source`, through the
    /// definition's context chain (NATIVE_ATTRIBUTES.md, "AttributeDefinition.GetValue"). A multi-context definition is
    /// `NotYet`. [`AttributeSystem::value_of`] is this step followed by [`AttributeSystem::value_from_context`]; a
    /// caller that writes the resolved context somewhere before the value chain runs uses the two steps.
    pub fn value_context(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        eval::value_context(self, attr, source, override_source, cx)
    }

    /// What a read of `attr` can refuse before it evaluates anything: a multi-context definition, a context or value
    /// resolver class this library does not implement, a None value chain entry. Nothing is resolved or evaluated.
    pub fn preflight_read(&self, attr: ObjRef, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
        eval::preflight_read(self, attr, cx)
    }

    /// `AttributeDefinition.GetBaseValue`: as [`AttributeSystem::value_of`] with each resolver's base-value method.
    pub fn base_value_of(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<AttrValue>, AttrError> {
        eval::base_value_of(self, attr, source, override_source, cx)
    }

    /// `AttributeDefinitionBase.GetValueFromContext(Context)`: the value chain on an already-resolved context; 0.0
    /// for a None context or an empty chain (NATIVE_ATTRIBUTES.md section 1).
    pub fn value_from_context(
        &self,
        attr: ObjRef,
        context: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        eval::value_from_context(self, attr, context, cx)
    }

    /// `AttributeDefinitionBase.GetBaseValueFromContext(Context)`: as [`AttributeSystem::value_from_context`] with
    /// each resolver's base-value method.
    pub fn base_value_from_context(
        &self,
        attr: ObjRef,
        context: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        eval::base_value_from_context(self, attr, context, cx)
    }

    /// `AttributeInitializationDefinition.EvaluateInitializationData(InitializationData, ContextSource,
    /// OptionalOverrideContextSource)` (NATIVE_PROGRESSION.md section 1, NATIVE_ATTRIBUTES.md section 1).
    pub fn evaluate_init(
        &self,
        data: &InitData,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        eval::evaluate_init(self, data, source, override_source, cx)
    }

    /// `AttributeExpression.EvaluateExpressions(Mode, ContextSource, Expressions, OptionalOverrideContextSource)`
    /// (NATIVE_ATTRIBUTES.md section 11).
    pub fn expressions_hold(
        &self,
        exprs: &[ExprData],
        mode: ExprMode,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<bool, AttrError> {
        expr::expressions_hold(self, exprs, mode, source, override_source, cx)
    }

    /// `AttributeDefinitionBase.AddAttributeModifier(Context, Modifier, bSuppressNotify)`: `context` is the
    /// **resolved** context, not a source; `modifier` is the `AttributeModifier` object (its `Type` and `Value` are
    /// read at each recompute). Notifies through [`crate::api::AttrWorld::attribute_changed`] unless
    /// `suppress_notify`. The outcome is the chain's success; an earlier writable resolver's change is not rolled
    /// back (see [`AddModifierOutcome`]).
    pub fn add_modifier(
        &self,
        attr: ObjRef,
        context: Option<ObjRef>,
        modifier: ObjRef,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<AddModifierOutcome, AttrError> {
        effects::add_modifier(self, attr, context, modifier, suppress_notify, cx)
    }

    /// `AttributeDefinitionBase.RemoveAttributeModifier(Context, Modifier, bSuppressNotify)`: removes every copy of
    /// `modifier`. The outcome is the chain's success, with no rollback (see [`RemoveModifierOutcome`]).
    pub fn remove_modifier(
        &self,
        attr: ObjRef,
        context: Option<ObjRef>,
        modifier: ObjRef,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<RemoveModifierOutcome, AttrError> {
        effects::remove_modifier(self, attr, context, modifier, suppress_notify, cx)
    }

    /// The read-only chain check of an add or a remove of a modifier through `attr`'s value chain on `context`: every
    /// entry's resolver class known and its storage lookup answering, no stack touched (lane
    /// IMPL-APPLY-SLOT-EFFECT-MODIFIERS, decision 2026-11-19-97). It predicts no outcome.
    pub fn check_modifier_chain(&self, attr: ObjRef, context: ObjRef, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
        effects::check_modifier_chain(self, attr, context, cx)
    }

    /// A new `Core.AttributeModifier` of `kind` and `value`, outer `outer` (the object the apply natives make).
    pub fn new_modifier(
        &self,
        outer: Option<ObjRef>,
        kind: crate::stack::ModifierType,
        value: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<ObjRef, AttrError> {
        effects::make_modifier(cx, outer, kind, value)
    }

    /// `AttributeDefinitionBase.SetAttributeBaseValue(ContextSource, BaseValue, bSuppressNotify)`. `Ok(true)` only
    /// when at least one context and one resolver were found and every set succeeded (NATIVE_ATTRIBUTES.md section
    /// 1), the bool the script sees. The calls stop at the first failure; earlier sets stay (no rollback).
    pub fn set_base(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        value: f32,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<bool, AttrError> {
        effects::set_base(self, attr, source, value, suppress_notify, cx)
    }

    /// `AttributeEffect.ApplyAttributeEffects(ContextSource, InAttributeEffects, out OutModifiers,
    /// OptionalOverrideContextSource)` (NATIVE_ATTRIBUTES.md section 7): each value evaluated once, at apply time;
    /// the override context tried first.
    pub fn apply_effects(
        &self,
        effects: &[EffectData],
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Applied, AttrError> {
        effects::apply_effects(self, effects, source, override_source, cx)
    }

    /// `AttributeEffect.RemoveAttributeEffects(EffectModifiers)`: removes every record's modifier from its context.
    pub fn remove_effects(&self, applied: Applied, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
        effects::remove_effects(self, applied, cx)
    }

    /// `Object.AddModifier(mod, AttributeName, bSuppressNotify)`: the property-level add of NATIVE_ATTRIBUTES.md
    /// section 3 on `object`'s property `attribute_name`, with no definition and no chains (section 1).
    pub fn add_modifier_by_name(
        &self,
        object: ObjRef,
        modifier: ObjRef,
        attribute_name: NameId,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<AddModifierOutcome, AttrError> {
        storage::add_modifier_by_name(self, object, modifier, attribute_name, suppress_notify, cx)
    }

    /// `Object.RemoveModifier(mod, AttributeName, bSuppressNotify)`: the property-level remove, by name.
    pub fn remove_modifier_by_name(
        &self,
        object: ObjRef,
        modifier: ObjRef,
        attribute_name: NameId,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<RemoveModifierOutcome, AttrError> {
        storage::remove_modifier_by_name(
            self,
            object,
            modifier,
            attribute_name,
            suppress_notify,
            cx,
        )
    }

    /// `Object.GetAttributeValueByName(AttributeName)`: the property's stored value, 0.0 if the name is not an
    /// attribute (NATIVE_ATTRIBUTES.md section 1).
    pub fn attribute_value_by_name(
        &self,
        object: ObjRef,
        attribute_name: NameId,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        storage::attribute_value_by_name(self, object, attribute_name, cx)
    }
}

/// The script VM's typed let into an attribute property (opcode 0x5F): stores `rhs` into the base companion as it
/// is, then recomputes the value from the base and the stack in `form`.
///
/// The conversion is the caller's: the VM converts the right-hand side with its own assignment rule first
/// (`interp.cpp:511-522`, ported in `ow-vm` by W3-INTERP), so this function holds no copy of it. `rhs` must already
/// have the base property's kind; a `Value` of another kind is **refused** with [`AttrError::BadData`] and nothing is
/// written. That refusal is deliberate: the C++ stores a mismatched kind silently. An `Int` base keeps the integer
/// exactly (16,777,217 included, `interp.cpp:593`) until the recompute. **No change
/// notification**: NATIVE_BYTECODE_OPCODES.md (opcode 5F, high confidence for what it does) reads it so,
/// NATIVE_LEVEL_UP_ATTRIBUTES.md (the pool rebase) depends on it and `interp.cpp:566-569` implements it. The "Open"
/// line of NATIVE_ATTRIBUTES.md about how the typed let runs predates that reading and is stale. Status: UNVERIFIED in
/// the running game.
pub fn let_base(
    space: &mut ObjectSpace,
    owner: ObjRef,
    companions: AttrCompanions,
    rhs: Value,
    form: CombineForm,
) -> Result<(), AttrError> {
    storage::let_base(space, owner, companions, rhs, form)
}
