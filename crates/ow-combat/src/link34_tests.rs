//! NATIVE_LINK34_AFTER_MIND.md section 10, tests 3 to 8, on invented packages (AGENTS.md rule 9). The class and field
//! names are the packages'; every value is invented. Each answer is checked write-free. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{property_payload, toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::world::{BalanceState, World};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC_NATIVE: u32 = 0x400 | 0x2000 | 0x20000;
const KINDS: [&str; 9] = ["Byte", "Bool", "Object", "Float", "Struct", "Array", "Name", "Interface", "Int"];

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], ret: Option<(&str, i32)>, flags: u32) {
    let spec = FunctionSpec::new(flags, name);
    t.function(owner, name, params, ret, &[], &spec, |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let pawn = t.class("Pawn", actor);
    let controller = t.class("Controller", actor);
    t.prop("Object", pawn, "Controller", 0, controller);
    t.prop("Object", pawn, "DrivenVehicle", 0, pawn);
    t.prop("Bool", pawn, "bCanBeInjured", 0, 0);
    native(&mut t, pawn, "InGodMode", &[], Some(("Bool", 0)), NATIVE);
    native(&mut t, pawn, "IsVulnerableToFriendlyFire", &[], Some(("Bool", 0)), NATIVE);
    t.prop("Object", controller, "Pawn", 0, pawn);
    for f in ["bGodMode", "bNonCheatGodMode"] {
        t.prop("Bool", controller, f, 0, 0);
    }
    native(&mut t, controller, "InGodMode", &[], Some(("Bool", 0)), NATIVE);
    let game = t.class("GameInfo", actor);
    let victim = Param { type_ref: object, ..Param::new("Object", "Victim") };
    let instigator = Param { type_ref: object, ..Param::new("Interface", "InstigatingTargetable") };
    native(&mut t, game, "IsFriendlyFire", &[victim, instigator], Some(("Bool", 0)), NATIVE);
    let damage = t.class("DamageTypeDefinition", object);
    native(&mut t, damage, "IsHealingDamageType", &[], Some(("Bool", 0)), NATIVE);
    // The init data (a toy struct of the attribute evaluator's fields).
    let init = toy_struct(&mut t, damage, "AttributeInitializationData", None);
    t.prop("Float", init, "BaseValueConstant", 0, 0);
    t.prop("Object", init, "BaseValueAttribute", 0, object);
    t.prop("Object", init, "InitializationDefinition", 0, object);
    t.prop("Float", init, "BaseValueScaleConstant", 0, 0);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let controller = t.p.add_import("Core", "Class", engine, "Controller");
    let component = t.class("AIComponent", object);
    toy_array_of(&mut t, component, "GodHolds", "Object", object);
    let mind = t.class("GearboxMind", controller);
    t.prop("Object", mind, "AIComponent", 0, component);
    native(&mut t, mind, "InGodMode", &[], Some(("Bool", 0)), NATIVE);
    t.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let game = t.p.add_import("Core", "Class", engine, "GameInfo");
    let damage = t.p.add_import("Core", "Class", engine, "DamageTypeDefinition");
    let gearbox = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_mind = t.p.add_import("Core", "Class", gearbox, "GearboxMind");
    let mind = t.class("WillowMind", gearbox_mind);
    native(&mut t, mind, "GetAIGod", &[], Some(("Bool", 0)), STATIC_NATIVE);
    native(&mut t, mind, "SetAIGod", &[Param::new("Bool", "bNewGod")], None, STATIC_NATIVE);
    native(&mut t, mind, "InGodMode", &[], Some(("Bool", 0)), NATIVE);
    let willow_game = t.class("WillowGameInfo", game);
    let coop = t.class("WillowCoopGameInfo", willow_game);
    let victim = Param { type_ref: object, ..Param::new("Object", "Victim") };
    let instigator = Param { type_ref: object, ..Param::new("Interface", "InstigatingTargetable") };
    native(&mut t, coop, "IsFriendlyFire", &[victim, instigator], Some(("Bool", 0)), NATIVE);
    // The damage type: DamageType a byte of an invented EDamageType whose seventh name is DAMAGE_TYPE_Healing.
    let willow_damage = t.class("WillowDamageTypeDefinition", damage);
    let names = ["DAMAGE_TYPE_A", "DAMAGE_TYPE_B", "DAMAGE_TYPE_C", "DAMAGE_TYPE_D", "DAMAGE_TYPE_E", "DAMAGE_TYPE_Impact", "DAMAGE_TYPE_Healing"];
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend(ints(&[0, names.len() as i32]));
    for n in names {
        payload.extend_from_slice(&t.p.fname(n, 0));
    }
    let mut e = Export::new(t.import("Enum"), "EDamageType", payload);
    e.outer = willow_damage;
    let kinds = t.p.add_export_with(e);
    t.prop("Byte", willow_damage, "DamageType", 0, kinds);
    let init = t.p.add_import("Core", "ScriptStruct", damage, "AttributeInitializationData");
    for f in ["FleshMomentumModifier", "ArmorMomentumModifier", "ShieldMomentumModifier"] {
        t.prop("Struct", willow_damage, f, 0, init);
    }
    native(&mut t, willow_damage, "IsHealingDamageType", &[], Some(("Bool", 0)), NATIVE);
    // The pawn's surface reads.
    let willow_pawn = t.class("WillowPawn", pawn);
    let shield_ref = toy_struct(&mut t, willow_pawn, "ToyShieldRef", None);
    t.prop("Object", shield_ref, "PoolManager", 0, object);
    t.prop("Byte", shield_ref, "PoolIndexInManager", 0, 0);
    t.prop("Byte", shield_ref, "PoolGUID", 0, 0);
    t.prop("Object", shield_ref, "Data", 0, object);
    t.prop("Struct", willow_pawn, "ShieldArmor", 0, shield_ref);
    for f in ["MyHitRegionHelper", "BodyClass"] {
        t.prop("Object", willow_pawn, f, 0, object);
    }
    let hit = toy_struct(&mut t, willow_pawn, "TraceHitInfo", None);
    t.prop("Name", hit, "BoneName", 0, 0);
    let params = [
        Param::new("Float", "Magnitude"),
        Param { type_ref: object, ..Param::new("Object", "InstigatedBy") },
        Param { type_ref: object, ..Param::new("Object", "DamageType") },
        Param { type_ref: hit, ..Param::new("Struct", "HitInfo") },
    ];
    native(&mut t, willow_pawn, "AdjustMomentumForSurfaceType", &params, Some(("Float", 0)), NATIVE);
    let player = t.class("WillowPlayerPawn", willow_pawn);
    t.prop("Float", player, "FriendlyFireDamageAcceptedBaseValue", 0, 0);
    native(&mut t, player, "IsVulnerableToFriendlyFire", &[], Some(("Bool", 0)), NATIVE);
    t.class("WillowAIPawn", willow_pawn);
    let body = t.class("BodyClassDefinition", object);
    toy_array_of(&mut t, body, "HitRegionList", "Object", object);
    t.prop("Object", body, "DefaultHitRegion", 0, object);
    let region = t.class("HitRegionDefinition", object);
    toy_array_of(&mut t, region, "RegionBoneNames", "Name", 0);
    t.prop("Object", region, "PhysicalMaterial", 0, object);
    t.prop("Byte", region, "DefaultDamageSurfaceType", 0, 0);
    let material = t.class("PhysicalMaterial", object);
    t.prop("Object", material, "PhysicalMaterialProperty", 0, object);
    let property = t.class("WillowPhysicalMaterialProperty", object);
    t.prop("Byte", property, "DamageSurfaceType", 0, 0);
    // Review of #798: a live shield pool (a manager of 16 pool slots, a pool with a GUID and a value), an
    // initialization definition whose formula reaches a nested attribute, and that attribute as an export the store
    // has not loaded.
    let shield_pool = t.class("ToyShieldPool", object);
    t.prop("Byte", shield_pool, "PoolGUID", 0, 0);
    t.prop("Float", shield_pool, "CurrentValue", 0, 0);
    let manager = t.class("ToyPoolManager", object);
    let payload = property_payload(&t.none, 16, 0, &ints(&[shield_pool]));
    let mut slots = Export::new(t.import("ObjectProperty"), "ResourcePools", payload);
    slots.outer = manager;
    t.p.add_export_with(slots);
    let definition = t.class("ToyInitDefinition", object);
    let formula = toy_struct(&mut t, definition, "ToyFormula", None);
    t.prop("Bool", formula, "bEnabled", 0, 0);
    for f in ["Multiplier", "Level", "Power", "Offset"] {
        t.prop("Struct", formula, f, 0, init);
    }
    let off = toy_struct(&mut t, definition, "ToyOff", None);
    for f in ["bEnabled", "bEnableMinValueRestriction", "bEnableMaxValueRestriction"] {
        t.prop("Bool", off, f, 0, 0);
    }
    t.prop("Struct", definition, "ValueFormula", 0, formula);
    for f in ["ConditionalInitialization", "RandomVariance", "RangeRestriction"] {
        t.prop("Struct", definition, f, 0, off);
    }
    for f in ["BaseValueMode", "RoundingMode"] {
        t.prop("Byte", definition, f, 0, 0);
    }
    let mut cold = ints(&[0]);
    cold.extend_from_slice(&t.none);
    let stored = t.p.add_export(body, "ToyColdAttribute", cold);
    // Its holder: a loaded holder's field is a store handle not yet loaded.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Cold", 0, object);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Cold", stored));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

struct Fx {
    world: World,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-link34-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
        .with_stub_policy(StubPolicy::refuse_all());
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
    let world = World::new(vm, BalanceState::new(snapshot), &[&crate::hit::Pack]).unwrap();
    Fx { world, dir }
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }
    fn make(&mut self, path: &str) -> ObjRef {
        let vm = self.vm();
        let c = vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = self.vm();
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, name).unwrap_or_else(|| panic!("no {name}"));
        *vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }
    fn get(&mut self, o: ObjRef, name: &str) -> Value {
        let vm = self.vm();
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, name).unwrap();
        vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn marks(&mut self) -> (u64, u64, u64) {
        let vm = self.vm();
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
    }
    /// `path` on `this` with `args`, checked write-free (test 8).
    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let before = self.marks();
        let vm = self.vm();
        let f = vm.find_function(path).unwrap();
        let r = vm.call(f, this, args).map(|r| r.ret);
        if !path.ends_with("SetAIGod") {
            assert_eq!(self.marks(), before, "{path}: nothing written, drawn or loaded");
        }
        r
    }
    fn lazy(&mut self, path: &str) -> ObjRef {
        let vm = self.vm();
        let c = vm.find_class(path).unwrap();
        vm.space.alloc_stand_in(c, "Lazy", Origin::Default { class: c }).unwrap()
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.what.to_string()
        }
        other => panic!("{other:?}"),
    }
}

