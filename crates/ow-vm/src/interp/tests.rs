//! Owner: lane W3-INTERP. Tests of the interpreter on an invented "Core" package (AGENTS.md rule 9), built with
//! `ow_package::testkit::script` and loaded through `DefStore::from_synthetic`.
//!
//! The cases re-express the `tests/vm_test.py` cases that use only this lane's features (its `make()` fixture, the
//! `--run` assertions, the hidden temporaries, the 0x5F attribute block, the census counters of `SumTo` and `NoneCtx`),
//! plus the GotoState wiring over package-loaded states (the `tests/vm_states_test.py` scenario `GoA`), and the error
//! and edge paths the C++ code has. `tests/runtime_test.py` runs only `--resolve`, `--scene-records` and `--payload`,
//! so none of its cases is this lane's.
//!
//! Most calls here pass their own object, one made with `Vm::instantiate` (zero slots, no class default object). The
//! cases that need a class default object (a call with no object, `Class.static`, `New`) get it from
//! `DefStore::class_default`, which answers since the W3-DEFS body landed;
//! `class_defaults_come_from_the_definition_store` asserts their results. The 0x5F let runs through W3-ATTREFFECTS'
//! `ow_attr::registry::let_base` with `vm_test.py`'s expected values.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::property_payload;
use ow_package::testkit::script::{
    Asm, CPF_OPT, CPF_OUT, FunctionIds, FunctionSpec, Param, ScriptToy, function_payload,
};
use ow_package::testkit::{Export, ints};

use crate::api::{CallCensus, StubPolicy, Vm};
use crate::error::VmError;
use crate::natives_core;

const FUNC_FINAL: u32 = 0x1;
const FUNC_DEFINED: u32 = 0x2;
const FUNC_NATIVE: u32 = 0x400;
const FUNC_PUBLIC: u32 = 0x20000;
const SCRIPT: u32 = FUNC_FINAL | FUNC_DEFINED | FUNC_PUBLIC;
const VIRTUAL: u32 = FUNC_DEFINED | FUNC_PUBLIC;
/// `static final native operator`, as `vm_test.py` declares its operators.
const OPERATOR: u32 = FUNC_NATIVE | 0x1000 | 0x23000;
const STACK_ATTRIBUTE: u64 = 1 << 63;
const PLAIN_ATTRIBUTE: u64 = 1 << 62;

/// The invented package and the exports the tests name.
struct Fixture {
    t: ScriptToy,
}

fn spec(flags: u32, friendly: &str) -> FunctionSpec {
    FunctionSpec::new(flags, friendly)
}

fn param(kind: &str, name: &str, flags: u64) -> Param {
    Param {
        flags,
        ..Param::new(kind, name)
    }
}

/// A script function on `owner` whose body `body` builds from the child ids.
fn script(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    params: &[Param],
    result: Option<(&str, i32)>,
    locals: &[(&str, &str)],
    body: impl FnOnce(&FunctionIds, &mut Asm),
) -> FunctionIds {
    t.function(
        owner,
        name,
        params,
        result,
        locals,
        &spec(SCRIPT, name),
        |ids| {
            let mut a = Asm::new();
            body(ids, &mut a);
            Some(a)
        },
    )
}

#[allow(clippy::too_many_arguments)] // one call per invented native, each line a full declaration
fn native(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    friendly: &str,
    number: u16,
    params: &[&str],
    result: Option<&str>,
    flags: u32,
) -> FunctionIds {
    let declared: Vec<Param> = params
        .iter()
        .enumerate()
        .map(|(i, k)| param(k, &format!("P{i}"), 0))
        .collect();
    let mut s = spec(flags, friendly);
    s.native = number;
    t.function(
        owner,
        name,
        &declared,
        result.map(|r| (r, 0)),
        &[],
        &s,
        |_| None,
    )
}

/// `Return <expr>` as one statement, then `EndOfScript`.
fn returns(a: &mut Asm, expr: impl FnOnce(&mut Asm)) {
    a.stmt().raw(&[0x04]);
    expr(a);
    a.end();
}

