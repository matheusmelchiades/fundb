# SP4 Proposal: Query Ergonomics and Engine Design for Causal Reasoning

**Agent:** Architect
**Sub-Problem:** SP4 — Query Ergonomics + Engine Design
**Date:** 2026-02-28

---

## 1. Overview

This proposal defines the physical query operators, FunQL syntax, and execution architecture that make the entire causal reasoning system queryable at query time. The design goal is a developer experience where causal questions are expressed in natural, readable syntax and the engine handles all complexity — operator composition, signal orchestration, timeout budgets, distributed execution, and progressive result delivery.

The principle is consistent with FunDB's SP4 mandate: a developer should be able to ask "did A cause B?" without knowing about Dempster-Shafer fusion, quasi-experimental cascades, or confounder search strategies. The query engine handles those details. The developer reads the result and acts on it.

---

## 2. FunQL Syntax Design

### 2.1 Design Philosophy

Four causal query verbs with increasing complexity and decreasing frequency of use:

| Verb | Use case | Typical latency | Signals used |
|------|----------|----------------|--------------|
| `INFER CAUSALITY FROM A TO B` | Quick point estimate: "is there a causal link?" | 50-150ms | S1 + S3 (fast path) |
| `ASSESS CAUSALITY FROM A TO B` | Full 5-signal assessment | 200-500ms | All 5 signals |
| `EXPLAIN CAUSALITY FROM A TO B` | Why did the engine reach that conclusion? | 200-500ms + Red Team | All 5 + Skeptic checklist |
| `TRACE CAUSALITY FROM A TO B` | Existing: enumerate causal paths | Existing | Existing |

### 2.2 INFER CAUSALITY

The lightest verb. Returns a single causal confidence score for a direct (A, B) pair using only the fast signals (temporal precedence and confounder scan). Suitable for high-frequency programmatic use where speed matters more than completeness.

```sql
-- Minimal form
INFER CAUSALITY FROM :event_a TO :event_b;

-- Returns:
-- {
--   causal_confidence:  float,        -- Bel(C) from Dempster-Shafer
--   uncertainty_width:  float,        -- width of ignorance interval
--   signals_used:       ["S1","S3"],  -- what was computed
--   latency_ms:         int
-- }

-- With options
INFER CAUSALITY
    FROM :event_a
    TO   :event_b
    WITH SIGNALS (temporal = ON, mechanism = ON, confounders = OFF, experiment = OFF, consensus = OFF)
    MIN_CONFIDENCE 0.5   -- return null if confidence is below threshold
    TIMEOUT 100ms;       -- hard timeout; returns partial result if exceeded

-- Bulk form: what caused event_b?
INFER CAUSALITY
    FROM candidates IN (SELECT id FROM events WHERE _valid_from < :event_b_time)
    TO   :event_b
    LIMIT 20
    ORDER BY causal_confidence DESC;
```

### 2.3 ASSESS CAUSALITY

The standard verb for thorough analysis. Runs all 5 signals and fuses them using the Statistician's Dempster-Shafer framework.

```sql
-- Standard form: assess a specific pair
ASSESS CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id;

-- Full form with signal configuration
ASSESS CAUSALITY
    FROM  :cause_event_id
    TO    :effect_event_id
    WITH SIGNALS (
        temporal    = ON,
        mechanism   = ON (max_tiers: 3),
        confounders = ON (lookback: '90 days', strategies: ALL),
        experiment  = ON (budget_ms: 400, prefer_method: 'DiD'),
        consensus   = ON
    )
    RETURN (
        causal_confidence,
        uncertainty_width,
        conflict_degree,
        plausibility,
        signal_values,      -- individual S1-S5 scores
        quasi_experiment    -- full NaturalExperimentResult from Historian
    );

-- Returns:
-- {
--   causal_confidence:  0.78,
--   uncertainty_width:  0.12,
--   conflict_degree:    0.04,
--   plausibility:       0.90,
--   signals_used:       ["S1","S2","S3","S4","S5"],
--   signal_values: {
--     S1_temporal:      0.91,
--     S2_mechanism:     0.72,
--     S3_confounder:    0.83,
--     S4_experiment:    0.87,
--     S5_consensus:     0.60
--   },
--   quasi_experiment: {
--     method_used:      "DiD",
--     effect_estimate:  0.0032,
--     confidence_interval: [0.0024, 0.0040],
--     ...
--   }
-- }
```

### 2.4 EXPLAIN CAUSALITY

Runs the full 5-signal assessment AND the Skeptic's Red Team checklist. Returns a structured breakdown of the reasoning, all warnings, and the checklist results. Designed for debugging, auditing, and understanding why the engine reached a conclusion.

