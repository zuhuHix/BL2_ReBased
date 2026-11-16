//! Owner: lane W6-MISSION. The bridge rules on invented packages (AGENTS.md rule 9). AI-assisted (Claude Code).
//!
//! A small script world built with `ow_package::testkit` (in the shape of `tests/mission_script_test.py`'s toys,
//! over invented classes): the classes and properties the bridge builds its graph from, every native it binds as a
//! declaration, and short script bodies for the paths under test (the validation reply, `AcceptMission`,
//! `ServerCompleteMission`, `UpdateMissionStatus`, `UpdateMissionObjective`, `IsMissionMoviePlaying`, an empty
//! `ExpLevelUp`). The natives' own rules are checked by calling the bridge's bodies with argument lists built here.
//! Expected values are written by hand from the rules. The reward arithmetic and the curve are `ow-balance`'s (their
//! tests); the `--mission-run` scenarios that need them are pinned against the C++ by `tests/mission_script_test.py`.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{DetSet, ObjectPath, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, PackageBuilder};
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_vm::api::{Arg, FunctionDecl, NativeCall, Vm};

use super::*;
use crate::tracker::Status;

static NEXT: AtomicU32 = AtomicU32::new(0);

const KINDS: [&str; 9] = ["Byte", "Object", "Bool", "Float", "Array", "Struct", "Name", "Int", "Str"];
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC: u32 = 0x2000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const ROLES: [&str; 4] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority"];

fn object_import(p: &mut PackageBuilder) -> i32 {
    let core = p.add_import("Core", "Package", 0, "Core");
    p.add_import("Core", "Class", core, "Object")
}

fn core() -> Vec<u8> {
    let mut t = ScriptToy::new(&[]);
    let object = t.class("Object", 0);
    t.class("Package", object);
    t.build()
}

fn param(kind: &str, name: &str, flags: u64, type_ref: i32) -> Param {
    let mut p = Param::new(kind, name);
    p.flags = flags;
    p.type_ref = type_ref;
    p
}

/// A native declaration of `owner`.
fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<(&str, i32)>, flags: u32) -> i32 {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(flags, name), |_| None).function
}

/// A script function whose body `body` writes (`return` and the end are added).
fn script(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    params: &[Param],
    result: Option<(&str, i32)>,
    body: impl FnOnce(&mut Asm, &ow_package::testkit::script::FunctionIds),
) -> i32 {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(SCRIPT, name), |ids| {
        let mut a = Asm::new();
        a.stmt();
        body(&mut a, ids);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    })
    .function
}

fn enum_export(t: &mut ScriptToy, outer: i32, name: &str, values: &[&str]) -> i32 {
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend_from_slice(&t.none);
    payload.extend(0i32.to_le_bytes());
    payload.extend((values.len() as i32).to_le_bytes());
    for v in values {
        payload.extend(t.p.fname(v, 0));
    }
    let mut e = Export::new(t.import("Enum"), name, payload);
    e.outer = outer;
    t.p.add_export_with(e)
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let object = object_import(&mut t.p);
    let actor = t.class("Actor", object);
    t.prop("Byte", actor, "Role", 0, 0);
    t.prop("Object", actor, "WorldInfo", 0, object);
    t.prop("Object", actor, "Owner", 0, object);
    toy_array_of(&mut t, actor, "Touching", "Object", object);
    native(&mut t, actor, "IsPlayerOwned", &[], Some(("Bool", 0)), NATIVE);
    enum_export(&mut t, actor, "ENetRole", &ROLES);
    let controller = t.class("PlayerController", actor);
    native(&mut t, controller, "IsPrimaryPlayer", &[], Some(("Bool", 0)), NATIVE);
    let pawn = t.class("Pawn", actor);
    t.prop("Object", pawn, "Controller", 0, object);
    let world = t.class("WorldInfo", actor);
    t.prop("Object", world, "GRI", 0, object);
    t.prop("Bool", world, "bIsMenuLevel", 0, 0);
    t.prop("Float", world, "TimeSeconds", 0, 0);
    native(&mut t, world, "IsMenuLevel", &[param("Str", "MapName", CPF_OPT, 0)], Some(("Bool", 0)), NATIVE | STATIC);
    let base = t.class("BehaviorBase", object);
    native(&mut t, base, "GetWorldInfo", &[], Some(("Object", object)), NATIVE);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let object = object_import(&mut t.p);
    let globals = t.class("GearboxGlobals", object);
    native(&mut t, globals, "GetGearboxGlobals", &[], Some(("Object", object)), NATIVE | STATIC);
    native(&mut t, globals, "GetBehaviorKernel", &[], Some(("Object", object)), NATIVE);
    t.class("BehaviorProviderDefinition", object);
    t.build()
}

