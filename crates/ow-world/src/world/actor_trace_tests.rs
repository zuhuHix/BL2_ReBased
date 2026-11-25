//! Tests of `Actor.Trace` in Off (lane IMPL-ACTOR-TRACE, NATIVE_ACTOR_TRACE.md section 3.6) on invented packages with
//! the game's class and field names (AGENTS.md rule 9) and an invented collision service. Every value is invented.
//! AI-assisted (Claude Code). The binding sites (the note's test 9) are tested in `ow-skills` and `ow-combat`.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::detmap::DetMap;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{Arg, NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::actors::Actors;
use crate::placement::{Bounds, Mode, SpawnCollision, Sweep, Test, install_collision};

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE_FLAGS: u32 = 0x400 | 0x20000;

const KINDS: [&str; 9] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "BoolProperty",
    "FloatProperty",
    "IntProperty",
    "ArrayProperty",
    "StructProperty",
];

/// `Core.upk` (invented): `Object` with `Vector`, a toy `TraceHitInfo`, and `ToyLookalike`, a struct that is not a
/// vector but has float X, Y and Z (review r1 of #633, item 3).
fn core() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&KINDS);
    let object = t.class("Object", 0);
    for name in ["Vector", "ToyLookalike"] {
        let s = toy_struct(&mut t, object, name, None);
        for f in ["X", "Y", "Z"] {
            t.prop("Float", s, f, 0, 0);
        }
    }
    let info = toy_struct(&mut t, object, "TraceHitInfo", None);
    t.prop("Float", info, "ToyField", 0, 0);
    t.p.build()
}

/// `Engine.upk` (invented): `Actor` (`Location`, `bCollideActors`, `bDeleteMe`, `CollisionComponent`, `Components`,
/// and the native `Trace` with the game's ten parameters), `Pawn`, `Terrain` and `PrimitiveComponent` (with its
/// `Owner`), over `Core.upk`'s `Object` and structs.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&KINDS);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let object = t.p.add_import("Core", "Class", core, "Object");
    let vector = t.p.add_import("Core", "ScriptStruct", object, "Vector");
    let info = t.p.add_import("Core", "ScriptStruct", object, "TraceHitInfo");
    let lookalike = t.p.add_import("Core", "ScriptStruct", object, "ToyLookalike");
    let actor = t.class("Actor", object);
    t.prop("Struct", actor, "Location", 0, vector);
    t.prop("Bool", actor, "bCollideActors", 0, 0);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    t.prop("Object", actor, "CollisionComponent", 0, object);
    toy_array_of(&mut t, actor, "Components", "Object", object);
    let s = |kind: &str, name: &str, flags: u64, type_ref: i32| Param { flags, type_ref, ..Param::new(kind, name) };
    let params = [
        s("Struct", "HitLocation", CPF_OUT, vector),
        s("Struct", "HitNormal", CPF_OUT, vector),
        s("Struct", "TraceEnd", 0, vector),
        s("Struct", "TraceStart", CPF_OPT, vector),
        s("Bool", "bTraceActors", CPF_OPT, 0),
        s("Struct", "Extent", CPF_OPT, vector),
        s("Struct", "HitInfo", CPF_OPT | CPF_OUT, info),
        s("Int", "ExtraTraceFlags", CPF_OPT, 0),
        s("Bool", "bTraceBulletListeners", CPF_OPT, 0),
        s("Object", "BulletListenerSource", CPF_OPT, object),
    ];
    t.function(actor, "Trace", &params, Some(("Object", actor)), &[], &FunctionSpec::new(NATIVE_FLAGS, "Trace"), |_| None);
    t.class("Pawn", actor);
    t.class("Terrain", actor);
    let primitive = t.class("PrimitiveComponent", object);
    t.prop("Object", primitive, "Owner", 0, actor);
    t.prop("Bool", primitive, "CollideActors", 0, 0);
    // NATIVE_ACTOR_TRACE.md section 5, test 8: a primitive class declaring `CollideActors` as an int.
    let int_collide = t.class("ToyIntCollidePrimitive", primitive);
    t.prop("Int", int_collide, "CollideActors", 0, 0);
    // Review r1 of #663: a primitive class declaring `CollideActors` as a static `bool[2]`.
    let pair_collide = t.class("ToyPairCollidePrimitive", primitive);
    let mut pair = ow_package::testkit::Export::new(
        t.import("BoolProperty"),
        "CollideActors",
        ow_package::testkit::reflect::property_payload(&t.none, 2, 0, &[]),
    );
    pair.outer = pair_collide;
    t.p.add_export_with(pair);
    let light = t.class("ToyLightComponent", object);
    // A field of the look-alike's type, so the fixture knows the struct.
    t.prop("Struct", light, "ToyShape", 0, lookalike);
    t.p.build()
}

/// `WillowGame.upk` (invented): `WillowPawn`, `WillowAIPawn`, a script-only child of it, the two recorded native
/// descendants, and `WillowPlayerPawn`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let willow_pawn = t.class("WillowPawn", pawn);
    let ai = t.class("WillowAIPawn", willow_pawn);
    t.class("ToyScriptAIPawn", ai);
    t.class("WillowAICranePawn", ai);
    t.class("DeployableTurretActor", ai);
    t.class("WillowPlayerPawn", willow_pawn);
    t.p.build()
}

