# FunDB — Complete Implementation Plan (Parallel Agent Edition)

**Version:** 1.0
**Language:** Rust (core engine), with Python/Go/JS SDKs
**Build System:** Cargo (monorepo workspace)
**Strategy:** Wave-based parallel execution — agents within a wave work simultaneously; next wave starts only when all blockers in current wave are done.

---

## Dependency Graph Overview

```
Wave 1: Foundation (no deps)
  ├── [A] Workspace setup + FunRecord data model
  ├── [B] Codec (MessagePack + FlatBuffers)
  ├── [C] FunQL Grammar + Lexer
  └── [D] Wire protocol skeleton

Wave 2: Storage primitives (depends: Wave 1-A, 1-B)
  ├── [A] MemTable (SkipList + concurrent ops)
  ├── [B] WAL (append-only log + recovery)
  ├── [C] SSTable format + block reader/writer
  └── [D] MVCC bitemporal versioning layer

Wave 3: Index engines (depends: Wave 2)
  ├── [A] B+Tree index
  ├── [B] HNSW vector index
  ├── [C] Graph SPO index
  ├── [D] Temporal interval index
  └── [E] Causal DAG index + confidence histogram index

Wave 4: Query Engine core (depends: Wave 1-C, Wave 3)
  ├── [A] Parser + AST + Binder
  ├── [B] Rule-based Optimizer
  ├── [C] Physical Operators (Volcano executor)
  └── [D] Compaction engine + LSM controller

Wave 5: Cognitive modules (depends: Wave 4)
  ├── [A] Confidence propagation engine
  ├── [B] Context-aware retrieval (MMR + WITHIN CONTEXT)
  ├── [C] Contradiction detection
  ├── [D] Causal Engine Tier 1 (explicit edges + TRACE CAUSALITY)
  └── [E] Agent Memory subsystem

Wave 6: Advanced Intelligence (depends: Wave 5)
  ├── [A] Semantic Interface Tier 1-2 (rule engine + ML classifier)
  ├── [B] Causal Engine Tier 2 (Granger + temporal + LLM oracle)
  ├── [C] Bidirectional Learning (LTR / LambdaMART)
  └── [D] Cost-based optimizer + adaptive index advisor

Wave 7: Distribution (depends: Wave 4 + Wave 5)
  ├── [A] Raft consensus
  ├── [B] Shard manager + consistent hashing
  └── [C] Distributed query execution (scatter/gather)

Wave 8: Protocol + SDKs (depends: Wave 4 core working)
  ├── [A] PostgreSQL wire protocol
  ├── [B] gRPC + REST API
  ├── [C] Python SDK
  └── [D] Go SDK

Wave 9: Experimental (depends: Wave 6)
  ├── [A] Causal Engine Tier 3 (SCM + do-calculus + counterfactuals)
  └── [B] Semantic Interface Tier 3 (ONNX embedded LLM)

Wave 10: Production Hardening (depends: all prior waves)
  ├── [A] Security (TLS, RBAC, tenant isolation)
  ├── [B] Observability (metrics, tracing, logs)
  ├── [C] Benchmarks + performance validation
  └── [D] Chaos engineering + correctness tests
```

---

## Crate Structure (Cargo Workspace)

```
fundb/
├── Cargo.toml                    ← workspace root
│
├── crates/
│   ├── fundb-core/               ← FunRecord, types, codecs (Wave 1)
│   ├── fundb-storage/            ← FunStore: LSM, WAL, SSTable, MVCC (Wave 2)
│   ├── fundb-indexes/            ← All index implementations (Wave 3)
│   ├── fundb-sql/                ← FunQL: lexer, parser, AST, binder (Wave 4-A)
│   ├── fundb-optimizer/          ← Rule-based + cost-based optimizer (Wave 4-B, 6-D)
│   ├── fundb-executor/           ← Volcano operators, execution engine (Wave 4-C)
│   ├── fundb-cognitive/          ← Confidence, context, contradiction, agent memory (Wave 5)
│   ├── fundb-causal/             ← Causal engine Tiers 1-3 (Wave 5-D, 6-B, 9-A)
│   ├── fundb-semantic/           ← Semantic Interface Tiers 1-3 (Wave 6-A, 9-B)
│   ├── fundb-learning/           ← LTR, LambdaMART, index advisor (Wave 6-C, 6-D)
│   ├── fundb-raft/               ← Raft consensus (Wave 7-A)
│   ├── fundb-cluster/            ← Shard manager, distributed query (Wave 7-B, 7-C)
│   ├── fundb-protocol/           ← PG wire, gRPC, REST (Wave 8-A, 8-B)
│   └── fundb-server/             ← Main binary, wires everything together
│
├── sdks/
│   ├── python/                   ← Python SDK (Wave 8-C)
│   ├── go/                       ← Go SDK (Wave 8-D)
│   └── javascript/               ← JS/TS SDK (future)
│
├── tests/
│   ├── integration/              ← End-to-end integration tests
│   ├── property/                 ← Property-based tests (proptest)
│   └── benchmarks/               ← Criterion benchmarks
│
└── tools/
    ├── fundb-cli/                ← REPL + admin tool
    └── fundb-bench/              ← Benchmark harness
```

---

## Wave 1 — Foundation

**Prerequisite:** None
**Parallelism:** All 4 agents work simultaneously
**Definition of Done:** FunRecord encodes/decodes round-trip correctly; FunQL tokenizes simple queries; PG wire handshake completes.

---

### Agent 1-A: Workspace Setup + FunRecord Data Model

**Crate:** `fundb-core`
**Estimated complexity:** Medium

#### Tasks
1. Initialize Cargo workspace with all crate stubs
2. Implement `FunRecord` struct with all fields:
   - Core identity: `_id` (UUID v7), `_collection`, `_tenant`
   - Temporal envelope: `_sys_from`, `_sys_to`, `_valid_from`, `_valid_to`
   - Knowledge payload: `data` (raw bytes, MessagePack)
   - Vector extensions: `_vectors: HashMap<String, Vec<f32>>`
   - Graph edges: `_edges: Vec<Edge>`
   - Time-series: `_timeseries: Vec<Sample>`
   - Confidence: `_confidence: f32`
   - Provenance: `_sources: Vec<Source>`
   - Contradiction refs: `_supports: Vec<Ref>`, `_contradicts: Vec<Ref>`
   - Causal edges: `_caused_by: Vec<CausalEdge>`, `_effects: Vec<CausalEdge>`
3. Implement `Edge`, `Sample`, `Source`, `Ref`, `CausalEdge`, `CausalType` types
4. Implement UUID v7 generation (time-ordered)
5. Implement `FunRecordBuilder` (fluent builder pattern)
6. Write unit tests: round-trip encoding, field defaults, UUID ordering

