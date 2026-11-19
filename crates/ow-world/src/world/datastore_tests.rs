//! Tests of the engine's data store client and the online game settings route (decision 2026-11-19-65) on invented
//! packages (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::cell::{Cell, RefCell};
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
use ow_vm::api::{NativeCall, NativeFn, Vm};
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
    /// What the fill toy's refresh does between its entry and its return (`ToyRefreshStep`; None: nothing).
    step: Rc<RefCell<Option<NativeFn>>>,
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
    Toy { vm, dir, route, step: Rc::new(RefCell::new(None)) }
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

/// The refresh, run as script through the real getter and lookup: with the route off it does nothing and makes
/// nothing. With the route on, the installed entry read is no longer overridden (decision 2026-11-19-80): the toy's
/// read (an empty script here) answers, the refresh returns, and the client and store stay published. The fill itself
/// is tested on the `ToyFill` packages below.
#[test]
fn the_refresh_reads_the_entry_without_an_override() {
    let mut t = toy(true);
    let pc = t.make("WillowGame.WillowPlayerController", "ToyPC");
    let refresh = "WillowGame.WillowPlayerController.UpdateOnlineGameSettings";
    t.route.set(false);
    t.call(refresh, Some(pc), vec![]).unwrap();
    assert!(t.vm.service::<DataStores>().unwrap().borrow().client().is_none(), "nothing made with the route off");
    t.route.set(true);
    t.call(refresh, Some(pc), vec![]).unwrap();
    let via = t.call(SETTINGS_VIA_DATASTORE, Some(pc), vec![]).unwrap();
    assert_eq!(via, Value::Object(None), "the toy's own (empty) script answers: nothing overrides it");
    // The empty toy script makes no lookup, so nothing is made: no override ran the route.
    assert!(t.vm.service::<DataStores>().unwrap().borrow().client().is_none(), "the route was not run for it");
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

// ---------------------------------------------------------------------------------------------------------------
// The settings fill (lane IMPL-SETTINGS-FILL, decision 2026-11-19-80), on the invented `ToyFill` packages.
// ---------------------------------------------------------------------------------------------------------------

/// `Engine.upk` for the fill (invented):
/// - the getter and the lookup, as in [`engine`];
/// - `UIDataStore` and `UIDataStore_OnlineGameSettings`, with its `GameSettingsCfgList` of `GameSettingsCfg`
///   (`GameSettingsClass`, `GameSettings`, `Provider`) and its `SettingsProviderClass`;
/// - `UIDataProvider` (native `AddPropertyNotificationChangeRequest`), `UIDynamicDataProvider` (`DataClass`) and
///   `UIDataProvider_Settings` (`Settings`, `SettingsArrayProviders` of ints, and a script `OnSettingValueUpdated`);
/// - `UIDynamicDataProvider.CleanupDataProvider`, a script the settings provider inherits, and the provider subclass
///   `ToySettingsProviderSub`, whose class defaults hold `SettingsArrayProviders` = [5];
/// - `Settings`, with `Properties` of `SettingsProperty` (`PropertyId`, `Data` of `SettingsData`: `Type`, `Value1`), the
///   enum `ESettingsDataType` (`SDT_Empty`, `SDT_Int32`, `SDT_Float`) and the native `SetIntProperty`;
/// - `GameEngine.GetDLCManager`, static native.
fn fill_engine(with_settings_provider: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "NameProperty",
        "ObjectProperty",
        "IntProperty",
        "ByteProperty",
        "ClassProperty",
        "StructProperty",
        "ArrayProperty",
        "ScriptStruct",
        "Enum",
    ]);
    let object = t.class("Object", 0);
    let class_tail = |o: i32| ow_package::testkit::ints(&[object, o]);
    let ui = t.class("UIInteraction", object);
    let spec = FunctionSpec::new(NATIVE | STATIC, "GetDataStoreClient");
    t.function(ui, "GetDataStoreClient", &[], Some(("Object", object)), &[], &spec, |_| None);
    let client = t.class("DataStoreClient", object);
    let params = [
        Param { type_ref: 0, ..Param::new("Name", "Tag") },
        Param { flags: CPF_OPT, type_ref: object, ..Param::new("Object", "Player") },
    ];
    let spec = FunctionSpec::new(NATIVE, "FindDataStore");
    t.function(client, "FindDataStore", &params, Some(("Object", object)), &[], &spec, |_| None);
    let strukt = |t: &mut ScriptToy, outer: i32, name: &str| {
        let mut e = ow_package::testkit::Export::new(t.import("ScriptStruct"), name, vec![0u8; 16]);
        e.outer = outer;
        t.p.add_export_with(e)
    };
    let array_of = |t: &mut ScriptToy, owner: i32, name: &str, kind: &str, type_ref: i32| {
        let array = t.p.next_export();
        t.prop("Array", owner, name, 0, array + 1);
        t.prop(kind, array, name, 0, type_ref);
    };
    let store = t.class("UIDataStore", object);
    t.prop("Name", store, "Tag", 0, 0);
    let settings_store = t.class("UIDataStore_OnlineGameSettings", store);
    let cfg = strukt(&mut t, settings_store, "GameSettingsCfg");
    t.prop_tail("Class", cfg, "GameSettingsClass", 0, &class_tail(object));
    t.prop("Object", cfg, "GameSettings", 0, object);
    t.prop("Object", cfg, "Provider", 0, object);
    array_of(&mut t, settings_store, "GameSettingsCfgList", "Struct", cfg);
    t.prop_tail("Class", settings_store, "SettingsProviderClass", 0, &class_tail(object));
    let provider = t.class("UIDataProvider", object);
    let spec = FunctionSpec::new(NATIVE, "AddPropertyNotificationChangeRequest");
    t.function(provider, "AddPropertyNotificationChangeRequest", &[], None, &[], &spec, |_| None);
    let dynamic = t.class("UIDynamicDataProvider", provider);
    t.prop_tail("Class", dynamic, "DataClass", 0, &class_tail(object));
    let spec = FunctionSpec::new(SCRIPT, "CleanupDataProvider");
    t.function(dynamic, "CleanupDataProvider", &[], None, &[], &spec, |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    if with_settings_provider {
        let settings_provider = t.class("UIDataProvider_Settings", dynamic);
        t.prop("Object", settings_provider, "Settings", 0, object);
        array_of(&mut t, settings_provider, "SettingsArrayProviders", "Int", 0);
        let spec = FunctionSpec::new(SCRIPT, "OnSettingValueUpdated");
        t.function(settings_provider, "OnSettingValueUpdated", &[], None, &[], &spec, |_| {
            let mut a = Asm::new();
            a.stmt().return_nothing();
            a.end();
            Some(a)
        });
        // A provider subclass that shadows Settings with an int (review r3 of #464, item 4).
        let shadow = t.class("ToyShadowProvider", settings_provider);
        t.prop("Int", shadow, "Settings", 0, 0);
        let sub = t.class("ToySettingsProviderSub", settings_provider);
        let mut defaults = ow_package::testkit::ints(&[0]);
        defaults.extend(t.p.tag("SettingsArrayProviders", "ArrayProperty", &ow_package::testkit::ints(&[1, 5])));
        defaults.extend_from_slice(&t.none);
        t.p.add_export(sub, "Default__ToySettingsProviderSub", defaults);
    }
    let settings = t.class("Settings", object);
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend_from_slice(&t.none);
    let entries = ["SDT_Empty", "SDT_Int32", "SDT_Float"];
    payload.extend(ow_package::testkit::ints(&[0, i32::try_from(entries.len()).unwrap()]));
    for e in entries {
        let n = t.p.fname(e, 0);
        payload.extend_from_slice(&n);
    }
    let mut x = ow_package::testkit::Export::new(t.import("Enum"), "ESettingsDataType", payload);
    x.outer = settings;
    let data_type = t.p.add_export_with(x);
    let data = strukt(&mut t, settings, "SettingsData");
    t.prop("Byte", data, "Type", 0, data_type);
    t.prop("Int", data, "Value1", 0, 0);
    let property = strukt(&mut t, settings, "SettingsProperty");
    t.prop("Int", property, "PropertyId", 0, 0);
    t.prop("Struct", property, "Data", 0, data);
    array_of(&mut t, settings, "Properties", "Struct", property);
    let spec = FunctionSpec::new(NATIVE, "SetIntProperty");
    let params = [Param::new("Int", "SettingId"), Param::new("Int", "Value")];
    t.function(settings, "SetIntProperty", &params, None, &[], &spec, |_| None);
    let engine_class = t.class("GameEngine", object);
    let spec = FunctionSpec::new(NATIVE | STATIC, "GetDLCManager");
    t.function(engine_class, "GetDLCManager", &[], Some(("Object", object)), &[], &spec, |_| None);
    t.p.build()
}

