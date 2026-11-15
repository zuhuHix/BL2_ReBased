//! Owner: lane W3-INTERP. Commands: `--run`, `--run-batch`, `--native`: the ports of their branches in `src/cli.cpp`.
//! AI-assisted (Claude Code).
//!
//! - `--native <name> [--native-args <args>]` is the C++ stub dispatch (`NativeRegistry`, `src/natives.cpp`): it
//!   loads no package and its table ships empty, so every name prints its `UNIMPLEMENTED <name>(<args>)` line with
//!   status `unimplemented` (architecture 3.8: `StubPolicy::LogAndZero`, pinned by `natives_test.py:16-20`).
//! - `<package> --run <Package.Class.Function> --cooked <dir> [--self <Package.Class>] [--arg <kind:value>]...` runs
//!   one script function on a VM over the cooked folder and prints `{"function","result","error","steps","args",
//!   "log"}` with the values in the C++ `Value::describe` form (`ow_vm::interp::describe`).
//! - `<package> --run-batch <file> --cooked <dir>` runs one case per line (`function TAB self-class TAB name=kind:value
//!   ...`, text hex-encoded) and prints one JSON object per case, as `tools/replay_trace.py` reads them.
//!
//! Both run commands load the source package first (as the C++ does, before the options are read), use
//! `StubPolicy::LogAndZero` and the Core natives, and run the VM on a thread of `ow_vm::interp::STACK_SIZE` bytes
//! (`ow_vm::interp::on_vm_stack`)
//! (`ow_vm::interp`, "Stack depth"). `mode` is the run's parity mode, read once by `main`; it goes to the definition
//! store and the VM. Error texts are the C++ ones where the C++ has one (a `std::stoll` / `std::stod` failure
//! included); a `NotYet` prints its own text. Lines end in `\n`; the parity harness reads the C++ `\r\n` as `\n`
//! (decision 2026-11-15-09).
//!
//! Until the W3-DEFS body lands, `DefStore::class_default` is `NotYet`, so `--self`, a batch case with a self class,
//! and any call that needs a class default object (a call with no `--self` runs against the owner's) print that
//! `NotYet`. The differences from the C++ output that stay, and why, are in decision 2026-11-15-14.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::fmt::fmt_g;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::package::Package;
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_vm::interp::{describe, instantiate_from_defaults, on_vm_stack};

use crate::args::{Command, USAGE};
use crate::cmd::properties::error_text;
use crate::cmd::tables::signed_number;
use crate::json::quote;

/// What a command printed and how it ended.
struct Outcome {
    stdout: String,
    /// The message printed after `ow-package: ` on stderr.
    error: Option<String>,
    code: u8,
}

impl Outcome {
    fn failed(stdout: String, message: String) -> Outcome {
        Outcome {
            stdout,
            error: Some(message),
            code: 1,
        }
    }
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let outcome = match command.flag {
        "--native" => native(args),
        flag => {
            let ran = on_vm_stack(|| match flag {
                "--run" => run_one(args, mode),
                _ => run_batch(args, mode),
            });
            ran.unwrap_or_else(|e| {
                Outcome::failed(String::new(), format!("cannot start the VM thread: {e}"))
            })
        }
    };
    // Ignore a failed write (a closed pipe), as the C++ binary does.
    let _ = std::io::stdout()
        .lock()
        .write_all(outcome.stdout.as_bytes());
    if let Some(message) = &outcome.error {
        let _ = writeln!(std::io::stderr().lock(), "ow-package: {message}");
    }
    ExitCode::from(outcome.code)
}

fn usage() -> String {
    USAGE.to_string()
}

// ------------------------------------------------------------------------------------------------ --native

/// `--native <name> [--native-args <args>]` (`args[0]` is `--native`).
fn native(args: &[String]) -> Outcome {
    let printed = (|| -> Result<String, String> {
        if args.len() < 2 || args.len() > 4 {
            return Err(usage());
        }
        let name = &args[1];
        let mut arguments = String::new();
        if args.len() > 2 {
            if args[2] != "--native-args" {
                return Err(usage());
            }
            arguments = args
                .get(3)
                .ok_or_else(|| "missing value for --native-args".to_string())?
                .clone();
            if args.len() != 4 {
                return Err(usage());
            }
        }
        if name.is_empty() {
            return Err(usage());
        }
        let log = format!("UNIMPLEMENTED {name}({arguments})");
        Ok(format!(
            "{{\"name\":{},\"status\":\"unimplemented\",\"log\":{}}}\n",
            quote(name),
            quote(&log)
        ))
    })();
    match printed {
        Ok(stdout) => Outcome {
            stdout,
            error: None,
            code: 0,
        },
        Err(message) => Outcome::failed(String::new(), message),
    }
}

