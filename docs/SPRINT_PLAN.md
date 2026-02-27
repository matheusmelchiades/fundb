# FunDB — Sprint Execution Plan

**Version:** 1.0
**Strategy:** 2-week sprints. Each sprint has a clear goal, set of agents, and definition of done.
**To start:** Run Sprint 0 today.

---

## How to read this plan

```
Sprint N
  Goal: One sentence — what works at the end of this sprint
  Agents: Who works in parallel
  Requirements covered: REQ-XXX-YYY links
  Definition of Done (DoD): What must be true before Sprint N+1 starts
  Blockers: Nothing can start Sprint N+1 if these fail
```

Each agent task below maps directly to a Wave in IMPLEMENTATION_PLAN.md.
Sprint = 2 weeks. Max 5 agents in parallel per sprint.

---

## Sprint 0 — Workspace & Foundations
**Goal:** Cargo workspace compiles; FunRecord round-trips; FunQL tokenizes; TCP server accepts connections.
**Duration:** 2 weeks
**Agents in parallel:** 4

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | Cargo workspace setup + FunRecord data model | Wave 1-A | REQ-CORE-001, REQ-CORE-002, REQ-CORE-004 |
| Agent-B | Codec (MessagePack + FlatBuffers + columnar pages) | Wave 1-B | REQ-STORE-003 |
| Agent-C | FunQL grammar, lexer, and parser skeleton | Wave 1-C | REQ-SQL-001 (partial) |
| Agent-D | PostgreSQL wire protocol TCP skeleton | Wave 1-D | REQ-PROTO-001 (partial) |

### DoD Sprint 0
- [ ] `cargo build --workspace` succeeds with zero errors
- [ ] `FunRecord` with all fields encodes/decodes round-trip (property-based test: 10K random inputs pass)
- [ ] FunQL tokenizer correctly tokenizes all example queries from ARCHITECTURE.md §5.1
- [ ] `psql -h localhost` connects and receives `ReadyForQuery` message (no actual queries yet)
- [ ] UUID v7 generation: 1000 sequential IDs sort chronologically

### Blockers for Sprint 1
- FunRecord struct must be final — storage engine is built on top of it. Field additions after Sprint 0 require cascading changes.
- Codec must produce stable binary format — changing it later breaks all stored data.

---

## Sprint 1 — Storage Engine
**Goal:** FunRecords persist durably to disk and survive crash recovery.
**Duration:** 2 weeks
**Agents in parallel:** 4
**Depends on:** Sprint 0 complete (FunRecord + Codec)

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | MemTable (concurrent skip-list + WAL write-through) | Wave 2-A | REQ-STORE-001 |
| Agent-B | WAL (append-only log + crash recovery) | Wave 2-B | REQ-STORE-001 |
| Agent-C | SSTable format + block reader/writer + bloom filter | Wave 2-C | REQ-STORE-001 |
| Agent-D | MVCC bitemporal versioning layer | Wave 2-D | REQ-STORE-002 |

### DoD Sprint 1
- [ ] Write 1M FunRecords → kill process → restart → all 1M readable (crash recovery test)
- [ ] `AS OF SYSTEM TIME T` returns correct version after 5 updates to same record
- [ ] `AS OF VALID TIME BETWEEN T1 AND T2` returns correct overlapping records
- [ ] 16 concurrent writers to MemTable: no data races (Loom or ThreadSanitizer)
- [ ] SSTable bloom filter achieves < 1% false positive rate on 100K key set

### Blockers for Sprint 2
- MVCC must support snapshot reads — indexes in Sprint 2 build on top of this.

---

## Sprint 2 — Index Engines
**Goal:** Vector search, graph traversal, and causal path queries work on single node.
**Duration:** 2 weeks
**Agents in parallel:** 5
**Depends on:** Sprint 1 complete

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | B+Tree index (scalar, string, temporal) | Wave 3-A | REQ-IDX-001 |
| Agent-B | HNSW vector index + adaptive ef_search + PQ compression | Wave 3-B | REQ-IDX-002 |
| Agent-C | Graph SPO triple store + BFS/DFS traversal | Wave 3-C | REQ-IDX-003 |
| Agent-D | Temporal interval index (bitemporal range queries) | Wave 3-D | REQ-IDX-005 |
| Agent-E | Causal DAG index + confidence histogram index | Wave 3-E | REQ-IDX-004 |

