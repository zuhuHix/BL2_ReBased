//! Owner: lane SLICE-RUN-OFF. Tests of `--slice-gate`'s grading and row format (no package is read).

use super::*;

fn ran_ok(steps: &[&str]) -> Option<Ran> {
    Some(Ran {
        steps: steps.iter().map(|s| (*s).to_string()).collect(),
        stop: None,
        error_lines: Vec::new(),
        new_rows: Vec::new(),
        evident: false,
        harness: Vec::new(),
    })
}

fn index(name: &str) -> usize {
    LINKS.iter().position(|l| l.name == name).unwrap()
}

#[test]
fn the_links_are_the_gate_s_fourteen_in_order() {
    // The same names, in the same order, as the existing checker's LINKS.
    let checker = include_str!("../../../../tools/check_slice_gate.py");
    let mut at = 0;
    for l in &LINKS {
        let needle = format!("('{}', ", l.name);
        let found = checker[at..].find(&needle).unwrap_or_else(|| panic!("{} in order", l.name));
        at += found + needle.len();
    }
    // A prerequisite comes earlier and has the same driver.
    for (i, l) in LINKS.iter().enumerate() {
        for need in l.needs {
            let j = index(need);
            assert!(j < i, "{} needs a later link", l.name);
            assert_eq!(LINKS[j].driver, l.driver, "{}", l.name);
        }
        let driven = !matches!(l.driver, Driver::None(_));
        assert_eq!(driven, !l.steps.is_empty() || l.name == "loot_drop_on_kill", "{}", l.name);
    }
}

#[test]
fn a_not_yet_text_is_split_into_its_subject_and_record() {
    let n = NotYet::not_read("ItemPool.SpawnBalancedInventoryFromPool", "NATIVE_LOOT.md");
    let (o, s) = classify("hit:0:1::Bullet", &format!("not yet: {n}"));
    assert_eq!(o, Outcome::NotYet);
    assert_eq!(s.native, "not yet: ItemPool.SpawnBalancedInventoryFromPool");
    assert_eq!(s.record, "NATIVE_LOOT.md");
    assert_eq!(s.step, "hit:0:1::Bullet");
    let n = NotYet::layer_absent("the pool", "ow-world pools", "lane X");
    let (o, s) = classify("maya:8", &n.to_string());
    assert_eq!((o, s.native.as_str(), s.record.as_str()), (Outcome::NotYet, "the pool", "lane X"));
    let n = NotYet::needs_real_game("crit base", 8, "COMBAT.md");
    assert_eq!(classify("x", &n.to_string()).0, Outcome::NotYet);
    let n = NotYet::not_ported("a rule", "src/x.cpp");
    assert_eq!(classify("x", &n.to_string()).1.record, "src/x.cpp");
}

#[test]
fn an_error_text_names_its_unbound_native_when_it_has_one() {
    let (o, s) = classify("use", "script: unbound native Actor.Foo(int,object) at step 3");
    assert_eq!(o, Outcome::Error);
    assert_eq!(s.native, "Actor.Foo(int,object)");
    assert_eq!(s.record, "");
    let (o, s) = classify("use", "a plain error");
    assert_eq!((o, s.native.as_str()), (Outcome::Error, ""));
}

#[test]
fn every_link_has_a_driver_and_one_that_did_not_run_is_an_error() {
    let rows = rows(&vec![None; LINKS.len()], Grading::Strict, |_| true);
    // Since links 2 (#453) and 6-7 (#452) every link has a headless driver; none is `none`.
    for (l, r) in LINKS.iter().zip(&rows) {
        assert_ne!(r.driver, "none", "{}", l.name);
    }
    // A driven link that did not run is an error, never a silent ok.
    assert_eq!(rows[index("spawn_maya")].result, Outcome::Error);
    for l in ["reach_fight_area", "equip_gun", "phaselock_enemy", "pick_up_gun", "pick_up_aimed_not_nearest"] {
        assert_eq!(rows[index(l)].result, Outcome::Error, "{l}");
    }
}

