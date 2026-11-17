//! Tests of `AdjustDamage`'s stages and the damage type natives on the invented packages of `crate::run::fixture`
//! (AGENTS.md rule 9). The whole armored-pawn reports are compared with the C++ in `ow-cli`'s `cmd/combat` tests;
//! here each rule on its own, worked by hand.

use ow_core::{Missing, ParityMode};
use ow_object::value::Value;

use super::*;
use crate::hit::access::set;
use crate::run::tests::{def, fresh, make};

/// The `applied` of each hit of a run (`hit.<class>.L<level>.<k>.applied` rows).
fn applied(mode: ParityMode, steps: &[&str]) -> (String, bool) {
    crate::run::tests::run_steps(mode, steps)
}

#[test]
fn the_level_difference_takes_the_higher_or_lower_scale_clamped_to_the_table() {
    // Player 8 against a level 4 pawn: |d| = 4, the table's one row, the higher attacker's 1.25; the toy's undeclared
    // attributes read as 1 (Reference). 100 x 1.25 / (1 + 0.25) x 0.5 (flesh) = 50.
    let (text, failed) = applied(
        ParityMode::Reference,
        &[
            "spawn:ToyPawns/Pawn_Armored:4",
            "player:8",
            "hit:0:100:DamageType_Toy:Grenade",
        ],
    );
    assert!(!failed, "{text}");
    assert!(
        text.contains("AdjustDamage level difference attacker=8 victim=4 scale=1.25"),
        "{text}"
    );
    assert!(text.contains("\"hit.Armored.L4.1.applied\":50"), "{text}");
    // Player 2 against level 4: the lower attacker's 0.75 -> 100 x 0.75 / 1.25 x 0.5 = 30.
    let (text, _) = applied(
        ParityMode::Reference,
        &[
            "spawn:ToyPawns/Pawn_Armored:4",
            "player:2",
            "hit:0:100:DamageType_Toy:Grenade",
        ],
    );
    assert!(text.contains("scale=0.75"), "{text}");
    assert!(text.contains("\"hit.Armored.L4.1.applied\":30"), "{text}");
}

#[test]
fn the_damage_source_multiplies_the_instigators_attribute() {
    // A bullet with InstigatedBulletDamageModifier 1.2: 100 x 1.25 x 1.2 / 1.25 x 0.5 = 60.
    let (text, failed) = applied(
        ParityMode::Reference,
        &[
            "spawn:ToyPawns/Pawn_Armored:4",
            "player:8",
            "attr:0:InstigatedBulletDamageModifier:1.2",
            "hit:0:100:DamageType_Toy:Bullet",
        ],
    );
    assert!(!failed, "{text}");
    assert!(text.contains("\"hit.Armored.L4.1.applied\":60"), "{text}");
    // Off stops earlier at the unread Willow hold virtual, before the attribute stage.
    let (text, failed) = applied(
        ParityMode::Off,
        &[
            "spawn:ToyPawns/Pawn_Armored:4",
            "player:8",
            "hit:0:100:DamageType_Toy:Bullet",
        ],
    );
    assert!(failed);
    assert!(
        text.contains(
            "AI hold virtual overrides"
        ),
        "{text}"
    );
}

#[test]
fn an_ai_instigator_takes_the_global_ai_damage_scale() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Armored:4");
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    let victim = run.enemy(0).unwrap().pawn;
    let attacker = run.enemy(1).unwrap().mind;
    let vm = &mut run.world().vm;
    let grenade = vm.find_class("WillowGame.WillowDmgSource_Grenade").unwrap();
    let t = def(vm, "DamageType_Toy");
    let r = crate::hit::hit_with_type(vm, victim, Some(attacker), 100.0, Some(t), Some(grenade))
        .unwrap();
    // 100 x 0.5 (GlobalAIDamageScale) x 1 (equal levels) / 1.25 x 0.5 = 20.
    assert!(
        r.events
            .iter()
            .any(|e| e == "AdjustDamage AI damage scale=0.5"),
        "{:?}",
        r.events
    );
    assert_eq!(r.applied, 20.0);
}

