//! `WillowGame.TargetableAttributeValueResolver`, a target's own property as an attribute value (lane
//! W6-COMBAT-RUN2, decision 2026-11-16-54). The class declares one field, `ValueToResolve` (a byte of its enum
//! `ETargetableValue`), and no script functions: its get is the inherited native, which no record has read
//! (NATIVE_ATTRIBUTES.md, "Not read yet"). The installed data has one such resolver, the only value resolver of
//! `D_Attributes.Targetable.Targetable_Is_Champion` and a subobject of it (counted locally in the export table of
//! `Startup.upk`). **Nothing here is a rule of the game**, and nothing is verified in the running game.
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ stand-in: `src/progression.cpp` `operandValue` answers an expression operand
//!   whose attribute path ends in `Targetable_Is_Champion` with `AttributeContext::champion`, which is false
//!   ("not a champion", `src/progression.hpp`) and which nothing sets. So **0.0**, whatever the context and the field,
//!   for a resolver that serves an attribute named that way. The C++ matches the attribute by path and never runs its
//!   chain; the port runs the chain and answers here. Every other attribute this class may serve is refused by the C++
//!   ("unsupported condition attribute"), so it is `NotYet { NotRead }` here too. The attribute is the resolver's
//!   outer, checked as the Noun resolver checks it: an outer that is not an attribute definition listing this
//!   resolver in its `ValueResolverChain` is `NotYet { NotRead }`.
//! - `ParityMode::Off` (lane IMPL-TARGETABLE-RESOLVER, decision 2026-11-26-NNN; clean room: implemented from
//!   docs/verification/NATIVE_TARGETABLE_RESOLVER.md section 5 alone, the executable not read; **UNVERIFIED in the
//!   running game**). Get and get-base answer the same, in order, the first answer met:
//!   1. `ValueToResolve` declared as a byte, else `BadData`; not 0 (`TARGETABLE_IsChampion`): **0.0**;
//!   2. no context: **0.0**; a context the store has not loaded (a handle, or a stand-in without its slots):
//!      [`CONTEXT_NOT_LOADED`];
//!   3. the `ITargetable` cast by the context's class chain (the class table only): none of [`TARGETABLE_CLASSES`] on
//!      it: **0.0**; an ancestry that ends, short of `Core.Object`, at a class with no super class cannot answer:
//!      `NotRead`;
//!   4. `IsChampion` by the context's first native class: `Engine.Pawn`, `WillowGame.WillowPawn` and
//!      `WillowGame.WillowPlayerPawn` **0.0**; `WillowGame.WillowAIPawn` its `BalanceDefinitionState.BalanceDefinition`
//!      (None **0.0**; not loaded [`BALANCE_NOT_LOADED`]; first native class `WillowGame.AIPawnBalanceDefinition`: its
//!      `Champion`, **1.0 or 0.0**; another class: [`DOES_SPAWN_CHAMPION`] naming it); any other class:
//!      [`IS_CHAMPION`] naming it.
//!
//!   It writes, loads and interns nothing. The first native class: a class with code in the store by its decoded
//!   body's native bit (a body that did not decode counts as native, as `ow-world`'s `first_native` does), a class
//!   without code by the class table's native mark.
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes (`NotPorted` in Reference, the C++ has no write;
//! `NotRead` in Off).

use ow_core::{NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver, WorldParts};
use crate::eval::{class_path, elements, ensure_loaded, object, property};
use crate::registry::AttributeSystem;

/// The record that lists the native as not read.
pub(crate) const RECORD: &str =
    "NATIVE_ATTRIBUTES.md (Not read yet: TargetableAttributeValueResolver)";
/// The C++ stand-in Reference mode follows.
const CPP: &str = "src/progression.cpp operandValue (AttributeContext::champion, false)";
/// The attribute name the C++ stand-in answers for (a path suffix there).
const CHAMPION: &str = "Targetable_Is_Champion";
/// The C++ stand-in's value: not a champion.
const NOT_A_CHAMPION: f32 = 0.0;

/// The Off rule's record.
pub(crate) const RECORD_OFF: &str = "docs/verification/NATIVE_TARGETABLE_RESOLVER.md (section 5)";
/// The Off refusals (module documentation).
pub const CONTEXT_NOT_LOADED: &str =
    "TargetableAttributeValueResolver: a context the store has not loaded (a load is a write)";
pub const BALANCE_NOT_LOADED: &str = "TargetableAttributeValueResolver: a balance definition the store has not loaded";
pub const IS_CHAMPION: &str = "TargetableAttributeValueResolver: IsChampion of";
pub const DOES_SPAWN_CHAMPION: &str = "TargetableAttributeValueResolver: DoesSpawnChampion of";
/// The classes with a native `ITargetable` table (the note's section 1).
pub const TARGETABLE_CLASSES: [&str; 3] =
    ["Engine.Pawn", "WillowGame.WillowInteractiveObject", "WillowGame.WillowProjectile"];
/// The first native classes whose `IsChampion` is false.
const NEVER_CHAMPION: [&str; 3] = ["Engine.Pawn", "WillowGame.WillowPawn", "WillowGame.WillowPlayerPawn"];

pub(crate) struct Targetable;

fn bad(reason: String) -> AttrError {
    AttrError::BadData { reason: reason.into() }
}

/// Whether `o` is not resident: a store handle not loaded (or failed), or a stand-in without its slots.
fn not_resident(space: &ObjectSpace, defs: Option<&ow_defs::DefStore>, o: ObjRef) -> Result<bool, AttrError> {
    let x = space.heap.get(o)?;
    let stand_in = x.slots.is_empty() && space.classes.get_layout(x.class).is_some_and(|l| l.slot_count() > 0);
    Ok(stand_in || defs.is_some_and(|d| d.is_unloaded(space, o) || d.is_failed(space, o)))
}

/// The path of the first native class on `class`'s chain, itself included (module documentation).
fn first_native(space: &ObjectSpace, defs: Option<&ow_defs::DefStore>, class: ClassId) -> Option<String> {
    let code = defs.map(|d| d.code());
    let mut cursor = Some(class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let layout = space.classes.get_layout(c)?;
        let native = match code.and_then(|code| code.key_of(c).map(|k| (code, k))) {
            Some((code, k)) => code.class(k).body.as_ref().map_or(true, |b| b.is_native()),
            None => space.classes.is_native(c),
        };
        if native {
            return Some(layout.path().as_str().to_string());
        }
        cursor = layout.super_class();
    }
    None
}

/// The `ITargetable` cast by the class chain (module documentation, step 3).
fn targetable(space: &ObjectSpace, class: ClassId) -> Result<bool, AttrError> {
    let mut cursor = Some(class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(layout) = space.classes.get_layout(c) else { break };
        let path = layout.path().as_str();
        if TARGETABLE_CLASSES.iter().any(|t| t.eq_ignore_ascii_case(path)) {
            return Ok(true);
        }
        if layout.super_class().is_none() && !path.eq_ignore_ascii_case("Core.Object") {
            return Err(NotYet::not_read(
                format!(
                    "TargetableAttributeValueResolver: the ITargetable cast over the class {path}, whose super class \
                     is not known"
                ),
                RECORD_OFF,
            )
            .into());
        }
        cursor = layout.super_class();
    }
    Ok(false)
}

/// `name` of `o` with its declared kind (`BadData` when undeclared).
fn field(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<(Value, PropKind), AttrError> {
    let class = space.heap.get(o)?.class;
    let p = space.prop(class, name).ok_or_else(|| bad(format!("{} declares no {name}", class_path(space, o))))?;
    let kind =
        space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(format!("{name} has no declaration")))?;
    Ok((space.read(o, PropAddr::scalar(p))?.clone(), kind))
}

/// Field `name` of the struct value `v`, by its layout.
fn struct_field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let id = space.names.find(name)?;
    let (i, _) = space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(usize::try_from(i).ok()?)
}

