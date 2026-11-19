//! The attribute slot natives on an invented class set (AGENTS.md rule 9; decision 2026-11-19-82). Expected values are
//! written by hand from NATIVE_INIT_ATTRIBUTE_SLOTS.md. Every init value is context-free (no attribute, no
//! initialization definition), so its value is its constant times its scale. The slot array is a static array of 4
//! here, so the limit is shown to stop at the declared dimension too.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
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

/// The invented dimension of `AttributeSlots` and of the replicated values.
const DIM: i32 = 4;
const ROLES: [&str; 4] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority"];

/// Declaration variants of the invented item class (review r2 of #468).
#[derive(Default)]
struct Variant {
    /// `bNetDirty` declared as an int, so a bool cannot be written to it.
    net_dirty_int: bool,
    /// No `Role` declared: a value-pass input that a base-pass refusal must come before.
    no_role: bool,
}

struct Toy {
    vm: Vm,
    aid: StructId,
    upgrade: StructId,
    effect: StructId,
    wdata: StructId,
    idata: StructId,
}

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind)
}

/// The slot fields shared by `AttributeSlotEffectData` and `AttributeSlotData` (section 3 step 3).
fn shared_fields(s: &mut ObjectSpace, obj: &PropKind, aid: StructId) -> Vec<PropertyDecl> {
    let mut f = Vec::new();
    for n in ["SlotName", "TargetInstanceDataName"] {
        f.push(decl(s, n, PropKind::Name));
    }
    for n in ["bExternalSlot", "bRunEffectsAsSkill", "bIncludeAlliesAsTarget", "bEnforceMinimumGrade", "bEnforceMaximumGrade"] {
        f.push(decl(s, n, PropKind::Bool));
    }
    for n in ["MinimumGrade", "MaximumGrade"] {
        f.push(decl(s, n, PropKind::Int));
    }
    for n in ["AttributeToModify", "ConstraintAttribute"] {
        f.push(decl(s, n, obj.clone()));
    }
    f.push(decl(s, "ModifierType", PropKind::Byte { enum_: None }));
    for n in ["BaseModifierValue", "PerGradeUpgrade"] {
        f.push(decl(s, n, PropKind::Struct(aid)));
    }
    f
}

impl Toy {
    fn new(mode: ParityMode) -> Toy {
        Toy::with(mode, Variant::default())
    }