/// `GD_ToyTrace.upk` (invented): `Holder`, an `Actor` export whose `CollisionComponent` names the `PrimitiveComponent`
/// export `Lazy`. Reading the loaded holder gives `Lazy` as an unloaded store handle.
fn definitions() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let primitive = t.p.add_import("Core", "Class", engine, "PrimitiveComponent");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let mut body = ow_package::testkit::ints(&[0]);
    body.extend_from_slice(&t.none);
    let lazy = t.p.add_export(primitive, "Lazy", body);
    let mut holder = ow_package::testkit::ints(&[0]);
    holder.extend(t.p.tag("CollisionComponent", "ObjectProperty", &ow_package::testkit::ints(&[lazy])));
    holder.extend_from_slice(&t.none);
    t.p.add_export(actor, "Holder", holder);
    t.p.build()
}

/// One sweep the service was asked: its kind, extent, start and end.
type Asked = (&'static str, [f32; 3], [f32; 3], [f32; 3]);

/// An invented collision service: the level answer the test sets, the bounds it holds per primitive, and the sweeps
/// it was asked.
#[derive(Default)]
struct Toy {
    level: Cell<Option<Sweep>>,
    bounds: RefCell<DetMap<ObjRef, Bounds>>,
    asked: RefCell<Vec<Asked>>,
    /// Every primitive `actor_bounds` was asked about, in order (review r1 of #663).
    bounds_asked: RefCell<Vec<ObjRef>>,
}

impl SpawnCollision for Toy {
    fn box_test(&self, _extent: [f32; 3], _at: [f32; 3], _mode: Mode) -> Result<Test, VmError> {
        panic!("Actor.Trace asks no box test")
    }
    fn box_sweep(&self, extent: [f32; 3], start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError> {
        self.asked.borrow_mut().push(("box", extent, start, end));
        Ok(self.level.get().unwrap_or(Sweep::Clear))
    }
    fn point_sweep(&self, start: [f32; 3], end: [f32; 3]) -> Result<Sweep, VmError> {
        self.asked.borrow_mut().push(("point", [0.0; 3], start, end));
        Ok(self.level.get().unwrap_or(Sweep::Clear))
    }
    fn actor_bounds(&self, _space: &ObjectSpace, primitive: ObjRef) -> Result<Option<Bounds>, VmError> {
        self.bounds_asked.borrow_mut().push(primitive);
        Ok(self.bounds.borrow().get(&primitive).copied())
    }
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

/// A world with an actor list (a plain `Actor` as its `WorldInfo`) and, unless `service` is None, the collision
/// service.
fn fx(service: Option<Toy>) -> (Fx, Option<Rc<Toy>>) {
    let dir = std::env::temp_dir().join(format!("ow-world-trace-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, e, w, d) = (core(), engine(), willow(), definitions());
    let files: [(&str, &[u8]); 4] =
        [("Core.upk", &c), ("Engine.upk", &e), ("WillowGame.upk", &w), ("GD_ToyTrace.upk", &d)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    // The class code is loaded by the fixture, so no answer below is the first to load a class.
    for class in [
        "Core.Object",
        "Engine.Actor",
        "Engine.Pawn",
        "Engine.Terrain",
        "Engine.PrimitiveComponent",
        "Engine.ToyLightComponent",
        "WillowGame.WillowPawn",
        "WillowGame.WillowAIPawn",
        "WillowGame.ToyScriptAIPawn",
        "WillowGame.WillowAICranePawn",
        "WillowGame.DeployableTurretActor",
        "WillowGame.WillowPlayerPawn",
    ] {
        vm.find_class(class).unwrap();
    }
    let f = vm.find_function(PATH).unwrap();
    vm.register_native_for(f, Rc::new(trace)).unwrap();
    let actor = vm.find_class("Engine.Actor").unwrap();
    let info =vm.space.alloc(actor, "ToyWorldInfo", None, Origin::Runtime).unwrap();
    let list = vm.add_service(Actors::new(actor)).unwrap();
    list.borrow_mut().place_world_info(info).unwrap();
    let toy = service.map(Rc::new);
    if let Some(t) = &toy {
        install_collision(&mut vm, t.clone()).unwrap();
    }
    (Fx { vm, dir }, toy)
}

type Counters = (u64, u64, u64, usize);

impl Fx {
    fn live(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn place(&mut self, o: ObjRef) {
        let list = crate::actors::actors_of(&self.vm).unwrap();
        list.borrow_mut().place(o).unwrap();
    }

    /// A live `Actor` placed in the level, with one primitive as its `CollisionComponent`: (actor, primitive). Both
    /// collide flags are set, so the hash would admit the primitive (NATIVE_ACTOR_TRACE.md section 5).
    fn obstacle(&mut self, class: &str, name: &str) -> (ObjRef, ObjRef) {
        self.obstacle_of(class, "Engine.PrimitiveComponent", name)
    }

    /// [`Fx::obstacle`] with a primitive of `primitive_class`.
    fn obstacle_of(&mut self, class: &str, primitive_class: &str, name: &str) -> (ObjRef, ObjRef) {
        let a = self.live(class, name);
        let p = self.live(primitive_class, &format!("{name}Mesh"));
        self.set(a, "CollisionComponent", Value::Object(Some(p)));
        self.set(a, "bCollideActors", Value::Bool(true));
        if primitive_class == "Engine.PrimitiveComponent" {
            self.set(p, "CollideActors", Value::Bool(true));
        }
        self.place(a);
        (a, p)
    }

    fn vector(&self, v: [f32; 3]) -> Value {
        self.xyz("Core.Object.Vector", v)
    }

    /// A struct of `path` with X, Y and Z set (the vector, or the look-alike).
    fn xyz(&self, path: &str, v: [f32; 3]) -> Value {
        let ty = self.vm.space.classes.struct_by_path(path).expect("a toy struct");
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let mut fields = vec![Value::Float(0.0); 3];
        for (k, n) in ["X", "Y", "Z"].iter().enumerate() {
            let (entry, _) = layout.field(self.vm.space.names.find(n).unwrap()).unwrap();
            fields[usize::try_from(entry).unwrap()] = Value::Float(v[k]);
        }
        Value::structure(ty, fields)
    }

    fn hit_info(&self) -> Value {
        let ty = self.vm.space.classes.struct_by_path("Core.Object.TraceHitInfo").expect("the toy TraceHitInfo");
        Value::structure(ty, vec![Value::Float(0.0)])
    }

    /// A target `WillowAIPawn` at the origin, placed in the level (the source actor of link 9's trace).
    fn target(&mut self) -> ObjRef {
        let t = self.live("WillowGame.WillowAIPawn", "ToyTarget");
        let p = self.live("Engine.PrimitiveComponent", "ToyTargetCylinder");
        self.set(t, "CollisionComponent", Value::Object(Some(p)));
        self.place(t);
        t
    }

    /// Link 9's arguments (note section 2): down 200 from `start`, through actors, extent (1, 1, 1), extra flags 8.
    fn link9(&self, start: [f32; 3]) -> Vec<Value> {
        let end = [start[0], start[1], start[2] - 200.0];
        vec![
            self.vector([9.0; 3]),
            self.vector([9.0; 3]),
            self.vector(end),
            self.vector(start),
            Value::Bool(true),
            self.vector([1.0; 3]),
        ]
    }

    /// Calls `Engine.Actor.Trace` from the host on `this`; with `extra` appended to `args` as the remaining
    /// parameters in order (`HitInfo` first).
    fn call(&mut self, this: Option<ObjRef>, args: Vec<Value>) -> Result<(Value, Value, Value), VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        let r = self.vm.call(f, this, args)?;
        Ok((r.ret, r.outs[0].clone(), r.outs[1].clone()))
    }

    fn counters(&self) -> Counters {
        (self.vm.space.heap.committed_writes(), self.vm.rng.draws(), self.vm.defs.load_transitions(), self.vm.space.names.len())
    }

    /// The call must refuse with a `NotYet` of `missing` whose text is `what`, write nothing to the outs, and leave
    /// the three counters and the name table as they were.
    fn refuses(&mut self, this: ObjRef, args: Vec<Value>, missing: Missing, what: &str) {
        let before = self.counters();
        match self.call(Some(this), args) {
            Err(VmError::NotYet(n)) => {
                assert_eq!((n.missing, n.what.as_ref(), n.record), (missing, what, RECORD), "{n:?}");
            }
            other => panic!("expected NotYet {what:?}, got {other:?}"),
        }
        assert_eq!(self.counters(), before, "{what}: write-free, RNG-free, load-free, no name interned");
    }

    /// Runs the native directly with one entry per parameter, `None` for an omitted one (what script's
    /// `EmptyParmValue` gives; a host `Vm::call` cannot omit a middle argument).
    fn direct(&mut self, this: ObjRef, args: Vec<Option<Value>>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        let args = args
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                let out = matches!(i, 0 | 1 | 6);
                match v {
                    Some(v) => Arg::new(v, out, true),
                    None => Arg::new(Value::None, out, false),
                }
            })
            .collect();
        let mut c = NativeCall::new(&mut self.vm, f, Some(this), args);
        trace(&mut c)
    }
}

fn not_read() -> Missing {
    Missing::NotRead
}

fn layer(name: &'static str) -> Missing {
    Missing::LayerAbsent(name)
}

/// Test 1: link 9's shape on a no-level world with no other actors: None, both outs zero.
#[test]
fn link_9_on_an_empty_world_is_a_miss_with_zero_outs() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let t = x.target();
    let zero = x.vector([0.0; 3]);
    let args = x.link9([0.0; 3]);
    let (ret, location, normal) = x.call(Some(t), args).unwrap();
    assert_eq!(ret, Value::Object(None));
    assert_eq!((location, normal), (zero.clone(), zero));
    let asked = toy.unwrap().asked.borrow().clone();
    assert_eq!(asked, vec![("box", [1.0; 3], [0.0; 3], [0.0, 0.0, -200.0])], "a box sweep from TraceStart to TraceEnd");
    // The no-level stand-in answers the same.
    let (mut x, _) = fx(None);
    install_collision(&mut x.vm, Rc::new(crate::placement::NoLevel)).unwrap();
    let t = x.target();
    let args = x.link9([0.0; 3]);
    assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None));
}

/// Test 2: a level hit from the service is T6, with no out written; `Unsupported` is the level sweep's refusal.
#[test]
fn a_level_hit_is_t6_and_an_unsupported_sweep_is_named() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    toy.level.set(Some(Sweep::Hit { location: [0.0, 0.0, -100.0], normal: [0.0, 0.0, 1.0], fraction: 0.5 }));
    let args = x.link9([0.0; 3]);
    x.refuses(t, args, not_read(), LEVEL_HIT);
    toy.level.set(Some(Sweep::Unsupported));
    let args = x.link9([0.0; 3]);
    x.refuses(t, args, not_read(), LEVEL_SWEEP);
}

