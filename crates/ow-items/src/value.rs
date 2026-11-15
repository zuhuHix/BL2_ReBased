//! Owner: lane W4-ROLL. The part value total of a roll and the sale value cap.
//!
//! **Part value total** (`InventoryPartMonetaryValueModifierTotal`; native `ComputeValueOfParts` as
//! `weapon_stats.sale_value` and NATIVE_ITEM_RULES.md section 2 read it): start from the definition's
//! `BaseMonetaryValueModifier` (0 when untagged: `Default__WillowInventoryDefinition` does not serialize it), then
//! multiply, in single precision at each step, by the `MonetaryValueMod` of every filled part in slot order and of the
//! prefix and title; an entry without one is skipped. The factor is the modifier attribute's constant (its one
//! `ConstantAttributeValueResolver`); a modifier without a constant leaves the total unknown (`None`). Verified for
//! weapons through the sale value on 40 of 40 spawned guns (the five checked calculators); UNVERIFIED for items.
//!
//! The sale value itself is the definition's `MonetaryValue` calculator fed this total and the item level, then
//! truncated and capped by [`sale_value_cap`]. The calculator reads attributes of the item's instance, so it is
//! evaluated with the stats (W4-STATS, `cards.rs`), not here.

use ow_core::num::f32r;

use crate::merge::RollData;
use crate::registry::Pv;

/// `ECurrencyType` in `Engine.upk` order, with the cap the sale value takes in that currency (read from native code,
/// NATIVE_WEAPON_RULES.md section 6; UNVERIFIED in game): Credits 99,999,999, Eridium 500, every other currency 999.
const CURRENCY_CAPS: [(&str, i64); 3] = [
    ("CURRENCY_Credits", 99_999_999),
    ("CURRENCY_Eridium", 500),
    ("CURRENCY_SeraphCrystals", 999),
];
/// The cap of a currency the enum has but [`CURRENCY_CAPS`] does not list.
const OTHER_CURRENCY_CAP: i64 = 999;

/// The sale value cap of a `FormOfCurrency` value (`weapon_stats.sale_value_cap`): Credits when untagged; a value
/// that is no `ECurrencyType` entry caps at 0. `known_currencies` is the enum's value list as the package declares it
/// (the caller reads it; the three names above are only those with their own cap).
pub fn sale_value_cap(currency: Option<&str>, known_currencies: &[&str]) -> i64 {
    let currency = currency.unwrap_or(CURRENCY_CAPS[0].0);
    if let Some((_, cap)) = CURRENCY_CAPS.iter().find(|(c, _)| *c == currency) {
        return *cap;
    }
    if known_currencies.contains(&currency) {
        OTHER_CURRENCY_CAP
    } else {
        0
    }
}

/// The part value total (module documentation): `None` when a modifier has no constant.
pub fn part_value_total(data: &RollData, definition: &str, sources: &[&str]) -> Option<f32> {
    let base = data
        .get(definition)
        .and_then(|p| p.at("BaseMonetaryValueModifier").number())
        .unwrap_or(0.0);
    let mut total = f32r(base);
    for path in sources {
        let modifier = data
            .get(path)
            .map(|p| p.at("MonetaryValueMod").clone())
            .unwrap_or(Pv::Null);
        if !modifier.truthy() {
            continue;
        }
        let factor = data.attribute_constant(&modifier)?;
        total = f32r(total * factor);
    }
    Some(total as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_follows_the_currency() {
        let known = [
            "CURRENCY_Credits",
            "CURRENCY_Eridium",
            "CURRENCY_SeraphCrystals",
            "CURRENCY_Torgue",
        ];
        assert_eq!(sale_value_cap(None, &known), 99_999_999);
        assert_eq!(sale_value_cap(Some("CURRENCY_Eridium"), &known), 500);
        assert_eq!(sale_value_cap(Some("CURRENCY_Torgue"), &known), 999);
        assert_eq!(sale_value_cap(Some("CURRENCY_Invented"), &known), 0);
    }

    #[test]
    fn no_modifier_leaves_the_base() {
        let data = RollData::new(&crate::registry::CensusInput::new(
            std::path::PathBuf::from("."),
        ));
        // Nothing is held: the base reads as 0 and no source has a modifier.
        assert_eq!(
            part_value_total(&data, "Test.Def", &["Test.Part"]),
            Some(0.0)
        );
    }
}
