//! Owner: lane W1-OBJ. The name table: names interned once, compared like FNames (architecture section 3.3).
//!
//! A name is interned case-insensitively with ASCII folding only, the way the C++ VM folds names (`lower()` in
//! `src/vm.cpp`, `std::tolower` in the C locale): `Foo`, `FOO` and `foo` are one [`NameId`]. The table keeps the
//! **first spelling** it was given and prints that one. This matches how FNames behave (one entry per case-folded
//! name) but differs from the C++ VM in one visible way: the C++ keeps each value's own spelling in a `std::string`,
//! so where a script or package spells one name two ways, the C++ prints each spelling and this table prints the
//! first. Output ports (W3-DECODE, W3-INTERP) that must stay byte-identical to the C++ check that case.
//!
//! `None` is always entry 0 ([`NameId::NONE`]), interned by [`NameTable::new`]. The empty string is an ordinary name
//! of its own (the C++ treats both `""` and `None` as a false name; [`Value::truth`](crate::value::Value::truth)
//! keeps that).

use crate::ids::NameId;
use ow_core::FixedHashMap;

/// Interned names. Lookups go through a fixed-hasher map; nothing iterates it, so its order never shows.
#[derive(Debug, Clone)]
pub struct NameTable {
    spellings: Vec<Box<str>>,
    by_folded: FixedHashMap<Box<str>, NameId>,
}

impl Default for NameTable {
    fn default() -> Self {
        Self::new()
    }
}

/// ASCII lower case, as `std::tolower` in the C locale; other bytes are kept.
fn fold(s: &str) -> Box<str> {
    s.to_ascii_lowercase().into_boxed_str()
}

impl NameTable {
    /// A table holding only `None` (entry 0).
    pub fn new() -> Self {
        let mut table = NameTable {
            spellings: Vec::new(),
            by_folded: FixedHashMap::default(),
        };
        let none = table.intern("None");
        debug_assert_eq!(none, NameId::NONE);
        table
    }

    /// The id of `s`, adding it with this spelling if no case variant of it is in the table yet.
    pub fn intern(&mut self, s: &str) -> NameId {
        let key = fold(s);
        if let Some(&id) = self.by_folded.get(&key) {
            return id;
        }
        let raw = u32::try_from(self.spellings.len()).expect("more than 2^32 names");
        let id = NameId::new(raw);
        self.spellings.push(s.into());
        self.by_folded.insert(key, id);
        id
    }

    /// The id of `s` (any case) if it is interned, without adding it.
    pub fn find(&self, s: &str) -> Option<NameId> {
        self.by_folded.get(&fold(s)).copied()
    }

    /// The first spelling interned for `n`.
    ///
    /// # Panics
    /// If `n` was not made by this table (an index past its end).
    pub fn text(&self, n: NameId) -> &str {
        &self.spellings[n.index()]
    }

    /// The first spelling of `n`, or `None` for an id this table did not make.
    pub fn get(&self, n: NameId) -> Option<&str> {
        self.spellings.get(n.index()).map(|s| &**s)
    }

    /// How many names are interned (at least 1: `None`).
    pub fn len(&self) -> usize {
        self.spellings.len()
    }

    /// Never true: `None` is always interned.
    pub fn is_empty(&self) -> bool {
        self.spellings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_entry_zero() {
        let mut t = NameTable::new();
        assert_eq!(t.len(), 1);
        assert_eq!(t.text(NameId::NONE), "None");
        assert_eq!(t.intern("NONE"), NameId::NONE);
        assert_eq!(t.find("none"), Some(NameId::NONE));
    }

    #[test]
    fn case_insensitive_and_the_first_spelling_is_kept() {
        let mut t = NameTable::new();
        let a = t.intern("MaxValue");
        assert_eq!(t.intern("MAXVALUE"), a);
        assert_eq!(t.intern("maxvalue"), a);
        assert_eq!(t.text(a), "MaxValue");
        let b = t.intern("MaxValueBaseValue");
        assert_ne!(a, b);
        assert_eq!(t.len(), 3);
        assert_eq!(t.find("maxVALUEbasevalue"), Some(b));
        assert_eq!(t.find("Missing"), None);
        assert_eq!(t.len(), 3, "find does not intern");
    }

    #[test]
    fn folding_is_ascii_only() {
        // std::tolower in the C locale leaves non-ASCII bytes alone, so these are two names.
        let mut t = NameTable::new();
        let upper = t.intern("\u{C4}rger");
        let lower = t.intern("\u{E4}rger");
        assert_ne!(upper, lower);
        assert_eq!(t.intern("\u{C4}RGER"), upper);
    }

    #[test]
    fn the_empty_name_is_its_own_entry() {
        let mut t = NameTable::new();
        let empty = t.intern("");
        assert_ne!(empty, NameId::NONE);
        assert_eq!(t.text(empty), "");
        assert_eq!(t.get(NameId::new(99)), None);
    }
}