#### Interfaces to expose
```rust
pub struct FunRecord { ... }
pub struct Edge { pub label: String, pub target: Uuid, pub props: Vec<u8> }
pub struct CausalEdge {
    pub source_id:        Uuid,
    pub target_id:        Uuid,
    pub relation:         CausalType,
    pub strength:         f32,
    pub mechanism:        Option<String>,
    // Resolved by OQ-2/6: origin tracking + confidence ceiling
    pub origin:           CausalOrigin,       // UserDeclared | Granger | Temporal | LlmValidated
    pub confidence:       f32,                // separate from strength; ceiling by origin
    // Resolved by OQ-7: stationarity metadata
    pub stability_score:  Option<f32>,        // 0.0–1.0, None for non-timeseries
    pub stability_status: StabilityStatus,    // Stable | Unstable | Provisional | NotApplicable
    // Resolved by OQ-10: ensemble discovery metadata
    pub direction_status: DirectionStatus,    // Confirmed | DirectionUncertain
    pub discovery_algo:   Option<String>,     // "ensemble" | "pc" | "notears" | "granger" | "user"
}
// See docs/architecture/causal-design-decisions.md for full enum definitions
pub enum CausalType { Caused, Influenced, Correlated, Preceded }
pub struct Source { pub origin: String, pub timestamp: i64, pub method: SourceMethod, pub confidence: f32 }
pub fn new_record_id() -> Uuid;  // UUID v7
```

---

### Agent 1-B: Codec (MessagePack + FlatBuffers)

**Crate:** `fundb-core` (codec module)
**Depends on:** 1-A interfaces

#### Tasks
1. Implement `FunEncoder`: serializes FunRecord to binary
   - Document payload: MessagePack (via `rmp-serde`)
   - IPC format: FlatBuffers schema for zero-copy reads
   - Vector columns: raw f32 little-endian arrays
2. Implement `FunDecoder`: deserializes binary to FunRecord
3. Implement column-oriented encoding for SSTable pages:
   - Scalar columns: integer/string encoding with prefix compression
   - Vector columns: f32 arrays with optional Product Quantization stub
   - Confidence column: f32 column with histogram in page header
4. Implement page header format (min/max stats, bloom filter wire format, row count)
5. Compression wrapper: Zstd for documents, Snappy for hot data
6. Write property-based tests: encode → decode must be identity for all valid inputs

#### Interfaces to expose
```rust
pub trait Encode { fn encode(&self) -> Result<Vec<u8>>; }
pub trait Decode: Sized { fn decode(bytes: &[u8]) -> Result<Self>; }
pub struct PageEncoder { ... }
pub struct PageDecoder { ... }
pub struct PageHeader { pub min_key: Vec<u8>, pub max_key: Vec<u8>, pub row_count: u32, pub bloom_filter: BloomFilter }
```

---

### Agent 1-C: FunQL Grammar + Lexer

**Crate:** `fundb-sql`

#### Tasks
1. Define complete FunQL grammar in LALR(1) compatible EBNF
   - Full SQL subset: SELECT, INSERT, UPDATE, DELETE, CREATE, DROP
   - Vector extensions: `_vector(field) <-> value < threshold`
   - Graph extensions: `TRAVERSE label(depth: n..m) -> collection`
   - Temporal extensions: `AS OF SYSTEM TIME '...'`, `AS OF VALID TIME BETWEEN`
   - Confidence extensions: `_confidence > threshold`, `_source_count`, `_contradiction_count`
   - Causal extensions: `TRACE CAUSALITY FROM x TO y`, `ESTIMATE EFFECT OF SET(x=v) ON y`
   - Context extensions: `WITHIN CONTEXT (max_tokens: n, coherence: f, diversity: f)`
   - Semantic extensions: `UNDERSTAND "natural language"`, `DISCOVER CAUSAL STRUCTURE IN`
   - Agent memory: `RECALL BY`, `REMEMBER`, `FORGET`
2. Implement lexer (hand-written or via `logos` crate):
   - All keywords, operators, literals
   - Special operators: `<->` (vector distance), `->` (graph target)
3. Implement parser (via `lalrpop` or hand-written recursive descent)
   - Produces typed AST nodes
4. Define all AST node types (enums/structs for every construct)
5. Write parser tests: parse every query example from ARCHITECTURE.md

#### Interfaces to expose
```rust
pub fn parse(input: &str) -> Result<Statement, ParseError>;
pub enum Statement { Select(SelectStmt), Insert(InsertStmt), ... }
pub enum Expr { ... }  // full expression tree
```

---

### Agent 1-D: Wire Protocol Skeleton

**Crate:** `fundb-protocol`

#### Tasks
1. Implement PostgreSQL wire protocol startup sequence:
   - Startup message parsing (client → server)
   - Authentication response (trust auth for dev mode)
   - `ReadyForQuery` response
2. Implement simple query protocol:
   - Parse `Query` message
   - Return `RowDescription`, `DataRow[]`, `CommandComplete` responses
   - Error response format (`ErrorResponse`)
3. TCP server skeleton: `TcpListener` + Tokio async accept loop
4. Connection handler: one coroutine per connection
5. Stub: forward raw query string to query engine (returns mock results for now)
6. Write integration test: `psql` can connect and run `SELECT 1`

#### Interfaces to expose
```rust
pub struct Server { ... }
impl Server {
    pub async fn bind(addr: SocketAddr) -> Result<Self>;
    pub async fn run(self, query_handler: impl QueryHandler) -> Result<()>;
}
pub trait QueryHandler: Send + Sync {
    async fn execute(&self, query: &str, conn_ctx: &ConnContext) -> Result<QueryResult>;
}
```

---

## Wave 2 — Storage Primitives

**Prerequisite:** Wave 1-A (FunRecord), 1-B (Codec)
**Parallelism:** All 4 agents work simultaneously
**Definition of Done:** Can write and read back FunRecords durably; MVCC time-travel returns correct versions.

---

### Agent 2-A: MemTable

**Crate:** `fundb-storage`

#### Tasks
1. Implement concurrent skip-list (lock-free using atomic CAS operations)
   - Keys: binary-encoded record keys (collection + _id)
   - Values: FunRecord (encoded)
   - Operations: insert, lookup, range_scan (iterator), delete (tombstone)
2. Implement MemTable wrapper:
   - Size tracking (bytes in memory)
   - Flush trigger when size > threshold (64MB default)
   - Immutable MemTable (frozen during flush)
3. Implement WAL write-through on every MemTable insert
4. Write tests: concurrent inserts from 16 threads, range scans, flush

---

### Agent 2-B: WAL (Write-Ahead Log)

**Crate:** `fundb-storage`

#### Tasks
1. Implement log record format:
   - Header: length, CRC32, log sequence number (LSN), type
   - Types: `Write(FunRecord)`, `Delete(key)`, `Checkpoint(LSN)`, `TxnBegin(txn_id)`, `TxnCommit(txn_id)`, `TxnAbort(txn_id)`
