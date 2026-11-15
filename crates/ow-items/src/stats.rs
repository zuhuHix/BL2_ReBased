//! Owner: lane W4-STATS. The stats of one rolled item: the attribute values a weapon or an item gets from its
//! definition, its parts and its name parts (architecture section 3.7). The card that presents them is
//! [`crate::cards`].
//!
//! **What the stats are** (weapon: script `WillowWeapon.InitializeInternal` and its helpers, read with
//! `research/script_disasm.py`; NATIVE_WEAPON_RULES.md sections 1 and 12; UNVERIFIED in game unless the record says
//! otherwise):
//!
//! 1. the bases (`CalculateWeaponBaseValues`, then `CalculatePartDependentWeaponBaseValues` once the parts are known);
//! 2. the effects, in this order: the type's `WeaponAttributeEffects`, each part's in slot order, the activated
//!    internal attribute slots, then the prefix's and the title's;
//! 3. each attribute's modifier stack folded with `ow_attr::stack::combine` in the `WeaponGen` form (the sums in
//!    single precision, `(base + PreAdd) * (1 + up) / (1 + |down|) + PostAdd`, no clamp; integer attributes truncate).
//!
//! An activated slot's grade is the type's base grade rounded half up plus every `GradeIncrease` of the type and the
//! parts, clamped maximum first; its value is `f32(f32((grade - 1) * PerGradeUpgrade) + BaseModifierValue)`
//! ([`slot_grade`], [`slot_value`]; read from native code, UNVERIFIED in game).
//!
//! **Two modes** (rule R3; the mode is the caller's):
//!
//! - [`ParityMode::Reference`] reproduces `tools/weapon_stats.evaluate` exactly ([`Values`], [`weapon_attributes`]),
//!   stand-ins included, so `tools/weapon_gen_parity.py`'s comparison can run on Rust output. The values come from
//!   the owners' tag streams ([`RollData`]). The bases are the tool's: the ten type fields of [`TYPE_BASES`] with the
//!   class defaults it restates ([`TYPE_DEFAULTS`], [`WEAPON_DEFAULTS`]) and a spinning barrel's spin-up time, a
//!   subset of the script's (UNVERIFIED as a list; enough for the card). An attribute operand resolves only for
//!   `Weapon_Is_<Maker>`, the weapon level and attributes with a constant resolver subobject
//!   (`weapon_stats.resolver_constant`); anything else leaves the value unresolved, as the tool does. The fold is
//!   `ow-attr`'s.
//! - [`ParityMode::Off`] follows the script functions as read, with every value through `ow-attr` (UNVERIFIED in
//!   game): [`instantiate_stats`] makes the item's instance in the offline space (or [`instantiate_onto`] takes the
//!   caller's, the world's weapon actor), [`weapon_base_values`] and [`weapon_part_base_values`] set the bases the two
//!   script functions set ([`WEAPON_BASE_VALUES`], [`WEAPON_PART_BASE_VALUES`]), and [`apply_weapon_effects`] applies
//!   the effects with `apply_effects`, so the stats are the instance's own attribute stacks, read back with `value_of`.
//!   Items (shield, grenade mod, class mod, relic) carry no stats of their own: their effects reach the owner (the
//!   shield pool, for one), which is the world's object; offline the effect values are evaluated through the evaluator
//!   with the instance as the source, and the owner's stacks folded as above ([`ItemStats`]).
//!
//! **Items** (`tools/prepare_stock_shield.py`, script `WillowItem.InitializeInternal` and
//! `ApplyAllExternalAttributeEffects`; UNVERIFIED in game): the definition's `ExternalAttributeEffects`, then every
//! part's and the name parts'; then the external slots that are not `bRunEffectsAsSkill`, graded over the
//! definition's, the parts' and the name parts' upgrades, at most `AttributeSlotMaxActivated` of them activated
//! (class default 19). The shield pool's base is 0 for its three attributes (`ShieldPool` names none; read from
//! data, UNVERIFIED in game).

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, EffectData, EvalCx, InitData};
use ow_attr::stack::{CombineForm, Combined, Modifier, ModifierId, ModifierType, NumKind, combine};
use ow_core::num::{f32r, pow_f64};
use ow_core::{DetMap, DetSet, NotYet, ObjectPath, ParityMode};
use ow_defs::overlay::{Overlay, PatchValue};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use crate::ItemError;
use crate::data::{DefinitionData, ItemKind, ItemSlot, WeaponSlot};
use crate::merge::{Props, RollData};
use crate::registry::{ITEM_SLOTS, Pv, WEAPON_SLOTS};
use crate::roll::Rolled;

/// The record a `NotYet` of the stats names.
pub const RECORD: &str = "architecture section 3.7 (ow-items stats)";

/// The weapon attributes' package (`D_Attributes.Weapon`).
pub const ATTR: &str = "D_Attributes.Weapon.";

/// `weapon_stats.TYPE_BASES`, the Reference bases: a `WeaponTypeDefinition` property and the attribute it initializes
/// (the attribute's `ObjectPropertyAttributeValueResolver.PropertyName` names the `WillowWeapon` property). Ten of the
/// script's 22 type fields ([`WEAPON_BASE_VALUES`]); the list is the tool's, UNVERIFIED as the game's set. The script
/// confirms `FireRate` into `FireInterval`.
pub const TYPE_BASES: [(&str, &str); 10] = [
    ("InstantHitDamage", "WeaponDamage"),
    ("FireRate", "WeaponFireInterval"),
    ("ClipSize", "WeaponClipSize"),
    ("ReloadTime", "WeaponReloadSpeed"),
    ("Spread", "WeaponSpread"),
    ("ProjectilesPerShot", "WeaponProjectilesPerShot"),
    ("StatusEffectDamage", "WeaponStatusEffectDamage"),
    (
        "BaseStatusEffectChanceModifier",
        "WeaponBaseStatusEffectChanceModifier",
    ),
    ("AutomaticBurstCount", "WeaponAutomaticBurstCount"),
    ("BurstInterval", "WeaponBurstInterval"),
];

/// A class default a cooked weapon type leaves out (`weapon_stats.TYPE_DEFAULTS`, the values of
/// `Default__WeaponTypeDefinition`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TypeDefault {
    Float(f64),
    Int(i64),
    /// An `AttributeInitializationData` with this `BaseValueConstant`.
    Constant(f64),
}

/// `weapon_stats.TYPE_DEFAULTS`.
pub const TYPE_DEFAULTS: [(&str, TypeDefault); 6] = [
    ("FireRate", TypeDefault::Float(0.4)),
    ("ReloadTime", TypeDefault::Float(2.1)),
    ("Spread", TypeDefault::Float(0.01)),
    ("ProjectilesPerShot", TypeDefault::Int(1)),
    ("BaseStatusEffectChanceModifier", TypeDefault::Constant(1.0)),
    ("AutomaticBurstCount", TypeDefault::Int(0)),
];

/// `weapon_stats.WEAPON_DEFAULTS`: `WillowWeapon` attribute bases no weapon type sets (the game's `obj dump` of
/// `Default__WillowWeapon`, which the tag walker does not decode).
pub const WEAPON_DEFAULTS: [(&str, f64); 3] = [
    ("WeaponShotCost", 1.0),
    ("WeaponStatusEffectChanceModifier", 1.0),
    ("WeaponProjectileSpeedMultiplier", 1.0),
];

/// `weapon_stats.INT_ATTRIBUTES`: the `IntAttributeProperty` attributes, whose fold truncates toward zero.
pub const INT_ATTRIBUTES: [&str; 4] = [
    "WeaponClipSize",
    "WeaponProjectilesPerShot",
    "WeaponShotCost",
    "WeaponAutomaticBurstCount",
];

/// The part price total attribute (`weapon_stats.PART_PRICE_TOTAL`).
pub const PART_PRICE_TOTAL: &str = "D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal";
/// The weapon level attribute.
pub const WEAPON_LEVEL: &str = "D_Attributes.Weapon.WeaponLevel";
/// The player's accuracy pool (`weapon_firing.ACCURACY_POOL`).
pub const ACCURACY_POOL: &str = "D_Resourcepools.PlayerPools.AccuracyPool";
/// `weapon_firing.ACCURACY_ATTRIBUTES`: the pool attributes a weapon's external effects modify, by card key.
pub const ACCURACY_ATTRIBUTES: [(&str, &str); 3] = [
    ("D_Attributes.AccuracyResourcePool.AccuracyMinValue", "min"),
    ("D_Attributes.AccuracyResourcePool.AccuracyMaxValue", "max"),
    (
        "D_Attributes.AccuracyResourcePool.AccuracyOnIdleRegenerationRate",
        "idle_rate",
    ),
];
/// The shield pool's three attributes (`prepare_stock_shield.STATS`), by card label.
pub const SHIELD_STATS: [(&str, &str); 3] = [
    ("Capacity", "D_Attributes.ShieldResourcePool.ShieldMaxValue"),
    (
        "Recharge Rate",
        "D_Attributes.ShieldResourcePool.ShieldOnIdleRegenerationRate",
    ),
    (
        "Recharge Delay",
        "D_Attributes.ShieldResourcePool.ShieldOnIdleRegenerationDelay",
    ),
];
/// The shield pool's base for its three attributes (`prepare_stock_shield.POOL_BASE`: `ShieldPool` names none;
/// read from data, UNVERIFIED in game).
pub const SHIELD_POOL_BASE: f32 = 0.0;
/// `AttributeSlotMaxActivated`'s class default (NATIVE_WEAPON_RULES.md section 1; UNVERIFIED in game).
pub const MAX_SLOTS_DEFAULT: i64 = 19;

/// How deep the oracle's reading follows init data into definitions before it gives up (the tool's recursion limit;
/// the same cut-off as the registry's, UNVERIFIED).
const MAX_DEPTH: u32 = 200;

fn raises(what: impl std::fmt::Display) -> ItemError {
    ItemError::BadData(format!("{what} (the reference tool raises here)"))
}

/// The last component of a dotted path.
pub fn short(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

// ---- the recipe ----------------------------------------------------------------------------------------------------

/// What a weapon's stats are evaluated from (`weapon_stats.evaluate`'s recipe): the weapon type, the part chosen per
/// slot in slot order, the name parts, the manufacturer and the balance (the level requirement's mission rule and
/// the card text read it).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WeaponRecipe {
    pub balance: Option<String>,
    pub manufacturer: Option<String>,
    pub weapon_type: String,
    /// `(slot, part)` in the game's slot order; a slot without a part is left out.
    pub parts: Vec<(String, String)>,
    pub prefix: Option<String>,
    pub title: Option<String>,
}

impl WeaponRecipe {
    pub fn new(weapon_type: &str) -> Self {
        WeaponRecipe {
            weapon_type: weapon_type.to_string(),
            ..Self::default()
        }
    }

    pub fn with_balance(mut self, balance: &str) -> Self {
        self.balance = Some(balance.to_string());
        self
    }

    pub fn with_manufacturer(mut self, manufacturer: &str) -> Self {
        self.manufacturer = Some(manufacturer.to_string());
        self
    }

    /// Adds `part` in `slot` (kept in [`WEAPON_SLOTS`] order).
    pub fn with_part(mut self, slot: &str, part: &str) -> Self {
        self.parts.retain(|(s, _)| s != slot);
        self.parts.push((slot.to_string(), part.to_string()));
        let rank = |s: &str| {
            WEAPON_SLOTS
                .iter()
                .position(|w| *w == s)
                .unwrap_or(usize::MAX)
        };
        self.parts.sort_by_key(|(s, _)| rank(s));
        self
    }

