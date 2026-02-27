use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// CacheKey
// ---------------------------------------------------------------------------

/// Identifies a cached block by the SSTable file path and the byte offset of
/// the block within that file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// SSTable file path (used as a file identifier).
    pub file_id: String,
    /// Byte offset of the block within the file.
    pub block_offset: u64,
}

// ---------------------------------------------------------------------------
// BlockCache
// ---------------------------------------------------------------------------

/// A fixed-capacity, byte-accurate LRU block cache.
///
/// Entries are evicted in least-recently-used order once the cache exceeds
/// `capacity_bytes`.  The cache is intended to be wrapped in a
/// [`SharedBlockCache`] (`Arc<Mutex<BlockCache>>`) so it can be shared safely
/// across threads.
pub struct BlockCache {
    /// Maximum number of bytes the cache may hold.
    capacity_bytes: usize,
    /// Current number of bytes stored in the cache.
    current_bytes: usize,
    /// Primary storage: maps each cache key to the raw block data.
    cache: HashMap<CacheKey, Vec<u8>>,
    /// LRU order: front = oldest (next to evict), back = most recently used.
    order: VecDeque<CacheKey>,
}

impl BlockCache {
    /// Create a new empty `BlockCache` with the given byte capacity.
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            capacity_bytes,
            current_bytes: 0,
            cache: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Look up a block in the cache.
    ///
    /// On a hit the key is moved to the back of the LRU deque (most recently
    /// used).  Returns `None` on a miss.
    pub fn get(&mut self, key: &CacheKey) -> Option<&[u8]> {
        if !self.cache.contains_key(key) {
            return None;
        }

        // Promote to most-recently-used: remove from current position and
        // push to the back of the deque.
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key.clone());

        self.cache.get(key).map(|v| v.as_slice())
    }

    /// Insert a block into the cache.
    ///
    /// If inserting the block would exceed `capacity_bytes`, the
    /// least-recently-used entries are evicted until there is room (or until
    /// the cache is empty).  If the new block itself is larger than
    /// `capacity_bytes` it is silently dropped.
    pub fn insert(&mut self, key: CacheKey, data: Vec<u8>) {
        let data_len = data.len();

        // A block that is larger than the total capacity can never fit; skip.
        if data_len > self.capacity_bytes {
            return;
        }

        // If the key already exists, remove it first so we can re-insert with
        // updated data and refresh its LRU position.
        if self.cache.contains_key(&key) {
            let old_len = self.cache[&key].len();
            self.cache.remove(&key);
            self.current_bytes -= old_len;
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
        }

        // Evict LRU entries until we have enough room.
        while self.current_bytes + data_len > self.capacity_bytes {
            if let Some(oldest_key) = self.order.pop_front() {
                if let Some(old_data) = self.cache.remove(&oldest_key) {
                    self.current_bytes -= old_data.len();
                }
            } else {
                // Cache is already empty — nothing left to evict.
                break;
            }
        }

        self.cache.insert(key.clone(), data);
        self.current_bytes += data_len;
        self.order.push_back(key);
    }

    /// Returns the maximum byte capacity of this cache.
    pub fn capacity_bytes(&self) -> usize {
        self.capacity_bytes
    }

    /// Returns the number of bytes currently stored in the cache.
    pub fn used_bytes(&self) -> usize {
        self.current_bytes
    }
}

// ---------------------------------------------------------------------------
// SharedBlockCache
// ---------------------------------------------------------------------------

/// A thread-safe, reference-counted handle to a [`BlockCache`].
pub type SharedBlockCache = Arc<Mutex<BlockCache>>;

