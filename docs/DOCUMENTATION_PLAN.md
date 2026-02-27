# FunDB — Documentation Master Plan

**Version:** 1.0
**Status:** Active
**Goal:** Cover every piece of knowledge needed to understand, build, extend, and operate FunDB — from theory to production.

---

## Overview

This plan defines the complete documentation corpus for FunDB. Each document has an owner role (which agent/team writes it), a priority (P0 = blocking, P1 = important, P2 = nice-to-have), and clear dependencies.

The documentation is organized in **7 pillars**, each with its own directory under `docs/`.

```
docs/
├── DOCUMENTATION_PLAN.md        ← this file
├── IMPLEMENTATION_PLAN.md       ← parallel build roadmap
│
├── architecture/                ← deep-dive technical specs (already started)
├── theory/                      ← academic foundations & research
├── specifications/              ← formal module specs (source of truth for implementors)
│   ├── storage/
│   ├── query-engine/
│   ├── indexes/
│   ├── cognitive/
│   ├── distribution/
│   └── protocol/
├── api/                         ← public-facing language & API reference
├── development/                 ← contributing, testing, local setup
├── operations/                  ← deployment, monitoring, tuning
└── sdk/                         ← client library documentation
```

---

## Pillar 1 — Architecture (deep technical specs)

> **Purpose:** "I want to understand exactly how FunDB works at a subsystem level."
> **Audience:** Senior engineers, architects, contributors

| Document | Path | Priority | Owner Role | Status | Dependencies |
|----------|------|----------|------------|--------|--------------|
| Architecture Overview v0.3 | `architecture/architecture.v0.3.md` | P0 | Architect | Done | — |
| Storage Engine Deep Dive | `architecture/storage-engine.md` | P0 | Storage Agent | TODO | — |
| Query Engine Deep Dive | `architecture/query-engine.md` | P0 | Query Agent | TODO | Storage |
| Index System Deep Dive | `architecture/index-system.md` | P0 | Index Agent | TODO | Storage |
| Cognitive Layer Deep Dive | `architecture/cognitive-layer.md` | P0 | Cognitive Agent | TODO | Query Engine |
| Semantic Interface Deep Dive | `architecture/semantic-interface.md` | P0 | SI Agent | TODO | Cognitive |
| Causal Reasoning Deep Dive | `architecture/causal-reasoning.md` | P0 | Causal Agent | TODO | Cognitive |
| Distribution System Deep Dive | `architecture/distribution.md` | P1 | Dist Agent | TODO | Storage + Query |
| Security Architecture | `architecture/security.md` | P1 | Security Agent | TODO | Distribution |
| Performance Model | `architecture/performance-model.md` | P1 | Perf Agent | TODO | All modules |

---

## Pillar 2 — Theory (academic foundations)

> **Purpose:** "I want to understand the science behind the cognitive features."
> **Audience:** Researchers, technically curious engineers, investors due diligence

| Document | Path | Priority | Key Topics |
|----------|------|----------|------------|
| Vector Search & HNSW Theory | `theory/vector-search.md` | P0 | ANN, HNSW, PQ compression, recall/precision trade-offs |
| Graph Theory for FunDB | `theory/graph-theory.md` | P1 | SPO triples, adjacency lists, BFS/DFS, path algorithms |
| Bitemporal Data Theory | `theory/bitemporal.md` | P0 | Snodgrass bitemporal model, SQL/Temporal, MVCC |
| Probabilistic Databases & Confidence | `theory/probabilistic-databases.md` | P0 | BayesDB, Trio, MayBMS, confidence propagation algebra |
| Information Retrieval: MMR & Context | `theory/information-retrieval.md` | P1 | TF-IDF, BM25, MMR algorithm, token budget optimization |
| Learning to Rank | `theory/learning-to-rank.md` | P1 | LambdaMART, pairwise/listwise ranking, online LTR |
| **Causal Inference: Complete Guide** | `theory/causal-inference.md` | **P0** | Full document below |
| Structural Causal Models (SCMs) | `theory/scm.md` | P0 | DAGs, do-calculus, interventions, counterfactuals |
| Causal Discovery Algorithms | `theory/causal-discovery.md` | P0 | PC, FCI, NOTEARS, GES, LiNGAM |
| Granger Causality | `theory/granger-causality.md` | P0 | Definition, F-test, VAR models, limitations |
| LLM-Assisted Causal Reasoning | `theory/llm-causal-oracle.md` | P0 | Novel approach: LLM hypothesis + statistical validation |

