//! NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md sections 3.1, 3.6, 3.8 and 4.2, and NATIVE_PICKUP_SAVE_ROTATION.md section 5,
//! on invented classes (AGENTS.md rule 9: invented objects and values, the class and field names the packages'). Every
//! refusal before a write leaves #518's counters, `class_code_loads` and the name table unchanged.

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::ExprMode;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;

/// The invented `EPhysics` order (only the names are the packages').
const PHYSICS: [&str; 4] = ["PHYS_None", "PHYS_Walking", "PHYS_Falling", "PHYS_RigidBody"];
const RIGID: u8 = 3;
const ANNOUNCE: &str = "WillowPlayerController.ScriptAnnounceAmmoGain(int,int)";

struct Fx {
    vm: Vm,
    update: FuncId,
    save_rotation: FuncId,
    dlc: FuncId,
    context: FuncId,
    evaluate: FuncId,
    rotator: StructId,
    data: StructId,
    /// The `ScriptAnnounceAmmoGain` calls: (kind, gain).
    events: Rc<RefCell<Vec<(i32, i32)>>>,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn param(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    decl(vm, name, kind).with_flags(PropertyDecl::PARAM)
}

fn fx_in(mode: ParityMode) -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let o = PropKind::Object { class: object };
    let ints: Vec<_> = ["Pitch", "Yaw", "Roll"].iter().map(|n| decl(&mut vm, n, PropKind::Int)).collect();
    let rotator = vm.space.classes.define_struct("Core.Object.Rotator", None, ints).unwrap();
    let values = PHYSICS.iter().map(|n| vm.space.names.intern(n)).collect();
    let physics = vm.space.classes.define_enum("Engine.Actor.EPhysics", values).unwrap();
    let props = vec![
        decl(&mut vm, "Rotation", PropKind::Struct(rotator)),
        decl(&mut vm, "Physics", PropKind::Byte { enum_: Some(physics) }),
        decl(&mut vm, "CollisionComponent", o.clone()),
        decl(&mut vm, "Instigator", o.clone()),
    ];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "Controller", o.clone()), decl(&mut vm, "DrivenVehicle", o.clone())];
    let pawn = vm.space.classes.define("Engine.Pawn", Some(actor), props).unwrap();
    vm.space.classes.define("Engine.Vehicle", Some(pawn), vec![]).unwrap();
    let props = vec![decl(&mut vm, "Pawn", o.clone()), decl(&mut vm, "ResourcePoolManager", o.clone())];
    let controller = vm.space.classes.define("Engine.Controller", Some(actor), props).unwrap();
    let caches: Vec<_> = AMMO.iter().map(|(_, cache, _)| decl(&mut vm, cache, PropKind::Float)).collect();
    let pc = vm.space.classes.define("WillowGame.WillowPlayerController", Some(controller), caches).unwrap();
    // A controller class with one cache mistyped.
    let mut caches: Vec<_> = AMMO.iter().skip(1).map(|(_, cache, _)| decl(&mut vm, cache, PropKind::Float)).collect();
    caches.push(decl(&mut vm, AMMO[0].1, PropKind::Int));
    vm.space.classes.define("Test.MistypedController", Some(controller), caches).unwrap();
    let gearbox = vm.space.classes.define("GearboxFramework.GearboxGlobalsDefinition", Some(object), vec![]).unwrap();
    let fields: Vec<_> = AMMO.iter().map(|(field, _, _)| decl(&mut vm, field, o.clone())).collect();
    vm.space.classes.define("WillowGame.GlobalsDefinition", Some(gearbox), fields).unwrap();
    let props = vec![decl(&mut vm, "TheGlobalsDefinition", PropKind::Object { class: gearbox })];
    vm.space.classes.define("GearboxFramework.GearboxGlobals", Some(object), props).unwrap();
    // An ammo definition class with a slot, so a slotless stand-in of it is not loaded.
    let props = vec![decl(&mut vm, "BaseValue", PropKind::Float)];
    vm.space.classes.define("Test.AmmoDefinition", Some(object), props).unwrap();
    let dropped = vm.space.classes.define("Engine.DroppedPickup", Some(actor), vec![]).unwrap();
    let pickup = vm.space.classes.define("WillowGame.WillowPickup", Some(dropped), vec![]).unwrap();
    // The inventory classes: the weapon and an item native, a script subclass of the weapon.
    let inventory = vm.space.classes.define("WillowGame.WillowInventory", Some(actor), vec![]).unwrap();
    let member = decl(&mut vm, "BalanceDefinition", o.clone());
    let data = vm.space.classes.define_struct("WillowGame.WillowWeapon.WeaponDefinitionData", None, vec![member]).unwrap();
    let props = vec![decl(&mut vm, "DefinitionData", PropKind::Struct(data))];
    let weapon = vm.space.classes.define("WillowGame.WillowWeapon", Some(inventory), props).unwrap();
    let item = vm.space.classes.define("WillowGame.WillowItem", Some(inventory), vec![]).unwrap();
    vm.space.classes.define("Test.ToyScriptWeapon", Some(weapon), vec![]).unwrap();
    for c in [inventory, weapon, item] {
        vm.space.classes.mark_native(c).unwrap();
    }
    let props = vec![decl(&mut vm, "DlcItemSet", o.clone())];
    vm.space.classes.define("WillowGame.WeaponBalanceDefinition", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "DlcItemSet", PropKind::Int)];
    vm.space.classes.define("Test.MistypedBalance", Some(object), props).unwrap();
    let expression = vm.space.classes.define("GearboxFramework.AttributeExpression", Some(object), vec![]).unwrap();
    let declare = |vm: &mut Vm, path: &str, key: &str, owner: ClassId, params: Vec<PropertyDecl>, result: Option<PropertyDecl>| {
        let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key).with_owner(owner).with_params(params);
        d.result = result;
        vm.declare_function(d).unwrap()
    };
    let p = vec![param(&mut vm, "bSilent", PropKind::Bool)];
    let update = declare(&mut vm, "WillowGame.WillowPlayerController.UpdateAmmoCounts", UPDATE_AMMO_COUNTS, pc, p, None);
    let p = vec![param(&mut vm, "AmmoType", PropKind::Int), param(&mut vm, "Amount", PropKind::Int)];
    let announce = declare(&mut vm, "WillowGame.WillowPlayerController.ScriptAnnounceAmmoGain", ANNOUNCE, pc, p, None);
    vm.declare_method(pc, "ScriptAnnounceAmmoGain", None, announce).unwrap();
    let ret = decl(&mut vm, "ReturnValue", PropKind::Struct(rotator));
    let save_rotation = declare(&mut vm, "WillowGame.WillowPickup.GetSaveRotation", GET_SAVE_ROTATION, pickup, vec![], Some(ret));
    let p = vec![param(&mut vm, "Other", o.clone())];
    let ret = decl(&mut vm, "ReturnValue", PropKind::Bool);
    let dlc = declare(&mut vm, "WillowGame.WillowInventory.IsDLCRequirementMet", IS_DLC_REQUIREMENT_MET, inventory, p, Some(ret));
    let ret = decl(&mut vm, "ReturnValue", o.clone());
    let context =
        declare(&mut vm, "WillowGame.WillowWeapon.GetResourcePoolContext", GET_RESOURCE_POOL_CONTEXT, weapon, vec![], Some(ret));
    let p = vec![
        param(&mut vm, "Mode", PropKind::Byte { enum_: None }),
        param(&mut vm, "ContextSource", o.clone()),
        param(&mut vm, "Expressions", PropKind::Array(Box::new(PropKind::Int))),
        param(&mut vm, "OptionalOverrideContextSource", o.clone()),
    ];
    let ret = decl(&mut vm, "ReturnValue", PropKind::Bool);
    let evaluate =
        declare(&mut vm, "GearboxFramework.AttributeExpression.EvaluateExpressions", EVALUATE_EXPRESSIONS, expression, p, Some(ret));
    let events = Rc::new(RefCell::new(Vec::new()));
    if mode == ParityMode::Off {
        register(&mut vm).unwrap();
        let seen = events.clone();
        vm.register_native(
            ANNOUNCE,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let (Value::Int(kind), Value::Int(gain)) = (c.arg(0)?.clone(), c.arg(1)?.clone()) else { panic!("ints") };
                seen.borrow_mut().push((kind, gain));
                Ok(Value::None)
            }),
        )
        .unwrap();
    } else {
        crate::pickup::register(&mut vm).unwrap();
    }
    Fx { vm, update, save_rotation, dlc, context, evaluate, rotator, data, events }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn make(&mut self, path: &str) -> ObjRef {
        let class = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc(class, "Toy", None, Origin::Runtime).unwrap()
    }

    /// A slotless stand-in of the class at `path`: an object the store has not loaded.
    fn stand_in(&mut self, path: &str) -> ObjRef {
        let class = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc_stand_in(class, "Lazy", Origin::Default { class }).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    /// A globals definition holding a fresh ammo definition in each of the eight fields.
    fn definition(&mut self) -> (ObjRef, Vec<ObjRef>) {
        let d = self.make("WillowGame.GlobalsDefinition");
        let mut ammo = Vec::new();
        for (field, _, _) in AMMO {
            let a = self.make("Test.AmmoDefinition");
            self.set(d, field, Value::Object(Some(a)));
            ammo.push(a);
        }
        (d, ammo)
    }

    /// The world's globals holding `definition`.
    fn globals(&mut self, definition: ObjRef) {
        let g = self.make("GearboxFramework.GearboxGlobals");
        self.set(g, "TheGlobalsDefinition", Value::Object(Some(definition)));
        let q = ow_world::query::install(&mut self.vm).unwrap();
        q.borrow_mut().set_globals(g);
    }

    fn caches(&self, pc: ObjRef) -> Vec<Value> {
        AMMO.iter().map(|(_, cache, _)| self.get(pc, cache)).collect()
    }

    /// [`update_ammo`] with a toy lookup answering `values` by ammo definition, recording its calls.
    fn update(&mut self, pc: ObjRef, d: ObjRef, silent: bool, values: &[(ObjRef, f32)]) -> (Result<(), VmError>, Vec<ObjRef>) {
        let mut asked = Vec::new();
        let mut lookup = |_: &mut Vm, controller: ObjRef, ammo: ObjRef| -> Result<f32, VmError> {
            assert_eq!(controller, pc);
            asked.push(ammo);
            Ok(values.iter().find(|(a, _)| *a == ammo).map_or(0.0, |(_, v)| *v))
        };
        let r = update_ammo(&mut self.vm, pc, d, silent, &mut lookup);
        (r, asked)
    }

    fn rotation(&self, pitch: i32) -> Value {
        Value::structure(self.rotator, vec![Value::Int(pitch), Value::Int(2), Value::Int(3)])
    }

    /// A `WillowPickup` with `Physics` and a `CollisionComponent` when `collision`.
    fn pickup(&mut self, physics: u8, collision: bool) -> ObjRef {
        let p = self.make("WillowGame.WillowPickup");
        let r = self.rotation(17);
        self.set(p, "Rotation", r);
        self.set(p, "Physics", Value::Byte(physics));
        if collision {
            let c = self.make("Core.Object");
            self.set(p, "CollisionComponent", Value::Object(Some(c)));
        }
        p
    }

    /// `GetSaveRotation` on `p`; reads only, whatever it answers.
    fn save_rotation(&mut self, p: ObjRef) -> Result<Value, VmError> {
        let before = self.marks();
        let r = self.vm.call(self.save_rotation, Some(p), vec![]).map(|o| o.ret);
        assert_eq!(self.marks(), before, "GetSaveRotation writes, loads and interns nothing");
        r
    }

    /// A weapon of the class at `path` whose `DefinitionData.BalanceDefinition` is `balance`.
    fn weapon(&mut self, path: &str, balance: Option<ObjRef>) -> ObjRef {
        let w = self.make(path);
        let data = Value::structure(self.data, vec![Value::Object(balance)]);
        self.set(w, "DefinitionData", data);
        w
    }

    /// `IsDLCRequirementMet` on `w`; reads only, whatever it answers.
    fn dlc(&mut self, w: ObjRef) -> Result<Value, VmError> {
        let before = self.marks();
        let r = self.vm.call(self.dlc, Some(w), vec![Value::Object(None)]).map(|o| o.ret);
        assert_eq!(self.marks(), before, "IsDLCRequirementMet writes, loads and interns nothing");
        r
    }

    fn context(&mut self, w: ObjRef) -> Value {
        let before = self.marks();
        let r = self.vm.call(self.context, Some(w), vec![]).unwrap().ret;
        assert_eq!(self.marks(), before, "GetResourcePoolContext writes nothing");
        r
    }

    fn evaluate(&mut self, mode: u8) -> Result<Value, VmError> {
        let args = vec![Value::Byte(mode), Value::Object(None), Value::array(vec![]), Value::Object(None)];
        self.vm.call(self.evaluate, None, args).map(|o| o.ret)
    }
}