/// `WillowGame.upk` for the fill (invented): the store (an online game settings store, its `Tag` default), the
/// settings class `WillowOnlineGameSettings` (a `Settings`), the replication info with `CurrentPlaythrough` and
/// `GetCurrPlaythrough`, and a toy controller whose `GetActiveMissionNumber` is native, with the refresh
/// `UpdateOnlineGameSettings`: a script that calls the invented native `ToyRefreshStep` (the test's step) and returns.
fn fill_willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "IntProperty", "ObjectProperty"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let base = t.p.add_import("Core", "Class", engine, "UIDataStore_OnlineGameSettings");
    let settings = t.p.add_import("Core", "Class", engine, "Settings");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    // An object of a class no package exports (an engine-native class): the Off resolver makes a stand-in for it.
    let native_class = t.p.add_import("Core", "Class", engine, "ToyNativeThing");
    let things = t.p.add_import("Core", "Package", 0, "ToyThings");
    let native_thing = t.p.add_import("Engine", "ToyNativeThing", things, "ToyNativeObject");
    let store = t.class("WillowOnlineGameSettings_DataStore", base);
    t.prop("Object", store, "ToyNative", 0, native_class);
    let mut defaults = ow_package::testkit::ints(&[0]);
    let tag = t.p.fname("WillowOnlineGameSettings", 0);
    defaults.extend(t.p.tag("Tag", "NameProperty", &tag));
    defaults.extend(t.p.object_tag("ToyNative", native_thing));
    defaults.extend_from_slice(&t.none);
    t.p.add_export(store, "Default__WillowOnlineGameSettings_DataStore", defaults);
    t.class("WillowOnlineGameSettings", settings);
    let gri = t.class("WillowGameReplicationInfo", object);
    t.prop("Int", gri, "CurrentPlaythrough", 0, 0);
    let spec = FunctionSpec::new(NATIVE, "GetCurrPlaythrough");
    t.function(gri, "GetCurrPlaythrough", &[], Some(("Int", 0)), &[], &spec, |_| None);
    let pc = t.class("WillowPlayerController", object);
    let spec = FunctionSpec::new(NATIVE, "GetActiveMissionNumber");
    t.function(pc, "GetActiveMissionNumber", &[], Some(("Int", 0)), &[], &spec, |_| None);
    let spec = FunctionSpec::new(NATIVE, "ToyRefreshStep");
    let step = t.function(pc, "ToyRefreshStep", &[], None, &[], &spec, |_| None).function;
    let spec = FunctionSpec::new(SCRIPT, "UpdateOnlineGameSettings");
    t.function(pc, "UpdateOnlineGameSettings", &[], None, &[], &spec, |_| {
        let mut a = Asm::new();
        a.stmt().final_function(step, |_| {});
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.p.build()
}

/// The fill world: the store class default's `GameSettingsCfgList` set to one entry of `settings_class` (None: an
/// entry without a class), and its `SettingsProviderClass` to `provider_class`.
fn fill_toy(settings_class: Option<&str>, provider_class: Option<&str>) -> Toy {
    fill_toy_with(true, settings_class, provider_class)
}

/// `Core.upk` for the fill (invented): its `Object`, the root the Off resolver gives an engine-native stand-in class.
fn fill_core() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    t.class("Object", 0);
    t.p.build()
}

