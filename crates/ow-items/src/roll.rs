//! Owner: lane W4-ROLL. One roll of one balance: manufacturer and grade, definition, parts, level, name parts,
//! rarity and the part value total, drawn from **one `CrtRand(seed)` per roll** through [`RollWorld`] (architecture
//! section 3.7): the roll's own draws and every evaluator draw come from that one generator, as the oracles draw one
//! `CrtRand(seed)` stream for the whole roll. That the game's rolls use the C runtime `rand()` this way is UNVERIFIED;
//! the seeding is the tools'.
//!
//! **Two modes** (rule R3, the mode is a value in [`RollCx`]):
//!
//! - [`ParityMode::Reference`] reproduces the oracles, stand-ins included, so the census (`tools/item_roll.py`'s
//!   `check`) and the weapon comparison (`weapon_recipe.roll`) can run on this output: shields, grenade mods, class
//!   mods and relics as `item_roll.roll`, weapons as `weapon_recipe.roll`. The stand-ins (architecture section 6.1):
//!   the manufacturer and grade drawn in the balance roll ([`crate::candidates::manufacturer_options`]); the uniform
//!   class-mod definition pick; a part weight that needs runtime state taken as 1 and a window bound that needs it
//!   taken as no bound (the oracle's `weapon_recipe.attribute_value` subset). Weapons take the root-most manufacturer
//!   and weapon type and draw no manufacturer, as `weapon_recipe.merge` does.
//! - [`ParityMode::Off`] is the game's place for each choice, as the records read it (UNVERIFIED in game): the
//!   pool-level candidate expansion, one candidate per manufacturer at `min(stage, cap)` with the cap of
//!   `ow_balance::cap::item_stage_cap` ([`crate::candidates::expand`]); a value that needs runtime state (a window
//!   bound, a weight, a spawn modifier, a rarity term) evaluated through the `ow-attr` evaluator, with the item as the
//!   source when the caller gives one (the part chooser runs on the item, NATIVE_LOOT.md `ChooseRandomParts`; the
//!   context is UNVERIFIED); name expressions through the evaluator; and **class mods answer `NotYet {
//!   NeedsRealGame(6) }`**: the game's `ClassModBalanceDefinition` override was not read. After W7 deletes Reference,
//!   class-mod rolls return `NotYet` and the census no longer covers class mods until question 6 is answered.
//!
//! **Question 7, the grenade-mod per-element minimum.** 1,035 grenade-mod part entries bound their stage window by
//! an attribute whose only value resolver is a constant resolver (a per-element minimum game stage,
//! ITEM_REGISTRY.md section 4). Which reading the game uses is not settled (real-game question 7). Reference reads
//! such a bound as no bound, as `prepare_vendors`, `prepare_stock_shield` and `item_roll.py` do. Off mode evaluates it
//! through the evaluator, which gives the resolver's constant, so the bound applies: the evaluator's reading,
//! UNVERIFIED (question 7).
//!
//! **Level** = the stage for an interpolating balance (`bInterpolateExpLevel`, class default true; level equals stage
//! is verified for 1,600 game-rolled guns, WEAPON_PARTS_CATALOG.md section 4), else the grade's `ExpLevel`
//! (`GetExpLevelFromManufacturerData`, read from native code, UNVERIFIED). Reference applies no stage cap, as both
//! oracles do by default.
//!
//! **Off mode and the item's instance** (architecture section 3.7): a caller that has the item's instance object
//! runs [`crate::candidates::expand`], draws a candidate ([`crate::candidates::pick_candidate`] or its pool's draw),
//! instantiates the item, then calls [`roll_candidate`] with [`RollCx::with_item`]. [`roll`] in Off mode without an
//! instance answers `NotYet` for the parts that need one (name expressions, context-dependent rarity terms).

use ow_attr::AttributeSystem;
use ow_attr::api::{
    AttrError, AttrWorld, Comparison, EngineQuery, EvalCx, ExprData, ExprMode, GlobalSlots,
    InitData, NumForms, Operand2Usage, WorldParts,
};
use ow_balance::types::BalanceSnapshot;
use ow_core::rand::{CrtRand, CrtStream, Rand15};
use ow_core::{DetMap, NotYet, ObjectPath, OrderedMap, ParityMode};
use ow_object::ids::{ObjRef, PropId};

use crate::ItemError;
use crate::candidates::{
    PartCandidates, PoolCandidate, apply_relic_hook, expand, interpolates, manufacturer_options,
    part_candidates, pick_candidate,
};
use crate::data::{DefinitionData, ItemKind, ItemSlot, SLOT_COUNT, WeaponSlot};
use crate::merge::{self, RollData};
use crate::names::{self, NameContext, choose, native_damage_type, tool_expressions_hold};
use crate::pick::{Picked, pick};
use crate::rarity::{self, Rarity, trunc_i64};
use crate::registry::{ITEM_SLOTS, Pv, Registry, WEAPON_SLOTS};
use crate::value::part_value_total;

/// The record a `NotYet` of the roll names.
pub const RECORD: &str = "architecture section 3.7 (ow-items roll)";

/// The kinds `item_roll.py` rolls.
pub const ITEM_KINDS: [&str; 4] = ["shield", "grenade_mod", "class_mod", "relic"];

// ---- the roll's world --------------------------------------------------------------------------------------------

/// The roll's world (architecture section 3.7): `rng` is the roll's one generator for every stream, the evaluator's
/// included; everything else is the inner world's, except `attribute_changed`, which is forwarded only when the inner
/// world runs no script (an offline world). Over a world that may run script (the Vm) it answers `NotYet {
/// NotPorted }`, so a nested evaluation can never draw from the inner world's generators (the adapter rule of 3.5).
pub struct RollWorld<'a> {
    pub inner: &'a mut dyn AttrWorld,
    pub rng: &'a mut CrtRand,
    pub inner_runs_script: bool,
}

impl<'a> RollWorld<'a> {
    pub fn new(
        inner: &'a mut dyn AttrWorld,
        rng: &'a mut CrtRand,
        inner_runs_script: bool,
    ) -> Self {
        RollWorld {
            inner,
            rng,
            inner_runs_script,
        }
    }
}

impl AttrWorld for RollWorld<'_> {
    fn parts(&mut self) -> WorldParts<'_> {
        self.inner.parts()
    }

    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        self.inner.globals()
    }

    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        self.inner.game_info()
    }

    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        self.inner.engine_query(q, source)
    }

    fn global_slots(&mut self) -> &mut GlobalSlots {
        self.inner.global_slots()
    }

    /// The inner world's clock: a read, which draws nothing (decision 2026-11-16-80). `currency_on_hand` is not
    /// forwarded: it runs script, which could draw from the inner world's generators (the trait default answers).
    fn time_seconds(&mut self) -> Result<Option<f32>, AttrError> {
        self.inner.time_seconds()
    }

    fn attribute_changed(
        &mut self,
        owner: ObjRef,
        value_prop: PropId,
        forms: NumForms,
    ) -> Result<(), AttrError> {
        if self.inner_runs_script {
            return Err(NotYet::not_ported(
                "a change notification during a roll over a world that runs script (it would draw from that world's \
                 generators, not the roll's one)",
                RECORD,
            )
            .into());
        }
        self.inner.attribute_changed(owner, value_prop, forms)
    }

    fn rng(&mut self, _s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Ok(self.rng)
    }
}

// ---- errors and results ----------------------------------------------------------------------------------------

/// The balance yields no item at this stage; `reason` is a short key (`item_roll.NotRollable`'s, plus
/// `no_candidate_at_stage` and `no_candidate_above_the_floor` of the Off-mode expansion).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotRollable {
    pub reason: &'static str,
    pub detail: String,
}

impl NotRollable {
    fn new(reason: &'static str, detail: impl Into<String>) -> Self {
        NotRollable {
            reason,
            detail: detail.into(),
        }
    }
}

/// One filled slot: the part, its weight and the candidates the pick drew among.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SlotPick {
    pub slot: String,
    pub part: String,
    pub weight: f64,
    pub candidates: Vec<(String, f64)>,
}

/// One rolled item.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Rolled {
    pub balance: String,
    /// The registry's kind (`weapon`, `shield`, `grenade_mod`, `class_mod`, `relic`).
    pub kind: String,
    pub mode: ParityMode,
    /// The stage the parts were drawn at (Off mode: capped).
    pub stage: i64,
    pub level: i64,
    /// `ManufacturerGradeIndex`: the level for an interpolating balance, else the grade index.
    pub grade_or_level: i32,
    pub manufacturer: Option<String>,
    pub grade: Option<usize>,
    pub definition: String,
    /// The filled slots in slot order.
    pub slots: Vec<SlotPick>,
    pub prefix: Option<String>,
    pub title: Option<String>,
    pub rarity: Rarity,
    /// Candidates the relic hook removed.
    pub hook_removed: usize,
    pub notes: Vec<String>,
    /// `crate::value::part_value_total`.
    pub value_total: Option<f32>,
}

/// What a roll gave.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum RollResult {
    Item(Box<Rolled>),
    NotRollable(NotRollable),
}

impl RollResult {
    /// The rolled item, if any.
    pub fn item(&self) -> Option<&Rolled> {
        match self {
            RollResult::Item(r) => Some(r),
            RollResult::NotRollable(_) => None,
        }
    }
}

/// What the roll needs besides the registry (architecture section 3.7): the balance state, the evaluator context
/// over a [`RollWorld`], the attribute system, the values the registry does not hold, and the mode.
#[non_exhaustive]
pub struct RollCx<'a> {
    pub balance: &'a BalanceSnapshot,
    /// `eval.world` is a [`RollWorld`].
    pub eval: EvalCx<'a>,
    pub sys: &'a AttributeSystem,
    pub(crate) data: &'a RollData,
    pub mode: ParityMode,
    /// The item's instance object, the source the Off-mode evaluations use (`None` offline until W4-STATS's
    /// `instantiate_stats` makes one).
    pub item: Option<ObjRef>,
    /// A manufacturer the caller chose (`item_roll.roll(manufacturer=...)`; in Off mode the pool's candidate): no
    /// manufacturer draw.
    pub manufacturer: Option<String>,
    /// Objects the caller already holds by path, tried before the world's store.
    pub known: DetMap<String, ObjRef>,
}

impl<'a> RollCx<'a> {
    pub fn new(
        balance: &'a BalanceSnapshot,
        eval: EvalCx<'a>,
        sys: &'a AttributeSystem,
        data: &'a RollData,
        mode: ParityMode,
    ) -> Self {
        RollCx {
            balance,
            eval,
            sys,
            data,
            mode,
            item: None,
            manufacturer: None,
            known: DetMap::new(),
        }
    }

    pub fn with_item(mut self, item: ObjRef) -> Self {
        self.item = Some(item);
        self
    }

    pub fn with_manufacturer(mut self, manufacturer: &str) -> Self {
        self.manufacturer = Some(manufacturer.to_string());
        self
    }

    pub fn with_known(mut self, path: &str, obj: ObjRef) -> Self {
        self.known.insert(path.to_string(), obj);
        self
    }

    fn rng(&mut self) -> Result<&mut dyn Rand15, ItemError> {
        Ok(self.eval.world.rng(CrtStream::ItemRoll)?)
    }

    /// The object at `path`: the caller's, else through the world's store.
    fn find(&mut self, path: &str) -> Result<ObjRef, ItemError> {
        if let Some(o) = self.known.get(path) {
            return Ok(*o);
        }
        let parsed =
            ObjectPath::parse(path).map_err(|e| ItemError::BadData(format!("{path}: {e}")))?;
        let parts = self.eval.world.parts();
        let space = parts.space;
        let Some(defs) = parts.defs else {
            return Err(NotYet::layer_absent(
                format!("finding {path} (the roll's world has no definition store)"),
                "ow-defs",
                RECORD,
            )
            .into());
        };
        match defs.find(space, &parsed).map_err(ItemError::from)? {
            Some(o) => Ok(o),
            None => Err(ItemError::BadData(format!("{path}: no package holds it"))),
        }
    }

    fn find_opt(&mut self, v: &Pv) -> Result<Option<ObjRef>, ItemError> {
        match v.as_str() {
            Some(p) if !p.is_empty() => self.find(p).map(Some),
            _ => Ok(None),
        }
    }

