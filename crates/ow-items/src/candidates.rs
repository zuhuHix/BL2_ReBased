//! Owner: lane W4-ROLL. Manufacturer and grade candidates of a balance, and the part candidates of one roll.
//!
//! **Manufacturer and grade.** Two forms, chosen by the parity mode:
//!
//! - [`manufacturer_options`] is `item_roll.py` step 1, a **Reference-mode stand-in** (architecture section 6.1): per
//!   manufacturer entry of the registry's leaf-most list, the first grade whose window holds the stage, weighted by
//!   the spawn modifier interpolated across the window in single precision; a grade weighing at most 1e-8 is no
//!   option. The roll then draws among the options itself (one option: no draw).
//! - [`expand`] is the game's place for that choice (NATIVE_LOOT.md, "Balance candidate expansion", read from native
//!   code, UNVERIFIED in game): the pool roll expands a balance into **one candidate per manufacturer entry** with a
//!   covering grade, at `s = min(stage, cap)`, walking the base chain from the balance itself and stopping at the
//!   first step that yields a candidate. Each candidate carries its definition
//!   (`GetInventoryDefinitionForManufacturerGrade`), its grade-or-level word and its weight. The pool then draws
//!   among every candidate of its entries ([`pick_candidate`] for a balance rolled on its own: always one draw,
//!   even for one candidate, which is why Off mode's draw order differs from the oracle's).
//!
//! **Parts** ([`part_candidates`]): `item_registry.candidates_at` over a variant's merged slots, with the stage
//! window applied as `weapon_recipe.slot_candidates` applies it (`stage < trunc(min)` or `trunc(max) < stage` drops
//! the entry), the entry weight (flat 100 with no manufacturer list, else the manufacturer's entry, else the
//! default), entries at or below 1e-8 dropped and a repeated part keeping its first position with its later weight
//! (an `OrderedMap`). How a value that needs runtime state is read is the caller's: under Reference the oracle's
//! stand-in (a window bound as no bound, a weight as `None`, taken as 1 by the roll); in Off mode through the
//! `ow-attr` evaluator (`roll.rs`). The weapon merge was checked in game for part identity; the item merge, weights
//! and windows are UNVERIFIED (ITEM_REGISTRY.md).
//!
//! **The relic hook** ([`apply_relic_hook`], NATIVE_ITEM_RULES.md section 1, read from native code, UNVERIFIED in
//! game): when the definition sets `bAllowInventoryDefToModifyPartWeight` (class default true for
//! `ArtifactDefinition` only), a part with slot upgrades none of which names a slot of the definition's
//! `AttributeSlotEffects` weighs 0. A slot name that is empty or `None` names no slot.

use ow_core::OrderedMap;
use ow_core::num::f32r;
use ow_core::rand::Rand15;

use crate::merge::{RollData, short};
use crate::pick::{MIN_WEIGHT, Picked, pick};
use crate::registry::{FLAT_WEIGHT, ITEM_SLOTS, Pv, WEAPON_SLOTS};
use crate::roll::RollError;

/// A value reader: a registry value (a number, `{"init": ...}`, or `None`) to a number, `None` when the value has
/// none (the stand-in's "needs runtime state", or no value at all).
pub type ValueOf<'a> = dyn FnMut(&Pv) -> Result<Option<f64>, RollError> + 'a;

// ---- the oracle's balance-level draw (Reference) ---------------------------------------------------------------------

/// One manufacturer option of [`manufacturer_options`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct GradeOption {
    pub manufacturer: String,
    /// The grade's index in the manufacturer's list.
    pub grade: usize,
    pub weight: f64,
}

/// `(grade['min_stage'] or 0, grade['max_stage'] or 0)`: a missing bound reads as 0, as the tool reads it.
fn window(grade: &Pv) -> (f64, f64) {
    let bound = |k: &str| grade.at(k).number().unwrap_or(0.0);
    (bound("min_stage"), bound("max_stage"))
}

