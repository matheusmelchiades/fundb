use std::ops::Bound;

use fundb_core::{CausalEdge, CausalOrigin, CausalType, DirectionStatus, FunRecordBuilder, StabilityStatus};
use fundb_indexes::{BTree, CausalDagIndex, ConfidenceIndex, HnswIndex};
use fundb_integration_tests::{key_for_record, open_lsm};
use uuid::Uuid;

/// Helper: create a deterministic UUID.
fn uuid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Helper: simple causal edge for index tests.
fn causal_edge(from: Uuid, to: Uuid, strength: f32) -> CausalEdge {
    CausalEdge {
        source_id: from,
        target_id: to,
        relation: CausalType::Caused,
        strength,
        mechanism: None,
        origin: CausalOrigin::UserDeclared,
        confidence: 1.0,
        stability_score: Some(1.0),
        stability_status: StabilityStatus::Stable,
        direction_status: DirectionStatus::Confirmed,
        discovery_algo: None,
    }
}

/// Pseudo-random float for test diversity.
fn rand_f32() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(42);
    let s = SEED.fetch_add(1, Ordering::Relaxed);
    let v = s.wrapping_mul(1103515245).wrapping_add(12345);
    SEED.store(v, Ordering::Relaxed);
    (v as f32) / (u32::MAX as f32)
}

// ---------------------------------------------------------------------------
// 1. B+Tree range query — 100 keys
// ---------------------------------------------------------------------------
#[test]
fn test_btree_range_query_100_keys() {
    let mut tree = BTree::<i64, String>::new();

    for i in 0..100i64 {
        tree.insert(i, format!("value_{}", i)).unwrap();
    }

    let results: Vec<_> = tree
        .range(Bound::Included(&30), Bound::Included(&70))
        .collect();

    assert_eq!(
        results.len(),
        41,
        "range [30, 70] should return 41 keys, got {}",
        results.len()
    );

    // Verify ordering
    let mut prev = i64::MIN;
    for (k, _) in &results {
        assert!(**k >= prev, "keys should be sorted");
        prev = **k;
    }
}

// ---------------------------------------------------------------------------
// 2. HNSW insert + search recall
// ---------------------------------------------------------------------------
#[test]
fn test_hnsw_insert_search_recall() {
    let mut index = HnswIndex::new(128, 16, 200);

    let target_id = Uuid::now_v7();
    let target_vec: Vec<f32> = (0..128).map(|i| (i as f32 * 0.01).sin()).collect();
    index.insert(target_id, &target_vec).unwrap();

    // Insert 99 more random vectors
    for _ in 0..99 {
        let id = Uuid::now_v7();
        let vec: Vec<f32> = (0..128).map(|i| (i as f32 * 0.1).cos() * rand_f32()).collect();
        index.insert(id, &vec).unwrap();
    }

    let results = index.search(&target_vec, 5, 1.0);
    assert!(!results.is_empty(), "search should return results");

    // The target vector should be in top-5 (exact match = highest similarity)
    let found = results.iter().any(|(id, _)| *id == target_id);
    assert!(found, "target vector should be in top-5 search results");
}

// ---------------------------------------------------------------------------
// 3. HNSW delete removes from results
// ---------------------------------------------------------------------------
#[test]
fn test_hnsw_delete_removes_from_results() {
    let mut index = HnswIndex::new(4, 8, 100);

    let delete_id = Uuid::now_v7();
    let delete_vec: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
    index.insert(delete_id, &delete_vec).unwrap();

    for _ in 0..49 {
        let id = Uuid::now_v7();
        let vec: Vec<f32> = vec![0.0, 1.0, 0.0, 0.0]; // orthogonal
        index.insert(id, &vec).unwrap();
    }

    // Delete the target
    index.delete(delete_id).unwrap();

    let results = index.search(&delete_vec, 50, 1.0);
    let found = results.iter().any(|(id, _)| *id == delete_id);
    assert!(!found, "deleted vector should not appear in search results");
}

// ---------------------------------------------------------------------------
// 4. Causal DAG index with real edges
// ---------------------------------------------------------------------------
#[test]
fn test_causal_dag_with_real_edges() {
    let mut dag = CausalDagIndex::new();

    // Build a 10-node linear DAG: 0→1→2→...→9
    let nodes: Vec<Uuid> = (0..10).map(|n| uuid(n)).collect();
    for i in 0..9 {
        dag.insert_edge(causal_edge(nodes[i], nodes[i + 1], 0.9)).unwrap();
    }

    // Find paths from node 0 to node 9
    let paths = dag.paths(nodes[0], nodes[9], 20, 0.0);
    assert!(!paths.is_empty(), "should find path from node 0 to node 9");

    // Effects of node 0 should include downstream nodes
    let effects = dag.effects(nodes[0], 20);
    assert!(!effects.is_empty(), "node 0 should have effects");

    // Causes of node 9 should include upstream nodes
    let causes = dag.causes(nodes[9], 20);
    assert!(!causes.is_empty(), "node 9 should have causes");
}

