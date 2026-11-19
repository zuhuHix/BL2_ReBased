//! The plan never loads a definition (review r3 of #468; hub ruling: a lazy definition load is a write). On an invented
//! `WillowGame.upk` in a synthetic store (AGENTS.md rule 9), loading `ToyHolder` makes its two references, the
//! definition `ToyDef` and the part `ToyPart`, store handles that are not loaded. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::testkit::reflect::{property_payload, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::initialize;
use crate::equip::natives::WEAPON_ALL_PARTS;

const DIM: u32 = 4;
const ROLES: [&str; 4] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority"];
static NEXT: AtomicU32 = AtomicU32::new(0);

fn enum_export(t: &mut ScriptToy, outer: i32, name: &str, values: &[&str]) -> i32 {
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend_from_slice(&t.none);
    payload.extend(0i32.to_le_bytes());
    payload.extend((values.len() as i32).to_le_bytes());
    for v in values {
        payload.extend(t.p.fname(v, 0));
    }
    let mut e = Export::new(t.import("Enum"), name, payload);
    e.outer = outer;
    t.p.add_export_with(e)
}

fn array_of_struct(t: &mut ScriptToy, owner: i32, name: &str, ty: i32) {
    let inner = t.p.next_export();
    t.prop("Array", owner, name, 0, inner + 1);
    t.prop("Struct", inner, name, 0, ty);
}

fn static_prop(t: &mut ScriptToy, kind: &str, owner: i32, name: &str, ty: i32) {
    let payload = property_payload(&t.none, DIM, 0, &ints(&[ty]));
    let mut e = Export::new(t.import(&format!("{kind}Property")), name, payload);
    e.outer = owner;
    t.p.add_export_with(e);
}

/// The slot fields the effect entry and the slot record share (section 3 step 3).
fn shared(t: &mut ScriptToy, owner: i32, object: i32, aid: i32) {
    for n in ["SlotName", "TargetInstanceDataName"] {
        t.prop("Name", owner, n, 0, 0);
    }
    for n in ["bExternalSlot", "bRunEffectsAsSkill", "bIncludeAlliesAsTarget", "bEnforceMinimumGrade", "bEnforceMaximumGrade"] {
        t.prop("Bool", owner, n, 0, 0);
    }
    for n in ["MinimumGrade", "MaximumGrade"] {
        t.prop("Int", owner, n, 0, 0);
    }
    for n in ["AttributeToModify", "ConstraintAttribute"] {
        t.prop("Object", owner, n, 0, object);
    }
    t.prop("Byte", owner, "ModifierType", 0, 0);
    for n in ["BaseModifierValue", "PerGradeUpgrade"] {
        t.prop("Struct", owner, n, 0, aid);
    }
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "ScriptStruct",
        "Enum",
        "BoolProperty",
        "ByteProperty",
        "IntProperty",
        "FloatProperty",
        "NameProperty",
        "ObjectProperty",
        "StructProperty",
        "ArrayProperty",
    ]);
    let object = t.class("Object", 0);
    let def = t.class("Def", object);
    let aid = toy_struct(&mut t, def, "AttributeInitializationData", None);
    for (k, n) in [("Float", "BaseValueConstant"), ("Object", "BaseValueAttribute"), ("Object", "InitializationDefinition"), ("Float", "BaseValueScaleConstant")] {
        t.prop(k, aid, n, 0, if k == "Object" { object } else { 0 });
    }
    let up = toy_struct(&mut t, def, "AttributeSlotUpgradeData", None);
    t.prop("Name", up, "SlotName", 0, 0);
    t.prop("Int", up, "GradeIncrease", 0, 0);
    t.prop("Bool", up, "bActivateSlot", 0, 0);
    let eff = toy_struct(&mut t, def, "AttributeSlotEffectData", None);
    shared(&mut t, eff, object, aid);
    t.prop("Bool", eff, "bIncludeInFunStats", 0, 0);
    array_of_struct(&mut t, def, "AttributeSlotUpgrades", up);
    array_of_struct(&mut t, def, "AttributeSlotEffects", eff);
    t.prop("Int", def, "AttributeSlotMaxActivated", 0, 0);
    t.prop("Struct", def, "AttributeSlotBaseGrade", 0, aid);
    let part = t.class("Part", object);
    array_of_struct(&mut t, part, "AttributeSlotUpgrades", up);
    array_of_struct(&mut t, part, "AttributeSlotEffects", eff);
    let inventory = t.class("WillowInventory", object);
    let role = enum_export(&mut t, inventory, "ENetRole", &ROLES);
    t.prop("Byte", inventory, "Role", 0, role);
    t.prop("Bool", inventory, "bNetDirty", 0, 0);
    let slot = toy_struct(&mut t, inventory, "AttributeSlotData", None);
    shared(&mut t, slot, object, aid);
    for n in ["bIncludeInModifierText", "bActivated"] {
        t.prop("Bool", slot, n, 0, 0);
    }
    t.prop("Int", slot, "EffectGrade", 0, 0);
    t.prop("Float", slot, "ComputedModifierValue", 0, 0);
    static_prop(&mut t, "Struct", inventory, "AttributeSlots", slot);
    static_prop(&mut t, "Float", inventory, "ReplicatedAttributeSlotModifierValues", 0);
    let weapon = t.class("WillowWeapon", inventory);
    let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
    t.prop("Object", data, "WeaponTypeDefinition", 0, object);
    for f in WEAPON_ALL_PARTS {
        t.prop("Object", data, f, 0, object);
    }
    t.prop("Struct", weapon, "DefinitionData", 0, data);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "TargetDef", 0, object);
    t.prop("Object", holder, "TargetPart", 0, object);
    let mut d = 0i32.to_le_bytes().to_vec();
    d.extend_from_slice(&t.none);
    let toy_def = t.p.add_export(def, "ToyDef", d.clone());
    let toy_part = t.p.add_export(part, "ToyPart", d);
    let mut h = 0i32.to_le_bytes().to_vec();
    h.extend(t.p.object_tag("TargetDef", toy_def));
    h.extend(t.p.object_tag("TargetPart", toy_part));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder", h);
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    def: ObjRef,
    part: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-slots-store-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let path = ObjectPath::parse("WillowGame.ToyHolder").unwrap();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let get = |vm: &Vm, n: &str| match ow_inventory_read(vm, holder, n) {
        Value::Object(Some(o)) => o,
        other => panic!("{other:?}"),
    };
    let (def, part) = (get(&vm, "TargetDef"), get(&vm, "TargetPart"));
    assert!(vm.defs.is_unloaded(&vm.space, def) && vm.defs.is_unloaded(&vm.space, part), "both are unloaded handles");
    Fx { vm, dir, def, part }
}

