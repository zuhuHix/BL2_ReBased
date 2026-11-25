//! Tests of `Actor.GetBoundingCylinder` in Off (lane IMPL-GET-BOUNDING-CYLINDER, NATIVE_GET_BOUNDING_CYLINDER.md at
//! 6800f603, section 4's tests list) on invented packages with the game's class and field names (AGENTS.md rule 9).
//! Every value is invented. AI-assisted (Claude Code).

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
use ow_package::testkit::script::{CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE_FLAGS: u32 = 0x400 | 0x20000;

/// `Engine.upk` (invented): the component classes, `Actor` (`CollisionComponent`, `Components`), `Pawn` and
/// `NavigationPoint` (`CylinderComponent`), each of the three declaring the native, and `Trigger`, an actor class whose
/// body was not read.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ObjectProperty",
        "BoolProperty",
        "FloatProperty",
        "ArrayProperty",
        "ScriptStruct",
        "StructProperty",
        "IntProperty",
    ]);
    let object = t.class("Object", 0);
    let vector = ow_package::testkit::reflect::toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let primitive = t.class("PrimitiveComponent", object);
    t.prop("Bool", primitive, "bAttached", 0, 0);
    t.prop("Bool", primitive, "CollideActors", 0, 0);
    t.prop("Bool", primitive, "BulletListener", 0, 0);
    for b in ["AbsoluteTranslation", "AbsoluteRotation", "AbsoluteScale"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    t.prop("Struct", primitive, "Translation", 0, vector);
    t.prop("Object", primitive, "Owner", 0, object);
    let cylinder = t.class("CylinderComponent", primitive);
    t.prop("Float", cylinder, "CollisionRadius", 0, 0);
    t.prop("Float", cylinder, "CollisionHeight", 0, 0);
    t.class("SkeletalMeshComponent", primitive);
    // Lane IMPL-SKIP-FLAG: two more primitive classes whose per-component check was not read, and a toy one.
    t.class("StaticMeshComponent", primitive);
    t.class("ParticleSystemComponent", primitive);
    t.class("ToyCylinderKind", cylinder);
    // Review r1 of #660, item 2: a primitive whose BulletListener is declared as an int.
    let int_skip = t.class("ToyIntSkipComponent", primitive);
    t.prop("Int", int_skip, "BulletListener", 0, 0);
    t.class("DynamicLightEnvironmentComponent", object);
    let actor = t.class("Actor", object);
    t.prop("Struct", actor, "Location", 0, vector);
    // Review r1 of #660, item 1: a float the radius can be written to, and an object the height cannot.
    let toy_radius = t.p.next_export() + 1;
    t.prop("Float", actor, "ToyRadius", 0, 0);
    let toy_height = t.p.next_export() + 1;
    t.prop("Object", actor, "ToyHeight", 0, object);
    t.prop("Object", actor, "CollisionComponent", 0, object);
    let array = t.p.next_export();
    t.prop("Array", actor, "Components", 0, array + 1);
    t.prop("Object", array, "Components", 0, object);
    let pawn = t.class("Pawn", actor);
    t.prop("Object", pawn, "CylinderComponent", 0, object);
    let nav = t.class("NavigationPoint", actor);
    t.prop("Object", nav, "CylinderComponent", 0, object);
    t.class("Trigger", actor);
    let mut on_actor = None;
    for class in [actor, pawn, nav] {
        let spec = FunctionSpec::new(NATIVE_FLAGS, "GetBoundingCylinder");
        let params = [
            Param { flags: CPF_OUT, ..Param::new("Float", "CollisionRadius") },
            Param { flags: CPF_OUT, ..Param::new("Float", "CollisionHeight") },
        ];
        let f = t.function(class, "GetBoundingCylinder", &params, None, &[], &spec, |_| None);
        on_actor.get_or_insert(f.function);
    }
    // `ToyAskCylinder`: GetBoundingCylinder(ToyRadius, ToyHeight), from script.
    let native = on_actor.unwrap();
    t.function(actor, "ToyAskCylinder", &[], None, &[], &FunctionSpec::new(0x1 | 0x2 | 0x20000, "ToyAskCylinder"), |_| {
        let mut a = ow_package::testkit::script::Asm::new();
        a.stmt().final_function(native, |a| {
            a.instance(toy_radius);
            a.instance(toy_height);
        });
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.p.build()
}

/// `WillowGame.upk` (invented): `WillowPickup` under `Actor` and `WillowPlayerPawn` under `Pawn`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let cylinder = t.p.add_import("Core", "Class", engine, "CylinderComponent");
    t.class("WillowPickup", actor);
    t.class("BulletListenerComponent", cylinder);
    t.class("WillowPlayerPawn", pawn);
    t.p.build()
}

/// `GD_ToyCylinders.upk` (invented): `HolderCyl`, `HolderLight` and `HolderActor`, three `Actor` exports whose
/// `CollisionComponent` names, in turn, a cylinder component, a light environment component (not a primitive) and
/// another actor. Reading a loaded holder gives the named export as an unloaded store handle.
fn definitions() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let cylinder = t.p.add_import("Core", "Class", engine, "CylinderComponent");
    let light = t.p.add_import("Core", "Class", engine, "DynamicLightEnvironmentComponent");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let empty = |t: &ScriptToy| {
        let mut body = ow_package::testkit::ints(&[0]);
        body.extend_from_slice(&t.none);
        body
    };
    let mut body = ow_package::testkit::ints(&[0]);
    body.extend(t.p.tag("CollisionRadius", "FloatProperty", &12.0f32.to_le_bytes()));
    body.extend_from_slice(&t.none);
    let cyl = t.p.add_export(cylinder, "Cyl", body);
    let b = empty(&t);
    let lamp = t.p.add_export(light, "Lamp", b);
    let b = empty(&t);
    let peer = t.p.add_export(actor, "Peer", b);
    for (name, target) in [("HolderCyl", cyl), ("HolderLight", lamp), ("HolderActor", peer)] {
        let mut holder = ow_package::testkit::ints(&[0]);
        holder.extend(t.p.tag("CollisionComponent", "ObjectProperty", &ow_package::testkit::ints(&[target])));
        holder.extend_from_slice(&t.none);
        t.p.add_export(actor, name, holder);
    }
    t.p.build()
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

fn fx_mode(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-world-gbc-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w, d) = (engine(), willow(), definitions());
    let files: [(&str, &[u8]); 3] = [("Engine.upk", &e), ("WillowGame.upk", &w), ("GD_ToyCylinders.upk", &d)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    if mode != ParityMode::Reference {
        register(&mut vm).unwrap();
    }
    // The class code the tests use is loaded here, by the fixture, so no answer below is the first to load a class.
    for class in [
        "Engine.Actor",
        "Engine.Pawn",
        "Engine.NavigationPoint",
        "Engine.Trigger",
        "Engine.CylinderComponent",
        "Engine.SkeletalMeshComponent",
        "Engine.PrimitiveComponent",
        "Engine.StaticMeshComponent",
        "Engine.ParticleSystemComponent",
        "Engine.ToyCylinderKind",
        "WillowGame.BulletListenerComponent",
        "Engine.DynamicLightEnvironmentComponent",
        "WillowGame.WillowPickup",
        "WillowGame.WillowPlayerPawn",
    ] {
        vm.find_class(class).unwrap();
    }
    Fx { vm, dir }
}

fn fx() -> Fx {
    fx_mode(ParityMode::Off)
}

impl Fx {
    fn live(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap()
    }

    fn template(&mut self, class: &str) -> ObjRef {
        self.vm.defs.class_default(&mut self.vm.space, class).unwrap()
    }

    /// A run-time object of `class` with `outer` as its outer, as VM `new` makes one.
    fn newed(&mut self, class: &str, outer: ObjRef) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.space.alloc(c, "ToyNewed", Some(outer), Origin::Runtime).unwrap()
    }

    /// #518's three counters (committed heap writes, generator draws, store load transitions) and the name table's
    /// length (lane FOLLOWUP-627-COUNTERS, the assertion #627's decision promised once #595 was on main).
    fn counters(&self) -> (u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
    }

    /// Everything a refusal or an answer must leave as it was (review r1 of #642, item 3): the heap fingerprint
    /// ([`Fx::fingerprint`]: every object's class, slots and store state, and the name table's size), the RNG streams'
    /// whole state, and [`Fx::counters`].
    fn snapshot(&self) -> (String, ow_core::rand::GameRng, (u64, u64, u64, usize)) {
        (self.fingerprint(), self.vm.rng.clone(), self.counters())
    }

    /// Runs `f` and asserts [`Fx::snapshot`] is unchanged (review r1 of #642, item 1: the direct host calls and the
    /// script errors).
    fn same<T>(&mut self, label: &str, f: impl FnOnce(&mut Fx) -> T) -> T {
        let before = self.snapshot();
        let r = f(self);
        assert!(self.snapshot() == before, "{label}: the heap, the RNG, #518's counters and the name table unchanged");
        r
    }

    fn export(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.space.alloc(c, "ToyExport", None, Origin::Export { package: 0, index: 1 }).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn cylinder(&mut self, r: f32, h: f32) -> ObjRef {
        let c = self.live("Engine.CylinderComponent");
        self.set(c, "CollisionRadius", Value::Float(r));
        self.set(c, "CollisionHeight", Value::Float(h));
        c
    }

    /// A plain `PrimitiveComponent` entry (its per-component check passes; lane IMPL-SKIP-FLAG).
    fn primitive(&mut self, attached: bool, collides: bool) -> ObjRef {
        let c = self.live("Engine.PrimitiveComponent");
        self.set(c, "bAttached", Value::Bool(attached));
        self.set(c, "CollideActors", Value::Bool(collides));
        c
    }

    fn components(&mut self, o: ObjRef, list: Vec<Option<ObjRef>>) {
        self.set(o, "Components", Value::array(list.into_iter().map(Value::Object).collect()));
    }

    /// The export the named loaded holder's `CollisionComponent` names, an unloaded handle.
    fn lazy(&mut self, holder: &str) -> ObjRef {
        let path = ow_core::ObjectPath::parse(&format!("GD_ToyCylinders.{holder}")).unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let class = self.vm.space.heap.get(holder).unwrap().class;
        let p = self.vm.space.prop(class, "CollisionComponent").unwrap();
        let Value::Object(Some(target)) = self.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
            panic!("the holder names its export")
        };
        assert!(self.vm.defs.is_unloaded(&self.vm.space, target), "a genuinely unloaded handle");
        target
    }

    /// Calls the native declared on `class` from the host, with both out arguments at -1: (radius, height) as the
    /// caller sees them afterwards.
    fn call(&mut self, class: &str, this: Option<ObjRef>) -> Result<(Value, Value), VmError> {
        let f = self.vm.find_function(&format!("Engine.{class}.GetBoundingCylinder")).unwrap();
        let r = self.vm.call(f, this, vec![Value::Float(-1.0), Value::Float(-1.0)])?;
        Ok((r.outs[0].clone(), r.outs[1].clone()))
    }

    /// Every heap object's class, slots and store state, and the name table's size: equal before and after means no
    /// slot was written, no object made, nothing loaded, no load tried (a failed load stays `is_unloaded` but turns
    /// `is_failed`, review r1 of #627) and no name interned.
    fn fingerprint(&self) -> String {
        let (space, defs) = (&self.vm.space, &self.vm.defs);
        let heap = &space.heap;
        let objects: String = heap
            .handles()
            .into_iter()
            .map(|h| {
                let o = heap.get(h).unwrap();
                format!("{h:?} {:?} {:?} {} {}\n", o.class, o.slots, defs.is_unloaded(space, h), defs.is_failed(space, h))
            })
            .collect();
        format!("{objects}names {}\n", space.names.len())
    }

    /// The answer for `o` must be a `NotYet` of `missing` with `what` on `record`, and leave the heap and store as
    /// they were.
    fn refuses(&self, o: ObjRef, missing: Missing, what: &str, record: &str) {
        let before = self.snapshot();
        let totals = counts(&self.vm);
        match answer(&self.vm, o) {
            Err(VmError::NotYet(n)) => {
                assert_eq!((n.missing, n.what.as_ref(), n.record), (missing, what, record));
            }
            other => panic!("expected the NotYet {what:?}, got {other:?}"),
        }
        assert!(self.snapshot() == before, "a refusal writes, makes, loads, draws and interns nothing");
        assert_eq!(counts(&self.vm), totals, "#518's three counters and the name table");
    }

    /// A scene layer on the world (the attach record's home), once.
    fn scene(&mut self) {
        crate::actors::add_empty_scene(&mut self.vm).unwrap();
    }

    /// `c` attached to `owner` as the port's attach paths leave it: `Owner` set, and the record published.
    fn attach(&mut self, owner: ObjRef, c: ObjRef) {
        self.scene();
        self.set(c, "Owner", Value::Object(Some(owner)));
        crate::actors::publish_attach(&mut self.vm, c, owner).unwrap();
    }

    /// The answer for `o` must be `want`, with #518's counters unchanged (the answer itself writes nothing).
    fn answers(&self, o: ObjRef, want: (f32, f32), label: &str) {
        let before = self.snapshot();
        assert_eq!(answer(&self.vm, o).unwrap(), want, "{label}");
        assert!(self.snapshot() == before, "{label}: the heap, the RNG, #518's counters and the name table unchanged");
    }
}