    pub fn with_names(mut self, prefix: Option<&str>, title: Option<&str>) -> Self {
        self.prefix = prefix.map(str::to_string);
        self.title = title.map(str::to_string);
        self
    }

    /// A weapon roll's recipe.
    pub fn from_rolled(r: &Rolled) -> Self {
        WeaponRecipe {
            balance: Some(r.balance.clone()),
            manufacturer: r.manufacturer.clone(),
            weapon_type: r.definition.clone(),
            parts: r
                .slots
                .iter()
                .map(|s| (s.slot.clone(), s.part.clone()))
                .collect(),
            prefix: r.prefix.clone(),
            title: r.title.clone(),
        }
    }

    /// A weapon's definition data as a recipe (the slots by [`WeaponSlot`] index).
    pub fn from_definition_data(d: &DefinitionData) -> Result<Self, ItemError> {
        if d.kind != ItemKind::Weapon {
            return Err(ItemError::BadData(format!(
                "a {:?} definition data is not a weapon",
                d.kind
            )));
        }
        let mut parts = Vec::new();
        for (i, slot) in WEAPON_SLOTS.iter().enumerate() {
            let Some(s) = WeaponSlot::from_index(i) else {
                continue;
            };
            if let Some(p) = d
                .weapon_part(s)
                .map_err(|e| ItemError::BadData(e.to_string()))?
            {
                parts.push((slot.to_string(), p.as_str().to_string()));
            }
        }
        let text = |p: &Option<ObjectPath>| p.as_ref().map(|p| p.as_str().to_string());
        Ok(WeaponRecipe {
            balance: text(&d.balance),
            manufacturer: text(&d.manufacturer),
            weapon_type: d.definition.as_str().to_string(),
            parts,
            prefix: text(&d.prefix),
            title: text(&d.title),
        })
    }

    /// `weapon_stats.ordered_parts`: the part paths in slot order.
    pub fn ordered_parts(&self) -> Vec<&str> {
        let rank = |s: &str| WEAPON_SLOTS.iter().position(|w| *w == s);
        let mut parts: Vec<(usize, &str)> = self
            .parts
            .iter()
            .filter(|(_, p)| !p.is_empty())
            .filter_map(|(s, p)| rank(s).map(|r| (r, p.as_str())))
            .collect();
        parts.sort_by_key(|(r, _)| *r);
        parts.into_iter().map(|(_, p)| p).collect()
    }

    /// The part in `slot`, if any.
    pub fn part(&self, slot: &str) -> Option<&str> {
        self.parts
            .iter()
            .find(|(s, _)| s == slot)
            .map(|(_, p)| p.as_str())
            .filter(|p| !p.is_empty())
    }

    /// The prefix, then the title, those present.
    pub fn name_parts(&self) -> Vec<&str> {
        [self.prefix.as_deref(), self.title.as_deref()]
            .into_iter()
            .flatten()
            .filter(|p| !p.is_empty())
            .collect()
    }

    /// The manufacturer's short name (`Weapon_Is_<Maker>`'s maker), empty without one.
    pub fn maker(&self) -> &str {
        self.manufacturer.as_deref().map_or("", short)
    }
}

// ---- the oracle's value reading (Reference) ------------------------------------------------------------------------

/// An object's decoded properties as the stats read them: the owner's tag stream, with a live overlay's values
/// over it when one is given.
pub type Obj = Rc<Pv>;

/// The tag-stream values the stats read ([`RollData`]) with the lookups `weapon_stats` builds over them, and an
/// optional live overlay (`tools/real_game/live_overlay.LivePackage`: a captured property replaces the decoded one).
/// The overlay patches the objects the stats read themselves (types, parts, name parts, presentations); the
/// definitions `weapon_recipe.attribute_value` follows are read unpatched (a capture of `weapon_dump.py` holds
/// weapon parts, types and name parts only).
pub struct StatsData<'a> {
    data: &'a RollData,
    constants: RefCell<DetMap<String, Option<Pv>>>,
    overlay: DetMap<String, Vec<(String, Pv)>>,
    objects: RefCell<DetMap<String, Obj>>,
}

impl std::fmt::Debug for StatsData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatsData").finish_non_exhaustive()
    }
}

/// A live capture's value in the decoded model (`json.loads`): a number without `.` or an exponent is an `int`.
pub fn patch_value(v: &PatchValue) -> Pv {
    match v {
        PatchValue::Null => Pv::Null,
        PatchValue::Bool(b) => Pv::Bool(*b),
        PatchValue::Number(n) => {
            let float = n.contains(['.', 'e', 'E']) || n.contains("NaN") || n.contains("Infinity");
            match (float, n.parse::<i64>()) {
                (false, Ok(i)) => Pv::Int(i),
                _ => match n.as_str() {
                    "NaN" => Pv::Float(f64::NAN),
                    "Infinity" => Pv::Float(f64::INFINITY),
                    "-Infinity" => Pv::Float(f64::NEG_INFINITY),
                    other => other.parse::<f64>().map_or(Pv::Null, Pv::Float),
                },
            }
        }
        PatchValue::Text(t) => Pv::Str(t.clone()),
        PatchValue::List(items) => Pv::List(items.iter().map(patch_value).collect()),
        PatchValue::Fields(fields) => Pv::Map(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), patch_value(v)))
                .collect(),
        ),
        _ => Pv::Null,
    }
}

impl<'a> StatsData<'a> {
    pub fn new(data: &'a RollData) -> Self {
        StatsData {
            data,
            constants: RefCell::new(DetMap::new()),
            overlay: DetMap::new(),
            objects: RefCell::new(DetMap::new()),
        }
    }

    /// The same reading with a live capture's values over the decoded ones (`LivePackage.props`).
    pub fn with_overlay(mut self, overlay: &Overlay) -> Self {
        for patch in &overlay.patches {
            self.overlay
                .entry(patch.object.as_str().to_string())
                .or_default()
                .push((patch.property.clone(), patch_value(&patch.value)));
        }
        self.objects.borrow_mut().clear();
        self
    }

    pub fn data(&self) -> &'a RollData {
        self.data
    }

    /// Whether a package holds `path`.
    pub fn holds(&self, path: &str) -> bool {
        self.data.holds(path)
    }

    /// `package.props(path)`: the tool raises for a path no package holds or that does not decode.
    pub fn props(&self, path: &str) -> Result<Obj, ItemError> {
        if let Some(o) = self.objects.borrow().get(path) {
            return Ok(Rc::clone(o));
        }
        let decoded: Props = self.data.props(path).map_err(raises)?;
        let mut value: Pv = (*decoded).clone();
        if let (Some(rows), Pv::Map(m)) = (self.overlay.get(path), &mut value) {
            for (key, live) in rows {
                let empty = match live {
                    Pv::Null => true,
                    Pv::List(v) => v.is_empty(),
                    Pv::Map(v) => v.is_empty(),
                    _ => false,
                };
                if empty && !m.contains_key(key) {
                    continue;
                }
                m.insert(key.clone(), live.clone());
            }
        }
        let o = Rc::new(value);
        self.objects
            .borrow_mut()
            .insert(path.to_string(), Rc::clone(&o));
        Ok(o)
    }

    /// `weapon_stats._optional_props`: `None` for a path no package holds; a path that does not decode raises.
    pub fn optional(&self, path: &str) -> Result<Option<Obj>, ItemError> {
        if path.is_empty() || !self.data.holds(path) {
            return Ok(None);
        }
        self.props(path).map(Some)
    }

    /// `package.extra_names[path][name]`: every element of a static array, from the owner's tag stream as the
    /// registry decoded it (a live overlay does not patch it, as `LivePackage` does not); raises as
    /// [`StatsData::props`].
    pub fn statics(&self, path: &str, name: &str) -> Result<Vec<Pv>, ItemError> {
        let decoded: Props = self.data.props(path).map_err(raises)?;
        Ok(decoded.statics(name).to_vec())
    }

    /// `package.props(path).get(key)` (raises as [`StatsData::props`]).
    pub fn prop(&self, path: &str, key: &str) -> Result<Pv, ItemError> {
        Ok(self.props(path)?.at(key).clone())
    }

    /// `weapon_stats.resolver_constant`: the `ConstantValue` of the attribute's own subobject named
    /// `ConstantAttributeValueResolver_*` (the first in path order), or `None`. Unlike the registry's
    /// `attribute_constant` it does not look at the attribute's `ValueResolverChain`, as the tool does not.
    pub fn resolver_constant(&self, attribute: &str) -> Result<Option<Pv>, ItemError> {
        if let Some(v) = self.constants.borrow().get(attribute) {
            return Ok(v.clone());
        }
        let resolver = self
            .data
            .below_of_class(attribute, "")
            .into_iter()
            .find(|p| {
                p.strip_prefix(attribute)
                    .and_then(|rest| rest.strip_prefix('.'))
                    .is_some_and(|name| {
                        !name.contains('.') && name.starts_with("ConstantAttributeValueResolver_")
                    })
            });
        let value = match resolver {
            None => None,
            Some(p) => {
                let v = self.props(&p)?.at("ConstantValue").clone();
                if v == Pv::Null { None } else { Some(v) }
            }
        };
        self.constants
            .borrow_mut()
            .insert(attribute.to_string(), value.clone());
        Ok(value)
    }
}

/// `weapon_stats.evaluate`'s `value` closure: an `AttributeInitializationData` through `weapon_recipe.attribute_value`,
/// retried after each attribute operand it resolves (`Weapon_Is_<Maker>`: 1 for the weapon's manufacturer, else 0;
/// an attribute with a constant resolver: the constant). An operand it cannot resolve is kept in `unresolved` and the
/// value is `None`. The resolved operands persist for the evaluation, as the tool's `known` dict does.
pub struct Values<'s, 'a> {
    stats: &'s StatsData<'a>,
    maker: String,
    known: Vec<(String, f64)>,
    pub unresolved: DetSet<String>,
}

impl<'s, 'a> Values<'s, 'a> {
    /// The reading for one weapon of `maker` (its short name) at `level`.
    pub fn new(stats: &'s StatsData<'a>, maker: &str, level: i64) -> Self {
        Values {
            stats,
            maker: maker.to_string(),
            // The tool's `float(level)`: an item level is far below 2^53, so the conversion is exact.
            known: vec![(WEAPON_LEVEL.to_string(), level as f64)],
            unresolved: DetSet::new(),
        }
    }

    pub fn stats(&self) -> &'s StatsData<'a> {
        self.stats
    }

    /// The value of `init` (module documentation), `Ok(None)` when it needs what the tool does not resolve.
    pub fn value(&mut self, init: &Pv) -> Result<Option<f64>, ItemError> {
        let empty = Pv::Map(DetMap::new());
        let init = match init {
            Pv::Map(_) => init,
            v if !v.truthy() => &empty,
            other => {
                return Err(raises(format!(
                    "an init that is not a struct: {}",
                    other.to_json()
                )));
            }
        };
        loop {
            match self.stats.data.attribute_value(init, &self.known) {
                Ok(Some(v)) => return Ok(Some(v)),
                Ok(None) => {}
                Err(_) => return Err(raises("an init the oracle's evaluator cannot read")),
            }
            let Some(missing) = self.find_missing(init, 0)? else {
                return Ok(None);
            };
            if let Some((_, maker)) = missing.rsplit_once("Weapon_Is_")
                && missing.contains(".Weapon_Is_")
            {
                let v = if maker == self.maker { 1.0 } else { 0.0 };
                self.known.push((missing, v));
                continue;
            }
            match self.stats.resolver_constant(&missing)? {
                None => {
                    self.unresolved.insert(missing);
                    return Ok(None);
                }
                Some(c) => {
                    let Some(v) = c.number() else {
                        return Err(raises(format!("{missing}'s constant is not a number")));
                    };
                    self.known.push((missing, v));
                }
            }
        }
    }

