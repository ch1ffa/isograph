use std::{any::Any, collections::HashMap, hash::Hash};

#[derive(Debug)]
pub struct HashMapStorage<K: Hash + Eq>(HashMap<K, Box<dyn Any>>);

impl<K: Eq + Hash> HashMapStorage<K> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn contains_key(&self, k: &K) -> bool {
        self.0.contains_key(k)
    }

    pub fn get<T: 'static>(&self, k: &K) -> Option<&T> {
        self.0.get(k).and_then(|any| any.downcast_ref::<T>())
    }

    pub fn get_mut<T: 'static>(&mut self, k: &K) -> Option<&mut T> {
        self.0.get_mut(k).and_then(|any| any.downcast_mut())
    }

    pub fn insert<T: 'static>(&mut self, k: K, value: T) {
        self.0.insert(k, Box::new(value));
    }

    pub fn remove<T: 'static>(&mut self, k: &K) -> Option<T> {
        self.0
            .remove(k)
            .and_then(|any| any.downcast().ok())
            .map(|any| *any)
    }
}

impl<K: Eq + Hash> Default for HashMapStorage<K> {
    fn default() -> Self {
        Self::new()
    }
}
