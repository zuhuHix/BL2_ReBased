//! Owner: lane W4-ROLL. The balance merge readings and the values the roll reads that the registry does not hold.
//!
//! **Two readings of a balance chain** ([`Reading`]). The registry keeps, per balance, the manufacturer list and the
//! inventory definition of the leaf-most step that sets them (as `prepare_vendors` and `item_roll.py` read it), and
//! writes the root-most value too when it differs (`manufacturers_root_first`, `inventory_definition_root_first`).
//! `weapon_recipe.merge` takes the root-most step. Both readings stay named functions here ([`leaf_most`],
//! [`root_most`]); [`reading_for`] chooses one per record, the one its oracle uses: root-most for a weapon balance
//! (`weapon_recipe.merge`), leaf-most for every other kind (`item_roll.py`). ITEM_REGISTRY.md section 4 counts where
//! they differ on the installed game: 12 class mod and 8 usable balances (manufacturer lists) and 1 usable balance
//! (definition), no weapon balance. Which one the game uses is not read (UNVERIFIED either way; NATIVE_LOOT.md's
//! `GetInventoryDefinitionForManufacturerGrade` answers from the balance itself first, which is the leaf-most step).
//! The part lists themselves are the registry's merge (definition lists, then each step's collection, root first).
//!
//! **The values** ([`RollData`]). The roll reads, besides the registry's records, the raw properties of balances,
//! definitions, parts, name parts, attributes, resolvers and the globals: `bInterpolateExpLevel`,
//! `bAllowInventoryDefToModifyPartWeight`, `AttributeSlotEffects`, `AttributeSlotUpgrades`, `BaseRarity`, `Rarity`,
//! `RarityLevelColors`, name parts' `PartName`, level window, `Priority` and `Expressions`, and the damage-type
//! selectors. The oracles read them from each object's tag stream (`ow-package --properties ... --property-offset 4
//! --array-schema <item_roll's schema lines>`), with the class defaults they assume where a property is untagged
//! (`Priority` 1, level window 1..100, `bInterpolateExpLevel` true). [`RollData`] reads them the same way, from the
//! same owner as the registry (the first holder in the tool's file order, decision 2026-11-15-15), so the census can
//! be compared with the oracle. Reading them from the definition store's heap objects (class defaults applied by the
//! store) is open: the store's body is not on main yet.
//!
//! **A copy, on purpose and for now.** The tag-stream decoder and `weapon_recipe.attribute_value` below repeat the
//! registry's private ones (`registry.rs`, W4-REGISTRY): the registry exposes no lookup, and this lane owns no edit of
//! that file. The schema differs (`item_roll.schema_lines`, a superset of the registry's), so the decoded values of
//! the properties both schemas type are the same. The copy goes once the registry exposes its census (handoff, open
//! item).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use ow_core::DetMap;
use ow_core::fmt::fmt_g;
use ow_core::num::{f32r, pow_f64};
use ow_package::package::Package;
use ow_package::props::{self, PropEntry, PropValue};
use ow_package::schema::SchemaTyper;

use crate::registry::{CensusInput, Pv};

/// `item_roll.schema_lines()`: `item_registry.SCHEMA_LINES`, then `prepare_stock_shield.SCHEMA_EXTRA`, then
/// `weapon_stats.SCHEMA_LINES`, one line per property name (the first wins), in that order.
pub const SCHEMA_LINES: &[&str] = &[
    "WeightedParts=StructProperty:WeightedPart",
    "ConsolidatedAttributeInitData=StructProperty:AttributeInitializationData",
    "Manufacturers=StructProperty:ManufacturerGradeData",
    "TitleList=ObjectProperty",
    "PrefixList=ObjectProperty",
    "Expressions=StructProperty:AttributeExpressionData",
    "ValueResolverChain=ObjectProperty",
    "DamageTypes=StructProperty:DamageTypeSelectorData",
    "BalancedItems=StructProperty:BalancedInventoryData",
    "ItemPools=StructProperty:ItemPoolInfo",
    "DefaultItemPoolList=StructProperty:ItemPoolInfo",
    "DefaultItemPoolIncludedLists=ObjectProperty",
    "CustomItemPoolList=StructProperty:ItemPoolInfo",
    "CustomItemPoolIncludedLists=ObjectProperty",
    "PlayThroughs=StructProperty:PlayThroughData",
    "ConditionalExpressionList=StructProperty:ConditionalExpression",
    "DefaultLoot=StructProperty:LootConfigurationData",
    "ItemAttachments=StructProperty:LootAttachmentData",
    "ExternalAttributeEffects=StructProperty:AttributeEffectData",
    "ClassModDefinitions=ObjectProperty",
    "Grades=StructProperty:GradeData",
    "AttributeSlotEffects=StructProperty:AttributeSlotEffectData",
    "AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData",
    "WeaponAttributeEffects=StructProperty:AttributeEffectData",
    "CustomPresentations=ObjectProperty",
    "WeaponCardAttributes=StructProperty:WeaponCardAttributeData",
    "WeaponCardPresentations=ObjectProperty",
    "DamageSurfaceChanceModifiers=StructProperty:StatusEffectChanceModifier",
    "RarityLevelColors=StructProperty:RarityLevelColor",
    "MappingData=StructProperty:WeaponStatusEffectPresentationMapping",
];