fn fixture() -> Fixture {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "State",
        "ScriptStruct",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "StrProperty",
        "NameProperty",
        "ByteProperty",
        "ObjectProperty",
        "ClassProperty",
        "ArrayProperty",
        "FloatAttributeProperty",
        "IntAttributeProperty",
    ]);
    let class_import = t.import("Class");
    let object = t.class("Object", 0);
    native(
        &mut t,
        object,
        "Add_IntInt",
        "+",
        146,
        &["Int", "Int"],
        Some("Int"),
        OPERATOR,
    );
    native(
        &mut t,
        object,
        "LessEq_IntInt",
        "<=",
        152,
        &["Int", "Int"],
        Some("Bool"),
        OPERATOR,
    );
    native(
        &mut t,
        object,
        "Concat_StrStr",
        "$",
        112,
        &["Str", "Str"],
        Some("Str"),
        OPERATOR,
    );
    native(
        &mut t,
        object,
        "AndAnd_BoolBool",
        "&&",
        130,
        &["Bool", "Bool"],
        Some("Bool"),
        OPERATOR,
    );
    let mystery = native(
        &mut t,
        object,
        "Mystery",
        "Mystery",
        999,
        &["Int"],
        Some("Int"),
        FUNC_NATIVE | FUNC_PUBLIC,
    )
    .function;
    let goto = {
        let declared = [
            param("Name", "NewState", CPF_OPT),
            param("Name", "Label", CPF_OPT),
            param("Bool", "bForceEvents", CPF_OPT),
            param("Bool", "bKeepStack", CPF_OPT),
        ];
        let mut s = spec(FUNC_NATIVE | FUNC_FINAL | FUNC_PUBLIC, "GotoState");
        s.native = 113;
        t.function(object, "GotoState", &declared, None, &[], &s, |_| None)
            .function
    };

    let foo = t.class("Foo", object);
    let bar = t.class("Bar", foo);
    let count = t.prop("Int", foo, "Count", 0, 0);

    script(&mut t, foo, "Answer", &[], Some(("Int", 0)), &[], |_, a| {
        returns(a, |a| {
            a.int_const(42);
        })
    });
    script(
        &mut t,
        foo,
        "Add2",
        &[param("Int", "A", 0), param("Int", "B", 0)],
        Some(("Int", 0)),
        &[],
        |ids, a| {
            returns(a, |a| {
                a.native(146, |a| {
                    a.local(ids.get("A")).local(ids.get("B"));
                });
            })
        },
    );
    // SumTo(n): i = 1; total = 0; while (i <= n) { total += i; i++ } return total. Built twice to learn the offsets.
    let sum_to = |ids: &FunctionIds, start: u16, end: u16| -> (Asm, u16, u16) {
        let (i, total, n) = (ids.get("I"), ids.get("Total"), ids.get("N"));
        let mut a = Asm::new();
        a.stmt().raw(&[0x0F]).local(i).raw(&[0x26]);
        a.stmt().raw(&[0x0F]).local(total).raw(&[0x25]);
        let looped = a.here() as u16;
        a.stmt().raw(&[0x07]).w(end).native(152, |a| {
            a.local(i).local(n);
        });
        a.stmt().raw(&[0x0F]).local(total).native(146, |a| {
            a.local(total).local(i);
        });
        a.stmt().raw(&[0x0F]).local(i).native(146, |a| {
            a.local(i).int_const(1);
        });
        a.stmt().raw(&[0x06]).w(start);
        let ended = a.here() as u16;
        a.stmt().raw(&[0x04]).local(total);
        a.end();
        (a, looped, ended)
    };
    t.function(
        foo,
        "SumTo",
        &[param("Int", "N", 0)],
        Some(("Int", 0)),
        &[("Int", "I"), ("Int", "Total")],
        &spec(SCRIPT, "SumTo"),
        |ids| {
            let (_, looped, ended) = sum_to(ids, 0, 0);
            Some(sum_to(ids, looped, ended).0)
        },
    );
    // Pick(n): switch (n) { case 1: return 10; case 2: return 20; default: return 30 }
    let pick = |ids: &FunctionIds, second: u16, default: u16| -> (Asm, u16, u16) {
        let n = ids.get("N");
        let mut a = Asm::new();
        a.stmt().raw(&[0x05]).reference(n).raw(&[1]).local(n);
        a.stmt().raw(&[0x0A]).w(second).int_const(1);
        a.stmt().raw(&[0x04]).int_const(10);
        let at_second = a.here() as u16;
        a.stmt().raw(&[0x0A]).w(default).int_const(2);
        a.stmt().raw(&[0x04]).int_const(20);
        let at_default = a.here() as u16;
        a.stmt().raw(&[0x0A]).w(0xFFFF);
        a.stmt().raw(&[0x04]).int_const(30);
        a.end();
        (a, at_second, at_default)
    };
    t.function(
        foo,
        "Pick",
        &[param("Int", "N", 0)],
        Some(("Int", 0)),
        &[],
        &spec(SCRIPT, "Pick"),
        |ids| {
            let (_, second, default) = pick(ids, 0, 0);
            Some(pick(ids, second, default).0)
        },
    );
    // Virtual dispatch: Foo.Get() = 1, Bar.Get() = 2, Foo.CallGet() = VirtualFunction Get.
    for (owner, value) in [(foo, 1), (bar, 2)] {
        t.function(
            owner,
            "Get",
            &[],
            Some(("Int", 0)),
            &[],
            &spec(VIRTUAL, "Get"),
            |_| {
                let mut a = Asm::new();
                returns(&mut a, |a| {
                    a.int_const(value);
                });
                Some(a)
            },
        );
    }
    let get_name = t.p.name("Get");
    script(
        &mut t,
        foo,
        "CallGet",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x1B]).i32(get_name).i32(0).raw(&[0x16]);
            })
        },
    );
    let nope_name = t.p.name("Nope");
    script(
        &mut t,
        foo,
        "CallNope",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x1B])
                    .i32(nope_name)
                    .i32(0)
                    .int_const(7)
                    .raw(&[0x16]);
            })
        },
    );
    script(
        &mut t,
        foo,
        "GetCount",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.instance(count);
            })
        },
    );
    // A class constant and a call through it run against the class default object.
    let util = t.class("Util", object);
    let level = t.prop("Int", util, "Level", 0, 0);
    let util_get = script(
        &mut t,
        util,
        "GetLevel",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.instance(level);
            })
        },
    )
    .function;
    script(
        &mut t,
        foo,
        "LevelThroughClass",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.context(
                    |a| {
                        a.raw(&[0x20]).reference(util);
                    },
                    |a| {
                        a.final_function(util_get, |_| {});
                    },
                );
            })
        },
    );
    // Opt(int a, optional int b = 5) = a + b
    script(
        &mut t,
        foo,
        "Opt",
        &[param("Int", "A", 0), param("Int", "B", CPF_OPT)],
        Some(("Int", 0)),
        &[],
        |ids, a| {
            a.stmt().raw(&[0x49]);
            let fix = a.bytes.len();
            a.w(0);
            let start = a.here();
            a.int_const(5);
            let size = (a.here() - start) as u16;
            a.bytes[fix..fix + 2].copy_from_slice(&size.to_le_bytes());
            a.stmt().raw(&[0x15]);
            returns(a, |a| {
                a.native(146, |a| {
                    a.local(ids.get("A")).local(ids.get("B"));
                });
            });
        },
    );
    // Bump(out int x) { x = x + 1 }
    script(
        &mut t,
        foo,
        "Bump",
        &[param("Int", "X", CPF_OUT)],
        Some(("Int", 0)),
        &[],
        |ids, a| {
            let x = ids.get("X");
            a.stmt().raw(&[0x0F, 0x48]).reference(x).native(146, |a| {
                a.local(x).int_const(1);
            });
            a.stmt()
                .raw(&[0x04, 0x3A])
                .reference(ids.get("ReturnValue"));
            a.end();
        },
    );
    script(
        &mut t,
        foo,
        "Join",
        &[param("Str", "A", 0), param("Str", "B", 0)],
        Some(("Str", 0)),
        &[],
        |ids, a| {
            returns(a, |a| {
                a.native(112, |a| {
                    a.local(ids.get("A")).local(ids.get("B"));
                });
            })
        },
    );
    script(
        &mut t,
        foo,
        "CallMystery",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.final_function(mystery, |a| {
                    a.int_const(5);
                });
            })
        },
    );
    // Context on a None object: yields a zero value; the census counts it.
    script(
        &mut t,
        foo,
        "NoneCtx",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x19, 0x2A]).w(1).reference(0).raw(&[0, 0x25]);
            })
        },
    );
    // Hidden typed temporaries 4C-50 (invented header pairs).
    for (suffix, code, kind) in [
        ("Int", 0x4C, "Int"),
        ("Float", 0x4D, "Float"),
        ("Byte", 0x4E, "Byte"),
        ("Bool", 0x4F, "Bool"),
        ("Object", 0x50, "Object"),
    ] {
        let mut s = spec(SCRIPT, &format!("Temp{suffix}"));
        s.hidden_locals = vec![0, 4];
        t.function(
            foo,
            &format!("Temp{suffix}"),
            &[],
            Some((kind, if kind == "Object" { object } else { 0 })),
            &[],
            &s,
            |_| {
                let mut a = Asm::new();
                returns(&mut a, |a| {
                    a.raw(&[code]).i32(1);
                });
                Some(a)
            },
        );
    }
    let temp =
        |t: &mut ScriptToy, name: &str, hidden: Vec<u16>, body: &dyn Fn(&mut Asm), kind: &str| {
            let mut s = spec(SCRIPT, name);
            s.hidden_locals = hidden;
            t.function(foo, name, &[], Some((kind, 0)), &[], &s, |_| {
                let mut a = Asm::new();
                body(&mut a);
                Some(a)
            });
        };
    temp(
        &mut t,
        "TempByteWrap",
        vec![0, 1 << 12],
        &|a| {
            a.stmt().raw(&[0x0F, 0x4E]).i32(1).int_const(300);
            a.stmt().raw(&[0x04, 0x4E]).i32(1);
            a.end();
        },
        "Byte",
    );
    for (name, operand, hidden) in [
        ("TempEven", 2, vec![0, 4]),
        ("TempFar", 3, vec![0, 4]),
        ("TempNoArray", 1, vec![]),
    ] {
        temp(
            &mut t,
            name,
            hidden,
            &|a| {
                returns(a, |a| {
                    a.raw(&[0x4C]).i32(operand);
                })
            },
            "Int",
        );
    }
    // Attributes: Speed lists (stack, base); its base lists (0, value). The stack holds Mod objects.
    let mod_class = t.class("Mod", object);
    t.prop("Byte", mod_class, "Type", 0, 0);
    t.prop("Float", mod_class, "Value", 0, 0);
    let first = t.p.next_export();
    let (stack, base, speed) = (first + 1, first + 2, first + 3);
    t.prop("Object", stack, "SpeedModifierStack_Element", 0, mod_class);
    t.prop("Array", foo, "SpeedModifierStack", 0, first);
    t.prop_tail(
        "FloatAttribute",
        foo,
        "SpeedBaseValue",
        PLAIN_ATTRIBUTE,
        &ints(&[0, speed]),
    );
    t.prop_tail(
        "FloatAttribute",
        foo,
        "Speed",
        STACK_ATTRIBUTE,
        &ints(&[stack, base]),
    );
    assert_eq!(t.p.next_export(), speed + 1);
    script(
        &mut t,
        foo,
        "SetSpeed",
        &[param("Float", "V", 0)],
        None,
        &[],
        |ids, a| {
            a.stmt()
                .raw(&[0x5F, 0x5E])
                .reference(speed)
                .local(ids.get("V"));
            a.stmt().return_nothing();
            a.end();
        },
    );
    let target = Param {
        type_ref: object,
        ..param("Object", "Target", 0)
    };
    script(
        &mut t,
        foo,
        "SetSpeedOn",
        &[param("Float", "V", 0), target],
        None,
        &[],
        |ids, a| {
            a.stmt()
                .raw(&[0x5F, 0x19])
                .local(ids.get("Target"))
                .w(0)
                .reference(0)
                .raw(&[0, 0x5E])
                .reference(speed)
                .local(ids.get("V"));
            a.stmt().return_nothing();
            a.end();
        },
    );
    script(
        &mut t,
        foo,
        "ReadSpeed",
        &[],
        Some(("Float", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x5E]).reference(speed);
            })
        },
    );
    // The rest of vm_test.py's attribute functions: the base read, a 5F through Context(Self), an int attribute Cap
    // with its own stack and base, and two right sides of other kinds (an int, which the assignment rule converts to
    // the float base, and a string, which it does not).
    let let_speed = |t: &mut ScriptToy, name: &str, kind: &str| {
        script(t, foo, name, &[param(kind, "V", 0)], None, &[], |ids, a| {
            a.stmt()
                .raw(&[0x5F, 0x5E])
                .reference(speed)
                .local(ids.get("V"));
            a.stmt().return_nothing();
            a.end();
        });
    };
    let_speed(&mut t, "SetSpeedInt", "Int");
    let_speed(&mut t, "SetSpeedStr", "Str");
    script(
        &mut t,
        foo,
        "SetSpeedSelf",
        &[param("Float", "V", 0)],
        None,
        &[],
        |ids, a| {
            a.stmt()
                .raw(&[0x5F, 0x19, 0x17])
                .w(0)
                .reference(0)
                .raw(&[0, 0x5E])
                .reference(speed)
                .local(ids.get("V"));
            a.stmt().return_nothing();
            a.end();
        },
    );
    let first = t.p.next_export();
    let (cap_stack, cap_base, cap) = (first + 1, first + 2, first + 3);
    t.prop(
        "Object",
        cap_stack,
        "CapModifierStack_Element",
        0,
        mod_class,
    );
    t.prop("Array", foo, "CapModifierStack", 0, first);
    t.prop_tail(
        "IntAttribute",
        foo,
        "CapBaseValue",
        PLAIN_ATTRIBUTE,
        &ints(&[0, cap]),
    );
    t.prop_tail(
        "IntAttribute",
        foo,
        "Cap",
        STACK_ATTRIBUTE,
        &ints(&[cap_stack, cap_base]),
    );
    script(
        &mut t,
        foo,
        "SetCap",
        &[param("Int", "V", 0)],
        None,
        &[],
        |ids, a| {
            a.stmt()
                .raw(&[0x5F, 0x5E])
                .reference(cap)
                .local(ids.get("V"));
            a.stmt().return_nothing();
            a.end();
        },
    );
    for (name, kind, op, reference) in [
        ("ReadSpeedBase", "Float", 0x01, base),
        ("ReadCap", "Int", 0x5E, cap),
        ("ReadCapBase", "Int", 0x01, cap_base),
    ] {
        script(&mut t, foo, name, &[], Some((kind, 0)), &[], |_, a| {
            returns(a, |a| {
                a.raw(&[op]).reference(reference);
            })
        });
    }
    // Out parameters alias the caller's storage (review r1 of PR #314). AliasTwo(out int A, out int B) { A = 1;
    // return B; }: with both arguments one variable, the C++ returns 1 and leaves the variable 1.
    let alias_two = script(
        &mut t,
        foo,
        "AliasTwo",
        &[param("Int", "A", CPF_OUT), param("Int", "B", CPF_OUT)],
        Some(("Int", 0)),
        &[],
        |ids, a| {
            a.stmt().raw(&[0x0F]).local(ids.get("A")).raw(&[0x26]);
            returns(a, |a| {
                a.local(ids.get("B"));
            });
        },
    )
    .function;
    script(
        &mut t,
        foo,
        "AliasLocal",
        &[],
        Some(("Int", 0)),
        &[("Int", "X")],
        |ids, a| {
            returns(a, |a| {
                a.final_function(alias_two, |a| {
                    a.local(ids.get("X")).local(ids.get("X"));
                });
            })
        },
    );
    script(
        &mut t,
        foo,
        "AliasLocalAfter",
        &[],
        Some(("Int", 0)),
        &[("Int", "X")],
        |ids, a| {
            a.stmt().final_function(alias_two, |a| {
                a.local(ids.get("X")).local(ids.get("X"));
            });
            returns(a, |a| {
                a.local(ids.get("X"));
            });
        },
    );
    script(
        &mut t,
        foo,
        "AliasCount",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.final_function(alias_two, |a| {
                    a.instance(count).instance(count);
                });
            })
        },
    );
    // A native's out write is seen at once by script it calls back into: PokeAndPeek(out int A) is registered by the
    // test (sets A = 5, then returns GetCount() on its object); CallPoke() = PokeAndPeek(Count).
    let poke = t
        .function(
            foo,
            "PokeAndPeek",
            &[param("Int", "A", CPF_OUT)],
            Some(("Int", 0)),
            &[],
            &spec(FUNC_NATIVE | FUNC_PUBLIC, "PokeAndPeek"),
            |_| None,
        )
        .function;
    script(
        &mut t,
        foo,
        "CallPoke",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.final_function(poke, |a| {
                    a.instance(count);
                });
            })
        },
    );
    // Frames past the bounds: a local of 65,535 elements; a recursion whose frames hold 4,001 slots each.
    for (name, dim, recurse) in [
        ("BigFrame", 65_535u32, false),
        ("BigRecursion", 4_000, true),
    ] {
        let function = t.p.next_export() + 2;
        t.prop("Int", function, "ReturnValue", 0x80 | 0x100 | 0x400, 0);
        let payload = property_payload(&t.none, dim, 0, &[]);
        let mut local = Export::new(t.import("IntProperty"), "Big", payload);
        local.outer = function;
        t.p.add_export_with(local);
        let mut a = Asm::new();
        returns(&mut a, |a| {
            if recurse {
                a.final_function(function, |_| {});
            } else {
                a.int_const(1);
            }
        });
        let payload = function_payload(&mut t.p, Some(&a), &spec(SCRIPT, name));
        let mut export = Export::new(t.import("Function"), name, payload);
        export.outer = foo;
        assert_eq!(t.p.add_export_with(export), function);
    }
    // The worst case of the stack: Deep() = 0 + (0 + (... 250 levels ... (Deep()) ...)), recursing to the depth limit.
    let deep = t.p.next_export() + 1;
    script(&mut t, foo, "Deep", &[], Some(("Int", 0)), &[], |ids, a| {
        assert_eq!(ids.function, deep);
        returns(a, |a| {
            for _ in 0..250 {
                a.raw(&[146]);
            }
            a.final_function(deep, |_| {});
            for _ in 0..250 {
                a.int_const(0).raw(&[0x16]);
            }
        })
    });
    // GotoLabel steps over (the C++), a state variable reads as an instance variable (the C++).
    let label = t.p.name("SomeLabel");
    script(
        &mut t,
        foo,
        "StepsOverGotoLabel",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            a.stmt().raw(&[0x0D, 0x21]).i32(label).i32(0);
            returns(a, |a| {
                a.int_const(1);
            });
        },
    );
    script(
        &mut t,
        foo,
        "ReadStateVariable",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x03]).reference(count);
            })
        },
    );
    // Heap typing: an object property declared Core.Object holds a class; one declared Bar refuses a Foo object.
    let owner_ref = t.prop("Object", foo, "Owner", 0, object);
    let bar_ref = t.prop("Object", foo, "BarRef", 0, bar);
    script(&mut t, foo, "SetOwnerClass", &[], None, &[], |_, a| {
        a.stmt()
            .raw(&[0x0F])
            .instance(owner_ref)
            .raw(&[0x20])
            .reference(foo);
        a.stmt().return_nothing();
        a.end();
    });
    script(
        &mut t,
        foo,
        "ReadOwner",
        &[],
        Some(("Object", object)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.instance(owner_ref);
            })
        },
    );
    let o = Param {
        type_ref: object,
        ..param("Object", "O", 0)
    };
    script(&mut t, foo, "SetBarRef", &[o], None, &[], |ids, a| {
        a.stmt().raw(&[0x0F]).instance(bar_ref).local(ids.get("O"));
        a.stmt().return_nothing();
        a.end();
    });
    // Native out reads go through the live alias (review r2 of PR #314): X = 1; X += ++X gives 4 in the C++ (`++X`
    // writes 2 through the alias before `+=` reads its first argument).
    let add_eq = {
        let mut sp = spec(OPERATOR, "+=");
        sp.native = 161;
        t.function(
            object,
            "AddEqual_IntInt",
            &[param("Int", "A", CPF_OUT), param("Int", "B", 0)],
            Some(("Int", 0)),
            &[],
            &sp,
            |_| None,
        )
        .function
    };
    let pre_inc = {
        let mut sp = spec(OPERATOR | 0x10, "++");
        sp.native = 163;
        t.function(
            object,
            "PreIncrement_Int",
            &[param("Int", "A", CPF_OUT)],
            Some(("Int", 0)),
            &[],
            &sp,
            |_| None,
        )
        .function
    };
    script(
        &mut t,
        foo,
        "IncTwice",
        &[],
        Some(("Int", 0)),
        &[("Int", "X")],
        |ids, a| {
            let x = ids.get("X");
            a.stmt().raw(&[0x0F]).local(x).raw(&[0x26]);
            a.stmt().final_function(add_eq, |a| {
                a.local(x).final_function(pre_inc, |a| {
                    a.local(x);
                });
            });
            returns(a, |a| {
                a.local(x);
            });
        },
    );
    // A native with two out parameters given one variable sees a write through the other: TwoOut (registered by the
    // test) sets A = 7 and returns B.
    let native_out = |t: &mut ScriptToy, name: &str, params: &[Param]| {
        t.function(
            foo,
            name,
            params,
            Some(("Int", 0)),
            &[],
            &spec(FUNC_NATIVE | FUNC_PUBLIC, name),
            |_| None,
        )
        .function
    };
    let two_out = native_out(
        &mut t,
        "TwoOut",
        &[param("Int", "A", CPF_OUT), param("Int", "B", CPF_OUT)],
    );
    let keep = native_out(&mut t, "Keep", &[param("Int", "A", CPF_OUT)]);
    script(
        &mut t,
        foo,
        "CallTwoOut",
        &[],
        Some(("Int", 0)),
        &[("Int", "X")],
        |ids, a| {
            returns(a, |a| {
                a.final_function(two_out, |a| {
                    a.local(ids.get("X")).local(ids.get("X"));
                });
            })
        },
    );
    // Keep (registered by the test) keeps a copy of its arguments past the call; CallKeep() = Keep(X); return X.
    script(
        &mut t,
        foo,
        "CallKeep",
        &[],
        Some(("Int", 0)),
        &[("Int", "X")],
        |ids, a| {
            a.stmt().final_function(keep, |a| {
                a.local(ids.get("X"));
            });
            returns(a, |a| {
                a.local(ids.get("X"));
            });
        },
    );
    // A function decoded first at depth: DeepN(N) = N <= 0 ? Leaf() : 0 + (... 250 levels ... DeepN(N - 1) ...), and
    // Leaf() nests 254 levels. DeepN(198) runs 199 frames, then Leaf decodes and runs at the 200th.
    let leaf = script(&mut t, foo, "Leaf", &[], Some(("Int", 0)), &[], |_, a| {
        returns(a, |a| {
            for _ in 0..254 {
                a.raw(&[146]);
            }
            a.int_const(1);
            for _ in 0..254 {
                a.int_const(0).raw(&[0x16]);
            }
        })
    })
    .function;
    let deep_n = t.p.next_export() + 2;
    let deep_body = |ids: &FunctionIds, other: u16| -> (Asm, u16) {
        let n = ids.get("N");
        let mut a = Asm::new();
        a.stmt().raw(&[0x07]).w(other).native(152, |a| {
            a.local(n).raw(&[0x25]);
        });
        a.stmt().raw(&[0x04]).final_function(leaf, |_| {});
        let at = a.here() as u16;
        returns(&mut a, |a| {
            for _ in 0..250 {
                a.raw(&[146]);
            }
            a.final_function(deep_n, |a| {
                a.native(146, |a| {
                    a.local(n).int_const(-1);
                });
            });
            for _ in 0..250 {
                a.int_const(0).raw(&[0x16]);
            }
        });
        (a, at)
    };
    let made = t.function(
        foo,
        "DeepN",
        &[param("Int", "N", 0)],
        Some(("Int", 0)),
        &[],
        &spec(SCRIPT, "DeepN"),
        |ids| {
            let (_, other) = deep_body(ids, 0);
            Some(deep_body(ids, other).0)
        },
    );
    assert_eq!(made.function, deep_n);
    // Recursion (the depth limit) and a loop that never ends (the step limit).
    let rec = t.p.next_export() + 1;
    script(&mut t, foo, "Rec", &[], Some(("Int", 0)), &[], |ids, a| {
        assert_eq!(ids.function, rec);
        returns(a, |a| {
            a.final_function(rec, |_| {});
        })
    });
    script(&mut t, foo, "Spin", &[], None, &[], |_, a| {
        a.stmt().raw(&[0x06]).w(0);
        a.end();
    });
    // An index no code package declares.
    script(
        &mut t,
        foo,
        "CallIndex900",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x63, 0x84]).int_const(1).raw(&[0x16]);
            })
        },
    );
    // Casts.
    let cast = |t: &mut ScriptToy, name: &str, kind: &str, result: &str, code: u8| {
        t.function(
            foo,
            name,
            &[param(kind, "V", 0)],
            Some((result, 0)),
            &[],
            &spec(SCRIPT, name),
            |ids| {
                let mut a = Asm::new();
                returns(&mut a, |a| {
                    a.raw(&[0x38, code]).local(ids.get("V"));
                });
                Some(a)
            },
        );
    };
    cast(&mut t, "IntToFloat", "Int", "Float", 0x3F);
    cast(&mut t, "FloatToInt", "Float", "Int", 0x44);
    cast(&mut t, "StrToInt", "Str", "Int", 0x4A);
    cast(&mut t, "StrToFloat", "Str", "Float", 0x4C);
    cast(&mut t, "StrToBool", "Str", "Bool", 0x4B);
    cast(&mut t, "FloatToStr", "Float", "Str", 0x55);
    cast(&mut t, "BoolToStr", "Bool", "Str", 0x54);
    cast(&mut t, "IntToByte", "Int", "Byte", 0x3D);
    cast(&mut t, "StrToName", "Str", "Name", 0x60);
    cast(&mut t, "StrToVector", "Str", "Int", 0x4D);
    let o = Param {
        type_ref: object,
        ..param("Object", "O", 0)
    };
    script(
        &mut t,
        foo,
        "AsBar",
        &[o],
        Some(("Object", object)),
        &[],
        |ids, a| {
            returns(a, |a| {
                a.raw(&[0x2E]).reference(bar).local(ids.get("O"));
            })
        },
    );
    // MetaBar(class C) = class<Bar>(C). A class property names its metaclass second, which `ScriptToy::function`
    // cannot write, so the children are written here.
    {
        let function = t.p.next_export() + 2;
        let meta = ints(&[class_import, object]);
        t.prop_tail(
            "Class",
            function,
            "ReturnValue",
            0x80 | 0x100 | 0x400,
            &meta,
        );
        let c = t.prop_tail("Class", function, "C", 0x80, &meta);
        let mut a = Asm::new();
        returns(&mut a, |a| {
            a.raw(&[0x13]).reference(bar).local(c);
        });
        let payload = function_payload(&mut t.p, Some(&a), &spec(SCRIPT, "MetaBar"));
        let mut export = Export::new(t.import("Function"), "MetaBar", payload);
        export.outer = foo;
        assert_eq!(t.p.add_export_with(export), function);
    }
    script(
        &mut t,
        foo,
        "Pick2",
        &[param("Bool", "B", 0)],
        Some(("Int", 0)),
        &[],
        |ids, a| {
            returns(a, |a| {
                a.raw(&[0x45])
                    .local(ids.get("B"))
                    .w(0)
                    .int_const(1)
                    .w(0)
                    .int_const(2);
            })
        },
    );
    script(
        &mut t,
        foo,
        "AssertFalse",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            a.stmt().raw(&[0x09]).w(7).raw(&[0]).false_();
            returns(a, |a| {
                a.int_const(1);
            });
        },
    );
    script(
        &mut t,
        foo,
        "FallsOff",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            a.end();
        },
    );
    script(
        &mut t,
        foo,
        "ShortCircuit",
        &[],
        Some(("Bool", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.native(130, |a| {
                    a.false_().final_function(mystery, |a| {
                        a.int_const(1);
                    });
                });
            })
        },
    );
    script(
        &mut t,
        foo,
        "Unicode",
        &[],
        Some(("Str", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x34]);
                for unit in [0x48u16, 0xD83D, 0xDE00, 0xD800, 0x21, 0] {
                    a.w(unit);
                }
            })
        },
    );
    script(&mut t, foo, "Ansi", &[], Some(("Str", 0)), &[], |_, a| {
        returns(a, |a| {
            a.raw(&[0x1F, b'c', 0xE9, 0]);
        })
    });
    script(&mut t, foo, "Broken", &[], Some(("Int", 0)), &[], |_, a| {
        a.stmt().raw(&[0x04, 0x5B]);
        a.end();
    });
    script(
        &mut t,
        foo,
        "StaticAnswer",
        &[],
        Some(("Int", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.class_context(foo, |a| {
                    a.raw(&[0x1B]).i32(get_name).i32(0).raw(&[0x16]);
                });
            })
        },
    );
    script(
        &mut t,
        foo,
        "MakeMod",
        &[],
        Some(("Object", object)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x11, 0x2A, 0x25, 0x25, 0x20]).reference(mod_class);
            })
        },
    );
    script(
        &mut t,
        foo,
        "Nested",
        &[param("Int", "N", 0)],
        Some(("Int", 0)),
        &[],
        |ids, a| {
            // return N + (N + 1): a native call inside a native call's arguments.
            returns(a, |a| {
                a.native(146, |a| {
                    a.local(ids.get("N")).native(146, |a| {
                        a.local(ids.get("N")).int_const(1);
                    });
                });
            })
        },
    );

    // Object.GotoState state events: Mach declares state A; BeginState appends to Trace and keeps its argument.
    let mach = t.class("Mach", object);
    let trace = t.prop("Str", mach, "Trace", 0, 0);
    let begin_arg = t.prop("Name", mach, "BeginArg", 0, 0);
    let mut state = Export::new(t.import("State"), "A", ints(&[0, 0, 0, 0]));
    state.outer = mach;
    let state_a = t.p.add_export_with(state);
    t.function(
        state_a,
        "BeginState",
        &[param("Name", "PreviousStateName", 0)],
        None,
        &[],
        &spec(VIRTUAL, "BeginState"),
        |ids| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x0F]).instance(trace).native(112, |a| {
                a.instance(trace).raw(&[0x1F, b'b', b'A', b';', 0]);
            });
            a.stmt()
                .raw(&[0x0F])
                .instance(begin_arg)
                .local(ids.get("PreviousStateName"));
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    for (owner, text) in [(state_a, b"A".as_slice()), (mach, b"-".as_slice())] {
        t.function(
            owner,
            "Where",
            &[],
            Some(("Str", 0)),
            &[],
            &spec(VIRTUAL, "Where"),
            |_| {
                let mut a = Asm::new();
                returns(&mut a, |a| {
                    a.raw(&[0x1F]).raw(text).raw(&[0]);
                });
                Some(a)
            },
        );
    }
    let state_name = t.p.name("A");
    let missing_name = t.p.name("Nope");
    let where_name = t.p.name("Where");
    for (name, target) in [("GoA", state_name), ("GoMissing", missing_name)] {
        script(&mut t, mach, name, &[], None, &[], |_, a| {
            a.stmt().final_function(goto, |a| {
                a.raw(&[0x21]).i32(target).i32(0).raw(&[0x0B, 0x28]);
            });
            a.stmt().return_nothing();
            a.end();
        });
    }
    script(
        &mut t,
        mach,
        "CallWhere",
        &[],
        Some(("Str", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x1B]).i32(where_name).i32(0).raw(&[0x16]);
            })
        },
    );
    script(
        &mut t,
        mach,
        "GlobalWhere",
        &[],
        Some(("Str", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.raw(&[0x37]).i32(where_name).i32(0).raw(&[0x16]);
            })
        },
    );
    script(
        &mut t,
        mach,
        "ReadTrace",
        &[],
        Some(("Str", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.instance(trace);
            })
        },
    );
    script(
        &mut t,
        mach,
        "ReadBeginArg",
        &[],
        Some(("Name", 0)),
        &[],
        |_, a| {
            returns(a, |a| {
                a.instance(begin_arg);
            })
        },
    );
    Fixture { t }
}

