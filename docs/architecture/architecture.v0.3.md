# FunDB — The AI-Native Cognitive Database

**Version:** 0.3.0-draft
**Classification:** Technical Architecture Document
**Audience:** Engineers, Architects, Investors

---

## Changelog

| Version | Date | Summary |
|---------|------|---------|
| 0.1.0 | 2026-02-28 | Initial architecture — multi-model engine, FunQL, HNSW, LSM storage |
| 0.2.0 | 2026-02-28 | Cognitive pillars added: Semantic Interface, Confidence & Provenance, Context-Aware Retrieval, Bidirectional Learning, Causal Reasoning (Tier 1-3 spec) |
| **0.3.0** | **2026-02-28** | **Causal Reasoning Engine fully specified from hackathon results: 5-signal framework, Dempster-Shafer fusion, NaturalExperimentDetector cascade, MechanismDetect, ConfounderSearch, Red Team checklist, computed confidence ceiling, physical operators, 303ms execution budget** |

---

## 1. Executive Summary

FunDB is a next-generation, AI-native cognitive database engine designed from first principles to serve the data infrastructure needs of Large Language Models, autonomous agents, RAG pipelines, multi-modal systems, and continuous learning architectures.

Unlike incumbent databases (MongoDB, PostgreSQL, Oracle) that retrofit AI capabilities via extensions or plugins, FunDB treats **vectors, graphs, documents, time-series, and agent memory as first-class citizens** in a single unified storage and query engine.

What makes FunDB fundamentally different from other "AI databases" is its **cognitive architecture**: the database doesn't just *store data for AI* — it **thinks like AI**. It understands semantic intent, tracks confidence and provenance, optimizes for context windows, learns from agent behavior, and reasons about causality.

**Core thesis:** The AI era demands a database that is a **cognitive partner**, not just intelligent storage — multi-modal, contextual, temporal, continuously adaptive, and capable of reasoning about knowledge rather than just retrieving data.

### 1.1 The Five Cognitive Pillars

Traditional databases operate on a **data paradigm**: store bytes, retrieve bytes, guarantee consistency. FunDB operates on a **knowledge paradigm** built on five pillars that no existing database combines:

| Pillar | What it means | Why it matters |
|--------|--------------|----------------|
| **Semantic Intent** | Queries express *what you mean*, not *how to get it* | AI agents shouldn't need to know schemas to ask questions |
| **Confidence & Provenance** | Every fact has a trust score, a source, and known contradictions | AI decisions are only as good as the confidence of their inputs |
| **Context Awareness** | The database optimizes output for the consumer's context window | LLMs have finite context — every wasted token is lost reasoning capacity |
| **Adaptive Learning** | The database learns from how agents use (or ignore) results | Relevance isn't static — it evolves with each interaction |
| **Causal Reasoning** | Data tracks not just *what* and *when*, but *why* | Understanding causality enables prediction, not just retrieval |

---

## 2. Design Principles

| # | Principle | Rationale |
|---|-----------|-----------|
| 1 | **AI-Native, not AI-Adapted** | Built for embeddings, graphs, and temporal data from day zero — no extensions or bolted-on modules |
| 2 | **Unified Multi-Model** | One engine, one query language, one transaction boundary across vectors, documents, graphs, and time-series |
| 3 | **Horizontal-First** | Every component is designed for distributed operation; single-node is a degenerate case of the cluster |
| 4 | **Adaptive Intelligence** | The database itself uses ML to optimize indexing, caching, and query planning based on workload patterns |
| 5 | **Temporal by Default** | All data is versioned with bitemporal semantics — enabling model reproducibility and continuous learning |
| 6 | **Developer Ergonomics** | Hybrid query language that feels natural for SQL users while exposing vector and graph primitives natively |
| 7 | **Knowledge over Data** | The fundamental unit is not a row or document, but a *piece of knowledge* — with confidence, provenance, and causal relationships |
| 8 | **Intent over Instruction** | Agents express *what they need*, not *how to query it* — the database infers the optimal retrieval strategy |
| 9 | **Context-Aware Output** | The database understands consumer constraints (token budgets, coherence needs) and optimizes output accordingly |
| 10 | **Bidirectional Learning** | The database learns from agents, and agents learn from the database — creating a feedback loop that improves over time |

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                     SEMANTIC INTERFACE LAYER                         │
│                                                                     │
│  ┌───────────────┐  ┌──────────────┐  ┌──────────────────────────┐ │
│  │  Intent       │  │  Context     │  │  Feedback                │ │
│  │  Parser       │  │  Optimizer   │  │  Collector               │ │
│  │  (NLU/LLM)   │  │  (MMR+Token) │  │  (Learning-to-Rank)     │ │
│  └──────┬────────┘  └──────┬───────┘  └──────────┬───────────────┘ │
│         │                  │                      │                 │
│         └──────────────────┼──────────────────────┘                 │
│                            │  generates deterministic FunQL         │
└────────────────────────────┼───────────────────────────────────────┘
                             │
┌────────────────────────────▼───────────────────────────────────────┐
│                        CLIENT LAYER                                 │
│  SDK (Python/Rust/Go/JS)  │  REST/gRPC API  │  Wire Protocol       │
│                           │                  │  (Postgres-compat)   │
└──────────────┬────────────┴────────┬─────────┴──────────────────────┘
               │                     │
┌──────────────▼─────────────────────▼────────────────────────────────┐
│                      QUERY ENGINE (FunQL)                            │
│  ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌───────────────────┐  │
│  │  Parser  │→ │ Optimizer │→ │ Planner  │→ │ Executor (Volcano) │  │
│  └──────────┘  └───────────┘  └──────────┘  └───────────────────┘  │
│       ▲              ▲                              │               │
│       │         Adaptive                            │               │
│       │         Statistics                          │               │
│       │         Collector                           ▼               │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │           UNIFIED CATALOG & METADATA                           │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐ │ │
│  │  │  Schema      │  │  Confidence  │  │  Causal              │ │ │
│  │  │  Registry    │  │  Propagation │  │  Graph               │ │ │
│  │  │              │  │  Engine      │  │  Metadata            │ │ │
│  │  └──────────────┘  └──────────────┘  └──────────────────────┘ │ │
│  └────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────┐
│                      INDEX MANAGER                                   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  │  B+Tree  │ │  HNSW    │ │ Temporal │ │  Graph   │ │  GIN/    │ │
│  │ (scalar) │ │ (vector) │ │  B+Tree  │ │  (SPO)   │ │  Inverted│ │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘ │
│                    ▲  Adaptive Index Advisor (ML)                    │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────┐
│                    STORAGE ENGINE (FunStore)                          │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │              LSM-Tree + Column Segments (hybrid)               │ │
│  │                                                                │ │
│  │  ┌──────────┐  ┌──────────────┐  ┌──────────────────────────┐│ │
│  │  │ MemTable │→ │  WAL (Raft)  │→ │  SSTable / Column Pages  ││ │
│  │  │ (SkipList)│  │              │  │  (Snappy/Zstd compressed)││ │
│  │  └──────────┘  └──────────────┘  └──────────────────────────┘│ │
│  │                                                                │ │
│  │  ┌──────────────────────────────────────────────────────────┐ │ │
│  │  │  MVCC Layer — Bitemporal Versioning (system + valid time)│ │ │
│  │  └──────────────────────────────────────────────────────────┘ │ │
│  └────────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────┐
│                   DISTRIBUTED FABRIC                                 │
│  ┌──────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │  Raft    │  │  Consistent  │  │   Gossip     │  │   Shard    │ │
│  │ Consensus│  │  Hashing     │  │   Protocol   │  │   Manager  │ │
│  └──────────┘  └──────────────┘  └──────────────┘  └────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. Storage Engine — FunStore

### 4.1 Hybrid LSM + Columnar Design

FunDB's storage engine, **FunStore**, employs a hybrid approach:

**Write Path (LSM-Tree):**
```
Client Write
    │
    ▼
┌──────────┐    ┌─────────┐
│ MemTable │───→│   WAL   │  (write-ahead log, persisted via Raft)
│(SkipList)│    │ (append) │
└────┬─────┘    └─────────┘
     │ flush threshold (64MB default)
     ▼
┌──────────────┐
│  L0 SSTable  │  (sorted, immutable)
└──────┬───────┘
       │ compaction
       ▼
┌──────────────┐
│  L1..Ln      │  (tiered + leveled hybrid compaction)
│  SSTables    │
└──────────────┘
```

**Why LSM for the write path:**
- AI workloads are write-heavy (embeddings ingestion, agent memory updates, streaming time-series)
- Sequential I/O on SSDs; write amplification controlled via tiered compaction at lower levels and leveled at higher levels
- MemTable uses a concurrent skip-list (lock-free) for high-throughput parallel writes

**Read Path (Columnar Segments):**

Once SSTables reach L2+, they are reorganized into **columnar page groups**:

```
┌─────────────────────────────────────────────┐
│              Column Page Group               │
│                                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐│
│  │  Scalar  │ │  Vector  │ │  Document    ││
│  │  Columns │ │  Columns │ │  Columns     ││
│  │ (int,str)│ │(f32[N])  │ │  (BSON-like) ││
│  └──────────┘ └──────────┘ └──────────────┘│
│                                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐│
│  │Confidence│ │Provenance│ │  Causal      ││
│  │  Column  │ │  Column  │ │  Edge Column ││
│  │(float32) │ │(source[])│ │  (ref[])     ││
│  └──────────┘ └──────────┘ └──────────────┘│
│                                              │
│  Page Header: min/max, bloom filter,         │
│  quantized vector summary, row count,        │
│  confidence distribution histogram           │
└─────────────────────────────────────────────┘
```

**Rationale:** Analytical queries and vector scans benefit from columnar layout (SIMD-friendly, cache-line aligned). Embedding columns use **Product Quantization (PQ)** compression — a 768-dim float32 vector (3KB) compresses to ~96 bytes with PQ64, enabling 30x storage reduction for cold data. Confidence and provenance columns are stored separately for efficient filtering by trust level.

### 4.2 Bitemporal MVCC

Every record carries two temporal dimensions:

```
┌─────────────────────────────────────────┐
│  Record Envelope                         │
│                                          │
│  record_id:    UUID v7 (time-ordered)    │
│  system_time:  [created_at, expired_at)  │
│  valid_time:   [valid_from, valid_to)    │
│  txn_id:       uint64                    │
│  tenant_id:    uint32                    │
│  payload:      encoded columns           │
│  tombstone:    bool                      │
└─────────────────────────────────────────┘
```

- **system_time:** When the database physically stored the version (immutable once written)
- **valid_time:** Application-defined validity window (e.g., "this embedding was valid from Jan 1 to Feb 15")

**Why bitemporal for AI:**
- **Model reproducibility:** "Give me the exact training dataset as of 2025-11-01" → query `system_time <= 2025-11-01`
- **Concept drift detection:** Compare embeddings at different `valid_time` ranges
- **Agent memory:** "What did the agent believe to be true at time T?"
- **Continuous learning:** Retrain on specific temporal slices without data corruption

### 4.3 Data Model — The FunRecord

FunDB stores all data types in a single physical format — the **FunRecord**. This is not just a row or document — it is a **unit of knowledge** with built-in epistemology:

```
FunRecord {
    // === Core Identity ===
    _id:          UUID v7
    _collection:  string              // logical namespace
    _tenant:      uint32              // isolation boundary

    // === Temporal Envelope ===
    _sys_from:    timestamp
    _sys_to:      timestamp           // MAX for current version
    _valid_from:  timestamp
    _valid_to:    timestamp

    // === Knowledge Payload ===
    data:         MessagePack-encoded document  // flexible schema-on-read

    // === Typed Extensions (separate column segments) ===
    _vectors:     map<string, float32[]>        // named embedding fields
    _edges:       []Edge{label, target, props}  // graph adjacency
    _timeseries:  []Sample{ts, value}           // time-series points

    // === Cognitive Metadata (NEW) ===
    _confidence:  float32             // 0.0–1.0, trust score for this fact
    _sources:     []Source {          // provenance chain
        origin:     string            // "model:gpt-4", "user:manual", "sensor:temp-01"
        timestamp:  timestamp
        method:     string            // "inference", "observation", "aggregation"
        confidence: float32           // source-level confidence
    }
    _supports:    []Ref{_id, strength}    // records that corroborate this fact
    _contradicts: []Ref{_id, strength}    // records that contradict this fact
    _caused_by:   []CausalEdge {          // causal provenance
        source_id:  UUID
        relation:   CausalType            // CAUSED | INFLUENCED | CORRELATED | PRECEDED
        strength:   float32               // 0.0–1.0
        mechanism:  string                // optional: "deployed new model" → "errors increased"
    }
    _effects:     []CausalEdge            // inverse: what did this record cause?
}
```

