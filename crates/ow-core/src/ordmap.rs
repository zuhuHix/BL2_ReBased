//! Owner: lane W0-SCAFFOLD (complete). The insertion-ordered map (architecture rule R6, 5.1 N7).
//!
//! The weapon and item picks walk their candidates in the order they were added (`src/weapon_gen.cpp` 744-760
//! `PickPart`, `tools/weapon_recipe.py` `pick`, and every Python dict the tools build), so the map they use must
//! iterate in insertion order: a key-ordered [`crate::DetMap`] would change every weighted pick. [`OrderedMap`]
//! behaves like a Python dict on insert: a new key goes to the end; inserting an existing key keeps its first
//! position and takes the later value.
//!
//! It is a plain vector of pairs with a linear key search. That is deliberate: it is easy to check, and the maps rule
//! code builds here (part candidates, pool entries) are small. A caller with a large map that is only looked up, never
//! iterated, can keep a separate index.

/// A map that iterates in insertion order. See the module documentation for the insert rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        OrderedMap {
            entries: Vec::new(),
        }
    }
}

impl<K: Eq, V> OrderedMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The same as [`OrderedMap::new`], under the name the scaffold's placeholders share.
    pub fn new_empty() -> Self {
        Self::default()
    }

    fn position(&self, key: &K) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == key)
    }

    /// Inserts `value` under `key`. A new key goes to the end. An existing key keeps its position, takes `value` and
    /// the old value is returned.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.position(&key) {
            Some(i) => Some(std::mem::replace(&mut self.entries[i].1, value)),
            None => {
                self.entries.push((key, value));
                None
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.position(key).map(|i| &self.entries[i].1)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.position(key).map(|i| &mut self.entries[i].1)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.position(key).is_some()
    }

    /// The position of `key` in insertion order.
    pub fn index_of(&self, key: &K) -> Option<usize> {
        self.position(key)
    }
}

impl<K, V> OrderedMap<K, V> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry at `index` in insertion order.
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.entries.get(index).map(|(k, v)| (k, v))
    }

    /// The entries in insertion order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = &K> {
        self.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }
}

impl<K, V> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<K: Eq, V> FromIterator<(K, V)> for OrderedMap<K, V> {
    /// Inserts in iteration order with the [`OrderedMap::insert`] rule, as `dict(pairs)` does.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = OrderedMap::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl<K: Eq, V> Extend<(K, V)> for OrderedMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_in_insertion_order_not_key_order() {
        let mut m = OrderedMap::new();
        assert_eq!(m.insert("c", 3), None);
        assert_eq!(m.insert("a", 1), None);
        assert_eq!(m.insert("b", 2), None);
        assert_eq!(
            m.iter().collect::<Vec<_>>(),
            [(&"c", &3), (&"a", &1), (&"b", &2)]
        );
        assert_eq!(m.keys().copied().collect::<Vec<_>>(), ["c", "a", "b"]);
        assert_eq!(m.values().copied().collect::<Vec<_>>(), [3, 1, 2]);
    }

    #[test]
    fn a_repeated_key_keeps_its_first_position_and_takes_the_later_value() {
        // The Python dict rule: d = {}; d['c'] = 3; d['a'] = 1; d['c'] = 30 -> [('c', 30), ('a', 1)]
        let mut m = OrderedMap::new();
        m.insert("c", 3);
        m.insert("a", 1);
        assert_eq!(m.insert("c", 30), Some(3));
        assert_eq!(m.into_iter().collect::<Vec<_>>(), [("c", 30), ("a", 1)]);
        let m: OrderedMap<_, _> = [("x", 1), ("y", 2), ("x", 3)].into_iter().collect();
        assert_eq!(m.len(), 2);
        assert_eq!(m.get_index(0), Some((&"x", &3)));
        assert_eq!(m.index_of(&"y"), Some(1));
    }

    #[test]
    fn lookups() {
        let mut m: OrderedMap<String, i32> = OrderedMap::new_empty();
        assert!(m.is_empty());
        m.extend([("k".to_string(), 1)]);
        assert!(m.contains_key(&"k".to_string()));
        *m.get_mut(&"k".to_string()).unwrap() += 1;
        assert_eq!(m.get(&"k".to_string()), Some(&2));
        assert_eq!(m.get(&"missing".to_string()), None);
        assert_eq!(m.get_index(5), None);
    }
}
