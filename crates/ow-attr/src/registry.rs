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
//! Every entry point returns `NotYet { NotPorted }` until its W3 lane fills it (W3-ATTREVAL: `resolve_context`,
//! `value_of`, `base_value_of`, `evaluate_init`, `expressions_hold`; W3-ATTREFFECTS: `add_modifier`,
//! `remove_modifier`, `set_base`, `apply_effects`, `remove_effects`, [`let_base`]). Those lanes replace the bodies
//! here and keep the signatures.

use std::fmt;

use ow_core::{DetMap, NotYet, ObjectPath};
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::layout::AttrCompanions;

use crate::api::{
    AddModifierOutcome, Applied, AttrError, AttrValue, ContextResolver, EffectData, EvalCx,
    ExprData, ExprMode, InitData, RemoveModifierOutcome, ValueResolver,
};
use crate::stack::CombineForm;

/// The record the "not ported" entry points cite.
const LANES: &str = "architecture.md section 3.5 (W3 attribute lanes)";

fn not_ported<T>(what: &'static str) -> Result<T, AttrError> {
    Err(NotYet::not_ported(what, LANES).into())
}

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

    /// Every resolver class the notes read (NATIVE_ATTRIBUTES.md sections 5 and 6). Empty until W3-ATTREVAL and
    /// W3-ATTRCTX register their classes here.
    pub fn standard() -> Self {
        Self::new_empty()
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

    /// `ResolveContext(ContextSource)`: the attribute definition `attr`'s context chain run on `source`
    /// (NATIVE_ATTRIBUTES.md section 1). `Ok(None)`: the chain did not resolve (an empty chain included).
    pub fn resolve_context(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let _ = (attr, source, cx);
        not_ported("AttributeSystem::resolve_context")
    }

    /// `GetValue(ContextSource, out ResolvedContext, OptionalOverrideContextSource)`: the override source is tried
    /// first, then `source`. `Ok(None)`: no context resolved (the native then returns 0.0 and a None context;
    /// `StaticGetAttributeValueOrDefault` returns its default).
    pub fn value_of(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<AttrValue>, AttrError> {
        let _ = (attr, source, override_source, cx);
        not_ported("AttributeSystem::value_of")
    }

    /// `GetBaseValue`: as [`AttributeSystem::value_of`] with each resolver's base-value method.
    pub fn base_value_of(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<AttrValue>, AttrError> {
        let _ = (attr, source, override_source, cx);
        not_ported("AttributeSystem::base_value_of")
    }

    /// `EvaluateInitializationData(InitializationData, ContextSource, OptionalOverrideContextSource)`
    /// (NATIVE_PROGRESSION.md section 1, NATIVE_ATTRIBUTES.md section 1).
    pub fn evaluate_init(
        &self,
        data: &InitData,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let _ = (data, source, override_source, cx);
        not_ported("AttributeSystem::evaluate_init")
    }

    /// `EvaluateExpressions(Mode, ContextSource, Expressions, OptionalOverrideContextSource)`
    /// (NATIVE_ATTRIBUTES.md section 11).
    pub fn expressions_hold(
        &self,
        exprs: &[ExprData],
        mode: ExprMode,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<bool, AttrError> {
        let _ = (exprs, mode, source, override_source, cx);
        not_ported("AttributeSystem::expressions_hold")
    }

    /// `AddAttributeModifier(Context, Modifier, bSuppressNotify)`: `context` is the **resolved** context, not a
    /// source; `modifier` is the `AttributeModifier` object (its `Type` and `Value` are read at each recompute).
    /// Notifies through [`crate::api::AttrWorld::attribute_changed`] unless `suppress_notify`.
    pub fn add_modifier(
        &self,
        attr: ObjRef,
        context: Option<ObjRef>,
        modifier: ObjRef,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<AddModifierOutcome, AttrError> {
        let _ = (attr, context, modifier, suppress_notify, cx);
        not_ported("AttributeSystem::add_modifier")
    }

    /// `RemoveAttributeModifier(Context, Modifier, bSuppressNotify)`: removes every copy of `modifier`.
    pub fn remove_modifier(
        &self,
        attr: ObjRef,
        context: Option<ObjRef>,
        modifier: ObjRef,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<RemoveModifierOutcome, AttrError> {
        let _ = (attr, context, modifier, suppress_notify, cx);
        not_ported("AttributeSystem::remove_modifier")
    }

    /// `SetAttributeBaseValue(ContextSource, BaseValue, bSuppressNotify)`. `Ok(true)` only when at least one
    /// context and one resolver were found and every set succeeded (NATIVE_ATTRIBUTES.md section 1), the bool the
    /// script sees.
    pub fn set_base(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        value: f32,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<bool, AttrError> {
        let _ = (attr, source, value, suppress_notify, cx);
        not_ported("AttributeSystem::set_base")
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
        let _ = (effects, source, override_source, cx);
        not_ported("AttributeSystem::apply_effects")
    }

    /// `AttributeEffect.RemoveAttributeEffects(EffectModifiers)`: removes every record's modifier from its context.
    pub fn remove_effects(&self, applied: Applied, cx: &mut EvalCx<'_>) -> Result<(), AttrError> {
        let _ = (applied, cx);
        not_ported("AttributeSystem::remove_effects")
    }
}

/// The script VM's typed let into an attribute property (opcode 0x5F): writes the base companion and recomputes the
/// value from the stack in `form`, with no notification. UNVERIFIED (NATIVE_ATTRIBUTES.md, Open: the typed-let
/// opcodes are read as "set the base and recompute"; whether they notify is not read).
pub fn let_base(
    space: &mut ObjectSpace,
    owner: ObjRef,
    companions: AttrCompanions,
    value: f32,
    form: CombineForm,
) -> Result<(), AttrError> {
    let _ = (space, owner, companions, value, form);
    not_ported("ow_attr::registry::let_base")
}