### DoD Sprint 2
- [ ] HNSW: 100K vectors, recall@10 ≥ 0.95 vs brute-force
- [ ] HNSW: adaptive ef_search never returns recall below 0.95 target
- [ ] Graph: 10K nodes, 50K edges, 3-hop BFS completes < 50ms
- [ ] Causal DAG: cycle insertion rejected with `CycleError`
- [ ] Causal DAG: `TRACE CAUSALITY FROM A TO C` returns path with correct strength product
- [ ] B+Tree: property-based test (100K random inserts → sorted scan correct) passes 100%
- [ ] Temporal: 100K intervals, point-in-time query returns correct overlapping set

---

## Sprint 3 — Query Engine Core
**Goal:** Execute SELECT queries end-to-end: scalar filters, vector search, graph traversal, temporal, confidence filters.
**Duration:** 2 weeks
**Agents in parallel:** 4
**Depends on:** Sprint 2 complete (indexes working)

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | Complete FunQL parser + AST + Binder (type system) | Wave 4-A | REQ-SQL-001, REQ-SQL-002 |
| Agent-B | Rule-based optimizer (predicate pushdown, vector pre-filter, confidence pruning) | Wave 4-B | REQ-OPT-001, REQ-OPT-002 |
| Agent-C | Physical operators + Volcano executor + plan-to-physical mapping | Wave 4-C | REQ-EXEC-001, REQ-EXEC-002 |
| Agent-D | LSM controller + compaction engine | Wave 4-D | REQ-STORE-001 (completion) |

### DoD Sprint 3
- [ ] All ARCHITECTURE.md §5.1 example queries parse and execute on single node
- [ ] `EXPLAIN` output for `WHERE category='science' AND _vector <-> q < 0.5` shows `B+Tree → VectorANN` order (pre-filter applied)
- [ ] `EXPLAIN` for `WHERE _confidence > 0.7` shows `ConfidenceFilter` before any scan
- [ ] 1M record write survives 3 compaction rounds with all records accessible
- [ ] Integration test: insert 1000 FunRecords, SELECT with each filter type returns correct results

### Milestone: Single-Node Alpha
**After Sprint 3:** FunDB can store and query data on a single node. Developers can run it locally. PostgreSQL wire protocol works for standard queries.

---

## Sprint 4 — Cognitive Modules (Part 1)
**Goal:** Confidence propagates through queries; WITHIN CONTEXT returns token-budget-aware results; agent memory works.
**Duration:** 2 weeks
**Agents in parallel:** 4
**Depends on:** Sprint 3 complete

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | Confidence propagation engine | Wave 5-A | REQ-CONF-001, REQ-CONF-002, REQ-CONF-003 |
| Agent-B | Context-aware retrieval (WITHIN CONTEXT + MMR) | Wave 5-B | REQ-CTX-001, REQ-CTX-002 |
| Agent-C | Contradiction detection (auto-link + confidence update) | Wave 5-C | REQ-CONTRA-001 |
| Agent-D | Agent Memory subsystem (REMEMBER, RECALL BY, decay, consolidation) | Wave 5-E | — |

### DoD Sprint 4
- [ ] JOIN propagation: `A._confidence=0.9 JOIN B._confidence=0.8` → result `_confidence=0.8`
- [ ] Causal chain: A(0.9)→B(0.85)→C(0.7) → propagated = 0.535 ± 0.001
- [ ] Equal contradictions: N=2 → 0.75, N=3 → 0.667 (REQ-CONF-003 formula)
- [ ] WITHIN CONTEXT: token budget never exceeded (max_tokens + 10% margin)
- [ ] WITHIN CONTEXT: diversity=0.0 returns most relevant; diversity=1.0 maximizes diversity
- [ ] Insert "sky is blue" + "sky is not blue" → auto-linked as contradictions
- [ ] Agent memory: store 1K memories, RECALL BY weighted score returns correct top-20

---

## Sprint 5 — Causal Engine Tier 1 + Python SDK
**Goal:** Explicit causality fully queryable; Python SDK ships; psql fully compatible.
**Duration:** 2 weeks
**Agents in parallel:** 4
**Depends on:** Sprint 4 complete

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | Causal Engine Tier 1 (TRACE CAUSALITY, feedback loop detection, visualization) | Wave 5-D | REQ-CAUSAL-001, REQ-CAUSAL-002 |
| Agent-B | PostgreSQL wire protocol (extended query, COPY, auth) | Wave 8-A | REQ-PROTO-001 |
| Agent-C | Python SDK (full interface + memory + feedback API) | Wave 8-C | REQ-PROTO-002 |
| Agent-D | gRPC + REST API | Wave 8-B | — |

