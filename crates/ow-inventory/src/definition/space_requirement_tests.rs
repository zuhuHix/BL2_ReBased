//! Tests of `GetInventorySpaceRequirement` (lane IMPL-GETINVENTORYSPACEREQUIREMENT; NATIVE_INVENTORY_EQUIP.md,
//! "Inventory space") on invented classes and objects (AGENTS.md rule 9). The class paths are the row's; the class
//! tree is invented so each case of the row, and each refusal, has one.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

struct W {
    vm: Vm,
    wdata: StructId,
    next: u32,
}

/// A world with `Engine.WillowInventory`, `WillowGame.WillowWeapon` (its `DefinitionData` holding a
/// `WeaponTypeDefinition`), the row's definition classes and one invented subclass, and both natives declared.
fn world(mode: ParityMode) -> W {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let obj = PropKind::Object { class: object };
    let field = PropertyDecl::new(space.names.intern("WeaponTypeDefinition"), obj.clone());
    let wdata = space.classes.define_struct("WillowGame.WillowWeaponTypes.WeaponDefinitionData", None, vec![field]).unwrap();
    let base = space.classes.define(INVENTORY_BASE, Some(object), vec![]).unwrap();
    let weapon_base = space.classes.define("Engine.Weapon", Some(base), vec![]).unwrap();
    let data = PropertyDecl::new(space.names.intern("DefinitionData"), PropKind::Struct(wdata));
    space.classes.define("WillowGame.WillowWeapon", Some(weapon_base), vec![data]).unwrap();
    let definition = space.classes.define("WillowGame.WillowInventoryDefinition", Some(object), vec![]).unwrap();
    for path in ["WillowGame.WeaponTypeDefinition", MISSION_ITEM_DEFINITION] {
        space.classes.define(path, Some(definition), vec![]).unwrap();
    }
    let item = space.classes.define("WillowGame.ItemDefinition", Some(definition), vec![]).unwrap();
    let equipable = space.classes.define("WillowGame.EquipableItemDefinition", Some(item), vec![]).unwrap();
    space.classes.define("WillowGame.ToyArtifactDefinition", Some(equipable), vec![]).unwrap();
    space.classes.define(USABLE_ITEM_DEFINITION, Some(item), vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    vm.stub_policy = StubPolicy::refuse_all();
    for (path, key, kind) in [
        (GET_INVENTORY_DEFINITION, "WillowInventory.GetInventoryDefinition()", obj.clone()),
        (GET_INVENTORY_SPACE_REQUIREMENT, "WillowInventory.GetInventorySpaceRequirement()", PropKind::Int),
    ] {
        let mut f = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key.to_string()).with_owner(base);
        f.result = Some(PropertyDecl::new(vm.space.names.intern("ReturnValue"), kind).with_flags(PropertyDecl::RETURN));
        vm.declare_function(f).unwrap();
    }
    register(&mut vm).unwrap();
    W { vm, wdata, next: 0 }
}

impl W {
    fn obj(&mut self, path: &str) -> ObjRef {
        let class = self.vm.space.classes.by_path(path).unwrap();
        self.next += 1;
        self.vm.space.alloc(class, &format!("O{}", self.next), None, Origin::Runtime).unwrap()
    }

    /// A weapon whose `DefinitionData.WeaponTypeDefinition` is a new object of class `definition` (None for no
    /// definition).
    fn weapon(&mut self, definition: Option<&str>) -> ObjRef {
        let d = definition.map(|p| self.obj(p));
        let g = self.obj("WillowGame.WillowWeapon");
        let class = self.vm.space.heap.get(g).unwrap().class;
        let p = self.vm.space.prop(class, "DefinitionData").unwrap();
        let v = Value::structure(self.wdata, vec![Value::Object(d)]);
        self.vm.space.write(g, PropAddr::scalar(p), v).unwrap();
        g
    }

    fn call(&mut self, this: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(GET_INVENTORY_SPACE_REQUIREMENT)?;
        Ok(self.vm.call(f, Some(this), vec![])?.ret)
    }

    /// The answer, with the heap's writes and the RNG's draws unchanged.
    fn quiet(&mut self, this: ObjRef) -> Result<Value, VmError> {
        let (writes, draws) = (self.vm.space.heap.committed_writes(), self.vm.rng.draws());
        let r = self.call(this);
        assert_eq!((self.vm.space.heap.committed_writes(), self.vm.rng.draws()), (writes, draws), "reads only");
        r
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            n.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// The row's two default classes answer 1, a mission item 0.
#[test]
fn the_read_definition_classes_answer_the_row() {
    let mut w = world(ParityMode::Off);
    for (definition, want) in [
        ("WillowGame.WeaponTypeDefinition", 1),
        ("WillowGame.EquipableItemDefinition", 1),
        (MISSION_ITEM_DEFINITION, 0),
    ] {
        let g = w.weapon(Some(definition));
        assert_eq!(w.quiet(g).unwrap(), Value::Int(want), "{definition}");
    }
}

/// A usable item (its flag field not named), a subclass of a read class, and no definition are each `NotRead`.
#[test]
fn what_the_row_does_not_read_is_refused() {
    let mut w = world(ParityMode::Off);
    let g = w.weapon(Some(USABLE_ITEM_DEFINITION));
    assert!(not_read(w.quiet(g)).contains("two low flag bits"));
    let g = w.weapon(Some("WillowGame.ToyArtifactDefinition"));
    assert!(not_read(w.quiet(g)).contains("WillowGame.ToyArtifactDefinition"));
    let g = w.weapon(Some("WillowGame.ItemDefinition"));
    assert!(not_read(w.quiet(g)).contains("whether it overrides the default"));
    let g = w.weapon(None);
    assert!(not_read(w.quiet(g)).contains("no definition"));
}

/// The inventory side is `GetInventoryDefinition`'s: an inventory class it does not read refuses with its text.
#[test]
fn the_definition_lookup_comes_first() {
    let mut w = world(ParityMode::Off);
    let g = w.obj("Engine.Weapon");
    assert!(not_read(w.quiet(g)).contains("GetInventoryDefinition on a Engine.Weapon"));
}

/// Reference binds nothing.
#[test]
fn reference_binds_nothing() {
    let mut w = world(ParityMode::Reference);
    let g = w.weapon(Some("WillowGame.WeaponTypeDefinition"));
    assert!(w.call(g).is_err(), "unbound under Reference");
}