/// The fill world, cold: registered, with nothing loaded or made.
fn fill_cold(with_settings_provider: bool) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-world-fill-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w, c) = (fill_engine(with_settings_provider), fill_willow(), fill_core());
    let files: [(&str, &[u8]); 3] = [("Core.upk", &c), ("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    // `Core.Object` defined, as it always is in the installed game (the Off resolver roots stand-in classes there).
    vm.find_class("Core.Object").unwrap();
    let step: Rc<RefCell<Option<NativeFn>>> = Rc::new(RefCell::new(None));
    let slot = step.clone();
    let f = vm.find_function("WillowGame.WillowPlayerController.ToyRefreshStep").unwrap();
    let key = vm.function(f).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| -> Result<Value, VmError> {
            let f = slot.borrow().clone();
            match f {
                Some(f) => f(c),
                None => Ok(Value::None),
            }
        }),
    )
    .unwrap();
    register(&mut vm).unwrap();
    Toy { vm, dir, route: Rc::new(Cell::new(true)), step }
}

fn fill_toy_with(with_settings_provider: bool, settings_class: Option<&str>, provider_class: Option<&str>) -> Toy {
    let mut t = fill_cold(with_settings_provider);
    let vm = &mut t.vm;
    let cdo = vm.defs.class_default(&mut vm.space, GLOBAL_DATA_STORES[0]).unwrap();
    let cfg = vm.space.classes.struct_by_path("Engine.UIDataStore_OnlineGameSettings.GameSettingsCfg").unwrap();
    let mut entry = vm.space.classes.struct_defaults(cfg).unwrap();
    let class = settings_class.map(|c| vm.find_class(c).unwrap());
    struct_set(&vm.space, &mut entry, "GameSettingsClass", Value::Class(class), "test").unwrap();
    write_field(&mut vm.space, cdo, "GameSettingsCfgList", Value::array(vec![entry]), "test").unwrap();
    let provider = provider_class.map(|c| vm.find_class(c).unwrap());
    write_field(&mut vm.space, cdo, "SettingsProviderClass", Value::Class(provider), "test").unwrap();
    t
}

