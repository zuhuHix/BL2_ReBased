//! Owner: lane W4-ROLL. Name parts: the prefix and title a roll gets (no random draw).
//!
//! **The rule** (`weapon_recipe.choose_name_parts` and `best_name_part`, read from native code; verified for
//! weapons on 1,600 of 1,600 game-rolled names, UNVERIFIED for items): start from the definition's `PrefixList` /
//! `TitleList`, then each chosen part's in slot order. Within one list the qualifying entry with the highest
//! `Priority` wins, a later one on ties; a later list replaces the current pick when its pick's priority is at least
//! the current one's. An entry qualifies when it has a `PartName`, its level window holds the level
//! (`MinExpLevelRequirement` 1 and `MaxExpLevelRequirement` 100 when untagged), its `Priority` (1 when untagged) is
//! above 0 and every one of its `Expressions` holds (an empty list holds).
//!
//! **Expressions.** How they are evaluated is the caller's ([`ExprHold`]):
//!
//! - Reference: the oracle's subset ([`tool_expressions_hold`]): an operand `Weapon_Is_<Maker>` is 1 for the item's
//!   own manufacturer by object name, an operand whose only value resolver is a `DamageTypeAttributeValueResolver`
//!   (`WeaponDamage_Is_<Element>`) compares the damage type's object name with each entry's
//!   `AssociatedDamageTypeName`; any other operand, or a second attribute operand, excludes the name part. Items have
//!   no damage type here (`item_roll.py`), so a damage expression on an item compares the not-matched value.
//! - Off mode: the `ow-attr` evaluator (`EvaluateExpressions` with the item as the source, `roll.rs`), which needs the
//!   item's instance object.
//!
//! The damage type the weapon names see is the elemental part's `CustomDamageTypeDefinition`, else the weapon type's
//! `DefaultDamageTypeDefinition` (`weapon_recipe.native_damage_type`, read from native code, NATIVE_WEAPON_RULES.md
//! section 12; UNVERIFIED in game).

use ow_core::num::f32r;

use crate::merge::RollData;
use crate::registry::Pv;
use crate::roll::RollError;

/// What the name expressions of one roll see.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NameContext<'a> {
    pub level: Option<i64>,
    pub manufacturer: Option<&'a str>,
    pub damage_type: Option<&'a str>,
}

impl<'a> NameContext<'a> {
    pub fn new(
        level: Option<i64>,
        manufacturer: Option<&'a str>,
        damage_type: Option<&'a str>,
    ) -> Self {
        NameContext {
            level,
            manufacturer,
            damage_type,
        }
    }
}

/// Whether a name part's `Expressions` list holds.
pub type ExprHold<'a> = dyn FnMut(&Pv, &NameContext<'_>) -> Result<bool, RollError> + 'a;

/// `weapon_recipe.name_priority`: `Priority`, 1 when untagged (`Default__WeaponNamePartDefinition`).
pub fn name_priority(data: &RollData, name_part: &str) -> f64 {
    data.get(name_part)
        .and_then(|p| p.at("Priority").number())
        .unwrap_or(1.0)
}

/// `weapon_recipe.name_part_qualifies` (module documentation).
pub fn qualifies(
    data: &RollData,
    name_part: &str,
    cx: &NameContext<'_>,
    holds: &mut ExprHold<'_>,
) -> Result<bool, RollError> {
    let Some(props) = data.get(name_part) else {
        return Ok(false);
    };
    if !props.at("PartName").truthy() {
        return Ok(false);
    }
    let low = props.at("MinExpLevelRequirement").number().unwrap_or(1.0);
    let high = props.at("MaxExpLevelRequirement").number().unwrap_or(100.0);
    if let Some(level) = cx.level {
        let level = level as f64;
        if !(low <= level && level <= high) {
            return Ok(false);
        }
    }
    // `not priority > 0`: NaN does not qualify either.
    if name_priority(data, name_part).partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return Ok(false);
    }
    holds(props.at("Expressions"), cx)
}

