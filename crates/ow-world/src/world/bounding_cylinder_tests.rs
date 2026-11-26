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
        "ByteProperty",
        "Enum",
    ]);
    let object = t.class("Object", 0);
    let vector = ow_package::testkit::reflect::toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    // Lane IMPL-SKELMESH-BOUNDS-SOURCE: a rotator, for the components' and actors' `Rotation`.
    let rotator = ow_package::testkit::reflect::toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }
    let primitive = t.class("PrimitiveComponent", object);
    // The fields the skeletal bounds rule reads on every primitive (NATIVE_COMPONENT_BOUNDS.md section 9).
    for b in ["HiddenGame", "bIgnoreOwnerHidden", "bCastHiddenShadow"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    t.prop("Byte", primitive, "DetailMode", 0, 0);
    t.prop("Float", primitive, "Scale", 0, 0);
    t.prop("Struct", primitive, "Scale3D", 0, vector);
    t.prop("Float", primitive, "BoundsScale", 0, 0);
    t.prop("Struct", primitive, "Rotation", 0, rotator);
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
    let skeletal = t.class("SkeletalMeshComponent", primitive);
    t.prop("Int", skeletal, "FramesPhysicsAsleep", 0, 0);
    for b in ["bSkipUpdateSkelPose", "bComponentUseFixedSkelBounds", "bUseBoundsFromParentAnimComponent", "bHasValidBodies"] {
        t.prop("Bool", skeletal, b, 0, 0);
    }
    for o in ["SkeletalMesh", "ParentAnimComponent", "PhysicsAsset"] {
        t.prop("Object", skeletal, o, 0, object);
    }
    t.prop("Struct", skeletal, "RootBoneTranslation", 0, vector);
    let bases = t.p.next_export();
    t.prop("Array", skeletal, "SpaceBases", 0, bases + 1);
    t.prop("Int", bases, "SpaceBases", 0, 0);
    t.class("ToySkeletalKind", skeletal);
    let mesh = t.class("SkeletalMesh", object);
    let kdops = t.p.next_export();
    t.prop("Array", mesh, "PerPolyCollisionBones", 0, kdops + 1);
    t.prop("Int", kdops, "PerPolyCollisionBones", 0, 0);
    t.class("PhysicsAsset", object);
    // Lane IMPL-SKIP-FLAG: two more primitive classes whose per-component check was not read, and a toy one.
    t.class("StaticMeshComponent", primitive);
    t.class("ParticleSystemComponent", primitive);
    t.class("ToyCylinderKind", cylinder);
    // Lane IMPL-COMPONENT-CHECK: a script-only primitive subclass.
    t.class("ToyPrimitiveKind", primitive);
    // Review r1 of #660, item 2: a primitive whose BulletListener is declared as an int.
    let int_skip = t.class("ToyIntSkipComponent", primitive);
    t.prop("Int", int_skip, "BulletListener", 0, 0);
    t.class("DynamicLightEnvironmentComponent", object);
    let actor = t.class("Actor", object);
    t.prop("Struct", actor, "Location", 0, vector);
    // The owner fields the skeletal bounds rule reads: `Physics` (an `EPhysics` byte), `bHidden`, the draw scales and
    // `Rotation`.
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend_from_slice(&t.none);
    let values = ["PHYS_None", "PHYS_Walking", "PHYS_Falling", "PHYS_RigidBody"];
    payload.extend(ow_package::testkit::ints(&[0, values.len() as i32]));
    for v in values {
        let bytes = t.p.fname(v, 0);
        payload.extend_from_slice(&bytes);
    }
    let mut e = ow_package::testkit::Export::new(t.import("Enum"), "EPhysics", payload);
    e.outer = actor;
    let physics = t.p.add_export_with(e);
    t.prop("Byte", actor, "Physics", 0, physics);
    t.prop("Bool", actor, "bHidden", 0, 0);
    t.prop("Float", actor, "DrawScale", 0, 0);
    t.prop("Struct", actor, "DrawScale3D", 0, vector);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    // Review of #764, item 4: the owner's pre-pivot, part of the simplest transform.
    t.prop("Struct", actor, "PrePivot", 0, vector);
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
        "Engine.ToySkeletalKind",
        "Engine.SkeletalMesh",
        "Engine.PhysicsAsset",
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
        (vm.space.heap.committed_writes(), vm.rng.draws(), (vm.defs.load_transitions() + vm.class_code_loads()), vm.space.names.len())
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
    (vm.space.heap.committed_writes(), vm.rng.draws(), (vm.defs.load_transitions() + vm.class_code_loads()), vm.space.names.len())
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

/// IMPL-COMPONENT-CHECK (NATIVE_COMPONENT_SKIP_FLAG.md section 6.1, tests 1, 2, 4 and 7): on a live, recorded,
/// attached, colliding entry with the bit clear, every primitive class passes the per-component check and meets its
/// box rule. The static mesh component gets the static layer's `LayerAbsent`,
/// and the particle component and the toy script-only subclasses of `PrimitiveComponent` and `CylinderComponent` "the
/// bounds update of <class>". On a template the same entry meets refusal L, not the check. Not colliding, or with the bit
/// set, none is asked. No answer carries the retired "the component check of" text.
#[test]
fn every_primitive_entry_passes_the_component_check() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let t = x.template("Engine.Actor");
    let update = |class: &str| format!("Actor.GetBoundingCylinder: the bounds update of {class}");
    let cases: Vec<(&str, Missing, String)> = vec![
        ("Engine.StaticMeshComponent", Missing::LayerAbsent(STATIC_LAYER), STATIC_BOUNDS.to_string()),
        ("Engine.ParticleSystemComponent", Missing::NotRead, update("Engine.ParticleSystemComponent")),
        ("Engine.ToyPrimitiveKind", Missing::NotRead, update("Engine.ToyPrimitiveKind")),
        ("Engine.ToyCylinderKind", Missing::NotRead, update("Engine.ToyCylinderKind")),
    ];
    for (class, missing, what) in cases {
        assert!(!what.contains("the component check of"), "{class}");
        let c = x.live(class);
        x.attach(pickup, c);
        x.set(c, "bAttached", Value::Bool(true));
        x.components(pickup, vec![Some(c)]);
        x.components(t, vec![Some(c)]);
        // Not colliding: skipped before the check.
        x.answers(pickup, (0.0, 0.0), "live, not colliding");
        x.answers(t, (0.0, 0.0), "template, not colliding");
        x.set(c, "CollideActors", Value::Bool(true));
        x.refuses(pickup, missing, &what, BOUNDS_RECORD);
        x.refuses(t, l(), REFUSAL_L, RECORD);
        // The bit set: skipped before the check.
        x.set(c, "BulletListener", Value::Bool(true));
        x.answers(pickup, (0.0, 0.0), "live, a bullet listener");
        x.answers(t, (0.0, 0.0), "template, a bullet listener");
    }
}

