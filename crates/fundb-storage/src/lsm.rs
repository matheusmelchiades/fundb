//! LSM-tree storage engine — STORY-4-4
//!
//! [`LsmTree`] is the top-level entry point for all reads and writes in
//! FunDB.  It coordinates:
//!
//! 1. **Active [`MemTable`]** — the mutable in-memory write buffer.
//! 2. **WAL** — every mutation is written to the [`Wal`] before it touches
//!    the memtable, providing crash-recovery guarantees.
//! 3. **Immutable [`ImmutableMemTable`]s** — when the active memtable exceeds
//!    [`LsmTree::flush_threshold_bytes`] it is frozen and queued for flushing
//!    to disk as an SSTable.
//! 4. **SSTable files** — sorted, immutable on-disk segments stored under
//!    `sstable_dir`.  New files are named by their creation timestamp in
//!    nanoseconds so that "newest" order corresponds to reverse-alphabetical
//!    order of file names.
//! 5. **[`BlockCache`]** — an LRU cache for record data loaded from SSTables.
//! 6. **[`CompactionPolicy`]** — drives background merges of SSTable files.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use tokio::sync::RwLock as TokioRwLock;

use fundb_core::{FunRecord, FunRecordBuilder, RecordKey};

use crate::{
    block_cache::{new_shared_cache, CacheKey, SharedBlockCache},
    compaction::CompactionPolicy,
    memtable::{ImmutableMemTable, MemTable},
    sstable::{merge_sstables, SstableReader, SstableWriter},
    wal::{Wal, WalEntry},
};

// ---------------------------------------------------------------------------
// LsmTree
// ---------------------------------------------------------------------------

/// Top-level LSM-tree storage engine.
///
/// All public methods are safe to call concurrently.  Internally, short-lived
/// `std::sync` locks guard the memtable and WAL; longer-lived async operations
/// (SSTable flushing, compaction) hold `tokio::sync::RwLock` guards.
pub struct LsmTree {
    /// Mutable in-memory write buffer.  Protected by a `TokioRwLock` so that
    /// async tasks can await the lock without blocking the thread pool.
    active: Arc<TokioRwLock<MemTable>>,

    /// Append-only write-ahead log.
    wal: Arc<Mutex<Wal>>,

    /// Read-only memtable snapshots waiting to be flushed to SSTable files.
    /// Ordered oldest-first; new snapshots are pushed to the back.
    immutables: Arc<TokioRwLock<Vec<ImmutableMemTable>>>,

    /// Directory where SSTable (`.sst`) files are stored.
    sstable_dir: PathBuf,

    /// Shared LRU block cache for SSTable reads.
    block_cache: SharedBlockCache,

    /// Active memtable size at which a flush is triggered (bytes).
    flush_threshold_bytes: usize,

    /// Policy that decides when and which files to compact.
    compaction_policy: CompactionPolicy,
}

impl LsmTree {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Open (or create) an LSM-tree rooted at `dir`.
    ///
    /// - Creates `dir` if it does not exist.
    /// - Opens the WAL at `<dir>/wal.log`, replaying any existing entries into
    ///   the active memtable so that no committed data is lost on restart.
    /// - Initialises the block cache with the given byte capacity.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be created, the WAL cannot be
    /// opened, or WAL recovery fails.
    pub fn open(dir: impl AsRef<Path>, cache_cap_bytes: usize) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();

        // Create the storage directory (and parents) if needed.
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create storage directory: {}", dir.display()))?;

        let wal_path = dir.join("wal.log");

        // Replay existing WAL entries into a fresh memtable.
        let active = MemTable::new();
        if wal_path.exists() {
            let entries = Wal::recover(&wal_path)
                .with_context(|| format!("WAL recovery failed at {}", wal_path.display()))?;
            for entry_result in entries {
                // Ignore individual entry errors — stop at first corruption.
                let Ok(entry) = entry_result else { break };
                match entry {
                    WalEntry::Write { key, record, .. } => {
                        // Best-effort insert; ignore errors during recovery.
                        let _ = active.insert(key, record);
                    }
                    WalEntry::Delete { .. } => {
                        // Tombstones: skip for now (the tombstone is already in
                        // the SSTable from the previous flush).
                    }
                    WalEntry::TxnCommit(_)
                    | WalEntry::TxnAbort(_)
                    | WalEntry::Checkpoint(_) => {}
                }
            }
        }

