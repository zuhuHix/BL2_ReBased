//! Owner: lane W4-REGISTRY. The item registry: every inventory balance of every kind, DLC included, with its base
//! chain, manufacturers and grade windows, definitions, merged part candidates per slot (weight source and game-stage
//! window, unevaluated), name parts and gestalt, plus the definitions, parts, part lists, name parts, manufacturers
//! and gestalts it names. It writes the `openwillow-item-registry/1` JSON of `tools/item_registry.py`, which is the
//! reference: [`Registry::to_json`] is meant to be byte-identical to its `registry.json` on the same install.
//!
//! **What is ported.** `build`, `balance_record`, `variant_slots`, `list_slots`, `describe_entries`, `init_value`,
//! `attribute_constant`, `grades_of`, `names_of` and `candidates_at` of the tool, with the helpers it imports
//! (`weapon_recipe.attribute_value`, `merge_slots`, `part_fragments`, `FLAT_WEIGHT`, `MIN_WEIGHT`) and the census
//! bookkeeping of `RegistryCensus.add` that the registry itself needs (owner, holders, first-holder class). The parts
//! of `census.json` that are counts of what the registry met are kept in [`CensusReport`]; the run timestamp, the
//! `unresolved` lists and the cross-checks against `weapon_recipe` and `prepare_vendors` (`check_*`) are not ported
//! (the Python tools they compare with are W4-ROLL's).
//!
//! **Where values come from.** The tool reads each object's tag stream (`ow-package --properties ... --property-offset
//! 4 --array-schema <SCHEMA_LINES>`) and writes what is tagged: an untagged property is `null`, an untagged index is
//! `-1`, and an attribute init written as `{"init": ...}` holds only its tagged fields. The definition store's heap
//! objects carry class and struct defaults instead, which would change those bytes. So values are read here the way
//! the tool reads them: the tag stream of the holder the census names, walked with [`SchemaTyper`] over the tool's
//! schema lines, and turned into the Python value the tool sees ([`Pv`], `weapon_recipe.plain`). Which holder owns a
//! path is the definition store's census (identity policy), not chosen here. Overlays patch heap objects and so do
//! not reach this reading: [`build_registry`] takes none (open item, see the handoff).
//!
//! **Rules.** Each rule keeps the evidence label of the tool and `docs/verification/ITEM_REGISTRY.md`: the weapon
//! merge was checked in game for part identity (WEAPON_PARTS_CATALOG.md); the item merge, the entry weights for items
//! and the game-stage windows for items are UNVERIFIED (see [`RULES`]). "The first package holding a path owns it" is
//! the tools' convention, UNVERIFIED as a game rule.
//!
//! **Differences from the tool that are not the tool's data** (none is expected on the installed game, each is
//! UNVERIFIED until the byte-identity run): a value the tool would crash on (an attribute init that is an empty
//! struct, a non-numeric constant) is read here as having no value; a negative base with a non-integral power goes
//! through the C `pow` (NaN) where Python would raise.

use std::cell::RefCell;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use ow_core::fmt::{fmt_g, fmt_py_repr, json_quote_py};
use ow_core::num::{f32r, pow_f64};
use ow_core::{DetMap, DetSet, OrderedMap};
use ow_defs::DefStore;
use ow_object::heap::ObjectSpace;
use ow_package::package::Package;
use ow_package::props::{self, PropEntry, PropValue};
use ow_package::schema::SchemaTyper;

use crate::ItemError;

pub const SCHEMA_TAG: &str = "openwillow-item-registry/1";
pub const SCHEMA_LINE: &str = "openwillow-item-registry/1: balances, definitions, parts, part_lists, name_parts, \
manufacturers, gestalts keyed by object path; candidates unevaluated (weights and stage windows as numbers or \
attribute init data); see the docstring of tools/item_registry.py";
/// The `tool` field: the registry names the reference tool whose shape it writes.
pub const TOOL: &str = "tools/item_registry.py";

/// `item_registry.SCHEMA_LINES`: `loot_pools.SCHEMA_LINES` (`weapon_recipe.SCHEMA_LINES` and its own), then
/// `prepare_vendors.SCHEMA_EXTRA`, then the grade list, without repeats, in that order.
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
];

/// `weapon_recipe.SLOTS`.
pub const WEAPON_SLOTS: [&str; 9] = [
    "Body",
    "Grip",
    "Barrel",
    "Sight",
    "Stock",
    "Elemental",
    "Accessory1",
    "Accessory2",
    "Material",
];
/// `prepare_vendors.ITEM_SLOTS`.
pub const ITEM_SLOTS: [&str; 9] = [
    "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Material",
];

/// Weight of an entry whose Manufacturers list is empty (`weapon_recipe.FLAT_WEIGHT`; read from native code for
/// weapons, NATIVE_WEAPON_RULES.md section 3; UNVERIFIED in game for items).
pub const FLAT_WEIGHT: f64 = 100.0;
/// Entries at or below this weight never become candidates (`weapon_recipe.MIN_WEIGHT`).
pub const MIN_WEIGHT: f64 = 1e-8;

/// The tool's `RULES`, written into the registry as they are.
pub const RULES: [&str; 9] = [
    "balance chain: BaseDefinition from the balance, leaf first, until a step has none",
    "manufacturers, InventoryDefinition, ClassModDefinitions: the leaf-most step that sets them (prepare_vendors)",
    "weapon part merge: WeaponPartListCollection per step, root first, weapon_recipe.merge_slots (checked in game for \
part identity, WEAPON_PARTS_CATALOG.md)",
    "item part merge: definition <Slot>Parts, then each step's collection, root first, weapon_recipe.merge_slots \
(UNVERIFIED for items: not confirmed in the running game)",
    "entry weight: flat 100 for an empty Manufacturers list, else the manufacturer's weight, else DefaultWeight \
(read from native code for weapons; UNVERIFIED in game for items)",
    "game-stage window per entry from the list's ConsolidatedAttributeInitData (UNVERIFIED for items)",
    "the relic per-type weight hook (WEAPON_PARTS_CATALOG.md section 2) is not read: relic weights are open",
    "name parts listed are the candidates (definition or weapon type lists, then every candidate part's lists); the \
choice among them is weapon_recipe.best_name_part and not made here",
    "first package holding a path owns it (base game, then DLC folders in name order); other holders are listed",
];

const WEAPON_BALANCES: [&str; 2] = ["WeaponBalanceDefinition", "MissionWeaponBalanceDefinition"];
const ITEM_BALANCES: [&str; 3] = [
    "InventoryBalanceDefinition",
    "ClassModBalanceDefinition",
    "ItemBalanceDefinition",
];
const DEFINITION_KINDS: [(&str, &str); 9] = [
    ("ShieldDefinition", "shield"),
    ("GrenadeModDefinition", "grenade_mod"),
    ("ClassModDefinition", "class_mod"),
    ("CrossDLCClassModDefinition", "class_mod"),
    ("ArtifactDefinition", "relic"),
    ("UsableItemDefinition", "usable"),
    ("UsableCustomizationItemDefinition", "customization"),
    ("MissionItemDefinition", "mission_item"),
    ("MarketingUnlockInventoryDefinition", "marketing_unlock"),
];
const PART_KINDS: [(&str, &str); 6] = [
    ("WeaponPartDefinition", "weapon"),
    ("ShieldPartDefinition", "shield"),
    ("GrenadeModPartDefinition", "grenade_mod"),
    ("ClassModPartDefinition", "class_mod"),
    ("ArtifactPartDefinition", "relic"),
    ("UsableItemPartDefinition", "usable"),
];
const LIST_CLASSES: [&str; 4] = [
    "ItemPartListDefinition",
    "ItemPartListCollectionDefinition",
    "WeaponPartListDefinition",
    "WeaponPartListCollectionDefinition",
];
const NAME_PART_CLASSES: [&str; 2] = ["ItemNamePartDefinition", "WeaponNamePartDefinition"];
const UNLISTED_SUFFIXES: [&str; 3] = [
    "BalanceDefinition",
    "PartListCollectionDefinition",
    "PartListDefinition",
];

// ---- the Python value model ----------------------------------------------------------------------------------------

/// A value as the reference tool holds it after `json.loads` of the `ow-package --properties` text and
/// `weapon_recipe.plain`: a reference is its path (or `None`), a tagged struct a dict of its tags, a fixed struct a
/// dict of its fields, an undecoded value `None`. A float the C++ printer writes without `.` or an exponent (`1`, `-2`)
/// is a Python `int` after `json.loads`, so it is [`Pv::Int`] here too. Dicts are key-ordered: the registry is
/// written with `sort_keys=True`, so no dict order of the tool reaches its bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Pv {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Pv>),
    Map(DetMap<String, Pv>),
}

impl Pv {
    /// Python truth.
    pub fn truthy(&self) -> bool {
        match self {
            Pv::Null => false,
            Pv::Bool(b) => *b,
            Pv::Int(i) => *i != 0,
            Pv::Float(x) => *x != 0.0,
            Pv::Str(s) => !s.is_empty(),
            Pv::List(v) => !v.is_empty(),
            Pv::Map(m) => !m.is_empty(),
        }
    }

    /// `dict.get(key)`: `None` for a missing key or a value that is not a dict (where the tool would raise, an empty
    /// struct read as a list is the only case met; it has no keys).
    pub fn get(&self, key: &str) -> Option<&Pv> {
        match self {
            Pv::Map(m) => m.get(key),
            _ => None,
        }
    }

    /// `dict.get(key)` with `None` as the value of a missing key.
    pub fn at(&self, key: &str) -> &Pv {
        self.get(key).unwrap_or(&Pv::Null)
    }

    /// The text of a string value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Pv::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The elements of a list (`value or []`: anything else is no elements).
    pub fn items(&self) -> &[Pv] {
        match self {
            Pv::List(v) => v,
            _ => &[],
        }
    }

    /// A Python number (`bool` counts, as in Python).
    pub fn number(&self) -> Option<f64> {
        match self {
            Pv::Int(i) => Some(*i as f64),
            Pv::Float(x) => Some(*x),
            Pv::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn text(s: &str) -> Pv {
        Pv::Str(s.to_string())
    }

    fn map<const N: usize>(entries: [(&str, Pv); N]) -> Pv {
        Pv::Map(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    /// `json.dumps(value, ensure_ascii=False, separators=(',', ':'), sort_keys=True)`.
    pub fn write_json(&self, out: &mut String) {
        match self {
            Pv::Null => out.push_str("null"),
            Pv::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Pv::Int(i) => out.push_str(&i.to_string()),
            Pv::Float(x) => out.push_str(&fmt_py_repr(*x)),
            Pv::Str(s) => out.push_str(&json_quote_py(s)),
            Pv::List(v) => {
                out.push('[');
                for (i, item) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_json(out);
                }
                out.push(']');
            }
            Pv::Map(m) => {
                out.push('{');
                for (i, (k, item)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&json_quote_py(k));
                    out.push(':');
                    item.write_json(out);
                }
                out.push('}');
            }
        }
    }

    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }
}

/// A dict key as `json.dumps` writes a non-string key (`None` -> `null`); a manufacturer row naming no manufacturer
/// is the only case.
fn key_text(v: &Pv) -> String {
    match v {
        Pv::Str(s) => s.clone(),
        Pv::Null => "null".to_string(),
        Pv::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
        Pv::Int(i) => i.to_string(),
        Pv::Float(x) => fmt_py_repr(*x),
        other => other.to_json(),
    }
}

/// A float as `json.loads` reads the C++ `setprecision(9)` text: an `int` when the text has no `.` and no exponent.
fn float_pv(x: f32) -> Pv {
    let text = fmt_g(f64::from(x), 9);
    if text.bytes().any(|b| matches!(b, b'.' | b'e' | b'E')) {
        // Finite (the walker refuses the rest), so the text parses; the fallback keeps the value itself.
        Pv::Float(text.parse().unwrap_or(f64::from(x)))
    } else {
        text.parse().map(Pv::Int).unwrap_or(Pv::Float(f64::from(x)))
    }
}

/// `weapon_recipe.plain` of one value of the `--properties` JSON.
fn plain(pkg: &Package, value: &PropValue) -> Result<Pv, props::PropError> {
    Ok(match value {
        PropValue::Int(i) => Pv::Int(i64::from(*i)),
        PropValue::Float(x) => float_pv(*x),
        PropValue::Bool(b) => Pv::Bool(*b),
        PropValue::Byte(b) => Pv::Int(i64::from(*b)),
        PropValue::Enum(s) | PropValue::Name(s) | PropValue::Str(s) => Pv::text(s),
        PropValue::Object(0) => Pv::Null,
        PropValue::Object(r) => Pv::Str(pkg.path_of(*r)?),
        PropValue::Struct { fields, .. } => {
            if !fields.is_empty() && fields.iter().all(|f| f.tag.is_none()) {
                // A fixed-layout struct: the C++ writes a dict of its fields, which `plain` keeps as it is.
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

/// A tag list as `plain` reads it: a dict by name (a later tag of a name replaces an earlier one), or `[]` when the
/// list is empty.
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

// ---- the census --------------------------------------------------------------------------------------------------

/// One file holding a path, with the class of its export there (`Package.Class`, `Class` for a class export).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub file: PathBuf,
    pub class: String,
    pub export: i32,
}

impl Holder {
    pub fn new(file: PathBuf, class: String, export: i32) -> Self {
        Holder {
            file,
            class,
            export,
        }
    }
}

/// An object path and every file holding it, in scan order (the first owns it under the store's policy).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathHolders {
    pub path: String,
    pub holders: Vec<Holder>,
}

impl PathHolders {
    pub fn new(path: String, holders: Vec<Holder>) -> Self {
        PathHolders { path, holders }
    }
}

/// What the registry needs from a scan of the install, in the tool's file order: every non-default path with its
/// holders, the `Default__` objects (path and class) apart, and the files that failed to load.
/// [`CensusInput::from_store`] makes it from `DefStore::census()`.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CensusInput {
    /// The install folder: package paths are written relative to it, and `WillowGame/CookedPCConsole` and `DLC/<x>`
    /// below it name the DLC folder of a file.
    pub root: PathBuf,
    /// Every file the tool lists (`weapon_catalog.package_files`, failed ones included), in its order: what
    /// `packages_scanned` counts and what orders `paths_with_two_classes`.
    pub files: Vec<PathBuf>,
    pub paths: Vec<PathHolders>,
    pub defaults: Vec<(String, String)>,
    /// Files that failed to load, with the text the tool keeps for them (`ow-package: <error>`, the reader's
    /// stderr line).
    pub load_failures: Vec<(PathBuf, String)>,
}

impl CensusInput {
    pub fn new(root: PathBuf) -> Self {
        CensusInput {
            root,
            ..Self::default()
        }
    }

    /// The definition store's census (`DefStore::census`, every file of the store's scan in its order) in the tool's
    /// file list and order (`weapon_catalog.package_files`): only `.upk` files, those below
    /// `WillowGame/CookedPCConsole` first, then those below `DLC/`, each group sorted as pathlib sorts. Within a path,
    /// holders keep their export order per file; paths are ordered by where the tool first meets them. So the owner
    /// of a path (its first holder) is the tool's "first package holding a path": `FirstInScanOrder` in the tool's
    /// scan order (UNVERIFIED as a game rule, as the tool's is).
    pub fn from_store(census: ow_defs::identity::Census, root: &Path) -> CensusInput {
        let mut ranked: Vec<((u8, Vec<String>), PathBuf)> = census
            .files
            .iter()
            .filter_map(|f| tool_file_key(f, root).map(|k| (k, f.clone())))
            .collect();
        ranked.sort();
        let files: Vec<PathBuf> = ranked.into_iter().map(|(_, f)| f).collect();
        let rank: DetMap<PathBuf, usize> = files
            .iter()
            .enumerate()
            .map(|(i, f)| (f.clone(), i))
            .collect();
        let holders_of = |owners: Vec<ow_defs::identity::Owner>| -> Vec<(usize, Holder)> {
            let mut kept: Vec<(usize, Holder)> = owners
                .into_iter()
                .filter_map(|o| {
                    rank.get(&o.file)
                        .map(|&r| (r, Holder::new(o.file, o.class, o.export)))
                })
                .collect();
            // Stable: the holders of one file keep their export order.
            kept.sort_by_key(|(r, _)| *r);
            kept
        };
        let mut input = CensusInput::new(root.to_path_buf());
        input.files = files.clone();
        let mut entries: Vec<((usize, i32), PathHolders)> = Vec::new();
        for entry in census.entries {
            let kept = holders_of(entry.holders);
            if let Some((r, first)) = kept.first() {
                let key = (*r, first.export);
                let holders = kept.into_iter().map(|(_, h)| h).collect();
                entries.push((key, PathHolders::new(entry.path, holders)));
            }
        }
        entries.sort_by_key(|(k, _)| *k);
        input.paths = entries.into_iter().map(|(_, e)| e).collect();
        let mut defaults: Vec<((usize, i32), (String, String))> = Vec::new();
        for entry in census.class_defaults {
            for (r, h) in holders_of(entry.holders) {
                defaults.push(((r, h.export), (entry.path.clone(), h.class)));
            }
        }
        defaults.sort_by_key(|(k, _)| *k);
        input.defaults = defaults.into_iter().map(|(_, d)| d).collect();
        input.load_failures = census
            .failures
            .into_iter()
            .filter(|f| rank.contains_key(&f.file))
            .map(|f| (f.file, format!("ow-package: {}", f.error)))
            .collect();
        input
    }
}

fn short(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

fn is_class_default(path: &str) -> bool {
    path.split('.').any(|s| s.starts_with("Default__"))
}

fn is_weapon_type(name: &str) -> bool {
    name.ends_with("WeaponTypeDefinition")
}

fn kind_of(table: &[(&'static str, &'static str)], name: &str) -> Option<&'static str> {
    table.iter().find(|(c, _)| *c == name).map(|(_, k)| *k)
}

fn definition_kind(name: &str) -> Option<&'static str> {
    if is_weapon_type(name) {
        Some("weapon")
    } else {
        kind_of(&DEFINITION_KINDS, name)
    }
}

fn is_balance(name: &str) -> bool {
    WEAPON_BALANCES.contains(&name) || ITEM_BALANCES.contains(&name)
}

/// The classes the registry lists (unqualified class name), `item_registry.relevant`.
fn relevant(name: &str) -> bool {
    is_balance(name)
        || definition_kind(name).is_some()
        || kind_of(&PART_KINDS, name).is_some()
        || LIST_CLASSES.contains(&name)
        || NAME_PART_CLASSES.contains(&name)
        || name == "ManufacturerDefinition"
        || name == "GestaltSkeletalMeshDefinition"
}

/// One decoded object: `decoded()` (one value per name, the last element of a static array) and `static_arrays()`
/// (every element of each name, in element order).
#[derive(Debug)]
struct Decoded {
    props: Pv,
    statics: DetMap<String, Vec<Pv>>,
}

/// One path seen twice with two classes (`paths_with_two_classes`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    pub first: String,
    pub second: String,
    pub second_package: PathBuf,
}

/// `RegistryCensus` (and the `weapon_catalog.Census` it extends): owner and class of every path, the holders of the
/// registry's paths, and the values read, plus the counts the census report keeps.
struct Census {
    root: PathBuf,
    order: Vec<String>,
    classes: DetMap<String, String>,
    owner: DetMap<String, (PathBuf, i32)>,
    holders: DetMap<String, Vec<PathBuf>>,
    class_defaults: DetMap<String, usize>,
    failed: DetMap<String, String>,
    null_part_entries: DetMap<String, usize>,
    null_name_entries: DetMap<String, usize>,
    unlisted_classes: DetMap<String, usize>,
    unlisted_part_data: DetMap<String, DetSet<String>>,
    conflicts: Vec<((usize, i32), Conflict)>,
    duplicates: usize,
    files: Vec<PathBuf>,
    rank: DetMap<PathBuf, usize>,
    load_failures: Vec<(PathBuf, String)>,
    typer: SchemaTyper,
    /// The last few packages read, most recent first: decoding is grouped by owner file ([`Census::preload`]), and
    /// what is read later (attribute definitions, resolvers) seldom needs more than a few files. Only decoded values
    /// stay for the run.
    packages: Vec<(PathBuf, Result<Arc<Package>, String>)>,
    cache: DetMap<String, Rc<Decoded>>,
    /// Paths that did not decode, with why (decoding is deterministic, so a failure is not retried).
    bad: DetMap<String, String>,
}

/// How many packages [`Census`] keeps loaded between lookups.
const PACKAGES_KEPT: usize = 4;

impl Census {
    /// `RegistryCensus.add` over every (path, class, file) of the scan, in scan order.
    fn new(input: CensusInput) -> Census {
        let mut typer = SchemaTyper::new_empty();
        // The lines are this module's constant and hold no repeat; a failure would be a bug caught by the tests.
        let _ = typer.add_text(&SCHEMA_LINES.join("\n"));
        let mut c = Census {
            root: input.root,
            order: Vec::new(),
            classes: DetMap::new(),
            owner: DetMap::new(),
            holders: DetMap::new(),
            class_defaults: DetMap::new(),
            failed: DetMap::new(),
            null_part_entries: DetMap::new(),
            null_name_entries: DetMap::new(),
            unlisted_classes: DetMap::new(),
            unlisted_part_data: DetMap::new(),
            conflicts: Vec::new(),
            duplicates: 0,
            rank: input
                .files
                .iter()
                .enumerate()
                .map(|(i, f)| (f.clone(), i))
                .collect(),
            files: input.files,
            load_failures: input.load_failures,
            typer,
            packages: Vec::new(),
            cache: DetMap::new(),
            bad: DetMap::new(),
        };
        for (_, class) in &input.defaults {
            let name = short(class);
            if relevant(name) {
                *c.class_defaults.entry(name.to_string()).or_default() += 1;
            }
        }
        for entry in input.paths {
            if is_class_default(&entry.path) {
                for h in &entry.holders {
                    let name = short(&h.class);
                    if relevant(name) {
                        *c.class_defaults.entry(name.to_string()).or_default() += 1;
                    }
                }
                continue;
            }
            for h in entry.holders {
                c.add(&entry.path, h);
            }
        }
        // `RegistryCensus.add` runs file by file, export by export: its conflicts come in that order.
        c.conflicts.sort_by_key(|(k, _)| *k);
        c
    }

