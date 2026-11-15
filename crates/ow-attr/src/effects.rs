//! Owner: lane W3-ATTREFFECTS. Modifiers through a definition's value chain, set-base, and
//! `AttributeEffect.ApplyAttributeEffects` / `RemoveAttributeEffects` (NATIVE_ATTRIBUTES.md sections 1 and 7). What
//! each step does to the stored numbers is `storage.rs`'s.
//!
//! Every rule here is read from native code or script and is **UNVERIFIED in the running game**.
//!
//! # The chain rule (architecture section 3.5)
//!
//! The value resolvers of the definition's `ValueResolverChain` are asked in order, each only while the previous one
//! succeeded; a null entry is a failure; the outcome is the last resolver asked. Only a resolver whose
//! [`crate::api::ValueResolver::storage`] names a number can take an add, a remove or a set-base (in the record only
//! `ObjectPropertyAttributeValueResolver`); every other one answers false. Nothing is rolled back: a later failure
//! leaves an earlier resolver's change in place.

use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use crate::api::{
    AddModifierOutcome, Applied, AppliedEffect, AttrError, EffectData, EvalCx, InitData, Refusal,
    RemoveModifierOutcome, Storage, WorldParts,
};
use crate::registry::AttributeSystem;
use crate::stack::ModifierType;
use crate::storage::{self, ATTRIBUTE_MODIFIER_CLASS};

fn bad(reason: String) -> AttrError {
    AttrError::BadData {
        reason: reason.into(),
    }
}