    fn with(mode: ParityMode, variant: Variant) -> Toy {
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
        let up = vec![
            decl(&mut s, "SlotName", PropKind::Name),
            decl(&mut s, "GradeIncrease", PropKind::Int),
            decl(&mut s, "bActivateSlot", PropKind::Bool),
        ];
        let upgrade = s.classes.define_struct("Test.AttributeSlotUpgradeData", None, up).unwrap();
        let mut ef = shared_fields(&mut s, &obj, aid);
        ef.push(decl(&mut s, "bIncludeInFunStats", PropKind::Bool));
        let effect = s.classes.define_struct("Test.AttributeSlotEffectData", None, ef).unwrap();
        let mut sf = shared_fields(&mut s, &obj, aid);
        sf.push(decl(&mut s, "bIncludeInModifierText", PropKind::Bool));
        sf.push(decl(&mut s, "bActivated", PropKind::Bool));
        sf.push(decl(&mut s, "EffectGrade", PropKind::Int));
        sf.push(decl(&mut s, "ComputedModifierValue", PropKind::Float));
        let slot = s.classes.define_struct("Test.AttributeSlotData", None, sf).unwrap();
        let lists = |s: &mut ObjectSpace| {
            vec![
                decl(s, "AttributeSlotUpgrades", PropKind::Array(Box::new(PropKind::Struct(upgrade)))),
                decl(s, "AttributeSlotEffects", PropKind::Array(Box::new(PropKind::Struct(effect)))),
            ]
        };
        let mut df = lists(&mut s);
        df.push(decl(&mut s, "AttributeSlotMaxActivated", PropKind::Int));
        df.push(decl(&mut s, "AttributeSlotBaseGrade", PropKind::Struct(aid)));
        df.push(decl(&mut s, "AttributeSlotEffectMode", PropKind::Byte { enum_: None }));
        s.classes.define("Test.Definition", Some(object), df).unwrap();
        let pf = lists(&mut s);
        s.classes.define("Test.Part", Some(object), pf).unwrap();
        let mut w = vec![decl(&mut s, "WeaponTypeDefinition", obj.clone())];
        for slot in super::super::natives::WEAPON_ALL_PARTS {
            w.push(decl(&mut s, slot, obj.clone()));
        }
        let wdata = s.classes.define_struct("Test.WeaponDefinitionData", None, w).unwrap();
        let mut i = vec![decl(&mut s, "ItemDefinition", obj.clone())];
        for slot in super::super::natives::ITEM_ALL_PARTS {
            i.push(decl(&mut s, slot, obj.clone()));
        }
        let idata = s.classes.define_struct("Test.ItemDefinitionData", None, i).unwrap();
        let roles = ROLES.iter().map(|r| s.names.intern(r)).collect();
        let role = s.classes.define_enum("Engine.Actor.ENetRole", roles).unwrap();
        let mut inv = vec![
            decl(&mut s, "bNetDirty", if variant.net_dirty_int { PropKind::Int } else { PropKind::Bool }),
            decl(&mut s, "AttributeSlots", PropKind::Struct(slot)).with_array_dim(DIM),
            decl(&mut s, "ReplicatedAttributeSlotModifierValues", PropKind::Float).with_array_dim(DIM),
            decl(&mut s, "TempStatModifier", PropKind::FloatAttribute).with_flags(PropertyDecl::STACK_ATTRIBUTE),
            decl(&mut s, "TempStatModifierBaseValue", PropKind::FloatAttribute).with_flags(PropertyDecl::PLAIN_ATTRIBUTE),
            decl(&mut s, "TempStatModifierModifierStack", objs),
        ];
        if !variant.no_role {
            inv.push(decl(&mut s, "Role", PropKind::Byte { enum_: Some(role) }));
        }
        let inventory = s.classes.define("WillowGame.WillowInventory", Some(object), inv).unwrap();
        let (v, b, st) = (
            s.names.intern("TempStatModifier"),
            s.names.intern("TempStatModifierBaseValue"),
            s.names.intern("TempStatModifierModifierStack"),
        );
        s.classes.link_attribute(inventory, v, b, st).unwrap();
        let wf = vec![decl(&mut s, "DefinitionData", PropKind::Struct(wdata))];
        s.classes.define("WillowGame.WillowWeapon", Some(inventory), wf).unwrap();
        let itf = vec![decl(&mut s, "DefinitionData", PropKind::Struct(idata))];
        s.classes.define("WillowGame.WillowItem", Some(inventory), itf).unwrap();
        s.classes.define("Test.Thing", Some(object), vec![]).unwrap();
        let attrs = AttributeSystem::standard().unwrap();
        let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(attrs), mode);
        let flag = decl(&mut vm.space, "bIncludeNameParts", PropKind::Bool).with_flags(PropertyDecl::PARAM);
        let mut f = FunctionDecl::native(ObjectPath::parse("WillowGame.WillowInventory.InitializeAttributeSlots").unwrap(), INIT_SLOTS);
        f.params = vec![flag];
        vm.declare_function(f).unwrap();
        let path = ObjectPath::parse("WillowGame.WillowInventory.InitializeAttributeSlotsForNameParts").unwrap();
        vm.declare_function(FunctionDecl::native(path, INIT_NAME_PART_SLOTS)).unwrap();
        register(&mut vm).unwrap();
        Toy { vm, aid, upgrade, effect, wdata, idata }
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

