//! Owner: lane W4-POOLS. The item pool roll (architecture section 3.7): a pool, an `ItemPoolListDefinition` or an
//! `AIPawnBalanceDefinition` rolled from one generator into the balances it drops.
//!
//! **Two modes** (rule R3; the mode is [`RollCx::mode`]):
//!
//! - [`ParityMode::Reference`] reproduces the oracle, `tools/loot_pools.py`'s CrtRand path (`roll_crt` /
//!   `roll_pool_crt`, the way `tools/weapon_gen_parity.py` `pool_drops` calls them), so the 7,680 pool rolls of the
//!   lane's gate can be compared on this output. It reads values the way the tool's `Values` does: constants,
//!   `ValueFormula` definitions with their enabled range restrictions, constant and conditional value resolvers (the
//!   resolver chain, or the attribute's resolver subobjects in export order), a designer attribute's `BaseValue`, and
//!   `NumberOfPlayers` from the snapshot; anything else is unresolved and weighs nothing. Numbers keep Python's int
//!   and float kinds ([`PyNum`]), because CPython's `sum()` adds an int into a float total without compensation.
//!   The tool's rules, all UNVERIFIED as the game's:
//!   - a listed pool draws one `rand()` (none when its probability is unresolved or its tree is not eligible) and
//!     fires when the probability is at least 1 or `rand()/32767` is below it;
//!   - a pool makes `round_half_even(Quantity)` draws (a missing or zero quantity counts as 1, a negative one as 0;
//!     [`ow_core::num::round_half_even`] is the oracle's rounding, UNVERIFIED as a game rule, decision table finding
//!     17); each draw is `total * rand()/32767` over the shares of the entries with a share, and the first entry whose
//!     closed interval holds it wins (the last one when rounding leaves the draw past every interval);
//!   - a sub-pool is expanded at the caller's stage, with a cycle guard and a depth of 12, as the tool's `expand`;
//!   - eligibility: a pool is eligible when its stage gate is not false and it holds an item entry or an eligible
//!     sub-pool; an ineligible sub-pool weighs 0, and an ineligible pool spawns nothing and draws nothing.
//!
//!   The oracle draws no manufacturer and rolls no item: a drop is the balance path and the chain of pools above it.
//! - [`ParityMode::Off`] is `ItemPool.SpawnBalancedInventoryFromPool` as NATIVE_LOOT.md reads it from native code
//!   (UNVERIFIED in game), every value through the `ow-attr` evaluator with no context source (offline):
//!   - the gate: the stage requirements truncated, `(min == 0 or (min <= stage and min <= cap)) and (max == 0 or
//!     stage <= max)`, and a reachable entry (a nested pool with entries that passes the same gate, or a balance);
//!     a pool that fails it spawns nothing and draws nothing;
//!   - one flat candidate list per pool in `BalancedItems` order: a nested pool with entries is one candidate (its
//!     weight 0 unless it passes the gate at the caller's stage); a balance entry is expanded at the pool level into
//!     one candidate per manufacturer ([`crate::candidates::expand`], W4-ROLL) at `min(stage, cap)`, each weighing its
//!     grade modifier times the entry weight over the number of candidates, in single precision; a candidate at or
//!     below 1e-8 is left out, and one identical to an earlier candidate replaces its weight in place;
//!   - the caller's variance formula (`GameStageVarianceFormula`): only on a pool with `bSupportsGameStageVariance`
//!     (class default true), the balance entry's stage plus the truncated variance, at least 1 when the caller's stage
//!     is above 0 (else at least 0), then raised to the pool's minimum and lowered to its maximum when those are above
//!     0;
//!   - `Quantity` truncated draws with replacement, `r = total * rand()/32767` in single precision, the first closed
//!     interval holding `r`; a picked nested pool rolls again with the same stage and variance;
//!   - `bDropOnDeath` of a drop: the entry's flag AND the flags of the entries above it ([`PoolDrop::drop_on_death`],
//!     `None` while an untagged flag's struct default is not read);
//!   - a source's pool list: `AIPawnBalanceDefinition.SetupPawnItemPoolList` (the playthrough entry
//!     `GetPlayThroughIndex` picks, its custom lists replacing the defaults when non-empty, included lists first) and
//!     `ItemPoolListDefinition.AddToItemPoolList` (included lists first, depth first); the death roll
//!     (`WillowPawn.DropLootOnDeath`, script) keeps the pools whose `bAutoReadyItems` is false (class default true) and
//!     fires a pool when its chance is above 0 and `FRand() <= chance` (single precision, drawn only for a positive
//!     chance).
//!
//!   The candidate a drop carries is what [`crate::roll::roll_candidate`] rolls once the caller has the item's
//!   instance (architecture section 3.7); this module rolls no item.
//!
//! **Off mode reads an untagged field** (`Quantity`, the stage requirements, the flags, the lists) along the class's
//! default-object chain (`Default__KeyedItemPoolDefinition`, then `Default__ItemPoolDefinition`, and the like, which
//! [`PoolData::one_file`] adds from the census), as the engine fills an untagged property from the class default.
//! When no default object of the chain tags it, the chain's end gives no object, `false` or an empty array, and a
//! struct (`Quantity`) is `NotYet`; a chain step whose default object is not in the scan is `NotYet`. A value that is
//! tagged but was not decoded is `BadData`, never read as untagged. An untagged entry field
//! (`BalancedInventoryData.bDropOnDeath`) keeps its struct default, which is not read ([`PoolDrop::drop_on_death`]
//! is then `None`).
//!
//! **Off mode answers `NotYet`** for what the records do not give: the global class mod pools that
//! the game fills at run time (decision 2026-11-13-04: the trigger and the class identifier list are open), the
//! uncommon chest multiplier (`GetUncommonChestItemPoolWeightMultiplier`, values not read; no pool in `Startup.upk`
//! sets the flag), the per-player ammo multiplier from playthrough index 2 (below it the record gives 1.0, which is
//! used, UNVERIFIED), the readied roll of `bAutoReadyItems` pools (whether `AddPoolItems` tests the probability is not
//! read), a `KeyedItemPoolDefinition` with no entries of its own (how its `Key` yields items is not read), and a
//! single-precision draw that rounding leaves past every interval (not read). A pool, sub-pool or list
//! that includes itself is `BadData`, and so is a `Quantity` above 10,000 draws.
//!
//! **Draw order.** The native spawns each item inside the pick loop (`InitializeInventory` runs the item's roll before
//! the next pick), so the game interleaves pool draws and item draws. This shape returns the drops first and leaves
//! the item rolls to the caller, so one generator cannot reproduce the native's order through it. The game's
//! sequence is not reproduced in either case (UNVERIFIED; decision 2026-11-15-25).
//!
//! **One generator.** Both modes draw from the roll world's one generator ([`crate::roll::RollWorld`]), asked for
//! as [`CrtStream::LootPool`]: the tools' seeding, not the game's sequence (UNVERIFIED that the game's pool roll uses
//! the C runtime `rand()` directly).
//!
//! **Where values come from.** [`PoolData`] reads one package's tag streams, as the oracle reads one package
//! (`loot_pools.py --package`, `weapon_gen_parity.py` reads `Startup.upk`): [`PoolData::one_file`] keeps that
//! file's exports in export order, which orders the resolver subobjects the tool scans. Reference mode never reads
//! the class default objects (the oracle does not).

use ow_attr::api::InitData;
use ow_core::num::{f32r, pow_f64, round_half_even, trunc_i32};
use ow_core::rand::{CrtStream, Rand15, frand_f32, frand_f64, weighted_draw};
use ow_core::{DetMap, DetSet, NotYet, ObjectPath, ParityMode};
use ow_object::ids::ObjRef;
use std::path::Path;

use crate::ItemError;
use crate::candidates::{PoolCandidate, expand as expand_balance};
use crate::pick::{MIN_WEIGHT, py_sum};
use crate::registry::{CensusInput, Holder, PathHolders, Props, Pv, Registry, TagLookup};
use crate::roll::RollCx;

/// The record a `NotYet` of the pool roll names.
pub const RECORD: &str =
    "docs/verification/NATIVE_LOOT.md (ItemPool.SpawnBalancedInventoryFromPool)";

/// The schema lines the oracle reads with (`weapon_recipe.SCHEMA_LINES` and `loot_pools.SCHEMA_LINES`' own), plus
/// `ItemPoolIncludedLists`, which the oracle has no line for and never reads (its `source_lists` ignores a list's
/// included lists). A schema line types only the array of its own name, so the extra line changes no value the
/// oracle reads; without it the array is undecoded and Off mode could not follow `AddToItemPoolList`.
pub const SCHEMA_LINES: [&str; 17] = [
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
    "ItemPoolIncludedLists=ObjectProperty",
];

/// `loot_pools.NUMBER_OF_PLAYERS`.
pub const NUMBER_OF_PLAYERS: &str = "D_Attributes.GameProperties.NumberOfPlayers";

/// The loot sources the oracle rolls with `roll_crt` (`weapon_gen_manifest.SOURCE_CLASSES`): unqualified class names.
pub const SOURCE_CLASSES: [&str; 2] = ["ItemPoolListDefinition", "AIPawnBalanceDefinition"];

/// The stages the parity run rolls at (`weapon_gen_parity.STAGES`).
pub const PARITY_STAGES: [i32; 6] = [1, 8, 15, 30, 50, 80];

/// The package the parity run reads (`weapon_gen_parity.py`).
pub const PARITY_PACKAGE: &str = "WillowGame/CookedPCConsole/Startup.upk";

/// The global class mod pools are below this path; the game fills them at run time (decision 2026-11-13-04).
pub const CLASS_MOD_POOLS: &str = "GD_Itempools.ClassModPools.";

/// The alias key prefix of a pool class's default object in [`PoolData`] (not a path any package exports).
const CLASS_DEFAULT_KEY: &str = "<class default>.";

/// The classes whose default objects Off mode reads (unqualified names), each with its superclass among them: the
/// class-default chain an untagged field is looked up along. `KeyedItemPoolDefinition`'s superclass is
/// `ItemPoolDefinition` (its class export in `WillowGame.upk`); the chain of the others ends at their own default
/// object.
const DEFAULT_CHAIN: [(&str, Option<&str>); 4] = [
    ("ItemPoolDefinition", None),
    ("KeyedItemPoolDefinition", Some("ItemPoolDefinition")),
    ("ItemPoolListDefinition", None),
    ("AIPawnBalanceDefinition", None),
];

/// What kind of value Off mode expects of a field. A tagged `None` is a null reference for an object; for every other
/// kind it is a value the walker left undecoded (the registry's reading gives `None` for it), which is `BadData`.
/// When no default object along the class chain tags the field, the chain's end gives: no object, `false`, an empty
/// array, and for a struct `NotYet` (its zero value is not read).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Object,
    Flag,
    Array,
    Struct,
}

/// A tagged value of `props` as Off mode reads it (`Kind`); `None` when untagged.
fn tagged<'p>(
    props: &'p Pv,
    key: &str,
    kind: Kind,
    path: &str,
) -> Result<Option<&'p Pv>, ItemError> {
    match props.get(key) {
        None => Ok(None),
        Some(Pv::Null) if kind != Kind::Object => Err(bad(format!(
            "{path}: {key} is tagged but its value was not decoded"
        ))),
        Some(v) => Ok(Some(v)),
    }
}

/// The largest truncated `Quantity` Off mode draws for; above it the roll is refused (`BadData`). The data's
/// quantities are single digits; the bound only keeps bad data from looping (ours, not the game's).
const MAX_OFF_QUANTITY: i32 = 10_000;

/// Which CPython `sum()` the oracle ran on. The two differ in how an int adds into a float total: 3.12 adds it
/// plainly, 3.14 with compensation (measured on 20,000 random mixed lists with each interpreter: 0 misses for its own
/// rule, 380 for the other's). A pool total of int and float weights depends on it, so the comparison names the
/// interpreter it ran on.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PySum {
    /// CPython 3.12 (and 3.13, which the lane did not measure).
    Py312,
    /// CPython 3.14.
    Py314,
}

/// `expand`'s depth limit in the oracle (`depth > 12` is a cycle).
const MAX_TREE_DEPTH: u32 = 12;
/// How deep the reference values follow init data, and the Off mode follows nested pools and lists, before giving
/// up with `BadData`, where Python reaches its recursion limit (the tool raises; ours is a budget, UNVERIFIED as any
/// game limit: the native's list expansion has no cycle guard, NATIVE_LOOT.md).
const MAX_DEPTH: u32 = 200;

fn bad(reason: impl Into<String>) -> ItemError {
    ItemError::BadData(reason.into())
}

fn short(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

/// The last segment of an object path.
fn object_name(path: &str) -> &str {
    path.rsplit(['.', ':']).next().unwrap_or(path)
}

// ---- the package view -------------------------------------------------------------------------------------------

/// One package's exports as the oracle's `weapon_recipe.Package` sees them: classes by path, decoded properties
/// through a [`TagLookup`] with [`SCHEMA_LINES`], and each attribute's resolver subobjects in export order
/// (`Values.subobjects`).
pub struct PoolData {
    lookup: TagLookup,
    exports: Vec<(String, String)>,
    classes: DetMap<String, String>,
    resolvers: DetMap<String, Vec<String>>,
}

impl std::fmt::Debug for PoolData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolData")
            .field("exports", &self.exports.len())
            .finish_non_exhaustive()
    }
}

impl PoolData {
    /// The view over `input`: the order of `input.paths` is the export order (use [`PoolData::one_file`]); a path's
    /// class is its first holder's (the lookup's owner).
    pub fn new(input: CensusInput) -> Result<PoolData, ItemError> {
        let mut exports = Vec::new();
        let mut classes = DetMap::new();
        let mut resolvers: DetMap<String, Vec<String>> = DetMap::new();
        for entry in &input.paths {
            let Some(holder) = entry.holders.first() else {
                continue;
            };
            if entry.path.starts_with(CLASS_DEFAULT_KEY) {
                // A class default object under its alias (PoolData::one_file): not an export of the package.
                continue;
            }
            let class = holder.class.clone();
            if !classes.contains_key(&entry.path) {
                if class.contains("ValueResolver") {
                    let parent = entry.path.rsplit_once('.').map_or("", |(p, _)| p);
                    resolvers
                        .entry(parent.to_string())
                        .or_default()
                        .push(entry.path.clone());
                }
                classes.insert(entry.path.clone(), class.clone());
                exports.push((entry.path.clone(), class));
            }
        }
        let lookup = TagLookup::new(input, &SCHEMA_LINES).map_err(bad)?;
        Ok(PoolData {
            lookup,
            exports,
            classes,
            resolvers,
        })
    }

