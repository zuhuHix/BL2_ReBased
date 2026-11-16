//! Owner: lane W6-FIRE. The slice rules on invented packages (AGENTS.md rule 9). AI-assisted (Claude Code).
//!
//! The script world is the bridge tests' (`bridge/tests.rs`, copied: the classes the bridge builds its graph from and
//! short script bodies for accept and turn-in), plus the slice's vocabulary: the behavior provider structures, the
//! enable-condition class, the enums the slice reads and a few behavior classes. `Startup.upk` holds `ToyMission` (one
//! set `SetA` {`RockPaper_GoToRange`, count 1} that completes the mission, and a provider whose `SetA` activation asks
//! for two sequence changes), `Other` (a second mission) and the globals. `Sanctuary_Dynamic.upk` holds the dummy's
//! provider at the stock path, with sequences whose conditions are invented, and, when asked, the dummy's pawn
//! archetype. Every expected value is written by hand from the rule. The C++ comparison on `tests/mission_script_test.py`'s
//! toys is local (decision 2026-11-16-38): no package may be tracked.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{DetSet, Missing, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::PackageBuilder;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::Export;
use ow_object::ids::PropAddr;
use ow_vm::api::{NativePack, Vm};
use ow_world::world::World;

use super::*;
use crate::tracker::Status;

static NEXT: AtomicU32 = AtomicU32::new(0);

const KINDS: [&str; 10] = ["Byte", "Object", "Bool", "Float", "Array", "Struct", "Name", "Int", "Str", "Class"];
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
    let context = toy_struct(&mut t, base, "BehaviorContextData", None);
    t.prop("Name", context, "InstancedDataContextName", 0, 0);
    t.prop("Object", context, "ContextObject", 0, object);
    t.prop("Byte", context, "BehaviorContext", 0, 0);
    t.prop("Struct", base, "ToyContextData", 0, context);
    // The slice's vocabulary (invented values; the names the slice reads).
    enum_export(&mut t, base, "EBehaviorContext", &["CONTEXT_Self", "CONTEXT_Other"]);
    let targetable = t.class("ITargetable", object);
    enum_export(&mut t, targetable, "EChangeStatus", &["CHANGE_Toggle", "CHANGE_Enable", "CHANGE_Disable"]);
    let remote = t.class("Behavior_RemoteEvent", object);
    t.prop("Name", remote, "EventName", 0, 0);
    // The health pool stand-in's vocabulary: the pool, its manager, the pawn's reference to it, and the curve's
    // definition class (every evaluation step present and off).
    let pool = t.class("ResourcePool", object);
    for name in ["Definition", "AssociatedProvider"] {
        t.prop("Object", pool, name, 0, object);
    }
    for name in ["bIsAuthoritative", "bCreatedAndNotModified"] {
        t.prop("Bool", pool, name, 0, 0);
    }
    for name in [
        "MaxValueBaseValue", "MaxValue", "MinValueBaseValue", "MinValue", "CurrentValue", "LastCurrentValue",
        "LastMinValue", "LastMaxValue",
    ] {
        t.prop("Float", pool, name, 0, 0);
    }
    t.prop("Byte", pool, "PoolGUID", 0, 0);
    let manager = t.class("ResourcePoolManager", object);
    toy_array_of(&mut t, manager, "ResourcePools", "Object", object);
    let reference = toy_struct(&mut t, actor, "ResourcePoolReference", None);
    t.prop("Object", reference, "PoolManager", 0, object);
    t.prop("Byte", reference, "PoolIndexInManager", 0, 0);
    t.prop("Byte", reference, "PoolGUID", 0, 0);
    t.prop("Object", reference, "Data", 0, object);
    t.prop("Struct", pawn, "HealthPool", 0, reference);
    let definition = t.class("AttributeInitializationDefinition", object);
    for (name, flags) in [
        ("ValueFormula", &["bEnabled"][..]),
        ("ConditionalInitialization", &["bEnabled"][..]),
        ("RandomVariance", &["bEnabled"][..]),
        ("RangeRestriction", &["bEnableMinValueRestriction", "bEnableMaxValueRestriction"][..]),
    ] {
        let st = toy_struct(&mut t, definition, &format!("Toy{name}"), None);
        for flag in flags {
            t.prop("Bool", st, flag, 0, 0);
        }
        t.prop("Struct", definition, name, 0, st);
    }
    t.prop("Byte", definition, "BaseValueMode", 0, 0);
    t.prop("Byte", definition, "RoundingMode", 0, 0);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let object = object_import(&mut t.p);
    let globals = t.class("GearboxGlobals", object);
    native(&mut t, globals, "GetGearboxGlobals", &[], Some(("Object", object)), NATIVE | STATIC);
    native(&mut t, globals, "GetBehaviorKernel", &[], Some(("Object", object)), NATIVE);
    let bpd = t.class("BehaviorProviderDefinition", object);
    let path = toy_struct(&mut t, 0, "NameBasedObjectPath", None);
    t.prop("Name", path, "PathComponentNames", 0, 0);
    let provider = t.class("ToyPathHolder", object);
    t.prop("Struct", provider, "ToyPath", 0, path);
    // The provider vocabulary (the shape of tracker/tests.rs's, over this toy's helpers).
    let var_types = enum_export(&mut t, bpd, "EBehaviorVariableType", &["BVAR_None", "BVAR_Object", "BVAR_Int"]);
    let link_types = enum_export(&mut t, bpd, "EBehaviorVariableLinkType", &["BVARLINK_Unknown", "BVARLINK_Input", "BVARLINK_Output"]);
    let sub = toy_struct(&mut t, 0, "SubarrayData", None);
    t.prop("Int", sub, "ArrayIndexAndLength", 0, 0);
    let mut defaults = t.p.bool_tag("bEnabled", true);
    defaults.extend_from_slice(&t.none);
    let user = toy_struct(&mut t, 0, "BehaviorEventUserData", Some(&defaults));
    t.prop("Name", user, "EventName", 0, 0);
    t.prop("Bool", user, "bEnabled", 0, 0);
    t.prop("Int", user, "MaxTriggerCount", 0, 0);
    t.prop("Float", user, "ReTriggerDelay", 0, 0);
    let event = toy_struct(&mut t, 0, "BehaviorEventData2", None);
    t.prop("Struct", event, "UserData", 0, user);
    t.prop("Struct", event, "OutputVariables", 0, sub);
    t.prop("Struct", event, "OutputLinks", 0, sub);
    let data = toy_struct(&mut t, 0, "BehaviorData2", None);
    t.prop("Object", data, "Behavior", 0, object);
    t.prop("Struct", data, "LinkedVariables", 0, sub);
    t.prop("Struct", data, "OutputLinks", 0, sub);
    let link = toy_struct(&mut t, 0, "BehaviorOutputLinkData", None);
    t.prop("Int", link, "LinkIdAndLinkedBehavior", 0, 0);
    t.prop("Float", link, "ActivateDelay", 0, 0);
    let var = toy_struct(&mut t, 0, "BehaviorVariableData", None);
    t.prop("Name", var, "Name", 0, 0);
    t.prop("Byte", var, "Type", 0, var_types);
    let vlink = toy_struct(&mut t, 0, "BehaviorVariableLinkData2", None);
    t.prop("Name", vlink, "PropertyName", 0, 0);
    t.prop("Byte", vlink, "VariableLinkType", 0, link_types);
    t.prop("Byte", vlink, "ConnectionIndex", 0, 0);
    t.prop("Struct", vlink, "LinkedVariables", 0, sub);
    let seq = toy_struct(&mut t, 0, "BehaviorSequenceData", None);
    t.prop("Name", seq, "BehaviorSequenceName", 0, 0);
    t.prop("Bool", seq, "bEnabledOnSpawn", 0, 0);
    t.prop("Bool", seq, "bSequenceEnabledMutex", 0, 0);
    t.prop("Object", seq, "CustomEnableCondition", 0, object);
    toy_array_of(&mut t, seq, "EventData2", "Struct", event);
    toy_array_of(&mut t, seq, "BehaviorData2", "Struct", data);
    toy_array_of(&mut t, seq, "VariableData", "Struct", var);
    toy_array_of(&mut t, seq, "ConsolidatedOutputLinkData", "Struct", link);
    toy_array_of(&mut t, seq, "ConsolidatedVariableLinkData", "Struct", vlink);
    toy_array_of(&mut t, seq, "ConsolidatedLinkedVariables", "Int", 0);
    toy_array_of(&mut t, bpd, "BehaviorSequences", "Struct", seq);
    let change = t.class("Behavior_ChangeRemoteBehaviorSequenceState", object);
    t.prop("Struct", change, "ProviderDefinitionPathName", 0, path);
    t.prop("Name", change, "SequenceName", 0, 0);
    t.prop("Byte", change, "Action", 0, 0);
    t.class("Behavior_AIHold", object);
    t.class("Behavior_SpecialMove", object);
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
    // The slice's vocabulary.
    t.prop("Object", mission, "BehaviorProvider", 0, object);
    let balance = t.class("AIPawnBalanceDefinition", object);
    enum_export(&mut t, balance, "EAITransformed", &["TRANSFORM_None"]);
    let math = t.class("Behavior_SimpleMath", object);
    enum_export(&mut t, math, "EBinaryMathOperation", &["OPERATION_Add"]);
    let condition = t.class("BehaviorSequenceEnableByMission", object);
    t.prop("Object", condition, "LinkedMission", 0, object);
    t.prop("Bool", condition, "bIsObjectiveSpecific", 0, 0);
    t.prop("Object", condition, "LinkedObjective", 0, object);
    let mission_states = toy_struct(&mut t, 0, "MissionStatesToLinkTo", None);
    for bit in ["bNotStarted", "bActive", "bRequiredObjectivesComplete", "bReadyToTurnIn", "bComplete", "bFailed"] {
        t.prop("Bool", mission_states, bit, 0, 0);
    }
    t.prop("Struct", condition, "MissionStatesToLinkTo", 0, mission_states);
    let objective_states = toy_struct(&mut t, 0, "ObjectiveStatesToLinkTo", None);
    for bit in ["bNotStarted", "bActive", "bComplete"] {
        t.prop("Bool", objective_states, bit, 0, 0);
    }
    t.prop("Struct", condition, "ObjectiveStatesToLinkTo", 0, objective_states);
    toy_array_of(&mut t, condition, "ObjectiveSetRestrictions", "Object", object);
    t.class("ToyOtherEnableCondition", object);
    // The player's class and its health pool data (the stand-in's inputs).
    let resource = t.class("ResourceDefinition", object);
    let mut class_tail = object.to_le_bytes().to_vec();
    class_tail.extend(object.to_le_bytes());
    t.prop_tail("Class", resource, "DefaultResourcePoolClass", 0, &class_tail);
    let pool_definition = t.class("ResourcePoolDefinition", object);
    t.prop("Object", pool_definition, "Resource", 0, object);
    t.prop("Struct", pool_definition, "BaseMaxValue", 0, aid);
    t.prop("Float", pool_definition, "BaseMinValue", 0, 0);
    let player_class = t.class("PlayerClassDefinition", object);
    t.prop("Object", player_class, "HealthPoolDefinition", 0, object);
    let status = t.class("Behavior_AttemptStatusEffect", object);
    t.prop("Object", status, "StatusEffect", 0, object);
    let willow_pawn = t.class("WillowPawn", engine_pawn);
    t.class("WillowPlayerPawn", engine_pawn);
    t.class("WillowAIPawn", willow_pawn);
    t.build()
}

