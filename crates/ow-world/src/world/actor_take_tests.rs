//! NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md sections 3.3, 3.4, 3.5 and 3.7 on invented classes (AGENTS.md rule 9:
//! invented objects and values, the class and field names the packages'). Every refusal before a write leaves #518's
//! counters, `class_code_loads` and the name table unchanged.

use std::cell::Cell;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{Arg, FunctionDecl, NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;

const GAINED_CHILD: &str = "Test.ToyOwner.GainedChild(object)";

struct Fx {
    vm: Vm,
    list: FuncId,
    noise: FuncId,
    vehicle: FuncId,
    set_owner: FuncId,
    /// What the toy owner's `GainedChild` does: flip this component's `bOwnerNoSee` on, or move the owner on.
    flip: Rc<Cell<Option<ObjRef>>>,
    reown: Rc<Cell<Option<ObjRef>>>,
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
    let objects = PropKind::Array(Box::new(o.clone()));
    let props = vec![
        decl(&mut vm, "Owner", o.clone()),
        decl(&mut vm, "bDeleteMe", PropKind::Bool),
        decl(&mut vm, "Components", objects.clone()),
        decl(&mut vm, "AllComponents", objects),
        decl(&mut vm, "bOnlyOwnerSee", PropKind::Bool),
        decl(&mut vm, "bNetDirty", PropKind::Bool),
    ];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), props).unwrap();
    let owner_class = vm.space.classes.define("Test.ToyOwner", Some(actor), vec![]).unwrap();
    let props = vec![decl(&mut vm, "Pawn", o.clone())];
    let controller = vm.space.classes.define("Engine.Controller", Some(actor), props).unwrap();
    let props = vec![decl(&mut vm, "DrivenVehicle", o.clone())];
    let pawn = vm.space.classes.define("Engine.Pawn", Some(actor), props).unwrap();
    let willow = vm.space.classes.define("WillowGame.WillowPawn", Some(pawn), vec![]).unwrap();
    vm.space.classes.define("Test.ToyScriptPawn", Some(willow), vec![]).unwrap();
    let vehicle = vm.space.classes.define("Engine.Vehicle", Some(pawn), vec![]).unwrap();
    for c in [pawn, willow, vehicle] {
        vm.space.classes.mark_native(c).unwrap();
    }
    let dummy = vec![decl(&mut vm, "Dummy", PropKind::Int)];
    let scene = vm.space.classes.define_struct("Engine.ActorComponent.ToyScene", None, dummy).unwrap();
    let props = vec![
        decl(&mut vm, "Owner", PropKind::Object { class: actor }),
        decl(&mut vm, "Scene", PropKind::Struct(scene)),
        decl(&mut vm, "bNeedsReattach", PropKind::Bool),
    ];
    let base = vm.space.classes.define("Engine.ActorComponent", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "bOnlyOwnerSee", PropKind::Bool), decl(&mut vm, "bOwnerNoSee", PropKind::Bool)];
    let primitive = vm.space.classes.define("Engine.PrimitiveComponent", Some(base), props).unwrap();
    vm.space.classes.define("Engine.SkeletalMeshComponent", Some(primitive), vec![]).unwrap();
    vm.space.classes.define("Engine.ToyLightComponent", Some(base), vec![]).unwrap();
    let declare = |vm: &mut Vm, path: &str, key: &str, owner: ClassId, params: Vec<PropertyDecl>, result: Option<PropertyDecl>| {
        let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key).with_owner(owner).with_params(params);
        d.result = result;
        vm.declare_function(d).unwrap()
    };
    let p = vec![
        param(&mut vm, "BaseClass", PropKind::Class { meta: object }),
        param(&mut vm, "Component", PropKind::Object { class: base }).with_flags(PropertyDecl::PARAM | PropertyDecl::OUT),
    ];
    let list = declare(&mut vm, "Engine.Actor.ComponentList", COMPONENT_LIST, actor, p, None);
    let p = vec![param(&mut vm, "Loudness", PropKind::Float), param(&mut vm, "NoiseType", PropKind::Name)];
    let noise = declare(&mut vm, "Engine.Actor.MakeNoise", MAKE_NOISE, actor, p, None);
    let p = vec![param(&mut vm, "bCheckDrivenVehicle", PropKind::Bool)];
    let ret = decl(&mut vm, "ReturnValue", PropKind::Bool);
    let vehicle_fn = declare(&mut vm, "Engine.Controller.IsUsingVehicle", IS_USING_VEHICLE, controller, p, Some(ret));
    let p = vec![param(&mut vm, "NewOwner", PropKind::Object { class: actor })];
    let set_owner = declare(&mut vm, "Engine.Actor.SetOwner", SET_OWNER, actor, p, None);
    let p = vec![param(&mut vm, "Other", PropKind::Object { class: actor })];
    let gained = declare(&mut vm, "Test.ToyOwner.GainedChild", GAINED_CHILD, owner_class, p, None);
    vm.declare_method(owner_class, "GainedChild", None, gained).unwrap();
    let (flip, reown) = (Rc::new(Cell::new(None::<ObjRef>)), Rc::new(Cell::new(None::<ObjRef>)));
    if mode == ParityMode::Off {
        register(&mut vm).unwrap();
        let (f, r) = (flip.clone(), reown.clone());
        vm.register_native(
            GAINED_CHILD,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let Value::Object(Some(child)) = c.arg(0)?.clone() else { panic!("child") };
                if let Some(component) = f.get() {
                    let p = c.vm.space.prop(c.vm.space.heap.get(component)?.class, "bOwnerNoSee").unwrap();
                    c.vm.space.write(component, PropAddr::scalar(p), Value::Bool(true))?;
                }
                if let Some(other) = r.get() {
                    let p = c.vm.space.prop(c.vm.space.heap.get(child)?.class, "Owner").unwrap();
                    c.vm.space.write(child, PropAddr::scalar(p), Value::Object(Some(other)))?;
                }
                Ok(Value::None)
            }),
        )
        .unwrap();
    }
    Fx { vm, list, noise, vehicle: vehicle_fn, set_owner, flip, reown }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn make(&mut self, path: &str) -> ObjRef {
        let class = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc(class, "Toy", None, Origin::Runtime).unwrap()
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

    fn list_of(&mut self, o: ObjRef, name: &str, items: &[Option<ObjRef>]) {
        self.set(o, name, Value::array(items.iter().map(|c| Value::Object(*c)).collect()));
    }

    /// `ComponentList` on `actor` with `base` (a class path or None), as a `foreach` starts it.
    fn component_list(&mut self, actor: ObjRef, base: Option<&str>) -> ow_vm::api::IteratorResult {
        let base = base.map(|p| self.vm.space.classes.by_path(p).unwrap());
        let it = self.vm.iterator_for(self.list).expect("bound in Off");
        let args = vec![Arg::new(Value::Class(base), false, true), Arg::new(Value::Object(None), true, true)];
        let before = self.marks();
        let r = it(&mut NativeCall::new(&mut self.vm, self.list, Some(actor), args)).unwrap();
        assert_eq!(self.marks(), before, "ComponentList writes nothing");
        r
    }

    fn vehicle(&mut self, controller: ObjRef, check: bool) -> Result<Value, VmError> {
        let before = self.marks();
        let r = self.vm.call(self.vehicle, Some(controller), vec![Value::Bool(check)]).map(|o| o.ret);
        assert_eq!(self.marks(), before, "IsUsingVehicle writes, loads and interns nothing");
        r
    }

    fn set_owner(&mut self, actor: ObjRef, owner: Option<ObjRef>) -> Result<(), VmError> {
        self.vm.call(self.set_owner, Some(actor), vec![Value::Object(owner)]).map(|_| ())
    }

    /// A primitive component with its two owner-visibility flags.
    fn primitive(&mut self, only_owner: bool, owner_no: bool) -> ObjRef {
        let c = self.make("Engine.SkeletalMeshComponent");
        self.set(c, "bOnlyOwnerSee", Value::Bool(only_owner));
        self.set(c, "bOwnerNoSee", Value::Bool(owner_no));
        c
    }
}

