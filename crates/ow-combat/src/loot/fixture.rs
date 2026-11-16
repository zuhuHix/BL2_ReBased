//! Owner: lane W6-LOOT. The loot part of `tests/combat_test.py`'s invented packages (AGENTS.md rule 9: invented
//! classes, values and script bodies, nothing from the game), added to the packages of `crate::run::fixture` through
//! one call in each of its `engine`, `willow_game` and `toy_pawns` builders. AI-assisted (Claude Code).
//!
//! This file is a child module of `run/fixture.rs` (declared there with a `#[path]`), so `ow-cli`'s tests, which
//! include that file by path, get it too, and it uses that file's private helpers through `super`.
//!
//! The data is the Python test's, value for value: two inventory balances (`Bal_Coin`, grade window 1 to 100, a
//! `WillowUsableItem`; `Bal_Gem`, window 5 to 100, a `WillowWeapon`), the pools `Pool_Coins` (coin 1 and gem 3,
//! quantity 3), `Pool_Gated` (minimum stage the attribute `Attr_Stage5`), `Pool_Empty`, `Pool_Inner`, `Pool_Nested`
//! (an empty pool weighing 1000 and `Pool_Inner` weighing 1), `Pool_Rand` (a random integer quantity 2 to 4),
//! `Pool_Fallback` (an entry naming an empty pool and a balance), `Pool_Half`, `Pool_Never`, `Pool_Ready`
//! (`bAutoReadyItems`), the lists `List_A` to `List_C`, the pawn balances `Balance_Toy` (game stage plus 1.9),
//! `Balance_Low` (experience level) and `Balance_None` (no playthrough), and the toy pawn `Pawn_Loot`, whose
//! `TakeDamage` calls `DropLootOnDeath` and then `SetHealth(0)`. Rust-only additions (not in the Python test):
//! `Balance_Two` (two playthrough entries), `List_Loop` (a list that includes itself) with `Balance_Loop`, the
//! batch's damage source `WillowDmgSource_Shotgun` and stock damage type `GD_Impact.DamageType.DmgType_Normal`, the
//! definition fields the Rust evaluator reads (`engine_loot`), `Balance_Unread` (a pool chance through an unread
//! resolver class, `ToyUnreadValueResolver`), `Balance_Partial` (`Pool_Partial`, a coin and `Pool_Self`, a pool
//! that holds itself) and `Balance_Bare` (`Pool_Bare`, whose quantity definition is a `ToyBareDefinition` without
//! the evaluator's fields, then `Pool_Coins`).

use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT};
use ow_package::testkit::{Export, PackageBuilder, ints};

use super::{NATIVE, P, SCRIPT, STATIC, Toy, data, object_payload, take_damage_params};

/// `Engine`: the random variance of an initialization definition (`RandomVariance`, `BaseValueMode`), and the other
/// fields the Rust evaluator reads on every definition (`ValueFormula`, `ConditionalInitialization`,
/// `RangeRestriction`, `RoundingMode`; the Python toy class lacks them, which the C++ evaluator does not mind). Each
/// struct declares only its enabling flags: the toy data leaves them false.
pub fn engine_loot(e: &mut Toy, definition_class: i32, data: i32) {
    let variance = e.strukt(definition_class, "RandomVariance");
    e.prop("Bool", variance, "bEnabled", 0, 0);
    e.prop("Bool", variance, "bUseIntegerRandomization", 0, 0);
    e.prop("Struct", variance, "LowerBound", 0, data);
    e.prop("Struct", variance, "UpperBound", 0, data);
    e.prop("Byte", definition_class, "BaseValueMode", 0, 0);
    e.prop("Struct", definition_class, "RandomVariance", 0, variance);
    for (name, flags) in [
        ("ValueFormula", &["bEnabled"][..]),
        ("ConditionalInitialization", &["bEnabled"][..]),
        ("RangeRestriction", &["bEnableMinValueRestriction", "bEnableMaxValueRestriction"][..]),
    ] {
        let s = e.strukt(definition_class, &format!("Toy{name}"));
        for f in flags {
            e.prop("Bool", s, f, 0, 0);
        }
        e.prop("Struct", definition_class, name, 0, s);
    }
    e.prop("Byte", definition_class, "RoundingMode", 0, 0);
}

