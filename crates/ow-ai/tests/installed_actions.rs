//! Owner: lane W7-AI (part 2). The headless action-tree run on the installed game (ignored by default: it needs
//! `OPENWILLOW_BL2`; run with `cargo test -p ow-ai --release --test installed_actions -- --ignored`).
//! AI-assisted (Claude Code).
//!
//! A stock regular bullymong (`--combat-run`'s `regular`, level 5) and a level-8 player pawn, spawned the way
//! `--combat-run` spawns them, the player at four placements; the whole AI tick (awareness, the target events, the
//! tree) runs in frames of 1/16 s for 1.5 s, or until it stops on a `NotYet`. The output
//! (`local/p2/w7-ai-actions/installed_run.json`, ignored) lists, per placement, the actions started and stopped, the
//! events sent, where it stopped, the action the selection would pick then (with the `CanRun` failures), the movement
//! requests and the stub lines the actions' script reached. Every result is UNVERIFIED: no live record holds a lone
//! bullymong's action choices.
//!
//! **Set-ups of the run, not rules** (decisions 2026-11-16-59 and -66): Reference mode (the stock spawn stops in Off
//! on a definition-store limit outside this lane); `AIDef` and `TargetingDef` set as live; the live iterator weights
//! written in memory; the exposure per case; an open-field movement host (every path exists).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use ow_ai::awareness::Vec3;
use ow_ai::awareness::actions::{Locomotion, brain, dry_select, install_locomotion, tick_brain};
use ow_ai::awareness::vm::Host;
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
    let p = ow_core::install::cooked_or_join(&PathBuf::from(root)).unwrap();
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

/// The run's host: the player and the bullymong are the registered targetables; the exposure is the case's.
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

/// The run's movement host: an open field (every path exists, every point is on the nav mesh); it records requests.
struct RunMoves {
    requests: Vec<String>,
}