    /// An `AttributeInitializationData` written as the tag stream holds it, through the evaluator. An untagged
    /// `BaseValueScaleConstant` is the struct default 1 and an untagged constant 0, as the tools read them.
    fn evaluate_init(&mut self, init: &Pv) -> Result<f64, ItemError> {
        let data = InitData {
            base_value_constant: init.at("BaseValueConstant").number().unwrap_or(0.0) as f32,
            base_value_attribute: self.find_opt(init.at("BaseValueAttribute"))?,
            initialization_definition: self.find_opt(init.at("InitializationDefinition"))?,
            base_value_scale_constant: match init.at("BaseValueScaleConstant") {
                Pv::Null => 1.0,
                v => v.number().unwrap_or(1.0) as f32,
            },
        };
        let sys = self.sys;
        let source = self.item;
        Ok(f64::from(sys.evaluate_init(
            &data,
            source,
            None,
            &mut self.eval,
        )?))
    }

    /// Off mode's reading of a registry value: a number as it is, `{"init": ...}` through the evaluator, `None`
    /// for no value.
    fn off_value(&mut self, v: &Pv) -> Result<Option<f64>, ItemError> {
        match v {
            Pv::Null => Ok(None),
            Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => Ok(v.number()),
            other => match other.get("init") {
                Some(init) => {
                    let init = init.clone();
                    self.evaluate_init(&init).map(Some)
                }
                None => Err(ItemError::BadData(format!(
                    "a registry value {}",
                    other.to_json()
                ))),
            },
        }
    }

    /// Off mode's rarity term: the oracle's subset where it gives a value (the evaluator gives the same there),
    /// else the evaluator.
    fn off_term(&mut self, v: &Pv) -> Result<Option<f64>, ItemError> {
        if let Some(x) = self.data.number(v) {
            return Ok(Some(x));
        }
        self.evaluate_init(v).map(Some)
    }

    /// Off mode's name expressions: `EvaluateExpressions` (And) with the item as the source.
    fn off_holds(&mut self, expressions: &Pv) -> Result<bool, ItemError> {
        let rows = expressions.items();
        if rows.is_empty() {
            return Ok(true);
        }
        let Some(item) = self.item else {
            return Err(NotYet::not_ported(
                "name expressions offline: they evaluate with the item's instance as the source, which W4-STATS's \
                 instantiate_stats makes",
                RECORD,
            )
            .into());
        };
        let mut exprs = Vec::new();
        for row in rows {
            let op = match row.at("ComparisonOperator") {
                Pv::Null => Comparison::EqualTo,
                v => match v.as_str() {
                    Some("OPERATOR_EqualTo") => Comparison::EqualTo,
                    Some("OPERATOR_NotEqualTo") => Comparison::NotEqualTo,
                    Some("OPERATOR_LessThan") => Comparison::LessThan,
                    Some("OPERATOR_LessThanOrEqualTo") => Comparison::LessThanOrEqual,
                    Some("OPERATOR_GreaterThan") => Comparison::GreaterThan,
                    Some("OPERATOR_GreaterThanOrEqualTo") => Comparison::GreaterThanOrEqual,
                    _ => {
                        return Err(ItemError::BadData(format!(
                            "comparison operator {}",
                            v.to_json()
                        )));
                    }
                },
            };
            if !matches!(row.at("Operand2Usage"), Pv::Null) {
                return Err(NotYet::not_read(
                    "a tagged Operand2Usage in a name expression (its enum value names are not read)",
                    RECORD,
                )
                .into());
            }
            exprs.push(ExprData {
                attribute_operand1: self.find_opt(row.at("AttributeOperand1"))?,
                comparison_operator: op,
                operand2_usage: Operand2Usage::PreferAttribute,
                attribute_operand2: self.find_opt(row.at("AttributeOperand2"))?,
                constant_operand2: row.at("ConstantOperand2").number().unwrap_or(0.0) as f32,
            });
        }
        let sys = self.sys;
        Ok(sys.expressions_hold(&exprs, ExprMode::And, Some(item), None, &mut self.eval)?)
    }
}

// ---- the roll ------------------------------------------------------------------------------------------------------

/// An `i64` the item fields store as `i32`.
fn to_i32(v: i64, what: &str) -> Result<i32, ItemError> {
    i32::try_from(v).map_err(|_| ItemError::BadData(format!("{what} {v} is out of range")))
}

fn not_rollable(reason: &'static str, detail: impl Into<String>) -> Result<RollResult, ItemError> {
    Ok(RollResult::NotRollable(NotRollable::new(reason, detail)))
}

/// One roll of `balance` at the snapshot's stage (module documentation).
pub fn roll(
    reg: &Registry,
    balance: &ObjectPath,
    cx: &mut RollCx,
) -> Result<RollResult, ItemError> {
    let path = balance.as_str();
    let Some(record) = reg.balance(path) else {
        return not_rollable("unknown_balance", path);
    };
    let kind = record.at("kind").as_str().unwrap_or("").to_string();
    let item_kind = ITEM_KINDS.contains(&kind.as_str());
    if kind != "weapon" && !item_kind {
        return not_rollable("other_kind", kind);
    }
    match cx.mode {
        ParityMode::Reference if kind == "weapon" => reference_weapon(path, record, cx),
        ParityMode::Reference => reference_item(reg, path, record, &kind, cx),
        _ => off_roll(reg, path, record, &kind, None, cx),
    }
}

/// An Off-mode roll of a candidate the caller (a pool) already drew: no manufacturer draw.
pub fn roll_candidate(
    reg: &Registry,
    balance: &ObjectPath,
    candidate: &PoolCandidate,
    cx: &mut RollCx,
) -> Result<RollResult, ItemError> {
    // Off mode only: the mode is refused before anything else is read (Reference rolls go through `roll`).
    if cx.mode == ParityMode::Reference {
        return Err(ItemError::WrongMode {
            call: "roll_candidate",
            mode: cx.mode,
        });
    }
    let path = balance.as_str();
    let Some(record) = reg.balance(path) else {
        return not_rollable("unknown_balance", path);
    };
    let kind = record.at("kind").as_str().unwrap_or("").to_string();
    off_roll(reg, path, record, &kind, Some(candidate), cx)
}

/// The first variant of the record whose definition is `definition`.
fn variant_of<'r>(record: &'r Pv, definition: &str) -> Option<&'r Pv> {
    record
        .at("variants")
        .items()
        .iter()
        .find(|v| v.at("definition").as_str() == Some(definition))
}

/// How a roll notes its slots (the two oracles word and count them differently).
struct SlotNotes<'n> {
    /// The note for a weight taken as 1.
    runtime: &'n str,
    /// `true`: noted when any window-passing entry had no weight (`weapon_recipe.roll`); `false`: when a candidate
    /// still has none after the repeats merged (`item_roll.roll`).
    per_entry: bool,
    /// The note for a slot whose every candidate weighs 0 (`None`: such a slot is skipped silently).
    empty: Option<&'n str>,
}

/// One pick per slot in `slots` order (a slot with no candidate takes no part and no draw).
fn pick_slots(
    found: &PartCandidates,
    slots: &[&str],
    rng: &mut dyn Rand15,
    notes: &mut Vec<String>,
    rule: &SlotNotes<'_>,
) -> Vec<SlotPick> {
    let mut out = Vec::new();
    for slot in slots {
        let Some(live) = found.get(*slot) else {
            continue;
        };
        if live.parts.is_empty() && rule.empty.is_none() {
            continue;
        }
        let runtime = if rule.per_entry {
            live.runtime_weight
        } else {
            live.parts.values().any(Option::is_none)
        };
        if runtime {
            notes.push(format!("{slot}: {}", rule.runtime));
        }
        let picked: Picked<String> = pick(
            live.parts
                .iter()
                .map(|(p, w)| (p.clone(), Some(w.unwrap_or(1.0)))),
            rng,
        );
        match picked.choice.clone() {
            Some(part) => out.push(SlotPick {
                slot: (*slot).to_string(),
                weight: picked.weight().unwrap_or(0.0),
                part,
                candidates: picked.candidates,
            }),
            None => {
                if let Some(note) = rule.empty {
                    notes.push(format!("{slot}: {note}"));
                }
            }
        }
    }
    out
}

/// The registry's prefix or title list of a definition or part.
fn registry_list(reg: &Registry, table: &str, path: &str, key: &str) -> Vec<Pv> {
    reg.tree().at(table).at(path).at(key).items().to_vec()
}