### DoD Sprint 5
- [ ] `TRACE CAUSALITY FROM A TO B MAX_DEPTH 5 MIN_STRENGTH 0.3` returns correct paths
- [ ] `MODE EQUILIBRIUM` causal model converges for 3-variable linear system
- [ ] Cycle insert in standard mode: `CycleError` raised
- [ ] `psycopg2` and `asyncpg` run full test suite: all standard queries pass
- [ ] Python SDK: `conn.understand("papers about X")` → executes correct FunQL
- [ ] Python SDK: `conn.trace_causality(a, b)` → returns CausalPath object
- [ ] Python SDK: `memory.remember()` + `memory.recall()` integration test passes

### Milestone: Cognitive Alpha
**After Sprint 5:** FunDB can be used by AI agents with confidence, provenance, explicit causality, agent memory, and WITHIN CONTEXT. Python SDK is developer-usable.

---

## Sprint 6 — Causal Engine Tier 2 + Semantic Interface
**Goal:** Granger causality discovers structure in time-series; UNDERSTAND parses natural language queries.
**Duration:** 2 weeks
**Agents in parallel:** 4
**Depends on:** Sprint 5 complete

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | Semantic Interface Tier 1-2 (rule engine + ML classifier) | Wave 6-A | REQ-SEM-001, REQ-SEM-002, REQ-SEM-003 |
| Agent-B | Causal Engine Tier 2 (Granger + ADF + rolling window + LLM oracle) | Wave 6-B | REQ-CAUSAL-003, REQ-CAUSAL-004 |
| Agent-C | Learning-to-Rank (LambdaMART + feedback loop) | Wave 6-C | REQ-LTR-001, REQ-LTR-002 |
| Agent-D | Cost-based optimizer + adaptive index advisor | Wave 6-D | REQ-OPT-003 |

### DoD Sprint 6
- [ ] Tier 1 parser: 70% of test query set matched by rules in < 1ms
- [ ] Tier 2 classifier: F1 ≥ 0.85 on 500-query labeled set
- [ ] Low-confidence `UNDERSTAND`: returns 3 candidate FunQL queries instead of executing
- [ ] Granger: synthetic A→B, A→C, B→D time-series → all 3 edges discovered at p < 0.05
- [ ] Granger: non-stationary series → ADF pre-check triggers differencing automatically
- [ ] LLM oracle: hallucinated edge that fails statistical validation is NOT persisted
- [ ] LTR: after 100 feedback signals, NDCG@10 higher than baseline (no LTR)
- [ ] Cost-based optimizer: selectivity < 1% → IndexScan chosen (verified via EXPLAIN)

---

## Sprint 7 — Distribution
**Goal:** 3-node cluster with Raft consensus; distributed ANN works correctly.
**Duration:** 3 weeks (longer — distributed systems are hard)
**Agents in parallel:** 3
**Depends on:** Sprint 3 complete (query engine single-node)

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | Raft consensus (leader election, log replication, snapshots) | Wave 7-A | REQ-DIST-001 |
| Agent-B | Shard manager (consistent hashing, routing, rebalancing) | Wave 7-B | REQ-DIST-002 |
| Agent-C | Distributed query execution (scatter/gather, distributed ANN merge) | Wave 7-C | REQ-DIST-003, REQ-DIST-004 |

### DoD Sprint 7
- [ ] 5-node cluster: kill leader → new leader elected within 3 timeouts
- [ ] Log consistent after re-join of failed node (no data loss)
- [ ] Adding node: moves ≤ K/N records (consistent hash property)
- [ ] 3-shard distributed ANN top-10: recall ≥ 0.95 vs centralized brute-force
- [ ] Cross-shard confidence propagation uses `min()` merge

### Milestone: Distributed Alpha
**After Sprint 7:** FunDB runs as a 3-node cluster. Can be deployed to Kubernetes.

---

## Sprint 8 — Causal Tier 3 + Production Hardening
**Goal:** SCM interventional queries work; system is production-ready.
**Duration:** 3 weeks
**Agents in parallel:** 5
**Depends on:** Sprints 6 + 7 complete

| Agent | Task | Plan ref | Requirements |
|-------|------|----------|--------------|
| Agent-A | Causal Tier 3: SCM + do-calculus + counterfactuals | Wave 9-A | REQ-CAUSAL-005, REQ-CAUSAL-006, REQ-CAUSAL-007 |
| Agent-B | Semantic Interface Tier 3 (embedded ONNX LLM) | Wave 9-B | — |
| Agent-C | Security (TLS, RBAC, tenant isolation, audit log) | Wave 10-A | REQ-SEC-001, REQ-SEC-002, REQ-SEC-003 |
| Agent-D | Observability (Prometheus metrics, OpenTelemetry tracing, Grafana dashboards) | Wave 10-B | — |
| Agent-E | Benchmarks + correctness tests + performance validation | Wave 10-C, 10-D | REQ-PERF-001…011 |

