//! Tests of the world, the actor rules, the timer pass and the world natives on an invented `Engine.upk` (AGENTS.md
//! rule 9: invented values only). Script event functions call the test native `Actor.Note(int)`, which records
//! (object, number), so the order of events is visible.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_core::sim::ClockError;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef};
use ow_object::value::Value;
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::{Asm, CPF_OPT, FunctionIds, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{Arg, FunctionDecl, NativeCall, Vm};
use ow_vm::error::VmError;
use ow_vm::natives_core;

use super::*;
use crate::actors::{SpawnRequest, destroy, field, set_field, set_owner, spawn};

const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC: u32 = 0x2000;

// ---------------------------------------------------------------------------------------------------------------
// The invented package
// ---------------------------------------------------------------------------------------------------------------

struct Ids {
    note: i32,
    set_timer: i32,
    clear_timer: i32,
    destroy: i32,
}

fn void(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    params: &[Param],
    body: impl FnOnce(&FunctionIds, &mut Asm),
) -> FunctionIds {
    let spec = FunctionSpec::new(SCRIPT, name);
    t.function(owner, name, params, None, &[], &spec, |ids| {
        let mut a = Asm::new();
        body(ids, &mut a);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    })
}

fn native(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    params: &[Param],
    result: Option<(&str, i32)>,
    flags: u32,
) -> i32 {
    let spec = FunctionSpec::new(flags, name);
    t.function(owner, name, params, result, &[], &spec, |_| None)
        .function
}

fn opt(kind: &str, name: &str, type_ref: i32) -> Param {
    Param {
        flags: CPF_OPT,
        type_ref,
        ..Param::new(kind, name)
    }
}

fn note(a: &mut Asm, ids: &Ids, k: i32) {
    a.stmt().final_function(ids.note, |a| {
        a.int_const(k);
    });
}

/// `SetTimer(rate, false, 'name')`.
fn set_timer(a: &mut Asm, ids: &Ids, rate: f32, name: &[u8; 8]) {
    a.stmt().final_function(ids.set_timer, |a| {
        a.float_const(rate);
        a.false_();
        a.raw(&[0x21]).raw(name);
    });
}

/// A state `name` of class `owner` with empty state code: its functions are declared with it as their owner.
fn toy_state(t: &mut ScriptToy, owner: i32, name: &str) -> i32 {
    let mut a = Asm::new();
    a.stmt().raw(&[0x0C]);
    let none = t.none;
    a.raw(&none).i32(0);
    a.end();
    let label = u16::try_from(a.bytes.len()).unwrap_or(0);
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend_from_slice(&none);
    payload.extend_from_slice(&ow_package::testkit::ints(&[0, 0, 0, 0, 0, 0, 0]));
    payload.extend_from_slice(&ow_package::testkit::ints(&[
        i32::try_from(a.memory_size()).unwrap(),
        i32::try_from(a.bytes.len()).unwrap(),
    ]));
    payload.extend_from_slice(&a.bytes);
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&label.to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&ow_package::testkit::ints(&[0]));
    let mut export = ow_package::testkit::Export::new(t.import("State"), name, payload);
    export.outer = owner;
    t.p.add_export_with(export)
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "State",
        "ScriptStruct",
        "Enum",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "NameProperty",
        "StrProperty",
        "ByteProperty",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
        "StrProperty",
        "ClassProperty",
    ]);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let rotator = toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }
    let init = toy_struct(&mut t, object, "AttributeInitializationData", None);
    t.prop("Float", init, "BaseValueConstant", 0, 0);
    t.prop("Object", init, "BaseValueAttribute", 0, object);
    t.prop("Object", init, "InitializationDefinition", 0, object);
    t.prop("Float", init, "BaseValueScaleConstant", 0, 0);

    let actor = t.class("Actor", object);
    let mut collide_actors = 0;
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend_from_slice(&t.none);
    let values = ["PHYS_None", "PHYS_Walking", "PHYS_Falling", "PHYS_Rotating"];
    payload.extend(ow_package::testkit::ints(&[0, values.len() as i32]));
    for v in values {
        let bytes = t.p.fname(v, 0);
        payload.extend_from_slice(&bytes);
    }
    let mut e = ow_package::testkit::Export::new(t.import("Enum"), "EPhysics", payload);
    e.outer = actor;
    let physics = t.p.add_export_with(e);
    for b in [
        "bStatic",
        "bNoDelete",
        "bDeleteMe",
        "bPendingDelete",
        "bCollideWorld",
        "bCollideWhenPlacing",
        "bCollideActors",
        "bNetTemporary",
        "bShouldBaseAtStartup",
        // The bring-up's fields (lane MAP-LOAD-C, NATIVE_MAP_LOAD.md sections 2.1 and 2.2).
        "bScriptInitialized",
        "bRouteBeginPlayEvenIfStatic",
        "bLoadIfPhysXLevel0",
        "bLoadIfPhysXLevel1",
        "bLoadIfPhysXLevel2",
        // The spawn's spot search (lane IMPL-SPAWN-PLACEMENT, NATIVE_SPAWN_PLACEMENT.md section 2).
        "bCollideComplex",
        // Its trace filters (lane IMPL-SPAWN-TRACE-FILTERS, NATIVE_SPAWN_PLACEMENT.md section 7).
        "bWorldGeometry",
    ] {
        let r = t.prop("Bool", actor, b, 0, 0);
        if b == "bCollideActors" {
            collide_actors = r;
        }
    }
    // The template's collision cylinder (lane IMPL-SPAWN-PLACEMENT): an invented component class with the game's
    // class and field names.
    // Its primitive parent carries the trace filters' blocking flags (lane IMPL-SPAWN-TRACE-FILTERS).
    let primitive = t.class("PrimitiveComponent", object);
    // `bAttached` and `CollideActors`: the extent's `Actor` rule (lane IMPL-LINK9-EXTENT, NATIVE_SPAWN_EXTENT.md).
    // `BulletListener`: the skip flag as NATIVE_SPAWN_TRACE_FILTER.md infers it (lane IMPL-LINK9-TRACE-FILTER).
    for b in ["BlockNonZeroExtent", "BlockZeroExtent", "BlockActors", "bAttached", "CollideActors", "BulletListener"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    let cylinder = t.class("CylinderComponent", primitive);
    t.prop("Float", cylinder, "CollisionRadius", 0, 0);
    t.prop("Float", cylinder, "CollisionHeight", 0, 0);
    t.prop("Object", actor, "CollisionComponent", 0, object);
    for b in ["Role", "RemoteRole", "TickGroup"] {
        t.prop("Byte", actor, b, 0, 0);
    }
    t.prop("Byte", actor, "Physics", 0, physics);
    for f in ["LifeSpan", "CreationTime", "LastRenderTime"] {
        t.prop("Float", actor, f, 0, 0);
    }
    for o in ["Owner", "Base", "Instigator", "WorldInfo"] {
        t.prop("Object", actor, o, 0, actor);
    }
    t.prop("Struct", actor, "Location", 0, vector);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    // `AllComponents`: the spawn's component update (lane IMPL-SPAWN-COMPONENT-ATTACH).
    for a in ["Attached", "Touching", "GeneratedEvents", "Components", "AllComponents"] {
        ow_package::testkit::reflect::toy_array_of(&mut t, actor, a, "Object", object);
    }
    let note_fn = native(
        &mut t,
        actor,
        "Note",
        &[Param::new("Int", "N")],
        None,
        NATIVE,
    );
    let name_obj = [
        opt("Name", "inTimerFunc", 0),
        opt("Object", "inObj", object),
    ];
    let set_timer_fn = native(
        &mut t,
        actor,
        "SetTimer",
        &[
            Param::new("Float", "InRate"),
            opt("Bool", "inbLoop", 0),
            opt("Name", "inTimerFunc", 0),
            opt("Object", "inObj", object),
        ],
        None,
        NATIVE,
    );
    let clear_timer_fn = native(&mut t, actor, "ClearTimer", &name_obj, None, NATIVE);
    native(
        &mut t,
        actor,
        "IsTimerActive",
        &name_obj,
        Some(("Bool", 0)),
        NATIVE,
    );
    native(
        &mut t,
        actor,
        "GetTimerCount",
        &name_obj,
        Some(("Float", 0)),
        NATIVE,
    );
    native(
        &mut t,
        actor,
        "GetTimerRate",
        &name_obj,
        Some(("Float", 0)),
        NATIVE,
    );
    native(
        &mut t,
        actor,
        "PauseTimer",
        &[
            Param::new("Bool", "bPause"),
            name_obj[0].clone(),
            name_obj[1].clone(),
        ],
        None,
        NATIVE,
    );
    native(
        &mut t,
        actor,
        "ModifyTimerTimeDilation",
        &[
            Param::new("Name", "TimerName"),
            Param::new("Float", "InTimerTimeDilation"),
            name_obj[1].clone(),
        ],
        None,
        NATIVE,
    );
    native(
        &mut t,
        actor,
        "ClearAllTimers",
        &[name_obj[1].clone()],
        None,
        NATIVE,
    );
    let destroy_fn = native(&mut t, actor, "Destroy", &[], Some(("Bool", 0)), NATIVE);
    let ids = Ids {
        note: note_fn,
        set_timer: set_timer_fn,
        clear_timer: clear_timer_fn,
        destroy: destroy_fn,
    };

    let world_info = t.class("WorldInfo", actor);
    for f in [
        "TimeSeconds",
        "RealTimeSeconds",
        "TimeDilation",
        "DeltaSeconds",
        "AudioTimeSeconds",
    ] {
        t.prop("Float", world_info, f, 0, 0);
    }
    for b in ["bBegunPlay", "bStartup"] {
        t.prop("Bool", world_info, b, 0, 0);
    }
    t.prop("Byte", world_info, "NetMode", 0, 0);
    // The world's two lists (decision 2026-11-19-92).
    t.prop("Object", world_info, "ControllerList", 0, actor);
    t.prop("Object", world_info, "PawnList", 0, actor);
    // The game info the world's hooks fall back to (decision 2026-11-16-49).
    t.prop("Object", world_info, "Game", 0, actor);
    // The map's own game class (lane IMPL-MAPLOAD-GAMEINFO): a class reference, its meta class the actor class.
    let class_ref = t.import("Class");
    t.prop_tail("Class", world_info, "DefaultGameType", 0, &ow_package::testkit::ints(&[class_ref, actor]));
    native(
        &mut t,
        world_info,
        "GetWorldInfo",
        &[],
        Some(("Object", world_info)),
        NATIVE | STATIC,
    );
    native(&mut t, world_info, "IsPlayInEditor", &[], Some(("Bool", 0)), NATIVE);

    let rearm = t.p.fname("Rearm", 0);
    let clear_me = t.p.fname("ClearMe", 0);
    let ping = t.p.fname("Ping", 0);
    let nothing = t.p.fname("Nothing", 0);
    let probe = t.class("Probe", actor);
    void(&mut t, probe, "Ping", &[], |_, a| note(a, &ids, 1));
    void(&mut t, probe, "Pong", &[], |_, a| note(a, &ids, 2));
    void(&mut t, probe, "Rearm", &[], |_, a| {
        note(a, &ids, 3);
        set_timer(a, &ids, 1.0, &rearm);
    });
    void(&mut t, probe, "ClearMe", &[], |_, a| {
        note(a, &ids, 4);
        a.stmt().final_function(ids.clear_timer, |a| {
            a.raw(&[0x21]).raw(&clear_me);
        });
    });
    void(&mut t, probe, "Kill", &[], |_, a| {
        note(a, &ids, 5);
        a.stmt().final_function(ids.destroy, |_| {});
    });
    // Appends a rate-0 timer 'Nothing' from inside a callback: the same pass visits and culls it.
    void(&mut t, probe, "Append", &[], |_, a| {
        note(a, &ids, 6);
        set_timer(a, &ids, 0.0, &nothing);
    });
    // Appends a live timer 'Ping' from inside a callback: visited in the same pass, not due (Count 0).
    void(&mut t, probe, "AppendPing", &[], |_, a| {
        note(a, &ids, 7);
        set_timer(a, &ids, 0.5, &ping);
    });
    void(
        &mut t,
        probe,
        "WithArg",
        &[Param::new("Int", "N")],
        |ids_, a| {
            a.stmt().final_function(ids.note, |a| {
                a.local(ids_.get("N"));
            });
        },
    );
    void(
        &mut t,
        probe,
        "Tick",
        &[Param::new("Float", "DeltaTime")],
        |_, a| note(a, &ids, 10),
    );
    void(&mut t, probe, "PreBeginPlay", &[], |_, a| note(a, &ids, 11));
    void(&mut t, probe, "PostBeginPlay", &[], |_, a| {
        note(a, &ids, 12)
    });
    void(&mut t, probe, "SetInitialState", &[], |_, a| {
        note(a, &ids, 13)
    });
    void(&mut t, probe, "Destroyed", &[], |_, a| note(a, &ids, 14));
    // The game info's InitGame(Options, out Error) for the bring-up (lane MAP-LOAD-C).
    void(
        &mut t,
        probe,
        "InitGame",
        &[Param::new("Str", "Options"), Param { flags: ow_package::testkit::script::CPF_OUT, ..Param::new("Str", "ErrorMessage") }],
        |_, a| note(a, &ids, 17),
    );
    void(
        &mut t,
        probe,
        "GainedChild",
        &[opt("Object", "Other", actor)],
        |_, a| note(a, &ids, 15),
    );
    void(
        &mut t,
        probe,
        "LostChild",
        &[opt("Object", "Other", actor)],
        |_, a| note(a, &ids, 16),
    );

    let dud = t.class("Dud", actor);
    void(&mut t, dud, "PreBeginPlay", &[], |_, a| {
        a.stmt().final_function(ids.destroy, |_| {});
    });
    // An invented actor whose PreBeginPlay turns on bCollideActors (lane IMPL-SPAWN-PLACEMENT): LetBool,
    // BoolVariable(instance bCollideActors), True.
    // An invented native the tests bind to answer a NotYet, and an actor whose PreBeginPlay calls it (review r1 of #463).
    let halt_fn = native(&mut t, actor, "Halt", &[], None, NATIVE);
    let halter = t.class("Halter", actor);
    void(&mut t, halter, "PreBeginPlay", &[], |_, a| {
        a.stmt().final_function(halt_fn, |_| {});
    });
    // A class whose default object no test builds before its spawn, its default export setting bCollideWorld (review
    // r1 of #514, item 3): the spawn builds and publishes it cold.
    let cold = t.class("ColdCollider", actor);
    let mut cold_default = ow_package::testkit::ints(&[0]);
    cold_default.extend(t.p.bool_tag("bCollideWorld", true));
    cold_default.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(cold, "Default__ColdCollider", cold_default));
    let collider = t.class("Collider", actor);
    void(&mut t, collider, "PreBeginPlay", &[], |_, a| {
        a.stmt().raw(&[0x14, 0x2D]).instance(collide_actors).raw(&[0x27]);
    });
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "NextController", 0, actor);
    t.prop("Object", controller, "Pawn", 0, actor);
    t.prop("Bool", controller, "bIsPlayer", 0, 0);
    t.prop("Object", controller, "ResourcePoolManager", 0, actor);
    let gearbox_mind = t.class("GearboxMind", controller);
    t.prop("Object", gearbox_mind, "MyRuleEngine", 0, object);
    let willow_mind = t.class("WillowMind", gearbox_mind);
    t.prop("Object", willow_mind, "InteractionProxy", 0, actor);
    // The kill's end, as script: the mind's PawnDied destroys the mind (Controller.PawnDied for a non-player).
    void(&mut t, willow_mind, "PawnDied", &[], |_, a| {
        a.stmt().final_function(ids.destroy, |_| {});
    });
    let pawn = t.class("Pawn", actor);
    t.prop("Object", pawn, "NextPawn", 0, actor);
    t.prop("Object", pawn, "Controller", 0, actor);
    // The kill's state (review r1 of #481, item 1): `Dying.BeginState` notes 30, standing for its dying writes.
    let dying = toy_state(&mut t, pawn, "Dying");
    void(&mut t, dying, "BeginState", &[], |_, a| note(a, &ids, 30));
    // Who is in the pawn list when a script asks (review r1 of #481, item 5).
    let snapshot = native(&mut t, actor, "SnapshotPawns", &[], None, NATIVE);
    let listed = t.class("ListedPawn", pawn);
    void(&mut t, listed, "Destroyed", &[], |_, a| {
        a.stmt().final_function(snapshot, |_| {});
    });
    let collide_now = native(&mut t, actor, "CollideNow", &[], None, NATIVE);
    let colliding = t.class("CollidingPawn", pawn);
    void(&mut t, colliding, "PreBeginPlay", &[], |_, a| {
        a.stmt().final_function(collide_now, |_| {});
    });
    let listed_owner = t.class("ListedOwner", actor);
    void(&mut t, listed_owner, "LostChild", &[opt("Object", "Other", actor)], |_, a| {
        a.stmt().final_function(snapshot, |_| {});
    });
    let refuse = native(&mut t, actor, "Refuse", &[], None, NATIVE);
    let doomed = t.class("DoomedPawn", pawn);
    void(&mut t, doomed, "Destroyed", &[], |_, a| {
        a.stmt().final_function(refuse, |_| {});
    });
    let local_player = t.class("LocalPlayer", object);
    // The local player's play actor (lane IMPL-MAPLOAD-PLAYER): `LocalPlayer.Actor`, `PlayerController.NetPlayerIndex`.
    t.prop("Object", local_player, "Actor", 0, actor);
    let player_controller = t.class("PlayerController", controller);
    t.prop("Object", player_controller, "Player", 0, object);
    t.prop("Bool", player_controller, "bPendingDestroy", 0, 0);
    t.prop("Byte", player_controller, "NetPlayerIndex", 0, 0);
    // A toy game info whose Login and PostLogin are natives the player tests bind (the game's are script).
    let toy_game = t.class("ToyGame", actor);
    let out = ow_package::testkit::script::CPF_OUT;
    native(
        &mut t,
        toy_game,
        "Login",
        &[
            Param::new("Str", "Portal"),
            Param::new("Str", "Options"),
            Param::new("Int", "UniqueId"),
            Param { flags: out, ..Param::new("Str", "ErrorMessage") },
        ],
        Some(("Object", player_controller)),
        NATIVE,
    );
    native(&mut t, toy_game, "PostLogin", &[Param { type_ref: player_controller, ..Param::new("Object", "NewPlayer") }], None, NATIVE);
    native(&mut t, player_controller, "IsLocalPlayerController", &[], Some(("Bool", 0)), NATIVE);
    let ui = t.class("UIInteraction", object);
    native(&mut t, ui, "GetDataStoreClient", &[], Some(("Object", object)), NATIVE | STATIC);
    // The player pawn data manager (decision 2026-11-19-24): the game's class names, which the natives' keys carry.
    let manager = t.class("WillowPlayerPawnDataManager", object);
    let mut path = Param::new("Str", "Path");
    path.flags = 0x80;
    native(&mut t, manager, "FindAlreadyLoadedObject", &[path], Some(("Object", object)), NATIVE);
    let globals = t.class("WillowGlobals", object);
    t.prop("Object", globals, "ThePlayerPawnDataManager", 0, manager);
    native(&mut t, globals, "GetPlayerPawnDataManager", &[], Some(("Object", manager)), NATIVE);
    let thing = t.class("Thing", object);
    // Two invented data exports for FindAlreadyLoadedObject (decision 2026-11-19-24, review r1 of #411): loading
    // ToyArchetype makes its Target, ToyTarget, a store handle that is not loaded.
    t.prop("Object", thing, "Target", 0, thing);
    let mut target = 0i32.to_le_bytes().to_vec();
    target.extend_from_slice(&t.none);
    let toy_target = t.p.add_export(thing, "ToyTarget", target);
    let mut archetype = 0i32.to_le_bytes().to_vec();
    archetype.extend(t.p.object_tag("Target", toy_target));
    archetype.extend_from_slice(&t.none);
    t.p.add_export(thing, "ToyArchetype", archetype);
    // An invented actor whose archetype names a definition (lane SPAWN-COMPLETE): BodyProbe_Arch's Body is ToyTarget,
    // a store handle that stays unloaded until something reads it. BodyProbe_Collider's archetype also sets
    // bCollideActors, so its spawn refuses after PreBeginPlay.
    let body_probe = t.class("BodyProbe", actor);
    t.prop("Object", body_probe, "Body", 0, thing);
    let mut body_arch = 0i32.to_le_bytes().to_vec();
    body_arch.extend(t.p.object_tag("Body", toy_target));
    body_arch.extend_from_slice(&t.none);
    t.p.add_export(body_probe, "BodyProbe_Arch", body_arch);
    let mut body_collider = 0i32.to_le_bytes().to_vec();
    body_collider.extend(t.p.object_tag("Body", toy_target));
    body_collider.extend(t.p.bool_tag("bCollideActors", true));
    body_collider.extend_from_slice(&t.none);
    t.p.add_export(body_probe, "BodyProbe_Collider", body_collider);
    // A probe archetype the store loads lazily (review r2 of #481, M1): ToyLazyHolder's Arch is an unloaded handle to
    // Probe_LazyArch, whose bCollideWorld asks for the spot search.
    let lazy_holder = t.class("LazyHolder", object);
    t.prop("Object", lazy_holder, "Arch", 0, probe);
    let mut lazy = 0i32.to_le_bytes().to_vec();
    lazy.extend(t.p.bool_tag("bCollideWorld", true));
    lazy.extend_from_slice(&t.none);
    let lazy_arch = t.p.add_export(probe, "Probe_LazyArch", lazy);
    let mut holder = 0i32.to_le_bytes().to_vec();
    holder.extend(t.p.object_tag("Arch", lazy_arch));
    holder.extend_from_slice(&t.none);
    t.p.add_export(lazy_holder, "ToyLazyHolder", holder);
    // The travel countdown (lane MAP-LOAD-C, NATIVE_TRAVEL.md).
    let willow_game = t.class("WillowGameInfo", actor);
    t.prop("Int", willow_game, "TravelPostCountdownTime", 0, 0);
    // The editor query (lane IMPL-IS-EDITOR).
    native(&mut t, willow_game, "IsEditor", &[], Some(("Bool", 0)), NATIVE);
    native(&mut t, willow_game, "TravelPostCountdownInProcess", &[], Some(("Bool", 0)), NATIVE);

    let definition = t.class("AttributeInitializationDefinition", object);
    native(
        &mut t,
        definition,
        "EvaluateInitializationData",
        &[
            Param {
                type_ref: init,
                ..Param::new("Struct", "InitializationData")
            },
            Param {
                type_ref: object,
                ..Param::new("Object", "ContextSource")
            },
            opt("Object", "OptionalOverrideContextSource", object),
        ],
        Some(("Float", 0)),
        NATIVE | STATIC,
    );
    // SetBaseValue (lane IMPL-SET-BASE-VALUE): its lazy-handle preflight test.
    let object_param = |name: &str| Param { type_ref: object, ..Param::new("Object", name) };
    let base_value = Param { type_ref: init, flags: ow_package::testkit::script::CPF_OUT, ..Param::new("Struct", "BaseValue") };
    let set_base_params = [object_param("DestAttribute"), base_value, object_param("ValueContextSource"), object_param("AttributeContextSource")];
    native(&mut t, definition, "SetBaseValue", &set_base_params, Some(("Bool", 0)), NATIVE | STATIC);
    // An attribute whose value resolver the store loads lazily (review r1 of #572): `Toy_LazyAttr`'s
    // `ValueResolverChain` names the export `Toy_LazyReader`, an unloaded handle once `Toy_LazyAttr` is loaded.
    let attr_base = t.class("AttributeDefinitionBase", object);
    for chain in ["ContextResolverChain", "ValueResolverChain"] {
        let array = t.p.next_export();
        t.prop("Array", attr_base, chain, 0, array + 1);
        t.prop("Object", array, chain, 0, object);
    }
    let attr_class = t.class("AttributeDefinition", attr_base);
    let reader_class = t.class("ObjectPropertyAttributeValueResolver", object);
    t.prop("Name", reader_class, "PropertyName", 0, 0);
    // Review r2 of #572: a context resolver class `ow-attr` implements (a pawn resolves to itself) and a valid target,
    // a pawn with a plain float attribute (the package flag bit 62); and `Toy_AttrHolder`, whose `Attr` names the
    // attribute export `Toy_UnreachedAttr`, an unloaded handle once the holder is loaded.
    t.class("PawnAttributeContextResolver", object);
    let attr_target = t.class("AttrTarget", pawn);
    t.prop("Float", attr_target, "Toughness", 1 << 62, 0);
    let holder_class = t.class("AttrHolder", object);
    t.prop("Object", holder_class, "Attr", 0, object);
    let lazy_attr_ref = t.p.next_export();
    let reader_ref = lazy_attr_ref + 1;
    let mut chain = ow_package::testkit::ints(&[1]);
    chain.extend_from_slice(&reader_ref.to_le_bytes());
    let mut lazy_attr = 0i32.to_le_bytes().to_vec();
    lazy_attr.extend(t.p.tag("ValueResolverChain", "ArrayProperty", &chain));
    lazy_attr.extend_from_slice(&t.none);
    t.p.add_export(attr_class, "Toy_LazyAttr", lazy_attr);
    let mut reader = 0i32.to_le_bytes().to_vec();
    reader.extend(t.p.name_tag("PropertyName", "Toughness"));
    reader.extend_from_slice(&t.none);
    t.p.add_export(reader_class, "Toy_LazyReader", reader);
    let unreached_ref = t.p.next_export();
    let mut unreached = 0i32.to_le_bytes().to_vec();
    unreached.extend_from_slice(&t.none);
    t.p.add_export(attr_class, "Toy_UnreachedAttr", unreached);
    let mut holder = 0i32.to_le_bytes().to_vec();
    holder.extend(t.p.object_tag("Attr", unreached_ref));
    holder.extend_from_slice(&t.none);
    t.p.add_export(holder_class, "Toy_AttrHolder", holder);
    // An actor class with a component field, and its component class (a Core.Component), for the instancing test.
    // Core.Component, which `core_package` defines for the instancing test (review r1 of #435).
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let component = t.p.add_import("Core", "Class", core, "Component");
    let toy_component = t.class("ToyComponent", component);
    t.prop("Int", toy_component, "Payload", 0, 0);
    // The fields the spawn's component update reads and writes (lane IMPL-SPAWN-COMPONENT-ATTACH).
    t.prop("Object", toy_component, "Owner", 0, actor);
    t.prop("Object", toy_component, "Scene", 0, object);
    for f in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform", "bSkipChildComponentUpdate"] {
        t.prop("Bool", toy_component, f, 0, 0);
    }
    // A component class without the fields the spawn's component update needs (lane IMPL-SPAWN-COMPONENT-ATTACH).
    t.class("ToyBareComponent", component);
    let toolbox = t.class("Toolbox", actor);
    t.prop("Object", toolbox, "Comp", 0, toy_component);
    t.prop("Object", toolbox, "Thing", 0, object);
    // An archetype export of Toolbox (the lazy-load test, review r3 of #435): its `Comp` names its own component
    // export, which the store loads lazily, and its `Thing` names the archetype itself.
    let arch_ref = t.p.next_export();
    let comp_ref = arch_ref + 1;
    let mut arch_payload = ow_package::testkit::ints(&[0]);
    arch_payload.extend(t.p.tag("Comp", "ObjectProperty", &comp_ref.to_le_bytes()));
    arch_payload.extend(t.p.tag("Thing", "ObjectProperty", &arch_ref.to_le_bytes()));
    arch_payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(toolbox, "Toolbox_Arch", arch_payload));
    let mut comp_payload = ow_package::testkit::ints(&[0, 0]);
    comp_payload.extend(t.p.int_tag("Payload", 7));
    comp_payload.extend_from_slice(&t.none);
    let mut comp = ow_package::testkit::Export::new(toy_component, "ToyComponent0", comp_payload);
    comp.outer = arch_ref;
    t.p.add_export_with(comp);
    // The inherited-component test (decision 2026-11-19-91): `ParentComp`, the export a parent class default's
    // component stands for (the test makes that component by hand); `Toolbox_Inherit`, an archetype whose own
    // `ToyComponent0` has `ParentComp` as its export archetype; `Toolbox_Bare`, an archetype with no sub-object.
    let parent_ref = t.p.next_export();
    let mut parent_payload = ow_package::testkit::ints(&[0, 0]);
    parent_payload.extend(t.p.int_tag("Payload", 5));
    parent_payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(toy_component, "ParentComp", parent_payload));
    let inherit_ref = t.p.next_export();
    let mut inherit_payload = ow_package::testkit::ints(&[0]);
    inherit_payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(toolbox, "Toolbox_Inherit", inherit_payload));
    let mut own_payload = ow_package::testkit::ints(&[0, 0]);
    own_payload.extend(t.p.int_tag("Payload", 9));
    own_payload.extend_from_slice(&t.none);
    let mut own = ow_package::testkit::Export::new(toy_component, "ToyComponent0", own_payload);
    own.outer = inherit_ref;
    own.archetype = parent_ref;
    t.p.add_export_with(own);
    let mut bare_payload = ow_package::testkit::ints(&[0]);
    bare_payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(toolbox, "Toolbox_Bare", bare_payload));
    // `Toolbox_Twin`: two sub-objects made from `ParentComp` (review r1 of #478, item 1).
    let twin_ref = t.p.next_export();
    let mut twin_payload = ow_package::testkit::ints(&[0]);
    twin_payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(toolbox, "Toolbox_Twin", twin_payload));
    for name in ["ToyComponent0", "ToyComponent1"] {
        let mut payload = ow_package::testkit::ints(&[0, 0]);
        payload.extend(t.p.int_tag("Payload", 3));
        payload.extend_from_slice(&t.none);
        let mut sub = ow_package::testkit::Export::new(toy_component, name, payload);
        sub.outer = twin_ref;
        sub.archetype = parent_ref;
        t.p.add_export_with(sub);
    }
    // `Toolbox_Pointer`: its `Thing` names `Toolbox_Arch`, so loading it gives a handle of that archetype the store has
    // not loaded yet (review r2 of #478).
    let mut pointer_payload = ow_package::testkit::ints(&[0]);
    pointer_payload.extend(t.p.tag("Thing", "ObjectProperty", &arch_ref.to_le_bytes()));
    pointer_payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(toolbox, "Toolbox_Pointer", pointer_payload));
    // Two `Actor` archetypes for the extent's loads (lane IMPL-LINK9-EXTENT, NATIVE_SPAWN_EXTENT.md section 5): each
    // lists its own primitive component (lazy until loaded), a None entry and its own non-primitive component.
    // `Actor_ExtentQuiet`'s primitive is unattached; `Actor_ExtentLive`'s is attached and collides.
    let thing = t.class("ExtentThing", object);
    t.prop("Int", thing, "Payload", 0, 0);
    for (name, live) in [("Actor_ExtentQuiet", false), ("Actor_ExtentLive", true)] {
        let arch = t.p.next_export();
        let (prim, other) = (arch + 1, arch + 2);
        let mut body = Vec::new();
        for r in [prim, 0, other] {
            body.extend_from_slice(&r.to_le_bytes());
        }
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.array_tag("Components", 3, &body));
        payload.extend_from_slice(&t.none);
        t.p.add_export_with(ow_package::testkit::Export::new(actor, name, payload));
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.bool_tag("bAttached", live));
        payload.extend(t.p.bool_tag("CollideActors", live));
        payload.extend_from_slice(&t.none);
        let mut e = ow_package::testkit::Export::new(primitive, "ExtentPrimitive", payload);
        e.outer = arch;
        t.p.add_export_with(e);
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.int_tag("Payload", 1));
        payload.extend_from_slice(&t.none);
        let mut e = ow_package::testkit::Export::new(thing, "ExtentOther", payload);
        e.outer = arch;
        t.p.add_export_with(e);
    }
    // Failing component loads (review r1 of #498, items 1 and 5): a primitive export whose archetype is an object of
    // an unrelated class fails its load with the store's `NotYet`. `Actor_ExtentFailsFirst` lists that primitive
    // alone; `Actor_ExtentFailsLater` lists a loadable primitive first, then it.
    let base = t.p.next_export();
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(thing, "ExtentBadBase", payload));
    for (name, good) in [("Actor_ExtentFailsFirst", false), ("Actor_ExtentFailsLater", true)] {
        let arch = t.p.next_export();
        let entries: Vec<i32> = if good { vec![arch + 1, arch + 2] } else { vec![arch + 1] };
        let mut body = Vec::new();
        for r in &entries {
            body.extend_from_slice(&r.to_le_bytes());
        }
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.array_tag("Components", entries.len() as i32, &body));
        payload.extend_from_slice(&t.none);
        t.p.add_export_with(ow_package::testkit::Export::new(actor, name, payload));
        for (k, _) in entries.iter().enumerate() {
            let bad = !good || k == 1;
            let mut payload = ow_package::testkit::ints(&[0]);
            payload.extend(t.p.bool_tag("bAttached", false));
            payload.extend_from_slice(&t.none);
            let mut e = ow_package::testkit::Export::new(primitive, if bad { "ExtentBadPrimitive" } else { "ExtentGoodPrimitive" }, payload);
            e.outer = arch;
            if bad {
                e.archetype = base;
            }
            t.p.add_export_with(e);
        }
    }
    // Review r1 of #528: a definition whose load itself fails, `Toolbox_Cycle` (its export archetype is itself, so the
    // Off store refuses it with `NotYet { NotRead }`, "its archetype chain is a cycle"), and `Toolbox_Broken`, an
    // archetype whose `Comp` names its own component and whose `Thing` names `Toolbox_Cycle`.
    let cycle_ref = t.p.next_export();
    let mut cycle_payload = ow_package::testkit::ints(&[0]);
    cycle_payload.extend_from_slice(&t.none);
    let mut cycle = ow_package::testkit::Export::new(toolbox, "Toolbox_Cycle", cycle_payload);
    cycle.archetype = cycle_ref;
    t.p.add_export_with(cycle);
    let broken_ref = t.p.next_export();
    let mut broken_payload = ow_package::testkit::ints(&[0]);
    broken_payload.extend(t.p.tag("Comp", "ObjectProperty", &(broken_ref + 1).to_le_bytes()));
    broken_payload.extend(t.p.tag("Thing", "ObjectProperty", &cycle_ref.to_le_bytes()));
    broken_payload.extend_from_slice(&t.none);
    t.p.add_export_with(ow_package::testkit::Export::new(toolbox, "Toolbox_Broken", broken_payload));
    let mut broken_comp_payload = ow_package::testkit::ints(&[0, 0]);
    broken_comp_payload.extend(t.p.int_tag("Payload", 4));
    broken_comp_payload.extend_from_slice(&t.none);
    let mut broken_comp = ow_package::testkit::Export::new(toy_component, "ToyComponent0", broken_comp_payload);
    broken_comp.outer = broken_ref;
    t.p.add_export_with(broken_comp);
    t.p.build()
}

