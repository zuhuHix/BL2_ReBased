//! Tests of the held weapon's steps and of `crate::hit::shot` on invented packages (AGENTS.md rule 9): two packages
//! beside the combat fixture, `ToyWeaponGame` (toy classes with the property names the shot reads) and `ToyWeapons`
//! (a toy weapon type, parts, firing modes and impacts). Every name and value is invented; the expected values are the
//! rules worked by hand. The combat fixture itself is not changed (its run output is pinned against the C++ in
//! `ow-cli`).

use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{PackageBuilder, ints};
use ow_vm::error::VmError;

use super::super::tests::{Dir, def, make, world_with};
use super::Run;
use crate::hit::access::{array, set};
use crate::hit::shot::{self, WeaponParts};

/// `ToyWeaponGame`: the toy classes.
fn toy_weapon_game() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Byte", "Object", "Int", "Float", "Bool", "Class", "Array"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let root = t.p.add_import("Core", "Class", engine, "Object");
    let prop = |t: &mut ScriptToy, kind: &str, owner: i32, name: &str| -> i32 {
        let tail = match kind {
            "Object" => ints(&[root]),
            "Class" => ints(&[root, root]),
            _ => ints(&[0]),
        };
        t.prop_tail(kind, owner, name, 0, &tail)
    };
    let array = |t: &mut ScriptToy, owner: i32, name: &str, inner: &str| {
        let index = t.p.next_export();
        t.prop("Array", owner, name, 0, index + 1);
        prop(t, inner, index, name);
    };
    let weapon = t.class("ToyWeapon", root);
    prop(&mut t, "Float", weapon, "InstantHitDamage");
    prop(&mut t, "Int", weapon, "ProjectilesPerShot");
    prop(&mut t, "Byte", weapon, "CurrentFireMode");
    array(&mut t, weapon, "InstantHitDamageTypes", "Class");
    array(&mut t, weapon, "InstantHitDamageTypeDefinitions", "Object");
    array(&mut t, weapon, "InstantHitImpactDefinitions", "Object");
    let kind = t.class("ToyWeaponType", root);
    prop(&mut t, "Class", kind, "InstantHitDamageType");
    for n in ["DefaultDamageTypeDefinition", "WeaponImpact", "DefaultFiringModeDefinition"] {
        prop(&mut t, "Object", kind, n);
    }
    let part = t.class("ToyWeaponPart", root);
    for n in ["CustomDamageTypeDefinition", "WeaponImpact", "CustomFiringModeDefinition"] {
        prop(&mut t, "Object", part, n);
    }
    let mode = t.class("ToyFiringMode", root);
    prop(&mut t, "Byte", mode, "FireType");
    prop(&mut t, "Object", mode, "DamageTypeDefinition");
    prop(&mut t, "Bool", mode, "bSuppressImpactEffects");
    array(&mut t, mode, "ImpactResponses", "Int");
    array(&mut t, mode, "OnAnyImpact", "Object");
    t.class("ToyImpact", root);
    t.build()
}

fn payload(p: &mut PackageBuilder, tags: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = ints(&[0]);
    for t in tags {
        out.extend(t);
    }
    out.extend_from_slice(&p.none());
    out
}

