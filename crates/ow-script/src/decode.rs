//! Owner: lane W2-SCRIPT. The bytecode decoder: the port of `Decoder`, `Expr`, `Code` and `decode` of
//! `src/script.cpp` / `src/script.hpp`.
//!
//! A function's script is read into an expression tree; nothing is executed here (execution is `ow-vm`'s). Every
//! structural check of the C++ decoder is here, in the same order and with the same message, so a function the C++
//! refuses is refused with the same text: an unknown opcode, a read past the script, a script that does not end in
//! `EndOfScript` exactly at its stored size, an in-memory size that disagrees with the header, a jump or case target
//! that is not a statement start, nesting deeper than 256. Untrusted bytes never panic: every read is checked.
//!
//! Object references are 4 bytes in the file and 8 in the in-memory script that jump offsets count in, so each
//! expression records both offsets (`file`, `mem`).

use std::fmt;

use ow_core::FixedHashMap;
use ow_package::package::Package;

use crate::function::FunctionInfo;
use crate::ops::{self, op};

/// How deep sub-expressions may nest (the C++ `MAX_DEPTH`).
pub const MAX_DEPTH: u32 = 256;

/// How many entries a label table may hold before it is refused as runaway (the C++ bound).
pub const MAX_LABELS: usize = 4096;

/// Why a function could not be read or decoded. `Display` gives the C++ `DecodeError` text, which the CLI prints
/// after `ow-package: ` and `--script-check --failures` lists after the function's path.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    // `readFunction`
    /// "export is not a function"
    NotAFunction,
    /// "function export too small" (under 40 bytes).
    ExportTooSmall,
    /// "function tail has no FriendlyName"
    NoFriendlyName,
    /// "header runs into the tail"
    HeaderIntoTail,
    /// "negative script size"
    NegativeScriptSize,
    /// "ScriptSize <n> does not end at the function tail"
    ScriptSizeMismatch { size: i32 },
    /// "function payload lies outside the package data". Not a C++ text: a [`Package`] that `Package::load` read
    /// never reaches it (the export table checks every payload's bounds); a hand-made `Package` or `FunctionInfo`
    /// whose offsets point past the data does, where the C++ would read out of bounds.
    PayloadBounds,
    // `decode`
    /// "function has no script"
    NoScript,
    /// "read past the end of the script"
    PastEnd,
    /// "FName index out of range"
    NameIndex,
    /// "negative FName number"
    NegativeNameNumber,
    /// "object reference out of range"
    ReferenceRange,
    /// "call parameters run past the script end"
    ParametersPastEnd,
    /// "expression nesting too deep"
    TooDeep,
    /// "runaway label table"
    RunawayLabelTable,
    /// "unknown opcode 0x<xx>"
    UnknownOpcode(u8),
    /// "stray EndFunctionParms at statement level"
    StrayEndFunctionParms,
    /// "script ended before its stored size"
    EndedEarly,
    /// "last token is not EndOfScript"
    NoEndOfScript,
    /// "memory size <decoded> != header ScriptBytecodeSize <header>"
    MemorySize { decoded: u32, header: u32 },
    /// "jump target <n> is not a statement start"
    JumpTarget(u16),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::NotAFunction => f.write_str("export is not a function"),
            DecodeError::ExportTooSmall => f.write_str("function export too small"),
            DecodeError::NoFriendlyName => f.write_str("function tail has no FriendlyName"),
            DecodeError::HeaderIntoTail => f.write_str("header runs into the tail"),
            DecodeError::NegativeScriptSize => f.write_str("negative script size"),
            DecodeError::ScriptSizeMismatch { size } => {
                write!(f, "ScriptSize {size} does not end at the function tail")
            }
            DecodeError::PayloadBounds => f.write_str("function payload lies outside the package data"),
            DecodeError::NoScript => f.write_str("function has no script"),
            DecodeError::PastEnd => f.write_str("read past the end of the script"),
            DecodeError::NameIndex => f.write_str("FName index out of range"),
            DecodeError::NegativeNameNumber => f.write_str("negative FName number"),
            DecodeError::ReferenceRange => f.write_str("object reference out of range"),
            DecodeError::ParametersPastEnd => f.write_str("call parameters run past the script end"),
            DecodeError::TooDeep => f.write_str("expression nesting too deep"),
            DecodeError::RunawayLabelTable => f.write_str("runaway label table"),
            DecodeError::UnknownOpcode(code) => write!(f, "unknown opcode 0x{code:02x}"),
            DecodeError::StrayEndFunctionParms => f.write_str("stray EndFunctionParms at statement level"),
            DecodeError::EndedEarly => f.write_str("script ended before its stored size"),
            DecodeError::NoEndOfScript => f.write_str("last token is not EndOfScript"),
            DecodeError::MemorySize { decoded, header } => {
                write!(f, "memory size {decoded} != header ScriptBytecodeSize {header}")
            }
            DecodeError::JumpTarget(target) => write!(f, "jump target {target} is not a statement start"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// One decoded expression (C++ `script::Expr`). Operands are stored by kind, in the order the opcode's layout lists
/// them (`crate::ops::Layout`); `Let` is `kids[0] = kids[1]`, a call's parameters are its `kids`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Expr {
    pub op: u8,
    /// The native index of a call token (`op >= 0x60`).
    pub native: u32,
    /// Offset of the token in the stored script.
    pub file: u32,
    /// The same offset in the in-memory script (what jumps count in).
    pub mem: u32,
    /// Object references, relative to the function's package.
    pub refs: Vec<i32>,
    /// FNames as text (`Name_N-1` for a nonzero number).
    pub names: Vec<String>,
    pub ints: Vec<i32>,
    pub floats: Vec<f32>,
    pub bytes: Vec<u8>,
    /// u16 operands (jump and skip offsets are in-memory offsets).
    pub words: Vec<u16>,
    /// `StringConst` bytes as stored, or `UnicodeStringConst` code units written as UTF-8 one unit at a time (as the
    /// C++ `utf8()` does, so a lone surrogate gives its three-byte form). Bytes, not `String`: an ANSI constant
    /// may hold bytes that are not UTF-8, and the disassembly prints them unchanged, as the C++ does.
    pub text: Vec<u8>,
    /// Sub-expressions; for calls, the parameters.
    pub kids: Vec<Expr>,
    /// `LabelTable` entries (name, offset).
    pub labels: Vec<(String, i32)>,
}

