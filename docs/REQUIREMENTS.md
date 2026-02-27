# FunDB — Requirements Document

**Version:** 1.0
**Status:** Approved
**Source:** ARCHITECTURE.md v0.3 + IMPLEMENTATION_PLAN.md + causal-design-decisions.md
**Format:** `REQ-[MODULE]-[NUMBER]` — each requirement is numbered, testable, and maps to an implementing agent.

**Priority:**
- `P0` — System cannot ship without this. Blocks everything.
- `P1` — Core differentiator. Required before public launch.
- `P2` — Important but deferrable to next release.

---

## Module Index

| Code | Module |
|------|--------|
| CORE | Core data model & codecs |
| STORE | Storage engine (LSM, WAL, MVCC) |
| IDX | Index engines |
| SQL | FunQL query language |
| OPT | Query optimizer |
| EXEC | Query executor |
| CONF | Confidence & provenance system |
| CTX | Context-aware retrieval |
| CONTRA | Contradiction detection |
| CAUSAL | Causal engine |
| SEM | Semantic Interface |
| LTR | Learning-to-Rank |
| DIST | Distribution & consensus |
| PROTO | Protocol & SDKs |
| PERF | Performance targets |
| SEC | Security |

---

## 1. Core Data Model (CORE)

### REQ-CORE-001 — FunRecord as unified knowledge unit
**P0 | Wave 1-A**

The system MUST store all data types (document, vector, graph edge, time-series sample, causal edge) in a single `FunRecord` structure within a single transaction.

**Acceptance criteria:**
- A single INSERT can atomically write: document payload + vector embeddings + graph edges + causal edges + confidence metadata
- All fields are consistent or the entire write is rolled back
- Round-trip encode/decode of any valid FunRecord produces byte-identical output

---

### REQ-CORE-002 — Cognitive metadata as first-class fields
**P0 | Wave 1-A**

Every `FunRecord` MUST natively carry:
- `_confidence: f32` (0.0–1.0) — trust score for this knowledge unit
- `_sources: []Source` — provenance chain with origin, method, and per-source confidence
- `_supports: []Ref` — records that corroborate this fact
- `_contradicts: []Ref` — records that contradict this fact
- `_caused_by: []CausalEdge` — what caused this record to be true
- `_effects: []CausalEdge` — what this record has caused

**Acceptance criteria:**
- These fields are stored in separate column segments (not embedded in the document blob)
- Query planner can filter and sort on these fields without deserializing the document payload

---

### REQ-CORE-003 — UUID v7 for time-ordered record IDs
**P0 | Wave 1-A**

Record IDs MUST be UUID v7 (time-ordered). Inserting 1000 records in sequence and sorting by ID MUST yield chronological order.

---

### REQ-CORE-004 — CausalEdge extended metadata
**P0 | Wave 1-A**
*(Resolved by OQ-6, OQ-7, OQ-10 in causal-design-decisions.md)*

`CausalEdge` MUST carry:
- `origin: CausalOrigin` — `UserDeclared | Granger{p_value, lag} | TemporalPrecedence{correlation} | LlmValidated{model, coherence_score}`
- `confidence: f32` — separate from `strength`; bounded by origin ceiling (LlmValidated ≤ 0.60)
- `stability_score: Option<f32>` — 0.0–1.0 for time-series edges, None otherwise
- `stability_status: StabilityStatus` — `Stable | Unstable | Provisional | NotApplicable`
- `direction_status: DirectionStatus` — `Confirmed | DirectionUncertain`
- `discovery_algo: Option<String>` — algorithm that produced this edge

---

## 2. Storage Engine (STORE)

### REQ-STORE-001 — LSM write path
**P0 | Wave 2**

Write path MUST use an LSM-tree architecture:
- Concurrent SkipList MemTable (lock-free, 64MB default flush threshold)
- Append-only WAL with CRC32 per record
- L0 SSTables (sorted, immutable) → L1..Ln with tiered+leveled hybrid compaction

**Acceptance criteria:**
- 16 concurrent writers to MemTable produce no data races (verified by `cargo test --features loom`)
- WAL recovery after truncation at any byte boundary produces consistent state
- 1M record write survives 10 compaction rounds with all records readable