/// The source actor's own primitive, read from its `CollisionComponent`.
fn own_primitive(x: &Fx, t: ObjRef) -> ObjRef {
    let class = x.vm.space.heap.get(t).unwrap().class;
    let p = x.vm.space.prop(class, "CollisionComponent").unwrap();
    let Value::Object(Some(own)) = x.vm.space.read(t, PropAddr::scalar(p)).unwrap().clone() else {
        panic!("the holder names a component")
    };
    own
}

/// Sets both collide flags on the source `t` and its own primitive, so that only the source exclusions keep a meeting
/// primitive of the source out of T7 (review r1 of #663: with the flags clear, the hash skip alone would give the miss).
fn collide_source(x: &mut Fx, t: ObjRef) {
    let own = own_primitive(x, t);
    x.set(t, "bCollideActors", Value::Bool(true));
    x.set(own, "CollideActors", Value::Bool(true));
}

const MEETS: Bounds = Bounds { min: [-5.0, -5.0, -150.0], max: [5.0, 5.0, -50.0] };
const FAR: Bounds = Bounds { min: [500.0, 500.0, 0.0], max: [510.0, 510.0, 10.0] };

/// Tests 3 and 4: a primitive whose bounds meet the swept box is T7, for both values of `bTraceActors` (the flag word
/// reaches the collision hash either way, so there is no false miss); the source actor's own primitive, which meets
/// the box too, never refuses.
#[test]
fn a_meeting_primitive_is_t7_and_the_source_is_left_out() {
    for trace_actors in [true, false] {
        let (mut x, toy) = fx(Some(Toy::default()));
        let toy = toy.unwrap();
        let t = x.target();
        collide_source(&mut x, t);
        let own = own_primitive(&x, t);
        toy.bounds.borrow_mut().insert(own, MEETS);
        let args = x.link9([0.0; 3]);
        assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None), "only the source's own primitive");
        assert!(!toy.bounds_asked.borrow().contains(&own), "the source's own primitive is left out before its bounds");
        // Its `Owner` naming another colliding actor: only the walk's own skip of the source actor keeps it out.
        let other = x.live("Engine.Actor", "ToyHolder");
        x.set(other, "bCollideActors", Value::Bool(true));
        x.set(own, "Owner", Value::Object(Some(other)));
        let args = x.link9([0.0; 3]);
        assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None), "the source actor's walk is skipped");
        assert!(!toy.bounds_asked.borrow().contains(&own), "still unasked");
        let (_, near) = x.obstacle("Engine.Actor", "Crate");
        toy.bounds.borrow_mut().insert(near, MEETS);
        let mut args = x.link9([0.0; 3]);
        args[4] = Value::Bool(trace_actors);
        let what = format!("Actor.Trace: {SOURCE_FILTERS} (the live actor primitive Crate's CrateMesh)");
        x.refuses(t, args, not_read(), &what);
    }
    assert_ne!(trace_flags(false, 8) & crate::trace_filter::SWEEP_HASH_MASK, 0);
}