impl Expr {
    /// Is this a native call token (`op >= 0x60`)?
    pub fn is_native_call(&self) -> bool {
        ops::is_native_call(self.op)
    }
}

/// One function's decoded script (C++ `script::Code`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Code {
    pub statements: Vec<Expr>,
    /// In-memory offset to statement index, for lookups (jump targets); the end of the script maps to
    /// `statements.len()`.
    pub statement_at: FixedHashMap<u32, usize>,
    pub memory_size: u32,
}

/// The C++ `utf8()` (`src/package.cpp`): one code point as UTF-8 bytes, with no check for surrogates.
fn push_utf8(out: &mut Vec<u8>, codepoint: u32) {
    if codepoint < 0x80 {
        out.push(codepoint as u8);
    } else if codepoint < 0x800 {
        out.push((0xc0 | (codepoint >> 6)) as u8);
        out.push((0x80 | (codepoint & 63)) as u8);
    } else if codepoint < 0x10000 {
        out.push((0xe0 | (codepoint >> 12)) as u8);
        out.push((0x80 | ((codepoint >> 6) & 63)) as u8);
        out.push((0x80 | (codepoint & 63)) as u8);
    } else {
        out.push((0xf0 | (codepoint >> 18)) as u8);
        out.push((0x80 | ((codepoint >> 12) & 63)) as u8);
        out.push((0x80 | ((codepoint >> 6) & 63)) as u8);
        out.push((0x80 | (codepoint & 63)) as u8);
    }
}