// ---------------------------------------------------------------------------------------------------------------
// The mission package and the map package
// ---------------------------------------------------------------------------------------------------------------

fn packed(first: usize, length: usize) -> i32 {
    // The fixtures stay far below 2^15 entries.
    ((first << 16) | length) as i32
}

fn class_import(p: &mut PackageBuilder, package: &str, name: &str) -> i32 {
    let outer = p.add_import("Core", "Package", 0, package);
    p.add_import("Core", "Class", outer, name)
}

/// One sequence of a provider: its flags, its condition (an export of the package) and each event's behaviors (exports
/// of the package, each linked from the event with the link id [`Seq::link_id`], in order).
struct Seq {
    name: &'static str,
    on_spawn: bool,
    mutex: bool,
    condition: Option<i32>,
    events: Vec<(&'static str, Vec<i32>)>,
    /// The id of every link (the tracker fires a set's activation with 4; the dummy's events take every id).
    link_id: i32,
}

fn seq(name: &'static str, condition: Option<i32>, events: Vec<(&'static str, Vec<i32>)>) -> Seq {
    Seq {
        name,
        on_spawn: false,
        mutex: false,
        condition,
        events,
        link_id: 0,
    }
}

/// The `BehaviorSequences` tag of a provider.
fn sequences_tag(p: &mut PackageBuilder, sequences: &[Seq]) -> Vec<u8> {
    let none = p.none();
    let sub = |p: &mut PackageBuilder, name: &str, first: usize, length: usize| {
        let mut body = p.int_tag("ArrayIndexAndLength", packed(first, length));
        body.extend_from_slice(&none);
        p.struct_tag(name, "SubarrayData", &body)
    };
    let mut all = Vec::new();
    for s in sequences {
        let mut behaviors: Vec<i32> = Vec::new();
        let mut links: Vec<usize> = Vec::new();
        let mut events = Vec::new();
        for (event, targets) in &s.events {
            let mut user = p.name_tag("EventName", event);
            user.extend_from_slice(&none);
            events.extend(p.struct_tag("UserData", "BehaviorEventUserData", &user));
            events.extend(sub(p, "OutputVariables", 0, 0));
            events.extend(sub(p, "OutputLinks", links.len(), targets.len()));
            events.extend_from_slice(&none);
            for &b in targets {
                let index = match behaviors.iter().position(|&x| x == b) {
                    Some(i) => i,
                    None => {
                        behaviors.push(b);
                        behaviors.len() - 1
                    }
                };
                links.push(index);
            }
        }
        let mut data = Vec::new();
        for &b in &behaviors {
            data.extend(p.object_tag("Behavior", b));
            data.extend(sub(p, "LinkedVariables", 0, 0));
            data.extend(sub(p, "OutputLinks", 0, 0));
            data.extend_from_slice(&none);
        }
        let mut link_data = Vec::new();
        for &target in &links {
            link_data.extend(p.int_tag("LinkIdAndLinkedBehavior", target as i32 | (s.link_id << 24)));
            link_data.extend(p.float_tag("ActivateDelay", 0.0));
            link_data.extend_from_slice(&none);
        }
        let mut body = p.name_tag("BehaviorSequenceName", s.name);
        body.extend(p.bool_tag("bEnabledOnSpawn", s.on_spawn));
        body.extend(p.bool_tag("bSequenceEnabledMutex", s.mutex));
        if let Some(c) = s.condition {
            body.extend(p.object_tag("CustomEnableCondition", c));
        }
        body.extend(p.array_tag("EventData2", s.events.len() as i32, &events));
        body.extend(p.array_tag("BehaviorData2", behaviors.len() as i32, &data));
        body.extend(p.array_tag("ConsolidatedOutputLinkData", links.len() as i32, &link_data));
        body.extend_from_slice(&none);
        all.extend(body);
    }
    p.array_tag("BehaviorSequences", sequences.len() as i32, &all)
}