#[test]
fn intrinsic_armor_divides_by_one_plus_a_hundredth() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Armored:4");
    run.apply("player:4");
    let (victim, controller) = (
        run.enemy(0).unwrap().pawn,
        run.player(0).unwrap().controller,
    );
    let vm = &mut run.world().vm;
    let grenade = vm.find_class("WillowGame.WillowDmgSource_Grenade").unwrap();
    set(vm, victim, "IntrinsicArmor", Value::Float(100.0)).unwrap();
    let r =
        crate::hit::hit_with_type(vm, victim, Some(controller), 80.0, None, Some(grenade)).unwrap();
    assert_eq!(r.applied, 40.0, "80 / (1 + 1)");
}

#[test]
fn the_surface_modifier_by_surface() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let t = def(vm, "DamageType_Toy");
    assert_eq!(
        surface_modifier(vm, t, 1, None).unwrap(),
        0.5,
        "flesh: the data's 0.5"
    );
    assert_eq!(
        surface_modifier(vm, t, 2, None).unwrap(),
        1.0,
        "armor: the toy class declares no field"
    );
    assert_eq!(
        surface_modifier(vm, t, 7, None).unwrap(),
        1.0,
        "another surface"
    );
    let native = vm
        .find_function("Engine.DamageTypeDefinition.GetSurfaceDamageTypeModifier")
        .unwrap();
    let r = vm
        .call(native, Some(t), vec![Value::Byte(1), Value::Object(None)])
        .unwrap();
    assert_eq!(r.ret, Value::Float(0.5));
}

#[test]
fn a_resistance_of_zero_with_a_base_of_zero_is_one_only_in_reference_mode() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, _dir) = fresh(mode);
        let vm = &mut run.world().vm;
        let pawn = make(vm, "WillowGame.WillowArmoredPawn");
        let resist = def(vm, "DamageType_Resist");
        let plain = def(vm, "DamageType_Toy");
        assert_eq!(
            resistance(vm, pawn, plain).unwrap(),
            1.0,
            "a type naming no attribute"
        );
        match (mode, resistance(vm, pawn, resist)) {
            (ParityMode::Reference, Ok(v)) => assert_eq!(v, 1.0),
            (ParityMode::Off, Err(VmError::NotYet(n))) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.to_string().contains("ResistToy"), "{n}");
            }
            (m, other) => panic!("{m:?}: {other:?}"),
        }
        // A value the pawn holds is read as it is, in both modes.
        set(vm, pawn, "ResistToy", Value::Float(0.4)).unwrap();
        assert_eq!(resistance(vm, pawn, resist).unwrap(), 0.4);
        set(vm, pawn, "ResistToy", Value::Float(0.0)).unwrap();
        set(vm, pawn, "ResistToyBaseValue", Value::Float(2.0)).unwrap();
        assert_eq!(
            resistance(vm, pawn, resist).unwrap(),
            0.0,
            "0 with a base that is not 0 is 0"
        );
        let native = vm
            .find_function("Engine.DamageTypeDefinition.GetPawnDamageTypeModifier")
            .unwrap();
        let r = vm
            .call(native, Some(resist), vec![Value::Object(None)])
            .unwrap();
        assert_eq!(r.ret, Value::Float(1.0), "no pawn");
    }
}

#[test]
fn the_critical_bonus_of_zero_with_base_zero_needs_the_real_game_in_off_mode() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let controller = make(vm, "WillowGame.WillowPlayerController");
    match crit_bonus(vm, controller, false) {
        Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NeedsRealGame(8)),
        other => panic!("{other:?}"),
    }
    set(
        vm,
        controller,
        "CurrentInstantHitCriticalHitBonus",
        Value::Float(2.0),
    )
    .unwrap();
    assert_eq!(crit_bonus(vm, controller, false).unwrap(), 2.0);
    // The melee bonus: the toy class does not declare it, so Off is NotYet { NotRead } (attribute rule).
    assert!(matches!(
        crit_bonus(vm, controller, true),
        Err(VmError::NotYet(_))
    ));
    let (mut run, _dir) = fresh(ParityMode::Reference);
    let vm = &mut run.world().vm;
    let controller = make(vm, "WillowGame.WillowPlayerController");
    assert_eq!(
        crit_bonus(vm, controller, false).unwrap(),
        0.0,
        "the C++ reads the stored 0"
    );
    assert_eq!(
        crit_bonus(vm, controller, true).unwrap(),
        1.0,
        "an undeclared one reads as 1"
    );
}