    fn add(&mut self, path: &str, h: Holder) {
        let name = short(&h.class).to_string();
        let first = self.classes.get(path).cloned();
        let is_relevant = relevant(&name);
        if is_relevant && first.is_some() && !self.holders.contains_key(path) {
            // The first holder had a class the registry does not list.
            if let Some((file, _)) = self.owner.get(path) {
                let file = file.clone();
                self.holders.entry(path.to_string()).or_default().push(file);
            }
        }
        if is_relevant || self.holders.contains_key(path) {
            self.holders
                .entry(path.to_string())
                .or_default()
                .push(h.file.clone());
        }
        if let Some(first) = first {
            self.duplicates += 1;
            // The tool decodes a path from its owner file's `{path: index}` dict, so a file exporting the path twice
            // gives its last export (weapon_recipe.Package.index).
            if let Some(owner) = self.owner.get_mut(path)
                && owner.0 == h.file
            {
                owner.1 = h.export;
            }
            if short(&first) != name && (is_relevant || relevant(short(&first))) {
                let key = (
                    self.rank.get(&h.file).copied().unwrap_or(usize::MAX),
                    h.export,
                );
                let conflict = Conflict {
                    path: path.to_string(),
                    first,
                    second: h.class,
                    second_package: h.file,
                };
                self.conflicts.push((key, conflict));
            }
            return;
        }
        if !is_relevant && UNLISTED_SUFFIXES.iter().any(|s| name.ends_with(s)) {
            *self.unlisted_classes.entry(name).or_default() += 1;
        }
        self.order.push(path.to_string());
        self.classes.insert(path.to_string(), h.class);
        self.owner.insert(path.to_string(), (h.file, h.export));
    }

    fn class_of(&self, path: &str) -> &str {
        self.classes.get(path).map_or("", String::as_str)
    }

    fn package(&mut self, file: &Path) -> Result<Arc<Package>, String> {
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

    /// The tool's `preload`: decodes `paths` grouped by owner file, each file read once and dropped after its group.
    fn preload(&mut self, paths: &[String]) {
        let mut groups: DetMap<PathBuf, Vec<(String, i32)>> = DetMap::new();
        for path in paths {
            if self.cache.contains_key(path) || self.bad.contains_key(path) {
                continue;
            }
            if let Some((file, export)) = self.owner.get(path) {
                groups
                    .entry(file.clone())
                    .or_default()
                    .push((path.clone(), *export));
            }
        }
        for (file, group) in groups {
            match Package::load(&file) {
                Ok(pkg) => {
                    for (path, export) in group {
                        match decode(&pkg, export, &self.typer) {
                            Ok(d) => {
                                self.cache.insert(path, Rc::new(d));
                            }
                            Err(e) => {
                                self.bad.insert(path, error_text(&e));
                            }
                        }
                    }
                }
                Err(e) => {
                    let text = error_text(&e);
                    for (path, _) in group {
                        self.bad.insert(path, text.clone());
                    }
                }
            }
        }
    }

    /// `census.props`: the decoded object, or the reason it is not (no holder, or the walk failed).
    fn props(&mut self, path: &str) -> Result<Rc<Decoded>, String> {
        if let Some(d) = self.cache.get(path) {
            return Ok(Rc::clone(d));
        }
        if let Some(e) = self.bad.get(path) {
            return Err(e.clone());
        }
        let Some((file, export)) = self.owner.get(path).cloned() else {
            return Err(format!("{path} is not an export of any package"));
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
                Err(e)
            }
        }
    }

    /// `census.get`: the decoded properties, or `None` (the reason is kept in `failed`).
    fn get(&mut self, path: &str) -> Option<Rc<Decoded>> {
        if path.is_empty() || !self.owner.contains_key(path) {
            return None;
        }
        match self.props(path) {
            Ok(d) => Some(d),
            Err(e) => {
                self.failed
                    .insert(path.to_string(), e.chars().take(200).collect());
                None
            }
        }
    }

    /// `census.get(path)` on a value that should be a path; anything else has no properties.
    fn get_pv(&mut self, path: &Pv) -> Option<Rc<Decoded>> {
        path.as_str().and_then(|p| self.get(p))
    }

    /// `(census.get(path) or {}).get(key)`.
    fn prop(&mut self, path: &str, key: &str) -> Pv {
        self.get(path).map_or(Pv::Null, |d| d.props.at(key).clone())
    }

    fn rel(&self, path: &str) -> String {
        let Some((file, _)) = self.owner.get(path) else {
            return String::new();
        };
        relative_posix(file, &self.root)
    }

