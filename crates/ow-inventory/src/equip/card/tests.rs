//! Tests of `CacheWeaponCard` in Off on a VM whose classes are built by hand (AGENTS.md rule 9: invented classes,
//! objects and values only; the class and property names are the ones NATIVE_CACHE_WEAPON_CARD_BODY.md reads).
//! Expected values are written by hand from the note. Every modifier value is context-free (no attribute, no
//! initialization definition), so its value is its constant times its scale. AI-assisted (Claude Code).

use std::cell::RefCell;
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
use ow_world::query::WorldQuery;

use super::*;
use crate::equip::read_prop;

/// The invented dimensions of `AttributeSlots` and of the replicated values (one more than [`REPLICATED`], so the cap
/// shows).
const SLOTS: i32 = 3;
const REP: i32 = 6;
const ROLES: [&str; 4] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority"];
const AUTHORITY: u8 = 3;

/// What a fixture changes from the declared schema.
#[derive(Clone, Copy, PartialEq)]
enum Schema {
    Good,
    /// The presentation's `BasePriority` declared an int.
    IntBasePriority,
    /// `WeaponCardModifierStats` an array of another struct.
    OtherStats,
    /// The world records no globals object.
    NoGlobals,
    /// `Role` declared a plain byte, with no enum (review r1 of #521, item 7).
    PlainRole,
    /// `GearboxGlobalsDefinition` only reserved, not defined (review r2 of #521, item 2).
    ReservedGearbox,
}

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind)
}

fn r(class: ClassId) -> PropKind {
    PropKind::Object { class }
}

fn arr(kind: PropKind) -> PropKind {
    PropKind::Array(Box::new(kind))
}

struct Fx {
    vm: Vm,
    query: Rc<RefCell<WorldQuery>>,
    weapon: ObjRef,
    /// The weapon type, the globals and their definition.
    t: ObjRef,
    globals: ObjRef,
    gd: ObjRef,
    aid: StructId,
    effect: StructId,
    priority: StructId,
    mapping: StructId,
    n: u32,
}

impl Fx {
    fn new() -> Fx {
        Fx::with(Schema::Good, ParityMode::Off)
    }