2. Implement log writer: append-only, fsync on commit
3. Implement log reader + recovery:
   - Scan from last checkpoint
   - Replay writes to MemTable
   - Handle partial writes (CRC mismatch = stop recovery)
4. Implement checkpoint: write checkpoint record, truncate old segments
5. Write tests: crash recovery (truncate WAL file mid-write, verify recovery)

---

### Agent 2-C: SSTable Format + Block Reader/Writer

**Crate:** `fundb-storage`

#### Tasks
1. Implement SSTable file format:
   - Data blocks (sorted key-value pairs, prefix-compressed keys)
   - Index block (one entry per data block: first key + offset)
   - Filter block (Bloom filter for key membership)
   - Footer (offsets to index + filter blocks, magic number)
2. Implement SSTable writer: accepts sorted key-value stream, produces file
3. Implement SSTable reader: open file, point lookup (bloom → index → data), range scan
4. Implement block cache (LRU, configurable size)
5. Implement compaction merger: merge N SSTables into one, handle tombstones
6. Write tests: write 100K records, verify all readable; range scans correct

---

### Agent 2-D: MVCC Bitemporal Versioning Layer

**Crate:** `fundb-storage`

#### Tasks
1. Implement bitemporal key encoding:
   - Key format: `collection | _id | sys_from | sys_to | valid_from | valid_to`
   - Supports range scans by time ranges
2. Implement MVCC write path:
   - New version = new key with new `sys_from = now()`, old version gets `sys_to = now()`
   - Atomic update via WAL
3. Implement time-travel read:
   - `AS OF SYSTEM TIME t` → scan for latest version where `sys_from <= t AND sys_to > t`
   - `AS OF VALID TIME BETWEEN t1 AND t2` → scan valid_time dimension
4. Implement snapshot isolation:
   - Transaction ID (monotonic counter)
   - Read snapshot: only see versions committed before txn started
5. Implement version garbage collection (GC):
   - Configurable retention window
   - Background GC task removes expired versions
6. Write tests: update record 5 times, verify time-travel returns correct version at each timestamp

---

## Wave 3 — Index Engines

**Prerequisite:** Wave 2 complete
**Parallelism:** All 5 agents work simultaneously
**Definition of Done:** Each index can insert, lookup, and scan with correct results and expected asymptotic performance.

---

### Agent 3-A: B+Tree Index

**Crate:** `fundb-indexes`

#### Tasks
1. Implement B+Tree node format (internal + leaf, configurable order)
2. Implement insert with splits (propagate up)
3. Implement delete with merges/borrows
4. Implement point lookup and range scan (iterator)
5. Implement prefix compression on string keys
6. Implement concurrent access (RWLock per node, or optimistic locking)
7. Integrate with SSTable: B+Tree pages stored in column page groups
8. Implement index on composite keys (collection + field + value)
9. Write property-based tests: insert random keys, verify sorted order, all lookups correct

---

### Agent 3-B: HNSW Vector Index

**Crate:** `fundb-indexes`

#### Tasks
1. Implement HNSW construction:
   - Layer assignment (random level with exponential decay)
   - `ef_construction` beam search for neighbor selection
   - `M` max neighbors per node per layer
   - Heuristic neighbor selection (diverse neighbors preferred)
2. Implement HNSW search:
   - Entry point = node with highest layer
   - Greedy descent from top layer
   - `ef_search` beam search at Layer 0
   - Returns top-K nearest neighbors
3. Implement incremental deletes (lazy tombstone marking)
4. Implement graph compaction (periodic rebuild of deleted-heavy graphs)
5. Implement multi-tenant isolation: separate HNSW graph per tenant
6. Implement Product Quantization compression:
   - PQ training: k-means on subvectors
   - PQ encoding: replace float32 subvectors with centroid IDs
   - PQ distance estimation: lookup table for fast approximate distance
7. Implement disk-resident lower layers (memory-mapped files, LRU eviction)
8. Write tests: build index with 100K vectors, verify recall@10 > 0.95

---

### Agent 3-C: Graph SPO Index

**Crate:** `fundb-indexes`

#### Tasks
1. Define edge format: `(subject: UUID, predicate: String, object: UUID, props: Vec<u8>, confidence: f32)`
2. Implement triple store with three indexes:
   - SPO index: lookup all predicates + objects for a given subject
   - POS index: lookup all subjects + objects for a given predicate
   - OSP index: reverse lookup (find subjects pointing to a given object)
3. Implement BFS traversal: `TRAVERSE label(depth: 1..3) FROM start`
   - Returns all nodes at each depth level
   - Supports depth limits, visited set (cycle prevention)
4. Implement DFS path finding: find all paths between two nodes up to max depth
5. Implement weighted traversal: edges have confidence scores, path score = product
6. Implement reverse index: given object, find all subjects pointing to it
7. Write tests: build citation graph 10K nodes, verify 3-hop traversal correct

---

### Agent 3-D: Temporal Interval Index

**Crate:** `fundb-indexes`

#### Tasks
1. Implement interval B+Tree (R-tree variant for 1D intervals):
   - Key: `(valid_from, valid_to)` pair
   - Supports interval overlap queries: "find all records valid during time T"
2. Implement temporal range queries:
   - Point-in-time: `AS OF VALID TIME T` → intervals containing T
   - Range: `AS OF VALID TIME BETWEEN T1 AND T2` → overlapping intervals
3. Implement system time index (separate from valid time):
   - Same structure, keyed on `(sys_from, sys_to)`
4. Integrate with MVCC layer for combined bitemporal queries
5. Write tests: insert 100K time intervals, verify point and range queries correct

---

### Agent 3-E: Causal DAG Index + Confidence Histogram Index

**Crate:** `fundb-indexes`
**Note:** Two related indexes sharing a crate module.

#### Tasks — Causal DAG Index
1. Implement DAG node store: `node_id → { label, confidence, _record_id }`
2. Implement edge store: `(source_id, target_id) → CausalEdge`
3. Implement forward adjacency list: `source_id → Vec<(target_id, edge_data)>`
4. Implement reverse adjacency list: `target_id → Vec<(source_id, edge_data)>`
5. Implement cycle detection on insert (DAG invariant enforcement)
6. Implement path query: BFS from A to B, return all paths up to max_depth
7. Implement transitive closure cache:
   - For top-K most queried (source, target) pairs, cache pre-computed paths
   - Cache invalidation on edge insert/delete
8. Implement causal strength propagation along paths: `strength = Π(edge_strengths)`

#### Tasks — Confidence Histogram Index
1. Implement confidence histogram per collection:
   - 100 buckets from 0.0 to 1.0
   - Stored in page header for fast pruning
