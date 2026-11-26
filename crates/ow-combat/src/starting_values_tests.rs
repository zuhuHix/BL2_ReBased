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
const HELD: [&str; 13] = [
    "Attr", "BaseAttr", "Def", "ParentAttr", "OwnAttr", "Broken", "NestAttr", "OperandAttr", "IfTrueAttr", "DefaultAttr",
    "AfterAttr", "ParentClass", "ChildClass",
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
    // The rest of an expression as `ow-attr`'s reader requires it (the shared walk reads expressions as the preflight).
    t.prop("Byte", expr, "ComparisonOperator", 0, 0);
    t.prop("Byte", expr, "Operand2Usage", 0, 0);
    t.prop("Float", expr, "ConstantOperand2", 0, 0);
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
    // A class declaring AttributeStartingValuesNew as an int (review of #815, item 4).
    let bad_categories = t.class("ToyBadCategoriesClass", object);
    toy_array_of(&mut t, bad_categories, "AttributeStartingValues", "Struct", entry);
    t.prop("Object", bad_categories, "AttributeParentClass", 0, class);
    t.prop("Int", bad_categories, "AttributeStartingValuesNew", 0, 0);
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
    // Discovery (the note's test 3): an archetype export whose AIClass is an exported class whose AttributeParentClass is
    // AIClass_Parent; both reached as unloaded handles from the archetype's copy.
    let mut c = ints(&[0]);
    c.extend(t.p.object_tag("AttributeParentClass", *nested.last().unwrap()));
    c.extend_from_slice(&none);
    let child = t.p.add_export(class, "AIClass_Child", c);
    nested.push(child);
    let mut pawn = ints(&[0]);
    pawn.extend(t.p.object_tag("AIClass", child));
    pawn.extend_from_slice(&none);
    t.p.add_export(archetype, "ToyPawn_Arch", pawn);
    // An archetype whose AIClass export is cut short: its load fails (review of #815, item 2).
    let broken_class = t.p.add_export(class, "AIClass_Broken", ints(&[0]));
    let mut pawn = ints(&[0]);
    pawn.extend(t.p.object_tag("AIClass", broken_class));
    pawn.extend_from_slice(&none);
    t.p.add_export(archetype, "ToyPawn_BrokenArch", pawn);
    // An AI class whose AttributeStartingValuesNew tag is an int's (review r2 of #815, item 4): with the categories
    // declared, the decoder skips it and leaves the default.
    let mut c = ints(&[0]);
    c.extend(t.p.int_tag("AttributeStartingValuesNew", 3));
    c.extend_from_slice(&none);
    let bad_tag = t.p.add_export(class, "AIClass_BadTag", c);
    let mut pawn = ints(&[0]);
    pawn.extend(t.p.object_tag("AIClass", bad_tag));
    pawn.extend_from_slice(&none);
    t.p.add_export(archetype, "ToyPawn_BadTagArch", pawn);
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

/// The note's test 3 (discovery), as review of #815 (items 2 and 3) has it: the archetype is the store's own load of
/// the export, logged first; its `AIClass` and that class's `AttributeParentClass` stay handles until the walk loads
/// and logs each, child first. Nothing is copied: a second discovery adds no object and no name.
#[test]
fn the_archetype_is_found_by_its_path() {
    let mut x = fx();
    let counts = |x: &Fx| (x.vm.space.heap.handles().len(), x.vm.space.names.len());
    assert_eq!(preload_archetype(&mut x.vm, "Engine", "Engine.ToyPawn_Arch").unwrap(), Some(2));
    let lines = x.lines();
    assert_eq!(lines[0], archetype_line("Engine.ToyPawn_Arch"), "{lines:?}");
    assert!(lines[1].contains("AIClass_Child") && lines[2].contains("AIClass_Parent"), "{lines:?}");
    assert!(!x.unloaded(x.held[11]) && !x.unloaded(x.held[12]));
    let before = counts(&x);
    assert_eq!(preload_archetype(&mut x.vm, "Engine", "Engine.ToyPawn_Arch").unwrap(), Some(0));
    assert_eq!(counts(&x), before, "a second discovery: no object, no name");
    // No such object, or no such package: nothing.
    assert_eq!(preload_archetype(&mut x.vm, "Engine", "Engine.NoSuchPawn").unwrap(), None);
    assert_eq!(preload_archetype(&mut x.vm, "NoSuchPackage", "Engine.ToyPawn_Arch").unwrap(), None);
}

/// Review of #815, item 2: an archetype whose `AIClass` export fails to load: the failure is logged and left (the
/// spawn's own refusal shows it), set-up goes on.
#[test]
fn a_broken_ai_class_is_logged_and_left() {
    let mut x = fx();
    assert_eq!(preload_archetype(&mut x.vm, "Engine", "Engine.ToyPawn_BrokenArch").unwrap(), Some(0));
    assert!(x.vm.log.iter().any(|l| l.contains("AIClass_Broken: failed")), "{:?}", x.vm.log);
}

/// `o.name` := `v` written raw, past the typed write's class check (a malformed value the typed write refuses).
fn raw(x: &mut Fx, o: ObjRef, name: &str, v: Value) {
    let class = x.vm.space.heap.get(o).unwrap().class;
    let p = x.vm.space.prop(class, name).unwrap();
    *x.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
}

/// The initialization data of a starting value whose `BaseValue` names `definition` only.
fn init_of(x: &Fx, definition: Option<ObjRef>) -> ow_attr::api::InitData {
    let e = x.entry(x.held[4], None, definition);
    let base = ow_attr::eval::field(&x.vm.space, &e, "BaseValue").unwrap().clone();
    ow_attr::eval::init_data(&x.vm.space, &base).unwrap()
}

/// Review of #815, item 1: a formula whose four terms name its own definition is followed once (a visit count), and a
/// chain of distinct definitions deeper than the bound stops the whole walk with the preflight's refusal.
#[test]
fn a_self_referential_formula_is_walked_once_and_the_depth_bound_stops_the_walk() {
    let mut x = fx();
    let me = x.make("Engine.AttributeInitializationDefinition");
    let mut formula = {
        let c = x.vm.space.heap.get(me).unwrap().class;
        let p = x.vm.space.prop(c, "ValueFormula").unwrap();
        x.vm.space.read(me, PropAddr::scalar(p)).unwrap().clone()
    };
    put(&x.vm, &mut formula, "bEnabled", Value::Bool(true));
    for term in ["Multiplier", "Level", "Power", "Offset"] {
        put(&x.vm, &mut formula, &format!("{term}.InitializationDefinition"), Value::Object(Some(me)));
    }
    x.set(me, "ValueFormula", formula.clone());
    let data = init_of(&x, Some(me));
    let attrs = x.vm.attrs.clone();
    let mut visits = 0;
    let mut count = |_: &mut ow_attr::api::EvalCx<'_>, _: ObjRef| -> Result<bool, ow_attr::api::AttrError> {
        visits += 1;
        Ok(true)
    };
    // A cycle refuses with the depth bound's error at its first repeat, as the preflight does (review r2 of #815).
    let e = attrs.walk_set_base_value(None, &data, &mut ow_attr::api::EvalCx::new(&mut x.vm), &mut count).unwrap_err();
    assert!(e.to_string().contains("nested deeper than"), "{e}");
    assert_eq!(visits, 1, "the definition once");
    // A chain of 40 distinct definitions, each one's Multiplier naming the next: the bound stops the walk.
    let chain: Vec<ObjRef> = (0..40).map(|_| x.make("Engine.AttributeInitializationDefinition")).collect();
    for w in chain.windows(2) {
        let mut f = formula.clone();
        for term in ["Level", "Power", "Offset"] {
            put(&x.vm, &mut f, &format!("{term}.InitializationDefinition"), Value::Object(None));
        }
        put(&x.vm, &mut f, "Multiplier.InitializationDefinition", Value::Object(Some(w[1])));
        x.set(w[0], "ValueFormula", f);
    }
    let data = init_of(&x, Some(chain[0]));
    let mut ok = |_: &mut ow_attr::api::EvalCx<'_>, _: ObjRef| -> Result<bool, ow_attr::api::AttrError> { Ok(true) };
    let e = attrs.walk_set_base_value(None, &data, &mut ow_attr::api::EvalCx::new(&mut x.vm), &mut ok).unwrap_err();
    assert!(e.to_string().contains("nested deeper than"), "{e}");
}

