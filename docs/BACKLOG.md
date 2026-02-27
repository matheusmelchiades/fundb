# FunDB — Product Backlog

**Role:** Product Owner
**Method:** Epic → Story → Task. Every story is independently assignable to an agent.
**Priority:** P0 = must ship, P1 = core value, P2 = differentiator, P3 = future

**Story format:**
```
STORY-[EPIC]-[N]
As [who], I want [what], so that [why].
Size: S (< 1 day) | M (1-2 days) | L (3-5 days) | XL (> 5 days)
Agent: who implements it
Depends on: stories that must be done first
Interface contract: what this story exposes to other stories
DoD: acceptance criteria (checkbox list)
```

---

## Dependency Map

```
EPIC-1 (Foundation)
  ├── STORY-1-1 (FunRecord) ──────────────────────────────┐
  ├── STORY-1-2 (Codec)  ←── needs STORY-1-1              │
  ├── STORY-1-3 (FunQL Lexer) ─────────────────────────── │ ─┐
  └── STORY-1-4 (PG Wire skeleton) ───────────────────── ─│──│─┐
                                                           │  │ │
EPIC-2 (Storage) ←── needs STORY-1-1, STORY-1-2          │  │ │
  ├── STORY-2-1 (MemTable)  ─┐                            │  │ │
  ├── STORY-2-2 (WAL)        ├── all parallel             │  │ │
  ├── STORY-2-3 (SSTable)  ──┘                            │  │ │
  └── STORY-2-4 (MVCC)                                    │  │ │
                                                           │  │ │
EPIC-3 (Indexes) ←── needs EPIC-2                         │  │ │
  ├── STORY-3-1 (B+Tree)  ──────┐                         │  │ │
  ├── STORY-3-2 (HNSW)  ────── ─┤ all parallel            │  │ │
  ├── STORY-3-3 (Graph SPO) ────┤                         │  │ │
  ├── STORY-3-4 (Temporal) ─────┤                         │  │ │
  └── STORY-3-5 (Causal DAG) ───┘                         │  │ │
                                                           │  │ │
EPIC-4 (Query Engine) ←── needs EPIC-3 + STORY-1-3        │  │ │
  ├── STORY-4-1 (Parser + Binder) ←── STORY-1-3 ──────────┘  │ │
  ├── STORY-4-2 (Optimizer) ←── STORY-4-1                    │ │
  ├── STORY-4-3 (Executor) ←── STORY-4-1                     │ │
  └── STORY-4-4 (LSM Controller) (parallel with 4-2, 4-3)    │ │
                                                              │ │
EPIC-5 (Cognitive) ←── needs EPIC-4                          │ │
  ├── STORY-5-1 (Confidence Propagation)  ─┐                  │ │
  ├── STORY-5-2 (WITHIN CONTEXT / MMR) ───┤ all parallel      │ │
  ├── STORY-5-3 (Contradiction Detection) ─┤                  │ │
  ├── STORY-5-4 (Causal Tier 1)  ──────── ┤                   │ │
  └── STORY-5-5 (Agent Memory) ───────── ─┘                   │ │
                                                               │ │
EPIC-6 (Intelligence) ←── needs EPIC-5                        │ │
  ├── STORY-6-1 (Semantic Interface)  ─┐                       │ │
  ├── STORY-6-2 (Causal Tier 2)  ─────┤ parallel              │ │
  ├── STORY-6-3 (LTR)  ─────────────  ┤                       │ │
  └── STORY-6-4 (Cost Optimizer) ────  ┘                       │ │
                                                                │ │
EPIC-7 (SDK) ←── needs EPIC-4 working (single-node)            │ │
  ├── STORY-7-1 (PG Wire full) ←── STORY-1-4 ─────────────────┘ │
  ├── STORY-7-2 (gRPC + REST) ─┐                                  │
  ├── STORY-7-3 (Python SDK) ──┤ parallel after STORY-7-1         │
  └── STORY-7-4 (Go SDK)  ─────┘                                  │
                                                                   │
EPIC-8 (Distribution) ←── needs EPIC-4 (single-node working)      │
  ├── STORY-8-1 (Raft) ─────────┐                                  │
  ├── STORY-8-2 (Shard Manager) ┤ 8-1 before 8-2, 8-2 before 8-3  │
  └── STORY-8-3 (Dist Query) ───┘                                  │
                                                                    │
EPIC-9 (Causal Tier 3) ←── needs EPIC-6                           │
  └── STORY-9-1 (SCM + Do-Calculus + Counterfactuals)              │
                                                                    │
EPIC-10 (Production) ←── needs all                                 │
  ├── STORY-10-1 (Security)  ─────────────────────────────────────┘
  ├── STORY-10-2 (Observability) ─┐ parallel
  └── STORY-10-3 (Benchmarks)  ───┘
```

---

## EPIC-1 — Foundation
**Value:** The skeleton compiles. Engineers can start building on top of it.

---

### STORY-1-1 — FunRecord: The Unit of Knowledge
**P0 | Size: M | Agent: foundation-agent-A**

> As a **database engine developer**, I want a single Rust struct that represents all data types (document, vector, graph, time-series, causal) with cognitive metadata, so that every other module has a stable, complete data model to build on.

**Depends on:** nothing

**Interface contract (what this story MUST expose before any other story starts):**
```rust
// crates/fundb-core/src/lib.rs
pub struct FunRecord { ... }           // all fields per ARCHITECTURE.md §4.3
pub struct Edge { ... }
pub struct CausalEdge { ... }          // with OQ resolutions: origin, confidence, stability, direction
pub struct Source { ... }
pub struct Sample { ... }
pub enum CausalType { Caused, Influenced, Correlated, Preceded }
pub enum CausalOrigin { UserDeclared, Granger { .. }, TemporalPrecedence { .. }, LlmValidated { .. } }
pub enum StabilityStatus { Stable, Unstable, Provisional, NotApplicable }
pub enum DirectionStatus { Confirmed, DirectionUncertain }
pub fn new_record_id() -> Uuid;        // UUID v7
pub struct FunRecordBuilder;           // fluent builder
```

**DoD:**
- [ ] All fields from ARCHITECTURE.md §4.3 are present (verify with grep against spec)
- [ ] `FunRecordBuilder` builds valid records
- [ ] UUID v7: 1000 IDs in sequence → sorted chronologically
- [ ] No unsafe code
- [ ] 100% field coverage in unit tests

---

### STORY-1-2 — Codec: Binary Encoding of FunRecord
**P0 | Size: M | Agent: foundation-agent-B**

> As a **storage engine**, I want to serialize any FunRecord to bytes and deserialize back to an identical struct, so that records can be persisted to disk and transmitted over the network.

**Depends on:** STORY-1-1 (FunRecord struct finalized)

