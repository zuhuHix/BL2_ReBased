//! Owner: lane W4-STATS. Tests of `cards.rs`: the card cases of `tests/weapon_gen_test.py` on the same invented
//! weapon (`crate::stats::tests`'s fixture), and the card rules one by one (AGENTS.md rule 9: invented values).

use super::*;
use crate::merge::RollData;
use crate::roll::tests::{Syn, V, konst, r};
use crate::stats::tests::{Install, early, effect, late, weapon_package};

const RED: [u8; 3] = [0xAB, 0xCD, 0xEF];

fn card_of(data: &RollData, recipe: &WeaponRecipe, level: i64) -> WeaponCard {
    let stats = StatsData::new(data);
    weapon_card(&stats, recipe, level, &CardText::new(RED, None)).unwrap()
}

fn shown(card: &WeaponCard, field: &str) -> Option<Num> {
    card.display
        .iter()
        .find(|(f, _)| *f == field)
        .map(|(_, n)| *n)
}

#[test]
fn the_modifier_stack_and_the_card_rounding() {
    let install = Install::new("card-round", weapon_package(vec![], vec![], |_| {}));
    let data = install.data();
    let card = card_of(&data, &early(), 1);
    assert_eq!(card.damage, Some(18.0));
    assert_eq!(card.magazine, Some(9.0));
    // Reload 33.35 is the float 33.349998474; x10 = 333.49998474 is a float tie that rounds to the even 333.5,
    // + 0.5 = 334 -> the card prints 33.4 (double arithmetic would print 33.3).
    assert_eq!(card.reload_time, Some(33.349998474121094));
    assert_eq!(
        card.display,
        vec![
            ("damage", Num::Int(18)),
            ("magazine", Num::Int(9)),
            ("fire_rate", Num::Float(2.5)),
            ("reload_time", Num::Float(33.4)),
            ("accuracy", Num::Float(90.0)),
        ]
    );
    // Fire rate 1 / float(0.4) = 2.49999996 -> float 2.5; accuracy 100 - 1.5 * 6.6666665 = 90.
    assert_eq!((card.fire_rate, card.accuracy), (Some(2.5), Some(90.0)));
    // Stage 8: 11.333333015 prints 12 (IntCeil).
    let late_card = card_of(&data, &late(), 8);
    assert_eq!(
        (late_card.damage, shown(&late_card, "damage")),
        (Some(11.333333015441895), Some(Num::Int(12)))
    );
}

#[test]
fn the_damage_stack_terms_rebuild_the_card_damage() {
    let install = Install::new("card-stack", weapon_package(vec![], vec![], |_| {}));
    let card = card_of(&install.data(), &early(), 1);
    let [base, pre, post, up, down] = card.damage_stack.unwrap();
    assert_eq!((base, pre), (10.0, 2.0));
    let rebuilt = (base + pre) * (1.0 + up) / (1.0 + down) + post;
    assert!((rebuilt - card.damage.unwrap()).abs() < 1e-4);
}

#[test]
fn rarity_price_level_and_text() {
    let install = Install::new("card-rarity", weapon_package(vec![], vec![], |_| {}));
    let data = install.data();
    let card = card_of(&data, &early(), 1);
    // Rarity: trunc(1.0) + trunc(1.9) = 2 -> the Uncommon row.
    assert_eq!(
        (card.rarity_level, card.rarity, card.rarity_color.as_deref()),
        (2, Some(2), Some("#123456"))
    );
    // Price: part total 2 (type) * 2 (Body.A) * 1.5 (prefix Sharp) = 6, * 50 = 300; not a checked calculator.
    assert_eq!((card.sale_value, card.sale_value_known), (Some(300), false));
    assert_eq!((card.level_requirement, card.level_line), (1, false));
    let pv = card.to_pv();
    assert_eq!(pv.at("fun_stats"), &Pv::Str("Hot stuff".into()));
    assert_eq!(
        pv.at("fun_stats_markup"),
        &Pv::Str("\u{2022} <font color=\"#ABCDEF\">Hot stuff</font>".into())
    );
    assert_eq!(
        (
            card.manufacturer.as_deref(),
            card.projectiles,
            card.shot_cost
        ),
        (Some("Acme"), Some(1), Some(1.0))
    );
    assert_eq!(card.name, "Sharp Blaster");
    // Stage 8: prefix Late has no modifier: 2 * 2 * 50 = 200; the level line shows above 1.
    let late_card = card_of(&data, &late(), 8);
    assert_eq!(
        (
            late_card.sale_value,
            late_card.level_requirement,
            late_card.level_line
        ),
        (Some(200), 8, true)
    );
    // Without the red colour the line is not red, and a presentation that shows a number is not a custom line.
    let stats = StatsData::new(&data);
    let neutral = weapon_card(&stats, &early(), 1, &CardText::default()).unwrap();
    assert_eq!(neutral.to_pv().at("fun_stats"), &Pv::Null);
}