// ------------------------------------------------------------------------------------------------ the VM

/// A VM over the cooked folder `cooked`, with the Core natives and the exploration stub policy.
fn open_vm(cooked: &Path, mode: ParityMode) -> Result<Vm, String> {
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)
        .map_err(|e| error_text(&e))?;
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    )
    .with_stub_policy(StubPolicy::LogAndZero);
    ow_vm::natives_core::register(&mut vm).map_err(|e| vm_text(&e))?;
    Ok(vm)
}

/// A VM error as the C++ `what()` reads: a script error's own message (the C++ `RuntimeError` text), anything else
/// its text and causes.
fn vm_text(e: &VmError) -> String {
    match e {
        VmError::Script { message, .. } => message.clone(),
        VmError::Def(e) => error_text(e),
        other => error_text(other),
    }
}

/// The C++ `nextValue`: the argument after `args[*at]`.
fn next_value(args: &[String], at: &mut usize, option: &str) -> Result<String, String> {
    *at += 1;
    args.get(*at)
        .cloned()
        .ok_or_else(|| format!("missing value for {option}"))
}

/// `std::stoll(text, &used)` (base 10): the value and the characters used. The MSVC exception texts.
fn stoll(text: &str) -> Result<(i64, usize), String> {
    let b = text.as_bytes();
    let mut at = 0;
    while at < b.len() && matches!(b[at], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        at += 1;
    }
    let start = at;
    if at < b.len() && (b[at] == b'+' || b[at] == b'-') {
        at += 1;
    }
    let digits = at;
    while at < b.len() && b[at].is_ascii_digit() {
        at += 1;
    }
    if at == digits {
        return Err("invalid stoll argument".into());
    }
    let value = text[start..at]
        .parse::<i64>()
        .map_err(|_| "stoll argument out of range".to_string())?;
    Ok((value, at))
}

// The C `strtod` prefix scanner, as `ow_vm::interp::convert` has it for the script casts (crate-private there,
// so the command-line parser keeps its own copy).

/// C `isspace` in the C locale.
fn is_c_space(c: u8) -> bool {
    c == b' ' || (0x09..=0x0d).contains(&c)
}

/// The length of the longest prefix of `s` (after white space) that C `strtod` reads as a decimal number, `inf`,
/// `infinity` or `nan` (optionally `nan(...)`), and whether it is a hexadecimal float. `None` when nothing converts.
fn strtod_prefix(s: &str) -> Option<(usize, usize, bool)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && is_c_space(b[i]) {
        i += 1;
    }
    let start = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let rest = &s[i..];
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("infinity") {
        return Some((start, i + 8, false));
    }
    if lower.starts_with("inf") {
        return Some((start, i + 3, false));
    }
    if lower.starts_with("nan") {
        let mut end = i + 3;
        if b.get(end) == Some(&b'(')
            && let Some(close) = b[end..].iter().position(|&c| c == b')')
            && b[end + 1..end + close]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            end += close + 1;
        }
        return Some((start, end, false));
    }
    if lower.starts_with("0x")
        && (lower
            .as_bytes()
            .get(2)
            .is_some_and(|c| c.is_ascii_hexdigit())
            || (lower.as_bytes().get(2) == Some(&b'.')
                && lower
                    .as_bytes()
                    .get(3)
                    .is_some_and(|c| c.is_ascii_hexdigit())))
    {
        return Some((start, i, true));
    }
    let digits_from = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut mantissa = i - digits_from;
    if i < b.len() && b[i] == b'.' {
        let dot = i;
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        mantissa += i - dot - 1;
        if mantissa == 0 {
            return None;
        }
    }
    if mantissa == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let exp_from = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_from {
            i = j;
        }
    }
    Some((start, i, false))
}