fn not_read(e: VmError) -> (String, bool) {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            (n.what.to_string(), ow_core::notyet::is_partial_writes(&n.what))
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// ComponentList and MakeNoise
// ---------------------------------------------------------------------------------------------------------------

/// 3.3: a None class iterates nothing; otherwise the non-None entries of `Components` at or below the class, in array
/// order (a subclass counts; another branch does not).
#[test]
fn component_list_filters_components_by_class_in_order() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let (a, light, b) = (x.primitive(false, false), x.make("Engine.ToyLightComponent"), x.primitive(false, false));
    x.list_of(actor, "Components", &[Some(a), None, Some(light), Some(b)]);
    assert!(x.component_list(actor, None).items.is_empty());
    let items = x.component_list(actor, Some("Engine.PrimitiveComponent")).items;
    assert_eq!(items, vec![Value::Object(Some(a)), Value::Object(Some(b))]);
    let items = x.component_list(actor, Some("Engine.ActorComponent")).items;
    assert_eq!(items, vec![Value::Object(Some(a)), Value::Object(Some(light)), Value::Object(Some(b))]);
}

/// The hub's correction on #785: the guard passes while `Components` is unchanged; a body that changed it is the
/// prefixed `NotRead`.
#[test]
fn component_list_s_guard_refuses_a_changed_array() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let a = x.primitive(false, false);
    x.list_of(actor, "Components", &[Some(a)]);
    let r = x.component_list(actor, Some("Engine.PrimitiveComponent"));
    let guard = r.guard.clone().expect("a guard");
    guard(&x.vm).unwrap();
    let b = x.primitive(false, false);
    x.list_of(actor, "Components", &[Some(a), Some(b)]);
    let (what, marked) = not_read(guard(&x.vm).unwrap_err());
    assert!(marked, "the body wrote");
    assert!(what.ends_with(COMPONENTS_CHANGED), "{what}");
}

