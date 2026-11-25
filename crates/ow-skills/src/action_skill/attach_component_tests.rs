//! Tests of `Actor.AttachComponent(object)` in the skill worlds (lane BIND-ATTACHCOMPONENT-SKILL-WORLDS): the action
//! skill pack binds `ow_inventory::attach_mesh`'s rule (NATIVE_ATTACH_MESH_TO_PICKUP.md part 1) for Phaselock's light
//! attach. One invented `Engine` package built with `ow_package::testkit` (AGENTS.md rule 9): `Object`, `Actor`
//! (`Components`, `bDeleteMe`, the native `AttachComponent`), `ActorComponent` (`Owner` and the three flags), a light
//! component under it, and a `Pawn` under `Actor`. Every call goes through the VM in a world built with the action skill
//! pack, and every refusal is checked before any write with #518's three counters.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::{Missing, NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::world::{BalanceState, World};

use ow_inventory::attach_mesh::ATTACH_COMPONENT_PATH;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 6] = ["Class", "Function", "ObjectProperty", "BoolProperty", "IntProperty", "ArrayProperty"];

/// The component fields the attach may write, checked unchanged after a refusal.
const WRITTEN: [&str; 4] = ["Owner", "bAttached", "bNeedsReattach", "bNeedsUpdateTransform"];

/// The `Engine` package; with `mismatch`, `Actor.AttachComponent` takes an int, so its native key is not the rule's (review r2 of
/// #623).
fn engine_with(mismatch: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let component = t.class("ActorComponent", object);
    let array = t.p.next_export();
    t.prop("Array", actor, "Components", 0, array + 1);
    t.prop("Object", array, "Components", 0, component);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    let new_component =
        if mismatch { Param::new("Int", "NewComponent") } else { Param { type_ref: component, ..Param::new("Object", "NewComponent") } };
    t.function(actor, "AttachComponent", &[new_component], None, &[], &FunctionSpec::new(NATIVE, "AttachComponent"), |_| None);
    t.prop("Object", component, "Owner", 0, actor);
    for flag in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform"] {
        t.prop("Bool", component, flag, 0, 0);
    }
    let light = t.class("LightComponent", component);
    t.class("PointLightComponent", light);
    t.class("Pawn", actor);
    t.build()
}

/// A second invented package, `Other`, whose own `Actor` declares a native `AttachComponent` under the same key (review
/// r1 of #623): a package binding of it is the conflicting handler.
fn other() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let component = t.class("ActorComponent", object);
    let new_component = Param { type_ref: component, ..Param::new("Object", "NewComponent") };
    t.function(actor, "AttachComponent", &[new_component], None, &[], &FunctionSpec::new(NATIVE, "AttachComponent"), |_| None);
    t.build()
}

const OTHER_ATTACH: &str = "Other.Actor.AttachComponent";