/// A VM over the fixture's package, its own temporary folder removed on drop.
struct Run {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

static NEXT: AtomicU32 = AtomicU32::new(0);

fn run_on(bytes: &[u8], mode: ParityMode) -> Run {
    let dir = std::env::temp_dir().join(format!(
        "ow-vm-interp-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Core.upk", bytes)],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    )
    .with_stub_policy(StubPolicy::LogAndZero);
    natives_core::register(&mut vm).unwrap();
    Run { vm, dir }
}

fn start(mode: ParityMode) -> Run {
    let mut f = fixture();
    run_on(&f.t.build(), mode)
}

impl Run {
    fn object(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.instantiate(c, "").unwrap()
    }

    fn call(
        &mut self,
        path: &str,
        this: Option<ObjRef>,
        args: Vec<Value>,
    ) -> Result<crate::api::CallResult, VmError> {
        let f = self.vm.find_function(path)?;
        self.vm.call(f, this, args)
    }

    /// Calls `path` on a new object of `Core.Foo` (or of `class`), returning the result.
    fn on(&mut self, class: &str, path: &str, args: Vec<Value>) -> Value {
        let o = self.object(class);
        self.call(path, Some(o), args).unwrap().ret
    }

    fn text(&self, v: &Value) -> String {
        super::describe(&self.vm, v)
    }
}