/// #518's three counters (heap writes, draws, load transitions) and the name table's length.
fn counts(vm: &Vm) -> (u64, u64, u64, usize) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
}

fn l() -> Missing {
    Missing::LayerAbsent(BOUNDS_LAYER)
}

fn script_error<T: std::fmt::Debug>(r: Result<T, VmError>, naming: &str) {
    match r {
        Err(VmError::Script { function, message }) => {
            assert!(function.contains(naming) || message.contains(naming), "{function}: {message}")
        }
        other => panic!("expected a script error naming {naming}, got {other:?}"),
    }
}

/// The three declared functions carry the three keys, the bound native writes both outs, and Reference binds none.
#[test]
fn the_three_keys_are_bound_and_write_the_outs() {
    let mut x = fx();
    for (class, key) in ["Actor", "Pawn", "NavigationPoint"].into_iter().zip(KEYS) {
        let f = x.vm.find_function(&format!("Engine.{class}.GetBoundingCylinder")).unwrap();
        assert_eq!(x.vm.function(f).unwrap().native_key.as_deref(), Some(key));
    }
    let nav = x.live("Engine.NavigationPoint");
    let c = x.cylinder(40.0, 80.0);
    x.set(nav, "CylinderComponent", Value::Object(Some(c)));
    let outs = x.same("a host call", |x| x.call("NavigationPoint", Some(nav)).unwrap());
    assert_eq!(outs, (Value::Float(40.0), Value::Float(80.0)));
    let pickup = x.live("WillowGame.WillowPickup");
    let outs = x.same("a host call", |x| x.call("Actor", Some(pickup)).unwrap());
    assert_eq!(outs, (Value::Float(0.0), Value::Float(0.0)), "no components");
    let r = fx_mode(ParityMode::Reference);
    for key in KEYS {
        assert!(!r.vm.native_keys().any(|k| k == key), "Reference binds no {key}");
    }
}

