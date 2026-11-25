//! `GetFiringModeDefinition` on invented classes (NATIVE_WEAPON_GET_FIRING_MODE.md section 5; AGENTS.md rule 9:
//! invented objects and values, the class and field names the packages'). Every call checks that nothing was written,
//! loaded, drawn or interned, and no class code loaded (test 9). Lazy objects are slotless stand-ins.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    f: FuncId,
    weapon: ClassId,
    data: StructId,
    part: ClassId,
    weapon_type: ClassId,
    mode: ClassId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx_in(mode: ParityMode) -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let o = PropKind::Object { class: object };
    let explosion = vec![decl(&mut vm, "ExplosionOverrideDefinition", o.clone())];
    let firing = vm.space.classes.define("WillowGame.FiringModeDefinition", Some(object), explosion).unwrap();
    let custom = vec![decl(&mut vm, "CustomFiringModeDefinition", PropKind::Object { class: firing })];
    let part = vm.space.classes.define("WillowGame.WeaponPartDefinition", Some(object), custom).unwrap();
    let default = vec![decl(&mut vm, "DefaultFiringModeDefinition", PropKind::Object { class: firing })];
    let weapon_type = vm.space.classes.define("WillowGame.WeaponTypeDefinition", Some(object), default).unwrap();
    let fields = vec![
        decl(&mut vm, "WeaponTypeDefinition", PropKind::Object { class: weapon_type }),
        decl(&mut vm, "BarrelPartDefinition", PropKind::Object { class: part }),
        decl(&mut vm, "ElementalPartDefinition", PropKind::Object { class: part }),
    ];
    let data = vm.space.classes.define_struct("WillowGame.WillowWeaponTypes.WeaponDefinitionData", None, fields).unwrap();
    let props = vec![decl(&mut vm, "DefinitionData", PropKind::Struct(data))];
    let weapon = vm.space.classes.define("WillowGame.WillowWeapon", Some(object), props).unwrap();
    let path = ObjectPath::parse("WillowGame.WillowWeapon.GetFiringModeDefinition").unwrap();
    let f = vm.declare_function(FunctionDecl::native(path, KEY).with_owner(weapon)).unwrap();
    register(&mut vm).unwrap();
    Fx { vm, f, weapon, data, part, weapon_type, mode: firing }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn make(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.vm.space.alloc(class, name, None, Origin::Runtime).unwrap()
    }

    fn lazy(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.vm.space.alloc_stand_in(class, name, Origin::Default { class }).unwrap()
    }

    fn firing_mode(&mut self, name: &str) -> ObjRef {
        let m = self.mode;
        self.make(m, name)
    }

    /// A part whose `CustomFiringModeDefinition` is `mode`.
    fn part(&mut self, name: &str, mode: Option<ObjRef>) -> ObjRef {
        let c = self.part;
        let p = self.make(c, name);
        self.set(p, "CustomFiringModeDefinition", Value::Object(mode));
        p
    }

    fn weapon_type(&mut self, name: &str, mode: Option<ObjRef>) -> ObjRef {
        let c = self.weapon_type;
        let t = self.make(c, name);
        self.set(t, "DefaultFiringModeDefinition", Value::Object(mode));
        t
    }

    /// A weapon whose definition data names these three (in the struct's field order: type, barrel, elemental).
    fn weapon(&mut self, barrel: Option<ObjRef>, elemental: Option<ObjRef>, weapon_type: Option<ObjRef>) -> ObjRef {
        let c = self.weapon;
        let w = self.make(c, "Weapon_0");
        let data = Value::structure(self.data, vec![Value::Object(weapon_type), Value::Object(barrel), Value::Object(elemental)]);
        self.set(w, "DefinitionData", data);
        w
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    /// The native on `w`; asserts nothing changed.
    fn ask(&mut self, w: ObjRef) -> Result<Option<ObjRef>, VmError> {
        let before = self.marks();
        let out = self.vm.call(self.f, Some(w), vec![]);
        assert_eq!(self.marks(), before, "a pure read");
        match out?.ret {
            Value::Object(o) => Ok(o),
            other => panic!("{other:?}"),
        }
    }
}