/// A `Behavior_ChangeRemoteBehaviorSequenceState` export of `outer` naming provider `leaf`, `sequence` and `action`.
fn change_behavior(p: &mut PackageBuilder, outer: i32, name: &str, leaf: &str, sequence: &str, action: u8) -> i32 {
    let class = class_import(p, "GearboxFramework", "Behavior_ChangeRemoteBehaviorSequenceState");
    let none = p.none();
    let mut path = p.name_tag("PathComponentNames", leaf);
    path.extend_from_slice(&none);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.struct_tag("ProviderDefinitionPathName", "NameBasedObjectPath", &path));
    payload.extend(p.name_tag("SequenceName", sequence));
    payload.extend(p.byte_tag("Action", action));
    payload.extend_from_slice(&none);
    let r = p.add_export(class, name, payload);
    p.set_outer(r, outer);
    r
}

/// `Startup`: `ToyMission` (set `SetA` {`RockPaper_GoToRange`} that completes it; its provider's `SetA` activation
/// enables the dummy's `MissionAsked` and asks the same of a provider `Elsewhere`), `Other`, and the globals. With
/// `health`, also the player's class at the stock path with a health pool definition (base maximum 100, invented),
/// its resource naming `Engine.ResourcePool`, and an experience curve whose every step is off (R(n) = 0).
fn startup(health: bool) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let none = p.none();
    let c = class_import(&mut p, "WillowGame", "MissionDefinition");
    let mission = p.add_export(c, "ToyMission", Vec::new());
    let c = class_import(&mut p, "WillowGame", "MissionObjectiveDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.int_tag("ObjectiveCount", 1));
    payload.extend_from_slice(&none);
    let objective = p.add_export(c, GO_TO_RANGE, payload);
    p.set_outer(objective, mission);
    let c = class_import(&mut p, "WillowGame", "MissionObjectiveSetDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.array_tag("ObjectiveDefinitions", 1, &objective.to_le_bytes()));
    payload.extend(p.bool_tag("bCanCompleteMission", true));
    payload.extend_from_slice(&none);
    let set = p.add_export(c, "SetA", payload);
    p.set_outer(set, mission);
    let c = class_import(&mut p, "GearboxFramework", "BehaviorProviderDefinition");
    let provider = p.add_export(c, "ToyMissionBpd", Vec::new());
    p.set_outer(provider, mission);
    let enable = change_behavior(&mut p, provider, "AskDummy", "BehaviorProviderDefinition_5", "MissionAsked", 1);
    let elsewhere = change_behavior(&mut p, provider, "AskElsewhere", "Elsewhere", "Any", 1);
    let mut events = seq("Main", None, vec![("SetA", vec![enable, elsewhere])]);
    events.on_spawn = true;
    events.link_id = 4;
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(sequences_tag(&mut p, &[events]));
    payload.extend_from_slice(&none);
    p.set_payload(provider, payload);
    let mut tags = 0i32.to_le_bytes().to_vec();
    tags.extend(p.array_tag("ObjectiveSetDefs", 1, &set.to_le_bytes()));
    tags.extend(p.object_tag("InitialObjectiveSet", set));
    tags.extend(p.bool_tag("bActivateInitialObjectiveSet", true));
    tags.extend(p.object_tag("BehaviorProvider", provider));
    let mut credits = p.float_tag("BaseValueConstant", 0.0);
    credits.extend(p.float_tag("BaseValueScaleConstant", 1.0));
    credits.extend_from_slice(&none);
    let mut reward = p.byte_tag("CurrencyRewardType", 0);
    reward.extend(p.struct_tag("CreditRewardMultiplier", "AttributeInitializationData", &credits));
    reward.extend_from_slice(&none);
    tags.extend(p.struct_tag("Reward", "MissionRewardData", &reward));
    tags.extend_from_slice(&none);
    p.set_payload(mission, tags);
    let c = class_import(&mut p, "WillowGame", "MissionDefinition");
    let mut other = 0i32.to_le_bytes().to_vec();
    other.extend_from_slice(&none);
    p.add_export(c, "Other", other);
    let package_class = p.add_import("Core", "Class", 0, "Package");
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&none);
    let gd = p.add_export(package_class, "GD_Globals", empty.clone());
    let general = p.add_export(package_class, "General", empty);
    p.set_outer(general, gd);
    let c = class_import(&mut p, "WillowGame", "GlobalsDefinition");
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(p.float_tag("PlayerInteractionDistance", 420.0));
    payload.extend_from_slice(&none);
    let globals = p.add_export(c, "Globals", payload);
    p.set_outer(globals, general);
    if health {
        let mut empty = 0i32.to_le_bytes().to_vec();
        empty.extend_from_slice(&none);
        let chain = |p: &mut PackageBuilder, names: &[&str]| -> i32 {
            let mut outer = 0;
            for n in names {
                let r = p.add_export(package_class, n, empty.clone());
                if outer != 0 {
                    p.set_outer(r, outer);
                }
                outer = r;
            }
            outer
        };
        let formulas = chain(&mut p, &["GD_Balance_Experience", "Formulas"]);
        let c = class_import(&mut p, "Engine", "AttributeInitializationDefinition");
        let curve = p.add_export(c, "Init_ExperienceRequiredForLevel", empty.clone());
        p.set_outer(curve, formulas);
        let character = chain(&mut p, &["GD_Siren", "Character"]);
        let pool_class = class_import(&mut p, "Engine", "ResourcePool");
        let c = class_import(&mut p, "WillowGame", "ResourceDefinition");
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(p.tag("DefaultResourcePoolClass", "ClassProperty", &pool_class.to_le_bytes()));
        payload.extend_from_slice(&none);
        let resource = p.add_export(c, "Health", payload);
        p.set_outer(resource, character);
        let c = class_import(&mut p, "WillowGame", "ResourcePoolDefinition");
        let mut base = p.float_tag("BaseValueConstant", 100.0);
        base.extend(p.float_tag("BaseValueScaleConstant", 1.0));
        base.extend_from_slice(&none);
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(p.object_tag("Resource", resource));
        payload.extend(p.struct_tag("BaseMaxValue", "AttributeInitializationData", &base));
        payload.extend_from_slice(&none);
        let pool = p.add_export(c, "HealthPool", payload);
        p.set_outer(pool, character);
        let c = class_import(&mut p, "WillowGame", "PlayerClassDefinition");
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(p.object_tag("HealthPoolDefinition", pool));
        payload.extend_from_slice(&none);
        let class = p.add_export(c, "CharClass_Siren", payload);
        p.set_outer(class, character);
    }
    p.build()
}