/// A live pickup with a colliding cylinder entry (the slice's case): L at the first primitive entry, and neither out
/// argument is written.
#[test]
fn a_live_pickup_s_cylinder_entry_is_l() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let light = x.live("Engine.DynamicLightEnvironmentComponent");
    let cyl = x.cylinder(18.0, 9.0);
    x.set(cyl, "CollideActors", Value::Bool(true));
    let mesh = x.primitive(false, false);
    x.components(pickup, vec![None, Some(cyl), Some(light), Some(mesh)]);
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
    // From the host the outs are copies, so the check is that the call fails before writing: no out comes back.
    let r = x.same("a host refusal", |x| x.call("Actor", Some(pickup)));
    assert!(matches!(r, Err(VmError::NotYet(ref n)) if n.what == REFUSAL_L));
}

/// A class-default actor: unattached (or attached but not colliding) loaded entries give 0 and 0; an attached
/// colliding entry reads the skip flag: set, it is skipped; clear, it reaches the bounds step, refusal L (lane
/// IMPL-SKIP-FLAG; S before).
#[test]
fn a_template_reads_its_entries_then_the_skip_flag() {
    let mut x = fx();
    let t = x.template("Engine.Actor");
    let (unattached, quiet, colliding) = (x.primitive(false, true), x.primitive(true, false), x.primitive(true, true));
    x.components(t, vec![None, Some(unattached), Some(quiet)]);
    x.answers(t, (0.0, 0.0), "");
    x.components(t, vec![Some(unattached), Some(colliding)]);
    x.refuses(t, l(), REFUSAL_L, RECORD);
    x.set(colliding, "BulletListener", Value::Bool(true));
    x.answers(t, (0.0, 0.0), "a bullet listener is skipped");
}