/// `Core.upk`: `Core.Object` and `Core.Component`, which only the instancing test's fixture loads.
fn core_package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let object = t.class("Object", 0);
    t.class("Component", object);
    t.p.build()
}

// ---------------------------------------------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------------------------------------------

static NEXT: AtomicU32 = AtomicU32::new(0);

pub(super) struct Fx {
    pub(super) w: World,
    pub(super) wi: ObjRef,
    pub(super) notes: Rc<RefCell<Vec<(ObjRef, i32)>>>,
    /// Each `SnapshotPawns()` call: the caller and the pawn list's members at that moment.
    pub(super) pawn_lists: Rc<RefCell<Vec<(ObjRef, Vec<ObjRef>)>>>,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn snapshot() -> BalanceSnapshot {
    BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap()
}

/// A world over the invented package: its `WorldInfo` placed, play begun, and `Role` = authority in the defaults
/// of every actor class the tests spawn.
fn fx() -> Fx {
    fx_with(true)
}

fn fx_with(begin: bool) -> Fx {
    fx_in(begin, ParityMode::Off)
}

pub(super) fn fx_in(begin: bool, mode: ParityMode) -> Fx {
    fx_files(begin, mode, false)
}

thread_local! {
    /// A `Note` number the test native refuses with an invented `NotYet` (review r1 of #478, item 4).
    static REFUSE_NOTE: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
}

/// [`fx_in`], with `Core.upk` (its `Core.Component`) beside the engine package when `core` is set.
fn fx_files(begin: bool, mode: ParityMode, core: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-world-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let core_bytes = core_package();
    let mut files: Vec<(&str, &[u8])> = vec![("Engine.upk", &bytes)];
    if core {
        files.push(("Core.upk", &core_bytes));
    }
    let defs = DefStore::from_synthetic(
        &dir,
        &files,
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    let notes = Rc::new(RefCell::new(Vec::new()));
    let sink = notes.clone();
    vm.register_native(
        "Actor.Note(int)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            let k = match c.arg(0)? {
                Value::Int(k) => *k,
                other => panic!("Note got {other:?}"),
            };
            if REFUSE_NOTE.with(|r| r.get() == Some(k)) {
                return Err(ow_core::NotYet::not_ported("an invented refusal in an event", "docs/invented.md").into());
            }
            sink.borrow_mut()
                .push((c.this.expect("Note on an actor"), k));
            Ok(Value::None)
        }),
    )
    .unwrap();
    let pawn_lists = Rc::new(RefCell::new(Vec::new()));
    let sink = pawn_lists.clone();
    vm.register_native(
        "Actor.SnapshotPawns()",
        Rc::new(move |c: &mut NativeCall<'_>| {
            let me = c.this.expect("SnapshotPawns on an actor");
            let wi = crate::actors::actors_of(c.vm)?.borrow().world_info().expect("a WorldInfo");
            let members = crate::actors::list_members(&c.vm.space, wi, crate::actors::WorldList::Pawns)?;
            sink.borrow_mut().push((me, members));
            Ok(Value::None)
        }),
    )
    .unwrap();
    vm.register_native(
        "Actor.CollideNow()",
        Rc::new(|c: &mut NativeCall<'_>| {
            let me = c.this.expect("CollideNow on an actor");
            set_field(&mut c.vm.space, me, "bCollideActors", Value::Bool(true))?;
            Ok(Value::None)
        }),
    )
    .unwrap();
    vm.register_native(
        "Actor.Refuse()",
        Rc::new(|_c: &mut NativeCall<'_>| Err(ow_core::NotYet::not_read("an invented refusal", "an invented record").into())),
    )
    .unwrap();
    let mut w = World::new(vm, BalanceState::new(snapshot()), &[&natives_core::Pack]).unwrap();
    for class in [
        "Probe", "Dud", "Collider", "Halter", "Controller", "PlayerController", "WorldInfo", "Pawn", "GearboxMind", "WillowMind", "DoomedPawn",

        "Probe", "Dud", "Controller", "PlayerController", "WorldInfo", "Pawn", "GearboxMind", "WillowMind", "DoomedPawn",
        "ListedPawn", "ListedOwner", "CollidingPawn",
    ] {
        let cdo =
            w.vm.defs
                .class_default(&mut w.vm.space, &format!("Engine.{class}"))
                .unwrap();
        set_field(&mut w.vm.space, cdo, "Role", Value::Byte(3)).unwrap();
    }
    let c = w.vm.find_class("Engine.WorldInfo").unwrap();
    let wi = ow_vm::interp::instantiate_from_defaults(&mut w.vm, c).unwrap();
    set_field(&mut w.vm.space, wi, "TimeDilation", Value::Float(1.0)).unwrap();
    w.actors().borrow_mut().place_world_info(wi).unwrap();
    if begin {
        w.actors().borrow_mut().begin_play();
    }
    Fx { w, wi, notes, pawn_lists, dir }
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.w.vm
    }

    pub(super) fn class(&mut self, name: &str) -> ow_object::ids::ClassId {
        self.w.vm.find_class(&format!("Engine.{name}")).unwrap()
    }

    fn spawn(&mut self, class: &str) -> ObjRef {
        let c = self.class(class);
        let wi = self.wi;
        spawn(&mut self.w.vm, wi, &SpawnRequest::new(Some(c)))
            .unwrap()
            .unwrap()
    }

    fn name(&mut self, s: &str) -> NameId {
        self.w.vm.space.names.intern(s)
    }

    pub(super) fn call(
        &mut self,
        function: &str,
        this: Option<ObjRef>,
        args: Vec<Value>,
    ) -> Result<Value, VmError> {
        let f = self.w.vm.find_function(&format!("Engine.{function}"))?;
        Ok(self.w.vm.call(f, this, args)?.ret)
    }

    /// `SetTimer(rate, looping, name)` on `actor`, optionally for `object`.
    fn set_timer(
        &mut self,
        actor: ObjRef,
        rate: f32,
        looping: bool,
        name: &str,
        object: Option<ObjRef>,
    ) {
        let n = self.name(name);
        let mut args = vec![Value::Float(rate), Value::Bool(looping), Value::Name(n)];
        if let Some(o) = object {
            args.push(Value::Object(Some(o)));
        }
        self.call("Actor.SetTimer", Some(actor), args).unwrap();
    }

    fn timer_rate(&mut self, actor: ObjRef, name: &str) -> f32 {
        let n = self.name(name);
        match self
            .call("Actor.GetTimerRate", Some(actor), vec![Value::Name(n)])
            .unwrap()
        {
            Value::Float(f) => f,
            other => panic!("{other:?}"),
        }
    }

    fn timer_count(&mut self, actor: ObjRef, name: &str) -> f32 {
        let n = self.name(name);
        match self
            .call("Actor.GetTimerCount", Some(actor), vec![Value::Name(n)])
            .unwrap()
        {
            Value::Float(f) => f,
            other => panic!("{other:?}"),
        }
    }

    /// The numbers `o` noted, in order, without the Tick (10) and begin-play (11 to 13) notes.
    fn noted(&self, o: ObjRef) -> Vec<i32> {
        self.notes
            .borrow()
            .iter()
            .filter(|(x, k)| *x == o && !(10..=13).contains(k))
            .map(|(_, k)| *k)
            .collect()
    }

    pub(super) fn all_notes(&self) -> Vec<(ObjRef, i32)> {
        self.notes.borrow().clone()
    }

    fn clear_notes(&self) {
        self.notes.borrow_mut().clear();
    }

    fn tick(&mut self, dt: f32) {
        self.w.tick(dt).unwrap();
    }
}

pub(super) fn run(f: impl FnOnce() + Send) {
    ow_vm::interp::on_vm_stack(f).unwrap();
}

fn missing(e: WorldError) -> Missing {
    match e {
        WorldError::Vm(VmError::NotYet(n)) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn vm_missing(e: VmError) -> Missing {
    match e {
        VmError::NotYet(n) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Timers
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_timer_is_due_only_when_its_count_is_strictly_greater_than_its_rate() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 1.0, false, "Ping", None);
        x.tick(0.5);
        x.tick(0.5);
        assert_eq!(
            x.noted(p),
            Vec::<i32>::new(),
            "Count 1.0 equals Rate 1.0: not due"
        );
        assert_eq!(x.timer_count(p, "Ping"), 1.0);
        x.tick(0.5);
        assert_eq!(x.noted(p), vec![1], "fires on the third tick");
        assert_eq!(
            x.timer_rate(p, "Ping"),
            -1.0,
            "a fired non-loop entry is removed"
        );
    });
}

#[test]
fn a_loop_fires_floor_count_over_rate_times_with_count_reduced_first() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 0.25, true, "Ping", None);
        x.tick(1.1);
        assert_eq!(
            x.noted(p),
            vec![1, 1, 1, 1],
            "floor(1.1 / 0.25) = 4 firings in one pass"
        );
        let left = x.timer_count(p, "Ping");
        assert_eq!(
            left,
            1.1f32 - 4.0 * 0.25,
            "Count less firings times Rate, in f32"
        );
        assert_eq!(x.timer_rate(p, "Ping"), 0.25, "a loop stays");
    });
}

#[test]
fn timer_dilation_scales_the_count_and_pause_stops_it() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 10.0, false, "Ping", None);
        let ping = x.name("Ping");
        x.call(
            "Actor.ModifyTimerTimeDilation",
            Some(p),
            vec![Value::Name(ping), Value::Float(2.0)],
        )
        .unwrap();
        x.tick(0.5);
        assert_eq!(
            x.timer_count(p, "Ping"),
            1.0,
            "Count grows by TimerTimeDilation * delta"
        );
        x.call(
            "Actor.PauseTimer",
            Some(p),
            vec![Value::Bool(true), Value::Name(ping)],
        )
        .unwrap();
        x.tick(0.5);
        assert_eq!(
            x.timer_count(p, "Ping"),
            1.0,
            "a paused Count does not grow"
        );
        let active = x
            .call("Actor.IsTimerActive", Some(p), vec![Value::Name(ping)])
            .unwrap();
        assert_eq!(active, Value::Bool(true), "a paused timer is active");
        // SetTimer on the same pair clears the pause, resets Count and keeps the dilation.
        x.set_timer(p, 10.0, false, "Ping", None);
        x.tick(0.5);
        assert_eq!(x.timer_count(p, "Ping"), 1.0);
    });
}

#[test]
fn clearing_inside_a_callback_removes_the_entry_without_shifting_the_next_one() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 0.25, true, "ClearMe", None);
        x.set_timer(p, 0.25, false, "Ping", None);
        x.tick(0.6);
        assert_eq!(
            x.noted(p),
            vec![4, 1],
            "ClearMe fires once of its two, then Ping still fires"
        );
        assert_eq!(x.timer_rate(p, "ClearMe"), -1.0);
        assert!(x.w.timers().borrow().of(p).is_empty());
    });
}