    fn with(schema: Schema, mode: ParityMode) -> Fx {
        let mut s = ObjectSpace::new();
        let object = s.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = r(object);
        // The attribute classes a base-value read walks (empty chains unless a test sets them).
        let objs = arr(obj.clone());
        let fields = vec![decl(&mut s, "ContextResolverChain", objs.clone()), decl(&mut s, "ValueResolverChain", objs)];
        let attr_base = s.classes.define("Engine.AttributeDefinitionBase", Some(object), fields).unwrap();
        let attr = s.classes.define("Engine.AttributeDefinition", Some(attr_base), vec![]).unwrap();
        s.classes.define("Test.UnreadContextResolver", Some(object), vec![]).unwrap();
        let fields = vec![
            decl(&mut s, "BaseValueConstant", PropKind::Float),
            decl(&mut s, "BaseValueAttribute", obj.clone()),
            decl(&mut s, "InitializationDefinition", obj.clone()),
            decl(&mut s, "BaseValueScaleConstant", PropKind::Float),
        ];
        let aid = s.classes.define_struct("Engine.AttributeInitializationDefinition.AttributeInitializationData", None, fields).unwrap();
        let fields = vec![
            decl(&mut s, "AttributeToModify", r(attr)),
            decl(&mut s, "ModifierType", PropKind::Byte { enum_: None }),
            decl(&mut s, "BaseModifierValue", PropKind::Struct(aid)),
        ];
        let effect = s.classes.define_struct("Test.AttributeEffectData", None, fields).unwrap();
        let fields = vec![decl(&mut s, "Attribute", r(attr)), decl(&mut s, "PriorityIncrease", PropKind::Float)];
        let priority = s.classes.define_struct("Test.AttributePriorityData", None, fields).unwrap();
        // The presentation, the damage types and the status-effect presentation's mapping.
        let base = if schema == Schema::IntBasePriority { PropKind::Int } else { PropKind::Float };
        let mut fields = vec![decl(&mut s, "BasePriority", base), decl(&mut s, "Attribute", r(attr))];
        for b in [
            "bDisplayAsPercentage",
            "bUseOneAsBaseValueForPresentationIfContextFailsToResolve",
            "bUseTwoAsBaseValueForPresentationIfContextFailsToResolve",
        ] {
            fields.push(decl(&mut s, b, PropKind::Bool));
        }
        let pres = s.classes.define("WillowGame.AttributePresentationDefinition", Some(object), fields).unwrap();
        let dmg = s.classes.define("Engine.DamageTypeDefinition", Some(object), vec![]).unwrap();
        let fields = vec![decl(&mut s, "WeaponCardPresentations", arr(r(pres)))];
        let wdmg = s.classes.define("WillowGame.WillowDamageTypeDefinition", Some(dmg), fields).unwrap();
        let fields = vec![decl(&mut s, "Presentation", r(pres)), decl(&mut s, "DamageType", r(dmg))];
        let mapping = s.classes.define_struct("Test.AttributePresentationDamageTypeMapping", None, fields).unwrap();
        let fields = vec![decl(&mut s, "MappingData", arr(PropKind::Struct(mapping)))];
        s.classes.define("WillowGame.WeaponStatusEffectAttributePresentationDefinition", Some(pres), fields).unwrap();
        // The presentation lists, their DLC holder and the balance.
        let fields = vec![decl(&mut s, "Attributes", arr(r(pres)))];
        let list = s.classes.define("Test.AttributePresentationListDefinition", Some(object), fields).unwrap();
        let lists = |s: &mut ObjectSpace| LISTS.iter().map(|l| decl(s, l, r(list))).collect::<Vec<_>>();
        let fields = lists(&mut s);
        let dlc = s.classes.define("Test.DownloadableItemSetDefinition", Some(object), fields).unwrap();
        let fields = vec![decl(&mut s, "DlcItemSet", r(dlc))];
        let balance = s.classes.define("Test.InventoryBalanceDefinition", Some(object), fields).unwrap();
        // The globals and their definition.
        let gbx_def = if schema == Schema::ReservedGearbox {
            s.classes.declare("GearboxFramework.GearboxGlobalsDefinition").unwrap()
        } else {
            s.classes.define("GearboxFramework.GearboxGlobalsDefinition", Some(object), vec![]).unwrap()
        };
        let fields = lists(&mut s);
        let parent = if schema == Schema::ReservedGearbox { object } else { gbx_def };
        s.classes.define(crate::globals::GLOBALS_DEFINITION_CLASS, Some(parent), fields).unwrap();
        let holder = if schema == Schema::ReservedGearbox { obj.clone() } else { r(gbx_def) };
        let fields = vec![decl(&mut s, "TheGlobalsDefinition", holder)];
        let gbx = s.classes.define("GearboxFramework.GearboxGlobals", Some(object), fields).unwrap();
        s.classes.define("WillowGame.WillowGlobals", Some(gbx), vec![]).unwrap();
        // The type, the part and the name part (a part).
        let name_part = s.classes.declare(super::super::name_parts::NAME_PART_CLASS).unwrap();
        let card_fields = |s: &mut ObjectSpace| {
            vec![
                decl(s, "WeaponCardAttributes", arr(PropKind::Struct(priority))),
                decl(s, "CustomPresentations", arr(r(pres))),
                decl(s, "WeaponAttributeEffects", arr(PropKind::Struct(effect))),
                decl(s, "ExternalAttributeEffects", arr(PropKind::Struct(effect))),
                decl(s, "PrefixList", arr(r(name_part))),
                decl(s, "TitleList", arr(r(name_part))),
            ]
        };
        let mut fields = card_fields(&mut s);
        fields.push(decl(&mut s, "DefaultDamageTypeDefinition", r(wdmg)));
        s.classes.define(WEAPON_TYPE_CLASS, Some(object), fields).unwrap();
        let mut fields = card_fields(&mut s);
        fields.push(decl(&mut s, "CustomDamageTypeDefinition", r(wdmg)));
        let part = s.classes.define(WEAPON_PART_CLASS, Some(object), fields).unwrap();
        let fields = vec![decl(&mut s, "AttributeOperand1", obj.clone())];
        let expr = s.classes.define_struct("Test.AttributeExpressionData", None, fields).unwrap();
        let fields = vec![
            decl(&mut s, "MinExpLevelRequirement", PropKind::Int),
            decl(&mut s, "MaxExpLevelRequirement", PropKind::Int),
            decl(&mut s, "Priority", PropKind::Float),
            decl(&mut s, "Expressions", arr(PropKind::Struct(expr))),
        ];
        s.classes.define(super::super::name_parts::NAME_PART_CLASS, Some(part), fields).unwrap();
        // A definition with the card's fields that is neither a type nor a part.
        let fields = card_fields(&mut s);
        s.classes.define("Test.Other", Some(object), fields).unwrap();
        // The weapon.
        let mut fields = vec![decl(&mut s, "WeaponTypeDefinition", obj.clone())];
        for f in WEAPON_PART_FIELDS {
            fields.push(decl(&mut s, f, obj.clone()));
        }
        fields.push(decl(&mut s, "PrefixPartDefinition", r(name_part)));
        fields.push(decl(&mut s, "TitlePartDefinition", r(name_part)));
        fields.push(decl(&mut s, "BalanceDefinition", r(balance)));
        let data = s.classes.define_struct("Test.WeaponDefinitionData", None, fields).unwrap();
        let fields = vec![
            decl(&mut s, "AttributePresentation", r(pres)),
            decl(&mut s, "OptionalConstraintPresentation", r(pres)),
            decl(&mut s, "ModifierValue", PropKind::Float),
            decl(&mut s, "bShouldDisplay", PropKind::Bool),
        ];
        let stats_path = if schema == Schema::OtherStats { "Test.OtherPresentationData" } else { "Test.ModifierValuePresentationData" };
        let stats = s.classes.define_struct(stats_path, None, fields).unwrap();
        let fields = vec![
            decl(&mut s, "bActivated", PropKind::Bool),
            decl(&mut s, "bIncludeInModifierText", PropKind::Bool),
            decl(&mut s, "AttributeToModify", obj.clone()),
            decl(&mut s, "ConstraintAttribute", obj.clone()),
            decl(&mut s, "ComputedModifierValue", PropKind::Float),
        ];
        let slot = s.classes.define_struct("Test.AttributeSlotData", None, fields).unwrap();
        let fields = vec![decl(&mut s, "ModifierValue", PropKind::Float), decl(&mut s, "ModifierStatIndex", PropKind::Byte { enum_: None })];
        let rep = s.classes.define_struct("Test.ReplicatedWeaponCardModifierValue", None, fields).unwrap();
        let roles = ROLES.iter().map(|n| s.names.intern(n)).collect();
        let role = s.classes.define_enum("Engine.Actor.ENetRole", roles).unwrap();
        let fields = vec![
            decl(&mut s, "DefinitionData", PropKind::Struct(data)),
            decl(&mut s, "ExpLevel", PropKind::Int),
            decl(&mut s, "WeaponCardModifierStats", arr(PropKind::Struct(stats))),
            decl(&mut s, "AttributeSlots", PropKind::Struct(slot)).with_array_dim(SLOTS),
            decl(&mut s, "ReplicatedWeaponCardModifierValues", PropKind::Struct(rep)).with_array_dim(REP),
            decl(&mut s, "Role", PropKind::Byte { enum_: (schema != Schema::PlainRole).then_some(role) }),
        ];
        let weapon_class = s.classes.define(WEAPON_CLASS, Some(object), fields).unwrap();
        let weapon = s.alloc(weapon_class, "Weapon_0", None, Origin::Runtime).unwrap();
        let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::standard().unwrap()), mode);
        let query = ow_world::query::install(&mut vm).unwrap();
        let mut x = Fx { vm, query, weapon, t: weapon, globals: weapon, gd: weapon, aid, effect, priority, mapping, n: 0 };
        x.set(weapon, "Role", Value::Byte(AUTHORITY));
        x.globals = x.obj("WillowGame.WillowGlobals");
        x.gd = x.obj(crate::globals::GLOBALS_DEFINITION_CLASS);
        let (g, gd) = (x.globals, x.gd);
        x.set(g, "TheGlobalsDefinition", Value::Object(Some(gd)));
        if schema != Schema::NoGlobals {
            x.query.borrow_mut().set_globals(g);
        }
        x.t = x.obj(WEAPON_TYPE_CLASS);
        let t = x.t;
        x.data(&[("WeaponTypeDefinition", Some(t))]);
        // Invented old contents: one stale stat, and replicated values of 9 at index 7.
        let stale = x.stat_value(Stat { presentation: None, constraint: None, value: 4.0, display: false });
        x.set(weapon, "WeaponCardModifierStats", Value::array(vec![stale]));
        for i in 0..REP {
            x.set_rep(i, 9.0, 7);
        }
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

    fn st(&mut self, ty: StructId, fields: Vec<(&str, Value)>) -> Value {
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.vm.space.names.find(n).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = x;
        }
        v
    }

    fn prop_struct(&self, name: &str) -> StructId {
        let class = self.vm.space.heap.get(self.weapon).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        match &self.vm.space.classes.decl(p).unwrap().kind {
            PropKind::Struct(s) => *s,
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(s) => s,
                ref k => panic!("{k:?}"),
            },
            k => panic!("{k:?}"),
        }
    }

    fn stat_value(&mut self, s: Stat) -> Value {
        let ty = self.prop_struct("WeaponCardModifierStats");
        self.st(
            ty,
            vec![
                ("AttributePresentation", Value::Object(s.presentation)),
                ("OptionalConstraintPresentation", Value::Object(s.constraint)),
                ("ModifierValue", Value::Float(s.value)),
                ("bShouldDisplay", Value::Bool(s.display)),
            ],
        )
    }

    fn element(&self, name: &str) -> ow_object::ids::PropId {
        let class = self.vm.space.heap.get(self.weapon).unwrap().class;
        self.vm.space.prop(class, name).unwrap()
    }

    fn set_rep(&mut self, i: i32, value: f32, index: u8) {
        let ty = self.prop_struct("ReplicatedWeaponCardModifierValues");
        let v = self.st(ty, vec![("ModifierValue", Value::Float(value)), ("ModifierStatIndex", Value::Byte(index))]);
        let p = self.element("ReplicatedWeaponCardModifierValues");
        let w = self.weapon;
        self.vm.space.write(w, PropAddr::element(p, u32::try_from(i).unwrap()), v).unwrap();
    }

    fn rep(&self, i: u32) -> (Value, Value) {
        let p = self.element("ReplicatedWeaponCardModifierValues");
        let v = self.vm.space.read(self.weapon, PropAddr::element(p, i)).unwrap().clone();
        (field(&self.vm, &v, "ModifierValue").unwrap(), field(&self.vm, &v, "ModifierStatIndex").unwrap())
    }

    /// Slot `i`: activated, included in the text, its two attributes and its computed value.
    fn slot(&mut self, i: u32, activated: bool, include: bool, modify: Option<ObjRef>, constraint: Option<ObjRef>, value: f32) {
        let ty = self.prop_struct("AttributeSlots");
        let v = self.st(
            ty,
            vec![
                ("bActivated", Value::Bool(activated)),
                ("bIncludeInModifierText", Value::Bool(include)),
                ("AttributeToModify", Value::Object(modify)),
                ("ConstraintAttribute", Value::Object(constraint)),
                ("ComputedModifierValue", Value::Float(value)),
            ],
        );
        let p = self.element("AttributeSlots");
        let w = self.weapon;
        self.vm.space.write(w, PropAddr::element(p, i), v).unwrap();
    }

    fn slot_flag(&self, i: u32) -> Value {
        let p = self.element("AttributeSlots");
        let v = self.vm.space.read(self.weapon, PropAddr::element(p, i)).unwrap().clone();
        field(&self.vm, &v, "bIncludeInModifierText").unwrap()
    }

    /// The weapon's definition data with these fields set (the rest None).
    fn data(&mut self, fields: &[(&str, Option<ObjRef>)]) {
        let ty = self.prop_struct("DefinitionData");
        let v = self.st(ty, fields.iter().map(|(n, o)| (*n, Value::Object(*o))).collect());
        let w = self.weapon;
        self.set(w, "DefinitionData", v);
    }

    fn attribute(&mut self) -> ObjRef {
        self.obj("Engine.AttributeDefinition")
    }

    /// A presentation of `class` with `base` priority for attribute `a`.
    fn pres_of(&mut self, class: &str, base: f32, a: Option<ObjRef>) -> ObjRef {
        let p = self.obj(class);
        self.set(p, "BasePriority", Value::Float(base));
        self.set(p, "Attribute", Value::Object(a));
        p
    }

    fn pres(&mut self, base: f32, a: Option<ObjRef>) -> ObjRef {
        self.pres_of("WillowGame.AttributePresentationDefinition", base, a)
    }

    fn objs(l: &[Option<ObjRef>]) -> Value {
        Value::array(l.iter().map(|o| Value::Object(*o)).collect())
    }

    /// Sets list field `which` of `holder` to a new list holding `entries`.
    fn list(&mut self, holder: ObjRef, which: &str, entries: &[Option<ObjRef>]) {
        let l = self.obj("Test.AttributePresentationListDefinition");
        self.set(l, "Attributes", Fx::objs(entries));
        self.set(holder, which, Value::Object(Some(l)));
    }

    /// A balance with a DLC set; the set.
    fn dlc(&mut self) -> ObjRef {
        let d = self.obj("Test.DownloadableItemSetDefinition");
        let b = self.obj("Test.InventoryBalanceDefinition");
        self.set(b, "DlcItemSet", Value::Object(Some(d)));
        let t = self.t;
        self.data(&[("WeaponTypeDefinition", Some(t)), ("BalanceDefinition", Some(b))]);
        d
    }

    /// An effect on `a` of kind `kind` (0 scale, 1 pre-add, 2 post-add) and constant value `c`.
    fn effect(&mut self, a: ObjRef, kind: u8, c: f32) -> Value {
        let aid = self.aid;
        let init = self.st(aid, vec![("BaseValueConstant", Value::Float(c)), ("BaseValueScaleConstant", Value::Float(1.0))]);
        let e = self.effect;
        self.st(e, vec![("AttributeToModify", Value::Object(Some(a))), ("ModifierType", Value::Byte(kind)), ("BaseModifierValue", init)])
    }

    fn priority_row(&mut self, a: ObjRef, inc: f32) -> Value {
        let p = self.priority;
        self.st(p, vec![("Attribute", Value::Object(Some(a))), ("PriorityIncrease", Value::Float(inc))])
    }

    fn mapping_row(&mut self, p: ObjRef, damage: Option<ObjRef>) -> Value {
        let m = self.mapping;
        self.st(m, vec![("Presentation", Value::Object(Some(p))), ("DamageType", Value::Object(damage))])
    }

    fn stats(&self) -> Vec<Stat> {
        let v = read_prop(&self.vm, self.weapon, "WeaponCardModifierStats").unwrap();
        let o = |e: &Value, n: &str| match field(&self.vm, e, n).unwrap() {
            Value::Object(o) => o,
            other => panic!("{n}: {other:?}"),
        };
        ow_attr::eval::elements("stats", &v)
            .unwrap()
            .iter()
            .map(|e| Stat {
                presentation: o(e, "AttributePresentation"),
                constraint: o(e, "OptionalConstraintPresentation"),
                value: match field(&self.vm, e, "ModifierValue").unwrap() {
                    Value::Float(f) => f,
                    other => panic!("{other:?}"),
                },
                display: field(&self.vm, e, "bShouldDisplay").unwrap() == Value::Bool(true),
            })
            .collect()
    }

    fn shown(&self) -> Vec<Option<ObjRef>> {
        self.stats().iter().map(|s| s.presentation).collect()
    }

    fn run(&mut self) -> Result<(), VmError> {
        let w = self.weapon;
        cache_weapon_card(&mut self.vm, w)
    }

    /// The old contents [`Fx::with`] wrote are still there.
    fn untouched(&self) {
        assert_eq!(self.stats().len(), 1, "the stale stat is still the only one");
        assert_eq!(self.stats()[0].value, 4.0);
        assert_eq!(self.rep(0), (Value::Float(9.0), Value::Byte(7)));
    }
}