---

### REQ-STORE-002 — Bitemporal MVCC
**P0 | Wave 2-D**

Every record version MUST carry two independent temporal dimensions:
- `system_time: [sys_from, sys_to)` — when the DB stored it (immutable)
- `valid_time: [valid_from, valid_to)` — application-defined validity window

**Acceptance criteria:**
- `AS OF SYSTEM TIME '2025-01-15'` returns the record as it existed at that system time
- `AS OF VALID TIME BETWEEN '2025-01-01' AND '2025-03-01'` returns records with overlapping valid windows
- Updating a record 5 times: time-travel at each intermediate timestamp returns correct version
- Snapshot isolation: reader sees only versions committed before transaction start

---

### REQ-STORE-003 — Columnar page groups for cognitive columns
**P1 | Wave 2-C**

Confidence, provenance, and causal edge columns MUST be stored in separate column segments from the document payload.

**Acceptance criteria:**
- A query `WHERE _confidence > 0.7` does NOT deserialize document payloads for filtered-out rows
- Page header contains confidence histogram enabling full-page skip when `max(confidence_in_page) < threshold`

---

## 3. Index Engines (IDX)

### REQ-IDX-001 — B+Tree for scalar fields
**P0 | Wave 3-A**

Scalar fields (integers, strings, timestamps) MUST be indexable via B+Tree supporting equality, range, and prefix queries.

**Acceptance criteria:**
- Insert 1M random integer keys, all readable via point lookup in O(log n)
- Range scan over 10K consecutive keys returns correct ordered results
- Property-based test: insert random keys → sorted scan yields sorted order (100% pass rate)

---

### REQ-IDX-002 — HNSW vector index with adaptive recall
**P0 | Wave 3-B**
*(Resolved by OQ-4)*

ANN vector search MUST use HNSW with:
- Configurable `M` (max connections, default 16) and `ef_construction` (default 200)
- Adaptive `ef_search`: starts at 50, increases by 50 until `recall_target` is met (default 0.95) or max 500
- Product Quantization compression for cold data (PQ64: 30× storage reduction)
- Per-tenant isolated graph partitions

**Acceptance criteria:**
- Build index with 100K 768-dim vectors, ANN top-10 achieves recall@10 ≥ 0.95 vs brute-force
- Adaptive ef_search never returns recall below `recall_target` for any collection density
- PQ64-compressed vectors decode without exceeding distance error > 5% vs full precision

---

### REQ-IDX-003 — Graph SPO triple store
**P0 | Wave 3-C**

Graph edges MUST be stored in a triple store with three indexes: SPO, POS, OSP.

**Acceptance criteria:**
- Build citation graph 10K nodes, 50K edges
- 3-hop BFS from any start node completes in < 50ms
- Weighted traversal: path score = product of edge strengths, verified against manual calculation

---

### REQ-IDX-004 — Causal DAG index with transitive closure cache
**P0 | Wave 3-E**

Causal edges MUST be stored in a DAG index that:
- Rejects cycles on insert (verified: attempting to insert cycle returns error)
- Caches transitive closures for top-K queried (source, target) pairs
- Invalidates cache entries when intervening edges change

**Acceptance criteria:**
- Insert cycle attempt: `A→B, B→C, C→A` — third insert returns `CycleError`
- `TRACE CAUSALITY FROM A TO C` with 1K nodes, 5 hops completes in < 15ms p99
- Path strength = `Π(edge_strengths)` verified numerically

---

### REQ-IDX-005 — Temporal interval index
**P0 | Wave 3-D**

Valid-time and system-time intervals MUST be indexed for efficient overlap queries.

**Acceptance criteria:**
- Insert 100K time intervals, `AS OF VALID TIME T` returns all containing intervals in < 5ms
- Range query `BETWEEN T1 AND T2` returns correct overlapping set vs brute-force scan

---

## 4. FunQL Query Language (SQL)

### REQ-SQL-001 — Complete FunQL grammar
**P0 | Wave 1-C / 4-A**

