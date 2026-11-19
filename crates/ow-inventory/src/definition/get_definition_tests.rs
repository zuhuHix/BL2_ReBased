//! Tests of `GetInventoryDefinition` (NATIVE_INVENTORY_DEFINITION.md section 3) on invented classes and objects
//! (AGENTS.md rule 9), in both parity modes. The class paths are the note's; which classes extend which, and which
//! declare a broken `DefinitionData`, is invented so that each step of section 3 has a case.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

struct W {
    vm: Vm,
    /// `WeaponTypeDefinition` alone, `ItemDefinition` alone, and both (item field first).
    wdata: StructId,
    idata: StructId,
    both: StructId,
    definition: ClassId,
    next: u32,
}

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind)
}

fn world(mode: ParityMode) -> W {
    world_spelled(mode, INVENTORY_BASE, "WillowGame.WillowWeapon")
}

/// The world with the base and `WillowWeapon` defined under the spellings `base_path` and `weapon_path`.
fn world_spelled(mode: ParityMode, base_path: &str, weapon_path: &str) -> W {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let obj = PropKind::Object { class: object };
    let st = |space: &mut ObjectSpace, path: &str, fields: &[&str]| {
        let d = fields.iter().map(|n| decl(space, n, obj.clone())).collect();
        space.classes.define_struct(path, None, d).unwrap()
    };
    let wdata = st(&mut space, "WillowGame.WillowWeaponTypes.WeaponDefinitionData", &["WeaponTypeDefinition"]);
    let idata = st(&mut space, "WillowGame.WillowItemTypes.ItemDefinitionData", &["ItemDefinition"]);
    let both = st(&mut space, "Test.BothDefinitionData", &["ItemDefinition", "WeaponTypeDefinition"]);
    let definition = space
        .classes
        .define("WillowGame.WillowInventoryDefinition", Some(object), vec![])
        .unwrap();
    let class = |space: &mut ObjectSpace, path: &str, parent: ClassId, data: Option<StructId>| {
        let d = data
            .map(|s| vec![decl(space, "DefinitionData", PropKind::Struct(s))])
            .unwrap_or_default();
        space.classes.define(path, Some(parent), d).unwrap()
    };
    let base = class(&mut space, base_path, object, None);
    let weapon_base = class(&mut space, "Engine.Weapon", base, None);
    let weapon = class(&mut space, weapon_path, weapon_base, Some(wdata));
    // A class with a read name in another package.
    class(&mut space, "Test.WillowWeapon", weapon_base, Some(wdata));
    class(&mut space, "WillowGame.WillowTurretWeapon", weapon, None);
    class(&mut space, "Test.DlcWeapon", weapon, None);
    // A read weapon class whose struct declares both fields, and one with no DefinitionData at all.
    class(&mut space, "WillowGame.WillowBuzzaxeWeapon", weapon_base, Some(both));
    class(&mut space, "WillowGame.WillowVehicleWeapon", weapon_base, None);
    let item = class(&mut space, "WillowGame.WillowItem", base, Some(idata));
    let equip = class(&mut space, "WillowGame.WillowEquipAbleItem", item, None);
    class(&mut space, "WillowGame.WillowShield", equip, None);
    class(&mut space, "Test.DlcItem", equip, None);
    // A read item class whose DefinitionData declares no ItemDefinition.
    class(&mut space, "WillowGame.WillowMissionItem", base, Some(wdata));
    class(&mut space, "Test.OtherInventory", base, None);
    // Declared but malformed layouts (step 5's last paragraph), on read classes: a DefinitionData that is no struct,
    // and a selected field that is no object reference.
    let d = vec![decl(&mut space, "DefinitionData", PropKind::Int)];
    space.classes.define("WillowGame.WillowArtifact", Some(base), d).unwrap();
    let ints = vec![decl(&mut space, "ItemDefinition", PropKind::Int)];
    let ints = space.classes.define_struct("Test.IntDefinitionData", None, ints).unwrap();
    class(&mut space, "WillowGame.WillowClassMod", base, Some(ints));
    // A fixed-array DefinitionData, a fixed-array selected field, and an interface-typed selected field.
    let d = vec![decl(&mut space, "DefinitionData", PropKind::Struct(idata)).with_array_dim(2)];
    space.classes.define("WillowGame.WillowGrenadeMod", Some(base), d).unwrap();
    let f = vec![decl(&mut space, "ItemDefinition", obj.clone()).with_array_dim(2)];
    let arrays = space.classes.define_struct("Test.ArrayDefinitionData", None, f).unwrap();
    class(&mut space, "WillowGame.WillowUsableItem", base, Some(arrays));
    let f = vec![decl(&mut space, "ItemDefinition", PropKind::Interface { class: object })];
    let iface = space.classes.define_struct("Test.InterfaceDefinitionData", None, f).unwrap();
    class(&mut space, "WillowGame.WillowUsableCustomizationItem", base, Some(iface));
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    vm.stub_policy = StubPolicy::refuse_all();
    let key = "WillowInventory.GetInventoryDefinition()".to_string();
    let mut f = FunctionDecl::native(ObjectPath::parse(GET_INVENTORY_DEFINITION).unwrap(), key).with_owner(base);
    f.result = Some(decl(&mut vm.space, "ReturnValue", obj).with_flags(PropertyDecl::RETURN));
    vm.declare_function(f).unwrap();
    register(&mut vm).unwrap();
    W { vm, wdata, idata, both, definition, next: 0 }
}

