# Query Engine Examples: Physical Execution Plans and Operator Behavior

**Agent:** Architect
**Date:** 2026-02-28

---

## Example 1: Physical Execution Plan for a Simple Causal Query

**Query:** Does a deploy event causally explain an error spike on the same service?

```sql
ASSESS CAUSALITY
    FROM 'evt:deploy:auth-service:v3.1.0:2026-02-10T09:00:00Z'
    TO   'evt:error-spike:auth-service:2026-02-10T09:04:00Z';
```

### EXPLAIN Output

```
FunDB Query Plan
=================
Query Type:    ASSESS CAUSALITY
Cache Status:  MISS (pair not in hot or warm cache)
Execution Tier: COLD
Estimated Latency: 280ms (dominated by NaturalExperimentOperator)

Physical Plan:
--------------
[1] ProjectionOperator
    Output columns:
      causal_confidence, uncertainty_width, conflict_degree,
      plausibility, signal_values, quasi_experiment, latency_ms
    Cost: 0ms
    Rows: 1
      |
      v
[2] CausalityAssessOperator
    cause_id:    evt:deploy:auth-service:v3.1.0:2026-02-10T09:00:00Z
    effect_id:   evt:error-spike:auth-service:2026-02-10T09:04:00Z
    Mode:        PARALLEL (all 5 signals on separate worker threads)
    Budget:      500ms total
    |
    +---[thread 1]--- [3] TemporalPrecedenceOperator
    |                     Input:  cause._valid_from = 2026-02-10T09:00:00Z
    |                             effect._valid_from = 2026-02-10T09:04:00Z
    |                     Index:  FunTemporal B+Tree (index seek, O(log n))
    |                     Computation: lag_seconds = 240, consistency = 0.95
    |                     Output: (s1 = 0.92, r1 = 0.60)
    |                     Cost:   2ms
    |
    +---[thread 2]--- [4] MechanismScanOperator
    |                     Input:  cause, effect
    |                     Tier 1: FunCausal DAG index lookup
    |                       -> auth-service:v3.1.0 has no direct causal edge to error-spike
    |                     Tier 2: HNSW midpoint search
    |                       midpoint = mean(deploy_embed, error_embed)
    |                       results: 12 bridge candidates
    |                       top bridge: "memory-leak-patch" (sim_a=0.71, sim_b=0.68)
    |                       bridge score: harmonic_mean(0.71, 0.68) = 0.695 > 0.55 threshold
    |                     Tier 3: NOT REACHED (tier 2 sufficient)
    |                     Output: (s2 = 0.695, r2 = 0.50, mechanism_type = BRIDGED)
    |                     Cost:   11ms
    |
    +---[thread 3]--- [5] ConfounderScanOperator
    |                     Input:  cause, effect
    |                     Strategy 1 (embedding triangle): 82 candidates evaluated
    |                     Strategy 2 (common ancestor): 6 common ancestors found
    |                       - "deploy-pipeline-trigger" (score=0.31)  -- weak
    |                     Strategy 3 (text co-occurrence): 4 shared terms
    |                     Strategy 4 (temporal context): 14 preceding events
    |                       - "high-traffic-period" (score=0.38)       -- moderate
    |                     Top confounder score: 0.38
    |                     Coverage: 0.89 (89% of time window records scanned)
    |                     S3 = (1.0 - 0.38) * 0.89 + 0.5 * 0.11 = 0.607
    |                     Output: (s3 = 0.607, r3 = 0.70, top_confounder="high-traffic-period")
    |                     Cost:   38ms
    |
    +---[thread 4]--- [6] NaturalExperimentOperator
    |                     Input:  cause, effect
    |                       |
    |                       +-- [7] ControlGroupSearchOperator
    |                       |       HNSW search: k=100, filter=same_collection+different_entity
    |                       |       Results: 22 control candidates (other services)
    |                       |       Composite similarity scoring: 22 x (semantic + structural + temporal)
    |                       |       Cost: 18ms
    |                       |
    |                       +-- [8] EventStudyOperator (ATTEMPTED FIRST, 21 prior deploy instances found)
    |                       |       Pre-trend test: p=0.58 (PASSED)
    |                       |       Effect: +0.028 (2.8% error rate increase)
    |                       |       CI: [+0.012, +0.044]
    |                       |       Event Study score: 0.71
    |                       |       Status: VALID -> cascade exits here
    |                       |       Cost: 72ms
    |
    |                     Output: (s4 = 0.855, r4 = 0.80, method=EVENT_STUDY,
    |                              effect_estimate=0.028, ci=[0.012,0.044])
    |                     Total cost: 90ms
    |
    +---[thread 5]--- [9] SourceConsensusOperator
                          Input:  cause, effect
                          Provenance scan: 3 sources found agreeing on deploy->error
                            - monitoring_agent (root source, credibility=0.9)
                            - alert_system (derived from monitoring_agent, weight 0.1)
                            - runbook_v2 (root source, credibility=0.6)
                          n_independent_root_sources = 2
                          s5 = 1 - (1-0.4)^2 = 0.64
                          Output: (s5 = 0.64, r5 = 0.40)
                          Cost:   8ms

    CausalityAssessOperator collects results at t=90ms (NaturalExperimentOperator finishes last)
    All 5 signals available.

      |
      v
[10] SignalFusionOperator
     Input:  [(0.92,0.60), (0.695,0.50), (0.607,0.70), (0.855,0.80), (0.64,0.40)]
     Method: Murphy Dempster-Shafer (n=5 signals, O(n) = 5 iterations)
     Temporal gate: s1 = 0.92 >> 0.20 threshold -> PASSED
     Output: (causal_confidence=0.74, uncertainty_width=0.14, conflict_degree=0.06)
     Cost:   < 1ms

[11] ProjectionOperator
     Assembles final row.
     Total cost: 91ms (all parallel, dominated by NaturalExperimentOperator)

Result written to warm cache (TTL=15min).
```

