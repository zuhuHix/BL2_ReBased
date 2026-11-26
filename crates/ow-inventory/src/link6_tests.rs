//! NATIVE_ACTOR_SETBASE.md tests 8 to 10 and 12, and NATIVE_LINK6_AFTER_TAKE.md tests 1 to 4 and 9, on invented
//! packages (AGENTS.md rule 9). The class and field names are the packages'; every value is invented. AI-assisted
//! (Claude Code).

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
use ow_package::testkit::reflect::{property_payload, toy_array_of, toy_struct};
use ow_package::testkit::{Export, ints};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC_NATIVE: u32 = 0x400 | 0x2000 | 0x20000;
const IMPORTS: [&str; 10] = [
    "ByteProperty",
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "BoolProperty",
    "IntProperty",
    "StructProperty",
    "FloatProperty",
    "ArrayProperty",
];

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<(&str, i32)>, flags: u32) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(flags, name), |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    t.prop("Object", actor, "Base", 0, actor);
    toy_array_of(&mut t, actor, "Attached", "Object", actor);
    native(&mut t, actor, "PlayAkEvent", &[Param { type_ref: object, ..Param::new("Object", "InSoundCue") }], Some(("Int", 0)), NATIVE);
    let manager = t.class("InventoryManager", actor);
    t.prop("Bool", manager, "bInfiniteAmmo", 0, 0);
    let expression = t.class("AttributeExpression", object);
    let expr = toy_struct(&mut t, expression, "AttributeExpressionData", None);
    t.prop("Object", expr, "AttributeOperand1", 0, object);
    t.prop("Byte", expr, "ComparisonOperator", 0, 0);
    t.prop("Byte", expr, "Operand2Usage", 0, 0);
    t.prop("Object", expr, "AttributeOperand2", 0, object);
    t.prop("Float", expr, "ConstantOperand2", 0, 0);
    let sound = toy_struct(&mut t, expression, "ConditionalSoundData", None);
    t.prop("Object", sound, "Event", 0, object);
    t.prop("Object", sound, "Cue", 0, object);
    toy_array_of(&mut t, sound, "Expressions", "Struct", expr);
    // A field of the entry's type, so loading the class defines the two structs (invented).
    t.prop("Struct", expression, "ToySound", 0, sound);
    let source = Param { type_ref: object, ..Param::new("Object", "ContextSource") };
    let list = Param { type_ref: sound, ..Param::new("Array", "SoundList") };
    native(&mut t, expression, "SelectAkEvent", &[source, list], Some(("Object", object)), STATIC_NATIVE);
    t.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let weapon = t.class("WillowWeapon", actor);
    t.prop("Object", weapon, "InvManager", 0, actor);
    let reference = toy_struct(&mut t, weapon, "ToyPoolReference", None);
    t.prop("Object", reference, "PoolManager", 0, object);
    t.prop("Byte", reference, "PoolIndexInManager", 0, 0);
    t.prop("Byte", reference, "PoolGUID", 0, 0);
    t.prop("Object", reference, "Data", 0, object);
    t.prop("Struct", weapon, "AmmoPool", 0, reference);
    t.prop("Bool", weapon, "bUseOverheatBehavior", 0, 0);
    for f in ["StoredAmmo", "OverheatAmmo", "ClipSize"] {
        t.prop("Int", weapon, f, 0, 0);
    }
    native(&mut t, weapon, "GetAmmoCount", &[], Some(("Int", 0)), NATIVE);
    let pickup = t.class("WillowPickup", actor);
    let globals = t.class("WillowGlobals", object);
    toy_array_of(&mut t, globals, "PickupList", "Object", pickup);
    native(&mut t, globals, "RemovePickup", &[Param { type_ref: object, ..Param::new("Object", "inPickup") }], None, NATIVE);
    // Review of #800: a live ammo pool (a manager of 16 pool slots, a pool with a GUID and a value).
    let pool = t.class("ToyAmmoPool", object);
    t.prop("Byte", pool, "PoolGUID", 0, 0);
    t.prop("Float", pool, "CurrentValue", 0, 0);
    let manager = t.class("ToyPoolManager", object);
    let payload = property_payload(&t.none, 16, 0, &ints(&[pool]));
    let mut slots = Export::new(t.import("ObjectProperty"), "ResourcePools", payload);
    slots.outer = manager;
    t.p.add_export_with(slots);
    t.build()
}