    fn name(&mut self, n: &str) -> Value {
        Value::Name(self.vm.space.names.intern(n))
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

    /// An init value that reads an attribute (not context-free).
    fn init_attr(&mut self) -> Value {
        let a = self.obj("Test.Thing", "SomeAttribute");
        let aid = self.aid;
        self.st(aid, vec![("BaseValueAttribute", Value::Object(Some(a))), ("BaseValueScaleConstant", Value::Float(1.0))])
    }

    fn up(&mut self, slot: &str, increase: i32, activate: bool) -> Value {
        let n = self.name(slot);
        let ty = self.upgrade;
        self.st(ty, vec![("SlotName", n), ("GradeIncrease", Value::Int(increase)), ("bActivateSlot", Value::Bool(activate))])
    }

    /// An effect entry: `base` + (grade - 1) x `per`, with `extra` fields.
    fn effect(&mut self, slot: &str, base: f32, per: f32, extra: Vec<(&str, Value)>) -> Value {
        let n = self.name(slot);
        let (b, p) = (self.init(base), self.init(per));
        let mut f = vec![("SlotName", n), ("BaseModifierValue", b), ("PerGradeUpgrade", p), ("ModifierType", Value::Byte(2))];
        f.extend(extra);
        let ty = self.effect;
        self.st(ty, f)
    }

    fn holder(&mut self, class: &str, name: &str, ups: Vec<Value>, effects: Vec<Value>) -> ObjRef {
        let o = self.obj(class, name);
        self.set(o, "AttributeSlotUpgrades", Value::array(ups));
        self.set(o, "AttributeSlotEffects", Value::array(effects));
        o
    }

    fn definition(&mut self, ups: Vec<Value>, effects: Vec<Value>, max: i32, base_grade: f32) -> ObjRef {
        let d = self.holder("Test.Definition", "Definition", ups, effects);
        self.set(d, "AttributeSlotMaxActivated", Value::Int(max));
        let g = self.init(base_grade);
        self.set(d, "AttributeSlotBaseGrade", g);
        d
    }

    fn weapon(&mut self, definition: Option<ObjRef>, parts: &[(&str, ObjRef)], authority: bool) -> ObjRef {
        let has_role = {
            let c = self.class("WillowGame.WillowInventory");
            self.vm.space.prop(c, "Role").is_some()
        };
        let w = self.obj("WillowGame.WillowWeapon", "Weapon_0");
        let mut fields = vec![("WeaponTypeDefinition", Value::Object(definition))];
        fields.extend(parts.iter().map(|(slot, p)| (*slot, Value::Object(Some(*p)))));
        let ty = self.wdata;
        let data = self.st(ty, fields);
        self.set(w, "DefinitionData", data);
        if has_role {
            self.set(w, "Role", Value::Byte(if authority { 3 } else { 1 }));
        }
        w
    }

    fn slot(&self, o: ObjRef, i: u32) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "AttributeSlots").unwrap();
        self.vm.space.read(o, PropAddr::element(p, i)).unwrap().clone()
    }

    fn put_slot(&mut self, o: ObjRef, i: u32, fields: Vec<(&str, Value)>) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "AttributeSlots").unwrap();
        let ty = self.slot(o, i).as_struct().unwrap().ty;
        let v = self.st(ty, fields);
        self.vm.space.write(o, PropAddr::element(p, i), v).unwrap();
    }

    fn replicated(&self, o: ObjRef, i: u32) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "ReplicatedAttributeSlotModifierValues").unwrap();
        self.vm.space.read(o, PropAddr::element(p, i)).unwrap().clone()
    }

    fn put_replicated(&mut self, o: ObjRef, i: u32, v: f32) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "ReplicatedAttributeSlotModifierValues").unwrap();
        self.vm.space.write(o, PropAddr::element(p, i), Value::Float(v)).unwrap();
    }

    fn f(&self, v: &Value, name: &str) -> Value {
        ow_attr::eval::field(&self.vm.space, v, name).unwrap().clone()
    }

    /// Slot `i`'s (name, activated, grade, computed).
    fn summary(&mut self, o: ObjRef, i: u32) -> (Value, Value, Value, Value) {
        let s = self.slot(o, i);
        (self.f(&s, "SlotName"), self.f(&s, "bActivated"), self.f(&s, "EffectGrade"), self.f(&s, "ComputedModifierValue"))
    }

    fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, Some(this), args)?.ret)
    }

    /// Every slot and replicated element of `o`, and its stat, flag and stack (to show nothing was written).
    fn snapshot(&self, o: ObjRef) -> Vec<Value> {
        let mut v: Vec<Value> = (0..DIM as u32).map(|i| self.slot(o, i)).collect();
        v.extend((0..DIM as u32).map(|i| self.replicated(o, i)));
        for n in ["TempStatModifier", "TempStatModifierBaseValue", "TempStatModifierModifierStack", "bNetDirty"] {
            v.push(self.get(o, n));
        }
        v
    }
}

fn not_yet(e: &VmError) -> (Missing, String) {
    match e {
        VmError::NotYet(n) => (n.missing.clone(), n.what.to_string()),
        other => panic!("not a NotYet: {other:?}"),
    }
}

