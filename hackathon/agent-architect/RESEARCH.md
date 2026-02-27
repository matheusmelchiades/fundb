# Agent Architect -- RESEARCH.md

## How Other Databases Implement Complex Analytical Operators

### 1. DuckDB: Custom Operators and Extension Architecture

DuckDB is the most relevant reference for FunDB's causal operator design because it demonstrates how to build complex analytical operators inside an embeddable, vectorized columnar engine.

**Key architectural patterns from DuckDB:**

- **Vectorized pull-based execution.** DuckDB uses a Volcano-style iterator model but processes data in vectors of 1024 or 2048 tuples at a time rather than row-by-row. Each operator's `GetChunk()` method returns a `DataChunk` (a collection of column vectors). This is directly compatible with FunDB's 1024-row batch model.

- **Extension system for custom operators.** DuckDB exposes `TableFunction`, `ScalarFunction`, `AggregateFunction`, and `PragmaFunction` extension points. A `TableFunction` can produce rows on demand (pull model), supports projection pushdown and filter pushdown via the `FunctionData` context, and can emit progress callbacks. FunDB's causal operators should follow this pattern: each causal operator is a specialized `TableFunction` that receives pushed-down predicates (e.g., `MIN_STRENGTH > 0.3`) and produces result chunks.

- **Operator fusion.** DuckDB's optimizer can fuse adjacent filter and projection operators into a single pipeline-breaking operator. For causal queries, this means the planner should be able to fuse `CausalPathScan + StrengthFilter + ProjectionPrune` into a single operator that only materializes paths meeting all criteria.

- **Statistics propagation.** DuckDB propagates cardinality estimates and min/max statistics through every operator. Causal operators must do the same: `CausalPathOperator` must estimate output cardinality based on DAG density, average branching factor, and depth limit. Without this, the cost model degenerates to heuristics.

- **Parallel execution.** DuckDB uses morsel-driven parallelism: data is partitioned into morsels and worker threads pull morsels from a shared queue. For causal queries that traverse a DAG, parallelism is trickier. The relevant pattern is DuckDB's recursive CTE execution: it materializes each BFS level, then parallelizes the next expansion step across morsels of the frontier.

**Reference:** Raasveldt & Muhleisen, "DuckDB: an Embeddable Analytical Database," SIGMOD 2019.

### 2. Neo4j: Efficient Path Query Execution in Graph Databases

Neo4j is the primary reference for how to execute path queries efficiently, which directly applies to FunDB's `CausalPathOperator` and `ConfounderSearchOperator`.

**Key patterns from Neo4j:**

- **Native graph storage with adjacency lists.** Neo4j stores nodes and relationships in separate fixed-size record stores. Each node has a pointer to its first relationship; relationships form a doubly-linked list per node. This enables O(1) neighbor access per node, independent of total graph size. FunDB's `_caused_by` and `_effects` edges in the FunRecord already follow this adjacency-list pattern within the columnar page groups.

- **Index-free adjacency.** The critical insight: when traversing from node A to node B via a relationship, Neo4j follows a direct pointer, not a join or index lookup. This gives O(k) traversal cost per hop where k is the degree of the current node, compared to O(log n) for an index-based join. FunDB's Causal DAG index should store forward pointers (`_effects`) and reverse pointers (`_caused_by`) as direct record references.

- **ShortestPath and AllShortestPaths operators.** Neo4j implements bidirectional BFS for shortest path queries. For `TRACE CAUSALITY FROM A TO B`, bidirectional search from both ends can dramatically reduce the search space: instead of exploring O(b^d) nodes (b = branching factor, d = depth), bidirectional search explores O(2 * b^(d/2)).

- **Variable-length pattern matching.** Cypher's `(a)-[r:CAUSES*1..5]->(b)` compiles to a `VarLengthExpand` operator that maintains a frontier and expands level by level. It supports inline predicate filtering (e.g., `WHERE ALL(rel IN r WHERE rel.strength > 0.3)`), which prunes paths eagerly. FunDB's `CausalPathOperator` should implement the same eager pruning.

- **Path uniqueness semantics.** Neo4j distinguishes between `RELATIONSHIP_UNIQUE` (no edge repeated) and `NODE_UNIQUE` (no node repeated) traversal. For causal DAGs, since DAGs are acyclic by definition, node uniqueness comes for free. However, when multiple causal paths exist between two nodes (diamond patterns), FunDB must decide whether to return all paths or aggregate them.

- **Degree-aware planning.** Neo4j's planner uses degree statistics (average, distribution) to estimate traversal cost and choose expansion direction. For causal queries, if node A has 2 effects and node B has 50 causes, it is cheaper to expand forward from A than backward from B. FunDB's cost model must incorporate DAG degree statistics.

