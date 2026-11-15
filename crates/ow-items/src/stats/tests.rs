//! Owner: lane W4-STATS. Tests of `stats.rs` and the shared fixture of `cards.rs`'s tests (AGENTS.md rule 9: every
//! object, number and name is invented, except the attribute, presentation and pool paths the rules read, which
//! are names of game objects).
//!
//! The weapon fixture is `tests/weapon_gen_test.py`'s hand-written manifest as an invented package: the same objects
//! and numbers, so the same hand-worked expectations hold (that test drove the C++ generator, which this lane
//! retires).

use super::*;
use crate::registry::{CensusInput, Holder, PathHolders};
use crate::roll::tests::{Syn, V, konst, r};
use ow_attr::api::{AttrWorld, GlobalSlots, NumForms, WorldParts};
use ow_core::rand::{CrtStream, Rand15};
use ow_object::ids::{ClassId, PropId, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use std::path::PathBuf;

pub(crate) const FILE: &str = "WillowGame/CookedPCConsole/StatsTest.upk";

/// A temporary install holding one invented package, removed on drop.
pub(crate) struct Install(pub(crate) PathBuf);

impl Install {
    pub(crate) fn new(tag: &str, bytes: Vec<u8>) -> Install {
        let dir = std::env::temp_dir().join(format!("ow-items-stats-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        Install(std::path::absolute(&dir).unwrap())
    }

    /// The census input as the store's census gives it (one file, every export in order).
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

    pub(crate) fn data(&self) -> RollData {
        RollData::new(&self.census()).unwrap()
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn effect(attribute: &'static str, kind: &'static str, value: V) -> V {
    V::Struct(
        "AttributeEffectData",
        vec![
            ("AttributeToModify", r(attribute)),
            ("ModifierType", V::Enum("EModifierType", kind)),
            ("BaseModifierValue", value),
        ],
    )
}

/// An init whose base is `attribute`, scaled by `scale`.
pub(crate) fn of_attr(attribute: &'static str, scale: f32) -> V {
    V::Struct(
        "AttributeInitializationData",
        vec![
            ("BaseValueAttribute", r(attribute)),
            ("BaseValueScaleConstant", V::Float(scale)),
        ],
    )
}

/// A colour as a tagged struct (the cooked `Color` is a fixed-layout struct, which this fixture's builder does not
/// write); the rules read its R, G and B either way.
fn rgb(r: i32, g: i32, b: i32) -> V {
    V::Struct(
        "TestColor",
        vec![
            ("R", V::Int(r)),
            ("G", V::Int(g)),
            ("B", V::Int(b)),
            ("A", V::Int(255)),
        ],
    )
}

fn constant_attribute(p: &mut Syn, path: &'static str, resolver: &'static str, value: f32) {
    p.add(path, "Engine.AttributeDefinition", vec![]);
    p.add(
        resolver,
        "GearboxFramework.ConstantAttributeValueResolver",
        vec![("ConstantValue", V::Float(value))],
    );
}

/// The weapon fixture (module documentation), with `type_extra` added to the type and `body_extra` to `Body.A`.
pub(crate) fn weapon_package(
    type_extra: Vec<(&'static str, V)>,
    body_extra: Vec<(&'static str, V)>,
    more: impl FnOnce(&mut Syn),
) -> Vec<u8> {
    let mut p = Syn::default();
    constant_attribute(
        &mut p,
        "Test.Price.Double",
        "Test.Price.Double.ConstantAttributeValueResolver_0",
        2.0,
    );
    constant_attribute(
        &mut p,
        "Test.Price.Half",
        "Test.Price.Half.ConstantAttributeValueResolver_0",
        1.5,
    );
    p.add("Test.Makers.Acme", "Engine.ManufacturerDefinition", vec![]);
    let mut weapon_type = vec![
        ("InstantHitDamage", konst(10.0)),
        ("FireRate", V::Float(0.4)),
        ("ClipSize", V::Float(12.0)),
        // 33.35 is the float 33.349998.
        ("ReloadTime", V::Float(33.35)),
        ("Spread", V::Float(1.5)),
        ("ProjectilesPerShot", V::Int(1)),
        (
            "WeaponAttributeEffects",
            V::Array(vec![effect(
                "D_Attributes.Weapon.WeaponDamage",
                "MT_PostAdd",
                of_attr("Test.Attr.Weapon_Is_Acme", 3.0),
            )]),
        ),
        ("BaseRarity", konst(1.0)),
        ("AttributeSlotBaseGrade", konst(1.0)),
        ("MonetaryValue", of_attr(PART_PRICE_TOTAL, 50.0)),
        // BaseMonetaryValueModifier: the part total starts at 2.
        ("BaseMonetaryValueModifier", V::Float(2.0)),
        ("bUsesPlayerLevelRequirement", V::Bool(true)),
        ("GestaltMesh", r("Test.Gestalt")),
    ];
    weapon_type.extend(type_extra);
    p.add("Test.Type", "WillowGame.WeaponTypeDefinition", weapon_type);
    let mut body = vec![
        (
            "WeaponAttributeEffects",
            V::Array(vec![
                effect("D_Attributes.Weapon.WeaponDamage", "MT_Scale", konst(-0.2)),
                effect(
                    "D_Attributes.Weapon.WeaponClipSize",
                    "MT_Scale",
                    konst(-0.2),
                ),
            ]),
        ),
        // Rarity 1.9 counts 1.
        ("Rarity", konst(1.9)),
        ("MonetaryValueMod", r("Test.Price.Double")),
        ("CustomPresentations", V::Array(vec![r("Pres.Red")])),
        ("GestaltModeSkeletalMeshName", V::Name("Frag_Body")),
    ];
    body.extend(body_extra);
    p.add("Body.A", "WillowGame.WeaponPartDefinition", body);
    p.add(
        "Grip.Early",
        "WillowGame.WeaponPartDefinition",
        vec![
            (
                "WeaponAttributeEffects",
                V::Array(vec![
                    effect("D_Attributes.Weapon.WeaponDamage", "MT_Scale", konst(0.5)),
                    effect("D_Attributes.Weapon.WeaponDamage", "MT_PreAdd", konst(2.0)),
                ]),
            ),
            ("GestaltModeSkeletalMeshName", V::Name("Frag_Grip")),
        ],
    );
    p.add("Grip.Late", "WillowGame.WeaponPartDefinition", vec![]);
    p.add("Barrel.A", "WillowGame.WeaponPartDefinition", vec![]);
    p.add(
        "Name.BodyTitle",
        "WillowGame.WeaponNamePartDefinition",
        vec![("PartName", V::Str("Blaster"))],
    );
    p.add(
        "Name.P2",
        "WillowGame.WeaponNamePartDefinition",
        vec![
            ("PartName", V::Str("Sharp")),
            ("MonetaryValueMod", r("Test.Price.Half")),
        ],
    );
    p.add(
        "Name.P4",
        "WillowGame.WeaponNamePartDefinition",
        vec![("PartName", V::Str("Late"))],
    );
    p.add(
        "Pres.Red",
        "WillowGame.AttributePresentationDefinition",
        vec![
            ("Description", V::Str("Hot stuff")),
            ("TextColor", rgb(0xAB, 0xCD, 0xEF)),
        ],
    );
    for (field, path, rounding) in crate::cards::PRESENTATIONS {
        if field == "weapon_zoom" {
            continue;
        }
        let mut props = vec![("RoundingMode", V::Enum("EAttributeRounding", rounding))];
        if field == "accuracy" {
            let remap = V::Struct(
                "AttributeRemapping",
                vec![
                    ("InputValueMn", konst(0.0)),
                    ("InputValueMx", konst(15.0)),
                    ("OutputValueMn", konst(100.0)),
                    ("OutputValueMx", konst(0.0)),
                ],
            );
            props.push(("RemappingData", remap));
        }
        p.add(path, "WillowGame.AttributePresentationDefinition", props);
    }
    let band = |low: i32, high: i32, rating: &'static str, color: V| {
        V::Struct(
            "RarityLevelColor",
            vec![
                ("MinLevel", V::Int(low)),
                ("MaxLevel", V::Int(high)),
                ("RarityRating", V::Enum("EItemRarity", rating)),
                ("Color", color),
            ],
        )
    };
    p.add(
        crate::rarity::GLOBALS,
        "WillowGame.GlobalsDefinition",
        vec![(
            "RarityLevelColors",
            V::Array(vec![
                band(1, 1, "RARITY_Common", rgb(255, 255, 255)),
                band(2, 2, "RARITY_Uncommon", rgb(0x12, 0x34, 0x56)),
            ]),
        )],
    );
    more(&mut p);
    p.build()
}

/// The fixture's gun at stage 1 (`gun(1, 1)` of the Python test): Body.A, Grip.Early, Barrel.A, "Sharp Blaster".
pub(crate) fn early() -> WeaponRecipe {
    WeaponRecipe::new("Test.Type")
        .with_balance("Test.Bal")
        .with_manufacturer("Test.Makers.Acme")
        .with_part("Body", "Body.A")
        .with_part("Grip", "Grip.Early")
        .with_part("Barrel", "Barrel.A")
        .with_names(Some("Name.P2"), Some("Name.BodyTitle"))
}

/// The stage-8 gun (`gun(8, 1)`): Grip.Late, "Late Blaster".
pub(crate) fn late() -> WeaponRecipe {
    early()
        .with_part("Grip", "Grip.Late")
        .with_names(Some("Name.P4"), Some("Name.BodyTitle"))
}

fn attributes(data: &RollData, recipe: &WeaponRecipe, level: i64) -> WeaponAttributes {
    let stats = StatsData::new(data);
    let mut v = Values::new(&stats, recipe.maker(), level);
    weapon_attributes(&mut v, recipe).unwrap()
}

#[test]
fn the_modifier_stack_folds_through_ow_attr() {
    let install = Install::new("stack", weapon_package(vec![], vec![], |_| {}));
    let data = install.data();
    let a = attributes(&data, &early(), 1);
    // Damage: (10 + 2) * (1 + 0.5) / (1 + 0.2f) + 3 (Weapon_Is_Acme * 3) = 17.99999996 -> float 18.
    assert_eq!(a.stat("WeaponDamage"), Some(18.0));
    // Clip: 12 / (1 + 0.2f) = 9.99999998, an integer attribute: truncated to 9.
    assert_eq!(a.stat("WeaponClipSize"), Some(9.0));
    assert_eq!(a.stat("WeaponReloadSpeed"), Some(33.349998474121094));
    // The class defaults the cooked type leaves out.
    assert_eq!(a.stat("WeaponShotCost"), Some(1.0));
    assert_eq!(a.stat("WeaponAutomaticBurstCount"), Some(0.0));
    assert_eq!(a.stat("WeaponBurstInterval"), None);
    // Stage 8 (Grip.Late, no grip effects): 10 / (1 + 0.2f) + 3 = 11.3333333 -> float 11.333333015.
    assert_eq!(
        attributes(&data, &late(), 8).stat("WeaponDamage"),
        Some(11.333333015441895)
    );
    // Another maker: Weapon_Is_Acme is 0, so the +3 goes.
    let other = early().with_manufacturer("Test.Makers.Other");
    assert_eq!(
        attributes(&data, &other, 1).stat("WeaponDamage"),
        Some(15.0)
    );
}

#[test]
fn an_unresolved_operand_leaves_the_value_unresolved() {
    let extra = vec![(
        "WeaponAttributeEffects",
        V::Array(vec![effect(
            "D_Attributes.Weapon.WeaponSpread",
            "MT_PreAdd",
            of_attr("Test.Attr.Runtime", 1.0),
        )]),
    )];
    let install = Install::new("unresolved", weapon_package(vec![], extra, |_| {}));
    let data = install.data();
    let stats = StatsData::new(&data);
    let mut v = Values::new(&stats, "Acme", 1);
    let a = weapon_attributes(&mut v, &early()).unwrap();
    assert_eq!(a.stat("WeaponSpread"), Some(1.5));
    assert!(v.unresolved.contains("Test.Attr.Runtime"));
}

#[test]
fn the_burst_rule_goes_through_the_modifier_stack() {
    let count = ("AutomaticBurstCount", V::Int(1));
    let interval = ("BurstInterval", V::Float(0.25));
    let part = (
        "WeaponAttributeEffects",
        V::Array(vec![effect(
            "D_Attributes.Weapon.WeaponAutomaticBurstCount",
            "MT_PreAdd",
            konst(2.0),
        )]),
    );
    let install = Install::new(
        "burst",
        weapon_package(vec![count.clone(), interval.clone()], vec![], |_| {}),
    );
    let a = attributes(&install.data(), &early(), 1);
    assert_eq!(
        (
            a.stat("WeaponAutomaticBurstCount"),
            a.stat("WeaponBurstInterval")
        ),
        (Some(1.0), Some(0.25))
    );
    let install = Install::new(
        "burst-part",
        weapon_package(vec![count, interval.clone()], vec![part.clone()], |_| {}),
    );
    // The type's 1 + the part's 2.
    assert_eq!(
        attributes(&install.data(), &early(), 1).stat("WeaponAutomaticBurstCount"),
        Some(3.0)
    );
    let install = Install::new(
        "burst-zero",
        weapon_package(vec![interval], vec![part], |_| {}),
    );
    // A type without a count (the class default 0) still takes the part's.
    assert_eq!(
        attributes(&install.data(), &early(), 1).stat("WeaponAutomaticBurstCount"),
        Some(2.0)
    );
}

fn slot_effect() -> (&'static str, V) {
    (
        "AttributeSlotEffects",
        V::Array(vec![V::Struct(
            "AttributeSlotEffectData",
            vec![
                ("SlotName", V::Name("Sp")),
                ("AttributeToModify", r("D_Attributes.Weapon.WeaponSpread")),
                ("ModifierType", V::Enum("EModifierType", "MT_PreAdd")),
                ("BaseModifierValue", konst(0.0)),
                ("PerGradeUpgrade", konst(0.5)),
                ("bEnforceMinimumGrade", V::Bool(true)),
                ("MinimumGrade", V::Int(0)),
                ("bEnforceMaximumGrade", V::Bool(true)),
                ("MaximumGrade", V::Int(4)),
            ],
        )]),
    )
}

fn slot_upgrade(increase: i32) -> (&'static str, V) {
    (
        "AttributeSlotUpgrades",
        V::Array(vec![V::Struct(
            "AttributeSlotUpgradeData",
            vec![
                ("SlotName", V::Name("Sp")),
                ("GradeIncrease", V::Int(increase)),
                ("bActivateSlot", V::Bool(true)),
            ],
        )]),
    )
}

#[test]
fn slot_grades_take_the_base_grade_the_clamps_and_the_constant_one() {
    // Base grade 1 + Body.A's -3 = -2 -> minimum 0 -> (0 - 1) * 0.5 = -0.5: spread 1.0.
    let install = Install::new(
        "slot-low",
        weapon_package(vec![slot_effect()], vec![slot_upgrade(-3)], |_| {}),
    );
    assert_eq!(
        attributes(&install.data(), &early(), 1).stat("WeaponSpread"),
        Some(1.0)
    );
    // Base grade 2 + 3 = 5 -> maximum 4 -> (4 - 1) * 0.5 = 1.5: spread 3.0.
    let mut high = vec![slot_effect()];
    high.push(("AttributeSlotBaseGrade", konst(2.0)));
    let install = Install::new(
        "slot-high",
        weapon_package(high, vec![slot_upgrade(3)], |_| {}),
    );
    let a = attributes(&install.data(), &early(), 1);
    assert_eq!(a.stat("WeaponSpread"), Some(3.0));
    assert_eq!(a.grades.get("Sp"), Some(&3.0));
    // An external slot does not reach the weapon.
    let mut external = slot_effect();
    if let V::Array(items) = &mut external.1
        && let V::Struct(_, fields) = &mut items[0]
    {
        fields.push(("bExternalSlot", V::Bool(true)));
    }
    let install = Install::new(
        "slot-external",
        weapon_package(vec![external], vec![slot_upgrade(-3)], |_| {}),
    );
    assert_eq!(
        attributes(&install.data(), &early(), 1).stat("WeaponSpread"),
        Some(1.5)
    );
}

#[test]
fn a_missing_slot_base_grade_is_the_class_default_one() {
    // The fixture's type sets the constant 1; the same type without it gives the same spread.
    let with = Install::new(
        "grade-one",
        weapon_package(vec![slot_effect()], vec![slot_upgrade(-3)], |_| {}),
    );
    let mut p_without = weapon_package(vec![slot_effect()], vec![slot_upgrade(-3)], |p| {
        // A second type, identical but for the base grade, under another path.
        p.add(
            "Test.TypeNoGrade",
            "WillowGame.WeaponTypeDefinition",
            vec![
                ("InstantHitDamage", konst(10.0)),
                ("Spread", V::Float(1.5)),
                slot_effect(),
            ],
        );
    });
    let without = Install::new("grade-none", std::mem::take(&mut p_without));
    let recipe = |t: &str| WeaponRecipe::new(t).with_part("Body", "Body.A");
    let a = attributes(&with.data(), &recipe("Test.Type"), 1);
    let b = attributes(&without.data(), &recipe("Test.TypeNoGrade"), 1);
    assert_eq!(a.stat("WeaponSpread"), Some(1.0));
    assert_eq!(b.stat("WeaponSpread"), Some(1.0));
}

#[test]
fn slot_grade_and_value_rules() {
    let effect = |min: i64, max: i64| {
        let mut m = DetMap::new();
        m.insert("bEnforceMinimumGrade".to_string(), Pv::Bool(true));
        m.insert("MinimumGrade".to_string(), Pv::Int(min));
        m.insert("bEnforceMaximumGrade".to_string(), Pv::Bool(true));
        m.insert("MaximumGrade".to_string(), Pv::Int(max));
        Pv::Map(m)
    };
    // Round half up: 1.5 -> 2; the maximum clamps first, then the minimum.
    assert_eq!(slot_grade(Some(1.5), 0.0, &Pv::Null), 2.0);
    assert_eq!(slot_grade(Some(1.0), 9.0, &effect(0, 4)), 4.0);
    assert_eq!(slot_grade(Some(1.0), -9.0, &effect(0, 4)), 0.0);
    assert_eq!(slot_grade(None, 0.0, &Pv::Null), 1.0);
    // (grade - 1) * per grade + base, single precision at each step.
    assert_eq!(slot_value(3.0, 0.25, 0.1), f32r(f32r(2.0 * 0.1) + 0.25));
}

#[test]
fn the_accuracy_pool_goes_through_the_modifier_stack() {
    let install = Install::new("pool-none", weapon_package(vec![], vec![], |_| {}));
    let data = install.data();
    assert_eq!(
        accuracy_pool(&StatsData::new(&data), &early(), 1.0, Some(18.0)).unwrap(),
        None
    );
    let pool = |p: &mut Syn| {
        p.add(
            ACCURACY_POOL,
            "Engine.ResourcePoolDefinition",
            vec![("BaseMinValue", konst(2.0)), ("BaseMaxValue", konst(10.0))],
        );
    };
    let install = Install::new("pool-base", weapon_package(vec![], vec![], pool));
    let data = install.data();
    let got = accuracy_pool(&StatsData::new(&data), &early(), 1.0, Some(18.0))
        .unwrap()
        .unwrap();
    assert_eq!(
        (got.min, got.max, got.idle_rate),
        (Some(2.0), Some(10.0), None)
    );
    // A part's external PreAdd and Scale on the minimum count; one on another attribute, or a weapon attribute's
    // external effect, does not.
    let external = (
        "ExternalAttributeEffects",
        V::Array(vec![
            effect(
                "D_Attributes.AccuracyResourcePool.AccuracyMinValue",
                "MT_PreAdd",
                konst(0.5),
            ),
            effect(
                "D_Attributes.AccuracyResourcePool.AccuracyMinValue",
                "MT_Scale",
                konst(0.5),
            ),
            effect("Test.Pool.Other", "MT_PreAdd", konst(5.0)),
            effect("D_Attributes.Weapon.WeaponDamage", "MT_PreAdd", konst(5.0)),
        ]),
    );
    let install = Install::new("pool-effects", weapon_package(vec![], vec![external], pool));
    let data = install.data();
    let got = accuracy_pool(&StatsData::new(&data), &early(), 1.0, Some(18.0))
        .unwrap()
        .unwrap();
    // (2 + 0.5) x (1 + 0.5)
    assert_eq!((got.min, got.max), (Some(3.75), Some(10.0)));
    assert_eq!(
        attributes(&data, &early(), 1).stat("WeaponDamage"),
        Some(18.0)
    );
}

#[test]
fn price_inits_resolve_constants_and_known_values() {
    let install = Install::new("price", weapon_package(vec![], vec![], |_| {}));
    let data = install.data();
    let stats = StatsData::new(&data);
    let mut unresolved = DetSet::new();
    let init = Pv::Map(
        [(
            "BaseValueAttribute".to_string(),
            Pv::Str("Test.Price.Double".to_string()),
        )]
        .into_iter()
        .collect(),
    );
    assert_eq!(
        price_init(&stats, &init, &[], &mut unresolved, 0).unwrap(),
        Some(2.0)
    );
    let runtime = Pv::Map(
        [(
            "BaseValueAttribute".to_string(),
            Pv::Str("Test.Runtime".to_string()),
        )]
        .into_iter()
        .collect(),
    );
    assert_eq!(
        price_init(&stats, &runtime, &[], &mut unresolved, 0).unwrap(),
        None
    );
    assert!(unresolved.contains("Test.Runtime"));
    assert_eq!(
        price_init(
            &stats,
            &runtime,
            &[("Test.Runtime".to_string(), 4.0)],
            &mut unresolved,
            0
        )
        .unwrap(),
        Some(4.0)
    );
    // The resolver constant is found by its subobject's name, not through the attribute's chain.
    assert_eq!(
        stats.resolver_constant("Test.Price.Half").unwrap(),
        Some(Pv::Float(1.5))
    );
    assert_eq!(stats.resolver_constant("Test.Price").unwrap(), None);
}

#[test]
fn the_gestalt_assembly_reads_every_static_array_element() {
    // A part with two indexed fragment names (a static array) and one named *_None, which draws nothing.
    let mut b = ow_package::testkit::PackageBuilder::new_empty();
    b.name("None");
    let core = b.add_import("Core", "Package", 0, "WillowGame");
    let class = b.add_import("Core", "Class", core, "WeaponPartDefinition");
    let type_class = b.add_import("Core", "Class", core, "WeaponTypeDefinition");
    let part = b.add_export(class, "Body.Twin", Vec::new());
    let none = b.add_export(class, "Body.Stock_None", Vec::new());
    let wt = b.add_export(type_class, "Type.Plain", Vec::new());
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend(b.name_tag("GestaltModeSkeletalMeshName", "Frag_Main"));
    for (i, n) in [(0, "Frag_Var0"), (1, "None"), (2, "Frag_Var2")] {
        let v = b.fname(n, 0);
        payload.extend(b.tag_full(
            "AdditionalGestaltModeSkeletalMeshNames",
            i,
            "NameProperty",
            8,
            &[],
            &v,
        ));
    }
    payload.extend(b.none());
    b.set_payload(part, payload);
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend(b.name_tag("GestaltModeSkeletalMeshName", "Frag_Never"));
    payload.extend(b.none());
    b.set_payload(none, payload);
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend(b.name_tag("GestaltMesh", "Gestalt_Test"));
    payload.extend(b.none());
    b.set_payload(wt, payload);
    let install = Install::new("gestalt", b.build());
    let input = install.census();
    let data = RollData::new(&input).unwrap();
    let statics = StaticArrays::new(&input).unwrap();
    assert_eq!(
        statics
            .names("Body.Twin", "AdditionalGestaltModeSkeletalMeshNames")
            .unwrap(),
        vec![
            Some("Frag_Var0".to_string()),
            Some("None".to_string()),
            Some("Frag_Var2".to_string())
        ]
    );
    let recipe = WeaponRecipe::new("Type.Plain")
        .with_part("Body", "Body.Twin")
        .with_part("Stock", "Body.Stock_None");
    let a = assembly(&StatsData::new(&data), &statics, &recipe).unwrap();
    assert_eq!(a.fragments, vec!["Frag_Main", "Frag_Var0", "Frag_Var2"]);
    assert_eq!(a.gestalt, Pv::Str("Gestalt_Test".to_string()));
    assert_eq!(a.material, Pv::Null);
}

#[test]
fn a_live_overlay_replaces_the_decoded_value() {
    let install = Install::new("overlay", weapon_package(vec![], vec![], |_| {}));
    let data = install.data();
    let capture = r#"{"Test.Type": {"class": "WeaponTypeDefinition", "properties": {"ClipSize": 30.0, "Spread": null}}}"#;
    let overlay = Overlay::from_live_capture("live", capture).unwrap();
    let stats = StatsData::new(&data).with_overlay(&overlay);
    let mut v = Values::new(&stats, "Acme", 1);
    let a = weapon_attributes(&mut v, &early()).unwrap();
    // 30 / (1 + 0.2f) = 24.99999995, truncated to 24; a captured null over a decoded value replaces it (the tool's
    // rule), so no spread base.
    assert_eq!(a.stat("WeaponClipSize"), Some(24.0));
    assert_eq!(a.stat("WeaponSpread"), None);
}

#[test]
fn recipes_keep_slot_order() {
    let r = WeaponRecipe::new("T")
        .with_part("Barrel", "B")
        .with_part("Body", "A")
        .with_part("Material", "M");
    assert_eq!(r.ordered_parts(), vec!["A", "B", "M"]);
    assert_eq!(
        r.with_part("Body", "A2").ordered_parts(),
        vec!["A2", "B", "M"]
    );
}

// ---- items: the stock shield (tests/prepare_stock_shield_test.py) -------------------------------------------------

const CAP: &str = "D_Attributes.ShieldResourcePool.ShieldMaxValue";
const RATE: &str = "D_Attributes.ShieldResourcePool.ShieldOnIdleRegenerationRate";
const DELAY: &str = "D_Attributes.ShieldResourcePool.ShieldOnIdleRegenerationDelay";

fn shield_slot(
    slot: &'static str,
    attribute: &'static str,
    per_grade: f32,
    external: bool,
    skill: bool,
    base: f32,
) -> V {
    V::Struct(
        "AttributeSlotEffectData",
        vec![
            ("SlotName", V::Name(slot)),
            ("AttributeToModify", r(attribute)),
            ("ModifierType", V::Enum("EModifierType", "MT_Scale")),
            ("BaseModifierValue", konst(base)),
            ("PerGradeUpgrade", konst(per_grade)),
            ("bExternalSlot", V::Bool(external)),
            ("bRunEffectsAsSkill", V::Bool(skill)),
        ],
    )
}

fn upgrade(slot: &'static str, increase: i32, activate: bool) -> V {
    V::Struct(
        "AttributeSlotUpgradeData",
        vec![
            ("SlotName", V::Name(slot)),
            ("GradeIncrease", V::Int(increase)),
            ("bActivateSlot", V::Bool(activate)),
        ],
    )
}

/// The Python test's `objects()`: capacity base 10 x 2^level, delay base 3 (a constant attribute), rate base 5;
/// slots Capacity and RechargeDelay (+0.1 per grade), an internal slot and a skill-run slot that must not reach the
/// pool; two bodies (Up: capacity +2, delay -2; Down: capacity -1, delay +1).
fn shield_package(
    extra_slot_effects: Vec<V>,
    max_activated: Option<i32>,
    title_upgrades: Vec<V>,
) -> Vec<u8> {
    let mut p = Syn::default();
    p.add(
        "Attr.Level",
        "Engine.AttributeDefinition",
        vec![("ValueResolverChain", V::Array(vec![r("Attr.Level.Prop_0")]))],
    );
    p.add(
        "Attr.Level.Prop_0",
        "Engine.ObjectPropertyAttributeValueResolver",
        vec![("PropertyName", V::Name("ExpLevel"))],
    );
    for (path, resolver, value) in [
        (
            "Attr.Ten",
            "Attr.Ten.ConstantAttributeValueResolver_0",
            10.0,
        ),
        ("Attr.Two", "Attr.Two.ConstantAttributeValueResolver_0", 2.0),
        (
            "Attr.Delay",
            "Attr.Delay.ConstantAttributeValueResolver_0",
            3.0,
        ),
    ] {
        constant_attribute(&mut p, path, resolver, value);
    }
    let formula = V::Struct(
        "BalanceFormula",
        vec![
            ("bEnabled", V::Bool(true)),
            ("Multiplier", of_attr("Attr.Ten", 1.0)),
            ("Level", of_attr("Attr.Two", 1.0)),
            ("Power", of_attr("Attr.Level", 1.0)),
        ],
    );
    p.add(
        "Init.Cap",
        "Engine.AttributeInitializationDefinition",
        vec![("ValueFormula", formula)],
    );
    p.add(
        "Parts.Up",
        "WillowGame.ShieldPartDefinition",
        vec![(
            "AttributeSlotUpgrades",
            V::Array(vec![
                upgrade("Capacity", 2, false),
                upgrade("RechargeDelay", -2, false),
            ]),
        )],
    );
    p.add(
        "Parts.Down",
        "WillowGame.ShieldPartDefinition",
        vec![(
            "AttributeSlotUpgrades",
            V::Array(vec![
                upgrade("Capacity", -1, false),
                upgrade("RechargeDelay", 1, false),
            ]),
        )],
    );
    p.add("Parts.Mat", "WillowGame.ShieldPartDefinition", vec![]);
    p.add("Parts.BatA", "WillowGame.ShieldPartDefinition", vec![]);
    let mut title = vec![("PartName", V::Str("Test Shield"))];
    if !title_upgrades.is_empty() {
        title.push(("AttributeSlotUpgrades", V::Array(title_upgrades)));
    }
    p.add("Names.Title", "WillowGame.ItemNamePartDefinition", title);
    let cap_init = V::Struct(
        "AttributeInitializationData",
        vec![
            ("InitializationDefinition", r("Init.Cap")),
            ("BaseValueScaleConstant", V::Float(1.0)),
        ],
    );
    let mut slots = vec![
        shield_slot("Capacity", CAP, 0.1, true, false, 0.0),
        shield_slot("RechargeDelay", DELAY, 0.1, true, false, 0.0),
        shield_slot("Inside", CAP, 1.0, false, false, 1.0),
        shield_slot("AsSkill", CAP, 1.0, true, true, 1.0),
    ];
    slots.extend(extra_slot_effects);
    let mut definition = vec![
        (
            "ExternalAttributeEffects",
            V::Array(vec![
                effect(CAP, "MT_PreAdd", cap_init),
                effect(DELAY, "MT_PreAdd", of_attr("Attr.Delay", 1.0)),
                effect(RATE, "MT_PreAdd", konst(5.0)),
            ]),
        ),
        ("AttributeSlotEffects", V::Array(slots)),
        (
            "AttributeSlotUpgrades",
            V::Array(vec![
                upgrade("Capacity", 0, true),
                upgrade("RechargeDelay", 0, true),
                upgrade("Inside", 0, true),
                upgrade("AsSkill", 0, true),
            ]),
        ),
    ];
    if let Some(n) = max_activated {
        definition.push(("AttributeSlotMaxActivated", V::Int(n)));
    }
    p.add("Items.Def", "WillowGame.ShieldDefinition", definition);
    p.build()
}

fn reference_shield(bytes: Vec<u8>, tag: &str, body: &str, level: i64) -> ItemStats {
    let install = Install::new(tag, bytes);
    let data = install.data();
    let stats = StatsData::new(&data);
    item_stats(
        &stats,
        "Items.Def",
        &[body, "Parts.BatA", "Parts.Mat"],
        &["Names.Title"],
        level,
    )
    .unwrap()
}

fn value_of(s: &ItemStats, label: &str) -> f64 {
    s.shield().into_iter().find(|(l, _)| *l == label).unwrap().1
}

#[test]
fn the_reference_shield_card_is_prepare_stock_shield_s() {
    // Level 2: capacity base 10 x 2^2 = 40. Up: capacity grade 1 + 2 = 3 -> +0.2 scale; delay grade -1 -> -0.2.
    let up = reference_shield(
        shield_package(vec![], None, vec![]),
        "shield-up",
        "Parts.Up",
        2,
    );
    assert!((value_of(&up, "Capacity") - 48.0).abs() < 1e-4);
    assert!((value_of(&up, "Recharge Delay") - 3.0 / 1.2).abs() < 1e-5);
    assert!((value_of(&up, "Recharge Rate") - 5.0).abs() < 1e-5);
    assert_eq!(
        up.reached,
        vec![
            ("Capacity".to_string(), 3.0),
            ("RechargeDelay".to_string(), -1.0)
        ]
    );
    // Down: grade 0 -> -0.1; the internal and the skill-run slots do not reach the pool.
    let down = reference_shield(
        shield_package(vec![], None, vec![]),
        "shield-down",
        "Parts.Down",
        2,
    );
    assert!((value_of(&down, "Capacity") - 40.0 / 1.1).abs() < 1e-4);
    assert!((value_of(&down, "Recharge Delay") - 3.3).abs() < 1e-5);
    assert!(
        !down
            .reached
            .iter()
            .any(|(s, _)| s == "Inside" || s == "AsSkill")
    );
    // Every effect of an active slot applies; a name part's upgrades count; the activation limit holds.
    let more = reference_shield(
        shield_package(
            vec![shield_slot("Capacity", RATE, 0.1, true, false, 0.0)],
            None,
            vec![],
        ),
        "shield-more",
        "Parts.Up",
        2,
    );
    assert!((value_of(&more, "Recharge Rate") - 6.0).abs() < 1e-5);
    let named = reference_shield(
        shield_package(vec![], None, vec![upgrade("Capacity", 1, false)]),
        "shield-name",
        "Parts.Up",
        2,
    );
    assert!((value_of(&named, "Capacity") - 52.0).abs() < 1e-4);
    let limited = reference_shield(
        shield_package(vec![], Some(1), vec![]),
        "shield-limit",
        "Parts.Up",
        2,
    );
    assert_eq!(
        limited
            .reached
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>(),
        vec!["Capacity"]
    );
    assert!((value_of(&limited, "Recharge Delay") - 3.0).abs() < 1e-5);
}

// ---- Off mode: the instance through ow-attr ----------------------------------------------------------------------

/// A world for the Off-mode tests: classes and objects built in the space (`Origin::Runtime`), no store.
struct HeapWorld {
    space: ObjectSpace,
    slots: GlobalSlots,
    globals: ObjRef,
    known: DetMap<String, ObjRef>,
    aid: StructId,
}

impl AttrWorld for HeapWorld {
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
    fn attribute_changed(&mut self, _: ObjRef, _: PropId, _: NumForms) -> Result<(), AttrError> {
        Ok(())
    }
    fn rng(&mut self, _: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Err(NotYet::not_ported("the stats test world has no generator", RECORD).into())
    }
}

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind).with_flags(flags)
}

impl HeapWorld {
    /// The attribute classes and structs the evaluator reads, the item and weapon classes with the stack attributes
    /// the tests use, and the modifier class.
    fn new() -> HeapWorld {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = PropKind::Object { class: object };
        let objs = PropKind::Array(Box::new(obj.clone()));
        let byte = PropKind::Byte { enum_: None };
        let st = |space: &mut ObjectSpace, path: &str, fields: Vec<(&str, PropKind)>| {
            let decls = fields
                .into_iter()
                .map(|(n, k)| decl(space, n, k, 0))
                .collect();
            space.classes.define_struct(path, None, decls).unwrap()
        };
        let aid = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.AttributeInitializationData",
            vec![
                ("BaseValueConstant", PropKind::Float),
                ("BaseValueAttribute", obj.clone()),
                ("InitializationDefinition", obj.clone()),
                ("BaseValueScaleConstant", PropKind::Float),
            ],
        );
        let formula = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.BalanceFormula",
            vec![
                ("bEnabled", PropKind::Bool),
                ("Multiplier", PropKind::Struct(aid)),
                ("Level", PropKind::Struct(aid)),
                ("Power", PropKind::Struct(aid)),
                ("Offset", PropKind::Struct(aid)),
            ],
        );
        let conditional = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.ConditionalInitializationExpressions",
            vec![
                ("bEnabled", PropKind::Bool),
                ("DefaultBaseValue", PropKind::Struct(aid)),
            ],
        );
        let variance = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.Variance",
            vec![
                ("bEnabled", PropKind::Bool),
                ("LowerBound", PropKind::Struct(aid)),
                ("UpperBound", PropKind::Struct(aid)),
                ("bUseIntegerRandomization", PropKind::Bool),
            ],
        );
        let range = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.Range",
            vec![
                ("bEnableMinValueRestriction", PropKind::Bool),
                ("MinValue", PropKind::Struct(aid)),
                ("bEnableMaxValueRestriction", PropKind::Bool),
                ("MaxValue", PropKind::Struct(aid)),
            ],
        );
        let effect = st(
            &mut space,
            "Engine.AttributeEffect.AttributeEffectData",
            vec![
                ("AttributeToModify", obj.clone()),
                ("ModifierType", byte.clone()),
                ("BaseModifierValue", PropKind::Struct(aid)),
            ],
        );
        let upgrade = st(
            &mut space,
            "WillowGame.WillowInventoryDefinition.AttributeSlotUpgradeData",
            vec![
                ("SlotName", PropKind::Name),
                ("GradeIncrease", PropKind::Int),
                ("bActivateSlot", PropKind::Bool),
            ],
        );
        let slot = st(
            &mut space,
            "WillowGame.WillowInventoryDefinition.AttributeSlotEffectData",
            vec![
                ("SlotName", PropKind::Name),
                ("AttributeToModify", obj.clone()),
                ("ModifierType", byte.clone()),
                ("BaseModifierValue", PropKind::Struct(aid)),
                ("PerGradeUpgrade", PropKind::Struct(aid)),
                ("bExternalSlot", PropKind::Bool),
                ("bRunEffectsAsSkill", PropKind::Bool),
                ("bEnforceMinimumGrade", PropKind::Bool),
                ("MinimumGrade", PropKind::Int),
                ("bEnforceMaximumGrade", PropKind::Bool),
                ("MaximumGrade", PropKind::Int),
            ],
        );
        let class =
            |space: &mut ObjectSpace, path: &str, sup: ClassId, decls: Vec<PropertyDecl>| {
                space.classes.define(path, Some(sup), decls).unwrap()
            };
        let decls = vec![
            decl(&mut space, "ContextResolverChain", objs.clone(), 0),
            decl(&mut space, "ValueResolverChain", objs.clone(), 0),
        ];
        let base = class(&mut space, "Engine.AttributeDefinitionBase", object, decls);
        class(&mut space, "Engine.AttributeDefinition", base, vec![]);
        let decls = vec![
            decl(&mut space, "ValueFormula", PropKind::Struct(formula), 0),
            decl(
                &mut space,
                "ConditionalInitialization",
                PropKind::Struct(conditional),
                0,
            ),
            decl(&mut space, "BaseValueMode", byte.clone(), 0),
            decl(&mut space, "RandomVariance", PropKind::Struct(variance), 0),
            decl(&mut space, "RangeRestriction", PropKind::Struct(range), 0),
            decl(&mut space, "RoundingMode", byte.clone(), 0),
        ];
        class(
            &mut space,
            "Engine.AttributeInitializationDefinition",
            object,
            decls,
        );
        let decls = vec![decl(&mut space, "ConstantValue", PropKind::Float, 0)];
        class(
            &mut space,
            "GearboxFramework.ConstantAttributeValueResolver",
            object,
            decls,
        );
        let decls = vec![decl(&mut space, "PropertyName", PropKind::Name, 0)];
        class(
            &mut space,
            "Engine.ObjectPropertyAttributeValueResolver",
            object,
            decls,
        );
        class(
            &mut space,
            "GearboxFramework.NoContextNeededAttributeContextResolver",
            object,
            vec![],
        );
        class(
            &mut space,
            "Engine.WeaponAttributeContextResolver",
            object,
            vec![],
        );
        let decls = vec![
            decl(&mut space, "Type", byte.clone(), 0),
            decl(&mut space, "Value", PropKind::Float, 0),
        ];
        let modifier = class(&mut space, "Core.AttributeModifier", object, decls);
        let globals_class = class(&mut space, "WillowGame.WillowGlobals", object, vec![]);
        // The definitions: one class holding every list the item and weapon rules read.
        let list = |s: StructId| PropKind::Array(Box::new(PropKind::Struct(s)));
        let decls = vec![
            decl(&mut space, "ExternalAttributeEffects", list(effect), 0),
            decl(&mut space, "WeaponAttributeEffects", list(effect), 0),
            decl(&mut space, "AttributeSlotEffects", list(slot), 0),
            decl(&mut space, "AttributeSlotUpgrades", list(upgrade), 0),
            decl(
                &mut space,
                "AttributeSlotBaseGrade",
                PropKind::Struct(aid),
                0,
            ),
            decl(&mut space, "AttributeSlotMaxActivated", PropKind::Int, 0),
            decl(&mut space, "InstantHitDamage", PropKind::Struct(aid), 0),
            decl(&mut space, "ClipSize", PropKind::Float, 0),
        ];
        class(&mut space, "Test.Definition", object, decls);
        // The instances: a shield with a level, a weapon with two stack attributes and a level.
        let stack = |space: &mut ObjectSpace, name: &str, int: bool| {
            let kind = if int {
                PropKind::IntAttribute
            } else {
                PropKind::FloatAttribute
            };
            vec![
                decl(space, name, kind.clone(), PropertyDecl::STACK_ATTRIBUTE),
                decl(
                    space,
                    &format!("{name}BaseValue"),
                    kind,
                    PropertyDecl::PLAIN_ATTRIBUTE,
                ),
                decl(
                    space,
                    &format!("{name}ModifierStack"),
                    PropKind::Array(Box::new(PropKind::Object { class: modifier })),
                    0,
                ),
            ]
        };
        // ExpLevel is a plain int attribute, as the game declares it (the level resolvers read it).
        let level = |space: &mut ObjectSpace| {
            decl(
                space,
                "ExpLevel",
                PropKind::IntAttribute,
                PropertyDecl::PLAIN_ATTRIBUTE,
            )
        };
        let decls = vec![level(&mut space)];
        class(&mut space, "WillowGame.WillowShield", object, decls);
        let actor = class(&mut space, "Engine.Actor", object, vec![]);
        let weapon = class(&mut space, "Engine.Weapon", actor, vec![]);
        let mut decls = stack(&mut space, "InstantHitDamage", false);
        decls.extend(stack(&mut space, "ClipSize", true));
        decls.push(level(&mut space));
        let willow = class(&mut space, "WillowGame.WillowWeapon", weapon, decls);
        for name in ["InstantHitDamage", "ClipSize"] {
            let v = space.names.intern(name);
            let b = space.names.intern(&format!("{name}BaseValue"));
            let s = space.names.intern(&format!("{name}ModifierStack"));
            space.classes.link_attribute(willow, v, b, s).unwrap();
        }
        let globals = space
            .alloc(globals_class, "Globals", None, Origin::Runtime)
            .unwrap();
        HeapWorld {
            space,
            slots: GlobalSlots::default(),
            globals,
            known: DetMap::new(),
            aid,
        }
    }

    fn class(&self, path: &str) -> ClassId {
        self.space.classes.by_path(path).unwrap()
    }

    fn obj(&mut self, path: &str, class: &str, props: Vec<(&str, Value)>) -> ObjRef {
        let c = self.class(class);
        let o = self.space.alloc(c, path, None, Origin::Runtime).unwrap();
        for (n, v) in props {
            let p = self
                .space
                .prop(c, n)
                .unwrap_or_else(|| panic!("no property {n}"));
            self.space.write(o, PropAddr::scalar(p), v).unwrap();
        }
        self.known.insert(path.to_string(), o);
        o
    }

    fn st(&mut self, path: &str, fields: Vec<(&str, Value)>) -> Value {
        let ty = self.space.classes.struct_by_path(path).unwrap();
        let mut v = self.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.space.names.find(n).unwrap();
            let entry = self
                .space
                .classes
                .struct_layout(ty)
                .unwrap()
                .field(id)
                .unwrap()
                .0;
            v.struct_mut().unwrap().fields[entry as usize] = x;
        }
        v
    }

    fn init(
        &mut self,
        constant: f32,
        attribute: Option<ObjRef>,
        definition: Option<ObjRef>,
    ) -> Value {
        let _ = self.aid;
        self.st(
            "Engine.AttributeInitializationDefinition.AttributeInitializationData",
            vec![
                ("BaseValueConstant", Value::Float(constant)),
                ("BaseValueAttribute", Value::Object(attribute)),
                ("InitializationDefinition", Value::Object(definition)),
                ("BaseValueScaleConstant", Value::Float(1.0)),
            ],
        )
    }

    fn attribute(&mut self, path: &str, context: &str, value: ObjRef) -> ObjRef {
        let c = self.obj(&format!("{path}.Context"), context, vec![]);
        let arr = |o: ObjRef| Value::array(vec![Value::Object(Some(o))]);
        self.obj(
            path,
            "Engine.AttributeDefinition",
            vec![
                ("ContextResolverChain", arr(c)),
                ("ValueResolverChain", arr(value)),
            ],
        )
    }

    fn constant(&mut self, path: &str, v: f32) -> ObjRef {
        let k = self.obj(
            &format!("{path}.ConstantAttributeValueResolver_0"),
            "GearboxFramework.ConstantAttributeValueResolver",
            vec![("ConstantValue", Value::Float(v))],
        );
        self.attribute(
            path,
            "GearboxFramework.NoContextNeededAttributeContextResolver",
            k,
        )
    }

    fn property(&mut self, path: &str, context: &str, property: &str) -> ObjRef {
        let n = Value::Name(self.space.names.intern(property));
        let k = self.obj(
            &format!("{path}.Prop_0"),
            "Engine.ObjectPropertyAttributeValueResolver",
            vec![("PropertyName", n)],
        );
        self.attribute(path, context, k)
    }

    fn effect(&mut self, attribute: ObjRef, kind: u8, value: Value) -> Value {
        self.st(
            "Engine.AttributeEffect.AttributeEffectData",
            vec![
                ("AttributeToModify", Value::Object(Some(attribute))),
                ("ModifierType", Value::Byte(kind)),
                ("BaseModifierValue", value),
            ],
        )
    }

    fn upgrade(&mut self, slot: &str, increase: i32, activate: bool) -> Value {
        let n = Value::Name(self.space.names.intern(slot));
        self.st(
            "WillowGame.WillowInventoryDefinition.AttributeSlotUpgradeData",
            vec![
                ("SlotName", n),
                ("GradeIncrease", Value::Int(increase)),
                ("bActivateSlot", Value::Bool(activate)),
            ],
        )
    }

    fn slot(
        &mut self,
        slot: &str,
        attribute: ObjRef,
        per_grade: f32,
        external: bool,
        skill: bool,
        base: f32,
    ) -> Value {
        let n = Value::Name(self.space.names.intern(slot));
        let (b, p) = (
            self.init(base, None, None),
            self.init(per_grade, None, None),
        );
        self.st(
            "WillowGame.WillowInventoryDefinition.AttributeSlotEffectData",
            vec![
                ("SlotName", n),
                ("AttributeToModify", Value::Object(Some(attribute))),
                ("ModifierType", Value::Byte(0)),
                ("BaseModifierValue", b),
                ("PerGradeUpgrade", p),
                ("bExternalSlot", Value::Bool(external)),
                ("bRunEffectsAsSkill", Value::Bool(skill)),
            ],
        )
    }
}