The FunQL parser MUST parse all query constructs defined in ARCHITECTURE.md Section 5.1:
- Standard SQL (SELECT, INSERT, UPDATE, DELETE, CREATE, DROP)
- Vector: `_vector(field) <-> value < threshold`, `ORDER BY _vector_distance`
- Graph: `TRAVERSE label(depth: n..m) -> collection`
- Temporal: `AS OF SYSTEM TIME`, `AS OF VALID TIME BETWEEN`
- Confidence: `_confidence > f`, `_source_count >= n`, `_contradiction_count = 0`
- Causal: `TRACE CAUSALITY FROM x TO y MAX_DEPTH n MIN_STRENGTH f`
- Context: `WITHIN CONTEXT (max_tokens: n, coherence: f, diversity: f, include_contradictions: bool)`
- Semantic: `UNDERSTAND "natural language"`
- Causal discovery: `DISCOVER CAUSAL STRUCTURE IN COLLECTION c ALGORITHM 'ensemble'`
- Interventional: `ESTIMATE EFFECT OF SET(x=v) ON y USING MODEL m`
- Counterfactual: `ESTIMATE COUNTERFACTUAL ... GIVEN observed HAD var=val PREDICT target`
- Memory: `REMEMBER`, `RECALL BY`, `FORGET`

**Acceptance criteria:**
- All example queries from ARCHITECTURE.md parse without error
- Invalid queries return structured `ParseError` with line and column

---

### REQ-SQL-002 — Type system with cognitive types
**P0 | Wave 4-A**

The binder MUST resolve and type-check: `vector(n)`, `confidence`, `causal_path`, `context_result`, in addition to standard SQL types.

---

## 5. Query Optimizer (OPT)

### REQ-OPT-001 — Scalar-before-vector predicate pushdown
**P0 | Wave 4-B**

When a query has both scalar filters and ANN vector search, the optimizer MUST apply scalar filters first to reduce the candidate set before ANN.

**Acceptance criteria:**
- Query `WHERE category='science' AND _vector(...) < 0.5` on 10M docs where category='science' is 1%:
  Optimizer chooses `B+Tree(category) → ANN on 100K subset` NOT `ANN on 10M → filter`
- Verified via `EXPLAIN` output showing `ConfidenceFilter` before `VectorANN` when applicable

---

### REQ-OPT-002 — Confidence early pruning
**P0 | Wave 4-B**

When `WHERE _confidence > threshold` is present, the optimizer MUST prune page groups whose `max(confidence) < threshold` without reading individual records.

---

### REQ-OPT-003 — Cost-based plan selection
**P1 | Wave 6-D**

The optimizer MUST choose between execution strategies (SeqScan vs IndexScan vs VectorANN) based on estimated cost using column histograms and collection statistics.

**Acceptance criteria:**
- When selectivity < 1%, optimizer chooses IndexScan over SeqScan (verified via EXPLAIN)
- When collection < 1000 rows, optimizer chooses SeqScan over IndexScan (lower overhead)

---

## 6. Query Executor (EXEC)

### REQ-EXEC-001 — Vectorized batch execution
**P0 | Wave 4-C**

The executor MUST process data in batches of 1024 rows. SIMD acceleration MUST be used for vector distance computations.

---

### REQ-EXEC-002 — All physical operators implemented
**P0 | Wave 4-C**

All operators listed in IMPLEMENTATION_PLAN Wave 4-C MUST be implemented:
`SeqScan, IndexScan, VectorANN, GraphTraverse, TemporalRangeScan, CausalPathScan, ConfidenceFilter, Projection, Filter, HashJoin, MergeJoin, Sort, Limit/TopK, HashAggregate, ContextOptimize`

**Acceptance criteria:**
- Integration test: every ARCHITECTURE.md example query executes end-to-end and returns correct results on single-node

---

## 7. Confidence & Provenance System (CONF)

### REQ-CONF-001 — Confidence propagation algebra
**P0 | Wave 5-A**

Confidence MUST propagate through query operators using defined algebraic rules:

| Operation | Rule |
|-----------|------|
| JOIN(A, B) | min(A._confidence, B._confidence) |
| UNION(A, B) | max(A._confidence, B._confidence) |
| AGGREGATE(group) | weighted_avg(group._confidence) |
| GRAPH_TRAVERSE path | Π(node_confidences × edge_strengths) |
| CAUSAL_CHAIN | Π(all strengths in chain) |
| NEGATION | 1.0 - A._confidence |

