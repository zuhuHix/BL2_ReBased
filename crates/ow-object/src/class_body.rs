//! Owner: lane W2-REFLECT. The Class export body: the port of `decodeClassBody` (`src/vm.cpp`) and its `ClassBody`
//! and `InterfaceEntry` (`src/vm.hpp`).
//!
//! The layout is the one `docs/verification/NATIVE_CLASS_SERIAL_LAYOUT.md` records and the C++ decoder reads, field
//! for field and check for check: a base-object part, a struct part (with its script bytes), a state part, then the
//! class part with the interface table, the editor-only lists and the default object reference. The body must end
//! exactly at the export's serial size. Nothing about the format is re-derived here (AGENTS.md rule 4); fields the
//! C++ only consumes are only consumed here.
//!
//! Status: the byte layout is checked structurally (`ow-package --class-check`: every Class export of the nine code
//! packages decodes exactly, on the C++ reader and on this one). What the fields mean, and any cast result built on
//! them, is UNVERIFIED in the running game (NATIVE_CLASS_SERIAL_LAYOUT.md).

use std::fmt;

use ow_package::package::Package;
use ow_package::reader::{ReadError, Reader};
use ow_package::tables;

/// Class flag bit 14 marks an interface class (C++ `CLASS_Interface`).
pub const CLASS_INTERFACE: u32 = 0x4000;

/// Class flag bit 7 (value 128) marks a native class. **Fitted, UNVERIFIED** (lane FOLLOWUP-602-CLASS-POLICY; the hub
/// accepted the fit on #640). The structural oracle is a census:
/// - **Package set:** the code packages that `reflect::load_code_packages` loads from the installed `CookedPCConsole`.
///   Of their class exports, 3,339 bodies decode, and the bit is set on 2,547 of them.
/// - **The native-function classes:** the world `--equip-run` builds (`ow-cli` `run_world`, Off) had 123 classes with a
///   decoded body loaded. 101 of them own a function whose declaration carries a native key (`FunctionDecl::is_native`,
///   its `owner` being the class). The bit is set on all 101.
///
/// It is a census, not a proof that a class without the bit has no native code. The census was a local, uncommitted
/// probe; rerunning it means walking those two sets the same way.
pub const CLASS_NATIVE: u32 = 1 << 7;

/// The end-of-script token a non-empty class script ends in (`script::EX_EndOfScript` in `src/script.hpp`; the
/// opcode table is `ow-script`'s, which this crate may not depend on).
pub const EX_END_OF_SCRIPT: u8 = 0x53;

/// The largest count a body's list may hold (the C++ `count()` bound).
pub const MAX_COUNT: i32 = 100_000;

/// One entry of a class's interface table: the interface class and the property that holds its table pointer
/// (`VfTable_<Interface>`, 0 for a native-only table). Both are references into the declaring package, as read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceEntry {
    pub class_ref: i32,
    pub table_property: i32,
}

/// The part of a Class export body that is kept (C++ `ClassBody`). The rest is consumed for its size only.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassBody {
    pub class_flags: u32,
    /// The within class (a reference).
    pub within: i32,
    /// `Default__<Class>` (a reference; the oracle checks it is an export of this class).
    pub default_object: i32,
    pub config_name: String,
    /// The interfaces this class names itself, in table order; super classes are not repeated.
    pub interfaces: Vec<InterfaceEntry>,
}

impl ClassBody {
    /// Whether class flag bit 14 is set (C++ `Class::isInterface`, once the body decoded).
    pub fn is_interface(&self) -> bool {
        self.class_flags & CLASS_INTERFACE != 0
    }

    /// Whether the fitted native-class bit is set ([`CLASS_NATIVE`], UNVERIFIED).
    pub fn is_native(&self) -> bool {
        self.class_flags & CLASS_NATIVE != 0
    }
}