### 2.1 Causal Inference Complete Guide — Detailed Outline

This is the most critical theory document. It enables the team to implement causal reasoning correctly.

```
theory/causal-inference.md
│
├── 1. Why Causality in Databases?
│   ├── The limits of correlation (spurious correlations)
│   ├── The question databases can't answer today: "why?"
│   └── Real examples: A/B tests, incident analysis, policy decisions
│
├── 2. Pearl's Ladder of Causation
│   ├── Level 1 — Association (seeing): P(Y|X)
│   │   └── What all current databases do
│   ├── Level 2 — Intervention (doing): P(Y|do(X=x))
│   │   └── What FunDB targets: "what happens if we change X?"
│   └── Level 3 — Counterfactuals (imagining): P(Y_x | X=x', Y=y')
│       └── "What would have happened if X had been different?"
│
├── 3. Structural Causal Models (SCMs)
│   ├── Definition: V (variables), E (edges), F (functions), U (noise)
│   ├── Directed Acyclic Graphs (DAGs)
│   ├── Markov condition and d-separation
│   ├── Identifiability: when can we compute causal effects from observational data?
│   └── The do-operator and graph surgery
│
├── 4. Causal Discovery: Learning Structure from Data
│   ├── Constraint-based methods
│   │   ├── PC Algorithm (Peter-Clark): steps, complexity, assumptions
│   │   ├── FCI (Fast Causal Inference): handles hidden confounders
│   │   └── When to use each
│   ├── Score-based methods
│   │   ├── GES (Greedy Equivalence Search)
│   │   └── NOTEARS: continuous optimization formulation (2018)
│   ├── Functional causal models
│   │   └── LiNGAM: exploits non-Gaussianity for full identification
│   └── Hybrid: FunDB's LLM-hypothesis approach
│       ├── Why LLMs have implicit causal knowledge
│       ├── Hypothesis generation protocol
│       ├── Statistical validation layer
│       └── Edge persistence and confidence scoring
│
├── 5. Granger Causality
│   ├── Formal definition: X Granger-causes Y if past X improves prediction of Y
│   ├── Vector Autoregression (VAR) model
│   ├── F-test for Granger causality
│   ├── Multivariate Granger (MVAR)
│   ├── Limitations: only captures statistical, not structural causality
│   └── When to use in FunDB (time-series collections)
│
├── 6. Interventional Inference (do-calculus)
│   ├── The three rules of do-calculus
│   ├── Back-door criterion
│   ├── Front-door criterion
│   ├── Implementation: graph surgery algorithm
│   └── Example: marketing_spend → revenue model
│
├── 7. Counterfactual Reasoning
│   ├── Twin network method (Pearl)
│   ├── Abduction → Action → Prediction three steps
│   ├── Structural equation solving
│   └── Confidence intervals via bootstrap resampling
│
├── 8. FunDB's Three-Tier Causal Implementation
│   ├── Tier 1: Explicit edges (declarative, user-defined)
│   ├── Tier 2: Inferred causality (Granger + temporal + LLM-hybrid)
│   └── Tier 3: Interventional (SCM + do-calculus, experimental)
│
├── 9. Known Limitations & Open Problems
│   ├── Causal discovery NP-hard in general (exponential in # variables)
│   ├── Faithfulness assumption violations
│   ├── Hidden confounders break identifiability
│   ├── Non-stationarity in time-series Granger tests
│   └── LLM hallucinations in hypothesis generation (mitigation strategies)
│
└── 10. Reference Papers
    ├── Pearl, J. (2009) Causality: Models, Reasoning and Inference
    ├── Spirtes et al. (1993) Causation, Prediction and Search
    ├── Zheng et al. (2018) DAGs with NO TEARS
    ├── Granger (1969) Investigating Causal Relations by Econometric Models
    └── Peters et al. (2017) Elements of Causal Inference
```

---

## Pillar 3 — Specifications (formal module specs)

> **Purpose:** "I need to implement this module. What exactly should it do?"
> **Audience:** Individual contributors, agent implementors
> These are the **source of truth** — precise enough that two implementations from the same spec produce compatible results.

