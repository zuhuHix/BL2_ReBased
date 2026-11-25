//! Owner: lane W7-WORLDLOGIC. Vending machines: where a map's machines come from, what each stocks from, the shop
//! price arithmetic and the shop reset interval. Port of `tools/prepare_vendors.py`'s machine reading (architecture
//! section 9, "Vendors"). AI-assisted (Claude Code). Decision 2026-11-16-60.
//!
//! **Every rule is UNVERIFIED in the running game unless a line says how it was confirmed.** The records are
//! VENDORS.md (package data and one read-only live dump), NATIVE_LOOT.md, and the installed script
//! (`WillowVendingMachine`, `WillowVendingMachineBase`, `WillowGameInfo.PostBeginPlay`, read with
//! `research/script_disasm.py`).
//!
//! **Where a machine comes from (package data, [`MapView::machines`]).** A level places a
//! `WillowPopulationOpportunityPoint` whose `PopulationDef` is below `GD_Population_Shopping.VendingMachine.`; the
//! population factory exported directly below that definition is a `PopulationFactoryVendingMachine` (its `ShopType`:
//! absent is the weapons machine, `SType_Items` the Ammo Dump, `SType_Health` Dr. Zed) or a
//! `PopulationFactoryBlackMarket`; the factory's `ObjectBalanceDefinition` lists `DefaultLoot` configurations, of which
//! `Inventory` gives the stock pools and `FeaturedItem` the item-of-the-day pools, each an `ItemAttachments` list of a
//! pool and its `PoolProbability`. Paths resolve in the point's own level package first, then the map package, then
//! `Startup.upk`, as the tool reads them.
//!
//! **Two modes for the stock.**
//! - [`ParityMode::Reference`] reproduces the tool's listing ([`listing`]): each attachment's pool expanded with
//!   `loot_pools.expand` (`ow_items::pools::expand`, W4-POOLS' port) and flattened into its leaf balances with their
//!   expected count per firing, sorted by count then path, counts rounded to 6 places. The tool's reading, not the
//!   game's.
//! - [`ParityMode::Off`]: how a machine fills its 30 slots (`WillowVendingMachine.GenerateInventory`, native) is not
//!   read, so [`generate_inventory`] is `NotYet`. What is settled is the input: the attachments the balance names.
//!   [`roll_attachment`] fires one attachment's pool once through W4-POOLS' Off-mode roll
//!   (`ow_items::pools::roll_pool`), with no variance formula (the machine's is not read); it is a building block for
//!   the native once read, not the shelf.
//!
//! **Prices.**
//! - The shop's price of an item, `trunc(value * markup)` in single precision, where `value` is the item's monetary
//!   value (`ow-items`) and `markup = item_level * exponentializer + base_increase` ([`Markup`]): **confirmed (live)**
//!   on 79 of 79 stock and featured rows at player level 8 (VENDORS.md section 3, which writes `floor`; every live
//!   price and value is positive, where the two agree; the truncation is the script's float-to-int cast in
//!   `GetFeaturedItem`, and for a negative product, which no data gives, it is UNVERIFIED which the stock native uses), with the two attribute values read
//!   live ([`Markup::SANCTUARY_LIVE`]; the packages hold no value for them). The featured item's price is script
//!   (`WillowVendingMachine.GetFeaturedItem`: a fixed cost when `FixedFeaturedItemCost` is above 0, else the item's
//!   monetary value times the featured markup evaluated with the item as the source, truncated, then capped by
//!   `WillowInventory.GetCappedCurrencyAmount`, a native not read here, so [`shop_price`] applies no cap). The stock
//!   price is the native `GetSellingPriceForInventory`, not read; the live rows fit the same arithmetic.
//! - Buy-back is script (`WillowVendingMachineBase.GetBuyBackList`): the item's own monetary value, no markup.
//! - The price paid when the player sells is not read ([`sell_price`] is `NotYet`): the same native quote asked live
//!   about the player's backpack returned the buy formula (VENDORS.md section 3).
//! - Whether an item can be bought is script (`WillowVendingMachineBase.GetItemStatus`), in this order: no backpack
//!   room (its space requirement is above 0 and the unreadied count plus it exceeds the maximum), then not enough of
//!   the item's currency (`price > GetCurrencyOnHand`), then an item used at once that the pawn cannot use (ammo on a
//!   full pool), else purchasable. The VM runs it; no Rust copy (architecture rule R2).
//!
//! **The shop reset.** `WillowGameInfo.PostBeginPlay` (script) starts the countdown at `MinutesBetweenShopResets * 60`
//! seconds ([`reset_interval_seconds`]; the class default read 20 minutes live) and sends it to the replication info
//! with the `ShopTimerRate` attribute. What counts it down and resets the machines (`ResetInventory`, script, which
//! clears the shelf, calls `GenerateInventory` and stamps `LastInventoryResetTime`) is native and not read:
//! [`reset_countdown`] is `NotYet`. `ShopResetCost` is 0 in the class default.