/// Section 6.1 test 3, unchanged: a `BulletListenerComponent` (the bit clear) passes the check and meets B.7's
/// `NotRead`; a `CylinderComponent` gives its box (the earlier tests).
#[test]
fn a_bullet_listener_component_meets_its_bounds_rule() {
    let mut x = fx();
    let pickup = x.live("WillowGame.BulletListenerComponent");
    let owner = x.live("WillowGame.WillowPickup");
    x.set(pickup, "CollideActors", Value::Bool(true));
    x.attach(owner, pickup);
    x.components(owner, vec![Some(pickup)]);
    x.refuses(
        owner,
        Missing::NotRead,
        "Actor.GetBoundingCylinder: the bounds update of WillowGame.BulletListenerComponent",
        BOUNDS_RECORD,
    );
}

/// Section 6.1 test 6: an entry that is not a primitive component is skipped at the class test, live and template.
#[test]
fn a_non_primitive_entry_is_skipped_at_the_class_test() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let t = x.template("Engine.Actor");
    let lamp = x.live("Engine.DynamicLightEnvironmentComponent");
    x.components(pickup, vec![Some(lamp)]);
    x.components(t, vec![Some(lamp)]);
    x.answers(pickup, (0.0, 0.0), "live");
    x.answers(t, (0.0, 0.0), "template");
}

