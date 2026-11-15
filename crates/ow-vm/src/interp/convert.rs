//! Owner: lane W3-INTERP. Value conversions of the interpreter: the C++ assignment rule (`interp.cpp:511-522`), the
//! primitive casts (`castPrimitive`, `rotatorToVector`, `vectorToRotator`), the string form a cast to string gives
//! (`describeForString`), the readable form `--run` prints (`Value::describe`, `src/vm.cpp`) and the text of the
//! string constants.
//!
//! The C++ `Value` keeps Int, Bool and Byte in one integer field and String, Name and a delegate's function in one
//! text field, so a conversion that reads "the integer" or "the text" of a value of another kind reads 0 or "" there;
//! [`int_of`], [`number_of`] and [`text_of`] keep that. Float to int is `int64_t(double)` then the 32-bit wrap
//! (`ow_core::num::trunc_i64_wrap_i32`), trigonometry and roots go through `ow_core::num`, floats print through
//! `ow_core::fmt`.

use std::rc::Rc;

use ow_core::NotYet;
use ow_core::fmt::{fmt_f, fmt_ostream};
use ow_core::num::{atan2_f64, cos_f64, sin_f64, sqrt_f64, trunc_i64_wrap_i32};
use ow_object::decode::native_layout;
use ow_object::value::{StructValue, Value};

use crate::api::Vm;
use crate::error::VmError;

/// The integer the C++ `Value::integer()` reads (then `makeInt`'s 32-bit wrap): Int, Byte and Bool as they are, a
/// float toward zero through `int64_t`, anything else 0.
pub(crate) fn int_of(v: &Value) -> i32 {
    match v {
        Value::Int(i) => *i,
        Value::Byte(b) => i32::from(*b),
        Value::Bool(b) => i32::from(*b),
        Value::Float(f) => trunc_i64_wrap_i32(f64::from(*f)),
        _ => 0,
    }
}

/// The C++ `makeByte(v.integer())`: the low 8 bits.
pub(crate) fn byte_of(v: &Value) -> u8 {
    int_of(v) as u8
}