fn not_read(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn script(e: VmError) {
    assert!(matches!(e, VmError::Script { .. }), "expected a data error, got {e:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// UpdateAmmoCounts
// ---------------------------------------------------------------------------------------------------------------

/// 3.1: the eight lookups in the native's order; each changed value written to its cache; a rise, not silent, is
/// `ScriptAnnounceAmmoGain(kind, gain)` with the gain truncated; a fall writes the cache with no event; an unchanged
/// value writes nothing.
#[test]
fn update_ammo_counts_refreshes_the_eight_caches_in_order() {
    let mut x = fx();
    let pc = x.make("WillowGame.WillowPlayerController");
    let (d, ammo) = x.definition();
    x.set(pc, AMMO[2].1, Value::Float(9.0));
    x.set(pc, AMMO[3].1, Value::Float(4.0));
    let values = [(ammo[0], 12.9), (ammo[2], 5.0), (ammo[3], 4.0), (ammo[7], 2.0)];
    let (r, asked) = x.update(pc, d, false, &values);
    r.unwrap();
    assert_eq!(asked, ammo, "every set definition, in AMMO's order");
    let want: Vec<Value> = [12.9, 0.0, 5.0, 4.0, 0.0, 0.0, 0.0, 2.0].iter().map(|v| Value::Float(*v)).collect();
    assert_eq!(x.caches(pc), want);
    assert_eq!(*x.events.borrow(), vec![(3, 12), (7, 2)], "the rises only, the kinds the note's");
    // Unchanged values: no write at all.
    let before = x.marks();
    let (r, _) = x.update(pc, d, false, &values);
    r.unwrap();
    assert_eq!(x.marks(), before, "nothing changed, nothing written");
    assert_eq!(x.events.borrow().len(), 2);
}

/// 3.1: `bSilent` writes the caches and raises no event.
#[test]
fn a_silent_update_announces_nothing() {
    let mut x = fx();
    let pc = x.make("WillowGame.WillowPlayerController");
    let (d, ammo) = x.definition();
    let (r, _) = x.update(pc, d, true, &[(ammo[1], 30.0), (ammo[6], 3.0)]);
    r.unwrap();
    assert_eq!(x.get(pc, AMMO[1].1), Value::Float(30.0));
    assert_eq!(x.get(pc, AMMO[6].1), Value::Float(3.0));
    assert!(x.events.borrow().is_empty());
}

/// 3.1: a None definition field is skipped (no lookup, no cache).
#[test]
fn a_none_definition_is_skipped() {
    let mut x = fx();
    let pc = x.make("WillowGame.WillowPlayerController");
    let (d, ammo) = x.definition();
    x.set(d, AMMO[4].0, Value::Object(None));
    x.set(pc, AMMO[4].1, Value::Float(8.0));
    let (r, asked) = x.update(pc, d, false, &[]);
    r.unwrap();
    assert_eq!(asked.len(), 7);
    assert!(!asked.contains(&ammo[4]));
    assert_eq!(x.get(pc, AMMO[4].1), Value::Float(8.0), "its cache untouched");
}

/// 3.1's preflight: an ammo definition the store has not loaded is the `NotRead`, before any lookup, with nothing
/// written, loaded or interned; so is a controller whose pawn is a vehicle (the lookups' own refusal).
#[test]
fn the_preflight_refuses_before_any_write() {
    let mut x = fx();
    let pc = x.make("WillowGame.WillowPlayerController");
    let (d, _) = x.definition();
    let lazy = x.stand_in("Test.AmmoDefinition");
    x.set(d, AMMO[5].0, Value::Object(Some(lazy)));
    let before = x.marks();
    let (r, asked) = x.update(pc, d, false, &[]);
    assert_eq!(not_read(r.unwrap_err()), AMMO_NOT_LOADED);
    assert!(asked.is_empty(), "no lookup ran");
    assert_eq!(x.marks(), before);
    // The vehicle.
    let (d, _) = x.definition();
    let vehicle = x.make("Engine.Vehicle");
    x.set(pc, "Pawn", Value::Object(Some(vehicle)));
    let before = x.marks();
    let (r, asked) = x.update(pc, d, false, &[]);
    assert_eq!(not_read(r.unwrap_err()), VEHICLE_POOL);
    assert!(asked.is_empty());
    assert_eq!(x.marks(), before);
    // A pawn on foot is not refused.
    let pawn = x.make("Engine.Pawn");
    x.set(pc, "Pawn", Value::Object(Some(pawn)));
    x.update(pc, d, false, &[]).0.unwrap();
}

/// 3.1: a cache declared as other than a float, or a definition class missing a field, is a data error before any
/// lookup.
#[test]
fn mistyped_declarations_are_data_errors() {
    let mut x = fx();
    let pc = x.make("Test.MistypedController");
    let (d, _) = x.definition();
    let before = x.marks();
    let (r, asked) = x.update(pc, d, false, &[]);
    script(r.unwrap_err());
    assert!(asked.is_empty());
    assert_eq!(x.marks(), before);
    let pc = x.make("WillowGame.WillowPlayerController");
    let bare = x.make("WillowGame.WillowPlayerController");
    let (r, _) = x.update(pc, bare, false, &[]);
    script(r.unwrap_err());
}

/// The hub's correction on #785: a lookup refusal after a cache write carries the partial-writes prefix; the first
/// lookup's refusal, before any write, does not.
#[test]
fn a_later_lookup_refusal_is_after_partial_writes() {
    for (fail_on, marked) in [(2usize, true), (1, false)] {
        let mut x = fx();
        let pc = x.make("WillowGame.WillowPlayerController");
        let (d, _) = x.definition();
        let mut calls = 0;
        let mut lookup = |_: &mut Vm, _: ObjRef, _: ObjRef| -> Result<f32, VmError> {
            calls += 1;
            if calls == fail_on {
                return Err(NotYet::not_read("a toy pool refusal", "TOY.md").into());
            }
            Ok(1.0)
        };
        let before = x.marks();
        match update_ammo(&mut x.vm, pc, d, true, &mut lookup) {
            Err(VmError::NotYet(n)) => {
                assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY.md"), "{n}");
                assert_eq!(ow_core::notyet::is_partial_writes(&n.what), marked, "fail on lookup {fail_on}: {}", n.what);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(x.marks() == before, !marked);
    }
}

/// 3.1 through the bound native: no world globals is nothing; with the globals, an unloaded ammo definition is the
/// `NotRead`, nothing written.
#[test]
fn the_native_reads_the_world_s_globals() {
    let mut x = fx();
    let pc = x.make("WillowGame.WillowPlayerController");
    let before = x.marks();
    x.vm.call(x.update, Some(pc), vec![Value::Bool(false)]).unwrap();
    assert_eq!(x.marks(), before, "no globals: nothing");
    let (d, _) = x.definition();
    let lazy = x.stand_in("Test.AmmoDefinition");
    x.set(d, AMMO[0].0, Value::Object(Some(lazy)));
    x.globals(d);
    let before = x.marks();
    let e = x.vm.call(x.update, Some(pc), vec![Value::Bool(false)]).unwrap_err();
    assert_eq!(not_read(e), AMMO_NOT_LOADED);
    assert_eq!(x.marks(), before);
}

// ---------------------------------------------------------------------------------------------------------------
// GetSaveRotation
// ---------------------------------------------------------------------------------------------------------------

/// NATIVE_PICKUP_SAVE_ROTATION.md section 5: not rigid body, or a rigid body with no `CollisionComponent`: the
/// actor's `Rotation`; a rigid body with one: the body pose's `LayerAbsent` (the default).
#[test]
fn get_save_rotation_answers_the_rotation_or_the_body_pose_s_layer() {
    let mut x = fx();
    let want = x.rotation(17);
    for (physics, collision) in [(0, true), (1, true), (RIGID, false)] {
        let p = x.pickup(physics, collision);
        assert_eq!(x.save_rotation(p).unwrap(), want, "physics {physics}, collision {collision}");
    }
    let p = x.pickup(RIGID, true);
    match x.save_rotation(p).unwrap_err() {
        VmError::NotYet(n) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert_eq!((n.what.as_ref(), n.record), (BODY_POSE, RECORD_SAVE_ROTATION));
        }
        other => panic!("{other:?}"),
    }
}

/// The hub's at-rest ruling: with the harness setting on, the rigid body with a `CollisionComponent` answers the
/// actor's `Rotation` and logs the stand-in line; Reference refuses the setting.
#[test]
fn the_at_rest_ruling_answers_the_rotation_with_its_line() {
    let mut x = fx();
    enable_at_rest_body_pose(&mut x.vm).unwrap();
    assert!(enable_at_rest_body_pose(&mut x.vm).is_err(), "turned on once");
    let p = x.pickup(RIGID, true);
    let logged = x.vm.log.len();
    assert_eq!(x.save_rotation(p).unwrap(), x.rotation(17));
    assert_eq!(x.vm.log[logged..], [AT_REST_LINE.to_string()]);
    // Not rigid: no stand-in line.
    let p = x.pickup(0, true);
    x.save_rotation(p).unwrap();
    assert_eq!(x.vm.log.len(), logged + 1);
    let mut r = fx_in(ParityMode::Reference);
    assert!(matches!(enable_at_rest_body_pose(&mut r.vm), Err(VmError::Registration { .. })));
}

// ---------------------------------------------------------------------------------------------------------------
// IsDLCRequirementMet
// ---------------------------------------------------------------------------------------------------------------

/// 3.6: no balance, or a balance with no `DlcItemSet`: true (through a script subclass of the weapon too); a set
/// item set is its `NotRead`; an unloaded balance its `NotRead`; another first native class names it.
#[test]
fn is_dlc_requirement_met_s_cases() {
    let mut x = fx();
    let w = x.weapon("WillowGame.WillowWeapon", None);
    assert_eq!(x.dlc(w).unwrap(), Value::Bool(true));
    let balance = x.make("WillowGame.WeaponBalanceDefinition");
    let w = x.weapon("Test.ToyScriptWeapon", Some(balance));
    assert_eq!(x.dlc(w).unwrap(), Value::Bool(true));
    let set = x.make("Core.Object");
    x.set(balance, "DlcItemSet", Value::Object(Some(set)));
    assert_eq!(not_read(x.dlc(w).unwrap_err()), DLC_ITEM_SET);
    let lazy = x.stand_in("WillowGame.WeaponBalanceDefinition");
    let w = x.weapon("WillowGame.WillowWeapon", Some(lazy));
    assert_eq!(not_read(x.dlc(w).unwrap_err()), BALANCE_NOT_LOADED);
    let item = x.make("WillowGame.WillowItem");
    assert_eq!(not_read(x.dlc(item).unwrap_err()), format!("{OTHER_INVENTORY} WillowGame.WillowItem"));
}

/// 3.6 and the hub's correction on #785: a `DlcItemSet` declared as other than an object is a data error.
#[test]
fn is_dlc_requirement_met_checks_the_declarations() {
    let mut x = fx();
    let balance = x.make("Test.MistypedBalance");
    let w = x.weapon("WillowGame.WillowWeapon", Some(balance));
    script(x.dlc(w).unwrap_err());
}

// ---------------------------------------------------------------------------------------------------------------
// GetResourcePoolContext and EvaluateExpressions
// ---------------------------------------------------------------------------------------------------------------

/// 3.8: the instigator's controller, else its driven vehicle's controller, else None.
#[test]
fn get_resource_pool_context_s_cases() {
    let mut x = fx();
    let w = x.weapon("WillowGame.WillowWeapon", None);
    assert_eq!(x.context(w), Value::Object(None), "no instigator");
    let pawn = x.make("Engine.Pawn");
    x.set(w, "Instigator", Value::Object(Some(pawn)));
    assert_eq!(x.context(w), Value::Object(None), "no controller, no vehicle");
    let vehicle = x.make("Engine.Vehicle");
    let driver = x.make("Engine.Controller");
    x.set(vehicle, "Controller", Value::Object(Some(driver)));
    x.set(pawn, "DrivenVehicle", Value::Object(Some(vehicle)));
    assert_eq!(x.context(w), Value::Object(Some(driver)), "the vehicle's controller");
    let own = x.make("Engine.Controller");
    x.set(pawn, "Controller", Value::Object(Some(own)));
    assert_eq!(x.context(w), Value::Object(Some(own)), "the instigator's own first");
}

/// 4.2: an empty list holds in both modes; a mode outside the enum is a data error.
#[test]
fn evaluate_expressions_s_modes() {
    let mut x = fx();
    assert_eq!(x.evaluate(0).unwrap(), Value::Bool(true));
    assert_eq!(x.evaluate(1).unwrap(), Value::Bool(true));
    script(x.evaluate(2).unwrap_err());
}

/// Reference binds none of the five keys (the stub policy answers).
#[test]
fn reference_binds_nothing() {
    let x = fx_in(ParityMode::Reference);
    for key in [UPDATE_AMMO_COUNTS, GET_SAVE_ROTATION, IS_DLC_REQUIREMENT_MET, GET_RESOURCE_POOL_CONTEXT, EVALUATE_EXPRESSIONS] {
        assert!(!x.vm.has_native(key), "{key}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Review r1 of #789: the LOW test gaps
// ---------------------------------------------------------------------------------------------------------------

/// A world for `EvaluateExpressions` over real expressions (the name parts' attribute fixture, cut down): a weapon
/// whose float stack attribute `Bonus` is 5, an attribute reading it through the weapon context resolver, and the
/// expression struct. Answers the VM, the weapon, the attribute and the struct.
fn expr_world() -> (Vm, ObjRef, ObjRef, StructId) {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let d = |space: &mut ObjectSpace, name: &str, kind: PropKind| PropertyDecl::new(space.names.intern(name), kind);
    let o = PropKind::Object { class: object };
    let objs = PropKind::Array(Box::new(o.clone()));
    let byte = PropKind::Byte { enum_: None };
    let decls = vec![d(&mut space, "ContextResolverChain", objs.clone()), d(&mut space, "ValueResolverChain", objs)];
    let base = space.classes.define("Engine.AttributeDefinitionBase", Some(object), decls).unwrap();
    let attribute = space.classes.define("Engine.AttributeDefinition", Some(base), vec![]).unwrap();
    let decls = vec![d(&mut space, "PropertyName", PropKind::Name)];
    let value_resolver = space.classes.define("Engine.ObjectPropertyAttributeValueResolver", Some(object), decls).unwrap();
    let context_resolver = space.classes.define("Engine.WeaponAttributeContextResolver", Some(object), vec![]).unwrap();
    let fields = vec![
        d(&mut space, "AttributeOperand1", o.clone()),
        d(&mut space, "ComparisonOperator", byte.clone()),
        d(&mut space, "Operand2Usage", byte.clone()),
        d(&mut space, "AttributeOperand2", o.clone()),
        d(&mut space, "ConstantOperand2", PropKind::Float),
    ];
    let expr = space.classes.define_struct("Test.AttributeExpressionData", None, fields).unwrap();
    let actor = space.classes.define("Engine.Actor", Some(object), vec![]).unwrap();
    let engine_weapon = space.classes.define("Engine.Weapon", Some(actor), vec![]).unwrap();
    let decls_m = vec![d(&mut space, "Type", byte), d(&mut space, "Value", PropKind::Float)];
    let modifier = space.classes.define("Core.AttributeModifier", Some(object), decls_m).unwrap();
    let decls = vec![
        d(&mut space, "Bonus", PropKind::FloatAttribute).with_flags(PropertyDecl::STACK_ATTRIBUTE),
        d(&mut space, "BonusBaseValue", PropKind::FloatAttribute).with_flags(PropertyDecl::PLAIN_ATTRIBUTE),
        d(&mut space, "BonusModifierStack", PropKind::Array(Box::new(PropKind::Object { class: modifier }))),
    ];
    let weapon_class = space.classes.define("WillowGame.WillowWeapon", Some(engine_weapon), decls).unwrap();
    let (v, b, k) = (space.names.intern("Bonus"), space.names.intern("BonusBaseValue"), space.names.intern("BonusModifierStack"));
    space.classes.link_attribute(weapon_class, v, b, k).unwrap();
    let alloc = |space: &mut ObjectSpace, c: ClassId, n: &str| space.alloc(c, n, None, Origin::Runtime).unwrap();
    let weapon = alloc(&mut space, weapon_class, "Weapon_0");
    let c = alloc(&mut space, context_resolver, "Context_0");
    let r = alloc(&mut space, value_resolver, "Value_0");
    let a = alloc(&mut space, attribute, "Bonus_0");
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::standard().unwrap()), ParityMode::Off);
    let put = |vm: &mut Vm, o: ObjRef, name: &str, v: Value| {
        let p = vm.space.prop(vm.space.heap.get(o).unwrap().class, name).unwrap();
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    };
    put(&mut vm, weapon, "Bonus", Value::Float(5.0));
    put(&mut vm, weapon, "BonusBaseValue", Value::Float(5.0));
    let name = Value::Name(vm.space.names.intern("Bonus"));
    put(&mut vm, r, "PropertyName", name);
    put(&mut vm, a, "ContextResolverChain", Value::array(vec![Value::Object(Some(c))]));
    put(&mut vm, a, "ValueResolverChain", Value::array(vec![Value::Object(Some(r))]));
    (vm, weapon, a, expr)
}

/// `attribute > constant` as a script row.
fn above(vm: &Vm, ty: StructId, attribute: ObjRef, constant: f32) -> Value {
    let mut v = vm.space.classes.struct_defaults(ty).unwrap();
    let layout = vm.space.classes.struct_layout(ty).unwrap();
    let at = |name: &str| layout.field(vm.space.names.find(name).unwrap()).unwrap().0 as usize;
    let (a, c, k) = (at("AttributeOperand1"), at("ComparisonOperator"), at("ConstantOperand2"));
    let s = v.struct_mut().unwrap();
    s.fields[a] = Value::Object(Some(attribute));
    s.fields[c] = Value::Byte(4);
    s.fields[k] = Value::Float(constant);
    v
}

/// Note 6.10: And and Or over two expressions on the weapon's `Bonus` (5): one holding (> 3), one not (> 7).
#[test]
fn evaluate_expressions_and_or_over_two_expressions() {
    let (mut vm, weapon, bonus, ty) = expr_world();
    let (yes, no) = (above(&vm, ty, bonus, 3.0), above(&vm, ty, bonus, 7.0));
    let both = [yes.clone(), no.clone()];
    let mut run = |mode, rows: &[Value]| expressions(&mut vm, mode, Some(weapon), rows, None).unwrap();
    assert!(!run(ExprMode::And, &both), "And: one fails");
    assert!(run(ExprMode::Or, &both), "Or: one holds");
    assert!(run(ExprMode::And, &[yes.clone(), yes]), "And: both hold");
    assert!(!run(ExprMode::Or, &[no.clone(), no]), "Or: neither holds");
}

/// A toy pickup class under `Core.Object` declaring `Rotation`, `Physics` and `CollisionComponent` with these kinds.
fn toy_pickup(x: &mut Fx, path: &str, rotation: PropKind, physics: PropKind) -> ObjRef {
    let object = x.vm.space.classes.by_path("Core.Object").unwrap();
    let decls = vec![
        PropertyDecl::new(x.vm.space.names.intern("Rotation"), rotation),
        PropertyDecl::new(x.vm.space.names.intern("Physics"), physics),
        PropertyDecl::new(x.vm.space.names.intern("CollisionComponent"), PropKind::Object { class: object }),
    ];
    x.vm.space.classes.define(path, Some(object), decls).unwrap();
    x.make(path)
}

/// NATIVE_PICKUP_SAVE_ROTATION.md section 6, test 5: an `EPhysics` enum with no `PHYS_RigidBody` is a data error,
/// with no class-code load (the helper's marks).
#[test]
fn get_save_rotation_needs_phys_rigid_body_by_name() {
    let mut x = fx();
    let names = ["PHYS_None", "PHYS_Walking"].iter().map(|n| x.vm.space.names.intern(n)).collect();
    let short = x.vm.space.classes.define_enum("Test.ShortPhysicsPickup.EShortPhysics", names).unwrap();
    let rotator = x.rotator;
    let p = toy_pickup(&mut x, "Test.ShortPhysicsPickup", PropKind::Struct(rotator), PropKind::Byte { enum_: Some(short) });
    script(x.save_rotation(p).unwrap_err());
}

/// Section 6, test 6: `Physics` or `Rotation` of the wrong kind is a data error.
#[test]
fn get_save_rotation_needs_the_kinds() {
    let mut x = fx();
    let actor = x.vm.space.classes.by_path("Engine.Actor").unwrap();
    let physics = x.vm.space.classes.decl(x.vm.space.prop(actor, "Physics").unwrap()).unwrap().kind.clone();
    let rotator = x.rotator;
    let p = toy_pickup(&mut x, "Test.IntPhysicsPickup", PropKind::Struct(rotator), PropKind::Int);
    script(x.save_rotation(p).unwrap_err());
    let p = toy_pickup(&mut x, "Test.IntRotationPickup", PropKind::Int, physics);
    script(x.save_rotation(p).unwrap_err());
}

/// Review r1 of #789: with an existing manager, a world without the pool-map service is the lookup's `LayerAbsent`,
/// raised by the preflight before any lookup or write (this fixture registers no pool pack).
#[test]
fn a_missing_pool_map_is_refused_by_the_preflight() {
    let mut x = fx();
    let pc = x.make("WillowGame.WillowPlayerController");
    let manager = x.make("Core.Object");
    x.set(pc, "ResourcePoolManager", Value::Object(Some(manager)));
    let (d, _) = x.definition();
    let before = x.marks();
    let (r, asked) = x.update(pc, d, false, &[]);
    match r.unwrap_err() {
        VmError::NotYet(n) => assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}"),
        other => panic!("{other:?}"),
    }
    assert!(asked.is_empty(), "no lookup ran");
    assert_eq!(x.marks(), before);
}
