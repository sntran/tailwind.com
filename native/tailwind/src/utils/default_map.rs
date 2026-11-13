//! A map with default values: `utils/default-map.ts` of Tailwind CSS.

use std::borrow::Borrow;
use crate::hash::HashMap;
use std::hash::Hash;

/// A map that makes a value for each key that it does not have. The map
/// keeps the value, so the factory runs one time for each key.
///
/// The upstream factory also gets the map. Here the factory gets only the
/// key, because the map is in use when the factory runs.
pub struct DefaultMap<K, V, F> {
    map: HashMap<K, V>,
    factory: F,
}

impl<K: Eq + Hash, V, F: FnMut(&K) -> V> DefaultMap<K, V, F> {
    /// A map with a factory for the default values.
    pub fn new(factory: F) -> Self {
        DefaultMap { map: HashMap::default(), factory }
    }

    /// The value for the key. The factory makes the value when the map does
    /// not have the key. The key is copied only in that condition.
    pub fn get<Q>(&mut self, key: &Q) -> &V
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ToOwned<Owned = K> + ?Sized,
    {
        if !self.map.contains_key(key) {
            let owned = key.to_owned();
            let value = (self.factory)(&owned);
            self.map.insert(owned, value);
        }
        &self.map[key]
    }

    /// The value for the key, only when the map has the key.
    pub fn peek<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.get(key)
    }

    /// `true` when the map has the key.
    pub fn has<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.contains_key(key)
    }

    /// Set the value for the key.
    pub fn set(&mut self, key: K, value: V) -> Option<V> {
        self.map.insert(key, value)
    }

    /// Remove the key.
    pub fn delete<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.map.remove(key)
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// The number of keys.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// `true` when the map has no key.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The keys and values, in no specified order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map.iter()
    }
}