    fn dlc_of(&self, path: &str) -> String {
        match self.owner.get(path) {
            Some((file, _)) => dlc_of_file(file, &self.root),
            None => "?".to_string(),
        }
    }
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

/// `ow-package --properties <export> --property-offset 4 --array-schema <schema>`, as `decoded()` and
/// `static_arrays()` read it. Fails where the C++ command fails (the range checks, the export's path, the walk).
fn decode(pkg: &Package, export: i32, typer: &SchemaTyper) -> Result<Decoded, props::PropError> {
    props::property_range(pkg, export, 4)?;
    pkg.path_of(export)?;
    let walked = props::read_tagged(pkg, export, 4, typer)?;
    let mut map = DetMap::new();
    let mut statics: DetMap<String, Vec<(i32, Pv)>> = DetMap::new();
    for e in walked.entries.iter().filter(|e| e.tag.is_some()) {
        let value = plain(pkg, &e.value)?;
        let decoded = !matches!(e.value, PropValue::Raw(_) | PropValue::Delegate { .. });
        if decoded {
            statics
                .entry(e.name.clone())
                .or_default()
                .push((e.index, value.clone()));
        }
        map.insert(e.name.clone(), value);
    }
    let statics = statics
        .into_iter()
        .map(|(k, mut v)| {
            // Python's sort is stable: equal indices keep their order.
            v.sort_by_key(|(i, _)| *i);
            (k, v.into_iter().map(|(_, x)| x).collect())
        })
        .collect();
    Ok(Decoded {
        props: Pv::Map(map),
        statics,
    })
}

/// `Path(file).relative_to(root).as_posix()`.
fn relative_posix(file: &Path, root: &Path) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Where `weapon_catalog.package_files` puts a file, or `None` for a file the tool does not list: group 0 for a
/// `.upk` below `WillowGame/CookedPCConsole`, 1 for a `.upk` below `DLC/`, then the sort key of its path. On Windows
/// the tool's paths compare and match case-insensitively (pathlib folds each part), elsewhere as they are.
fn tool_file_key(file: &Path, root: &Path) -> Option<(u8, Vec<String>)> {
    let is_upk = file.extension().is_some_and(|e| {
        let e = e.to_string_lossy();
        if cfg!(windows) {
            e.eq_ignore_ascii_case("upk")
        } else {
            e == "upk"
        }
    });
    if !is_upk {
        return None;
    }
    let rel = file.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let fold = |p: &str| {
        if cfg!(windows) {
            p.to_lowercase()
        } else {
            p.to_string()
        }
    };
    let key: Vec<String> = parts.iter().map(|p| fold(p)).collect();
    if parts.len() >= 3 && key[0] == fold("WillowGame") && key[1] == fold("CookedPCConsole") {
        return Some((0, key));
    }
    if parts.len() >= 2 && key[0] == fold("DLC") {
        return Some((1, key));
    }
    None
}

/// `weapon_catalog.package_files`' DLC name of a file: `base` below `WillowGame/CookedPCConsole`, the first part
/// below `DLC/` as it is on disk, `?` for a file the tool does not list.
fn dlc_of_file(file: &Path, root: &Path) -> String {
    match tool_file_key(file, root) {
        Some((0, _)) => "base".to_string(),
        Some(_) => file
            .strip_prefix(root)
            .ok()
            .and_then(|rel| rel.components().nth(1))
            .map_or("?".to_string(), |c| {
                c.as_os_str().to_string_lossy().into_owned()
            }),
        None => "?".to_string(),
    }
}

// ---- values ------------------------------------------------------------------------------------------------------

/// `weapon_recipe.f32` on a number.
fn f32n(x: f64) -> f64 {
    f32r(x)
}

const SETS_BASE_VALUE: &str = "BASEVALUE_InitializationDefSetsBaseValue";

/// The base-value modes of `weapon_recipe.BASE_VALUE_MODES` (EBaseValueMode, Engine.upk).
fn base_value_mode(mode: &str, b: Option<f64>, f: f64) -> Option<Option<f64>> {
    Some(match mode {
        "BASEVALUE_InitializationDefSetsBaseValue" => Some(f),
        "BASEVALUE_InitializationDefAddsToBaseValue" => b.map(|b| b + f),
        "BASEVALUE_InitializationDefScalesBaseValue" => b.map(|b| b * f),
        "BASEVALUE_InitializationDefOffsetByBaseValue" => b.map(|b| f - b),
        _ => return None,
    })
}

/// `weapon_recipe.formula_value`: Multiplier * (Level ^ Power + Offset), each step rounded to single precision; the
/// power is skipped when it is 1 (NATIVE_PROGRESSION.md section 1; read from native code, UNVERIFIED in game).
/// `Err` where Python's `math.pow` raises (a zero base with a negative power, an overflow).
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
        f32n(r)
    };
    Ok(f32n(multiplier * f32n(raised + offset)))
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

/// How deep [`attribute_value`] follows init data into definitions (formula terms, range bounds) before it gives
/// up with `Err`, where the tool gets a `RecursionError` (a `RuntimeError`, which its callers catch as "no value").
/// Cyclic init data reaches either limit; acyclic data deeper than this and shallower than Python's stack limit
/// would differ. The cut-off is ours and UNVERIFIED: Python's depends on its recursion limit and stack.
const MAX_INIT_DEPTH: u32 = 200;

/// `weapon_recipe.attribute_value` with no attribute values given: `Ok(None)` when the init needs runtime state,
/// `Err` where the tool raises (a definition no package holds or that does not decode, or init data nested deeper
/// than [`MAX_INIT_DEPTH`]). Order read from the game's evaluator (NATIVE_PROGRESSION.md section 1, UNVERIFIED in
/// game); single precision throughout.
fn attribute_value(census: &mut Census, init: &Pv) -> Result<Option<f64>, ()> {
    attribute_value_at(census, init, 0)
}

fn attribute_value_at(census: &mut Census, init: &Pv, depth: u32) -> Result<Option<f64>, ()> {
    if depth > MAX_INIT_DEPTH {
        return Err(());
    }
    let definition = init.at("InitializationDefinition");
    let mut base = if init.at("BaseValueAttribute").truthy() {
        None
    } else {
        let constant = init.at("BaseValueConstant");
        Some(f32n(if constant.truthy() {
            constant.number().ok_or(())?
        } else {
            0.0
        }))
    };
    let props = if definition.truthy() {
        let path = definition.as_str().ok_or(())?;
        Some(census.props(path).map_err(|_| ())?)
    } else {
        None
    };
    if let Some(d) = &props {
        let p = &d.props;
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
                match attribute_value_at(census, term, depth + 1)? {
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
                Some(Some(v)) => base = Some(f32n(v)),
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
    let mut value = f32n(base * scale);
    let Some(d) = props else {
        return Ok(Some(value));
    };
    let clamp = d.props.at("RangeRestriction");
    // Python max(value, low): value unless low is larger.
    if clamp.at("bEnableMinValueRestriction").truthy()
        && clamp.at("MinValue").truthy()
        && let Some(low) = attribute_value_at(census, clamp.at("MinValue"), depth + 1)?
        && low > value
    {
        value = low;
    }
    if clamp.at("bEnableMaxValueRestriction").truthy()
        && clamp.at("MaxValue").truthy()
        && let Some(high) = attribute_value_at(census, clamp.at("MaxValue"), depth + 1)?
        && high < value
    {
        value = high;
    }
    Ok(Some(f32n(rounding(d.props.at("RoundingMode"), value))))
}

/// `item_registry.attribute_constant`: the ConstantValue of an attribute whose ValueResolverChain is exactly one
/// ConstantAttributeValueResolver. A hint for later lanes, not applied (that the game reads it unchanged is
/// UNVERIFIED).
fn attribute_constant(census: &mut Census, attribute: &Pv) -> Pv {
    let props = if attribute.truthy() {
        census.get_pv(attribute)
    } else {
        None
    };
    let chain = props.map_or(Pv::Null, |d| d.props.at("ValueResolverChain").clone());
    let chain = chain.items();
    if chain.len() != 1 {
        return Pv::Null;
    }
    let class = chain[0].as_str().map_or("", |p| census.class_of(p));
    if !short(class).ends_with("ConstantAttributeValueResolver") {
        return Pv::Null;
    }
    census
        .get_pv(&chain[0])
        .map_or(Pv::Null, |d| d.props.at("ConstantValue").clone())
}

/// `item_registry.init_value`: one ConsolidatedAttributeInitData entry as a number, `{"init": data}` when it needs
/// runtime state, `None` without a valid index.
fn init_value(census: &mut Census, consolidated: &[Pv], index: &Pv) -> Pv {
    let Pv::Int(i) = index else {
        return Pv::Null;
    };
    let Some(init) = usize::try_from(*i).ok().and_then(|i| consolidated.get(i)) else {
        return Pv::Null;
    };
    if let Ok(Some(v)) = attribute_value(census, init) {
        return Pv::Float(v);
    }
    let mut out = DetMap::new();
    out.insert("init".to_string(), init.clone());
    let constant = attribute_constant(census, init.at("BaseValueAttribute"));
    if constant != Pv::Null && !init.at("InitializationDefinition").truthy() {
        out.insert("attribute_constant".to_string(), constant);
    }
    Pv::Map(out)
}

/// `dict.get(key, -1)`.
fn index_or_minus_one(row: &Pv, key: &str) -> Pv {
    row.get(key).cloned().unwrap_or(Pv::Int(-1))
}

/// `item_registry.value_at`: a registry value at one game stage, or `None`.
fn value_at(census: &mut Census, value: &Pv) -> Option<f64> {
    match value {
        Pv::Null => None,
        Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => value.number(),
        // `value['init']`: a dict without it is a KeyError, which the tool reads as no value.
        other => {
            let init = other.get("init")?.clone();
            attribute_value(census, &init).ok().flatten()
        }
    }
}

// ---- part lists ----------------------------------------------------------------------------------------------------

/// `item_registry.describe_entries`: WeightedParts rows -> registry candidates (unevaluated).
fn describe_entries(census: &mut Census, rows: &Pv, consolidated: &[Pv], list: &str) -> Vec<Pv> {
    let mut out = Vec::new();
    for row in rows.items() {
        if !row.at("Part").truthy() {
            *census
                .null_part_entries
                .entry(list.to_string())
                .or_default() += 1;
            continue;
        }
        let makers = row.at("Manufacturers");
        let weight = if makers.truthy() {
            let mut by = DetMap::new();
            for m in makers.items() {
                let v = init_value(
                    census,
                    consolidated,
                    &index_or_minus_one(m, "DefaultWeightIndex"),
                );
                by.insert(key_text(m.at("Manufacturer")), v);
            }
            let default = init_value(
                census,
                consolidated,
                &index_or_minus_one(row, "DefaultWeightIndex"),
            );
            Pv::map([
                ("source", Pv::text("manufacturer")),
                ("by_manufacturer", Pv::Map(by)),
                ("default", default),
            ])
        } else {
            Pv::map([
                ("source", Pv::text("flat")),
                ("value", Pv::Float(FLAT_WEIGHT)),
            ])
        };
        let min = init_value(
            census,
            consolidated,
            &index_or_minus_one(row, "MinGameStageIndex"),
        );
        let max = init_value(
            census,
            consolidated,
            &index_or_minus_one(row, "MaxGameStageIndex"),
        );
        out.push(Pv::map([
            ("part", row.at("Part").clone()),
            ("list", Pv::text(list)),
            ("weight", weight),
            ("min_stage", min),
            ("max_stage", max),
        ]));
    }
    out
}

type Slots = OrderedMap<String, Vec<Pv>>;

/// `item_registry.list_slots`: the enabled `<Slot>PartData` of one collection and its mode, or `None`.
fn list_slots(census: &mut Census, path: &str, slots: &[&str]) -> Option<(Slots, Pv)> {
    let d = census.get(path)?;
    let p = &d.props;
    let consolidated = p.at("ConsolidatedAttributeInitData").items().to_vec();
    if let Pv::Map(m) = p {
        for key in m.keys() {
            if let Some(slot) = key.strip_suffix("PartData")
                && !slots.contains(&slot)
            {
                census
                    .unlisted_part_data
                    .entry(key.clone())
                    .or_default()
                    .insert(path.to_string());
            }
        }
    }
    let mut found = Slots::new();
    for slot in slots {
        let data = p.at(&format!("{slot}PartData"));
        if data.truthy() && data.at("bEnabled").truthy() {
            let entries = describe_entries(census, data.at("WeightedParts"), &consolidated, path);
            found.insert((*slot).to_string(), entries);
        }
    }
    let mode = p.at("PartReplacementMode");
    let mode = if mode.truthy() {
        mode.clone()
    } else {
        Pv::text("EPRM_Additive")
    };
    Some((found, mode))
}

/// `item_registry.single_list`: the candidates of one ItemPartListDefinition, or `None`.
fn single_list(census: &mut Census, path: &str) -> Option<Vec<Pv>> {
    let d = census.get(path)?;
    let consolidated = d.props.at("ConsolidatedAttributeInitData").items().to_vec();
    Some(describe_entries(
        census,
        d.props.at("WeightedParts"),
        &consolidated,
        path,
    ))
}

/// `weapon_recipe.merge_slots`: one part list's enabled slots on top of `merged` (Complete replaces every slot,
/// Additive appends, anything else replaces the slot; UNVERIFIED rule, see the tool).
fn merge_slots(merged: &Slots, slots: &Slots, mode: &Pv) -> Slots {
    let mode = mode.as_str().unwrap_or("");
    let mut out = if mode == "EPRM_Complete" {
        Slots::new()
    } else {
        merged.clone()
    };
    for (slot, entries) in slots.iter() {
        let next = if mode == "EPRM_Additive" {
            let mut v = out.get(slot.as_str()).cloned().unwrap_or_default();
            v.extend(entries.iter().cloned());
            v
        } else {
            entries.clone()
        };
        out.insert(slot.clone(), next);
    }
    out
}

/// `item_registry.variant_slots`: the merged slots of one definition (`None`: the collections only), the merge
/// history, and the part lists that did not decode.
fn variant_slots(
    census: &mut Census,
    steps_root_first: &[String],
    definition: &Pv,
    weapon: bool,
) -> (Pv, Vec<Pv>, Vec<(String, Pv)>) {
    let mut merged = Slots::new();
    let mut history = Vec::new();
    let mut problems = Vec::new();
    let slots: &[&str] = if weapon { &WEAPON_SLOTS } else { &ITEM_SLOTS };
    if definition.truthy() && !weapon {
        let props = census
            .get_pv(definition)
            .map_or(Pv::Null, |d| d.props.clone());
        for slot in ITEM_SLOTS {
            let source = props.at(&format!("{slot}Parts"));
            if !source.truthy() {
                continue;
            }
            let entries = source.as_str().and_then(|s| single_list(census, s));
            let Some(entries) = entries else {
                problems.push(("part list".to_string(), source.clone()));
                continue;
            };
            merged.insert(slot.to_string(), entries);
            history.push(Pv::map([
                ("source", definition.clone()),
                ("list", source.clone()),
                ("mode", Pv::text("definition")),
                ("slots", Pv::List(vec![Pv::text(slot)])),
            ]));
        }
    }
    for step in steps_root_first {
        let collection = if weapon {
            census.prop(step, "WeaponPartListCollection")
        } else {
            let first = census.prop(step, "PartListCollection");
            if first.truthy() {
                first
            } else {
                census.prop(step, "ItemPartListCollection")
            }
        };
        if !collection.truthy() {
            continue;
        }
        let list = collection.as_str().unwrap_or("");
        // The collection's own class names its slots (an inventory balance naming a weapon type can hold a weapon
        // part list collection).
        let is_weapon_list = short(census.class_of(list)) == "WeaponPartListCollectionDefinition";
        let read_slots: &[&str] = if is_weapon_list { &WEAPON_SLOTS } else { slots };
        let Some((found, mode)) = list_slots(census, list, read_slots) else {
            problems.push(("part list".to_string(), collection.clone()));
            continue;
        };
        let mut names: Vec<String> = found.keys().cloned().collect();
        names.sort();
        history.push(Pv::map([
            ("source", Pv::text(step)),
            ("list", collection.clone()),
            ("mode", mode.clone()),
            ("slots", Pv::List(names.into_iter().map(Pv::Str).collect())),
        ]));
        merged = merge_slots(&merged, &found, &mode);
    }
    let mut out = DetMap::new();
    for slot in slots.iter().chain(WEAPON_SLOTS.iter()) {
        if let Some(entries) = merged.get(*slot) {
            out.insert((*slot).to_string(), Pv::List(entries.clone()));
        }
    }
    (Pv::Map(out), history, problems)
}

// ---- balances ------------------------------------------------------------------------------------------------------

/// `item_registry.walk_chain`: (steps leaf first, end, missing path).
fn walk_chain(census: &mut Census, balance: &str) -> (Vec<String>, &'static str, Pv) {
    let mut steps: Vec<String> = Vec::new();
    let mut cursor = Pv::text(balance);
    loop {
        let path = cursor.as_str().unwrap_or("").to_string();
        if cursor.as_str().is_some() && steps.contains(&path) {
            return (steps, "cycle", cursor);
        }
        if !census.owner.contains_key(&path) {
            return (steps, "missing", cursor);
        }
        let Some(d) = census.get(&path) else {
            return (steps, "decode_failed", cursor);
        };
        steps.push(path);
        cursor = d.props.at("BaseDefinition").clone();
        if !cursor.truthy() {
            return (steps, "root", Pv::Null);
        }
    }
}

/// `item_registry.first_set`: the first step's value of `key` that is set and not empty, and that step.
fn first_set<'a, I>(census: &mut Census, steps: I, key: &str) -> (Pv, Pv)
where
    I: IntoIterator<Item = &'a String>,
{
    for step in steps {
        let value = census.prop(step, key);
        if value.truthy() {
            return (value, Pv::text(step));
        }
    }
    (Pv::Null, Pv::Null)
}

/// `item_registry.grades_of`.
fn grades_of(census: &mut Census, maker: &Pv) -> Vec<Pv> {
    let mut rows = Vec::new();
    for grade in maker.at("Grades").items() {
        let need = grade.at("GameStageRequirement");
        let modifiers = grade.at("GradeModifiers");
        let mut spawn = DetMap::new();
        for (key, field) in [
            ("min", "MinSpawnProbabilityModifier"),
            ("max", "MaxSpawnProbabilityModifier"),
        ] {
            let value = if grade.at(field).truthy() {
                init_value(census, &[grade.at(field).clone()], &Pv::Int(0))
            } else {
                Pv::Null
            };
            spawn.insert(key.to_string(), value);
        }
        rows.push(Pv::map([
            ("min_stage", need.at("MinGameStage").clone()),
            ("max_stage", need.at("MaxGameStage").clone()),
            ("exp_level", modifiers.at("ExpLevel").clone()),
            (
                "definition",
                modifiers.at("CustomInventoryDefinition").clone(),
            ),
            ("spawn_probability", Pv::Map(spawn)),
        ]));
    }
    rows
}

/// `list(dict.fromkeys(values))`.
fn unique(values: impl IntoIterator<Item = Pv>) -> Vec<Pv> {
    let mut out: Vec<Pv> = Vec::new();
    for v in values {
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// `item_registry.balance_record`: the record and the part lists that did not decode.
fn balance_record(census: &mut Census, path: &str) -> (DetMap<String, Pv>, Vec<(String, Pv)>) {
    let cls = short(census.class_of(path)).to_string();
    let weapon = WEAPON_BALANCES.contains(&cls.as_str());
    let (steps, end, missing) = walk_chain(census, path);
    let root_first: Vec<String> = steps.iter().rev().cloned().collect();
    let (makers, makers_from) = first_set(census, &steps, "Manufacturers");
    let (makers_root, _) = first_set(census, &root_first, "Manufacturers");
    let (definition, _) = first_set(census, &steps, "InventoryDefinition");
    let (definition_root, _) = first_set(census, &root_first, "InventoryDefinition");
    let (class_mods, _) = first_set(census, &steps, "ClassModDefinitions");
    let mut manufacturers = Vec::new();
    for m in makers.items() {
        let grades = grades_of(census, m);
        manufacturers.push(Pv::map([
            ("manufacturer", m.at("Manufacturer").clone()),
            ("grades", Pv::List(grades)),
        ]));
    }
    let mut definitions: Vec<Pv> = Vec::new();
    let grade_defs: Vec<Pv> = manufacturers
        .iter()
        .flat_map(|m| {
            m.at("grades")
                .items()
                .iter()
                .map(|g| g.at("definition").clone())
        })
        .collect();
    for value in std::iter::once(definition.clone())
        .chain(class_mods.items().iter().cloned())
        .chain(grade_defs)
    {
        if value.truthy() && !definitions.contains(&value) {
            definitions.push(value);
        }
    }
    let variant_defs = if weapon {
        vec![definition.clone()]
    } else if class_mods.truthy() {
        unique(class_mods.items().iter().cloned())
    } else {
        unique(
            std::iter::once(definition.clone())
                .chain(definitions.iter().filter(|d| **d != definition).cloned()),
        )
    };
    let mut variants = Vec::new();
    let mut problems = Vec::new();
    for item in &variant_defs {
        let (slots, history, trouble) = variant_slots(census, &root_first, item, weapon);
        problems.extend(trouble);
        variants.push(Pv::map([
            ("definition", item.clone()),
            ("slots", slots),
            ("merge", Pv::List(history)),
        ]));
    }
    let kinds: DetSet<&str> = definitions
        .iter()
        .filter_map(|d| definition_kind(short(d.as_str().map_or("", |p| census.class_of(p)))))
        .collect();
    let kind = if weapon {
        "weapon".to_string()
    } else if kinds.len() == 1 && kinds.contains("weapon") {
        // An inventory balance naming a weapon type (vehicle, turret, enemy weapons).
        "weapon_by_inventory_balance".to_string()
    } else if cls == "ClassModBalanceDefinition" {
        "class_mod".to_string()
    } else if kinds.len() == 1 {
        kinds
            .iter()
            .next()
            .map_or(String::new(), |k| (*k).to_string())
    } else if kinds.is_empty() {
        "no_definition".to_string()
    } else {
        "mixed".to_string()
    };
    let gestalt = if definition.truthy() {
        census
            .get_pv(&definition)
            .map_or(Pv::Null, |d| d.props.at("GestaltMesh").clone())
    } else {
        Pv::Null
    };
    let mut record = DetMap::new();
    let mut put = |k: &str, v: Pv| {
        record.insert(k.to_string(), v);
    };
    put("class", Pv::text(&cls));
    put("kind", Pv::Str(kind));
    put("package", Pv::Str(census.rel(path)));
    put("dlc", Pv::Str(census.dlc_of(path)));
    put(
        "packages_holding",
        Pv::Int(census.holders.get(path).map_or(0, |h| h.len() as i64)),
    );
    put(
        "chain",
        Pv::List(steps.iter().map(|s| Pv::text(s)).collect()),
    );
    put("chain_end", Pv::text(end));
    put("chain_missing", missing);
    put("manufacturers", Pv::List(manufacturers));
    put("manufacturers_from", makers_from);
    put("inventory_definition", definition.clone());
    put(
        "class_mod_definitions",
        Pv::List(class_mods.items().to_vec()),
    );
    put("definitions", Pv::List(definitions));
    put("gestalt", gestalt);
    put("variants", Pv::List(variants));
    if definition_root != definition {
        put("inventory_definition_root_first", definition_root);
    }
    if makers_root != makers {
        put(
            "manufacturers_root_first",
            Pv::List(
                makers_root
                    .items()
                    .iter()
                    .map(|m| m.at("Manufacturer").clone())
                    .collect(),
            ),
        );
    }
    (record, problems)
}

/// `item_registry.names_of`: prefix and title lists without empty entries (each empty entry is counted).
fn names_of(census: &mut Census, path: &str, props: &Pv) -> (Vec<Pv>, Vec<Pv>) {
    let mut out = Vec::new();
    for key in ["PrefixList", "TitleList"] {
        let values = props.at(key).items();
        let empty = values.iter().filter(|v| !v.truthy()).count();
        *census
            .null_name_entries
            .entry(path.to_string())
            .or_default() += empty;
        out.push(
            values
                .iter()
                .filter(|v| v.truthy())
                .cloned()
                .collect::<Vec<_>>(),
        );
    }
    let title = out.pop().unwrap_or_default();
    let prefix = out.pop().unwrap_or_default();
    (prefix, title)
}

/// `weapon_recipe.part_fragments`: the gestalt fragment names one part draws (UNVERIFIED rule, matched on the 6
/// slice guns, WEAPON_VISUALS.md).
fn part_fragments(path: &str, d: &Decoded) -> Vec<Pv> {
    let last = path.rsplit('.').next().unwrap_or(path);
    if last.to_lowercase().ends_with("_none") {
        return Vec::new();
    }
    let mut names = vec![d.props.at("GestaltModeSkeletalMeshName").clone()];
    if let Some(extra) = d.statics.get("AdditionalGestaltModeSkeletalMeshNames") {
        names.extend(extra.iter().cloned());
    }
    names
        .into_iter()
        .filter(|n| n.truthy() && n.as_str() != Some("None"))
        .collect()
}

// ---- the registry --------------------------------------------------------------------------------------------------

/// The order of a variant's slot dict in the tool (`item_registry.variant_slots`, `order`): the balance's own slots,
/// then the weapon slots a weapon part list added. The registry's dicts are written with sorted keys, but every list
/// built by walking the slots (name parts, the census's checks) follows this order.
fn slot_order(weapon_balance: bool) -> Vec<&'static str> {
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

/// A variant's slots in the tool's dict order.
fn ordered_slots(slots: &Pv, weapon_balance: bool) -> Vec<(&'static str, &Pv)> {
    slot_order(weapon_balance)
        .into_iter()
        .filter_map(|s| slots.get(s).map(|v| (s, v)))
        .collect()
}

fn is_weapon_balance(record: &Pv) -> bool {
    record
        .at("class")
        .as_str()
        .is_some_and(|c| WEAPON_BALANCES.contains(&c))
}

/// The tool's `census.json` without what is not a result of the build: the run's timestamp (`generated`) and time
/// (`seconds`), and the cross-checks against `weapon_recipe` and `prepare_vendors` (`check_*`, W4-ROLL's tools).
/// `decode_failures` holds the paths the tool's would, with this reader's error text (the tool's text embeds its
/// subprocess command line).
#[derive(Debug, Clone, PartialEq)]
pub struct CensusReport {
    tree: Pv,
}

impl CensusReport {
    pub fn tree(&self) -> &Pv {
        &self.tree
    }

    /// One field, as the tool names it.
    pub fn at(&self, key: &str) -> &Pv {
        self.tree.at(key)
    }

    /// The JSON bytes, sorted keys, as `item_registry.write_json` writes `census.json`.
    pub fn to_json(&self) -> String {
        self.tree.to_json()
    }
}

/// The item registry: the `openwillow-item-registry/1` tree, the census report, and the values it read (so that
/// [`Registry::candidates_at`] can evaluate attribute inits).
pub struct Registry {
    tree: Pv,
    report: CensusReport,
    census: RefCell<Census>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry").finish_non_exhaustive()
    }
}

/// `{slot: {part: weight}}` in the tool's slot order and candidate order.
pub type Candidates = OrderedMap<String, OrderedMap<String, Option<f64>>>;

impl Registry {
    /// The whole registry as the tool writes it (sorted keys, no timestamp).
    pub fn tree(&self) -> &Pv {
        &self.tree
    }

    /// `registry.json`'s bytes: `json.dumps(registry, ensure_ascii=False, separators=(',', ':'), sort_keys=True)`,
    /// without a final newline.
    pub fn to_json(&self) -> String {
        self.tree.to_json()
    }

    pub fn report(&self) -> &CensusReport {
        &self.report
    }

    /// One balance record.
    pub fn balance(&self, path: &str) -> Option<&Pv> {
        self.tree.at("balances").get(path)
    }

    /// `item_registry.candidates_at` on variant `variant` of balance `balance`: `{slot: {part: weight}}` at one
    /// stage for one manufacturer. The stage window applies as `weapon_recipe.slot_candidates` applies it; entries at
    /// or below [`MIN_WEIGHT`] are dropped; a repeated part keeps its later weight at its first position. A weight
    /// that needs runtime state stays `None`. `None` for a balance or variant the registry does not hold.
    ///
    /// **The tool's offline stand-in, not the roll's rule.** It evaluates attribute inits with
    /// `weapon_recipe.attribute_value`'s subset (no attribute values, no conditionals), exactly as the tool and
    /// `prepare_stock_shield` do, so it can be compared with them. W4-ROLL's roll path evaluates weights and windows
    /// through the `ow-attr` evaluator in context. Rules and labels as the tool's (UNVERIFIED for items).
    pub fn candidates_at(
        &self,
        balance: &str,
        variant: usize,
        stage: i64,
        manufacturer: Option<&str>,
    ) -> Option<Candidates> {
        let record = self.balance(balance)?;
        let slots = record.at("variants").items().get(variant)?.at("slots");
        Some(self.candidates_at_slots(slots, is_weapon_balance(record), stage, manufacturer))
    }

    /// [`Registry::candidates_at`] on a slot dict of this registry; `weapon_balance` gives the tool's slot order.
    pub fn candidates_at_slots(
        &self,
        slots: &Pv,
        weapon_balance: bool,
        stage: i64,
        manufacturer: Option<&str>,
    ) -> Candidates {
        let mut census = self.census.borrow_mut();
        let mut out = Candidates::new();
        let stage_f = stage as f64;
        for (slot, entries) in ordered_slots(slots, weapon_balance) {
            let mut live: OrderedMap<String, Option<f64>> = OrderedMap::new();
            for c in entries.items() {
                let low = value_at(&mut census, c.at("min_stage"));
                let high = value_at(&mut census, c.at("max_stage"));
                if low.is_some_and(|l| stage_f < l.trunc())
                    || high.is_some_and(|h| h.trunc() < stage_f)
                {
                    continue;
                }
                let weight = c.at("weight");
                let w = match manufacturer.filter(|m| !m.is_empty()) {
                    Some(maker) if weight.at("source").as_str() != Some("flat") => {
                        match weight.at("by_manufacturer").get(maker) {
                            Some(v) => value_at(&mut census, v),
                            None => value_at(&mut census, weight.at("default")),
                        }
                    }
                    _ => Some(FLAT_WEIGHT),
                };
                if w.is_none_or(|w| w > MIN_WEIGHT) {
                    live.insert(key_text(c.at("part")), w);
                }
            }
            if !live.is_empty() {
                out.insert(slot.to_string(), live);
            }
        }
        out
    }
}

/// Builds the registry of the install the store's root names (the folder holding `WillowGame/CookedPCConsole` and
/// `DLC/`; architecture section 3.7) from the store's census, taken in the tool's file order
/// ([`CensusInput::from_store`]). The registry's `game` field is the store's root as Python's `str(Path(..))` prints
/// it ([`python_path_text`]), so the tool run with `--game <that folder>` writes the same text.
///
/// Values are read from the owner's tag stream (module documentation); `space` is not used until values come from
/// heap objects. Answers `NotYet` while `DefStore::census` has no body (W3-DEFS).
pub fn build_registry(defs: &mut DefStore, space: &mut ObjectSpace) -> Result<Registry, ItemError> {
    let _ = space;
    let census = defs.census()?;
    let root = defs.packages().root().to_path_buf();
    let game = python_path_text(&root.to_string_lossy());
    Ok(build_from_census(
        CensusInput::from_store(census, &root),
        &game,
    ))
}

/// A Python `Counter` as the report writes it (`dict(counter)`): a key exists once it was touched, even by 0.
type Counts = DetMap<String, i64>;

fn bump(c: &mut Counts, key: &str, by: i64) {
    *c.entry(key.to_string()).or_insert(0) += by;
}

fn counts_pv(c: &Counts) -> Pv {
    Pv::Map(c.iter().map(|(k, v)| (k.clone(), Pv::Int(*v))).collect())
}

fn nested_pv(c: &DetMap<String, Counts>) -> Pv {
    Pv::Map(c.iter().map(|(k, v)| (k.clone(), counts_pv(v))).collect())
}

/// What the build hands the census report.
struct Built<'a> {
    by_class: &'a OrderedMap<String, Vec<String>>,
    balances: &'a DetMap<String, Pv>,
    definitions: &'a DetMap<String, DetMap<String, Pv>>,
    parts: &'a DetMap<String, DetMap<String, Pv>>,
    part_lists: &'a Pv,
    name_parts: &'a Pv,
    /// Balance, definition and part paths in the order the tool's dicts hold them (its build order).
    balance_order: &'a [String],
    definition_order: &'a [String],
    part_order: &'a [String],
    problems: &'a [(String, String, Pv)],
    skipped: Counts,
    skipped_examples: DetMap<String, Vec<String>>,
}

/// `item_registry.resolve_problem`: `None` when `path` is an export of a class `expect` accepts, else the reason.
fn resolve_problem(census: &Census, path: &Pv, expect: &dyn Fn(&str) -> bool) -> Option<String> {
    let Some(class) = path.as_str().and_then(|p| census.classes.get(p)) else {
        return Some("no installed package exports it".to_string());
    };
    if expect(short(class)) {
        return None;
    }
    let p = path.as_str().unwrap_or("");
    let others: DetSet<&str> = census
        .conflicts
        .iter()
        .filter(|(_, c)| c.path == p)
        .map(|(_, c)| c.second.as_str())
        .collect();
    let extra = if others.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = others.iter().map(|o| short(o)).collect();
        format!(" (another package holds it as {})", names.join(", "))
    };
    Some(format!(
        "class {} is not the expected kind{extra}",
        short(class)
    ))
}

/// `item_registry.census_report`, then the fields `run` adds that are results of the build.
fn census_report(census: &Census, b: Built<'_>) -> Pv {
    // Unresolved references, in the tool's order: the part lists that did not decode, then the checks.
    let mut unresolved: OrderedMap<String, Vec<(Pv, String, String)>> = OrderedMap::new();
    fn push_row(
        rows: &mut OrderedMap<String, Vec<(Pv, String, String)>>,
        what: &str,
        row: (Pv, String, String),
    ) {
        match rows.get_mut(what) {
            Some(v) => v.push(row),
            None => {
                rows.insert(what.to_string(), vec![row]);
            }
        }
    }
    for (owner, what, path) in b.problems {
        let row = (path.clone(), "does not decode".to_string(), owner.clone());
        push_row(&mut unresolved, &format!("{what} (does not decode)"), row);
    }
    let is_part = |n: &str| kind_of(&PART_KINDS, n).is_some();
    let is_list = |n: &str| LIST_CLASSES.contains(&n);
    let is_definition = |n: &str| definition_kind(n).is_some();
    let is_maker = |n: &str| n == "ManufacturerDefinition";
    let is_name = |n: &str| NAME_PART_CLASSES.contains(&n);
    let is_gestalt = |n: &str| n == "GestaltSkeletalMeshDefinition";
    let mut check = |what: &str, path: &Pv, expect: &dyn Fn(&str) -> bool, owner: &str| {
        if let Some(reason) = resolve_problem(census, path, expect) {
            push_row(
                &mut unresolved,
                what,
                (path.clone(), reason, owner.to_string()),
            );
        }
    };
    let names_of_record = |r: &Pv| -> Vec<Pv> {
        let mut v = r.at("prefix").items().to_vec();
        v.extend(r.at("title").items().iter().cloned());
        v
    };
    for path in b.balance_order {
        let Some(record) = b.balances.get(path) else {
            continue;
        };
        let weapon = is_weapon_balance(record);
        for variant in record.at("variants").items() {
            for (_, entries) in ordered_slots(variant.at("slots"), weapon) {
                for c in entries.items() {
                    check("part", c.at("part"), &is_part, path);
                    check("part list", c.at("list"), &is_list, path);
                }
            }
            for step in variant.at("merge").items() {
                check("part list", step.at("list"), &is_list, path);
            }
        }
        for item in record.at("definitions").items() {
            check("definition", item, &is_definition, path);
        }
        for maker in record.at("manufacturers").items() {
            check("manufacturer", maker.at("manufacturer"), &is_maker, path);
        }
        for name in names_of_record(record.at("name_parts")) {
            check("name part", &name, &is_name, path);
        }
        if record.at("gestalt").truthy() {
            check("gestalt", record.at("gestalt"), &is_gestalt, path);
        }
    }
    for path in b.definition_order {
        let Some(record) = b.definitions.get(path) else {
            continue;
        };
        let record = Pv::Map(record.clone());
        if let Pv::Map(lists) = record.at("part_lists") {
            for source in lists.values() {
                check("part list", source, &is_list, path);
            }
        }
        for name in names_of_record(&record) {
            check("name part", &name, &is_name, path);
        }
    }
    for path in b.part_order {
        let Some(record) = b.parts.get(path) else {
            continue;
        };
        for name in names_of_record(&Pv::Map(record.clone())) {
            check("name part", &name, &is_name, path);
        }
    }
    let mut merged = DetMap::new();
    let mut unresolved_counts = Counts::new();
    for (what, rows) in unresolved.iter() {
        let mut by_path: OrderedMap<String, (Pv, String, Vec<String>)> = OrderedMap::new();
        for (path, reason, owner) in rows {
            let key = key_text(path);
            match by_path.get_mut(key.as_str()) {
                Some(entry) => {
                    if !entry.2.contains(owner) {
                        entry.2.push(owner.clone());
                    }
                }
                None => {
                    by_path.insert(key, (path.clone(), reason.clone(), vec![owner.clone()]));
                }
            }
        }
        let mut list: Vec<(String, Pv)> = by_path
            .iter()
            .map(|(k, (path, reason, owners))| {
                let row = Pv::map([
                    ("path", path.clone()),
                    ("reason", Pv::Str(reason.clone())),
                    (
                        "named_by",
                        Pv::List(owners.iter().map(|o| Pv::text(o)).collect()),
                    ),
                ]);
                (k.clone(), row)
            })
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        unresolved_counts.insert(what.clone(), list.len() as i64);
        merged.insert(
            what.clone(),
            Pv::List(list.into_iter().map(|(_, r)| r).collect()),
        );
    }

    // Per kind.
    let mut per_kind: DetMap<String, Counts> = DetMap::new();
    let mut slot_cover: DetMap<String, DetMap<String, DetSet<String>>> = DetMap::new();
    let mut slot_empty: DetMap<String, Counts> = DetMap::new();
    let mut no_slots = Counts::new();
    let mut unresolved_values = Counts::new();
    let mut constant_hints = Counts::new();
    for record in b.balances.values() {
        let kind = record.at("kind").as_str().unwrap_or("").to_string();
        let counts = per_kind.entry(kind.clone()).or_default();
        bump(counts, "balances", 1);
        bump(
            counts,
            &format!("chain_{}", record.at("chain_end").as_str().unwrap_or("")),
            1,
        );
        bump(
            counts,
            "variants",
            record.at("variants").items().len() as i64,
        );
        if !record.at("manufacturers").truthy() {
            bump(counts, "balances_without_manufacturer", 1);
        }
        if record.get("inventory_definition_root_first").is_some() {
            bump(counts, "definition_differs_root_first", 1);
        }
        if record.get("manufacturers_root_first").is_some() {
            bump(counts, "manufacturers_differ_root_first", 1);
        }
        if !record
            .at("variants")
            .items()
            .iter()
            .any(|v| v.at("slots").truthy())
        {
            bump(&mut no_slots, &kind, 1);
        }
        for variant in record.at("variants").items() {
            if let Pv::Map(slots) = variant.at("slots") {
                for (slot, entries) in slots {
                    if entries.truthy() {
                        let cover = slot_cover
                            .entry(kind.clone())
                            .or_default()
                            .entry(slot.clone())
                            .or_default();
                        cover.extend(entries.items().iter().map(|c| key_text(c.at("part"))));
                    } else {
                        bump(slot_empty.entry(kind.clone()).or_default(), slot, 1);
                    }
                }
                for c in slots.values().flat_map(Pv::items) {
                    for key in ["min_stage", "max_stage"] {
                        if let Pv::Map(m) = c.at(key) {
                            bump(&mut unresolved_values, key, 1);
                            bump(
                                &mut constant_hints,
                                key,
                                i64::from(m.contains_key("attribute_constant")),
                            );
                        }
                    }
                    let weight = c.at("weight");
                    if weight.at("source").as_str() == Some("manufacturer") {
                        let mut values: Vec<&Pv> = match weight.at("by_manufacturer") {
                            Pv::Map(m) => m.values().collect(),
                            _ => Vec::new(),
                        };
                        values.push(weight.at("default"));
                        if values.iter().any(|v| matches!(v, Pv::Map(_))) {
                            bump(&mut unresolved_values, "weight", 1);
                        }
                    }
                }
            }
        }
    }
    for (kind, counts) in per_kind.iter_mut() {
        counts.insert(
            "balances_with_no_part_slot".to_string(),
            no_slots.get(kind).copied().unwrap_or(0),
        );
    }
    let mut definitions_by_kind = Counts::new();
    let mut definitions_unused = Counts::new();
    for r in b.definitions.values() {
        let kind = r.get("kind").and_then(Pv::as_str).unwrap_or("");
        bump(&mut definitions_by_kind, kind, 1);
        if r.get("balances").is_some_and(|v| v.items().is_empty()) {
            bump(&mut definitions_unused, kind, 1);
        }
    }
    let mut parts_by_kind = Counts::new();
    let mut parts_unlisted = Counts::new();
    for r in b.parts.values() {
        let kind = r.get("kind").and_then(Pv::as_str).unwrap_or("");
        bump(&mut parts_by_kind, kind, 1);
        if r.get("balances").is_some_and(|v| v.items().is_empty()) {
            bump(&mut parts_unlisted, kind, 1);
        }
    }
    let mut per_dlc: DetMap<String, Counts> = DetMap::new();
    let dlc_of = |r: &Pv| r.at("dlc").as_str().unwrap_or("").to_string();
    for r in b.balances.values() {
        let key = format!("balances_{}", r.at("kind").as_str().unwrap_or(""));
        bump(per_dlc.entry(dlc_of(r)).or_default(), &key, 1);
    }
    for (prefix, table) in [("definitions", b.definitions), ("parts", b.parts)] {
        for r in table.values() {
            let r = Pv::Map(r.clone());
            let key = format!("{prefix}_{}", r.at("kind").as_str().unwrap_or(""));
            bump(per_dlc.entry(dlc_of(&r)).or_default(), &key, 1);
        }
    }
    for (name, table) in [("part_lists", b.part_lists), ("name_parts", b.name_parts)] {
        if let Pv::Map(m) = table {
            for r in m.values() {
                bump(per_dlc.entry(dlc_of(r)).or_default(), name, 1);
            }
        }
    }
    let mut holders_many = Counts::new();
    for (path, files) in &census.holders {
        if files.len() > 1 {
            bump(
                &mut holders_many,
                census.classes.get(path).map_or("?", |c| short(c)),
                1,
            );
        }
    }
    let mut skipped_all = b.skipped;
    for (name, count) in &census.class_defaults {
        skipped_all.insert(
            format!("{name}: class default object (Default__), not a game object"),
            *count as i64,
        );
    }
    let parts_per_slot: DetMap<String, Pv> = slot_cover
        .iter()
        .map(|(kind, slots)| {
            let m: DetMap<String, Pv> = slots
                .iter()
                .map(|(s, parts)| (s.clone(), Pv::Int(parts.len() as i64)))
                .collect();
            (kind.clone(), Pv::Map(m))
        })
        .collect();
    let game = python_path_text(&census.root.to_string_lossy());
    let sep = if cfg!(windows) { "\\" } else { "/" };
    let conflicts: Vec<Pv> = census
        .conflicts
        .iter()
        .map(|(_, c)| {
            // `str(file)`: the tool's file is `game / <relative path>`.
            let rel = relative_posix(&c.second_package, &census.root).replace('/', sep);
            Pv::map([
                ("path", Pv::text(&c.path)),
                ("first", Pv::text(&c.first)),
                ("second", Pv::text(&c.second)),
                ("second_package", Pv::Str(format!("{game}{sep}{rel}"))),
            ])
        })
        .collect();
    let null_lists: Counts = census
        .null_part_entries
        .iter()
        .map(|(k, v)| (k.clone(), *v as i64))
        .collect();
    let null_owners: Counts = census
        .null_name_entries
        .iter()
        .filter(|(_, v)| **v > 0)
        .map(|(k, v)| (k.clone(), *v as i64))
        .collect();
    let unlisted_data: Counts = census
        .unlisted_part_data
        .iter()
        .map(|(k, v)| (k.clone(), v.len() as i64))
        .collect();
    let unlisted_classes: Counts = census
        .unlisted_classes
        .iter()
        .map(|(k, v)| (k.clone(), *v as i64))
        .collect();
    let scan_errors: DetMap<String, Pv> = census
        .load_failures
        .iter()
        .map(|(file, text)| (relative_posix(file, &census.root), Pv::text(text)))
        .collect();
    let mut t = DetMap::new();
    let mut put = |k: &str, v: Pv| {
        t.insert(k.to_string(), v);
    };
    put("schema", Pv::Str(format!("{SCHEMA_TAG}/census")));
    put(
        "objects_by_class",
        Pv::Map(
            b.by_class
                .iter()
                .map(|(k, v)| (k.clone(), Pv::Int(v.len() as i64)))
                .collect(),
        ),
    );
    put("balances_by_kind", nested_pv(&per_kind));
    put("definitions_by_kind", counts_pv(&definitions_by_kind));
    put(
        "definitions_without_balance_by_kind",
        counts_pv(&definitions_unused),
    );
    put("parts_by_kind", counts_pv(&parts_by_kind));
    put("parts_no_balance_lists_by_kind", counts_pv(&parts_unlisted));
    put("parts_per_slot", Pv::Map(parts_per_slot));
    put("enabled_slots_left_empty", nested_pv(&slot_empty));
    put(
        "candidate_values_needing_runtime_state",
        counts_pv(&unresolved_values),
    );
    put(
        "of_which_attribute_constant_hint",
        counts_pv(&constant_hints),
    );
    put("by_dlc", nested_pv(&per_dlc));
    put(
        "paths_in_several_packages_by_class",
        counts_pv(&holders_many),
    );
    put("paths_with_two_classes", Pv::List(conflicts));
    put(
        "unlisted_classes",
        Pv::map([
            ("export_classes", counts_pv(&unlisted_classes)),
            ("part_data_properties", counts_pv(&unlisted_data)),
        ]),
    );
    put("skipped", counts_pv(&skipped_all));
    put(
        "skipped_examples",
        Pv::Map(
            b.skipped_examples
                .iter()
                .map(|(k, v)| (k.clone(), Pv::List(v.iter().map(|p| Pv::text(p)).collect())))
                .collect(),
        ),
    );
    put(
        "decode_failures",
        Pv::Map(
            census
                .failed
                .iter()
                .map(|(k, v)| (k.clone(), Pv::text(v)))
                .collect(),
        ),
    );
    put(
        "part_list_rows_naming_no_part",
        Pv::map([
            ("rows", Pv::Int(null_lists.values().sum())),
            ("lists", counts_pv(&null_lists)),
        ]),
    );
    put(
        "name_list_entries_empty",
        Pv::map([
            (
                "entries",
                Pv::Int(census.null_name_entries.values().sum::<usize>() as i64),
            ),
            ("owners", counts_pv(&null_owners)),
        ]),
    );
    put("unresolved_counts", counts_pv(&unresolved_counts));
    put("unresolved", Pv::Map(merged));
    // What `item_registry.run` adds.
    put("packages_scanned", Pv::Int(census.files.len() as i64));
    put("scan_errors", Pv::Map(scan_errors));
    put(
        "duplicate_export_paths_all_classes",
        Pv::Int(census.duplicates as i64),
    );
    Pv::Map(t)
}

/// Builds the registry from a census (the tool's `build`). `game` is written into the `game` field.
pub fn build_from_census(input: CensusInput, game: &str) -> Registry {
    let mut census = Census::new(input);
    let mut by_class: OrderedMap<String, Vec<String>> = OrderedMap::new();
    for path in census.order.clone() {
        let name = short(census.class_of(&path)).to_string();
        if relevant(&name) {
            match by_class.get_mut(name.as_str()) {
                Some(v) => v.push(path),
                None => {
                    by_class.insert(name, vec![path]);
                }
            }
        }
    }
    for paths in by_class.values_mut() {
        paths.sort();
    }
    // The tool's preload: balances, definitions, parts and part lists, decoded grouped by owner file.
    let decode_first: Vec<String> = by_class
        .iter()
        .filter(|(name, _)| {
            is_balance(name)
                || definition_kind(name).is_some()
                || kind_of(&PART_KINDS, name).is_some()
                || LIST_CLASSES.contains(&name.as_str())
        })
        .flat_map(|(_, paths)| paths.iter().cloned())
        .collect();
    census.preload(&decode_first);

    let mut skipped = Counts::new();
    let mut skipped_examples: DetMap<String, Vec<String>> = DetMap::new();
    let mut skip = |reason: String, path: &str| {
        bump(&mut skipped, &reason, 1);
        let examples = skipped_examples.entry(reason).or_default();
        if examples.len() < 5 {
            examples.push(path.to_string());
        }
    };

    // Definitions and weapon types.
    let mut definitions: DetMap<String, DetMap<String, Pv>> = DetMap::new();
    let mut definition_order: Vec<String> = Vec::new();
    for (name, paths) in by_class.iter() {
        let Some(kind) = definition_kind(name) else {
            continue;
        };
        for path in paths {
            let Some(d) = census.get(path) else {
                skip(format!("{name}: does not decode"), path);
                continue;
            };
            let (prefix, title) = names_of(&mut census, path, &d.props);
            let slots: &[&str] = if kind == "weapon" {
                &WEAPON_SLOTS
            } else {
                &ITEM_SLOTS
            };
            let mut lists = DetMap::new();
            for slot in slots {
                let v = d.props.at(&format!("{slot}Parts"));
                if v.truthy() {
                    lists.insert((*slot).to_string(), v.clone());
                }
            }
            let mut r = DetMap::new();
            r.insert("class".into(), Pv::text(name));
            r.insert("kind".into(), Pv::text(kind));
            r.insert("package".into(), Pv::Str(census.rel(path)));
            r.insert("dlc".into(), Pv::Str(census.dlc_of(path)));
            r.insert("part_lists".into(), Pv::Map(lists));
            r.insert("prefix".into(), Pv::List(prefix));
            r.insert("title".into(), Pv::List(title));
            r.insert("gestalt".into(), d.props.at("GestaltMesh").clone());
            r.insert(
                "manufacturer_override".into(),
                d.props.at("ManufacturerOverride").clone(),
            );
            r.insert(
                "required_player_class".into(),
                d.props.at("RequiredPlayerClass").clone(),
            );
            r.insert(
                "customization".into(),
                d.props.at("CustomizationDef").clone(),
            );
            r.insert("balances".into(), Pv::List(Vec::new()));
            definitions.insert(path.clone(), r);
            definition_order.push(path.clone());
        }
    }

    // Parts.
    let mut parts: DetMap<String, DetMap<String, Pv>> = DetMap::new();
    let mut part_order: Vec<String> = Vec::new();
    for (name, kind) in PART_KINDS {
        let paths = by_class.get(name).cloned().unwrap_or_default();
        for path in &paths {
            let Some(d) = census.get(path) else {
                skip(format!("{name}: does not decode"), path);
                continue;
            };
            let (prefix, title) = names_of(&mut census, path, &d.props);
            let part_type = {
                let t = d.props.at("PartType");
                if t.truthy() {
                    t.clone()
                } else {
                    d.props.at("ShieldPartType").clone()
                }
            };
            let mut r = DetMap::new();
            r.insert("class".into(), Pv::text(name));
            r.insert("kind".into(), Pv::text(kind));
            r.insert("package".into(), Pv::Str(census.rel(path)));
            r.insert("dlc".into(), Pv::Str(census.dlc_of(path)));
            r.insert("part_type".into(), part_type);
            r.insert(
                "manufacturer_scope".into(),
                d.props.at("ManufacturerDefinitionForScopeMovie").clone(),
            );
            r.insert("prefix".into(), Pv::List(prefix));
            r.insert("title".into(), Pv::List(title));
            r.insert(
                "gestalt_fragments".into(),
                Pv::List(part_fragments(path, &d)),
            );
            r.insert("material".into(), d.props.at("Material").clone());
            r.insert("balances".into(), Pv::List(Vec::new()));
            parts.insert(path.clone(), r);
            part_order.push(path.clone());
        }
    }

    // Balances.
    let mut balances: DetMap<String, Pv> = DetMap::new();
    let mut balance_order: Vec<String> = Vec::new();
    let mut problems: Vec<(String, String, Pv)> = Vec::new();
    let mut balance_names: Vec<&str> = WEAPON_BALANCES
        .iter()
        .chain(ITEM_BALANCES.iter())
        .copied()
        .collect();
    balance_names.sort();
    for name in balance_names {
        let paths = by_class.get(name).cloned().unwrap_or_default();
        for path in &paths {
            if census.get(path).is_none() {
                skip(format!("{name}: does not decode"), path);
                continue;
            }
            let (mut record, trouble) = balance_record(&mut census, path);
            problems.extend(trouble.into_iter().map(|(what, r)| (path.clone(), what, r)));
            let weapon = WEAPON_BALANCES.contains(&name);
            let mut prefix: Vec<Pv> = Vec::new();
            let mut title: Vec<Pv> = Vec::new();
            for item in record.get("definitions").map_or(&[][..], Pv::items) {
                let Some(def) = item.as_str().and_then(|k| definitions.get_mut(k)) else {
                    continue;
                };
                if let Some(Pv::List(b)) = def.get_mut("balances") {
                    b.push(Pv::text(path));
                }
                prefix.extend(def.get("prefix").map_or(&[][..], Pv::items).iter().cloned());
                title.extend(def.get("title").map_or(&[][..], Pv::items).iter().cloned());
            }
            for variant in record.get("variants").map_or(&[][..], Pv::items) {
                // The tool walks each variant's slots in its dict order, not in the sorted order they are written in.
                for (_, entries) in ordered_slots(variant.at("slots"), weapon) {
                    for candidate in entries.items() {
                        let Some(part) =
                            candidate.at("part").as_str().and_then(|k| parts.get_mut(k))
                        else {
                            continue;
                        };
                        if let Some(Pv::List(b)) = part.get_mut("balances")
                            && b.last().and_then(Pv::as_str) != Some(path.as_str())
                        {
                            b.push(Pv::text(path));
                        }
                        prefix.extend(
                            part.get("prefix")
                                .map_or(&[][..], Pv::items)
                                .iter()
                                .cloned(),
                        );
                        title.extend(part.get("title").map_or(&[][..], Pv::items).iter().cloned());
                    }
                }
            }
            record.insert(
                "name_parts".into(),
                Pv::map([
                    ("prefix", Pv::List(unique(prefix))),
                    ("title", Pv::List(unique(title))),
                ]),
            );
            balances.insert(path.clone(), Pv::Map(record));
            balance_order.push(path.clone());
        }
    }
    for record in definitions.values_mut().chain(parts.values_mut()) {
        if let Some(Pv::List(b)) = record.get_mut("balances") {
            let set: DetSet<String> = b
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            *b = set.into_iter().map(Pv::Str).collect();
        }
    }

    let table = |census: &Census, names: &[&str]| -> Pv {
        let mut m = DetMap::new();
        for name in names {
            for p in by_class.get(*name).map_or(&[][..], Vec::as_slice) {
                m.insert(
                    p.clone(),
                    Pv::map([
                        ("class", Pv::text(short(census.class_of(p)))),
                        ("package", Pv::Str(census.rel(p))),
                        ("dlc", Pv::Str(census.dlc_of(p))),
                    ]),
                );
            }
        }
        Pv::Map(m)
    };
    let part_lists = table(&census, &LIST_CLASSES);
    let name_parts = table(&census, &NAME_PART_CLASSES);
    let report = census_report(
        &census,
        Built {
            by_class: &by_class,
            balances: &balances,
            definitions: &definitions,
            parts: &parts,
            part_lists: &part_lists,
            name_parts: &name_parts,
            balance_order: &balance_order,
            definition_order: &definition_order,
            part_order: &part_order,
            problems: &problems,
            skipped,
            skipped_examples,
        },
    );

    let mut tree = DetMap::new();
    tree.insert("schema".to_string(), Pv::text(SCHEMA_TAG));
    tree.insert("schema_line".to_string(), Pv::text(SCHEMA_LINE));
    tree.insert("game".to_string(), Pv::text(game));
    tree.insert("tool".to_string(), Pv::text(TOOL));
    tree.insert(
        "rules".to_string(),
        Pv::List(RULES.iter().map(|r| Pv::text(r)).collect()),
    );
    tree.insert(
        "slots".to_string(),
        Pv::map([
            (
                "weapon",
                Pv::List(WEAPON_SLOTS.iter().map(|s| Pv::text(s)).collect()),
            ),
            (
                "item",
                Pv::List(ITEM_SLOTS.iter().map(|s| Pv::text(s)).collect()),
            ),
        ]),
    );
    tree.insert("balances".to_string(), Pv::Map(balances));
    tree.insert(
        "definitions".to_string(),
        Pv::Map(
            definitions
                .into_iter()
                .map(|(k, v)| (k, Pv::Map(v)))
                .collect(),
        ),
    );
    tree.insert(
        "parts".to_string(),
        Pv::Map(parts.into_iter().map(|(k, v)| (k, Pv::Map(v))).collect()),
    );
    tree.insert("part_lists".to_string(), part_lists);
    tree.insert("name_parts".to_string(), name_parts);
    tree.insert(
        "manufacturers".to_string(),
        table(&census, &["ManufacturerDefinition"]),
    );
    tree.insert(
        "gestalts".to_string(),
        table(&census, &["GestaltSkeletalMeshDefinition"]),
    );
    Registry {
        tree: Pv::Map(tree),
        report: CensusReport { tree: report },
        census: RefCell::new(census),
    }
}

/// `str(Path(text))` as the tool writes its `game` field. On Windows: separators as `\`, empty and `.` parts dropped,
/// no trailing separator except after a drive (`C:\`) or a UNC share (`\\server\share\`), a UNC prefix kept. Other
/// platforms keep `/` (a leading `//` stays, as POSIX pathlib keeps it).
pub fn python_path_text(text: &str) -> String {
    let sep = if cfg!(windows) { '\\' } else { '/' };
    let is_sep = |c: char| c == '/' || (cfg!(windows) && c == '\\');
    let leading = text.chars().take_while(|c| is_sep(*c)).count();
    let parts: Vec<&str> = text
        .split(is_sep)
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let unc = cfg!(windows) && leading == 2 && parts.len() >= 2;
    let mut out = String::new();
    if unc || (!cfg!(windows) && leading == 2) {
        out.push(sep);
        out.push(sep);
    } else if leading > 0 {
        out.push(sep);
    }
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push(sep);
        }
        out.push_str(p);
    }
    // A drive alone (`C:\`) or a share alone (`\\server\share\`) keeps its root separator.
    let drive_root =
        cfg!(windows) && parts.len() == 1 && parts[0].ends_with(':') && text.len() > parts[0].len();
    if drive_root || (unc && parts.len() == 2) {
        out.push(sep);
    }
    if out.is_empty() {
        out.push('.');
    }
    out
}