/// `ToyWeapons`: the toy weapon type and its data (module documentation).
fn toy_weapons() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let game = p.add_import("Core", "Package", 0, "ToyWeaponGame");
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let pawns = p.add_import("Core", "Package", 0, "ToyPawns");
    let class = |p: &mut PackageBuilder, name: &str| p.add_import("Core", "Class", game, name);
    let (type_class, part_class, mode_class, impact_class) = (
        class(&mut p, "ToyWeaponType"),
        class(&mut p, "ToyWeaponPart"),
        class(&mut p, "ToyFiringMode"),
        class(&mut p, "ToyImpact"),
    );
    let bullet = p.add_import("Core", "Class", willow, "WillowDmgSource_Bullet");
    let damage_type = p.add_import("WillowGame", "DamageTypeDefinition", pawns, "DamageType_Toy");
    let type_definition = p.add_import("Core", "Class", willow, "DamageTypeDefinition");
    let none = payload(&mut p, Vec::new());
    let impact = p.add_export(impact_class, "Impact_Toy", none.clone());
    let impact_shock = p.add_export(impact_class, "Impact_Shock", none.clone());
    let shock = p.add_export(type_definition, "DamageType_Shock", none.clone());
    let mode = |p: &mut PackageBuilder, name: &str, fire_type: u8, extra: Vec<Vec<u8>>| {
        let mut tags = vec![p.byte_tag("FireType", fire_type)];
        tags.extend(extra);
        let body = payload(p, tags);
        p.add_export(mode_class, name, body)
    };
    let bullet_mode = mode(&mut p, "Mode_Bullet", 0, Vec::new());
    let beam = mode(&mut p, "Mode_Beam", 1, Vec::new());
    let responses = {
        let tag = p.array_tag("ImpactResponses", 1, &ints(&[3]));
        mode(&mut p, "Mode_Responses", 3, vec![tag])
    };
    let suppress = {
        let tag = p.bool_tag("bSuppressImpactEffects", true);
        mode(&mut p, "Mode_Suppress", 3, vec![tag])
    };
    let own_type = {
        let tag = p.object_tag("DamageTypeDefinition", shock);
        mode(&mut p, "Mode_OwnType", 0, vec![tag])
    };
    let rocket = mode(&mut p, "Mode_Rocket", 2, Vec::new());
    let any_impact = {
        let tag = p.array_tag("OnAnyImpact", 1, &ints(&[impact]));
        mode(&mut p, "Mode_AnyImpact", 0, vec![tag])
    };
    let tags = vec![
        p.tag("InstantHitDamageType", "ClassProperty", &ints(&[bullet])),
        p.object_tag("DefaultDamageTypeDefinition", damage_type),
        p.object_tag("WeaponImpact", impact),
        p.object_tag("DefaultFiringModeDefinition", bullet_mode),
    ];
    let body = payload(&mut p, tags);
    p.add_export(type_class, "Type_Toy", body);
    // A type with no default damage type: Off's damage-surface stage refuses a hit that carries one and names no
    // region, which the end-to-end test does not want to reach.
    let tags = vec![
        p.tag("InstantHitDamageType", "ClassProperty", &ints(&[bullet])),
        p.object_tag("WeaponImpact", impact),
        p.object_tag("DefaultFiringModeDefinition", bullet_mode),
    ];
    let body = payload(&mut p, tags);
    p.add_export(type_class, "Type_Plain", body);
    // A type with no firing mode at all (review r1 of #393: no shot is read for it).
    let tags = vec![
        p.tag("InstantHitDamageType", "ClassProperty", &ints(&[bullet])),
        p.object_tag("WeaponImpact", impact),
    ];
    let body = payload(&mut p, tags);
    p.add_export(type_class, "Type_NoMode", body);
    let tags = vec![
        p.object_tag("CustomDamageTypeDefinition", shock),
        p.object_tag("WeaponImpact", impact_shock),
    ];
    let body = payload(&mut p, tags);
    p.add_export(part_class, "Part_Shock", body);
    for (name, m) in [
        ("Barrel_Beam", beam),
        ("Barrel_Responses", responses),
        ("Barrel_Suppress", suppress),
        ("Barrel_OwnType", own_type),
        ("Barrel_Rocket", rocket),
        ("Barrel_AnyImpact", any_impact),
        // Not a firing mode: an impact object, whose class declares none of a mode's properties (bad data).
        ("Barrel_NotAMode", impact),
    ] {
        let tags = vec![p.object_tag("CustomFiringModeDefinition", m)];
        let body = payload(&mut p, tags);
        p.add_export(part_class, name, body);
    }
    // An elemental part with its own firing mode: the elemental step NATIVE_WEAPON_GET_FIRING_MODE.md settles.
    let tags = vec![p.object_tag("CustomFiringModeDefinition", beam)];
    let body = payload(&mut p, tags);
    p.add_export(part_class, "Part_ElementalMode", body);
    // A part with neither override: the type's damage type, impact and firing mode stay.
    let body = payload(&mut p, Vec::new());
    p.add_export(part_class, "Part_Plain", body);
    p.build()
}