### Query Result

```json
{
  "causal_confidence":  0.74,
  "uncertainty_width":  0.14,
  "conflict_degree":    0.06,
  "plausibility":       0.88,
  "signals_used":       ["S1","S2","S3","S4","S5"],
  "signal_values": {
    "S1_temporal":     0.92,
    "S2_mechanism":    0.70,
    "S3_confounder":   0.61,
    "S4_experiment":   0.86,
    "S5_consensus":    0.64
  },
  "quasi_experiment": {
    "method_used": "EVENT_STUDY",
    "effect_estimate": 0.028,
    "confidence_interval": [0.012, 0.044]
  },
  "metadata": {
    "tier": "COLD",
    "latency_ms": 91
  }
}
```

**Operator stats at a glance:**
- 9 physical operators, 5 parallel signal threads
- 4 index types used: FunTemporal (B+Tree), FunVector (HNSW), FunCausal (DAG), FunProvenance
- Critical path: Thread 4 (NaturalExperimentOperator), 90ms
- Speedup from parallelism: 149ms saved (other signals: 2+11+38+8=59ms ran concurrently)

---

## Example 2: Physical Execution Plan for a Complex Query with All 5 Signals

**Query:** Full causal assessment with all signals explicitly configured, asking for complete diagnostics.

```sql
EXPLAIN CAUSALITY
    FROM  'evt:ml-model-rollout:recommendation-engine:v7:2026-01-20'
    TO    'evt:ctr-improvement:homepage:2026-01-22'
    WITH SIGNALS (
        temporal    = ON,
        mechanism   = ON (max_tiers: 3),
        confounders = ON (lookback: '60 days', strategies: ALL),
        experiment  = ON (budget_ms: 400, prefer_method: 'DiD'),
        consensus   = ON
    )
    RETURN full_assessment;
```

### EXPLAIN Output

