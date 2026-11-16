//! Tests of the down state's natives and the currency on the invented packages of `crate::run::fixture` (AGENTS.md
//! rule 9).

use ow_core::ParityMode;
use ow_object::value::Value;

use super::*;
use crate::run::tests::{fresh, make};

fn native(vm: &mut Vm, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Value {
    let f = vm.find_function(path).unwrap();
    vm.call(f, Some(this.unwrap()), args).unwrap().ret
}

#[test]
fn off_mode_keeps_the_first_reason_of_an_injury_and_drops_one_for_a_healthy_pawn() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let pawn = make(vm, "WillowGame.WillowAIPawn");
    let set_reason = |vm: &mut Vm, r: u8| {
        native(
            vm,
            "WillowGame.WillowPawn.SetSecondWindReason",
            Some(pawn),
            vec![Value::Byte(r)],
        )
    };
    set_reason(vm, 2);
    assert_eq!(
        get(vm, pawn, "SecondWindReason").unwrap(),
        Some(Value::Byte(0)),
        "a healthy pawn: lost"
    );
    set(vm, pawn, "bIsInjured", Value::Bool(true)).unwrap();
    set_reason(vm, 1);
    set_reason(vm, 3);
    assert_eq!(
        get(vm, pawn, "SecondWindReason").unwrap(),
        Some(Value::Byte(1)),
        "the first reason wins"
    );
    native(
        vm,
        "WillowGame.WillowPawn.ClearSecondWindReason",
        Some(pawn),
        Vec::new(),
    );
    assert_eq!(
        get(vm, pawn, "SecondWindReason").unwrap(),
        Some(Value::Byte(0))
    );
    let events = crate::hit::with_state(vm, |s| s.events.clone()).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.starts_with("SetSecondWindReason"))
            .count(),
        3,
        "each call is logged"
    );
}

#[test]
fn reference_mode_stores_the_reason_as_the_cxx_does() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    let vm = &mut run.world().vm;
    let pawn = make(vm, "WillowGame.WillowAIPawn");
    native(
        vm,
        "WillowGame.WillowPawn.SetSecondWindReason",
        Some(pawn),
        vec![Value::Byte(2)],
    );
    native(
        vm,
        "WillowGame.WillowPawn.SetSecondWindReason",
        Some(pawn),
        vec![Value::Byte(4)],
    );
    assert_eq!(
        get(vm, pawn, "SecondWindReason").unwrap(),
        Some(Value::Byte(4))
    );
}

#[test]
fn an_ai_pawn_is_injured_by_its_own_byte_in_off_mode() {
    for (mode, expected) in [(ParityMode::Off, true), (ParityMode::Reference, false)] {
        let (mut run, _dir) = fresh(mode);
        let vm = &mut run.world().vm;
        let pawn = make(vm, "WillowGame.WillowAIPawn");
        assert!(!is_injured(vm, pawn).unwrap());
        set(vm, pawn, "bInjured", Value::Byte(1)).unwrap();
        assert_eq!(is_injured(vm, pawn).unwrap(), expected, "{mode:?}");
        let player = make(vm, "WillowGame.WillowPlayerPawn");
        set(vm, player, "bIsInjured", Value::Bool(true)).unwrap();
        assert_eq!(
            native(vm, "Engine.Pawn.IsInjured", Some(player), Vec::new()),
            Value::Bool(true)
        );
    }
}

#[test]
fn the_currency_is_added_within_its_caps_and_never_below_zero() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let info = make(vm, "WillowGame.WillowPlayerReplicationInfo");
    let add = |vm: &mut Vm, ty: u8, delta: i32| {
        native(
            vm,
            "WillowGame.WillowPlayerReplicationInfo.AddCurrencyOnHand",
            Some(info),
            vec![Value::Byte(ty), Value::Int(delta)],
        );
    };
    let on_hand = |vm: &mut Vm, ty: u8| {
        native(
            vm,
            "WillowGame.WillowPlayerReplicationInfo.GetCurrencyOnHand",
            Some(info),
            vec![Value::Byte(ty)],
        )
    };
    add(vm, 0, 3000);
    add(vm, 0, -500);
    assert_eq!(on_hand(vm, 0), Value::Int(2500));
    add(vm, 0, -9000);
    assert_eq!(on_hand(vm, 0), Value::Int(0), "never below 0");
    add(vm, 1, 900);
    assert_eq!(
        on_hand(vm, 1),
        Value::Int(ERIDIUM_CAP as i32),
        "Eridium caps at 500"
    );
    add(vm, 5, 5000);
    assert_eq!(on_hand(vm, 5), Value::Int(OTHER_CAP as i32));
    add(vm, 3, 7);
    assert_eq!(
        on_hand(vm, 3),
        Value::Int(0),
        "golden keys are not counted here"
    );
    set_currency_on_hand(vm, info, 0, CREDITS_CAP - 1).unwrap();
    add(vm, 0, 10);
    assert_eq!(currency_on_hand(vm, info, 0).unwrap(), CREDITS_CAP);
    assert!(
        set_currency_on_hand(vm, info, 13, 1).is_err(),
        "outside the table"
    );
    assert_eq!(currency_on_hand(vm, info, 13).unwrap(), 0);
}

#[test]
fn can_target_and_the_revive_percentage() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let pawn = make(vm, "WillowGame.WillowAIPawn");
    native(
        vm,
        "Engine.Actor.SetCanTarget",
        Some(pawn),
        vec![Value::Bool(true)],
    );
    assert_eq!(
        native(vm, "Engine.Actor.CanTarget", Some(pawn), Vec::new()),
        Value::Bool(true)
    );
    let player = make(vm, "WillowGame.WillowPlayerPawn");
    native(
        vm,
        "WillowGame.WillowPawn.SetRevivePct",
        Some(player),
        vec![Value::Float(0.5)],
    );
    assert_eq!(
        native(
            vm,
            "WillowGame.WillowPawn.GetRevivePct",
            Some(player),
            Vec::new()
        ),
        Value::Float(0.0),
        "no info: dropped"
    );
    let info = make(vm, "WillowGame.WillowPlayerReplicationInfo");
    set(
        vm,
        player,
        "PlayerReplicationInfo",
        Value::Object(Some(info)),
    )
    .unwrap();
    native(
        vm,
        "WillowGame.WillowPawn.SetRevivePct",
        Some(player),
        vec![Value::Float(0.5)],
    );
    assert_eq!(
        native(
            vm,
            "WillowGame.WillowPawn.GetRevivePct",
            Some(player),
            Vec::new()
        ),
        Value::Float(0.5)
    );
}

#[test]
fn a_player_pawn_prepared_for_the_down_state_has_the_world_and_its_info() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("player:8");
    let player = run.player(0).unwrap();
    let vm = &mut run.world().vm;
    set(vm, player.pawn, "WorldInfo", Value::Object(None)).unwrap();
    prepare_player_pawn(vm, player.pawn).unwrap();
    let world = crate::hit::with_state(vm, |s| s.world).unwrap();
    assert_eq!(object(vm, player.pawn, "WorldInfo").unwrap(), world);
    let info = object(vm, player.controller, "PlayerReplicationInfo").unwrap();
    assert_eq!(
        object(vm, player.pawn, "PlayerReplicationInfo").unwrap(),
        info
    );
}