/// The check itself, its guard (IMPL-COMPONENT-CHECK): it passes for every primitive class above, writing nothing,
/// and an object whose chain never reaches `Engine.PrimitiveComponent` (no path asks one) is `CHAIN_NOT_READ`.
#[test]
fn the_component_check_passes_a_primitive_chain_and_guards_the_rest() {
    let mut x = fx();
    for class in [
        "Engine.PrimitiveComponent",
        "Engine.CylinderComponent",
        "Engine.SkeletalMeshComponent",
        "Engine.ToyPrimitiveKind",
        "WillowGame.BulletListenerComponent",
    ] {
        let c = x.live(class);
        x.same(class, |x| component_check(&x.vm.space, c)).unwrap();
    }
    let lamp = x.live("Engine.DynamicLightEnvironmentComponent");
    match x.same("the guard", |x| component_check(&x.vm.space, lamp)) {
        Err(VmError::NotYet(n)) => {
            let want = format!("{CHAIN_NOT_READ} Engine.DynamicLightEnvironmentComponent");
            assert_eq!((n.missing, n.what.as_ref(), n.record), (Missing::NotRead, want.as_str(), SKIP_RECORD));
        }
        other => panic!("expected the guard's NotRead, got {other:?}"),
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

/// Review r1 of #681: the class test with no `PrimitiveComponent` id (the caller found none in the table) still
/// refuses a chain through a native stand-in, and answers no for a package class.
#[test]
fn the_class_test_refuses_a_stand_in_with_no_primitive_id() {
    let mut x = fx();
    let mut object = x.vm.find_class("Engine.PrimitiveComponent").unwrap();
    while let Some(up) = x.vm.space.classes.get_layout(object).and_then(|l| l.super_class()) {
        object = up;
    }
    let native = x.vm.space.classes.define("Test.NativeOnlyComponent", Some(object), vec![]).unwrap();
    x.vm.space.classes.mark_native(native).unwrap();
    let light = x.vm.find_class("Engine.DynamicLightEnvironmentComponent").unwrap();
    let space = &x.vm.space;
    assert_eq!(primitive_class(space, light, None, "Actor.GetBoundingCylinder", RECORD).unwrap(), false);
    let Err(VmError::NotYet(n)) = primitive_class(space, native, None, "Actor.GetBoundingCylinder", RECORD) else {
        panic!("a stand-in must be refused")
    };
    assert_eq!(n.missing, Missing::NotRead);
}

/// Lane UNIFY-STANDIN-RULE: the `Components` walks' class test and the live collision walks' class test are one rule.
/// For the same classes they agree: a primitive is yes / `Primitive`, a native stand-in is `NotRead` / `Uncertain`
/// naming it, a package non-primitive is no / `Other`, and so is a class that is only declared (no object of one can
/// exist, but the class id can be asked about).
#[test]
fn both_class_tests_give_the_same_answer() {
    use crate::trace_filter::{PartClass, part_class};
    let mut x = fx();
    let primitive = x.vm.find_class("Engine.PrimitiveComponent").unwrap();
    let mut root = primitive;
    while let Some(up) = x.vm.space.classes.get_layout(root).and_then(|l| l.super_class()) {
        root = up;
    }
    let native = x.vm.space.classes.define("Test.NativeOnlyComponent", Some(root), vec![]).unwrap();
    x.vm.space.classes.mark_native(native).unwrap();
    let declared = x.vm.space.classes.declare("Test.DeclaredOnlyComponent").unwrap();
    let light = x.vm.find_class("Engine.DynamicLightEnvironmentComponent").unwrap();
    let space = &x.vm.space;
    for p in [Some(primitive), None] {
        for class in [primitive, native, declared, light] {
            let walk = primitive_class(space, class, p, "Actor.GetBoundingCylinder", RECORD);
            match (part_class(space, class, p), walk) {
                (PartClass::Primitive, Ok(true)) | (PartClass::Other, Ok(false)) => {}
                (PartClass::Uncertain(path), Err(VmError::NotYet(n))) => {
                    assert_eq!(n.missing, Missing::NotRead);
                    assert!(n.what.ends_with(&path), "{}", n.what);
                }
                (a, b) => panic!("class {class:?} with {p:?}: {} vs {b:?}", match a {
                    PartClass::Primitive => "Primitive",
                    PartClass::Other => "Other",
                    PartClass::Uncertain(_) => "Uncertain",
                }),
            }
        }
    }
    assert!(matches!(part_class(space, declared, Some(primitive)), PartClass::Other));
    assert!(matches!(part_class(space, native, None), PartClass::Uncertain(_)));
}

// ---------------------------------------------------------------------------------------------------------------
// Lane IMPL-SKELMESH-BOUNDS-SOURCE: a live skeletal mesh entry's bounds (NATIVE_COMPONENT_BOUNDS.md section 9.6,
// tests of section 9.7). Every value is invented.

use crate::world::skeletal::{MeshBounds, ReferenceSkeleton, SkeletalMeshes, SkeletonSource};
use crate::world::skeletal_bounds::{
    BODIES, BODIES_LAYER, DETAIL_MODE, HIDDEN_NO_MESH, PARENT_BOUNDS, PARENT_ROOT, PER_BONE, SKELETAL_BOUNDS,
    SKIP_POSE, SLEEPING, STORED_LAYER, TEMPLATE,
};

/// A source whose only answer is the mesh bounds it was given, for any mesh export.
struct Bounds(MeshBounds);

impl SkeletonSource for Bounds {
    fn skeleton(&self, _: &str, _: &str, _: &str) -> Result<Option<ReferenceSkeleton>, String> {
        Ok(None)
    }
    fn mesh_bounds_in(&self, _: &ow_package::package::Package, _: i32) -> Result<Option<MeshBounds>, String> {
        Ok(Some(self.0))
    }
}

/// The bounds the stub source answers: origin (1, 2, 3), extent (4, 5, 6), two bones.
const TOY_BOUNDS: MeshBounds = MeshBounds { origin: [1.0, 2.0, 3.0], extent: [4.0, 5.0, 6.0], bones: 2 };

impl Fx {
    /// Member `m` of struct field `n` of `o` set to `v`.
    fn member(&mut self, o: ObjRef, n: &str, m: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, n).unwrap();
        let mut s = self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone();
        let ty = s.as_struct().unwrap().ty;
        let id = self.vm.space.names.find(m).unwrap();
        let (i, _) = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
        s.struct_mut().unwrap().fields[usize::try_from(i).unwrap()] = v;
        self.set(o, n, s);
    }

    fn vector3(&mut self, o: ObjRef, n: &str, v: [f32; 3]) {
        for (m, f) in ["X", "Y", "Z"].into_iter().zip(v) {
            self.member(o, n, m, Value::Float(f));
        }
    }

    /// A live pickup with one entry: a live, recorded, colliding `SkeletalMeshComponent` (class defaults otherwise).
    fn skeletal(&mut self) -> (ObjRef, ObjRef) {
        let pickup = self.live("WillowGame.WillowPickup");
        let c = self.live("Engine.SkeletalMeshComponent");
        self.attach(pickup, c);
        self.set(c, "CollideActors", Value::Bool(true));
        self.components(pickup, vec![Some(c)]);
        (pickup, c)
    }

    /// Unit scales on the component and its owner (the uniform, unit combined scale), and `BoundsScale`.
    fn unit_scales(&mut self, pickup: ObjRef, c: ObjRef, bounds_scale: f32) {
        self.set(c, "Scale", Value::Float(1.0));
        self.vector3(c, "Scale3D", [1.0; 3]);
        self.set(c, "BoundsScale", Value::Float(bounds_scale));
        self.set(pickup, "DrawScale", Value::Float(1.0));
        self.vector3(pickup, "DrawScale3D", [1.0; 3]);
    }

    /// A mesh the store can place in a package export (the accessor's precondition), resident.
    fn mesh_export(&mut self) -> ObjRef {
        self.export("Engine.SkeletalMesh")
    }

    fn bounds_source(&mut self, b: MeshBounds) {
        SkeletalMeshes::install(&mut self.vm, Box::new(Bounds(b))).unwrap();
    }
}

fn not_read() -> Missing {
    Missing::NotRead
}

fn stored() -> Missing {
    Missing::LayerAbsent(STORED_LAYER)
}

/// 9.7 test 1, under the hub's revised ruling on 9.5 (review r1 of #707): class defaults and no mesh reach source e,
/// which is not answered: the template's `NotRead`, alone and after the pickup's cylinder has given its box. The
/// per-component check passes on the way.
#[test]
fn a_skeletal_entry_with_no_mesh_refuses_on_its_template() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD);
    x.set(c, "BoundsScale", Value::Float(2.0));
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD);
    let cyl = x.cylinder(64.0, 20.0);
    x.attach(pickup, cyl);
    x.set(cyl, "CollideActors", Value::Bool(true));
    x.components(pickup, vec![Some(cyl), Some(c)]);
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD);
}