### Storage Specifications

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| FunRecord Format Spec | `specifications/storage/funrecord.md` | **P0** | Exact binary layout, field encoding, versioning |
| MemTable Spec | `specifications/storage/memtable.md` | P0 | SkipList invariants, concurrent ops, flush protocol |
| WAL Spec | `specifications/storage/wal.md` | P0 | Log format, checkpointing, recovery protocol |
| SSTable Spec | `specifications/storage/sstable.md` | P0 | File format, bloom filter, block compression |
| Compaction Spec | `specifications/storage/compaction.md` | P0 | Tiered + leveled strategy, triggers, I/O scheduling |
| MVCC Bitemporal Spec | `specifications/storage/mvcc-bitemporal.md` | P0 | Version chain, garbage collection, time-travel reads |
| Columnar Page Group Spec | `specifications/storage/columnar-pages.md` | P1 | Column layout, PQ encoding, page headers, stats |
| Confidence & Provenance Column Spec | `specifications/storage/confidence-columns.md` | P0 | Storage format, histogram structure, propagation rules |

### Query Engine Specifications

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| FunQL Grammar (EBNF) | `specifications/query-engine/funql-grammar.ebnf` | **P0** | Complete formal grammar |
| FunQL AST Spec | `specifications/query-engine/ast.md` | P0 | All AST node types, their fields, visitor pattern |
| Type System Spec | `specifications/query-engine/type-system.md` | P0 | Type hierarchy, coercions, vector type rules |
| Binder Spec | `specifications/query-engine/binder.md` | P0 | Name resolution, type checking, error messages |
| Rule-Based Optimizer Spec | `specifications/query-engine/optimizer-rules.md` | P0 | All optimization rules, order, preconditions |
| Cost Model Spec | `specifications/query-engine/cost-model.md` | P1 | Cost formulas per operator, statistics used |
| Physical Operators Spec | `specifications/query-engine/operators.md` | P0 | Interface, contract, each operator's algorithm |
| WITHIN CONTEXT Execution Spec | `specifications/query-engine/context-retrieval.md` | P0 | MMR algorithm, token estimation, coherence scoring |

### Index Specifications

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| B+Tree Spec | `specifications/indexes/btree.md` | P0 | Node format, split/merge, range scan, prefix compression |
| HNSW Spec | `specifications/indexes/hnsw.md` | P0 | Layer construction, ef_construction, search, PQ integration |
| Graph Index Spec (SPO) | `specifications/indexes/graph-spo.md` | P0 | Triple format, adjacency lists, BFS/DFS, reverse index |
| Temporal Index Spec | `specifications/indexes/temporal.md` | P0 | Interval tree, bitemporal range queries |
| Causal DAG Index Spec | `specifications/indexes/causal-dag.md` | **P0** | DAG structure, transitive closure, insert/query ops |
| Inverted Text Index Spec | `specifications/indexes/inverted.md` | P1 | Posting lists, BM25, delta encoding |
| Confidence Index Spec | `specifications/indexes/confidence.md` | P0 | Histogram, B+Tree for point/range confidence queries |

### Cognitive Module Specifications

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| Confidence Propagation Engine Spec | `specifications/cognitive/confidence-propagation.md` | **P0** | Algebra rules per operator, lazy evaluation, caching |
| Contradiction Detection Spec | `specifications/cognitive/contradiction-detection.md` | P0 | Semantic polarity check, auto-linking protocol, events |
| Context Retrieval (MMR) Spec | `specifications/cognitive/mmr-context.md` | P0 | Full MMR algorithm, token budget, coherence threshold |
| Semantic Interface Spec (Tier 1-2) | `specifications/cognitive/semantic-interface-t1t2.md` | P0 | NER rules, intent classifier interface, FunQL generator |
| Semantic Interface Spec (Tier 3) | `specifications/cognitive/semantic-interface-t3.md` | P1 | ONNX model spec, prompt format, fallback protocol |
| Learning-to-Rank Spec | `specifications/cognitive/ltr.md` | P1 | LambdaMART, feature vector, online update protocol |
| Agent Memory Spec | `specifications/cognitive/agent-memory.md` | P0 | Memory schema, RECALL BY execution, consolidation, decay |
| **Causal Engine Spec (Tier 1)** | `specifications/cognitive/causal-tier1.md` | **P0** | Explicit edge API, DAG validation, TRACE CAUSALITY query |
| **Causal Engine Spec (Tier 2)** | `specifications/cognitive/causal-tier2.md` | **P0** | Granger test impl, temporal precedence, LLM-oracle protocol |
| **Causal Engine Spec (Tier 3)** | `specifications/cognitive/causal-tier3.md` | **P0** | SCM DDL, graph surgery, do-calculus evaluation, twin networks |