/// A run over the combat fixture plus the two weapon packages, in `mode`.
fn armed_run(mode: ParityMode) -> (Run, Dir) {
    let (w, dir) = world_with(
        mode,
        vec![
            ("ToyWeaponGame.upk", toy_weapon_game()),
            ("ToyWeapons.upk", toy_weapons()),
        ],
    );
    (Run::new(w).unwrap(), dir)
}

/// A toy weapon instance with `damage` and one projectile, as W4-STATS leaves the bases (invented values).
fn toy_weapon(run: &mut Run, damage: f32) -> ObjRef {
    let vm = &mut run.world().vm;
    let weapon = make(vm, "ToyWeaponGame.ToyWeapon");
    set(vm, weapon, "InstantHitDamage", Value::Float(damage)).unwrap();
    set(vm, weapon, "ProjectilesPerShot", Value::Int(1)).unwrap();
    weapon
}

/// The weapon parts: the toy type and the given elemental and barrel parts of `ToyWeapons`.
fn parts(run: &mut Run, elemental: Option<&str>, barrel: Option<&str>) -> WeaponParts {
    parts_of(run, "Type_Toy", elemental, barrel)
}

/// [`parts`] with the weapon type `weapon_type` of `ToyWeapons`.
fn parts_of(run: &mut Run, weapon_type: &str, elemental: Option<&str>, barrel: Option<&str>) -> WeaponParts {
    let vm = &mut run.world().vm;
    let weapon_type = def(vm, &format!("ToyWeapons.{weapon_type}"));
    let elemental = elemental.map(|n| def(vm, &format!("ToyWeapons.{n}")));
    let barrel = barrel.map(|n| def(vm, &format!("ToyWeapons.{n}")));
    WeaponParts::new(weapon_type, elemental, barrel)
}

/// The object `v` holds, and the fixture object at `path`.
fn same(run: &mut Run, v: &Value, path: &str) -> bool {
    let vm = &mut run.world().vm;
    crate::hit::access::object_of(v) == Some(def(vm, path))
}

fn class_path(run: &mut Run, v: &Value) -> String {
    let Value::Class(Some(c)) = v else {
        return format!("{v:?}");
    };
    let vm = &run.world().vm;
    vm.space.classes.get_layout(*c).map(|l| l.path().to_string()).unwrap()
}