fn willowgame() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let object = object_import(&mut t.p);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let player_controller = t.p.add_import("Core", "Class", engine, "PlayerController");
    let engine_pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let world_class = t.p.add_import("Core", "Class", engine, "WorldInfo");
    let world_info = t.p.add_import("Core", "ObjectProperty", actor, "WorldInfo");
    let gri = t.p.add_import("Core", "ObjectProperty", world_class, "GRI");
    let gearbox = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_globals = t.p.add_import("Core", "Class", gearbox, "GearboxGlobals");

    // The mission data.
    let mission = t.class("MissionDefinition", object);
    toy_array_of(&mut t, mission, "ObjectiveSetDefs", "Object", object);
    t.prop("Object", mission, "InitialObjectiveSet", 0, object);
    t.prop("Bool", mission, "bActivateInitialObjectiveSet", 0, 0);
    let reward = toy_struct(&mut t, 0, "MissionRewardData", None);
    t.prop("Byte", reward, "CurrencyRewardType", 0, 0);
    toy_array_of(&mut t, reward, "RewardItems", "Object", object);
    toy_array_of(&mut t, reward, "RewardItemPools", "Object", object);
    t.prop("Struct", mission, "Reward", 0, reward);
    t.prop("Struct", mission, "AlternativeReward", 0, reward);
    native(&mut t, mission, "GetExperienceReward", &[param("Object", "InWPC", 0, object), param("Bool", "bGetAltReward", 0, 0)], Some(("Int", 0)), NATIVE);
    native(&mut t, mission, "GetGameStage", &[], Some(("Int", 0)), NATIVE);
    native(&mut t, mission, "GetCurrencyRewardType", &[param("Bool", "bGetAltReward", 0, 0)], Some(("Byte", 0)), NATIVE);
    native(&mut t, mission, "GetCurrencyReward", &[param("Object", "InWPC", 0, object), param("Bool", "bGetAltReward", 0, 0)], Some(("Int", 0)), NATIVE);
    native(&mut t, mission, "ShouldGrantAlternateReward", &[param("Array", "ObjectivesProgress", CPF_OUT, 0)], Some(("Bool", 0)), NATIVE);
    native(&mut t, mission, "GetItemRewardsForPlayer", &[param("Object", "WillowPC", 0, object), param("Object", "MissionReward", CPF_OUT, object)], None, NATIVE);
    let set = t.class("MissionObjectiveSetDefinition", object);
    toy_array_of(&mut t, set, "ObjectiveDefinitions", "Object", object);
    t.prop("Bool", set, "bCanCompleteMission", 0, 0);
    let objective = t.class("MissionObjectiveDefinition", object);
    t.prop("Int", objective, "ObjectiveCount", 0, 0);
    t.prop("Bool", objective, "bObjectiveIsOptional", 0, 0);

    // The tracker and the replication info.
    let tracker = t.class("MissionTracker", object);
    t.prop("Bool", tracker, "bDataValidated", 0, 0);
    let pair = [param("Object", "InMission", 0, object), param("Object", "WillowPC", CPF_OPT, object)];
    let activate = native(&mut t, tracker, "ActivateMission", &pair, None, NATIVE);
    let complete = native(&mut t, tracker, "CompleteMission", &pair, None, NATIVE);
    let play_turn_in = native(&mut t, tracker, "PlayTurnIn", &[param("Object", "InMission", 0, object)], None, NATIVE);
    for (name, result) in [("GetMissionStatus", "Byte"), ("CanStartMission", "Bool"), ("CanEndMission", "Bool"), ("GetCompletedBranch", "Byte")] {
        native(&mut t, tracker, name, &[param("Object", "InMission", 0, object)], Some((result, 0)), NATIVE);
    }
    native(&mut t, tracker, "IsDataValid", &[], Some(("Bool", 0)), NATIVE);
    let validate = native(&mut t, tracker, "ValidateData", &[], None, NATIVE);
    for name in ["IsMissionObjectiveActive", "IsMissionObjectiveComplete", "IsObjectiveSetActive"] {
        native(&mut t, tracker, name, &[param("Object", "Objective", 0, object)], Some(("Bool", 0)), NATIVE);
    }
    native(&mut t, tracker, "UpdateObjective", &[param("Object", "Objective", 0, object), param("Int", "ObjectiveBit", CPF_OPT, 0)], None, NATIVE);
    native(&mut t, tracker, "RegisterMissionObserver", &[param("Object", "Observer", 0, object), param("Object", "Mission", 0, object)], None, NATIVE);
    script(&mut t, tracker, "TriggerMissionStatusChangedDelegates", &[], None, |_, _| {});
    script(&mut t, tracker, "TriggerMissionObjectivesChangedDelegates", &[param("Object", "Mission", 0, object)], None, |_, _| {});
    let replication = t.class("WillowGameReplicationInfo", object);
    let tracker_prop = t.prop("Object", replication, "MissionTracker", 0, object);
    t.prop("Int", replication, "CurrentPlaythrough", 0, 0);

    // The globals.
    let globals = t.class("WillowGlobals", gearbox_globals);
    native(&mut t, globals, "GetWillowGlobals", &[], Some(("Object", object)), NATIVE | STATIC);
    native(&mut t, globals, "GetGlobalsDefinition", &[], Some(("Object", object)), NATIVE);
    let definition = t.class("GlobalsDefinition", object);
    t.prop("Float", definition, "PlayerInteractionDistance", 0, 0);

    // The player.
    let pri = t.class("WillowPlayerReplicationInfo", object);
    for name in ["ExpLevel", "ExpPointsNextLevelAt", "GeneralSkillPoints"] {
        t.prop("Int", pri, name, 0, 0);
    }
    let imission = t.class("IMission", object);
    let status = toy_struct(&mut t, imission, "MissionStatusPlayerData", None);
    t.prop("Object", status, "MissionDef", 0, object);
    t.prop("Byte", status, "Status", 0, 0);
    t.prop("Bool", status, "bNeedsRewards", 0, 0);
    let controller = t.class("WillowPlayerController", player_controller);
    let playthrough = toy_struct(&mut t, controller, "MissionPlaythroughData", None);
    toy_array_of(&mut t, playthrough, "MissionList", "Struct", status);
    toy_array_of(&mut t, controller, "MissionPlaythroughs", "Struct", playthrough);
    t.prop("Object", controller, "PlayerReplicationInfo", 0, object);
    for name in ["Pawn", "PlayerClass", "CharacterClass", "MyWillowPawn", "ResourcePoolManager"] {
        t.prop("Object", controller, name, 0, object);
    }
    native(&mut t, controller, "NativeGetMissionIndex", &[param("Object", "InMission", 0, object)], Some(("Int", 0)), NATIVE);
    native(&mut t, controller, "GetCurrentPlaythrough", &[], Some(("Int", 0)), NATIVE);
    native(&mut t, controller, "GetHUDMovie", &[], Some(("Object", object)), NATIVE);
    native(&mut t, controller, "UpdateLcdMissionStatus", &[], None, NATIVE);
    native(&mut t, controller, "PlayUIAkEvent", &[param("Object", "Event", 0, object)], None, NATIVE);
    let exp_earn = native(
        &mut t,
        controller,
        "ExpEarn",
        &[param("Int", "Exp", 0, 0), param("Byte", "Source", 0, 0), param("Byte", "ExpType", CPF_OPT, 0)],
        None,
        NATIVE,
    );
    native(&mut t, controller, "GetMaxExpLevel", &[], Some(("Int", 0)), NATIVE);
    native(&mut t, controller, "GetExpPointsRequiredForLevel", &[param("Int", "Level", 0, 0)], Some(("Int", 0)), NATIVE);
    native(&mut t, controller, "RecalculateAttributeInitializedState", &[], None, NATIVE);
    // Self.WorldInfo.GRI.MissionTracker.<call>
    let on_tracker = move |a: &mut Asm, call: &dyn Fn(&mut Asm)| {
        a.context(
            |a| {
                a.context(|a| {
                    a.context(|a| {
                        a.instance(world_info);
                    }, |a| {
                        a.instance(gri);
                    });
                }, |a| {
                    a.instance(tracker_prop);
                });
            },
            |a| call(a),
        );
    };
    script(&mut t, controller, "ClientValidateMissionData", &[], None, |a, _| {
        on_tracker(a, &|a| {
            a.final_function(validate, |_| {});
        });
    });
    // ExpEarn(0, NewMissionStatus): the hook's marker.
    script(&mut t, controller, "UpdateMissionStatus", &[param("Object", "Mission", 0, object), param("Byte", "NewMissionStatus", 0, 0)], None, |a, ids| {
        let status = ids.get("NewMissionStatus");
        a.final_function(exp_earn, |a| {
            a.int_const(0);
            a.local(status);
        });
    });
    script(&mut t, controller, "UpdateMissionObjective", &[param("Object", "Objective", 0, object), param("Int", "Bit", 0, 0)], None, |a, _| {
        a.final_function(exp_earn, |a| {
            a.int_const(0);
            a.byte_const(210);
        });
    });
    script(&mut t, controller, "IsMissionMoviePlaying", &[], Some(("Bool", 0)), |a, _| {
        a.final_function(exp_earn, |a| {
            a.int_const(0);
            a.byte_const(100);
        });
    });
    let mission_pair = [param("Object", "Mission", 0, object), param("Object", "MissionDirector", 0, object)];
    script(&mut t, controller, "AcceptMission", &mission_pair, None, |a, ids| {
        let m = ids.get("Mission");
        on_tracker(a, &|a| {
            a.final_function(activate, |a| {
                a.local(m);
                a.self_();
            });
        });
    });
    script(&mut t, controller, "ServerCompleteMission", &mission_pair, None, |a, ids| {
        let m = ids.get("Mission");
        on_tracker(a, &|a| {
            a.final_function(complete, |a| {
                a.local(m);
                a.self_();
            });
        });
        a.stmt();
        on_tracker(a, &|a| {
            a.final_function(play_turn_in, |a| {
                a.local(m);
            });
        });
    });
    // An ExpLevelUp that does not raise the level: the loop's guard.
    script(&mut t, controller, "ExpLevelUp", &[param("Bool", "bCheated", 0, 0)], None, |_, _| {});
    let willow_pawn = t.class("WillowPawn", engine_pawn);
    t.class("WillowPlayerPawn", engine_pawn);
    t.class("WillowAIPawn", willow_pawn);
    t.build()
}

