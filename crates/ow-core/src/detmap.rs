//! Owner: lane W0-SCAFFOLD (complete). The deterministic maps and sets rule and output code use (architecture rule
//! R6, 5.1 N7).
//!
//! [`DetMap`] iterates in key order, which is the order of the C++ `std::map` the CLI's JSON output and several
//! censuses use. Use it **only where key order is the oracle's order**. Where the oracle iterates in insertion order
//! (the weapon and item picks walk their candidates in the order they were added, `src/weapon_gen.cpp` 744-760), a
//! `DetMap` would change every pick: use [`crate::OrderedMap`] or a `Vec` there.
//!
//! The key type decides the order too. `ObjectPath` and `PackageName` order case-folded (with `:` as `.`), and two
//! keys that differ only in case are one key, so `DetMap<ObjectPath, _>` matches an oracle only if that oracle folds
//! its keys the same way. An oracle keyed by the raw string (a C++ `std::map<std::string, _>`, Python
//! `json.dumps(sort_keys=True)`) sorts by the exact bytes and keeps case variants apart: mirror it with
//! `DetMap<String, _>`.
//!
//! Hash maps are for lookups only and are never iterated in rule or output code. The only ones the workspace uses are
//! [`FixedHashMap`] and [`FixedHashSet`], built with a fixed hasher so nothing about them depends on a per-process
//! random seed. This file is the one place `tests/crate_edges_test.py` lets name the standard hash containers.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, DefaultHasher};

/// The key-ordered map: iteration is in `Ord` order of the keys, the same on every run and every machine.
pub type DetMap<K, V> = std::collections::BTreeMap<K, V>;

/// The key-ordered set.
pub type DetSet<K> = std::collections::BTreeSet<K>;

/// A hash map with a fixed hasher, for lookups only. Build it with `FixedHashMap::default()`.
pub type FixedHashMap<K, V> = HashMap<K, V, BuildHasherDefault<DefaultHasher>>;

/// A hash set with a fixed hasher, for lookups only. Build it with `FixedHashSet::default()`.
pub type FixedHashSet<K> = HashSet<K, BuildHasherDefault<DefaultHasher>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectPath;

    #[test]
    fn iterates_in_key_order_whatever_the_insertion_order() {
        let mut m: DetMap<&str, i32> = DetMap::new();
        for (k, v) in [("b", 2), ("c", 3), ("a", 1)] {
            m.insert(k, v);
        }
        assert_eq!(
            m.into_iter().collect::<Vec<_>>(),
            [("a", 1), ("b", 2), ("c", 3)]
        );
        let s: DetSet<i32> = [3, 1, 2].into_iter().collect();
        assert_eq!(s.into_iter().collect::<Vec<_>>(), [1, 2, 3]);
    }

    #[test]
    fn the_key_type_decides_order_and_merging() {
        // Raw strings: byte order, upper case first, case variants kept apart (a C++ std::map<std::string>).
        let raw: DetMap<String, i32> = [("b", 1), ("B", 2), ("a", 3)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        assert_eq!(raw.keys().collect::<Vec<_>>(), ["B", "a", "b"]);
        // Object paths: folded order, case variants merged into one key.
        let mut folded: DetMap<ObjectPath, i32> = DetMap::new();
        for (k, v) in [("b", 1), ("B", 2), ("a", 3)] {
            folded.insert(ObjectPath::parse(k).unwrap(), v);
        }
        assert_eq!(folded.len(), 2);
        assert_eq!(
            folded.keys().map(|k| k.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    #[test]
    fn fixed_hash_containers_look_up() {
        let mut m: FixedHashMap<&str, i32> = FixedHashMap::default();
        m.insert("k", 1);
        assert_eq!(m.get("k"), Some(&1));
        let s: FixedHashSet<i32> = [1, 2].into_iter().collect();
        assert!(s.contains(&2));
    }
}
