// crates/fundb-cluster/src/dist_query.rs
//
// STORY-8-3: Distributed Query Execution — scatter/gather coordinator.
//
// This module implements the DistQueryCoordinator which fans out queries to all
// relevant shards in parallel, collects results, and merges them according to the
// query type and OQ-9 cross-shard confidence propagation rules.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use crate::sharding::{NodeId, ShardId, ShardMap};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Runtime tuning knobs for the distributed query coordinator.
#[derive(Debug, Clone)]
pub struct DistQueryConfig {
    /// For ANN / VectorScan: send `oversample_factor × top_k` to each shard,
    /// then re-rank globally and trim to `top_k`.
    pub oversample_factor: usize,
    /// Per-shard deadline in milliseconds before a shard result is considered
    /// timed out (counted as a failed shard).
    pub timeout_ms: u64,
    /// Maximum number of shards queried concurrently via tokio::spawn.
    pub max_concurrent_shards: usize,
}

impl Default for DistQueryConfig {
    fn default() -> Self {
        Self {
            oversample_factor: 3,
            timeout_ms: 5000,
            max_concurrent_shards: 16,
        }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during distributed query execution.
#[derive(Debug)]
pub enum DistQueryError {
    /// A shard did not respond within the configured timeout.
    ShardTimeout(ShardId),
    /// A shard returned an application-level error.
    ShardFailed { shard: ShardId, reason: String },
    /// The collection maps to zero shards (collection unknown or empty cluster).
    NoShards,
    /// Final merge step encountered an inconsistency.
    MergeError(String),
}

impl std::fmt::Display for DistQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistQueryError::ShardTimeout(id) => {
                write!(f, "shard {id} timed out. Check shard health and network connectivity between nodes")
            }
            DistQueryError::ShardFailed { shard, reason } => {
                write!(f, "shard {shard} failed: {reason}")
            }
            DistQueryError::NoShards => {
                write!(f, "no shards found for the requested collection. The collection may not exist or the cluster has no available nodes")
            }
            DistQueryError::MergeError(msg) => {
                write!(f, "failed to merge results from shards: {msg}")
            }
        }
    }
}

impl std::error::Error for DistQueryError {}

// ---------------------------------------------------------------------------
// Query types
// ---------------------------------------------------------------------------

/// The distributed query variants understood by the coordinator.
#[derive(Debug, Clone)]
pub enum DistQuery {
    /// Full collection scan with an optional predicate string and row limit.
    Scan {
        filter: Option<String>,
        limit: Option<usize>,
    },
    /// Approximate Nearest Neighbour search against a named vector index.
    VectorScan {
        query_vector: Vec<f32>,
        top_k: usize,
        index_name: String,
    },
    /// Single-record fetch routed to exactly one shard by UUID hash.
    PointLookup { record_id: Uuid },
    /// Aggregation over a numeric field with an optional filter.
    Aggregate {
        op: AggOp,
        field: String,
        filter: Option<String>,
    },
}

/// Supported aggregation operators.
#[derive(Debug, Clone, PartialEq)]
pub enum AggOp {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

// ---------------------------------------------------------------------------
// Per-shard result and merged result
// ---------------------------------------------------------------------------

/// A single record returned by a shard execution.
#[derive(Debug, Clone)]
pub struct ShardRecord {
    /// Stable record identifier (UUID v7).
    pub id: Uuid,
    /// Epistemic confidence at the time of retrieval (0.0–1.0).
    pub confidence: f64,
    /// Relevance / similarity score (higher = more relevant).
    pub score: f32,
    /// MessagePack-serialised FunRecord fields (opaque bytes).
    pub payload: Vec<u8>,
}

/// Raw result returned by a single shard.
#[derive(Debug)]
pub struct ShardResult {
    pub shard_id: ShardId,
    pub node_id: NodeId,
    pub records: Vec<ShardRecord>,
    /// Partial row count for `Aggregate { Count }`.
    pub partial_count: Option<u64>,
    /// Partial numeric sum for `Aggregate { Sum / Avg / Min / Max }`.
    pub partial_sum: Option<f64>,
    /// Wall-clock time spent executing on that shard (milliseconds).
    pub elapsed_ms: u64,
}

/// Final merged result returned to the caller.
#[derive(Debug)]
pub struct MergedResult {
    /// Globally sorted / trimmed record set.
    pub records: Vec<ShardRecord>,
    /// Total row count (set for `Aggregate { Count }`).
    pub total_count: Option<u64>,
    /// Final aggregate value (set for non-Count aggregations).
    pub aggregate_value: Option<f64>,
    /// Number of shards that participated in this query.
    pub shards_queried: usize,
    /// Number of shards that timed out or returned an error.
    pub shards_failed: usize,
    /// Total wall-clock time for the scatter/gather round-trip (milliseconds).
    pub total_elapsed_ms: u64,
}

// ---------------------------------------------------------------------------
// Mock shard executor
// ---------------------------------------------------------------------------

/// An in-process shard executor used by tests (and optionally development) in
/// place of real network RPCs.
pub struct MockShardExecutor {
    data: HashMap<ShardId, Vec<ShardRecord>>,
}

impl MockShardExecutor {
    /// Create an empty executor with no data seeded.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Populate a shard with a fixed set of records.
    pub fn seed_shard(&mut self, shard: ShardId, records: Vec<ShardRecord>) {
        self.data.insert(shard, records);
    }

