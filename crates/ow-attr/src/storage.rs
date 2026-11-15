//! Owner: lane W3-ATTREFFECTS. The property-level rules (NATIVE_ATTRIBUTES.md section 3): what an add, a remove and
//! a set-base do to the stored numbers of a stack or plain attribute, the change notification that follows
//! (section 4), the by-name `Object` natives (section 1) and the script VM's typed let (opcode 0x5F).
//!
//! Every rule here is read from native code and is **UNVERIFIED in the running game**.
//!
//! # The stored numbers
//!
//! A **stack attribute** is a triple of properties on its owner: the value (`X`, the property with the stack flag),
//! its base (`XBaseValue`) and its modifier stack (`XModifierStack`, an array of `Core.AttributeModifier`
//! references). The value is always the base combined with the whole stack ([`crate::stack::combine`]), recomputed
//! after every change here; nothing recomputes it lazily. Each modifier object's `Type` and `Value` are read at each
//! recompute, as the C++ recompute reads them (`interp.cpp:546-561`).
//!
//! A **plain attribute** (the plain flag, no stack) is one number edited in place: an add changes it permanently
//! ([`crate::stack::plain_apply`]), a remove is a successful no-op, a set-base writes it.
//!
//! Every operation computes its result first and writes after, so an error (a modifier object without `Type`, a
//! stale reference in a stack) leaves the owner as it was.

use std::rc::Rc;

use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef, PropAddr, PropId};
use ow_object::layout::{AttrCompanions, PropKind, PropertyDecl};
use ow_object::value::Value;

use crate::api::{
    AddModifierOutcome, AttrError, EvalCx, Refusal, RemoveModifierOutcome, Storage, modifier_id_of,
};
use crate::registry::AttributeSystem;
use crate::stack::{
    CombineForm, Combined, Modifier, ModifierType, NumKind, combine, plain_apply, store_as,
};

/// The class of the modifier objects a stack holds and `ApplyAttributeEffects` creates (NATIVE_WEAPON_RULES.md
/// section 1, NATIVE_SKILLS.md section 4).
pub const ATTRIBUTE_MODIFIER_CLASS: &str = "Core.AttributeModifier";

fn bad(reason: String) -> AttrError {
    AttrError::BadData {
        reason: reason.into(),
    }
}

/// The text of name `n`, for messages.
fn name_text(space: &ObjectSpace, n: NameId) -> String {
    space.names.text(n).into_owned()
}

/// The declaration `p` names, or a layout error.
fn decl_of(space: &ObjectSpace, p: PropId) -> Result<&PropertyDecl, AttrError> {
    space
        .classes
        .decl(p)
        .ok_or_else(|| ow_object::layout::LayoutError::NotAProperty(p).into())
}

/// The numeric kind of an attribute property's stored number, from its declared kind. `None` for a kind that holds
/// no number (a plain-flagged bool, a struct).
fn num_kind(kind: &PropKind) -> Option<NumKind> {
    match kind {
        PropKind::Float | PropKind::FloatAttribute => Some(NumKind::Float),
        PropKind::Int | PropKind::IntAttribute => Some(NumKind::Int),
        PropKind::Byte { .. } | PropKind::ByteAttribute { .. } => Some(NumKind::Byte),
        _ => None,
    }
}

fn to_value(c: Combined) -> Value {
    match c {
        Combined::Float(x) => Value::Float(x),
        Combined::Int(i) => Value::Int(i),
        Combined::Byte(b) => Value::Byte(b),
    }
}

/// A stored number as a float, as the game's "read value" gives it (int and byte converted).
fn number(v: &Value, what: &str) -> Result<f32, AttrError> {
    match v {
        Value::Float(x) => Ok(*x),
        Value::Int(i) => Ok(Combined::Int(*i).to_f32()),
        Value::Byte(b) => Ok(f32::from(*b)),
        other => Err(bad(format!(
            "{what} holds a {}, not a number",
            other.kind_name()
        ))),
    }
}

fn read_number(
    space: &ObjectSpace,
    owner: ObjRef,
    p: PropId,
    what: &str,
) -> Result<f32, AttrError> {
    number(space.read(owner, PropAddr::scalar(p))?, what)
}