/// The invented `GearboxFramework.upk` (review of #800, item 2): the `IBehaviorConsumer` interface, an actor class
/// that implements it (with its `VfTable_IBehaviorConsumer` table property), and one that does not.
fn gearbox() -> Vec<u8> {
    use ow_package::testkit::reflect::{CLASS_INTERFACE, ClassBodySpec, Declarations, PropSpec, class_body};
    let mut d = Declarations::with_imports(&["Class", "ScriptStruct", "StructProperty", "ArrayProperty", "ObjectProperty"]);
    // A decodable root (the interface rule reads the whole super chain's bodies; the toy Engine's classes have none).
    let root_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let root = d.p.add_export(0, "ToyConsumerRoot", root_body);
    d.default_object(root, "ToyConsumerRoot");
    let consumer = d.real_class("IBehaviorConsumer", root, root, 2 | CLASS_INTERFACE, &[], None);
    // The implementing class: its interface entry names its table property, a struct of an invented table type.
    let table_type = d.struct_("ToyVfTable", None);
    let at = d.p.next_export();
    let (table, default) = (at + 1, at + 2);
    let spec = ClassBodySpec { interfaces: vec![(consumer, table)], first_child: table, ..ClassBodySpec::new(root, root, default) };
    let body = class_body(&mut d.p, &spec);
    let mut export = Export::new(0, "ToyConsumerBase", body);
    export.super_ = root;
    assert_eq!(d.p.add_export_with(export), at);
    let spec = PropSpec { type_ref: table_type, ..PropSpec::new("Struct", at, "VfTable_IBehaviorConsumer") };
    assert_eq!(d.prop_with(&spec), table);
    assert_eq!(d.default_object(at, "ToyConsumerBase"), default);
    d.array_of(at, "Attached", "Object", root);
    let plain = d.real_class("ToyPlainBase", root, root, 2, &[], None);
    d.array_of(plain, "Attached", "Object", root);
    d.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-link6-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    if mode == ParityMode::Off {
        register(&mut vm).unwrap();
    }
    Fx { vm, dir }
}

impl Fx {
    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }
    fn get(&self, o: ObjRef, name: &str) -> Value {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        (
            self.vm.space.heap.committed_writes(),
            self.vm.rng.draws(),
            self.vm.defs.load_transitions(),
            self.vm.class_code_loads(),
            self.vm.space.names.len(),
        )
    }
    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        self.vm.call(f, this, args).map(|r| r.ret)
    }
    /// A `ConditionalSoundData` entry: `event`, and `failing` expressions that fail (no operand). Built from the
    /// types' defaults, its fields set by name.
    fn sound(&mut self, event: Option<ObjRef>, failing: usize) -> Value {
        use ow_object::layout::PropKind;
        self.vm.find_class("Engine.AttributeExpression").unwrap();
        let ty = self.vm.space.classes.struct_by_path("Engine.AttributeExpression.ConditionalSoundData").unwrap();
        let set = |vm: &Vm, v: &mut Value, name: &str, x: Value| {
            let id = vm.space.names.find(name).unwrap();
            let s = v.struct_mut().unwrap();
            let (i, _) = vm.space.classes.struct_layout(s.ty).unwrap().field(id).unwrap();
            s.fields[i as usize] = x;
        };
        let id = self.vm.space.names.find("Expressions").unwrap();
        let expr_ty = match &self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().1.kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(s) => s,
                ref other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        let expr = self.vm.space.classes.default_value(&PropKind::Struct(expr_ty)).unwrap();
        let mut entry = self.vm.space.classes.default_value(&PropKind::Struct(ty)).unwrap();
        set(&self.vm, &mut entry, "Event", Value::Object(event));
        set(&self.vm, &mut entry, "Expressions", Value::array((0..failing).map(|_| expr.clone()).collect()));
        entry
    }
}

// ACTOR_SETBASE test 8: GetAmmoCount (the live-pool case needs a pool slot: not built here).
#[test]
fn get_ammo_count_by_the_rule() {
    let mut x = fx(ParityMode::Off);
    let weapon = x.make("WillowGame.WillowWeapon");
    let manager = x.make("Engine.InventoryManager");
    for (f, v) in [("StoredAmmo", 30), ("OverheatAmmo", 7), ("ClipSize", 12)] {
        x.set(weapon, f, Value::Int(v));
    }
    let count = |x: &mut Fx| {
        let before = x.marks();
        let v = x.call(GET_AMMO_COUNT, Some(weapon), vec![]).unwrap();
        assert_eq!(x.marks(), before, "reads only");
        v
    };
    assert_eq!(count(&mut x), Value::Int(30), "no pool: StoredAmmo");
    x.set(weapon, "bUseOverheatBehavior", Value::Bool(true));
    assert_eq!(count(&mut x), Value::Int(7), "overheat");
    x.set(weapon, "InvManager", Value::Object(Some(manager)));
    x.set(manager, "bInfiniteAmmo", Value::Bool(true));
    assert_eq!(count(&mut x), Value::Int(12), "infinite ammo: ClipSize");
}

