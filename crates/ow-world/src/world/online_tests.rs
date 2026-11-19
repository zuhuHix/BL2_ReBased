//! Tests of the login status path's natives (decision 2026-11-19-95) on invented packages (AGENTS.md rule 9).
//! AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, FunctionIds, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativePack, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC: u32 = 0x2000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;

/// `Engine.upk`: `Engine` and `GameEngine` below it (its `OnlineSubsystem` object slot, the static native
/// `GetOnlineSubsystem`, and a script `Probe` that returns that native's answer), and `ToySubsystem`.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty", "StrProperty", "BoolProperty"]);
    let object = t.class("Object", 0);
    let save = t.function(object, "SaveConfig", &[], None, &[], &FunctionSpec::new(NATIVE, "SaveConfig"), |_| None).function;
    let pc = t.class("PlayerController", object);
    let params = [Param::new("Str", "NewOption"), Param::new("Str", "NewValue"), Param::new("Bool", "bSaveDefault")];
    t.function(pc, "UpdateURL", &params, None, &[], &FunctionSpec::new(NATIVE, "UpdateURL"), |_| None);
    // The name change (section 6.2): the controller's WorldInfo, the WorldInfo's Game, a script ServerChangeName (its
    // body is replaced in Off), the base GameInfo.ChangeName whose invented body calls SaveConfig (so a test sees it
    // ran), and ToyGameInfo's override, whose body would call SaveConfig too.
    let world = t.class("WorldInfo", object);
    let info = t.class("GameInfo", object);
    t.prop("Object", world, "Game", 0, info);
    t.prop("Object", pc, "WorldInfo", 0, world);
    let spec = FunctionSpec::new(SCRIPT, "ServerChangeName");
    t.function(pc, "ServerChangeName", &[Param::new("Str", "S")], None, &[], &spec, |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    let other = Param { kind: "Object".into(), name: "Other".into(), flags: 0, type_ref: object };
    let change = [other, Param::new("Str", "S"), Param::new("Bool", "bNameChange")];
    let saves = |_: &FunctionIds| {
        let mut a = Asm::new();
        a.stmt().final_function(save, |_| {});
        a.stmt().return_nothing();
        a.end();
        Some(a)
    };
    t.function(info, "ChangeName", &change, None, &[], &FunctionSpec::new(SCRIPT, "ChangeName"), saves);
    let toy_info = t.class("ToyGameInfo", info);
    t.function(toy_info, "ChangeName", &change, None, &[], &FunctionSpec::new(SCRIPT, "ChangeName"), saves);
    t.class("ToyInterface", object);
    let base = t.class("Engine", object);
    let game = t.class("GameEngine", base);
    t.prop("Object", game, "OnlineSubsystem", 0, object);
    let toy_sub = t.class("ToySubsystem", object);
    t.prop("Object", toy_sub, "PlayerInterface", 0, object);
    let spec = FunctionSpec::new(NATIVE | STATIC, "GetOnlineSubsystem");
    let get = t.function(game, "GetOnlineSubsystem", &[], Some(("Object", object)), &[], &spec, |_| None).function;
    t.function(game, "Probe", &[], Some(("Object", object)), &[], &FunctionSpec::new(SCRIPT, "Probe"), |_| {
        let mut a = Asm::new();
        a.stmt().raw(&[0x04]).final_function(get, |_| {});
        a.end();
        Some(a)
    });
    t.p.build()
}

/// `OnlineSubsystemSteamworks.upk`: the subsystem class, its two login state fields and its native
/// `GetLoginStatus(LocalUserNum)`.
fn steamworks() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ByteProperty", "IntProperty", "ObjectProperty"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let sub = t.class("OnlineSubsystemSteamworks", object);
    t.prop("Object", sub, "PlayerInterface", 0, object);
    t.prop("Byte", sub, "LoggedInStatus", 0, 0);
    t.prop("Int", sub, "LoggedInPlayerNum", 0, 0);
    let spec = FunctionSpec::new(NATIVE, "GetLoginStatus");
    let params = [Param::new("Byte", "LocalUserNum")];
    t.function(sub, "GetLoginStatus", &params, Some(("Byte", 0)), &[], &spec, |_| None);
    t.p.build()
}