// Test 3.
#[test]
fn is_friendly_fire_is_false_but_on_a_coop_game_info() {
    let mut x = fx();
    let victim = x.make("Engine.Pawn");
    let args = || vec![Value::Object(Some(victim)), Value::Interface(None)];
    for class in ["WillowGame.WillowGameInfo", "Engine.GameInfo"] {
        let game = x.make(class);
        assert_eq!(x.call("Engine.GameInfo.IsFriendlyFire", Some(game), args()).unwrap(), Value::Bool(false), "{class}");
    }
    let coop = x.make("WillowGame.WillowCoopGameInfo");
    assert_eq!(not_read(x.call("WillowGame.WillowCoopGameInfo.IsFriendlyFire", Some(coop), args())), COOP_FRIENDLY_FIRE);
    // A mistyped argument (review of #798, finding 6): VmError::Script.
    let game = x.make("Engine.GameInfo");
    for bad in [vec![Value::Int(3), Value::Interface(None)], vec![Value::Object(Some(victim)), Value::Int(3)]] {
        match x.call("Engine.GameInfo.IsFriendlyFire", Some(game), bad) {
            Err(VmError::Script { .. }) => {}
            other => panic!("{other:?}"),
        }
    }
}

// Test 4.
#[test]
fn in_god_mode_by_controller_class() {
    let mut x = fx();
    let pawn = x.make("Engine.Pawn");
    let god = |x: &mut Fx, o| x.call("Engine.Pawn.InGodMode", Some(o), vec![]).unwrap();
    assert_eq!(god(&mut x, pawn), Value::Bool(false), "no controller, no vehicle");
    let c = x.make("Engine.Controller");
    x.set(pawn, "Controller", Value::Object(Some(c)));
    for f in ["bGodMode", "bNonCheatGodMode"] {
        x.set(c, f, Value::Bool(true));
        assert_eq!(god(&mut x, pawn), Value::Bool(true), "{f}");
        x.set(c, f, Value::Bool(false));
    }
    assert_eq!(god(&mut x, pawn), Value::Bool(false));
    // A vehicle driver's controller.
    let driver = x.make("Engine.Pawn");
    let vehicle = x.make("Engine.Pawn");
    x.set(driver, "DrivenVehicle", Value::Object(Some(vehicle)));
    x.set(vehicle, "Controller", Value::Object(Some(c)));
    x.set(c, "bGodMode", Value::Bool(true));
    assert_eq!(god(&mut x, driver), Value::Bool(true), "the driven vehicle's controller");
    // A WillowMind: the AI-god flag, the pawn's bCanBeInjured, the god holds.
    let mind = x.make("WillowGame.WillowMind");
    let mind_god = |x: &mut Fx| x.call("WillowGame.WillowMind.InGodMode", Some(mind), vec![]).unwrap();
    assert_eq!(mind_god(&mut x), Value::Bool(false), "nothing");
    x.call("WillowGame.WillowMind.SetAIGod", None, vec![Value::Bool(true)]).unwrap();
    assert_eq!(x.call("WillowGame.WillowMind.GetAIGod", None, vec![]).unwrap(), Value::Bool(true), "round trip");
    assert_eq!(mind_god(&mut x), Value::Bool(true), "the AI-god flag");
    x.call("WillowGame.WillowMind.SetAIGod", None, vec![Value::Bool(false)]).unwrap();
    let component = x.make("GearboxFramework.AIComponent");
    let tag = x.make("Engine.Object");
    x.set(component, "GodHolds", Value::array(vec![Value::Object(Some(tag))]));
    x.set(mind, "AIComponent", Value::Object(Some(component)));
    assert_eq!(mind_god(&mut x), Value::Bool(true), "no Pawn: the god hold decides");
    let mind_pawn = x.make("Engine.Pawn");
    x.set(mind, "Pawn", Value::Object(Some(mind_pawn)));
    x.set(mind_pawn, "bCanBeInjured", Value::Bool(true));
    assert_eq!(mind_god(&mut x), Value::Bool(false), "the mind's pawn can be injured");
    x.set(mind_pawn, "bCanBeInjured", Value::Bool(false));
    assert_eq!(mind_god(&mut x), Value::Bool(true), "a god hold");
    x.set(component, "GodHolds", Value::array(vec![]));
    assert_eq!(mind_god(&mut x), Value::Bool(false), "no hold");
    // Unloaded objects: the not-loaded NotRead.
    let lazy = x.lazy("WillowGame.WillowMind");
    let p = x.make("Engine.Pawn");
    x.set(p, "Controller", Value::Object(Some(lazy)));
    assert_eq!(not_read(x.call("Engine.Pawn.InGodMode", Some(p), vec![])), GOD_NOT_LOADED);
    // A driven vehicle the store has not loaded (review of #798, finding 5).
    let lazy_vehicle = x.lazy("Engine.Pawn");
    let rider = x.make("Engine.Pawn");
    x.set(rider, "DrivenVehicle", Value::Object(Some(lazy_vehicle)));
    assert_eq!(not_read(x.call("Engine.Pawn.InGodMode", Some(rider), vec![])), GOD_NOT_LOADED);
    let lazy_component = x.lazy("GearboxFramework.AIComponent");
    x.set(mind, "AIComponent", Value::Object(Some(lazy_component)));
    assert_eq!(not_read(x.call("WillowGame.WillowMind.InGodMode", Some(mind), vec![])), GOD_NOT_LOADED);
}

