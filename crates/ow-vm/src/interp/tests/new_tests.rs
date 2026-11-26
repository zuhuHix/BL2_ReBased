//! `New` with five operands (decision 2026-11-26, NEW-FIVE-OPERANDS; NATIVE_TOUCH_COMPONENT_TEMPLATE.md sections 3.1,
//! 3.3 and 6) on invented packages (AGENTS.md rule 9): `Core` (a `Foo` holding the calls, `Thing` and `SubThing` with
//! no decodable class body, three classes with real bodies and chosen flags, a lazy `Thing` reached through a holder,
//! and the native `Op(int)`, which logs its argument and answers one operand) and `Engine` (an `Actor`). AI-assisted
//! (Claude Code).

use ow_object::heap::Origin;
use ow_object::ids::ClassId;
use ow_package::testkit::reflect::{ClassBodySpec, class_body};

use super::*;
use crate::interp::eval::{
    NEW_CONFIG, NEW_DEFAULT_NOT_BUILT, NEW_INSTANCING, NEW_TEMPLATE_COPY, NEW_TEMPLATE_NOT_LOADED,
    NEW_TEMPLATE_OTHER_CLASS,
};

const HAS_COMPONENTS: u32 = 0x0080_0000;
const PER_OBJECT_CONFIG: u32 = 0x0000_0400;

/// `New(o, n, f, c, t)` with each operand written by `operand(a, i)`.
fn new_of(a: &mut Asm, operand: impl Fn(&mut Asm, usize)) {
    a.raw(&[0x11]);
    for i in 0..5 {
        operand(a, i);
    }
}

fn core() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "IntProperty", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let foo = t.class("Foo", object);
    let thing = t.class("Thing", object);
    t.prop("Int", thing, "Count", 0, 0);
    let sub = t.class("SubThing", thing);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Held", 0, thing);
    // Classes with a real body and chosen class flags, each with its default object export.
    let real = |t: &mut ScriptToy, name: &str, flags: u32| -> i32 {
        let at = t.p.next_export();
        let spec = ClassBodySpec { flags, ..ClassBodySpec::new(object, object, at + 1) };
        let body = class_body(&mut t.p, &spec);
        let mut export = Export::new(0, name, body);
        export.super_ = object;
        t.p.add_export_with(export);
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&t.none);
        t.p.add_export(at, &format!("Default__{name}"), payload);
        at
    };
    let plain = real(&mut t, "Plain", 2);
    let comp = real(&mut t, "Comp", 2 | HAS_COMPONENTS);
    let config = real(&mut t, "Config", 2 | PER_OBJECT_CONFIG);
    // A lazy Thing behind a holder.
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let lazy = t.p.add_export(thing, "Thing_Lazy", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Held", lazy));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "HolderObject", h);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
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
    let t_param = [Param { type_ref: object, ..param("Object", "T", 0) }];
    for (name, class) in [("MakeThing", thing), ("MakePlain", plain), ("MakeComp", comp), ("MakeConfig", config)] {
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
    for class in ["Core.Foo", "Core.Thing", "Core.SubThing", "Core.Plain", "Core.Comp", "Core.Config", "Engine.Actor"] {
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
/// another class, then the class flags; a template every check passes is the copy's `NotPorted`.
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
    // Test 7: bit 23, then bit 10; a class with neither reaches the copy.
    for (function, class, missing, what) in [
        ("MakeComp", "Core.Comp", Missing::NotRead, NEW_INSTANCING),
        ("MakeConfig", "Core.Config", Missing::NotRead, NEW_CONFIG),
        ("MakePlain", "Core.Plain", Missing::NotPorted, NEW_TEMPLATE_COPY),
    ] {
        r.built(class);
        let c = r.class(class);
        let template = r.vm.instantiate(c, "Template").unwrap();
        r.refused(function, template, missing, what);
    }
}
