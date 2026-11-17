//! `WillowGame.AmmoDropWeightAttributeValueResolver`: the weight of an ammo or health drop from what the player
//! holds of a resource (lane FIX-ATTR-RESOLVERS, decision 2026-11-16-73). The class declares `Resource`,
//! `ResourceThreshold`, `AboveThresholdWeight`, `MinBelowThresholdWeight`, `MaxBelowThresholdWeight` and
//! `NoPoolWeight` (initialization data but the resource), and no script function: its get is a native read in
//! NATIVE_ATTR_RESOLVERS.md; the Off rule awaits a fresh implementer. The death roll's ammo and health pools weigh their entries with it.
//! **Nothing here is a rule of the game**, and nothing is verified in the running game.
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ stand-in (`src/progression.cpp`, `ammoDropWeight`; COMBAT_LOOT.md calls the
//!   shape and the direction guesses): the local player's pool for `Resource`
//!   ([`EngineQuery::LocalPlayerPool`], whatever the context, as the C++'s `playerPool`); with none, `NoPoolWeight`.
//!   Otherwise `share = CurrentValue / MaxValue` (0 when the maximum is not above 0) and `ResourceThreshold`; at or
//!   above it `AboveThresholdWeight`; below it `Max + (share / threshold) * (Min - Max)` of the two below-threshold
//!   weights (`Min` when the threshold is not above 0). Single precision, the weights evaluated on the context in
//!   the C++'s order (no-pool, or threshold, then above, or min then max).
//! - `ParityMode::Off`: `NotYet { NotRead }`; the native is read in NATIVE_ATTR_RESOLVERS.md,
//!   and the Off rule awaits a fresh implementer.
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EngineQuery, EvalCx, Storage, ValueResolver};
use crate::eval::{evaluate_init, init_data, number, object, parity, property, required};
use crate::registry::AttributeSystem;

pub(crate) const RECORD: &str = "NATIVE_ATTR_RESOLVERS.md (AmmoDropWeightAttributeValueResolver)";
const CPP: &str = "src/progression.cpp (ammoDropWeight)";

pub(crate) struct AmmoDropWeight;

/// The weight `name` of the resolver, evaluated on the context.
fn weight(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    resolver: ObjRef,
    context: ObjRef,
    name: &str,
) -> Result<f32, AttrError> {
    let v = required(cx, resolver, name)?;
    let data = init_data(cx.world.parts().space, &v)?;
    evaluate_init(sys, &data, Some(context), None, cx)
}

/// A float property of the pool (`MaxValue`, `CurrentValue`); a pool class without it is what the C++ reads as 0.
fn pool_number(cx: &mut EvalCx<'_>, pool: ObjRef, name: &str) -> Result<f32, AttrError> {
    match property(cx, pool, name)? {
        Some(v) => number(name, &v),
        None => Ok(0.0),
    }
}

fn read(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    resolver: ObjRef,
    context: ObjRef,
) -> Result<f32, AttrError> {
    if parity(cx) != ParityMode::Reference {
        return Err(NotYet::not_read(
            "AmmoDropWeightAttributeValueResolver in Off mode: native read in NATIVE_ATTR_RESOLVERS.md; the Off rule awaits a fresh implementer",
            RECORD,
        )
        .into());
    }
    let resource = match property(cx, resolver, "Resource")? {
        Some(v) => object("Resource", &v)?,
        None => None,
    };
    let pool = match resource {
        Some(resource) => cx
            .world
            .engine_query(EngineQuery::LocalPlayerPool { resource }, context)?,
        None => None,
    };
    let Some(pool) = pool else {
        return weight(sys, cx, resolver, context, "NoPoolWeight");
    };
    let maximum = pool_number(cx, pool, "MaxValue")?;
    let share = if maximum > 0.0 {
        pool_number(cx, pool, "CurrentValue")? / maximum
    } else {
        0.0
    };
    let threshold = weight(sys, cx, resolver, context, "ResourceThreshold")?;
    if share >= threshold {
        return weight(sys, cx, resolver, context, "AboveThresholdWeight");
    }
    let low = weight(sys, cx, resolver, context, "MinBelowThresholdWeight")?;
    let high = weight(sys, cx, resolver, context, "MaxBelowThresholdWeight")?;
    Ok(if threshold > 0.0 {
        high + (share / threshold) * (low - high)
    } else {
        low
    })
}

impl ValueResolver for AmmoDropWeight {
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
                "AmmoDropWeightAttributeValueResolver add, remove or set-base: the C++ has no write for this class",
                CPP,
            ),
            _ => NotYet::not_read("AmmoDropWeightAttributeValueResolver add, remove or set-base", RECORD),
        }
        .into())
    }
}