### DoD Sprint 8 = Production Ready
- [ ] `ESTIMATE EFFECT OF SET(X=2) ON Y` returns estimate within 5% of ground truth (synthetic SCM)
- [ ] `ESTIMATE COUNTERFACTUAL` result matches analytical solution
- [ ] Ensemble causal discovery: 5-variable DAG, ≥80% edges recovered
- [ ] NOTEARS non-convergence: ensemble continues, warning logged
- [ ] Tenant A query never returns tenant B data (security test)
- [ ] All REQ-PERF-001…011 met (benchmark suite passes)
- [ ] Chaos test: random node kill during write workload → no data loss
- [ ] Prometheus metrics endpoint live; Grafana dashboard importable

### Milestone: Public Beta
**After Sprint 8:** FunDB is ready for early adopters.

---

## Go SDK (parallel track, any sprint ≥ 5)

| Agent | Task | Sprint |
|-------|------|--------|
| Agent-Go | Go SDK full interface | Sprint 5-6 parallel |

---

## Sprint Summary

| Sprint | Goal | Duration | Max agents | Key milestone |
|--------|------|----------|------------|---------------|
| 0 | Workspace + FunRecord | 2w | 4 | Foundation |
| 1 | Storage engine | 2w | 4 | Persistent storage |
| 2 | Index engines | 2w | 5 | Queryable indexes |
| 3 | Query engine | 2w | 4 | **Single-Node Alpha** |
| 4 | Cognitive modules | 2w | 4 | Confidence + Context |
| 5 | Causal Tier 1 + Python SDK | 2w | 4 | **Cognitive Alpha** |
| 6 | Causal Tier 2 + Semantic | 2w | 4 | Causal inference |
| 7 | Distribution | 3w | 3 | **Distributed Alpha** |
| 8 | Causal Tier 3 + Production | 3w | 5 | **Public Beta** |

**Total:** ~22 weeks with max parallelism (5 agents per sprint)
**Critical path (single agent):** ~44 weeks

---

## Starting Right Now — Sprint 0 Agent Assignments

To begin immediately, spawn these 4 agents in parallel:

```
Agent-0A: Initialize Cargo workspace + implement FunRecord
  Reads: ARCHITECTURE.md §4.3, IMPLEMENTATION_PLAN.md Wave 1-A, REQUIREMENTS.md CORE
  Writes: crates/fundb-core/src/record.rs, crates/fundb-core/src/lib.rs

Agent-0B: Implement codecs (MessagePack + columnar pages)
  Reads: ARCHITECTURE.md §4.1, IMPLEMENTATION_PLAN.md Wave 1-B, REQUIREMENTS.md STORE
  Writes: crates/fundb-core/src/codec.rs, crates/fundb-core/src/page.rs
  Depends on: Agent-0A interface (FunRecord struct)

Agent-0C: FunQL grammar + lexer
  Reads: ARCHITECTURE.md §5.1, IMPLEMENTATION_PLAN.md Wave 1-C, REQUIREMENTS.md SQL
  Writes: crates/fundb-sql/src/lexer.rs, crates/fundb-sql/src/grammar.lalrpop
  No dependency on 0A/0B

Agent-0D: PostgreSQL wire protocol skeleton
  Reads: IMPLEMENTATION_PLAN.md Wave 1-D, PostgreSQL wire protocol docs
  Writes: crates/fundb-protocol/src/pg_wire.rs, crates/fundb-server/src/main.rs
  No dependency on 0A/0B/0C
```

**Sprint 0 ends when:** all 4 DoD items checked above.

---

## Agent Instructions Template

When spawning any agent, include:

```
You are implementing [TASK NAME] for FunDB, an AI-native cognitive database written in Rust.

Read these documents first (in order):
1. /docs/REQUIREMENTS.md — requirements for your module
2. /docs/IMPLEMENTATION_PLAN.md — your specific Wave/Agent section
3. /ARCHITECTURE.md — overall system design context

Your crate: [CRATE NAME]
Your wave: Wave [N]-[X]
Requirements to satisfy: [REQ-XXX-YYY list]

Rules:
- Write tests BEFORE implementation (TDD)
- Define your public interface (traits/structs) BEFORE writing implementations
- If you discover an issue that blocks you, document it as a new OQ in IMPLEMENTATION_PLAN.md
- Do NOT change interfaces defined by other agents without coordination
- Performance targets: see REQ-PERF-001…011 in REQUIREMENTS.md
```