fn not_yet(r: Result<impl std::fmt::Debug, VmError>) -> NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn script_message(r: Result<impl std::fmt::Debug, VmError>) -> String {
    match r {
        Err(VmError::Script { message, .. }) => message,
        other => panic!("expected a script error, got {other:?}"),
    }
}

#[test]
fn constants_locals_natives_and_the_32_bit_wrap() {
    let mut r = start(ParityMode::Off);
    assert_eq!(r.on("Core.Foo", "Core.Foo.Answer", vec![]), Value::Int(42));
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.Add2",
            vec![Value::Int(40), Value::Int(2)]
        ),
        Value::Int(42)
    );
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.Add2",
            vec![Value::Int(i32::MAX), Value::Int(1)]
        ),
        Value::Int(i32::MIN)
    );
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.Join",
            vec![Value::string("ab"), Value::string("cd")]
        ),
        Value::string("abcd")
    );
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.Nested", vec![Value::Int(3)]),
        Value::Int(7)
    );
    assert!(r.vm.log.is_empty(), "{:?}", r.vm.log);
}

#[test]
fn jumps_and_switch() {
    let mut r = start(ParityMode::Off);
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.SumTo", vec![Value::Int(10)]),
        Value::Int(55)
    );
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.SumTo", vec![Value::Int(0)]),
        Value::Int(0)
    );
    for (n, want) in [(1, 10), (2, 20), (3, 30)] {
        assert_eq!(
            r.on("Core.Foo", "Core.Foo.Pick", vec![Value::Int(n)]),
            Value::Int(want)
        );
    }
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.Pick2", vec![Value::Bool(true)]),
        Value::Int(1)
    );
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.Pick2", vec![Value::Bool(false)]),
        Value::Int(2)
    );
}

#[test]
fn virtual_dispatch_follows_the_receivers_class() {
    let mut r = start(ParityMode::Off);
    assert_eq!(r.on("Core.Foo", "Core.Foo.CallGet", vec![]), Value::Int(1));
    assert_eq!(r.on("Core.Bar", "Core.Foo.CallGet", vec![]), Value::Int(2));
    // A name no class of the chain declares: a stub site whose arguments are still evaluated.
    assert_eq!(r.on("Core.Bar", "Core.Foo.CallNope", vec![]), Value::None);
    assert_eq!(r.vm.log, ["UNIMPLEMENTED function Bar.Nope"]);
    // Under Refuse, the same site is an error carrying its key.
    r.vm.stub_policy = StubPolicy::refuse_all();
    let o = r.object("Core.Foo");
    assert_eq!(
        r.call("Core.Foo.CallNope", Some(o), vec![]).unwrap_err(),
        VmError::UnboundNative("Foo.Nope".into())
    );
}