/// `ToyMission`: one set `SetA` {`Only` (count 1)} that completes the mission, no provider; `Other`, a second
/// mission of the package; `GD_Globals.General.Globals` with an invented interaction distance.
fn mission_package(optional: bool) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let none = p.none();
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let class = |p: &mut PackageBuilder, name: &str| p.add_import("Core", "Class", willow, name);
    let c = class(&mut p, "MissionDefinition");
    let mission = p.add_export(c, "ToyMission", Vec::new());
    let c = class(&mut p, "MissionObjectiveDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.int_tag("ObjectiveCount", 1));
    if optional {
        payload.extend(p.bool_tag("bObjectiveIsOptional", true));
    }
    payload.extend_from_slice(&none);
    let only = p.add_export(c, "Only", payload);
    p.set_outer(only, mission);
    let c = class(&mut p, "MissionObjectiveSetDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.array_tag("ObjectiveDefinitions", 1, &only.to_le_bytes()));
    payload.extend(p.bool_tag("bCanCompleteMission", true));
    payload.extend_from_slice(&none);
    let set = p.add_export(c, "SetA", payload);
    p.set_outer(set, mission);
    let mut tags = 0i32.to_le_bytes().to_vec();
    tags.extend(p.array_tag("ObjectiveSetDefs", 1, &set.to_le_bytes()));
    tags.extend(p.object_tag("InitialObjectiveSet", set));
    tags.extend(p.bool_tag("bActivateInitialObjectiveSet", true));
    let mut reward = p.byte_tag("CurrencyRewardType", 0);
    reward.extend_from_slice(&none);
    tags.extend(p.struct_tag("Reward", "MissionRewardData", &reward));
    tags.extend_from_slice(&none);
    p.set_payload(mission, tags);
    let c = class(&mut p, "MissionDefinition");
    let mut other = 0i32.to_le_bytes().to_vec();
    other.extend_from_slice(&none);
    p.add_export(c, "Other", other);
    let package_class = p.add_import("Core", "Class", 0, "Package");
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&none);
    let gd = p.add_export(package_class, "GD_Globals", empty.clone());
    let general = p.add_export(package_class, "General", empty);
    p.set_outer(general, gd);
    let c = class(&mut p, "GlobalsDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.float_tag("PlayerInteractionDistance", 420.0));
    payload.extend_from_slice(&none);
    let globals = p.add_export(c, "Globals", payload);
    p.set_outer(globals, general);
    p.build()
}

struct Env {
    world: World,
    mission: MissionSystem,
    script: MissionScript,
    dir: PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Env {
    fn new(optional: bool) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-mission-bridge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let files: Vec<(String, Vec<u8>)> = vec![
            ("Core.upk".into(), core()),
            ("Engine.upk".into(), engine()),
            ("GearboxFramework.upk".into(), gearbox()),
            ("WillowGame.upk".into(), willowgame()),
            ("TestMission.upk".into(), mission_package(optional)),
        ];
        let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let defs = DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .expect("synthetic store");
        let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::standard().unwrap()), ParityMode::Off);
        let mut world = world(vm).expect("world");
        ow_vm::natives_core::register(&mut world.vm).expect("core natives");
        let name = PackageName::parse("TestMission").unwrap();
        let mission = MissionSystem::new(&mut world.vm, &name, "ToyMission").expect("mission");
        let script = MissionScript::new(&mut world, &mission).expect("bridge");
        Env {
            world,
            mission,
            script,
            dir,
        }
    }

    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }

    /// The mission export `path` of the mission package, loaded.
    fn object(&mut self, path: &str) -> ObjRef {
        let name = PackageName::parse("TestMission").unwrap();
        tracker::find_object(&mut self.world.vm, &name, path).unwrap().expect("export")
    }

    /// Runs the bridge's body `body` as the native at `path` on `this` with `args` (each supplied).
    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>, body: Body) -> Result<Value, MissionError> {
        self.call_with(path, this, args.into_iter().map(|v| Arg::new(v, false, true)).collect(), body)
    }

    fn call_with(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Arg>, body: Body) -> Result<Value, MissionError> {
        let vm = &mut self.world.vm;
        let f = match vm.find_function(path) {
            Ok(f) => f,
            Err(_) => vm.declare_function(FunctionDecl::native(ObjectPath::parse(path).unwrap(), path)).unwrap(),
        };
        let mut c = NativeCall::new(vm, f, this, args);
        body(&self.script, &mut c)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The graph and the script paths
// ---------------------------------------------------------------------------------------------------------------

/// The graph the script reads: the controller's world, its replication info and tracker, authority from the
/// ENetRole enum by name, one playthrough; the validation reply ran (`IsDataValid` answers its flag).
#[test]
fn the_graph_is_built_and_the_data_validated() {
    let mut env = Env::new(false);
    let o = env.script.inner.o;
    let space = &env.world.vm.space;
    assert_eq!(get(space, o.controller, "WorldInfo").unwrap(), Value::Object(Some(o.world_info)));
    assert_eq!(get(space, o.world_info, "GRI").unwrap(), Value::Object(Some(o.replication)));
    assert_eq!(get(space, o.replication, "MissionTracker").unwrap(), Value::Object(Some(o.tracker)));
    assert_eq!(get(space, o.controller, "Role").unwrap(), Value::Byte(3), "ROLE_Authority's position");
    assert_eq!(get(space, o.world_info, "Role").unwrap(), Value::Byte(3));
    assert_eq!(get(space, o.controller, "MissionPlaythroughs").unwrap().elements().map(<[Value]>::len), Some(1));
    assert_eq!(get(space, o.tracker, "bDataValidated").unwrap(), Value::Bool(true));
    let valid = env.call("WillowGame.MissionTracker.IsDataValid", Some(o.tracker), vec![], MissionScript::n_data_valid);
    assert_eq!(valid.unwrap(), Value::Bool(true));
    // The world's globals and world info are the bridge's objects.
    let wi = env.world.actors().borrow().world_info();
    assert_eq!(wi, Some(o.world_info));
    assert_eq!(env.world.query().borrow().globals(), Some(o.globals));
    assert!(env.script.errors().is_empty(), "{:?}", env.script.errors());
}

/// `AcceptMission` reaches `ActivateMission`; the status routine's hook runs `UpdateMissionStatus` with the status
/// number (its `ExpEarn` marker), and the Active branch locks the region's stage first; a refused second accept
/// changes nothing; the tick's kickoff hook runs `IsMissionMoviePlaying` once.
#[test]
fn accept_runs_the_script_and_the_status_hook() {
    let mut env = Env::new(false);
    env.script.set_region_game_stage(4);
    let (m, s) = (env.mission.clone(), env.script.clone());
    assert!(s.accept(env.vm(), &DetSet::new()).unwrap());
    assert_eq!(m.status(), Status::Active);
    s.set_region_game_stage(9);
    assert_eq!(s.current_game_stage(), 4, "locked at Active");
    assert!(!s.accept(env.vm(), &DetSet::new()).unwrap());
    m.tick(env.vm(), 0.5).unwrap();
    m.tick(env.vm(), 0.5).unwrap();
    let sources: Vec<i32> = s.exp_earned().iter().map(|e| e.source).collect();
    assert_eq!(sources, [1, 100], "the Active hook, then the kickoff hook once");
    // ExpEarn records the call before it needs the curve, which this package set does not hold.
    assert!(s.errors().iter().all(|e| e.contains("experience curve")), "{:?}", s.errors());
}

/// The objective hook tells the controller (`UpdateMissionObjective`) once per applied update; `ServerCompleteMission`
/// completes the mission and runs `PlayTurnIn`; the ReadyToTurnIn and Complete hooks carry 3 and 4.
#[test]
fn turn_in_runs_complete_and_play_turn_in() {
    let mut env = Env::new(false);
    let (m, s) = (env.mission.clone(), env.script.clone());
    s.accept(env.vm(), &DetSet::new()).unwrap();
    m.update_objective(env.vm(), "Only", 0).unwrap();
    assert_eq!(s.objective_updates(), 1);
    assert_eq!(m.status(), Status::ReadyToTurnIn);
    m.drain();
    assert!(s.turn_in(env.vm()).unwrap());
    assert_eq!(m.status(), Status::Complete);
    let kinds: Vec<EffectKind> = m.drain().iter().map(|e| e.kind).collect();
    assert_eq!(kinds, [EffectKind::StatusChanged, EffectKind::Reward]);
    let sources: Vec<i32> = s.exp_earned().iter().map(|e| e.source).collect();
    assert_eq!(sources, [1, 210, 3, 4]);
}

/// The controller's record follows a restored mission (`sync_restored`), and `NativeGetMissionIndex` finds it; a
/// mission with no record is -1.
#[test]
fn the_controller_record_follows_a_restored_mission() {
    let mut env = Env::new(false);
    let (m, s) = (env.mission.clone(), env.script.clone());
    assert_eq!(s.controller_status(env.vm()).unwrap(), -1);
    assert!(m.load_state("status=1\nactive=ToyMission.SetA\n"));
    s.sync_restored(env.vm()).unwrap();
    assert_eq!(s.controller_status(env.vm()).unwrap(), 1);
    assert!(!s.controller_needs_rewards(env.vm()).unwrap());
    let c = s.player_controller();
    let def = m.definition();
    let index = env.call("WillowGame.WillowPlayerController.NativeGetMissionIndex", Some(c), vec![Value::Object(Some(def))], MissionScript::n_mission_index);
    assert_eq!(index.unwrap(), Value::Int(0));
    let other = env.object("Other");
    let index = env.call("WillowGame.WillowPlayerController.NativeGetMissionIndex", Some(c), vec![Value::Object(Some(other))], MissionScript::n_mission_index);
    assert_eq!(index.unwrap(), Value::Int(-1));
}

/// The level-up loop: an `ExpLevelUp` that does not raise the level stops it with a note, and nothing is gained; the
/// controller's `Pawn` and `PlayerClass` are given back as they were.
#[test]
fn a_level_up_that_raises_nothing_stops_with_a_note() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let pri = s.inner.o.pri;
    set(&mut env.vm().space, pri, "ExpLevel", Value::Int(3)).unwrap();
    set(&mut env.vm().space, pri, "ExpPointsNextLevelAt", Value::Int(10)).unwrap();
    s.inner.state.borrow_mut().pool = 20.0;
    s.update_experience_pool(env.vm()).unwrap();
    assert_eq!(s.notes(), ["ExpLevelUp did not raise ExpLevel"]);
    assert!(s.take_gains().is_empty());
    assert_eq!(get(&env.vm().space, s.inner.o.controller, "Pawn").unwrap(), Value::Object(None));
    // Below the requirement the loop does not run at all.
    s.inner.state.borrow_mut().pool = 5.0;
    s.inner.state.borrow_mut().notes.clear();
    s.update_experience_pool(env.vm()).unwrap();
    assert!(s.notes().is_empty());
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

/// The tracker's queries answer for this mission (by object or by its path) and answer "no record" for another.
#[test]
fn the_tracker_queries_answer_for_the_mission() {
    let mut env = Env::new(false);
    let (m, s) = (env.mission.clone(), env.script.clone());
    let t = Some(s.inner.o.tracker);
    let mine = Value::Object(Some(m.definition()));
    let other = Value::Object(Some(env.object("Other")));
    let status = |env: &mut Env, v: &Value| env.call("WillowGame.MissionTracker.GetMissionStatus", t, vec![v.clone()], MissionScript::n_status).unwrap();
    assert_eq!(status(&mut env, &mine), Value::Byte(0));
    let start = env.call("WillowGame.MissionTracker.CanStartMission", t, vec![mine.clone()], MissionScript::n_can_start);
    assert_eq!(start.unwrap(), Value::Bool(true));
    let start = env.call("WillowGame.MissionTracker.CanStartMission", t, vec![other.clone()], MissionScript::n_can_start);
    assert_eq!(start.unwrap(), Value::Bool(false), "a mission without a record is not offered");
    s.accept(env.vm(), &DetSet::new()).unwrap();
    assert_eq!(status(&mut env, &mine), Value::Byte(1));
    assert_eq!(status(&mut env, &other), Value::Byte(0));
    let only = Value::Object(Some(env.object("ToyMission.Only")));
    let active = env.call("WillowGame.MissionTracker.IsMissionObjectiveActive", t, vec![only.clone()], MissionScript::n_objective_active);
    assert_eq!(active.unwrap(), Value::Bool(true));
    let set_a = Value::Object(Some(env.object("ToyMission.SetA")));
    let set_active = env.call("WillowGame.MissionTracker.IsObjectiveSetActive", t, vec![set_a], MissionScript::n_set_active);
    assert_eq!(set_active.unwrap(), Value::Bool(true));
    // UpdateObjective with the bit omitted.
    let args = vec![Arg::new(only.clone(), false, true), Arg::new(Value::None, false, false)];
    env.call_with("WillowGame.MissionTracker.UpdateObjective", t, args, MissionScript::n_update_objective).unwrap();
    assert_eq!(m.status(), Status::ReadyToTurnIn);
    let done = env.call("WillowGame.MissionTracker.IsMissionObjectiveComplete", t, vec![only], MissionScript::n_objective_complete);
    assert_eq!(done.unwrap(), Value::Bool(true));
    let end = env.call("WillowGame.MissionTracker.CanEndMission", t, vec![mine.clone()], MissionScript::n_can_end);
    assert_eq!(end.unwrap(), Value::Bool(true));
    let branch = env.call("WillowGame.MissionTracker.GetCompletedBranch", t, vec![mine], MissionScript::n_branch);
    assert_eq!(branch.unwrap(), Value::Byte(1));
}

/// A native of a bound class called on another object: the unbound behaviour (a log line and the zero result).
#[test]
fn a_call_outside_the_binding_is_the_unbound_behaviour() {
    let mut env = Env::new(false);
    let c = Some(env.script.player_controller());
    let def = Value::Object(Some(env.mission.definition()));
    let before = env.vm().log.len();
    let got = env.call("WillowGame.MissionTracker.GetMissionStatus", c, vec![def], MissionScript::n_status).unwrap();
    assert_eq!(got, Value::Byte(0));
    let line = &env.vm().log[before];
    assert_eq!(line, "UNIMPLEMENTED MissionTracker.GetMissionStatus(object) (outside the mission bridge binding)");
}

/// `IsPlayerOwned`: the owner chain's root, then its controller; the player's pawn is, an unowned object is not.
#[test]
fn player_ownership_follows_the_owner_chain() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let pawn = s.player_pawn(env.vm()).unwrap();
    let owned = env.call("Engine.Actor.IsPlayerOwned", Some(pawn), vec![], MissionScript::n_player_owned);
    assert_eq!(owned.unwrap(), Value::Bool(true));
    let marcus = s.pawn_for(env.vm(), Toucher::Marcus).unwrap();
    let owned = env.call("Engine.Actor.IsPlayerOwned", Some(marcus), vec![], MissionScript::n_player_owned);
    assert_eq!(owned.unwrap(), Value::Bool(false), "no player controller");
    // Owned by the player's pawn: the root's controller.
    set(&mut env.vm().space, marcus, "Owner", Value::Object(Some(pawn))).unwrap();
    let owned = env.call("Engine.Actor.IsPlayerOwned", Some(marcus), vec![], MissionScript::n_player_owned);
    assert_eq!(owned.unwrap(), Value::Bool(true));
    let c = s.player_controller();
    let owned = env.call("Engine.Actor.IsPlayerOwned", Some(c), vec![], MissionScript::n_player_owned);
    assert_eq!(owned.unwrap(), Value::Bool(true), "the controller itself");
}