#[test]
fn a_critical_region_multiplies_the_bonus_and_its_impact_response() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Bodied:4");
    run.apply("player:4");
    let (victim, controller) = (
        run.enemy(0).unwrap().pawn,
        run.player(0).unwrap().controller,
    );
    let vm = &mut run.world().vm;
    // The default region (no bone): x1.0, flesh -> 100 x 0.5 = 50.
    let grenade = vm.find_class("WillowGame.WillowDmgSource_Grenade").unwrap();
    let t = def(vm, "DamageType_Toy");
    let r = crate::hit::hit_with_type(vm, victim, Some(controller), 100.0, Some(t), Some(grenade))
        .unwrap();
    assert_eq!(r.applied, 50.0);
    assert!(
        r.events
            .iter()
            .any(|e| e == "AdjustDamage surface=1 modifier=0.5"),
        "{:?}",
        r.events
    );
}

#[test]
fn the_source_kind_is_the_last_table_class_the_source_derives_from() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let bullet = vm.find_class("WillowGame.WillowDmgSource_Bullet").unwrap();
    let melee = vm.find_class("WillowGame.WillowDmgSource_Melee").unwrap();
    assert_eq!(source_kind(vm, Some(bullet)), Some(0));
    assert_eq!(source_kind(vm, Some(melee)), Some(1));
    assert_eq!(source_kind(vm, None), None);
    let other = vm.find_class("WillowGame.WillowDamagePipeline").unwrap();
    assert_eq!(source_kind(vm, Some(other)), None);
}

#[test]
fn a_stage_left_out_is_not_yet_in_off_mode_and_a_note_in_reference_mode() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    match left_out(vm, None, "a stage", Gap::NotPorted) {
        Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NotPorted),
        other => panic!("{other:?}"),
    }
    match left_out(vm, Some("k"), "a datum", Gap::NotRead) {
        Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NotRead),
        other => panic!("{other:?}"),
    }
    let (mut run, _dir) = fresh(ParityMode::Reference);
    let vm = &mut run.world().vm;
    left_out(vm, None, "every time", Gap::NotPorted).unwrap();
    left_out(vm, None, "every time", Gap::NotPorted).unwrap();
    left_out(vm, Some("k"), "once", Gap::NotRead).unwrap();
    left_out(vm, Some("k"), "once", Gap::NotRead).unwrap();
    let events = crate::hit::with_state(vm, |s| s.events.clone()).unwrap();
    assert_eq!(
        events,
        ["note: every time", "note: every time", "note: once"]
    );
}

#[test]
fn an_ai_bullet_refuses_unread_willow_hold_before_weapon_scales() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, _dir) = fresh(mode);
        run.apply("spawn:ToyPawns/Pawn_Armored:4");
        run.apply("spawn:ToyPawns/Pawn_Toy:4");
        let victim = run.enemy(0).unwrap().pawn;
        let attacker = run.enemy(1).unwrap().mind;
        let vm = &mut run.world().vm;
        let bullet = vm.find_class("WillowGame.WillowDmgSource_Bullet").unwrap();
        let r = crate::hit::hit_with_type(vm, victim, Some(attacker), 100.0, None, Some(bullet));
        match (mode, r) {
            (ParityMode::Off, Err(VmError::NotYet(n))) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.to_string().contains("AI hold virtual overrides"), "{n}");
            }
            (ParityMode::Reference, Ok(r)) => assert!(
                r.events.iter().any(|e| e
                    == "note: AdjustDamage stage 1: the AI weapon scales of bullet and rocket damage are not implemented"),
                "{:?}",
                r.events
            ),
            (m, other) => panic!("{m:?}: {other:?}"),
        }
    }
}