#[test]
fn instance_variables_read_the_objects_slots() {
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Bar");
    let foo = r.vm.find_class("Core.Foo").unwrap();
    let count = r.vm.space.prop(foo, "Count").unwrap();
    r.vm.space
        .write(o, PropAddr::scalar(count), Value::Int(9))
        .unwrap();
    assert_eq!(
        r.call("Core.Foo.GetCount", Some(o), vec![]).unwrap().ret,
        Value::Int(9)
    );
}

#[test]
fn optional_and_out_parameters() {
    let mut r = start(ParityMode::Off);
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.Opt", vec![Value::Int(1)]),
        Value::Int(6)
    );
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.Opt",
            vec![Value::Int(1), Value::Int(10)]
        ),
        Value::Int(11)
    );
    let o = r.object("Core.Foo");
    let bumped = r
        .call("Core.Foo.Bump", Some(o), vec![Value::Int(41)])
        .unwrap();
    assert_eq!(bumped.outs, vec![Value::Int(42)]);
    // ReturnNothing: the return slot, never written, is the zero int.
    assert_eq!(bumped.ret, Value::Int(0));
}

#[test]
fn an_unbound_native_is_a_logged_stub_with_a_zero_result() {
    let mut r = start(ParityMode::Off);
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.CallMystery", vec![]),
        Value::Int(0)
    );
    assert_eq!(r.vm.log, ["UNIMPLEMENTED Object.Mystery(int)"]);
    r.vm.log.clear();
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.CallIndex900", vec![]),
        Value::None
    );
    assert_eq!(r.vm.log, ["UNIMPLEMENTED native index 900"]);
    // && skips its second operand when the first decides: Mystery is not called.
    r.vm.log.clear();
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.ShortCircuit", vec![]),
        Value::Bool(false)
    );
    assert!(r.vm.log.is_empty(), "{:?}", r.vm.log);
}

#[test]
fn hidden_typed_temporaries() {
    let mut r = start(ParityMode::Off);
    let zeros = [
        ("Int", Value::Int(0)),
        ("Float", Value::Float(0.0)),
        ("Byte", Value::Byte(0)),
        ("Bool", Value::Bool(false)),
        ("Object", Value::Object(None)),
    ];
    for (suffix, zero) in zeros {
        assert_eq!(
            r.on("Core.Foo", &format!("Core.Foo.Temp{suffix}"), vec![]),
            zero,
            "{suffix}"
        );
    }
    // A byte slot keeps wrapping to a byte: 300 -> 44.
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.TempByteWrap", vec![]),
        Value::Byte(44)
    );
    for broken in ["TempEven", "TempFar", "TempNoArray"] {
        let o = r.object("Core.Foo");
        let message = script_message(r.call(&format!("Core.Foo.{broken}"), Some(o), vec![]));
        assert!(
            message.contains("outside the local-variable array"),
            "{broken}: {message}"
        );
    }
}

#[test]
fn context_on_none_counts_in_the_census_and_yields_none() {
    let mut r = start(ParityMode::Off);
    r.vm.census = Some(CallCensus::default());
    assert_eq!(r.on("Core.Foo", "Core.Foo.NoneCtx", vec![]), Value::None);
    let census = r.vm.census.take().unwrap();
    assert_eq!(census.none_contexts.get("Core.Foo.NoneCtx"), Some(&1));
}

#[test]
fn the_census_counts_sum_to_as_vm_test_does() {
    let mut r = start(ParityMode::Off);
    r.vm.census = Some(CallCensus::default());
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.SumTo", vec![Value::Int(3)]),
        Value::Int(6)
    );
    let census = r.vm.census.take().unwrap();
    // The loop test runs 4 times, the two additions 3 times each; one script function entered once.
    assert_eq!(
        census.native_calls.get("Core.Object.LessEq_IntInt"),
        Some(&4)
    );
    assert_eq!(census.native_calls.get("Core.Object.Add_IntInt"), Some(&6));
    assert_eq!(census.script_calls.get("Core.Foo.SumTo"), Some(&1));
    assert!(census.stub_calls.is_empty());
}

#[test]
fn the_trace_lists_calls_in_order() {
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Bar");
    r.vm.trace = Some(Vec::new());
    r.call("Core.Foo.CallGet", Some(o), vec![]).unwrap();
    assert_eq!(
        r.vm.trace.take().unwrap(),
        ["Core.Foo.CallGet", "Core.Bar.Get"]
    );
}

#[test]
fn steps_count_as_the_cxx_ticks() {
    let mut r = start(ParityMode::Off);
    r.on("Core.Foo", "Core.Foo.Answer", vec![]);
    // The call, the Return statement, the IntConst.
    assert_eq!(r.vm.steps(), 3);
    r.on(
        "Core.Foo",
        "Core.Foo.Add2",
        vec![Value::Int(1), Value::Int(2)],
    );
    // Call, Return, the native call expression, two local reads (eval + lval each), the native's own call.
    assert_eq!(r.vm.steps(), 8);
}

#[test]
fn the_step_limit_stops_a_runaway_script() {
    let mut r = start(ParityMode::Off);
    r.vm.limits.step_limit = 50;
    let o = r.object("Core.Foo");
    assert_eq!(
        r.call("Core.Foo.Spin", Some(o), vec![]).unwrap_err(),
        VmError::StepLimit
    );
    // The next top-level call starts a fresh budget.
    assert_eq!(
        r.call("Core.Foo.Answer", Some(o), vec![]).unwrap().ret,
        Value::Int(42)
    );
}

#[test]
fn the_depth_limit_names_the_innermost_callers() {
    // 200 nested frames need more than a 1 MiB thread in a debug build (module documentation of `interp`).
    let outcome = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut r = start(ParityMode::Off);
            let o = r.object("Core.Foo");
            let message = script_message(r.call("Core.Foo.Rec", Some(o), vec![]));
            let depth_after = r.vm.code.stack.len();
            (message, depth_after)
        })
        .unwrap()
        .join()
        .unwrap();
    let rec = "Core.Foo.Rec";
    assert_eq!(
        outcome.0,
        format!("call depth limit reached in {rec} <- {rec} <- {rec} <- {rec} <- {rec}")
    );
    assert_eq!(outcome.1, 0, "every frame was popped");
}

#[test]
fn casts() {
    let mut r = start(ParityMode::Off);
    let mut on = |path: &str, v: Value| r.on("Core.Foo", &format!("Core.Foo.{path}"), vec![v]);
    assert_eq!(on("IntToFloat", Value::Int(3)), Value::Float(3.0));
    assert_eq!(on("FloatToInt", Value::Float(-2.75)), Value::Int(-2));
    assert_eq!(on("FloatToInt", Value::Float(f32::NAN)), Value::Int(0));
    assert_eq!(on("StrToInt", Value::string("  42abc")), Value::Int(42));
    assert_eq!(
        on("StrToInt", Value::string("-99999999999")),
        Value::Int(i32::MIN)
    );
    assert_eq!(on("StrToInt", Value::string("x")), Value::Int(0));
    assert_eq!(
        on("StrToFloat", Value::string(" 1.5e2xyz")),
        Value::Float(150.0)
    );
    assert_eq!(on("StrToBool", Value::string("TRUE")), Value::Bool(true));
    assert_eq!(on("StrToBool", Value::string("yes")), Value::Bool(false));
    assert_eq!(
        on("FloatToStr", Value::Float(1.5)),
        Value::string("1.500000")
    );
    assert_eq!(on("BoolToStr", Value::Bool(true)), Value::string("True"));
    assert_eq!(on("IntToByte", Value::Int(300)), Value::Byte(44));
    let named = on("StrToName", Value::string("Hello"));
    assert_eq!(r.text(&named), "'Hello'");
    // A code the C++ does not handle: a stub site that returns the value.
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.StrToVector",
            vec![Value::string("1,2,3")]
        ),
        Value::string("1,2,3")
    );
    assert_eq!(r.vm.log, ["UNIMPLEMENTED primitive cast 0x4d"]);
    // A hexadecimal float is not parsed here.
    let o = r.object("Core.Foo");
    let n = not_yet(r.call("Core.Foo.StrToFloat", Some(o), vec![Value::string("0x1p3")]));
    assert_eq!(n.missing, Missing::NotRead);
}

#[test]
fn dynamic_and_meta_casts() {
    let mut r = start(ParityMode::Off);
    let bar_obj = r.object("Core.Bar");
    let foo_obj = r.object("Core.Foo");
    let bar = r.vm.find_class("Core.Bar").unwrap();
    let foo = r.vm.find_class("Core.Foo").unwrap();
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.AsBar",
            vec![Value::Object(Some(bar_obj))]
        ),
        Value::Object(Some(bar_obj))
    );
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.AsBar",
            vec![Value::Object(Some(foo_obj))]
        ),
        Value::Object(None)
    );
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.AsBar", vec![Value::Object(None)]),
        Value::Object(None)
    );
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.MetaBar",
            vec![Value::Class(Some(bar))]
        ),
        Value::Class(Some(bar))
    );
    assert_eq!(
        r.on(
            "Core.Foo",
            "Core.Foo.MetaBar",
            vec![Value::Class(Some(foo))]
        ),
        Value::Class(None)
    );
}

#[test]
fn assert_logs_and_a_function_that_falls_off_its_end_returns_none() {
    let mut r = start(ParityMode::Off);
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.AssertFalse", vec![]),
        Value::Int(1)
    );
    assert_eq!(r.vm.log, ["Assertion failed at line 7"]);
    assert_eq!(r.on("Core.Foo", "Core.Foo.FallsOff", vec![]), Value::None);
}

#[test]
fn string_constants() {
    let mut r = start(ParityMode::Off);
    // A surrogate pair is its character; a lone surrogate is U+FFFD (decision 2026-11-15-07).
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.Unicode", vec![]),
        Value::string("H\u{1F600}\u{FFFD}!")
    );
    // An ANSI byte past ASCII is its Latin-1 character.
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.Ansi", vec![]),
        Value::string("c\u{e9}")
    );
}