// Test 5.
#[test]
fn is_healing_damage_type() {
    let mut x = fx();
    let d = x.make("WillowGame.WillowDamageTypeDefinition");
    let path = "WillowGame.WillowDamageTypeDefinition.IsHealingDamageType";
    x.set(d, "DamageType", Value::Byte(6));
    assert_eq!(x.call(path, Some(d), vec![]).unwrap(), Value::Bool(true));
    x.set(d, "DamageType", Value::Byte(5));
    assert_eq!(x.call(path, Some(d), vec![]).unwrap(), Value::Bool(false));
    let base = x.make("Engine.DamageTypeDefinition");
    assert_eq!(x.call("Engine.DamageTypeDefinition.IsHealingDamageType", Some(base), vec![]).unwrap(), Value::Bool(false));
}

// Test 6.
#[test]
fn adjust_momentum_for_surface_type() {
    let mut x = fx();
    let path = "WillowGame.WillowPawn.AdjustMomentumForSurfaceType";
    let pawn = x.make("WillowGame.WillowAIPawn");
    let instigator = x.make("Engine.Controller");
    let hit = x.get(pawn, "MyHitRegionHelper");
    let _ = hit;
    let hit_info = |x: &mut Fx, bone: &str| {
        let vm = x.vm();
        let ty = vm.space.classes.struct_by_path("WillowGame.WillowPawn.TraceHitInfo").unwrap();
        let n = vm.space.names.intern(bone);
        Value::structure(ty, vec![Value::Name(n)])
    };
    let args = |x: &mut Fx, d: Option<ObjRef>, bone: &str| {
        let h = hit_info(x, bone);
        vec![Value::Float(10.0), Value::Object(Some(instigator)), Value::Object(d), h]
    };
    // No damage type: the magnitude.
    let a = args(&mut x, None, "Head");
    assert_eq!(x.call(path, Some(pawn), a).unwrap(), Value::Float(10.0));
    // A damage type with constant scales: flesh 2, armor 3, shield 4.
    let d = x.make("WillowGame.WillowDamageTypeDefinition");
    for (field, k) in [("FleshMomentumModifier", 2.0f32), ("ArmorMomentumModifier", 3.0), ("ShieldMomentumModifier", 4.0)] {
        let mut v = x.get(d, field);
        {
            let vm = x.vm();
            let st = v.struct_mut().unwrap();
            let layout = vm.space.classes.struct_layout(st.ty).unwrap().clone();
            for (f, val) in [("BaseValueConstant", k), ("BaseValueScaleConstant", 1.0)] {
                let (i, _) = layout.field(vm.space.names.find(f).unwrap()).unwrap();
                st.fields[i as usize] = Value::Float(val);
            }
        }
        x.set(d, field, v);
    }
    // No helper: surface 0, scale 1.0.
    let a = args(&mut x, Some(d), "Head");
    assert_eq!(x.call(path, Some(pawn), a).unwrap(), Value::Float(10.0), "no helper");
    // A helper and a body class: the region by bone, its material property's surface 2 (armor).
    let helper = x.make("Engine.Object");
    let body = x.make("WillowGame.BodyClassDefinition");
    let region = x.make("WillowGame.HitRegionDefinition");
    let material = x.make("WillowGame.PhysicalMaterial");
    let property = x.make("WillowGame.WillowPhysicalMaterialProperty");
    let head = x.vm().space.names.intern("Head");
    x.set(region, "RegionBoneNames", Value::array(vec![Value::Name(head)]));
    x.set(region, "PhysicalMaterial", Value::Object(Some(material)));
    x.set(material, "PhysicalMaterialProperty", Value::Object(Some(property)));
    x.set(property, "DamageSurfaceType", Value::Byte(2));
    x.set(region, "DefaultDamageSurfaceType", Value::Byte(1));
    x.set(body, "HitRegionList", Value::array(vec![Value::Object(Some(region))]));
    x.set(pawn, "MyHitRegionHelper", Value::Object(Some(helper)));
    x.set(pawn, "BodyClass", Value::Object(Some(body)));
    let a = args(&mut x, Some(d), "Head");
    assert_eq!(x.call(path, Some(pawn), a).unwrap(), Value::Float(30.0), "armor by the material property");
    // A region with no material: its DefaultDamageSurfaceType (1, flesh).
    x.set(region, "PhysicalMaterial", Value::Object(None));
    let a = args(&mut x, Some(d), "Head");
    assert_eq!(x.call(path, Some(pawn), a).unwrap(), Value::Float(20.0), "the region's default");
    // Another bone, no default region: surface 0, scale 1.0.
    let a = args(&mut x, Some(d), "Foot");
    assert_eq!(x.call(path, Some(pawn), a).unwrap(), Value::Float(10.0), "no region");
    // A base DamageTypeDefinition: 0.0.
    let base = x.make("Engine.DamageTypeDefinition");
    let a = args(&mut x, Some(base), "Head");
    assert_eq!(x.call(path, Some(pawn), a).unwrap(), Value::Float(0.0), "the base class");
    // An unloaded init-data definition: NotRead before anything is evaluated.
    let lazy = x.lazy("WillowGame.BodyClassDefinition");
    let mut v = x.get(d, "FleshMomentumModifier");
    {
        let vm = x.vm();
        let st = v.struct_mut().unwrap();
        let layout = vm.space.classes.struct_layout(st.ty).unwrap().clone();
        let (i, _) = layout.field(vm.space.names.find("InitializationDefinition").unwrap()).unwrap();
        st.fields[i as usize] = Value::Object(Some(lazy));
    }
    x.set(d, "FleshMomentumModifier", v);
    let a = args(&mut x, Some(d), "Head");
    assert_eq!(not_read(x.call(path, Some(pawn), a)), NOT_LOADED);
}