#[test]
fn link_2_walks_with_the_shared_mover_and_passes_in_the_fight_area() {
    let i = index("reach_fight_area");
    let l = &LINKS[i];
    assert_eq!(l.driver.name(), "sanctuary-walk");
    assert_eq!(l.steps, ["load", "spawn", "walk", "fight_area"]);
    assert!(l.needs.is_empty() && matches!(l.evidence, Evidence::Driver(_)));
    let status = vec![0u8; LINKS.len()];
    // The walk runs through the driver's own invocation on invented collision (AGENTS.md rule 9): a flat floor
    // 40,000 uu across, and Maya standing at its middle.
    use ow_assets::movement::collide::{Brush, CollisionWorld};
    use ow_assets::movement::mover::{MovementParams, Mover};
    use ow_assets::movement::sanctuary::scripted_walk;
    let (lo, hi) = ([-20_000.0, -20_000.0, -100.0], [20_000.0, 20_000.0, 0.0]);
    let points: Vec<[f64; 3]> = (0..8)
        .map(|k| {
            [
                if k & 4 == 0 { lo[0] } else { hi[0] },
                if k & 2 == 0 { lo[1] } else { hi[1] },
                if k & 1 == 0 { lo[2] } else { hi[2] },
            ]
        })
        .collect();
    let floor = CollisionWorld::new(vec![Brush::from_hull(&points, &[], 0).expect("a box")]);
    let maya = || Mover::spawn(MovementParams::maya(), [0.0, 0.0, 200.0], Default::default());
    // A passing walk leaves no stop and no evidence yet: the fight area (decision 2026-11-19-76) decides.
    let report = scripted_walk(&floor, &mut maya());
    assert!(report.passed(), "the walk on a flat floor passes its own checks");
    let mut r = ran_ok(&["load", "spawn", "walk"]).unwrap();
    walk_graded(&mut r, &report);
    assert!(r.stop.is_none() && !r.evident);
    // The fixture's area around an invented centre at Maya's end, the dummy placed there: the link is ok.
    let area = fight_area::FIGHT_AREA;
    let near = [report.end[0] + 10.0, report.end[1], report.end[2]];
    fight_area::graded(&mut r, &area, near, report.end, || Ok(near));
    assert!(r.evident, "{:?}", r.stop);
    assert_eq!(r.steps.last().map(String::as_str), Some("fight_area"));
    let mut ran = vec![None; LINKS.len()];
    ran[i] = Some(r);
    let graded = rows(&ran, Grading::Strict, |j| evident(j, &ran, &status));
    assert_eq!(graded[i].result, Outcome::Ok, "{:?}", graded[i].stop);
    // The same walk with the area's centre far from Maya's end: an error at `fight_area`, not ok.
    let mut r = ran_ok(&["load", "spawn", "walk"]).unwrap();
    walk_graded(&mut r, &report);
    let far = [report.end[0] + area.radius + 1.0, report.end[1], report.end[2]];
    fight_area::graded(&mut r, &area, far, report.end, || Ok(far));
    ran[i] = Some(r);
    let graded = rows(&ran, Grading::Strict, |j| evident(j, &ran, &status));
    let stop = graded[i].stop.as_ref().unwrap();
    assert_eq!((graded[i].result, stop.step.as_str()), (Outcome::Error, "fight_area"));
    // A walk that fails its own checks is an error at `walk`, with no evidence: with no floor, Maya falls through.
    let report = scripted_walk(&CollisionWorld::new(vec![]), &mut maya());
    assert!(!report.passed() && report.fell_through);
    let mut r = ran_ok(&["load", "spawn", "walk"]).unwrap();
    walk_graded(&mut r, &report);
    assert!(!r.evident);
    ran[i] = Some(r);
    let graded = rows(&ran, Grading::Strict, |j| evident(j, &ran, &status));
    let stop = graded[i].stop.as_ref().unwrap();
    assert_eq!(
        (graded[i].result, stop.step.as_str()),
        (Outcome::Error, "walk")
    );
    assert!(stop.text.contains("fell through true"), "{}", stop.text);
}

#[test]
fn links_6_and_7_have_the_pickup_drivers_and_their_own_evidence() {
    let pickup = &LINKS[index("pick_up_gun")];
    assert_eq!(pickup.driver.name(), "pickup");
    assert_eq!(pickup.steps, ["make", "touch", "key"]);
    assert!(pickup.needs.is_empty());
    assert!(matches!(pickup.evidence, Evidence::Driver(_)));
    let aimed = &LINKS[index("pick_up_aimed_not_nearest")];
    assert_eq!(aimed.driver.name(), "pickup-aimed");
    assert_eq!(aimed.steps, ["make", "touch", "look", "key"]);
    assert!(aimed.needs.is_empty() && matches!(aimed.evidence, Evidence::Driver(_)));
}