impl Locomotion for RunMoves {
    fn create_path(&mut self, _vm: &mut Vm, _handle: ObjRef, dest: Vec3) -> Result<bool, VmError> {
        self.requests.push(format!("path to {dest:?}"));
        Ok(true)
    }
    fn clear_path(&mut self, _vm: &mut Vm, _handle: ObjRef, _stop: bool) -> Result<bool, VmError> {
        self.requests.push("clear path".to_string());
        Ok(true)
    }
    fn path_is_valid(&mut self, _vm: &mut Vm, _pawn: ObjRef) -> Result<bool, VmError> {
        Ok(true)
    }
    fn invalidate_path(&mut self, _vm: &mut Vm, _pawn: ObjRef) -> Result<(), VmError> {
        self.requests.push("invalidate path".to_string());
        Ok(())
    }
    fn reached_destination(&mut self, _vm: &mut Vm, _pawn: ObjRef) -> Result<bool, VmError> {
        Ok(false)
    }
    fn can_move_to(&mut self, _vm: &mut Vm, _pawn: ObjRef, goal: Vec3) -> Result<bool, VmError> {
        self.requests.push(format!("can move to {goal:?}"));
        Ok(true)
    }
    fn set_movement_speed(&mut self, _vm: &mut Vm, _pawn: ObjRef, speed: u8) -> Result<(), VmError> {
        self.requests.push(format!("movement speed {speed}"));
        Ok(())
    }
    fn nearest_on_navmesh(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<Option<Vec3>, VmError> {
        let at = get(vm, pawn, "Location")?.expect("Location");
        let axis = |n: &str| {
            ow_combat::hit::access::field(vm, &at, n).map_or(0.0, |v| ow_combat::hit::access::number(&v) as f32)
        };
        Ok(Some([axis("X"), axis("Y"), axis("Z")]))
    }
}

struct Case {
    name: &'static str,
    at: Vec3,
    exposure: f32,
}

const CASES: [Case; 4] = [
    Case { name: "front_3000_clear", at: [3000.0, 0.0, 0.0], exposure: 1.0 },
    Case { name: "front_500_clear", at: [500.0, 0.0, 0.0], exposure: 1.0 },
    Case { name: "behind_800_blocked", at: [-800.0, 0.0, 0.0], exposure: 0.0 },
    Case { name: "front_6000_clear", at: [6000.0, 0.0, 0.0], exposure: 1.0 },
];

fn place(vm: &mut Vm, actor: ObjRef, at: Vec3) {
    let mut v = get(vm, actor, "Location").unwrap().expect("Location");
    for (axis, x) in ["X", "Y", "Z"].iter().zip(at) {
        assert!(set_field(vm, &mut v, axis, Value::Float(x)));
    }
    set(vm, actor, "Location", v).unwrap();
}

fn quote(s: &str) -> String {
    ow_core::fmt::json_quote(s)
}

/// One placement on a fresh run (Reference mode for the stock spawn; the live iterator weights written in memory, as
/// part 1's run). Returns the case's JSON object.
fn one(cooked: PathBuf, case: &Case) -> String {
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
    for item in array(vm, targeting, "Prioritization").unwrap() {
        let Value::Object(Some(it)) = item else {
            continue;
        };
        let name = ow_combat::hit::access::class_name(vm, it);
        if name == "WillowTI_PrioritizeIdle" || name == "WillowTI_PrioritizeAttacking" {
            set(vm, it, "Weight", Value::Float(0.0)).unwrap();
        }
    }
    let names: Vec<String> = array(vm, aidef, "NodeList")
        .unwrap()
        .iter()
        .map(|n| match ow_combat::hit::access::field(vm, n, "NodeName") {
            Some(Value::Str(s)) => s.to_string(),
            _ => String::new(),
        })
        .collect();
    let globals = ow_combat::hit::stock_object(vm, ow_ai::awareness::vm::GLOBALS_PATH).expect("globals");
    let host: Rc<RefCell<dyn Host>> =
        Rc::new(RefCell::new(RunHost { player: player.pawn, ai: enemy.pawn, exposure: case.exposure }));
    ow_ai::awareness::vm::install(vm, host, globals).unwrap();
    ow_ai::awareness::vm::Pack.register(vm).unwrap();
    let moves = Rc::new(RefCell::new(RunMoves { requests: Vec::new() }));
    install_locomotion(vm, moves.clone()).unwrap();
    ow_ai::awareness::actions::Pack.register(vm).unwrap();
    place(vm, enemy.pawn, [0.0, 0.0, 0.0]);
    place(vm, player.pawn, case.at);
    let mut tick_host = RunHost { player: player.pawn, ai: enemy.pawn, exposure: case.exposure };
    let log_start = vm.log.len();
    let mut steps = Vec::new();
    let mut stop = String::new();
    let mut t = 0.0f32;
    while t < 1.5 {
        t += 0.0625;
        vm.clock.advance(0.0625);
        match tick_brain(vm, &mut tick_host, enemy.mind, globals) {
            Ok(r) => {
                for c in &r.changes {
                    if let Some((n, failed)) = c.stopped {
                        steps.push(format!(
                            "{} stop {}{}",
                            fmt_g(f64::from(t), 4),
                            names[n],
                            if failed { " (failed)" } else { "" }
                        ));
                    }
                    if let Some(n) = c.started {
                        steps.push(format!("{} start {}", fmt_g(f64::from(t), 4), names[n]));
                    }
                }
                for (event, order) in &r.events {
                    let to: Vec<&str> = order.iter().map(|&i| names[i].as_str()).collect();
                    steps.push(format!("{} {event} to {}", fmt_g(f64::from(t), 4), to.join(", ")));
                }
            }
            Err(e) => {
                stop = format!("{} {e}", fmt_g(f64::from(t), 4));
                break;
            }
        }
    }
    let (picked, fails) = match dry_select(vm, enemy.mind) {
        Ok((p, f)) => (
            p.map(|i| names[i].clone()).unwrap_or_else(|| "none".to_string()),
            f.into_iter().map(|(i, why)| format!("{}: {why}", names[i])).collect::<Vec<_>>(),
        ),
        Err(e) => (format!("error: {e}"), vec![]),
    };
    let target = ow_ai::awareness::vm::state(vm, enemy.mind).and_then(|s| s.current) == Some(player.pawn);
    let stubs: std::collections::BTreeSet<String> =
        vm.log[log_start..].iter().filter(|l| l.starts_with("UNIMPLEMENTED")).cloned().collect();
    let current = brain(vm, enemy.mind).and_then(|b| b.tree.current).map(|i| names[i].clone()).unwrap_or_default();
    format!(
        "{{\"case\":{},\"target_is_player\":{target},\"running\":{},\"steps\":[{}],\"stopped_on\":{},\"would_pick\":{},\"can_run_failures\":[{}],\"movement_requests\":[{}],\"stubs_reached\":[{}]}}",
        quote(case.name),
        quote(&current),
        steps.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","),
        quote(&stop),
        quote(&picked),
        fails.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","),
        moves.borrow().requests.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","),
        stubs.iter().map(|s| quote(s)).collect::<Vec<_>>().join(","),
    )
}

#[test]
#[ignore = "needs the installed game (OPENWILLOW_BL2)"]
fn the_stock_bullymong_chooses_its_actions() {
    let Some(cooked) = cooked() else { return };
    let mut rows = Vec::new();
    for case in &CASES {
        let row = one(cooked.clone(), case);
        println!("{row}");
        rows.push(row);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w7-ai-actions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("installed_run.json"), format!("[\n{}\n]\n", rows.join(",\n"))).unwrap();
}
