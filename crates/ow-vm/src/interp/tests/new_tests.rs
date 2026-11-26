//! `New` with five operands (decision 2026-11-26, NEW-FIVE-OPERANDS; NATIVE_TOUCH_COMPONENT_TEMPLATE.md sections 3.1,
//! 3.3 and 6) and option B's template copy (decision 2026-11-26-NNN, IMPL-NEW-TEMPLATE-COPY) on invented packages
//! (AGENTS.md rule 9): `Core` (an `Object` declaring `ObjectArchetype`, a `Foo` holding the calls, `Thing` and
//! `SubThing` with no decodable class body, three classes with real bodies and chosen flags, `Plain` holding an int, a
//! transient int and an array of objects, a lazy `Thing` reached through a holder, and the native `Op(int)`, which
//! logs its argument and answers one operand) and `Engine` (an `Actor`, and `Remote`, which nothing loads). The
//! component-instancing pass (NATIVE_NEW_COMPONENT_INSTANCING.md section 5) runs on `Holds`, a bit-23 class on a bit-23
//! `HoldsBase`, whose fields are of the component class `Gadget` (a `Core.Component`), a struct holding one, `Thing`
//! and `Engine.Remote`. AI-assisted (Claude Code).

use ow_object::heap::Origin;
use ow_object::ids::ClassId;
use ow_package::testkit::reflect::{ClassBodySpec, class_body, toy_struct};

use super::*;
use crate::interp::eval::{
    NEW_CLASSIFY_NOT_LOADED, NEW_COMPONENT_INSTANCING, NEW_CONFIG, NEW_DEFAULT_NOT_BUILT, NEW_INSTANCING,
    NEW_TEMPLATE_NOT_LOADED, NEW_TEMPLATE_OTHER_CLASS,
};

const HAS_COMPONENTS: u32 = 0x0080_0000;
const PER_OBJECT_CONFIG: u32 = 0x0000_0400;
/// The transient property flag (`PropertyDecl::TRANSIENT`).
const TRANSIENT: u64 = 0x2000;

/// `New(o, n, f, c, t)` with each operand written by `operand(a, i)`.
fn new_of(a: &mut Asm, operand: impl Fn(&mut Asm, usize)) {
    a.raw(&[0x11]);
    for i in 0..5 {
        operand(a, i);
    }
}