/// The Off get (module documentation). Reads only.
pub(super) fn read_off(cx: &mut EvalCx<'_>, resolver: ObjRef, context: Option<ObjRef>) -> Result<f32, AttrError> {
    let WorldParts { space, defs, .. } = cx.world.parts();
    let (space, defs): (&ObjectSpace, Option<&ow_defs::DefStore>) = (space, defs.as_deref());
    if not_resident(space, defs, resolver)? {
        return Err(NotYet::not_read(
            "TargetableAttributeValueResolver: the resolver is not loaded (a load is a write)",
            RECORD_OFF,
        )
        .into());
    }
    match field(space, resolver, "ValueToResolve")? {
        (Value::Byte(0), PropKind::Byte { .. }) => {}
        (Value::Byte(_), PropKind::Byte { .. }) => return Ok(0.0),
        _ => return Err(bad("ValueToResolve is not declared as a byte".into())),
    }
    let Some(context) = context else { return Ok(0.0) };
    if not_resident(space, defs, context)? {
        return Err(NotYet::not_read(CONTEXT_NOT_LOADED, RECORD_OFF).into());
    }
    let class = space.heap.get(context)?.class;
    if !targetable(space, class)? {
        return Ok(0.0);
    }
    let native = first_native(space, defs, class).unwrap_or_else(|| class_path(space, context));
    if NEVER_CHAMPION.iter().any(|n| n.eq_ignore_ascii_case(&native)) {
        return Ok(0.0);
    }
    if !native.eq_ignore_ascii_case("WillowGame.WillowAIPawn") {
        return Err(NotYet::not_read(format!("{IS_CHAMPION} {native}"), RECORD_OFF).into());
    }
    let state = match field(space, context, "BalanceDefinitionState")? {
        (v, PropKind::Struct(_)) => v,
        _ => return Err(bad("BalanceDefinitionState is not declared as a struct".into())),
    };
    let balance = match struct_field(space, &state, "BalanceDefinition") {
        Some(Value::Object(b)) => *b,
        _ => return Err(bad("BalanceDefinitionState declares no object BalanceDefinition".into())),
    };
    let Some(balance) = balance else { return Ok(0.0) };
    if not_resident(space, defs, balance)? {
        return Err(NotYet::not_read(BALANCE_NOT_LOADED, RECORD_OFF).into());
    }
    let balance_class = space.heap.get(balance)?.class;
    let balance_native = first_native(space, defs, balance_class).unwrap_or_else(|| class_path(space, balance));
    if !balance_native.eq_ignore_ascii_case("WillowGame.AIPawnBalanceDefinition") {
        return Err(NotYet::not_read(format!("{DOES_SPAWN_CHAMPION} {balance_native}"), RECORD_OFF).into());
    }
    match field(space, balance, "Champion")? {
        (Value::Bool(b), PropKind::Bool) => Ok(if b { 1.0 } else { 0.0 }),
        _ => Err(bad("Champion is not declared as a bool".into())),
    }
}

