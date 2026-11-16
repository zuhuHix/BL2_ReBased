//! `GearboxFramework.RandomAttributeValueResolver`: a random number between two bounds as an attribute value (lane
//! FIX-ATTR-RESOLVERS, decision 2026-11-16-73). The class declares `LowerBound` and `UpperBound` (initialization
//! data), `bUseIntegerRandomization` and a value lifetime with saved values, and no script function: its get is a
//! native no record has read (NATIVE_ATTRIBUTES.md, "Not read yet"). The ammo pools' quantities and the adult
//! bullymong's item stage offset read it. **Nothing here is a rule of the game**, and nothing is verified.
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ stand-in (`src/progression.cpp`, `RandomAttributeValueResolver` and
//!   `randomBetween`): the two bounds evaluated on the context, lower first, then one draw from the evaluator's
//!   generator (`CrtStream::AttributeVariance`) mapped as the definition's random variance maps it
//!   (`crate::eval::random_between`: real `low + d * (high - low)`, integer `ceil(low) + floor(d * span)` inclusive);
//!   the draw **replaces** the running value. `ValueLifetime` and the saved values are not modelled: a fresh draw on
//!   every read, as the C++.
//! - `ParityMode::Off`: `NotYet { NotRead }` until the native is read (NATIVE_ATTR_RESOLVERS.md).
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::eval::{evaluate_init, flag, init_data, parity, random_between, required};
use crate::registry::AttributeSystem;

pub(crate) const RECORD: &str =
    "NATIVE_ATTR_RESOLVERS.md (RandomAttributeValueResolver; NATIVE_ATTRIBUTES.md Not read yet)";
const CPP: &str = "src/progression.cpp (RandomAttributeValueResolver, randomBetween)";

pub(crate) struct Random;

/// The bound `name` of the resolver, evaluated on the context.
fn bound(sys: &AttributeSystem, cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef, name: &str) -> Result<f32, AttrError> {
    let v = required(cx, resolver, name)?;
    let data = init_data(cx.world.parts().space, &v)?;
    evaluate_init(sys, &data, Some(context), None, cx)
}

fn read(sys: &AttributeSystem, cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    if parity(cx) != ParityMode::Reference {
        return Err(NotYet::not_read(
            "RandomAttributeValueResolver in Off mode: its native is not read",
            RECORD,
        )
        .into());
    }
    let integer = flag("bUseIntegerRandomization", &required(cx, resolver, "bUseIntegerRandomization")?)?;
    let low = bound(sys, cx, resolver, context, "LowerBound")?;
    let high = bound(sys, cx, resolver, context, "UpperBound")?;
    random_between(low, high, integer, cx)
}

impl ValueResolver for Random {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(sys, cx, resolver, context)
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
                "RandomAttributeValueResolver add, remove or set-base: the C++ has no write for this class",
                CPP,
            ),
            _ => NotYet::not_read("RandomAttributeValueResolver add, remove or set-base", RECORD),
        }
        .into())
    }
}