/// The property `name` (any case) on the class of `o`, own or inherited.
fn prop_named(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<PropId>, AttrError> {
    let class = space.heap.get(o)?.class;
    Ok(space.prop(class, name))
}

/// The modifier object `m` as the stack sums see it: its `Type` and `Value`. `Ok(None)` for a type outside 0..=2,
/// which the recompute ignores (`interp.cpp`, "anything else ignored") and the plain add does nothing with. An object
/// without `Type` or `Value` is refused (the C++ logs and skips it; a modifier object of another class is a data bug).
fn read_modifier(space: &ObjectSpace, m: ObjRef) -> Result<Option<Modifier>, AttrError> {
    let missing = || {
        bad(format!(
            "attribute modifier {m:?} has no Type or Value property"
        ))
    };
    let type_prop = prop_named(space, m, "Type")?.ok_or_else(missing)?;
    let value_prop = prop_named(space, m, "Value")?.ok_or_else(missing)?;
    let raw = match space.read(m, PropAddr::scalar(type_prop))? {
        Value::Byte(b) => i64::from(*b),
        Value::Int(i) => i64::from(*i),
        other => {
            return Err(bad(format!(
                "attribute modifier {m:?} has a {} Type",
                other.kind_name()
            )));
        }
    };
    let value = match space.read(m, PropAddr::scalar(value_prop))? {
        Value::Float(x) => *x,
        other => {
            return Err(bad(format!(
                "attribute modifier {m:?} has a {} Value",
                other.kind_name()
            )));
        }
    };
    Ok(ModifierType::from_raw(raw).map(|kind| Modifier {
        kind,
        value,
        id: modifier_id_of(m),
    }))
}

/// The stack array's elements on `owner`.
fn stack_items(
    space: &ObjectSpace,
    owner: ObjRef,
    stack: PropId,
) -> Result<Rc<Vec<Value>>, AttrError> {
    match space.read(owner, PropAddr::scalar(stack))? {
        Value::Array(items) => Ok(items.clone()),
        other => Err(bad(format!(
            "modifier stack holds a {}, not an array",
            other.kind_name()
        ))),
    }
}

/// The value of a stack attribute with base `base` and stack `items`, in `kind` and `form`. A null element is
/// skipped, as the C++ recompute skips it.
fn stack_value(
    space: &ObjectSpace,
    base: f32,
    items: &[Value],
    form: CombineForm,
    kind: NumKind,
) -> Result<Value, AttrError> {
    let mut mods = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Value::Object(Some(m)) => mods.extend(read_modifier(space, *m)?),
            Value::Object(None) => {}
            other => {
                return Err(bad(format!(
                    "modifier stack element is a {}, not an object",
                    other.kind_name()
                )));
            }
        }
    }
    Ok(to_value(combine(base, &mods, form, kind)))
}

/// Writes a new stack (when given) and the recomputed value; both are computed before anything is written.
fn write_stack(
    space: &mut ObjectSpace,
    owner: ObjRef,
    c: AttrCompanions,
    kind: NumKind,
    items: Option<Vec<Value>>,
    form: CombineForm,
) -> Result<(), AttrError> {
    let base = read_number(space, owner, c.base, "the base companion")?;
    let value = match &items {
        Some(new) => stack_value(space, base, new, form, kind)?,
        None => stack_value(
            space,
            base,
            &stack_items(space, owner, c.stack)?,
            form,
            kind,
        )?,
    };
    if let Some(new) = items {
        space.write(owner, PropAddr::scalar(c.stack), Value::array(new))?;
    }
    space.write(owner, PropAddr::scalar(c.value), value)?;
    Ok(())
}

