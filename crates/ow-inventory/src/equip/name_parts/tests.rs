//! Tests of the name-part rule, the card refusal and the preflight on a VM whose classes are built by hand (AGENTS.md
//! rule 9: invented classes, objects and values only; the class and property names are the ones the rules read).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind)
}

/// What a fixture changes from the declared schema (each a malformed declaration the schema check refuses).
#[derive(Clone, Copy, PartialEq)]
enum Schema {
    Good,
    /// `Priority` declared an int.
    IntPriority,
    /// `TitleList` on the part type an array of plain objects.
    PlainTitleList,
    /// `PrefixPartDefinition` typed as a plain object.
    PlainPrefixField,
    /// The weapon declares no `ExpLevel`.
    NoExpLevel,
    /// The definition data struct's own name only ends with `WeaponDefinitionData` (review r1 of #506, item 4).
    SuffixStruct,
}

struct Fx {
    vm: Vm,
    weapon: ObjRef,
    /// Attributes for expressions: `Bonus` on the weapon (5) and one whose context resolver is not registered.
    bonus: ObjRef,
    unread: ObjRef,
    n: u32,
}

impl Fx {
    fn new(schema: Schema) -> Fx {
        Fx::in_mode(schema, ParityMode::Off)
    }

    fn in_mode(schema: Schema, mode: ParityMode) -> Fx {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = PropKind::Object { class: object };
        let objs = PropKind::Array(Box::new(obj.clone()));
        let byte = PropKind::Byte { enum_: None };
        let class = |space: &mut ObjectSpace, path: &str, sup: ClassId, decls: Vec<PropertyDecl>| {
            space.classes.define(path, Some(sup), decls).unwrap()
        };
        // The attribute classes an expression reads.
        let decls = vec![decl(&mut space, "ContextResolverChain", objs.clone()), decl(&mut space, "ValueResolverChain", objs.clone())];
        let base = class(&mut space, "Engine.AttributeDefinitionBase", object, decls);
        class(&mut space, "Engine.AttributeDefinition", base, vec![]);
        let decls = vec![decl(&mut space, "PropertyName", PropKind::Name)];
        class(&mut space, "Engine.ObjectPropertyAttributeValueResolver", object, decls);
        class(&mut space, "Engine.WeaponAttributeContextResolver", object, vec![]);
        class(&mut space, "Test.UnreadContextResolver", object, vec![]);
        let expr_fields = vec![
            decl(&mut space, "AttributeOperand1", obj.clone()),
            decl(&mut space, "ComparisonOperator", byte.clone()),
            decl(&mut space, "Operand2Usage", byte.clone()),
            decl(&mut space, "AttributeOperand2", obj.clone()),
            decl(&mut space, "ConstantOperand2", PropKind::Float),
        ];
        let expr = space.classes.define_struct("Test.AttributeExpressionData", None, expr_fields).unwrap();
        // The name part, then the type and part classes holding its lists.
        let priority = if schema == Schema::IntPriority { PropKind::Int } else { PropKind::Float };
        let decls = vec![
            decl(&mut space, "MinExpLevelRequirement", PropKind::Int),
            decl(&mut space, "MaxExpLevelRequirement", PropKind::Int),
            decl(&mut space, "Priority", priority),
            decl(&mut space, "Expressions", PropKind::Array(Box::new(PropKind::Struct(expr)))),
        ];
        let name_part = class(&mut space, NAME_PART_CLASS, object, decls);
        let np = PropKind::Object { class: name_part };
        let list = PropKind::Array(Box::new(np.clone()));
        let decls = vec![decl(&mut space, "PrefixList", list.clone()), decl(&mut space, "TitleList", list.clone())];
        class(&mut space, WEAPON_TYPE_CLASS, object, decls);
        let title = if schema == Schema::PlainTitleList { objs.clone() } else { list };
        let decls = vec![decl(&mut space, "PrefixList", np_list(np.clone())), decl(&mut space, "TitleList", title)];
        class(&mut space, WEAPON_PART_CLASS, object, decls);
        // The weapon and its definition data.
        let mut fields = vec![decl(&mut space, "WeaponTypeDefinition", obj.clone())];
        for f in WEAPON_PART_FIELDS {
            fields.push(decl(&mut space, f, obj.clone()));
        }
        let prefix = if schema == Schema::PlainPrefixField { obj.clone() } else { np.clone() };
        fields.push(decl(&mut space, "PrefixPartDefinition", prefix));
        fields.push(decl(&mut space, "TitlePartDefinition", np));
        let data_path = if schema == Schema::SuffixStruct { "Test.FooWeaponDefinitionData" } else { "Test.WeaponDefinitionData" };
        let data = space.classes.define_struct(data_path, None, fields).unwrap();
        let actor = class(&mut space, "Engine.Actor", object, vec![]);
        let engine_weapon = class(&mut space, "Engine.Weapon", actor, vec![]);
        let mut decls = vec![decl(&mut space, "DefinitionData", PropKind::Struct(data))];
        if schema != Schema::NoExpLevel {
            decls.push(decl(&mut space, "ExpLevel", PropKind::Int));
        }
        // `Bonus`: a float stack attribute (value, base, stack), as the attribute layer reads one.
        let decls_m = vec![decl(&mut space, "Type", byte.clone()), decl(&mut space, "Value", PropKind::Float)];
        let modifier = class(&mut space, "Core.AttributeModifier", object, decls_m);
        decls.push(decl(&mut space, "Bonus", PropKind::FloatAttribute).with_flags(PropertyDecl::STACK_ATTRIBUTE));
        decls.push(decl(&mut space, "BonusBaseValue", PropKind::FloatAttribute).with_flags(PropertyDecl::PLAIN_ATTRIBUTE));
        decls.push(decl(&mut space, "BonusModifierStack", PropKind::Array(Box::new(PropKind::Object { class: modifier }))));
        let weapon_class = class(&mut space, WEAPON_CLASS, engine_weapon, decls);
        let (v, b, k) = (space.names.intern("Bonus"), space.names.intern("BonusBaseValue"), space.names.intern("BonusModifierStack"));
        space.classes.link_attribute(weapon_class, v, b, k).unwrap();
        let weapon = space.alloc(weapon_class, "Weapon_0", None, Origin::Runtime).unwrap();
        let vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::standard().unwrap()), mode);
        let mut x = Fx { vm, weapon, bonus: weapon, unread: weapon, n: 0 };
        if schema != Schema::NoExpLevel {
            x.set(weapon, "ExpLevel", Value::Int(10));
        }
        x.set(weapon, "Bonus", Value::Float(5.0));
        x.set(weapon, "BonusBaseValue", Value::Float(5.0));
        x.bonus = x.attribute("Engine.WeaponAttributeContextResolver");
        x.unread = x.attribute("Test.UnreadContextResolver");
        x
    }

    fn obj(&mut self, class: &str) -> ObjRef {
        self.n += 1;
        let c = self.vm.space.classes.by_path(class).unwrap();
        self.vm.space.alloc(c, &format!("Test_{}", self.n), None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap_or_else(|| panic!("no property {name}"));
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// An attribute reading the weapon's `Bonus` through context resolver class `context`.
    fn attribute(&mut self, context: &str) -> ObjRef {
        let c = self.obj(context);
        let k = self.obj("Engine.ObjectPropertyAttributeValueResolver");
        let n = Value::Name(self.vm.space.names.intern("Bonus"));
        self.set(k, "PropertyName", n);
        let a = self.obj("Engine.AttributeDefinition");
        self.set(a, "ContextResolverChain", Value::array(vec![Value::Object(Some(c))]));
        self.set(a, "ValueResolverChain", Value::array(vec![Value::Object(Some(k))]));
        a
    }

    /// `attribute > constant` (ComparisonOperator 4, no second attribute).
    fn above(&mut self, attribute: Option<ObjRef>, constant: f32) -> Value {
        let ty = self.vm.space.classes.struct_by_path("Test.AttributeExpressionData").unwrap();
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let at = |name: &str| layout.field(self.vm.space.names.find(name).unwrap()).unwrap().0 as usize;
        let (a, c, k) = (at("AttributeOperand1"), at("ComparisonOperator"), at("ConstantOperand2"));
        let s = v.struct_mut().unwrap();
        s.fields[a] = Value::Object(attribute);
        s.fields[c] = Value::Byte(4);
        s.fields[k] = Value::Float(constant);
        v
    }

    /// A name part for levels `min..=max` with `priority` and `exprs`.
    fn part_name(&mut self, min: i32, max: i32, priority: f32, exprs: Vec<Value>) -> ObjRef {
        let p = self.obj(NAME_PART_CLASS);
        self.set(p, "MinExpLevelRequirement", Value::Int(min));
        self.set(p, "MaxExpLevelRequirement", Value::Int(max));
        self.set(p, "Priority", Value::Float(priority));
        self.set(p, "Expressions", Value::array(exprs));
        p
    }

    /// A definition of `class` with these prefix and title lists (None entries kept).
    fn holder(&mut self, class: &str, prefixes: &[Option<ObjRef>], titles: &[Option<ObjRef>]) -> ObjRef {
        let h = self.obj(class);
        let list = |l: &[Option<ObjRef>]| Value::array(l.iter().map(|o| Value::Object(*o)).collect());
        self.set(h, "PrefixList", list(prefixes));
        self.set(h, "TitleList", list(titles));
        h
    }

    /// Sets the weapon's definition data: `weapon_type`, the part slots in order, and the two current picks.
    fn data(&mut self, weapon_type: Option<ObjRef>, parts: &[Option<ObjRef>], current: (Option<ObjRef>, Option<ObjRef>)) {
        let ty = self.vm.space.classes.struct_by_path("Test.WeaponDefinitionData").unwrap();
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let mut put = |name: &str, o: Option<ObjRef>| {
            let at = layout.field(self.vm.space.names.find(name).unwrap()).unwrap().0 as usize;
            v.struct_mut().unwrap().fields[at] = Value::Object(o);
        };
        put("WeaponTypeDefinition", weapon_type);
        for (f, p) in WEAPON_PART_FIELDS.iter().zip(parts) {
            put(f, *p);
        }
        put("PrefixPartDefinition", current.0);
        put("TitlePartDefinition", current.1);
        let w = self.weapon;
        self.set(w, "DefinitionData", v);
    }

    /// The weapon's two picks.
    fn picks(&self) -> (Option<ObjRef>, Option<ObjRef>) {
        let data = read_prop(&self.vm, self.weapon, "DefinitionData").unwrap();
        let get = |name: &str| match field(&self.vm, &data, name).unwrap() {
            Value::Object(o) => o,
            other => panic!("{name}: {other:?}"),
        };
        (get("PrefixPartDefinition"), get("TitlePartDefinition"))
    }

    fn run(&mut self) -> Result<(), VmError> {
        let w = self.weapon;
        choose_random_name_parts(&mut self.vm, w)
    }
}

fn np_list(np: PropKind) -> PropKind {
    PropKind::Array(Box::new(np))
}

use super::super::read_prop;

fn not_yet(e: VmError) -> ow_core::NotYet {
    match e {
        VmError::NotYet(n) => n,
        VmError::Attr(ow_attr::api::AttrError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ---- ChooseRandomNameParts ------------------------------------------------------------------------------------

#[test]
fn the_highest_priority_qualifying_entry_is_picked_and_a_later_entry_wins_a_tie() {
    let mut x = Fx::new(Schema::Good);
    let low = x.part_name(1, 100, 1.0, vec![]);
    let high = x.part_name(1, 100, 3.0, vec![]);
    let tie = x.part_name(1, 100, 3.0, vec![]);
    let lower = x.part_name(1, 100, 2.0, vec![]);
    let t = x.holder(WEAPON_TYPE_CLASS, &[Some(low), None, Some(high), Some(tie), Some(lower)], &[Some(low)]);
    x.data(Some(t), &[], (None, None));
    x.run().unwrap();
    assert_eq!(x.picks(), (Some(tie), Some(low)), "the later of two equal priorities; a None entry skipped");
}

#[test]
fn an_entry_outside_the_level_range_or_with_no_priority_does_not_qualify() {
    let mut x = Fx::new(Schema::Good);
    // The weapon is level 10.
    let above = x.part_name(11, 100, 9.0, vec![]);
    let below = x.part_name(1, 9, 9.0, vec![]);
    let zero = x.part_name(1, 100, 0.0, vec![]);
    let negative = x.part_name(1, 100, -1.0, vec![]);
    let edges = x.part_name(10, 10, 0.5, vec![]);
    let t = x.holder(WEAPON_TYPE_CLASS, &[Some(above), Some(below), Some(zero), Some(negative), Some(edges)], &[Some(zero)]);
    x.data(Some(t), &[], (None, None));
    x.run().unwrap();
    assert_eq!(x.picks(), (Some(edges), None), "min and max are inclusive; priority must be above 0");
}

#[test]
fn an_entry_qualifies_only_when_its_expressions_all_hold() {
    let mut x = Fx::new(Schema::Good);
    let bonus = x.bonus;
    let (holds, fails, none) = (x.above(Some(bonus), 1.0), x.above(Some(bonus), 10.0), x.above(None, 0.0));
    let both = x.part_name(1, 100, 1.0, vec![holds.clone(), holds.clone()]);
    let one_fails = x.part_name(1, 100, 5.0, vec![holds, fails]);
    let no_attribute = x.part_name(1, 100, 5.0, vec![none]);
    let t = x.holder(WEAPON_TYPE_CLASS, &[Some(both), Some(one_fails)], &[Some(no_attribute)]);
    x.data(Some(t), &[], (None, None));
    x.run().unwrap();
    assert_eq!(x.picks(), (Some(both), None), "And over the weapon's Bonus 5: one false expression disqualifies");
}

#[test]
fn a_part_replaces_a_pick_at_equal_or_higher_priority_and_never_clears_one() {
    let mut x = Fx::new(Schema::Good);
    let type_prefix = x.part_name(1, 100, 2.0, vec![]);
    let type_title = x.part_name(1, 100, 2.0, vec![]);
    let t = x.holder(WEAPON_TYPE_CLASS, &[Some(type_prefix)], &[Some(type_title)]);
    let equal = x.part_name(1, 100, 2.0, vec![]);
    let lower = x.part_name(1, 100, 1.0, vec![]);
    let out_of_range = x.part_name(50, 100, 9.0, vec![]);
    // Slot 1 offers an equal prefix (replaces) and nothing for the title (keeps); slot 3 a lower prefix (kept out),
    // an unqualified title (no clear). Slot 2 is None (skipped).
    let p1 = x.holder(WEAPON_PART_CLASS, &[Some(equal)], &[]);
    let p3 = x.holder(WEAPON_PART_CLASS, &[Some(lower)], &[Some(out_of_range)]);
    x.data(Some(t), &[Some(p1), None, Some(p3)], (None, None));
    x.run().unwrap();
    assert_eq!(x.picks(), (Some(equal), Some(type_title)));
}

#[test]
fn a_part_fills_a_pick_the_type_left_none_and_the_last_slot_is_walked() {
    let mut x = Fx::new(Schema::Good);
    let t = x.holder(WEAPON_TYPE_CLASS, &[], &[]);
    let low = x.part_name(1, 100, 0.25, vec![]);
    let last = x.holder(WEAPON_PART_CLASS, &[], &[Some(low)]);
    let mut parts = vec![None; WEAPON_PART_FIELDS.len()];
    *parts.last_mut().unwrap() = Some(last);
    x.data(Some(t), &parts, (None, None));
    x.run().unwrap();
    assert_eq!(x.picks(), (None, Some(low)));
}

#[test]
fn both_fields_are_written_none_included() {
    let mut x = Fx::new(Schema::Good);
    let old = x.part_name(1, 100, 1.0, vec![]);
    let t = x.holder(WEAPON_TYPE_CLASS, &[], &[]);
    x.data(Some(t), &[], (Some(old), Some(old)));
    x.run().unwrap();
    assert_eq!(x.picks(), (None, None), "no qualifying entry anywhere: both picks written as None");
}

#[test]
fn a_weapon_with_no_type_writes_nothing() {
    let mut x = Fx::new(Schema::Good);
    let old = x.part_name(1, 100, 1.0, vec![]);
    let p = x.holder(WEAPON_PART_CLASS, &[], &[]);
    x.data(None, &[Some(p)], (Some(old), Some(old)));
    x.run().unwrap();
    assert_eq!(x.picks(), (Some(old), Some(old)));
}

#[test]
fn an_expression_not_yet_is_returned_unprefixed_and_writes_nothing() {
    let mut x = Fx::new(Schema::Good);
    let unread = x.unread;
    let refused = x.above(Some(unread), 0.0);
    let first = x.part_name(1, 100, 1.0, vec![]);
    let blocked = x.part_name(1, 100, 1.0, vec![refused]);
    let t = x.holder(WEAPON_TYPE_CLASS, &[Some(first)], &[Some(blocked)]);
    let old = x.part_name(1, 100, 1.0, vec![]);
    x.data(Some(t), &[], (Some(old), Some(old)));
    let n = not_yet(x.run().unwrap_err());
    // The native has written nothing: the caller layer that wrote adds the mark (review r1 of #506).
    assert!(!n.what.contains("partial writes"), "{}", n.what);
    assert_eq!(x.picks(), (Some(old), Some(old)), "the prefix pick computed before the refusal is not written");
}

// ---- CacheWeaponCard and the preflight -----------------------------------------------------------------------

#[test]
fn cache_weapon_card_at_call_time_is_an_unprefixed_not_read() {
    let mut x = Fx::new(Schema::Good);
    let w = x.weapon;
    let before = read_prop(&x.vm, w, "DefinitionData");
    let n = not_yet(cache_weapon_card(&mut x.vm, w).unwrap_err());
    assert_eq!(n.missing, Missing::NotRead);
    assert_eq!(n.what, CACHE_WEAPON_CARD_NOT_READ, "no prefix: the native writes nothing (review r1 of #506)");
    assert_eq!(n.record, RECORD);
    assert_eq!(read_prop(&x.vm, w, "DefinitionData"), before);
}

#[test]
fn the_preflight_refuses_the_card_first_unprefixed_whatever_the_schema() {
    for schema in [Schema::Good, Schema::IntPriority, Schema::NoExpLevel] {
        let mut x = Fx::new(schema);
        let n = not_yet(preflight(&mut x.vm).unwrap_err());
        assert_eq!(n.missing, Missing::NotRead);
        assert_eq!(n.what, CACHE_WEAPON_CARD_NOT_READ, "no prefix: nothing written yet");
        assert_eq!(n.record, RECORD);
    }
}

#[test]
fn the_name_part_schema_holds_on_the_declared_classes() {
    let mut x = Fx::new(Schema::Good);
    name_part_schema(&mut x.vm).unwrap();
}

#[test]
fn each_schema_failure_is_a_script_error_naming_the_field() {
    for (schema, field) in [
        (Schema::IntPriority, "Priority"),
        (Schema::PlainTitleList, "TitleList"),
        (Schema::PlainPrefixField, "PrefixPartDefinition"),
        (Schema::NoExpLevel, "ExpLevel"),
        (Schema::SuffixStruct, "FooWeaponDefinitionData"),
    ] {
        let mut x = Fx::new(schema);
        match name_part_schema(&mut x.vm).unwrap_err() {
            VmError::Script { message: m, .. } => assert!(m.contains(field), "{field}: {m}"),
            other => panic!("{field}: expected VmError::Script, got {other:?}"),
        }
    }
}

// ---- Registration ---------------------------------------------------------------------------------------------

/// Declares both natives on the weapon class with invented keys and registers them.
fn declared(x: &mut Fx) -> (ow_object::ids::FuncId, ow_object::ids::FuncId) {
    let weapon = x.vm.space.classes.by_path(WEAPON_CLASS).unwrap();
    let mut declare = |path: &str, key: &str| {
        let path = ow_core::ObjectPath::parse(path).unwrap();
        x.vm.declare_function(FunctionDecl::native(path, key).with_owner(weapon)).unwrap()
    };
    let choose = declare(CHOOSE_RANDOM_NAME_PARTS, "Test.ChooseKey");
    let card = declare(CACHE_WEAPON_CARD, "Test.CardKey");
    register(&mut x.vm).unwrap();
    (choose, card)
}

#[test]
fn register_binds_both_natives_by_their_own_keys_in_off() {
    let mut x = Fx::new(Schema::Good);
    let (choose, card) = declared(&mut x);
    let pick = x.part_name(1, 100, 1.0, vec![]);
    let t = x.holder(WEAPON_TYPE_CLASS, &[Some(pick)], &[]);
    x.data(Some(t), &[], (None, None));
    let w = x.weapon;
    x.vm.call(choose, Some(w), vec![]).unwrap();
    assert_eq!(x.picks(), (Some(pick), None));
    let n = not_yet(x.vm.call(card, Some(w), vec![]).unwrap_err());
    assert_eq!(n.what, CACHE_WEAPON_CARD_NOT_READ);
}

#[test]
fn reference_binds_neither_native() {
    let mut x = Fx::in_mode(Schema::Good, ParityMode::Reference);
    let (_, card) = declared(&mut x);
    let w = x.weapon;
    match x.vm.call(card, Some(w), vec![]) {
        Err(VmError::NotYet(n)) => assert!(!n.what.contains(CACHE_WEAPON_CARD_NOT_READ), "{}", n.what),
        Err(_) => {}
        Ok(v) => panic!("Reference ran an unbound native: {v:?}"),
    }
}


// ---- Item 3 of review r1 of #506: untagged fields read through the class defaults -------------------------------

mod store {
    //! An invented `WillowGame.upk` in a synthetic store (AGENTS.md rule 9): `Default__WeaponNamePartDefinition` tags
    //! `MinExpLevelRequirement` 1, `MaxExpLevelRequirement` 100 and `Priority` 1, and the name part `ToyName` tags
    //! none of them. Loaded, it qualifies at level 10.

    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use ow_attr::AttributeSystem;
    use ow_core::{ObjectPath, ParityMode};
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::{ObjectSpace, Origin};
    use ow_object::value::Value;
    use ow_package::testkit::ints;
    use ow_package::testkit::reflect::toy_struct;
    use ow_package::testkit::script::ScriptToy;
    use ow_vm::api::Vm;

    use super::super::{NAME_PART_CLASS, qualifies};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn package() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "ScriptStruct",
            "IntProperty",
            "FloatProperty",
            "ObjectProperty",
            "ArrayProperty",
            "StructProperty",
        ]);
        let object = t.class("Object", 0);
        let name_part = t.class("WeaponNamePartDefinition", object);
        t.prop("Int", name_part, "MinExpLevelRequirement", 0, 0);
        t.prop("Int", name_part, "MaxExpLevelRequirement", 0, 0);
        t.prop("Float", name_part, "Priority", 0, 0);
        let expr = toy_struct(&mut t, name_part, "AttributeExpressionData", None);
        t.prop("Object", expr, "AttributeOperand1", 0, object);
        let inner = t.p.next_export();
        t.prop("Array", name_part, "Expressions", 0, inner + 1);
        t.prop("Struct", inner, "Expressions", 0, expr);
        let mut d = ints(&[0]);
        d.extend(t.p.int_tag("MinExpLevelRequirement", 1));
        d.extend(t.p.int_tag("MaxExpLevelRequirement", 100));
        d.extend(t.p.float_tag("Priority", 1.0));
        d.extend_from_slice(&t.none);
        t.p.add_export(name_part, "Default__WeaponNamePartDefinition", d);
        let mut untagged = ints(&[0]);
        untagged.extend_from_slice(&t.none);
        t.p.add_export(name_part, "ToyName", untagged);
        // A weapon-like owner for the expressions' context (no expressions here, so it is never read).
        t.class("Owner", object);
        t.p.build()
    }

    #[test]
    fn an_untagged_name_part_takes_its_class_defaults_and_qualifies() {
        let dir = std::env::temp_dir().join(format!(
            "ow-inventory-name-parts-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = package();
        let defs =
            DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
                .unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::standard().unwrap()), ParityMode::Off);
        let path = ObjectPath::parse("WillowGame.ToyName").unwrap();
        let entry = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let name_part = vm.find_class(NAME_PART_CLASS).unwrap();
        assert_eq!(vm.space.heap.get(entry).unwrap().class, name_part);
        let read = |vm: &Vm, n: &str| crate::equip::read_prop(vm, entry, n).unwrap();
        assert_eq!(read(&vm, "MinExpLevelRequirement"), Value::Int(1), "from the class defaults, not 0");
        assert_eq!(read(&vm, "MaxExpLevelRequirement"), Value::Int(100));
        assert_eq!(read(&vm, "Priority"), Value::Float(1.0));
        let owner_class = vm.find_class("WillowGame.Owner").unwrap();
        let weapon = vm.space.alloc(owner_class, "Owner_0", None, Origin::Runtime).unwrap();
        assert!(qualifies(&mut vm, weapon, 10, entry).unwrap(), "level 10 is inside 1 to 100 and Priority 1 is above 0");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
