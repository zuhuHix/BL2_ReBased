//! `GearboxFramework.NounAttributeValueResolver`, the "AI parameter" resolver. The class declares only
//! `DefaultValue` (initialization data) and `ReplicationStrategy`. **Every rule here is UNVERIFIED in the running
//! game.**
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ reading (`src/progression.cpp` `nounValue`, [`reference_read`]): the
//!   `BaseValue` of the context's `AIClass.AttributeStartingValues` entry for this attribute, else `DefaultValue`,
//!   both evaluated with the context as the source; nothing is written. Get and get-base are the same read.
//! - `ParityMode::Off`, the native rule of NATIVE_NOUN_ATTRIBUTES.md, implemented in the clean room from that note
//!   alone (lane FIX-NOUN-OFF, decision 2026-11-16-43; docs/LANE_BRIEF.md, "Clean room (native code)"): the noun
//!   store on the context ("Off mode" below).
//!
//! **The attribute.** The C++ reading is handed the attribute it serves; [`ValueResolver`] is not. Every installed
//! Noun resolver (15) is a subobject of its one attribute definition and listed in that attribute's
//! `ValueResolverChain` (the packages), so the resolver's outer is taken as the attribute. The assumption is checked:
//! an outer that is not an attribute definition, or whose `ValueResolverChain` does not list this resolver, is
//! `NotYet { NotRead }`.
//!
//! **Stricter than the C++.** Where `nounValue` falls back or skips, this reading answers `BadData`: an `AIClass`
//! that is not an object reference (the C++ reads no starting values and falls back to `DefaultValue`), and a
//! starting-value entry with no `Attribute` field (the C++ skips that entry). Neither shape is in the installed data.
//!
//! **Writes** (add, remove, set-base) in Reference mode answer `NotYet { NotPorted }`: the C++ has no noun write.
//!
//! # Off mode (note sections 3 to 5)
//!
//! - **Key:** the name of the resolver's outer object (`NameId` equality, the number included). No outer: no store
//!   (reads 0.0, writes false). Unlike the Reference reading, the outer need not be an attribute.
//! - **Provider:** a context whose class declares `NounState` (checklist item 2; `ow-object` models no interfaces).
//!   Anything else: no store.
//! - **Strategy 0** (every installed mind and weapon noun): the entry is the first `NounState` element whose
//!   `NounName` is the key. A missing one is created by reads and writes alike: `DefaultValue` is evaluated on the
//!   context first (`d`), then an element (key, `Value` = `BaseValue` = `d`, empty stack, the strategy, slot -1)
//!   goes on the end of the array as it stands after that evaluation. Reads give the entry's `Value` / `BaseValue`;
//!   the chain's previous value is ignored. The writes are [`add`], [`remove`] and [`set_base`] on
//!   [`Storage::Noun`], with the recompute of section 5 and no change notification.
//! - **Strategy 1 or 2 on a provider** (`IsPhaselocked` on a pawn): `NotYet { NotPorted }`. The note's table needs
//!   to know whether the provider hands over a replicated array, which the note marks as not read (only expected),
//!   and a created entry's slot is followed by a call to the provider whose effect was not read. A strategy outside
//!   0..=2 is `NotYet { NotRead }` (the table has no row for it).
//! - **Mirror:** an entry that already holds a replicated slot (`IndexInReplicationArray` not -1) cannot be written,
//!   for the same reasons: `NotYet { NotPorted }`, before anything changes. Reading it is fine.
//! - **Net mode:** the attribute worlds are standalone (the slice is single player), so the client row of the table
//!   is not implemented (checklist item 6); every row ported here is the same in every net mode.
//! - **A null modifier** cannot reach the resolver: `AttributeSystem::add_modifier` and `remove_modifier` take an
//!   object reference, so the note's "a null modifier returns false" is the caller's to answer.