/// The pawn rule: an export-origin pawn with a separate cylinder is T; a live one gets the actor default, not the
/// cylinder; a class-default one gets the cylinder's values.
#[test]
fn the_pawn_rule_by_origin() {
    let mut x = fx();
    for class in ["Engine.Pawn", "WillowGame.WillowPlayerPawn"] {
        let export = x.export(class);
        let c = x.cylinder(22.0, 44.0);
        x.set(export, "CylinderComponent", Value::Object(Some(c)));
        x.refuses(export, Missing::NotRead, REFUSAL_T, RECORD);

        let live = x.live(class);
        x.set(live, "CylinderComponent", Value::Object(Some(c)));
        x.answers(live, (0.0, 0.0), "{class}: the actor default, no components");
        x.components(live, vec![Some(c)]);
        // Lane IMPL-COMPONENT-BOUNDS: on a live receiver a non-colliding entry is skipped before attachment is asked.
        assert_eq!(answer(&x.vm, live).unwrap(), (0.0, 0.0), "{class}: the cylinder does not collide");
        x.set(c, "CollideActors", Value::Bool(true));
        x.refuses(live, l(), REFUSAL_L, RECORD);
        x.set(c, "CollideActors", Value::Bool(false));

        let t = x.template(class);
        x.set(t, "CylinderComponent", Value::Object(Some(c)));
        x.answers(t, (22.0, 44.0), "{class}");
        // The same component as the collision component: the actor default.
        x.set(t, "CollisionComponent", Value::Object(Some(c)));
        x.answers(t, (0.0, 0.0), "{class}");
    }
}

/// A navigation point answers its cylinder, live or template; without one, the actor default.
#[test]
fn a_navigation_point_answers_its_cylinder() {
    let mut x = fx();
    let live = x.live("Engine.NavigationPoint");
    let t = x.template("Engine.NavigationPoint");
    for nav in [live, t] {
        let c = x.cylinder(50.0, 75.0);
        x.set(nav, "CylinderComponent", Value::Object(Some(c)));
        x.answers(nav, (50.0, 75.0), "");
    }
    let nav = x.live("Engine.NavigationPoint");
    x.answers(nav, (0.0, 0.0), "");
    let colliding = x.primitive(true, true);
    x.components(nav, vec![Some(colliding)]);
    x.refuses(nav, l(), REFUSAL_L, RECORD);
}

/// Lazy handles are never loaded: a primitive entry is N and stays unloaded; a non-primitive entry is skipped with
/// nothing loaded; a lazy cylinder component and a lazy receiver are N.
#[test]
fn lazy_handles_are_n_or_skipped_and_never_loaded() {
    let mut x = fx();
    let (cyl, lamp, peer) = (x.lazy("HolderCyl"), x.lazy("HolderLight"), x.lazy("HolderActor"));
    let pickup = x.live("WillowGame.WillowPickup");
    x.components(pickup, vec![Some(cyl)]);
    x.refuses(pickup, Missing::NotRead, REFUSAL_N, RECORD);
    x.components(pickup, vec![Some(lamp)]);
    // `answers` checks the snapshot, so the non-primitive lazy entry was skipped, not loaded.
    x.answers(pickup, (0.0, 0.0), "the non-primitive lazy entry");
    let t = x.template("Engine.Pawn");
    x.set(t, "CylinderComponent", Value::Object(Some(cyl)));
    x.refuses(t, Missing::NotRead, REFUSAL_N, RECORD);
    x.refuses(peer, Missing::NotRead, REFUSAL_N, RECORD);
    for h in [cyl, lamp, peer] {
        assert!(x.vm.defs.is_unloaded(&x.vm.space, h), "still unloaded");
    }
}

/// A class outside step 1 is C, main's text and record, even a subclass of `Actor`.
#[test]
fn a_class_whose_body_was_not_read_is_c() {
    let mut x = fx();
    let trigger = x.live("Engine.Trigger");
    x.refuses(trigger, Missing::NotRead, "the bounding-cylinder rule of Engine.Trigger", EXTENT_RECORD);
}

/// Malformed data is a script error naming the field: a cylinder component without `CollisionRadius` (a pawn
/// template's `CylinderComponent` that is a mesh: no class test is made on it), and a call with no receiver. A
/// `Components` or `CollideActors` of the wrong kind cannot be built on these classes: the heap refuses the write.
#[test]
fn malformed_data_is_a_script_error() {
    let mut x = fx();
    let t = x.template("Engine.Pawn");
    let mesh = x.primitive(false, false);
    x.set(t, "CylinderComponent", Value::Object(Some(mesh)));
    script_error(x.same("a script error", |x| answer(&x.vm, t)), "CollisionRadius");
    script_error(x.same("a host script error", |x| x.call("Actor", None)), NATIVE);
}