    /// `weapon_stats.evaluate.find_missing`: the first base attribute with no value, the init's own first, then its
    /// definition's formula terms and range bounds in the order the struct declares them (the tool walks the decoded
    /// dict in tag order).
    fn find_missing(&self, init: &Pv, depth: u32) -> Result<Option<String>, ItemError> {
        if depth > MAX_DEPTH {
            return Err(raises("init data nested too deep"));
        }
        let attribute = init.at("BaseValueAttribute");
        if attribute.truthy() {
            let Some(a) = attribute.as_str() else {
                return Err(raises("a BaseValueAttribute that is not a path"));
            };
            if !self.known.iter().any(|(k, _)| k == a) {
                return Ok(Some(a.to_string()));
            }
        }
        let definition = init.at("InitializationDefinition");
        if !definition.truthy() {
            return Ok(None);
        }
        let Some(path) = definition.as_str() else {
            return Err(raises("an InitializationDefinition that is not a path"));
        };
        let props = self.stats.props(path)?;
        for (group, keys) in [
            (
                "ValueFormula",
                &["Multiplier", "Level", "Power", "Offset"][..],
            ),
            ("RangeRestriction", &["MinValue", "MaxValue"][..]),
        ] {
            let block = props.at(group);
            let mut terms: Vec<&Pv> = keys.iter().filter_map(|k| block.get(k)).collect();
            // A field the declaration order above does not name (none is known) comes after, by name.
            if let Pv::Map(m) = block {
                terms.extend(
                    m.iter()
                        .filter(|(k, _)| !keys.contains(&k.as_str()))
                        .map(|(_, v)| v),
                );
            }
            for term in terms {
                if matches!(term, Pv::Map(_))
                    && let Some(found) = self.find_missing(term, depth + 1)?
                {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }
}

// ---- the modifier stacks ---------------------------------------------------------------------------------------------

/// One attribute's modifiers: the tool's single-precision sums (`MT_PreAdd`, `MT_Scale`, `MT_PostAdd`, the positive
/// and the non-positive scales) and the modifiers in stack order, which `ow_attr::stack::combine` folds.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bucket {
    pub pre_add: f64,
    pub scale: f64,
    pub post_add: f64,
    pub scale_up: f64,
    pub scale_down: f64,
    pub modifiers: Vec<Modifier>,
}

/// The modifier stacks by attribute path, in the tool's add order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stacks(pub DetMap<String, Bucket>);

/// `MT_PreAdd`, `MT_Scale`, `MT_PostAdd` as `ow-attr`'s modifier type.
pub fn modifier_type(kind: &str) -> Option<ModifierType> {
    match kind {
        "MT_Scale" => Some(ModifierType::Scale),
        "MT_PreAdd" => Some(ModifierType::PreAdd),
        "MT_PostAdd" => Some(ModifierType::PostAdd),
        _ => None,
    }
}

impl Stacks {
    /// `weapon_stats.evaluate.add`: the amount in single precision onto its sum; a scale also onto the positive or
    /// the non-positive side (as its magnitude). A kind the enum does not have is kept in no sum the fold reads.
    pub fn add(&mut self, attribute: &str, kind: &str, amount: f64) {
        let amount = f32r(amount);
        let b = self.0.entry(attribute.to_string()).or_default();
        match kind {
            "MT_PreAdd" => b.pre_add = f32r(b.pre_add + amount),
            "MT_PostAdd" => b.post_add = f32r(b.post_add + amount),
            "MT_Scale" => {
                b.scale = f32r(b.scale + amount);
                if amount > 0.0 {
                    b.scale_up = f32r(b.scale_up + amount);
                } else {
                    b.scale_down = f32r(b.scale_down + amount.abs());
                }
            }
            _ => {}
        }
        if let Some(kind) = modifier_type(kind) {
            // The id only tells modifiers apart in a stack; a fold reads type and value. `amount` is a float value
            // (f32r above), so the cast is exact.
            let id = ModifierId(b.modifiers.len() as u64);
            b.modifiers.push(Modifier {
                kind,
                value: amount as f32,
                id,
            });
        }
    }

    pub fn get(&self, attribute: &str) -> Option<&Bucket> {
        self.0.get(attribute)
    }

    /// `weapon_stats.combine(start, mods, integer)` through `ow_attr::stack::combine` (`WeaponGen` form): the value
    /// of `attribute` from its base `start` (a single-precision value).
    pub fn fold(&self, attribute: &str, start: f64, integer: bool) -> f64 {
        let mods = self.0.get(attribute).map_or(&[][..], |b| &b.modifiers[..]);
        // `start` is a single-precision value (every base enters through f32r), so the cast is exact.
        let kind = if integer {
            NumKind::Int
        } else {
            NumKind::Float
        };
        match combine(start as f32, mods, CombineForm::WeaponGen, kind) {
            Combined::Float(x) => f64::from(x),
            Combined::Int(i) => f64::from(i),
            Combined::Byte(b) => f64::from(b),
        }
    }
}

/// `weapon_stats.slot_grade`: the base grade rounded half up plus the increases, clamped maximum first, then minimum
/// (each when enforced).
pub fn slot_grade(base_grade: Option<f64>, increases: f64, effect: &Pv) -> f64 {
    let mut grade = (base_grade.unwrap_or(1.0) + 0.5).floor() + increases;
    let bound = |k: &str| effect.at(k).number().unwrap_or(0.0);
    if effect.at("bEnforceMaximumGrade").truthy() && grade > bound("MaximumGrade") {
        grade = bound("MaximumGrade");
    }
    if effect.at("bEnforceMinimumGrade").truthy() && grade < bound("MinimumGrade") {
        grade = bound("MinimumGrade");
    }
    grade
}

/// `weapon_stats.slot_value`: `f32(f32((grade - 1) * PerGradeUpgrade) + BaseModifierValue)`.
pub fn slot_value(grade: f64, base_modifier: f64, per_grade: f64) -> f64 {
    f32r(f32r((grade - 1.0) * per_grade) + base_modifier)
}

/// `weapon_recipe.formula_value`: `Multiplier * (Level ^ Power + Offset)`, each step in single precision, the power
/// skipped at 1. `Err` where Python's `math.pow` raises.
fn formula_value(m: f64, l: f64, p: f64, o: f64) -> Result<f64, ItemError> {
    let raised = if p == 1.0 {
        l
    } else {
        if l == 0.0 && p < 0.0 {
            return Err(raises("0 to a negative power"));
        }
        if l < 0.0 && p.fract() != 0.0 {
            return Err(raises("a negative level to a fractional power"));
        }
        let r = pow_f64(l, p);
        if r.is_infinite() && l.is_finite() && p.is_finite() {
            return Err(raises("a power that overflows"));
        }
        f32r(r)
    };
    Ok(f32r(m * f32r(raised + o)))
}

/// `weapon_stats.price_init`: an init for the price calculators and the accuracy pool. Like `attribute_value`, but a
/// base attribute resolves from `known` or its constant resolver, and range, rounding and conditionals are not
/// applied (none occurs in those chains). An operand it cannot resolve goes into `unresolved`.
pub fn price_init(
    stats: &StatsData<'_>,
    init: &Pv,
    known: &[(String, f64)],
    unresolved: &mut DetSet<String>,
    depth: u32,
) -> Result<Option<f64>, ItemError> {
    if depth > MAX_DEPTH {
        return Err(raises("price init data nested too deep"));
    }
    let empty = Pv::Map(DetMap::new());
    let init = if init.truthy() { init } else { &empty };
    let attribute = init.at("BaseValueAttribute");
    let definition = init.at("InitializationDefinition");
    let constant = init.at("BaseValueConstant");
    let mut base = if constant.truthy() {
        constant
            .number()
            .ok_or_else(|| raises("a BaseValueConstant that is not a number"))?
    } else {
        0.0
    };
    if attribute.truthy() {
        let a = attribute
            .as_str()
            .ok_or_else(|| raises("an attribute that is not a path"))?;
        let found = match known.iter().find(|(k, _)| k == a) {
            Some((_, v)) => Some(*v),
            None => match stats.resolver_constant(a)? {
                Some(c) => Some(
                    c.number()
                        .ok_or_else(|| raises("a constant that is not a number"))?,
                ),
                None => None,
            },
        };
        match found {
            Some(v) => base = v,
            None => {
                unresolved.insert(a.to_string());
                return Ok(None);
            }
        }
    }
    if definition.truthy() {
        let path = definition
            .as_str()
            .ok_or_else(|| raises("a definition that is not a path"))?;
        let props = stats.props(path)?;
        let mode = props.at("BaseValueMode");
        let mode = if mode.truthy() {
            mode.as_str().unwrap_or("").to_string()
        } else {
            "BASEVALUE_InitializationDefSetsBaseValue".to_string()
        };
        let formula = props.at("ValueFormula");
        if formula.at("bEnabled").truthy() {
            let mut terms = [0.0; 4];
            let mut missing = false;
            for (slot, key) in terms
                .iter_mut()
                .zip(["Multiplier", "Level", "Power", "Offset"])
            {
                match price_init(stats, formula.at(key), known, unresolved, depth + 1)? {
                    Some(v) => *slot = v,
                    None => missing = true,
                }
            }
            if missing {
                return Ok(None);
            }
            let value = formula_value(terms[0], terms[1], terms[2], terms[3])?;
            let b = f32r(base);
            base = f32r(match mode.as_str() {
                "BASEVALUE_InitializationDefSetsBaseValue" => value,
                "BASEVALUE_InitializationDefAddsToBaseValue" => b + value,
                "BASEVALUE_InitializationDefScalesBaseValue" => b * value,
                "BASEVALUE_InitializationDefOffsetByBaseValue" => value - b,
                _ => {
                    unresolved.insert(format!("{path}: {mode}"));
                    return Ok(None);
                }
            });
        }
    }
    let scale = match init.get("BaseValueScaleConstant") {
        None | Some(Pv::Null) => 1.0,
        Some(s) => s
            .number()
            .ok_or_else(|| raises("a scale that is not a number"))?,
    };
    Ok(Some(f32r(base * scale)))
}

// ---- the weapon's attributes -------------------------------------------------------------------------------------

/// A weapon's evaluated attributes (`weapon_stats.evaluate`'s attribute half).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponAttributes {
    /// Every base with a value, by attribute path.
    pub base: DetMap<String, f64>,
    pub stacks: Stacks,
    /// The folded value of every attribute with a base (`final`), by path.
    pub values: DetMap<String, f64>,
    /// The slot grades the type and the parts add, by slot name.
    pub grades: DetMap<String, f64>,
    /// The barrel part, its spin flag and the decoded barrel (empty without a barrel).
    pub barrel: Option<String>,
    pub spinning: bool,
}

impl WeaponAttributes {
    /// The value of `D_Attributes.Weapon.<name>`.
    pub fn stat(&self, name: &str) -> Option<f64> {
        self.values.get(&format!("{ATTR}{name}")).copied()
    }
}

fn effect_kind(effect: &Pv) -> Result<&str, ItemError> {
    effect
        .get("ModifierType")
        .ok_or_else(|| raises("an effect without a ModifierType"))?
        .as_str()
        .ok_or_else(|| raises("a ModifierType that is not a name"))
}

fn effect_attribute(effect: &Pv) -> Result<&str, ItemError> {
    match effect.get("AttributeToModify") {
        None => Err(raises("an effect without an AttributeToModify")),
        Some(Pv::Str(s)) => Ok(s),
        // A None attribute is a key of its own in the tool's dict; no attribute is named "".
        Some(Pv::Null) => Ok(""),
        Some(_) => Err(raises("an AttributeToModify that is not a path")),
    }
}

/// `weapon_stats.evaluate` up to `final` (Reference; module documentation).
pub fn weapon_attributes(
    v: &mut Values<'_, '_>,
    recipe: &WeaponRecipe,
) -> Result<WeaponAttributes, ItemError> {
    let stats = v.stats;
    let weapon_type = stats.props(&recipe.weapon_type)?;
    let mut base: DetMap<String, Option<f64>> = DetMap::new();
    for (field, name) in TYPE_BASES {
        let raw = match weapon_type.get(field) {
            Some(raw) => Some(raw.clone()),
            None => TYPE_DEFAULTS
                .iter()
                .find(|(f, _)| *f == field)
                .map(|(_, d)| match d {
                    TypeDefault::Float(x) => Pv::Float(*x),
                    TypeDefault::Int(i) => Pv::Int(*i),
                    TypeDefault::Constant(c) => {
                        let mut m = DetMap::new();
                        m.insert("BaseValueConstant".to_string(), Pv::Float(*c));
                        Pv::Map(m)
                    }
                }),
        };
        let value = match raw {
            None | Some(Pv::Null) => None,
            Some(map @ Pv::Map(_)) => v.value(&map)?,
            Some(other) => Some(f32r(
                other
                    .number()
                    .ok_or_else(|| raises(format!("{field} is not a number")))?,
            )),
        };
        base.insert(format!("{ATTR}{name}"), value);
    }
    for (name, default) in WEAPON_DEFAULTS {
        let key = format!("{ATTR}{name}");
        if base.get(&key).copied().flatten().is_none() {
            base.insert(key, Some(default));
        }
    }
    let barrel_path = recipe.part("Barrel").map(str::to_string);
    let barrel = match &barrel_path {
        Some(p) => Some(stats.props(p)?),
        None => None,
    };
    let at = |k: &str| barrel.as_ref().map_or(Pv::Null, |b| b.at(k).clone());
    let spinning = at("bIsSpinningEnabled").truthy();
    if spinning && at("SpinUpDuration").truthy() {
        let spin = v.value(&at("SpinUpDuration"))?;
        base.insert(format!("{ATTR}WeaponBarrelSpinUpDuration"), spin);
    }

    let mut stacks = Stacks::default();
    let apply =
        |v: &mut Values<'_, '_>, stacks: &mut Stacks, props: &Pv| -> Result<(), ItemError> {
            for effect in props.at("WeaponAttributeEffects").items() {
                let attribute = effect_attribute(effect)?;
                let kind = effect_kind(effect)?;
                if let Some(amount) = v.value(effect.at("BaseModifierValue"))? {
                    stacks.add(attribute, kind, amount);
                }
            }
            Ok(())
        };
    apply(v, &mut stacks, &weapon_type)?;
    let parts = recipe.ordered_parts();
    let mut grades: DetMap<String, f64> = DetMap::new();
    let mut activated: DetSet<String> = DetSet::new();
    let mut count_upgrades = |props: &Pv| -> Result<(), ItemError> {
        for upgrade in props.at("AttributeSlotUpgrades").items() {
            let name = slot_name(upgrade)?;
            let increase = upgrade.at("GradeIncrease");
            let increase = if increase.truthy() {
                increase.number().unwrap_or(0.0)
            } else {
                0.0
            };
            *grades.entry(name.clone()).or_insert(0.0) += increase;
            if upgrade.at("bActivateSlot").truthy() {
                activated.insert(name);
            }
        }
        Ok(())
    };
    count_upgrades(&weapon_type)?;
    for part in &parts {
        let props = stats.props(part)?;
        apply(v, &mut stacks, &props)?;
        count_upgrades(&props)?;
    }
    let base_grade = match weapon_type.get("AttributeSlotBaseGrade") {
        Some(g) if g.truthy() => v.value(g)?,
        _ => Some(1.0),
    };
    for effect in weapon_type.at("AttributeSlotEffects").items() {
        let name = effect.at("SlotName").as_str().unwrap_or("").to_string();
        // Only activated internal slots modify the weapon (external ones go to the owner's pools).
        if !activated.contains(&name) || effect.at("bExternalSlot").truthy() {
            continue;
        }
        let grade = slot_grade(
            base_grade,
            grades.get(&name).copied().unwrap_or(0.0),
            effect,
        );
        let per_grade = v.value(effect.at("PerGradeUpgrade"))?.unwrap_or(0.0);
        let base_modifier = v.value(effect.at("BaseModifierValue"))?.unwrap_or(0.0);
        let amount = slot_value(grade, base_modifier, per_grade);
        stacks.add(effect_attribute(effect)?, effect_kind(effect)?, amount);
    }
    for name in recipe.name_parts() {
        let props = stats.props(name)?;
        apply(v, &mut stacks, &props)?;
    }

    let mut values = DetMap::new();
    let keys: DetSet<String> = base.keys().chain(stacks.0.keys()).cloned().collect();
    for attribute in keys {
        let Some(start) = base.get(&attribute).copied().flatten() else {
            continue;
        };
        let integer = INT_ATTRIBUTES
            .iter()
            .any(|n| attribute == format!("{ATTR}{n}"));
        values.insert(attribute.clone(), stacks.fold(&attribute, start, integer));
    }
    Ok(WeaponAttributes {
        base: base
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect(),
        stacks,
        values,
        grades,
        barrel: barrel_path,
        spinning,
    })
}

fn slot_name(upgrade: &Pv) -> Result<String, ItemError> {
    match upgrade.get("SlotName") {
        None => Err(raises("a slot upgrade without a SlotName")),
        Some(Pv::Str(s)) => Ok(s.clone()),
        Some(Pv::Null) => Ok(String::new()),
        Some(_) => Err(raises("a SlotName that is not a name")),
    }
}

// ---- the accuracy pool ----------------------------------------------------------------------------------------------

/// The player's accuracy pool while the weapon is held (`weapon_firing.accuracy_pool_of`): the pool's base minimum,
/// maximum and idle rate, each with the weapon's external modifiers (the type's and the parts'
/// `ExternalAttributeEffects`, and its activated external slots graded as internal ones are). `None` without the
/// pool. UNVERIFIED: the external slot grading reuses the internal rule (checked against the live pool minimum per
/// gun, decision 2026-11-06-37).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct AccuracyPool {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub idle_rate: Option<f64>,
}