fn core() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "IntProperty",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    t.prop("Object", object, "ObjectArchetype", 0, object);
    let foo = t.class("Foo", object);
    let thing = t.class("Thing", object);
    t.prop("Int", thing, "Count", 0, 0);
    let sub = t.class("SubThing", thing);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Held", 0, thing);
    // Classes with a real body and chosen class flags, each with its default object export.
    let real_on = |t: &mut ScriptToy, super_: i32, name: &str, flags: u32, tags: &[u8]| -> i32 {
        let at = t.p.next_export();
        let spec = ClassBodySpec { flags, ..ClassBodySpec::new(super_, object, at + 1) };
        let body = class_body(&mut t.p, &spec);
        let mut export = Export::new(0, name, body);
        export.super_ = super_;
        t.p.add_export_with(export);
        let mut payload = ints(&[0]);
        payload.extend_from_slice(tags);
        payload.extend_from_slice(&t.none);
        t.p.add_export(at, &format!("Default__{name}"), payload);
        at
    };
    let real = |t: &mut ScriptToy, name: &str, flags: u32, tags: &[u8]| real_on(t, object, name, flags, tags);
    // The component class, and a struct holding one.
    let component = t.class("Component", object);
    let gadget = t.class("Gadget", component);
    let gadget_box = toy_struct(&mut t, object, "GadgetBox", None);
    t.prop("Object", gadget_box, "Inside", 0, gadget);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let remote = t.p.add_import("Core", "Class", engine, "Remote");
    // Plain's class default: Count 5, Temp (transient) 2.
    let mut tags = t.p.int_tag("Count", 5);
    tags.extend(t.p.int_tag("Temp", 2));
    let plain = real(&mut t, "Plain", 2, &tags);
    t.prop("Int", plain, "Count", 0, 0);
    t.prop("Int", plain, "Temp", TRANSIENT, 0);
    let inner = t.prop("Object", 0, "ItemsInner", 0, object);
    let items = t.prop("Array", plain, "Items", 0, inner);
    t.p.set_outer(inner, items);
    t.prop("Object", plain, "Gear", 0, gadget);
    // Holds (bit 23) on HoldsBase (bit 23).
    let holds_base = real(&mut t, "HoldsBase", 2 | HAS_COMPONENTS, &[]);
    t.prop("Object", holds_base, "BaseFar", 0, remote);
    let holds = real_on(&mut t, holds_base, "Holds", 2 | HAS_COMPONENTS, &[]);
    t.prop("Object", holds, "Part", 0, gadget);
    let part_inner = t.prop("Object", 0, "PartsInner", 0, gadget);
    let parts = t.prop("Array", holds, "Parts", 0, part_inner);
    t.p.set_outer(part_inner, parts);
    t.prop("Struct", holds, "Box", 0, gadget_box);
    t.prop("Object", holds, "TempPart", TRANSIENT, gadget);
    t.prop("Object", holds, "Other", 0, thing);
    t.prop("Object", holds, "Far", 0, remote);
    let comp = real(&mut t, "Comp", 2 | HAS_COMPONENTS, &[]);
    let config = real(&mut t, "Config", 2 | PER_OBJECT_CONFIG, &[]);
    // A lazy Thing behind a holder.
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let lazy = t.p.add_export(thing, "Thing_Lazy", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Held", lazy));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "HolderObject", h);
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let op = t
        .function(foo, "Op", &[param("Int", "Which", 0)], None, &[], &spec(FUNC_NATIVE | FUNC_FINAL | FUNC_PUBLIC, "Op"), |_| None)
        .function;
    let call_op = move |a: &mut Asm, n: i32| {
        a.raw(&[0x1C]).reference(op).int_const(n).raw(&[0x16]);
    };
    let result = Some(("Object", object));
    let ordered = |class_op: i32| {
        move |_: &FunctionIds, a: &mut Asm| {
            returns(a, |a| new_of(a, |a, i| call_op(a, [0, 1, 2, class_op, 4][i])));
        }
    };
    script(&mut t, foo, "Ordered", &[], result, &[], ordered(3));
    script(&mut t, foo, "OrderedNoClass", &[], result, &[], ordered(5));
    let plain_new = |class: i32| {
        move |_: &FunctionIds, a: &mut Asm| {
            returns(a, |a| {
                new_of(a, |a, i| match i {
                    0 => {
                        a.raw(&[0x2A]);
                    }
                    3 => {
                        a.raw(&[0x20]).reference(class);
                    }
                    _ => {
                        a.raw(&[0x0B]);
                    }
                })
            });
        }
    };
    script(&mut t, foo, "Make", &[], result, &[], plain_new(thing));
    script(&mut t, foo, "MakeActor", &[], result, &[], plain_new(actor));
    script(&mut t, foo, "MakeHoldsBare", &[], result, &[], plain_new(holds));
    script(&mut t, foo, "MakeConfigBare", &[], result, &[], plain_new(config));
    // Test 8: a parameter of component type on Holds is not a field.
    let g_param = [Param { type_ref: gadget, ..param("Object", "G", 0) }];
    let use_spec = spec(FUNC_NATIVE | FUNC_FINAL | FUNC_PUBLIC, "Use");
    t.function(holds, "Use", &g_param, None, &[], &use_spec, |_| None);
    let t_param = [Param { type_ref: object, ..param("Object", "T", 0) }];
    for (name, class) in
        [("MakeThing", thing), ("MakePlain", plain), ("MakeComp", comp), ("MakeConfig", config), ("MakeHolds", holds)]
    {
        script(&mut t, foo, name, &t_param, result, &[], |ids, a| {
            let template = ids.get("T");
            returns(a, |a| {
                new_of(a, |a, i| match i {
                    0 => {
                        a.raw(&[0x2A]);
                    }
                    3 => {
                        a.raw(&[0x20]).reference(class);
                    }
                    4 => {
                        a.local(template);
                    }
                    _ => {
                        a.raw(&[0x0B]);
                    }
                })
            });
        });
    }
    let _ = sub;
    t.build()
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function"]);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let object = t.p.add_import("Core", "Class", core, "Object");
    t.class("Actor", object);
    t.class("Remote", object);
    t.build()
}

