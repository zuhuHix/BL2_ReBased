//! `WillowGame.AmmoDropWeightAttributeValueResolver`: the weight of an ammo or health drop from what the player
//! holds of a resource (lane FIX-ATTR-RESOLVERS, decision 2026-11-16-73). The class declares `Resource`,
//! `ResourceThreshold`, `AboveThresholdWeight`, `MinBelowThresholdWeight`, `MaxBelowThresholdWeight` and
//! `NoPoolWeight` (initialization data but the resource), and no script function: its get is a native read in
//! NATIVE_ATTR_RESOLVERS.md; the Off implementation below follows that note. The death roll's ammo and health pools
//! weigh their entries with it.
//! **The native reading remains UNVERIFIED in the running game**.
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
//! - `ParityMode::Off`, the native as NATIVE_ATTR_RESOLVERS.md section 4 reads it (lane FIX-ATTR-RESOLVERS-OFF, the
//!   fresh implementer, decision 2026-11-16-80). UNVERIFIED in the game:
//!   - `ResourceThreshold` first; with no `Resource`, `NoPoolWeight`; with one, `MinBelowThresholdWeight` and
//!     `MaxBelowThresholdWeight` next, before any pool.
//!   - Every controller of the world's list in order ([`EngineQuery::WorldController`]), each one's pool of the
//!     resource ([`EngineQuery::ControllerPool`]; `None` skips the controller). A pool still `bCreatedAndNotModified`
//!     is skipped; every other pool counts. (The note's network-client clearing of that flag is not modelled: these
//!     worlds are standalone.)
//!   - A counting pool, with `M = MaxValue * threshold`, `m = MinValue`, `c = CurrentValue`, `f = (M - c) / (M - m)`:
//!     adds nothing when `M - m` is not above 0, `c` is below 0 or above `M`, or `f` is below 0; else adds
//!     `Min + (Max - Min) * min(f, 1)` to a sum.
//!   - The answer: no pool counted (or an empty list), `NoPoolWeight`; pools counted but none adding,
//!     `AboveThresholdWeight` (evaluated then); else the sum. Single precision.
//!   - No world yet answers either query: the controller virtual and the provider method are not read (the note's
//!     "Open"), so the walk is `NotYet` at the first query on today's worlds.
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EngineQuery, EvalCx, Storage, ValueResolver};
use crate::eval::{evaluate_init, flag, init_data, number, object, parity, property, required};
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

/// The longest controller list walked (a Rust-only bound against a world that never ends its list).
const MAX_CONTROLLERS: u32 = 4096;

/// A float property the pool class must have.
fn pool_required(cx: &mut EvalCx<'_>, pool: ObjRef, name: &str) -> Result<f32, AttrError> {
    number(name, &required(cx, pool, name)?)
}

/// The native's value (module documentation).
fn native(sys: &AttributeSystem, cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    let threshold = weight(sys, cx, resolver, context, "ResourceThreshold")?;
    let resource = match property(cx, resolver, "Resource")? {
        Some(v) => object("Resource", &v)?,
        None => None,
    };
    let Some(resource) = resource else {
        return weight(sys, cx, resolver, context, "NoPoolWeight");
    };
    let low = weight(sys, cx, resolver, context, "MinBelowThresholdWeight")?;
    let high = weight(sys, cx, resolver, context, "MaxBelowThresholdWeight")?;
    let (mut counted, mut added, mut sum) = (false, false, 0.0f32);
    let mut index = 0u32;
    while let Some(controller) = cx.world.engine_query(EngineQuery::WorldController { index }, context)? {
        index += 1;
        if index > MAX_CONTROLLERS {
            return Err(AttrError::World(format!(
                "AmmoDropWeightAttributeValueResolver: the controller list is longer than {MAX_CONTROLLERS}"
            )));
        }
        let Some(pool) = cx.world.engine_query(EngineQuery::ControllerPool { resource }, controller)? else {
            continue;
        };
        if flag("bCreatedAndNotModified", &required(cx, pool, "bCreatedAndNotModified")?)? {
            continue;
        }
        counted = true;
        let top = pool_required(cx, pool, "MaxValue")? * threshold;
        let bottom = pool_required(cx, pool, "MinValue")?;
        let current = pool_required(cx, pool, "CurrentValue")?;
        let range = top - bottom;
        if !(range > 0.0) || current < 0.0 || current > top {
            continue;
        }
        let f = (top - current) / range;
        if f < 0.0 {
            continue;
        }
        sum += low + (high - low) * f.min(1.0);
        added = true;
    }
    if !counted {
        return weight(sys, cx, resolver, context, "NoPoolWeight");
    }
    if !added {
        return weight(sys, cx, resolver, context, "AboveThresholdWeight");
    }
    Ok(sum)
}

fn read(sys: &AttributeSystem, cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    if parity(cx) != ParityMode::Reference {
        return native(sys, cx, resolver, context);
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