fn not_yet(e: VmError) -> ow_core::NotYet {
    match e {
        VmError::NotYet(n) => n,
        VmError::Attr(ow_attr::api::AttrError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// A named `NotRead` of this record, unprefixed.
fn refused(e: VmError, what: &str) {
    let n = not_yet(e);
    assert_eq!(n.what, what);
    assert_eq!(n.missing, Missing::NotRead);
    assert_eq!(n.record, RECORD);
}

fn script(e: VmError) -> String {
    match e {
        VmError::Script { message, .. } => message,
        other => panic!("expected VmError::Script, got {other:?}"),
    }
}

// ---- The presentation lookup (section 2) ------------------------------------------------------------------------

/// Slot 0 shows the lookup: activated and included, its `AttributeToModify` presentation is the slot entry's.
fn slot0_shows(x: &mut Fx, a: ObjRef) -> Option<ObjRef> {
    x.slot(0, true, true, Some(a), None, 1.5);
    x.run().unwrap();
    x.stats()[0].presentation
}

#[test]
fn the_lookup_takes_the_dlc_lists_then_the_globals_lists_each_override_first() {
    // Levels: DLC override, DLC default, globals override, globals default; the first that holds the attribute wins.
    for first in 0..4 {
        let mut x = Fx::new();
        let a = x.attribute();
        let d = x.dlc();
        let gd = x.gd;
        let mut ps = Vec::new();
        for (level, (holder, list)) in [(d, LISTS[0]), (d, LISTS[1]), (gd, LISTS[0]), (gd, LISTS[1])].into_iter().enumerate() {
            let p = x.pres(1.0, Some(a));
            ps.push(p);
            if level >= first {
                x.list(holder, list, &[Some(p)]);
            }
        }
        assert_eq!(slot0_shows(&mut x, a), Some(ps[first]), "level {first}");
    }
}

#[test]
fn the_first_non_none_entry_for_the_attribute_is_taken_and_no_balance_reads_only_the_globals() {
    let mut x = Fx::new();
    let (a, b) = (x.attribute(), x.attribute());
    let (other, first, second) = (x.pres(1.0, Some(b)), x.pres(1.0, Some(a)), x.pres(1.0, Some(a)));
    let gd = x.gd;
    x.list(gd, LISTS[1], &[None, Some(other), Some(first), Some(second)]);
    assert_eq!(slot0_shows(&mut x, a), Some(first));
}

#[test]
fn a_status_presentation_is_replaced_by_its_mapping_for_the_weapons_damage_type() {
    let status = "WillowGame.WeaponStatusEffectAttributePresentationDefinition";
    // (type default damage, elemental part's custom damage, which mapping row wins: 0 = the other, 1 = the type's,
    // 2 = None's, none = no match)
    for case in 0..4 {
        let mut x = Fx::new();
        let a = x.attribute();
        let s = x.pres_of(status, 1.0, Some(a));
        let gd = x.gd;
        x.list(gd, LISTS[1], &[Some(s)]);
        let (fire, ice) = (x.obj("WillowGame.WillowDamageTypeDefinition"), x.obj("WillowGame.WillowDamageTypeDefinition"));
        let (on_ice, on_fire, on_none) = (x.pres(1.0, Some(a)), x.pres(1.0, Some(a)), x.pres(1.0, Some(a)));
        let rows = match case {
            3 => vec![x.mapping_row(on_ice, Some(ice))],
            _ => vec![x.mapping_row(on_ice, Some(ice)), x.mapping_row(on_fire, Some(fire)), x.mapping_row(on_none, None)],
        };
        x.set(s, "MappingData", Value::array(rows));
        let t = x.t;
        match case {
            0 | 3 => x.set(t, "DefaultDamageTypeDefinition", Value::Object(Some(fire))),
            1 => {
                // The elemental part's custom damage type comes first.
                x.set(t, "DefaultDamageTypeDefinition", Value::Object(Some(fire)));
                let e = x.obj(WEAPON_PART_CLASS);
                x.set(e, "CustomDamageTypeDefinition", Value::Object(Some(ice)));
                x.data(&[("WeaponTypeDefinition", Some(t)), ("ElementalPartDefinition", Some(e))]);
            }
            _ => {}
        }
        let shown = slot0_shows(&mut x, a);
        let want = [Some(on_fire), Some(on_ice), Some(on_none), None][case];
        assert_eq!(shown, want, "case {case}");
        // No match: the slot entry has no presentation and slot 0's flag is cleared.
        assert_eq!(x.slot_flag(0), Value::Bool(case != 3), "case {case}");
    }
}

// ---- Slot 0 and the slot count (step 3) -------------------------------------------------------------------------

#[test]
fn slot_zero_gives_its_entry_with_its_constraint_presentation_and_value() {
    let mut x = Fx::new();
    let (a, c) = (x.attribute(), x.attribute());
    let (pa, pc) = (x.pres(1.0, Some(a)), x.pres(1.0, Some(c)));
    let gd = x.gd;
    x.list(gd, LISTS[1], &[Some(pa), Some(pc)]);
    x.slot(0, true, true, Some(a), Some(c), 2.5);
    x.run().unwrap();
    assert_eq!(x.stats(), vec![Stat { presentation: Some(pa), constraint: Some(pc), value: 2.5, display: true }]);
    assert_eq!(x.slot_flag(0), Value::Bool(true), "found: the flag is kept");
}

#[test]
fn slot_zero_not_included_or_not_activated_gives_no_entry() {
    for (activated, include) in [(true, false), (false, true)] {
        let mut x = Fx::new();
        let a = x.attribute();
        x.slot(0, activated, include, Some(a), None, 2.5);
        x.run().unwrap();
        assert!(x.stats().is_empty(), "{activated} {include}: the stale stat is cleared, nothing added");
        assert_eq!(x.slot_flag(0), Value::Bool(include), "a slot with no entry keeps its flag");
    }
}

#[test]
fn an_activated_slot_past_zero_is_the_slot_count_refusal_with_nothing_written() {
    for i in 1..u32::try_from(SLOTS).unwrap() {
        let mut x = Fx::new();
        let a = x.attribute();
        x.slot(0, true, true, Some(a), None, 1.0);
        x.slot(i, true, false, None, None, 0.0);
        refused(x.run().unwrap_err(), SLOT_COUNT);
        x.untouched();
    }
}

// ---- The entries, their order and the refusals (sections 3.2 and 3.3) --------------------------------------------

#[test]
fn custom_and_damage_presentations_are_appended_largest_priority_first() {
    let mut x = Fx::new();
    let (p1, p2, p3, low) = (x.pres(1.0, None), x.pres(2.0, None), x.pres(3.0, None), x.pres(0.00005, None));
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p1), None, Some(low)]));
    let part = x.obj(WEAPON_PART_CLASS);
    x.set(part, "CustomPresentations", Fx::objs(&[Some(p3), Some(p1)]));
    let dmg = x.obj("WillowGame.WillowDamageTypeDefinition");
    x.set(dmg, "WeaponCardPresentations", Fx::objs(&[Some(p2)]));
    x.set(t, "DefaultDamageTypeDefinition", Value::Object(Some(dmg)));
    x.data(&[("WeaponTypeDefinition", Some(t)), ("GripPartDefinition", Some(part))]);
    x.run().unwrap();
    assert_eq!(x.shown(), vec![Some(p3), Some(p2), Some(p1)], "below 0.0001 left out; p1 seen twice adds 0");
    assert!(x.stats().iter().all(|s| s.value == 0.0 && s.display && s.constraint.is_none()));
}