```sql
-- Explain the reasoning behind a causal assessment
EXPLAIN CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id;

-- Returns everything from ASSESS CAUSALITY plus:
-- {
--   ...assess_results,
--   red_team_checklist: {
--     direction_check:          { passed: true },
--     common_cause_scan:        { passed: true, candidates_found: 0 },
--     collider_check:           { passed: true },
--     sample_bias_check:        { passed: true },
--     aggregation_check:        { passed: true },
--     multiple_comparisons:     { passed: true, n_tests: 1 },
--     effect_size:              { cohens_d: 0.81, passed: true },
--     temporal_stability:       { passed: true, drift_detected: false },
--     mechanism_grounding:      { passed: true, intermediates_found: 3 },
--     source_independence:      { root_sources: 2, total_sources: 5 },
--     feedback_loop_check:      { passed: true },
--     sample_size:              { power: 0.91, passed: true },
--     adversarial_pattern:      { manipulation_risk: 0.02, passed: true },
--     regression_to_mean:       { triggered_by_extreme: false, passed: true },
--     confidence_ceiling:       { ceiling: 0.85, applied: false }
--   },
--   explanation_text: "A caused B with confidence 0.78. Evidence: 5 signals all support
--                      causation. Strongest evidence: Natural experiment (DiD, effect=0.0032,
--                      p<0.001). Weakest link: Source consensus is based on 5 sources but
--                      only 2 are independent. Red Team: all 15 checks passed."
-- }

-- Explain with natural language output (for LLM consumption)
EXPLAIN CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id
    FORMAT natural_language
    CONTEXT_BUDGET 500;  -- limit explanation to ~500 tokens
```

### 2.5 TRACE CAUSALITY

Existing operator — kept backward-compatible. Now enhanced to surface signal strength along paths.

```sql
-- Existing: enumerate causal paths (unchanged behavior)
TRACE CAUSALITY
    FROM :source
    TO   :target
    MAX_DEPTH 5
    MIN_STRENGTH 0.3;

-- Enhanced: include signal metadata per edge
TRACE CAUSALITY
    FROM :source
    TO   :target
    MAX_DEPTH 5
    MIN_STRENGTH 0.3
    INCLUDE SIGNAL_METADATA;  -- new optional clause
-- Adds signal_values and method_used to each edge in the path
```

---

## 3. Physical Operators

All causal operators follow DuckDB's Volcano model: each operator implements `Init()`, `GetChunk()`, and `Close()`. They process data in batches of 1024 rows (FunDB's standard DataChunk size). Operators are composed via pull-based pipelining: the top-level `CausalityAssessOperator` pulls from its children, which pull from their children.

### 3.1 Operator Hierarchy

```
CausalityAssessOperator          -- Orchestrates all 5 signals, runs fusion
    |
    +-- TemporalPrecedenceOperator  -- Computes S1
    |
    +-- MechanismScanOperator       -- Computes S2 (from Semanticist)
    |
    +-- ConfounderScanOperator      -- Computes S3 (from Semanticist)
    |
    +-- NaturalExperimentOperator   -- Computes S4 (from Historian)
    |       |
    |       +-- ControlGroupSearchOperator   -- HNSW-based control group search
    |       +-- ITSOperator                  -- Interrupted Time Series
    |       +-- EventStudyOperator           -- Event Study
    |       +-- DifferenceInDifferencesOperator
    |       +-- SyntheticControlOperator
    |       +-- RDDOperator                  -- Regression Discontinuity
    |
    +-- SourceConsensusOperator     -- Computes S5
    |
    +-- SignalFusionOperator        -- Murphy Dempster-Shafer combination
    |
    +-- RedTeamOperator             -- Skeptic's 15-point checklist (EXPLAIN only)
```

### 3.2 CausalityAssessOperator

The root operator for `ASSESS CAUSALITY` and `EXPLAIN CAUSALITY` queries. It orchestrates parallel signal computation and then invokes fusion.

