//! Tests of the hit chain's natives and helpers on the invented packages of `crate::run::fixture` (AGENTS.md rule 9).
//! The natives are declared there with the signatures the records give, so every call goes through the key the
//! loader derives; the expected values are the rules worked by hand.

use ow_core::ParityMode;
use ow_object::value::Value;

use super::*;
use crate::run::tests::{def, fresh, make};

/// Calls the native at `path` on `this` with `args`.
fn native(
    vm: &mut Vm,
    path: &str,
    this: Option<ObjRef>,
    args: Vec<Value>,
) -> Result<Value, VmError> {
    let f = vm.find_function(path)?;
    Ok(vm.call(f, this, args)?.ret)
}

#[test]
fn bind_skips_a_missing_function_and_leaves_a_lower_packs_key_to_it() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    bind_fn(vm, "WillowGame.NoSuchClass.Nothing", |_| Ok(Value::None)).unwrap();
    // W5-POOLS bound SetHealth first: binding it again is left to that pack, not an error.
    bind_fn(vm, "Engine.Pawn.SetHealth", |_| Ok(Value::None)).unwrap();
    let pawn = make(vm, "WillowGame.WillowAIPawn");
    native(
        vm,
        "Engine.Pawn.SetHealth",
        Some(pawn),
        vec![Value::Float(7.0)],
    )
    .unwrap();
    assert_eq!(get(vm, pawn, "HealthVar").unwrap(), Some(Value::Float(7.0)));
}

