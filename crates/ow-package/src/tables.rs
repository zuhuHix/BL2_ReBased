//! Owner: lane W1-PKG. The name, import and export tables of a version 832 / licensee 46 package. Entry layouts,
//! minimum entry sizes (12, 28, 68 bytes) and checks are the C++ `Package::load` (`src/package.cpp`), which
//! recovered and verified them against the installed packages (`tools/verify_packages.py`); nothing is re-derived.

use std::fmt;

use crate::reader::{ReadError, Reader};

/// The smallest name-table entry: an FString of length 0 (4 bytes) and 8 flag bytes.
pub const NAME_ENTRY_MIN: usize = 12;
/// An import entry: class package, class name (FNames), outer reference, object name (FName).
pub const IMPORT_ENTRY_SIZE: usize = 28;
/// The smallest export entry (no net objects).
pub const EXPORT_ENTRY_MIN: usize = 68;

/// One import or export (C++ `Object`). References are package-relative: negative for imports (`-1` is the first),
/// positive for exports (`1` is the first), 0 for none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectEntry {
    /// Export: the class reference (0: the export is a class). Imports leave it 0.
    pub class: i32,
    /// Export: the super-struct reference. Imports leave it 0.
    pub super_: i32,
    pub outer: i32,
    /// Export: the archetype reference. Imports leave it 0.
    pub archetype: i32,
    /// The name with its number, as `Name_N-1` when the number is nonzero.
    pub name: String,
    /// Export: the serial size. Imports leave it 0.
    pub size: i32,
    /// Export: the serial offset in the decoded data. Imports leave it 0.
    pub offset: i32,
    /// Import: the class's package name. Exports leave it empty.
    pub class_package: String,
    /// Import: the class name. Exports leave it empty.
    pub class_name: String,
}

/// Why a table could not be read. `Display` gives the C++ exception text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableError {
    Read(ReadError),
    /// "invalid export payload bounds": a negative size or offset, or a payload past the data's end.
    PayloadBounds,
    /// "invalid net object count": negative or more than a quarter of the data length.
    NetObjectCount,
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableError::Read(e) => e.fmt(f),
            TableError::PayloadBounds => f.write_str("invalid export payload bounds"),
            TableError::NetObjectCount => f.write_str("invalid net object count"),
        }
    }
}

impl std::error::Error for TableError {}

impl From<ReadError> for TableError {
    fn from(e: ReadError) -> Self {
        TableError::Read(e)
    }
}

/// The text of name `index` with `number`: the name itself for number 0, else `name_<number - 1>` (C++
/// `Package::name`). The caller has checked `index` and `number` (`Reader::name`).
pub fn name_text(names: &[String], index: i32, number: i32) -> Result<String, ReadError> {
    let base = usize::try_from(index)
        .ok()
        .and_then(|i| names.get(i))
        .ok_or(ReadError::InvalidName)?;
    if number == 0 {
        Ok(base.clone())
    } else {
        Ok(format!("{base}_{}", number - 1))
    }
}

/// Reads an FName at the cursor and returns its text (C++ `Package::name(Reader&)`).
pub fn read_name(r: &mut Reader<'_>, names: &[String]) -> Result<String, ReadError> {
    let count = i32::try_from(names.len()).map_err(|_| ReadError::InvalidName)?;
    let (index, number) = r.name(count)?;
    name_text(names, index, number)
}

/// The name table: `count` entries at `offset`, each an FString and 8 flag bytes.
pub fn read_names(r: &mut Reader<'_>, count: i32, offset: i32) -> Result<Vec<String>, ReadError> {
    r.table(count, offset, NAME_ENTRY_MIN)?;
    let mut names = Vec::with_capacity(count as usize);
    for _ in 0..count {
        names.push(r.string()?);
        r.skip(8)?;
    }
    Ok(names)
}

/// The import table: `imports` entries at `offset`. Outer references are checked against the header's import and
/// export counts, as the C++ does (the export count is not checked yet at this point, there either).
pub fn read_imports(
    r: &mut Reader<'_>,
    names: &[String],
    imports: i32,
    offset: i32,
    exports: i32,
) -> Result<Vec<ObjectEntry>, ReadError> {
    r.table(imports, offset, IMPORT_ENTRY_SIZE)?;
    let mut table = Vec::with_capacity(imports as usize);
    for _ in 0..imports {
        let class_package = read_name(r, names)?;
        let class_name = read_name(r, names)?;
        let outer = r.reference(imports, exports)?;
        let name = read_name(r, names)?;
        table.push(ObjectEntry {
            outer,
            name,
            class_package,
            class_name,
            ..ObjectEntry::default()
        });
    }
    Ok(table)
}