This design means:
- A single record can simultaneously be a **document**, have **vector embeddings**, participate in a **graph**, contain **time-series** data, carry **confidence metadata**, track its **provenance**, and express **causal relationships**
- No need to sync data across separate stores — one record, one transaction, one consistency boundary
- AI agents can query not just *what* is true, but *how confident* the database is, *where* the information came from, and *why* it changed

---

## 5. Query Engine — FunQL

### 5.1 Language Design

FunQL is a superset of SQL with native primitives for vectors, graphs, temporal queries, confidence filtering, and causal reasoning:

```sql
-- Classic relational
SELECT * FROM users WHERE age > 25;

-- Vector similarity search (ANN)
SELECT * FROM documents
WHERE _vector('content_embedding') <-> [0.1, 0.2, ...] < 0.3
ORDER BY _vector_distance ASC
LIMIT 10;

-- Graph traversal
SELECT * FROM entities
TRAVERSE follows(depth: 1..3)
WHERE start.type = 'user' AND end.type = 'topic'
RETURN path;

-- Confidence-aware query (NEW)
SELECT title, abstract, _confidence, _sources
FROM research_papers
WHERE _vector('embedding') <-> :query < 0.5
  AND _confidence > 0.7
  AND _source_count >= 2
  AND _contradiction_count = 0
ORDER BY _confidence * (1 - _vector_distance) DESC
LIMIT 10;

-- Causal trace query (NEW)
SELECT * FROM events
TRACE CAUSALITY FROM :event_a TO :event_b
  MAX_DEPTH 5
  MIN_STRENGTH 0.3
RETURN causal_path, total_strength;

-- Context-optimized retrieval (NEW)
SELECT * FROM knowledge_base
WHERE _vector('embedding') <-> :query < 0.6
WITHIN CONTEXT (
  max_tokens: 4000,
  coherence: 0.6,
  diversity: 0.3,
  include_contradictions: true
);

-- Hybrid: vector + filter + graph + confidence + temporal
SELECT d.title, d.summary, d._confidence
FROM documents d
WHERE d._vector('embedding') <-> :query_vector < 0.5
  AND d.category = 'research'
  AND d._confidence > 0.6
  AND d TRAVERSE cites(depth: 1..2) -> papers
  AS OF SYSTEM TIME '2025-06-01'
ORDER BY _vector_distance ASC
LIMIT 20;

-- Time-travel query
SELECT * FROM agent_memory
AS OF VALID TIME BETWEEN '2025-01-01' AND '2025-06-01'
WHERE agent_id = 'agent-42';

-- Time-series aggregation
SELECT time_bucket('1 hour', ts) AS bucket,
       avg(value) AS avg_metric
FROM metrics
WHERE ts BETWEEN now() - interval '7 days' AND now()
GROUP BY bucket;
```

### 5.2 Query Planner Architecture

```
                    FunQL Query String
                          │
                          ▼
                 ┌────────────────┐
                 │   PARSER       │  (LALR(1) grammar, Rust)
                 │   → AST        │
                 └───────┬────────┘
                         │
                         ▼
                 ┌────────────────┐
                 │   BINDER       │  (resolve names, types, schemas)
                 │   → Logical    │
                 │     Plan       │
                 └───────┬────────┘
                         │
                         ▼
              ┌──────────────────────┐
              │   OPTIMIZER          │
              │                      │
              │  Rule-Based Phase:   │
              │  • Predicate pushdown│
              │  • Projection prune  │
              │  • Join reorder      │
              │  • Vector pre-filter │
              │    (scalar before    │
              │     ANN)             │
              │  • Graph path prune  │
              │  • Confidence prune  │
              │    (early filter by  │
              │     trust threshold) │
              │                      │
              │  Cost-Based Phase:   │
              │  • Cascades/Columbia │
              │    framework         │
              │  • Histogram stats   │
              │  • Vector index sel. │
              │    estimation        │
              │  • Distributed cost  │
              │    model (network,   │
              │    shard fan-out)    │
              │  • Confidence prop.  │
              │    cost estimation   │
              │                      │
              │  Adaptive Phase:     │
              │  • Runtime stats     │
              │    feedback loop     │
              │  • ML cardinality    │
              │    estimator         │
              │  • Index advisor     │
              │    recommendations   │
              │  • LTR ranking       │
              │    adjustment        │
              └──────────┬───────────┘
                         │
                         ▼
              ┌──────────────────────┐
              │   PLANNER            │
              │                      │
              │  Physical Plan:      │
              │  • SeqScan           │
              │  • IndexScan(B+Tree) │
              │  • VectorANN(HNSW)   │
              │  • GraphTraverse(BFS)│
              │  • TemporalRangeScan │
              │  • CausalPathScan    │
              │  • ConfidenceFilter  │
              │  • ContextOptimize   │
              │  • HashJoin          │
              │  • MergeJoin         │
              │  • ShardScatter      │
              │  • ShardGather       │
              │  • TopK              │
              └──────────┬───────────┘
                         │
                         ▼
              ┌──────────────────────┐
              │   EXECUTOR           │
              │   (Volcano / Pull)   │
              │                      │
              │  Vectorized batches  │
              │  (1024 rows/batch)   │
              │  SIMD acceleration   │
              │  for vector distance │
              └──────────────────────┘
```

**Key optimization: Vector Pre-Filtering**

The most impactful optimization for RAG workloads is **scalar-before-vector** predicate pushdown:

```
Input:  WHERE category = 'science' AND _vector <-> q < 0.5 LIMIT 10

Naive:  ANN scan all vectors → filter by category → SLOW (scans millions)
FunDB:  B+Tree(category='science') → candidate set → ANN on subset → FAST
```

The optimizer recognizes that scalar predicates can dramatically reduce the candidate set before expensive ANN computation. For a 10M document collection where `category='science'` selects 100K docs, this reduces ANN computation by 99%.

**Key optimization: Confidence Early Pruning (NEW)**

```
Input:  WHERE _confidence > 0.7 AND _vector <-> q < 0.5 LIMIT 10

FunDB:  ConfidenceFilter(>0.7) → candidate set → ANN on subset → FAST
```

Since confidence is stored as a separate column with a histogram in the page header, the planner can prune entire page groups where `max(confidence) < threshold` without reading individual records.

### 5.3 Distributed Query Execution

```
           Coordinator Node
          ┌───────────────────┐
          │  Scatter/Gather   │
          │  Plan Fragmenter  │
          └─┬──────┬────────┬─┘
            │      │        │
     ┌──────▼─┐ ┌──▼─────┐ ┌▼────────┐
     │Shard 1 │ │Shard 2 │ │Shard 3  │
     │Local   │ │Local   │ │Local    │
     │Execute │ │Execute │ │Execute  │
     └────┬───┘ └───┬────┘ └────┬────┘
          │         │           │
          └─────────┼───────────┘
                    │
              ┌─────▼─────┐
              │   Merge   │  (Top-K merge for ANN,
              │   Node    │   union for graph traversal,
              │           │   confidence-weighted merge)
              └───────────┘
```

For distributed ANN queries, each shard returns its local top-K candidates. The coordinator performs a **merge-rerank** step, which guarantees recall equivalent to centralized search when K_local >= K_global * rerank_factor (default: 3x).

---

## 6. Semantic Interface — Intent-Based Queries

### 6.1 Overview

The Semantic Interface is the layer that separates FunDB from every other database: **agents express intent, FunDB figures out the query**.

```
┌────────────────────────────────────────────────────────────────┐
│                    SEMANTIC INTERFACE                            │
│                                                                 │
│  Input:  "papers about attention mechanisms cited by Vaswani"   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  INTENT PARSER                                            │  │
│  │                                                           │  │
│  │  1. Entity Recognition:                                   │  │
│  │     "attention mechanisms" → concept (needs vector search) │  │
│  │     "Vaswani" → entity (needs graph/scalar lookup)        │  │
│  │     "cited by" → relationship (needs graph traversal)     │  │
│  │     "papers" → collection hint                            │  │
│  │                                                           │  │
│  │  2. Intent Classification:                                │  │
│  │     RETRIEVAL + GRAPH_TRAVERSAL + SEMANTIC_SIMILARITY     │  │
│  │                                                           │  │
│  │  3. Strategy Selection:                                   │  │
│  │     → Vector search on "attention mechanisms" embedding   │  │
│  │     → Graph traversal on citation edges from Vaswani      │  │
│  │     → Intersection of both result sets                    │  │
│  └──────────────────────────┬───────────────────────────────┘  │
│                              │                                  │
│  ┌──────────────────────────▼───────────────────────────────┐  │
│  │  GENERATED FunQL (deterministic, inspectable):            │  │
│  │                                                           │  │
│  │  SELECT p.title, p.abstract, p._confidence               │  │
│  │  FROM papers p                                            │  │
│  │  WHERE p._vector('embedding') <-> embed(:query) < 0.5    │  │
│  │    AND p TRAVERSE cited_by(depth: 1..2)                   │  │
│  │        -> authors WHERE name LIKE '%Vaswani%'             │  │
│  │  ORDER BY _vector_distance ASC                            │  │
│  │  LIMIT 20;                                                │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

### 6.2 How It Works

The Intent Parser is **not a general-purpose LLM call**. It is a focused, deterministic pipeline:

```
Intent Query String
       │
       ▼
┌──────────────┐
│  Tokenizer   │  (domain-aware, knows FunDB collection names & schemas)
│  + NER       │  (recognizes entities, concepts, relationships)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  Intent      │  Three-tier classification:
│  Classifier  │
│              │  Tier 1 — Rule-based patterns (fastest, <1ms):
│              │    "find X similar to Y" → VECTOR_SEARCH
│              │    "X connected to Y" → GRAPH_TRAVERSAL
│              │    "X before/after Y" → TEMPORAL_QUERY
│              │
│              │  Tier 2 — Lightweight ML model (~50MB, <5ms):
│              │    Handles compound intents and ambiguity
│              │    Trained on query intent dataset
│              │
│              │  Tier 3 — Embedded LLM fallback (~500MB, <100ms):
│              │    For complex natural language only
│              │    ONNX-optimized small model
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  FunQL       │  Deterministic template-based generation
│  Generator   │
│              │  • Maps intent graph to FunQL AST
│              │  • Validates against catalog (collections, fields exist?)
│              │  • Returns FunQL string + confidence score
│              │  • If confidence < threshold: returns candidate queries
│              │    for agent to choose from
└──────┬───────┘
       │
       ▼
  FunQL String → Standard Query Engine
```

### 6.3 Semantic Interface Query Syntax

```sql
-- Natural language intent (processed by Semantic Interface)
UNDERSTAND "papers about attention mechanisms cited by Vaswani's network"
WITH confidence > 0.7
WITHIN last 2 years
DEPTH 2;

-- Intent with explicit constraints
UNDERSTAND "users who behave similarly to user-42"
IN COLLECTION users
USING VECTOR 'behavior_embedding'
WITH min_similarity 0.7;