**Interface contract:**
```rust
// crates/fundb-core/src/codec.rs
pub trait Encode { fn encode(&self) -> Result<Bytes>; }
pub trait Decode: Sized { fn decode(bytes: &[u8]) -> Result<Self>; }
impl Encode for FunRecord { ... }
impl Decode for FunRecord { ... }

pub struct PageEncoder;               // columnar layout for SSTable pages
pub struct PageDecoder;
pub struct PageHeader {               // per ARCHITECTURE.md §4.1 column page groups
    pub min_key: Bytes,
    pub max_key: Bytes,
    pub row_count: u32,
    pub bloom_filter: BloomFilter,
    pub confidence_histogram: [u32; 100],   // per OQ-9
}
pub fn estimate_tokens(record: &FunRecord) -> u32;  // per OQ-5 resolution
```

**DoD:**
- [ ] Property-based test: encode(r) → decode → encode produces identical bytes (10K random FunRecords)
- [ ] `estimate_tokens` uses formula `max(byte_length/4, vector_dims*6 + scalar_fields*3)` per OQ-5
- [ ] Columnar page: confidence column stored separately from document payload
- [ ] Zstd compression wrapper works for documents
- [ ] All tests pass in < 500ms

---

### STORY-1-3 — FunQL Lexer + Parser Skeleton
**P0 | Size: L | Agent: foundation-agent-C**

> As a **query engine developer**, I want a parser that turns FunQL text into a typed AST, so that all query constructs from the spec can be represented in memory and processed by the optimizer and executor.

**Depends on:** nothing (independent of data model)

**Interface contract:**
```rust
// crates/fundb-sql/src/lib.rs
pub fn parse(input: &str) -> Result<Statement, ParseError>;

pub enum Statement {
    Select(SelectStmt), Insert(InsertStmt), Update(UpdateStmt), Delete(DeleteStmt),
    CreateCollection(CreateCollectionStmt), CreateIndex(CreateIndexStmt),
    CreateCausalModel(CreateCausalModelStmt),
    Understand(UnderstandStmt),
    TraceCausality(TraceCausalityStmt),
    EstimateEffect(EstimateEffectStmt),
    Counterfactual(CounterfactualStmt),
    DiscoverCausal(DiscoverCausalStmt),
    Remember(RememberStmt), RecallBy(RecallByStmt), Forget(ForgetStmt),
}

pub struct ParseError { pub line: u32, pub column: u32, pub message: String }
```

**DoD:**
- [ ] All example queries in ARCHITECTURE.md §5.1 parse without error
- [ ] Invalid SQL returns `ParseError` with correct line + column
- [ ] `UNDERSTAND "..."`, `TRACE CAUSALITY`, `ESTIMATE EFFECT`, `WITHIN CONTEXT` parse correctly
- [ ] `REMEMBER`, `RECALL BY`, `FORGET` parse correctly
- [ ] Fuzzing: 10K random strings → no panics (only errors)

---

### STORY-1-4 — PostgreSQL Wire Protocol: TCP Handshake
**P0 | Size: S | Agent: foundation-agent-D**

> As a **developer**, I want `psql` and any PostgreSQL client to connect to FunDB, so that we have zero-friction access from day one without requiring a new client.

**Depends on:** nothing

**Interface contract:**
```rust
// crates/fundb-protocol/src/lib.rs
pub struct Server;
impl Server {
    pub async fn bind(addr: SocketAddr) -> Result<Self>;
    pub async fn run(self, handler: Arc<dyn QueryHandler>) -> Result<()>;
}
pub trait QueryHandler: Send + Sync {
    async fn execute(&self, query: &str, ctx: &ConnContext) -> Result<QueryResult>;
}
pub struct QueryResult { pub columns: Vec<Column>, pub rows: Vec<Row> }
pub struct ConnContext { pub tenant_id: u32, pub agent_id: Option<String> }
```

**DoD:**
- [ ] `psql -h localhost -p 5433 -U fun` connects and receives `ReadyForQuery`
- [ ] Simple `SELECT 1` returns row with value `1`
- [ ] Connection refused after server shutdown (no zombie sockets)
- [ ] Handles 100 concurrent connections without panic

---

## EPIC-2 — Storage Engine
**Value:** Data survives crashes. The database is durable.

---

### STORY-2-1 — MemTable: In-Memory Write Buffer
**P0 | Size: M | Agent: storage-agent-A**

> As the **write path**, I want a concurrent in-memory structure that accepts writes from many goroutines simultaneously, so that FunDB can ingest data at high throughput before flushing to disk.

**Depends on:** STORY-1-1, STORY-1-2

**Interface contract:**
```rust
// crates/fundb-storage/src/memtable.rs
pub struct MemTable;
impl MemTable {
    pub fn insert(&self, key: RecordKey, record: FunRecord) -> Result<()>;
    pub fn get(&self, key: &RecordKey) -> Option<FunRecord>;
    pub fn range(&self, from: &RecordKey, to: &RecordKey) -> impl Iterator<Item=(RecordKey, FunRecord)>;
    pub fn size_bytes(&self) -> usize;
    pub fn freeze(self) -> ImmutableMemTable;
}
pub struct ImmutableMemTable;
impl ImmutableMemTable {
    pub fn iter(&self) -> impl Iterator<Item=(RecordKey, FunRecord)>;
}
```

**DoD:**
- [ ] 16 concurrent writers: zero data races (run with `--cfg loom` or ThreadSanitizer)
- [ ] `size_bytes()` accurate within 5% of actual memory usage
- [ ] `freeze()` produces immutable snapshot: subsequent writes go to new MemTable
- [ ] Iterator over frozen MemTable returns keys in sorted order

---

### STORY-2-2 — WAL: Crash-Safe Write Log
**P0 | Size: M | Agent: storage-agent-B**

> As the **durability layer**, I want every write appended to a sequential log before acknowledging success, so that no committed data is lost on process crash.

**Depends on:** STORY-1-2 (codec for record serialization)

**Interface contract:**
```rust
// crates/fundb-storage/src/wal.rs
pub struct Wal;
impl Wal {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn append(&mut self, entry: WalEntry) -> Result<Lsn>;  // fsync on commit entry
    pub fn recover(path: &Path) -> Result<impl Iterator<Item=WalEntry>>;
    pub fn checkpoint(&mut self, lsn: Lsn) -> Result<()>;
}
pub enum WalEntry { Write { txn_id: u64, record: FunRecord }, Delete { txn_id: u64, key: RecordKey }, TxnCommit(u64), TxnAbort(u64), Checkpoint(Lsn) }
pub type Lsn = u64;
```

**DoD:**
- [ ] Write 10K records → `kill -9` → restart → recover() replays all committed writes
- [ ] Truncate WAL at random byte: recovery stops at truncation point, no panic
- [ ] Partial writes (power loss simulation): CRC check skips corrupted entries
- [ ] `checkpoint()` allows truncation of entries before checkpoint LSN

---

### STORY-2-3 — SSTable: Sorted On-Disk Storage
**P0 | Size: L | Agent: storage-agent-C**

> As the **read path**, I want immutable, sorted files on disk with bloom filters, so that reads can skip irrelevant files without scanning them.

**Depends on:** STORY-1-2, STORY-2-2