#[test]
fn a_non_loop_timer_re_set_by_its_own_callback_is_kept() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 0.5, false, "Rearm", None);
        x.tick(1.0);
        assert_eq!(x.noted(p), vec![3]);
        assert_eq!(
            (x.timer_rate(p, "Rearm"), x.timer_count(p, "Rearm")),
            (1.0, 0.0),
            "re-set: kept, Count 0"
        );
        x.tick(1.5);
        assert_eq!(x.noted(p), vec![3, 3]);
    });
}

#[test]
fn a_callback_that_destroys_the_actor_stops_the_pass() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 0.1, false, "Kill", None);
        x.set_timer(p, 0.1, false, "Ping", None);
        x.tick(0.5);
        assert_eq!(
            x.noted(p),
            vec![5, 14],
            "Kill, its Destroyed event; Ping never fires"
        );
        assert!(crate::actors::is_destroyed(&x.w.vm.space, p).unwrap());
        assert!(
            x.w.timers().borrow().of(p).is_empty(),
            "destroy drops the actor's timer entries"
        );
    });
}

#[test]
fn the_pass_culls_cleared_missing_and_orphaned_entries_and_visits_appended_ones() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        let other = x.spawn("Probe");
        x.set_timer(p, 0.1, false, "NoSuchFunction", None);
        x.set_timer(p, 0.0, false, "Ping", None);
        x.set_timer(p, 0.1, false, "Pong", Some(other));
        x.set_timer(p, 0.1, false, "Append", None);
        let log_before = x.w.vm.log.len();
        x.tick(0.5);
        assert_eq!(
            x.noted(other),
            vec![2],
            "the callback runs on the timer's object"
        );
        assert_eq!(x.noted(p), vec![6]);
        assert!(
            x.w.timers().borrow().of(p).is_empty(),
            "missing function, rate 0 and the appended rate-0 entry all gone"
        );
        assert_eq!(
            x.w.vm.log.len(),
            log_before,
            "a missing timer function is silent"
        );

        x.set_timer(p, 0.1, false, "Ping", Some(other));
        destroy(&mut x.w.vm, other).unwrap();
        x.clear_notes();
        x.tick(0.5);
        assert!(
            x.all_notes().iter().all(|(_, k)| *k == 10),
            "a destroyed target removes the entry unfired"
        );
        assert!(x.w.timers().borrow().of(p).is_empty());

        x.set_timer(p, 0.1, false, "AppendPing", None);
        x.tick(0.5);
        let left = x.w.timers().borrow().of(p).to_vec();
        assert_eq!(
            left.len(),
            1,
            "the appended Ping is visited (not due) and kept"
        );
        assert_eq!((left[0].rate, left[0].count), (0.5, 0.0));
    });
}

#[test]
fn callbacks_get_zeroed_parameters_and_static_actors_ignore_set_timer() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 0.1, false, "WithArg", None);
        x.tick(0.5);
        assert_eq!(x.noted(p), vec![0]);
        set_field(&mut x.w.vm.space, p, "bStatic", Value::Bool(true)).unwrap();
        x.set_timer(p, 0.1, false, "Ping", None);
        assert!(x.w.timers().borrow().of(p).is_empty());
    });
}

#[test]
fn a_due_timer_with_a_negative_rate_is_not_yet() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 0.05, false, "Pong", None);
        x.set_timer(p, -1.0, false, "Ping", None);
        assert_eq!(x.timer_rate(p, "Ping"), -1.0, "stored as given");
        assert!(matches!(
            missing(x.w.tick(0.1).unwrap_err()),
            Missing::NotRead
        ));
        // Refused before the pass changed anything: Pong (first, due) neither grew nor fired.
        assert_eq!(x.noted(p), Vec::<i32>::new());
        assert_eq!(x.timer_count(p, "Pong"), 0.0);
        // The same before step 1 for a loop that would come due more than i32::MAX times.
        let mut y = fx();
        let q = y.spawn("Probe");
        y.set_timer(q, 1e-30, true, "Ping", None);
        assert!(matches!(
            missing(y.w.tick(0.1).unwrap_err()),
            Missing::NotRead
        ));
        assert_eq!(y.timer_count(q, "Ping"), 0.0);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn an_actor_tick_runs_tick_then_timers_then_life_span() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        x.set_timer(p, 0.1, false, "Ping", None);
        set_field(&mut x.w.vm.space, p, "LifeSpan", Value::Float(0.3)).unwrap();
        x.clear_notes();
        x.tick(0.2);
        assert_eq!(
            x.all_notes(),
            vec![(p, 10), (p, 1)],
            "Tick event, then the timer"
        );
        assert_eq!(
            field(&x.w.vm.space, p, "LifeSpan").unwrap(),
            Value::Float(0.3f32 - 0.2)
        );
        x.tick(0.2);
        assert_eq!(x.noted(p), vec![1, 14], "LifeSpan ran out: destroyed");
        assert!(!x.w.actors().borrow().list().contains(&p));
    });
}

#[test]
fn actors_tick_by_tick_group_then_list_order_and_role_none_is_not_ticked() {
    run(|| {
        let mut x = fx();
        let late = x.spawn("Probe");
        let early = x.spawn("Probe");
        let idle = x.spawn("Probe");
        set_field(&mut x.w.vm.space, late, "TickGroup", Value::Byte(2)).unwrap();
        set_field(&mut x.w.vm.space, idle, "Role", Value::Byte(0)).unwrap();
        x.set_timer(idle, 0.01, false, "Ping", None);
        x.clear_notes();
        x.tick(0.1);
        assert_eq!(x.all_notes(), vec![(early, 10), (late, 10)]);
        set_field(&mut x.w.vm.space, late, "TickGroup", Value::Byte(5)).unwrap();
        assert!(
            x.w.tick(0.1).is_err(),
            "TickGroup 5 is not a group (TG_MAX)"
        );
    });
}

#[test]
fn the_tick_writes_world_time_and_copies_the_time_dilation() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        set_field(&mut x.w.vm.space, wi, "TimeDilation", Value::Float(0.5)).unwrap();
        let step = x.w.tick(0.2).unwrap();
        assert_eq!(step.dt, 0.2f32 * 0.5);
        assert_eq!(
            field(&x.w.vm.space, wi, "TimeSeconds").unwrap(),
            Value::Float(0.1)
        );
        assert_eq!(
            field(&x.w.vm.space, wi, "RealTimeSeconds").unwrap(),
            Value::Float(0.2)
        );
        // The frame's dilated delta (lane W5-MAPLOAD).
        assert_eq!(
            field(&x.w.vm.space, wi, "DeltaSeconds").unwrap(),
            Value::Float(0.1)
        );
        let p = x.spawn("Probe");
        assert_eq!(
            field(&x.w.vm.space, p, "CreationTime").unwrap(),
            Value::Float(0.1)
        );
        set_field(&mut x.w.vm.space, wi, "TimeDilation", Value::Float(-1.0)).unwrap();
        assert!(matches!(
            x.w.tick(0.1).unwrap_err(),
            WorldError::Clock(ClockError::BadTimeDilation(_))
        ));
        assert!(matches!(
            x.w.tick(f32::NAN).unwrap_err(),
            WorldError::BadDelta(_)
        ));
    });
}

#[test]
fn identical_delta_sequences_replay_identically() {
    run(|| {
        let deltas = [0.016f32, 0.033, 0.5, 0.0, 0.1];
        let mut out = Vec::new();
        for _ in 0..2 {
            let mut x = fx();
            let p = x.spawn("Probe");
            x.set_timer(p, 0.05, true, "Ping", None);
            for d in deltas {
                x.tick(d);
            }
            let wi = x.wi;
            out.push((
                field(&x.w.vm.space, wi, "TimeSeconds").unwrap(),
                x.noted(p).len(),
                x.timer_count(p, "Ping").to_bits(),
            ));
        }
        assert_eq!(out[0], out[1]);
    });
}

#[test]
fn state_code_and_physics_wait_for_their_layers() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        let s = x.name("Busy");
        x.w.vm.space.heap.get_mut(p).unwrap().state = Some(s);
        assert!(matches!(
            missing(x.w.tick(0.1).unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        let seen = Rc::new(RefCell::new(0));
        let s2 = seen.clone();
        x.w.set_state_step(Rc::new(move |_, _, _| {
            *s2.borrow_mut() += 1;
            Ok(())
        }))
        .unwrap();
        x.tick(0.1);
        assert_eq!(*seen.borrow(), 1);
        x.w.vm.space.heap.get_mut(p).unwrap().state = None;
        set_field(&mut x.w.vm.space, p, "Physics", Value::Byte(2)).unwrap();
        assert!(matches!(
            missing(x.w.tick(0.1).unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        // Role 0 in PHYS_Walking gets no physics step; in PHYS_Falling it does.
        set_field(&mut x.w.vm.space, p, "Role", Value::Byte(0)).unwrap();
        set_field(&mut x.w.vm.space, p, "Physics", Value::Byte(1)).unwrap();
        x.tick(0.1);
        set_field(&mut x.w.vm.space, p, "Physics", Value::Byte(2)).unwrap();
        assert!(matches!(
            missing(x.w.tick(0.1).unwrap_err()),
            Missing::LayerAbsent(_)
        ));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Spawn and destroy
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn spawn_sets_the_fields_then_sends_the_events_in_the_record_order() {
    run(|| {
        let mut x = fx();
        let owner = x.spawn("Probe");
        let instigator = x.spawn("Probe");
        let wi = x.wi;
        set_field(
            &mut x.w.vm.space,
            wi,
            "Instigator",
            Value::Object(Some(instigator)),
        )
        .unwrap();
        let c = x.class("Probe");
        x.clear_notes();
        let a = spawn(
            &mut x.w.vm,
            wi,
            &SpawnRequest::new(Some(c)).with_owner(Some(owner)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(x.all_notes(), vec![(owner, 15), (a, 11), (a, 12), (a, 13)]);
        let space = &x.w.vm.space;
        assert_eq!(
            field(space, a, "Owner").unwrap(),
            Value::Object(Some(owner))
        );
        assert_eq!(
            field(space, a, "Instigator").unwrap(),
            Value::Object(Some(instigator)),
            "the spawner's"
        );
        assert_eq!(
            field(space, a, "WorldInfo").unwrap(),
            Value::Object(Some(wi))
        );
        assert_eq!(
            field(space, a, "Location").unwrap(),
            field(space, wi, "Location").unwrap()
        );
        assert_eq!(x.w.actors().borrow().list().last(), Some(&a));
    });
}

#[test]
fn spawn_refusals_return_none_and_collision_is_not_yet() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        assert_eq!(
            spawn(&mut x.w.vm, wi, &SpawnRequest::new(None)).unwrap(),
            None
        );
        let thing = x.class("Thing");
        assert_eq!(
            spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(thing))).unwrap(),
            None,
            "not an actor"
        );
        let dud = x.class("Dud");
        assert_eq!(
            spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(dud))).unwrap(),
            None,
            "destroyed in PreBeginPlay"
        );
        let probe = x.class("Probe");
        let other =
            x.w.vm
                .defs
                .class_default(&mut x.w.vm.space, "Engine.Dud")
                .unwrap();
        let req = SpawnRequest::new(Some(probe)).with_template(Some(other));
        assert_eq!(
            spawn(&mut x.w.vm, wi, &req).unwrap(),
            None,
            "a template of another class"
        );
        let cdo =
            x.w.vm
                .defs
                .class_default(&mut x.w.vm.space, "Engine.Probe")
                .unwrap();
        set_field(&mut x.w.vm.space, cdo, "bCollideActors", Value::Bool(true)).unwrap();
        let before = x.w.actors().borrow().list().len();
        // Off decides the encroachment check on the live actor after PreBeginPlay (NATIVE_SPAWN_PLACEMENT.md section 6,
        // decision 2026-11-19-79): the actor is made, then the NotYet is terminal and prefixed.
        let e = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap_err();
        let VmError::NotYet(n) = e else { panic!("expected NotYet") };
        assert!(matches!(n.missing, Missing::LayerAbsent(_)));
        assert!(n.what.starts_with("stopped after partial writes: the post-spawn encroachment check"), "{n:?}");
        assert_eq!(x.w.actors().borrow().list().len(), before + 1, "made, past PreBeginPlay");
        set_field(&mut x.w.vm.space, cdo, "bCollideActors", Value::Bool(false)).unwrap();
        set_field(&mut x.w.vm.space, cdo, "bNoDelete", Value::Bool(true)).unwrap();
        assert_eq!(
            spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap(),
            None,
            "no-delete after play began"
        );
    });
}

/// An archetype of the class (an object of it other than the class default): in Off the actor is made from its
/// slots (decision 2026-11-19-44); under Reference it stays `NotYet { NotRead }`, as before.
#[test]
fn an_archetype_template_spawns_from_its_slots_in_off_only() {
    run(|| {
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut x = fx_in(true, mode);
            let wi = x.wi;
            let probe = x.class("Probe");
            let cdo =
                x.w.vm
                    .defs
                    .class_default(&mut x.w.vm.space, "Engine.Probe")
                    .unwrap();
            let archetype =
                x.w.vm
                    .space
                    .alloc(probe, "Probe_ToyArchetype", None, ow_object::heap::Origin::Runtime)
                    .unwrap();
            let slots = x.w.vm.space.heap.get(cdo).unwrap().slots.clone();
            x.w.vm.space.heap.get_mut(archetype).unwrap().slots = slots;
            set_field(&mut x.w.vm.space, archetype, "LifeSpan", Value::Float(7.0)).unwrap();
            let req = SpawnRequest::new(Some(probe)).with_template(Some(archetype));
            match mode {
                ParityMode::Off => {
                    let a = spawn(&mut x.w.vm, wi, &req).unwrap().unwrap();
                    assert_eq!(field(&x.w.vm.space, a, "LifeSpan").unwrap(), Value::Float(7.0));
                    assert_eq!(field(&x.w.vm.space, cdo, "LifeSpan").unwrap(), Value::Float(0.0));
                }
                ParityMode::Reference => {
                    let e = spawn(&mut x.w.vm, wi, &req).unwrap_err();
                    assert_eq!(vm_missing(e), Missing::NotRead);
                }
            }
        }
    });
}

/// The archetype's own collision flags count as the class defaults' do (review r1 of #435): an archetype that sets
/// `bCollideWorld` or (after play began) `bCollideActors` is refused with the collision `NotYet`, nothing made.
#[test]
fn an_archetype_that_sets_collision_flags_is_refused() {
    run(|| {
        for flag_name in ["bCollideWorld", "bCollideActors"] {
            let mut x = fx();
            let wi = x.wi;
            let probe = x.class("Probe");
            let archetype = toy_archetype(&mut x, probe, "Probe_ToyArchetype");
            set_field(&mut x.w.vm.space, archetype, flag_name, Value::Bool(true)).unwrap();
            let before = x.w.actors().borrow().list().len();
            let req = SpawnRequest::new(Some(probe)).with_template(Some(archetype));
            let e = spawn(&mut x.w.vm, wi, &req).unwrap_err();
            assert!(matches!(vm_missing(e), Missing::LayerAbsent(_)), "{flag_name}");
            // The spot search refuses before anything is made; in Off the encroachment check is decided on the live
            // actor after PreBeginPlay (decision 2026-11-19-79), so that actor is made.
            let made = usize::from(flag_name == "bCollideActors");
            assert_eq!(x.w.actors().borrow().list().len(), before + made, "{flag_name}");
        }
    });
}

/// An archetype whose properties are not loaded (a stand-in with no slots), and a live actor as the template, are
/// `NotYet { NotRead }` before anything is made (review r1 of #435).
#[test]
fn an_archetype_with_no_slots_or_a_live_actor_is_not_yet() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        let probe = x.class("Probe");
        let stand_in = x
            .w
            .vm
            .space
            .alloc_stand_in(probe, "Probe_StandIn", ow_object::heap::Origin::Resource { package: 0, index: 1 })
            .unwrap();
        let live = x.spawn("Probe");
        for template in [stand_in, live] {
            let before = x.w.actors().borrow().list().len();
            let req = SpawnRequest::new(Some(probe)).with_template(Some(template));
            let e = spawn(&mut x.w.vm, wi, &req).unwrap_err();
            assert_eq!(vm_missing(e), Missing::NotRead);
            assert_eq!(x.w.actors().borrow().list().len(), before, "nothing made");
        }
    });
}

/// An Off spawn from an archetype that has a component: the actor gets its own copy, outered to it and holding the
/// archetype's values; the archetype keeps its own (decision 2026-11-19-44).
#[test]
fn an_off_spawn_from_an_archetype_copies_its_components() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let toolbox = x.class("Toolbox");
        let archetype = toy_archetype(&mut x, toolbox, "Toolbox_ToyArchetype");
        let component_class = x.class("ToyComponent");
        let component = x
            .w
            .vm
            .space
            .alloc(component_class, "ToyComponent0", Some(archetype), ow_object::heap::Origin::Runtime)
            .unwrap();
        set_field(&mut x.w.vm.space, component, "Payload", Value::Int(42)).unwrap();
        set_field(&mut x.w.vm.space, archetype, "Comp", Value::Object(Some(component))).unwrap();
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(archetype));
        let actor = spawn(&mut x.w.vm, wi, &req).unwrap().unwrap();
        let Value::Object(Some(copy)) = field(&x.w.vm.space, actor, "Comp").unwrap() else {
            panic!("no component");
        };
        assert_ne!(copy, component);
        assert_eq!(x.w.vm.space.heap.get(copy).unwrap().outer, Some(actor));
        assert_eq!(field(&x.w.vm.space, copy, "Payload").unwrap(), Value::Int(42));
        assert_eq!(field(&x.w.vm.space, archetype, "Comp").unwrap(), Value::Object(Some(component)));
    });
}

/// The spawn's component update (lane IMPL-SPAWN-COMPONENT-ATTACH, NATIVE_SPAWN_COMPONENT_ATTACH.md): in Off, before
/// and after play began, the spawned actor's own copy in `Components` gets the actor as `Owner` and both update flags
/// cleared; `ToyComponent` is not an AI component class, so its `bAttached` and the actor's `AllComponents` stay as
/// they were. The standard Off world logged its empty scene layer as a harness input; a Reference world has none.
#[test]
fn an_off_spawn_gives_its_components_the_actor_as_owner() {
    run(|| {
        // Reference installs no layer and logs nothing (its archetype spawns are refused before any component).
        let reference = fx_files(true, ParityMode::Reference, true);
        assert!(reference.w.vm.service::<crate::actors::SceneLayer>().is_none());
        assert!(!reference.w.vm.log.iter().any(|l| l == crate::actors::component_attach::HARNESS_LINE));
        for (begin, mode) in [(true, ParityMode::Off), (false, ParityMode::Off)] {
            let mut x = fx_files(begin, mode, true);
            let wi = x.wi;
            let toolbox = x.class("Toolbox");
            let archetype = toy_archetype(&mut x, toolbox, "Toolbox_ToyArchetype");
            let component_class = x.class("ToyComponent");
            let component = x
                .w
                .vm
                .space
                .alloc(component_class, "ToyComponent0", Some(archetype), ow_object::heap::Origin::Runtime)
                .unwrap();
            for f in ["bNeedsReattach", "bNeedsUpdateTransform"] {
                set_field(&mut x.w.vm.space, component, f, Value::Bool(true)).unwrap();
            }
            set_field(&mut x.w.vm.space, archetype, "Comp", Value::Object(Some(component))).unwrap();
            // `Components` is typed with the fixture's own root class, which `ToyComponent` (a `Core.Component`) is not
            // under; the packages fill it through tags, so the test writes the slot as they do, unchecked.
            let p = x.w.vm.space.prop(toolbox, "Components").unwrap();
            *x.w.vm.space.slot_mut(archetype, ow_object::ids::PropAddr::scalar(p)).unwrap() =
                Value::array(vec![Value::Object(None), Value::Object(Some(component))]);
            let req = SpawnRequest::new(Some(toolbox)).with_template(Some(archetype));
            let actor = spawn(&mut x.w.vm, wi, &req).unwrap().unwrap();
            let Value::Object(Some(copy)) = field(&x.w.vm.space, actor, "Comp").unwrap() else {
                panic!("no component");
            };
            let label = format!("begin {begin}, {mode:?}");
            {
                assert_ne!(copy, component, "{label}");
                assert_eq!(field(&x.w.vm.space, copy, "Owner").unwrap(), Value::Object(Some(actor)), "{label}");
                for f in ["bNeedsReattach", "bNeedsUpdateTransform"] {
                    assert_eq!(field(&x.w.vm.space, copy, f).unwrap(), Value::Bool(false), "{label}: {f}");
                }
                assert_eq!(field(&x.w.vm.space, copy, "bAttached").unwrap(), Value::Bool(false), "{label}");
                assert_eq!(field(&x.w.vm.space, actor, "AllComponents").unwrap(), Value::array(vec![]), "{label}");
                assert_eq!(field(&x.w.vm.space, component, "Owner").unwrap(), Value::Object(None), "the archetype's own");
                assert!(x.w.vm.log.iter().any(|l| l == crate::actors::component_attach::HARNESS_LINE), "{label}");
                // The `Components` entry and the `Comp` slot name the same copy (the re-pointing of the instancing).
                assert_eq!(
                    field(&x.w.vm.space, actor, "Components").unwrap(),
                    Value::array(vec![Value::Object(None), Value::Object(Some(copy))]),
                    "{label}"
                );
            }
        }
    });
}