/// A primitive owned by the source through its `Owner` field, met in another actor's walk, is left out too; a
/// terrain's meeting primitive names the terrain handling.
#[test]
fn an_owned_primitive_is_left_out_and_a_terrain_names_its_handling() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    collide_source(&mut x, t);
    let (_, carried) = x.obstacle("Engine.Actor", "Carrier");
    x.set(carried, "Owner", Value::Object(Some(t)));
    toy.bounds.borrow_mut().insert(carried, MEETS);
    let args = x.link9([0.0; 3]);
    assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None));
    assert!(!toy.bounds_asked.borrow().contains(&carried), "a primitive the source owns is left out before its bounds");
    let (_, ground) = x.obstacle("Engine.Terrain", "Ground");
    toy.bounds.borrow_mut().insert(ground, MEETS);
    let args = x.link9([0.0; 3]);
    let what = format!("Actor.Trace: {} (the live actor primitive Ground's GroundMesh)", crate::trace_filter::TERRAIN_HANDLING);
    x.refuses(t, args, not_read(), &what);
}

/// Test 5: a primitive with known-disjoint bounds is left out before any filter field past the two collide flags is
/// read, and a non-primitive component is skipped: the answer is the miss, and the heap
/// is untouched (a host call keeps the two outs in its result).
#[test]
fn a_disjoint_primitive_is_left_out_unread() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    let (_, far) = x.obstacle("Engine.Actor", "Faraway");
    toy.bounds.borrow_mut().insert(far, FAR);
    let lamp = x.live("Engine.ToyLightComponent", "Lamp");
    let holder = x.live("Engine.Actor", "LampHolder");
    x.set(holder, "Components", Value::array(vec![Value::Object(Some(lamp)), Value::Object(None)]));
    x.place(holder);
    let before = x.counters();
    let args = x.link9([0.0; 3]);
    let (ret, location, _) = x.call(Some(t), args).unwrap();
    assert_eq!((ret, location), (Value::Object(None), x.vector([0.0; 3])));
    assert_eq!(x.counters(), before);
}