impl Toy {
    /// Sets what the refresh's step does.
    fn on_step(&self, f: impl Fn(&mut NativeCall<'_>) -> Result<Value, VmError> + 'static) {
        *self.step.borrow_mut() = Some(Rc::new(f));
    }

    /// Runs the refresh (the wrapped script) on a new toy controller.
    fn refresh(&mut self) -> Result<Value, VmError> {
        let pc = self.make("WillowGame.WillowPlayerController", "ToyPC");
        self.call(REFRESH, Some(pc), vec![])
    }

    /// The world's one settings entry after the client is made: (GameSettings, Provider).
    fn entry(&mut self) -> (Option<ObjRef>, Option<ObjRef>) {
        self.client();
        let store = self.vm.service::<DataStores>().unwrap().borrow().globals()[0];
        let list = read_field(&self.vm.space, store, "GameSettingsCfgList", "test").unwrap();
        let e = list.elements().unwrap()[0].clone();
        let obj = |v: Value| match v {
            Value::Object(o) => o,
            other => panic!("{other:?}"),
        };
        (
            obj(struct_get(&self.vm.space, &e, "GameSettings", "test").unwrap()),
            obj(struct_get(&self.vm.space, &e, "Provider", "test").unwrap()),
        )
    }

    /// Sets the settings' `Properties` to `(PropertyId, Type, Value1)` entries.
    fn set_properties(&mut self, settings: ObjRef, props: &[(i32, u8, i32)]) {
        let pty = self.vm.space.classes.struct_by_path("Engine.Settings.SettingsProperty").unwrap();
        let dty = self.vm.space.classes.struct_by_path("Engine.Settings.SettingsData").unwrap();
        let mut out = Vec::new();
        for &(id, ty, v) in props {
            let mut data = self.vm.space.classes.struct_defaults(dty).unwrap();
            struct_set(&self.vm.space, &mut data, "Type", Value::Byte(ty), "test").unwrap();
            struct_set(&self.vm.space, &mut data, "Value1", Value::Int(v), "test").unwrap();
            let mut p = self.vm.space.classes.struct_defaults(pty).unwrap();
            struct_set(&self.vm.space, &mut p, "PropertyId", Value::Int(id), "test").unwrap();
            struct_set(&self.vm.space, &mut p, "Data", data, "test").unwrap();
            out.push(p);
        }
        write_field(&mut self.vm.space, settings, "Properties", Value::array(out), "test").unwrap();
    }

    /// The settings' `Properties` as `(PropertyId, Type, Value1)`.
    fn properties(&self, settings: ObjRef) -> Vec<(i32, u8, i32)> {
        let list = read_field(&self.vm.space, settings, "Properties", "test").unwrap();
        list.elements()
            .unwrap()
            .iter()
            .map(|p| {
                let data = struct_get(&self.vm.space, p, "Data", "test").unwrap();
                let id = struct_get(&self.vm.space, p, "PropertyId", "test").unwrap();
                let ty = struct_get(&self.vm.space, &data, "Type", "test").unwrap();
                let v = struct_get(&self.vm.space, &data, "Value1", "test").unwrap();
                match (id, ty, v) {
                    (Value::Int(i), Value::Byte(t), Value::Int(v)) => (i, t, v),
                    other => panic!("{other:?}"),
                }
            })
            .collect()
    }
}

/// At store creation and binding (NATIVE_SETTINGS_FILL.md section 4): the entry gets a new `WillowOnlineGameSettings`
/// and a `UIDataProvider_Settings` bound to it (`Settings`, `DataClass`), with `SettingsArrayProviders` empty. A
/// `SettingsProviderClass` that is not a settings provider falls back to `UIDataProvider_Settings`. An entry without a
/// settings class gets neither object.
#[test]
fn the_store_fills_its_entry_at_creation() {
    for provider_class in [Some("Engine.UIDataProvider_Settings"), Some("Engine.UIDataProvider"), None] {
        let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), provider_class);
        let (settings, provider) = t.entry();
        let (settings, provider) = (settings.expect("a settings object"), provider.expect("a provider"));
        let class_path = |t: &Toy, o: ObjRef| ow_attr::eval::class_path(&t.vm.space, o);
        assert_eq!(class_path(&t, settings), "WillowGame.WillowOnlineGameSettings", "{provider_class:?}");
        assert_eq!(class_path(&t, provider), "Engine.UIDataProvider_Settings", "{provider_class:?}");
        assert_eq!(read_field(&t.vm.space, provider, "Settings", "test").unwrap(), Value::Object(Some(settings)));
        let settings_class = t.vm.space.heap.get(settings).unwrap().class;
        assert_eq!(read_field(&t.vm.space, provider, "DataClass", "test").unwrap(), Value::Class(Some(settings_class)));
        let arrays = read_field(&t.vm.space, provider, "SettingsArrayProviders", "test").unwrap();
        assert_eq!(arrays.elements().map(<[Value]>::len), Some(0), "not built");
    }
    let mut t = fill_toy(None, None);
    assert_eq!(t.entry(), (None, None), "an entry without a class gets neither object");
}