2. Implement B+Tree on confidence values for range queries
3. Implement `_source_count` and `_contradiction_count` virtual columns (computed from provenance/contradiction arrays)
4. Write tests for both indexes

---

## Wave 4 — Query Engine Core

**Prerequisite:** Wave 1-C (parser stubs), Wave 3 complete
**Parallelism:** 4 agents, but 4-A must complete before 4-B/4-C can finalize
**Definition of Done:** Can execute SELECT with scalar filters, vector search, graph traversal, temporal queries, confidence filters end-to-end on a single node.

---

### Agent 4-A: Parser + AST + Binder (complete FunQL)

**Crate:** `fundb-sql`
**Depends on:** Agent 1-C stubs

#### Tasks
1. Complete parser for all FunQL constructs (extend Wave 1-C skeleton)
2. Implement complete AST:
   - `SelectStmt { projections, from, where, order_by, limit, within_context, as_of }`
   - `InsertStmt`, `UpdateStmt`, `DeleteStmt`
   - `CreateCollectionStmt`, `CreateIndexStmt`, `CreateCausalModelStmt`
   - `UnderstandStmt`, `TraceCausalityStmt`, `EstimateEffectStmt`, `CounterfactualStmt`
   - `RecallByStmt`, `DiscoverCausalStructureStmt`
3. Implement Binder:
   - Catalog lookup (collection names, field names, types)
   - Type checking (vector field must be float array, confidence must be float, etc.)
   - Resolve `*` projections
   - Bind parameter placeholders (`:param_name`)
4. Define `LogicalPlan` enum: all logical operators
5. Implement logical plan builder (AST → LogicalPlan)

---

### Agent 4-B: Rule-Based Optimizer

**Crate:** `fundb-optimizer`
**Depends on:** 4-A (LogicalPlan)

#### Tasks
1. Implement rule engine framework:
   - `Rule` trait: `fn apply(&self, plan: &LogicalPlan) -> Option<LogicalPlan>`
   - Bottom-up plan traversal
   - Rule application loop (until fixpoint)
2. Implement rules:
   - **Predicate pushdown**: push filters past joins and projections
   - **Projection pruning**: remove unused columns
   - **Join reorder**: reorder joins by estimated cardinality
   - **Vector pre-filter**: if scalar filter + vector search, scalar filter first
   - **Graph path pruning**: prune impossible paths using schema constraints
   - **Confidence early pruning**: push confidence filter before ANN scan
   - **Temporal predicate pushdown**: push AS OF into table scan
   - **Causal path pruning**: use transitive closure to prune impossible paths
3. Implement statistics module:
   - Collection row counts
   - Column histograms (value distribution)
   - Confidence histogram per collection
   - Vector index size
4. Implement simple cardinality estimation (histogram-based)

---

### Agent 4-C: Physical Operators + Volcano Executor

**Crate:** `fundb-executor`
**Depends on:** 4-A (LogicalPlan), Wave 3 indexes

#### Tasks
1. Implement `PhysicalPlan` enum and `PhysicalOperator` trait:
   ```rust
   pub trait PhysicalOperator: Send {
       fn next_batch(&mut self) -> Result<Option<RecordBatch>>;
       fn schema(&self) -> Schema;
   }
   ```
2. Implement operators:
   - `SeqScan`: full collection scan
   - `IndexScan(B+Tree)`: point + range scan
   - `VectorANN(HNSW)`: approximate nearest neighbor search
   - `GraphTraverse`: BFS/DFS on SPO index
   - `TemporalRangeScan`: bitemporal interval scan
   - `CausalPathScan`: BFS on causal DAG index
   - `ConfidenceFilter`: filter by confidence column
   - `Projection`: select + compute expressions
   - `Filter`: evaluate predicates on record batches
   - `HashJoin`: standard hash join
   - `MergeJoin`: sort-merge join for ordered inputs
   - `Sort`: external merge sort (spill to disk if needed)
   - `Limit/TopK`: heap-based top-K for ORDER BY + LIMIT
   - `HashAggregate`: GROUP BY with aggregation functions
   - `ContextOptimize`: MMR selection (stub, completed in Wave 5-B)
3. Implement `RecordBatch` (1024 rows/batch, columnar layout)
4. Implement query context: parameter bindings, transaction snapshot
5. Implement plan-to-physical mapping (LogicalPlan → PhysicalPlan)
6. Write integration tests: run all ARCHITECTURE.md example queries, verify correct results

---

### Agent 4-D: LSM Controller + Compaction Engine

**Crate:** `fundb-storage`
**Depends on:** Wave 2 complete

#### Tasks
1. Implement LSM-tree controller:
   - Level management (L0...Ln)
   - L0 flush trigger: when MemTable full → flush to L0
   - L0 → L1 compaction trigger: when L0 has > 4 SSTables
   - Leveled compaction for L1+: when level size > threshold
2. Implement tiered compaction strategy (STCS) for lower levels
3. Implement leveled compaction strategy (LCS) for upper levels
4. Implement background compaction thread pool
5. Implement read path: search MemTable → L0 SSTables (newest first) → L1..Ln
6. Implement Bloom filter at each level to skip irrelevant SSTables
7. Write tests: write 1M records, verify all readable after multiple compaction rounds

---

## Wave 5 — Cognitive Modules

**Prerequisite:** Wave 4 complete (operators executable)
**Parallelism:** All 5 agents simultaneously
**Definition of Done:** Confidence propagates correctly through JOIN; MMR selects coherent results within token budget; TRACE CAUSALITY returns correct paths; agent memory recalls by semantic+recency.

---

### Agent 5-A: Confidence Propagation Engine

**Crate:** `fundb-cognitive`

#### Tasks
1. Implement `ConfidencePropagation` algebra:
   - `JOIN(A, B) → min(A._confidence, B._confidence)`
   - `UNION(A, B) → max(A._confidence, B._confidence)`
   - `AGGREGATE(group) → weighted_avg(group._confidence)`
   - `GRAPH_TRAVERSE path → product of edge strengths × node confidences`
   - `CAUSAL_CHAIN → product of all strengths`
   - `NEGATION(NOT A) → 1.0 - A._confidence`
2. Implement confidence propagation operator in executor:
   - Wraps any operator, augments output with propagated confidence
3. Implement lazy propagation cache:
   - Cache propagation results for repeated sub-expressions
   - Cache invalidation on underlying data updates
4. Implement automatic confidence update on:
   - New corroborating source: `confidence += 0.1` (capped at 1.0)
   - New contradiction: `confidence -= 0.2 × contradiction_confidence`
   - Time decay: `confidence *= decay_factor(age)` for time-sensitive collections
5. Write property-based tests: verify propagation algebra laws (commutativity, associativity)

---

### Agent 5-B: Context-Aware Retrieval (WITHIN CONTEXT + MMR)

**Crate:** `fundb-cognitive`