/// `std::stod(text, &used)`: decimal text, `inf` and `nan` ([`strtod_prefix`]); an overflow to an
/// infinity is the MSVC `out_of_range`. A hexadecimal float is refused here (no parser for it is written).
fn stod(text: &str) -> Result<(f64, usize), String> {
    let Some((start, end, hex)) = strtod_prefix(text) else {
        return Err("invalid stod argument".into());
    };
    if hex {
        return Err(format!(
            "hexadecimal float text is not read by the Rust binary: {text}"
        ));
    }
    let lower = text[start..end].to_ascii_lowercase();
    let unsigned = lower.trim_start_matches(['+', '-']);
    let value = if unsigned.starts_with("inf") {
        f64::INFINITY
    } else if unsigned.starts_with("nan") {
        f64::NAN
    } else {
        let v = unsigned
            .parse::<f64>()
            .map_err(|_| "invalid stod argument".to_string())?;
        if v.is_infinite() {
            return Err("stod argument out of range".into());
        }
        v
    };
    Ok((
        if lower.starts_with('-') {
            -value
        } else {
            value
        },
        end,
    ))
}

/// The C++ `split` of `--arg` lists: the pieces between commas (an empty text is one empty piece).
fn split(list: &str) -> Vec<String> {
    list.split(',').map(String::from).collect()
}

// ------------------------------------------------------------------------------------------------ --run

/// A struct `--arg` converted once the function is found (`v:`, `r:`, `st:`).
struct PendingStruct {
    slot: usize,
    ty: String,
    fields: Vec<String>,
}

fn run_one(args: &[String], mode: ParityMode) -> Outcome {
    let mut out = String::new();
    match run_one_inner(args, mode, &mut out) {
        Ok(code) => Outcome {
            stdout: out,
            error: None,
            code,
        },
        Err(message) => Outcome::failed(out, message),
    }
}

fn run_one_inner(args: &[String], mode: ParityMode, out: &mut String) -> Result<u8, String> {
    Package::load(Path::new(&args[0])).map_err(|e| error_text(&e))?;
    let (mut function_path, mut self_class) = (String::new(), String::new());
    let mut cooked = PathBuf::new();
    let mut values: Vec<Value> = Vec::new();
    let mut pending: Vec<PendingStruct> = Vec::new();
    let mut i = 2;
    while i < args.len() {
        let option = args[i].as_str();
        match option {
            "--run" | "--disasm" => {}
            "--cooked" => cooked = PathBuf::from(next_value(args, &mut i, "--cooked")?),
            "--self" => self_class = next_value(args, &mut i, "--self")?,
            "--arg" => {
                let text = next_value(args, &mut i, "--arg")?;
                let (kind, body) = text.split_once(':').ok_or_else(usage)?;
                match kind {
                    "i" => values.push(Value::Int(stoll(body)?.0 as i32)),
                    "f" => values.push(Value::Float(stod(body)?.0 as f32)),
                    "b" => values.push(Value::Bool(body == "true" || body == "1")),
                    "y" => values.push(Value::Byte(stoll(body)?.0 as u8)),
                    "s" => values.push(Value::string(body)),
                    "n" => values.push(Value::Str(body.into())), // made a name once the VM has its name table
                    "as" | "ai" | "af" => {
                        let mut items = Vec::new();
                        for part in split(body) {
                            items.push(match kind {
                                "as" => Value::string(&part),
                                "ai" => Value::Int(stoll(&part)?.0 as i32),
                                _ => Value::Float(stod(&part)?.0 as f32),
                            });
                        }
                        values.push(Value::array(items));
                    }
                    "v" | "r" => {
                        let parts = split(body);
                        if parts.len() != 3 {
                            return Err(usage());
                        }
                        let ty = if kind == "v" { "Vector" } else { "Rotator" };
                        pending.push(PendingStruct {
                            slot: values.len(),
                            ty: ty.into(),
                            fields: parts,
                        });
                        values.push(Value::None);
                    }
                    "st" => {
                        let (ty, rest) = body.split_once(':').ok_or_else(usage)?;
                        pending.push(PendingStruct {
                            slot: values.len(),
                            ty: ty.into(),
                            fields: split(rest),
                        });
                        values.push(Value::None);
                    }
                    _ => return Err(usage()),
                }
            }
            _ if function_path.is_empty() => function_path = option.to_string(),
            _ => return Err(usage()),
        }
        i += 1;
    }
    if function_path.is_empty() || cooked.as_os_str().is_empty() {
        return Err(usage());
    }
    let mut vm = open_vm(&cooked, mode)?;
    let this = if self_class.is_empty() {
        None
    } else {
        let class = vm.find_class(&self_class).map_err(|e| vm_text(&e))?;
        Some(instantiate_from_defaults(&mut vm, class).map_err(|e| vm_text(&e))?)
    };
    let function = vm.find_function(&function_path).map_err(|e| vm_text(&e))?;
    // `n:` values become names now that the VM's name table exists.
    for (at, kind) in args_kinds(args) {
        if kind == "n"
            && let Some(Value::Str(s)) = values.get(at)
        {
            let name = vm.space.names.intern(&s.clone());
            values[at] = Value::Name(name);
        }
    }
    for p in &pending {
        values[p.slot] = make_struct(&mut vm, p)?;
    }
    let path = vm
        .function(function)
        .map(|d| d.path.as_str().to_string())
        .unwrap_or_default();
    let params = vm.function(function).map_or(0, |d| d.params.len());
    let called = if values.len() > params {
        Err(format!("too many arguments for {path}"))
    } else {
        vm.call(function, this, values).map_err(|e| vm_text(&e))
    };
    let (result, error, outs) = match &called {
        Ok(r) => (Some(describe(&vm, &r.ret)), None, r.outs.clone()),
        Err(e) => (None, Some(e.clone()), Vec::new()),
    };
    out.push_str(&format!(
        "{{\"function\":{},\"result\":{},\"error\":{},\"steps\":{},\"args\":[",
        quote(&path),
        result.as_deref().map_or_else(|| "null".to_string(), quote),
        error.as_deref().map_or_else(|| "null".to_string(), quote),
        vm.steps()
    ));
    let shown: Vec<String> = outs.iter().map(|v| quote(&describe(&vm, v))).collect();
    out.push_str(&shown.join(","));
    out.push_str("],\"log\":[");
    let log: Vec<String> = vm.log.iter().map(|l| quote(l)).collect();
    out.push_str(&log.join(","));
    out.push_str("]}\n");
    Ok(if error.is_none() { 0 } else { 1 })
}