/// Review of #815, item 4: a definition that declares no `ValueFormula` (here another class's object in the slot) is
/// a data error, and so is an `AttributeStartingValuesNew` declared as an int, before any definition loads.
#[test]
fn malformed_data_is_a_script_error() {
    let mut x = fx();
    let own = x.held[4];
    let not_a_definition = x.make("Engine.AIClassDefinition");
    let mut e = x.entry(own, None, None);
    put(&x.vm, &mut e, "BaseValue.InitializationDefinition", Value::Object(Some(not_a_definition)));
    let archetype = x.make("Engine.ToyPawn");
    raw(&mut x, archetype, "AttributeStartingValues", Value::array(vec![e]));
    assert!(matches!(preload_from(&mut x.vm, archetype), Err(VmError::Script { .. })));
    let mut x = fx();
    let class = x.make("Engine.ToyBadCategoriesClass");
    let e = x.entry(x.held[0], None, None);
    x.set(class, "AttributeStartingValues", Value::array(vec![e]));
    let archetype = x.make("Engine.ToyPawn");
    raw(&mut x, archetype, "AIClass", Value::Object(Some(class)));
    let before = x.vm.defs.load_transitions();
    assert!(matches!(preload_from(&mut x.vm, archetype), Err(VmError::Script { .. })));
    assert_eq!(x.vm.defs.load_transitions(), before, "no definition loaded");
    assert!(x.unloaded(x.held[0]));
}