/// `weapon_recipe.best_name_part`: the qualifying entry of one list with the highest priority, a later one on ties.
pub fn best_name_part(
    data: &RollData,
    names: &[Pv],
    cx: &NameContext<'_>,
    holds: &mut ExprHold<'_>,
) -> Result<Option<String>, RollError> {
    let mut best: Option<String> = None;
    for name in names.iter().filter_map(Pv::as_str) {
        if qualifies(data, name, cx, holds)?
            && best
                .as_deref()
                .is_none_or(|b| name_priority(data, b) <= name_priority(data, name))
        {
            best = Some(name.to_string());
        }
    }
    Ok(best)
}

/// The prefix or title over `lists` (the definition's first, then each chosen part's in slot order): the
/// module documentation's rule.
pub fn choose(
    data: &RollData,
    lists: &[Vec<Pv>],
    cx: &NameContext<'_>,
    holds: &mut ExprHold<'_>,
) -> Result<Option<String>, RollError> {
    let mut lists = lists.iter();
    let mut current = match lists.next() {
        Some(own) => best_name_part(data, own, cx, holds)?,
        None => None,
    };
    for names in lists {
        let candidate = best_name_part(data, names, cx, holds)?;
        match (&current, &candidate) {
            (None, _) => current = candidate,
            (Some(c), Some(n)) if name_priority(data, c) <= name_priority(data, n) => {
                current = candidate
            }
            _ => {}
        }
    }
    Ok(current)
}

/// `weapon_recipe.native_damage_type`: the elemental part's `CustomDamageTypeDefinition`, else the weapon type's
/// `DefaultDamageTypeDefinition`.
pub fn native_damage_type(
    data: &RollData,
    elemental: Option<&str>,
    weapon_type: Option<&str>,
) -> Option<String> {
    if let Some(part) = elemental
        && let Some(custom) = data.get(part).and_then(|p| {
            p.at("CustomDamageTypeDefinition")
                .as_str()
                .map(str::to_string)
        })
    {
        return Some(custom);
    }
    weapon_type.and_then(|t| data.get(t)).and_then(|p| {
        p.at("DefaultDamageTypeDefinition")
            .as_str()
            .map(str::to_string)
    })
}

// ---- the oracle's expression subset (Reference) -------------------------------------------------------------------

const DAMAGE_TYPE_RESOLVER: &str = "DamageTypeAttributeValueResolver";

/// `([(damage type name, value if matched)], value if not matched)`.
type Selector = (Vec<(Option<String>, f64)>, f64);

/// `weapon_recipe.damage_type_selector`: the [`Selector`] of an attribute whose only value resolver is a damage-type
/// resolver, else `None`.
fn damage_type_selector(data: &RollData, attribute: &str) -> Option<Selector> {
    let props = data.props(attribute).ok()?;
    let listed: Vec<String> = props
        .at("ValueResolverChain")
        .items()
        .iter()
        .filter_map(|p| p.as_str().map(str::to_string))
        .collect();
    let chain = if props.at("ValueResolverChain").truthy() {
        listed
    } else {
        data.below_of_class(attribute, DAMAGE_TYPE_RESOLVER)
    };
    if chain.len() != 1 {
        return None;
    }
    let resolver = &chain[0];
    let named = resolver
        .rsplit('.')
        .next()
        .is_some_and(|n| n.starts_with(DAMAGE_TYPE_RESOLVER));
    if !(data.class_of(resolver).ends_with(DAMAGE_TYPE_RESOLVER) || named) {
        return None;
    }
    let props = data.props(resolver).ok()?;
    let empty = Pv::Map(Default::default());
    let or_empty = |v: &Pv| if v.truthy() { v.clone() } else { empty.clone() };
    let mut matched = Vec::new();
    for row in props.at("DamageTypes").items() {
        let value = data
            .attribute_value(&or_empty(row.at("ValueIfMatched")), &[])
            .ok()??;
        matched.push((
            row.at("AssociatedDamageTypeName")
                .as_str()
                .map(str::to_string),
            value,
        ));
    }
    let default = data
        .attribute_value(&or_empty(props.at("ValueIfNotMatched")), &[])
        .ok()??;
    Some((matched, default))
}