/// Create a new [`SharedBlockCache`] with the given byte capacity.
pub fn new_shared_cache(capacity_bytes: usize) -> SharedBlockCache {
    Arc::new(Mutex::new(BlockCache::new(capacity_bytes)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(file: &str, offset: u64) -> CacheKey {
        CacheKey {
            file_id: file.to_string(),
            block_offset: offset,
        }
    }

    // -----------------------------------------------------------------------
    // test_insert_and_get
    // -----------------------------------------------------------------------

    #[test]
    fn test_insert_and_get() {
        let mut cache = BlockCache::new(1024);

        let k1 = make_key("sst_1", 0);
        let k2 = make_key("sst_1", 100);
        let k3 = make_key("sst_2", 0);

        cache.insert(k1.clone(), vec![1, 2, 3]);
        cache.insert(k2.clone(), vec![4, 5, 6, 7]);
        cache.insert(k3.clone(), vec![8, 9]);

        assert_eq!(cache.get(&k1), Some(&[1u8, 2, 3][..]));
        assert_eq!(cache.get(&k2), Some(&[4u8, 5, 6, 7][..]));
        assert_eq!(cache.get(&k3), Some(&[8u8, 9][..]));
    }

    // -----------------------------------------------------------------------
    // test_lru_eviction
    // -----------------------------------------------------------------------

    /// Cache capacity is 100 bytes.
    /// Insert block A (60 bytes) — fits fine.
    /// Insert block B (60 bytes) — A must be evicted to make room.
    /// get(A) must return None, get(B) must return Some.
    #[test]
    fn test_lru_eviction() {
        let mut cache = BlockCache::new(100);

        let ka = make_key("sst", 0);
        let kb = make_key("sst", 100);

        cache.insert(ka.clone(), vec![0u8; 60]);
        cache.insert(kb.clone(), vec![1u8; 60]);

        // A should have been evicted to make room for B.
        assert!(
            cache.get(&ka).is_none(),
            "block A should be evicted after inserting block B"
        );
        assert!(
            cache.get(&kb).is_some(),
            "block B should be present in the cache"
        );
    }

    // -----------------------------------------------------------------------
    // test_used_bytes
    // -----------------------------------------------------------------------

    #[test]
    fn test_used_bytes() {
        let mut cache = BlockCache::new(200);

        assert_eq!(cache.used_bytes(), 0);

        let k1 = make_key("f", 0);
        let k2 = make_key("f", 50);
        let k3 = make_key("f", 100);

        cache.insert(k1.clone(), vec![0u8; 80]);
        assert_eq!(cache.used_bytes(), 80);

        cache.insert(k2.clone(), vec![0u8; 80]);
        assert_eq!(cache.used_bytes(), 160);

        // Inserting k3 (80 bytes) with only 40 bytes free forces eviction of k1.
        cache.insert(k3.clone(), vec![0u8; 80]);

        // After eviction(s), used_bytes must not exceed capacity.
        assert!(
            cache.used_bytes() <= cache.capacity_bytes(),
            "used_bytes {} exceeds capacity {}",
            cache.used_bytes(),
            cache.capacity_bytes()
        );

        // k1 was evicted; k3 must be present.
        assert!(cache.get(&k1).is_none(), "k1 should have been evicted");
        assert!(cache.get(&k3).is_some(), "k3 should be present");
    }

    // -----------------------------------------------------------------------
    // test_shared_cache
    // -----------------------------------------------------------------------

    #[test]
    fn test_shared_cache_basic() {
        let cache = new_shared_cache(512);
        let key = make_key("shared_sst", 0);
        {
            let mut guard = cache.lock().unwrap();
            guard.insert(key.clone(), vec![42u8; 10]);
        }
        {
            let mut guard = cache.lock().unwrap();
            let val = guard.get(&key);
            assert_eq!(val, Some(&[42u8; 10][..]));
        }
    }

    // -----------------------------------------------------------------------
    // test_capacity
    // -----------------------------------------------------------------------

    #[test]
    fn test_capacity_bytes() {
        let cache = BlockCache::new(4096);
        assert_eq!(cache.capacity_bytes(), 4096);
    }

    // -----------------------------------------------------------------------
    // test_oversized_block_not_inserted
    // -----------------------------------------------------------------------

    #[test]
    fn test_oversized_block_dropped() {
        let mut cache = BlockCache::new(10);
        let key = make_key("big", 0);
        // Block is larger than total capacity — should be silently dropped.
        cache.insert(key.clone(), vec![0u8; 20]);
        assert!(
            cache.get(&key).is_none(),
            "oversized block must not be inserted"
        );
        assert_eq!(cache.used_bytes(), 0);
    }
}