/// The note's test 3 (a loop): a parent chain that comes back to a class already seen stops there; each class's list
/// is read once.
#[test]
fn a_looping_parent_chain_stops_at_a_class_already_seen() {
    let mut x = fx();
    let archetype = x.chain();
    let class = match x.vm.space.read(archetype, PropAddr::scalar(x.vm.space.prop(x.vm.space.heap.get(archetype).unwrap().class, "AIClass").unwrap())).unwrap() {
        Value::Object(Some(c)) => *c,
        other => panic!("{other:?}"),
    };
    let parent = match x.vm.space.read(class, PropAddr::scalar(x.vm.space.prop(x.vm.space.heap.get(class).unwrap().class, "AttributeParentClass").unwrap())).unwrap() {
        Value::Object(Some(c)) => *c,
        other => panic!("{other:?}"),
    };
    x.set(parent, "AttributeParentClass", Value::Object(Some(class)));
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 7, "as without the loop");
}

/// The note's test 4: what the preload walks is what `SetBaseValue`'s preflight checks. Before the preload the preflight
/// refuses the nested formula's unloaded attribute (the note's bounded alternative's remaining refusal); after it the
/// preflight passes.
#[test]
fn after_the_preload_the_native_s_preflight_passes() {
    let mut x = fx();
    let (own, nest) = (x.held[4], x.held[6]);
    let outer = x.make("Engine.AttributeInitializationDefinition");
    let mut formula = {
        let c = x.vm.space.heap.get(outer).unwrap().class;
        let p = x.vm.space.prop(c, "ValueFormula").unwrap();
        x.vm.space.read(outer, PropAddr::scalar(p)).unwrap().clone()
    };
    put(&x.vm, &mut formula, "bEnabled", Value::Bool(true));
    put(&x.vm, &mut formula, "Multiplier.BaseValueAttribute", Value::Object(Some(nest)));
    x.set(outer, "ValueFormula", formula);
    let entry = x.entry(own, None, Some(outer));
    let base = ow_attr::eval::field(&x.vm.space, &entry, "BaseValue").unwrap().clone();
    let data = ow_attr::eval::init_data(&x.vm.space, &base).unwrap();
    let attrs = x.vm.attrs.clone();
    x.vm.defs.ensure_loaded(&mut x.vm.space, own).unwrap();
    let refused = attrs.preflight_set_base_value(own, &data, &mut ow_attr::api::EvalCx::new(&mut x.vm)).unwrap_err();
    assert!(refused.to_string().contains(ow_attr::eval::PREFLIGHT_UNLOADED), "{refused}");
    let archetype = x.make("Engine.ToyPawn");
    x.set(archetype, "AttributeStartingValues", Value::array(vec![entry]));
    preload_from(&mut x.vm, archetype).unwrap();
    assert!(!x.unloaded(nest));
    attrs.preflight_set_base_value(own, &data, &mut ow_attr::api::EvalCx::new(&mut x.vm)).unwrap();
}