/// What the map package holds besides the conditions every fixture has.
#[derive(Clone, Copy, Default)]
struct Map {
    /// The dummy's pawn archetype at the stock path.
    pawn: bool,
    /// An OnTakeDamage `Behavior_AIHold` on the dummy.
    hold: bool,
    /// An OnSpawned condition of another class on sequence `Odd`.
    odd_condition: bool,
    /// The player's class and its health pool data in `Startup` ([`startup`]).
    health: bool,
}

/// `Sanctuary_Dynamic`: the dummy's provider at the stock path. Sequences (each enabled / disabled event raises the
/// remote event `<name>On` / `<name>Off`):
/// - `Idle` (enabled on spawn; `OnSpawned` raises `Spawned`, toggles `Toggled`, sends an unknown action to `Idle`
///   and a change to provider `Stranger`; `OnTakeDamage` raises `Hit`, an attempted status effect and, with
///   [`Map::hold`], an AI hold);
/// - `ObjSeq` (the objective {Active}), `RestrictA` / `RestrictB` (the same, restricted to `SetA` / a set that is never
///   active), `MissionLevel` ({Complete}), `MutexA` ({Active, ReadyToTurnIn}) and `MutexB` ({ReadyToTurnIn}), one
///   mutex group;
/// - `OtherDone` (mission `Other` {Complete}), `NoObjective` (objective-specific, no objective), `Outside` (an
///   objective of `Other`), `Toggled` and `MissionAsked` (no condition), and with [`Map::odd_condition`] `Odd`.
fn dynamic(map: Map) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let none = p.none();
    let package_class = p.add_import("Core", "Class", 0, "Package");
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&none);
    let gd = p.add_export(package_class, "GD_TargetDummy", empty.clone());
    let character = p.add_export(package_class, "Character", empty.clone());
    p.set_outer(character, gd);
    let class = p.add_export(package_class, "CharClass_TargetDummy", empty.clone());
    p.set_outer(class, character);
    let c = class_import(&mut p, "GearboxFramework", "BehaviorProviderDefinition");
    let provider = p.add_export(c, "BehaviorProviderDefinition_5", Vec::new());
    p.set_outer(provider, class);
    if map.pawn {
        let c = class_import(&mut p, "Engine", "Pawn");
        let pawn = p.add_export(c, "Pawn_TargetDummy", empty);
        p.set_outer(pawn, character);
    }
    // The mission package's objects, by import.
    let startup = p.add_import("Core", "Package", 0, "Startup");
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let _ = p.add_import("Core", "Class", willow, "MissionDefinition");
    let mission = p.add_import("WillowGame", "MissionDefinition", startup, "ToyMission");
    let objective = p.add_import("WillowGame", "MissionObjectiveDefinition", mission, GO_TO_RANGE);
    let set_a = p.add_import("WillowGame", "MissionObjectiveSetDefinition", mission, "SetA");
    let other = p.add_import("WillowGame", "MissionDefinition", startup, "Other");
    let other_objective = p.add_import("WillowGame", "MissionObjectiveDefinition", other, "Nothing");
    let never = p.add_import("WillowGame", "MissionObjectiveSetDefinition", mission, "NeverSet");

    let remote = class_import(&mut p, "Engine", "Behavior_RemoteEvent");
    let behavior = |p: &mut PackageBuilder, class: i32, name: &str, tags: Vec<u8>| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(tags);
        payload.extend_from_slice(&none);
        let r = p.add_export(class, name, payload);
        p.set_outer(r, provider);
        r
    };
    let on_off = |p: &mut PackageBuilder, name: &'static str, condition: Option<i32>| -> Seq {
        let on = format!("{name}On");
        let off = format!("{name}Off");
        let t = p.name_tag("EventName", &on);
        let on_ref = behavior(p, remote, &format!("{name}_{on}"), t);
        let t = p.name_tag("EventName", &off);
        let off_ref = behavior(p, remote, &format!("{name}_{off}"), t);
        seq(
            name,
            condition,
            vec![("OnBehaviorSequenceEnabled", vec![on_ref]), ("OnBehaviorSequenceDisabled", vec![off_ref])],
        )
    };
    let condition_class = class_import(&mut p, "WillowGame", "BehaviorSequenceEnableByMission");
    let condition = |p: &mut PackageBuilder,
                     name: &str,
                     linked: i32,
                     objective: Option<i32>,
                     mission_bits: &[&str],
                     objective_bits: &[&str],
                     restrictions: &[i32]| {
        let none = p.none();
        let mut body = p.object_tag("LinkedMission", linked);
        body.extend(p.bool_tag("bIsObjectiveSpecific", objective.is_some() || !objective_bits.is_empty()));
        if !mission_bits.is_empty() {
            let mut bits = Vec::new();
            for b in mission_bits {
                bits.extend(p.bool_tag(b, true));
            }
            bits.extend_from_slice(&none);
            body.extend(p.struct_tag("MissionStatesToLinkTo", "MissionStatesToLinkTo", &bits));
        }
        if let Some(o) = objective {
            body.extend(p.object_tag("LinkedObjective", o));
        }
        if !objective_bits.is_empty() {
            let mut bits = Vec::new();
            for b in objective_bits {
                bits.extend(p.bool_tag(b, true));
            }
            bits.extend_from_slice(&none);
            body.extend(p.struct_tag("ObjectiveStatesToLinkTo", "ObjectiveStatesToLinkTo", &bits));
        }
        if !restrictions.is_empty() {
            let refs: Vec<u8> = restrictions.iter().flat_map(|r| r.to_le_bytes()).collect();
            body.extend(p.array_tag("ObjectiveSetRestrictions", restrictions.len() as i32, &refs));
        }
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(body);
        payload.extend_from_slice(&none);
        let r = p.add_export(condition_class, name, payload);
        p.set_outer(r, provider);
        r
    };

    let mut sequences = Vec::new();
    // Idle and its OnSpawned / OnTakeDamage behaviors.
    let mut idle = on_off(&mut p, "Idle", None);
    idle.on_spawn = true;
    let t = p.name_tag("EventName", "Spawned");
    let spawned = behavior(&mut p, remote, "Idle_Spawned", t);
    let toggle = change_behavior(&mut p, provider, "Idle_Toggle", "BehaviorProviderDefinition_5", "Toggled", 0);
    let unknown = change_behavior(&mut p, provider, "Idle_Unknown", "BehaviorProviderDefinition_5", "Idle", 7);
    let stranger = change_behavior(&mut p, provider, "Idle_Stranger", "Stranger", "Idle", 1);
    idle.events.push(("OnSpawned", vec![spawned, toggle, unknown, stranger]));
    let t = p.name_tag("EventName", "Hit");
    let hit = behavior(&mut p, remote, "Idle_Hit", t);
    let status_class = class_import(&mut p, "WillowGame", "Behavior_AttemptStatusEffect");
    let t = p.object_tag("StatusEffect", other);
    let status = behavior(&mut p, status_class, "Idle_Burn", t);
    let mut on_hit = vec![hit, status];
    if map.hold {
        let hold_class = class_import(&mut p, "GearboxFramework", "Behavior_AIHold");
        on_hit.push(behavior(&mut p, hold_class, "Idle_Hold", Vec::new()));
    }
    idle.events.push(("OnTakeDamage", on_hit));
    sequences.push(idle);
    let active = ["bActive"];
    let c = condition(&mut p, "ObjCondition", mission, Some(objective), &active, &active, &[]);
    sequences.push(on_off(&mut p, "ObjSeq", Some(c)));
    let c = condition(&mut p, "RestrictACondition", mission, Some(objective), &active, &active, &[set_a]);
    sequences.push(on_off(&mut p, "RestrictA", Some(c)));
    let c = condition(&mut p, "RestrictBCondition", mission, Some(objective), &active, &active, &[never]);
    sequences.push(on_off(&mut p, "RestrictB", Some(c)));
    let c = condition(&mut p, "MissionLevelCondition", mission, None, &["bComplete"], &[], &[]);
    sequences.push(on_off(&mut p, "MissionLevel", Some(c)));
    let c = condition(&mut p, "MutexACondition", mission, None, &["bActive", "bReadyToTurnIn"], &[], &[]);
    let mut mutex_a = on_off(&mut p, "MutexA", Some(c));
    mutex_a.mutex = true;
    sequences.push(mutex_a);
    let c = condition(&mut p, "MutexBCondition", mission, None, &["bReadyToTurnIn"], &[], &[]);
    let mut mutex_b = on_off(&mut p, "MutexB", Some(c));
    mutex_b.mutex = true;
    sequences.push(mutex_b);
    let c = condition(&mut p, "OtherDoneCondition", other, None, &["bComplete"], &[], &[]);
    sequences.push(on_off(&mut p, "OtherDone", Some(c)));
    let c = condition(&mut p, "NoObjectiveCondition", mission, None, &[], &["bNotStarted"], &[]);
    sequences.push(on_off(&mut p, "NoObjective", Some(c)));
    let c = condition(&mut p, "OutsideCondition", mission, Some(other_objective), &[], &["bNotStarted"], &[]);
    sequences.push(on_off(&mut p, "Outside", Some(c)));
    sequences.push(on_off(&mut p, "Toggled", None));
    sequences.push(on_off(&mut p, "MissionAsked", None));
    if map.odd_condition {
        let odd_class = class_import(&mut p, "WillowGame", "ToyOtherEnableCondition");
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend_from_slice(&none);
        let odd = p.add_export(odd_class, "OddCondition", payload);
        p.set_outer(odd, provider);
        sequences.push(on_off(&mut p, "Odd", Some(odd)));
    }
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend(sequences_tag(&mut p, &sequences));
    payload.extend_from_slice(&none);
    p.set_payload(provider, payload);
    p.build()
}

