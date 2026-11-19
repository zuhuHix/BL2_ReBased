//! The value natives on an invented class set (AGENTS.md rule 9; decision 2026-11-19-36). Expected values are written
//! by hand from NATIVE_INVENTORY_VALUE.md. Every init value here has scale 1 and no base attribute, so its value is its
//! constant whatever the evaluator's formula. The normalized damage tests (decision 2026-11-19-45) add two toy resolvers,
//! `Test.Same` (the context is the source) and `Test.IsWeapon` (40 on a weapon, an error on anything else).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, ContextResolver, EngineQuery, ValueResolver, WorldParts};
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm, WorldHooks};
use ow_vm::error::VmError;

use super::*;
use crate::equip::read_prop;

struct Toy {
    vm: Vm,
    aid: StructId,
    wdata: StructId,
    idata: StructId,
}

/// A toy context resolver: the context is the source itself.
struct Same;
impl ContextResolver for Same {
    fn resolve(
        &self,
        _: &AttributeSystem,
        _: ObjRef,
        _: ObjRef,
        source: Option<ObjRef>,
        _: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        Ok(source)
    }
}

/// A toy value resolver: 40 on a weapon context, a data error on any other.
struct IsWeapon(ClassId);
impl ValueResolver for IsWeapon {
    fn value(&self, _: &AttributeSystem, _: ObjRef, context: ObjRef, _: f32, cx: &mut EvalCx<'_>) -> Result<f32, AttrError> {
        let space = cx.world.parts().space;
        let class = space.heap.get(context)?.class;
        if space.classes.is_child_of(class, self.0) {
            Ok(40.0)
        } else {
            Err(AttrError::BadData { reason: "IsWeapon: the context is not a weapon".into() })
        }
    }
}

/// Toy world hooks: only the globals object, made once (the mission-item rarity tests).
struct GlobalsOnly(ObjRef);
impl WorldHooks for GlobalsOnly {
    fn globals(&mut self, _: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        Ok(self.0)
    }
    fn game_info(&mut self, _: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        Err(AttrError::BadData { reason: "GlobalsOnly has no game info".into() })
    }
    fn engine_query(&mut self, _: EngineQuery, _: ObjRef, _: WorldParts<'_>) -> Result<Option<ObjRef>, AttrError> {
        Err(AttrError::BadData { reason: "GlobalsOnly answers no engine query".into() })
    }
}