-- The UNDERSTAND keyword triggers the Semantic Interface
-- Everything else goes directly to the FunQL engine
-- Agents can mix both:
SELECT custom_field FROM (
    UNDERSTAND "relevant research papers"
    WITHIN last 6 months
) AS results
WHERE results.citation_count > 10;
```

### 6.4 Why This Works (And Isn't Just "Text-to-SQL")

| Feature | Generic Text-to-SQL | FunDB Semantic Interface |
|---------|---------------------|--------------------------|
| Schema knowledge | Must be provided in prompt | Built-in — reads catalog directly |
| Vector awareness | Cannot generate ANN queries | Native — knows when to use vector search |
| Graph awareness | Cannot generate traversals | Native — recognizes relationship patterns |
| Temporal awareness | Limited | Native — understands time expressions |
| Determinism | LLM output varies | Template-based generation is deterministic |
| Latency | 500ms-2s (LLM call) | <5ms for Tier 1-2, <100ms for Tier 3 |
| Hallucination | Can generate invalid SQL | Validates against catalog before returning |

---

## 7. Confidence & Provenance System

### 7.1 Confidence as a First-Class Primitive

Every FunRecord carries a `_confidence` score (0.0–1.0) that represents the database's trust level in that piece of knowledge. This is not optional metadata — it is a **core primitive** that affects query planning, ranking, and result selection.

**Confidence sources:**

```
┌────────────────────────────────────────────────────────┐
│              CONFIDENCE COMPUTATION                      │
│                                                         │
│  Base Confidence:                                       │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Explicitly set by writer:                       │   │
│  │    INSERT INTO facts (data, _confidence)         │   │
│  │    VALUES ('Earth orbits Sun', 0.99);            │   │
│  │                                                   │   │
│  │  Inferred from source:                           │   │
│  │    Model inference  → base 0.7 (configurable)    │   │
│  │    Human input      → base 0.9                   │   │
│  │    Sensor reading   → base varies by sensor spec │   │
│  │    Aggregation      → computed from inputs       │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  Modifiers (applied automatically):                     │
│  ┌─────────────────────────────────────────────────┐   │
│  │  +0.1 per independent corroborating source       │   │
│  │  -0.2 per contradiction (weighted by contra.     │   │
│  │        source confidence)                         │   │
│  │  -decay(time) for time-sensitive facts            │   │
│  │  Capped at [0.0, 1.0]                            │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  Final: _confidence = clamp(base + Σmodifiers, 0, 1)   │
└────────────────────────────────────────────────────────┘
```

### 7.2 Confidence Propagation in Queries

When queries combine multiple records (JOINs, aggregations, graph traversals), confidence propagates through algebraic rules:

```
Operation              │ Propagation Rule
───────────────────────┼──────────────────────────────────────
JOIN(A, B)             │ min(A._confidence, B._confidence)
UNION(A, B)            │ max(A._confidence, B._confidence)
AGGREGATE(group)       │ weighted_avg(group._confidence, weight=row_relevance)
GRAPH_TRAVERSE(A→B→C)  │ A._confidence × edge_AB.strength × B._confidence × ...
CAUSAL_CHAIN(A→...→Z)  │ product of all strengths in chain (confidence decays)
NEGATION(NOT A)        │ 1 - A._confidence
```

**Example:**

```sql
-- "How confident is the database that Vaswani influenced transformer adoption?"
SELECT total_confidence
FROM (
    TRACE CAUSALITY FROM 'vaswani-attention-paper' TO 'transformer-adoption'
    MAX_DEPTH 5
    MIN_STRENGTH 0.2
    RETURN causal_path, total_strength AS total_confidence
);

-- Returns:
-- path: paper → cited_by_500_papers(0.95) → adopted_by_google(0.88) → industry_shift(0.75)
-- total_confidence: 0.95 × 0.88 × 0.75 = 0.627
```

### 7.3 Provenance Tracking

Every record tracks its full lineage:

```sql
-- Who/what created this record?
SELECT _sources FROM research_papers WHERE _id = :paper_id;

-- Returns:
-- [
--   { origin: "arxiv:2301.12345", method: "import", confidence: 0.95 },
--   { origin: "model:embedding-3-small", method: "inference", confidence: 0.85 },
--   { origin: "user:reviewer-7", method: "manual_annotation", confidence: 0.90 }
-- ]

-- Find all records that came from a specific model version
SELECT * FROM knowledge_base
WHERE 'model:gpt-4-0613' IN _source_origins;

-- Find records with conflicting information
SELECT a._id, b._id, a.data, b.data
FROM facts a
JOIN a._contradicts AS c ON c.target = b._id
WHERE a._collection = 'medical_knowledge'
ORDER BY abs(a._confidence - b._confidence) ASC;  -- most contentious first
```

### 7.4 Contradiction Detection

FunDB can automatically detect contradictions when new data is inserted:

```
New Record Insert
       │
       ▼
┌──────────────────┐
│  Semantic Check  │  Compare embedding of new record against existing
│  (optional,      │  records in same collection
│   configurable)  │
│                  │  If cosine_sim > 0.85 AND semantic_polarity = opposite:
│                  │    → Auto-link as _contradicts
│                  │    → Adjust confidence of both records
│                  │    → Emit CONTRADICTION event
└──────────────────┘
```

This is configurable per collection — high-stakes collections (medical, legal) can enable strict contradiction detection, while others can disable it for performance.

---

## 8. Context-Aware Retrieval

### 8.1 The Problem

LLMs have finite context windows. Returning 1000 relevant documents when the agent can only process 20 is waste. Returning 20 documents that are all semantically identical is also waste. The database should optimize **what** it returns for the consumer's constraints.

### 8.2 The WITHIN CONTEXT Clause

```sql
SELECT * FROM knowledge_base
WHERE _vector('embedding') <-> :query < 0.6
WITHIN CONTEXT (
    max_tokens: 4000,          -- hard budget
    coherence: 0.6,            -- min pairwise similarity between results
    diversity: 0.3,            -- MMR lambda (0=max diversity, 1=max relevance)
    include_contradictions: true,  -- show both sides
    priority: [_confidence DESC, recency DESC]  -- tiebreaker
);
```

### 8.3 Execution Pipeline

```
Step 1: Candidate Generation
    Standard ANN search → oversample 10x the final count
    e.g., max_tokens=4000 ≈ ~20 records → fetch 200 candidates

Step 2: Token Estimation
    For each candidate: estimated_tokens = byte_length(data) / 4
    Running total tracks budget consumption

Step 3: MMR Selection (Maximal Marginal Relevance)
    ┌─────────────────────────────────────────────────┐
    │  WHILE token_budget > 0 AND candidates remain:  │
    │                                                  │
    │  score(doc) = λ × sim(doc, query)               │
    │             - (1-λ) × max(sim(doc, selected))   │
    │               ▲ relevance    ▲ redundancy        │
    │                                                  │
    │  Select doc with highest score                   │
    │  Add to result set                               │
    │  Subtract doc tokens from budget                 │
    └─────────────────────────────────────────────────┘

Step 4: Coherence Check
    Compute pairwise similarity of selected set
    If avg_similarity < coherence_threshold:
        Remove most divergent document, re-run step 3

Step 5: Contradiction Inclusion (if enabled)
    For each selected doc with _contradicts links:
        Include highest-confidence contradiction
        (still within token budget)

Step 6: Return ordered result
    Sort by priority criteria
    Include metadata: total_confidence, coverage_score, coherence_score
```

### 8.4 Response Metadata

Context-aware queries return metadata about the result quality:

```json
{
    "results": [...],
    "context_metadata": {
        "tokens_used": 3847,
        "tokens_budget": 4000,
        "coverage_score": 0.73,
        "coherence_score": 0.68,
        "diversity_score": 0.45,
        "avg_confidence": 0.82,
        "contradictions_found": 2,
        "candidates_evaluated": 200,
        "candidates_selected": 18
    }
}
```

This lets the agent know: "I used 18 of 200 candidates, covering 73% of the topic, with 82% average confidence, and there are 2 active contradictions in the results."

---

## 9. Indexing Strategy

### 9.1 Index Types

| Index Type | Data Type | Algorithm | Use Case |
|------------|-----------|-----------|----------|
| **FunBTree** | Scalars (int, string, timestamp) | B+Tree with prefix compression | Equality, range, temporal |
| **FunVector** | float32[], float16[], binary | HNSW (Hierarchical Navigable Small World) | ANN similarity search |
| **FunGraph** | Edges (SPO triples) | Adjacency list + reverse index | Traversal, path queries |
| **FunText** | Text fields | Inverted index (BM25 + TF-IDF) | Full-text search, hybrid retrieval |
| **FunTemporal** | Timestamp ranges | Interval B+Tree (R-tree variant) | Bitemporal range queries |
| **FunComposite** | Mixed | Multi-dimensional (scalar + vector) | Pre-filtered ANN |
| **FunCausal** | Causal edges | DAG index with transitive closure cache | Causal path queries |
| **FunConfidence** | float32 confidence scores | Histogram + B+Tree | Confidence-filtered queries |

### 9.2 HNSW Implementation Details

```
Layer 3:   [A] ──────────────────── [M]
            │                         │
Layer 2:   [A] ───── [D] ──── [H] ── [M]
            │         │        │       │
Layer 1:   [A]─[B]─[D]─[F]─[H]─[J]─[M]─[N]
            │   │   │   │   │   │   │   │
Layer 0:   [A][B][C][D][E][F][G][H][I][J][K][L][M][N]

Parameters (tunable per collection):
  M  = 16    (max connections per node)
  ef_construction = 200  (build-time beam width)
  ef_search = 50..500    (query-time beam width, adaptive)
```

**FunDB HNSW enhancements over vanilla:**
1. **Quantization-aware search:** Uses PQ codes for initial distance estimation, full precision only for top candidates
2. **Tenant-isolated graphs:** Each tenant's vectors live in a separate HNSW graph partition — no cross-tenant leakage during traversal
3. **Incremental updates:** Supports insert/delete without full rebuild (lazy tombstone + periodic graph compaction)
4. **Disk-resident layers:** Only Layer 2+ fits in memory; Layer 0-1 use memory-mapped files with LRU eviction

### 9.3 Causal Index (NEW)

The FunCausal index enables efficient causal path queries:

```
Structure: DAG (Directed Acyclic Graph) index
           with materialized transitive closure for frequent paths

┌──────────┐     ┌──────────┐     ┌──────────┐
│  Event A │────→│  Event B │────→│  Event C │
│  (0.9)   │     │  (0.85)  │     │  (0.7)   │
└──────────┘     └────┬─────┘     └──────────┘
                      │
                      ▼
                 ┌──────────┐
                 │  Event D │
                 │  (0.6)   │
                 └──────────┘

Transitive Closure Cache (top-K frequent paths):
  A → C : strength 0.9 × 0.85 = 0.765
  A → D : strength 0.9 × 0.85 × 0.6 = 0.459

Index operations:
  INSERT_CAUSAL(source, target, type, strength) → O(1) amortized
  QUERY_PATH(from, to, max_depth) → O(d × branching_factor)
  QUERY_EFFECTS(event, max_depth) → BFS on DAG index
  QUERY_CAUSES(event, max_depth) → Reverse BFS
```

### 9.4 Adaptive Index Advisor

FunDB includes an ML-driven **Index Advisor** that continuously monitors query patterns:

```
┌──────────────────┐     ┌─────────────────┐     ┌──────────────────┐
│  Query Telemetry │────→│  Pattern        │────→│  Index           │
│  Collector       │     │  Classifier     │     │  Recommender     │
│                  │     │  (lightweight   │     │                  │
│  • query plans   │     │   gradient      │     │  • suggest new   │
│  • exec times    │     │   boosted tree) │     │  • suggest drop  │
│  • scan ratios   │     │                 │     │  • suggest merge  │
│  • cache misses  │     │  Clusters:      │     │  • auto-create   │
│  • confidence    │     │  • point lookup │     │    (if enabled)  │
│    filter usage  │     │  • range scan   │     │                  │
│  • causal path   │     │  • ANN search   │     │                  │
│    depth stats   │     │  • graph trav.  │     │                  │
│                  │     │  • causal trace │     │                  │
└──────────────────┘     └─────────────────┘     └──────────────────┘
```

The advisor:
- Tracks the ratio of `rows_scanned / rows_returned` per query pattern
- Identifies **missing indexes** when scan ratios exceed thresholds
- Identifies **unused indexes** consuming storage and write amplification
- Monitors causal path query patterns to decide when to materialize transitive closures
- Can **auto-create** indexes in autonomous mode, or **suggest** in advisory mode
- Uses a lightweight GBT model (< 10MB) trained on workload telemetry — no external ML dependency

---

## 10. Consistency Model

### 10.1 Tunable Consistency Spectrum

FunDB provides a **tunable consistency model** per operation:

```
 Strong ◄──────────────────────────────────────► Eventual
   │                                                │
   │  SERIALIZABLE   BOUNDED    CAUSAL    EVENTUAL  │
   │  (Raft quorum)  STALENESS  SESSION   (async)   │
   │                                                │
   │  Default for:   Default:   Default:  Default:  │
   │  transactions   analytics  reads     time-     │
   │  schema DDL     dashboards agent     series    │
   │                            memory    ingestion │