**Acceptance criteria:**
- Property-based test: JOIN commutativity holds (swapping A and B produces same confidence)
- Causal chain `A(0.9) → B(0.85) → C(0.7)` propagated confidence = `0.9 × 0.85 × 0.7 = 0.535` ± 0.001

---

### REQ-CONF-002 — Automatic confidence mutation on new evidence
**P0 | Wave 5-A**

When a new corroborating or contradicting record is inserted, existing record confidence MUST update automatically:
- New corroborating source: `confidence = min(1.0, confidence + 0.1)`
- New contradiction: `confidence = max(0.0, confidence - 0.2 × contradiction_confidence)`

---

### REQ-CONF-003 — Equal-evidence uncertainty formula
**P0 | Wave 5-A**
*(Resolved by OQ-3)*

When N sources with equal confidence contradict each other:
`final_confidence = 0.5 + 1/(2N)`

**Acceptance criteria:**
- N=2: result = 0.75
- N=3: result ≈ 0.667
- N=10: result = 0.55
- N→∞: approaches 0.5

---

## 8. Context-Aware Retrieval (CTX)

### REQ-CTX-001 — WITHIN CONTEXT execution
**P0 | Wave 5-B**

`WITHIN CONTEXT (max_tokens, coherence, diversity, include_contradictions)` MUST:
1. Oversample 10× the estimated final result count
2. Run MMR selection respecting token budget (`tokens = max(byte_length/4, vector_dims*6 + scalar_fields*3)`)
3. Enforce coherence threshold on selected set (pairwise average similarity ≥ `coherence`)
4. Include top contradiction per selected record if `include_contradictions: true`

**Acceptance criteria:**
- `max_tokens: 4000` never returns a result set exceeding 4400 tokens (10% margin)
- `diversity: 0.0` returns most relevant (no diversity penalty), `diversity: 1.0` maximizes diversity
- `coherence: 0.8` returns a set where pairwise similarity is ≥ 0.8

---

### REQ-CTX-002 — Context metadata in response
**P0 | Wave 5-B**

Every `WITHIN CONTEXT` query response MUST include:
`tokens_used, candidates_evaluated, candidates_selected, avg_confidence, coherence_score, contradictions_found`

---

## 9. Contradiction Detection (CONTRA)

### REQ-CONTRA-001 — Auto-detection on insert
**P1 | Wave 5-C**

When `contradiction_detection: enabled` is set on a collection, inserts MUST trigger semantic polarity check:
- Records with cosine_sim > 0.85 are candidates
- Candidates with opposing polarity (negation or antonym) are auto-linked as `_contradicts`

**Acceptance criteria:**
- Insert "The sky is blue" then "The sky is not blue": auto-linked as contradictions
- Confidence of both records decremented by `0.2 × contradiction_confidence`
- `CONTRADICTION` event emitted

---

## 10. Causal Engine (CAUSAL)

### REQ-CAUSAL-001 — Tier 1: Explicit causal edges
**P0 | Wave 5-D**

Users and agents MUST be able to declare causal edges explicitly:

```sql
INSERT INTO _causal_edges (source_id, target_id, relation, strength, mechanism)
VALUES (:a, :b, 'CAUSED', 0.85, 'explanation');
```

**Acceptance criteria:**
- Cycle detection: inserting edge that creates a cycle returns `CycleError`
- `TRACE CAUSALITY FROM :a TO :b` returns all paths with correct strength products
- Supports `MAX_DEPTH`, `MIN_STRENGTH`, `MIN_STABILITY` parameters

---

### REQ-CAUSAL-002 — Tier 1: Feedback loop handling via temporal unrolling
**P0 | Wave 5-D**
*(Resolved by OQ-1)*

Feedback loops (A→B→A) MUST be represented as temporal chains, not cyclic edges.
DAG invariant is strictly enforced for standard mode.
`MODE EQUILIBRIUM` opt-in allows simultaneous equations via fixed-point iteration (for steady-state economic/physical models).