/// The slot build and the value pass on the authority: the definition's upgrade activates `A` with the base grade
/// rounded half up (1.5 -> 2) plus its increase; a part adds to `A` and activates `B`, found in that part's effects;
/// an upgrade to a slot no list names is ignored, and an upgrade that never activates `C` writes nothing.
#[test]
fn a_weapon_s_slots_are_built_from_the_definition_then_its_parts() {
    let mut t = Toy::new(ParityMode::Off);
    let (ua, uc) = (t.up("A", 1, true), t.up("C", 5, false));
    let ea = t.effect("A", 10.0, 2.0, vec![("bIncludeInFunStats", Value::Bool(true))]);
    let ec = t.effect("C", 1.0, 1.0, vec![]);
    let definition = t.definition(vec![ua, uc], vec![ea, ec], 19, 1.5);
    let (ua2, ub, uz) = (t.up("A", 3, true), t.up("B", 0, true), t.up("Z", 9, true));
    let eb = t.effect("B", 0.5, 0.25, vec![]);
    let body = t.holder("Test.Part", "Body", vec![ua2, ub, uz], vec![eb]);
    let w = t.weapon(Some(definition), &[("BodyPartDefinition", body)], true);
    t.call("WillowGame.WillowInventory.InitializeAttributeSlots", w, vec![Value::Bool(false)]).unwrap();
    let (a, b) = (t.name("A"), t.name("B"));
    // A: 2 (base grade) + 1 + 3 = 6, activated once; its value 10 + 5 x 2 = 20.
    assert_eq!(t.summary(w, 0), (a, Value::Bool(true), Value::Int(6), Value::Float(20.0)));
    // B: 2 + 0 = 2; 0.5 + 1 x 0.25.
    assert_eq!(t.summary(w, 1), (b, Value::Bool(true), Value::Int(2), Value::Float(0.75)));
    let s0 = t.slot(w, 0);
    assert_eq!(t.f(&s0, "bIncludeInModifierText"), Value::Bool(true), "from bIncludeInFunStats");
    assert_eq!(t.f(&s0, "ModifierType"), Value::Byte(2));
    assert_eq!(t.summary(w, 2).1, Value::Bool(false), "C was never activated, so it is never written");
    assert_eq!((t.replicated(w, 0), t.replicated(w, 1)), (Value::Float(20.0), Value::Float(0.75)));
    assert_eq!(t.get(w, "bNetDirty"), Value::Bool(true));
    assert_eq!(t.get(w, "TempStatModifier"), Value::Float(2.0), "the last activated slot's grade");
    assert_eq!(t.get(w, "TempStatModifierBaseValue"), Value::Float(2.0));
    assert_eq!(t.get(w, "TempStatModifierModifierStack"), Value::array(Vec::new()));
}

/// The clamps after each copy: the maximum first, then the minimum, so the minimum wins a conflict.
#[test]
fn the_maximum_caps_then_the_minimum_raises() {
    let mut t = Toy::new(ParityMode::Off);
    let (ua, ub) = (t.up("A", 9, true), t.up("B", 9, true));
    let max_only = vec![("bEnforceMaximumGrade", Value::Bool(true)), ("MaximumGrade", Value::Int(4))];
    let conflict = vec![
        ("bEnforceMaximumGrade", Value::Bool(true)),
        ("MaximumGrade", Value::Int(3)),
        ("bEnforceMinimumGrade", Value::Bool(true)),
        ("MinimumGrade", Value::Int(7)),
    ];
    let (ea, eb) = (t.effect("A", 0.0, 1.0, max_only), t.effect("B", 0.0, 1.0, conflict));
    let definition = t.definition(vec![ua, ub], vec![ea, eb], 19, 1.0);
    let w = t.weapon(Some(definition), &[], true);
    initialize(&mut t.vm, w, false, false).unwrap();
    assert_eq!(t.summary(w, 0).2, Value::Int(4), "10 capped at 4");
    assert_eq!(t.summary(w, 1).2, Value::Int(7), "capped at 3, then raised to 7");
}

/// The slot limit: the definition's count, 19 when out of 1 to 19, and never past the declared dimension (4 here).
#[test]
fn the_slot_limit_drops_activations_past_it() {
    for (max, written) in [(1, 1), (0, 4), (25, 4)] {
        let mut t = Toy::new(ParityMode::Off);
        let names = ["A", "B", "C", "D", "E"];
        let ups = names.iter().map(|n| t.up(n, 0, true)).collect();
        let effects = names.iter().map(|n| t.effect(n, 1.0, 0.0, vec![])).collect();
        let definition = t.definition(ups, effects, max, 1.0);
        let w = t.weapon(Some(definition), &[], true);
        initialize(&mut t.vm, w, false, false).unwrap();
        let active = (0..DIM as u32).filter(|&i| t.summary(w, i).1 == Value::Bool(true)).count();
        assert_eq!(active, written, "max {max}");
    }
}