use std::path::Path;
use std::sync::Arc;

use ow_core::num::trunc_i32;
use ow_core::{DetMap, NotYet, ParityMode};
use ow_defs::identity::Census;
use ow_items::ItemError;
use ow_items::pools::{
    EntryKind, PoolData, PoolDrop, PoolRequest, PyNum, PySum, Tree, Values, expand, roll_pool,
};
use ow_items::registry::{CensusInput, Holder, PathHolders, Props, Pv, Registry, TagLookup, decode_at};
use ow_items::roll::RollCx;
use ow_object::prefix::PropertyPrefix;
use ow_package::package::Package;
use ow_package::props;
use ow_package::schema::SchemaTyper;

#[cfg(test)]
pub(crate) mod tests;

/// The record this module follows.
pub const RECORD: &str = "docs/verification/VENDORS.md";

/// The population definitions of the shops are below this path.
pub const POPULATION: &str = "GD_Population_Shopping.VendingMachine.";
/// The startup package every path falls back to.
pub const STARTUP: &str = "WillowGame/CookedPCConsole/Startup.upk";
/// The folder of the map packages, relative to the install.
pub const COOKED: &str = "WillowGame/CookedPCConsole";
/// The code package holding `GlobalsDefinition`'s class default.
pub const WILLOW_GAME: &str = "WillowGame/CookedPCConsole/WillowGame.upk";
/// The slots of a machine's shelf (`WillowVendingMachine.ShopInventory`, a static array; the script walks 30).
pub const SHELF_SLOTS: usize = 30;

/// The schema lines the map reading adds to W4-POOLS' (`prepare_vendors.SCHEMA_EXTRA`, the two the machines need,
/// and the exit points a station's destination lists).
pub const SCHEMA_EXTRA: [&str; 3] = [
    "DefaultLoot=StructProperty:LootConfigurationData",
    "ItemAttachments=StructProperty:LootAttachmentData",
    "ExitPoints=ObjectProperty",
];

/// The alias key of `GlobalsDefinition`'s class default in [`MapView`]'s lookup (not a path any package exports).
const GLOBALS_DEFAULT: &str = "<class default>.GlobalsDefinition";

/// What went wrong reading or rolling a machine.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum VendorError {
    NotYet(NotYet),
    /// The pool layer's error.
    Item(ItemError),
    /// Data this module cannot use, with what was read.
    Data(String),
    /// A call made in a parity mode it does not serve.
    WrongMode { call: &'static str, mode: ParityMode },
}

impl std::fmt::Display for VendorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VendorError::NotYet(n) => write!(f, "{n}"),
            VendorError::Item(e) => write!(f, "{e}"),
            VendorError::Data(text) => f.write_str(text),
            VendorError::WrongMode { call, mode } => write!(f, "{call} does not serve parity mode {mode:?}"),
        }
    }
}

impl std::error::Error for VendorError {}

impl From<NotYet> for VendorError {
    fn from(n: NotYet) -> Self {
        VendorError::NotYet(n)
    }
}

impl From<ItemError> for VendorError {
    fn from(e: ItemError) -> Self {
        match e {
            ItemError::NotYet(n) => VendorError::NotYet(n),
            other => VendorError::Item(other),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The machine
// ---------------------------------------------------------------------------------------------------------------

/// The kind of shop.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShopKind {
    /// No `ShopType`: Marcus's weapons.
    Weapons,
    /// `SType_Items`: the Ammo Dump.
    Items,
    /// `SType_Health`: Dr. Zed.
    Health,
    /// A `PopulationFactoryBlackMarket` (eridium upgrades; its stock is script over the upgrade manager, not pools).
    BlackMarket,
}

impl ShopKind {
    /// The tool's name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            ShopKind::Weapons => "weapons",
            ShopKind::Items => "items",
            ShopKind::Health => "health",
            ShopKind::BlackMarket => "blackmarket",
        }
    }
}