use ow_core::{NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::{LayoutError, PropKind};
use ow_object::value::Value;

use crate::api::{
    ProbeOp,
    AddModifierOutcome, AttrError, EvalCx, RemoveModifierOutcome, Storage, ValueResolver,
};
use crate::eval::{
    byte, class_path, elements, ensure_loaded, evaluate_value, field, name as name_of, number,
    object, property, required,
};
use crate::registry::AttributeSystem;
use crate::stack::{CombineForm, NumKind};
use crate::storage::stack_value;

/// Where the native rule is written down.
pub(crate) const NOTE: &str = "NATIVE_NOUN_ATTRIBUTES.md";
/// The C++ reading Reference mode follows.
const CPP: &str = "src/progression.cpp nounValue";
/// The property that makes a context a noun provider and holds its entries.
const NOUN_STATE: &str = "NounState";

pub(crate) struct Noun;

/// The world's parity mode ([`crate::eval::parity`]).
fn mode(cx: &mut EvalCx<'_>) -> ParityMode {
    crate::eval::parity(cx)
}

/// Reference mode's writes: the C++ reading has none.
fn reference_write() -> AttrError {
    NotYet::not_ported(
        "NounAttributeValueResolver add, remove or set-base in the reference reading: the C++ has no noun write",
        CPP,
    )
    .into()
}

fn bad(reason: String) -> AttrError {
    AttrError::BadData {
        reason: reason.into(),
    }
}

/// The attribute this resolver serves, for the C++ reading, which is handed it: the resolver's outer, which must be
/// an attribute definition whose `ValueResolverChain` lists this resolver (module documentation).
fn attribute(cx: &mut EvalCx<'_>, resolver: ObjRef) -> Result<ObjRef, AttrError> {
    ensure_loaded(cx, resolver)?;
    let owner = cx.world.parts().space.heap.get(resolver)?.outer;
    if let Some(a) = owner
        && let Some(chain) = property(cx, a, "ValueResolverChain")?
    {
        for entry in elements("ValueResolverChain", &chain)? {
            if object("ValueResolverChain entry", entry)? == Some(resolver) {
                return Ok(a);
            }
        }
    }
    let space = cx.world.parts().space;
    Err(NotYet::not_read(
        format!(
            "NounAttributeValueResolver (reference reading) whose outer {} is not an attribute definition listing \
             it in its ValueResolverChain: the resolver is not told which attribute it serves",
            owner.map_or_else(|| "None".to_string(), |o| class_path(space, o))
        ),
        CPP,
    )
    .into())
}

/// Reference mode, the C++ reading (module documentation). The store gives one object per path, so object identity
/// is the C++ path match. A context without an `AIClass` property, or with None there, reads `DefaultValue`, as the
/// C++ does for any resolved object.
fn reference_read(
    sys: &AttributeSystem,
    resolver: ObjRef,
    context: ObjRef,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let attr = attribute(cx, resolver)?;
    let class = match property(cx, context, "AIClass")? {
        Some(v) => object("AIClass", &v)?,
        None => None,
    };
    if let Some(class) = class
        && let Some(starting) = property(cx, class, "AttributeStartingValues")?
    {
        let chosen = {
            let space = cx.world.parts().space;
            let mut chosen = None;
            for entry in elements("AttributeStartingValues", &starting)? {
                if object("Attribute", field(space, entry, "Attribute")?)? == Some(attr) {
                    chosen = Some(field(space, entry, "BaseValue")?.clone());
                    break;
                }
            }
            chosen
        };
        if let Some(data) = chosen {
            return evaluate_value(sys, &data, Some(context), None, cx);
        }
    }
    let data = required(cx, resolver, "DefaultValue")?;
    evaluate_value(sys, &data, Some(context), None, cx)
}

// ---------------------------------------------------------------------------------------------------------------
// Off mode: the noun store (module documentation, "Off mode")
// ---------------------------------------------------------------------------------------------------------------

/// `NotYet { NotPorted }` for the parts of the note's table that depend on what the note marks as not read.
fn replication_not_read(what: String) -> AttrError {
    NotYet::not_ported(format!("NounAttributeValueResolver {what}"), NOTE).into()
}

/// The element struct of the provider property `state`, which must be an array of structs.
fn entry_struct(space: &ObjectSpace, state: PropId) -> Result<StructId, AttrError> {
    let decl = space
        .classes
        .decl(state)
        .ok_or(LayoutError::NotAProperty(state))?;
    match &decl.kind {
        PropKind::Array(inner) => match inner.as_ref() {
            PropKind::Struct(s) => Ok(*s),
            other => Err(bad(format!(
                "{NOUN_STATE} is an array of {}, not of NounAttributeState",
                other.kind_name()
            ))),
        },
        other => Err(bad(format!(
            "{NOUN_STATE} is a {}, not an array",
            other.kind_name()
        ))),
    }
}

/// The provider's `NounState` elements.
fn entries(space: &ObjectSpace, owner: ObjRef, state: PropId) -> Result<Vec<Value>, AttrError> {
    let v = space.read(owner, PropAddr::scalar(state))?;
    Ok(elements(NOUN_STATE, v)?.to_vec())
}

/// Element `index` of `items`.
fn element(items: &[Value], index: usize) -> Result<&Value, AttrError> {
    items
        .get(index)
        .ok_or_else(|| bad(format!("{NOUN_STATE} has no element {index}")))
}

/// Sets field `name` of the struct value `v`.
fn set_field(space: &ObjectSpace, v: &mut Value, name: &str, x: Value) -> Result<(), AttrError> {
    let s = v
        .struct_mut()
        .ok_or_else(|| bad(format!("a {NOUN_STATE} element is not a struct")))?;
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
    let slot = usize::try_from(entry)
        .ok()
        .and_then(|i| s.fields.get_mut(i))
        .ok_or_else(|| bad(format!("a {NOUN_STATE} element lacks field {name}")))?;
    *slot = x;
    Ok(())
}

/// The index of the first element whose `NounName` is `key` (note section 4, "entry").
fn find(space: &ObjectSpace, items: &[Value], key: NameId) -> Result<Option<usize>, AttrError> {
    for (i, e) in items.iter().enumerate() {
        if name_of("NounName", field(space, e, "NounName")?)? == key {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

/// The entry of `resolver` on `context` (note sections 3 and 4), created when missing; `Ok(None)` where the table
/// gives no store (no outer, not a provider). Reads and writes both come here.
fn entry(
    sys: &AttributeSystem,
    resolver: ObjRef,
    context: ObjRef,
    cx: &mut EvalCx<'_>,
) -> Result<Option<Storage>, AttrError> {
    entry_with(sys, resolver, context, cx, None)
}

/// [`entry`], or the read-only probe when `probe` is given (lane IMPL-APPLY-SLOT-EFFECT-MODIFIERS, review r1 of #483;
/// lane FIX-483-FOLLOWUP): the same strategy checks, then
/// - an existing entry with a replicated slot is refused as the operation's write to it would be (the refusal
///   `rewrite` gives, decided here so a preflight sees it): an add always rewrites the entry, a removal only when the
///   entry's stack holds the modifier (F6; a removal of an absent modifier changes nothing, so it passes);
/// - a missing entry has its `DefaultValue` validated without being evaluated ([`crate::eval::preflight_init`], F1):
///   evaluating could create another noun's entry or draw a random variance, which a preflight must not do. Nothing
///   is appended, and the entry would be created with no replicated slot, so it passes. Execution evaluates the
///   default once, when its [`entry`] creates the entry.
fn entry_with(
    sys: &AttributeSystem,
    resolver: ObjRef,
    context: ObjRef,
    cx: &mut EvalCx<'_>,
    probe: Option<ProbeOp>,
) -> Result<Option<Storage>, AttrError> {
    ensure_loaded(cx, resolver)?;
    ensure_loaded(cx, context)?;
    let (key, state) = {
        let space = cx.world.parts().space;
        let Some(outer) = space.heap.get(resolver)?.outer else {
            return Ok(None);
        };
        let key = space.heap.get(outer)?.name;
        let class = space.heap.get(context)?.class;
        let Some(state) = space.prop(class, NOUN_STATE) else {
            return Ok(None);
        };
        (key, state)
    };
    let strategy = byte(
        "ReplicationStrategy",
        &required(cx, resolver, "ReplicationStrategy")?,
    )?;
    match strategy {
        0 => {}
        1 | 2 => {
            return Err(replication_not_read(format!(
                "with ReplicationStrategy {strategy} on a provider: whether the provider hands over a replicated \
                 array, and what the replicated-copy-changed call does, are not read"
            )));
        }
        other => {
            return Err(NotYet::not_read(
                format!(
                    "NounAttributeValueResolver with ReplicationStrategy {other}, outside the note's table"
                ),
                NOTE,
            )
            .into());
        }
    }
    {
        let space = cx.world.parts().space;
        let items = entries(space, context, state)?;
        if let Some(index) = find(space, &items, key)? {
            let rewrites = match probe {
                None => false,
                Some(ProbeOp::Add) => true,
                Some(ProbeOp::Remove(m)) => stack_of(space, context, state, index)?.contains(&Value::Object(Some(m))),
            };
            if rewrites {
                replicated_slot_refused(space, element(&items, index)?)?;
            }
            return Ok(Some(Storage::Noun {
                owner: context,
                state,
                index,
            }));
        }
    }
    // Missing: `d` first, then the element goes on the end of the array as it stands after that evaluation.
    let default = required(cx, resolver, "DefaultValue")?;
    if probe.is_some() {
        crate::eval::preflight_init(sys, &default, cx)?;
        return Ok(None);
    }
    let d = evaluate_value(sys, &default, Some(context), None, cx)?;
    let space = cx.world.parts().space;
    let mut items = entries(space, context, state)?;
    let mut e = space.classes.struct_defaults(entry_struct(space, state)?)?;
    set_field(space, &mut e, "NounName", Value::Name(key))?;
    set_field(space, &mut e, "Value", Value::Float(d))?;
    set_field(space, &mut e, "BaseValue", Value::Float(d))?;
    set_field(space, &mut e, "ModifierStack", Value::array(Vec::new()))?;
    set_field(space, &mut e, "ReplicationStrategy", Value::Byte(strategy))?;
    set_field(space, &mut e, "IndexInReplicationArray", Value::Int(-1))?;
    let index = items.len();
    items.push(e);
    space.write(context, PropAddr::scalar(state), Value::array(items))?;
    Ok(Some(Storage::Noun {
        owner: context,
        state,
        index,
    }))
}

/// Field `name` (`Value` or `BaseValue`) of the entry `storage` names, or 0.0 where there is no store.
fn read_entry(cx: &mut EvalCx<'_>, storage: Option<Storage>, name: &str) -> Result<f32, AttrError> {
    match storage {
        Some(Storage::Noun {
            owner,
            state,
            index,
        }) => entry_number(cx.world.parts().space, owner, state, index, name),
        _ => Ok(0.0),
    }
}

/// Field `name` of element `index` of `owner`'s noun store, as a float.
pub(crate) fn entry_number(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    name: &str,
) -> Result<f32, AttrError> {
    let items = entries(space, owner, state)?;
    number(name, field(space, element(&items, index)?, name)?)
}

/// The `ModifierStack` of element `index`.
fn stack_of(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
) -> Result<Vec<Value>, AttrError> {
    let items = entries(space, owner, state)?;
    let e = element(&items, index)?;
    Ok(elements("ModifierStack", field(space, e, "ModifierStack")?)?.to_vec())
}

/// The refusal of a write to an entry with a replicated slot (module documentation, "Mirror"), as `rewrite` gives it.
fn replicated_slot_refused(space: &ObjectSpace, e: &Value) -> Result<(), AttrError> {
    let slot = match field(space, e, "IndexInReplicationArray")? {
        Value::Int(i) => *i,
        other => {
            return Err(bad(format!(
                "IndexInReplicationArray holds a {}, not an int",
                other.kind_name()
            )));
        }
    };
    if slot != -1 {
        return Err(replication_not_read(format!(
            "write to an entry with replicated slot {slot}: the mirror needs the provider's replicated array and \
             its change call, which are not read"
        )));
    }
    Ok(())
}

/// Rewrites element `index` with a new stack or base and the recomputed value (note section 5). Everything is
/// computed first and written after, so an error leaves the entry as it was. An entry with a replicated slot is
/// refused before anything changes (module documentation, "Mirror").
fn rewrite(
    space: &mut ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    stack: Option<Vec<Value>>,
    base: Option<f32>,
) -> Result<(), AttrError> {
    let mut items = entries(space, owner, state)?;
    let mut e = element(&items, index)?.clone();
    let slot = match field(space, &e, "IndexInReplicationArray")? {
        Value::Int(i) => *i,
        other => {
            return Err(bad(format!(
                "IndexInReplicationArray holds a {}, not an int",
                other.kind_name()
            )));
        }
    };
    if slot != -1 {
        return Err(replication_not_read(format!(
            "write to an entry with replicated slot {slot}: the mirror needs the provider's replicated array and \
             its change call, which are not read"
        )));
    }
    let stack = match stack {
        Some(s) => s,
        None => elements("ModifierStack", field(space, &e, "ModifierStack")?)?.to_vec(),
    };
    let base = match base {
        Some(b) => b,
        None => number("BaseValue", field(space, &e, "BaseValue")?)?,
    };
    // Section 5: single precision, sums in stack order, other types skipped, no truncation, no clamp. That is the
    // VM's opcode 5F form on a float, whatever combine form the call carries.
    let value = stack_value(space, base, &stack, CombineForm::Vm5F, NumKind::Float)?;
    set_field(space, &mut e, "ModifierStack", Value::array(stack))?;
    set_field(space, &mut e, "BaseValue", Value::Float(base))?;
    set_field(space, &mut e, "Value", value)?;
    items[index] = e;
    space.write(owner, PropAddr::scalar(state), Value::array(items))?;
    Ok(())
}

/// Add (note section 5): the same modifier object already on the entry's stack is refused; otherwise it is
/// appended and the value recomputed.
pub(crate) fn add(
    space: &mut ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    modifier: ObjRef,
) -> Result<AddModifierOutcome, AttrError> {
    let mut stack = stack_of(space, owner, state, index)?;
    let held = Value::Object(Some(modifier));
    if stack.contains(&held) {
        return Ok(AddModifierOutcome::Duplicate);
    }
    stack.push(held);
    rewrite(space, owner, state, index, Some(stack), None)?;
    Ok(AddModifierOutcome::Added)
}

/// Remove (note section 5): every copy of the modifier object; none there is refused and changes nothing.
pub(crate) fn remove(
    space: &mut ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    modifier: ObjRef,
) -> Result<RemoveModifierOutcome, AttrError> {
    let stack = stack_of(space, owner, state, index)?;
    let held = Value::Object(Some(modifier));
    let kept: Vec<Value> = stack.iter().filter(|v| **v != held).cloned().collect();
    let count = stack.len().saturating_sub(kept.len());
    if count == 0 {
        return Ok(RemoveModifierOutcome::NotPresent);
    }
    rewrite(space, owner, state, index, Some(kept), None)?;
    Ok(RemoveModifierOutcome::Removed { count })
}

/// Set-base (note section 5): the base, then the recompute.
pub(crate) fn set_base(
    space: &mut ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    v: f32,
) -> Result<(), AttrError> {
    rewrite(space, owner, state, index, None, Some(v))
}

impl ValueResolver for Noun {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        match mode(cx) {
            ParityMode::Off => {
                let s = entry(sys, resolver, context, cx)?;
                read_entry(cx, s, "Value")
            }
            ParityMode::Reference => reference_read(sys, resolver, context, cx),
        }
    }

    fn base_value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        match mode(cx) {
            ParityMode::Off => {
                let s = entry(sys, resolver, context, cx)?;
                read_entry(cx, s, "BaseValue")
            }
            ParityMode::Reference => reference_read(sys, resolver, context, cx),
        }
    }

    /// Off: the entry (created when missing), or `None` where there is no store (every write then answers false).
    /// Reference: `NotYet { NotPorted }` (module documentation), never a quiet `None`.
    fn storage(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        match mode(cx) {
            ParityMode::Off => entry(sys, resolver, context, cx),
            ParityMode::Reference => Err(reference_write()),
        }
    }

    /// The read-only probe (review r1 of #483, lane FIX-483-FOLLOWUP): [`entry_with`] without creating or evaluating;
    /// a replicated slot the operation would rewrite is refused here.
    fn probe_storage(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        op: ProbeOp,
        cx: &mut EvalCx<'_>,
    ) -> Result<(), AttrError> {
        match mode(cx) {
            ParityMode::Off => entry_with(sys, resolver, context, cx, Some(op)).map(|_| ()),
            ParityMode::Reference => Err(reference_write()),
        }
    }
}
