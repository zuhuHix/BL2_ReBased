//! The value natives on an invented class set (AGENTS.md rule 9; decision 2026-11-19-36). Expected values are written
//! by hand from NATIVE_INVENTORY_VALUE.md. Every init value here has scale 1 and no base attribute, so its value is its
//! constant whatever the evaluator's formula.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::equip::read_prop;

struct Toy {
    vm: Vm,
    aid: StructId,
    wdata: StructId,
    idata: StructId,
}

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind)
}

impl Toy {
    fn new(mode: ParityMode) -> Toy {
        let mut s = ObjectSpace::new();
        let object = s.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = PropKind::Object { class: object };
        let objs = PropKind::Array(Box::new(obj.clone()));
        let fields = vec![
            decl(&mut s, "BaseValueConstant", PropKind::Float),
            decl(&mut s, "BaseValueAttribute", obj.clone()),
            decl(&mut s, "InitializationDefinition", obj.clone()),
            decl(&mut s, "BaseValueScaleConstant", PropKind::Float),
        ];
        let aid = s
            .classes
            .define_struct("Engine.AttributeInitializationDefinition.AttributeInitializationData", None, fields)
            .unwrap();
        let chains = vec![decl(&mut s, "ContextResolverChain", objs.clone()), decl(&mut s, "ValueResolverChain", objs)];
        let base = s.classes.define("Engine.AttributeDefinitionBase", Some(object), chains).unwrap();
        s.classes.define("Engine.AttributeDefinition", Some(base), vec![]).unwrap();
        s.classes.define("Engine.AttributeInitializationDefinition", Some(object), vec![]).unwrap();
        let constant = vec![decl(&mut s, "ConstantValue", PropKind::Float)];
        s.classes.define("GearboxFramework.ConstantAttributeValueResolver", Some(object), constant).unwrap();
        // The 13 entries `crate::definition`'s cap rule checks: three named, ten reserved (invented names).
        let mut currencies: Vec<String> =
            ["CURRENCY_Credits", "CURRENCY_Eridium", "CURRENCY_SeraphCrystals"].iter().map(|c| c.to_string()).collect();
        currencies.extend((0..10).map(|i| format!("CURRENCY_ToyReserved{i}")));
        let values = currencies.iter().map(|c| s.names.intern(c)).collect();
        let currency = s.classes.define_enum("Test.ECurrency", values).unwrap();
        let definition = vec![
            decl(&mut s, "BaseRarity", PropKind::Struct(aid)),
            decl(&mut s, "BaseMonetaryValueModifier", PropKind::Float),
            decl(&mut s, "MonetaryValue", PropKind::Struct(aid)),
            decl(&mut s, "FormOfCurrency", PropKind::Byte { enum_: Some(currency) }),
        ];
        let definition = s.classes.define("Test.Definition", Some(object), definition).unwrap();
        s.classes.define("WillowGame.MissionItemDefinition", Some(definition), vec![]).unwrap();
        let part = vec![decl(&mut s, "Rarity", PropKind::Struct(aid)), decl(&mut s, "MonetaryValueMod", obj.clone())];
        s.classes.define("Test.Part", Some(object), part).unwrap();
        let mut w = vec![decl(&mut s, "WeaponTypeDefinition", obj.clone())];
        for slot in WEAPON.slots {
            w.push(decl(&mut s, slot, obj.clone()));
        }
        let wdata = s.classes.define_struct("Test.WeaponDefinitionData", None, w).unwrap();
        let mut i = vec![decl(&mut s, "ItemDefinition", obj.clone())];
        for slot in ITEM.slots {
            i.push(decl(&mut s, slot, obj.clone()));
        }
        let idata = s.classes.define_struct("Test.ItemDefinitionData", None, i).unwrap();
        let fields = |s: &mut ObjectSpace, data: StructId| {
            vec![
                decl(s, "DefinitionData", PropKind::Struct(data)),
                decl(s, "RarityLevel", PropKind::Int),
                decl(s, "MonetaryValueModifierTotal", PropKind::Float),
                decl(s, "MonetaryValue", PropKind::Int),
            ]
        };
        let inventory = s.classes.define("WillowGame.WillowInventory", Some(object), vec![]).unwrap();
        let wf = fields(&mut s, wdata);
        s.classes.define("WillowGame.WillowWeapon", Some(inventory), wf).unwrap();
        let itf = fields(&mut s, idata);
        s.classes.define("WillowGame.WillowItem", Some(inventory), itf).unwrap();
        let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::standard().unwrap()), mode);
        for (path, key) in [
            ("WillowGame.WillowWeapon.ComputeValueOfParts", WEAPON_VALUE),
            ("WillowGame.WillowItem.ComputeValueOfParts", ITEM_VALUE),
        ] {
            vm.declare_function(FunctionDecl::native(ObjectPath::parse(path).unwrap(), key)).unwrap();
        }
        register(&mut vm).unwrap();
        Toy { vm, aid, wdata, idata }
    }

    fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }

    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.class(class);
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        read_prop(&self.vm, o, name).unwrap()
    }

    fn st(&mut self, ty: StructId, fields: Vec<(&str, Value)>) -> Value {
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.vm.space.names.find(n).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = x;
        }
        v
    }

    /// An init value of constant `c` (scale 1, no attribute).
    fn init(&mut self, c: f32) -> Value {
        let aid = self.aid;
        self.st(aid, vec![("BaseValueConstant", Value::Float(c)), ("BaseValueScaleConstant", Value::Float(1.0))])
    }

    /// An attribute whose value chain is one constant resolver of `c`.
    fn constant_attribute(&mut self, name: &str, c: f32) -> ObjRef {
        let r = self.obj("GearboxFramework.ConstantAttributeValueResolver", &format!("{name}_Value"));
        self.set(r, "ConstantValue", Value::Float(c));
        let a = self.obj("Engine.AttributeDefinition", name);
        self.set(a, "ValueResolverChain", Value::array(vec![Value::Object(Some(r))]));
        a
    }

    fn definition(&mut self, class: &str, rarity: f32, modifier: f32, money: f32, currency: u8) -> ObjRef {
        let d = self.obj(class, "Definition");
        let r = self.init(rarity);
        self.set(d, "BaseRarity", r);
        self.set(d, "BaseMonetaryValueModifier", Value::Float(modifier));
        let m = self.init(money);
        self.set(d, "MonetaryValue", m);
        self.set(d, "FormOfCurrency", Value::Byte(currency));
        d
    }

    fn part(&mut self, name: &str, rarity: f32, modifier: Option<f32>) -> ObjRef {
        let p = self.obj("Test.Part", name);
        let r = self.init(rarity);
        self.set(p, "Rarity", r);
        if let Some(m) = modifier {
            let a = self.constant_attribute(&format!("{name}_Mod"), m);
            self.set(p, "MonetaryValueMod", Value::Object(Some(a)));
        }
        p
    }

    fn weapon(&mut self, definition: Option<ObjRef>, parts: &[(&str, ObjRef)]) -> ObjRef {
        let w = self.obj("WillowGame.WillowWeapon", "Weapon_0");
        let mut fields = vec![("WeaponTypeDefinition", Value::Object(definition))];
        fields.extend(parts.iter().map(|(slot, p)| (*slot, Value::Object(Some(*p)))));
        let ty = self.wdata;
        let data = self.st(ty, fields);
        self.set(w, "DefinitionData", data);
        w
    }

    fn item(&mut self, definition: ObjRef) -> ObjRef {
        let i = self.obj("WillowGame.WillowItem", "Item_0");
        let ty = self.idata;
        let data = self.st(ty, vec![("ItemDefinition", Value::Object(Some(definition)))]);
        self.set(i, "DefinitionData", data);
        i
    }

    fn call(&mut self, path: &str, this: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, Some(this), Vec::new())?.ret)
    }
}

