//! Owner: lane W0-SCAFFOLD (complete). Object paths and package names.
//!
//! Both compare like an FName: case-insensitively, with ASCII case folding only, the way the C++ reader folds names
//! (`folded()` in `src/package.cpp`, `std::tolower` in the C locale). Each value keeps the spelling it was made from
//! for printing. Equality, ordering and hashing all use the folded form, so `DetMap` keys and lookups agree.
//!
//! An object path is the outer chain from the outermost object to the object itself. The C++ reader joins it with
//! `.` (`Package::path`, `src/package.cpp`); the game, bl-sdk and OpenBLCMM write a sub-object as `Owner:Name`
//! (`tools/weapon_catalog.py`, `tools/weapon_balance.py`). Both separators are accepted, and for comparing, hashing
//! and splitting `:` counts as `.`, so `A.B:C` and `A.B.C` are the same path; printing keeps the spelling given. The
//! first component is the outermost object, which for a cooked package's top-level export is the package-like group
//! the tools call its package. Nothing else about the path is interpreted here.
//!
//! Ordering is the folded byte order. A `DetMap<ObjectPath, _>` therefore sorts case-folded and merges keys that
//! differ only in case or separator: it matches an oracle only if that oracle folds its keys the same way. An oracle
//! keyed by the raw string (a C++ `std::map<std::string, _>`, Python `json.dumps(sort_keys=True)`) sorts by the exact
//! bytes and keeps case variants apart; mirror it with `DetMap<String, _>`.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Why a text is not a valid object path or package name.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentError {
    /// The text is empty.
    Empty,
    /// A component between separators (`.` or `:`) is empty (a leading, trailing or doubled separator); `at` is its
    /// byte offset.
    EmptyComponent { at: usize },
    /// A control character (below 0x20, or 0x7F) at byte offset `at`.
    ControlCharacter { at: usize },
    /// A package name holds a `.`, `:`, `/` or `\` at byte offset `at`.
    Separator { at: usize },
}

impl fmt::Display for IdentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentError::Empty => f.write_str("empty name"),
            IdentError::EmptyComponent { at } => write!(f, "empty path component at byte {at}"),
            IdentError::ControlCharacter { at } => write!(f, "control character at byte {at}"),
            IdentError::Separator { at } => write!(f, "separator in a package name at byte {at}"),
        }
    }
}

impl std::error::Error for IdentError {}

fn check_text(s: &str) -> Result<(), IdentError> {
    if s.is_empty() {
        return Err(IdentError::Empty);
    }
    if let Some(at) = s.bytes().position(|b| b < 0x20 || b == 0x7F) {
        return Err(IdentError::ControlCharacter { at });
    }
    Ok(())
}

/// The byte as comparing sees it: ASCII lower case, and `:` as `.`.
fn fold(b: u8) -> u8 {
    if b == b':' {
        b'.'
    } else {
        b.to_ascii_lowercase()
    }
}

fn is_separator(b: u8) -> bool {
    b == b'.' || b == b':'
}

fn folded_cmp(a: &str, b: &str) -> Ordering {
    a.bytes().map(fold).cmp(b.bytes().map(fold))
}

fn folded_hash<H: Hasher>(s: &str, state: &mut H) {
    for b in s.bytes() {
        state.write_u8(fold(b));
    }
    state.write_u8(0xFF); // terminator, so "ab"+"c" and "a"+"bc" differ when hashed in sequence
}

