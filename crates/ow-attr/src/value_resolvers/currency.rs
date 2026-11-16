//! `WillowGame.CurrencyAttributeValueResolver`: an amount of currency on hand as an attribute value (lane
//! FIX-ATTR-RESOLVERS, decision 2026-11-16-73). The class declares `FormOfCurrency` (an `ECurrencyType` byte) and no
//! script function: its get is a native read in NATIVE_ATTR_RESOLVERS.md; the Off implementation below follows
//! that note. The respawn fee (`GlobalsDefinition.TeleportCost`, 0.07 of the cash) reads it through its context
//! resolver chain (decision 2026-11-16-80). **The native reading remains UNVERIFIED in the running game**.
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ stand-in (`src/progression.cpp`, `CurrencyAttributeValueResolver`): the resolved
//!   context is the replication info, or, when it declares no `Currency` (a controller), its `PlayerReplicationInfo`;
//!   the value is `Currency[FormOfCurrency].CurrentAmount` of that info. A context with neither, a missing slot or a
//!   slot without `CurrentAmount` is what the C++ refuses ("unsupported currency attribute"), here
//!   `NotYet { NotRead }` naming it. The live fee rows match this arithmetic (COMBAT_DOWN_RESPAWN.md), which says
//!   nothing about the native.
//! - `ParityMode::Off`, the native as NATIVE_ATTR_RESOLVERS.md section 2 reads it (lane FIX-ATTR-RESOLVERS-OFF, the
//!   fresh implementer, decision 2026-11-16-80): a context that is not a `WillowGame.WillowPlayerReplicationInfo` (or
//!   a subclass) answers 0.0, **a controller included**: the native does not move to the controller's replication
//!   info (whether an attribute reaches the info is its context chain's business). On a replication info, its
//!   `GetCurrencyOnHand(FormOfCurrency)` as the script call would answer it ([`AttrWorld::currency_on_hand`]: the
//!   VM's binding of that native, `ow_combat::down`), the int converted to a float. The cast's class is the note's
//!   medium-confidence reading. UNVERIFIED in the game.
//!
//! [`AttrWorld::currency_on_hand`]: crate::api::AttrWorld::currency_on_hand
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes (`NotPorted` in Reference: the C++ has no write).

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::eval::{byte, ensure_loaded, field, number, object, parity, property, required};
use crate::registry::AttributeSystem;

/// The analyst note read by a fresh implementer of the Off rule.
pub(crate) const RECORD: &str = "NATIVE_ATTR_RESOLVERS.md (CurrencyAttributeValueResolver)";
/// The C++ stand-in Reference mode follows.
const CPP: &str = "src/progression.cpp (CurrencyAttributeValueResolver)";

pub(crate) struct Currency;

fn refused(what: impl Into<String>) -> AttrError {
    NotYet::not_read(
        format!(
            "CurrencyAttributeValueResolver (the C++ stand-in): {}",
            what.into()
        ),
        CPP,
    )
    .into()
}

/// Element `index` of `obj`'s property `name` (a static array, or a scalar at 0); `None` when the class does not
/// declare it or the index is past its size.
fn element(
    cx: &mut EvalCx<'_>,
    obj: ObjRef,
    name: &str,
    index: u32,
) -> Result<Option<Value>, AttrError> {
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
        return Err(refused(
            "no replication info or no Currency table in the context",
        ));
    }
    let entry = element(cx, info, "Currency", u32::from(at))?
        .ok_or_else(|| refused(format!("no currency slot {at}")))?;
    let space = cx.world.parts().space;
    let amount = field(space, &entry, "CurrentAmount")
        .map_err(|_| refused(format!("slot {at} has no CurrentAmount")))?;
    // The C++ `float(amount->number())`: an int amount to single precision.
    number("CurrentAmount", amount)
}

/// The replication-info class the native's cast names (NATIVE_ATTR_RESOLVERS.md section 2, medium confidence).
pub(crate) const REPLICATION_INFO: &str = "WillowGame.WillowPlayerReplicationInfo";

/// The native's value (module documentation).
fn native(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    if !crate::context_resolvers::is_a(cx.world.parts().space, context, REPLICATION_INFO)? {
        return Ok(0.0);
    }
    let form = byte("FormOfCurrency", &required(cx, resolver, "FormOfCurrency")?)?;
    let amount = cx.world.currency_on_hand(context, form)?;
    // The int to single precision (the note: "the int converted to a float"); an amount past 2^24 rounds, as a
    // float conversion does.
    Ok(amount as f32)
}

fn read(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    match parity(cx) {
        ParityMode::Reference => reference(cx, resolver, context),
        _ => native(cx, resolver, context),
    }
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
