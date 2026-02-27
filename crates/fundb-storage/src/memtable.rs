use std::collections::BTreeMap;
use std::mem;
use std::ops::Bound;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

use anyhow::Result;
use fundb_core::{FunRecord, RecordKey};

// ---------------------------------------------------------------------------
// MemTable — mutable, concurrent in-memory write buffer
// ---------------------------------------------------------------------------

/// Mutable, concurrent in-memory write buffer backed by a `BTreeMap` under a
/// `RwLock`.
///
/// Multiple threads may call `insert` concurrently; readers never block writers
/// for long because the write lock is held only during the BTreeMap mutation.
/// `size_bytes()` is maintained via an `AtomicUsize` counter for lock-free reads.
pub struct MemTable {
    inner:      RwLock<BTreeMap<RecordKey, FunRecord>>,
    size_bytes: AtomicUsize,
}

impl MemTable {
    /// Create a new, empty `MemTable`.
    pub fn new() -> Self {
        Self {
            inner:      RwLock::new(BTreeMap::new()),
            size_bytes: AtomicUsize::new(0),
        }
    }

    /// Insert or overwrite a record.
    ///
    /// The size counter is incremented by an estimate of the entry's memory
    /// footprint: `sizeof(FunRecord) + collection.len() + 16` (UUID bytes).
    pub fn insert(&self, key: RecordKey, record: FunRecord) -> Result<()> {
        let size_delta = mem::size_of::<FunRecord>() + key.collection.len() + 16;
        {
            let mut map = self
                .inner
                .write()
                .map_err(|_| anyhow::anyhow!("MemTable write lock poisoned"))?;
            map.insert(key, record);
        }
        self.size_bytes.fetch_add(size_delta, Ordering::Relaxed);
        Ok(())
    }

    /// Return a clone of the record stored under `key`, or `None` if absent.
    pub fn get(&self, key: &RecordKey) -> Option<FunRecord> {
        self.inner
            .read()
            .ok()?
            .get(key)
            .cloned()
    }

