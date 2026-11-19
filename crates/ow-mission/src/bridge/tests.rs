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
use ow_package::testkit::{Export, PackageBuilder, ints};
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_vm::api::{Arg, FunctionDecl, NativeCall, NativePack, Vm};

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
    let targetable = t.class("ITargetable", object);
    enum_export(&mut t, targetable, "EChangeStatus", &["CHANGE_Toggle", "CHANGE_Enable", "CHANGE_Disable"]);
    t.class("AkEvent", object);
    let context = toy_struct(&mut t, base, "BehaviorContextData", None);
    t.prop("Name", context, "InstancedDataContextName", 0, 0);
    t.prop("Object", context, "ContextObject", 0, object);
    t.prop("Byte", context, "BehaviorContext", 0, 0);
    t.prop("Struct", base, "ToyContextData", 0, context);
    // The component base the store reads at 8 (the director's dialog component).
    t.class("ActorComponent", object);
    let resolve = [
        param("Struct", "ContextData", 0, context),
        param("Object", "SelfObject", 0, object),
        param("Object", "MyInstigatorObject", 0, object),
        param("Object", "OtherEventParticipantObject", 0, object),
    ];
    native(&mut t, base, "GetBehaviorContext", &resolve, Some(("Object", object)), NATIVE);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let object = object_import(&mut t.p);
    let globals = t.class("GearboxGlobals", object);
    native(&mut t, globals, "GetGearboxGlobals", &[], Some(("Object", object)), NATIVE | STATIC);
    native(&mut t, globals, "GetBehaviorKernel", &[], Some(("Object", object)), NATIVE);
    provider_vocabulary(&mut t, object);
    dialog_vocabulary(&mut t, object);
    let path = toy_struct(&mut t, 0, "NameBasedObjectPath", None);
    t.prop("Name", path, "PathComponentNames", 0, 0);
    let provider = t.class("ToyPathHolder", object);
    t.prop("Struct", provider, "ToyPath", 0, path);
    // The behavior kernel's script-facing natives (ow_behavior::kernel::natives) and the provider resolver.
    let helpers = t.class("BehaviorHelpers", object);
    let resolve = [
        param("Object", "SourceBehavior", 0, object),
        param("Object", "ProviderReference", 0, object),
        param("Struct", "PathName", 0, path),
    ];
    native(&mut t, helpers, "ResolveBehaviorProviderDefinitionReference", &resolve, Some(("Object", object)), NATIVE | STATIC);
    let kernel = t.class("BehaviorKernel", object);
    let handle = toy_struct(&mut t, kernel, "BehaviorConsumerHandle", None);
    t.prop("Int", handle, "PID", 0, 0);
    let link = [param("Int", "KernelInfo", 0, 0), param("Int", "OutputLinkId", 0, 0)];
    native(&mut t, kernel, "ActivateBehaviorOutputLink", &link, None, NATIVE | STATIC);
    let enabled = [
        param("Struct", "ConsumerHandle", 0, handle),
        param("Object", "ProviderDefinition", 0, object),
        param("Name", "SequenceName", 0, 0),
    ];
    native(&mut t, kernel, "IsBehaviorSequenceEnabled", &enabled, Some(("Bool", 0)), NATIVE | STATIC);
    let event = [
        param("Struct", "ConsumerHandle", 0, handle),
        param("Object", "ProviderDefinition", 0, object),
        param("Name", "EventName", 0, 0),
        param("Int", "EventOutputToActivate", CPF_OPT, 0),
    ];
    native(&mut t, kernel, "ActivateBehaviorEventFromScript", &event, None, NATIVE | STATIC);
    let broadcast = [param("Struct", "ConsumerHandle", 0, handle), param("Name", "EventName", 0, 0)];
    native(&mut t, kernel, "BroadcastBehaviorEventFromScript", &broadcast, None, NATIVE | STATIC);
    native(&mut t, kernel, "RegisterBehaviorConsumer", &[param("Object", "Consumer", 0, object)], Some(("Struct", handle)), NATIVE | STATIC);
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
    toy_array_of(&mut t, mission, "ObjectiveDefs", "Object", object);
    t.prop("Object", mission, "InitialObjectiveSet", 0, object);
    t.prop("Bool", mission, "bActivateInitialObjectiveSet", 0, 0);
    let aid = toy_struct(&mut t, 0, "AttributeInitializationData", None);
    t.prop("Float", aid, "BaseValueConstant", 0, 0);
    t.prop("Object", aid, "BaseValueAttribute", 0, object);
    t.prop("Object", aid, "InitializationDefinition", 0, object);
    t.prop("Float", aid, "BaseValueScaleConstant", 0, 0);
    let reward = toy_struct(&mut t, 0, "MissionRewardData", None);
    t.prop("Byte", reward, "CurrencyRewardType", 0, 0);
    t.prop("Struct", reward, "CreditRewardMultiplier", 0, aid);
    t.prop("Struct", reward, "OtherCurrencyReward", 0, aid);
    toy_array_of(&mut t, reward, "RewardItems", "Object", object);
    toy_array_of(&mut t, reward, "RewardItemPools", "Object", object);
    t.prop("Struct", mission, "Reward", 0, reward);
    t.prop("Struct", mission, "AlternativeReward", 0, reward);
    t.prop("Object", mission, "BehaviorProvider", 0, object);
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
    t.prop("Bool", objective, "bAllOrNothing", 0, 0);

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
    // An out array<int> (its inner property is adopted by the parameter).
    let inner = t.prop("Int", 0, "ToyProgressInner", 0, 0);
    let progress = [param("Object", "Mission", 0, object), param("Array", "Progress", CPF_OUT, inner), param("Bool", "bToyFlag", 0, 0)];
    let spec = FunctionSpec::new(NATIVE, "GetObjectivesProgress");
    let ids = t.function(tracker, "GetObjectivesProgress", &progress, None, &[], &spec, |_| None);
    t.p.set_outer(inner, ids.get("Progress"));
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
    // A damage modifier whose class default is not zero (an invented 1.0): the bridge's controller starts from it.
    t.prop("Float", controller, "ReceivedMeleeDamageModifier", 0, 0);
    let defaults = {
        let mut payload = ints(&[0]);
        payload.extend(t.p.float_tag("ReceivedMeleeDamageModifier", 1.0));
        payload.extend_from_slice(&t.p.none());
        payload
    };
    t.p.add_export(controller, "Default__WillowPlayerController", defaults);
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
    // The client movie RPC the use chain overrides (presentation; an empty body here).
    script(&mut t, controller, "ClientGFxPlayMovie", &[], None, |_, _| {});
    let willow_pawn = t.class("WillowPawn", engine_pawn);
    let handle = toy_struct(&mut t, willow_pawn, "ToyConsumerHandle", None);
    t.prop("Int", handle, "PID", 0, 0);
    t.prop("Struct", willow_pawn, "ConsumerHandle", 0, handle);
    native(&mut t, willow_pawn, "GetBehaviorConsumerHandle", &[], Some(("Struct", handle)), NATIVE);
    t.class("WillowPlayerPawn", engine_pawn);
    let ai_pawn = t.class("WillowAIPawn", willow_pawn);
    t.prop("Object", ai_pawn, "BodyClass", 0, object);
    t.prop("Object", ai_pawn, "MissionDirectives", 0, object);
    t.prop("Object", ai_pawn, "AIClass", 0, object);
    t.prop("Object", ai_pawn, "DialogComponent", 0, object);
    // The director's AI class and dialog component (invented classes; the component is an actor component).
    let ai_class = t.class("AIClassDefinition", object);
    t.prop("Int", ai_class, "ToyLevel", 0, 0);
    let actor_component = t.p.add_import("Core", "Class", engine, "ActorComponent");
    let component = t.class("ToyDialogComponent", actor_component);
    t.prop("Float", component, "ToyVolume", 0, 0);
    t.build()
}