**Reference:** Robinson, Webber & Eifrem, "Graph Databases," O'Reilly, 2015. Neo4j internal architecture documentation.

### 3. PostgreSQL: Custom Operator Implementation via Extensions

PostgreSQL provides the most battle-tested example of how to add custom data types and operators to a mature query engine, which informs FunDB's integration approach.

**Key patterns from PostgreSQL:**

- **Custom types and operators via `CREATE TYPE` and `CREATE OPERATOR`.** PostgreSQL allows defining new data types (e.g., `causal_score`), operators (e.g., `A ?-> B` for "does A cause B?"), and operator classes for index integration. FunDB should define causal types at the engine level rather than as extensions, but the abstraction boundaries are instructive.

- **Custom index access methods (Index AM).** PostgreSQL's `amhandler` API allows defining new index types. The GiST (Generalized Search Tree) framework is particularly relevant: it defines a generic tree structure where the specific key comparison, penalty, and picksplit functions are pluggable. FunDB's Causal DAG index could be built as a specialized access method following similar abstraction boundaries.

- **Cost estimation hooks.** PostgreSQL's `EXPLAIN` output includes row estimates and costs for each operator. Custom functions can provide cost estimate callbacks (`procost`). Causal operators must integrate with FunDB's cost model by providing analogous callbacks.

- **Set-returning functions (SRFs).** Complex operators in PostgreSQL are often implemented as SRFs that produce multiple rows per input row. The `CausalPathOperator` naturally fits this pattern: given one (source, target) pair as input, it produces multiple path rows as output.

**Reference:** PostgreSQL 16 documentation, "Index Access Method Interface Definition."

### 4. ClickHouse: Materialized Views for Pre-Computation

ClickHouse demonstrates how materialized views and pre-aggregation can transform expensive analytical queries into near-instant lookups, which directly applies to FunDB's causal query caching strategy.

**Key patterns from ClickHouse:**

- **Materialized views as insert triggers.** In ClickHouse, a materialized view is defined with a `SELECT` query and a target table. Every insert into the source table triggers the `SELECT` to execute on the new data, and results are inserted into the target table. For causal reasoning, FunDB could use this pattern: when a new `_caused_by` edge is inserted, a materialized view automatically updates the transitive closure cache for affected paths.

- **AggregatingMergeTree for incremental aggregation.** ClickHouse's `AggregatingMergeTree` engine stores partial aggregation states that are merged during reads. For causal scores, FunDB could maintain partial Bayesian evidence states (partial likelihood ratios, partial confounder counts) that are incrementally updated as new evidence arrives and merged at query time.

- **Projection (materialized sort orders).** ClickHouse projections store the same data pre-sorted in different orders. For the Causal DAG index, maintaining both forward-sorted (cause -> effects) and reverse-sorted (effect -> causes) projections enables bidirectional traversal without runtime sorting.

**Reference:** ClickHouse documentation, "Materialized Views and Projections."

### 5. CockroachDB: Distributed Execution of Complex Operators

CockroachDB provides the reference for executing complex analytical operations across a distributed cluster with range-partitioned data.

**Key patterns from CockroachDB:**

- **DistSQL execution engine.** CockroachDB compiles SQL into a distributed execution DAG (DistSQL plan) where processors run on the nodes that own the relevant data ranges. The coordinator assembles partial results from each node. For causal queries, this means: if a causal chain spans shards 1, 2, and 3, the planner must schedule partial traversals on each shard and a merge step on the coordinator.

- **Table readers and joiners co-located with data.** CockroachDB pushes computation to data rather than pulling data to computation. For the `NaturalExperimentOperator`, which needs to scan historical data for quasi-experiments, this pattern is essential: the heavy scan and filter logic runs on each shard, and only matching candidates are sent to the coordinator.

- **Flow control and backpressure.** In distributed execution, slow shards can bottleneck the entire query. CockroachDB uses per-flow memory budgets and backpressure signals. FunDB's distributed causal queries must implement similar budgets, especially since causal path expansion can explode combinatorially.

- **Distributed hash joins for cross-shard merging.** When causal chains cross shard boundaries, FunDB needs a mechanism analogous to CockroachDB's distributed hash join: shard 1 produces partial paths ending at shard-boundary nodes, shard 2 continues those paths from its boundary nodes, and the coordinator stitches the segments together.

**Reference:** Taft et al., "CockroachDB: The Resilient Geo-Distributed SQL Database," SIGMOD 2020.

---

## Relevant Database Research Papers on Probabilistic and Causal Queries