/// The fill is planned before anything is made (review r1 of #464, item 2): on a cold world with no
/// `UIDataProvider_Settings` class, the getter refuses with the **full** heap count unchanged (class default objects
/// included, which `heap.live` leaves out) and no client or store kept.
#[test]
fn a_fill_that_cannot_be_planned_leaves_nothing() {
    let mut t = fill_cold(false);
    let before = t.vm.space.heap.len();
    assert!(t.call(GET_DATA_STORE_CLIENT, None, vec![]).is_err(), "no UIDataProvider_Settings class");
    assert_eq!(t.vm.space.heap.len(), before, "nothing made, not even the store's class default object");
    let s = t.vm.service::<DataStores>().unwrap();
    assert!(s.borrow().client().is_none() && s.borrow().globals().is_empty());
}

/// The plan's read of the class defaults publishes nothing: on a cold world the read leaves the full heap count as it
/// was, and the getter then builds the class default object and fills the store as usual.
#[test]
fn the_plans_read_of_the_class_defaults_publishes_nothing() {
    let mut t = fill_cold(true);
    let before = t.vm.space.heap.len();
    let tag = t.vm.defs.with_class_default(&mut t.vm.space, GLOBAL_DATA_STORES[0], |space, cdo| read_field(space, cdo, "Tag", "test"));
    assert!(matches!(tag.unwrap().unwrap(), Value::Name(_)));
    assert_eq!(t.vm.space.heap.len(), before, "the read freed what it built");
    let class = t.vm.find_class(GLOBAL_DATA_STORES[0]).unwrap();
    assert!(plan_fill(&mut t.vm, class).unwrap().is_some());
    assert_eq!(t.vm.space.heap.len(), before, "the whole plan made nothing");
    t.client();
    assert!(t.vm.space.heap.len() > before, "the getter made the store");
}

/// A provider subclass whose class defaults hold a non-empty `SettingsArrayProviders` gets it empty (review r1 of
/// #464, item 6).
#[test]
fn a_provider_subclass_starts_with_no_array_providers() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), Some("Engine.ToySettingsProviderSub"));
    let cdo = t.vm.defs.class_default(&mut t.vm.space, "Engine.ToySettingsProviderSub").unwrap();
    let held = read_field(&t.vm.space, cdo, "SettingsArrayProviders", "test").unwrap();
    assert_eq!(held.elements().map(<[Value]>::len), Some(1), "the fixture's default holds one");
    let (_, provider) = t.entry();
    let provider = provider.unwrap();
    assert_eq!(ow_attr::eval::class_path(&t.vm.space, provider), "Engine.ToySettingsProviderSub");
    let arrays = read_field(&t.vm.space, provider, "SettingsArrayProviders", "test").unwrap();
    assert_eq!(arrays.elements().map(<[Value]>::len), Some(0));
}