/// `TestDirector`: invented director archetypes (`WillowAIPawn`s outside any level).
/// - At the stock path [`MARCUS_PAWN`]: `BodyClass` names `TestMission`'s `Body_Toy` by import; `MissionDirectives`
///   names `Directives_Toy` (a `GlobalsDefinition` with an invented interaction distance of 7); `AIClass` names
///   `AIClass_Toy` (`ToyLevel` 3); `DialogComponent` names `Dialog_Toy`, his subobject, an actor component whose tags
///   start 8 bytes in (`ToyVolume` 0.5).
/// - `GD_Marcus.Character.Pawn_Imported`: `AIClass` and `DialogComponent` are imports of a package no file holds, so
///   neither names an export.
///
/// No provider at [`MARCUS_PROVIDER`], so placing either binds no use chain.
fn director_package() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let none = p.none();
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let mission = p.add_import("Core", "Package", 0, "TestMission");
    let nowhere = p.add_import("Core", "Package", 0, "GD_Nowhere");
    let body = p.add_import("GearboxFramework", "BodyClassDefinition", mission, "Body_Toy");
    let lost_ai = p.add_import("WillowGame", "AIClassDefinition", nowhere, "AIClass_Lost");
    let lost_dialog = p.add_import("WillowGame", "ToyDialogComponent", nowhere, "Dialog_Lost");
    let package_class = p.add_import("Core", "Class", 0, "Package");
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&none);
    let gd = p.add_export(package_class, "GD_Marcus", empty.clone());
    let character = p.add_export(package_class, "Character", empty);
    p.set_outer(character, gd);
    let c = p.add_import("Core", "Class", willow, "GlobalsDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.float_tag("PlayerInteractionDistance", 7.0));
    payload.extend_from_slice(&none);
    let directives = p.add_export(c, "Directives_Toy", payload);
    p.set_outer(directives, character);
    let c = p.add_import("Core", "Class", willow, "AIClassDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.int_tag("ToyLevel", 3));
    payload.extend_from_slice(&none);
    let ai_class = p.add_export(c, "AIClass_Toy", payload);
    p.set_outer(ai_class, character);
    let c = p.add_import("Core", "Class", willow, "ToyDialogComponent");
    let mut payload = [0i32, 0].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
    payload.extend(p.float_tag("ToyVolume", 0.5));
    payload.extend_from_slice(&none);
    let dialog = p.add_export(c, "Dialog_Toy", payload);
    let c = p.add_import("Core", "Class", willow, "WillowAIPawn");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.object_tag("BodyClass", body));
    payload.extend(p.object_tag("MissionDirectives", directives));
    payload.extend(p.object_tag("AIClass", ai_class));
    payload.extend(p.object_tag("DialogComponent", dialog));
    payload.extend_from_slice(&none);
    let pawn = p.add_export(c, "Pawn_Marcus", payload);
    p.set_outer(pawn, character);
    p.set_outer(dialog, pawn);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.object_tag("MissionDirectives", directives));
    payload.extend(p.object_tag("AIClass", lost_ai));
    payload.extend(p.object_tag("DialogComponent", lost_dialog));
    payload.extend_from_slice(&none);
    let imported = p.add_export(c, "Pawn_Imported", payload);
    p.set_outer(imported, character);
    p.build()
}

/// `ToyMission`: one set `SetA` {`Only` (count 1)} that completes the mission, no provider, a reward whose credit
/// multiplier is `multiplier` and which lists `Other` as a reward item when `items`; `Other`, a second mission of the
/// package; `GD_Globals.General.Globals` with an invented interaction distance.
///
/// With `progress` (the `GetObjectivesProgress` tests): `SetA` also holds `Pair` (count 2, `bAllOrNothing`), and the
/// mission's `ObjectiveDefs` are [`Pair`, `Only`, `Spare`], `Spare` (count 1) being in no set.
fn mission_package(optional: bool, multiplier: f32, items: bool, progress: bool) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let none = p.none();
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let dialog = dialog_data(&mut p);
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
    let mut extra = Vec::new();
    if progress {
        for (name, count, all) in [("Pair", 2, true), ("Spare", 1, false)] {
            let mut payload = 0i32.to_le_bytes().to_vec();
            payload.extend(p.int_tag("ObjectiveCount", count));
            if all {
                payload.extend(p.bool_tag("bAllOrNothing", true));
            }
            payload.extend_from_slice(&none);
            let o = p.add_export(c, name, payload);
            p.set_outer(o, mission);
            extra.push(o);
        }
    }
    let mut members = vec![only];
    members.extend(extra.first());
    let count = members.len() as i32;
    let members: Vec<u8> = members.iter().flat_map(|o| o.to_le_bytes()).collect();
    let c = class(&mut p, "MissionObjectiveSetDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.array_tag("ObjectiveDefinitions", count, &members));
    payload.extend(p.bool_tag("bCanCompleteMission", true));
    payload.extend_from_slice(&none);
    let set = p.add_export(c, "SetA", payload);
    p.set_outer(set, mission);
    let mut tags = 0i32.to_le_bytes().to_vec();
    tags.extend(p.array_tag("ObjectiveSetDefs", 1, &set.to_le_bytes()));
    tags.extend(p.object_tag("InitialObjectiveSet", set));
    tags.extend(p.bool_tag("bActivateInitialObjectiveSet", true));
    if progress {
        let defs: Vec<u8> = [extra[0], only, extra[1]].iter().flat_map(|o| o.to_le_bytes()).collect();
        tags.extend(p.array_tag("ObjectiveDefs", 3, &defs));
    }
    let c = class(&mut p, "MissionDefinition");
    let mut other = 0i32.to_le_bytes().to_vec();
    other.extend_from_slice(&none);
    let other = p.add_export(c, "Other", other);
    let mut reward = p.byte_tag("CurrencyRewardType", 0);
    let mut credits = p.float_tag("BaseValueConstant", multiplier);
    credits.extend(p.float_tag("BaseValueScaleConstant", 1.0));
    credits.extend_from_slice(&none);
    reward.extend(p.struct_tag("CreditRewardMultiplier", "AttributeInitializationData", &credits));
    if items {
        reward.extend(p.array_tag("RewardItems", 1, &other.to_le_bytes()));
    }
    reward.extend_from_slice(&none);
    tags.extend(p.struct_tag("Reward", "MissionRewardData", &reward));
    let provider = mission_provider(&mut p, mission, &dialog);
    tags.extend(p.object_tag("BehaviorProvider", provider));
    tags.extend_from_slice(&none);
    p.set_payload(mission, tags);
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
        Env::with_mode(optional, ParityMode::Off)
    }

    fn with_mode(optional: bool, mode: ParityMode) -> Env {
        Env::with_reward(optional, 0.0, false, mode)
    }

    /// A reward with credit multiplier `multiplier` and, with `items`, one reward item.
    fn with_reward(optional: bool, multiplier: f32, items: bool, mode: ParityMode) -> Env {
        Env::build(mission_package(optional, multiplier, items, false), mode)
    }

    /// The `GetObjectivesProgress` mission ([`mission_package`] with `progress`).
    fn with_progress(mode: ParityMode) -> Env {
        Env::build(mission_package(false, 0.0, false, true), mode)
    }

    fn build(mission: Vec<u8>, mode: ParityMode) -> Env {
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
            ("TestMission.upk".into(), mission),
            ("TestDirector.upk".into(), director_package()),
        ];
        let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let defs =
            DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, mode).expect("synthetic store");
        let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::standard().unwrap()), mode);
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