/// The Python test's shield as heap objects; the item level is read through the evaluator: an
/// `ObjectPropertyAttributeValueResolver` of `ExpLevel` on the item's instance (a no-context resolver gives the
/// source itself), where the Python tool substitutes the level for such an attribute.
fn heap_shield(w: &mut HeapWorld, max_activated: Option<i32>, title_upgrade: bool) {
    let level = w.property(
        "Attr.Level",
        "GearboxFramework.NoContextNeededAttributeContextResolver",
        "ExpLevel",
    );
    let ten = w.constant("Attr.Ten", 10.0);
    let two = w.constant("Attr.Two", 2.0);
    let delay = w.constant("Attr.Delay", 3.0);
    let (cap, rate, delay_attr) = (
        w.constant(CAP, 0.0),
        w.constant(RATE, 0.0),
        w.constant(DELAY, 0.0),
    );
    let (m, l, p) = (
        w.init(1.0, Some(ten), None),
        w.init(1.0, Some(two), None),
        w.init(1.0, Some(level), None),
    );
    let o = w.init(0.0, None, None);
    let formula = w.st(
        "Engine.AttributeInitializationDefinition.BalanceFormula",
        vec![
            ("bEnabled", Value::Bool(true)),
            ("Multiplier", m),
            ("Level", l),
            ("Power", p),
            ("Offset", o),
        ],
    );
    let init_cap = w.obj(
        "Init.Cap",
        "Engine.AttributeInitializationDefinition",
        vec![("ValueFormula", formula)],
    );
    let up = vec![
        w.upgrade("Capacity", 2, false),
        w.upgrade("RechargeDelay", -2, false),
    ];
    w.obj(
        "Parts.Up",
        "Test.Definition",
        vec![("AttributeSlotUpgrades", Value::array(up))],
    );
    let down = vec![
        w.upgrade("Capacity", -1, false),
        w.upgrade("RechargeDelay", 1, false),
    ];
    w.obj(
        "Parts.Down",
        "Test.Definition",
        vec![("AttributeSlotUpgrades", Value::array(down))],
    );
    w.obj("Parts.BatA", "Test.Definition", vec![]);
    w.obj("Parts.Mat", "Test.Definition", vec![]);
    let title = if title_upgrade {
        vec![w.upgrade("Capacity", 1, false)]
    } else {
        vec![]
    };
    w.obj(
        "Names.Title",
        "Test.Definition",
        vec![("AttributeSlotUpgrades", Value::array(title))],
    );
    let cap_value = w.init(0.0, None, Some(init_cap));
    let delay_value = w.init(1.0, Some(delay), None);
    let rate_value = w.init(5.0, None, None);
    let effects = vec![
        w.effect(cap, 1, cap_value),
        w.effect(delay_attr, 1, delay_value),
        w.effect(rate, 1, rate_value),
    ];
    let slots = vec![
        w.slot("Capacity", cap, 0.1, true, false, 0.0),
        w.slot("RechargeDelay", delay_attr, 0.1, true, false, 0.0),
        w.slot("Inside", cap, 1.0, false, false, 1.0),
        w.slot("AsSkill", cap, 1.0, true, true, 1.0),
    ];
    let upgrades = vec![
        w.upgrade("Capacity", 0, true),
        w.upgrade("RechargeDelay", 0, true),
        w.upgrade("Inside", 0, true),
        w.upgrade("AsSkill", 0, true),
    ];
    // The class default of AttributeSlotBaseGrade is the constant 1 (a store-loaded definition holds it).
    let one = w.init(1.0, None, None);
    let mut props = vec![
        ("AttributeSlotBaseGrade", one),
        ("ExternalAttributeEffects", Value::array(effects)),
        ("AttributeSlotEffects", Value::array(slots)),
        ("AttributeSlotUpgrades", Value::array(upgrades)),
    ];
    if let Some(n) = max_activated {
        props.push(("AttributeSlotMaxActivated", Value::Int(n)));
    }
    w.obj("Items.Def", "Test.Definition", props);
}