/// Review of #800, item 3: a live ammo pool answers its `CurrentValue` (truncated), and the reference's cache (`Data`)
/// is not written back; a pool whose GUID does not match falls back to `StoredAmmo`.
#[test]
fn get_ammo_count_reads_a_live_pool_and_writes_no_cache() {
    let mut x = fx(ParityMode::Off);
    let weapon = x.make("WillowGame.WillowWeapon");
    x.set(weapon, "StoredAmmo", Value::Int(30));
    let manager = x.make("WillowGame.ToyPoolManager");
    let pool = x.make("WillowGame.ToyAmmoPool");
    x.set(pool, "PoolGUID", Value::Byte(9));
    x.set(pool, "CurrentValue", Value::Float(17.75));
    {
        let class = x.vm.space.heap.get(manager).unwrap().class;
        let p = x.vm.space.prop(class, "ResourcePools").unwrap();
        *x.vm.space.slot_mut(manager, PropAddr::element(p, 4)).unwrap() = Value::Object(Some(pool));
    }
    let point = |x: &mut Fx, guid: u8| {
        let mut r = x.get(weapon, "AmmoPool");
        {
            let s = r.struct_mut().unwrap();
            let layout = x.vm.space.classes.struct_layout(s.ty).unwrap().clone();
            for (f, v) in [("PoolManager", Value::Object(Some(manager))), ("PoolIndexInManager", Value::Byte(4)), ("PoolGUID", Value::Byte(guid))] {
                let (i, _) = layout.field(x.vm.space.names.find(f).unwrap()).unwrap();
                s.fields[i as usize] = v;
            }
        }
        x.set(weapon, "AmmoPool", r);
    };
    point(&mut x, 9);
    let reference = x.get(weapon, "AmmoPool");
    let before = x.marks();
    assert_eq!(x.call(GET_AMMO_COUNT, Some(weapon), vec![]).unwrap(), Value::Int(17), "the pool's value");
    assert_eq!(x.marks(), before, "reads only");
    assert_eq!(x.get(weapon, "AmmoPool"), reference, "the cache is not written back");
    point(&mut x, 8);
    assert_eq!(x.call(GET_AMMO_COUNT, Some(weapon), vec![]).unwrap(), Value::Int(30), "a stale GUID: StoredAmmo");
}

// ACTOR_SETBASE test 9: SelectAkEvent (an entry that holds needs an attribute: not built here).
#[test]
fn select_ak_event_takes_the_last_fallback() {
    let mut x = fx(ParityMode::Off);
    let (a, b) = (x.make("Engine.Object"), x.make("Engine.Object"));
    // The rule itself (the toy package cannot declare an array parameter, so the VM call has none).
    let select = |x: &mut Fx, list: Vec<Value>| Value::Object(select_ak_event(&mut x.vm, None, &list).unwrap());
    assert_eq!(select(&mut x, vec![]), Value::Object(None), "an empty list");
    let one = x.sound(Some(a), 0);
    assert_eq!(select(&mut x, vec![one]), Value::Object(Some(a)), "one empty-expression entry");
    let (fails, empty) = (x.sound(Some(a), 1), x.sound(Some(b), 0));
    assert_eq!(select(&mut x, vec![fails, empty]), Value::Object(Some(b)), "a failing entry changes nothing");
    let (first, last) = (x.sound(Some(a), 0), x.sound(Some(b), 0));
    assert_eq!(select(&mut x, vec![first, last]), Value::Object(Some(b)), "the last fallback");
}