### Distribution Specifications

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| Raft Consensus Spec | `specifications/distribution/raft.md` | P0 | Leader election, log replication, membership changes |
| Shard Manager Spec | `specifications/distribution/sharding.md` | P0 | Consistent hashing, shard map, rebalancing |
| Distributed Query Spec | `specifications/distribution/distributed-query.md` | P0 | Scatter/gather, partial aggregation, distributed ANN merge |
| Gossip Protocol Spec | `specifications/distribution/gossip.md` | P1 | Node discovery, health propagation, anti-entropy |
| Multi-Tenant Isolation Spec | `specifications/distribution/multi-tenant.md` | P0 | Tenant boundaries, data plane isolation, quota enforcement |

### Protocol Specifications

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| PostgreSQL Wire Protocol Impl Spec | `specifications/protocol/pg-wire.md` | P0 | Message types, extended query protocol, type mapping |
| gRPC API Spec | `specifications/protocol/grpc.md` | P0 | Protobuf definitions, service contracts, streaming |
| REST API Spec | `specifications/protocol/rest.md` | P1 | Endpoints, auth, request/response schema |
| SDK Protocol Spec | `specifications/protocol/sdk-protocol.md` | P0 | Connection pooling, retry logic, telemetry reporting |

---

## Pillar 4 — API Reference

> **Purpose:** "I want to use FunDB. What's the complete language/API?"
> **Audience:** Application developers, AI/ML engineers

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| FunQL Language Reference | `api/funql-reference.md` | **P0** | Every keyword, operator, function — with examples |
| UNDERSTAND Syntax Reference | `api/understand-syntax.md` | P0 | Semantic Interface queries, WITH/WITHIN/DEPTH clauses |
| WITHIN CONTEXT Reference | `api/context-retrieval.md` | P0 | All parameters, metadata response, examples |
| Causal Query Reference | `api/causal-queries.md` | **P0** | TRACE, DISCOVER, ESTIMATE EFFECT, ESTIMATE COUNTERFACTUAL |
| Agent Memory Reference | `api/agent-memory.md` | P0 | RECALL BY syntax, memory management, consolidation |
| FunRecord Field Reference | `api/funrecord-fields.md` | P0 | All _-prefixed fields, types, semantics |
| REST API Reference | `api/rest-api.md` | P1 | OpenAPI 3.0 spec |
| Error Reference | `api/errors.md` | P1 | All error codes, causes, remediation |

---

## Pillar 5 — Development

> **Purpose:** "I want to contribute to FunDB. How?"
> **Audience:** Contributors, agent implementors, CI/CD systems

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| Contributing Guide | `development/contributing.md` | P0 | Setup, PR process, coding standards |
| Local Development Setup | `development/local-setup.md` | P0 | Rust toolchain, dependencies, build, first run |
| Testing Strategy | `development/testing.md` | **P0** | Unit, integration, property-based, chaos engineering |
| Benchmark Suite | `development/benchmarks.md` | P0 | How to run benchmarks, baseline results, regression detection |
| Code Architecture Guide | `development/code-architecture.md` | P0 | Module layout, crate structure, interfaces |
| Storage Engine Developer Guide | `development/storage-dev.md` | P1 | How to add a new column type, compaction strategy |
| Index Developer Guide | `development/index-dev.md` | P1 | How to implement a new index type |
| Cognitive Module Developer Guide | `development/cognitive-dev.md` | P1 | How to add a new cognitive feature |
| Causal Engine Developer Guide | `development/causal-dev.md` | **P0** | How to implement causal discovery algorithms |

---

## Pillar 6 — Operations

> **Purpose:** "I'm running FunDB in production. How do I operate it?"
> **Audience:** DevOps, SREs, platform engineers