/// Fires the change notification for `prop` on `owner` when it was not suppressed and the property carries the
/// notify flag (NATIVE_ATTRIBUTES.md section 4; checklist item 5 of section 3).
fn notify(
    owner: ObjRef,
    prop: PropId,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<(), AttrError> {
    if suppress_notify {
        return Ok(());
    }
    let notifies = decl_of(cx.world.parts().space, prop)?.notifies();
    if notifies {
        cx.world.attribute_changed(owner, prop, cx.forms)?;
    }
    Ok(())
}

/// The property a storage notifies for: a stack attribute's value property, or the plain property.
fn notify_prop(s: Storage) -> (ObjRef, PropId) {
    match s {
        Storage::Stack {
            owner, companions, ..
        } => (owner, companions.value),
        Storage::Plain { owner, prop, .. } => (owner, prop),
    }
}

/// The property-level add (section 3) on `s`: a stack attribute refuses a modifier object already on it, else
/// appends it and recomputes; a plain attribute is edited in place (a type outside 0..=2 changes nothing and still
/// succeeds, UNVERIFIED). Notifies after a success.
pub(crate) fn add_to(
    s: Storage,
    modifier: ObjRef,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<AddModifierOutcome, AttrError> {
    let forms = cx.forms;
    let outcome = {
        let space: &mut ObjectSpace = cx.world.parts().space;
        let m = read_modifier(space, modifier)?;
        match s {
            Storage::Stack {
                owner,
                companions,
                kind,
            } => {
                let items = stack_items(space, owner, companions.stack)?;
                let held = Value::Object(Some(modifier));
                if items.contains(&held) {
                    AddModifierOutcome::Duplicate
                } else {
                    let mut new = (*items).clone();
                    new.push(held);
                    write_stack(space, owner, companions, kind, Some(new), forms.combine)?;
                    AddModifierOutcome::Added
                }
            }
            Storage::Plain { owner, prop, kind } => {
                if let Some(m) = m {
                    let stored = read_number(space, owner, prop, "the plain attribute")?;
                    let edited = to_value(store_as(plain_apply(stored, &m), kind));
                    space.write(owner, PropAddr::scalar(prop), edited)?;
                }
                AddModifierOutcome::AppliedInPlace
            }
        }
    };
    if outcome.succeeded() {
        let (owner, prop) = notify_prop(s);
        notify(owner, prop, suppress_notify, cx)?;
    }
    Ok(outcome)
}

/// The property-level remove (section 3) on `s`: a stack attribute loses **every** copy of the modifier object and
/// is recomputed (not present: false); a plain attribute answers true and changes nothing. Notifies after a stack
/// removal; a plain no-op changes nothing, so it does not notify (UNVERIFIED: the record says the notification
/// follows "a successful change").
pub(crate) fn remove_from(
    s: Storage,
    modifier: ObjRef,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<RemoveModifierOutcome, AttrError> {
    let forms = cx.forms;
    let Storage::Stack {
        owner,
        companions,
        kind,
    } = s
    else {
        return Ok(RemoveModifierOutcome::PlainNoOp);
    };
    let count = {
        let space: &mut ObjectSpace = cx.world.parts().space;
        let items = stack_items(space, owner, companions.stack)?;
        let held = Value::Object(Some(modifier));
        let kept: Vec<Value> = items.iter().filter(|v| **v != held).cloned().collect();
        let count = items.len() - kept.len();
        if count > 0 {
            write_stack(space, owner, companions, kind, Some(kept), forms.combine)?;
        }
        count
    };
    if count == 0 {
        return Ok(RemoveModifierOutcome::NotPresent);
    }
    notify(owner, companions.value, suppress_notify, cx)?;
    Ok(RemoveModifierOutcome::Removed { count })
}

/// The property-level set-base (section 3) on `s`: a stack attribute's base companion is written (converted to its
/// kind as a store converts, [`store_as`]) and the value recomputed from the stack; a plain attribute's number is
/// written. Notifies.
pub(crate) fn set_base_of(
    s: Storage,
    v: f32,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<(), AttrError> {
    let forms = cx.forms;
    {
        let space: &mut ObjectSpace = cx.world.parts().space;
        match s {
            Storage::Stack {
                owner,
                companions,
                kind,
            } => {
                let base_kind = num_kind(&decl_of(space, companions.base)?.kind)
                    .ok_or_else(|| bad("the base companion does not hold a number".to_string()))?;
                let base = to_value(store_as(v, base_kind));
                let items = stack_items(space, owner, companions.stack)?;
                let value =
                    stack_value(space, number(&base, "base")?, &items, forms.combine, kind)?;
                space.write(owner, PropAddr::scalar(companions.base), base)?;
                space.write(owner, PropAddr::scalar(companions.value), value)?;
            }
            Storage::Plain { owner, prop, kind } => {
                space.write(owner, PropAddr::scalar(prop), to_value(store_as(v, kind)))?;
            }
        }
    }
    let (owner, prop) = notify_prop(s);
    notify(owner, prop, suppress_notify, cx)
}

/// Where the attribute property `p` of `owner` keeps its number: a stack attribute (the stack flag, with its
/// companions), a plain attribute (the plain flag), or `None` for a property that is neither (every operation then
/// answers false). Used by the by-name natives; a value resolver that names a property may use it too.
///
/// A stack-flagged property without companions is the record's "Modifier stack not found for attribute", a data
/// bug: [`AttrError::BadData`]. So is an attribute flag on a property that holds no number (a bool, say), as
/// W3-ATTREVAL's `ObjectPropertyAttributeValueResolver` answers it.
pub fn storage_of(
    space: &ObjectSpace,
    owner: ObjRef,
    p: PropId,
) -> Result<Option<Storage>, AttrError> {
    let decl = decl_of(space, p)?;
    let not_a_number = || {
        bad(format!(
            "{}: an attribute flag on a {} property",
            name_text(space, decl.name),
            decl.kind.kind_name()
        ))
    };
    if decl.is_stack_attribute() {
        let companions = match decl.attr {
            Some(c) if c.value == p => c,
            _ => {
                return Err(bad(format!(
                    "Modifier stack not found for attribute {}",
                    name_text(space, decl.name)
                )));
            }
        };
        let kind = num_kind(&decl.kind).ok_or_else(not_a_number)?;
        return Ok(Some(Storage::Stack {
            owner,
            companions,
            kind,
        }));
    }
    if decl.is_plain_attribute() {
        let kind = num_kind(&decl.kind).ok_or_else(not_a_number)?;
        return Ok(Some(Storage::Plain {
            owner,
            prop: p,
            kind,
        }));
    }
    Ok(None)
}

/// The storage of `object`'s property `name`. The lookup walks the class chain, nearest declaration first: the
/// record read the helper on the object's own class and notes that the data needs inherited properties (its Open
/// item), so the inherited walk is UNVERIFIED.
fn storage_by_name(
    space: &ObjectSpace,
    object: ObjRef,
    name: NameId,
) -> Result<Option<Storage>, AttrError> {
    let class = space.heap.get(object)?.class;
    match space.classes.prop(class, name) {
        Some(p) => storage_of(space, object, p),
        None => Ok(None),
    }
}

/// [`AttributeSystem::add_modifier_by_name`]: the property-level add on `object`'s property, no definition, no
/// chains. A name that is not an attribute: `Refused(NotAnAttribute)`, the native's false.
pub(crate) fn add_modifier_by_name(
    sys: &AttributeSystem,
    object: ObjRef,
    modifier: ObjRef,
    attribute_name: NameId,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<AddModifierOutcome, AttrError> {
    let _ = sys;
    match storage_by_name(cx.world.parts().space, object, attribute_name)? {
        Some(s) => add_to(s, modifier, suppress_notify, cx),
        None => Ok(AddModifierOutcome::Refused(Refusal::NotAnAttribute)),
    }
}

/// [`AttributeSystem::remove_modifier_by_name`].
pub(crate) fn remove_modifier_by_name(
    sys: &AttributeSystem,
    object: ObjRef,
    modifier: ObjRef,
    attribute_name: NameId,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<RemoveModifierOutcome, AttrError> {
    let _ = sys;
    match storage_by_name(cx.world.parts().space, object, attribute_name)? {
        Some(s) => remove_from(s, modifier, suppress_notify, cx),
        None => Ok(RemoveModifierOutcome::Refused(Refusal::NotAnAttribute)),
    }
}

/// [`AttributeSystem::attribute_value_by_name`]: the stored number (a stack attribute's value property, a plain
/// attribute's own), 0.0 when the name is not an attribute.
pub(crate) fn attribute_value_by_name(
    sys: &AttributeSystem,
    object: ObjRef,
    attribute_name: NameId,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let _ = sys;
    let space = cx.world.parts().space;
    match storage_by_name(space, object, attribute_name)? {
        Some(Storage::Stack {
            owner, companions, ..
        }) => read_number(space, owner, companions.value, "the attribute value"),
        Some(Storage::Plain { owner, prop, .. }) => {
            read_number(space, owner, prop, "the plain attribute")
        }
        None => Ok(0.0),
    }
}

/// [`crate::registry::let_base`]: refuses an `rhs` whose kind differs from the base companion's
/// (`AttrError::BadData`), converts nothing, keeps an integer exact. The value is recomputed from the new base and the
/// stack in `form`; nothing notifies. Both are computed before either is written.
pub(crate) fn let_base(
    space: &mut ObjectSpace,
    owner: ObjRef,
    companions: AttrCompanions,
    rhs: Value,
    form: CombineForm,
) -> Result<(), AttrError> {
    let value_decl = decl_of(space, companions.value)?;
    let kind = match (value_decl.kind.is_attribute(), num_kind(&value_decl.kind)) {
        (true, Some(k)) => k,
        _ => {
            return Err(bad(format!(
                "let attribute: {} is not an attribute value property",
                name_text(space, value_decl.name)
            )));
        }
    };
    let base_decl = decl_of(space, companions.base)?;
    let fits = matches!(
        (num_kind(&base_decl.kind), &rhs),
        (Some(NumKind::Float), Value::Float(_))
            | (Some(NumKind::Int), Value::Int(_))
            | (Some(NumKind::Byte), Value::Byte(_))
    );
    if !fits {
        return Err(bad(format!(
            "let attribute: a {} value for the {} base {}; the VM converts the right side first",
            rhs.kind_name(),
            base_decl.kind.kind_name(),
            name_text(space, base_decl.name)
        )));
    }
    let items = stack_items(space, owner, companions.stack)?;
    // interp.cpp:557 reads the base as float(base.number()): an int above 2^24 rounds here, not in the base.
    let value = stack_value(space, number(&rhs, "base")?, &items, form, kind)?;
    space.write(owner, PropAddr::scalar(companions.base), rhs)?;
    space.write(owner, PropAddr::scalar(companions.value), value)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The owner's notification override (NATIVE_ATTRIBUTES.md section 4)
// ---------------------------------------------------------------------------------------------------------------

/// What [`owner_notification`] did.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Notified {
    /// A `ResourcePool`'s `CurrentValue` changed and was clamped to `[MinValue, MaxValue]`, min test first
    /// (`before` and `after` equal when it was inside).
    PoolCurrentValue { before: f32, after: f32 },
    /// A `Skill`'s `Grade` changed: its `bForceRefreshModifiersNextTick` is now set.
    SkillRefreshFlagged,
    /// A known owner, another property: its override does nothing for it.
    NoEffect,
    /// The owner's class is neither of the two read overrides. Whether it overrides the virtual is not read
    /// (real-game question 11): the world counts it and nothing changes.
    NotRead,
}

/// Whether a class in `o`'s class chain (its own included) is named `name`, any case.
fn class_chain_has(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<bool, AttrError> {
    let mut cursor = Some(space.heap.get(o)?.class);
    while let Some(c) = cursor {
        let Some(layout) = space.classes.get_layout(c) else {
            break;
        };
        if layout.path().name().eq_ignore_ascii_case(name) {
            return Ok(true);
        }
        cursor = layout.super_class();
    }
    Ok(false)
}

/// The two notification overrides the record read, for an [`crate::api::AttrWorld::attribute_changed`]
/// implementation to call: the rule lives here, below the VM (architecture R2), and the world decides what else to
/// run. Status: UNVERIFIED (NATIVE_ATTRIBUTES.md section 4).
///
/// - `ResourcePool` (or a subclass: a virtual override is inherited; that no subclass overrides it again is
///   UNVERIFIED), property `CurrentValue`: below `MinValue` it becomes `MinValue`; at or above `MaxValue` it becomes
///   `MaxValue`; else unchanged. The min test is first, so a min above the max wins.
/// - `Skill` (or a subclass), property `Grade`: sets `bForceRefreshModifiersNextTick`.
/// - Any other owner: [`Notified::NotRead`], nothing changes.
///
/// Classes are matched by class name, not package, so the package each lives in need not be assumed. The write of
/// the clamp does not notify again.
pub fn owner_notification(
    space: &mut ObjectSpace,
    owner: ObjRef,
    value_prop: PropId,
) -> Result<Notified, AttrError> {
    let changed = decl_of(space, value_prop)?.name;
    let changed = space.names.text(changed).into_owned();
    if class_chain_has(space, owner, "ResourcePool")? {
        if !changed.eq_ignore_ascii_case("CurrentValue") {
            return Ok(Notified::NoEffect);
        }
        let prop = |space: &ObjectSpace, n: &str| {
            prop_named(space, owner, n)?.ok_or_else(|| bad(format!("a ResourcePool without {n}")))
        };
        let (current, min, max) = (
            prop(space, "CurrentValue")?,
            prop(space, "MinValue")?,
            prop(space, "MaxValue")?,
        );
        let before = read_number(space, owner, current, "CurrentValue")?;
        let low = read_number(space, owner, min, "MinValue")?;
        let high = read_number(space, owner, max, "MaxValue")?;
        let after = if before < low {
            low
        } else if before >= high {
            high
        } else {
            before
        };
        if after.to_bits() != before.to_bits() {
            let kind = num_kind(&decl_of(space, current)?.kind).ok_or_else(|| {
                bad("a ResourcePool CurrentValue that holds no number".to_string())
            })?;
            space.write(
                owner,
                PropAddr::scalar(current),
                to_value(store_as(after, kind)),
            )?;
        }
        return Ok(Notified::PoolCurrentValue { before, after });
    }
    if class_chain_has(space, owner, "Skill")? {
        if !changed.eq_ignore_ascii_case("Grade") {
            return Ok(Notified::NoEffect);
        }
        let flag = prop_named(space, owner, "bForceRefreshModifiersNextTick")?
            .ok_or_else(|| bad("a Skill without bForceRefreshModifiersNextTick".to_string()))?;
        space.write(owner, PropAddr::scalar(flag), Value::Bool(true))?;
        return Ok(Notified::SkillRefreshFlagged);
    }
    Ok(Notified::NotRead)
}

#[cfg(test)]
mod tests {
    use ow_object::layout::AttrCompanions;
    use ow_object::value::Value;

    use super::*;
    use crate::effects::fixture::{World, system, world};

    const PRE: u8 = 1;
    const POST: u8 = 2;

    fn companions(w: &World, o: ObjRef, prop: &str) -> AttrCompanions {
        let class = w.space.heap.get(o).unwrap().class;
        let p = w.space.prop(class, prop).unwrap();
        w.space.classes.decl(p).unwrap().attr.unwrap()
    }

    #[test]
    fn the_by_name_natives_reach_the_property_without_a_definition() {
        let (mut w, c) = world();
        let gun = w.obj(c.weapon, "Gun");
        w.base(gun, "InstantHitDamage", Value::Float(10.0));
        let m = w.modifier(&c, PRE, 2.0);
        let name = |w: &mut World, s: &str| w.space.names.intern(s);
        let (damage, spread, nope) = (
            name(&mut w, "InstantHitDamage"),
            name(&mut w, "Spread"),
            name(&mut w, "NoSuchProperty"),
        );
        let sys = system();
        let mut cx = EvalCx::new(&mut w);
        let added = sys
            .add_modifier_by_name(gun, m, damage, false, &mut cx)
            .unwrap();
        assert_eq!(added, AddModifierOutcome::Added);
        assert_eq!(sys.attribute_value_by_name(gun, damage, &mut cx), Ok(12.0));
        let again = sys
            .add_modifier_by_name(gun, m, damage, false, &mut cx)
            .unwrap();
        assert_eq!(again, AddModifierOutcome::Duplicate);
        let removed = sys
            .remove_modifier_by_name(gun, m, damage, false, &mut cx)
            .unwrap();
        assert_eq!(removed, RemoveModifierOutcome::Removed { count: 1 });
        assert_eq!(sys.attribute_value_by_name(gun, damage, &mut cx), Ok(10.0));
        // Not an attribute, or no property of that name: false, and a read of 0.
        for n in [spread, nope] {
            let refused = AddModifierOutcome::Refused(Refusal::NotAnAttribute);
            assert_eq!(
                sys.add_modifier_by_name(gun, m, n, false, &mut cx),
                Ok(refused)
            );
            let refused = RemoveModifierOutcome::Refused(Refusal::NotAnAttribute);
            assert_eq!(
                sys.remove_modifier_by_name(gun, m, n, false, &mut cx),
                Ok(refused)
            );
            assert_eq!(sys.attribute_value_by_name(gun, n, &mut cx), Ok(0.0));
        }
    }

    /// The base companion carries the plain flag, so by name it is a plain attribute: an add edits it in place and
    /// does not recompute the value (section 3 read literally; UNVERIFIED that any caller does this).
    #[test]
    fn by_name_the_base_companion_is_a_plain_attribute() {
        let (mut w, c) = world();
        let gun = w.obj(c.weapon, "Gun");
        w.base(gun, "InstantHitDamage", Value::Float(10.0));
        let m = w.modifier(&c, POST, 1.0);
        let base = w.space.names.intern("InstantHitDamageBaseValue");
        let sys = system();
        let mut cx = EvalCx::new(&mut w);
        let added = sys
            .add_modifier_by_name(gun, m, base, false, &mut cx)
            .unwrap();
        assert_eq!(added, AddModifierOutcome::AppliedInPlace);
        assert_eq!(sys.attribute_value_by_name(gun, base, &mut cx), Ok(11.0));
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(10.0));
    }

    #[test]
    fn let_base_keeps_an_int_exact_and_recomputes_without_notifying() {
        let (mut w, c) = world();
        let gun = w.obj(c.weapon, "Gun");
        let clip = companions(&w, gun, "ClipSize");
        let rhs = Value::Int(16_777_217);
        let_base(&mut w.space, gun, clip, rhs, CombineForm::Vm5F).unwrap();
        assert_eq!(w.get(gun, "ClipSizeBaseValue"), Value::Int(16_777_217));
        // The recompute reads the base as a float: 2^24 + 1 rounds to 2^24 there, not in the base.
        assert_eq!(w.get(gun, "ClipSize"), Value::Int(16_777_216));
        // ClipSize carries the notify flag; the typed let does not notify.
        assert!(w.notified.is_empty());

        let damage = companions(&w, gun, "InstantHitDamage");
        let m = w.modifier(&c, PRE, 2.0);
        w.set(
            gun,
            "InstantHitDamageModifierStack",
            Value::array(vec![Value::Object(Some(m))]),
        );
        let_base(
            &mut w.space,
            gun,
            damage,
            Value::Float(5.0),
            CombineForm::WeaponGen,
        )
        .unwrap();
        assert_eq!(w.get(gun, "InstantHitDamageBaseValue"), Value::Float(5.0));
        assert_eq!(w.get(gun, "InstantHitDamage"), Value::Float(7.0));
    }

    #[test]
    fn let_base_refuses_another_kind_and_writes_nothing() {
        let (mut w, c) = world();
        let gun = w.obj(c.weapon, "Gun");
        w.base(gun, "ClipSize", Value::Int(12));
        let clip = companions(&w, gun, "ClipSize");
        for rhs in [Value::Float(3.0), Value::Byte(3), Value::Bool(true)] {
            let e = let_base(&mut w.space, gun, clip, rhs, CombineForm::Vm5F).unwrap_err();
            assert!(matches!(e, AttrError::BadData { .. }), "{e:?}");
        }
        assert_eq!(w.get(gun, "ClipSizeBaseValue"), Value::Int(12));
        assert_eq!(w.get(gun, "ClipSize"), Value::Int(12));
        // Companions whose value side is not an attribute value property.
        let spread = w.space.prop(c.weapon, "Spread").unwrap();
        let wrong = AttrCompanions {
            value: spread,
            ..clip
        };
        let e = let_base(&mut w.space, gun, wrong, Value::Int(1), CombineForm::Vm5F).unwrap_err();
        assert!(matches!(e, AttrError::BadData { .. }));
    }

    #[test]
    fn the_owner_notification_of_an_unknown_owner_is_not_read() {
        let (mut w, c) = world();
        let gun = w.obj(c.weapon, "Gun");
        let clip = w.space.prop(c.weapon, "ClipSize").unwrap();
        assert_eq!(
            owner_notification(&mut w.space, gun, clip),
            Ok(Notified::NotRead)
        );
        // A pool's other property and a skill's other property: known owners, no effect.
        let pool = w.obj(c.pool, "Pool");
        let max = w.space.prop(c.pool, "MaxValue").unwrap();
        assert_eq!(
            owner_notification(&mut w.space, pool, max),
            Ok(Notified::NoEffect)
        );
        // Exactly at the maximum: stays the maximum, unchanged.
        w.base(pool, "MaxValue", Value::Float(50.0));
        w.set(pool, "CurrentValue", Value::Float(50.0));
        let current = w.space.prop(c.pool, "CurrentValue").unwrap();
        let at_max = Notified::PoolCurrentValue {
            before: 50.0,
            after: 50.0,
        };
        assert_eq!(owner_notification(&mut w.space, pool, current), Ok(at_max));
    }
}