/// NATIVE_LINK67_PICKUP_GUN_INIT.md section 7, test 5 (lane IMPL-LINK67-PICKUP-GUN-INIT): the pickup's copied component
/// with an export mesh. No mesh layer: the stored-bounds `LayerAbsent`. With a layer and the gun's `Scale` 0.92, a
/// uniform scale, the scaled box (the hub's ruling, lane SCALED-BOUNDS-TRANSFORM; before it, the world transform's
/// `NotRead`); `Scale` 1: the box (origin plus the root offset, the extent times `BoundsScale`).
#[test]
fn the_pickup_gun_s_copied_component_takes_source_f() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    let mesh = x.mesh_export();
    x.set(c, "SkeletalMesh", Value::Object(Some(mesh)));
    x.refuses(pickup, stored(), SKELETAL_BOUNDS, BOUNDS_RECORD);
    x.bounds_source(TOY_BOUNDS);
    x.unit_scales(pickup, c, 1.0);
    let s = 0.92f32;
    x.set(c, "Scale", Value::Float(s));
    let scaled = box_cylinder(&[([s * 1.0, s * 2.0, s * 3.0], [4.0 * s, 5.0 * s, 6.0 * s])]);
    x.answers(pickup, (scaled[0], scaled[2]), "the gun's uniform 0.92");
    x.set(c, "Scale", Value::Float(1.0));
    x.vector3(c, "RootBoneTranslation", [0.5, 0.0, 0.0]);
    let want = box_cylinder(&[([1.5, 2.0, 3.0], [4.0, 5.0, 6.0])]);
    x.answers(pickup, (want[0], want[2]), "the stored box at unit scale");
}