#[test]
fn the_fire_mode_setup_follows_the_script_and_the_damage_type_rule() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        // No elemental part: the type's source, default damage type and impact; an empty definitions array grows to one.
        let (mut run, _dir) = armed_run(mode);
        let weapon = toy_weapon(&mut run, 10.0);
        let p = parts(&mut run, None, None);
        run.hold_weapon(weapon, &p).unwrap();
        let vm = &mut run.world().vm;
        let sources = array(vm, weapon, "InstantHitDamageTypes").unwrap();
        let types = array(vm, weapon, "InstantHitDamageTypeDefinitions").unwrap();
        let impacts = array(vm, weapon, "InstantHitImpactDefinitions").unwrap();
        assert_eq!((sources.len(), types.len(), impacts.len()), (1, 1, 1));
        assert_eq!(class_path(&mut run, &sources[0]), "WillowGame.WillowDmgSource_Bullet");
        assert!(same(&mut run, &types[0], "ToyPawns.DamageType_Toy"));
        assert!(same(&mut run, &impacts[0], "ToyWeapons.Impact_Toy"));

        // An elemental part with a custom damage type and its own impact: both replace the type's.
        let (mut run, _dir) = armed_run(mode);
        let weapon = toy_weapon(&mut run, 10.0);
        let p = parts(&mut run, Some("Part_Shock"), None);
        run.hold_weapon(weapon, &p).unwrap();
        let vm = &mut run.world().vm;
        let types = array(vm, weapon, "InstantHitDamageTypeDefinitions").unwrap();
        let impacts = array(vm, weapon, "InstantHitImpactDefinitions").unwrap();
        assert!(same(&mut run, &types[0], "ToyWeapons.DamageType_Shock"));
        assert!(same(&mut run, &impacts[0], "ToyWeapons.Impact_Shock"));

        // An elemental part with neither: the type's stay (the native reads only the elemental part, then the type).
        let (mut run, _dir) = armed_run(mode);
        let weapon = toy_weapon(&mut run, 10.0);
        let p = parts(&mut run, Some("Part_Plain"), Some("Part_Plain"));
        run.hold_weapon(weapon, &p).unwrap();
        let vm = &mut run.world().vm;
        let types = array(vm, weapon, "InstantHitDamageTypeDefinitions").unwrap();
        let impacts = array(vm, weapon, "InstantHitImpactDefinitions").unwrap();
        assert!(same(&mut run, &types[0], "ToyPawns.DamageType_Toy"));
        assert!(same(&mut run, &impacts[0], "ToyWeapons.Impact_Toy"));
        let vm = &mut run.world().vm;
        let mode_of = shot::firing_mode(vm, &p).unwrap();
        assert_eq!(mode_of, Some(def(vm, "ToyWeapons.Mode_Bullet")));
    }
}

#[test]
fn a_shot_hands_the_chain_the_weapons_damage_source_type_impact_and_causer() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, _dir) = armed_run(mode);
        let weapon = toy_weapon(&mut run, 12.5);
        let p = parts(&mut run, Some("Part_Shock"), None);
        run.hold_weapon(weapon, &p).unwrap();
        let held_mode = run.held.unwrap().1;
        let vm = &mut run.world().vm;
        let s = shot::shot(vm, weapon, held_mode).unwrap();
        assert_eq!(s.damage, 12.5);
        assert_eq!(s.fire_type, 0);
        assert_eq!(s.damage_type, Some(def(vm, "ToyWeapons.DamageType_Shock")));
        assert_eq!(s.parts.impact, Some(def(vm, "ToyWeapons.Impact_Shock")));
        assert_eq!(s.parts.causer, Some(weapon));
        assert_eq!(
            vm.space.classes.get_layout(s.source).unwrap().path().to_string(),
            "WillowGame.WillowDmgSource_Bullet"
        );
    }
}

/// The `NotYet` a shot from `weapon` with barrel `barrel` answers, with its missing kind.
fn refusal(mode: ParityMode, barrel: Option<&str>, edit: impl FnOnce(&mut Run, ObjRef)) -> (String, Missing) {
    let (mut run, _dir) = armed_run(mode);
    let weapon = toy_weapon(&mut run, 10.0);
    let p = parts(&mut run, None, barrel);
    run.hold_weapon(weapon, &p).unwrap();
    edit(&mut run, weapon);
    let held_mode = run.held.unwrap().1;
    match shot::shot(&mut run.world().vm, weapon, held_mode) {
        Err(VmError::NotYet(n)) => (n.to_string(), n.missing.clone()),
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

#[test]
fn a_shot_refuses_what_the_notes_do_not_read_in_both_modes() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (text, missing) = refusal(mode, Some("Barrel_Beam"), |_, _| {});
        assert!(text.contains("a Beam shot"), "{text}");
        assert!(matches!(missing, Missing::NotPorted), "{missing:?}");
        let (text, missing) = refusal(mode, Some("Barrel_Responses"), |_, _| {});
        assert!(text.contains("ImpactResponses"), "{text}");
        assert!(matches!(missing, Missing::NotRead), "{missing:?}");
        let (text, _) = refusal(mode, Some("Barrel_OwnType"), |_, _| {});
        assert!(text.contains("its own DamageTypeDefinition"), "{text}");
        let (text, missing) = refusal(mode, Some("Barrel_Rocket"), |_, _| {});
        assert!(text.contains("a Rocket shot"), "{text}");
        assert!(matches!(missing, Missing::NotPorted), "{missing:?}");
        let (text, missing) = refusal(mode, Some("Barrel_AnyImpact"), |_, _| {});
        assert!(text.contains("OnAnyImpact"), "{text}");
        assert!(matches!(missing, Missing::NotRead), "{missing:?}");
        let (text, _) = refusal(mode, None, |run, w| {
            set(&mut run.world().vm, w, "ProjectilesPerShot", Value::Int(3)).unwrap();
        });
        assert!(text.contains("each of 3 projectiles"), "{text}");
        let (text, _) = refusal(mode, None, |run, w| {
            set(&mut run.world().vm, w, "CurrentFireMode", Value::Byte(1)).unwrap();
        });
        assert!(text.contains("fire mode 1"), "{text}");
    }
}