```

| Level | Mechanism | Latency | Use Case |
|-------|-----------|---------|----------|
| **SERIALIZABLE** | Raft quorum write + linearizable reads | ~5-15ms | Financial transactions, schema changes |
| **BOUNDED_STALENESS** | Read from replica within time bound T | ~1-5ms | Analytics, dashboards (T=5s default) |
| **CAUSAL_SESSION** | Session token tracks causal dependencies | ~1-3ms | Agent memory (read-your-writes guarantee) |
| **EVENTUAL** | Async replication, best-effort | < 1ms | Time-series ingestion, embedding batch loads |

### 10.2 Transaction Model

```sql
BEGIN TRANSACTION ISOLATION SERIALIZABLE;

-- Insert document with embedding and confidence
INSERT INTO papers (title, abstract, _vectors.content, _confidence, _sources)
VALUES ('Attention Is All You Need',
        'We propose a new architecture...',
        embed('Attention Is All You Need...'),
        0.95,
        [{ origin: 'arxiv:1706.03762', method: 'import' }]);

-- Create graph edges
INSERT INTO _edges (source, label, target, properties)
VALUES (last_insert_id(), 'authored_by', 'vaswani-2017', {role: 'primary'});

-- Create causal link
INSERT INTO _caused_by (source, target, relation, strength, mechanism)
VALUES ('transformer-adoption', last_insert_id(), 'CAUSED', 0.9, 'foundational paper');

COMMIT;
```

Transactions span document writes, vector updates, graph edge creation, confidence metadata, and causal links **atomically**. This is a key differentiator — in MongoDB + separate vector DB setups, cross-store consistency requires application-level orchestration.

---

## 11. Distributed Architecture

### 11.1 Cluster Topology

```
┌─────────────────────────────────────────────────────────┐
│                   CONTROL PLANE                          │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │  Meta Leader │  │  Meta        │  │  Meta         │  │
│  │  (Raft)     │  │  Follower 1  │  │  Follower 2   │  │
│  └─────────────┘  └──────────────┘  └───────────────┘  │
│                                                          │
│  Responsibilities:                                       │
│  • Shard map / routing table                            │
│  • Schema catalog                                        │
│  • Tenant registry                                       │
│  • Cluster membership                                    │
│  • Confidence propagation rules                         │
│  • Causal index transitive closure schedules            │
└────────────────────────────┬────────────────────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
    ┌─────────▼──┐  ┌───────▼────┐  ┌──────▼─────┐
    │  Data Node │  │  Data Node │  │  Data Node │
    │  Group 1   │  │  Group 2   │  │  Group 3   │
    │            │  │            │  │            │
    │ ┌────────┐ │  │ ┌────────┐ │  │ ┌────────┐ │
    │ │Leader  │ │  │ │Leader  │ │  │ │Leader  │ │
    │ │Shard   │ │  │ │Shard   │ │  │ │Shard   │ │
    │ │A1,B2,C3│ │  │ │A2,B3,C1│ │  │ │A3,B1,C2│ │
    │ ├────────┤ │  │ ├────────┤ │  │ ├────────┤ │
    │ │Follower│ │  │ │Follower│ │  │ │Follower│ │
    │ │A1,B2,C3│ │  │ │A2,B3,C1│ │  │ │A3,B1,C2│ │
    │ └────────┘ │  │ └────────┘ │  │ └────────┘ │
    └────────────┘  └────────────┘  └────────────┘

    Sharding: consistent hashing on (_tenant, _collection, _id)
    Replication factor: 3 (configurable)
    Shard split: automatic at 256MB (configurable)
```

### 11.2 Sharding Strategy

**Shard Key:** `hash(_tenant_id, _collection) → virtual_shard → physical_node`

- **Virtual shards:** 4096 virtual shards mapped to physical nodes via consistent hashing
- **Range-based sub-sharding** within a virtual shard for temporal data (enabling time-range partition pruning)
- **Vector co-location:** Vectors for the same collection/tenant are co-located on the same shard group to maintain HNSW graph locality
- **Causal co-location:** Records connected by causal edges are preferentially co-located to minimize cross-shard causal path queries

**Rebalancing:**
- Automatic shard splitting when size exceeds threshold
- Background data migration with rate limiting to avoid I/O starvation
- Zero-downtime rebalancing — reads continue from old shard until migration completes

### 11.3 Replication

```
Write Flow (Raft-based):

Client ──→ Leader ──→ WAL append ──→ Replicate to followers
                                      │
                         ┌─────────────┼─────────────┐
                         ▼             ▼             ▼
                    Follower 1    Follower 2    Follower 3
                         │             │             │
                    ACK ─┘        ACK ─┘             │
                         │                           │
                    Quorum (2/3) ──→ Commit ──→ Reply to client
```

- **Synchronous replication** (Raft quorum) for SERIALIZABLE writes
- **Asynchronous replication** for EVENTUAL consistency (time-series, bulk embedding loads)
- **Read replicas** serve BOUNDED_STALENESS and CAUSAL reads, offloading the leader
- **Cross-region replication** with configurable lag tolerance for geo-distributed deployments

---

## 12. Security & Multi-Tenant Isolation

### 12.1 Isolation Architecture

```
┌───────────────────────────────────────────────────┐
│                 TENANT ISOLATION                    │
│                                                     │
│  Level 1: Logical Isolation (default)              │
│  ┌─────────────────────────────────────────────┐   │
│  │  • tenant_id embedded in every record       │   │
│  │  • Query engine injects tenant_id predicate │   │
│  │  • Separate HNSW graphs per tenant          │   │
│  │  • Index partitioning by tenant             │   │
│  │  • Separate causal DAG per tenant           │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  Level 2: Resource Isolation (premium)             │
│  ┌─────────────────────────────────────────────┐   │
│  │  • Dedicated shard groups per tenant         │   │
│  │  • CPU / memory / IOPS quotas (cgroups v2)  │   │
│  │  • Separate WAL streams                     │   │
│  │  • Network namespace isolation              │   │
│  └─────────────────────────────────────────────┘   │
│                                                     │
│  Level 3: Physical Isolation (enterprise)          │
│  ┌─────────────────────────────────────────────┐   │
│  │  • Dedicated nodes per tenant               │   │
│  │  • Separate encryption keys (per-tenant KMS)│   │
│  │  • Independent backup schedules             │   │
│  │  • Full network segmentation                │   │
│  └─────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────┘
```

### 12.2 Security Features

| Feature | Implementation |
|---------|---------------|
| **Encryption at rest** | AES-256-GCM per page; key hierarchy: master key → tenant key → page key |
| **Encryption in transit** | mTLS between all nodes; TLS 1.3 for client connections |
| **Authentication** | SCRAM-SHA-256, X.509 certificates, OAuth 2.0 / OIDC integration |
| **Authorization** | RBAC with collection-level and field-level ACLs |
| **Audit log** | Immutable, append-only audit trail with tamper detection (Merkle tree) |
| **Vector privacy** | Optional differential privacy noise injection for shared embedding spaces |
| **Data residency** | Shard placement constraints to enforce geographic data residency |
| **Provenance integrity** | _sources chain is append-only; provenance cannot be retroactively altered |

### 12.3 AI-Specific Security

- **Prompt injection protection:** Query parameterization prevents injection through vector payloads
- **Embedding isolation:** Tenant HNSW graphs are physically separate — no ANN traversal can cross tenant boundaries
- **Model access control:** Built-in `embed()` function supports ACLs on which tenants can use which embedding models
- **PII detection:** Optional column-level PII tagging with automatic redaction in query results
- **Confidence tampering protection:** _confidence scores are computed server-side; clients can set initial confidence but modifiers (corroboration, contradiction) are system-managed

---

## 13. Agent Memory Subsystem

A first-class subsystem for AI agent state management with **bidirectional learning**:

### 13.1 Memory Operations

```sql
-- Create an agent memory space
CREATE AGENT MEMORY space_alpha
  WITH (
    retention_policy = '90 days',
    max_entries = 100000,
    embedding_model = 'text-embedding-3-small',
    consolidation_strategy = 'semantic_merge',
    learning_mode = 'bidirectional'           -- NEW: enable feedback learning
  );

-- Store a memory
INSERT INTO AGENT MEMORY space_alpha
  (agent_id, content, importance, context, _confidence)
VALUES
  ('agent-42',
   'User prefers concise responses',
   0.85,
   'conversation-2025-01-15',
   0.80);

-- Retrieve relevant memories (semantic + recency + importance)
SELECT * FROM AGENT MEMORY space_alpha
WHERE agent_id = 'agent-42'
RECALL BY (
  semantic_similarity(:current_context, weight: 0.5),
  recency(decay: 'exponential', half_life: '7 days', weight: 0.3),
  importance(weight: 0.2)
)
LIMIT 20;

-- Consolidate old memories (merge similar, forget irrelevant)
CONSOLIDATE AGENT MEMORY space_alpha
WHERE agent_id = 'agent-42'
  AND created_at < now() - interval '30 days';
```

### 13.2 Generative Memory Model

The memory subsystem implements a **generative memory model** inspired by cognitive science:
- **Encoding:** New memories are automatically embedded and indexed
- **Retrieval:** Multi-signal ranking combining semantic similarity, temporal recency, and importance scores
- **Consolidation:** Periodic background process that merges semantically similar memories and decays unimportant ones
- **Forgetting:** Configurable retention policies with graceful degradation (memories don't disappear — they lose priority)

### 13.3 Bidirectional Learning (NEW)

The memory subsystem learns from agent behavior to improve future retrievals:

```
┌──────────────────────────────────────────────────────────────────┐
│               BIDIRECTIONAL LEARNING LOOP                         │
│                                                                   │
│  ┌─────────────┐    ┌──────────────┐    ┌──────────────────────┐│
│  │  Agent      │───→│  RECALL BY   │───→│  Results             ││
│  │  queries    │    │  (retrieval) │    │  [r1, r2, ..., r20]  ││
│  │  memory     │    │              │    │                       ││
│  └─────────────┘    └──────────────┘    └──────────┬───────────┘│
│                                                     │            │
│                                          Agent uses │            │
│                                          r2, r5, r7 │            │
│                                          ignores rest│            │
│                                                     │            │
│  ┌──────────────────────────────────────────────────▼──────────┐│
│  │  FEEDBACK COLLECTOR                                          ││
│  │                                                              ││
│  │  Implicit signals:                                           ││
│  │  • Which results were included in agent's next action?      ││
│  │  • Which were ignored?                                       ││
│  │  • Did the agent re-query with different terms? (= poor     ││
│  │    results)                                                  ││
│  │  • Did the agent's task succeed after using these memories?  ││
│  │                                                              ││
│  │  Explicit signals:                                           ││
│  │  • Agent calls FEEDBACK(memory_id, useful: true/false)      ││
│  │  • Agent updates importance score                            ││
│  └──────────────────────────────────┬───────────────────────────┘│
│                                      │                           │
│  ┌──────────────────────────────────▼───────────────────────────┐│
│  │  LEARNING-TO-RANK MODEL (per agent, per collection)          ││
│  │                                                              ││
│  │  Algorithm: LambdaMART (gradient boosted trees)              ││
│  │  Model size: ~1MB per agent                                  ││
│  │  Features:                                                   ││
│  │  • semantic_similarity (vector distance)                     ││
│  │  • temporal_recency (time decay)                             ││
│  │  • importance_score (explicit)                               ││
│  │  • access_frequency (how often retrieved)                    ││
│  │  • use_frequency (how often actually used)                   ││
│  │  • context_match (embedding sim to current context)          ││
│  │  • confidence_score                                          ││
│  │  • source_reliability (historical accuracy of source)        ││
│  │                                                              ││
│  │  Retraining: Online incremental updates every N queries      ││
│  │  Cold start: Falls back to default RECALL BY weights         ││
│  └──────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────┘
```

**Key insight:** The default `RECALL BY` weights (0.5 semantic, 0.3 recency, 0.2 importance) are a starting point. Over time, the LTR model learns that for *this specific agent*, recency matters more (it works on fast-moving topics), or confidence matters more (it makes critical decisions), and adjusts rankings accordingly.

```sql
-- Check how the model has adapted for an agent
SELECT * FROM AGENT MEMORY space_alpha
EXPLAIN RANKING
WHERE agent_id = 'agent-42';