/// The weapon's normalized damage fields (section 5); `true` marks a stack (an array of modifier objects).
const NORMALIZED_FIELDS: [(&str, bool); 8] = [
    ("InstantHitDamageModifierStack", true),
    ("MeleeDamageModifierStack", true),
    ("NormalizedInstantHitDamageModifierStack", true),
    ("NormalizedMeleeDamageModifierStack", true),
    ("NormalizedInstantHitDamageBaseValue", false),
    ("NormalizedMeleeDamageBaseValue", false),
    ("NormalizedInstantHitDamage", false),
    ("NormalizedMeleeDamage", false),
];

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
        let chains = vec![decl(&mut s, "ContextResolverChain", objs.clone()), decl(&mut s, "ValueResolverChain", objs.clone())];
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
            decl(&mut s, "NormalizedInstantHitDamage", PropKind::Struct(aid)),
            decl(&mut s, "NormalizedMeleeDamage", PropKind::Struct(aid)),
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
        let mut wf = fields(&mut s, wdata);
        for (name, stack) in NORMALIZED_FIELDS {
            let kind = if stack { objs.clone() } else { PropKind::Float };
            wf.push(decl(&mut s, name, kind));
        }
        let weapon = s.classes.define("WillowGame.WillowWeapon", Some(inventory), wf).unwrap();
        let modifier = vec![decl(&mut s, "Type", PropKind::Byte { enum_: None }), decl(&mut s, "Value", PropKind::Float)];
        s.classes.define("Engine.AttributeModifier", Some(object), modifier).unwrap();
        s.classes.define("Test.Same", Some(object), vec![]).unwrap();
        s.classes.define("Test.IsWeapon", Some(object), vec![]).unwrap();
        let globals = vec![decl(&mut s, "MissionItemRarityLevel", PropKind::Int)];
        s.classes.define("Test.GlobalsDefinition", Some(object), globals).unwrap();
        s.classes.define("WillowGame.WillowGlobals", Some(object), vec![]).unwrap();
        let itf = fields(&mut s, idata);
        s.classes.define("WillowGame.WillowItem", Some(inventory), itf).unwrap();
        let attrs = AttributeSystem::standard()
            .unwrap()
            .with_context_resolver("Test.Same", Same)
            .unwrap()
            .with_value_resolver("Test.IsWeapon", IsWeapon(weapon))
            .unwrap();
        let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(attrs), mode);
        for (path, key) in [
            ("WillowGame.WillowWeapon.ComputeValueOfParts", WEAPON_VALUE),
            ("WillowGame.WillowItem.ComputeValueOfParts", ITEM_VALUE),
            ("WillowGame.WillowWeapon.RecomputeNormalizedDamageValues", NORMALIZED_DAMAGE),
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

    /// A modifier object of raw type `kind` (0 Scale, 1 PreAdd, 2 PostAdd) and value `v`.
    fn modifier(&mut self, name: &str, kind: u8, v: f32) -> Value {
        let m = self.obj("Engine.AttributeModifier", name);
        self.set(m, "Type", Value::Byte(kind));
        self.set(m, "Value", Value::Float(v));
        Value::Object(Some(m))
    }

    /// An init value read from `attr` (scale 1).
    fn init_attr(&mut self, attr: ObjRef) -> Value {
        let aid = self.aid;
        self.st(aid, vec![("BaseValueAttribute", Value::Object(Some(attr))), ("BaseValueScaleConstant", Value::Float(1.0))])
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
        self.item_with(definition, &[])
    }

    fn item_with(&mut self, definition: ObjRef, parts: &[(&str, ObjRef)]) -> ObjRef {
        let i = self.obj("WillowGame.WillowItem", "Item_0");
        let ty = self.idata;
        let mut fields = vec![("ItemDefinition", Value::Object(Some(definition)))];
        fields.extend(parts.iter().map(|(slot, p)| (*slot, Value::Object(Some(*p)))));
        let data = self.st(ty, fields);
        self.set(i, "DefinitionData", data);
        i
    }

    /// A world whose globals object answers `GetGlobalsDefinition` with whatever `answer` holds when called (a toy
    /// native in place of the installed function).
    fn with_globals(&mut self, answer: Rc<Cell<Option<ObjRef>>>) {
        let g = self.obj("WillowGame.WillowGlobals", "Globals");
        self.vm.set_world_hooks(Rc::new(RefCell::new(GlobalsOnly(g)))).unwrap();
        let path = ObjectPath::parse("WillowGame.WillowGlobals.GetGlobalsDefinition").unwrap();
        let key = "WillowGlobals.GetGlobalsDefinition()";
        let mut f = FunctionDecl::native(path, key);
        let object = self.class("Core.Object");
        f.result = Some(decl(&mut self.vm.space, "ReturnValue", PropKind::Object { class: object }).with_flags(PropertyDecl::RETURN));
        self.vm.declare_function(f).unwrap();
        self.vm.register_native(key, Rc::new(move |_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Object(answer.get())))).unwrap();
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

/// A mission item's rarity is the globals definition's `MissionItemRarityLevel`, whatever its own parts (an item part
/// and a title name part of rarity 9 here), and 0 when the globals give no definition. Its money and modifier total
/// still follow the rule (review of #425).
#[test]
fn a_mission_items_rarity_comes_from_the_globals_definition() {
    let mut t = Toy::new(ParityMode::Off);
    let answer = Rc::new(Cell::new(None));
    t.with_globals(answer.clone());
    let gd = t.obj("Test.GlobalsDefinition", "GlobalsDefinition");
    t.set(gd, "MissionItemRarityLevel", Value::Int(3));
    answer.set(Some(gd));
    let mission = t.definition("WillowGame.MissionItemDefinition", 4.2, 2.0, 50.0, 0);
    let alpha = t.part("Alpha", 9.0, Some(1.5));
    let title = t.part("Title", 9.0, None);
    let parts = [("AlphaItemPartDefinition", alpha), ("TitleItemNamePartDefinition", title)];
    let m = t.item_with(mission, &parts);
    t.call("WillowGame.WillowItem.ComputeValueOfParts", m).unwrap();
    assert_eq!(t.get(m, "RarityLevel"), Value::Int(3), "the globals definition's level, not 4 + 9 + 9");
    assert_eq!(t.get(m, "MonetaryValueModifierTotal"), Value::Float(3.0));
    assert_eq!(t.get(m, "MonetaryValue"), Value::Int(50));
    answer.set(None);
    let m = t.item_with(mission, &parts);
    t.call("WillowGame.WillowItem.ComputeValueOfParts", m).unwrap();
    assert_eq!(t.get(m, "RarityLevel"), Value::Int(0), "no globals definition: 0");
}

/// A weapon skips the mission rule: a weapon whose definition is (oddly) a mission item definition sums its rarity,
/// and asks for no globals (this toy has none, which would be an error).
#[test]
fn a_weapon_never_takes_the_mission_rule() {
    let mut t = Toy::new(ParityMode::Off);
    let mission = t.definition("WillowGame.MissionItemDefinition", 2.0, 1.0, 10.0, 0);
    let title = t.part("Title", 3.0, None);
    let w = t.weapon(Some(mission), &[("TitlePartDefinition", title)]);
    t.call("WillowGame.WillowWeapon.ComputeValueOfParts", w).unwrap();
    assert_eq!(t.get(w, "RarityLevel"), Value::Int(5));
}

/// Reference binds nothing (no C++ stand-in).
#[test]
fn reference_binds_nothing() {
    let t = Toy::new(ParityMode::Reference);
    for key in [WEAPON_VALUE, ITEM_VALUE, NORMALIZED_DAMAGE] {
        assert!(!t.vm.has_native(key), "{key}");
    }
}

/// `RecomputeNormalizedDamageValues` (section 5): each stack is copied; each base is the type definition's init value
/// evaluated with the weapon as context (the instant-hit one reads an attribute only a weapon context answers, 40);
/// each value is the base with the copied stack, as a float. Instant-hit: (40 + 2) x (1 + 0.5) = 63, a None element
/// and a modifier of type 7 skipped. Melee: the constant 10, plus 3 = 13.
#[test]
fn normalized_damage_uses_the_weapon_and_its_copied_stacks() {
    let mut t = Toy::new(ParityMode::Off);
    let definition = t.definition("Test.Definition", 0.0, 1.0, 0.0, 0);
    let same = t.obj("Test.Same", "Same");
    let is_weapon = t.obj("Test.IsWeapon", "IsWeapon");
    let attr = t.obj("Engine.AttributeDefinition", "NormalizedDamageAttr");
    t.set(attr, "ContextResolverChain", Value::array(vec![Value::Object(Some(same))]));
    t.set(attr, "ValueResolverChain", Value::array(vec![Value::Object(Some(is_weapon))]));
    let instant = t.init_attr(attr);
    t.set(definition, "NormalizedInstantHitDamage", instant);
    let melee = t.init(10.0);
    t.set(definition, "NormalizedMeleeDamage", melee);
    let w = t.weapon(Some(definition), &[]);
    let instant_stack = Value::array(vec![
        t.modifier("Scale", 0, 0.5),
        t.modifier("PreAdd", 1, 2.0),
        Value::Object(None),
        t.modifier("Unknown", 7, 100.0),
    ]);
    let melee_stack = Value::array(vec![t.modifier("PostAdd", 2, 3.0)]);
    t.set(w, "InstantHitDamageModifierStack", instant_stack.clone());
    t.set(w, "MeleeDamageModifierStack", melee_stack.clone());
    t.call("WillowGame.WillowWeapon.RecomputeNormalizedDamageValues", w).unwrap();
    assert_eq!(t.get(w, "NormalizedInstantHitDamageModifierStack"), instant_stack);
    assert_eq!(t.get(w, "NormalizedInstantHitDamageBaseValue"), Value::Float(40.0));
    assert_eq!(t.get(w, "NormalizedInstantHitDamage"), Value::Float(63.0));
    assert_eq!(t.get(w, "NormalizedMeleeDamageModifierStack"), melee_stack);
    assert_eq!(t.get(w, "NormalizedMeleeDamageBaseValue"), Value::Float(10.0));
    assert_eq!(t.get(w, "NormalizedMeleeDamage"), Value::Float(13.0));
}

/// A weapon without a type definition is `NotYet { NotRead }` (the note does not say what the native does then), and
/// nothing is written; an evaluation the evaluator cannot do refuses before any write too.
#[test]
fn normalized_damage_without_a_type_or_with_an_unread_attribute_writes_nothing() {
    let mut t = Toy::new(ParityMode::Off);
    let w = t.weapon(None, &[]);
    t.set(w, "NormalizedInstantHitDamage", Value::Float(7.0));
    let e = t.call("WillowGame.WillowWeapon.RecomputeNormalizedDamageValues", w).unwrap_err();
    let text = e.to_string();
    assert_eq!(not_yet_kind(&e), Some(Missing::NotRead), "{text}");
    assert!(text.contains("without a WeaponTypeDefinition"), "{text}");
    assert_eq!(t.get(w, "NormalizedInstantHitDamage"), Value::Float(7.0));
    // The melee base reads an attribute whose context resolver class is registered nowhere: the evaluator's NotYet,
    // after the instant-hit value was computed but before anything was written.
    let definition = t.definition("Test.Definition", 0.0, 1.0, 0.0, 0);
    let unread = t.obj("Core.Object", "Unread");
    let attr = t.obj("Engine.AttributeDefinition", "UnreadAttr");
    t.set(attr, "ContextResolverChain", Value::array(vec![Value::Object(Some(unread))]));
    let melee = t.init_attr(attr);
    t.set(definition, "NormalizedMeleeDamage", melee);
    let instant = t.init(5.0);
    t.set(definition, "NormalizedInstantHitDamage", instant);
    let w = t.weapon(Some(definition), &[]);
    t.set(w, "NormalizedInstantHitDamage", Value::Float(7.0));
    let e = t.call("WillowGame.WillowWeapon.RecomputeNormalizedDamageValues", w).unwrap_err();
    let text = e.to_string();
    assert_eq!(not_yet_kind(&e), Some(Missing::NotRead), "{text}");
    assert!(text.contains("context resolver class Core.Object"), "{text}");
    assert_eq!(t.get(w, "NormalizedInstantHitDamage"), Value::Float(7.0));
}

/// The kind of a `NotYet` error, or None for any other error.
fn not_yet_kind(e: &VmError) -> Option<Missing> {
    match e {
        VmError::NotYet(n) => Some(n.missing.clone()),
        _ => None,
    }
}
