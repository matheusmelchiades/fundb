<p align="center">
  <img src="assets/brand/logo-full.svg" alt="FunDB" width="320">
  <br>
  <em>The cognitive database for the AI era</em>
</p>

<p align="center">
  <a href="https://github.com/fundb/fundb/actions/workflows/ci.yml"><img src="https://github.com/fundb/fundb/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/fundb/fundb/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-1A1A2E?style=flat-square&labelColor=E8872B&logoColor=white" alt="License"></a>
  <a href="https://github.com/fundb/fundb/releases"><img src="https://img.shields.io/badge/v0.1.0-cognitive%20database-1A1A2E?style=flat-square&labelColor=E8872B&logoColor=white" alt="Version"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.85+-1A1A2E?style=flat-square&labelColor=C46A15&logo=rust&logoColor=white" alt="Rust"></a>
  <img src="https://img.shields.io/badge/PostgreSQL-wire%20compatible-1A1A2E?style=flat-square&labelColor=3D5A80&logo=postgresql&logoColor=white" alt="PostgreSQL Compatible">
  <img src="https://img.shields.io/badge/tests-407%20passing-1A1A2E?style=flat-square&labelColor=0D9488" alt="Tests">
</p>

---

FunDB is an AI-native database that unifies **vectors, graphs, documents, time-series, and agent memory** under a single cognitive engine. Instead of integrating 5 different databases, connect your AI agents to one engine that understands semantics, tracks confidence, optimizes for context windows, and reasons about causality.

It speaks **PostgreSQL wire protocol** — your existing tools already work.

```
 5 databases → 1 FunDB
 ┌──────────┐
 │ Pinecone │──→ vectors
 │ Neo4j    │──→ graphs        ┌─────────────────┐
 │ MongoDB  │──→ documents  ══>│     FunDB        │
 │ InfluxDB │──→ time-series   │  one engine,     │
 │ Redis    │──→ agent memory  │  one query,      │
 └──────────┘                  │  one transaction  │
                               └─────────────────┘
```

## Why FunDB?

Every piece of data in FunDB carries **cognitive metadata** that no other database tracks natively:

| Feature | What it means | Why it matters |
|---------|---------------|----------------|
| **Confidence & Provenance** | Every fact has a trust score and a source chain | AI decisions are only as good as the data confidence |
| **Causal Reasoning** | Track not just *what* and *when*, but *why* | Answer "what happens if we change X?" directly in the database |
| **Context-Aware Retrieval** | Output optimized for LLM token budgets | No more wasting 50% of your context window on irrelevant results |
| **Semantic Intent** | Query with `UNDERSTAND "find recent papers about transformers"` | AI agents describe *what* they need, not *how* to get it |
| **Agent Memory** | `REMEMBER` / `RECALL BY` with semantic decay | Persistent, searchable memory for autonomous agents |
| **Bitemporal MVCC** | System time + valid time on every record | "What did the model believe at time T?" — reproducibility built in |

## Quickstart

### Option 1: Docker (recommended)

```bash
# Start FunDB
docker compose up -d

# Connect with psql
psql -h localhost -p 5433 -U fundb

# Or use the built-in CLI
docker exec -it fundb fundb
```

### Option 2: Build from source

```bash
# Requires Rust 1.85+
git clone https://github.com/fundb/fundb.git
cd fundb
cargo build --release

# Start the server
./target/release/fundb-server

# Connect with the CLI (in another terminal)
./target/release/fundb
```

### Option 3: Connect via psql

FunDB speaks PostgreSQL wire protocol. Connect with any PostgreSQL client:

```bash
psql -h localhost -p 5433 -U fundb -d fundb
```

### Your first queries

```sql
-- Create a collection and insert a document with an embedding
INSERT INTO research_papers (title, abstract, _vectors, _confidence)
VALUES (
  'Attention Is All You Need',
  'We propose a new simple network architecture, the Transformer...',
  '{"content_embedding": [0.1, 0.2, 0.3, ...]}',
  0.95
);

-- Vector similarity search with confidence filtering
SELECT title, _confidence, _sources
FROM research_papers
WHERE _vector('content_embedding') <-> [0.1, 0.2, ...] < 0.5
  AND _confidence > 0.7
ORDER BY _vector_distance ASC
LIMIT 10;

-- Context-aware retrieval (optimized for LLM token budgets)
SELECT * FROM research_papers
WHERE _vector('content_embedding') <-> :query < 0.5
WITHIN CONTEXT (
  max_tokens: 4000,
  coherence: 0.8,
  diversity: 0.3,
  include_contradictions: true
);

-- Trace causal chains
TRACE CAUSALITY
FROM deployment_event TO error_spike
MAX_DEPTH 5
MIN_STRENGTH 0.6;

-- Semantic intent (natural language → FunQL)
UNDERSTAND 'find recent papers about transformers with high confidence';

-- Agent memory
REMEMBER agent_id='agent-007'
  CONTENT 'User prefers concise answers'
  IMPORTANCE 0.8;

RECALL BY agent_id='agent-007'
  QUERY 'user preferences'
  TOP_K 5;
```