/// The presentation natives and the globals: `GetHUDMovie` None, `IsPrimaryPlayer` true, `IsMenuLevel` the world's
/// bit, the one globals object, the configured globals definition and its interaction distance.
#[test]
fn the_controller_helpers_and_the_globals() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let c = Some(s.player_controller());
    assert_eq!(env.call("WillowGame.WillowPlayerController.GetHUDMovie", c, vec![], MissionScript::n_hud_movie).unwrap(), Value::Object(None));
    assert_eq!(env.call("Engine.PlayerController.IsPrimaryPlayer", c, vec![], MissionScript::n_primary).unwrap(), Value::Bool(true));
    assert_eq!(env.call("Engine.WorldInfo.IsMenuLevel", None, vec![], MissionScript::n_menu_level).unwrap(), Value::Bool(false));
    set(&mut env.vm().space, s.inner.o.world_info, "bIsMenuLevel", Value::Bool(true)).unwrap();
    assert_eq!(env.call("Engine.WorldInfo.IsMenuLevel", None, vec![], MissionScript::n_menu_level).unwrap(), Value::Bool(true));
    let g = env.call("WillowGame.WillowGlobals.GetWillowGlobals", None, vec![], MissionScript::n_globals).unwrap();
    assert_eq!(g, Value::Object(Some(s.inner.o.globals)));
    let definition = env.call("WillowGame.WillowGlobals.GetGlobalsDefinition", None, vec![], MissionScript::n_globals_definition).unwrap();
    assert_eq!(definition, Value::Object(Some(env.object("GD_Globals.General.Globals"))));
    assert_eq!(s.player_interaction_distance(env.vm()).unwrap(), 420.0);
    assert_eq!(env.call("WillowGame.WillowPlayerController.GetCurrentPlaythrough", c, vec![], MissionScript::n_playthrough).unwrap(), Value::Int(0));
    assert_eq!(env.call("WillowGame.WillowPlayerController.GetMaxExpLevel", c, vec![], MissionScript::n_max_level).unwrap(), Value::Int(MAX_LEVEL));
}