#[test]
fn a_card_attribute_adds_its_increase_to_an_existing_entry_and_needs_a_record_for_a_new_one() {
    let mut x = Fx::new();
    let (a, b) = (x.attribute(), x.attribute());
    let (pa, pb, p2) = (x.pres(1.0, Some(a)), x.pres(9.0, Some(b)), x.pres(2.0, None));
    let gd = x.gd;
    x.list(gd, LISTS[1], &[Some(pa), Some(pb)]);
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(pa), Some(p2)]));
    // The part's card attributes, visited after the type: `a` raises pa by 5; `b` has no record, so pb is skipped.
    let part = x.obj(WEAPON_PART_CLASS);
    let rows = vec![x.priority_row(a, 5.0), x.priority_row(b, 1.0)];
    x.set(part, "WeaponCardAttributes", Value::array(rows));
    x.data(&[("WeaponTypeDefinition", Some(t)), ("BodyPartDefinition", Some(part))]);
    x.run().unwrap();
    assert_eq!(x.shown(), vec![Some(pa), Some(p2)], "pa at 1 + 5 before p2 at 2");
}

#[test]
fn an_appended_entry_with_a_record_is_the_display_refusal() {
    // A record from the first map (a card attribute), from the second (a custom presentation), and from a zero value
    // (a record is made on first sight).
    for case in 0..3 {
        let mut x = Fx::new();
        let a = x.attribute();
        let pa = x.pres(1.0, Some(a));
        let gd = x.gd;
        x.list(gd, LISTS[1], &[Some(pa)]);
        let t = x.t;
        let (list, value) = match case {
            0 => ("WeaponAttributeEffects", 2.0),
            1 => ("ExternalAttributeEffects", 2.0),
            _ => ("WeaponAttributeEffects", 0.0),
        };
        let e = x.effect(a, 1, value);
        x.set(t, list, Value::array(vec![e]));
        if case == 1 {
            x.set(t, "CustomPresentations", Fx::objs(&[Some(pa)]));
        } else {
            let row = x.priority_row(a, 0.5);
            x.set(t, "WeaponCardAttributes", Value::array(vec![row]));
        }
        refused(x.run().unwrap_err(), DISPLAY);
        x.untouched();
    }
}