#[test]
fn a_function_that_does_not_decode_says_why_on_every_call() {
    let mut r = start(ParityMode::Off);
    for _ in 0..2 {
        let o = r.object("Core.Foo");
        assert_eq!(
            script_message(r.call("Core.Foo.Broken", Some(o), vec![])),
            "cannot run Core.Foo.Broken: unknown opcode 0x5b"
        );
    }
}

#[test]
fn class_defaults_come_from_the_definition_store() {
    let mut r = start(ParityMode::Off);
    // No object: the frame runs against the owner's class default object (DefStore::class_default, W3-DEFS), and
    // Answer returns its constant 42 whatever object it runs on.
    let f = r.vm.find_function("Core.Foo.Answer").unwrap();
    assert_eq!(r.vm.call(f, None, vec![]).unwrap().ret, Value::Int(42));
    let o = r.object("Core.Foo");
    // A class constant (Util) and a call through it run GetLevel on Util's class default object, whose Level no
    // default tag sets: the declared zero.
    assert_eq!(
        r.call("Core.Foo.LevelThroughClass", Some(o), vec![])
            .unwrap()
            .ret,
        Value::Int(0)
    );
    // A virtual call in Foo's class context runs on Foo's class default object, so Foo.Get (1), not Bar.Get (2).
    assert_eq!(
        r.call("Core.Foo.StaticAnswer", Some(o), vec![])
            .unwrap()
            .ret,
        Value::Int(1)
    );
    // `new class'Mod'` makes a Mod from Mod's class default object; Off mode names it per class, so the first is
    // Mod_0.
    let made = r.call("Core.Foo.MakeMod", Some(o), vec![]).unwrap().ret;
    assert_eq!(r.text(&made), "Mod'Mod_0'");
}

/// Adds a `Core.Mod` modifier (`Type`, `Value`) to `o`'s stack property `stack`, as `vm_test.py`'s `AddSpeedMod` /
/// `AddCapMod` do through `New` and `AddItem` (W4-INTERP2's); adding one recomputes nothing.
fn add_modifier(r: &mut Run, o: ObjRef, stack: &str, kind: u8, amount: f32) {
    let mod_class = r.vm.find_class("Core.Mod").unwrap();
    let m = r.vm.instantiate(mod_class, "").unwrap();
    let ty = r.vm.space.prop(mod_class, "Type").unwrap();
    let value = r.vm.space.prop(mod_class, "Value").unwrap();
    r.vm.space
        .write(m, PropAddr::scalar(ty), Value::Byte(kind))
        .unwrap();
    r.vm.space
        .write(m, PropAddr::scalar(value), Value::Float(amount))
        .unwrap();
    let foo = r.vm.find_class("Core.Foo").unwrap();
    let at = PropAddr::scalar(r.vm.space.prop(foo, stack).unwrap());
    let mut items = r.vm.space.read(o, at).unwrap().clone();
    items.array_mut().unwrap().push(Value::Object(Some(m)));
    r.vm.space.write(o, at, items).unwrap();
}

fn read_on(r: &mut Run, o: ObjRef, f: &str) -> Value {
    r.call(&format!("Core.Foo.{f}"), Some(o), vec![])
        .unwrap()
        .ret
}

fn set_on(r: &mut Run, o: ObjRef, f: &str, v: Value) {
    r.call(&format!("Core.Foo.{f}"), Some(o), vec![v]).unwrap();
}

#[test]
fn let_attribute() {
    // The attributes block of tests/vm_test.py, with its expected values (the C++ rule interp.cpp:545-595): the 5F
    // converts the right side by the assignment rule (interp.cpp:511-522), stores it in the base and recomputes the
    // value from base and stack, value = (base + pre) * ((1 + scale up) / (1 - scale down)) + post, int attributes
    // truncated toward zero. Modifier Type 0 scale, 1 pre-add, 2 post-add.
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Foo");
    let notified = Rc::new(std::cell::Cell::new(0u32));
    let seen = notified.clone();
    r.vm.register_attribute_notify(
        "Core.Foo",
        Rc::new(move |_, _, _, _| {
            seen.set(seen.get() + 1);
            Ok(())
        }),
    )
    .unwrap();
    // 0x5E reads the value property.
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(0.0));
    // Value and base both set by the 5F: no stale base.
    set_on(&mut r, o, "SetSpeed", Value::Float(10.0));
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(10.0));
    assert_eq!(read_on(&mut r, o, "ReadSpeedBase"), Value::Float(10.0));
    // Adding modifiers recomputes nothing; only a later 5F does: (20 + 5) * 1.5 + 3. The stack never touches the base.
    for (kind, amount) in [(1, 5.0), (0, 0.5), (2, 3.0)] {
        add_modifier(&mut r, o, "SpeedModifierStack", kind, amount);
    }
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(10.0));
    set_on(&mut r, o, "SetSpeed", Value::Float(20.0));
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(40.5));
    assert_eq!(read_on(&mut r, o, "ReadSpeedBase"), Value::Float(20.0));
    // Scale -1 joins the denominator: (2 + 5) * (1.5 / 2) + 3, through Context(Self).
    add_modifier(&mut r, o, "SpeedModifierStack", 0, -1.0);
    set_on(&mut r, o, "SetSpeedSelf", Value::Float(2.0));
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(8.25));
    assert_eq!(read_on(&mut r, o, "ReadSpeedBase"), Value::Float(2.0));
    // An int right side is converted to the float base first (the assignment rule): (7 + 5) * (1.5 / 2) + 3.
    set_on(&mut r, o, "SetSpeedInt", Value::Int(7));
    assert_eq!(read_on(&mut r, o, "ReadSpeedBase"), Value::Float(7.0));
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(12.0));
    // Int attributes truncate toward zero: 10 * 1.25 + 0.9 = 13.4 -> 13; -12.5 + 0.9 = -11.6 -> -11, not -12.
    add_modifier(&mut r, o, "CapModifierStack", 0, 0.25);
    add_modifier(&mut r, o, "CapModifierStack", 2, 0.9);
    set_on(&mut r, o, "SetCap", Value::Int(10));
    assert_eq!(read_on(&mut r, o, "ReadCap"), Value::Int(13));
    assert_eq!(read_on(&mut r, o, "ReadCapBase"), Value::Int(10));
    set_on(&mut r, o, "SetCap", Value::Int(-10));
    assert_eq!(read_on(&mut r, o, "ReadCap"), Value::Int(-11));
    assert_eq!(read_on(&mut r, o, "ReadCapBase"), Value::Int(-10));
    // No change notification runs on a 5F (interp.cpp:566-569).
    assert_eq!(notified.get(), 0);
    // A right side the assignment rule leaves in another kind is refused by let_base and nothing is written. The C++
    // stores the string in the base silently; the refusal is ow-attr's (registry::let_base).
    let refused = r.call("Core.Foo.SetSpeedStr", Some(o), vec![Value::string("x")]);
    assert!(matches!(refused, Err(VmError::Attr(_))), "{refused:?}");
    assert_eq!(read_on(&mut r, o, "ReadSpeedBase"), Value::Float(7.0));
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(12.0));
    // A None context logs and still evaluates the right side; nothing changes.
    r.vm.log.clear();
    r.call(
        "Core.Foo.SetSpeedOn",
        Some(o),
        vec![Value::Float(4.0), Value::Object(None)],
    )
    .unwrap();
    assert_eq!(r.vm.log, ["Attempt to assign attribute through None"]);
    assert_eq!(read_on(&mut r, o, "ReadSpeed"), Value::Float(12.0));
}

#[test]
fn finding_functions_and_classes() {
    let mut r = start(ParityMode::Off);
    let f = r.vm.find_function("core.foo.answer").unwrap();
    assert_eq!(r.vm.find_function("Core.Foo.Answer").unwrap(), f);
    let foo = r.vm.find_class("Core.Foo").unwrap();
    let d = r.vm.function(f).unwrap();
    assert_eq!(d.path.as_str(), "Core.Foo.Answer");
    assert_eq!(d.owner, Some(foo));
    let add = r.vm.find_function("Core.Object.Add_IntInt").unwrap();
    assert_eq!(
        r.vm.function(add).unwrap().native_key.as_deref(),
        Some("Object.+(int,int)")
    );
    assert!(r.vm.function(add).unwrap().is_static);
    let goto = r.vm.find_function("Core.Object.GotoState").unwrap();
    assert_eq!(
        r.vm.function(goto).unwrap().native_key.as_deref(),
        Some("Object.GotoState(name,name,bool,bool)")
    );
    assert_eq!(
        script_message(r.vm.find_function("Core.Foo.Missing")),
        "function not found: Core.Foo.Missing"
    );
    assert_eq!(
        script_message(r.vm.find_function("Core.Foo.Count")),
        "not a function: Core.Foo.Count"
    );
    assert_eq!(
        script_message(r.vm.find_function("NoDot")),
        "function path must be Package.Class.Function: NoDot"
    );
    assert_eq!(
        script_message(r.vm.find_class("Core.Nope")),
        "class not found: Core.Nope"
    );
    // A state's function is owned by the class that owns the state.
    let mach = r.vm.find_class("Core.Mach").unwrap();
    let begin = r.vm.find_function("Core.Mach.A.BeginState").unwrap();
    assert_eq!(r.vm.function(begin).unwrap().owner, Some(mach));
}

#[test]
fn find_method_and_states_follow_the_loaded_classes() {
    let mut r = start(ParityMode::Off);
    let mach = r.vm.find_class("Core.Mach").unwrap();
    let a = r.vm.space.names.intern("a");
    let nope = r.vm.space.names.intern("Nope");
    assert!(r.vm.class_declares_state(mach, a).unwrap());
    assert!(!r.vm.class_declares_state(mach, nope).unwrap());
    let in_state = r.vm.find_method(mach, "WHERE", Some(a)).unwrap().unwrap();
    let plain = r.vm.find_method(mach, "Where", None).unwrap().unwrap();
    assert_eq!(
        r.vm.function(in_state).unwrap().path.as_str(),
        "Core.Mach.A.Where"
    );
    assert_eq!(
        r.vm.function(plain).unwrap().path.as_str(),
        "Core.Mach.Where"
    );
    // A state the chain does not declare falls back to the class's function.
    assert_eq!(
        r.vm.find_method(mach, "Where", Some(nope)).unwrap(),
        Some(plain)
    );
    assert_eq!(r.vm.find_method(mach, "Missing", None).unwrap(), None);
    // Inherited: Bar sees Foo's functions.
    let bar = r.vm.find_class("Core.Bar").unwrap();
    let answer = r.vm.find_function("Core.Foo.Answer").unwrap();
    assert_eq!(r.vm.find_method(bar, "answer", None).unwrap(), Some(answer));
}