**Interface contract:**
```rust
// crates/fundb-storage/src/sstable.rs
pub struct SstableWriter;
impl SstableWriter {
    pub fn new(path: &Path) -> Self;
    pub fn add(&mut self, key: RecordKey, record: FunRecord) -> Result<()>;  // must be called in key order
    pub fn finish(self) -> Result<SstableReader>;
}
pub struct SstableReader;
impl SstableReader {
    pub fn get(&self, key: &RecordKey) -> Result<Option<FunRecord>>;
    pub fn range(&self, from: &RecordKey, to: &RecordKey) -> impl Iterator<Item=Result<(RecordKey, FunRecord)>>;
    pub fn may_contain(&self, key: &RecordKey) -> bool;  // bloom filter check
}
pub fn merge_sstables(inputs: Vec<SstableReader>, output: &Path) -> Result<SstableReader>;
```

**DoD:**
- [ ] Write 100K records → read all back → identical to written (no data loss or corruption)
- [ ] Bloom filter false positive rate < 1% for 100K keys
- [ ] `merge_sstables` correctly handles tombstones (deleted records)
- [ ] Block cache (LRU 32MB default) reduces repeated read latency by ≥ 50%

---

### STORY-2-4 — MVCC: Bitemporal Version Control
**P0 | Size: L | Agent: storage-agent-D**

> As an **AI agent**, I want to query "what did the database know at time T?", so that I can reproduce past states for model training and audit causal chains historically.

**Depends on:** STORY-2-1, STORY-2-2, STORY-2-3

**Interface contract:**
```rust
// crates/fundb-storage/src/mvcc.rs
pub struct MvccStore;
impl MvccStore {
    pub fn write(&self, txn: &Transaction, record: FunRecord) -> Result<()>;
    pub fn read_as_of_system(&self, key: &RecordKey, system_ts: Timestamp) -> Result<Option<FunRecord>>;
    pub fn read_as_of_valid(&self, key: &RecordKey, valid_from: Timestamp, valid_to: Timestamp) -> Result<Vec<FunRecord>>;
    pub fn begin_txn(&self) -> Transaction;
    pub fn commit(&self, txn: Transaction) -> Result<()>;
}
pub struct Transaction { pub txn_id: u64, pub snapshot_ts: Timestamp }
```

**DoD:**
- [ ] Update record 5 times → `read_as_of_system(t)` at each timestamp returns correct version
- [ ] `read_as_of_valid(T1, T2)` returns only records with overlapping valid_time window
- [ ] Snapshot isolation: reader in txn X does NOT see writes committed after X started
- [ ] GC: versions older than retention window are cleaned up (configurable, default 30 days)

---

## EPIC-3 — Index Engines
**Value:** Queries are fast. Not every query scans everything.

All 5 stories in this epic are **fully parallel** — they share no code.

---

### STORY-3-1 — B+Tree: Scalar Field Index
**P0 | Size: M | Agent: index-agent-A**

> As the **query planner**, I want a B+Tree index over scalar fields (integers, strings, timestamps), so that equality and range queries skip full collection scans.

**Depends on:** EPIC-2 complete

**Interface contract:**
```rust
// crates/fundb-indexes/src/btree.rs
pub struct BTree<K: Ord + Encode, V: Encode + Decode>;
impl<K, V> BTree<K, V> {
    pub fn insert(&mut self, key: K, value: V) -> Result<()>;
    pub fn get(&self, key: &K) -> Result<Option<V>>;
    pub fn range(&self, from: Bound<&K>, to: Bound<&K>) -> impl Iterator<Item=Result<(K, V)>>;
    pub fn delete(&mut self, key: &K) -> Result<bool>;
}
```

**DoD:**
- [ ] Property-based: insert 100K random keys → sorted range scan returns all in order (100% pass)
- [ ] Concurrent reads: 8 readers + 1 writer → no deadlock in 10K operations
- [ ] Prefix scan for string keys: `range("fun".., "fun\xFF")` returns all keys starting with "fun"

---

### STORY-3-2 — HNSW: Vector Similarity Index
**P0 | Size: XL | Agent: index-agent-B**

> As an **AI agent doing RAG**, I want approximate nearest neighbor search over millions of embeddings in under 5ms, so that semantic retrieval is fast enough for real-time applications.

**Depends on:** EPIC-2 complete

**Interface contract:**
```rust
// crates/fundb-indexes/src/hnsw.rs
pub struct HnswIndex;
impl HnswIndex {
    pub fn new(dims: usize, m: usize, ef_construction: usize) -> Self;
    pub fn insert(&mut self, id: Uuid, vector: &[f32]) -> Result<()>;
    pub fn search(&self, query: &[f32], k: usize, recall_target: f32) -> Vec<(Uuid, f32)>;  // adaptive ef_search per OQ-4
    pub fn delete(&mut self, id: Uuid) -> Result<()>;  // lazy tombstone
    pub fn len(&self) -> usize;
}
pub struct PqEncoder { pub num_subspaces: usize }
impl PqEncoder {
    pub fn train(&mut self, vectors: &[Vec<f32>]) -> Result<()>;
    pub fn encode(&self, vector: &[f32]) -> Vec<u8>;
    pub fn approximate_distance(&self, encoded_a: &[u8], encoded_b: &[u8]) -> f32;
}
```

**DoD:**
- [ ] 100K 768-dim vectors: `search(q, 10, 0.95)` achieves recall@10 ≥ 0.95 vs brute-force
- [ ] Adaptive ef_search: recall never drops below `recall_target` for any vector distribution
- [ ] PQ64: 768-dim f32 vector (3KB) → encoded ≤ 100 bytes
- [ ] Delete + compact: no deleted nodes returned in search results after compaction
- [ ] Disk-resident lower layers: index larger than RAM fits on disk via mmap

---

### STORY-3-3 — Graph SPO: Edge Traversal Index
**P0 | Size: M | Agent: index-agent-C**

> As an **AI agent building a knowledge graph**, I want to traverse relationships between entities in multi-hop paths, so that I can answer "who influenced whom" and "what is connected to what" queries.

**Depends on:** EPIC-2 complete

**Interface contract:**
```rust
// crates/fundb-indexes/src/graph.rs
pub struct GraphIndex;
impl GraphIndex {
    pub fn insert_edge(&mut self, s: Uuid, predicate: &str, o: Uuid, confidence: f32, props: Vec<u8>) -> Result<()>;
    pub fn delete_edge(&mut self, s: Uuid, predicate: &str, o: Uuid) -> Result<()>;
    pub fn outgoing(&self, subject: Uuid, predicate: Option<&str>) -> Vec<Edge>;
    pub fn incoming(&self, object: Uuid, predicate: Option<&str>) -> Vec<Edge>;
    pub fn bfs(&self, start: Uuid, predicate: &str, max_depth: u32) -> Vec<(Uuid, u32, f32)>;  // (node, depth, path_strength)
    pub fn path(&self, from: Uuid, to: Uuid, max_depth: u32) -> Vec<Vec<Uuid>>;  // all paths
}
```

**DoD:**
- [ ] 10K nodes, 50K edges: 3-hop BFS completes < 50ms
- [ ] `path(a, b, 5)` returns all paths, each verified correct by manual inspection of test graph
- [ ] `path_strength = Π(edge_confidences)` matches manual calculation
- [ ] Cycle in test graph: BFS terminates (visited set prevents infinite loop)

---