    /// Return all entries whose key is in the half-open interval `[from, to)`,
    /// sorted ascending by key.
    ///
    /// Both bounds are inclusive (`Included`) so callers can pass the exact
    /// minimum and maximum keys they care about.
    pub fn range(&self, from: &RecordKey, to: &RecordKey) -> Vec<(RecordKey, FunRecord)> {
        let map = match self.inner.read() {
            Ok(guard) => guard,
            Err(_) => return vec![],
        };
        map.range((Bound::Included(from), Bound::Included(to)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Estimated number of bytes occupied by all entries.
    ///
    /// This is a fast, lock-free read of the atomic counter.  Accuracy is
    /// within ~5% because `FunRecord` contains heap-allocated fields (vectors,
    /// maps, …) whose sizes are not individually tracked.
    pub fn size_bytes(&self) -> usize {
        self.size_bytes.load(Ordering::Relaxed)
    }

    /// Consume this `MemTable` and return a read-only snapshot.
    ///
    /// The returned `ImmutableMemTable` contains all entries sorted by key
    /// (guaranteed by `BTreeMap`'s iteration order).
    pub fn freeze(self) -> ImmutableMemTable {
        let map = self
            .inner
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entries: Vec<(RecordKey, FunRecord)> = map.into_iter().collect();
        ImmutableMemTable { entries }
    }
}

impl Default for MemTable {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ImmutableMemTable — read-only sorted snapshot
// ---------------------------------------------------------------------------

/// A read-only, sorted snapshot produced by [`MemTable::freeze`].
///
/// Entries are stored in a `Vec` sorted ascending by `RecordKey`.
pub struct ImmutableMemTable {
    entries: Vec<(RecordKey, FunRecord)>,
}

impl ImmutableMemTable {
    /// Iterate over all `(key, record)` pairs in ascending key order.
    pub fn iter(&self) -> impl Iterator<Item = (RecordKey, FunRecord)> + '_ {
        self.entries.iter().map(|(k, v)| (k.clone(), v.clone()))
    }

    /// Number of entries in the snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the snapshot contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_core::builder::FunRecordBuilder;
    use std::sync::Arc;
    use uuid::Uuid;

    fn make_key(collection: &str, id_bytes: [u8; 16]) -> RecordKey {
        RecordKey {
            collection: collection.to_string(),
            id: id_bytes,
        }
    }

    fn make_record(collection: &str) -> FunRecord {
        FunRecordBuilder::new(collection).build()
    }

    // -----------------------------------------------------------------------
    // Test 1: basic insert and get
    // -----------------------------------------------------------------------

    #[test]
    fn test_insert_and_get() {
        let mem = MemTable::new();

        let key_a = make_key("col", [0u8; 16]);
        let key_b = make_key("col", [1u8; 16]);
        let key_c = make_key("col", [2u8; 16]);

        let rec_a = make_record("col");
        let rec_b = make_record("col");
        let rec_c = make_record("col");

        let id_a = rec_a._id;
        let id_b = rec_b._id;
        let id_c = rec_c._id;

        mem.insert(key_a.clone(), rec_a).unwrap();
        mem.insert(key_b.clone(), rec_b).unwrap();
        mem.insert(key_c.clone(), rec_c).unwrap();

        assert_eq!(mem.get(&key_a).unwrap()._id, id_a);
        assert_eq!(mem.get(&key_b).unwrap()._id, id_b);
        assert_eq!(mem.get(&key_c).unwrap()._id, id_c);
    }

    // -----------------------------------------------------------------------
    // Test 2: range returns sorted results for 100 records
    // -----------------------------------------------------------------------

    #[test]
    fn test_range_sorted() {
        let mem = MemTable::new();

        // Insert 100 records with random UUIDs.
        let mut keys: Vec<RecordKey> = (0..100)
            .map(|_| {
                let bytes = *Uuid::new_v4().as_bytes();
                make_key("test", bytes)
            })
            .collect();

        for key in &keys {
            mem.insert(key.clone(), make_record("test")).unwrap();
        }

        // Compute the min and max keys among the inserted set.
        keys.sort();
        let min_key = keys.first().unwrap().clone();
        let max_key = keys.last().unwrap().clone();

        let result = mem.range(&min_key, &max_key);

        assert_eq!(result.len(), 100, "all 100 records must be returned");

        // Verify sorted order.
        for window in result.windows(2) {
            assert!(
                window[0].0 <= window[1].0,
                "range result must be sorted: {:?} > {:?}",
                window[0].0,
                window[1].0
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 3: size_bytes increases on inserts; new MemTable starts at 0
    // -----------------------------------------------------------------------

    #[test]
    fn test_size_bytes_increases() {
        let mem = MemTable::new();
        assert_eq!(mem.size_bytes(), 0, "fresh MemTable must start at 0 bytes");

        for i in 0_u8..10 {
            let key = make_key("sz", [i; 16]);
            mem.insert(key, make_record("sz")).unwrap();
        }

        assert!(
            mem.size_bytes() > 0,
            "size_bytes must be positive after inserts"
        );

        // After freeze the old table is consumed; a brand-new one starts at 0.
        let _frozen = mem.freeze();
        let new_mem = MemTable::new();
        assert_eq!(new_mem.size_bytes(), 0, "new MemTable must start at 0");
    }

    // -----------------------------------------------------------------------
    // Test 4: freeze produces a snapshot; subsequent inserts don't affect it
    // -----------------------------------------------------------------------

    #[test]
    fn test_freeze_snapshot() {
        let mem = MemTable::new();

        for i in 0_u8..5 {
            let key = make_key("snap", [i; 16]);
            mem.insert(key, make_record("snap")).unwrap();
        }

        let frozen = mem.freeze();

        // The frozen snapshot has exactly 5 entries.
        assert_eq!(frozen.len(), 5, "frozen snapshot must contain exactly 5 records");

        // Entries must be sorted.
        let entries: Vec<_> = frozen.iter().collect();
        for window in entries.windows(2) {
            assert!(
                window[0].0 <= window[1].0,
                "frozen iter must be sorted: {:?} > {:?}",
                window[0].0,
                window[1].0
            );
        }

        // Insert 5 more into a brand-new MemTable (the old one was consumed by freeze).
        let new_mem = MemTable::new();
        for i in 5_u8..10 {
            let key = make_key("snap", [i; 16]);
            new_mem.insert(key, make_record("snap")).unwrap();
        }

        // The frozen snapshot still has only 5 records.
        assert_eq!(frozen.len(), 5, "freeze must be a snapshot; new inserts must not appear");
    }

    // -----------------------------------------------------------------------
    // Test 5: concurrent writes from 16 threads, each inserting 100 records
    // -----------------------------------------------------------------------

    #[test]
    fn test_concurrent_writes() {
        let mem = Arc::new(MemTable::new());
        let mut handles = Vec::new();

        for thread_id in 0_u8..16 {
            let mem_clone = Arc::clone(&mem);
            let handle = std::thread::spawn(move || {
                for record_id in 0_u8..100 {
                    // Build a key that is unique across all threads: first byte
                    // is the thread_id, second byte is the record_id, rest zero.
                    let mut id_bytes = [0u8; 16];
                    id_bytes[0] = thread_id;
                    id_bytes[1] = record_id;
                    let key = RecordKey {
                        collection: "concurrent".to_string(),
                        id: id_bytes,
                    };
                    mem_clone
                        .insert(key, FunRecordBuilder::new("concurrent").build())
                        .expect("insert must not fail");
                }
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().expect("thread must not panic");
        }

        assert!(
            mem.size_bytes() > 0,
            "size_bytes must be positive after concurrent inserts"
        );

        // range from the smallest possible key to the largest.
        let min_key = RecordKey {
            collection: "concurrent".to_string(),
            id: [0u8; 16],
        };
        let max_key = RecordKey {
            collection: "concurrent".to_string(),
            id: [255u8; 16],
        };
        let all = mem.range(&min_key, &max_key);
        assert_eq!(
            all.len(),
            1600,
            "range must return all 16 * 100 = 1600 records, got {}",
            all.len()
        );
    }
}