/// An item initialised before: its activated slot keeps its place and data; the working record of that name is not
/// written, and a new one goes after it.
#[test]
fn activated_slots_are_kept_and_new_records_follow_them() {
    let mut t = Toy::new(ParityMode::Off);
    let (ua, ub) = (t.up("A", 5, true), t.up("B", 1, true));
    let (ea, eb) = (t.effect("A", 100.0, 0.0, vec![]), t.effect("B", 3.0, 0.0, vec![]));
    let definition = t.definition(vec![ua, ub], vec![ea, eb], 19, 1.0);
    let w = t.weapon(Some(definition), &[], false);
    let a = t.name("A");
    t.put_slot(w, 0, vec![("SlotName", a.clone()), ("bActivated", Value::Bool(true)), ("EffectGrade", Value::Int(1))]);
    t.put_replicated(w, 0, 7.5);
    t.put_replicated(w, 1, 2.5);
    initialize(&mut t.vm, w, false, false).unwrap();
    // Not the authority: every slot's value is its replicated element; nothing else of the pass is written.
    assert_eq!(t.summary(w, 0), (a, Value::Bool(true), Value::Int(1), Value::Float(7.5)), "kept, grade untouched");
    let b = t.name("B");
    assert_eq!(t.summary(w, 1), (b, Value::Bool(true), Value::Int(2), Value::Float(2.5)));
    assert_eq!(t.get(w, "bNetDirty"), Value::Bool(false));
}

/// The replication boundary off the authority (review r2 of #468): every slot below the limit takes its replicated
/// element, an inactive one included, and a slot at or past the limit keeps its value.
#[test]
fn off_the_authority_the_replicated_values_stop_at_the_limit() {
    let mut t = Toy::new(ParityMode::Off);
    let ua = t.up("A", 0, true);
    let ea = t.effect("A", 1.0, 1.0, vec![]);
    let definition = t.definition(vec![ua], vec![ea], 2, 1.0);
    let w = t.weapon(Some(definition), &[], false);
    for i in 0..DIM as u32 {
        t.put_slot(w, i, vec![("ComputedModifierValue", Value::Float(-1.0))]);
        t.put_replicated(w, i, 100.0 + i as f32);
    }
    initialize(&mut t.vm, w, false, false).unwrap();
    assert_eq!(t.summary(w, 0).1, Value::Bool(true), "A is written at 0");
    assert_eq!(t.summary(w, 0).3, Value::Float(100.0));
    assert_eq!(t.summary(w, 1).1, Value::Bool(false), "slot 1 stays inactive");
    assert_eq!(t.summary(w, 1).3, Value::Float(101.0), "an inactive slot below the limit takes its replicated value");
    for i in 2..DIM as u32 {
        assert_eq!(t.summary(w, i).3, Value::Float(-1.0), "slot {i} is past the limit of 2: untouched");
    }
}

/// Every planned write is checked before the first (review r2 of #468): a `bNetDirty` declared as an int refuses the
/// call with the item unchanged.
#[test]
fn a_write_that_cannot_be_made_refuses_before_any_write() {
    let mut t = Toy::with(ParityMode::Off, Variant { net_dirty_int: true, ..Variant::default() });
    let ua = t.up("A", 1, true);
    let ea = t.effect("A", 1.0, 1.0, vec![]);
    let d = t.definition(vec![ua], vec![ea], 19, 1.0);
    let w = t.weapon(Some(d), &[], true);
    let before = t.snapshot(w);
    let e = initialize(&mut t.vm, w, false, false).unwrap_err();
    assert!(!matches!(e, VmError::NotYet(_)), "a heap kind error, not a NotYet: {e:?}");
    assert_eq!(t.snapshot(w), before, "nothing written");
}

/// The refusal order (review r2 of #468): the value pass's inputs (here a class with no `Role`) are read only when the
/// value pass runs, so a base-pass refusal comes first.
#[test]
fn a_base_pass_refusal_comes_before_the_value_pass_inputs() {
    let mut t = Toy::with(ParityMode::Off, Variant { no_role: true, ..Variant::default() });
    let ua = t.up("A", 1, true);
    let ea = t.effect("A", 1.0, 1.0, vec![]);
    let d = t.definition(vec![ua], vec![ea], 19, 1.0);
    let g = t.init_attr();
    t.set(d, "AttributeSlotBaseGrade", g);
    let w = t.weapon(Some(d), &[], true);
    let (_, what) = not_yet(&initialize(&mut t.vm, w, false, false).unwrap_err());
    assert!(what.starts_with("IAS-BASE-GRADE-CONTEXT: "), "{what}");
    // Without the base-pass refusal, the missing Role is the value pass's data error.
    let d2 = t.definition(vec![], vec![], 19, 1.0);
    let w2 = t.weapon(Some(d2), &[], true);
    assert!(!matches!(initialize(&mut t.vm, w2, false, false).unwrap_err(), VmError::NotYet(_)));
}