    /// The census input of one file of a store's census (`file` relative to `root`, as `PARITY_PACKAGE`): its exports
    /// in export order, each with its holder in that file only, as the oracle reads one package; and the default
    /// objects of [`DEFAULT_CHAIN`]'s classes.
    pub fn one_file(census: &ow_defs::identity::Census, root: &Path, file: &str) -> CensusInput {
        Self::from_files(census, root, &[file])
    }

    /// The census input of several files (each relative to `root`), for callers that need pools beyond one package
    /// (W6-LOOT): a path's holders in those files only, its owner the first holder in `files_in` order, paths
    /// ordered by their owner's file in that order, then by export; and the default objects of [`DEFAULT_CHAIN`]'s
    /// classes. With one file this is [`PoolData::one_file`]. Not an oracle reading: the tool reads one package.
    pub fn from_files(
        census: &ow_defs::identity::Census,
        root: &Path,
        files_in: &[&str],
    ) -> CensusInput {
        let rels: Vec<std::path::PathBuf> =
            files_in.iter().map(|f| f.split('/').collect()).collect();
        let rank = |file: &Path| rels.iter().position(|r| file.ends_with(r));
        let mut rows: Vec<((usize, i32), PathHolders)> = Vec::new();
        let mut found: Vec<(usize, std::path::PathBuf)> = Vec::new();
        for entry in &census.entries {
            let mut holders: Vec<(usize, Holder)> = entry
                .holders
                .iter()
                .filter_map(|o| {
                    rank(&o.file)
                        .map(|r| (r, Holder::new(o.file.clone(), o.class.clone(), o.export)))
                })
                .collect();
            // Stable: the holders of one file keep their export order.
            holders.sort_by_key(|(r, _)| *r);
            if let Some((r, first)) = holders.first() {
                if !found.iter().any(|(_, f)| *f == first.file) {
                    found.push((*r, first.file.clone()));
                }
                let key = (*r, first.export);
                let holders = holders.into_iter().map(|(_, h)| h).collect();
                rows.push((key, PathHolders::new(entry.path.clone(), holders)));
            }
        }
        rows.sort_by_key(|(key, _)| *key);
        found.sort_by_key(|(r, _)| *r);
        let mut files: Vec<std::path::PathBuf> = found.into_iter().map(|(_, f)| f).collect();
        let mut input = CensusInput::new(root.to_path_buf());
        input.paths = rows.into_iter().map(|(_, p)| p).collect();
        // The pool classes' default objects (in WillowGame.upk), under an alias key: the registry's census reads
        // no `Default__` path, and Off mode reads an untagged pool field from them.
        for entry in &census.class_defaults {
            let Some(class) = entry.path.strip_prefix("Default__") else {
                continue;
            };
            if !DEFAULT_CHAIN.iter().any(|(c, _)| *c == class) {
                continue;
            }
            if let Some(o) = entry.holders.first() {
                if !files.contains(&o.file) {
                    files.push(o.file.clone());
                }
                input.paths.push(PathHolders::new(
                    format!("{CLASS_DEFAULT_KEY}{class}"),
                    vec![Holder::new(o.file.clone(), o.class.clone(), o.export)],
                ));
            }
        }
        input.files = files;
        input
    }

    /// The class default object of a pool class (unqualified name), when the census held it.
    fn class_default(&self, class: &str) -> Option<Props> {
        self.lookup.get(&format!("{CLASS_DEFAULT_KEY}{class}"))
    }

    /// Off mode's read of an object's field: the tagged value (`Kind`: a tagged value that did not decode is
    /// `BadData`), else the first default object along the class-default chain ([`DEFAULT_CHAIN`]) that tags it,
    /// else the chain end's value for the kind. A class outside the chain, or a chain step whose default object is
    /// not in the scan, is `NotYet`.
    fn field(&self, path: &str, key: &str, kind: Kind) -> Result<Pv, ItemError> {
        let props = self.props(path)?;
        if let Some(v) = tagged(&props, key, kind, path)? {
            return Ok(v.clone());
        }
        let mut class = short(self.class_of(path)).to_string();
        loop {
            let Some((_, parent)) = DEFAULT_CHAIN.iter().find(|(c, _)| *c == class) else {
                return Err(NotYet::not_read(
                    format!(
                        "{path}: an untagged {key} (the class defaults of {class} are not read)"
                    ),
                    RECORD,
                )
                .into());
            };
            let Some(default) = self.class_default(&class) else {
                return Err(NotYet::not_read(
                    format!("{path}: an untagged {key} (no Default__{class} object in the scan)"),
                    RECORD,
                )
                .into());
            };
            if let Some(v) = tagged(&default, key, kind, &format!("Default__{class}"))? {
                return Ok(v.clone());
            }
            match parent {
                Some(p) => class = (*p).to_string(),
                None => break,
            }
        }
        match kind {
            Kind::Object => Ok(Pv::Null),
            Kind::Flag => Ok(Pv::Bool(false)),
            Kind::Array => Ok(Pv::List(Vec::new())),
            Kind::Struct => Err(NotYet::not_read(
                format!(
                    "{path}: {key} is tagged by no default object of its class chain (the struct's zero value is \
                     not read)"
                ),
                RECORD,
            )
            .into()),
        }
    }

    /// Every export `(path, class)` in export order.
    pub fn exports(&self) -> &[(String, String)] {
        &self.exports
    }

    /// The class of `path` (`""` for a path the package does not export).
    pub fn class_of(&self, path: &str) -> &str {
        self.classes.get(path).map_or("", String::as_str)
    }

    /// `package.props(path)`: the tool raises for a path the package does not export or does not decode.
    fn props(&self, path: &str) -> Result<Props, ItemError> {
        if !self.classes.contains_key(path) {
            return Err(bad(format!("{path} is not an export of the pool package")));
        }
        self.lookup
            .props(path)
            .map_err(|e| bad(format!("{path}: {e}")))
    }

    /// The pools and lists the parity run rolls (`weapon_gen_parity.py`): every pool class below `.WeaponPools.` and
    /// every `ItemPoolListDefinition`, sorted.
    pub fn parity_pools(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .exports
            .iter()
            .filter(|(p, c)| {
                (c.contains("ItemPoolDefinition") && p.contains(".WeaponPools."))
                    || short(c) == "ItemPoolListDefinition"
            })
            .map(|(p, _)| p.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

// ---- Python numbers ---------------------------------------------------------------------------------------------

/// A Python number as the tool holds it: a value the C++ printer writes without `.` or an exponent is an `int`
/// after `json.loads`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyNum {
    Int(i64),
    Float(f64),
}

impl PyNum {
    pub fn f(self) -> f64 {
        match self {
            // Data integers are small; the conversion is exact below 2^53, as Python's int-to-float comparison is.
            PyNum::Int(i) => i as f64,
            PyNum::Float(x) => x,
        }
    }

    fn truthy(self) -> bool {
        self.f() != 0.0
    }

    /// A Python number, `None` for `None`; anything else is where the tool's arithmetic raises.
    fn from_pv(v: &Pv) -> Result<Option<PyNum>, ItemError> {
        Ok(Some(match v {
            Pv::Null => return Ok(None),
            Pv::Int(i) => PyNum::Int(*i),
            Pv::Bool(b) => PyNum::Int(i64::from(*b)),
            Pv::Float(x) => PyNum::Float(*x),
            other => return Err(bad(format!("a number expected, got {}", other.to_json()))),
        }))
    }

    fn mul(self, other: PyNum) -> Result<PyNum, ItemError> {
        match (self, other) {
            (PyNum::Int(a), PyNum::Int(b)) => a
                .checked_mul(b)
                .map(PyNum::Int)
                .ok_or_else(|| bad("an integer product out of range")),
            (a, b) => Ok(PyNum::Float(a.f() * b.f())),
        }
    }
}

/// CPython's `sum()` from the integer 0: ints add exactly while every item is an int; the first float is a plain add;
/// then floats add with Neumaier compensation, and ints add plainly under 3.12 (`f_result += (double)value`) or with
/// the same compensation under 3.14 ([`PySum`]); the compensation is added at the end when non-zero and finite.
/// [`py_sum`] is the all-float case (the same under both).
fn py_sum_mixed(items: impl IntoIterator<Item = PyNum>, rule: PySum) -> Result<PyNum, ItemError> {
    let mut items = items.into_iter();
    let mut int_total: i64 = 0;
    let mut sum;
    loop {
        match items.next() {
            None => return Ok(PyNum::Int(int_total)),
            Some(PyNum::Int(i)) => {
                int_total = int_total
                    .checked_add(i)
                    .ok_or_else(|| bad("an integer sum out of range"))?;
            }
            Some(PyNum::Float(x)) => {
                sum = PyNum::Int(int_total).f() + x;
                break;
            }
        }
    }
    let mut c = 0.0f64;
    for item in items {
        match item {
            PyNum::Float(x) => {
                let t = sum + x;
                if sum.abs() >= x.abs() {
                    c += (sum - t) + x;
                } else {
                    c += (x - t) + sum;
                }
                sum = t;
            }
            PyNum::Int(i) if rule == PySum::Py312 => sum += PyNum::Int(i).f(),
            PyNum::Int(i) => {
                let x = PyNum::Int(i).f();
                let t = sum + x;
                if sum.abs() >= x.abs() {
                    c += (sum - t) + x;
                } else {
                    c += (x - t) + sum;
                }
                sum = t;
            }
        }
    }
    if c != 0.0 && c.is_finite() {
        sum += c;
    }
    Ok(PyNum::Float(sum))
}

// ---- the reference values (loot_pools.Values) -------------------------------------------------------------------

const SETS_BASE_VALUE: &str = "BASEVALUE_InitializationDefSetsBaseValue";

/// `loot_pools.Values` (not editor-like): init data to a number, `None` (recorded in `unresolved`) where the tool
/// has no value.
pub struct Values<'d> {
    data: &'d PoolData,
    context: Vec<(String, PyNum)>,
    sum: PySum,
    /// The definitions and attributes that had no value.
    pub unresolved: DetSet<String>,
}

impl<'d> Values<'d> {
    /// `Values(package, {NUMBER_OF_PLAYERS: players})` on the interpreter `sum` names.
    pub fn new(data: &'d PoolData, players: f64, sum: PySum) -> Values<'d> {
        Values {
            data,
            context: vec![(NUMBER_OF_PLAYERS.to_string(), PyNum::Float(players))],
            sum,
            unresolved: DetSet::new(),
        }
    }

    /// `Values.init`.
    pub fn init(&mut self, data: &Pv) -> Result<Option<PyNum>, ItemError> {
        self.init_at(data, 0)
    }

    fn init_at(&mut self, data: &Pv, depth: u32) -> Result<Option<PyNum>, ItemError> {
        if depth > MAX_DEPTH {
            return Err(bad("init data nested too deep"));
        }
        let definition = data.at("InitializationDefinition");
        let attribute = data.at("BaseValueAttribute");
        let base = if definition.truthy() {
            let path = definition
                .as_str()
                .ok_or_else(|| bad("an InitializationDefinition that is not a path"))?;
            self.definition(path, depth + 1)?
        } else if attribute.truthy() {
            let path = attribute
                .as_str()
                .ok_or_else(|| bad("a BaseValueAttribute that is not a path"))?;
            self.attribute(path, depth + 1)?
        } else {
            match PyNum::from_pv(data.at("BaseValueConstant"))? {
                Some(n) if n.truthy() => Some(n),
                _ => Some(PyNum::Float(0.0)),
            }
        };
        let Some(base) = base else {
            return Ok(None);
        };
        let scale = PyNum::from_pv(data.at("BaseValueScaleConstant"))?.unwrap_or(PyNum::Float(1.0));
        base.mul(scale).map(Some)
    }

    fn definition(&mut self, path: &str, depth: u32) -> Result<Option<PyNum>, ItemError> {
        let props = self.data.props(path)?;
        let mode = props.at("BaseValueMode");
        let sets = !mode.truthy() || mode.as_str() == Some(SETS_BASE_VALUE);
        if !sets
            || props
                .at("ConditionalInitialization")
                .at("bEnabled")
                .truthy()
        {
            self.unresolved.insert(path.to_string());
            return Ok(None);
        }
        let formula = props.at("ValueFormula");
        if !formula.at("bEnabled").truthy() {
            self.unresolved.insert(path.to_string());
            return Ok(None);
        }
        let mut terms = [None; 4];
        for (slot, key) in terms
            .iter_mut()
            .zip(["Multiplier", "Level", "Power", "Offset"])
        {
            *slot = self.init_at(formula.at(key), depth + 1)?;
        }
        let [Some(m), Some(l), Some(p), Some(o)] = terms else {
            return Ok(None);
        };
        let mut value = PyNum::Float(formula_value(m.f(), l.f(), p.f(), o.f())?);
        let clamp = props.at("RangeRestriction");
        if clamp.at("bEnableMinValueRestriction").truthy()
            && let Some(low) = self.init_at(clamp.at("MinValue"), depth + 1)?
            && low.f() > value.f()
        {
            // Python's max(value, low) keeps the first of equal values.
            value = low;
        }
        if clamp.at("bEnableMaxValueRestriction").truthy()
            && let Some(high) = self.init_at(clamp.at("MaxValue"), depth + 1)?
            && high.f() < value.f()
        {
            value = high;
        }
        Ok(Some(value))
    }

    /// `Values.attribute`.
    pub fn attribute(&mut self, path: &str, depth: u32) -> Result<Option<PyNum>, ItemError> {
        if depth > MAX_DEPTH {
            return Err(bad("init data nested too deep"));
        }
        if let Some((_, v)) = self.context.iter().find(|(k, _)| k == path) {
            return Ok(Some(*v));
        }
        let data = self.data;
        if data.class_of(path).ends_with("DesignerAttributeDefinition") {
            // The runtime may set a designer attribute; its default is BaseValue (the tool's reading).
            let props = data.props(path)?;
            return self.init_at(props.at("BaseValue"), depth + 1);
        }
        let props = data.props(path)?;
        let chain: Vec<Pv> = if props.at("ValueResolverChain").truthy() {
            props.at("ValueResolverChain").items().to_vec()
        } else {
            data.resolvers
                .get(path)
                .map(|v| v.iter().map(|p| Pv::Str(p.clone())).collect())
                .unwrap_or_default()
        };
        for resolver in &chain {
            let resolver = resolver
                .as_str()
                .ok_or_else(|| bad(format!("{path}: a resolver that is not a path")))?;
            let kind = data.class_of(resolver).to_string();
            let rp = data.props(resolver)?;
            if kind.ends_with(".ConstantAttributeValueResolver") {
                // The tool returns ConstantValue as it is: an untagged one is None, not recorded as unresolved.
                return PyNum::from_pv(rp.at("ConstantValue"));
            }
            if kind.ends_with(".ConditionalAttributeValueResolver")
                && let Some(v) = self.conditional(rp.at("ValueExpressions"), depth + 1)?
            {
                return Ok(Some(v));
            }
        }
        self.unresolved.insert(path.to_string());
        Ok(None)
    }

    fn conditional(&mut self, expressions: &Pv, depth: u32) -> Result<Option<PyNum>, ItemError> {
        for case in expressions.at("ConditionalExpressionList").items() {
            let mut verdicts = Vec::new();
            for e in case.at("Expressions").items() {
                verdicts.push(self.expression(e, depth + 1)?);
            }
            if verdicts.contains(&None) {
                return Ok(None);
            }
            if verdicts.iter().all(|v| *v == Some(true)) {
                return self.init_at(case.at("BaseValueIfTrue"), depth + 1);
            }
        }
        let default = expressions.at("DefaultBaseValue");
        if default.truthy() {
            self.init_at(default, depth + 1)
        } else {
            Ok(None)
        }
    }

    fn expression(&mut self, e: &Pv, depth: u32) -> Result<Option<bool>, ItemError> {
        let operand = |v: &Pv| -> Result<Option<String>, ItemError> {
            if !v.truthy() {
                return Ok(None);
            }
            v.as_str()
                .map(|s| Some(s.to_string()))
                .ok_or_else(|| bad("an attribute operand that is not a path"))
        };
        let left = match operand(e.at("AttributeOperand1"))? {
            Some(a) => self.attribute(&a, depth + 1)?,
            None => None,
        };
        let right = match operand(e.at("AttributeOperand2"))? {
            Some(a) => self.attribute(&a, depth + 1)?,
            None => match PyNum::from_pv(e.at("ConstantOperand2"))? {
                Some(n) if n.truthy() => Some(n),
                _ => Some(PyNum::Float(0.0)),
            },
        };
        let test: Option<fn(f64, f64) -> bool> = match e.at("ComparisonOperator").as_str() {
            Some("OPERATOR_EqualTo") => Some(|a, b| a == b),
            Some("OPERATOR_NotEqualTo") => Some(|a, b| a != b),
            Some("OPERATOR_GreaterThan") => Some(|a, b| a > b),
            Some("OPERATOR_GreaterThanOrEqualTo") => Some(|a, b| a >= b),
            Some("OPERATOR_LessThan") => Some(|a, b| a < b),
            Some("OPERATOR_LessThanOrEqualTo") => Some(|a, b| a <= b),
            _ => None,
        };
        Ok(match (left, right, test) {
            (Some(l), Some(r), Some(t)) => Some(t(l.f(), r.f())),
            _ => None,
        })
    }
}

/// `weapon_recipe.formula_value`: `Multiplier * (Level ^ Power + Offset)` with each step rounded to single, the power
/// skipped when it is 1. Where Python raises (zero to a negative power, a negative base to a fractional power, an
/// overflow) this is `BadData`.
fn formula_value(multiplier: f64, level: f64, power: f64, offset: f64) -> Result<f64, ItemError> {
    let raised = if power == 1.0 {
        level
    } else {
        if level == 0.0 && power < 0.0 {
            return Err(bad("zero to a negative power in a ValueFormula"));
        }
        if level < 0.0 && power.fract() != 0.0 {
            return Err(bad(
                "a negative level to a fractional power in a ValueFormula",
            ));
        }
        let r = pow_f64(level, power);
        if r.is_infinite() && level.is_finite() && power.is_finite() {
            return Err(bad("a ValueFormula power that overflows"));
        }
        f32r(r)
    };
    Ok(f32r(multiplier * f32r(raised + offset)))
}

// ---- the reference tree (loot_pools.expand) ---------------------------------------------------------------------

/// What a pool entry names.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Pool,
    Item,
}

/// One entry of an expanded pool (`expand`'s children).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: EntryKind,
    pub path: String,
    pub weight: Option<PyNum>,
    /// `False` only for a sub-pool that is not eligible.
    pub eligible: bool,
    /// The share among the eligible entries, 0 for the others.
    pub share: f64,
    pub pool: Option<Box<Tree>>,
}

