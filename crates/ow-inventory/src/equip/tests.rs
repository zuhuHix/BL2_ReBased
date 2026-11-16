//! Tests of the equip rules and the two overrides on a VM whose classes are built by hand (AGENTS.md rule 9: invented
//! classes, objects and values only; the class and property names are the ones the rules read). The world module
//! `World` is shared with the manager and spawn tests.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AppliedEffect, Applied, EvalCx, InitData};
use ow_attr::stack::ModifierType;
use ow_core::{NotYet, ParityMode};
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind).with_flags(flags)
}

/// The invented world: the attribute classes the evaluator reads, the actor classes the resolvers and rules read, and
/// a few objects.
pub(crate) struct World {
    pub vm: Vm,
    pub controller: ObjRef,
    pub pawn: ObjRef,
    pub manager: ObjRef,
    /// A float stack attribute `Bonus` on the controller, as an attribute definition with the controller resolver.
    pub controller_bonus: ObjRef,
    /// A float stack attribute `Bonus` on the weapon, through the weapon resolver.
    pub weapon_bonus: ObjRef,
}

/// A stack attribute: the value, base and stack properties.
fn stack(space: &mut ObjectSpace, name: &str, int: bool, modifier: ClassId) -> Vec<PropertyDecl> {
    let kind = if int { PropKind::IntAttribute } else { PropKind::FloatAttribute };
    vec![
        decl(space, name, kind.clone(), PropertyDecl::STACK_ATTRIBUTE),
        decl(space, &format!("{name}BaseValue"), kind, PropertyDecl::PLAIN_ATTRIBUTE),
        decl(
            space,
            &format!("{name}ModifierStack"),
            PropKind::Array(Box::new(PropKind::Object { class: modifier })),
            0,
        ),
    ]
}

fn link(space: &mut ObjectSpace, class: ClassId, name: &str) {
    let v = space.names.intern(name);
    let b = space.names.intern(&format!("{name}BaseValue"));
    let s = space.names.intern(&format!("{name}ModifierStack"));
    space.classes.link_attribute(class, v, b, s).unwrap();
}