### STORY-3-4 — Temporal Interval Index
**P0 | Size: M | Agent: index-agent-D**

> As an **ML engineer**, I want to query which records were "true" during a specific time window, so that I can reconstruct exact training datasets for model reproducibility.

**Depends on:** EPIC-2 complete

**Interface contract:**
```rust
// crates/fundb-indexes/src/temporal.rs
pub struct TemporalIndex;
impl TemporalIndex {
    pub fn insert(&mut self, id: Uuid, valid_from: Timestamp, valid_to: Timestamp) -> Result<()>;
    pub fn point_query(&self, at: Timestamp) -> Vec<Uuid>;          // records valid at T
    pub fn range_query(&self, from: Timestamp, to: Timestamp) -> Vec<Uuid>;  // overlapping
    pub fn delete(&mut self, id: Uuid) -> Result<()>;
}
```

**DoD:**
- [ ] 100K random intervals: `point_query(T)` returns correct set (verified vs brute-force)
- [ ] `range_query(T1, T2)` returns all intervals with any overlap with [T1, T2]
- [ ] Latency: point query on 100K intervals < 5ms

---

### STORY-3-5 — Causal DAG Index + Confidence Histogram
**P0 | Size: L | Agent: index-agent-E**

> As the **causal reasoning engine**, I want a DAG index that enforces acyclicity and supports path queries with strength propagation, so that `TRACE CAUSALITY FROM A TO B` works correctly and efficiently.

**Depends on:** EPIC-2 complete, STORY-1-1 (CausalEdge type)

**Interface contract:**
```rust
// crates/fundb-indexes/src/causal.rs
pub struct CausalDagIndex;
impl CausalDagIndex {
    pub fn insert_edge(&mut self, edge: &CausalEdge) -> Result<(), CausalError>;
    pub fn paths(&self, from: Uuid, to: Uuid, max_depth: u32, min_strength: f32) -> Vec<CausalPath>;
    pub fn effects(&self, from: Uuid, max_depth: u32) -> Vec<(Uuid, f32)>;  // (node, cumulative_strength)
    pub fn causes(&self, of: Uuid, max_depth: u32) -> Vec<(Uuid, f32)>;
    pub fn materialize_closure(&mut self, from: Uuid, to: Uuid);  // cache hot path
}
pub struct CausalPath { pub nodes: Vec<Uuid>, pub edges: Vec<CausalEdge>, pub total_strength: f32 }
pub enum CausalError { Cycle { detected_at: Uuid }, InvalidStrength }

// crates/fundb-indexes/src/confidence.rs
pub struct ConfidenceIndex;
impl ConfidenceIndex {
    pub fn insert(&mut self, id: Uuid, confidence: f32) -> Result<()>;
    pub fn range(&self, min: f32, max: f32) -> Vec<Uuid>;
    pub fn histogram(&self) -> [u32; 100];  // for page header pruning
    pub fn update(&mut self, id: Uuid, new_confidence: f32) -> Result<()>;
}
```

**DoD:**
- [ ] `insert_edge` that creates a cycle → returns `CausalError::Cycle`
- [ ] Path strength = product of all edge strengths (numerically verified)
- [ ] Transitive closure cache: querying same (from, to) pair twice → second query hits cache
- [ ] `confidence.range(0.7, 1.0)` returns only records with confidence ≥ 0.7

---

## EPIC-4 — Query Engine
**Value:** A developer can run a SELECT and get results. The system is usable.

---

### STORY-4-1 — FunQL Parser: Complete + Binder
**P0 | Size: L | Agent: query-agent-A**

> As the **optimizer**, I want a complete, typed logical plan from any valid FunQL statement, so that I can apply optimizations without parsing concerns.

**Depends on:** STORY-1-3 (parser skeleton), EPIC-3 complete (for catalog-aware binding)

**Interface contract:**
```rust
// crates/fundb-sql/src/binder.rs
pub fn bind(stmt: Statement, catalog: &Catalog) -> Result<LogicalPlan, BindError>;

pub enum LogicalPlan {
    Scan { collection: String, predicate: Option<Expr>, projections: Vec<Expr> },
    VectorScan { collection: String, vector_field: String, query: Vec<f32>, threshold: f32 },
    GraphTraverse { from: Expr, predicate: String, depth: RangeInclusive<u32> },
    CausalTrace { from: Expr, to: Expr, max_depth: u32, min_strength: f32, min_stability: Option<f32> },
    Filter { input: Box<LogicalPlan>, predicate: Expr },
    Project { input: Box<LogicalPlan>, exprs: Vec<Expr> },
    Join { left: Box<LogicalPlan>, right: Box<LogicalPlan>, condition: Expr },
    Aggregate { input: Box<LogicalPlan>, group_by: Vec<Expr>, aggregates: Vec<AggExpr> },
    Sort { input: Box<LogicalPlan>, order_by: Vec<SortExpr> },
    Limit { input: Box<LogicalPlan>, n: usize },
    ContextOptimize { input: Box<LogicalPlan>, options: ContextOptions },
    Understand { intent: String, options: UnderstandOptions },   // Semantic Interface entry
    EstimateEffect { model: String, set_vars: HashMap<String,f64>, predict: String },
    Counterfactual { model: String, observed: HashMap<String,f64>, had: HashMap<String,f64>, predict: String },
}
```

**DoD:**
- [ ] All ARCHITECTURE.md §5.1 example queries produce valid `LogicalPlan` (no bind errors)
- [ ] Type mismatch (e.g., `_confidence > "string"`) returns `BindError` with clear message
- [ ] Unresolved collection name returns `BindError`
- [ ] `WITHIN CONTEXT (...)` parameters parsed into `ContextOptions` correctly

---

### STORY-4-2 — Rule-Based Optimizer
**P0 | Size: M | Agent: query-agent-B**

> As the **executor**, I want the optimal physical query plan delivered to me, so that I don't run full collection scans when a 10ms index lookup would do.

**Depends on:** STORY-4-1

**Interface contract:**
```rust
// crates/fundb-optimizer/src/lib.rs
pub fn optimize_rules(plan: LogicalPlan, stats: &Statistics) -> LogicalPlan;
// returns LogicalPlan with rules applied (predicate pushdown, vector pre-filter, etc.)

pub fn to_physical(plan: LogicalPlan, catalog: &Catalog) -> PhysicalPlan;
// maps logical operators to physical implementations
```

**DoD:**
- [ ] `EXPLAIN` for `WHERE category='X' AND _vector <-> q < 0.5`: shows B+Tree scan before VectorANN
- [ ] `EXPLAIN` for `WHERE _confidence > 0.7`: shows ConfidenceFilter before any other scan
- [ ] `EXPLAIN` for `AS OF SYSTEM TIME T`: shows temporal filter pushed into scan operator
- [ ] All rules are idempotent: applying twice produces same plan as applying once

---

### STORY-4-3 — Volcano Executor + Physical Operators
**P0 | Size: XL | Agent: query-agent-C**

> As a **query**, I want to be executed by pulling batches of 1024 rows through a tree of operators until results are delivered, so that memory usage stays bounded and SIMD acceleration applies.

