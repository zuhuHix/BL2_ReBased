//! Owner: lane W1-TESTKIT. Synthetic packages for Rust tests (AGENTS.md rule 9): the Rust counterparts of the
//! `package()`, `tag()` and `container()` helpers in `tests/` (header 832/46, names, imports, exports, payloads, LZO
//! containers made of literal runs, the partial-compression shape). Compiled for this crate's tests and, behind the
//! `test-support` feature, for every other crate's tests; later lanes extend it here instead of writing their own.
//!
//! It writes bytes only and never reads them back (W1-PKG's reader is checked against it, not built into it). Every
//! value a caller passes is invented; nothing here comes from a game file.
//!
//! The byte form is the one the Python helpers write, and `testkit/tests.rs` pins it: for invented inputs, the bytes
//! here equal the bytes that `tests/package_test.py`, `properties_test.py`, `runtime_test.py`,
//! `global_resolve_test.py`, `assets_test.py` and `behavior_test.py` produce. The layout facts come from those helpers
//! and from what `src/package.cpp` (`Package::load`) and `src/container.cpp` (`decode_package`) read; none is new.
//!
//! | Python helper | Here |
//! |---|---|
//! | `package()` of any of those files, `Package().build()` of `behavior_test.py` / `kismet_test.py` | [`PackageBuilder::build`] |
//! | `fname(name, number)` | [`PackageBuilder::fname`] |
//! | `tag(...)`, `Tags.int/float/bool/...` | [`PackageBuilder::tag`] and the typed tag methods |
//! | `container(data, block)` (`package_test.py`), `literal(data)` (`assets_test.py`) | [`container`] |
//! | the partial-compression package of `package_test.py` | [`PackageBuilder::build_partial`], [`partial`] |
//! | `fstring(value)` (`properties_test.py`) | [`fstring`] |
//! | `struct.pack_into('<I', ...)` | [`put_u32`] |

/// The UE3 package tag, the first word of a package and of every LZO container.
pub const PACKAGE_TAG: u32 = 0x9e2a_83c1;
/// File version 832 in the low half, licensee 46 in the high half: the second word of a BL2 package.
pub const VERSION_WORD: u32 = 832 | (46 << 16);
/// The summary size every plain fixture writes (`tests/*.py` `package()`): twelve words.
pub const PLAIN_HEADER_SIZE: usize = 48;
/// The summary size of the partially compressed fixture (`package_test.py`): the 112-byte summary with its one-row
/// chunk table, as the decoded package's header.
pub const PARTIAL_HEADER_SIZE: usize = 128;
/// The package flag `src/container.cpp` tests for a partial compression table.
pub const PARTIAL_COMPRESSION_FLAG: u32 = 0x0200_0000;
/// The container block size `package_test.py` and `assets_test.py` use by default.
pub const DEFAULT_BLOCK: usize = 131_072;
/// Bytes of one import row (`src/package.cpp`: class package, class name, outer, name).
pub const IMPORT_ROW: usize = 28;
/// Bytes of one export row as the fixtures write it (no net objects).
pub const EXPORT_ROW: usize = 68;

/// How a name-table entry stores its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameForm {
    /// A positive length (bytes with the terminator), the text's UTF-8 bytes, a zero byte. What
    /// `name.encode() + b'\0'` writes in `runtime_test.py`, `assets_test.py` and `behavior_test.py`.
    Ansi,
    /// A negative length (UTF-16 code units with the terminator), UTF-16LE text, a zero unit: `fstring()` in
    /// `properties_test.py`.
    Utf16,
}

#[derive(Debug, Clone)]
struct ImportRow {
    class_package: i32,
    class_name: i32,
    outer: i32,
    name: i32,
}

#[derive(Debug, Clone)]
struct ExportRow {
    class: i32,
    super_: i32,
    outer: i32,
    name: i32,
    number: i32,
    archetype: i32,
    flags: u64,
    payload: Vec<u8>,
}

/// An export to add with [`PackageBuilder::add_export_with`]; [`Export::new`] fills the rest with the zeros the
/// Python helpers write.
#[derive(Debug, Clone)]
pub struct Export {
    /// Class reference (negative: import, positive: export, 0: none).
    pub class: i32,
    /// Super reference.
    pub super_: i32,
    /// Outer reference.
    pub outer: i32,
    /// Object name, added to the name table when the export is added.
    pub name: String,
    /// Name number (0: none; `n` prints as `_{n-1}`).
    pub number: i32,
    /// Archetype reference.
    pub archetype: i32,
    /// The eight bytes of object flags after the archetype.
    pub flags: u64,
    /// The serialized object, placed after the export table in export order.
    pub payload: Vec<u8>,
}