#[test]
fn the_sale_value_takes_the_currency_cap() {
    let big = |currency: &'static str| {
        vec![
            (
                "MonetaryValue",
                crate::stats::tests::of_attr(crate::stats::PART_PRICE_TOTAL, 5000.0),
            ),
            ("FormOfCurrency", V::Enum("ECurrencyType", currency)),
        ]
    };
    for (currency, cap) in [
        ("CURRENCY_Eridium", 500),
        ("CURRENCY_SeraphCrystals", 999),
        ("CURRENCY_Credits", 30000),
    ] {
        let install = Install::new(
            &format!("card-cap-{cap}"),
            weapon_package(big(currency), vec![], |_| {}),
        );
        // 2 * 2 * 1.5 * 5000 = 30000, capped by the currency.
        assert_eq!(
            card_of(&install.data(), &early(), 1).sale_value,
            Some(cap),
            "{currency}"
        );
    }
    let install = Install::new(
        "card-cap-bad",
        weapon_package(big("CURRENCY_Nonsense"), vec![], |_| {}),
    );
    assert_eq!(card_of(&install.data(), &early(), 1).sale_value, Some(0));
}

#[test]
fn a_mission_balance_requires_level_0() {
    let install = Install::new(
        "card-mission",
        weapon_package(vec![], vec![], |p| {
            p.add(
                "Test.Bal",
                "WillowGame.MissionWeaponBalanceDefinition",
                vec![],
            );
        }),
    );
    let card = card_of(&install.data(), &late(), 8);
    assert_eq!((card.level_requirement, card.level_line), (0, false));
}

fn slag(p: &mut Syn, mapping: bool) {
    p.add(
        "Dmg.Amp",
        "WillowGame.WillowDamageTypeDefinition",
        vec![("StatusEffect", r("Status.Amp"))],
    );
    let surface = V::Struct(
        "StatusEffectChanceModifier",
        vec![
            (
                "SurfaceType",
                V::Enum("EDamageSurfaceType", "DMGSURFACE_Generic"),
            ),
            ("BaseChance", konst(30.0)),
        ],
    );
    p.add(
        "Status.Amp",
        "WillowGame.StatusEffectDefinition",
        vec![
            (
                "StatusEffectType",
                V::Enum("EStatusEffectType", "STATUS_EFFECT_Amp"),
            ),
            ("DamageSurfaceChanceModifiers", V::Array(vec![surface])),
        ],
    );
    if mapping {
        let row = V::Struct(
            "WeaponStatusEffectPresentationMapping",
            vec![
                ("DamageType", r("Dmg.Amp")),
                ("Presentation", r("Pres.Slag")),
            ],
        );
        p.add(
            STATUS_CHANCE_MAPPING,
            "WillowGame.WeaponStatusEffectAttributePresentationDefinition",
            vec![("MappingData", V::Array(vec![row]))],
        );
        let remap = V::Struct(
            "AttributeRemapping",
            vec![
                ("InputValueMx", konst(3.33)),
                ("OutputValueMx", konst(100.0)),
            ],
        );
        p.add(
            "Pres.Slag",
            "WillowGame.AttributePresentationDefinition",
            vec![
                ("bValueRemappingEnabled", V::Bool(true)),
                ("RemappingData", remap),
            ],
        );
    }
}