/// `ExpEarn` records each call with the omitted type as -1.
#[test]
fn exp_earn_records_the_call() {
    let mut env = Env::new(false);
    let c = Some(env.script.player_controller());
    let args = vec![Arg::new(Value::Int(5), false, true), Arg::new(Value::Byte(7), false, true), Arg::new(Value::None, false, false)];
    let _ = env.call_with("WillowGame.WillowPlayerController.ExpEarn", c, args, MissionScript::n_exp_earn);
    let e = env.script.exp_earned();
    assert_eq!((e[0].amount, e[0].source, e[0].exp_type), (5, 7, -1));
}

/// The mission's reward natives: the game stage (0 without a region stage), the currency type of the reward, no
/// currency for type 0 with a zero multiplier (the data's multiplier is absent here: a typed error), and
/// `GetExperienceReward` of a mission with an optional objective is `NotYet`.
#[test]
fn the_reward_natives_answer_from_the_data() {
    let mut env = Env::new(false);
    let def = Some(env.mission.definition());
    assert_eq!(env.call("WillowGame.MissionDefinition.GetGameStage", def, vec![], MissionScript::n_game_stage).unwrap(), Value::Int(0));
    let t = env.call("WillowGame.MissionDefinition.GetCurrencyRewardType", def, vec![Value::Bool(false)], MissionScript::n_currency_type);
    assert_eq!(t.unwrap(), Value::Byte(0));
    let c = env.call("WillowGame.MissionDefinition.GetCurrencyReward", def, vec![Value::Object(None), Value::Bool(false)], MissionScript::n_currency);
    assert_eq!(c.unwrap_err().to_string(), "mission script bridge: the reward has no CreditRewardMultiplier");
    let items = env.call("WillowGame.MissionDefinition.GetItemRewardsForPlayer", def, vec![Value::Object(None), Value::Object(None)], MissionScript::n_item_rewards);
    assert_eq!(items.unwrap(), Value::None);
    let mut env = Env::new(true);
    let def = Some(env.mission.definition());
    let xp = env.call("WillowGame.MissionDefinition.GetExperienceReward", def, vec![Value::Object(None), Value::Bool(false)], MissionScript::n_xp_reward);
    assert!(xp.unwrap_err().not_yet().is_some());
}