/// Tests 1 to 5 and 9: the barrel's mode first, then the elemental part's, then the type's default (None included),
/// then None; every call writes nothing.
#[test]
fn the_barrel_then_the_elemental_part_then_the_type() {
    let mut x = fx();
    let [barrel_mode, elemental_mode, type_mode] = ["BarrelMode", "ElementalMode", "TypeMode"].map(|n| x.firing_mode(n));
    let barrel = x.part("Barrel", Some(barrel_mode));
    let elemental = x.part("Elemental", Some(elemental_mode));
    let ty = x.weapon_type("Type", Some(type_mode));
    let w = x.weapon(Some(barrel), Some(elemental), Some(ty));
    assert_eq!(x.ask(w).unwrap(), Some(barrel_mode), "test 1");
    let plain_barrel = x.part("PlainBarrel", None);
    let w = x.weapon(Some(plain_barrel), Some(elemental), Some(ty));
    assert_eq!(x.ask(w).unwrap(), Some(elemental_mode), "test 2");
    let plain_elemental = x.part("PlainElemental", None);
    let w = x.weapon(Some(plain_barrel), Some(plain_elemental), Some(ty));
    assert_eq!(x.ask(w).unwrap(), Some(type_mode), "test 3");
    let no_mode_type = x.weapon_type("NoModeType", None);
    let w = x.weapon(Some(plain_barrel), Some(plain_elemental), Some(no_mode_type));
    assert_eq!(x.ask(w).unwrap(), None, "test 3, a None default");
    let w = x.weapon(None, None, Some(ty));
    assert_eq!(x.ask(w).unwrap(), Some(type_mode), "test 4");
    let w = x.weapon(None, None, None);
    assert_eq!(x.ask(w).unwrap(), None, "test 5");
}

/// Test 6: an unloaded object on the rule's path is the `NotRead` naming it, nothing loaded; off the path, no refusal.
#[test]
fn an_unloaded_object_on_the_path_is_not_read_and_off_it_is_nothing() {
    let mut x = fx();
    let mode = x.firing_mode("Mode");
    let ty = x.weapon_type("Type", Some(mode));
    let (part, weapon_type) = (x.part, x.weapon_type);
    let lazy_barrel = x.lazy(part, "LazyBarrel");
    let lazy_elemental = x.lazy(part, "LazyElemental");
    let lazy_type = x.lazy(weapon_type, "LazyType");
    let plain = x.part("Plain", None);
    let with_mode = x.part("WithMode", Some(mode));
    for (w, label) in [
        (x.weapon(Some(lazy_barrel), None, Some(ty)), "barrel part LazyBarrel"),
        (x.weapon(Some(plain), Some(lazy_elemental), Some(ty)), "elemental part LazyElemental"),
        (x.weapon(None, None, Some(lazy_type)), "weapon type LazyType"),
    ] {
        match x.ask(w) {
            Err(VmError::NotYet(n)) => {
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.what.contains(label) && n.what.contains("which the store has not loaded"), "{n}");
                assert_eq!(n.record, RECORD);
            }
            other => panic!("{label}: {other:?}"),
        }
    }
    let w = x.weapon(Some(with_mode), Some(lazy_elemental), Some(lazy_type));
    assert_eq!(x.ask(w).unwrap(), Some(mode), "behind a barrel with a mode");
    let w = x.weapon(None, Some(with_mode), Some(lazy_type));
    assert_eq!(x.ask(w).unwrap(), Some(mode), "behind an elemental part with a mode");
}

/// Test 7: a mode that is an unloaded handle is returned as the same handle, still unloaded.
#[test]
fn an_unloaded_mode_is_returned_as_it_is() {
    let mut x = fx();
    let m = x.mode;
    let lazy_mode = x.lazy(m, "LazyMode");
    let ty = x.weapon_type("Type", Some(lazy_mode));
    let w = x.weapon(None, None, Some(ty));
    assert_eq!(x.ask(w).unwrap(), Some(lazy_mode));
    assert!(x.vm.space.heap.get(lazy_mode).unwrap().slots.is_empty(), "not loaded");
}