// ---------------------------------------------------------------------------------------------------------------
// The environment
// ---------------------------------------------------------------------------------------------------------------

struct Env {
    world: World,
    slice: FireMissionSlice,
    dir: PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A world over the invented install (the store in a fresh temporary folder, which the caller removes).
fn build(map: Map, mode: ParityMode) -> (World, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "ow-mission-slice-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let files: Vec<(String, Vec<u8>)> = vec![
        ("Core.upk".into(), core()),
        ("Engine.upk".into(), engine()),
        ("GearboxFramework.upk".into(), gearbox()),
        ("WillowGame.upk".into(), willowgame()),
        ("Startup.upk".into(), startup(map.health)),
        ("Sanctuary_Dynamic.upk".into(), dynamic(map)),
    ];
    let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    let defs = DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, mode).expect("synthetic store");
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::standard().unwrap()), mode);
    let mut world = bridge::world(vm).expect("world");
    ow_vm::natives_core::register(&mut world.vm).expect("core natives");
    ow_behavior::handlers::Pack.register(&mut world.vm).expect("world handlers");
    (world, dir)
}

impl Env {
    fn new(map: Map, mode: ParityMode) -> Env {
        let (mut world, dir) = build(map, mode);
        let map_name = PackageName::parse("Sanctuary_Dynamic").unwrap();
        let slice = FireMissionSlice::new(&mut world, "ToyMission", &map_name, DUMMY_PROVIDER).expect("slice");
        Env { world, slice, dir }
    }