/// `accuracy_pool_of` with the card's level and damage as the known attribute values.
pub fn accuracy_pool(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
    level: f64,
    damage: Option<f64>,
) -> Result<Option<AccuracyPool>, ItemError> {
    if !stats.holds(ACCURACY_POOL) {
        return Ok(None);
    }
    let pool = stats.props(ACCURACY_POOL)?;
    let mut known = vec![(WEAPON_LEVEL.to_string(), level)];
    if let Some(d) = damage {
        known.push((format!("{ATTR}WeaponDamage"), d));
    }
    let mut unresolved = DetSet::new();
    let number = |init: &Pv, unresolved: &mut DetSet<String>| -> Result<Option<f64>, ItemError> {
        match init {
            Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => Ok(init.number()),
            v if !v.truthy() => Ok(None),
            v => price_init(stats, v, &known, unresolved, 0),
        }
    };
    let base = [
        number(pool.at("BaseMinValue"), &mut unresolved)?,
        number(pool.at("BaseMaxValue"), &mut unresolved)?,
        number(pool.at("BaseOnIdleRegenerationRate"), &mut unresolved)?,
    ];
    let weapon_type = stats.props(&recipe.weapon_type)?;
    let mut stacks = Stacks::default();
    let mut grades: DetMap<String, f64> = DetMap::new();
    let mut activated: DetSet<String> = DetSet::new();
    let mut holders: Vec<Option<Obj>> = vec![Some(weapon_type.clone())];
    for part in recipe.ordered_parts() {
        holders.push(if stats.holds(part) {
            Some(stats.props(part)?)
        } else {
            None
        });
    }
    for props in holders.iter().flatten() {
        for effect in props.at("ExternalAttributeEffects").items() {
            // The tool evaluates the amount before it looks at the attribute (the call's arguments), so an amount
            // that raises raises for any attribute.
            let amount = number(effect.at("BaseModifierValue"), &mut unresolved)?;
            let attribute = effect.at("AttributeToModify").as_str().unwrap_or("");
            let Some((_, key)) = ACCURACY_ATTRIBUTES.iter().find(|(a, _)| *a == attribute) else {
                continue;
            };
            let Some(amount) = amount else {
                continue;
            };
            let kind = effect.at("ModifierType").as_str().unwrap_or("");
            stacks.add(key, kind, amount);
        }
        for upgrade in props.at("AttributeSlotUpgrades").items() {
            let name = slot_name(upgrade)?;
            let increase = upgrade.at("GradeIncrease");
            let increase = if increase.truthy() {
                increase.number().unwrap_or(0.0)
            } else {
                0.0
            };
            *grades.entry(name.clone()).or_insert(0.0) += increase;
            if upgrade.at("bActivateSlot").truthy() {
                activated.insert(name);
            }
        }
    }
    let base_grade = match weapon_type.get("AttributeSlotBaseGrade") {
        Some(g) if g.truthy() => number(g, &mut unresolved)?,
        _ => Some(1.0),
    };
    for effect in weapon_type.at("AttributeSlotEffects").items() {
        let name = effect.at("SlotName").as_str().unwrap_or("").to_string();
        if !activated.contains(&name) || !effect.at("bExternalSlot").truthy() {
            continue;
        }
        let grade = slot_grade(
            base_grade,
            grades.get(&name).copied().unwrap_or(0.0),
            effect,
        );
        let base_modifier = number(effect.at("BaseModifierValue"), &mut unresolved)?.unwrap_or(0.0);
        let per_grade = number(effect.at("PerGradeUpgrade"), &mut unresolved)?.unwrap_or(0.0);
        let amount = slot_value(grade, base_modifier, per_grade);
        let attribute = effect.at("AttributeToModify").as_str().unwrap_or("");
        if let Some((_, key)) = ACCURACY_ATTRIBUTES.iter().find(|(a, _)| *a == attribute) {
            let kind = effect.at("ModifierType").as_str().unwrap_or("");
            stacks.add(key, kind, amount);
        }
    }
    // The pool values fold as weapon attributes do; a start is a decoded number or an init result, so f32r is the
    // identity on an init result and rounds a decoded double the way the tool's f32 sums would see it.
    let fold = |key: &str, start: Option<f64>| start.map(|s| stacks.fold_raw(key, s));
    Ok(Some(AccuracyPool {
        min: fold("min", base[0]),
        max: fold("max", base[1]),
        idle_rate: fold("idle_rate", base[2]),
    }))
}

impl Stacks {
    /// `weapon_stats.combine(start, mods)` for a float attribute whose start may be any double (the tool does not
    /// round the start: `f32((start + pre) * factor + post)` in double). Folded in double as `ow-attr`'s `WeaponGen`
    /// form does, with the start unrounded.
    fn fold_raw(&self, attribute: &str, start: f64) -> f64 {
        let b = self.0.get(attribute).cloned().unwrap_or_default();
        let factor = (1.0 + b.scale_up) / (1.0 + b.scale_down);
        f32r((start + b.pre_add) * factor + b.post_add)
    }
}