#### Tasks
1. Implement MMR (Maximal Marginal Relevance) algorithm:
   ```
   score(doc) = λ × sim(doc, query) - (1-λ) × max(sim(doc, selected))
   ```
   - Configurable `λ` (= `diversity` parameter, inverted)
   - Uses HNSW distance computations for similarity
2. Implement `ContextOptimize` physical operator (completes Wave 4-C stub):
   - Step 1: Oversample (10× final target count)
   - Step 2: Token estimation per record (byte_length / 4)
   - Step 3: MMR selection within token budget
   - Step 4: Coherence check (pairwise similarity of selected set)
   - Step 5: Contradiction inclusion (if `include_contradictions: true`)
3. Implement `WITHIN CONTEXT` clause execution:
   - Parse parameters: `max_tokens`, `coherence`, `diversity`, `include_contradictions`, `priority`
   - Build ContextOptimize operator with parameters
4. Implement response metadata:
   - `tokens_used`, `coverage_score`, `coherence_score`, `diversity_score`, `avg_confidence`
   - `contradictions_found`, `candidates_evaluated`, `candidates_selected`
5. Write tests: verify token budget respected, diversity parameter affects output, coherence threshold enforced

---

### Agent 5-C: Contradiction Detection

**Crate:** `fundb-cognitive`

#### Tasks
1. Implement contradiction detector triggered on INSERT:
   - Compute embedding of new record (or use provided `_vectors.content`)
   - Scan collection for records with cosine_sim > 0.85
   - For high-similarity pairs, check semantic polarity:
     - Simple negation detection: "X is true" vs "X is not true"
     - Antonym detection: "X is hot" vs "X is cold"
2. Implement polarity classifier:
   - Rule-based: negation words, antonym lookup table
   - ML fallback: lightweight binary classifier (~5MB)
3. Implement auto-linking:
   - Add `_contradicts` edge between conflicting records
   - Adjust confidence of both records
   - Emit `CONTRADICTION` event on message bus
4. Implement contradiction query support:
   - `_contradiction_count` virtual column in query engine
   - `SELECT contradictions FROM ...` syntax
5. Implement contradiction resolution workflow:
   - Mark one contradiction as `resolved: true`
   - Update confidence accordingly
6. Write tests: insert conflicting facts, verify auto-linking, confidence adjustment

---

### Agent 5-D: Causal Engine — Tier 1 (Explicit Causality)

**Crate:** `fundb-causal`

#### Tasks
1. Implement explicit causal edge API:
   ```sql
   INSERT INTO _causal_edges (source_id, target_id, relation, strength, mechanism)
   VALUES (:source, :target, 'CAUSED', 0.85, 'deployed new model → errors increased');
   ```
2. Implement DAG validation on insert: reject cycles
3. Implement `TRACE CAUSALITY FROM :A TO :B` query execution:
   - BFS on FunCausal index (Wave 3-E)
   - Respect `MAX_DEPTH` and `MIN_STRENGTH` parameters
   - Return all paths with total_strength (product of edge strengths)
   - Sort by total_strength DESC
4. Implement `QUERY_EFFECTS(:event, MAX_DEPTH :n)`: what does this event cause?
5. Implement `QUERY_CAUSES(:event, MAX_DEPTH :n)`: what caused this event?
6. Implement causal path visualization output:
   - JSON format: nodes + edges with strengths
   - Mermaid diagram format
   - DOT (Graphviz) format
7. Write tests: build causal graph 1K nodes, verify paths correct, cycle detection works

---

### Agent 5-E: Agent Memory Subsystem

**Crate:** `fundb-cognitive`

#### Tasks
1. Implement `AgentMemory` collection schema:
   ```
   {
     agent_id: string,
     content: string,
     _vectors: { semantic: float32[] },
     _confidence: float32,
     memory_type: enum { episodic, semantic, procedural },
     importance: float32,       // 0.0–1.0
     access_count: u32,
     last_accessed: timestamp,
     decay_rate: f32,           // per-memory configurable
   }
   ```
2. Implement `REMEMBER` syntax:
   ```sql
   REMEMBER "The user prefers Python over Go for scripting tasks"
   FOR AGENT :agent_id
   WITH importance 0.8
   AS semantic;
   ```
3. Implement `RECALL BY` execution:
   ```sql
   RECALL BY semantic_similarity(:query, weight: 0.5)
            + recency(weight: 0.3)
            + importance(weight: 0.2)
   FOR AGENT :agent_id
   LIMIT 20;
   ```
   - Compute weighted score for each memory: `score = Σ(weight_i × metric_i)`
   - Return top-N by score
4. Implement memory decay: background job reduces `_confidence` by `decay_rate × time_since_last_access`
5. Implement memory consolidation:
   - Find semantically similar memories (cosine_sim > 0.9)
   - Merge into single memory with combined confidence
   - Increase importance of merged memory
6. Implement `FORGET :memory_id` (soft delete with tombstone)
7. Write tests: store 1K memories, recall query returns correct ranking by weighted score

---

## Wave 6 — Advanced Intelligence

**Prerequisite:** Wave 5 complete
**Parallelism:** All 4 agents simultaneously
**Definition of Done:** `UNDERSTAND "..."` parses correctly for 90% of test cases; Granger test identifies known causal pairs in synthetic data; LTR ranking improves after feedback; cost-based optimizer selects correct plans.

---

### Agent 6-A: Semantic Interface Tier 1-2

**Crate:** `fundb-semantic`

#### Tasks
1. Implement Tier 1 — Rule-Based Intent Patterns (`<1ms`):
   - Pattern: `"find X similar to Y"` → `VectorSearch(X, query=Y)`
   - Pattern: `"X connected to Y via Z"` → `GraphTraverse(Z, from=X, to=Y)`
   - Pattern: `"X before/after Y"` → `TemporalQuery`
   - Pattern: `"papers/documents/records about X"` → `VectorSearch(X)`
   - 20+ base patterns covering common intents
2. Implement entity extraction:
   - Collection name matching (catalog lookup)
   - Entity recognition: quoted strings, proper nouns
   - Relationship words: "cited by", "related to", "caused by", "after", "similar to"
   - Time expressions: "last 2 years", "before 2025", "recent"
3. Implement FunQL generator from structured intent:
   - Intent graph → FunQL AST
   - Validate against catalog (collections, fields exist?)
   - Return FunQL string + confidence score (0.0–1.0)
4. Implement Tier 2 — Lightweight ML Classifier (`<5ms`):
   - Feature extraction from tokenized query
   - Gradient boosted tree classifier (via `lightgbm` Rust bindings or custom)
   - Intent classes: VECTOR_SEARCH, GRAPH_TRAVERSAL, TEMPORAL, HYBRID, CAUSAL, MEMORY
   - Training data: synthetic query-intent pairs