/// Template-or-live is asked only where it is used (note section 4, step 2): an export-origin `WillowPickup` with no
/// primitive entry gives 0 and 0, not T; an export navigation point with a cylinder gives the cylinder's values; an
/// export actor's primitive entry reaches step 5's `bAttached` test, where T is raised.
#[test]
fn an_export_receiver_is_t_only_where_its_origin_is_used() {
    let mut x = fx();
    let pickup = x.export("WillowGame.WillowPickup");
    let light = x.live("Engine.DynamicLightEnvironmentComponent");
    x.components(pickup, vec![None, Some(light)]);
    x.answers(pickup, (0.0, 0.0), "");
    let nav = x.export("Engine.NavigationPoint");
    let c = x.cylinder(31.0, 62.0);
    x.set(nav, "CylinderComponent", Value::Object(Some(c)));
    x.answers(nav, (31.0, 62.0), "");
    let actor = x.export("Engine.Actor");
    let mesh = x.primitive(false, false);
    x.components(actor, vec![Some(mesh)]);
    x.refuses(actor, Missing::NotRead, REFUSAL_T, RECORD);
}

/// A loaded stand-in with no slots among the `Components` is N, like a lazy handle: its fields cannot be read.
#[test]
fn a_stand_in_with_no_slots_is_n() {
    let mut x = fx();
    let class = x.vm.find_class("Engine.SkeletalMeshComponent").unwrap();
    let stand_in = x.vm.space.alloc_stand_in(class, "ToyStandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    let pickup = x.live("WillowGame.WillowPickup");
    x.components(pickup, vec![Some(stand_in)]);
    x.refuses(pickup, Missing::NotRead, REFUSAL_N, RECORD);
}

/// A recheck of #595: a run-time object is a template when an object on its outer chain is one, as VM `new` can make
/// it. A pawn `new`ed into a class default gets the pawn rule's cylinder; an actor `new`ed into one reads its entries'
/// `bAttached` (no refusal L), then meets the skip flag; under a `new`ed run-time outer the walk goes on up.
#[test]
fn a_run_time_object_with_a_template_outer_is_a_template() {
    let mut x = fx();
    for class in ["Engine.Pawn", "WillowGame.WillowPlayerPawn"] {
        let t = x.template(class);
        let pawn = x.newed(class, t);
        let c = x.cylinder(22.0, 44.0);
        x.set(pawn, "CylinderComponent", Value::Object(Some(c)));
        x.answers(pawn, (22.0, 44.0), class);
        let deeper = x.newed(class, pawn);
        x.set(deeper, "CylinderComponent", Value::Object(Some(c)));
        x.answers(deeper, (22.0, 44.0), "two outers up");
    }
    let t = x.template("Engine.Actor");
    let actor = x.newed("Engine.Actor", t);
    let (unattached, colliding) = (x.primitive(false, true), x.primitive(true, true));
    x.components(actor, vec![Some(unattached)]);
    x.answers(actor, (0.0, 0.0), "an unattached entry is skipped");
    x.components(actor, vec![Some(unattached), Some(colliding)]);
    x.refuses(actor, l(), REFUSAL_L, RECORD);
}

/// The rest of the walk: a run-time object under only run-time outers is live (refusal L at its colliding entry), and
/// one with an export on its chain is T, because the export could be an archetype.
#[test]
fn a_run_time_outer_chain_is_live_and_an_export_on_it_is_t() {
    let mut x = fx();
    let root = x.live("Engine.Actor");
    let actor = x.newed("Engine.Actor", root);
    let mesh = x.primitive(true, true);
    x.components(actor, vec![Some(mesh)]);
    x.refuses(actor, l(), REFUSAL_L, RECORD);
    let export = x.export("Engine.Actor");
    let under_export = x.newed("Engine.Actor", export);
    x.components(under_export, vec![Some(mesh)]);
    x.refuses(under_export, Missing::NotRead, REFUSAL_T, RECORD);
    let pawn = x.newed("Engine.Pawn", export);
    let c = x.cylinder(22.0, 44.0);
    x.set(pawn, "CylinderComponent", Value::Object(Some(c)));
    x.refuses(pawn, Missing::NotRead, REFUSAL_T, RECORD);
}

/// An exported non-pawn whose `Components` holds a resident primitive is T, with nothing written or loaded and #518's
/// counters unchanged; the same export with only a non-primitive entry never asks and answers the actor default.
#[test]
fn an_exported_non_pawn_with_a_resident_primitive_is_t() {
    let mut x = fx();
    for class in ["Engine.Actor", "WillowGame.WillowPickup"] {
        let actor = x.export(class);
        let light = x.live("Engine.DynamicLightEnvironmentComponent");
        x.components(actor, vec![Some(light)]);
        x.answers(actor, (0.0, 0.0), class);
        for (attached, collides) in [(false, false), (true, true)] {
            let mesh = x.primitive(attached, collides);
            x.components(actor, vec![Some(light), Some(mesh)]);
            x.refuses(actor, Missing::NotRead, REFUSAL_T, RECORD);
        }
    }
}

/// Review r1 of #627: a really lazy outer, a store handle not yet loaded, on a run-time object's chain is refusal T,
/// and the walk neither loads it nor tries to (it stays unloaded and not failed; [`Fx::refuses`] also checks every
/// object and the name table).
#[test]
fn a_lazy_outer_is_t_and_never_loaded_or_tried() {
    let mut x = fx();
    let outer = x.lazy("HolderActor");
    let actor = x.newed("Engine.Actor", outer);
    let mesh = x.primitive(true, true);
    x.components(actor, vec![Some(mesh)]);
    x.refuses(actor, Missing::NotRead, REFUSAL_T, RECORD);
    let pawn = x.newed("Engine.Pawn", outer);
    let c = x.cylinder(22.0, 44.0);
    x.set(pawn, "CylinderComponent", Value::Object(Some(c)));
    x.refuses(pawn, Missing::NotRead, REFUSAL_T, RECORD);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, outer), "still unloaded");
    assert!(!x.vm.defs.is_failed(&x.vm.space, outer), "no load was tried");
}