| Document | Path | Priority | Content |
|----------|------|----------|---------|
| Deployment Guide | `operations/deployment.md` | P0 | Docker, Kubernetes, bare metal |
| Configuration Reference | `operations/configuration.md` | P0 | All config options, defaults, tuning advice |
| Monitoring & Metrics | `operations/monitoring.md` | P0 | Prometheus metrics, Grafana dashboards, key alerts |
| Backup & Recovery | `operations/backup-recovery.md` | P1 | Snapshot protocol, point-in-time recovery |
| Scaling Guide | `operations/scaling.md` | P1 | When to add nodes, shard rebalancing, capacity planning |
| Security Hardening | `operations/security-hardening.md` | P1 | TLS, RBAC, network isolation, audit logging |
| Troubleshooting Guide | `operations/troubleshooting.md` | P1 | Common issues, EXPLAIN output interpretation, slow queries |
| Upgrade Guide | `operations/upgrade.md` | P2 | Version migration, backward compatibility guarantees |

---

## Pillar 7 — SDK Documentation

> **Purpose:** "I'm writing Python/Go/Rust code that talks to FunDB."
> **Audience:** Application developers

| Document | Path | Priority | Languages |
|----------|------|----------|-----------|
| Python SDK Guide | `sdk/python.md` | P0 | `pip install fundb` — full tutorial |
| Go SDK Guide | `sdk/go.md` | P0 | `go get fundb` — full tutorial |
| Rust SDK Guide | `sdk/rust.md` | P0 | Native Rust client |
| JavaScript/TypeScript SDK | `sdk/javascript.md` | P1 | Node.js + browser |
| SDK Migration Guide | `sdk/migration.md` | P2 | From PostgreSQL / MongoDB / Pinecone |
| RAG Pipeline Cookbook | `sdk/rag-cookbook.md` | **P0** | End-to-end RAG with FunDB — 5 use cases |
| Agent Memory Cookbook | `sdk/agent-memory-cookbook.md` | P0 | Building persistent agent memory |
| Causal Analysis Cookbook | `sdk/causal-cookbook.md` | **P0** | Real examples: incident analysis, A/B attribution |

---

## Documentation Writing Order (Priority Queue)

Given dependencies, the recommended writing order across agents:

### Sprint 1 — Foundations (parallel, no deps)
1. `theory/causal-inference.md` — Complete causal theory guide
2. `specifications/storage/funrecord.md` — The core data format
3. `specifications/query-engine/funql-grammar.ebnf` — The formal grammar
4. `theory/probabilistic-databases.md` — Confidence theory foundation
5. `theory/bitemporal.md` — Bitemporal theory foundation

### Sprint 2 — Module Specs (parallel, depends on Sprint 1)
6. `specifications/storage/memtable.md`, `wal.md`, `sstable.md`, `mvcc-bitemporal.md`
7. `specifications/indexes/btree.md`, `hnsw.md`, `graph-spo.md`
8. `specifications/cognitive/confidence-propagation.md`
9. `specifications/cognitive/causal-tier1.md`, `causal-tier2.md`

### Sprint 3 — Query Engine & Cognitive (parallel, depends on Sprint 2)
10. `specifications/query-engine/operators.md`, `optimizer-rules.md`
11. `specifications/cognitive/mmr-context.md`, `semantic-interface-t1t2.md`
12. `specifications/cognitive/causal-tier3.md`
13. `api/funql-reference.md`, `api/causal-queries.md`

### Sprint 4 — Integration & Developer Docs
14. `development/testing.md`, `development/benchmarks.md`
15. `sdk/rag-cookbook.md`, `sdk/causal-cookbook.md`
16. `operations/deployment.md`, `operations/monitoring.md`

---

## Document Template

Every specification document must follow this template:

```markdown
# [Module Name] Specification

**Version:** x.y
**Status:** draft | review | approved
**Owner:** [agent/team name]
**Depends on:** [list of specs this depends on]
**Referenced by:** [list of specs that depend on this]

---

## 1. Purpose & Scope
What problem does this module solve? What's out of scope?

## 2. Concepts & Terminology
Define all terms used in this document.

## 3. Data Structures
Exact definitions of all data structures (Rust-like pseudocode).

## 4. Algorithms
Step-by-step algorithms with complexity analysis.

## 5. Interface
Public API / function signatures this module exposes.

## 6. Error Conditions
What can go wrong and how it's handled.

## 7. Performance Characteristics
Time/space complexity, expected throughputs, bottlenecks.

## 8. Testing Requirements
What must be tested, edge cases, property-based test requirements.

## 9. References
Papers, prior art, implementation references.
```

---

*Total documents: ~75 | P0 documents: ~35 | Estimated writing effort: 8-10 agent-sprints in parallel*