/// A VM over both packages in `mode`, with `Foo.Op(int)` bound: it logs `op <n>` and answers operand `n` (0: no outer,
/// 1 and 2: None, 3: the class `Core.Thing`, 4: no template, 5: no class).
fn run(mode: ParityMode) -> Run {
    let dir = std::env::temp_dir().join(format!("ow-vm-new-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, e) = (core(), engine());
    let defs =
        DefStore::from_synthetic(&dir, &[("Core.upk", &c), ("Engine.upk", &e)], IdentityPolicy::FirstInScanOrder, mode)
            .unwrap();
    let mut vm =
        Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode).with_stub_policy(StubPolicy::LogAndZero);
    natives_core::register(&mut vm).unwrap();
    vm.register_native(
        "Foo.Op(int)",
        Rc::new(|c: &mut crate::api::NativeCall<'_>| {
            let n = match c.arg(0)? {
                Value::Int(n) => *n,
                other => panic!("{other:?}"),
            };
            c.vm.log.push(format!("op {n}"));
            Ok(match n {
                3 => Value::Class(Some(c.vm.find_class("Core.Thing")?)),
                5 => Value::Class(None),
                2 => Value::Int(0),
                _ => Value::Object(None),
            })
        }),
    )
    .unwrap();
    for class in [
        "Core.Foo",
        "Core.Thing",
        "Core.SubThing",
        "Core.Plain",
        "Core.Comp",
        "Core.Config",
        "Core.Gadget",
        "Core.Holds",
        "Engine.Actor",
    ] {
        vm.find_class(class).unwrap();
    }
    Run { vm, dir }
}

impl Run {
    fn ops(&mut self) -> Vec<String> {
        let ops = self.vm.log.iter().filter(|l| l.starts_with("op ")).cloned().collect();
        self.vm.log.clear();
        ops
    }

    fn make(&mut self, function: &str, args: Vec<Value>) -> Result<Value, VmError> {
        let foo = self.object("Core.Foo");
        self.call(&format!("Core.Foo.{function}"), Some(foo), args).map(|r| r.ret)
    }

    /// Writes, loads, class-code loads, the name table and the object count.
    fn marks(&self) -> (u64, u64, u64, usize, usize) {
        let vm = &self.vm;
        (
            vm.space.heap.committed_writes(),
            vm.defs.load_transitions(),
            vm.class_code_loads(),
            vm.space.names.len(),
            vm.space.heap.handles().len(),
        )
    }

    fn class(&mut self, path: &str) -> ClassId {
        self.vm.find_class(path).unwrap()
    }

    /// A template refusal of `function` on `template`: its text and kind, with nothing made, written, loaded or
    /// interned.
    fn refused(&mut self, function: &str, template: ObjRef, missing: Missing, what: &str) {
        let foo = self.object("Core.Foo");
        let f = self.vm.find_function(&format!("Core.Foo.{function}")).unwrap();
        let before = self.marks();
        let n = not_yet(self.vm.call(f, Some(foo), vec![Value::Object(Some(template))]));
        assert_eq!((&n.missing, &*n.what), (&missing, what), "{function}");
        assert_eq!(self.marks(), before, "{function}: nothing made, written, loaded or interned");
    }

    fn built(&mut self, path: &str) {
        self.vm.defs.class_default(&mut self.vm.space, path).unwrap();
    }
}

fn object_of(v: &Value) -> ObjRef {
    match v {
        Value::Object(Some(o)) => *o,
        other => panic!("{other:?}"),
    }
}

/// Test 10: Off evaluates outer, name, flags, class, template in that order; with no class, the other four too.
#[test]
fn off_evaluates_the_five_operands_in_order() {
    let mut r = run(ParityMode::Off);
    let made = r.make("Ordered", vec![]).unwrap();
    assert_eq!(r.ops(), ["op 0", "op 1", "op 2", "op 3", "op 4"]);
    let thing = r.class("Core.Thing");
    assert_eq!(r.vm.space.heap.get(object_of(&made)).unwrap().class, thing);
    assert_eq!(r.make("OrderedNoClass", vec![]).unwrap(), Value::Object(None));
    assert_eq!(r.ops(), ["op 0", "op 1", "op 2", "op 5", "op 4"], "no class: the other four are still evaluated");
}