impl Export {
    /// An export of `class` called `name` holding `payload`; every other field is 0.
    pub fn new(class: i32, name: &str, payload: impl Into<Vec<u8>>) -> Self {
        Export {
            class,
            super_: 0,
            outer: 0,
            name: name.to_owned(),
            number: 0,
            archetype: 0,
            flags: 0,
            payload: payload.into(),
        }
    }
}

/// Where [`PackageBuilder::build_with_layout`] put each part, as byte offsets into the (decoded) package, so tests
/// can corrupt one field (`put_u32`) the way `package_test.py` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The summary size written in word 2.
    pub header_size: usize,
    /// Start of the name table.
    pub names_at: usize,
    /// Start of the import table.
    pub imports_at: usize,
    /// Start of the export table.
    pub exports_at: usize,
    /// Start of each export's payload, in export order.
    pub payloads_at: Vec<usize>,
    /// Total length of the package.
    pub len: usize,
}

impl Layout {
    /// Offset of the row of export `reference` (1-based, as package references are).
    pub fn export_row(&self, reference: i32) -> usize {
        assert!(reference >= 1, "export references start at 1");
        self.exports_at + (reference as usize - 1) * EXPORT_ROW
    }

    /// Offset of the size word of export `reference`; the offset word follows it.
    pub fn export_size_field(&self, reference: i32) -> usize {
        self.export_row(reference) + 32
    }

    /// Offset of the row of import `reference` (-1, -2, ...).
    pub fn import_row(&self, reference: i32) -> usize {
        assert!(reference <= -1, "import references start at -1");
        self.imports_at + (-(reference as i64) as usize - 1) * IMPORT_ROW
    }
}

/// A synthetic BL2 package (version 832, licensee 46): a name table, imports, exports and their payloads, written in
/// the order every `tests/` helper uses (summary, names, imports, exports, payloads).
///
/// Names are added on first use, in call order, as `behavior_test.py`'s `Package` does; to match a helper with a
/// fixed name list, declare the list first with [`PackageBuilder::names`] or [`PackageBuilder::names_utf16`].
/// [`PackageBuilder::build`] adds `None` last if nothing used it, as `Package().build()` does.
#[derive(Debug, Default, Clone)]
pub struct PackageBuilder {
    names: Vec<(String, NameForm)>,
    imports: Vec<ImportRow>,
    exports: Vec<ExportRow>,
}

impl PackageBuilder {
    /// A builder with no names, imports or exports.
    pub fn new_empty() -> Self {
        PackageBuilder::default()
    }

    /// The index of `text` in the name table, added as [`NameForm::Ansi`] if absent.
    pub fn name(&mut self, text: &str) -> i32 {
        self.name_in(text, NameForm::Ansi)
    }

    /// The index of `text`, added as [`NameForm::Utf16`] if absent. A name already present keeps its form.
    pub fn name_utf16(&mut self, text: &str) -> i32 {
        self.name_in(text, NameForm::Utf16)
    }

