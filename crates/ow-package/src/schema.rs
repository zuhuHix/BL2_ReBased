//! Owner: lane W2-PROPS. The schema typer: array element types from `--array-schema` files (`tools/*-arrays.schema`),
//! the port of `loadSchema` in `src/cli.cpp` and of the `arrayTypes` map of the C++ `Package`.
//!
//! A schema file holds `Property=ElementType` lines; empty lines and lines starting with `#` are skipped, a trailing
//! `\r` is dropped. As in the C++ map, a property is looked up by its name only, whatever class or struct declares
//! it, and several files can be loaded into one typer (a key twice, in one file or across files, is refused).
//!
//! This typer is temporary: W3-DECODE's reflection typer replaces it, and its gate retires the schema files
//! (architecture section 6, `ow-object`).

use std::fmt;
use std::path::Path;

use ow_core::DetMap;

use crate::props::{ArrayScope, ArrayTyper, ElementType};

/// Why a schema could not be loaded. `Display` gives the C++ text where the C++ loader has the case.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    /// "cannot open array schema"
    Open,
    /// "invalid array schema line": no `=`, or nothing before or after it.
    InvalidLine,
    /// "duplicate array schema key"
    DuplicateKey,
    /// "array schema is not UTF-8". Rust only: the C++ loader takes any bytes. The tracked schemas are ASCII; a
    /// non-UTF-8 file is refused rather than read with replaced characters, which could match or collide wrongly.
    NotUtf8,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SchemaError::Open => "cannot open array schema",
            SchemaError::InvalidLine => "invalid array schema line",
            SchemaError::DuplicateKey => "duplicate array schema key",
            SchemaError::NotUtf8 => "array schema is not UTF-8",
        })
    }
}

impl std::error::Error for SchemaError {}