// ---- the gestalt ---------------------------------------------------------------------------------------------------

/// A weapon's assembly fields of `weapon_recipe.roll`: the type's gestalt mesh, the fragment names the parts draw
/// (sorted, unique) and the material part's material.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    pub gestalt: Pv,
    pub fragments: Vec<String>,
    pub material: Pv,
}

/// `weapon_recipe.part_fragments` for every part, the type's `GestaltMesh` and the material part's `Material`
/// (evidence: the 6 slice guns' gestalt data, WEAPON_VISUALS.md; UNVERIFIED rule).
pub fn assembly(stats: &StatsData<'_>, recipe: &WeaponRecipe) -> Result<Assembly, ItemError> {
    let gestalt = if recipe.weapon_type.is_empty() {
        Pv::Null
    } else {
        stats.prop(&recipe.weapon_type, "GestaltMesh")?
    };
    let mut fragments: DetSet<String> = DetSet::new();
    for (_, part) in &recipe.parts {
        if short(part).to_lowercase().ends_with("_none") {
            continue;
        }
        let props = stats.props(part)?;
        let mut names: Vec<Option<String>> = vec![
            props
                .at("GestaltModeSkeletalMeshName")
                .as_str()
                .map(str::to_string),
        ];
        names.extend(
            stats
                .statics(part, "AdditionalGestaltModeSkeletalMeshNames")?
                .iter()
                .map(|n| n.as_str().map(str::to_string)),
        );
        for n in names.into_iter().flatten() {
            if !n.is_empty() && n != "None" {
                fragments.insert(n);
            }
        }
    }
    let material = match recipe.part("Material") {
        Some(p) => stats.prop(p, "Material")?,
        None => Pv::Null,
    };
    Ok(Assembly {
        gestalt,
        fragments: fragments.into_iter().collect(),
        material,
    })
}

// ---- items ---------------------------------------------------------------------------------------------------------

/// An item's stats (module documentation, "Items"): the owner's modifier stacks the item's external effects and
/// external slots build, and the slots that reached the owner with their grades.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemStats {
    pub external: Stacks,
    pub reached: Vec<(String, f64)>,
}

impl ItemStats {
    /// The value an owner attribute takes over `base` (the shield pool's base is [`SHIELD_POOL_BASE`]).
    pub fn value_over(&self, attribute: &str, base: f32) -> f64 {
        self.external.fold(attribute, f64::from(base), false)
    }

    /// The shield card's three numbers over the pool's base, by label ([`SHIELD_STATS`]).
    pub fn shield(&self) -> Vec<(&'static str, f64)> {
        SHIELD_STATS
            .iter()
            .map(|(label, attr)| (*label, self.value_over(attr, SHIELD_POOL_BASE)))
            .collect()
    }

    /// The shield card's numbers in `mode`. Reference refuses (`BadData`) a number that is not finite or not above 0,
    /// as `prepare_stock_shield.stats` refuses it (a capacity of 0 equips as no shield, and the host would replace a
    /// rate or delay of 0 or less with its stand-ins). Off mode gives the numbers as they are: whether the game
    /// treats such a shield specially is not read.
    pub fn shield_card(&self, mode: ParityMode) -> Result<Vec<(&'static str, f64)>, ItemError> {
        let values = self.shield();
        if mode == ParityMode::Reference {
            for (label, v) in &values {
                if !v.is_finite() || *v <= 0.0 {
                    return Err(ItemError::BadData(format!(
                        "{label} evaluates to {}; it must be above 0 (prepare_stock_shield refuses it)",
                        ow_core::fmt::fmt_py_repr(*v)
                    )));
                }
            }
        }
        Ok(values)
    }
}

/// One item effect or slot entry as both readings hand it over: the attribute, the modifier type, the values.
struct ItemEffect {
    attribute: String,
    kind: String,
}

/// The item rule (module documentation) over `holders` (the definition, the parts in slot order, the name parts),
/// with `value` giving each init's number. `upgrades`, `effects` and `slot_effects` read a holder's lists.
fn item_rule(
    holders: &[Pv],
    definition: &Pv,
    mut value: impl FnMut(&Pv) -> Result<f64, ItemError>,
) -> Result<ItemStats, ItemError> {
    let mut stats = ItemStats::default();
    for holder in holders {
        for effect in holder.at("ExternalAttributeEffects").items() {
            let e = item_effect(effect);
            let amount = value(effect.at("BaseModifierValue"))?;
            stats.external.add(&e.attribute, &e.kind, amount);
        }
    }
    let base_grade = match definition.get("AttributeSlotBaseGrade") {
        Some(g) if g.truthy() => value(g)?,
        _ => 1.0,
    };
    let limit = match definition.get("AttributeSlotMaxActivated") {
        Some(Pv::Int(n)) if (1..=19).contains(n) => *n,
        _ => MAX_SLOTS_DEFAULT,
    };
    let mut increases: DetMap<String, f64> = DetMap::new();
    let mut active: Vec<String> = Vec::new();
    for holder in holders {
        for upgrade in holder.at("AttributeSlotUpgrades").items() {
            let name = upgrade.at("SlotName").as_str().unwrap_or("").to_string();
            let increase = upgrade.at("GradeIncrease");
            let increase = if increase.truthy() {
                increase.number().unwrap_or(0.0)
            } else {
                0.0
            };
            *increases.entry(name.clone()).or_insert(0.0) += increase;
            // `limit` is 1..=19, so the conversion of the count cannot fail.
            let full = i64::try_from(active.len()).map_or(true, |n| n >= limit);
            if upgrade.at("bActivateSlot").truthy() && !active.contains(&name) && !full {
                active.push(name);
            }
        }
    }
    for effect in definition.at("AttributeSlotEffects").items() {
        let name = effect.at("SlotName").as_str().unwrap_or("").to_string();
        if !active.contains(&name) || !effect.at("AttributeToModify").truthy() {
            continue;
        }
        if !effect.at("bExternalSlot").truthy() || effect.at("bRunEffectsAsSkill").truthy() {
            continue;
        }
        let grade = slot_grade(
            Some(base_grade),
            increases.get(&name).copied().unwrap_or(0.0),
            effect,
        );
        let base_modifier = value(effect.at("BaseModifierValue"))?;
        let per_grade = value(effect.at("PerGradeUpgrade"))?;
        let e = item_effect(effect);
        stats.external.add(
            &e.attribute,
            &e.kind,
            slot_value(grade, base_modifier, per_grade),
        );
        stats.reached.push((name, grade));
    }
    Ok(stats)
}

fn item_effect(effect: &Pv) -> ItemEffect {
    let kind = effect.at("ModifierType");
    ItemEffect {
        attribute: effect
            .at("AttributeToModify")
            .as_str()
            .unwrap_or("")
            .to_string(),
        // `prepare_stock_shield`: an untagged ModifierType is the enum's first value, MT_Scale.
        kind: kind
            .as_str()
            .filter(|k| !k.is_empty())
            .unwrap_or("MT_Scale")
            .to_string(),
    }
}

/// An item's stats in Reference mode (`prepare_stock_shield.stats`): every init through `weapon_recipe.attribute_value`
/// with the item level for an attribute whose resolver reads `ExpLevel` and the constant of an attribute with a
/// constant resolver; anything else refuses, as the tool does (`BadData`).
pub fn item_stats(
    stats: &StatsData<'_>,
    definition: &str,
    parts: &[&str],
    names: &[&str],
    level: i64,
) -> Result<ItemStats, ItemError> {
    let mut holders: Vec<Pv> = vec![(*stats.props(definition)?).clone()];
    for p in parts.iter().chain(names) {
        holders.push((*stats.props(p)?).clone());
    }
    let mut cache: Vec<(String, f64)> = Vec::new();
    // `level` is an item level, far below 2^53: exact.
    let lvl = level as f64;
    let value = |init: &Pv| -> Result<f64, ItemError> {
        if !matches!(init, Pv::Map(_)) {
            return Ok(if init.truthy() {
                init.number()
                    .ok_or_else(|| raises("an init that is not a number"))?
            } else {
                0.0
            });
        }
        let mut known = Vec::new();
        for attribute in init_attributes(stats, init, 0)? {
            if let Some((_, v)) = cache.iter().find(|(k, _)| *k == attribute) {
                known.push((attribute, *v));
                continue;
            }
            let v = if object_property(stats, &attribute)?.as_deref() == Some("ExpLevel") {
                Some(lvl)
            } else {
                stats
                    .resolver_constant(&attribute)?
                    .and_then(|c| c.number())
            };
            let Some(v) = v else {
                return Err(ItemError::BadData(format!(
                    "attribute {attribute} has no package value (not a constant, not the item level)"
                )));
            };
            cache.push((attribute.clone(), v));
            known.push((attribute, v));
        }
        stats
            .data()
            .attribute_value(init, &known)
            .map_err(|_| raises("an item init"))?
            .ok_or_else(|| {
                ItemError::BadData(format!(
                    "an initialization did not resolve: {}",
                    init.to_json()
                ))
            })
    };
    let definition_props = holders[0].clone();
    item_rule(&holders, &definition_props, value)
}

/// `prepare_vendors.init_attributes`: every base attribute an init and its definition's formula terms name (a set;
/// the order is the first meeting).
fn init_attributes(stats: &StatsData<'_>, init: &Pv, depth: u32) -> Result<Vec<String>, ItemError> {
    if depth > MAX_DEPTH {
        return Err(raises("init data nested too deep"));
    }
    let mut out = Vec::new();
    if !matches!(init, Pv::Map(_)) {
        return Ok(out);
    }
    if let Some(a) = init
        .at("BaseValueAttribute")
        .as_str()
        .filter(|a| !a.is_empty())
    {
        out.push(a.to_string());
    }
    if let Some(d) = init
        .at("InitializationDefinition")
        .as_str()
        .filter(|d| !d.is_empty())
    {
        let formula = stats
            .optional(d)?
            .map(|p| p.at("ValueFormula").clone())
            .unwrap_or(Pv::Null);
        for key in ["Multiplier", "Level", "Power", "Offset"] {
            for a in init_attributes(stats, formula.at(key), depth + 1)? {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
        }
    }
    Ok(out)
}

/// `prepare_vendors.object_property`: the `PropertyName` of the first `ObjectPropertyAttributeValueResolver` in an
/// attribute's `ValueResolverChain` (or, without a chain, among its resolver subobjects).
fn object_property(stats: &StatsData<'_>, attribute: &str) -> Result<Option<String>, ItemError> {
    let chain = stats
        .optional(attribute)?
        .map(|p| p.at("ValueResolverChain").items().to_vec())
        .unwrap_or_default();
    let resolvers: Vec<String> = if !chain.is_empty() {
        chain
            .iter()
            .filter_map(|r| r.as_str().map(str::to_string))
            .collect()
    } else {
        stats
            .data()
            .below_of_class(attribute, "")
            .into_iter()
            .filter(|p| stats.data().class_of(p).contains("ValueResolver"))
            .collect()
    };
    for r in resolvers {
        if stats
            .data()
            .class_of(&r)
            .ends_with("ObjectPropertyAttributeValueResolver")
        {
            return Ok(stats
                .optional(&r)?
                .and_then(|p| p.at("PropertyName").as_str().map(str::to_string)));
        }
    }
    Ok(None)
}

// ---- Off mode: the instance through ow-attr -------------------------------------------------------------------------

/// The class of an item's instance by kind (the classes the records name; the definition's own `InventoryClass` is
/// not read, so a subclass a definition names is not used: UNVERIFIED).
pub fn instance_class(kind: ItemKind) -> Result<&'static str, ItemError> {
    Ok(match kind {
        ItemKind::Weapon => "WillowGame.WillowWeapon",
        ItemKind::Shield => "WillowGame.WillowShield",
        ItemKind::GrenadeMod => "WillowGame.WillowGrenadeMod",
        ItemKind::ClassMod => "WillowGame.WillowClassMod",
        ItemKind::Relic => "WillowGame.WillowArtifact",
        ItemKind::Usable => "WillowGame.WillowUsableItem",
        other => {
            return Err(NotYet::not_read(
                format!("the instance class of a {other:?} item"),
                RECORD,
            )
            .into());
        }
    })
}