#[test]
fn links_8_and_9_have_their_own_drivers_and_their_own_evidence() {
    let (equip, cast) = (&LINKS[index("equip_gun")], &LINKS[index("phaselock_enemy")]);
    assert_eq!((equip.driver.name(), cast.driver.name()), ("equip-run", "phaselock-cast"));
    assert_eq!(equip.steps, ["make", "init", "bases", "equip", "unequip"], "the spawn's init runs first");
    assert_eq!(cast.steps, ["spawn", "cast"], "the world spawns the cast's actors first (decision 2026-11-19-70)");
    assert!(equip.needs.is_empty() && cast.needs.is_empty());
    assert!(matches!(equip.evidence, Evidence::Driver(_)) && matches!(cast.evidence, Evidence::Driver(_)));
    // The weapon line parses as an --equip-run line.
    assert!(EQUIP_WEAPON.split_whitespace().count() > 3 && !EQUIP_WEAPON.contains("  "));
}

#[test]
fn a_driver_s_own_evidence_decides_and_a_stop_wins_over_it() {
    let mut ran = vec![None; LINKS.len()];
    let (e, c) = (index("equip_gun"), index("phaselock_enemy"));
    let status = vec![0u8; LINKS.len()];
    // Steps that ran with no stop: ok only when the driver's own check found its evidence (the shared `evident`).
    ran[e] = Some(Ran { evident: true, ..ran_ok(&["make", "init", "bases", "equip", "unequip"]).unwrap() });
    // Link 9 (lane LINK9-EVIDENCE-CHECK): its driver's own check decides, as link 8's does.
    ran[c] = Some(Ran { evident: false, ..ran_ok(&["spawn", "cast"]).unwrap() });
    assert!(evident(e, &ran, &status) && !evident(c, &ran, &status));
    let graded = rows(&ran, Grading::Strict, |i| evident(i, &ran, &status));
    assert_eq!(graded[e].result, Outcome::Ok);
    let cast_row = &graded[c];
    assert_eq!(cast_row.result, Outcome::Error, "a cast that returns without the skill's lift writes is not evidence");
    assert!(cast_row.stop.as_ref().unwrap().text.contains("LiftedPawn"), "{:?}", cast_row.stop);
    ran[c] = Some(Ran { evident: true, ..ran_ok(&["spawn", "cast"]).unwrap() });
    let graded = rows(&ran, Grading::Strict, |i| evident(i, &ran, &status));
    assert_eq!(graded[c].result, Outcome::Ok, "with its own evidence");
    // A stop is the row's reason, whatever the evidence flag says.
    let mut r = Ran { evident: true, ..ran_ok(&["make", "init", "bases", "equip"]).unwrap() };
    let unbound = ow_vm::error::VmError::UnboundNative("WillowWeapon.AllParts(x)".into());
    assert!(step_error_typed(&mut r, "equip", &unbound, Grading::Strict));
    ran[e] = Some(r);
    let graded = rows(&ran, Grading::Strict, |i| evident(i, &ran, &status));
    let stop = graded[e].stop.as_ref().unwrap();
    assert_eq!((graded[e].result, stop.step.as_str()), (Outcome::Error, "equip"));
    assert_eq!(stop.native, "WillowWeapon.AllParts(x)");
}

#[test]
fn the_equip_evidence_needs_new_records_none_left_and_no_error_line() {
    assert!(equip_evident(0, Some(0), Some(3), Some(0)));
    assert!(equip_evident(0, Some(1), Some(2), Some(0)), "one new record is enough");
    assert!(!equip_evident(0, Some(3), Some(3), Some(0)), "records already there before the equip are not new");
    assert!(!equip_evident(0, Some(0), Some(0), Some(0)), "an equip that added nothing");
    assert!(!equip_evident(0, Some(0), Some(3), Some(1)), "the unequip left one");
    assert!(!equip_evident(1, Some(0), Some(3), Some(0)), "an error line");
    assert!(!equip_evident(0, None, Some(3), Some(0)), "the equip step never ran");
    assert!(!equip_evident(0, Some(0), Some(3), None), "the unequip step never ran");
}

