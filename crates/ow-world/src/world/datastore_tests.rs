//! Tests of the engine's data store client and the online game settings route (decision 2026-11-19-65) on invented
//! packages (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::cell::Cell;
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
use ow_package::testkit::script::{Asm, CPF_OPT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC: u32 = 0x2000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;

/// `Engine.upk`: `UIInteraction.GetDataStoreClient`, `DataStoreClient.FindDataStore`, `UIDataStore` (its `Tag`),
/// `Settings.SetIntProperty`, `GameEngine.GetDLCManager` and the online subsystem's
/// `OnlineGameInterface.GetGameSettings` (branch 1, which nothing binds). `with_client` false leaves `DataStoreClient`
/// out.
fn engine(with_client: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "NameProperty", "ObjectProperty", "IntProperty"]);
    let object = t.class("Object", 0);
    let ui = t.class("UIInteraction", object);
    let spec = FunctionSpec::new(NATIVE | STATIC, "GetDataStoreClient");
    t.function(ui, "GetDataStoreClient", &[], Some(("Object", object)), &[], &spec, |_| None);
    if with_client {
        let client = t.class("DataStoreClient", object);
        let params = [
            Param { type_ref: 0, ..Param::new("Name", "Tag") },
            Param { flags: CPF_OPT, type_ref: object, ..Param::new("Object", "Player") },
        ];
        let spec = FunctionSpec::new(NATIVE, "FindDataStore");
        t.function(client, "FindDataStore", &params, Some(("Object", object)), &[], &spec, |_| None);
    }
    let store = t.class("UIDataStore", object);
    t.prop("Name", store, "Tag", 0, 0);
    let settings = t.class("Settings", object);
    let spec = FunctionSpec::new(NATIVE, "SetIntProperty");
    let params = [Param::new("Int", "SettingId"), Param::new("Int", "Value")];
    t.function(settings, "SetIntProperty", &params, None, &[], &spec, |_| None);
    let engine_class = t.class("GameEngine", object);
    let spec = FunctionSpec::new(NATIVE | STATIC, "GetDLCManager");
    t.function(engine_class, "GetDLCManager", &[], Some(("Object", object)), &[], &spec, |_| None);
    let online = t.class("OnlineGameInterface", object);
    let spec = FunctionSpec::new(NATIVE, "GetGameSettings");
    t.function(online, "GetGameSettings", &[Param::new("Name", "SessionName")], Some(("Object", object)), &[], &spec, |_| None);
    t.p.build()
}

/// `WillowGame.upk`: the online game settings store (its class defaults' `Tag` `WillowOnlineGameSettings`), the
/// replication info's `GetCurrPlaythrough`, the DLC manager's two queries, and a toy `WillowPlayerController`:
/// - its natives `GetActiveMissionNumber` and `GetActivePlotCriticalMissionNumber`;
/// - `GetOnlineGameSettingsDatastore`, a native here standing for the store lookup script: it asks the real
///   `GetDataStoreClient` and `FindDataStore` (or answers None while the test turns the route off);
/// - the script `GetOnlineGameSettingsViaDatastore` (empty body: Off overrides it);
/// - the script `UpdateOnlineGameSettings`, the refresh: it calls `GetOnlineGameSettingsViaDatastore` and returns.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "IntProperty", "ObjectProperty"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let base = t.p.add_import("Core", "Class", engine, "UIDataStore");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let store = t.class("WillowOnlineGameSettings_DataStore", base);
    let mut defaults = ow_package::testkit::ints(&[0]);
    let tag = t.p.fname("WillowOnlineGameSettings", 0);
    defaults.extend(t.p.tag("Tag", "NameProperty", &tag));
    defaults.extend_from_slice(&t.none);
    t.p.add_export(store, "Default__WillowOnlineGameSettings_DataStore", defaults);
    let gri = t.class("WillowGameReplicationInfo", object);
    let spec = FunctionSpec::new(NATIVE, "GetCurrPlaythrough");
    t.function(gri, "GetCurrPlaythrough", &[], Some(("Int", 0)), &[], &spec, |_| None);
    let dlc = t.class("WillowDownloadableContentManager", object);
    for name in ["HasCompatibilityData", "StaticGetDlcContentIdForCurrentMap"] {
        let spec = FunctionSpec::new(NATIVE, name);
        t.function(dlc, name, &[], Some(("Int", 0)), &[], &spec, |_| None);
    }
    let pc = t.class("WillowPlayerController", object);
    for name in ["GetActiveMissionNumber", "GetActivePlotCriticalMissionNumber"] {
        let spec = FunctionSpec::new(NATIVE, name);
        t.function(pc, name, &[], Some(("Int", 0)), &[], &spec, |_| None);
    }
    let spec = FunctionSpec::new(NATIVE, "GetOnlineGameSettingsDatastore");
    t.function(pc, "GetOnlineGameSettingsDatastore", &[], Some(("Object", object)), &[], &spec, |_| None);
    let empty = |_: &ow_package::testkit::script::FunctionIds| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    };
    let spec = FunctionSpec::new(SCRIPT, "GetOnlineGameSettingsViaDatastore");
    let via = t.function(pc, "GetOnlineGameSettingsViaDatastore", &[], Some(("Object", object)), &[], &spec, empty).function;
    let spec = FunctionSpec::new(SCRIPT, "UpdateOnlineGameSettings");
    t.function(pc, "UpdateOnlineGameSettings", &[], None, &[], &spec, |_| {
        let mut a = Asm::new();
        a.stmt().final_function(via, |_| {});
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.p.build()
}

struct Toy {
    vm: Vm,
    dir: PathBuf,
    /// Whether the toy store lookup asks the real getter and lookup (else it answers None).
    route: Rc<Cell<bool>>,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn toy(with_client: bool) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-world-ds-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(with_client), willow());
    let files: [(&str, &[u8]); 2] = [("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let route = Rc::new(Cell::new(true));
    let on = route.clone();
    let f = vm.find_function("WillowGame.WillowPlayerController.GetOnlineGameSettingsDatastore").unwrap();
    let key = vm.function(f).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| -> Result<Value, VmError> {
            if !on.get() {
                return Ok(Value::Object(None));
            }
            // As the script does: the engine's client, then the store by its tag.
            let get = c.vm.find_function(GET_DATA_STORE_CLIENT)?;
            let client = c.vm.call(get, None, vec![])?.ret;
            let Value::Object(Some(client)) = client else {
                return Ok(Value::Object(None));
            };
            let find = c.vm.find_function(FIND_DATA_STORE)?;
            let tag = Value::Name(c.vm.space.names.intern("WillowOnlineGameSettings"));
            Ok(c.vm.call(find, Some(client), vec![tag])?.ret)
        }),
    )
    .unwrap();
    register(&mut vm).unwrap();
    Toy { vm, dir, route }
}

