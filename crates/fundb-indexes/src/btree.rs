// crates/fundb-indexes/src/btree.rs
//
// STORY-3-1: B+Tree scalar index backed by std::collections::BTreeMap.
// Supports equality and range queries over integer, string, and timestamp keys.

use std::collections::BTreeMap;
use std::ops::Bound;
use anyhow::Result;

/// A generic in-memory B+Tree index backed by `std::collections::BTreeMap`.
///
/// `K` must implement `Ord + Clone` plus serde serialization/deserialization.
/// `V` must implement `Clone` plus serde serialization/deserialization.
///
/// This wrapper provides a type-safe, ergonomic API for scalar-field indexing
/// inside FunDB. All heavy lifting (balancing, iteration order) is delegated
/// to the standard-library implementation.
pub struct BTree<K, V> {
    inner: BTreeMap<K, V>,
}

impl<K, V> BTree<K, V>
where
    K: Ord + Clone + serde::Serialize + serde::de::DeserializeOwned,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned,
{
    /// Create a new, empty `BTree` index.
    pub fn new() -> Self {
        BTree {
            inner: BTreeMap::new(),
        }
    }

    /// Insert a key-value pair into the index.
    ///
    /// If the key already exists its value is overwritten (last-write-wins).
    pub fn insert(&mut self, key: K, value: V) -> Result<()> {
        self.inner.insert(key, value);
        Ok(())
    }

    /// Look up an exact key. Returns `Ok(Some(value))` when found, `Ok(None)`
    /// when the key does not exist.
    pub fn get(&self, key: &K) -> Result<Option<V>> {
        Ok(self.inner.get(key).cloned())
    }

    /// Return an iterator over all key-value pairs whose key falls within the
    /// half-open interval `[from, to)` (bounds are caller-controlled via
    /// `std::ops::Bound`).
    ///
    /// Results are yielded in ascending key order.
    pub fn range<'a>(
        &'a self,
        from: Bound<&'a K>,
        to: Bound<&'a K>,
    ) -> impl Iterator<Item = (&'a K, &'a V)> {
        self.inner.range((from, to))
    }

    /// Remove the entry for `key`. Returns `Ok(true)` if the key was present,
    /// `Ok(false)` if it was not.
    pub fn delete(&mut self, key: &K) -> Result<bool> {
        Ok(self.inner.remove(key).is_some())
    }

    /// Return the number of entries currently stored in the index.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Return `true` when the index contains no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<K, V> Default for BTree<K, V>
