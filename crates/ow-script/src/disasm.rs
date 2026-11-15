//! Owner: lane W2-SCRIPT. Readable pseudo-code for an expression and for a whole function: the port of `format`,
//! `formatExpr` and `disassemble` of `src/script.cpp`, which `--disasm` prints.
//!
//! The text is bytes, not `String`: string constants are printed as stored (through the C++ `quote()` escapes), and
//! an ANSI constant may hold bytes that are not UTF-8. Floats print as the C++ `std::ostream << float` does (`%g`,
//! precision 6) through `ow_core::fmt`.

use std::io::Write as _;

use ow_core::fmt::{fmt_ostream, json_quote_bytes};
use ow_package::package::{Package, PackageError};

use crate::decode::{Code, Expr};
use crate::function::FunctionInfo;
use crate::ops::{self, op};

/// A reference as text: `None` for 0, the object's path, or `<bad ref N>` when the path cannot be built (C++
/// `qualifiedName`).
fn qualified_name(package: &Package, reference: i32) -> String {
    if reference == 0 {
        return "None".to_string();
    }
    match package.path_of(reference) {
        Ok(path) => path,
        Err(_) => format!("<bad ref {reference}>"),
    }
}

fn format_list(package: &Package, list: &[Expr], out: &mut Vec<u8>) {
    for (i, e) in list.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(b", ");
        }
        format_expr(package, e, out);
    }
}

fn format_expr(package: &Package, e: &Expr, out: &mut Vec<u8>) {
    if e.is_native_call() {
        let _ = write!(out, "native_{}(", e.native);
        format_list(package, &e.kids, out);
        out.push(b')');
        return;
    }
    if e.op == op::STRING_CONST || e.op == op::UNICODE_STRING_CONST {
        out.extend_from_slice(&json_quote_bytes(&e.text));
        return;
    }
    if e.op == op::LABEL_TABLE {
        out.extend_from_slice(b"LabelTable[");
        for (i, (name, offset)) in e.labels.iter().enumerate() {
            if i > 0 {
                out.extend_from_slice(b", ");
            }
            let _ = write!(out, "{name}@{offset}");
        }
        out.push(b']');
        return;
    }
    let name = match e.op {
        op::CASE => "Case",
        op::CONDITIONAL => "Conditional",
        code => ops::layout_for(code).map_or("?", |l| l.name),
    };
    out.extend_from_slice(name.as_bytes());
    let mut parts: Vec<String> = Vec::new();
    parts.extend(e.refs.iter().map(|&r| qualified_name(package, r)));
    parts.extend(e.names.iter().cloned());
    parts.extend(e.ints.iter().map(|v| v.to_string()));
    parts.extend(e.floats.iter().map(|&v| fmt_ostream(f64::from(v))));
    parts.extend(e.bytes.iter().map(|v| v.to_string()));
    parts.extend(e.words.iter().map(|v| v.to_string()));
    if parts.is_empty() && e.kids.is_empty() {
        return;
    }
    out.push(b'(');
    let mut first = true;
    for part in &parts {
        if !first {
            out.extend_from_slice(b", ");
        }
        out.extend_from_slice(part.as_bytes());
        first = false;
    }
    for kid in &e.kids {
        if !first {
            out.extend_from_slice(b", ");
        }
        format_expr(package, kid, out);
        first = false;
    }
    out.push(b')');
}

/// One expression as pseudo-code (C++ `script::format`).
pub fn format(package: &Package, expr: &Expr) -> Vec<u8> {
    let mut out = Vec::new();
    format_expr(package, expr, &mut out);
    out
}

/// A whole function: a `// <path>  flags 0x<hex>` line, then one `<file offset>: <statement>` line per statement,
/// each ending in `\n` (C++ `script::disassemble`). Fails only when the function's own path cannot be built.
pub fn disassemble(package: &Package, function: &FunctionInfo, code: &Code) -> Result<Vec<u8>, PackageError> {
    let mut out = Vec::new();
    let _ = writeln!(out, "// {}  flags 0x{:x}", package.path_of(function.index)?, function.flags);
    for statement in &code.statements {
        let _ = write!(out, "{}: ", statement.file);
        format_expr(package, statement, &mut out);
        out.push(b'\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::decode;
    use crate::decode::test_support::{add, load, toy};
    use crate::function::read_function;
    use ow_package::testkit::script::{Asm, DEFAULT_FLAGS, FunctionSpec};

    fn text(package: &Package, index: i32) -> String {
        let info = read_function(package, index).unwrap();
        let code = decode(package, &info).unwrap();
        String::from_utf8(disassemble(package, &info, &code).unwrap()).unwrap()
    }

    #[test]
    fn a_whole_function() {
        let mut t = toy();
        let count_name = t.p.name("Count");
        let mut a = Asm::new();
        // Let(A, IntConst(7)); Context(Self, 0, None, 0, InstanceVariable(Count)); FloatConsts; VectorConst;
        // NameConst(Count_2); ObjectConst(None); FinalFunction(F, A, ...); Return(Nothing); EndOfScript.
        a.raw(&[0x0F]).local(4).int_const(7);
        a.context(|a| { a.self_(); }, |a| { a.instance(2); });
        a.raw(&[0x04]).float_const(1.5);
        a.raw(&[0x04]).float_const(1e-7);
        a.raw(&[0x04]).float_const(1e8);
        a.raw(&[0x23]).f32(1.0).f32(2.5).f32(-0.0);
        a.raw(&[0x21]).i32(count_name).i32(3);
        a.raw(&[0x20]).reference(0);
        a.final_function(5, |a| { a.local(4).byte_const(200); });
        a.return_nothing().end();
        let index = add(&mut t, "F", Some(a), &FunctionSpec::new(DEFAULT_FLAGS, "F"));
        let package = load(&mut t);
        assert_eq!(text(&package, index), concat!(
            "// Thing.F  flags 0x20003\n",
            "0: Let(LocalVariable(Thing.F.A), IntConst(7))\n",
            "11: Context(None, 0, 0, Self, InstanceVariable(Thing.Count))\n",
            "25: Return(FloatConst(1.5))\n",
            "31: Return(FloatConst(1e-07))\n",
            "37: Return(FloatConst(1e+08))\n",
            "43: VectorConst(1, 2.5, -0)\n",
            "56: NameConst(Count_2)\n",
            "65: ObjectConst(None)\n",
            "70: FinalFunction(Thing.F, LocalVariable(Thing.F.A), ByteConst(200))\n",
            "83: Return(Nothing)\n",
            "85: EndOfScript\n",
        ));
    }

    #[test]
    fn a_reference_without_a_path_prints_as_bad() {
        let mut t = toy();
        let mut a = Asm::new();
        a.raw(&[0x20]).reference(2).end();
        let index = add(&mut t, "F", Some(a), &FunctionSpec::new(DEFAULT_FLAGS, "F"));
        t.p.set_outer(2, 2); // Count is its own outer: a cycle
        let package = load(&mut t);
        assert_eq!(text(&package, index), "// Thing.F  flags 0x20003\n0: ObjectConst(<bad ref 2>)\n5: EndOfScript\n");
        // The function's own path is not optional: a cycle there fails the disassembly.
        t.p.set_outer(index, index);
        let package = load(&mut t);
        let info = read_function(&package, index).unwrap();
        let code = decode(&package, &info).unwrap();
        assert_eq!(disassemble(&package, &info, &code), Err(PackageError::OuterChain));
    }
}