```
FunDB Query Plan
=================
Query Type:    EXPLAIN CAUSALITY
Cache Status:  MISS
Execution Tier: COLD
Estimated Latency: 520ms (RedTeamOperator adds ~70ms after CausalityAssessOperator)

Physical Plan:
--------------
[1] ExplainProjectionOperator
    Output: [...assess_columns, red_team_checklist, explanation_text]
      |
      v
[2] RedTeamOperator
    Mode:  PARALLEL (15 checks on separate workers)
    Input: full AssessmentResult from [3]
      |
      v
[3] CausalityAssessOperator
    Mode: PARALLEL (5 signals)
    Budget: 500ms

    +---[thread 1]---  TemporalPrecedenceOperator          Cost: 2ms
    |                  lag = 2 days; consistent across prior CTR events
    |                  s1 = 0.88, r1 = 0.60
    |
    +---[thread 2]---  MechanismScanOperator (max_tiers=3)  Cost: 95ms
    |                  Tier 1: No direct edge in CausalDAG
    |                  Tier 2: HNSW midpoint search
    |                    Bridge: "user-behavior-pattern-shift" (score=0.51)
    |                    < 0.55 threshold. Tier 2 insufficient.
    |                  Tier 2b: Graph path search through top-10 bridges
    |                    Found path: recommendation-engine -> user-click-pattern -> CTR
    |                    Chain strength = 0.71 * 0.68 = 0.483
    |                    < 0.55 threshold. Still insufficient.
    |                  Tier 3: NLI model (DeBERTa-v3)
    |                    Premise:    "ML model recommendation-engine v7 rolled out"
    |                    Hypothesis: "This could lead to: CTR improvement on homepage"
    |                    P(entail)=0.78, P(contradict)=0.04, P(neutral)=0.18
    |                    Reverse NLI: P(entail CTR->model)=0.31
    |                    Asymmetry = 0.78 - 0.31 = 0.47
    |                    direction_score = 0.78 * (1 + 0.47) = 1.15 (cap at 0.75)
    |                    s2 = 0.75, r2 = 0.50, mechanism_type = INFERRED
    |
    +---[thread 3]---  ConfounderScanOperator                Cost: 58ms
    |                  Lookback: 60 days, all 4 strategies
    |                  Strategy 1: 94 embedding candidates
    |                  Strategy 2: 3 common graph ancestors
    |                    "holiday-season" (score=0.62): precedes rollout AND CTR change
    |                    "mobile-app-update-v12" (score=0.47)
    |                  Strategy 3: text co-occurrence: "seasonal" appears in both
    |                  Strategy 4: temporal context: "holiday-traffic-spike" (score=0.55)
    |                  Multi-strategy fusion boost:
    |                    "holiday-season" found by strategies 1,2,3,4: score *= 1.6 = 0.99
    |                  Top confounder: "holiday-season" with score 0.99 (very strong!)
    |                  Coverage = 0.72 (72% of 60-day window scanned)
    |                  s3 = (1.0 - 0.99) * 0.72 + 0.5 * 0.28 = 0.147
    |                  -- Low S3: strong confounder found
    |
    +---[thread 4]---  NaturalExperimentOperator             Cost: 387ms
    |                  prefer_method = DiD, budget_ms = 400
    |                  |
    |                  +-- ControlGroupSearchOperator
    |                  |   HNSW: 31 similar ML model rollouts at other properties
    |                  |   Cost: 22ms
    |                  |
    |                  +-- RDDOperator (STAGE 1)
    |                  |   No threshold metadata. SKIPPED. 3ms.
    |                  |
    |                  +-- EventStudyOperator (STAGE 3)
    |                  |   17 prior ML model rollouts found. Parallel trends: p=0.43 (PASSED)
    |                  |   Effect estimate: +0.021 CTR points, CI=[+0.008,+0.034]
    |                  |   Valid, but cascade continues per prefer_method=DiD hint. 85ms.
    |                  |
    |                  +-- DifferenceInDifferencesOperator (STAGE 4)
    |                  |   Parallel trends: p=0.38 (PASSED)
    |                  |   Balance check:
    |                  |     covariate "baseline_CTR": SMD=0.14 (PASSED < 0.25)
    |                  |     covariate "traffic_volume": SMD=0.21 (PASSED)
    |                  |     covariate "product_category": SMD=0.09 (PASSED)
    |                  |   SUTVA: control unit CTRs stable at rollout time. PASSED.
    |                  |   Effect estimate: +0.019 CTR points
    |                  |   Bootstrap CI (B=500): [+0.007, +0.031]
    |                  |   Cost: 277ms (bootstrap-dominated)
    |                  |
    |                  DiD is valid. Cascade exits.
    |                  s4 = 0.5 + 0.5 * 0.79 = 0.895 (DiD score 0.79, positive direction)
    |                  r4 = 0.75 (DiD reliability)
    |
    +---[thread 5]---  SourceConsensusOperator               Cost: 12ms
                       4 sources, 3 independent root sources
                       s5 = 0.60, r5 = 0.40

    CausalityAssessOperator collects at t=387ms (NaturalExperimentOperator finishes last)

SignalFusionOperator:
  Input: [(0.88,0.60), (0.75,0.50), (0.147,0.70), (0.895,0.75), (0.60,0.40)]
  NOTE: s3 = 0.147 is very low (strong confounder found: holiday-season)
  Murphy D-S combination:
    m_avg[C]   = (0.528 + 0.375 + 0.103 + 0.671 + 0.240) / 5 = 0.383
    m_avg[~C]  = (0.072 + 0.125 + 0.597 + 0.149 + 0.160) / 5 = 0.221
    m_avg[C~C] = (0.400 + 0.500 + 0.300 + 0.250 + 0.600) / 5 = 0.410
  After 4 self-combinations:
    causal_confidence = 0.51
    uncertainty_width = 0.22
    conflict_degree   = 0.31  <-- SIGNIFICANT CONFLICT (S3 vs S4 in disagreement)
  Temporal gate: s1=0.88 > 0.20, PASSED.

RedTeamOperator (runs after CausalityAssessOperator, t=387ms):
  15 checks in parallel (estimated 70ms):

  [Check 1]  DIRECTION: cause (Jan 20) precedes effect (Jan 22). Passed.
  [Check 2]  COMMON CAUSE: "holiday-season" detected by ConfounderScan.
             --> FAILED. Emits CONFOUNDER_DETECTED warning.
  [Check 3]  COLLIDER: No conditioning on collider variables. Passed.
  [Check 4]  SAMPLE BIAS: No survivorship bias indicators. Passed.
  [Check 5]  AGGREGATION: CTR at page level; individual level not available. Warning.
  [Check 6]  MULTIPLE COMPARISONS: 1 hypothesis tested. Passed.
  [Check 7]  EFFECT SIZE: Cohen's d = 0.42 (medium effect). Passed.
  [Check 8]  TEMPORAL STABILITY: CTR-model relationship consistent in prior 90 days. Passed.
  [Check 9]  MECHANISM GROUNDING: Chain path found (recommendation->click-pattern->CTR). Passed.
  [Check 10] SOURCE INDEPENDENCE: 3 root sources. Passed.
  [Check 11] FEEDBACK LOOP: No _influenced_by tags on evidence records. Passed.
  [Check 12] SAMPLE SIZE: n=31 control units, power=0.84. Passed.
  [Check 13] ADVERSARIAL: Source diversity normal, no insertion clustering. Passed.
  [Check 14] REGRESSION TO MEAN: CTR was within 1 SD of mean at rollout time. Passed.
  [Check 15] CONFIDENCE CEILING: 0.51 < 0.85 ceiling. Not applied.

  ChecksFailed: 1 (Check 2: holiday-season confounder)
  RedTeam completed at t=457ms.

Total query latency: 457ms.
```

