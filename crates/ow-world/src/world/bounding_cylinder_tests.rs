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
    ]);
    let object = t.class("Object", 0);
    let primitive = t.class("PrimitiveComponent", object);
    t.prop("Bool", primitive, "bAttached", 0, 0);
    t.prop("Bool", primitive, "CollideActors", 0, 0);
    let cylinder = t.class("CylinderComponent", primitive);
    t.prop("Float", cylinder, "CollisionRadius", 0, 0);
    t.prop("Float", cylinder, "CollisionHeight", 0, 0);
    t.class("SkeletalMeshComponent", primitive);
    t.class("DynamicLightEnvironmentComponent", object);
    let actor = t.class("Actor", object);
    t.prop("Object", actor, "CollisionComponent", 0, object);
    let array = t.p.next_export();
    t.prop("Array", actor, "Components", 0, array + 1);
    t.prop("Object", array, "Components", 0, object);
    let pawn = t.class("Pawn", actor);
    t.prop("Object", pawn, "CylinderComponent", 0, object);
    let nav = t.class("NavigationPoint", actor);
    t.prop("Object", nav, "CylinderComponent", 0, object);
    t.class("Trigger", actor);
    for class in [actor, pawn, nav] {
        let spec = FunctionSpec::new(NATIVE_FLAGS, "GetBoundingCylinder");
        let params = [
            Param { flags: CPF_OUT, ..Param::new("Float", "CollisionRadius") },
            Param { flags: CPF_OUT, ..Param::new("Float", "CollisionHeight") },
        ];
        t.function(class, "GetBoundingCylinder", &params, None, &[], &spec, |_| None);
    }
    t.p.build()
}

/// `WillowGame.upk` (invented): `WillowPickup` under `Actor` and `WillowPlayerPawn` under `Pawn`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    t.class("WillowPickup", actor);
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

    /// This base's stand-in for #518's counters, which reached main after #595's base: every heap object's class, slots
    /// and store state ([`Fx::fingerprint`]: no write, no new object, no load) and the RNG streams' whole state (no
    /// draw). Once #595 is rebased onto main, `committed_writes`, `load_transitions` and `draws` replace it.
    fn counters(&self) -> (String, ow_core::rand::GameRng) {
        (self.fingerprint(), self.vm.rng.clone())
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

    fn primitive(&mut self, attached: bool, collides: bool) -> ObjRef {
        let c = self.live("Engine.SkeletalMeshComponent");
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

    /// Every heap object's class, slots and store state: equal before and after means no slot was written, no object
    /// made and nothing loaded.
    fn fingerprint(&self) -> String {
        let heap = &self.vm.space.heap;
        heap.handles()
            .into_iter()
            .map(|h| {
                let o = heap.get(h).unwrap();
                format!("{h:?} {:?} {:?} {}\n", o.class, o.slots, self.vm.defs.is_unloaded(&self.vm.space, h))
            })
            .collect()
    }

    /// The answer for `o` must be a `NotYet` of `missing` with `what` on `record`, and leave the heap and store as
    /// they were.
    fn refuses(&self, o: ObjRef, missing: Missing, what: &str, record: &str) {
        let before = self.fingerprint();
        let counters = self.counters();
        match answer(&self.vm, o) {
            Err(VmError::NotYet(n)) => {
                assert_eq!((n.missing, n.what.as_ref(), n.record), (missing, what, record));
            }
            other => panic!("expected the NotYet {what:?}, got {other:?}"),
        }
        assert_eq!(self.fingerprint(), before, "a refusal writes, makes and loads nothing");
        assert_eq!(self.counters(), counters, "#518's counters");
    }

    /// The answer for `o` must be `want`, with #518's counters unchanged (the answer itself writes nothing).
    fn answers(&self, o: ObjRef, want: (f32, f32), label: &str) {
        let counters = self.counters();
        assert_eq!(answer(&self.vm, o).unwrap(), want, "{label}");
        assert_eq!(self.counters(), counters, "{label}: #518's counters");
    }
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
    assert_eq!(x.call("NavigationPoint", Some(nav)).unwrap(), (Value::Float(40.0), Value::Float(80.0)));
    let pickup = x.live("WillowGame.WillowPickup");
    assert_eq!(x.call("Actor", Some(pickup)).unwrap(), (Value::Float(0.0), Value::Float(0.0)), "no components");
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
    let before = x.fingerprint();
    assert!(matches!(x.call("Actor", Some(pickup)), Err(VmError::NotYet(ref n)) if n.what == REFUSAL_L));
    assert_eq!(x.fingerprint(), before);
}

/// A class-default actor: unattached (or attached but not colliding) loaded entries give 0 and 0; an attached
/// colliding entry is S.
#[test]
fn a_template_reads_its_entries_then_meets_the_skip_flag() {
    let mut x = fx();
    let t = x.template("Engine.Actor");
    let (unattached, quiet, colliding) = (x.primitive(false, true), x.primitive(true, false), x.primitive(true, true));
    x.components(t, vec![None, Some(unattached), Some(quiet)]);
    assert_eq!(answer(&x.vm, t).unwrap(), (0.0, 0.0));
    x.components(t, vec![Some(unattached), Some(colliding)]);
    x.refuses(t, Missing::NotRead, SKIP_FLAG, RECORD);
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
        assert_eq!(answer(&x.vm, live).unwrap(), (0.0, 0.0), "{class}: the actor default, no components");
        x.components(live, vec![Some(c)]);
        x.refuses(live, l(), REFUSAL_L, RECORD);

        let t = x.template(class);
        x.set(t, "CylinderComponent", Value::Object(Some(c)));
        assert_eq!(answer(&x.vm, t).unwrap(), (22.0, 44.0), "{class}");
        // The same component as the collision component: the actor default.
        x.set(t, "CollisionComponent", Value::Object(Some(c)));
        assert_eq!(answer(&x.vm, t).unwrap(), (0.0, 0.0), "{class}");
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
        assert_eq!(answer(&x.vm, nav).unwrap(), (50.0, 75.0));
    }
    let nav = x.live("Engine.NavigationPoint");
    assert_eq!(answer(&x.vm, nav).unwrap(), (0.0, 0.0));
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
    let before = x.fingerprint();
    assert_eq!(answer(&x.vm, pickup).unwrap(), (0.0, 0.0));
    assert_eq!(x.fingerprint(), before, "the non-primitive lazy entry was skipped, not loaded");
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
    script_error(answer(&x.vm, t), "CollisionRadius");
    script_error(x.call("Actor", None), NATIVE);
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
    assert_eq!(answer(&x.vm, pickup).unwrap(), (0.0, 0.0));
    let nav = x.export("Engine.NavigationPoint");
    let c = x.cylinder(31.0, 62.0);
    x.set(nav, "CylinderComponent", Value::Object(Some(c)));
    assert_eq!(answer(&x.vm, nav).unwrap(), (31.0, 62.0));
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
    x.refuses(actor, Missing::NotRead, SKIP_FLAG, RECORD);
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