/// The export table: `exports` entries at `offset`: class, super and outer references, name, archetype reference,
/// 8 flag bytes, serial size and offset (checked against the data), 4 bytes, the net object count (checked) and its
/// 4-byte entries, then 20 bytes.
pub fn read_exports(
    r: &mut Reader<'_>,
    names: &[String],
    imports: i32,
    exports: i32,
    offset: i32,
) -> Result<Vec<ObjectEntry>, TableError> {
    let data_len = r.bytes().len();
    r.table(exports, offset, EXPORT_ENTRY_MIN)?;
    let mut table = Vec::with_capacity(exports as usize);
    for _ in 0..exports {
        let class = r.reference(imports, exports)?;
        let super_ = r.reference(imports, exports)?;
        let outer = r.reference(imports, exports)?;
        let name = read_name(r, names)?;
        let archetype = r.reference(imports, exports)?;
        r.skip(8)?;
        let size = r.i32()?;
        let payload_offset = r.i32()?;
        if size < 0
            || payload_offset < 0
            || payload_offset as usize > data_len
            || size as usize > data_len - payload_offset as usize
        {
            return Err(TableError::PayloadBounds);
        }
        r.skip(4)?;
        let net = r.i32()?;
        if net < 0 || net as usize > data_len / 4 {
            return Err(TableError::NetObjectCount);
        }
        r.skip(net as usize * 4)?;
        r.skip(20)?;
        table.push(ObjectEntry {
            class,
            super_,
            outer,
            archetype,
            name,
            size,
            offset: payload_offset,
            ..ObjectEntry::default()
        });
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(values: &[i32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn names() -> Vec<String> {
        vec!["None".into(), "Thing".into()]
    }

    #[test]
    fn name_numbers_print_one_less() {
        let n = names();
        assert_eq!(name_text(&n, 1, 0).unwrap(), "Thing");
        assert_eq!(name_text(&n, 1, 1).unwrap(), "Thing_0");
        assert_eq!(name_text(&n, 1, 12).unwrap(), "Thing_11");
        assert_eq!(name_text(&n, 2, 0), Err(ReadError::InvalidName));
        let data = le(&[1, 3, 2, 0]);
        let mut r = Reader::new(&data);
        assert_eq!(read_name(&mut r, &n).unwrap(), "Thing_2");
        assert_eq!(read_name(&mut r, &n), Err(ReadError::InvalidName));
    }

    #[test]
    fn name_table_reads_strings_and_skips_flags() {
        let mut data = vec![0u8; 4];
        for s in ["None", "Thing"] {
            data.extend_from_slice(&le(&[s.len() as i32 + 1]));
            data.extend_from_slice(s.as_bytes());
            data.push(0);
            data.extend_from_slice(&[0xaa; 8]);
        }
        let mut r = Reader::new(&data);
        assert_eq!(read_names(&mut r, 2, 4).unwrap(), names());
        assert_eq!(r.pos(), data.len());
        // A count the data cannot hold at 12 bytes an entry is refused before any read.
        assert_eq!(
            read_names(&mut Reader::new(&data), 4, 4),
            Err(ReadError::InvalidTableBounds)
        );
    }

    #[test]
    fn imports_carry_class_package_and_name() {
        let data = le(&[1, 0, 0, 1, -1, 1, 2]);
        let mut r = Reader::new(&data);
        let imports = read_imports(&mut r, &names(), 1, 0, 0).unwrap();
        assert_eq!(
            imports,
            vec![ObjectEntry {
                outer: -1,
                name: "Thing_1".into(),
                class_package: "Thing".into(),
                class_name: "None_0".into(),
                ..ObjectEntry::default()
            }]
        );
        // An outer past the import count is refused.
        let data = le(&[1, 0, 0, 1, -2, 1, 2]);
        assert_eq!(
            read_imports(&mut Reader::new(&data), &names(), 1, 0, 0),
            Err(ReadError::InvalidReference)
        );
    }

    fn export(size: i32, offset: i32, net: i32) -> Vec<i32> {
        let mut e = vec![0, 0, 0, 1, 0, 0, 0, 0, size, offset, 0, net];
        e.extend(std::iter::repeat_n(7, net as usize));
        e.extend([0; 5]);
        e
    }

    #[test]
    fn exports_check_payload_bounds_and_net_counts() {
        let data = le(&export(4, 0, 2));
        assert_eq!(data.len(), EXPORT_ENTRY_MIN + 8);
        let exports = read_exports(&mut Reader::new(&data), &names(), 0, 1, 0).unwrap();
        assert_eq!(exports[0].name, "Thing");
        assert_eq!((exports[0].size, exports[0].offset), (4, 0));

        let len = le(&export(0, 0, 0)).len() as i32;
        for (size, offset) in [(-1, 0), (0, -1), (1, len), (0, len + 1), (len + 1, 0)] {
            let data = le(&export(size, offset, 0));
            assert_eq!(
                read_exports(&mut Reader::new(&data), &names(), 0, 1, 0),
                Err(TableError::PayloadBounds),
                "{size} {offset}"
            );
        }
        let data = le(&export(0, len, 0));
        assert!(read_exports(&mut Reader::new(&data), &names(), 0, 1, 0).is_ok());
        let mut bad_net = export(0, 0, 0);
        bad_net[11] = -1;
        assert_eq!(
            read_exports(&mut Reader::new(&le(&bad_net)), &names(), 0, 1, 0),
            Err(TableError::NetObjectCount)
        );
        bad_net[11] = 1000;
        assert_eq!(
            read_exports(&mut Reader::new(&le(&bad_net)), &names(), 0, 1, 0),
            Err(TableError::NetObjectCount)
        );
        bad_net[11] = 3;
        assert_eq!(
            read_exports(&mut Reader::new(&le(&bad_net)), &names(), 0, 1, 0),
            Err(TableError::Read(ReadError::Truncated))
        );
    }
}