**Depends on:** STORY-4-1, STORY-4-2, EPIC-3

**Interface contract:**
```rust
// crates/fundb-executor/src/lib.rs
pub trait PhysicalOperator: Send {
    fn next_batch(&mut self) -> Result<Option<RecordBatch>>;
    fn schema(&self) -> Schema;
}
pub struct RecordBatch { pub rows: Vec<FunRecord>, pub count: usize }

pub fn execute(plan: PhysicalPlan, ctx: &QueryContext) -> impl Iterator<Item=Result<RecordBatch>>;
pub struct QueryContext { pub txn: Transaction, pub params: HashMap<String, Value>, pub agent_id: Option<String> }
```

**DoD:**
- [ ] All ARCHITECTURE.md §5.1 example queries execute and return correct results (manual verification on test dataset)
- [ ] `RecordBatch` size ≤ 1024 rows always
- [ ] SIMD: vector distance computation using AVX2 where available (3× speedup vs scalar, measured)
- [ ] No memory leak: run 10K queries with `valgrind` or ASAN — zero leaks

---

### STORY-4-4 — LSM Controller + Compaction
**P0 | Size: L | Agent: query-agent-D**

> As the **storage engine**, I want automatic background compaction that keeps read performance stable as data accumulates, so that query latency doesn't degrade over time.

**Depends on:** STORY-2-1, STORY-2-2, STORY-2-3

**Interface contract:**
```rust
// crates/fundb-storage/src/lsm.rs
pub struct LsmTree;
impl LsmTree {
    pub fn write(&self, record: FunRecord) -> Result<()>;
    pub fn get(&self, key: &RecordKey) -> Result<Option<FunRecord>>;
    pub fn scan(&self, from: &RecordKey, to: &RecordKey) -> impl Iterator<Item=Result<(RecordKey,FunRecord)>>;
    pub fn flush_memtable(&self) -> Result<()>;
    pub fn trigger_compaction(&self) -> Result<()>;  // normally background, but testable
}
```

**DoD:**
- [ ] Write 1M records → 10 compaction rounds → all 1M still readable
- [ ] L0 SSTable count never exceeds 8 (compaction triggers before that)
- [ ] Read latency at L5 < 2× read latency at L0 (compaction keeps levels balanced)

---

## EPIC-5 — Cognitive Modules
**Value:** FunDB stops being a database and becomes a knowledge system.

All 5 stories are **parallel** once EPIC-4 is done.

---

### STORY-5-1 — Confidence Propagation Engine
**P0 | Size: M | Agent: cognitive-agent-A**

> As an **AI agent making decisions**, I want query results to carry computed confidence scores that reflect the chain of evidence, so that I know how much to trust each piece of returned knowledge.

**Depends on:** STORY-4-3 (executor), STORY-3-5 (confidence index)

**Interface contract:**
```rust
// crates/fundb-cognitive/src/confidence.rs
pub struct ConfidencePropagator;
impl ConfidencePropagator {
    pub fn propagate_join(a: f32, b: f32) -> f32;        // min(a, b)
    pub fn propagate_union(a: f32, b: f32) -> f32;       // max(a, b)
    pub fn propagate_aggregate(scores: &[(f32, f32)]) -> f32;  // weighted avg, (confidence, weight)
    pub fn propagate_graph_path(nodes: &[f32], edges: &[f32]) -> f32;  // Π all
    pub fn propagate_negation(c: f32) -> f32;            // 1.0 - c
    pub fn confidence_for_n_equal_contradictions(n: u32) -> f32;  // 0.5 + 1/(2N)
}

// Operator wrapper that augments executor output with propagated confidence
pub struct ConfidencePropagatingOperator<O: PhysicalOperator>;
```

**DoD:**
- [ ] JOIN(0.9, 0.8) → 0.8 (min)
- [ ] Causal chain (0.9, 0.85, 0.7) → 0.535 ± 0.001 (product)
- [ ] `confidence_for_n_equal_contradictions(2)` → 0.75, n=3 → 0.667, n=10 → 0.55
- [ ] Property-based: `propagate_join` is commutative and associative
- [ ] Auto-increment on new corroborating source: `min(1.0, conf + 0.1)`
- [ ] Auto-decrement on contradiction: `max(0.0, conf - 0.2 * contra_conf)`

---

### STORY-5-2 — WITHIN CONTEXT: Token-Budget Retrieval
**P0 | Size: L | Agent: cognitive-agent-B**

> As an **LLM with a 4000-token context window**, I want the database to return the most useful, non-redundant set of results that fits in my window, so that I don't waste my context on duplicate or irrelevant information.

**Depends on:** STORY-4-3, STORY-3-2 (HNSW for similarity in MMR)

**Interface contract:**
```rust
// crates/fundb-cognitive/src/context.rs
pub struct ContextOptimizer;
impl ContextOptimizer {
    pub fn select(
        candidates: Vec<(FunRecord, f32)>,  // (record, relevance_score)
        options: ContextOptions,
    ) -> (Vec<FunRecord>, ContextMetadata);
}
pub struct ContextOptions {
    pub max_tokens: u32,
    pub coherence: f32,   // 0.0–1.0 min pairwise similarity of selected set
    pub diversity: f32,   // MMR lambda (0=max diversity, 1=max relevance)
    pub include_contradictions: bool,
    pub priority: Vec<SortKey>,
}
pub struct ContextMetadata {
    pub tokens_used: u32, pub tokens_budget: u32,
    pub coverage_score: f32, pub coherence_score: f32, pub diversity_score: f32,
    pub avg_confidence: f32, pub contradictions_found: u32,
    pub candidates_evaluated: u32, pub candidates_selected: u32,
}
```

**DoD:**
- [ ] `max_tokens: 4000` → `tokens_used ≤ 4400` (10% tolerance, per OQ-5 formula)
- [ ] `diversity: 0.0` → selected set pairwise similarity ≥ 0.9 (maximum relevance)
- [ ] `diversity: 1.0` → selected set pairwise similarity ≤ 0.5 (maximum diversity)
- [ ] `coherence: 0.8` → all returned records pairwise similarity ≥ 0.8 or set trimmed
- [ ] `include_contradictions: true` → at least 1 contradicting record included if exists
- [ ] `ContextMetadata` fields are all populated and accurate

---

### STORY-5-3 — Contradiction Detection
**P1 | Size: M | Agent: cognitive-agent-C**

> As a **knowledge base maintainer**, I want the system to automatically detect when a new fact contradicts an existing one, so that the agent querying it receives a trust signal rather than silently conflicting information.

**Depends on:** STORY-4-3, STORY-3-2 (for semantic similarity check)

**Interface contract:**
```rust
// crates/fundb-cognitive/src/contradiction.rs
pub struct ContradictionDetector;
impl ContradictionDetector {
    pub fn check_insert(new: &FunRecord, collection: &str, store: &LsmTree) -> Vec<ContradictionCandidate>;
    pub fn auto_link(record_a: &mut FunRecord, record_b: &mut FunRecord, strength: f32);
    pub fn resolve(record_id: Uuid, store: &mut LsmTree) -> Result<()>;
}
pub struct ContradictionCandidate { pub existing_id: Uuid, pub similarity: f32, pub polarity: Polarity }
pub enum Polarity { Conflicting, Corroborating }
```

