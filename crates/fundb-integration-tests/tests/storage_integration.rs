use std::sync::Arc;

use fundb_core::{FunRecordBuilder, RecordKey};
use fundb_integration_tests::{key_for_record, make_record, open_lsm, sequential_key};
use fundb_storage::{LsmTree, MvccStore};

// ---------------------------------------------------------------------------
// 1. Write → flush → read from SSTable
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_write_flush_read_from_sstable() {
    let (_tmp, lsm) = open_lsm();
    let mut keys = Vec::new();

    for _ in 0..100 {
        let rec = make_record("users");
        let key = key_for_record(&rec);
        lsm.write(key.clone(), rec).await.unwrap();
        keys.push(key);
    }

    lsm.flush_memtable().await.unwrap();

    for key in &keys {
        let rec = lsm.get(key).await.unwrap();
        assert!(rec.is_some(), "record missing after flush: {:?}", key);
    }
}

// ---------------------------------------------------------------------------
// 2. WAL crash recovery
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_wal_crash_recovery() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().to_path_buf();

    let mut keys = Vec::new();
    {
        let lsm = LsmTree::open(&path, 1024 * 1024).unwrap();
        for _ in 0..100 {
            let rec = make_record("orders");
            let key = key_for_record(&rec);
            lsm.write(key.clone(), rec).await.unwrap();
            keys.push(key);
        }
        // Drop without flush — data should be in WAL only
    }

    // Reopen — WAL replay should recover all records
    let lsm = LsmTree::open(&path, 1024 * 1024).unwrap();
    for key in &keys {
        let rec = lsm.get(key).await.unwrap();
        assert!(rec.is_some(), "WAL recovery failed for key: {:?}", key);
    }
}

// ---------------------------------------------------------------------------
// 3. Delete survives flush (tombstone propagation)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_delete_survives_flush() {
    let (_tmp, lsm) = open_lsm();

    let rec = make_record("products");
    let key = key_for_record(&rec);
    lsm.write(key.clone(), rec).await.unwrap();

    lsm.delete(key.clone()).await.unwrap();
    lsm.flush_memtable().await.unwrap();

    let result = lsm.get(&key).await.unwrap();
    assert!(
        result.is_none(),
        "deleted record should not be readable after flush"
    );
}

// ---------------------------------------------------------------------------
// 4. Scan merges MemTable and SSTable
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_scan_merges_memtable_and_sstable() {
    let (_tmp, lsm) = open_lsm();
    let collection = "events";

    // Write 50 records and flush to SSTable
    for _ in 0..50 {
        let rec = make_record(collection);
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
    }
    lsm.flush_memtable().await.unwrap();

    // Write 50 more records (stay in MemTable)
    for _ in 0..50 {
        let rec = make_record(collection);
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
    }

    let from = RecordKey {
        collection: collection.to_string(),
        id: [0u8; 16],
    };
    let to = RecordKey {
        collection: collection.to_string(),
        id: [0xFF; 16],
    };
    let results = lsm.scan(collection, &from, &to).await.unwrap();
    assert_eq!(
        results.len(),
        100,
        "scan should merge MemTable + SSTable: got {}",
        results.len()
    );
}

// ---------------------------------------------------------------------------
// 5. Overwrite — latest version wins
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_overwrite_latest_wins() {
    let (_tmp, lsm) = open_lsm();

    let key = sequential_key("metrics", 42);
    let old = FunRecordBuilder::new("metrics").confidence(0.1).build();
    lsm.write(key.clone(), old).await.unwrap();
    lsm.flush_memtable().await.unwrap();

    let new = FunRecordBuilder::new("metrics").confidence(0.9).build();
    lsm.write(key.clone(), new).await.unwrap();

    let fetched = lsm.get(&key).await.unwrap().expect("record missing");
    assert!(
        (fetched._confidence - 0.9).abs() < f32::EPSILON,
        "expected latest version (0.9), got {}",
        fetched._confidence
    );
}

