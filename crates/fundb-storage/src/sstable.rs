use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use fundb_core::page::BloomFilter;
use fundb_core::record::FunRecord;
use fundb_core::types::RecordKey;

// ---------------------------------------------------------------------------
// File format constants
// ---------------------------------------------------------------------------

/// Magic bytes at the start of every SSTable file: "FUNDBSST" as 8 ASCII bytes.
const MAGIC: &[u8; 8] = b"FUNDBSST";

// ---------------------------------------------------------------------------
// Helper: serialise / deserialise a length-prefixed msgpack value
// ---------------------------------------------------------------------------

/// Write a value as a 4-byte LE length-prefixed MessagePack blob.
fn write_framed<W: Write, T: serde::Serialize>(writer: &mut W, value: &T) -> Result<()> {
    let bytes = rmp_serde::to_vec(value)
        .map_err(|e| anyhow!("msgpack serialise error: {}", e))?;
    let len = bytes.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&bytes)?;
    Ok(())
}

/// Read a 4-byte LE length-prefixed MessagePack blob and deserialise it.
fn read_framed<R: Read, T: serde::de::DeserializeOwned>(reader: &mut R) -> Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    rmp_serde::from_slice(&buf).map_err(|e| anyhow!("msgpack deserialise error: {}", e))
}

// ---------------------------------------------------------------------------
// SstableWriter
// ---------------------------------------------------------------------------

/// Buffers entries in memory and writes an SSTable file on [`finish`].
///
/// Entries are expected to be added in ascending key order; however
/// [`finish`] performs a sort before writing so out-of-order additions are
/// handled safely.
pub struct SstableWriter {
    path: PathBuf,
    entries: Vec<(RecordKey, FunRecord)>,
}

impl SstableWriter {
    /// Create a new writer that will eventually write to `path`.
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            entries: Vec::new(),
        }
    }

    /// Buffer an entry.  Entries should be provided in ascending key order but
    /// the writer will sort them on [`finish`] to be safe.
    pub fn add(&mut self, key: RecordKey, record: FunRecord) -> Result<()> {
        self.entries.push((key, record));
        Ok(())
    }

    /// Sort the buffered entries, write the SSTable file, and return a
    /// [`SstableReader`] opened on the same path.
    ///
    /// File format:
    /// ```text
    /// [8 bytes]  magic "FUNDBSST"
    /// [4 bytes]  u32 LE entry_count
    /// For each entry:
    ///   [4 bytes]  u32 LE key_len
    ///   [key_len]  rmp_serde(RecordKey)
    ///   [4 bytes]  u32 LE record_len
    ///   [record_len] rmp_serde(FunRecord)
    /// [4 bytes]  u32 LE bloom_len
    /// [bloom_len]  rmp_serde bloom filter bits + k
    /// ```
    pub fn finish(mut self) -> Result<SstableReader> {
        // Sort by key so the on-disk order is deterministic.
        self.entries.sort_by(|a, b| a.0.cmp(&b.0));

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
        let mut writer = BufWriter::new(file);

        // --- Magic ---
        writer.write_all(MAGIC)?;

        // --- Entry count ---
        let entry_count = self.entries.len() as u32;
        writer.write_all(&entry_count.to_le_bytes())?;

        // --- Entries ---
        for (key, record) in &self.entries {
            write_framed(&mut writer, key)?;
            write_framed(&mut writer, record)?;
        }

        // --- Bloom filter ---
        // Build and serialise the bloom filter so it is stored at the end of
        // the file.  We insert the msgpack bytes of each key.
        let capacity = self.entries.len().max(1);
        let mut bloom = BloomFilter::new(capacity, 0.001);
        for (key, _) in &self.entries {
            let key_bytes = rmp_serde::to_vec(key).unwrap_or_default();
            bloom.insert(&key_bytes);
        }

        // Serialise the bloom filter as (bits: Vec<u64>, k: u8).
        let bloom_payload: (Vec<u64>, u8) = (bloom.bits.clone(), bloom.k);
        write_framed(&mut writer, &bloom_payload)?;

        writer.flush()?;

        SstableReader::open(&self.path)
    }
}

// ---------------------------------------------------------------------------
// SstableReader
// ---------------------------------------------------------------------------

/// Reads an SSTable file, keeping all entries and the bloom filter in memory.
pub struct SstableReader {
    path: PathBuf,
    entries: Vec<(RecordKey, FunRecord)>,
    bloom: BloomFilter,
}

impl SstableReader {
    /// Open an existing SSTable file and load all entries into memory.
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;