/// The spawn's component update refuses before the actor is made, so each refusal leaves the level list and the
/// heap as they were before the spawn, and the template's component untouched (NATIVE_SPAWN_COMPONENT_ATTACH.md
/// section 4 at bc83e3b6):
/// - an entry that is not this spawn's own copy (a component the archetype does not own, which the instancing
///   shares): `NotYet { NotRead }`;
/// - an attached template component: `NotYet { NotRead }`;
/// - a non-empty `AllComponents` on the source: `NotYet { NotRead }`;
/// - a component class without `Owner`: `VmError::Script`.
#[test]
fn the_spawn_s_component_update_refusals_leave_no_actor() {
    run(|| {
        for case in ["not a copy", "attached", "AllComponents", "no Owner"] {
            let mut x = fx_files(true, ParityMode::Off, true);
            let wi = x.wi;
            let toolbox = x.class("Toolbox");
            let archetype = toy_archetype(&mut x, toolbox, "Toolbox_ToyArchetype");
            let class = x.class(if case == "no Owner" { "ToyBareComponent" } else { "ToyComponent" });
            let outer = if case == "not a copy" { None } else { Some(archetype) };
            let component = x.w.vm.space.alloc(class, "ToyComponent0", outer, ow_object::heap::Origin::Runtime).unwrap();
            if case == "attached" {
                set_field(&mut x.w.vm.space, component, "bAttached", Value::Bool(true)).unwrap();
            }
            let list = Value::array(vec![Value::Object(Some(component))]);
            let components = x.w.vm.space.prop(toolbox, "Components").unwrap();
            *x.w.vm.space.slot_mut(archetype, ow_object::ids::PropAddr::scalar(components)).unwrap() = list.clone();
            if case == "AllComponents" {
                let all = x.w.vm.space.prop(toolbox, "AllComponents").unwrap();
                *x.w.vm.space.slot_mut(archetype, ow_object::ids::PropAddr::scalar(all)).unwrap() = list;
            }
            let (listed, objects) = (x.w.actors().borrow().list().len(), x.w.vm.space.heap.handles().len());
            let req = SpawnRequest::new(Some(toolbox)).with_template(Some(archetype));
            let e = spawn(&mut x.w.vm, wi, &req).unwrap_err();
            match (case, e) {
                ("no Owner", VmError::Script { message, .. }) => assert!(message.contains("declares no Owner"), "{message}"),
                ("not a copy", VmError::NotYet(n)) => {
                    assert_eq!(n.missing, Missing::NotRead);
                    assert!(n.what.contains(crate::actors::component_attach::NOT_A_COPY_WHY), "{n}");
                }
                ("attached", VmError::NotYet(n)) => {
                    assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, crate::actors::component_attach::ATTACHED));
                }
                ("AllComponents", VmError::NotYet(n)) => {
                    assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, crate::actors::component_attach::PRE_PASS));
                }
                (case, other) => panic!("{case}: unexpected {other:?}"),
            }
            assert_eq!(x.w.actors().borrow().list().len(), listed, "{case}: no actor listed");
            assert_eq!(x.w.vm.space.heap.handles().len(), objects, "{case}: no object made");
            if case != "no Owner" {
                assert_eq!(field(&x.w.vm.space, component, "Owner").unwrap(), Value::Object(None), "{case}");
            }
        }
    });
}

/// A destroyed actor, a pending-delete archetype and an archetype with an `Owner` are refused as templates with
/// `NotYet { NotRead }`, and so is an archetype holding a sub-object Off cannot copy; in each case nothing is made: the
/// heap and the level list keep their counts (review r2 of #435, items 4 to 6).
#[test]
fn templates_off_cannot_spawn_from_leave_nothing_behind() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let probe = x.class("Probe");
        let toolbox = x.class("Toolbox");
        let gone = x.spawn("Probe");
        assert!(destroy(&mut x.w.vm, gone).unwrap());
        let pending = toy_archetype(&mut x, probe, "Probe_Pending");
        set_field(&mut x.w.vm.space, pending, "bPendingDelete", Value::Bool(true)).unwrap();
        let deleted = toy_archetype(&mut x, probe, "Probe_Deleted");
        set_field(&mut x.w.vm.space, deleted, "bDeleteMe", Value::Bool(true)).unwrap();
        let owned = toy_archetype(&mut x, probe, "Probe_Owned");
        set_field(&mut x.w.vm.space, owned, "Owner", Value::Object(Some(wi))).unwrap();
        let holder = toy_archetype(&mut x, toolbox, "Toolbox_Holder");
        let thing_class = x.class("Object");
        let plain = x.w.vm.space.alloc(thing_class, "PlainThing", Some(holder), ow_object::heap::Origin::Runtime).unwrap();
        set_field(&mut x.w.vm.space, holder, "Thing", Value::Object(Some(plain))).unwrap();
        for (class, template, what) in [
            (probe, gone, "destroyed"),
            (probe, pending, "pending-delete"),
            (probe, deleted, "pending-delete"),
            (probe, owned, "Owner is set"),
            (toolbox, holder, "not a component"),
        ] {
            let (heap, list) = (x.w.vm.space.heap.len(), x.w.actors().borrow().list().len());
            let req = SpawnRequest::new(Some(class)).with_template(Some(template));
            match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
                VmError::NotYet(n) => {
                    assert_eq!(n.missing, Missing::NotRead, "{what}");
                    assert!(n.what.contains(what), "{what}: {n}");
                }
                e => panic!("{what}: expected NotYet, got {e}"),
            }
            assert_eq!(x.w.vm.space.heap.len(), heap, "{what}: nothing on the heap");
            assert_eq!(x.w.actors().borrow().list().len(), list, "{what}: nothing in the level list");
        }
    });
}

/// A component a parent class's default object owns, held by a template's slot (decision 2026-11-19-91), hand-made:
/// a `ToyComponent` named `ParentComp`, standing for that export of the fixture's package, outered to a hand-made
/// `Default__ToyParent`.
fn parent_default_component(x: &mut Fx, payload: i32) -> ObjRef {
    let path = ow_core::ObjectPath::parse("Engine.Toolbox_Arch").unwrap();
    let arch = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
    let ow_object::heap::Origin::Export { package, .. } = x.w.vm.space.heap.get(arch).unwrap().origin else {
        panic!("the fixture's archetype is an export");
    };
    let index = {
        let (code, store) = x.w.vm.defs.code_mut();
        code.find_export(store, package, "ParentComp")
    };
    assert!(index > 0, "the fixture exports ParentComp");
    let toolbox = x.class("Toolbox");
    let owner = x.w.vm.space.alloc(toolbox, "Default__ToyParent", None, ow_object::heap::Origin::Runtime).unwrap();
    let component_class = x.class("ToyComponent");
    let made = x
        .w
        .vm
        .space
        .alloc(component_class, "ParentComp", Some(owner), ow_object::heap::Origin::Export { package, index })
        .unwrap();
    set_field(&mut x.w.vm.space, made, "Payload", Value::Int(payload)).unwrap();
    made
}

/// A template whose slot names a parent class default's component (decision 2026-11-19-91): the template's own
/// sub-object whose export archetype is that component stands for it, and the spawned actor gets its own copy of that
/// sub-object, with the sub-object's values; the parent default's component and the template are left as they were.
#[test]
fn an_inherited_component_is_instanced_from_the_templates_own_sub_object() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let parent = parent_default_component(&mut x, 5);
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Inherit").unwrap();
        let template = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        set_field(&mut x.w.vm.space, template, "Comp", Value::Object(Some(parent))).unwrap();
        let plan = crate::actors::plan_components(&mut x.w.vm, template).unwrap();
        assert_eq!(plan.inherited.len(), 1, "{plan:?}");
        let (reference, own) = plan.inherited[0];
        assert_eq!(reference, parent);
        assert_eq!(x.w.vm.space.heap.get(own).unwrap().outer, Some(template), "the stand-in is the template's own");
        assert_eq!(plan.originals, vec![own]);
        let toolbox = x.class("Toolbox");
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(template));
        let actor = spawn(&mut x.w.vm, wi, &req).unwrap().unwrap();
        let Value::Object(Some(copy)) = field(&x.w.vm.space, actor, "Comp").unwrap() else {
            panic!("no component");
        };
        assert!(copy != parent && copy != own, "the actor's own copy");
        assert_eq!(x.w.vm.space.heap.get(copy).unwrap().outer, Some(actor));
        assert_eq!(field(&x.w.vm.space, copy, "Payload").unwrap(), Value::Int(9), "the template sub-object's values");
        assert_eq!(field(&x.w.vm.space, template, "Comp").unwrap(), Value::Object(Some(parent)), "the template unchanged");
        assert_eq!(field(&x.w.vm.space, parent, "Payload").unwrap(), Value::Int(5), "the parent default's unchanged");
        // A second actor gets another copy.
        let other = spawn(&mut x.w.vm, wi, &req).unwrap().unwrap();
        let Value::Object(Some(second)) = field(&x.w.vm.space, other, "Comp").unwrap() else {
            panic!("no component");
        };
        assert_ne!(second, copy);
    });
}

/// A template with no sub-object made from the parent default's component keeps the refusal, with nothing made.
#[test]
fn an_inherited_component_without_a_stand_in_is_refused() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let parent = parent_default_component(&mut x, 5);
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Bare").unwrap();
        let template = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        set_field(&mut x.w.vm.space, template, "Comp", Value::Object(Some(parent))).unwrap();
        let (heap, list) = (x.w.vm.space.heap.len(), x.w.actors().borrow().list().len());
        let toolbox = x.class("Toolbox");
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(template));
        match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.what.contains("with no sub-object of the template made from it"), "{n}");
                assert!(n.record.contains("2026-11-19-91"), "{n}");
            }
            e => panic!("expected NotYet, got {e}"),
        }
        assert_eq!(x.w.vm.space.heap.len(), heap, "nothing on the heap");
        assert_eq!(x.w.actors().borrow().list().len(), list, "nothing in the level list");
    });
}

/// A refusal decided from the template's own slots comes before any load (review r3 of #435, item 3): the archetype
/// export's lazy component is still not loaded, and nothing was logged, when its reference back to itself is refused.
#[test]
fn a_refusal_from_the_templates_slots_loads_nothing() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Arch").unwrap();
        let arch = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        x.w.vm.log.extend(x.w.vm.defs.take_log());
        let Value::Object(Some(comp)) = field(&x.w.vm.space, arch, "Comp").unwrap() else {
            panic!("no component");
        };
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, comp), "the fixture's component is lazy");
        let log = x.w.vm.log.len();
        match crate::actors::plan_components(&mut x.w.vm, arch).unwrap_err() {
            VmError::NotYet(n) => assert!(n.what.contains("back to the template"), "{n}"),
            e => panic!("expected NotYet, got {e}"),
        }
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, comp), "still not loaded");
        assert_eq!(x.w.vm.log.len(), log, "nothing logged");
    });
}

/// A level list another caller holds refuses the spawn before anything is made (review r3 of #435, item 2), also on
/// the path that would allocate the actor and its copies.
#[test]
fn a_held_level_list_refuses_before_anything_is_made() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let toolbox = x.class("Toolbox");
        let archetype = toy_archetype(&mut x, toolbox, "Toolbox_Held");
        let component_class = x.class("ToyComponent");
        let component = x
            .w
            .vm
            .space
            .alloc(component_class, "ToyComponent0", Some(archetype), ow_object::heap::Origin::Runtime)
            .unwrap();
        set_field(&mut x.w.vm.space, archetype, "Comp", Value::Object(Some(component))).unwrap();
        let actors = x.w.actors();
        let held = actors.borrow();
        let heap = x.w.vm.space.heap.len();
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(archetype));
        assert!(spawn(&mut x.w.vm, wi, &req).is_err());
        assert_eq!(x.w.vm.space.heap.len(), heap, "nothing on the heap");
        drop(held);
        assert!(spawn(&mut x.w.vm, wi, &req).unwrap().is_some(), "the same spawn once the list is free");
    });
}

/// A plain spawn from the class defaults, the slice gate's cast actors' (decision 2026-11-19-70): each actor gets its
/// own copy of the class default's component, outered to it, and two actors get two copies; the class default keeps
/// its own.
#[test]
fn a_plain_spawn_gives_each_actor_its_own_class_default_component() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let toolbox = x.class("Toolbox");
        let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Toolbox").unwrap();
        let component_class = x.class("ToyComponent");
        let component = x
            .w
            .vm
            .space
            .alloc(component_class, "ToyComponent0", Some(cdo), ow_object::heap::Origin::Runtime)
            .unwrap();
        set_field(&mut x.w.vm.space, component, "Payload", Value::Int(9)).unwrap();
        set_field(&mut x.w.vm.space, cdo, "Comp", Value::Object(Some(component))).unwrap();
        let mut copies = Vec::new();
        for _ in 0..2 {
            let actor = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(toolbox))).unwrap().unwrap();
            let Value::Object(Some(copy)) = field(&x.w.vm.space, actor, "Comp").unwrap() else {
                panic!("no component");
            };
            assert_ne!(copy, component);
            assert_eq!(x.w.vm.space.heap.get(copy).unwrap().outer, Some(actor));
            assert_eq!(field(&x.w.vm.space, copy, "Payload").unwrap(), Value::Int(9));
            copies.push(copy);
        }
        assert_ne!(copies[0], copies[1], "each actor its own");
        assert_eq!(field(&x.w.vm.space, cdo, "Comp").unwrap(), Value::Object(Some(component)));
    });
}

/// An archetype of `class`: a new object with the class defaults' slots.
fn toy_archetype(x: &mut Fx, class: ow_object::ids::ClassId, name: &str) -> ObjRef {
    let path = x.w.vm.space.classes.get_layout(class).unwrap().path().as_str().to_string();
    let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, &path).unwrap();
    let archetype = x.w.vm.space.alloc(class, name, None, ow_object::heap::Origin::Runtime).unwrap();
    let slots = x.w.vm.space.heap.get(cdo).unwrap().slots.clone();
    x.w.vm.space.heap.get_mut(archetype).unwrap().slots = slots;
    archetype
}

#[test]
fn before_play_begins_spawn_sends_no_begin_play_events() {
    run(|| {
        let mut x = fx_with(false);
        let cdo =
            x.w.vm
                .defs
                .class_default(&mut x.w.vm.space, "Engine.Probe")
                .unwrap();
        for b in ["bCollideActors", "bCollideWorld", "bShouldBaseAtStartup"] {
            set_field(&mut x.w.vm.space, cdo, b, Value::Bool(true)).unwrap();
        }
        let c = x.class("Probe");
        let wi = x.wi;
        let req = SpawnRequest::new(Some(c)).with_no_collision_fail(true);
        let p = spawn(&mut x.w.vm, wi, &req).unwrap().unwrap();
        assert_eq!(x.noted(p), Vec::<i32>::new());
        assert!(
            x.all_notes().is_empty(),
            "no begin-play event before play begins"
        );
        // Before play the encroachment check and the base search do not run, so neither refuses.
    });
}

#[test]
fn the_base_search_is_refused_before_the_actor_is_made() {
    run(|| {
        let mut x = fx();
        let cdo =
            x.w.vm
                .defs
                .class_default(&mut x.w.vm.space, "Engine.Probe")
                .unwrap();
        for b in ["bCollideWorld", "bShouldBaseAtStartup"] {
            set_field(&mut x.w.vm.space, cdo, b, Value::Bool(true)).unwrap();
        }
        let c = x.class("Probe");
        let wi = x.wi;
        let before = x.w.actors().borrow().list().len();
        x.clear_notes();
        let req = SpawnRequest::new(Some(c)).with_no_collision_fail(true);
        let e = spawn(&mut x.w.vm, wi, &req).unwrap_err();
        assert!(matches!(vm_missing(e), Missing::LayerAbsent(_)));
        assert_eq!(
            x.w.actors().borrow().list().len(),
            before,
            "nothing made or listed"
        );
        assert!(x.all_notes().is_empty(), "no event sent");
        // In PHYS_Walking no base search is needed and the spawn goes through.
        set_field(&mut x.w.vm.space, cdo, "Physics", Value::Byte(1)).unwrap();
        assert!(spawn(&mut x.w.vm, wi, &req).unwrap().is_some());
    });
}

#[test]
fn each_actor_ticks_once_per_tick_and_a_spawned_one_waits_for_the_next() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        let cdo =
            x.w.vm
                .defs
                .class_default(&mut x.w.vm.space, "Engine.Probe")
                .unwrap();
        set_field(&mut x.w.vm.space, cdo, "TickGroup", Value::Byte(2)).unwrap();
        let s = x.name("Busy");
        x.w.vm.space.heap.get_mut(p).unwrap().state = Some(s);
        let spawned: Rc<RefCell<Option<ObjRef>>> = Rc::new(RefCell::new(None));
        let out = spawned.clone();
        let (probe, wi) = (x.class("Probe"), x.wi);
        // p's state step (after its Tick event) moves p to group 2 and spawns a Probe into group 2, once.
        x.w.set_state_step(Rc::new(move |vm, a, _| {
            set_field(&mut vm.space, a, "TickGroup", Value::Byte(2))?;
            if out.borrow().is_none() {
                let n = spawn(vm, wi, &SpawnRequest::new(Some(probe)))?;
                *out.borrow_mut() = n;
            }
            Ok(())
        }))
        .unwrap();
        x.clear_notes();
        x.tick(0.1);
        let fresh = spawned.borrow().expect("spawned");
        let ticks = |x: &Fx, o: ObjRef| {
            x.all_notes()
                .iter()
                .filter(|(a, k)| *a == o && *k == 10)
                .count()
        };
        assert_eq!(
            ticks(&x, p),
            1,
            "moved from group 0 to 2 during its tick: still one tick"
        );
        assert_eq!(
            ticks(&x, fresh),
            0,
            "spawned during the tick: not ticked in it"
        );
        x.clear_notes();
        x.tick(0.1);
        assert_eq!((ticks(&x, p), ticks(&x, fresh)), (1, 1));
    });
}

#[test]
fn physics_skips_a_simulated_proxy_and_an_actor_with_a_base_is_not_yet() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        let other = x.spawn("Probe");
        set_field(&mut x.w.vm.space, p, "Physics", Value::Byte(2)).unwrap();
        set_field(&mut x.w.vm.space, p, "Role", Value::Byte(2)).unwrap();
        x.tick(0.1); // Role 2: no physics step, so no NotYet without a physics hook
        set_field(&mut x.w.vm.space, p, "Role", Value::Byte(3)).unwrap();
        set_field(&mut x.w.vm.space, p, "Base", Value::Object(Some(other))).unwrap();
        let seen = Rc::new(RefCell::new(0));
        let s2 = seen.clone();
        x.w.set_physics_step(Rc::new(move |_, _, _| {
            *s2.borrow_mut() += 1;
            Ok(())
        }))
        .unwrap();
        assert!(matches!(
            missing(x.w.tick(0.1).unwrap_err()),
            Missing::NotRead
        ));
        assert_eq!(*seen.borrow(), 0, "the hook is not asked for a based actor");
        set_field(&mut x.w.vm.space, p, "Base", Value::Object(None)).unwrap();
        x.tick(0.1);
        assert_eq!(*seen.borrow(), 1);
    });
}

#[test]
fn destroy_refuses_static_and_non_authority_actors_and_runs_the_events() {
    run(|| {
        let mut x = fx();
        let owner = x.spawn("Probe");
        let c = x.class("Probe");
        let wi = x.wi;
        let a = spawn(
            &mut x.w.vm,
            wi,
            &SpawnRequest::new(Some(c)).with_owner(Some(owner)),
        )
        .unwrap()
        .unwrap();
        set_field(&mut x.w.vm.space, a, "Role", Value::Byte(2)).unwrap();
        assert!(!destroy(&mut x.w.vm, a).unwrap(), "below authority");
        set_field(&mut x.w.vm.space, a, "bNetTemporary", Value::Bool(true)).unwrap();
        set_field(&mut x.w.vm.space, a, "bNoDelete", Value::Bool(true)).unwrap();
        assert!(!destroy(&mut x.w.vm, a).unwrap(), "no-delete");
        set_field(&mut x.w.vm.space, a, "bNoDelete", Value::Bool(false)).unwrap();
        x.clear_notes();
        assert!(
            destroy(&mut x.w.vm, a).unwrap(),
            "a net-temporary actor below authority may be destroyed"
        );
        assert_eq!(
            x.all_notes(),
            vec![(a, 14), (owner, 16)],
            "Destroyed, then LostChild on the owner"
        );
        let space = &x.w.vm.space;
        assert_eq!(field(space, a, "bDeleteMe").unwrap(), Value::Bool(true));
        assert_eq!(
            field(space, a, "bPendingDelete").unwrap(),
            Value::Bool(true)
        );
        assert_eq!(field(space, a, "Owner").unwrap(), Value::Object(None));
        assert!(!x.w.actors().borrow().list().contains(&a));
        assert!(destroy(&mut x.w.vm, a).unwrap(), "already destroyed: true");
        let r = x.call("Actor.Destroy", Some(owner), Vec::new()).unwrap();
        assert_eq!(r, Value::Bool(true), "through the native");
    });
}

/// Lane IMPL-COMPONENT-BOUNDS (review r1 of #643, item 4): `Actor.Destroy` drops the destroyed actor's attach-record
/// entries, as owner and as component. A second actor stands in for a component here (it declares `Owner`, set by
/// the spawn).
#[test]
fn destroy_drops_the_actors_attach_record_entries() {
    run(|| {
        let mut x = fx();
        let owner = x.spawn("Probe");
        let c = x.class("Probe");
        let wi = x.wi;
        let part = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(c)).with_owner(Some(owner))).unwrap().unwrap();
        let held = x.spawn("Probe");
        crate::actors::publish_attach(&mut x.w.vm, part, owner).unwrap();
        assert!(crate::actors::attached_to(&x.w.vm, part, owner).unwrap(), "recorded");
        assert!(destroy(&mut x.w.vm, owner).unwrap());
        let layer = x.w.vm.service::<crate::actors::SceneLayer>().expect("the Off world's scene layer");
        assert_eq!(layer.borrow().recorded_owner(part), None, "the owner's entries are gone");
        assert!(!crate::actors::attached_to(&x.w.vm, part, owner).unwrap());
        // As the component: an entry naming the destroyed actor as the component goes too.
        set_field(&mut x.w.vm.space, held, "Owner", Value::Object(Some(part))).unwrap();
        crate::actors::publish_attach(&mut x.w.vm, held, part).unwrap();
        assert!(destroy(&mut x.w.vm, held).unwrap());
        assert_eq!(layer.borrow().recorded_owner(held), None, "the component's entry is gone");
    });
}