#[test]
fn an_entry_with_a_record_below_the_threshold_is_left_out_without_a_refusal() {
    let mut x = Fx::new();
    let a = x.attribute();
    let pa = x.pres(0.0, Some(a));
    let gd = x.gd;
    x.list(gd, LISTS[1], &[Some(pa)]);
    let t = x.t;
    let e = x.effect(a, 0, -0.5);
    x.set(t, "WeaponAttributeEffects", Value::array(vec![e]));
    let row = x.priority_row(a, 0.0);
    x.set(t, "WeaponCardAttributes", Value::array(vec![row]));
    x.run().unwrap();
    assert!(x.stats().is_empty());
}

#[test]
fn two_appended_entries_of_equal_priority_are_the_tie_refusal() {
    let mut x = Fx::new();
    let (p, q) = (x.pres(2.0, None), x.pres(2.0, None));
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p), Some(q)]));
    refused(x.run().unwrap_err(), TIE);
    x.untouched();
}

#[test]
fn a_visited_definition_that_is_neither_a_type_nor_a_part_is_the_visit_refusal() {
    let mut x = Fx::new();
    let t = x.t;
    let other = x.obj("Test.Other");
    x.data(&[("WeaponTypeDefinition", Some(t)), ("SightPartDefinition", Some(other))]);
    refused(x.run().unwrap_err(), VISIT);
    x.untouched();
}

// ---- The writes (step 7) ------------------------------------------------------------------------------------------

#[test]
fn the_authority_replicates_the_entries_after_the_slot_entry() {
    let mut x = Fx::new();
    let a = x.attribute();
    let pa = x.pres(1.0, Some(a));
    let gd = x.gd;
    x.list(gd, LISTS[1], &[Some(pa)]);
    x.slot(0, true, true, Some(a), None, 3.0);
    let (p1, p2) = (x.pres(1.0, None), x.pres(2.0, None));
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p1), Some(p2)]));
    x.run().unwrap();
    assert_eq!(x.shown(), vec![Some(pa), Some(p2), Some(p1)], "the slot entry first");
    assert_eq!(x.rep(0), (Value::Float(0.0), Value::Byte(1)));
    assert_eq!(x.rep(1), (Value::Float(0.0), Value::Byte(2)));
    assert_eq!(x.rep(2), (Value::Float(9.0), Value::Byte(7)), "the rest left as they were");
}

#[test]
fn replication_fills_at_most_five_and_only_on_the_authority() {
    for role in [AUTHORITY, 1] {
        let mut x = Fx::new();
        let w = x.weapon;
        x.set(w, "Role", Value::Byte(role));
        let ps: Vec<_> = (1..=6).map(|i| Some(x.pres(i as f32, None))).collect();
        let t = x.t;
        x.set(t, "CustomPresentations", Fx::objs(&ps));
        x.run().unwrap();
        assert_eq!(x.stats().len(), 6, "all six entries are written");
        for i in 0..u32::try_from(REP).unwrap() {
            let want = if role == AUTHORITY && i < 5 {
                (Value::Float(0.0), Value::Byte(u8::try_from(i).unwrap()))
            } else {
                (Value::Float(9.0), Value::Byte(7))
            };
            assert_eq!(x.rep(i), want, "role {role}, index {i}");
        }
    }
}

#[test]
fn a_weapon_with_no_type_writes_nothing() {
    let mut x = Fx::new();
    x.data(&[]);
    x.run().unwrap();
    x.untouched();
}

// ---- The globals (step 1) and the preflight (section 4) -----------------------------------------------------------