/// The world's parity mode: its store's. A world without a store runs the Rust rules (`Off`).
fn mode(cx: &mut EvalCx<'_>) -> ParityMode {
    cx.world.parts().defs.map_or(ParityMode::Off, |d| d.mode())
}

fn unread(what: &str) -> AttrError {
    NotYet::not_read(
        format!("TargetableAttributeValueResolver {what}: its native is not read"),
        RECORD,
    )
    .into()
}

/// The attribute this resolver serves: its outer, which must be an attribute definition whose `ValueResolverChain`
/// lists it (the installed shape). `None` otherwise.
fn attribute(cx: &mut EvalCx<'_>, resolver: ObjRef) -> Result<Option<ObjRef>, AttrError> {
    ensure_loaded(cx, resolver)?;
    let owner = cx.world.parts().space.heap.get(resolver)?.outer;
    if let Some(a) = owner
        && let Some(chain) = property(cx, a, "ValueResolverChain")?
    {
        for entry in elements("ValueResolverChain", &chain)? {
            if object("ValueResolverChain entry", entry)? == Some(resolver) {
                return Ok(Some(a));
            }
        }
    }
    Ok(None)
}

/// Get and get-base (module documentation).
fn read(cx: &mut EvalCx<'_>, resolver: ObjRef, context: Option<ObjRef>) -> Result<f32, AttrError> {
    if mode(cx) != ParityMode::Reference {
        return read_off(cx, resolver, context);
    }
    let Some(attr) = attribute(cx, resolver)? else {
        let space = cx.world.parts().space;
        let owner = space.heap.get(resolver)?.outer;
        return Err(NotYet::not_read(
            format!(
                "TargetableAttributeValueResolver (the C++ stand-in) whose outer {} is not an attribute definition \
                 listing it in its ValueResolverChain: the stand-in is keyed by the attribute",
                owner.map_or_else(|| "None".to_string(), |o| class_path(space, o))
            ),
            CPP,
        )
        .into());
    };
    let space = cx.world.parts().space;
    let name = space.heap.get(attr)?.name;
    if space.names.text(name).as_ref() == CHAMPION {
        Ok(NOT_A_CHAMPION)
    } else {
        Err(NotYet::not_read(
            format!(
                "TargetableAttributeValueResolver for the attribute {}: the C++ stands in only {CHAMPION}",
                space.names.text(name)
            ),
            CPP,
        )
        .into())
    }
}

impl ValueResolver for Targetable {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver, Some(context))
    }

    fn base_value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver, Some(context))
    }

    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        match mode(cx) {
            ParityMode::Reference => Err(NotYet::not_ported(
                "TargetableAttributeValueResolver add, remove or set-base: the C++ has no write for this class",
                CPP,
            )
            .into()),
            _ => Err(unread("add, remove or set-base")),
        }
    }
}