**Acceptance criteria:**
- Inserting `A→B, B→A` in standard mode: second insert fails with `CycleError`
- `CREATE CAUSAL MODEL m MODE EQUILIBRIUM` accepts simultaneous equations and solves via iteration

---

### REQ-CAUSAL-003 — Tier 2: Granger causality with stationarity handling
**P0 | Wave 6-B**
*(Resolved by OQ-7)*

For time-series collections, `DISCOVER CAUSAL STRUCTURE` MUST:
1. Run ADF test on each series; apply differencing if non-stationary
2. Run Johansen cointegration test for cointegrated series; use VECM accordingly
3. Run rolling window Granger test; persist edges only if p-value < 0.05 in ≥60% of windows
4. Populate `stability_score` and `stability_status` on every discovered edge

**Acceptance criteria:**
- Synthetic time-series with known causal structure (A→B, A→C, B→D): Granger recovers all edges at p < 0.05
- Non-stationary series: ADF pre-check triggers differencing automatically, test runs on differenced series
- Stability score for a permanent causal edge ≈ 1.0; for a transient edge < 0.5

---

### REQ-CAUSAL-004 — Tier 2: LLM-oracle protocol with Three-Layer Shield
**P0 | Wave 6-B**
*(Resolved by OQ-6)*

LLM-generated causal hypotheses MUST pass:
1. Statistical validation (Granger or PC test with p < 0.05)
2. Confidence ceiling: `LlmValidated` edges start at confidence ≤ 0.60
3. Counterfactual coherence check on 20% holdout data

**Acceptance criteria:**
- LLM-suggested edge that fails Granger test is NOT persisted
- LLM-suggested edge that passes all three layers is persisted with `origin: LlmValidated`, `confidence ≤ 0.60`
- Human confirmation on an LLM edge raises confidence by 0.25 (up to 0.95 max)

---

### REQ-CAUSAL-005 — Tier 2: Ensemble causal discovery
**P0 | Wave 9-A**
*(Resolved by OQ-10)*

`DISCOVER CAUSAL STRUCTURE ALGORITHM 'ensemble'` MUST run PC + NOTEARS + Granger (where applicable) and merge results:
- Edge in ≥2 algorithms → confidence 0.75–0.85
- Edge in 1 algorithm → confidence 0.55
- Direction conflict → `DirectionUncertain` status

**Acceptance criteria:**
- Known 5-variable DAG (synthetic data): ensemble recovers ≥80% of edges correctly
- NOTEARS non-convergence after 3 restarts: ensemble continues with PC + Granger result, logs warning
- `DIRECTION_UNCERTAIN` edges visible via `WHERE direction_status = 'DIRECTION_UNCERTAIN'`

---

### REQ-CAUSAL-006 — Tier 3: SCM with interventional queries
**P1 | Wave 9-A**
*(Resolved by OQ-8 — linear v1 mandatory)*

`CREATE CAUSAL MODEL` with linear equations MUST support:
- `INTERVENE ON model SET var=val PREDICT target` → point estimate + confidence interval
- Graph surgery: incoming edges to intervened variable are removed during intervention

**Acceptance criteria:**
- Synthetic linear SCM (ground truth known): `INTERVENE ON m SET X=2 PREDICT Y` returns estimate within 5% of ground truth
- Confidence interval computed via bootstrap (1000 samples) covers ground truth 95% of the time

---

### REQ-CAUSAL-007 — Tier 3: Counterfactual queries
**P1 | Wave 9-A**

`ESTIMATE COUNTERFACTUAL ... GIVEN observed HAD var=val PREDICT target` MUST implement Pearl's three-step:
1. Abduction: infer exogenous noise from observed data
2. Action: set counterfactual value, remove incoming edges
3. Prediction: propagate through modified SCM

**Acceptance criteria:**
- Synthetic SCM: counterfactual result matches analytical solution within numerical precision

---

## 11. Semantic Interface (SEM)

### REQ-SEM-001 — Tier 1 rule-based parsing < 1ms
**P0 | Wave 6-A**

Common intent patterns (vector search, graph traversal, temporal queries) MUST be parsed by rule-based matching in < 1ms.