/// The bridge's objects start from their class default objects, as the C++ `Runtime::instantiate` copies them (lane
/// W6-COMBAT-RUN2, decision 2026-11-16-54): the controller's invented default 1.0 survives, and what the bridge sets
/// afterwards (its role) is still set.
#[test]
fn the_bridge_objects_start_from_their_class_defaults() {
    let env = Env::new(false);
    let o = env.script.inner.o;
    let space = &env.world.vm.space;
    assert_eq!(get(space, o.controller, "ReceivedMeleeDamageModifier").unwrap(), Value::Float(1.0));
    assert_eq!(get(space, o.controller, "Role").unwrap(), Value::Byte(3));
    assert!(env.script.errors().is_empty(), "{:?}", env.script.errors());
    // The controller is the world's one local player (`Actor.LocalPlayerControllers`, decision 2026-11-16-81).
    assert_eq!(env.world.actors().borrow().local_players(), [o.controller]);
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
    // Off mode under the gameplay stub policy: the refusal of any unbound native.
    let mut env = Env::new(false);
    let c = Some(env.script.player_controller());
    let def = Value::Object(Some(env.mission.definition()));
    let refused = env.call("WillowGame.MissionTracker.GetMissionStatus", c, vec![def.clone()], MissionScript::n_status);
    assert!(matches!(refused, Err(BehaviorError::Vm(VmError::UnboundNative(k))) if k == "MissionTracker.GetMissionStatus(object)"));
    // A policy that allows the key: the stub.
    env.vm().stub_policy = ow_vm::api::StubPolicy::LogAndZero;
    let before = env.vm().log.len();
    let got = env.call("WillowGame.MissionTracker.GetMissionStatus", c, vec![def], MissionScript::n_status).unwrap();
    assert_eq!(got, Value::Byte(0));
    assert_eq!(env.vm().log[before], "UNIMPLEMENTED MissionTracker.GetMissionStatus(object) (outside the mission bridge binding)");
    // Reference mode: the C++ stub line and zero, whatever the policy.
    let mut env = Env::with_mode(false, ParityMode::Reference);
    let c = Some(env.script.player_controller());
    let def = Value::Object(Some(env.mission.definition()));
    let before = env.vm().log.len();
    let got = env.call("WillowGame.MissionTracker.GetMissionStatus", c, vec![def], MissionScript::n_status).unwrap();
    assert_eq!(got, Value::Byte(0));
    assert_eq!(env.vm().log[before], "UNIMPLEMENTED MissionTracker.GetMissionStatus(object) (outside the mission bridge binding)");
}

/// The C++ "not implemented" paths (architecture R3): Reference logs the C++ stub line and goes on; Off answers the
/// honest `NotYet` (each caller names its kind: `NotRead` credits, `LayerAbsent` W6-LOOT items, `NotPorted` others).
#[test]
fn not_implemented_is_the_cxx_line_in_reference_and_not_yet_in_off() {
    let mut env = Env::with_mode(false, ParityMode::Reference);
    let before = env.vm().log.len();
    MissionScript::not_implemented(env.vm(), "X.Y: a path", NotYet::not_read("x", RECORD)).unwrap();
    assert_eq!(env.vm().log[before], "UNIMPLEMENTED X.Y: a path (not implemented)");
    env.call("Engine.BehaviorBase.RunBehaviors", None, vec![], MissionScript::n_run_behaviors).unwrap();
    assert!(env.vm().log.last().unwrap().ends_with("(outside the level-up stand-in)"));
    let mut env = Env::new(false);
    let before = env.vm().log.len();
    let refused = MissionScript::not_implemented(env.vm(), "X.Y: a path", NotYet::not_read("x", RECORD)).unwrap_err();
    assert!(matches!(refused.not_yet(), Some(n) if n.missing == ow_core::Missing::NotRead));
    assert_eq!(env.vm().log.len(), before, "no stub line in Off mode");
    let outside = env.call("Engine.BehaviorBase.RunBehaviors", None, vec![], MissionScript::n_run_behaviors);
    assert!(matches!(outside.unwrap_err().not_yet(), Some(n) if n.missing == ow_core::Missing::NotPorted));
}

/// `GetExperienceReward` of a mission with an optional objective: `NotYet` in Off mode; Reference goes on as the C++
/// (here to the reward data, whose percentage the invented reward does not hold).
#[test]
fn optional_objective_experience_by_mode() {
    let mut env = Env::with_mode(true, ParityMode::Reference);
    let def = Some(env.mission.definition());
    let xp = env.call("WillowGame.MissionDefinition.GetExperienceReward", def, vec![Value::Object(None), Value::Bool(false)], MissionScript::n_xp_reward);
    assert_eq!(xp.unwrap_err().to_string(), "mission script bridge: the reward has no ExperienceRewardPercentage");
    assert!(env.vm().log.iter().any(|l| l == "UNIMPLEMENTED MissionDefinition.GetExperienceReward: optional objective experience (not implemented)"));
}

const PROGRESS: &str = "WillowGame.MissionTracker.GetObjectivesProgress";
const PROGRESS_KEY: &str = "MissionTracker.GetObjectivesProgress(object,array,bool)";

/// `GetObjectivesProgress` through the VM on the bridge's tracker: the out array as the caller sees it afterwards.
fn objectives_progress(env: &mut Env, mission: ObjRef, passed: &[i32], flag: bool) -> Result<Vec<Value>, VmError> {
    let tracker = env.script.inner.o.tracker;
    let vm = env.vm();
    let f = vm.find_function(PROGRESS).unwrap();
    let passed = Value::Array(Rc::new(passed.iter().map(|&v| Value::Int(v)).collect()));
    let done = vm.call(f, Some(tracker), vec![Value::Object(Some(mission)), passed, Value::Bool(flag)])?;
    Ok(done.outs[1].elements().unwrap().to_vec())
}

fn ints_of(values: &[i32]) -> Vec<Value> {
    values.iter().map(|&v| Value::Int(v)).collect()
}

/// `GetObjectivesProgress` in Off mode (NATIVE_SLICE_OFF_STOPS.md, UNVERIFIED; decision 2026-11-19-05): another
/// mission has no record and the array stays as passed; this mission's record gives one value per `ObjectiveDefs`
/// entry ([`Pair`, `Only`, `Spare`]), `Spare` (in no set) 0. With the flag, an all-or-nothing objective at 0 stays 0;
/// one with progress needs the unread "is complete" test: `NotYet { NotRead }`.
#[test]
fn objectives_progress_in_off_mode() {
    let mut env = Env::with_progress(ParityMode::Off);
    assert!(env.vm().has_native(PROGRESS_KEY));
    let (m, s) = (env.mission.clone(), env.script.clone());
    let (def, other) = (m.definition(), env.object("Other"));
    assert_eq!(objectives_progress(&mut env, other, &[7], false).unwrap(), ints_of(&[7]), "no record: left as passed");
    assert_eq!(objectives_progress(&mut env, def, &[7], false).unwrap(), ints_of(&[0, 0, 0]), "a record, before accept");
    assert!(s.accept(env.vm(), &DetSet::new()).unwrap());
    m.update_objective(env.vm(), "Only", 0).unwrap();
    assert_eq!(objectives_progress(&mut env, def, &[], false).unwrap(), ints_of(&[0, 1, 0]));
    assert_eq!(objectives_progress(&mut env, def, &[], true).unwrap(), ints_of(&[0, 1, 0]), "Pair is 0 either way");
    m.update_objective(env.vm(), "Pair", 0).unwrap();
    assert_eq!(objectives_progress(&mut env, def, &[], false).unwrap(), ints_of(&[1, 1, 0]));
    let refused = objectives_progress(&mut env, def, &[], true).unwrap_err();
    assert!(matches!(&refused, VmError::NotYet(n) if n.missing == ow_core::Missing::NotRead), "{refused}");
}

/// Reference keeps the C++ stub (R3): the bridge does not bind `GetObjectivesProgress` there.
#[test]
fn objectives_progress_is_unbound_in_reference_mode() {
    let env = Env::with_progress(ParityMode::Reference);
    assert!(!env.world.vm.has_native(PROGRESS_KEY));
    assert!(env.world.vm.has_native("MissionTracker.GetMissionStatus(object)"), "the bridge's other natives are bound");
}