/// The writes are published in the note's order (review r1 of #464, item 3): when the provider cannot be made, the
/// entry already holds its settings object, and its `Provider` stays empty.
#[test]
fn a_provider_failure_keeps_the_settings_written_before_it() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), None);
    let class = t.vm.find_class(GLOBAL_DATA_STORES[0]).unwrap();
    let plan = plan_fill(&mut t.vm, class).unwrap().unwrap();
    let store = ow_vm::interp::instantiate_from_defaults(&mut t.vm, class).unwrap();
    let provider_class = plan.provider;
    let mut make = |vm: &mut Vm, c: ClassId| -> Result<ObjRef, VmError> {
        if c == provider_class {
            return Err(VmError::NotYet(ow_core::NotYet::not_read("an invented provider failure", "an invented record")));
        }
        ow_vm::interp::instantiate_from_defaults(vm, c)
    };
    assert!(apply_fill(&mut t.vm, store, &plan, &mut make).is_err());
    let list = read_field(&t.vm.space, store, "GameSettingsCfgList", "test").unwrap();
    let e = list.elements().unwrap()[0].clone();
    let settings = struct_get(&t.vm.space, &e, "GameSettings", "test").unwrap();
    assert!(matches!(settings, Value::Object(Some(_))), "the settings object is published: {settings:?}");
    assert_eq!(struct_get(&t.vm.space, &e, "Provider", "test").unwrap(), Value::Object(None));
}

/// `SetIntProperty` (section 3): the first entry with the id gets the value when its type is `SDT_Int32`; a float
/// entry, and an id the settings do not list, change nothing.
#[test]
fn set_int_property_writes_the_first_integer_match_only() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), None);
    let (settings, _) = t.entry();
    let settings = settings.unwrap();
    t.set_properties(settings, &[(7, 1, 0), (7, 1, 0), (8, 2, 5)]);
    t.call(SET_INT_PROPERTY, Some(settings), vec![Value::Int(7), Value::Int(42)]).unwrap();
    assert_eq!(t.properties(settings), [(7, 1, 42), (7, 1, 0), (8, 2, 5)], "the first match only");
    t.call(SET_INT_PROPERTY, Some(settings), vec![Value::Int(8), Value::Int(9)]).unwrap();
    assert_eq!(t.properties(settings)[2], (8, 2, 5), "a float entry is left alone");
    t.call(SET_INT_PROPERTY, Some(settings), vec![Value::Int(99), Value::Int(1)]).unwrap();
    assert_eq!(t.properties(settings).len(), 3, "no entry added");
}

/// `GetCurrPlaythrough` reads `CurrentPlaythrough`; `GetDLCManager` answers None (the hub's ruling); a call on the
/// settings provider is the named `NotYet`, for a native, for the provider's own script, and for a script it inherits
/// (review r1 of #464, item 4), refused at dispatch. On another receiver the inherited script runs as script.
#[test]
fn the_engine_reads_and_the_provider_refusal() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), None);
    let gri = t.make("WillowGame.WillowGameReplicationInfo", "ToyGRI");
    write_field(&mut t.vm.space, gri, "CurrentPlaythrough", Value::Int(2), "test").unwrap();
    assert_eq!(t.call(GET_CURR_PLAYTHROUGH, Some(gri), vec![]).unwrap(), Value::Int(2));
    assert_eq!(t.call(GET_DLC_MANAGER, None, vec![]).unwrap(), Value::Object(None));
    let (_, provider) = t.entry();
    let provider = provider.unwrap();
    // No refresh runs here, so nothing is marked.
    for path in [
        "Engine.UIDataProvider.AddPropertyNotificationChangeRequest",
        "Engine.UIDataProvider_Settings.OnSettingValueUpdated",
        "Engine.UIDynamicDataProvider.CleanupDataProvider",
    ] {
        match t.call(path, Some(provider), vec![]) {
            Err(VmError::NotYet(n)) => assert!(n.what == PROVIDER_NOT_YET && n.missing == Missing::NotRead, "{path}: {n}"),
            other => panic!("{path}: {other:?}"),
        }
    }
    let dynamic = t.make("Engine.UIDynamicDataProvider", "ToyDynamicProvider");
    let cleanup = "Engine.UIDynamicDataProvider.CleanupDataProvider";
    assert_eq!(t.call(cleanup, Some(dynamic), vec![]).unwrap(), Value::None, "run as script on another receiver");
    let other = t.make("Engine.UIDataProvider", "ToyOtherProvider");
    not_yet(
        t.call("Engine.UIDataProvider.AddPropertyNotificationChangeRequest", Some(other), vec![]),
        |m| *m == Missing::NotRead,
        "not a settings provider",
    );
}