/// `WillowGame.upk`: `WillowPlayerInput` with its native `SetupInputDevices`, and `WillowPlayerController` with its
/// `InputContextSelector`, `PlayerInput`, native `UpdateInputContext` and script `SetupInputDevices`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let input = t.class("WillowPlayerInput", object);
    let spec = FunctionSpec::new(NATIVE, "SetupInputDevices");
    t.function(input, "SetupInputDevices", &[], None, &[], &spec, |_| None);
    let pc = t.class("WillowPlayerController", object);
    t.prop("Object", pc, "InputContextSelector", 0, object);
    t.prop("Object", pc, "PlayerInput", 0, object);
    let spec = FunctionSpec::new(NATIVE, "UpdateInputContext");
    t.function(pc, "UpdateInputContext", &[], None, &[], &spec, |_| None);
    // The script the Off override replaces (its invented body is never run in Off).
    t.function(pc, "SetupInputDevices", &[], None, &[], &FunctionSpec::new(SCRIPT, "SetupInputDevices"), |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.p.build()
}

/// `ToyEngines.upk`: an engine class of its own, not below `Engine.GameEngine`.
fn engines() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let object = t.class("Object", 0);
    t.class("ToyEngine", object);
    t.p.build()
}

struct Toy {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn toy(mode: ParityMode, bind: bool) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-world-online-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, s, w, n) = (engine(), steamworks(), willow(), engines());
    let files: [(&str, &[u8]); 4] =
        [("Engine.upk", &e), ("OnlineSubsystemSteamworks.upk", &s), ("WillowGame.upk", &w), ("ToyEngines.upk", &n)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    if bind {
        register(&mut vm).unwrap();
    }
    Toy { vm, dir }
}

impl Toy {
    fn make(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    }

    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        Ok(self.vm.call(f, this, args)?.ret)
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn engine(&mut self, class: &str, slot: Option<Value>) -> ObjRef {
        let e = self.make(class);
        if let Some(v) = slot {
            self.set(e, "OnlineSubsystem", v);
        }
        self.vm.add_service(EngineObject(e)).unwrap();
        e
    }
}

fn absent(r: Result<Value, VmError>, why: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert!(matches!(n.missing, Missing::LayerAbsent { .. }), "{n:?}");
            assert_eq!(n.record, RECORD);
            assert!(n.what.contains(why), "{}", n.what);
        }
        other => panic!("expected NotYet(LayerAbsent) naming {why:?}, got {other:?}"),
    }
}

/// The answers of `GetOnlineSubsystem`, which exclude each other (the note's section 7): no engine object is the named
/// `NotYet { LayerAbsent }` (never None); an engine that is not a `GameEngine` is None; an empty slot is None, the field
/// as stored; a subsystem whose interfaces a set-up initialised is that object, and one no set-up recorded is the named
/// `NotYet { NotRead }` for that initialisation (an unset `PlayerInterface` is not taken as status 0). Nothing is made
/// or changed by any of them.
#[test]
fn get_online_subsystem_has_four_exclusive_answers() {
    let mut t = toy(ParityMode::Off, true);
    absent(t.call(GET_ONLINE_SUBSYSTEM, None, vec![]), "the world has no engine object");
    let mut t = toy(ParityMode::Off, true);
    t.engine("Engine.Engine", None);
    assert_eq!(t.call(GET_ONLINE_SUBSYSTEM, None, vec![]).unwrap(), Value::Object(None), "not a GameEngine");
    let mut t = toy(ParityMode::Off, true);
    t.engine("Engine.GameEngine", None);
    assert_eq!(t.call(GET_ONLINE_SUBSYSTEM, None, vec![]).unwrap(), Value::Object(None), "an empty slot, as stored");
    let mut t = toy(ParityMode::Off, true);
    let sub = t.make("Engine.ToySubsystem");
    t.engine("Engine.GameEngine", Some(Value::Object(Some(sub))));
    match t.call(GET_ONLINE_SUBSYSTEM, None, vec![]) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains("player interface initialisation"), "{}", n.what);
        }
        other => panic!("no set-up record: {other:?}"),
    }
    t.vm.add_service(OnlineSetUp { interfaces: vec![sub], login_state: vec![] }).unwrap();
    let heap = t.vm.space.heap.len();
    assert_eq!(t.call(GET_ONLINE_SUBSYSTEM, None, vec![]).unwrap(), Value::Object(Some(sub)));
    assert_eq!(subsystem(&mut t.vm).unwrap(), Some(sub), "the read-only form agrees");
    assert_eq!(t.vm.space.heap.len(), heap, "nothing made");
}