/// One bridge per fresh world: `new` refuses a world that already has actors.
#[test]
fn a_second_bridge_on_one_world_is_refused() {
    let mut env = Env::new(false);
    let mission = env.mission.clone();
    let refused = MissionScript::new(&mut env.world, &mission).unwrap_err();
    assert_eq!(refused.to_string(), "mission script bridge: the world already has actors (one bridge per fresh world)");
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
/// currency for type 0 with a zero multiplier, and
/// `GetExperienceReward` of a mission with an optional objective is `NotYet`.
#[test]
fn the_reward_natives_answer_from_the_data() {
    let mut env = Env::new(false);
    let def = Some(env.mission.definition());
    assert_eq!(env.call("WillowGame.MissionDefinition.GetGameStage", def, vec![], MissionScript::n_game_stage).unwrap(), Value::Int(0));
    let t = env.call("WillowGame.MissionDefinition.GetCurrencyRewardType", def, vec![Value::Bool(false)], MissionScript::n_currency_type);
    assert_eq!(t.unwrap(), Value::Byte(0));
    let c = env.call("WillowGame.MissionDefinition.GetCurrencyReward", def, vec![Value::Object(None), Value::Bool(false)], MissionScript::n_currency);
    assert_eq!(c.unwrap(), Value::Int(0), "a zero credit multiplier: 0 credits");
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


/// The behavior kernel's natives have one owner (decision 2026-11-16-50): the world handlers with the kernel pack,
/// the combat pack and the bridge's use chain, registered in every order, give the same world. Before the kernel
/// pack, the bridge refused a key the combat pack had bound first (`duplicate native registration`), and the combat
/// pack skipped a key the bridge had bound, so its script behaviors' output links went to the bridge's list.
#[test]
fn the_kernel_combat_and_mission_packs_give_one_world_in_every_order() {
    #[derive(Debug, Clone, Copy)]
    enum P {
        Behavior,
        Combat,
        Mission,
    }
    use P::{Behavior, Combat, Mission};
    let orders = [
        [Behavior, Combat, Mission],
        [Behavior, Mission, Combat],
        [Combat, Behavior, Mission],
        [Combat, Mission, Behavior],
        [Mission, Behavior, Combat],
        [Mission, Combat, Behavior],
    ];
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut worlds = Vec::new();
        for order in orders {
            let mut env = Env::with_mode(false, mode);
            let s = env.script.clone();
            for p in order {
                let vm = env.vm();
                match p {
                    Behavior => {
                        ow_behavior::handlers::Pack.register(vm).unwrap();
                        natives::Pack.register(vm).unwrap();
                    }
                    Combat => ow_combat::hit::Pack.register(vm).unwrap(),
                    Mission => s.bind_use(vm).unwrap(),
                }
            }
            let vm = env.vm();
            // A pawn the combat pack hands a handle to.
            let pawn = new_object(vm, "WillowGame.WillowAIPawn").unwrap();
            s.set_pid(vm, pawn, -1).unwrap();
            ow_combat::death::prepare_for_death(vm, pawn).unwrap();
            let call = |vm: &mut Vm, path: &str, this: Option<ObjRef>, args: Vec<Value>| {
                let f = vm.find_function(path).unwrap();
                vm.call(f, this, args).map(|r| r.ret)
            };
            let handle = call(vm, natives::CONSUMER_HANDLE, Some(pawn), vec![]).unwrap();
            let custom = Value::Name(vm.space.names.intern("Custom"));
            call(vm, natives::BROADCAST_FROM_SCRIPT, None, vec![handle.clone(), custom]).unwrap();
            // The director's handle reaches the bridge's body (no director placed: not registered, false).
            let pid = s.inner.marcus_pid.get().unwrap();
            let marcus = with_field(&vm.space, &handle, "PID", Value::Int(pid)).unwrap();
            let idle = Value::Name(vm.space.names.intern("Idle"));
            let enabled = call(vm, natives::IS_SEQUENCE_ENABLED, None, vec![marcus, Value::Object(Some(pawn)), idle]);
            // A handle nobody owns: Off the record's nothing; Reference the C++ combat body (its note for any PID).
            let stranger = with_field(&vm.space, &handle, "PID", Value::Int(55)).unwrap();
            let name = Value::Name(vm.space.names.intern("Stranger"));
            call(vm, natives::BROADCAST_FROM_SCRIPT, None, vec![stranger, name]).unwrap();
            // An unprepared pawn's handle: Off the plain read; Reference two crates' C++ bodies, so NotYet.
            let other = new_object(vm, "WillowGame.WillowAIPawn").unwrap();
            s.set_pid(vm, other, -1).unwrap();
            let unowned = call(vm, natives::CONSUMER_HANDLE, Some(other), vec![]).map_err(|e| e.to_string());
            let (_, links) = natives::collect(vm, |vm| {
                for id in [1, 0, 1] {
                    call(vm, natives::OUTPUT_LINK, None, vec![Value::Int(0), Value::Int(id)]).unwrap();
                }
            })
            .unwrap();
            let events = ow_combat::hit::with_state(vm, |c| c.events.clone()).unwrap();
            let owners = [MARCUS_PID, 100].map(|pid| natives::owner_of(vm, pid).unwrap());
            worlds.push(format!("{handle:?} {events:?} {enabled:?} {links:?} {owners:?} {unowned:?} {:?}", vm.log));
        }
        let first = worlds[0].clone();
        for (order, world) in orders.iter().zip(&worlds) {
            assert_eq!(world, &first, "{mode:?} {order:?}");
        }
        assert!(first.contains("BroadcastBehaviorEventFromScript Custom"), "{first}");
        assert!(first.contains("Ok(Bool(false)) [1, 0, 1]"), "{first}");
        assert!(first.contains(r#"[Some(Crate("ow-mission")), Some(Crate("ow-combat"))]"#), "{first}");
        match mode {
            ParityMode::Off => {
                assert!(!first.contains("BroadcastBehaviorEventFromScript Stranger"), "{first}");
                assert!(first.contains("Ok(Struct("), "{first}");
            }
            ParityMode::Reference => {
                assert!(first.contains("BroadcastBehaviorEventFromScript Stranger"), "{first}");
                assert!(first.contains("two crates' Reference bodies"), "{first}");
            }
        }
    }
}

/// A consumer the kernel registered before the director's use chain is bound keeps its handle: the bridge takes the
/// next free one and answers for that one only (review r1 of PR #352).
#[test]
fn a_kernel_consumer_registered_first_keeps_its_handle() {
    let mut env = Env::with_mode(false, ParityMode::Off);
    let s = env.script.clone();
    let vm = env.vm();
    natives::Pack.register(vm).unwrap();
    let skill = new_object(vm, "WillowGame.WillowAIPawn").unwrap();
    let f = vm.find_function(natives::REGISTER_CONSUMER).unwrap();
    let handle = vm.call(f, None, vec![Value::Object(Some(skill))]).unwrap().ret;
    assert_eq!(field(&vm.space, &handle, "PID"), Some(&Value::Int(0)));
    s.bind_use(vm).unwrap();
    assert_eq!(s.inner.marcus_pid.get(), Some(1));
    assert_eq!(natives::owner_of(vm, 0).unwrap(), Some(natives::Owner::Kernel));
    assert_eq!(natives::owner_of(vm, 1).unwrap(), Some(natives::Owner::Crate(CONSUMER_OWNER)));
    // The bridge answers for its own handle only: with the director's provider lost, its body refuses; the kernel's
    // consumer gets the kernel rule (no provider registered: false).
    s.inner.marcus_lost.set(true);
    let f = vm.find_function(natives::IS_SEQUENCE_ENABLED).unwrap();
    let idle = Value::Name(vm.space.names.intern("Idle"));
    let at = |vm: &mut Vm, pid: i32| {
        let h = with_field(&vm.space, &handle, "PID", Value::Int(pid)).unwrap();
        vm.call(f, None, vec![h, Value::Object(Some(skill)), idle.clone()]).map(|r| r.ret)
    };
    assert!(matches!(at(vm, 1), Err(VmError::NotYet(_))), "the bridge's body");
    assert_eq!(at(vm, 0).unwrap(), Value::Bool(false), "the kernel's rule");
}

/// Reference on a VM holding one crate is that crate's C++ binding (review r1 of PR #352). The bridge alone: the
/// broadcast it never bound is the stub, a pawn other than the director is outside its binding, and an unowned
/// sequence query is its own body (false, no stub). The combat pack alone: the sequence query it never bound is the
/// stub, and its broadcast body writes its note for a PID it does not know.
#[test]
fn reference_follows_the_cxx_binding_of_the_one_crate_present() {
    let call = |vm: &mut Vm, path: &str, this: Option<ObjRef>, args: Vec<Value>| {
        let f = vm.find_function(path).unwrap();
        vm.call(f, this, args).map(|r| r.ret)
    };
    let key = |vm: &mut Vm, path: &str| {
        let f = vm.find_function(path).unwrap();
        vm.function(f).and_then(|d| d.native_key.clone()).unwrap()
    };
    // The bridge alone.
    let mut env = Env::with_mode(false, ParityMode::Reference);
    let s = env.script.clone();
    let vm = env.vm();
    vm.stub_policy = ow_vm::api::StubPolicy::LogAndZero;
    s.bind_use(vm).unwrap();
    let pawn = new_object(vm, "WillowGame.WillowAIPawn").unwrap();
    s.set_pid(vm, pawn, 55).unwrap();
    let handle = get(&vm.space, pawn, "ConsumerHandle").unwrap();
    let name = Value::Name(vm.space.names.intern("Custom"));
    call(vm, natives::BROADCAST_FROM_SCRIPT, None, vec![handle.clone(), name.clone()]).unwrap();
    let stub = format!("UNIMPLEMENTED {}", key(vm, natives::BROADCAST_FROM_SCRIPT));
    assert_eq!(vm.log.last(), Some(&stub));
    call(vm, natives::CONSUMER_HANDLE, Some(pawn), vec![]).unwrap();
    let outside = format!("UNIMPLEMENTED {} (outside the mission bridge binding)", key(vm, natives::CONSUMER_HANDLE));
    assert_eq!(vm.log.last(), Some(&outside));
    let before = vm.log.len();
    let idle = Value::Name(vm.space.names.intern("Idle"));
    let enabled = call(vm, natives::IS_SEQUENCE_ENABLED, None, vec![handle.clone(), Value::Object(Some(pawn)), idle.clone()]);
    assert_eq!(enabled.unwrap(), Value::Bool(false));
    assert_eq!(vm.log.len(), before, "the bridge's body, no stub line");
    // The combat pack alone.
    let mut env = Env::with_mode(false, ParityMode::Reference);
    let s = env.script.clone();
    let vm = env.vm();
    vm.stub_policy = ow_vm::api::StubPolicy::LogAndZero;
    ow_combat::hit::Pack.register(vm).unwrap();
    let pawn = new_object(vm, "WillowGame.WillowAIPawn").unwrap();
    s.set_pid(vm, pawn, 55).unwrap();
    let handle = get(&vm.space, pawn, "ConsumerHandle").unwrap();
    let name = Value::Name(vm.space.names.intern("Custom"));
    let idle = Value::Name(vm.space.names.intern("Idle"));
    call(vm, natives::IS_SEQUENCE_ENABLED, None, vec![handle.clone(), Value::Object(Some(pawn)), idle]).unwrap();
    let stub = format!("UNIMPLEMENTED {}", key(vm, natives::IS_SEQUENCE_ENABLED));
    assert_eq!(vm.log.last(), Some(&stub));
    call(vm, natives::BROADCAST_FROM_SCRIPT, None, vec![handle, name]).unwrap();
    let events = ow_combat::hit::with_state(vm, |c| c.events.clone()).unwrap();
    assert!(events.contains(&"BroadcastBehaviorEventFromScript Custom".to_string()), "{events:?}");
}

/// The level-up's `RunBehaviors` outside the level-up is the C++ log line; inside it is `NotYet` (lane W6-SKILLS);
/// the health pool is `NotYet`; the director's dialog is `NotYet`.
#[test]
fn the_stand_ins_of_other_lanes_are_not_yet() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    s.inner.state.borrow_mut().in_level_up = true;
    let inside = env.call("Engine.BehaviorBase.RunBehaviors", None, vec![], MissionScript::n_run_behaviors);
    assert!(inside.unwrap_err().not_yet().is_some());
    assert!(s.set_player_health(20.0).unwrap_err().not_yet().is_some());
    // The director's dialog runs since the dialog landed (a_bridge_dialog_event_is_seen_by_the_tracker); its component
    // without a director is a typed error, not a stub.
    let component = s.inner.o.globals;
    s.inner.state.borrow_mut().dialog_component = Some(component);
    let talk = env.call("GearboxFramework.GearboxDialogComponent.TriggerEvent", Some(component), vec![], MissionScript::n_trigger_event);
    assert_eq!(talk.unwrap_err().to_string(), "mission script bridge: the director's component has no director");
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

/// The last line `env`'s VM logged.
fn last_log(env: &mut Env) -> String {
    env.vm().log.last().cloned().unwrap_or_default()
}

/// `GetCurrencyReward` with a non-zero credit multiplier (the credit formula's values are not decoded): `NotYet {
/// NotRead }` in Off mode; Reference logs the C++ line and answers 0 credits.
#[test]
fn credits_with_a_multiplier_by_mode() {
    let args = || vec![Value::Object(None), Value::Bool(false)];
    let mut env = Env::with_reward(false, 2.0, false, ParityMode::Off);
    let def = Some(env.mission.definition());
    let off = env.call("WillowGame.MissionDefinition.GetCurrencyReward", def, args(), MissionScript::n_currency);
    assert!(matches!(off.unwrap_err().not_yet(), Some(n) if n.missing == ow_core::Missing::NotRead));
    let mut env = Env::with_reward(false, 2.0, false, ParityMode::Reference);
    let def = Some(env.mission.definition());
    let got = env.call("WillowGame.MissionDefinition.GetCurrencyReward", def, args(), MissionScript::n_currency);
    assert_eq!(got.unwrap(), Value::Int(0));
    assert_eq!(
        last_log(&mut env),
        "UNIMPLEMENTED MissionDefinition.GetCurrencyReward: credits need MissionCreditRewardFormula attribute values (not implemented)"
    );
}

/// `GetCurrencyReward` of a mission with an optional objective (their currency rewards are not ported): `NotYet {
/// NotPorted }` in Off mode; Reference logs the C++ line and goes on (a zero multiplier: 0 credits).
#[test]
fn optional_objective_currency_by_mode() {
    let args = || vec![Value::Object(None), Value::Bool(false)];
    let mut env = Env::with_mode(true, ParityMode::Off);
    let def = Some(env.mission.definition());
    let off = env.call("WillowGame.MissionDefinition.GetCurrencyReward", def, args(), MissionScript::n_currency);
    assert!(matches!(off.unwrap_err().not_yet(), Some(n) if n.missing == ow_core::Missing::NotPorted));
    let mut env = Env::with_mode(true, ParityMode::Reference);
    let def = Some(env.mission.definition());
    let got = env.call("WillowGame.MissionDefinition.GetCurrencyReward", def, args(), MissionScript::n_currency);
    assert_eq!(got.unwrap(), Value::Int(0));
    assert_eq!(
        last_log(&mut env),
        "UNIMPLEMENTED MissionDefinition.GetCurrencyReward: optional objective currency rewards (not implemented)"
    );
}

/// `GetItemRewardsForPlayer` with a reward item (balance generation and pool rolls are lane W6-LOOT's): `NotYet {
/// LayerAbsent }` in Off mode; Reference logs the C++ line and gives no items.
#[test]
fn reward_items_by_mode() {
    let args = || vec![Value::Object(None), Value::Object(None)];
    let mut env = Env::with_reward(false, 0.0, true, ParityMode::Off);
    let def = Some(env.mission.definition());
    let off = env.call("WillowGame.MissionDefinition.GetItemRewardsForPlayer", def, args(), MissionScript::n_item_rewards);
    assert!(matches!(off.unwrap_err().not_yet(), Some(n) if n.missing == ow_core::Missing::LayerAbsent(LOOT_LAYER)));
    let mut env = Env::with_reward(false, 0.0, true, ParityMode::Reference);
    let def = Some(env.mission.definition());
    let got = env.call("WillowGame.MissionDefinition.GetItemRewardsForPlayer", def, args(), MissionScript::n_item_rewards);
    assert_eq!(got.unwrap(), Value::None);
    assert_eq!(
        last_log(&mut env),
        "UNIMPLEMENTED MissionDefinition.GetItemRewardsForPlayer: item rewards need balance generation and pool rolls (NATIVE_LOOT.md) (not implemented)"
    );
}

/// The default value of the struct type that property `prop` of class `class` declares.
fn struct_zero(vm: &mut Vm, class: &str, prop: &str) -> Value {
    let c = vm.find_class(class).unwrap();
    let p = vm.space.prop(c, prop).expect("struct property");
    let Some(PropKind::Struct(ty)) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
        panic!("{class}.{prop} is not a struct");
    };
    vm.space.classes.struct_defaults(ty).unwrap()
}

/// A `BehaviorContextData` value with selector `selector` and instance-data name `name`.
fn context_data(env: &mut Env, selector: u8, name: &str) -> Value {
    let vm = env.vm();
    let zero = struct_zero(vm, "Engine.BehaviorBase", "ToyContextData");
    let named = Value::Name(vm.space.names.intern(name));
    let v = with_field(&vm.space, &zero, "BehaviorContext", Value::Byte(selector)).unwrap();
    with_field(&vm.space, &v, "InstancedDataContextName", named).unwrap()
}

/// `GetBehaviorContext` with a named `InstancedDataContextName` (the instance-data path is not ported): `NotYet {
/// NotPorted }` in Off mode; Reference logs the C++ line and answers None. Without a name the selector decides.
#[test]
fn instance_data_context_by_mode() {
    let mut env = Env::new(false);
    let a = env.script.player_controller();
    let unnamed = context_data(&mut env, 0, "None");
    let got = env.call("Engine.BehaviorBase.GetBehaviorContext", None, vec![unnamed, Value::Object(Some(a))], MissionScript::n_behavior_context);
    assert_eq!(got.unwrap(), Value::Object(Some(a)));
    let named = context_data(&mut env, 0, "ToyInstance");
    let off = env.call("Engine.BehaviorBase.GetBehaviorContext", None, vec![named, Value::Object(Some(a))], MissionScript::n_behavior_context);
    assert!(matches!(off.unwrap_err().not_yet(), Some(n) if n.missing == ow_core::Missing::NotPorted));
    let mut env = Env::with_mode(false, ParityMode::Reference);
    let a = env.script.player_controller();
    let named = context_data(&mut env, 0, "ToyInstance");
    let got = env.call("Engine.BehaviorBase.GetBehaviorContext", None, vec![named, Value::Object(Some(a))], MissionScript::n_behavior_context);
    assert_eq!(got.unwrap(), Value::Object(None));
    assert_eq!(last_log(&mut env), "UNIMPLEMENTED BehaviorBase.GetBehaviorContext instance-data path (not implemented)");
}

/// Once the director's provider is lost (a failed lend), every reader and caller of it is refused with
/// `PROVIDER_LOST`: the registration check (`IsBehaviorSequenceEnabled`, `ActivateBehaviorEventFromScript`), the
/// provider resolver's path branch and the use key (whose script run records the refusal).
#[test]
fn a_lost_director_provider_refuses_its_readers() {
    let mut env = Env::new(false);
    let s = env.script.clone();
    s.inner.has_marcus.set(true);
    s.inner.marcus_lost.set(true);
    let other = Value::Object(Some(env.object("Other")));
    let lost = |r: Result<Value, MissionError>| matches!(r, Err(e) if e.not_yet().is_some_and(|n| n.what == tracker::PROVIDER_LOST));
    let enabled = env.call("GearboxFramework.BehaviorKernel.IsBehaviorSequenceEnabled", None, vec![Value::None, other.clone(), Value::None], MissionScript::n_sequence_enabled);
    assert!(lost(enabled));
    let fire = env.call("GearboxFramework.BehaviorKernel.ActivateBehaviorEventFromScript", None, vec![Value::None, other, Value::None], MissionScript::n_event_from_script);
    assert!(lost(fire));
    let vm = env.vm();
    let zero = struct_zero(vm, "GearboxFramework.ToyPathHolder", "ToyPath");
    let named = Value::Name(vm.space.names.intern("Somewhere"));
    let path = with_field(&vm.space, &zero, "PathComponentNames", named).unwrap();
    let resolve = env.call(
        "GearboxFramework.BehaviorHelpers.ResolveBehaviorProviderDefinitionReference",
        None,
        vec![Value::Object(None), Value::Object(None), path],
        MissionScript::n_resolve_provider,
    );
    assert!(lost(resolve));
    s.use_marcus(env.vm(), &DetSet::new()).unwrap();
    assert!(s.errors().iter().any(|e| e.contains(tracker::PROVIDER_LOST)), "{:?}", s.errors());
}

// ---------------------------------------------------------------------------------------------------------------
// The dialog: one dialog shared by the tracker's behavior and the bridge's component native
// ---------------------------------------------------------------------------------------------------------------

const VARIABLE_TYPES: [&str; 3] = ["BVAR_None", "BVAR_Object", "BVAR_Int"];
const LINK_TYPES: [&str; 3] = ["BVARLINK_Unknown", "BVARLINK_Input", "BVARLINK_Output"];

/// The provider vocabulary the kernel decodes (the same shape as `crate::tracker`'s test fixture).
fn provider_vocabulary(t: &mut ScriptToy, object: i32) {
    let bpd = t.class("BehaviorProviderDefinition", object);
    enum_export(t, bpd, "EBehaviorVariableType", &VARIABLE_TYPES);
    enum_export(t, bpd, "EBehaviorVariableLinkType", &LINK_TYPES);
    let sub = toy_struct(t, 0, "SubarrayData", None);
    t.prop("Int", sub, "ArrayIndexAndLength", 0, 0);
    let defaults = {
        let mut d = t.p.bool_tag("bEnabled", true);
        d.extend_from_slice(&t.none);
        d
    };
    let user = toy_struct(t, 0, "BehaviorEventUserData", Some(&defaults));
    t.prop("Name", user, "EventName", 0, 0);
    t.prop("Bool", user, "bEnabled", 0, 0);
    t.prop("Int", user, "MaxTriggerCount", 0, 0);
    t.prop("Float", user, "ReTriggerDelay", 0, 0);
    let event = toy_struct(t, 0, "BehaviorEventData2", None);
    t.prop("Struct", event, "UserData", 0, user);
    t.prop("Struct", event, "OutputVariables", 0, sub);
    t.prop("Struct", event, "OutputLinks", 0, sub);
    let data = toy_struct(t, 0, "BehaviorData2", None);
    t.prop("Object", data, "Behavior", 0, object);
    t.prop("Struct", data, "LinkedVariables", 0, sub);
    t.prop("Struct", data, "OutputLinks", 0, sub);
    let link = toy_struct(t, 0, "BehaviorOutputLinkData", None);
    t.prop("Int", link, "LinkIdAndLinkedBehavior", 0, 0);
    t.prop("Float", link, "ActivateDelay", 0, 0);
    let seq = toy_struct(t, 0, "BehaviorSequenceData", None);
    t.prop("Name", seq, "BehaviorSequenceName", 0, 0);
    t.prop("Bool", seq, "bEnabledOnSpawn", 0, 0);
    t.prop("Bool", seq, "bSequenceEnabledMutex", 0, 0);
    t.prop("Object", seq, "CustomEnableCondition", 0, object);
    toy_array_of(t, seq, "EventData2", "Struct", event);
    toy_array_of(t, seq, "BehaviorData2", "Struct", data);
    toy_array_of(t, seq, "ConsolidatedOutputLinkData", "Struct", link);
    toy_array_of(t, bpd, "BehaviorSequences", "Struct", seq);
}

/// The dialog vocabulary the component trigger and `Behavior_TriggerDialogEvent` read (the names `crate::dialog`
/// reads; NATIVE_DIALOG.md), the smallest route: a group's events, a talk act's talk data, a body class.
fn dialog_vocabulary(t: &mut ScriptToy, object: i32) {
    let event = toy_struct(t, 0, "DialogEventData", None);
    t.prop("Object", event, "Tag", 0, object);
    t.prop("Bool", event, "bEnabled", 0, 0);
    t.prop("Object", event, "OutputAction", 0, object);
    let entry = toy_struct(t, 0, "TalkEntry", None);
    t.prop("Object", entry, "NameTag", 0, object);
    t.prop("Object", entry, "TalkAkEvent", 0, object);
    t.class("GearboxDialogEventTag", object);
    let group = t.class("GearboxDialogGroup", object);
    toy_array_of(t, group, "DialogEvents", "Struct", event);
    let talk = t.class("GearboxDialogAct_Talk", object);
    toy_array_of(t, talk, "TalkData", "Struct", entry);
    t.prop("Float", talk, "OutputDelay", 0, 0);
    t.class("DialogNameTag", object);
    let body = t.class("BodyClassDefinition", object);
    t.prop("Object", body, "DialogName", 0, object);
    toy_array_of(t, body, "DialogGroups", "Object", group);
    let behavior = t.class("Behavior_TriggerDialogEvent", object);
    t.prop("Object", behavior, "EventTag", 0, object);
    t.prop("Object", behavior, "Group", 0, object);
    t.prop("Bool", behavior, "bForcePlayImmediate", 0, 0);
}

/// The dialog objects of the mission package: a name tag, an audio event, two tags (`Tag_Use` for the director's
/// component, `Tag_Mission` for the mission's behavior), a talk act for each whose one entry is the name tag's, one
/// group listing both events, and a body class whose dialog name is the name tag and whose group is that group.
struct DialogData {
    tag_mission: i32,
    group: i32,
}

fn dialog_data(p: &mut PackageBuilder) -> DialogData {
    let none = p.none();
    let gearbox = p.add_import("Core", "Package", 0, "GearboxFramework");
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let class = |p: &mut PackageBuilder, outer: i32, name: &str| p.add_import("Core", "Class", outer, name);
    let add = |p: &mut PackageBuilder, class: i32, name: &str, tags: Vec<Vec<u8>>| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        for t in tags {
            payload.extend(t);
        }
        payload.extend_from_slice(&none);
        p.add_export(class, name, payload)
    };
    let c = class(p, gearbox, "DialogNameTag");
    let name_tag = add(p, c, "DialogName_Toy", vec![]);
    let c = class(p, engine, "AkEvent");
    let ak = add(p, c, "Ak_Toy", vec![]);
    let c = class(p, gearbox, "GearboxDialogEventTag");
    let tag_use = add(p, c, "Tag_Use", vec![]);
    let tag_mission = add(p, c, "Tag_Mission", vec![]);
    let talk_data = |p: &mut PackageBuilder| {
        let mut entry = p.object_tag("NameTag", name_tag);
        entry.extend(p.object_tag("TalkAkEvent", ak));
        entry.extend_from_slice(&none);
        p.array_tag("TalkData", 1, &entry)
    };
    let c = class(p, gearbox, "GearboxDialogAct_Talk");
    let t = talk_data(p);
    let act_use = add(p, c, "Act_Use", vec![t]);
    let t = talk_data(p);
    let act_mission = add(p, c, "Act_Mission", vec![t]);
    let mut events = Vec::new();
    for (tag, act) in [(tag_use, act_use), (tag_mission, act_mission)] {
        events.extend(p.object_tag("Tag", tag));
        events.extend(p.bool_tag("bEnabled", true));
        events.extend(p.object_tag("OutputAction", act));
        events.extend_from_slice(&none);
    }
    let c = class(p, gearbox, "GearboxDialogGroup");
    let t = p.array_tag("DialogEvents", 2, &events);
    let group = add(p, c, "Group_Toy", vec![t]);
    let c = class(p, gearbox, "BodyClassDefinition");
    let t1 = p.object_tag("DialogName", name_tag);
    let t2 = p.array_tag("DialogGroups", 1, &group.to_le_bytes());
    add(p, c, "Body_Toy", vec![t1, t2]);
    DialogData { tag_mission, group }
}

/// The mission's provider: one sequence `Main` whose event `Chat` (link id 0) runs `Talk`, a
/// `Behavior_TriggerDialogEvent` of `Tag_Mission` on `Group_Toy` that plays at once (`bForcePlayImmediate`).
fn mission_provider(p: &mut PackageBuilder, mission: i32, dialog: &DialogData) -> i32 {
    let none = p.none();
    let gearbox = p.add_import("Core", "Package", 0, "GearboxFramework");
    let c = p.add_import("Core", "Class", gearbox, "BehaviorProviderDefinition");
    let provider = p.add_export(c, "ToyMissionBpd", Vec::new());
    p.set_outer(provider, mission);
    let c = p.add_import("Core", "Class", gearbox, "Behavior_TriggerDialogEvent");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.object_tag("EventTag", dialog.tag_mission));
    payload.extend(p.object_tag("Group", dialog.group));
    payload.extend(p.bool_tag("bForcePlayImmediate", true));
    payload.extend_from_slice(&none);
    let talk = p.add_export(c, "Talk", payload);
    p.set_outer(talk, provider);
    let sub = |p: &mut PackageBuilder, name: &str, first: i32, length: i32| {
        let mut body = p.int_tag("ArrayIndexAndLength", (first << 16) | length);
        body.extend_from_slice(&none);
        p.struct_tag(name, "SubarrayData", &body)
    };
    let mut user = p.name_tag("EventName", "Chat");
    user.extend_from_slice(&none);
    let mut event = p.struct_tag("UserData", "BehaviorEventUserData", &user);
    event.extend(sub(p, "OutputVariables", 0, 0));
    event.extend(sub(p, "OutputLinks", 0, 1));
    event.extend_from_slice(&none);
    let mut data = p.object_tag("Behavior", talk);
    data.extend(sub(p, "LinkedVariables", 0, 0));
    data.extend(sub(p, "OutputLinks", 0, 0));
    data.extend_from_slice(&none);
    let mut link = p.int_tag("LinkIdAndLinkedBehavior", 0);
    link.extend(p.float_tag("ActivateDelay", 0.0));
    link.extend_from_slice(&none);
    let mut seq = p.name_tag("BehaviorSequenceName", "Main");
    seq.extend(p.bool_tag("bEnabledOnSpawn", true));
    seq.extend(p.array_tag("EventData2", 1, &event));
    seq.extend(p.array_tag("BehaviorData2", 1, &data));
    seq.extend(p.array_tag("ConsolidatedOutputLinkData", 1, &link));
    seq.extend_from_slice(&none);
    let mut tags = 0i32.to_le_bytes().to_vec();
    tags.extend(p.array_tag("BehaviorSequences", 1, &seq));
    tags.extend_from_slice(&none);
    p.set_payload(provider, tags);
    provider
}