struct Decoder<'a> {
    data: &'a [u8],
    names: &'a [String],
    imports: i64,
    exports: i64,
    pos: usize,
    end: usize,
    base: usize,
    /// Object references so far: 4 bytes in the file, 8 in the in-memory script.
    refs: u32,
}

impl<'a> Decoder<'a> {
    fn need(&self, count: usize) -> Result<(), DecodeError> {
        if self.pos > self.end || count > self.end - self.pos {
            return Err(DecodeError::PastEnd);
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        self.need(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn i32(&mut self) -> Result<i32, DecodeError> {
        self.need(4)?;
        let at = self.pos;
        let v = i32::from_le_bytes([self.data[at], self.data[at + 1], self.data[at + 2], self.data[at + 3]]);
        self.pos += 4;
        Ok(v)
    }

    fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_bits(self.i32()? as u32))
    }

    fn fname(&mut self) -> Result<String, DecodeError> {
        let index = self.i32()?;
        let number = self.i32()?;
        let Some(name) = usize::try_from(index).ok().and_then(|i| self.names.get(i)) else {
            return Err(DecodeError::NameIndex);
        };
        if number < 0 {
            return Err(DecodeError::NegativeNameNumber);
        }
        Ok(if number != 0 {
            format!("{name}_{}", number - 1)
        } else {
            name.clone()
        })
    }

    fn reference(&mut self) -> Result<i32, DecodeError> {
        let value = self.i32()?;
        if i64::from(value) < -self.imports || i64::from(value) > self.exports {
            return Err(DecodeError::ReferenceRange);
        }
        self.refs = self.refs.wrapping_add(1);
        Ok(value)
    }

    /// The in-memory offset of the read position (C++ `uint32_t(pos - base) + 4 * refs`, in 32-bit arithmetic).
    fn memory_offset(&self) -> u32 {
        ((self.pos - self.base) as u32).wrapping_add(self.refs.wrapping_mul(4))
    }

    // The recursion is `expression` -> (`operands` | `parameters`) -> `expression`, up to MAX_DEPTH + 1 levels. Its
    // frames hold no `Expr` by value except the one being built (each child is pushed straight into its parent's
    // `kids`), and the leaf tokens are read in their own functions, so legal input (256 levels) stays far inside
    // a 1 MiB main-thread stack in unoptimised builds too (a unit test decodes 256 levels on a 1 MiB thread).

    /// Call parameters: expressions up to `EndFunctionParms`, pushed onto `kids`.
    fn parameters(&mut self, kids: &mut Vec<Expr>, depth: u32) -> Result<(), DecodeError> {
        loop {
            if self.pos >= self.end {
                return Err(DecodeError::ParametersPastEnd);
            }
            if self.data[self.pos] == op::END_FUNCTION_PARMS {
                self.pos += 1;
                return Ok(());
            }
            self.expression(depth + 1, kids)?;
        }
    }

    /// Decodes one expression at `depth` and pushes it onto `out`.
    #[allow(clippy::field_reassign_with_default)] // see the comment at `e`
    fn expression(&mut self, depth: u32, out: &mut Vec<Expr>) -> Result<(), DecodeError> {
        if depth > MAX_DEPTH {
            return Err(DecodeError::TooDeep);
        }
        // Fields set one by one, not with `..Expr::default()`: in an unoptimised build the struct update keeps a
        // second Expr in this recursive frame.
        let mut e = Expr::default();
        e.file = (self.pos - self.base) as u32;
        e.mem = self.memory_offset();
        e.op = self.u8()?;
        if e.op >= op::FIRST_NATIVE {
            e.native = u32::from(e.op);
            self.parameters(&mut e.kids, depth)?;
        } else if e.op >= op::EXTENDED_NATIVE {
            e.native = u32::from(e.op - op::EXTENDED_NATIVE) * 256 + u32::from(self.u8()?);
            self.parameters(&mut e.kids, depth)?;
        } else {
            match e.op {
                op::STRING_CONST => self.string(&mut e.text)?,
                op::UNICODE_STRING_CONST => self.unicode_string(&mut e.text)?,
                op::LABEL_TABLE => self.label_table(&mut e.labels)?,
                op::CASE => {
                    let next = self.u16()?;
                    e.words.push(next);
                    if next != 0xFFFF {
                        // 0xFFFF is the default case, which has no value.
                        self.expression(depth + 1, &mut e.kids)?;
                    }
                }
                op::CONDITIONAL => {
                    self.expression(depth + 1, &mut e.kids)?;
                    e.words.push(self.u16()?);
                    self.expression(depth + 1, &mut e.kids)?;
                    e.words.push(self.u16()?);
                    self.expression(depth + 1, &mut e.kids)?;
                }
                _ => self.operands(&mut e, depth)?,
            }
        }
        out.push(e);
        Ok(())
    }

    /// The operands of a token with a layout, in the layout's order.
    fn operands(&mut self, e: &mut Expr, depth: u32) -> Result<(), DecodeError> {
        let Some(layout) = ops::layout_for(e.op) else {
            return Err(DecodeError::UnknownOpcode(e.op));
        };
        for letter in layout.operands.bytes() {
            match letter {
                b'r' => {
                    let r = self.reference()?;
                    e.refs.push(r);
                }
                b'n' => self.name_operand(&mut e.names)?,
                b'i' => {
                    let v = self.i32()?;
                    e.ints.push(v);
                }
                b'f' => {
                    let v = self.f32()?;
                    e.floats.push(v);
                }
                b'b' => {
                    let v = self.u8()?;
                    e.bytes.push(v);
                }
                b'w' => {
                    let v = self.u16()?;
                    e.words.push(v);
                }
                b'E' => self.expression(depth + 1, &mut e.kids)?,
                // 'P' is the only other letter in the table (a unit test checks it).
                _ => self.parameters(&mut e.kids, depth)?,
            }
        }
        Ok(())
    }

    #[inline(never)]
    fn name_operand(&mut self, names: &mut Vec<String>) -> Result<(), DecodeError> {
        let name = self.fname()?;
        names.push(name);
        Ok(())
    }

    /// `StringConst`: bytes up to a zero byte, kept as stored.
    #[inline(never)]
    fn string(&mut self, text: &mut Vec<u8>) -> Result<(), DecodeError> {
        loop {
            let c = self.u8()?;
            if c == 0 {
                return Ok(());
            }
            text.push(c);
        }
    }

    /// `UnicodeStringConst`: u16 code units up to a zero unit, each written as UTF-8 on its own.
    #[inline(never)]
    fn unicode_string(&mut self, text: &mut Vec<u8>) -> Result<(), DecodeError> {
        loop {
            let c = self.u16()?;
            if c == 0 {
                return Ok(());
            }
            push_utf8(text, u32::from(c));
        }
    }

    /// `LabelTable`: (FName, i32 offset) entries up to the name `None`, at most [`MAX_LABELS`].
    #[inline(never)]
    fn label_table(&mut self, labels: &mut Vec<(String, i32)>) -> Result<(), DecodeError> {
        loop {
            let name = self.fname()?;
            let offset = self.i32()?;
            if name == "None" {
                return Ok(());
            }
            labels.push((name, offset));
            if labels.len() > MAX_LABELS {
                return Err(DecodeError::RunawayLabelTable);
            }
        }
    }
}

/// Decodes a function's script (C++ `script::decode`). Fails on any structural violation: unknown opcode, reading
/// past the script, the script not ending in `EndOfScript` exactly at its stored size, the in-memory size
/// disagreeing with the header, a jump or case target that is not a statement start, or nesting deeper than
/// [`MAX_DEPTH`].
pub fn decode(package: &Package, function: &FunctionInfo) -> Result<Code, DecodeError> {
    if function.script_size == 0 {
        return Err(DecodeError::NoScript);
    }
    let begin = function.script_offset;
    let end = begin
        .checked_add(function.script_size)
        .filter(|&end| end <= package.data.len())
        .ok_or(DecodeError::PayloadBounds)?;
    let mut d = Decoder {
        data: &package.data,
        names: &package.names,
        imports: package.imports.len() as i64,
        exports: package.exports.len() as i64,
        pos: begin,
        end,
        base: begin,
        refs: 0,
    };
    let mut code = Code::default();
    while d.pos < d.end {
        let memory = d.memory_offset();
        // A statement never starts with EndFunctionParms: one there means a call consumed too few operands (the
        // dynamic-array tokens carry their own terminator; this check found the layouts that missed it).
        if d.data[d.pos] == op::END_FUNCTION_PARMS {
            return Err(DecodeError::StrayEndFunctionParms);
        }
        d.expression(0, &mut code.statements)?;
        let last_is_end = code.statements.last().map(|s| s.op) == Some(op::END_OF_SCRIPT);
        code.statement_at.entry(memory).or_insert(code.statements.len() - 1);
        if last_is_end {
            break;
        }
    }
    if d.pos != d.end {
        return Err(DecodeError::EndedEarly);
    }
    if code.statements.last().map(|s| s.op) != Some(op::END_OF_SCRIPT) {
        return Err(DecodeError::NoEndOfScript);
    }
    code.memory_size = d.memory_offset();
    code.statement_at.entry(code.memory_size).or_insert(code.statements.len());
    if code.memory_size != function.memory_size {
        return Err(DecodeError::MemorySize {
            decoded: code.memory_size,
            header: function.memory_size,
        });
    }
    // Every Jump / JumpIfNot / Case target must be the start of a statement. The walk order is the C++ one (a stack:
    // the last statement first, then its last child first), so the first bad target reported is the same.
    let mut stack: Vec<&Expr> = code.statements.iter().collect();
    while let Some(e) = stack.pop() {
        let target = e.words.first().copied();
        let checked = match e.op {
            op::JUMP | op::JUMP_IF_NOT => target,
            op::CASE => target.filter(|&t| t != 0xFFFF),
            _ => None,
        };
        if let Some(t) = checked
            && !code.statement_at.contains_key(&u32::from(t))
        {
            return Err(DecodeError::JumpTarget(t));
        }
        stack.extend(e.kids.iter());
    }
    Ok(code)
}

/// Synthetic packages for this crate's tests (invented values only), built with the `ow-package` testkit.
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;