/// The `--arg` kinds in order with the position of the value each made.
fn args_kinds(args: &[String]) -> Vec<(usize, String)> {
    let mut kinds = Vec::new();
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--cooked" | "--self" => i += 1,
            "--arg" => {
                if let Some(text) = args.get(i + 1)
                    && let Some((kind, _)) = text.split_once(':')
                {
                    kinds.push((kinds.len(), kind.to_string()));
                }
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    kinds
}

/// A `v:`, `r:` or `st:` argument: the C++ `zeroStruct("Vector"/"Rotator")` with the three fields in the C++ order, or
/// `newStruct(Package.Struct)` (its defaults applied) with `Field=value` pairs, each converted by the field's kind.
fn make_struct(vm: &mut Vm, p: &PendingStruct) -> Result<Value, String> {
    let native = p.ty == "Vector" || p.ty == "Rotator";
    let path = if native {
        format!("Core.Object.{}", p.ty)
    } else {
        let Some((package, rest)) = p.ty.split_once('.') else {
            return Err(format!("struct path must be Package.Struct: {}", p.ty));
        };
        struct_path(vm, package, rest).ok_or_else(|| format!("struct not found: {}", p.ty))?
    };
    let id = vm.space.classes.struct_by_path(&path).ok_or_else(|| {
        format!("not yet: the struct {path} is not loaded (a struct is defined when a class or function that uses it loads)")
    })?;
    let mut value = vm
        .space
        .classes
        .struct_defaults(id)
        .map_err(|e| error_text(&e))?;
    let layout = vm
        .space
        .classes
        .struct_layout(id)
        .ok_or_else(|| format!("struct not found: {}", p.ty))?
        .clone();
    let Some(s) = value.struct_mut() else {
        return Err(format!("struct not found: {}", p.ty));
    };
    for (f, text) in p.fields.iter().enumerate() {
        if native {
            let names: &[&str] = if p.ty == "Vector" {
                &["X", "Y", "Z"]
            } else {
                &["Pitch", "Yaw", "Roll"]
            };
            if f < 3
                && let Some((entry, _)) =
                    vm.space.names.find(names[f]).and_then(|n| layout.field(n))
            {
                s.fields[entry as usize] = if p.ty == "Vector" {
                    Value::Float(stod(text)?.0 as f32)
                } else {
                    Value::Int(stoll(text)?.0 as i32)
                };
            }
            continue;
        }
        let field = text.split_once('=');
        let slot = field
            .and_then(|(name, _)| vm.space.names.find(name))
            .and_then(|n| layout.field(n))
            .map(|(entry, _)| entry as usize)
            .ok_or_else(|| format!("unknown struct field in --arg: {text}"))?;
        let number = field.map_or("", |(_, v)| v);
        s.fields[slot] = match &s.fields[slot] {
            Value::Int(_) => Value::Int(stoll(number)?.0 as i32),
            Value::Byte(_) => Value::Byte(stoll(number)?.0 as u8),
            Value::Bool(_) => Value::Bool(number == "true" || number == "1"),
            _ => Value::Float(stod(number)?.0 as f32),
        };
    }
    Ok(value)
}

/// The class-table path of the struct export at `rest` of package `package` (C++ `findExport`).
fn struct_path(vm: &mut Vm, package: &str, rest: &str) -> Option<String> {
    let (code, store) = vm.defs.code_mut();
    let id = code.package(store, package).ok()?;
    let index = code.find_export(store, id, rest);
    if index == 0 {
        return None;
    }
    let pkg = store.get(id);
    Some(format!("{}.{}", pkg.name, pkg.path_of(index).ok()?))
}

// ------------------------------------------------------------------------------------------------ --run-batch

/// The C++ property class name a declaration's kind is (the `type` the batch's checks print).
fn type_name(kind: &PropKind) -> &'static str {
    match kind {
        PropKind::Int => "IntProperty",
        PropKind::Float => "FloatProperty",
        PropKind::Bool => "BoolProperty",
        PropKind::Byte { .. } => "ByteProperty",
        PropKind::Str => "StrProperty",
        PropKind::Name => "NameProperty",
        PropKind::Object { .. } => "ObjectProperty",
        PropKind::Interface { .. } => "InterfaceProperty",
        PropKind::Class { .. } => "ClassProperty",
        PropKind::Struct(_) => "StructProperty",
        PropKind::Array(_) => "ArrayProperty",
        PropKind::Delegate => "DelegateProperty",
        PropKind::FloatAttribute => "FloatAttributeProperty",
        PropKind::IntAttribute => "IntAttributeProperty",
        PropKind::ByteAttribute { .. } => "ByteAttributeProperty",
        PropKind::Map { .. } => "MapProperty",
        _ => "Property",
    }
}