/// `GetBehaviorContext`: the selector picks Self (0), the instigator (1), the other participant (2) or the struct's
/// own object (4); 3 and anything else give None; a named instance-data context gives None and a stub line.
#[test]
fn the_behavior_context_resolver_follows_the_selector() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let def = env.mission.definition();
    let vm = env.vm();
    // A struct value of BehaviorContextData's shape is not in this package set: use the mission's reward struct type
    // with the selector fields absent, so only the arguments decide (selector 0: Self).
    let (a, b, c) = (s.player_controller(), s.inner.o.tracker, s.inner.o.globals);
    let reward = prop(&vm.space, def, "Reward").unwrap().unwrap();
    let args = vec![reward, Value::Object(Some(a)), Value::Object(Some(b)), Value::Object(Some(c))];
    let got = env.call("Engine.BehaviorBase.GetBehaviorContext", None, args, MissionScript::n_behavior_context);
    assert_eq!(got.unwrap(), Value::Object(Some(a)), "no selector field: 0, SelfObject");
}

/// `ResolveBehaviorProviderDefinitionReference` without a director: an empty path and no reference give None; a
/// reference is returned as it is.
#[test]
fn the_provider_resolver_prefers_the_path_then_the_reference() {
    let mut env = Env::new(false);
    let o = env.object("Other");
    let got = env.call(
        "GearboxFramework.BehaviorHelpers.ResolveBehaviorProviderDefinitionReference",
        None,
        vec![Value::Object(None), Value::Object(Some(o)), Value::None],
        MissionScript::n_resolve_provider,
    );
    assert_eq!(got.unwrap(), Value::Object(Some(o)));
    let got = env.call(
        "GearboxFramework.BehaviorHelpers.ResolveBehaviorProviderDefinitionReference",
        None,
        vec![Value::Object(None), Value::Object(None), Value::None],
        MissionScript::n_resolve_provider,
    );
    assert_eq!(got.unwrap(), Value::Object(None));
}