/// Reference keeps the C++ order (its decoder reads four operands): the class, then (for a class) the outer, then the
/// template as the C++ next statement; name and flags are not evaluated.
#[test]
fn reference_keeps_the_four_operand_order() {
    let mut r = run(ParityMode::Reference);
    r.make("Ordered", vec![]).unwrap();
    assert_eq!(r.ops(), ["op 3", "op 0", "op 4"]);
    assert_eq!(r.make("OrderedNoClass", vec![]).unwrap(), Value::Object(None));
    assert_eq!(r.ops(), ["op 5", "op 4"]);
    // A template is evaluated and dropped: the object is made from the class defaults, as before.
    let thing = r.class("Core.Thing");
    let template = r.vm.instantiate(thing, "T").unwrap();
    let made = r.make("MakeThing", vec![Value::Object(Some(template))]).unwrap();
    assert_eq!(r.vm.space.heap.get(object_of(&made)).unwrap().class, thing);
}

/// Test 5: no template gives the class defaults, as before.
#[test]
fn no_template_is_the_class_defaults() {
    let mut r = run(ParityMode::Off);
    let made = object_of(&r.make("Make", vec![]).unwrap());
    let thing = r.class("Core.Thing");
    assert_eq!(r.vm.space.heap.get(made).unwrap().class, thing);
}

/// Test 6: an `Actor` class is None with the script error logged, and no object of it is made.
#[test]
fn an_actor_class_is_none_with_the_error_logged() {
    let mut r = run(ParityMode::Off);
    let actor = r.class("Engine.Actor");
    assert_eq!(r.make("MakeActor", vec![]).unwrap(), Value::Object(None));
    assert!(r.vm.log.iter().any(|l| l.contains("use Spawn")), "{:?}", r.vm.log);
    let heap = &r.vm.space.heap;
    assert!(
        !heap.handles().into_iter().any(|o| heap.get(o).is_ok_and(|x| x.class == actor && !matches!(x.origin, Origin::Default { .. }))),
        "no Actor made"
    );
}

/// Tests 4, 8 and 9, and section 3.3's order: an unloaded template, then a class default not built, then a template of
/// another class, then the class flags (a template every check passes is copied: the tests below).
#[test]
fn a_template_is_refused_by_the_first_check_that_fails() {
    let mut r = run(ParityMode::Off);
    // An unloaded store handle.
    let path = ObjectPath::parse("Core.HolderObject").unwrap();
    let holder = r.vm.defs.find(&mut r.vm.space, &path).unwrap().unwrap();
    let p = r.vm.space.prop(r.vm.space.heap.get(holder).unwrap().class, "Held").unwrap();
    let lazy = object_of(&r.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone());
    assert!(r.vm.defs.is_unloaded(&r.vm.space, lazy));
    r.refused("MakeThing", lazy, Missing::NotRead, NEW_TEMPLATE_NOT_LOADED);
    assert!(r.vm.defs.is_unloaded(&r.vm.space, lazy), "still unloaded");
    // A resident template, the class default not built yet.
    let thing = r.class("Core.Thing");
    let own = r.vm.instantiate(thing, "OwnThing").unwrap();
    r.refused("MakeThing", own, Missing::NotRead, NEW_DEFAULT_NOT_BUILT);
    // The default built; a template of a subclass.
    r.built("Core.Thing");
    let sub = r.class("Core.SubThing");
    let sub_template = r.vm.instantiate(sub, "Sub").unwrap();
    r.refused("MakeThing", sub_template, Missing::NotRead, NEW_TEMPLATE_OTHER_CLASS);
    // Thing's body does not decode, so its flags cannot rule the instancing step out.
    r.refused("MakeThing", own, Missing::NotRead, NEW_INSTANCING);
    // Test 7: bit 10 is the configuration read.
    r.built("Core.Config");
    let c = r.class("Core.Config");
    let template = r.vm.instantiate(c, "Template").unwrap();
    r.refused("MakeConfig", template, Missing::NotRead, NEW_CONFIG);
    // Bit 23 with no object field: the pass has nothing to check, so the object is made.
    r.built("Core.Comp");
    let c = r.class("Core.Comp");
    let template = r.vm.instantiate(c, "Template").unwrap();
    let foo = r.object("Core.Foo");
    r.make_on(foo, "MakeComp", template);
}