fn ow_inventory_read(vm: &Vm, o: ObjRef, name: &str) -> Value {
    crate::equip::read_prop(vm, o, name).unwrap()
}

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn ty(&self, o: ObjRef, name: &str) -> StructId {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        match &self.vm.space.classes.decl(p).unwrap().kind {
            PropKind::Struct(t) => *t,
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(t) => t,
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    fn st(&mut self, ty: StructId, fields: Vec<(&str, Value)>) -> Value {
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.vm.space.names.intern(n);
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = x;
        }
        v
    }

    fn name(&mut self, n: &str) -> Value {
        Value::Name(self.vm.space.names.intern(n))
    }

    fn init(&mut self, holder: ObjRef, field: &str, c: f32, attribute: Option<ObjRef>) -> Value {
        let ty = self.ty(holder, field);
        self.st(
            ty,
            vec![
                ("BaseValueConstant", Value::Float(c)),
                ("BaseValueScaleConstant", Value::Float(1.0)),
                ("BaseValueAttribute", Value::Object(attribute)),
            ],
        )
    }

    /// The definition loaded (a test's own setup step), with an effect and an upgrade for slot `A`.
    fn load_def(&mut self, base_grade_attribute: Option<ObjRef>, upgrade: bool) {
        let def = self.vm.defs.load(&mut self.vm.space, self.def).unwrap();
        let ety = self.ty(def, "AttributeSlotEffects");
        let a = self.name("A");
        let (b, p) = (self.init(def, "AttributeSlotBaseGrade", 1.0, None), self.init(def, "AttributeSlotBaseGrade", 1.0, None));
        let effect = self.st(ety, vec![("SlotName", a.clone()), ("BaseModifierValue", b), ("PerGradeUpgrade", p)]);
        self.set(def, "AttributeSlotEffects", Value::array(vec![effect]));
        if upgrade {
            let uty = self.ty(def, "AttributeSlotUpgrades");
            let u = self.st(uty, vec![("SlotName", a), ("GradeIncrease", Value::Int(1)), ("bActivateSlot", Value::Bool(true))]);
            self.set(def, "AttributeSlotUpgrades", Value::array(vec![u]));
        }
        self.set(def, "AttributeSlotMaxActivated", Value::Int(19));
        let g = self.init(def, "AttributeSlotBaseGrade", 1.0, base_grade_attribute);
        self.set(def, "AttributeSlotBaseGrade", g);
    }

    fn make(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    /// A weapon on `definition` with `parts`, not the authority.
    fn weapon(&mut self, definition: ObjRef, parts: &[(&str, ObjRef)]) -> ObjRef {
        let w = self.make("WillowGame.WillowWeapon");
        let ty = self.ty(w, "DefinitionData");
        let mut fields = vec![("WeaponTypeDefinition", Value::Object(Some(definition)))];
        fields.extend(parts.iter().map(|(f, p)| (*f, Value::Object(Some(*p)))));
        let data = self.st(ty, fields);
        self.set(w, "DefinitionData", data);
        self.set(w, "Role", Value::Byte(1));
        w
    }

    fn slots(&self, w: ObjRef) -> Vec<Value> {
        let class = self.vm.space.heap.get(w).unwrap().class;
        let p = self.vm.space.prop(class, "AttributeSlots").unwrap();
        (0..DIM).map(|i| self.vm.space.read(w, PropAddr::element(p, i)).unwrap().clone()).collect()
    }

    fn handle_state(&self, o: ObjRef) -> (bool, usize) {
        (self.vm.defs.is_unloaded(&self.vm.space, o), self.vm.space.heap.get(o).unwrap().slots.len())
    }
}

fn not_yet(e: &VmError) -> &ow_core::NotYet {
    match e {
        VmError::NotYet(n) => n,
        other => panic!("not a NotYet: {other:?}"),
    }
}

/// An unloaded definition: a named `NotYet` before any write, and the store, the handle's heap object and the item are
/// all left as they were.
#[test]
fn an_unloaded_definition_is_refused_and_not_loaded() {
    let mut x = fx();
    let def = x.def;
    let w = x.weapon(def, &[]);
    let (before_handle, before_slots) = (x.handle_state(def), x.slots(w));
    let e = initialize(&mut x.vm, w, false, false).unwrap_err();
    let n = not_yet(&e);
    assert_eq!(n.missing, Missing::NotPorted);
    assert!(n.what.contains("is not loaded") && !n.what.contains("partial writes"), "{n}");
    assert_eq!(x.handle_state(def), before_handle, "still an unloaded handle, its heap object unchanged");
    assert!(before_handle.0);
    assert_eq!(x.slots(w), before_slots, "the item unchanged");
}

/// The base pass visits its holders in order: the definition's base-grade refusal comes first, and a later regular
/// part that is not loaded is neither reached nor loaded.
#[test]
fn a_base_pass_refusal_comes_before_a_later_unloaded_part() {
    let mut x = fx();
    let attribute = x.make("WillowGame.Holder");
    x.load_def(Some(attribute), true);
    let (def, part) = (x.def, x.part);
    let w = x.weapon(def, &[("BodyPartDefinition", part)]);
    let e = initialize(&mut x.vm, w, false, false).unwrap_err();
    assert!(not_yet(&e).what.starts_with("IAS-BASE-GRADE-CONTEXT: "), "{e:?}");
    assert!(x.handle_state(part).0, "the part was not loaded");
}

/// The name-part pass consults the regular parts only after the definition misses: a name-part upgrade the definition
/// covers succeeds, and an unloaded regular part stays unloaded.
#[test]
fn a_name_part_upgrade_the_definition_covers_does_not_reach_an_unloaded_part() {
    let mut x = fx();
    x.load_def(None, false);
    let prefix = x.make("WillowGame.Part");
    let a = x.name("A");
    let uty = x.ty(prefix, "AttributeSlotUpgrades");
    let u = x.st(uty, vec![("SlotName", a.clone()), ("GradeIncrease", Value::Int(2)), ("bActivateSlot", Value::Bool(true))]);
    x.set(prefix, "AttributeSlotUpgrades", Value::array(vec![u]));
    let (def, part) = (x.def, x.part);
    let w = x.weapon(def, &[("BodyPartDefinition", part), ("PrefixPartDefinition", prefix)]);
    initialize(&mut x.vm, w, false, true).unwrap();
    let s0 = x.slots(w)[0].clone();
    let f = |n: &str| ow_attr::eval::field(&x.vm.space, &s0, n).unwrap().clone();
    assert_eq!((f("SlotName"), f("bActivated"), f("EffectGrade")), (a, Value::Bool(true), Value::Int(3)), "1 + 2");
    assert!(x.handle_state(part).0, "the regular part was never needed, so never loaded");
}