5. Implement `UNDERSTAND "..."` statement execution:
   - Try Tier 1 first
   - If confidence < 0.7, try Tier 2
   - If still < 0.5, return top-3 candidate FunQL queries for agent to choose
6. Write tests: 200 natural language queries, verify correct FunQL generated for 90%

---

### Agent 6-B: Causal Engine Tier 2 (Granger + Temporal + LLM Oracle)

**Crate:** `fundb-causal`

#### Tasks
1. Implement Granger Causality test:
   - Vector Autoregression (VAR) model fitting on time-series pairs
   - F-test for Granger causality: `H0: X does not Granger-cause Y`
   - Return: F-statistic, p-value, optimal lag selection (AIC/BIC)
   - Multivariate extension: MVAR Granger for multiple series
2. Implement temporal precedence causality:
   - For events in same context: if A consistently precedes B, link as `PRECEDED`
   - Statistical threshold: Pearson correlation of time-shifted series
3. Implement `DISCOVER CAUSAL STRUCTURE IN COLLECTION :c` execution:
   - For time-series collections: run Granger tests on all variable pairs
   - For event collections: run temporal precedence analysis
   - Store discovered edges in causal DAG (Tier 1 storage) with `source: 'granger'` or `source: 'temporal'`
4. Implement LLM-Oracle protocol (Tier 2.5):
   - Build context prompt from collection schema + sample data + prior known edges
   - Call Semantic Interface (which calls embedded LLM or external API)
   - Parse LLM output: extract hypothesized causal pairs
   - Run statistical validation test for each hypothesis
   - Persist confirmed edges with `source: 'llm-validated'` and `confidence = f(p_value)`
5. Write tests: synthetic time-series with known causal structure, verify Granger recovers edges

---

### Agent 6-C: Bidirectional Learning (Learning-to-Rank)

**Crate:** `fundb-learning`

#### Tasks
1. Implement telemetry collection:
   - Log every query execution: query hash, result IDs, positions returned
   - Log usage signals: which results the agent subsequently read (via SDK callback)
   - Store as lightweight event log per agent_id
2. Implement feature vector for each (query, document) pair:
   - Vector similarity score
   - Confidence score
   - Recency (age of document)
   - Source count
   - Previous access count by this agent
   - Position in original ranking
3. Implement LambdaMART (simplified):
   - Gradient boosted trees targeting NDCG
   - Online incremental updates (new feedback → update leaf weights)
   - Per-agent models stored as small files (~1MB each)
4. Implement re-ranking operator in executor:
   - After initial vector search, apply LTR model to rerank
   - Only apply if agent has > 100 feedback signals (cold start fallback = original ranking)
5. Implement feedback API in SDK:
   ```python
   db.feedback(query_id="...", used_result_ids=["id1", "id3"], ignored_result_ids=["id2"])
   ```
6. Write tests: generate synthetic feedback, verify ranking improves (NDCG increases)

---

### Agent 6-D: Cost-Based Optimizer + Adaptive Index Advisor

**Crate:** `fundb-optimizer`
**Depends on:** Agent 4-B (rule-based optimizer)

#### Tasks
1. Implement Cascades/Columbia framework:
   - Memo structure: equivalence classes of plans
   - Transformation rules: produce alternative logical plans
   - Implementation rules: logical → physical operators
   - Top-down optimization with pruning
2. Implement cost model for physical operators:
   - `SeqScan`: rows × row_size / io_bandwidth
   - `IndexScan(B+Tree)`: log(rows) × index_access_cost + result_rows × fetch_cost
   - `VectorANN(HNSW)`: ef_search × dimension × distance_cost
   - `GraphTraverse`: branching_factor^depth × node_fetch_cost
   - `CausalPathScan`: DAG_size × (1 - transitive_cache_hit_rate) × traverse_cost
   - `HashJoin`: build_cost + probe_cost
3. Implement statistics collection:
   - Background job updates column histograms from sample of recent data
   - Confidence histogram per collection
   - Index size and fill factor
4. Implement Adaptive Index Advisor:
   - Query pattern clustering (gradient boosted tree, ~10MB)
   - Missing index detection: `rows_scanned / rows_returned > 100` → suggest index
   - Unused index detection: index not used in last N days
   - Causal transitive closure recommendation: top-K frequent path queries
5. Write tests: verify cost-based optimizer chooses index scan over seq scan when selectivity < 1%

---

## Wave 7 — Distribution

**Prerequisite:** Wave 4 + Wave 5-A (confidence propagation in distributed context)
**Parallelism:** 3 agents, but 7-A must complete before 7-B/7-C fully integrate
**Definition of Done:** 3-node cluster handles writes with Raft quorum; distributed ANN query returns correct results; tenant isolation holds across nodes.

---

### Agent 7-A: Raft Consensus

**Crate:** `fundb-raft`

#### Tasks
1. Implement Raft state machine: Follower, Candidate, Leader transitions
2. Implement leader election: randomized timeouts, RequestVote RPC
3. Implement log replication: AppendEntries RPC, commit index advancement
4. Implement membership changes: one-at-a-time (safe) membership reconfiguration
5. Implement log compaction: snapshots to avoid unbounded log growth
6. Implement WAL-backed Raft log (reuse Wave 2-B WAL format)
7. Use `tarpc` or `tonic` (gRPC) for inter-node RPCs
8. Write tests: 5-node cluster, kill leader, verify new leader elected, log consistent

---

### Agent 7-B: Shard Manager

**Crate:** `fundb-cluster`

#### Tasks
1. Implement consistent hashing ring:
   - Virtual nodes (100 per physical node)
   - Key → shard mapping: `hash(collection + _id) → shard`
   - Rebalancing: when nodes join/leave, migrate minimal number of shards
2. Implement shard map:
   - In-memory map: `shard_id → [leader_addr, follower_addrs]`
   - Stored in control plane (Raft-replicated metadata)
3. Implement routing layer:
   - Client request → determine which shard(s) own the data
   - Route to appropriate shard leader
   - For scatter queries: fan-out to all shards
4. Implement tenant-to-shard affinity: all records for a tenant map to same shard group
5. Write tests: 4-shard cluster, verify records route to correct shard, rebalancing moves minimal data

---

### Agent 7-C: Distributed Query Execution

**Crate:** `fundb-cluster`
**Depends on:** 7-B

#### Tasks
1. Implement query fragmenter: split physical plan into coordinator fragment + shard fragments
2. Implement scatter: coordinator sends fragment to each relevant shard
3. Implement gather: merge results from shards:
   - For scalar queries: merge sorted streams
   - For ANN queries: merge-rerank (each shard returns K_local, coordinator merges top-K_global)
   - For graph traversal: union node sets per depth level, dedup
   - For causal paths: merge path sets, combine strengths
   - For aggregations: partial aggregation on shards → final aggregation on coordinator