/// The kind of shop a population factory makes: `factory_class` its class (any package prefix), `shop_type` its
/// `ShopType` when tagged. An unknown factory class or shop type is an error naming it.
pub fn shop_kind(factory_class: &str, shop_type: Option<&str>) -> Result<ShopKind, String> {
    match short(factory_class) {
        "PopulationFactoryBlackMarket" => Ok(ShopKind::BlackMarket),
        "PopulationFactoryVendingMachine" => match shop_type {
            None => Ok(ShopKind::Weapons),
            Some("SType_Items") => Ok(ShopKind::Items),
            Some("SType_Health") => Ok(ShopKind::Health),
            Some(other) => Err(format!("unknown ShopType {other}")),
        },
        other => Err(format!("unknown factory class {other}")),
    }
}

/// One `ItemAttachments` entry: a pool and its probability (init data, unevaluated).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    pub pool: String,
    pub probability: Pv,
}

/// One vending machine as the packages describe it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    /// `<level>:<point name>`, the tool's id.
    pub id: String,
    pub point: String,
    pub kind: ShopKind,
    pub factory: String,
    pub balance: String,
    /// The factory's `ObjectDefinition`, else the balance's `DefaultInteractiveObject`.
    pub definition: Option<String>,
    /// The `Inventory` configuration's attachments.
    pub inventory: Vec<Attachment>,
    /// The `FeaturedItem` configuration's attachments.
    pub featured: Vec<Attachment>,
    /// The factory's `CommerceMarkup` and its `FeaturedItem.CommerceMarkup` (init data), when tagged.
    pub markup: Option<Pv>,
    pub featured_markup: Option<Pv>,
}

/// What reading one opportunity point gave.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MachineRead {
    /// The point is not a shop.
    NotAShop,
    Machine(Box<Machine>),
    /// A shop point that does not resolve, and why (the tool's `unresolved` entry).
    Skipped(String),
}

fn short(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

fn text(v: &Pv) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty() && *s != "None").map(str::to_string)
}

fn attachments(config: &Pv) -> Vec<Attachment> {
    config
        .at("ItemAttachments")
        .items()
        .iter()
        .filter_map(|a| {
            text(a.at("ItemPool")).map(|pool| Attachment {
                pool,
                probability: a.at("PoolProbability").clone(),
            })
        })
        .collect()
}

/// `prepare_vendors.build_machine`: the machine at the opportunity point `point_path` (decoded `point`) of `level`,
/// `id` its tool id. Paths resolve in the tool's package order (the level's lookup).
pub fn read_machine(level: &LevelView, id: &str, point_path: &str, point: &Pv) -> MachineRead {
    let lookup = &level.lookup;
    let Some(popdef) = text(point.at("PopulationDef")) else {
        return MachineRead::NotAShop;
    };
    if !popdef.starts_with(POPULATION) {
        return MachineRead::NotAShop;
    }
    let Some((factory_path, factory_class)) = level.factory_of(&popdef) else {
        return MachineRead::Skipped(format!("{popdef}: no population factory export"));
    };
    let Some(factory) = lookup.get(&factory_path) else {
        return MachineRead::Skipped(format!("{factory_path}: did not decode"));
    };
    let kind = match shop_kind(&factory_class, factory.get("ShopType").and_then(Pv::as_str)) {
        Ok(k) => k,
        Err(e) => return MachineRead::Skipped(format!("{factory_path}: {e}")),
    };
    let balance_path = text(factory.at("ObjectBalanceDefinition"));
    let balance: Option<Props> = balance_path.as_deref().and_then(|p| lookup.get(p));
    let (Some(balance_path), Some(balance)) = (balance_path, balance.filter(|b| b.truthy())) else {
        return MachineRead::Skipped(format!("{factory_path}: ObjectBalanceDefinition did not resolve"));
    };
    let definition =
        text(factory.at("ObjectDefinition")).or_else(|| text(balance.at("DefaultInteractiveObject")));
    // The tool builds a dict by configuration name, so a repeated name keeps its last configuration.
    let config = |name: &str| -> Vec<Attachment> {
        balance
            .at("DefaultLoot")
            .items()
            .iter()
            .rev()
            .find(|c| c.at("ConfigurationName").as_str() == Some(name))
            .map(attachments)
            .unwrap_or_default()
    };
    let tagged = |v: &Pv| v.truthy().then(|| v.clone());
    MachineRead::Machine(Box::new(Machine {
        id: id.to_string(),
        point: point_path.to_string(),
        kind,
        factory: factory_path,
        balance: balance_path,
        definition,
        inventory: config("Inventory"),
        featured: config("FeaturedItem"),
        markup: tagged(factory.at("CommerceMarkup")),
        featured_markup: tagged(factory.at("FeaturedItem").at("CommerceMarkup")),
    }))
}