fn shield_data(body: &str) -> DefinitionData {
    let p = |s: &str| Some(ObjectPath::parse(s).unwrap());
    let mut parts: [Option<ObjectPath>; 9] = Default::default();
    parts[ItemSlot::Alpha.index()] = p(body);
    parts[ItemSlot::Beta.index()] = p("Parts.BatA");
    parts[ItemSlot::Material.index()] = p("Parts.Mat");
    DefinitionData {
        kind: ItemKind::Shield,
        balance: p("Grades.Shield"),
        definition: ObjectPath::parse("Items.Def").unwrap(),
        manufacturer: None,
        manufacturer_grade: 2,
        parts,
        prefix: None,
        title: p("Names.Title"),
        game_stage: ow_balance::GameStage(2),
        unique_id: 0,
    }
}

fn off_shield(body: &str, max_activated: Option<i32>, title_upgrade: bool) -> ItemStats {
    let mut w = HeapWorld::new();
    heap_shield(&mut w, max_activated, title_upgrade);
    let known = w.known.clone();
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = EvalCx::new(&mut w);
    instantiate_stats(&shield_data(body), 2, &sys, &mut cx, &known)
        .unwrap()
        .external
}

#[test]
fn the_stock_shield_card_is_reproduced_through_the_real_evaluator() {
    // prepare_stock_shield_test.py at its level (2): the same numbers, every init through ow-attr's evaluator.
    let up = off_shield("Parts.Up", None, false);
    assert!((value_of(&up, "Capacity") - 48.0).abs() < 1e-4);
    assert!((value_of(&up, "Recharge Delay") - 3.0 / 1.2).abs() < 1e-5);
    assert!((value_of(&up, "Recharge Rate") - 5.0).abs() < 1e-5);
    assert_eq!(
        up.reached,
        vec![
            ("Capacity".to_string(), 3.0),
            ("RechargeDelay".to_string(), -1.0)
        ]
    );
    let down = off_shield("Parts.Down", None, false);
    assert!((value_of(&down, "Capacity") - 40.0 / 1.1).abs() < 1e-4);
    assert!((value_of(&down, "Recharge Delay") - 3.3).abs() < 1e-5);
    let named = off_shield("Parts.Up", None, true);
    assert!((value_of(&named, "Capacity") - 52.0).abs() < 1e-4);
    let limited = off_shield("Parts.Up", Some(1), false);
    assert_eq!(
        limited
            .reached
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>(),
        vec!["Capacity"]
    );
    // The two modes agree on the same invented shield, bit for bit.
    let reference = reference_shield(
        shield_package(vec![], None, vec![]),
        "shield-modes",
        "Parts.Up",
        2,
    );
    for (label, _) in SHIELD_STATS {
        assert_eq!(
            value_of(&up, label).to_bits(),
            value_of(&reference, label).to_bits(),
            "{label}"
        );
    }
    // The card text: three decimals, trailing zeros removed (prepare_stock_shield's card).
    assert_eq!(
        crate::cards::shield_number_text(value_of(&up, "Capacity")),
        "48"
    );
    assert_eq!(
        crate::cards::shield_number_text(value_of(&up, "Recharge Delay")),
        "2.5"
    );
}