struct Fx {
    world: World,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Where the inventory's own registration of the key runs, for the combined worlds.
#[derive(Clone, Copy, PartialEq)]
enum Inventory {
    /// Not registered: the Phaselock world's pack set.
    Absent,
    /// Registered on the VM before the world's packs.
    Before,
    /// Registered on the world's VM after its packs.
    After,
}

/// A world in `mode` built with the action skill pack, the inventory's registration placed by `inventory`, and the
/// empty scene layer when `scene` is set.
fn fx(mode: ParityMode, scene: bool, inventory: Inventory) -> Fx {
    fx_with(mode, scene, inventory, false, false)
}

/// [`fx`]; with `conflict`, a refusing handler bound for `Other`'s function before any pack.
/// With `mismatch`, `Engine`'s `AttachComponent` is declared under another native key ([`engine_with`]).
fn fx_with(mode: ParityMode, scene: bool, inventory: Inventory, conflict: bool, mismatch: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-skills-attach-component-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let e = engine_with(mismatch);
    let o = other();
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("Other.upk", o.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["Engine.Actor", "Engine.Pawn", "Engine.PointLightComponent", "Other.Actor"] {
        vm.find_class(c).unwrap();
    }
    if conflict {
        let f = vm.find_function(OTHER_ATTACH).unwrap();
        vm.register_native_for(f, Rc::new(|_: &mut NativeCall<'_>| Err(NotYet::not_ported("Other attach", "TEST").into())))
            .unwrap();
    }
    if inventory == Inventory::Before {
        ow_inventory::attach_mesh::register(&mut vm).unwrap();
    }
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap();
    let packs: [&dyn NativePack; 1] = [&crate::action_skill::Pack];
    let mut world = World::new(vm, BalanceState::new(snapshot), &packs).unwrap();
    if inventory == Inventory::After {
        ow_inventory::attach_mesh::register(&mut world.vm).unwrap();
    }
    if scene {
        ow_inventory::attach_mesh::add_empty_scene(&mut world.vm).unwrap();
    }
    Fx { world, dir }
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.world.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.world.vm, c).unwrap()
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let vm = &self.world.vm;
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = &mut self.world.vm;
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn written(&self, o: ObjRef) -> Vec<Value> {
        WRITTEN.iter().map(|f| self.get(o, f)).collect()
    }

    /// #518's three counters: committed heap writes, RNG draws and definition load transitions.
    fn counters(&self) -> [u64; 3] {
        let vm = &self.world.vm;
        [vm.space.heap.committed_writes(), vm.rng.draws(), (vm.defs.load_transitions() + vm.class_code_loads())]
    }

    fn attach(&mut self, actor: ObjRef, component: ObjRef) -> Result<Value, VmError> {
        let f = self.world.vm.find_function(ATTACH_COMPONENT_PATH)?;
        Ok(self.world.vm.call(f, Some(actor), vec![Value::Object(Some(component))])?.ret)
    }

    fn bound(&mut self) -> bool {
        self.bound_at(ATTACH_COMPONENT_PATH)
    }

    fn bound_at(&mut self, path: &str) -> bool {
        let f = self.world.vm.find_function(path).unwrap();
        self.world.vm.is_bound(f)
    }
}

fn not_yet(r: Result<Value, VmError>) -> NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Phaselock's case in the skill world: a light component attached to a pawn. The attach makes only the rule's
/// script-visible writes, exactly four committed (the two flags, `Owner`, `Components`): both flags false, `Owner` the
/// pawn, the component appended. No draw and no load.
#[test]
fn the_skill_world_attaches_a_light_to_a_pawn() {
    let mut x = fx(ParityMode::Off, true, Inventory::Absent);
    assert!(x.bound(), "the action skill pack binds the key in Off");
    let pawn = x.obj("Engine.Pawn");
    let light = x.obj("Engine.PointLightComponent");
    x.set(light, "bNeedsReattach", Value::Bool(true));
    x.set(light, "bNeedsUpdateTransform", Value::Bool(true));
    let before = x.counters();
    assert_eq!(x.attach(pawn, light).unwrap(), Value::None);
    let after = x.counters();
    assert_eq!(after[0] - before[0], 4, "exactly the four planned writes");
    assert_eq!((after[1], after[2]), (before[1], before[2]), "no draw and no load");
    assert_eq!(
        x.written(light),
        vec![Value::Object(Some(pawn)), Value::Bool(false), Value::Bool(false), Value::Bool(false)]
    );
    assert_eq!(x.get(pawn, "Components"), Value::array(vec![Value::Object(Some(light))]));
}

/// The skill world's own scene layer: `World::new` installs the empty scene layer in Off (#610, a logged harness
/// input), so a skill world that adds none itself still has one and the attach answers, with the rule's four
/// committed writes (the two flags, `Owner`, `Components`) and no draw and no load. A world without the layer, which
/// `World::new` can no longer build, is #610's own test in `ow-world/src/actors/component_attach_tests.rs`.
#[test]
fn the_skill_world_s_own_scene_layer_lets_the_attach_answer() {
    let mut x = fx(ParityMode::Off, false, Inventory::Absent);
    let pawn = x.obj("Engine.Pawn");
    let light = x.obj("Engine.PointLightComponent");
    x.set(light, "bNeedsReattach", Value::Bool(true));
    x.set(light, "bNeedsUpdateTransform", Value::Bool(true));
    let before = x.counters();
    assert_eq!(x.attach(pawn, light).unwrap(), Value::None);
    let after = x.counters();
    assert_eq!(after[0] - before[0], 4, "exactly the four planned writes");
    assert_eq!((after[1], after[2]), (before[1], before[2]), "no draw and no load");
    assert_eq!(
        x.written(light),
        vec![Value::Object(Some(pawn)), Value::Bool(false), Value::Bool(false), Value::Bool(false)]
    );
    assert_eq!(x.get(pawn, "Components"), Value::array(vec![Value::Object(Some(light))]));
}

/// A light already attached is the rule's detach-before-attach refusal, not read, before any write.
#[test]
fn an_attached_light_refuses_before_any_write() {
    let mut x = fx(ParityMode::Off, true, Inventory::Absent);
    let pawn = x.obj("Engine.Pawn");
    let light = x.obj("Engine.PointLightComponent");
    x.set(light, "bAttached", Value::Bool(true));
    let (before, counters) = (x.written(light), x.counters());
    let n = not_yet(x.attach(pawn, light));
    assert_eq!(n.missing, Missing::NotRead);
    assert!(!ow_core::notyet::is_partial_writes(&n.what), "a preflight refusal is unmarked");
    assert_eq!((x.written(light), x.counters()), (before, counters));
}

/// A world with both registrations, in either order, binds the one function once (no duplicate registration) and
/// gives the same attach. The orders differ in coverage, by design: inventory first binds the key VM-wide, so
/// `Other`'s function is bound too; the action-skill pack first binds `Engine`'s alone, and inventory reuses it.
#[test]
fn both_registration_orders_keep_one_binding() {
    for (order, other_bound) in [(Inventory::Before, true), (Inventory::After, false)] {
        let mut x = fx(ParityMode::Off, true, order);
        assert!(x.bound());
        assert_eq!(x.bound_at(OTHER_ATTACH), other_bound);
        let pawn = x.obj("Engine.Pawn");
        let light = x.obj("Engine.PointLightComponent");
        x.attach(pawn, light).unwrap();
        assert_eq!(x.get(light, "Owner"), Value::Object(Some(pawn)));
        assert_eq!(x.get(pawn, "Components"), Value::array(vec![Value::Object(Some(light))]));
    }
}

/// Reference: the action skill pack binds nothing new, so the key stays unbound as before.
#[test]
fn reference_binds_nothing_new() {
    let mut x = fx(ParityMode::Reference, true, Inventory::Absent);
    assert!(!x.bound());
}

/// Review r1 of #623: a conflicting handler bound for another package's same-keyed function is never mistaken for the
/// reuse. The inventory's registration keeps its duplicate-registration error with or without the action-skill pack's
/// `Engine` binding, and the action-skill pack still binds `Engine`'s function.
#[test]
fn a_conflicting_binding_keeps_the_duplicate_error() {
    let mut x = fx_with(ParityMode::Off, true, Inventory::Absent, true, false);
    assert!(x.bound(), "the action-skill pack binds Engine's function beside the conflict");
    assert!(matches!(
        ow_inventory::attach_mesh::register(&mut x.world.vm),
        Err(VmError::Registration { reason: "duplicate native registration", .. })
    ));
    let pawn = x.obj("Engine.Pawn");
    let light = x.obj("Engine.PointLightComponent");
    x.attach(pawn, light).unwrap();
    assert_eq!(x.get(light, "Owner"), Value::Object(Some(pawn)));
}

/// Review r2 of #623: when `Engine`'s `AttachComponent` is declared under another native key, the action-skill pack
/// binds nothing and records no reuse, so `Other`'s conflicting handler, the key's only binding, still gives the
/// inventory its duplicate-registration error.
#[test]
fn a_mismatched_signature_binds_nothing_and_keeps_the_error() {
    let mut x = fx_with(ParityMode::Off, true, Inventory::Absent, true, true);
    let f = x.world.vm.find_function(ATTACH_COMPONENT_PATH).unwrap();
    assert_ne!(x.world.vm.function(f).unwrap().native_key.as_deref(), Some(ow_inventory::attach_mesh::ATTACH_COMPONENT));
    assert!(!x.bound(), "the action-skill pack leaves the other signature unbound");
    assert!(matches!(
        ow_inventory::attach_mesh::register(&mut x.world.vm),
        Err(VmError::Registration { reason: "duplicate native registration", .. })
    ));
}