```
CausalityAssessOperator {

    Init(cause_id: UUID, effect_id: UUID, signal_config: SignalConfig):
        self.cause  = FunDB.load(cause_id)
        self.effect = FunDB.load(effect_id)
        self.config = signal_config
        self.signal_futures = {}
        self.result = null

        // Start all enabled signals in parallel (each runs on a worker thread)
        IF config.temporal_on:
            self.signal_futures[S1] = thread_pool.submit(TemporalPrecedenceOperator.compute,
                                                          self.cause, self.effect)
        IF config.mechanism_on:
            self.signal_futures[S2] = thread_pool.submit(MechanismScanOperator.compute,
                                                          self.cause, self.effect, config.mechanism)
        IF config.confounders_on:
            self.signal_futures[S3] = thread_pool.submit(ConfounderScanOperator.compute,
                                                          self.cause, self.effect, config.confounders)
        IF config.experiment_on:
            self.signal_futures[S4] = thread_pool.submit(NaturalExperimentOperator.compute,
                                                          self.cause, self.effect, config.experiment)
        IF config.consensus_on:
            self.signal_futures[S5] = thread_pool.submit(SourceConsensusOperator.compute,
                                                          self.cause, self.effect)

    GetChunk():
        IF self.result is not null:
            RETURN EMPTY  // single-row operator, already returned result

        // Collect results from all signal futures
        // Apply per-signal timeout: signal gets its allocated budget
        signals = {}
        FOR signal_id, future IN self.signal_futures.items():
            try:
                signals[signal_id] = future.get(timeout = SIGNAL_TIMEOUTS[signal_id])
            EXCEPT TimeoutError:
                signals[signal_id] = None  // missing signal = ignorance mass in fusion
                self.warnings.append(Warning(f"{signal_id} timed out, treated as absent"))

        // Temporal gate: check S1 before fusion
        s1 = signals.get(S1)
        IF s1 is not None AND s1.value < 0.2:
            // Temporal violation: return early with low score
            self.result = build_temporal_violation_result(s1)
            RETURN DataChunk([self.result])

        // Run signal fusion
        fusion_input = [(sig.value, sig.reliability) if sig else None
                        for sig in [signals.get(i) for i in [S1,S2,S3,S4,S5]]]
        fusion_result = SignalFusionOperator.fuse(fusion_input)
        fusion_result = apply_temporal_gate(fusion_result, s1)

        self.result = build_result(fusion_result, signals, self.warnings)
        RETURN DataChunk([self.result])

    Close():
        // Cancel any pending futures
        FOR future IN self.signal_futures.values():
            future.cancel()
}
```

**Parallelism:** All 5 signal operators launch concurrently on FunDB's worker thread pool. The total latency is bounded by `max(signal_latencies)`, not `sum(signal_latencies)`. In practice, S4 (natural experiment) dominates at 100-400ms; the other signals complete in 1-100ms and return before S4 finishes.

### 3.3 NaturalExperimentOperator

Wraps the Historian's cascade algorithm. Follows the TableFunction pattern: produces exactly one output row per (cause, effect) pair.

```
NaturalExperimentOperator {

    Init(cause: FunRecord, effect: FunRecord, config: ExperimentConfig):
        self.cause  = cause
        self.effect = effect
        self.config = config
        self.result_emitted = false

    GetChunk():
        IF self.result_emitted:
            RETURN EMPTY

        result = NaturalExperimentDetector.run(
            cause_event_id  = self.cause._id,
            effect_event_id = self.effect._id,
            config          = self.config
        )

        // Convert to S4 signal format for SignalFusionOperator
        output_row = {
            s4_value:       result.s4_signal,
            s4_reliability: result.method_reliability(),  // per-method r4 value
            found_experiment: result.found_valid_experiment,
            method_used:    result.method_used,
            effect_estimate: result.effect_estimate,
            ci_lower:       result.confidence_interval[0] if result.confidence_interval else null,
            ci_upper:       result.confidence_interval[1] if result.confidence_interval else null,
            qe_score:       result.quasi_experiment_score,
            diagnostics:    result.validity_diagnostics
        }

        self.result_emitted = true
        RETURN DataChunk([output_row])

    // Statistics for the query planner
    EstimatedOutputCardinality() -> int: RETURN 1
    EstimatedCostMs() -> float:
        RETURN config.budget_ms * 0.8  // typically uses 80% of budget
}
```

### 3.4 SignalFusionOperator

Stateless operator. Takes 5 (value, reliability) pairs, applies Murphy Dempster-Shafer combination, applies temporal gate, returns fused result.

```
SignalFusionOperator {

    // Static method — no state needed
    fuse(signals: list[tuple[float, float] | None]) -> FusionResult:

        // Direct delegation to Statistician's fuse_signals()
        result = fuse_signals(signals)  // from Statistician's PROPOSAL

        RETURN FusionResult {
            causal_confidence:  result.causal_confidence,
            uncertainty_width:  result.uncertainty_width,
            conflict_degree:    result.conflict_degree,
            plausibility:       result.plausibility
        }

    GetChunk(input: DataChunk) -> DataChunk:
        output = []
        FOR row IN input.rows:
            signals = [
                (row.s1, row.r1) if row.s1 is not null else None,
                (row.s2, row.r2) if row.s2 is not null else None,
                (row.s3, row.r3) if row.s3 is not null else None,
                (row.s4, row.r4) if row.s4 is not null else None,
                (row.s5, row.r5) if row.s5 is not null else None
            ]
            output.append(self.fuse(signals))
        RETURN DataChunk(output)
}
```

### 3.5 MechanismScanOperator

Wraps the Semanticist's `MechanismDetect` algorithm.

```
MechanismScanOperator {

    compute(cause: FunRecord, effect: FunRecord, config: MechanismConfig) -> SignalResult:

        result = MechanismDetect(cause, effect, max_tiers=config.max_tiers,
                                 min_score=config.min_score)

        RETURN SignalResult {
            signal_id:   S2,
            value:       result.mechanism_score,
            reliability: 0.50,  // Statistician's default r2
            evidence:    result.mechanism_evidence,
            type:        result.mechanism_type
        }
}
```