/// The note's test 3 (an unloaded `AIClass`, as from another package): the class and then its unloaded parent are
/// each loaded and logged before their fields are read, child first.
#[test]
fn an_unloaded_ai_class_and_its_parent_are_loaded_in_order() {
    let mut x = fx();
    let (parent, child) = (x.held[11], x.held[12]);
    assert!(x.unloaded(child) && x.unloaded(parent));
    let archetype = x.make("Engine.ToyPawn");
    x.set(archetype, "AIClass", Value::Object(Some(child)));
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 2);
    let lines = x.lines();
    assert!(lines[0].contains("AIClass_Child") && lines[1].contains("AIClass_Parent"), "{lines:?}");
}

/// Review r2 of #815, item 1: 20 definitions, each formula naming the next in all four terms (4^19 paths, all within
/// the depth bound). The preload's walk and the native's preflight check each definition once: a visit count of the
/// order of 20, and the same answer as a walk of every path (the preflight passes).
#[test]
fn a_fan_out_chain_is_checked_once_per_definition() {
    let mut x = fx();
    let chain: Vec<ObjRef> = (0..20).map(|_| x.make("Engine.AttributeInitializationDefinition")).collect();
    let mut formula = {
        let c = x.vm.space.heap.get(chain[0]).unwrap().class;
        let p = x.vm.space.prop(c, "ValueFormula").unwrap();
        x.vm.space.read(chain[0], PropAddr::scalar(p)).unwrap().clone()
    };
    put(&x.vm, &mut formula, "bEnabled", Value::Bool(true));
    for w in chain.windows(2) {
        let mut f = formula.clone();
        for term in ["Multiplier", "Level", "Power", "Offset"] {
            put(&x.vm, &mut f, &format!("{term}.InitializationDefinition"), Value::Object(Some(w[1])));
        }
        x.set(w[0], "ValueFormula", f);
    }
    let data = init_of(&x, Some(chain[0]));
    let attrs = x.vm.attrs.clone();
    let mut visits = 0;
    let mut count = |_: &mut ow_attr::api::EvalCx<'_>, _: ObjRef| -> Result<bool, ow_attr::api::AttrError> {
        visits += 1;
        Ok(true)
    };
    attrs.walk_set_base_value(None, &data, &mut ow_attr::api::EvalCx::new(&mut x.vm), &mut count).unwrap();
    assert_eq!(visits, 20, "the preload: each definition once");
    let own = x.held[4];
    x.vm.defs.ensure_loaded(&mut x.vm.space, own).unwrap();
    let (r, checked) = attrs.preflight_set_base_value_counted(own, &data, &mut ow_attr::api::EvalCx::new(&mut x.vm));
    r.unwrap();
    assert!(checked <= 25, "the preflight: each definition once ({checked} objects checked)");
}

/// Review r2 of #815, items 2 to 4: an archetype not in the named package is logged and left (the export is resolved in
/// that package, not through the path's holder); a slotless stand-in in the class chain is skipped, logged; a skipped
/// `AttributeStartingValuesNew` tag is malformed, before any definition loads.
#[test]
fn discovery_s_named_package_stand_ins_and_skipped_tags() {
    let mut x = fx();
    assert_eq!(preload_archetype(&mut x.vm, "Engine", "Engine.NoSuchPawn").unwrap(), None);
    assert!(x.vm.log.iter().any(|l| l.contains("no such export in Engine")), "{:?}", x.vm.log);
    // A slotless stand-in as the AI class: skipped, logged, no error.
    let class = x.vm.space.classes.by_path("Engine.AIClassDefinition").unwrap();
    let stand_in = x.vm.space.alloc_stand_in(class, "Missing", ow_object::heap::Origin::Default { class }).unwrap();
    let archetype = x.make("Engine.ToyPawn");
    x.set(archetype, "AIClass", Value::Object(Some(stand_in)));
    assert_eq!(preload_from(&mut x.vm, archetype).unwrap(), 0);
    assert!(x.vm.log.iter().any(|l| l.contains("skipped (a stand-in without its slots")), "{:?}", x.vm.log);
    // A skipped categories tag: a data error, nothing loaded past the class.
    let mut x = fx_with(ParityMode::Off, true);
    let before = x.vm.defs.load_transitions();
    let r = preload_archetype(&mut x.vm, "Engine", "Engine.ToyPawn_BadTagArch");
    assert!(matches!(r, Err(VmError::Script { .. })), "{r:?}");
    assert!(x.vm.defs.load_transitions() - before <= 2, "the archetype and its class only");
    assert!(x.unloaded(x.held[0]));
}