#[test]
fn the_pipeline_pool_reuses_a_released_entry_and_fills_the_four_fields() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let actor = make(vm, "Engine.Actor");
    let t = make(vm, "WillowGame.DamageTypeDefinition");
    let args = vec![
        Value::Object(Some(t)),
        Value::Object(None),
        Value::Bool(true),
        Value::Object(Some(actor)),
    ];
    let first = object_of(
        &native(
            vm,
            "Engine.Actor.GetDamagePipeline",
            Some(actor),
            args.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        get(vm, first, "DamageTypeDef").unwrap(),
        Some(Value::Object(Some(t)))
    );
    assert_eq!(
        get(vm, first, "bCanHitFriendly").unwrap(),
        Some(Value::Bool(true))
    );
    assert_eq!(
        get(vm, first, "DirectHitObject").unwrap(),
        Some(Value::Object(Some(actor)))
    );
    let read = native(
        vm,
        "Engine.Actor.GetPipelineDamageType",
        Some(actor),
        vec![Value::Object(Some(first))],
    )
    .unwrap();
    assert_eq!(read, Value::Object(Some(t)));
    native(
        vm,
        "Engine.Actor.GetDamagePipeline",
        Some(actor),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(pipeline_counts(vm).unwrap(), (2, 2));
    native(
        vm,
        "Engine.Actor.ReleaseDamagePipeline",
        Some(actor),
        vec![Value::Object(Some(first))],
    )
    .unwrap();
    assert_eq!(pipeline_counts(vm).unwrap(), (2, 1));
    // A fresh object from the class defaults goes into the freed entry: the pool does not grow.
    let again = object_of(
        &native(
            vm,
            "Engine.Actor.GetDamagePipeline",
            Some(actor),
            Vec::new(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_ne!(again, first);
    assert_eq!(
        get(vm, again, "bCanHitFriendly").unwrap(),
        Some(Value::Bool(false))
    );
    assert_eq!(pipeline_counts(vm).unwrap(), (2, 2));
    let events = with_state(vm, |s| s.events.clone()).unwrap();
    assert_eq!(
        events.iter().filter(|e| *e == "GetDamagePipeline").count(),
        3
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| *e == "ReleaseDamagePipeline")
            .count(),
        1
    );
}

#[test]
fn damage_proceeds_with_authority_and_health_or_an_injurable_pawn() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let pawn = make(vm, "WillowGame.WillowAIPawn");
    let check = |vm: &mut Vm| {
        native(
            vm,
            "Engine.Pawn.PawnCheckTakeDamagePreconditions",
            Some(pawn),
            Vec::new(),
        )
        .unwrap()
    };
    // Role 0: aborts (true) whatever the health.
    set(vm, pawn, "HealthVar", Value::Float(5.0)).unwrap();
    assert_eq!(check(vm), Value::Bool(true));
    set(vm, pawn, "Role", Value::Byte(3)).unwrap();
    assert_eq!(check(vm), Value::Bool(false));
    set(vm, pawn, "HealthVar", Value::Float(0.0)).unwrap();
    assert_eq!(check(vm), Value::Bool(true));
    set(vm, pawn, "bCanBeInjured", Value::Bool(true)).unwrap();
    assert_eq!(check(vm), Value::Bool(false));
}

#[test]
fn take_damage_inner_stores_the_damage_and_the_tracker_adds_to_its_parts() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let pawn = make(vm, "WillowGame.WillowAIPawn");
    native(
        vm,
        "Engine.Actor.ActorTakeDamageInner",
        Some(pawn),
        vec![Value::Float(12.5)],
    )
    .unwrap();
    assert_eq!(
        get(vm, pawn, "MostRecentDamageTaken").unwrap(),
        Some(Value::Float(12.5))
    );
    let t = make(vm, "WillowGame.DamageTypeDefinition");
    let ty = vm
        .space
        .classes
        .struct_by_path("Engine.DamageTypeDefinition.RecentDamageTracker")
        .unwrap();
    let mut tracker = vm.space.classes.struct_defaults(ty).unwrap();
    for (name, v) in [
        ("TotalDamage", 1.0),
        ("HealthDamage", 2.0),
        ("ShieldDamage", 3.0),
    ] {
        assert!(set_field(vm, &mut tracker, name, Value::Float(v)));
    }
    let f = vm
        .find_function("Engine.DamageTypeDefinition.RecordRecentDamage")
        .unwrap();
    let out = vm
        .call(
            f,
            Some(t),
            vec![
                tracker,
                Value::Float(4.0),
                Value::Object(None),
                Value::Object(None),
                Value::Bool(true),
            ],
        )
        .unwrap();
    let written = &out.outs[0];
    assert_eq!(field(vm, written, "TotalDamage"), Some(Value::Float(5.0)));
    assert_eq!(field(vm, written, "HealthDamage"), Some(Value::Float(2.0)));
    assert_eq!(field(vm, written, "ShieldDamage"), Some(Value::Float(7.0)));
}

#[test]
fn the_opinion_follows_the_four_steps() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let pawn_with = |vm: &mut Vm, allegiance: Option<&str>| {
        let p = make(vm, "WillowGame.WillowAIPawn");
        let a = allegiance.map(|path| def(vm, path));
        set(vm, p, "Allegiance", Value::Object(a)).unwrap();
        p
    };
    let a = pawn_with(vm, Some("Allegiance_A"));
    let a2 = pawn_with(vm, Some("Allegiance_A"));
    let b = pawn_with(vm, Some("Allegiance_B"));
    let c = pawn_with(vm, Some("Allegiance_C"));
    let d = pawn_with(vm, Some("Allegiance_D"));
    let none = pawn_with(vm, None);
    assert_eq!(
        opinion(vm, a, Some(a2)).unwrap(),
        2,
        "the same allegiance: A's SelfOpinion"
    );
    assert_eq!(
        opinion(vm, a, Some(b)).unwrap(),
        0,
        "A's MyOpinions entry for B"
    );
    assert_eq!(
        opinion(vm, a, Some(d)).unwrap(),
        2,
        "D's OtherOpinions entry for A"
    );
    assert_eq!(
        opinion(vm, a, Some(c)).unwrap(),
        0,
        "C forces its ForcedOtherOpinion on all others"
    );
    assert_eq!(
        opinion(vm, b, Some(d)).unwrap(),
        2,
        "else B's own DefaultOpinion"
    );
    assert_eq!(
        opinion(vm, none, Some(a)).unwrap(),
        1,
        "no allegiance of its own: Neutral"
    );
    assert_eq!(
        opinion(vm, a, Some(none)).unwrap(),
        1,
        "the other has none: Neutral"
    );
    assert_eq!(opinion(vm, a, None).unwrap(), 1);
    // A controller answers through its pawn.
    let mind = make(vm, "WillowGame.WillowMind");
    set(vm, mind, "Pawn", Value::Object(Some(b))).unwrap();
    assert_eq!(opinion(vm, a, Some(mind)).unwrap(), 0);
    // The natives compare the opinion with the enum.
    assert_eq!(
        native(
            vm,
            "Engine.Pawn.IsEnemy",
            Some(a),
            vec![Value::Object(Some(b))]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        native(
            vm,
            "Engine.Pawn.IsFriendly",
            Some(a),
            vec![Value::Object(Some(a2))]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        native(
            vm,
            "Engine.Pawn.IsNeutral",
            Some(none),
            vec![Value::Object(Some(a))]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        native(
            vm,
            "Engine.Pawn.GetOpinion",
            Some(a),
            vec![Value::Object(Some(b))]
        )
        .unwrap(),
        Value::Byte(0)
    );
}

#[test]
fn the_hit_region_is_the_bones_region_else_the_default() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let pawn = make(vm, "WillowGame.WillowArmoredPawn");
    assert_eq!(
        hit_region(vm, pawn, &Value::None).unwrap(),
        None,
        "no body class: no region"
    );
    let body = def(vm, "BodyClass_Toy");
    set(vm, pawn, "BodyClass", Value::Object(Some(body))).unwrap();
    let ty = vm
        .space
        .classes
        .struct_by_path("WillowGame.WillowPawn.TraceHitInfo")
        .unwrap();
    let info = |vm: &mut Vm, bone: &str| {
        let mut v = vm.space.classes.struct_defaults(ty).unwrap();
        let name = Value::Name(vm.space.names.intern(bone));
        assert!(set_field(vm, &mut v, "BoneName", name));
        v
    };
    let head = def(vm, "Region_Head");
    let default = def(vm, "Region_Body");
    let by_head = info(vm, "head");
    assert_eq!(
        hit_region(vm, pawn, &by_head).unwrap(),
        Some(head),
        "bone names compare in any case"
    );
    let by_spine = info(vm, "Spine");
    assert_eq!(hit_region(vm, pawn, &by_spine).unwrap(), Some(default));
    let none = info(vm, "None");
    assert_eq!(hit_region(vm, pawn, &none).unwrap(), Some(default));
    let notes = with_state(vm, |s| s.events.clone()).unwrap();
    assert_eq!(
        notes
            .iter()
            .filter(|e| e.starts_with("note: hit region: the hit names no bone"))
            .count(),
        1,
        "said once"
    );
    let by_name = info(vm, "Head");
    let native_region = native(
        vm,
        "WillowGame.WillowPawn.GetHitRegionForTakenDamage",
        Some(pawn),
        vec![Value::Object(None), by_name],
    )
    .unwrap();
    assert_eq!(native_region, Value::Object(Some(head)));
}

#[test]
fn the_level_of_each_kind_of_actor() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Toy:6");
    run.apply("player:9");
    let enemy = run.enemy(0).unwrap();
    let player = run.player(0).unwrap();
    let vm = &mut run.world().vm;
    assert_eq!(level_of(vm, Some(enemy.pawn)).unwrap(), 6);
    assert_eq!(
        level_of(vm, Some(enemy.mind)).unwrap(),
        6,
        "a mind answers with its pawn's level"
    );
    assert_eq!(level_of(vm, Some(player.controller)).unwrap(), 9);
    assert_eq!(
        level_of(vm, Some(player.pawn)).unwrap(),
        9,
        "a player pawn answers through its controller"
    );
    assert_eq!(level_of(vm, None).unwrap(), 0);
}

#[test]
fn a_player_joins_the_party_list_of_the_game_replication_info() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("player:3");
    run.apply("player:5");
    let vm = &mut run.world().vm;
    let world = with_state(vm, |s| s.world).unwrap().unwrap();
    let gri = object(vm, world, "GRI").unwrap().unwrap();
    let list = access::array(vm, gri, "PRIArray").unwrap();
    assert_eq!(list.len(), 2);
    let info = object_of(&list[1]).unwrap();
    assert_eq!(get(vm, info, "ExpLevel").unwrap(), Some(Value::Int(5)));
    let owner = object(vm, info, "Owner").unwrap().unwrap();
    assert!(is_player_controller(vm, Some(owner)).unwrap());
    assert_eq!(with_state(vm, |s| s.players.len()).unwrap(), 2);
}

