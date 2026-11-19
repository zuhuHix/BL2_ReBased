//! Owner: lane SLICE-RUN-OFF. Tests of `--slice-gate`'s grading and row format (no package is read).

use super::*;

fn ran_ok(steps: &[&str]) -> Option<Ran> {
    Some(Ran {
        steps: steps.iter().map(|s| (*s).to_string()).collect(),
        stop: None,
        error_lines: Vec::new(),
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
fn links_without_a_driver_are_not_yet_with_their_reason() {
    let rows = rows(&vec![None; LINKS.len()], Grading::Strict, |_| true);
    for l in ["reach_fight_area", "pick_up_gun", "pick_up_aimed_not_nearest", "equip_gun", "phaselock_enemy"] {
        let r = &rows[index(l)];
        assert_eq!((r.result, r.driver), (Outcome::NotYet, "none"), "{l}");
        assert!(r.stop.as_ref().unwrap().text.starts_with("no headless driver"), "{l}");
    }
    // A driven link that did not run is an error, never a silent ok.
    assert_eq!(rows[index("spawn_maya")].result, Outcome::Error);
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
    // The five links without a driver keep it from passing.
    assert_eq!(exit_code(&rows_ok), NOT_PORTED);
    let mut fourteen = rows_ok.clone();
    for r in &mut fourteen {
        r.result = Outcome::Ok;
    }
    assert_eq!(exit_code(&fourteen), 0);
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
    };
    assert_eq!(
        row.json(),
        "{\"n\":3,\"link\":\"kill_enemy\",\"grading\":\"strict\",\"result\":\"notyet\",\"driver\":\"combat-run\",\
         \"steps\":[\"spawn:baby:6\"],\"stop\":{\"step\":\"spawn:baby:6\",\"native\":\"A.B\",\"record\":\"R.md\",\
         \"text\":\"t \\\"q\\\"\"},\"blocked_by\":null,\"after_errors_in\":null,\
         \"error_lines\":[{\"step\":\"use\",\"text\":\"e\"}],\"scope\":\"s\"}"
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
    });
    let n = NotYet::not_read("Pawn routing", "NATIVE_AI_HOLDS.md");
    ran[index("mission_objective")] = Some(Ran {
        steps: vec!["hit:fire".into()],
        stop: Some((Outcome::NotYet, stop_of_not_yet("hit:fire", &n))),
        error_lines: Vec::new(),
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
