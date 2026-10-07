//! Small ordered tables backed by a sorted vector.
//!
//! Animation tables use deterministic key order without tree-node code for
//! every value type. Lookups take O(log n); insertion and removal take O(n).
//! Equal-key insertion keeps the stored key and returns the replaced value.

use std::borrow::Borrow;
use std::fmt;
use std::iter::FusedIterator;

#[derive(Clone, PartialEq, Eq)]
pub struct OrderedMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> OrderedMap<K, V> {
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator + FusedIterator {
        self.entries.iter().map(|(key, _)| key)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator + FusedIterator {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut V> + ExactSizeIterator + FusedIterator {
        self.entries.iter_mut().map(|(_, value)| value)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K: Ord, V> OrderedMap<K, V> {
    fn search<Q>(&self, key: &Q) -> Result<usize, usize>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.entries.binary_search_by(|(stored, _)| stored.borrow().cmp(key))
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.search(&key) {
            Ok(index) => Some(std::mem::replace(&mut self.entries[index].1, value)),
            Err(index) => {
                self.entries.insert(index, (key, value));
                None
            }
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.search(key).ok().map(|index| &self.entries[index].1)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let index = self.search(key).ok()?;
        Some(&mut self.entries[index].1)
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.search(key).is_ok()
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.search(key).ok().map(|index| self.entries.remove(index).1)
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for OrderedMap<K, V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_map().entries(self.iter()).finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OrderedSet<K> {
    entries: OrderedMap<K, ()>,
}

impl<K> Default for OrderedSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> OrderedSet<K> {
    pub const fn new() -> Self {
        Self { entries: OrderedMap::new() }
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator + FusedIterator {
        self.entries.keys()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K: Ord> OrderedSet<K> {
    pub fn insert(&mut self, key: K) -> bool {
        self.entries.insert(key, ()).is_none()
    }

    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.entries.contains_key(key)
    }
}

impl<K: fmt::Debug> fmt::Debug for OrderedSet<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;
    use std::collections::{BTreeMap, BTreeSet};

    fn map_rows(map: &OrderedMap<String, i32>) -> Vec<(&str, i32)> {
        map.iter().map(|(key, value)| (key.as_str(), *value)).collect()
    }

    fn tree_rows(map: &BTreeMap<String, i32>) -> Vec<(&str, i32)> {
        map.iter().map(|(key, value)| (key.as_str(), *value)).collect()
    }

    #[test]
    fn unordered_insert_and_replacement_match_tree_sorted_observables() {
        let mut map = OrderedMap::new();
        let mut tree = BTreeMap::new();
        assert!(map.is_empty());
        for (key, value) in [("z", 1), ("a", 2), ("中", 3), ("β", 4),
            ("A", 5), ("10", 6), ("2", 7), ("a", 8)] {
            assert_eq!(map.insert(key.to_owned(), value), tree.insert(key.to_owned(), value));
            assert_eq!(map_rows(&map), tree_rows(&tree));
            assert_eq!(map.len(), tree.len());
            assert_eq!(map.is_empty(), tree.is_empty());
        }
        assert_eq!(map.keys().map(String::as_str).collect::<Vec<_>>(), tree.keys().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(map.iter().rev().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), tree.iter().rev().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>());
        assert_eq!(map.clone(), map);
        assert_eq!(format!("{map:?}"), format!("{tree:?}"));
    }

    #[test]
    fn borrowed_lookup_mutation_remove_and_reinsert_match_tree() {
        let mut map = OrderedMap::default();
        let mut tree = BTreeMap::new();
        for (key, value) in [("delta", 4), ("alpha", 1), ("charlie", 3), ("bravo", 2)] {
            map.insert(key.to_owned(), value);
            tree.insert(key.to_owned(), value);
        }
        for key in ["alpha", "bravo", "charlie", "delta", "absent", ""] {
            assert_eq!(map.get(key), tree.get(key));
            assert_eq!(map.contains_key(key), tree.contains_key(key));
            assert_eq!(map.get_mut(key).map(|v| { *v += 10; *v }), tree.get_mut(key).map(|v| { *v += 10; *v }));
        }
        for (index, value) in map.values_mut().enumerate() { *value += index as i32; }
        for (index, value) in tree.values_mut().enumerate() { *value += index as i32; }
        assert_eq!(map_rows(&map), tree_rows(&tree));
        for key in ["bravo", "alpha", "delta", "missing", "charlie"] {
            assert_eq!(map.remove(key), tree.remove(key));
            assert_eq!(map_rows(&map), tree_rows(&tree));
        }
        assert!(map.is_empty());
        assert_eq!(map.insert("bravo".to_owned(), 99), tree.insert("bravo".to_owned(), 99));
        assert_eq!(map.get(&"bravo".to_owned()), tree.get(&"bravo".to_owned()));
        assert_eq!(map_rows(&map), tree_rows(&tree));
    }

    #[derive(Clone, Debug)]
    struct IdentityKey { text: &'static str, identity: u32 }
    impl PartialEq for IdentityKey { fn eq(&self, other: &Self) -> bool { self.text == other.text } }
    impl Eq for IdentityKey {}
    impl PartialOrd for IdentityKey { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) } }
    impl Ord for IdentityKey { fn cmp(&self, other: &Self) -> Ordering { self.text.cmp(other.text) } }
    impl Borrow<str> for IdentityKey { fn borrow(&self) -> &str { self.text } }
    #[derive(Debug, PartialEq, Eq)]
    struct NonDefaultValue(u32);

    #[test]
    fn equal_key_replacement_preserves_original_identity_without_default_bounds() {
        let mut map: OrderedMap<IdentityKey, NonDefaultValue> = OrderedMap::default();
        let mut tree = BTreeMap::new();
        for (identity, value) in [(11, 1), (22, 2), (33, 3)] {
            let key = IdentityKey { text: "same", identity };
            assert_eq!(map.insert(key.clone(), NonDefaultValue(value)), tree.insert(key, NonDefaultValue(value)));
            assert_eq!(map.keys().next().unwrap().identity, tree.keys().next().unwrap().identity);
            assert_eq!(map.keys().next().unwrap().identity, 11);
            assert_eq!(map.get("same"), tree.get("same"));
        }
        assert_eq!(map.remove("same"), tree.remove("same"));
        let fresh = IdentityKey { text: "same", identity: 44 };
        assert_eq!(map.insert(fresh.clone(), NonDefaultValue(4)), tree.insert(fresh, NonDefaultValue(4)));
        assert_eq!(map.keys().next().unwrap().identity, 44);
        assert_eq!(format!("{map:?}"), format!("{tree:?}"));
    }

    #[test]
    fn ordered_set_matches_tree_duplicate_flags_borrowed_membership_and_key_identity() {
        let mut set: OrderedSet<IdentityKey> = OrderedSet::default();
        let mut tree = BTreeSet::new();
        assert!(set.is_empty());
        for (text, identity) in [("z", 1), ("a", 2), ("m", 3), ("a", 4), ("z", 5)] {
            let key = IdentityKey { text, identity };
            assert_eq!(set.insert(key.clone()), tree.insert(key));
            assert_eq!(set.iter().map(|k| (k.text, k.identity)).collect::<Vec<_>>(), tree.iter().map(|k| (k.text, k.identity)).collect::<Vec<_>>());
            assert_eq!(set.len(), tree.len());
            assert_eq!(set.is_empty(), tree.is_empty());
        }
        for text in ["a", "m", "z", "missing", ""] { assert_eq!(set.contains(text), tree.contains(text)); }
        assert_eq!(set.iter().rev().map(|k| k.text).collect::<Vec<_>>(), tree.iter().rev().map(|k| k.text).collect::<Vec<_>>());
        assert_eq!(format!("{set:?}"), format!("{tree:?}"));
    }
}