/// Reference: the C++ stand-in (a list of reasons, any hold).
#[test]
fn a_pawn_stays_demigod_until_every_hold_is_released() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    let pawn = run.enemy(0).unwrap().pawn;
    assert!(!demigod(&mut run.world().vm, pawn).unwrap());
    run.apply("hold:0:Safe");
    run.apply("hold:0:Other");
    run.apply("release:0:Safe");
    assert!(demigod(&mut run.world().vm, pawn).unwrap());
    run.apply("release:0:Safe");
    assert!(
        demigod(&mut run.world().vm, pawn).unwrap(),
        "a reason released twice releases nothing more"
    );
    run.apply("release:0:Other");
    assert!(!demigod(&mut run.world().vm, pawn).unwrap());
    assert!(!run.failed(), "{:?}", run.errors());
}

/// Off cannot apply the base list rule to the unread Willow virtual override.
#[test]
fn off_refuses_willow_holds_without_mutating_the_list() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    let mind = run.enemy(0).unwrap().mind;
    let component = object(&mut run.world().vm, mind, "AIComponent").unwrap().unwrap();
    for step in ["hold:0:Safe", "release:0:Safe"] {
        run.apply(step);
    }
    assert_eq!(run.errors().len(), 2, "{:?}", run.errors());
    assert!(run.errors().iter().all(|e| e.contains("AI hold virtual overrides")));
    assert!(access::array(&mut run.world().vm, component, "DemigodHolds").unwrap().is_empty());
}