    /// Declares `texts` in order as ANSI names (those already present are skipped).
    pub fn names<'a>(&mut self, texts: impl IntoIterator<Item = &'a str>) -> &mut Self {
        for text in texts {
            self.name(text);
        }
        self
    }

    /// Declares `texts` in order as UTF-16 names (those already present are skipped).
    pub fn names_utf16<'a>(&mut self, texts: impl IntoIterator<Item = &'a str>) -> &mut Self {
        for text in texts {
            self.name_utf16(text);
        }
        self
    }

    fn name_in(&mut self, text: &str, form: NameForm) -> i32 {
        let index = match self.names.iter().position(|(n, _)| n == text) {
            Some(i) => i,
            None => {
                self.names.push((text.to_owned(), form));
                self.names.len() - 1
            }
        };
        i32::try_from(index).expect("name table index fits i32")
    }

    /// An FName as stored in a package: name index and number, two little-endian `i32`. Adds `text` if absent.
    pub fn fname(&mut self, text: &str, number: i32) -> [u8; 8] {
        let index = self.name(text);
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&index.to_le_bytes());
        out[4..].copy_from_slice(&number.to_le_bytes());
        out
    }

    /// Adds an import row and returns its reference (-1 for the first). Names are added in the order class package,
    /// class name, name.
    pub fn add_import(
        &mut self,
        class_package: &str,
        class_name: &str,
        outer: i32,
        name: &str,
    ) -> i32 {
        let class_package = self.name(class_package);
        let class_name = self.name(class_name);
        let name = self.name(name);
        self.imports.push(ImportRow {
            class_package,
            class_name,
            outer,
            name,
        });
        -i32::try_from(self.imports.len()).expect("import count fits i32")
    }

    /// Adds an export of `class` called `name` holding `payload`, other fields 0, and returns its reference (1 for
    /// the first).
    pub fn add_export(&mut self, class: i32, name: &str, payload: impl Into<Vec<u8>>) -> i32 {
        self.add_export_with(Export::new(class, name, payload))
    }

    /// Adds an export with every field given and returns its reference.
    pub fn add_export_with(&mut self, export: Export) -> i32 {
        let name = self.name(&export.name);
        self.exports.push(ExportRow {
            class: export.class,
            super_: export.super_,
            outer: export.outer,
            name,
            number: export.number,
            archetype: export.archetype,
            flags: export.flags,
            payload: export.payload,
        });
        i32::try_from(self.exports.len()).expect("export count fits i32")
    }

    /// Replaces the payload of export `reference` (for payloads that refer to exports added after it, as
    /// `behavior_test.py` does).
    pub fn set_payload(&mut self, reference: i32, payload: impl Into<Vec<u8>>) {
        self.export_mut(reference).payload = payload.into();
    }

    /// Sets the outer of export `reference`.
    pub fn set_outer(&mut self, reference: i32, outer: i32) {
        self.export_mut(reference).outer = outer;
    }

    fn export_mut(&mut self, reference: i32) -> &mut ExportRow {
        let index = usize::try_from(reference - 1).expect("export references start at 1");
        &mut self.exports[index]
    }

    /// A property tag: name, type, size (the value's length), array index 0, then `value`. `tag()` of
    /// `assets_test.py`.
    pub fn tag(&mut self, name: &str, kind: &str, value: &[u8]) -> Vec<u8> {
        self.tag_full(name, 0, kind, value.len(), &[], value)
    }

    /// A tag whose type-specific detail (a struct name, an enum name, a bool byte) sits between the header and the
    /// value; the size is the value's length. `tag(name, kind, value, extra)` of `assets_test.py`.
    pub fn tag_detail(&mut self, name: &str, kind: &str, detail: &[u8], value: &[u8]) -> Vec<u8> {
        self.tag_full(name, 0, kind, value.len(), detail, value)
    }

    /// A tag with every field given: name, array index, type, size word, detail, value. `size` need not be the
    /// value's length (malformed-tag tests).
    pub fn tag_full(
        &mut self,
        name: &str,
        index: i32,
        kind: &str,
        size: usize,
        detail: &[u8],
        value: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(24 + detail.len() + value.len());
        out.extend_from_slice(&self.fname(name, 0));
        out.extend_from_slice(&self.fname(kind, 0));
        out.extend_from_slice(
            &i32::try_from(size)
                .expect("tag size fits i32")
                .to_le_bytes(),
        );
        out.extend_from_slice(&index.to_le_bytes());
        out.extend_from_slice(detail);
        out.extend_from_slice(value);
        out
    }

    /// `IntProperty`.
    pub fn int_tag(&mut self, name: &str, value: i32) -> Vec<u8> {
        self.tag(name, "IntProperty", &value.to_le_bytes())
    }

    /// `FloatProperty`.
    pub fn float_tag(&mut self, name: &str, value: f32) -> Vec<u8> {
        self.tag(name, "FloatProperty", &value.to_le_bytes())
    }

    /// `BoolProperty`: size 0, the value as one byte after the header.
    pub fn bool_tag(&mut self, name: &str, value: bool) -> Vec<u8> {
        self.tag_detail(name, "BoolProperty", &[u8::from(value)], &[])
    }

    /// `NameProperty` holding `value` (number 0).
    pub fn name_tag(&mut self, name: &str, value: &str) -> Vec<u8> {
        let value = self.fname(value, 0);
        self.tag(name, "NameProperty", &value)
    }

    /// `ObjectProperty` holding the package reference `reference`.
    pub fn object_tag(&mut self, name: &str, reference: i32) -> Vec<u8> {
        self.tag(name, "ObjectProperty", &reference.to_le_bytes())
    }

    /// `StrProperty` holding `value` as a UTF-16 [`fstring`].
    pub fn str_tag(&mut self, name: &str, value: &str) -> Vec<u8> {
        self.tag(name, "StrProperty", &fstring(value))
    }

    /// `ByteProperty` of enum `enum_name` holding the name `value`.
    ///
    /// The typed tag methods add new names in the order `behavior_test.py`'s `Tags` does (Python evaluates the
    /// arguments first): the value and detail names, then the tag's name and type.
    pub fn enum_tag(&mut self, name: &str, enum_name: &str, value: &str) -> Vec<u8> {
        let value = self.fname(value, 0);
        let detail = self.fname(enum_name, 0);
        self.tag_detail(name, "ByteProperty", &detail, &value)
    }

    /// `ByteProperty` with no enum (`None`) holding one byte.
    pub fn byte_tag(&mut self, name: &str, value: u8) -> Vec<u8> {
        let detail = self.fname("None", 0);
        self.tag_detail(name, "ByteProperty", &detail, &[value])
    }

    /// `StructProperty` of struct `struct_name` with the serialized `body`.
    pub fn struct_tag(&mut self, name: &str, struct_name: &str, body: &[u8]) -> Vec<u8> {
        let detail = self.fname(struct_name, 0);
        self.tag_detail(name, "StructProperty", &detail, body)
    }

    /// `ArrayProperty`: the element count, then the elements' `body`; the size covers both.
    pub fn array_tag(&mut self, name: &str, count: i32, body: &[u8]) -> Vec<u8> {
        let mut value = count.to_le_bytes().to_vec();
        value.extend_from_slice(body);
        self.tag(name, "ArrayProperty", &value)
    }

    /// The `None` FName that ends a tag stream.
    pub fn none(&mut self) -> [u8; 8] {
        self.fname("None", 0)
    }

    /// The plain package (48-byte summary).
    pub fn build(&mut self) -> Vec<u8> {
        self.build_with_layout().0
    }

    /// The plain package and where its parts are.
    pub fn build_with_layout(&mut self) -> (Vec<u8>, Layout) {
        self.name("None");
        let (tables, layout) = self.tables(PLAIN_HEADER_SIZE);
        let mut out = self.summary(&layout, 0);
        out.extend_from_slice(&tables);
        debug_assert_eq!(out.len(), layout.len);
        (out, layout)
    }

    /// The same tables as a partially compressed package, the shape `package_test.py` builds: a 112-byte summary with
    /// the compression flag and a one-row chunk table, 16 unused bytes, then the tables (logically at byte 128) as one
    /// LZO container of literal runs. Decoded, it is a package with a 128-byte header.
    pub fn build_partial(&mut self) -> Vec<u8> {
        self.name("None");
        let (tables, layout) = self.tables(PARTIAL_HEADER_SIZE);
        let mut out = self.summary(&layout, PARTIAL_COMPRESSION_FLAG);
        // After the six table words: 20 bytes of dependency / GUID-table fields and a 16-byte GUID, left zero.
        out.resize(80, 0);
        // One generation row (the three words package_test.py writes; the readers skip them), the engine and cooker
        // versions (0), codec 2 (LZO), one chunk.
        for word in [1u32, 1, 1, 0, 0, 0, 2, 1] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        debug_assert_eq!(out.len(), 112);
        let stream = container(&tables, DEFAULT_BLOCK);
        let stream_at = PARTIAL_HEADER_SIZE + 16;
        for word in [PARTIAL_HEADER_SIZE, tables.len(), stream_at, stream.len()] {
            out.extend_from_slice(
                &u32::try_from(word)
                    .expect("chunk word fits u32")
                    .to_le_bytes(),
            );
        }
        out.resize(stream_at, 0);
        out.extend_from_slice(&stream);
        out
    }

    /// Names, imports, exports and payloads as they sit after a summary of `header_size` bytes.
    fn tables(&self, header_size: usize) -> (Vec<u8>, Layout) {
        let mut out = Vec::new();
        for (text, form) in &self.names {
            match form {
                NameForm::Ansi => {
                    let length = i32::try_from(text.len() + 1).expect("name length fits i32");
                    out.extend_from_slice(&length.to_le_bytes());
                    out.extend_from_slice(text.as_bytes());
                    out.push(0);
                }
                NameForm::Utf16 => out.extend_from_slice(&fstring(text)),
            }
            out.extend_from_slice(&[0u8; 8]);
        }
        let imports_at = header_size + out.len();
        for row in &self.imports {
            for word in [
                row.class_package,
                0,
                row.class_name,
                0,
                row.outer,
                row.name,
                0,
            ] {
                out.extend_from_slice(&word.to_le_bytes());
            }
        }
        let exports_at = header_size + out.len();
        let mut payload_at = exports_at + self.exports.len() * EXPORT_ROW;
        let mut payloads_at = Vec::with_capacity(self.exports.len());
        for row in &self.exports {
            payloads_at.push(payload_at);
            let size = i32::try_from(row.payload.len()).expect("payload size fits i32");
            let offset = i32::try_from(payload_at).expect("payload offset fits i32");
            for word in [
                row.class,
                row.super_,
                row.outer,
                row.name,
                row.number,
                row.archetype,
            ] {
                out.extend_from_slice(&word.to_le_bytes());
            }
            out.extend_from_slice(&row.flags.to_le_bytes());
            // size, offset, one unread word, the net object count (0), 20 trailing bytes.
            for word in [size, offset, 0, 0] {
                out.extend_from_slice(&word.to_le_bytes());
            }
            out.extend_from_slice(&[0u8; 20]);
            payload_at += row.payload.len();
        }
        for row in &self.exports {
            out.extend_from_slice(&row.payload);
        }
        let layout = Layout {
            header_size,
            names_at: header_size,
            imports_at,
            exports_at,
            payloads_at,
            len: header_size + out.len(),
        };
        (out, layout)
    }

    /// The first eleven or twelve summary words: tag, version, header size, an empty folder name, package flags, the
    /// name, export and import counts and offsets (in that order), and for the plain form one final zero word.
    fn summary(&self, layout: &Layout, flags: u32) -> Vec<u8> {
        let count = |n: usize| u32::try_from(n).expect("table count fits u32");
        let mut words = vec![
            PACKAGE_TAG,
            VERSION_WORD,
            count(layout.header_size),
            0,
            flags,
            count(self.names.len()),
            count(layout.names_at),
            count(self.exports.len()),
            count(layout.exports_at),
            count(self.imports.len()),
            count(layout.imports_at),
        ];
        if flags & PARTIAL_COMPRESSION_FLAG == 0 {
            words.push(0);
        }
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
}