/// An expanded pool (`expand`'s result).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub path: String,
    /// The tool's cycle marker (a pool already above, or deeper than 12): no entries, eligibility unset.
    pub cycle: bool,
    pub quantity: Option<PyNum>,
    /// The stage gate: `None` when a requirement has no value.
    pub gate: Option<bool>,
    /// `None` for a cycle (the tool's missing key, read as eligible by the parent).
    pub eligible: Option<bool>,
    pub entries: Vec<Node>,
}

/// `loot_pools.pool_gate`.
fn pool_gate(values: &mut Values, pool: &str, stage: i32) -> Result<Option<bool>, ItemError> {
    let props = values.data.props(pool)?;
    let s = f64::from(stage);
    for (key, at_least) in [
        ("MinGameStageRequirement", true),
        ("MaxGameStageRequirement", false),
    ] {
        let v = props.at(key);
        if !v.truthy() {
            continue;
        }
        let path = v
            .as_str()
            .ok_or_else(|| bad(format!("{pool}: {key} is not a path")))?;
        let Some(bound) = values.attribute(path, 0)? else {
            return Ok(None);
        };
        let holds = if at_least {
            s >= bound.f()
        } else {
            s <= bound.f()
        };
        if !holds {
            return Ok(Some(false));
        }
    }
    Ok(Some(true))
}

/// `loot_pools.expand` at `stage`.
pub fn expand(values: &mut Values, pool: &str, stage: i32) -> Result<Tree, ItemError> {
    expand_at(values, pool, stage, 0, &mut Vec::new())
}

fn expand_at(
    values: &mut Values,
    pool: &str,
    stage: i32,
    depth: u32,
    seen: &mut Vec<String>,
) -> Result<Tree, ItemError> {
    if seen.iter().any(|p| p == pool) || depth > MAX_TREE_DEPTH {
        return Ok(Tree {
            path: pool.to_string(),
            cycle: true,
            quantity: None,
            gate: None,
            eligible: None,
            entries: Vec::new(),
        });
    }
    let gate = pool_gate(values, pool, stage)?;
    let props = values.data.props(pool)?;
    let quantity = match props.at("Quantity") {
        q if q.truthy() => values.init(q)?,
        _ => Some(PyNum::Float(1.0)),
    };
    // pool_entries: every weight first, in data order.
    let mut children = Vec::new();
    for entry in props.at("BalancedItems").items() {
        let sub = entry.at("ItmPoolDefinition");
        let child = if sub.truthy() {
            sub
        } else {
            entry.at("InvBalanceDefinition")
        };
        if !child.truthy() {
            continue;
        }
        let path = child
            .as_str()
            .ok_or_else(|| bad(format!("{pool}: an entry that is not a path")))?
            .to_string();
        let weight = values.init(entry.at("Probability"))?;
        let kind = if sub.truthy() {
            EntryKind::Pool
        } else {
            EntryKind::Item
        };
        children.push(Node {
            kind,
            path,
            weight,
            eligible: true,
            share: 0.0,
            pool: None,
        });
    }
    seen.push(pool.to_string());
    for node in &mut children {
        if node.kind == EntryKind::Pool {
            let tree = expand_at(values, &node.path, stage, depth + 1, seen)?;
            if tree.eligible == Some(false) {
                node.eligible = false;
            }
            node.pool = Some(Box::new(tree));
        }
    }
    seen.pop();
    let counted = |c: &Node| c.eligible && c.weight.is_some_and(|w| w.truthy());
    let total = py_sum_mixed(
        children
            .iter()
            .filter(|c| counted(c))
            .filter_map(|c| c.weight),
        values.sum,
    )?;
    for c in &mut children {
        if total.f() > 0.0 && counted(c) {
            // Python's true division: exact operands below 2^53, so one correctly rounded quotient.
            c.share = c.weight.map_or(0.0, |w| w.f()) / total.f();
        }
    }
    let eligible = gate != Some(false)
        && children
            .iter()
            .any(|c| c.kind == EntryKind::Item || c.eligible);
    Ok(Tree {
        path: pool.to_string(),
        cycle: false,
        quantity,
        gate,
        eligible: Some(eligible),
        entries: children,
    })
}

/// One listed pool of a source (`loot_pools.table`'s pools).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub pool: String,
    pub probability: Option<PyNum>,
    pub tree: Tree,
}

/// `loot_pools.source_lists`: `(pool, probability init)` pairs in data order.
fn source_lists(data: &PoolData, source: &str, depth: u32) -> Result<Vec<(Pv, Pv)>, ItemError> {
    if depth > MAX_DEPTH {
        return Err(bad(format!("{source}: pool lists nested too deep")));
    }
    let class = data.class_of(source).to_string();
    let props = data.props(source)?;
    let pairs_of = |list: &Pv| -> Vec<(Pv, Pv)> {
        list.items()
            .iter()
            .map(|e| (e.at("ItemPool").clone(), e.at("PoolProbability").clone()))
            .collect()
    };
    let listed = |v: &Pv| -> Result<String, ItemError> {
        v.as_str()
            .map(str::to_string)
            .ok_or_else(|| bad(format!("{source}: an included list that is not a path")))
    };
    if class.ends_with("ItemPoolListDefinition") {
        return Ok(pairs_of(props.at("ItemPools")));
    }
    if class.ends_with("AIPawnBalanceDefinition") {
        let mut pairs = pairs_of(props.at("DefaultItemPoolList"));
        for l in props.at("DefaultItemPoolIncludedLists").items() {
            pairs.extend(source_lists(data, &listed(l)?, depth + 1)?);
        }
        let first = props
            .at("PlayThroughs")
            .items()
            .first()
            .cloned()
            .unwrap_or(Pv::Null);
        pairs.extend(pairs_of(first.at("CustomItemPoolList")));
        for l in first.at("CustomItemPoolIncludedLists").items() {
            pairs.extend(source_lists(data, &listed(l)?, depth + 1)?);
        }
        return Ok(pairs);
    }
    if class.contains("ItemPoolDefinition") {
        let one = Pv::Map(DetMap::from([(
            "BaseValueConstant".to_string(),
            Pv::Float(1.0),
        )]));
        return Ok(vec![(Pv::Str(source.to_string()), one)]);
    }
    Err(bad(format!(
        "{source}: unsupported loot source class {class}"
    )))
}

/// `loot_pools.table`'s pools: each listed pool with its probability and tree.
pub fn table(values: &mut Values, source: &str, stage: i32) -> Result<Vec<Listed>, ItemError> {
    let mut out = Vec::new();
    for (pool, probability) in source_lists(values.data, source, 0)? {
        if !pool.truthy() {
            continue;
        }
        let pool = pool
            .as_str()
            .ok_or_else(|| bad(format!("{source}: a listed pool that is not a path")))?
            .to_string();
        let probability = values.init(&probability)?;
        let tree = expand(values, &pool, stage)?;
        out.push(Listed {
            pool,
            probability,
            tree,
        });
    }
    Ok(out)
}

// ---- drops ------------------------------------------------------------------------------------------------------

/// One balance a pool roll drops.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PoolDrop {
    /// The balance (`InvBalanceDefinition`).
    pub balance: String,
    /// The pools above the drop, outermost first (the oracle's `chain`).
    pub chain: Vec<String>,
    /// Off mode: the pool-level candidate the draw picked, for [`crate::roll::roll_candidate`]. `None` in
    /// Reference mode (the oracle draws no manufacturer).
    pub candidate: Option<PoolCandidate>,
    /// Off mode: the entry's `bDropOnDeath` AND those of the entries above it; `None` when an untagged flag
    /// decides it (the struct default of `BalancedInventoryData.bDropOnDeath` is not read) and in Reference mode.
    pub drop_on_death: Option<bool>,
}

impl PoolDrop {
    fn reference(balance: &str, chain: Vec<String>) -> PoolDrop {
        PoolDrop {
            balance: balance.to_string(),
            chain,
            candidate: None,
            drop_on_death: None,
        }
    }
}

/// `loot_pools.crt_choice`: one draw among entries with a share.
fn crt_choice<'t>(entries: &[&'t Node], rng: &mut dyn Rand15) -> &'t Node {
    let total = py_sum(entries.iter().map(|e| e.share));
    let r = weighted_draw(rng, total);
    let mut running = 0.0f64;
    for e in entries {
        if running <= r && r <= running + e.share {
            return e;
        }
        running += e.share;
    }
    // r == total after float rounding: the last entry (entries is never empty here).
    entries[entries.len() - 1]
}

/// `max(0, int(round(quantity or 1)))`.
fn reference_quantity(q: Option<PyNum>) -> Result<u32, ItemError> {
    let n = match q {
        Some(PyNum::Int(i)) if i != 0 => i as f64,
        Some(PyNum::Float(x)) if x != 0.0 => {
            if !x.is_finite() {
                return Err(bad("a pool Quantity that is not finite"));
            }
            round_half_even(x)
        }
        _ => 1.0,
    };
    if n <= 0.0 {
        return Ok(0);
    }
    // Range-checked: a whole number in 1..=u32::MAX converts exactly; beyond it the tool would loop for hours.
    if n > f64::from(u32::MAX) {
        return Err(bad("a pool Quantity out of range"));
    }
    Ok(n as u32)
}

/// `loot_pools.roll_pool_crt`: one firing of an expanded pool.
pub fn roll_tree(tree: &Tree, rng: &mut dyn Rand15) -> Result<Vec<PoolDrop>, ItemError> {
    let mut drops = Vec::new();
    roll_tree_into(tree, rng, &[], &mut drops)?;
    Ok(drops)
}