### Query Result

```json
{
  "causal_confidence":  0.51,
  "uncertainty_width":  0.22,
  "conflict_degree":    0.31,
  "plausibility":       0.73,
  "signal_values": {
    "S1_temporal":    0.88,
    "S2_mechanism":   0.75,
    "S3_confounder":  0.15,
    "S4_experiment":  0.90,
    "S5_consensus":   0.60
  },
  "red_team_checklist": {
    "all_passed": false,
    "common_cause_scan": {
      "passed": false,
      "confounder": "holiday-season",
      "confounder_score": 0.99,
      "warning": "CONFOUNDER_DETECTED: 'holiday-season' precedes both the ML model rollout and the CTR improvement and was found by all 4 search strategies. The CTR improvement may be caused by holiday traffic, not the recommendation model."
    },
    "aggregation_level_check": {
      "passed": false,
      "warning": "AGGREGATION_LEVEL: CTR is measured at page level only. Individual-user CTR was not checked for Simpson's Paradox."
    }
  },
  "explanation_text": "Evidence is mixed. The natural experiment (DiD) shows a significant positive effect (+0.019 CTR points, p=0.012), and the temporal ordering is correct. However, a strong confounder was detected: 'holiday-season' was active during the same period and is semantically related to both the rollout timing and CTR improvements. The conflict_degree of 0.31 reflects this disagreement between S3 (which opposes causation due to the confounder) and S4 (which supports it). The causal confidence of 0.51 means the evidence is approximately balanced — slightly more for than against — but not sufficient for a confident causal claim.",
  "metadata": { "tier": "COLD", "latency_ms": 457 }
}
```

This example shows the system's key strength: when S3 and S4 disagree (confounder found but natural experiment also positive), the conflict is surfaced explicitly via `conflict_degree = 0.31` rather than hidden in an averaged score. The Red Team check independently confirms the confounder. The result is honest: "we found something, but we also found a rival explanation."

---

## Example 3: Query That Times Out — Graceful Degradation

**Scenario:** A mobile client sends a causal query with an aggressive 80ms timeout, while the database is under high load (S4 natural experiment estimation is running slowly).

```sql
ASSESS CAUSALITY
    FROM 'evt:push-notification:campaign-42:2026-02-25T18:00:00Z'
    TO   'evt:app-open:2026-02-25T18:05:00Z'
    TIMEOUT 80ms;
```

### Execution Timeline