## Features

### Multi-Modal Data in One Engine

FunDB stores all data types in a single **FunRecord** — a unified knowledge unit:

```
FunRecord {
    _id:          UUID v7 (time-ordered)
    data:         { flexible document payload }
    _vectors:     { "embedding_name": [f32; N] }
    _edges:       [{ label, target, props }]
    _timeseries:  [{ timestamp, value }]
    _confidence:  0.0–1.0
    _sources:     [{ origin, method, confidence }]
    _caused_by:   [{ source_id, relation, strength }]
}
```

One record, one transaction, one consistency boundary — across all data types.

### FunQL: SQL for the AI Era

FunQL extends SQL with native primitives for AI workloads:

```sql
-- Standard SQL works as expected
SELECT * FROM users WHERE age > 25;

-- Vector search (ANN via HNSW)
SELECT * FROM docs
WHERE _vector('embedding') <-> :query_vec < 0.3
ORDER BY _vector_distance ASC LIMIT 10;

-- Graph traversal
SELECT * FROM entities
TRAVERSE follows(depth: 1..3)
WHERE start.type = 'user' AND end.type = 'topic'
RETURN path;

-- Bitemporal time-travel
SELECT * FROM events
AS OF SYSTEM TIME '2025-06-15'
AS OF VALID TIME BETWEEN '2025-01-01' AND '2025-06-01';

-- Confidence-aware filtering
SELECT * FROM facts
WHERE _confidence > 0.8
  AND _source_count >= 2
  AND _contradiction_count = 0;
```

### Causal Reasoning (Unique to FunDB)

FunDB is the only database with native causal inference — from explicit edges to structural causal models:

```sql
-- Tier 1: Declare causal relationships
INSERT INTO _causal_edges (source_id, target_id, relation, strength, mechanism)
VALUES (:deploy_id, :error_id, 'CAUSED', 0.85, 'new model version introduced regression');

-- Tier 2: Discover causal structure from data
DISCOVER CAUSAL STRUCTURE IN sales_data
ALGORITHM 'ensemble'
MAX_DEPTH 4;

-- Tier 3: Interventional queries (do-calculus)
ESTIMATE EFFECT OF SET(marketing_spend = 50000)
ON revenue
USING MODEL quarterly_model;

-- Tier 3: Counterfactual reasoning
ESTIMATE COUNTERFACTUAL
  GIVEN revenue = 100000, marketing_spend = 30000
  HAD marketing_spend = 50000
  PREDICT revenue;
```

### Context-Aware Retrieval

Results are optimized for LLM context windows with MMR (Maximal Marginal Relevance):

```sql
SELECT * FROM knowledge_base
WHERE _vector('embedding') <-> :query < 0.5
WITHIN CONTEXT (
  max_tokens: 4000,      -- token budget for your LLM
  coherence: 0.8,        -- minimum pairwise similarity
  diversity: 0.3,        -- diversity penalty (0=most relevant, 1=most diverse)
  include_contradictions: true
);

-- Response includes metadata:
-- tokens_used, candidates_evaluated, candidates_selected,
-- avg_confidence, coherence_score, contradictions_found
```

### Confidence Propagation

Confidence flows through query operations with algebraic rules:

| Operation | Rule |
|-----------|------|
| `JOIN(A, B)` | `min(A._confidence, B._confidence)` |
| `UNION(A, B)` | `max(A._confidence, B._confidence)` |
| `AGGREGATE(group)` | `weighted_avg(group._confidence)` |
| `GRAPH_TRAVERSE(path)` | `product(node_confidences * edge_strengths)` |
| `CAUSAL_CHAIN` | `product(all_strengths_in_chain)` |