-- Returns:
-- {
--   "learned_weights": {
--     "semantic_similarity": 0.35,  -- decreased (agent cares less about exact match)
--     "recency": 0.40,             -- increased (agent values fresh information)
--     "importance": 0.10,          -- decreased
--     "confidence": 0.15           -- new signal learned from behavior
--   },
--   "model_accuracy": 0.78,
--   "training_samples": 1247
-- }
```

---

## 14. Causal Reasoning Engine

> **v0.3 Note:** This section was fully redesigned following an internal hackathon sprint (2026-02-28). The previous v0.2 tier-based design was a placeholder. This version contains concrete algorithms, physical operators, latency budgets, and honest failure mode analysis derived from five specialized agents working independently and then in cross-team debate. See [hackathon/](../../hackathon/) for full research notes.

### 14.1 Core Design Philosophy

The fundamental challenge of causal reasoning in a database: **no observational data can substitute for a randomized experiment.** Every causal claim from FunDB is an inference from imperfect signals — never certainty.

FunDB's approach is **multi-signal causal assessment**: combine five heterogeneous signals that measure different aspects of causality, fuse them with mathematically rigorous evidence theory, and be radically honest about uncertainty. The output is not a yes/no answer — it is a belief interval with a conflict flag and a detailed warning list.

This is computationally tractable because FunDB already has all the ingredients in one engine: vectors (for semantic mechanism detection), graphs (for causal paths and confounder search), time-series (for quasi-experiments), confidence scores (for provenance weighting), and bitemporal data (for temporal ordering). No external system is needed.

### 14.2 The Five Causal Signals

Each signal measures a different necessary (but not sufficient) condition for causation. Together they form a computational instantiation of Bradford Hill's criteria (1965):

| Signal | Bradford Hill Criterion | FunDB Source | Reliability (default) |
|--------|------------------------|--------------|----------------------|
| **S1** Temporal Precedence | Temporality | Bitemporal `_valid_from` | r₁ = 0.60 |
| **S2** Semantic Mechanism | Plausibility + Coherence | HNSW embeddings + NLI model | r₂ = 0.50 |
| **S3** Confounder Exclusion | Specificity | Graph + vector + text search | r₃ = 0.70 |
| **S4** Natural Experiments | Experiment | Time-series + HNSW control groups | r₄ = per-method (see 14.5) |
| **S5** Source Consensus | Consistency | `_sources` provenance + `_supports` refs | r₅ = 0.40 |

**Critical rule:** S1 (temporal precedence) is a **necessary condition gate**, not just one signal among equals. If B precedes A in time, no amount of other evidence should yield high causal confidence. The gate is applied post-fusion: `final_score = min(fusion_score, s1 × 0.5)` when `s1 < 0.2`.

### 14.3 Signal Fusion — Modified Dempster-Shafer

Signals are combined using Murphy's modified Dempster-Shafer evidence theory. This framework was chosen over Bayesian combination specifically because it **explicitly represents ignorance**: when a signal is unavailable, it adds pure uncertainty mass rather than silently reverting to a prior.

**Frame of discernment:** `Θ = {C, ¬C}` — causal vs. not causal.

**Signal-to-mass conversion:**
```
Given signal value sᵢ ∈ [0,1] and reliability rᵢ ∈ (0,1]:

  m({C})    = rᵢ × sᵢ
  m({¬C})   = rᵢ × (1 − sᵢ)
  m({C,¬C}) = 1 − rᵢ          ← ignorance mass

Missing signal → rᵢ = 0 → m({C,¬C}) = 1.0 (total ignorance, no update)
```

**Murphy combination (n signals):**
```
Step 1: Average all mass functions → m_avg
Step 2: Self-combine m_avg using Dempster's rule (n−1) times
Step 3: Extract outputs:
  causal_confidence = Bel(C) = m_final({C})
  uncertainty_width = Pl(C) − Bel(C) = m_final({C,¬C})
  conflict_degree   = accumulated K (conflict factor)
  plausibility      = Pl(C) = m_final({C}) + m_final({C,¬C})
```

**Theoretical guarantees:**
- Adding a supporting signal never decreases confidence (monotonicity)
- Missing signals always increase `uncertainty_width`, never decrease it
- `causal_confidence + uncertainty_width ≤ 1.0` always holds
- Combination order does not affect result (commutativity)

**Computed confidence ceiling (Subgroup A result):**

The maximum achievable confidence from observational data is not a hardcoded constant — it is computed from the quality of the best quasi-experimental evidence:

```
C_max = 0.90 × f_method × f_independence

f_method:
  RDD              → 1.00   (strongest: threshold-based randomization)
  Event Study      → 0.90
  DiD              → 0.85
  Synthetic Control → 0.85
  ITS              → 0.70   (weakest: no control group)
  None found       → 0.67   (observational only)

f_independence = sqrt(n_eff / n)
  where n_eff = effective signal count adjusted for inter-signal correlation
  (eigenvalue decomposition of 5×5 correlation matrix ρ)

Typical C_max with DiD as best method: ~0.65
Typical C_max with RDD: ~0.80
No experiment found: ~0.57
```

The ceiling is enforced: `causal_confidence = min(causal_confidence, C_max)`. The output includes `max_achievable_confidence: C_max` and its breakdown so the consumer understands why certainty is limited.

### 14.4 Signal S2 — MechanismDetect (Semantic)

Detects whether a plausible causal mechanism exists between events A and B. Operates in three tiers with early exit:

```
Tier 1 — Direct Evidence (< 1ms):
  Check FunCausal DAG index for existing causal edges A → ... → B (depth ≤ 3)
  If found: score = product(edge.strength), type = DIRECT
  Cap: none (direct edges can reach 1.0)

Tier 2 — Semantic Bridge (< 10ms):
  Search HNSW for records M near midpoint((A.vec + B.vec)/2)
  Score each bridge: harmonic_mean(sim(M,A), sim(M,B)) × temporal_bonus × M._confidence
  Temporal bonus 1.2× if A._valid_from < M._valid_from < B._valid_from
  If top bridge score > 0.55: return score, type = BRIDGED
  Cap: 0.85 (bridge alone is not proof)

  Augmentation (Subgroup B integration):
    Enrich HNSW search with: query = 0.7×A.vec + 0.3×bridge.vec
    This uses semantic bridges as hints for control group search in S4

Tier 3 — NLI Inference (< 80ms on CPU):
  Model: DeBERTa-v3-base fine-tuned on MNLI (~400MB ONNX, shared with Semantic Interface)
  Forward:  P(entailment | A.text → B.text)
  Reverse:  P(entailment | B.text → A.text)
  Asymmetry bonus: direction_score = forward × (1 + max(0, forward − reverse))
  Cap: 0.75 (NLI without data grounding is weakest evidence)
  Tag: mechanism_source = "model_inferred" (vs. "data_grounded" for Tiers 1-2)
```

**Evidence type enum** (surfaces quality to developers):
`DIRECT` | `BRIDGED` | `CHAIN` | `INFERRED` | `NONE`

### 14.5 Signal S3 — ConfounderSearch (Semantic)

Searches the database for records that could be common causes of both A and B. Uses four parallel strategies, fusing by multi-strategy convergence:

```
Strategy 1 — Embedding Triangle:
  HNSW search from A.vec, filter: _valid_from < min(A, B)
  Score: harmonic_mean(sim(Z,A), sim(Z,B)) × (1 − |sim_a − sim_b|)

Strategy 2 — Common Ancestor:
  Graph reverse-traverse from A (depth=2) and B (depth=2)
  Intersection = structural confounder candidates

Strategy 3 — Text Co-occurrence:
  Find TF-IDF terms shared between documents related to A and B
  Search FunText for records matching shared terms, pre-dating both A and B

Strategy 4 — Temporal Context:
  Temporal range scan: [min(A,B) − 30 days, min(A,B)]
  Filter: cosine_sim > 0.3 with both A and B
  Weight by recency (exponential decay)

Fusion: records found by N strategies get 1 + 0.2×(N−1) boost
Coverage score: records_evaluated / total_records_in_window
  (honest about what was NOT searched)
```

**S3 signal to fusion:** `s3 = 1.0 − max(confounder_scores)` (strong confounder = low s3 = evidence against causation).

**Coverage score feeds the ceiling:** Low coverage → lower `f_independence` → lower `C_max`.

**r₄ per method:**

| Method | r₄ | When used |
|--------|-----|-----------|
| RDD | 0.90 | Threshold-based assignment detected |
| Event Study | 0.80 | ≥5 instances of same cause type |
| DiD | 0.75 | Control group passes balance (SMD < 0.25) + parallel trends (p > 0.05) |
| Synthetic Control | 0.75 | Pre-treatment RMSE < 10% |
| ITS | 0.55 | Always available; no control group |

**r₄ dynamic degradation (Team Lead decision):**

The base r₄ is degraded by validity diagnostics, not fixed:
```
r4_effective = r4_base × r4_degradation

r4_degradation = 1.0 − (0.3 × parallel_trends_fail)
                       − (0.2 × balance_fail_severity)
                       − (0.15 × confounder_signal_strength)
```

This means a DiD that barely passes parallel trends (p=0.06) contributes less than a DiD with p=0.45.

### 14.6 Signal S4 — NaturalExperimentDetector (Quasi-Experimental)

Automatically finds historical quasi-experiments for a (cause, effect) pair using a cascade of methods ordered by statistical credibility:

```
NaturalExperimentDetector(cause_id, effect_id):

  Pre-flight: minimum 10 observations, cause precedes effect temporally
  If fails → return {s4: 0.5, method: NONE}  ← ignorance, not evidence against

  Stage 1 — RDD Detection (if threshold-based assignment exists):
    Detect running variables with threshold rules in cause metadata
    McCrary density test (p > 0.1 required)
    Local bandwidth selection (Imbens-Kalyanaraman)
    If valid → return {s4, method: RDD, r4: 0.90}

  Stage 2 — Event Study (if ≥5 similar causes exist):
    Find similar causes via HNSW: sim(cause.vec, candidate.vec) > 0.75
    Compute dynamic treatment effects at each lag −T..+T
    Pre-treatment coefficients test (pre-trend p > 0.1 required)
    If valid → return {s4, method: EVENT_STUDY, r4: 0.80}

  Stage 3 — DiD (if control group passes balance):
    Control group: entities similar to treated entity that did NOT receive cause
    Composite similarity = 0.5×semantic + 0.3×graph_neighborhood + 0.2×temporal_context
    Balance check: SMD < 0.25 for all covariates
    Parallel trends test: p > 0.05 on pre-treatment period
    SUTVA check: exclude controls affected by treatment spillover
    If valid → return {s4, method: DID, r4: 0.75}

  Stage 4 — Synthetic Control (if pre-treatment fit is good):
    Donor pool: HNSW candidates with similarity > 0.6
    Constrained optimization: minimize pre-treatment RMSE
    Accept if RMSE < 10% of pre-treatment mean
    Placebo inference: fraction of placebos with larger absolute effect
    If valid → return {s4, method: SYNTHETIC_CONTROL, r4: 0.75}

  Stage 5 — ITS (always available as fallback):
    Segmented regression on effect time-series around cause timestamp
    Newey-West HAC standard errors (autocorrelation-robust)
    Regression-to-mean warning if effect was >2 SD at cause time
    return {s4, method: ITS, r4: 0.55}
```

**S4 to signal conversion:** `s4 = 0.5 + 0.5 × qe_score × direction_sign`
- When no experiment found: `s4 = 0.5` (ignorance center of D-S frame, not evidence against)
- Positive effect in expected direction: `s4 > 0.5`
- Negative or null effect: `s4 < 0.5`

### 14.7 Signal S5 — Source Consensus

```
n_independent = count of root sources (sources with no tracked upstream in _sources chain)
s5 = 1 − (1 − per_source_credibility)^n_independent