// ACTOR_SETBASE test 10: PlayAkEvent and the audio stand-in.
#[test]
fn play_ak_event_and_the_audio_stand_in() {
    let mut x = fx(ParityMode::Off);
    let actor = x.make("Engine.Actor");
    let event = x.make("Engine.Object");
    let before = x.marks();
    assert_eq!(x.call(PLAY_AK_EVENT, Some(actor), vec![Value::Object(None)]).unwrap(), Value::Int(0), "a None event");
    match x.call(PLAY_AK_EVENT, Some(actor), vec![Value::Object(Some(event))]) {
        Err(VmError::NotYet(n)) => assert!(matches!(n.missing, Missing::LayerAbsent(_)) && n.what.as_ref() == AUDIO, "{n}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before, "nothing written");
    enable_audio_stand_in(&mut x.vm).unwrap();
    assert!(x.vm.log.iter().any(|l| l == AUDIO_INPUT), "the set-up line");
    let logged = x.vm.log.len();
    let before = x.marks();
    assert_eq!(x.call(PLAY_AK_EVENT, Some(actor), vec![Value::Object(Some(event))]).unwrap(), Value::Int(0), "the default handle");
    assert_eq!(x.marks(), before, "nothing written");
    assert_eq!(&x.vm.log[logged..], [AUDIO_LINE.to_string()], "the stand-in line, once");
    let mut r = fx(ParityMode::Reference);
    assert!(matches!(enable_audio_stand_in(&mut r.vm), Err(VmError::Registration { .. })), "Off only");
}

// LINK6_AFTER_TAKE tests 1, 3 and 4: RemovePickup (a base that provides the interface needs interface tables: not built).
#[test]
fn remove_pickup_removes_every_copy_and_refuses_before_any_write() {
    let mut x = fx(ParityMode::Off);
    let globals = x.make("WillowGame.WillowGlobals");
    let (a, p, b) = (x.make("WillowGame.WillowPickup"), x.make("WillowGame.WillowPickup"), x.make("WillowGame.WillowPickup"));
    let list = |xs: &[ObjRef]| Value::array(xs.iter().map(|o| Value::Object(Some(*o))).collect());
    x.set(globals, "PickupList", list(&[a, p, b, p]));
    let before = x.vm.space.heap.committed_writes();
    x.call(REMOVE_PICKUP, Some(globals), vec![Value::Object(Some(p))]).unwrap();
    assert_eq!(x.get(globals, "PickupList"), list(&[a, b]));
    assert_eq!(x.vm.space.heap.committed_writes() - before, 1, "one write");
    // A None pickup: a script error.
    assert!(matches!(x.call(REMOVE_PICKUP, Some(globals), vec![Value::Object(None)]), Err(VmError::Script { .. })));
    // A base, with no IBehaviorConsumer in the class table: the unresolved-interface NotRead, before any write.
    let base = x.make("Engine.Actor");
    x.set(p, "Base", Value::Object(Some(base)));
    x.set(globals, "PickupList", list(&[p]));
    let before = x.marks();
    match x.call(REMOVE_PICKUP, Some(globals), vec![Value::Object(Some(p))]) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, CONSUMER_UNRESOLVED)),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before, "nothing written, loaded or interned");
    assert_eq!(x.get(globals, "PickupList"), list(&[p]));
    // Review of #800, item 2: a base whose class provides IBehaviorConsumer is the events' NotRead before any write,
    // nothing interned; a base whose class does not is removed as usual.
    x.vm.find_class("GearboxFramework.IBehaviorConsumer").unwrap();
    let consumer = x.make("GearboxFramework.ToyConsumerBase");
    x.set(p, "Base", Value::Object(Some(consumer)));
    // The class's interface entries resolved, as a cast would have resolved them (set-up, before the counters).
    let iface = x.vm.space.classes.by_path("GearboxFramework.IBehaviorConsumer").unwrap();
    let (consumer_class, plain_class) =
        (x.vm.space.heap.get(consumer).unwrap().class, x.vm.find_class("GearboxFramework.ToyPlainBase").unwrap());
    assert!(x.vm.implements(consumer_class, iface), "the toy consumer implements the interface");
    assert!(!x.vm.implements(plain_class, iface), "the plain base does not");
    let before = x.marks();
    match x.call(REMOVE_PICKUP, Some(globals), vec![Value::Object(Some(p))]) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, CONSUMER_EVENTS)),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before, "the consumer: nothing written, loaded or interned");
    assert_eq!(x.get(globals, "PickupList"), list(&[p]));
    let plain = x.make("GearboxFramework.ToyPlainBase");
    x.set(p, "Base", Value::Object(Some(plain)));
    x.call(REMOVE_PICKUP, Some(globals), vec![Value::Object(Some(p))]).unwrap();
    assert_eq!(x.get(globals, "PickupList"), list(&[]), "a base that is no consumer: removed");
    // An unloaded pickup: NotRead.
    let c = x.vm.find_class("WillowGame.WillowPickup").unwrap();
    let lazy = x.vm.space.alloc_stand_in(c, "Lazy", Origin::Default { class: c }).unwrap();
    match x.call(REMOVE_PICKUP, Some(globals), vec![Value::Object(Some(lazy))]) {
        Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NotRead),
        other => panic!("{other:?}"),
    }
}

// ACTOR_SETBASE test 12 and LINK6_AFTER_TAKE test 9: Reference binds none of the keys.
#[test]
fn reference_binds_none_of_them() {
    let mut x = fx(ParityMode::Reference);
    crate::pickup::register(&mut x.vm).unwrap();
    for path in [GET_AMMO_COUNT, SELECT_AK_EVENT, PLAY_AK_EVENT, REMOVE_PICKUP] {
        let f = x.vm.find_function(path).unwrap();
        assert!(!x.vm.is_bound(f), "{path}");
    }
}