// ---- Lane IMPL-COMPONENT-BOUNDS (NATIVE_COMPONENT_BOUNDS.md section 7 tests, #639 r1) ----------------------------

/// Tests 1 and 2 (#639), with the skip flag read (lane IMPL-SKIP-FLAG): a recorded colliding cylinder whose
/// `BulletListener` is set is skipped; clear, it reaches the bounds step and answers its box.
#[test]
fn a_recorded_colliding_entry_reads_the_skip_flag() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let cyl = x.cylinder(64.0, 20.0);
    x.set(cyl, "CollideActors", Value::Bool(true));
    x.attach(pickup, cyl);
    x.components(pickup, vec![Some(cyl)]);
    x.set(cyl, "BulletListener", Value::Bool(true));
    x.answers(pickup, (0.0, 0.0), "a bullet listener is skipped");
    x.set(cyl, "BulletListener", Value::Bool(false));
    x.answers(pickup, cylinder_box(64.0, 20.0), "the cylinder's box");
}

/// The answer for one cylinder entry of radius `r` and half height `h` at the owner's origin.
fn cylinder_box(r: f32, h: f32) -> (f32, f32) {
    let [radius, _, height] = crate::placement::box_cylinder(&[([0.0; 3], [r, r, h])]);
    (radius, height)
}

/// Tests 3 and 4: a colliding entry not in the record is L; non-colliding entries, recorded or not, are skipped, and
/// an actor with only those (and non-primitives) answers 0 and 0.
#[test]
fn an_unrecorded_colliding_entry_is_l_and_non_colliding_ones_are_skipped() {
    let mut x = fx();
    x.scene();
    let pickup = x.live("WillowGame.WillowPickup");
    let quiet = x.cylinder(10.0, 10.0);
    let recorded_quiet = x.cylinder(11.0, 11.0);
    x.attach(pickup, recorded_quiet);
    let light = x.live("Engine.DynamicLightEnvironmentComponent");
    x.components(pickup, vec![Some(quiet), Some(recorded_quiet), Some(light)]);
    assert_eq!(answer(&x.vm, pickup).unwrap(), (0.0, 0.0));
    let colliding = x.primitive(false, true);
    x.components(pickup, vec![Some(quiet), Some(colliding)]);
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
}

/// Test 5, order: a non-colliding recorded entry, then an unrecorded colliding one, is L; a recorded colliding entry
/// first is counted, then the later (lazy) entry is N, never loaded.
#[test]
fn the_first_reached_entry_decides() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let quiet = x.cylinder(10.0, 10.0);
    x.attach(pickup, quiet);
    let colliding = x.primitive(false, true);
    x.components(pickup, vec![Some(quiet), Some(colliding)]);
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
    let first = x.cylinder(12.0, 12.0);
    x.set(first, "CollideActors", Value::Bool(true));
    x.attach(pickup, first);
    let lazy = x.lazy("HolderCyl");
    x.components(pickup, vec![Some(first), Some(lazy)]);
    x.refuses(pickup, Missing::NotRead, REFUSAL_N, RECORD);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the later entry was never loaded");
}

/// Tests 6 and 7: the record's lifecycle and the scoped lookup. A record naming another owner, a component whose
/// `Owner` changed, a destroyed component and a destroyed owner each count as "attachment unknown" (L); destroying
/// the owner through the world drops its entries.
#[test]
fn stale_or_foreign_records_are_attachment_unknown() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let other = x.live("WillowGame.WillowPickup");
    let cyl = x.cylinder(64.0, 20.0);
    x.set(cyl, "CollideActors", Value::Bool(true));
    x.components(pickup, vec![Some(cyl)]);
    // Recorded with another owner.
    x.attach(other, cyl);
    x.set(cyl, "Owner", Value::Object(Some(pickup)));
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
    // Recorded with this owner, then its Owner changed.
    x.attach(pickup, cyl);
    x.answers(pickup, cylinder_box(64.0, 20.0), "attached and current");
    x.set(cyl, "Owner", Value::Object(Some(other)));
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
    // Review r1 of #643, item 1: attached to A, its Owner changed to B, then back to A with no new attach. The
    // record is stale (its Owner write count moved), so attachment is unknown: L, not S.
    x.attach(pickup, cyl);
    x.answers(pickup, cylinder_box(64.0, 20.0), "attached and current");
    x.set(cyl, "Owner", Value::Object(Some(other)));
    x.set(cyl, "Owner", Value::Object(Some(pickup)));
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
    // A new attach to A makes it current again.
    x.attach(pickup, cyl);
    x.answers(pickup, cylinder_box(64.0, 20.0), "attached and current");
    // A destroyed component (the world's destroy path is tested in `world::tests`).
    x.vm.space.heap.destroy(cyl).unwrap();
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
    // No scene layer: no record at all.
    let mut y = fx();
    let p = y.live("WillowGame.WillowPickup");
    let c = y.cylinder(5.0, 5.0);
    y.set(c, "CollideActors", Value::Bool(true));
    y.set(c, "Owner", Value::Object(Some(p)));
    y.components(p, vec![Some(c)]);
    assert!(!crate::actors::attached_to(&y.vm, c, p).unwrap());
    y.refuses(p, l(), REFUSAL_L, RECORD);
}

