//! Owner: lane W0-SCAFFOLD (complete). The key-ordered map rule and output code iterate (architecture rule R6, N7).
//!
//! [`DetMap`] iterates in key order, which is the order of the C++ `std::map` the CLI's JSON output and several
//! censuses use. Use it **only where key order is the oracle's order**. Where the oracle iterates in insertion order
//! (the weapon and item picks walk their candidates in the order they were added, `src/weapon_gen.cpp` 744-760), a
//! `DetMap` would change every pick: use [`crate::OrderedMap`] or a `Vec` there.
//!
//! `HashMap` and `HashSet` are never iterated in rule or output code; for lookups only, they are built with a fixed
//! hasher (`BuildHasherDefault<DefaultHasher>`), never `HashMap::new`.

/// The key-ordered map: iteration is in `Ord` order of the keys, the same on every run and every machine.
pub type DetMap<K, V> = std::collections::BTreeMap<K, V>;

/// The key-ordered set.
pub type DetSet<K> = std::collections::BTreeSet<K>;

#[cfg(test)]
mod tests {
    use super::*;

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
}