fn roll_tree_into(
    tree: &Tree,
    rng: &mut dyn Rand15,
    chain: &[String],
    drops: &mut Vec<PoolDrop>,
) -> Result<(), ItemError> {
    let eligible: Vec<&Node> = tree.entries.iter().filter(|e| e.share > 0.0).collect();
    if eligible.is_empty() || tree.eligible == Some(false) {
        return Ok(());
    }
    let mut here = chain.to_vec();
    here.push(tree.path.clone());
    for _ in 0..reference_quantity(tree.quantity)? {
        let entry = crt_choice(&eligible, rng);
        match (&entry.kind, &entry.pool) {
            (EntryKind::Item, _) => drops.push(PoolDrop::reference(&entry.path, here.clone())),
            (EntryKind::Pool, Some(sub)) => roll_tree_into(sub, rng, &here, drops)?,
            (EntryKind::Pool, None) => return Err(bad("a sub-pool entry without its tree")),
        }
    }
    Ok(())
}

/// `loot_pools.roll_crt`: a source's listed pools, each fired with its probability.
pub fn roll_table(listed: &[Listed], rng: &mut dyn Rand15) -> Result<Vec<PoolDrop>, ItemError> {
    let mut drops = Vec::new();
    for l in listed {
        let Some(chance) = l.probability else {
            continue;
        };
        if l.tree.eligible == Some(false) {
            continue;
        }
        let draw = frand_f64(rng);
        if chance.f() >= 1.0 || draw < chance.f() {
            roll_tree_into(&l.tree, rng, &[], &mut drops)?;
        }
    }
    Ok(drops)
}

/// `weapon_gen_parity.pool_drops`: a source (an `ItemPoolListDefinition` or `AIPawnBalanceDefinition`) through
/// [`table`] and [`roll_table`], anything else through [`expand`] and [`roll_tree`].
pub fn reference_drops(
    values: &mut Values,
    source: &str,
    stage: i32,
    rng: &mut dyn Rand15,
) -> Result<Vec<PoolDrop>, ItemError> {
    if SOURCE_CLASSES.contains(&short(values.data.class_of(source))) {
        let listed = table(values, source, stage)?;
        return roll_table(&listed, rng);
    }
    let tree = expand(values, source, stage)?;
    roll_tree(&tree, rng)
}

// ---- the request ------------------------------------------------------------------------------------------------

/// Which listed pools of a source a roll fires.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listing {
    /// Every listed pool: the oracle's reading (Reference mode only).
    Every,
    /// The death roll (`WillowPawn.DropLootOnDeath`): pools whose `bAutoReadyItems` is false (Off mode).
    OnDeath,
    /// The readied roll (`AddPoolItems`): pools whose `bAutoReadyItems` is true. Off mode answers `NotYet`.
    Readied,
}

/// What a caller passes besides the pool and the roll context.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PoolRequest {
    pub listing: Listing,
    /// The caller's `GameStageVarianceFormula` as init data (an interactive object's; enemy drops pass none). Off
    /// mode only.
    pub variance: Option<Pv>,
    /// The interpreter whose `sum()` the oracle ran on (Reference mode only; default [`PySum::Py314`]).
    pub python_sum: PySum,
}

impl PoolRequest {
    /// The request each mode serves by default: every listed pool in Reference mode, the death roll in Off mode.
    pub fn for_mode(mode: ParityMode) -> PoolRequest {
        PoolRequest {
            listing: if mode == ParityMode::Reference {
                Listing::Every
            } else {
                Listing::OnDeath
            },
            variance: None,
            python_sum: PySum::Py314,
        }
    }

    pub fn with_python_sum(mut self, rule: PySum) -> Self {
        self.python_sum = rule;
        self
    }

    pub fn with_variance(mut self, variance: Pv) -> Self {
        self.variance = Some(variance);
        self
    }

    pub fn with_listing(mut self, listing: Listing) -> Self {
        self.listing = listing;
        self
    }
}

/// The pool roll (module documentation): `source` is a pool, an `ItemPoolListDefinition` or an
/// `AIPawnBalanceDefinition`, rolled at the snapshot's stage from the roll world's one generator. Reference mode
/// needs no registry; Off mode expands balances from `reg`'s records and refuses a request it does not serve.
pub fn roll_pool(
    pools: &PoolData,
    reg: Option<&Registry>,
    source: &ObjectPath,
    request: &PoolRequest,
    cx: &mut RollCx,
) -> Result<Vec<PoolDrop>, ItemError> {
    let stage = cx.balance.stage.0;
    match cx.mode {
        ParityMode::Reference => {
            if request.listing != Listing::Every || request.variance.is_some() {
                return Err(ItemError::WrongMode {
                    call: "roll_pool with a death or readied listing or a variance formula",
                    mode: cx.mode,
                });
            }
            let players = f64::from(cx.balance.players.get());
            let mut values = Values::new(pools, players, request.python_sum);
            let rng = cx.eval.world.rng(CrtStream::LootPool)?;
            reference_drops(&mut values, source.as_str(), stage, rng)
        }
        _ => {
            if request.listing == Listing::Every {
                return Err(ItemError::WrongMode {
                    call: "roll_pool with every listed pool (the oracle's reading)",
                    mode: cx.mode,
                });
            }
            let Some(reg) = reg else {
                return Err(NotYet::layer_absent(
                    "the Off-mode pool roll without the item registry",
                    "ow-items registry",
                    RECORD,
                )
                .into());
            };
            let snap = cx.balance;
            let cap =
                ow_balance::cap::item_stage_cap(snap.playthrough, snap.op, snap.level_cap_packs)?.0;
            let mut off = Off {
                pools,
                reg,
                cx,
                cap,
            };
            off.source(source.as_str(), stage, request)
        }
    }
}

// ---- Off mode (NATIVE_LOOT.md) -----------------------------------------------------------------------------------

/// One Off-mode candidate of a pool's list.
#[derive(Debug, Clone)]
enum Candidate {
    Pool {
        path: String,
        drop: Option<bool>,
    },
    Balance {
        balance: String,
        candidate: PoolCandidate,
        drop: Option<bool>,
    },
}

impl Candidate {
    fn same(&self, other: &Candidate) -> bool {
        match (self, other) {
            (Candidate::Pool { path: a, .. }, Candidate::Pool { path: b, .. }) => a == b,
            (
                Candidate::Balance {
                    balance: a,
                    candidate: x,
                    ..
                },
                Candidate::Balance {
                    balance: b,
                    candidate: y,
                    ..
                },
            ) => {
                a == b
                    && x.manufacturer == y.manufacturer
                    && x.definition == y.definition
                    && x.grade_or_level == y.grade_or_level
                    && x.stage == y.stage
            }
            _ => false,
        }
    }
}

/// `a AND b` of two flags, `None` when an unknown flag decides it.
fn and_flags(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    match (a, b) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        _ => None,
    }
}

/// A tagged bool, `None` when untagged.
fn flag(v: &Pv) -> Option<bool> {
    match v {
        Pv::Null => None,
        other => Some(other.truthy()),
    }
}

struct Off<'r, 'c, 'a> {
    pools: &'r PoolData,
    reg: &'r Registry,
    cx: &'c mut RollCx<'a>,
    cap: i32,
}

/// An entry's tagged value of `kind`, `Null` when untagged.
fn entry_value(entry: &Pv, key: &str, kind: Kind, pool: &str) -> Result<Pv, ItemError> {
    Ok(tagged(entry, key, kind, pool)?.cloned().unwrap_or(Pv::Null))
}

/// An entry's object path, `None` when untagged, null or empty.
fn entry_path(entry: &Pv, key: &str, pool: &str) -> Result<Option<String>, ItemError> {
    match tagged(entry, key, Kind::Object, pool)? {
        None | Some(Pv::Null) => Ok(None),
        Some(v) => match v.as_str() {
            Some("") => Ok(None),
            Some(p) => Ok(Some(p.to_string())),
            None => Err(bad(format!("{pool}: {key} is not a path"))),
        },
    }
}

/// `seen` holding `path` is a cycle: `BadData` naming it.
fn refuse_cycle(seen: &[String], path: &str, what: &str) -> Result<(), ItemError> {
    if seen.iter().any(|p| p == path) {
        return Err(bad(format!(
            "{path}: {what} include themselves (a cycle through {})",
            seen.join(" -> ")
        )));
    }
    if seen.len() > MAX_DEPTH as usize {
        return Err(bad(format!("{path}: {what} nested too deep")));
    }
    Ok(())
}