/// The C++ `Value::number()`: a float as it is, Int, Byte and Bool as a double, anything else 0.
pub(crate) fn number_of(v: &Value) -> f64 {
    match v {
        Value::Float(f) => f64::from(*f),
        Value::Int(i) => f64::from(*i),
        Value::Byte(b) => f64::from(*b),
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// The C++ `makeFloat(v.number())`: rounded once to single precision.
pub(crate) fn float_of(v: &Value) -> f32 {
    number_of(v) as f32
}

/// The C++ text field: a string's or a name's text, a bound delegate's function name, anything else "".
pub(crate) fn text_of(vm: &Vm, v: &Value) -> String {
    match v {
        Value::Str(s) => s.to_string(),
        Value::Name(n) => vm.space.names.text(*n).into_owned(),
        Value::Delegate(Some(d)) => vm.space.names.text(d.function).into_owned(),
        _ => String::new(),
    }
}

/// The C++ `assign`: when the target already holds a value of another kind, the numeric kinds the compiler treats as
/// compatible convert to the target's kind (a byte slot given an int keeps wrapping to a byte) and a string given to
/// a name slot becomes a name; otherwise the value is stored as it is.
pub(crate) fn assign(vm: &mut Vm, current: &Value, value: Value) -> Value {
    use Value as V;
    if std::mem::discriminant(current) == std::mem::discriminant(&value)
        || matches!(current, V::None)
    {
        return value;
    }
    match (current, &value) {
        (V::Int(_), V::Byte(_) | V::Bool(_) | V::Float(_)) => V::Int(int_of(&value)),
        (V::Float(_), V::Int(_) | V::Byte(_)) => V::Float(float_of(&value)),
        (V::Byte(_), V::Int(_) | V::Bool(_)) => V::Byte(byte_of(&value)),
        (V::Bool(_), V::Int(_) | V::Byte(_)) => V::Bool(value.truth(&vm.space.names)),
        (V::Name(_), V::Str(s)) => V::Name(vm.space.names.intern(s)),
        _ => value,
    }
}

/// `castPrimitive`: the primitive cast `code` of `v`. A code the C++ does not handle is a stub site
/// (`primitive cast 0x<hh>`) that returns `v`.
pub(crate) fn cast(vm: &mut Vm, code: u8, v: Value) -> Result<Value, VmError> {
    Ok(match code {
        0x3A | 0x41 | 0x44 => Value::Int(int_of(&v)),
        0x3B | 0x3E | 0x45 | 0x47 | 0x48 => Value::Bool(v.truth(&vm.space.names)),
        0x3C | 0x3F | 0x42 => Value::Float(float_of(&v)),
        0x3D | 0x40 | 0x43 => Value::Byte(byte_of(&v)),
        0x49 | 0x4A => Value::Int(c_strtol(&text_of(vm, &v))),
        0x4B => {
            let t = text_of(vm, &v).to_ascii_lowercase();
            Value::Bool(t == "true" || t == "1")
        }
        0x4C => Value::Float(c_strtod(&text_of(vm, &v))? as f32),
        0x52..=0x5A => Value::string(&describe_for_string(vm, &v)),
        0x60 => {
            let t = text_of(vm, &v);
            Value::Name(vm.space.names.intern(&t))
        }
        0x39 => rotator_to_vector(vm, &v)?,
        0x50 => vector_to_rotator(vm, &v)?,
        // Object <-> interface views: interfaces are W4-INTERP2's.
        0x36 | 0x37 | 0x46 => {
            return Err(NotYet::not_ported(
                format!("primitive cast 0x{code:02x} (an object-interface view)"),
                "lane W4-INTERP2 (interfaces)",
            )
            .into());
        }
        _ => {
            vm.stub_site(
                &format!("primitive cast 0x{code:02x}"),
                format!("UNIMPLEMENTED primitive cast 0x{code:02x}"),
            )?;
            v
        }
    })
}

/// C `strtol(s, nullptr, 10)` as MSVC gives it (`long` is 32 bits): leading white space, a sign, decimal digits up to
/// the first other character; no digits gives 0; a value past the range gives `LONG_MAX` / `LONG_MIN`.
pub(crate) fn c_strtol(s: &str) -> i32 {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && is_c_space(b[i]) {
        i += 1;
    }
    let negative = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut value: i64 = 0;
    let mut overflow = false;
    while i < b.len() && b[i].is_ascii_digit() {
        value = value * 10 + i64::from(b[i] - b'0');
        if value > i64::from(i32::MAX) + 1 {
            overflow = true;
            value = i64::from(i32::MAX) + 1;
        }
        i += 1;
    }
    if negative {
        if overflow || -value < i64::from(i32::MIN) {
            i32::MIN
        } else {
            (-value) as i32
        }
    } else if overflow || value > i64::from(i32::MAX) {
        i32::MAX
    } else {
        value as i32
    }
}

/// C `isspace` in the C locale.
fn is_c_space(c: u8) -> bool {
    c == b' ' || (0x09..=0x0d).contains(&c)
}

/// The length of the longest prefix of `s` (after white space) that C `strtod` reads as a decimal number, `inf`,
/// `infinity` or `nan` (optionally `nan(...)`), and whether it is a hexadecimal float. `None` when nothing converts.
pub fn strtod_prefix(s: &str) -> Option<(usize, usize, bool)> {
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

/// C `strtod(s, nullptr)` for decimal text, `inf`, `infinity` and `nan` (MSVC's UCRT rounds correctly, as Rust's
/// parser does): the longest prefix that converts, 0 when none does. A hexadecimal float (`0x1.8p3`) is
/// `NotYet { NotRead }`: no parser for it is written here.
pub(crate) fn c_strtod(s: &str) -> Result<f64, VmError> {
    let Some((start, end, hex)) = strtod_prefix(s) else {
        return Ok(0.0);
    };
    if hex {
        return Err(NotYet::not_read(
            format!("string to float of the hexadecimal text {s:?}"),
            "lane W3-INTERP (C strtod hexadecimal floats are not parsed)",
        )
        .into());
    }
    let text = &s[start..end];
    let lower = text.to_ascii_lowercase();
    let unsigned = lower.trim_start_matches(['+', '-']);
    let negative = lower.starts_with('-');
    let value = if unsigned.starts_with("inf") {
        f64::INFINITY
    } else if unsigned.starts_with("nan") {
        f64::NAN
    } else {
        unsigned.parse::<f64>().unwrap_or(0.0)
    };
    Ok(if negative { -value } else { value })
}

/// `describeForString`: what a cast to string gives. Ints and bytes in decimal, bools as `True`/`False`, floats as
/// `%f`, strings and names as their text, an object as its name, a class as its name, `None` for a null reference,
/// a struct as `Field=value` pairs joined by spaces (a static-array field shows an empty value, as the C++ array
/// value has no string form), anything else "".
pub fn describe_for_string(vm: &Vm, v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Byte(b) => b.to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Float(f) => fmt_f(f64::from(*f), 6),
        Value::Str(s) => s.to_string(),
        Value::Name(n) => vm.space.names.text(*n).into_owned(),
        Value::Object(o) | Value::Interface(o) => match o.and_then(|o| vm.space.heap.get(o).ok()) {
            Some(object) => vm.space.names.text(object.name).into_owned(),
            None => "None".into(),
        },
        Value::Class(c) => match c.and_then(|c| vm.space.classes.get_layout(c)) {
            Some(l) => l.path().name().to_string(),
            None => "None".into(),
        },
        Value::Struct(s) => struct_fields(vm, s)
            .into_iter()
            .map(|(name, values)| match values.as_slice() {
                [one] => format!("{name}={}", describe_for_string(vm, one)),
                _ => format!("{name}="),
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// `Value::describe` (`src/vm.cpp`): the readable form `--run` prints. `None`, numbers, `true`/`false`, a float as the
/// ostream prints it, `"text"`, `'name'`, `Class'Name'` for an object, `Class'Name'` for a class, `Delegate(Obj.Fn)`,
/// `[a,b]` for an array and `Type{Field=value,...}` for a struct, a static-array field as an array. The eight native
/// structs list their fields in the C++ order.
pub fn describe(vm: &Vm, v: &Value) -> String {
    match v {
        Value::None => "None".into(),
        Value::Int(i) => i.to_string(),
        Value::Byte(b) => b.to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Float(f) => fmt_ostream(f64::from(*f)),
        Value::Str(s) => format!("\"{s}\""),
        Value::Name(n) => format!("'{}'", vm.space.names.text(*n)),
        Value::Object(o) | Value::Interface(o) => match o.and_then(|o| vm.space.heap.get(o).ok()) {
            Some(object) => {
                let class = vm
                    .space
                    .classes
                    .get_layout(object.class)
                    .map_or("Object", |l| l.path().name());
                format!("{class}'{}'", vm.space.names.text(object.name))
            }
            None => "None".into(),
        },
        Value::Class(c) => match c.and_then(|c| vm.space.classes.get_layout(c)) {
            Some(l) => format!("Class'{}'", l.path().name()),
            None => "None".into(),
        },
        Value::Delegate(d) => {
            let object = d
                .and_then(|d| d.object)
                .and_then(|o| vm.space.heap.get(o).ok())
                .map_or_else(
                    || "None".to_string(),
                    |o| vm.space.names.text(o.name).into_owned(),
                );
            let function = d.map_or_else(String::new, |d| {
                vm.space.names.text(d.function).into_owned()
            });
            format!("Delegate({object}.{function})")
        }
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(|e| describe(vm, e)).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Struct(s) => {
            let name = vm
                .space
                .classes
                .struct_layout(s.ty)
                .map_or(String::new(), |l| l.path().name().to_string());
            let parts: Vec<String> = struct_fields(vm, s)
                .into_iter()
                .map(|(field, values)| match values.as_slice() {
                    [one] => format!("{field}={}", describe(vm, one)),
                    many => {
                        let items: Vec<String> = many.iter().map(|e| describe(vm, e)).collect();
                        format!("{field}=[{}]", items.join(","))
                    }
                })
                .collect();
            format!("{name}{{{}}}", parts.join(","))
        }
        // `Value` may grow; a variant the C++ has no form for prints as its kind.
        #[allow(unreachable_patterns)]
        other => other.kind_name().to_string(),
    }
}

/// A struct's fields as the C++ aggregate lists them: the eight native structs in the C++ order (fields the layout
/// lacks are left out), every other struct in its declared order; each with its value, or its elements for a
/// static-array field.
fn struct_fields(vm: &Vm, s: &StructValue) -> Vec<(String, Vec<Value>)> {
    let Some(layout) = vm.space.classes.struct_layout(s.ty) else {
        return Vec::new();
    };
    let entry = |first: u32, dim: i32| -> Vec<Value> {
        let first = first as usize;
        let dim = usize::try_from(dim.max(1)).unwrap_or(1);
        s.fields
            .get(first..first + dim)
            .map(<[Value]>::to_vec)
            .unwrap_or_default()
    };
    if let Some((names, _)) = native_layout(layout.path().name()) {
        return names
            .iter()
            .filter_map(|field| {
                let id = vm.space.names.find(field)?;
                let (first, decl) = layout.field(id)?;
                Some((field.to_string(), entry(first, decl.array_dim)))
            })
            .collect();
    }
    layout
        .fields()
        .map(|(first, decl)| {
            (
                vm.space.names.text(decl.name).into_owned(),
                entry(first, decl.array_dim),
            )
        })
        .collect()
}

/// The field `name` (any case) of a struct value as a number (the C++ `field(name)->number()`), 0 when `v` is not a
/// struct or has no such field.
fn field_number(vm: &Vm, v: &Value, name: &str) -> f64 {
    let Value::Struct(s) = v else {
        return 0.0;
    };
    vm.space
        .names
        .find(name)
        .and_then(|n| vm.space.classes.struct_layout(s.ty)?.field(n))
        .and_then(|(first, _)| s.fields.get(first as usize))
        .map_or(0.0, number_of)
}

/// The C++ `zeroStruct("Vector")` / `zeroStruct("Rotator")` with fields set by name: the `Core.Object` struct of
/// that name at its zero value. `NotYet { LayerAbsent }` while the struct is not loaded.
fn native_struct(vm: &Vm, name: &str, fields: &[(&str, Value)]) -> Result<Value, VmError> {
    let path = format!("Core.Object.{name}");
    let classes = &vm.space.classes;
    let layout = classes
        .struct_by_path(&path)
        .and_then(|s| classes.struct_layout(s))
        .ok_or_else(|| {
            VmError::from(NotYet::layer_absent(
                format!("the {path} struct"),
                "ow-object reflection of the Core package",
                "lane W3-INTERP (a struct is defined when a class or function that uses it loads)",
            ))
        })?;
    let mut value = layout.zero().clone();
    if let Some(s) = value.struct_mut() {
        for (field, v) in fields {
            if let Some((first, _)) = vm.space.names.find(field).and_then(|n| layout.field(n))
                && let Some(slot) = s.fields.get_mut(first as usize)
            {
                *slot = v.clone();
            }
        }
    }
    Ok(value)
}

/// `rotatorToVector`: the rotator's forward axis (rotator units are 1/65536 of a turn). Standard UE3 convention,
/// UNVERIFIED against the game (the C++ label).
fn rotator_to_vector(vm: &Vm, r: &Value) -> Result<Value, VmError> {
    let k = std::f64::consts::PI / 32768.0;
    let pitch = field_number(vm, r, "Pitch") * k;
    let yaw = field_number(vm, r, "Yaw") * k;
    native_struct(
        vm,
        "Vector",
        &[
            ("X", Value::Float((cos_f64(pitch) * cos_f64(yaw)) as f32)),
            ("Y", Value::Float((cos_f64(pitch) * sin_f64(yaw)) as f32)),
            ("Z", Value::Float(sin_f64(pitch) as f32)),
        ],
    )
}

/// `vectorToRotator`: yaw and pitch of the vector, roll 0. UNVERIFIED against the game (the C++ label).
fn vector_to_rotator(vm: &Vm, v: &Value) -> Result<Value, VmError> {
    let k = 32768.0 / std::f64::consts::PI;
    let (x, y, z) = (
        field_number(vm, v, "X"),
        field_number(vm, v, "Y"),
        field_number(vm, v, "Z"),
    );
    native_struct(
        vm,
        "Rotator",
        &[
            ("Yaw", Value::Int(trunc_i64_wrap_i32(atan2_f64(y, x) * k))),
            (
                "Pitch",
                Value::Int(trunc_i64_wrap_i32(
                    atan2_f64(z, sqrt_f64(x * x + y * y)) * k,
                )),
            ),
        ],
    )
}

/// An ANSI string constant's text: each stored byte is one character (Latin-1, the UE3 widening of an ANSI
/// character). For ASCII this is the C++ text; past ASCII the C++ keeps the raw byte.
pub(crate) fn ansi_text(bytes: &[u8]) -> Rc<str> {
    bytes
        .iter()
        .map(|&b| char::from(b))
        .collect::<String>()
        .into()
}

/// A Unicode string constant's text from the decoder's form (each UTF-16 unit written as its own UTF-8 sequence):
/// the units are recovered and decoded as UTF-16, so a surrogate pair becomes its character and a lone surrogate
/// U+FFFD (the `Value::Str` limit, decision 2026-11-15-07).
pub(crate) fn unicode_text(bytes: &[u8]) -> Rc<str> {
    let mut units = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let (unit, width) = if b < 0x80 {
            (u16::from(b), 1)
        } else if b & 0xE0 == 0xC0 && i + 1 < bytes.len() {
            (
                (u16::from(b & 0x1F) << 6) | u16::from(bytes[i + 1] & 0x3F),
                2,
            )
        } else if b & 0xF0 == 0xE0 && i + 2 < bytes.len() {
            (
                (u16::from(b & 0x0F) << 12)
                    | (u16::from(bytes[i + 1] & 0x3F) << 6)
                    | u16::from(bytes[i + 2] & 0x3F),
                3,
            )
        } else {
            // Not a form the decoder writes: one replacement unit per byte.
            (0xFFFD, 1)
        };
        units.push(unit);
        i += width;
    }
    char::decode_utf16(units)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect::<String>()
        .into()
}
