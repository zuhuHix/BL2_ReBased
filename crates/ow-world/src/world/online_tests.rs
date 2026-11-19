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
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
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
    t.function(object, "SaveConfig", &[], None, &[], &FunctionSpec::new(NATIVE, "SaveConfig"), |_| None);
    let pc = t.class("PlayerController", object);
    let params = [Param::new("Str", "NewOption"), Param::new("Str", "NewValue"), Param::new("Bool", "bSaveDefault")];
    t.function(pc, "UpdateURL", &params, None, &[], &FunctionSpec::new(NATIVE, "UpdateURL"), |_| None);
    let base = t.class("Engine", object);
    let game = t.class("GameEngine", base);
    t.prop("Object", game, "OnlineSubsystem", 0, object);
    t.class("ToySubsystem", object);
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
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ByteProperty", "IntProperty"]);
    let object = t.class("Object", 0);
    let sub = t.class("OnlineSubsystemSteamworks", object);
    t.prop("Byte", sub, "LoggedInStatus", 0, 0);
    t.prop("Int", sub, "LoggedInPlayerNum", 0, 0);
    let spec = FunctionSpec::new(NATIVE, "GetLoginStatus");
    let params = [Param::new("Byte", "LocalUserNum")];
    t.function(sub, "GetLoginStatus", &params, Some(("Byte", 0)), &[], &spec, |_| None);
    t.p.build()
}

/// `WillowGame.upk`: `WillowPlayerInput` with its native `SetupInputDevices`, and `WillowPlayerController` with its
/// `InputContextSelector`, its native `UpdateInputContext` and a script `ToyInputTail` cut to the installed
/// `SetupInputDevices` tail: clear `InputContextSelector`, then call `UpdateInputContext`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let input = t.class("WillowPlayerInput", object);
    let spec = FunctionSpec::new(NATIVE, "SetupInputDevices");
    t.function(input, "SetupInputDevices", &[], None, &[], &spec, |_| None);
    let pc = t.class("WillowPlayerController", object);
    let selector = t.prop("Object", pc, "InputContextSelector", 0, object);
    let spec = FunctionSpec::new(NATIVE, "UpdateInputContext");
    let update = t.function(pc, "UpdateInputContext", &[], None, &[], &spec, |_| None).function;
    t.function(pc, "ToyInputTail", &[], None, &[], &FunctionSpec::new(SCRIPT, "ToyInputTail"), |_| {
        let mut a = Asm::new();
        a.stmt().raw(&[0x0F]).instance(selector).raw(&[0x2A]);
        a.stmt().final_function(update, |_| {});
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
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
    let (e, s, w) = (engine(), steamworks(), willow());
    let files: [(&str, &[u8]); 3] = [("Engine.upk", &e), ("OnlineSubsystemSteamworks.upk", &s), ("WillowGame.upk", &w)];
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

/// The natives after the login status: each its own named `NotYet { NotRead }` when reached, in the script's order.
/// The toy tail clears `InputContextSelector` before `UpdateInputContext` refuses, and the clear stays (no rollback;
/// the caller marks the stop terminal). `SetupInputDevices`, `UpdateURL` and `SaveConfig` refuse by themselves.
#[test]
fn the_natives_after_the_login_status_are_each_named_not_read() {
    let mut t = toy(ParityMode::Off, true);
    let not_read = |r: Result<Value, VmError>, name: &str| match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{name}");
            assert_eq!(n.record, RECORD);
            assert!(n.what.starts_with(name), "{}", n.what);
        }
        other => panic!("{name}: {other:?}"),
    };
    let pc = t.make("WillowGame.WillowPlayerController");
    let marker = t.make("WillowGame.WillowPlayerInput");
    t.set(pc, "InputContextSelector", Value::Object(Some(marker)));
    not_read(t.call("WillowGame.WillowPlayerController.ToyInputTail", Some(pc), vec![]), "WillowPlayerController.UpdateInputContext");
    let c = t.vm.space.heap.get(pc).unwrap().class;
    let p = t.vm.space.prop(c, "InputContextSelector").unwrap();
    assert_eq!(*t.vm.space.read(pc, p.into()).unwrap(), Value::Object(None), "the clear came first and stays");
    not_read(t.call("WillowGame.WillowPlayerInput.SetupInputDevices", Some(marker), vec![]), "WillowPlayerInput.SetupInputDevices");
    let epc = t.make("Engine.PlayerController");
    let args = vec![Value::Str("Name".into()), Value::Str("Maya".into()), Value::Bool(true)];
    not_read(t.call("Engine.PlayerController.UpdateURL", Some(epc), args), "PlayerController.UpdateURL");
    not_read(t.call("Engine.Object.SaveConfig", Some(epc), vec![]), "Object.SaveConfig");
}