impl W {
    fn obj(&mut self, path: &str) -> ObjRef {
        let class = self.vm.space.classes.by_path(path).unwrap();
        self.next += 1;
        self.vm.space.alloc(class, &format!("O{}", self.next), None, Origin::Runtime).unwrap()
    }

    fn def(&mut self) -> ObjRef {
        let class = self.definition;
        self.next += 1;
        self.vm.space.alloc(class, &format!("D{}", self.next), None, Origin::Runtime).unwrap()
    }

    /// An inventory of class `path` whose `DefinitionData` (of struct `ty`) holds `fields`, in the struct's order.
    fn inventory(&mut self, path: &str, ty: StructId, fields: &[Option<ObjRef>]) -> ObjRef {
        let o = self.obj(path);
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "DefinitionData").unwrap();
        let v = Value::structure(ty, fields.iter().map(|f| Value::Object(*f)).collect());
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
        o
    }

    fn call(&mut self, this: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(GET_INVENTORY_DEFINITION)?;
        Ok(self.vm.call(f, Some(this), vec![])?.ret)
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(n.record, DEFINITION_RECORD);
            n.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

#[test]
fn it_binds_under_engine_willow_inventory() {
    assert_eq!(GET_INVENTORY_DEFINITION, "Engine.WillowInventory.GetInventoryDefinition");
    assert_eq!(READ_WEAPON_CLASSES.len() + READ_ITEM_CLASSES.len(), 13, "the note's thirteen read classes");
}

#[test]
fn a_read_weapon_class_answers_its_weapon_type_definition_as_stored() {
    let mut w = world(ParityMode::Off);
    let (d, wdata) = (w.def(), w.wdata);
    for path in ["WillowGame.WillowWeapon", "WillowGame.WillowTurretWeapon"] {
        let g = w.inventory(path, wdata, &[Some(d)]);
        assert_eq!(w.call(g).unwrap(), Value::Object(Some(d)), "{path}");
        // A declared field that holds None answers None (step 6).
        let g = w.inventory(path, wdata, &[None]);
        assert_eq!(w.call(g).unwrap(), Value::Object(None), "{path}");
    }
}

#[test]
fn a_read_item_class_answers_its_item_definition_as_stored() {
    let mut w = world(ParityMode::Off);
    let (d, idata) = (w.def(), w.idata);
    for path in ["WillowGame.WillowItem", "WillowGame.WillowEquipAbleItem", "WillowGame.WillowShield"] {
        let i = w.inventory(path, idata, &[Some(d)]);
        assert_eq!(w.call(i).unwrap(), Value::Object(Some(d)), "{path}");
        let i = w.inventory(path, idata, &[None]);
        assert_eq!(w.call(i).unwrap(), Value::Object(None), "{path}");
    }
}

#[test]
fn the_field_is_chosen_by_class_not_by_which_is_set() {
    // A weapon class whose struct declares ItemDefinition first: it still answers WeaponTypeDefinition, None included.
    let mut w = world(ParityMode::Off);
    let (d, both) = (w.def(), w.both);
    let g = w.inventory("WillowGame.WillowBuzzaxeWeapon", both, &[Some(d), None]);
    assert_eq!(w.call(g).unwrap(), Value::Object(None));
    let g = w.inventory("WillowGame.WillowBuzzaxeWeapon", both, &[None, Some(d)]);
    assert_eq!(w.call(g).unwrap(), Value::Object(Some(d)));
}

#[test]
fn the_exact_base_answers_none() {
    let mut w = world(ParityMode::Off);
    let o = w.obj(INVENTORY_BASE);
    assert_eq!(w.call(o).unwrap(), Value::Object(None));
    assert!(w.vm.log.is_empty(), "no diagnostic: {:?}", w.vm.log);
}

#[test]
fn an_unread_class_is_a_named_not_yet() {
    let mut w = world(ParityMode::Off);
    let (wdata, idata) = (w.wdata, w.idata);
    // A plain Engine.Weapon, another WillowInventory subclass, and descendants beyond the thirteen read classes.
    let o = w.obj("Engine.Weapon");
    assert!(not_read(w.call(o)).contains("GetInventoryDefinition on a Engine.Weapon"));
    let o = w.obj("Test.OtherInventory");
    assert!(not_read(w.call(o)).contains("Test.OtherInventory"));
    let o = w.inventory("Test.DlcWeapon", wdata, &[None]);
    assert!(not_read(w.call(o)).contains("Test.DlcWeapon"));
    let o = w.inventory("Test.DlcItem", idata, &[None]);
    assert!(not_read(w.call(o)).contains("Test.DlcItem"));
}

#[test]
fn a_missing_layout_is_a_named_not_yet() {
    let mut w = world(ParityMode::Off);
    let o = w.obj("WillowGame.WillowVehicleWeapon");
    let text = not_read(w.call(o));
    assert!(text.contains("GetInventoryDefinition on a class without DefinitionData"), "{text}");
    let wdata = w.wdata;
    let o = w.inventory("WillowGame.WillowMissionItem", wdata, &[None]);
    let text = not_read(w.call(o));
    assert!(text.contains("GetInventoryDefinition on a DefinitionData without ItemDefinition"), "{text}");
}

#[test]
fn a_declared_but_malformed_layout_is_a_data_error() {
    let mut w = world(ParityMode::Off);
    for (path, said) in [
        ("WillowGame.WillowArtifact", "DefinitionData is declared as Int x 1, not one struct"),
        ("WillowGame.WillowGrenadeMod", "DefinitionData is declared as Struct("),
        ("WillowGame.WillowClassMod", "DefinitionData.ItemDefinition is declared as Int x 1"),
        ("WillowGame.WillowUsableItem", "DefinitionData.ItemDefinition is declared as Object {"),
        ("WillowGame.WillowUsableCustomizationItem", "DefinitionData.ItemDefinition is declared as Interface {"),
    ] {
        let o = w.obj(path);
        match w.call(o) {
            Err(VmError::Script { function, message }) => {
                assert!(function.contains("GetInventoryDefinition"), "{path}: {function}");
                assert!(message.contains(said), "{path}: {message}");
            }
            other => panic!("{path}: expected a data error, got {other:?}"),
        }
    }
    // The array cases name their dimension.
    let o = w.obj("WillowGame.WillowUsableItem");
    assert!(format!("{:?}", w.call(o)).contains(" x 2, not one object reference"));
}

#[test]
fn an_equivalent_spelling_matches_and_another_package_does_not() {
    // `willowgame.WILLOWWEAPON` and `engine:WillowInventory` name the read classes under the VM's path rules.
    let mut w = world_spelled(ParityMode::Off, "engine:WillowInventory", "willowgame.WILLOWWEAPON");
    let (d, wdata) = (w.def(), w.wdata);
    let g = w.inventory("WillowGame.WillowWeapon", wdata, &[Some(d)]);
    assert_eq!(w.call(g).unwrap(), Value::Object(Some(d)));
    let o = w.obj(INVENTORY_BASE);
    assert_eq!(w.call(o).unwrap(), Value::Object(None));
    // The same name in another package is not a read class.
    let g = w.inventory("Test.WillowWeapon", wdata, &[Some(d)]);
    assert!(not_read(w.call(g)).contains("Test.WillowWeapon"));
}

#[test]
fn reference_leaves_it_unbound() {
    let mut w = world(ParityMode::Reference);
    let wdata = w.wdata;
    let g = w.inventory("WillowGame.WillowWeapon", wdata, &[None]);
    match w.call(g) {
        Err(VmError::UnboundNative(key)) => assert_eq!(key, "WillowInventory.GetInventoryDefinition()"),
        other => panic!("expected UnboundNative, got {other:?}"),
    }
}