#[test]
fn past_errors_grading_notes_an_error_line_and_goes_on_but_a_not_yet_stops() {
    let mut r = Ran::default();
    let unbound = ow_vm::error::VmError::UnboundNative("A.B()".into());
    assert!(!step_error_typed(&mut r, "equip", &unbound, Grading::PastErrors));
    assert_eq!((r.stop.is_none(), r.error_lines.len()), (true, 1));
    let n = NotYet::not_read("A.C", "NATIVE_X.md");
    assert!(step_error_typed(&mut r, "unequip", &ow_vm::error::VmError::NotYet(n), Grading::PastErrors));
    let (outcome, stop) = r.stop.clone().unwrap();
    assert_eq!((outcome, stop.native.as_str(), stop.record.as_str()), (Outcome::NotYet, "A.C", "NATIVE_X.md"));
}

/// The equip driver grades by variant (review r3 of #468): in past-errors grading, a script error whose message reads
/// like a `NotYet`'s display is an error line, not a terminal not-yet stop.
#[test]
fn a_script_error_with_not_yet_text_is_an_error_line_in_past_errors() {
    let mut r = Ran::default();
    let looks = NotYet::not_read("A.C", "NATIVE_X.md").to_string();
    let fake = ow_vm::error::VmError::Script { function: "toy".into(), message: looks.clone() };
    assert!(!step_error_typed(&mut r, "equip", &fake, Grading::PastErrors), "goes on");
    assert!(r.stop.is_none());
    assert_eq!(r.error_lines, [("equip".to_string(), looks)]);
}

#[test]
fn a_stop_blocks_the_links_that_need_it_and_missing_evidence_is_an_error() {
    let mut ran = vec![None; LINKS.len()];
    ran[index("spawn_maya")] = ran_ok(&["maya:8", "players:1"]);
    let n = NotYet::not_read("ItemPool.Spawn", "NATIVE_LOOT.md");
    ran[index("kill_enemy")] = Some(Ran {
        steps: vec!["spawn:baby:6".into()],
        stop: Some((Outcome::NotYet, stop_of_not_yet("spawn:baby:6", &n))),
        error_lines: Vec::new(),
        new_rows: Vec::new(),
        evident: false,
        harness: Vec::new(),
    });
    ran[index("enemy_attacks_maya")] = ran_ok(&["spawn:adult:7", "swing:1:melee"]);
    ran[index("go_down")] = ran_ok(&["cash:3000"]);
    ran[index("mission_accept")] = ran_ok(&["accept"]);
    // Evidence: none for go_down (no down.total row), and the accept left the mission Active.
    let rows = rows(&ran, Grading::Strict, |i| LINKS[i].name != "go_down");
    let r = |name| &rows[index(name)];
    assert_eq!(r("spawn_maya").result, Outcome::Ok);
    assert_eq!(r("kill_enemy").result, Outcome::NotYet);
    let loot = r("loot_drop_on_kill");
    assert_eq!((loot.result, loot.blocked_by), (Outcome::NotYet, Some("kill_enemy")));
    assert_eq!(loot.stop.as_ref().unwrap().native, "ItemPool.Spawn");
    assert_eq!(r("enemy_attacks_maya").result, Outcome::Ok);
    let down = r("go_down");
    assert_eq!(down.result, Outcome::Error);
    assert!(down.stop.as_ref().unwrap().text.contains("down.total."));
    assert_eq!(r("bleed_out_respawn").blocked_by, Some("go_down"));
    assert_eq!(r("bleed_out_respawn").result, Outcome::Error);
    assert_eq!(r("mission_accept").result, Outcome::Ok);
    // The objective link did not run: an error, and the turn-in is blocked by it.
    assert_eq!(r("mission_objective").result, Outcome::Error);
    assert_eq!(r("mission_turn_in").blocked_by, Some("mission_objective"));
    assert_eq!(exit_code(&rows), 1);
}

#[test]
fn the_exit_code_is_zero_only_for_fourteen_ok() {
    let all_ok: Vec<Option<Ran>> = LINKS.iter().map(|_| ran_ok(&["x"])).collect();
    let rows_ok = rows(&all_ok, Grading::Strict, |_| true);
    // Every link has a driver since links 2 (#453) and 6-7 (#452): fourteen driven, evident runs pass.
    assert!(rows_ok.iter().all(|r| r.result == Outcome::Ok));
    assert_eq!(exit_code(&rows_ok), 0);
    // One not-yet row keeps it from passing, and one error makes it fail.
    let mut one_not_yet = rows_ok.clone();
    one_not_yet[0].result = Outcome::NotYet;
    assert_eq!(exit_code(&one_not_yet), NOT_PORTED);
    one_not_yet[1].result = Outcome::Error;
    assert_eq!(exit_code(&one_not_yet), 1);
}