// ---- the two readings ----------------------------------------------------------------------------------------------

/// Which step of a balance chain gives its manufacturers and inventory definition (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// The leaf-most step that sets them: the registry's own fields, `item_roll.py`, `prepare_vendors`.
    LeafMost,
    /// The root-most step that sets them: `weapon_recipe.merge`.
    RootMost,
}

/// A balance's manufacturers (in list order, `None` for an entry naming none) and inventory definition under one
/// reading.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceChoice {
    pub manufacturers: Vec<Option<String>>,
    pub definition: Option<String>,
}

fn text(v: &Pv) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// The leaf-most reading: the registry record's `manufacturers` and `inventory_definition`.
pub fn leaf_most(record: &Pv) -> BalanceChoice {
    BalanceChoice {
        manufacturers: record
            .at("manufacturers")
            .items()
            .iter()
            .map(|m| text(m.at("manufacturer")))
            .collect(),
        definition: text(record.at("inventory_definition")),
    }
}

/// The root-most reading: the record's `manufacturers_root_first` and `inventory_definition_root_first` where the
/// registry wrote them (it does only where they differ from the leaf-most values), else the leaf-most values.
pub fn root_most(record: &Pv) -> BalanceChoice {
    let leaf = leaf_most(record);
    BalanceChoice {
        manufacturers: match record.get("manufacturers_root_first") {
            Some(list) => list.items().iter().map(text).collect(),
            None => leaf.manufacturers,
        },
        definition: match record.get("inventory_definition_root_first") {
            Some(v) => text(v),
            None => leaf.definition,
        },
    }
}

/// The weapon balance classes (`item_registry.WEAPON_BALANCES`).
pub const WEAPON_BALANCE_CLASSES: [&str; 2] =
    ["WeaponBalanceDefinition", "MissionWeaponBalanceDefinition"];

/// Whether the record is a weapon balance's (by its class, as the registry classifies it).
pub fn is_weapon_balance(record: &Pv) -> bool {
    record
        .at("class")
        .as_str()
        .is_some_and(|c| WEAPON_BALANCE_CLASSES.contains(&c))
}

/// The reading the record's oracle uses (module documentation).
pub fn reading_for(record: &Pv) -> Reading {
    if is_weapon_balance(record) {
        Reading::RootMost
    } else {
        Reading::LeafMost
    }
}

/// The record's values under `reading`.
pub fn choice(record: &Pv, reading: Reading) -> BalanceChoice {
    match reading {
        Reading::LeafMost => leaf_most(record),
        Reading::RootMost => root_most(record),
    }
}

/// (manufacturers differ, definition differs) between the two readings of one record.
pub fn readings_differ(record: &Pv) -> (bool, bool) {
    let (leaf, root) = (leaf_most(record), root_most(record));
    (
        leaf.manufacturers != root.manufacturers,
        leaf.definition != root.definition,
    )
}

// ---- the values ----------------------------------------------------------------------------------------------------

/// Where `weapon_recipe.attribute_value` raises (a definition no package holds or that does not decode, a value that is
/// not a number, `math.pow` refusing, init data nested too deep); its callers read that as "no value".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raises;

/// Why a path has no properties (the tool's `KeyError` or `RuntimeError`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Miss {
    /// No package holds the path.
    NoHolder,
    /// Its owner's tag stream did not decode (the reader's error text).
    Undecoded(String),
}