/// The output link native appends to the innermost running behavior's list only.
#[test]
fn output_links_go_to_the_running_behavior() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let _ = env.call("GearboxFramework.BehaviorKernel.ActivateBehaviorOutputLink", None, vec![Value::None, Value::Int(1)], MissionScript::n_output_link);
    s.inner.state.borrow_mut().selected.push(Vec::new());
    for id in [0, 2, 2] {
        env.call("GearboxFramework.BehaviorKernel.ActivateBehaviorOutputLink", None, vec![Value::None, Value::Int(id)], MissionScript::n_output_link).unwrap();
    }
    assert_eq!(s.inner.state.borrow().selected, [vec![0, 2, 2]], "duplicates kept, nothing before the run");
}

/// The level-up's `RunBehaviors` outside the level-up is the C++ log line; inside it is `NotYet` (lane W6-SKILLS);
/// the health pool is `NotYet`; the director's dialog is `NotYet`.
#[test]
fn the_stand_ins_of_other_lanes_are_not_yet() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let before = env.vm().log.len();
    env.call("Engine.BehaviorBase.RunBehaviors", None, vec![], MissionScript::n_run_behaviors).unwrap();
    assert!(env.vm().log[before].ends_with("(outside the level-up stand-in)"));
    s.inner.state.borrow_mut().in_level_up = true;
    let inside = env.call("Engine.BehaviorBase.RunBehaviors", None, vec![], MissionScript::n_run_behaviors);
    assert!(inside.unwrap_err().not_yet().is_some());
    assert!(s.set_player_health(20.0).unwrap_err().not_yet().is_some());
    let component = s.inner.o.globals;
    s.inner.state.borrow_mut().dialog_component = Some(component);
    let talk = env.call("GearboxFramework.GearboxDialogComponent.TriggerEvent", Some(component), vec![], MissionScript::n_trigger_event);
    assert!(matches!(talk.unwrap_err().not_yet(), Some(n) if n.missing == ow_core::Missing::LayerAbsent("ow-mission dialog (lane W5-DIALOG)")));
}