/// The dialog effects the tracker raised since the last drain: (tag, talker, outcome).
fn dialog_lines(m: &MissionSystem) -> Vec<(String, String, String)> {
    m.drain()
        .into_iter()
        .filter(|e| e.kind == EffectKind::Dialog)
        .map(|e| {
            let outcome = e
                .detail
                .split(';')
                .find_map(|kv| kv.strip_prefix("outcome="))
                .unwrap_or_default()
                .to_string();
            (e.a, e.c, outcome)
        })
        .collect()
}

/// One dialog serves the tracker and the bridge (decision 2026-11-15-34, addendum): the director's component
/// `TriggerEvent` (the bridge) runs on the tracker's own `Rc`, so the line it starts is the tracker's `Dialog` effect,
/// and the talker it registered (the director's name tag) is the one the tracker's dialog behavior then finds. Before
/// the bridge's event the same behavior has no talker. Run in `mode`.
fn a_bridge_dialog_event_is_seen_by_the_tracker(mode: ParityMode) {
    let mut env = Env::with_mode(false, mode);
    let (m, s) = (env.mission.clone(), env.script.clone());
    m.dialog().borrow_mut().set_test_line_length(1.0);
    m.accept(env.vm(), &DetSet::new()).unwrap();
    m.drain();
    // The tracker's dialog behavior first: Tag_Mission's one entry names DialogName_Toy, which nothing registered.
    m.custom_event(env.vm(), "Chat").unwrap();
    assert_eq!(
        dialog_lines(&m),
        [("Tag_Mission".to_string(), "DialogName_Toy".to_string(), "no talker".to_string())]
    );
    // The bridge: the director's component raises Tag_Use; the director's body class names DialogName_Toy.
    let director = new_object(env.vm(), "WillowGame.WillowAIPawn").unwrap();
    let body = env.object("Body_Toy");
    set(&mut env.vm().space, director, "BodyClass", Value::Object(Some(body))).unwrap();
    let component = s.inner.o.globals;
    {
        let mut st = s.inner.state.borrow_mut();
        st.director = Some(director);
        st.dialog_component = Some(component);
    }
    let tag_use = Value::Object(Some(env.object("Tag_Use")));
    let got = env.call(
        "GearboxFramework.GearboxDialogComponent.TriggerEvent",
        Some(component),
        vec![tag_use, Value::Object(None), Value::Object(None)],
        MissionScript::n_trigger_event,
    );
    assert_eq!(got.unwrap(), Value::None);
    assert_eq!(
        dialog_lines(&m),
        [("Tag_Use".to_string(), "DialogName_Toy".to_string(), "started".to_string())],
        "the bridge's line is the tracker's effect"
    );
    assert_eq!(s.inner.state.borrow().use_.on_use_tag, "Tag_Use");
    assert_eq!(s.inner.state.borrow().use_.on_use_group_count, 1);
    // The tracker's dialog behavior again: its handler finds the talker the bridge registered, and the bridge's line,
    // still live in the same group at the same priority, holds this one back.
    m.custom_event(env.vm(), "Chat").unwrap();
    assert_eq!(
        dialog_lines(&m),
        [("Tag_Mission".to_string(), "DialogName_Toy".to_string(), "blocked by priority".to_string())]
    );
    // Once the bridge's line has ended on the tracker's clock (the kernel's time hook), the behavior's line starts.
    m.tick(env.vm(), 2.0).unwrap();
    m.custom_event(env.vm(), "Chat").unwrap();
    assert_eq!(
        dialog_lines(&m),
        [("Tag_Mission".to_string(), "DialogName_Toy".to_string(), "started".to_string())]
    );
    assert!(m.errors().is_empty(), "{:?}", m.errors());
    assert!(Rc::ptr_eq(&m.dialog(), &env.mission.dialog()));
}

