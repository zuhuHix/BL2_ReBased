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
    ] {
        t.prop("Bool", actor, b, 0, 0);
    }
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
    for a in ["Attached", "Touching", "GeneratedEvents"] {
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
    // The game info the world's hooks fall back to (decision 2026-11-16-49).
    t.prop("Object", world_info, "Game", 0, actor);
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
    let controller = t.class("Controller", actor);
    let player_controller = t.class("PlayerController", controller);
    t.prop("Object", player_controller, "Player", 0, object);
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
    // The travel countdown (lane MAP-LOAD-C, NATIVE_TRAVEL.md).
    let willow_game = t.class("WillowGameInfo", actor);
    t.prop("Int", willow_game, "TravelPostCountdownTime", 0, 0);
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
    let dir = std::env::temp_dir().join(format!(
        "ow-world-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", &bytes)],
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
            sink.borrow_mut()
                .push((c.this.expect("Note on an actor"), k));
            Ok(Value::None)
        }),
    )
    .unwrap();
    let mut w = World::new(vm, BalanceState::new(snapshot()), &[&natives_core::Pack]).unwrap();
    for class in ["Probe", "Dud", "Controller", "PlayerController", "WorldInfo"] {
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
    Fx { w, wi, notes, dir }
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
        let e = spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap_err();
        assert!(matches!(vm_missing(e), Missing::LayerAbsent(_)));
        assert_eq!(x.w.actors().borrow().list().len(), before, "nothing made");
        set_field(&mut x.w.vm.space, cdo, "bCollideActors", Value::Bool(false)).unwrap();
        set_field(&mut x.w.vm.space, cdo, "bNoDelete", Value::Bool(true)).unwrap();
        assert_eq!(
            spawn(&mut x.w.vm, wi, &SpawnRequest::new(Some(probe))).unwrap(),
            None,
            "no-delete after play began"
        );
    });
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

#[test]
fn destroy_is_not_yet_where_this_world_cannot_run_the_routine() {
    run(|| {
        let mut x = fx();
        let ctrl = x.spawn("Controller");
        assert!(matches!(
            vm_missing(destroy(&mut x.w.vm, ctrl).unwrap_err()),
            Missing::NotRead
        ));
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
fn a_controller_is_the_cxx_stand_in_in_reference_and_not_yet_in_off() {
    run(|| {
        // Off: the controller and player case waits for its implementer; nothing changes before the refusal.
        let mut x = fx();
        let ctrl = x.spawn("Controller");
        match destroy(&mut x.w.vm, ctrl).unwrap_err() {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert_eq!(n.record, crate::actors::RECORD_CONTROLLER);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(field(&x.w.vm.space, ctrl, "bDeleteMe").unwrap(), Value::Bool(false));
        assert_eq!(field(&x.w.vm.space, ctrl, "bPendingDelete").unwrap(), Value::Bool(false));

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

/// `UIInteraction.GetDataStoreClient` (decision 2026-11-19-19): `NotYet { LayerAbsent }` outside Reference (no world
/// models the engine's data store client); unbound under Reference.
#[test]
fn get_data_store_client_is_not_yet_outside_reference() {
    run(|| {
        let mut x = fx();
        let r = x.call("UIInteraction.GetDataStoreClient", None, Vec::new());
        assert!(matches!(&r, Err(VmError::NotYet(n)) if matches!(n.missing, ow_core::Missing::LayerAbsent(_))), "{r:?}");
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
        // A run-time object with no outer of the same name is skipped, and no package holds Engine.Nothing.
        let thing = x.class("Thing");
        x.vm().space.alloc(thing, "Nothing", None, ow_object::heap::Origin::Runtime).unwrap();
        assert_eq!(find(&mut x, "Engine.Nothing").unwrap(), Value::Object(None));
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