#[test]
fn a_suppressed_impact_leaves_the_pipeline_without_one() {
    let (mut run, _dir) = armed_run(ParityMode::Off);
    let weapon = toy_weapon(&mut run, 10.0);
    let p = parts(&mut run, None, Some("Barrel_Suppress"));
    run.hold_weapon(weapon, &p).unwrap();
    let held_mode = run.held.unwrap().1;
    let s = shot::shot(&mut run.world().vm, weapon, held_mode).unwrap();
    assert_eq!(s.parts.impact, None);
    assert_eq!(s.fire_type, 3);
}

/// The text of the step result whose step is `step`, in a report.
fn result_of<'a>(report: &'a str, step: &str) -> &'a str {
    let at = report.find(&format!("\"step\":\"{step}\"")).unwrap_or_else(|| panic!("no {step} in {report}"));
    let rest = &report[at..];
    &rest[..rest.find("},{\"step\"").unwrap_or(rest.len())]
}

/// A run that spawns the toy pawn `pawn` of `ToyPawns`, makes the player, holds a 10-damage toy weapon of
/// `weapon_type` and applies `steps`. In Off the pawn's mind gets no AI component first: the fixture's component is a
/// `WillowAIComponent`, whose hold overrides are unread (`NotYet`, NATIVE_AI_HOLDS.md), and that question is the hit
/// chain's, not the shot's. Invented state.
fn fired(mode: ParityMode, pawn: &str, weapon_type: &str, steps: &[&str]) -> (String, bool) {
    let (mut run, _dir) = armed_run(mode);
    run.apply(&format!("spawn:ToyPawns/{pawn}:4"));
    run.apply("player:8");
    if mode == ParityMode::Off {
        let enemy = run.enemy(0).unwrap();
        set(&mut run.world().vm, enemy.mind, "AIComponent", Value::Object(None)).unwrap();
    }
    let weapon = toy_weapon(&mut run, 10.0);
    let p = parts_of(&mut run, weapon_type, None, None);
    run.hold_weapon(weapon, &p).unwrap();
    for s in steps {
        run.apply(s);
    }
    let failed = run.failed();
    (run.report(0.0), failed)
}

/// The `rows` of a report, one `"id":value` text each, sorted, with the shot's prefix read as the hit's.
fn rows_of(report: &str) -> Vec<String> {
    let rows = &report["{\"rows\":{".len()..report.find("},\"pawns\"").unwrap()];
    let mut out: Vec<String> = rows.split(',').map(|r| r.replace("\"fire.", "\"hit.")).collect();
    out.sort();
    out
}

