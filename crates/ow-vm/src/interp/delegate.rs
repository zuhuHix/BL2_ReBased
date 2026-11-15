//! Owner: lane W4-INTERP2 (architecture section 8). Delegates: delegate calls, delegate values and the delegate
//! comparisons, a port of `callDelegate` and the delegate cases of `eval` in `src/interp.cpp` (`LetDelegate` is the
//! plain `Let` there and in `eval.rs`). AI-assisted (Claude Code).
//!
//! - **A delegate value** names an object and a function. `DelegateProperty` and `InstanceDelegate` give the running
//!   context's object and the named function; `EmptyDelegate` gives an unbound delegate (`Value::Delegate(None)`, the
//!   C++ delegate kind with no object and no function).
//! - **A delegate call** (`DelegateFunction`: a byte saying the delegate is a local, the delegate's own function, the
//!   delegate property's name, then the arguments) reads the delegate property of the context object. When it is
//!   bound to an object and a function name, the function is found on that object's class in its current state and
//!   called on that object; otherwise the delegate's own declaration (its default body) is called on the context
//!   object. No function is logged (`delegate <name> is not bound`) and gives None. The arguments are bound as for any
//!   call (`eval::call_function`).
//! - **A delegate held in a local variable.** The C++ does not read it and always calls the declaration (its own note:
//!   "not supported yet", UNVERIFIED). Under `ParityMode::Reference` that is reproduced; in Off mode the local is read
//!   and an unbound one calls the declaration as above (both readings agree), while a bound one is `NotYet`: which
//!   function the game calls then is not read (architecture R3, a stand-in only under Reference).
//! - **The comparisons** (`==` and `!=`, delegate with delegate or with a function) compare the object and the
//!   function name of both sides, as the C++ compares its value's object and text fields: a value that is not a
//!   delegate contributes its object (an object reference), its text (a string or a name), or nothing. Names compare
//!   by their text as interned (names are interned case-insensitively, so the first spelling met is the text; the C++
//!   compares the two spellings byte for byte).
//!
//! Status: a port of the C++ reading; every rule here is UNVERIFIED in the running game.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::{DelegateValue, Value};
use ow_script::decode::Expr;
use ow_script::ops::op;

use super::eval::{Ctx, Frame, call_function, eval, local_place_of, read};
use super::load::{function_at, loaded_for_read, script_error};
use crate::api::Vm;
use crate::error::VmError;

/// The delegate values and comparisons (every delegate opcode but the call and `LetDelegate`).
pub(crate) fn expression(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    match e.op {
        op::EMPTY_DELEGATE => Ok(Value::Delegate(None)),
        op::DELEGATE_PROPERTY | op::INSTANCE_DELEGATE => {
            let name = e
                .names
                .first()
                .ok_or_else(|| missing(f, e, "a function name"))?;
            let function = vm.space.names.intern(name);
            Ok(Value::Delegate(Some(DelegateValue {
                object: ctx,
                function,
            })))
        }
        op::EQUAL_EQUAL_DEL_DEL
        | op::EQUAL_EQUAL_DEL_FUNC
        | op::NOT_EQUAL_DEL_DEL
        | op::NOT_EQUAL_DEL_FUNC => {
            let a = eval(
                vm,
                f,
                e.kids.first().ok_or_else(|| missing(f, e, "operand 0"))?,
                ctx,
            )?;
            let b = eval(
                vm,
                f,
                e.kids.get(1).ok_or_else(|| missing(f, e, "operand 1"))?,
                ctx,
            )?;
            let same = parts(vm, &a) == parts(vm, &b);
            let equal = matches!(e.op, op::EQUAL_EQUAL_DEL_DEL | op::EQUAL_EQUAL_DEL_FUNC);
            Ok(Value::Bool(if equal { same } else { !same }))
        }
        other => Err(script_error(
            f.path(),
            format!("expression 0x{other:02x} is not a delegate expression"),
        )),
    }
}

fn missing(f: &Frame, e: &Expr, what: &str) -> VmError {
    script_error(
        f.path(),
        format!("expression 0x{:02x} has no {what} in {}", e.op, f.path()),
    )
}