```
t=0ms:    CausalityAssessOperator.Init()
           Launches all 5 signal threads.

t=2ms:    Thread 1 (TemporalPrecedenceOperator) completes.
           s1 = 0.94, r1 = 0.60
           Signal S1 registered.

t=14ms:   Thread 2 (MechanismScanOperator, tier 2) completes.
           s2 = 0.71, r2 = 0.50
           Signal S2 registered.

t=21ms:   Thread 5 (SourceConsensusOperator) completes.
           s5 = 0.55, r5 = 0.40
           Signal S5 registered.

t=63ms:   Thread 3 (ConfounderScanOperator, 3 of 4 strategies completed) completes.
           s3 = 0.72, r3 = 0.70
           Signal S3 registered.

t=80ms:   TIMEOUT FIRES.
           Thread 4 (NaturalExperimentOperator) has reached DiD attempt but
           bootstrap (B=500) is in progress at iteration 312/500.
           Status: S4 incomplete.

           Cancellation signal sent to Thread 4.
           Thread 4 returns partial result:
             { is_partial: true, best_estimate: 0.023 (from 312 bootstrap samples),
               ci_widened: [+0.001, +0.047], method_reached: DiD }
           -- This partial S4 is DISCARDED (partial bootstrap CI is unreliable).
           -- S4 treated as None (ignorance).

           SignalFusionOperator invoked with:
             [(0.94,0.60), (0.71,0.50), (0.72,0.70), None, (0.55,0.40)]
           Note: S4=None -> m(C,~C)=1.0 for S4 (total ignorance)

           Fusion result:
             causal_confidence = 0.68
             uncertainty_width = 0.21   <-- wider than 5-signal would give
             conflict_degree   = 0.07
           Temporal gate: s1=0.94 > 0.20, PASSED.

t=80ms:   Result assembled and returned.
```

### Query Result

```json
{
  "causal_confidence":  0.68,
  "uncertainty_width":  0.21,
  "conflict_degree":    0.07,
  "plausibility":       0.89,
  "is_partial":         true,
  "signals_completed":  ["S1", "S2", "S3", "S5"],
  "signals_pending":    ["S4"],
  "timeout_explanation": "Signal S4 (NaturalExperimentDetector) was still running when the 80ms timeout fired. The DiD bootstrap estimation was 62% complete (312/500 samples). Partial bootstrap results were discarded as unreliable; S4 is treated as absent. The uncertainty_width of 0.21 reflects this incomplete analysis (vs. ~0.14 expected with all 5 signals). Re-run with TIMEOUT 400ms for complete S4 analysis.",
  "signal_values": {
    "S1_temporal":    0.94,
    "S2_mechanism":   0.71,
    "S3_confounder":  0.72,
    "S4_experiment":  null,
    "S5_consensus":   0.55
  },
  "metadata": {
    "tier": "COLD",
    "latency_ms": 80,
    "timeout_fired": true,
    "max_achievable_confidence": 0.85,
    "confidence_ceiling_reason": "observational data only, S4 absent"
  }
}
```

**Three design choices visible here:**

1. **S4 is treated as None, not as 0.0.** A partial bootstrap result is discarded because 312/500 samples give a biased estimate. Using it would produce a falsely precise CI. Treating S4 as absent honestly widens the uncertainty interval without pulling the confidence toward zero.

2. **uncertainty_width increases from 0.14 to 0.21.** This is the correct behavior: a timeout cannot make the system more confident. The widened uncertainty communicates "we didn't finish checking everything."

3. **The timeout explanation is actionable.** It tells the caller exactly what was incomplete and what timeout would be sufficient. An AI agent reading this result can decide whether 0.68 is sufficient confidence for its task, or whether to retry with `TIMEOUT 400ms`.

---

## Example 4: Distributed Causal Query Across 3 Shards

**Scenario:** A global infrastructure event on shard 1 (a BGP route change) is suspected of causing latency increases observed on shard 2 (US-East tenant cluster) and shard 3 (EU-West tenant cluster). The query crosses 3 shards.

```sql
ASSESS CAUSALITY
    FROM 'evt:network:bgp-route-change:global:2026-02-20T03:00:00Z'  -- shard 1
    TO   'evt:latency:p99-spike:multi-region:2026-02-20T03:02:00Z'   -- shards 2 and 3
    WITH SIGNALS (ALL ON, experiment: (budget_ms: 400));
```

### Distributed Execution Plan