impl Toy {
    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        Ok(self.vm.call(f, this, args)?.ret)
    }

    fn name(&mut self, s: &str) -> Value {
        Value::Name(self.vm.space.names.intern(s))
    }

    fn make(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn tag_of(&self, o: ObjRef) -> String {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "Tag").unwrap();
        match self.vm.space.read(o, PropAddr::scalar(p)).unwrap() {
            Value::Name(n) => self.vm.space.names.text(*n).to_string(),
            other => panic!("{other:?}"),
        }
    }

    fn client(&mut self) -> ObjRef {
        match self.call(GET_DATA_STORE_CLIENT, None, vec![]).unwrap() {
            Value::Object(Some(c)) => c,
            other => panic!("{other:?}"),
        }
    }
}

fn not_yet(r: Result<Value, VmError>, missing: fn(&Missing) -> bool, what: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert!(missing(&n.missing), "{n}");
            assert!(n.what.contains(what), "{n}");
        }
        other => panic!("expected NotYet naming {what:?}, got {other:?}"),
    }
}

/// `GetDataStoreClient` answers the world's one client, the same on every call, made with the online game settings
/// store from its class defaults (its `Tag`).
#[test]
fn the_client_is_the_worlds_with_the_online_game_settings_store() {
    let mut t = toy(true);
    let client = t.client();
    assert_eq!(t.client(), client, "the same client");
    let s = t.vm.service::<DataStores>().unwrap();
    let globals = s.borrow().globals().to_vec();
    assert_eq!(globals.len(), 1);
    assert_eq!(t.tag_of(globals[0]), "WillowOnlineGameSettings");
    assert_eq!(s.borrow().client(), Some(client));
}

/// `FindDataStore` (review r2 of #448, item 1): None for a None tag; the global store by its tag (any case), with or
/// without a player (the player's own stores come first and the port's players hold none); None for a tag no store
/// has; `NotYet` only on another client. Nothing is made by any of them.
#[test]
fn find_data_store_searches_the_player_then_the_global_stores() {
    let mut t = toy(true);
    let client = t.client();
    let store = t.vm.service::<DataStores>().unwrap().borrow().globals()[0];
    let player = t.make("Engine.Object", "ToyPlayer");
    let heap = t.vm.space.heap.len();
    let none = t.name("None");
    assert_eq!(t.call(FIND_DATA_STORE, Some(client), vec![none]).unwrap(), Value::Object(None));
    for tag in ["WillowOnlineGameSettings", "willowonlinegamesettings"] {
        let v = t.name(tag);
        assert_eq!(t.call(FIND_DATA_STORE, Some(client), vec![v.clone()]).unwrap(), Value::Object(Some(store)), "{tag}");
        let with_player = t.call(FIND_DATA_STORE, Some(client), vec![v, Value::Object(Some(player))]);
        assert_eq!(with_player.unwrap(), Value::Object(Some(store)), "{tag} with a player");
    }
    let unknown = t.name("ToyUnknownStore");
    assert_eq!(t.call(FIND_DATA_STORE, Some(client), vec![unknown]).unwrap(), Value::Object(None));
    let tag = t.name("WillowOnlineGameSettings");
    not_yet(t.call(FIND_DATA_STORE, Some(player), vec![tag]), |m| *m == Missing::NotPorted, "other than the world's");
    assert_eq!(t.vm.space.heap.len(), heap, "nothing made");
}

