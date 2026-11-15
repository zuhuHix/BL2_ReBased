//! Owner: lane W1-OBJ. The name table: names interned once, compared like FNames (architecture section 3.3).
//!
//! A name is interned case-insensitively with ASCII folding only, the way the C++ VM folds names (`lower()` in
//! `src/vm.cpp`, `std::tolower` in the C locale): `Foo`, `FOO` and `foo` are one [`NameId`]. The table keeps the
//! **first spelling** it was given and prints that one. This matches how FNames behave (one entry per case-folded
//! name) but differs from the C++ VM in one visible way: the C++ keeps each value's own spelling in a `std::string`,
//! so where a script or package spells one name two ways, the C++ prints each spelling and this table prints the
//! first. Output ports (W3-DECODE, W3-INTERP) that must stay byte-identical to the C++ check that case.
//!
//! **Instance numbers.** As a UE3 FName, a [`NameId`] is a base entry plus a number. [`NameTable::intern`] splits a
//! trailing `_<digits>` off into the number: the text must end in `_` and one or more ASCII digits, the part before
//! the `_` must not be empty, the digits must not start with `0` unless they are exactly `0`, and the value must be
//! below `i32::MAX` (FName's number is an `int32` holding value + 1). `Projectile_1234` is entry `Projectile` with
//! number 1235; `Foo_0` is `Foo` with number 1; `Foo_01`, `_5` and `Foo_` are whole entries. Splitting and printing are
//! inverse, so the text a name prints is the text it was interned from (up to the first spelling of the base). The
//! rule is the public UE3 convention, UNVERIFIED for this executable. Packages store the number themselves
//! ([`NameTable::intern_number`]), which goes through the same text so that one text is always one id.
//!
//! `None` is always entry 0 ([`NameId::NONE`]), interned by [`NameTable::new`]. The empty string is an ordinary name
//! of its own (the C++ treats both `""` and `None` as a false name; [`Value::truth`](crate::value::Value::truth)
//! keeps that).

use std::borrow::Cow;

use crate::ids::NameId;
use ow_core::FixedHashMap;

/// Interned names. Lookups go through a fixed-hasher map; nothing iterates it, so its order never shows.
#[derive(Debug, Clone)]
pub struct NameTable {
    spellings: Vec<Box<str>>,
    by_folded: FixedHashMap<Box<str>, u32>,
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

/// The base text and instance number of `s` (module documentation).
fn split(s: &str) -> (&str, u32) {
    let digits = s.bytes().rev().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits == s.len() {
        return (s, 0);
    }
    let (head, tail) = s.split_at(s.len() - digits);
    let Some(base) = head.strip_suffix('_') else {
        return (s, 0);
    };
    if base.is_empty() || (tail.len() > 1 && tail.starts_with('0')) {
        return (s, 0);
    }
    match tail.parse::<u32>() {
        Ok(value) if value < i32::MAX as u32 => (base, value + 1),
        _ => (s, 0),
    }
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

    fn entry(&mut self, base: &str) -> u32 {
        let key = fold(base);
        if let Some(&index) = self.by_folded.get(&key) {
            return index;
        }
        let index = u32::try_from(self.spellings.len()).expect("more than 2^32 names");
        self.spellings.push(base.into());
        self.by_folded.insert(key, index);
        index
    }

    /// The id of `s`, with any instance-number suffix split off, adding the base with this spelling if no case
    /// variant of it is in the table yet.
    pub fn intern(&mut self, s: &str) -> NameId {
        let (base, number) = split(s);
        NameId::new(self.entry(base), number)
    }

    /// The id of the name a package's name reference stores as base text `base` and instance `number`. It goes
    /// through the text the pair prints as, so one text always gives one id: `("Foo_3", 0)` is the same name as
    /// `("Foo", 4)` and as `intern("Foo_3")`.
    pub fn intern_number(&mut self, base: &str, number: u32) -> NameId {
        match number {
            0 => self.intern(base),
            n => self.intern(&format!("{base}_{}", n - 1)),
        }
    }

    /// The name with `n`'s base text and instance `number` (`number` 0: the base alone), through
    /// [`NameTable::intern_number`], so the result is the one id of its text: for `n = intern("Foo_5_3")` (base
    /// `Foo_5`), `with_number(n, 0)` is `intern("Foo_5")`, which is the entry `Foo` with number 6.
    ///
    /// # Panics
    /// If `n` was not made by this table.
    pub fn with_number(&mut self, n: NameId, number: u32) -> NameId {
        let base = self.base_text(n).to_owned();
        self.intern_number(&base, number)
    }

    /// The id of `s` (any case) if its base is interned, without adding it.
    pub fn find(&self, s: &str) -> Option<NameId> {
        let (base, number) = split(s);
        self.by_folded
            .get(&fold(base))
            .map(|&index| NameId::new(index, number))
    }

    /// The first spelling of the base of `n`, without the instance number.
    ///
    /// # Panics
    /// If `n` was not made by this table (an index past its end).
    pub fn base_text(&self, n: NameId) -> &str {
        &self.spellings[n.index()]
    }

    /// The text of `n`: the first spelling of its base, then `_<number - 1>` if it has a number.
    ///
    /// # Panics
    /// If `n` was not made by this table (an index past its end).
    pub fn text(&self, n: NameId) -> Cow<'_, str> {
        self.get(n).expect("a NameId from another name table")
    }

