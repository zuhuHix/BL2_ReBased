//! `WillowGame.CurrencyAttributeValueResolver`: an amount of currency on hand as an attribute value (lane
//! FIX-ATTR-RESOLVERS, decision 2026-11-16-73). The class declares `FormOfCurrency` (an `ECurrencyType` byte) and no
//! script function: its get is a native no record has read (NATIVE_ATTRIBUTES.md, "Not read yet"). The respawn fee
//! (`GlobalsDefinition.TeleportCost`, 0.07 of the cash) reads it through a controller context. **Nothing here is a
//! rule of the game**, and nothing is verified in the running game.
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ stand-in (`src/progression.cpp`, `CurrencyAttributeValueResolver`): the resolved
//!   context is the replication info, or, when it declares no `Currency` (a controller), its `PlayerReplicationInfo`;
//!   the value is `Currency[FormOfCurrency].CurrentAmount` of that info. A context with neither, a missing slot or a
//!   slot without `CurrentAmount` is what the C++ refuses ("unsupported currency attribute"), here
//!   `NotYet { NotRead }` naming it. The live fee rows match this arithmetic (COMBAT_DOWN_RESPAWN.md), which says
//!   nothing about the native.
//! - `ParityMode::Off`: `NotYet { NotRead }` until the native is read (the analyst note
//!   NATIVE_ATTR_RESOLVERS.md says what is open).
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes (`NotPorted` in Reference: the C++ has no write).

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::eval::{byte, ensure_loaded, field, number, object, parity, property};
use crate::registry::AttributeSystem;

/// The record that lists the native as not read, and the analyst note.
pub(crate) const RECORD: &str =
    "NATIVE_ATTR_RESOLVERS.md (CurrencyAttributeValueResolver; NATIVE_ATTRIBUTES.md Not read yet)";
/// The C++ stand-in Reference mode follows.
const CPP: &str = "src/progression.cpp (CurrencyAttributeValueResolver)";

pub(crate) struct Currency;

fn refused(what: impl Into<String>) -> AttrError {
    NotYet::not_read(
        format!("CurrencyAttributeValueResolver (the C++ stand-in): {}", what.into()),
        CPP,
    )
    .into()
}

/// Element `index` of `obj`'s property `name` (a static array, or a scalar at 0); `None` when the class does not
/// declare it or the index is past its size.
fn element(cx: &mut EvalCx<'_>, obj: ObjRef, name: &str, index: u32) -> Result<Option<Value>, AttrError> {
    ensure_loaded(cx, obj)?;
    let space = cx.world.parts().space;
    let class = space.heap.get(obj)?.class;
    let Some(p) = space.prop(class, name) else {
        return Ok(None);
    };
    let dim = space.classes.decl(p).map_or(1, |d| d.array_dim);
    if index >= u32::try_from(dim).unwrap_or(1) {
        return Ok(None);
    }
    Ok(Some(space.read(obj, PropAddr { prop: p, index })?.clone()))
}

/// The C++ stand-in's value (module documentation).
fn reference(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    let mut info = context;
    if property(cx, info, "Currency")?.is_none()
        && let Some(held) = property(cx, info, "PlayerReplicationInfo")?
        && let Some(held) = object("PlayerReplicationInfo", &held)?
    {
        info = held;
    }
    let form = property(cx, resolver, "FormOfCurrency")?
        .ok_or_else(|| refused("the resolver declares no FormOfCurrency"))?;
    let at = byte("FormOfCurrency", &form)?;
    if property(cx, info, "Currency")?.is_none() {
        return Err(refused("no replication info or no Currency table in the context"));
    }
    let entry = element(cx, info, "Currency", u32::from(at))?
        .ok_or_else(|| refused(format!("no currency slot {at}")))?;
    let space = cx.world.parts().space;
    let amount = field(space, &entry, "CurrentAmount").map_err(|_| refused(format!("slot {at} has no CurrentAmount")))?;
    // The C++ `float(amount->number())`: an int amount to single precision.
    number("CurrentAmount", amount)
}

fn read(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    if parity(cx) != ParityMode::Reference {
        return Err(NotYet::not_read(
            "CurrencyAttributeValueResolver in Off mode: its native is not read",
            RECORD,
        )
        .into());
    }
    reference(cx, resolver, context)
}

impl ValueResolver for Currency {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver, context)
    }

    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        Err(match parity(cx) {
            ParityMode::Reference => NotYet::not_ported(
                "CurrencyAttributeValueResolver add, remove or set-base: the C++ has no write for this class",
                CPP,
            ),
            _ => NotYet::not_read("CurrencyAttributeValueResolver add, remove or set-base", RECORD),
        }
        .into())
    }
}