Per-source credibility:
  No track record → 0.3
  Some confirmed claims → 0.6
  Strong track record → 0.8

Rate limiting: max 5 sources from same API key/agent within 24h count as 1
Null results tracked: negative queries logged, consensus reflects (confirm:N, disconfirm:M)
```

### 14.8 FunQL Operators

Four verbs with increasing depth and cost:

```sql
-- Fast (S1 + S3 only, ~35ms):
INFER CAUSALITY FROM :event_a TO :event_b;

-- Full assessment (all 5 signals, ~303ms worst case):
ASSESS CAUSALITY
  FROM 'deploy-v2.3.1' TO 'error-spike-2025-02-15'
  WITH natural_experiment_timeout = 380ms
  WITH confounder_lookback = '30 days';

-- Full assessment + Red Team checklist (~363ms):
EXPLAIN CAUSALITY
  FROM 'deploy-v2.3.1' TO 'error-spike-2025-02-15';

-- Explicit path traversal (existing, unchanged):
TRACE CAUSALITY FROM :a TO :b MAX_DEPTH 5 MIN_STRENGTH 0.3;

-- Write confirmed causal edge + update learning loop:
CONFIRM CAUSALITY FROM 'deploy-v2.3.1' TO 'error-spike-2025-02-15'
  WITH strength = 0.92
  WITH mechanism = 'tokenizer regression broke CJK inputs';
```

**Output schema (ASSESS CAUSALITY):**
```json
{
  "causal_confidence":         0.762,
  "uncertainty_width":         0.118,
  "conflict_degree":           0.089,
  "plausibility":              0.880,
  "max_achievable_confidence": 0.638,
  "relation":                  "INFLUENCED",
  "signals": {
    "s1_temporal":    { "value": 0.88, "reliability": 0.60 },
    "s2_mechanism":   { "value": 0.73, "type": "BRIDGED", "reliability": 0.50 },
    "s3_confounder":  { "value": 0.52, "coverage": 0.71,  "reliability": 0.70 },
    "s4_experiment":  { "value": 0.91, "method": "DID",   "reliability": 0.69 },
    "s5_consensus":   { "value": 0.62, "n_independent": 2, "reliability": 0.40 }
  },
  "warnings": [
    { "type": "PARTIAL_CONFOUNDER_DETECTED", "severity": "medium",
      "message": "Variable 'traffic_spike' explains ~30% of correlation." },
    { "type": "LOW_SOURCE_COUNT", "severity": "low",
      "message": "Only 2 independent sources. Consensus signal is weak." }
  ],
  "is_partial": false,
  "computation_ms": 187
}
```

### 14.9 Physical Execution Plan

Worst-case latency for `ASSESS CAUSALITY` (all 5 signals, cold path):

```
Timeline (ms):
0ms   ┌─ S1: Temporal scan (bitemporal index) ─────────────────┐ 15ms
0ms   ├─ S2: MechanismDetect ──────────────┐                   │
      │   Tier 1: DAG lookup (1ms)          │                   │
      │   Tier 2: HNSW midpoint (10ms)      │ 135ms worst       │
      │   Tier 3: NLI model (80ms)          │                   │
0ms   ├─ S3: ConfounderSearch ─────────────────────────┐        │
      │   4 strategies in parallel (35ms→70ms w/ load) │ 70ms   │
0ms   ├─ S4: NaturalExperimentDetector ──────────────────────┐  │
      │   Cascade (ITS only: 40ms, DiD: 285ms worst)         │  │
0ms   ├─ S5: Source consensus (provenance graph lookup) ──┐  │  │
                                                          │  │  │
135ms └──────────────── S2 completes ────────────────────┘  │  │
20ms  └────── S5 completes ─────────────────────────────────┘  │
70ms  └────── S3 completes ──────────────────────────────────── │
285ms └────── S4 completes (critical path) ──────────────────── │
                                                                 │
285ms ┌─ SignalFusionOperator (< 1μs) ────────────────────────┐  │
285ms ├─ Ceiling computation (C_max) ─────────────────────────┘  │
285ms ├─ Temporal gate check ─────────────────────────────────┘   │
                                                                    │
285ms Total signals done                                            │
285ms + fusion overhead ≈ 303ms worst case (within 500ms budget)   │

S4 hard cancellation at 380ms → treat S4 as None → is_partial: true
```

**Parallelism model:** S1, S2, S3, S4, S5 run concurrently on separate threads. S4 uses dedicated HNSW reader slots (Team Lead decision) to avoid cache contention with the other signals' vector searches.

### 14.10 Red Team Checklist (EXPLAIN CAUSALITY)

The `EXPLAIN CAUSALITY` verb runs the Skeptic's 15-point checklist and includes the results in the output. Checks run in ~60ms additional overhead:

```
1.  DIRECTION_CHECK         — Could B cause A instead?
2.  COMMON_CAUSE_SCAN       — Is there C preceding both A and B?
3.  COLLIDER_CHECK          — Are we conditioning on a collider?
4.  SAMPLE_BIAS_CHECK       — Survivorship bias in the dataset?
5.  AGGREGATION_LEVEL_CHECK — Does the claim reverse at different granularity?
6.  MULTIPLE_COMPARISONS    — FDR correction on all tests in session
7.  EFFECT_SIZE_CHECK       — Is the effect practically meaningful?
8.  TEMPORAL_STABILITY      — Does it hold in recent data?
9.  MECHANISM_GROUNDING     — Are intermediate events observable?
10. SOURCE_INDEPENDENCE     — Count root sources, not total sources
11. FEEDBACK_LOOP_CHECK     — Was any evidence created from a previous version of this claim?
12. SAMPLE_SIZE_CHECK       — Statistical power for observed effect size
13. ADVERSARIAL_PATTERN     — Too clean? Concentrated source? Recent injection?
14. REGRESSION_TO_MEAN      — Was cause triggered by extreme value of effect?
15. CONFIDENCE_CEILING      — Is C_max being enforced?
```

Each failed check produces a structured warning with severity, message, and recommended action.

### 14.11 Open Problems (Honest)

The hackathon resolved the core architecture but left six problems open for future sprints:

1. **Feedback loop (partial solution only):** When FunDB infers causation and an agent acts on it, the resulting data can reinforce the (possibly false) claim. Current mitigation: `_influenced_by` provenance tags on actions, excluded from re-evaluation. Full solution requires tracking actions outside FunDB — deferred to v2.
2. **Rho matrix (unvalidated):** The inter-signal correlation matrix ρ used in `f_independence` is estimated, not empirically measured. Requires a calibration dataset of confirmed/disconfirmed causal claims.
3. **Binary causation frame:** The D-S frame `{C, ¬C}` loses the distinction between CAUSED, INFLUENCED, and CORRELATED. Post-hoc threshold mapping is a workaround, not a solution.
4. **Domain mismatch in NLI:** The NLI model is general-purpose. Specialized domains (biochemistry, legal, financial) may require domain-specific fine-tuning.
5. **Latent confounders:** No algorithm can find confounders not recorded in the database. The coverage score communicates this, but cannot solve it.
6. **Time-varying SUTVA:** In distributed systems, treatment spillover changes over time (e.g., cascading failures). Static SUTVA checking is insufficient.

```
┌─────────────────────────────────────────────────────────────┐
│              CAUSAL REASONING TIERS                           │
│                                                              │
│  Tier 1 — Explicit Causality (fully implemented)            │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  User/agent declares: "A caused B"                     │ │
│  │  Stored as _caused_by / _effects edges in FunRecord    │ │
│  │  Queryable via TRACE CAUSALITY syntax                  │ │
│  │  DAG index for efficient path traversal                │ │
│  │                                                        │ │
│  │  Difficulty: 2/10 — graph edges, solved problem        │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Tier 2 — Inferred Causality (implemented)                  │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Temporal precedence: A happened before B in context   │ │
│  │  Granger causality: time-series A predicts B           │ │
│  │  Co-occurrence: A and B change together consistently   │ │
│  │  Statistical tests with confidence scores              │ │
│  │                                                        │ │
│  │  Difficulty: 5/10 — known algorithms, engineering      │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Tier 3 — Interventional Causality (research frontier)      │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Do-calculus: "If I change X, what happens to Y?"      │ │
│  │  Counterfactual: "Would Y have happened without X?"    │ │
│  │  Requires structural causal model (SCM)                │ │
│  │  Based on Judea Pearl's framework                      │ │
│  │                                                        │ │
│  │  Difficulty: 8/10 — active research, partial solutions │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### 14.2 Tier 1 — Explicit Causal Edges

The simplest and most immediately useful form: agents and users declare causal relationships as first-class edges.

```sql
-- Declare a causal relationship
INSERT INTO _caused_by (
    source_id,      -- the effect
    target_id,      -- the cause
    relation,       -- CAUSED | INFLUENCED | CORRELATED | PRECEDED
    strength,       -- 0.0–1.0
    mechanism       -- human-readable explanation
) VALUES (
    'error-spike-2025-02-15',
    'deploy-v2.3.1',
    'CAUSED',
    0.92,
    'New model version introduced regression in embedding pipeline'
);

-- Query: what caused this error?
SELECT * FROM events
TRACE CAUSALITY TO 'error-spike-2025-02-15'
MAX_DEPTH 3
MIN_STRENGTH 0.3
RETURN causal_path, total_strength;

-- Returns:
-- ┌────────────────────────┬──────────┬─────────────────────────────┐
-- │ causal_path            │ strength │ mechanism                    │
-- ├────────────────────────┼──────────┼─────────────────────────────┤
-- │ deploy-v2.3.1          │ 0.92     │ regression in embedding...  │
-- │   → config-change-2.3  │ 0.85     │ changed batch size param    │
-- │     → team-decision-01 │ 0.70     │ optimize for throughput     │
-- └────────────────────────┴──────────┴─────────────────────────────┘

-- Query: what are the downstream effects of this decision?
SELECT * FROM events
TRACE CAUSALITY FROM 'team-decision-01'
MAX_DEPTH 5
MIN_STRENGTH 0.2
ORDER BY total_strength DESC;
```

### 14.3 Tier 2 — Inferred Causality

FunDB can automatically detect potential causal relationships from data patterns:

**Temporal Precedence:**
```sql
-- Find events that consistently precede error spikes
SELECT candidate_cause, lag, correlation
FROM INFER CAUSALITY (
    effect:  (SELECT ts, value FROM metrics WHERE name = 'error_rate'),
    search:  (SELECT ts, value FROM metrics WHERE name IN ('deploys', 'load', 'memory')),
    method:  'temporal_precedence',
    window:  '1 hour',
    min_correlation: 0.5
);
```

**Granger Causality (for time-series):**
```sql
-- Does deploy frequency Granger-cause error rates?
SELECT granger_test(
    cause_series:  (SELECT ts, value FROM metrics WHERE name = 'deploy_count'
                    GROUP BY time_bucket('1h', ts)),
    effect_series: (SELECT ts, value FROM metrics WHERE name = 'error_rate'
                    GROUP BY time_bucket('1h', ts)),
    max_lag: 12,          -- test up to 12 hours of lag
    significance: 0.05    -- p-value threshold
);

-- Returns:
-- {
--   "granger_causes": true,
--   "optimal_lag": 3,           -- errors spike ~3 hours after deploys
--   "p_value": 0.003,
--   "f_statistic": 8.72,
--   "confidence": 0.85,
--   "suggested_causal_edge": {  -- auto-suggestion
--     "source": "deploy_count",
--     "target": "error_rate",
--     "relation": "INFLUENCED",
--     "strength": 0.85,
--     "mechanism": "auto:granger(lag=3h, p=0.003)"
--   }
-- }
```

**Co-occurrence Analysis:**
```sql
-- Which events consistently co-occur with customer churn?
SELECT * FROM INFER CAUSALITY (
    effect:  'customer_churn_events',
    search:  'all_customer_events',
    method:  'co_occurrence',
    window:  '30 days before effect',
    min_support: 0.1,
    min_confidence: 0.5
);
```

