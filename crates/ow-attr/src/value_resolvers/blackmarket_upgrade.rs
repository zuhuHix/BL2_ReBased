//! `WillowGame.BlackMarketUpgradeAttributeValueResolver`: the context player controller's black-market upgrade level
//! (lane IMPL-BLACKMARKET-RESOLVER, decision 2026-11-19-72). The class declares one field,
//! `AssociatedBlackMarketUpgrade` (a `BlackMarketUpgradeDefinition`), and no script function: its value is native,
//! read in NATIVE_BLACKMARKET_UPGRADES.md (analyst lane ANALYST-BLACKMARKET-RESOLVER, decision 2026-11-19-63). This
//! module is the implementer half of the clean room (docs/LANE_BRIEF.md): it was coded from that note alone. The
//! installed game reaches it through the ammo pools' black-market attributes (`Att_BM_*`). **Every rule here is
//! UNVERIFIED in the running game.**
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`: the C++ has no rule for this class, so every get, get-base and write answers the same
//!   `NotYet { NotRead }` an unregistered class gives (`value resolver class ...`, NATIVE_ATTRIBUTES.md section 6).
//!   Reference is unchanged.
//! - `ParityMode::Off`, the note's sections 2 to 4 and 6. Get and get-base are one rule; the chain's previous value is
//!   not used. Given the resolved context (a None context never reaches a value resolver: `GetValue` answers before
//!   the value chain runs), in this order:
//!   1. **The controller check:** the context is accepted when it is a `WillowGame.WillowPlayerController` or a
//!      subclass (the class is the note's inference, UNVERIFIED). When the space declares no such class, every
//!      context is accepted, as the note reads the by-name lookup failing.
//!   2. **The index**, whether or not step 1 accepted the context. No `AssociatedBlackMarketUpgrade`: -1, nothing
//!      read or written. A warm cache (the upgrade's `CachedUpgradeIndex` is not -1): that index, with no globals
//!      lookup. A cold cache (-1): [`AttrWorld::globals_definition`]; with none found, the index stays -1 and nothing
//!      is written; otherwise every non-None entry of its `BlackMarketUpgrades` is assigned its position (a duplicate
//!      ends with its last position), and the asked upgrade's field is read again (an unlisted upgrade stays -1).
//!   3. **The result:** with an accepted context and an index other than -1, the controller's `BlackMarketUpgrades`
//!      entry at the index as a float, after the array is grown with zeros to index + 1 entries when it is shorter
//!      (existing entries kept). Otherwise 0.0, the controller untouched.
//!
//!   **The write order** (the hub's ruling in review r1 of #451): every check is made before either write (both
//!   effects are decided, the cache fields and the array are read and their kinds checked, the listed upgrades are
//!   loaded), then the cache assignments are committed, then the array is zero-filled, then the entry is read. On a
//!   cold lookup the index the growth is decided for comes from the pending assignments (the last occurrence of a
//!   duplicate, -1 for an unlisted upgrade), not from a reread of the field. A refusal from those checks leaves the
//!   cache fields and the array as they were. No Off rollback exists or is needed: nothing after the first write can
//!   refuse except a heap error. Errors propagate unchanged: [`AttrWorld::globals`]'s
//!   `NotYet { LayerAbsent }` (a world without engine globals) and the store's are not "no globals found". Bad data:
//!   a cache field that is not an int, a globals definition without a `BlackMarketUpgrades` array of objects, a
//!   controller without a `BlackMarketUpgrades` array of ints. The note's two `NotYet { NotPorted }` cases (a needed
//!   write the world cannot make, [`CACHE_WRITE`] and [`ARRAY_WRITE`]) arise when the object to write is a stand-in
//!   holding no properties, which the object space cannot write; both are refused before any write.
//!
//! **Writes** (add, remove, set-base): in Off, the class default: `storage` is `Ok(None)`, so each answers false and
//! nothing changes.

use ow_core::{NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr, PropId};
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::context_resolvers::is_a;
use crate::eval::{ensure_loaded, object, parity, required};
use crate::registry::AttributeSystem;

/// The resolver's class path in the packages.
pub(crate) const CLASS: &str = "WillowGame.BlackMarketUpgradeAttributeValueResolver";
/// The class a context must be, a subclass included (note section 2, inferred there).
pub(crate) const CONTROLLER: &str = "WillowGame.WillowPlayerController";
/// The cache field's value for "not computed" (note section 3).
const COLD: i32 = -1;
/// The note's names for the two writes a world cannot make (section 6, "Off answers, per case").
pub(crate) const CACHE_WRITE: &str =
    "BlackMarketUpgradeAttributeValueResolver: the cold CachedUpgradeIndex assignment";
pub(crate) const ARRAY_WRITE: &str =
    "BlackMarketUpgradeAttributeValueResolver: zero-filling the controller's BlackMarketUpgrades";
/// Where the rule is written down.
const NOTE: &str = "NATIVE_BLACKMARKET_UPGRADES.md section 6";

/// Whether `o` is a stand-in that holds no properties although its class has some: the object space cannot write
/// it (a write is a missing slot), so a needed write to it is the note's `NotYet { NotPorted }`.
fn stand_in(space: &ObjectSpace, o: ObjRef) -> Result<bool, AttrError> {
    let obj = space.heap.get(o)?;
    let fields = space.classes.get_layout(obj.class).map_or(0, |l| l.slot_count());
    Ok(obj.slots.is_empty() && fields > 0)
}

pub(crate) struct BlackMarketUpgrade;

/// What the registry answers for a class it does not implement: Reference keeps it.
fn reference_refusal() -> AttrError {
    NotYet::not_read(
        format!("value resolver class {CLASS}"),
        "NATIVE_ATTRIBUTES.md section 6",
    )
    .into()
}

fn bad(reason: String) -> AttrError {
    AttrError::BadData {
        reason: format!("BlackMarketUpgradeAttributeValueResolver: {reason}").into(),
    }
}

/// The property `name` of `obj`'s class, or bad data naming both.
fn prop_of(space: &ObjectSpace, obj: ObjRef, name: &str, what: &str) -> Result<PropId, AttrError> {
    let class = space.heap.get(obj)?.class;
    space
        .prop(class, name)
        .ok_or_else(|| bad(format!("{what} has no {name}")))
}

/// The upgrade definition's `CachedUpgradeIndex` and its property (loaded first).
fn cache_field(cx: &mut EvalCx<'_>, upgrade: ObjRef) -> Result<(PropId, i32), AttrError> {
    ensure_loaded(cx, upgrade)?;
    let space = cx.world.parts().space;
    let p = prop_of(space, upgrade, "CachedUpgradeIndex", "an upgrade definition")?;
    match space.read(upgrade, PropAddr::scalar(p))? {
        Value::Int(i) => Ok((p, *i)),
        other => Err(bad(format!(
            "CachedUpgradeIndex is a {}, not an int",
            other.kind_name()
        ))),
    }
}

/// The cold-cache walk's assignments (note section 3), decided and checked, not written: `(upgrade, field,
/// position)` for every non-None entry, in list order. `None`: no globals definition is found.
fn plan_cache(cx: &mut EvalCx<'_>) -> Result<Option<Vec<(ObjRef, PropId, i32)>>, AttrError> {
    let Some(definition) = cx.world.globals_definition()? else {
        return Ok(None);
    };
    ensure_loaded(cx, definition)?;
    let list = match required(cx, definition, "BlackMarketUpgrades")? {
        Value::Array(items) => items,
        other => {
            return Err(bad(format!(
                "the globals definition's BlackMarketUpgrades is a {}, not an array",
                other.kind_name()
            )));
        }
    };
    let mut plan = Vec::new();
    for (position, entry) in list.iter().enumerate() {
        let upgrade = match entry {
            Value::Object(o) => *o,
            other => {
                return Err(bad(format!(
                    "a BlackMarketUpgrades entry of the globals definition is a {}, not an object",
                    other.kind_name()
                )));
            }
        };
        let Some(upgrade) = upgrade else { continue };
        ensure_loaded(cx, upgrade)?;
        if stand_in(cx.world.parts().space, upgrade)? {
            return Err(NotYet::not_ported(CACHE_WRITE, NOTE).into());
        }
        let (p, _) = cache_field(cx, upgrade)?;
        let position = i32::try_from(position)
            .map_err(|_| bad("the globals definition's BlackMarketUpgrades is longer than an int".to_string()))?;
        plan.push((upgrade, p, position));
    }
    Ok(Some(plan))
}

/// The controller's `BlackMarketUpgrades` (an int array) and its property.
fn levels(space: &ObjectSpace, controller: ObjRef) -> Result<(PropId, Vec<i32>), AttrError> {
    let p = prop_of(space, controller, "BlackMarketUpgrades", "the player controller")?;
    let items = match space.read(controller, PropAddr::scalar(p))? {
        Value::Array(items) => items.clone(),
        other => {
            return Err(bad(format!(
                "the controller's BlackMarketUpgrades is a {}, not an array",
                other.kind_name()
            )));
        }
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        match item {
            Value::Int(i) => out.push(*i),
            other => {
                return Err(bad(format!(
                    "a BlackMarketUpgrades entry of the controller is a {}, not an int",
                    other.kind_name()
                )));
            }
        }
    }
    Ok((p, out))
}

/// Step 1: whether `context` counts as a controller (module documentation).
fn accepted(space: &ObjectSpace, context: ObjRef) -> Result<bool, AttrError> {
    if space.classes.by_path(CONTROLLER).is_none() {
        return Ok(true);
    }
    is_a(space, context, CONTROLLER)
}

/// The Off rule (module documentation).
fn native(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    // Step 1, read only.
    let controller = accepted(cx.world.parts().space, context)?;
    // Step 2: the index, and the cache assignments it needs.
    let Some(upgrade) = object(
        "AssociatedBlackMarketUpgrade",
        &required(cx, resolver, "AssociatedBlackMarketUpgrade")?,
    )?
    else {
        return Ok(0.0);
    };
    let (field, cached) = cache_field(cx, upgrade)?;
    let (plan, index) = if cached != COLD {
        (Vec::new(), cached)
    } else {
        match plan_cache(cx)? {
            None => (Vec::new(), COLD),
            Some(plan) => {
                // The effective index comes from the pending assignments, not a reread of the field (the hub's
                // ruling in the final round of #451): the last occurrence of the asked upgrade, or -1 when it is not
                // listed.
                let index = plan
                    .iter()
                    .rev()
                    .find(|(u, _, _)| *u == upgrade)
                    .map_or(COLD, |(_, _, i)| *i);
                (plan, index)
            }
        }
    };
    // Step 3's write, decided and checked before any write.
    let growth = if controller && index != COLD {
        let at = usize::try_from(index)
            .map_err(|_| bad(format!("CachedUpgradeIndex {index} is below -1")))?;
        let space = cx.world.parts().space;
        if stand_in(space, context)? {
            // The controller holds no properties: its array cannot be grown (nor read). Refused before any write.
            return Err(NotYet::not_ported(ARRAY_WRITE, NOTE).into());
        }
        let (p, mut items) = levels(space, context)?;
        let grow = items.len() <= at;
        if grow {
            items.resize(at + 1, 0);
        }
        Some((p, items, at, grow))
    } else {
        None
    };
    // The writes: the cache, then the zero-fill.
    let space = cx.world.parts().space;
    for (u, p, position) in &plan {
        space.write(*u, PropAddr::scalar(*p), Value::Int(*position))?;
    }
    debug_assert!(plan.is_empty() || {
        matches!(space.read(upgrade, PropAddr::scalar(field)), Ok(Value::Int(i)) if *i == index)
    });
    let Some((p, items, at, grow)) = growth else {
        return Ok(0.0);
    };
    if grow {
        let values = items.iter().map(|i| Value::Int(*i)).collect();
        space.write(context, PropAddr::scalar(p), Value::array(values))?;
    }
    // The entry, an int as a float.
    Ok(items[at] as f32)
}

fn read(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    match parity(cx) {
        ParityMode::Reference => Err(reference_refusal()),
        _ => native(cx, resolver, context),
    }
}

impl ValueResolver for BlackMarketUpgrade {
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
        match parity(cx) {
            ParityMode::Reference => Err(reference_refusal()),
            _ => Ok(None),
        }
    }
}