#[test]
fn destroy_is_not_yet_where_this_world_cannot_run_the_routine() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        let other = x.spawn("Probe");
        set_field(&mut x.w.vm.space, p, "Base", Value::Object(Some(other))).unwrap();
        assert!(matches!(
            vm_missing(destroy(&mut x.w.vm, p).unwrap_err()),
            Missing::NotRead
        ));
        set_field(&mut x.w.vm.space, p, "Base", Value::Object(None)).unwrap();
        set_field(
            &mut x.w.vm.space,
            p,
            "GeneratedEvents",
            Value::array(vec![Value::Object(Some(other))]),
        )
        .unwrap();
        assert!(matches!(
            vm_missing(destroy(&mut x.w.vm, p).unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        set_field(
            &mut x.w.vm.space,
            p,
            "GeneratedEvents",
            Value::array(Vec::new()),
        )
        .unwrap();
        let s = x.name("Busy");
        x.w.vm.space.heap.get_mut(p).unwrap().state = Some(s);
        assert!(matches!(
            vm_missing(destroy(&mut x.w.vm, p).unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        x.w.vm.space.heap.get_mut(p).unwrap().state = None;
        assert_eq!(
            field(&x.w.vm.space, p, "bPendingDelete").unwrap(),
            Value::Bool(false),
            "nothing changed"
        );
        assert!(destroy(&mut x.w.vm, p).unwrap());
    });
}

#[test]
fn a_controller_is_the_cxx_stand_in_in_reference_and_generic_in_off() {
    run(|| {
        // Off (decision 2026-11-19-92, review r1 of #472): an AI controller takes the generic routine, whose first
        // refusal is the actor step's unread notification, before any write.
        let mut x = fx();
        let ctrl = x.spawn("Controller");
        match destroy(&mut x.w.vm, ctrl).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert_eq!(n.what, crate::actors::NOTIFICATION_NOT_READ);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(field(&x.w.vm.space, ctrl, "bPendingDelete").unwrap(), Value::Bool(false));
        assert!(x.w.actors().borrow().list().contains(&ctrl), "still listed");

        // Reference: the C++ stand-in, marked and true; no event, still listed; a second destroy is true.
        let mut x = fx_in(true, ParityMode::Reference);
        let ctrl = x.spawn("Controller");
        let notes_before = x.notes.borrow().len();
        assert!(destroy(&mut x.w.vm, ctrl).unwrap());
        assert_eq!(field(&x.w.vm.space, ctrl, "bDeleteMe").unwrap(), Value::Bool(true));
        assert_eq!(field(&x.w.vm.space, ctrl, "bPendingDelete").unwrap(), Value::Bool(false));
        assert_eq!(x.notes.borrow().len(), notes_before, "no script event");
        assert!(x.w.actors().borrow().list().contains(&ctrl), "the stand-in removes nothing");
        assert!(destroy(&mut x.w.vm, ctrl).unwrap(), "already destroyed: true");
        // Other actors keep the world's routine in Reference too.
        let p = x.spawn("Probe");
        assert!(destroy(&mut x.w.vm, p).unwrap());
        assert!(!x.w.actors().borrow().list().contains(&p));
    });
}

#[test]
fn set_owner_refuses_a_loop_and_sends_lost_then_gained_child() {
    run(|| {
        let mut x = fx();
        let a = x.spawn("Probe");
        let b = x.spawn("Probe");
        let c = x.spawn("Probe");
        set_owner(&mut x.w.vm, b, Some(a)).unwrap();
        set_owner(&mut x.w.vm, c, Some(b)).unwrap();
        x.clear_notes();
        set_owner(&mut x.w.vm, a, Some(c)).unwrap();
        assert_eq!(
            field(&x.w.vm.space, a, "Owner").unwrap(),
            Value::Object(None),
            "a loop is refused"
        );
        assert!(x.all_notes().is_empty());
        set_owner(&mut x.w.vm, c, Some(a)).unwrap();
        assert_eq!(x.all_notes(), vec![(b, 16), (a, 15)]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Iterators, world natives, services
// ---------------------------------------------------------------------------------------------------------------

/// Runs the engine iterator `key` with BaseClass `base`, as a `foreach` would.
fn iterate(
    vm: &mut Vm,
    path: &str,
    key: &str,
    base: Option<ow_object::ids::ClassId>,
) -> Vec<Value> {
    let f = match vm.find_function(path) {
        Ok(f) => f,
        Err(_) => vm
            .declare_function(FunctionDecl::native(
                ow_core::ObjectPath::parse(path).unwrap(),
                key,
            ))
            .unwrap(),
    };
    let it = vm.iterator_for(f).expect("an iterator is bound");
    let args = vec![
        Arg::new(Value::Class(base), false, true),
        Arg::new(Value::Object(None), true, true),
    ];
    it(&mut NativeCall::new(vm, f, None, args)).unwrap().items
}

#[test]
fn the_actor_iterators_walk_the_level_list() {
    run(|| {
        // A static actor placed by the map before play, then two spawned ones and a non-actor object.
        let mut y = fx_with(false);
        let static_one = {
            let c = y.class("Probe");
            let o = ow_vm::interp::instantiate_from_defaults(&mut y.w.vm, c).unwrap();
            set_field(&mut y.w.vm.space, o, "bStatic", Value::Bool(true)).unwrap();
            o
        };
        y.w.actors().borrow_mut().place(static_one).unwrap();
        y.w.actors().borrow_mut().set_first_dynamic(1).unwrap();
        y.w.actors().borrow_mut().begin_play();
        let a = y.spawn("Probe");
        let b = y.spawn("Probe");
        let thing = y.class("Thing");
        let stray = y.w.vm.instantiate(thing, "").unwrap();
        assert!(!y.w.vm.space.heap.get(stray).unwrap().destroyed);
        destroy(&mut y.w.vm, a).unwrap();
        let wi = y.wi;
        let all = iterate(
            &mut y.w.vm,
            "Engine.Actor.AllActors",
            "Actor.AllActors(class,object,class)",
            None,
        );
        assert_eq!(
            all,
            vec![
                Value::Object(Some(wi)),
                Value::Object(Some(static_one)),
                Value::Object(Some(b))
            ],
            "a None BaseClass is every actor; destroyed skipped; non-actors are not in the level"
        );
        let probe = y.class("Probe");
        let probes = iterate(
            &mut y.w.vm,
            "Engine.Actor.AllActors",
            "Actor.AllActors(class,object,class)",
            Some(probe),
        );
        assert_eq!(
            probes,
            vec![Value::Object(Some(static_one)), Value::Object(Some(b))]
        );
        let dynamic = iterate(
            &mut y.w.vm,
            "Engine.Actor.DynamicActors",
            "Actor.DynamicActors(class,object,class)",
            None,
        );
        assert_eq!(
            dynamic,
            vec![Value::Object(Some(b))],
            "from the first dynamic index, static skipped"
        );
    });
}

#[test]
fn evaluate_initialization_data_runs_the_evaluator() {
    run(|| {
        let mut x = fx();
        x.class("AttributeInitializationDefinition");
        let s =
            x.w.vm
                .space
                .classes
                .struct_by_path("Engine.Object.AttributeInitializationData")
                .unwrap();
        let data = Value::structure(
            s,
            vec![
                Value::Float(3.0),
                Value::Object(None),
                Value::Object(None),
                Value::Float(2.0),
            ],
        );
        let r = x
            .call(
                "AttributeInitializationDefinition.EvaluateInitializationData",
                None,
                vec![data, Value::Object(None)],
            )
            .unwrap();
        assert_eq!(
            r,
            Value::Float(6.0),
            "BaseValueConstant times BaseValueScaleConstant"
        );
    });
}

/// `WorldInfo.IsPlayInEditor` (decision 2026-11-16-81): false outside Reference (the game never runs in the
/// editor); unbound under Reference, as in the C++.
#[test]
fn is_play_in_editor_is_false_outside_reference() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        assert_eq!(x.call("WorldInfo.IsPlayInEditor", Some(wi), Vec::new()).unwrap(), Value::Bool(false));
        let x = fx_in(true, ParityMode::Reference);
        assert!(!x.w.vm.has_native("WorldInfo.IsPlayInEditor()"), "unbound under Reference");
    });
}

/// `WillowGameInfo.IsEditor` (lane IMPL-IS-EDITOR, the hub's ruling): false in Off (the port always runs the game, never
/// the editor), with nothing read or written; unbound under Reference, which keeps the stub policy.
#[test]
fn is_editor_is_false_in_off_and_unbound_in_reference() {
    run(|| {
        let mut x = fx();
        let c = x.w.vm.find_class("Engine.WillowGameInfo").unwrap();
        let game = x.w.vm.space.alloc(c, "ToyGame", None, ow_object::heap::Origin::Runtime).unwrap();
        let before = format!("{:?}", x.w.vm.space.heap);
        assert_eq!(x.call("WillowGameInfo.IsEditor", Some(game), Vec::new()).unwrap(), Value::Bool(false));
        assert_eq!(format!("{:?}", x.w.vm.space.heap), before, "nothing written");
        let x = fx_in(true, ParityMode::Reference);
        assert!(!x.w.vm.has_native(crate::world::IS_EDITOR), "unbound under Reference");
    });
}

/// `WorldInfo.AllNavigationPoints` (lane IMPL-ALL-NAVIGATION-POINTS): `World::new` binds the iterator in Off and not
/// under Reference. A second registration of the key is refused only where the world bound it.
#[test]
fn all_navigation_points_is_bound_in_off_only() {
    run(|| {
        let empty = || -> ow_vm::api::IteratorFn { Rc::new(|_c: &mut NativeCall<'_>| Ok(ow_vm::api::IteratorResult::default())) };
        let mut x = fx();
        assert!(x.w.vm.register_iterator(crate::world::nav_points::KEY, empty()).is_err(), "bound in Off");
        let mut x = fx_in(true, ParityMode::Reference);
        assert!(x.w.vm.register_iterator(crate::world::nav_points::KEY, empty()).is_ok(), "unbound under Reference");
    });
}

/// `SkeletalMeshComponent.ForceSkelUpdate()` and `UpdateAnimations()` (lane BIND-UPDATEANIMATIONS-FORCESKELUPDATE,
/// NATIVE_BONE_TRANSFORM_LAYER.md 6.4): `World::new` itself binds both keys in Off and neither under Reference, so
/// every slice world built through it (combat, equip, pickup, cast) has them.
#[test]
fn world_new_binds_force_skel_update_and_update_animations_in_off_only() {
    use crate::world::bone_transform::{FORCE_SKEL_UPDATE, UPDATE_ANIMATIONS};
    run(|| {
        let x = fx();
        assert!(x.w.vm.has_native(FORCE_SKEL_UPDATE), "ForceSkelUpdate bound in Off");
        assert!(x.w.vm.has_native(UPDATE_ANIMATIONS), "UpdateAnimations bound in Off");
        let x = fx_in(true, ParityMode::Reference);
        assert!(!x.w.vm.has_native(FORCE_SKEL_UPDATE), "ForceSkelUpdate unbound under Reference");
        assert!(!x.w.vm.has_native(UPDATE_ANIMATIONS), "UpdateAnimations unbound under Reference");
    });
}

/// `PlayerController.IsLocalPlayerController` (decision 2026-11-19-15, public UE3 semantics): a controller in the
/// world's local-player list is local; an unlisted one with no `Player` is not; an unlisted one with a `Player` the
/// world does not model is `NotYet`; a destroyed listed one is not. Without a world: `NotYet { LayerAbsent }`. Unbound
/// under Reference (the C++ stub).
#[test]
fn is_local_player_controller_follows_the_local_player_list() {
    run(|| {
        let mut x = fx();
        let (local, remote, odd) = (x.spawn("PlayerController"), x.spawn("PlayerController"), x.spawn("PlayerController"));
        x.w.actors().borrow_mut().add_local_player(local).unwrap();
        let wi = x.wi;
        set_field(&mut x.w.vm.space, odd, "Player", Value::Object(Some(wi))).unwrap();
        let ask = |x: &mut Fx, pc| x.call("PlayerController.IsLocalPlayerController", Some(pc), Vec::new());
        assert_eq!(ask(&mut x, local).unwrap(), Value::Bool(true));
        assert_eq!(ask(&mut x, remote).unwrap(), Value::Bool(false));
        let refused = ask(&mut x, odd).unwrap_err();
        assert!(matches!(&refused, VmError::NotYet(n) if n.missing == ow_core::Missing::NotPorted), "{refused}");
        // Destroying a controller is not yet ported in Off (NATIVE_ACTOR_DESTROY.md): mark it as the destroy would.
        set_field(&mut x.w.vm.space, local, "bDeleteMe", Value::Bool(true)).unwrap();
        assert_eq!(ask(&mut x, local).unwrap(), Value::Bool(false), "a destroyed controller");
        let x = fx_in(true, ParityMode::Reference);
        assert!(!x.w.vm.has_native("PlayerController.IsLocalPlayerController()"), "unbound under Reference");
    });
}

#[test]
fn is_local_player_controller_without_a_world_is_not_yet() {
    run(|| {
        let dir = std::env::temp_dir().join(format!("ow-world-bare-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let bytes = package();
        let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        let f = vm.find_function("Engine.PlayerController.IsLocalPlayerController").unwrap();
        let refused = super::is_local_player_controller(&mut NativeCall::new(&mut vm, f, None, Vec::new())).unwrap_err();
        assert!(matches!(&refused, VmError::NotYet(n) if matches!(n.missing, ow_core::Missing::LayerAbsent(_))), "{refused}");
        let _ = std::fs::remove_dir_all(&dir);
    });
}

/// `UIInteraction.GetDataStoreClient` is the world's (decision 2026-11-19-65; its rules are tested in
/// `datastore::tests`): bound outside Reference, unbound under Reference (R3). This fixture declares no
/// `Engine.DataStoreClient`, so the call is a typed error that leaves nothing on the heap.
#[test]
fn get_data_store_client_is_the_worlds_outside_reference() {
    run(|| {
        let mut x = fx();
        assert!(x.w.vm.has_native("UIInteraction.GetDataStoreClient()"));
        let heap = x.w.vm.space.heap.len();
        assert!(x.call("UIInteraction.GetDataStoreClient", None, Vec::new()).is_err());
        assert_eq!(x.w.vm.space.heap.len(), heap, "nothing made");
        let x = fx_in(true, ParityMode::Reference);
        assert!(!x.w.vm.has_native("UIInteraction.GetDataStoreClient()"), "unbound under Reference");
    });
}

/// The player pawn data manager (NATIVE_PAWN_DATA.md; decision 2026-11-19-24), Off: the getter reads the field and is
/// `NotYet` before the set-up; the set-up makes one manager and keeps it; an existing manager the set-up would have to
/// pick is `NotYet`.
#[test]
fn the_pawn_data_manager_is_read_from_the_globals_after_the_set_up() {
    run(|| {
        let mut x = fx();
        pawn_data::Pack.register(x.vm()).unwrap();
        let class = x.class("WillowGlobals");
        let globals = ow_vm::interp::instantiate_from_defaults(x.vm(), class).unwrap();
        let get = |x: &mut Fx| x.call("WillowGlobals.GetPlayerPawnDataManager", Some(globals), Vec::new());
        let before = get(&mut x);
        assert!(matches!(&before, Err(VmError::NotYet(n)) if n.missing == Missing::NotRead), "{before:?}");
        let made = pawn_data::set_up(x.vm(), globals).unwrap();
        assert_eq!(get(&mut x).unwrap(), Value::Object(Some(made)));
        assert_eq!(get(&mut x).unwrap(), Value::Object(Some(made)), "the same object on every call");
        assert_eq!(pawn_data::set_up(x.vm(), globals).unwrap(), made, "a set field is kept");
        // A second globals object with an empty field while a manager is alive: the engine check is not read.
        let other = ow_vm::interp::instantiate_from_defaults(x.vm(), class).unwrap();
        let refused = pawn_data::set_up(x.vm(), other).unwrap_err();
        assert!(matches!(&refused, VmError::NotYet(n) if n.missing == Missing::NotRead), "{refused}");
    });
}

/// `FindAlreadyLoadedObject`, Off: an object in memory at the full path (any case), None for nothing there, and
/// `NotYet { NotRead }` for a partial path and for an empty one.
#[test]
fn find_already_loaded_object_looks_up_full_paths_in_memory() {
    run(|| {
        let mut x = fx();
        pawn_data::Pack.register(x.vm()).unwrap();
        let globals_class = x.class("WillowGlobals");
        let globals = ow_vm::interp::instantiate_from_defaults(x.vm(), globals_class).unwrap();
        let manager = pawn_data::set_up(x.vm(), globals).unwrap();
        let thing = x.class("Thing");
        // A top-level package names its own path (Object.PathName); the toy package set has no Core, so declare it.
        let package = x.vm().space.classes.define("Core.Package", None, vec![]).unwrap();
        let outer = x.vm().space.alloc(package, "ToyPackage", None, ow_object::heap::Origin::Runtime).unwrap();
        let inner = x.vm().space.alloc(thing, "ToyArchetype", Some(outer), ow_object::heap::Origin::Runtime).unwrap();
        let find = |x: &mut Fx, p: &str| {
            x.call("WillowPlayerPawnDataManager.FindAlreadyLoadedObject", Some(manager), vec![Value::Str(p.into())])
        };
        assert_eq!(find(&mut x, "ToyPackage.ToyArchetype").unwrap(), Value::Object(Some(inner)));
        assert_eq!(find(&mut x, "toypackage.TOYARCHETYPE").unwrap(), Value::Object(Some(inner)), "names compare in any case");
        assert_eq!(find(&mut x, "ToyPackage.Nothing").unwrap(), Value::Object(None));
        for partial in ["ToyArchetype", ""] {
            let r = find(&mut x, partial);
            assert!(matches!(&r, Err(VmError::NotYet(n)) if n.missing == Missing::NotRead), "{partial:?}: {r:?}");
        }
    });
}

/// `FindAlreadyLoadedObject` and the store (review r1 of #411): a loaded definition is in memory; a store handle that
/// is not loaded, and a path an installed package holds that nothing has named yet, are `NotYet { NotRead }`; a path no
/// package holds is None. A run-time object with no outer that shares the name is skipped, not an error.
#[test]
fn find_already_loaded_object_asks_the_store_for_paths_not_in_memory() {
    run(|| {
        let mut x = fx();
        pawn_data::Pack.register(x.vm()).unwrap();
        let globals_class = x.class("WillowGlobals");
        let globals = ow_vm::interp::instantiate_from_defaults(x.vm(), globals_class).unwrap();
        let manager = pawn_data::set_up(x.vm(), globals).unwrap();
        let find = |x: &mut Fx, p: &str| {
            x.call("WillowPlayerPawnDataManager.FindAlreadyLoadedObject", Some(manager), vec![Value::Str(p.into())])
        };
        let not_read = |r: &Result<Value, VmError>| matches!(r, Err(VmError::NotYet(n)) if n.missing == Missing::NotRead);
        // Named by nothing yet: the store's export tables hold it.
        assert!(not_read(&find(&mut x, "Engine.ToyArchetype")), "a held path nothing has named");
        // Loaded: in memory. Its reference ToyTarget is now a handle the store has not loaded.
        let path = ow_core::ObjectPath::parse("Engine.ToyArchetype").unwrap();
        let vm = &mut x.w.vm;
        let loaded = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let target = ow_core::ObjectPath::parse("Engine.ToyTarget").unwrap();
        let handle = crate::actors::object(&x.w.vm.space, loaded, "Target").unwrap();
        assert!(handle.is_some_and(|h| x.w.vm.defs.is_unloaded(&x.w.vm.space, h)), "ToyTarget is an unloaded handle");
        assert_eq!(find(&mut x, "Engine.ToyArchetype").unwrap(), Value::Object(Some(loaded)));
        assert!(not_read(&find(&mut x, "engine.toytarget")), "an unloaded handle: {target:?}");
        // A run-time object with no outer of the same name is skipped but remembered: no package holds Engine.Nothing,
        // yet the game may hold that object at a path the port cannot name, so the answer is NotYet, not None.
        let thing = x.class("Thing");
        x.vm().space.alloc(thing, "Nothing", None, ow_object::heap::Origin::Runtime).unwrap();
        assert!(not_read(&find(&mut x, "Engine.Nothing")), "a skipped unnamed match is NotYet");
    });
}

/// Reference keeps both unbound (no C++ stand-in).
#[test]
fn the_pawn_data_natives_are_unbound_in_reference() {
    run(|| {
        let mut x = fx_in(true, ParityMode::Reference);
        pawn_data::Pack.register(x.vm()).unwrap();
        assert!(!x.w.vm.has_native(pawn_data::GET_MANAGER));
        assert!(!x.w.vm.has_native(pawn_data::FIND_LOADED));
    });
}

#[test]
fn get_world_info_and_the_services() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        assert_eq!(
            x.call("WorldInfo.GetWorldInfo", None, Vec::new()).unwrap(),
            Value::Object(Some(wi))
        );
        assert!(x.vm().service::<BalanceState>().is_some());
        assert_eq!(
            x.w.balance().borrow().snapshot.playthrough,
            Playthrough::NORMAL
        );
        let p = x.spawn("Probe");
        x.w.query().borrow_mut().set_globals(p);
        assert_eq!(ow_attr::api::AttrWorld::globals(x.vm()).unwrap(), p);
    });
}

/// Decision 2026-11-16-49: `World::new` makes the query follow its actor list, so with no game info set the game
/// info is the placed `WorldInfo`'s `Game`: `NotYet` while it is None, that object once set; a set game info wins.
#[test]
fn the_worlds_game_info_follows_its_placed_world_info() {
    run(|| {
        let mut x = fx();
        assert!(matches!(
            ow_attr::api::AttrWorld::game_info(x.vm()).unwrap_err(),
            ow_attr::api::AttrError::NotYet(_)
        ));
        let game = x.spawn("Probe");
        let wi = x.wi;
        set_field(&mut x.vm().space, wi, "Game", Value::Object(Some(game))).unwrap();
        assert_eq!(
            ow_attr::api::AttrWorld::game_info(x.vm()).unwrap(),
            Some(game)
        );
        let other = x.spawn("Probe");
        x.w.query().borrow_mut().set_game_info(other);
        assert_eq!(
            ow_attr::api::AttrWorld::game_info(x.vm()).unwrap(),
            Some(other)
        );
    });
}

#[test]
fn the_world_natives_without_a_world_are_not_yet() {
    run(|| {
        let mut x = fx();
        let p = x.spawn("Probe");
        // A bare VM over the same package, with the world's natives but no world services.
        let bytes = package();
        let dir = x.dir.join("bare");
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Engine.upk", &bytes)],
            IdentityPolicy::FirstInScanOrder,
            ParityMode::Off,
        )
        .unwrap();
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            ParityMode::Off,
        );
        Pack.register(&mut vm).unwrap();
        let c = vm.find_class("Engine.Probe").unwrap();
        let q = vm.instantiate(c, "").unwrap();
        let f = vm.find_function("Engine.Actor.ClearTimer").unwrap();
        assert!(matches!(
            vm_missing(vm.call(f, Some(q), Vec::new()).unwrap_err()),
            Missing::LayerAbsent(_)
        ));
        let _ = p;
    });
}


// ---------------------------------------------------------------------------------------------------------------
// The spawn's placement (lane IMPL-SPAWN-PLACEMENT, NATIVE_SPAWN_PLACEMENT.md at its review-r1 head, decision
// 2026-11-19-79). Invented geometry: a scripted service answering each query in turn.
// ---------------------------------------------------------------------------------------------------------------

mod placement {
    use super::*;
    use crate::placement::{
        BOUNDING_CYLINDER_OF, Bounds, ENCROACHMENT, Mode, RELOCATION, SPOT_SEARCH, SpawnCollision,
        Sweep, Test, install_collision,
    };

    /// One query the service was asked.
    #[derive(Debug, Clone, PartialEq)]
    enum Asked {
        Test([f32; 3], [f32; 3], Mode),
        BoxSweep([f32; 3], [f32; 3], [f32; 3]),
        PointSweep([f32; 3], [f32; 3]),
    }

    /// A scripted service: box tests and sweeps answer from their queues in order, then `rest` / Clear.
    struct Script {
        tests: RefCell<Vec<Test>>,
        sweeps: RefCell<Vec<Sweep>>,
        rest: Test,
        asked: RefCell<Vec<Asked>>,
        /// The bounds it reports for every live actor primitive (None: it holds none).
        bounds: Option<Bounds>,
    }

    impl SpawnCollision for Script {
        fn box_test(&self, extent: [f32; 3], at: [f32; 3], mode: Mode) -> Result<Test, VmError> {
            self.asked.borrow_mut().push(Asked::Test(extent, at, mode));
            let mut q = self.tests.borrow_mut();
            Ok(if q.is_empty() { self.rest } else { q.remove(0) })
        }
        fn box_sweep(&self, extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError> {
            self.asked.borrow_mut().push(Asked::BoxSweep(extent, start, end));
            let mut q = self.sweeps.borrow_mut();
            Ok(if q.is_empty() { Sweep::Clear } else { q.remove(0) })
        }
        fn point_sweep(&self, start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError> {
            self.asked.borrow_mut().push(Asked::PointSweep(start, end));
            Ok(Sweep::Clear)
        }
        fn actor_bounds(&self, _space: &ObjectSpace, _primitive: ObjRef) -> Result<Option<Bounds>, VmError> {
            Ok(self.bounds)
        }
    }

    fn script(x: &mut Fx, tests: Vec<Test>, sweeps: Vec<Sweep>, rest: Test) -> Rc<Script> {
        let s = Rc::new(Script {
            tests: RefCell::new(tests),
            sweeps: RefCell::new(sweeps),
            rest,
            asked: RefCell::new(Vec::new()),
            bounds: None,
        });
        install_collision(&mut x.w.vm, s.clone()).unwrap();
        s
    }

    /// A clear service that reports `bounds` for every live actor primitive.
    fn script_with_bounds(x: &mut Fx, bounds: Bounds) {
        let s = Rc::new(Script {
            tests: RefCell::new(vec![]),
            sweeps: RefCell::new(vec![]),
            rest: Test::Clear,
            asked: RefCell::new(Vec::new()),
            bounds: Some(bounds),
        });
        install_collision(&mut x.w.vm, s).unwrap();
    }

    /// A live `class` actor whose collision cylinder has the blocking flags `blocks`.
    fn blocker(x: &mut Fx, class: &str, world_geometry: bool, blocks: &[&str]) -> ObjRef {
        let actor = x.spawn(class);
        let c = x.class("CylinderComponent");
        let comp = ow_vm::interp::instantiate_from_defaults(&mut x.w.vm, c).unwrap();
        for b in blocks {
            set_field(&mut x.w.vm.space, comp, b, Value::Bool(true)).unwrap();
        }
        set_field(&mut x.w.vm.space, actor, "CollisionComponent", Value::Object(Some(comp))).unwrap();
        set_field(&mut x.w.vm.space, actor, "bWorldGeometry", Value::Bool(world_geometry)).unwrap();
        actor
    }

    /// The Probe class default collides with the world, with a cylinder of radius and half height (None: none).
    fn colliding_probe(x: &mut Fx, cylinder: Option<(f32, f32)>) -> ow_object::ids::ClassId {
        let probe = x.class("Probe");
        let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Probe").unwrap();
        set_field(&mut x.w.vm.space, cdo, "bCollideWorld", Value::Bool(true)).unwrap();
        let comp = cylinder.map(|(r, h)| {
            let c = x.class("CylinderComponent");
            let comp = ow_vm::interp::instantiate_from_defaults(&mut x.w.vm, c).unwrap();
            set_field(&mut x.w.vm.space, comp, "CollisionRadius", Value::Float(r)).unwrap();
            set_field(&mut x.w.vm.space, comp, "CollisionHeight", Value::Float(h)).unwrap();
            comp
        });
        set_field(&mut x.w.vm.space, cdo, "CollisionComponent", Value::Object(comp)).unwrap();
        probe
    }

    fn what(e: VmError) -> (Missing, String) {
        match e {
            VmError::NotYet(n) => (n.missing, n.what.into_owned()),
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    fn count(x: &Fx) -> usize {
        x.w.actors().borrow().list().len()
    }

    #[test]
    fn no_service_is_layer_absent_before_the_extent_question() {
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            let probe = colliding_probe(&mut x, None);
            let before = count(&x);
            let (missing, text) = what(spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap_err());
            assert!(matches!(missing, Missing::LayerAbsent(_)), "{missing:?}");
            assert_eq!(text, SPOT_SEARCH);
            assert_eq!(count(&x), before);
        });
    }

    #[test]
    fn no_cylinder_is_not_read_even_in_a_clear_service() {
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            script(&mut x, vec![], vec![], Test::Clear);
            let probe = colliding_probe(&mut x, None);
            let before = count(&x);
            let (missing, text) = what(spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap_err());
            // The probe's class is not one of the five whose rule was read (lane IMPL-LINK9-EXTENT).
            assert_eq!((missing, text), (Missing::NotRead, format!("{BOUNDING_CYLINDER_OF} Engine.Probe")));
            assert_eq!(count(&x), before);
        });
    }

    /// The `Actor` archetype `name` of the package (its components still lazy), set to collide with the world.
    fn extent_archetype(x: &mut Fx, name: &str) -> (ObjRef, Vec<Option<ObjRef>>) {
        let path = ow_core::ObjectPath::parse(&format!("Engine.{name}")).unwrap();
        let arch = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        x.w.vm.log.extend(x.w.vm.defs.take_log());
        set_field(&mut x.w.vm.space, arch, "bCollideWorld", Value::Bool(true)).unwrap();
        let entries = crate::actors::objects(&x.w.vm.space, arch, "Components").unwrap();
        assert_eq!(entries.len(), 3, "{entries:?}");
        for e in entries.iter().flatten() {
            assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, *e), "the fixture's components are lazy");
        }
        (arch, entries)
    }

    /// NATIVE_SPAWN_EXTENT.md section 5, step 5 (lane IMPL-LINK9-EXTENT), through the spawn: the primitive entry is
    /// loaded before the spot search, the None and non-primitive entries load nothing, and every `NotYet` after the
    /// load is marked "stopped after partial writes: ", its kind kept, nothing made.
    /// - Attached and colliding, its skip flag clear: the bounds step's refusal L (`LayerAbsent`), marked (lane
    ///   IMPL-SKIP-FLAG; the unread skip flag before).
    /// - Unattached: the extent is (0, 0, 0), the spot search's first test is a zero-extent box test, and the next
    ///   `NotYet` (the instancing's, on this fixture's primitive class, which is no `Core.Component`) is marked too.
    #[test]
    fn the_extent_loads_the_primitive_entries_and_marks_what_follows() {
        run(|| {
            for (name, want) in [
                ("Actor_ExtentLive", crate::world::bounding_cylinder::REFUSAL_L),
                ("Actor_ExtentQuiet", "a sub-object of the template that is not a component"),
            ] {
                let mut x = fx_files(true, ParityMode::Off, true);
                let wi = x.wi;
                let s = script(&mut x, vec![], vec![], Test::Clear);
                let (arch, entries) = extent_archetype(&mut x, name);
                let (prim, other) = (entries[0].unwrap(), entries[2].unwrap());
                let before = count(&x);
                let actor = x.class("Actor");
                let req = SpawnRequest::new(Some(actor)).with_template(Some(arch));
                let (missing, text) = what(spawn(&mut x.w.vm, wi, &req).unwrap_err());
                let kind = if name == "Actor_ExtentLive" {
                    Missing::LayerAbsent(crate::world::bounding_cylinder::BOUNDS_LAYER)
                } else {
                    Missing::NotRead
                };
                assert_eq!(missing, kind, "{name}: {text}");
                assert!(text.starts_with("stopped after partial writes: "), "{name}: {text}");
                assert_eq!(text.matches("stopped after partial writes: ").count(), 1, "{name}: marked once: {text}");
                assert!(text.contains(want), "{name}: {text}");
                assert!(!x.w.vm.defs.is_unloaded(&x.w.vm.space, prim), "{name}: the primitive entry was loaded");
                assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, other), "{name}: the other entry was not");
                let asked = s.asked.borrow();
                if name == "Actor_ExtentQuiet" {
                    assert_eq!(asked.len(), 1, "{asked:?}");
                    assert!(matches!(asked[0], Asked::Test([0.0, 0.0, 0.0], ..)), "{asked:?}");
                } else {
                    assert!(asked.is_empty(), "the extent refuses before the search: {asked:?}");
                }
                assert_eq!(count(&x), before, "{name}: nothing made");
            }
        });
    }

    /// A failing component load (review r1 of #498, items 1 and 5): a fresh attempt is a write even when it fails, so
    /// its `NotYet` is marked, once; a later load's failure after a good load is marked, once; a retry of the Failed
    /// handle writes nothing, so the same `NotYet` comes back unmarked. Nothing is made.
    #[test]
    fn a_failing_component_load_marks_its_not_yet_once() {
        run(|| {
            let mark = "stopped after partial writes: ";
            for name in ["Actor_ExtentFailsFirst", "Actor_ExtentFailsLater"] {
                let mut x = fx_files(true, ParityMode::Off, true);
                let wi = x.wi;
                script(&mut x, vec![], vec![], Test::Clear);
                let path = ow_core::ObjectPath::parse(&format!("Engine.{name}")).unwrap();
                let arch = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
                x.w.vm.log.extend(x.w.vm.defs.take_log());
                set_field(&mut x.w.vm.space, arch, "bCollideWorld", Value::Bool(true)).unwrap();
                let actor = x.class("Actor");
                let req = SpawnRequest::new(Some(actor)).with_template(Some(arch));
                let before = count(&x);
                let (missing, text) = what(spawn(&mut x.w.vm, wi, &req).unwrap_err());
                assert_eq!(missing, Missing::NotRead, "{name}: {text}");
                assert!(text.starts_with(mark), "{name}: {text}");
                assert_eq!(text.matches(mark).count(), 1, "{name}: marked once: {text}");
                assert!(text.contains("ExtentBadBase"), "{name}: the store's archetype NotYet: {text}");
                assert_eq!(count(&x), before, "{name}: nothing made");
                if name == "Actor_ExtentFailsFirst" {
                    let (missing, again) = what(spawn(&mut x.w.vm, wi, &req).unwrap_err());
                    assert_eq!(missing, Missing::NotRead);
                    assert_eq!(format!("{mark}{again}"), text, "the retry of the Failed handle is unmarked");
                }
            }
        });
    }

    /// Without a collision service the spot search's refusal comes first and loads nothing (NATIVE_SPAWN_EXTENT.md
    /// section 5's next stop, step 1): clean, no prefix.
    #[test]
    fn no_service_refuses_before_the_extent_loads() {
        run(|| {
            let mut x = fx_files(true, ParityMode::Off, true);
            let wi = x.wi;
            let (arch, entries) = extent_archetype(&mut x, "Actor_ExtentLive");
            let actor = x.class("Actor");
            let req = SpawnRequest::new(Some(actor)).with_template(Some(arch));
            let (missing, text) = what(spawn(&mut x.w.vm, wi, &req).unwrap_err());
            assert!(matches!(missing, Missing::LayerAbsent(_)), "{missing:?}");
            assert_eq!(text, SPOT_SEARCH);
            assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, entries[0].unwrap()), "nothing loaded");
        });
    }

    #[test]
    fn a_clear_service_and_no_tested_live_actor_keep_the_location() {
        // NATIVE_SPAWN_PLACEMENT.md section 7 (lane IMPL-SPAWN-TRACE-FILTERS): the filters are read, the service's
        // first box test is clear and no live actor primitive is tested, so the spawn is made where it was asked.
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            let s = script(&mut x, vec![], vec![], Test::Clear);
            let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
            let before = count(&x);
            assert!(spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap().is_some());
            let asked = s.asked.borrow();
            assert_eq!(asked.len(), 1, "one box test: {asked:?}");
            assert!(matches!(asked[0], Asked::Test(..)), "{asked:?}");
            assert_eq!(count(&x), before + 1);
        });
    }

    /// Live actors in the box test's walk (NATIVE_SPAWN_PLACEMENT.md section 7, reviews r3 of #465 and r1 of #477;
    /// NATIVE_SPAWN_TRACE_FILTER.md, lane IMPL-LINK9-TRACE-FILTER). A primitive any read test rejects (the skip flag,
    /// the blocking flag, the per-component test, the owner's row) is not tested and the spawn is made; one every read
    /// test accepts refuses, naming the unread rule, with no bounds from the service taken to meet the box. Nothing is
    /// made when the search stops.
    #[test]
    fn a_blocking_live_primitive_with_no_bounds_makes_the_box_test_unsupported() {
        run(|| {
            let none: &[&str] = &[];
            let admission = "the collision hash's admission beyond the per-component test (the live actor primitive";
            for (class, world_geometry, blocks, want) in [
                ("Actor", true, &["BlockNonZeroExtent", "CollideActors"][..], Some(admission)),
                ("Probe", false, &["BlockNonZeroExtent", "CollideActors"][..], Some("the trace filter of Engine.Probe (the live actor primitive")),
                // The owner's Actor row rejects it without bWorldGeometry.
                ("Actor", false, &["BlockNonZeroExtent", "CollideActors"][..], None),
                // The per-component test: the primitive's CollideActors clear.
                ("Actor", true, &["BlockNonZeroExtent"][..], None),
                // The skip flag set.
                ("Actor", true, &["BlockNonZeroExtent", "CollideActors", "BulletListener"][..], None),
                ("Actor", true, none, None),
                ("Actor", true, &["BlockZeroExtent", "CollideActors"][..], None),
            ] {
                let mut x = fx();
                let wi = x.wi;
                script(&mut x, vec![], vec![], Test::Clear);
                let b = blocker(&mut x, class, world_geometry, blocks);
                set_field(&mut x.w.vm.space, b, "bCollideActors", Value::Bool(true)).unwrap();
                let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
                let before = count(&x);
                let got = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe)));
                match want {
                    Some(rule) => {
                        let (missing, text) = what(got.unwrap_err());
                        assert_eq!(missing, Missing::NotRead, "{class}");
                        assert!(text.starts_with(SPOT_SEARCH), "{text}");
                        assert!(text.contains(rule), "{class}: {text}");
                        assert!(text.contains("whose bounds the collision service does not hold"), "{text}");
                        assert_eq!(count(&x), before, "{class}: nothing made");
                    }
                    None => {
                        assert!(got.unwrap().is_some(), "{class} {blocks:?}");
                        assert_eq!(count(&x), before + 1);
                    }
                }
            }
        });
    }

    /// With bounds from the service, a blocking primitive is in the walk only when its bounds meet the box.
    #[test]
    fn a_blocking_live_primitive_whose_bounds_miss_the_box_is_not_walked() {
        run(|| {
            // The probe asks for a 10 x 10 x 20 box at the origin (the fixture's default location).
            for (bounds, refused) in [
                (Bounds { min: [500.0, 500.0, 500.0], max: [510.0, 510.0, 510.0] }, false),
                (Bounds { min: [-5.0, -5.0, -5.0], max: [5.0, 5.0, 5.0] }, true),
            ] {
                let mut x = fx();
                let wi = x.wi;
                script_with_bounds(&mut x, bounds);
                let b = blocker(&mut x, "Actor", true, &["BlockNonZeroExtent", "CollideActors"]);
                set_field(&mut x.w.vm.space, b, "bCollideActors", Value::Bool(true)).unwrap();
                let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
                let got = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe)));
                if refused {
                    let (missing, text) = what(got.unwrap_err());
                    assert_eq!(missing, Missing::NotRead);
                    assert!(text.contains("the collision hash's admission beyond the per-component test (the live actor primitive"), "{text}");
                    assert!(!text.contains("does not hold"), "{text}");
                } else {
                    assert!(got.unwrap().is_some());
                }
            }
        });
    }

    /// Lane LIVE-COLLISION-STANDIN: a live actor's component whose class runs through an engine-native stand-in is
    /// not skipped as a non-primitive. Service bounds that miss the box rule it out and the spawn is made; bounds that
    /// meet it, or none, refuse the spot search with `NotRead` naming the component, and nothing is made.
    #[test]
    fn a_live_stand_in_component_is_ruled_out_only_by_its_bounds() {
        run(|| {
            let far = Bounds { min: [500.0, 500.0, 500.0], max: [510.0, 510.0, 510.0] };
            let near = Bounds { min: [-5.0, -5.0, -5.0], max: [5.0, 5.0, 5.0] };
            for bounds in [Some(far), Some(near), None] {
                let mut x = fx();
                let wi = x.wi;
                match bounds {
                    Some(b) => script_with_bounds(&mut x, b),
                    None => {
                        script(&mut x, vec![], vec![], Test::Clear);
                    }
                }
                let actor = x.spawn("Actor");
                set_field(&mut x.w.vm.space, actor, "bCollideActors", Value::Bool(true)).unwrap();
                let mut root = x.class("CylinderComponent");
                while let Some(up) = x.w.vm.space.classes.get_layout(root).and_then(|l| l.super_class()) {
                    root = up;
                }
                let native = x.w.vm.space.classes.define("Test.NativeOnlyComponent", Some(root), vec![]).unwrap();
                x.w.vm.space.classes.mark_native(native).unwrap();
                let part = x
                    .w
                    .vm
                    .space
                    .alloc_stand_in(native, "NativePart", ow_object::heap::Origin::Resource { package: 0, index: -1 })
                    .unwrap();
                set_field(&mut x.w.vm.space, actor, "CollisionComponent", Value::Object(Some(part))).unwrap();
                let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
                let before = count(&x);
                let got = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe)));
                if bounds == Some(far) {
                    assert!(got.unwrap().is_some(), "disjoint bounds rule the part out");
                    assert_eq!(count(&x), before + 1);
                } else {
                    let (missing, text) = what(got.unwrap_err());
                    assert_eq!(missing, Missing::NotRead, "{bounds:?}");
                    assert!(text.starts_with(SPOT_SEARCH), "{text}");
                    assert!(text.contains(crate::trace_filter::STAND_IN_PART) && text.contains("Test.NativeOnlyComponent"), "{text}");
                    assert_eq!(count(&x), before, "nothing made");
                }
            }
        });
    }

    /// Lane WALK-INVALID-BOUNDS: service bounds that are not finite and ordered (a NaN, an inverted box) are never
    /// taken for a miss. A primitive the filters keep is refused as `Actor.Trace` refuses it (its text, its record),
    /// in the spawn's box test and in the touch's line walk; one the filters leave out (its `CollideActors` clear) is
    /// left out whatever its bounds, and the spawn is made.
    #[test]
    fn invalid_bounds_are_refused_once_the_filters_keep_the_primitive() {
        run(|| {
            let nan = Bounds { min: [f32::NAN, -5.0, -5.0], max: [5.0, 5.0, 5.0] };
            let inverted = Bounds { min: [600.0, 600.0, 600.0], max: [500.0, 500.0, 500.0] };
            for bounds in [nan, inverted] {
                for kept in [true, false] {
                    let mut x = fx();
                    let wi = x.wi;
                    script_with_bounds(&mut x, bounds);
                    let flags: &[&str] = if kept { &["BlockNonZeroExtent", "BlockZeroExtent", "CollideActors"] } else { &["BlockNonZeroExtent", "BlockZeroExtent"] };
                    let b = blocker(&mut x, "Actor", true, flags);
                    set_field(&mut x.w.vm.space, b, "bCollideActors", Value::Bool(true)).unwrap();
                    let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
                    let before = count(&x);
                    let got = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe)));
                    if !kept {
                        assert!(got.unwrap().is_some(), "{bounds:?}: a primitive the filters leave out");
                        continue;
                    }
                    let VmError::NotYet(n) = got.unwrap_err() else { panic!() };
                    assert_eq!((n.missing, n.record), (Missing::NotRead, crate::world::actor_trace::RECORD), "{bounds:?}");
                    assert!(n.what.starts_with(SPOT_SEARCH) && n.what.contains(crate::placement::INVALID_BOUNDS), "{}", n.what);
                    assert!(crate::world::actor_trace::BAD_BOUNDS.ends_with(crate::placement::INVALID_BOUNDS));
                    assert_eq!(count(&x), before, "nothing made");
                    // The touch's line walk over the same live primitive, from another actor.
                    let model = crate::placement::collision_model(&x.w.vm).unwrap().unwrap();
                    let other = x.spawn("Actor");
                    let unloaded = |_: ObjRef| false;
                    let src = crate::trace_filter::Source { actor: other, unloaded: &unloaded, admission_established: true };
                    let region = Bounds { min: [-1.0; 3], max: [1.0; 3] };
                    let line = crate::placement::actor_walk(
                        &x.w.vm,
                        &*model,
                        crate::trace_filter::Query::PointLine,
                        region,
                        Some(&src),
                        "touch",
                        "R",
                    );
                    let (missing, text) = what(line.unwrap_err());
                    assert_eq!(missing, Missing::NotRead);
                    assert!(text.contains(crate::placement::INVALID_BOUNDS), "{text}");
                }
            }
        });
    }

    /// Review r2 of #477, item 1: a sweep's walk checks a primitive's service bounds before its filter data. A
    /// 100-unit sweep with the BSP at 0.5 walks to 0.55: an unloaded primitive whose bounds lie beyond it is not
    /// read (the BSP hit is the answer); the same primitive inside it is a named refusal for its unloaded data.
    #[test]
    fn a_sweep_skips_an_unloaded_primitive_beyond_the_capped_segment_before_reading_it() {
        run(|| {
            for (bounds, refused) in [
                (Bounds { min: [60.0, -1.0, -1.0], max: [62.0, 1.0, 1.0] }, false),
                (Bounds { min: [50.0, -1.0, -1.0], max: [52.0, 1.0, 1.0] }, true),
            ] {
                let mut x = fx();
                let bsp = Sweep::Hit { location: [50.0, 0.0, 0.0], normal: [-1.0, 0.0, 0.0], fraction: 0.5 };
                let s = Rc::new(Script {
                    tests: RefCell::new(vec![]),
                    sweeps: RefCell::new(vec![bsp]),
                    rest: Test::Clear,
                    asked: RefCell::new(Vec::new()),
                    bounds: Some(bounds),
                });
                install_collision(&mut x.w.vm, s).unwrap();
                let actor = x.spawn("Actor");
                let c = x.class("CylinderComponent");
                let origin = ow_object::heap::Origin::Resource { package: 0, index: -1 };
                let lazy = x.w.vm.space.alloc_stand_in(c, "LazyCylinder", origin).unwrap();
                set_field(&mut x.w.vm.space, actor, "CollisionComponent", Value::Object(Some(lazy))).unwrap();
                let got = crate::placement::box_sweep_in(&x.w.vm, [1.0; 3], [0.0; 3], [100.0, 0.0, 0.0]);
                if refused {
                    let (missing, text) = what(got.unwrap_err());
                    assert_eq!(missing, Missing::NotRead);
                    assert!(text.contains("LazyCylinder, which is not loaded"), "{text}");
                } else {
                    assert_eq!(got.unwrap(), Some(([50.0, 0.0, 0.0], [-1.0, 0.0, 0.0], 0.5)));
                }
            }
        });
    }

    /// Review r1 of #477, item 3: a box the level blocks reads no actor, and a primitive whose properties are not
    /// loaded is a named refusal once a query reaches the walk.
    #[test]
    fn the_level_is_asked_first_and_missing_primitive_data_is_a_named_refusal() {
        run(|| {
            for level in [Test::Blocked, Test::Clear] {
                let mut x = fx();
                let wi = x.wi;
                script(&mut x, vec![], vec![], level);
                let actor = x.spawn("Actor");
                let c = x.class("CylinderComponent");
                let origin = ow_object::heap::Origin::Resource { package: 0, index: -1 };
                let lazy = x.w.vm.space.alloc_stand_in(c, "LazyCylinder", origin).unwrap();
                set_field(&mut x.w.vm.space, actor, "CollisionComponent", Value::Object(Some(lazy))).unwrap();
                // A zero extent: a blocked first test fails the search at once (note section 3, step 2).
                let probe = colliding_probe(&mut x, Some((0.0, 0.0)));
                let got = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe)));
                match level {
                    Test::Blocked => assert_eq!(got.unwrap(), None, "the search fails; no actor read"),
                    _ => {
                        let (missing, text) = what(got.unwrap_err());
                        assert_eq!(missing, Missing::NotRead);
                        assert!(text.contains("LazyCylinder, which is not loaded"), "{text}");
                    }
                }
            }
        });
    }

    #[test]
    fn the_flags_are_the_templates_alone() {
        run(|| {
            // An archetype asking for the search over a class default without it: the search runs (no service).
            let mut x = fx();
            let wi = x.wi;
            let probe = x.class("Probe");
            let archetype = toy_archetype(&mut x, probe, "Probe_SearchArchetype");
            set_field(&mut x.w.vm.space, archetype, "bCollideWorld", Value::Bool(true)).unwrap();
            let req = SpawnRequest::new(Some(probe)).with_template(Some(archetype));
            let (missing, text) = what(spawn(&mut x.w.vm, wi, &req).unwrap_err());
            assert!(matches!(missing, Missing::LayerAbsent(_)));
            assert_eq!(text, SPOT_SEARCH);
            // A class default asking for it under an archetype without it: no search.
            let mut x = fx();
            let wi = x.wi;
            let probe = x.class("Probe");
            let archetype = toy_archetype(&mut x, probe, "Probe_PlainArchetype");
            let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Probe").unwrap();
            set_field(&mut x.w.vm.space, cdo, "bCollideWorld", Value::Bool(true)).unwrap();
            set_field(&mut x.w.vm.space, archetype, "bCollideWorld", Value::Bool(false)).unwrap();
            let req = SpawnRequest::new(Some(probe)).with_template(Some(archetype));
            assert!(spawn(&mut x.w.vm, wi, &req).unwrap().is_some());
        });
    }

    #[test]
    fn the_encroachment_and_relocation_not_yets_follow_the_live_actor() {
        run(|| {
            // With a service the check (lane IMPL-ENCROACHMENT-CHECK) passes the attach and location steps (no
            // primitive, not moved) and stops at the live actor's shape: Collider is not one of the extent's five
            // classes.
            let shape = format!("{BOUNDING_CYLINDER_OF} Engine.Collider");
            for (with_service, no_fail, want, layer) in [
                (false, false, ENCROACHMENT, true),
                (true, false, shape.as_str(), false),
                (false, true, RELOCATION, false),
            ] {
                let mut x = fx();
                let wi = x.wi;
                if with_service {
                    script(&mut x, vec![], vec![], Test::Clear);
                }
                let before = count(&x);
                let c = x.class("Collider");
                let req = SpawnRequest::new(Some(c)).with_no_collision_fail(no_fail);
                let (missing, text) = what(spawn(&mut x.w.vm, wi, &req).unwrap_err());
                assert_eq!(text, format!("stopped after partial writes: {want}"));
                assert_eq!(matches!(missing, Missing::LayerAbsent(_)), layer, "{missing:?}");
                assert_eq!(count(&x), before + 1, "{want}: made, past PreBeginPlay");
            }
            // An actor that does not collide with actors passes, as before.
            let mut x = fx();
            let wi = x.wi;
            let probe = x.class("Probe");
            assert!(spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap().is_some());
        });
    }

    /// A live `Actor` spawned by the world (nothing collides yet), then set to collide with actors, with the spawn's
    /// passed location and rotation (those it now holds): the encroachment check is then called on it directly.
    fn live_collider(x: &mut Fx) -> (ObjRef, Value, Value) {
        let a = x.spawn("Actor");
        set_field(&mut x.w.vm.space, a, "bCollideActors", Value::Bool(true)).unwrap();
        let location = field(&x.w.vm.space, a, "Location").unwrap();
        let rotation = field(&x.w.vm.space, a, "Rotation").unwrap();
        (a, location, rotation)
    }

    fn cylinder(x: &mut Fx, r: f32, h: f32, collides: bool) -> ObjRef {
        let c = x.class("CylinderComponent");
        let comp = ow_vm::interp::instantiate_from_defaults(&mut x.w.vm, c).unwrap();
        set_field(&mut x.w.vm.space, comp, "CollisionRadius", Value::Float(r)).unwrap();
        set_field(&mut x.w.vm.space, comp, "CollisionHeight", Value::Float(h)).unwrap();
        set_field(&mut x.w.vm.space, comp, "CollideActors", Value::Bool(collides)).unwrap();
        comp
    }

    fn encroach_not_read(r: Result<(), VmError>) -> String {
        match r.unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead, "{n}");
                assert!(!n.what.starts_with("stopped after partial writes"), "the check returns it unmarked: {n}");
                n.what.into_owned()
            }
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    /// NATIVE_ENCROACHMENT_CHECK.md section 3, step 3, in its order (lane IMPL-ENCROACHMENT-CHECK), each refusal
    /// unmarked and `NotRead` under the note's record: the service; the attach step for a primitive with
    /// `CollideActors`, whatever `bAttached` says; the move for a passed location or rotation that differs; the level
    /// part for a Blocked or Unsupported box test of the live shape; then no colliding primitive, no overlap, `Ok`.
    #[test]
    fn the_encroachment_check_follows_the_note_s_order() {
        use crate::placement::encroachment::{ATTACH_STEP, LEVEL_PART, MOVE_STEP, RECORD_ENCROACHMENT, check};
        run(|| {
            let mut x = fx();
            let (a, location, rotation) = live_collider(&mut x);
            match check(&x.w.vm, a, &location, &rotation).unwrap_err() {
                VmError::NotYet(n) => {
                    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
                    assert_eq!(n.what, ENCROACHMENT);
                }
                other => panic!("{other:?}"),
            }
            let s = script(&mut x, vec![Test::Blocked, Test::Unsupported], vec![], Test::Clear);
            // The attach step: an unattached primitive with CollideActors is never taken to pass.
            let comp = cylinder(&mut x, 10.0, 20.0, true);
            set_field(&mut x.w.vm.space, comp, "bAttached", Value::Bool(false)).unwrap();
            set_field(&mut x.w.vm.space, a, "CollisionComponent", Value::Object(Some(comp))).unwrap();
            let what = encroach_not_read(check(&x.w.vm, a, &location, &rotation));
            assert!(what.starts_with(ATTACH_STEP), "{what}");
            match check(&x.w.vm, a, &location, &rotation).unwrap_err() {
                VmError::NotYet(n) => assert_eq!(n.record, RECORD_ENCROACHMENT),
                other => panic!("{other:?}"),
            }
            set_field(&mut x.w.vm.space, comp, "CollideActors", Value::Bool(false)).unwrap();
            // The move: a passed location or rotation other than the live actor's.
            for k in [0usize, 1] {
                let (mut l, mut r) = (location.clone(), rotation.clone());
                let moved = if k == 0 { &mut l } else { &mut r };
                let f = &mut moved.struct_mut().unwrap().fields[0];
                *f = match f {
                    Value::Float(v) => Value::Float(*v + 1.0),
                    Value::Int(v) => Value::Int(*v + 1),
                    other => panic!("{other:?}"),
                };
                assert_eq!(encroach_not_read(check(&x.w.vm, a, &l, &r)), MOVE_STEP);
            }
            assert!(s.asked.borrow().is_empty(), "no query before the location step passes");
            // The level part, with the live shape (the cylinder's extent).
            assert_eq!(encroach_not_read(check(&x.w.vm, a, &location, &rotation)), LEVEL_PART);
            assert_eq!(encroach_not_read(check(&x.w.vm, a, &location, &rotation)), LEVEL_PART);
            check(&x.w.vm, a, &location, &rotation).unwrap();
            let asked = s.asked.borrow();
            assert_eq!(asked.len(), 3, "{asked:?}");
            assert!(matches!(asked[0], Asked::Test([10.0, 10.0, 20.0], _, Mode::Template { complex: false })), "{asked:?}");
        });
    }

    /// The port's overlap walk (note section 3, step 3.6), reached only once the attach step is read, so tested
    /// directly: the actor itself and an actor based on it are skipped; a live primitive with no service bounds is the
    /// overlap query's `NotRead`; bounds that miss the region leave the walk; bounds that meet it are the per-class
    /// test's `NotRead`.
    #[test]
    fn the_encroachment_walk_needs_bounds_and_the_per_class_test() {
        use crate::placement::encroachment::{OVERLAP_QUERY, PER_CLASS_TEST, walk};
        run(|| {
            let region = Bounds { min: [-1.0; 3], max: [1.0; 3] };
            for (bounds, want) in [
                (None, Some(OVERLAP_QUERY)),
                (Some(Bounds { min: [5.0; 3], max: [6.0; 3] }), None),
                (Some(Bounds { min: [0.0; 3], max: [2.0; 3] }), Some(PER_CLASS_TEST)),
            ] {
                let mut x = fx();
                let (a, _, _) = live_collider(&mut x);
                let own = cylinder(&mut x, 1.0, 1.0, true);
                set_field(&mut x.w.vm.space, a, "CollisionComponent", Value::Object(Some(own))).unwrap();
                // An actor based on the new one is skipped, its primitive never asked.
                let rider = blocker(&mut x, "Actor", false, &[]);
                set_field(&mut x.w.vm.space, rider, "Base", Value::Object(Some(a))).unwrap();
                let model = Script {
                    tests: RefCell::new(vec![]),
                    sweeps: RefCell::new(vec![]),
                    rest: Test::Clear,
                    asked: RefCell::new(Vec::new()),
                    bounds,
                };
                assert!(walk(&x.w.vm, &model, a, region).is_ok(), "only the actor itself and its rider");
                blocker(&mut x, "Actor", false, &[]);
                let got = walk(&x.w.vm, &model, a, region);
                match want {
                    None => got.unwrap(),
                    Some(w) => {
                        let what = encroach_not_read(got);
                        assert!(what.starts_with(w), "{what}");
                    }
                }
            }
        });
    }

    /// Through the spawn, a passing check (review r1 of #503, low 3): a probe that collides with actors and with the
    /// world, whose cylinder does not have `CollideActors`, passes the check (no colliding primitive, not moved, a
    /// Clear level part, an empty overlap set) and goes on to PostBeginPlay and SetInitialState; it is made at the
    /// location the spot search returned (the requested one, the first test being Clear).
    #[test]
    fn a_colliding_actor_with_no_colliding_primitive_passes_the_check_to_post_begin_play() {
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            let s = script(&mut x, vec![], vec![], Test::Clear);
            let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
            let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Probe").unwrap();
            set_field(&mut x.w.vm.space, cdo, "bCollideActors", Value::Bool(true)).unwrap();
            let asked_for = field(&x.w.vm.space, wi, "Location").unwrap();
            let before = count(&x);
            let p = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap().expect("made");
            assert_eq!(count(&x), before + 1);
            let begun: Vec<i32> = x.all_notes().into_iter().filter(|(o, _)| *o == p).map(|(_, k)| k).collect();
            assert_eq!(begun, vec![11, 12, 13], "PreBeginPlay, PostBeginPlay, SetInitialState");
            assert_eq!(field(&x.w.vm.space, p, "Location").unwrap(), asked_for, "the spot search's location");
            // The spot search's first test, then the check's level part: both box tests of the cylinder's extent.
            let asked = s.asked.borrow();
            assert_eq!(asked.len(), 2, "{asked:?}");
            assert!(asked.iter().all(|a| matches!(a, Asked::Test([10.0, 10.0, 20.0], ..))), "{asked:?}");
        });
    }

    /// Through the spawn: a live actor whose primitive has `CollideActors` stops at the attach step, marked by the
    /// spawn's boundary, its kind kept; the actor stays made.
    #[test]
    fn a_spawn_s_attach_step_refusal_is_marked() {
        use crate::placement::encroachment::ATTACH_STEP;
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            script(&mut x, vec![], vec![], Test::Clear);
            let comp = cylinder(&mut x, 5.0, 5.0, true);
            let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Collider").unwrap();
            set_field(&mut x.w.vm.space, cdo, "CollisionComponent", Value::Object(Some(comp))).unwrap();
            let before = count(&x);
            let c = x.class("Collider");
            let (missing, text) = what(spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(c))).unwrap_err());
            assert_eq!(missing, Missing::NotRead);
            assert!(text.starts_with(&format!("stopped after partial writes: {ATTACH_STEP}")), "{text}");
            assert_eq!(count(&x), before + 1, "made, past PreBeginPlay");
        });
    }

    #[test]
    fn reference_keeps_its_collision_refusals() {
        // A service and a cylinder are installed (review r1 of #463, item 4): Reference still answers its legacy
        // refusal, while Off, with the trace filters read (lane IMPL-SPAWN-TRACE-FILTERS), makes the spawn.
        run(|| {
            for mode in [ParityMode::Reference, ParityMode::Off] {
                let mut x = fx_in(true, mode);
                let wi = x.wi;
                script(&mut x, vec![], vec![], Test::Clear);
                let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
                let before = count(&x);
                let got = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe)));
                match mode {
                    ParityMode::Reference => {
                        let (missing, text) = what(got.unwrap_err());
                        assert!(matches!(missing, Missing::LayerAbsent(_)));
                        assert_eq!(text, "the spawn placement spot search");
                        assert_eq!(count(&x), before);
                    }
                    _ => {
                        assert!(got.unwrap().is_some());
                        assert_eq!(count(&x), before + 1);
                    }
                }
            }
        });
    }

    #[test]
    fn an_off_refusal_after_insertion_is_prefixed_and_the_actor_stays() {
        // Review r1 of #463, item 1: PreBeginPlay answers a NotYet; the actor is already registered.
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            x.w.vm
                .register_native(
                    "Actor.Halt()",
                    Rc::new(|_c: &mut NativeCall<'_>| -> Result<Value, VmError> {
                        Err(ow_core::NotYet::not_read("an invented halt", "test").into())
                    }),
                )
                .unwrap();
            let before = count(&x);
            let c = x.class("Halter");
            let (missing, text) = what(spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(c))).unwrap_err());
            assert_eq!(missing, Missing::NotRead);
            assert_eq!(text, "stopped after partial writes: an invented halt");
            assert_eq!(count(&x), before + 1, "the actor stays registered");
        });
    }

    #[test]
    fn reference_keeps_the_base_precheck_before_the_spawner_reads() {
        // Review r2 of #463: after play began, a bNoCollisionFail spawn of a template that needs a base search, by a
        // spawner that is gone: Reference answers the base-search NotYet before it reads the spawner.
        run(|| {
            let mut x = fx_in(true, ParityMode::Reference);
            let probe = x.class("Probe");
            let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Probe").unwrap();
            set_field(&mut x.w.vm.space, cdo, "bCollideWorld", Value::Bool(true)).unwrap();
            set_field(&mut x.w.vm.space, cdo, "bShouldBaseAtStartup", Value::Bool(true)).unwrap();
            let c = x.class("Dud");
            let gone = ow_vm::interp::instantiate_from_defaults(&mut x.w.vm, c).unwrap();
            x.w.vm.space.free(gone).unwrap();
            let req = SpawnRequest::new(Some(probe)).with_no_collision_fail(true);
            let (missing, text) = what(spawn(&mut x.w.vm, gone, &req).unwrap_err());
            assert!(matches!(missing, Missing::LayerAbsent(_)), "{missing:?}");
            assert_eq!(text, "the base search after PostBeginPlay");
        });
    }

    #[test]
    fn the_placement_refusal_comes_before_the_base_precheck() {
        // Review r1 of #463, item 2: a template after play began that needs both, with no service.
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
            let cdo = x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Probe").unwrap();
            set_field(&mut x.w.vm.space, cdo, "bShouldBaseAtStartup", Value::Bool(true)).unwrap();
            let before = count(&x);
            let (missing, text) = what(spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap_err());
            assert!(matches!(missing, Missing::LayerAbsent(_)), "{missing:?}");
            assert_eq!(text, SPOT_SEARCH);
            assert_eq!(count(&x), before);
        });
    }

    #[test]
    fn a_borrowed_collision_service_is_a_borrow_error_not_an_absent_service() {
        // Review r1 of #463, item 3.
        run(|| {
            let mut x = fx();
            let wi = x.wi;
            script(&mut x, vec![], vec![], Test::Clear);
            let probe = colliding_probe(&mut x, Some((10.0, 20.0)));
            let service = x.w.vm.service::<crate::placement::CollisionService>().unwrap();
            let held = service.borrow_mut();
            let e = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap_err();
            assert!(matches!(e, VmError::ServiceBorrowed(_)), "{e:?}");
            drop(held);
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A controller's and a pawn's destroy, the kill's unlinking (lane IMPL-ACTOR-DESTROY-CONTROLLER, decision
// 2026-11-19-92; NATIVE_ACTOR_DESTROY_CONTROLLER.md)
// ---------------------------------------------------------------------------------------------------------------

fn obj(x: &Fx, o: ObjRef, name: &str) -> Option<ObjRef> {
    match field(&x.w.vm.space, o, name).unwrap() {
        Value::Object(r) => r,
        other => panic!("{name}: {other:?}"),
    }
}

fn members(x: &Fx, list: crate::actors::WorldList) -> Vec<ObjRef> {
    crate::actors::list_members(&x.w.vm.space, x.wi, list).unwrap()
}

/// A spawn adds a controller and a pawn at the head of their lists, newest first (section 4).
#[test]
fn a_spawn_links_controllers_and_pawns_newest_first() {
    use crate::actors::WorldList::{Controllers, Pawns};
    run(|| {
        let mut x = fx();
        let (c1, c2) = (x.spawn("WillowMind"), x.spawn("Controller"));
        let (p1, p2) = (x.spawn("Pawn"), x.spawn("Pawn"));
        x.spawn("Probe");
        assert_eq!(members(&x, Controllers), [c2, c1]);
        assert_eq!(members(&x, Pawns), [p2, p1]);
    });
}

/// The mind's destroy refuses with the note's first row, the actor step's notification, before any write (section 5.2,
/// review r1 of #472), whatever its fields hold.
#[test]
fn a_minds_destroy_answers_the_notifications_not_yet_before_any_write() {
    use crate::actors::WorldList::Controllers;
    run(|| {
        let mut x = fx();
        let mind = x.spawn("WillowMind");
        let proxy = x.spawn("Probe");
        set_field(&mut x.w.vm.space, mind, "InteractionProxy", Value::Object(Some(proxy))).unwrap();
        match destroy(&mut x.w.vm, mind).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert_eq!(n.what, crate::actors::NOTIFICATION_NOT_READ, "no prefix: nothing written");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(field(&x.w.vm.space, mind, "bPendingDelete").unwrap(), Value::Bool(false));
        assert_eq!(field(&x.w.vm.space, proxy, "bPendingDelete").unwrap(), Value::Bool(false));
        assert_eq!(members(&x, Controllers), [mind]);
    });
}

/// The rows after the notification, in order (section 5.2): the controller list, then the rule engine. And the
/// world-drop step they lead to (section 5.4): the pool manager destroyed and cleared, the unlink with
/// `NextController` None, then the interaction proxy destroyed (the field kept); a middle member's predecessor takes
/// its link.
#[test]
fn the_minds_later_rows_and_its_world_drop_step() {
    use crate::actors::WorldList::Controllers;
    use crate::actors::{plan_controller_rows, world_drop};
    run(|| {
        let mut x = fx();
        let first = x.spawn("Controller");
        let mind = x.spawn("WillowMind");
        let last = x.spawn("Controller");
        let (pool, proxy, engine) = (x.spawn("Probe"), x.spawn("Probe"), x.spawn("Probe"));
        set_field(&mut x.w.vm.space, mind, "MyRuleEngine", Value::Object(Some(engine))).unwrap();
        let actors = x.w.actors();
        match plan_controller_rows(&x.w.vm, &actors, mind).unwrap_err() {
            VmError::NotYet(n) => assert_eq!(n.what, crate::actors::RULE_ENGINE_NOT_READ),
            other => panic!("{other:?}"),
        }
        set_field(&mut x.w.vm.space, mind, "MyRuleEngine", Value::Object(None)).unwrap();
        set_field(&mut x.w.vm.space, mind, "ResourcePoolManager", Value::Object(Some(pool))).unwrap();
        set_field(&mut x.w.vm.space, mind, "InteractionProxy", Value::Object(Some(proxy))).unwrap();
        let drop = plan_controller_rows(&x.w.vm, &actors, mind).unwrap();
        world_drop(&mut x.w.vm, mind, &drop).unwrap();
        assert_eq!(members(&x, Controllers), [last, first], "the predecessor takes its link");
        assert_eq!(obj(&x, mind, "NextController"), None);
        assert_eq!(obj(&x, mind, "ResourcePoolManager"), None);
        assert_eq!(obj(&x, mind, "InteractionProxy"), Some(proxy), "the proxy field is not cleared");
        for o in [pool, proxy] {
            assert_eq!(field(&x.w.vm.space, o, "bDeleteMe").unwrap(), Value::Bool(true));
        }
        let drop = plan_controller_rows(&x.w.vm, &actors, last).unwrap();
        world_drop(&mut x.w.vm, last, &drop).unwrap();
        assert_eq!(members(&x, Controllers), [first], "the head unlinked through the list's head field");
    });
}

/// A pawn's destroy unlinks it from `PawnList`; its own `NextPawn` is left as it was (section 3). A modelled list
/// without the pawn is not a refusal (section 5.5): the list is left unchanged.
#[test]
fn a_pawns_destroy_unlinks_it_and_leaves_its_link() {
    use crate::actors::WorldList::Pawns;
    run(|| {
        let mut x = fx();
        let (p1, p2) = (x.spawn("Pawn"), x.spawn("Pawn"));
        assert!(destroy(&mut x.w.vm, p2).unwrap());
        assert_eq!(members(&x, Pawns), [p1]);
        assert_eq!(obj(&x, p2, "NextPawn"), Some(p1), "NextPawn left");
        let stray = x.spawn("Pawn");
        set_field(&mut x.w.vm.space, x.wi, "PawnList", Value::Object(Some(p1))).unwrap();
        assert!(destroy(&mut x.w.vm, stray).unwrap());
        assert_eq!(members(&x, Pawns), [p1], "not in the list: left unchanged");
    });
}

/// A world whose `WorldInfo` does not declare the lists holds neither (section 5.2).
#[test]
fn a_world_info_without_the_lists_holds_neither() {
    use crate::actors::WorldList::{Controllers, Pawns};
    run(|| {
        let mut x = fx_with(false);
        let probe = x.spawn("Probe");
        let mut actors = crate::actors::Actors::new(x.class("Actor"));
        actors.place_world_info(probe).unwrap();
        assert_eq!(crate::actors::list_holder(&x.w.vm.space, &actors, Controllers).unwrap(), None);
        assert_eq!(crate::actors::list_holder(&x.w.vm.space, &actors, Pawns).unwrap(), None);
    });
}

/// The layer refusal through `destroy`: a world whose `WorldInfo` class does not declare the pawn list.
#[test]
fn a_pawns_destroy_without_the_list_is_layer_absent() {
    run(|| {
        let mut x = fx();
        let pawn = x.spawn("Pawn");
        // Swap the level's WorldInfo for a probe, which declares no lists (the service is the world's).
        let probe = x.spawn("Probe");
        let actor_class = x.class("Actor");
        let fresh = {
            let mut a = crate::actors::Actors::new(actor_class);
            a.place_world_info(probe).unwrap();
            a.begin_play();
            a
        };
        *x.w.actors().borrow_mut() = fresh;
        match destroy(&mut x.w.vm, pawn).unwrap_err() {
            VmError::NotYet(n) => assert!(matches!(n.missing, Missing::LayerAbsent(_)) && n.what.contains("pawn list"), "{n}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(field(&x.w.vm.space, pawn, "bPendingDelete").unwrap(), Value::Bool(false), "nothing written");
    });
}

/// A `NotYet` after the first write is terminal (section 5): the pawn's `Destroyed()` refuses after `bPendingDelete`,
/// and the destroy answers it marked "stopped after partial writes", kind and record kept, with its write left (no
/// rollback).
#[test]
fn a_refusal_after_the_first_write_is_marked() {
    run(|| {
        let mut x = fx();
        let pawn = x.spawn("DoomedPawn");
        match destroy(&mut x.w.vm, pawn).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert_eq!(n.record, "an invented record");
                assert!(n.what.starts_with(crate::actors::PARTIAL_WRITES), "{n}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(field(&x.w.vm.space, pawn, "bPendingDelete").unwrap(), Value::Bool(true));
    });
}

/// The player case (NATIVE_ACTOR_DESTROY.md section 3; review r1 of #481, item 3), by the world's own local-player
/// identity: a local player's controller is `NotYet { LayerAbsent }` (the viewport removal is the host's, which the
/// port lacks) before any write; a player controller with a `Player` the world does not list as local is a network
/// player's, `NotYet { NotPorted }`.
#[test]
fn a_player_controllers_destroy_follows_the_player_case() {
    run(|| {
        let mut x = fx();
        let pc = x.spawn("PlayerController");
        x.w.actors().borrow_mut().add_local_player(pc).unwrap();
        assert!(matches!(vm_missing(destroy(&mut x.w.vm, pc).unwrap_err()), Missing::LayerAbsent(_)));
        assert_eq!(field(&x.w.vm.space, pc, "bPendingDestroy").unwrap(), Value::Bool(false), "nothing written");
        let net_pc = x.spawn("PlayerController");
        let net = {
            let c = x.class("Object");
            x.w.vm.space.alloc(c, "ToyConnection", None, ow_object::heap::Origin::Runtime).unwrap()
        };
        set_field(&mut x.w.vm.space, net_pc, "Player", Value::Object(Some(net))).unwrap();
        assert_eq!(vm_missing(destroy(&mut x.w.vm, net_pc).unwrap_err()), Missing::NotPorted);
    });
}

/// The kill's script path ends in the mind's own `Destroy()` (section 1, step 3): run as script from a toy
/// `PawnDied`, the destroy's refusal (the notification, before any write) reaches the caller typed, and the mind is
/// left as it was.
#[test]
fn a_minds_pawn_died_reaches_its_destroy_through_script() {
    use crate::actors::WorldList::Controllers;
    run(|| {
        let mut x = fx();
        let mind = x.spawn("WillowMind");
        let f = x.w.vm.find_function("Engine.WillowMind.PawnDied").unwrap();
        match x.w.vm.call(f, Some(mind), vec![]).unwrap_err() {
            VmError::NotYet(n) => assert_eq!(n.what, crate::actors::NOTIFICATION_NOT_READ),
            other => panic!("{other:?}"),
        }
        assert_eq!(field(&x.w.vm.space, mind, "bDeleteMe").unwrap(), Value::Bool(false));
        assert_eq!(members(&x, Controllers), [mind]);
    });
}

/// The kill's earliest certain point (review r1 of #481, item 1; note section 5.1): at the entry of the pawn's
/// `Dying.BeginState`, a non-player controller that still possesses it is checked with its destroy's read-only
/// preflight. Its refusal stops the kill before the body: no dying write (note 30), and both possession links are
/// left. The refusal is marked (it comes after the death's earlier writes), kind kept. A player's controller is not
/// checked, and the body runs.
#[test]
fn the_kill_refuses_at_dying_begin_state_before_its_writes() {
    run(|| {
        let mut x = fx();
        let pawn = x.spawn("Pawn");
        let mind = x.spawn("WillowMind");
        set_field(&mut x.w.vm.space, pawn, "Controller", Value::Object(Some(mind))).unwrap();
        set_field(&mut x.w.vm.space, mind, "Pawn", Value::Object(Some(pawn))).unwrap();
        x.clear_notes();
        let f = x.w.vm.find_function("Engine.Pawn.Dying.BeginState").unwrap();
        match x.w.vm.call(f, Some(pawn), vec![]).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.what.starts_with(crate::actors::PARTIAL_WRITES), "{n}");
                assert!(n.what.ends_with(crate::actors::NOTIFICATION_NOT_READ), "{n}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(x.noted(pawn), Vec::<i32>::new(), "the body did not run");
        assert_eq!(obj(&x, pawn, "Controller"), Some(mind));
        assert_eq!(obj(&x, mind, "Pawn"), Some(pawn));
        assert_eq!(field(&x.w.vm.space, mind, "bPendingDelete").unwrap(), Value::Bool(false));

        // A player's controller: not the destroy path, so the body runs.
        set_field(&mut x.w.vm.space, mind, "bIsPlayer", Value::Bool(true)).unwrap();
        x.w.vm.call(f, Some(pawn), vec![]).unwrap();
        assert_eq!(x.noted(pawn), vec![30]);
    });
}

/// The pawn list as the destroy's own scripts see it (review r1 of #481, item 5): the pawn's `Destroyed()` runs
/// before the world-drop step, so it still sees itself listed; its owner's `LostChild` runs after, and no longer does.
#[test]
fn a_pawns_destroy_scripts_see_its_membership_in_order() {
    run(|| {
        let mut x = fx();
        let owner = x.spawn("ListedOwner");
        let pawn = x.spawn("ListedPawn");
        set_owner(&mut x.w.vm, pawn, Some(owner)).unwrap();
        x.pawn_lists.borrow_mut().clear();
        assert!(destroy(&mut x.w.vm, pawn).unwrap());
        let seen = x.pawn_lists.borrow().clone();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert_eq!(seen[0].0, pawn);
        assert!(seen[0].1.contains(&pawn), "Destroyed() sees itself listed");
        assert_eq!(seen[1].0, owner);
        assert!(!seen[1].1.contains(&pawn), "the owner's LostChild sees it gone");
    });
}

/// A spawn that refuses after `PreBeginPlay` made the actor collide is not linked into its list (review r1 of #481,
/// item 4), and the refusal is marked: the actor was made and listed in the level first.
#[test]
fn a_spawn_refused_after_pre_begin_play_is_not_linked_and_is_marked() {
    use crate::actors::WorldList::Pawns;
    run(|| {
        let mut x = fx();
        let class = x.class("CollidingPawn");
        let wi = x.wi;
        match spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(class))).unwrap_err() {
            VmError::NotYet(n) => assert!(n.what.starts_with(crate::actors::PARTIAL_WRITES), "{n}"),
            other => panic!("{other:?}"),
        }
        assert!(members(&x, Pawns).is_empty(), "not reachable through the pawn list");
    });
}

/// Review r1 of #514, item 3: a first spawn whose class default is cold builds and publishes it, a write, so its later
/// `NotYet` is marked even with no archetype or extent load. `ColdCollider`'s default asks for the spot search, and its
/// class is not one of the extent rule's five, so the search refuses on the extent; the same spawn after a warm default
/// is unmarked.
#[test]
fn a_refusal_after_a_cold_class_default_build_is_marked() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        let installed = std::rc::Rc::new(crate::placement::NoLevel);
        crate::placement::install_collision(&mut x.w.vm, installed).unwrap();
        let class = x.class("ColdCollider");
        let req = SpawnRequest::new(Some(class));
        let heap = x.w.vm.space.heap.len();
        let (missing, text) = match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => (n.missing, n.what.into_owned()),
            other => panic!("{other:?}"),
        };
        assert!(x.w.vm.space.heap.len() > heap, "the default was built");
        assert_eq!(missing, Missing::NotRead);
        let want = "the bounding-cylinder rule of Engine.ColdCollider";
        assert_eq!(text, format!("{}: {want}", "stopped after partial writes"), "marked once");
        // The default is now cached: the same refusal is unmarked.
        match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => assert_eq!(n.what, want),
            other => panic!("{other:?}"),
        }
    });
}