/// Test 6: missing bounds give T5; a destroyed actor is skipped.
#[test]
fn missing_bounds_are_t5() {
    let (mut x, _toy) = fx(Some(Toy::default()));
    let t = x.target();
    let (gone, _) = x.obstacle("Engine.Actor", "Gone");
    x.set(gone, "bDeleteMe", Value::Bool(true));
    let args = x.link9([0.0; 3]);
    assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None), "a destroyed actor is skipped");
    x.obstacle("Engine.Actor", "NoBounds");
    let args = x.link9([0.0; 3]);
    x.refuses(t, args, layer(BOUNDS_LAYER), NO_BOUNDS);
}

/// Test 7: the omitted defaults. `TraceStart` is the actor's `Location`, `bTraceActors` its `bCollideActors` (clear
/// here, and the walk still runs), and `Extent` zero, so the level part is a point sweep.
#[test]
fn omitted_arguments_take_the_defaults() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    let at = x.vector([10.0, 20.0, 30.0]);
    x.set(t, "Location", at);
    let args = vec![x.vector([9.0; 3]), x.vector([9.0; 3]), x.vector([10.0, 20.0, -170.0])];
    assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None));
    assert_eq!(toy.asked.borrow().clone(), vec![("point", [0.0; 3], [10.0, 20.0, 30.0], [10.0, 20.0, -170.0])]);
    let (_, near) = x.obstacle("Engine.Actor", "Crate");
    toy.bounds.borrow_mut().insert(near, Bounds { min: [5.0, 15.0, -100.0], max: [15.0, 25.0, 0.0] });
    let args = vec![x.vector([9.0; 3]), x.vector([9.0; 3]), x.vector([10.0, 20.0, -170.0])];
    let what = format!("Actor.Trace: {SOURCE_FILTERS} (the live actor primitive Crate's CrateMesh)");
    x.refuses(t, args, not_read(), &what);
}

/// The flag word (note section 1).
#[test]
fn the_flag_word_follows_the_note() {
    use crate::trace_filter::*;
    assert_eq!(trace_flags(false, 0), SWEEP);
    let actors = PAWNS | MOVERS | LEVEL_BSP | VOLUMES | OTHER_ACTORS | WORLD_GEOMETRY | TERRAIN;
    assert_eq!(trace_flags(true, 0), actors | PROJ_TARGETS);
    assert_eq!(trace_flags(true, 8), actors | BLOCKING_ONLY);
}

/// Test 8: no receiver, T1, a malformed argument, T2 (each option, ahead of T3), T3 and T4, each before any write,
/// with the counters and the name table unchanged.
#[test]
fn the_preflight_refusals_are_write_free_and_in_order() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    let args = x.link9([0.0; 3]);
    assert!(matches!(x.call(None, args), Err(VmError::Script { .. })), "no receiver");
    let ai = x.vm.find_class(SOURCE_CLASS).unwrap();
    let stand_in = x.vm.space.alloc_stand_in(ai, "ToyStandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    let args = x.link9([0.0; 3]);
    x.refuses(stand_in, args, not_read(), "Actor.Trace: the actor ToyStandIn is not loaded (a load is a write)");
    let mut args = x.link9([0.0; 3]);
    args[2] = Value::Float(1.0);
    let before = x.counters();
    assert!(matches!(x.call(Some(t), args), Err(VmError::Script { .. })), "TraceEnd is not a vector");
    assert_eq!(x.counters(), before);
    // T2 on a class T3 would refuse, so each option is seen to come first.
    let player = x.live("WillowGame.WillowPlayerPawn", "ToyPlayer");
    let info = x.hit_info();
    let cases: [(usize, Value, &str); 4] = [
        (6, info, "a HitInfo argument"),
        (7, Value::Int(8 | 2), "ExtraTraceFlags 10 (a bit other than 8)"),
        (8, Value::Bool(true), "bTraceBulletListeners set"),
        (9, Value::Object(Some(t)), "a BulletListenerSource"),
    ];
    for (at, v, option) in cases {
        let mut args: Vec<Option<Value>> = x.link9([0.0; 3]).into_iter().map(Some).collect();
        args.resize(10, None);
        args[at] = Some(v);
        let before = x.counters();
        match x.direct(player, args) {
            Err(VmError::NotYet(n)) => {
                let what = format!("Actor.Trace: {option} (its effect on the query was not read)");
                assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, what.as_str()));
            }
            other => panic!("{option}: {other:?}"),
        }
        assert_eq!(x.counters(), before, "{option}");
    }
    // Bit 8 alone, a clear bTraceBulletListeners and a None source are read options: no T2.
    let mut args: Vec<Option<Value>> = x.link9([0.0; 3]).into_iter().map(Some).collect();
    args.extend([None, Some(Value::Byte(8)), Some(Value::Bool(false)), Some(Value::Object(None))]);
    assert_eq!(x.direct(t, args).unwrap(), Value::Object(None));
    // T3: every class but WillowAIPawn and its script-only children.
    for class in ["WillowGame.WillowPlayerPawn", "WillowGame.WillowAICranePawn", "WillowGame.DeployableTurretActor", "Engine.Actor"] {
        let o = x.live(class, "ToySource");
        let args = x.link9([0.0; 3]);
        x.refuses(o, args, not_read(), &format!("Actor.Trace: the trace source of {class} (its virtual was not read)"));
    }
    // From another source the first target is just another actor: its primitive is given far bounds.
    toy.bounds.borrow_mut().insert(own_primitive(&x, t), FAR);
    let child = x.live("WillowGame.ToyScriptAIPawn", "ToyChild");
    let args = x.link9([0.0; 3]);
    assert_eq!(x.call(Some(child), args).unwrap().0, Value::Object(None), "a script-only child takes the rule");
    // T4: no collision service.
    let (mut y, _) = fx(None);
    let t2 = y.target();
    let args = y.link9([0.0; 3]);
    y.refuses(t2, args, layer(COLLISION_LAYER), NO_COLLISION);
}

