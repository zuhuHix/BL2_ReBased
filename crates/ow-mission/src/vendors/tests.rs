//! Tests of the vending-machine reading and rules on an invented map (AGENTS.md rule 9: every path, class use and
//! number is invented; the class and property names are the vocabulary the reading follows).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::notyet::Missing;
use ow_core::{DetMap, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_package::testkit::{PackageBuilder, ints};

use super::*;

// ---- an invented package builder (exports are outermost objects holding the dotted path in their name) ----------

#[derive(Clone)]
pub(crate) enum V {
    Bool(bool),
    Float(f32),
    Name(&'static str),
    Ref(&'static str),
    Enum(&'static str, &'static str),
    Struct(&'static str, Vec<(&'static str, V)>),
    Array(Vec<V>),
}

pub(crate) type Fields = Vec<(&'static str, V)>;

#[derive(Default)]
pub(crate) struct Syn {
    exports: Vec<(&'static str, &'static str, Fields)>,
    /// Per export: the bytes before its tags (default: the 4-byte net index) and the bytes after them.
    frames: Vec<(Vec<u8>, Vec<u8>)>,
    imports: Vec<(String, i32)>,
}

impl Syn {
    pub(crate) fn add(&mut self, path: &'static str, class: &'static str, fields: Fields) {
        self.add_framed(path, class, fields, ints(&[0]), Vec::new());
    }

    /// [`Syn::add`] with `prefix` before the tags and `tail` after them (invented bytes).
    pub(crate) fn add_framed(
        &mut self,
        path: &'static str,
        class: &'static str,
        fields: Fields,
        prefix: Vec<u8>,
        tail: Vec<u8>,
    ) {
        self.exports.push((path, class, fields));
        self.frames.push((prefix, tail));
    }

    fn import(&mut self, b: &mut PackageBuilder, class: &str, outer: i32, name: &str) -> i32 {
        let key = format!("{class}/{outer}/{name}");
        if let Some((_, r)) = self.imports.iter().find(|(k, _)| *k == key) {
            return *r;
        }
        let r = b.add_import("Core", class, outer, name);
        self.imports.push((key, r));
        r
    }

    pub(crate) fn build(mut self) -> Vec<u8> {
        let mut b = PackageBuilder::new_empty();
        b.name("None");
        let classes: Vec<(&str, &str)> = self.exports.iter().map(|(p, c, _)| (*p, *c)).collect();
        let mut indices = Vec::new();
        for (path, class) in classes {
            let (package, name) = class.split_once('.').unwrap();
            let outer = self.import(&mut b, "Package", 0, package);
            let class_ref = self.import(&mut b, "Class", outer, name);
            indices.push(b.add_export(class_ref, path, Vec::new()));
        }
        let all: Vec<Fields> = self.exports.iter().map(|(_, _, f)| f.clone()).collect();
        for (i, fields) in all.iter().enumerate() {
            let (prefix, tail) = self.frames[i].clone();
            let mut payload = prefix;
            payload.extend(self.body(&mut b, fields));
            payload.extend(tail);
            b.set_payload(indices[i], payload);
        }
        b.build()
    }

    fn reference(&mut self, b: &mut PackageBuilder, path: &'static str) -> i32 {
        match self.exports.iter().position(|(q, _, _)| *q == path) {
            Some(i) => i32::try_from(i).unwrap() + 1,
            None => self.import(b, "Object", 0, path),
        }
    }

    fn value(&mut self, b: &mut PackageBuilder, name: &str, v: &V) -> Vec<u8> {
        match v {
            V::Bool(x) => b.bool_tag(name, *x),
            V::Float(x) => b.float_tag(name, *x),
            V::Name(s) => b.name_tag(name, s),
            V::Ref(p) => {
                let r = self.reference(b, p);
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
                            let r = self.reference(b, p);
                            data.extend(ints(&[r]));
                        }
                        _ => panic!("fixture arrays hold structs or references"),
                    }
                }
                b.array_tag(name, i32::try_from(items.len()).unwrap(), &data)
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

fn konst(x: f32) -> V {
    V::Struct("AttributeInitializationData", vec![
        ("BaseValueConstant", V::Float(x)),
        ("BaseValueScaleConstant", V::Float(1.0)),
    ])
}

fn balanced(field: &'static str, path: &'static str, w: f32) -> V {
    V::Struct("BalancedInventoryData", vec![(field, V::Ref(path)), ("Probability", konst(w))])
}

fn config(name: &'static str, pool: &'static str, p: f32) -> V {
    V::Struct("LootConfigurationData", vec![
        ("ConfigurationName", V::Name(name)),
        (
            "ItemAttachments",
            V::Array(vec![V::Struct("LootAttachmentData", vec![
                ("ItemPool", V::Ref(pool)),
                ("PoolProbability", konst(p)),
            ])]),
        ),
    ])
}

pub(crate) const POINT: &str = "WillowGame.WillowPopulationOpportunityPoint";
const POOL: &str = "WillowGame.ItemPoolDefinition";
const GUN: &str = "WillowGame.WeaponBalanceDefinition";

/// The map package: a weapons shop point, a point that is no shop, a shop point without a factory, the streaming
/// object naming the side level, and the shops' data.
pub(crate) fn map() -> Vec<u8> {
    map_with(Syn::default())
}

/// [`map`] with the exports of `s` first.
pub(crate) fn map_with(mut s: Syn) -> Vec<u8> {
    s.add("TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_0", POINT, vec![(
        "PopulationDef",
        V::Ref("GD_Population_Shopping.VendingMachine.Pop_Guns"),
    )]);
    s.add("TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_1", POINT, vec![(
        "PopulationDef",
        V::Ref("GD_Population_Treasure.Pop_Box"),
    )]);
    s.add("TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_2", POINT, vec![(
        "PopulationDef",
        V::Ref("GD_Population_Shopping.VendingMachine.Pop_Broken"),
    )]);
    s.add("TheWorld.PersistentLevel.LevelStreamingKismet_0", "Engine.LevelStreamingKismet", vec![(
        "PackageName",
        V::Name("Toy_Side"),
    )]);
    s.add("GD_Population_Shopping.VendingMachine.Pop_Guns", "WillowGame.WillowPopulationDefinition", vec![]);
    s.add(
        "GD_Population_Shopping.VendingMachine.Pop_Guns.Factory",
        "WillowGame.PopulationFactoryVendingMachine",
        vec![("ObjectBalanceDefinition", V::Ref("Bal.Guns")), ("CommerceMarkup", konst(1.5))],
    );
    s.add("GD_Population_Shopping.VendingMachine.Pop_Broken", "WillowGame.WillowPopulationDefinition", vec![]);
    s.add("GD_Population_Shopping.VendingMachine.Pop_Meds", "WillowGame.WillowPopulationDefinition", vec![]);
    s.add(
        "GD_Population_Shopping.VendingMachine.Pop_Meds.Factory",
        "WillowGame.PopulationFactoryVendingMachine",
        vec![
            ("ShopType", V::Enum("EShopType", "SType_Health")),
            ("ObjectBalanceDefinition", V::Ref("Bal.Meds")),
        ],
    );
    s.add("Bal.Guns", "WillowGame.InteractiveObjectBalanceDefinition", vec![(
        "DefaultLoot",
        V::Array(vec![config("Inventory", "Pool.Stock", 1.0), config("FeaturedItem", "Pool.Featured", 0.5)]),
    )]);
    s.add("Bal.Meds", "WillowGame.InteractiveObjectBalanceDefinition", vec![(
        "DefaultLoot",
        V::Array(vec![config("Inventory", "Pool.Missing", 1.0)]),
    )]);
    s.add("Pool.Stock", POOL, vec![
        ("Quantity", konst(2.0)),
        (
            "BalancedItems",
            V::Array(vec![
                balanced("InvBalanceDefinition", "Bal.A", 3.0),
                balanced("InvBalanceDefinition", "Bal.B", 1.0),
                balanced("ItmPoolDefinition", "Pool.Sub", 4.0),
            ]),
        ),
    ]);
    s.add("Pool.Sub", POOL, vec![(
        "BalancedItems",
        V::Array(vec![balanced("InvBalanceDefinition", "Bal.A", 1.0)]),
    )]);
    s.add("Pool.Featured", POOL, vec![(
        "BalancedItems",
        V::Array(vec![balanced("InvBalanceDefinition", "Bal.C", 1.0)]),
    )]);
    for b in ["Bal.A", "Bal.B", "Bal.C"] {
        s.add(b, GUN, vec![]);
    }
    s.build()
}

/// The side level: one health shop point whose data is in the map package.
fn side() -> Vec<u8> {
    let mut s = Syn::default();
    s.add("TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_0", POINT, vec![(
        "PopulationDef",
        V::Ref("GD_Population_Shopping.VendingMachine.Pop_Meds"),
    )]);
    s.build()
}

fn willow_game() -> Vec<u8> {
    let mut s = Syn::default();
    s.add("Default__GlobalsDefinition", "WillowGame.GlobalsDefinition", vec![(
        "MinutesBetweenShopResets",
        V::Float(12.5),
    )]);
    s.build()
}

fn startup() -> Vec<u8> {
    let mut s = Syn::default();
    s.add("Toy.Startup", "Core.Object", vec![]);
    s.build()
}

static NEXT: AtomicU32 = AtomicU32::new(0);

pub(crate) struct Map {
    pub(crate) view: MapView,
    dir: PathBuf,
}

impl Drop for Map {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The view of the map `Toy_P` over the invented `map` package (and the side level, `WillowGame.upk` and
/// `Startup.upk` of this file), in `mode`.
pub(crate) fn open_map(mode: ParityMode, map: Vec<u8>) -> Map {
    let dir = std::env::temp_dir().join(format!(
        "ow-mission-vendors-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let files = [
        ("WillowGame/CookedPCConsole/Toy_P.upk", map),
        ("WillowGame/CookedPCConsole/Toy_Side.upk", side()),
        ("WillowGame/CookedPCConsole/WillowGame.upk", willow_game()),
        ("WillowGame/CookedPCConsole/Startup.upk", startup()),
    ];
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let mut defs = DefStore::from_synthetic(&dir, &refs, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let census = defs.census().unwrap();
    let root = defs.packages().root().to_path_buf();
    let view = MapView::open(&census, &root, "Toy_P").unwrap();
    Map { view, dir }
}

fn open(mode: ParityMode) -> Map {
    open_map(mode, map())
}

fn machines(map: &Map) -> Vec<(usize, MachineRead)> {
    map.view.machines()
}

// ---- tests -------------------------------------------------------------------------------------------------------

#[test]
fn the_shop_kind_follows_the_factory_class_and_shop_type() {
    let f = "WillowGame.PopulationFactoryVendingMachine";
    assert_eq!(shop_kind(f, None), Ok(ShopKind::Weapons));
    assert_eq!(shop_kind(f, Some("SType_Items")), Ok(ShopKind::Items));
    assert_eq!(shop_kind(f, Some("SType_Health")), Ok(ShopKind::Health));
    assert!(shop_kind(f, Some("SType_Other")).is_err());
    assert_eq!(shop_kind("WillowGame.PopulationFactoryBlackMarket", None), Ok(ShopKind::BlackMarket));
    assert!(shop_kind("WillowGame.PopulationFactoryBalancedAIPawn", None).is_err());
}

#[test]
fn the_map_lists_its_streamed_levels_and_its_shop_points_only() {
    let map = open(ParityMode::Off);
    let names: Vec<&str> = map.view.levels.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Toy_P", "Toy_Side"]);
    let found = machines(&map);
    assert_eq!(found.len(), 3, "two shops and one broken shop point; the treasure point is no shop");
    let MachineRead::Machine(guns) = &found[0].1 else { panic!("{:?}", found[0]) };
    assert_eq!(guns.id, "Toy_P:WillowPopulationOpportunityPoint_0");
    assert_eq!(guns.kind, ShopKind::Weapons);
    assert_eq!(guns.balance, "Bal.Guns");
    assert_eq!(guns.inventory.len(), 1);
    assert_eq!(guns.inventory[0].pool, "Pool.Stock");
    assert_eq!(guns.featured[0].pool, "Pool.Featured");
    assert!(guns.markup.is_some());
    assert!(guns.featured_markup.is_none());
    assert!(matches!(&found[1].1, MachineRead::Skipped(r) if r.contains("no population factory")));
    let MachineRead::Machine(meds) = &found[2].1 else { panic!("{:?}", found[2]) };
    assert_eq!((found[2].0, meds.kind), (1, ShopKind::Health));
    assert_eq!(meds.id, "Toy_Side:WillowPopulationOpportunityPoint_0");
}

#[test]
fn the_reference_listing_flattens_each_pool_as_the_tool_does() {
    let map = open(ParityMode::Reference);
    let found = machines(&map);
    let MachineRead::Machine(guns) = &found[0].1 else { panic!() };
    let pools = &map.view.levels[0].pools;
    let mut values = map.view.values(0).unwrap();
    let rows = listing(pools, &mut values, &guns.inventory, 8, ParityMode::Reference).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].probability.map(PyNum::f), Some(1.0));
    // Two draws: Bal.A 2 x 3/8 directly plus 2 x 4/8 through the sub-pool, Bal.B 2 x 1/8; largest first.
    let got: Vec<(&str, f64, &str)> =
        rows[0].items.iter().map(|l| (l.balance.as_str(), l.weight, l.class.as_str())).collect();
    assert_eq!(got, [("Bal.A", 1.75, "WeaponBalanceDefinition"), ("Bal.B", 0.25, "WeaponBalanceDefinition")]);
    assert!(rows[0].unresolved.is_empty(), "{:?}", rows[0].unresolved);
    let featured = listing(pools, &mut values, &guns.featured, 8, ParityMode::Reference).unwrap();
    assert_eq!(featured[0].probability.map(PyNum::f), Some(0.5));
    assert_eq!(featured[0].items.len(), 1);
}

#[test]
fn a_pool_no_package_holds_lists_nothing_and_says_so() {
    let map = open(ParityMode::Reference);
    let found = machines(&map);
    let MachineRead::Machine(meds) = &found[2].1 else { panic!() };
    let level = found[2].0;
    let mut values = map.view.values(level).unwrap();
    let rows =
        listing(&map.view.levels[level].pools, &mut values, &meds.inventory, 8, ParityMode::Reference).unwrap();
    assert!(rows[0].items.is_empty());
    assert_eq!(rows[0].unresolved, ["pool not found in the packages"]);
}

#[test]
fn the_listing_is_the_tools_reading_and_off_mode_refuses_it() {
    let map = open(ParityMode::Off);
    let found = machines(&map);
    let MachineRead::Machine(guns) = &found[0].1 else { panic!() };
    let mut values = map.view.values(0).unwrap();
    let e = listing(&map.view.levels[0].pools, &mut values, &guns.inventory, 8, ParityMode::Off).unwrap_err();
    assert!(matches!(e, VendorError::WrongMode { .. }), "{e:?}");
}

#[test]
fn the_shelf_is_not_read_in_either_mode() {
    let map = open(ParityMode::Off);
    let found = machines(&map);
    let MachineRead::Machine(guns) = &found[0].1 else { panic!() };
    let VendorError::NotYet(n) = generate_inventory(guns).unwrap_err() else { panic!() };
    assert_eq!(n.missing, Missing::NotRead);
}

#[test]
fn the_reset_interval_is_read_from_the_class_default_and_the_countdown_is_not_read() {
    let map = open(ParityMode::Off);
    let minutes = map.view.minutes_between_resets().unwrap();
    assert_eq!(minutes, 12.5);
    assert_eq!(reset_interval_seconds(12.5), 750.0);
    let VendorError::NotYet(n) = reset_countdown(750.0, 1.0).unwrap_err() else { panic!() };
    assert_eq!(n.missing, Missing::NotRead);
}

#[test]
fn the_shop_price_is_value_times_markup_truncated_in_single_precision() {
    let m = Markup::new(0.5, 2.0);
    assert_eq!(m.at(3), 3.5);
    assert_eq!(shop_price(10, m.at(3)), Ok(35));
    // Truncated toward zero, the script's float-to-int cast. VENDORS.md section 3 writes floor: the same on every live
    // (positive) row; for a negative product (no data gives one) trunc gives -13 where floor would give -14, and
    // which the game's stock native does is UNVERIFIED.
    assert_eq!(shop_price(7, 1.99), Ok(13));
    assert_eq!(shop_price(-7, 1.99), Ok(-13));
    // The int converts to a float first: above 2^24 it rounds to an even value.
    assert_eq!(shop_price(16_777_217, 1.0), Ok(16_777_216));
    assert!(shop_price(i32::MAX, 4.0).is_err());
    assert!(shop_price(1, f32::NAN).is_err());
}

#[test]
fn the_live_markup_constants_are_the_recorded_ones() {
    // VENDORS.md section 3: 0.14 per level plus 4.1 (an invented value and level below).
    let factor = Markup::SANCTUARY_LIVE.at(10);
    assert_eq!(factor, 10.0f32 * 0.14 + 4.1);
    assert_eq!(shop_price(100, factor), Ok(550));
}

#[test]
fn the_sale_price_is_not_read() {
    let VendorError::NotYet(n) = sell_price(10, 3).unwrap_err() else { panic!() };
    assert_eq!(n.missing, Missing::NotRead);
}

#[test]
fn flatten_adds_shares_down_the_tree() {
    let map = open(ParityMode::Reference);
    let mut values = map.view.values(0).unwrap();
    let tree = expand(&mut values, "Pool.Stock", 8).unwrap();
    let mut out = DetMap::new();
    flatten(&tree, 2.0, &mut out, 0).unwrap();
    assert_eq!(out.get("Bal.A"), Some(&3.5));
    assert_eq!(out.get("Bal.B"), Some(&0.5));
}

#[test]
fn a_placed_point_is_read_at_the_first_prefix_that_decodes_exactly() {
    // 26 invented zero bytes before the tags: walked from 4 or from 8 they read as the `None` tag at once and leave
    // the rest as trailing bytes, so only the 26-byte prefix decodes exactly.
    let mut s = Syn::default();
    s.add_framed(
        "TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_9",
        POINT,
        vec![
            ("PopulationDef", V::Ref("GD_Population_Shopping.VendingMachine.Pop_Guns")),
            ("ToyRadius", V::Float(5000.0)),
            ("ToyOdds", V::Float(0.1)),
        ],
        vec![0u8; 26],
        Vec::new(),
    );
    let map = open_map(ParityMode::Off, map_with(s));
    let point = map.view.levels[0].props("TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_9");
    let point = point.unwrap().expect("held by the level");
    assert_eq!(point.at("PopulationDef").as_str(), Some("GD_Population_Shopping.VendingMachine.Pop_Guns"));
    // Floats as the registry's conversion gives them (`%.9g`, read back): a whole one is an int, 0.1f its 9 digits.
    assert_eq!(point.at("ToyRadius"), &Pv::Int(5000));
    assert_eq!(point.at("ToyOdds"), &Pv::Float(0.100000001));
    let found = map.view.machines();
    let MachineRead::Machine(m) = &found[0].1 else { panic!("{:?}", found[0]) };
    assert_eq!((m.id.as_str(), m.kind), ("Toy_P:WillowPopulationOpportunityPoint_9", ShopKind::Weapons));
}

#[test]
fn a_point_that_fits_no_prefix_is_skipped_with_the_reason() {
    // A 4-byte prefix, the `None` tag and 3 invented trailing bytes: 4 leaves trailing bytes, 8 runs past the
    // payload, 26 is past it.
    let mut s = Syn::default();
    s.add_framed(
        "TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_10",
        POINT,
        vec![],
        ints(&[0]),
        vec![1, 2, 3],
    );
    let map = open_map(ParityMode::Off, map_with(s));
    let e = map.view.levels[0].props("TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_10").unwrap_err();
    assert!(e.contains("did not decode with an established prefix"), "{e}");
    let found = map.view.machines();
    let MachineRead::Skipped(reason) = &found[0].1 else { panic!("{:?}", found[0]) };
    assert!(reason.contains("WillowPopulationOpportunityPoint_10") && reason.contains("did not decode"), "{reason}");
}

#[test]
fn a_repeated_configuration_name_keeps_the_last_one() {
    let mut s = Syn::default();
    s.add("TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_7", POINT, vec![(
        "PopulationDef",
        V::Ref("GD_Population_Shopping.VendingMachine.Pop_Twice"),
    )]);
    s.add("GD_Population_Shopping.VendingMachine.Pop_Twice", "WillowGame.WillowPopulationDefinition", vec![]);
    s.add(
        "GD_Population_Shopping.VendingMachine.Pop_Twice.Factory",
        "WillowGame.PopulationFactoryVendingMachine",
        vec![("ObjectBalanceDefinition", V::Ref("Bal.Twice"))],
    );
    s.add("Bal.Twice", "WillowGame.InteractiveObjectBalanceDefinition", vec![(
        "DefaultLoot",
        V::Array(vec![config("Inventory", "Pool.Stock", 1.0), config("Inventory", "Pool.Featured", 1.0)]),
    )]);
    let map = open_map(ParityMode::Off, map_with(s));
    let found = map.view.machines();
    let MachineRead::Machine(m) = &found[0].1 else { panic!("{:?}", found[0]) };
    assert_eq!(m.inventory.len(), 1);
    assert_eq!(m.inventory[0].pool, "Pool.Featured");
}
