//! Owner: lane W7-AI (part 1). The headless awareness run on the installed game (ignored by default: it needs
//! `OPENWILLOW_BL2`; run with `cargo test -p ow-ai --release --test installed_awareness -- --ignored`).
//! AI-assisted (Claude Code).
//!
//! A stock regular bullymong (`--combat-run`'s `regular`, level 5) and a level-8 player pawn are spawned the way
//! `--combat-run` spawns them, the player is placed around the bullymong, and the AI ticks in frames of 1/16 s for
//! 1.25 s per placement. The output (`local/p2/w7-ai-awareness/installed_run.json`, ignored) lists, per case, whether
//! the bullymong became aware (threat level 2 or more), its target and the record's numbers, with the rule's
//! expectation.
//!
//! **Set-ups of the run, not rules** (decision 2026-11-16-59):
//! - The run is in Reference mode: in Off the stock spawn stops on a definition-store limit outside this lane (a
//!   reference into a class default object does not load). The awareness rules are the same in both modes.
//! - The component's `AIDef` and `TargetingDef` are set as the live game shows them (the AI class's `AIDef` and its
//!   `TargetingDef`): the native that binds them is not read.
//! - The host answers the exposure per case (1.0 clear, 0.0 blocked): there is no level geometry in a headless run.
//! - The cooked targeting definition weights `WillowTI_PrioritizeIdle` 0.5 and `WillowTI_PrioritizeAttacking` 0.25;
//!   the live game read 0 and 0 (L8). The run is made twice: on the cooked data (expected `NotYet`, the two iterators'
//!   inputs are not read), and with the live weights written in memory.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use ow_ai::awareness::vm::{Host, mind_attacked_by, state, tick_mind};
use ow_ai::awareness::{ThreatLevel, Vec3};
use ow_attr::registry::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_combat::hit::access::{array, get, object, set, set_field};
use ow_core::ParityMode;
use ow_core::fmt::fmt_g;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::world::{BalanceState, World};

fn cooked() -> Option<PathBuf> {
    let root = std::env::var_os("OPENWILLOW_BL2")?;
    let p = PathBuf::from(root).join("WillowGame").join("CookedPCConsole");
    p.is_dir().then_some(p)
}

/// The world `--combat-run` builds (ow-cli `cmd/combat.rs` `world`).
fn world(cooked: PathBuf, mode: ParityMode) -> World {
    let defs = DefStore::open(cooked, IdentityPolicy::FirstInScanOrder, mode).expect("store");
    let attrs = AttributeSystem::standard().expect("attributes");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode).with_stub_policy(StubPolicy::LogAndZero);
    ow_vm::natives_core::register(&mut vm).expect("core natives");
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0).unwrap(),
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap();
    let pack = ow_combat::hit::Pack;
    let packs: [&dyn NativePack; 1] = [&pack];
    World::new(vm, BalanceState::new(snapshot), &packs).expect("world")
}

/// The run's host: the player and the bullymong are the registered targetables (the AI's own pawn must be one,
/// `ValidTarget` step 7); the exposure is the case's.
struct RunHost {
    player: ObjRef,
    ai: ObjRef,
    exposure: f32,
}

impl Host for RunHost {
    fn exposure(&mut self, _vm: &mut Vm, _viewer: ObjRef, _target: ObjRef) -> Result<f32, VmError> {
        Ok(self.exposure)
    }
    fn targetables(&mut self, _vm: &mut Vm) -> Result<Vec<ObjRef>, VmError> {
        Ok(vec![self.player, self.ai])
    }
    fn can_be_targeted(&mut self, _vm: &mut Vm, _actor: ObjRef) -> Result<bool, VmError> {
        Ok(true)
    }
}

struct Case {
    name: &'static str,
    at: Vec3,
    exposure: f32,
    shot: bool,
    /// The rule's answer: aware (level 2 or more) and targeted at the end.
    expect_aware: bool,
}

const CASES: [Case; 8] = [
    Case { name: "front_3000_clear", at: [3000.0, 0.0, 0.0], exposure: 1.0, shot: false, expect_aware: true },
    Case { name: "front_4200_clear", at: [4200.0, 0.0, 0.0], exposure: 1.0, shot: false, expect_aware: false },
    Case { name: "front_3000_blocked", at: [3000.0, 0.0, 0.0], exposure: 0.0, shot: false, expect_aware: false },
    Case { name: "behind_3000_clear", at: [-3000.0, 0.0, 0.0], exposure: 1.0, shot: false, expect_aware: false },
    // Exactly side-on (dot 0): the cone test is strict and the class default SightViewCone is 0.0, so not inside.
    // The live mind read -4.37e-8 (L9), which would put dot 0 inside: the cone edge is open question 17.
    Case { name: "side_3000_clear", at: [0.0, 3000.0, 0.0], exposure: 1.0, shot: false, expect_aware: false },
    Case { name: "behind_800_blocked", at: [-800.0, 0.0, 0.0], exposure: 0.0, shot: false, expect_aware: true },
    Case { name: "behind_900_blocked", at: [-900.0, 0.0, 0.0], exposure: 0.0, shot: false, expect_aware: false },
    Case { name: "behind_6000_shot", at: [-6000.0, 0.0, 0.0], exposure: 0.0, shot: true, expect_aware: true },
];

fn place(vm: &mut Vm, actor: ObjRef, at: Vec3) {
    let mut v = get(vm, actor, "Location").unwrap().expect("Location");
    for (axis, x) in ["X", "Y", "Z"].iter().zip(at) {
        assert!(set_field(vm, &mut v, axis, Value::Float(x)));
    }
    set(vm, actor, "Location", v).unwrap();
}