#[test]
fn a_fired_shot_lands_as_the_hit_with_the_same_inputs_in_both_modes() {
    // The toy pawn in both modes; the armored one (its TakeDamage runs AdjustDamage) in Reference, where the toy
    // mind's undeclared Received* attributes read as 1 (Off refuses them, NotYet { NotRead }, for the hit as well).
    let cases = [
        (ParityMode::Off, "Pawn_Toy", "Type_Toy", "hit:0:10:ToyPawns.DamageType_Toy:Bullet"),
        (ParityMode::Reference, "Pawn_Toy", "Type_Toy", "hit:0:10:ToyPawns.DamageType_Toy:Bullet"),
        (ParityMode::Reference, "Pawn_Armored", "Type_Plain", "hit:0:10::Bullet"),
    ];
    for (mode, pawn, weapon_type, hit_step) in cases {
        let (shot_text, shot_failed) = fired(mode, pawn, weapon_type, &["fire:0"]);
        let (hit_text, hit_failed) = fired(mode, pawn, weapon_type, &[hit_step]);
        assert!(!shot_failed, "{mode:?} {pawn}: {shot_text}");
        assert!(!hit_failed, "{mode:?} {pawn}: {hit_text}");
        // The same rows under their own prefix, and the same result once the shot's own event line is taken out.
        assert_eq!(rows_of(&shot_text), rows_of(&hit_text), "{mode:?} {pawn}");
        let shot = result_of(&shot_text, "fire:0");
        let line = shot[shot.find("\"TakeDamage shot impact=").unwrap()..].split_inclusive("\",").next().unwrap();
        assert!(line.contains("impact=Impact_Toy causer=ToyWeapon_"), "{line}");
        let hit = result_of(&hit_text, hit_step);
        assert_eq!(
            shot.replace(line, "").replace("\"step\":\"fire:0\"", ""),
            hit.replace(&format!("\"step\":\"{hit_step}\""), ""),
            "{mode:?} {pawn}"
        );
        if pawn == "Pawn_Toy" {
            // It landed: the toy's TakeDamage told the victim of a 10-damage hit and set its health.
            assert!(shot.contains("NotifyTakeHit damage=10"), "{mode:?}: {shot}");
            assert!(shot.contains("Engine.Pawn.SetHealth"), "{mode:?}: {shot}");
        } else {
            // The armored toy's TakeDamage runs the damage pipeline on the shot's 10 (its toy arithmetic is not the
            // game's; the equality above is the point).
            assert!(shot.contains("AdjustDamage initial=10 "), "{shot}");
        }
    }
}

