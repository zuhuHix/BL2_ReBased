//! NATIVE_SPAWN_SETBASEVALUE.md section 7, tests 1 to 6, on an invented package (AGENTS.md rule 9; every value is
//! invented, the class and field names are the packages'). The attributes, their resolvers and the initialization
//! definition are exports a holder references, so a load of the holder leaves them unloaded store handles; the AI
//! classes and the archetype are made at run time. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const KINDS: [&str; 7] = ["Object", "Float", "Struct", "Array", "Int", "Byte", "Bool"];
/// The holder's fields, each naming one export.
const HELD: [&str; 12] = [
    "Attr", "BaseAttr", "Def", "ParentAttr", "OwnAttr", "Broken", "NestAttr", "OperandAttr", "IfTrueAttr", "DefaultAttr",
    "AfterAttr", "ParentClass",
];

/// The invented `Engine.upk`.
fn engine(categories: bool) -> Vec<u8> {
    let mut t = ScriptToy::new(&KINDS);
    let object = t.class("Object", 0);
    let attribute = t.class("AttributeDefinition", object);
    toy_array_of(&mut t, attribute, "ContextResolverChain", "Object", object);
    toy_array_of(&mut t, attribute, "ValueResolverChain", "Object", object);
    let resolver = t.class("ToyResolver", object);
    let definition = t.class("AttributeInitializationDefinition", object);
    let init = toy_struct(&mut t, definition, "AttributeInitializationData", None);
    t.prop("Float", init, "BaseValueConstant", 0, 0);
    t.prop("Object", init, "BaseValueAttribute", 0, attribute);
    t.prop("Object", init, "InitializationDefinition", 0, definition);
    t.prop("Float", init, "BaseValueScaleConstant", 0, 0);
    // The definition's four parts, as the evaluator reads them (the hub's point (b) on #811).
    let formula = toy_struct(&mut t, definition, "AttributeInitializationFormula", None);
    t.prop("Bool", formula, "bEnabled", 0, 0);
    for f in ["Multiplier", "Level", "Power", "Offset"] {
        t.prop("Struct", formula, f, 0, init);
    }
    let expr = toy_struct(&mut t, definition, "AttributeExpressionData", None);
    t.prop("Object", expr, "AttributeOperand1", 0, attribute);
    t.prop("Object", expr, "AttributeOperand2", 0, attribute);
    let cond_entry = toy_struct(&mut t, definition, "ConditionalExpression", None);
    toy_array_of(&mut t, cond_entry, "Expressions", "Struct", expr);
    t.prop("Struct", cond_entry, "BaseValueIfTrue", 0, init);
    let conditional = toy_struct(&mut t, definition, "ConditionalInitialization", None);
    t.prop("Bool", conditional, "bEnabled", 0, 0);
    toy_array_of(&mut t, conditional, "ConditionalExpressionList", "Struct", cond_entry);
    t.prop("Struct", conditional, "DefaultBaseValue", 0, init);
    let variance = toy_struct(&mut t, definition, "RandomVariance", None);
    t.prop("Bool", variance, "bEnabled", 0, 0);
    let range = toy_struct(&mut t, definition, "RangeRestriction", None);
    for f in ["bEnableMinValueRestriction", "bEnableMaxValueRestriction"] {
        t.prop("Bool", range, f, 0, 0);
    }
    t.prop("Struct", definition, "ValueFormula", 0, formula);
    t.prop("Struct", definition, "ConditionalInitialization", 0, conditional);
    t.prop("Struct", definition, "RandomVariance", 0, variance);
    t.prop("Struct", definition, "RangeRestriction", 0, range);
    let class = t.class("AIClassDefinition", object);
    let entry = toy_struct(&mut t, class, "AttributeStartingValue", None);
    t.prop("Object", entry, "Attribute", 0, attribute);
    t.prop("Struct", entry, "BaseValue", 0, init);
    toy_array_of(&mut t, class, "AttributeStartingValues", "Struct", entry);
    t.prop("Object", class, "AttributeParentClass", 0, class);
    if categories {
        toy_array_of(&mut t, class, "AttributeStartingValuesNew", "Struct", entry);
    }
    let archetype = t.class("ToyPawn", object);
    t.prop("Object", archetype, "AIClass", 0, class);
    toy_array_of(&mut t, archetype, "AttributeStartingValues", "Struct", entry);
    // A pawn class whose AttributeStartingValues is an int (test 3).
    let mistyped = t.class("ToyMistypedClass", object);
    t.prop("Int", mistyped, "AttributeStartingValues", 0, 0);
    t.prop("Object", mistyped, "AttributeParentClass", 0, class);
    let holder = t.class("ToyHolder", object);
    for f in HELD {
        t.prop("Object", holder, f, 0, object);
    }
    // The exports. Attr: one context resolver, value resolvers V1, V2, None, V3 (subobjects of Attr).
    let none = t.none;
    let empty = |none: &[u8]| {
        let mut p = ints(&[0]);
        p.extend_from_slice(none);
        p
    };
    let refs = |r: &[i32]| r.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let a = t.p.next_export();
    let (c1, v1, v2, v3) = (a + 1, a + 2, a + 3, a + 4);
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag("ContextResolverChain", 1, &refs(&[c1])));
    p.extend(t.p.array_tag("ValueResolverChain", 4, &refs(&[v1, v2, 0, v3])));
    p.extend_from_slice(&none);
    assert_eq!(t.p.add_export(attribute, "Attr_Toy", p), a);
    for (i, name) in ["Context_0", "Value_0", "Value_1", "Value_2"].into_iter().enumerate() {
        let mut e = Export::new(resolver, name, empty(&none));
        e.outer = a;
        assert_eq!(t.p.add_export_with(e), a + 1 + i as i32);
    }
    let base_attr = t.p.add_export(attribute, "Attr_Base", empty(&none));
    let def = t.p.add_export(definition, "Def_Toy", empty(&none));
    let parent_attr = t.p.add_export(attribute, "Attr_Parent", empty(&none));
    let own_attr = t.p.add_export(attribute, "Attr_Own", empty(&none));
    // Cut short: its load fails.
    let broken = t.p.add_export(attribute, "Attr_Broken", ints(&[0]));
    let mut nested: Vec<i32> = ["Attr_Nest", "Attr_Operand", "Attr_IfTrue", "Attr_Default", "Attr_After"]
        .into_iter()
        .map(|n| t.p.add_export(attribute, n, empty(&none)))
        .collect();
    // An exported AI class (empty lists): a parent the child names as an unloaded handle (the hub's point (a)).
    nested.push(t.p.add_export(class, "AIClass_Parent", empty(&none)));
    let mut h = ints(&[0]);
    let targets = [a, base_attr, def, parent_attr, own_attr, broken].into_iter().chain(nested);
    for (f, r) in HELD.into_iter().zip(targets) {
        h.extend(t.p.object_tag(f, r));
    }
    h.extend_from_slice(&none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    held: Vec<ObjRef>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_with(mode: ParityMode, categories: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-sv-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let e = engine(categories);
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice())], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    let holder = vm.defs.find(&mut vm.space, &ObjectPath::parse("Engine.ToyHolderObject").unwrap()).unwrap().unwrap();
    let class = vm.space.heap.get(holder).unwrap().class;
    let held = HELD
        .iter()
        .map(|f| {
            let p = vm.space.prop(class, f).unwrap();
            match vm.space.read(holder, PropAddr::scalar(p)).unwrap() {
                Value::Object(Some(o)) => *o,
                other => panic!("{other:?}"),
            }
        })
        .collect();
    Fx { vm, dir, held }
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, false)
}