/// The object and the text a comparison compares (the C++ value's `o` and `s` fields).
fn parts(vm: &Vm, v: &Value) -> (Option<ObjRef>, String) {
    match v {
        Value::Delegate(Some(d)) => (d.object, vm.space.names.text(d.function).into_owned()),
        Value::Object(o) | Value::Interface(o) => (*o, String::new()),
        Value::Str(s) => (None, s.to_string()),
        Value::Name(n) => (None, vm.space.names.text(*n).into_owned()),
        _ => (None, String::new()),
    }
}

/// `DelegateFunction` (C++ `callDelegate`).
pub(crate) fn call(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let property = e
        .names
        .first()
        .ok_or_else(|| missing(f, e, "a delegate property name"))?
        .clone();
    let local = e.bytes.first().is_some_and(|&b| b != 0);
    let declared = *e
        .refs
        .first()
        .ok_or_else(|| missing(f, e, "a delegate function reference"))?;
    let held = if local {
        local_delegate(vm, f, &property)?
    } else {
        property_delegate(vm, f, ctx, &property)?
    };
    let (function, target) = match bound(vm, &held) {
        Some((object, name)) => {
            let (class, state) = {
                let o = vm.space.heap.get(object)?;
                (o.class, o.state)
            };
            (vm.find_method(class, &name, state)?, Some(object))
        }
        // The delegate's own declaration (its default body), on the context object.
        None => (function_at(vm, f.code.package, declared)?, ctx),
    };
    let Some(function) = function else {
        vm.log.push(format!("delegate {property} is not bound"));
        return Ok(Value::None);
    };
    call_function(vm, f, function, target, e)
}

/// A delegate bound to an object and a function name (the C++ `o && !s.empty()`).
fn bound(vm: &Vm, v: &Value) -> Option<(ObjRef, String)> {
    match v {
        Value::Delegate(Some(DelegateValue {
            object: Some(object),
            function,
        })) => {
            let name = vm.space.names.text(*function).into_owned();
            (!name.is_empty()).then_some((*object, name))
        }
        _ => None,
    }
}

/// The delegate property `name` of the context object (the C++ `rt.property(*ctx, name)`), None without one.
fn property_delegate(vm: &mut Vm, f: &Frame, ctx: Ctx, name: &str) -> Result<Value, VmError> {
    let Some(object) = ctx else {
        return Ok(Value::None);
    };
    if !loaded_for_read(vm, object, f.path())? {
        return Ok(Value::None);
    }
    let class = vm.space.heap.get(object)?.class;
    let Some(prop) = vm
        .space
        .names
        .find(name)
        .and_then(|n| vm.space.classes.prop(class, n))
    else {
        return Ok(Value::None);
    };
    Ok(vm.space.read(object, PropAddr::scalar(prop))?.clone())
}

/// A delegate held in a local variable (module documentation): the C++ reading under Reference, the local's value in
/// Off mode when it is not bound, `NotYet` when it is.
fn local_delegate(vm: &mut Vm, f: &Frame, name: &str) -> Result<Value, VmError> {
    if vm.parity == ParityMode::Reference {
        return Ok(Value::None);
    }
    let wanted = vm.space.names.find(name);
    let entry = f
        .code
        .locals
        .iter()
        .position(|l| Some(l.decl.name) == wanted);
    let value = match entry {
        Some(entry) => read(vm, &local_place_of(f, entry))?,
        None => Value::None,
    };
    if bound(vm, &value).is_some() {
        return Err(NotYet::not_read(
            format!("calling the bound delegate held in local variable {name} of {}", f.path()),
            "decision 2026-11-15-20 (the C++ calls the declaration; the game's call through a local is not read)",
        )
        .into());
    }
    Ok(value)
}