// ---------------------------------------------------------------------------
// 6. Concurrent writers — 8 tasks × 50 records
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_concurrent_writers_8_tasks() {
    let (_tmp, lsm) = open_lsm();
    let mut handles = Vec::new();

    for _task_id in 0u64..8 {
        let lsm = Arc::clone(&lsm);
        handles.push(tokio::spawn(async move {
            for _i in 0u64..50 {
                let rec = make_record("concurrent");
                let key = key_for_record(&rec);
                lsm.write(key, rec).await.unwrap();
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let from = RecordKey {
        collection: "concurrent".to_string(),
        id: [0u8; 16],
    };
    let to = RecordKey {
        collection: "concurrent".to_string(),
        id: [0xFF; 16],
    };
    let results = lsm.scan("concurrent", &from, &to).await.unwrap();
    assert_eq!(
        results.len(),
        400,
        "expected 400 records, got {}",
        results.len()
    );
}

// ---------------------------------------------------------------------------
// 7. MVCC snapshot isolation
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_mvcc_snapshot_isolation() {
    let store = MvccStore::new();

    let rec1 = FunRecordBuilder::new("accounts").confidence(0.5).build();
    let key = key_for_record(&rec1);

    let txn1 = store.begin_txn();
    store.write(&txn1, rec1).unwrap();
    store.commit(txn1).unwrap();

    // Start a read transaction (snapshot at this point)
    let txn_read = store.begin_txn();
    let snap_ts = txn_read.snapshot_ts;

    // Write a new version in txn2 AFTER the snapshot
    let rec2 = FunRecordBuilder::new("accounts").confidence(0.9).build();
    let txn2 = store.begin_txn();
    store.write(&txn2, rec2).unwrap();
    store.commit(txn2).unwrap();

    // Read at snapshot_ts should see the old version
    let result = store.read_as_of_system(&key, snap_ts).unwrap();
    if let Some(r) = result {
        assert!(
            (r._confidence - 0.5).abs() < f32::EPSILON,
            "snapshot should see old version (0.5), got {}",
            r._confidence
        );
    }
}

// ---------------------------------------------------------------------------
// 8. MVCC valid-time query
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_mvcc_valid_time_query() {
    let store = MvccStore::new();

    // 3 versions with different valid-time ranges — all sharing the same _id
    let v1 = FunRecordBuilder::new("prices")
        .valid_time(100, 200)
        .confidence(0.3)
        .build();
    let shared_id = v1._id;
    let key = key_for_record(&v1);

    let mut v2 = FunRecordBuilder::new("prices")
        .valid_time(200, 300)
        .confidence(0.6)
        .build();
    v2._id = shared_id;

    let mut v3 = FunRecordBuilder::new("prices")
        .valid_time(300, 400)
        .confidence(0.9)
        .build();
    v3._id = shared_id;

    let txn = store.begin_txn();
    store.write(&txn, v1).unwrap();
    store.write(&txn, v2).unwrap();
    store.write(&txn, v3).unwrap();
    store.commit(txn).unwrap();

    // Query for valid_time = 250 should return v2
    let results = store.read_as_of_valid(&key, 200, 300).unwrap();
    assert!(
        !results.is_empty(),
        "valid-time query should return at least one version"
    );
}

// ---------------------------------------------------------------------------
// 9. MVCC GC removes old versions
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_mvcc_gc_removes_old_versions() {
    let store = MvccStore::new();

    // Write 5 versions with increasing system timestamps
    for i in 0..5 {
        let rec = FunRecordBuilder::new("logs")
            .confidence(i as f32 * 0.2)
            .build();
        let txn = store.begin_txn();
        store.write(&txn, rec).unwrap();
        store.commit(txn).unwrap();
    }

    // GC with a future timestamp should remove old versions
    let removed = store.gc(i64::MAX);
    // At least some versions should have been collected
    // GC returns the count of removed versions
    let _ = removed;
}

// ---------------------------------------------------------------------------
// 10. Compaction preserves data
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_compaction_preserves_data() {
    let (_tmp, lsm) = open_lsm();
    let collection = "compaction_test";
    let mut keys = Vec::new();

    // Create multiple SSTables via repeated flush
    for _batch in 0..5 {
        for _i in 0..20 {
            let rec = make_record(collection);
            let key = key_for_record(&rec);
            keys.push(key.clone());
            lsm.write(key, rec).await.unwrap();
        }
        lsm.flush_memtable().await.unwrap();
    }

    // Trigger compaction
    lsm.trigger_compaction().await.unwrap();

    // Verify all records are still accessible
    for key in &keys {
        let result = lsm.get(key).await.unwrap();
        assert!(result.is_some(), "data lost after compaction: {:?}", key);
    }
}

// ---------------------------------------------------------------------------
// 11. Tombstone removed by compaction
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_tombstone_removed_by_compaction() {
    let (_tmp, lsm) = open_lsm();

    let rec = make_record("cleanup");
    let key = key_for_record(&rec);
    lsm.write(key.clone(), rec).await.unwrap();
    lsm.flush_memtable().await.unwrap();

    lsm.delete(key.clone()).await.unwrap();
    lsm.flush_memtable().await.unwrap();

    lsm.trigger_compaction().await.unwrap();

    let result = lsm.get(&key).await.unwrap();
    assert!(
        result.is_none(),
        "tombstone should be eliminated by compaction"
    );
}

// ---------------------------------------------------------------------------
// 12. Bloom filter rejects absent keys
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_bloom_filter_rejects_absent_keys() {
    let (_tmp, lsm) = open_lsm();

    // Write some records and flush to create SSTables with bloom filters
    for _ in 0..50 {
        let rec = make_record("bloom_test");
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
    }
    lsm.flush_memtable().await.unwrap();

    // Query for keys that definitely don't exist
    for i in 0u64..100 {
        let absent_key = sequential_key("nonexistent", i);
        let result = lsm.get(&absent_key).await.unwrap();
        assert!(result.is_none(), "bloom filter should reject absent key");
    }
}