impl Run {
    /// `Core.Foo.<function>(template)` on one `Foo` object (`make` makes a new `Foo` each call).
    fn make_on(&mut self, foo: ObjRef, function: &str, template: ObjRef) -> ObjRef {
        let f = self.vm.find_function(&format!("Core.Foo.{function}")).unwrap();
        object_of(&self.vm.call(f, Some(foo), vec![Value::Object(Some(template))]).unwrap().ret)
    }

    fn slot(&self, o: ObjRef, name: &str) -> usize {
        let class = self.vm.space.heap.get(o).unwrap().class;
        self.vm.space.prop(class, name).unwrap_or_else(|| panic!("no {name}")).slot as usize
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        self.vm.space.heap.get(o).unwrap().slots[self.slot(o, name)].clone()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let s = self.slot(o, name);
        self.vm.space.heap.get_mut(o).unwrap().slots[s] = v;
    }

    /// A `Plain` template (its default built) with `Count` 7, `Temp` 9, `Items` [a, b] and its own `ObjectArchetype`
    /// set to `Foo`'s object, and the two element objects.
    fn plain_template(&mut self) -> (ObjRef, ObjRef, ObjRef) {
        self.built("Core.Plain");
        let plain = self.class("Core.Plain");
        let thing = self.class("Core.Thing");
        let template = self.vm.instantiate(plain, "PlainTemplate").unwrap();
        let (a, b) = (self.vm.instantiate(thing, "A").unwrap(), self.vm.instantiate(thing, "B").unwrap());
        let foo = self.object("Core.Foo");
        self.set(template, "Count", Value::Int(7));
        self.set(template, "Temp", Value::Int(9));
        self.set(template, "Items", Value::array(vec![Value::Object(Some(a)), Value::Object(Some(b))]));
        self.set(template, "ObjectArchetype", Value::Object(Some(foo)));
        (template, a, b)
    }
}

/// Tests 1 and 2: a template of the exact class, bits 23 and 10 clear, is copied. Its values come over (`Count` 7),
/// the transient field comes from the class default (`Temp` 2, not 9), the archetype is the template (the heap field
/// and the declared `ObjectArchetype` slot), and a None outer leaves it unset. The writes are the made object and its
/// one generated name; nothing is loaded and no class code is loaded.
#[test]
fn a_template_is_copied_with_its_transient_fields_from_the_class_default() {
    let mut r = run(ParityMode::Off);
    let (template, _, _) = r.plain_template();
    let foo = r.object("Core.Foo");
    let before = r.marks();
    let made = r.make_on(foo, "MakePlain", template);
    let after = r.marks();
    assert_ne!(made, template);
    let plain = r.class("Core.Plain");
    assert_eq!(r.vm.space.heap.get(made).unwrap().class, plain);
    assert_eq!(r.get(made, "Count"), Value::Int(7), "from the template");
    assert_eq!(r.get(made, "Temp"), Value::Int(2), "transient: from the class default");
    assert_eq!(r.get(template, "Temp"), Value::Int(9), "the template is unchanged");
    assert_eq!(r.vm.space.heap.get(made).unwrap().archetype, Some(template));
    assert_eq!(r.get(made, "ObjectArchetype"), Value::Object(Some(template)), "the declared slot too");
    assert_eq!(r.vm.space.heap.get(made).unwrap().outer, None, "the outer operand is None");
    assert!(after.0 > before.0, "writes counted");
    assert_eq!((after.1, after.2), (before.1, before.2), "nothing loaded, no class code loaded");
    assert_eq!(after.3, before.3 + 1, "one generated name interned");
    assert_eq!(after.4, before.4 + 1, "one object made");
}