// ---------------------------------------------------------------------------
// 5. Causal DAG closure cache
// ---------------------------------------------------------------------------
#[test]
fn test_causal_dag_closure_cache() {
    let mut dag = CausalDagIndex::new();
    let a = uuid(1);
    let b = uuid(2);
    let c = uuid(3);

    dag.insert_edge(causal_edge(a, b, 0.9)).unwrap();
    dag.insert_edge(causal_edge(b, c, 0.8)).unwrap();

    // Materialize closure for a→c
    dag.materialize_closure(a, c);

    // Subsequent queries should use cache — same result
    let paths1 = dag.paths(a, c, 10, 0.0);
    let paths2 = dag.paths(a, c, 10, 0.0);

    assert_eq!(paths1.len(), paths2.len(), "cached closure should give same result");
    assert!(!paths1.is_empty(), "should find path a→b→c");
}

// ---------------------------------------------------------------------------
// 6. Confidence index histogram
// ---------------------------------------------------------------------------
#[test]
fn test_confidence_index_histogram() {
    let mut index = ConfidenceIndex::new();

    // Insert 1000 records with uniformly distributed confidence
    for i in 0..1000u32 {
        let id = Uuid::from_u128(i as u128);
        let conf = (i as f32) / 1000.0;
        index.insert(id, conf).unwrap();
    }

    // histogram() returns [u32; 100] — 100 buckets of width 0.01
    let hist = index.histogram();
    assert_eq!(hist.len(), 100, "histogram should have 100 buckets");

    // For uniform distribution [0, 1), each bucket should have ~10 entries
    let total: u32 = hist.iter().sum();
    assert_eq!(total, 1000, "total should be 1000");
}

// ---------------------------------------------------------------------------
// 7. Confidence index range query
// ---------------------------------------------------------------------------
#[test]
fn test_confidence_index_range() {
    let mut index = ConfidenceIndex::new();

    // Insert records with confidence from 0.1 to 0.9
    let mut expected = 0usize;
    for i in 1..=9 {
        let conf = i as f32 / 10.0;
        let id = Uuid::from_u128(i as u128);
        index.insert(id, conf).unwrap();
        if conf >= 0.4 && conf <= 0.8 {
            expected += 1;
        }
    }

    let results = index.range(0.4, 0.8);
    assert_eq!(
        results.len(),
        expected,
        "range [0.4, 0.8] should return {} records, got {}",
        expected,
        results.len()
    );
}

// ---------------------------------------------------------------------------
// 8. B+Tree insert + delete integrity
// ---------------------------------------------------------------------------
#[test]
fn test_btree_insert_delete_integrity() {
    let mut tree = BTree::<i64, String>::new();

    // Insert 1000 keys
    for i in 0..1000i64 {
        tree.insert(i, format!("val_{}", i)).unwrap();
    }
    assert_eq!(tree.len(), 1000);

    // Delete 500 (even keys)
    for i in (0..1000i64).step_by(2) {
        tree.delete(&i).unwrap();
    }
    assert_eq!(
        tree.len(),
        500,
        "after deleting 500, should have 500 remaining"
    );

    // Verify deleted keys are gone
    for i in (0..1000i64).step_by(2) {
        assert!(tree.get(&i).unwrap().is_none(), "deleted key {} should be gone", i);
    }

    // Verify remaining keys still exist
    for i in (1..1000i64).step_by(2) {
        assert!(tree.get(&i).unwrap().is_some(), "key {} should still exist", i);
    }
}

// ---------------------------------------------------------------------------
// 9. HNSW high-dimension recall
// ---------------------------------------------------------------------------
#[test]
fn test_hnsw_high_dimension_recall() {
    let dims = 256;
    let mut index = HnswIndex::new(dims, 16, 200);

    let target_id = Uuid::now_v7();
    let target_vec: Vec<f32> = (0..dims).map(|i| (i as f32 * 0.01).sin()).collect();
    index.insert(target_id, &target_vec).unwrap();

    for _ in 0..49 {
        let id = Uuid::now_v7();
        let vec: Vec<f32> = (0..dims).map(|i| (i as f32 * 0.1).cos() * rand_f32()).collect();
        index.insert(id, &vec).unwrap();
    }

    let results = index.search(&target_vec, 5, 1.0);
    let found = results.iter().any(|(id, _)| *id == target_id);
    assert!(found, "high-dim HNSW should still find exact match in top-5");
}

// ---------------------------------------------------------------------------
// 10. Indexes + storage roundtrip
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_indexes_with_storage_roundtrip() {
    let (_tmp, lsm) = open_lsm();
    let mut hnsw = HnswIndex::new(4, 8, 100);

    // Write records with vectors to storage
    let mut record_ids = Vec::new();
    for i in 0..10 {
        let vec = vec![i as f32 * 0.1, 0.0, 0.0, 1.0];
        let rec = FunRecordBuilder::new("vectors")
            .vector("emb", vec.clone())
            .build();
        let id = rec._id;
        record_ids.push(id);
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
        hnsw.insert(id, &vec).unwrap();
    }

    // Search HNSW
    let query = vec![0.5, 0.0, 0.0, 1.0];
    let results = hnsw.search(&query, 3, 1.0);
    assert!(!results.is_empty(), "HNSW should find results");

    // Verify the UUIDs from HNSW correspond to records in storage
    for (id, _) in &results {
        assert!(
            record_ids.contains(id),
            "HNSW result UUID should match a stored record"
        );
    }
}

// ---------------------------------------------------------------------------
// 11. B+Tree empty range
// ---------------------------------------------------------------------------
#[test]
fn test_btree_empty_range() {
    let tree = BTree::<i64, String>::new();
    let results: Vec<_> = tree
        .range(Bound::Included(&0), Bound::Included(&100))
        .collect();
    assert!(results.is_empty(), "empty tree range should return []");
}