/// Test 8: the record leaves `bAttached` as it was (the attach paths' own tests check their writes).
#[test]
fn the_record_does_not_write_b_attached() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let cyl = x.cylinder(64.0, 20.0);
    x.attach(pickup, cyl);
    let class = x.vm.space.heap.get(cyl).unwrap().class;
    let p = x.vm.space.prop(class, "bAttached").unwrap();
    assert_eq!(x.vm.space.read(cyl, PropAddr::scalar(p)).unwrap(), &Value::Bool(false));
}

// ---- Lane IMPL-SKIP-FLAG (NATIVE_COMPONENT_SKIP_FLAG.md section 5 tests) -------------------------------------------

/// Note tests 1 and 2: `BulletListener` set skips an entry on a live and on a template receiver, with no refusal;
/// clear, the entry goes on to the bounds step (the live entry's own box rule, L on a template).
#[test]
fn the_skip_flag_skips_a_set_entry_and_passes_a_clear_one() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let plain = x.primitive(false, true);
    x.attach(pickup, plain);
    x.components(pickup, vec![Some(plain)]);
    x.set(plain, "BulletListener", Value::Bool(true));
    x.answers(pickup, (0.0, 0.0), "live: skipped");
    x.set(plain, "BulletListener", Value::Bool(false));
    x.refuses(pickup, Missing::NotRead, "Actor.GetBoundingCylinder: the bounds update of Engine.PrimitiveComponent", BOUNDS_RECORD);
    let t = x.template("Engine.Actor");
    let entry = x.primitive(true, true);
    x.components(t, vec![Some(entry)]);
    x.set(entry, "BulletListener", Value::Bool(true));
    x.answers(t, (0.0, 0.0), "template: skipped");
    x.set(entry, "BulletListener", Value::Bool(false));
    x.refuses(t, l(), REFUSAL_L, RECORD);
}

/// Note test 3: a `BulletListener` that holds another kind is a `VmError::Script` naming it, with nothing changed.
#[test]
fn a_skip_flag_of_another_kind_is_a_script_error() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let plain = x.primitive(false, true);
    x.attach(pickup, plain);
    x.components(pickup, vec![Some(plain)]);
    let class = x.vm.space.heap.get(plain).unwrap().class;
    let p = x.vm.space.prop(class, SKIP_FIELD).unwrap();
    *x.vm.space.slot_mut(plain, PropAddr::scalar(p)).unwrap() = Value::Int(1);
    let r = x.same("a data error", |x| answer(&x.vm, pickup));
    script_error(r, SKIP_FIELD);
}

/// Note test 4: the per-component check, asked last, on a live recorded colliding entry with the bit clear and on a
/// template's attached colliding entry: a `CylinderComponent` passes and gives its box (L on a template); a
/// `BulletListenerComponent` passes, then meets B.7's `NotRead`; skeletal, static-mesh, particle and a toy cylinder
/// subclass are the check's `NotRead`. With the bit set, or not colliding, none of them is asked.
#[test]
fn the_component_check_passes_the_read_classes_and_refuses_the_rest() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let listener = x.live("WillowGame.BulletListenerComponent");
    x.set(listener, "CollideActors", Value::Bool(true));
    x.attach(pickup, listener);
    x.components(pickup, vec![Some(listener)]);
    x.refuses(
        pickup,
        Missing::NotRead,
        "Actor.GetBoundingCylinder: the bounds update of WillowGame.BulletListenerComponent",
        BOUNDS_RECORD,
    );
    let t = x.template("Engine.Actor");
    for class in [
        "Engine.SkeletalMeshComponent",
        "Engine.StaticMeshComponent",
        "Engine.ParticleSystemComponent",
        "Engine.ToyCylinderKind",
    ] {
        let c = x.live(class);
        x.attach(pickup, c);
        x.set(c, "bAttached", Value::Bool(true));
        let want = format!("{COMPONENT_CHECK_OF} {class} (not read)");
        // Not colliding: skipped before the check.
        x.components(pickup, vec![Some(c)]);
        x.components(t, vec![Some(c)]);
        x.answers(pickup, (0.0, 0.0), "live, not colliding");
        x.answers(t, (0.0, 0.0), "template, not colliding");
        x.set(c, "CollideActors", Value::Bool(true));
        x.refuses(pickup, Missing::NotRead, &want, SKIP_RECORD);
        x.refuses(t, Missing::NotRead, &want, SKIP_RECORD);
        // The bit set: skipped before the check.
        x.set(c, "BulletListener", Value::Bool(true));
        x.answers(pickup, (0.0, 0.0), "live, a bullet listener");
        x.answers(t, (0.0, 0.0), "template, a bullet listener");
    }
}