4. Implement distributed confidence propagation: merge confidence from multiple shards
5. Implement distributed WITHIN CONTEXT: gather candidates from all shards, MMR on coordinator
6. Write tests: 3-shard cluster, verify distributed ANN result matches single-node result

---

## Wave 8 — Protocol + SDKs

**Prerequisite:** Wave 4 core (single-node queries work end-to-end)
**Parallelism:** All 4 agents simultaneously
**Definition of Done:** Python SDK can execute all ARCHITECTURE.md example queries; `psql` works; gRPC API accessible.

---

### Agent 8-A: PostgreSQL Wire Protocol (complete)

**Crate:** `fundb-protocol`
**Extends:** Wave 1-D skeleton

#### Tasks
1. Implement extended query protocol (prepare/bind/execute cycle)
2. Implement `RowDescription` with correct field types (including custom types: vector, causal_path)
3. Implement `DataRow` serialization for all FunRecord field types
4. Implement `COPY` protocol for bulk data loading
5. Implement connection pooling support (server-side)
6. Implement authentication: MD5, SCRAM-SHA-256
7. Handle `EXPLAIN` and `EXPLAIN ANALYZE` output
8. Write tests: psql + psycopg2 + asyncpg compatibility

---

### Agent 8-B: gRPC + REST API

**Crate:** `fundb-protocol`

#### Tasks
1. Define Protobuf schema (`fundb.proto`):
   - `ExecuteQuery(query: string, params: map) → QueryResult`
   - `StreamQuery(query: string) → stream RecordBatch`
   - `Understand(intent: string, constraints: UnderstandOptions) → QueryResult`
   - `RecordFeedback(query_id: string, used_ids: [], ignored_ids: [])`
   - `DiscoverCausalStructure(collection: string, options: CausalDiscoveryOptions) → CausalGraph`
2. Implement gRPC service (via `tonic`)
3. Implement REST API (via `axum`):
   - `POST /query` → execute FunQL
   - `POST /understand` → Semantic Interface
   - `GET /collections` → list collections
   - `POST /collections/:name/causal/discover` → trigger causal discovery
   - `GET /collections/:name/causal/graph` → get causal graph
4. Implement OpenAPI 3.0 spec generation
5. Write tests: HTTP integration tests, gRPC integration tests

---

### Agent 8-C: Python SDK

**Location:** `sdks/python/`

#### Tasks
1. Implement `AsyncConnection` and `SyncConnection` classes
2. Implement query execution: `conn.execute(query, params)` → `ResultSet`
3. Implement `ResultSet`: iterable, `.fetchall()`, `.fetchone()`, `.to_pandas()`
4. Implement helper methods:
   - `conn.insert(collection, data, vectors=None, confidence=None, sources=None)`
   - `conn.semantic_search(collection, query_text, top_k=10, confidence_min=None)`
   - `conn.understand(intent, **options)` → `ResultSet`
   - `conn.trace_causality(from_id, to_id, max_depth=5)` → `CausalPath`
   - `conn.estimate_effect(model, set_vars, predict_var)` → `InterventionResult`
5. Implement agent memory helpers:
   - `memory.remember(agent_id, content, importance=0.5)`
   - `memory.recall(agent_id, query, top_k=20)`
6. Implement feedback API: `conn.feedback(query_id, used_ids, ignored_ids)`
7. Publish to PyPI: `pip install fundb`
8. Write test suite + cookbook notebooks (RAG, agent memory, causal)

---

### Agent 8-D: Go SDK

**Location:** `sdks/go/`

#### Tasks
1. Implement `Client` struct with connection pool
2. Implement `Query(ctx, sql, args...) (*Rows, error)`
3. Implement typed result scanning: `rows.Scan(&field1, &field2)`
4. Implement helper methods matching Python SDK
5. Implement context propagation for cancellation/timeout
6. Publish to pkg.go.dev: `go get github.com/fundb/fundb-go`
7. Write test suite

---

## Wave 9 — Experimental Features

**Prerequisite:** Wave 6 complete
**Parallelism:** 2 agents simultaneously
**Definition of Done:** SCM can be defined, interventional query returns correct result on synthetic data; Tier 3 UNDERSTAND uses embedded LLM for complex queries.

---

### Agent 9-A: Causal Engine Tier 3 (SCM + do-calculus + Counterfactuals)

**Crate:** `fundb-causal`

#### Tasks
1. Implement `CREATE CAUSAL MODEL` DDL:
   - Store SCM definition: variables, edges (structure), equations (functions)
   - Validate DAG structure
2. Implement functional equation fitting:
   - Linear regression per variable: `Y = β0 + β1 X1 + β2 X2 + ε`
   - Store learned coefficients
3. Implement do-calculus (graph surgery):
   - Intervention: remove all incoming edges to intervened variable, set to fixed value
   - Propagate through SCM using fitted equations
   - Return point estimate + confidence interval (bootstrap)
4. Implement `INTERVENE ON model SET var=val PREDICT target` execution
5. Implement twin network counterfactual:
   - Step 1 (Abduction): infer exogenous noise values from observed data
   - Step 2 (Action): set counterfactual variable value, remove incoming edges
   - Step 3 (Prediction): propagate through modified SCM
6. Implement `COUNTERFACTUAL ON model GIVEN observed HAD var=val PREDICT target` execution
7. Implement PC algorithm for automated causal discovery (constraint-based):
   - Start with complete undirected graph
   - Remove edges using conditional independence tests
   - Orient v-structures
   - Apply Meek rules
8. Implement NOTEARS (continuous optimization) as alternative to PC:
   - Minimize `||X - XW||^2_F s.t. h(W) = 0` where `h(W) = tr(e^W - I) - d = 0`
   - Use L-BFGS optimizer
9. Write tests: synthetic linear SCM, verify intervention result matches ground truth

---

### Agent 9-B: Semantic Interface Tier 3 (Embedded LLM)

**Crate:** `fundb-semantic`

#### Tasks
1. Integrate ONNX Runtime for embedded model inference
2. Implement model manager: download, cache, and load ONNX models
3. Implement prompt construction for query intent parsing:
   - System prompt: FunDB schema, FunQL grammar reference
   - User prompt: natural language query
   - Output format: structured JSON with intent + generated FunQL
4. Implement output parser: extract FunQL from LLM response, validate against catalog
5. Implement fallback chain:
   - Tier 1 (rules) → Tier 2 (ML) → Tier 3 (LLM)
   - Only call Tier 3 if lower tiers return confidence < threshold
6. Implement candidate query mode:
   - If Tier 3 confidence < 0.5, return top-3 candidate FunQL queries
7. Write tests: complex ambiguous queries that require Tier 3

---

## Wave 10 — Production Hardening

**Prerequisite:** Waves 1-9
**Parallelism:** All 4 agents simultaneously

---

### Agent 10-A: Security

