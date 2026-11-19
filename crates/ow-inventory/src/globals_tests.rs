//! Tests of the globals natives (NATIVE_WILLOW_GLOBALS.md; lane IMPL-WILLOW-GLOBALS, decision 2026-11-19-94) on three
//! invented packages built with `ow_package::testkit` (AGENTS.md rule 9). `Engine` has `Object`; `GearboxFramework`
//! has `GearboxGlobals` with its definition and population master fields and its two natives; `WillowGame` has
//! `WillowGlobals` with `PickupList` and its three natives, `GlobalsDefinition` with `RarityLevelColors`, `WillowPickup`,
//! and `ToyBadGlobals` / `ToyBadDefinition` with the wrong declared schemas. Every native is reached through the VM.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::StubPolicy;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
const STATIC_NATIVE: u32 = 0x2000 | NATIVE;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 9] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "IntProperty",
    "ByteProperty",
    "StructProperty",
    "ArrayProperty",
    "BoolProperty",
];

fn native(t: &mut ScriptToy, owner: i32, name: &str, flags: u32, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(flags, name), |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    t.class("Object", 0);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let definition = t.class("GearboxGlobalsDefinition", object);
    let master = t.class("PopulationMaster", object);
    native(&mut t, master, "RemoveSpawnedActor", NATIVE, &[Param { type_ref: object, ..Param::new("Object", "Actor") }], None);
    let globals = t.class("GearboxGlobals", object);
    t.prop("Object", globals, "TheGlobalsDefinition", 0, definition);
    t.prop("Object", globals, "ThePopulationMaster", 0, master);
    native(&mut t, globals, "GetGearboxGlobals", STATIC_NATIVE, &[], Some(("Object", globals)));
    native(&mut t, globals, "GetPopulationMaster", NATIVE, &[], Some(("Object", object)));
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let gbx = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_globals = t.p.add_import("Core", "Class", gbx, "GearboxGlobals");
    let gearbox_definition = t.p.add_import("Core", "Class", gbx, "GearboxGlobalsDefinition");
    let pickup = t.class("WillowPickup", object);
    let globals = t.class("WillowGlobals", gearbox_globals);
    toy_array_of(&mut t, globals, "PickupList", "Object", pickup);
    native(&mut t, globals, "GetWillowGlobals", STATIC_NATIVE, &[], Some(("Object", globals)));
    native(&mut t, globals, "AddPickup", NATIVE, &[Param { type_ref: pickup, ..Param::new("Object", "Pickup") }], None);
    native(&mut t, globals, "GetGlobalsDefinition", NATIVE, &[], Some(("Object", object)));
    let gd = t.class("GlobalsDefinition", gearbox_definition);
    let band = toy_struct(&mut t, gd, "RarityLevelColor", None);
    t.prop("Int", band, "MinLevel", 0, 0);
    t.prop("Int", band, "MaxLevel", 0, 0);
    t.prop("Byte", band, "DropLifeSpanType", 0, 0);
    toy_array_of(&mut t, gd, "RarityLevelColors", "Struct", band);
    native(&mut t, gd, "GetDropLifeSpanType", NATIVE, &[Param::new("Int", "RarityLevel")], Some(("Byte", 0)));
    let bad = t.class("ToyBadGlobals", object);
    toy_array_of(&mut t, bad, "PickupList", "Int", 0);
    // The right shapes with the wrong reference types: elements of Object, a definition field of Object.
    let wrong = t.class("ToyWrongGlobals", object);
    toy_array_of(&mut t, wrong, "PickupList", "Object", object);
    t.prop("Object", wrong, "TheGlobalsDefinition", 0, object);
    t.prop("Object", wrong, "ThePopulationMaster", 0, object);
    let bad_definition = t.class("ToyBadDefinition", object);
    toy_array_of(&mut t, bad_definition, "RarityLevelColors", "Int", 0);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    globals: ObjRef,
    gd: ObjRef,
    query: std::rc::Rc<std::cell::RefCell<ow_world::query::WorldQuery>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-globals-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow_game());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["WillowGame.WillowGlobals", "WillowGame.GlobalsDefinition", "GearboxFramework.GearboxGlobals"] {
        vm.find_class(c).unwrap();
    }
    let query = ow_world::query::install(&mut vm).unwrap();
    register(&mut vm).unwrap();
    let globals = make(&mut vm, "WillowGame.WillowGlobals");
    let gd = make(&mut vm, "WillowGame.GlobalsDefinition");
    Fx { vm, dir, globals, gd, query }
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn call(&mut self, path: &str, me: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, me, args)?.ret)
    }

    fn list(&self) -> Vec<Value> {
        pickup_list(&self.vm, self.globals, "test").unwrap()
    }

    /// `RarityLevelColors` set to `bands`, each field by name.
    fn set_bands(&mut self, bands: &[(i32, i32, u8)]) {
        let class = self.vm.space.heap.get(self.gd).unwrap().class;
        let p = self.vm.space.prop(class, "RarityLevelColors").unwrap();
        let ty = match &self.vm.space.classes.decl(p).unwrap().kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(ty) => ty,
                ref k => panic!("{k:?}"),
            },
            k => panic!("{k:?}"),
        };
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let mut entries = Vec::new();
        for &(min, max, kind) in bands {
            let mut fields = vec![Value::None; 3];
            for (name, v) in [("MinLevel", Value::Int(min)), ("MaxLevel", Value::Int(max)), ("DropLifeSpanType", Value::Byte(kind))] {
                let (index, _) = layout.field(self.vm.space.names.find(name).unwrap()).unwrap();
                fields[usize::try_from(index).unwrap()] = v;
            }
            entries.push(Value::structure(ty, fields));
        }
        self.vm.space.write(self.gd, PropAddr::scalar(p), Value::array(entries)).unwrap();
    }
}