/// The name-part pass's in-place grades are planned one write per applied upgrade, in upgrade order, and a kept slot
/// with no upgrade is not rewritten (review r2 of #468): prefix B+1 then A+2, title B+3.
#[test]
fn in_place_grades_are_written_per_upgrade_in_upgrade_order() {
    let mut t = Toy::new(ParityMode::Off);
    let definition = t.definition(vec![], vec![], 19, 1.0);
    let (pb, pa, tb) = (t.up("B", 1, false), t.up("A", 2, false), t.up("B", 3, false));
    let prefix = t.holder("Test.Part", "Prefix", vec![pb, pa], vec![]);
    let title = t.holder("Test.Part", "Title", vec![tb], vec![]);
    let w = t.weapon(Some(definition), &[("PrefixPartDefinition", prefix), ("TitlePartDefinition", title)], false);
    let (a, b, c) = (t.name("A"), t.name("B"), t.name("C"));
    for (i, n) in [(0, a.clone()), (1, b.clone()), (2, c.clone())] {
        t.put_slot(w, i, vec![("SlotName", n), ("bActivated", Value::Bool(true)), ("EffectGrade", Value::Int(1))]);
    }
    let s = plan(&mut t.vm, w, false, true).unwrap();
    let mut grades = Vec::new();
    for op in &s.ops {
        if let Op::Slot(i, v) = op {
            grades.push((*i, t.f(v, "EffectGrade")));
        }
    }
    // The three in-place writes, then the value pass's (off the authority: every slot below the limit).
    assert_eq!(&grades[..3], [(1, Value::Int(2)), (0, Value::Int(3)), (1, Value::Int(5))]);
    assert_eq!(grades.len(), 3 + DIM as usize, "no other in-place write; slot C (no upgrade) only in the value pass");
    s.commit(&mut t.vm).unwrap();
    assert_eq!((t.summary(w, 0).2, t.summary(w, 1).2, t.summary(w, 2).2), (Value::Int(3), Value::Int(5), Value::Int(1)));
}

/// `EffectGrade - 1` that would wrap (a base grade of 0 plus `GradeIncrease` i32::MIN) is the note's
/// IAS-ROUNDING-RANGE port restriction (hub ruling, review r2 of #468), refused in the plan on the authority.
#[test]
fn a_grade_whose_step_would_wrap_is_refused_in_the_plan() {
    let mut t = Toy::new(ParityMode::Off);
    let ua = t.up("A", i32::MIN, true);
    let ea = t.effect("A", 1.0, 1.0, vec![]);
    let d = t.definition(vec![ua], vec![ea], 19, 0.0);
    let w = t.weapon(Some(d), &[], true);
    let before = t.snapshot(w);
    let (missing, what) = not_yet(&initialize(&mut t.vm, w, false, false).unwrap_err());
    assert_eq!(missing, Missing::NotRead);
    assert!(what.starts_with("IAS-ROUNDING-RANGE: "), "{what}");
    assert_eq!(t.snapshot(w), before, "nothing written");
}

/// The name-part pass (with the bool, and on its own): a name part's upgrade grows an activated slot in place without
/// a clamp, and activates a slot found in the definition's effects, never in the name part's own.
#[test]
fn the_name_part_pass_grows_in_place_and_looks_up_in_the_definition() {
    for alone in [false, true] {
        let mut t = Toy::new(ParityMode::Off);
        let ua = t.up("A", 1, true);
        let capped = vec![("bEnforceMaximumGrade", Value::Bool(true)), ("MaximumGrade", Value::Int(2))];
        let (ea, ec) = (t.effect("A", 0.0, 1.0, capped), t.effect("C", 4.0, 1.0, vec![]));
        let definition = t.definition(vec![ua], vec![ea, ec], 19, 1.0);
        let (pa, pc, pd) = (t.up("A", 5, false), t.up("C", 1, true), t.up("D", 1, true));
        let ed = t.effect("D", 1.0, 1.0, vec![]);
        let prefix = t.holder("Test.Part", "Prefix", vec![pa, pc, pd], vec![ed]);
        let w = t.weapon(Some(definition), &[("PrefixPartDefinition", prefix)], true);
        if alone {
            initialize(&mut t.vm, w, false, false).unwrap();
            t.call("WillowGame.WillowInventory.InitializeAttributeSlotsForNameParts", w, vec![]).unwrap();
        } else {
            t.call("WillowGame.WillowInventory.InitializeAttributeSlots", w, vec![Value::Bool(true)]).unwrap();
        }
        let (a, c) = (t.name("A"), t.name("C"));
        // A: 1 + 1 = 2 (at its cap) from the build, then +5 in place, not clamped again: 7; value 0 + 6 x 1.
        assert_eq!(t.summary(w, 0), (a, Value::Bool(true), Value::Int(7), Value::Float(6.0)), "alone {alone}");
        // C: from the definition's effects, 1 + 1 = 2; value 4 + 1.
        assert_eq!(t.summary(w, 1), (c, Value::Bool(true), Value::Int(2), Value::Float(5.0)), "alone {alone}");
        assert_eq!(t.summary(w, 2).1, Value::Bool(false), "D is only in the name part's own effects: ignored");
    }
}