**DoD:**
- [ ] Insert "sky is blue" + "sky is not blue" → auto-linked as contradictions
- [ ] Both records have confidence decremented by `0.2 × contradiction_confidence`
- [ ] `CONTRADICTION` event emitted (observers can subscribe)
- [ ] Corroborating detection: "water is H2O" + "water consists of hydrogen and oxygen" → linked as `_supports`

---

### STORY-5-4 — Causal Engine Tier 1: Explicit Causality
**P0 | Size: L | Agent: cognitive-agent-D**

> As an **AI agent doing incident analysis**, I want to declare causal relationships and query causal paths, so that I can answer "what caused this failure?" and "what will this change affect?"

**Depends on:** STORY-4-3, STORY-3-5 (CausalDagIndex)

**Interface contract:**
```rust
// crates/fundb-causal/src/tier1.rs
pub struct CausalEngine;
impl CausalEngine {
    pub fn insert_edge(&mut self, edge: CausalEdge) -> Result<(), CausalError>;  // cycle check
    pub fn trace(&self, from: Uuid, to: Uuid, opts: TraceOptions) -> Vec<CausalPath>;
    pub fn effects_of(&self, event: Uuid, max_depth: u32) -> Vec<(Uuid, f32)>;
    pub fn causes_of(&self, event: Uuid, max_depth: u32) -> Vec<(Uuid, f32)>;
    pub fn visualize(&self, paths: &[CausalPath], format: VisFormat) -> String;
}
pub struct TraceOptions { pub max_depth: u32, pub min_strength: f32, pub min_stability: Option<f32> }
pub enum VisFormat { Json, Mermaid, Dot }
```

**DoD:**
- [ ] `insert_edge(A→B); insert_edge(B→C); insert_edge(C→A)` → third insert returns `CycleError`
- [ ] `MODE EQUILIBRIUM` model with 3-variable simultaneous equations converges to correct equilibrium
- [ ] `trace(A, C, depth=5)` returns path A→B→C with strength = `strength_AB × strength_BC`
- [ ] `min_stability: Some(0.6)` filters out unstable edges from trace results
- [ ] `visualize(paths, Mermaid)` produces valid Mermaid diagram

---

### STORY-5-5 — Agent Memory Subsystem
**P0 | Size: L | Agent: cognitive-agent-E**

> As an **autonomous AI agent**, I want persistent memory that I can recall by a blend of semantic similarity, recency, and importance, so that I build up knowledge over sessions without forgetting everything on restart.

**Depends on:** STORY-4-3, STORY-3-2 (HNSW for semantic recall)

**Interface contract:**
```rust
// crates/fundb-cognitive/src/memory.rs
pub struct AgentMemory;
impl AgentMemory {
    pub fn remember(&mut self, agent_id: &str, content: &str, opts: RememberOptions) -> Result<Uuid>;
    pub fn recall(&self, agent_id: &str, query: &str, weights: RecallWeights, top_k: usize) -> Vec<MemoryResult>;
    pub fn forget(&mut self, memory_id: Uuid) -> Result<()>;
    pub fn consolidate(&mut self, agent_id: &str) -> Result<u32>;  // returns # memories merged
    pub fn decay_all(&mut self) -> Result<()>;  // background job
}
pub struct RecallWeights { pub semantic: f32, pub recency: f32, pub importance: f32 }  // must sum to 1.0
pub struct RememberOptions { pub importance: f32, pub memory_type: MemoryType, pub decay_rate: f32 }
pub struct MemoryResult { pub memory: FunRecord, pub score: f32, pub components: RecallComponents }
```

**DoD:**
- [ ] Store 1000 memories → `recall(q, {semantic:0.5, recency:0.3, importance:0.2}, top_k=20)` returns correct top-20 by weighted score (verified manually on test set)
- [ ] `consolidate`: two memories with semantic_sim > 0.9 → merged into one with higher importance
- [ ] `decay_all`: memories not accessed for N days have reduced confidence
- [ ] `forget`: memory with tombstone does NOT appear in future recalls

---

## EPIC-6 — Advanced Intelligence
**Value:** FunDB understands natural language and discovers causal structure automatically.

---

### STORY-6-1 — Semantic Interface: Intent-to-FunQL
**P0 | Size: L | Agent: intelligence-agent-A**

> As an **AI agent**, I want to say `UNDERSTAND "papers about attention cited by Vaswani"` and get correct results, so that I don't need to know FunQL syntax to query the database.

**Depends on:** STORY-4-1 (complete LogicalPlan, for validation), STORY-5-5 (agent context)

**Interface contract:**
```rust
// crates/fundb-semantic/src/lib.rs
pub struct SemanticInterface;
impl SemanticInterface {
    pub fn parse_intent(&self, intent: &str, catalog: &Catalog) -> IntentResult;
}
pub enum IntentResult {
    Confident(LogicalPlan, f32),        // plan + confidence score
    Candidates(Vec<(String, f32)>),     // 3 FunQL strings + confidence if ambiguous
    Failed(String),                     // reason
}
```

**DoD:**
- [ ] Tier 1 (rules): 70% of test set (200 curated queries) matched correctly in < 1ms
- [ ] Tier 2 (ML): 85% F1 on labeled 500-query test set in < 5ms
- [ ] Low confidence (< 0.5): returns 3 candidates instead of executing
- [ ] `UNDERSTAND "papers similar to X cited by Y last 2 years"` → FunQL with vector + graph + temporal clauses

---

### STORY-6-2 — Causal Engine Tier 2: Statistical Discovery
**P0 | Size: XL | Agent: intelligence-agent-B**

> As a **data scientist**, I want the database to automatically discover causal structure in my time-series data, with LLM-suggested hypotheses validated statistically, so that I can understand why things happen without manually specifying every relationship.

**Depends on:** STORY-5-4 (Tier 1 to persist discovered edges), STORY-5-1 (confidence ceilings)

**Interface contract:**
```rust
// crates/fundb-causal/src/tier2.rs
pub struct GrangerDiscovery;
impl GrangerDiscovery {
    pub fn test_pair(&self, x: &[f64], y: &[f64], max_lag: usize) -> GrangerResult;
    pub fn test_with_stationarity(&self, x: &[f64], y: &[f64], max_lag: usize) -> GrangerResult;  // ADF pre-check
    pub fn rolling_window_test(&self, x: &[f64], y: &[f64], window_size: usize, step: usize) -> RollingGrangerResult;
    pub fn discover_collection(&self, collection: &str, store: &LsmTree) -> Vec<CausalEdge>;
}
pub struct GrangerResult { pub f_stat: f64, pub p_value: f64, pub lag: usize, pub differenced: bool }
pub struct RollingGrangerResult { pub edges: Vec<CausalEdge>, pub stability_score: f32, pub stability_status: StabilityStatus }

pub struct LlmOracle;
impl LlmOracle {
    pub fn hypothesize(&self, schema: &Schema, sample_data: &[FunRecord], llm: &dyn LlmClient) -> Vec<(Uuid, Uuid, String)>;
    pub fn validate_and_persist(&self, hypotheses: Vec<(Uuid, Uuid, String)>, store: &LsmTree, causal: &mut CausalEngine) -> Vec<CausalEdge>;
}
```

