//! Tests of the engine's data store client and the online game settings route (decision 2026-11-19-65) on invented
//! packages (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::testkit::script::{CPF_OPT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC: u32 = 0x2000;

/// `Engine.upk`: `UIInteraction.GetDataStoreClient`, `DataStoreClient.FindDataStore`, `UIDataStore` (its `Tag`) and
/// `Settings.SetIntProperty`. `with_client` false leaves `DataStoreClient` out.
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
    t.p.build()
}

/// `WillowGame.upk`: the online game settings store, its class defaults' `Tag` `WillowOnlineGameSettings`, and
/// `WillowGameReplicationInfo.GetCurrPlaythrough`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "IntProperty",
        "ScriptStruct",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let base = t.p.add_import("Core", "Class", engine, "UIDataStore");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let store = t.class("WillowOnlineGameSettings_DataStore", base);
    // The store's settings list (the installed defaults hold one entry whose GameSettings is empty; this fixture's
    // defaults hold none, and a test writes the entry).
    let cfg = ow_package::testkit::reflect::toy_struct(&mut t, store, "GameSettingsCfg", None);
    t.prop("Object", cfg, "GameSettings", 0, object);
    ow_package::testkit::reflect::toy_array_of(&mut t, store, "GameSettingsCfgList", "Struct", cfg);
    let mut defaults = ow_package::testkit::ints(&[0]);
    let tag = t.p.fname("WillowOnlineGameSettings", 0);
    defaults.extend(t.p.tag("Tag", "NameProperty", &tag));
    defaults.extend_from_slice(&t.none);
    t.p.add_export(store, "Default__WillowOnlineGameSettings_DataStore", defaults);
    let gri = t.class("WillowGameReplicationInfo", object);
    let spec = FunctionSpec::new(NATIVE, "GetCurrPlaythrough");
    t.function(gri, "GetCurrPlaythrough", &[], Some(("Int", 0)), &[], &spec, |_| None);
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

