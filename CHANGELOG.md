# Changelog

All notable changes to FunDB will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **INSERT pipeline**: Full end-to-end `INSERT INTO ... VALUES (...)` support — data is persisted to LSM-Tree storage and readable via `SELECT`
- **Real query handler** (`FunDBHandler`): Replaced `StubHandler` with a production handler that routes SQL through the full parse → bind → execute pipeline
- **`LogicalPlan::Insert`**: New plan variant for INSERT statements with proper binding (collections are created implicitly on first write)
- **RecordBatch → QueryResult**: SELECT queries now deserialize MessagePack record data and return real column values over the PostgreSQL wire protocol
- **VSCode extension**: `.funsql` file icons (light/dark themes), marketplace icon, `fundb://` connection string, README

### Changed
- Server now opens LSM-Tree storage at `./fundb_data` on startup instead of using a stub
- Binder no longer maps INSERT to `LogicalPlan::Empty` — it produces a full `LogicalPlan::Insert` with collection, columns, and values

---

## [0.1.0] - 2026-03-01

Initial release of FunDB -- the AI-native cognitive database.

### Added

#### Core Data Model
- **FunRecord** unified knowledge unit: document + vector + graph + time-series + causal + cognitive metadata in a single record
- Cognitive metadata as first-class fields: `_confidence`, `_sources`, `_supports`, `_contradicts`, `_caused_by`, `_effects`
- UUID v7 time-ordered record IDs
- CausalEdge with extended metadata (origin, stability, direction status)

#### Storage Engine (FunStore)
- LSM-tree write path: concurrent SkipList MemTable, append-only WAL with CRC32, SSTable with tiered+leveled hybrid compaction
- Bitemporal MVCC: system_time + valid_time dimensions for time-travel queries
- Columnar page groups for cognitive columns with confidence histogram page headers

#### Index Engines
- B+Tree index for scalar fields (equality, range, prefix queries)
- HNSW vector index with adaptive recall and Product Quantization compression
- Graph SPO triple store with three indexes (SPO, POS, OSP)
- Causal DAG index with cycle detection and transitive closure cache
- Temporal interval index for bitemporal range queries

#### Query Engine (FunQL)
- Complete FunQL parser: SQL superset with vector, graph, temporal, confidence, causal, context, semantic, and memory primitives
- Semantic binder with cognitive type system (`vector(n)`, `confidence`, `causal_path`, `context_result`)
- Rule-based optimizer with scalar-before-vector predicate pushdown and confidence early pruning
- Cost-based plan selection using column histograms and collection statistics
- Volcano-style executor with all physical operators: SeqScan, IndexScan, VectorANN, GraphTraverse, TemporalRangeScan, CausalPathScan, ConfidenceFilter, Projection, Filter, HashJoin, MergeJoin, Sort, Limit/TopK, HashAggregate, ContextOptimize

#### Confidence & Provenance
- Confidence propagation algebra (JOIN=min, UNION=max, AGGREGATE=weighted_avg, GRAPH_TRAVERSE=product, NEGATION=complement)
- Automatic confidence mutation on new corroborating/contradicting evidence
- Equal-evidence uncertainty formula: `0.5 + 1/(2N)`
- Contradiction detection with semantic polarity check

#### Context-Aware Retrieval
- `WITHIN CONTEXT` execution with MMR selection, token budget management, coherence threshold, and diversity control
- Context metadata in response: tokens_used, candidates_evaluated, avg_confidence, coherence_score

#### Causal Reasoning
- **Tier 1:** Explicit causal edges with cycle detection, `TRACE CAUSALITY` with MAX_DEPTH/MIN_STRENGTH/MIN_STABILITY
- **Tier 2:** Granger causality with ADF stationarity test, Johansen cointegration, rolling window stability; LLM-oracle protocol with Three-Layer Shield; ensemble discovery (PC + NOTEARS + Granger)
- **Tier 3:** Structural Causal Models (SCM) with linear equations, interventional queries (`ESTIMATE EFFECT OF SET(x=v) ON y`), counterfactual queries (Pearl's three-step: abduction, action, prediction)

#### Semantic Interface
- Tier 1: Rule-based intent parsing (<1ms, 20+ patterns)
- Tier 2: ML classifier fallback (<5ms)
- `UNDERSTAND` queries with low-confidence candidate fallback

#### Agent Memory
- `REMEMBER`, `RECALL BY`, `FORGET` commands
- Weighted scoring with semantic decay

#### Learning-to-Rank
- LambdaMART implementation with bidirectional feedback loop
- SDK feedback API: `feedback(query_id, used_ids, ignored_ids)`

#### Distribution
- Raft consensus with leader election and log replication
- Consistent hashing shard routing
- Distributed ANN merge-rerank across shards
- Cross-shard confidence propagation with min() merge strategy

#### Protocol & SDKs
- PostgreSQL wire protocol v3.0 (compatible with `psql`, `psycopg2`, `asyncpg`)
- Extended query protocol (Parse, Bind, Execute, Sync)
- SCRAM-SHA-256 and MD5 authentication
- COPY protocol for bulk data loading
- REST API: `/health`, `/query`, `/understand`, `/causal/trace`
- Python, Node.js, and Go SDK specifications

#### CLI
- Interactive REPL with multi-line SQL, meta-commands, history
- Output formats: table (psql-style), JSON, CSV
- Meta-commands: `\understand`, `\causal`, `\memory`, `\connect`, `\format`

#### Infrastructure
- Multi-stage Docker build (Rust 1.85 builder + Debian Bookworm runtime)
- Docker Compose: single-node (dev) and 3-node cluster profiles
- Multi-tenant RBAC with audit logging
- Prometheus metrics and OpenTelemetry tracing
- GitHub Actions CI/CD (test + clippy + fmt + release build + Docker build)

#### Testing
- 407 tests passing (100%): unit + integration + property-based
- 93 integration tests across 8 modules

[0.1.0]: https://github.com/fundb/fundb/releases/tag/v0.1.0