**DoD:**
- [ ] Synthetic A→B, A→C, B→D time-series → all 3 edges discovered at p < 0.05
- [ ] Non-stationary series → ADF pre-check triggers differencing; test still finds correct edges
- [ ] LLM oracle: edge that fails Granger test NOT persisted (Three-Layer Shield, OQ-6)
- [ ] LLM oracle: persisted edge has confidence ≤ 0.60 (ceiling, per OQ-6)
- [ ] `stability_score` for permanent causal edge → ≥ 0.85; for transient edge → ≤ 0.4

---

### STORY-6-3 — Learning-to-Rank
**P1 | Size: L | Agent: intelligence-agent-C**

> As an **AI agent that queries the same database repeatedly**, I want my search results to improve over time based on what I actually use, so that the database learns my preferences without me having to configure anything.

**Depends on:** STORY-4-3 (executor integration point)

**Interface contract:**
```rust
// crates/fundb-learning/src/ltr.rs
pub struct LtrRanker;
impl LtrRanker {
    pub fn record_feedback(&mut self, agent_id: &str, query_id: Uuid, used: &[Uuid], ignored: &[Uuid]);
    pub fn rerank(&self, agent_id: &str, query: &str, candidates: Vec<(Uuid, f32)>) -> Vec<(Uuid, f32)>;
    pub fn ndcg_at_10(&self, agent_id: &str) -> Option<f32>;  // None if < 100 signals
}
```

**DoD:**
- [ ] Cold start (< 100 signals): returns original ranking unchanged
- [ ] After 100 synthetic feedback signals: NDCG@10 ≥ original NDCG@10 + 0.05
- [ ] Per-agent models: agent A's feedback does NOT affect agent B's rankings
- [ ] Model serialized < 2MB per agent

---

### STORY-6-4 — Cost-Based Optimizer
**P1 | Size: L | Agent: intelligence-agent-D**

> As the **query engine**, I want to choose between execution strategies based on statistical cost estimation, so that the query plan is near-optimal without developer hints.

**Depends on:** STORY-4-2 (rule-based optimizer to extend)

**Interface contract:**
```rust
// crates/fundb-optimizer/src/cost.rs
pub struct CostOptimizer;
impl CostOptimizer {
    pub fn optimize(plan: LogicalPlan, stats: &Statistics) -> PhysicalPlan;
}
pub struct Statistics {
    pub collection_row_count: HashMap<String, u64>,
    pub column_histograms: HashMap<(String, String), Histogram>,
    pub confidence_histograms: HashMap<String, [u32; 100]>,
    pub index_sizes: HashMap<String, u64>,
}
```

**DoD:**
- [ ] Selectivity < 1%: `EXPLAIN` shows IndexScan chosen over SeqScan
- [ ] Collection < 1000 rows: SeqScan chosen (index overhead not worth it)
- [ ] Statistics update in background without blocking queries

---

## EPIC-7 — SDKs & Protocol
**Value:** External developers can build on FunDB.

All 4 stories are **parallel** once EPIC-4 single-node works.

---

### STORY-7-1 — PostgreSQL Wire Protocol (Complete)
**P0 | Size: M | Agent: sdk-agent-A**

> As a **developer**, I want every existing PostgreSQL client (`psycopg2`, `asyncpg`, `pgx`, `sqlx`) to work with FunDB without modification, so that FunDB has zero-friction adoption.

**Depends on:** STORY-1-4 (skeleton), STORY-4-3 (executor)

**DoD:**
- [ ] `psycopg2.connect(...)` executes all FunQL examples
- [ ] `asyncpg.connect(...)` async queries work
- [ ] `COPY FROM STDIN` bulk load: 100K records in < 5 seconds
- [ ] `EXPLAIN` and `EXPLAIN ANALYZE` return parseable output
- [ ] SCRAM-SHA-256 authentication works

---

### STORY-7-2 — gRPC + REST API
**P1 | Size: M | Agent: sdk-agent-B**

> As a **microservice developer**, I want a gRPC and REST API, so that any language can call FunDB without the PostgreSQL wire protocol.

**Depends on:** STORY-4-3

**DoD:**
- [ ] `POST /query` executes FunQL, returns JSON result
- [ ] `POST /understand` calls Semantic Interface
- [ ] `POST /causal/discover` triggers causal discovery
- [ ] gRPC streaming for large result sets
- [ ] OpenAPI 3.0 spec auto-generated

---

### STORY-7-3 — Python SDK
**P0 | Size: L | Agent: sdk-agent-C**

> As a **Python AI developer**, I want a `pip install fundb` SDK with idiomatic Python helpers for vector search, agent memory, and causal queries, so that I can integrate FunDB in 10 lines of code.

**Depends on:** STORY-7-1 or STORY-7-2 (at least one protocol working)

**DoD:**
- [ ] `pip install fundb` installs cleanly
- [ ] All methods in REQ-PROTO-002 work
- [ ] `conn.understand("...")` → ResultSet
- [ ] `conn.trace_causality(a, b)` → CausalPath with `.to_dict()`, `.to_mermaid()`
- [ ] `conn.feedback(query_id, used, ignored)` triggers LTR update
- [ ] Cookbook notebook: RAG in 20 lines, runs end-to-end

---

### STORY-7-4 — Go SDK
**P1 | Size: M | Agent: sdk-agent-D**

> As a **Go backend developer**, I want a `go get fundb` SDK with idiomatic Go patterns, so that I can build production services on FunDB.

**Depends on:** STORY-7-1

**DoD:**
- [ ] `go get github.com/fundb/fundb-go` installs cleanly
- [ ] Context propagation for cancellation/timeout on all operations
- [ ] Full method parity with Python SDK
- [ ] Published to pkg.go.dev

---

## EPIC-8 — Distribution
**Value:** FunDB scales horizontally and survives node failures.

---

### STORY-8-1 — Raft Consensus
**P0 | Size: XL | Agent: dist-agent-A**

> As a **production operator**, I want FunDB to survive node failures without data loss, so that I can deploy it with confidence that it won't lose my data.

**Depends on:** STORY-2-2 (WAL), STORY-4-4 (LSM)

**Interface contract:**
```rust
// crates/fundb-raft/src/lib.rs
pub struct RaftNode;
impl RaftNode {
    pub fn new(id: NodeId, peers: Vec<PeerAddr>, storage: Arc<dyn RaftStorage>) -> Self;
    pub async fn propose(&self, entry: Vec<u8>) -> Result<()>;    // blocks until committed
    pub async fn read_index(&self) -> Result<u64>;                // linearizable read
    pub fn is_leader(&self) -> bool;
    pub fn leader_id(&self) -> Option<NodeId>;
}
```

**DoD:**
- [ ] 5-node cluster: kill leader → new leader elected within 3 × election_timeout
- [ ] Split-brain: network partition → no two nodes believe they are leader simultaneously
- [ ] Log consistent: re-join failed node → log identical to peers (no entries lost)
- [ ] Snapshot: log compaction prevents unbounded growth (tested at 1M entries)