impl Fx {
    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
    /// A starting-value entry for `attribute`, its `BaseValue` naming `base` and `definition`.
    fn entry(&self, attribute: ObjRef, base: Option<ObjRef>, definition: Option<ObjRef>) -> Value {
        let vm = &self.vm;
        let ty = vm.space.classes.struct_by_path("Engine.AIClassDefinition.AttributeStartingValue").unwrap();
        let mut v = vm.space.classes.default_value(&PropKind::Struct(ty)).unwrap();
        let set = |v: &mut Value, name: &str, x: Value| {
            let s = v.struct_mut().unwrap();
            let (i, _) = vm.space.classes.struct_layout(s.ty).unwrap().field(vm.space.names.find(name).unwrap()).unwrap();
            s.fields[i as usize] = x;
        };
        set(&mut v, "Attribute", Value::Object(Some(attribute)));
        let mut data = ow_attr::eval::field(&vm.space, &v, "BaseValue").unwrap().clone();
        set(&mut data, "BaseValueAttribute", Value::Object(base));
        set(&mut data, "InitializationDefinition", Value::Object(definition));
        set(&mut v, "BaseValue", data);
        v
    }
    fn unloaded(&self, o: ObjRef) -> bool {
        self.vm.defs.is_unloaded(&self.vm.space, o)
    }
    fn lines(&self) -> Vec<String> {
        self.vm.log.iter().filter(|l| l.starts_with("harness input: preload") && !l.ends_with(": loaded")).cloned().collect()
    }
    /// The archetype with an AI class (Attr, BaseAttr and Def) whose parent class has ParentAttr.
    fn chain(&mut self) -> ObjRef {
        let [attr, base, def, parent_attr, ..] = self.held[..] else { unreachable!() };
        let parent = self.make("Engine.AIClassDefinition");
        let e = self.entry(parent_attr, None, None);
        self.set(parent, "AttributeStartingValues", Value::array(vec![e]));
        let class = self.make("Engine.AIClassDefinition");
        let e = self.entry(attr, Some(base), Some(def));
        self.set(class, "AttributeStartingValues", Value::array(vec![e]));
        self.set(class, "AttributeParentClass", Value::Object(Some(parent)));
        let archetype = self.make("Engine.ToyPawn");
        self.set(archetype, "AIClass", Value::Object(Some(class)));
        archetype
    }
}