/// Whether the grade's window holds `stage` (inclusive both ends).
pub fn window_holds(grade: &Pv, stage: i64) -> bool {
    let (low, high) = window(grade);
    let s = stage as f64;
    low <= s && s <= high
}

/// `item_roll.grade_weight`: the spawn modifier interpolated across the grade's window, single precision; a
/// modifier that is not a number reads as 1 (min) or as the min (max), as the tool reads it.
pub fn grade_weight(grade: &Pv, stage: i64) -> f64 {
    let (low, high) = window(grade);
    let spawn = grade.at("spawn_probability");
    let as_number = |v: &Pv| match v {
        Pv::Int(_) | Pv::Float(_) => v.number(),
        _ => None,
    };
    let first = as_number(spawn.at("min")).unwrap_or(1.0);
    let last = as_number(spawn.at("max")).unwrap_or(first);
    let span = high - low;
    if span > 0.0 {
        f32r(first + (stage as f64 - low) / span * (last - first))
    } else {
        f32r(first)
    }
}

/// `item_roll.manufacturer_options` (module documentation). Reference mode only.
pub fn manufacturer_options(record: &Pv, stage: i64) -> Vec<GradeOption> {
    let mut options = Vec::new();
    for entry in record.at("manufacturers").items() {
        let Some(maker) = entry.at("manufacturer").as_str().filter(|m| !m.is_empty()) else {
            continue;
        };
        for (index, grade) in entry.at("grades").items().iter().enumerate() {
            if window_holds(grade, stage) {
                let weight = grade_weight(grade, stage);
                if weight > MIN_WEIGHT {
                    options.push(GradeOption {
                        manufacturer: maker.to_string(),
                        grade: index,
                        weight,
                    });
                }
                break;
            }
        }
    }
    options
}

// ---- the game's pool-level expansion (Off mode) ----------------------------------------------------------------------

/// One balance candidate of the pool roll (NATIVE_LOOT.md, "Balance candidate expansion").
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PoolCandidate {
    /// The chain step whose manufacturer list gave the candidate.
    pub step: String,
    pub manufacturer: String,
    /// The grade's index in that step's list.
    pub grade: usize,
    /// `GetInventoryDefinitionForManufacturerGrade` on the original balance.
    pub definition: String,
    /// The grade-or-level word `InitializeInventory` receives: `s` when the original balance interpolates its level,
    /// else the grade index.
    pub grade_or_level: i32,
    /// The stage stored on the item, `s = min(stage, cap)`.
    pub stage: i32,
    /// `GetExpLevelFromManufacturerData`.
    pub level: i32,
    /// The grade's spawn modifier at `s`, single precision.
    pub weight: f32,
}

/// A grade read from a step's own `Manufacturers` row, in the registry's shape (`item_registry.grades_of`) with the
/// spawn modifiers left as `{"init": ...}`.
fn grades_of(row: &Pv) -> Vec<Pv> {
    let mut out = Vec::new();
    for grade in row.at("Grades").items() {
        let need = grade.at("GameStageRequirement");
        let modifiers = grade.at("GradeModifiers");
        let init = |field: &str| {
            let v = grade.at(field);
            if v.truthy() {
                map(vec![("init", v.clone())])
            } else {
                Pv::Null
            }
        };
        out.push(map(vec![
            ("min_stage", need.at("MinGameStage").clone()),
            ("max_stage", need.at("MaxGameStage").clone()),
            ("exp_level", modifiers.at("ExpLevel").clone()),
            (
                "definition",
                modifiers.at("CustomInventoryDefinition").clone(),
            ),
            (
                "spawn_probability",
                map(vec![
                    ("min", init("MinSpawnProbabilityModifier")),
                    ("max", init("MaxSpawnProbabilityModifier")),
                ]),
            ),
        ]));
    }
    out
}