/// The C++ `fromHex`: two hex digits per byte. The bytes are read as UTF-8 (a `Value::Str` is UTF-8; bytes that are
/// not give U+FFFD, where the C++ keeps them raw).
fn from_hex(hex: &str) -> Result<String, String> {
    if !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid hex text".into());
    }
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0))
        .collect();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The batch's `integer`: `std::stoll` that must use the whole text.
fn integer(body: &str) -> Result<i64, String> {
    let (value, used) = stoll(body)?;
    if used != body.len() {
        return Err("invalid integer".into());
    }
    Ok(value)
}

/// The batch's `number`: `std::stod` that must use the whole text and be finite.
fn number(body: &str) -> Result<f64, String> {
    let (value, used) = stod(body)?;
    if used != body.len() || !value.is_finite() {
        return Err("invalid number".into());
    }
    Ok(value)
}

fn run_batch(args: &[String], mode: ParityMode) -> Outcome {
    let mut out = String::new();
    match run_batch_inner(args, mode, &mut out) {
        Ok(()) => Outcome {
            stdout: out,
            error: None,
            code: 0,
        },
        Err(message) => Outcome::failed(out, message),
    }
}

fn run_batch_inner(args: &[String], mode: ParityMode, out: &mut String) -> Result<(), String> {
    Package::load(Path::new(&args[0])).map_err(|e| error_text(&e))?;
    if args.len() != 5 || args[3] != "--cooked" {
        return Err(usage());
    }
    let cases = std::fs::read(&args[2]).map_err(|_| "cannot open batch file".to_string())?;
    let mut vm = open_vm(Path::new(&args[4]), mode)?;
    let text = String::from_utf8_lossy(&cases).into_owned();
    let mut number_of_case = 0usize;
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        vm.log.clear();
        let mut native = false;
        let mut kind = "None";
        let mut shown = String::new();
        let error = match batch_case(&mut vm, &fields, &mut native) {
            Ok(result) => {
                (kind, shown) = batch_value(&vm, &result);
                None
            }
            Err(message) => Some(message),
        };
        let unimplemented: Vec<String> = vm
            .log
            .iter()
            .filter_map(|l| l.strip_prefix("UNIMPLEMENTED "))
            .map(quote)
            .collect();
        out.push_str(&format!(
            "{{\"case\":{number_of_case},\"type\":{},\"value\":{},\"native\":{native},\"error\":{},\"unimplemented\":[{}]}}\n",
            quote(kind),
            quote(&shown),
            error.as_deref().map_or_else(|| "null".to_string(), quote),
            unimplemented.join(",")
        ));
        number_of_case += 1;
    }
    Ok(())
}