/// Test 8, continued: an unresident candidate and an unresident owner each refuse, nothing loaded. The candidate is
/// a genuine unloaded store handle (the loaded holder's `CollisionComponent`).
#[test]
fn unresident_candidates_and_owners_refuse_without_a_load() {
    let (mut x, _toy) = fx(Some(Toy::default()));
    let t = x.target();
    let path = ow_core::ObjectPath::parse("GD_ToyTrace.Holder").unwrap();
    let holder = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let lazy = own_primitive(&x, holder);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "a genuinely unloaded handle");
    x.place(holder);
    let args = x.link9([0.0; 3]);
    x.refuses(t, args, not_read(), "Actor.Trace: the component Lazy is not loaded (a load is a write)");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "still unloaded");
    let (mut x, _toy) = fx(Some(Toy::default()));
    let t = x.target();
    let actor = x.vm.find_class("Engine.Actor").unwrap();
    let ghost = x.vm.space.alloc_stand_in(actor, "ToyGhost", Origin::Resource { package: 0, index: -1 }).unwrap();
    let (_, part) = x.obstacle("Engine.Actor", "Haunted");
    x.set(part, "Owner", Value::Object(Some(ghost)));
    let args = x.link9([0.0; 3]);
    x.refuses(t, args, not_read(), "Actor.Trace: the actor ToyGhost is not loaded (a load is a write)");
}


impl Fx {
    /// Runs the native directly with the given arguments (an alias or a wrong `out` mark the host call cannot build).
    fn direct_args(&mut self, this: ObjRef, args: Vec<Arg>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        let mut c = NativeCall::new(&mut self.vm, f, Some(this), args);
        trace(&mut c)
    }

    fn location_of(&self, o: ObjRef) -> (PropAddr, Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let at = PropAddr::scalar(self.vm.space.prop(class, "Location").unwrap());
        (at, self.vm.space.read(o, at).unwrap().clone())
    }
}

/// Review r1 of #633, item 1: both outs and their alias writes are checked before either is written. A HitLocation
/// aliasing a property with a HitNormal not marked out refuses with the property and the counters unchanged; with
/// both outs aliasing properties, both properties are zeroed.
#[test]
fn an_out_alias_is_not_written_when_the_other_out_is_refused() {
    let (mut x, _toy) = fx(Some(Toy::default()));
    let t = x.target();
    let witness = x.live("Engine.Actor", "ToyWitness");
    let seven = x.vector([7.0; 3]);
    x.set(witness, "Location", seven.clone());
    let (at, _) = x.location_of(witness);
    let mut args = x.link9([0.0; 3]);
    let normal = args.remove(1);
    let location = args.remove(0);
    let mut given = vec![Arg::aliasing_property(witness, at, location.clone()), Arg::new(normal.clone(), false, true)];
    given.extend(args.iter().cloned().map(|v| Arg::new(v, false, true)));
    let before = x.counters();
    match x.direct_args(t, given) {
        Err(VmError::BadArgument { index, .. }) => assert_eq!(index, 1),
        other => panic!("expected the HitNormal refusal, got {other:?}"),
    }
    assert_eq!(x.location_of(witness).1, seven, "the HitLocation alias was not written");
    assert_eq!(x.counters(), before, "write-free, RNG-free, load-free, no name interned");
    // Both outs alias properties: the clear sweep zeroes both.
    let other = x.live("Engine.Actor", "ToyOtherWitness");
    x.set(other, "Location", seven.clone());
    let (other_at, _) = x.location_of(other);
    let mut given = vec![Arg::aliasing_property(witness, at, location), Arg::aliasing_property(other, other_at, normal)];
    given.extend(args.into_iter().map(|v| Arg::new(v, false, true)));
    assert_eq!(x.direct_args(t, given).unwrap(), Value::Object(None));
    let zero = x.vector([0.0; 3]);
    assert_eq!((x.location_of(witness).1, x.location_of(other).1), (zero.clone(), zero.clone()));
    // Review r2 of #633: both outs aliasing one place. The first write cannot invalidate the second's destination
    // (a vector changes no array or reference), so both checks hold through both writes.
    x.set(witness, "Location", seven.clone());
    let mut args = x.link9([0.0; 3]);
    let normal = args.remove(1);
    let location = args.remove(0);
    let mut given = vec![Arg::aliasing_property(witness, at, location), Arg::aliasing_property(witness, at, normal)];
    given.extend(args.into_iter().map(|v| Arg::new(v, false, true)));
    assert_eq!(x.direct_args(t, given).unwrap(), Value::Object(None));
    assert_eq!(x.location_of(witness).1, zero);
}