    /// Execute a query against a single shard and return a `ShardResult`.
    /// All execution is synchronous and in-process (no I/O).
    pub fn execute_on_shard(&self, shard: ShardId, query: &DistQuery) -> ShardResult {
        let t0 = Instant::now();
        let shard_records = self.data.get(&shard).cloned().unwrap_or_default();

        let (records, partial_count, partial_sum) = match query {
            DistQuery::Scan { filter: _, limit } => {
                // Return all records, honouring an optional per-shard limit.
                let mut out = shard_records.clone();
                if let Some(lim) = limit {
                    out.truncate(*lim);
                }
                (out, None, None)
            }

            DistQuery::VectorScan {
                query_vector,
                top_k,
                index_name: _,
            } => {
                // Compute a deterministic mock similarity score from payload bytes.
                let q_mag = query_vector
                    .iter()
                    .map(|v| v * v)
                    .sum::<f32>()
                    .sqrt()
                    .max(1e-9);

                let mut scored: Vec<ShardRecord> = shard_records
                    .into_iter()
                    .map(|mut r| {
                        let byte_sum: f32 = r
                            .payload
                            .iter()
                            .take(4)
                            .map(|&b| b as f32 / 255.0)
                            .sum::<f32>();
                        r.score = byte_sum / q_mag;
                        r
                    })
                    .collect();

                scored.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                scored.truncate(*top_k);
                (scored, None, None)
            }

            DistQuery::PointLookup { record_id } => {
                let found: Vec<ShardRecord> = shard_records
                    .into_iter()
                    .filter(|r| r.id == *record_id)
                    .collect();
                (found, None, None)
            }

            DistQuery::Aggregate {
                op,
                field: _,
                filter: _,
            } => {
                let count = shard_records.len() as u64;
                let sum: f64 = shard_records.iter().map(|r| r.confidence).sum();
                match op {
                    AggOp::Count => (vec![], Some(count), None),
                    AggOp::Sum => (vec![], None, Some(sum)),
                    AggOp::Avg => (vec![], Some(count), Some(sum)),
                    AggOp::Min => {
                        let min = shard_records
                            .iter()
                            .map(|r| r.confidence)
                            .fold(f64::INFINITY, f64::min);
                        let min_val = if min.is_infinite() { 0.0 } else { min };
                        (vec![], None, Some(min_val))
                    }
                    AggOp::Max => {
                        let max = shard_records
                            .iter()
                            .map(|r| r.confidence)
                            .fold(f64::NEG_INFINITY, f64::max);
                        let max_val = if max.is_infinite() { 0.0 } else { max };
                        (vec![], None, Some(max_val))
                    }
                }
            }
        };

        let elapsed_ms = t0.elapsed().as_millis() as u64;

        ShardResult {
            shard_id: shard,
            node_id: shard as NodeId,
            records,
            partial_count,
            partial_sum,
            elapsed_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Distributed Query Coordinator
// ---------------------------------------------------------------------------

/// Coordinates scatter/gather distributed query execution across all shards of a
/// collection.
pub struct DistQueryCoordinator {
    shard_map: Arc<ShardMap>,
    config: DistQueryConfig,
    /// Optional mock executor injected for unit-testing without network.
    mock_executor: Option<Arc<MockShardExecutor>>,
}

impl DistQueryCoordinator {
    /// Construct a coordinator backed by real shard addresses from `shard_map`.
    pub fn new(shard_map: Arc<ShardMap>, config: DistQueryConfig) -> Self {
        Self {
            shard_map,
            config,
            mock_executor: None,
        }
    }

    /// Construct a coordinator that delegates shard execution to a
    /// `MockShardExecutor` (for tests).
    pub fn new_with_mock(
        shard_map: Arc<ShardMap>,
        config: DistQueryConfig,
        executor: Arc<MockShardExecutor>,
    ) -> Self {
        Self {
            shard_map,
            config,
            mock_executor: Some(executor),
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Scatter `query` to all shards that own data for `collection`, gather
    /// results, and return a merged `MergedResult`.
    pub async fn execute(
        &self,
        collection: &str,
        query: DistQuery,
    ) -> Result<MergedResult, DistQueryError> {
        let wall_start = Instant::now();

        // --- 1. Determine relevant shards ------------------------------------
        let shards = self.shard_map.shards_for_collection(collection);
        if shards.is_empty() {
            return Err(DistQueryError::NoShards);
        }

        // For PointLookup we only hit the single shard that owns the record.
        let target_shards: Vec<ShardId> = match &query {
            DistQuery::PointLookup { record_id } => {
                let shard = self.route_write(collection, record_id);
                vec![shard]
            }
            _ => shards,
        };

        // --- 2. Build the per-shard query (oversample for ANN) ---------------
        let scattered_query = match &query {
            DistQuery::VectorScan {
                query_vector,
                top_k,
                index_name,
            } => DistQuery::VectorScan {
                query_vector: query_vector.clone(),
                top_k: top_k.saturating_mul(self.config.oversample_factor),
                index_name: index_name.clone(),
            },
            other => other.clone(),
        };

        // --- 3. Scatter: execute on each shard (bounded concurrency) ---------
        let shard_results = self.scatter(target_shards, scattered_query).await;

        // --- 4. Gather: separate successes from failures ---------------------
        let mut successes: Vec<ShardResult> = Vec::new();
        let mut failures: usize = 0;

        for outcome in shard_results {
            match outcome {
                Ok(result) => successes.push(result),
                Err(_) => failures += 1,
            }
        }

        // --- 5. Merge --------------------------------------------------------
        let merged = self.merge(query, successes, failures)?;

        let total_elapsed_ms = wall_start.elapsed().as_millis() as u64;

        Ok(MergedResult {
            total_elapsed_ms,
            ..merged
        })
    }

    /// Deterministically route a write for `record_id` in `collection` to a
    /// shard.  The same `(collection, record_id)` pair always maps to the same
    /// shard (XOR-fold of UUID bytes combined with an FNV-1a hash of the
    /// collection name, then modulo the known shard count).
    pub fn route_write(&self, collection: &str, record_id: &Uuid) -> ShardId {
        let shards = self.shard_map.shards_for_collection(collection);
        if shards.is_empty() {
            return 0;
        }

        // XOR-fold the 128-bit UUID into a u64.
        let uuid_bytes = record_id.as_bytes();
        let lo = u64::from_le_bytes(uuid_bytes[0..8].try_into().unwrap());
        let hi = u64::from_le_bytes(uuid_bytes[8..16].try_into().unwrap());
        let uuid_hash = lo ^ hi;

        // FNV-1a of collection name for namespace isolation.
        let collection_hash = fnv1a(collection.as_bytes());

        let combined = uuid_hash ^ collection_hash;
        let index = (combined as usize) % shards.len();
        shards[index]
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Fan out `query` to all `shards` concurrently (up to
    /// `max_concurrent_shards` at a time), returning one outcome per shard.
    async fn scatter(
        &self,
        shards: Vec<ShardId>,
        query: DistQuery,
    ) -> Vec<Result<ShardResult, DistQueryError>> {
        use tokio::task::JoinHandle;

        let query = Arc::new(query);
        let mut results: Vec<Result<ShardResult, DistQueryError>> =
            Vec::with_capacity(shards.len());

        // Process shards in windows of `max_concurrent_shards`.
        for chunk in shards.chunks(self.config.max_concurrent_shards) {
            let mut handles: Vec<JoinHandle<Result<ShardResult, DistQueryError>>> = Vec::new();

            for &shard_id in chunk {
                let q = Arc::clone(&query);
                let mock = self.mock_executor.clone();
                let timeout_ms = self.config.timeout_ms;

                let handle = tokio::spawn(async move {
                    let fut = execute_on_shard(shard_id, q, mock);
                    match tokio::time::timeout(
                        std::time::Duration::from_millis(timeout_ms),
                        fut,
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_elapsed) => Err(DistQueryError::ShardTimeout(shard_id)),
                    }
                });

                handles.push(handle);
            }

            for handle in handles {
                let outcome = handle.await.unwrap_or_else(|join_err| {
                    Err(DistQueryError::MergeError(format!("join error: {join_err}")))
                });
                results.push(outcome);
            }
        }

        results
    }

    /// Merge per-shard results into a single `MergedResult`.
    fn merge(
        &self,
        query: DistQuery,
        shard_results: Vec<ShardResult>,
        failures: usize,
    ) -> Result<MergedResult, DistQueryError> {
        let shards_queried = shard_results.len() + failures;

        match query {
            // -----------------------------------------------------------------
            // Scan: concatenate, apply OQ-9 confidence floor, sort by
            // confidence DESC, honour global limit.
            // -----------------------------------------------------------------
            DistQuery::Scan { limit, .. } => {
                let mut all_records: Vec<ShardRecord> = Vec::new();
                for sr in shard_results {
                    let floor = confidence_floor_for_shard(&sr);
                    for mut rec in sr.records {
                        // OQ-9: min(record.confidence, shard_floor)
                        rec.confidence = rec.confidence.min(floor);
                        all_records.push(rec);
                    }
                }
                all_records.sort_by(|a, b| {
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                if let Some(lim) = limit {
                    all_records.truncate(lim);
                }
                Ok(MergedResult {
                    records: all_records,
                    total_count: None,
                    aggregate_value: None,
                    shards_queried,
                    shards_failed: failures,
                    total_elapsed_ms: 0,
                })
            }

            // -----------------------------------------------------------------
            // VectorScan: merge, apply OQ-9, sort by score DESC, trim to top_k.
            // -----------------------------------------------------------------
            DistQuery::VectorScan { top_k, .. } => {
                let mut all_records: Vec<ShardRecord> = Vec::new();
                for sr in shard_results {
                    let floor = confidence_floor_for_shard(&sr);
                    for mut rec in sr.records {
                        // OQ-9: cross-shard confidence propagation
                        rec.confidence = rec.confidence.min(floor);
                        all_records.push(rec);
                    }
                }
                all_records.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                all_records.truncate(top_k);
                Ok(MergedResult {
                    records: all_records,
                    total_count: None,
                    aggregate_value: None,
                    shards_queried,
                    shards_failed: failures,
                    total_elapsed_ms: 0,
                })
            }

            // -----------------------------------------------------------------
            // PointLookup: return the first non-empty shard response.
            // -----------------------------------------------------------------
            DistQuery::PointLookup { .. } => {
                let records = shard_results
                    .into_iter()
                    .flat_map(|sr| sr.records)
                    .take(1)
                    .collect();
                Ok(MergedResult {
                    records,
                    total_count: None,
                    aggregate_value: None,
                    shards_queried,
                    shards_failed: failures,
                    total_elapsed_ms: 0,
                })
            }

            // -----------------------------------------------------------------
            // Aggregate: combine partial statistics.
            // -----------------------------------------------------------------
            DistQuery::Aggregate { op, .. } => {
                match op {
                    AggOp::Count => {
                        let total: u64 = shard_results
                            .iter()
                            .filter_map(|sr| sr.partial_count)
                            .sum();
                        Ok(MergedResult {
                            records: vec![],
                            total_count: Some(total),
                            aggregate_value: None,
                            shards_queried,
                            shards_failed: failures,
                            total_elapsed_ms: 0,
                        })
                    }
                    AggOp::Sum => {
                        let total: f64 = shard_results
                            .iter()
                            .filter_map(|sr| sr.partial_sum)
                            .sum();
                        Ok(MergedResult {
                            records: vec![],
                            total_count: None,
                            aggregate_value: Some(total),
                            shards_queried,
                            shards_failed: failures,
                            total_elapsed_ms: 0,
                        })
                    }
                    AggOp::Avg => {
                        let total_sum: f64 = shard_results
                            .iter()
                            .filter_map(|sr| sr.partial_sum)
                            .sum();
                        let total_count: u64 = shard_results
                            .iter()
                            .filter_map(|sr| sr.partial_count)
                            .sum();
                        let avg = if total_count == 0 {
                            0.0
                        } else {
                            total_sum / total_count as f64
                        };
                        Ok(MergedResult {
                            records: vec![],
                            total_count: None,
                            aggregate_value: Some(avg),
                            shards_queried,
                            shards_failed: failures,
                            total_elapsed_ms: 0,
                        })
                    }
                    AggOp::Min => {
                        let min = shard_results
                            .iter()
                            .filter_map(|sr| sr.partial_sum)
                            .fold(f64::INFINITY, f64::min);
                        let value = if min.is_infinite() { 0.0 } else { min };
                        Ok(MergedResult {
                            records: vec![],
                            total_count: None,
                            aggregate_value: Some(value),
                            shards_queried,
                            shards_failed: failures,
                            total_elapsed_ms: 0,
                        })
                    }
                    AggOp::Max => {
                        let max = shard_results
                            .iter()
                            .filter_map(|sr| sr.partial_sum)
                            .fold(f64::NEG_INFINITY, f64::max);
                        let value = if max.is_infinite() { 0.0 } else { max };
                        Ok(MergedResult {
                            records: vec![],
                            total_count: None,
                            aggregate_value: Some(value),
                            shards_queried,
                            shards_failed: failures,
                            total_elapsed_ms: 0,
                        })
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Execute a query on a single shard via the mock executor (test path) or a
/// real network RPC (production, not yet wired).
async fn execute_on_shard(
    shard_id: ShardId,
    query: Arc<DistQuery>,
    mock: Option<Arc<MockShardExecutor>>,
) -> Result<ShardResult, DistQueryError> {
    if let Some(exec) = mock {
        Ok(exec.execute_on_shard(shard_id, &query))
    } else {
        Err(DistQueryError::ShardFailed {
            shard: shard_id,
            reason: "real network RPC not yet implemented".into(),
        })
    }
}

/// Compute a per-shard confidence floor (OQ-9).
///
/// Defaults to `1.0` (no degradation).  A production system would derive this
/// from replication lag, index freshness, or quorum confirmation rate.
#[inline]
fn confidence_floor_for_shard(_sr: &ShardResult) -> f64 {
    1.0
}

/// FNV-1a 64-bit hash of `data`.
fn fnv1a(data: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    let mut hash = OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::Arc;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn node_addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port).parse().unwrap()
    }

    /// Build a `ShardMap` that already knows about `shards` for `collection`.
    ///
    /// We add one physical node per desired shard and route enough records so
    /// that `shards_for_collection` returns exactly those shard IDs.
    ///
    /// Because the consistent-hash ring may map multiple virtual-node entries
    /// to the same shard, we iterate until we have at least one confirmed hit
    /// per shard ID.
    fn make_shard_map_for(collection: &str, desired_shards: &[ShardId]) -> Arc<ShardMap> {
        let mut m = ShardMap::new(1); // 1 virtual node per physical node

        // Add one node per desired shard so we get exactly `N` ring entries
        // (1 vnode × N nodes = N shards total).
        for (i, &shard) in desired_shards.iter().enumerate() {
            let _ = m.add_node(shard as NodeId, node_addr(9000 + i as u16));
        }

        // Force the collection to record all the shards we care about by
        // iterating UUID space until each desired shard has been hit.
        let mut seen: std::collections::HashSet<ShardId> =
            std::collections::HashSet::new();
        let mut counter: u128 = 0;
        while seen.len() < desired_shards.len() {
            let id = Uuid::from_u128(counter);
            let shard = m.shard_for(collection, &id);
            seen.insert(shard);
            counter += 1;
            if counter > 100_000 {
                break; // safety valve
            }
        }

        Arc::new(m)
    }

    fn make_record(id: Uuid, confidence: f64, score: f32) -> ShardRecord {
        ShardRecord {
            id,
            confidence,
            score,
            payload: vec![0x80], // empty msgpack map
        }
    }

    fn make_record_with_payload(
        id: Uuid,
        confidence: f64,
        score: f32,
        payload: Vec<u8>,
    ) -> ShardRecord {
        ShardRecord {
            id,
            confidence,
            score,
            payload,
        }
    }

    // -----------------------------------------------------------------------
    // 1. route_write is deterministic
    // -----------------------------------------------------------------------

    #[test]
    fn test_route_write_deterministic() {
        let shard_map = make_shard_map_for("events", &[0, 1, 2, 3]);
        let coord = DistQueryCoordinator::new(Arc::clone(&shard_map), DistQueryConfig::default());

        let record_id = Uuid::new_v4();
        let shard_a = coord.route_write("events", &record_id);
        let shard_b = coord.route_write("events", &record_id);
        let shard_c = coord.route_write("events", &record_id);

        assert_eq!(shard_a, shard_b, "same record must always map to same shard");
        assert_eq!(shard_b, shard_c, "routing must be deterministic across calls");
    }

    // -----------------------------------------------------------------------
    // 2. Scan with a single shard returns that shard's records
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_scan_single_shard() {
        // Build a map with one node/shard and route records so the collection
        // is registered on that shard.
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(42, node_addr(9042));
        // Register the collection on the shard.
        let _shard = raw_map.shard_for("users", &Uuid::from_u128(1));
        let shard_map = Arc::new(raw_map);

        // The shard ID is whatever shard_for returned; seed mock with that ID.
        let shard_ids = shard_map.shards_for_collection("users");
        assert!(!shard_ids.is_empty());
        let shard_id = shard_ids[0];

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let mut exec = MockShardExecutor::new();
        exec.seed_shard(
            shard_id,
            vec![make_record(id1, 0.9, 1.0), make_record(id2, 0.7, 0.5)],
        );

        let coord = DistQueryCoordinator::new_with_mock(
            shard_map,
            DistQueryConfig::default(),
            Arc::new(exec),
        );

        let result = coord
            .execute("users", DistQuery::Scan { filter: None, limit: None })
            .await
            .expect("scan must succeed");

        assert_eq!(result.records.len(), 2);
        assert_eq!(result.shards_queried, 1);
        assert_eq!(result.shards_failed, 0);
    }

    // -----------------------------------------------------------------------
    // 3. Scan across 2 shards: merged result sorted by confidence DESC
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_scan_merge_sorted() {
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(0, node_addr(9000));
        raw_map.add_node(1, node_addr(9001));

        // Pre-populate shards for the collection by routing test records.
        // With 1 vnode each, UUIDs 0 and 1 will typically land on different shards.
        let mut shard_set = std::collections::HashSet::new();
        let mut counter = 0u128;
        while shard_set.len() < 2 {
            shard_set.insert(raw_map.shard_for("articles", &Uuid::from_u128(counter)));
            counter += 1;
        }
        let shard_ids: Vec<ShardId> = {
            let mut v: Vec<_> = shard_set.into_iter().collect();
            v.sort_unstable();
            v
        };
        let shard_map = Arc::new(raw_map);

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();
        let id_d = Uuid::new_v4();

        let mut exec = MockShardExecutor::new();
        exec.seed_shard(
            shard_ids[0],
            vec![make_record(id_a, 0.5, 0.0), make_record(id_b, 0.9, 0.0)],
        );
        exec.seed_shard(
            shard_ids[1],
            vec![make_record(id_c, 0.3, 0.0), make_record(id_d, 0.7, 0.0)],
        );

        let coord = DistQueryCoordinator::new_with_mock(
            shard_map,
            DistQueryConfig::default(),
            Arc::new(exec),
        );

        let result = coord
            .execute("articles", DistQuery::Scan { filter: None, limit: None })
            .await
            .expect("scan must succeed");

        assert_eq!(result.records.len(), 4);

        // Verify descending confidence order.
        let confidences: Vec<f64> = result.records.iter().map(|r| r.confidence).collect();
        let mut sorted = confidences.clone();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
        assert_eq!(
            confidences, sorted,
            "records must be sorted by confidence DESC"
        );

        // The highest-confidence record (0.9) must come first.
        assert_eq!(result.records[0].id, id_b);
    }

    // -----------------------------------------------------------------------
    // 4. VectorScan with oversample: 2 shards × (top_k × oversample) records,
    //    trimmed to top_k globally.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_vector_scan_oversample() {
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(0, node_addr(9000));
        raw_map.add_node(1, node_addr(9001));

        let mut shard_set = std::collections::HashSet::new();
        let mut counter = 0u128;
        while shard_set.len() < 2 {
            shard_set.insert(raw_map.shard_for("embeddings", &Uuid::from_u128(counter)));
            counter += 1;
        }
        let shard_ids: Vec<ShardId> = {
            let mut v: Vec<_> = shard_set.into_iter().collect();
            v.sort_unstable();
            v
        };
        let shard_map = Arc::new(raw_map);

        // 15 records per shard; oversample_factor=3, top_k=5 means the
        // coordinator asks each shard for 15 records → up to 30 total, then
        // trims to 5.
        let mut exec = MockShardExecutor::new();
        for (i, &shard) in shard_ids.iter().enumerate() {
            let records: Vec<ShardRecord> = (0..15)
                .map(|j| {
                    let payload = vec![((i as u8 * 15 + j) % 255), 0, 0, 0];
                    make_record_with_payload(Uuid::new_v4(), 0.8, 0.0, payload)
                })
                .collect();
            exec.seed_shard(shard, records);
        }

        let config = DistQueryConfig {
            oversample_factor: 3,
            timeout_ms: 5000,
            max_concurrent_shards: 16,
        };

        let coord = DistQueryCoordinator::new_with_mock(shard_map, config, Arc::new(exec));

        let result = coord
            .execute(
                "embeddings",
                DistQuery::VectorScan {
                    query_vector: vec![1.0, 0.0, 0.0, 0.0],
                    top_k: 5,
                    index_name: "content".into(),
                },
            )
            .await
            .expect("vector scan must succeed");

        assert_eq!(
            result.records.len(),
            5,
            "final result must be trimmed to top_k=5"
        );
        assert_eq!(result.shards_queried, 2);
    }

    // -----------------------------------------------------------------------
    // 5. PointLookup returns the record from the correct shard
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_point_lookup_routing() {
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(0, node_addr(9000));
        raw_map.add_node(1, node_addr(9001));
        raw_map.add_node(2, node_addr(9002));

        // Ensure all 3 shards are registered for "logs".
        let mut shard_set = std::collections::HashSet::new();
        let mut counter = 0u128;
        while shard_set.len() < 3 {
            shard_set.insert(raw_map.shard_for("logs", &Uuid::from_u128(counter)));
            counter += 1;
        }
        let shard_map = Arc::new(raw_map);

        let target_id = Uuid::new_v4();
        let decoy_id = Uuid::new_v4();

        // Determine which shard route_write will use for target_id.
        let coord_tmp =
            DistQueryCoordinator::new(Arc::clone(&shard_map), DistQueryConfig::default());
        let target_shard = coord_tmp.route_write("logs", &target_id);

        let all_shards = shard_map.shards_for_collection("logs");
        let mut exec = MockShardExecutor::new();
        for &shard in &all_shards {
            let mut records = vec![make_record(decoy_id, 0.5, 0.5)];
            if shard == target_shard {
                records.push(make_record(target_id, 0.95, 0.9));
            }
            exec.seed_shard(shard, records);
        }

        let coord = DistQueryCoordinator::new_with_mock(
            shard_map,
            DistQueryConfig::default(),
            Arc::new(exec),
        );

        let result = coord
            .execute("logs", DistQuery::PointLookup { record_id: target_id })
            .await
            .expect("point lookup must succeed");

        assert_eq!(result.records.len(), 1);
        assert_eq!(
            result.records[0].id, target_id,
            "must return the target record, not a decoy"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Aggregate Count: 2 shards × 10 records = 20
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_aggregate_count() {
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(0, node_addr(9000));
        raw_map.add_node(1, node_addr(9001));

        let mut shard_set = std::collections::HashSet::new();
        let mut counter = 0u128;
        while shard_set.len() < 2 {
            shard_set.insert(raw_map.shard_for("metrics", &Uuid::from_u128(counter)));
            counter += 1;
        }
        let shard_ids: Vec<ShardId> = {
            let mut v: Vec<_> = shard_set.into_iter().collect();
            v.sort_unstable();
            v
        };
        let shard_map = Arc::new(raw_map);

        let mut exec = MockShardExecutor::new();
        for &shard in &shard_ids {
            let records: Vec<ShardRecord> =
                (0..10).map(|_| make_record(Uuid::new_v4(), 0.8, 0.5)).collect();
            exec.seed_shard(shard, records);
        }

        let coord = DistQueryCoordinator::new_with_mock(
            shard_map,
            DistQueryConfig::default(),
            Arc::new(exec),
        );

        let result = coord
            .execute(
                "metrics",
                DistQuery::Aggregate {
                    op: AggOp::Count,
                    field: "value".into(),
                    filter: None,
                },
            )
            .await
            .expect("aggregate count must succeed");

        assert_eq!(
            result.total_count,
            Some(20),
            "count must be sum of all shard counts"
        );
        assert!(result.records.is_empty());
    }

    // -----------------------------------------------------------------------
    // 7. Aggregate Avg: partial sums correctly combined
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_aggregate_avg() {
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(0, node_addr(9000));
        raw_map.add_node(1, node_addr(9001));

        let mut shard_set = std::collections::HashSet::new();
        let mut counter = 0u128;
        while shard_set.len() < 2 {
            shard_set.insert(raw_map.shard_for("stats", &Uuid::from_u128(counter)));
            counter += 1;
        }
        let shard_ids: Vec<ShardId> = {
            let mut v: Vec<_> = shard_set.into_iter().collect();
            v.sort_unstable();
            v
        };
        let shard_map = Arc::new(raw_map);

        // Shard 0: 4 records each with confidence 0.5  → sum=2.0, count=4
        // Shard 1: 6 records each with confidence 1.0  → sum=6.0, count=6
        // Global avg = (2.0 + 6.0) / (4 + 6) = 0.8
        let mut exec = MockShardExecutor::new();
        exec.seed_shard(
            shard_ids[0],
            (0..4).map(|_| make_record(Uuid::new_v4(), 0.5, 0.0)).collect(),
        );
        exec.seed_shard(
            shard_ids[1],
            (0..6).map(|_| make_record(Uuid::new_v4(), 1.0, 0.0)).collect(),
        );

        let coord = DistQueryCoordinator::new_with_mock(
            shard_map,
            DistQueryConfig::default(),
            Arc::new(exec),
        );

        let result = coord
            .execute(
                "stats",
                DistQuery::Aggregate {
                    op: AggOp::Avg,
                    field: "confidence".into(),
                    filter: None,
                },
            )
            .await
            .expect("aggregate avg must succeed");

        let avg = result
            .aggregate_value
            .expect("aggregate_value must be set");
        assert!(
            (avg - 0.8).abs() < 1e-9,
            "expected avg=0.8, got {avg}"
        );
    }

    // -----------------------------------------------------------------------
    // 8. No shards → NoShards error
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_no_shards_error() {
        // A shard map that has no records routed for "unknown_collection".
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(0, node_addr(9000));
        // Route only to "other_collection", not to the one we'll query.
        raw_map.shard_for("other_collection", &Uuid::from_u128(1));
        let shard_map = Arc::new(raw_map);
        let exec = Arc::new(MockShardExecutor::new());

        let coord = DistQueryCoordinator::new_with_mock(
            shard_map,
            DistQueryConfig::default(),
            exec,
        );

        let err = coord
            .execute(
                "unknown_collection",
                DistQuery::Scan {
                    filter: None,
                    limit: None,
                },
            )
            .await
            .expect_err("should return NoShards error");

        assert!(
            matches!(err, DistQueryError::NoShards),
            "expected NoShards, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // 9. OQ-9: confidence min propagation — floor does not raise confidence
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_confidence_min_propagation() {
        // Default shard confidence floor = 1.0.
        // A record with confidence 0.4 → min(0.4, 1.0) = 0.4 (unchanged).
        let mut raw_map = ShardMap::new(1);
        raw_map.add_node(0, node_addr(9000));
        let shard_id = raw_map.shard_for("knowledge", &Uuid::from_u128(0));
        let shard_map = Arc::new(raw_map);

        let low_conf_id = Uuid::new_v4();
        let mut exec = MockShardExecutor::new();
        exec.seed_shard(shard_id, vec![make_record(low_conf_id, 0.4, 0.0)]);

        let coord = DistQueryCoordinator::new_with_mock(
            shard_map,
            DistQueryConfig::default(),
            Arc::new(exec),
        );

        let result = coord
            .execute("knowledge", DistQuery::Scan { filter: None, limit: None })
            .await
            .expect("scan must succeed");

        let record = result
            .records
            .iter()
            .find(|r| r.id == low_conf_id)
            .expect("low-confidence record must be in result");

        assert!(
            (record.confidence - 0.4).abs() < 1e-9,
            "confidence must not be raised above its original value; got {}",
            record.confidence
        );
    }

    // -----------------------------------------------------------------------
    // 10. DistQueryConfig::default() values match spec
    // -----------------------------------------------------------------------

    #[test]
    fn test_dist_query_config_default() {
        let cfg = DistQueryConfig::default();
        assert_eq!(cfg.oversample_factor, 3, "oversample_factor must default to 3");
        assert_eq!(cfg.timeout_ms, 5000, "timeout_ms must default to 5000");
        assert_eq!(
            cfg.max_concurrent_shards, 16,
            "max_concurrent_shards must default to 16"
        );
    }
}