fn expect_marked(r: Result<Value, VmError>, missing: Missing, what: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, missing, "{n}");
            assert!(n.what.starts_with(PARTIAL_WRITES) && n.what.contains(what), "{n}");
        }
        other => panic!("expected a marked NotYet naming {what:?}, got {other:?}"),
    }
}

fn expect_unmarked(r: Result<Value, VmError>, what: &str) {
    match r {
        Err(VmError::NotYet(n)) => assert!(!n.what.contains(PARTIAL_WRITES) && n.what.contains(what), "{n}"),
        other => panic!("expected an unmarked NotYet naming {what:?}, got {other:?}"),
    }
}

const MISSION: &str = "WillowGame.WillowPlayerController.GetActiveMissionNumber";
/// What `GetActiveMissionNumber` answers in Off since decision 2026-11-19-93: the toy world has no level, so the
/// fallback answers, and its current playthrough is the unread virtual (`NotYet { NotRead }`).
const MISSION_STOP: &str = super::super::mission_numbers::PLAYTHROUGH_QUERY;

/// Through the wrapped refresh (review r1 of #464, item 1): the client lookup's fill, then a second lookup, then
/// `GetActiveMissionNumber`, whose `NotYet` (the unread playthrough virtual) leaves the refresh marked, kind kept.
/// A second lookup does not clear the mark.
#[test]
fn a_not_yet_after_the_refreshs_fill_is_marked_past_a_second_lookup() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), None);
    t.on_step(|c| {
        data_store_client(c.vm)?;
        data_store_client(c.vm)?;
        let pc = c.this;
        let f = c.vm.find_function(MISSION)?;
        Ok(c.vm.call(f, pc, vec![])?.ret)
    });
    expect_marked(t.refresh(), Missing::NotRead, MISSION_STOP);
}

/// A `NotYet` from outside the listed bindings, after a setting was written in the refresh, is marked as it leaves the
/// refresh (kind kept); a non-`NotYet` error leaves unchanged.
#[test]
fn any_not_yet_leaving_the_refresh_after_a_write_is_marked() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), None);
    let (settings, _) = t.entry();
    let settings = settings.unwrap();
    t.set_properties(settings, &[(1, 1, 0)]);
    let write = move |c: &mut NativeCall<'_>| -> Result<(), VmError> {
        let f = c.vm.find_function(SET_INT_PROPERTY)?;
        c.vm.call(f, Some(settings), vec![Value::Int(1), Value::Int(31)])?;
        Ok(())
    };
    t.on_step(move |c| {
        write(c)?;
        Err(ow_core::NotYet::not_ported("an invented NotYet outside the bindings", "an invented record").into())
    });
    expect_marked(t.refresh(), Missing::NotPorted, "an invented NotYet outside the bindings");
    assert_eq!(t.properties(settings), [(1, 1, 31)], "the write stays: nothing rolled back");
    t.on_step(move |c| {
        write(c)?;
        Err(VmError::ServiceBorrowed("an invented service"))
    });
    assert!(matches!(t.refresh(), Err(VmError::ServiceBorrowed("an invented service"))), "other errors unchanged");
}

/// Outside a refresh nothing is marked: a lookup and a setting called standalone leave no mark, so a later unread
/// call, standalone or in a refresh that writes nothing first, is unmarked.
#[test]
fn writes_outside_a_refresh_leave_no_mark() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), None);
    let (settings, _) = t.entry();
    let settings = settings.unwrap();
    t.set_properties(settings, &[(1, 1, 0)]);
    t.call(SET_INT_PROPERTY, Some(settings), vec![Value::Int(1), Value::Int(31)]).unwrap();
    let pc = t.make("WillowGame.WillowPlayerController", "ToyPC");
    expect_unmarked(t.call(MISSION, Some(pc), vec![]), MISSION_STOP);
    t.on_step(|c| {
        data_store_client(c.vm)?;
        let pc = c.this;
        let f = c.vm.find_function(MISSION)?;
        Ok(c.vm.call(f, pc, vec![])?.ret)
    });
    expect_unmarked(t.refresh(), MISSION_STOP);
    assert!(t.vm.service::<DataStores>().unwrap().borrow().refresh.is_empty(), "no refresh left open");
}