### 1. Probabilistic Databases

**MayBMS and Trio:** These systems from the 2000s-2010s extended relational databases with probabilistic semantics. Each tuple carries a probability of existence, and query operators propagate probabilities through the relational algebra. The key results:

- Query evaluation over probabilistic databases is #P-hard in general, but tractable for "safe queries" (hierarchical queries without self-joins).
- Approximation via Monte Carlo sampling provides bounded-error estimates for intractable queries.
- FunDB's confidence propagation rules (Section 7.2 of ARCHITECTURE.md) are a simplified version of this: `JOIN -> min`, `UNION -> max`, `TRAVERSE -> product`. These rules trade theoretical rigor for computational tractability, which is the right trade-off for a production database.

**Reference:** Suciu, Olteanu, Re, Koch, "Probabilistic Databases," Morgan & Claypool, 2011.

### 2. Graph Reachability and Transitive Closure

**Reachability indexing** is a well-studied problem. Key results relevant to FunDB:

- **2-Hop labeling** (Cohen et al., 2003): Pre-compute for each node a set of "hubs" such that any reachable pair shares a hub. Query time is O(sqrt(n)) but pre-computation is expensive. Suitable for static DAGs; not suitable for FunDB's dynamic causal graph.
- **GRAIL** (Yildirim, Chaoji, Zaki, 2010): Assigns random interval labels to nodes via DFS. Reachability queries are answered by checking interval containment. False positives require verification by actual traversal. Low storage overhead, suitable for dynamic graphs. This is a strong candidate for FunDB's causal DAG reachability checks.
- **Tree decomposition** approaches: Decompose the DAG into a tree structure for O(log n) reachability queries. Useful when the DAG has low treewidth, which causal graphs often do (most events have few direct causes).

**Reference:** Cohen et al., "Reachability and Distance Queries via 2-Hop Labels," SODA 2003. Yildirim et al., "GRAIL: Scalable Reachability Index on Large Graphs," VLDB 2010.

### 3. Temporal Event Pattern Mining

**Chronicle mining and temporal pattern discovery** are relevant to FunDB's `NaturalExperimentOperator`:

- **SASE (Stream-based and Shared Processing)**: Pattern matching over event streams using NFA-based evaluation. Could be adapted for detecting temporal patterns in FunDB's time-series data.
- **Episode mining** (Mannila, Toivonen, Verkamo, 1997): Discovers frequent temporal patterns (episodes) in event sequences. The `NaturalExperimentOperator` is essentially looking for episodes where a treatment event was followed by an outcome event in certain contexts but not others.

**Reference:** Mannila, Toivonen, Verkamo, "Discovery of Frequent Episodes in Event Sequences," Data Mining and Knowledge Discovery, 1997.

### 4. Causal Inference in Databases

**CAPE (Causal Analysis via Probabilistic Estimation)** by Salimi et al. (2020): Integrates causal inference directly into SQL query processing. Key ideas:

- Formalize causal queries as extensions to SQL: `SELECT AVG(outcome) FROM data INTERVENE ON treatment = 1` implements the do-calculus.
- Use database statistics (histograms, join selectivity) to estimate causal effects without full table scans.
- Leverage database indexes to accelerate propensity score matching (finding control groups).
- FunDB's design can build on this work, especially the insight that existing database statistics can serve double duty for causal estimation.

**Reference:** Salimi, Cole, Li, Suciu, "Causal Relational Learning," SIGMOD 2020.

### 5. Approximate Query Processing

For expensive causal computations, approximate query processing (AQP) provides degradation strategies:

- **BlinkDB**: Uses stratified sampling to answer aggregate queries with bounded error and confidence intervals. For causal queries, sampling the DAG or the confounder search space can provide approximate causal scores with bounded uncertainty.
- **Online aggregation** (Hellerstein et al., 1997): Returns progressively refined results as more data is processed. The user can stop when the confidence interval is tight enough. This is directly applicable to FunDB's timeout strategy: if the causal computation exceeds the time budget, return the current best estimate with its confidence interval.

**Reference:** Agarwal et al., "BlinkDB: Queries with Bounded Errors and Bounded Response Times on Very Large Data," EuroSys 2013. Hellerstein et al., "Online Aggregation," SIGMOD 1997.

---

## Trade-offs: Pre-Computation vs. Query-Time Computation for Causal Inference

### The Fundamental Tension

Causal reasoning in a database faces a core trade-off between freshness and latency:

| Dimension | Pre-Computation | Query-Time Computation |
|-----------|----------------|----------------------|
| **Latency** | Sub-millisecond (lookup) | Milliseconds to seconds (compute) |
| **Freshness** | Stale until refresh | Always current |
| **Storage** | High (materialized paths/scores) | Low (only raw edges) |
| **Write amplification** | High (every edge insert triggers updates) | None |
| **Correctness** | May be inconsistent during updates | Always consistent |
| **Scalability** | O(V^2) worst case for transitive closure | O(d * b^d) per query |

### FunDB's Recommended Hybrid Strategy

Based on the research, the optimal design for FunDB is a **tiered computation model**:

**Tier A -- Always Pre-Computed (Hot Paths):**
- Direct causal edges (`_caused_by`, `_effects`) are stored in the FunRecord and indexed. Cost: O(1) per edge lookup.
- Top-K most frequently queried causal paths are materialized in the transitive closure cache. Refreshed incrementally on edge insert/delete.
- Causal DAG reachability labels (GRAIL-style) are maintained for fast "does A eventually cause B?" checks. Cost: O(sqrt(n)) per query.

**Tier B -- Computed On Demand, Cached (Warm Paths):**
- Full causal path enumeration (all paths from A to B with depth <= d) is computed at query time using bidirectional BFS on the DAG index, then cached with a TTL.
- Causal scores (the 5-signal fusion) for a specific (cause, effect) pair are computed on first query and cached with an invalidation trigger on relevant edge or data changes.
- The cache is an LRU map keyed by `(source_id, target_id, max_depth, min_strength)`.

**Tier C -- Always Computed (Cold Paths):**
- Natural experiment detection: requires scanning historical data and cannot be meaningfully pre-computed because the query parameters (which cause, which effect) are not known in advance.
- Confounder search: the search space is too large to pre-compute for all possible (cause, effect) pairs.
- Counterfactual/interventional queries: depend on the specific SCM and intervention, making pre-computation infeasible.

**Adaptive promotion:** The Index Advisor (Section 9.4 of ARCHITECTURE.md) monitors which causal queries are frequent and promotes Tier C computations to Tier B caches, and Tier B caches to Tier A materialized paths, when the query frequency justifies the storage and write amplification costs.

### Storage Overhead Estimates

For a causal DAG with V nodes and E edges:

- **Raw edges:** 32 bytes/edge (source UUID + target UUID) = 32E bytes
- **GRAIL labels:** ~16 bytes/node (2 intervals) = 16V bytes
- **Transitive closure (top-1000 paths):** ~128 bytes/path (2 UUIDs + strength + depth + timestamp) * 1000 = 128KB
- **LRU cache (warm paths):** Configurable, default 64MB per shard

For a 1M-edge causal graph: raw edges = 32MB, GRAIL labels = ~160KB (assuming 10K nodes), total index overhead = ~35MB. This is modest compared to HNSW vector indexes which typically consume 1-4GB for 1M vectors.

### Incremental Maintenance

When a new causal edge `(A CAUSED B, strength=0.8)` is inserted:

1. **Immediate:** Store in `_caused_by` column of B's FunRecord and `_effects` column of A's FunRecord. Update the DAG adjacency index. Cost: O(1).
2. **Synchronous:** Update GRAIL labels for affected subgraph (only nodes in the path from A's ancestors to B's descendants). Cost: O(affected_nodes) which is typically small.
3. **Asynchronous (background):** Invalidate LRU cache entries that include A or B. Recompute top-K materialized paths if A or B is a high-degree node. Recompute affected transitive closure entries. Cost: variable, bounded by background thread budget.

This design ensures writes are not blocked by expensive causal index maintenance while keeping the most important indexes (direct edges, reachability) synchronously consistent.

---

## Summary of Key Design Decisions from Research

1. **Vectorized batch execution** (from DuckDB): All causal operators process DataChunks of 1024 rows, not individual rows.
2. **Index-free adjacency** (from Neo4j): Causal edge traversal follows direct pointers, not index lookups.
3. **Bidirectional BFS** (from Neo4j): `TRACE CAUSALITY FROM A TO B` uses bidirectional search to reduce search space.
4. **Materialized views for hot paths** (from ClickHouse): Frequently accessed causal paths are pre-computed and incrementally maintained.
5. **DistSQL-style distributed execution** (from CockroachDB): Causal traversals are partitioned across shards with boundary stitching at the coordinator.
6. **GRAIL-style reachability labels** (from graph DB research): Fast O(sqrt(n)) reachability checks without full path enumeration.
7. **Online aggregation for timeouts** (from AQP research): If causal computation exceeds the time budget, return the best available estimate with a confidence interval.
8. **Three-tier computation strategy** (synthesis): Pre-compute hot paths, cache warm paths, compute cold paths on demand.