/// Review r1 of #633, item 2: a non-finite endpoint or extent, or a negative extent, refuses before the service is
/// asked; a primitive's non-finite or inverted bounds refuse instead of proving a miss.
#[test]
fn bad_query_values_and_unsound_bounds_refuse() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    let cases: [(usize, [f32; 3]); 5] = [
        (3, [f32::NAN, 0.0, 0.0]),
        (2, [0.0, f32::INFINITY, -200.0]),
        (5, [1.0, f32::NAN, 1.0]),
        (5, [1.0, 1.0, f32::NEG_INFINITY]),
        (5, [1.0, -1.0, 1.0]),
    ];
    for (at, v) in cases {
        let mut args = x.link9([0.0; 3]);
        args[at] = x.vector(v);
        x.refuses(t, args, not_read(), BAD_QUERY);
    }
    assert!(toy.asked.borrow().is_empty(), "no sweep was asked");
    let (_, odd) = x.obstacle("Engine.Actor", "Odd");
    for bounds in [
        Bounds { min: [f32::NAN, -5.0, -150.0], max: [5.0, 5.0, -50.0] },
        Bounds { min: [-5.0, -5.0, -150.0], max: [5.0, f32::INFINITY, -50.0] },
        Bounds { min: [600.0, -5.0, -150.0], max: [500.0, 5.0, -50.0] },
    ] {
        toy.bounds.borrow_mut().insert(odd, bounds);
        let args = x.link9([0.0; 3]);
        x.refuses(t, args, not_read(), BAD_BOUNDS);
    }
}

/// Review r1 of #633, item 3: a struct with float X, Y and Z that is not `Core.Object.Vector` is a script error at
/// argument validation, for TraceEnd, TraceStart and both outs.
#[test]
fn a_vector_look_alike_is_a_script_error() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    for at in [2, 3, 0, 1] {
        let mut args = x.link9([0.0; 3]);
        args[at] = x.xyz("Core.Object.ToyLookalike", [1.0; 3]);
        let before = x.counters();
        match x.call(Some(t), args) {
            Err(VmError::Script { .. }) => {}
            other => panic!("argument {at}: expected a script error, got {other:?}"),
        }
        assert_eq!(x.counters(), before, "argument {at}");
    }
    assert!(toy.asked.borrow().is_empty(), "refused at argument validation, before the service");
}

/// NATIVE_ACTOR_TRACE.md section 5, tests 1 and 2: a primitive the collision hash cannot admit is left out before its
/// bounds are asked. With no bounds in the service it would be T5; instead the trace is a miss, and nothing is written,
/// loaded, drawn or interned. Test 1: the primitive's `CollideActors` clear. Test 2: the owner's `bCollideActors` clear
/// and the primitive's set.
#[test]
fn a_primitive_the_hash_cannot_admit_is_left_out_before_its_bounds() {
    for (primitive_flag, owner_flag) in [(false, true), (true, false), (false, false)] {
        let (mut x, toy) = fx(Some(Toy::default()));
        let toy = toy.unwrap();
        let t = x.target();
        let (a, p) = x.obstacle("Engine.Actor", "NoBounds");
        x.set(p, "CollideActors", Value::Bool(primitive_flag));
        x.set(a, "bCollideActors", Value::Bool(owner_flag));
        let before = x.counters();
        let args = x.link9([0.0; 3]);
        let (ret, location, _) = x.call(Some(t), args).unwrap();
        assert_eq!((ret, location), (Value::Object(None), x.vector([0.0; 3])), "({primitive_flag}, {owner_flag})");
        assert_eq!(x.counters(), before, "({primitive_flag}, {owner_flag})");
        assert!(!toy.bounds_asked.borrow().contains(&p), "({primitive_flag}, {owner_flag}): its bounds never asked");
    }
}