#[test]
fn goto_state_under_reference_runs_the_state_events_of_loaded_states() {
    let mut r = start(ParityMode::Reference);
    let m = r.object("Core.Mach");
    assert_eq!(
        r.call("Core.Mach.CallWhere", Some(m), vec![]).unwrap().ret,
        Value::string("-")
    );
    r.call("Core.Mach.GoA", Some(m), vec![]).unwrap();
    assert_eq!(
        r.call("Core.Mach.ReadTrace", Some(m), vec![]).unwrap().ret,
        Value::string("bA;")
    );
    let arg = r
        .call("Core.Mach.ReadBeginArg", Some(m), vec![])
        .unwrap()
        .ret;
    assert_eq!(r.text(&arg), "'None'");
    // In state A the state's own Where runs; Global. ignores the state.
    assert_eq!(
        r.call("Core.Mach.CallWhere", Some(m), vec![]).unwrap().ret,
        Value::string("A")
    );
    assert_eq!(
        r.call("Core.Mach.GlobalWhere", Some(m), vec![])
            .unwrap()
            .ret,
        Value::string("-")
    );
    // An undeclared state only sets the name.
    r.call("Core.Mach.GoMissing", Some(m), vec![]).unwrap();
    assert_eq!(
        r.vm.log,
        ["GotoState: state Nope is not declared on Mach; name set, no state events"]
    );
    // Outside Reference, GotoState is the game's state machine (W5-STATES, `crate::states`), which reads the State
    // export's body. This fixture's state A has a 16-byte stand-in body, which does not read: a typed Decode error.
    // The state machine's own tests (`states.rs`) build real State bodies.
    let mut off = start(ParityMode::Off);
    let m = off.object("Core.Mach");
    let e = off.call("Core.Mach.GoA", Some(m), vec![]).unwrap_err();
    assert!(matches!(e, VmError::Decode(_)), "{e:?}");
}

#[test]
fn a_hand_declared_script_function_still_has_no_bytecode() {
    let mut r = start(ParityMode::Off);
    let f =
        r.vm.declare_function(crate::api::FunctionDecl::script(
            ow_core::ObjectPath::parse("Game.Thing.Run").unwrap(),
        ))
        .unwrap();
    assert_eq!(
        not_yet(r.vm.call(f, None, vec![])).missing,
        Missing::NotPorted
    );
}

#[test]
fn describe_prints_the_cxx_forms() {
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Bar");
    let bar = r.vm.find_class("Core.Bar").unwrap();
    let name = r.vm.space.names.intern("Thing");
    let cases = [
        (Value::None, "None"),
        (Value::Int(-3), "-3"),
        (Value::Byte(7), "7"),
        (Value::Bool(true), "true"),
        (Value::Float(0.1), "0.1"),
        (Value::Float(1234567.0), "1.23457e+06"),
        (Value::string("hi"), "\"hi\""),
        (Value::Name(name), "'Thing'"),
        (Value::Object(None), "None"),
        (Value::Class(Some(bar)), "Class'Bar'"),
        (Value::array(vec![Value::Int(1), Value::Int(2)]), "[1,2]"),
    ];
    for (v, want) in cases {
        assert_eq!(r.text(&v), want);
    }
    assert_eq!(r.text(&Value::Object(Some(o))), "Bar'Bar_0'");
}

#[test]
fn out_parameters_alias_the_callers_storage_live() {
    // The C++ values (review r1 of PR #314): AliasTwo(x, x) with x = 0 returns 1 and leaves x = 1.
    let mut r = start(ParityMode::Off);
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.AliasLocal", vec![]),
        Value::Int(1)
    );
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.AliasLocalAfter", vec![]),
        Value::Int(1)
    );
    // The same through an instance variable (a heap place).
    let o = r.object("Core.Foo");
    assert_eq!(
        r.call("Core.Foo.AliasCount", Some(o), vec![]).unwrap().ret,
        Value::Int(1)
    );
    let foo = r.vm.find_class("Core.Foo").unwrap();
    let count = PropAddr::scalar(r.vm.space.prop(foo, "Count").unwrap());
    assert_eq!(r.vm.space.read(o, count).unwrap(), &Value::Int(1));
}

#[test]
fn a_natives_out_write_is_seen_by_script_it_calls_back_into() {
    // The C++ writes a native's out argument through the caller's alias at once, so GetCount() called back from the
    // native sees 5.
    let mut r = start(ParityMode::Off);
    r.vm.register_native(
        "Foo.PokeAndPeek(int)",
        Rc::new(|c: &mut crate::api::NativeCall<'_>| {
            c.set_out(0, Value::Int(5))?;
            let get = c.vm.find_function("Core.Foo.GetCount")?;
            let this = c.this;
            Ok(c.call_script(get, this, vec![])?.ret)
        }),
    )
    .unwrap();
    let o = r.object("Core.Foo");
    assert_eq!(
        r.call("Core.Foo.CallPoke", Some(o), vec![]).unwrap().ret,
        Value::Int(5)
    );
}

#[test]
fn frames_past_the_bounds_are_refused_before_they_are_allocated() {
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Foo");
    let message = script_message(r.call("Core.Foo.BigFrame", Some(o), vec![]));
    assert!(
        message.contains("needs 65536 value slots") && message.contains("Rust-only refusal"),
        "{message}"
    );
    // All active frames together: 4,001 slots a frame meets the live bound before a raised depth limit.
    let outcome = super::on_vm_stack(|| {
        let mut r = start(ParityMode::Off);
        r.vm.limits.max_call_depth = 2_000;
        let o = r.object("Core.Foo");
        let message = script_message(r.call("Core.Foo.BigRecursion", Some(o), vec![]));
        (message, r.vm.code.frames.len(), r.vm.code.live_slots)
    })
    .unwrap();
    assert!(
        outcome
            .0
            .contains("for all active frames (a Rust-only refusal)"),
        "{}",
        outcome.0
    );
    assert_eq!((outcome.1, outcome.2), (0, 0), "every frame was popped");
}

#[test]
fn the_worst_case_nesting_fits_the_vm_stack() {
    // 200 frames, each a statement nesting 252 levels eval -> native -> call: the depth limit stops it, on a thread of
    // STACK_SIZE bytes (a debug build included), and the nesting counter unwinds.
    let outcome = super::on_vm_stack(|| {
        let mut r = start(ParityMode::Off);
        let o = r.object("Core.Foo");
        let message = script_message(r.call("Core.Foo.Deep", Some(o), vec![]));
        (message, r.vm.code.nesting)
    })
    .unwrap();
    let deep = "Core.Foo.Deep";
    assert_eq!(
        outcome.0,
        format!("call depth limit reached in {deep} <- {deep} <- {deep} <- {deep} <- {deep}")
    );
    assert_eq!(outcome.1, 0);
    // With the depth limit raised, the nesting bound stops it first, with a typed error.
    let outcome = super::on_vm_stack(|| {
        let mut r = start(ParityMode::Off);
        r.vm.limits.max_call_depth = 250;
        let o = r.object("Core.Foo");
        let message = script_message(r.call("Core.Foo.Deep", Some(o), vec![]));
        (message, r.vm.code.nesting, r.vm.code.frames.len())
    })
    .unwrap();
    assert!(
        outcome
            .0
            .contains("expressions nest deeper than 51400 levels"),
        "{}",
        outcome.0
    );
    assert_eq!((outcome.1, outcome.2), (0, 0));
}

#[test]
fn goto_label_steps_over_and_a_state_variable_reads_as_an_instance_variable() {
    // A GotoLabel in a function: stepped over under Reference (the C++); NotYet otherwise (W5-STATES, decision
    // 2026-11-15-27: where the game resumes is not read, and no installed function has one).
    let mut reference = start(ParityMode::Reference);
    assert_eq!(
        reference.on("Core.Foo", "Core.Foo.StepsOverGotoLabel", vec![]),
        Value::Int(1)
    );
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Foo");
    assert_eq!(
        not_yet(r.call("Core.Foo.StepsOverGotoLabel", Some(o), vec![])).missing,
        Missing::NotRead
    );
    let o = r.object("Core.Foo");
    let foo = r.vm.find_class("Core.Foo").unwrap();
    let count = PropAddr::scalar(r.vm.space.prop(foo, "Count").unwrap());
    r.vm.space.write(o, count, Value::Int(4)).unwrap();
    assert_eq!(
        r.call("Core.Foo.ReadStateVariable", Some(o), vec![])
            .unwrap()
            .ret,
        Value::Int(4)
    );
}

#[test]
fn heap_typing_of_classes_and_stand_in_objects() {
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Foo");
    let foo = r.vm.find_class("Core.Foo").unwrap();
    // A class into an object property declared Core.Object: stored and read back.
    r.call("Core.Foo.SetOwnerClass", Some(o), vec![]).unwrap();
    assert_eq!(
        r.call("Core.Foo.ReadOwner", Some(o), vec![]).unwrap().ret,
        Value::Class(Some(foo))
    );
    // A Foo object into a property declared Bar: the heap's WrongClass.
    let other = r.object("Core.Foo");
    let refused = r.call(
        "Core.Foo.SetBarRef",
        Some(o),
        vec![Value::Object(Some(other))],
    );
    assert!(matches!(refused, Err(VmError::Heap(_))), "{refused:?}");
    // An object of an engine-native stand-in class: its chain is unknown, so NotYet.
    let object = r.vm.find_class("Core.Object").unwrap();
    let native =
        r.vm.space
            .classes
            .define("Engine.StandIn", Some(object), vec![])
            .unwrap();
    r.vm.space.classes.mark_native(native).unwrap();
    let stand_in = r.vm.instantiate(native, "S").unwrap();
    let n = not_yet(r.call(
        "Core.Foo.SetBarRef",
        Some(o),
        vec![Value::Object(Some(stand_in))],
    ));
    assert_eq!(n.missing, Missing::NotRead);
}