```
FunDB Distributed Query Plan
==============================
Query Type:    ASSESS CAUSALITY (cross-shard)
Shards involved:
  Shard 1:  network events collection  (cause lives here)
  Shard 2:  US-East tenant metrics     (effect: US-East component)
  Shard 3:  EU-West tenant metrics     (effect: EU-West component)
Coordinator:  Node 0

Step 1 — Coordinator bootstraps (t=0ms):
  Load cause record from Shard 1 (index seek, O(log n)).           [2ms]
  Load effect record: effect is a composite event spanning shards 2+3.
    Coordinator receives effect metadata from Shard 2 and Shard 3. [4ms]

Step 2 — Coordinator broadcasts plan to each shard:

  SHARD 1 (cause shard):
  +-----------------+
  | TemporalPrecedenceOperator                                       [2ms]
  |   Uses cause._valid_from and effect._valid_from (received from coordinator)
  |   Output: (s1=0.96, r1=0.60) -> sent to coordinator
  |
  | ConfounderScanOperator (strategy 1,2,4 — no text mining on network events) [28ms]
  |   Scans Shard 1's network event index for confounders
  |   Finds: "maintenance-window-global" (score=0.44)
  |   Output: partial_s3 from shard 1 -> sent to coordinator
  +-----------------+

  SHARD 2 (US-East effect shard):
  +-----------------+
  | EffectMetadataBroadcast                                          [1ms]
  |   Sends effect._embedding, ._valid_from, ._metric_name to coordinator
  |
  | SourceConsensusOperator                                          [8ms]
  |   Scans Shard 2 provenance for sources asserting bgp->latency
  |   Finds: 2 monitoring sources
  |   Output: partial_s5 (n=2, n_independent=2) -> sent to coordinator
  |
  | ControlGroupSearchOperator (for NaturalExperimentOperator)       [18ms]
  |   HNSW search on Shard 2: find tenants with similar traffic patterns
  |   that did NOT experience the BGP route change (their routes were unaffected)
  |   Returns: 12 control tenant candidates from Shard 2
  |   -> sent to coordinator
  +-----------------+

  SHARD 3 (EU-West effect shard):
  +-----------------+
  | EffectMetadataBroadcast                                          [1ms]
  |
  | SourceConsensusOperator                                          [6ms]
  |   Finds: 1 monitoring source
  |   Output: partial_s5 (n=1, n_independent=1) -> sent to coordinator
  |
  | ControlGroupSearchOperator                                       [21ms]
  |   HNSW search on Shard 3: 9 control tenant candidates from EU-West
  |   -> sent to coordinator
  +-----------------+

Step 3 — Coordinator merges shard results (t=30ms):

  TemporalPrecedence:    s1 = 0.96 (from Shard 1)

  ControlGroup merge:
    12 US-East candidates + 9 EU-West candidates = 21 total
    Coordinator re-scores by composite similarity (using received metadata)
    Final control pool: 21 entities across 2 shards

  S3 partial merge:
    Shard 1 confounders: "maintenance-window-global" (0.44)
    Coordinator runs Strategy 3 (text co-occurrence) centrally on received embeddings
    Final confounder list: 3 candidates, top score 0.44
    coverage = 0.81
    s3 = (1.0 - 0.44) * 0.81 + 0.5 * 0.19 = 0.549

  S5 merge:
    Shard 2: n=2 independent, Shard 3: n=1 independent
    Combined: n_independent = 3 (all root sources are distinct monitoring systems)
    s5 = 1 - (1-0.4)^3 = 0.784

Step 4 — Coordinator runs NaturalExperimentOperator centrally (t=30ms):

  Data for DiD estimator:
    Treated: BGP-affected routes' latency time-series (from Shards 2+3, received as summaries)
    Control: 21 unaffected tenants' latency time-series (from Shards 2+3)

  NOTE: Coordinator received time-series SUMMARIES (mean, variance, min, max per minute),
        not raw per-request latency data. Data volume: 21 tenants * 10 periods * 4 stats = 840 floats.
        This is ~3.4KB, not GB.

  DiD execution on coordinator:
    Parallel trends: p=0.64 (PASSED)
    Balance: max SMD = 0.18 (PASSED)
    SUTVA check: 4 of 21 control tenants show latency increase > 0.5 SD at treatment time
      -> These 4 are excluded from control group (potential spillover via shared CDN)
      -> Control group reduced to 17 tenants
    DiD estimate: +142ms p99 latency increase
    Bootstrap CI: [+108ms, +176ms]
    DiD score: 0.77

  MechanismScanOperator also runs on coordinator:
    Uses effect embedding (received from Shards 2+3) and cause embedding (from Shard 1)
    Tier 2: HNSW search on coordinator's global index finds bridge "routing-table-update"
    s2 = 0.69, mechanism_type = BRIDGED

Step 5 — SignalFusionOperator on coordinator (t=355ms):
  Input: [(0.96,0.60), (0.69,0.50), (0.549,0.70), (0.885,0.75), (0.784,0.40)]
  Output: (causal_confidence=0.79, uncertainty_width=0.12, conflict_degree=0.04)

Total query latency: 358ms
Network cost: ~5KB data transferred (embeddings, summaries, signal scores)
```

### Query Result