    fn off() -> Env {
        Env::new(Map::default(), ParityMode::Off)
    }

    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }

    /// The remote events of the events since the last call.
    fn remotes(&mut self) -> Vec<String> {
        self.slice
            .drain()
            .into_iter()
            .filter(|e| e.kind == HostKind::RemoteEvent)
            .map(|e| e.a)
            .collect()
    }

    fn accept(&mut self, completed: &DetSet<String>) -> bool {
        let s = self.slice.clone();
        s.accept(self.vm(), completed).expect("accept")
    }

    fn spawn(&mut self) {
        let s = self.slice.clone();
        s.spawn_dummy(self.vm()).expect("spawn");
    }

    fn range(&mut self) -> bool {
        let s = self.slice.clone();
        s.enter_range(self.vm()).expect("range")
    }

    fn turn_in(&mut self) -> bool {
        let s = self.slice.clone();
        s.turn_in(self.vm()).expect("turn in")
    }
}

fn none_completed() -> DetSet<String> {
    DetSet::new()
}

// ---------------------------------------------------------------------------------------------------------------
// The rules
// ---------------------------------------------------------------------------------------------------------------

/// Before the spawn the provider observes nothing; the spawn registers it (the `bEnabledOnSpawn` sequences), the
/// conditions deliver their verdicts at once, and only then does `OnSpawned` fire.
#[test]
fn the_registration_comes_before_on_spawned() {
    let mut env = Env::off();
    assert!(env.accept(&none_completed()));
    // The mission's own change reaches the provider (no condition is observed yet).
    assert_eq!(env.remotes(), ["MissionAskedOn"]);
    env.spawn();
    // The registration disables every sequence without events, enables Idle, then each verdict in sequence order;
    // OnSpawned runs last.
    assert_eq!(env.remotes(), ["IdleOn", "ObjSeqOn", "RestrictAOn", "MutexAOn", "Spawned", "ToggledOn"]);
}

/// The objective's progress, the set and the status gate the verdicts: completing the objective turns its sequences
/// off and makes the mission ReadyToTurnIn (MutexB enables, disabling MutexA first); the turn-in makes the
/// mission-level condition hold and MutexB's {ReadyToTurnIn} no longer.
#[test]
fn the_verdicts_follow_the_tracker() {
    let mut env = Env::off();
    env.accept(&none_completed());
    env.spawn();
    env.remotes();
    assert!(env.range());
    assert_eq!(env.remotes(), ["ObjSeqOff", "RestrictAOff", "MutexAOff", "MutexBOn"]);
    assert_eq!(env.slice.mission().status(), Status::ReadyToTurnIn);
    assert!(env.turn_in());
    assert_eq!(env.remotes(), ["MissionLevelOn", "MutexBOff"]);
    let enabled = env.slice.dummy_enabled_sequences().unwrap();
    assert_eq!(enabled, ["Idle", "MissionLevel", "Toggled"]);
}

/// `ObjectiveSetRestrictions` keeps a true verdict only while a listed set is active (RestrictB's set never is).
#[test]
fn a_restriction_needs_a_listed_set_active() {
    let mut env = Env::off();
    env.accept(&none_completed());
    env.spawn();
    let enabled = env.slice.dummy_enabled_sequences().unwrap();
    assert!(enabled.contains(&"RestrictA".to_string()));
    assert!(!enabled.contains(&"RestrictB".to_string()));
}

/// A mission other than the slice's is Complete when the completed set names it, else NotStarted.
#[test]
fn another_mission_is_complete_from_the_completed_set() {
    let mut env = Env::off();
    let mut completed = DetSet::new();
    completed.insert("Other".to_string());
    env.accept(&completed);
    env.spawn();
    assert!(env.slice.dummy_enabled_sequences().unwrap().contains(&"OtherDone".to_string()));
    let mut env = Env::off();
    env.accept(&none_completed());
    env.spawn();
    assert!(!env.slice.dummy_enabled_sequences().unwrap().contains(&"OtherDone".to_string()));
}

/// An objective-specific condition without an objective is false; one naming an objective outside its mission is
/// false with an error line.
#[test]
fn objective_specific_conditions_need_the_mission_s_objective() {
    let mut env = Env::off();
    env.accept(&none_completed());
    env.spawn();
    let enabled = env.slice.dummy_enabled_sequences().unwrap();
    assert!(!enabled.contains(&"NoObjective".to_string()));
    assert!(!enabled.contains(&"Outside".to_string()));
    assert!(
        env.slice
            .errors()
            .contains(&"enable condition names an objective outside its mission: Startup.Other.Nothing".to_string()),
        "{:?}",
        env.slice.errors()
    );
}

/// A condition of another class is an error line, once however often the conditions are applied.
#[test]
fn an_unsupported_condition_class_is_reported_once() {
    let mut env = Env::new(
        Map {
            odd_condition: true,
            ..Map::default()
        },
        ParityMode::Off,
    );
    env.accept(&none_completed());
    env.spawn();
    env.range();
    let line = "unsupported enable condition class WillowGame.ToyOtherEnableCondition".to_string();
    assert_eq!(env.slice.errors().iter().filter(|l| **l == line).count(), 1);
}

/// The dummy's own sequence changes: Toggle flips (UNVERIFIED reading), an action outside the enum is an error line,
/// a change naming another provider is the provider's error line; each named change is a SequenceChange event.
#[test]
fn the_dummy_s_sequence_changes() {
    let mut env = Env::off();
    env.spawn();
    let events = env.slice.drain();
    let changes: Vec<(String, String)> = events
        .iter()
        .filter(|e| e.kind == HostKind::SequenceChange)
        .map(|e| (e.b.clone(), e.c.clone()))
        .collect();
    assert_eq!(
        changes,
        [("Toggled".to_string(), "CHANGE_Toggle".to_string()), ("Idle".to_string(), "?7".to_string())]
    );
    assert!(events.iter().any(|e| e.kind == HostKind::RemoteEvent && e.a == "ToggledOn"));
    let errors = env.slice.errors();
    assert!(errors.contains(&"unknown sequence change action '?7' for Idle".to_string()), "{errors:?}");
    assert!(errors.contains(&"sequence change targets another provider: Stranger".to_string()), "{errors:?}");
}