/// Script that calls the native gets the typed `VmError::NotYet` with its kind and record (graded by variant): the
/// runtime check the note rules for, at the call.
#[test]
fn script_reaching_the_native_stops_on_the_typed_not_yet() {
    let mut t = toy(ParityMode::Off, true);
    let e = t.make("Engine.GameEngine");
    absent(t.call("Engine.GameEngine.Probe", Some(e), vec![]), "the world has no engine object");
}

/// `OnlineSubsystemSteamworks.GetLoginStatus` on a subsystem whose login state no set-up filled: `NotYet { NotRead }`
/// naming that state, whatever the number and whatever the fields hold (class defaults are not the game's state).
#[test]
fn steamworks_login_status_is_not_read_until_the_set_up_fills_it() {
    let mut t = toy(ParityMode::Off, true);
    let sub = t.make("OnlineSubsystemSteamworks.OnlineSubsystemSteamworks");
    t.set(sub, "LoggedInStatus", Value::Byte(2));
    for n in [0u8, 1, 255] {
        match t.call(STEAMWORKS_LOGIN_STATUS, Some(sub), vec![Value::Byte(n)]) {
            Err(VmError::NotYet(e)) => {
                assert_eq!(e.missing, Missing::NotRead);
                assert_eq!(e.record, RECORD);
                assert!(e.what.contains("login state"), "{}", e.what);
            }
            other => panic!("{n}: {other:?}"),
        }
    }
    // A record that lists another subsystem does not cover this one.
    let other = t.make("OnlineSubsystemSteamworks.OnlineSubsystemSteamworks");
    t.vm.add_service(OnlineSetUp { interfaces: vec![other, sub], login_state: vec![other] }).unwrap();
    assert!(matches!(t.call(STEAMWORKS_LOGIN_STATUS, Some(sub), vec![Value::Byte(0)]), Err(VmError::NotYet(_))));
}

/// With the set-up record, the rule of section 4: `LoggedInStatus` as stored for the number equal to
/// `LoggedInPlayerNum`, 0 for any other. It writes nothing.
#[test]
fn steamworks_login_status_follows_the_filled_state() {
    let mut t = toy(ParityMode::Off, true);
    let sub = t.make("OnlineSubsystemSteamworks.OnlineSubsystemSteamworks");
    t.set(sub, "LoggedInStatus", Value::Byte(2));
    t.set(sub, "LoggedInPlayerNum", Value::Int(1));
    t.vm.add_service(OnlineSetUp { interfaces: vec![], login_state: vec![sub] }).unwrap();
    for (n, want) in [(1u8, 2u8), (0, 0), (255, 0)] {
        assert_eq!(t.call(STEAMWORKS_LOGIN_STATUS, Some(sub), vec![Value::Byte(n)]).unwrap(), Value::Byte(want), "{n}");
    }
    t.set(sub, "LoggedInPlayerNum", Value::Int(255));
    assert_eq!(t.call(STEAMWORKS_LOGIN_STATUS, Some(sub), vec![Value::Byte(255)]).unwrap(), Value::Byte(2), "widened");
    t.set(sub, "LoggedInPlayerNum", Value::Int(-1));
    assert_eq!(t.call(STEAMWORKS_LOGIN_STATUS, Some(sub), vec![Value::Byte(255)]).unwrap(), Value::Byte(0), "not -1");
}

/// The world's pack binds both natives in Off and neither in Reference (R3), so Reference keeps the stub policy.
#[test]
fn the_pack_binds_both_in_off_and_neither_in_reference() {
    for (mode, bound) in [(ParityMode::Off, true), (ParityMode::Reference, false)] {
        let mut t = toy(mode, false);
        crate::world::Pack.register(&mut t.vm).unwrap();
        // Bound before either package is loaded: the keys are the ones the loaded declarations carry.
        let unread = UNREAD.iter().map(|&(k, _)| k);
        for key in [GET_ONLINE_SUBSYSTEM_KEY, STEAMWORKS_LOGIN_STATUS_KEY].into_iter().chain(unread) {
            assert_eq!(t.vm.has_native(key), bound, "{mode:?} {key}");
        }
        let unread = [
            "WillowGame.WillowPlayerInput.SetupInputDevices",
            "WillowGame.WillowPlayerController.UpdateInputContext",
            "Engine.PlayerController.UpdateURL",
            "Engine.Object.SaveConfig",
        ]
        .into_iter()
        .zip(UNREAD.iter().map(|&(k, _)| k));
        let known = [(GET_ONLINE_SUBSYSTEM, GET_ONLINE_SUBSYSTEM_KEY), (STEAMWORKS_LOGIN_STATUS, STEAMWORKS_LOGIN_STATUS_KEY)];
        for (path, key) in known.into_iter().chain(unread) {
            let class = path.rsplit_once('.').unwrap().0;
            t.vm.find_class(class).unwrap();
            let f = t.vm.find_function(path).unwrap();
            assert_eq!(t.vm.function(f).unwrap().native_key.as_deref(), Some(key), "{path}");
        }
    }
}