/// The hub's ruling (lane SCALED-BOUNDS-TRANSFORM): a uniform, positive combined scale is answered, the box scaled by
/// it (the extents, so the cylinder's radius and height); the factor may come from the owner's `DrawScale`. A
/// non-uniform scale (the component's `Scale3D` or the owner's `DrawScale3D`), a zero, a negative or a NaN scale is the
/// world transform's `NotRead`.
#[test]
fn a_uniform_positive_scale_scales_the_stored_box() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    let mesh = x.mesh_export();
    x.set(c, "SkeletalMesh", Value::Object(Some(mesh)));
    x.bounds_source(TOY_BOUNDS);
    x.unit_scales(pickup, c, 1.0);
    let unit = box_cylinder(&[([1.0, 2.0, 3.0], [4.0, 5.0, 6.0])]);
    x.answers(pickup, (unit[0], unit[2]), "unit scale");
    let s = 0.92f32;
    x.set(c, "Scale", Value::Float(s));
    let scaled = box_cylinder(&[([s * 1.0, s * 2.0, s * 3.0], [4.0 * s, 5.0 * s, 6.0 * s])]);
    x.answers(pickup, (scaled[0], scaled[2]), "0.92");
    assert!(scaled[0] < unit[0] && scaled[2] < unit[2], "the radius and the height shrink");
    // The factor through the owner: DrawScale 0.5 with the component at 1.
    x.set(c, "Scale", Value::Float(1.0));
    x.set(pickup, "DrawScale", Value::Float(0.5));
    let half = box_cylinder(&[([0.5, 1.0, 1.5], [2.0, 2.5, 3.0])]);
    x.answers(pickup, (half[0], half[2]), "the owner's DrawScale");
    x.set(pickup, "DrawScale", Value::Float(1.0));
    // Non-uniform: the component's Scale3D, then the owner's DrawScale3D.
    x.vector3(c, "Scale3D", [1.0, 1.0, 0.5]);
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.vector3(c, "Scale3D", [1.0; 3]);
    x.vector3(pickup, "DrawScale3D", [2.0, 1.0, 1.0]);
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.vector3(pickup, "DrawScale3D", [1.0; 3]);
    // Zero, negative and NaN.
    for bad in [0.0f32, -1.0, f32::NAN] {
        x.set(c, "Scale", Value::Float(bad));
        x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    }
    x.set(c, "Scale", Value::Float(1.0));
    // Review of #764, item 2: "uniform" is exactly equal, with no tolerance. One ulp above 1 on one axis is refused.
    let near = 1.000_000_1f32;
    assert_ne!(near, 1.0, "the axis differs from 1 as an f32");
    x.vector3(c, "Scale3D", [1.0, 1.0, near]);
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.vector3(c, "Scale3D", [1.0; 3]);
    x.answers(pickup, (unit[0], unit[2]), "unit scale again");
}

/// Review of #764, item 4: a non-zero owner `PrePivot` leaves the simplest transform, so the stored box is the world
/// transform's `NotRead` (UNVERIFIED: the port's reading that a pre-pivot moves the owner's frame); zero answers again.
#[test]
fn a_non_zero_pre_pivot_is_the_world_transform() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    let mesh = x.mesh_export();
    x.set(c, "SkeletalMesh", Value::Object(Some(mesh)));
    x.bounds_source(TOY_BOUNDS);
    x.unit_scales(pickup, c, 1.0);
    let unit = box_cylinder(&[([1.0, 2.0, 3.0], [4.0, 5.0, 6.0])]);
    x.answers(pickup, (unit[0], unit[2]), "zero PrePivot");
    for pivot in [[0.0, 0.0, 1.0], [-0.5, 0.0, 0.0]] {
        x.vector3(pickup, "PrePivot", pivot);
        x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    }
    x.vector3(pickup, "PrePivot", [0.0; 3]);
    x.answers(pickup, (unit[0], unit[2]), "zero PrePivot again");
}

/// 9.7 tests 2 and 9a: a mesh takes source f. With no accessor answer, the stored-bounds `LayerAbsent`. With one, the
/// simplest transform gives `Location` + the mesh origin + `RootBoneTranslation` and the extent × `BoundsScale`, each
/// times a uniform scale (lane SCALED-BOUNDS-TRANSFORM); a non-zero owner `Rotation`, a non-uniform scale or a component
/// `Translation` is the world transform's `NotRead`.
#[test]
fn a_skeletal_entry_with_a_mesh_takes_the_stored_bounds() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    let mesh = x.mesh_export();
    x.set(c, "SkeletalMesh", Value::Object(Some(mesh)));
    x.refuses(pickup, stored(), SKELETAL_BOUNDS, BOUNDS_RECORD);
    x.bounds_source(TOY_BOUNDS);
    x.unit_scales(pickup, c, 2.0);
    x.vector3(c, "Scale3D", [1.0, 1.0, 2.0]);
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.vector3(c, "Scale3D", [1.0; 3]);
    x.vector3(pickup, "Location", [10.0, 20.0, 30.0]);
    x.vector3(c, "RootBoneTranslation", [0.5, 0.0, 0.0]);
    let want = box_cylinder(&[([11.5, 22.0, 33.0], [8.0, 10.0, 12.0])]);
    x.answers(pickup, (want[0], want[2]), "the simplest transform");
    x.member(pickup, "Rotation", "Yaw", Value::Int(16384));
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.member(pickup, "Rotation", "Yaw", Value::Int(0));
    // A uniform 2: the box scaled about the owner (origin and root offset times 2, the extent times 2 × BoundsScale).
    x.set(c, "Scale", Value::Float(2.0));
    let doubled = box_cylinder(&[([13.0, 24.0, 36.0], [16.0, 20.0, 24.0])]);
    x.answers(pickup, (doubled[0], doubled[2]), "a uniform scale of 2");
    x.set(c, "Scale", Value::Float(1.0));
    // A translated component (lane IMPL-BOUNDING-CYLINDER-AND-AFTER, NATIVE_BOUNDING_CYLINDER_AND_AFTER.md section 2):
    // under zero rotations and unit scale the box moves by the translation; scaled or rotated, the NotRead.
    x.vector3(c, "Translation", [0.0, 0.0, -86.0]);
    let moved = box_cylinder(&[([11.5, 22.0, -53.0], [8.0, 10.0, 12.0])]);
    x.answers(pickup, (moved[0], moved[2]), "translated, unit scale");
    x.set(c, "Scale", Value::Float(2.0));
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.set(c, "Scale", Value::Float(1.0));
    x.member(c, "Rotation", "Pitch", Value::Int(100));
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.member(c, "Rotation", "Pitch", Value::Int(0));
    x.vector3(c, "Translation", [0.0; 3]);
    // A non-empty SpaceBases with a mesh that lists per-poly collision bones: the per-bone step, not read.
    x.set(c, "SpaceBases", Value::array(vec![Value::Int(0), Value::Int(0)]));
    x.set(mesh, "PerPolyCollisionBones", Value::array(vec![Value::Int(0)]));
    x.refuses(pickup, not_read(), PER_BONE, BOUNDS_RECORD);
    x.set(mesh, "PerPolyCollisionBones", Value::array(vec![]));
    x.answers(pickup, (want[0], want[2]), "no per-poly collision bones");
}