// Test 1: the walk.
#[test]
fn the_walk_loads_the_chain_s_definitions_parents_first() {
    let mut x = fx();
    let archetype = x.chain();
    let [attr, base, def, parent_attr, ..] = x.held[..] else { unreachable!() };
    let loaded = preload_from(&mut x.vm, archetype).unwrap();
    for o in [attr, base, def, parent_attr] {
        assert!(!x.unloaded(o), "{o:?} loaded");
    }
    // Attr's resolvers: the context resolver and the two value resolvers before the None, not the third.
    let chains = |name: &str| -> Vec<Option<ObjRef>> {
        let class = x.vm.space.heap.get(attr).unwrap().class;
        let p = x.vm.space.prop(class, name).unwrap();
        match x.vm.space.read(attr, PropAddr::scalar(p)).unwrap() {
            Value::Array(a) => a.iter().map(|v| if let Value::Object(o) = v { *o } else { None }).collect(),
            other => panic!("{other:?}"),
        }
    };
    let context = chains("ContextResolverChain");
    let value = chains("ValueResolverChain");
    assert!(!x.unloaded(context[0].unwrap()));
    assert!(!x.unloaded(value[0].unwrap()) && !x.unloaded(value[1].unwrap()));
    assert_eq!(value[2], None);
    assert!(x.unloaded(value[3].unwrap()), "the value resolver after the None stays unloaded");
    assert_eq!(loaded, 7);
    let lines = x.lines();
    assert_eq!(lines.len(), 7, "{lines:?}");
    assert!(lines[0].contains("Attr_Parent"), "parents first: {lines:?}");
    assert!(lines[1].contains("Attr_Toy"), "{lines:?}");
    assert!(lines.iter().all(|l| l.ends_with("(a SetBaseValue definition the game keeps resident; UNVERIFIED)")));
    // A second run loads and logs nothing.
    let logged = x.vm.log.len();
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 0);
    assert_eq!(x.vm.log.len(), logged);
}

// Test 2: the archetype's own list.
#[test]
fn the_archetype_s_own_list_is_preloaded() {
    let mut x = fx();
    let own = x.held[4];
    let archetype = x.make("Engine.ToyPawn");
    let e = x.entry(own, None, None);
    x.set(archetype, "AttributeStartingValues", Value::array(vec![e]));
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 1);
    assert!(!x.unloaded(own));
}