impl Off<'_, '_, '_> {
    fn find(&mut self, path: &str) -> Result<ObjRef, ItemError> {
        if let Some(o) = self.cx.known.get(path) {
            return Ok(*o);
        }
        let parsed = ObjectPath::parse(path).map_err(|e| bad(format!("{path}: {e}")))?;
        let parts = self.cx.eval.world.parts();
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
            None => Err(bad(format!("{path}: no package holds it"))),
        }
    }

    fn find_opt(&mut self, v: &Pv) -> Result<Option<ObjRef>, ItemError> {
        match v.as_str() {
            Some(p) if !p.is_empty() => self.find(p).map(Some),
            _ => Ok(None),
        }
    }

    /// An `AttributeInitializationData` through the evaluator with no context source. An untagged
    /// `BaseValueScaleConstant` is the struct default 1 and an untagged constant 0 (as W4-ROLL reads them).
    fn init(&mut self, init: &Pv) -> Result<f32, ItemError> {
        let number = |v: &Pv, default: f32| -> Result<f32, ItemError> {
            match v {
                Pv::Null => Ok(default),
                other => other
                    .number()
                    // Single precision is the field's type; a tagged float or int converts by rounding.
                    .map(|x| x as f32)
                    .ok_or_else(|| bad(format!("a number expected, got {}", other.to_json()))),
            }
        };
        let data = InitData {
            base_value_constant: number(init.at("BaseValueConstant"), 0.0)?,
            base_value_attribute: self.find_opt(init.at("BaseValueAttribute"))?,
            initialization_definition: self.find_opt(init.at("InitializationDefinition"))?,
            base_value_scale_constant: number(init.at("BaseValueScaleConstant"), 1.0)?,
        };
        let sys = self.cx.sys;
        Ok(sys.evaluate_init(&data, None, None, &mut self.cx.eval)?)
    }

    /// An attribute's value through the evaluator, truncated; an unset requirement is 0.
    fn requirement(&mut self, v: &Pv) -> Result<i32, ItemError> {
        let Some(attr) = self.find_opt(v)? else {
            return Ok(0);
        };
        let data = InitData {
            base_value_constant: 0.0,
            base_value_attribute: Some(attr),
            initialization_definition: None,
            base_value_scale_constant: 1.0,
        };
        let sys = self.cx.sys;
        let value = sys.evaluate_init(&data, None, None, &mut self.cx.eval)?;
        Ok(trunc_i32(value))
    }

    /// `(min, max)` of a pool's stage requirements.
    fn window(&mut self, pool: &str) -> Result<(i32, i32), ItemError> {
        let min = self
            .pools
            .field(pool, "MinGameStageRequirement", Kind::Object)?;
        let max = self
            .pools
            .field(pool, "MaxGameStageRequirement", Kind::Object)?;
        let min = self.requirement(&min)?;
        let max = self.requirement(&max)?;
        Ok((min, max))
    }

    /// A pool's `BalancedItems`. A `KeyedItemPoolDefinition` with none is `NotYet`: on the installed game all 83
    /// tag only `Key` and `bAutoReadyItems`, and how a key yields items is not read.
    fn entries(&self, pool: &str) -> Result<Vec<Pv>, ItemError> {
        let entries = self
            .pools
            .field(pool, "BalancedItems", Kind::Array)?
            .items()
            .to_vec();
        if entries.is_empty() && short(self.pools.class_of(pool)) == "KeyedItemPoolDefinition" {
            return Err(NotYet::not_read(
                format!(
                    "{pool}: a keyed pool with no entries of its own (how its Key yields items)"
                ),
                RECORD,
            )
            .into());
        }
        Ok(entries)
    }

    /// Whether `pool` has entries of its own; a global class mod pool, empty in the packages and filled at run time,
    /// is `NotYet`.
    fn has_entries(&self, pool: &str) -> Result<bool, ItemError> {
        let has = !self.entries(pool)?.is_empty();
        if !has && pool.starts_with(CLASS_MOD_POOLS) {
            return Err(NotYet::not_read(
                format!(
                    "{pool}: the run-time fill of the class mod pools (decision 2026-11-13-04; what triggers it and \
                     the class identifier list are open)"
                ),
                RECORD,
            )
            .into());
        }
        Ok(has)
    }

    /// The pool gate (window and a reachable entry) at `stage`; `seen` is the path of pools being gated above, and a
    /// pool already on it is a cycle (`BadData`).
    fn gate(&mut self, pool: &str, stage: i32, seen: &mut Vec<String>) -> Result<bool, ItemError> {
        refuse_cycle(seen, pool, "nested pools")?;
        let (min, max) = self.window(pool)?;
        let open = (min == 0 || (min <= stage && min <= self.cap)) && (max == 0 || stage <= max);
        if !open {
            return Ok(false);
        }
        seen.push(pool.to_string());
        let mut reachable = false;
        for entry in self.entries(pool)? {
            if let Some(sub) = entry_path(&entry, "ItmPoolDefinition", pool)?
                && self.has_entries(&sub)?
                && self.gate(&sub, stage, seen)?
            {
                reachable = true;
                break;
            }
            if entry_path(&entry, "InvBalanceDefinition", pool)?.is_some() {
                reachable = true;
                break;
            }
        }
        seen.pop();
        Ok(reachable)
    }

    /// The balance entry's stage with the caller's variance (module documentation).
    fn entry_stage(
        &mut self,
        pool: &str,
        stage: i32,
        variance: Option<&Pv>,
        window: (i32, i32),
    ) -> Result<i32, ItemError> {
        let Some(variance) = variance else {
            return Ok(stage);
        };
        // bSupportsGameStageVariance: class default true (NATIVE_LOOT.md; the default object holds it).
        if flag(
            &self
                .pools
                .field(pool, "bSupportsGameStageVariance", Kind::Flag)?,
        ) == Some(false)
        {
            return Ok(stage);
        }
        let v = trunc_i32(self.init(variance)?);
        let mut s = stage
            .checked_add(v)
            .ok_or_else(|| bad("a stage plus its variance out of range"))?;
        s = s.max(if stage > 0 { 1 } else { 0 });
        let (min, max) = window;
        if min > 0 {
            s = s.max(min);
        }
        if max > 0 {
            s = s.min(max);
        }
        Ok(s)
    }

    /// The pool's candidate list (module documentation), with weights. `seen` is the spawn path above the pool,
    /// the pool itself included.
    fn candidates(
        &mut self,
        pool: &str,
        stage: i32,
        variance: Option<&Pv>,
        seen: &mut Vec<String>,
    ) -> Result<Vec<(Candidate, f32)>, ItemError> {
        let window = self.window(pool)?;
        let uncommon = self
            .pools
            .field(pool, "bEligibleForUncommonWeightMultiplier", Kind::Flag)?
            .truthy();
        let mut list: Vec<(Candidate, f32)> = Vec::new();
        for entry in self.entries(pool)? {
            let drop = flag(&entry_value(&entry, "bDropOnDeath", Kind::Flag, pool)?);
            let probability = entry_value(&entry, "Probability", Kind::Struct, pool)?;
            if let Some(sub) = entry_path(&entry, "ItmPoolDefinition", pool)?
                && self.has_entries(&sub)?
            {
                let def = probability
                    .at("InitializationDefinition")
                    .as_str()
                    .unwrap_or("");
                if uncommon && object_name(def) == "Weight_2_Uncommon" {
                    return Err(NotYet::not_read(
                        format!(
                            "{pool}: the uncommon chest multiplier (GetUncommonChestItemPoolWeightMultiplier's values)"
                        ),
                        RECORD,
                    )
                    .into());
                }
                let mut w = self.init(&probability)?;
                if !self.gate(&sub, stage, seen)? {
                    w = 0.0;
                }
                add_candidate(&mut list, Candidate::Pool { path: sub, drop }, w);
                continue;
            }
            let Some(balance) = entry_path(&entry, "InvBalanceDefinition", pool)? else {
                continue;
            };
            let s = self.entry_stage(pool, stage, variance, window)?;
            let w = self.init(&probability)?;
            let Some(record) = self.reg.balance(&balance) else {
                return Err(bad(format!(
                    "{pool}: {balance} is not a balance the registry holds"
                )));
            };
            let record = record.clone();
            let data = self.cx.data;
            let cap = self.cap;
            let found = {
                let me = &mut *self;
                expand_balance(data, &record, s, cap, &mut |v: &Pv| me.value(v))?
            };
            // At most u16-many candidates in practice; the count converts to single exactly below 2^24.
            let n = found.len() as f32;
            for c in found {
                let weight = c.weight * w / n;
                add_candidate(
                    &mut list,
                    Candidate::Balance {
                        balance: balance.clone(),
                        candidate: c,
                        drop,
                    },
                    weight,
                );
            }
        }
        Ok(list)
    }

    /// A registry value (a number, `{"init": ...}`, or none) through the evaluator.
    fn value(&mut self, v: &Pv) -> Result<Option<f64>, ItemError> {
        match v {
            Pv::Null => Ok(None),
            Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => Ok(v.number()),
            other => match other.get("init") {
                Some(init) => {
                    let init = init.clone();
                    self.init(&init).map(|x| Some(f64::from(x)))
                }
                None => Err(bad(format!("a registry value {}", other.to_json()))),
            },
        }
    }

    /// One draw over the list in single precision: the first closed interval holding `r`.
    fn draw(&mut self, list: &[(Candidate, f32)]) -> Result<usize, ItemError> {
        let rng = self.cx.eval.world.rng(CrtStream::LootPool)?;
        single_draw(list.iter().map(|(_, w)| *w), rng.rand15())
    }

    /// `SpawnBalancedInventoryFromPool` on `pool`. `chain` is the spawn path above it; a pool already on it is a
    /// cycle (`BadData`).
    fn pool(
        &mut self,
        pool: &str,
        stage: i32,
        variance: Option<&Pv>,
        may_drop: Option<bool>,
        chain: &[String],
        drops: &mut Vec<PoolDrop>,
    ) -> Result<(), ItemError> {
        refuse_cycle(chain, pool, "pools")?;
        // NATIVE_LOOT.md step 1: Quantity is evaluated first, before the gate, so a random quantity draws even for
        // a pool the gate then closes.
        let quantity = self.pools.field(pool, "Quantity", Kind::Struct)?;
        let quantity = trunc_i32(self.init(&quantity)?);
        if quantity > MAX_OFF_QUANTITY {
            return Err(bad(format!(
                "{pool}: a Quantity of {quantity} (above the {MAX_OFF_QUANTITY} draws this port allows)"
            )));
        }
        if !self.gate(pool, stage, &mut chain.to_vec())? {
            return Ok(());
        }
        let mut here = chain.to_vec();
        here.push(pool.to_string());
        let list = self.candidates(pool, stage, variance, &mut here.clone())?;
        for _ in 0..quantity.max(0) {
            if list.is_empty() {
                continue;
            }
            let i = self.draw(&list)?;
            match &list[i].0 {
                Candidate::Pool { path, drop } => {
                    let flag = and_flags(*drop, may_drop);
                    self.pool(path, stage, variance, flag, &here, drops)?;
                }
                Candidate::Balance {
                    balance,
                    candidate,
                    drop,
                } => drops.push(PoolDrop {
                    balance: balance.clone(),
                    chain: here.clone(),
                    candidate: Some(candidate.clone()),
                    drop_on_death: and_flags(*drop, may_drop),
                }),
            }
        }
        Ok(())
    }

    /// `AddToItemPoolList`: included lists first (depth first), then the list's own pools. `seen` is the path of
    /// lists above; a list already on it is a cycle (`BadData`; the native has no guard, NATIVE_LOOT.md).
    fn add_list(
        &self,
        list: &str,
        seen: &mut Vec<String>,
        out: &mut Vec<(String, Pv)>,
    ) -> Result<(), ItemError> {
        refuse_cycle(seen, list, "pool lists")?;
        seen.push(list.to_string());
        for included in self
            .pools
            .field(list, "ItemPoolIncludedLists", Kind::Array)?
            .items()
        {
            let path = included
                .as_str()
                .ok_or_else(|| bad(format!("{list}: an included list that is not a path")))?;
            self.add_list(path, seen, out)?;
        }
        seen.pop();
        push_pools(
            &self.pools.field(list, "ItemPools", Kind::Array)?,
            list,
            out,
        )
    }

    /// The listed pools of a source, with their probability init data.
    fn listed(&self, source: &str) -> Result<Vec<(String, Pv)>, ItemError> {
        let class = short(self.pools.class_of(source)).to_string();
        let mut out = Vec::new();
        match class.as_str() {
            "ItemPoolListDefinition" => self.add_list(source, &mut Vec::new(), &mut out)?,
            "AIPawnBalanceDefinition" => {
                let entries = self.pools.field(source, "PlayThroughs", Kind::Array)?;
                let entries = entries.items();
                if entries.is_empty() {
                    return Ok(out);
                }
                // GetPlayThroughIndex: the player's playthrough, at most the last entry.
                let index = usize::from(self.cx.balance.playthrough.index()).min(entries.len() - 1);
                let entry = &entries[index];
                // A playthrough entry's custom list replaces the default list when it is non-empty.
                let pick = |custom: &str, default: &str| -> Result<Pv, ItemError> {
                    let c = entry_value(entry, custom, Kind::Array, source)?;
                    if c.items().is_empty() {
                        self.pools.field(source, default, Kind::Array)
                    } else {
                        Ok(c)
                    }
                };
                let included = pick(
                    "CustomItemPoolIncludedLists",
                    "DefaultItemPoolIncludedLists",
                )?;
                for l in included.items() {
                    let path = l.as_str().ok_or_else(|| {
                        bad(format!("{source}: an included list that is not a path"))
                    })?;
                    self.add_list(path, &mut vec![source.to_string()], &mut out)?;
                }
                push_pools(
                    &pick("CustomItemPoolList", "DefaultItemPoolList")?,
                    source,
                    &mut out,
                )?;
            }
            _ => {}
        }
        Ok(out)
    }

    /// `GetAmmoDropsPerPlayerMultiplier` (NATIVE_LOOT.md): 1.0 below playthrough index 2 (UNVERIFIED, read from
    /// native code); from index 2 the `BalanceModifiers` entry's value, which is not read (`NotYet`).
    fn ammo_multiplier(&self, pool: &str) -> Result<f32, ItemError> {
        if self.cx.balance.playthrough.index() < 2 {
            return Ok(1.0);
        }
        Err(NotYet::not_read(
            format!(
                "{pool}: the per-player ammo multiplier from playthrough index 2 (GetAmmoDropsPerPlayerMultiplier's \
                 BalanceModifiers values)"
            ),
            RECORD,
        )
        .into())
    }

    fn source(
        &mut self,
        source: &str,
        stage: i32,
        request: &PoolRequest,
    ) -> Result<Vec<PoolDrop>, ItemError> {
        let mut drops = Vec::new();
        let variance = request.variance.as_ref();
        let class = self.pools.class_of(source).to_string();
        if !SOURCE_CLASSES.contains(&short(&class)) {
            if !class.contains("ItemPoolDefinition") {
                return Err(bad(format!(
                    "{source}: unsupported loot source class {class}"
                )));
            }
            self.pool(source, stage, variance, Some(true), &[], &mut drops)?;
            return Ok(drops);
        }
        if request.listing == Listing::Readied {
            return Err(NotYet::not_read(
                "the readied roll of bAutoReadyItems pools (whether AddPoolItems tests the pool probability)",
                RECORD,
            )
            .into());
        }
        for (pool, probability) in self.listed(source)? {
            // bAutoReadyItems: class default true; the death roll keeps the pools that set it false.
            if flag(&self.pools.field(&pool, "bAutoReadyItems", Kind::Flag)?) != Some(false) {
                continue;
            }
            let def = probability
                .at("InitializationDefinition")
                .as_str()
                .unwrap_or("");
            // The multiplier first: from playthrough index 2 it is NotYet before anything is evaluated.
            let multiplier = if object_name(def) == "AmmoDrops_PerPlayer" {
                self.ammo_multiplier(&pool)?
            } else {
                1.0
            };
            let chance = self.init(&probability)? * multiplier;
            if chance <= 0.0 {
                continue;
            }
            let rng = self.cx.eval.world.rng(CrtStream::LootPool)?;
            if frand_f32(rng) <= chance {
                self.pool(&pool, stage, variance, Some(true), &[], &mut drops)?;
            }
        }
        Ok(drops)
    }
}

/// Adds a candidate to a pool's list: one at or below the 1e-8 floor is left out, and one identical to an earlier
/// candidate replaces that candidate's weight in place.
fn add_candidate(list: &mut Vec<(Candidate, f32)>, c: Candidate, w: f32) {
    if f64::from(w) <= MIN_WEIGHT {
        return;
    }
    match list.iter_mut().find(|(d, _)| d.same(&c)) {
        Some(slot) => slot.1 = w,
        None => list.push((c, w)),
    }
}

/// The single-precision pool draw: `r = total * rand / 32767`, the first closed interval holding `r`; `NotYet` when
/// rounding leaves `r` past every interval (what the native does then is not read).
fn single_draw(weights: impl Iterator<Item = f32> + Clone, rand: u16) -> Result<usize, ItemError> {
    let total = weights.clone().fold(0.0f32, |t, w| t + w);
    let r = total * f32::from(rand) / 32_767.0;
    let mut running = 0.0f32;
    for (i, w) in weights.enumerate() {
        if running <= r && r <= running + w {
            return Ok(i);
        }
        running += w;
    }
    Err(NotYet::not_read(
        "a single-precision pool draw past every interval (what the native does then)",
        RECORD,
    )
    .into())
}

/// The `ItemPoolInfo` entries of a list with a pool, in order (Off mode: an undecoded value is `BadData`).
fn push_pools(list: &Pv, owner: &str, out: &mut Vec<(String, Pv)>) -> Result<(), ItemError> {
    for e in list.items() {
        if let Some(p) = entry_path(e, "ItemPool", owner)? {
            out.push((p, entry_value(e, "PoolProbability", Kind::Struct, owner)?));
        }
    }
    Ok(())
}

/// A source's drops as the parity line `{"pool":...,"stage":...,"seed":...,"drops":[...]}` (the C++ batch header's
/// shape, `weapon_gen_parity.py`): the balance paths in drop order.
pub fn parity_line(source: &str, stage: i32, seed: u32, drops: &[PoolDrop]) -> String {
    let tree = Pv::Map(DetMap::from([
        ("pool".to_string(), Pv::Str(source.to_string())),
        ("stage".to_string(), Pv::Int(i64::from(stage))),
        ("seed".to_string(), Pv::Int(i64::from(seed))),
        (
            "drops".to_string(),
            Pv::List(drops.iter().map(|d| Pv::Str(d.balance.clone())).collect()),
        ),
    ]));
    tree.to_json()
}

/// An Off-mode drop as JSON: balance, chain, the candidate's manufacturer, definition, stage, level and weight, and
/// the drop flag (`null` when not known).
pub fn off_drop_json(d: &PoolDrop) -> Pv {
    let mut m = DetMap::new();
    m.insert("balance".to_string(), Pv::Str(d.balance.clone()));
    m.insert(
        "chain".to_string(),
        Pv::List(d.chain.iter().map(|p| Pv::Str(p.clone())).collect()),
    );
    m.insert(
        "drop_on_death".to_string(),
        d.drop_on_death.map_or(Pv::Null, Pv::Bool),
    );
    if let Some(c) = &d.candidate {
        m.insert("manufacturer".to_string(), Pv::Str(c.manufacturer.clone()));
        m.insert("definition".to_string(), Pv::Str(c.definition.clone()));
        m.insert("stage".to_string(), Pv::Int(i64::from(c.stage)));
        m.insert("level".to_string(), Pv::Int(i64::from(c.level)));
        m.insert(
            "grade_or_level".to_string(),
            Pv::Int(i64::from(c.grade_or_level)),
        );
        m.insert("weight".to_string(), Pv::Float(f64::from(c.weight)));
    }
    Pv::Map(m)
}

#[cfg(test)]
mod tests {
    //! Synthetic pools (AGENTS.md rule 9): W4-ROLL's invented package (its balances, for the Off-mode expansion)
    //! plus invented pools, lists, a pawn and attributes. Every name and number is invented; the Reference cases
    //! mirror `tests/loot_pools_test.py`'s, so the same draws give the same drops.

    use super::*;
    use crate::merge::RollData;
    use crate::registry::build_from_census;
    use crate::roll::RollWorld;
    use crate::roll::tests::{GRENADE, SHIELD, Syn, TestWorld, V, konst, package, r, snapshot};
    use ow_attr::AttributeSystem;
    use ow_attr::api::EvalCx;
    use ow_core::notyet::Missing;
    use ow_core::rand::CrtRand;
    use ow_package::package::Package;
    use std::path::PathBuf;