/// 9.7 tests 3, 3a, 4 and 5: visibility and source a. `HiddenGame` (or an owner's `bHidden` without
/// `bIgnoreOwnerHidden`, or `bComponentUseFixedSkelBounds`) takes source a: with no mesh the "no mesh" `NotRead`, with
/// one the stored-bounds `LayerAbsent`. `bCastHiddenShadow` makes a hidden component visible again.
#[test]
fn a_hidden_or_fixed_skeletal_entry_takes_source_a() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    x.set(c, "HiddenGame", Value::Bool(true));
    x.refuses(pickup, not_read(), HIDDEN_NO_MESH, BOUNDS_RECORD);
    // 3a: a parent with a fixed-bounds mesh does not come first.
    let parent = x.live("Engine.SkeletalMeshComponent");
    let parent_mesh = x.mesh_export();
    x.set(parent, "SkeletalMesh", Value::Object(Some(parent_mesh)));
    x.set(parent, "bComponentUseFixedSkelBounds", Value::Bool(true));
    x.set(c, "ParentAnimComponent", Value::Object(Some(parent)));
    x.refuses(pickup, not_read(), HIDDEN_NO_MESH, BOUNDS_RECORD);
    x.set(c, "ParentAnimComponent", Value::Object(None));
    let mesh = x.mesh_export();
    x.set(c, "SkeletalMesh", Value::Object(Some(mesh)));
    x.refuses(pickup, stored(), SKELETAL_BOUNDS, BOUNDS_RECORD);
    x.set(c, "SkeletalMesh", Value::Object(None));
    x.set(c, "bCastHiddenShadow", Value::Bool(true));
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD); // "a hidden shadow caster is visible: source e", source e
    x.set(c, "bCastHiddenShadow", Value::Bool(false));
    x.set(c, "HiddenGame", Value::Bool(false));
    // 4: the owner hidden.
    x.set(pickup, "bHidden", Value::Bool(true));
    x.refuses(pickup, not_read(), HIDDEN_NO_MESH, BOUNDS_RECORD);
    x.set(c, "bIgnoreOwnerHidden", Value::Bool(true));
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD); // "the owner's bHidden ignored", source e
    x.set(pickup, "bHidden", Value::Bool(false));
    // 5: fixed bounds while visible.
    x.set(c, "bComponentUseFixedSkelBounds", Value::Bool(true));
    x.refuses(pickup, not_read(), HIDDEN_NO_MESH, BOUNDS_RECORD);
}

/// 9.7 tests 6, 7 and 8: the early refusals, before any source. `DetailMode` 1; a sleeping rigid-body owner (5 frames
/// or another `Physics` goes on); `bSkipUpdateSkelPose`.
#[test]
fn the_early_returns_refuse_before_the_source() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    x.set(c, "DetailMode", Value::Byte(1));
    x.refuses(pickup, not_read(), DETAIL_MODE, BOUNDS_RECORD);
    x.set(c, "DetailMode", Value::Byte(0));
    x.set(pickup, "Physics", Value::Byte(3));
    x.set(c, "FramesPhysicsAsleep", Value::Int(6));
    x.refuses(pickup, not_read(), SLEEPING, BOUNDS_RECORD);
    x.set(c, "FramesPhysicsAsleep", Value::Int(5));
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD); // "five frames asleep goes on", source e
    x.set(c, "FramesPhysicsAsleep", Value::Int(6));
    x.set(pickup, "Physics", Value::Byte(2));
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD); // "another Physics goes on", source e
    x.set(c, "bSkipUpdateSkelPose", Value::Bool(true));
    x.refuses(pickup, not_read(), SKIP_POSE, BOUNDS_RECORD);
}