#### Tasks
1. Implement TLS/mTLS for all server connections
2. Implement RBAC:
   - `CREATE ROLE`, `GRANT`, `REVOKE` DDL
   - Permission checks on every collection/operation
3. Implement 3-tier multi-tenant isolation:
   - Tenant isolation at storage level (separate column page groups)
   - Query planner: reject cross-tenant queries
   - Network: tenant-scoped API keys
4. Implement audit log: every query, by whom, when, what collections accessed
5. Implement secrets management: encrypted credential storage

---

### Agent 10-B: Observability

#### Tasks
1. Implement Prometheus metrics exporter:
   - Query latency histograms (per query type)
   - Write throughput (ops/sec)
   - Replication lag
   - Confidence propagation cache hit rate
   - Causal path cache hit rate
   - LTR model accuracy (NDCG per agent)
2. Implement OpenTelemetry distributed tracing:
   - Trace per query, span per operator, span per shard
3. Implement structured logging (JSON, with trace correlation IDs)
4. Create Grafana dashboard templates

---

### Agent 10-C: Benchmarks + Performance Validation

#### Tasks
1. Implement benchmark harness (`fundb-bench`):
   - Data generator: synthetic FunRecords with vectors, edges, causal links
   - Workload generator: configurable mix of query types
2. Run and record baseline benchmarks against performance targets from ARCHITECTURE.md:
   - Vector search p99 < 5ms @ 10M vectors
   - Write throughput > 100K ops/sec
   - Graph traversal p99 < 10ms @ 3 hops, 1M edges
   - Context retrieval p99 < 25ms
   - Causal path p99 < 15ms
3. Implement regression detection: fail CI if any metric degrades > 10%
4. Run comparison benchmarks against pgvector, Weaviate, Qdrant on shared workload

---

### Agent 10-D: Correctness Testing

#### Tasks
1. Implement property-based test suite (via `proptest`):
   - Storage: encode → decode round-trip for all record types
   - MVCC: time-travel always returns correct version
   - HNSW: recall@10 > 0.95 on random vectors
   - Confidence propagation: algebra laws hold
   - Causal DAG: no cycles ever stored
2. Implement chaos engineering harness:
   - Random node kills during write workload → verify no data loss
   - Network partition simulation → verify Raft correctness
   - Disk full simulation → verify graceful degradation
3. Implement correctness test comparing FunDB results to known-correct implementations:
   - SQL: compare to DuckDB for relational queries
   - Vector: compare to FAISS brute-force for ANN correctness
   - Causal: compare to `causalnex` Python library for Granger tests

---

## Open Questions Tracking

**Legend:** RESOLVED = decision made, see `docs/architecture/causal-design-decisions.md` | DECIDED = decision below | OPEN = still needs work

| # | Question | Status | Decision |
|---|----------|--------|----------|
| OQ-1 | How to handle non-DAG causal graphs (feedback loops)? | **RESOLVED** | Temporal unrolling as default (DAG strict); `MODE EQUILIBRIUM` opt-in for steady-state systems with fixed-point iteration. See Decision 1 in causal-design-decisions.md |
| OQ-2 | Training data for Tier 2 intent classifier? | **DECIDED** | Generate synthetic query-intent pairs from FunQL grammar + collection schemas at startup. Augment with user query logs after 1000 real queries. Bootstrap problem solved by rule-based Tier 1 covering the first 1000 queries. |
| OQ-3 | Confidence with equal contradicting sources? | **DECIDED** | When `N` sources with confidence `c` contradict: `final_confidence = 0.5 + (1/(2N))`. Two equal sources → 0.75. Three equal sources → 0.67. As N → ∞, converges to 0.5 (maximum uncertainty). Never goes below 0.5 — that would be confidence the negative is true, which requires its own evidence. |
| OQ-4 | HNSW recall for variable-density vector spaces? | **DECIDED** | Adaptive `ef_search`: start at 50, increase by 50 until either recall target met (verified by sampling 1% of results against brute-force) or max ef_search=500 reached. Cost: at most 10× slower for pathological distributions. Collection-level `recall_target` config (default 0.95). |
| OQ-5 | Token estimation function for different document types? | **DECIDED** | `tokens = max(byte_length / 4, vector_dims * 6 + scalar_fields * 3)`. Over-estimate deliberately: 20% safety margin built in. Wrong by ±15% in practice — acceptable since `max_tokens` is a soft budget, not a hard cutoff. |
| OQ-6 | LLM hallucinations in causal oracle? | **RESOLVED** | Three-Layer Shield: (1) statistical validation, (2) confidence ceiling 0.60 for LLM-originated edges, (3) counterfactual coherence check on 20% holdout. See Decision 2 in causal-design-decisions.md |
| OQ-7 | Non-stationarity in Granger tests? | **RESOLVED** | ADF pre-check → differencing → Johansen cointegration → rolling window Granger → stability_score. New `stability_score` and `stability_status` fields in CausalEdge. See Decision 3 in causal-design-decisions.md |
| OQ-8 | Linear only or nonlinear SCM equations? | **RESOLVED** | Linear (v1 mandatory) + GAM/additive (v1.5 bonus) + Neural SCM (v2 research track, no commitment). `EQUATION_TYPE` enum in FunQL extensible. See Decision 4 in causal-design-decisions.md |
| OQ-9 | Confidence index in distributed propagation? | **DECIDED** | Each shard computes local confidence propagation independently. Coordinator merges using `min()` for JOIN-semantics (most conservative). Confidence metadata included in scatter/gather protocol as a fixed 4-byte float per row — negligible network overhead. |
| OQ-10 | NOTEARS doesn't converge fallback? | **RESOLVED** | Ensemble voting: PC + NOTEARS + Granger. Edge persists if ≥2 algorithms agree. Direction conflicts marked `DIRECTION_UNCERTAIN`. `ALGORITHM 'notears'` mode available for explicit failure. See Decision 5 in causal-design-decisions.md |

**All questions resolved.** Reference: [docs/architecture/causal-design-decisions.md](architecture/causal-design-decisions.md)

---

## Agent Quickstart Checklist

When you are assigned a task, do this first:

```
1. Read ARCHITECTURE.md sections relevant to your module
2. Read theory/ documents relevant to your module
3. Read your specification doc (specifications/<module>/<name>.md)
   If spec doesn't exist yet: WRITE IT FIRST, then implement
4. Check Open Questions table — if any OQ affects your module, investigate
5. Define your module's interface (traits/structs/functions) BEFORE implementation
6. Write tests for the interface BEFORE writing the implementation (TDD)
7. When done: update the specification doc with any decisions made during implementation
8. Report any new open questions discovered to the team
```

---

*Total implementation tasks: ~180 | Waves: 10 | Maximum parallelism: 5 agents simultaneously | Estimated total effort without parallelism: ~36 months | With max parallelism: ~8-10 months*