    use ow_package::package::Package;
    use ow_package::testkit::script::{Asm, DEFAULT_FLAGS, FunctionSpec, Param, ScriptToy};

    /// A toy with a class `Thing` (export 1) and an int property `Thing.Count` (export 2).
    pub fn toy() -> ScriptToy {
        let mut t = ScriptToy::new(&["Int", "Float"]);
        let thing = t.class("Thing", 0);
        t.prop("Int", thing, "Count", 0, 0);
        t
    }

    /// Adds `Thing.<name>(int A, int B)` with `body` and `spec`; returns the function's export index.
    pub fn add(t: &mut ScriptToy, name: &str, body: Option<Asm>, spec: &FunctionSpec) -> i32 {
        let params = [Param::new("Int", "A"), Param::new("Int", "B")];
        t.function(1, name, &params, None, &[], spec, |_| body).function
    }

    pub fn load(t: &mut ScriptToy) -> Package {
        Package::from_decoded(PathBuf::new(), "Toy".into(), t.build()).expect("the toy package loads")
    }

    /// A package holding one script function `Thing.F` with `script` as its body, the in-memory size taken from
    /// the body unless `memory` is given. Exports: 1 Thing, 2 Count, 3 B, 4 A, 5 F.
    pub fn one(script: Asm, memory: Option<i32>) -> (Package, i32) {
        let mut t = toy();
        let spec = FunctionSpec { memory_size: memory, ..FunctionSpec::new(DEFAULT_FLAGS, "F") };
        let index = add(&mut t, "F", Some(script), &spec);
        (load(&mut t), index)
    }

