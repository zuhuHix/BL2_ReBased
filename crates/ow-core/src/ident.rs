//! Owner: lane W0-SCAFFOLD (complete). Object paths and package names.
//!
//! Both compare like an FName: case-insensitively, with ASCII case folding only, the way the C++ reader folds names
//! (`folded()` in `src/package.cpp`, `std::tolower` in the C locale). Each value keeps the spelling it was made from
//! for printing. Equality, ordering and hashing all use the folded form, so `DetMap` keys and lookups agree.
//!
//! An object path is the dot-joined outer chain that `Package::path` prints in the C++ reader
//! (`GD_Weap_Shared.Foo.Bar`): its first component is the outermost object, which for a cooked package's top-level
//! export is the package-like group the tools call its package. Nothing else about the path is interpreted here.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Why a text is not a valid object path or package name.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentError {
    /// The text is empty.
    Empty,
    /// A component between dots is empty (a leading, trailing or doubled dot); `at` is its byte offset.
    EmptyComponent { at: usize },
    /// A control character (below 0x20, or 0x7F) at byte offset `at`.
    ControlCharacter { at: usize },
    /// A package name holds a `.`, `/` or `\` at byte offset `at`.
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

fn folded_cmp(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

fn folded_hash<H: Hasher>(s: &str, state: &mut H) {
    for b in s.bytes() {
        state.write_u8(b.to_ascii_lowercase());
    }
    state.write_u8(0xFF); // terminator, so "ab"+"c" and "a"+"bc" differ when hashed in sequence
}

macro_rules! folded_traits {
    ($t:ident) => {
        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                self.0.eq_ignore_ascii_case(&other.0)
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

/// A dot-separated object path, compared case-insensitively.
#[derive(Debug, Clone)]
pub struct ObjectPath(Box<str>);

folded_traits!(ObjectPath);

impl ObjectPath {
    /// Accepts any non-empty text without control characters whose dot-separated components are all non-empty.
    pub fn parse(s: &str) -> Result<Self, IdentError> {
        check_text(s)?;
        let mut start = 0;
        for (i, b) in s.bytes().enumerate() {
            if b == b'.' {
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
        self.0.split('.').next().unwrap_or(&self.0)
    }

    /// The last component: the object's own name.
    pub fn name(&self) -> &str {
        self.0.rsplit('.').next().unwrap_or(&self.0)
    }

    /// The components from the outermost to the object itself.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
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
    /// Accepts any non-empty text without control characters, dots or path separators.
    pub fn parse(s: &str) -> Result<Self, IdentError> {
        check_text(s)?;
        if let Some(at) = s.bytes().position(|b| b == b'.' || b == b'/' || b == b'\\') {
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
        assert_eq!(PackageName::parse(""), Err(IdentError::Empty));
    }
}