#[test]
fn no_globals_is_layer_absent_and_an_unset_definition_is_not_read() {
    let mut x = Fx::with(Schema::NoGlobals, ParityMode::Off);
    assert!(matches!(not_yet(x.run().unwrap_err()).missing, Missing::LayerAbsent(_)));
    x.untouched();
    let mut x = Fx::new();
    let g = x.globals;
    x.set(g, "TheGlobalsDefinition", Value::Object(None));
    assert_eq!(not_yet(x.run().unwrap_err()).missing, Missing::NotRead);
    x.untouched();
}

#[test]
fn the_preflight_holds_on_the_declared_classes() {
    let mut x = Fx::new();
    preflight(&mut x.vm).unwrap();
}

#[test]
fn the_preflight_refuses_globals_of_another_class_and_an_unset_definition() {
    let mut x = Fx::new();
    let other = x.obj("GearboxFramework.GearboxGlobals");
    let gd = x.gd;
    x.set(other, "TheGlobalsDefinition", Value::Object(Some(gd)));
    x.query.borrow_mut().set_globals(other);
    assert!(script(preflight(&mut x.vm).unwrap_err()).contains("not a WillowGlobals"));
    let mut x = Fx::new();
    let g = x.globals;
    x.set(g, "TheGlobalsDefinition", Value::Object(None));
    assert_eq!(not_yet(preflight(&mut x.vm).unwrap_err()).missing, Missing::NotRead);
}

#[test]
fn each_card_schema_failure_is_a_script_error_naming_the_field() {
    for (schema, field) in
        [(Schema::IntBasePriority, "BasePriority"), (Schema::OtherStats, "WeaponCardModifierStats"), (Schema::PlainRole, "Role")]
    {
        let mut x = Fx::with(schema, ParityMode::Off);
        let m = script(preflight(&mut x.vm).unwrap_err());
        assert!(m.contains(field), "{field}: {m}");
    }
}

// ---- Registration ---------------------------------------------------------------------------------------------

fn declared(x: &mut Fx) -> ow_object::ids::FuncId {
    let weapon = x.vm.space.classes.by_path(WEAPON_CLASS).unwrap();
    let path = ObjectPath::parse(CACHE_WEAPON_CARD).unwrap();
    let f = x.vm.declare_function(FunctionDecl::native(path, "Test.CardKey").with_owner(weapon)).unwrap();
    register(&mut x.vm).unwrap();
    f
}

#[test]
fn register_binds_the_native_by_its_own_key_in_off() {
    let mut x = Fx::new();
    let f = declared(&mut x);
    let p = x.pres(1.0, None);
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p)]));
    let w = x.weapon;
    x.vm.call(f, Some(w), vec![]).unwrap();
    assert_eq!(x.shown(), vec![Some(p)]);
}

#[test]
fn reference_binds_nothing() {
    let mut x = Fx::with(Schema::Good, ParityMode::Reference);
    let f = declared(&mut x);
    let w = x.weapon;
    assert!(x.vm.call(f, Some(w), vec![]).is_err(), "Reference ran an unbound native");
    x.untouched();
}

// ---- Review r1 of #521 -------------------------------------------------------------------------------------------

/// Item 7: a plain-byte `Role` fails before any card write (the authority is resolved while computing).
#[test]
fn a_role_with_no_authority_enum_fails_before_the_card_is_written() {
    let mut x = Fx::with(Schema::PlainRole, ParityMode::Off);
    let p = x.pres(1.0, None);
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p)]));
    assert!(script(x.run().unwrap_err()).contains("Role"));
    x.untouched();
}

/// Item 5: an appended entry's base value is read, and its refusal passes through typed with nothing written.
#[test]
fn an_appended_entry_whose_base_value_read_refuses_passes_the_refusal_through() {
    let mut x = Fx::new();
    let resolver = x.obj("Test.UnreadContextResolver");
    let a = x.attribute();
    x.set(a, "ContextResolverChain", Fx::objs(&[Some(resolver)]));
    let p = x.pres(1.0, Some(a));
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p)]));
    let n = not_yet(x.run().unwrap_err());
    assert!(n.what.contains("Test.UnreadContextResolver"), "{}", n.what);
    assert!(!n.what.contains("partial writes"), "nothing loaded or written: {}", n.what);
    x.untouched();
}

/// Item 2: the slot count is checked before the type is read past its field: a type that would fail to read (a
/// definition of another class, which declares no `DefaultDamageTypeDefinition`) is never reached.
#[test]
fn the_slot_count_comes_before_the_type_is_read() {
    let mut x = Fx::new();
    let other = x.obj("Test.Other");
    x.data(&[("WeaponTypeDefinition", Some(other))]);
    // Without the slot, the type's read is an error.
    assert!(script(x.run().unwrap_err()).contains("DefaultDamageTypeDefinition"));
    x.slot(1, true, false, None, None, 0.0);
    refused(x.run().unwrap_err(), SLOT_COUNT);
    x.untouched();
}

/// Item 2: a failure after the computation loaded or allocated is marked once; one with no change is not.
#[test]
fn a_failure_after_a_load_of_the_computation_is_marked() {
    let mut x = Fx::new();
    let e = marked_after_loads(&mut x.vm, |vm| -> Result<(), VmError> {
        let c = vm.space.classes.by_path("Test.Other").unwrap();
        vm.space.alloc(c, "Allocated", None, Origin::Runtime).unwrap();
        Err(not_read(TIE))
    })
    .unwrap_err();
    let n = not_yet(e);
    assert!(n.what.starts_with("stopped after partial writes: "), "{}", n.what);
    assert_eq!(n.what.matches("partial writes").count(), 1);
    let e = marked_after_loads(&mut x.vm, |_| -> Result<(), VmError> { Err(not_read(TIE)) }).unwrap_err();
    assert_eq!(not_yet(e).what, TIE);
}

// ---- Review r2 of #521 -------------------------------------------------------------------------------------------