    pub fn raw(bytes: &[u8]) -> Asm {
        let mut a = Asm::new();
        a.raw(bytes);
        a
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{one, raw};
    use super::*;
    use crate::disasm::format;
    use crate::function::read_function;
    use ow_package::testkit::script::Asm;

    fn decoded(script: Asm, memory: Option<i32>) -> Result<(Package, Code), DecodeError> {
        let (package, index) = one(script, memory);
        let info = read_function(&package, index)?;
        let code = decode(&package, &info)?;
        Ok((package, code))
    }

    fn lines(script: Asm) -> Vec<String> {
        let (package, code) = decoded(script, None).expect("decodes");
        code.statements
            .iter()
            .map(|s| String::from_utf8(format(&package, s)).expect("UTF-8 in these tests"))
            .collect()
    }

    fn error(script: Asm, memory: Option<i32>) -> DecodeError {
        match decoded(script, memory) {
            Ok(_) => panic!("decoded, expected a refusal"),
            Err(e) => e,
        }
    }

    // The cases of tests/script_disasm_test.py, on the C++ decoder's text (`format`).

    #[test]
    fn constants_and_return() {
        let mut a = Asm::new();
        a.raw(&[0x04]).int_const(5).end();
        assert_eq!(lines(a), ["Return(IntConst(5))", "EndOfScript"]);
    }

    #[test]
    fn native_calls_end_at_end_of_function_parms() {
        assert_eq!(lines(raw(&[0x70, 0x26, 0x25, 0x16, 0x53]))[0], "native_112(IntOne, IntZero)");
    }

    #[test]
    fn extended_native_index_is_two_bytes() {
        // (0x61 - 0x60) * 256 + 0x17 = 279
        let (_, code) = decoded(raw(&[0x61, 0x17, 0x16, 0x53]), None).unwrap();
        assert_eq!(code.statements[0].native, 279);
        assert_eq!(lines(raw(&[0x61, 0x17, 0x16, 0x53]))[0], "native_279()");
    }

    #[test]
    fn strings() {
        let mut a = raw(b"\x1fhi\0\x34");
        a.raw(&[0xE9, 0x00, 0x00, 0x00]).raw(&[0x53]);
        assert_eq!(lines(a)[..2], ["\"hi\"", "\"\u{e9}\""]);
    }

    #[test]
    fn string_bytes_are_kept_as_stored() {
        // An ANSI byte that is not UTF-8 passes through; a lone surrogate gets its three-byte form (C++ `utf8()`);
        // a control byte and a quote get the C++ `quote()` escapes.
        let mut a = raw(b"\x1f\xe9\x01\"\0\x34");
        a.raw(&[0x00, 0xD8, 0x00, 0x00, 0x53]);
        let (package, code) = decoded(a, None).unwrap();
        assert_eq!(code.statements[0].text, b"\xe9\x01\"");
        assert_eq!(format(&package, &code.statements[0]), b"\"\xe9\\u0001\\\"\"");
        assert_eq!(code.statements[1].text, b"\xed\xa0\x80");
    }

    #[test]
    fn unknown_opcode_is_a_decode_error() {
        for code in [0x2B, 0x5B, 0x5C, 0x5D] {
            assert_eq!(error(raw(&[code, 0x53]), None), DecodeError::UnknownOpcode(code));
        }
        assert_eq!(DecodeError::UnknownOpcode(0x2B).to_string(), "unknown opcode 0x2b");
    }

    // MemoryOffsets: object references are 4 bytes in the file but 8 in the in-memory script that jumps count in.

    #[test]
    fn reference_adds_four_bytes_of_memory() {
        let script = || {
            let mut a = Asm::new();
            a.raw(&[0x04]).local(1).raw(&[0x53]); // Return(LocalVariable) EndOfScript: 7 bytes in the file
            a
        };
        let (_, code) = decoded(script(), Some(11)).unwrap();
        assert_eq!(code.memory_size, 11);
        let e = error(script(), Some(7));
        assert_eq!(e, DecodeError::MemorySize { decoded: 11, header: 7 });
        assert_eq!(e.to_string(), "memory size 11 != header ScriptBytecodeSize 7");
    }

    #[test]
    fn jump_target_must_be_a_statement_start() {
        let ok = raw(&[0x06, 5, 0, 0x04, 0x25, 0x53]); // Jump to the EndOfScript at offset 5
        decoded(ok, Some(6)).unwrap();
        let bad = raw(&[0x06, 4, 0, 0x04, 0x25, 0x53]); // offset 4 is inside Return(IntZero)
        let e = error(bad, Some(6));
        assert_eq!(e, DecodeError::JumpTarget(4));
        assert_eq!(e.to_string(), "jump target 4 is not a statement start");
    }

    #[test]
    fn jump_targets_are_measured_in_memory_offsets() {
        // Let(LocalVariable, LocalVariable) takes 11 file bytes but 19 memory bytes, so the jump that skips it
        // targets memory offset 3 + 19 = 22, not file offset 14.
        let mut a = raw(&[0x06]);
        a.w(22).raw(&[0x0F]).local(1).local(2).raw(&[0x53]);
        let (_, code) = decoded(a, None).unwrap();
        assert_eq!(code.memory_size, 15 + 8);
        assert_eq!(code.statement_at.get(&22), Some(&2));
        assert_eq!(code.statement_at.get(&23), Some(&3));
        assert_eq!((code.statements[1].file, code.statements[1].mem), (3, 3));
        assert_eq!((code.statements[2].file, code.statements[2].mem), (14, 22));
    }

    // The other C++ checks, one by one.

    #[test]
    fn the_end_of_the_script_is_checked() {
        assert_eq!(error(raw(&[0x1D, 1, 2]), None), DecodeError::PastEnd); // IntConst cut short
        assert_eq!(error(raw(&[0x04, 0x25]), None), DecodeError::NoEndOfScript);
        assert_eq!(error(raw(&[0x53, 0x0B]), None), DecodeError::EndedEarly);
        assert_eq!(error(raw(&[0x16, 0x53]), None), DecodeError::StrayEndFunctionParms);
        assert_eq!(error(raw(&[0x70, 0x25]), None), DecodeError::ParametersPastEnd);
        assert_eq!(error(raw(&[0x1F, b'a']), None), DecodeError::PastEnd); // an unterminated string
    }

    #[test]
    fn names_and_references_are_checked() {
        // NameConst with name index 99 (out of range), then with number -1.
        let mut a = raw(&[0x21]);
        a.i32(99).i32(0).raw(&[0x53]);
        assert_eq!(error(a, None), DecodeError::NameIndex);
        let mut a = raw(&[0x21]);
        a.i32(0).i32(-1).raw(&[0x53]);
        assert_eq!(error(a, None), DecodeError::NegativeNameNumber);
        // The toy has 5 exports and 6 imports: 6 and -7 are out of range, 5 and -6 in range.
        for r in [6, -7] {
            let mut a = raw(&[0x20]);
            a.reference(r).raw(&[0x53]);
            assert_eq!(error(a, None), DecodeError::ReferenceRange);
        }
        let mut a = raw(&[0x20]);
        a.reference(5).raw(&[0x20]).reference(-6).raw(&[0x53]);
        decoded(a, None).unwrap();
    }

    #[test]
    fn nesting_deeper_than_256_is_refused() {
        // The statement is at depth 0, so 256 nested Returns put the IntZero at depth 256, 257 at depth 257.
        let nested = |n: usize| {
            let mut bytes = vec![0x04; n];
            bytes.extend_from_slice(&[0x25, 0x53]);
            raw(&bytes)
        };
        // On a 1 MiB thread, the default stack of a Windows main thread: 256 levels needed 512-768 KiB in an
        // unoptimised build and 128-256 KiB in a release build (measured 2026-11-15 on pc-hummer).
        let deepest = std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(move || decoded(nested(256), None).map(|(_, code)| code.statements.len()))
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(deepest, Ok(2));
        assert_eq!(error(nested(257), None), DecodeError::TooDeep);
    }

    #[test]
    fn label_tables_end_at_none_and_have_a_bound() {
        let (package, _) = one(raw(&[0x53]), None);
        let count = package.names.iter().position(|n| n == "Count").unwrap() as i32;
        let none = package.names.iter().position(|n| n == "None").unwrap() as i32;
        // LabelTable(Count_1@7), ended by None.
        let mut a = raw(&[0x0C]);
        a.i32(count).i32(2).i32(7).i32(none).i32(0).i32(-1).raw(&[0x53]);
        assert_eq!(lines(a)[0], "LabelTable[Count_1@7]");
        let mut a = raw(&[0x0C]);
        for _ in 0..=MAX_LABELS {
            a.i32(count).i32(0).i32(0);
        }
        assert_eq!(error(a, None), DecodeError::RunawayLabelTable);
    }

    #[test]
    fn case_default_and_conditional() {
        // Switch(Count, 4, IntZero) at 0; Case(15, IntOne) at 11; Case(default) at 15; Case(default); Conditional(True, 0, IntZero, 0, IntOne).
        let mut a = raw(&[0x05]);
        a.reference(2).raw(&[4, 0x25]);
        a.raw(&[0x0A]).w(15).raw(&[0x26]);
        a.raw(&[0x0A]).w(0xFFFF);
        a.raw(&[0x45, 0x27]).w(0).raw(&[0x25]).w(0).raw(&[0x26]);
        a.raw(&[0x53]);
        assert_eq!(lines(a), [
            "Switch(Thing.Count, 4, IntZero)",
            "Case(15, IntOne)",
            "Case(65535)",
            "Conditional(0, 0, True, IntZero, IntOne)",
            "EndOfScript"
        ]);
        let mut a = raw(&[0x0A]);
        a.w(3).raw(&[0x26, 0x53]);
        assert_eq!(error(a, None), DecodeError::JumpTarget(3));
    }

    #[test]
    fn the_first_bad_target_is_the_cxx_one() {
        // Two bad jumps: the C++ walks a stack, so the last statement is checked first.
        assert_eq!(error(raw(&[0x06, 1, 0, 0x06, 2, 0, 0x53]), None), DecodeError::JumpTarget(2));
    }

    #[test]
    fn no_script_is_an_error() {
        let (package, index) = one(Asm::new(), None);
        let info = read_function(&package, index).unwrap();
        assert!(!info.has_script());
        assert_eq!(decode(&package, &info), Err(DecodeError::NoScript));
    }

    #[test]
    fn a_function_info_past_the_data_is_refused() {
        let (package, index) = one(raw(&[0x53]), None);
        let mut info = read_function(&package, index).unwrap();
        info.script_offset = package.data.len();
        assert_eq!(decode(&package, &info), Err(DecodeError::PayloadBounds));
    }

    #[test]
    fn every_layout_letter_is_known() {
        for code in 0u8..=255 {
            if let Some(layout) = ops::layout_for(code) {
                assert!(layout.operands.bytes().all(|b| b"rnifbwEP".contains(&b)), "0x{code:02x}");
            }
        }
    }
}
