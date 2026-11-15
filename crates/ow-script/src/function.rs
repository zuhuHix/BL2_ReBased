//! Owner: lane W2-SCRIPT. A function export's header and tail: the port of `FunctionInfo`, `isFunctionExport` and
//! `readFunction` of `src/script.cpp`.
//!
//! The layout is the one the C++ port recovered from the packages (`docs/verification/SCRIPT_BYTECODE_DISASM.md`,
//! `NATIVE_BYTECODE_OPCODES.md` for the local-variable array), copied here as it is:
//!
//! - header: `[count u16][count u16 values][44 bytes][ScriptBytecodeSize i32][ScriptSize i32]`, then the script;
//! - tail: `[iNative u16][OperPrecedence u8][FunctionFlags u32][RepOffset u16 if FUNC_Net][FriendlyName FName]`.
//!
//! `FunctionFlags` sits at a different distance from the end depending on `FUNC_Net`, which is itself one of the
//! flags; the FriendlyName must be a valid name for the tail to be accepted, and the script size must end exactly at
//! the tail.

use ow_package::package::Package;

use crate::decode::DecodeError;
use crate::ops::flags::{FUNC_NATIVE, FUNC_NET};

/// The smallest function export the reader accepts (the C++ bound).
pub const MIN_FUNCTION_SIZE: i32 = 40;

/// One function export's header and tail (C++ `script::FunctionInfo`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionInfo {
    /// The export index in its package.
    pub index: i32,
    /// The export name.
    pub name: String,
    /// FriendlyName from the tail (the operator symbol for operators), without its number.
    pub friendly_name: String,
    pub flags: u32,
    /// `iNative` (0 for script functions).
    pub native: u16,
    pub operator_precedence: u8,
    /// `ScriptBytecodeSize`: the in-memory size, in which object references take 8 bytes.
    pub memory_size: u32,
    /// Offset of the script bytes inside the package data.
    pub script_offset: usize,
    /// `ScriptSize`: the stored size.
    pub script_size: usize,
    /// The Gearbox local-variable array of the header (`NATIVE_BYTECODE_OPCODES.md`): N 16-bit values read as N/2
    /// pairs (frame offset, tag), one per hidden typed temporary. Opcode 4C-50 operand 2k+1 selects pair k. Empty
    /// for natives and for functions without temporaries.
    pub hidden_locals: Vec<u16>,
}

impl FunctionInfo {
    /// Does the function carry a script (natives and empty bodies do not)?
    pub fn has_script(&self) -> bool {
        self.script_size != 0
    }
}