/// A nested refresh that wrote (the first lookup's fill) marks the outer one: the outer refresh's later `NotYet` is
/// marked.
#[test]
fn a_nested_refreshs_write_marks_the_outer_refresh() {
    let mut t = fill_toy(Some("WillowGame.WillowOnlineGameSettings"), None);
    let depth = Rc::new(Cell::new(0));
    t.on_step(move |c| {
        if depth.get() == 0 {
            depth.set(1);
            let f = c.vm.find_function(REFRESH)?;
            let pc = c.this;
            c.vm.call(f, pc, vec![])?;
            return Err(ow_core::NotYet::not_read("an invented outer NotYet", "an invented record").into());
        }
        data_store_client(c.vm)?;
        Ok(Value::None)
    });
    expect_marked(t.refresh(), Missing::NotRead, "an invented outer NotYet");
}

/// The plan's read publishes nothing even when the class defaults reference an engine-native object (review r3 of
/// #464, item 2): the resolver's stand-in is freed with the read, and the store made later gets its own.
#[test]
fn the_plans_read_frees_the_resolvers_stand_ins() {
    let mut t = fill_cold(true);
    let before = t.vm.space.heap.len();
    let class = t.vm.find_class(GLOBAL_DATA_STORES[0]).unwrap();
    assert!(plan_fill(&mut t.vm, class).unwrap().is_some());
    assert_eq!(t.vm.space.heap.len(), before, "nothing left, the resolver's stand-in included");
    t.client();
    let store = t.vm.service::<DataStores>().unwrap().borrow().globals()[0];
    assert!(matches!(read_field(&t.vm.space, store, "ToyNative", "test").unwrap(), Value::Object(Some(_))));
}

/// The provider's fields are checked on the selected provider class (review r3 of #464, item 4): a subclass that
/// shadows `Settings` with an int is refused before anything is made.
#[test]
fn a_provider_that_shadows_its_settings_field_is_refused_before_anything_is_made() {
    let mut t = fill_cold(true);
    let cdo = t.vm.defs.class_default(&mut t.vm.space, GLOBAL_DATA_STORES[0]).unwrap();
    let cfg = t.vm.space.classes.struct_by_path("Engine.UIDataStore_OnlineGameSettings.GameSettingsCfg").unwrap();
    let mut entry = t.vm.space.classes.struct_defaults(cfg).unwrap();
    let settings = t.vm.find_class("WillowGame.WillowOnlineGameSettings").unwrap();
    struct_set(&t.vm.space, &mut entry, "GameSettingsClass", Value::Class(Some(settings)), "test").unwrap();
    write_field(&mut t.vm.space, cdo, "GameSettingsCfgList", Value::array(vec![entry]), "test").unwrap();
    let shadow = t.vm.find_class("Engine.ToyShadowProvider").unwrap();
    write_field(&mut t.vm.space, cdo, "SettingsProviderClass", Value::Class(Some(shadow)), "test").unwrap();
    let before = t.vm.space.heap.len();
    assert!(t.call(GET_DATA_STORE_CLIENT, None, vec![]).is_err());
    assert_eq!(t.vm.space.heap.len(), before, "nothing made");
}

/// A wrapped script's body is bound to its own call (review r3 of #464, item 5): run on another VM it is refused, and
/// run while a service is borrowed it is `ServiceBorrowed`, before any bytecode.
#[test]
fn a_wrapped_body_runs_only_on_its_own_vm_with_no_service_borrowed() {
    let mut t = toy(true);
    let pc = t.make("WillowGame.WillowPlayerController", "ToyPC");
    t.vm.wrap_script(
        SETTINGS_VIA_DATASTORE,
        Rc::new(|_vm: &mut Vm, _this: Option<ObjRef>, body: &mut ow_vm::api::ScriptBody<'_>| {
            let mut other = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
            body(&mut other)
        }),
    )
    .unwrap();
    match t.call(SETTINGS_VIA_DATASTORE, Some(pc), vec![]) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("its own call's VM"), "{message}"),
        other => panic!("{other:?}"),
    }
    let mut t = toy(true);
    let pc = t.make("WillowGame.WillowPlayerController", "ToyPC");
    t.vm.wrap_script(
        SETTINGS_VIA_DATASTORE,
        Rc::new(|vm: &mut Vm, _this: Option<ObjRef>, body: &mut ow_vm::api::ScriptBody<'_>| {
            let stores = vm.service::<DataStores>().unwrap();
            let _held = stores.borrow();
            body(vm)
        }),
    )
    .unwrap();
    assert!(matches!(t.call(SETTINGS_VIA_DATASTORE, Some(pc), vec![]), Err(VmError::ServiceBorrowed(_))));
}