/// 3.4: `MakeNoise` does nothing.
#[test]
fn make_noise_is_nothing() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let noise = x.vm.space.names.intern("Toy");
    let before = x.marks();
    x.vm.call(x.noise, Some(actor), vec![Value::Float(1.0), Value::Name(noise)]).unwrap();
    assert_eq!(x.marks(), before);
}

// ---------------------------------------------------------------------------------------------------------------
// IsUsingVehicle
// ---------------------------------------------------------------------------------------------------------------

/// 3.5: no pawn: false; an on-foot pawn (a script subclass too): false, unless the flag is set with a driven vehicle;
/// another first native class names it; an unloaded pawn is its `NotRead`.
#[test]
fn is_using_vehicle_s_cases() {
    let mut x = fx();
    let pc = x.make("Engine.Controller");
    assert_eq!(x.vehicle(pc, true).unwrap(), Value::Bool(false), "no pawn");
    let pawn = x.make("Test.ToyScriptPawn");
    x.set(pc, "Pawn", Value::Object(Some(pawn)));
    assert_eq!(x.vehicle(pc, true).unwrap(), Value::Bool(false));
    let car = x.make("Engine.Vehicle");
    x.set(pawn, "DrivenVehicle", Value::Object(Some(car)));
    assert_eq!(x.vehicle(pc, false).unwrap(), Value::Bool(false), "the flag clear");
    assert_eq!(not_read(x.vehicle(pc, true).unwrap_err()).0, VEHICLE_ANSWER);
    x.set(pc, "Pawn", Value::Object(Some(car)));
    assert_eq!(not_read(x.vehicle(pc, false).unwrap_err()).0, format!("{PAWN_ANSWER} Engine.Vehicle"));
    let class = x.vm.space.classes.by_path("WillowGame.WillowPawn").unwrap();
    let lazy = x.vm.space.alloc_stand_in(class, "LazyPawn", Origin::Default { class }).unwrap();
    x.set(pc, "Pawn", Value::Object(Some(lazy)));
    assert_eq!(not_read(x.vehicle(pc, false).unwrap_err()).0, NOT_LOADED);
}

// ---------------------------------------------------------------------------------------------------------------
// SetOwner
// ---------------------------------------------------------------------------------------------------------------