/// A lazy archetype load is a write (review r2 of #481, M1): a spawn whose unloaded archetype loads and then asks for
/// the spot search refuses with the collision `NotYet` marked "stopped after partial writes", its kind kept.
#[test]
fn a_refusal_after_a_lazy_archetype_load_is_marked() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        let probe = x.class("Probe");
        let path = ow_core::ObjectPath::parse("Engine.ToyLazyHolder").unwrap();
        let vm = &mut x.w.vm;
        let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let arch = crate::actors::object(&x.w.vm.space, holder, "Arch").unwrap().unwrap();
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, arch), "the fixture's archetype is lazy");
        let req = SpawnRequest::new(Some(probe)).with_template(Some(arch));
        match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => {
                assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
                assert!(n.what.starts_with(crate::actors::PARTIAL_WRITES), "{n}");
            }
            other => panic!("{other:?}"),
        }
    });
}

/// Review r1 of #478, item 1: a template with two sub-objects made from the inherited component's export is ambiguous,
/// a `NotYet { NotRead }` with nothing made.
#[test]
fn an_inherited_component_with_two_stand_ins_is_refused() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let parent = parent_default_component(&mut x, 5);
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Twin").unwrap();
        let template = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        set_field(&mut x.w.vm.space, template, "Comp", Value::Object(Some(parent))).unwrap();
        let (heap, list) = (x.w.vm.space.heap.len(), x.w.actors().borrow().list().len());
        let toolbox = x.class("Toolbox");
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(template));
        match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.what.contains("more than one sub-object"), "{n}");
            }
            e => panic!("expected NotYet, got {e}"),
        }
        assert_eq!(x.w.vm.space.heap.len(), heap, "nothing on the heap");
        assert_eq!(x.w.actors().borrow().list().len(), list, "nothing in the level list");
    });
}