    /// The text of `n`, or `None` for an id this table did not make.
    pub fn get(&self, n: NameId) -> Option<Cow<'_, str>> {
        let base = self.spellings.get(n.index())?;
        Some(match n.number() {
            0 => Cow::Borrowed(&**base),
            number => Cow::Owned(format!("{base}_{}", number - 1)),
        })
    }

    /// How many base texts are interned (at least 1: `None`). Numbered names add none.
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
        assert_eq!(t.get(NameId::new(99, 0)), None);
    }

    #[test]
    fn numbered_names_share_one_entry() {
        let mut t = NameTable::new();
        let first = t.intern("Projectile_1234");
        assert_eq!((t.base_text(first), first.number()), ("Projectile", 1235));
        assert_eq!(t.text(first), "Projectile_1234");
        let entries = t.len();
        for k in 0..1000 {
            let n = t.intern(&format!("Projectile_{k}"));
            assert_eq!(n.index(), first.index());
            assert_eq!(t.text(n), format!("Projectile_{k}"));
        }
        assert_eq!(
            t.len(),
            entries,
            "spawning numbered names does not grow the table"
        );
        let plain = t.intern("projectile");
        assert_eq!((plain.index(), plain.number()), (first.index(), 0));
        assert_eq!(t.intern("PROJECTILE_1234"), first);
        assert_ne!(t.intern("Projectile_1233"), first);
        assert_eq!(
            t.find("projectile_7"),
            Some(NameId::new(first.raw_index(), 8))
        );
    }

    #[test]
    fn what_is_not_split() {
        let mut t = NameTable::new();
        assert_eq!(t.intern("Foo_0").number(), 1);
        assert_eq!(t.intern("Foo_9").number(), 10);
        for whole in [
            "Foo_01",
            "_5",
            "Foo_",
            "Foo5",
            "12",
            "Foo_2147483647",
            "Foo_99999999999",
        ] {
            let n = t.intern(whole);
            assert_eq!(n.number(), 0, "{whole}");
            assert_eq!(t.base_text(n), whole);
        }
        assert_eq!(t.intern("Foo_2147483646").number(), i32::MAX as u32);
        // Only the last suffix is split.
        let n = t.intern("Foo__3");
        assert_eq!((t.base_text(n), n.number()), ("Foo_", 4));
        assert_eq!(t.text(n), "Foo__3");
    }

    #[test]
    fn renumbering_keeps_one_id_per_text() {
        let mut t = NameTable::new();
        // Foo_5_3: base Foo_5, number 4. Without its number it is the text Foo_5, whose id is (Foo, 6).
        let a = t.intern("Foo_5_3");
        assert_eq!((t.base_text(a), a.number()), ("Foo_5", 4));
        let plain = t.with_number(a, 0);
        assert_eq!(plain, t.intern("Foo_5"));
        assert_eq!((t.base_text(plain), plain.number()), ("Foo", 6));
        assert_ne!(
            plain,
            NameId::new(a.raw_index(), 0),
            "a raw renumbering would mint a second id"
        );
        assert_eq!(t.with_number(a, 4), a);
        assert_eq!(t.with_number(a, 1), t.intern("Foo_5_0"));
        // Foo_3_4 both ways: down to Foo_3, and from Foo_3's base (Foo) up again.
        let b = t.intern("Foo_3_4");
        assert_eq!((t.base_text(b), b.number()), ("Foo_3", 5));
        let foo_3 = t.with_number(b, 0);
        assert_eq!(foo_3, t.intern("Foo_3"));
        assert_eq!(t.text(foo_3), "Foo_3");
        assert_eq!(
            t.with_number(foo_3, 5),
            t.intern("Foo_4"),
            "Foo_3's base is Foo"
        );
        assert_eq!(t.intern_number("Foo_3", 5), b);
        // Every result prints as it compares.
        for n in [a, plain, b, foo_3] {
            let text = t.text(n).into_owned();
            assert_eq!(t.intern(&text), n, "{text}");
        }
    }

    #[test]
    fn package_names_carry_their_number() {
        let mut t = NameTable::new();
        let n = t.intern_number("Actor_5", 2);
        assert_eq!((t.base_text(n), n.number()), ("Actor_5", 2));
        assert_eq!(t.text(n), "Actor_5_1");
        // One text, one id, however it was stored.
        assert_eq!(t.intern_number("Foo_3", 0), t.intern("Foo_3"));
        assert_eq!(t.intern_number("Foo", 4), t.intern("Foo_3"));
        assert_eq!(t.intern_number("", 4), t.intern("_3"));
        assert_eq!(t.intern_number("Foo", u32::MAX), t.intern("Foo_4294967294"));
        assert_eq!(t.intern_number("None", 0), NameId::NONE);
        assert_eq!(t.intern_number("None", 1), NameId::new(0, 1));
    }
}
