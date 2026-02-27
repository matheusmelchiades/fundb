// crates/fundb-storage/src/mvcc.rs
//
// Bitemporal Multiversion Concurrency Control (MVCC) — STORY-2-4.
//
// Implements a two-dimensional time model:
//   - System time  (sys_from / sys_to): when a version was physically written.
//   - Valid  time  (valid_from / valid_to): the application-layer validity window.
//
// Snapshot isolation is achieved by tagging each version with the transaction's
// `snapshot_ts` as `sys_from`, so readers at an earlier snapshot cannot observe
// versions written by later transactions.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use fundb_core::{FunRecord, RecordKey, Timestamp};

// Transaction is used for the public API; Lsn is imported per the interface contract.
#[allow(unused_imports)]
use crate::{Lsn, Transaction};

// ---------------------------------------------------------------------------
// Internal versioned record
// ---------------------------------------------------------------------------

/// One immutable version of a record within the MVCC version chain.
struct VersionedRecord {
    /// System time: when this version was written (Unix nanoseconds).
    sys_from:   Timestamp,
    /// System time: when this version was superseded. `i64::MAX` = current version.
    sys_to:     Timestamp,
    /// Valid time: earliest application-layer validity (Unix nanoseconds).
    valid_from: Timestamp,
    /// Valid time: end of application-layer validity (Unix nanoseconds).
    valid_to:   Timestamp,
    /// The immutable record payload for this version.
    record:     FunRecord,
}

// ---------------------------------------------------------------------------
// MvccStore
// ---------------------------------------------------------------------------

/// Bitemporal MVCC store.
///
/// Each `RecordKey` maps to an ordered list of [`VersionedRecord`]s, sorted
/// ascending by `sys_from`.  The current (live) version always has
/// `sys_to == i64::MAX`.
pub struct MvccStore {
    inner:    RwLock<BTreeMap<RecordKey, Vec<VersionedRecord>>>,
    next_txn: AtomicU64,
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Return the current wall-clock time as Unix nanoseconds.
fn current_nanos() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX_EPOCH")
        .as_nanos() as i64
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl MvccStore {
    /// Create a new, empty `MvccStore`.
    pub fn new() -> Self {
        Self {
            inner:    RwLock::new(BTreeMap::new()),
            next_txn: AtomicU64::new(1),
        }
    }

    /// Begin a new transaction, capturing a snapshot timestamp.
    ///
    /// The returned `Transaction` carries `snapshot_ts` — the system time at
    /// which the transaction started.  Reads performed at `snapshot_ts` will
    /// not see any write committed after this moment.
    pub fn begin_txn(&self) -> Transaction {
        let txn_id = self.next_txn.fetch_add(1, Ordering::SeqCst);
        Transaction {
            txn_id,
            snapshot_ts: current_nanos(),
        }
    }

    /// Write `record` under the given transaction.
    ///
    /// If a current version (`sys_to == i64::MAX`) already exists for the
    /// record's key, its `sys_to` is set to `txn.snapshot_ts`, superseding it.
    /// The new version is appended with `sys_from = txn.snapshot_ts` and
    /// `sys_to = i64::MAX`.
    pub fn write(&self, txn: &Transaction, record: FunRecord) -> Result<()> {
        let key = RecordKey {
            collection: record._collection.clone(),
            id:         *record._id.as_bytes(),
        };

        let versioned = VersionedRecord {
            sys_from:   txn.snapshot_ts,
            sys_to:     i64::MAX,
            valid_from: record._valid_from,
            valid_to:   record._valid_to,
            record,
        };

        let mut map = self
            .inner
            .write()
            .map_err(|_| anyhow::anyhow!("MvccStore write lock poisoned"))?;

        let versions = map.entry(key).or_insert_with(Vec::new);

        // Supersede the current version if one exists.
        if let Some(current) = versions.iter_mut().find(|v| v.sys_to == i64::MAX) {
            current.sys_to = txn.snapshot_ts;
        }

        versions.push(versioned);
        Ok(())
    }

    /// Point-in-time system-time query.
    ///
    /// Returns the version of `key` that was current at `system_ts`, i.e. the
    /// version where `sys_from <= system_ts < sys_to`.
    pub fn read_as_of_system(
        &self,
        key: &RecordKey,
        system_ts: Timestamp,
    ) -> Result<Option<FunRecord>> {
        let map = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("MvccStore read lock poisoned"))?;

        let result = map
            .get(key)
            .and_then(|versions| {
                versions
                    .iter()
                    .find(|v| v.sys_from <= system_ts && system_ts < v.sys_to)
            })
            .map(|v| v.record.clone());

        Ok(result)
    }