---

### STORY-8-2 — Shard Manager
**P0 | Size: L | Agent: dist-agent-B**

> As the **query router**, I want to know which shard holds any given record and route writes/reads there, so that the database distributes data evenly and scales horizontally.

**Depends on:** STORY-8-1

**Interface contract:**
```rust
// crates/fundb-cluster/src/sharding.rs
pub struct ShardMap;
impl ShardMap {
    pub fn shard_for(&self, collection: &str, record_id: &Uuid) -> ShardId;
    pub fn shards_for_collection(&self, collection: &str) -> Vec<ShardId>;
    pub fn leader_addr(&self, shard: ShardId) -> SocketAddr;
    pub fn add_node(&mut self, node: NodeId, addr: SocketAddr) -> Vec<ShardMigration>;
    pub fn remove_node(&mut self, node: NodeId) -> Vec<ShardMigration>;
}
```

**DoD:**
- [ ] Adding a node to 3-node cluster: ≤ K/N records migrated (consistent hash property)
- [ ] All writes route to correct shard leader
- [ ] Tenant A's records consistently map to same shard group

---

### STORY-8-3 — Distributed Query Execution
**P0 | Size: XL | Agent: dist-agent-C**

> As the **coordinator node**, I want to scatter query fragments to shards and gather results back, so that distributed queries are transparent to the client.

**Depends on:** STORY-8-2, STORY-4-3

**DoD:**
- [ ] 3-shard distributed ANN top-10: recall ≥ 0.95 vs centralized (merge-rerank with 3× oversample)
- [ ] Distributed JOIN: result identical to single-node (verified on test dataset)
- [ ] Cross-shard confidence propagation: `min()` merge applied correctly (OQ-9)
- [ ] Distributed `WITHIN CONTEXT`: candidates gathered from all shards, MMR on coordinator

---

## EPIC-9 — Causal Tier 3
**Value:** FunDB can answer "what would have happened if…" questions.

---

### STORY-9-1 — SCM: Interventions + Counterfactuals + Ensemble Discovery
**P1 | Size: XL | Agent: causal-agent**

> As a **decision maker**, I want to ask "what would revenue have been if we hadn't cut marketing spend?" and get a statistically grounded answer, so that I can make better decisions using historical data.

**Depends on:** STORY-6-2 (Tier 2), STORY-5-4 (Tier 1)

**DoD:**
- [ ] `CREATE CAUSAL MODEL` with linear equations stores DAG + coefficients
- [ ] `INTERVENE ON m SET X=2 PREDICT Y` → estimate within 5% of ground truth (synthetic SCM)
- [ ] `ESTIMATE COUNTERFACTUAL ... GIVEN ... HAD ... PREDICT` → matches analytical solution
- [ ] PC algorithm: 5-variable DAG → recovers ≥ 80% of edges
- [ ] NOTEARS non-convergence after 3 restarts → ensemble continues with PC+Granger, warning logged
- [ ] `DIRECTION_UNCERTAIN` edges appear in results with correct status

---

## EPIC-10 — Production Hardening
**Value:** FunDB can run in production safely.

---

### STORY-10-1 — Security (Multi-tenant, TLS, RBAC)
**P0 | Size: L | Agent: security-agent**

> As a **SaaS operator**, I want tenants completely isolated and all connections encrypted, so that one tenant's data never leaks to another and I can comply with security standards.

**Depends on:** EPIC-4, EPIC-8

**DoD:**
- [ ] SQL injection attempt in tenant A does NOT return tenant B data
- [ ] TLS 1.3 enforced on all connections
- [ ] GRANT/REVOKE collection-level permissions work
- [ ] Audit log: every query logged with agent_id, timestamp, collections touched

---

### STORY-10-2 — Observability (Metrics, Tracing, Dashboards)
**P1 | Size: M | Agent: ops-agent-A**

> As a **production engineer**, I want Prometheus metrics and distributed traces, so that I can alert on degradation and debug slow queries.

**DoD:**
- [ ] `/metrics` endpoint returns Prometheus-format metrics (query latency histograms, write throughput, replication lag)
- [ ] OpenTelemetry traces: each query produces a trace with per-operator spans
- [ ] Grafana dashboard template importable and shows all key metrics

---

### STORY-10-3 — Benchmarks + Performance Validation
**P0 | Size: L | Agent: ops-agent-B**

> As the **engineering team**, I want automated benchmarks that fail CI if performance regresses, so that we never accidentally ship a slow release.

**DoD:**
- [ ] All REQ-PERF-001…011 targets met (ARCHITECTURE.md §19)
- [ ] Regression detection: > 10% degradation in any metric fails CI
- [ ] Comparison benchmark vs pgvector and Qdrant on shared ANN workload: FunDB wins on hybrid queries

---

## Parallel Tracks Summary

The following groups of stories can be assigned to agents simultaneously:

| Track | Stories | Start condition |
|-------|---------|----------------|
| **Track A** | STORY-1-1 | Immediately |
| **Track B** | STORY-1-3 | Immediately (independent of Track A) |
| **Track C** | STORY-1-4 | Immediately (independent of A, B) |
| **Track D** | STORY-1-2 | After Track A interface contract published |
| **Track E** | STORY-2-1, 2-2, 2-3, 2-4 | After Tracks A + D |
| **Track F** | STORY-3-1 thru 3-5 | All parallel, after Track E |
| **Track G** | STORY-4-1 | After Tracks B + F |
| **Track H** | STORY-4-2, 4-3, 4-4 | After Track G (4-1 must be done first) |
| **Track I** | STORY-5-1 thru 5-5 | All parallel, after Track H |
| **Track J** | STORY-6-1 thru 6-4 | All parallel, after Track I |
| **Track K** | STORY-7-1 thru 7-4 | All parallel, starts after Track H (single-node working) |
| **Track L** | STORY-8-1 → 8-2 → 8-3 | Sequential within track, starts after Track H |
| **Track M** | STORY-9-1 | After Track J (6-2 complete) |
| **Track N** | STORY-10-1 thru 10-3 | After all tracks |

**Maximum active parallel agents at any point: 5**
(Track F: 5 agents; Track I: 5 agents; Track J: 4 agents + Track K starting)

---

## Story Size Summary

| Size | Count | Stories |
|------|-------|---------|
| **XL** | 6 | 3-2 (HNSW), 4-3 (Executor), 6-2 (Granger), 8-1 (Raft), 8-3 (Dist Query), 9-1 (SCM) |
| **L** | 12 | 1-3, 2-3, 2-4, 3-5, 4-1, 4-4, 5-2, 5-4, 5-5, 6-1, 6-3, 6-4, 7-3, 8-2, 10-3 |
| **M** | 11 | 1-1, 1-2, 2-1, 2-2, 3-1, 3-3, 3-4, 5-1, 5-3, 7-1, 7-2, 7-4, 8-2, 10-2 |
| **S** | 2 | 1-4, 7-4 |

**Total stories: 31**
**P0 stories: 21 | P1 stories: 8 | P2: 2**