/// The mission's sequence changes go to the dummy when they name it, else they are an error line; the mission's remote
/// events carry the mission's path.
#[test]
fn mission_effects_are_routed() {
    let mut env = Env::off();
    env.spawn();
    env.slice.drain();
    env.accept(&none_completed());
    let events = env.slice.drain();
    assert!(
        events
            .iter()
            .any(|e| e.kind == HostKind::SequenceChange && e.b == "MissionAsked" && e.c == "CHANGE_Enable")
    );
    assert!(events.iter().any(|e| e.kind == HostKind::RemoteEvent && e.a == "MissionAskedOn"));
    assert!(
        env.slice
            .errors()
            .contains(&"mission sequence change targets an unbound provider: Elsewhere".to_string())
    );
    let kinds: Vec<HostKind> = events.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&HostKind::Status) && kinds.contains(&HostKind::ObjectiveSet), "{kinds:?}");
}

/// The range wrapper updates the objective directly when the map has no waypoint; true only when it changed.
#[test]
fn the_range_wrapper_without_a_waypoint() {
    let mut env = Env::off();
    assert!(!env.range(), "not updatable before the accept");
    env.accept(&none_completed());
    assert!(env.range());
    assert!(env.slice.drain().iter().any(|e| e.kind == HostKind::ObjectiveComplete && e.a == GO_TO_RANGE));
}

/// Without a damage the hit raises OnTakeDamage directly (its outputs the damage type and source); the status effect
/// behavior hands its definition to the host.
#[test]
fn a_hit_without_a_damage_raises_the_event() {
    let mut env = Env::off();
    env.spawn();
    env.slice.drain();
    let s = env.slice.clone();
    assert!(s.hit_dummy(env.vm(), true).unwrap());
    let events = env.slice.drain();
    assert!(events.iter().any(|e| e.kind == HostKind::RemoteEvent && e.a == "Hit"));
    assert!(events.iter().any(|e| e.kind == HostKind::StatusEffect && e.a == "Other"));
}

/// The player's hit through the stock pawn chain is not wired into the slice yet (W6-COMBAT-RUN2): `NotYet` with a
/// pawn and a damage, in both
/// modes; without a pawn the event is raised directly, as the C++.
#[test]
fn the_pawn_hit_chain_is_not_yet() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut env = Env::new(
            Map {
                pawn: true,
                ..Map::default()
            },
            mode,
        );
        env.spawn();
        assert!(env.slice.dummy_pawn().is_some(), "the archetype gives a pawn");
        let s = env.slice.clone();
        let e = s.damage_dummy(env.vm(), INCENDIARY_IMPACT, "", 10.0).unwrap_err();
        assert!(matches!(e.not_yet().map(|n| &n.missing), Some(Missing::NotPorted)), "{e:?}");
        assert!(!env.slice.not_yet().is_empty());
    }
    let mut env = Env::off();
    let s = env.slice.clone();
    assert!(env.slice.dummy_pawn().is_none());
    assert!(s.damage_dummy(env.vm(), "", "", 10.0).unwrap(), "no pawn: the event is raised directly");
}

/// `Behavior_AIHold` on the dummy before it has a mind: the C++ no-op in Reference mode, `NotYet` in Off mode.
#[test]
fn the_ai_hold_stand_in_by_mode() {
    let hold = Map {
        hold: true,
        ..Map::default()
    };
    let mut env = Env::new(hold, ParityMode::Reference);
    env.spawn();
    let s = env.slice.clone();
    assert!(s.hit_dummy(env.vm(), false).unwrap());
    assert!(env.slice.dummy_trace().unwrap().iter().any(|l| l.ends_with("Idle_Hold")));
    let mut env = Env::new(hold, ParityMode::Off);
    env.spawn();
    let s = env.slice.clone();
    let e = s.hit_dummy(env.vm(), false).unwrap_err();
    assert!(matches!(e.not_yet().map(|n| &n.missing), Some(Missing::NotPorted)), "{e:?}");
}

/// The health pool stand-in: `health:` is `NotYet` in Off mode; in Reference mode it is the C++ (no class data in the
/// plain fixture, so no pool and no health events).
#[test]
fn the_health_stand_in_by_mode() {
    let mut env = Env::off();
    let s = env.slice.clone();
    let e = s.set_player_health(env.vm(), 20.0).unwrap_err();
    assert!(matches!(e.not_yet().map(|n| &n.missing), Some(Missing::NotRead)), "{e:?}");
    let mut env = Env::new(Map::default(), ParityMode::Reference);
    let s = env.slice.clone();
    s.set_player_health(env.vm(), 20.0).unwrap();
    assert!(env.slice.drain().is_empty());
}

/// The value of int property `name` of `o` (test helper).
fn int_of(vm: &Vm, o: ObjRef, name: &str) -> i32 {
    match prop(&vm.space, o, name).unwrap() {
        Some(Value::Int(i)) => i,
        other => panic!("{name}: {other:?}"),
    }
}

/// A stand-in for the installed `ExpLevelUp`, which the toy controller's empty script lacks: one level and one skill
/// point on the controller's replication info (invented; the bridge's own tests cover the script path).
fn level_up_stand_in(vm: &mut Vm) {
    let f: ow_vm::api::NativeFn = Rc::new(|c| {
        let controller = c.this.expect("the controller");
        let pri = match prop(&c.vm.space, controller, "PlayerReplicationInfo").unwrap() {
            Some(Value::Object(Some(o))) => o,
            other => panic!("{other:?}"),
        };
        for name in ["ExpLevel", "GeneralSkillPoints"] {
            let v = int_of(c.vm, pri, name);
            let class = c.vm.space.heap.get(pri)?.class;
            let p = c.vm.space.prop(class, name).unwrap();
            c.vm.space.write(pri, PropAddr::scalar(p), Value::Int(v + 1))?;
        }
        Ok(Value::None)
    });
    vm.override_script("WillowGame.WillowPlayerController.ExpLevelUp", f).unwrap();
}