/// Review r1 of #478, item 3: a slot naming an object inside a parent class default's component is refused, not
/// shared into the parent default's graph.
#[test]
fn an_object_inside_an_inherited_component_is_refused() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let parent = parent_default_component(&mut x, 5);
        let thing = x.class("Object");
        let inner = x.w.vm.space.alloc(thing, "Inner", Some(parent), ow_object::heap::Origin::Runtime).unwrap();
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Inherit").unwrap();
        let template = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        set_field(&mut x.w.vm.space, template, "Thing", Value::Object(Some(inner))).unwrap();
        let toolbox = x.class("Toolbox");
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(template));
        match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => assert!(n.what.contains("nested inside an inherited component"), "{n}"),
            e => panic!("expected NotYet, got {e}"),
        }
    });
}

/// Review r1 of #478, item 4: in Off a `NotYet` after the actor joined the level list (here an event's) is terminal,
/// prefixed, its kind and record kept, the actor left in the list; Reference keeps the text as it was.
#[test]
fn a_not_yet_after_registration_is_prefixed_in_off() {
    run(|| {
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut x = fx_files(true, mode, false);
            let wi = x.wi;
            let probe = x.class("Probe");
            let list = x.w.actors().borrow().list().len();
            REFUSE_NOTE.with(|r| r.set(Some(11)));
            let got = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe)));
            REFUSE_NOTE.with(|r| r.set(None));
            match got.unwrap_err() {
                VmError::NotYet(n) => {
                    assert_eq!((n.missing, n.record), (Missing::NotPorted, "docs/invented.md"), "{mode:?}");
                    let prefixed = n.what.starts_with("stopped after partial writes: an invented refusal");
                    assert_eq!(prefixed, mode == ParityMode::Off, "{mode:?}: {n}");
                }
                e => panic!("{mode:?}: expected NotYet, got {e}"),
            }
            assert_eq!(x.w.actors().borrow().list().len(), list + 1, "{mode:?}: the actor stays registered");
        }
    });
}

