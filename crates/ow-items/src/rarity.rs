//! Owner: lane W4-ROLL. Rarity: not rolled, it follows from the parts (NATIVE_ITEM_RULES.md section 2 and
//! NATIVE_WEAPON_RULES.md section 4, read from native code and data; UNVERIFIED in game in detail).
//!
//! **Level** = trunc of the definition's `BaseRarity` (0 when untagged) plus trunc of the `Rarity` of each filled
//! part slot in slot order and of the two name parts. A term with no value (it needs runtime state, or the oracle's
//! evaluator raises) adds 0 and is listed as unresolved, as `item_roll.py` reads it; how a value is read is the
//! caller's (Reference: `item_roll.number`; Off mode: the evaluator).
//!
//! **Name parts in the sum** ([`NAME_PARTS_IN_RARITY`]) is a reading, not a Reference stand-in (architecture section
//! 6.1): which setting the item rarity sum uses was not resolved in the item code; the weapon rarity on record sums
//! both name parts (NATIVE_WEAPON_RULES.md section 12, where every weapon name part has rarity 0), and six legendary
//! shield titles carry a rarity. The same reading in both modes, UNVERIFIED; real-game question 6 settles it. It
//! moves three legendary shields from Very Rare to Legendary (ITEM_ROLL.md section 2).
//!
//! **Band**: the `RarityRating` of the first `GlobalsDefinition.RarityLevelColors` entry whose `[MinLevel, MaxLevel]`
//! holds the level, by name without its prefix (no colours: decision 2026-11-08-02); a level in no entry has no
//! band. **Not implemented**: the mission-item exception (a `MissionItemDefinition` takes
//! `GlobalsDefinition.MissionItemRarityLevel`, 500, instead of the sum); the roll rolls no mission item.

use crate::ItemError;
use crate::merge::RollData;
use crate::registry::Pv;

/// `math.trunc(x)` to an integer, refused where Python's raises or the result leaves `i64` (N5: no `as` on an
/// unchecked float): `x` must be finite and truncate into `i64`. `what` names the oracle step for the message.
pub(crate) fn trunc_i64(x: f64, what: &str) -> Result<i64, ItemError> {
    // 2^63 is exactly representable; every finite f64 strictly inside (-2^63 - 1, 2^63) truncates into i64.
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    let t = x.trunc();
    if !t.is_finite() || !(-LIMIT..LIMIT).contains(&t) {
        return Err(ItemError::BadData(format!(
            "{what}: {x} does not truncate to an integer"
        )));
    }
    // In range and integral: the conversion is exact.
    Ok(t as i64)
}

/// [`trunc_i64`] into `i32`.
pub(crate) fn trunc_i32(x: f64, what: &str) -> Result<i32, ItemError> {
    let t = trunc_i64(x, what)?;
    i32::try_from(t).map_err(|_| ItemError::BadData(format!("{what}: {t} is out of range")))
}

/// The globals definition the bands are read from (`weapon_stats.GLOBALS`).
pub const GLOBALS: &str = "GD_Globals.General.Globals";

/// The name parts count in the item rarity sum (module documentation; UNVERIFIED reading, question 6).
pub const NAME_PARTS_IN_RARITY: bool = true;

/// One roll's rarity.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rarity {
    pub level: i64,
    pub band: Option<String>,
    /// The sources whose term had no value (counted as 0).
    pub unresolved: Vec<String>,
    pub name_parts_counted: bool,
}

/// A number reader for rarity terms: `None` when the term has no value.
pub type TermOf<'a> = dyn FnMut(&Pv) -> Result<Option<f64>, ItemError> + 'a;

/// The truncated terms `(source, value or None)` of the module documentation's sum: the definition, the parts (in
/// the order given), then the name parts when [`NAME_PARTS_IN_RARITY`].
pub fn terms(
    data: &RollData,
    definition: &str,
    parts: &[&str],
    names: &[Option<&str>],
    value: &mut TermOf<'_>,
) -> Result<Vec<(String, Option<i64>)>, ItemError> {
    let mut out = Vec::new();
    let read = |path: &str, key: &str, value: &mut TermOf<'_>| -> Result<Option<i64>, ItemError> {
        let raw = data
            .get(path)
            .map(|p| p.at(key).clone())
            .unwrap_or(Pv::Null);
        let v = if matches!(raw, Pv::Null) {
            Some(0.0)
        } else {
            value(&raw)?
        };
        // item_roll.rarity_terms: math.trunc(value) of each term.
        v.map(|x| trunc_i64(x, "a rarity term (item_roll.rarity_terms)"))
            .transpose()
    };
    out.push((
        definition.to_string(),
        read(definition, "BaseRarity", value)?,
    ));
    let mut sources: Vec<&str> = parts.to_vec();
    if NAME_PARTS_IN_RARITY {
        sources.extend(names.iter().flatten());
    }
    for source in sources {
        out.push((source.to_string(), read(source, "Rarity", value)?));
    }
    Ok(out)
}

/// The band of `level` (module documentation).
pub fn band(data: &RollData, level: i64) -> Option<String> {
    let globals = data.get(GLOBALS)?;
    for row in globals.at("RarityLevelColors").items() {
        let low = row.at("MinLevel").number().unwrap_or(0.0);
        let high = row.at("MaxLevel").number().unwrap_or(0.0);
        let l = level as f64;
        if low <= l && l <= high {
            let rating = row.at("RarityRating").as_str()?;
            return Some(match rating.split_once('_') {
                Some((_, rest)) => rest.to_string(),
                None => rating.to_string(),
            });
        }
    }
    None
}

/// item_roll.rarity's `sum(v for _, v in terms if v is not None)`: Python's integers do not overflow, so a sum that
/// leaves `i64` is refused instead of wrapping or panicking.
pub(crate) fn sum_terms(terms: &[(String, Option<i64>)]) -> Result<i64, ItemError> {
    terms
        .iter()
        .filter_map(|(_, v)| *v)
        .try_fold(0i64, |sum, v| sum.checked_add(v))
        .ok_or_else(|| {
            ItemError::BadData("item_roll.rarity: the rarity sum leaves the i64 range".to_string())
        })
}

/// The rarity of one roll (module documentation).
pub fn rarity(
    data: &RollData,
    definition: &str,
    parts: &[&str],
    names: &[Option<&str>],
    value: &mut TermOf<'_>,
) -> Result<Rarity, ItemError> {
    let terms = terms(data, definition, parts, names, value)?;
    let level = sum_terms(&terms)?;
    Ok(Rarity {
        level,
        band: band(data, level),
        unresolved: terms
            .into_iter()
            .filter(|(_, v)| v.is_none())
            .map(|(s, _)| s)
            .collect(),
        name_parts_counted: NAME_PARTS_IN_RARITY,
    })
}