#[test]
fn the_status_chance_maps_through_the_presentation_and_clamps() {
    let typed = |m: f32| {
        vec![
            ("DefaultDamageTypeDefinition", r("Dmg.Amp")),
            ("BaseStatusEffectChanceModifier", konst(m)),
        ]
    };
    // No remap for the damage type: Generic BaseChance 30 * 2.0 = 60.
    let install = Install::new(
        "card-slag-plain",
        weapon_package(typed(2.0), vec![], |p| slag(p, false)),
    );
    let card = card_of(&install.data(), &early(), 1);
    let status = card.status.clone().unwrap();
    assert_eq!(
        (status.status_chance, shown(&card, "status_chance")),
        (Some(60.0), Some(Num::Float(60.0)))
    );
    // Remap [0, 3.33] -> [0, 100]: slope f32(100 / 3.33f) = 30.030031, * 2.0 = 60.060062 -> prints 60.1.
    let install = Install::new(
        "card-slag",
        weapon_package(typed(2.0), vec![], |p| slag(p, true)),
    );
    let card = card_of(&install.data(), &early(), 1);
    let status = card.status.clone().unwrap();
    assert_eq!(
        (status.status_chance, shown(&card, "status_chance")),
        (Some(60.060062408447266), Some(Num::Float(60.1)))
    );
    assert_eq!(status.status_chance_by_surface.get("generic"), Some(&60.0));
    assert_eq!(status.status_dps, None);
    assert_eq!(status.status_effect, Pv::Str("STATUS_EFFECT_Amp".into()));
    // The input clamps at 3.33: a combined modifier of 5 shows 100.
    let install = Install::new(
        "card-slag-five",
        weapon_package(typed(5.0), vec![], |p| slag(p, true)),
    );
    assert_eq!(
        card_of(&install.data(), &early(), 1)
            .status
            .unwrap()
            .status_chance,
        Some(100.0)
    );
}

#[test]
fn the_card_damage_type_reads_any_part_after_the_elemental_one() {
    let objects = |p: &mut Syn| {
        p.add(
            "Dmg.Zap",
            "WillowGame.WillowDamageTypeDefinition",
            vec![("DamageType", V::Enum("EDamageType", "DAMAGE_TYPE_Shock"))],
        );
        p.add("Dmg.Burn", "WillowGame.WillowDamageTypeDefinition", vec![]);
        p.add("Elem.Plain", "WillowGame.WeaponPartDefinition", vec![]);
        p.add(
            "Elem.Part_Fire",
            "WillowGame.WeaponPartDefinition",
            vec![("CustomDamageTypeDefinition", r("Dmg.Burn"))],
        );
    };
    let body = vec![("CustomDamageTypeDefinition", r("Dmg.Zap"))];
    let install = Install::new("card-dmg", weapon_package(vec![], body, objects));
    let data = install.data();
    // The body's custom damage type shows on the card (the card's any-part rule; the name rule reads only the
    // elemental part, W4-ROLL).
    let card = card_of(&data, &early().with_part("Elemental", "Elem.Plain"), 1);
    assert_eq!(
        (card.damage_type.as_deref(), card.element.as_deref()),
        (Some("Dmg.Zap"), Some("Shock"))
    );
    // An elemental part's own comes first; an element the enum does not name falls back to the part's suffix.
    let card = card_of(&data, &early().with_part("Elemental", "Elem.Part_Fire"), 1);
    assert_eq!(
        (card.damage_type.as_deref(), card.element.as_deref()),
        (Some("Dmg.Burn"), Some("Fire"))
    );
}

#[test]
fn the_zoom_line_and_the_ammo_line() {
    let type_extra = vec![
        ("ZoomedEndFOV", V::Float(40.0)),
        (
            "WeaponCardAttributes",
            V::Array(vec![V::Struct(
                "WeaponCardAttributeData",
                vec![("Attribute", r("D_Attributes.Weapon.WeaponZoomEndFOV"))],
            )]),
        ),
        (
            "WeaponAttributeEffects",
            V::Array(vec![effect(
                "D_Attributes.Weapon.WeaponShotCost",
                "MT_PreAdd",
                konst(1.0),
            )]),
        ),
    ];
    let install = Install::new(
        "card-zoom",
        weapon_package(type_extra, vec![], |p| {
            p.add(
                ZOOM_PRESENTATION,
                "WillowGame.AttributePresentationDefinition",
                vec![(
                    "RoundingMode",
                    V::Enum("EAttributeRounding", "ATTRROUNDING_Float"),
                )],
            );
        }),
    );
    let card = card_of(&install.data(), &early(), 1);
    // 80 / 40 = 2; the shot cost 1 + 1 = 2 gives the ammo line; the red line comes first.
    assert_eq!(card.weapon_zoom, Some(2.0));
    assert_eq!(card.shot_cost, Some(2.0));
    let texts: Vec<&str> = card.text_lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        texts,
        vec!["Hot stuff", "2.0x Weapon Zoom", "Consumes 2 ammo per shot."]
    );
}