/// 9.7 tests 9 and 10: the physics sources. Its own `PhysicsAsset` with `bHasValidBodies` clear is skipped; set, with
/// a uniform scale, a mesh and a matching bone count, the bodies' `LayerAbsent` (its text naming the bones layer).
/// With `SpaceBases` empty the count is false (skipped to f); with it non-empty and no accessor answer, the
/// stored-bounds `LayerAbsent`. A parent with a fixed-bounds mesh is source b; a parent with a `PhysicsAsset` and
/// `bUseBoundsFromParentAnimComponent` is the parent's stored `Bounds` (`BOUNDS_LAYER`); a lazy parent is refusal N.
#[test]
fn the_physics_and_parent_sources() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    let asset = x.live("Engine.PhysicsAsset");
    x.set(c, "PhysicsAsset", Value::Object(Some(asset)));
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD); // "bHasValidBodies clear: skipped to e", source e
    x.set(c, "bHasValidBodies", Value::Bool(true));
    x.refuses(pickup, not_read(), TEMPLATE, BOUNDS_RECORD); // "no mesh: cannot use the bones", source e
    let mesh = x.mesh_export();
    x.set(c, "SkeletalMesh", Value::Object(Some(mesh)));
    x.unit_scales(pickup, c, 1.0);
    // SpaceBases empty: the count is false, so c is skipped to f (no accessor: the stored-bounds layer).
    x.refuses(pickup, stored(), SKELETAL_BOUNDS, BOUNDS_RECORD);
    // SpaceBases not empty and no accessor answer: the count cannot be decided.
    x.set(c, "SpaceBases", Value::array(vec![Value::Int(0), Value::Int(0)]));
    x.refuses(pickup, stored(), SKELETAL_BOUNDS, BOUNDS_RECORD);
    x.bounds_source(TOY_BOUNDS);
    assert!(BODIES.contains(BONES_LAYER), "the bodies' refusal names the bones layer");
    x.refuses(pickup, Missing::LayerAbsent(BODIES_LAYER), BODIES, BOUNDS_RECORD);
    // A count that does not match: c is skipped, and source f answers through the simplest transform.
    x.set(c, "SpaceBases", Value::array(vec![Value::Int(0)]));
    let want = box_cylinder(&[([1.0, 2.0, 3.0], [4.0, 5.0, 6.0])]);
    x.answers(pickup, (want[0], want[2]), "a bone count that differs");
    x.set(c, "PhysicsAsset", Value::Object(None));
    // Source b.
    let parent = x.live("Engine.SkeletalMeshComponent");
    let parent_mesh = x.mesh_export();
    x.set(parent, "SkeletalMesh", Value::Object(Some(parent_mesh)));
    x.set(parent, "bComponentUseFixedSkelBounds", Value::Bool(true));
    x.set(c, "ParentAnimComponent", Value::Object(Some(parent)));
    x.refuses(pickup, not_read(), PARENT_ROOT, BOUNDS_RECORD);
    // Review r1 of #707: PARENT_ROOT only in the simplest transform; otherwise the world transform's NotRead.
    x.vector3(c, "Scale3D", [1.0, 2.0, 1.0]);
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.vector3(c, "Scale3D", [1.0; 3]);
    // Review of #764, item 3: a uniform non-unit scale is the simplest transform since the scaled-bounds ruling, so
    // source b answers PARENT_ROOT there, no longer the world transform's NotRead.
    x.set(c, "Scale", Value::Float(2.0));
    x.refuses(pickup, not_read(), PARENT_ROOT, BOUNDS_RECORD);
    x.set(c, "Scale", Value::Float(1.0));
    // Review of #764, item 4: a non-zero owner PrePivot on source b is the world transform's NotRead.
    x.vector3(pickup, "PrePivot", [0.0, 0.0, 1.0]);
    x.refuses(pickup, not_read(), WORLD_TRANSFORM, BOUNDS_RECORD);
    x.vector3(pickup, "PrePivot", [0.0; 3]);
    // Source d.
    x.set(parent, "bComponentUseFixedSkelBounds", Value::Bool(false));
    let parent_asset = x.live("Engine.PhysicsAsset");
    x.set(parent, "PhysicsAsset", Value::Object(Some(parent_asset)));
    x.set(c, "bUseBoundsFromParentAnimComponent", Value::Bool(true));
    x.refuses(pickup, l(), PARENT_BOUNDS, BOUNDS_RECORD);
    x.set(c, "bUseBoundsFromParentAnimComponent", Value::Bool(false));
    x.refuses(pickup, Missing::LayerAbsent(BODIES_LAYER), BODIES, BOUNDS_RECORD);
    // Review r1 of #707: a parent without the fixed-bounds flag never needs its mesh resident, so a lazy parent mesh
    // there is no refusal N.
    let lazy_mesh = x.lazy("HolderActor");
    x.set(parent, "SkeletalMesh", Value::Object(Some(lazy_mesh)));
    x.set(parent, "PhysicsAsset", Value::Object(None));
    x.answers(pickup, (want[0], want[2]), "the parent's mesh is not required: source f answers");
    x.set(parent, "bComponentUseFixedSkelBounds", Value::Bool(true));
    x.refuses(pickup, Missing::NotRead, REFUSAL_N, RECORD);
    // A lazy parent: refusal N.
    let lazy = x.lazy("HolderCyl");
    x.set(c, "ParentAnimComponent", Value::Object(Some(lazy)));
    x.refuses(pickup, Missing::NotRead, REFUSAL_N, RECORD);
}