/// Is export `index` a UFunction: is its class an object named `Function`? (C++ `isFunctionExport`: the class
/// reference is resolved in this package and its name compared; a bad reference is no function.)
pub fn is_function_export(package: &Package, index: i32) -> bool {
    if index <= 0 || index as usize > package.exports.len() {
        return false;
    }
    let class = package.exports[index as usize - 1].class;
    class != 0 && package.object(class).is_ok_and(|c| c.name == "Function")
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, DecodeError> {
    match data.get(at..at.wrapping_add(2)) {
        Some(b) if b.len() == 2 => Ok(u16::from_le_bytes([b[0], b[1]])),
        _ => Err(DecodeError::PayloadBounds),
    }
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, DecodeError> {
    match data.get(at..at.wrapping_add(4)) {
        Some(b) if b.len() == 4 => Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        _ => Err(DecodeError::PayloadBounds),
    }
}

/// Reads the header and tail of function export `index` (C++ `readFunction`). Natives return after the tail: only
/// flags, `iNative`, precedence and FriendlyName are meaningful for them.
pub fn read_function(package: &Package, index: i32) -> Result<FunctionInfo, DecodeError> {
    if !is_function_export(package, index) {
        return Err(DecodeError::NotAFunction);
    }
    let export = &package.exports[index as usize - 1];
    let data = package.data.as_slice();
    let (Ok(start), Ok(size)) = (usize::try_from(export.offset), usize::try_from(export.size)) else {
        return Err(DecodeError::PayloadBounds);
    };
    let finish = start.checked_add(size).ok_or(DecodeError::PayloadBounds)?;
    if export.size < MIN_FUNCTION_SIZE {
        return Err(DecodeError::ExportTooSmall);
    }
    if finish > data.len() {
        return Err(DecodeError::PayloadBounds);
    }
    // From here every offset lies in start..finish: the export is at least 40 bytes, the tail at most 17.
    let names = &package.names;
    let valid_name = |at: usize| -> Result<bool, DecodeError> {
        let name_index = u32_at(data, at)? as i32;
        let number = u32_at(data, at + 4)? as i32;
        Ok(name_index >= 0 && (name_index as usize) < names.len() && (0..100_000).contains(&number))
    };
    if !valid_name(finish - 8)? {
        return Err(DecodeError::NoFriendlyName);
    }
    let mut flags = u32_at(data, finish - 12)?;
    let mut tail = 15;
    let net_flags = u32_at(data, finish - 14)?;
    if net_flags & FUNC_NET != 0 {
        flags = net_flags;
        tail = 17;
    }
    let friendly_index = u32_at(data, finish - 8)? as usize;
    let tail_start = finish - tail;
    let mut info = FunctionInfo {
        index,
        name: export.name.clone(),
        friendly_name: names[friendly_index].clone(),
        flags,
        native: u16_at(data, tail_start)?,
        operator_precedence: data[tail_start + 2],
        ..FunctionInfo::default()
    };
    // Natives have no script: only the tail (flags, iNative, FriendlyName) is meaningful.
    if flags & FUNC_NATIVE != 0 {
        return Ok(info);
    }

    let count = usize::from(u16_at(data, start)?);
    let size_at = start + 2 + 2 * count + 44;
    if size_at + 4 > tail_start {
        return Err(DecodeError::HeaderIntoTail);
    }
    for i in 0..count {
        info.hidden_locals.push(u16_at(data, start + 2 + 2 * i)?);
    }
    let memory = u32_at(data, size_at - 4)? as i32;
    let script_size = u32_at(data, size_at)? as i32;
    if script_size < 0 || memory < 0 {
        return Err(DecodeError::NegativeScriptSize);
    }
    if size_at + 4 + script_size as usize != tail_start {
        return Err(DecodeError::ScriptSizeMismatch { size: script_size });
    }
    info.memory_size = memory as u32;
    info.script_offset = size_at + 4;
    info.script_size = script_size as usize;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::test_support::{add, load, raw, toy};
    use ow_package::testkit::script::{DEFAULT_FLAGS, FunctionSpec};
    use ow_package::testkit::{Export, ints};

    const BODY: &[u8] = &[0x04, 0x25, 0x53];

    fn read(spec: &FunctionSpec) -> (Package, Result<FunctionInfo, DecodeError>) {
        let mut t = toy();
        let index = add(&mut t, "F", Some(raw(BODY)), spec);
        let package = load(&mut t);
        let info = read_function(&package, index);
        (package, info)
    }

    /// The function `Thing.F` with its payload changed by `edit` after it was written.
    fn read_edited(spec: &FunctionSpec, edit: impl FnOnce(&mut Vec<u8>)) -> Result<FunctionInfo, DecodeError> {
        let mut t = toy();
        let index = add(&mut t, "F", Some(raw(BODY)), spec);
        let mut payload = ow_package::testkit::script::function_payload(&mut t.p, Some(&raw(BODY)), spec);
        edit(&mut payload);
        t.p.set_payload(index, payload);
        let package = load(&mut t);
        read_function(&package, index)
    }

    // The FunctionHeader cases of tests/script_disasm_test.py.

    #[test]
    fn local_variable_array_shifts_the_script_size_field() {
        for count in [0usize, 2, 6] {
            let spec = FunctionSpec {
                hidden_locals: vec![0x10; count],
                header_words: [7; 10],
                ..FunctionSpec::new(0x2003, "F")
            };
            let (package, info) = read(&spec);
            let info = info.unwrap();
            assert_eq!((info.memory_size, info.script_size), (3, 3));
            assert_eq!(&package.data[info.script_offset..info.script_offset + 3], BODY);
            assert_eq!(info.hidden_locals, vec![0x10; count]);
        }
    }

    #[test]
    fn net_functions_have_a_longer_tail() {
        let spec = FunctionSpec { rep_offset: Some(0), ..FunctionSpec::new(0x2043, "F") };
        let (package, info) = read(&spec);
        let info = info.unwrap();
        assert_eq!(info.flags, 0x2043);
        assert_eq!(&package.data[info.script_offset..info.script_offset + info.script_size], BODY);
    }

    #[test]
    fn script_size_must_end_exactly_at_the_tail() {
        let e = read_edited(&FunctionSpec::new(0x2003, "F"), |p| p.extend_from_slice(&[0, 0]));
        assert_eq!(e, Err(DecodeError::ScriptSizeMismatch { size: 3 }));
        assert_eq!(e.unwrap_err().to_string(), "ScriptSize 3 does not end at the function tail");
    }

    // The rest of readFunction.

    #[test]
    fn the_tail_of_a_script_function() {
        let spec = FunctionSpec { precedence: 16, native: 0, ..FunctionSpec::new(DEFAULT_FLAGS, "Friendly") };
        let (_, info) = read(&spec);
        let info = info.unwrap();
        assert_eq!(
            (info.index, info.name.as_str(), info.friendly_name.as_str(), info.flags, info.operator_precedence),
            (5, "F", "Friendly", DEFAULT_FLAGS, 16)
        );
        assert!(info.has_script());
    }

    #[test]
    fn natives_stop_after_the_tail() {
        let spec = FunctionSpec { native: 146, hidden_locals: vec![1, 2], ..FunctionSpec::new(0x400 | 0x1000, "+") };
        let (_, info) = read(&spec);
        let info = info.unwrap();
        assert_eq!((info.native, info.friendly_name.as_str()), (146, "+"));
        assert_eq!((info.script_size, info.memory_size, info.hidden_locals.len()), (0, 0, 0));
    }

    #[test]
    fn only_function_exports_are_read() {
        let (package, _) = read(&FunctionSpec::new(DEFAULT_FLAGS, "F"));
        for index in [0, 1, 2, 6, -1] {
            assert!(!is_function_export(&package, index));
            assert_eq!(read_function(&package, index), Err(DecodeError::NotAFunction));
        }
        assert!(is_function_export(&package, 5));
    }

    #[test]
    fn small_exports_and_bad_tails_are_refused() {
        let mut t = toy();
        let function = t.import("Function");
        let small = t.p.add_export_with(Export::new(function, "Small", vec![0u8; 39]));
        let package = load(&mut t);
        assert_eq!(read_function(&package, small), Err(DecodeError::ExportTooSmall));

        let spec = FunctionSpec::new(DEFAULT_FLAGS, "F");
        let no_name = |p: &mut Vec<u8>| {
            let at = p.len() - 8;
            p[at..].copy_from_slice(&ints(&[999, 0]));
        };
        assert_eq!(read_edited(&spec, no_name), Err(DecodeError::NoFriendlyName));
        let big_number = |p: &mut Vec<u8>| {
            let at = p.len() - 4;
            p[at..].copy_from_slice(&ints(&[100_000]));
        };
        assert_eq!(read_edited(&spec, big_number), Err(DecodeError::NoFriendlyName));
        let long_array = |p: &mut Vec<u8>| p[..2].copy_from_slice(&1000u16.to_le_bytes());
        assert_eq!(read_edited(&spec, long_array), Err(DecodeError::HeaderIntoTail));
        let negative = FunctionSpec { memory_size: Some(-1), ..spec.clone() };
        assert_eq!(read(&negative).1, Err(DecodeError::NegativeScriptSize));
    }

    #[test]
    fn a_hand_made_package_past_its_data_is_refused() {
        let (mut package, _) = read(&FunctionSpec::new(DEFAULT_FLAGS, "F"));
        package.exports[4].offset = package.data.len() as i32;
        assert_eq!(read_function(&package, 5), Err(DecodeError::PayloadBounds));
    }
}