fn map(entries: Vec<(&str, Pv)>) -> Pv {
    Pv::Map(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

/// `bInterpolateExpLevel` of a balance step (class default true, NATIVE_LOOT.md; untagged reads as the default).
pub fn interpolates(data: &RollData, step: &str) -> bool {
    match data.get(step).map(|p| p.at("bInterpolateExpLevel").clone()) {
        None | Some(Pv::Null) => true,
        Some(v) => v.truthy(),
    }
}

/// One step's manufacturer entries `(manufacturer, grades)` in the registry's shape: the registry's own list for
/// the step it came from (`manufacturers_from`), the step's own `Manufacturers` property otherwise.
fn step_makers(data: &RollData, record: &Pv, step: &str) -> Vec<(Option<String>, Vec<Pv>)> {
    if record.at("manufacturers_from").as_str() == Some(step) {
        return record
            .at("manufacturers")
            .items()
            .iter()
            .map(|m| {
                (
                    m.at("manufacturer").as_str().map(str::to_string),
                    m.at("grades").items().to_vec(),
                )
            })
            .collect();
    }
    let Some(props) = data.get(step) else {
        return Vec::new();
    };
    props
        .at("Manufacturers")
        .items()
        .iter()
        .map(|row| {
            (
                row.at("Manufacturer").as_str().map(str::to_string),
                grades_of(row),
            )
        })
        .collect()
}

/// `GetInventoryDefinitionForManufacturerGrade` (NATIVE_LOOT.md), over the chain from `from` (an index into
/// `chain`): the step's own entry for the manufacturer, else the base's answer. The `InventoryClass` test is not
/// applied: it needs the definition's class default, which a tag stream does not carry (UNVERIFIED gap, open until
/// the roll reads heap objects).
fn definition_for(
    data: &RollData,
    record: &Pv,
    chain: &[String],
    from: usize,
    maker: &str,
    word: i32,
) -> Option<String> {
    let step = chain.get(from)?;
    let own_definition = data
        .get(step)
        .and_then(|p| p.at("InventoryDefinition").as_str().map(str::to_string));
    let makers = step_makers(data, record, step);
    let answer = match makers.iter().find(|(m, _)| m.as_deref() == Some(maker)) {
        None => None,
        Some(_) if interpolates(data, step) => own_definition,
        Some((_, grades)) => {
            let index = (word & 0xFFFF) as usize;
            match grades.get(index) {
                None => None,
                Some(g) => g
                    .at("definition")
                    .as_str()
                    .map(str::to_string)
                    .or(own_definition),
            }
        }
    };
    answer.or_else(|| definition_for(data, record, chain, from + 1, maker, word))
}

/// `GetExpLevelFromManufacturerData` (NATIVE_LOOT.md): the word for an interpolating balance; otherwise the grade's
/// `ExpLevel` of the first step listing the manufacturer with the grade in range (high 16 bits of the word zero), the
/// high 16 bits capped at 63 when set; 1 when nothing resolves.
fn level_for(data: &RollData, record: &Pv, chain: &[String], maker: &str, word: i32) -> i32 {
    if chain.first().is_some_and(|b| interpolates(data, b)) {
        return word;
    }
    for step in chain {
        let makers = step_makers(data, record, step);
        if let Some((_, grades)) = makers.iter().find(|(m, _)| m.as_deref() == Some(maker))
            && let Some(g) = grades.get((word & 0xFFFF) as usize)
        {
            let high = (word >> 16) & 0xFFFF;
            if high != 0 {
                return high.min(63);
            }
            return g.at("exp_level").number().map_or(1, |v| v as i32);
        }
    }
    1
}

/// The balance candidate expansion (module documentation) at `stage` capped by `cap`. `value` reads the spawn
/// modifiers (Off mode: through the evaluator). The chain is the registry record's (`chain`, leaf first).
pub fn expand(
    data: &RollData,
    record: &Pv,
    stage: i32,
    cap: i32,
    value: &mut ValueOf<'_>,
) -> Result<Vec<PoolCandidate>, RollError> {
    let s = stage.min(cap);
    let chain: Vec<String> = record
        .at("chain")
        .items()
        .iter()
        .filter_map(|p| p.as_str().map(str::to_string))
        .collect();
    let original_interpolates = chain.first().is_some_and(|b| interpolates(data, b));
    for step in &chain {
        let mut found = Vec::new();
        for (maker, grades) in step_makers(data, record, step) {
            let Some(maker) = maker.filter(|m| !m.is_empty()) else {
                continue;
            };
            let Some((index, grade)) = grades
                .iter()
                .enumerate()
                .find(|(_, g)| window_holds(g, i64::from(s)))
            else {
                continue;
            };
            let word = if original_interpolates {
                s
            } else {
                index as i32
            };
            let Some(definition) = definition_for(data, record, &chain, 0, &maker, word) else {
                continue;
            };
            let spawn = grade.at("spawn_probability");
            let min_mod = value(spawn.at("min"))?.unwrap_or(1.0) as f32;
            let max_mod = match value(spawn.at("max"))? {
                Some(v) => v as f32,
                None => min_mod,
            };
            let (low, high) = window(grade);
            let span = (high - low) as f32;
            let weight = if span > 0.0 {
                min_mod + (s as f32 - low as f32) / span * (max_mod - min_mod)
            } else {
                min_mod
            };
            found.push(PoolCandidate {
                step: step.clone(),
                manufacturer: maker.clone(),
                grade: index,
                definition,
                grade_or_level: word,
                stage: s,
                level: level_for(data, record, &chain, &maker, word),
                weight,
            });
        }
        if !found.is_empty() {
            return Ok(found);
        }
    }
    Ok(Vec::new())
}

/// A balance rolled on its own, as a pool with that one balance entry would draw it (NATIVE_LOOT.md, pool roll steps
/// 3 and 4): each candidate weighs its modifier times the entry weight 1 over the number of candidates, in single
/// precision; a candidate at or below 1e-8 is left out; then one weighted draw, also for a single candidate. The draw
/// is [`pick`]'s form (double), the form `loot_pools.roll_crt` uses; the game's own form is UNVERIFIED (W4-POOLS
/// owns the pool roll).
pub fn pick_candidate(candidates: &[PoolCandidate], rng: &mut dyn Rand15) -> Picked<usize> {
    let n = candidates.len().max(1) as f32;
    pick(
        candidates
            .iter()
            .enumerate()
            .map(|(i, c)| (i, Some(f64::from(c.weight * 1.0 / n)))),
        rng,
    )
}

// ---- parts -------------------------------------------------------------------------------------------------------

/// One slot's candidates: `{part: weight}` (a `None` weight needs runtime state), present when at least one entry
/// of the slot passed its stage window, even if none weighs above the floor (`weapon_recipe.roll` notes such a slot).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlotCandidates {
    pub parts: OrderedMap<String, Option<f64>>,
    /// Some entry that passed its window had no weight (`weapon_recipe.roll`'s note is per entry, before the
    /// repeats merge).
    pub runtime_weight: bool,
}

/// The slots in the tool's slot order.
pub type PartCandidates = OrderedMap<String, SlotCandidates>;

/// The tool's slot dict order: the balance's own slots, then the weapon slots a weapon part list added.
pub fn slot_order(weapon_balance: bool) -> Vec<&'static str> {
    let own: &[&'static str] = if weapon_balance {
        &WEAPON_SLOTS
    } else {
        &ITEM_SLOTS
    };
    let mut out: Vec<&'static str> = Vec::new();
    for slot in own.iter().chain(WEAPON_SLOTS.iter()) {
        if !out.contains(slot) {
            out.push(slot);
        }
    }
    out
}