    const FILE: &str = "WillowGame/CookedPCConsole/Startup.upk";
    const IP: &str = "WillowGame.ItemPoolDefinition";
    const GUN: &str = "WillowGame.WeaponBalanceDefinition";
    const ATTR: &str = "Engine.AttributeDefinition";
    const CONSTANT: &str = "GearboxFramework.ConstantAttributeValueResolver";

    fn init(fields: Vec<(&'static str, V)>) -> V {
        V::Struct("AttributeInitializationData", fields)
    }

    /// A constant base and an attribute.
    fn by_attr(c: f32, attribute: &'static str) -> V {
        init(vec![
            ("BaseValueConstant", V::Float(c)),
            ("BaseValueAttribute", r(attribute)),
            ("BaseValueScaleConstant", V::Float(1.0)),
        ])
    }

    fn by_def(definition: &'static str) -> V {
        init(vec![
            ("InitializationDefinition", r(definition)),
            ("BaseValueScaleConstant", V::Float(1.0)),
        ])
    }

    fn formula(m: V, l: V, p: V, o: Option<V>) -> V {
        let mut fields = vec![
            ("bEnabled", V::Bool(true)),
            ("Multiplier", m),
            ("Level", l),
            ("Power", p),
        ];
        if let Some(o) = o {
            fields.push(("Offset", o));
        }
        V::Struct("AttributeInitializationValueFormula", fields)
    }

    fn entry(pool: Option<&'static str>, balance: Option<&'static str>, w: V) -> V {
        let mut fields = vec![];
        if let Some(p) = pool {
            fields.push(("ItmPoolDefinition", r(p)));
        }
        if let Some(b) = balance {
            fields.push(("InvBalanceDefinition", r(b)));
        }
        fields.push(("Probability", w));
        V::Struct("BalancedInventoryData", fields)
    }

    fn off_entry(balance: &'static str, w: f32, drop: Option<bool>) -> V {
        let mut fields = vec![
            ("InvBalanceDefinition", r(balance)),
            ("Probability", konst(w)),
        ];
        if let Some(d) = drop {
            fields.push(("bDropOnDeath", V::Bool(d)));
        }
        V::Struct("BalancedInventoryData", fields)
    }

    fn items(entries: Vec<V>) -> (&'static str, V) {
        ("BalancedItems", V::Array(entries))
    }

    fn listed(pool: &'static str, p: V) -> V {
        V::Struct(
            "ItemPoolInfo",
            vec![("ItemPool", r(pool)), ("PoolProbability", p)],
        )
    }

    fn condition(value: f32, op: &'static str) -> V {
        V::Struct(
            "ConditionalExpression",
            vec![
                ("BaseValueIfTrue", konst(value)),
                (
                    "Expressions",
                    V::Array(vec![V::Struct(
                        "AttributeExpressionData",
                        vec![
                            ("AttributeOperand1", r(NUMBER_OF_PLAYERS)),
                            ("ComparisonOperator", V::Enum("EComparisonOperator", op)),
                            ("ConstantOperand2", V::Float(1.0)),
                        ],
                    )]),
                ),
            ],
        )
    }

    fn reference_pools(p: &mut Syn) {
        let fdef = "Engine.AttributeInitializationDefinition";
        p.add(
            "W.Common",
            fdef,
            vec![(
                "ValueFormula",
                formula(konst(100.0), konst(1.0), konst(1.0), None),
            )],
        );
        p.add(
            "W.Rare",
            fdef,
            vec![
                (
                    "ValueFormula",
                    formula(konst(1.0), konst(1.0), konst(1.0), None),
                ),
                // Cooked data omits false flags; a disabled restriction must not clamp.
                (
                    "RangeRestriction",
                    V::Struct("Restriction", vec![("MinValue", konst(100.0))]),
                ),
            ],
        );
        p.add(
            "W.Mod",
            fdef,
            vec![(
                "ValueFormula",
                formula(
                    by_attr(0.0, "D.CommonMod"),
                    by_def("W.Common"),
                    konst(1.0),
                    None,
                ),
            )],
        );
        p.add(
            "D.CommonMod",
            "WillowGame.DesignerAttributeDefinition",
            vec![("BaseValue", konst(1.0))],
        );
        // 2 x (3^2 + 1) = 20; a max restriction of 7 clamps it.
        p.add(
            "W.Offset",
            fdef,
            vec![(
                "ValueFormula",
                formula(konst(2.0), konst(3.0), konst(2.0), Some(konst(1.0))),
            )],
        );
        p.add(
            "W.Capped",
            fdef,
            vec![
                (
                    "ValueFormula",
                    formula(konst(2.0), konst(3.0), konst(2.0), Some(konst(1.0))),
                ),
                (
                    "RangeRestriction",
                    V::Struct(
                        "Restriction",
                        vec![
                            ("bEnableMaxValueRestriction", V::Bool(true)),
                            ("MaxValue", konst(7.0)),
                        ],
                    ),
                ),
            ],
        );
        p.add("Odds", ATTR, vec![]);
        p.add(
            "Odds.Cond_0",
            "GearboxFramework.ConditionalAttributeValueResolver",
            vec![(
                "ValueExpressions",
                V::Struct(
                    "ConditionalExpressionList",
                    vec![(
                        "ConditionalExpressionList",
                        V::Array(vec![
                            condition(0.5, "OPERATOR_EqualTo"),
                            condition(0.25, "OPERATOR_GreaterThan"),
                        ]),
                    )],
                ),
            )],
        );
        p.add("Stage2", ATTR, vec![]);
        p.add(
            "Stage2.Const_0",
            CONSTANT,
            vec![("ConstantValue", V::Float(2.0))],
        );
        p.add(
            "Stage9",
            ATTR,
            vec![("ValueResolverChain", V::Array(vec![r("Stage9_Const")]))],
        );
        p.add(
            "Stage9_Const",
            CONSTANT,
            vec![("ConstantValue", V::Float(9.0))],
        );
        p.add("Unset", ATTR, vec![]);
        p.add("Unset.Const_0", CONSTANT, vec![]);
        p.add("Unknown.Attr", ATTR, vec![]);
        for gun in ["Gun.A", "Gun.B", "Gun.R", "Gun.L"] {
            p.add(gun, GUN, vec![]);
        }
        p.add(
            "Pool.WeaponPools.Guns",
            IP,
            vec![items(vec![
                entry(Some("Pool.Common"), None, by_def("W.Mod")),
                entry(Some("Pool.Rare"), None, by_def("W.Rare")),
                entry(Some("Pool.Late"), None, by_def("W.Rare")),
            ])],
        );
        p.add(
            "Pool.Common",
            IP,
            vec![
                ("MinGameStageRequirement", r("Stage2")),
                items(vec![
                    entry(None, Some("Gun.A"), konst(3.0)),
                    entry(None, Some("Gun.B"), konst(1.0)),
                ]),
            ],
        );
        p.add(
            "Pool.Rare",
            IP,
            vec![
                ("Quantity", konst(2.0)),
                items(vec![entry(None, Some("Gun.R"), konst(1.0))]),
            ],
        );
        p.add(
            "Pool.Late",
            IP,
            vec![
                ("MinGameStageRequirement", r("Stage9")),
                items(vec![entry(None, Some("Gun.L"), konst(1.0))]),
            ],
        );
        p.add(
            "Pool.Wrap",
            IP,
            vec![items(vec![entry(Some("Pool.Late"), None, konst(1.0))])],
        );
        p.add("Pool.Empty", IP, vec![]);
        p.add(
            "Pool.Outer",
            IP,
            vec![items(vec![
                entry(Some("Pool.Wrap"), None, konst(3.0)),
                entry(Some("Pool.Empty"), None, konst(5.0)),
                entry(Some("Pool.Rare"), None, konst(1.0)),
            ])],
        );
        for (pool, q) in [
            ("Pool.Half", 2.5),
            ("Pool.OddHalf", 3.5),
            ("Pool.Zero", 0.0),
            ("Pool.Negative", -2.0),
        ] {
            p.add(
                pool,
                IP,
                vec![
                    ("Quantity", konst(q)),
                    items(vec![entry(None, Some("Gun.A"), konst(1.0))]),
                ],
            );
        }
        p.add(
            "Pool.CycA",
            IP,
            vec![items(vec![entry(Some("Pool.CycB"), None, konst(1.0))])],
        );
        p.add(
            "Pool.CycB",
            IP,
            vec![items(vec![entry(Some("Pool.CycA"), None, konst(1.0))])],
        );
        p.add(
            "List",
            "WillowGame.ItemPoolListDefinition",
            vec![(
                "ItemPools",
                V::Array(vec![
                    listed("Pool.WeaponPools.Guns", by_attr(0.0, "Odds")),
                    listed("Pool.Rare", by_attr(0.0, "Unknown.Attr")),
                ]),
            )],
        );
        p.add(
            "Pawn",
            "WillowGame.AIPawnBalanceDefinition",
            vec![
                ("DefaultItemPoolIncludedLists", V::Array(vec![r("List")])),
                (
                    "PlayThroughs",
                    V::Array(vec![V::Struct(
                        "PlayThroughData",
                        vec![(
                            "CustomItemPoolList",
                            V::Array(vec![listed("Pool.Late", konst(1.0))]),
                        )],
                    )]),
                ),
            ],
        );
    }

    /// Off mode: the roll fixture's balances in pools, a gate attribute, a class mod pool left empty.
    fn off_pools(p: &mut Syn) {
        p.add(
            "Off.Gear",
            IP,
            vec![
                ("Quantity", konst(3.0)),
                items(vec![
                    off_entry(GRENADE, 2.0, Some(true)),
                    off_entry(SHIELD, 1.0, Some(false)),
                ]),
            ],
        );
        p.add(
            "Off.Untagged",
            IP,
            vec![items(vec![off_entry(SHIELD, 1.0, None)])],
        );
        p.add("Gate60", ATTR, vec![]);
        p.add(
            "Off.Gated",
            IP,
            vec![
                ("Quantity", konst(1.0)),
                ("MinGameStageRequirement", r("Gate60")),
                items(vec![off_entry(SHIELD, 1.0, Some(true))]),
            ],
        );
        p.add(
            "Off.Outer",
            IP,
            vec![
                ("Quantity", konst(2.0)),
                items(vec![
                    V::Struct(
                        "BalancedInventoryData",
                        vec![
                            ("ItmPoolDefinition", r("Off.Gear")),
                            ("Probability", konst(1.0)),
                            ("bDropOnDeath", V::Bool(true)),
                        ],
                    ),
                    V::Struct(
                        "BalancedInventoryData",
                        vec![
                            ("ItmPoolDefinition", r("Off.Gated")),
                            ("Probability", konst(5.0)),
                        ],
                    ),
                ]),
            ],
        );
        p.add("GD_Itempools.ClassModPools.Pool_Test", IP, vec![]);
        p.add(
            "Off.ClassMods",
            IP,
            vec![
                ("Quantity", konst(1.0)),
                items(vec![entry(
                    Some("GD_Itempools.ClassModPools.Pool_Test"),
                    None,
                    konst(1.0),
                )]),
            ],
        );
        p.add(
            "Off.List",
            "WillowGame.ItemPoolListDefinition",
            vec![(
                "ItemPools",
                V::Array(vec![
                    listed("Off.Gear", konst(1.0)),
                    listed("Off.Death", konst(1.0)),
                ]),
            )],
        );
        p.add(
            "Off.Death",
            IP,
            vec![
                ("Quantity", konst(1.0)),
                ("bAutoReadyItems", V::Bool(false)),
                items(vec![off_entry(SHIELD, 1.0, Some(true))]),
            ],
        );
        // Stand in for the class default objects (the alias keys below point at them): the pool default tags what the
        // installed game's tags (Quantity 1, the two flags true); the others tag nothing.
        p.add(
            "Off.Default",
            IP,
            vec![
                ("Quantity", konst(1.0)),
                ("bAutoReadyItems", V::Bool(true)),
                ("bSupportsGameStageVariance", V::Bool(true)),
            ],
        );
        p.add("Off.KeyedDefault", KEYED, vec![]);
        p.add("Off.ListDefault", LIST_CLASS, vec![]);
        p.add("Off.PawnDefault", PAWN, vec![]);
        off_more(p);
    }

    struct Install(PathBuf);

    impl Drop for Install {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct Fx {
        _install: Install,
        pools: PoolData,
        input: CensusInput,
    }

    impl Fx {
        fn new(tag: &str) -> Fx {
            let dir =
                std::env::temp_dir().join(format!("ow-items-pools-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let path = dir.join(FILE);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut p = package();
            reference_pools(&mut p);
            off_pools(&mut p);
            std::fs::write(&path, p.build()).unwrap();
            let dir = std::path::absolute(&dir).unwrap();
            let mut input = CensusInput::new(dir.clone());
            let pkg = Package::load(&dir.join(FILE)).unwrap();
            input.files = vec![pkg.source.clone()];
            for i in 1..=pkg.exports.len() as i32 {
                let path = pkg.path_of(i).unwrap();
                let class = pkg.path_of(pkg.object(i).unwrap().class).unwrap();
                input.paths.push(PathHolders::new(
                    path,
                    vec![Holder::new(pkg.source.clone(), class, i)],
                ));
            }
            let mut with_default = input.clone();
            for (stand_in, class) in [
                ("Off.Default", "ItemPoolDefinition"),
                ("Off.KeyedDefault", "KeyedItemPoolDefinition"),
                ("Off.ListDefault", "ItemPoolListDefinition"),
                ("Off.PawnDefault", "AIPawnBalanceDefinition"),
            ] {
                let default = input
                    .paths
                    .iter()
                    .find(|e| e.path == stand_in)
                    .unwrap()
                    .clone();
                with_default.paths.push(PathHolders::new(
                    format!("{CLASS_DEFAULT_KEY}{class}"),
                    default.holders,
                ));
            }
            Fx {
                pools: PoolData::new(with_default).unwrap(),
                input,
                _install: Install(dir),
            }
        }

        fn values(&self, players: f64) -> Values<'_> {
            Values::new(&self.pools, players, PySum::Py314)
        }
    }

    fn items_of(drops: &[PoolDrop]) -> Vec<&str> {
        drops.iter().map(|d| d.balance.as_str()).collect()
    }

    fn num(v: Option<PyNum>) -> Option<f64> {
        v.map(PyNum::f)
    }

    fn pv_init_def(path: &str) -> Pv {
        Pv::Map(DetMap::from([
            (
                "InitializationDefinition".to_string(),
                Pv::Str(path.to_string()),
            ),
            ("BaseValueScaleConstant".to_string(), Pv::Float(1.0)),
        ]))
    }

    #[test]
    fn reference_values_read_as_the_tool_reads_them() {
        let fx = Fx::new("values");
        let mut v = fx.values(1.0);
        // MinValue 100 is not enabled; the designer BaseValue 1 times 100; the offset inside the multiplier.
        assert_eq!(num(v.init(&pv_init_def("W.Rare")).unwrap()), Some(1.0));
        assert_eq!(num(v.init(&pv_init_def("W.Mod")).unwrap()), Some(100.0));
        assert_eq!(num(v.init(&pv_init_def("W.Offset")).unwrap()), Some(20.0));
        assert_eq!(num(v.init(&pv_init_def("W.Capped")).unwrap()), Some(7.0));
        assert_eq!(num(v.attribute("Odds", 0).unwrap()), Some(0.5));
        // A resolver subobject, and a resolver chain.
        assert_eq!(num(v.attribute("Stage2", 0).unwrap()), Some(2.0));
        assert_eq!(num(v.attribute("Stage9", 0).unwrap()), Some(9.0));
        assert_eq!(
            num(fx.values(3.0).attribute("Odds", 0).unwrap()),
            Some(0.25)
        );
        // An unknown attribute is unresolved, never guessed; an untagged ConstantValue is None without a record.
        assert_eq!(v.attribute("Unknown.Attr", 0).unwrap(), None);
        assert_eq!(v.attribute("Unset", 0).unwrap(), None);
        assert!(v.unresolved.contains("Unknown.Attr"));
        assert!(!v.unresolved.contains("Unset"));
        // A path the package does not export is where the tool raises.
        assert!(matches!(
            v.attribute("Not.There", 0),
            Err(ItemError::BadData(_))
        ));
    }

    #[test]
    fn python_sum_adds_ints_into_a_float_total_without_compensation() {
        let big = 1e16;
        let mixed = py_sum_mixed(
            [PyNum::Float(big), PyNum::Int(1), PyNum::Int(1)],
            PySum::Py312,
        )
        .unwrap();
        assert_eq!(mixed, PyNum::Float(big));
        assert_eq!(py_sum([big, 1.0, 1.0]), big + 2.0);
        assert_eq!(
            py_sum_mixed([PyNum::Int(3), PyNum::Int(1)], PySum::Py314).unwrap(),
            PyNum::Int(4)
        );
        assert_eq!(py_sum_mixed([], PySum::Py314).unwrap(), PyNum::Int(0));
        assert!(py_sum_mixed([PyNum::Int(i64::MAX), PyNum::Int(1)], PySum::Py312).is_err());
    }

    #[test]
    fn expand_shares_gates_and_eligibility() {
        let fx = Fx::new("expand");
        let mut v = fx.values(1.0);
        let tree = expand(&mut v, "Pool.WeaponPools.Guns", 5).unwrap();
        let shares: Vec<(&str, f64)> = tree
            .entries
            .iter()
            .map(|e| (e.path.as_str(), e.share))
            .collect();
        // Late is gated off (stage 5 < 9); Common 100 vs Rare 1.
        assert_eq!(
            shares,
            [
                ("Pool.Common", 100.0 / 101.0),
                ("Pool.Rare", 1.0 / 101.0),
                ("Pool.Late", 0.0)
            ]
        );
        let late = expand(&mut v, "Pool.Late", 5).unwrap();
        assert_eq!((late.gate, late.eligible), (Some(false), Some(false)));
        // Wrap holds only the gated Late; Empty holds nothing.
        let outer = expand(&mut v, "Pool.Outer", 5).unwrap();
        let shares: Vec<f64> = outer.entries.iter().map(|e| e.share).collect();
        assert_eq!(shares, [0.0, 0.0, 1.0]);
        let wrap = expand(&mut v, "Pool.Wrap", 5).unwrap();
        assert_eq!((wrap.gate, wrap.eligible), (Some(true), Some(false)));
        let later = expand(&mut v, "Pool.Outer", 9).unwrap();
        let shares: Vec<f64> = later.entries.iter().map(|e| e.share).collect();
        assert_eq!(shares, [0.75, 0.0, 0.25]);
        // A cycle is cut where it closes, as the tool cuts it.
        let cyc = expand(&mut v, "Pool.CycA", 5).unwrap();
        let inner = cyc.entries[0].pool.as_ref().unwrap();
        assert!(inner.entries[0].pool.as_ref().unwrap().cycle);
        assert_eq!(roll_tree(&cyc, &mut CrtRand::new(1)).unwrap(), vec![]);
    }

    #[test]
    fn the_crt_pool_roll_walks_shares() {
        let fx = Fx::new("walk");
        let mut v = fx.values(1.0);
        // Gun.A 0.75, Gun.B 0.25; srand(1) draws 41: r = 0.00125 -> Gun.A.
        let tree = expand(&mut v, "Pool.Common", 5).unwrap();
        assert_eq!(
            items_of(&roll_tree(&tree, &mut CrtRand::new(1)).unwrap()),
            ["Gun.A"]
        );
        let mut rng = CrtRand::new(1);
        for _ in 0..3 {
            rng.rand15();
        }
        // The next draw is 26500: r = 0.8088 -> Gun.B.
        assert_eq!(items_of(&roll_tree(&tree, &mut rng).unwrap()), ["Gun.B"]);
        let rare = expand(&mut v, "Pool.Rare", 1).unwrap();
        let drops = roll_tree(&rare, &mut CrtRand::new(9)).unwrap();
        assert_eq!(items_of(&drops), ["Gun.R", "Gun.R"]);
        assert_eq!(drops[0].chain, ["Pool.Rare"]);
        // A gated top pool spawns nothing and draws nothing.
        let late = expand(&mut v, "Pool.Late", 5).unwrap();
        let mut rng = CrtRand::new(1);
        assert_eq!(roll_tree(&late, &mut rng).unwrap(), vec![]);
        assert_eq!(rng.rand15(), 41);
    }

    #[test]
    fn the_quantity_rounds_half_to_even_and_zero_counts_as_one() {
        let fx = Fx::new("quantity");
        let mut v = fx.values(1.0);
        for (pool, n) in [
            ("Pool.Half", 2),
            ("Pool.OddHalf", 4),
            ("Pool.Zero", 1),
            ("Pool.Negative", 0),
        ] {
            let tree = expand(&mut v, pool, 5).unwrap();
            assert_eq!(
                roll_tree(&tree, &mut CrtRand::new(3)).unwrap().len(),
                n,
                "{pool}"
            );
        }
    }

    #[test]
    fn the_crt_source_roll_fires_listed_pools() {
        let fx = Fx::new("source");
        let mut v = fx.values(1.0);
        let listed = table(&mut v, "Pawn", 10).unwrap();
        let names: Vec<&str> = listed.iter().map(|l| l.pool.as_str()).collect();
        assert_eq!(names, ["Pool.WeaponPools.Guns", "Pool.Rare", "Pool.Late"]);
        let chances: Vec<Option<f64>> = listed.iter().map(|l| num(l.probability)).collect();
        assert_eq!(chances, [Some(0.5), None, Some(1.0)]);
        // srand(1) draws 41 (0.00125 < 0.5, fires; one draw inside); Rare is unresolved (no draw); Late always fires.
        let drops = roll_table(&listed, &mut CrtRand::new(1)).unwrap();
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[1].balance, "Gun.L");
        // srand(6000) starts with 19632: 0.599 >= 0.5 skips Guns.
        assert_eq!(
            items_of(&roll_table(&listed, &mut CrtRand::new(6000)).unwrap()),
            ["Gun.L"]
        );
        // weapon_gen_parity.pool_drops: a list goes through the table.
        let mut v = fx.values(1.0);
        let list = reference_drops(&mut v, "List", 10, &mut CrtRand::new(1)).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            fx.pools.parity_pools(),
            [
                "List",
                "Off.AmmoList",
                "Off.List",
                "Off.ListDefault",
                "Off.ListOuter",
                "Off.ListSelf",
                "Pool.WeaponPools.Guns"
            ]
        );
        assert_eq!(
            parity_line("List", 10, 1, &list),
            "{\"drops\":[\"Gun.A\"],\"pool\":\"List\",\"seed\":1,\"stage\":10}"
        );
    }

    // ---- Off mode ----

    struct OffFx {
        fx: Fx,
        reg: Registry,
        data: RollData,
        sys: AttributeSystem,
    }

    impl OffFx {
        fn new(tag: &str) -> OffFx {
            let fx = Fx::new(tag);
            OffFx {
                reg: build_from_census(fx.input.clone(), "Game"),
                data: RollData::new(&fx.input).unwrap(),
                sys: AttributeSystem::standard().unwrap(),
                fx,
            }
        }

        fn roll(
            &self,
            source: &str,
            stage: i32,
            seed: u32,
            mode: ParityMode,
            request: &PoolRequest,
        ) -> (Result<Vec<PoolDrop>, ItemError>, CrtRand) {
            let mut world = TestWorld::new();
            let gate = world.constant_attribute(60.0);
            let snap = snapshot(stage);
            let mut rng = CrtRand::new(seed);
            let result = {
                let mut rw = RollWorld::new(&mut world, &mut rng, false);
                let mut cx = RollCx::new(&snap, EvalCx::new(&mut rw), &self.sys, &self.data, mode)
                    .with_known("Gate60", gate);
                roll_pool(
                    &self.fx.pools,
                    Some(&self.reg),
                    &ObjectPath::parse(source).unwrap(),
                    request,
                    &mut cx,
                )
            };
            (result, rng)
        }

        fn off(&self, source: &str, stage: i32, seed: u32) -> Result<Vec<PoolDrop>, ItemError> {
            self.roll(
                source,
                stage,
                seed,
                ParityMode::Off,
                &PoolRequest::for_mode(ParityMode::Off),
            )
            .0
        }
    }

    #[test]
    fn off_mode_expands_balances_at_the_pool_level() {
        let fx = OffFx::new("off-expand");
        // Quantity 3, with replacement.
        let drops = fx.off("Off.Gear", 8, 7).unwrap();
        assert_eq!(drops.len(), 3);
        for d in &drops {
            let c = d.candidate.as_ref().unwrap();
            assert_eq!(d.chain, ["Off.Gear"]);
            assert_eq!((c.stage, c.level), (8, 8));
            if d.balance == GRENADE {
                assert_eq!(d.drop_on_death, Some(true));
            } else {
                assert_eq!(d.balance, SHIELD);
                assert_eq!(d.drop_on_death, Some(false));
            }
        }
        // The stage is capped (playthrough 1: 50) before the grade windows are tested.
        let high = fx.off("Off.Gear", 80, 7).unwrap();
        assert!(
            high.iter()
                .all(|d| d.candidate.as_ref().unwrap().stage == 50)
        );
        // The candidate list: the grenade's two makers share its entry weight 2, the shield's one maker weighs 1.
        let mut world = TestWorld::new();
        let snap = snapshot(8);
        let mut rng = CrtRand::new(1);
        let mut rw = RollWorld::new(&mut world, &mut rng, false);
        let mut cx = RollCx::new(
            &snap,
            EvalCx::new(&mut rw),
            &fx.sys,
            &fx.data,
            ParityMode::Off,
        );
        let mut off = Off {
            pools: &fx.fx.pools,
            reg: &fx.reg,
            cx: &mut cx,
            cap: 50,
        };
        let list = off
            .candidates("Off.Gear", 8, None, &mut vec!["Off.Gear".to_string()])
            .unwrap();
        let weights: Vec<f32> = list.iter().map(|(_, w)| *w).collect();
        let bolt = 0.5f32 + 7.0f32 / 10.0f32 * 0.5f32;
        assert_eq!(weights, [1.0 * 2.0 / 2.0, bolt * 2.0 / 2.0, 1.0]);
    }

    #[test]
    fn off_mode_gates_by_the_cap_and_draws_nothing_when_closed() {
        let fx = OffFx::new("off-gate");
        // Off.Gated needs stage 60 and the cap is 50: never open, so it weighs 0 in Outer and Outer always picks
        // Gear; the drop flag is Gear's entry AND Outer's.
        for seed in 1..20 {
            let drops = fx.off("Off.Outer", 70, seed).unwrap();
            assert_eq!(drops.len(), 6, "{seed}");
            assert!(drops.iter().all(|d| d.chain == ["Off.Outer", "Off.Gear"]));
        }
        let (gated, mut rng) = fx.roll(
            "Off.Gated",
            70,
            1,
            ParityMode::Off,
            &PoolRequest::for_mode(ParityMode::Off),
        );
        assert_eq!(gated.unwrap(), vec![]);
        assert_eq!(rng.rand15(), 41);
    }

    #[test]
    fn off_mode_reads_an_untagged_field_from_the_class_default() {
        let fx = OffFx::new("off-default");
        // Off.Untagged has no Quantity: the default object (here the fixture's Off.Default) gives 1.
        let drops = fx.off("Off.Untagged", 8, 3).unwrap();
        assert_eq!(drops.len(), 1);
        // An untagged entry flag: its struct default is not read.
        assert_eq!(drops[0].drop_on_death, None);
        // Without a default object, the field is NotYet.
        let bare = PoolData::new(fx.fx.input.clone()).unwrap();
        assert!(matches!(
            bare.field("Off.Untagged", "Quantity", Kind::Struct),
            Err(ItemError::NotYet(_))
        ));
        assert_eq!(
            bare.field("Off.Gear", "Quantity", Kind::Struct).unwrap(),
            fx.fx
                .pools
                .field("Off.Gear", "Quantity", Kind::Struct)
                .unwrap()
        );
    }

    #[test]
    fn off_mode_answers_not_yet_where_the_records_stop() {
        let fx = OffFx::new("off-notyet");
        let not_read = |r: Result<Vec<PoolDrop>, ItemError>| matches!(r, Err(ItemError::NotYet(n)) if n.missing == Missing::NotRead);
        assert!(not_read(fx.off("Off.ClassMods", 8, 1)));
        let readied = PoolRequest::for_mode(ParityMode::Off).with_listing(Listing::Readied);
        assert!(not_read(
            fx.roll("Off.List", 8, 1, ParityMode::Off, &readied).0
        ));
        // The death roll keeps only the pool whose bAutoReadyItems is false.
        let death = fx.off("Off.List", 8, 1).unwrap();
        assert_eq!(death.len(), 1);
        assert_eq!(death[0].chain, ["Off.Death"]);
        // Each mode refuses the other's request.
        let every = PoolRequest::for_mode(ParityMode::Reference);
        assert!(matches!(
            fx.roll("Off.Gear", 8, 1, ParityMode::Off, &every).0,
            Err(ItemError::WrongMode { .. })
        ));
        let varied = every.clone().with_variance(Pv::Map(DetMap::new()));
        assert!(matches!(
            fx.roll("Pool.Rare", 8, 1, ParityMode::Reference, &varied).0,
            Err(ItemError::WrongMode { .. })
        ));
        // Reference mode through roll_pool is the oracle's roll.
        let (reference, _) = fx.roll("Pool.Rare", 8, 9, ParityMode::Reference, &every);
        assert_eq!(items_of(&reference.unwrap()), ["Gun.R", "Gun.R"]);
    }

    #[test]
    fn off_mode_applies_the_callers_variance_within_the_pool_window() {
        let fx = OffFx::new("off-variance");
        let variance = |v: f32| {
            Pv::Map(DetMap::from([
                ("BaseValueConstant".to_string(), Pv::Float(f64::from(v))),
                ("BaseValueScaleConstant".to_string(), Pv::Float(1.0)),
            ]))
        };
        let request = |v: f32| PoolRequest::for_mode(ParityMode::Off).with_variance(variance(v));
        // Stage 8 minus 20: at least 1 (the caller's stage is above 0).
        let low = fx
            .roll("Off.Gear", 8, 2, ParityMode::Off, &request(-20.0))
            .0
            .unwrap();
        assert!(low.iter().all(|d| d.candidate.as_ref().unwrap().stage == 1));
        // Stage 8 plus 2.9: the variance is truncated to 2.
        let up = fx
            .roll("Off.Gear", 8, 2, ParityMode::Off, &request(2.9))
            .0
            .unwrap();
        assert!(up.iter().all(|d| d.candidate.as_ref().unwrap().stage == 10));
    }

    // ---- review r1: more Off-mode cases ----

    const KEYED: &str = "WillowGame.KeyedItemPoolDefinition";
    const LIST_CLASS: &str = "WillowGame.ItemPoolListDefinition";
    const PAWN: &str = "WillowGame.AIPawnBalanceDefinition";

    fn death_pool(p: &mut Syn, path: &'static str, balance: &'static str) {
        p.add(
            path,
            IP,
            vec![
                ("Quantity", konst(1.0)),
                ("bAutoReadyItems", V::Bool(false)),
                items(vec![off_entry(balance, 1.0, Some(true))]),
            ],
        );
    }

    fn off_more(p: &mut Syn) {
        // A keyed pool with no Quantity of its own: the keyed default tags none, the pool default gives 1.
        p.add(
            "Off.Keyed",
            KEYED,
            vec![items(vec![off_entry(SHIELD, 1.0, Some(true))])],
        );
        p.add("Off.KeyedEmpty", KEYED, vec![("Key", V::Name("SomeKey"))]);
        p.add("Gate10", ATTR, vec![]);
        p.add(
            "Off.Max",
            IP,
            vec![
                ("Quantity", konst(1.0)),
                ("MaxGameStageRequirement", r("Gate10")),
                items(vec![off_entry(SHIELD, 1.0, Some(true))]),
            ],
        );
        death_pool(p, "Off.Death2", GRENADE);
        death_pool(p, "Off.DeathAlt", SHIELD);
        p.add(
            "Off.ListOuter",
            LIST_CLASS,
            vec![
                ("ItemPoolIncludedLists", V::Array(vec![r("Off.List")])),
                (
                    "ItemPools",
                    V::Array(vec![listed("Off.Death2", konst(1.0))]),
                ),
            ],
        );
        p.add(
            "Off.ListSelf",
            LIST_CLASS,
            vec![("ItemPoolIncludedLists", V::Array(vec![r("Off.ListSelf")]))],
        );
        p.add(
            "Off.PoolSelf",
            IP,
            vec![
                ("Quantity", konst(2.0)),
                items(vec![entry(Some("Off.PoolSelf"), None, konst(1.0))]),
            ],
        );
        p.add(
            "Off.Pawn",
            PAWN,
            vec![
                (
                    "DefaultItemPoolIncludedLists",
                    V::Array(vec![r("Off.List")]),
                ),
                (
                    "DefaultItemPoolList",
                    V::Array(vec![listed("Off.DeathAlt", konst(1.0))]),
                ),
                (
                    "PlayThroughs",
                    V::Array(vec![V::Struct(
                        "PlayThroughData",
                        vec![(
                            "CustomItemPoolList",
                            V::Array(vec![listed("Off.Death2", konst(1.0))]),
                        )],
                    )]),
                ),
            ],
        );
        p.add(
            "Off.Huge",
            IP,
            vec![
                ("Quantity", konst(20_000.0)),
                items(vec![off_entry(SHIELD, 1.0, Some(true))]),
            ],
        );
        p.add(
            "Off.Uncommon",
            IP,
            vec![
                ("Quantity", konst(1.0)),
                ("bEligibleForUncommonWeightMultiplier", V::Bool(true)),
                items(vec![entry(
                    Some("Off.Death"),
                    None,
                    by_def("GD_PoolTest.Weight_2_Uncommon"),
                )]),
            ],
        );
        p.add(
            "Off.AmmoList",
            LIST_CLASS,
            vec![(
                "ItemPools",
                V::Array(vec![listed(
                    "Off.Death",
                    by_def("GD_PoolTest.AmmoDrops_PerPlayer"),
                )]),
            )],
        );
    }

    fn snap_uvhm(stage: i32) -> ow_balance::types::BalanceSnapshot {
        use ow_balance::types::*;
        BalanceSnapshot::new(
            Playthrough::UVHM,
            OpLevel::NONE,
            PlayerCount::SINGLE,
            ExpLevel::new(stage),
            GameStage(stage),
            AwesomeLevel::new(0),
            LevelCapUpgrades::ALL,
        )
        .unwrap()
    }

    impl OffFx {
        fn roll_snap(
            &self,
            source: &str,
            snap: &ow_balance::types::BalanceSnapshot,
            seed: u32,
        ) -> Result<Vec<PoolDrop>, ItemError> {
            let mut world = TestWorld::new();
            let gate10 = world.constant_attribute(10.0);
            let gate60 = world.constant_attribute(60.0);
            let mut rng = CrtRand::new(seed);
            let mut rw = RollWorld::new(&mut world, &mut rng, false);
            let mut cx = RollCx::new(
                snap,
                EvalCx::new(&mut rw),
                &self.sys,
                &self.data,
                ParityMode::Off,
            )
            .with_known("Gate10", gate10)
            .with_known("Gate60", gate60);
            roll_pool(
                &self.fx.pools,
                Some(&self.reg),
                &ObjectPath::parse(source).unwrap(),
                &PoolRequest::for_mode(ParityMode::Off),
                &mut cx,
            )
        }
    }

    fn chains(drops: &[PoolDrop]) -> Vec<Vec<&str>> {
        drops
            .iter()
            .map(|d| d.chain.iter().map(String::as_str).collect())
            .collect()
    }

    #[test]
    fn python_3_12_and_3_14_add_an_int_into_a_float_sum_differently() {
        // sum([0.1, 2, 0.2, 5]): 0x1.d333333333334p+2 on CPython 3.12.10, 0x1.d333333333333p+2 on 3.14.7 (measured).
        let items = [
            PyNum::Float(0.1),
            PyNum::Int(2),
            PyNum::Float(0.2),
            PyNum::Int(5),
        ];
        let bits = |rule| match py_sum_mixed(items, rule).unwrap() {
            PyNum::Float(x) => x.to_bits(),
            other => panic!("{other:?}"),
        };
        assert_eq!(bits(PySum::Py312), 4_619_905_087_747_339_060);
        assert_eq!(bits(PySum::Py314), 4_619_905_087_747_339_059);
    }

    #[test]
    fn off_mode_follows_included_lists_first() {
        let fx = OffFx::new("off-included");
        // Off.ListOuter includes Off.List (whose death pool is Off.Death), then lists Off.Death2 itself.
        let drops = fx.off("Off.ListOuter", 8, 1).unwrap();
        assert_eq!(chains(&drops), [vec!["Off.Death"], vec!["Off.Death2"]]);
        // A list that includes itself is refused.
        assert!(matches!(
            fx.off("Off.ListSelf", 8, 1),
            Err(ItemError::BadData(_))
        ));
    }

    #[test]
    fn off_mode_custom_playthrough_lists_replace_the_defaults() {
        let fx = OffFx::new("off-pawn");
        // Included lists first (the default ones: the playthrough entry has none), then the custom pool list, which
        // replaces the default list (Off.DeathAlt never rolls).
        let drops = fx.off("Off.Pawn", 8, 1).unwrap();
        assert_eq!(chains(&drops), [vec!["Off.Death"], vec!["Off.Death2"]]);
    }

    #[test]
    fn off_mode_walks_the_class_default_chain() {
        let fx = OffFx::new("off-keyed");
        // Default__KeyedItemPoolDefinition tags no Quantity: Default__ItemPoolDefinition's 1 applies.
        assert_eq!(fx.off("Off.Keyed", 8, 1).unwrap().len(), 1);
        assert!(matches!(
            fx.off("Off.KeyedEmpty", 8, 1),
            Err(ItemError::NotYet(n)) if n.missing == Missing::NotRead
        ));
        assert_eq!(
            fx.fx
                .pools
                .field("Off.Keyed", "Quantity", Kind::Struct)
                .unwrap(),
            fx.fx
                .pools
                .field("Off.Default", "Quantity", Kind::Struct)
                .unwrap()
        );
        // The chain's end: no object, false, an empty array; a struct is NotYet.
        assert_eq!(
            fx.fx
                .pools
                .field("Off.Keyed", "MaxGameStageRequirement", Kind::Object)
                .unwrap(),
            Pv::Null
        );
        assert_eq!(
            fx.fx
                .pools
                .field(
                    "Off.Keyed",
                    "bEligibleForUncommonWeightMultiplier",
                    Kind::Flag
                )
                .unwrap(),
            Pv::Bool(false)
        );
        assert_eq!(
            fx.fx
                .pools
                .field("Off.ListSelf", "ItemPools", Kind::Array)
                .unwrap(),
            Pv::List(vec![])
        );
        assert!(matches!(
            fx.fx.pools.field("Off.Keyed", "Probability", Kind::Struct),
            Err(ItemError::NotYet(_))
        ));
        // A tagged value that did not decode is BadData, never read as untagged.
        let undecoded = Pv::Map(DetMap::from([("Quantity".to_string(), Pv::Null)]));
        assert!(matches!(
            tagged(&undecoded, "Quantity", Kind::Struct, "P"),
            Err(ItemError::BadData(_))
        ));
        assert_eq!(
            tagged(&undecoded, "Quantity", Kind::Object, "P").unwrap(),
            Some(&Pv::Null)
        );
        assert_eq!(
            tagged(&undecoded, "Other", Kind::Struct, "P").unwrap(),
            None
        );
    }

    #[test]
    fn off_mode_closes_a_max_window_above_its_bound() {
        let fx = OffFx::new("off-max");
        assert_eq!(fx.roll_snap("Off.Max", &snapshot(10), 1).unwrap().len(), 1);
        assert_eq!(fx.roll_snap("Off.Max", &snapshot(11), 1).unwrap(), vec![]);
    }

    #[test]
    fn off_mode_refuses_cycles_and_runaway_quantities() {
        let fx = OffFx::new("off-cycle");
        assert!(matches!(
            fx.off("Off.PoolSelf", 8, 1),
            Err(ItemError::BadData(_))
        ));
        assert!(matches!(
            fx.off("Off.Huge", 8, 1),
            Err(ItemError::BadData(_))
        ));
    }

    #[test]
    fn off_mode_multipliers_answer_where_the_record_stops() {
        let fx = OffFx::new("off-multipliers");
        let not_read = |r: Result<Vec<PoolDrop>, ItemError>| matches!(r, Err(ItemError::NotYet(n)) if n.missing == Missing::NotRead);
        assert!(not_read(fx.off("Off.Uncommon", 8, 1)));
        // The per-player ammo multiplier: NotYet from playthrough index 2, 1.0 below it.
        assert!(not_read(fx.roll_snap("Off.AmmoList", &snap_uvhm(8), 1)));
        let snap = snapshot(8);
        let mut world = TestWorld::new();
        let mut rng = CrtRand::new(1);
        let mut rw = RollWorld::new(&mut world, &mut rng, false);
        let mut cx = RollCx::new(
            &snap,
            EvalCx::new(&mut rw),
            &fx.sys,
            &fx.data,
            ParityMode::Off,
        );
        let off = Off {
            pools: &fx.fx.pools,
            reg: &fx.reg,
            cx: &mut cx,
            cap: 50,
        };
        assert_eq!(off.ammo_multiplier("Off.Death").unwrap(), 1.0);
    }

    #[test]
    fn off_mode_candidate_list_floor_replace_and_draw() {
        let pool = |p: &str| Candidate::Pool {
            path: p.to_string(),
            drop: None,
        };
        let mut list = Vec::new();
        add_candidate(&mut list, pool("A"), 2.0);
        add_candidate(&mut list, pool("B"), 1e-8); // at the floor: left out
        add_candidate(&mut list, pool("C"), 1.0);
        add_candidate(&mut list, pool("A"), 5.0); // identical: replaces A's weight in place
        let shown: Vec<(String, f32)> = list
            .iter()
            .map(|(c, w)| match c {
                Candidate::Pool { path, .. } => (path.clone(), *w),
                Candidate::Balance { balance, .. } => (balance.clone(), *w),
            })
            .collect();
        assert_eq!(shown, [("A".to_string(), 5.0), ("C".to_string(), 1.0)]);
        // The draw: total 6, rand 0 -> A, rand 32767 -> C (the closed last interval).
        let ws = [5.0f32, 1.0];
        assert_eq!(single_draw(ws.iter().copied(), 0).unwrap(), 0);
        assert_eq!(single_draw(ws.iter().copied(), 32_767).unwrap(), 1);
        // Weights whose single-precision total times 32767 over 32767 lands past the last interval: NotYet.
        let past = [f32::from_bits(0x4013_1c7f), f32::from_bits(0x3fc6_4701)];
        assert!(matches!(
            single_draw(past.iter().copied(), 32_767),
            Err(ItemError::NotYet(n)) if n.missing == Missing::NotRead
        ));
    }
}