#[test]
fn a_row_is_one_line_of_json_with_every_field() {
    let row = Row {
        n: 3,
        link: "kill_enemy",
        result: Outcome::NotYet,
        driver: "combat-run",
        steps: vec!["spawn:baby:6".into()],
        stop: Some(Stop {
            step: "spawn:baby:6".into(),
            native: "A.B".into(),
            record: "R.md".into(),
            text: "t \"q\"".into(),
        }),
        blocked_by: None,
        grading: Grading::Strict,
        after_errors_in: None,
        error_lines: vec![("use".into(), "e".into())],
        stopped: true,
        scope: "s",
        harness: vec!["harness input: x \"y\"".into()],
    };
    assert_eq!(
        row.json(),
        "{\"n\":3,\"link\":\"kill_enemy\",\"grading\":\"strict\",\"result\":\"notyet\",\"driver\":\"combat-run\",\
         \"steps\":[\"spawn:baby:6\"],\"stop\":{\"step\":\"spawn:baby:6\",\"native\":\"A.B\",\"record\":\"R.md\",\
         \"text\":\"t \\\"q\\\"\"},\"blocked_by\":null,\"after_errors_in\":null,\
         \"error_lines\":[{\"step\":\"use\",\"text\":\"e\"}],\"scope\":\"s\",\"harness\":[\"harness input: x \\\"y\\\"\"]}"
    );
    let ok = Row {
        stop: None,
        result: Outcome::Ok,
        blocked_by: Some("spawn_maya"),
        ..row
    };
    assert!(ok.json().contains("\"stop\":null,\"blocked_by\":\"spawn_maya\""));
    assert!(!ok.json().contains('\n'));
}

#[test]
fn pawn_placeholders_bind_to_the_spawned_index_or_refuse() {
    let bound = vec![("baby".to_string(), 0), ("adult".to_string(), 1)];
    assert_eq!(bind("swing:{adult}:melee", &bound).unwrap(), "swing:1:melee");
    assert_eq!(bind("hit:{baby}:9999::Bullet", &bound).unwrap(), "hit:0:9999::Bullet");
    assert!(bind("swing:{adult}:melee", &bound[..1]).is_err());
}

#[test]
fn past_errors_grading_lets_a_link_run_after_error_lines_but_not_after_a_hard_stop() {
    let mut ran = vec![None; LINKS.len()];
    // The accept went on past an error line; the objective stopped hard on a NotYet.
    ran[index("mission_accept")] = Some(Ran {
        steps: vec!["use".into(), "accept".into()],
        stop: None,
        error_lines: vec![("use".into(), "mission script: unbound native A.B(name)".into())],
        new_rows: Vec::new(),
        evident: false,
        harness: Vec::new(),
    });
    let n = NotYet::not_read("Pawn routing", "NATIVE_AI_HOLDS.md");
    ran[index("mission_objective")] = Some(Ran {
        steps: vec!["hit:fire".into()],
        stop: Some((Outcome::NotYet, stop_of_not_yet("hit:fire", &n))),
        error_lines: Vec::new(),
        new_rows: Vec::new(),
        evident: false,
        harness: Vec::new(),
    });
    let past = rows(&ran, Grading::PastErrors, |_| true);
    let r = |name| &past[index(name)];
    let accept = r("mission_accept");
    assert_eq!((accept.result, accept.stopped), (Outcome::Error, false));
    assert_eq!(accept.stop.as_ref().unwrap().native, "A.B(name)");
    let objective = r("mission_objective");
    assert_eq!(
        (objective.result, objective.blocked_by, objective.after_errors_in),
        (Outcome::NotYet, None, Some("mission_accept"))
    );
    assert_eq!(objective.stop.as_ref().unwrap().record, "NATIVE_AI_HOLDS.md");
    assert_eq!(r("mission_turn_in").blocked_by, Some("mission_objective"));
    assert!(accept.json().contains("\"grading\":\"past-errors\""));
    // Under strict grading the same error line blocks the objective.
    let strict = rows(&ran, Grading::Strict, |_| true);
    assert_eq!(strict[index("mission_accept")].result, Outcome::Error);
    assert_eq!(strict[index("mission_objective")].blocked_by, Some("mission_accept"));
}