/// Why a Class export body did not decode. `Display` gives the C++ exception text, which `--class-check` prints.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassBodyError {
    /// "class export index out of range"
    IndexOutOfRange,
    /// "class export has a negative offset or size"
    NegativeOffsetOrSize,
    /// "class body super reference differs from the export table"
    SuperDiffers,
    /// "class body has a negative script size"
    NegativeScriptSize,
    /// "class body script does not end in EndOfScript"
    ScriptNotTerminated,
    /// "class body count out of range"
    CountOutOfRange,
    /// "class body interface entry names no class"
    InterfaceWithoutClass,
    /// "class body does not end at the serial size"
    TrailingBytes,
    /// A read that failed (past the serial size, a reference outside the tables, a bad name or string). Prints as
    /// the reader's text, as the C++ rethrows it unchanged.
    Read(ReadError),
}

impl fmt::Display for ClassBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ClassBodyError::IndexOutOfRange => "class export index out of range",
            ClassBodyError::NegativeOffsetOrSize => "class export has a negative offset or size",
            ClassBodyError::SuperDiffers => {
                "class body super reference differs from the export table"
            }
            ClassBodyError::NegativeScriptSize => "class body has a negative script size",
            ClassBodyError::ScriptNotTerminated => "class body script does not end in EndOfScript",
            ClassBodyError::CountOutOfRange => "class body count out of range",
            ClassBodyError::InterfaceWithoutClass => "class body interface entry names no class",
            ClassBodyError::TrailingBytes => "class body does not end at the serial size",
            ClassBodyError::Read(e) => return e.fmt(f),
        })
    }
}

impl std::error::Error for ClassBodyError {}

impl From<ReadError> for ClassBodyError {
    fn from(e: ReadError) -> Self {
        ClassBodyError::Read(e)
    }
}

/// The body reader: the C++ lambdas `ref`, `name`, `u16`, `count`, `nameList` over one export's bytes.
struct Body<'a> {
    r: Reader<'a>,
    package: &'a Package,
    imports: i32,
    exports: i32,
}

impl Body<'_> {
    fn reference(&mut self) -> Result<i32, ReadError> {
        self.r.reference(self.imports, self.exports)
    }

    fn name(&mut self) -> Result<String, ReadError> {
        tables::read_name(&mut self.r, &self.package.names)
    }

    fn u16(&mut self) -> Result<u16, ReadError> {
        let b = self.r.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn count(&mut self) -> Result<i32, ClassBodyError> {
        let n = self.r.i32()?;
        if !(0..=MAX_COUNT).contains(&n) {
            return Err(ClassBodyError::CountOutOfRange);
        }
        Ok(n)
    }

    fn name_list(&mut self) -> Result<(), ClassBodyError> {
        for _ in 0..self.count()? {
            self.name()?;
        }
        Ok(())
    }
}