#[test]
fn a_bridge_dialog_event_is_seen_by_the_tracker_in_reference_mode() {
    a_bridge_dialog_event_is_seen_by_the_tracker(ParityMode::Reference);
}

#[test]
fn a_bridge_dialog_event_is_seen_by_the_tracker_in_off_mode() {
    a_bridge_dialog_event_is_seen_by_the_tracker(ParityMode::Off);
}

/// The director (lane FIX-DIRECTOR-STORE, decision 2026-11-16-48): each placement makes a new object from his
/// archetype (the C++ `placeMarcus`, `instantiateExport` at 4), not the definition store's object for the path. His
/// references are the store's objects: his body class is the store's `Body_Toy`, which the dialog loads, so his
/// component's event speaks under its name tag; his directive table, his AI class (at 4) and his dialog component
/// (at 8) are new objects made from their exports (the C++ `loadRef` and `instantiateExport`), with their tags. A
/// reference that names no export (an import no package holds) is left as it is (the C++ `resourceIndex > 0`).
fn the_director_is_a_new_object_with_the_store_s_references(mode: ParityMode) {
    let mut env = Env::with_mode(false, mode);
    let (m, s) = (env.mission.clone(), env.script.clone());
    m.dialog().borrow_mut().set_test_line_length(1.0);
    m.accept(env.vm(), &DetSet::new()).unwrap();
    m.drain();
    // (The toy has no experience curve, so the accept's ExpEarn already left an error; only later ones count here.)
    let earlier = s.errors().len();
    let name = PackageName::parse("TestDirector").unwrap();
    assert!(s.place_marcus(env.vm(), &name, MARCUS_PAWN).unwrap());
    let director = s.director().expect("a director");
    let body = env.object("Body_Toy");
    let vm = env.vm();
    let find = |vm: &mut Vm, path: &str| vm.defs.find(&mut vm.space, &ObjectPath::parse(path).unwrap()).unwrap().unwrap();
    let archetype = find(vm, MARCUS_PAWN);
    assert_ne!(director, archetype, "{mode:?}: a new object, not the store's");
    assert_eq!(prop(&vm.space, director, "BodyClass").unwrap(), Some(Value::Object(Some(body))));
    let Value::Object(Some(directives)) = get(&vm.space, director, "MissionDirectives").unwrap() else {
        panic!("no directive table");
    };
    assert_ne!(directives, find(vm, "GD_Marcus.Character.Directives_Toy"));
    assert_eq!(get(&vm.space, directives, "PlayerInteractionDistance").unwrap(), Value::Float(7.0));
    let Value::Object(Some(ai_class)) = get(&vm.space, director, "AIClass").unwrap() else {
        panic!("no AI class");
    };
    assert_ne!(ai_class, find(vm, "GD_Marcus.Character.AIClass_Toy"));
    assert_eq!(get(&vm.space, ai_class, "ToyLevel").unwrap(), Value::Int(3));
    let component = s.inner.state.borrow().dialog_component.expect("his dialog component");
    assert_eq!(get(&vm.space, director, "DialogComponent").unwrap(), Value::Object(Some(component)));
    assert_ne!(component, find(vm, "GD_Marcus.Character.Pawn_Marcus.Dialog_Toy"));
    // Its tags start 8 bytes in: read from 4 the stream would not reach this value.
    assert_eq!(get(&vm.space, component, "ToyVolume").unwrap(), Value::Float(0.5));
    // His component's event: the body class is read through the store (no stand-in, no error).
    let component = s.inner.o.globals;
    s.inner.state.borrow_mut().dialog_component = Some(component);
    let tag_use = Value::Object(Some(env.object("Tag_Use")));
    let got = env.call(
        "GearboxFramework.GearboxDialogComponent.TriggerEvent",
        Some(component),
        vec![tag_use, Value::Object(None), Value::Object(None)],
        MissionScript::n_trigger_event,
    );
    assert_eq!(got.unwrap(), Value::None);
    assert_eq!(
        dialog_lines(&m),
        [("Tag_Use".to_string(), "DialogName_Toy".to_string(), "started".to_string())]
    );
    // A second placement makes another new object.
    assert!(s.place_marcus(env.vm(), &name, MARCUS_PAWN).unwrap());
    assert_ne!(s.director(), Some(director));
    // An AI class and a component that name no export are left as the archetype's references: no object is made.
    assert!(s.place_marcus(env.vm(), &name, "GD_Marcus.Character.Pawn_Imported").unwrap());
    let imported = s.director().expect("a director");
    let vm = env.vm();
    for property in ["AIClass", "DialogComponent"] {
        let Value::Object(Some(o)) = get(&vm.space, imported, property).unwrap() else {
            panic!("{property}: no reference");
        };
        assert!(
            matches!(vm.space.heap.get(o).unwrap().origin, Origin::Resource { index, .. } if index < 0),
            "{mode:?}: {property} is the import's stand-in"
        );
    }
    assert!(m.errors().is_empty(), "{:?}", m.errors());
    assert_eq!(s.errors().len(), earlier, "{:?}", s.errors());
}

#[test]
fn the_director_is_a_new_object_with_the_store_s_references_in_reference_mode() {
    the_director_is_a_new_object_with_the_store_s_references(ParityMode::Reference);
}

#[test]
fn the_director_is_a_new_object_with_the_store_s_references_in_off_mode() {
    the_director_is_a_new_object_with_the_store_s_references(ParityMode::Off);
}
