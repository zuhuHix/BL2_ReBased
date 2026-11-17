//! `GearboxFramework.RandomAttributeValueResolver`: a random number between two bounds as an attribute value (lane
//! FIX-ATTR-RESOLVERS, decision 2026-11-16-73). The class declares `LowerBound` and `UpperBound` (initialization
//! data), `bUseIntegerRandomization` and a value lifetime with saved values, and no script function: its get is a
//! native read in NATIVE_ATTR_RESOLVERS.md; the Off implementation below follows that note. The ammo pools'
//! quantities and the adult bullymong's item stage offset read it. **The native reading remains UNVERIFIED in the running game**.
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ stand-in (`src/progression.cpp`, `RandomAttributeValueResolver` and
//!   `randomBetween`): the two bounds evaluated on the context, lower first, then one draw from the evaluator's
//!   generator (`CrtStream::AttributeVariance`) mapped as the definition's random variance maps it
//!   (`crate::eval::random_between`: real `low + d * (high - low)`, integer `ceil(low) + floor(d * span)` inclusive);
//!   the draw **replaces** the running value. `ValueLifetime` and the saved values are not modelled: a fresh draw on
//!   every read, as the C++.
//! - `ParityMode::Off`, the native as NATIVE_ATTR_RESOLVERS.md section 3 reads it (lane FIX-ATTR-RESOLVERS-OFF, the
//!   fresh implementer, decision 2026-11-16-80). UNVERIFIED in the game:
//!   - **The draw:** both bounds evaluated on the context, lower first, then one 15-bit draw `r` of the attribute
//!     stream (`CrtStream::AttributeVariance`, the project's stream for attribute randomness; which C `rand()` the
//!     engine's call shares is not read). Real: `low + r / 32767 * (high - low)` in single precision (both ends
//!     reachable). Integer: `span = trunc(high) + 1 - trunc(low)`; below 1, `trunc(low)` with no draw; else
//!     `trunc(low) + trunc(r / 32768 * span)`. The note does not give the precision of that product; for a span up to
//!     512 single and double give the same pick, so a wider span is `NotYet` (decision 80's question).
//!   - **The lifetime:** `ValueLifetime` is evaluated first. At most 0.0001: a fresh draw, nothing saved. Above: the
//!     resolver's own `SavedRandomValues` is walked in order, keyed by the context's object name, against the world's
//!     `TimeSeconds` (0 with no world): an entry expired before the clock is removed as it is met; the first live
//!     entry of the key answers its `SavedValue` (later entries untouched); with none, a fresh draw is appended with
//!     expiration clock + lifetime and answered. The bounds are evaluated only for a draw (the note places their
//!     evaluation in the draw).
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes.

use ow_core::rand::CrtStream;
use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::eval::{elements, evaluate_init, field, flag, init_data, name, number, parity, random_between, required};
use crate::registry::AttributeSystem;

pub(crate) const RECORD: &str = "NATIVE_ATTR_RESOLVERS.md (RandomAttributeValueResolver)";
const CPP: &str = "src/progression.cpp (RandomAttributeValueResolver, randomBetween)";

pub(crate) struct Random;

/// The bound `name` of the resolver, evaluated on the context.
fn bound(
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

/// Values at or below this lifetime are not saved (the note).
const KEEP_ABOVE: f32 = 0.0001;

/// The widest integer span whose pick is the same in single and double precision: `r / 32768` is exact in both, and
/// `r * span` stays below 2^24 for `r < 2^15` and `span <= 2^9`.
const EXACT_SPAN: i64 = 512;

/// `trunc`: the float-to-int conversion toward zero, for a value in the int range; outside it (or not finite) bad
/// data, since what the engine's conversion gives there is not read.
fn trunc(what: &str, x: f32) -> Result<i64, AttrError> {
    // i32::MIN and i32::MAX + 1 are exact in f32, so the check is exact and the cast below cannot saturate.
    if !(x.is_finite() && x >= -2_147_483_648.0 && x < 2_147_483_648.0) {
        return Err(AttrError::BadData {
            reason: format!("RandomAttributeValueResolver: {what} {} is outside the int range", ow_core::fmt::fmt_g(f64::from(x), 9)).into(),
        });
    }
    Ok(x as i64)
}

/// The native's draw (module documentation).
fn draw(sys: &AttributeSystem, cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    let integer = flag("bUseIntegerRandomization", &required(cx, resolver, "bUseIntegerRandomization")?)?;
    let low = bound(sys, cx, resolver, context, "LowerBound")?;
    let high = bound(sys, cx, resolver, context, "UpperBound")?;
    if !integer {
        let r = f32::from(cx.world.rng(CrtStream::AttributeVariance)?.rand15());
        return Ok(low + r / 32767.0 * (high - low));
    }
    let (first, last) = (trunc("LowerBound", low)?, trunc("UpperBound", high)?);
    let span = last + 1 - first;
    if span < 1 {
        // In the int range, so exact up to 2^24 and rounded beyond, as the engine's int-to-float conversion.
        return Ok(first as f32);
    }
    if span > EXACT_SPAN {
        return Err(NotYet::not_read(
            format!(
                "RandomAttributeValueResolver: an integer span of {span}, where the precision of r / 32768 * span \
                 (not given by the note) changes the pick"
            ),
            RECORD,
        )
        .into());
    }
    let r = f64::from(cx.world.rng(CrtStream::AttributeVariance)?.rand15());
    // Exact for this span in either precision (EXACT_SPAN); the product is below `span`, so the cast cannot saturate.
    let pick = (r / 32768.0 * span as f64).trunc() as i64;
    Ok((first + pick) as f32)
}

/// Writes field `name` of the struct value `v`.
fn set_field(space: &ow_object::heap::ObjectSpace, v: &mut Value, name: &str, x: Value) -> Result<(), AttrError> {
    let bad = || AttrError::BadData { reason: format!("SavedRandomValue has no field {name}").into() };
    let id = space.names.find(name).ok_or_else(bad)?;
    let ty = v.as_struct().map(|s| s.ty).ok_or_else(bad)?;
    let (index, _) = space.classes.struct_layout(ty).and_then(|l| l.field(id)).ok_or_else(bad)?;
    let index = usize::try_from(index).map_err(|_| bad())?;
    let slot = v.struct_mut().and_then(|s| s.fields.get_mut(index)).ok_or_else(bad)?;
    *slot = x;
    Ok(())
}

/// The native's value with a lifetime (module documentation).
fn native(sys: &AttributeSystem, cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    let lifetime = bound(sys, cx, resolver, context, "ValueLifetime")?;
    if lifetime <= KEEP_ABOVE {
        return draw(sys, cx, resolver, context);
    }
    let key = cx.world.parts().space.heap.get(context)?.name;
    let clock = cx.world.time_seconds()?.unwrap_or(0.0);
    let saved = required(cx, resolver, "SavedRandomValues")?;
    let mut list = elements("SavedRandomValues", &saved)?.to_vec();
    let mut found = None;
    let mut i = 0;
    {
        let space = cx.world.parts().space;
        while i < list.len() {
            let entry = &list[i];
            if number("ExpirationTime", field(space, entry, "ExpirationTime")?)? < clock {
                list.remove(i);
                continue;
            }
            if name("ContextName", field(space, entry, "ContextName")?)? == key {
                found = Some(number("SavedValue", field(space, entry, "SavedValue")?)?);
                break;
            }
            i += 1;
        }
    }
    let space = cx.world.parts().space;
    let class = space.heap.get(resolver)?.class;
    let p = space.prop(class, "SavedRandomValues").ok_or_else(|| AttrError::BadData {
        reason: "RandomAttributeValueResolver has no SavedRandomValues".into(),
    })?;
    // Commit ordered cleanup before evaluating bounds: they may re-enter this resolver.
    space.write(resolver, PropAddr::scalar(p), Value::array(list))?;
    if let Some(value) = found {
        return Ok(value);
    }
    let value = draw(sys, cx, resolver, context)?;
    let space = cx.world.parts().space;
    let mut entry = match space.classes.decl(p).map(|d| d.kind.clone()) {
        Some(PropKind::Array(inner)) => space.classes.zero_value(&inner),
        _ => {
            return Err(AttrError::BadData {
                reason: "RandomAttributeValueResolver.SavedRandomValues is not an array".into(),
            });
        }
    };
    set_field(space, &mut entry, "ContextName", Value::Name(key))?;
    set_field(space, &mut entry, "SavedValue", Value::Float(value))?;
    set_field(space, &mut entry, "ExpirationTime", Value::Float(clock + lifetime))?;
    // Append to the current array, preserving entries saved during bound evaluation.
    let mut list = elements("SavedRandomValues", space.read(resolver, PropAddr::scalar(p))?)?.to_vec();
    list.push(entry);
    space.write(resolver, PropAddr::scalar(p), Value::array(list))?;
    Ok(value)
}

fn read(sys: &AttributeSystem, cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    if parity(cx) != ParityMode::Reference {
        return native(sys, cx, resolver, context);
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