// Test 3: declarations.
#[test]
fn a_mistyped_list_is_a_script_error_with_nothing_loaded() {
    let mut x = fx();
    let class = x.make("Engine.ToyMistypedClass");
    let archetype = x.make("Engine.ToyPawn");
    // The AIClass field takes an AIClassDefinition: the mistyped class is written past the heap's check.
    let c = x.vm.space.heap.get(archetype).unwrap().class;
    let p = x.vm.space.prop(c, "AIClass").unwrap();
    *x.vm.space.slot_mut(archetype, PropAddr::scalar(p)).unwrap() = Value::Object(Some(class));
    let before = x.vm.defs.load_transitions();
    assert!(matches!(preload_from(&mut x.vm, archetype), Err(VmError::Script { .. })));
    assert_eq!(x.vm.defs.load_transitions(), before, "nothing loaded");
    // A pawn that declares no AIClass at all.
    let plain = x.make("Engine.AttributeDefinition");
    assert!(matches!(preload_from(&mut x.vm, plain), Err(VmError::Script { .. })));
}

// Test 4: the categories.
#[test]
fn a_non_empty_category_list_is_not_read_before_any_definition_loads() {
    let mut x = fx_with(ParityMode::Off, true);
    let archetype = x.chain();
    let class = {
        let c = x.vm.space.heap.get(archetype).unwrap().class;
        let p = x.vm.space.prop(c, "AIClass").unwrap();
        match x.vm.space.read(archetype, PropAddr::scalar(p)).unwrap() {
            Value::Object(Some(o)) => *o,
            other => panic!("{other:?}"),
        }
    };
    // Declared but empty: no refusal (a fixture of its own, so the run above loads nothing here).
    let mut y = fx_with(ParityMode::Off, true);
    let other = y.chain();
    assert_eq!(preload_from(&mut y.vm, other).unwrap(), 7, "an empty category list refuses nothing");
    let e = x.entry(x.held[0], None, None);
    x.set(class, "AttributeStartingValuesNew", Value::array(vec![e]));
    let before = x.vm.defs.load_transitions();
    match preload_from(&mut x.vm, archetype) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, CATEGORIES)),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.vm.defs.load_transitions(), before, "no definition loaded");
    assert!(x.unloaded(x.held[0]));
}

// Test 5: a failed load is logged and left.
#[test]
fn a_failed_load_is_logged_and_the_walk_goes_on() {
    let mut x = fx();
    let (own, broken) = (x.held[4], x.held[5]);
    let archetype = x.make("Engine.ToyPawn");
    let entries = vec![x.entry(broken, None, None), x.entry(own, None, None)];
    x.set(archetype, "AttributeStartingValues", Value::array(entries));
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 1, "the good one loads");
    assert!(x.vm.log.iter().any(|l| l.contains("Attr_Broken: failed")), "{:?}", x.vm.log);
    assert!(!x.unloaded(own));
}

// Test 6: Reference does nothing.
#[test]
fn reference_does_nothing() {
    let mut x = fx_with(ParityMode::Reference, false);
    let archetype = x.chain();
    let logged = x.vm.log.len();
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 0);
    assert_eq!(x.vm.log.len(), logged);
    assert!(x.unloaded(x.held[0]));
}

/// `v` with the field at `path` (names separated by dots, through nested structs) set to `x`.
fn put(vm: &Vm, v: &mut Value, path: &str, x: Value) {
    let (head, rest) = match path.split_once('.') {
        Some((h, r)) => (h, Some(r)),
        None => (path, None),
    };
    let s = v.struct_mut().unwrap();
    let (i, _) = vm.space.classes.struct_layout(s.ty).unwrap().field(vm.space.names.find(head).unwrap()).unwrap();
    match rest {
        Some(r) => put(vm, &mut s.fields[i as usize], r, x),
        None => s.fields[i as usize] = x,
    }
}