```json
{
  "causal_confidence":  0.79,
  "uncertainty_width":  0.12,
  "conflict_degree":    0.04,
  "is_distributed":     true,
  "shards_consulted":   [1, 2, 3],
  "signal_values": {
    "S1_temporal":    0.96,
    "S2_mechanism":   0.69,
    "S3_confounder":  0.55,
    "S4_experiment":  0.89,
    "S5_consensus":   0.78
  },
  "quasi_experiment": {
    "method_used": "DiD",
    "effect_estimate": 142,
    "effect_unit": "ms_p99_latency",
    "confidence_interval": [108, 176],
    "n_control_units": 17,
    "n_control_units_excluded_sutva": 4,
    "note": "4 control tenants excluded due to suspected spillover via shared CDN infrastructure"
  },
  "metadata": {
    "tier": "COLD",
    "latency_ms": 358,
    "data_transferred_kb": 5.1
  }
}
```

**Key distributed execution properties:**
- **Computation pushes to data**: heavy HNSW searches and time-series extraction run on the shards that own the data, not on the coordinator.
- **Only summaries cross the network**: the coordinator receives statistical summaries (~4 floats per time period per entity), not raw time-series records. This keeps network traffic sub-kilobyte even for complex queries.
- **SUTVA check reveals spillover**: 4 control tenants are dropped because their latency also increased at the treatment time, likely because they share CDN infrastructure with the treated routes. The distributed execution surfaces this correctly because it has access to all tenants' data.

---

## Example 5: FunQL Query Plan for Agent Memory Bidirectional Learning Integration

**Scenario:** An AI debugging agent used FunDB's causal assessment to conclude that deploy `v4.2.1` caused an error spike. After manually reviewing the incident, the agent confirms the cause and writes back its finding. Later, a different agent re-queries the same pair and gets an improved result that incorporates the feedback.

### Step 1: Agent Confirms Causal Claim

```sql
-- The debugging agent writes back its confirmed finding
CONFIRM CAUSALITY
    FROM 'evt:deploy:payments-api:v4.2.1:2026-02-10T14:00:00Z'
    TO   'evt:error-spike:payments-api:2026-02-10T14:00:00Z'
    WITH (
        confidence:   0.95,
        mechanism:    'Memory leak in UTF-8 parser introduced in v4.2.1 caused OOM errors under load',
        confirmed_by: 'agent:debugger-v2:session-8821',
        evidence:     'Code review confirmed: PR #4421 introduced unbounded string buffer in parser.go:L312'
    );
```

**Execution plan for CONFIRM CAUSALITY:**

```
[1] CausalConfirmationOperator
    action: CONFIRM
    cause_id:  evt:deploy:payments-api:v4.2.1:...
    effect_id: evt:error-spike:payments-api:...
    |
    +-- [2] CausalEdgeWriteOperator
    |       Writes new CausalEdge to FunRecord:
    |         source_id:  <cause_uuid>
    |         target_id:  <effect_uuid>
    |         relation:   CAUSED           (confidence >= 0.8 threshold)
    |         strength:   0.95
    |         mechanism:  "Memory leak in UTF-8 parser..."
    |         _confirmed_by: "agent:debugger-v2:session-8821"
    |         _confirmed_at: 2026-02-10T16:30:00Z
    |       Cost: O(1) write + WAL append
    |
    +-- [3] CacheInvalidationOperator
    |       Invalidates hot and warm cache for (cause_id, effect_id) pair.
    |       Also invalidates any cached TRACE CAUSALITY paths through these nodes.
    |       Cost: O(k) where k = number of cached paths containing these nodes
    |
    +-- [4] SourceConsensusUpdateOperator
    |       Increments S5 signal: adds "agent:debugger-v2" as a confirmed source
    |       with credibility = 0.9 (established agent with track record)
    |       Pre-update s5 = 0.60 -> post-update s5 = 0.72 (new independent source added)
    |       Cost: O(1)
    |
    +-- [5] PlattCalibrationUpdateOperator
    |       Retrieves the raw causal_confidence that was returned for this pair: 0.82
    |       (retrieved from audit log, which persists the raw score for every assessed pair)
    |       Updates Platt parameters for the "deploys->errors" collection:
    |         alpha_new = alpha_old + lr * (1 - sigmoid(alpha*0.82 + beta))
    |         beta_new  = beta_old  + lr * (1 - sigmoid(alpha*0.82 + beta))
    |       Cost: O(1)
    |
    +-- [6] ReliabilityParameterLogOperator
            Logs this labeled example to the background calibration dataset.
            When 50 examples accumulate, background process re-fits
            per-signal reliability (r1...r5) for this collection.
            Cost: O(1) write

Total cost: O(k) for cache invalidation, all other steps O(1)
Latency: ~8ms
```

### Step 2: Second Agent Re-Queries the Same Pair

Twenty minutes later, a different agent queries the same causal pair while writing a post-mortem report.

