//! Write-Ahead Log (WAL) — STORY-2-2
//!
//! Every write is appended to a sequential, framed MessagePack log before the
//! caller receives an acknowledgement, guaranteeing no committed data is lost
//! on process crash.
//!
//! # Frame format
//!
//! ```text
//! [4 bytes: entry_len as u32 little-endian]
//! [entry_len bytes: MessagePack-encoded WalEntry]
//! [4 bytes: xxh3_64(body) as u32, little-endian]
//! ```
//!
//! Frames are always appended; existing bytes are never overwritten.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read as IoRead, Write as IoWrite};
use std::path::Path;

use anyhow::{Context, Result};
use fundb_core::{FunRecord, RecordKey};
use xxhash_rust::xxh3::xxh3_64;

use crate::Lsn;

// ---------------------------------------------------------------------------
// WalEntry
// ---------------------------------------------------------------------------

/// A single operation recorded in the WAL.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum WalEntry {
    /// A record was written (inserted or updated) within a transaction.
    Write {
        txn_id: u64,
        key: RecordKey,
        record: FunRecord,
    },
    /// A record was deleted within a transaction.
    Delete { txn_id: u64, key: RecordKey },
    /// The transaction with the given ID committed successfully.
    TxnCommit(u64),
    /// The transaction with the given ID was aborted.
    TxnAbort(u64),
    /// A durable checkpoint has been established at the given LSN.
    Checkpoint(Lsn),
}

// ---------------------------------------------------------------------------
// Wal
// ---------------------------------------------------------------------------

/// Write-Ahead Log handle.
///
/// Holds an append-only file handle and tracks the current LSN.  The LSN
/// starts at 0 (no entries written) and increments by 1 for each `append`
/// call, so the first entry returns LSN 1.
pub struct Wal {
    /// Path to the WAL file on disk.
    _path: std::path::PathBuf,
    /// Append-only file handle.
    file: File,
    /// LSN of the most recently written entry (0 = empty log).
    current_lsn: Lsn,
}

impl Wal {
    /// Open (or create) the WAL file at `path`.
    ///
    /// If the file already exists its existing content is preserved; new entries
    /// will be appended after the last byte.  The `current_lsn` is recovered by
    /// replaying the existing frames so subsequent `append` calls continue the
    /// sequence correctly.
    pub fn open(path: &Path) -> Result<Self> {
        // Count existing valid frames to initialise the LSN counter.
        let current_lsn = Self::count_valid_entries(path)?;

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("failed to open WAL at {}", path.display()))?;

        Ok(Self {
            _path: path.to_path_buf(),
            file,
            current_lsn,
        })
    }

    /// Append a [`WalEntry`] to the log, returning the new [`Lsn`].
    ///
    /// For [`WalEntry::TxnCommit`] entries an `fsync` is issued after the
    /// write to guarantee durability before the caller is notified of success.
    pub fn append(&mut self, entry: WalEntry) -> Result<Lsn> {
        let is_commit = matches!(entry, WalEntry::TxnCommit(_));

        // Serialize to MessagePack.
        let body =
            rmp_serde::to_vec(&entry).context("failed to serialize WalEntry to MessagePack")?;

        // Compute checksum over the serialized body.
        let checksum = xxh3_64(&body) as u32;

        // Write frame: [len: u32 LE] [body] [crc: u32 LE]
        let len = body.len() as u32;
        self.file
            .write_all(&len.to_le_bytes())
            .context("WAL write: failed to write entry length")?;
        self.file
            .write_all(&body)
            .context("WAL write: failed to write entry body")?;
        self.file
            .write_all(&checksum.to_le_bytes())
            .context("WAL write: failed to write checksum")?;

        // Only fsync on commit for durability; other entries are buffered.
        if is_commit {
            self.file
                .sync_data()
                .context("WAL fsync failed on TxnCommit")?;
        }

        self.current_lsn += 1;
        Ok(self.current_lsn)
    }

    /// Return an iterator that reads and validates every frame in the WAL file.
    ///
    /// Iteration stops at the first corrupted or truncated frame (CRC mismatch
    /// or short read).  Entries before the corruption point are yielded as
    /// `Ok(WalEntry)`; no `Err` items are produced for the corruption itself —
    /// the iterator simply terminates.
    pub fn recover(path: &Path) -> Result<impl Iterator<Item = Result<WalEntry>>> {
        let file = File::open(path)
            .with_context(|| format!("failed to open WAL for recovery at {}", path.display()))?;
        Ok(WalRecoveryIter::new(BufReader::new(file)))
    }

    /// Record a checkpoint at `lsn` by appending a [`WalEntry::Checkpoint`].
    ///
    /// The caller is responsible for any log truncation needed after the
    /// checkpoint is established; this method only writes the marker entry.
    pub fn checkpoint(&mut self, lsn: Lsn) -> Result<()> {
        self.append(WalEntry::Checkpoint(lsn))?;
        Ok(())
    }

    /// Return the LSN of the most recently written entry (0 if the log is empty).
    pub fn current_lsn(&self) -> Lsn {
        self.current_lsn
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Count the number of valid (non-corrupted) frames in the file at `path`.
    ///
    /// Used during `open` to recover the LSN counter without holding the file
    /// open in read mode.
    fn count_valid_entries(path: &Path) -> Result<Lsn> {
        // If the file does not yet exist the LSN starts at 0.
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e).context("failed to open WAL for LSN counting"),
        };

        let mut reader = BufReader::new(file);
        let mut count: Lsn = 0;

        loop {
            match read_frame(&mut reader) {
                Ok(Some(_)) => count += 1,
                Ok(None) => break, // clean EOF
                Err(_) => break,   // corruption — stop counting
            }
        }

        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Frame I/O helpers