#[test]
fn an_off_mode_weapon_holds_its_stats_on_its_instance() {
    let mut w = HeapWorld::new();
    let damage = w.property(
        "D_Attributes.Weapon.WeaponDamage",
        "Engine.WeaponAttributeContextResolver",
        "InstantHitDamage",
    );
    let clip = w.property(
        "D_Attributes.Weapon.WeaponClipSize",
        "Engine.WeaponAttributeContextResolver",
        "ClipSize",
    );
    let base = w.init(10.0, None, None);
    let (scale, pre) = (w.init(0.5, None, None), w.init(2.0, None, None));
    let down = w.init(-0.2, None, None);
    let type_effects = vec![w.effect(damage, 1, pre)];
    w.obj(
        "Test.Type",
        "Test.Definition",
        vec![
            ("InstantHitDamage", base),
            ("ClipSize", Value::Float(12.0)),
            ("WeaponAttributeEffects", Value::array(type_effects)),
        ],
    );
    let down2 = down.clone();
    let body_effects = vec![
        w.effect(damage, 0, scale),
        w.effect(damage, 0, down),
        w.effect(clip, 0, down2),
    ];
    w.obj(
        "Body.A",
        "Test.Definition",
        vec![("WeaponAttributeEffects", Value::array(body_effects))],
    );
    let known = w.known.clone();
    let mut parts: [Option<ObjectPath>; 9] = Default::default();
    parts[WeaponSlot::Body.index()] = Some(ObjectPath::parse("Body.A").unwrap());
    let data = DefinitionData {
        kind: ItemKind::Weapon,
        balance: None,
        definition: ObjectPath::parse("Test.Type").unwrap(),
        manufacturer: None,
        manufacturer_grade: 1,
        parts,
        prefix: None,
        title: None,
        game_stage: ow_balance::GameStage(1),
        unique_id: 0,
    };
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = EvalCx::new(&mut w);
    let instance = instantiate_stats(&data, 1, &sys, &mut cx, &known).unwrap();
    // The attribute paths resolve through `known` only in this test world: the weapon's attributes are read by
    // path from the world's store otherwise.
    let read = |cx: &mut EvalCx<'_>, a: ObjRef| {
        sys.value_of(a, Some(instance.object), None, cx)
            .unwrap()
            .unwrap()
            .value
    };
    // (10 + 2) * (1 + 0.5) / (1 + 0.2) = 15; clip trunc(12 / 1.2) = 9 (the same fold as Reference's).
    assert_eq!(read(&mut cx, damage), 15.0);
    assert_eq!(read(&mut cx, clip), 9.0);
}