fn not_read(r: Result<Value, VmError>, starts: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n:?}");
            assert_eq!(n.record, RECORD);
            assert!(n.what.starts_with(starts), "expected {starts:?}: {}", n.what);
        }
        other => panic!("expected NotYet(NotRead) starting {starts:?}, got {other:?}"),
    }
}

/// `UpdateURL` and `SaveConfig` are each a named `NotYet { NotRead }` when reached.
#[test]
fn the_url_and_config_natives_are_each_named_not_read() {
    let mut t = toy(ParityMode::Off, true);
    let pc = t.make("Engine.PlayerController");
    let args = vec![Value::Str("Name".into()), Value::Str("Maya".into()), Value::Bool(true)];
    not_read(t.call("Engine.PlayerController.UpdateURL", Some(pc), args), "PlayerController.UpdateURL");
    not_read(t.call("Engine.Object.SaveConfig", Some(pc), vec![]), "Object.SaveConfig");
}

fn selector(t: &mut Toy, pc: ObjRef) -> Value {
    let c = t.vm.space.heap.get(pc).unwrap().class;
    let p = t.vm.space.prop(c, "InputContextSelector").unwrap();
    t.vm.space.read(pc, p.into()).unwrap().clone()
}

/// Review r1 of #479, item 1: `SetupInputDevices` in Off. Without a player input it clears `InputContextSelector`,
/// then `UpdateInputContext` refuses: terminal at this boundary, with the exact prefix, its kind and record kept, and
/// the clear in place. With a `WillowPlayerInput`, its native refuses first, before the clear, unmarked.
#[test]
fn setup_input_devices_marks_the_refusal_after_its_clear() {
    let mut t = toy(ParityMode::Off, true);
    let pc = t.make("WillowGame.WillowPlayerController");
    let marker = t.make("WillowGame.WillowPlayerInput");
    t.set(pc, "InputContextSelector", Value::Object(Some(marker)));
    not_read(
        t.call(SETUP_INPUT_DEVICES, Some(pc), vec![]),
        "stopped after partial writes: WillowPlayerController.UpdateInputContext",
    );
    assert_eq!(selector(&mut t, pc), Value::Object(None), "the clear came first and stays");

    t.set(pc, "InputContextSelector", Value::Object(Some(marker)));
    t.set(pc, "PlayerInput", Value::Object(Some(marker)));
    not_read(t.call(SETUP_INPUT_DEVICES, Some(pc), vec![]), "WillowPlayerInput.SetupInputDevices");
    assert_eq!(selector(&mut t, pc), Value::Object(Some(marker)), "refused before the clear");
}

/// Review r1 of #479, item 2: an engine object of a class that is not below `GameEngine` answers None while
/// `Engine.GameEngine` is not loaded, and the answer loads nothing.
#[test]
fn a_non_game_engine_answers_none_without_loading_game_engine() {
    let mut t = toy(ParityMode::Off, false);
    let e = t.make("ToyEngines.ToyEngine");
    t.vm.add_service(EngineObject(e)).unwrap();
    assert!(t.vm.space.classes.by_path(GAME_ENGINE).is_none(), "GameEngine is not loaded");
    let classes = t.vm.space.classes.len();
    assert_eq!(subsystem(&mut t.vm).unwrap(), None);
    assert!(t.vm.space.classes.by_path(GAME_ENGINE).is_none(), "still not loaded");
    assert_eq!(t.vm.space.classes.len(), classes, "no class loaded");
}