        // --- Magic ---
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(anyhow!(
                "invalid SSTable magic in {:?}: expected FUNDBSST",
                path
            ));
        }

        // --- Entry count ---
        let mut count_buf = [0u8; 4];
        file.read_exact(&mut count_buf)?;
        let entry_count = u32::from_le_bytes(count_buf) as usize;

        // --- Entries ---
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let key: RecordKey = read_framed(&mut file)?;
            let record: FunRecord = read_framed(&mut file)?;
            entries.push((key, record));
        }

        // --- Bloom filter ---
        let bloom_payload: (Vec<u64>, u8) = read_framed(&mut file)?;
        let bloom = BloomFilter {
            bits: bloom_payload.0,
            k: bloom_payload.1,
        };

        Ok(Self {
            path: path.to_path_buf(),
            entries,
            bloom,
        })
    }

    /// Perform a point lookup.
    ///
    /// First checks the bloom filter; if the key is definitely absent returns
    /// `Ok(None)` without touching the entries vec.  Otherwise performs a
    /// binary search on the sorted entries.
    pub fn get(&self, key: &RecordKey) -> Result<Option<FunRecord>> {
        if !self.may_contain(key) {
            return Ok(None);
        }

        match self.entries.binary_search_by(|(k, _)| k.cmp(key)) {
            Ok(idx) => Ok(Some(self.entries[idx].1.clone())),
            Err(_) => Ok(None),
        }
    }

    /// Return an iterator over all entries whose key satisfies
    /// `from <= key <= to` (inclusive on both ends).
    pub fn range<'a>(
        &'a self,
        from: &'a RecordKey,
        to: &'a RecordKey,
    ) -> impl Iterator<Item = Result<(RecordKey, FunRecord)>> + 'a {
        // Binary search for the start of the range.
        let start = self
            .entries
            .partition_point(|(k, _)| k < from);

        self.entries[start..]
            .iter()
            .take_while(move |(k, _)| k <= to)
            .map(|(k, v)| Ok((k.clone(), v.clone())))
    }

    /// Returns `true` if the bloom filter reports that `key` *may* be present
    /// in this SSTable.  A `false` return is a definitive "not present".
    pub fn may_contain(&self, key: &RecordKey) -> bool {
        let key_bytes = rmp_serde::to_vec(key).unwrap_or_default();
        self.bloom.contains(&key_bytes)
    }

    /// Number of entries in this SSTable.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// merge_sstables
// ---------------------------------------------------------------------------