/// A valid LZO container of `data` in blocks of `block` bytes, each block one literal run with an end marker:
/// `container()` of `package_test.py` (and, for one block of 4 to 238 bytes, `literal()` of `assets_test.py`).
/// Words: tag, block size, total stream size, decoded size, then (stream size, decoded size) per block.
///
/// Panics if a block (including the last) is shorter than 4 bytes, as the Python helper asserts.
pub fn container(data: &[u8], block: usize) -> Vec<u8> {
    assert!(block > 0, "block size must be positive");
    let mut streams = Vec::new();
    let mut sizes = Vec::new();
    for chunk in data.chunks(block) {
        assert!(
            chunk.len() >= 4,
            "a literal-run block needs at least 4 bytes"
        );
        let start = streams.len();
        if chunk.len() <= 238 {
            streams.push(17 + chunk.len() as u8);
        } else {
            let (zeros, last) = ((chunk.len() - 19) / 255, (chunk.len() - 19) % 255);
            streams.resize(streams.len() + 1 + zeros, 0);
            streams.push(last as u8 + 1);
        }
        streams.extend_from_slice(chunk);
        streams.extend_from_slice(&[0x11, 0, 0]);
        sizes.push(streams.len() - start);
        sizes.push(chunk.len());
    }
    let word = |n: usize| {
        u32::try_from(n)
            .expect("container word fits u32")
            .to_le_bytes()
    };
    let mut out = Vec::with_capacity(16 + 4 * sizes.len() + streams.len());
    out.extend_from_slice(&PACKAGE_TAG.to_le_bytes());
    out.extend_from_slice(&word(block));
    out.extend_from_slice(&word(streams.len()));
    out.extend_from_slice(&word(data.len()));
    for size in sizes {
        out.extend_from_slice(&word(size));
    }
    out.extend_from_slice(&streams);
    out
}

/// The partially compressed form of `builder`'s package: [`PackageBuilder::build_partial`].
pub fn partial(builder: &mut PackageBuilder) -> Vec<u8> {
    builder.build_partial()
}

/// An FString in UTF-16: the negative count of UTF-16 code units with the terminator, then UTF-16LE text and a zero
/// unit. `fstring()` of `properties_test.py`.
pub fn fstring(text: &str) -> Vec<u8> {
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let length = -i32::try_from(units.len()).expect("string length fits i32");
    let mut out = length.to_le_bytes().to_vec();
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

/// An FString in one byte per character: the count of bytes with the terminator, the UTF-8 bytes, a zero byte.
pub fn fstring_ansi(text: &str) -> Vec<u8> {
    let length = i32::try_from(text.len() + 1).expect("string length fits i32");
    let mut out = length.to_le_bytes().to_vec();
    out.extend_from_slice(text.as_bytes());
    out.push(0);
    out
}

/// Little-endian `u32` words (`pack()` / `words()` of the Python tests).
pub fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Little-endian `i32` words (`ints()` / `w32()` of the Python tests).
pub fn ints(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Overwrites the little-endian word at `at` (`struct.pack_into('<I', data, at, value)`), for corrupt-field tests.
pub fn put_u32(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests;