/// An item's instance with its stats (Off mode): the object, and for an item the owner's stacks its external
/// effects build (a weapon's stats are the instance's own attributes).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub object: ObjRef,
    pub external: ItemStats,
}

/// The objects an Off-mode evaluation reads, by path: `known` first (objects the caller built), then the definition
/// store of the world.
pub fn find(
    cx: &mut EvalCx<'_>,
    known: &DetMap<String, ObjRef>,
    path: &str,
) -> Result<ObjRef, ItemError> {
    if let Some(o) = known.get(path) {
        return Ok(*o);
    }
    let p = ObjectPath::parse(path).map_err(|e| ItemError::BadData(format!("{path}: {e}")))?;
    let mut parts = cx.world.parts();
    let space: &mut ObjectSpace = parts.space;
    let defs = match parts.defs.as_deref_mut() {
        Some(d) => d,
        None => {
            return Err(NotYet::layer_absent(
                format!("the object {path} (no definition store in this world)"),
                "ow-defs",
                RECORD,
            )
            .into());
        }
    };
    defs.find(space, &p)?
        .ok_or_else(|| ItemError::BadData(format!("{path}: no package holds it")))
}

/// The class at `path`, laid out: through the world's store when it has one (a class the space only declares is laid
/// out there), else the space's own (tests build their classes in the space).
fn class_id(cx: &mut EvalCx<'_>, path: &str) -> Result<ow_object::ids::ClassId, ItemError> {
    let mut parts = cx.world.parts();
    let space: &mut ObjectSpace = parts.space;
    if let Some(d) = parts.defs.as_deref_mut() {
        return Ok(d.class_id(space, path)?);
    }
    match space.classes.by_path(path) {
        Some(c) => Ok(c),
        None => Err(NotYet::layer_absent(
            format!("the class {path} (not laid out in this space)"),
            "ow-object reflection of the WillowGame classes",
            RECORD,
        )
        .into()),
    }
}

/// The property `name` of `obj` as a heap value (`None` when its class declares none).
fn read(cx: &mut EvalCx<'_>, obj: ObjRef, name: &str) -> Result<Option<Value>, ItemError> {
    Ok(ow_attr::eval::property(cx, obj, name)?)
}

fn elements(v: Option<Value>) -> Result<Vec<Value>, ItemError> {
    match v {
        None => Ok(Vec::new()),
        Some(v) => Ok(ow_attr::eval::elements("an array", &v)?.to_vec()),
    }
}

fn field(cx: &mut EvalCx<'_>, v: &Value, name: &str) -> Result<Value, ItemError> {
    let parts = cx.world.parts();
    Ok(ow_attr::eval::field(parts.space, v, name)?.clone())
}

fn init_of(cx: &mut EvalCx<'_>, v: &Value) -> Result<InitData, ItemError> {
    let parts = cx.world.parts();
    Ok(ow_attr::eval::init_data(parts.space, v)?)
}

fn init_field(cx: &mut EvalCx<'_>, v: &Value, name: &str) -> Result<InitData, ItemError> {
    let f = field(cx, v, name)?;
    init_of(cx, &f)
}

fn name_field(cx: &mut EvalCx<'_>, v: &Value, name: &str) -> Result<String, ItemError> {
    let f = field(cx, v, name)?;
    name_text(cx, &f)
}

fn name_text(cx: &mut EvalCx<'_>, v: &Value) -> Result<String, ItemError> {
    match v {
        Value::Name(n) => {
            let parts = cx.world.parts();
            Ok(parts.space.names.text(*n).to_string())
        }
        other => Err(ItemError::BadData(format!(
            "expected a name, found {}",
            other.kind_name()
        ))),
    }
}

fn enum_value(v: &Value) -> Result<u8, ItemError> {
    match v {
        Value::Byte(b) => Ok(*b),
        other => Err(ItemError::BadData(format!(
            "expected an enum byte, found {}",
            other.kind_name()
        ))),
    }
}

fn number_of(v: &Value) -> Result<f64, ItemError> {
    match v {
        Value::Float(x) => Ok(f64::from(*x)),
        Value::Int(i) => Ok(f64::from(*i)),
        Value::Byte(b) => Ok(f64::from(*b)),
        other => Err(ItemError::BadData(format!(
            "expected a number, found {}",
            other.kind_name()
        ))),
    }
}

fn bool_of(v: &Value) -> bool {
    matches!(v, Value::Bool(true))
}

/// `EModifierType` by its value (`MT_Scale` 0, `MT_PreAdd` 1, `MT_PostAdd` 2).
fn modifier_name(b: u8) -> Result<&'static str, ItemError> {
    match ModifierType::from_raw(i64::from(b)) {
        Some(ModifierType::Scale) => Ok("MT_Scale"),
        Some(ModifierType::PreAdd) => Ok("MT_PreAdd"),
        Some(ModifierType::PostAdd) => Ok("MT_PostAdd"),
        _ => Err(ItemError::BadData(format!(
            "ModifierType {b} is outside the enum"
        ))),
    }
}

/// An `AttributeEffectData` heap struct as `ow-attr`'s effect.
fn effect_data(cx: &mut EvalCx<'_>, v: &Value) -> Result<EffectData, ItemError> {
    let attribute =
        ow_attr::eval::object("AttributeToModify", &field(cx, v, "AttributeToModify")?)?;
    let kind = enum_value(&field(cx, v, "ModifierType")?)?;
    let kind = ModifierType::from_raw(i64::from(kind))
        .ok_or_else(|| ItemError::BadData(format!("ModifierType {kind} is outside the enum")))?;
    let init = init_field(cx, v, "BaseModifierValue")?;
    Ok(EffectData::new(attribute, kind, init))
}

/// An object's dotted path: its name after its outers' (a cycle or a chain deeper than 64 is cut, giving the path so
/// far; heap objects built by the store and the tests have short acyclic chains).
fn path_of(cx: &mut EvalCx<'_>, obj: ObjRef) -> String {
    let parts = cx.world.parts();
    let space = &*parts.space;
    let mut names = Vec::new();
    let mut cursor = Some(obj);
    while let Some(o) = cursor {
        if names.len() >= 64 {
            break;
        }
        let Ok(object) = space.heap.get(o) else {
            break;
        };
        names.push(space.names.text(object.name).to_string());
        cursor = object.outer;
    }
    names.reverse();
    names.join(".")
}

/// The item's instance (Off mode; module documentation): an object of its kind's class ([`instance_class`]) with
/// `ExpLevel` at `level` where the class declares it, then [`instantiate_onto`]. `known` names objects the caller
/// holds by path (tests); everything else comes from the world's definition store.
pub fn instantiate_stats(
    data: &DefinitionData,
    level: i32,
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    known: &DetMap<String, ObjRef>,
) -> Result<Instance, ItemError> {
    let class = class_id(cx, instance_class(data.kind)?)?;
    let object = {
        let parts = cx.world.parts();
        let name = format!("{}_Stats", short(data.definition.as_str()));
        parts
            .space
            .alloc(class, &name, None, Origin::Runtime)
            .map_err(AttrError::from)?
    };
    {
        let parts = cx.world.parts();
        if let Some(p) = parts.space.prop(class, "ExpLevel") {
            let kind = parts.space.classes.decl(p).map(|d| d.kind.clone());
            let v = match kind {
                Some(
                    ow_object::layout::PropKind::Float
                    | ow_object::layout::PropKind::FloatAttribute,
                ) => {
                    // An item level is far below 2^24: exact in single precision.
                    Value::Float(level as f32)
                }
                _ => Value::Int(level),
            };
            parts
                .space
                .write(object, PropAddr::scalar(p), v)
                .map_err(AttrError::from)?;
        }
    }
    instantiate_onto(object, data, sys, cx, known)
}

/// The stats of `data` set up on `object`, an instance the caller has (the world's weapon actor, or the object
/// [`instantiate_stats`] made). A weapon runs, in the script's order (`WillowWeapon.InitializeInternal`),
/// [`weapon_base_values`], [`weapon_part_base_values`] with its barrel and body, and [`apply_weapon_effects`]; its
/// stats are then its own attribute stacks. An item's external effects and slots are evaluated with `object` as the
/// source ([`ItemStats`]).
pub fn instantiate_onto(
    object: ObjRef,
    data: &DefinitionData,
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    known: &DetMap<String, ObjRef>,
) -> Result<Instance, ItemError> {
    let definition = find(cx, known, data.definition.as_str())?;
    let mut parts: Vec<ObjRef> = Vec::new();
    for p in data.parts.iter().flatten() {
        parts.push(find(cx, known, p.as_str())?);
    }
    let mut names: Vec<ObjRef> = Vec::new();
    for n in [&data.prefix, &data.title].into_iter().flatten() {
        names.push(find(cx, known, n.as_str())?);
    }
    if data.kind == ItemKind::Weapon {
        let mut slot = |s: WeaponSlot| -> Result<Option<ObjRef>, ItemError> {
            match data.parts.get(s.index()).and_then(Option::as_ref) {
                Some(p) => Ok(Some(find(cx, known, p.as_str())?)),
                None => Ok(None),
            }
        };
        let (barrel, body) = (slot(WeaponSlot::Barrel)?, slot(WeaponSlot::Body)?);
        weapon_base_values(object, definition, sys, cx)?;
        weapon_part_base_values(object, barrel, body, sys, cx)?;
        apply_weapon_effects(object, definition, &parts, &names, sys, cx)?;
        return Ok(Instance {
            object,
            external: ItemStats::default(),
        });
    }
    let mut holders = vec![definition];
    holders.extend(parts.iter().copied());
    holders.extend(names.iter().copied());
    let external = item_instance(object, &holders, sys, cx)?;
    Ok(Instance { object, external })
}

/// How one base of [`WEAPON_BASE_VALUES`] is formed from its source field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseFrom {
    /// The field's value as it is (a plain float or int field).
    Copy,
    /// `EvaluateInitializationData(field, the weapon)`.
    Evaluate,
}