### 3.6 ConfounderScanOperator

Wraps the Semanticist's `ConfounderSearch` algorithm. Converts coverage and top confounder score to S3.

```
ConfounderScanOperator {

    compute(cause: FunRecord, effect: FunRecord, config: ConfounderConfig) -> SignalResult:

        result = ConfounderSearch(cause, effect, config)

        // S3 conversion:
        //   No confounders found + high coverage = strong signal (near 1.0)
        //   Strong confounder found = weak signal (near 0.0)
        //   Low coverage = moderate signal (near 0.5)
        top_score = max(c.confounder_score for c in result.confounders, default=0.0)
        coverage  = result.coverage_score

        s3 = (1.0 - top_score) * coverage + 0.5 * (1.0 - coverage)
        // When coverage=1.0: s3 = 1 - top_score (full confidence)
        // When coverage=0.0: s3 = 0.5 (total ignorance about confounders)
        // When coverage=0.5 and top_score=0.8: s3 = 0.2*0.5 + 0.5*0.5 = 0.35

        RETURN SignalResult {
            signal_id:         S3,
            value:             s3,
            reliability:       0.70,  // Statistician's default r3
            confounders_found: result.confounders,
            coverage:          coverage
        }
}
```

### 3.7 RedTeamOperator

Runs the Skeptic's 15-point checklist. Only invoked for `EXPLAIN CAUSALITY`. Takes the full assessment result as input and adds checklist annotations.

```
RedTeamOperator {

    run(cause: FunRecord, effect: FunRecord,
        assessment: AssessmentResult) -> RedTeamResult:

        checklist = RedTeamChecklist()

        // Each check is independent and runs in parallel
        checks = parallel_execute([
            check_direction(cause, effect),                   // 1
            check_common_cause(cause, effect),                // 2
            check_collider(cause, effect),                    // 3
            check_sample_bias(cause, effect),                 // 4
            check_aggregation_level(cause, effect),           // 5
            check_multiple_comparisons(assessment),           // 6
            check_effect_size(assessment),                    // 7
            check_temporal_stability(cause, effect),          // 8
            check_mechanism_grounding(assessment),            // 9
            check_source_independence(cause, effect),         // 10
            check_feedback_loop(cause, effect),               // 11
            check_sample_size(assessment),                    // 12
            check_adversarial_pattern(cause, effect),         // 13
            check_regression_to_mean(cause, effect),          // 14
            check_confidence_ceiling(assessment)              // 15
        ])

        RETURN RedTeamResult {
            all_passed:  all(c.passed for c in checks),
            checks:      checks,
            warnings:    [c.warning for c in checks if not c.passed],
            explanation_text: build_explanation(assessment, checks)
        }
}
```

---

## 4. Execution Plan: Volcano Model Composition

The causal operators compose in the standard Volcano pull-based model. The top-level client pulls a DataChunk from the root operator, which recursively pulls from its children.

### 4.1 ASSESS CAUSALITY Execution Plan

```
Physical Plan for:
    ASSESS CAUSALITY FROM :cause_id TO :effect_id WITH SIGNALS (ALL ON)

ProjectionOperator
  columns: [causal_confidence, uncertainty_width, conflict_degree, plausibility,
            signal_values, quasi_experiment]
    |
    v
CausalityAssessOperator
  cause_id:  :cause_id
  effect_id: :effect_id
  signals:   ALL
    |
    +--[parallel]-- TemporalPrecedenceOperator
    |                 input:  (cause, effect) from FunRecord lookup
    |                 output: (s1_value=0.91, r1=0.60)
    |
    +--[parallel]-- MechanismScanOperator
    |                 input:  (cause, effect)
    |                 output: (s2_value=0.72, r2=0.50, mechanism_type=BRIDGED)
    |
    +--[parallel]-- ConfounderScanOperator
    |                 input:  (cause, effect)
    |                 output: (s3_value=0.83, r3=0.70, n_confounders_found=0)
    |
    +--[parallel]-- NaturalExperimentOperator
    |                 input:  (cause, effect, config)
    |                 output: (s4_value=0.87, r4=0.75, method=DiD, effect=0.0032)
    |                   |
    |                   +-- ControlGroupSearchOperator
    |                   |     (HNSW k=100, composite similarity scoring)
    |                   +-- DifferenceInDifferencesOperator
    |                         (parallel trends test, balance check, bootstrap CI)
    |
    +--[parallel]-- SourceConsensusOperator
                      input:  (cause, effect)
                      output: (s5_value=0.60, r5=0.40, n_sources=5, n_independent=2)
    |
    v  [all 5 signals collected, possibly with timeouts for slow ones]
    |
SignalFusionOperator
  input:   [(s1,r1), (s2,r2), (s3,r3), (s4,r4), (s5,r5)]
  method:  Murphy Dempster-Shafer
  output:  (causal_confidence=0.78, uncertainty_width=0.12, conflict_degree=0.04)
```