where
    K: Ord + Clone + serde::Serialize + serde::de::DeserializeOwned,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned,
{
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Bound;
    use std::sync::{Arc, RwLock};

    // -----------------------------------------------------------------------
    // 1. Basic insert / get / delete round-trip
    // -----------------------------------------------------------------------
    #[test]
    fn test_insert_get_delete() {
        let mut idx: BTree<String, i32> = BTree::new();

        let pairs = [
            ("alpha".to_string(), 1),
            ("bravo".to_string(), 2),
            ("charlie".to_string(), 3),
            ("delta".to_string(), 4),
            ("echo".to_string(), 5),
        ];

        for (k, v) in &pairs {
            idx.insert(k.clone(), *v).unwrap();
        }

        assert_eq!(idx.len(), 5);

        // All keys should be retrievable.
        for (k, v) in &pairs {
            assert_eq!(idx.get(k).unwrap(), Some(*v));
        }

        // Delete key at index 2 ("charlie").
        let deleted_key = &pairs[2].0;
        assert!(idx.delete(deleted_key).unwrap());

        // After deletion the key must be gone and length reduced by 1.
        assert_eq!(idx.get(deleted_key).unwrap(), None);
        assert_eq!(idx.len(), 4);

        // Deleting an already-absent key returns false.
        assert!(!idx.delete(deleted_key).unwrap());
    }

    // -----------------------------------------------------------------------
    // 2. Range over all entries is sorted ascending
    // -----------------------------------------------------------------------
    #[test]
    fn test_range_sorted() {
        let mut idx: BTree<i64, i64> = BTree::new();

        // Deterministic pseudo-random sequence: i * 7 mod 997 for i in 0..100.
        // 997 is prime so the sequence has no repeated values.
        for i in 0i64..100 {
            let k = i * 7 % 997;
            idx.insert(k, k).unwrap();
        }

        assert_eq!(idx.len(), 100);

        let entries: Vec<(&i64, &i64)> =
            idx.range(Bound::Unbounded, Bound::Unbounded).collect();

        assert_eq!(entries.len(), 100);

        // Verify ascending order.
        for w in entries.windows(2) {
            assert!(w[0].0 < w[1].0, "expected ascending order: {:?} < {:?}", w[0].0, w[1].0);
        }
    }

    // -----------------------------------------------------------------------
    // 3. Bounded range returns exactly the expected subset
    // -----------------------------------------------------------------------
    #[test]
    fn test_range_bounded() {
        let mut idx: BTree<i32, i32> = BTree::new();

        for k in 1i32..=100 {
            idx.insert(k, k).unwrap();
        }

        let lo = 25i32;
        let hi = 75i32;
        let entries: Vec<(&i32, &i32)> =
            idx.range(Bound::Included(&lo), Bound::Included(&hi)).collect();

        assert_eq!(entries.len(), 51, "expected 51 entries (25..=75 inclusive)");

        for (k, _v) in &entries {
            assert!(
                **k >= 25 && **k <= 75,
                "key {} is outside [25, 75]",
                k
            );
        }
    }

    // -----------------------------------------------------------------------
    // 4. String prefix scan using lexicographic range
    // -----------------------------------------------------------------------
    #[test]
    fn test_prefix_scan_strings() {
        let mut idx: BTree<String, u32> = BTree::new();

        let words = ["fundb", "funql", "funx", "other", "elephant"];
        for (i, w) in words.iter().enumerate() {
            idx.insert(w.to_string(), i as u32).unwrap();
        }

        // All strings that start with "fun" satisfy: "fun" <= s < "fuo"
        // because the next character after 'n' in ASCII is 'o'.
        let lo = "fun".to_string();
        let hi = "fuo".to_string();

        let entries: Vec<(&String, &u32)> =
            idx.range(Bound::Included(&lo), Bound::Excluded(&hi)).collect();

        let keys: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();

        assert_eq!(
            entries.len(),
            3,
            "expected 3 'fun'-prefixed keys, got {:?}",
            keys
        );

        for (k, _) in &entries {
            assert!(
                k.starts_with("fun"),
                "unexpected key in prefix scan: {}",
                k
            );
        }

        // Exact membership check.
        assert!(keys.contains(&"fundb"));
        assert!(keys.contains(&"funql"));
        assert!(keys.contains(&"funx"));
    }

    // -----------------------------------------------------------------------
    // 5. Concurrent readers + one writer — no panic, no deadlock
    // -----------------------------------------------------------------------
    #[test]
    fn test_concurrent_read() {
        let shared: Arc<RwLock<BTree<i32, String>>> =
            Arc::new(RwLock::new(BTree::new()));

        let num_readers = 8usize;
        let reads_per_thread = 1_000usize;
        let writes = 1_000i32;

        // Seed the index with an initial entry so readers always have something
        // to query even before the writer thread starts.
        {
            let mut w = shared.write().unwrap();
            w.insert(0, "seed".to_string()).unwrap();
        }

        let mut handles = Vec::new();

        // Spawn reader threads.
        for t in 0..num_readers {
            let arc = Arc::clone(&shared);
            let h = std::thread::spawn(move || {
                for i in 0..reads_per_thread {
                    let key = ((t * reads_per_thread + i) % writes as usize) as i32;
                    let guard = arc.read().unwrap();
                    // We don't assert a specific value because the writer may or
                    // may not have inserted `key` yet — we just verify no panic.
                    let _ = guard.get(&key).unwrap();
                }
            });
            handles.push(h);
        }

        // Spawn writer thread.
        {
            let arc = Arc::clone(&shared);
            let h = std::thread::spawn(move || {
                for i in 0..writes {
                    let mut guard = arc.write().unwrap();
                    guard.insert(i, format!("value-{}", i)).unwrap();
                }
            });
            handles.push(h);
        }

        // Join all threads — any panic propagates here via unwrap().
        for h in handles {
            h.join().expect("thread panicked");
        }

        // Final sanity: all written entries are present.
        let guard = shared.read().unwrap();
        assert_eq!(guard.len(), writes as usize);
        for i in 0..writes {
            assert_eq!(guard.get(&i).unwrap(), Some(format!("value-{}", i)));
        }
    }
}