/// `WillowWeapon.CalculateWeaponBaseValues` as read from script (`research/script_disasm.py`, 2026-11-15;
/// UNVERIFIED in game): each entry sets the base of a weapon attribute property (opcode 0x5F, the attribute let,
/// which recomputes the value and sends no change notification) from a field of the weapon type, in this order.
/// After these, `InstantHitMomentum` is multiplied by 100; the three entries of [`WEAPON_OVERHEAT_VALUES`] follow
/// only for a type with `bUseOverheatBehavior`.
pub const WEAPON_BASE_VALUES: [(&str, &str, BaseFrom); 22] = [
    ("ShotCost", "ShotCost", BaseFrom::Copy),
    ("ClipSize", "ClipSize", BaseFrom::Copy),
    ("InstantHitDamage", "InstantHitDamage", BaseFrom::Evaluate),
    (
        "InstantHitMomentum",
        "InstantHitMomentum",
        BaseFrom::Evaluate,
    ),
    ("ProjectilesPerShot", "ProjectilesPerShot", BaseFrom::Copy),
    ("MeleeDamage", "MeleeDamage", BaseFrom::Evaluate),
    (
        "BaseStatusEffectChanceModifier",
        "BaseStatusEffectChanceModifier",
        BaseFrom::Evaluate,
    ),
    (
        "StatusEffectDamage",
        "StatusEffectDamage",
        BaseFrom::Evaluate,
    ),
    ("ExtraShotDelay", "ExtraShotDelay", BaseFrom::Evaluate),
    ("AimError", "AIAimError", BaseFrom::Copy),
    ("Spread", "Spread", BaseFrom::Copy),
    (
        "PerShotAccuracyImpulse",
        "PerShotAccuracyImpulse",
        BaseFrom::Copy,
    ),
    ("WeaponRange", "Range", BaseFrom::Copy),
    ("FireInterval", "FireRate", BaseFrom::Copy),
    ("BurstInterval", "BurstInterval", BaseFrom::Copy),
    ("AutomaticBurstCount", "AutomaticBurstCount", BaseFrom::Copy),
    (
        "BurstShotAccuracyImpulseScale",
        "BurstShotAccuracyImpulseScale",
        BaseFrom::Copy,
    ),
    ("ReloadTime", "ReloadTime", BaseFrom::Copy),
    ("EquipTime", "EquipTime", BaseFrom::Copy),
    ("PutDownTime", "PutDownTime", BaseFrom::Copy),
    ("ZoomedEndFOV", "ZoomedEndFOV", BaseFrom::Copy),
    ("ZoomedRate", "ZoomedRate", BaseFrom::Copy),
];

/// The factor `CalculateWeaponBaseValues` multiplies `InstantHitMomentum` by after setting it (script, UNVERIFIED in
/// game).
pub const MOMENTUM_SCALE: f32 = 100.0;

/// The overheat bases `CalculateWeaponBaseValues` sets when the type has `bUseOverheatBehavior` (script, UNVERIFIED).
pub const WEAPON_OVERHEAT_VALUES: [(&str, &str); 3] = [
    ("OverheatRegenDelay", "OverheatRegenDelay"),
    ("FireRegenDelay", "FireRegenDelay"),
    ("RegenRate", "RegenRate"),
];

/// `WillowWeapon.CalculatePartDependentWeaponBaseValues` as read from script (2026-11-15; UNVERIFIED in game): for
/// the barrel, then the body, when the part is set: with `bIsSpinningEnabled` its spin-up and spin-down durations,
/// with `bFlapsEnabled` its flap expand and collapse durations, each `EvaluateInitializationData(field, the weapon)`
/// into a weapon attribute base; and the weapon's `BodyVariation` takes the body's `PartVariation` (a plain
/// assignment). Each entry: (part flag, part field, barrel property, body property). That the flap block does not
/// sit inside the spin block is this reading of the jump targets, UNVERIFIED.
pub const WEAPON_PART_BASE_VALUES: [(&str, &str, &str, &str); 4] = [
    (
        "bIsSpinningEnabled",
        "SpinUpDuration",
        "BarrelSpinUpDuration",
        "MagazineSpinUpDuration",
    ),
    (
        "bIsSpinningEnabled",
        "SpinDownDuration",
        "BarrelSpinDownDuration",
        "MagazineSpinDownDuration",
    ),
    (
        "bFlapsEnabled",
        "FlapsExpandDuration",
        "BarrelFlapsExpandDuration",
        "BodyFlapsExpandDuration",
    ),
    (
        "bFlapsEnabled",
        "FlapsCollapseDuration",
        "BarrelFlapsCollapseDuration",
        "BodyFlapsCollapseDuration",
    ),
];

/// The property `name` of `obj`'s class, `BadData` when the class declares none (the script names it, so a class
/// without it is not the class the script runs on).
fn declared(
    cx: &mut EvalCx<'_>,
    obj: ObjRef,
    name: &str,
) -> Result<ow_object::ids::PropId, ItemError> {
    let parts = cx.world.parts();
    let class = parts.space.heap.get(obj).map_err(AttrError::from)?.class;
    parts.space.prop(class, name).ok_or_else(|| {
        ItemError::BadData(format!(
            "{} declares no {name}",
            ow_attr::eval::class_path(parts.space, obj)
        ))
    })
}

/// The 0x5F attribute let on `weapon`'s property `name`: `rhs` converted to the property's kind (a float into an int
/// or byte attribute as the attribute path converts it, `ow_attr::stack::store_as`; an int into a float as C does),
/// stored into the base companion and the value recomputed in the world's combine form, with no change notification
/// (`ow_attr::registry::let_base`; NATIVE_BYTECODE_OPCODES opcode 5F). The script-side conversion of the right side is
/// the VM's (`ow-vm` W3-INTERP); this offline copy follows the attribute store's, UNVERIFIED where they could differ.
fn let_attribute_base(
    cx: &mut EvalCx<'_>,
    weapon: ObjRef,
    name: &str,
    rhs: Value,
) -> Result<(), ItemError> {
    let prop = declared(cx, weapon, name)?;
    let form = cx.forms.combine;
    let parts = cx.world.parts();
    let decl =
        parts.space.classes.decl(prop).ok_or_else(|| {
            ItemError::BadData(format!("{name}: a property without a declaration"))
        })?;
    let Some(companions) = decl.attr else {
        return Err(ItemError::BadData(format!(
            "{name} is not an attribute property (no base or stack companion), so the attribute let cannot set it"
        )));
    };
    let kind = match decl.kind {
        ow_object::layout::PropKind::IntAttribute => NumKind::Int,
        ow_object::layout::PropKind::ByteAttribute { .. } => NumKind::Byte,
        _ => NumKind::Float,
    };
    let as_float = |v: &Value| -> Result<f32, ItemError> {
        match v {
            Value::Float(x) => Ok(*x),
            // The C conversion of an int or byte to float (round to nearest).
            Value::Int(i) => Ok(*i as f32),
            Value::Byte(b) => Ok(f32::from(*b)),
            other => Err(ItemError::BadData(format!(
                "{name}: a {} cannot be an attribute base",
                other.kind_name()
            ))),
        }
    };
    let converted = match (kind, &rhs) {
        (NumKind::Int, Value::Int(i)) => Value::Int(*i),
        (NumKind::Int, Value::Byte(b)) => Value::Int(i32::from(*b)),
        (NumKind::Byte, Value::Byte(b)) => Value::Byte(*b),
        (k, v) => match ow_attr::stack::store_as(as_float(v)?, k) {
            Combined::Float(x) => Value::Float(x),
            Combined::Int(i) => Value::Int(i),
            Combined::Byte(b) => Value::Byte(b),
        },
    };
    ow_attr::registry::let_base(parts.space, weapon, companions, converted, form)?;
    Ok(())
}

/// The value of `holder`'s field `field` for a base: an init evaluated with `weapon` as the source, or a plain
/// number as it is.
fn base_source(
    cx: &mut EvalCx<'_>,
    sys: &AttributeSystem,
    weapon: ObjRef,
    holder: ObjRef,
    field_name: &str,
    from: BaseFrom,
) -> Result<Value, ItemError> {
    let prop = declared(cx, holder, field_name)?;
    let v = {
        let parts = cx.world.parts();
        parts
            .space
            .read(holder, PropAddr::scalar(prop))
            .map_err(AttrError::from)?
            .clone()
    };
    match (from, &v) {
        (BaseFrom::Evaluate, Value::Struct(_)) => {
            let init = init_of(cx, &v)?;
            Ok(Value::Float(sys.evaluate_init(
                &init,
                Some(weapon),
                None,
                cx,
            )?))
        }
        (BaseFrom::Evaluate, other) => Err(ItemError::BadData(format!(
            "{field_name} is a {}, not an AttributeInitializationData",
            other.kind_name()
        ))),
        (BaseFrom::Copy, _) => Ok(v),
    }
}

/// `CalculateWeaponBaseValues` on `weapon` from `weapon_type` ([`WEAPON_BASE_VALUES`]; the type's untagged fields
/// hold their class defaults, as the store loads them). The bases W5-EQUIP's override of the script function binds.
pub fn weapon_base_values(
    weapon: ObjRef,
    weapon_type: ObjRef,
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
) -> Result<(), ItemError> {
    for (property, field_name, from) in WEAPON_BASE_VALUES {
        let v = base_source(cx, sys, weapon, weapon_type, field_name, from)?;
        let_attribute_base(cx, weapon, property, v)?;
    }
    let momentum = declared(cx, weapon, "InstantHitMomentum")?;
    let current = {
        let parts = cx.world.parts();
        parts
            .space
            .read(weapon, PropAddr::scalar(momentum))
            .map_err(AttrError::from)?
            .clone()
    };
    let scaled = match current {
        Value::Float(x) => Value::Float(x * MOMENTUM_SCALE),
        other => {
            return Err(ItemError::BadData(format!(
                "InstantHitMomentum holds a {}, not a float",
                other.kind_name()
            )));
        }
    };
    let_attribute_base(cx, weapon, "InstantHitMomentum", scaled)?;
    let overheat = declared(cx, weapon_type, "bUseOverheatBehavior")?;
    let uses_overheat = {
        let parts = cx.world.parts();
        bool_of(
            parts
                .space
                .read(weapon_type, PropAddr::scalar(overheat))
                .map_err(AttrError::from)?,
        )
    };
    if uses_overheat {
        for (property, field_name) in WEAPON_OVERHEAT_VALUES {
            let v = base_source(cx, sys, weapon, weapon_type, field_name, BaseFrom::Copy)?;
            let_attribute_base(cx, weapon, property, v)?;
        }
    }
    Ok(())
}

/// `CalculatePartDependentWeaponBaseValues` on `weapon` ([`WEAPON_PART_BASE_VALUES`]).
pub fn weapon_part_base_values(
    weapon: ObjRef,
    barrel: Option<ObjRef>,
    body: Option<ObjRef>,
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
) -> Result<(), ItemError> {
    for (part, is_barrel) in [(barrel, true), (body, false)] {
        let Some(part) = part else {
            continue;
        };
        for (flag, field_name, barrel_property, body_property) in WEAPON_PART_BASE_VALUES {
            let set = {
                let prop = declared(cx, part, flag)?;
                let parts = cx.world.parts();
                bool_of(
                    parts
                        .space
                        .read(part, PropAddr::scalar(prop))
                        .map_err(AttrError::from)?,
                )
            };
            if !set {
                continue;
            }
            let v = base_source(cx, sys, weapon, part, field_name, BaseFrom::Evaluate)?;
            let property = if is_barrel {
                barrel_property
            } else {
                body_property
            };
            let_attribute_base(cx, weapon, property, v)?;
        }
        if !is_barrel {
            let from = declared(cx, part, "PartVariation")?;
            let to = declared(cx, weapon, "BodyVariation")?;
            let parts = cx.world.parts();
            let v = parts
                .space
                .read(part, PropAddr::scalar(from))
                .map_err(AttrError::from)?
                .clone();
            parts
                .space
                .write(weapon, PropAddr::scalar(to), v)
                .map_err(AttrError::from)?;
        }
    }
    Ok(())
}