**Pipeline characteristics:**
- The 5 signal operators run in parallel on FunDB's worker pool.
- `CausalityAssessOperator.GetChunk()` blocks until all futures complete (or timeout).
- The `SignalFusionOperator` is stateless and runs inline — no separate pipeline stage.
- Total output: exactly 1 row per (cause, effect) pair. Cardinality = 1.
- Total latency: `max(S1_latency, S2_latency, S3_latency, S4_latency, S5_latency)` = dominated by S4.

### 4.2 EXPLAIN CAUSALITY Execution Plan

`EXPLAIN CAUSALITY` adds `RedTeamOperator` after fusion:

```
Physical Plan for:
    EXPLAIN CAUSALITY FROM :cause_id TO :effect_id

ExplainProjectionOperator
  columns: [...assess_columns, red_team_checklist, explanation_text]
    |
    v
RedTeamOperator
  input:  full AssessmentResult from CausalityAssessOperator
  checks: all 15 checks (parallel execution)
    |
    v
CausalityAssessOperator  [same as above]
    |
    [same signal tree as above]
```

### 4.3 INFER CAUSALITY Execution Plan (Fast Path)

`INFER CAUSALITY` uses only the fast signals and skips fusion complexity:

```
Physical Plan for:
    INFER CAUSALITY FROM :cause_id TO :effect_id TIMEOUT 100ms

InferProjectionOperator
  columns: [causal_confidence, uncertainty_width, signals_used]
    |
    v
CausalityAssessOperator
  signals: temporal=ON, mechanism=ON, confounders=OFF, experiment=OFF, consensus=OFF
    |
    +--[parallel]-- TemporalPrecedenceOperator  (< 2ms)
    +--[parallel]-- MechanismScanOperator (max_tiers=1 for speed, < 5ms)
    |
    v
SignalFusionOperator
  input:   [(s1,r1), (s2,r2), None, None, None]
  output:  (causal_confidence, uncertainty_width=high because 3 signals absent)
```

Latency: 5-15ms. The high `uncertainty_width` honestly communicates that only 2 of 5 signals were used.

---

## 5. Three-Tier Computation Strategy

Following the research synthesis, causal queries are routed to the appropriate tier based on data freshness and pre-computation status.

### 5.1 Tier A: Hot Paths (Pre-Computed, < 1ms)

Pre-computed at write time. Served from in-memory cache.

**Contents:**
- Direct causal edges (`_caused_by`, `_effects`) from FunRecord: O(1) lookup per edge.
- GRAIL-style reachability labels for fast "does A eventually cause B?" checks.
- Top-1000 most-queried causal pairs: full `CausalityAssessResult` materialized and versioned.
- Index Advisor (Section 9.4 of ARCHITECTURE.md) determines which pairs qualify as "top-1000" based on query frequency.

**When used:**
```sql
-- Hot path: result already computed
ASSESS CAUSALITY FROM :cause TO :effect;
-- If (cause, effect) is in the hot cache: return in < 1ms
-- If not: fall through to Tier B or C
```

**Invalidation:** When a new causal edge touching `cause` or `effect` is inserted, the cached result is marked stale. It is recomputed asynchronously in the background (write-ahead log + background worker). During recomputation, the stale result is returned with `result_freshness: "stale"` metadata.

### 5.2 Tier B: Warm Paths (Cached On-Demand, 1-50ms)

LRU cache keyed by `(cause_id, effect_id, signal_config_hash, max_depth, min_strength)`. Populated on first query, evicted when stale or memory-pressured.

**Contents:**
- Full causal path enumeration (bidirectional BFS, cached after first query).
- Signal fusion results for pairs that were queried but not promoted to Tier A.
- Confounder search results (expensive; worth caching because the search space is stable).

**TTL and invalidation:**
```
Cache entry TTL = 15 minutes (configurable per collection)
Invalidated by:
  - New causal edge inserted touching cause or effect
  - New record inserted in the relevant time window (for confounder search)
  - Explicit INVALIDATE CAUSAL CACHE FOR :event_id command
```

**Cache size:** Default 64MB per shard. Holds approximately 500,000 simple fusion results or 5,000 full assessment results with quasi-experiment details.

### 5.3 Tier C: Cold Paths (Computed On-Demand, 50-500ms)

Always computed fresh. Never pre-computed because the search space is too large or the inputs are not known in advance.