/// The C++ result line's type and text of `v`.
fn batch_value(vm: &Vm, v: &Value) -> (&'static str, String) {
    match v {
        Value::Int(i) => ("Int", i.to_string()),
        Value::Byte(b) => ("Byte", b.to_string()),
        Value::Bool(b) => ("Bool", if *b { "1" } else { "0" }.to_string()),
        Value::Float(f) => ("Float", fmt_g(f64::from(*f), 9)),
        Value::Str(s) => ("String", s.to_string()),
        Value::Name(n) => ("Name", vm.space.names.text(*n).into_owned()),
        Value::Object(o) | Value::Interface(o) => (
            "Object",
            o.and_then(|o| vm.space.heap.get(o).ok()).map_or_else(
                || "None".to_string(),
                |o| vm.space.names.text(o.name).into_owned(),
            ),
        ),
        Value::None => ("None", String::new()),
        _ => ("Unsupported", String::new()),
    }
}

fn batch_case(vm: &mut Vm, fields: &[&str], native: &mut bool) -> Result<Value, String> {
    if fields.len() < 2 {
        return Err("malformed case".into());
    }
    let function = vm.find_function(fields[0]).map_err(|e| vm_text(&e))?;
    let decl = vm.function(function).cloned().ok_or("no such function")?;
    *native = decl.is_native();
    let this = if fields[1].is_empty() {
        None
    } else {
        let class = vm.find_class(fields[1]).map_err(|e| vm_text(&e))?;
        Some(instantiate_from_defaults(vm, class).map_err(|e| vm_text(&e))?)
    };
    if let (Some(object), Some(owner)) = (this, decl.owner) {
        let class = vm.space.heap.get(object).map_err(|e| error_text(&e))?.class;
        if !vm.space.classes.is_child_of(class, owner) {
            return Err("self class does not inherit function owner".into());
        }
    }
    let params = &decl.params;
    let mut values = vec![Value::None; params.len()];
    let mut given = vec![false; params.len()];
    for field in &fields[2..] {
        let equal = field.find('=');
        let colon = field[equal.unwrap_or(0)..]
            .find(':')
            .map(|c| c + equal.unwrap_or(0));
        let (Some(equal), Some(colon)) = (equal, colon) else {
            return Err("malformed argument".into());
        };
        let (name, kind, body) = (
            &field[..equal],
            &field[equal + 1..colon],
            &field[colon + 1..],
        );
        let mut slot = params.len();
        for (p, decl) in params.iter().enumerate() {
            if vm.space.names.text(decl.name).eq_ignore_ascii_case(name) {
                slot = p;
            }
        }
        if slot == params.len() {
            return Err(format!("no parameter named {name}"));
        }
        if given[slot] {
            return Err(format!("duplicate parameter {name}"));
        }
        let declared = type_name(&params[slot].kind);
        values[slot] = match kind {
            "i" => Value::Int(integer(body)? as i32),
            "f" => Value::Float(number(body)? as f32),
            "b" => match body {
                "0" => Value::Bool(false),
                "1" => Value::Bool(true),
                _ => return Err("invalid bool".into()),
            },
            "y" => Value::Byte(integer(body)? as u8),
            "s" => Value::string(&from_hex(body)?),
            "n" => Value::Name(vm.space.names.intern(&from_hex(body)?)),
            "o" => {
                if !body.is_empty() {
                    return Err("only null object arguments are supported".into());
                }
                Value::Object(None)
            }
            "d" => match declared {
                "IntProperty" => Value::Int(signed_number(body)?),
                "FloatProperty" => Value::Float(number(body)? as f32),
                "ByteProperty" => {
                    let v = integer(body)?;
                    if !(0..=255).contains(&v) {
                        return Err("byte out of range".into());
                    }
                    Value::Byte(v as u8)
                }
                "BoolProperty" => Value::Bool(number(body)? != 0.0),
                other => return Err(format!("number given for a {other} parameter")),
            },
            "t" => match declared {
                "NameProperty" => Value::Name(vm.space.names.intern(&from_hex(body)?)),
                "StrProperty" => Value::string(&from_hex(body)?),
                other => return Err(format!("text given for a {other} parameter")),
            },
            other => return Err(format!("unknown argument kind {other}")),
        };
        let fits = matches!(
            (declared, &values[slot]),
            ("IntProperty", Value::Int(_))
                | ("FloatProperty", Value::Float(_))
                | ("BoolProperty", Value::Bool(_))
                | ("ByteProperty", Value::Byte(_))
                | ("StrProperty", Value::Str(_))
                | ("NameProperty", Value::Name(_))
                | ("ObjectProperty", Value::Object(_))
        );
        if !fits {
            return Err(format!("argument type does not match {declared}"));
        }
        given[slot] = true;
    }
    // Omitted trailing optionals stay omitted; no zero is put in for a missing input.
    let mut count = given.len();
    while count > 0 && !given[count - 1] && params[count - 1].is_optional() {
        count -= 1;
    }
    for p in 0..count {
        if !given[p] {
            return Err(format!(
                "missing parameter {}",
                vm.space.names.text(params[p].name)
            ));
        }
    }
    values.truncate(count);
    vm.call(function, this, values)
        .map(|r| r.ret)
        .map_err(|e| vm_text(&e))
}