#[test]
fn the_mind_demigod_flag_is_its_own() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let mind = make(vm, "WillowGame.WillowMind");
    let flag = |vm: &mut Vm| with_state(vm, |s| s.mind_demigod.get(&mind).copied()).unwrap();
    assert_eq!(flag(vm), None);
    with_state(vm, |s| s.mind_demigod.insert(mind, true)).unwrap();
    assert_eq!(flag(vm), Some(true));
}

#[test]
fn a_melee_definition_and_a_thrown_rock_deliver_from_their_data() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let melee = instantiate_copy(vm, "ToyPawns", "Melee_Toy")
        .unwrap()
        .unwrap();
    let d = delivery_of(vm, melee, None).unwrap();
    assert_eq!(d.what, "Melee_Toy");
    assert_eq!(evaluate_for(vm, &d.damage, None).unwrap(), 60.0);
    let source = vm.find_class("WillowGame.WillowDmgSource_Melee").unwrap();
    assert_eq!(d.source, Some(source));
    assert_eq!(path_of(vm, d.damage_type), "DamageType_Toy");
    let throw = instantiate_copy(vm, "ToyPawns", "Throw_Toy")
        .unwrap()
        .unwrap();
    let cause = instantiate_copy(vm, "ToyPawns", "Cause_Toy")
        .unwrap()
        .unwrap();
    let d = delivery_of(vm, throw, Some(cause)).unwrap();
    assert_eq!(d.what, "Throw_Toy + Cause_Toy");
    assert_eq!(
        evaluate_for(vm, &d.damage, None).unwrap(),
        30.0,
        "10 x (1.5 x 2.0)"
    );
    let grenade = vm.find_class("WillowGame.WillowDmgSource_Grenade").unwrap();
    assert_eq!(d.source, Some(grenade));
    let refused = |r: Result<Delivery, VmError>| access::error_text(&r.unwrap_err());
    assert_eq!(
        refused(delivery_of(vm, throw, None)),
        "combat swing: a thrown projectile needs the projectile's Behavior_CauseDamage (the second definition)"
    );
    let pawn = instantiate_copy(vm, "ToyPawns", "Pawn_Toy")
        .unwrap()
        .unwrap();
    assert_eq!(
        refused(delivery_of(vm, pawn, None)),
        "combat swing: Pawn_Toy is neither a MeleeDefinition nor a Behavior_AIThrowProjectileAtTarget"
    );
}

#[test]
fn an_archetype_copy_is_a_new_object_with_the_exports_values() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let one = instantiate_copy(vm, "ToyPawns", "Pawn_Toy")
        .unwrap()
        .unwrap();
    let two = instantiate_copy(vm, "ToyPawns", "Pawn_Toy")
        .unwrap()
        .unwrap();
    assert_ne!(one, two);
    assert_eq!(get(vm, one, "HealthVar").unwrap(), Some(Value::Float(11.0)));
    set(vm, one, "HealthVar", Value::Float(1.0)).unwrap();
    assert_eq!(
        get(vm, two, "HealthVar").unwrap(),
        Some(Value::Float(11.0)),
        "the copies do not share values"
    );
    // The AI class reference is relinked to the store's definition.
    let class = object(vm, one, "AIClass").unwrap().unwrap();
    assert_eq!(class, def(vm, "AIClass_Toy"));
    assert_eq!(
        instantiate_copy(vm, "ToyPawns", "NoSuchExport").unwrap(),
        None
    );
}