impl World {
    pub(crate) fn new() -> World {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = PropKind::Object { class: object };
        let objs = PropKind::Array(Box::new(obj.clone()));
        let byte = PropKind::Byte { enum_: None };
        let st = |space: &mut ObjectSpace, path: &str, fields: Vec<(&str, PropKind)>| {
            let decls = fields.into_iter().map(|(n, k)| decl(space, n, k, 0)).collect();
            space.classes.define_struct(path, None, decls).unwrap()
        };
        let aid = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.AttributeInitializationData",
            vec![
                ("BaseValueConstant", PropKind::Float),
                ("BaseValueAttribute", obj.clone()),
                ("InitializationDefinition", obj.clone()),
                ("BaseValueScaleConstant", PropKind::Float),
            ],
        );
        let effect = st(
            &mut space,
            "Engine.AttributeEffect.AttributeEffectData",
            vec![
                ("AttributeToModify", obj.clone()),
                ("ModifierType", byte.clone()),
                ("BaseModifierValue", PropKind::Struct(aid)),
            ],
        );
        let applied = st(
            &mut space,
            APPLIED_STRUCT,
            vec![
                ("Context", obj.clone()),
                ("Attribute", obj.clone()),
                ("Modifier", obj.clone()),
            ],
        );
        let slot = st(
            &mut space,
            "Test.AttributeSlotEffectData",
            vec![("bExternalSlot", PropKind::Bool), ("bRunEffectsAsSkill", PropKind::Bool)],
        );
        let class = |space: &mut ObjectSpace, path: &str, sup: ClassId, decls: Vec<PropertyDecl>| {
            space.classes.define(path, Some(sup), decls).unwrap()
        };
        let decls = vec![
            decl(&mut space, "ContextResolverChain", objs.clone(), 0),
            decl(&mut space, "ValueResolverChain", objs.clone(), 0),
        ];
        let base = class(&mut space, "Engine.AttributeDefinitionBase", object, decls);
        class(&mut space, "Engine.AttributeDefinition", base, vec![]);
        class(&mut space, "Engine.AttributeInitializationDefinition", object, vec![]);
        let decls = vec![decl(&mut space, "PropertyName", PropKind::Name, 0)];
        class(&mut space, "Engine.ObjectPropertyAttributeValueResolver", object, decls);
        class(&mut space, "Engine.ControllerAttributeContextResolver", object, vec![]);
        class(&mut space, "Engine.WeaponAttributeContextResolver", object, vec![]);
        // A context resolver class the attribute system has no entry for: resolving through it is NotYet.
        class(&mut space, "Test.UnreadContextResolver", object, vec![]);
        // A value resolver class with no entry: adding or removing a modifier through it is NotYet.
        class(&mut space, "Test.UnreadValueResolver", object, vec![]);
        // The flaky value resolver of the rollback test (registered by that test only).
        class(&mut space, "Test.FlakyValueResolver", object, vec![]);
        let decls = vec![decl(&mut space, "Type", byte.clone(), 0), decl(&mut space, "Value", PropKind::Float, 0)];
        let modifier = class(&mut space, "Core.AttributeModifier", object, decls);
        // Definitions: one class for item definitions, weapon types and parts.
        let list = |s: StructId| PropKind::Array(Box::new(PropKind::Struct(s)));
        let decls = vec![
            decl(&mut space, "ExternalAttributeEffects", list(effect), 0),
            decl(&mut space, "WeaponAttributeEffects", list(effect), 0),
            decl(&mut space, "AttributeSlotEffects", list(slot), 0),
        ];
        class(&mut space, "Test.Definition", object, decls);
        // Definition data: the weapon's and the item's part fields.
        let mut wfields = vec![("WeaponTypeDefinition", obj.clone())];
        wfields.extend(WEAPON_PART_FIELDS.iter().map(|f| (*f, obj.clone())));
        let wdata = st(&mut space, "Test.WeaponDefinitionData", wfields);
        let mut ifields = vec![("ItemDefinition", obj.clone())];
        ifields.extend(ITEM_PART_FIELDS.iter().map(|f| (*f, obj.clone())));
        let idata = st(&mut space, "Test.ItemDefinitionData", ifields);
        // Actors.
        let decls = vec![decl(&mut space, "Instigator", obj.clone(), 0)];
        let actor = class(&mut space, "Engine.Actor", object, decls);
        let mut decls = vec![decl(&mut space, "Pawn", obj.clone(), 0)];
        decls.extend(stack(&mut space, "Bonus", false, modifier));
        let controller_class = class(&mut space, "Engine.Controller", actor, decls);
        link(&mut space, controller_class, "Bonus");
        let decls = vec![
            decl(&mut space, "Controller", obj.clone(), 0),
            decl(&mut space, "InvManager", obj.clone(), 0),
            decl(&mut space, "Weapon", obj.clone(), 0),
        ];
        let pawn_class = class(&mut space, "Engine.Pawn", actor, decls);
        let decls = vec![decl(&mut space, "EquippedItems", obj.clone(), 0).with_array_dim(4)];
        let willow_pawn = class(&mut space, "WillowGame.WillowPawn", pawn_class, decls);
        let decls = vec![decl(&mut space, "InventoryChain", obj.clone(), 0)];
        let manager_class = class(&mut space, "Engine.InventoryManager", actor, decls);
        let mut decls = vec![
            decl(&mut space, "BackpackInventoryCount", PropKind::Int, 0),
            decl(&mut space, "InventorySlotMax_Misc", PropKind::Int, 0),
        ];
        decls.extend(stack(&mut space, "WeaponReadyMax", true, modifier));
        let willow_manager = class(&mut space, "WillowGame.WillowInventoryManager", manager_class, decls);
        link(&mut space, willow_manager, "WeaponReadyMax");
        let decls = vec![
            decl(&mut space, "Inventory", obj.clone(), 0),
            decl(&mut space, "SourceDefinitionName", PropKind::Name, 0),
            decl(&mut space, "bDropOnDeath", PropKind::Bool, 0),
        ];
        let inventory = class(&mut space, "Engine.Inventory", actor, decls);
        let decls = vec![decl(&mut space, "bReadied", PropKind::Bool, 0)];
        let willow_inventory = class(&mut space, "Engine.WillowInventory", inventory, decls);
        let engine_weapon = class(&mut space, "Engine.Weapon", willow_inventory, vec![]);
        let records = PropKind::Array(Box::new(PropKind::Struct(applied)));
        let mut decls = vec![
            decl(&mut space, "DefinitionData", PropKind::Struct(wdata), 0),
            decl(&mut space, "ExternalAttributeModifiers", records.clone(), 0),
        ];
        decls.extend(stack(&mut space, "Bonus", false, modifier));
        let weapon = class(&mut space, "WillowGame.WillowWeapon", engine_weapon, decls);
        link(&mut space, weapon, "Bonus");
        let decls = vec![
            decl(&mut space, "DefinitionData", PropKind::Struct(idata), 0),
            decl(&mut space, "ExternalAttributeModifiers", records.clone(), 0),
            decl(&mut space, "PartExternalAttributeModifiers", records, 0),
        ];
        class(&mut space, "WillowGame.WillowItem", willow_inventory, decls);
        let alloc = |space: &mut ObjectSpace, c: ClassId, name: &str| space.alloc(c, name, None, Origin::Runtime).unwrap();
        let controller = alloc(&mut space, controller_class, "Controller_0");
        let pawn = alloc(&mut space, willow_pawn, "Pawn_0");
        let manager = alloc(&mut space, willow_manager, "Manager_0");
        let vm = Vm::new(
            space,
            DefStore::new_empty(),
            Rc::new(AttributeSystem::standard().unwrap()),
            ParityMode::Off,
        );
        let mut w = World {
            vm,
            controller,
            pawn,
            manager,
            controller_bonus: controller,
            weapon_bonus: controller,
        };
        w.set(controller, "Pawn", Value::Object(Some(pawn)));
        w.set(pawn, "Controller", Value::Object(Some(controller)));
        w.set(pawn, "InvManager", Value::Object(Some(manager)));
        w.controller_bonus = w.attribute("Test.ControllerBonus", "Engine.ControllerAttributeContextResolver", "Bonus");
        w.weapon_bonus = w.attribute("Test.WeaponBonus", "Engine.WeaponAttributeContextResolver", "Bonus");
        w
    }

    pub(crate) fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }

    pub(crate) fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.class(class);
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    pub(crate) fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap_or_else(|| panic!("no property {name}"));
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    pub(crate) fn get(&self, o: ObjRef, name: &str) -> Value {
        read_prop(&self.vm, o, name).unwrap_or_else(|| panic!("no property {name}"))
    }

    pub(crate) fn st(&mut self, path: &str, fields: Vec<(&str, Value)>) -> Value {
        let ty = self.vm.space.classes.struct_by_path(path).unwrap();
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.vm.space.names.find(n).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = x;
        }
        v
    }

    /// An attribute on the controller's `Bonus` whose context resolver class is not registered: resolving it, so
    /// applying or removing a modifier through it, is `NotYet`.
    pub(crate) fn unread_attribute(&mut self) -> ObjRef {
        self.attribute("Test.UnreadBonus", "Test.UnreadContextResolver", "Bonus")
    }

    /// An attribute on the controller whose value resolver class is not registered.
    pub(crate) fn unread_value_attribute(&mut self) -> ObjRef {
        let c = self.obj("Engine.ControllerAttributeContextResolver", "Test.UnreadValue.Context");
        let k = self.obj("Test.UnreadValueResolver", "Test.UnreadValue.Value");
        let a = self.obj("Engine.AttributeDefinition", "Test.UnreadValue");
        self.set(a, "ContextResolverChain", Value::array(vec![Value::Object(Some(c))]));
        self.set(a, "ValueResolverChain", Value::array(vec![Value::Object(Some(k))]));
        a
    }

    pub(crate) fn attribute(&mut self, path: &str, context: &str, property: &str) -> ObjRef {
        let c = self.obj(context, &format!("{path}.Context"));
        let n = Value::Name(self.vm.space.names.intern(property));
        let k = self.obj("Engine.ObjectPropertyAttributeValueResolver", &format!("{path}.Value"));
        self.set(k, "PropertyName", n);
        let a = self.obj("Engine.AttributeDefinition", path);
        self.set(a, "ContextResolverChain", Value::array(vec![Value::Object(Some(c))]));
        self.set(a, "ValueResolverChain", Value::array(vec![Value::Object(Some(k))]));
        a
    }

    pub(crate) fn init(&mut self, constant: f32) -> Value {
        self.st(
            "Engine.AttributeInitializationDefinition.AttributeInitializationData",
            vec![
                ("BaseValueConstant", Value::Float(constant)),
                ("BaseValueScaleConstant", Value::Float(1.0)),
            ],
        )
    }

    pub(crate) fn effect(&mut self, attribute: Option<ObjRef>, kind: u8, constant: f32) -> Value {
        let init = self.init(constant);
        self.st(
            "Engine.AttributeEffect.AttributeEffectData",
            vec![
                ("AttributeToModify", Value::Object(attribute)),
                ("ModifierType", Value::Byte(kind)),
                ("BaseModifierValue", init),
            ],
        )
    }

    /// A definition holding `external` effects in `ExternalAttributeEffects`.
    pub(crate) fn definition(&mut self, name: &str, external: Vec<Value>) -> ObjRef {
        let d = self.obj("Test.Definition", name);
        self.set(d, "ExternalAttributeEffects", Value::array(external));
        d
    }

    /// A weapon held by the pawn, of type `weapon_type` with `parts` in its first part fields.
    pub(crate) fn weapon(&mut self, weapon_type: Option<ObjRef>, parts: &[ObjRef]) -> ObjRef {
        let w = self.obj("WillowGame.WillowWeapon", "Weapon_0");
        let mut fields = vec![("WeaponTypeDefinition", Value::Object(weapon_type))];
        for (f, p) in WEAPON_PART_FIELDS.iter().zip(parts) {
            fields.push((f, Value::Object(Some(*p))));
        }
        let data = self.st("Test.WeaponDefinitionData", fields);
        self.set(w, "DefinitionData", data);
        let pawn = self.pawn;
        self.set(w, "Instigator", Value::Object(Some(pawn)));
        w
    }

    /// An item held by the pawn, of definition `definition` with `parts` in its first part fields.
    pub(crate) fn item(&mut self, definition: Option<ObjRef>, parts: &[ObjRef]) -> ObjRef {
        let i = self.obj("WillowGame.WillowItem", "Item_0");
        let mut fields = vec![("ItemDefinition", Value::Object(definition))];
        for (f, p) in ITEM_PART_FIELDS.iter().zip(parts) {
            fields.push((f, Value::Object(Some(*p))));
        }
        let data = self.st("Test.ItemDefinitionData", fields);
        self.set(i, "DefinitionData", data);
        let pawn = self.pawn;
        self.set(i, "Instigator", Value::Object(Some(pawn)));
        i
    }

    pub(crate) fn float(&self, o: ObjRef, name: &str) -> f32 {
        match self.get(o, name) {
            Value::Float(x) => x,
            other => panic!("{name}: {other:?}"),
        }
    }

    pub(crate) fn records(&self, o: ObjRef, name: &str) -> Applied {
        let v = self.get(o, name);
        applied_from_values(&self.vm, &v).unwrap()
    }
}