macro_rules! folded_traits {
    ($t:ident) => {
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                folded_cmp(&self.0, &other.0) == Ordering::Equal
            }
        }
        impl Eq for $t {}
        impl PartialOrd for $t {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for $t {
            fn cmp(&self, other: &Self) -> Ordering {
                folded_cmp(&self.0, &other.0)
            }
        }
        impl Hash for $t {
            fn hash<H: Hasher>(&self, state: &mut H) {
                folded_hash(&self.0, state)
            }
        }
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

/// An object path with `.` or `:` separators, compared case-insensitively with `:` equal to `.`.
#[derive(Debug, Clone)]
pub struct ObjectPath(Box<str>);

folded_traits!(ObjectPath);

impl ObjectPath {
    /// Accepts any non-empty text without control characters whose components (split at `.` and `:`) are all
    /// non-empty.
    pub fn parse(s: &str) -> Result<Self, IdentError> {
        check_text(s)?;
        let mut start = 0;
        for (i, b) in s.bytes().enumerate() {
            if is_separator(b) {
                if i == start {
                    return Err(IdentError::EmptyComponent { at: i });
                }
                start = i + 1;
            }
        }
        if start == s.len() {
            return Err(IdentError::EmptyComponent { at: start });
        }
        Ok(ObjectPath(s.into()))
    }

    /// The first component: the outermost object.
    pub fn package(&self) -> &str {
        self.components().next().unwrap_or(&self.0)
    }

    /// The last component: the object's own name.
    pub fn name(&self) -> &str {
        self.0.rsplit(['.', ':']).next().unwrap_or(&self.0)
    }

    /// The components from the outermost to the object itself, split at `.` and `:`.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split(['.', ':'])
    }

    /// The path as it was spelled when made.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A package name without folder or extension (`WillowGame`, `Sanctuary_P`), compared case-insensitively.
#[derive(Debug, Clone)]
pub struct PackageName(Box<str>);

folded_traits!(PackageName);

impl PackageName {
    /// Accepts any non-empty text without control characters, object path separators (`.`, `:`) or folder
    /// separators.
    pub fn parse(s: &str) -> Result<Self, IdentError> {
        check_text(s)?;
        if let Some(at) = s
            .bytes()
            .position(|b| is_separator(b) || b == b'/' || b == b'\\')
        {
            return Err(IdentError::Separator { at });
        }
        Ok(PackageName(s.into()))
    }

    /// The name as it was spelled when made.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

    fn hash_of<T: Hash>(t: &T) -> u64 {
        BuildHasherDefault::<DefaultHasher>::default().hash_one(t)
    }

    #[test]
    fn object_paths_compare_like_fnames_and_keep_their_spelling() {
        let a = ObjectPath::parse("GD_Test.Parts.Barrel_A").unwrap();
        let b = ObjectPath::parse("gd_test.PARTS.barrel_a").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.cmp(&b), Ordering::Equal);
        assert_eq!(hash_of(&a), hash_of(&b));
        assert_eq!(a.to_string(), "GD_Test.Parts.Barrel_A");
        assert_eq!(b.as_str(), "gd_test.PARTS.barrel_a");
        assert_eq!(a.package(), "GD_Test");
        assert_eq!(a.name(), "Barrel_A");
        assert_eq!(
            a.components().collect::<Vec<_>>(),
            ["GD_Test", "Parts", "Barrel_A"]
        );
        let single = ObjectPath::parse("Core").unwrap();
        assert_eq!((single.package(), single.name()), ("Core", "Core"));
    }

    #[test]
    fn ordering_is_the_folded_byte_order() {
        // Folded bytes decide: "a.z" < "a_b.y" because '.' (0x2E) < '_' (0x5F).
        let mut set = BTreeSet::new();
        for s in ["b.X", "A_b.y", "a.Z", "A.z"] {
            set.insert(ObjectPath::parse(s).unwrap());
        }
        let order: Vec<&str> = set.iter().map(|p| p.as_str()).collect();
        assert_eq!(order, ["a.Z", "A_b.y", "b.X"]); // "A.z" equals "a.Z"; insert keeps the one already there
    }

    #[test]
    fn object_path_refusals() {
        assert_eq!(ObjectPath::parse(""), Err(IdentError::Empty));
        assert_eq!(
            ObjectPath::parse(".A"),
            Err(IdentError::EmptyComponent { at: 0 })
        );
        assert_eq!(
            ObjectPath::parse("A..B"),
            Err(IdentError::EmptyComponent { at: 2 })
        );
        assert_eq!(
            ObjectPath::parse("A.B."),
            Err(IdentError::EmptyComponent { at: 4 })
        );
        assert_eq!(
            ObjectPath::parse("A.\tB"),
            Err(IdentError::ControlCharacter { at: 2 })
        );
        assert_eq!(
            ObjectPath::parse("A.:B"),
            Err(IdentError::EmptyComponent { at: 2 })
        );
        assert_eq!(
            ObjectPath::parse("A:"),
            Err(IdentError::EmptyComponent { at: 2 })
        );
    }

    #[test]
    fn a_colon_sub_object_equals_the_dotted_path() {
        // The game and bl-sdk write Owner:Name; the C++ reader writes Owner.Name.
        let colon = ObjectPath::parse("A.B:C").unwrap();
        let dotted = ObjectPath::parse("a.b.c").unwrap();
        assert_eq!(colon, dotted);
        assert_eq!(colon.cmp(&dotted), Ordering::Equal);
        assert_eq!(hash_of(&colon), hash_of(&dotted));
        assert_eq!(colon.name(), "C");
        assert_eq!(colon.package(), "A");
        assert_eq!(colon.components().collect::<Vec<_>>(), ["A", "B", "C"]);
        assert_eq!(colon.to_string(), "A.B:C");
        assert_ne!(colon, ObjectPath::parse("A.BC").unwrap());
    }

    #[test]
    fn package_names() {
        let a = PackageName::parse("WillowGame").unwrap();
        assert_eq!(a, PackageName::parse("WILLOWGAME").unwrap());
        assert_eq!(a.as_str(), "WillowGame");
        assert_eq!(
            PackageName::parse("Willow.upk"),
            Err(IdentError::Separator { at: 6 })
        );
        assert_eq!(
            PackageName::parse("a/b"),
            Err(IdentError::Separator { at: 1 })
        );
        assert_eq!(
            PackageName::parse("a:b"),
            Err(IdentError::Separator { at: 1 })
        );
        assert_eq!(PackageName::parse(""), Err(IdentError::Empty));
    }
}