#[test]
fn presentation_rounding_rules() {
    // Half up in single precision: 33.349998 (a float tie at x10) prints 33.4; 1.7499998 prints 1.7.
    assert_eq!(
        present(f64::from(33.35f32), "ATTRROUNDING_Float", 1).unwrap(),
        Num::Float(33.4)
    );
    assert_eq!(
        present(f64::from(1.7499998f32), "ATTRROUNDING_Float", 1).unwrap(),
        Num::Float(1.7)
    );
    assert_eq!(
        present(2.01, "ATTRROUNDING_IntCeil", 1).unwrap(),
        Num::Int(3)
    );
    assert_eq!(
        present(2.99, "ATTRROUNDING_IntFloor", 1).unwrap(),
        Num::Int(2)
    );
    assert_eq!(
        present(2.5, "ATTRROUNDING_IntRound", 1).unwrap(),
        Num::Int(3)
    );
    // Under 1e-8 is 0; the precision clamps to 0..10.
    assert_eq!(
        present(-1e-9, "ATTRROUNDING_IntCeil", 1).unwrap(),
        Num::Int(0)
    );
    assert_eq!(
        present(2.25, "ATTRROUNDING_Float", 0).unwrap(),
        Num::Float(2.0)
    );
    assert_eq!(
        present(2.25, "ATTRROUNDING_Float", -3).unwrap(),
        Num::Float(2.0)
    );
    assert!(present(f64::INFINITY, "ATTRROUNDING_IntCeil", 1).is_err());
    // The remap: clamped input, an empty range gives the output low.
    assert_eq!(remap_value([0.0, 10.0, 0.0, 100.0], 30.0), 100.0);
    assert_eq!(remap_value([0.0, 10.0, 0.0, 100.0], -3.0), 0.0);
    assert_eq!(remap_value([5.0, 5.0, 7.0, 0.0], 1.0), 7.0);
}

#[test]
fn game_text_is_plain_and_markup_keeps_element_tags() {
    assert_eq!(
        plain_game_text("[shock]Zap[-shock] it;  now <b>x</b> &amp; [skill]"),
        "Zap it, now x &"
    );
    assert_eq!(plain_game_text("  \u{1c}a\u{a0}\tb "), "a b");
    assert_eq!(
        markup_text(
            "Deals [fire]heat[-fire] & [style]x[-style] [y]",
            "#FFFFFF",
            "#FF0000"
        ),
        "Deals <font color=\"#FF0000\">heat</font> &amp; <font color=\"#FFFFFF\">x</font> "
    );
    assert_eq!(
        html_unescape("&lt;a&gt; &quot;b&quot; &#65;&#x42; &amp &unknown;"),
        "<a> \"b\" AB & &unknown;"
    );
}

#[test]
fn localization_files_override_card_text() {
    let install = Install::new(
        "card-int",
        weapon_package(vec![], vec![], |p| {
            p.add("Pres", "Core.Package", vec![]);
        }),
    );
    let data = install.data();
    let stats = StatsData::new(&data);
    let dir = install
        .0
        .join("WillowGame")
        .join("Localization")
        .join("INT");
    std::fs::create_dir_all(&dir).unwrap();
    let text = "[Red AttributePresentationDefinition]\r\nDescription=\"Cold stuff\"\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend(u.to_le_bytes());
    }
    std::fs::write(dir.join("Pres.int"), bytes).unwrap();
    let l10n = Localization::new(&install.0);
    assert_eq!(localization_key(&stats, "Pres.Red").unwrap(), "Red");
    assert_eq!(
        l10n.get(&stats, "Pres.Red", "Description")
            .unwrap()
            .as_deref(),
        Some("Cold stuff")
    );
    let card = weapon_card(&stats, &early(), 1, &CardText::new(RED, Some(&l10n))).unwrap();
    assert_eq!(card.to_pv().at("fun_stats"), &Pv::Str("Cold stuff".into()));
}