// ---------------------------------------------------------------------------

/// Read one frame from `reader`.
///
/// Returns:
/// - `Ok(Some(entry))` — a valid frame was read and deserialized.
/// - `Ok(None)`        — clean EOF at frame boundary.
/// - `Err(_)`          — short read, CRC mismatch, or deserialization error.
fn read_frame<R: IoRead>(reader: &mut R) -> Result<Option<WalEntry>> {
    // --- Read the 4-byte length prefix ---
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            // Check if we consumed 0 bytes (true clean EOF) or partial bytes
            // (corruption). We treat both as "stop" by returning Ok(None).
            return Ok(None);
        }
        Err(e) => return Err(e).context("WAL: failed to read frame length"),
    }
    let entry_len = u32::from_le_bytes(len_buf) as usize;

    // --- Read the body ---
    let mut body = vec![0u8; entry_len];
    reader.read_exact(&mut body).map_err(|_| {
        anyhow::anyhow!(
            "WAL: short read on frame body (expected {} bytes)",
            entry_len
        )
    })?;

    // --- Read the 4-byte CRC ---
    let mut crc_buf = [0u8; 4];
    reader
        .read_exact(&mut crc_buf)
        .map_err(|_| anyhow::anyhow!("WAL: short read on CRC field"))?;
    let stored_crc = u32::from_le_bytes(crc_buf);

    // --- Verify CRC ---
    let computed_crc = xxh3_64(&body) as u32;
    if computed_crc != stored_crc {
        return Err(anyhow::anyhow!(
            "WAL: CRC mismatch (stored={:#010x}, computed={:#010x})",
            stored_crc,
            computed_crc
        ));
    }

    // --- Deserialize ---
    let entry: WalEntry = rmp_serde::from_slice(&body)
        .context("WAL: failed to deserialize WalEntry from MessagePack")?;

    Ok(Some(entry))
}

// ---------------------------------------------------------------------------
// Recovery iterator
// ---------------------------------------------------------------------------

/// Lazy frame-by-frame iterator over a WAL file.
struct WalRecoveryIter<R: IoRead> {
    reader: R,
    done: bool,
}

impl<R: IoRead> WalRecoveryIter<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            done: false,
        }
    }
}