/// Note test 5: an unrecorded colliding entry is still L before its skip flag is read, set or not.
#[test]
fn an_unrecorded_entry_is_l_before_the_skip_flag() {
    let mut x = fx();
    x.scene();
    let pickup = x.live("WillowGame.WillowPickup");
    let cyl = x.cylinder(64.0, 20.0);
    x.set(cyl, "CollideActors", Value::Bool(true));
    x.components(pickup, vec![Some(cyl)]);
    for bit in [false, true] {
        x.set(cyl, "BulletListener", Value::Bool(bit));
        x.refuses(pickup, l(), REFUSAL_L, RECORD);
    }
    // Review r1 of #660, item 3a: a malformed flag is never read before L.
    let class = x.vm.space.heap.get(cyl).unwrap().class;
    let p = x.vm.space.prop(class, SKIP_FIELD).unwrap();
    *x.vm.space.slot_mut(cyl, PropAddr::scalar(p)).unwrap() = Value::Int(7);
    x.refuses(pickup, l(), REFUSAL_L, RECORD);
}

/// Review r1 of #660, item 2: a `BulletListener` declared as an int is a `VmError::Script` naming it on the live and
/// template paths, even when its slot holds a bool.
#[test]
fn a_non_bool_skip_declaration_is_a_script_error() {
    let mut x = fx();
    x.vm.find_class("Engine.ToyIntSkipComponent").unwrap();
    let c = x.live("Engine.ToyIntSkipComponent");
    x.set(c, "CollideActors", Value::Bool(true));
    x.set(c, "bAttached", Value::Bool(true));
    let class = x.vm.space.heap.get(c).unwrap().class;
    let p = x.vm.space.prop(class, SKIP_FIELD).unwrap();
    assert_eq!(x.vm.space.classes.decl(p).map(|d| d.kind.clone()), Some(ow_object::layout::PropKind::Int));
    *x.vm.space.slot_mut(c, PropAddr::scalar(p)).unwrap() = Value::Bool(true);
    let pickup = x.live("WillowGame.WillowPickup");
    x.attach(pickup, c);
    x.components(pickup, vec![Some(c)]);
    let r = x.same("live", |x| answer(&x.vm, pickup));
    script_error(r, SKIP_FIELD);
    let t = x.template("Engine.Actor");
    x.components(t, vec![Some(c)]);
    let r = x.same("template", |x| answer(&x.vm, t));
    script_error(r, SKIP_FIELD);
}

/// Review r1 of #660, item 1: from script, a writable radius and a height destination of another kind. Both outs are
/// checked before either write, so the call fails with the radius property unchanged and nothing counted.
#[test]
fn a_bad_height_destination_leaves_the_radius_unwritten() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let cyl = x.cylinder(64.0, 20.0);
    x.set(cyl, "CollideActors", Value::Bool(true));
    x.attach(pickup, cyl);
    x.components(pickup, vec![Some(cyl)]);
    x.set(pickup, "ToyRadius", Value::Float(-3.0));
    x.answers(pickup, cylinder_box(64.0, 20.0), "the answer the outs would carry");
    let f = x.vm.find_function("Engine.Actor.ToyAskCylinder").unwrap();
    let r = x.same("a refused out write", |x| x.vm.call(f, Some(pickup), vec![]).map(|r| r.ret));
    assert!(!matches!(r, Ok(_) | Err(VmError::NotYet(_))), "the height write is refused: {r:?}");
    let class = x.vm.space.heap.get(pickup).unwrap().class;
    let p = x.vm.space.prop(class, "ToyRadius").unwrap();
    assert_eq!(x.vm.space.read(pickup, PropAddr::scalar(p)).unwrap(), &Value::Float(-3.0), "the radius is unchanged");
}

/// Lane UNDECLARED-COMPONENT-CLASS: on a live receiver, a `Components` entry whose class is an engine-native stand-in
/// is `NotRead` naming that class, before its flags are read and with nothing changed ([`Fx::same`]); a package
/// non-primitive is skipped as before.
#[test]
fn a_native_stand_in_component_class_is_not_read() {
    let mut x = fx();
    x.scene();
    let pickup = x.live("WillowGame.WillowPickup");
    // As the definition store makes one for a class with no package export: defined under Core.Object, native.
    // The fixture's root class (its `Object`), as the store would use `Core.Object`.
    let mut object = x.vm.find_class("Engine.PrimitiveComponent").unwrap();
    while let Some(up) = x.vm.space.classes.get_layout(object).and_then(|l| l.super_class()) {
        object = up;
    }
    let native = x.vm.space.classes.define("Test.NativeOnlyComponent", Some(object), vec![]).unwrap();
    x.vm.space.classes.mark_native(native).unwrap();
    let entry = x.vm.space.alloc_stand_in(native, "Native", Origin::Resource { package: 0, index: -1 }).unwrap();
    let light = x.live("Engine.DynamicLightEnvironmentComponent");
    x.components(pickup, vec![Some(light), Some(entry)]);
    let err = x.same("native stand-in", |x| answer(&x.vm, pickup).unwrap_err());
    let VmError::NotYet(n) = err else { panic!("{err:?}") };
    assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD));
    assert!(n.what.contains("engine-native stand-in class") && n.what.contains("Test.NativeOnlyComponent"), "{}", n.what);
    x.components(pickup, vec![Some(light)]);
    assert_eq!(answer(&x.vm, pickup).unwrap(), (0.0, 0.0), "a defined non-primitive is skipped");
}