/// Test 3: the template's array is the made object's own array with the same elements (the element objects shared,
/// not duplicated); changing one array leaves the other alone.
#[test]
fn a_template_s_array_is_copied_not_shared() {
    let mut r = run(ParityMode::Off);
    let (template, a, b) = r.plain_template();
    let foo = r.object("Core.Foo");
    let made = r.make_on(foo, "MakePlain", template);
    let both = Value::array(vec![Value::Object(Some(a)), Value::Object(Some(b))]);
    assert_eq!(r.get(made, "Items"), both);
    let s = r.slot(made, "Items");
    r.vm.space.heap.get_mut(made).unwrap().slots[s].array_mut().unwrap().pop();
    assert_eq!(r.get(made, "Items"), Value::array(vec![Value::Object(Some(a))]), "changed in place");
    assert_eq!(r.get(template, "Items"), both, "the template's array is untouched");
    let objects = r.vm.space.heap.handles().len();
    let again = r.make_on(foo, "MakePlain", template);
    assert_eq!(r.vm.space.heap.handles().len(), objects + 1, "the elements are not duplicated");
    assert_eq!(r.get(again, "Items"), both);
}

/// No template: the heap archetype stays unset (the class default's values, as before).
#[test]
fn no_template_leaves_the_archetype_unset() {
    let mut r = run(ParityMode::Off);
    let made = object_of(&r.make("Make", vec![]).unwrap());
    assert_eq!(r.vm.space.heap.get(made).unwrap().archetype, None);
}

/// Reference is unchanged: a template is evaluated and dropped, so the object holds the class defaults.
#[test]
fn reference_drops_the_template() {
    let mut r = run(ParityMode::Reference);
    let (template, _, _) = r.plain_template();
    let made = object_of(&r.make("MakePlain", vec![Value::Object(Some(template))]).unwrap());
    assert_eq!(r.get(made, "Count"), Value::Int(5), "the class default's value");
    assert_eq!(r.vm.space.heap.get(made).unwrap().archetype, None);
}

// The component-instancing pass (NATIVE_NEW_COMPONENT_INSTANCING.md section 5, decision 2026-11-26-NNN).

impl Run {
    /// A `Holds` template (its default built) with every object field None, and a `Gadget`.
    fn holds_template(&mut self) -> (ObjRef, ObjRef) {
        self.built("Core.Holds");
        let holds = self.class("Core.Holds");
        let gadget = self.class("Core.Gadget");
        let template = self.vm.instantiate(holds, "HoldsTemplate").unwrap();
        let g = self.vm.instantiate(gadget, "G").unwrap();
        (template, g)
    }

    /// `Core.Foo.<function>()` (no template) refused with `what`: nothing made, written, loaded or interned.
    fn refused_bare(&mut self, function: &str, what: &str) {
        let foo = self.object("Core.Foo");
        let f = self.vm.find_function(&format!("Core.Foo.{function}")).unwrap();
        let before = self.marks();
        let n = not_yet(self.vm.call(f, Some(foo), vec![]));
        assert_eq!((&n.missing, &*n.what), (&Missing::NotRead, what), "{function}");
        assert!(!n.what.starts_with("stopped after partial writes"), "{}", n.what);
        assert_eq!(self.marks(), before, "{function}: nothing made, written, loaded or interned");
    }
}

/// Tests 1, 3 (empty), 8, 10 (None) and 12: a bit-23 template whose component fields are None, whose array is empty,
/// whose unloaded-class field is None and whose non-component field holds an object is made; a component-typed
/// parameter of the class never takes part.
#[test]
fn a_bit_23_template_with_no_component_value_is_made() {
    let mut r = run(ParityMode::Off);
    let (template, _) = r.holds_template();
    let thing = r.class("Core.Thing");
    let other = r.vm.instantiate(thing, "OtherThing").unwrap();
    r.set(template, "Other", Value::Object(Some(other)));
    let foo = r.object("Core.Foo");
    let made = r.make_on(foo, "MakeHolds", template);
    assert_eq!(r.get(made, "Part"), Value::Object(None));
    assert_eq!(r.get(made, "Other"), Value::Object(Some(other)), "test 12: copied as is");
}