// ---------------------------------------------------------------------------------------------------------------
// The Reference listing (prepare_vendors.stock)
// ---------------------------------------------------------------------------------------------------------------

/// One leaf balance of a listing.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Leaf {
    pub balance: String,
    /// The expected count per firing, rounded to 6 places (`round(count, 6)`).
    pub weight: f64,
    /// The balance's class name without its package, `unknown` when no package holds it.
    pub class: String,
}

/// One attachment of a listing (`prepare_vendors.stock`'s row).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct StockRow {
    pub pool: String,
    /// The evaluated `PoolProbability`, `None` where the packages do not resolve it.
    pub probability: Option<PyNum>,
    pub items: Vec<Leaf>,
    /// What did not resolve (the tool's `unresolved` reasons for this row).
    pub unresolved: Vec<String>,
}

/// `loot_pools.flatten`: the expected count of each leaf balance in one firing of `tree`, added in walk order.
pub fn flatten(tree: &Tree, factor: f64, out: &mut DetMap<String, f64>, depth: u32) -> Result<(), VendorError> {
    if depth > 64 {
        return Err(VendorError::Data(format!("{}: pool tree nested too deep", tree.path)));
    }
    let quantity = match tree.quantity.map(PyNum::f) {
        Some(q) if q != 0.0 => q,
        _ => 1.0,
    };
    for entry in &tree.entries {
        let p = factor * quantity * entry.share;
        if p == 0.0 {
            continue;
        }
        match (&entry.kind, &entry.pool) {
            (EntryKind::Item, _) => {
                let total = out.entry(entry.path.clone()).or_insert(0.0);
                *total += p;
            }
            (_, Some(sub)) => flatten(sub, p, out, depth + 1)?,
            (_, None) => {
                return Err(VendorError::Data(format!("{}: a sub-pool entry without its tree", entry.path)));
            }
        }
    }
    Ok(())
}

/// Python's `round(x, 6)` on a finite double: the value correctly rounded to 6 decimal places (`%.6f`, through
/// `ow_core::fmt`, rule N6), read back. A text that does not read back (not met for a finite count) keeps `x`.
fn round6(x: f64) -> f64 {
    ow_core::fmt::fmt_f(x, 6).parse().unwrap_or(x)
}

/// Paths of entries whose weight did not resolve (`prepare_vendors.unweighed`).
fn unweighed(tree: &Tree, out: &mut Vec<String>) {
    for e in &tree.entries {
        if e.weight.is_none() {
            out.push(e.path.clone());
        }
        if let Some(sub) = &e.pool {
            unweighed(sub, out);
        }
    }
}