fn g(x: f32) -> String {
    fmt_g(f64::from(x), 7)
}

/// One case on a fresh run; `live` writes the live iterator weights first. Returns the case's JSON object.
fn one(cooked: PathBuf, case: &Case, live: bool) -> String {
    let world = world(cooked, ParityMode::Reference);
    let mut run = ow_combat::run::Run::new(world).expect("run");
    run.apply("spawn:regular:5");
    run.apply("player:8");
    assert!(run.errors().is_empty(), "{:?}", run.errors());
    let (enemy, player) = (run.enemy(0).unwrap(), run.player(0).unwrap());
    let vm = &mut run.world().vm;
    let component = object(vm, enemy.mind, "AIComponent").unwrap().expect("component");
    let class = object(vm, enemy.pawn, "AIClass").unwrap().expect("AI class");
    let aidef = object(vm, class, "AIDef").unwrap().expect("AIDef");
    let targeting = object(vm, aidef, "TargetingDef").unwrap().expect("TargetingDef");
    set(vm, component, "AIDef", Value::Object(Some(aidef))).unwrap();
    set(vm, component, "TargetingDef", Value::Object(Some(targeting))).unwrap();
    if live {
        for item in array(vm, targeting, "Prioritization").unwrap() {
            let Value::Object(Some(it)) = item else { continue };
            let name = ow_combat::hit::access::class_name(vm, it);
            if name == "WillowTI_PrioritizeIdle" || name == "WillowTI_PrioritizeAttacking" {
                set(vm, it, "Weight", Value::Float(0.0)).unwrap();
            }
        }
    }
    let globals = ow_combat::hit::stock_object(vm, ow_ai::awareness::vm::GLOBALS_PATH).expect("globals");
    place(vm, enemy.pawn, [0.0, 0.0, 0.0]);
    place(vm, player.pawn, case.at);
    let mut host = RunHost { player: player.pawn, ai: enemy.pawn, exposure: case.exposure };
    let mut outcome = String::from("ok");
    let mut t = 0.0f32;
    while t < 1.25 {
        t += 0.0625;
        vm.clock.advance(0.0625);
        if case.shot
            && t == 0.5
            && let Err(e) = mind_attacked_by(vm, &mut host, enemy.mind, globals, Some(player.pawn), true)
        {
            outcome = format!("error: {e}");
            break;
        }
        if let Err(e) = tick_mind(vm, &mut host, enemy.mind, globals) {
            outcome = format!("error: {e}");
            break;
        }
    }
    let s = state(vm, enemy.mind);
    let record = s.as_ref().and_then(|s| s.record(player.pawn).cloned());
    let level = record.as_ref().map_or(ThreatLevel::NoThreat, |r| r.threat);
    let target = s.as_ref().and_then(|s| s.current) == Some(player.pawn);
    let aware = level.seen();
    let mut out = String::new();
    write!(
        out,
        "{{\"case\":\"{}\",\"weights\":\"{}\",\"outcome\":{:?},\"record\":{},\"level\":\"{}\",\"aware\":{},\
         \"target_is_player\":{},\"expect_aware\":{},\"agrees\":{}",
        case.name,
        if live { "live" } else { "cooked" },
        outcome,
        record.is_some(),
        level.name(),
        aware,
        target,
        case.expect_aware,
        outcome == "ok" && aware == case.expect_aware && target == case.expect_aware,
    )
    .unwrap();
    if let Some(r) = &record {
        write!(
            out,
            ",\"distance\":{},\"dot_to_target\":{},\"exposure\":\"{:?}\",\"priority\":{},\"aggro_time\":{},\
             \"forget_time\":{}",
            g(r.distance),
            g(r.dot_to_target),
            r.exposure,
            g(r.priority),
            g(r.aggro_time),
            g(r.forget_time),
        )
        .unwrap();
    }
    if let Some(s) = s.as_ref().map(|s| s.settings.clone()) {
        write!(
            out,
            ",\"search_radius\":{},\"close_threat_radius\":{},\"collision_radius\":\"{:?}\",\"sight_view_cone\":{},\
             \"update_rate\":{},\"seen_time\":{},\"provoked_time\":{}",
            g(s.search_radius),
            g(s.close_threat_radius),
            s.collision_radius,
            g(s.sight_view_cone),
            g(s.update_rate),
            g(s.seen_time),
            g(s.provoked_time),
        )
        .unwrap();
    }
    let provoked = get(vm, enemy.mind, "bProvoked").unwrap();
    let counter = get(vm, player.pawn, "NumAICurrentlyTargeting").unwrap();
    write!(out, ",\"mind_bProvoked\":\"{provoked:?}\",\"player_NumAICurrentlyTargeting\":\"{counter:?}\"}}").unwrap();
    out
}

#[test]
#[ignore = "needs the installed game (OPENWILLOW_BL2)"]
fn the_stock_bullymong_notices_the_player() {
    let Some(cooked) = cooked() else { return };
    let mut rows = Vec::new();
    for live in [false, true] {
        for case in &CASES {
            let row = one(cooked.clone(), case, live);
            println!("{row}");
            rows.push(row);
        }
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w7-ai-awareness");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("installed_run.json"), format!("[\n{}\n]\n", rows.join(",\n"))).unwrap();
    // The live-weight rows all follow the rule; the cooked ones stop on the unread iterators.
    for row in &rows {
        if row.contains("\"weights\":\"live\"") {
            assert!(row.contains("\"agrees\":true"), "{row}");
        }
    }
}