/// `item_registry.candidates_at` over a variant's `slots` (module documentation), with `value` reading every
/// stage bound and weight. Unlike the tool's dict, a slot whose window-passing entries all weigh at or below the
/// floor is kept with no parts (see [`SlotCandidates`]); a slot with no entry in its window is left out.
pub fn part_candidates(
    slots: &Pv,
    weapon_balance: bool,
    stage: i64,
    manufacturer: Option<&str>,
    value: &mut ValueOf<'_>,
) -> Result<PartCandidates, RollError> {
    let mut out = PartCandidates::new();
    let s = stage as f64;
    for slot in slot_order(weapon_balance) {
        let Some(entries) = slots.get(slot) else {
            continue;
        };
        let mut live = SlotCandidates::default();
        let mut passed = false;
        for c in entries.items() {
            let low = value(c.at("min_stage"))?;
            let high = value(c.at("max_stage"))?;
            if low.is_some_and(|l| s < l.trunc()) || high.is_some_and(|h| h.trunc() < s) {
                continue;
            }
            passed = true;
            let weight = c.at("weight");
            let w = match manufacturer.filter(|m| !m.is_empty()) {
                Some(maker) if weight.at("source").as_str() != Some("flat") => {
                    match weight.at("by_manufacturer").get(maker) {
                        Some(v) => value(v)?,
                        None => value(weight.at("default"))?,
                    }
                }
                _ => Some(FLAT_WEIGHT),
            };
            live.runtime_weight |= w.is_none();
            if w.is_none_or(|w| w > MIN_WEIGHT) {
                let part = match c.at("part") {
                    Pv::Str(p) => p.clone(),
                    other => other.to_json(),
                };
                live.parts.insert(part, w);
            }
        }
        if passed {
            out.insert(slot.to_string(), live);
        }
    }
    Ok(out)
}