/// A link's own rows (review r1 of #392): the kill's death roll writes `loot.<row class>.L<level>.<n>.*`
/// (`ow_combat::loot::loot_rows`, keyed by the hit's id), which is link 5's evidence; `loot.roll.*` comes only from a
/// `rollpool` step, which the gate does not run.
#[test]
fn link_5_is_graded_on_the_loot_rows_of_link_3_s_kill() {
    let kill_rows = vec![
        "hit.PrimalBeastBaby.L6.1.arriving".to_string(),
        "kill.PrimalBeastBaby.L6.p8".to_string(),
        "loot.PrimalBeastBaby.L6.1.item_stage".to_string(),
        "loot.PrimalBeastBaby.L6.1.pools_tried".to_string(),
    ];
    let ran = Ran { new_rows: kill_rows, ..Ran::default() };
    assert!(evidence_in(Some(&ran), "kill."));
    assert!(evidence_in(Some(&ran), "loot."));
    assert_eq!(LINKS[index("loot_drop_on_kill")].evidence, Evidence::Row("loot."));
    let no_roll = Ran { new_rows: vec!["kill.PrimalBeastBaby.L6.p8".into()], ..Ran::default() };
    assert!(!evidence_in(Some(&no_roll), "loot."), "a kill whose death roll made no rows");
    assert!(!evidence_in(None, "loot."));
}

/// The reviewer's case: link 4's swing lands with arrival 0 (no `ratio.` row, no error) and link 13's later swings by
/// the same adult write `ratio.` rows. Link 4 is graded on its own rows only: an error, and link 13 is blocked by it.
#[test]
fn a_link_is_not_graded_on_a_later_link_s_rows() {
    let mut ran = vec![None; LINKS.len()];
    ran[index("spawn_maya")] = ran_ok(&["maya:8", "players:1"]);
    ran[index("enemy_attacks_maya")] = Some(Ran {
        steps: vec!["spawn:adult:7".into(), "swing:0:melee".into()],
        new_rows: vec!["health.PrimalBeastAdult.L7".into(), "arrival.PrimalBeastAdult.L7".into()],
        harness: Vec::new(),
        ..Ran::default()
    });
    ran[index("go_down")] = Some(Ran {
        steps: vec!["swing:0:melee".into()],
        new_rows: vec!["arrival.PrimalBeastAdult.L7#2".into(), "ratio.PrimalBeastAdult.L7".into(), "down.total.1".into()],
        harness: Vec::new(),
        ..Ran::default()
    });
    let graded = rows(&ran, Grading::Strict, |i| match LINKS[i].evidence {
        Evidence::Row(prefix) => evidence_in(ran[i].as_ref(), prefix),
        _ => true,
    });
    let attack = &graded[index("enemy_attacks_maya")];
    assert_eq!(attack.result, Outcome::Error);
    assert!(attack.stop.as_ref().unwrap().text.contains("ratio."));
    assert_eq!(graded[index("go_down")].blocked_by, Some("enemy_attacks_maya"));
}

/// The per-link diff counts a repeated id each time (two kills of one class give the same kill id).
#[test]
fn new_ids_is_a_multiset_difference() {
    let before = vec!["kill.A.L1.p8".to_string(), "health.A.L1".to_string()];
    let after = ["health.A.L1", "kill.A.L1.p8", "kill.A.L1.p8", "loot.A.L1.2.items_dropped"];
    assert_eq!(new_ids(&before, after.into_iter()), vec!["kill.A.L1.p8".to_string(), "loot.A.L1.2.items_dropped".into()]);
    assert!(new_ids(&before, before.iter().map(String::as_str)).is_empty());
}

/// `--map` parsing (review r1 of #556, item 5; lane MAP-ROUTE-FOLLOWUP): last on the line, with either grading; any
/// other place or a missing name is the usage message.
#[test]
fn the_map_option_is_parsed_last() {
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let parse = |a: &[&str]| parse_args(&args(a));
    assert_eq!(parse(&["--slice-gate", "--cooked", "d"]), Some(("d".into(), Grading::Strict, None)));
    assert_eq!(
        parse(&["--slice-gate", "--cooked", "d", "--map", "Sanctuary_P"]),
        Some(("d".into(), Grading::Strict, Some("Sanctuary_P".into())))
    );
    assert_eq!(
        parse(&["--slice-gate", "--cooked", "d", "--past-errors", "--map", "Sanctuary_P"]),
        Some(("d".into(), Grading::PastErrors, Some("Sanctuary_P".into())))
    );
    assert_eq!(parse(&["--slice-gate", "--cooked", "d", "--map", "Sanctuary_P", "--past-errors"]), None, "--map is last");
    assert_eq!(parse(&["--slice-gate", "--cooked", "d", "--map"]), None, "no name");
    assert_eq!(parse(&["--slice-gate", "--map", "Sanctuary_P", "--cooked", "d"]), None);
}