fn missing(r: Result<Value, VmError>) -> Missing {
    match r {
        Err(VmError::NotYet(n)) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// The two getters answer the world's one globals object; a world without one is a `NotYet { LayerAbsent }`, not None.
#[test]
fn the_getters_answer_the_worlds_one_globals_object() {
    let mut x = fx(ParityMode::Off);
    assert!(matches!(missing(x.call(GET_WILLOW_GLOBALS, None, vec![])), Missing::LayerAbsent(_)));
    x.query.borrow_mut().set_globals(x.globals);
    for path in [GET_WILLOW_GLOBALS, GET_GEARBOX_GLOBALS] {
        assert_eq!(x.call(path, None, vec![]).unwrap(), Value::Object(Some(x.globals)), "{path}");
        assert_eq!(x.call(path, None, vec![]).unwrap(), Value::Object(Some(x.globals)), "the same on every call");
    }
}

/// `AddPickup` appends as given, a pickup already listed again and None too, with nothing else read.
#[test]
fn add_pickup_appends_with_no_duplicate_check() {
    let mut x = fx(ParityMode::Off);
    let (a, b) = (make(&mut x.vm, "WillowGame.WillowPickup"), make(&mut x.vm, "WillowGame.WillowPickup"));
    let g = Some(x.globals);
    for p in [Some(a), Some(b), Some(a), None] {
        x.call(ADD_PICKUP, g, vec![Value::Object(p)]).unwrap();
    }
    let want: Vec<Value> = [Some(a), Some(b), Some(a), None].into_iter().map(Value::Object).collect();
    assert_eq!(x.list(), want);
}

/// A `PickupList` or `RarityLevelColors` declared with the wrong shape is a data error, before anything is written.
#[test]
fn the_declared_schemas_are_checked() {
    let mut x = fx(ParityMode::Off);
    let bad = make(&mut x.vm, "WillowGame.ToyBadGlobals");
    assert!(matches!(pickup_list(&x.vm, bad, "test"), Err(VmError::Script { message, .. }) if message.contains("PickupList")));
    let bad = make(&mut x.vm, "WillowGame.ToyBadDefinition");
    assert!(matches!(rarity_bands(&x.vm, bad, "test"), Err(VmError::Script { message, .. }) if message.contains("RarityLevelColors")));
}

/// `GetGlobalsDefinition` answers `TheGlobalsDefinition` when it holds a `GlobalsDefinition`; otherwise the named
/// `NotYet { NotRead }` (the first load is not run).
#[test]
fn the_globals_definition_is_a_read_or_not_yet() {
    let mut x = fx(ParityMode::Off);
    let g = Some(x.globals);
    assert_eq!(missing(x.call(GET_GLOBALS_DEFINITION, g, vec![])), Missing::NotRead);
    let other = make(&mut x.vm, "GearboxFramework.GearboxGlobalsDefinition");
    x.set(x.globals, "TheGlobalsDefinition", Value::Object(Some(other)));
    assert_eq!(missing(x.call(GET_GLOBALS_DEFINITION, g, vec![])), Missing::NotRead, "not a GlobalsDefinition");
    x.set(x.globals, "TheGlobalsDefinition", Value::Object(Some(x.gd)));
    assert_eq!(x.call(GET_GLOBALS_DEFINITION, g, vec![]).unwrap(), Value::Object(Some(x.gd)));
}

/// `GetDropLifeSpanType`: the first band holding the level, both ends included; 3 outside every band and with none.
#[test]
fn the_drop_life_span_type_is_the_first_band_holding_the_level() {
    let mut x = fx(ParityMode::Off);
    let d = Some(x.gd);
    assert_eq!(x.call(GET_DROP_LIFE_SPAN_TYPE, d, vec![Value::Int(1)]).unwrap(), Value::Byte(3), "no bands");
    x.set_bands(&[(0, 2, 1), (3, 5, 2), (4, 9, 0)]);
    for (level, kind) in [(0, 1), (2, 1), (3, 2), (5, 2), (4, 2), (6, 0), (9, 0), (10, 3), (-1, 3)] {
        assert_eq!(x.call(GET_DROP_LIFE_SPAN_TYPE, d, vec![Value::Int(level)]).unwrap(), Value::Byte(kind), "level {level}");
    }
}

/// `GetPopulationMaster` answers the field as it is, None included.
#[test]
fn the_population_master_is_the_field_as_it_is() {
    let mut x = fx(ParityMode::Off);
    let g = Some(x.globals);
    assert_eq!(x.call(GET_POPULATION_MASTER, g, vec![]).unwrap(), Value::Object(None));
    let master = make(&mut x.vm, "GearboxFramework.PopulationMaster");
    x.set(x.globals, "ThePopulationMaster", Value::Object(Some(master)));
    let f = x.vm.find_function(REMOVE_SPAWNED_ACTOR[0]).unwrap();
    match x.vm.call(f, Some(master), vec![Value::Object(None)]).unwrap_err() {
        VmError::NotYet(n) => assert!(n.missing == Missing::NotRead && n.what.contains("RemoveSpawnedActor"), "{n}"),
        e => panic!("{e}"),
    }
    assert_eq!(x.call(GET_POPULATION_MASTER, g, vec![]).unwrap(), Value::Object(Some(master)));
}

/// Reference binds none of them; a key already bound is kept (the mission bridge's own getters).
#[test]
fn reference_binds_nothing_and_a_bound_key_is_kept() {
    let mut x = fx(ParityMode::Reference);
    for path in PATHS {
        let f = x.vm.find_function(path).unwrap();
        let key = x.vm.function(f).unwrap().native_key.clone().unwrap();
        assert!(!x.vm.has_native(&key), "{key}");
    }
    let mut y = fx(ParityMode::Off);
    assert!(register(&mut y.vm).is_ok(), "registering again keeps the bound keys");
    drop(x);
}

/// The preflight's schemas (section 6.2 step 4, and the note's last round): the declared reference types, each mismatch
/// a data error raised before any value is read. The toy globals pass; the wrong reference types fail.
#[test]
fn the_declared_reference_types_are_checked() {
    let mut x = fx(ParityMode::Off);
    let g = x.globals;
    globals_schemas(&mut x.vm, g, "test").unwrap();
    let wrong = make(&mut x.vm, "WillowGame.ToyWrongGlobals");
    match globals_schemas(&mut x.vm, wrong, "test").unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("TheGlobalsDefinition"), "{message}"),
        e => panic!("{e}"),
    }
    declared_reference(&mut x.vm, wrong, "ThePopulationMaster", None, "test").unwrap();
    match pickup_list_declared(&mut x.vm, wrong, "test").unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("WillowPickup"), "{message}"),
        e => panic!("{e}"),
    }
}

/// With no globals object the getters answer the note's own *layer absent*, "the world's globals object".
#[test]
fn a_world_without_globals_names_them() {
    let mut x = fx(ParityMode::Off);
    match x.call(GET_GEARBOX_GLOBALS, None, vec![]).unwrap_err() {
        VmError::NotYet(n) => assert!(n.what == "the world's globals object" && n.record == RECORD, "{n}"),
        e => panic!("{e}"),
    }
}