/// All seven natives after the settings are `NotYet { NotRead }` naming the note (review r2 of #448, item 5); the
/// online subsystem's `GetGameSettings` (branch 1) is bound by nothing: it is bypassed, not refused.
#[test]
fn the_natives_after_the_settings_are_not_yet_and_branch_1_is_unbound() {
    let mut t = toy(true);
    for path in UNREAD {
        let class_path = path.rsplit_once('.').unwrap().0;
        let me = t.make(class_path, "ToyThis");
        let f = t.vm.find_function(path).unwrap();
        let n = t.vm.function(f).unwrap().params.len();
        let args = vec![Value::Int(1); n];
        not_yet(t.call(path, Some(me), args), |m| *m == Missing::NotRead, "behind the online game settings refresh");
    }
    let f = t.vm.find_function("Engine.OnlineGameInterface.GetGameSettings").unwrap();
    let key = t.vm.function(f).unwrap().native_key.clone().unwrap();
    assert!(!t.vm.has_native(&key), "branch 1 binds nothing");
}

/// The refresh, run as script through the real getter and lookup (review r2 of #448, items 2 and 5; r3, item 2):
/// with no store, the route answers None and the refresh returns; the first refresh with the route on makes and
/// publishes the client and store, and its settings-entry refusal (open question 2) is marked as after partial
/// writes, its kind kept; the client and store stay published; a later refresh's refusal is not marked.
#[test]
fn the_refresh_stops_at_the_settings_entry_read() {
    let mut t = toy(true);
    let pc = t.make("WillowGame.WillowPlayerController", "ToyPC");
    let refresh = "WillowGame.WillowPlayerController.UpdateOnlineGameSettings";
    t.route.set(false);
    t.call(refresh, Some(pc), vec![]).unwrap();
    assert!(t.vm.service::<DataStores>().unwrap().borrow().client().is_none(), "nothing made with the route off");
    t.route.set(true);
    match t.call(refresh, Some(pc), vec![]).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.starts_with(PARTIAL_WRITES) && n.what.contains("open question 2"), "{n}");
            assert_eq!(n.record, RECORD);
        }
        e => panic!("expected NotYet, got {e}"),
    }
    let s = t.vm.service::<DataStores>().unwrap();
    let (client, globals) = (s.borrow().client(), s.borrow().globals().to_vec());
    assert!(client.is_some() && globals.len() == 1, "the client and store stay published");
    match t.call(refresh, Some(pc), vec![]).unwrap_err() {
        VmError::NotYet(n) => assert!(!n.what.contains(PARTIAL_WRITES) && n.what.contains("open question 2"), "{n}"),
        e => panic!("expected NotYet, got {e}"),
    }
    // The lookup alone is not refused: a caller that only wants the store gets it.
    let tag = t.name("WillowOnlineGameSettings");
    assert_eq!(t.call(FIND_DATA_STORE, client, vec![tag]).unwrap(), Value::Object(Some(globals[0])));
}

/// The service's borrow and every class are taken before anything is made (review r2 of #448, item 3): a held borrow,
/// or a missing class, refuses with no instance left behind and no client kept.
#[test]
fn a_client_that_cannot_be_made_leaves_nothing() {
    let mut t = toy(false);
    let object = t.vm.find_class("Engine.Object").unwrap();
    let live = |t: &Toy| t.vm.space.heap.live(&t.vm.space.classes, object).count();
    let before = live(&t);
    assert!(t.call(GET_DATA_STORE_CLIENT, None, vec![]).is_err(), "no DataStoreClient class");
    assert_eq!(live(&t), before, "no instance left on the heap");
    let s = t.vm.service::<DataStores>().unwrap();
    assert!(s.borrow().client().is_none() && s.borrow().globals().is_empty());
    let mut t = toy(true);
    let before = live(&t);
    let s = t.vm.service::<DataStores>().unwrap();
    let held = s.borrow();
    assert!(matches!(data_store_client(&mut t.vm), Err(VmError::ServiceBorrowed(_))));
    drop(held);
    assert_eq!(live(&t), before, "nothing made behind the held borrow");
}

/// A `NotYet` after the getter began making objects is marked as after partial writes, its kind kept; any other
/// error passes unchanged (review r2 of #448, item 4).
#[test]
fn a_failure_after_the_first_write_is_marked() {
    let marked = after_writes(VmError::NotYet(ow_core::NotYet::not_read("an invented NotYet", "an invented record")));
    match marked {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.starts_with(PARTIAL_WRITES), "{n}");
        }
        e => panic!("{e}"),
    }
    let other = after_writes(VmError::ServiceBorrowed("an invented service"));
    assert!(matches!(other, VmError::ServiceBorrowed("an invented service")));
}