/// An initialization definition whose `RandomVariance` is enabled (bounds 0 and 1): evaluating an effect that names it
/// draws once from the attribute-variance stream.
fn variance_definition(x: &mut Fx) -> ObjRef {
    let (aid, object) = (x.aid, x.vm.space.classes.by_path("Core.Object").unwrap());
    let sp = &mut x.vm.space;
    let enabled = |sp: &mut ObjectSpace, path: &str, extra: Vec<PropertyDecl>| {
        let mut fields = vec![decl(sp, "bEnabled", PropKind::Bool)];
        fields.extend(extra);
        sp.classes.define_struct(path, None, fields).unwrap()
    };
    let formula = enabled(sp, "Test.AttributeInitializationFormula", vec![]);
    let conditional = enabled(sp, "Test.ConditionalInitialization", vec![]);
    let extra = vec![
        decl(sp, "LowerBound", PropKind::Struct(aid)),
        decl(sp, "UpperBound", PropKind::Struct(aid)),
        decl(sp, "bUseIntegerRandomization", PropKind::Bool),
    ];
    let variance = enabled(sp, "Test.AttributeInitializationRandomVariance", extra);
    let fields =
        vec![decl(sp, "bEnableMinValueRestriction", PropKind::Bool), decl(sp, "bEnableMaxValueRestriction", PropKind::Bool)];
    let range = sp.classes.define_struct("Test.AttributeInitializationRangeRestriction", None, fields).unwrap();
    let fields = vec![
        decl(sp, "ValueFormula", PropKind::Struct(formula)),
        decl(sp, "ConditionalInitialization", PropKind::Struct(conditional)),
        decl(sp, "RandomVariance", PropKind::Struct(variance)),
        decl(sp, "RangeRestriction", PropKind::Struct(range)),
        decl(sp, "BaseValueMode", PropKind::Byte { enum_: None }),
        decl(sp, "RoundingMode", PropKind::Byte { enum_: None }),
    ];
    sp.classes.define("Test.AttributeInitializationDefinition", Some(object), fields).unwrap();
    let d = x.obj("Test.AttributeInitializationDefinition");
    let bound = |x: &mut Fx, c: f32| x.st(aid, vec![("BaseValueConstant", Value::Float(c)), ("BaseValueScaleConstant", Value::Float(1.0))]);
    let (low, high) = (bound(x, 0.0), bound(x, 1.0));
    let v = x.st(variance, vec![("bEnabled", Value::Bool(true)), ("LowerBound", low), ("UpperBound", high)]);
    x.set(d, "RandomVariance", v);
    d
}

/// Item 1: a random draw is a write. Through the native's binding, an effect whose value draws from the variance
/// stream, then two appended entries of equal priority: the tie refusal, marked once, with the draw counted and
/// nothing else written.
#[test]
fn a_draw_then_a_refusal_through_the_binding_is_marked() {
    let mut x = Fx::new();
    let f = declared(&mut x);
    let d = variance_definition(&mut x);
    let a = x.attribute();
    let aid = x.aid;
    let init = x.st(
        aid,
        vec![("InitializationDefinition", Value::Object(Some(d))), ("BaseValueScaleConstant", Value::Float(1.0))],
    );
    let e = x.effect;
    let effect = x.st(e, vec![("AttributeToModify", Value::Object(Some(a))), ("ModifierType", Value::Byte(1)), ("BaseModifierValue", init)]);
    let (p, q) = (x.pres(2.0, None), x.pres(2.0, None));
    let t = x.t;
    x.set(t, "WeaponAttributeEffects", Value::array(vec![effect]));
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p), Some(q)]));
    let (writes, draws, loads) = (x.vm.space.heap.committed_writes(), x.vm.rng.draws(), x.vm.defs.load_transitions());
    let w = x.weapon;
    let n = not_yet(x.vm.call(f, Some(w), vec![]).unwrap_err());
    assert_eq!(n.what, format!("stopped after partial writes: {TIE}"));
    assert_eq!(n.missing, Missing::NotRead);
    assert_eq!(n.record, RECORD);
    assert_eq!(x.vm.rng.draws(), draws + 1, "one variance draw");
    assert_eq!((x.vm.space.heap.committed_writes(), x.vm.defs.load_transitions()), (writes, loads), "no heap write, no load");
    x.untouched();
    // Without the draw the same tie is unmarked.
    let mut x = Fx::new();
    let f = declared(&mut x);
    let (p, q) = (x.pres(2.0, None), x.pres(2.0, None));
    let t = x.t;
    x.set(t, "CustomPresentations", Fx::objs(&[Some(p), Some(q)]));
    let w = x.weapon;
    assert_eq!(not_yet(x.vm.call(f, Some(w), vec![]).unwrap_err()).what, TIE);
}

/// Item 2: the preflight's globals check looks the declared holder class up without loading it: a reserved
/// `GearboxGlobalsDefinition` is a data error naming it, and stays reserved.
#[test]
fn the_preflight_globals_check_loads_no_class() {
    let mut x = Fx::with(Schema::ReservedGearbox, ParityMode::Off);
    let m = script(preflight(&mut x.vm).unwrap_err());
    assert!(m.contains("GearboxFramework.GearboxGlobalsDefinition is not loaded"), "{m}");
    let c = x.vm.space.classes.by_path("GearboxFramework.GearboxGlobalsDefinition").unwrap();
    assert!(!x.vm.space.classes.get_layout(c).unwrap().is_defined(), "still reserved");
}

/// Item 6: a key bound before registration keeps its first handler.
#[test]
fn a_key_bound_first_keeps_its_handler() {
    let mut x = Fx::new();
    let weapon = x.vm.space.classes.by_path(WEAPON_CLASS).unwrap();
    let path = ObjectPath::parse(CACHE_WEAPON_CARD).unwrap();
    let f = x.vm.declare_function(FunctionDecl::native(path, "Test.CardKey").with_owner(weapon)).unwrap();
    let ran = Rc::new(std::cell::Cell::new(false));
    let seen = ran.clone();
    x.vm.register_native(
        "Test.CardKey",
        Rc::new(move |_: &mut ow_vm::api::NativeCall<'_>| {
            seen.set(true);
            Ok(Value::None)
        }),
    )
    .unwrap();
    register(&mut x.vm).unwrap();
    let w = x.weapon;
    x.vm.call(f, Some(w), vec![]).unwrap();
    assert!(ran.get(), "the first handler ran");
    x.untouched();
}

/// Items 1 and 4: the preflight is read-only. The schema meets a class a field declares that is only reserved, a
/// subclass in another package, and refuses it without loading; the setup loads it with its parent, and the schema
/// then goes on to its next refusal.
mod reserved {
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use ow_attr::AttributeSystem;
    use ow_core::ParityMode;
    use ow_defs::DefStore;
    use ow_defs::identity::IdentityPolicy;
    use ow_object::heap::ObjectSpace;
    use ow_package::testkit::reflect::{toy_array_of, toy_struct};
    use ow_package::testkit::script::ScriptToy;
    use ow_vm::api::Vm;
    use ow_vm::error::VmError;

    use super::super::{card_schema, setup};

    static NEXT: AtomicU32 = AtomicU32::new(0);
    const IMPORTS: [&str; 4] = ["Class", "ScriptStruct", "ObjectProperty", "StructProperty"];
    const NAME_IMPORTS: [&str; 3] = ["Class", "ArrayProperty", "ObjectProperty"];