**Contents:**
- `NaturalExperimentDetector` runs (the Historian's cascade).
- Confounder search for a novel (cause, effect) pair not in the cache.
- Counterfactual queries (`IF NOT A, what would B have been?`).
- Any query with `BYPASS CACHE` option.

**Budget enforcement:**
```
Each cold-path operator receives a millisecond budget from the query's total timeout.
If the budget is exceeded, the operator returns its current best result with:
  is_partial:        true
  confidence_width:  widened to reflect incomplete analysis
  signals_completed: [list of which signals finished before timeout]
  signals_pending:   [list of which signals were still running]
```

### 5.4 Tier Routing Logic

```
route_causal_query(cause_id, effect_id, signal_config):

    // Check Tier A (hot cache)
    cache_key = build_hot_cache_key(cause_id, effect_id, signal_config)
    hot_result = hot_cache.get(cache_key)
    IF hot_result is not None AND NOT hot_result.is_stale:
        RETURN hot_result WITH metadata.tier = "HOT"

    // Check Tier B (warm cache)
    warm_result = lru_cache.get(cache_key)
    IF warm_result is not None AND NOT warm_result.is_expired:
        RETURN warm_result WITH metadata.tier = "WARM"

    // Tier C: compute fresh
    result = CausalityAssessOperator.execute(cause_id, effect_id, signal_config)
    lru_cache.put(cache_key, result, ttl=15min)

    // Promote to hot cache if query frequency warrants
    IF index_advisor.should_promote(cause_id, effect_id):
        hot_cache.put(cache_key, result)
        schedule_background_refresh(cache_key, refresh_interval=5min)

    RETURN result WITH metadata.tier = "COLD"
```

---

## 6. Timeout and Progressive Results

Causal queries are expensive and may exceed the client's time budget. The engine handles this gracefully rather than returning an error.

### 6.1 Per-Signal Timeouts

Each signal has a default time budget within the overall query budget:

| Signal | Default budget | Can exceed? |
|--------|---------------|------------|
| S1 Temporal | 5ms | No — trivial computation |
| S2 Mechanism | 100ms | Yes, with reduced tier (skip NLI) |
| S3 Confounder | 75ms | Yes, with reduced strategies (skip text mining) |
| S4 Experiment | 400ms | Yes, with fallback to ITS only |
| S5 Consensus | 20ms | No — simple provenance scan |

### 6.2 Graceful Degradation Protocol

```
When a query's TIMEOUT is exceeded:

  1. CANCEL: Send cancellation signal to all running signal futures.
  2. COLLECT: Gather results from signals that have completed.
  3. WIDEN: For each incomplete signal, substitute None (ignorance mass).
     This widens uncertainty_width — an honest representation of partial analysis.
  4. FUSE: Run SignalFusionOperator on available signals.
  5. TAG: Set result.is_partial = true.
  6. EXPLAIN: Set result.timeout_explanation to describe what was not computed.
  7. RETURN: Return the partial result immediately.

Example partial result:
{
  "causal_confidence":  0.65,
  "uncertainty_width":  0.28,   // wider than a full 5-signal result
  "is_partial":         true,
  "signals_completed":  ["S1", "S2", "S3"],   // fast signals completed
  "signals_pending":    ["S4", "S5"],          // did not complete in time
  "timeout_explanation": "Natural experiment detection (S4) was incomplete due to
                          500ms timeout. Result reflects 3 of 5 signals only.
                          Re-run with TIMEOUT 1000ms for complete analysis.",
  "latency_ms":         500
}
```

### 6.3 Streaming Results (Progressive Refinement)

For interactive use, the engine can stream signal results as they complete:

```sql
-- Stream results as signals complete (WebSocket / SSE)
ASSESS CAUSALITY
    FROM :cause_id
    TO   :effect_id
    STREAM;

-- Client receives a sequence of partial results:
-- t=3ms:   { signals_done: ["S1"], partial_confidence: 0.70, uncertainty: 0.40 }
-- t=15ms:  { signals_done: ["S1","S2"], partial_confidence: 0.72, uncertainty: 0.35 }
-- t=80ms:  { signals_done: ["S1","S2","S3","S5"], partial_confidence: 0.74, uncertainty: 0.20 }
-- t=350ms: { signals_done: ["S1","S2","S3","S4","S5"], final_confidence: 0.78, uncertainty: 0.12, is_final: true }
```

This allows clients to display a progressively-narrowing confidence interval in real time, which is more informative than waiting for the full result or receiving a timeout error.

---

## 7. Distributed Execution

FunDB's sharded architecture requires causal queries to work across shard boundaries. Not all causal data for a given (cause, effect) pair lives on the same shard.

### 7.1 Data Partitioning for Causal Queries

FunDB partitions FunRecords by tenant_id (default) or entity_id. A causal query for cause on shard 2 and effect on shard 5 requires cross-shard coordination.

**Observation:** For most causal queries, the cause and effect belong to the same tenant/entity, so they live on the same shard. Cross-shard queries are the minority case (typically: global event A caused behavior on tenant B).

### 7.2 Distributed Execution Plan

For cross-shard causal queries, the coordinator (query planner) generates a DistSQL-style plan:

```
Coordinator Plan for ASSESS CAUSALITY FROM :cause (shard 2) TO :effect (shard 5):

  CoordinatorNode:
    ProjectionOperator
      |
      SignalFusionOperator
        |
        GatherOperator  (assembles results from all shard nodes)
          |
          +-- Shard2Node (executes where cause lives):
          |     TemporalPrecedenceOperator(cause, effect_metadata)
          |     MechanismScanOperator(cause, effect_metadata)
          |     ConfounderScanOperator(cause, effect)
          |     NaturalExperimentOperator(cause, effect_metadata)
          |       [HNSW control group search runs local to shard 2's data]
          |
          +-- Shard5Node (executes where effect lives):
                EffectMetadataOperator(effect)
                [Returns effect._embedding, ._valid_from, ._metric_name to coordinator]
                SourceConsensusOperator(cause_metadata, effect)
```

**Data movement minimization:** Each shard does the heavy lifting locally. The coordinator receives only the signal scores, not raw time-series data. For the `NaturalExperimentOperator`, control group search runs on the shard that holds the relevant historical data, and only the statistical result (effect estimate, CI, diagnostics) is returned to the coordinator.

### 7.3 Cross-Shard Control Group Search

The Historian's control group construction (HNSW search for similar entities) is the most complex distributed operation:

```
Distributed control group search for NaturalExperimentOperator:

  Coordinator:
    1. Broadcast cause._embedding to all shards.
    2. Each shard performs local HNSW search (k=30, local candidates only).
    3. Shards return their top-30 candidates with similarity scores.
    4. Coordinator merges and selects global top-100.
    5. Coordinator assigns selected control entities to the shard that owns their data.
    6. Each shard extracts the time-series for its assigned control entities.
    7. Shards return time-series summaries (mean, variance per period) — not raw data.
    8. Coordinator runs DiD/SCM estimator on the aggregated summaries.

Memory overhead:
  Per shard: 30 candidates * (UUID + score + summary stats) = ~3KB
  Total coordinator memory: 100 candidates * ~30 bytes = ~3KB
  This is negligible.

Latency overhead:
  Network round trips: 2 (broadcast + merge)
  Approximate overhead: 5-15ms depending on cluster topology
  Well within the 500ms cold-path budget.
```

### 7.4 Shard-Local Hot Cache

Each shard maintains its own hot cache for pairs where both cause and effect live on the same shard. The coordinator hot cache covers cross-shard pairs. Cache invalidation propagates via the write-ahead log to all relevant shards.

---

## 8. Cost Model and Query Planning

The query planner needs cost estimates to choose execution strategies, decide when to use the cache, and allocate signal budgets.

### 8.1 Cardinality Estimates for Causal Operators

| Operator | Output cardinality | Cost estimate |
|----------|------------------|---------------|
| `CausalityAssessOperator` | 1 row per input pair | Dominated by S4 budget |
| `NaturalExperimentOperator` | 1 row per input pair | `config.budget_ms` |
| `ControlGroupSearchOperator` | k rows (config.max_control_candidates) | O(log n + k*d) |
| `MechanismScanOperator` | 1 row per input pair | 1-100ms by tier |
| `ConfounderScanOperator` | 1-20 rows per input pair | ~62ms (parallel strategies) |
| `RedTeamOperator` | 1 row per input pair | ~30ms (parallel checks) |

### 8.2 Plan Alternatives

For `ASSESS CAUSALITY`, the planner chooses between:

1. **Cache hit:** O(1) lookup — use if available and fresh.
2. **Fast path (2 signals):** `INFER CAUSALITY` pattern — for tight timeout budgets.
3. **Standard path (5 signals, parallel):** Default `ASSESS CAUSALITY`.
4. **Distributed path:** Same as standard but with cross-shard data movement.

The planner selects based on: (a) cache hit status, (b) client-specified `TIMEOUT`, (c) whether cause and effect are on the same shard.

### 8.3 Statistics for Cost Estimation

The planner uses these per-collection statistics, maintained by the Adaptive Statistics Collector:

```
CausalStatistics {
    n_events_total:          1_000_000,
    n_causal_edges:          50_000,
    avg_hnsw_search_latency: 12ms,   // measured, updated hourly
    avg_s4_latency:          280ms,  // measured, updated hourly
    causal_pair_cache_hit_rate: 0.34  // fraction of queries hitting warm cache
}
```

These statistics feed into the 3-tier routing decision and per-signal budget allocation.

---

## 9. Agent Memory: Bidirectional Learning Integration

FunDB's bidirectional learning system (ARCHITECTURE.md Section 12) allows agents to update the database based on the outcomes of decisions made using FunDB's data. For causal reasoning, this creates a learning loop where confirmed causal relationships improve future assessments.

### 9.1 Writing Back Confirmed Causal Claims

When an agent confirms (or disconfirms) a causal claim:

```sql
-- Agent confirms a causal relationship (e.g., after investigation)
CONFIRM CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id
    WITH (
        confidence: 0.9,
        mechanism:  'Deploy introduced memory leak causing OOM errors',
        confirmed_by: 'agent:debugger-v2',
        evidence:   'Code review confirmed memory leak in v4.2.1 parser'
    );

-- This writes a new CausalEdge with relation=CAUSED, strength=0.9
-- AND invalidates the hot/warm cache for this pair
-- AND updates S5 (source consensus) by adding a confirmed source
-- AND triggers Platt calibration re-fit if feedback loop is active

-- Agent disconfirms (Red Team finding: confounder was the true cause)
DISCONFIRM CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id
    WITH (
        reason: 'batch_job_schedule is the true confounder',
        true_cause: :batch_job_event_id,
        disconfirmed_by: 'agent:skeptic-v1'
    );
```

### 9.2 Platt Calibration Update

Each confirmed/disconfirmed causal claim is a labeled training example for the Statistician's Platt calibration:

```
On CONFIRM CAUSALITY (true label = 1):
    raw_score = retrieve_cached_causal_confidence(cause, effect)
    platt_calibration.online_update(raw_score, label=1)
    // SGD step: alpha += learning_rate * (1 - sigmoid(alpha * raw_score + beta))
    //           beta  += learning_rate * (1 - sigmoid(alpha * raw_score + beta))

On DISCONFIRM CAUSALITY (true label = 0):
    raw_score = retrieve_cached_causal_confidence(cause, effect)
    platt_calibration.online_update(raw_score, label=0)
    // SGD step: alpha -= learning_rate * sigmoid(alpha * raw_score + beta)
    //           beta  -= learning_rate * sigmoid(alpha * raw_score + beta)
```

This is O(1) per feedback event and requires storing only 2 float parameters (alpha, beta) per collection.

### 9.3 Reliability Parameter Learning

Over time, the system learns which signals are most reliable for a given collection:

```
Background process (runs every 24 hours):
    FOR each collection C:
        confirmed_pairs = SELECT (cause, effect, true_label)
                          FROM causal_feedback WHERE collection = C
                          AND created_at > NOW() - 90 days
                          LIMIT 1000

        IF len(confirmed_pairs) < 50:
            SKIP  // insufficient feedback

        // Logistic regression: for each signal Si, estimate optimal ri
        // that minimizes calibration error on the labeled set
        optimal_r = fit_reliability_params(confirmed_pairs, signals=[S1..S5])

        // Update collection-level reliability parameters
        collection_config[C].signal_reliabilities = optimal_r
        invalidate_warm_cache(collection=C)

// Effect: collections with rich feedback get increasingly accurate causal assessments.
// Collections with no feedback use the global defaults from the Statistician's proposal.
```

### 9.4 FunQL for Bidirectional Learning

```sql
-- Query the learning state for a collection
SELECT
    signal_name,
    current_reliability,
    n_feedback_examples,
    calibration_brier_score
FROM CAUSAL_LEARNING_STATE
WHERE collection = :collection_name;

-- Force re-calibration immediately
RECALIBRATE CAUSAL MODEL
    FOR COLLECTION :collection_name
    USING FEEDBACK FROM LAST 90 DAYS;

-- Inspect what the model has learned about a specific causal pair
SELECT *
FROM CAUSAL_PAIR_HISTORY
WHERE cause_id = :cause_event_id AND effect_id = :effect_event_id
ORDER BY assessment_time DESC;
```

---

## 10. Summary: Operator Complexity and SLA

| Query Verb | Operators invoked | Cold path SLA | Warm path SLA | Hot path SLA |
|------------|------------------|--------------|--------------|--------------|
| `INFER CAUSALITY` (fast) | S1 + S2 + SignalFusion | 15ms | 5ms | 1ms |
| `ASSESS CAUSALITY` | S1+S2+S3+S4+S5 + SignalFusion | 500ms | 50ms | 1ms |
| `EXPLAIN CAUSALITY` | All above + RedTeam | 600ms | 80ms | 5ms |
| `TRACE CAUSALITY` | BidirBFS on CausalDAG | 50ms | 10ms | 1ms |
| `EVALUATE QUASI_EXPERIMENT` | NaturalExperimentOperator | 400ms | 40ms | N/A |
| `CONFIRM/DISCONFIRM CAUSALITY` | Cache invalidation + Platt update | 10ms | N/A | N/A |

**Design invariants enforced by the engine:**
1. No causal operator blocks the write path. Signal computation is always read-only.
2. Every causal query returns within its specified `TIMEOUT`. No query hangs indefinitely.
3. Partial results are always returned when a timeout fires. Never an error.
4. The `uncertainty_width` grows monotonically as signals are dropped due to timeouts. Timeouts cannot make the system falsely confident.
5. Cache invalidation is synchronous for hot-path entries, asynchronous for warm-path entries, to avoid blocking writes.