        let wal = Wal::open(&wal_path)
            .with_context(|| format!("failed to open WAL at {}", wal_path.display()))?;

        let block_cache = new_shared_cache(cache_cap_bytes);

        Ok(Self {
            active: Arc::new(TokioRwLock::new(active)),
            wal: Arc::new(Mutex::new(wal)),
            immutables: Arc::new(TokioRwLock::new(Vec::new())),
            sstable_dir: dir,
            block_cache,
            flush_threshold_bytes: 64 * 1024 * 1024, // 64 MiB
            compaction_policy: CompactionPolicy::Leveled {
                level_size_ratio: 10,
                max_levels: 7,
            },
        })
    }

    // -----------------------------------------------------------------------
    // Write
    // -----------------------------------------------------------------------

    /// Insert or overwrite a record.
    ///
    /// 1. Appends a [`WalEntry::Write`] to the WAL.
    /// 2. Inserts the record into the active [`MemTable`].
    /// 3. If the memtable has grown past [`flush_threshold_bytes`], triggers
    ///    an asynchronous flush.
    pub async fn write(&self, key: RecordKey, record: FunRecord) -> Result<()> {
        // Write to WAL first for durability.
        {
            let mut wal = self
                .wal
                .lock()
                .map_err(|_| anyhow::anyhow!("WAL mutex poisoned"))?;
            wal.append(WalEntry::Write {
                txn_id: 0,
                key: key.clone(),
                record: record.clone(),
            })
            .context("WAL append failed during write")?;
        }

        // Insert into the active memtable.
        {
            let active = self.active.read().await;
            active
                .insert(key, record)
                .context("memtable insert failed")?;
        }

        // Check if we should flush.
        let size = self.active.read().await.size_bytes();
        if size >= self.flush_threshold_bytes {
            self.flush_memtable().await?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Delete
    // -----------------------------------------------------------------------

    /// Mark a record as deleted.
    ///
    /// 1. Appends a [`WalEntry::Delete`] to the WAL.
    /// 2. Inserts a tombstone (an empty `FunRecord` with `_confidence = 0.0`)
    ///    into the active [`MemTable`] so that subsequent reads return `None`.
    pub async fn delete(&self, key: RecordKey) -> Result<()> {
        // Write to WAL.
        {
            let mut wal = self
                .wal
                .lock()
                .map_err(|_| anyhow::anyhow!("WAL mutex poisoned"))?;
            wal.append(WalEntry::Delete {
                txn_id: 0,
                key: key.clone(),
            })
            .context("WAL append failed during delete")?;
        }

        // Insert a tombstone record so the key is shadowed in the memtable.
        // We use `_confidence = 0.0` as the tombstone marker (matching the
        // convention used in `sstable.rs` tests).
        let tombstone = tombstone_record(&key.collection);
        {
            let active = self.active.read().await;
            active
                .insert(key, tombstone)
                .context("memtable tombstone insert failed")?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Get
    // -----------------------------------------------------------------------

    /// Point-lookup for a single record.
    ///
    /// Search order: active memtable → immutables (newest first) → SSTables
    /// (newest first).  Returns `None` if no non-tombstone record is found.
    pub async fn get(&self, key: &RecordKey) -> Result<Option<FunRecord>> {
        // 1. Check active memtable.
        {
            let active = self.active.read().await;
            if let Some(record) = active.get(key) {
                return Ok(filter_tombstone(record));
            }
        }

        // 2. Check immutables, most recent first.
        {
            let immutables = self.immutables.read().await;
            for imm in immutables.iter().rev() {
                if let Some(record) = imm.iter().find(|(k, _)| k == key).map(|(_, v)| v) {
                    return Ok(filter_tombstone(record));
                }
            }
        }

        // 3. Scan SSTables, newest first.
        let sst_files = sorted_sst_files(&self.sstable_dir)?;
        for path in sst_files.iter().rev() {
            let path_str = path.to_string_lossy().to_string();
            let cache_key = CacheKey {
                file_id: format!("{}:{}", path_str, rmp_serde::to_vec(key)
                    .map(|b| hex_bytes(&b))
                    .unwrap_or_default()),
                block_offset: 0,
            };

            // Check block cache first.
            {
                let mut cache = self
                    .block_cache
                    .lock()
                    .map_err(|_| anyhow::anyhow!("block cache mutex poisoned"))?;
                if let Some(cached_bytes) = cache.get(&cache_key) {
                    // Deserialize cached record.
                    if let Ok(record) = rmp_serde::from_slice::<FunRecord>(cached_bytes) {
                        return Ok(filter_tombstone(record));
                    }
                }
            }

            // Open SSTable and perform lookup.
            let reader = SstableReader::open(path)
                .with_context(|| format!("failed to open SSTable: {}", path.display()))?;

            if let Some(record) = reader.get(key)? {
                // Populate block cache.
                if let Ok(bytes) = rmp_serde::to_vec(&record) {
                    let mut cache = self
                        .block_cache
                        .lock()
                        .map_err(|_| anyhow::anyhow!("block cache mutex poisoned"))?;
                    cache.insert(cache_key, bytes);
                }
                return Ok(filter_tombstone(record));
            }
        }

        Ok(None)
    }

    // -----------------------------------------------------------------------
    // Scan
    // -----------------------------------------------------------------------

    /// Range scan returning all records with `from <= key <= to`.
    ///
    /// Results are merged from the active memtable, all immutables, and all
    /// SSTables.  The latest write wins for each key.  Tombstones are
    /// suppressed from the output.
    ///
    /// Returns a `Vec` sorted ascending by [`RecordKey`].
    pub async fn scan(
        &self,
        _collection: &str,
        from: &RecordKey,
        to: &RecordKey,
    ) -> Result<Vec<(RecordKey, FunRecord)>> {
        // Collect all (key, record) pairs into a BTreeMap; later insertions
        // overwrite earlier ones so the newest value wins.  We process sources
        // oldest-first so that newer sources overwrite.
        let mut merged: BTreeMap<RecordKey, FunRecord> = BTreeMap::new();

        // 1. SSTables — oldest files first.
        let sst_files = sorted_sst_files(&self.sstable_dir)?;
        for path in &sst_files {
            let reader = SstableReader::open(path)
                .with_context(|| format!("failed to open SSTable: {}", path.display()))?;
            for result in reader.range(from, to) {
                let (key, record) = result?;
                merged.insert(key, record);
            }
        }

        // 2. Immutables — oldest first.
        {
            let immutables = self.immutables.read().await;
            for imm in immutables.iter() {
                for (key, record) in imm.iter() {
                    if &key >= from && &key <= to {
                        merged.insert(key, record);
                    }
                }
            }
        }

        // 3. Active memtable — newest, overwrites everything.
        {
            let active = self.active.read().await;
            for (key, record) in active.range(from, to) {
                merged.insert(key, record);
            }
        }

        // Filter tombstones and collect into a sorted Vec.
        let result = merged
            .into_iter()
            .filter(|(_, record)| !is_tombstone(record))
            .collect();

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Flush
    // -----------------------------------------------------------------------

    /// Freeze the active memtable and asynchronously flush it to a new SSTable.
    ///
    /// Steps:
    /// 1. Swap the active [`MemTable`] for a fresh one, obtaining the old one.
    /// 2. Freeze the old memtable into an [`ImmutableMemTable`] and push it to
    ///    the immutables list.
    /// 3. Spawn a Tokio task that writes the SSTable file.
    /// 4. On completion, remove the `ImmutableMemTable` from the list.
    /// 5. Check whether compaction is warranted.
    pub async fn flush_memtable(&self) -> Result<()> {
        // Swap in a new empty memtable and take ownership of the old one.
        let old_memtable = {
            let mut active = self.active.write().await;
            std::mem::replace(&mut *active, MemTable::new())
        };

        // Freeze it.
        let imm = old_memtable.freeze();

        // Push to the immutables list.
        let imm_idx = {
            let mut immutables = self.immutables.write().await;
            immutables.push(imm);
            immutables.len() - 1 // index of the newly pushed entry
        };

        // Generate a timestamped SSTable filename.
        let ts_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let sst_path = self.sstable_dir.join(format!("{:030}.sst", ts_nanos));

        // Clone Arc handles to move into the spawned task.
        let immutables_arc = Arc::clone(&self.immutables);

        // Spawn a task to write the SSTable file.
        tokio::spawn(async move {
            // Re-read the immutable from the list.
            let entries: Vec<(RecordKey, FunRecord)> = {
                let imms = immutables_arc.read().await;
                if let Some(imm) = imms.get(imm_idx) {
                    imm.iter().collect()
                } else {
                    return;
                }
            };

            // Write SSTable synchronously inside the async task.
            let result: Result<()> = (|| {
                let mut writer = SstableWriter::new(&sst_path);
                for (key, record) in entries {
                    writer.add(key, record)?;
                }
                writer.finish()?;
                Ok(())
            })();

            if let Err(e) = result {
                // Log the error; we cannot surface it to the original caller here.
                eprintln!("[LsmTree] flush_memtable: SSTable write failed: {}", e);
                return;
            }

            // Remove the ImmutableMemTable now that the SSTable is on disk.
            let mut imms = immutables_arc.write().await;
            if imm_idx < imms.len() {
                imms.remove(imm_idx);
            }
        });

        // Check whether compaction should run.
        let sst_files = sorted_sst_files(&self.sstable_dir).unwrap_or_default();
        if self.compaction_policy.should_compact(&sst_files) {
            self.trigger_compaction().await?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Compaction
    // -----------------------------------------------------------------------

    /// Run one compaction cycle according to `compaction_policy`.
    ///
    /// 1. Lists all SSTable files, sorted oldest-first.
    /// 2. Asks the [`CompactionPolicy`] which files to merge.
    /// 3. Merges them into a new SSTable file.
    /// 4. Deletes the old files.
    pub async fn trigger_compaction(&self) -> Result<()> {
        let sst_files = sorted_sst_files(&self.sstable_dir)?;
        let to_merge = self.compaction_policy.select_files(&sst_files);

        if to_merge.len() < 2 {
            // Nothing meaningful to merge.
            return Ok(());
        }

        // Open the selected SSTables.
        let mut readers: Vec<SstableReader> = Vec::with_capacity(to_merge.len());
        for path in &to_merge {
            let reader = SstableReader::open(path)
                .with_context(|| format!("compaction: failed to open SSTable: {}", path.display()))?;
            readers.push(reader);
        }

        // Write the merged SSTable.
        let ts_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let merged_path = self.sstable_dir.join(format!("{:030}.sst", ts_nanos));

        merge_sstables(readers, &merged_path)
            .context("compaction: merge_sstables failed")?;

        // Delete the old files.
        for path in &to_merge {
            if let Err(e) = fs::remove_file(path) {
                eprintln!(
                    "[LsmTree] compaction: failed to delete old SSTable {}: {}",
                    path.display(),
                    e
                );
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a tombstone `FunRecord` — an empty record with `_confidence = 0.0`
/// that signals a logical deletion.
fn tombstone_record(collection: &str) -> FunRecord {
    FunRecordBuilder::new(collection).confidence(0.0).build()
}

/// Return `None` if `record` is a tombstone (confidence == 0.0), otherwise
/// wrap it in `Some`.
fn filter_tombstone(record: FunRecord) -> Option<FunRecord> {
    if is_tombstone(&record) {
        None
    } else {
        Some(record)
    }
}

/// Returns `true` if this record is a tombstone (confidence == 0.0).
fn is_tombstone(record: &FunRecord) -> bool {
    record._confidence == 0.0
}

/// Return all `.sst` files in `dir`, sorted ascending by filename
/// (timestamp-based names → ascending = oldest first).
fn sorted_sst_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("failed to read directory: {}", dir.display())),
    };

    let mut files: Vec<PathBuf> = read_dir
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("sst") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // Sort ascending by filename so oldest (smallest timestamp) comes first.
    files.sort_by(|a, b| {
        a.file_name()
            .cmp(&b.file_name())
    });

    Ok(files)
}

/// Convert bytes to a lowercase hex string for use in cache keys.
fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_core::builder::FunRecordBuilder;
    use tempfile::TempDir;

    fn make_key(collection: &str, id: [u8; 16]) -> RecordKey {
        RecordKey {
            collection: collection.to_string(),
            id,
        }
    }

    fn make_record(collection: &str) -> FunRecord {
        FunRecordBuilder::new(collection).build()
    }

    // -----------------------------------------------------------------------
    // Test: open creates directory and returns Ok
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_open_creates_dir() {
        let tmp = TempDir::new().unwrap();
        let db_dir = tmp.path().join("db");
        assert!(!db_dir.exists());

        let tree = LsmTree::open(&db_dir, 1024 * 1024).unwrap();
        assert!(db_dir.exists());
        drop(tree);
    }

    // -----------------------------------------------------------------------
    // Test: write and get round-trip
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_write_and_get() {
        let tmp = TempDir::new().unwrap();
        let tree = LsmTree::open(tmp.path(), 64 * 1024 * 1024).unwrap();

        let key = make_key("users", [1u8; 16]);
        let record = make_record("users");
        let record_id = record._id;

        tree.write(key.clone(), record).await.unwrap();

        let found = tree.get(&key).await.unwrap();
        assert!(found.is_some(), "record must be found after write");
        assert_eq!(found.unwrap()._id, record_id);
    }

    // -----------------------------------------------------------------------
    // Test: delete makes the key invisible
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_delete_makes_key_invisible() {
        let tmp = TempDir::new().unwrap();
        let tree = LsmTree::open(tmp.path(), 64 * 1024 * 1024).unwrap();

        let key = make_key("users", [2u8; 16]);
        tree.write(key.clone(), make_record("users")).await.unwrap();
        tree.delete(key.clone()).await.unwrap();

        let found = tree.get(&key).await.unwrap();
        assert!(found.is_none(), "record must be invisible after delete");
    }

    // -----------------------------------------------------------------------
    // Test: get returns None for a key that was never written
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_missing_key() {
        let tmp = TempDir::new().unwrap();
        let tree = LsmTree::open(tmp.path(), 64 * 1024 * 1024).unwrap();

        let key = make_key("docs", [99u8; 16]);
        let found = tree.get(&key).await.unwrap();
        assert!(found.is_none());
    }

    // -----------------------------------------------------------------------
    // Test: scan returns sorted, non-tombstone results
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_scan_basic() {
        let tmp = TempDir::new().unwrap();
        let tree = LsmTree::open(tmp.path(), 64 * 1024 * 1024).unwrap();

        for i in 0u8..10 {
            let key = make_key("items", [i; 16]);
            tree.write(key, make_record("items")).await.unwrap();
        }

        // Delete key 5.
        tree.delete(make_key("items", [5u8; 16])).await.unwrap();

        let from = make_key("items", [0u8; 16]);
        let to   = make_key("items", [9u8; 16]);
        let results = tree.scan("items", &from, &to).await.unwrap();

        // 10 written, 1 deleted → 9 visible.
        assert_eq!(results.len(), 9, "expected 9 non-deleted records");

        // Verify sort order.
        for w in results.windows(2) {
            assert!(w[0].0 <= w[1].0, "results must be sorted");
        }

        // Key 5 must not appear.
        let key5 = make_key("items", [5u8; 16]);
        assert!(
            results.iter().all(|(k, _)| k != &key5),
            "deleted key must not appear in scan"
        );
    }

    // -----------------------------------------------------------------------
    // Test: WAL replay restores data after re-open
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_wal_replay_on_reopen() {
        let tmp = TempDir::new().unwrap();
        let key  = make_key("recover", [42u8; 16]);
        let record_id;

        {
            let tree   = LsmTree::open(tmp.path(), 64 * 1024 * 1024).unwrap();
            let record = make_record("recover");
            record_id  = record._id;
            tree.write(key.clone(), record).await.unwrap();
            // Drop without explicit flush — data lives in WAL.
        }

        // Re-open; WAL replay should restore the record.
        let tree2 = LsmTree::open(tmp.path(), 64 * 1024 * 1024).unwrap();
        let found = tree2.get(&key).await.unwrap();
        assert!(found.is_some(), "record must survive process restart via WAL replay");
        assert_eq!(found.unwrap()._id, record_id);
    }

    // -----------------------------------------------------------------------
    // Test: flush_memtable produces an SSTable file
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_flush_produces_sst_file() {
        let tmp = TempDir::new().unwrap();
        let tree = LsmTree::open(tmp.path(), 64 * 1024 * 1024).unwrap();

        for i in 0u8..5 {
            let key = make_key("flush", [i; 16]);
            tree.write(key, make_record("flush")).await.unwrap();
        }

        tree.flush_memtable().await.unwrap();

        // Give the background task a moment to write the file.
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let files = sorted_sst_files(tmp.path()).unwrap();
        assert!(!files.is_empty(), "at least one .sst file must exist after flush");
    }
}