/// Definition classes whose class default sets `bAllowInventoryDefToModifyPartWeight` (NATIVE_ITEM_RULES.md
/// section 1).
const HOOK_CLASS_DEFAULT: [&str; 1] = ["ArtifactDefinition"];

/// Slot names of `AttributeSlotUpgrades` / `AttributeSlotEffects` rows; an empty name or the name `None` is no slot.
pub fn slot_names(rows: &Pv) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for row in rows.items() {
        if let Some(name) = row.at("SlotName").as_str()
            && !name.is_empty()
            && name != "None"
            && !out.iter().any(|n| n == name)
        {
            out.push(name.to_string());
        }
    }
    out
}

/// Whether the hook applies to `definition`: its own flag when tagged, else its class's default (the registry's
/// class for the definition).
pub fn hook_applies(registry_tree: &Pv, data: &RollData, definition: &str) -> bool {
    let class = registry_tree
        .at("definitions")
        .at(definition)
        .at("class")
        .as_str()
        .map(|c| short(c).to_string())
        .unwrap_or_default();
    match data
        .get(definition)
        .map(|p| p.at("bAllowInventoryDefToModifyPartWeight").clone())
    {
        None | Some(Pv::Null) => HOOK_CLASS_DEFAULT.contains(&class.as_str()),
        Some(flag) => flag.truthy(),
    }
}

/// The relic answer: a part with no slot upgrades, or one naming a slot the definition defines, keeps its weight.
pub fn hook_keeps(data: &RollData, defined: &[String], part: &str) -> bool {
    let upgrades = data
        .get(part)
        .map(|p| slot_names(p.at("AttributeSlotUpgrades")))
        .unwrap_or_default();
    upgrades.is_empty() || upgrades.iter().any(|u| defined.contains(u))
}