// Test 7.
#[test]
fn is_vulnerable_to_friendly_fire() {
    let mut x = fx();
    let player = x.make("WillowGame.WillowPlayerPawn");
    let path = "WillowGame.WillowPlayerPawn.IsVulnerableToFriendlyFire";
    assert_eq!(x.call(path, Some(player), vec![]).unwrap(), Value::Bool(false), "0.0");
    x.set(player, "FriendlyFireDamageAcceptedBaseValue", Value::Float(0.5));
    assert_eq!(x.call(path, Some(player), vec![]).unwrap(), Value::Bool(true), "0.5");
    let ai = x.make("WillowGame.WillowAIPawn");
    assert_eq!(x.call("Engine.Pawn.IsVulnerableToFriendlyFire", Some(ai), vec![]).unwrap(), Value::Bool(false));
}

/// The momentum scale under review of #798: the HitInfo must be a struct (finding 7); a live shield pool with a value
/// is surface 3 (finding 6), its manager checked resident first (finding 8); a nested attribute the store has not
/// loaded answers `NotRead` and loads nothing (finding 1; [`Fx::call`] checks the counters).
#[test]
fn adjust_momentum_review_of_798() {
    let mut x = fx();
    let path = "WillowGame.WillowPawn.AdjustMomentumForSurfaceType";
    let pawn = x.make("WillowGame.WillowAIPawn");
    let instigator = x.make("Engine.Controller");
    let d = x.make("WillowGame.WillowDamageTypeDefinition");
    let set_fields = |x: &mut Fx, v: &mut Value, values: Vec<(&str, Value)>| {
        let vm = x.vm();
        let st = v.struct_mut().unwrap();
        let layout = vm.space.classes.struct_layout(st.ty).unwrap().clone();
        for (f, val) in values {
            let (i, _) = layout.field(vm.space.names.find(f).unwrap()).unwrap();
            st.fields[i as usize] = val;
        }
    };
    let mut shield = x.get(d, "ShieldMomentumModifier");
    set_fields(&mut x, &mut shield, vec![("BaseValueConstant", Value::Float(4.0)), ("BaseValueScaleConstant", Value::Float(1.0))]);
    x.set(d, "ShieldMomentumModifier", shield);
    let hit = {
        let vm = x.vm();
        let ty = vm.space.classes.struct_by_path("WillowGame.WillowPawn.TraceHitInfo").unwrap();
        let n = vm.space.names.intern("Head");
        Value::structure(ty, vec![Value::Name(n)])
    };
    let args = |h: Value| vec![Value::Float(10.0), Value::Object(Some(instigator)), Value::Object(Some(d)), h];
    // Finding 7: a HitInfo that is not a struct.
    match x.call(path, Some(pawn), args(Value::Int(0))) {
        Err(VmError::Script { .. }) => {}
        other => panic!("{other:?}"),
    }
    // Finding 6: a live shield pool with a value, surface 3 (scale 4).
    let manager = x.make("WillowGame.ToyPoolManager");
    let pool = x.make("WillowGame.ToyShieldPool");
    x.set(pool, "PoolGUID", Value::Byte(7));
    x.set(pool, "CurrentValue", Value::Float(25.0));
    {
        let vm = x.vm();
        let class = vm.space.heap.get(manager).unwrap().class;
        let p = vm.space.prop(class, "ResourcePools").unwrap();
        *vm.space.slot_mut(manager, PropAddr::element(p, 2)).unwrap() = Value::Object(Some(pool));
    }
    let point = |x: &mut Fx, m: ObjRef| {
        let mut r = x.get(pawn, "ShieldArmor");
        set_fields(x, &mut r, vec![("PoolManager", Value::Object(Some(m))), ("PoolIndexInManager", Value::Byte(2)), ("PoolGUID", Value::Byte(7))]);
        x.set(pawn, "ShieldArmor", r);
    };
    point(&mut x, manager);
    assert_eq!(x.call(path, Some(pawn), args(hit.clone())).unwrap(), Value::Float(40.0), "the shield's scale");
    x.set(pool, "CurrentValue", Value::Float(0.0));
    assert_eq!(x.call(path, Some(pawn), args(hit.clone())).unwrap(), Value::Float(10.0), "an empty pool: surface 0");
    // Finding 8: a manager the store has not loaded.
    let lazy_manager = x.lazy("WillowGame.ToyPoolManager");
    point(&mut x, lazy_manager);
    assert_eq!(not_read(x.call(path, Some(pawn), args(hit.clone()))), NOT_LOADED);
    point(&mut x, manager);
    // Finding 1: the flesh scale's definition is resident, its formula's multiplier names an attribute the store has
    // not loaded. Surface 1 by a region's default.
    let cold = {
        let vm = x.vm();
        let path = ow_core::ObjectPath::parse("WillowGame.ToyHolderObject").unwrap();
        let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        match x.get(holder, "Cold") {
            Value::Object(Some(o)) => o,
            other => panic!("{other:?}"),
        }
    };
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, cold), "the attribute starts unloaded");
    let definition = x.make("WillowGame.ToyInitDefinition");
    let mut formula = x.get(definition, "ValueFormula");
    let mut multiplier = {
        let vm = x.vm();
        let st = formula.as_struct().unwrap();
        let layout = vm.space.classes.struct_layout(st.ty).unwrap().clone();
        let (i, _) = layout.field(vm.space.names.find("Multiplier").unwrap()).unwrap();
        st.fields[i as usize].clone()
    };
    set_fields(&mut x, &mut multiplier, vec![("BaseValueAttribute", Value::Object(Some(cold)))]);
    set_fields(&mut x, &mut formula, vec![("bEnabled", Value::Bool(true)), ("Multiplier", multiplier)]);
    x.set(definition, "ValueFormula", formula);
    let mut flesh = x.get(d, "FleshMomentumModifier");
    set_fields(&mut x, &mut flesh, vec![("InitializationDefinition", Value::Object(Some(definition)))]);
    x.set(d, "FleshMomentumModifier", flesh);
    let helper = x.make("Engine.Object");
    let body = x.make("WillowGame.BodyClassDefinition");
    let region = x.make("WillowGame.HitRegionDefinition");
    x.set(region, "DefaultDamageSurfaceType", Value::Byte(1));
    x.set(body, "DefaultHitRegion", Value::Object(Some(region)));
    x.set(pawn, "MyHitRegionHelper", Value::Object(Some(helper)));
    x.set(pawn, "BodyClass", Value::Object(Some(body)));
    let what = not_read(x.call(path, Some(pawn), args(hit)));
    assert!(what.contains("is an unloaded handle (a load is a write)"), "{what}");
    assert!(x.world.vm.defs.is_unloaded(&x.world.vm.space, cold), "nothing loaded");
}