**Acceptance criteria:**
- 20+ base patterns cover ≥70% of test query set
- Latency p99 < 1ms for Tier 1 patterns (measured over 10K queries)

---

### REQ-SEM-002 — Tier 2 ML classifier < 5ms
**P1 | Wave 6-A**

Queries not matched by Tier 1 MUST fall through to a lightweight ML classifier returning intent class in < 5ms.

**Acceptance criteria:**
- F1 ≥ 0.85 on intent classification test set of 500 labeled queries
- Latency p99 < 5ms

---

### REQ-SEM-003 — Low-confidence fallback returns candidates
**P0 | Wave 6-A**

When confidence < 0.50, `UNDERSTAND` MUST return top-3 candidate FunQL queries for the agent to select, rather than executing speculatively.

---

## 12. Learning-to-Rank (LTR)

### REQ-LTR-001 — Bidirectional feedback loop
**P1 | Wave 6-C**

The system MUST improve ranking for each agent based on their usage signals (which results were used vs ignored).

**Acceptance criteria:**
- After 100+ feedback signals, NDCG@10 for that agent increases compared to baseline (no LTR)
- Cold start (< 100 signals): system falls back to base vector ranking

---

### REQ-LTR-002 — SDK feedback API
**P1 | Wave 6-C**

Client SDKs MUST expose a `feedback(query_id, used_ids, ignored_ids)` method.

---

## 13. Distribution (DIST)

### REQ-DIST-001 — Raft consensus with leader election
**P0 | Wave 7-A**

**Acceptance criteria:**
- 5-node cluster: kill leader node → new leader elected within 3 election timeout periods
- Log consistent across all nodes after re-join of failed node (no entries lost)

---

### REQ-DIST-002 — Consistent hashing shard routing
**P0 | Wave 7-B**

Record routing MUST use consistent hashing. Adding/removing a node moves no more than `K/N` records (where K = total records, N = total nodes).

---

### REQ-DIST-003 — Distributed ANN merge-rerank
**P0 | Wave 7-C**

Distributed ANN queries MUST guarantee recall equivalent to centralized search when `K_local ≥ K_global × 3`.

**Acceptance criteria:**
- 3-shard cluster, 1M vectors per shard: distributed ANN top-10 matches centralized brute-force with recall ≥ 0.95

---

### REQ-DIST-004 — Distributed confidence propagation
**P0 | Wave 7-C**
*(Resolved by OQ-9)*

Cross-shard confidence propagation MUST use `min()` merge strategy (most conservative).
Confidence metadata MUST be included in scatter/gather protocol as a fixed 4-byte float per row.

---

## 14. Protocol & SDKs (PROTO)

### REQ-PROTO-001 — PostgreSQL wire protocol compatibility
**P0 | Wave 8-A**

**Acceptance criteria:**
- `psql`, `psycopg2`, and `asyncpg` can connect and execute all standard SQL queries
- `EXPLAIN` and `EXPLAIN ANALYZE` return valid output

---

### REQ-PROTO-002 — Python SDK with cognitive helpers
**P0 | Wave 8-C**

Python SDK MUST expose:
- `conn.execute(funql, params)` → ResultSet
- `conn.semantic_search(collection, query_text, ...)` → ResultSet
- `conn.understand(intent, **options)` → ResultSet
- `conn.trace_causality(from_id, to_id, max_depth)` → CausalPath
- `conn.estimate_effect(model, set_vars, predict_var)` → InterventionResult
- `memory.remember(agent_id, content, importance)`, `memory.recall(agent_id, query, top_k)`
- `conn.feedback(query_id, used_ids, ignored_ids)`

---

### REQ-PROTO-003 — Go SDK
**P1 | Wave 8-D**

Go SDK with equivalent interface to Python SDK.

---

## 15. Performance Requirements (PERF)

All targets from ARCHITECTURE.md Section 19 are requirements. Key ones:

| REQ | Metric | Target | Conditions |
|-----|--------|--------|------------|
| REQ-PERF-001 | Vector search p99 | < 5ms | 10M vectors, 768d, top-10, single node |
| REQ-PERF-002 | Vector search distributed p99 | < 15ms | 100M vectors, 768d, top-10, 3 shards |
| REQ-PERF-003 | Point read p99 | < 2ms | Single document by _id |
| REQ-PERF-004 | Document write p99 | < 5ms | Single doc + 1 embedding + confidence, Raft quorum |
| REQ-PERF-005 | Graph traversal p99 | < 10ms | 3-hop BFS, 1M edges |
| REQ-PERF-006 | Causal path query p99 | < 15ms | 5-hop, 1M causal edges, transitive cache |
| REQ-PERF-007 | Context retrieval p99 | < 25ms | 10M docs, 4000 token budget, MMR |
| REQ-PERF-008 | Write throughput | > 100K ops/s | Per node, EVENTUAL consistency, batched |
| REQ-PERF-009 | Granger causality test | < 500ms | Two series, 10K points, 12 lags |
| REQ-PERF-010 | Semantic intent Tier 1 | < 1ms | Rule-based |
| REQ-PERF-011 | Semantic intent Tier 2 | < 5ms | ML classifier |

---

## 16. Security (SEC)

### REQ-SEC-001 — Multi-tenant data isolation
**P0 | Wave 10-A**

A query in tenant A MUST never return data belonging to tenant B.

**Acceptance criteria:**
- Query planner injects tenant filter on every collection scan
- Bypass attempt via raw SQL injection returns error, not data

---

### REQ-SEC-002 — TLS for all connections
**P1 | Wave 10-A**

All client-server and inter-node connections MUST support TLS 1.3.

---

### REQ-SEC-003 — RBAC
**P1 | Wave 10-A**

Role-based access control MUST support collection-level READ / WRITE / ADMIN permissions.

---

## Requirements Traceability Matrix

| Requirement | Architecture Section | Implementation Wave | Agent |
|-------------|---------------------|--------------------|----|
| REQ-CORE-001 | §4.3 FunRecord | Wave 1-A | Agent 1-A |
| REQ-CORE-002 | §4.3, §7 | Wave 1-A | Agent 1-A |
| REQ-CORE-004 | causal-design-decisions §1-5 | Wave 1-A | Agent 1-A |
| REQ-STORE-001 | §4.1 LSM | Wave 2-A/B/C/D | Agents 2-A…D |
| REQ-STORE-002 | §4.2 Bitemporal | Wave 2-D | Agent 2-D |
| REQ-IDX-002 | §9.2 HNSW | Wave 3-B | Agent 3-B |
| REQ-IDX-004 | §9.3 Causal | Wave 3-E | Agent 3-E |
| REQ-SQL-001 | §5.1 FunQL | Wave 1-C, 4-A | Agents 1-C, 4-A |
| REQ-CONF-001 | §7.2 Propagation | Wave 5-A | Agent 5-A |
| REQ-CONF-003 | OQ-3 resolution | Wave 5-A | Agent 5-A |
| REQ-CTX-001 | §8.3 MMR | Wave 5-B | Agent 5-B |
| REQ-CAUSAL-001 | §14.1 Tier 1 | Wave 5-D | Agent 5-D |
| REQ-CAUSAL-002 | OQ-1 resolution | Wave 5-D | Agent 5-D |
| REQ-CAUSAL-003 | §14.2, OQ-7 | Wave 6-B | Agent 6-B |
| REQ-CAUSAL-004 | §14.3, OQ-6 | Wave 6-B | Agent 6-B |
| REQ-CAUSAL-005 | OQ-10 | Wave 9-A | Agent 9-A |
| REQ-CAUSAL-006 | §14.4 SCM, OQ-8 | Wave 9-A | Agent 9-A |
| REQ-CAUSAL-007 | §14.4 Counterfactuals | Wave 9-A | Agent 9-A |
| REQ-SEM-001 | §6.2 Tier 1 | Wave 6-A | Agent 6-A |
| REQ-DIST-004 | OQ-9 | Wave 7-C | Agent 7-C |
| REQ-PERF-001…011 | §19 Performance | Wave 10-C | Agent 10-C |

---

*Total requirements: 41 | P0: 28 | P1: 11 | P2: 2*
*Every requirement is testable. Every requirement maps to an implementation wave and agent.*