/// Review r1 of #479, item 3: an initialised subsystem's `PlayerInterface` receiver. Steamworks, or an empty
/// interface, gives the subsystem; another class is a named `NotYet { NotRead }` naming it, before any dispatch.
#[test]
fn a_non_steamworks_player_interface_is_named_not_read() {
    let mut t = toy(ParityMode::Off, true);
    let sub = t.make("Engine.ToySubsystem");
    t.engine("Engine.GameEngine", Some(Value::Object(Some(sub))));
    t.vm.add_service(OnlineSetUp { interfaces: vec![sub], login_state: vec![] }).unwrap();
    assert_eq!(subsystem(&mut t.vm).unwrap(), Some(sub), "an empty interface, modelled as empty");
    let steam = t.make("OnlineSubsystemSteamworks.OnlineSubsystemSteamworks");
    t.set(sub, "PlayerInterface", Value::Object(Some(steam)));
    assert_eq!(subsystem(&mut t.vm).unwrap(), Some(sub), "the Steamworks receiver");
    let other = t.make("Engine.ToyInterface");
    t.set(sub, "PlayerInterface", Value::Object(Some(other)));
    match subsystem(&mut t.vm) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains("Engine.ToyInterface"), "{}", n.what);
        }
        other => panic!("{other:?}"),
    }
}

/// Review r1 of #479, item 4: `ServerChangeName` in Off. An empty name, no `WorldInfo` and no game info return with
/// no effects; the base `GameInfo.ChangeName` runs as installed (its invented body reaches `SaveConfig`); an override
/// is a named `NotYet { NotRead }` before its body runs.
#[test]
fn server_change_name_guards_the_resolved_change_name() {
    let mut t = toy(ParityMode::Off, true);
    let pc = t.make("Engine.PlayerController");
    let name = |s: &str| vec![Value::Str(s.into())];
    assert_eq!(t.call(SERVER_CHANGE_NAME, Some(pc), name("")).unwrap(), Value::None, "empty name");
    assert_eq!(t.call(SERVER_CHANGE_NAME, Some(pc), name("Maya")).unwrap(), Value::None, "no WorldInfo");
    let world = t.make("Engine.WorldInfo");
    t.set(pc, "WorldInfo", Value::Object(Some(world)));
    assert_eq!(t.call(SERVER_CHANGE_NAME, Some(pc), name("Maya")).unwrap(), Value::None, "no game info");
    let base = t.make("Engine.GameInfo");
    t.set(world, "Game", Value::Object(Some(base)));
    not_read(t.call(SERVER_CHANGE_NAME, Some(pc), name("Maya")), "Object.SaveConfig");
    let over = t.make("Engine.ToyGameInfo");
    t.set(world, "Game", Value::Object(Some(over)));
    not_read(t.call(SERVER_CHANGE_NAME, Some(pc), name("Maya")), "PlayerController.ServerChangeName: the game info Engine.ToyGameInfo");
}

/// The slice worlds' offline engine object (decision 2026-11-19-NNN; port Off answer: the port has no online
/// subsystem, UNVERIFIED against a logged-in game). In Off it is a `GameEngine` whose `OnlineSubsystem` slot is empty,
/// so `GetOnlineSubsystem` answers None (no subsystem is made). A second registration is refused. In Reference it is
/// refused and no engine object is registered.
#[test]
fn the_offline_engine_object_has_an_empty_online_subsystem_slot() {
    let mut t = toy(ParityMode::Off, true);
    let engine = register_offline_engine(&mut t.vm).unwrap().expect("the toy declares Engine.GameEngine");
    assert_eq!(t.vm.service::<EngineObject>().unwrap().borrow().0, engine);
    let class = t.vm.space.heap.get(engine).unwrap().class;
    assert_eq!(t.vm.find_class(GAME_ENGINE).unwrap(), class, "a GameEngine");
    assert_eq!(t.call(GET_ONLINE_SUBSYSTEM, None, vec![]).unwrap(), Value::Object(None), "the empty-slot rule");
    assert!(matches!(register_offline_engine(&mut t.vm), Err(VmError::Registration { .. })), "only once");

    let mut t = toy(ParityMode::Reference, false);
    assert!(matches!(register_offline_engine(&mut t.vm), Err(VmError::Registration { .. })));
    assert!(t.vm.service::<EngineObject>().is_none(), "Reference keeps no engine object");
}

/// A package set with no `Engine.GameEngine` registers no engine object, and its `GetOnlineSubsystem` keeps the
/// no-engine `NotYet { LayerAbsent }`.
#[test]
fn a_package_set_without_game_engine_registers_no_engine_object() {
    let dir = std::env::temp_dir().join(format!("ow-world-online-noengine-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let n = engines();
    let files: [(&str, &[u8]); 1] = [("ToyEngines.upk", &n)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    assert_eq!(register_offline_engine(&mut vm).unwrap(), None);
    assert!(vm.service::<EngineObject>().is_none());
    absent(subsystem(&mut vm).map(Value::Object), "the world has no engine object");
    let _ = std::fs::remove_dir_all(&dir);
}