**Auto-linking inferred causal edges:**

```sql
-- Enable automatic causal inference for a collection
ALTER COLLECTION metrics
SET causal_inference = ON (
    methods: ['temporal_precedence', 'granger'],
    min_confidence: 0.7,
    auto_link: true,            -- automatically create _caused_by edges
    require_approval: false     -- or true, to queue for human review
);
```

### 14.4 Tier 3 — Interventional Causality (Experimental)

This tier implements Pearl's do-calculus for counterfactual and interventional queries. It requires a **Structural Causal Model (SCM)** — a DAG of causal variables with functional relationships.

```sql
-- Define a structural causal model
CREATE CAUSAL MODEL revenue_model AS (
    VARIABLES:
        marketing_spend    REAL,
        website_traffic    REAL,
        conversion_rate    REAL,
        revenue            REAL,

    STRUCTURE:
        marketing_spend -> website_traffic,
        marketing_spend -> conversion_rate,
        website_traffic -> revenue,
        conversion_rate -> revenue,

    -- Functional relationships (learned from data or specified)
    EQUATIONS:
        website_traffic  = f1(marketing_spend) LEARN FROM metrics,
        conversion_rate  = f2(marketing_spend) LEARN FROM metrics,
        revenue          = f3(website_traffic, conversion_rate) LEARN FROM metrics
);

-- Interventional query: "What would revenue be if we set marketing_spend to $100K?"
SELECT expected_revenue, confidence_interval
FROM INTERVENE ON revenue_model
SET marketing_spend = 100000
PREDICT revenue;

-- Counterfactual: "Would revenue have been higher if we hadn't cut marketing?"
SELECT actual_revenue, counterfactual_revenue, difference
FROM COUNTERFACTUAL ON revenue_model
GIVEN observed_data = (SELECT * FROM metrics WHERE month = '2025-01')
HAD marketing_spend = 50000    -- actual was 30000
PREDICT revenue;
```

**Implementation approach:**
- SCM structure stored as a DAG in the graph engine
- Functional relationships learned using lightweight regression models
- Interventions computed by graph surgery (removing incoming edges to intervened variable)
- Counterfactuals use abduction-action-prediction three-step process
- Confidence intervals computed via bootstrap resampling

**Current limitations (honest):**
- Requires explicit SCM definition (auto-discovery is research frontier)
- Linear/additive models only in v1 (nonlinear via neural SCMs planned for v2)
- Correctness depends on SCM specification — garbage model in, garbage answers out
- Computational cost scales with model complexity

### 14.5 Causal Visualization

```sql
-- Generate a causal graph visualization
SELECT * FROM CAUSAL GRAPH
WHERE collection = 'incident_reports'
  AND _sys_from > now() - interval '90 days'
  AND strength > 0.3
FORMAT 'dot';    -- Graphviz DOT format

-- FORMAT options: 'dot', 'mermaid', 'json', 'd3'
```

Returns a visualization-ready graph of all causal relationships in the dataset, useful for dashboards and debugging.

---

## 15. Competitive Analysis

### 15.1 Feature Matrix

| Capability | **FunDB** | PostgreSQL + pgvector | MongoDB Atlas Vector | Oracle 23ai |
|------------|-----------|----------------------|---------------------|-------------|
| Vector search (native) | **First-class** | Extension (pgvector) | Add-on feature | Extension |
| Graph queries (native) | **First-class** | Requires Apache AGE ext. | $graphLookup (limited) | Property Graph (SQL/PGQ) |
| Document model | **First-class** | JSONB (good) | Native (excellent) | JSON duality views |
| Time-series | **First-class** | TimescaleDB extension | Time-series collections | Native |
| Bitemporal versioning | **Built-in** | Manual | Manual | Flashback (system time only) |
| Agent memory | **Built-in** | Not available | Not available | Not available |
| Confidence/provenance | **Built-in** | Not available | Not available | Not available |
| Intent-based queries | **Built-in** | Not available | Not available | Not available |
| Context-aware retrieval | **Built-in** | Not available | Not available | Not available |
| Causal reasoning | **Built-in** | Not available | Not available | Not available |
| Bidirectional learning | **Built-in** | Not available | Not available | Not available |
| Hybrid queries | **Single engine** | Multiple extensions | Aggregation pipeline | Separate subsystems |
| Adaptive indexing | **ML-driven** | Manual EXPLAIN ANALYZE | Manual | Automatic (limited) |
| Multi-tenant isolation | **3-tier native** | Row-level security | Atlas isolation | Pluggable databases |
| Distributed ANN | **Native scatter-gather** | Not distributed | Atlas search (Lucene) | Not distributed |

### 15.2 Why Not Just Extend PostgreSQL?

PostgreSQL is exceptional, but extending it for AI has fundamental limitations:

1. **Process-per-connection model** — Poor for thousands of concurrent agent connections. FunDB uses async I/O with coroutines (Tokio runtime)
2. **Single-node MVCC** — pgvector indexes are local. Distributed ANN requires application-level sharding. FunDB distributes HNSW natively
3. **Extension isolation** — pgvector, PostGIS, Apache AGE, TimescaleDB are separate extensions with separate query planners. They cannot co-optimize `WHERE location <-> point < 5km AND embedding <-> query < 0.3`. FunDB's unified planner reasons across all data types
4. **Write amplification** — PostgreSQL's heap + WAL + index design causes significant write amplification for embedding-heavy workloads. FunDB's LSM design is 3-5x more write-efficient for this pattern
5. **Buffer pool limitations** — HNSW indexes in pgvector compete with relational data for shared_buffers. FunDB has separate memory allocators for vector indexes with dedicated memory pools
6. **No epistemic primitives** — PostgreSQL has no concept of confidence, provenance, or causality. Adding these as columns is possible but without engine-level support (confidence propagation, contradiction detection, causal indexing), the burden falls entirely on the application

### 15.3 Why Not MongoDB?

1. **No native vector indexing** — MongoDB Atlas Vector Search delegates to Lucene. This adds network hops, serialization overhead, and consistency gaps between the document store and the vector index
2. **Weak graph support** — `$graphLookup` is limited to single-collection recursive lookups. Real knowledge graphs need multi-collection path queries with weighted edges
3. **No temporal versioning** — Change streams provide event sourcing, but not bitemporal queries. Model reproducibility requires manual snapshotting
4. **Query language fragmentation** — MQL for documents, Atlas Search for vectors, aggregation pipeline for analytics. FunDB unifies these in one language
5. **No knowledge primitives** — No confidence scoring, no provenance tracking, no causal reasoning. MongoDB stores data; FunDB stores knowledge

### 15.4 Why Not Oracle 23ai?

1. **Cost and licensing** — Oracle's enterprise pricing is prohibitive for startups and mid-size AI companies
2. **Legacy architecture** — Decades of backward compatibility constraints limit how aggressively Oracle can optimize for AI workloads
3. **Vendor lock-in** — Proprietary protocol, tooling, and cloud dependency. FunDB is open-core with wire-protocol compatibility for PostgreSQL clients
4. **Operational complexity** — Oracle's DBA requirements are incompatible with the operational simplicity AI teams need

---

## 16. Trade-offs & Limitations

### 16.1 Honest Trade-offs

| Trade-off | Explanation | Mitigation |
|-----------|-------------|------------|
| **New ecosystem** | No existing tooling, ORMs, or community | PostgreSQL wire-protocol compatibility for day-one client support |
| **LSM write amplification** | Compaction consumes background I/O | Tiered compaction strategy + SSD-optimized I/O scheduling |
| **HNSW memory pressure** | Large vector collections require significant RAM for graph layers | Disk-resident lower layers + PQ compression |
| **Complexity** | Unified multi-model + cognitive features is inherently more complex | Sensible defaults; "just works" for 80% of use cases. Cognitive features are opt-in per collection |
| **Maturity** | New database vs. 30+ years of PostgreSQL hardening | Extensive property-based testing + chaos engineering from day 1 |
| **OLTP trade-off** | LSM reads are slower than B-Tree for point lookups | Bloom filters + hot row cache mitigate; FunDB is not targeting pure OLTP |
| **Consistency cost** | Raft quorum writes add latency vs. single-node | Tunable consistency — use EVENTUAL for tolerant workloads |
| **Confidence overhead** | Storing and propagating confidence adds storage and compute | ~8 bytes per record (float32 + flags); propagation is lazy and cached |
| **Causal completeness** | Tier 3 causal reasoning depends on correct SCM specification | Tier 1-2 are robust; Tier 3 is opt-in and clearly marked as experimental |
| **Semantic Interface accuracy** | Intent parsing may misinterpret ambiguous queries | Three-tier fallback; low-confidence parses return candidates for agent to choose |

### 16.2 What FunDB is NOT

- **Not a replacement for OLTP databases** — For pure transactional workloads (banking, e-commerce order management), PostgreSQL/MySQL remain superior
- **Not a data warehouse** — For petabyte-scale analytical workloads, Snowflake/BigQuery/ClickHouse are purpose-built
- **Not a message queue** — For streaming, use Kafka/Pulsar alongside FunDB
- **Not a general-purpose LLM** — The Semantic Interface parses query intent; it does not generate text, summarize, or reason about content
- **Not a causal oracle** — Causal reasoning is powerful but depends on data quality and model specification; it augments human reasoning, not replaces it

FunDB occupies the **AI knowledge infrastructure** niche — the cognitive database layer between your models and your application, where data becomes knowledge.

---

## 17. Ideal Use Cases

### 17.1 Primary Use Cases

| Use Case | Why FunDB Excels |
|----------|-----------------|
| **RAG Pipelines** | Single query combines vector similarity + metadata filtering + graph context + confidence filtering + context optimization |
| **Autonomous AI Agents** | Built-in agent memory with semantic recall, temporal awareness, consolidation, and bidirectional learning |
| **Knowledge Graphs + Embeddings** | Unified graph traversal + vector search + confidence + causal edges in one transaction |
| **Multi-Modal AI Applications** | Store text, image, audio embeddings alongside structured metadata in one record with provenance tracking |
| **ML Feature Stores** | Bitemporal versioning enables point-in-time feature retrieval for training with confidence-weighted features |
| **Continuous Learning Systems** | Time-travel queries reconstruct exact training datasets at any historical point |
| **Real-time AI Serving** | Sub-5ms vector search with pre-filtered ANN for low-latency inference |
| **Conversational AI** | Agent memory + temporal context + knowledge graph + confidence = richer conversations |
| **Incident Analysis** | Causal reasoning traces root causes across systems: "what caused this outage?" |
| **Decision Support** | Interventional queries: "what would happen if we changed X?" with confidence intervals |

### 17.2 Example Deployment: AI Research Platform

```
┌────────────────────────────────────────────────────────────────┐
│                 AI Research Platform                             │
│                                                                 │
│  ┌──────────────┐  ┌─────────────┐  ┌───────────────────────┐ │
│  │  Paper       │  │  Citation   │  │  Experiment           │ │
│  │  Embeddings  │  │  Graph      │  │  Time-Series          │ │
│  │  (vectors)   │  │  (edges)    │  │  (metrics)            │ │
│  │  + confidence│  │  + causal   │  │  + Granger causality  │ │
│  └──────┬───────┘  └──────┬──────┘  └───────┬───────────────┘ │
│         │                 │                  │                  │
│         └─────────────────┼──────────────────┘                  │
│                           │                                     │
│                    ┌──────▼──────┐                              │
│                    │   FunDB     │ ← Single database            │
│                    │   Cluster   │   for everything             │
│                    └─────────────┘                              │
│                                                                 │
│  Intent Query: "Find papers similar to my draft, written by    │
│   authors in my collaboration graph, published in the          │
│   last 2 years, with reproducible experiments, that I can      │
│   trust (confidence > 0.8)"                                    │
│                                                                 │
│  → ONE hybrid query across vectors + graph + temporal          │
│    + confidence + context-optimized for agent's window         │
└────────────────────────────────────────────────────────────────┘
```

---

## 18. Technology Stack