pub(crate) fn missing(e: VmError) -> Missing {
    match e {
        VmError::NotYet(n) => n.missing,
        VmError::Attr(ow_attr::api::AttrError::NotYet(n)) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ---- conversions ----------------------------------------------------------------------------------------------

#[test]
fn an_effect_struct_reads_as_ow_attr_s_effect() {
    let mut w = World::new();
    let a = w.controller_bonus;
    let v = w.effect(Some(a), 2, 3.5);
    let e = effect_data(&w.vm, &v).unwrap();
    assert_eq!(e, EffectData::new(Some(a), ModifierType::PostAdd, InitData::constant(3.5)));
    let bad = w.effect(Some(a), 7, 1.0);
    let err = effect_data(&w.vm, &bad).unwrap_err();
    assert!(err.to_string().contains("outside the enum"), "{err}");
}

#[test]
fn records_round_trip_through_the_heap_struct_and_none_fields_are_left_out() {
    let mut w = World::new();
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let m = w.obj("Core.AttributeModifier", "M");
    let r = AppliedEffect {
        context: w.controller,
        attribute: w.controller_bonus,
        modifier: m,
    };
    let values = applied_values(&w.vm, ty, &Applied(vec![r])).unwrap();
    assert_eq!(values.len(), 1);
    let mut all = values.clone();
    all.push(w.st(APPLIED_STRUCT, vec![("Attribute", Value::Object(Some(m)))]));
    let back = applied_from_values(&w.vm, &Value::array(all)).unwrap();
    assert_eq!(back, Applied(vec![r]));
}

// ---- ApplyAttributeEffects -----------------------------------------------------------------------------------

/// Declares `ApplyAttributeEffects` and `CalculateWeaponBaseValues` as script functions (the loader would declare
/// them from the packages) and binds the overrides.
fn declare_and_bind(w: &mut World) -> (ow_object::ids::FuncId, ow_object::ids::FuncId) {
    let object = w.class("Core.Object");
    let applied = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let effect = w.vm.space.classes.struct_by_path("Engine.AttributeEffect.AttributeEffectData").unwrap();
    let params = vec![
        decl(&mut w.vm.space, "ContextSource", PropKind::Object { class: object }, PropertyDecl::PARAM),
        decl(
            &mut w.vm.space,
            "InAttributeEffects",
            PropKind::Array(Box::new(PropKind::Struct(effect))),
            PropertyDecl::PARAM | PropertyDecl::OUT,
        ),
        decl(
            &mut w.vm.space,
            "OutModifiers",
            PropKind::Array(Box::new(PropKind::Struct(applied))),
            PropertyDecl::PARAM | PropertyDecl::OUT,
        ),
        decl(
            &mut w.vm.space,
            "OptionalOverrideContextSource",
            PropKind::Object { class: object },
            PropertyDecl::PARAM | PropertyDecl::OPTIONAL,
        ),
    ];
    let path = ow_core::ObjectPath::parse(APPLY_ATTRIBUTE_EFFECTS).unwrap();
    let apply = w
        .vm
        .declare_function(FunctionDecl::script(path).with_static(true).with_params(params))
        .unwrap();
    let path = ow_core::ObjectPath::parse(CALCULATE_WEAPON_BASE_VALUES).unwrap();
    let weapon = w.class("WillowGame.WillowWeapon");
    let bases = w
        .vm
        .declare_function(FunctionDecl::script(path).with_owner(weapon))
        .unwrap();
    bind_overrides(&mut w.vm).unwrap();
    (apply, bases)
}

#[test]
fn the_apply_override_appends_its_records_to_the_out_array() {
    let mut w = World::new();
    let (apply, _) = declare_and_bind(&mut w);
    let a = w.controller_bonus;
    let effects = Value::array(vec![w.effect(Some(a), 2, 4.0), w.effect(None, 2, 9.0)]);
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let earlier = AppliedEffect {
        context: w.pawn,
        attribute: a,
        modifier: w.obj("Core.AttributeModifier", "Earlier"),
    };
    let existing = Value::array(applied_values(&w.vm, ty, &Applied(vec![earlier])).unwrap());
    let controller = w.controller;
    let r = w
        .vm
        .call(apply, None, vec![Value::Object(Some(controller)), effects, existing])
        .unwrap();
    let out = applied_from_values(&w.vm, &r.outs[2]).unwrap();
    assert_eq!(out.0.len(), 2, "the earlier record kept, one added (the None attribute leaves none)");
    assert_eq!(out.0[0], earlier);
    assert_eq!(out.0[1].context, controller);
    assert_eq!(w.float(controller, "Bonus"), 4.0, "PostAdd 4 on a 0 base");
}

#[test]
fn the_override_source_is_tried_first() {
    let mut w = World::new();
    let (apply, _) = declare_and_bind(&mut w);
    let weapon = w.weapon(None, &[]);
    let a = w.weapon_bonus;
    let effects = Value::array(vec![w.effect(Some(a), 2, 1.5)]);
    let controller = w.controller;
    // From the controller alone the weapon resolver finds the pawn's Weapon (None here): no record.
    let r = w
        .vm
        .call(apply, None, vec![Value::Object(Some(controller)), effects.clone(), Value::array(vec![])])
        .unwrap();
    assert!(applied_from_values(&w.vm, &r.outs[2]).unwrap().0.is_empty());
    // With the weapon as the override the effect lands on it.
    let r = w
        .vm
        .call(
            apply,
            None,
            vec![
                Value::Object(Some(controller)),
                effects,
                Value::array(vec![]),
                Value::Object(Some(weapon)),
            ],
        )
        .unwrap();
    let out = applied_from_values(&w.vm, &r.outs[2]).unwrap();
    assert_eq!(out.0.len(), 1);
    assert_eq!(out.0[0].context, weapon);
    assert_eq!(w.float(weapon, "Bonus"), 1.5);
}

#[test]
fn bind_overrides_refuses_a_vm_that_lacks_the_script_functions() {
    let mut w = World::new();
    assert!(bind_overrides(&mut w.vm).is_err());
}

#[test]
fn calculate_weapon_base_values_without_a_type_is_not_read() {
    let mut w = World::new();
    let (_, bases) = declare_and_bind(&mut w);
    let weapon = w.weapon(None, &[]);
    let e = w.vm.call(bases, Some(weapon), vec![]).unwrap_err();
    assert!(e.to_string().contains("without a WeaponTypeDefinition"), "{e}");
    assert_eq!(missing(e), Missing::NotRead);
    // With a type the override reaches ow-items' rule, which reads the type's fields: this invented type has none.
    let t = w.definition("Test.Type", vec![]);
    let weapon = w.weapon(Some(t), &[]);
    let e = w.vm.call(bases, Some(weapon), vec![]).unwrap_err();
    assert!(e.to_string().contains("declares no"), "{e}");
}

#[test]
fn the_base_values_recompute_in_the_vm_s_let_form() {
    assert_eq!(let_forms().combine, CombineForm::Vm5F);
    assert_eq!(let_forms().pow, NumForms::default().pow);
}

// ---- equip and unequip ---------------------------------------------------------------------------------------

#[test]
fn the_equip_controller_is_the_instigator_s_controller() {
    let mut w = World::new();
    let item = w.item(None, &[]);
    assert_eq!(equip_controller(&mut w.vm, item).unwrap(), Some(w.controller));
    let loose = w.obj("WillowGame.WillowItem", "Loose");
    assert_eq!(equip_controller(&mut w.vm, loose).unwrap(), None, "no instigator");
}

#[test]
fn without_a_controller_the_manager_s_owning_controller_is_asked() {
    let mut w = World::new();
    let pawn = w.pawn;
    w.set(pawn, "Controller", Value::Object(None));
    let item = w.item(None, &[]);
    // A hand-built manager class declares no GetOwningPlayerController: the lookup is the script's, so it is asked.
    let e = equip_controller(&mut w.vm, item).unwrap_err();
    assert!(e.to_string().contains("GetOwningPlayerController"), "{e}");
    // A manager that is not a WillowInventoryManager gives none.
    let other = w.obj("Engine.InventoryManager", "Plain");
    w.set(pawn, "InvManager", Value::Object(Some(other)));
    assert_eq!(equip_controller(&mut w.vm, item).unwrap(), None);
}

#[test]
fn an_item_applies_its_definition_s_then_its_parts_effects_and_keeps_the_records() {
    let mut w = World::new();
    let a = w.controller_bonus;
    let e1 = w.effect(Some(a), 2, 2.0);
    let def = w.definition("Test.Shield", vec![e1]);
    let e2 = w.effect(Some(a), 2, 0.5);
    let part = w.definition("Test.Part", vec![e2]);
    let item = w.item(Some(def), &[part]);
    let out = apply_item_external_effects(&mut w.vm, item).unwrap();
    assert_eq!(out, ExternalOutcome::Applied { records: 1, part_records: 1 });
    let c = w.controller;
    assert_eq!(w.float(c, "Bonus"), 2.5);
    assert_eq!(w.records(item, "ExternalAttributeModifiers").0[0].context, c);
    // The guard: applying again does nothing.
    assert_eq!(apply_item_external_effects(&mut w.vm, item).unwrap(), ExternalOutcome::AlreadyApplied);
    assert_eq!(w.float(c, "Bonus"), 2.5);
    // Unequip removes both lists' modifiers and empties them.
    remove_item_external_effects(&mut w.vm, item).unwrap();
    assert_eq!(w.float(c, "Bonus"), 0.0);
    assert!(w.records(item, "ExternalAttributeModifiers").0.is_empty());
    assert!(w.records(item, "PartExternalAttributeModifiers").0.is_empty());
}

#[test]
fn an_item_whose_part_records_exist_applies_only_its_definition() {
    let mut w = World::new();
    let a = w.controller_bonus;
    let e1 = w.effect(Some(a), 2, 2.0);
    let def = w.definition("Test.Shield", vec![e1]);
    let e2 = w.effect(Some(a), 2, 0.5);
    let part = w.definition("Test.Part", vec![e2]);
    let item = w.item(Some(def), &[part]);
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let held = AppliedEffect {
        context: w.pawn,
        attribute: a,
        modifier: w.obj("Core.AttributeModifier", "Held"),
    };
    let v = Value::array(applied_values(&w.vm, ty, &Applied(vec![held])).unwrap());
    w.set(item, "PartExternalAttributeModifiers", v);
    let out = apply_item_external_effects(&mut w.vm, item).unwrap();
    assert_eq!(out, ExternalOutcome::Applied { records: 1, part_records: 0 });
    let c = w.controller;
    assert_eq!(w.float(c, "Bonus"), 2.0);
}

#[test]
fn an_external_slot_is_not_yet_and_a_skill_or_internal_slot_is_nothing_to_apply() {
    let mut w = World::new();
    let def = w.definition("Test.ClassMod", vec![]);
    let slot = |w: &mut World, external, skill| {
        w.st(
            "Test.AttributeSlotEffectData",
            vec![("bExternalSlot", Value::Bool(external)), ("bRunEffectsAsSkill", Value::Bool(skill))],
        )
    };
    let quiet = vec![slot(&mut w, false, false), slot(&mut w, true, true)];
    w.set(def, "AttributeSlotEffects", Value::array(quiet));
    let item = w.item(Some(def), &[]);
    assert!(apply_item_external_effects(&mut w.vm, item).is_ok());
    let def2 = w.definition("Test.ClassMod2", vec![]);
    let loud = vec![slot(&mut w, true, false)];
    w.set(def2, "AttributeSlotEffects", Value::array(loud));
    let item2 = w.item(Some(def2), &[]);
    assert_eq!(
        missing(apply_item_external_effects(&mut w.vm, item2).unwrap_err()),
        Missing::NotRead
    );
}

#[test]
fn a_weapon_applies_its_type_s_and_parts_effects_with_the_instigator_s_controller() {
    let mut w = World::new();
    let a = w.weapon_bonus;
    let c = w.controller_bonus;
    let e1 = w.effect(Some(a), 2, 1.0);
    let e2 = w.effect(Some(c), 2, 3.0);
    let t = w.definition("Test.Type", vec![e1, e2]);
    let e3 = w.effect(Some(a), 2, 0.25);
    let part = w.definition("Test.Barrel", vec![e3]);
    let weapon = w.weapon(Some(t), &[part]);
    let out = apply_weapon_external_effects(&mut w.vm, weapon).unwrap();
    assert_eq!(out, ExternalOutcome::Applied { records: 3, part_records: 0 });
    let controller = w.controller;
    assert_eq!(w.float(weapon, "Bonus"), 1.25, "the override (the weapon) wins for its own attribute");
    assert_eq!(w.float(controller, "Bonus"), 3.0);
    assert_eq!(apply_weapon_external_effects(&mut w.vm, weapon).unwrap(), ExternalOutcome::AlreadyApplied);
    remove_weapon_external_effects(&mut w.vm, weapon).unwrap();
    assert_eq!(w.float(weapon, "Bonus"), 0.0);
    assert_eq!(w.float(controller, "Bonus"), 0.0);
    assert!(w.records(weapon, "ExternalAttributeModifiers").0.is_empty());
}

#[test]
fn a_weapon_without_an_instigator_applies_with_no_source() {
    let mut w = World::new();
    let a = w.weapon_bonus;
    let c = w.controller_bonus;
    let e1 = w.effect(Some(a), 2, 1.0);
    let e2 = w.effect(Some(c), 2, 3.0);
    let t = w.definition("Test.Type", vec![e1, e2]);
    let weapon = w.weapon(Some(t), &[]);
    w.set(weapon, "Instigator", Value::Object(None));
    let out = apply_weapon_external_effects(&mut w.vm, weapon).unwrap();
    // The weapon's own attribute through the override; the controller's has no context.
    assert_eq!(out, ExternalOutcome::Applied { records: 1, part_records: 0 });
    let controller = w.controller;
    assert_eq!(w.float(controller, "Bonus"), 0.0);
}

#[test]
fn item_errors_keep_their_kinds_in_the_vm() {
    let n = item_error("x", ItemError::NotYet(NotYet::not_read("a", "b")));
    assert!(matches!(n, VmError::NotYet(_)));
    let b = item_error("x", ItemError::BadData("why".into()));
    assert!(matches!(b, VmError::Script { .. }));
    let _ = EvalCx::new;
}

// ---- review r1: all or nothing ---------------------------------------------------------------------------------

#[test]
fn a_failed_item_apply_takes_back_what_it_added_and_a_retry_fails_again() {
    let mut w = World::new();
    let good = w.controller_bonus;
    let unread = w.unread_attribute();
    let e1 = w.effect(Some(good), 2, 2.0);
    let def = w.definition("Test.Shield", vec![e1]);
    let e2 = w.effect(Some(good), 2, 0.5);
    let e3 = w.effect(Some(unread), 2, 1.0);
    let part = w.definition("Test.Part", vec![e2, e3]);
    let item = w.item(Some(def), &[part]);
    let c = w.controller;
    let before = w.get(c, "BonusModifierStack");
    // The definition's effect and the part's first effect are applied, then the part's second one fails.
    assert_eq!(missing(apply_item_external_effects(&mut w.vm, item).unwrap_err()), Missing::NotRead);
    assert_eq!(w.float(c, "Bonus"), 0.0, "the stack is back as it was");
    assert_eq!(w.get(c, "BonusModifierStack"), before);
    assert!(w.records(item, "ExternalAttributeModifiers").0.is_empty());
    assert!(w.records(item, "PartExternalAttributeModifiers").0.is_empty());
    // The retry is not answered AlreadyApplied: it fails the same way and still leaves nothing.
    assert_eq!(missing(apply_item_external_effects(&mut w.vm, item).unwrap_err()), Missing::NotRead);
    assert_eq!(w.float(c, "Bonus"), 0.0);
}

#[test]
fn an_external_slot_refuses_before_any_effect_is_applied() {
    let mut w = World::new();
    let good = w.controller_bonus;
    let e1 = w.effect(Some(good), 2, 2.0);
    let def = w.definition("Test.ClassMod", vec![e1]);
    let slot = w.st(
        "Test.AttributeSlotEffectData",
        vec![("bExternalSlot", Value::Bool(true)), ("bRunEffectsAsSkill", Value::Bool(false))],
    );
    w.set(def, "AttributeSlotEffects", Value::array(vec![slot]));
    let e2 = w.effect(Some(good), 2, 0.5);
    let part = w.definition("Test.Part", vec![e2]);
    let item = w.item(Some(def), &[part]);
    let c = w.controller;
    let before = w.get(c, "BonusModifierStack");
    assert_eq!(missing(apply_item_external_effects(&mut w.vm, item).unwrap_err()), Missing::NotRead);
    assert_eq!(w.float(c, "Bonus"), 0.0);
    assert_eq!(w.get(c, "BonusModifierStack"), before, "the stack is unchanged after the Err");
    assert!(w.records(item, "ExternalAttributeModifiers").0.is_empty());
    assert_eq!(missing(apply_item_external_effects(&mut w.vm, item).unwrap_err()), Missing::NotRead);
}

#[test]
fn a_failed_weapon_apply_takes_back_what_it_added() {
    let mut w = World::new();
    let a = w.weapon_bonus;
    let c = w.controller_bonus;
    let unread = w.unread_attribute();
    let e1 = w.effect(Some(a), 2, 1.0);
    let e2 = w.effect(Some(c), 2, 3.0);
    let t = w.definition("Test.Type", vec![e1, e2]);
    let e3 = w.effect(Some(unread), 2, 1.0);
    let part = w.definition("Test.Barrel", vec![e3]);
    let weapon = w.weapon(Some(t), &[part]);
    assert_eq!(missing(apply_weapon_external_effects(&mut w.vm, weapon).unwrap_err()), Missing::NotRead);
    let controller = w.controller;
    assert_eq!(w.float(weapon, "Bonus"), 0.0);
    assert_eq!(w.float(controller, "Bonus"), 0.0);
    assert!(w.records(weapon, "ExternalAttributeModifiers").0.is_empty());
}

#[test]
fn unequip_tries_every_record_and_empties_both_arrays_before_it_reports_the_first_error() {
    let mut w = World::new();
    let good = w.controller_bonus;
    let e1 = w.effect(Some(good), 2, 2.0);
    let def = w.definition("Test.Shield", vec![e1]);
    let e2 = w.effect(Some(good), 2, 0.5);
    let part = w.definition("Test.Part", vec![e2]);
    let item = w.item(Some(def), &[part]);
    apply_item_external_effects(&mut w.vm, item).unwrap();
    let c = w.controller;
    assert_eq!(w.float(c, "Bonus"), 2.5);
    // A record that cannot be removed (its attribute's value resolver is not read) goes first in the item's own list.
    let unread = w.unread_value_attribute();
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let stray = AppliedEffect {
        context: c,
        attribute: unread,
        modifier: w.obj("Core.AttributeModifier", "Stray"),
    };
    let mut own = w.records(item, "ExternalAttributeModifiers");
    own.0.insert(0, stray);
    let v = Value::array(applied_values(&w.vm, ty, &own).unwrap());
    w.set(item, "ExternalAttributeModifiers", v);
    assert_eq!(missing(remove_item_external_effects(&mut w.vm, item).unwrap_err()), Missing::NotRead);
    assert_eq!(w.float(c, "Bonus"), 0.0, "the records after the failing one were removed");
    assert!(w.records(item, "ExternalAttributeModifiers").0.is_empty());
    assert!(w.records(item, "PartExternalAttributeModifiers").0.is_empty());
}

// ---- review r2: a rollback that fails, and what a rollback keeps -----------------------------------------------

/// A writable value resolver over the context's `Bonus` whose storage fails once `broken` is set; reading its value
/// sets it. So an effect whose value reads it breaks the resolver for every later add or remove.
struct Flaky {
    broken: Rc<std::cell::Cell<bool>>,
}

impl ow_attr::api::ValueResolver for Flaky {
    fn value(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        _cx: &mut EvalCx<'_>,
    ) -> Result<f32, ow_attr::api::AttrError> {
        self.broken.set(true);
        Ok(0.0)
    }

    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ow_attr::api::Storage>, ow_attr::api::AttrError> {
        if self.broken.get() {
            return Err(NotYet::not_read("the flaky test resolver, once broken", "a test").into());
        }
        let parts = cx.world.parts();
        let class = parts.space.heap.get(context)?.class;
        let p = parts.space.prop(class, "Bonus").expect("Bonus");
        ow_attr::storage::storage_of(parts.space, context, p)
    }
}

fn live_modifiers(w: &World) -> usize {
    let c = w.class("Core.AttributeModifier");
    w.vm.space.live(c).count()
}

fn stack_len(w: &World, o: ObjRef) -> usize {
    match w.get(o, "BonusModifierStack") {
        Value::Array(a) => a.len(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_rollback_that_fails_is_reported_naming_both_errors() {
    let mut w = World::new();
    let broken = Rc::new(std::cell::Cell::new(false));
    let attrs = AttributeSystem::standard()
        .unwrap()
        .with_value_resolver("Test.FlakyValueResolver", Flaky { broken: broken.clone() })
        .unwrap();
    w.vm.attrs = Rc::new(attrs);
    // The flaky attribute: the controller's Bonus through the flaky resolver.
    let ctx = w.obj("Engine.ControllerAttributeContextResolver", "Test.Flaky.Context");
    let k = w.obj("Test.FlakyValueResolver", "Test.Flaky.Value");
    let flaky = w.obj("Engine.AttributeDefinition", "Test.Flaky");
    w.set(flaky, "ContextResolverChain", Value::array(vec![Value::Object(Some(ctx))]));
    w.set(flaky, "ValueResolverChain", Value::array(vec![Value::Object(Some(k))]));
    // The definition's effect adds through the flaky resolver (it works: nothing broke it yet). The part's effect, in
    // a later apply call, reads the flaky attribute for its value, which breaks the resolver, and its own context
    // does not resolve: that call fails, and the equip's rollback of the definition's effect fails on the broken
    // storage. (Within one apply call ow-attr cleans up itself; this test is about the equip's own rollback.)
    let e1 = w.effect(Some(flaky), 2, 2.0);
    let unread = w.unread_attribute();
    let mut e2 = w.effect(Some(unread), 2, 1.0);
    let reads_flaky = w.st(
        "Engine.AttributeInitializationDefinition.AttributeInitializationData",
        vec![
            ("BaseValueAttribute", Value::Object(Some(flaky))),
            ("BaseValueScaleConstant", Value::Float(1.0)),
        ],
    );
    let at = {
        let ty = w.vm.space.classes.struct_by_path("Engine.AttributeEffect.AttributeEffectData").unwrap();
        let id = w.vm.space.names.find("BaseModifierValue").unwrap();
        w.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0 as usize
    };
    e2.struct_mut().unwrap().fields[at] = reads_flaky;
    let def = w.definition("Test.Shield", vec![e1]);
    let part = w.definition("Test.Part", vec![e2]);
    let item = w.item(Some(def), &[part]);
    let c = w.controller;
    let e = apply_item_external_effects(&mut w.vm, item).unwrap_err();
    let text = e.to_string();
    assert!(text.contains("the cleanup after a failure failed"), "{text}");
    assert!(text.contains("the flaky test resolver"), "the rollback's error: {text}");
    assert!(text.contains("UnreadContextResolver"), "the apply's error: {text}");
    assert!(!matches!(e, VmError::NotYet(_)), "the rollback error outranks the apply's NotYet");
    // The inconsistent state the error reports: the first modifier is still on the stack, and no record names it.
    assert_eq!(stack_len(&w, c), 1);
    assert_eq!(w.float(c, "Bonus"), 2.0);
    assert!(w.records(item, "ExternalAttributeModifiers").0.is_empty());
}

#[test]
fn a_rollback_frees_the_modifiers_it_removed() {
    let mut w = World::new();
    let good = w.controller_bonus;
    let unread = w.unread_attribute();
    let e1 = w.effect(Some(good), 2, 2.0);
    let e2 = w.effect(Some(unread), 2, 1.0);
    let def = w.definition("Test.Shield", vec![e1]);
    let part = w.definition("Test.Part", vec![e2]);
    let item = w.item(Some(def), &[part]);
    let before = live_modifiers(&w);
    assert!(apply_item_external_effects(&mut w.vm, item).is_err());
    assert_eq!(live_modifiers(&w), before, "the definition's modifier was freed with its removal");
}

#[test]
fn a_failed_equip_of_one_item_leaves_another_item_s_modifier_in_place() {
    let mut w = World::new();
    let good = w.controller_bonus;
    let e1 = w.effect(Some(good), 2, 2.0);
    let def_a = w.definition("Test.ShieldA", vec![e1]);
    let a = w.item(Some(def_a), &[]);
    apply_item_external_effects(&mut w.vm, a).unwrap();
    let c = w.controller;
    let a_modifier = w.records(a, "ExternalAttributeModifiers").0[0].modifier;
    let unread = w.unread_attribute();
    let e2 = w.effect(Some(good), 2, 0.5);
    let e3 = w.effect(Some(unread), 2, 1.0);
    let def_b = w.definition("Test.ShieldB", vec![e2, e3]);
    let b = w.obj("WillowGame.WillowItem", "Item_B");
    let data = w.st("Test.ItemDefinitionData", vec![("ItemDefinition", Value::Object(Some(def_b)))]);
    w.set(b, "DefinitionData", data);
    let pawn = w.pawn;
    w.set(b, "Instigator", Value::Object(Some(pawn)));
    assert!(apply_item_external_effects(&mut w.vm, b).is_err());
    assert_eq!(w.float(c, "Bonus"), 2.0, "A's value");
    assert_eq!(w.get(c, "BonusModifierStack"), Value::array(vec![Value::Object(Some(a_modifier))]));
    assert!(w.records(b, "ExternalAttributeModifiers").0.is_empty());
}

#[test]
fn existing_part_records_stay_identical_after_a_failure() {
    let mut w = World::new();
    let good = w.controller_bonus;
    let unread = w.unread_attribute();
    let e1 = w.effect(Some(good), 2, 2.0);
    let e2 = w.effect(Some(unread), 2, 1.0);
    let def = w.definition("Test.Shield", vec![e1, e2]);
    let item = w.item(Some(def), &[]);
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let held = AppliedEffect {
        context: w.pawn,
        attribute: good,
        modifier: w.obj("Core.AttributeModifier", "Held"),
    };
    let parts = Value::array(applied_values(&w.vm, ty, &Applied(vec![held])).unwrap());
    w.set(item, "PartExternalAttributeModifiers", parts.clone());
    assert!(apply_item_external_effects(&mut w.vm, item).is_err());
    assert_eq!(w.get(item, "PartExternalAttributeModifiers"), parts);
    assert!(w.records(item, "ExternalAttributeModifiers").0.is_empty());
}