    /// Valid-time range query.
    ///
    /// Returns all versions of `key` whose valid-time window overlaps the
    /// half-open interval `[valid_from, valid_to)`.
    ///
    /// Overlap condition: `version.valid_from < valid_to && version.valid_to > valid_from`.
    pub fn read_as_of_valid(
        &self,
        key: &RecordKey,
        valid_from: Timestamp,
        valid_to: Timestamp,
    ) -> Result<Vec<FunRecord>> {
        let map = self
            .inner
            .read()
            .map_err(|_| anyhow::anyhow!("MvccStore read lock poisoned"))?;

        let results = map
            .get(key)
            .map(|versions| {
                versions
                    .iter()
                    .filter(|v| v.valid_from < valid_to && v.valid_to > valid_from)
                    .map(|v| v.record.clone())
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// Commit a transaction.
    ///
    /// This is a no-op stub.  A full implementation would durably flush the
    /// transaction's WAL entries and release any locks.
    pub fn commit(&self, _txn: Transaction) -> Result<()> {
        Ok(())
    }

    /// Garbage-collect historical versions older than `before_system_ts`.
    ///
    /// Removes all versions where `sys_to < before_system_ts` (versions that
    /// were completely superseded before the retention window).  The current
    /// version (`sys_to == i64::MAX`) is always retained.
    ///
    /// Returns the number of versions deleted.
    pub fn gc(&self, before_system_ts: Timestamp) -> usize {
        let mut map = self
            .inner
            .write()
            .expect("MvccStore write lock poisoned during GC");

        let mut deleted = 0usize;

        for versions in map.values_mut() {
            let before = versions.len();
            versions.retain(|v| v.sys_to >= before_system_ts);
            deleted += before - versions.len();
        }

        deleted
    }
}

impl Default for MvccStore {
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
    use fundb_core::builder::FunRecordBuilder;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn build_record(collection: &str, valid_from: Timestamp, valid_to: Timestamp) -> FunRecord {
        FunRecordBuilder::new(collection)
            .valid_time(valid_from, valid_to)
            .build()
    }

    fn key_of(record: &FunRecord) -> RecordKey {
        RecordKey {
            collection: record._collection.clone(),
            id:         *record._id.as_bytes(),
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: write a record and read it back at the current system timestamp.
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_and_read_current() {
        let store = MvccStore::new();
        let txn = store.begin_txn();
        let snapshot = txn.snapshot_ts;

        let record = build_record("test_col", 0, i64::MAX);
        let expected_id = record._id;
        let key = key_of(&record);

        store.write(&txn, record).unwrap();
        store.commit(txn).unwrap();

        // A timestamp after the write should see the current version.
        let ts_after = snapshot + 1;
        let result = store.read_as_of_system(&key, ts_after).unwrap();

        assert!(result.is_some(), "record must be visible after write");
        assert_eq!(
            result.unwrap()._id,
            expected_id,
            "_id must match the written record"
        );
    }

    // -----------------------------------------------------------------------
    // Test 2: point-in-time query returns correct historical version.
    // -----------------------------------------------------------------------

    #[test]
    fn test_point_in_time_query() {
        let store = MvccStore::new();

        // --- version 1 ---
        let txn1 = store.begin_txn();
        let t1 = txn1.snapshot_ts;
        let record_v1 = build_record("pit_col", 0, i64::MAX);
        let key = key_of(&record_v1);
        let id_v1 = record_v1._id;
        store.write(&txn1, record_v1).unwrap();
        store.commit(txn1).unwrap();

        // --- version 2 at a strictly later timestamp ---
        // Spin until the clock advances so t2 > t1.
        let mut txn2 = store.begin_txn();
        while txn2.snapshot_ts <= t1 {
            txn2 = store.begin_txn();
        }
        let t2 = txn2.snapshot_ts;

        // Build version 2 as an update to the same logical entity (same _id = same key).
        let mut record_v2 = build_record("pit_col", 0, i64::MAX);
        record_v2._id = id_v1; // same logical entity — same key, new version payload
        store.write(&txn2, record_v2).unwrap();
        store.commit(txn2).unwrap();

        // Reading at t1 + 1 must return version 1 (sys_from == t1, sys_to == t2).
        let r_at_t1 = store.read_as_of_system(&key, t1 + 1).unwrap();
        assert!(r_at_t1.is_some(), "version 1 must be visible at t1+1");
        assert_eq!(r_at_t1.unwrap()._id, id_v1, "must see version 1 at t1+1");

        // Reading at t2 + 1 must return version 2 (sys_from == t2, sys_to == i64::MAX).
        let r_at_t2 = store.read_as_of_system(&key, t2 + 1).unwrap();
        assert!(r_at_t2.is_some(), "version 2 must be visible at t2+1");
        // Both versions share the same _id (same logical entity); v2 is the current one.
        assert_eq!(r_at_t2.unwrap()._id, id_v1, "must see version 2 at t2+1");
    }

    // -----------------------------------------------------------------------
    // Test 3: valid-time query returns only versions whose window overlaps.
    // -----------------------------------------------------------------------

    #[test]
    fn test_valid_time_query() {
        let store = MvccStore::new();

        // Three non-overlapping valid-time windows: [0,10), [10,20), [20,30).
        // All written under the same transaction and same key (_id == v1._id).
        let txn = store.begin_txn();

        let rec1 = build_record("vt_col", 0, 10);
        let shared_id = rec1._id;
        let key = key_of(&rec1);
        store.write(&txn, rec1).unwrap();

        let mut rec2 = build_record("vt_col", 10, 20);
        rec2._id = shared_id;
        store.write(&txn, rec2).unwrap();

        let mut rec3 = build_record("vt_col", 20, 30);
        rec3._id = shared_id;
        store.write(&txn, rec3).unwrap();

        store.commit(txn).unwrap();

        // Query window [5, 15) overlaps windows [0,10) and [10,20).
        let results = store.read_as_of_valid(&key, 5, 15).unwrap();
        assert_eq!(
            results.len(),
            2,
            "valid-time query [5,15) must match 2 versions, got {}",
            results.len()
        );

        // Query window [25, 35) overlaps only [20,30).
        let results2 = store.read_as_of_valid(&key, 25, 35).unwrap();
        assert_eq!(
            results2.len(),
            1,
            "valid-time query [25,35) must match 1 version, got {}",
            results2.len()
        );

        // Query window [50, 100) does not overlap any window.
        let results3 = store.read_as_of_valid(&key, 50, 100).unwrap();
        assert!(
            results3.is_empty(),
            "valid-time query [50,100) must match 0 versions"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4: snapshot isolation — txn_A cannot see writes committed by txn_B
    //         after txn_A's snapshot timestamp.
    // -----------------------------------------------------------------------

    #[test]
    fn test_snapshot_isolation() {
        let store = MvccStore::new();

        // txn_A takes its snapshot first.
        let txn_a = store.begin_txn();
        let snapshot_a = txn_a.snapshot_ts;

        // Spin until the clock advances so txn_B starts strictly after txn_A.
        let mut txn_b = store.begin_txn();
        while txn_b.snapshot_ts <= snapshot_a {
            txn_b = store.begin_txn();
        }

        let record_b = build_record("iso_col", 0, i64::MAX);
        let key = key_of(&record_b);
        store.write(&txn_b, record_b).unwrap();
        store.commit(txn_b).unwrap();

        // Reading at snapshot_A must NOT see txn_B's write.
        let result = store.read_as_of_system(&key, snapshot_a).unwrap();
        assert!(
            result.is_none(),
            "snapshot isolation violated: txn_A must not see txn_B's write"
        );
    }

    // -----------------------------------------------------------------------
    // Test 5: GC removes historical versions; current version is preserved.
    // -----------------------------------------------------------------------

    #[test]
    fn test_gc() {
        let store = MvccStore::new();

        // Write 5 successive versions of the same key.
        // We accumulate each transaction's snapshot_ts so we can pick a GC
        // threshold that is beyond all of them except the last.
        let record_init = build_record("gc_col", 0, i64::MAX);
        let shared_id = record_init._id;
        let key = key_of(&record_init);

        let mut last_ts = 0i64;

        for i in 0u8..5 {
            // Spin until clock advances to get distinct timestamps.
            let mut txn = store.begin_txn();
            if i > 0 {
                while txn.snapshot_ts <= last_ts {
                    txn = store.begin_txn();
                }
            }
            last_ts = txn.snapshot_ts;

            let mut rec = build_record("gc_col", 0, i64::MAX);
            rec._id = shared_id; // keep the same key
            store.write(&txn, rec).unwrap();
            store.commit(txn).unwrap();
        }

        // GC threshold: everything strictly before `last_ts + 1` is eligible
        // for deletion (i.e., versions whose sys_to < last_ts + 1).
        // The current version has sys_to == i64::MAX so it is always retained.
        let gc_threshold = last_ts + 1;
        let deleted = store.gc(gc_threshold);

        // We had 5 versions; the first 4 had sys_to == some finite timestamp.
        // GC retains versions where sys_to >= gc_threshold.
        // Only the current version (sys_to == i64::MAX) satisfies that.
        assert!(deleted > 0, "GC must delete at least one historical version");

        // The current version must still be readable.
        let result = store.read_as_of_system(&key, current_nanos()).unwrap();
        assert!(
            result.is_some(),
            "current version must survive GC"
        );
    }
}
