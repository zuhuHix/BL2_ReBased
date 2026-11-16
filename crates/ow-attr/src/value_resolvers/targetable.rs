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
//! - `ParityMode::Off`: `NotYet { NotRead }`. Neither the class script (it has none) nor a record settles what the
//!   native answers, so no value is guessed.
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes (`NotPorted` in Reference, the C++ has no write;
//! `NotRead` in Off).

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
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

pub(crate) struct Targetable;

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
fn read(cx: &mut EvalCx<'_>, resolver: ObjRef) -> Result<f32, AttrError> {
    if mode(cx) != ParityMode::Reference {
        return Err(unread("get in Off mode"));
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
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver)
    }

    fn base_value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver)
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