/// A raw `PrefixList` / `TitleList` of an object, empty entries left out.
fn raw_list(data: &RollData, path: &str, key: &str) -> Vec<Pv> {
    data.get(path)
        .map(|p| {
            p.at(key)
                .items()
                .iter()
                .filter(|v| v.truthy())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// `item_roll.roll` (Reference).
fn reference_item(
    reg: &Registry,
    balance: &str,
    record: &Pv,
    kind: &str,
    cx: &mut RollCx,
) -> Result<RollResult, ItemError> {
    let data = cx.data;
    let stage = i64::from(cx.balance.stage.0);
    let mut notes = Vec::new();
    let makers = record.at("manufacturers").items();
    let options = manufacturer_options(record, stage);
    let (maker, grade_index, grade) = if makers.is_empty() {
        (None, None, None)
    } else if options.is_empty() {
        return not_rollable("no_drawable_grade_at_stage", format!("stage {stage}"));
    } else {
        let chosen = if let Some(forced) = cx.manufacturer.clone() {
            match options.iter().find(|o| o.manufacturer == forced) {
                Some(o) => o.clone(),
                None => return not_rollable("manufacturer_not_an_option", forced),
            }
        } else if options.len() == 1 {
            options[0].clone()
        } else {
            let picked = pick(
                options
                    .iter()
                    .map(|o| ((o.manufacturer.clone(), o.grade), Some(o.weight))),
                cx.rng()?,
            );
            let Some((m, g)) = picked.choice else {
                return not_rollable("no_drawable_grade_at_stage", format!("stage {stage}"));
            };
            match options.iter().find(|o| o.manufacturer == m && o.grade == g) {
                Some(o) => o.clone(),
                None => {
                    return not_rollable("no_drawable_grade_at_stage", format!("stage {stage}"));
                }
            }
        };
        let grade = makers
            .iter()
            .find(|e| e.at("manufacturer").as_str() == Some(chosen.manufacturer.as_str()))
            .and_then(|e| e.at("grades").items().get(chosen.grade))
            .cloned();
        (Some(chosen.manufacturer), Some(chosen.grade), grade)
    };
    let interpolating = interpolates(data, balance);
    let definitions: Vec<String> = if kind == "class_mod" {
        let mut out: Vec<String> = Vec::new();
        for d in record.at("class_mod_definitions").items() {
            let d = d.as_str().unwrap_or("").to_string();
            if !out.contains(&d) {
                out.push(d);
            }
        }
        out
    } else if let Some(d) = grade
        .as_ref()
        .filter(|_| !interpolating)
        .and_then(|g| g.at("definition").as_str().filter(|d| !d.is_empty()))
    {
        vec![d.to_string()]
    } else {
        record
            .at("inventory_definition")
            .as_str()
            .filter(|d| !d.is_empty())
            .map(|d| vec![d.to_string()])
            .unwrap_or_default()
    };
    let definition = match definitions.len() {
        0 => return not_rollable("no_definition", ""),
        1 => definitions[0].clone(),
        // The uniform class-mod definition pick: a Reference stand-in (architecture section 6.1).
        _ => match pick(
            definitions.iter().map(|d| (d.clone(), Some(1.0))),
            cx.rng()?,
        )
        .choice
        {
            Some(d) => d,
            None => return not_rollable("no_definition", ""),
        },
    };
    let Some(variant) = variant_of(record, &definition) else {
        return not_rollable("definition_has_no_variant", definition);
    };
    let mut found = part_candidates(
        variant.at("slots"),
        merge::is_weapon_balance(record),
        stage,
        maker.as_deref(),
        &mut |v| Ok(reference_value(data, v)),
    )?;
    let removed = apply_relic_hook(reg.tree(), data, &definition, &mut found);
    let rule = SlotNotes {
        runtime: "a weight needs runtime state; taken as 1",
        per_entry: false,
        empty: None,
    };
    let slots = pick_slots(&found, &ITEM_SLOTS, cx.rng()?, &mut notes, &rule);
    let level = match &grade {
        Some(g) if !interpolating => match g.at("exp_level").number() {
            // item_roll.roll: `grade.get('exp_level') or 1` (an int in the data).
            Some(l) if l != 0.0 => trunc_i64(l, "a grade's exp_level (item_roll.roll step 4)")?,
            _ => 1,
        },
        _ => stage,
    };
    let name_cx = NameContext::new(Some(level), maker.as_deref(), None);
    let mut holds = |e: &Pv, n: &NameContext<'_>| -> Result<bool, ItemError> {
        Ok(tool_expressions_hold(data, e, n))
    };
    let mut pick_name = |key: &str| -> Result<Option<String>, ItemError> {
        let mut lists = vec![registry_list(reg, "definitions", &definition, key)];
        for s in &slots {
            lists.push(registry_list(reg, "parts", &s.part, key));
        }
        choose(data, &lists, &name_cx, &mut holds)
    };
    let prefix = pick_name("prefix")?;
    let title = pick_name("title")?;
    let parts: Vec<&str> = slots.iter().map(|s| s.part.as_str()).collect();
    let names = [prefix.as_deref(), title.as_deref()];
    let rarity = rarity::rarity(data, &definition, &parts, &names, &mut |v| {
        Ok(data.number(v))
    })?;
    let grade_or_level = if interpolating {
        to_i32(level, "the item level")?
    } else {
        to_i32(grade_index.map_or(0, |g| g as i64), "the grade index")?
    };
    let value_total = value_total(data, &definition, &parts, &names);
    Ok(RollResult::Item(Box::new(Rolled {
        balance: balance.to_string(),
        kind: kind.to_string(),
        mode: ParityMode::Reference,
        stage,
        level,
        grade_or_level,
        manufacturer: maker,
        grade: grade_index,
        definition,
        slots,
        prefix,
        title,
        rarity,
        hook_removed: removed.len(),
        notes,
        value_total,
    })))
}

/// The oracle's stand-in reading of a registry value: a number as it is, `{"init": ...}` through
/// `weapon_recipe.attribute_value` (no attribute values), `None` when that needs runtime state or raises.
fn reference_value(data: &RollData, v: &Pv) -> Option<f64> {
    match v {
        Pv::Null => None,
        Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => v.number(),
        other => {
            let init = other.get("init")?;
            data.attribute_value(init, &[]).ok().flatten()
        }
    }
}

fn value_total(
    data: &RollData,
    definition: &str,
    parts: &[&str],
    names: &[Option<&str>],
) -> Option<f32> {
    let mut sources: Vec<&str> = parts.to_vec();
    sources.extend(names.iter().flatten());
    part_value_total(data, definition, &sources)
}

/// `weapon_recipe.roll` (Reference): the root-most manufacturer and weapon type, no manufacturer draw, level = stage.
fn reference_weapon(balance: &str, record: &Pv, cx: &mut RollCx) -> Result<RollResult, ItemError> {
    let data = cx.data;
    let stage = i64::from(cx.balance.stage.0);
    let chosen = merge::choice(record, merge::Reading::RootMost);
    let maker = chosen.manufacturers.first().cloned().flatten();
    let weapon_type = chosen.definition.clone();
    let Some(variant) = record.at("variants").items().first() else {
        return not_rollable("definition_has_no_variant", "");
    };
    let found = part_candidates(
        variant.at("slots"),
        true,
        stage,
        maker.as_deref(),
        &mut |v| Ok(reference_value(data, v)),
    )?;
    let mut notes = Vec::new();
    let rule = SlotNotes {
        runtime: "some weights need runtime attributes; treated as 1",
        per_entry: true,
        empty: Some("every candidate weighs 0; the game leaves the slot empty"),
    };
    let slots = pick_slots(&found, &WEAPON_SLOTS, cx.rng()?, &mut notes, &rule);
    let elemental = slots
        .iter()
        .find(|s| s.slot == "Elemental")
        .map(|s| s.part.as_str());
    let damage = native_damage_type(data, elemental, weapon_type.as_deref());
    let name_cx = NameContext::new(Some(stage), maker.as_deref(), damage.as_deref());
    let mut holds = |e: &Pv, n: &NameContext<'_>| -> Result<bool, ItemError> {
        Ok(tool_expressions_hold(data, e, n))
    };
    let mut pick_name = |key: &str| -> Result<Option<String>, ItemError> {
        let mut lists = Vec::new();
        if let Some(t) = &weapon_type {
            lists.push(raw_list(data, t, key));
        } else {
            lists.push(Vec::new());
        }
        for s in &slots {
            lists.push(raw_list(data, &s.part, key));
        }
        choose(data, &lists, &name_cx, &mut holds)
    };
    let prefix = pick_name("PrefixList")?;
    let title = pick_name("TitleList")?;
    let definition = weapon_type.unwrap_or_default();
    let parts: Vec<&str> = slots.iter().map(|s| s.part.as_str()).collect();
    let names = [prefix.as_deref(), title.as_deref()];
    let rarity = rarity::rarity(data, &definition, &parts, &names, &mut |v| {
        Ok(data.number(v))
    })?;
    let value_total = value_total(data, &definition, &parts, &names);
    Ok(RollResult::Item(Box::new(Rolled {
        balance: balance.to_string(),
        kind: "weapon".to_string(),
        mode: ParityMode::Reference,
        stage,
        level: stage,
        grade_or_level: to_i32(stage, "the stage")?,
        manufacturer: maker,
        grade: None,
        definition,
        slots,
        prefix,
        title,
        rarity,
        hook_removed: 0,
        notes,
        value_total,
    })))
}

/// The Off-mode roll (module documentation), of `chosen` when the caller drew the candidate.
fn off_roll(
    reg: &Registry,
    balance: &str,
    record: &Pv,
    kind: &str,
    chosen: Option<&PoolCandidate>,
    cx: &mut RollCx,
) -> Result<RollResult, ItemError> {
    if kind == "class_mod" {
        return Err(NotYet::needs_real_game(
            "which of a class mod balance's ClassModDefinitions the game rolls (its ClassModBalanceDefinition \
             override is not read)",
            6,
            RECORD,
        )
        .into());
    }
    if kind != "weapon" && !ITEM_KINDS.contains(&kind) {
        return not_rollable("other_kind", kind);
    }
    let data = cx.data;
    let snap = cx.balance;
    let cap = ow_balance::cap::item_stage_cap(snap.playthrough, snap.op, snap.level_cap_packs)?;
    let candidate = match chosen {
        Some(c) => c.clone(),
        None => {
            let found = expand(data, record, snap.stage.0, cap.0, &mut |v| cx.off_value(v))?;
            if found.is_empty() {
                return not_rollable("no_candidate_at_stage", format!("stage {}", snap.stage.0));
            }
            let found = match cx.manufacturer.clone() {
                Some(m) => match found.into_iter().find(|c| c.manufacturer == m) {
                    Some(c) => vec![c],
                    None => return not_rollable("manufacturer_not_an_option", m),
                },
                None => found,
            };
            if found.len() == 1 && cx.manufacturer.is_some() {
                found[0].clone()
            } else {
                match pick_candidate(&found, cx.rng()?).choice {
                    Some(i) => found[i].clone(),
                    None => return not_rollable("no_candidate_above_the_floor", ""),
                }
            }
        }
    };
    let weapon = kind == "weapon";
    let definition = candidate.definition.clone();
    // A weapon balance has one variant (its weapon type's); an item balance one per definition.
    let variant = if weapon {
        record.at("variants").items().first()
    } else {
        variant_of(record, &definition)
    };
    let Some(variant) = variant else {
        return not_rollable("definition_has_no_variant", definition);
    };
    let stage = i64::from(candidate.stage);
    let maker = candidate.manufacturer.clone();
    let mut found = part_candidates(
        variant.at("slots"),
        merge::is_weapon_balance(record),
        stage,
        Some(&maker),
        &mut |v| cx.off_value(v),
    )?;
    let removed = apply_relic_hook(reg.tree(), data, &definition, &mut found);
    // Only the slots the pick draws: a slot pick_slots never reads cannot make the roll NotYet.
    let slot_names: &[&str] = if weapon { &WEAPON_SLOTS } else { &ITEM_SLOTS };
    if let Some((slot, _)) = found
        .iter()
        .filter(|(slot, _)| slot_names.contains(&slot.as_str()))
        .find(|(_, s)| s.parts.values().any(Option::is_none))
    {
        return Err(NotYet::not_read(
            format!("a part weight with no init data in slot {slot} (no weight index)"),
            RECORD,
        )
        .into());
    }
    let mut notes = Vec::new();
    let rule = SlotNotes {
        runtime: "",
        per_entry: false,
        empty: None,
    };
    let slots = pick_slots(&found, slot_names, cx.rng()?, &mut notes, &rule);
    let level = i64::from(candidate.level);
    let elemental = slots
        .iter()
        .find(|s| s.slot == "Elemental")
        .map(|s| s.part.as_str());
    let damage = if weapon {
        native_damage_type(data, elemental, Some(&definition))
    } else {
        None
    };
    let name_cx = NameContext::new(Some(level), Some(&maker), damage.as_deref());
    let mut prefix_title = [None, None];
    for (i, key) in ["PrefixList", "TitleList"].iter().enumerate() {
        let mut lists = vec![raw_list(data, &definition, key)];
        for s in &slots {
            lists.push(raw_list(data, &s.part, key));
        }
        let mut holds =
            |e: &Pv, _: &NameContext<'_>| -> Result<bool, ItemError> { cx.off_holds(e) };
        prefix_title[i] = names::choose(data, &lists, &name_cx, &mut holds)?;
    }
    let [prefix, title] = prefix_title;
    let parts: Vec<&str> = slots.iter().map(|s| s.part.as_str()).collect();
    let names = [prefix.as_deref(), title.as_deref()];
    let rarity = rarity::rarity(data, &definition, &parts, &names, &mut |v| cx.off_term(v))?;
    let value_total = value_total(data, &definition, &parts, &names);
    Ok(RollResult::Item(Box::new(Rolled {
        balance: balance.to_string(),
        kind: kind.to_string(),
        mode: ParityMode::Off,
        stage,
        level,
        grade_or_level: candidate.grade_or_level,
        manufacturer: Some(maker),
        grade: Some(candidate.grade),
        definition,
        slots,
        prefix,
        title,
        rarity,
        hook_removed: removed.len(),
        notes,
        value_total,
    })))
}

// ---- output -------------------------------------------------------------------------------------------------------

fn pv_map(entries: Vec<(&str, Pv)>) -> Pv {
    Pv::Map(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

fn pv_opt(v: &Option<String>) -> Pv {
    v.as_ref().map_or(Pv::Null, |s| Pv::Str(s.clone()))
}

/// The labels the item JSON carries (`item_roll.UNVERIFIED`'s topics, in this crate's words).
const UNVERIFIED: [&str; 8] = [
    "manufacturer and grade: Reference draws them in the balance roll (stand-in); Off mode at the pool level",
    "class mod definition: Reference picks uniformly (stand-in); Off mode is NotYet (question 6)",
    "part merge, entry weights and stage windows reused from weapons for items",
    "EPRM_Complete merge: the registry's (every slot replaced), reading (b); reading (a) not modelled",
    "relic weight hook as read from native code",
    "name parts counted in the rarity sum (question 6)",
    "level = stage for an interpolating balance; Reference applies no stage cap",
    "the game's random sequence is not reproduced; one seeded CrtRand per roll",
];

impl Rolled {
    /// The item's definition data (`DefinitionData`): the nine slots by their index for the kind, the game stage
    /// as stored, `ManufacturerGradeIndex` as the grade-or-level word, no unique id.
    pub fn definition_data(&self) -> Result<DefinitionData, ItemError> {
        let path =
            |p: &str| ObjectPath::parse(p).map_err(|e| ItemError::BadData(format!("{p}: {e}")));
        let kind = match self.kind.as_str() {
            "weapon" => ItemKind::Weapon,
            "shield" => ItemKind::Shield,
            "grenade_mod" => ItemKind::GrenadeMod,
            "class_mod" => ItemKind::ClassMod,
            "relic" => ItemKind::Relic,
            other => {
                return Err(ItemError::BadData(format!(
                    "no definition data for kind {other}"
                )));
            }
        };
        let mut parts: [Option<ObjectPath>; SLOT_COUNT] = std::array::from_fn(|_| None);
        for s in &self.slots {
            let index = if kind == ItemKind::Weapon {
                WEAPON_SLOTS
                    .iter()
                    .position(|n| *n == s.slot)
                    .and_then(WeaponSlot::from_index)
                    .map(WeaponSlot::index)
            } else {
                ITEM_SLOTS
                    .iter()
                    .position(|n| *n == s.slot)
                    .and_then(ItemSlot::from_index)
                    .map(ItemSlot::index)
            };
            let Some(index) = index else {
                return Err(ItemError::BadData(format!(
                    "slot {} of a {} roll",
                    s.slot, self.kind
                )));
            };
            parts[index] = Some(path(&s.part)?);
        }
        let opt = |v: &Option<String>| v.as_deref().map(path).transpose();
        Ok(DefinitionData {
            kind,
            balance: Some(path(&self.balance)?),
            definition: path(&self.definition)?,
            manufacturer: opt(&self.manufacturer)?,
            manufacturer_grade: self.grade_or_level,
            parts,
            prefix: opt(&self.prefix)?,
            title: opt(&self.title)?,
            game_stage: ow_balance::GameStage(to_i32(self.stage, "the stage")?),
            unique_id: 0,
        })
    }

    /// The roll in `item_roll.roll`'s dict shape, which `item_roll.check` reads.
    pub fn to_item_pv(&self, seed: u32) -> Pv {
        let parts = Pv::Map(
            self.slots
                .iter()
                .map(|s| (s.slot.clone(), Pv::Str(s.part.clone())))
                .collect(),
        );
        pv_map(vec![
            ("balance", Pv::Str(self.balance.clone())),
            ("kind", Pv::Str(self.kind.clone())),
            ("seed", Pv::Int(i64::from(seed))),
            ("stage", Pv::Int(self.stage)),
            ("level", Pv::Int(self.level)),
            ("manufacturer", pv_opt(&self.manufacturer)),
            ("grade", self.grade.map_or(Pv::Null, |g| Pv::Int(g as i64))),
            ("definition", Pv::Str(self.definition.clone())),
            ("parts", parts),
            (
                "name_parts",
                pv_map(vec![
                    ("prefix", pv_opt(&self.prefix)),
                    ("title", pv_opt(&self.title)),
                ]),
            ),
            (
                "rarity",
                pv_map(vec![
                    ("level", Pv::Int(self.rarity.level)),
                    ("band", pv_opt(&self.rarity.band)),
                    (
                        "unresolved",
                        Pv::List(
                            self.rarity
                                .unresolved
                                .iter()
                                .cloned()
                                .map(Pv::Str)
                                .collect(),
                        ),
                    ),
                    (
                        "name_parts_counted",
                        Pv::Bool(self.rarity.name_parts_counted),
                    ),
                ]),
            ),
            ("hook_removed", Pv::Int(self.hook_removed as i64)),
            (
                "notes",
                Pv::List(self.notes.iter().cloned().map(Pv::Str).collect()),
            ),
            (
                "unverified",
                Pv::List(
                    UNVERIFIED
                        .iter()
                        .map(|s| Pv::Str((*s).to_string()))
                        .collect(),
                ),
            ),
            (
                "value_total",
                self.value_total
                    .map_or(Pv::Null, |v| Pv::Float(f64::from(v))),
            ),
        ])
    }

    /// The roll in `weapon_recipe.roll`'s shape, for the fields the weapon comparison reads: balance,
    /// manufacturer, seed, game stage, weapon type, name, title, prefix, parts (part, weight, candidates), notes.
    pub fn to_weapon_pv(&self, seed: u32, data: &RollData) -> Pv {
        let described = |n: &Option<String>| match n {
            None => Pv::Null,
            Some(p) => pv_map(vec![
                ("part", Pv::Str(p.clone())),
                (
                    "text",
                    data.get(p).map_or(Pv::Null, |d| d.at("PartName").clone()),
                ),
                ("priority", Pv::Float(names::name_priority(data, p))),
            ]),
        };
        let text = |n: &Option<String>| {
            n.as_ref()
                .and_then(|p| data.get(p))
                .and_then(|d| d.at("PartName").as_str().map(str::to_string))
                .filter(|t| !t.is_empty())
        };
        let name: Vec<String> = [text(&self.prefix), text(&self.title)]
            .into_iter()
            .flatten()
            .collect();
        let mut parts = OrderedMap::new();
        for s in &self.slots {
            parts.insert(
                s.slot.clone(),
                pv_map(vec![
                    ("part", Pv::Str(s.part.clone())),
                    ("weight", Pv::Float(s.weight)),
                    (
                        "candidates",
                        Pv::List(
                            s.candidates
                                .iter()
                                .map(|(p, w)| {
                                    pv_map(vec![
                                        ("part", Pv::Str(p.clone())),
                                        ("weight", Pv::Float(*w)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]),
            );
        }
        pv_map(vec![
            ("balance", Pv::Str(self.balance.clone())),
            ("manufacturer", pv_opt(&self.manufacturer)),
            ("seed", Pv::Int(i64::from(seed))),
            ("game_stage", Pv::Int(self.stage)),
            (
                "weapon_type",
                if self.definition.is_empty() {
                    Pv::Null
                } else {
                    Pv::Str(self.definition.clone())
                },
            ),
            ("name", Pv::Str(name.join(" "))),
            ("title", described(&self.title)),
            ("prefix", described(&self.prefix)),
            (
                "parts",
                Pv::Map(parts.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
            ),
            (
                "notes",
                Pv::List(self.notes.iter().cloned().map(Pv::Str).collect()),
            ),
        ])
    }
}

/// `item_roll.spread_seeds`: `count` seeds spread over the 32-bit range (consecutive small seeds all draw near 0
/// first with the C runtime generator).
pub fn spread_seeds(count: u32) -> Vec<u32> {
    (1..=count).map(|i| i.wrapping_mul(0x9E37_79B1)).collect()
}

/// The stages the census rolls at (`item_roll.CENSUS_STAGES`, `weapon_gen_parity.STAGES`).
pub const CENSUS_STAGES: [i32; 6] = [1, 8, 15, 30, 50, 80];

#[cfg(test)]
pub(crate) mod tests {
    //! Synthetic rolls (AGENTS.md rule 9): one invented package built with `ow_package::testkit`, every name and
    //! number invented except the globals path the band rule reads and the record constants the tests name.

    use super::*;
    use crate::registry::{CensusInput, Holder, PathHolders, build_from_census};
    use ow_balance::types::{
        AwesomeLevel, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
    };
    use ow_core::notyet::Missing;
    use ow_object::heap::{ObjectSpace, Origin};
    use ow_object::ids::{ClassId, PropAddr};
    use ow_object::layout::{PropKind, PropertyDecl};
    use ow_object::value::Value;
    use ow_package::package::Package;
    use ow_package::testkit::{PackageBuilder, ints};
    use std::path::PathBuf;

    // ---- the package fixture ----

    #[derive(Clone)]
    pub(crate) enum V {
        Int(i32),
        Float(f32),
        Bool(bool),
        Str(&'static str),
        Name(&'static str),
        Ref(Option<&'static str>),
        Enum(&'static str, &'static str),
        Struct(&'static str, Vec<(&'static str, V)>),
        Array(Vec<V>),
    }

    pub(crate) fn r(path: &'static str) -> V {
        V::Ref(Some(path))
    }

    type Props = Vec<(&'static str, V)>;

    /// One package whose exports are outermost objects holding the dotted path in their own name; a reference to a
    /// path the package does not export becomes an import, as the registry's fixture does.
    #[derive(Default)]
    pub(crate) struct Syn {
        exports: Vec<(&'static str, &'static str, Props)>,
        imports: Vec<(&'static str, &'static str, i32, String)>,
    }

    impl Syn {
        pub(crate) fn add(&mut self, path: &'static str, class: &'static str, props: Props) {
            self.exports.push((path, class, props));
        }

        fn import(
            &mut self,
            b: &mut PackageBuilder,
            cp: &'static str,
            cn: &'static str,
            outer: i32,
            name: &str,
        ) -> i32 {
            let key = (cp, cn, outer, name.to_string());
            if let Some(i) = self.imports.iter().position(|k| *k == key) {
                return -(i as i32 + 1);
            }
            self.imports.push(key);
            b.add_import(cp, cn, outer, name)
        }

        pub(crate) fn build(mut self) -> Vec<u8> {
            let mut b = PackageBuilder::new_empty();
            b.name("None");
            let mut indices = Vec::new();
            let exports: Vec<(&'static str, &'static str)> =
                self.exports.iter().map(|(p, c, _)| (*p, *c)).collect();
            for (path, class) in &exports {
                let (package, name) = class.split_once('.').unwrap();
                let outer = self.import(&mut b, "Core", "Package", 0, package);
                let class_ref = self.import(&mut b, "Core", "Class", outer, name);
                indices.push(b.add_export(class_ref, path, Vec::new()));
            }
            let all: Vec<Props> = self.exports.iter().map(|(_, _, p)| p.clone()).collect();
            for (i, props) in all.iter().enumerate() {
                let mut payload = ints(&[0]);
                payload.extend(self.body(&mut b, props));
                b.set_payload(indices[i], payload);
            }
            b.build()
        }

        fn reference(&mut self, b: &mut PackageBuilder, path: Option<&'static str>) -> i32 {
            match path {
                None => 0,
                Some(p) => match self.exports.iter().position(|(q, _, _)| *q == p) {
                    Some(i) => i as i32 + 1,
                    None => self.import(b, "Core", "Object", 0, p),
                },
            }
        }

        fn value(&mut self, b: &mut PackageBuilder, name: &str, v: &V) -> Vec<u8> {
            match v {
                V::Int(i) => b.int_tag(name, *i),
                V::Float(x) => b.float_tag(name, *x),
                V::Bool(x) => b.bool_tag(name, *x),
                V::Str(s) => b.str_tag(name, s),
                V::Name(s) => b.name_tag(name, s),
                V::Ref(p) => {
                    let r = self.reference(b, *p);
                    b.object_tag(name, r)
                }
                V::Enum(e, value) => b.enum_tag(name, e, value),
                V::Struct(s, fields) => {
                    let body = self.body(b, fields);
                    b.struct_tag(name, s, &body)
                }
                V::Array(items) => {
                    let mut data = Vec::new();
                    for item in items {
                        match item {
                            V::Struct(_, fields) => data.extend(self.body(b, fields)),
                            V::Ref(p) => {
                                let r = self.reference(b, *p);
                                data.extend(ints(&[r]));
                            }
                            _ => panic!("fixture arrays hold structs or references"),
                        }
                    }
                    b.array_tag(name, items.len() as i32, &data)
                }
            }
        }

        fn body(&mut self, b: &mut PackageBuilder, fields: &[(&'static str, V)]) -> Vec<u8> {
            let mut out = Vec::new();
            for (n, v) in fields {
                out.extend(self.value(b, n, v));
            }
            out.extend(b.none());
            out
        }
    }

    pub(crate) fn konst(value: f32) -> V {
        V::Struct(
            "AttributeInitializationData",
            vec![
                ("BaseValueConstant", V::Float(value)),
                ("BaseValueScaleConstant", V::Float(1.0)),
            ],
        )
    }

    /// The consolidated init data every part list here shares: 0 -> 1, 1 -> 100, 2 -> 5, 3 -> 30, 4 -> 0, 5 -> the
    /// per-element minimum attribute (a constant resolver of 20; runtime state for the oracle's subset).
    fn consolidated() -> (&'static str, V) {
        let attr = V::Struct(
            "AttributeInitializationData",
            vec![
                ("BaseValueAttribute", r(MIN_STAGE_ATTR)),
                ("BaseValueScaleConstant", V::Float(1.0)),
            ],
        );
        (
            "ConsolidatedAttributeInitData",
            V::Array(vec![
                konst(1.0),
                konst(100.0),
                konst(5.0),
                konst(30.0),
                konst(0.0),
                attr,
            ]),
        )
    }

    fn weighted(
        part: &'static str,
        low: i32,
        high: i32,
        makers: &[(&'static str, i32)],
        default: Option<i32>,
    ) -> V {
        let rows = makers
            .iter()
            .map(|(m, i)| {
                V::Struct(
                    "ManufacturerWeight",
                    vec![("Manufacturer", r(m)), ("DefaultWeightIndex", V::Int(*i))],
                )
            })
            .collect();
        let mut fields = vec![
            ("Part", r(part)),
            ("Manufacturers", V::Array(rows)),
            ("MinGameStageIndex", V::Int(low)),
            ("MaxGameStageIndex", V::Int(high)),
        ];
        if let Some(d) = default {
            fields.push(("DefaultWeightIndex", V::Int(d)));
        }
        V::Struct("WeightedPart", fields)
    }

    /// A flat entry with no stage window.
    fn w(part: &'static str) -> V {
        weighted(part, -1, -1, &[], None)
    }

    fn list(rows: Vec<V>) -> Props {
        vec![("WeightedParts", V::Array(rows)), consolidated()]
    }

    fn part_data(rows: Vec<V>) -> V {
        V::Struct(
            "PartData",
            vec![
                ("bEnabled", V::Bool(true)),
                ("WeightedParts", V::Array(rows)),
            ],
        )
    }

    fn grade(
        low: i32,
        high: i32,
        level: i32,
        definition: Option<&'static str>,
        spawn: Option<(f32, f32)>,
    ) -> V {
        let mods = V::Struct(
            "Mods",
            vec![
                ("ExpLevel", V::Int(level)),
                ("CustomInventoryDefinition", V::Ref(definition)),
            ],
        );
        let need = V::Struct(
            "Need",
            vec![
                ("MinGameStage", V::Int(low)),
                ("MaxGameStage", V::Int(high)),
            ],
        );
        let mut fields = vec![("GradeModifiers", mods), ("GameStageRequirement", need)];
        if let Some((min, max)) = spawn {
            fields.push(("MinSpawnProbabilityModifier", konst(min)));
            fields.push(("MaxSpawnProbabilityModifier", konst(max)));
        }
        V::Struct("GradeData", fields)
    }

    fn makers(rows: Vec<(&'static str, Vec<V>)>) -> V {
        V::Array(
            rows.into_iter()
                .map(|(m, g)| {
                    V::Struct(
                        "ManufacturerGradeData",
                        vec![("Manufacturer", r(m)), ("Grades", V::Array(g))],
                    )
                })
                .collect(),
        )
    }

    fn rarity(v: f32) -> (&'static str, V) {
        ("Rarity", konst(v))
    }

    fn slot_rows(names: &[&'static str]) -> V {
        V::Array(
            names
                .iter()
                .map(|n| V::Struct("Slot", vec![("SlotName", V::Name(n))]))
                .collect(),
        )
    }

    pub(crate) const ACME: &str = "GD_RollTest.Makers.Acme";
    pub(crate) const BOLT: &str = "GD_RollTest.Makers.Bolt";
    pub(crate) const MIN_STAGE_ATTR: &str = "GD_RollTest.Attr.MinStage";
    const IB: &str = "WillowGame.InventoryBalanceDefinition";
    const LIST: &str = "WillowGame.ItemPartListDefinition";
    pub(crate) const SHIELD: &str = "GD_RollTest.Bal.Shield";
    pub(crate) const GRENADE: &str = "GD_RollTest.Bal.Grenade";
    pub(crate) const FIXED: &str = "GD_RollTest.Bal.Fixed";
    pub(crate) const CLASS_MOD: &str = "GD_RollTest.Bal.ClassMod";
    pub(crate) const RELIC: &str = "GD_RollTest.Bal.Relic";
    pub(crate) const WEAPON: &str = "GD_RollTest.Bal.Weapon";
    pub(crate) const LATE: &str = "GD_RollTest.Bal.Late";
    const CHAIN_BASE: &str = "GD_RollTest.Chain.Base";
    const LEAF_UNCOVERED: &str = "GD_RollTest.Chain.LeafUncovered";
    const LEAF_COVERED: &str = "GD_RollTest.Chain.LeafCovered";
    const FIXED_BASE: &str = "GD_RollTest.Chain.FixedBase";
    const LEAF_RANGE: &str = "GD_RollTest.Chain.LeafRange";
    const LEAF_FIXED_OVER_INTERP: &str = "GD_RollTest.Chain.LeafFixedOverInterp";

    pub(crate) fn package() -> Syn {
        let mut p = Syn::default();
        p.add(ACME, "Engine.ManufacturerDefinition", vec![]);
        p.add(BOLT, "Engine.ManufacturerDefinition", vec![]);
        // The bands (the globals path is the one the rule reads; the levels are invented).
        let band = |low: i32, high: i32, rating: &'static str| {
            V::Struct(
                "RarityLevelColor",
                vec![
                    ("MinLevel", V::Int(low)),
                    ("MaxLevel", V::Int(high)),
                    ("RarityRating", V::Enum("EItemRarity", rating)),
                ],
            )
        };
        p.add(
            GLOBALS_PATH,
            "WillowGame.GlobalsDefinition",
            vec![(
                "RarityLevelColors",
                V::Array(vec![
                    band(0, 0, "RARITY_Unknown"),
                    band(1, 3, "RARITY_Common"),
                    band(4, 6, "RARITY_Uncommon"),
                    band(7, 20, "RARITY_Rare"),
                ]),
            )],
        );
        // The per-element minimum: an attribute whose only value resolver is a constant resolver.
        p.add(
            MIN_STAGE_ATTR,
            "Engine.AttributeDefinition",
            vec![(
                "ValueResolverChain",
                V::Array(vec![r("GD_RollTest.Attr.MinStage_Constant")]),
            )],
        );
        p.add(
            "GD_RollTest.Attr.MinStage_Constant",
            "GearboxFramework.ConstantAttributeValueResolver",
            vec![("ConstantValue", V::Float(20.0))],
        );
        // Name parts.
        p.add(
            "GD_RollTest.Names.Title_Shield",
            "WillowGame.ItemNamePartDefinition",
            vec![
                ("PartName", V::Str("Bulwark")),
                ("Priority", V::Float(2.0)),
                rarity(1.0),
            ],
        );
        p.add(
            "GD_RollTest.Names.Prefix_Low",
            "WillowGame.ItemNamePartDefinition",
            vec![
                ("PartName", V::Str("Low")),
                ("MaxExpLevelRequirement", V::Int(10)),
            ],
        );

        // Shield: two body parts in Alpha (rarity 1 and 2), one material with the title.
        p.add(
            "GD_RollTest.Shield.A1",
            "WillowGame.ShieldPartDefinition",
            vec![rarity(1.0)],
        );
        p.add(
            "GD_RollTest.Shield.A2",
            "WillowGame.ShieldPartDefinition",
            vec![rarity(2.0)],
        );
        p.add(
            "GD_RollTest.Shield.M1",
            "WillowGame.ShieldPartDefinition",
            vec![
                rarity(2.0),
                (
                    "TitleList",
                    V::Array(vec![r("GD_RollTest.Names.Title_Shield")]),
                ),
            ],
        );
        p.add(
            "GD_RollTest.Shield.List_A",
            LIST,
            list(vec![w("GD_RollTest.Shield.A1"), w("GD_RollTest.Shield.A2")]),
        );
        p.add(
            "GD_RollTest.Shield.List_M",
            LIST,
            list(vec![w("GD_RollTest.Shield.M1")]),
        );
        p.add(
            "GD_RollTest.Shield.Def",
            "WillowGame.ShieldDefinition",
            vec![
                ("AlphaParts", r("GD_RollTest.Shield.List_A")),
                ("MaterialParts", r("GD_RollTest.Shield.List_M")),
                ("BaseRarity", konst(1.0)),
                (
                    "PrefixList",
                    V::Array(vec![r("GD_RollTest.Names.Prefix_Low")]),
                ),
            ],
        );
        p.add(
            SHIELD,
            IB,
            vec![
                ("InventoryDefinition", r("GD_RollTest.Shield.Def")),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(1, 100, 1, None, None)])]),
                ),
            ],
        );

        // Grenade mod: two manufacturers; Delta has a part weighted for Acme only (5, default 0), a flat part, and
        // a part whose window starts at the per-element minimum (20).
        p.add(
            "GD_RollTest.Grenade.D1",
            "WillowGame.GrenadeModPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Grenade.D2",
            "WillowGame.GrenadeModPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Grenade.D3",
            "WillowGame.GrenadeModPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Grenade.E1",
            "WillowGame.GrenadeModPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Grenade.E2",
            "WillowGame.GrenadeModPartDefinition",
            vec![],
        );
        // Epsilon: E1's weight for Bolt is the per-element attribute (runtime state for the oracle's subset).
        p.add(
            "GD_RollTest.Grenade.List_E",
            LIST,
            list(vec![
                weighted("GD_RollTest.Grenade.E1", -1, -1, &[(BOLT, 5)], Some(1)),
                w("GD_RollTest.Grenade.E2"),
            ]),
        );
        p.add(
            "GD_RollTest.Grenade.List_D",
            LIST,
            list(vec![
                weighted("GD_RollTest.Grenade.D1", -1, -1, &[(ACME, 2)], Some(4)),
                w("GD_RollTest.Grenade.D2"),
                weighted("GD_RollTest.Grenade.D3", 5, -1, &[], None),
            ]),
        );
        p.add(
            "GD_RollTest.Grenade.Def",
            "WillowGame.GrenadeModDefinition",
            vec![
                ("DeltaParts", r("GD_RollTest.Grenade.List_D")),
                ("EpsilonParts", r("GD_RollTest.Grenade.List_E")),
                ("BaseRarity", konst(5.0)),
            ],
        );
        p.add(
            GRENADE,
            IB,
            vec![
                ("InventoryDefinition", r("GD_RollTest.Grenade.Def")),
                (
                    "Manufacturers",
                    makers(vec![
                        (ACME, vec![grade(1, 100, 1, None, None)]),
                        (BOLT, vec![grade(1, 11, 1, None, Some((0.5, 1.0)))]),
                    ]),
                ),
            ],
        );
        // A balance that does not interpolate its level: the grade's level and own definition.
        p.add(
            "GD_RollTest.Grenade.Def_Fixed",
            "WillowGame.GrenadeModDefinition",
            vec![("DeltaParts", r("GD_RollTest.Grenade.List_D"))],
        );
        p.add(
            FIXED,
            IB,
            vec![
                ("InventoryDefinition", r("GD_RollTest.Grenade.Def")),
                ("bInterpolateExpLevel", V::Bool(false)),
                (
                    "Manufacturers",
                    makers(vec![(
                        ACME,
                        vec![grade(
                            1,
                            100,
                            12,
                            Some("GD_RollTest.Grenade.Def_Fixed"),
                            None,
                        )],
                    )]),
                ),
            ],
        );

        // A balance whose only grade starts at stage 20.
        p.add(
            LATE,
            IB,
            vec![
                ("InventoryDefinition", r("GD_RollTest.Shield.Def")),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(20, 100, 1, None, None)])]),
                ),
            ],
        );

        // Base chains for the Off-mode expansion (NATIVE_LOOT.md: the walk from the balance itself to its bases,
        // stopping at the first step that yields a candidate).
        p.add(
            CHAIN_BASE,
            IB,
            vec![
                ("InventoryDefinition", r("GD_RollTest.Shield.Def")),
                (
                    "Manufacturers",
                    makers(vec![
                        (ACME, vec![grade(1, 100, 3, None, None)]),
                        (BOLT, vec![grade(1, 100, 4, None, None)]),
                    ]),
                ),
            ],
        );
        // The leaf's only grade does not cover stage 8: the base yields, Bolt listed by the base only.
        p.add(
            LEAF_UNCOVERED,
            IB,
            vec![
                ("BaseDefinition", r(CHAIN_BASE)),
                ("InventoryDefinition", r("GD_RollTest.Shield.Def")),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(60, 100, 1, None, None)])]),
                ),
            ],
        );
        // The leaf yields: the base is not consulted.
        p.add(
            LEAF_COVERED,
            IB,
            vec![
                ("BaseDefinition", r(CHAIN_BASE)),
                ("InventoryDefinition", r("GD_RollTest.Shield.Def")),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(1, 100, 1, None, None)])]),
                ),
            ],
        );
        // Non-interpolating base whose second grade covers stage 8, and a non-interpolating leaf with one grade:
        // the base's grade index 1 is out of range in the leaf, so the leaf defers to the base.
        p.add(
            FIXED_BASE,
            IB,
            vec![
                ("InventoryDefinition", r("GD_RollTest.Grenade.Def")),
                ("bInterpolateExpLevel", V::Bool(false)),
                (
                    "Manufacturers",
                    makers(vec![(
                        ACME,
                        vec![
                            grade(60, 100, 2, None, None),
                            grade(1, 50, 7, Some("GD_RollTest.Grenade.Def_Fixed"), None),
                        ],
                    )]),
                ),
            ],
        );
        p.add(
            LEAF_RANGE,
            IB,
            vec![
                ("BaseDefinition", r(FIXED_BASE)),
                ("InventoryDefinition", r("GD_RollTest.Grenade.Def")),
                ("bInterpolateExpLevel", V::Bool(false)),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(60, 100, 9, None, None)])]),
                ),
            ],
        );
        // A non-interpolating leaf over the interpolating base: the leaf yields nothing at stage 8, so the base
        // yields; the leaf lists Bolt (grade 0 in range: the leaf's ExpLevel 5) but not Acme (the interpolating base
        // answers the word unchanged).
        p.add(
            LEAF_FIXED_OVER_INTERP,
            IB,
            vec![
                ("BaseDefinition", r(CHAIN_BASE)),
                ("InventoryDefinition", r("GD_RollTest.Shield.Def")),
                ("bInterpolateExpLevel", V::Bool(false)),
                (
                    "Manufacturers",
                    makers(vec![(BOLT, vec![grade(60, 100, 5, None, None)])]),
                ),
            ],
        );

        // Class mod: two definitions.
        p.add(
            "GD_RollTest.ClassMod.A",
            "WillowGame.ClassModPartDefinition",
            vec![rarity(3.0)],
        );
        p.add(
            "GD_RollTest.ClassMod.List_A",
            LIST,
            list(vec![w("GD_RollTest.ClassMod.A")]),
        );
        for def in ["GD_RollTest.ClassMod.Def1", "GD_RollTest.ClassMod.Def2"] {
            p.add(
                def,
                "WillowGame.ClassModDefinition",
                vec![("AlphaParts", r("GD_RollTest.ClassMod.List_A"))],
            );
        }
        p.add(
            CLASS_MOD,
            "WillowGame.ClassModBalanceDefinition",
            vec![
                (
                    "ClassModDefinitions",
                    V::Array(vec![
                        r("GD_RollTest.ClassMod.Def1"),
                        r("GD_RollTest.ClassMod.Def2"),
                    ]),
                ),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(1, 100, 1, None, None)])]),
                ),
            ],
        );

        // Relic: Effect1 defined; E1 upgrades Effect1 (kept), E2 upgrades Effect2 (removed), E3 upgrades nothing
        // (kept).
        p.add(
            "GD_RollTest.Relic.E1",
            "WillowGame.ArtifactPartDefinition",
            vec![("AttributeSlotUpgrades", slot_rows(&["Effect1"]))],
        );
        p.add(
            "GD_RollTest.Relic.E2",
            "WillowGame.ArtifactPartDefinition",
            vec![("AttributeSlotUpgrades", slot_rows(&["Effect2"]))],
        );
        p.add(
            "GD_RollTest.Relic.E3",
            "WillowGame.ArtifactPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Relic.List_A",
            LIST,
            list(vec![
                w("GD_RollTest.Relic.E1"),
                w("GD_RollTest.Relic.E2"),
                w("GD_RollTest.Relic.E3"),
            ]),
        );
        p.add(
            "GD_RollTest.Relic.Def",
            "WillowGame.ArtifactDefinition",
            vec![
                ("AlphaParts", r("GD_RollTest.Relic.List_A")),
                ("AttributeSlotEffects", slot_rows(&["Effect1"])),
            ],
        );
        p.add(
            RELIC,
            IB,
            vec![
                ("InventoryDefinition", r("GD_RollTest.Relic.Def")),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(1, 100, 1, None, None)])]),
                ),
            ],
        );

        // Weapon: a body pair and an elemental part; a title that needs the Acme manufacturer.
        p.add(
            "GD_RollTest.Names.Title_Acme",
            "WillowGame.WeaponNamePartDefinition",
            vec![
                ("PartName", V::Str("Acme Special")),
                (
                    "Expressions",
                    V::Array(vec![V::Struct(
                        "AttributeExpressionData",
                        vec![
                            ("AttributeOperand1", r("GD_RollTest.Attr.Weapon_Is_Acme")),
                            ("ConstantOperand2", V::Float(1.0)),
                        ],
                    )]),
                ),
            ],
        );
        p.add(
            "GD_RollTest.Weapon.B1",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Weapon.B2",
            "WillowGame.WeaponPartDefinition",
            vec![(
                "TitleList",
                V::Array(vec![r("GD_RollTest.Names.Title_Acme")]),
            )],
        );
        p.add(
            "GD_RollTest.Weapon.E1",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Weapon.X1",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Weapon.X2",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        p.add(
            "GD_RollTest.Weapon.Coll",
            "WillowGame.WeaponPartListCollectionDefinition",
            vec![
                (
                    "BodyPartData",
                    part_data(vec![w("GD_RollTest.Weapon.B1"), w("GD_RollTest.Weapon.B2")]),
                ),
                (
                    "ElementalPartData",
                    part_data(vec![weighted(
                        "GD_RollTest.Weapon.E1",
                        -1,
                        -1,
                        &[(BOLT, 1)],
                        Some(4),
                    )]),
                ),
                (
                    "Accessory1PartData",
                    part_data(vec![
                        weighted("GD_RollTest.Weapon.X1", -1, -1, &[(ACME, 5)], None),
                        w("GD_RollTest.Weapon.X2"),
                    ]),
                ),
                consolidated(),
            ],
        );
        p.add(
            "GD_RollTest.Weapon.Type",
            "WillowGame.WeaponTypeDefinition",
            vec![],
        );
        p.add(
            WEAPON,
            "WillowGame.WeaponBalanceDefinition",
            vec![
                ("InventoryDefinition", r("GD_RollTest.Weapon.Type")),
                (
                    "Manufacturers",
                    makers(vec![(ACME, vec![grade(1, 100, 1, None, None)])]),
                ),
                ("WeaponPartListCollection", r("GD_RollTest.Weapon.Coll")),
            ],
        );
        p
    }

    pub(crate) const FILE: &str = "WillowGame/CookedPCConsole/RollTest.upk";
    const GLOBALS_PATH: &str = crate::rarity::GLOBALS;

    /// A temporary install, removed on drop.
    pub(crate) struct Install(pub(crate) PathBuf);

    impl Install {
        pub(crate) fn new(tag: &str) -> Install {
            Install::at(
                std::env::temp_dir().join(format!("ow-items-roll-{tag}-{}", std::process::id())),
            )
        }

        pub(crate) fn at(dir: PathBuf) -> Install {
            let _ = std::fs::remove_dir_all(&dir);
            let path = dir.join(FILE);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, package().build()).unwrap();
            Install(std::path::absolute(&dir).unwrap())
        }

        /// The census input as the store's census will give it (one file, every export in order).
        pub(crate) fn census(&self) -> CensusInput {
            let mut input = CensusInput::new(self.0.clone());
            let pkg = Package::load(&self.0.join(FILE)).unwrap();
            input.files = vec![pkg.source.clone()];
            for i in 1..=pkg.exports.len() as i32 {
                let path = pkg.path_of(i).unwrap();
                let class = pkg.path_of(pkg.object(i).unwrap().class).unwrap();
                input.paths.push(PathHolders::new(
                    path,
                    vec![Holder::new(pkg.source.clone(), class, i)],
                ));
            }
            input
        }
    }

    impl Drop for Install {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // ---- the world ----

    /// A bare world: a space with the classes the evaluator test reads, a globals object, no store.
    pub(crate) struct TestWorld {
        pub space: ObjectSpace,
        pub globals: ObjRef,
        pub slots: GlobalSlots,
        pub attribute: ClassId,
        pub constant: ClassId,
        pub no_context: ClassId,
    }

    impl TestWorld {
        pub(crate) fn new() -> TestWorld {
            let mut space = ObjectSpace::new();
            let object = space.classes.define("Core.Object", None, vec![]).unwrap();
            let objs = PropKind::Array(Box::new(PropKind::Object { class: object }));
            let chains = vec![
                PropertyDecl::new(space.names.intern("ContextResolverChain"), objs.clone()),
                PropertyDecl::new(space.names.intern("ValueResolverChain"), objs),
            ];
            let base = space
                .classes
                .define("Engine.AttributeDefinitionBase", Some(object), chains)
                .unwrap();
            let attribute = space
                .classes
                .define("Engine.AttributeDefinition", Some(base), vec![])
                .unwrap();
            let decls = vec![PropertyDecl::new(
                space.names.intern("ConstantValue"),
                PropKind::Float,
            )];
            let constant = space
                .classes
                .define(
                    "GearboxFramework.ConstantAttributeValueResolver",
                    Some(object),
                    decls,
                )
                .unwrap();
            let no_context = space
                .classes
                .define(
                    "GearboxFramework.NoContextNeededAttributeContextResolver",
                    Some(object),
                    vec![],
                )
                .unwrap();
            let globals_class = space
                .classes
                .define("WillowGame.WillowGlobals", Some(object), vec![])
                .unwrap();
            let globals = space
                .alloc(globals_class, "WillowGlobals_0", None, Origin::Runtime)
                .unwrap();
            TestWorld {
                space,
                globals,
                slots: GlobalSlots::default(),
                attribute,
                constant,
                no_context,
            }
        }

        /// An attribute whose value is the constant `v` (no context needed).
        pub(crate) fn constant_attribute(&mut self, v: f32) -> ObjRef {
            let k = self
                .space
                .alloc(self.constant, "Constant", None, Origin::Runtime)
                .unwrap();
            let p = self.space.prop(self.constant, "ConstantValue").unwrap();
            self.space
                .write(k, PropAddr::scalar(p), Value::Float(v))
                .unwrap();
            let c = self
                .space
                .alloc(self.no_context, "Context", None, Origin::Runtime)
                .unwrap();
            let a = self
                .space
                .alloc(self.attribute, "Attribute", None, Origin::Runtime)
                .unwrap();
            let arr = |o: ObjRef| Value::array(vec![Value::Object(Some(o))]);
            let p = self
                .space
                .prop(self.attribute, "ContextResolverChain")
                .unwrap();
            self.space.write(a, PropAddr::scalar(p), arr(c)).unwrap();
            let p = self
                .space
                .prop(self.attribute, "ValueResolverChain")
                .unwrap();
            self.space.write(a, PropAddr::scalar(p), arr(k)).unwrap();
            a
        }
    }

    impl AttrWorld for TestWorld {
        fn parts(&mut self) -> WorldParts<'_> {
            WorldParts::new(&mut self.space, None)
        }
        fn globals(&mut self) -> Result<ObjRef, AttrError> {
            Ok(self.globals)
        }
        fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
            Ok(None)
        }
        fn global_slots(&mut self) -> &mut GlobalSlots {
            &mut self.slots
        }
        fn attribute_changed(
            &mut self,
            _: ObjRef,
            _: PropId,
            _: NumForms,
        ) -> Result<(), AttrError> {
            Err(NotYet::not_ported("test world", RECORD).into())
        }
        fn rng(&mut self, _: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
            Err(NotYet::not_ported("test world has no generator", RECORD).into())
        }
    }

    pub(crate) fn snapshot(stage: i32) -> BalanceSnapshot {
        BalanceSnapshot::new(
            Playthrough::NORMAL,
            OpLevel::NONE,
            PlayerCount::SINGLE,
            ExpLevel::new(stage),
            GameStage(stage),
            AwesomeLevel::new(0),
            LevelCapUpgrades::NONE,
        )
        .unwrap()
    }

    /// The fixture: its install, registry and values.
    pub(crate) struct Fx {
        pub _install: Install,
        pub reg: Registry,
        pub data: RollData,
        pub sys: AttributeSystem,
    }

    impl Fx {
        pub(crate) fn new(tag: &str) -> Fx {
            let install = Install::new(tag);
            let input = install.census();
            Fx {
                data: RollData::new(&input).unwrap(),
                reg: build_from_census(input, "Game"),
                _install: install,
                sys: AttributeSystem::standard().unwrap(),
            }
        }

        /// One roll; returns the result and the generator after it.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn roll_with(
            &self,
            world: &mut TestWorld,
            balance: &str,
            stage: i32,
            seed: u32,
            mode: ParityMode,
            known: &[(&str, ObjRef)],
            manufacturer: Option<&str>,
        ) -> (Result<RollResult, ItemError>, CrtRand) {
            let snap = snapshot(stage);
            let mut rng = CrtRand::new(seed);
            let result = {
                let mut rw = RollWorld::new(world, &mut rng, false);
                let mut cx = RollCx::new(&snap, EvalCx::new(&mut rw), &self.sys, &self.data, mode);
                for (p, o) in known {
                    cx = cx.with_known(p, *o);
                }
                if let Some(m) = manufacturer {
                    cx = cx.with_manufacturer(m);
                }
                roll(&self.reg, &ObjectPath::parse(balance).unwrap(), &mut cx)
            };
            (result, rng)
        }

        pub(crate) fn roll(
            &self,
            balance: &str,
            stage: i32,
            seed: u32,
            mode: ParityMode,
        ) -> Result<RollResult, ItemError> {
            self.roll_with(&mut TestWorld::new(), balance, stage, seed, mode, &[], None)
                .0
        }

        pub(crate) fn item(
            &self,
            balance: &str,
            stage: i32,
            seed: u32,
            mode: ParityMode,
        ) -> Rolled {
            match self.roll(balance, stage, seed, mode).unwrap() {
                RollResult::Item(r) => *r,
                RollResult::NotRollable(n) => panic!("not rollable: {n:?}"),
            }
        }
    }

    fn part_of<'r>(r: &'r Rolled, slot: &str) -> Option<&'r str> {
        r.slots
            .iter()
            .find(|s| s.slot == slot)
            .map(|s| s.part.as_str())
    }

    // ---- the tests ----

    #[test]
    fn a_reference_shield_roll_is_reproducible_and_sums_its_rarity() {
        let fx = Fx::new("shield");
        let a = fx.item(SHIELD, 8, 0x9E37_79B1, ParityMode::Reference);
        let b = fx.item(SHIELD, 8, 0x9E37_79B1, ParityMode::Reference);
        assert_eq!(a, b);
        assert_eq!(a.manufacturer.as_deref(), Some(ACME));
        assert_eq!(a.level, 8);
        assert_eq!(part_of(&a, "Material"), Some("GD_RollTest.Shield.M1"));
        // The prefix holds up to level 10, the title comes from the material part.
        assert_eq!(a.prefix.as_deref(), Some("GD_RollTest.Names.Prefix_Low"));
        assert_eq!(a.title.as_deref(), Some("GD_RollTest.Names.Title_Shield"));
        let alpha = if part_of(&a, "Alpha") == Some("GD_RollTest.Shield.A1") {
            1
        } else {
            2
        };
        // base 1 + alpha + material 2 + the title's 1 (the reading of NAME_PARTS_IN_RARITY).
        assert_eq!(a.rarity.level, 1 + alpha + 2 + 1);
        assert!(a.rarity.name_parts_counted);
        assert_eq!(
            a.rarity.band.as_deref(),
            Some(if a.rarity.level <= 6 {
                "Uncommon"
            } else {
                "Rare"
            })
        );
        // Above the prefix's window it is gone.
        let high = fx.item(SHIELD, 30, 0x9E37_79B1, ParityMode::Reference);
        assert_eq!(high.prefix, None);
        // Both Alpha parts come up over a few seeds.
        let seen: Vec<String> = crate::roll::spread_seeds(20)
            .into_iter()
            .map(|s| {
                part_of(&fx.item(SHIELD, 8, s, ParityMode::Reference), "Alpha")
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert!(seen.iter().any(|p| p.ends_with("A1")) && seen.iter().any(|p| p.ends_with("A2")));
    }

    #[test]
    fn the_name_parts_reading_counts_in_both_modes() {
        // Not a Reference stand-in (architecture section 6.1): the same sum in Off mode.
        let fx = Fx::new("names-rarity");
        let off = fx.item(SHIELD, 8, 7, ParityMode::Off);
        assert!(off.rarity.name_parts_counted);
        let alpha = if part_of(&off, "Alpha") == Some("GD_RollTest.Shield.A1") {
            1
        } else {
            2
        };
        assert_eq!(off.rarity.level, 1 + alpha + 2 + 1);
    }

    #[test]
    fn the_reference_roll_draws_the_manufacturer_in_the_balance_roll() {
        let fx = Fx::new("grenade");
        let mut makers = Vec::new();
        for seed in crate::roll::spread_seeds(20) {
            let r = fx.item(GRENADE, 8, seed, ParityMode::Reference);
            // Acme's D1 weighs 5 (its entry), Bolt's takes the default 0, so Bolt never draws D1.
            if r.manufacturer.as_deref() == Some(BOLT) {
                assert_ne!(part_of(&r, "Delta"), Some("GD_RollTest.Grenade.D1"));
            }
            // D3's window starts at the per-element minimum, which the stand-in reads as no bound.
            makers.push(r.manufacturer.unwrap());
        }
        assert!(makers.iter().any(|m| m == ACME) && makers.iter().any(|m| m == BOLT));
        // Bolt's E1 weight needs runtime state: item_roll's note, and the weight taken as 1.
        let bolt = crate::roll::spread_seeds(20)
            .into_iter()
            .map(|s| fx.item(GRENADE, 8, s, ParityMode::Reference))
            .find(|r| r.manufacturer.as_deref() == Some(BOLT))
            .unwrap();
        assert_eq!(
            bolt.notes,
            ["Epsilon: a weight needs runtime state; taken as 1"]
        );
        // D3 is a candidate at stage 8 under Reference (no bound).
        let any_d3 = crate::roll::spread_seeds(40).into_iter().any(|s| {
            part_of(&fx.item(GRENADE, 8, s, ParityMode::Reference), "Delta")
                == Some("GD_RollTest.Grenade.D3")
        });
        assert!(any_d3);
        // At stage 12 Bolt's only grade (1..11) no longer holds: one option, so no manufacturer draw.
        let r = fx.item(GRENADE, 12, 5, ParityMode::Reference);
        assert_eq!(r.manufacturer.as_deref(), Some(ACME));
        // A manufacturer the caller names skips the draw; one that is no option is not rollable.
        let mut world = TestWorld::new();
        let (forced, _) = fx.roll_with(
            &mut world,
            GRENADE,
            8,
            5,
            ParityMode::Reference,
            &[],
            Some(BOLT),
        );
        assert_eq!(
            forced.unwrap().item().unwrap().manufacturer.as_deref(),
            Some(BOLT)
        );
        let (forced, _) = fx.roll_with(
            &mut world,
            GRENADE,
            12,
            5,
            ParityMode::Reference,
            &[],
            Some(BOLT),
        );
        assert!(
            matches!(forced.unwrap(), RollResult::NotRollable(n) if n.reason == "manufacturer_not_an_option")
        );
    }

    #[test]
    fn off_mode_expands_at_the_pool_level_and_draws_once_more() {
        let fx = Fx::new("pool-level");
        // The shield has one manufacturer: Reference draws none, Off mode's candidate pick draws one (the pool
        // always draws), so the generator is one step further after an Off-mode roll.
        let seed = 12_345;
        let mut world = TestWorld::new();
        let (reference, after_reference) = fx.roll_with(
            &mut world,
            SHIELD,
            8,
            seed,
            ParityMode::Reference,
            &[],
            None,
        );
        let (off, after_off) =
            fx.roll_with(&mut world, SHIELD, 8, seed, ParityMode::Off, &[], None);
        let (reference, off) = (reference.unwrap(), off.unwrap());
        let (reference, off) = (reference.item().unwrap(), off.item().unwrap());
        let mut one_more = after_reference.clone();
        one_more.rand15();
        assert_eq!(after_off, one_more);
        assert_eq!(off.mode, ParityMode::Off);
        assert_eq!(off.manufacturer, reference.manufacturer);
        // Off mode caps the stage (playthrough 1: 50); Reference does not.
        let (high, _) = fx.roll_with(&mut world, SHIELD, 80, seed, ParityMode::Off, &[], None);
        let high = high.unwrap();
        assert_eq!(high.item().unwrap().stage, 50);
        assert_eq!(high.item().unwrap().level, 50);
        assert_eq!(fx.item(SHIELD, 80, seed, ParityMode::Reference).level, 80);
        // The grenade's two manufacturers are two candidates of one expansion, drawn by the candidate pick.
        let record = fx.reg.balance(GRENADE).unwrap();
        let found = expand(&fx.data, record, 8, 50, &mut |v| Ok(v.number())).unwrap();
        let makers: Vec<&str> = found.iter().map(|c| c.manufacturer.as_str()).collect();
        assert_eq!(makers, [ACME, BOLT]);
        // Bolt's grade weighs 0.5 + 7/10 * 0.5 at stage 8 (single precision).
        assert_eq!(found[1].weight, 0.5f32 + 7.0f32 / 10.0f32 * 0.5f32);
        // The stage is capped before the windows are tested: at stage 80 with cap 50 Bolt's 1..11 grade does not hold,
        // and the stored stage and level are 50.
        let capped = expand(&fx.data, record, 80, 50, &mut |v| Ok(v.number())).unwrap();
        assert_eq!(
            capped
                .iter()
                .map(|c| c.manufacturer.as_str())
                .collect::<Vec<_>>(),
            [ACME]
        );
        assert_eq!(
            (capped[0].stage, capped[0].level, capped[0].grade_or_level),
            (50, 50, 50)
        );
        // A non-interpolating balance passes the grade index as the word and takes the grade's level and its own
        // definition.
        let fixed = expand(&fx.data, fx.reg.balance(FIXED).unwrap(), 8, 50, &mut |v| {
            Ok(v.number())
        })
        .unwrap();
        assert_eq!(fixed.len(), 1);
        assert_eq!((fixed[0].grade_or_level, fixed[0].level), (0, 12));
        assert_eq!(fixed[0].definition, "GD_RollTest.Grenade.Def_Fixed");
        let reference_fixed = fx.item(FIXED, 8, 3, ParityMode::Reference);
        assert_eq!(reference_fixed.level, 12);
        assert_eq!(reference_fixed.definition, "GD_RollTest.Grenade.Def_Fixed");
    }

    /// The expansion at stage 8, cap 50, with the numbers the registry evaluated.
    fn expand_at_8(fx: &Fx, balance: &str) -> Vec<PoolCandidate> {
        expand(
            &fx.data,
            fx.reg.balance(balance).unwrap(),
            8,
            50,
            &mut |v| Ok(v.number()),
        )
        .unwrap()
    }

    fn summary(found: &[PoolCandidate]) -> Vec<(&str, &str, usize, i32, i32, &str)> {
        found
            .iter()
            .map(|c| {
                (
                    c.step.as_str(),
                    c.manufacturer.as_str(),
                    c.grade,
                    c.grade_or_level,
                    c.level,
                    c.definition.as_str(),
                )
            })
            .collect()
    }

    #[test]
    fn off_mode_walks_the_base_chain_to_the_first_step_that_yields() {
        let fx = Fx::new("chain");
        let shield = "GD_RollTest.Shield.Def";
        // The leaf's grade does not cover stage 8: the base yields both its manufacturers. Bolt is listed only by the
        // base, so its definition is the base's answer; Acme is listed by the leaf, which interpolates and answers
        // its own definition. Both levels are the stage (interpolating).
        assert_eq!(
            summary(&expand_at_8(&fx, LEAF_UNCOVERED)),
            [
                (CHAIN_BASE, ACME, 0, 8, 8, shield),
                (CHAIN_BASE, BOLT, 0, 8, 8, shield)
            ]
        );
        // The leaf yields, so the base (which also lists Bolt) is not consulted.
        assert_eq!(
            summary(&expand_at_8(&fx, LEAF_COVERED)),
            [(LEAF_COVERED, ACME, 0, 8, 8, shield)]
        );
        // Non-interpolating: the base's grade 1 is out of range in the leaf's one-grade list, so both the definition
        // and the level defer to the base (its grade 1: its own definition, ExpLevel 7).
        assert_eq!(
            summary(&expand_at_8(&fx, LEAF_RANGE)),
            [(FIXED_BASE, ACME, 1, 1, 7, "GD_RollTest.Grenade.Def_Fixed")]
        );
    }

    #[test]
    fn an_interpolating_base_answers_the_level_word_unchanged() {
        // GetExpLevelFromManufacturerData hands the question to the base, and a base that interpolates answers the
        // word (here the grade index 0 of a non-interpolating original), not its grade's ExpLevel (3).
        let fx = Fx::new("chain-level");
        assert_eq!(
            summary(&expand_at_8(&fx, LEAF_FIXED_OVER_INTERP)),
            [
                (CHAIN_BASE, ACME, 0, 0, 0, "GD_RollTest.Shield.Def"),
                // The leaf lists Bolt with grade 0 in range: its own ExpLevel 5 and its own definition.
                (CHAIN_BASE, BOLT, 0, 0, 5, "GD_RollTest.Shield.Def"),
            ]
        );
    }

    #[test]
    fn roll_candidate_serves_off_mode_only() {
        let fx = Fx::new("candidate");
        let candidate = expand_at_8(&fx, SHIELD).remove(0);
        let snap = snapshot(8);
        let mut world = TestWorld::new();
        for mode in [ParityMode::Reference, ParityMode::Off] {
            // An unknown balance: Reference is refused first, Off mode answers not rollable.
            let mut rng = CrtRand::new(1);
            let mut rw = RollWorld::new(&mut world, &mut rng, false);
            let mut cx = RollCx::new(&snap, EvalCx::new(&mut rw), &fx.sys, &fx.data, mode);
            let missing = ObjectPath::parse("GD_RollTest.Bal.Missing").unwrap();
            match roll_candidate(&fx.reg, &missing, &candidate, &mut cx) {
                Err(ItemError::WrongMode { call, mode: m }) => {
                    assert_eq!(
                        (call, m, mode),
                        (
                            "roll_candidate",
                            ParityMode::Reference,
                            ParityMode::Reference
                        )
                    )
                }
                Ok(RollResult::NotRollable(n)) => {
                    assert_eq!((n.reason, mode), ("unknown_balance", ParityMode::Off))
                }
                other => panic!("{other:?}"),
            }
            let mut rng = CrtRand::new(1);
            let mut rw = RollWorld::new(&mut world, &mut rng, false);
            let mut cx = RollCx::new(&snap, EvalCx::new(&mut rw), &fx.sys, &fx.data, mode);
            let got = roll_candidate(
                &fx.reg,
                &ObjectPath::parse(SHIELD).unwrap(),
                &candidate,
                &mut cx,
            );
            match mode {
                ParityMode::Reference => assert_eq!(
                    got,
                    Err(ItemError::WrongMode {
                        call: "roll_candidate",
                        mode: ParityMode::Reference
                    })
                ),
                _ => {
                    let r = got.unwrap();
                    let r = r.item().unwrap();
                    assert_eq!(r.mode, ParityMode::Off);
                    assert_eq!(r.manufacturer.as_deref(), Some(ACME));
                }
            }
        }
    }

    #[test]
    fn a_rarity_sum_that_leaves_i64_is_refused() {
        // Two terms just over i64::MAX / 2 each pass trunc_i64 but overflow the sum; Python's bigint sum would not.
        let half = i64::MAX / 2 + 1;
        let terms = vec![
            ("Test.A".to_string(), Some(half)),
            ("Test.B".to_string(), Some(half)),
        ];
        assert!(
            matches!(crate::rarity::sum_terms(&terms), Err(ItemError::BadData(m)) if m.contains("item_roll.rarity"))
        );
        let fine = vec![
            ("Test.A".to_string(), Some(half)),
            ("Test.B".to_string(), None),
            ("Test.C".to_string(), Some(-half)),
        ];
        assert_eq!(crate::rarity::sum_terms(&fine), Ok(0));
    }

    #[test]
    fn a_rarity_term_that_does_not_truncate_is_refused() {
        // math.trunc raises on inf and NaN; the port refuses them instead of an `as` conversion.
        for x in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN, 1e19] {
            assert!(
                matches!(
                    crate::rarity::trunc_i64(x, "test"),
                    Err(ItemError::BadData(_))
                ),
                "{x}"
            );
        }
        assert_eq!(crate::rarity::trunc_i64(-2.9, "test"), Ok(-2));
        assert!(matches!(
            crate::rarity::trunc_i32(3e9, "test"),
            Err(ItemError::BadData(_))
        ));
    }

    #[test]
    fn class_mods_pick_uniformly_only_under_reference() {
        let fx = Fx::new("class-mod");
        let defs: Vec<String> = crate::roll::spread_seeds(20)
            .into_iter()
            .map(|s| fx.item(CLASS_MOD, 8, s, ParityMode::Reference).definition)
            .collect();
        assert!(
            defs.iter().any(|d| d.ends_with("Def1")) && defs.iter().any(|d| d.ends_with("Def2"))
        );
        match fx.roll(CLASS_MOD, 8, 1, ParityMode::Off) {
            Err(ItemError::NotYet(n)) => assert_eq!(n.missing, Missing::NeedsRealGame(6)),
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    #[test]
    fn the_relic_hook_removes_parts_naming_another_slot() {
        let fx = Fx::new("relic");
        for seed in crate::roll::spread_seeds(20) {
            for mode in [ParityMode::Reference, ParityMode::Off] {
                let r = fx.item(RELIC, 8, seed, mode);
                assert_eq!(r.hook_removed, 1);
                assert_ne!(part_of(&r, "Alpha"), Some("GD_RollTest.Relic.E2"));
            }
        }
    }

    #[test]
    fn off_mode_evaluates_a_runtime_window_through_the_evaluator() {
        // Question 7's reading: the per-element minimum (a constant resolver of 20) bounds D3's window in Off mode.
        let fx = Fx::new("window");
        let mut world = TestWorld::new();
        let attribute = world.constant_attribute(20.0);
        let known = [(MIN_STAGE_ATTR, attribute)];
        let mut d3_below = false;
        let mut d3_above = false;
        for seed in crate::roll::spread_seeds(40) {
            let (r, _) = fx.roll_with(&mut world, GRENADE, 8, seed, ParityMode::Off, &known, None);
            d3_below |=
                part_of(r.unwrap().item().unwrap(), "Delta") == Some("GD_RollTest.Grenade.D3");
            let (r, _) = fx.roll_with(&mut world, GRENADE, 30, seed, ParityMode::Off, &known, None);
            d3_above |=
                part_of(r.unwrap().item().unwrap(), "Delta") == Some("GD_RollTest.Grenade.D3");
        }
        assert!(
            !d3_below,
            "below the evaluated minimum the part is no candidate"
        );
        assert!(d3_above);
        // Without a store or a known object, the evaluator's path answers NotYet (no guessed bound).
        match fx.roll(GRENADE, 8, 1, ParityMode::Off) {
            Err(ItemError::NotYet(n)) => assert_eq!(n.missing, Missing::LayerAbsent("ow-defs")),
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    #[test]
    fn a_reference_weapon_roll_names_by_manufacturer() {
        let fx = Fx::new("weapon");
        for seed in crate::roll::spread_seeds(20) {
            let r = fx.item(WEAPON, 15, seed, ParityMode::Reference);
            assert_eq!(r.kind, "weapon");
            assert_eq!(r.manufacturer.as_deref(), Some(ACME));
            assert_eq!(r.definition, "GD_RollTest.Weapon.Type");
            // E1 weighs 100 for Bolt (index 1) and the default 0 (index 4) for Acme: never drawn here.
            assert_eq!(part_of(&r, "Elemental"), None);
            assert!(
                r.notes
                    .iter()
                    .any(|n| n.starts_with("Elemental: every candidate weighs 0"))
            );
            // X1's Acme weight needs runtime state: taken as 1, with weapon_recipe's note.
            assert!(
                r.notes
                    .iter()
                    .any(|n| n == "Accessory1: some weights need runtime attributes; treated as 1")
            );
            // The title needs Weapon_Is_Acme == 1, which the subset reads from the attribute's name.
            let b2 = part_of(&r, "Body") == Some("GD_RollTest.Weapon.B2");
            assert_eq!(
                r.title.as_deref() == Some("GD_RollTest.Names.Title_Acme"),
                b2
            );
        }
        // Off mode evaluates the expression through the evaluator, which needs the item's instance (X1's weight
        // is evaluated through the evaluator too: the attribute is given).
        let mut world = TestWorld::new();
        let attribute = world.constant_attribute(20.0);
        let mut seen_not_yet = false;
        for seed in crate::roll::spread_seeds(10) {
            match fx
                .roll_with(
                    &mut world,
                    WEAPON,
                    15,
                    seed,
                    ParityMode::Off,
                    &[(MIN_STAGE_ATTR, attribute)],
                    None,
                )
                .0
            {
                Err(ItemError::NotYet(n)) => {
                    assert_eq!(n.missing, Missing::NotPorted);
                    seen_not_yet = true;
                }
                Ok(r) => assert_ne!(
                    part_of(r.item().unwrap(), "Body"),
                    Some("GD_RollTest.Weapon.B2")
                ),
                Err(e) => panic!("{e}"),
            }
        }
        assert!(seen_not_yet);
    }

    #[test]
    fn unknown_balances_and_kinds_are_not_rollable() {
        let fx = Fx::new("refusals");
        for mode in [ParityMode::Reference, ParityMode::Off] {
            assert!(matches!(
                fx.roll("GD_RollTest.Bal.Missing", 8, 1, mode).unwrap(),
                RollResult::NotRollable(n) if n.reason == "unknown_balance"
            ));
        }
        // No grade holds stage 8 for the late balance, nor stage 0 for the shield.
        assert!(matches!(
            fx.roll(LATE, 8, 1, ParityMode::Reference).unwrap(),
            RollResult::NotRollable(n) if n.reason == "no_drawable_grade_at_stage"
        ));
        assert!(matches!(
            fx.roll(SHIELD, 0, 1, ParityMode::Reference).unwrap(),
            RollResult::NotRollable(n) if n.reason == "no_drawable_grade_at_stage"
        ));
        assert!(matches!(
            fx.roll(SHIELD, 0, 1, ParityMode::Off).unwrap(),
            RollResult::NotRollable(n) if n.reason == "no_candidate_at_stage"
        ));
    }

    #[test]
    fn the_definition_data_holds_the_roll() {
        let fx = Fx::new("data");
        let r = fx.item(SHIELD, 8, 11, ParityMode::Reference);
        let d = r.definition_data().unwrap();
        assert_eq!(d.kind, ItemKind::Shield);
        assert_eq!(d.manufacturer_grade, 8);
        assert_eq!(d.game_stage, GameStage(8));
        assert_eq!(
            d.parts[ItemSlot::Material.index()]
                .as_ref()
                .map(|p| p.as_str()),
            Some("GD_RollTest.Shield.M1")
        );
        assert_eq!(
            d.title.as_ref().map(|p| p.as_str()),
            Some("GD_RollTest.Names.Title_Shield")
        );
        let pv = r.to_item_pv(11);
        assert_eq!(
            pv.at("parts").at("Material").as_str(),
            Some("GD_RollTest.Shield.M1")
        );
        assert_eq!(pv.at("rarity").at("level"), &Pv::Int(r.rarity.level));
    }

    #[test]
    fn the_roll_world_hands_out_one_generator_and_guards_script_worlds() {
        let mut inner = TestWorld::new();
        let prop = inner
            .space
            .prop(inner.attribute, "ValueResolverChain")
            .unwrap();
        let mut rng = CrtRand::new(3);
        let mut expected = CrtRand::new(3);
        {
            let mut w = RollWorld::new(&mut inner, &mut rng, true);
            let a = w.rng(CrtStream::ItemRoll).unwrap().rand15();
            let b = w.rng(CrtStream::AttributeVariance).unwrap().rand15();
            assert_eq!((a, b), (expected.rand15(), expected.rand15()));
            let globals = w.globals().unwrap();
            assert!(matches!(
                w.attribute_changed(globals, prop, NumForms::default()),
                Err(AttrError::NotYet(n)) if n.missing == Missing::NotPorted
            ));
        }
    }

    #[test]
    fn the_store_census_gives_the_same_rolls() {
        // The definition store's census of the same install (CensusInput::from_store) gives the same registry and
        // values as the hand-built one, so the same rolls.
        let fx = Fx::new("store");
        let mut defs = ow_defs::DefStore::open(
            fx._install.0.clone(),
            ow_defs::identity::IdentityPolicy::FirstInScanOrder,
            ParityMode::Reference,
        )
        .unwrap();
        let census = defs.census().unwrap();
        let input = CensusInput::from_store(census, &fx._install.0);
        let store = Fx {
            data: RollData::new(&input).unwrap(),
            reg: build_from_census(input, "Game"),
            _install: Install(PathBuf::new()),
            sys: AttributeSystem::standard().unwrap(),
        };
        for balance in [SHIELD, GRENADE, FIXED, CLASS_MOD, RELIC, WEAPON, LATE] {
            for seed in spread_seeds(5) {
                assert_eq!(
                    store.roll(balance, 15, seed, ParityMode::Reference),
                    fx.roll(balance, 15, seed, ParityMode::Reference),
                    "{balance} {seed}"
                );
            }
        }
    }

    /// Not a gate: writes the fixture install and the Rust rolls under `$OW_ITEM_ROLL_DIFF_DIR`, so that
    /// `tools/item_roll.py` and `tools/weapon_recipe.py` can roll and check the same packages (lane handoff).
    #[test]
    #[ignore = "writes a fixture for a manual comparison with tools/item_roll.py and tools/weapon_recipe.py"]
    fn write_the_fixture_for_the_tools() {
        let Ok(dir) = std::env::var("OW_ITEM_ROLL_DIFF_DIR") else {
            return;
        };
        let dir = PathBuf::from(dir);
        let install = Install::at(dir.join("Game"));
        let input = install.census();
        let data = RollData::new(&input).unwrap();
        let reg = build_from_census(input, "Game");
        let sys = AttributeSystem::standard().unwrap();
        let mut items = String::new();
        let mut weapons = String::new();
        for balance in [SHIELD, GRENADE, FIXED, CLASS_MOD, RELIC, WEAPON, LATE] {
            for stage in CENSUS_STAGES {
                for seed in spread_seeds(20).into_iter().chain(1..=20) {
                    let snap = snapshot(stage);
                    let mut rng = CrtRand::new(seed);
                    let mut world = TestWorld::new();
                    let mut rw = RollWorld::new(&mut world, &mut rng, false);
                    let mut cx = RollCx::new(
                        &snap,
                        EvalCx::new(&mut rw),
                        &sys,
                        &data,
                        ParityMode::Reference,
                    );
                    let result = roll(&reg, &ObjectPath::parse(balance).unwrap(), &mut cx).unwrap();
                    let line = match &result {
                        RollResult::Item(r) if r.kind == "weapon" => {
                            weapons.push_str(&r.to_weapon_pv(seed, &data).to_json());
                            weapons.push('\n');
                            continue;
                        }
                        RollResult::Item(r) => r.to_item_pv(seed).to_json(),
                        RollResult::NotRollable(n) => format!(
                            "{{\"balance\":\"{balance}\",\"stage\":{stage},\"seed\":{seed},\"not_rollable\":\"{}\"}}",
                            n.reason
                        ),
                    };
                    items.push_str(&line);
                    items.push('\n');
                }
            }
        }
        std::fs::write(dir.join("rust_items.jsonl"), items).unwrap();
        std::fs::write(dir.join("rust_weapons.jsonl"), weapons).unwrap();
        std::mem::forget(install);
    }
}