#[test]
fn not_implemented_and_an_undeclared_attribute_follow_the_parity_mode() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let mind = make(vm, "WillowGame.WillowMind");
    assert!(matches!(
        not_implemented(vm, "a thing", RECORD),
        Err(VmError::NotYet(_))
    ));
    assert!(matches!(
        attribute(vm, mind, "NoSuchAttribute", 1.0),
        Err(VmError::NotYet(_))
    ));
    let (mut run, _dir) = fresh(ParityMode::Reference);
    let vm = &mut run.world().vm;
    let mind = make(vm, "WillowGame.WillowMind");
    not_implemented(vm, "a thing", RECORD).unwrap();
    assert!(
        vm.log
            .contains(&"UNIMPLEMENTED a thing (not implemented)".to_string())
    );
    assert_eq!(attribute(vm, mind, "NoSuchAttribute", 2.0).unwrap(), 2.0);
    assert_eq!(attribute(vm, mind, "NoSuchAttribute", 2.0).unwrap(), 2.0);
    let notes = with_state(vm, |s| s.events.clone()).unwrap();
    assert_eq!(
        notes,
        vec![
            "note: WillowMind.NoSuchAttribute is not a property of the VM object (read as 2)"
                .to_string()
        ]
    );
}

#[test]
fn a_hit_runs_the_installed_take_damage_and_releases_its_pipeline() {
    // This integration path uses the C++ Willow hold stand-in; Off refuses its unread virtuals.

    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    run.apply("player:8");
    let (pawn, controller) = (
        run.enemy(0).unwrap().pawn,
        run.player(0).unwrap().controller,
    );
    let vm = &mut run.world().vm;
    let bullet = vm.find_class("WillowGame.WillowDmgSource_Bullet").unwrap();
    let r = hit(vm, pawn, Some(controller), 10.0, "", Some(bullet)).unwrap();
    assert_eq!(
        (r.health_before, r.health_after, r.arriving),
        (40.5, 0.0, 10.0)
    );
    assert_eq!(
        r.events[0],
        "TakeDamage damage=10 type= source=WillowDmgSource_Bullet"
    );
    assert_eq!(r.order[0], "WillowGame.WillowAIPawn.TakeDamage");
    assert!(r.order.contains(&"Engine.Pawn.NotifyTakeHit".to_string()));
    assert_eq!(
        pipeline_counts(vm).unwrap().1,
        0,
        "the hit's pipeline is released"
    );
    assert!(
        vm.census.is_none() && vm.trace.is_none(),
        "the outer census and trace are restored"
    );
    let missing = hit(vm, pawn, Some(controller), 1.0, "NoSuchType", None).unwrap_err();
    assert_eq!(
        access::error_text(&missing),
        "combat hit: no such stock object NoSuchType"
    );
}

#[test]
fn an_unresolved_archetype_reference_is_refused_in_off_mode_and_kept_in_reference_mode() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, _dir) = fresh(mode);
        let vm = &mut run.world().vm;
        // A stand-in for ToyPawns' import of the package Core: the store holds no definition at that path.
        let package = {
            let (_, store) = vm.defs.code_mut();
            store
                .load(&ow_core::PackageName::parse("ToyPawns").unwrap())
                .unwrap()
        };
        let class = vm.find_class("Engine.Actor").unwrap();
        let origin = ow_object::heap::Origin::Resource { package, index: -1 };
        let stand_in = vm.space.alloc_stand_in(class, "Core", origin).unwrap();
        let value = Value::Object(Some(stand_in));
        match (mode, relink_value(vm, &value, 0)) {
            (ParityMode::Off, Err(e)) => {
                assert!(
                    access::error_text(&e).contains("the archetype's reference Core"),
                    "{e}"
                )
            }
            (ParityMode::Reference, Ok(v)) => assert_eq!(v, value),
            (m, other) => panic!("{m:?}: {other:?}"),
        }
        // Past the nesting bound.
        match (mode, relink_value(vm, &Value::Int(1), RELINK_DEPTH + 1)) {
            (ParityMode::Off, Err(e)) => {
                assert!(access::error_text(&e).contains("nests deeper"), "{e}")
            }
            (ParityMode::Reference, Ok(v)) => assert_eq!(v, Value::Int(1)),
            (m, other) => panic!("{m:?}: {other:?}"),
        }
    }
}