/// Loads `obj` in place when it is a definition handle the store has not loaded yet; anything else is left as it is.
/// A world without a store has no handles to load.
fn ensure_loaded(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<(), AttrError> {
    let WorldParts { space, defs, .. } = cx.world.parts();
    if let Some(defs) = defs {
        defs.ensure_loaded(space, obj)?;
    }
    Ok(())
}

/// The attribute definition `attr`'s `ValueResolverChain`, in order (a null entry kept as `None`).
fn value_chain(cx: &mut EvalCx<'_>, attr: ObjRef) -> Result<Vec<Option<ObjRef>>, AttrError> {
    ensure_loaded(cx, attr)?;
    let space: &ObjectSpace = cx.world.parts().space;
    let class = space.heap.get(attr)?.class;
    let chain = space.prop(class, "ValueResolverChain").ok_or_else(|| {
        bad(format!(
            "{attr:?} has no ValueResolverChain: not an attribute definition"
        ))
    })?;
    let Value::Array(items) = space.read(attr, PropAddr::scalar(chain))? else {
        return Err(bad("ValueResolverChain is not an array".to_string()));
    };
    items
        .iter()
        .map(|item| match item {
            Value::Object(r) => Ok(*r),
            other => Err(bad(format!(
                "ValueResolverChain element is a {}",
                other.kind_name()
            ))),
        })
        .collect()
}

/// The storage resolver `r` names on `context`, or `None` when its class takes no writes.
///
/// `None` also comes back when a writable resolver names a property that is not an attribute (`storage::storage_of`):
/// the contract's `Option<Storage>` does not tell the two apart, so both are reported as `Refusal::NotWritable`. The
/// native's bool is the same (false) either way; only the refusal reason is coarser (open item).
fn storage_at(
    sys: &AttributeSystem,
    r: ObjRef,
    context: ObjRef,
    cx: &mut EvalCx<'_>,
) -> Result<Option<Storage>, AttrError> {
    ensure_loaded(cx, r)?;
    let resolver = sys.value_resolver(cx.world.parts().space, r)?;
    resolver.storage(sys, r, context, cx)
}

/// The add through the chain, and whether any resolver stored the modifier (so it is referenced).
fn add_chain(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    modifier: ObjRef,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<(AddModifierOutcome, bool), AttrError> {
    let Some(context) = context else {
        return Ok((AddModifierOutcome::Refused(Refusal::NoContext), false));
    };
    let chain = value_chain(cx, attr)?;
    let mut outcome = AddModifierOutcome::Refused(Refusal::NoValueResolvers);
    let mut stored = false;
    for entry in chain {
        outcome = match entry {
            None => AddModifierOutcome::Refused(Refusal::NullResolver),
            Some(r) => match storage_at(sys, r, context, cx)? {
                None => AddModifierOutcome::Refused(Refusal::NotWritable),
                Some(s) => storage::add_to(s, modifier, suppress_notify, cx)?,
            },
        };
        if !outcome.succeeded() {
            break;
        }
        stored = true;
    }
    Ok((outcome, stored))
}

/// [`AttributeSystem::add_modifier`].
pub(crate) fn add_modifier(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    modifier: ObjRef,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<AddModifierOutcome, AttrError> {
    add_chain(sys, attr, context, modifier, suppress_notify, cx).map(|(outcome, _)| outcome)
}

/// [`AttributeSystem::remove_modifier`].
pub(crate) fn remove_modifier(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    modifier: ObjRef,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<RemoveModifierOutcome, AttrError> {
    let Some(context) = context else {
        return Ok(RemoveModifierOutcome::Refused(Refusal::NoContext));
    };
    let chain = value_chain(cx, attr)?;
    let mut outcome = RemoveModifierOutcome::Refused(Refusal::NoValueResolvers);
    for entry in chain {
        outcome = match entry {
            None => RemoveModifierOutcome::Refused(Refusal::NullResolver),
            Some(r) => match storage_at(sys, r, context, cx)? {
                None => RemoveModifierOutcome::Refused(Refusal::NotWritable),
                Some(s) => storage::remove_from(s, modifier, suppress_notify, cx)?,
            },
        };
        if !outcome.succeeded() {
            break;
        }
    }
    Ok(outcome)
}

/// [`AttributeSystem::set_base`]: the contexts first (`ResolveContexts`), then for each value resolver and each
/// context one set. True only with at least one context and one resolver and every set succeeding; the first failure
/// stops the calls, and earlier sets stay.
pub(crate) fn set_base(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    value: f32,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<bool, AttrError> {
    let contexts = sys.resolve_contexts(attr, source, cx)?;
    set_base_in(sys, attr, &contexts, value, suppress_notify, cx)
}

/// The sets of [`set_base`] on contexts already resolved.
fn set_base_in(
    sys: &AttributeSystem,
    attr: ObjRef,
    contexts: &[ObjRef],
    value: f32,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<bool, AttrError> {
    let chain = value_chain(cx, attr)?;
    if contexts.is_empty() || chain.is_empty() {
        return Ok(false);
    }
    for entry in chain {
        let Some(r) = entry else {
            return Ok(false);
        };
        for &context in contexts {
            match storage_at(sys, r, context, cx)? {
                None => return Ok(false),
                Some(s) => storage::set_base_of(s, value, suppress_notify, cx)?,
            }
        }
    }
    Ok(true)
}

/// The two steps of the apply loop that belong to the evaluator (W3-ATTREVAL): the value of an effect's
/// initialization data, and an attribute's context. Production code uses [`Chains`] only; the seam lets the loop be
/// tested on its own.
trait EvalSteps {
    fn value(
        &self,
        sys: &AttributeSystem,
        data: &InitData,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError>;

    fn context(
        &self,
        sys: &AttributeSystem,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError>;
}

/// The evaluator and the context chain of the one [`AttributeSystem`].
struct Chains;

impl EvalSteps for Chains {
    fn value(
        &self,
        sys: &AttributeSystem,
        data: &InitData,
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        sys.evaluate_init(data, source, override_source, cx)
    }

    fn context(
        &self,
        sys: &AttributeSystem,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        sys.resolve_context(attr, source, cx)
    }
}

/// A new `Core.AttributeModifier` with `Type` and `Value` set, outer the context source.
///
/// Its name is the class name without a number: the per-class numbering of objects made at run time is the VM's
/// (`Vm::instantiate`), which `ow-attr` cannot reach; nothing in the rules reads a modifier's name (open item).
fn new_modifier(
    cx: &mut EvalCx<'_>,
    outer: Option<ObjRef>,
    kind: ModifierType,
    value: f32,
) -> Result<ObjRef, AttrError> {
    let WorldParts { space, defs, .. } = cx.world.parts();
    let class = match (space.classes.by_path(ATTRIBUTE_MODIFIER_CLASS), defs) {
        (Some(c), _) => c,
        (None, Some(defs)) => defs.class_id(space, ATTRIBUTE_MODIFIER_CLASS)?,
        (None, None) => {
            return Err(bad(format!(
                "no class {ATTRIBUTE_MODIFIER_CLASS} in a world without a definition store"
            )));
        }
    };
    let m = space.alloc(
        class,
        "AttributeModifier",
        outer,
        ow_object::heap::Origin::Runtime,
    )?;
    let missing = || bad(format!("{ATTRIBUTE_MODIFIER_CLASS} has no Type or Value"));
    let type_prop = space.prop(class, "Type").ok_or_else(missing)?;
    let value_prop = space.prop(class, "Value").ok_or_else(missing)?;
    let raw = kind as u8;
    let type_value = match space.read(m, PropAddr::scalar(type_prop))? {
        Value::Int(_) => Value::Int(i32::from(raw)),
        _ => Value::Byte(raw),
    };
    space.write(m, PropAddr::scalar(type_prop), type_value)?;
    space.write(m, PropAddr::scalar(value_prop), Value::Float(value))?;
    Ok(m)
}

/// The apply loop of NATIVE_ATTRIBUTES.md section 7, per entry in order:
/// 1. the value: `EvaluateInitializationData(BaseModifierValue, ContextSource, Override)`, once, now (also for an
///    entry without an attribute, as the script evaluates it before resolving, so its random variance draws);
/// 2. an entry without `AttributeToModify` is skipped;
/// 3. a new modifier object with the entry's type and that value;
/// 4. the context: the override's if one was given and it resolves, else the source's;
/// 5. with a context and a successful `AddAttributeModifier`, a record (context, attribute, modifier).
///
/// A failed entry leaves no record. Its modifier object is freed when no resolver stored it, because nothing else
/// can hold it (the script leaves it to the garbage collector).
fn apply_with(
    sys: &AttributeSystem,
    steps: &dyn EvalSteps,
    effects: &[EffectData],
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Applied, AttrError> {
    let mut applied = Applied::default();
    for effect in effects {
        let value = steps.value(
            sys,
            &effect.base_modifier_value,
            source,
            override_source,
            cx,
        )?;
        let Some(attr) = effect.attribute_to_modify else {
            continue;
        };
        let modifier = new_modifier(cx, source, effect.modifier_type, value)?;
        let mut context = None;
        if override_source.is_some() {
            context = steps.context(sys, attr, override_source, cx)?;
        }
        if context.is_none() {
            context = steps.context(sys, attr, source, cx)?;
        }
        let (outcome, stored) = match context {
            Some(_) => add_chain(sys, attr, context, modifier, false, cx)?,
            None => (AddModifierOutcome::Refused(Refusal::NoContext), false),
        };
        match context {
            Some(context) if outcome.succeeded() => applied.0.push(AppliedEffect {
                context,
                attribute: attr,
                modifier,
            }),
            _ if !stored => {
                cx.world.parts().space.free(modifier)?;
            }
            _ => {}
        }
    }
    Ok(applied)
}

/// [`AttributeSystem::apply_effects`].
pub(crate) fn apply_effects(
    sys: &AttributeSystem,
    effects: &[EffectData],
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Applied, AttrError> {
    apply_with(sys, &Chains, effects, source, override_source, cx)
}

/// [`AttributeSystem::remove_effects`]: every record's modifier removed from its context through the attribute's
/// chain, in record order, whatever each removal answers; the records are consumed (the script empties the array).
/// The modifier objects are not freed: a script copy of the records may still name them.
pub(crate) fn remove_effects(
    sys: &AttributeSystem,
    applied: Applied,
    cx: &mut EvalCx<'_>,
) -> Result<(), AttrError> {
    for record in applied.0 {
        remove_modifier(
            sys,
            record.attribute,
            Some(record.context),
            record.modifier,
            false,
            cx,
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod fixture {
    //! A synthetic world for the effect and storage tests (rule 9: invented classes, names and values only). It has
    //! no definition store, so every definition is a plain runtime object. Until W3-ATTREVAL registers the real
    //! `ObjectPropertyAttributeValueResolver`, a test resolver class stands in for it: it reads the property named by
    //! its own `PropertyName` field, and its storage is [`storage::storage_of`] on that property.

    use ow_core::NotYet;
    use ow_core::rand::{CrtStream, Rand15};
    use ow_object::heap::{ObjectSpace, Origin};
    use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
    use ow_object::layout::{PropKind, PropertyDecl};
    use ow_object::value::Value;

    use crate::api::{
        AttrError, AttrWorld, EvalCx, GlobalSlots, NumForms, Storage, ValueResolver, WorldParts,
    };
    use crate::registry::AttributeSystem;
    use crate::storage::{self, Notified};

    pub(crate) const STACK: u64 = PropertyDecl::STACK_ATTRIBUTE;
    pub(crate) const PLAIN: u64 = PropertyDecl::PLAIN_ATTRIBUTE;
    pub(crate) const NOTIFY: u64 = PropertyDecl::NOTIFY;

    /// The test world: a space, the global slots, no generator, and a log of every notification with what the
    /// owner's override did.
    pub(crate) struct World {
        pub space: ObjectSpace,
        pub slots: GlobalSlots,
        pub notified: Vec<(ObjRef, String, Notified)>,
    }

    impl AttrWorld for World {
        fn parts(&mut self) -> WorldParts<'_> {
            WorldParts::new(&mut self.space, None)
        }
        fn globals(&mut self) -> Result<ObjRef, AttrError> {
            Err(NotYet::layer_absent("the engine globals object", "ow-world", "test world").into())
        }
        fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
            Ok(None)
        }
        fn global_slots(&mut self) -> &mut GlobalSlots {
            &mut self.slots
        }
        fn attribute_changed(
            &mut self,
            owner: ObjRef,
            value_prop: PropId,
            _forms: NumForms,
        ) -> Result<(), AttrError> {
            let done = storage::owner_notification(&mut self.space, owner, value_prop)?;
            let name = self.space.classes.decl(value_prop).map(|d| d.name).unwrap();
            let name = self.space.names.text(name).into_owned();
            self.notified.push((owner, name, done));
            Ok(())
        }
        fn rng(&mut self, _s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
            Err(NotYet::layer_absent("a random source", "ow-world", "test world").into())
        }
    }

    /// The writable test resolver (see the module doc).
    pub(crate) struct NamedProperty;

    impl NamedProperty {
        fn target(
            cx: &mut EvalCx<'_>,
            resolver: ObjRef,
            context: ObjRef,
        ) -> Result<Option<PropId>, AttrError> {
            let space = cx.world.parts().space;
            let class = space.heap.get(resolver)?.class;
            let field = space.prop(class, "PropertyName").unwrap();
            let Value::Name(n) = space.read(resolver, PropAddr::scalar(field))?.clone() else {
                panic!("PropertyName is a name");
            };
            Ok(space.classes.prop(space.heap.get(context)?.class, n))
        }
    }

    impl ValueResolver for NamedProperty {
        fn value(
            &self,
            _sys: &AttributeSystem,
            resolver: ObjRef,
            context: ObjRef,
            _previous: f32,
            cx: &mut EvalCx<'_>,
        ) -> Result<f32, AttrError> {
            let p = Self::target(cx, resolver, context)?.unwrap();
            match cx.world.parts().space.read(context, PropAddr::scalar(p))? {
                Value::Float(x) => Ok(*x),
                _ => Ok(0.0),
            }
        }
        fn storage(
            &self,
            _sys: &AttributeSystem,
            resolver: ObjRef,
            context: ObjRef,
            cx: &mut EvalCx<'_>,
        ) -> Result<Option<Storage>, AttrError> {
            match Self::target(cx, resolver, context)? {
                Some(p) => storage::storage_of(cx.world.parts().space, context, p),
                None => Ok(None),
            }
        }
    }

    /// A resolver that answers a number and takes no writes (the class default storage), as a constant does.
    pub(crate) struct ReadOnly;

    impl ValueResolver for ReadOnly {
        fn value(
            &self,
            _sys: &AttributeSystem,
            _resolver: ObjRef,
            _context: ObjRef,
            _previous: f32,
            _cx: &mut EvalCx<'_>,
        ) -> Result<f32, AttrError> {
            Ok(1.0)
        }
    }

    pub(crate) fn system() -> AttributeSystem {
        AttributeSystem::new_empty()
            .with_value_resolver("Test.NamedPropertyValueResolver", NamedProperty)
            .unwrap()
            .with_value_resolver("Test.ReadOnlyValueResolver", ReadOnly)
            .unwrap()
    }

    pub(crate) struct Classes {
        pub object: ClassId,
        pub modifier: ClassId,
        pub definition: ClassId,
        pub named: ClassId,
        pub read_only: ClassId,
        pub unread: ClassId,
        pub weapon: ClassId,
        pub pool: ClassId,
        pub skill: ClassId,
    }

    fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
        PropertyDecl::new(space.names.intern(name), kind).with_flags(flags)
    }

    /// The three declarations of attribute `name`: value, `<name>BaseValue`, `<name>ModifierStack`.
    fn triple(
        space: &mut ObjectSpace,
        name: &str,
        int: bool,
        notify: u64,
        m: ClassId,
    ) -> Vec<PropertyDecl> {
        let kind = if int {
            PropKind::IntAttribute
        } else {
            PropKind::FloatAttribute
        };
        let stack = PropKind::Array(Box::new(PropKind::Object { class: m }));
        vec![
            decl(space, name, kind.clone(), STACK | notify),
            decl(space, &format!("{name}BaseValue"), kind, PLAIN),
            decl(space, &format!("{name}ModifierStack"), stack, 0),
        ]
    }

    fn link(space: &mut ObjectSpace, c: ClassId, name: &str) {
        let v = space.names.intern(name);
        let b = space.names.intern(&format!("{name}BaseValue"));
        let s = space.names.intern(&format!("{name}ModifierStack"));
        space.classes.link_attribute(c, v, b, s).unwrap();
    }

    pub(crate) fn world() -> (World, Classes) {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let define = |space: &mut ObjectSpace, path: &str, props: Vec<PropertyDecl>| {
            space.classes.define(path, Some(object), props).unwrap()
        };
        let props = vec![
            decl(&mut space, "Type", PropKind::Byte { enum_: None }, 0),
            decl(&mut space, "Value", PropKind::Float, 0),
        ];
        let modifier = define(&mut space, "Core.AttributeModifier", props);
        let chain = PropKind::Array(Box::new(PropKind::Object { class: object }));
        let props = vec![decl(&mut space, "ValueResolverChain", chain, 0)];
        let definition = define(&mut space, "Test.AttributeDefinition", props);
        let props = vec![decl(&mut space, "PropertyName", PropKind::Name, 0)];
        let named = define(&mut space, "Test.NamedPropertyValueResolver", props);
        let read_only = define(&mut space, "Test.ReadOnlyValueResolver", vec![]);
        let unread = define(&mut space, "Test.UnreadValueResolver", vec![]);

        let mut props = triple(&mut space, "InstantHitDamage", false, 0, modifier);
        props.extend(triple(&mut space, "ClipSize", true, NOTIFY, modifier));
        props.push(decl(&mut space, "ReloadCnt", PropKind::Int, PLAIN));
        props.push(decl(&mut space, "Spread", PropKind::Float, 0));
        // A stack-flagged property whose companions were never linked: the record's "stack not found".
        props.push(decl(&mut space, "Broken", PropKind::FloatAttribute, STACK));
        props.push(decl(&mut space, "Flagged", PropKind::Bool, PLAIN));
        let weapon = define(&mut space, "Test.Weapon", props);
        link(&mut space, weapon, "InstantHitDamage");
        link(&mut space, weapon, "ClipSize");

        let mut props = vec![decl(
            &mut space,
            "CurrentValue",
            PropKind::FloatAttribute,
            PLAIN | NOTIFY,
        )];
        props.extend(triple(&mut space, "MinValue", false, 0, modifier));
        props.extend(triple(&mut space, "MaxValue", false, 0, modifier));
        let pool = define(&mut space, "Engine.ResourcePool", props);
        link(&mut space, pool, "MinValue");
        link(&mut space, pool, "MaxValue");

        let mut props = triple(&mut space, "Grade", true, NOTIFY, modifier);
        props.push(decl(
            &mut space,
            "bForceRefreshModifiersNextTick",
            PropKind::Bool,
            0,
        ));
        let skill = define(&mut space, "Test.Skill", props);
        link(&mut space, skill, "Grade");

        let w = World {
            space,
            slots: GlobalSlots::default(),
            notified: Vec::new(),
        };
        let c = Classes {
            object,
            modifier,
            definition,
            named,
            read_only,
            unread,
            weapon,
            pool,
            skill,
        };
        (w, c)
    }

    impl World {
        pub(crate) fn obj(&mut self, c: ClassId, name: &str) -> ObjRef {
            self.space.alloc(c, name, None, Origin::Runtime).unwrap()
        }

        fn addr(&self, o: ObjRef, prop: &str) -> PropAddr {
            let class = self.space.heap.get(o).unwrap().class;
            PropAddr::scalar(self.space.prop(class, prop).unwrap())
        }

        pub(crate) fn get(&self, o: ObjRef, prop: &str) -> Value {
            self.space.read(o, self.addr(o, prop)).unwrap().clone()
        }

        pub(crate) fn set(&mut self, o: ObjRef, prop: &str, v: Value) {
            let a = self.addr(o, prop);
            self.space.write(o, a, v).unwrap();
        }

        /// A stack attribute's base and value set to `base` (as a fresh object has them).
        pub(crate) fn base(&mut self, o: ObjRef, prop: &str, base: Value) {
            self.set(o, &format!("{prop}BaseValue"), base.clone());
            self.set(o, prop, base);
        }

        /// The modifier objects on `prop`'s stack, in order.
        pub(crate) fn stack(&self, o: ObjRef, prop: &str) -> Vec<ObjRef> {
            match self.get(o, &format!("{prop}ModifierStack")) {
                Value::Array(items) => items
                    .iter()
                    .map(|v| match v {
                        Value::Object(Some(m)) => *m,
                        other => panic!("stack element {other:?}"),
                    })
                    .collect(),
                other => panic!("stack {other:?}"),
            }
        }

        pub(crate) fn modifier(&mut self, c: &Classes, kind: u8, value: f32) -> ObjRef {
            let m = self.obj(c.modifier, "AttributeModifier");
            self.set(m, "Type", Value::Byte(kind));
            self.set(m, "Value", Value::Float(value));
            m
        }

        /// A resolver object of the named-property class for property `prop`.
        pub(crate) fn resolver(&mut self, c: &Classes, prop: &str) -> ObjRef {
            let r = self.obj(c.named, "NamedPropertyValueResolver");
            let n = self.space.names.intern(prop);
            self.set(r, "PropertyName", Value::Name(n));
            r
        }

        /// An attribute definition with value chain `chain`.
        pub(crate) fn attribute(&mut self, c: &Classes, chain: &[Option<ObjRef>]) -> ObjRef {
            let a = self.obj(c.definition, "Att_Test");
            let items = chain.iter().map(|r| Value::Object(*r)).collect();
            self.set(a, "ValueResolverChain", Value::array(items));
            a
        }

        /// An attribute definition whose one resolver names `prop`.
        pub(crate) fn property_attribute(&mut self, c: &Classes, prop: &str) -> ObjRef {
            let r = self.resolver(c, prop);
            self.attribute(c, &[Some(r)])
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use ow_core::Missing;
    use ow_object::heap::ObjectSpace;
    use ow_object::ids::ObjRef;
    use ow_object::value::Value;

    use super::fixture::{Classes, World, system, world};
    use super::*;
    use crate::api::{AttrError, EvalCx, NumForms};
    use crate::stack::CombineForm;
    use crate::storage::Notified;

    const SCALE: u8 = 0;
    const PRE: u8 = 1;
    const POST: u8 = 2;

    fn not_yet(e: AttrError) -> Missing {
        match e {
            AttrError::NotYet(n) => n.missing,
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    /// A stand-in of the evaluator's two steps (W3-ATTREVAL's) for the loop tests: a constant initialization value
    /// (constant times scale; anything else is not these tests'), and a fixed table (attribute, source) -> context.
    /// It counts the value evaluations.
    struct TestSteps {
        contexts: Vec<(ObjRef, Option<ObjRef>, ObjRef)>,
        evaluated: Cell<usize>,
    }

    impl TestSteps {
        fn new(contexts: Vec<(ObjRef, Option<ObjRef>, ObjRef)>) -> Self {
            TestSteps {
                contexts,
                evaluated: Cell::new(0),
            }
        }
    }

    impl EvalSteps for TestSteps {
        fn value(
            &self,
            _sys: &AttributeSystem,
            data: &InitData,
            _source: Option<ObjRef>,
            _override_source: Option<ObjRef>,
            _cx: &mut EvalCx<'_>,
        ) -> Result<f32, AttrError> {
            self.evaluated.set(self.evaluated.get() + 1);
            assert!(
                data.base_value_attribute.is_none() && data.initialization_definition.is_none()
            );
            Ok(data.base_value_constant * data.base_value_scale_constant)
        }
        fn context(
            &self,
            _sys: &AttributeSystem,
            attr: ObjRef,
            source: Option<ObjRef>,
            _cx: &mut EvalCx<'_>,
        ) -> Result<Option<ObjRef>, AttrError> {
            Ok(self
                .contexts
                .iter()
                .find(|(a, s, _)| *a == attr && *s == source)
                .map(|(_, _, c)| *c))
        }
    }

    fn effect(attr: Option<ObjRef>, kind: ModifierType, value: f32) -> EffectData {
        EffectData::new(attr, kind, InitData::constant(value))
    }

    fn add(w: &mut World, attr: ObjRef, ctx: Option<ObjRef>, m: ObjRef) -> AddModifierOutcome {
        let sys = system();
        sys.add_modifier(attr, ctx, m, false, &mut EvalCx::new(w))
            .unwrap()
    }

    fn remove(
        w: &mut World,
        attr: ObjRef,
        ctx: Option<ObjRef>,
        m: ObjRef,
    ) -> RemoveModifierOutcome {
        let sys = system();
        sys.remove_modifier(attr, ctx, m, false, &mut EvalCx::new(w))
            .unwrap()
    }

    fn apply(
        w: &mut World,
        steps: &TestSteps,
        effects: &[EffectData],
        source: Option<ObjRef>,
        override_source: Option<ObjRef>,
    ) -> Applied {
        let sys = system();
        apply_with(
            &sys,
            steps,
            effects,
            source,
            override_source,
            &mut EvalCx::new(w),
        )
        .unwrap()
    }

    fn weapon(w: &mut World, c: &Classes) -> ObjRef {
        let gun = w.obj(c.weapon, "Gun");
        w.base(gun, "InstantHitDamage", Value::Float(10.0));
        w.base(gun, "ClipSize", Value::Int(12));
        gun
    }

    #[test]
    fn a_stack_add_recomputes_and_refuses_the_same_modifier_twice() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let attr = w.property_attribute(&c, "InstantHitDamage");
        let m = w.modifier(&c, PRE, 2.0);
        assert_eq!(add(&mut w, attr, Some(gun), m), AddModifierOutcome::Added);
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(12.0));
        assert_eq!(
            add(&mut w, attr, Some(gun), m),
            AddModifierOutcome::Duplicate
        );
        assert_eq!(w.stack(gun, "InstantHitDamage"), vec![m]);
        // The base companion is never the computed value.
        assert_eq!(w.get(gun, "InstantHitDamageBaseValue"), Value::Float(10.0));
        // A modifier object's Value is read at each recompute: a later change shows at the next one.
        w.set(m, "Value", Value::Float(4.0));
        let other = w.modifier(&c, POST, 1.0);
        add(&mut w, attr, Some(gun), other);
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(15.0));
    }

    #[test]
    fn a_remove_takes_every_copy_and_an_absent_modifier_is_false() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let attr = w.property_attribute(&c, "InstantHitDamage");
        let m = w.modifier(&c, PRE, 1.0);
        let keep = w.modifier(&c, POST, 0.5);
        // A stack loaded with one modifier twice (only the add refuses a duplicate).
        let items = vec![
            Value::Object(Some(m)),
            Value::Object(Some(keep)),
            Value::Object(Some(m)),
        ];
        w.set(gun, "InstantHitDamageModifierStack", Value::array(items));
        let removed = RemoveModifierOutcome::Removed { count: 2 };
        assert_eq!(remove(&mut w, attr, Some(gun), m), removed);
        assert_eq!(w.stack(gun, "InstantHitDamage"), vec![keep]);
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(10.5));
        let absent = remove(&mut w, attr, Some(gun), m);
        assert_eq!(absent, RemoveModifierOutcome::NotPresent);
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(10.5));
    }

    /// `weapon_gen_test.py` `test_modifier_stack_and_card_rounding` and
    /// `test_damage_stack_terms_rebuild_the_card_damage`: the weapon type's effects first, then the parts in slot
    /// order (body, grip). The type's PostAdd is `Weapon_Is_Acme * 3` there, an attribute read that is the
    /// evaluator's; here it is the constant 3 it evaluates to.
    #[test]
    fn weapon_part_effects_apply_in_slot_order() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let damage = w.property_attribute(&c, "InstantHitDamage");
        let clip = w.property_attribute(&c, "ClipSize");
        let type_effects = [effect(Some(damage), ModifierType::PostAdd, 3.0)];
        let body = [
            effect(Some(damage), ModifierType::Scale, -0.2),
            effect(Some(clip), ModifierType::Scale, -0.2),
        ];
        let grip = [
            effect(Some(damage), ModifierType::Scale, 0.5),
            effect(Some(damage), ModifierType::PreAdd, 2.0),
        ];
        let steps = TestSteps::new(vec![(damage, Some(gun), gun), (clip, Some(gun), gun)]);
        let mut records = Vec::new();
        for slot in [&type_effects[..], &body[..], &grip[..]] {
            records.extend(apply(&mut w, &steps, slot, Some(gun), None).0);
        }
        // (10 + 2) * (1 + 0.5) / (1 + 0.2f) + 3 = 17.99999996 -> 18; clip 12 / (1 + 0.2f) = 9.99999998 -> 9.
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(18.0));
        assert_eq!(w.get(gun, "ClipSize"), Value::Int(9));
        let order: Vec<ObjRef> = records
            .iter()
            .filter(|r| r.attribute == damage)
            .map(|r| r.modifier)
            .collect();
        assert_eq!(w.stack(gun, "InstantHitDamage"), order);
        assert_eq!(records.len(), 5);

        // Stage 8 picks the grip without effects: 10 / (1 + 0.2f) + 3 = 11.333333015.
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let damage = w.property_attribute(&c, "InstantHitDamage");
        let steps = TestSteps::new(vec![(damage, Some(gun), gun)]);
        let effects = [
            effect(Some(damage), ModifierType::PostAdd, 3.0),
            effect(Some(damage), ModifierType::Scale, -0.2),
        ];
        apply(&mut w, &steps, &effects, Some(gun), None);
        let Value::Float(late) = w.get(gun, "InstantHitDamage") else {
            panic!("a float attribute");
        };
        assert_eq!(f64::from(late), 11.333_333_015_441_895);
    }

    /// The recompute takes the caller's combine form: the VM's 5F form gives another clip size for the same stack.
    #[test]
    fn the_recompute_uses_the_callers_combine_form() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let clip = w.property_attribute(&c, "ClipSize");
        let m = w.modifier(&c, SCALE, -0.2);
        let forms = NumForms {
            combine: CombineForm::Vm5F,
            ..NumForms::default()
        };
        let sys = system();
        let mut cx = EvalCx::new(&mut w).with_forms(forms);
        sys.add_modifier(clip, Some(gun), m, true, &mut cx).unwrap();
        // All single precision: 12 * (1 / 1.2f) rounds to 10.0f, truncated to 10 (WeaponGen gives 9).
        assert_eq!(w.get(gun, "ClipSize"), Value::Int(10));
    }

    /// `weapon_gen_test.py` `test_accuracy_pool_goes_through_the_modifier_stack`: a part's PreAdd and Scale on the
    /// pool's minimum count; an effect aimed at another attribute does not land there.
    #[test]
    fn external_effects_land_on_the_pool_minimum() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let pool = w.obj(c.pool, "AccuracyPool");
        w.base(pool, "MinValue", Value::Float(2.0));
        w.base(pool, "MaxValue", Value::Float(10.0));
        let min = w.property_attribute(&c, "MinValue");
        let damage = w.property_attribute(&c, "InstantHitDamage");
        let effects = [
            effect(Some(min), ModifierType::PreAdd, 0.5),
            effect(Some(min), ModifierType::Scale, 0.5),
            effect(Some(damage), ModifierType::PreAdd, 5.0),
        ];
        let steps = TestSteps::new(vec![(min, Some(gun), pool), (damage, Some(gun), gun)]);
        apply(&mut w, &steps, &effects, Some(gun), None);
        // (2 + 0.5) x (1 + 0.5)
        assert_eq!(w.get(pool, "MinValue"), Value::Float(3.75));
        assert_eq!(w.get(pool, "MaxValue"), Value::Float(10.0));
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(15.0));
    }

    /// `skill_effects_test.py` `test_effect_values`: a Scale 0.25 on one attribute, a PostAdd whose value is a
    /// number times its scale 0.5 on another, and an entry without an attribute, which leaves no record. Each value is
    /// evaluated once, at apply time, and kept on the modifier object.
    #[test]
    fn skill_effect_entries_apply_in_order_and_skip_one_without_an_attribute() {
        let (mut w, c) = world();
        let controller = w.obj(c.object, "Controller");
        let gun = weapon(&mut w, &c);
        let a = w.property_attribute(&c, "InstantHitDamage");
        let b = w.property_attribute(&c, "ClipSize");
        let mut b_value = InitData::constant(1.0);
        b_value.base_value_scale_constant = 0.5;
        let effects = [
            effect(Some(a), ModifierType::Scale, 0.25),
            EffectData::new(Some(b), ModifierType::PostAdd, b_value),
            effect(None, ModifierType::Scale, 0.0),
        ];
        let steps = TestSteps::new(vec![(a, Some(controller), gun), (b, Some(controller), gun)]);
        let before = w.space.heap.len();
        let applied = apply(&mut w, &steps, &effects, Some(controller), None);
        assert_eq!(
            steps.evaluated.get(),
            3,
            "the entry without an attribute is evaluated too"
        );
        assert_eq!(
            w.space.heap.len(),
            before + 2,
            "no modifier for the skipped entry"
        );
        let got: Vec<(ObjRef, ObjRef)> =
            applied.0.iter().map(|r| (r.context, r.attribute)).collect();
        assert_eq!(got, vec![(gun, a), (gun, b)]);
        let (ma, mb) = (applied.0[0].modifier, applied.0[1].modifier);
        let fields = |w: &World, m| (w.get(m, "Type"), w.get(m, "Value"));
        assert_eq!(fields(&w, ma), (Value::Byte(SCALE), Value::Float(0.25)));
        assert_eq!(fields(&w, mb), (Value::Byte(POST), Value::Float(0.5)));
        assert_eq!(w.space.heap.get(ma).unwrap().outer, Some(controller));
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(12.5));
        // 12 + 0.5 on an int attribute: truncated, 12.
        assert_eq!(w.get(gun, "ClipSize"), Value::Int(12));
    }

    #[test]
    fn the_override_context_is_tried_first_then_the_source() {
        let (mut w, c) = world();
        let controller = w.obj(c.object, "Controller");
        let item = w.obj(c.object, "Item");
        let on_item = weapon(&mut w, &c);
        let on_controller = weapon(&mut w, &c);
        let item_attr = w.property_attribute(&c, "InstantHitDamage");
        let owner_attr = w.property_attribute(&c, "InstantHitDamage");
        let steps = TestSteps::new(vec![
            (item_attr, Some(item), on_item),
            (item_attr, Some(controller), on_controller),
            (owner_attr, Some(controller), on_controller),
        ]);
        let effects = [
            effect(Some(item_attr), ModifierType::PostAdd, 1.0),
            effect(Some(owner_attr), ModifierType::PostAdd, 2.0),
        ];
        let applied = apply(&mut w, &steps, &effects, Some(controller), Some(item));
        let contexts: Vec<ObjRef> = applied.0.iter().map(|r| r.context).collect();
        assert_eq!(contexts, vec![on_item, on_controller]);
        assert_eq!(w.get(on_item, "InstantHitDamage"), Value::Float(11.0));
        assert_eq!(w.get(on_controller, "InstantHitDamage"), Value::Float(12.0));
    }

    #[test]
    fn a_failed_entry_leaves_no_record_and_frees_its_modifier() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let unresolved = w.property_attribute(&c, "InstantHitDamage");
        let ro = w.obj(c.read_only, "Constant");
        let constant = w.attribute(&c, &[Some(ro)]);
        let steps = TestSteps::new(vec![(constant, Some(gun), gun)]);
        let effects = [
            effect(Some(unresolved), ModifierType::PostAdd, 1.0),
            effect(Some(constant), ModifierType::PostAdd, 1.0),
        ];
        let before = w.space.heap.len();
        let applied = apply(&mut w, &steps, &effects, Some(gun), None);
        assert!(applied.0.is_empty());
        assert_eq!(w.space.heap.len(), before);
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(10.0));
    }

    #[test]
    fn remove_effects_takes_the_modifiers_back_off() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let damage = w.property_attribute(&c, "InstantHitDamage");
        let steps = TestSteps::new(vec![(damage, Some(gun), gun)]);
        let effects = [
            effect(Some(damage), ModifierType::Scale, 0.5),
            effect(Some(damage), ModifierType::PostAdd, 1.0),
        ];
        let applied = apply(&mut w, &steps, &effects, Some(gun), None);
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(16.0));
        let sys = system();
        sys.remove_effects(applied, &mut EvalCx::new(&mut w))
            .unwrap();
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(10.0));
        assert!(w.stack(gun, "InstantHitDamage").is_empty());
    }

    /// NATIVE_ATTRIBUTES.md section 3, the correction to NATIVE_SKILLS: an effect on a plain attribute edits it in
    /// place, and taking the effect back (a respec, a deactivated skill) answers true and leaves the edit.
    #[test]
    fn a_respec_of_a_plain_attribute_keeps_the_edit() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        w.set(gun, "ReloadCnt", Value::Int(3));
        let reload = w.property_attribute(&c, "ReloadCnt");
        let steps = TestSteps::new(vec![(reload, Some(gun), gun)]);
        let effects = [effect(Some(reload), ModifierType::PreAdd, 2.5)];
        let applied = apply(&mut w, &steps, &effects, Some(gun), None);
        // 3 + 2.5 = 5.5 on an int: truncated to 5.
        assert_eq!(w.get(gun, "ReloadCnt"), Value::Int(5));
        let m = applied.0[0].modifier;
        let no_op = remove(&mut w, reload, Some(gun), m);
        assert_eq!(no_op, RemoveModifierOutcome::PlainNoOp);
        assert!(no_op.succeeded());
        let sys = system();
        sys.remove_effects(applied, &mut EvalCx::new(&mut w))
            .unwrap();
        assert_eq!(w.get(gun, "ReloadCnt"), Value::Int(5));
        // A plain attribute keeps no stack, so the same modifier edits it again.
        let again = add(&mut w, reload, Some(gun), m);
        assert_eq!(again, AddModifierOutcome::AppliedInPlace);
        assert_eq!(w.get(gun, "ReloadCnt"), Value::Int(7));
    }

    #[test]
    fn a_plain_scale_multiplies_raw_and_an_unknown_type_changes_nothing() {
        let (mut w, c) = world();
        let pool = w.obj(c.pool, "Pool");
        w.base(pool, "MaxValue", Value::Float(1000.0));
        w.set(pool, "CurrentValue", Value::Float(40.0));
        let current = w.property_attribute(&c, "CurrentValue");
        let half = w.modifier(&c, SCALE, 0.5);
        let done = add(&mut w, current, Some(pool), half);
        assert_eq!(done, AddModifierOutcome::AppliedInPlace);
        assert_eq!(
            w.get(pool, "CurrentValue"),
            Value::Float(20.0),
            "no 1 + scale"
        );
        let odd = w.modifier(&c, 7, 5.0);
        let done = add(&mut w, current, Some(pool), odd);
        assert_eq!(done, AddModifierOutcome::AppliedInPlace);
        assert_eq!(w.get(pool, "CurrentValue"), Value::Float(20.0));
        // On a stack the unknown type stays on the array and is left out of the sums.
        let gun = weapon(&mut w, &c);
        let damage = w.property_attribute(&c, "InstantHitDamage");
        assert_eq!(
            add(&mut w, damage, Some(gun), odd),
            AddModifierOutcome::Added
        );
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(10.0));
        assert_eq!(w.stack(gun, "InstantHitDamage"), vec![odd]);
    }

    /// NATIVE_ATTRIBUTES.md section 4: `ResourcePool` clamps `CurrentValue` after a notified change; below the
    /// minimum it becomes the minimum, at or above the maximum the maximum, the min test first.
    #[test]
    fn the_resource_pool_clamps_its_current_value_on_notification() {
        let (mut w, c) = world();
        let pool = w.obj(c.pool, "Pool");
        w.base(pool, "MinValue", Value::Float(0.0));
        w.base(pool, "MaxValue", Value::Float(100.0));
        w.set(pool, "CurrentValue", Value::Float(90.0));
        let current = w.property_attribute(&c, "CurrentValue");
        let plus = w.modifier(&c, POST, 25.0);
        add(&mut w, current, Some(pool), plus);
        assert_eq!(w.get(pool, "CurrentValue"), Value::Float(100.0));
        let clamp = Notified::PoolCurrentValue {
            before: 115.0,
            after: 100.0,
        };
        assert_eq!(w.notified, vec![(pool, "CurrentValue".to_string(), clamp)]);

        let negate = w.modifier(&c, SCALE, -1.0);
        add(&mut w, current, Some(pool), negate);
        assert_eq!(
            w.get(pool, "CurrentValue"),
            Value::Float(0.0),
            "-100 is below the minimum"
        );

        // Suppressed: no notification, so no clamp.
        let sys = system();
        for _ in 0..2 {
            let mut cx = EvalCx::new(&mut w);
            sys.add_modifier(current, Some(pool), plus, true, &mut cx)
                .unwrap();
        }
        assert_eq!(w.get(pool, "CurrentValue"), Value::Float(50.0));
        assert_eq!(w.notified.len(), 2);

        // A minimum above the maximum wins. A change of the maximum is not the clamp's (no notify flag there).
        w.base(pool, "MinValue", Value::Float(80.0));
        w.base(pool, "MaxValue", Value::Float(60.0));
        let zero = w.modifier(&c, POST, 0.0);
        add(&mut w, current, Some(pool), zero);
        assert_eq!(w.get(pool, "CurrentValue"), Value::Float(80.0));
        let max = w.property_attribute(&c, "MaxValue");
        add(&mut w, max, Some(pool), plus);
        assert_eq!(w.get(pool, "CurrentValue"), Value::Float(80.0));
        assert_eq!(w.notified.len(), 3);
    }

    #[test]
    fn a_skill_grade_change_flags_the_refresh_and_other_owners_are_not_read() {
        let (mut w, c) = world();
        let skill = w.obj(c.skill, "Skill");
        w.base(skill, "Grade", Value::Int(1));
        let grade = w.property_attribute(&c, "Grade");
        let bonus = w.modifier(&c, POST, 1.0);
        add(&mut w, grade, Some(skill), bonus);
        assert_eq!(w.get(skill, "Grade"), Value::Int(2));
        assert_eq!(
            w.get(skill, "bForceRefreshModifiersNextTick"),
            Value::Bool(true)
        );
        let removed = remove(&mut w, grade, Some(skill), bonus);
        assert_eq!(removed, RemoveModifierOutcome::Removed { count: 1 });
        // The weapon's ClipSize has the flag, but whether its class overrides the virtual is not read: counted, no
        // effect. InstantHitDamage has no flag: no notification at all.
        let gun = weapon(&mut w, &c);
        let clip = w.property_attribute(&c, "ClipSize");
        let damage = w.property_attribute(&c, "InstantHitDamage");
        let m = w.modifier(&c, PRE, 1.0);
        add(&mut w, clip, Some(gun), m);
        add(&mut w, damage, Some(gun), m);
        let seen: Vec<(String, Notified)> =
            w.notified.iter().map(|(_, n, d)| (n.clone(), *d)).collect();
        let flagged = ("Grade".to_string(), Notified::SkillRefreshFlagged);
        let unread = ("ClipSize".to_string(), Notified::NotRead);
        assert_eq!(seen, vec![flagged.clone(), flagged, unread]);
    }

    #[test]
    fn the_chain_answers_by_its_last_resolver_without_rollback() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let m = w.modifier(&c, PRE, 1.0);
        let named = w.resolver(&c, "InstantHitDamage");
        let ro = w.obj(c.read_only, "Constant");
        let unread = w.obj(c.unread, "Unread");
        let refused = AddModifierOutcome::Refused;

        let attr = w.property_attribute(&c, "InstantHitDamage");
        assert_eq!(add(&mut w, attr, None, m), refused(Refusal::NoContext));
        let empty = w.attribute(&c, &[]);
        let got = add(&mut w, empty, Some(gun), m);
        assert_eq!(got, refused(Refusal::NoValueResolvers));
        let null = w.attribute(&c, &[None, Some(named)]);
        assert_eq!(
            add(&mut w, null, Some(gun), m),
            refused(Refusal::NullResolver)
        );
        let constant = w.attribute(&c, &[Some(ro)]);
        assert_eq!(
            add(&mut w, constant, Some(gun), m),
            refused(Refusal::NotWritable)
        );
        let got = remove(&mut w, constant, Some(gun), m);
        assert_eq!(got, RemoveModifierOutcome::Refused(Refusal::NotWritable));
        // A property that is not an attribute: the property-level answer is false.
        let spread = w.property_attribute(&c, "Spread");
        assert_eq!(
            add(&mut w, spread, Some(gun), m),
            refused(Refusal::NotWritable)
        );
        // An unread resolver class is NotYet, not a false.
        let odd = w.attribute(&c, &[Some(unread)]);
        let sys = system();
        let e = sys
            .add_modifier(odd, Some(gun), m, false, &mut EvalCx::new(&mut w))
            .unwrap_err();
        assert_eq!(not_yet(e), Missing::NotRead);
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(10.0));

        // Writable, then read-only: the chain fails, and the first resolver's add stays.
        let two = w.attribute(&c, &[Some(named), Some(ro)]);
        assert_eq!(
            add(&mut w, two, Some(gun), m),
            refused(Refusal::NotWritable)
        );
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(11.0));
        assert_eq!(w.stack(gun, "InstantHitDamage"), vec![m]);
    }

    #[test]
    fn a_missing_stack_and_a_bad_modifier_are_data_errors_that_write_nothing() {
        let (mut w, c) = world();
        let gun = weapon(&mut w, &c);
        let m = w.modifier(&c, PRE, 1.0);
        let sys = system();
        let broken = w.property_attribute(&c, "Broken");
        let e = sys
            .add_modifier(broken, Some(gun), m, false, &mut EvalCx::new(&mut w))
            .unwrap_err();
        let stack_missing = matches!(&e, AttrError::BadData { reason } if reason.contains("Modifier stack not found"));
        assert!(stack_missing, "{e:?}");
        let flagged = w.property_attribute(&c, "Flagged");
        let e = sys
            .add_modifier(flagged, Some(gun), m, false, &mut EvalCx::new(&mut w))
            .unwrap_err();
        assert_eq!(not_yet(e), Missing::NotRead);
        // An object of another class handed in as the modifier: refused before anything is written.
        let stranger = w.obj(c.object, "Stranger");
        let damage = w.property_attribute(&c, "InstantHitDamage");
        let e = sys
            .add_modifier(damage, Some(gun), stranger, false, &mut EvalCx::new(&mut w))
            .unwrap_err();
        assert!(matches!(e, AttrError::BadData { .. }));
        assert!(w.stack(gun, "InstantHitDamage").is_empty());
        // Not an attribute definition at all.
        let e = sys
            .add_modifier(gun, Some(gun), m, false, &mut EvalCx::new(&mut w))
            .unwrap_err();
        assert!(matches!(e, AttrError::BadData { .. }));
    }

    #[test]
    fn set_base_writes_every_context_and_stops_at_the_first_failure() {
        let (mut w, c) = world();
        let one = weapon(&mut w, &c);
        let two = weapon(&mut w, &c);
        let damage = w.property_attribute(&c, "InstantHitDamage");
        let m = w.modifier(&c, POST, 1.0);
        add(&mut w, damage, Some(one), m);
        let sys = system();
        let set = |w: &mut World, attr, contexts: &[ObjRef], v| {
            set_base_in(&sys, attr, contexts, v, false, &mut EvalCx::new(w)).unwrap()
        };
        assert!(set(&mut w, damage, &[one, two], 20.0));
        assert_eq!(w.get(one, "InstantHitDamageBaseValue"), Value::Float(20.0));
        assert_eq!(
            w.get(one, "InstantHitDamage"),
            Value::Float(21.0),
            "recomputed from the stack"
        );
        assert_eq!(w.get(two, "InstantHitDamage"), Value::Float(20.0));
        // An int base truncates; the Grade change notifies.
        let skill = w.obj(c.skill, "Skill");
        let grade = w.property_attribute(&c, "Grade");
        assert!(set(&mut w, grade, &[skill], 3.9));
        let grade_now = (w.get(skill, "GradeBaseValue"), w.get(skill, "Grade"));
        assert_eq!(grade_now, (Value::Int(3), Value::Int(3)));
        assert_eq!(
            w.get(skill, "bForceRefreshModifiersNextTick"),
            Value::Bool(true)
        );
        // No context, no resolver: false. A read-only second resolver fails after the first resolver's set, which
        // stays.
        assert!(!set(&mut w, damage, &[], 1.0));
        let empty = w.attribute(&c, &[]);
        assert!(!set(&mut w, empty, &[one], 1.0));
        let named = w.resolver(&c, "InstantHitDamage");
        let ro = w.obj(c.read_only, "Constant");
        let mixed = w.attribute(&c, &[Some(named), Some(ro)]);
        assert!(!set(&mut w, mixed, &[two], 5.0));
        assert_eq!(w.get(two, "InstantHitDamage"), Value::Float(5.0));
        // A plain attribute: the number itself, then the pool's clamp.
        let pool = w.obj(c.pool, "Pool");
        w.base(pool, "MaxValue", Value::Float(100.0));
        let current = w.property_attribute(&c, "CurrentValue");
        assert!(set(&mut w, current, &[pool], 250.0));
        assert_eq!(w.get(pool, "CurrentValue"), Value::Float(100.0));
    }

    #[test]
    fn a_world_without_the_modifier_class_cannot_make_one() {
        let (mut w, _) = world();
        let mut bare = ObjectSpace::new();
        bare.classes.define("Core.Object", None, vec![]).unwrap();
        w.space = bare;
        let e = new_modifier(&mut EvalCx::new(&mut w), None, ModifierType::Scale, 1.0).unwrap_err();
        let named = matches!(&e, AttrError::BadData { reason } if reason.contains(ATTRIBUTE_MODIFIER_CLASS));
        assert!(named, "{e:?}");
    }
}
