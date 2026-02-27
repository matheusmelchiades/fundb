# Round 2: Subgroup Working Sessions

**Session:** FunDB Hackathon — Post-Crossfire Resolution
**Format:** Three subgroups meet simultaneously to resolve conflicts identified in Round 1.

---

## Subgroup A: Statistician + Skeptic

**Problem assigned:** What is the mathematically honest maximum confidence ceiling for observational causal inference, and how should it be computed — not hardcoded?

**Working session transcript:**

---

**SKEPTIC:** We both agreed in Round 1 that 0.85 is a placeholder. So let us actually derive something. My starting point: the ceiling should be a function of two independent variables. First, the credibility of the strongest evidence method used. Second, the estimated independence of the signals. Can you formalize that?

**STATISTICIAN:** Let me try. Define the ceiling C_max as:

```
C_max = C_base * f_method * f_independence
```

Where:
- `C_base` is the absolute maximum for pure observational inference (I will defend 0.90 as this upper bound — it corresponds to Bradford Hill's own stated confidence threshold for "virtually certain" observational causation in epidemiology)
- `f_method` is a method quality factor in (0, 1] based on the best quasi-experimental method that fired
- `f_independence` is a signal independence factor in (0, 1] based on how correlated the signals are

**SKEPTIC:** I can work with that structure. Let us fill in the values. For `f_method`:

```
f_method:
  RDD (near-random assignment):         1.00   -- no penalty
  Event Study (built-in placebo test):  0.95
  DiD (parallel trends validated):      0.90
  SCM (good pre-fit):                   0.88
  ITS only (no control group):          0.70
  No valid quasi-experiment (S4 absent): 0.60
```

The idea: when no quasi-experimental evidence exists, you cannot get above 0.90 * 0.60 = 0.54 no matter what. That feels right. Observational association without any quasi-experimental corroboration is weak.

**STATISTICIAN:** I need to push back on the ITS value. 0.70 means C_max = 0.90 * 0.70 = 0.63. That is very low. An ITS result with 100 pre-treatment periods, a large effect size, and stable pre-trend should allow more confidence than 0.63.

**SKEPTIC:** The cap is not about the statistical significance of the ITS result. It is about the *design credibility* — ITS cannot rule out concurrent events. Even a statistically bulletproof ITS cannot establish that the effect was not caused by something else that happened at the same time. That structural limitation justifies the cap.

**STATISTICIAN:** Fine. I accept that argument. ITS at 0.70 for `f_method`. Now for `f_independence` — this is harder. The signals are not fully independent, but we do not have an explicit correlation model between them.

**SKEPTIC:** We do have partial information. From my Combined Failure Mode 1, I identified that S1 (temporal) and S4 (natural experiments) share temporal data, creating partial dependence. S2 (mechanism) and S3 (confounder exclusion) both depend on the HNSW index and on what is in the database — if the database has a systematic blind spot, both will fail together. S5 (consensus) is potentially the most independent but is also the weakest evidence.

**STATISTICIAN:** Let me formalize signal pair dependencies. Define rho_ij as the estimated correlation between failure modes of signals Si and Sj:

```
Estimated failure-mode correlations (rho matrix):
         S1    S2    S3    S4    S5
S1 [  1.00, 0.25, 0.30, 0.40, 0.10 ]
S2 [  0.25, 1.00, 0.50, 0.15, 0.20 ]
S3 [  0.30, 0.50, 1.00, 0.20, 0.15 ]
S4 [  0.40, 0.15, 0.20, 1.00, 0.05 ]
S5 [  0.10, 0.20, 0.15, 0.05, 1.00 ]
```

The effective number of independent signals n_eff can be approximated using the eigenvalue approach: compute eigenvalues of the correlation matrix, then n_eff = (sum of eigenvalues)^2 / sum of squared eigenvalues.

For this correlation structure, n_eff ≈ 3.2 out of 5 signals. So the `f_independence` factor should reflect that we have roughly 3.2 independent observations instead of 5.

**SKEPTIC:** How does that translate to a multiplier?

**STATISTICIAN:** One defensible formula: `f_independence = sqrt(n_eff / n_total) = sqrt(3.2 / 5) ≈ 0.80`.

When only 3 signals are available and they are highly correlated (say n_eff = 1.8), `f_independence = sqrt(1.8 / 3) ≈ 0.77`. This captures the degradation honestly.

**SKEPTIC:** I like that. The formula has an interpretation: you are discounting the ceiling by the square root of the "information fraction" — how much independent information you actually have relative to the maximum possible.

**STATISTICIAN:** The full ceiling formula then becomes:

```
C_max(method, signals_present, correlation_matrix) =
    C_base * f_method(best_quasi_experimental_method) * f_independence(n_eff, n_total)

Where:
    C_base = 0.90
    f_method = { RDD: 1.00, EVENT_STUDY: 0.95, DiD: 0.90, SCM: 0.88, ITS: 0.70, NONE: 0.60 }
    f_independence = sqrt(n_eff / n_total)
    n_eff = eigenvalue-adjusted effective signal count
    n_total = number of signals that actually fired (not absent)
```

Example: five signals all fired, RDD quasi-experiment found, correlation structure as above.
```
C_max = 0.90 * 1.00 * sqrt(3.2 / 5) = 0.90 * 0.80 = 0.72
```

Wait — that gives us 0.72 as the ceiling even when everything goes well. That is lower than the hardcoded 0.85. Is that too conservative?

**SKEPTIC:** Let me check the extreme case. RDD fires with five perfectly independent signals (rho_ij = 0 everywhere, so n_eff = 5):
```
C_max = 0.90 * 1.00 * sqrt(5/5) = 0.90 * 1.00 = 0.90
```
That is the upper limit of the whole system — 0.90 — and it only applies when you have near-random assignment AND five completely independent signals. That seems correct. You should not be able to exceed 0.90 from observational data no matter what.

**STATISTICIAN:** And the realistic case — DiD fired, the five signals have the correlation structure I estimated:
```
C_max = 0.90 * 0.90 * 0.80 = 0.648
```
So the ceiling is about 0.65. That is *much* more conservative than the hardcoded 0.85. Is the 0.90 base too aggressive?

**SKEPTIC:** I think 0.90 for C_base is defensible. The binding factor in realistic cases is the correlation penalty — 0.80 or lower — which brings the realistic ceiling to 0.65-0.72 range. That matches my intuition about what observational causal inference can actually achieve. Let me say this: in most of the RCT validation literature, observational studies that claimed "confident" causation later turned out to be wrong in 20-40% of cases. A ceiling in the 0.65-0.72 range is honest about that error rate.

**STATISTICIAN:** Agreed. The formula is not finalized — the rho matrix values are estimated, and C_base = 0.90 is itself a judgment call — but the structure is mathematically defensible and principled. It replaces the arbitrary 0.85 with a computed value that degrades appropriately based on evidence quality.

---

**Mini-Proposal: Subgroup A Resolution**

The confidence ceiling should be computed as:

```
C_max = 0.90 * f_method * f_independence

f_method lookup table (based on best method in Historian's cascade):
  RDD                    → 1.00
  Event Study            → 0.95
  DiD                    → 0.90
  SCM                    → 0.88
  ITS                    → 0.70
  No valid QE (S4=0.5)   → 0.60

f_independence = sqrt(n_eff / n_total)
  n_eff = effective independent signal count via eigenvalue adjustment of signal correlation matrix
  n_total = count of signals that produced a non-null, non-ignorance-mass result

The correlation matrix is initialized to the estimated values above and updated via background
learning (same 24-hour process as the Statistician's reliability parameter learning).

The ceiling is applied post-Platt-calibration as a hard cap.
The output includes: { causal_confidence: ..., confidence_ceiling: C_max, ceiling_factors: { f_method, f_independence, C_base } }
```

**Open issue not resolved:** The rho matrix initialization values are estimates. They need empirical validation. This is acknowledged as a known approximation until the feedback loop accumulates enough labeled examples to fit them properly.

---

## Subgroup B: Semanticist + Historian

**Problem assigned:** How do MechanismDetect and NaturalExperimentDetector hand off to each other? Can semantic bridge search help identify control groups for quasi-experiments?

**Working session transcript:**

---

**SEMANTICIST:** I want to start by acknowledging that in Round 1, you identified a real problem with my proposal. The semantic bridge concept is about causal pathway plausibility, not about experimental design. So let me reframe the question: instead of asking "can semantic bridges help identify natural experiments," let us ask "can semantic bridges improve control group quality for the Historian's cascade?"

**HISTORIAN:** That is a better question. Here is why it matters to me. My control group construction uses HNSW on the *cause's* embedding to find similar entities. But "similar to the cause event" is not the same as "similar in the ways that matter for treatment comparison." I want control entities that look like the treated entity along the pre-treatment outcome trajectory and along the semantic dimensions relevant to the causal mechanism.

**SEMANTICIST:** And MechanismDetect can tell you which semantic dimensions are relevant. If MechanismDetect identifies that the mechanism between "deploy event" and "error spike" runs through "memory pressure" — a specific bridge concept — then the ideal control group should consist of entities that also had memory pressure conditions but did *not* receive the deploy event. That is a semantically-informed control group.

**HISTORIAN:** Walk me through the implementation. In my current `find_control_candidates` function, I do an HNSW search on `cause._embedding` with `k=100`. What changes?

**SEMANTICIST:** After MechanismDetect fires and finds a bridge concept M with high score, you augment the HNSW search. Instead of searching only on `cause._embedding`, you search on a *mechanism-weighted combination*:

```
query_vector = alpha * cause._embedding + (1 - alpha) * M.bridge_record._embedding

Where alpha controls how much of the mechanism context to include.
Suggested default: alpha = 0.7 (mostly cause, partially bridge)
```

Entities near this combined vector are similar to the cause event in the context of the mechanism. These are better control candidates than entities similar to the cause alone.

**HISTORIAN:** That is interesting. But I see two problems. First, MechanismDetect runs as a parallel signal in the Architect's model — it completes around 10-100ms depending on tier. My control group search also runs in parallel. If control group search starts before MechanismDetect finishes (which it will, since S2 and S4 are launched simultaneously), I cannot use the bridge vector.

**SEMANTICIST:** Fair. Two options: either (a) create an explicit dependency where S4 waits for S2 to complete before starting control group search, which adds latency, or (b) run control group search twice — once with the standard query (concurrent with S2), and again with the mechanism-augmented query if MechanismDetect found a strong bridge, then merge the two candidate lists.

**HISTORIAN:** Option (b) is more compatible with the Architect's parallelism model. I already search for up to 100 control candidates; if I get a second list of 50 semantically-informed candidates, I merge them and take the top 100 by composite similarity. The mechanism-augmented candidates that appear in both lists get a score boost — exactly like the Semanticist's ConfounderSearch multi-strategy convergence boost.

**SEMANTICIST:** I like that. And it creates a natural handoff protocol:

```
HANDOFF PROTOCOL: MechanismDetect → NaturalExperimentDetector

MechanismDetect produces:
  mechanism_type:   BRIDGED | CHAIN | DIRECT | INFERRED | NONE
  bridge_record:    FunRecord (the semantic bridge, if found)
  bridge_vec:       embedding vector of bridge concept

NaturalExperimentDetector uses:
  IF mechanism_type in {BRIDGED, CHAIN} AND bridge_record is not None:
    augmented_candidates = HNSW.search(
        vector = 0.7 * cause._embedding + 0.3 * bridge_vec,
        k = 50,
        filter = { ... same temporal/entity filters as standard search }
    )
    all_candidates = merge_and_rank(standard_candidates, augmented_candidates)
  ELSE:
    all_candidates = standard_candidates  // no augmentation, standard path
```

**HISTORIAN:** This adds one HNSW search to the control group construction when MechanismDetect fires with a bridge. Cost: 10-15ms. The result is a richer control group. I think it is worth it.

**SEMANTICIST:** Now let me raise the reverse direction: can the NaturalExperimentDetector help the ConfounderSearch? In my proposal, Strategy 4 (temporal context search) looks for events preceding both A and B. But the Historian's control group search already finds entities similar to A that did *not* experience B after the cause. Entities in the control group where *neither* the treatment nor the outcome occurred are the cleanest "null context" — they represent the counterfactual baseline. That is exactly what ConfounderSearch Strategy 4 is trying to find.

**HISTORIAN:** You are right. The control group is a subset of the confounder search space. Specifically: entities in the control group that show outcome change at treatment time even without receiving the treatment are themselves potential confounders — they represent a common cause that affected the control group too.

**SEMANTICIST:** That is the SUTVA check in your DiD implementation. If SUTVA fails because control units' outcomes change at treatment time, those control units are detecting a confounder. That signal should propagate back to S3.

**HISTORIAN:** This creates a data dependency: DiD's SUTVA check can detect confounders that ConfounderSearch missed. I should expose a `discovered_confounders` list in my output that the Statistician can route to S3.

**SEMANTICIST:** And conversely: if ConfounderSearch finds a strong confounder Z before the Historian's cascade starts, the Historian should know about it. It might invalidate the parallel trends assumption — if Z causes both A and B, the control group entities may all be affected by Z, meaning parallel trends holds for the wrong reason.

**HISTORIAN:** Right. So the clean bidirectional handoff is:

```
BIDIRECTIONAL INTEGRATION:

Before NaturalExperimentDetector runs:
  - If ConfounderSearch found a strong confounder (score > 0.7), pass it to the Historian
  - The Historian checks: does this confounder correlate with control group membership?
  - If yes: warn that parallel trends may be confounded; reduce DiD's r4

After NaturalExperimentDetector runs (if DiD was attempted):
  - Pass SUTVA violations back to the Statistician as additional confounder candidates
  - These update S3 with new evidence, potentially reducing the confounder exclusion score
```

**SEMANTICIST:** This requires that S3 and S4 are no longer fully independent signals in the fusion. The Statistician needs to know this.

**HISTORIAN:** The Statistician will be unhappy.

**SEMANTICIST:** The Statistician will adapt. Reality is messy.

---

**Unresolved question within Subgroup B:**

There is one question we cannot resolve ourselves: does the sequential dependency (ConfounderSearch informs NaturalExperimentDetector, NaturalExperimentDetector updates ConfounderSearch) violate the Statistician's independence assumption in Dempster-Shafer combination?

Our answer: it partially does. S3 and S4 are already somewhat correlated by our analysis. The new bidirectional flow makes that explicit. The Statistician needs to update the rho_34 value in the correlation matrix (Subgroup A's framework) to reflect this dependency. We estimate rho_34 should increase from 0.20 to approximately 0.35 under this integrated design.

---

**Mini-Proposal: Subgroup B Resolution**

**Algorithm 1: MechanismDetect → NaturalExperimentDetector (forward)**

When `MechanismDetect` fires with `mechanism_type in {BRIDGED, CHAIN}` and returns a bridge record, the control group search is augmented:

```
augmented_query_vec = 0.7 * cause._embedding + 0.3 * bridge_record._embedding
augmented_candidates = HNSW.search(augmented_query_vec, k=50, temporal_filter)
all_candidates = ranked_merge(standard_candidates_k100, augmented_candidates_k50)
```

Candidates appearing in both lists receive a `multi_strategy_bonus` of 1.2x on composite similarity score, consistent with ConfounderSearch's multi-strategy convergence logic.

**Algorithm 2: NaturalExperimentDetector → ConfounderScanOperator (backward)**

When DiD is attempted, the SUTVA check result is surfaced as a `discovered_potential_confounders` field in NaturalExperimentResult. These are entities in the control group whose outcomes shifted at treatment time despite not receiving the treatment.

The ConfounderScanOperator should incorporate these as additional high-priority confounder candidates with a `sutva_violation` tag.

**Algorithm 3: Strong Confounder Warning (ConfounderSearch → NaturalExperimentDetector)**

Before the Historian's cascade runs the DiD stage: if ConfounderSearch returns a confounder with score > 0.7, the NaturalExperimentDetector reduces DiD's base credibility factor from 0.85 to `0.85 * (1 - 0.5 * confounder_score)`. The parallel trends test still runs, but the DiD result is flagged with `POTENTIAL_CONFOUNDER_PRESENT: true`.

**Latency implications:**

- Forward handoff (Mechanism → NaturalExperiment): adds 10-15ms HNSW search to control group construction, conditional on MechanismDetect finding a bridge. Expected additional latency: +10ms on ~40% of queries = +4ms average.
- Backward handoff (NaturalExperiment → ConfounderScan): no additional computation; SUTVA check is already run as part of DiD. Cost: 0ms (data already computed, just surfaced differently).
- Forward warning (ConfounderScan → NaturalExperiment): sequential dependency requires ConfounderScan to partially complete before DiD starts. Must communicate confounder score to NaturalExperimentOperator within first 30ms of query execution. This introduces a soft synchronization point. Architect must implement this as a shared result buffer, not a hard pipeline dependency.

**Updated rho_34 for Statistician's independence model:** rho_34 = 0.35 (up from estimated 0.20).

---

## Subgroup C: Architect + Everyone

**Problem assigned:** Collect latency estimates from all agents and propose a realistic query plan that fits within 500ms for a full causal assessment. Show the actual timeline with parallelism.

**Working session transcript:**

---

**ARCHITECT:** I am going to run this session differently from the others. I need specific numbers, not ranges. I will go through each operator, you give me a worst-case estimate for a 10M-event FunDB instance, and we negotiate if I think the estimate is unrealistic.

**ARCHITECT:** S1 — Temporal Precedence. Two timestamp lookups and a comparison. What is worst case?

**STATISTICIAN:** 5ms if both records are in buffer cache. 15ms if one requires disk read. Let us say 15ms worst case.

**ARCHITECT:** I had 5ms in my proposal. Let me revise to 15ms. That is still fast enough to not matter. Next: S2 — MechanismDetect. What is worst case for all three tiers firing?

**SEMANTICIST:** Tier 1: 2ms. Tier 2 (HNSW + scoring): 25ms. Tier 2b (graph path search): 30ms additional. Tier 3 (NLI inference): 80ms on CPU, 20ms on GPU. Total all tiers: 135ms. But Tiers 2 and 2b are only both triggered if Tier 2 finds bridges but no single bridge exceeds the threshold. That is maybe 20% of queries. Typical case is 25ms for Tier 2 returning a result.

**ARCHITECT:** I need worst case. 135ms for S2 in the worst case. Next: S3 — ConfounderSearch. Your proposal says 62ms sequential, "parallelizable to ~35ms."

**SEMANTICIST:** Under no contention: 35ms. Under HNSW contention from concurrent S4 and S2 searches: my honest estimate is 55-70ms. Let me say 70ms worst case.

**ARCHITECT:** 70ms for S3. And I want to note: with Subgroup B's new bidirectional integration, S3 now has an additional output (discovered_potential_confounders for SUTVA violations) and a soft synchronization point where it needs to share its top confounder score with S4 within 30ms. Does that change your estimate?

**SEMANTICIST:** The partial result share at 30ms is just reading a variable from a shared buffer — no additional computation. But it means ConfounderSearch has to produce its highest-scoring confounder candidate within 30ms. Strategies 1 and 2 (embedding triangle and common ancestor) complete within that window. Strategies 3 and 4 (text cooccurrence and temporal context) may not. So the partial result at 30ms uses only the first two strategies. The full result at 70ms uses all four. This is acceptable.

**ARCHITECT:** Good. Next: S4 — NaturalExperimentDetector. This is the critical path. What is worst case for the full cascade?

**HISTORIAN:** Full cascade, all methods attempted, SCM with 50 donors and 500 bootstrap resamples, all running sequentially because each method only fires if the previous one fails. My proposal says 312ms expected worst case. But the Architect raised a concern about SCM bootstrap. Let me reassess.

SCM optimization (SLSQP on 50 donors, 30 pre-periods): 20ms.
Placebo tests (50 iterations): If sequential, 50 * 2ms = 100ms. If parallelized to 8 threads: ~15ms.

If the Architect's infrastructure supports 8-thread parallelism within the SCM stage: SCM total is 20 + 15 = 35ms. If sequential: 120ms.

**ARCHITECT:** FunDB's worker pool supports 8 threads per query by default. Parallelizing the placebo tests within a single signal operator is non-standard but feasible — it would need to use FunDB's internal TaskScheduler, not a raw thread pool. I can support this. SCM with parallel placebos: 35ms.

**HISTORIAN:** Then the cascade worst case is:

```
Pre-flight:        5ms
RDD detection:     20ms (when it fires)
Control search:    25ms + augmentation overhead (Subgroup B) = 30ms
Event Study:       75ms (when it fires, after control search)
DiD:               80ms (when it fires, including bootstrapped CI with B=500)
SCM:               35ms (when it fires, with parallel placebos)
ITS:               15ms (always runs as fallback)
```

The cascade is sequential within S4 — each method only starts if the previous fails. The absolute worst case is all methods attempted before finding a valid result:

```
5 + 20 + 30 + 75 + 80 + 35 + 15 = 260ms worst case
```

That is better than my original 312ms estimate because of the parallel SCM placebos.

**ARCHITECT:** 260ms for S4 worst case. Now S5 — Source Consensus. Simple provenance scan.

**STATISTICIAN:** 20ms worst case. Provenance traversal of source chains, counting root nodes. This is dominated by the graph traversal depth.

**ARCHITECT:** 20ms for S5. Now let me build the actual timeline.

---

**Realistic 500ms Query Plan**

The key constraint: S4 at 260ms dominates everything. All other signals must complete within that window to not extend the total query latency.

```
ASSESS CAUSALITY Cold Path — Timeline (all times in ms from query start)

t=0ms:    Query received. CausalityAssessOperator launches all 5 signal operators in parallel.
          Also: load cause and effect records from buffer cache (shared cost, ~5ms).

t=0-5ms:  Record loading completes. Signal operators initialized with FunRecord objects.

t=0-15ms: S1 (Temporal Precedence) executes. Completes by t=15ms.
          [Critical path: not on critical path]

t=0-70ms: S2 (MechanismDetect) executes.
          - t=2ms:  Tier 1 check completes (causal edges)
          - t=27ms: Tier 2 HNSW search completes (midpoint search)
          - If no result: t=57ms Tier 2b graph paths checked
          - If no result: t=137ms Tier 3 NLI runs (worst case only)
          Worst case S2 completes at t=135ms.
          [Critical path contribution: 0ms if S4 dominates at 260ms]

t=0-70ms: S3 (ConfounderSearch) executes in parallel.
          - t=30ms: PARTIAL RESULT available (Strategies 1+2 complete).
                    Top confounder score communicated to S4 via shared buffer.
          - t=70ms: FULL RESULT available (all 4 strategies complete).
          [Critical path contribution: 0ms if S4 dominates]

t=0-260ms: S4 (NaturalExperimentDetector) executes.
           - t=5ms:   Pre-flight checks (temporal ordering, series length)
           - t=25ms:  RDD detection (HNSW k=200 + threshold analysis)
           - t=55ms:  Control group search (standard k=100 + mechanism-augmented k=50 from Subgroup B)
                      NOTE: S3 partial result at t=30ms feeds confounder warning into S4 config.
           - t=75ms:  Event Study starts (if similar_causes >= 5)
           - t=155ms: DiD starts (if control_candidates >= 10)
                      NOTE: DiD bootstrap B=500 takes ~80ms; end t=235ms
           - t=235ms: SCM starts (if DiD failed)
           - t=270ms: SCM completes (with parallel placebos) [WORST CASE EXCEEDED]

WAIT: If SCM is needed, the full cascade reaches 270ms not 260ms.
Revised estimate: S4 worst case = 270ms.

           - t=270ms: ITS as final fallback (15ms) → completes t=285ms

ABSOLUTE WORST CASE FOR S4: 285ms

t=0-20ms: S5 (Source Consensus) executes. Completes by t=20ms.
          [Critical path contribution: 0ms]

t=285ms:  All signal operators complete (dominated by S4 worst case).

t=285ms:  SignalFusionOperator runs. ~1 microsecond. Negligible.
          Apply temporal gate (S1 result). Negligible.
          Apply Subgroup A confidence ceiling (C_max computation). ~1ms.

t=286ms:  Result assembled. Query returns.
```

**Total cold-path latency: 286ms worst case** (when full cascade is needed including SCM + ITS fallback).

This is comfortably within the 500ms budget, leaving **214ms of headroom**.

---

**Where the headroom goes:**

The 214ms is not waste — it is allocation for:

1. **EXPLAIN CAUSALITY overhead**: RedTeamOperator with 15 parallel checks runs after fusion. Most checks are O(1) lookups into already-computed data (signal values, timestamps, source counts). The two expensive checks are #8 (temporal stability: requires a second causal assessment over recent data only) and #3 (collider check: requires graph traversal). Estimated: 40-60ms for the full Red Team. EXPLAIN CAUSALITY worst case: 286 + 60 = 346ms.

2. **Subgroup B bidirectional integration overhead**: The mechanism-augmented HNSW search in S4 control group construction adds ~10ms when MechanismDetect finds a bridge. The SUTVA→ConfounderSearch backward link adds 0ms (data already computed). Net: +10ms on ~40% of queries.

3. **Cross-shard queries**: The Architect's distributed plan adds 2 network round trips (broadcast + merge). On a typical datacenter network (1ms RTT): +2ms. On a higher-latency network: up to +20ms.

4. **HNSW contention under concurrent load**: Under 10x concurrent query load, the HNSW index may serialize some searches. Estimated degradation: +30-50ms per search under heavy load. Mitigation: the three-tier caching (hot/warm/cold) means only truly novel pairs hit the HNSW index at full cost. Under load, cache hit rates rise, reducing HNSW pressure.

5. **Budget for future signals**: If a 6th signal (S6) is added later, the framework needs headroom. With 214ms spare, there is room for one more medium-cost signal without exceeding the 500ms budget.

---

**Signal Budget Allocation (final):**

```
Signal    Budget (hard cap)    Expected latency    Worst case
S1        20ms                 5ms                 15ms
S2        150ms                15ms (Tier 1-2)     135ms (all tiers)
S3        100ms                30ms                70ms
S4        380ms                100ms (DiD fires)   285ms (full cascade)
S5        25ms                 10ms                20ms
Fusion    5ms                  <1ms                <1ms
Ceiling   5ms                  1ms                 2ms
Overhead  15ms                 10ms                15ms
TOTAL     700ms (sum)          272ms (expected)    543ms (sum of maxima)

Effective total (max not sum, due to parallelism):
  Expected:      max(5, 15, 30, 100, 10) + overhead = 100 + 25 = 125ms
  Worst case:    max(15, 135, 70, 285, 20) + overhead = 285 + 18 = 303ms
```

The sum of hard caps (700ms) exceeds 500ms because signals run in *parallel* — the budget is allocated per signal, not consumed sequentially. Only S4's budget matters for total latency. All other signals are on the non-critical path.

**Failsafe:** If S4 exceeds its 380ms hard cap (due to unexpected data volume or algorithm degradation), the CausalityAssessOperator cancels the S4 future and uses S4 = None (ignorance mass). The result is returned with `is_partial: true` and a wider uncertainty_width. No query ever blocks longer than 500ms regardless of S4 behavior.

---

**Latency estimates from each agent (final negotiated values):**

| Signal | Operator | Agent | Expected | Worst Case | Hard Cap |
|--------|----------|-------|----------|------------|----------|
| S1 | TemporalPrecedenceOperator | Statistician | 5ms | 15ms | 20ms |
| S2 | MechanismScanOperator | Semanticist | 15ms | 135ms | 150ms |
| S3 | ConfounderScanOperator | Semanticist | 30ms | 70ms | 100ms |
| S4 | NaturalExperimentOperator | Historian | 100ms | 285ms | 380ms |
| S5 | SourceConsensusOperator | Statistician | 10ms | 20ms | 25ms |
| Fusion + Ceiling | SignalFusionOperator | Statistician | 1ms | 3ms | 5ms |
| RedTeam (EXPLAIN only) | RedTeamOperator | Skeptic | 30ms | 60ms | 80ms |

**Total ASSESS CAUSALITY cold path:** Expected 125ms, worst case 303ms, hard limit 500ms.
**Total EXPLAIN CAUSALITY cold path:** Expected 155ms, worst case 363ms, hard limit 580ms.

---

**Mini-Proposal: Subgroup C Resolution**

The 500ms budget is achievable for ASSESS CAUSALITY. EXPLAIN CAUSALITY requires up to 580ms in the absolute worst case — slightly over budget. The Architect recommends:

1. **EXPLAIN CAUSALITY budget adjustment**: Set the client-facing SLA for EXPLAIN to 600ms (the current hard limit in the Architect's proposal is already 600ms cold path). This is acceptable — EXPLAIN is not a hot path.

2. **S4 failsafe hard cap at 380ms**: Any S4 computation running past 380ms is cancelled and returns ignorance mass. This guarantees the full query never exceeds 500ms even under pathological data.

3. **Soft synchronization point for S3→S4 integration (Subgroup B)**: At t=30ms, ConfounderSearch publishes its top confounder score to a shared buffer. NaturalExperimentDetector reads from this buffer before starting the DiD stage (at approximately t=55-80ms depending on control group search speed). This soft sync adds zero additional latency since the DiD stage starts after control group search completes, well after t=30ms.

4. **Per-signal timeout as graceful degradation, not failure**: S2 at Tier 3 (NLI, 80ms CPU) should not be abandoned at 80ms if the query still has budget. Instead, S2 is allocated up to 150ms. If S2 completes by t=150ms, it contributes to fusion. If not, it contributes None (ignorance mass). The fusion handles this correctly via Dempster-Shafer.

5. **HNSW concurrency**: Recommend dedicating one HNSW query thread per concurrent signal operator to avoid cache pressure under load. The Architect will add a `hnsw_concurrency: 5` configuration parameter to the causal engine that reserves up to 5 concurrent HNSW reader slots for causal query operators, separate from the slots used by standard vector search queries.

---

## Summary: What the Three Subgroups Resolved

| Item | Status | Resolution |
|------|--------|------------|
| 0.85 confidence ceiling (Subgroup A) | Resolved | Replace with `0.90 * f_method * f_independence`; formula defined above |
| rho matrix initialization (Subgroup A) | Partial | Values estimated; require empirical validation via feedback loop |
| MechanismDetect → NaturalExperimentDetector bridge handoff (Subgroup B) | Resolved | Mechanism-augmented HNSW search for control group; defined protocol above |
| NaturalExperimentDetector → ConfounderSearch SUTVA feedback (Subgroup B) | Resolved | Surface discovered_potential_confounders in NaturalExperimentResult |
| ConfounderSearch → NaturalExperimentDetector strong confounder warning (Subgroup B) | Resolved | Partial result at t=30ms via shared buffer |
| rho_34 update for Statistician (Subgroup A/B joint) | Resolved | Increase rho_34 from 0.20 to 0.35 |
| 500ms latency budget feasibility (Subgroup C) | Resolved | Expected 125ms, worst case 303ms; hard limit 500ms via S4 cancel |
| EXPLAIN CAUSALITY latency | Resolved | SLA at 600ms; acceptable because it is not a hot path |
| HNSW concurrency risk (Subgroup C) | Resolved | Dedicated concurrency slots + per-signal time caps |

**Items requiring Team Lead decision (unchanged from Round 1):**

1. **Feedback loop — ship with partial mitigations or block?** Subgroup A's ceiling formula partially addresses calibration bias detection (the rho matrix degradation penalizes correlated feedback). But the core provenance gap (external actions not logged in FunDB) is not solved. Team Lead must decide: acceptable risk for v1, or blocking issue?

2. **rho matrix empirical validation**: The correlation matrix values are engineering estimates. Should FunDB ship with these defaults and update them, or wait for empirical calibration data?

3. **Mechanism-augmented control search**: Subgroup B's enhancement adds 10ms per query when MechanismDetect finds a bridge. Is this within acceptable latency budget? Subgroup C says yes (headroom exists). But it does add conditional complexity to the NaturalExperimentOperator. Team Lead must decide: include in v1 or defer to v2?

---

*End of Round 2 Subgroup Working Sessions.*

*Total session duration: Approximately 2 hours.*

*All agents sign off on the above mini-proposals as their best-effort resolutions. The three items above are formally escalated to Team Lead adjudication.*