New evidence automatically updates existing confidence: corroboration increases it, contradiction decreases it.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     SEMANTIC INTERFACE LAYER                        │
│  Intent Parser (NLU)  │  Context Optimizer (MMR)  │  Feedback (LTR) │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│                          CLIENT LAYER                               │
│  SDK (Python/Go/JS)  │  REST API  │  PostgreSQL Wire Protocol       │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│                    QUERY ENGINE (FunQL)                              │
│  Parser → Binder → Optimizer (rule + cost) → Executor (Volcano)     │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  UNIFIED CATALOG: Schema │ Confidence Engine │ Causal Graph │    │
│  └─────────────────────────────────────────────────────────────┘    │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│                       INDEX MANAGER                                 │
│  B+Tree (scalar)  │  HNSW (vector)  │  SPO (graph)  │  Temporal    │
│  Causal DAG       │  GIN/Inverted   │  Confidence    │              │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│                 STORAGE ENGINE (FunStore)                            │
│  LSM-Tree: MemTable (SkipList) → WAL → SSTable → Column Pages      │
│  MVCC: Bitemporal versioning (system_time + valid_time)             │
│  Compression: Zstd + Snappy + Product Quantization (PQ64)           │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────────────┐
│                    DISTRIBUTED FABRIC                                │
│  Raft Consensus  │  Consistent Hashing  │  Gossip  │  Shard Manager │
└─────────────────────────────────────────────────────────────────────┘
```

**16 crates**, single workspace, 407 tests passing.

## Crate Structure

| Crate | Purpose |
|-------|---------|
| `fundb-core` | Core types, FunRecord, codecs |
| `fundb-storage` | LSM-tree storage engine |
| `fundb-indexes` | B+Tree, HNSW, SPO graph, causal DAG, temporal |
| `fundb-sql` | FunQL parser, lexer, binder |
| `fundb-optimizer` | Rule-based + cost-based query optimizer |
| `fundb-executor` | Volcano-style query executor |
| `fundb-cognitive` | Semantic intent, context optimization |
| `fundb-causal` | SCM, interventions, counterfactuals |
| `fundb-semantic` | Semantic analysis, intent parsing |
| `fundb-learning` | Learning-to-rank, feedback loop |
| `fundb-raft` | Raft consensus protocol |
| `fundb-cluster` | Distributed coordination, shard management |
| `fundb-protocol` | PostgreSQL wire protocol |
| `fundb-server` | HTTP REST API + PG wire server |
| `fundb-cli` | Interactive REPL + single-command execution |

## Docker

```bash
# Single node (development)
docker compose up -d

# 3-node cluster
docker compose --profile cluster up -d

# Check health
curl http://localhost:8080/health
# {"status":"ok","version":"0.1.0"}
```

**Ports:**
| Port | Protocol |
|------|----------|
| 5433 | PostgreSQL wire protocol |
| 8080 | HTTP REST API |

## REST API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check |
| `POST` | `/query` | Execute FunQL query |
| `POST` | `/understand` | Semantic intent parsing |
| `POST` | `/causal/trace` | Trace causal paths |

## CLI

```bash
# Interactive REPL
fundb -h localhost -p 5433

# Single command
fundb -c "SELECT * FROM users LIMIT 5"

# JSON output
fundb -f json -c "SELECT * FROM events"

# CSV output
fundb -f csv -c "SELECT * FROM metrics"
```

**Meta-commands:**

| Command | Description |
|---------|-------------|
| `\help` | Show help |
| `\status` | Connection info |
| `\format <fmt>` | Switch output (table/json/csv) |
| `\understand <text>` | Semantic intent query |
| `\causal <from> <to>` | Trace causal path |
| `\memory list` | List agent memories |
| `\memory recall <q>` | Recall memories |
| `\quit` | Exit |

## Benchmarks

> Benchmarks are measured on a single node with default configuration.
> Formal benchmark suite with reproducible results is coming soon.

| Operation | Target | Conditions |
|-----------|--------|------------|
| Vector search (p99) | < 5ms | 10M vectors, 768d, top-10 |
| Point read (p99) | < 2ms | Single document by _id |
| Document write (p99) | < 5ms | Single doc + embedding + confidence |
| Graph traversal (p99) | < 10ms | 3-hop BFS, 1M edges |
| Causal path query (p99) | < 15ms | 5-hop, 1M causal edges |
| Context retrieval (p99) | < 25ms | 10M docs, 4000 token budget |
| Write throughput | > 100K ops/s | Per node, batched |
| Semantic intent Tier 1 | < 1ms | Rule-based parsing |

## Security

- Multi-tenant data isolation with per-query tenant filtering
- RBAC with collection-level READ / WRITE / ADMIN permissions
- SCRAM-SHA-256 and MD5 authentication
- Audit logging
- TLS 1.3 support for all connections

## Observability

- Prometheus metrics endpoint
- OpenTelemetry tracing
- Structured logging via `tracing` crate

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and PR process.

## License

FunDB is licensed under [Apache License 2.0](LICENSE).

## Status

FunDB is in **v0.1.0** (initial release). The core engine is feature-complete with 407+ tests passing across 16 crates. `INSERT` and `SELECT` queries are fully functional end-to-end via the PostgreSQL wire protocol. We are actively working on developer experience, documentation, and ecosystem integrations.

See the [CHANGELOG](CHANGELOG.md) for detailed release notes.