fn toy(with_client: bool) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-world-ds-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(with_client), willow());
    let files: [(&str, &[u8]); 2] = [("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    register(&mut vm).unwrap();
    Toy { vm, dir }
}

impl Toy {
    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        Ok(self.vm.call(f, this, args)?.ret)
    }

    fn name(&mut self, s: &str) -> Value {
        Value::Name(self.vm.space.names.intern(s))
    }

    fn tag_of(&self, o: ObjRef) -> String {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "Tag").unwrap();
        match self.vm.space.read(o, PropAddr::scalar(p)).unwrap() {
            Value::Name(n) => self.vm.space.names.text(*n).to_string(),
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
    let Value::Object(Some(client)) = t.call(GET_DATA_STORE_CLIENT, None, vec![]).unwrap() else {
        panic!("no client");
    };
    assert_eq!(t.call(GET_DATA_STORE_CLIENT, None, vec![]).unwrap(), Value::Object(Some(client)), "the same client");
    let s = t.vm.service::<DataStores>().unwrap();
    let globals = s.borrow().globals().to_vec();
    assert_eq!(globals.len(), 1);
    assert_eq!(t.tag_of(globals[0]), "WillowOnlineGameSettings");
    assert_eq!(s.borrow().client(), Some(client));
}

/// `FindDataStore`: None for a None tag; the global store by its tag (any case); `NotYet` for a tag no registered
/// store has, for a call with a player, and on another client. Nothing is made by any of them.
#[test]
fn find_data_store_searches_the_global_stores_by_tag() {
    let mut t = toy(true);
    let Value::Object(Some(client)) = t.call(GET_DATA_STORE_CLIENT, None, vec![]).unwrap() else {
        panic!("no client");
    };
    let store = t.vm.service::<DataStores>().unwrap().borrow().globals()[0];
    let none = t.name("None");
    assert_eq!(t.call(FIND_DATA_STORE, Some(client), vec![none]).unwrap(), Value::Object(None));
    // The settings entry is not assumed empty (section 5): with no entry, or an entry with no GameSettings, the
    // lookup is NotYet naming open question 2.
    let tag = t.name("WillowOnlineGameSettings");
    not_yet(t.call(FIND_DATA_STORE, Some(client), vec![tag.clone()]), |m| *m == Missing::NotRead, "open question 2");
    let class = t.vm.space.heap.get(store).unwrap().class;
    let list = t.vm.space.prop(class, "GameSettingsCfgList").unwrap();
    let PropKind::Array(inner) = t.vm.space.classes.decl(list).unwrap().kind.clone() else { panic!() };
    let empty_entry = t.vm.space.classes.zero_value(&inner);
    t.vm.space.write(store, PropAddr::scalar(list), Value::Array(Rc::new(vec![empty_entry.clone()]))).unwrap();
    not_yet(t.call(FIND_DATA_STORE, Some(client), vec![tag]), |m| *m == Missing::NotRead, "open question 2");
    // An entry whose GameSettings is set: the store is answered, by its tag in any case.
    let settings_class = t.vm.find_class("Engine.Settings").unwrap();
    let settings = t.vm.space.alloc(settings_class, "ToySettings", None, Origin::Runtime).unwrap();
    let s = empty_entry.as_struct().unwrap();
    let layout = t.vm.space.classes.struct_layout(s.ty).unwrap();
    let (at, _) = layout.field(t.vm.space.names.find("GameSettings").unwrap()).unwrap();
    let mut filled = ow_object::value::StructValue::clone(s);
    filled.fields[at as usize] = Value::Object(Some(settings));
    t.vm.space
        .write(store, PropAddr::scalar(list), Value::Array(Rc::new(vec![Value::Struct(Rc::new(filled))])))
        .unwrap();
    let heap = t.vm.space.heap.len();
    for tag in ["WillowOnlineGameSettings", "willowonlinegamesettings"] {
        let v = t.name(tag);
        assert_eq!(t.call(FIND_DATA_STORE, Some(client), vec![v]).unwrap(), Value::Object(Some(store)), "{tag}");
    }
    let unknown = t.name("ToyUnknownStore");
    not_yet(t.call(FIND_DATA_STORE, Some(client), vec![unknown]), |m| *m == Missing::NotRead, "no global data store");
    let player_class = t.vm.find_class("Engine.Object").unwrap();
    let player = t.vm.space.alloc(player_class, "ToyPlayer", None, Origin::Runtime).unwrap();
    let tag = t.name("WillowOnlineGameSettings");
    not_yet(
        t.call(FIND_DATA_STORE, Some(client), vec![tag.clone(), Value::Object(Some(player))]),
        |m| *m == Missing::NotPorted,
        "with a player",
    );
    not_yet(t.call(FIND_DATA_STORE, Some(player), vec![tag]), |m| *m == Missing::NotPorted, "other than the world's");
    assert_eq!(t.vm.space.heap.len(), heap + 1, "only the test's own player object");
    let _ = settings;
}

/// The natives behind the route are `NotYet { NotRead }` naming the note.
#[test]
fn the_natives_behind_the_route_are_not_yet() {
    let mut t = toy(true);
    let class = t.vm.find_class("WillowGame.WillowGameReplicationInfo").unwrap();
    let gri = t.vm.space.alloc(class, "ToyGRI", None, Origin::Runtime).unwrap();
    not_yet(t.call(UNREAD[2], Some(gri), vec![]), |m| *m == Missing::NotRead, "GetCurrPlaythrough");
    let class = t.vm.find_class("Engine.Settings").unwrap();
    let settings = t.vm.space.alloc(class, "ToySettings", None, Origin::Runtime).unwrap();
    not_yet(t.call(UNREAD[6], Some(settings), vec![Value::Int(1), Value::Int(2)]), |m| *m == Missing::NotRead, "SetIntProperty");
}

/// A client that cannot be made (no `DataStoreClient` class) is a typed error that leaves nothing behind: the store
/// made before it is freed (only its class default stays, the definition store's cache), and the world keeps no
/// client.
#[test]
fn a_client_that_cannot_be_made_leaves_nothing() {
    let mut t = toy(false);
    let object = t.vm.find_class("Engine.Object").unwrap();
    let live = |t: &Toy| t.vm.space.heap.live(&t.vm.space.classes, object).count();
    let before = live(&t);
    assert!(t.call(GET_DATA_STORE_CLIENT, None, vec![]).is_err());
    assert_eq!(live(&t), before, "no instance left on the heap");
    let s = t.vm.service::<DataStores>().unwrap();
    assert!(s.borrow().client().is_none() && s.borrow().globals().is_empty());
}