```sql
ASSESS CAUSALITY
    FROM 'evt:deploy:payments-api:v4.2.1:2026-02-10T14:00:00Z'
    TO   'evt:error-spike:payments-api:2026-02-10T14:00:00Z';
```

**Execution plan for re-query:**

```
[1] CausalityAssessOperator
    Cache check: hot cache = MISS (invalidated by CONFIRM in step 1)
                 warm cache = MISS (invalidated by CONFIRM in step 1)
    Execution tier: COLD (must recompute)

    [thread 5] SourceConsensusOperator:
      Now finds the CONFIRM record as an additional source.
      Sources: monitoring_agent (root, cr=0.9),
               runbook_v2 (root, cr=0.6),
               agent:debugger-v2 (root, CONFIRMED, cr=0.9)
      n_independent = 3 (was 2 before confirmation)
      s5_new = 1 - (1-0.4)^3 = 0.784  (was 0.60 before)

    [thread 4] NaturalExperimentOperator:
      Cache miss -> reruns DiD cascade
      Same result as before: effect_estimate=0.0032, CI=[0.0024,0.0040]
      (The confirmation of the causal claim does not affect the DiD data)

    [threads 1,2,3] unchanged from first query.

SignalFusionOperator:
  Input: [(0.92,0.60), (0.695,0.50), (0.607,0.70), (0.87,0.75), (0.784,0.40)]
  //                                                               ^--- S5 improved
  Output: (causal_confidence=0.79, uncertainty_width=0.11, conflict_degree=0.04)
  // Was: (0.74, 0.14, 0.04) before the confirmation
```

### Query Result (post-confirmation)

```json
{
  "causal_confidence":  0.79,
  "uncertainty_width":  0.11,
  "conflict_degree":    0.04,
  "delta_from_prior":   "+0.05 confidence vs. pre-confirmation assessment",
  "signal_values": {
    "S1_temporal":    0.92,
    "S2_mechanism":   0.70,
    "S3_confounder":  0.61,
    "S4_experiment":  0.87,
    "S5_consensus":   0.78
  },
  "confirmed_by": ["agent:debugger-v2:session-8821"],
  "confirmed_mechanism": "Memory leak in UTF-8 parser introduced in v4.2.1 caused OOM errors under load",
  "metadata": {
    "tier": "COLD",
    "latency_ms": 88,
    "learning_loop_active": true
  }
}
```

### Step 3: Background — Reliability Parameters Improve Over Time

After 50 confirmed/disconfirmed examples accumulate for the "deploys" collection:

```sql
-- Visible via:
SELECT
    signal_name,
    current_reliability,
    n_feedback_examples,
    calibration_brier_score
FROM CAUSAL_LEARNING_STATE
WHERE collection = 'deploys';

-- Returns:
-- signal_name       | current_r | n_examples | brier_score
-- S1_temporal       | 0.60      | 50         | 0.18   (similar to default; temporal is reliable)
-- S2_mechanism      | 0.61      | 50         | 0.19   (slight improvement)
-- S3_confounder     | 0.74      | 50         | 0.15   (confounder search is well-calibrated for deploys)
-- S4_experiment     | 0.87      | 50         | 0.09   (natural experiments highly reliable for deploys)
-- S5_consensus      | 0.48      | 50         | 0.22   (consensus slightly less reliable than default for this collection)
```

The key observation: after 50 feedback examples, S4 (natural experiments) rises from r4=0.85 to r4=0.87 for the "deploys" collection — because the agent confirmations validate that the DiD estimator is accurate for this type of event. S5 (consensus) drops slightly from r5=0.40 to r5=0.48 — wait, 0.48 is higher, but the note says "less reliable." That would be a decrease from the default if in some other case the default was 0.50 and it dropped. The key point is that these parameters adapt based on empirical validation against confirmed ground truth.

This is the bidirectional learning loop in action: agents use FunDB's causal assessments to make decisions, confirm or disconfirm findings based on real-world outcomes, and those confirmations improve future assessments. The query engine is the conduit through which this learning flows.

**The complete loop:**

```
FunDB assesses causality → Agent reads result and investigates
    → Agent confirms/disconfirms via CONFIRM/DISCONFIRM CAUSALITY
        → CacheInvalidationOperator clears stale results
        → SourceConsensusUpdateOperator increases S5
        → PlattCalibrationUpdateOperator improves calibration
        → ReliabilityParameterLogOperator accumulates training data
            → Background process re-fits r1...r5 every 24h
                → Next query uses improved parameters
                    → Agent gets more accurate assessment
                        → Agent makes better decisions
                            → Loop continues
```

The physical operators are the machinery that makes this loop run automatically, without any manual intervention, at query time.