/// An item uses its own lists (`ItemDefinition`, the Alpha part).
#[test]
fn an_item_uses_its_definition_and_parts() {
    let mut t = Toy::new(ParityMode::Off);
    let definition = t.definition(vec![], vec![], 19, 1.0);
    let ua = t.up("A", 2, true);
    let ea = t.effect("A", 1.0, 1.0, vec![]);
    let alpha = t.holder("Test.Part", "Alpha", vec![ua], vec![ea]);
    let i = t.obj("WillowGame.WillowItem", "Item_0");
    let ty = t.idata;
    let data = t.st(ty, vec![("ItemDefinition", Value::Object(Some(definition))), ("AlphaItemPartDefinition", Value::Object(Some(alpha)))]);
    t.set(i, "DefinitionData", data);
    t.set(i, "Role", Value::Byte(3));
    initialize(&mut t.vm, i, false, false).unwrap();
    let a = t.name("A");
    assert_eq!(t.summary(i, 0), (a, Value::Bool(true), Value::Int(3), Value::Float(3.0)));
}

/// The note's named NotYets (section 7), each decided before any write and each text starting with its identifier:
/// a missing definition, a base grade or slot value that is not context-free, a rounding out of range, the runtime
/// skill outside mode 2, and item slots that are not a packed, unique prefix (an empty slot before an activated one,
/// with or without a copy to place, and a repeated name).
#[test]
fn the_named_not_yets_come_before_any_write() {
    type Setup = fn(&mut Toy) -> ObjRef;
    let cases: [(&str, Setup); 9] = [
        ("IAS-NO-DEFINITION", |t| t.weapon(None, &[], true)),
        ("IAS-BASE-GRADE-CONTEXT", |t| {
            let ua = t.up("A", 1, true);
            let ea = t.effect("A", 1.0, 1.0, vec![]);
            let d = t.definition(vec![ua], vec![ea], 19, 1.0);
            let g = t.init_attr();
            t.set(d, "AttributeSlotBaseGrade", g);
            t.weapon(Some(d), &[], true)
        }),
        ("IAS-ROUNDING-RANGE", |t| {
            let ua = t.up("A", 1, true);
            let ea = t.effect("A", 1.0, 1.0, vec![]);
            let d = t.definition(vec![ua], vec![ea], 19, 2.0e9);
            t.weapon(Some(d), &[], true)
        }),
        ("IAS-ROUNDING-RANGE", |t| {
            // A grade sum that would wrap a 32-bit int.
            let ua = t.up("A", i32::MAX, true);
            let ea = t.effect("A", 1.0, 1.0, vec![]);
            let d = t.definition(vec![ua], vec![ea], 19, 1.0);
            t.weapon(Some(d), &[], true)
        }),
        ("IAS-VALUE-CONTEXT", |t| {
            let ua = t.up("A", 1, true);
            let b = t.init_attr();
            let ea = t.effect("A", 1.0, 1.0, vec![("BaseModifierValue", b)]);
            let d = t.definition(vec![ua], vec![ea], 19, 1.0);
            t.weapon(Some(d), &[], true)
        }),
        ("IAS-RUNTIME-SKILL", |t| {
            let ua = t.up("A", 1, true);
            let skill = vec![("bExternalSlot", Value::Bool(true)), ("bRunEffectsAsSkill", Value::Bool(true))];
            let ea = t.effect("A", 1.0, 1.0, skill);
            let d = t.definition(vec![ua], vec![ea], 19, 1.0);
            t.set(d, "AttributeSlotEffectMode", Value::Byte(1));
            t.weapon(Some(d), &[], true)
        }),
        ("IAS-SLOT-SHAPE", |t| {
            let ub = t.up("B", 1, true);
            let eb = t.effect("B", 1.0, 1.0, vec![]);
            let d = t.definition(vec![ub], vec![eb], 19, 1.0);
            let w = t.weapon(Some(d), &[], false);
            let a = t.name("A");
            t.put_slot(w, 1, vec![("SlotName", a), ("bActivated", Value::Bool(true))]);
            w
        }),
        ("IAS-SLOT-SHAPE", |t| {
            // No copy to place: the shape is refused all the same (the note's step 2).
            let d = t.definition(vec![], vec![], 19, 1.0);
            let w = t.weapon(Some(d), &[], false);
            let a = t.name("A");
            t.put_slot(w, 2, vec![("SlotName", a), ("bActivated", Value::Bool(true))]);
            w
        }),
        ("IAS-SLOT-SHAPE", |t| {
            let d = t.definition(vec![], vec![], 19, 1.0);
            let w = t.weapon(Some(d), &[], false);
            let a = t.name("A");
            t.put_slot(w, 0, vec![("SlotName", a.clone()), ("bActivated", Value::Bool(true))]);
            t.put_slot(w, 1, vec![("SlotName", a), ("bActivated", Value::Bool(true))]);
            w
        }),
    ];
    for (subject, setup) in cases {
        let mut t = Toy::new(ParityMode::Off);
        let w = setup(&mut t);
        let before = t.snapshot(w);
        let e = initialize(&mut t.vm, w, false, false).unwrap_err();
        let (missing, what) = not_yet(&e);
        assert_eq!(missing, Missing::NotRead, "{subject}");
        assert!(what.starts_with(&format!("{subject}: ")), "{subject}: {what}");
        assert!(!what.contains("partial writes"), "{subject}: decided before any write");
        assert_eq!(t.snapshot(w), before, "{subject}: nothing written");
    }
}