/// A weapon: rarity is the sum of the truncated values (2 + 1 + 3 = 6, not the truncated sum 8); the modifier total
/// multiplies the base by the set parts' modifiers, the prefix name part included (2 x 1.5 x 3 = 9); a part with no
/// modifier and an empty slot change nothing; the money value is `ComputeMonetaryValue`'s body, the truncated constant
/// (credits).
#[test]
fn a_weapons_value_follows_its_parts_and_name_parts() {
    let mut t = Toy::new(ParityMode::Off);
    let definition = t.definition("Test.Definition", 2.7, 2.0, 1234.9, 0);
    let body = t.part("Body", 1.9, Some(1.5));
    let grip = t.part("Grip", 0.4, None);
    let prefix = t.part("Prefix", 3.5, Some(3.0));
    let w = t.weapon(Some(definition), &[("BodyPartDefinition", body), ("GripPartDefinition", grip), ("PrefixPartDefinition", prefix)]);
    t.call("WillowGame.WillowWeapon.ComputeValueOfParts", w).unwrap();
    assert_eq!(t.get(w, "RarityLevel"), Value::Int(6));
    assert_eq!(t.get(w, "MonetaryValueModifierTotal"), Value::Float(9.0));
    assert_eq!(t.get(w, "MonetaryValue"), Value::Int(1234));
}