/// `prepare_vendors.stock` for one configuration's attachments at `stage` (Reference mode: the tool's reading).
pub fn listing(
    pools: &PoolData,
    values: &mut Values<'_>,
    attachments: &[Attachment],
    stage: i32,
    mode: ParityMode,
) -> Result<Vec<StockRow>, VendorError> {
    if mode != ParityMode::Reference {
        return Err(VendorError::WrongMode {
            call: "vendors::listing (prepare_vendors.py's reading of the pools)",
            mode,
        });
    }
    let mut rows = Vec::new();
    for a in attachments {
        let mut unresolved = Vec::new();
        let probability = values.init(&a.probability)?;
        if probability.is_none() {
            unresolved.push("PoolProbability not resolved from the packages".to_string());
        }
        if pools.class_of(&a.pool).is_empty() {
            // The tool expands it anyway, over no properties: its Quantity falls back to 1 and its gate to true,
            // so it adds neither note, and with no entries the tree is not eligible, so it lists no leaf. The row
            // below is that result (W4-POOLS' `expand` refuses a path the package does not export). A pool that is
            // exported but fails to decode is different: the tool writes a row with a note, while `expand` here
            // returns the error, which stops the listing (open item; none of the Sanctuary pools fails).
            unresolved.push("pool not found in the packages".to_string());
            rows.push(StockRow {
                pool: a.pool.clone(),
                probability,
                items: Vec::new(),
                unresolved,
            });
            continue;
        }
        let tree = expand(values, &a.pool, stage)?;
        if tree.quantity.is_none() {
            unresolved.push("Quantity not resolved from the packages".to_string());
        }
        if tree.gate.is_none() {
            unresolved.push("stage gate not resolved from the packages".to_string());
        }
        let mut missing = Vec::new();
        unweighed(&tree, &mut missing);
        unresolved.extend(missing.into_iter().map(|p| format!("entry weight not resolved: {p}")));
        let mut counts = DetMap::new();
        if tree.eligible != Some(false) {
            flatten(&tree, 1.0, &mut counts, 0)?;
        }
        let mut items: Vec<(String, f64)> = counts.into_iter().collect();
        // Sorted by count, largest first, then by path (`key=lambda kv: (-kv[1], kv[0])`).
        items.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let items = items
            .into_iter()
            .map(|(balance, count)| {
                let class = pools.class_of(&balance);
                if class.is_empty() {
                    unresolved.push(format!("leaf balance {balance} is in no package"));
                }
                Leaf {
                    class: if class.is_empty() { "unknown".to_string() } else { short(class).to_string() },
                    balance,
                    weight: round6(count),
                }
            })
            .collect();
        rows.push(StockRow {
            pool: a.pool.clone(),
            probability,
            items,
            unresolved,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------------------------------------------
// Off mode: the shelf and one attachment's roll
// ---------------------------------------------------------------------------------------------------------------

/// `WillowVendingMachine.GenerateInventory` (native, not read): how a machine fills its shelf from its attachments.
pub fn generate_inventory(_machine: &Machine) -> Result<Vec<PoolDrop>, VendorError> {
    Err(NotYet::not_read(
        "WillowVendingMachine.GenerateInventory: how a machine fills its 30 shelf slots and picks the featured item \
         from its attachments (VENDORS.md section 2, NATIVE_LOOT.md Not read yet)",
        RECORD,
    )
    .into())
}

/// One firing of one attachment's pool through W4-POOLS' Off-mode roll (`ow_items::pools::roll_pool`), at the roll
/// context's stage, with no variance formula (the machine's is not read). The attachment's probability is not drawn
/// here: how the native uses it is part of `GenerateInventory`. Off mode only.
pub fn roll_attachment(
    pools: &PoolData,
    reg: &Registry,
    attachment: &Attachment,
    cx: &mut RollCx,
) -> Result<Vec<PoolDrop>, VendorError> {
    if cx.mode != ParityMode::Off {
        return Err(VendorError::WrongMode {
            call: "vendors::roll_attachment (the Off-mode pool roll)",
            mode: cx.mode,
        });
    }
    let path = ow_core::ObjectPath::parse(&attachment.pool)
        .map_err(|e| VendorError::Data(format!("{}: {e}", attachment.pool)))?;
    Ok(roll_pool(pools, Some(reg), &path, &PoolRequest::for_mode(cx.mode), cx)?)
}

// ---------------------------------------------------------------------------------------------------------------
// Prices and the reset
// ---------------------------------------------------------------------------------------------------------------

/// The shop markup `item_level * exponentializer + base_increase` (the shape of `Init_MarkupCalc_P1` / `P2`: an offset
/// plus the unowned item level times the exponentializer), in single precision.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Markup {
    pub exponentializer: f32,
    pub base_increase: f32,
}

impl Markup {
    /// `GD_Economy.VendingMachine.Att_SellExponentializer` and `Att_BaseVendingMarkupIncrease` as read live in
    /// Sanctuary on the first playthrough (VENDORS.md section 3; the packages hold no value for them). Whether they
    /// change with the playthrough or the map is not read.
    pub const SANCTUARY_LIVE: Markup = Markup {
        exponentializer: 0.14,
        base_increase: 4.1,
    };

    pub fn new(exponentializer: f32, base_increase: f32) -> Self {
        Markup {
            exponentializer,
            base_increase,
        }
    }

    /// The markup factor at an item level.
    pub fn at(self, item_level: i32) -> f32 {
        // An item level is far below 2^24, where i32 to f32 is exact.
        (item_level as f32) * self.exponentializer + self.base_increase
    }
}

/// The shop price: `trunc(value * factor)` in single precision (the script's int-to-float, float multiply and
/// float-to-int). No currency cap is applied (module documentation). A product outside the int range is an error.
pub fn shop_price(monetary_value: i32, factor: f32) -> Result<i32, VendorError> {
    // The script converts the int to a float; above 2^24 that rounds, as the game's conversion does.
    let product = (monetary_value as f32) * factor;
    let price = trunc_i32(product);
    if price == i32::MIN && product != -2_147_483_648.0 {
        return Err(VendorError::Data(format!(
            "a shop price out of range: {monetary_value} x {factor}"
        )));
    }
    Ok(price)
}

/// The price the shop pays when the player sells (not read; VENDORS.md section 3).
pub fn sell_price(_monetary_value: i32, _item_level: i32) -> Result<i32, VendorError> {
    Err(NotYet::not_read(
        "the price a shop pays for the player's item (GetSellingPriceForInventory on the player's own item answered \
         the buy formula live; no sale was observed)",
        RECORD,
    )
    .into())
}

/// The first shop countdown, in seconds: `MinutesBetweenShopResets * 60` in single precision
/// (`WillowGameInfo.PostBeginPlay`, script).
pub fn reset_interval_seconds(minutes_between_resets: f32) -> f32 {
    minutes_between_resets * 60.0
}

/// What counts the shop countdown down and resets the machines (native, not read).
pub fn reset_countdown(_seconds_until_reset: f32, _dt: f32) -> Result<f32, VendorError> {
    Err(NotYet::not_read(
        "the shop countdown (WillowGameInfo's SecondsUntilShopsReset and ShopTimerRate are set by script; what counts \
         them down and calls ResetInventory on the machines is native)",
        RECORD,
    )
    .into())
}

// ---------------------------------------------------------------------------------------------------------------
// The map view (the install, read as the tool reads it)
// ---------------------------------------------------------------------------------------------------------------

/// One level package of a map, read with its own package first (a placed point's path exists in more than one
/// level package), then the map package, then `Startup.upk`.
pub struct LevelView {
    /// The level package's file stem (`Sanctuary_P`).
    pub name: String,
    /// Paths and classes of the level's own exports, in export order.
    pub exports: Vec<(String, String)>,
    /// Definitions, read as the registry reads them (the tag stream after the 4-byte prefix).
    pub lookup: TagLookup,
    pub pools: PoolData,
    /// The level's own package and export index by path, for [`LevelView::props`].
    package: Arc<Package>,
    own: DetMap<String, i32>,
    typer: SchemaTyper,
}

/// The property prefixes the tool tries, in its order (`prepare_slice_world.PREFIXES`): an ordinary object, a
/// component, a placed actor.
const PREFIXES: [PropertyPrefix; 3] =
    [PropertyPrefix::Ordinary, PropertyPrefix::Component, PropertyPrefix::PlacedActor];

/// The tool's `Reader.props`: the decoded properties of `export` (`ow_items::registry::decode_at`, the registry's
/// own reading and value conversion) after the first prefix of [`PREFIXES`] that decodes with no trailing byte. A
/// prefix whose walk or conversion fails, or that leaves trailing bytes, gives way to the next, as the tool's loop
/// does when `ow-package --properties` fails or reports trailing bytes. One known difference from the tool: a tagged
/// property the walker leaves undecoded (`Raw`, a delegate) is kept as `Null` by the registry's conversion, where
/// the tool's JSON has no value for it; every read here asks for a named property, so a `Null` and an absent one
/// read alike, except for `Pv::get`, which sees the `Null` as tagged (no property read here is of those kinds).
pub fn decode_exact(pkg: &Package, export: i32, typer: &SchemaTyper) -> Result<Pv, String> {
    let mut last = String::new();
    for prefix in PREFIXES {
        let start = prefix.bytes();
        let decoded = decode_at(pkg, export, start, typer);
        let range = props::property_range(pkg, export, start);
        match (decoded, range) {
            (Ok((pv, end)), Ok((from, to))) => {
                // The payload's size: from the stream's start back to the payload's start, then to its end.
                let size = to.saturating_sub(from).saturating_add(start);
                if end == size {
                    return Ok(pv);
                }
                last = format!("{} trailing byte(s) after prefix {start}", size.saturating_sub(end));
            }
            (Err(e), _) | (_, Err(e)) => last = format!("prefix {start}: {e}"),
        }
    }
    Err(format!("did not decode with an established prefix ({last})"))
}

impl LevelView {
    /// The decoded properties of `path`: an object of this level's own package as the tool reads it
    /// ([`decode_exact`]), anything else through [`LevelView::lookup`]. `Ok(None)` when no package of the level's
    /// order holds it; `Err` with the reason when it does not decode.
    pub fn props(&self, path: &str) -> Result<Option<Pv>, String> {
        match self.own.get(path) {
            Some(&export) => decode_exact(&self.package, export, &self.typer).map(Some),
            None if !self.lookup.holds(path) => Ok(None),
            None => self.lookup.props(path).map(|p| Some((*p).clone())),
        }
    }

    /// `prepare_vendors.factory_of`: the first export directly below `popdef` whose class names a population factory,
    /// in the tool's order (the level's package, the map package, `Startup.upk`, each in export order).
    pub fn factory_of(&self, popdef: &str) -> Option<(String, String)> {
        let start = format!("{popdef}.");
        let depth = popdef.matches('.').count() + 1;
        self.pools
            .exports()
            .iter()
            .find(|(p, c)| p.starts_with(&start) && p.matches('.').count() == depth && c.contains("PopulationFactory"))
            .cloned()
    }
}

/// An object the map reading had to leave out, and why.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    /// The level package's file stem.
    pub level: String,
    pub path: String,
    pub reason: String,
}

impl Skip {
    pub fn new(level: &str, path: &str, reason: impl Into<String>) -> Self {
        Skip {
            level: level.to_string(),
            path: path.to_string(),
            reason: reason.into(),
        }
    }
}

/// A map: the persistent level and the streamed levels its `LevelStreaming` objects name
/// (`prepare_slice_world.levels`), plus `GlobalsDefinition`'s class default.
pub struct MapView {
    pub levels: Vec<LevelView>,
    globals: TagLookup,
}

fn level_file(name: &str) -> String {
    format!("{COOKED}/{name}.upk")
}

fn schema() -> Vec<&'static str> {
    let mut lines: Vec<&str> = SCHEMA_EXTRA.to_vec();
    lines.extend(ow_items::pools::SCHEMA_LINES);
    lines
}

impl MapView {
    /// The view of the map `persistent` (a file stem under [`COOKED`]) over a store's census; `root` is the install.
    pub fn open(census: &Census, root: &Path, persistent: &str) -> Result<MapView, VendorError> {
        let has = |name: &str| {
            let rel = level_file(name);
            census.files.iter().any(|f| ow_core::install::ends_with(f, &rel))
        };
        if !has(persistent) {
            return Err(VendorError::Data(format!("{persistent}: no such map package in the census")));
        }
        let mut names = vec![persistent.to_string()];
        let first = Self::level(census, root, persistent, persistent)?;
        for (path, class) in &first.exports {
            if !short(class).starts_with("LevelStreaming") {
                continue;
            }
            let props = first
                .props(path)
                .map_err(|e| VendorError::Data(format!("{persistent}:{path}: {e}")))?;
            let Some(props) = props else { continue };
            if let Some(name) = text(props.at("PackageName"))
                && !names.contains(&name)
                && has(&name)
            {
                names.push(name);
            }
        }
        let mut levels = vec![first];
        for name in names.iter().skip(1) {
            levels.push(Self::level(census, root, name, persistent)?);
        }
        Ok(MapView {
            levels,
            globals: Self::globals(census, root)?,
        })
    }

    fn level(census: &Census, root: &Path, name: &str, persistent: &str) -> Result<LevelView, VendorError> {
        let own = level_file(name);
        let map = level_file(persistent);
        let mut files = vec![own.as_str()];
        if name != persistent {
            files.push(map.as_str());
        }
        files.push(STARTUP);
        let input = PoolData::from_files(census, root, &files);
        let mut exports = Vec::new();
        let mut own_exports = DetMap::new();
        let mut file = None;
        for p in &input.paths {
            let Some(h) = p.holders.first() else { continue };
            if ow_core::install::ends_with(&h.file, &own) {
                exports.push((p.path.clone(), h.class.clone()));
                own_exports.insert(p.path.clone(), h.export);
                file.get_or_insert_with(|| h.file.clone());
            }
        }
        let file = file
            .or_else(|| census.files.iter().find(|f| ow_core::install::ends_with(f, &own)).cloned())
            .ok_or_else(|| VendorError::Data(format!("{own}: not in the census")))?;
        let package = Package::load(&file).map_err(|e| VendorError::Data(format!("{own}: {e}")))?;
        let mut typer = SchemaTyper::new_empty();
        typer
            .add_text(&schema().join("\n"))
            .map_err(|e| VendorError::Data(format!("the map schema lines: {e}")))?;
        let lookup = TagLookup::new(input.clone(), &schema()).map_err(VendorError::Data)?;
        let pools = PoolData::new(input)?;
        Ok(LevelView {
            name: name.to_string(),
            exports,
            lookup,
            pools,
            package,
            own: own_exports,
            typer,
        })
    }

    fn globals(census: &Census, root: &Path) -> Result<TagLookup, VendorError> {
        let mut input = CensusInput::new(root.to_path_buf());
        for entry in &census.class_defaults {
            if entry.path != "Default__GlobalsDefinition" {
                continue;
            }
            if let Some(o) = entry.holders.iter().find(|o| ow_core::install::ends_with(&o.file, WILLOW_GAME)) {
                input.files.push(o.file.clone());
                input.paths.push(PathHolders::new(
                    GLOBALS_DEFAULT.to_string(),
                    vec![Holder::new(o.file.clone(), o.class.clone(), o.export)],
                ));
            }
        }
        TagLookup::new(input, &[]).map_err(VendorError::Data)
    }

    /// `GlobalsDefinition.MinutesBetweenShopResets` from the class default (the tool's reading).
    pub fn minutes_between_resets(&self) -> Result<f64, VendorError> {
        let props = self
            .globals
            .props(GLOBALS_DEFAULT)
            .map_err(|e| VendorError::Data(format!("GlobalsDefinition's class default: {e}")))?;
        props
            .at("MinutesBetweenShopResets")
            .number()
            .ok_or_else(|| VendorError::Data("GlobalsDefinition has no MinutesBetweenShopResets default".into()))
    }

    /// Every opportunity point of every level, in level then export order, read as a shop
    /// (`prepare_vendors.build_document`'s loop). Points that are not shops are left out; a point that does not
    /// decode is a [`MachineRead::Skipped`] with the reason.
    pub fn machines(&self) -> Vec<(usize, MachineRead)> {
        let mut out = Vec::new();
        for (i, level) in self.levels.iter().enumerate() {
            for (path, class) in &level.exports {
                if path.matches('.').count() != 2 || !path.starts_with("TheWorld.PersistentLevel.") {
                    continue;
                }
                if short(class) != "WillowPopulationOpportunityPoint" {
                    continue;
                }
                let name = path.rsplit('.').next().unwrap_or(path);
                let id = format!("{}:{name}", level.name);
                let point = match level.props(path) {
                    Ok(Some(p)) => p,
                    Ok(None) => continue,
                    Err(e) => {
                        // Not known to be a shop: recorded, never dropped.
                        out.push((i, MachineRead::Skipped(format!("{id}: the point did not decode: {e}"))));
                        continue;
                    }
                };
                match read_machine(level, &id, path, &point) {
                    MachineRead::NotAShop => {}
                    read => out.push((i, read)),
                }
            }
        }
        out
    }

    /// Values over a level's pools, as the tool's (`NumberOfPlayers` 1).
    pub fn values(&self, level: usize) -> Option<Values<'_>> {
        self.levels.get(level).map(|l| Values::new(&l.pools, 1.0, PySum::Py314))
    }
}