/// A file's bytes as the C++ binary reads them through a text-mode `std::ifstream` on Windows (the MSVC reference
/// build): reading stops at the first 0x1A (Ctrl-Z) byte, and each `\r\n` becomes `\n` before lines are split. A
/// lone `\r` stays. Used for the `--array-schema` files and the `--properties-batch` index file.
///
/// UNVERIFIED against a non-MSVC build of the C++ binary, whose text mode does neither.
pub fn text_mode(bytes: &[u8]) -> Vec<u8> {
    let end = bytes.iter().position(|&b| b == 0x1a).unwrap_or(bytes.len());
    let bytes = &bytes[..end];
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Array element types by property name, from schema files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaTyper {
    types: DetMap<String, ElementType>,
}

impl SchemaTyper {
    /// A typer with no entries: every array with elements stays raw.
    pub fn new_empty() -> Self {
        SchemaTyper::default()
    }

    /// Adds the lines of the schema file at `path` (C++ `loadSchema`).
    pub fn load_file(&mut self, path: &Path) -> Result<(), SchemaError> {
        let bytes = std::fs::read(path).map_err(|_| SchemaError::Open)?;
        let text = String::from_utf8(text_mode(&bytes)).map_err(|_| SchemaError::NotUtf8)?;
        self.add_text(&text)
    }

    /// Adds the lines of a schema file's text. Lines are split at `\n` as `std::getline` does.
    pub fn add_text(&mut self, text: &str) -> Result<(), SchemaError> {
        for line in text.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some(equal) = line.find('=') else {
                return Err(SchemaError::InvalidLine);
            };
            if equal == 0 || equal + 1 == line.len() {
                return Err(SchemaError::InvalidLine);
            }
            let (key, spec) = (&line[..equal], &line[equal + 1..]);
            if self.types.contains_key(key) {
                return Err(SchemaError::DuplicateKey);
            }
            self.types.insert(key.to_string(), ElementType::parse(spec));
        }
        Ok(())
    }

    /// How many properties the typer knows.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

impl ArrayTyper for SchemaTyper {
    /// The type listed for `property`; the scope is ignored, as the C++ map is keyed by property name only.
    fn element(&self, _scope: &ArrayScope<'_>, property: &str) -> Option<ElementType> {
        self.types.get(property).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCOPE: ArrayScope<'static> = ArrayScope { owner: "Any", owner_ref: None, path: &[] };

    #[test]
    fn text_mode_stops_at_ctrl_z() {
        assert_eq!(text_mode(b"A=IntProperty\n\x1aB=IntProperty\n"), b"A=IntProperty\n");
        assert_eq!(text_mode(b"1\x1a\n"), b"1");
        let mut t = SchemaTyper::new_empty();
        t.add_text(std::str::from_utf8(&text_mode(b"A=IntProperty\x1a\nbad line")).unwrap()).unwrap();
        assert_eq!((t.len(), t.element(&SCOPE, "A")), (1, Some(ElementType::Int)));
    }

    #[test]
    fn text_mode_turns_crlf_into_lf_first() {
        assert_eq!(text_mode(b"a\r\nb\r\r\nc\rd\r"), b"a\nb\r\nc\rd\r");
        // `X=Y\r\r\n`: text mode leaves `X=Y\r`, and the loader drops that `\r`, so the value is `Y`.
        let mut t = SchemaTyper::new_empty();
        t.add_text(std::str::from_utf8(&text_mode(b"X=IntProperty\r\r\n")).unwrap()).unwrap();
        assert_eq!(t.element(&SCOPE, "X"), Some(ElementType::Int));
    }

    #[test]
    fn lines_follow_the_cxx_loader() {
        let mut t = SchemaTyper::new_empty();
        t.add_text("# comment\r\n\r\nA=IntProperty\r\nB=StructProperty:Vector\nC=Weird=Thing\nD=StructProperty:")
            .unwrap();
        assert_eq!(t.len(), 4);
        assert_eq!(t.element(&SCOPE, "A"), Some(ElementType::Int));
        assert_eq!(t.element(&SCOPE, "B"), Some(ElementType::Struct("Vector".into())));
        // Only the first `=` splits; the rest is the value, kept as text.
        assert_eq!(t.element(&SCOPE, "C"), Some(ElementType::Other("Weird=Thing".into())));
        assert_eq!(t.element(&SCOPE, "D"), Some(ElementType::Struct(String::new())));
        assert_eq!(t.element(&SCOPE, "E"), None);
        // A key differing only in case is another key (std::map<std::string, ...>).
        assert_eq!(t.element(&SCOPE, "a"), None);
    }

    #[test]
    fn bad_lines_and_duplicates_are_refused() {
        for bad in ["NoEqual", "=IntProperty", "A=", "A=\r", " "] {
            let result = SchemaTyper::new_empty().add_text(bad);
            // " " has no '=' either: only a fully empty line is skipped.
            assert_eq!(result, Err(SchemaError::InvalidLine), "{bad:?}");
        }
        assert_eq!(
            SchemaTyper::new_empty().add_text("A=IntProperty\nA=IntProperty"),
            Err(SchemaError::DuplicateKey)
        );
        let mut t = SchemaTyper::new_empty();
        t.add_text("A=IntProperty").unwrap();
        assert_eq!(t.add_text("A=FloatProperty"), Err(SchemaError::DuplicateKey), "across files");
        assert_eq!(SchemaError::InvalidLine.to_string(), "invalid array schema line");
        assert_eq!(SchemaError::DuplicateKey.to_string(), "duplicate array schema key");
    }

    #[test]
    fn files_open_or_fail_with_the_cxx_text() {
        let error = SchemaTyper::new_empty()
            .load_file(Path::new("this-schema-does-not-exist.schema"))
            .unwrap_err();
        assert_eq!(error.to_string(), "cannot open array schema");
        let dir = std::env::temp_dir().join(format!("ow-package-schema-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.schema");
        std::fs::write(&path, b"A=Int\xffProperty\n").unwrap();
        assert_eq!(SchemaTyper::new_empty().load_file(&path), Err(SchemaError::NotUtf8));
        std::fs::write(&path, b"A=IntProperty\n").unwrap();
        let mut t = SchemaTyper::new_empty();
        t.load_file(&path).unwrap();
        assert_eq!(t.element(&SCOPE, "A"), Some(ElementType::Int));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn element_specs_round_trip() {
        for spec in [
            "IntProperty",
            "FloatProperty",
            "NameProperty",
            "StrProperty",
            "ObjectProperty",
            "ByteProperty",
            "StructProperty:Vector",
            "StructProperty:",
            "ClassProperty",
            "",
        ] {
            assert_eq!(ElementType::parse(spec).spec(), spec);
        }
    }
}