/// 9.7 tests 10 (no accessor) and 11: source b with no accessor answer is the stored-bounds `LayerAbsent`; a field of
/// the wrong kind is a `VmError::Script` naming it, with nothing written.
#[test]
fn source_b_without_an_accessor_and_wrong_kinds() {
    let mut x = fx();
    let (pickup, c) = x.skeletal();
    let parent = x.live("Engine.SkeletalMeshComponent");
    let parent_mesh = x.mesh_export();
    x.set(parent, "SkeletalMesh", Value::Object(Some(parent_mesh)));
    x.set(parent, "bComponentUseFixedSkelBounds", Value::Bool(true));
    x.set(c, "ParentAnimComponent", Value::Object(Some(parent)));
    x.refuses(pickup, stored(), SKELETAL_BOUNDS, BOUNDS_RECORD);
    x.set(c, "ParentAnimComponent", Value::Object(None));
    for (o, name) in [(c, "DetailMode"), (pickup, "bHidden")] {
        let class = x.vm.space.heap.get(o).unwrap().class;
        let p = x.vm.space.prop(class, name).unwrap();
        let old = x.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone();
        *x.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = Value::Int(1);
        let r = x.same("a data error", |x| answer(&x.vm, pickup));
        script_error(r, name);
        *x.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = old;
    }
}

/// 9.7 test 12: a `SkeletalMeshComponent` subclass is not the exact class: "the bounds update of <class>".
#[test]
fn a_skeletal_subclass_keeps_the_unread_bounds_update() {
    let mut x = fx();
    let pickup = x.live("WillowGame.WillowPickup");
    let c = x.live("Engine.ToySkeletalKind");
    x.attach(pickup, c);
    x.set(c, "CollideActors", Value::Bool(true));
    x.components(pickup, vec![Some(c)]);
    x.refuses(pickup, Missing::NotRead, "Actor.GetBoundingCylinder: the bounds update of Engine.ToySkeletalKind", BOUNDS_RECORD);
}

/// NATIVE_BOUNDING_CYLINDER_AND_AFTER.md section 9, test 1 (lane IMPL-BOUNDING-CYLINDER-AND-AFTER): a cylinder entry
/// with a `Translation`, zero rotations and unit scale is centred at `Location` + `Translation`; scaled or rotated it
/// is the world transform's `NotRead`; untranslated it is the owner's `Location` as before. Reads only.
#[test]
fn a_translated_cylinder_under_unit_scale_moves_by_its_translation() {
    let mut x = fx();
    let (pickup, _) = x.skeletal();
    let c = x.cylinder(42.0, 80.0);
    x.set(c, "Owner", Value::Object(Some(pickup)));
    x.unit_scales(pickup, c, 1.0);
    x.vector3(pickup, "Location", [10.0, 20.0, 30.0]);
    assert_eq!(entry_box(&x.vm, pickup, c).unwrap(), ([10.0, 20.0, 30.0], [42.0, 42.0, 80.0]));
    x.vector3(c, "Translation", [0.0, 0.0, -86.0]);
    let before = x.vm.space.heap.committed_writes();
    assert_eq!(entry_box(&x.vm, pickup, c).unwrap(), ([10.0, 20.0, -56.0], [42.0, 42.0, 80.0]));
    assert_eq!(x.vm.space.heap.committed_writes(), before, "reads only");
    for (scale, pitch) in [(2.0, 0), (1.0, 100)] {
        x.set(c, "Scale", Value::Float(scale));
        x.member(c, "Rotation", "Pitch", Value::Int(pitch));
        match entry_box(&x.vm, pickup, c) {
            Err(VmError::NotYet(n)) => assert_eq!(n.what.as_ref(), WORLD_TRANSFORM, "{n}"),
            other => panic!("scale {scale} pitch {pitch}: {other:?}"),
        }
    }
    // The #799 review's conditions: a non-zero PrePivot, an Absolute flag, or another Owner is the NotRead too.
    x.set(c, "Scale", Value::Float(1.0));
    x.member(c, "Rotation", "Pitch", Value::Int(0));
    x.vector3(pickup, "PrePivot", [0.0, 0.0, 5.0]);
    assert!(matches!(entry_box(&x.vm, pickup, c), Err(VmError::NotYet(n)) if n.what.as_ref() == WORLD_TRANSFORM), "PrePivot");
    x.vector3(pickup, "PrePivot", [0.0; 3]);
    x.set(c, "AbsoluteTranslation", Value::Bool(true));
    assert!(matches!(entry_box(&x.vm, pickup, c), Err(VmError::NotYet(n)) if n.what.as_ref() == WORLD_TRANSFORM), "Absolute");
    x.set(c, "AbsoluteTranslation", Value::Bool(false));
    x.set(c, "Owner", Value::Object(None));
    assert!(matches!(entry_box(&x.vm, pickup, c), Err(VmError::NotYet(n)) if n.what.as_ref() == WORLD_TRANSFORM), "Owner");
}
