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
    assert!(matches!(equip.evidence, Evidence::Driver(_)) && matches!(cast.evidence, Evidence::NotChosen(_)));
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
    // Link 9 is never ok, even with its driver's flag set: no evidence check is chosen.
    ran[c] = Some(Ran { evident: true, ..ran_ok(&["spawn", "cast"]).unwrap() });
    assert!(evident(e, &ran, &status) && !evident(c, &ran, &status));
    let graded = rows(&ran, Grading::Strict, |i| evident(i, &ran, &status));
    assert_eq!(graded[e].result, Outcome::Ok);
    let cast_row = &graded[c];
    assert_eq!(cast_row.result, Outcome::Error, "a cast that returns is not evidence yet");
    assert!(cast_row.stop.as_ref().unwrap().text.contains("no evidence check is chosen yet"));
    // A stop is the row's reason, whatever the evidence flag says.
    let mut r = Ran { evident: true, ..ran_ok(&["make", "init", "bases", "equip"]).unwrap() };
    assert!(step_error(&mut r, "equip", "unbound native WillowWeapon.AllParts(x)", Grading::Strict));
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
    assert!(!step_error(&mut r, "equip", "unbound native A.B()", Grading::PastErrors));
    assert_eq!((r.stop.is_none(), r.error_lines.len()), (true, 1));
    let text = NotYet::not_read("A.C", "NATIVE_X.md").to_string();
    assert!(step_error(&mut r, "unequip", &text, Grading::PastErrors));
    assert_eq!(r.stop.as_ref().unwrap().0, Outcome::NotYet);
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