/// The hook on `found` when it applies (module documentation): the removed `(slot, part)` pairs. A slot left with
/// no candidate is removed.
pub fn apply_relic_hook(
    registry_tree: &Pv,
    data: &RollData,
    definition: &str,
    found: &mut PartCandidates,
) -> Vec<(String, String)> {
    let mut removed = Vec::new();
    if !hook_applies(registry_tree, data, definition) {
        return removed;
    }
    let defined = data
        .get(definition)
        .map(|p| slot_names(p.at("AttributeSlotEffects")))
        .unwrap_or_default();
    let slots: Vec<String> = found.keys().cloned().collect();
    for slot in slots {
        let Some(entry) = found.get(slot.as_str()) else {
            continue;
        };
        let mut keep = OrderedMap::new();
        for (part, weight) in entry.parts.iter() {
            if hook_keeps(data, &defined, part) {
                keep.insert(part.clone(), *weight);
            } else {
                removed.push((slot.clone(), part.clone()));
            }
        }
        if keep.is_empty() {
            found.shift_remove(slot.as_str());
        } else if let Some(entry) = found.get_mut(slot.as_str()) {
            entry.parts = keep;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::tests::{map as m, s};
    use crate::registry::CensusInput;
    use std::path::PathBuf;

    fn grade(low: i64, high: i64, min: Option<f64>, max: Option<f64>) -> Pv {
        let f = |v: Option<f64>| v.map_or(Pv::Null, Pv::Float);
        m(vec![
            ("min_stage", Pv::Int(low)),
            ("max_stage", Pv::Int(high)),
            ("exp_level", Pv::Int(3)),
            ("definition", Pv::Null),
            (
                "spawn_probability",
                m(vec![("min", f(min)), ("max", f(max))]),
            ),
        ])
    }

    fn record(makers: Vec<(&str, Vec<Pv>)>) -> Pv {
        m(vec![
            ("class", s("InventoryBalanceDefinition")),
            ("chain", Pv::List(vec![s("Test.Bal")])),
            ("manufacturers_from", s("Test.Bal")),
            ("inventory_definition", s("Test.Def")),
            (
                "manufacturers",
                Pv::List(
                    makers
                        .into_iter()
                        .map(|(n, g)| m(vec![("manufacturer", s(n)), ("grades", Pv::List(g))]))
                        .collect(),
                ),
            ),
        ])
    }

    fn numbers(v: &Pv) -> Result<Option<f64>, RollError> {
        Ok(v.number().filter(|_| !matches!(v, Pv::Null)))
    }

    #[test]
    fn the_oracles_options_take_the_first_covering_grade_with_a_positive_weight() {
        let r = record(vec![
            // The first grade holding stage 8 weighs 0: no option, and the later grade is not tried.
            (
                "Test.A",
                vec![grade(1, 10, Some(0.0), None), grade(1, 100, None, None)],
            ),
            // 0.5 at stage 1 rising to 1 at stage 11: 0.5 + 7/10 * 0.5 = 0.85 at stage 8, in single precision.
            (
                "Test.B",
                vec![
                    grade(20, 30, None, None),
                    grade(1, 11, Some(0.5), Some(1.0)),
                ],
            ),
            ("Test.C", vec![grade(9, 100, None, None)]),
        ]);
        let options = manufacturer_options(&r, 8);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].manufacturer, "Test.B");
        assert_eq!(options[0].grade, 1);
        assert_eq!(options[0].weight, f64::from(0.85f32));
    }

    #[test]
    fn off_mode_needs_the_balances_own_definition() {
        // GetInventoryDefinitionForManufacturerGrade answers from the balance's own InventoryDefinition (NATIVE_LOOT.md):
        // with no package data the balance has none, so the manufacturer yields no candidate. The expansion with
        // package data is tested in roll.rs (`off_mode_expands_at_the_pool_level_and_draws_once_more`).
        let data = RollData::new(&CensusInput::new(PathBuf::from(".")));
        let r = record(vec![("Test.A", vec![grade(1, 100, None, None)])]);
        assert!(expand(&data, &r, 8, 50, &mut numbers).unwrap().is_empty());
    }

    fn candidate(manufacturer: &str, weight: f32) -> PoolCandidate {
        PoolCandidate {
            step: "Test.Bal".to_string(),
            manufacturer: manufacturer.to_string(),
            grade: 0,
            definition: "Test.Def".to_string(),
            grade_or_level: 8,
            stage: 8,
            level: 8,
            weight,
        }
    }

    #[test]
    fn a_balance_on_its_own_draws_once_even_for_one_candidate() {
        let mut rng = ow_core::rand::CrtRand::new(1);
        let picked = pick_candidate(&[candidate("Test.B", 0.5)], &mut rng);
        assert_eq!(picked.choice, Some(0));
        assert_ne!(rng, ow_core::rand::CrtRand::new(1));
        // A candidate at or below the floor takes no share; each weighs its modifier over the candidate count.
        let both = [candidate("Test.A", 0.0), candidate("Test.B", 0.5)];
        let picked = pick_candidate(&both, &mut ow_core::rand::CrtRand::new(1));
        assert_eq!(picked.choice, Some(1));
        assert_eq!(picked.candidates, vec![(1, f64::from(0.5f32 / 2.0))]);
        // Nothing above the floor: no draw.
        let mut rng = ow_core::rand::CrtRand::new(1);
        assert_eq!(
            pick_candidate(&[candidate("Test.A", 0.0)], &mut rng).choice,
            None
        );
        assert_eq!(rng, ow_core::rand::CrtRand::new(1));
    }

    #[test]
    fn part_windows_weights_and_repeats_follow_the_tool() {
        let entry = |part: &str, low: Pv, high: Pv, weight: Pv| {
            m(vec![
                ("part", s(part)),
                ("min_stage", low),
                ("max_stage", high),
                ("weight", weight),
            ])
        };
        let flat = m(vec![("source", s("flat")), ("value", Pv::Float(100.0))]);
        let by = m(vec![
            ("source", s("manufacturer")),
            ("by_manufacturer", m(vec![("Test.A", Pv::Float(5.0))])),
            ("default", Pv::Float(0.0)),
        ]);
        let runtime = m(vec![(
            "init",
            m(vec![("BaseValueAttribute", s("Test.Attr"))]),
        )]);
        let slots = m(vec![(
            "Alpha",
            Pv::List(vec![
                entry("Test.P1", Pv::Float(1.0), Pv::Float(10.9), flat.clone()),
                entry("Test.P2", Pv::Float(9.0), Pv::Null, flat.clone()),
                entry("Test.P3", Pv::Null, Pv::Null, by.clone()),
                entry("Test.P4", runtime.clone(), Pv::Null, flat.clone()),
                entry("Test.P1", Pv::Null, Pv::Null, by.clone()),
            ]),
        )]);
        // The stand-in reading: a value needing runtime state has none (no bound).
        let mut stand_in = |v: &Pv| -> Result<Option<f64>, RollError> {
            Ok(match v {
                Pv::Int(_) | Pv::Float(_) => v.number(),
                _ => None,
            })
        };
        let found = part_candidates(&slots, false, 10, Some("Test.A"), &mut stand_in).unwrap();
        let alpha = &found.get("Alpha").unwrap().parts;
        // P1 holds stage 10 (trunc 10.9 = 10), then its repeat takes the manufacturer weight 5 at P1's position;
        // P2 starts at 9; P3 weighs 5 for Test.A; P4's runtime bound is no bound.
        let got: Vec<(&str, Option<f64>)> = alpha.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        assert_eq!(
            got,
            [
                ("Test.P1", Some(5.0)),
                ("Test.P2", Some(100.0)),
                ("Test.P3", Some(5.0)),
                ("Test.P4", Some(100.0))
            ]
        );
        // Another manufacturer takes the default 0, so P3 is dropped and P1's repeat does not remove it.
        let found = part_candidates(&slots, false, 10, Some("Test.Z"), &mut stand_in).unwrap();
        let alpha = &found.get("Alpha").unwrap().parts;
        assert_eq!(alpha.get("Test.P1"), Some(&Some(100.0)));
        assert!(!alpha.contains_key("Test.P3"));
        // At stage 11 the first P1 entry is out of its window, so P1 takes the position of its repeat (no
        // manufacturer given: every entry weighs the flat 100).
        let found = part_candidates(&slots, false, 11, None, &mut stand_in).unwrap();
        let keys: Vec<&str> = found
            .get("Alpha")
            .unwrap()
            .parts
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["Test.P2", "Test.P3", "Test.P4", "Test.P1"]);
    }

    #[test]
    fn slot_names_skip_empty_and_none() {
        let rows = Pv::List(vec![
            m(vec![("SlotName", s("Effect1"))]),
            m(vec![("SlotName", s("None"))]),
            m(vec![("SlotName", s(""))]),
            m(vec![]),
            m(vec![("SlotName", s("Effect1"))]),
        ]);
        assert_eq!(slot_names(&rows), vec!["Effect1".to_string()]);
    }
}