/// `ApplyAllWeaponAttributeEffects`, the internal slots and `ApplyNamePartWeaponAttributeEffects` on `weapon`: the
/// type's `WeaponAttributeEffects`, each part's in slot order, the activated internal slots (graded over the type's
/// and the parts' upgrades; at most the type's `AttributeSlotMaxActivated` slots activate, class default 19, values
/// outside 1..19 read as 19: the record's rule for weapons, not applied by the Python tool, UNVERIFIED in game), then
/// the name parts', all applied with `apply_effects` and `weapon` as the source. Returns the applied records (what
/// an unequip removes).
pub fn apply_weapon_effects(
    weapon: ObjRef,
    weapon_type: ObjRef,
    parts: &[ObjRef],
    names: &[ObjRef],
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
) -> Result<ow_attr::api::Applied, ItemError> {
    let effects_of = |cx: &mut EvalCx<'_>, holder: ObjRef| -> Result<Vec<EffectData>, ItemError> {
        let mut out = Vec::new();
        for v in elements(read(cx, holder, "WeaponAttributeEffects")?)? {
            out.push(effect_data(cx, &v)?);
        }
        Ok(out)
    };
    let limit = match read(cx, weapon_type, "AttributeSlotMaxActivated")? {
        Some(Value::Int(n)) if (1..=19).contains(&n) => usize::try_from(n).unwrap_or(19),
        Some(Value::Byte(n)) if (1..=19).contains(&n) => usize::from(n),
        _ => 19,
    };
    let mut effects = effects_of(cx, weapon_type)?;
    let mut grades: DetMap<String, f64> = DetMap::new();
    let mut activated: Vec<String> = Vec::new();
    let mut count = |cx: &mut EvalCx<'_>, holder: ObjRef| -> Result<(), ItemError> {
        for u in elements(read(cx, holder, "AttributeSlotUpgrades")?)? {
            let name = name_field(cx, &u, "SlotName")?;
            let increase = number_of(&field(cx, &u, "GradeIncrease")?)?;
            *grades.entry(name.clone()).or_insert(0.0) += increase;
            if bool_of(&field(cx, &u, "bActivateSlot")?)
                && !activated.contains(&name)
                && activated.len() < limit
            {
                activated.push(name);
            }
        }
        Ok(())
    };
    count(cx, weapon_type)?;
    for p in parts {
        effects.extend(effects_of(cx, *p)?);
        count(cx, *p)?;
    }
    let base_grade = match read(cx, weapon_type, "AttributeSlotBaseGrade")? {
        Some(v) => {
            let init = init_of(cx, &v)?;
            f64::from(sys.evaluate_init(&init, Some(weapon), None, cx)?)
        }
        None => 1.0,
    };
    let mut slot_effects = Vec::new();
    for e in elements(read(cx, weapon_type, "AttributeSlotEffects")?)? {
        let name = name_field(cx, &e, "SlotName")?;
        if !activated.contains(&name) || bool_of(&field(cx, &e, "bExternalSlot")?) {
            continue;
        }
        let bound = |cx: &mut EvalCx<'_>, k: &str| -> Result<Pv, ItemError> {
            Ok(match field(cx, &e, k)? {
                Value::Bool(b) => Pv::Bool(b),
                other => Pv::Float(number_of(&other)?),
            })
        };
        let mut m = DetMap::new();
        for k in [
            "bEnforceMaximumGrade",
            "MaximumGrade",
            "bEnforceMinimumGrade",
            "MinimumGrade",
        ] {
            m.insert(k.to_string(), bound(cx, k)?);
        }
        let grade = slot_grade(
            Some(base_grade),
            grades.get(&name).copied().unwrap_or(0.0),
            &Pv::Map(m),
        );
        let per = init_field(cx, &e, "PerGradeUpgrade")?;
        let per = sys.evaluate_init(&per, Some(weapon), None, cx)?;
        let base = init_field(cx, &e, "BaseModifierValue")?;
        let base = sys.evaluate_init(&base, Some(weapon), None, cx)?;
        let amount = slot_value(grade, f64::from(base), f64::from(per));
        let attribute =
            ow_attr::eval::object("AttributeToModify", &field(cx, &e, "AttributeToModify")?)?;
        let kind = enum_value(&field(cx, &e, "ModifierType")?)?;
        let kind = ModifierType::from_raw(i64::from(kind)).ok_or_else(|| {
            ItemError::BadData(format!("ModifierType {kind} is outside the enum"))
        })?;
        // `amount` is a single-precision value (slot_value rounds it), so the cast is exact.
        slot_effects.push(EffectData::new(
            attribute,
            kind,
            InitData::constant(amount as f32),
        ));
    }
    effects.extend(slot_effects);
    for n in names {
        effects.extend(effects_of(cx, *n)?);
    }
    Ok(sys.apply_effects(&effects, Some(weapon), None, cx)?)
}

/// The item half of [`instantiate_stats`]: the item rule with each init evaluated through `ow-attr` with the
/// instance as the source.
fn item_instance(
    item: ObjRef,
    holders: &[ObjRef],
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
) -> Result<ItemStats, ItemError> {
    // The holders' lists read into the oracle's value model, each init kept as its heap value by index.
    let mut inits: Vec<Value> = Vec::new();
    let mut keep = |v: Value| -> Pv {
        inits.push(v);
        let mut m = DetMap::new();
        let index = i64::try_from(inits.len() - 1).unwrap_or(i64::MAX);
        m.insert("heap_init".to_string(), Pv::Int(index));
        Pv::Map(m)
    };
    let mut holder_pvs = Vec::new();
    for (i, h) in holders.iter().enumerate() {
        let mut m = DetMap::new();
        let mut effects = Vec::new();
        for e in elements(read(cx, *h, "ExternalAttributeEffects")?)? {
            let attribute =
                ow_attr::eval::object("AttributeToModify", &field(cx, &e, "AttributeToModify")?)?;
            let attribute = match attribute {
                Some(a) => Pv::Str(path_of(cx, a)),
                None => Pv::Null,
            };
            let kind = modifier_name(enum_value(&field(cx, &e, "ModifierType")?)?)?;
            let mut em = DetMap::new();
            em.insert("AttributeToModify".to_string(), attribute);
            em.insert("ModifierType".to_string(), Pv::Str(kind.to_string()));
            em.insert(
                "BaseModifierValue".to_string(),
                keep(field(cx, &e, "BaseModifierValue")?),
            );
            effects.push(Pv::Map(em));
        }
        m.insert("ExternalAttributeEffects".to_string(), Pv::List(effects));
        let mut upgrades = Vec::new();
        for u in elements(read(cx, *h, "AttributeSlotUpgrades")?)? {
            let mut um = DetMap::new();
            um.insert(
                "SlotName".to_string(),
                Pv::Str(name_field(cx, &u, "SlotName")?),
            );
            um.insert(
                "GradeIncrease".to_string(),
                Pv::Float(number_of(&field(cx, &u, "GradeIncrease")?)?),
            );
            um.insert(
                "bActivateSlot".to_string(),
                Pv::Bool(bool_of(&field(cx, &u, "bActivateSlot")?)),
            );
            upgrades.push(Pv::Map(um));
        }
        m.insert("AttributeSlotUpgrades".to_string(), Pv::List(upgrades));
        if i == 0 {
            if let Some(g) = read(cx, *h, "AttributeSlotBaseGrade")? {
                m.insert("AttributeSlotBaseGrade".to_string(), keep(g));
            }
            if let Some(n) = read(cx, *h, "AttributeSlotMaxActivated")? {
                // A byte or int property: the count of slots (anything else is no count: the class default).
                let count = match n {
                    Value::Int(i) => Pv::Int(i64::from(i)),
                    Value::Byte(b) => Pv::Int(i64::from(b)),
                    _ => Pv::Null,
                };
                m.insert("AttributeSlotMaxActivated".to_string(), count);
            }
            let mut slot_effects = Vec::new();
            for e in elements(read(cx, *h, "AttributeSlotEffects")?)? {
                let mut em = DetMap::new();
                em.insert(
                    "SlotName".to_string(),
                    Pv::Str(name_field(cx, &e, "SlotName")?),
                );
                let attribute = ow_attr::eval::object(
                    "AttributeToModify",
                    &field(cx, &e, "AttributeToModify")?,
                )?;
                em.insert(
                    "AttributeToModify".to_string(),
                    attribute.map_or(Pv::Null, |a| Pv::Str(path_of(cx, a))),
                );
                let kind = modifier_name(enum_value(&field(cx, &e, "ModifierType")?)?)?;
                em.insert("ModifierType".to_string(), Pv::Str(kind.to_string()));
                em.insert(
                    "BaseModifierValue".to_string(),
                    keep(field(cx, &e, "BaseModifierValue")?),
                );
                em.insert(
                    "PerGradeUpgrade".to_string(),
                    keep(field(cx, &e, "PerGradeUpgrade")?),
                );
                for k in [
                    "bExternalSlot",
                    "bRunEffectsAsSkill",
                    "bEnforceMaximumGrade",
                    "bEnforceMinimumGrade",
                ] {
                    em.insert(k.to_string(), Pv::Bool(bool_of(&field(cx, &e, k)?)));
                }
                for k in ["MaximumGrade", "MinimumGrade"] {
                    em.insert(k.to_string(), Pv::Float(number_of(&field(cx, &e, k)?)?));
                }
                slot_effects.push(Pv::Map(em));
            }
            m.insert("AttributeSlotEffects".to_string(), Pv::List(slot_effects));
        }
        holder_pvs.push(Pv::Map(m));
    }
    let definition = holder_pvs.first().cloned().unwrap_or(Pv::Null);
    item_rule(
        &holder_pvs,
        &definition,
        |init: &Pv| -> Result<f64, ItemError> {
            let Pv::Int(i) = init.at("heap_init") else {
                return Ok(0.0);
            };
            // `i` is an index this function wrote into `inits`.
            let v = usize::try_from(*i)
                .ok()
                .and_then(|i| inits.get(i))
                .cloned()
                .ok_or_else(|| {
                    ItemError::BadData("an init index outside the item's inits".into())
                })?;
            let data = init_of(cx, &v)?;
            Ok(f64::from(sys.evaluate_init(&data, Some(item), None, cx)?))
        },
    )
}

/// A weapon instance's stat: `D_Attributes.Weapon.<name>` read through `ow-attr` (`value_of`, the instance as the
/// source). `None` when the attribute's chain does not resolve on the instance.
pub fn instance_stat(
    weapon: ObjRef,
    name: &str,
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
) -> Result<Option<f64>, ItemError> {
    let attribute = find(cx, &DetMap::new(), &format!("{ATTR}{name}"))?;
    Ok(sys
        .value_of(attribute, Some(weapon), None, cx)?
        .map(|v| f64::from(v.value)))
}

/// The stats of a mode (rule R3): Reference reads the tag streams ([`weapon_attributes`], [`item_stats`]); Off
/// instantiates ([`instantiate_stats`]). A helper for callers that hold the mode as a value.
pub fn reads_tag_streams(mode: ParityMode) -> bool {
    mode == ParityMode::Reference
}

/// The item slot names (`prepare_vendors.ITEM_SLOTS`), for callers building an item's part list.
pub fn item_slot_names() -> [&'static str; 9] {
    ITEM_SLOTS
}

/// The item slot of a name, for definition data.
pub fn item_slot(name: &str) -> Option<ItemSlot> {
    ITEM_SLOTS
        .iter()
        .position(|s| *s == name)
        .and_then(ItemSlot::from_index)
}

#[cfg(test)]
pub(crate) mod tests;