/// How many packages the lookup keeps loaded between lookups (decoding is grouped by owner file, and later reads
/// seldom need more than a few files).
const PACKAGES_KEPT: usize = 4;

/// How deep [`RollData::attribute_value`] follows init data into definitions before it gives up with "raises",
/// as the registry's copy does (decision 2026-11-15-15: the tool gets a `RecursionError`; the cut-off is ours,
/// UNVERIFIED).
const MAX_INIT_DEPTH: u32 = 200;

struct Lookup {
    classes: DetMap<String, String>,
    owner: DetMap<String, (PathBuf, i32)>,
    typer: SchemaTyper,
    packages: Vec<(PathBuf, Result<Arc<Package>, String>)>,
    cache: DetMap<String, Rc<Pv>>,
    bad: DetMap<String, String>,
}

/// The raw properties the roll reads (module documentation), from the same census the registry is built from.
pub struct RollData {
    lookup: RefCell<Lookup>,
}

impl std::fmt::Debug for RollData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RollData").finish_non_exhaustive()
    }
}

fn is_class_default(path: &str) -> bool {
    path.split('.').any(|s| s.starts_with("Default__"))
}

/// The last component of a class path.
pub fn short(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

fn error_text(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

impl RollData {
    /// The lookup over `input` (the registry's census input, [`CensusInput::from_store`]): each path's owner is its
    /// first holder, and a file exporting it twice gives its last export, as the registry reads it.
    pub fn new(input: &CensusInput) -> RollData {
        let mut typer = SchemaTyper::new_empty();
        // The lines are this module's constant and hold no repeat; a failure would be a bug the tests catch.
        let _ = typer.add_text(&SCHEMA_LINES.join("\n"));
        let mut classes = DetMap::new();
        let mut owner: DetMap<String, (PathBuf, i32)> = DetMap::new();
        for entry in &input.paths {
            if is_class_default(&entry.path) {
                continue;
            }
            for h in &entry.holders {
                match owner.get_mut(&entry.path) {
                    None => {
                        classes.insert(entry.path.clone(), h.class.clone());
                        owner.insert(entry.path.clone(), (h.file.clone(), h.export));
                    }
                    Some(o) if o.0 == h.file => o.1 = h.export,
                    Some(_) => {}
                }
            }
        }
        RollData {
            lookup: RefCell::new(Lookup {
                classes,
                owner,
                typer,
                packages: Vec::new(),
                cache: DetMap::new(),
                bad: DetMap::new(),
            }),
        }
    }

    /// Whether a package holds `path` (`path in census.owner`).
    pub fn holds(&self, path: &str) -> bool {
        self.lookup.borrow().owner.contains_key(path)
    }

    /// The class of `path`'s owner export (`census.classes.get(path, '')`).
    pub fn class_of(&self, path: &str) -> String {
        self.lookup
            .borrow()
            .classes
            .get(path)
            .cloned()
            .unwrap_or_default()
    }

    /// The paths below `prefix.` whose class ends with `class_suffix`, in path order.
    pub fn below_of_class(&self, prefix: &str, class_suffix: &str) -> Vec<String> {
        let lookup = self.lookup.borrow();
        let start = format!("{prefix}.");
        lookup
            .classes
            .range(start.clone()..)
            .take_while(|(p, _)| p.starts_with(&start))
            .filter(|(_, c)| c.ends_with(class_suffix))
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// `census.props(path)`: the decoded properties, or why there are none.
    pub fn props(&self, path: &str) -> Result<Rc<Pv>, Miss> {
        self.lookup.borrow_mut().props(path)
    }

    /// `census.get(path)`: the decoded properties, or `None`.
    pub fn get(&self, path: &str) -> Option<Rc<Pv>> {
        if path.is_empty() {
            return None;
        }
        self.props(path).ok()
    }

    /// `props_of(lookup, path)` of `item_roll.py`: the properties of a path value, or an empty dict.
    pub fn props_of(&self, path: &Pv) -> Rc<Pv> {
        path.as_str()
            .and_then(|p| self.get(p))
            .unwrap_or_else(|| Rc::new(Pv::Map(DetMap::new())))
    }

    /// `weapon_recipe.attribute_value(lookup, init, level, attributes)`: `Ok(None)` when the init needs runtime
    /// state, `Err(Raises)` where the tool raises (a definition no package holds or that does not decode, a value that
    /// is not a number, init data nested deeper than [`MAX_INIT_DEPTH`]). `attributes` gives attribute values by
    /// path. Order read from the game's evaluator (NATIVE_PROGRESSION.md section 1, UNVERIFIED in game); single
    /// precision throughout.
    pub fn attribute_value(
        &self,
        init: &Pv,
        attributes: &[(String, f64)],
    ) -> Result<Option<f64>, Raises> {
        self.attribute_value_at(init, attributes, 0)
            .map_err(|()| Raises)
    }

    fn attribute_value_at(
        &self,
        init: &Pv,
        attributes: &[(String, f64)],
        depth: u32,
    ) -> Result<Option<f64>, ()> {
        if depth > MAX_INIT_DEPTH {
            return Err(());
        }
        let definition = init.at("InitializationDefinition");
        let attribute = init.at("BaseValueAttribute");
        let mut base = if attribute.truthy() {
            attribute
                .as_str()
                .and_then(|a| attributes.iter().find(|(k, _)| k == a))
                .map(|(_, v)| f32r(*v))
        } else {
            let constant = init.at("BaseValueConstant");
            Some(f32r(if constant.truthy() {
                constant.number().ok_or(())?
            } else {
                0.0
            }))
        };
        let props = if definition.truthy() {
            let path = definition.as_str().ok_or(())?;
            Some(self.props(path).map_err(|_| ())?)
        } else {
            None
        };
        if let Some(p) = &props {
            if p.at("ConditionalInitialization").at("bEnabled").truthy() {
                return Ok(None);
            }
            let formula = p.at("ValueFormula");
            if formula.at("bEnabled").truthy() {
                let mut terms = [0.0; 4];
                for (slot, key) in terms
                    .iter_mut()
                    .zip(["Multiplier", "Level", "Power", "Offset"])
                {
                    let term = formula.at(key);
                    let empty = Pv::Map(DetMap::new());
                    let term = if term.truthy() { term } else { &empty };
                    match self.attribute_value_at(term, attributes, depth + 1)? {
                        Some(v) => *slot = v,
                        None => return Ok(None),
                    }
                }
                let mode = p.at("BaseValueMode");
                let mode = if mode.truthy() {
                    mode.as_str().unwrap_or("")
                } else {
                    SETS_BASE_VALUE
                };
                // The tool checks the mode before it evaluates the formula, so an unknown mode never raises.
                if base_value_mode(mode, Some(0.0), 0.0).is_none()
                    || (base.is_none() && mode != SETS_BASE_VALUE)
                {
                    return Ok(None);
                }
                let f = formula_value(terms[0], terms[1], terms[2], terms[3])?;
                match base_value_mode(mode, base, f) {
                    Some(Some(v)) => base = Some(f32r(v)),
                    _ => return Ok(None),
                }
            }
        }
        let Some(base) = base else {
            return Ok(None);
        };
        let scale = match init.get("BaseValueScaleConstant") {
            None | Some(Pv::Null) => 1.0,
            Some(s) => s.number().ok_or(())?,
        };
        let mut value = f32r(base * scale);
        let Some(p) = props else {
            return Ok(Some(value));
        };
        let clamp = p.at("RangeRestriction");
        // Python max(value, low): value unless low is larger.
        if clamp.at("bEnableMinValueRestriction").truthy()
            && clamp.at("MinValue").truthy()
            && let Some(low) =
                self.attribute_value_at(clamp.at("MinValue"), attributes, depth + 1)?
            && low > value
        {
            value = low;
        }
        if clamp.at("bEnableMaxValueRestriction").truthy()
            && clamp.at("MaxValue").truthy()
            && let Some(high) =
                self.attribute_value_at(clamp.at("MaxValue"), attributes, depth + 1)?
            && high < value
        {
            value = high;
        }
        Ok(Some(f32r(rounding(p.at("RoundingMode"), value))))
    }

    /// `item_registry.attribute_constant`: the `ConstantValue` of an attribute whose `ValueResolverChain` is exactly
    /// one `ConstantAttributeValueResolver`, else `None` (that the game reads it unchanged is UNVERIFIED).
    pub fn attribute_constant(&self, attribute: &Pv) -> Option<f64> {
        let props = attribute.as_str().and_then(|a| self.get(a))?;
        let chain = props.at("ValueResolverChain").items();
        if chain.len() != 1 {
            return None;
        }
        let resolver = chain[0].as_str()?;
        if !short(&self.class_of(resolver)).ends_with("ConstantAttributeValueResolver") {
            return None;
        }
        self.get(resolver)?.at("ConstantValue").number()
    }

    /// `item_roll.number`: a number as it is; an attribute init through [`RollData::attribute_value`], and, when
    /// that needs runtime state and the base is an attribute with a constant resolver, again with that constant as the
    /// attribute's value. `None` when it still needs runtime state or the tool would raise.
    pub fn number(&self, init: &Pv) -> Option<f64> {
        match init {
            Pv::Null => return None,
            Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => return init.number(),
            _ => {}
        }
        let value = self.attribute_value(init, &[]).ok()?;
        if value.is_some() {
            return value;
        }
        let attribute = init.at("BaseValueAttribute");
        if !attribute.truthy() {
            return None;
        }
        let constant = self.attribute_constant(attribute)?;
        let path = attribute.as_str()?.to_string();
        self.attribute_value(init, &[(path, constant)])
            .ok()
            .flatten()
    }
}

impl Lookup {
    fn package(&mut self, file: &std::path::Path) -> Result<Arc<Package>, String> {
        let at = match self.packages.iter().position(|(f, _)| f == file) {
            Some(at) => at,
            None => {
                let loaded = Package::load(file).map_err(|e| error_text(&e));
                self.packages.insert(0, (file.to_path_buf(), loaded));
                self.packages.truncate(PACKAGES_KEPT);
                0
            }
        };
        let entry = self.packages.remove(at);
        let result = entry.1.clone();
        self.packages.insert(0, entry);
        result
    }

    fn props(&mut self, path: &str) -> Result<Rc<Pv>, Miss> {
        if let Some(d) = self.cache.get(path) {
            return Ok(Rc::clone(d));
        }
        if let Some(e) = self.bad.get(path) {
            return Err(Miss::Undecoded(e.clone()));
        }
        let Some((file, export)) = self.owner.get(path).cloned() else {
            return Err(Miss::NoHolder);
        };
        let result = self
            .package(&file)
            .and_then(|pkg| decode(&pkg, export, &self.typer).map_err(|e| error_text(&e)));
        match result {
            Ok(d) => {
                let d = Rc::new(d);
                self.cache.insert(path.to_string(), Rc::clone(&d));
                Ok(d)
            }
            Err(e) => {
                self.bad.insert(path.to_string(), e.clone());
                Err(Miss::Undecoded(e))
            }
        }
    }
}

// ---- the decoder (a copy of the registry's, see the module documentation) -------------------------------------------

/// `ow-package --properties <export> --property-offset 4 --array-schema <schema>` as `weapon_recipe.decoded` reads
/// it: one value per name (a later tag of a name replaces an earlier one, so a static array keeps its last element).
fn decode(pkg: &Package, export: i32, typer: &SchemaTyper) -> Result<Pv, props::PropError> {
    props::property_range(pkg, export, 4)?;
    pkg.path_of(export)?;
    let walked = props::read_tagged(pkg, export, 4, typer)?;
    let mut map = DetMap::new();
    for e in walked.entries.iter().filter(|e| e.tag.is_some()) {
        map.insert(e.name.clone(), plain(pkg, &e.value)?);
    }
    Ok(Pv::Map(map))
}

/// A float as `json.loads` reads the C++ `setprecision(9)` text: an `int` when the text has no `.` and no exponent.
fn float_pv(x: f32) -> Pv {
    let text = fmt_g(f64::from(x), 9);
    if text.bytes().any(|b| matches!(b, b'.' | b'e' | b'E')) {
        Pv::Float(text.parse().unwrap_or(f64::from(x)))
    } else {
        text.parse().map(Pv::Int).unwrap_or(Pv::Float(f64::from(x)))
    }
}

/// `weapon_recipe.plain` of one value.
fn plain(pkg: &Package, value: &PropValue) -> Result<Pv, props::PropError> {
    Ok(match value {
        PropValue::Int(i) => Pv::Int(i64::from(*i)),
        PropValue::Float(x) => float_pv(*x),
        PropValue::Bool(b) => Pv::Bool(*b),
        PropValue::Byte(b) => Pv::Int(i64::from(*b)),
        PropValue::Enum(s) | PropValue::Name(s) | PropValue::Str(s) => Pv::Str(s.clone()),
        PropValue::Object(0) => Pv::Null,
        PropValue::Object(r) => Pv::Str(pkg.path_of(*r)?),
        PropValue::Struct { fields, .. } => {
            if !fields.is_empty() && fields.iter().all(|f| f.tag.is_none()) {
                let mut m = DetMap::new();
                for f in fields {
                    m.insert(f.name.clone(), plain(pkg, &f.value)?);
                }
                Pv::Map(m)
            } else {
                tags_pv(pkg, fields)?
            }
        }
        PropValue::Array(values) => Pv::List(
            values
                .iter()
                .map(|v| plain(pkg, v))
                .collect::<Result<_, _>>()?,
        ),
        // Raw and Delegate print as `null` with status "unsupported": `None`.
        _ => Pv::Null,
    })
}

/// A tag list as `plain` reads it: a dict by name, or `[]` when the list is empty.
fn tags_pv(pkg: &Package, entries: &[PropEntry]) -> Result<Pv, props::PropError> {
    if entries.is_empty() {
        return Ok(Pv::List(Vec::new()));
    }
    let mut m = DetMap::new();
    for e in entries.iter().filter(|e| e.tag.is_some()) {
        m.insert(e.name.clone(), plain(pkg, &e.value)?);
    }
    Ok(Pv::Map(m))
}

// ---- weapon_recipe.attribute_value's arithmetic (a copy of the registry's) --------------------------------------------

const SETS_BASE_VALUE: &str = "BASEVALUE_InitializationDefSetsBaseValue";

/// `weapon_recipe.BASE_VALUE_MODES` (EBaseValueMode, Engine.upk).
fn base_value_mode(mode: &str, b: Option<f64>, f: f64) -> Option<Option<f64>> {
    Some(match mode {
        "BASEVALUE_InitializationDefSetsBaseValue" => Some(f),
        "BASEVALUE_InitializationDefAddsToBaseValue" => b.map(|b| b + f),
        "BASEVALUE_InitializationDefScalesBaseValue" => b.map(|b| b * f),
        "BASEVALUE_InitializationDefOffsetByBaseValue" => b.map(|b| f - b),
        _ => return None,
    })
}

/// `weapon_recipe.formula_value`: Multiplier * (Level ^ Power + Offset), each step in single precision, the power
/// skipped at 1. `Err` where Python's `math.pow` raises.
fn formula_value(multiplier: f64, level: f64, power: f64, offset: f64) -> Result<f64, ()> {
    let raised = if power == 1.0 {
        level
    } else {
        if level == 0.0 && power < 0.0 {
            return Err(());
        }
        let r = pow_f64(level, power);
        if r.is_infinite() && level.is_finite() && power.is_finite() {
            return Err(());
        }
        f32r(r)
    };
    Ok(f32r(multiplier * f32r(raised + offset)))
}

/// `weapon_recipe.ROUNDING` (EAttributeInitializationRounding, Engine.upk).
fn rounding(mode: &Pv, x: f64) -> f64 {
    match mode.as_str() {
        Some("ATTRROUNDING_IntRound") => (x + 0.5).floor(),
        Some("ATTRROUNDING_IntFloor") => x.floor(),
        Some("ATTRROUNDING_IntCeil") => x.ceil(),
        _ => x,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn map(entries: Vec<(&str, Pv)>) -> Pv {
        Pv::Map(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    pub(crate) fn s(v: &str) -> Pv {
        Pv::Str(v.to_string())
    }

    /// An invented balance record in the registry's shape.
    fn record(
        class: &str,
        leaf: &[&str],
        root: Option<&[&str]>,
        def: &str,
        root_def: Option<Option<&str>>,
    ) -> Pv {
        let makers = leaf
            .iter()
            .map(|m| map(vec![("manufacturer", s(m)), ("grades", Pv::List(vec![]))]))
            .collect();
        let mut fields = vec![
            ("class", s(class)),
            ("manufacturers", Pv::List(makers)),
            ("inventory_definition", s(def)),
        ];
        if let Some(root) = root {
            fields.push((
                "manufacturers_root_first",
                Pv::List(root.iter().map(|m| s(m)).collect()),
            ));
        }
        if let Some(d) = root_def {
            fields.push(("inventory_definition_root_first", d.map_or(Pv::Null, s)));
        }
        map(fields)
    }

    #[test]
    fn the_two_readings_are_kept_and_one_is_chosen_per_record() {
        // A class mod balance whose leaf lists manufacturer A and whose root lists B and C (the registry's
        // `manufacturers_root_first`), as the 12 class mod balances of ITEM_REGISTRY.md section 4 do.
        let class_mod = record(
            "ClassModBalanceDefinition",
            &["Test.MakerA"],
            Some(&["Test.MakerB", "Test.MakerC"]),
            "Test.Def",
            None,
        );
        assert_eq!(
            leaf_most(&class_mod).manufacturers,
            vec![Some("Test.MakerA".to_string())]
        );
        assert_eq!(
            root_most(&class_mod).manufacturers,
            vec![
                Some("Test.MakerB".to_string()),
                Some("Test.MakerC".to_string())
            ]
        );
        assert_eq!(readings_differ(&class_mod), (true, false));
        assert_eq!(reading_for(&class_mod), Reading::LeafMost);
        assert_eq!(
            choice(&class_mod, reading_for(&class_mod)),
            leaf_most(&class_mod)
        );

        // A usable balance whose root-most definition differs (the one usable balance of section 4), including a
        // root-first value of None.
        let usable = record(
            "InventoryBalanceDefinition",
            &["Test.MakerA"],
            None,
            "Test.LeafDef",
            Some(None),
        );
        assert_eq!(readings_differ(&usable), (false, true));
        assert_eq!(root_most(&usable).definition, None);

        // A weapon balance takes the root-most values (weapon_recipe.merge); without differing values both agree.
        let weapon = record(
            "WeaponBalanceDefinition",
            &["Test.MakerA"],
            Some(&["Test.MakerB"]),
            "Test.Type",
            Some(Some("Test.RootType")),
        );
        assert_eq!(reading_for(&weapon), Reading::RootMost);
        let chosen = choice(&weapon, reading_for(&weapon));
        assert_eq!(chosen.manufacturers, vec![Some("Test.MakerB".to_string())]);
        assert_eq!(chosen.definition, Some("Test.RootType".to_string()));
        let plain = record(
            "WeaponBalanceDefinition",
            &["Test.MakerA"],
            None,
            "Test.Type",
            None,
        );
        assert_eq!(readings_differ(&plain), (false, false));
        assert_eq!(leaf_most(&plain), root_most(&plain));
    }

    #[test]
    fn the_schema_holds_one_line_per_property() {
        let mut seen: Vec<&str> = Vec::new();
        for line in SCHEMA_LINES {
            let name = line.split('=').next().unwrap();
            assert!(!seen.contains(&name), "{name} twice");
            seen.push(name);
        }
        let mut typer = SchemaTyper::new_empty();
        typer.add_text(&SCHEMA_LINES.join("\n")).unwrap();
        assert_eq!(typer.len(), SCHEMA_LINES.len());
        // The registry's lines come first and unchanged (a superset of the registry's typing).
        assert_eq!(
            &SCHEMA_LINES[..crate::registry::SCHEMA_LINES.len()],
            crate::registry::SCHEMA_LINES
        );
    }

    #[test]
    fn attribute_values_follow_the_oracles_subset() {
        let data = RollData::new(&CensusInput::new(PathBuf::from(".")));
        let k = |v: f64| {
            map(vec![
                ("BaseValueConstant", Pv::Float(v)),
                ("BaseValueScaleConstant", Pv::Float(2.0)),
            ])
        };
        assert_eq!(data.attribute_value(&k(1.5), &[]), Ok(Some(3.0)));
        // An attribute base needs runtime state unless a value is given for it.
        let a = map(vec![
            ("BaseValueAttribute", s("Test.Attr")),
            ("BaseValueScaleConstant", Pv::Float(1.0)),
        ]);
        assert_eq!(data.attribute_value(&a, &[]), Ok(None));
        assert_eq!(
            data.attribute_value(&a, &[("Test.Attr".to_string(), 4.0)]),
            Ok(Some(4.0))
        );
        // A definition no package holds raises; number() reads that as no value.
        let d = map(vec![("InitializationDefinition", s("Test.Missing"))]);
        assert_eq!(data.attribute_value(&d, &[]), Err(Raises));
        assert_eq!(data.number(&d), None);
        assert_eq!(data.number(&Pv::Int(3)), Some(3.0));
        assert_eq!(data.number(&Pv::Null), None);
    }
}