/// 3.7: the owner set, `bNetDirty` set, and the owner-visibility refresh: a primitive with `bOwnerNoSee` or
/// `bOnlyOwnerSee` gets `bNeedsReattach`; a plain one and a non-primitive do not.
#[test]
fn set_owner_refreshes_owner_visibility_and_marks_the_actor_dirty() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let (plain, no_see, only) = (x.primitive(false, false), x.primitive(false, true), x.primitive(true, false));
    let light = x.make("Engine.ToyLightComponent");
    x.list_of(actor, "AllComponents", &[Some(plain), Some(no_see), None, Some(only), Some(light)]);
    let owner = x.make("Engine.Actor");
    x.set_owner(actor, Some(owner)).unwrap();
    assert_eq!(x.get(actor, "Owner"), Value::Object(Some(owner)));
    assert_eq!(x.get(actor, "bNetDirty"), Value::Bool(true));
    let marked: Vec<_> = [plain, no_see, only, light].iter().map(|c| x.get(*c, "bNeedsReattach")).collect();
    assert_eq!(marked, [false, true, true, false].map(Value::Bool));
    // The actor's own bOnlyOwnerSee: every primitive.
    let actor = x.make("Engine.Actor");
    let plain = x.primitive(false, false);
    x.list_of(actor, "AllComponents", &[Some(plain)]);
    x.set(actor, "bOnlyOwnerSee", Value::Bool(true));
    x.set_owner(actor, Some(owner)).unwrap();
    assert_eq!(x.get(plain, "bNeedsReattach"), Value::Bool(true));
}

/// 3.7: an unchanged owner and an ownership loop write nothing (no `bNetDirty`).
#[test]
fn a_set_owner_that_changes_nothing_writes_nothing() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let owner = x.make("Engine.Actor");
    x.set(owner, "Owner", Value::Object(Some(actor)));
    let before = x.marks();
    x.set_owner(actor, None).unwrap();
    x.set_owner(actor, Some(owner)).unwrap();
    assert_eq!(x.marks(), before);
    assert_eq!(x.get(actor, "bNetDirty"), Value::Bool(false));
}

/// 3.7: a `GainedChild` that moves the owner on stops the native after it (no refresh, no `bNetDirty`).
#[test]
fn an_owner_event_that_reowns_stops_the_native() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let no_see = x.primitive(false, true);
    x.list_of(actor, "AllComponents", &[Some(no_see)]);
    let (owner, other) = (x.make("Test.ToyOwner"), x.make("Engine.Actor"));
    x.reown.set(Some(other));
    x.set_owner(actor, Some(owner)).unwrap();
    assert_eq!(x.get(actor, "Owner"), Value::Object(Some(other)));
    assert_eq!(x.get(actor, "bNetDirty"), Value::Bool(false));
    assert_eq!(x.get(no_see, "bNeedsReattach"), Value::Bool(false));
}

/// The hub's correction on #785: a `GainedChild` that changes which components the refresh reattaches is the
/// prefixed `NotRead` (the owner was written); a reattach refusal planned up front writes nothing.
#[test]
fn a_refresh_the_owner_events_changed_is_refused() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let plain = x.primitive(false, false);
    x.list_of(actor, "AllComponents", &[Some(plain)]);
    let owner = x.make("Test.ToyOwner");
    x.flip.set(Some(plain));
    let (what, marked) = not_read(x.set_owner(actor, Some(owner)).unwrap_err());
    assert!(what.ends_with(REFRESH_CHANGED), "{what}");
    assert!(marked, "the owner was written");
    assert_eq!(x.get(actor, "bNetDirty"), Value::Bool(false));
    // Up front: a component in a scene with no owner is the reattach's refusal, before any write.
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let c = x.primitive(false, true);
    let scene = x.vm.space.classes.struct_by_path("Engine.ActorComponent.ToyScene").unwrap();
    x.set(c, "Scene", Value::structure(scene, vec![Value::Int(1)]));
    x.list_of(actor, "AllComponents", &[Some(c)]);
    let owner = x.make("Engine.Actor");
    let before = x.marks();
    let (_, marked) = not_read(x.set_owner(actor, Some(owner)).unwrap_err());
    assert!(!marked);
    assert_eq!(x.marks(), before, "nothing written");
    assert_eq!(x.get(actor, "Owner"), Value::Object(None));
}

/// Reference binds none of the four keys.
#[test]
fn reference_binds_nothing() {
    let x = fx_in(ParityMode::Reference);
    for key in [MAKE_NOISE, IS_USING_VEHICLE, SET_OWNER] {
        assert!(!x.vm.has_native(key), "{key}");
    }
    assert!(x.vm.iterator_for(x.list).is_none(), "{COMPONENT_LIST}");
}