/// Tests 2, 3, 4 and 10: a non-None value in a component field, an array element, a struct member, or a field whose
/// class is not loaded is refused before anything is made.
#[test]
fn a_non_none_component_value_is_refused_before_anything_is_made() {
    let cases: [(&str, fn(&mut Run, ObjRef, ObjRef), &str); 5] = [
        ("Part", |r, t, g| r.set(t, "Part", Value::Object(Some(g))), NEW_COMPONENT_INSTANCING),
        (
            "Parts",
            |r, t, g| r.set(t, "Parts", Value::array(vec![Value::Object(None), Value::Object(Some(g))])),
            NEW_COMPONENT_INSTANCING,
        ),
        (
            "Box",
            |r, t, g| {
                let s = r.slot(t, "Box");
                r.vm.space.heap.get_mut(t).unwrap().slots[s].struct_mut().unwrap().fields[0] = Value::Object(Some(g));
            },
            NEW_COMPONENT_INSTANCING,
        ),
        ("Far", |r, t, g| r.set(t, "Far", Value::Object(Some(g))), NEW_CLASSIFY_NOT_LOADED),
        ("BaseFar", |r, t, g| r.set(t, "BaseFar", Value::Object(Some(g))), NEW_CLASSIFY_NOT_LOADED),
    ];
    for (field, set, what) in cases {
        let mut r = run(ParityMode::Off);
        let (template, g) = r.holds_template();
        set(&mut r, template, g);
        let remote = r.vm.space.classes.by_path("Engine.Remote");
        assert!(
            remote.is_none_or(|c| !r.vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined())),
            "Engine.Remote is not loaded"
        );
        r.refused("MakeHolds", template, Missing::NotRead, what);
        assert!(!field.is_empty());
    }
}

/// Test 11: the walk takes the super class's fields first: `HoldsBase.BaseFar` (an unloaded class) is met before
/// `Holds.Part` (a component), and names the refusal.
#[test]
fn the_super_class_s_field_is_met_first() {
    let mut r = run(ParityMode::Off);
    let (template, g) = r.holds_template();
    r.set(template, "Part", Value::Object(Some(g)));
    r.set(template, "BaseFar", Value::Object(Some(g)));
    r.refused("MakeHolds", template, Missing::NotRead, NEW_CLASSIFY_NOT_LOADED);
    r.set(template, "BaseFar", Value::Object(None));
    r.refused("MakeHolds", template, Missing::NotRead, NEW_COMPONENT_INSTANCING);
}

/// Test 5: a transient component field set on the template but None on the class default is made: its would-be value
/// is the class default's.
#[test]
fn a_transient_component_value_comes_from_the_class_default() {
    let mut r = run(ParityMode::Off);
    let (template, g) = r.holds_template();
    r.set(template, "TempPart", Value::Object(Some(g)));
    let foo = r.object("Core.Foo");
    let made = r.make_on(foo, "MakeHolds", template);
    assert_eq!(r.get(made, "TempPart"), Value::Object(None));
}

/// Test 6: a class without bit 23 whose template holds a component reference is made, the reference copied as is.
#[test]
fn without_bit_23_a_component_reference_is_copied() {
    let mut r = run(ParityMode::Off);
    let (template, _, _) = r.plain_template();
    let gadget = r.class("Core.Gadget");
    let g = r.vm.instantiate(gadget, "G").unwrap();
    r.set(template, "Gear", Value::Object(Some(g)));
    let foo = r.object("Core.Foo");
    let made = r.make_on(foo, "MakePlain", template);
    assert_eq!(r.get(made, "Gear"), Value::Object(Some(g)));
}

/// Test 9 and bit 10 with no template: the class default's values go through the same rule. A bit-23 or bit-10 class
/// whose default is not built is refused (building it is a write); with the default built and its component field
/// None the object is made; with that field holding a component, refused; bit 10 is the configuration read.
#[test]
fn the_no_template_path_checks_the_class_default_s_values() {
    let mut r = run(ParityMode::Off);
    r.refused_bare("MakeHoldsBare", NEW_DEFAULT_NOT_BUILT);
    r.built("Core.Holds");
    let made = object_of(&r.make("MakeHoldsBare", vec![]).unwrap());
    let holds = r.class("Core.Holds");
    assert_eq!(r.vm.space.heap.get(made).unwrap().class, holds);
    let default = r.vm.defs.class_default(&mut r.vm.space, "Core.Holds").unwrap();
    let gadget = r.class("Core.Gadget");
    let g = r.vm.instantiate(gadget, "G").unwrap();
    r.set(default, "Part", Value::Object(Some(g)));
    r.refused_bare("MakeHoldsBare", NEW_COMPONENT_INSTANCING);
    r.built("Core.Config");
    r.refused_bare("MakeConfigBare", NEW_CONFIG);
}