/// Delegate calls, values and comparisons, and the interface query (`QueryInterface`, the casts, `InterfaceContext`,
/// the iterators' InterfaceClass filter) with `tests/vm_test.py`'s interface-table cases, on the shared fixture of
/// `aggregate.rs`.
#[cfg(test)]
mod tests {
    use ow_core::{Missing, ParityMode};
    use ow_object::value::{DelegateValue, Value};
    use ow_package::testkit::reflect::{ClassBodySpec, class_body};
    use ow_package::testkit::script::{CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
    use ow_package::testkit::{Export, ints};

    use crate::error::VmError;
    use crate::interp::aggregate::tests::{OPERATOR, Run, SCRIPT, Toy, class_param, let_, returns};

    const CPF_PARM: u64 = 0x80;
    const FUNC_DELEGATE: u32 = 0x100000;

    /// Foo with a delegate property `__OnHit__Delegate`, its declaration `OnHit()` (returns Self, the default
    /// body), `Target()` (returns Self) and the functions that bind, call and compare the delegate.
    fn delegates() -> Toy {
        let mut toy = Toy::new();
        let (foo, object) = (toy.foo, toy.object);
        let property = toy.t.prop("Delegate", foo, "__OnHit__Delegate", 0, 0);
        let on_hit = toy
            .function(
                foo,
                "OnHit",
                SCRIPT | FUNC_DELEGATE,
                &[],
                Some(("Object", object)),
                &[],
                |_, a| {
                    returns(a, |a| {
                        a.self_();
                    })
                },
            )
            .function;
        toy.function(
            foo,
            "Target",
            0x2 | 0x20000,
            &[],
            Some(("Object", object)),
            &[],
            |_, a| {
                returns(a, |a| {
                    a.self_();
                })
            },
        );
        let names = |toy: &mut Toy, n: &str| toy.t.p.fname(n, 0);
        let (prop_name, target, missing) = (
            names(&mut toy, "__OnHit__Delegate"),
            names(&mut toy, "Target"),
            names(&mut toy, "Missing"),
        );
        // CallHit() = OnHit() through the property (DelegateFunction: not local, the declaration, the property).
        toy.script("CallHit", &[], Some(("Object", object)), |_, a| {
            returns(a, |a| {
                a.raw(&[0x42, 0])
                    .reference(on_hit)
                    .raw(&prop_name)
                    .raw(&[0x16]);
            })
        });
        let o = Param {
            type_ref: object,
            ..Param::new("Object", "O")
        };
        // Bind(O) = (__OnHit__Delegate = O.Target); BindMissing(O) names a function O does not have.
        for (name, function) in [("Bind", target), ("BindMissing", missing)] {
            toy.script(name, std::slice::from_ref(&o), None, |ids, a| {
                a.stmt().raw(&[0x44]).instance(property);
                a.context(
                    |a| {
                        a.local(ids.get("O"));
                    },
                    |a| {
                        a.raw(&[0x4B]).raw(&function);
                    },
                );
                a.end();
            });
        }
        toy.script("Unbind", &[], None, |_, a| {
            a.stmt().raw(&[0x44]).instance(property).raw(&[0x3F]);
            a.end();
        });
        // IsTarget(O) = (__OnHit__Delegate == O.Target), and the three other comparisons.
        for (name, op) in [
            ("IsTarget", 0x3Bu8),
            ("IsNotTarget", 0x3C),
            ("IsTargetFunc", 0x3D),
            ("IsNotTargetFunc", 0x3E),
        ] {
            toy.script(
                name,
                std::slice::from_ref(&o),
                Some(("Bool", 0)),
                |ids, a| {
                    returns(a, |a| {
                        a.raw(&[op]).instance(property);
                        a.context(
                            |a| {
                                a.local(ids.get("O"));
                            },
                            |a| {
                                a.raw(&[0x43]).raw(&target).reference(0);
                            },
                        );
                        a.raw(&[0x16]);
                    })
                },
            );
        }
        toy.script("IsEmpty", &[], Some(("Bool", 0)), |_, a| {
            returns(a, |a| {
                a.raw(&[0x3B]).instance(property).raw(&[0x3F, 0x16]);
            })
        });
        // CallLocal(D) = D() for a delegate parameter (DelegateFunction with the local byte set).
        let d_name = names(&mut toy, "D");
        toy.script(
            "CallLocal",
            &[Param::new("Delegate", "D")],
            Some(("Object", object)),
            |_, a| {
                returns(a, |a| {
                    a.raw(&[0x42, 1])
                        .reference(on_hit)
                        .raw(&d_name)
                        .raw(&[0x16]);
                })
            },
        );
        toy
    }

    fn obj(o: ow_object::ids::ObjRef) -> Value {
        Value::Object(Some(o))
    }

    #[test]
    fn a_delegate_calls_the_bound_function_on_its_object_or_else_its_declaration() {
        let mut toy = delegates();
        let mut r = toy.run(ParityMode::Off);
        let (a, b) = (r.object("Core.Foo"), r.object("Core.Foo"));
        // Unbound: the declaration's default body, on the context object.
        assert_eq!(r.on(a, "Core.Foo.CallHit", vec![]), obj(a));
        // Bound to b.Target: Target runs on b.
        r.on(a, "Core.Foo.Bind", vec![obj(b)]);
        assert_eq!(r.on(a, "Core.Foo.CallHit", vec![]), obj(b));
        assert_eq!(
            r.on(a, "Core.Foo.IsTarget", vec![obj(b)]),
            Value::Bool(true)
        );
        assert_eq!(
            r.on(a, "Core.Foo.IsTarget", vec![obj(a)]),
            Value::Bool(false)
        );
        assert_eq!(
            r.on(a, "Core.Foo.IsNotTarget", vec![obj(b)]),
            Value::Bool(false)
        );
        assert_eq!(
            r.on(a, "Core.Foo.IsTargetFunc", vec![obj(b)]),
            Value::Bool(true)
        );
        assert_eq!(
            r.on(a, "Core.Foo.IsNotTargetFunc", vec![obj(a)]),
            Value::Bool(true)
        );
        assert_eq!(r.on(a, "Core.Foo.IsEmpty", vec![]), Value::Bool(false));
        // A function the bound object does not have: logged, None.
        r.on(a, "Core.Foo.BindMissing", vec![obj(b)]);
        assert_eq!(r.on(a, "Core.Foo.CallHit", vec![]), Value::None);
        assert!(
            r.vm.log
                .iter()
                .any(|l| l == "delegate __OnHit__Delegate is not bound"),
            "{:?}",
            r.vm.log
        );
        // EmptyDelegate unbinds it.
        r.on(a, "Core.Foo.Unbind", vec![]);
        assert_eq!(r.on(a, "Core.Foo.IsEmpty", vec![]), Value::Bool(true));
        assert_eq!(r.on(a, "Core.Foo.CallHit", vec![]), obj(a));
    }

    #[test]
    fn a_delegate_held_in_a_local_follows_the_cxx_under_reference_and_is_not_yet_bound_in_off_mode()
    {
        let mut toy = delegates();
        let bytes = toy.t.build();
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut r = crate::interp::aggregate::tests::run_on(&bytes, mode);
            let (a, b) = (r.object("Core.Foo"), r.object("Core.Foo"));
            let target = r.vm.space.names.intern("Target");
            let bound = Value::Delegate(Some(DelegateValue {
                object: Some(b),
                function: target,
            }));
            // Unbound (not supplied): the declaration, in both readings.
            assert_eq!(r.on(a, "Core.Foo.CallLocal", vec![]), obj(a));
            let called = r.call("Core.Foo.CallLocal", Some(a), vec![bound]);
            match mode {
                // The C++ never reads the local: the declaration again.
                ParityMode::Reference => assert_eq!(called.unwrap().ret, obj(a)),
                _ => match called {
                    Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NotRead),
                    other => panic!("expected NotYet, got {other:?}"),
                },
            }
        }
    }

    // ---- interfaces

    /// A class export with a decodable body (class reference 0, as in the installed packages) and its default
    /// object, as `vm_test.py`'s `make_class` writes them (no `VfTable_` property: the query does not read it).
    fn real_class(
        t: &mut ScriptToy,
        name: &str,
        super_ref: i32,
        within: i32,
        flags: u32,
        table: &[i32],
    ) -> i32 {
        let at = t.p.next_export();
        let spec = ClassBodySpec {
            flags,
            interfaces: table.iter().map(|&c| (c, 0)).collect(),
            ..ClassBodySpec::new(super_ref, within, at + 1)
        };
        let body = class_body(&mut t.p, &spec);
        let mut export = Export::new(0, name, body);
        export.super_ = super_ref;
        assert_eq!(t.p.add_export_with(export), at);
        let mut default = ints(&[0]);
        default.extend_from_slice(&t.none);
        let mut export = Export::new(at, &format!("Default__{name}"), default);
        export.outer = 0;
        t.p.add_export_with(export);
        at
    }

    const INTERFACE: u32 = 2 | 0x4000;

    /// vm_test.py's interface-table fixture (all names invented): IThing and IThingChild declare no function, Impl
    /// names IThing, Derived inherits it, ChildImpl names the derived interface only, Plain names nothing; and two
    /// actors for the iterators' filter.
    fn interfaces() -> Toy {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "Function",
            "State",
            "ScriptStruct",
            "IntProperty",
            "ObjectProperty",
            "ClassProperty",
            "DelegateProperty",
        ]);
        let object = real_class(&mut t, "Object", 0, 1, 2, &[]);
        let thing = real_class(&mut t, "IThing", object, object, INTERFACE, &[]);
        let thing_child = real_class(&mut t, "IThingChild", thing, object, INTERFACE, &[]);
        let impl_ = real_class(&mut t, "Impl", object, object, 2, &[thing]);
        real_class(&mut t, "Derived", impl_, object, 2, &[]);
        real_class(&mut t, "ChildImpl", object, object, 2, &[thing_child]);
        real_class(&mut t, "Plain", object, object, 2, &[]);
        let actor = real_class(&mut t, "Actor", object, object, 2, &[]);
        real_class(&mut t, "ImplActor", actor, object, 2, &[thing]);
        real_class(&mut t, "PlainActor", actor, object, 2, &[]);
        let foo = real_class(&mut t, "Foo", object, object, 2, &[]);
        let mut add = FunctionSpec::new(OPERATOR, "+");
        add.native = 146;
        let operands = [Param::new("Int", "P0"), Param::new("Int", "P1")];
        t.function(
            object,
            "Add_IntInt",
            &operands,
            Some(("Int", 0)),
            &[],
            &add,
            |_| None,
        );
        let mut toy = Toy { t, object, foo };
        let query = toy.native(
            object,
            "QueryInterface",
            &[Param::new("Class", "InterfaceClass")],
            Some(("Object", object)),
            0,
        );
        class_param(&mut toy, query.get("InterfaceClass"), CPF_PARM, object);
        let all = toy.native(
            actor,
            "AllActors",
            &[
                Param::new("Class", "BaseClass"),
                Param {
                    flags: CPF_OUT,
                    type_ref: actor,
                    ..Param::new("Object", "Actor")
                },
                Param {
                    flags: CPF_OPT,
                    ..Param::new("Class", "InterfaceClass")
                },
            ],
            None,
            304,
        );
        class_param(&mut toy, all.get("BaseClass"), CPF_PARM, object);
        class_param(
            &mut toy,
            all.get("InterfaceClass"),
            CPF_PARM | CPF_OPT,
            object,
        );
        // AskThing() = QueryInterface(IThing), AskChild() = QueryInterface(IThingChild), on Object.
        for (name, iface) in [("AskThing", thing), ("AskChild", thing_child)] {
            toy.function(
                object,
                name,
                SCRIPT,
                &[],
                Some(("Object", object)),
                &[],
                |_, a| {
                    returns(a, |a| {
                        a.final_function(query.function, |a| {
                            a.raw(&[0x20]).reference(iface);
                        });
                    })
                },
            );
        }
        let o = Param {
            type_ref: object,
            ..Param::new("Object", "O")
        };
        // The casts to IThing, an object-to-interface primitive cast and an InterfaceContext, on Foo.
        for (name, op) in [("CastDynamic", 0x2Eu8), ("CastInterface", 0x52)] {
            toy.script(
                name,
                std::slice::from_ref(&o),
                Some(("Object", object)),
                |ids, a| {
                    returns(a, |a| {
                        a.raw(&[op]).reference(thing).local(ids.get("O"));
                    })
                },
            );
        }
        toy.script(
            "View",
            std::slice::from_ref(&o),
            Some(("Object", object)),
            |ids, a| {
                returns(a, |a| {
                    a.raw(&[0x51, 0x38, 0x46]).local(ids.get("O"));
                })
            },
        );
        // CountThings(C) = foreach AllActors(C, A, IThing) N++; return N.
        let params = [
            Param::new("Class", "C"),
            Param {
                type_ref: actor,
                ..Param::new("Object", "A")
            },
            Param::new("Int", "N"),
        ];
        let ids = toy.script("CountThings", &params, Some(("Int", 0)), |ids, a| {
            let (c, item, n) = (ids.get("C"), ids.get("A"), ids.get("N"));
            a.stmt()
                .raw(&[0x2F, 0x61, 0x30])
                .local(c)
                .local(item)
                .raw(&[0x20])
                .reference(thing)
                .raw(&[0x16]);
            let at = a.bytes.len();
            a.w(0);
            let_(
                a,
                |a| {
                    a.local(n);
                },
                |a| {
                    a.native(146, |a| {
                        a.local(n).int_const(1);
                    });
                },
            );
            a.stmt().raw(&[0x31]);
            let end = u16::try_from(a.here()).unwrap();
            a.stmt().raw(&[0x30]);
            a.bytes[at..at + 2].copy_from_slice(&end.to_le_bytes());
            returns(a, |a| {
                a.local(n);
            });
        });
        class_param(&mut toy, ids.get("C"), CPF_PARM, object);
        toy
    }

    #[test]
    fn query_interface_reads_the_interface_tables_of_the_class_chain() {
        let mut toy = interfaces();
        let mut r = toy.run(ParityMode::Off);
        let ask = |r: &mut Run, function: &str, class: &str| {
            let o = r.object(&format!("Core.{class}"));
            let got = r.on(o, &format!("Core.Object.{function}"), vec![]);
            (got, o)
        };
        // Own table, a super class's table, a table naming a derived interface.
        for class in ["Impl", "Derived", "ChildImpl"] {
            let (got, o) = ask(&mut r, "AskThing", class);
            assert_eq!(got, obj(o), "{class}");
        }
        assert_eq!(ask(&mut r, "AskThing", "Plain").0, Value::Object(None));
        let (got, o) = ask(&mut r, "AskChild", "ChildImpl");
        assert_eq!(got, obj(o));
        // A base-interface implementer is not the derived one.
        for class in ["Impl", "Derived"] {
            assert_eq!(
                ask(&mut r, "AskChild", class).0,
                Value::Object(None),
                "{class}"
            );
        }
        // The interface class itself, and a child of it, is a yes by the class chain.
        let (got, o) = ask(&mut r, "AskThing", "IThingChild");
        assert_eq!(got, obj(o));
    }

    #[test]
    fn the_casts_and_the_interface_context_answer_through_the_interface_query() {
        let mut toy = interfaces();
        let mut r = toy.run(ParityMode::Off);
        let foo = r.object("Core.Foo");
        let (impl_, plain) = (r.object("Core.Impl"), r.object("Core.Plain"));
        for f in ["Core.Foo.CastDynamic", "Core.Foo.CastInterface"] {
            assert_eq!(r.on(foo, f, vec![obj(impl_)]), obj(impl_), "{f}");
            assert_eq!(r.on(foo, f, vec![obj(plain)]), Value::Object(None), "{f}");
            assert_eq!(
                r.on(foo, f, vec![Value::Object(None)]),
                Value::Object(None),
                "{f}"
            );
        }
        // InterfaceContext and the object-interface cast pass the reference through.
        assert_eq!(r.on(foo, "Core.Foo.View", vec![obj(plain)]), obj(plain));
    }

    #[test]
    fn the_iterators_interface_filter_keeps_the_implementers() {
        let mut toy = interfaces();
        let mut r = toy.run(ParityMode::Off);
        let foo = r.object("Core.Foo");
        r.object("Core.ImplActor");
        r.object("Core.PlainActor");
        r.object("Core.ImplActor");
        let actor = Value::Class(Some(r.vm.find_class("Core.Actor").unwrap()));
        assert_eq!(
            r.on(foo, "Core.Foo.CountThings", vec![actor]),
            Value::Int(2)
        );
    }
}