/// Review r2 of #478: a lazy load is a write. A template the store has not loaded is loaded by the spawn, and the
/// plan's refusal after that load (its reference back to itself) is terminal in Off: prefixed, its kind kept, nothing
/// made.
#[test]
fn a_refusal_after_the_spawns_lazy_load_is_prefixed() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Pointer").unwrap();
        let pointer = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        let Value::Object(Some(arch)) = field(&x.w.vm.space, pointer, "Thing").unwrap() else {
            panic!("no archetype handle");
        };
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, arch), "the archetype is not loaded yet");
        let list = x.w.actors().borrow().list().len();
        let toolbox = x.class("Toolbox");
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(arch));
        match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.what.starts_with("stopped after partial writes: "), "{n}");
                assert!(n.what.contains("back to the template"), "{n}");
            }
            e => panic!("expected NotYet, got {e}"),
        }
        assert!(!x.w.vm.defs.is_unloaded(&x.w.vm.space, arch), "the spawn loaded it");
        assert_eq!(x.w.actors().borrow().list().len(), list, "nothing in the level list");
        // With the archetype loaded, the same refusal writes nothing first and is not prefixed.
        match spawn(&mut x.w.vm, wi, &req).unwrap_err() {
            VmError::NotYet(n) => assert!(!n.what.starts_with("stopped after partial writes"), "{n}"),
            e => panic!("expected NotYet, got {e}"),
        }
    });
}

/// Lane SPAWN-COMPLETE (hub ruling): an Off spawn loads the definitions the new actor's own object properties point to,
/// one level deep, before any script runs on it. `ToyTarget` is unloaded before the spawn and loaded after it, and the
/// actor's `Body` is that same definition.
#[test]
fn a_spawn_loads_the_definitions_its_actor_points_to() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        let class = x.class("BodyProbe");
        let path = ow_core::ObjectPath::parse("Engine.BodyProbe_Arch").unwrap();
        let vm = &mut x.w.vm;
        let arch = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let target = crate::actors::object(&x.w.vm.space, arch, "Body").unwrap().unwrap();
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, target), "ToyTarget is unloaded before the spawn");
        let req = SpawnRequest::new(Some(class)).with_template(Some(arch));
        let actor = spawn(&mut x.w.vm, wi, &req).unwrap().unwrap();
        assert!(!x.w.vm.defs.is_unloaded(&x.w.vm.space, target), "loaded by the spawn");
        assert_eq!(crate::actors::object(&x.w.vm.space, actor, "Body").unwrap(), Some(target));
    });
}

/// Lane SPAWN-COMPLETE: the spawn's definition load is a write, so a refusal after it is marked "stopped after partial
/// writes", its kind kept (here the post-spawn encroachment check of an actor that collides with actors).
#[test]
fn a_refusal_after_the_spawn_s_definition_load_is_marked() {
    run(|| {
        let mut x = fx();
        let wi = x.wi;
        let class = x.class("BodyProbe");
        let path = ow_core::ObjectPath::parse("Engine.BodyProbe_Collider").unwrap();
        let vm = &mut x.w.vm;
        let arch = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let target = crate::actors::object(&x.w.vm.space, arch, "Body").unwrap().unwrap();
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, target));
        let req = SpawnRequest::new(Some(class)).with_template(Some(arch)).with_no_collision_fail(true);
        match spawn(&mut x.w.vm, wi, &req) {
            Err(VmError::NotYet(n)) => assert!(n.what.starts_with(crate::actors::PARTIAL_WRITES), "{n}"),
            other => panic!("expected a marked NotYet, got {other:?}"),
        }
        assert!(!x.w.vm.defs.is_unloaded(&x.w.vm.space, target), "the load happened before the refusal");
    });
}

/// The run-time objects of `classes` (an actor and its component copies are made `Origin::Runtime`).
fn runtime_of(x: &Fx, classes: &[ow_object::ids::ClassId]) -> usize {
    let space = &x.w.vm.space;
    space
        .heap
        .handles()
        .into_iter()
        .filter(|&o| {
            space.heap.get(o).is_ok_and(|v| v.origin == ow_object::heap::Origin::Runtime && classes.contains(&v.class))
        })
        .count()
}

/// Review r1 of #528, items 1 to 3: a definition the actor points to whose load itself fails (`Toolbox_Cycle`, the
/// store's `NotYet { NotRead }`) refuses the spawn with that `NotYet`, its kind kept, and marked, since the load (and
/// the plan's component load) moved the counters; the actor and the component copy are not left behind. The next
/// spawn from the same archetype meets the cached failure: the same typed refusal, unmarked (nothing was written).
#[test]
fn a_definition_load_that_fails_refuses_every_spawn_with_its_kind_and_leaves_nothing() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let toolbox = x.class("Toolbox");
        let component = x.class("ToyComponent");
        x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Toolbox").unwrap();
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Broken").unwrap();
        let arch = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        let Value::Object(Some(cycle)) = field(&x.w.vm.space, arch, "Thing").unwrap() else {
            panic!("no Thing handle");
        };
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, cycle) && !x.w.vm.defs.is_failed(&x.w.vm.space, cycle));
        let (made, list) = (runtime_of(&x, &[toolbox, component]), x.w.actors().borrow().list().len());
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(arch)).with_no_collision_fail(true);
        let first = match spawn(&mut x.w.vm, wi, &req) {
            Err(VmError::NotYet(n)) => n,
            other => panic!("expected the load's NotYet, got {other:?}"),
        };
        assert_eq!(first.missing, Missing::NotRead);
        assert!(first.what.starts_with("stopped after partial writes: "), "{first}");
        assert!(first.what.contains("archetype chain is a cycle"), "{first}");
        assert!(x.w.vm.defs.is_failed(&x.w.vm.space, cycle), "the load ran and failed");
        assert_eq!(runtime_of(&x, &[toolbox, component]), made, "no actor and no component copy left");
        assert_eq!(x.w.actors().borrow().list().len(), list, "nothing in the level list");
        let second = match spawn(&mut x.w.vm, wi, &req) {
            Err(VmError::NotYet(n)) => n,
            other => panic!("expected the cached NotYet, got {other:?}"),
        };
        assert_eq!(second.missing, Missing::NotRead);
        assert_eq!(format!("stopped after partial writes: {}", second.what), first.what, "the same error, unmarked");
        assert_eq!(runtime_of(&x, &[toolbox, component]), made);
        assert_eq!(x.w.actors().borrow().list().len(), list);
    });
}

/// Review r1 of #528, item 4: building a cold class default object is a write, so a refusal after it is marked; with
/// the class default warm, the same refusal (here a destroyed template) writes nothing and is not.
#[test]
fn a_cold_class_default_marks_the_refusal_after_it() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let toolbox = x.class("Toolbox");
        let dead = x.w.vm.space.alloc(toolbox, "Toolbox_Dead", None, ow_object::heap::Origin::Runtime).unwrap();
        x.w.vm.space.heap.destroy(dead).unwrap();
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(dead)).with_no_collision_fail(true);
        for marked in [true, false] {
            match spawn(&mut x.w.vm, wi, &req) {
                Err(VmError::NotYet(n)) => {
                    assert!(n.what.contains("destroyed actor as its ActorTemplate"), "{n}");
                    assert_eq!(n.what.starts_with(crate::actors::PARTIAL_WRITES), marked, "{n}");
                }
                other => panic!("expected NotYet, got {other:?}"),
            }
        }
    });
}

/// Review r1 of #528, item 4: a template whose load fails afresh is marked (the failed load is a transition); the
/// cached failure on the next spawn returns the same typed error unmarked.
#[test]
fn a_cached_template_failure_is_not_marked() {
    run(|| {
        let mut x = fx_files(true, ParityMode::Off, true);
        let wi = x.wi;
        let toolbox = x.class("Toolbox");
        x.w.vm.defs.class_default(&mut x.w.vm.space, "Engine.Toolbox").unwrap();
        let path = ow_core::ObjectPath::parse("Engine.Toolbox_Broken").unwrap();
        let arch = x.w.vm.defs.find(&mut x.w.vm.space, &path).unwrap().unwrap();
        let Value::Object(Some(cycle)) = field(&x.w.vm.space, arch, "Thing").unwrap() else {
            panic!("no Thing handle");
        };
        let req = SpawnRequest::new(Some(toolbox)).with_template(Some(cycle)).with_no_collision_fail(true);
        for marked in [true, false] {
            match spawn(&mut x.w.vm, wi, &req) {
                Err(VmError::NotYet(n)) => {
                    assert_eq!(n.missing, Missing::NotRead);
                    assert!(n.what.contains("archetype chain is a cycle"), "{n}");
                    assert_eq!(n.what.starts_with(crate::actors::PARTIAL_WRITES), marked, "{n}");
                }
                other => panic!("expected NotYet, got {other:?}"),
            }
        }
    });
}

/// Lane IMPL-SETSCALE: `PrimitiveComponent.SetScale` is bound in Off only; Reference keeps the stub policy.
#[test]
fn set_scale_is_bound_outside_reference_only() {
    run(|| {
        let x = fx();
        assert!(x.w.vm.has_native(crate::world::primitive_scale::SET_SCALE));
        let x = fx_in(true, ParityMode::Reference);
        assert!(!x.w.vm.has_native(crate::world::primitive_scale::SET_SCALE), "unbound under Reference");
    });
}