/// Decodes the body of Class export `index` and checks that it ends exactly at the export's serial size (C++
/// `decodeClassBody`). Any violation is an error: a reference outside the tables, a negative or oversized count, a
/// bad name, missing or trailing bytes.
pub fn decode_class_body(package: &Package, index: i32) -> Result<ClassBody, ClassBodyError> {
    let exported = usize::try_from(index)
        .ok()
        .filter(|&i| i >= 1)
        .and_then(|i| package.exports.get(i - 1))
        .ok_or(ClassBodyError::IndexOutOfRange)?;
    if exported.size < 0 || exported.offset < 0 {
        return Err(ClassBodyError::NegativeOffsetOrSize);
    }
    let start = exported.offset as usize;
    // `size_t` arithmetic in the C++; both are non-negative i32 here, so the sum cannot overflow a 64-bit usize.
    let mut r = Reader::new(&package.data).with_limit(start + exported.size as usize);
    r.set_pos(start);
    // A table length past i32 cannot come from a package (its counts are i32); refuse it like a bad reference.
    let imports = i32::try_from(package.imports.len()).map_err(|_| ReadError::InvalidReference)?;
    let exports = i32::try_from(package.exports.len()).map_err(|_| ReadError::InvalidReference)?;
    let mut b = Body {
        r,
        package,
        imports,
        exports,
    };

    // Base object and struct part.
    b.r.i32()?; // unknown reference, deliberately not range checked (AkAudio.WwiseSoundVolume points one past the table)
    b.reference()?; // next field of the field chain (null for classes)
    if b.reference()? != exported.super_ {
        return Err(ClassBodyError::SuperDiffers);
    }
    b.reference()?; // script-text slot (null)
    b.reference()?; // first child
    b.reference()?; // C++-text slot (null)
    b.r.i32()?; // source line (unknown meaning)
    b.r.i32()?; // text position (unknown meaning)
    let memory_size = b.r.i32()?;
    let script_size = b.r.i32()?;
    if memory_size < 0 || script_size < 0 {
        return Err(ClassBodyError::NegativeScriptSize);
    }
    let script_size = script_size as usize;
    b.r.require(script_size)?;
    if script_size != 0 && b.r.byte(b.r.pos() + script_size - 1)? != EX_END_OF_SCRIPT {
        return Err(ClassBodyError::ScriptNotTerminated);
    }
    b.r.skip(script_size)?;

    // State part.
    b.r.u32()?; // probe mask
    b.u16()?; // label table offset
    b.u16()?; // state flags
    for _ in 0..b.count()? {
        // function map: (name, function export)
        b.name()?;
        b.reference()?;
    }

    // Class part.
    let class_flags = b.r.u32()?;
    let within = b.reference()?;
    let config_name = b.name()?;
    for _ in 0..b.count()? {
        // component map: (name, default subobject export or null)
        b.name()?;
        b.reference()?;
    }
    let mut interfaces = Vec::new();
    for _ in 0..b.count()? {
        let class_ref = b.reference()?;
        let table_property = b.reference()?;
        if class_ref == 0 {
            return Err(ClassBodyError::InterfaceWithoutClass);
        }
        interfaces.push(InterfaceEntry {
            class_ref,
            table_property,
        });
    }
    for _ in 0..4 {
        // four name lists: (version >= 603), hide categories, auto-expand categories (guess), unused
        b.name_list()?;
    }
    b.r.u32()?; // unknown number (0 or 1)
    b.name_list()?; // name list (version >= 789)
    b.r.string()?; // group label (unknown meaning)
    b.name()?; // discarded name
    b.r.skip(1)?; // unknown byte
    let default_object = b.reference()?;
    if b.r.pos() != b.r.limit() {
        return Err(ClassBodyError::TrailingBytes);
    }
    Ok(ClassBody {
        class_flags,
        within,
        default_object,
        config_name,
        interfaces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_package::testkit::PackageBuilder;
    use ow_package::testkit::reflect::{ClassBodySpec, class_body};
    use std::path::PathBuf;

    fn load(p: &mut PackageBuilder) -> Package {
        Package::from_decoded(PathBuf::new(), "Core".into(), p.build())
            .expect("synthetic package loads")
    }

    /// A package with Object (a decodable body) and its default object, then a second class `Thing` whose body is
    /// built from `spec` and then changed by `change`.
    fn one_class(
        spec: impl FnOnce(&mut ClassBodySpec),
        change: impl FnOnce(&mut Vec<u8>),
    ) -> (Package, i32) {
        let mut p = PackageBuilder::new_empty();
        let object_body = class_body(&mut p, &ClassBodySpec::new(0, 1, 2));
        p.add_export(0, "Object", object_body);
        let none = p.fname("None", 0);
        p.add_export(1, "Default__Object", [ints(0), none.to_vec()].concat());
        let mut s = ClassBodySpec::new(1, 1, 4);
        spec(&mut s);
        let mut body = class_body(&mut p, &s);
        change(&mut body);
        let mut thing = ow_package::testkit::Export::new(0, "Thing", body);
        thing.super_ = 1;
        let index = p.add_export_with(thing);
        p.add_export(index, "Default__Thing", [ints(0), none.to_vec()].concat());
        (load(&mut p), index)
    }

    fn ints(v: i32) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    #[test]
    fn a_body_decodes_to_its_kept_fields() {
        let (pkg, index) = one_class(
            |s| {
                s.flags = 2 | CLASS_INTERFACE;
                s.interfaces = vec![(1, 0)];
                s.script = vec![0x0b, EX_END_OF_SCRIPT];
            },
            |_| {},
        );
        let body = decode_class_body(&pkg, index).expect("decodes");
        assert_eq!(body.class_flags, 2 | CLASS_INTERFACE);
        assert!(body.is_interface());
        assert_eq!((body.within, body.default_object), (1, 4));
        assert_eq!(body.config_name, "None");
        assert_eq!(
            body.interfaces,
            vec![InterfaceEntry {
                class_ref: 1,
                table_property: 0
            }]
        );
        assert_eq!(
            decode_class_body(&pkg, 1)
                .expect("Object decodes")
                .default_object,
            2
        );
    }

    /// The malformed bodies `tests/vm_test.py` rejects (its `mutate` cases), plus every other C++ check.
    #[test]
    fn every_check_of_the_cxx_decoder_refuses() {
        let refuse = |spec: fn(&mut ClassBodySpec), change: fn(&mut Vec<u8>)| {
            let (pkg, index) = one_class(spec, change);
            decode_class_body(&pkg, index).unwrap_err()
        };
        assert_eq!(refuse(|_| {}, |b| b.push(0)), ClassBodyError::TrailingBytes);
        assert_eq!(
            refuse(|_| {}, |b| b.truncate(b.len() - 1)),
            ClassBodyError::Read(ReadError::Truncated)
        );
        assert_eq!(
            refuse(|_| {}, |b| b[8..12].copy_from_slice(&3i32.to_le_bytes())),
            ClassBodyError::SuperDiffers
        );
        // The first word is not range checked.
        let (pkg, index) = one_class(
            |_| {},
            |b| b[0..4].copy_from_slice(&99_999i32.to_le_bytes()),
        );
        assert!(decode_class_body(&pkg, index).is_ok());
        // A reference outside the tables (the next-field word).
        assert_eq!(
            refuse(
                |_| {},
                |b| b[4..8].copy_from_slice(&99_999i32.to_le_bytes())
            ),
            ClassBodyError::Read(ReadError::InvalidReference)
        );
        // Negative function map count (bytes 48..52 with an empty script), and one past the bound.
        assert_eq!(
            refuse(
                |_| {},
                |b| b[48..52].copy_from_slice(&(-1i32).to_le_bytes())
            ),
            ClassBodyError::CountOutOfRange
        );
        assert_eq!(
            refuse(
                |_| {},
                |b| b[48..52].copy_from_slice(&(MAX_COUNT + 1).to_le_bytes())
            ),
            ClassBodyError::CountOutOfRange
        );
        assert_eq!(
            refuse(
                |_| {},
                |b| b[32..36].copy_from_slice(&(-1i32).to_le_bytes())
            ),
            ClassBodyError::NegativeScriptSize
        );
        assert_eq!(
            refuse(|s| s.script = vec![0x0b, 0x0b], |_| {}),
            ClassBodyError::ScriptNotTerminated
        );
        assert_eq!(
            refuse(|s| s.interfaces = vec![(0, 0)], |_| {}),
            ClassBodyError::InterfaceWithoutClass
        );
        // A bad config name index.
        assert_eq!(
            refuse(
                |_| {},
                |b| b[60..64].copy_from_slice(&9_999i32.to_le_bytes())
            ),
            ClassBodyError::Read(ReadError::InvalidName)
        );
    }

    #[test]
    fn index_and_export_fields_are_checked_first() {
        let (mut pkg, index) = one_class(|_| {}, |_| {});
        assert_eq!(
            decode_class_body(&pkg, 0),
            Err(ClassBodyError::IndexOutOfRange)
        );
        assert_eq!(
            decode_class_body(&pkg, -1),
            Err(ClassBodyError::IndexOutOfRange)
        );
        assert_eq!(
            decode_class_body(&pkg, 99),
            Err(ClassBodyError::IndexOutOfRange)
        );
        pkg.exports[index as usize - 1].size = -1;
        assert_eq!(
            decode_class_body(&pkg, index),
            Err(ClassBodyError::NegativeOffsetOrSize)
        );
    }

    #[test]
    fn error_texts_are_the_cxx_texts() {
        assert_eq!(
            ClassBodyError::TrailingBytes.to_string(),
            "class body does not end at the serial size"
        );
        assert_eq!(
            ClassBodyError::Read(ReadError::Truncated).to_string(),
            "truncated package"
        );
    }
}