#[cfg(test)]
mod tests {
    //! `tests/item_registry_test.py` re-expressed: the same invented install (one base package, one DLC package),
    //! built with `ow_package::testkit`, every name and number invented (AGENTS.md rule 9).

    use super::*;
    use ow_defs::identity::IdentityPolicy;
    use ow_package::testkit::{PackageBuilder, ints};

    /// A value of the fixture, as the Python test writes it.
    #[derive(Clone)]
    enum V {
        Int(i32),
        Float(f32),
        Bool(bool),
        Ref(Option<&'static str>),
        Enum(&'static str, &'static str),
        Struct(&'static str, Vec<(&'static str, V)>),
        Array(Vec<V>),
    }

    fn r(path: &'static str) -> V {
        V::Ref(Some(path))
    }

    type Props = Vec<(&'static str, V)>;

    /// One synthetic package: exports are outermost objects whose own name holds the dotted path; a reference to a
    /// path this package does not export becomes an import (`Core.Object`), as in the Python test.
    #[derive(Default)]
    struct Syn {
        exports: Vec<(&'static str, &'static str, Props)>,
        imports: Vec<(&'static str, &'static str, i32, String)>,
    }

    impl Syn {
        fn add(&mut self, path: &'static str, class: &'static str, props: Props) {
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

        fn build(mut self) -> Vec<u8> {
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

    fn konst(value: f32) -> V {
        V::Struct(
            "AttributeInitializationData",
            vec![
                ("BaseValueConstant", V::Float(value)),
                ("BaseValueScaleConstant", V::Float(1.0)),
            ],
        )
    }

    fn weighted(
        part: Option<&'static str>,
        low: i32,
        high: i32,
        weight: Option<i32>,
        makers: &[(&'static str, i32)],
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
            ("Part", V::Ref(part)),
            ("Manufacturers", V::Array(rows)),
            ("MinGameStageIndex", V::Int(low)),
            ("MaxGameStageIndex", V::Int(high)),
        ];
        if let Some(w) = weight {
            fields.push(("DefaultWeightIndex", V::Int(w)));
        }
        V::Struct("WeightedPart", fields)
    }

    fn w(part: &'static str) -> V {
        weighted(Some(part), 0, 1, None, &[])
    }

    fn part_data(rows: Vec<V>, enabled: bool) -> V {
        V::Struct(
            "PartData",
            vec![
                ("bEnabled", V::Bool(enabled)),
                ("WeightedParts", V::Array(rows)),
            ],
        )
    }

    fn grade(low: i32, high: i32, definition: Option<&'static str>) -> V {
        let mods = V::Struct(
            "Mods",
            vec![
                ("ExpLevel", V::Int(1)),
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
        V::Struct(
            "GradeData",
            vec![("GradeModifiers", mods), ("GameStageRequirement", need)],
        )
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

    fn consolidated() -> (&'static str, V) {
        let values = vec![
            konst(1.0),
            konst(100.0),
            konst(5.0),
            konst(30.0),
            konst(7.0),
        ];
        ("ConsolidatedAttributeInitData", V::Array(values))
    }

    fn mode(m: &'static str) -> (&'static str, V) {
        ("PartReplacementMode", V::Enum("EPartReplacementMode", m))
    }

    fn one(class: &'static str, rows: Vec<V>) -> Props {
        vec![(class, V::Array(rows))]
    }

    const ALPHA: &str = "GD_Syn.Makers.Alpha";
    const BETA: &str = "GD_Syn.Makers.Beta";
    const IB: &str = "WillowGame.InventoryBalanceDefinition";
    const COLL: &str = "WillowGame.ItemPartListCollectionDefinition";
    const WCOLL: &str = "WillowGame.WeaponPartListCollectionDefinition";

    fn base_package() -> Syn {
        let mut p = Syn::default();
        p.add(
            "Default__ShieldDefinition",
            "WillowGame.ShieldDefinition",
            vec![],
        );
        p.add(ALPHA, "Engine.ManufacturerDefinition", vec![]);
        p.add(BETA, "Engine.ManufacturerDefinition", vec![]);
        p.add(
            "GD_Syn.Names.Title_Shield",
            "WillowGame.ItemNamePartDefinition",
            vec![("PartName", V::Int(1))],
        );
        p.add(
            "GD_Syn.Names.Title_Gun",
            "WillowGame.WeaponNamePartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Gestalt.Shield",
            "GearboxFramework.GestaltSkeletalMeshDefinition",
            vec![],
        );
        // Shield: a definition list for Alpha, an additive collection on the base balance (Alpha + Beta, Beta names
        // a part no package exports), a selective collection on the leaf that replaces Alpha.
        p.add(
            "GD_Syn.Shield.Body_A",
            "WillowGame.ShieldPartDefinition",
            one("TitleList", vec![r("GD_Syn.Names.Title_Shield")]),
        );
        p.add(
            "GD_Syn.Shield.Body_B",
            "WillowGame.ShieldPartDefinition",
            vec![],
        );
        // An empty name entry: counted, left out.
        p.add(
            "GD_Syn.Shield.Cap_A",
            "WillowGame.ShieldPartDefinition",
            one("TitleList", vec![V::Ref(None)]),
        );
        p.add(
            "GD_Syn.Shield.Cap_B",
            "WillowGame.ShieldPartDefinition",
            vec![],
        );
        let body_b = || weighted(Some("GD_Syn.Shield.Body_B"), 2, 3, Some(0), &[(ALPHA, 4)]);
        p.add(
            "GD_Syn.Shield.List_Alpha",
            "WillowGame.ItemPartListDefinition",
            vec![
                (
                    "WeightedParts",
                    V::Array(vec![w("GD_Syn.Shield.Body_A"), body_b()]),
                ),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Shield.Def",
            "WillowGame.ShieldDefinition",
            vec![
                ("AlphaParts", r("GD_Syn.Shield.List_Alpha")),
                ("GestaltMesh", r("GD_Syn.Gestalt.Shield")),
            ],
        );
        p.add(
            "GD_Syn.Shield.Coll_Add",
            COLL,
            vec![
                (
                    "AlphaPartData",
                    part_data(vec![w("GD_Syn.Shield.Cap_A")], true),
                ),
                (
                    "BetaPartData",
                    part_data(
                        vec![w("GD_Syn.Shield.Cap_B"), w("GD_Syn.Shield.Ghost")],
                        true,
                    ),
                ),
                (
                    "GammaPartData",
                    part_data(vec![w("GD_Syn.Shield.Cap_A")], false),
                ),
                // A slot outside the nine: unlisted.
                (
                    "IotaPartData",
                    part_data(vec![w("GD_Syn.Shield.Cap_A")], true),
                ),
                mode("EPRM_Additive"),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Shield.Coll_Sel",
            COLL,
            vec![
                ("AlphaPartData", part_data(vec![body_b()], true)),
                mode("EPRM_Selective"),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Grades.Shield_Base",
            IB,
            vec![
                ("InventoryDefinition", r("GD_Syn.Shield.Def")),
                (
                    "Manufacturers",
                    makers(vec![(ALPHA, vec![grade(1, 4, None), grade(5, 50, None)])]),
                ),
                ("PartListCollection", r("GD_Syn.Shield.Coll_Add")),
            ],
        );
        p.add(
            "GD_Syn.Grades.Shield_Leaf",
            IB,
            vec![
                ("BaseDefinition", r("GD_Syn.Grades.Shield_Base")),
                ("PartListCollection", r("GD_Syn.Shield.Coll_Sel")),
            ],
        );
        // One balance of every other item kind.
        p.add(
            "GD_Syn.Grenade.Part",
            "WillowGame.GrenadeModPartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Grenade.Part2",
            "WillowGame.GrenadeModPartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Grenade.List",
            "WillowGame.ItemPartListDefinition",
            // A row naming no part.
            vec![
                (
                    "WeightedParts",
                    V::Array(vec![
                        w("GD_Syn.Grenade.Part"),
                        weighted(None, 0, 1, None, &[]),
                    ]),
                ),
                consolidated(),
            ],
        );
        // No PartReplacementMode: additive.
        p.add(
            "GD_Syn.Grenade.Coll",
            COLL,
            vec![
                (
                    "DeltaPartData",
                    part_data(vec![w("GD_Syn.Grenade.Part2")], true),
                ),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Grenade.Def",
            "WillowGame.GrenadeModDefinition",
            vec![("DeltaParts", r("GD_Syn.Grenade.List"))],
        );
        p.add(
            "GD_Syn.Grades.Grenade",
            IB,
            vec![
                ("InventoryDefinition", r("GD_Syn.Grenade.Def")),
                (
                    "Manufacturers",
                    makers(vec![(BETA, vec![grade(1, 100, None)])]),
                ),
                ("PartListCollection", r("GD_Syn.Grenade.Coll")),
            ],
        );
        p.add(
            "GD_Syn.ClassMod.Part",
            "WillowGame.ClassModPartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.ClassMod.Def_A",
            "WillowGame.ClassModDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.ClassMod.Def_B",
            "WillowGame.ClassModDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.ClassMod.Bal.PartList",
            COLL,
            vec![
                (
                    "BetaPartData",
                    part_data(vec![w("GD_Syn.ClassMod.Part")], true),
                ),
                mode("EPRM_Selective"),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.ClassMod.Bal",
            "WillowGame.ClassModBalanceDefinition",
            vec![
                (
                    "ClassModDefinitions",
                    V::Array(vec![r("GD_Syn.ClassMod.Def_A"), r("GD_Syn.ClassMod.Def_B")]),
                ),
                ("ItemPartListCollection", r("GD_Syn.ClassMod.Bal.PartList")),
                (
                    "Manufacturers",
                    makers(vec![
                        (ALPHA, vec![grade(1, 100, None)]),
                        (BETA, vec![grade(1, 100, None)]),
                    ]),
                ),
            ],
        );
        p.add(
            "GD_Syn.Relic.Part",
            "WillowGame.ArtifactPartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Relic.List",
            "WillowGame.ItemPartListDefinition",
            vec![
                ("WeightedParts", V::Array(vec![w("GD_Syn.Relic.Part")])),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Relic.Def",
            "WillowGame.ArtifactDefinition",
            vec![("EtaParts", r("GD_Syn.Relic.List"))],
        );
        p.add(
            "GD_Syn.Grades.Relic",
            IB,
            vec![
                ("InventoryDefinition", r("GD_Syn.Relic.Def")),
                (
                    "Manufacturers",
                    makers(vec![(ALPHA, vec![grade(1, 100, None)])]),
                ),
            ],
        );
        p.add(
            "GD_Syn.Usable.Def",
            "WillowGame.UsableItemDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Grades.Usable",
            IB,
            vec![("InventoryDefinition", r("GD_Syn.Usable.Def"))],
        );
        p.add(
            "GD_Syn.Custom.Def",
            "WillowGame.UsableCustomizationItemDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Grades.Custom",
            IB,
            vec![("InventoryDefinition", r("GD_Syn.Custom.Def"))],
        );
        p.add(
            "GD_Syn.Mission.Def",
            "WillowGame.MissionItemDefinition",
            vec![],
        );
        // No balance names it.
        p.add(
            "GD_Syn.Mission.Unused",
            "WillowGame.MissionItemDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Grades.Mission",
            IB,
            vec![("InventoryDefinition", r("GD_Syn.Mission.Def"))],
        );
        // Weapon: one type, two barrels, one collection, one balance.
        p.add(
            "GD_Syn.Gun.Type",
            "WillowGame.WeaponTypeDefinition",
            one("TitleList", vec![r("GD_Syn.Names.Title_Gun")]),
        );
        p.add(
            "GD_Syn.Gun.Barrel_A",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Gun.Barrel_B",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        let barrel_b = weighted(Some("GD_Syn.Gun.Barrel_B"), 0, 1, Some(0), &[(BETA, 4)]);
        p.add(
            "GD_Syn.Gun.Coll",
            WCOLL,
            vec![
                (
                    "BarrelPartData",
                    part_data(vec![w("GD_Syn.Gun.Barrel_A"), barrel_b], true),
                ),
                mode("EPRM_Selective"),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Gun.Grip_A",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Gun.Coll_Complete",
            WCOLL,
            vec![
                (
                    "GripPartData",
                    part_data(vec![w("GD_Syn.Gun.Grip_A")], true),
                ),
                mode("EPRM_Complete"),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Gun.Complete",
            "WillowGame.WeaponBalanceDefinition",
            vec![
                ("BaseDefinition", r("GD_Syn.Gun.Balance")),
                ("WeaponPartListCollection", r("GD_Syn.Gun.Coll_Complete")),
            ],
        );
        // A balance class the registry does not read.
        p.add(
            "GD_Syn.Odd.Balance",
            "WillowGame.SynthOddBalanceDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Gun.Sight_A",
            "WillowGame.WeaponPartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Gun.Coll_Turret",
            WCOLL,
            vec![
                (
                    "SightPartData",
                    part_data(vec![w("GD_Syn.Gun.Sight_A")], true),
                ),
                mode("EPRM_Selective"),
                consolidated(),
            ],
        );
        // An inventory balance naming a weapon type.
        p.add(
            "GD_Syn.Grades.Turret",
            IB,
            vec![
                ("InventoryDefinition", r("GD_Syn.Gun.Type")),
                ("PartListCollection", r("GD_Syn.Gun.Coll_Turret")),
            ],
        );
        p.add(
            "GD_Syn.Gun.Balance",
            "WillowGame.WeaponBalanceDefinition",
            vec![
                ("InventoryDefinition", r("GD_Syn.Gun.Type")),
                ("WeaponPartListCollection", r("GD_Syn.Gun.Coll")),
                (
                    "Manufacturers",
                    makers(vec![(BETA, vec![grade(1, 100, None)])]),
                ),
            ],
        );
        // Chains that do not reach a root: a base no package holds, and a two-step cycle.
        p.add(
            "GD_Syn.Grades.Orphan",
            IB,
            vec![("BaseDefinition", r("GD_Syn.Grades.Nowhere"))],
        );
        p.add(
            "GD_Syn.Grades.Loop_A",
            IB,
            vec![("BaseDefinition", r("GD_Syn.Grades.Loop_B"))],
        );
        p.add(
            "GD_Syn.Grades.Loop_B",
            IB,
            vec![("BaseDefinition", r("GD_Syn.Grades.Loop_A"))],
        );
        add_slot_titles(&mut p);
        p
    }

    /// Parts with titles in two slots whose sorted order differs from the slot order (Body before Barrel; Gamma
    /// before Delta): the name parts follow the slot order, as the tool walks its slot dict.
    fn add_slot_titles(p: &mut Syn) {
        p.add(
            "GD_Syn.Names.Title_Body",
            "WillowGame.WeaponNamePartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Names.Title_Barrel",
            "WillowGame.WeaponNamePartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Names.Title_Gamma",
            "WillowGame.ItemNamePartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Names.Title_Delta",
            "WillowGame.ItemNamePartDefinition",
            vec![],
        );
        p.add(
            "GD_Syn.Gun.Body_T",
            "WillowGame.WeaponPartDefinition",
            one("TitleList", vec![r("GD_Syn.Names.Title_Body")]),
        );
        p.add(
            "GD_Syn.Gun.Barrel_T",
            "WillowGame.WeaponPartDefinition",
            one("TitleList", vec![r("GD_Syn.Names.Title_Barrel")]),
        );
        p.add(
            "GD_Syn.Gun.Coll_Titles",
            WCOLL,
            vec![
                (
                    "BodyPartData",
                    part_data(vec![w("GD_Syn.Gun.Body_T")], true),
                ),
                (
                    "BarrelPartData",
                    part_data(vec![w("GD_Syn.Gun.Barrel_T")], true),
                ),
                mode("EPRM_Selective"),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Gun.Titles",
            "WillowGame.WeaponBalanceDefinition",
            vec![
                ("InventoryDefinition", r("GD_Syn.Gun.Type")),
                ("WeaponPartListCollection", r("GD_Syn.Gun.Coll_Titles")),
            ],
        );
        p.add(
            "GD_Syn.Grenade.Gamma_T",
            "WillowGame.GrenadeModPartDefinition",
            one("TitleList", vec![r("GD_Syn.Names.Title_Gamma")]),
        );
        p.add(
            "GD_Syn.Grenade.Delta_T",
            "WillowGame.GrenadeModPartDefinition",
            one("TitleList", vec![r("GD_Syn.Names.Title_Delta")]),
        );
        p.add(
            "GD_Syn.Grenade.Coll_Titles",
            COLL,
            vec![
                (
                    "GammaPartData",
                    part_data(vec![w("GD_Syn.Grenade.Gamma_T")], true),
                ),
                (
                    "DeltaPartData",
                    part_data(vec![w("GD_Syn.Grenade.Delta_T")], true),
                ),
                consolidated(),
            ],
        );
        p.add(
            "GD_Syn.Grades.Grenade_Titles",
            IB,
            vec![
                ("InventoryDefinition", r("GD_Syn.Grenade.Def")),
                ("PartListCollection", r("GD_Syn.Grenade.Coll_Titles")),
            ],
        );
    }

    fn dlc_package() -> Syn {
        let mut p = Syn::default();
        // The same path again: the base game package owns it.
        p.add(ALPHA, "Engine.ManufacturerDefinition", vec![]);
        // The same path with another class.
        p.add(
            "GD_Syn.Grades.Usable",
            "WillowGame.ItemPoolDefinition",
            vec![],
        );
        p.add(
            "GD_SynDLC.Grades.Shield",
            IB,
            vec![
                // An import: the base lives in the base game's package.
                ("BaseDefinition", r("GD_Syn.Grades.Shield_Base")),
                (
                    "Manufacturers",
                    makers(vec![(BETA, vec![grade(1, 100, Some("GD_Syn.Shield.Def"))])]),
                ),
            ],
        );
        p
    }

    const BASE_FILE: &str = "WillowGame/CookedPCConsole/SynStartup.upk";
    const DLC_FILE: &str = "DLC/SynDLC/Content/SynDLC_Startup.upk";

    /// A temporary install, removed on drop.
    struct Install(PathBuf);

    impl Install {
        fn new(tag: &str) -> Install {
            Install::at(
                std::env::temp_dir()
                    .join(format!("ow-items-registry-{tag}-{}", std::process::id())),
            )
        }

        fn at(dir: PathBuf) -> Install {
            let _ = std::fs::remove_dir_all(&dir);
            for (name, bytes) in [
                (BASE_FILE, base_package().build()),
                (DLC_FILE, dlc_package().build()),
            ] {
                let path = dir.join(name);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, bytes).unwrap();
            }
            Install(std::path::absolute(&dir).unwrap())
        }

        /// `weapon_catalog.package_files` + `RegistryCensus.add`'s input: the base game first, then the DLC folder,
        /// every export in export order. The stand-in for `DefStore::census()` until the store's body lands.
        fn census(&self) -> CensusInput {
            let mut input = CensusInput::new(self.0.clone());
            input.files = vec![self.0.join(BASE_FILE), self.0.join(DLC_FILE)]
                .into_iter()
                .map(|f| Package::load(&f).unwrap().source.clone())
                .collect();
            let mut index: DetMap<String, usize> = DetMap::new();
            for name in [BASE_FILE, DLC_FILE] {
                let pkg = Package::load(&self.0.join(name)).unwrap();
                for i in 1..=pkg.exports.len() as i32 {
                    let path = pkg.path_of(i).unwrap();
                    let class = match pkg.object(i).unwrap().class {
                        0 => "Class".to_string(),
                        c => pkg.path_of(c).unwrap(),
                    };
                    if is_class_default(&path) {
                        input.defaults.push((path, class));
                        continue;
                    }
                    let holder = Holder::new(pkg.source.clone(), class, i);
                    match index.get(&path) {
                        Some(&at) => input.paths[at].holders.push(holder),
                        None => {
                            index.insert(path.clone(), input.paths.len());
                            input.paths.push(PathHolders::new(path, vec![holder]));
                        }
                    }
                }
            }
            input
        }

        fn registry(&self) -> Registry {
            build_from_census(self.census(), "Game")
        }
    }

    impl Drop for Install {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn parts_of(slot: &Pv) -> Vec<&str> {
        slot.items()
            .iter()
            .map(|c| c.at("part").as_str().unwrap())
            .collect()
    }

    fn strs(v: &Pv) -> Vec<&str> {
        v.items()
            .iter()
            .map(|x| x.as_str().unwrap_or("<null>"))
            .collect()
    }

    fn slots_parts(variant: &Pv) -> Vec<(String, Vec<String>)> {
        let Pv::Map(m) = variant.at("slots") else {
            panic!("slots")
        };
        m.iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    parts_of(v).into_iter().map(String::from).collect(),
                )
            })
            .collect()
    }

    /// A report field that is a dict of counts, as (key, count) rows.
    fn counts(v: &Pv) -> Vec<(&str, i64)> {
        let Pv::Map(m) = v else {
            panic!("not a dict: {v:?}")
        };
        m.iter()
            .map(|(k, v)| match v {
                Pv::Int(n) => (k.as_str(), *n),
                other => panic!("not a count: {other:?}"),
            })
            .collect()
    }

    fn modes(variant: &Pv) -> Vec<&str> {
        variant
            .at("merge")
            .items()
            .iter()
            .map(|m| m.at("mode").as_str().unwrap())
            .collect()
    }

    #[test]
    fn one_balance_per_kind() {
        let install = Install::new("kinds");
        let reg = install.registry();
        assert_eq!(reg.report().at("scan_errors"), &Pv::Map(DetMap::new()));
        for (path, kind) in [
            ("GD_Syn.Grades.Shield_Leaf", "shield"),
            ("GD_Syn.Grades.Grenade", "grenade_mod"),
            ("GD_Syn.ClassMod.Bal", "class_mod"),
            ("GD_Syn.Grades.Relic", "relic"),
            ("GD_Syn.Grades.Usable", "usable"),
            ("GD_Syn.Grades.Custom", "customization"),
            ("GD_Syn.Grades.Mission", "mission_item"),
            ("GD_Syn.Gun.Balance", "weapon"),
        ] {
            assert_eq!(
                reg.balance(path).unwrap().at("kind").as_str(),
                Some(kind),
                "{path}"
            );
        }
        assert_eq!(reg.tree().at("schema").as_str(), Some(SCHEMA_TAG));
    }

    #[test]
    fn base_chain_and_merge() {
        let install = Install::new("chain");
        let reg = install.registry();
        let leaf = reg.balance("GD_Syn.Grades.Shield_Leaf").unwrap();
        assert_eq!(
            strs(leaf.at("chain")),
            ["GD_Syn.Grades.Shield_Leaf", "GD_Syn.Grades.Shield_Base"]
        );
        assert_eq!(leaf.at("chain_end").as_str(), Some("root"));
        assert_eq!(
            leaf.at("inventory_definition").as_str(),
            Some("GD_Syn.Shield.Def")
        );
        assert_eq!(leaf.at("gestalt").as_str(), Some("GD_Syn.Gestalt.Shield"));
        let variant = &leaf.at("variants").items()[0];
        let slots = variant.at("slots");
        // The definition list (Body_A, Body_B), the additive collection appends Cap_A, the selective one replaces
        // Alpha.
        assert_eq!(parts_of(slots.at("Alpha")), ["GD_Syn.Shield.Body_B"]);
        assert_eq!(
            parts_of(slots.at("Beta")),
            ["GD_Syn.Shield.Cap_B", "GD_Syn.Shield.Ghost"]
        );
        assert!(slots.get("Gamma").is_none(), "a disabled slot adds nothing");
        assert_eq!(
            modes(variant),
            ["definition", "EPRM_Additive", "EPRM_Selective"]
        );
        let base = reg.balance("GD_Syn.Grades.Shield_Base").unwrap();
        let base_slots = base.at("variants").items()[0].at("slots");
        assert_eq!(
            parts_of(base_slots.at("Alpha")),
            [
                "GD_Syn.Shield.Body_A",
                "GD_Syn.Shield.Body_B",
                "GD_Syn.Shield.Cap_A"
            ]
        );
    }

    #[test]
    fn weights_windows_and_manufacturers() {
        let install = Install::new("weights");
        let reg = install.registry();
        let leaf = reg.balance("GD_Syn.Grades.Shield_Leaf").unwrap().clone();
        let slots = leaf.at("variants").items()[0].at("slots").clone();
        let body_b = &slots.at("Alpha").items()[0];
        let mut by = DetMap::new();
        by.insert(ALPHA.to_string(), Pv::Float(7.0));
        let expected = Pv::map([
            ("source", Pv::text("manufacturer")),
            ("by_manufacturer", Pv::Map(by)),
            ("default", Pv::Float(1.0)),
        ]);
        assert_eq!(body_b.at("weight"), &expected);
        assert_eq!(
            (body_b.at("min_stage"), body_b.at("max_stage")),
            (&Pv::Float(5.0), &Pv::Float(30.0))
        );
        let flat = &slots.at("Beta").items()[0];
        assert_eq!(
            flat.at("weight"),
            &Pv::map([("source", Pv::text("flat")), ("value", Pv::Float(100.0))])
        );
        let maker = &leaf.at("manufacturers").items()[0];
        assert_eq!(
            maker.at("manufacturer").as_str(),
            Some(ALPHA),
            "inherited from the base balance"
        );
        let windows: Vec<(&Pv, &Pv)> = maker
            .at("grades")
            .items()
            .iter()
            .map(|g| (g.at("min_stage"), g.at("max_stage")))
            .collect();
        assert_eq!(
            windows,
            [(&Pv::Int(1), &Pv::Int(4)), (&Pv::Int(5), &Pv::Int(50))]
        );
        const LEAF: &str = "GD_Syn.Grades.Shield_Leaf";
        let at = reg.candidates_at(LEAF, 0, 10, Some(ALPHA)).unwrap();
        assert_eq!(
            at.get("Alpha").unwrap().iter().collect::<Vec<_>>(),
            [(&"GD_Syn.Shield.Body_B".to_string(), &Some(7.0))]
        );
        assert!(
            !reg.candidates_at(LEAF, 0, 3, Some(ALPHA))
                .unwrap()
                .contains_key("Alpha"),
            "below its window"
        );
        let beta = reg.candidates_at(LEAF, 0, 10, Some(BETA)).unwrap();
        assert_eq!(
            beta.get("Alpha").unwrap().get("GD_Syn.Shield.Body_B"),
            Some(&Some(1.0)),
            "another maker takes DefaultWeight"
        );
    }

    #[test]
    fn class_mod_variants_and_names() {
        let install = Install::new("classmod");
        let reg = install.registry();
        let record = reg.balance("GD_Syn.ClassMod.Bal").unwrap();
        let defs: Vec<&str> = record
            .at("variants")
            .items()
            .iter()
            .map(|v| v.at("definition").as_str().unwrap())
            .collect();
        assert_eq!(defs, ["GD_Syn.ClassMod.Def_A", "GD_Syn.ClassMod.Def_B"]);
        let makers: Vec<&str> = record
            .at("manufacturers")
            .items()
            .iter()
            .map(|m| m.at("manufacturer").as_str().unwrap())
            .collect();
        assert_eq!(makers, [ALPHA, BETA]);
        let base = reg.balance("GD_Syn.Grades.Shield_Base").unwrap();
        assert_eq!(
            strs(base.at("name_parts").at("title")),
            ["GD_Syn.Names.Title_Shield"]
        );
        let gun = reg.balance("GD_Syn.Gun.Balance").unwrap();
        assert_eq!(
            strs(gun.at("name_parts").at("title")),
            ["GD_Syn.Names.Title_Gun"]
        );
    }

    #[test]
    fn census_reports_missing_and_broken_chains() {
        let install = Install::new("broken");
        let reg = install.registry();
        let parts = reg.report().at("unresolved").at("part");
        assert_eq!(
            strs(&Pv::List(
                parts.items().iter().map(|r| r.at("path").clone()).collect()
            )),
            ["GD_Syn.Shield.Ghost"]
        );
        assert_eq!(
            parts.items()[0].at("reason").as_str(),
            Some("no installed package exports it")
        );
        let orphan = reg.balance("GD_Syn.Grades.Orphan").unwrap();
        assert_eq!(
            (
                orphan.at("chain_end").as_str(),
                orphan.at("chain_missing").as_str()
            ),
            (Some("missing"), Some("GD_Syn.Grades.Nowhere"))
        );
        assert_eq!(
            reg.balance("GD_Syn.Grades.Loop_A")
                .unwrap()
                .at("chain_end")
                .as_str(),
            Some("cycle")
        );
        assert_eq!(
            counts(reg.report().at("definitions_without_balance_by_kind")),
            [("mission_item", 1)]
        );
        assert_eq!(
            counts(reg.report().at("skipped")),
            [(
                "ShieldDefinition: class default object (Default__), not a game object",
                1
            )]
        );
    }

    #[test]
    fn duplicate_paths_and_dlc() {
        let install = Install::new("dlc");
        let reg = install.registry();
        assert_eq!(
            counts(reg.report().at("paths_in_several_packages_by_class")),
            [
                ("InventoryBalanceDefinition", 1),
                ("ManufacturerDefinition", 1)
            ]
        );
        let two: Vec<&str> = reg
            .report()
            .at("paths_with_two_classes")
            .items()
            .iter()
            .map(|c| c.at("path").as_str().unwrap())
            .collect();
        assert_eq!(two, ["GD_Syn.Grades.Usable"]);
        let alpha = reg.tree().at("manufacturers").at(ALPHA);
        assert_eq!(
            (alpha.at("dlc").as_str(), alpha.at("package").as_str()),
            (Some("base"), Some(BASE_FILE))
        );
        let dlc = reg.balance("GD_SynDLC.Grades.Shield").unwrap();
        assert_eq!(dlc.at("dlc").as_str(), Some("SynDLC"));
        assert_eq!(
            strs(dlc.at("chain")),
            ["GD_SynDLC.Grades.Shield", "GD_Syn.Grades.Shield_Base"]
        );
        assert_eq!(dlc.at("chain_end").as_str(), Some("root"));
        assert_eq!(dlc.at("kind").as_str(), Some("shield"));
        let grade = &dlc.at("manufacturers").items()[0].at("grades").items()[0];
        assert_eq!(grade.at("definition").as_str(), Some("GD_Syn.Shield.Def"));
        assert_eq!(
            counts(reg.report().at("by_dlc").at("SynDLC")),
            [("balances_shield", 1)]
        );
    }

    #[test]
    fn the_stock_shield_candidates_at_a_level() {
        // The tool's check_stock_shield on the base balance at level 10: Alpha holds Body_A, Body_B (10 is inside
        // 5..30) and Cap_A; Beta holds Cap_B and Ghost (it weighs 100 but no package exports it: kept).
        let install = Install::new("stock");
        let reg = install.registry();
        let record = reg.balance("GD_Syn.Grades.Shield_Base").unwrap().clone();
        let slots = record.at("variants").items()[0].at("slots").clone();
        let maker = record.at("manufacturers").items()[0]
            .at("manufacturer")
            .as_str()
            .map(String::from);
        let at = reg.candidates_at_slots(&slots, false, 10, maker.as_deref());
        let sizes: Vec<(&str, usize)> = at.iter().map(|(k, v)| (k.as_str(), v.len())).collect();
        assert_eq!(sizes, [("Alpha", 3), ("Beta", 2)]);
        assert_eq!(at.values().map(|v| v.len()).product::<usize>(), 6);
    }

    #[test]
    fn modes_and_skipped_rows() {
        let install = Install::new("modes");
        let reg = install.registry();
        let grenade = &reg
            .balance("GD_Syn.Grades.Grenade")
            .unwrap()
            .at("variants")
            .items()[0];
        assert_eq!(
            parts_of(grenade.at("slots").at("Delta")),
            ["GD_Syn.Grenade.Part", "GD_Syn.Grenade.Part2"]
        );
        assert_eq!(modes(grenade), ["definition", "EPRM_Additive"]);
        let complete = &reg
            .balance("GD_Syn.Gun.Complete")
            .unwrap()
            .at("variants")
            .items()[0];
        assert_eq!(
            slots_parts(complete),
            [("Grip".to_string(), vec!["GD_Syn.Gun.Grip_A".to_string()])]
        );
        // The row is counted each time a balance reads its list (the tool's describe_entries): two grenade balances
        // name the definition that holds it.
        assert_eq!(
            reg.report().at("part_list_rows_naming_no_part").at("rows"),
            &Pv::Int(2)
        );
        assert_eq!(
            reg.report().at("name_list_entries_empty").at("entries"),
            &Pv::Int(1)
        );
        assert_eq!(
            reg.tree().at("parts").at("GD_Syn.Shield.Cap_A").at("title"),
            &Pv::List(Vec::new())
        );
    }

    #[test]
    fn weapon_list_under_an_inventory_balance() {
        let install = Install::new("turret");
        let reg = install.registry();
        let turret = reg.balance("GD_Syn.Grades.Turret").unwrap();
        assert_eq!(
            turret.at("kind").as_str(),
            Some("weapon_by_inventory_balance")
        );
        let variant = &turret.at("variants").items()[0];
        assert_eq!(
            slots_parts(variant),
            [("Sight".to_string(), vec!["GD_Syn.Gun.Sight_A".to_string()])]
        );
    }

    #[test]
    fn unlisted_classes() {
        let install = Install::new("unlisted");
        let reg = install.registry();
        assert_eq!(
            counts(reg.report().at("unlisted_classes").at("export_classes")),
            [("SynthOddBalanceDefinition", 1)]
        );
        assert_eq!(
            counts(
                reg.report()
                    .at("unlisted_classes")
                    .at("part_data_properties")
            ),
            [("IotaPartData", 1)]
        );
        assert!(reg.balance("GD_Syn.Odd.Balance").is_none());
    }

    #[test]
    fn same_bytes_twice() {
        let install = Install::new("twice");
        let first = install.registry().to_json();
        assert!(!first.contains("\"generated\""));
        assert_eq!(first, install.registry().to_json());
    }

    /// The install's census as `DefStore::census` gives it: every file of the store's scan in the store's order
    /// (path order, so `DLC/` before `WillowGame/`), each path with every holder, plus holders in files the tool does
    /// not list (a `.umap`, a package outside the two folders), which must not change the owner.
    fn store_census(install: &Install) -> ow_defs::identity::Census {
        use ow_defs::identity::{Census, CensusEntry, Owner};
        let tool = install.census();
        let decoy_map = install.0.join("DLC/SynDLC/Content/Syn_Map.umap");
        let decoy_pkg = install.0.join("Engine/SynEngine.upk");
        let mut census = Census::default();
        census.files = vec![
            decoy_map.clone(),
            install.0.join(DLC_FILE),
            decoy_pkg.clone(),
            install.0.join(BASE_FILE),
        ];
        let owner = |h: &Holder| {
            let stem = h.file.file_stem().unwrap().to_string_lossy().into_owned();
            Owner::new(stem, h.file.clone(), h.export, h.class.clone(), None)
        };
        // Store order: the DLC file's exports first, then the base file's.
        let mut entries: Vec<CensusEntry> = Vec::new();
        for file in [install.0.join(DLC_FILE), install.0.join(BASE_FILE)] {
            for p in &tool.paths {
                for h in p.holders.iter().filter(|h| h.file == file) {
                    let holder = owner(h);
                    match entries.iter_mut().find(|e| e.path == p.path) {
                        Some(e) => e.holders.push(holder),
                        None => entries.push(CensusEntry::new(p.path.clone(), vec![holder])),
                    }
                }
            }
        }
        // Decoys: a map and a stray package holding the manufacturer as another class, listed first.
        for e in entries.iter_mut().filter(|e| e.path == ALPHA) {
            e.holders.insert(
                0,
                Owner::new(
                    "Syn_Map".into(),
                    decoy_map.clone(),
                    1,
                    "WillowGame.Odd".into(),
                    None,
                ),
            );
            e.holders.insert(
                1,
                Owner::new(
                    "SynEngine".into(),
                    decoy_pkg.clone(),
                    1,
                    "Engine.Odd".into(),
                    None,
                ),
            );
        }
        census.entries = entries;
        let base = Package::load(&install.0.join(BASE_FILE)).unwrap();
        for (path, class) in &tool.defaults {
            let export = base.find_export(path).unwrap().unwrap();
            let h = Holder::new(base.source.clone(), class.clone(), export);
            census
                .class_defaults
                .push(CensusEntry::new(path.clone(), vec![owner(&h)]));
        }
        census
    }

    #[test]
    fn the_store_census_in_the_tools_order_gives_the_same_bytes() {
        let install = Install::new("storeorder");
        let tool = install.registry().to_json();
        let input = CensusInput::from_store(store_census(&install), &install.0);
        assert_eq!(input, install.census());
        assert_eq!(build_from_census(input, "Game").to_json(), tool);
    }

    #[test]
    fn scan_errors_are_keyed_by_the_posix_path_with_the_readers_line() {
        use ow_defs::identity::FileFailure;
        use ow_defs::store::DefError;
        let install = Install::new("scanerrors");
        let mut census = store_census(&install);
        let broken = install.0.join("DLC/SynDLC/Content/Broken.upk");
        let outside = install.0.join("Engine/Broken.upk");
        census.files.push(broken.clone());
        census.files.push(outside.clone());
        let failure = |file: &PathBuf| {
            let error = DefError::Io {
                path: "x".into(),
                reason: "bad magic".into(),
            };
            FileFailure::new(file.clone(), error)
        };
        census.failures = vec![failure(&broken), failure(&outside)];
        let reg = build_from_census(CensusInput::from_store(census, &install.0), "Game");
        let Pv::Map(m) = reg.report().at("scan_errors") else {
            panic!("scan_errors")
        };
        let errors: Vec<(&str, &str)> = m
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str().unwrap()))
            .collect();
        // The file outside the tool's list is not the tool's scan error.
        assert_eq!(
            errors,
            [("DLC/SynDLC/Content/Broken.upk", "ow-package: x: bad magic")]
        );
    }

    #[test]
    fn a_path_exported_twice_in_one_file_reads_its_last_export() {
        // weapon_recipe.Package.index keeps the last export of a path; the census lists both, first one first.
        let mut p = Syn::default();
        p.add(
            "GD_Dup.Gestalt_A",
            "GearboxFramework.GestaltSkeletalMeshDefinition",
            vec![],
        );
        p.add(
            "GD_Dup.Gestalt_B",
            "GearboxFramework.GestaltSkeletalMeshDefinition",
            vec![],
        );
        p.add(
            "GD_Dup.Def",
            "WillowGame.ShieldDefinition",
            vec![("GestaltMesh", r("GD_Dup.Gestalt_A"))],
        );
        p.add(
            "GD_Dup.Def",
            "WillowGame.ShieldDefinition",
            vec![("GestaltMesh", r("GD_Dup.Gestalt_B"))],
        );
        let dir =
            std::env::temp_dir().join(format!("ow-items-registry-dup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join(BASE_FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, p.build()).unwrap();
        let root = std::path::absolute(&dir).unwrap();
        let pkg = Package::load(&root.join(BASE_FILE)).unwrap();
        let mut input = CensusInput::new(root.clone());
        let mut seen: Vec<String> = Vec::new();
        for i in 1..=pkg.exports.len() as i32 {
            let path = pkg.path_of(i).unwrap();
            let class = pkg.path_of(pkg.object(i).unwrap().class).unwrap();
            let h = Holder::new(pkg.source.clone(), class, i);
            match seen.iter().position(|s| *s == path) {
                Some(at) => input.paths[at].holders.push(h),
                None => {
                    seen.push(path.clone());
                    input.paths.push(PathHolders::new(path, vec![h]));
                }
            }
        }
        let reg = build_from_census(input, "Game");
        let def = reg.tree().at("definitions").at("GD_Dup.Def");
        assert_eq!(def.at("gestalt").as_str(), Some("GD_Dup.Gestalt_B"));
        assert_eq!(
            reg.report().at("duplicate_export_paths_all_classes"),
            &Pv::Int(1)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn name_parts_follow_the_slot_order_not_the_sorted_keys() {
        let install = Install::new("slotorder");
        let reg = install.registry();
        let gun = reg.balance("GD_Syn.Gun.Titles").unwrap();
        assert_eq!(
            strs(gun.at("name_parts").at("title")),
            [
                "GD_Syn.Names.Title_Gun",
                "GD_Syn.Names.Title_Body",
                "GD_Syn.Names.Title_Barrel"
            ]
        );
        let grenade = reg.balance("GD_Syn.Grades.Grenade_Titles").unwrap();
        assert_eq!(
            strs(grenade.at("name_parts").at("title")),
            ["GD_Syn.Names.Title_Gamma", "GD_Syn.Names.Title_Delta"]
        );
        let at = reg
            .candidates_at("GD_Syn.Grades.Grenade_Titles", 0, 10, None)
            .unwrap();
        assert_eq!(
            at.keys().map(String::as_str).collect::<Vec<_>>(),
            ["Gamma", "Delta"]
        );
    }

    #[test]
    fn cyclic_init_data_gives_no_value_instead_of_overflowing() {
        // An init definition whose formula's multiplier is initialised by the same definition: the tool's
        // attribute_value recurses until Python raises RecursionError, which init_value reads as no value.
        let mut p = Syn::default();
        let cyclic = V::Struct(
            "AttributeInitializationData",
            vec![
                ("InitializationDefinition", r("GD_Cyc.Init")),
                ("BaseValueConstant", V::Float(1.0)),
            ],
        );
        p.add(
            "GD_Cyc.Init",
            "GearboxFramework.AttributeInitializationDefinition",
            vec![(
                "ValueFormula",
                V::Struct(
                    "AttributeInitializationValueFormula",
                    vec![("bEnabled", V::Bool(true)), ("Multiplier", cyclic)],
                ),
            )],
        );
        let dir =
            std::env::temp_dir().join(format!("ow-items-registry-cyc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join(BASE_FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, p.build()).unwrap();
        let pkg = Package::load(&file).unwrap();
        let mut input = CensusInput::new(std::path::absolute(&dir).unwrap());
        let class = pkg.path_of(pkg.object(1).unwrap().class).unwrap();
        input.paths.push(PathHolders::new(
            "GD_Cyc.Init".into(),
            vec![Holder::new(pkg.source.clone(), class, 1)],
        ));
        let mut census = Census::new(input);
        let mut init = DetMap::new();
        init.insert(
            "InitializationDefinition".to_string(),
            Pv::text("GD_Cyc.Init"),
        );
        assert_eq!(
            attribute_value(&mut census, &Pv::Map(init.clone())),
            Err(())
        );
        assert!(matches!(
            init_value(&mut census, &[Pv::Map(init)], &Pv::Int(0)),
            Pv::Map(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_store_census_is_not_there_yet() {
        // Flips when the body of DefStore::census() lands (W3-DEFS): build_registry then runs on the store.
        let install = Install::new("store");
        let mode = ow_core::ParityMode::Off;
        let mut defs =
            DefStore::open(install.0.clone(), IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let mut space = ObjectSpace::new();
        let error = build_registry(&mut defs, &mut space).unwrap_err();
        assert!(matches!(error, ItemError::NotYet(_)), "{error}");
    }

    #[test]
    fn floats_read_as_json_loads_reads_the_cxx_text() {
        assert_eq!(float_pv(1.0), Pv::Int(1));
        assert_eq!(float_pv(-2.0), Pv::Int(-2));
        assert_eq!(float_pv(-0.0), Pv::Int(0));
        assert_eq!(float_pv(0.1), Pv::Float(0.100000001));
        assert_eq!(float_pv(1e30), Pv::Float(1.00000002e30));
        assert_eq!(Pv::Float(0.100000001).to_json(), "0.100000001");
        assert_eq!(Pv::Float(f32r(0.1)).to_json(), "0.10000000149011612");
        assert_eq!(Pv::Float(100.0).to_json(), "100.0");
    }

    #[test]
    fn json_is_sorted_and_python_escaped() {
        let v = Pv::map([
            ("b", Pv::List(vec![Pv::Null, Pv::Bool(true)])),
            ("a", Pv::text("x\"\n\u{e9}")),
        ]);
        assert_eq!(v.to_json(), "{\"a\":\"x\\\"\\n\u{e9}\",\"b\":[null,true]}");
    }

    #[test]
    fn attribute_values_follow_weapon_recipe() {
        let mut census = Census::new(CensusInput::default());
        let init = |c: f64, s: Option<f64>| {
            let mut m = DetMap::new();
            m.insert("BaseValueConstant".to_string(), Pv::Float(c));
            if let Some(s) = s {
                m.insert("BaseValueScaleConstant".to_string(), Pv::Float(s));
            }
            Pv::Map(m)
        };
        // f32(f32(0.1) * 3) in single precision.
        assert_eq!(
            attribute_value(&mut census, &init(0.1, Some(3.0))),
            Ok(Some(f32r(f32r(0.1) * 3.0)))
        );
        assert_eq!(
            attribute_value(&mut census, &init(2.0, None)),
            Ok(Some(2.0))
        );
        let mut needs = DetMap::new();
        needs.insert(
            "BaseValueAttribute".to_string(),
            Pv::text("D_Attributes.Syn"),
        );
        assert_eq!(attribute_value(&mut census, &Pv::Map(needs)), Ok(None));
        let mut unknown = DetMap::new();
        unknown.insert(
            "InitializationDefinition".to_string(),
            Pv::text("GD_Syn.Nowhere"),
        );
        assert_eq!(attribute_value(&mut census, &Pv::Map(unknown)), Err(()));
        assert_eq!(formula_value(2.0, 3.0, 2.0, 1.0), Ok(20.0));
        assert_eq!(formula_value(2.0, 0.0, -1.0, 1.0), Err(()));
    }

    #[test]
    #[cfg(not(windows))]
    fn on_posix_the_tool_lists_only_lower_case_upk() {
        // rglob('*.upk') matches the case there; on Windows it folds it.
        let root = Path::new("/game");
        assert_eq!(
            tool_file_key(Path::new("/game/WillowGame/CookedPCConsole/Foo.UPK"), root),
            None
        );
        assert!(
            tool_file_key(Path::new("/game/WillowGame/CookedPCConsole/Foo.upk"), root).is_some()
        );
        // POSIX pathlib keeps a leading `//`.
        assert_eq!(python_path_text("//x/y"), "//x/y");
        assert_eq!(python_path_text("/x//y/"), "/x/y");
    }

    #[test]
    fn python_paths_print_as_pathlib_does() {
        if cfg!(windows) {
            assert_eq!(
                python_path_text("C:/Games/Borderlands 2/"),
                "C:\\Games\\Borderlands 2"
            );
            assert_eq!(python_path_text("C:\\Games\\.\\BL2"), "C:\\Games\\BL2");
            assert_eq!(python_path_text("C:/"), "C:\\");
            assert_eq!(
                python_path_text("\\\\server\\share\\BL2\\"),
                "\\\\server\\share\\BL2"
            );
            assert_eq!(python_path_text("//server/share"), "\\\\server\\share\\");
        }
        assert_eq!(python_path_text(""), ".");
    }

    /// Not a gate: writes the fixture install and the Rust registry under `$OW_ITEM_REGISTRY_DIFF_DIR`, so that the
    /// tool's own registry of the same packages can be compared byte for byte (see the lane handoff).
    #[test]
    #[ignore = "writes a fixture for a manual comparison with tools/item_registry.py"]
    fn write_the_fixture_for_the_tool() {
        let Ok(dir) = std::env::var("OW_ITEM_REGISTRY_DIFF_DIR") else {
            return;
        };
        let dir = PathBuf::from(dir);
        let install = Install::at(dir.join("Game"));
        let game = python_path_text(&install.0.to_string_lossy());
        let reg = build_from_census(install.census(), &game);
        std::fs::write(dir.join("rust_registry.json"), reg.to_json()).unwrap();
        std::fs::write(dir.join("rust_census.json"), reg.report().to_json()).unwrap();
        std::mem::forget(install);
    }
}