impl<R: IoRead> Iterator for WalRecoveryIter<R> {
    type Item = Result<WalEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match read_frame(&mut self.reader) {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(_) => {
                // Corruption: stop iteration, surface nothing more.
                self.done = true;
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_core::builder::FunRecordBuilder;
    use tempfile::TempDir;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_key(collection: &str) -> RecordKey {
        RecordKey {
            collection: collection.to_string(),
            id: [0u8; 16],
        }
    }

    fn make_record(collection: &str) -> FunRecord {
        FunRecordBuilder::new(collection).build()
    }

    fn make_write_entry(txn_id: u64, collection: &str) -> WalEntry {
        WalEntry::Write {
            txn_id,
            key: make_key(collection),
            record: make_record(collection),
        }
    }

    // -----------------------------------------------------------------------
    // test_append_and_recover
    // -----------------------------------------------------------------------

    /// Write 5 Write entries + 1 TxnCommit; recover and verify 6 entries in order.
    #[test]
    fn test_append_and_recover() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("test.wal");

        {
            let mut wal = Wal::open(&wal_path).unwrap();
            for i in 0..5 {
                wal.append(make_write_entry(1, &format!("col_{}", i)))
                    .unwrap();
            }
            wal.append(WalEntry::TxnCommit(1)).unwrap();
        }

        let recovered: Vec<_> = Wal::recover(&wal_path)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(recovered.len(), 6, "expected 6 recovered entries");

        // First 5 should be Write entries.
        for (i, entry) in recovered[..5].iter().enumerate() {
            match entry {
                WalEntry::Write { txn_id, key, .. } => {
                    assert_eq!(*txn_id, 1);
                    assert_eq!(key.collection, format!("col_{}", i));
                }
                other => panic!("expected WalEntry::Write at index {}, got {:?}", i, other),
            }
        }

        // Last entry should be TxnCommit(1).
        assert!(
            matches!(recovered[5], WalEntry::TxnCommit(1)),
            "expected TxnCommit(1) as last entry"
        );
    }

    // -----------------------------------------------------------------------
    // test_partial_write_recovery
    // -----------------------------------------------------------------------

    /// Write 3 valid entries, then append 3 garbage bytes; recover should yield
    /// exactly 3 entries and not panic.
    #[test]
    fn test_partial_write_recovery() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("partial.wal");

        {
            let mut wal = Wal::open(&wal_path).unwrap();
            for i in 0..3 {
                wal.append(make_write_entry(2, &format!("col_{}", i)))
                    .unwrap();
            }
        }

        // Corrupt the tail of the file by appending 3 garbage bytes.
        {
            let mut f = OpenOptions::new().append(true).open(&wal_path).unwrap();
            f.write_all(&[0xDE, 0xAD, 0xBE]).unwrap();
        }

        let recovered: Vec<_> = Wal::recover(&wal_path).unwrap().collect();

        // All recovered items must be Ok.
        for item in &recovered {
            assert!(item.is_ok(), "unexpected Err in recovered entries");
        }

        assert_eq!(
            recovered.len(),
            3,
            "expected exactly 3 valid entries before the corruption"
        );
    }

    // -----------------------------------------------------------------------
    // test_lsn_increments
    // -----------------------------------------------------------------------

    /// Write 10 entries and verify the returned LSNs are 1..=10.
    #[test]
    fn test_lsn_increments() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("lsn.wal");

        let mut wal = Wal::open(&wal_path).unwrap();
        let lsns: Vec<Lsn> = (0..10)
            .map(|i| {
                wal.append(make_write_entry(3, &format!("col_{}", i)))
                    .unwrap()
            })
            .collect();

        assert_eq!(
            lsns,
            (1u64..=10).collect::<Vec<_>>(),
            "LSNs must be 1..=10 in order"
        );
        assert_eq!(wal.current_lsn(), 10);
    }

    // -----------------------------------------------------------------------
    // test_checkpoint_entry
    // -----------------------------------------------------------------------

    /// Call checkpoint(5), recover, and verify the last entry is Checkpoint(5).
    #[test]
    fn test_checkpoint_entry() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("checkpoint.wal");

        {
            let mut wal = Wal::open(&wal_path).unwrap();
            wal.append(make_write_entry(4, "col_a")).unwrap();
            wal.checkpoint(5).unwrap();
        }

        let recovered: Vec<_> = Wal::recover(&wal_path)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(recovered.len(), 2);
        assert!(
            matches!(recovered[1], WalEntry::Checkpoint(5)),
            "last entry must be Checkpoint(5), got {:?}",
            recovered[1]
        );
    }

    // -----------------------------------------------------------------------
    // test_delete_entry
    // -----------------------------------------------------------------------

    /// Write a Delete entry, recover it, and verify it round-trips correctly.
    #[test]
    fn test_delete_entry() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("delete.wal");

        let txn_id = 99u64;
        let key = RecordKey {
            collection: "docs".to_string(),
            id: [7u8; 16],
        };

        {
            let mut wal = Wal::open(&wal_path).unwrap();
            wal.append(WalEntry::Delete {
                txn_id,
                key: key.clone(),
            })
            .unwrap();
        }

        let recovered: Vec<_> = Wal::recover(&wal_path)
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(recovered.len(), 1);
        match &recovered[0] {
            WalEntry::Delete { txn_id: t, key: k } => {
                assert_eq!(*t, txn_id);
                assert_eq!(k.collection, "docs");
                assert_eq!(k.id, [7u8; 16]);
            }
            other => panic!("expected WalEntry::Delete, got {:?}", other),
        }
    }
}