/// An unset `Rarity` adds 0 (the review of #419): a part with an all-zero `Rarity` struct leaves the definition's rarity.
#[test]
fn a_part_with_an_unset_rarity_adds_nothing() {
    let mut t = Toy::new(ParityMode::Off);
    let definition = t.definition("Test.Definition", 2.0, 1.0, 10.0, 0);
    let unset = t.obj("Test.Part", "Unset");
    let w = t.weapon(Some(definition), &[("BodyPartDefinition", unset)]);
    t.call("WillowGame.WillowWeapon.ComputeValueOfParts", w).unwrap();
    assert_eq!(t.get(w, "RarityLevel"), Value::Int(2));
    assert_eq!(t.get(w, "MonetaryValueModifierTotal"), Value::Float(1.0), "no MonetaryValueMod: the total stays");
}

/// No definition: nothing is written by ComputeValueOfParts.
#[test]
fn without_a_definition_nothing_is_written() {
    let mut t = Toy::new(ParityMode::Off);
    let w = t.weapon(None, &[]);
    t.set(w, "RarityLevel", Value::Int(41));
    t.call("WillowGame.WillowWeapon.ComputeValueOfParts", w).unwrap();
    assert_eq!(t.get(w, "RarityLevel"), Value::Int(41));
}

/// An item uses the weapon's rule on its own definition; a mission item reads the globals definition instead, which
/// a VM with no world cannot give (an error, not a guessed 0).
#[test]
fn an_item_uses_the_rule_and_a_mission_item_needs_the_globals() {
    let mut t = Toy::new(ParityMode::Off);
    let definition = t.definition("Test.Definition", 4.2, 1.0, 50.0, 0);
    let i = t.item(definition);
    t.call("WillowGame.WillowItem.ComputeValueOfParts", i).unwrap();
    assert_eq!(t.get(i, "RarityLevel"), Value::Int(4));
    assert_eq!(t.get(i, "MonetaryValue"), Value::Int(50));
    let mission = t.definition("WillowGame.MissionItemDefinition", 4.2, 1.0, 50.0, 0);
    let m = t.item(mission);
    assert!(t.call("WillowGame.WillowItem.ComputeValueOfParts", m).is_err());
}

/// Reference binds nothing (no C++ stand-in).
#[test]
fn reference_binds_nothing() {
    let t = Toy::new(ParityMode::Reference);
    for key in [WEAPON_VALUE, ITEM_VALUE] {
        assert!(!t.vm.has_native(key), "{key}");
    }
}