/// `WillowGame`: the data classes the pool roll reads (names as the game's), the pawn balance with its native, the
/// pool native, and the toy loot pawn.
pub fn willow_game_loot(
    w: &mut Toy,
    globals_class: i32,
    data: i32,
    willow_pawn: i32,
    ai_pawn: i32,
    set_health: i32,
) {
    let pool_info = w.strukt(globals_class, "ItemPoolInfo");
    w.prop("Object", pool_info, "ItemPool", 0, 0);
    w.prop("Struct", pool_info, "PoolProbability", 0, data);
    let entry = w.strukt(globals_class, "BalancedInventoryData");
    for n in ["ItmPoolDefinition", "InvBalanceDefinition"] {
        w.prop("Object", entry, n, 0, 0);
    }
    w.prop("Struct", entry, "Probability", 0, data);
    w.prop("Bool", entry, "bDropOnDeath", 0, 0);
    let item_pool = w.cls("ItemPoolDefinition", 0);
    w.array_of(item_pool, "BalancedItems", "Struct", entry);
    w.prop("Struct", item_pool, "Quantity", 0, data);
    w.prop("Bool", item_pool, "bAutoReadyItems", 0, 0);
    for n in ["MinGameStageRequirement", "MaxGameStageRequirement"] {
        w.prop("Object", item_pool, n, 0, 0);
    }
    let list = w.cls("ItemPoolListDefinition", 0);
    w.array_of(list, "ItemPools", "Struct", pool_info);
    w.array_of(list, "ItemPoolIncludedLists", "Object", 0);
    let window = w.strukt(globals_class, "StageWindow");
    for n in ["MinGameStage", "MaxGameStage"] {
        w.prop("Int", window, n, 0, 0);
    }
    let modifiers = w.strukt(globals_class, "GradeModifiers");
    w.prop("Int", modifiers, "ExpLevel", 0, 0);
    w.prop("Object", modifiers, "CustomInventoryDefinition", 0, 0);
    let grade = w.strukt(globals_class, "GradeEntry");
    w.prop("Struct", grade, "GameStageRequirement", 0, window);
    for n in ["MinSpawnProbabilityModifier", "MaxSpawnProbabilityModifier"] {
        w.prop("Struct", grade, n, 0, data);
    }
    w.prop("Struct", grade, "GradeModifiers", 0, modifiers);
    let maker = w.strukt(globals_class, "ManufacturerEntry");
    w.prop("Object", maker, "Manufacturer", 0, 0);
    w.array_of(maker, "Grades", "Struct", grade);
    let balance = w.cls("InventoryBalanceDefinition", 0);
    w.prop("Object", balance, "InventoryDefinition", 0, 0);
    w.array_of(balance, "Manufacturers", "Struct", maker);
    w.prop("Bool", balance, "bInterpolateExpLevel", 0, 0);
    w.prop("Object", balance, "BaseDefinition", 0, 0);
    let definition = w.cls("InventoryDefinition", 0);
    w.prop("Class", definition, "InventoryClass", 0, 0);
    w.cls("ManufacturerDefinition", 0);
    for n in ["WillowUsableItem", "WillowWeapon"] {
        let c = w.cls(n, 0);
        w.prop("Bool", c, "bDropOnDeath", 0, 0);
    }
    let play_through = w.strukt(globals_class, "PlayThroughData");
    w.array_of(play_through, "CustomItemPoolIncludedLists", "Object", 0);
    w.array_of(play_through, "CustomItemPoolList", "Struct", pool_info);
    let pawn_balance = w.cls("AIPawnBalanceDefinition", 0);
    w.prop("Int", pawn_balance, "BaseItemGameStage", 0, 0);
    w.prop("Struct", pawn_balance, "ItemGameStageOffset", 0, data);
    w.array_of(pawn_balance, "DefaultItemPoolIncludedLists", "Object", 0);
    w.array_of(pawn_balance, "DefaultItemPoolList", "Struct", pool_info);
    w.array_of(pawn_balance, "PlayThroughs", "Struct", play_through);
    w.native(
        pawn_balance,
        "SetupPawnItemPoolList",
        &[P("Object", "SpawnedPawn", 0, 0)],
        Some(("Bool", 0)),
    );
    let pool_class = w.cls("ItemPool", 0);
    w.function(
        pool_class,
        "SpawnBalancedInventoryFromPool",
        &[
            P("Object", "Definition", 0, 0),
            P("Int", "GameStage", 0, 0),
            P("Int", "AwesomeLevel", 0, 0),
            P("Object", "ContextSource", 0, 0),
            P("Object", "SpawnedInventory", CPF_OUT, 0),
            P("Object", "GameStageVarianceFormula", CPF_OPT, 0),
            P("Float", "OuterPoolChance", CPF_OPT, 0),
            P("Bool", "bInventoryMayDropOnDeath", CPF_OPT, 0),
        ],
        Some(("Bool", 0)),
        NATIVE | STATIC,
        |_| None,
    );
    // The source a loot batch kills with (the run's `hit:<n>:9999:<type>:Shotgun`); the Python toy lacks it.
    w.cls("WillowDmgSource_Shotgun", 0);
    // A value resolver class no resolver registry knows (the evaluator's `NotYet`).
    w.cls("ToyUnreadValueResolver", 0);
    // An initialization definition class that declares none of the fields the Rust evaluator reads.
    w.cls("ToyBareDefinition", 0);
    let loot_pawn = w.cls("WillowLootPawn", ai_pawn);
    w.array_of(loot_pawn, "ItemPoolList", "Struct", pool_info);
    w.prop("Int", loot_pawn, "GameStageForSpawnedInventory", 0, 0);
    w.prop("Int", loot_pawn, "AwesomeLevel", 0, 0);
    let (drop_loot, _) = w.function(willow_pawn, "DropLootOnDeath", &[], None, SCRIPT, |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    // TakeDamage: DropLootOnDeath(); SetHealth(0.0)
    w.function(
        loot_pawn,
        "TakeDamage",
        &take_damage_params(),
        None,
        SCRIPT,
        |_| {
            let mut a = Asm::new();
            a.stmt().final_function(drop_loot, |_| {});
            a.stmt().final_function(set_health, |a| {
                a.float_const(0.0);
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
}

/// The tags of one `BalancedInventoryData` entry.
fn entry(p: &mut PackageBuilder, pool: Option<i32>, balance: Option<i32>, weight: f32, drop: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if let Some(pool) = pool {
        out.extend(p.object_tag("ItmPoolDefinition", pool));
    }
    if let Some(balance) = balance {
        out.extend(p.object_tag("InvBalanceDefinition", balance));
    }
    out.extend(data(p, "Probability", weight, 1.0));
    out.extend(p.bool_tag("bDropOnDeath", drop));
    out.extend_from_slice(&p.none());
    out
}

/// One `PlayThroughData` entry with empty custom lists.
fn entry_empty(p: &mut PackageBuilder) -> Vec<u8> {
    let mut entry = p.array_tag("CustomItemPoolIncludedLists", 0, &[]);
    entry.extend(p.array_tag("CustomItemPoolList", 0, &[]));
    entry.extend_from_slice(&p.none());
    entry
}

/// The tags of one `ItemPoolInfo` entry.
fn info(p: &mut PackageBuilder, pool: i32, chance: f32) -> Vec<u8> {
    let mut out = p.object_tag("ItemPool", pool);
    out.extend(data(p, "PoolProbability", chance, 1.0));
    out.extend_from_slice(&p.none());
    out
}

/// `ToyPawns`: the loot data of the module documentation, and the stock damage type a loot batch kills with
/// (`GD_Impact.DamageType.DmgType_Normal`, an invented definition with no values). `ai_class` is the toy AI class
/// the loot pawn uses.
pub fn toy_pawns_loot(p: &mut PackageBuilder, core: i32, engine: i32, willow: i32, ai_class: i32) {
    let package_class = p.add_import("Core", "Class", core, "Package");
    let type_class = p.add_import("Core", "Class", willow, "DamageTypeDefinition");
    let none = object_payload(p, Vec::new());
    let group = p.add_export(package_class, "GD_Impact", none.clone());
    let mut sub = Export::new(package_class, "DamageType", none.clone());
    sub.outer = group;
    let sub = p.add_export_with(sub);
    let mut normal = Export::new(type_class, "DmgType_Normal", none);
    normal.outer = sub;
    p.add_export_with(normal);
    let gearbox = p.add_import("Core", "Package", 0, "GearboxFramework");
    let attribute_class = p.add_import("Core", "Class", engine, "AttributeDefinition");
    let definition_class = p.add_import("Core", "Class", engine, "AttributeInitializationDefinition");
    let no_context_class = p.add_import("Core", "Class", gearbox, "NoContextNeededAttributeContextResolver");
    let constant_class = p.add_import("Core", "Class", gearbox, "ConstantAttributeValueResolver");
    let none = object_payload(p, Vec::new());
    let no_context = p.add_export(no_context_class, "NoContext_0", none);
    let tags = vec![p.float_tag("ConstantValue", 5.0)];
    let payload = object_payload(p, tags);
    let constant_five = p.add_export(constant_class, "Constant_5", payload);
    let tags = vec![
        p.array_tag("ContextResolverChain", 1, &ints(&[no_context])),
        p.array_tag("ValueResolverChain", 1, &ints(&[constant_five])),
    ];
    let payload = object_payload(p, tags);
    let stage_five = p.add_export(attribute_class, "Attr_Stage5", payload);
    let mut body = p.bool_tag("bEnabled", true);
    body.extend(p.bool_tag("bUseIntegerRandomization", true));
    body.extend(data(p, "LowerBound", 2.0, 1.0));
    body.extend(data(p, "UpperBound", 4.0, 1.0));
    body.extend_from_slice(&p.none());
    let tags = vec![p.struct_tag("RandomVariance", "RandomVariance", &body)];
    let payload = object_payload(p, tags);
    let random_quantity = p.add_export(definition_class, "Random_2to4", payload);
    let maker_class = p.add_import("Core", "Class", willow, "ManufacturerDefinition");
    let none = object_payload(p, Vec::new());
    let maker = p.add_export(maker_class, "Mfr_Toy", none);
    let definition = p.add_import("Core", "Class", willow, "InventoryDefinition");
    let usable = p.add_import("Core", "Class", willow, "WillowUsableItem");
    let weapon = p.add_import("Core", "Class", willow, "WillowWeapon");
    let tags = vec![p.tag("InventoryClass", "ClassProperty", &ints(&[usable]))];
    let payload = object_payload(p, tags);
    let coin_definition = p.add_export(definition, "Def_Coin", payload);
    let tags = vec![p.tag("InventoryClass", "ClassProperty", &ints(&[weapon]))];
    let payload = object_payload(p, tags);
    let gem_definition = p.add_export(definition, "Def_Gem", payload);
    let balance_class = p.add_import("Core", "Class", willow, "InventoryBalanceDefinition");
    let balance = |p: &mut PackageBuilder, name: &str, def: i32, low: i32, high: i32| {
        let mut window = p.int_tag("MinGameStage", low);
        window.extend(p.int_tag("MaxGameStage", high));
        window.extend_from_slice(&p.none());
        let mut grade = p.struct_tag("GameStageRequirement", "StageWindow", &window);
        grade.extend(data(p, "MinSpawnProbabilityModifier", 1.0, 1.0));
        grade.extend(data(p, "MaxSpawnProbabilityModifier", 1.0, 1.0));
        let mut modifiers = p.int_tag("ExpLevel", 1);
        modifiers.extend_from_slice(&p.none());
        grade.extend(p.struct_tag("GradeModifiers", "GradeModifiers", &modifiers));
        grade.extend_from_slice(&p.none());
        let mut row = p.object_tag("Manufacturer", maker);
        row.extend(p.array_tag("Grades", 1, &grade));
        row.extend_from_slice(&p.none());
        let tags = vec![
            p.object_tag("InventoryDefinition", def),
            p.array_tag("Manufacturers", 1, &row),
            p.bool_tag("bInterpolateExpLevel", true),
        ];
        let payload = object_payload(p, tags);
        p.add_export(balance_class, name, payload)
    };
    let coin = balance(p, "Bal_Coin", coin_definition, 1, 100);
    let gem = balance(p, "Bal_Gem", gem_definition, 5, 100);
    let pool_class = p.add_import("Core", "Class", willow, "ItemPoolDefinition");
    let item_pool = |p: &mut PackageBuilder,
                         name: &str,
                         entries: Vec<Vec<u8>>,
                         quantity: Option<Vec<u8>>,
                         ready: bool,
                         minimum: Option<i32>| {
        let count = i32::try_from(entries.len()).expect("a few entries");
        let body = entries.concat();
        let mut tags = vec![p.array_tag("BalancedItems", count, &body)];
        tags.push(match quantity {
            Some(q) => q,
            None => data(p, "Quantity", 1.0, 1.0),
        });
        tags.push(p.bool_tag("bAutoReadyItems", ready));
        if let Some(m) = minimum {
            tags.push(p.object_tag("MinGameStageRequirement", m));
        }
        let payload = object_payload(p, tags);
        p.add_export(pool_class, name, payload)
    };
    let coins = vec![
        entry(p, None, Some(coin), 1.0, true),
        entry(p, None, Some(gem), 3.0, false),
    ];
    let three = data(p, "Quantity", 3.0, 1.0);
    let pool_coins = item_pool(p, "Pool_Coins", coins, Some(three), false, None);
    let gated = vec![entry(p, None, Some(coin), 1.0, true)];
    let pool_gated = item_pool(p, "Pool_Gated", gated, None, false, Some(stage_five));
    let pool_empty = item_pool(p, "Pool_Empty", Vec::new(), None, false, None);
    let inner = vec![entry(p, None, Some(gem), 1.0, true)];
    let pool_inner = item_pool(p, "Pool_Inner", inner, None, false, None);
    let nested = vec![
        entry(p, Some(pool_empty), None, 1000.0, true),
        entry(p, Some(pool_inner), None, 1.0, false),
    ];
    let pool_nested = item_pool(p, "Pool_Nested", nested, None, false, None);
    let mut quantity = p.float_tag("BaseValueConstant", 0.0);
    quantity.extend(p.float_tag("BaseValueScaleConstant", 1.0));
    quantity.extend(p.object_tag("InitializationDefinition", random_quantity));
    quantity.extend_from_slice(&p.none());
    let quantity = p.struct_tag("Quantity", "AttributeInitializationData", &quantity);
    let random = vec![entry(p, None, Some(coin), 1.0, true)];
    let pool_random = item_pool(p, "Pool_Rand", random, Some(quantity), false, None);
    // An entry naming an empty pool and a balance: the empty pool counts as absent, the balance is used.
    let fallback = vec![entry(p, Some(pool_empty), Some(coin), 1.0, true)];
    let pool_fallback = item_pool(p, "Pool_Fallback", fallback, None, false, None);
    let half = vec![entry(p, None, Some(coin), 1.0, true)];
    let pool_half = item_pool(p, "Pool_Half", half, None, false, None);
    let never = vec![entry(p, None, Some(coin), 1.0, true)];
    let pool_never = item_pool(p, "Pool_Never", never, None, false, None);
    let ready = vec![entry(p, None, Some(coin), 1.0, true)];
    let pool_ready = item_pool(p, "Pool_Ready", ready, None, true, None);
    let list_class = p.add_import("Core", "Class", willow, "ItemPoolListDefinition");
    let items = info(p, pool_half, 0.5);
    let tags = vec![p.array_tag("ItemPools", 1, &items)];
    let payload = object_payload(p, tags);
    let list_c = p.add_export(list_class, "List_C", payload);
    let mut items = info(p, pool_coins, 1.0);
    items.extend(info(p, pool_ready, 1.0));
    items.extend(info(p, pool_gated, 1.0));
    let tags = vec![
        p.array_tag("ItemPools", 3, &items),
        p.array_tag("ItemPoolIncludedLists", 1, &ints(&[list_c])),
    ];
    let payload = object_payload(p, tags);
    let list_b = p.add_export(list_class, "List_B", payload);
    let items = info(p, pool_never, 0.0);
    let tags = vec![p.array_tag("ItemPools", 1, &items)];
    let payload = object_payload(p, tags);
    let list_a = p.add_export(list_class, "List_A", payload);
    let pawn_balance_class = p.add_import("Core", "Class", willow, "AIPawnBalanceDefinition");
    let mut first = p.array_tag("CustomItemPoolIncludedLists", 1, &ints(&[list_b]));
    first.extend(p.array_tag("CustomItemPoolList", 0, &[]));
    first.extend_from_slice(&p.none());
    for (name, base, offset, play_throughs) in [
        ("Balance_Toy", 1, 1.9f32, 1),
        ("Balance_Low", 0, 0.0, 1),
        ("Balance_None", 1, 0.0, 0),
    ] {
        let mut own = info(p, pool_nested, 1.0);
        own.extend(info(p, pool_random, 1.0));
        own.extend(info(p, pool_fallback, 1.0));
        let body = if play_throughs == 1 { first.clone() } else { Vec::new() };
        let tags = vec![
            p.int_tag("BaseItemGameStage", base),
            data(p, "ItemGameStageOffset", offset, 1.0),
            p.array_tag("DefaultItemPoolIncludedLists", 1, &ints(&[list_a])),
            p.array_tag("DefaultItemPoolList", 3, &own),
            p.array_tag("PlayThroughs", play_throughs, &body),
        ];
        let payload = object_payload(p, tags);
        p.add_export(pawn_balance_class, name, payload);
    }
    // Two playthrough entries: the first's custom list is List_B (as above), the second's List_A, so the entry
    // GetPlayThroughIndex picks shows in the list.
    let mut second = p.array_tag("CustomItemPoolIncludedLists", 1, &ints(&[list_a]));
    second.extend(p.array_tag("CustomItemPoolList", 0, &[]));
    second.extend_from_slice(&p.none());
    let mut both = first.clone();
    both.extend(second);
    let own = info(p, pool_coins, 1.0);
    let tags = vec![
        p.int_tag("BaseItemGameStage", 1),
        p.array_tag("DefaultItemPoolList", 1, &own),
        p.array_tag("PlayThroughs", 2, &both),
    ];
    let payload = object_payload(p, tags);
    p.add_export(pawn_balance_class, "Balance_Two", payload);
    // A list that includes itself, and a balance that includes it (the game's walk has no guard).
    let looping = p.next_export();
    let tags = vec![p.array_tag("ItemPoolIncludedLists", 1, &ints(&[looping]))];
    let payload = object_payload(p, tags);
    assert_eq!(p.add_export(list_class, "List_Loop", payload), looping);
    let empty = entry_empty(p);
    let tags = vec![
        p.int_tag("BaseItemGameStage", 1),
        p.array_tag("DefaultItemPoolIncludedLists", 1, &ints(&[looping])),
        p.array_tag("PlayThroughs", 1, &empty),
    ];
    let payload = object_payload(p, tags);
    p.add_export(pawn_balance_class, "Balance_Loop", payload);
    // A pool chance through a resolver nobody has read: the evaluator answers NotYet.
    let unread_class = p.add_import("Core", "Class", willow, "ToyUnreadValueResolver");
    let none = object_payload(p, Vec::new());
    let unread = p.add_export(unread_class, "Unread_0", none);
    let tags = vec![
        p.array_tag("ContextResolverChain", 1, &ints(&[no_context])),
        p.array_tag("ValueResolverChain", 1, &ints(&[unread])),
    ];
    let payload = object_payload(p, tags);
    let attr_unread = p.add_export(attribute_class, "Attr_Unread", payload);
    let mut chance = p.float_tag("BaseValueConstant", 0.0);
    chance.extend(p.object_tag("BaseValueAttribute", attr_unread));
    chance.extend(p.float_tag("BaseValueScaleConstant", 1.0));
    chance.extend_from_slice(&p.none());
    let mut unread_info = p.object_tag("ItemPool", pool_half);
    unread_info.extend(p.struct_tag("PoolProbability", "AttributeInitializationData", &chance));
    unread_info.extend_from_slice(&p.none());
    // A pool that holds itself (weight 1000) and a coin (weight 1e-6): its picks nest until the depth bound; and a
    // pool of a coin and that pool, so a roll can make items before the error.
    let selfish = p.next_export();
    let entries = vec![
        entry(p, Some(selfish), None, 1000.0, true),
        entry(p, None, Some(coin), 1e-6, true),
    ];
    assert_eq!(item_pool(p, "Pool_Self", entries, None, false, None), selfish);
    let entries = vec![
        entry(p, None, Some(coin), 1.0, true),
        entry(p, Some(selfish), None, 1.0, true),
    ];
    let four = data(p, "Quantity", 4.0, 1.0);
    let pool_partial = item_pool(p, "Pool_Partial", entries, Some(four), false, None);
    let partial_info = info(p, pool_partial, 1.0);
    // A quantity whose definition lacks the fields the Rust evaluator reads (a data error the C++ does not raise:
    // it reads an absent field as disabled), then the coin pool, so the roll is seen to go on.
    let bare_class = p.add_import("Core", "Class", willow, "ToyBareDefinition");
    let none = object_payload(p, Vec::new());
    let bare = p.add_export(bare_class, "Bare_0", none);
    let mut quantity = p.float_tag("BaseValueConstant", 1.0);
    quantity.extend(p.float_tag("BaseValueScaleConstant", 1.0));
    quantity.extend(p.object_tag("InitializationDefinition", bare));
    quantity.extend_from_slice(&p.none());
    let quantity = p.struct_tag("Quantity", "AttributeInitializationData", &quantity);
    let entries = vec![entry(p, None, Some(coin), 1.0, true)];
    let pool_bare = item_pool(p, "Pool_Bare", entries, Some(quantity), false, None);
    let mut bare_info = info(p, pool_bare, 1.0);
    bare_info.extend(info(p, pool_coins, 1.0));
    let empty = entry_empty(p);
    let tags = vec![
        p.int_tag("BaseItemGameStage", 1),
        p.array_tag("DefaultItemPoolList", 2, &bare_info),
        p.array_tag("PlayThroughs", 1, &empty),
    ];
    let payload = object_payload(p, tags);
    p.add_export(pawn_balance_class, "Balance_Bare", payload);
    for (name, own) in [("Balance_Unread", unread_info), ("Balance_Partial", partial_info)] {
        let empty = entry_empty(p);
        let tags = vec![
            p.int_tag("BaseItemGameStage", 1),
            p.array_tag("DefaultItemPoolList", 1, &own),
            p.array_tag("PlayThroughs", 1, &empty),
        ];
        let payload = object_payload(p, tags);
        p.add_export(pawn_balance_class, name, payload);
    }
    let loot_pawn = p.add_import("Core", "Class", willow, "WillowLootPawn");
    let tags = vec![
        p.object_tag("AIClass", ai_class),
        p.float_tag("HealthVar", 11.0),
        p.float_tag("HealthMaxVar", 12.0),
    ];
    let payload = object_payload(p, tags);
    p.add_export_with(Export::new(loot_pawn, "Pawn_Loot", payload));
}