/// Test 8: `DefinitionData`, a part's `CustomFiringModeDefinition` or the type's `DefaultFiringModeDefinition` declared
/// as something else: `VmError::Script` naming it.
#[test]
fn malformed_declarations_are_script_errors() {
    let mut x = fx();
    let object = x.vm.space.classes.by_path("Core.Object").unwrap();
    let odd = decl(&mut x.vm, "DefinitionData", PropKind::Int);
    let odd_weapon = x.vm.space.classes.define("Test.OddWeapon", Some(x.weapon), vec![odd]).unwrap();
    let w = x.make(odd_weapon, "OddWeapon");
    let script = |e: VmError| match e {
        VmError::Script { message, .. } => message,
        other => panic!("{other:?}"),
    };
    assert!(script(x.ask(w).unwrap_err()).contains("DefinitionData"));
    let odd = decl(&mut x.vm, "CustomFiringModeDefinition", PropKind::Int);
    let odd_part = x.vm.space.classes.define("Test.OddPart", Some(object), vec![odd]).unwrap();
    let barrel = x.make(odd_part, "OddBarrel");
    // The struct declares the part as a WeaponPartDefinition, so the odd part goes in through a raw slot.
    let w = x.weapon(None, None, None);
    let p = x.vm.space.prop(x.weapon, "DefinitionData").unwrap();
    let data = Value::structure(x.data, vec![Value::Object(None), Value::Object(Some(barrel)), Value::Object(None)]);
    *x.vm.space.slot_mut(w, PropAddr::scalar(p)).unwrap() = data;
    assert!(script(x.ask(w).unwrap_err()).contains("CustomFiringModeDefinition"));
    let odd = decl(&mut x.vm, "DefaultFiringModeDefinition", PropKind::Float);
    let odd_type = x.vm.space.classes.define("Test.OddType", Some(object), vec![odd]).unwrap();
    let ty = x.make(odd_type, "OddType");
    let data = Value::structure(x.data, vec![Value::Object(Some(ty)), Value::Object(None), Value::Object(None)]);
    *x.vm.space.slot_mut(w, PropAddr::scalar(p)).unwrap() = data;
    assert!(script(x.ask(w).unwrap_err()).contains("DefaultFiringModeDefinition"));
}

/// Test 10: a toy subclass of `WillowWeapon` gets the same answer (no native subclass overrides it).
#[test]
fn a_subclass_gets_the_same_answer() {
    let mut x = fx();
    let mode = x.firing_mode("Mode");
    let ty = x.weapon_type("Type", Some(mode));
    let sub = x.vm.space.classes.define("WillowGame.WillowTurretWeapon", Some(x.weapon), vec![]).unwrap();
    let w = x.make(sub, "Turret");
    let data = Value::structure(x.data, vec![Value::Object(Some(ty)), Value::Object(None), Value::Object(None)]);
    x.set(w, "DefinitionData", data);
    assert_eq!(x.ask(w).unwrap(), Some(mode));
}

/// Test 11 (the native's part of link 8's shape): the type default's mode with `ExplosionOverrideDefinition` None is
/// the answer, so `InitializeInternal`'s explosion block has nothing to call. The script tail itself runs on the
/// installed slice gate (link 8), not here.
#[test]
fn link_8_s_type_default_mode() {
    let mut x = fx();
    let mode = x.firing_mode("Bullet_Toy_Default");
    let ty = x.weapon_type("WT_Toy", Some(mode));
    let barrel = x.part("Barrel_Toy", None);
    let w = x.weapon(Some(barrel), None, Some(ty));
    assert_eq!(x.ask(w).unwrap(), Some(mode));
    let p = x.vm.space.prop(x.mode, "ExplosionOverrideDefinition").unwrap();
    assert_eq!(x.vm.space.read(mode, PropAddr::scalar(p)).unwrap(), &Value::Object(None));
}

/// Test 12: Reference binds nothing.
#[test]
fn reference_binds_nothing() {
    let x = fx_in(ParityMode::Reference);
    assert!(!x.vm.has_native(KEY));
}