/// NATIVE_ACTOR_TRACE.md section 5, tests 6 and 7: with both flags set, missing bounds are T5 and meeting bounds
/// T7, as before.
#[test]
fn an_admissible_primitive_keeps_t5_and_t7() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    let (_, p) = x.obstacle("Engine.Actor", "Crate");
    let args = x.link9([0.0; 3]);
    x.refuses(t, args, layer(BOUNDS_LAYER), NO_BOUNDS);
    toy.bounds.borrow_mut().insert(p, MEETS);
    let args = x.link9([0.0; 3]);
    let what = format!("Actor.Trace: {SOURCE_FILTERS} (the live actor primitive Crate's CrateMesh)");
    x.refuses(t, args, not_read(), &what);
}

/// NATIVE_ACTOR_TRACE.md section 5, test 8: a primitive class declaring `CollideActors` as an int, or as a static
/// `bool[2]`, is a `VmError::Script` naming the class, with nothing written, loaded, drawn or interned.
#[test]
fn a_collide_flag_of_another_kind_is_a_script_error() {
    // An int, and (review r1 of #663) a static `bool[2]`.
    for class in ["ToyIntCollidePrimitive", "ToyPairCollidePrimitive"] {
        let (mut x, toy) = fx(Some(Toy::default()));
        let toy = toy.unwrap();
        let t = x.target();
        let (_, p) = x.obstacle_of("Engine.Actor", &format!("Engine.{class}"), "Odd");
        toy.bounds.borrow_mut().insert(p, MEETS);
        let before = x.counters();
        let args = x.link9([0.0; 3]);
        match x.call(Some(t), args) {
            Err(VmError::Script { message, .. }) => {
                assert!(message.contains(class) && message.contains("CollideActors"), "{message}")
            }
            other => panic!("{class}: expected a script error, got {other:?}"),
        }
        assert_eq!(x.counters(), before, "{class}");
        assert!(toy.bounds_asked.borrow().is_empty(), "{class}: refused before its bounds");
    }
}

/// NATIVE_ACTOR_TRACE.md section 5, tests 3 and 5 (#659 r1): a primitive with the collide flags clear whose bounds
/// meet the swept box is still a miss (its bounds are never asked); a terrain's primitive with both flags clear is not
/// skipped, so its meeting bounds name the terrain handling (T7).
#[test]
fn a_clear_flag_wins_over_meeting_bounds_but_not_for_terrain() {
    let (mut x, toy) = fx(Some(Toy::default()));
    let toy = toy.unwrap();
    let t = x.target();
    let (a, p) = x.obstacle("Engine.Actor", "Quiet");
    x.set(p, "CollideActors", Value::Bool(false));
    x.set(a, "bCollideActors", Value::Bool(false));
    toy.bounds.borrow_mut().insert(p, MEETS);
    let before = x.counters();
    let args = x.link9([0.0; 3]);
    assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None), "meeting bounds, flags clear: a miss");
    assert_eq!(x.counters(), before);
    assert!(!toy.bounds_asked.borrow().contains(&p), "a clear flag: its bounds never asked");
    // Each flag clear on its own, with meeting bounds: a miss too, the bounds still unasked.
    for (primitive_flag, owner_flag) in [(false, true), (true, false)] {
        x.set(p, "CollideActors", Value::Bool(primitive_flag));
        x.set(a, "bCollideActors", Value::Bool(owner_flag));
        let args = x.link9([0.0; 3]);
        assert_eq!(x.call(Some(t), args).unwrap().0, Value::Object(None), "({primitive_flag}, {owner_flag})");
        assert!(!toy.bounds_asked.borrow().contains(&p), "({primitive_flag}, {owner_flag}): its bounds never asked");
    }
    let (ground, gp) = x.obstacle("Engine.Terrain", "Ground");
    x.set(gp, "CollideActors", Value::Bool(false));
    x.set(ground, "bCollideActors", Value::Bool(false));
    toy.bounds.borrow_mut().insert(gp, MEETS);
    let args = x.link9([0.0; 3]);
    let what = format!(
        "Actor.Trace: {} (the live actor primitive Ground's GroundMesh)",
        crate::trace_filter::TERRAIN_HANDLING
    );
    x.refuses(t, args, not_read(), &what);
    assert!(toy.bounds_asked.borrow().contains(&gp), "a terrain's primitive goes on to its bounds");
    assert!(!toy.bounds_asked.borrow().contains(&p), "the skipped primitive's bounds were still never asked");
}

/// NATIVE_ACTOR_TRACE.md section 5, test 4, order (b): a primitive with `CollideActors` clear whose owner is an
/// unresident stand-in is T1 naming the owner, so residency comes before the skip, and nothing is loaded.
#[test]
fn residency_comes_before_the_skip() {
    let (mut x, _toy) = fx(Some(Toy::default()));
    let t = x.target();
    let actor = x.vm.find_class("Engine.Actor").unwrap();
    let ghost = x.vm.space.alloc_stand_in(actor, "ToyGhost", Origin::Resource { package: 0, index: -1 }).unwrap();
    let (_, part) = x.obstacle("Engine.Actor", "Faded");
    x.set(part, "CollideActors", Value::Bool(false));
    x.set(part, "Owner", Value::Object(Some(ghost)));
    let args = x.link9([0.0; 3]);
    x.refuses(t, args, not_read(), "Actor.Trace: the actor ToyGhost is not loaded (a load is a write)");
}