#[test]
fn fire_needs_a_player_and_a_held_weapon() {
    let (mut run, _dir) = armed_run(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    run.apply("fire:0");
    run.apply("player:8");
    run.apply("fire:0");
    run.apply("fire");
    run.apply("weapon:");
    run.apply("weapon:no/such/file.txt");
    let text = run.report(0.0);
    assert!(text.contains("a shot needs a player"), "{text}");
    assert!(text.contains("a shot needs a held weapon"), "{text}");
    assert!(text.contains("combat-run: fire:<pawn>"), "{text}");
    assert!(text.contains("combat-run: weapon:<weapon file>"), "{text}");
    assert!(text.contains("the weapon file no/such/file.txt"), "{text}");
    // The weapon step names no pawn: no state is added after it.
    assert!(!result_of(&text, "weapon:no/such/file.txt").contains("\"state\""), "{text}");
}

/// Review r1 of #393: no firing mode is no shot read (`NotYet { NotRead }`, the records disagree); a mode of the
/// wrong class is bad data. Lane IMPL-GETFIRINGMODEDEFINITION: in Off, an elemental part with its own mode and no
/// barrel mode answers the elemental part's mode (NATIVE_WEAPON_GET_FIRING_MODE.md section 2, the shared rule); in
/// Reference it is still refused, as before (review of #735).
#[test]
fn no_firing_mode_and_a_wrong_class_are_refused_and_the_elemental_mode_answers() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, _dir) = armed_run(mode);
        let weapon = toy_weapon(&mut run, 10.0);
        let p = parts_of(&mut run, "Type_NoMode", None, None);
        run.hold_weapon(weapon, &p).unwrap();
        assert_eq!(run.held.unwrap().1, None);
        match shot::shot(&mut run.world().vm, weapon, None) {
            Err(VmError::NotYet(n)) => {
                assert!(matches!(n.missing, Missing::NotRead), "{n}");
                assert!(n.to_string().contains("no firing mode"), "{n}");
            }
            other => panic!("{mode:?}: expected a NotYet, got {other:?}"),
        }

        let (mut run, _dir) = armed_run(mode);
        let p = parts(&mut run, Some("Part_ElementalMode"), None);
        let vm = &mut run.world().vm;
        match mode {
            ParityMode::Off => assert_eq!(shot::firing_mode(vm, &p).unwrap(), Some(def(vm, "ToyWeapons.Mode_Beam"))),
            _ => match shot::firing_mode(vm, &p) {
                Err(VmError::NotYet(n)) => assert!(n.to_string().contains("elemental part"), "{n}"),
                other => panic!("{mode:?}: expected the old NotYet, got {other:?}"),
            },
        }
        // A barrel mode settles it whatever the elemental part holds: both records take the barrel's first.
        let p = parts(&mut run, Some("Part_ElementalMode"), Some("Barrel_Suppress"));
        let vm = &mut run.world().vm;
        assert_eq!(shot::firing_mode(vm, &p).unwrap(), Some(def(vm, "ToyWeapons.Mode_Suppress")));

        let (mut run, _dir) = armed_run(mode);
        let weapon = toy_weapon(&mut run, 10.0);
        let p = parts(&mut run, None, Some("Barrel_NotAMode"));
        run.hold_weapon(weapon, &p).unwrap();
        let held_mode = run.held.unwrap().1;
        let e = shot::shot(&mut run.world().vm, weapon, held_mode).unwrap_err();
        assert!(!matches!(e, VmError::NotYet(_)), "{e:?}");
        assert!(crate::hit::access::error_text(&e).contains("has no property FireType"), "{e:?}");
    }
}

/// Review r1 of #393: the causer takes the declared `DamageCauser` kind (the installed `WillowPawn` declares an
/// interface, the combat fixture's toy pawns an object); another kind gets its zero value.
#[test]
fn the_causer_follows_the_declared_parameter_kind() {
    use ow_object::layout::PropKind;
    let (mut run, _dir) = armed_run(ParityMode::Off);
    let weapon = toy_weapon(&mut run, 10.0);
    let vm = &mut run.world().vm;
    let class = vm.find_class("ToyWeaponGame.ToyWeapon").unwrap();
    let arg = |vm: &ow_vm::api::Vm, k: PropKind| crate::hit::causer_arg(vm, &k, Some(weapon));
    assert_eq!(arg(vm, PropKind::Interface { class }), Value::Interface(Some(weapon)));
    assert_eq!(arg(vm, PropKind::Object { class }), Value::Object(Some(weapon)));
    assert_eq!(arg(vm, PropKind::Int), Value::Int(0));
}

/// Lane IMPL-GETFIRINGMODEDEFINITION (NATIVE_WEAPON_GET_FIRING_MODE.md section 3, the shot helper): the shot helper and
/// the Off native's load-free rule give the same answer on the same parts once they are resident, for each of the
/// rule's steps (barrel, elemental part, type default).
#[test]
fn the_shot_helper_and_the_native_s_rule_agree_on_resident_parts() {
    use ow_inventory::equip::firing_mode::{resident, rule};
    let (mut run, _dir) = armed_run(ParityMode::Off);
    for (elemental, barrel) in [
        (None, Some("Barrel_Suppress")),
        (Some("Part_ElementalMode"), None),
        (Some("Part_Plain"), None),
        (None, None),
    ] {
        let p = parts(&mut run, elemental, barrel);
        let vm = &mut run.world().vm;
        let shot_answer = shot::firing_mode(vm, &p).unwrap();
        let native_answer = rule(vm, p.barrel, p.elemental, Some(p.weapon_type), &mut resident).unwrap();
        assert_eq!(shot_answer, native_answer, "{elemental:?} {barrel:?}");
    }
}
