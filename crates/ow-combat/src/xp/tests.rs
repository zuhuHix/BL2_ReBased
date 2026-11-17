//! Tests of the kill-experience glue on the invented packages of `crate::run::fixture` (AGENTS.md rule 9). The data
//! (`fixture::startup`): the formula 20 x 1.5 = 30 for every kill, the weights (0, 1) for one player and (0.25, 0.5)
//! for two, the level table (1: 1.5 / 0.8, 2: 2.0 / 0.5). The amounts are worked by hand as in `tests/combat_test.py`.

use ow_core::ParityMode;
use ow_object::value::Value;

use super::*;
use crate::run::tests::{fresh, run_steps};

/// The `AwardCombatExperienceToParty` lines of a kill of a level 5 pawn after `steps`.
fn credits(steps: &[&str]) -> Vec<String> {
    let mut all = vec!["spawn:ToyPawns/Pawn_Doomed:5"];
    all.extend_from_slice(steps);
    all.push("hit:0:10::Bullet");
    // The kill script reaches the C++ Willow hold stand-in; Off refuses that unread virtual.
    let (text, failed) = run_steps(ParityMode::Reference, &all);
    assert!(!failed, "{text}");
    text.match_indices("AwardCombatExperienceToParty player_level=")
        .map(|(at, _)| {
            let rest = &text[at..];
            rest[..rest.find('"').unwrap()].to_string()
        })
        .collect()
}

#[test]
fn one_player_below_the_victim_takes_the_higher_scale() {
    assert_eq!(
        credits(&["player:4", "players:1"]),
        ["AwardCombatExperienceToParty player_level=4 victim_level=5 ExpEarn(45, 1, 1)"]
    );
}

#[test]
fn two_players_split_with_the_killers_bonus_and_each_ones_level_scale() {
    assert_eq!(
        credits(&["player:4", "player:7", "players:2"]),
        [
            "AwardCombatExperienceToParty player_level=4 victim_level=5 ExpEarn(33, 1, 1)",
            "AwardCombatExperienceToParty player_level=7 victim_level=5 ExpEarn(7, 1, 1)",
        ]
    );
}

#[test]
fn a_difference_past_the_table_uses_its_last_row_and_equal_levels_none() {
    assert_eq!(
        credits(&["player:1", "players:1"]),
        ["AwardCombatExperienceToParty player_level=1 victim_level=5 ExpEarn(60, 1, 1)"]
    );
    assert_eq!(
        credits(&["player:5", "players:1"]),
        ["AwardCombatExperienceToParty player_level=5 victim_level=5 ExpEarn(30, 1, 1)"]
    );
}

#[test]
fn a_player_count_outside_the_table_is_bonus_zero_weight_one() {
    // The WillowGameInfo default is 0 players.
    assert_eq!(
        credits(&["player:4"]),
        ["AwardCombatExperienceToParty player_level=4 victim_level=5 ExpEarn(45, 1, 1)"]
    );
    let (mut run, _dir) = fresh(ParityMode::Reference);
    let vm = &mut run.world().vm;
    let g = crate::hit::globals(vm).unwrap();
    assert_eq!(
        player_count(vm, g, 2).unwrap().get(),
        2,
        "inside the table: the count itself"
    );
    assert_eq!(
        player_count(vm, g, 0).unwrap().get(),
        PlayerCount::MAX,
        "outside: a count past the table's two rows"
    );
    assert_eq!(player_count(vm, g, 9).unwrap().get(), PlayerCount::MAX);
}

#[test]
fn the_award_is_recorded_with_the_victims_level_and_each_credit() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Doomed:5");
    run.apply("player:4");
    run.apply("hit:0:10::Bullet");
    let pawn = run.enemy(0).unwrap().pawn;
    let controller = run.player(0).unwrap().controller;
    let vm = &mut run.world().vm;
    let kills = crate::hit::with_state(vm, |s| s.kills.clone()).unwrap();
    assert_eq!(kills.len(), 1);
    assert_eq!(
        (kills[0].killed, kills[0].killed_level, kills[0].award),
        (Some(pawn), 5, 30.0)
    );
    assert_eq!(
        kills[0].credits,
        vec![Credit {
            controller,
            player_level: 4,
            amount: 45
        }]
    );
    // The toy ExpEarn stores what it was given.
    assert_eq!(
        get(vm, controller, "LastExp").unwrap(),
        Some(Value::Int(45))
    );
}

#[test]
fn a_spawned_pawn_takes_its_experience_multiplier_from_its_class() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    let pawn = run.enemy(0).unwrap().pawn;
    let vm = &mut run.world().vm;
    assert_eq!(
        get(vm, pawn, "ExperiencePointMultiplier").unwrap(),
        Some(Value::Float(0.75))
    );
    assert_eq!(
        get(vm, pawn, "ExperiencePointMultiplierBaseValue").unwrap(),
        Some(Value::Float(0.75))
    );
}

#[test]
fn a_balance_rule_error_keeps_its_not_yet() {
    let n = ow_core::NotYet::not_read("x", RECORD);
    assert!(matches!(
        balance_error(BalanceError::NotYet(n)),
        VmError::NotYet(_)
    ));
}