    fn engine() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&IMPORTS);
        t.class("Object", 0);
        t.build()
    }

    /// `GearboxFramework`: `GearboxGlobalsDefinition`, the globals holder's declared type (a card root).
    fn gearbox() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&IMPORTS);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let object = t.p.add_import("Core", "Class", engine, "Object");
        t.class("GearboxGlobalsDefinition", object);
        t.build()
    }

    /// `WillowGame`: the card's root classes; the weapon's `DefinitionData.BalanceDefinition` names `ToyPkg.ToyBalance`.
    fn willow_game() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&IMPORTS);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let object = t.p.add_import("Core", "Class", engine, "Object");
        let toy = t.p.add_import("Core", "Package", 0, "ToyPkg");
        let balance = t.p.add_import("Core", "Class", toy, "ToyBalance");
        for c in [
            "WeaponTypeDefinition",
            "WeaponPartDefinition",
            "WeaponNamePartDefinition",
            "AttributePresentationDefinition",
            "WillowGlobals",
        ] {
            t.class(c, object);
        }
        let gbx = t.class("GlobalsDefinition", object);
        let _ = gbx;
        let presentation = t.class("ToyPresentationBase", object);
        t.class("WeaponStatusEffectAttributePresentationDefinition", presentation);
        let weapon = t.class("WillowWeapon", object);
        let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
        t.prop("Object", data, "BalanceDefinition", 0, balance);
        t.prop("Struct", weapon, "DefinitionData", 0, data);
        t.build()
    }

    /// `ToyPkg`: `ToyBalance`, a subclass of `InventoryBalanceDefinition`, whose `DlcItemSet` names a DLC set that
    /// declares none of the presentation lists.
    fn toy_pkg() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&IMPORTS);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let object = t.p.add_import("Core", "Class", engine, "Object");
        let dlc = t.class("DownloadableItemSetDefinition", object);
        let base = t.class("InventoryBalanceDefinition", object);
        t.prop("Object", base, "DlcItemSet", 0, dlc);
        t.class("ToyBalance", base);
        t.build()
    }

    #[test]
    fn the_schema_refuses_a_reserved_class_and_the_setup_loads_it() {
        let dir = std::env::temp_dir().join(format!(
            "ow-inventory-card-reserved-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let (e, g, w, k) = (engine(), gearbox(), willow_game(), toy_pkg());
        let defs = DefStore::from_synthetic(
            &dir,
            &[
                ("Engine.upk", e.as_slice()),
                ("GearboxFramework.upk", g.as_slice()),
                ("WillowGame.upk", w.as_slice()),
                ("ToyPkg.upk", k.as_slice()),
            ],
            IdentityPolicy::FirstInScanOrder,
            ParityMode::Off,
        )
        .unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        // Before the setup nothing is loaded: the schema finds no root class and loads none.
        assert!(card_schema(&vm).is_err());
        assert!(vm.space.classes.by_path("WillowGame.WillowWeapon").is_none());
        // The roots alone (as a test of the read-only walk): the balance class is reserved, refused, not loaded.
        vm.find_class("WillowGame.WillowWeapon").unwrap();
        for c in ["WeaponTypeDefinition", "WeaponPartDefinition", "GlobalsDefinition", "AttributePresentationDefinition"] {
            vm.find_class(&format!("WillowGame.{c}")).unwrap();
        }
        vm.find_class("WillowGame.WeaponStatusEffectAttributePresentationDefinition").unwrap();
        let reserved = vm.space.classes.by_path("ToyPkg.ToyBalance").expect("the field reserves its class");
        assert!(!vm.space.classes.get_layout(reserved).unwrap().is_defined());
        let m = message(card_schema(&vm).unwrap_err());
        assert!(m.contains("ToyPkg.ToyBalance is not loaded"), "{m}");
        assert!(!vm.space.classes.get_layout(reserved).unwrap().is_defined(), "the schema loaded nothing");
        // The setup loads it with its parent; the schema then refuses the next thing, the DLC set's missing list.
        setup(&mut vm).unwrap();
        let layout = vm.space.classes.get_layout(reserved).unwrap();
        assert!(layout.is_defined() && layout.super_class().is_some(), "loaded with its ancestry");
        let m = message(card_schema(&vm).unwrap_err());
        assert!(m.contains("declares no WeaponAttributePresentationOverride"), "{m}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `WillowGame` for the name-part test: the card's roots; the type's `PrefixList` holds `ToyPkg.ToyNamePart`.
    fn willow_names() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&NAME_IMPORTS);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let object = t.p.add_import("Core", "Class", engine, "Object");
        let toy = t.p.add_import("Core", "Package", 0, "ToyPkg");
        let toy_name_part = t.p.add_import("Core", "Class", toy, "ToyNamePart");
        for c in [
            "WillowWeapon",
            "WeaponPartDefinition",
            "WeaponNamePartDefinition",
            "GlobalsDefinition",
            "AttributePresentationDefinition",
            "WeaponStatusEffectAttributePresentationDefinition",
            "WillowGlobals",
        ] {
            t.class(c, object);
        }
        let ty = t.class("WeaponTypeDefinition", object);
        toy_array_of(&mut t, ty, "PrefixList", "Object", toy_name_part);
        t.build()
    }

    /// `ToyPkg` for the name-part test: `ToyNamePart`, a subclass of `WillowGame.WeaponNamePartDefinition`.
    fn toy_names() -> Vec<u8> {
        let mut t = ScriptToy::with_imports(&NAME_IMPORTS);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let name_part = t.p.add_import("Core", "Class", willow, "WeaponNamePartDefinition");
        t.class("ToyNamePart", name_part);
        t.build()
    }

    /// Review r2 of #521, item 4: a `PrefixList` entry class that is a reserved subclass of the name part fails
    /// `is_child_of` until its ancestry is loaded; the setup resolves it, so the name-part test holds.
    #[test]
    fn the_setup_resolves_a_name_part_subclass_with_its_ancestry() {
        let dir = std::env::temp_dir().join(format!(
            "ow-inventory-card-names-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let (e, g, w, k) = (engine(), gearbox(), willow_names(), toy_names());
        let defs = DefStore::from_synthetic(
            &dir,
            &[
                ("Engine.upk", e.as_slice()),
                ("GearboxFramework.upk", g.as_slice()),
                ("WillowGame.upk", w.as_slice()),
                ("ToyPkg.upk", k.as_slice()),
            ],
            IdentityPolicy::FirstInScanOrder,
            ParityMode::Off,
        )
        .unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        let name_part = vm.find_class("WillowGame.WeaponNamePartDefinition").unwrap();
        vm.find_class("WillowGame.WeaponTypeDefinition").unwrap();
        let toy = vm.space.classes.by_path("ToyPkg.ToyNamePart").expect("the list reserves its class");
        assert!(!vm.space.classes.get_layout(toy).unwrap().is_defined());
        assert!(!vm.space.classes.is_child_of(toy, name_part), "a reserved class has no parent yet");
        setup(&mut vm).unwrap();
        assert!(vm.space.classes.get_layout(toy).unwrap().is_defined());
        assert!(vm.space.classes.is_child_of(toy, name_part), "the setup loaded its ancestry");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn message(e: VmError) -> String {
        match e {
            VmError::Script { message, .. } => message,
            other => panic!("expected VmError::Script, got {other:?}"),
        }
    }
}