/// Without a director the lists are empty and the use key does nothing; a package without the screen's classes has
/// no screen (a note names the C++ text).
#[test]
fn without_a_director_nothing_is_offered() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let name = PackageName::parse("TestMission").unwrap();
    assert!(!s.place_marcus(env.vm(), &name, MARCUS_PAWN).unwrap());
    assert!(!s.has_marcus());
    assert_eq!(s.mission_lists(env.vm(), &DetSet::new()).unwrap(), MissionLists::default());
    let (opened, json) = s.use_marcus_json(env.vm(), &DetSet::new()).unwrap();
    assert!(!opened);
    assert!(json.starts_with("{\"interface_opened\":false,"));
    let director = s.inner.o.globals;
    s.bind_screen(env.vm(), director);
    assert_eq!(s.notes(), ["mission screen not hosted: class not found: WillowGame.QuestAcceptGFxMovie"]);
}

/// The report's shape (`--mission-run`'s `script` object).
#[test]
fn the_report_has_the_cxx_keys() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    let json = s.report_json(env.vm()).unwrap();
    assert_eq!(
        json,
        "{\"player_status\":-1,\"needs_rewards\":false,\"player_level\":0,\"experience_pool\":0,\
         \"interaction_distance\":420,\"exp_earned\":[],\"stubs\":[],\"notes\":[]}"
    );
}