| Component | Technology | Rationale |
|-----------|-----------|-----------|
| **Core engine** | Rust | Memory safety without GC, predictable latency, SIMD intrinsics |
| **Async runtime** | Tokio | Best-in-class async I/O for Rust; mature ecosystem |
| **Consensus** | Custom Raft (Rust) | Tight integration with storage engine; no external dependency |
| **Serialization** | MessagePack + FlatBuffers | MessagePack for documents (compact), FlatBuffers for IPC (zero-copy) |
| **Compression** | Zstd (documents) + PQ (vectors) | Best compression ratio for general data; PQ for vector-specific compression |
| **Client protocol** | PostgreSQL wire protocol + gRPC | PostgreSQL compatibility for existing tooling; gRPC for high-perf SDKs |
| **Embedding runtime** | ONNX Runtime (embedded) | Built-in embedding computation without external service calls |
| **Intent parser** | Rule engine + ONNX classifier + small LLM | Three-tier: rules (<1ms), ML (<5ms), LLM fallback (<100ms) |
| **Causal inference** | Custom Rust implementation | Granger tests, temporal precedence, SCM solver |
| **Learning-to-rank** | LambdaMART (Rust port) | Per-agent ranking models, ~1MB each, online incremental updates |
| **Build system** | Cargo + Bazel (for multi-lang SDKs) | Cargo for Rust; Bazel for cross-language SDK builds |

---

## 19. Performance Targets (Design Goals)

### 19.1 Core Engine

| Metric | Target | Conditions |
|--------|--------|------------|
| Vector search (ANN) | < 5ms p99 | 10M vectors, 768 dimensions, top-10, single node |
| Vector search (ANN, distributed) | < 15ms p99 | 100M vectors, 768d, top-10, 3 shards |
| Point read | < 2ms p99 | Single document by _id |
| Document write | < 5ms p99 | Single document with 1 embedding + confidence, Raft quorum |
| Graph traversal | < 10ms p99 | 3-hop BFS, 1M edges |
| Hybrid query (vector + filter + graph) | < 20ms p99 | 10M docs, selective filter |
| Context-optimized retrieval | < 25ms p99 | 10M docs, 4000 token budget, MMR selection |
| Write throughput | > 100K ops/sec | Per node, EVENTUAL consistency, batched |
| Agent memory recall | < 10ms p99 | 100K memories per agent, top-20 recall |
| Confidence propagation | < 2ms overhead | Per query, cached propagation rules |

### 19.2 Semantic Interface

| Metric | Target | Conditions |
|--------|--------|------------|
| Semantic intent parse (Tier 1) | < 1ms p99 | Rule-based pattern matching |
| Semantic intent parse (Tier 2) | < 5ms p99 | ML classifier |
| Semantic intent parse (Tier 3) | < 100ms p99 | LLM fallback |

### 19.3 Causal Reasoning Engine

These targets are derived from the hackathon's physical execution analysis (§14.8). All 5 signals run fully in parallel; the critical path is S4 (NaturalExperimentDetector).

| Operation | Target | Signal breakdown | Notes |
|-----------|--------|-----------------|-------|
| `INFER CAUSALITY` (fast path) | < 35ms p99 | S1 (18ms) + S5 (8ms) + fusion (2ms) | S2/S3/S4 skipped; uses only temporal + consensus signals |
| `ASSESS CAUSALITY` (standard) | < 150ms p99 | S1+S2+S3+S5 parallel + fusion | S4 (NaturalExperiment) skipped unless dataset qualifies |
| `ASSESS CAUSALITY` (full, worst case) | < 303ms p99 | All 5 signals parallel, S4=285ms critical path | Triggered when RDD/Event Study assumptions pass |
| `ASSESS CAUSALITY` (hard cancel) | 380ms timeout | S4 cancelled via CancellationToken | Prevents tail latency from unbounded statistical scans |
| `EXPLAIN CAUSALITY` | < 363ms p99 | Full ASSESS + causal trace (50ms) + rationale formatting | Includes signal-by-signal narrative |
| `INFER CAUSALITY` (pre-computed, hot) | < 1ms | Confidence score cached from prior ASSESS | LRU TTL-based invalidation |
| MechanismDetect (S2) — warm | < 45ms | Tier 1 HNSW (5ms) + Tier 2 HNSW midpoint (40ms) | Tier 3 NLI skipped unless Tier 2 score < 0.3 |
| MechanismDetect (S2) — cold (NLI) | < 180ms | + Tier 3 NLI cross-encoder reranking | Capped at 5 candidate mechanisms |
| ConfounderSearch (S3) — standard | < 120ms | 4 strategies in parallel: embedding triangle, common ancestor, text co-occurrence, temporal context | |
| NaturalExperimentDetector (S4) — RDD | < 180ms | RDD with McCrary test + LATE computation | Requires ≥ 500 records near threshold |
| NaturalExperimentDetector (S4) — DiD | < 285ms | DiD with parallel trends test + bootstrap CI | Requires ≥ 2 periods, ≥ 50 units per group |
| Confidence ceiling computation | < 1ms | `C_max = 0.90 × f_method × f_independence` | Pre-computed during signal collection |
| Murphy D-S fusion | < 2ms | Full 5-signal combination | Algebraic closed-form, no iteration |
| Red Team checklist | < 1ms | 15 checks on pre-aggregated stats | Pure in-memory computation |

**Causal cache hit rates (design targets):**

| Cache tier | Expected hit rate | Invalidation |
|------------|------------------|--------------|
| Hot (pre-computed score) | 60–70% | On new conflicting evidence or record update |
| Warm (partial signals cached) | 20–25% | 30-minute LRU TTL |
| Cold (full on-demand) | 10–15% | No cache — always fresh |

---

## 20. Roadmap

### 20.1 Phase Summary

| Phase | Timeline | Theme |
|-------|----------|-------|
| Phase 1 — Foundation | Months 1-6 | Storage, transactions, basic cognitive metadata |
| Phase 2 — Distribution | Months 7-12 | Cluster, consensus, causal graph at scale |
| Phase 3 — Intelligence | Months 13-18 | Full 5-signal causal inference, semantic interface, adaptive learning |
| Phase 4 — Enterprise | Months 19-24 | Production hardening, feedback loop partial solution, commercial launch |

### 20.2 Phase 1 — Foundation (Months 1–6)

**Engine:**
- Storage engine: LSM-Tree (write path) + columnar block format (read path)
- Bitemporal MVCC (`valid_time` + `system_time` on every record)
- Single-node FunQL parser and execution engine
- B+Tree (point reads) + HNSW (vector ANN) indexes
- Basic security: TLS, RBAC, audit log

**Cognitive metadata — Tier 1:**
- `FunRecord` extended with `_confidence`, `_sources`, `_provenance`, `_caused_by[]`, `_effects[]`
- `ASSERT CONFIDENCE` / `SET PROVENANCE` FunQL operators
- Explicit causal edges: `LINK CAUSE→EFFECT WITH confidence=X` stored as typed graph edges
- Causal DAG construction from explicit edges; `TRACE CAUSALITY FROM X` path query

### 20.3 Phase 2 — Distribution (Months 7–12)

**Cluster:**
- Raft consensus, range-based sharding, distributed FunQL execution (DistSQL model)
- Cross-shard join and aggregation pushdown
- Multi-tenant isolation (namespace-level keyspace separation)

**Cognitive metadata — Tier 2:**
- Confidence propagation engine: derived facts inherit confidence from source facts
- Contradiction detection: `FIND CONTRADICTIONS` surfaces conflicting `_confidence` claims
- Causal DAG index: shard-local adjacency + global coordinator index, causal path queries < 15ms p99
- Dedicated HNSW index capacity reserved for causal operators (prevents eviction under load)

### 20.4 Phase 3 — Intelligence (Months 13–18)

**AI subsystems:**
- Built-in embedding runtime (ONNX, 768d default, pluggable models)
- Agent memory subsystem (`REMEMBER`, `RECALL`, `FORGET` operators; LTR ranking per agent)
- Semantic Interface Tier 1 (rule-based) + Tier 2 (ML classifier) — deterministic FunQL output
- Context-Aware Retrieval (`WITHIN CONTEXT` operator, MMR + token budget optimization)
- Bidirectional learning: LambdaMART model per agent, feedback from implicit signals

**Causal Reasoning — 5-signal engine (hackathon v0.3 spec):**
- `ASSESS CAUSALITY` operator: all 5 signals in parallel, 303ms worst-case budget
- S1 — TemporalPrecedenceSignal (18ms): Granger causality + cross-correlation + ECM
- S2 — SemanticMechanismSignal (45ms warm / 180ms cold): MechanismDetect 3-tier algorithm (HNSW direct edges → HNSW midpoint bridge → NLI cross-encoder)
- S3 — ConfounderExclusionSignal (120ms): ConfounderSearch 4-strategy parallel scan (embedding triangle, common ancestor, text co-occurrence, temporal context)
- S4 — NaturalExperimentSignal (285ms worst case): NaturalExperimentDetector cascade (RDD → Event Study → DiD → Synthetic Control → ITS), per-method assumption validation
- S5 — SourceConsensusSignal (8ms): pre-computed from `_supports` / `_contradicts` fields
- Murphy's modified Dempster-Shafer fusion: combines 5 mass functions with explicit ignorance mass
- Computed confidence ceiling: `C_max = 0.90 × f_method × f_independence` (replaces hardcoded 0.85)
- Red Team checklist: 15 adversarial checks pre-flight on any `ASSESS CAUSALITY` call
- `EXPLAIN CAUSALITY` operator: signal-by-signal rationale + uncertainty narrative
- 3-tier computation cache: hot (pre-computed score, <1ms), warm (partial signals, 1–50ms), cold (full on-demand, 50–303ms)
- `INFER CAUSALITY` fast path: S1 + S5 only, < 35ms, for high-frequency real-time use

### 20.5 Phase 4 — Enterprise (Months 19–24)

**Production hardening:**
- Cross-region replication with conflict resolution
- Backup/restore, point-in-time recovery
- Monitoring dashboard (causal query latency histograms, signal quality breakdown, cache hit rates)
- SOC2 Type II compliance

**Cognitive metadata — Tier 3:**
- Semantic Interface Tier 3 (LLM fallback): natural language → deterministic FunQL with intent verification
- Causal visualization: interactive DAG explorer with confidence heatmap and signal breakdown
- Causal export: `EXPORT CAUSAL GRAPH AS DOT | JSON | RDF`

**Feedback loop (partial solution — see §14.11):**
- `_causal_provenance` tag on every record inferred from causal reasoning
- Temporal quarantine: inferred-causal records excluded from S4 natural experiment scans
- Confidence decay on self-referential causal chains (decay_rate = 0.15/cycle)
- *Full feedback loop isolation requires v2 architecture; acknowledged as open problem*

**Commercial launch:** GA release with SLAs, enterprise support, SDK for Python, TypeScript, Go.

---

## 21. Conclusion

FunDB is not an incremental improvement on existing databases. It is a **purpose-built cognitive data engine for the AI era** — designed for workloads that didn't exist five years ago and will dominate the next decade.

The database industry's current approach of bolting AI capabilities onto 30-year-old architectures (PostgreSQL + pgvector, MongoDB + Atlas Vector Search) creates fragmented developer experiences, consistency gaps, and performance ceilings. But even the new "AI-native" databases (Pinecone, Weaviate, Qdrant) only solve one dimension — vector search — without addressing the deeper question: **what does a database look like when the primary consumer is not a human, but an AI?**

FunDB answers that question with five cognitive pillars:

1. **Semantic Intent** — AI expresses what it needs, not how to query it
2. **Confidence & Provenance** — Every fact carries its trust level and lineage
3. **Context Awareness** — Output is optimized for the consumer's constraints
4. **Adaptive Learning** — The database improves with every interaction
5. **Causal Reasoning** — Data tracks not just what and when, but why

These pillars, combined with a unified engine for vectors, documents, graphs, time-series, and agent memory, make FunDB the first database designed to be a **cognitive partner** for AI — not just a store, but a collaborator.

**The right time to build this is now.** The explosion of LLMs, autonomous agents, and RAG systems has created a $50B+ infrastructure market that no existing database fully serves. FunDB is designed to be the default knowledge infrastructure for every AI application.

---

*"The best database for AI is one that thinks with AI."*

**— FunDB Team**