#[cfg(test)]
mod tests {
    //! The command lines of `natives_test.py` for `--native`, the argument parsers against the C++ runtime's texts,
    //! and `--run` / `--run-batch` on an invented package (AGENTS.md rule 9). The C++ binary gave the same output for
    //! each case here (local differential, decision 2026-11-15-14).

    use super::*;
    use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn native_prints_the_stub_line_and_refuses_bad_command_lines() {
        let done = native(&strings(&[
            "--native",
            "WillowGame.WillowWeapon.Fire",
            "--native-args",
            "a,b",
        ]));
        assert_eq!(done.code, 0);
        assert_eq!(
            done.stdout,
            "{\"name\":\"WillowGame.WillowWeapon.Fire\",\"status\":\"unimplemented\",\
             \"log\":\"UNIMPLEMENTED WillowGame.WillowWeapon.Fire(a,b)\"}\n"
        );
        let bare = native(&strings(&["--native", "Engine.Actor.Trace"]));
        assert!(
            bare.stdout
                .contains("\"log\":\"UNIMPLEMENTED Engine.Actor.Trace()\"")
        );
        for (bad, message) in [
            (vec!["--native"], USAGE),
            (vec!["--native", "", "--native-args", "x"], USAGE),
            (
                vec!["--native", "A", "--native-args"],
                "missing value for --native-args",
            ),
            (vec!["--native", "A", "--bogus", "x"], USAGE),
            (vec!["--native", "A", "--native-args", "x", "extra"], USAGE),
        ] {
            let done = native(&strings(&bad));
            assert_eq!(
                (done.code, done.stdout.as_str(), done.error.as_deref()),
                (1, "", Some(message)),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn stoll_and_stod_follow_the_msvc_runtime() {
        assert_eq!(stoll(" -42x"), Ok((-42, 4)));
        assert_eq!(stoll("+7"), Ok((7, 2)));
        assert_eq!(stoll("x").unwrap_err(), "invalid stoll argument");
        assert_eq!(
            stoll("99999999999999999999").unwrap_err(),
            "stoll argument out of range"
        );
        assert_eq!(stod("1.5e2xyz"), Ok((150.0, 5)));
        assert_eq!(stod(" .5"), Ok((0.5, 3)));
        assert_eq!(stod("1e"), Ok((1.0, 1)));
        assert!(stod("-inf").unwrap().0.is_infinite());
        assert_eq!(stod("x").unwrap_err(), "invalid stod argument");
        assert_eq!(stod("1e999").unwrap_err(), "stod argument out of range");
        assert_eq!(integer("12").unwrap(), 12);
        assert_eq!(integer("1oops").unwrap_err(), "invalid integer");
        assert_eq!(number("2.5").unwrap(), 2.5);
        assert_eq!(number("nan").unwrap_err(), "invalid number");
        assert_eq!(from_hex("610962").unwrap(), "a\tb");
        assert_eq!(from_hex("6").unwrap_err(), "invalid hex text");
        assert_eq!(from_hex("zz").unwrap_err(), "invalid hex text");
    }

    /// `Core.Object` with one operator native (`+`, 146) and `Core.Foo.Answer` (script, returns 42).
    fn package() -> (PathBuf, PathBuf) {
        let mut t = ScriptToy::new(&["Int"]);
        let object = t.class("Object", 0);
        let mut add = FunctionSpec::new(0x400 | 0x1000 | 0x23000, "+");
        add.native = 146;
        t.function(
            object,
            "Add_IntInt",
            &[Param::new("Int", "A"), Param::new("Int", "B")],
            Some(("Int", 0)),
            &[],
            &add,
            |_| None,
        );
        let foo = t.class("Foo", object);
        t.function(
            foo,
            "Answer",
            &[],
            Some(("Int", 0)),
            &[],
            &FunctionSpec::new(0x20003, "Answer"),
            |_| {
                let mut a = ow_package::testkit::script::Asm::new();
                a.stmt().raw(&[0x04]).int_const(42);
                a.end();
                Some(a)
            },
        );
        let dir =
            std::env::temp_dir().join(format!("ow-cli-vm-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pkg = dir.join("Core.upk");
        std::fs::write(&pkg, t.build()).unwrap();
        (dir, pkg)
    }

    #[test]
    fn run_and_run_batch_on_an_invented_package() {
        let (dir, pkg) = package();
        let p = pkg.display().to_string();
        let d = dir.display().to_string();
        // A native called directly needs no class default object.
        let done = run_one(
            &strings(&[
                &p,
                "--run",
                "Core.Object.Add_IntInt",
                "--cooked",
                &d,
                "--arg",
                "i:40",
                "--arg",
                "i:2",
            ]),
            ParityMode::Off,
        );
        assert_eq!(done.code, 0, "{:?}", done.error);
        assert_eq!(
            done.stdout,
            "{\"function\":\"Core.Object.Add_IntInt\",\"result\":\"42\",\"error\":null,\"steps\":1,\
             \"args\":[\"40\",\"2\"],\"log\":[]}\n"
        );
        let too_many = run_one(
            &strings(&[
                &p,
                "--run",
                "Core.Object.Add_IntInt",
                "--cooked",
                &d,
                "--arg",
                "i:1",
                "--arg",
                "i:2",
                "--arg",
                "i:3",
            ]),
            ParityMode::Off,
        );
        assert_eq!(too_many.code, 1);
        assert!(
            too_many.stdout.contains(
                "\"error\":\"too many arguments for Core.Object.Add_IntInt\",\"steps\":0"
            )
        );
        let missing = run_one(
            &strings(&[&p, "--run", "Core.Foo.Missing", "--cooked", &d]),
            ParityMode::Off,
        );
        assert_eq!(
            (missing.stdout.as_str(), missing.error.as_deref()),
            ("", Some("function not found: Core.Foo.Missing"))
        );
        for bad in [
            vec![p.as_str(), "--run", "--cooked", &d],
            vec![p.as_str(), "--run", "Core.Foo.Answer"],
            vec![p.as_str(), "--run", "A", "B", "--cooked", &d],
            vec![p.as_str(), "--run", "A", "--cooked", &d, "--arg", "zz"],
        ] {
            assert_eq!(
                run_one(&strings(&bad), ParityMode::Off).error.as_deref(),
                Some(USAGE),
                "{bad:?}"
            );
        }
        let batch = dir.join("cases.tsv");
        std::fs::write(
            &batch,
            "Core.Object.Add_IntInt\t\tB=d:2\ta=d:40\r\n\nCore.Foo.Missing\t\nCore.Object.Add_IntInt\t\tA=d:1\noneField\n",
        )
        .unwrap();
        let done = run_batch(
            &strings(&[
                &p,
                "--run-batch",
                &batch.display().to_string(),
                "--cooked",
                &d,
            ]),
            ParityMode::Off,
        );
        assert_eq!(done.code, 0);
        let lines: Vec<&str> = done.stdout.lines().collect();
        assert_eq!(
            lines,
            [
                "{\"case\":0,\"type\":\"Int\",\"value\":\"42\",\"native\":true,\"error\":null,\"unimplemented\":[]}",
                "{\"case\":1,\"type\":\"None\",\"value\":\"\",\"native\":false,\"error\":\"function not found: Core.Foo.Missing\",\"unimplemented\":[]}",
                "{\"case\":2,\"type\":\"None\",\"value\":\"\",\"native\":true,\"error\":\"missing parameter B\",\"unimplemented\":[]}",
                "{\"case\":3,\"type\":\"None\",\"value\":\"\",\"native\":false,\"error\":\"malformed case\",\"unimplemented\":[]}",
            ]
        );
        let none = run_batch(
            &strings(&[&p, "--run-batch", "nope.tsv", "--cooked", &d]),
            ParityMode::Off,
        );
        assert_eq!(none.error.as_deref(), Some("cannot open batch file"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