/// A row of `name` (lane SLICE-GATE-STEP-SHARE): `result`, the steps it ran, and a stop at `stop` when given.
fn share_row(name: &'static str, result: Outcome, steps: &[&str], stop: Option<&str>) -> Row {
    Row {
        n: index(name) + 1,
        link: name,
        result,
        driver: "combat-run",
        steps: steps.iter().map(|s| (*s).to_string()).collect(),
        stop: stop.map(|s| Stop {
            step: s.into(),
            ..Stop::default()
        }),
        blocked_by: None,
        grading: Grading::Strict,
        after_errors_in: None,
        error_lines: Vec::new(),
        stopped: stop.is_some(),
        scope: "s",
        harness: Vec::new(),
    }
}

/// The fourteen rows in the shape of main d3c7a13's installed Off strict reading (invented stops; the step names are
/// the links' own): 4 ok, 10 NotYet.
fn share_rows() -> Vec<Row> {
    let all = |name: &str| LINKS[index(name)].steps.to_vec();
    let ny = Outcome::NotYet;
    vec![
        share_row("spawn_maya", ny, &["maya:8"], Some("maya:8")),
        share_row("reach_fight_area", Outcome::Ok, &all("reach_fight_area"), None),
        share_row("kill_enemy", ny, &[], Some("maya:8")),
        share_row("enemy_attacks_maya", ny, &[], Some("maya:8")),
        share_row("loot_drop_on_kill", ny, &[], Some("maya:8")),
        share_row("pick_up_gun", ny, &["make", "touch"], Some("touch")),
        share_row("pick_up_aimed_not_nearest", ny, &["make", "touch"], Some("touch")),
        share_row("equip_gun", ny, &["make", "init"], Some("init")),
        share_row("phaselock_enemy", ny, &["spawn", "cast"], Some("cast")),
        share_row("mission_accept", Outcome::Ok, &all("mission_accept"), None),
        share_row("mission_objective", Outcome::Ok, &all("mission_objective"), None),
        share_row("mission_turn_in", Outcome::Ok, &all("mission_turn_in"), None),
        share_row("go_down", ny, &[], Some("maya:8")),
        share_row("bleed_out_respawn", ny, &[], Some("maya:8")),
    ]
}

#[test]
fn the_step_totals_are_the_links_own_step_lists() {
    // The hub's hand-recorded totals, in gate order.
    let totals: Vec<usize> = share_rows().iter().map(|r| step_counts(r).1).collect();
    assert_eq!(totals, [2, 4, 2, 2, 0, 3, 4, 5, 2, 8, 8, 6, 4, 1]);
}

#[test]
fn the_step_report_gives_main_s_hand_count() {
    let rows = share_rows();
    let passed: Vec<usize> = rows.iter().map(|r| step_counts(r).0).collect();
    assert_eq!(passed, [0, 4, 0, 0, 0, 1, 1, 1, 1, 8, 8, 6, 0, 0]);
    let lines = step_report(&rows);
    assert_eq!(lines.len(), 16);
    assert_eq!(lines[0], "link 1 spawn_maya: notyet, steps 0/2");
    assert_eq!(lines[4], "link 5 loot_drop_on_kill: notyet, steps 0/0");
    assert_eq!(lines[7], "link 8 equip_gun: notyet, steps 1/5");
    assert_eq!(lines[9], "link 10 mission_accept: ok, steps 8/8");
    assert_eq!(lines[14], "links ok 4/14");
    // 1 + 1/3 + 1/4 + 1/5 + 1/2 + 3 = 5.283; over 14, 37.7%.
    assert_eq!(lines[15], "step share X = 5.283/14 = 37.7%");
}

#[test]
fn a_link_with_no_steps_counts_one_when_ok_and_the_report_changes_no_grading() {
    let mut rows = share_rows();
    let before: Vec<String> = rows.iter().map(Row::json).collect();
    let code = exit_code(&rows);
    let _ = step_report(&rows);
    assert_eq!(rows.iter().map(Row::json).collect::<Vec<_>>(), before, "the rows are unchanged");
    assert_eq!(exit_code(&rows), code);
    assert_eq!(step_share(&rows[4]), 0.0, "no steps, notyet: 0");
    rows[4].result = Outcome::Ok;
    rows[4].stop = None;
    assert_eq!(step_share(&rows[4]), 1.0, "no steps, ok: 1");
}