/// K-way merge of multiple SSTables into a single output SSTable.
///
/// All entries from all `inputs` are collected, sorted by key, then
/// deduplicated keeping the **last** value seen for each key.  Inputs should
/// be provided in oldest-to-newest order so that the newest version of each
/// key survives deduplication.
pub fn merge_sstables(inputs: Vec<SstableReader>, output: &Path) -> Result<SstableReader> {
    // Collect all entries from all inputs.  Later entries (from later inputs)
    // win during deduplication because we reverse-iterate after sort.
    let mut all: Vec<(RecordKey, FunRecord)> = inputs
        .into_iter()
        .flat_map(|r| r.entries.into_iter())
        .collect();

    // Stable-sort by key so that equal keys from later inputs end up *after*
    // those from earlier inputs.
    all.sort_by(|a, b| a.0.cmp(&b.0));

    // Deduplicate: keep only the last occurrence of each key (newest wins).
    // We iterate from the back and keep an entry only if the *next* entry has
    // a different key.
    let mut merged: Vec<(RecordKey, FunRecord)> = Vec::with_capacity(all.len());
    let mut prev_key: Option<RecordKey> = None;

    for (key, record) in all.into_iter().rev() {
        // If this key is the same as the one we already kept, skip (the later
        // one was already pushed).
        if prev_key.as_ref() == Some(&key) {
            continue;
        }
        prev_key = Some(key.clone());
        merged.push((key, record));
    }

    // Reverse so entries are back in ascending key order.
    merged.reverse();

    // Write the merged data to the output SSTable.
    let mut writer = SstableWriter::new(output);
    for (key, record) in merged {
        writer.add(key, record)?;
    }
    writer.finish()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_core::builder::FunRecordBuilder;
    use tempfile::TempDir;
    use uuid::Uuid;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_key(collection: &str, id: [u8; 16]) -> RecordKey {
        RecordKey {
            collection: collection.to_string(),
            id,
        }
    }

    fn make_record(collection: &str) -> FunRecord {
        FunRecordBuilder::new(collection).build()
    }

    /// Create an SSTable from the given (key, record) pairs in the temp dir.
    fn write_sstable(
        dir: &TempDir,
        name: &str,
        entries: Vec<(RecordKey, FunRecord)>,
    ) -> SstableReader {
        let path = dir.path().join(name);
        let mut writer = SstableWriter::new(&path);
        for (k, v) in entries {
            writer.add(k, v).unwrap();
        }
        writer.finish().unwrap()
    }

    // -----------------------------------------------------------------------
    // test_write_read_100
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_read_100() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sst");

        let mut writer = SstableWriter::new(&path);
        let mut expected: Vec<(RecordKey, FunRecord)> = Vec::new();

        for _ in 0..100 {
            let id = Uuid::new_v4();
            let key = RecordKey {
                collection: "test".to_string(),
                id: *id.as_bytes(),
            };
            let record = FunRecordBuilder::new("test").build();
            expected.push((key.clone(), record.clone()));
            writer.add(key, record).unwrap();
        }

        let reader = writer.finish().unwrap();

        assert_eq!(reader.len(), 100, "must have 100 entries");

        // Sort expected by key (same as what the writer does internally).
        expected.sort_by(|a, b| a.0.cmp(&b.0));

        for (key, orig_record) in &expected {
            let found = reader.get(key).unwrap();
            assert!(
                found.is_some(),
                "key {:?} must be present in the SSTable",
                key
            );
            assert_eq!(
                found.unwrap()._id,
                orig_record._id,
                "record ID mismatch for key {:?}",
                key
            );
        }
    }

    // -----------------------------------------------------------------------
    // test_bloom_false_positive_rate
    // -----------------------------------------------------------------------

    #[test]
    fn test_bloom_false_positive_rate() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bloom_test.sst");

        let n = 10_000usize;
        let mut writer = SstableWriter::new(&path);
        let mut inserted_keys: Vec<RecordKey> = Vec::with_capacity(n);

        for _ in 0..n {
            let id = Uuid::new_v4();
            let key = RecordKey {
                collection: "bloom".to_string(),
                id: *id.as_bytes(),
            };
            let record = FunRecordBuilder::new("bloom").build();
            inserted_keys.push(key.clone());
            writer.add(key, record).unwrap();
        }

        let reader = writer.finish().unwrap();

        // Verify there are no false negatives before checking the FPR.
        // (inserted_keys is used here to suppress any dead_code lint)
        for key in &inserted_keys {
            assert!(
                reader.may_contain(key),
                "bloom filter must not produce a false negative"
            );
        }

        // Generate 10_000 non-inserted keys and count false positives.
        let mut false_positives = 0usize;
        for _ in 0..n {
            let id = Uuid::new_v4();
            let key = RecordKey {
                collection: "bloom_absent".to_string(),
                id: *id.as_bytes(),
            };
            if reader.may_contain(&key) {
                false_positives += 1;
            }
        }

        let fpr = false_positives as f64 / n as f64;
        assert!(
            fpr < 0.01,
            "false positive rate {:.4} exceeds 1% ({} / {})",
            fpr,
            false_positives,
            n
        );
    }

    // -----------------------------------------------------------------------
    // test_range_query
    // -----------------------------------------------------------------------

    #[test]
    fn test_range_query() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("range_test.sst");

        let n = 50usize;
        let mut writer = SstableWriter::new(&path);

        // Generate 50 keys deterministically so we can sort them and pick
        // known boundary indices.
        let mut keys: Vec<RecordKey> = (0..n)
            .map(|_| RecordKey {
                collection: "range".to_string(),
                id: *Uuid::new_v4().as_bytes(),
            })
            .collect();
        keys.sort(); // sort so indices 10 and 40 are well-defined

        for key in &keys {
            let record = FunRecordBuilder::new("range").build();
            writer.add(key.clone(), record).unwrap();
        }

        let reader = writer.finish().unwrap();

        // range(keys[10], keys[40]) should return entries at indices 10..=40 → 31 entries.
        let from = &keys[10];
        let to = &keys[40];

        let results: Vec<_> = reader
            .range(from, to)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            results.len(),
            31,
            "range(key[10], key[40]) must return 31 entries, got {}",
            results.len()
        );

        // Verify the boundaries are inclusive.
        assert_eq!(&results[0].0, from);
        assert_eq!(&results[30].0, to);
    }

    // -----------------------------------------------------------------------
    // test_merge_sstables
    // -----------------------------------------------------------------------

    #[test]
    fn test_merge_sstables() {
        let dir = TempDir::new().unwrap();

        // Create 3 SSTables with 10 non-overlapping records each.
        // We use byte 0 of the ID to ensure keys are in distinct ranges.
        let mut sst_readers = Vec::new();
        for sst_idx in 0usize..3 {
            let mut entries = Vec::new();
            for rec_idx in 0u8..10 {
                let mut id = [0u8; 16];
                id[0] = sst_idx as u8;
                id[1] = rec_idx;
                let key = make_key("merge", id);
                let record = FunRecordBuilder::new("merge").build();
                entries.push((key, record));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let reader = write_sstable(&dir, &format!("sst_{}.sst", sst_idx), entries);
            sst_readers.push(reader);
        }

        let merged_path = dir.path().join("merged.sst");
        let merged = merge_sstables(sst_readers, &merged_path).unwrap();

        assert_eq!(
            merged.len(),
            30,
            "merged SSTable must have 30 entries, got {}",
            merged.len()
        );

        // Verify sorted order.
        let keys: Vec<&RecordKey> = merged.entries.iter().map(|(k, _)| k).collect();
        for i in 1..keys.len() {
            assert!(
                keys[i - 1] <= keys[i],
                "merged SSTable is not sorted at index {}",
                i
            );
        }
    }

    // -----------------------------------------------------------------------
    // test_tombstone_handling
    // -----------------------------------------------------------------------

    /// Write record with key K to SSTable-1.
    /// Write a delete marker (empty record, confidence 0.0) to SSTable-2.
    /// Merge with SSTable-2 last.
    /// Verify the merged SSTable contains the SSTable-2 version (the tombstone).
    #[test]
    fn test_tombstone_handling() {
        let dir = TempDir::new().unwrap();

        let key_id = *Uuid::new_v4().as_bytes();
        let key = make_key("tomb", key_id);

        // SSTable-1: the original record.
        let original = FunRecordBuilder::new("tomb").confidence(0.9).build();
        let sst1 = write_sstable(&dir, "sst1.sst", vec![(key.clone(), original.clone())]);

        // SSTable-2: the tombstone / delete marker — empty data, confidence 0.0.
        let tombstone = FunRecordBuilder::new("tomb").confidence(0.0).build();
        let sst2 = write_sstable(&dir, "sst2.sst", vec![(key.clone(), tombstone.clone())]);

        // Merge: sst1 first (oldest), sst2 last (newest) → sst2 version must win.
        let merged_path = dir.path().join("merged_tomb.sst");
        let merged = merge_sstables(vec![sst1, sst2], &merged_path).unwrap();

        assert_eq!(merged.len(), 1, "merged SSTable must have exactly 1 entry");

        let result = merged.get(&key).unwrap().expect("key must be present");
        assert!(
            (result._confidence - 0.0).abs() < 1e-6,
            "merged record must be the SSTable-2 tombstone (confidence 0.0), got {}",
            result._confidence
        );
        assert_eq!(
            result._id, tombstone._id,
            "merged record ID must match the SSTable-2 tombstone"
        );
    }

    // -----------------------------------------------------------------------
    // test_roundtrip_empty_sstable
    // -----------------------------------------------------------------------

    #[test]
    fn test_roundtrip_empty_sstable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.sst");

        let writer = SstableWriter::new(&path);
        let reader = writer.finish().unwrap();

        assert_eq!(reader.len(), 0);

        // get on an empty SSTable must return None.
        let key = make_key("empty", [0u8; 16]);
        assert!(reader.get(&key).unwrap().is_none());

        // range on an empty SSTable must produce no items.
        let results: Vec<_> = reader
            .range(&key, &key)
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert!(results.is_empty());
    }

    // -----------------------------------------------------------------------
    // test_may_contain_no_false_negatives
    // -----------------------------------------------------------------------

    #[test]
    fn test_may_contain_no_false_negatives() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fn_test.sst");

        let mut writer = SstableWriter::new(&path);
        let mut keys = Vec::new();

        for _ in 0..200 {
            let id = *Uuid::new_v4().as_bytes();
            let key = make_key("fn", id);
            let record = make_record("fn");
            keys.push(key.clone());
            writer.add(key, record).unwrap();
        }

        let reader = writer.finish().unwrap();

        // Bloom filter must return true for every inserted key (no false negatives).
        for key in &keys {
            assert!(
                reader.may_contain(key),
                "bloom filter must not produce false negatives for {:?}",
                key
            );
        }
    }
}