#[test]
fn find_method_loads_a_class_whose_code_is_not_loaded() {
    // Mach's layout is defined straight through the code index, so its functions and states are not loaded.
    let mut r = start(ParityMode::Reference);
    let mach = {
        let (code, store) = r.vm.defs.code_mut();
        let key = code.find_class(store, "Core.Mach").unwrap();
        code.define_layout(store, &mut r.vm.space.names, &mut r.vm.space.classes, key)
            .unwrap()
    };
    assert!(!r.vm.code.classes.contains_key(&mach));
    assert!(r.vm.find_method(mach, "Where", None).unwrap().is_some());
    // GotoState called from the host on an object of a class nobody loaded: the state events still run.
    let mut r = start(ParityMode::Reference);
    let mach = {
        let (code, store) = r.vm.defs.code_mut();
        let key = code.find_class(store, "Core.Mach").unwrap();
        code.define_layout(store, &mut r.vm.space.names, &mut r.vm.space.classes, key)
            .unwrap()
    };
    let m = r.vm.instantiate(mach, "").unwrap();
    let goto = r.vm.find_function("Core.Object.GotoState").unwrap();
    let a = r.vm.space.names.intern("A");
    r.vm.call(goto, Some(m), vec![Value::Name(a)]).unwrap();
    assert_eq!(
        r.call("Core.Mach.ReadTrace", Some(m), vec![]).unwrap().ret,
        Value::string("bA;")
    );
}

#[test]
fn a_function_that_cannot_run_says_so_before_the_depth_check() {
    // The C++ `invoke` decodes before it checks the depth.
    let mut r = start(ParityMode::Off);
    r.vm.limits.max_call_depth = 0;
    let o = r.object("Core.Foo");
    assert_eq!(
        script_message(r.call("Core.Foo.Broken", Some(o), vec![])),
        "cannot run Core.Foo.Broken: unknown opcode 0x5b"
    );
    let message = script_message(r.call("Core.Foo.Answer", Some(o), vec![]));
    assert_eq!(message, "call depth limit reached in Core.Foo.Answer");
}

#[test]
fn let_attribute_on_an_object_without_the_property() {
    // The companions are found first (from the declaring class); the object must hold the property.
    let mut r = start(ParityMode::Off);
    let o = r.object("Core.Foo");
    let mach = r.object("Core.Mach");
    let message = script_message(r.call(
        "Core.Foo.SetSpeedOn",
        Some(o),
        vec![Value::Float(1.0), Value::Object(Some(mach))],
    ));
    assert_eq!(
        message,
        "let attribute: Speed is missing on its object in Core.Foo.SetSpeedOn"
    );
}

#[test]
fn a_natives_out_read_goes_through_the_live_alias() {
    // X = 1; X += ++X: the C++ gives 4 (`++X` writes 2 through the alias, `+=` reads 2 + 2).
    let mut r = start(ParityMode::Off);
    assert_eq!(r.on("Core.Foo", "Core.Foo.IncTwice", vec![]), Value::Int(4));
    // A native given one variable for two out parameters sees a write through the other: 7.
    r.vm.register_native(
        "Foo.TwoOut(int,int)",
        Rc::new(|c: &mut crate::api::NativeCall<'_>| {
            c.set_out(0, Value::Int(7))?;
            Ok(c.arg(1)?.clone())
        }),
    )
    .unwrap();
    assert_eq!(
        r.on("Core.Foo", "Core.Foo.CallTwoOut", vec![]),
        Value::Int(7)
    );
}

#[test]
fn an_alias_kept_past_its_frame_is_a_stale_frame_error() {
    use crate::api::{Arg, NativeCall};
    use std::cell::RefCell;
    let mut r = start(ParityMode::Off);
    let kept: Rc<RefCell<Option<Vec<Arg>>>> = Rc::new(RefCell::new(None));
    let refused: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (k, seen) = (kept.clone(), refused.clone());
    r.vm.register_native(
        "Foo.Keep(int)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            let earlier = k.borrow_mut().take();
            match earlier {
                // First call: keep a copy of the arguments (the alias names CallKeep's X).
                None => *k.borrow_mut() = Some(c.args.clone()),
                // Second call: write through the copy. Its frame returned; CallKeep's new frame holds the same index.
                Some(args) => {
                    let function = c.function;
                    let mut old = NativeCall::new(c.vm, function, None, args);
                    for e in [
                        old.set_out(0, Value::Int(99)).unwrap_err(),
                        old.arg(0).unwrap_err(),
                    ] {
                        seen.borrow_mut().push(script_message(Err::<(), _>(e)));
                    }
                }
            }
            Ok(Value::Int(0))
        }),
    )
    .unwrap();
    let o = r.object("Core.Foo");
    assert_eq!(
        r.call("Core.Foo.CallKeep", Some(o), vec![]).unwrap().ret,
        Value::Int(0)
    );
    // After the call returned: the kept alias names a frame no longer on the stack.
    {
        let args = kept.borrow().clone().unwrap();
        let keep = r.vm.find_function("Core.Foo.Keep").unwrap();
        let mut late = NativeCall::new(&mut r.vm, keep, None, args.clone());
        let e = late.set_out(0, Value::Int(5)).unwrap_err();
        assert!(script_message(Err::<(), _>(e)).starts_with("stale frame"));
        *kept.borrow_mut() = Some(args);
    }
    // A reused index: the second CallKeep's frame sits where the first one did. The kept copy is refused, and the new
    // frame's X is not written (CallKeep returns 0, not 99).
    assert_eq!(
        r.call("Core.Foo.CallKeep", Some(o), vec![]).unwrap().ret,
        Value::Int(0)
    );
    let refused = refused.borrow();
    assert_eq!(refused.len(), 2);
    assert!(
        refused.iter().all(|m| m.starts_with("stale frame")),
        "{refused:?}"
    );
}

#[test]
fn a_decode_at_depth_fits_the_vm_stack() {
    // DeepN(198): 199 frames of 252 levels each, then Leaf, decoded first at the 200th frame, nesting 254 levels.
    let outcome = super::on_vm_stack(|| {
        let mut r = start(ParityMode::Off);
        let o = r.object("Core.Foo");
        let ret = r
            .call("Core.Foo.DeepN", Some(o), vec![Value::Int(198)])
            .map(|c| c.ret == Value::Int(1))
            .map_err(|e| e.to_string());
        (ret, r.vm.code.nesting)
    })
    .unwrap();
    assert_eq!(outcome, (Ok(true), 0));
}

#[test]
fn a_panic_on_the_vm_thread_is_handed_back() {
    let caught = super::on_vm_stack(|| -> u32 { panic!("a VM thread panic, on purpose") });
    assert!(matches!(caught, Err(super::VmThreadError::Panicked)));
    assert_eq!(super::on_vm_stack(|| 7).unwrap(), 7);
}

/// `AttrWorld::script_context` on the VM (lane FIX-DESIGNER-RESOLVER, decision 2026-11-16-62): the resolver class's
/// script `GetAttributeContext(Attribute, AttributeContextSource)` runs with the two by parameter name and its object
/// is the answer (invented classes: `Echo` returns the source, `Mute` returns None, `Count` an int, `Native` declares
/// the function native, `Plain` declares none). The answer is the same in both modes: the script is the rule.
#[test]
fn a_script_context_resolver_runs_its_get_attribute_context() {
    use ow_attr::api::{AttrError, AttrWorld};
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty", "IntProperty"]);
    let object = t.class("Object", 0);
    let typed = |name: &str| Param {
        type_ref: object,
        ..param("Object", name, 0)
    };
    let params = [typed("Attribute"), typed("AttributeContextSource")];
    let echo = t.class("Echo", object);
    script(&mut t, echo, "GetAttributeContext", &params, Some(("Object", object)), &[], |ids, a| {
        returns(a, |a| {
            a.local(ids.get("AttributeContextSource"));
        })
    });
    let mute = t.class("Mute", object);
    script(&mut t, mute, "GetAttributeContext", &params, Some(("Object", object)), &[], |_, a| {
        returns(a, |a| {
            a.raw(&[0x2A]);
        })
    });
    let count = t.class("Count", object);
    script(&mut t, count, "GetAttributeContext", &params, Some(("Int", 0)), &[], |_, a| {
        returns(a, |a| {
            a.int_const(7);
        })
    });
    let native_class = t.class("Native", object);
    let mut s = spec(FUNC_NATIVE | FUNC_PUBLIC, "GetAttributeContext");
    s.native = 0;
    t.function(native_class, "GetAttributeContext", &params, Some(("Object", object)), &[], &s, |_| None);
    t.class("Plain", object);
    let bytes = t.build();
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut r = run_on(&bytes, mode);
        let vm = &mut r.vm;
        let make = |vm: &mut Vm, path: &str| {
            let c = vm.find_class(path).unwrap();
            vm.instantiate(c, "").unwrap()
        };
        let (attr, source) = (make(vm, "Core.Plain"), make(vm, "Core.Plain"));
        let echo = make(vm, "Core.Echo");
        assert_eq!(vm.script_context(echo, attr, Some(source)).unwrap(), Some(source), "{mode:?}");
        assert_eq!(vm.script_context(echo, attr, None).unwrap(), None, "{mode:?}: a None source passes None");
        let mute = make(vm, "Core.Mute");
        assert_eq!(vm.script_context(mute, attr, Some(source)).unwrap(), None, "{mode:?}");
        let count = make(vm, "Core.Count");
        assert!(
            matches!(vm.script_context(count, attr, Some(source)), Err(AttrError::BadData { .. })),
            "{mode:?}"
        );
        for path in ["Core.Native", "Core.Plain"] {
            let o = make(vm, path);
            match vm.script_context(o, attr, Some(source)) {
                Err(AttrError::NotYet(n)) => {
                    assert_eq!(n.missing, Missing::NotRead, "{mode:?} {path}");
                    assert!(n.what.contains("no script function to run"), "{}", n.what);
                }
                other => panic!("{mode:?} {path}: {other:?}"),
            }
        }
    }
}