#[test]
fn past_errors_grading_drops_the_stop_and_the_error_steps() {
    let mut r = share_row("mission_turn_in", Outcome::NotYet, &["use", "turnin", "tick:0.1", "tick:1"], Some("turnin"));
    r.grading = Grading::PastErrors;
    r.error_lines = vec![("turnin".into(), "e".into()), ("tick:1".into(), "e".into())];
    assert_eq!(step_counts(&r), (2, 6));
    // Strict grading: only a stop on the last step is dropped.
    let s = share_row("mission_turn_in", Outcome::NotYet, &["use", "turnin"], Some("turnin"));
    assert_eq!(step_counts(&s), (1, 6));
    // A stop at a step that never ran drops nothing (review r1 of #674).
    let k = share_row("pick_up_gun", Outcome::NotYet, &["make", "touch"], Some("key"));
    assert_eq!(step_counts(&k), (2, 3));
}

/// Lane GATE-SETUP-HARNESS-LINES: a world's set-up harness lines (and only those) are kept, in order, each marked as
/// set-up, and a row that carries one shows it in its `harness` list.
#[test]
fn a_set_up_harness_line_reaches_the_row() {
    let log: Vec<String> = vec![
        "harness input: the engine object's PhysXLevel = 0 (an invented line)".into(),
        "a note that is not a harness input".into(),
        "harness input: no engine object; PhysXLevel not set".into(),
    ];
    let lines = setup_harness(&log, &[]);
    assert_eq!(
        lines,
        [
            "world set-up: harness input: the engine object's PhysXLevel = 0 (an invented line)",
            "world set-up: harness input: no engine object; PhysXLevel not set",
        ]
    );
    let mut row = share_row("spawn_maya", Outcome::NotYet, &["maya:8"], Some("maya:8"));
    row.harness = lines.clone();
    row.harness.push("harness input: a step's own line".into());
    let json = row.json();
    assert!(json.contains("\"harness\":[\"world set-up: harness input: the engine object's PhysXLevel = 0"), "{json}");
    assert!(json.ends_with("\"harness input: a step's own line\"]}"), "{json}");
}

/// Review of #777, item 1: on the `--map` route the route's notes are logged too; the set-up lines leave them out, so
/// the first combat row shows each once and the other rows not at all.
#[test]
fn the_map_route_s_notes_are_not_repeated_as_set_up_lines() {
    let route: Vec<String> = vec![
        "harness input: the world's globals (an invented route note)".into(),
        "harness input: the local player (an invented route note)".into(),
    ];
    let mut log = route.clone();
    log.push("harness input: the engine object's PhysXLevel = 0 (an invented line)".into());
    let setup = setup_harness(&log, &route);
    assert_eq!(setup, ["world set-up: harness input: the engine object's PhysXLevel = 0 (an invented line)"]);
    let steps = vec!["harness input: a step's own line".to_string()];
    let first = row_harness(Some(&route), &setup, &steps);
    assert_eq!(first, [route[0].as_str(), route[1].as_str(), setup[0].as_str(), steps[0].as_str()]);
    for line in &route {
        assert_eq!(first.iter().filter(|l| l.ends_with(line.as_str())).count(), 1, "{line} once");
    }
    let later = row_harness(None, &setup, &steps);
    assert_eq!(later, [setup[0].as_str(), steps[0].as_str()], "no route notes on a later row");
}

/// Review of #777, item 2: a row's harness list is the route's notes, then the set-up lines, then its steps' lines.
#[test]
fn a_row_s_harness_list_is_route_then_set_up_then_steps() {
    let setup = vec!["world set-up: harness input: s1".to_string(), "world set-up: harness input: s2".to_string()];
    let steps = vec!["harness input: t1".to_string(), "harness input: t2".to_string()];
    assert_eq!(row_harness(None, &setup, &steps), [&setup[..], &steps[..]].concat());
    assert_eq!(row_harness(None, &[], &steps), steps);
    assert_eq!(row_harness(None, &setup, &[]), setup);
    let route = vec!["harness input: r1".to_string()];
    assert_eq!(row_harness(Some(&route), &setup, &steps), [&route[..], &setup[..], &steps[..]].concat());
    assert_eq!(row_harness(Some(&[]), &setup, &steps), [&setup[..], &steps[..]].concat());
}
