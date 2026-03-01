use std::sync::Arc;

use fundb_core::RecordKey;
use fundb_storage::LsmTree;
use tempfile::TempDir;

use crate::records::{key_for_record, make_record};

/// Open a fresh LsmTree in a temporary directory.
/// Returns (TempDir, Arc<LsmTree>) — keep TempDir alive for the duration of the test.
pub fn open_lsm() -> (TempDir, Arc<LsmTree>) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let lsm = LsmTree::open(tmp.path(), 1024 * 1024).expect("failed to open LsmTree");
    (tmp, Arc::new(lsm))
}

/// Open LsmTree and pre-populate with `n` records in the given collection.
/// Returns (TempDir, Arc<LsmTree>, Vec<RecordKey>).
pub async fn open_lsm_with_records(
    collection: &str,
    n: usize,
) -> (TempDir, Arc<LsmTree>, Vec<RecordKey>) {
    let (tmp, lsm) = open_lsm();
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        let record = make_record(collection);
        let key = key_for_record(&record);
        lsm.write(key.clone(), record).await.expect("write failed");
        keys.push(key);
    }
    (tmp, lsm, keys)
}
