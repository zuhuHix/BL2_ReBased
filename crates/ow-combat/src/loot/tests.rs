//! Tests of the loot rules on the invented packages of `crate::run::fixture` with the loot part of
//! `loot/fixture.rs` (AGENTS.md rule 9). The expected values are the rules worked by hand on that data (the same data
//! as `tests/combat_test.py`'s loot section), and the whole-report digests are the C++ binary's output.

use std::path::PathBuf;

use ow_balance::types::Playthrough;
use ow_core::notyet::Missing;
use ow_object::value::Value;
use ow_vm::api::Vm;

use super::*;
use crate::run::combat_run;
use crate::run::tests::{fresh, run_steps, world};

/// Writes the fixture's packages to `OW_LOOT_FIXTURE` (default `local/p2/w6-loot/rust_fixture`), so the C++ binary
/// can run on the same bytes: `cargo test -p ow-combat -- --ignored write_loot_fixture`.
#[test]
#[ignore]
fn write_loot_fixture() {
    let dir = std::env::var("OW_LOOT_FIXTURE").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w6-loot/rust_fixture")
    });
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in crate::run::fixture::files() {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The S-g step lists against the C++ binary
// ---------------------------------------------------------------------------------------------------------------

/// `seed:<seed> player:8`, then `kills` times a loot pawn of `balance` spawned at `level`/`stage` and hit.
fn kills(seed: u32, kills: usize, balance: &str, level: i32, stage: i32) -> String {
    let mut steps = vec![format!("seed:{seed}"), "player:8".to_string()];
    for i in 0..kills {
        steps.push(format!("spawn:ToyPawns/Pawn_Loot:{level}:{stage}:{balance}"));
        steps.push(format!("hit:{i}:10::Bullet"));
    }
    steps.join(" ")
}

/// The scenarios: name, steps, and the FNV-1a 64 of the C++ binary's stdout line for them (newline read as `\n`)
/// after [`normalize`]. Where they come from: `cargo test -p ow-combat -- --ignored write_loot_fixture` wrote the
/// fixture; `build/Release/ow-package.exe <dir>/ToyPawns.upk --combat-run --cooked <dir> <steps>` ran on it, the C++
/// binary built in this lane's worktree on main 34117d3, 2026-11-16, pc-hummer (the same digests came from a C++ build
/// of the earlier base d9679ce, whose `src/` is the same). The Rust report equals it in Reference mode only: Off mode
/// runs the installed `DropLootOnDeath`.
fn cases() -> Vec<(&'static str, String, u64)> {
    vec![
        ("loot_toy", kills(3, 12, "Balance_Toy", 4, 6), 0x8903_5317_ff2b_8766),
        ("loot_low", kills(3, 6, "Balance_Low", 4, 6), 0xdae4_a1c0_94e6_3359),
        ("loot_cap", kills(3, 2, "Balance_Toy", 4, 59), 0xefb9_d712_b29f_ab27),
        ("loot_none", kills(3, 1, "Balance_None", 4, 6), 0x8982_9ca3_5284_dd04),
        ("loot_seed5", kills(5, 8, "Balance_Toy", 4, 6), 0x53e9_0c6c_4054_3098),
        (
            "loot_default_seed",
            "player:8 spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy hit:0:10::Bullet spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy hit:1:10::Bullet".into(),
            0x3fe9_7718_1d9f_f573,
        ),
        (
            "rollpool",
            "seed:3 spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low rollpool:0:Pool_Gated:4:5 rollpool:0:Pool_Gated:5:5 rollpool:0:Pool_Coins:4:7".into(),
            0x0e69_fba1_124f_ac39,
        ),
        (
            "rollpool2",
            "seed:7 spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy rollpool:0:Pool_Nested rollpool:0:Pool_Rand:7:6 rollpool:0:Pool_Fallback:3:2 rollpool:0:Pool_Empty rollpool:0:Pool_Coins:60:3".into(),
            0xd32c_5a12_d5ba_1389,
        ),
        (
            "lootbatch",
            "seed:5 player:8 lootbatch:ToyPawns/Pawn_Loot:4:6:5:toy spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy hit:0:10::Bullet lootbatch:ToyPawns/Pawn_Loot:3:9:2".into(),
            0x0362_69e7_17a1_509b,
        ),
        (
            "refusals",
            "seed:x rollpool:0 spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy rollpool:0 rollpool:0:NoSuchPool rollpool:0:Pool_Coins:x lootbatch:a lootbatch:a:1:1:1 spawn:ToyPawns/Pawn_Loot:4:6:NoBalance".into(),
            0xa6fd_5e11_3a60_73d1,
        ),
        (
            "seeds",
            "spawn:ToyPawns/Pawn_Toy:4 seed:4294967295 seed:4294967296 seed:-1".into(),
            0x2eb0_1324_f841_cbd0,
        ),
        (
            "two",
            "spawn:ToyPawns/Pawn_Loot:4:6:Balance_Two spawn:ToyPawns/Pawn_Loot:4:6:Balance_Loop".into(),
            0x20c8_3016_3807_b951,
        ),
    ]
}

/// FNV-1a 64.
fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Removes `,"<key>":<number>`.
fn drop_number_field(text: &str, key: &str) -> String {
    let pattern = format!(",\"{key}\":");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(&pattern) {
        out.push_str(&rest[..at]);
        let after = &rest[at + pattern.len()..];
        let end = after
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e')))
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// W6-COMBAT's normalization (`ow-cli` `cmd/combat/tests.rs`): the two timing fields and the `SetHealth <value>`
/// event lines (the C++ pool half's log, which the Rust `SetHealth` does not write) removed.
fn normalize(text: &str) -> String {
    let text = drop_number_field(text, "milliseconds");
    let text = drop_number_field(&text, "elapsed_ms");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find("\"SetHealth ") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let close = after.find('"').map_or(after.len(), |i| i + 1);
        let after = &after[close..];
        rest = after.strip_prefix(',').unwrap_or(after);
    }
    out.push_str(rest);
    out
}

fn report(steps: &str, mode: ParityMode) -> (String, bool) {
    let (w, _dir) = world(mode);
    let steps: Vec<String> = steps.split(' ').map(str::to_string).collect();
    combat_run(w, &steps).unwrap()
}

#[test]
fn the_sg_step_lists_equal_the_cxx_binary_in_reference_mode() {
    for (name, steps, cxx) in cases() {
        let (text, _) = report(&steps, ParityMode::Reference);
        assert_eq!(
            fnv1a64(format!("{}\n", normalize(&text)).as_bytes()),
            cxx,
            "{name}: {text}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Off mode: the replacement of the stand-in (architecture 6.1, "21 stand-in inventories")
// ---------------------------------------------------------------------------------------------------------------

fn json(text: &str) -> String {
    text.to_string()
}

#[test]
fn off_mode_makes_no_stand_in_inventory_the_pool_native_is_not_yet_before_any_draw() {
    // The step list of tests/combat_loot_standin_test.py, which Reference answers with 21 stand-ins.
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("seed:3");
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low");
    let vm = &mut run.world().vm;
    let before = (vm.rng.crt(OFF_STREAM).clone(), vm.rng.crt(REFERENCE_STREAM).clone());
    run.apply("rollpool:0:Pool_Coins:4:7");
    let vm = &mut run.world().vm;
    let after = (vm.rng.crt(OFF_STREAM).clone(), vm.rng.crt(REFERENCE_STREAM).clone());
    assert_eq!(before, after, "nothing drawn");
    let errors = run.errors().join("\n");
    assert!(errors.contains("rollpool:0:Pool_Coins:4:7: "), "{errors}");
    for text in [
        "ItemPool.SpawnBalancedInventoryFromPool in Off mode",
        "roll_pool evaluates with no context source",
        "does not report the native's return value",
        "not ported to Rust yet",
    ] {
        assert!(errors.contains(text), "{text}: {errors}");
    }
    let text = run.report(0.0);
    assert!(!text.contains("loot.roll.Pool_Coins"), "{text}");
    // Reference makes the stand-ins.
    let (text, failed) = run_steps(
        ParityMode::Reference,
        &["seed:3", "spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low", "rollpool:0:Pool_Coins:4:7"],
    );
    assert!(!failed, "{text}");
    assert!(text.contains("\"loot.roll.Pool_Coins.stage4.items\":21"), "{text}");
    assert!(json(&text).contains("{\"class\":\"WillowUsableItem\",\"drop_on_death\":true}"), "{text}");
}

#[test]
fn off_mode_stops_before_loot_at_unread_willow_hold_virtual() {
    let steps = ["seed:3", "player:8", "spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy", "hit:0:10::Bullet"];
    let (text, failed) = run_steps(ParityMode::Off, &steps);
    assert!(failed && text.contains("AIComponent has no DemigodHolds array"), "{text}");
    // Off stops before the loot script, at the Willow component's demigod list (the toy AIComponent declares no DemigodHolds (the combat fixture is pinned against the C++ digests); since decision 2026-11-19-43 Off reads the Willow component's demigod list, so it stops there).
    assert!(!text.contains("WillowGame.WillowPawn.DropLootOnDeath"), "{text}");
    assert!(!text.contains("\"loot\":"), "{text}");
    assert!(!text.contains("DropLootOnDeath: item stage"), "{text}");
    let (text, failed) = run_steps(ParityMode::Reference, &steps);
    assert!(!failed, "{text}");
    assert!(text.contains("\"loot\":{\"item_stage\":7,"), "{text}");
    assert!(text.contains("DropLootOnDeath: item stage 7, 6 pools rolled (1 carried-gear pools skipped)"), "{text}");
}

#[test]
fn off_mode_loot_batch_is_not_yet_before_any_kill() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("player:8");
    run.apply("lootbatch:ToyPawns/Pawn_Loot:4:6:3");
    assert!(run.enemy(0).is_none(), "no pawn was spawned");
    let errors = run.errors().join("\n");
    assert!(errors.contains("lootbatch in Off mode") && errors.contains("not ported to Rust yet"), "{errors}");
}

#[test]
fn a_variance_formula_is_the_cxx_note_in_reference_and_not_yet_in_off() {
    for mode in [ParityMode::Reference, ParityMode::Off] {
        let (mut run, _dir) = fresh(mode);
        run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low");
        let pawn = run.enemy(0).unwrap().pawn;
        let vm = &mut run.world().vm;
        let pool = crate::run::tests::def(vm, "Pool_Coins");
        let f = vm.find_function(FROM_POOL).unwrap();
        let args = vec![
            Value::Object(Some(pool)),
            Value::Int(4),
            Value::Int(0),
            Value::Object(Some(pawn)),
            Value::array(Vec::new()),
            Value::Object(Some(pool)),
        ];
        let r = vm.call(f, None, args);
        match mode {
            ParityMode::Reference => {
                assert_eq!(r.unwrap().ret, Value::Bool(true));
                assert!(vm.log.iter().any(|l| l
                    == "UNIMPLEMENTED SpawnBalancedInventoryFromPool: a game stage variance formula (not implemented)"));
            }
            ParityMode::Off => match r.unwrap_err() {
                VmError::NotYet(n) => assert_eq!(n.missing, Missing::NotPorted),
                other => panic!("{other:?}"),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The spawn's loot setup and SetupPawnItemPoolList (both modes)
// ---------------------------------------------------------------------------------------------------------------

fn setup_of(text: &str) -> String {
    let at = text.find("\"loot_setup\":").expect("a loot setup");
    let rest = &text[at..];
    rest[..rest.find('}').unwrap() + 1].to_string()
}

#[test]
fn the_item_stage_is_the_game_stage_plus_the_truncated_offset_or_the_experience_level() {
    for mode in [ParityMode::Reference, ParityMode::Off] {
        // BaseItemGameStage 1: the game stage 6 plus trunc(1.9) = 7.
        let (text, _) = run_steps(mode, &["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy"]);
        assert!(setup_of(&text).contains("\"item_stage\":7"), "{text}");
        // BaseItemGameStage 0: the experience level 4, offset 0.
        let (text, _) = run_steps(mode, &["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low"]);
        assert!(setup_of(&text).contains("\"item_stage\":4"), "{text}");
        // The stage 59 + 1 = 60 is kept (the cap applies to an item's level only).
        let (text, _) = run_steps(mode, &["spawn:ToyPawns/Pawn_Loot:4:59:Balance_Toy"]);
        assert!(setup_of(&text).contains("\"item_stage\":60"), "{text}");
    }
}

#[test]
fn the_pool_list_is_the_included_lists_depth_first_then_the_own_pools() {
    for mode in [ParityMode::Reference, ParityMode::Off] {
        // The custom included list List_B (which includes List_C first) replaces the default List_A; the custom own
        // list is empty, so the default own list stays. Pool_Ready is listed (the death roll skips it).
        let (text, _) = run_steps(mode, &["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy"]);
        assert!(
            setup_of(&text).contains(
                "\"pools\":[\"Pool_Half\",\"Pool_Coins\",\"Pool_Ready\",\"Pool_Gated\",\"Pool_Nested\",\"Pool_Rand\",\"Pool_Fallback\"]"
            ),
            "{text}"
        );
    }
}

#[test]
fn the_setup_logs_the_cxx_event_line() {
    // The line goes to the event list, which the next hit clears before it starts (as the C++ does).
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_None");
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy");
    let events = with_state(&run.world().vm, |s| s.events.clone()).unwrap();
    assert!(events.contains(&"SetupPawnItemPoolList Balance_None: 0 pools, item game stage 6 (no list)".to_string()), "{events:?}");
    assert!(events.contains(&"SetupPawnItemPoolList Balance_Toy: 7 pools, item game stage 7".to_string()), "{events:?}");
}

#[test]
fn a_balance_with_no_playthrough_entry_gets_no_list_and_answers_false() {
    for mode in [ParityMode::Reference, ParityMode::Off] {
        let (text, failed) = run_steps(mode, &["spawn:ToyPawns/Pawn_Loot:4:6:Balance_None"]);
        assert_eq!(failed, mode == ParityMode::Off, "{text}");
        // Off: the toy AIComponent declares no DemigodHolds (the combat fixture is pinned against the C++ digests); since decision 2026-11-19-43 Off reads the Willow component's demigod list, so it stops there.
        if failed { assert!(text.contains("AIComponent has no DemigodHolds array"), "{text}"); }
        assert!(setup_of(&text).contains("\"pools\":[]"), "{text}");
    }
}

/// A run over the fixture in `mode` whose world is on playthrough `pt`, with a loot pawn of `balance` spawned.
fn on_playthrough(mode: ParityMode, pt: Playthrough, balance: &str) -> (crate::run::Run, crate::run::tests::Dir) {
    let (mut run, dir) = fresh(mode);
    {
        let vm = &mut run.world().vm;
        let state = vm.service::<BalanceState>().unwrap();
        state.borrow_mut().snapshot.playthrough = pt;
    }
    run.apply(&format!("spawn:ToyPawns/Pawn_Loot:4:6:{balance}"));
    (run, dir)
}

fn pool_list(run: &mut crate::run::Run) -> Vec<String> {
    let pawn = run.enemy(0).expect("spawned").pawn;
    let vm = &mut run.world().vm;
    let list = get(vm, pawn, "ItemPoolList").unwrap().unwrap();
    items(&list)
        .iter()
        .map(|e| path_of(vm, field(vm, e, "ItemPool").as_ref().and_then(object_of)))
        .collect()
}

#[test]
fn off_mode_reads_the_world_playthrough_at_most_the_last_entry_and_reference_reads_the_first() {
    let first = ["Pool_Half", "Pool_Coins", "Pool_Ready", "Pool_Gated", "Pool_Coins"];
    let second = ["Pool_Never", "Pool_Coins"];
    let (mut run, _d) = on_playthrough(ParityMode::Off, Playthrough::NORMAL, "Balance_Two");
    assert_eq!(pool_list(&mut run), first);
    let (mut run, _d) = on_playthrough(ParityMode::Off, Playthrough::TVHM, "Balance_Two");
    assert_eq!(pool_list(&mut run), second);
    // Past the last entry: the last one.
    let (mut run, _d) = on_playthrough(ParityMode::Off, Playthrough::UVHM, "Balance_Two");
    assert_eq!(pool_list(&mut run), second);
    // A one-entry balance on the second playthrough: its only entry.
    let (mut run, _d) = on_playthrough(ParityMode::Off, Playthrough::TVHM, "Balance_Low");
    assert_eq!(pool_list(&mut run).len(), 7);
    // Reference: the C++ models the first playthrough only.
    let (mut run, _d) = on_playthrough(ParityMode::Reference, Playthrough::TVHM, "Balance_Two");
    assert_eq!(pool_list(&mut run), first);
}

#[test]
fn a_list_that_includes_itself_is_refused_in_off_mode_and_stops_at_the_cxx_depth_in_reference() {
    let (text, _) = run_steps(ParityMode::Off, &["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Loop"]);
    assert!(text.contains("combat loot: the item pool list List_Loop includes itself"), "{text}");
    let (text, _) = run_steps(ParityMode::Reference, &["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Loop"]);
    assert!(text.contains("combat loot: item pool lists nest too deep"), "{text}");
}

#[test]
fn off_mode_without_the_balance_state_is_not_yet_for_the_playthrough() {
    let mut vm = Vm::new(
        ow_object::heap::ObjectSpace::new(),
        ow_defs::DefStore::new_empty(),
        std::rc::Rc::new(ow_attr::AttributeSystem::new_empty()),
        ParityMode::Off,
    );
    match play_through_index(&mut vm, 2).unwrap_err() {
        VmError::NotYet(n) => assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(play_through_index(&mut vm, 0).unwrap(), None);
    let mut vm = Vm::new(
        ow_object::heap::ObjectSpace::new(),
        ow_defs::DefStore::new_empty(),
        std::rc::Rc::new(ow_attr::AttributeSystem::new_empty()),
        ParityMode::Reference,
    );
    assert_eq!(play_through_index(&mut vm, 3).unwrap(), Some(0));
}

// ---------------------------------------------------------------------------------------------------------------
// The pool roll (Reference: combat_loot.cpp's reading of NATIVE_LOOT.md)
// ---------------------------------------------------------------------------------------------------------------

/// The rows of a Reference run.
fn rows(steps: &[&str]) -> String {
    let (text, failed) = run_steps(ParityMode::Reference, steps);
    assert!(!failed, "{text}");
    text
}

#[test]
fn the_stage_gate_closes_below_the_minimum_and_draws_nothing() {
    let text = rows(&[
        "seed:3",
        "spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low",
        "rollpool:0:Pool_Gated:4:5",
        "rollpool:0:Pool_Gated:5:5",
    ]);
    assert!(text.contains("\"loot.roll.Pool_Gated.stage4.true\":0,\"loot.roll.Pool_Gated.stage5.items\":5"), "{text}");
    assert!(text.contains("\"loot.roll.Pool_Gated.stage4.items\":0"), "{text}");
    assert!(text.contains("\"loot.roll.Pool_Gated.stage5.true\":5"), "{text}");
}

#[test]
fn a_pool_whose_balances_give_no_candidate_fails_each_pick() {
    // Pool_Inner's only balance (Bal_Gem) has the grade window 5 to 100: at stage 3 the gate passes (a balance entry)
    // but no candidate exists, so every pick fails and the call answers false.
    let text = rows(&["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low", "rollpool:0:Pool_Inner:3:2", "rollpool:0:Pool_Inner:5:2"]);
    assert!(text.contains("\"loot.roll.Pool_Inner.stage3.true\":0"), "{text}");
    assert!(text.contains("\"loot.roll.Pool_Inner.stage3.items\":0"), "{text}");
    assert!(text.contains("\"loot.roll.Pool_Inner.stage5.true\":2"), "{text}");
    // An empty pool fails its gate (no reachable entry).
    let text = rows(&["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low", "rollpool:0:Pool_Empty:5:1"]);
    assert!(text.contains("\"loot.roll.Pool_Empty.stage5.true\":0"), "{text}");
}

#[test]
fn an_empty_nested_pool_takes_no_share_and_the_outer_flag_ands_into_the_item() {
    // Pool_Nested: the empty pool (weight 1000) is absent, so the inner pool always rolls its gem, whose own flag is
    // true but the outer entry's is false.
    let text = rows(&["seed:11", "spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy", "rollpool:0:Pool_Nested:7:20"]);
    assert!(text.contains("\"loot.roll.Pool_Nested.stage7.true\":20"), "{text}");
    assert!(text.contains("\"loot.roll.Pool_Nested.stage7.items\":20"), "{text}");
    assert!(text.contains("\"inventories\":[{\"class\":\"WillowWeapon\",\"drop_on_death\":false}]"), "{text}");
    // An entry naming an empty pool and a balance uses the balance.
    let text = rows(&["spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy", "rollpool:0:Pool_Fallback:7:3"]);
    assert!(text.contains("\"loot.roll.Pool_Fallback.stage7.items\":3"), "{text}");
    assert!(text.contains("{\"class\":\"WillowUsableItem\",\"drop_on_death\":true}"), "{text}");
}

#[test]
fn a_random_integer_quantity_reaches_both_ends() {
    let mut seen = DetSet::new();
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy");
    let pawn = run.enemy(0).unwrap().pawn;
    for _ in 0..40 {
        let vm = &mut run.world().vm;
        let pool = crate::run::tests::def(vm, "Pool_Rand");
        let mut made = Vec::new();
        assert!(spawn_from_pool(vm, pool, 7, 0, Some(pawn), true, &mut made, 0).unwrap());
        seen.insert(made.len());
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec![2, 3, 4]);
}

type DetSet<T> = ow_core::DetSet<T>;

#[test]
fn weights_one_to_three_pick_about_a_quarter_coins_and_the_entry_flag_decides_the_drop() {
    let text = rows(&[
        "seed:3",
        "player:8",
        "spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy",
        "rollpool:0:Pool_Coins:7:60",
    ]);
    assert!(text.contains("\"loot.roll.Pool_Coins.stage7.items\":180"), "{text}");
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("seed:3");
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy");
    let pawn = run.enemy(0).unwrap().pawn;
    let vm = &mut run.world().vm;
    let pool = crate::run::tests::def(vm, "Pool_Coins");
    let mut made = Vec::new();
    for _ in 0..60 {
        assert!(spawn_from_pool(vm, pool, 7, 0, Some(pawn), true, &mut made, 0).unwrap());
    }
    let coins = made.iter().filter(|i| i.balance == "Bal_Coin").count();
    assert_eq!(made.len(), 180);
    assert!((22..=68).contains(&coins), "{coins}");
    for item in &made {
        // Bal_Coin's entry flag is true, Bal_Gem's false; the classes come from the balances' definitions.
        let (class, flag) = if item.balance == "Bal_Coin" { ("WillowUsableItem", true) } else { ("WillowWeapon", false) };
        assert_eq!((item.class_name.as_str(), item.drop_on_death, item.level), (class, flag, 7));
        assert_eq!(item.manufacturer, "Mfr_Toy");
    }
    // The caller's flag false makes every item undroppable.
    let mut made = Vec::new();
    spawn_from_pool(vm, pool, 7, 0, Some(pawn), false, &mut made, 0).unwrap();
    assert!(made.iter().all(|i| !i.drop_on_death));
}

#[test]
fn a_grade_window_that_excludes_the_stage_gives_no_candidate_and_the_level_is_capped() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Low");
    let pawn = run.enemy(0).unwrap().pawn;
    let vm = &mut run.world().vm;
    let pool = crate::run::tests::def(vm, "Pool_Coins");
    let mut made = Vec::new();
    for _ in 0..10 {
        spawn_from_pool(vm, pool, 4, 0, Some(pawn), true, &mut made, 0).unwrap();
    }
    assert!(made.iter().all(|i| i.balance == "Bal_Coin" && i.level == 4), "{made:?}");
    let mut made = Vec::new();
    spawn_from_pool(vm, pool, 60, 0, Some(pawn), true, &mut made, 0).unwrap();
    assert!(made.iter().all(|i| i.level == 50), "{made:?}");
}

#[test]
fn the_same_seed_repeats_the_record_another_does_not_and_the_default_is_the_cxx_seed() {
    let a = normalize(&report(&kills(5, 6, "Balance_Toy", 4, 6), ParityMode::Reference).0);
    let b = normalize(&report(&kills(5, 6, "Balance_Toy", 4, 6), ParityMode::Reference).0);
    let c = normalize(&report(&kills(6, 6, "Balance_Toy", 4, 6), ParityMode::Reference).0);
    assert_eq!(a, b);
    assert_ne!(a, c);
    // Without a seed step the generator starts at DEFAULT_SEED.
    let steps = "player:8 spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy hit:0:10::Bullet";
    let default = normalize(&report(steps, ParityMode::Reference).0);
    let explicit = normalize(&report(&format!("seed:{DEFAULT_SEED} {steps}"), ParityMode::Reference).0);
    assert_eq!(default.replace(&format!("{{\"step\":\"seed:{DEFAULT_SEED}\",\"result\":{{}}}},"), ""), default);
    assert_eq!(
        explicit.replace(&format!("{{\"step\":\"seed:{DEFAULT_SEED}\",\"result\":{{}}}},"), ""),
        default
    );
}

#[test]
fn the_seed_step_restarts_the_loot_stream_of_each_mode() {
    for (mode, stream, other) in [
        (ParityMode::Reference, REFERENCE_STREAM, OFF_STREAM),
        (ParityMode::Off, OFF_STREAM, REFERENCE_STREAM),
    ] {
        let (mut run, _dir) = fresh(mode);
        let untouched = run.world().vm.rng.crt(other).clone();
        run.apply("seed:9");
        let vm = &mut run.world().vm;
        assert_eq!(vm.rng.crt(stream).state, 9);
        assert_eq!(*vm.rng.crt(other), untouched);
    }
    // Reference starts from the C++ default seed.
    let (mut run, _dir) = fresh(ParityMode::Reference);
    assert_eq!(run.world().vm.rng.crt(REFERENCE_STREAM).state, DEFAULT_SEED);
}

// ---------------------------------------------------------------------------------------------------------------
// The death roll (Reference)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_death_roll_skips_ready_pools_draws_first_and_tries_a_pool_at_most_its_chance() {
    let (text, failed) = report(&kills(3, 30, "Balance_Toy", 4, 6), ParityMode::Reference);
    assert!(!failed, "{text}");
    let drops = text.matches("\"loot\":{\"item_stage\":7,").count();
    assert_eq!(drops, 30);
    // Chance 0.5: tried exactly when the draw is at most 0.5; chance 1: always tried, 3 picks.
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("seed:3");
    run.apply("player:8");
    for i in 0..30 {
        run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy");
        run.apply(&format!("hit:{i}:10::Bullet"));
    }
    let _ = run.report(0.0);
    // The records are reported and kept.
    let all = drops_from(&run.world().vm, 0).unwrap();
    assert_eq!(all.len(), 30);
    let mut half = 0;
    for d in &all {
        let names: Vec<&str> = d.pools.iter().map(|p| leaf(&p.pool)).collect();
        assert_eq!(names, ["Pool_Half", "Pool_Coins", "Pool_Gated", "Pool_Nested", "Pool_Rand", "Pool_Fallback"]);
        let h = &d.pools[0];
        assert_eq!(h.tried, h.roll <= 0.5);
        half += usize::from(h.tried);
        let c = &d.pools[1];
        assert!(c.tried && c.returned && c.items.len() == 3, "{c:?}");
        for item in &c.items {
            assert_eq!(item.dropped, item.drop_on_death);
        }
    }
    assert!((8..=22).contains(&half), "{half}");
}

#[test]
fn an_item_is_dropped_when_its_flag_is_set_unless_a_weapon_falls_below_kill_z() {
    assert!(is_dropped(true, false, true));
    assert!(is_dropped(true, true, false));
    assert!(!is_dropped(true, true, true));
    assert!(!is_dropped(false, false, false));
    assert!(below_kill_z(Some(-5.0), Some(0.0)));
    assert!(!below_kill_z(Some(5.0), Some(0.0)));
    assert!(!below_kill_z(None, Some(0.0)));
    assert!(!below_kill_z(Some(-5.0), None));
}

#[test]
fn a_pawn_without_the_item_stage_drops_nothing() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    let pawn = run.enemy(0).unwrap().pawn;
    let vm = &mut run.world().vm;
    drop_loot(vm, pawn).unwrap();
    assert_eq!(drop_count(vm).unwrap(), 0);
}

// ---------------------------------------------------------------------------------------------------------------
// Reports and the batch sums
// ---------------------------------------------------------------------------------------------------------------

fn item(balance: &str, level: i32, dropped: bool) -> LootItem {
    LootItem {
        pool: "P".into(),
        balance: balance.into(),
        manufacturer: "M".into(),
        class_name: "C".into(),
        class: None,
        level,
        drop_on_death: dropped,
        dropped,
    }
}

fn pool(name: &str, chance: f64, tried: bool, returned: bool, items: Vec<LootItem>) -> LootPool {
    LootPool {
        pool: format!("G.{name}"),
        chance,
        roll: 0.25,
        tried,
        returned,
        items,
        ..LootPool::default()
    }
}

/// A real object to stand for a pawn in a record (the run is kept alive with it).
fn some_object() -> (ObjRef, crate::run::Run, crate::run::tests::Dir) {
    let (mut run, dir) = fresh(ParityMode::Off);
    let o = crate::run::tests::make(&mut run.world().vm, "WillowGame.ItemPool");
    (o, run, dir)
}

#[test]
fn the_loot_json_and_rows_are_the_cxx_shapes() {
    let (pawn, _run, _dir) = some_object();
    let mut failed = pool("Pool_B", 0.5, false, false, Vec::new());
    failed.error = "bad \"x\"".into();
    let drop = LootDrop {
        pawn,
        stage: 7,
        awesome: 0,
        below_kill_z: false,
        pools: vec![pool("Pool_A", 1.0, true, true, vec![item("Bal_X", 7, true), item("Bal_Y", 7, false)]), failed],
    };
    assert_eq!(
        loot_json(&drop),
        "{\"item_stage\":7,\"awesome\":0,\"below_kill_z\":false,\"pools\":[{\"pool\":\"G.Pool_A\",\"chance\":1,\"roll\":0.25,\
         \"tried\":true,\"returned\":true,\"items\":[{\"balance\":\"Bal_X\",\"manufacturer\":\"M\",\"class\":\"C\",\"level\":7,\
         \"drop_on_death\":true,\"dropped\":true},{\"balance\":\"Bal_Y\",\"manufacturer\":\"M\",\"class\":\"C\",\"level\":7,\
         \"drop_on_death\":false,\"dropped\":false}]},{\"pool\":\"G.Pool_B\",\"chance\":0.5,\"roll\":0.25,\"tried\":false,\
         \"returned\":false,\"error\":\"bad \\\"x\\\"\",\"items\":[]}]}"
    );
    let rows: Vec<(String, f64)> = loot_rows(&drop, "Loot.L4.1");
    assert_eq!(
        rows,
        vec![
            ("loot.Loot.L4.1.Pool_A.returned".to_string(), 1.0),
            ("loot.Loot.L4.1.Pool_A.items".to_string(), 2.0),
            ("loot.Loot.L4.1.item_stage".to_string(), 7.0),
            ("loot.Loot.L4.1.pools_tried".to_string(), 1.0),
            ("loot.Loot.L4.1.items_dropped".to_string(), 1.0),
        ]
    );
}

#[test]
fn a_batch_sums_the_rolls_per_pool_with_the_cxx_keys() {
    let (pawn, _run, _dir) = some_object();
    let mut batch = Batch::new();
    let mut errored = pool("Pool_Ammo", 0.0, false, false, Vec::new());
    errored.error = "x".into();
    batch.add(&LootDrop {
        pawn,
        stage: 6,
        awesome: 0,
        below_kill_z: false,
        pools: vec![
            pool("Pool_Money_1or2", 0.25, true, true, vec![item("M", 6, true), item("M", 6, true)]),
            errored.clone(),
        ],
    })
    .unwrap();
    batch.add(&LootDrop {
        pawn,
        stage: 7,
        awesome: 0,
        below_kill_z: false,
        pools: vec![
            pool("Pool_Money_1or2", 0.5, true, false, vec![item("M", 6, false)]),
            errored,
        ],
    })
    .unwrap();
    assert_eq!(batch.kills(), 2);
    let got: Vec<(String, f64)> = batch.rows("t");
    let expect: Vec<(&str, f64)> = vec![
        ("loot.batch.t.n", 2.0),
        ("loot.batch.t.stage.6", 1.0),
        ("loot.batch.t.stage.7", 1.0),
        ("loot.batch.t.stage_min", 6.0),
        ("loot.batch.t.stage_max", 7.0),
        ("loot.batch.t.level_mismatch", 1.0),
        ("loot.batch.t.Pool_Ammo.chance", 0.0),
        ("loot.batch.t.Pool_Ammo.chance_max", 0.0),
        ("loot.batch.t.Pool_Ammo.tried", 0.0),
        ("loot.batch.t.Pool_Ammo.true", 0.0),
        ("loot.batch.t.Pool_Ammo.items", 0.0),
        ("loot.batch.t.Pool_Ammo.dropped", 0.0),
        ("loot.batch.t.Pool_Ammo.errors", 2.0),
        ("loot.batch.t.Pool_Money_1or2.chance", 0.25),
        ("loot.batch.t.Pool_Money_1or2.chance_max", 0.5),
        ("loot.batch.t.Pool_Money_1or2.tried", 2.0),
        ("loot.batch.t.Pool_Money_1or2.true", 1.0),
        ("loot.batch.t.Pool_Money_1or2.items", 3.0),
        ("loot.batch.t.Pool_Money_1or2.dropped", 2.0),
        ("loot.batch.t.Pool_Money_1or2.qty.1", 1.0),
        ("loot.batch.t.Pool_Money_1or2.qty.2", 1.0),
    ];
    let expect: Vec<(String, f64)> = expect.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    assert_eq!(got, expect);
}

// ---------------------------------------------------------------------------------------------------------------
// The loot-spawn behavior handlers
// ---------------------------------------------------------------------------------------------------------------

// The invented provider packages of the `ow-behavior` tests (that crate's file; it uses only `ow_package::testkit`).
#[allow(dead_code)]
#[path = "../../../ow-behavior/src/kernel/fixtures/packages.rs"]
mod packages;

static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A VM in `mode` over the `ow-behavior` fixture's install, its `WillowGame` replaced by one that declares
/// `Behavior_SpawnLoot`, and the provider `ToyLoot`: event `Ping` runs that behavior, then `After`. Loot is enabled.
fn spawn_loot_provider(mode: ParityMode) -> (Vm, ow_behavior::kernel::Provider, crate::run::tests::Dir) {
    use packages::{Seq, Spec, beh, ev, install, provider_package};
    let main = Seq {
        name: "Main",
        enabled_on_spawn: Some(true),
        events: vec![ev("Ping", (0, 1))],
        behaviors: vec![
            beh("Loot", "Behavior_SpawnLoot", (1, 1)),
            beh("After", "Behavior_SpawnLoot", (0, 0)),
        ],
        links: vec![(0, -1, 0.0), (1, -1, 0.0)],
        ..Seq::default()
    };
    let mut files = install(&[("ToyLoot", provider_package(&Spec::new("ToyLoot", vec![main], vec![])))]);
    let mut d = ow_package::testkit::reflect::Declarations::new(&["Object", "Struct", "Bool"]);
    let core = d.p.add_import("Core", "Package", 0, "Core");
    let object = d.p.add_import("Core", "Class", core, "Object");
    let context = d.struct_("ToyContext", None);
    d.prop("Bool", context, "bSupportsDefaultOutputLink", 0);
    let class = d.cls("Behavior_SpawnLoot", object);
    d.prop("Struct", class, "Context", context);
    for f in &mut files {
        if f.0 == "WillowGame.upk" {
            f.1 = d.build();
        }
    }
    let dir = std::env::temp_dir().join(format!(
        "ow-combat-loot-behavior-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    let defs =
        ow_defs::DefStore::from_synthetic(&dir, &refs, ow_defs::identity::IdentityPolicy::FirstInScanOrder, mode)
            .unwrap();
    let mut vm = Vm::new(
        ow_object::heap::ObjectSpace::new(),
        defs,
        std::rc::Rc::new(ow_attr::AttributeSystem::new_empty()),
        mode,
    );
    ow_vm::natives_core::register(&mut vm).unwrap();
    enable(&mut vm).unwrap();
    let definition = crate::run::tests::def(&mut vm, "ToyLoot");
    let p = ow_behavior::kernel::Provider::new(&mut vm, definition).unwrap();
    (vm, p, crate::run::tests::Dir(dir))
}

#[test]
fn a_loot_spawn_behavior_is_unsupported_on_its_branch_and_the_event_goes_on_in_both_modes() {
    for mode in [ParityMode::Reference, ParityMode::Off] {
        let (mut vm, mut p, _dir) = spawn_loot_provider(mode);
        // Nothing is registered for the loot-spawn classes: the registry's default answers.
        for class in SPAWN_BEHAVIORS {
            assert_eq!(
                ow_behavior::registry::lookup(&vm, class).unwrap_err().missing,
                Missing::NotPorted,
                "{class}"
            );
        }
        // The event returns: the branch stops at the unsupported behavior (recorded with its NotYet), not the event.
        p.fire_event(&mut vm, "Ping", &ow_core::DetMap::new(), -1, &[]).unwrap();
        assert_eq!(
            p.errors,
            vec!["unsupported behavior class WillowGame.Behavior_SpawnLoot (Loot)".to_string()],
            "{mode:?}"
        );
        assert_eq!(p.not_yet.len(), 1);
        assert_eq!(p.not_yet[0].missing, Missing::NotPorted);
        assert!(p.trace.contains(&"Ping -> Loot".to_string()), "{:?}", p.trace);
        assert!(!p.trace.contains(&"Ping -> After".to_string()), "{:?}", p.trace);
        // And a second event still runs.
        p.fire_event(&mut vm, "Ping", &ow_core::DetMap::new(), -1, &[]).unwrap();
        assert_eq!(p.errors.len(), 2);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// One owner for the pool native's key, and no half-done enable (review r1 of PR #340)
// ---------------------------------------------------------------------------------------------------------------

/// The key the fixture's package set declares for `path`.
fn key_of(vm: &mut Vm, path: &str) -> String {
    declared_native(vm, path).expect("declared native").1
}

#[test]
fn the_pool_native_is_the_loot_one_whichever_pack_registers_first() {
    for mode in [ParityMode::Reference, ParityMode::Off] {
        // ow-inventory's natives first, then loot. The fixture's key differs from the installed one (`object` where
        // the installed `SpawnedInventory` is an `array`), so this order test cannot put both packs on one key; it
        // shows that ow-inventory leaves the fixture's key unbound and loot then binds it.
        let (mut w, _dir) = world(mode);
        ow_inventory::spawn::register(&mut w.vm).unwrap();
        let key = key_of(&mut w.vm, FROM_POOL);
        assert_ne!(key, ow_inventory::spawn::FROM_POOL);
        assert!(!w.vm.has_native(&key), "ow-inventory binds nothing on the fixture's key");
        enable(&mut w.vm).unwrap();
        let key = key_of(&mut w.vm, FROM_POOL);
        assert!(w.vm.has_native(&key));
        assert!(!w.vm.has_native(ow_inventory::spawn::FROM_POOL));
        // Loot first, then ow-inventory's natives.
        let (mut w, _dir) = world(mode);
        enable(&mut w.vm).unwrap();
        ow_inventory::spawn::register(&mut w.vm).unwrap();
        let key = key_of(&mut w.vm, FROM_POOL);
        assert!(w.vm.has_native(&key));
        assert!(w.vm.has_native(ow_inventory::spawn::FROM_BALANCE));
    }
}

#[test]
fn enable_refuses_a_loot_key_another_pack_bound_before_binding_anything() {
    for path in [FROM_POOL, SETUP] {
        let (mut w, _dir) = world(ParityMode::Reference);
        let key = key_of(&mut w.vm, path);
        let other: ow_vm::api::NativeFn = std::rc::Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::None));
        w.vm.register_native(&key, other).unwrap();
        let before = w.vm.rng.crt(REFERENCE_STREAM).clone();
        for _ in 0..2 {
            // A second call is refused too: no half-done state answers Ok.
            match enable(&mut w.vm).unwrap_err() {
                VmError::Registration { key: k, .. } => assert_eq!(k, key),
                other => panic!("{other:?}"),
            }
        }
        assert!(w.vm.service::<LootState>().is_none());
        assert_eq!(*w.vm.rng.crt(REFERENCE_STREAM), before, "no reseed");
        // The other key was not bound either (every key is checked before any is bound).
        let other_path = if path == FROM_POOL { SETUP } else { FROM_POOL };
        let other_key = key_of(&mut w.vm, other_path);
        assert!(!w.vm.has_native(&other_key), "{path}");
    }
    // After a success, a second call does nothing.
    let (mut w, _dir) = world(ParityMode::Reference);
    enable(&mut w.vm).unwrap();
    enable(&mut w.vm).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------
// What the Reference death roll catches (review r1 of PR #340)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_not_yet_in_a_pool_chance_stops_the_reference_death_roll() {
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("player:8");
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Unread");
    run.apply("hit:0:10::Bullet");
    let errors = run.errors().join("\n");
    assert!(errors.starts_with("hit:0:10::Bullet: "), "{errors}");
    assert!(errors.contains("ToyUnreadValueResolver"), "{errors}");
    assert_eq!(drop_count(&run.world().vm).unwrap(), 0, "no roll recorded");
}

#[test]
fn a_data_error_in_a_pool_quantity_is_not_evaluated_and_the_reference_roll_goes_on() {
    // Pool_Bare's quantity definition lacks ValueFormula and the other fields the Rust evaluator reads: an attribute
    // system data error (the C++ reads the fields as absent). Reference reports the pool, as the C++ reports what it
    // catches, and rolls the next pool.
    let (mut run, _dir) = fresh(ParityMode::Reference);
    run.apply("player:8");
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Bare");
    run.apply("hit:0:10::Bullet");
    assert!(!run.failed(), "{:?}", run.errors());
    let drops = drops(&run.world().vm).unwrap();
    assert_eq!(drops.len(), 1);
    let pools = &drops[0].pools;
    assert_eq!(pools.len(), 2);
    assert_eq!(leaf(&pools[0].pool), "Pool_Bare");
    assert!(pools[0].error.contains("ValueFormula"), "{:?}", pools[0]);
    assert!(!pools[0].tried && pools[0].items.is_empty());
    assert_eq!(leaf(&pools[1].pool), "Pool_Coins");
    assert!(pools[1].error.is_empty() && pools[1].tried && pools[1].items.len() == 3, "{:?}", pools[1]);
    let events = with_state(&run.world().vm, |s| s.events.clone()).unwrap();
    assert!(events.iter().any(|e| e.starts_with("loot Pool_Bare: not evaluated (") && e.contains("ValueFormula")), "{events:?}");
}

#[test]
fn a_rule_error_is_reported_on_its_pool_and_keeps_the_items_made_before_it() {
    // Pool_Partial: four picks between a coin and Pool_Self, whose picks nest until the depth bound (a rule error).
    // Some seed makes a coin before the first nesting pick.
    let mut kept = None;
    for seed in 0..64u32 {
        let (mut run, _dir) = fresh(ParityMode::Reference);
        run.apply(&format!("seed:{seed}"));
        run.apply("player:8");
        run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Partial");
        run.apply("hit:0:10::Bullet");
        assert!(!run.failed(), "{:?}", run.errors());
        let drops = drops(&run.world().vm).unwrap();
        let pool = &drops[0].pools[0];
        assert_eq!(pool.error, "combat loot: item pools nest too deep");
        assert!(!pool.tried);
        if !pool.items.is_empty() {
            kept = Some(pool.items.clone());
            break;
        }
    }
    let kept = kept.expect("a seed with an item before the error");
    assert!(kept.iter().all(|i| i.balance == "Bal_Coin" && !i.dropped), "{kept:?}");
}

#[test]
fn the_drop_loot_override_body_is_not_yet_in_off_mode() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Loot:4:6:Balance_Toy");
    let pawn = run.enemy(0).unwrap().pawn;
    let vm = &mut run.world().vm;
    match drop_loot(vm, pawn).unwrap_err() {
        VmError::NotYet(n) => assert_eq!(n.missing, Missing::NotPorted),
        other => panic!("{other:?}"),
    }
    assert_eq!(drop_count(vm).unwrap(), 0);
}

// ---------------------------------------------------------------------------------------------------------------
// The local player's pool (lane FIX-ATTR-RESOLVERS, decision 2026-11-16-73)
// ---------------------------------------------------------------------------------------------------------------

/// The local player's pool through the world's query, from `source`.
fn local_pool(run: &mut crate::run::Run, resource: ObjRef, source: ObjRef) -> Result<Option<ObjRef>, ow_attr::api::AttrError> {
    use ow_attr::api::{AttrWorld, EngineQuery};
    run.world().vm.engine_query(EngineQuery::LocalPlayerPool { resource }, source)
}

#[test]
fn the_local_player_pool_is_the_first_players_health_pool_for_its_resource_in_reference_only() {
    let (mut run, _d) = fresh(ParityMode::Reference);
    run.apply("spawn:ToyPawns/Pawn_Toy:4");
    let pawn = run.enemy(0).expect("spawned").pawn;
    let vm = &mut run.world().vm;
    let pool = ow_world::pools::health_pool(&vm.space, pawn).unwrap().expect("the toy pawn's health pool");
    let definition = crate::hit::access::object(vm, pool, "Definition").unwrap().expect("a pool definition");
    let resource = crate::hit::access::object(vm, definition, "Resource").unwrap().expect("a resource");
    // No player yet: nothing.
    assert_eq!(local_pool(&mut run, resource, pawn).unwrap(), None);
    // A player whose pawn holds that pool (the toy pawn stands in for the player's): its pool, for its resource only.
    run.apply("player:8");
    let vm = &mut run.world().vm;
    let controller = with_state(vm, |s| s.players[0]).unwrap();
    crate::hit::access::set(vm, controller, "Pawn", Value::Object(Some(pawn))).unwrap();
    assert_eq!(local_pool(&mut run, resource, pawn).unwrap(), Some(pool));
    assert_eq!(local_pool(&mut run, definition, pawn).unwrap(), None, "another resource: none");
}

#[test]
fn off_mode_registers_no_local_player_pool() {
    let (mut run, _d) = fresh(ParityMode::Off);
    let vm = &mut run.world().vm;
    let actor = vm.find_class("Engine.Actor").unwrap();
    let o = ow_vm::interp::instantiate_from_defaults(vm, actor).unwrap();
    match local_pool(&mut run, o, o) {
        Err(ow_attr::api::AttrError::NotYet(n)) => assert!(matches!(n.missing, Missing::LayerAbsent(_))),
        other => panic!("{other:?}"),
    }
}