/// The hub's point (b) on #811: the walk follows a definition into the nested data the preflight walks. A formula
/// term's attribute and its definition; that definition's conditional: an expression operand and the entry's
/// `BaseValueIfTrue`, then an entry with no expressions (always holds), so the entry after it and `DefaultBaseValue`
/// are not reached and stay unloaded.
#[test]
fn the_walk_follows_nested_formulas_and_conditionals() {
    let mut x = fx();
    let h = x.held.clone();
    let (nest, operand, if_true, default, after) = (h[6], h[7], h[8], h[9], h[10]);
    let inner = x.make("Engine.AttributeInitializationDefinition");
    let outer = x.make("Engine.AttributeInitializationDefinition");
    // The outer definition's formula: Multiplier names `nest` and the inner definition.
    let mut formula = {
        let c = x.vm.space.heap.get(outer).unwrap().class;
        let p = x.vm.space.prop(c, "ValueFormula").unwrap();
        x.vm.space.read(outer, PropAddr::scalar(p)).unwrap().clone()
    };
    put(&x.vm, &mut formula, "bEnabled", Value::Bool(true));
    put(&x.vm, &mut formula, "Multiplier.BaseValueAttribute", Value::Object(Some(nest)));
    put(&x.vm, &mut formula, "Multiplier.InitializationDefinition", Value::Object(Some(inner)));
    x.set(outer, "ValueFormula", formula);
    // The inner definition's conditional.
    let mut conditional = {
        let c = x.vm.space.heap.get(inner).unwrap().class;
        let p = x.vm.space.prop(c, "ConditionalInitialization").unwrap();
        x.vm.space.read(inner, PropAddr::scalar(p)).unwrap().clone()
    };
    let entry_ty = x.vm.space.classes.struct_by_path("Engine.AttributeInitializationDefinition.ConditionalExpression").unwrap();
    let expr_ty = x.vm.space.classes.struct_by_path("Engine.AttributeInitializationDefinition.AttributeExpressionData").unwrap();
    let mut expr = x.vm.space.classes.default_value(&PropKind::Struct(expr_ty)).unwrap();
    put(&x.vm, &mut expr, "AttributeOperand1", Value::Object(Some(operand)));
    let blank = x.vm.space.classes.default_value(&PropKind::Struct(entry_ty)).unwrap();
    let mut first = blank.clone();
    put(&x.vm, &mut first, "Expressions", Value::array(vec![expr]));
    put(&x.vm, &mut first, "BaseValueIfTrue.BaseValueAttribute", Value::Object(Some(if_true)));
    let always = blank.clone();
    let mut later = blank;
    put(&x.vm, &mut later, "BaseValueIfTrue.BaseValueAttribute", Value::Object(Some(after)));
    put(&x.vm, &mut conditional, "bEnabled", Value::Bool(true));
    put(&x.vm, &mut conditional, "ConditionalExpressionList", Value::array(vec![first, always, later]));
    put(&x.vm, &mut conditional, "DefaultBaseValue.BaseValueAttribute", Value::Object(Some(default)));
    x.set(inner, "ConditionalInitialization", conditional);
    // A starting value whose BaseValue names the outer definition.
    let archetype = x.make("Engine.ToyPawn");
    let e = x.entry(h[4], None, Some(outer));
    x.set(archetype, "AttributeStartingValues", Value::array(vec![e]));
    preload_from(&mut x.vm, archetype).unwrap();
    for (name, o) in [("the formula term", nest), ("the operand", operand), ("BaseValueIfTrue", if_true)] {
        assert!(!x.unloaded(o), "{name} loaded");
    }
    assert!(x.unloaded(after), "past the always entry: not reached");
    assert!(x.unloaded(default), "the default: not reached");
}

/// The hub's point (a) on #811: a child class whose `AttributeParentClass` is an unloaded handle; the walk loads that
/// parent (logged) and reads its lists.
#[test]
fn an_unloaded_parent_class_is_followed() {
    let mut x = fx();
    let parent = x.held[11];
    assert!(x.unloaded(parent));
    let class = x.make("Engine.AIClassDefinition");
    x.set(class, "AttributeParentClass", Value::Object(Some(parent)));
    let archetype = x.make("Engine.ToyPawn");
    x.set(archetype, "AIClass", Value::Object(Some(class)));
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 1, "the parent class");
    assert!(!x.unloaded(parent));
    assert!(x.lines().iter().any(|l| l.contains("AIClass_Parent")));
}