/// The base grade's rounding (section 3): 2x + 0.5 in single precision, to the nearest int with ties to even, then
/// halved toward minus infinity. A half goes up for negative values too, and just below a half the single-precision
/// step rounds up.
#[test]
fn the_base_grade_rounds_as_the_note_says() {
    for (x, n) in [(1.5f32, 2), (2.5, 3), (-0.5, 0), (-1.5, -1), (-2.5, -2), (0.49999997, 1), (0.4, 0), (1.0, 1), (-1_073_741_824.0, -1_073_741_824)] {
        assert_eq!(round_base_grade(x, "t").unwrap(), n, "{x}");
    }
    // 2x + 0.5 = -2^31 converts exactly (above); at 2^31 and past -2^31, or not finite, it is refused.
    for x in [1.1e9f32, -1.1e9, 1_073_741_824.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let (_, what) = not_yet(&round_base_grade(x, "t").unwrap_err());
        assert!(what.starts_with("IAS-ROUNDING-RANGE: "), "{x}: {what}");
    }
}

/// The skill part in mode 2: no skill and no refusal; the slot's value is still computed.
#[test]
fn mode_2_has_no_runtime_skill_and_is_not_refused() {
    let mut t = Toy::new(ParityMode::Off);
    let ua = t.up("A", 1, true);
    let skill = vec![("bExternalSlot", Value::Bool(true)), ("bRunEffectsAsSkill", Value::Bool(true))];
    let ea = t.effect("A", 1.0, 1.0, skill);
    let d = t.definition(vec![ua], vec![ea], 19, 1.0);
    t.set(d, "AttributeSlotEffectMode", Value::Byte(2));
    let w = t.weapon(Some(d), &[], true);
    initialize(&mut t.vm, w, false, false).unwrap();
    assert_eq!(t.summary(w, 0).3, Value::Float(2.0), "1 + (2 - 1) x 1");
}

/// Not the authority: the skill part is not reached, so a run-as-skill slot is no NotYet there.
#[test]
fn off_the_authority_the_skill_part_is_not_reached() {
    let mut t = Toy::new(ParityMode::Off);
    let ua = t.up("A", 1, true);
    let skill = vec![("bExternalSlot", Value::Bool(true)), ("bRunEffectsAsSkill", Value::Bool(true))];
    let ea = t.effect("A", 1.0, 1.0, skill);
    let d = t.definition(vec![ua], vec![ea], 19, 1.0);
    let w = t.weapon(Some(d), &[], false);
    initialize(&mut t.vm, w, false, false).unwrap();
    assert_eq!(t.summary(w, 0).1, Value::Bool(true));
}

/// Reference binds nothing (architecture R3): the call answers the stub policy, not this module.
#[test]
fn reference_binds_nothing() {
    let t = Toy::new(ParityMode::Reference);
    assert!(!t.vm.has_native(INIT_SLOTS) && !t.vm.has_native(INIT_NAME_PART_SLOTS));
    let o = Toy::new(ParityMode::Off);
    assert!(o.vm.has_native(INIT_SLOTS) && o.vm.has_native(INIT_NAME_PART_SLOTS));
}