/// What the stand-in reproduces (Reference): `player:` builds the pool over the class's `HealthPoolDefinition` (its
/// resource's `DefaultResourcePoolClass`, in a manager on the controller) and seeds it full; a level-up then reports
/// the C++ order Level, SkillPoints, MaxHealth, Health; and the player pawn's `HealthPool` points at the pool (the
/// C++ `linkPlayerHealth`: the manager, slot 0, the pool). Off: the same level-up is `NotYet { NotRead }`.
#[test]
fn the_health_stand_in_reproduces_the_cxx_events() {
    let health = Map {
        health: true,
        ..Map::default()
    };
    let mut env = Env::new(health, ParityMode::Reference);
    level_up_stand_in(env.vm());
    let s = env.slice.clone();
    s.set_player_experience(env.vm(), 3, 0).unwrap();
    s.cheat_level_up(env.vm()).unwrap();
    let events: Vec<(HostKind, String, String)> = s.drain().into_iter().map(|e| (e.kind, e.a, e.b)).collect();
    let text = |x: &str| x.to_string();
    assert_eq!(
        events,
        [
            (HostKind::Level, text("4"), text("")),
            (HostKind::SkillPoints, text("1"), text("")),
            (HostKind::MaxHealth, text("100.000000"), text("")),
            (HostKind::Health, text("100.000000"), text("100.000000")),
        ],
        "{:?}",
        s.errors()
    );
    let controller = s.script().player_controller();
    let manager = match prop(&env.world.vm.space, controller, "ResourcePoolManager").unwrap() {
        Some(Value::Object(Some(m))) => m,
        other => panic!("no manager: {other:?}"),
    };
    let pawn = s.script().player_pawn(env.vm()).unwrap();
    let reference = prop(&env.world.vm.space, pawn, "HealthPool").unwrap().expect("the pawn's reference");
    let r = ow_world::pools::read_ref(&env.world.vm.space, &reference).unwrap();
    assert_eq!((r.manager, r.index), (Some(manager), 0));
    assert!(r.data.is_some(), "the cached pool");

    let mut env = Env::new(health, ParityMode::Off);
    level_up_stand_in(env.vm());
    let s = env.slice.clone();
    s.set_player_experience(env.vm(), 3, 0).unwrap();
    let e = s.cheat_level_up(env.vm()).unwrap_err();
    assert!(matches!(e.not_yet().map(|n| &n.missing), Some(Missing::NotRead)), "{e:?}");
}

/// The tick adds its seconds to `WorldInfo.TimeSeconds` (the C++ `advanceTime`), as a float.
#[test]
fn the_tick_advances_the_game_time() {
    let mut env = Env::off();
    let s = env.slice.clone();
    s.tick(env.vm(), 0.5).unwrap();
    s.tick(env.vm(), 0.25).unwrap();
    let wi = env.slice.inner.world_info.expect("the bridge's WorldInfo");
    assert_eq!(prop(&env.world.vm.space, wi, "TimeSeconds").unwrap(), Some(Value::Float(0.75)));
}

/// One dialog: the slice's is the mission's own `Rc` (the C++ `mission_.dialog()`), and the dummy's pawn is a talker
/// of it after the spawn (its body class has no dialog name here, so nothing is registered for it).
#[test]
fn the_dialog_is_the_mission_s() {
    let mut env = Env::new(
        Map {
            pawn: true,
            ..Map::default()
        },
        ParityMode::Off,
    );
    assert!(Rc::ptr_eq(&env.slice.mission().dialog(), &env.slice.dialog()));
    env.spawn();
    assert!(env.slice.mission().with_dialog(|_| ()).is_ok(), "no borrow is left after the spawn");
}

/// The `--slice-run` event kinds, in the C++ order of its `kinds` table.
#[test]
fn the_event_kinds_have_the_cxx_names() {
    let kinds = [
        HostKind::RemoteEvent,
        HostKind::Dialog,
        HostKind::StatusEffect,
        HostKind::MissionWeaponGranted,
        HostKind::MissionWeaponRemoved,
        HostKind::Reward,
        HostKind::Status,
        HostKind::ObjectiveSet,
        HostKind::ObjectiveComplete,
        HostKind::Experience,
        HostKind::Level,
        HostKind::MissionInterface,
        HostKind::OnUseDialog,
        HostKind::SkillPoints,
        HostKind::MaxHealth,
        HostKind::Health,
        HostKind::SequenceChange,
        HostKind::TurnInDialog,
        HostKind::ActionSkillCooldown,
        HostKind::WeaponDamageScale,
        HostKind::DismissalDialog,
        HostKind::MissionInterfaceClosed,
        HostKind::RewardPage,
        HostKind::RewardReceived,
    ];
    let names: Vec<&str> = kinds.iter().map(|k| k.json_name()).collect();
    assert_eq!(
        names,
        [
            "remote_event", "dialog", "status_effect", "mission_weapon_granted", "mission_weapon_removed", "reward",
            "status", "objective_set", "objective_complete", "experience", "level", "mission_interface",
            "on_use_dialog", "skill_points", "max_health", "health", "sequence_change", "turn_in_dialog",
            "action_skill_cooldown", "weapon_damage_scale", "dismissal_dialog", "mission_interface_closed",
            "reward_page", "reward_received",
        ]
    );
}

/// A provider that is not in the map package is the C++ error text.
#[test]
fn a_missing_provider_is_the_cxx_error() {
    let (mut world, dir) = build(Map::default(), ParityMode::Off);
    let map_name = PackageName::parse("Sanctuary_Dynamic").unwrap();
    let e = FireMissionSlice::new(&mut world, "ToyMission", &map_name, "GD_TargetDummy.Nowhere").unwrap_err();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(e.to_string(), "dummy behavior provider not found: GD_TargetDummy.Nowhere");
}

/// Replay determinism (architecture 5.2): the same scripted run, executed twice in one process on fresh worlds, gives
/// the same events, errors, trace and report.
#[test]
fn a_scripted_run_replays_identically() {
    fn run() -> Vec<String> {
        let mut env = Env::new(
            Map {
                pawn: true,
                ..Map::default()
            },
            ParityMode::Off,
        );
        let s = env.slice.clone();
        let mut out = Vec::new();
        let completed = none_completed();
        s.set_region_game_stage(4);
        out.push(format!("accept {}", s.accept(env.vm(), &completed).unwrap()));
        s.tick(env.vm(), 0.02).unwrap();
        s.spawn_dummy(env.vm()).unwrap();
        out.push(format!("range {}", s.enter_range(env.vm()).unwrap()));
        s.hit_dummy(env.vm(), true).unwrap();
        s.tick(env.vm(), 1.0).unwrap();
        out.push(format!("turnin {}", s.turn_in(env.vm()).unwrap()));
        s.tick(env.vm(), 0.5).unwrap();
        for e in s.drain() {
            out.push(format!("{} {} {} {} {}", e.kind.json_name(), e.a, e.b, e.c, e.detail));
        }
        out.extend(s.errors());
        out.extend(s.dummy_trace().unwrap());
        out.extend(s.dummy_enabled_sequences().unwrap());
        out.push(s.script().report_json(env.vm()).unwrap());
        out
    }
    let first = run();
    assert!(first.len() > 10, "{first:?}");
    assert_eq!(first, run());
}