/// One operand of the subset: the manufacturer name a `Weapon_Is_<Maker>` attribute names, or a damage selector.
enum Operand {
    Maker(String),
    Damage(Vec<(Option<String>, f64)>, f64),
}

fn name_operand(data: &RollData, attribute: &str) -> Option<Operand> {
    if let Some(at) = attribute.rfind("Weapon_Is_")
        && attribute.contains(".Weapon_Is_")
    {
        return Some(Operand::Maker(
            attribute[at + "Weapon_Is_".len()..].to_string(),
        ));
    }
    if attribute.is_empty() {
        return None;
    }
    damage_type_selector(data, attribute).map(|(m, d)| Operand::Damage(m, d))
}

fn operand_value(operand: &Operand, cx: &NameContext<'_>) -> f64 {
    match operand {
        Operand::Maker(name) => {
            let own = cx
                .manufacturer
                .and_then(|m| m.rsplit_once('.'))
                .map(|(_, n)| n);
            if own == Some(name.as_str()) { 1.0 } else { 0.0 }
        }
        Operand::Damage(matched, default) => {
            let name = cx.damage_type.map(|d| d.rsplit('.').next().unwrap_or(d));
            matched
                .iter()
                .find(|(key, _)| key.as_deref() == name)
                .map_or(*default, |(_, v)| *v)
        }
    }
}

/// `weapon_recipe.name_expressions_hold`: every expression holds; one the subset cannot evaluate excludes the name
/// part (module documentation).
pub fn tool_expressions_hold(data: &RollData, expressions: &Pv, cx: &NameContext<'_>) -> bool {
    for e in expressions.items() {
        let op = e.at("ComparisonOperator");
        let op = if op.truthy() {
            op.as_str().unwrap_or("")
        } else {
            "OPERATOR_EqualTo"
        };
        let compare: fn(f64, f64) -> bool = match op {
            "OPERATOR_EqualTo" => |a, b| a == b,
            "OPERATOR_NotEqualTo" => |a, b| a != b,
            "OPERATOR_LessThan" => |a, b| a < b,
            "OPERATOR_LessThanOrEqualTo" => |a, b| a <= b,
            "OPERATOR_GreaterThan" => |a, b| a > b,
            "OPERATOR_GreaterThanOrEqualTo" => |a, b| a >= b,
            _ => return false,
        };
        if e.at("AttributeOperand2").truthy() {
            return false;
        }
        let attribute = e.at("AttributeOperand1").as_str().unwrap_or("");
        let Some(operand) = name_operand(data, attribute) else {
            return false;
        };
        let constant = f32r(e.at("ConstantOperand2").number().unwrap_or(0.0));
        if !compare(operand_value(&operand, cx), constant) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maker_operand_compares_object_names() {
        let cx = NameContext::new(Some(5), Some("Test.Makers.Acme"), None);
        assert_eq!(operand_value(&Operand::Maker("Acme".into()), &cx), 1.0);
        assert_eq!(operand_value(&Operand::Maker("Other".into()), &cx), 0.0);
        let none = NameContext::new(Some(5), None, None);
        assert_eq!(operand_value(&Operand::Maker("Acme".into()), &none), 0.0);
    }

    #[test]
    fn the_damage_operand_falls_back_to_the_not_matched_value() {
        let op = Operand::Damage(vec![(Some("DmgType_Fire".into()), 1.0)], 0.0);
        let fire = NameContext::new(None, None, Some("Test.Damage.DmgType_Fire"));
        assert_eq!(operand_value(&op, &fire), 1.0);
        // An item has no damage type here: the not-matched value.
        assert_eq!(operand_value(&op, &NameContext::new(None, None, None)), 0.0);
    }
}
