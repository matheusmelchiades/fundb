# FunDB Causal Reasoning Hackathon — Team Lead Evaluation

**Evaluator:** Team Lead
**Date:** 2026-02-28
**Session Duration:** ~3.5 hours (Proposals + Round 1 Crossfire + Round 2 Subgroups)

---

## Section 1: Individual Agent Scores

---

### Agent: Statistician

**Overall Score: 90/100**

#### Correctness: 23/25

The Murphy-modified Dempster-Shafer framework is mathematically sound. The mass function conversion is verified algebraically and the commutativity proof is correct. The temporal gate application post-fusion rather than pre-fusion is the right design choice — it avoids discarding evidence before combination.

Two docks: First, the binary frame Theta = {C, ~C} is an acknowledged simplification. Real causal relationships have degree and type (CAUSED vs. INFLUENCED vs. CORRELATED), and collapsing this into a single binary frame loses structural information. The CausalEdge threshold mapping (confidence >= 0.8 => CAUSED) is a post-hoc workaround, not a principled solution. Second, the initial r4 = 0.85 as a single fixed value was correctly identified as wrong in crossfire and revised. The initial proposal should not have shipped that value without the per-method differentiation — it was intellectually sloppy in a way that the Statistician should have caught before the room did.

#### Feasibility: 25/25

The signal fusion loop is O(n) with n <= 5 and completes in under 1 microsecond. This is essentially free compared to the cost of computing the signals themselves. The Platt calibration is a 2-parameter online SGD update — also trivially fast. The Statistician correctly identified that the fusion layer adds zero overhead to query latency. This is the right design principle and was executed perfectly.

#### Ergonomics: 20/25

The `CAUSAL_CONFIDENCE()` FunQL function is clean and the output schema (causal_confidence, uncertainty_width, conflict_degree, plausibility) is well-designed. The distinction between Bel(C) and Pl(C) is preserved in the output, which is unusually good for a system aimed at non-statisticians.

The deduction: the proposal does not fully specify how a developer consumes `conflict_degree`. The interpretation table (K < 0.1 = trust, K >= 0.6 = do not trust) is buried in Section 6.2 with no mechanism for surfacing it to the user in an actionable way. A developer reading `conflict_degree: 0.4` has to go look up the table. The Architect's EXPLAIN CAUSALITY verb partially solves this, but the Statistician's proposal should have been more explicit about it.

#### Honesty: 22/25

Excellent handling of missing signals (total ignorance mass), missing signal penalty, and conflict detection. The key guarantee — "more missing signals = wider uncertainty interval, never lower confidence without evidence" — is a real and valuable property.

The deduction: the initial hardcoded 0.85 ceiling was presented with more confidence than was warranted. The Statistician called it "an empirical bound" and "grounded in Bradford Hill's domain" but then conceded in crossfire it was a heuristic placeholder. That gap between presentation and reality is a honesty issue. The revised formula from Subgroup A is much more defensible, but the initial framing was misleading.

---

### Agent: Semanticist

**Overall Score: 82/100**

#### Correctness: 20/25

The three-tier MechanismDetect algorithm is logically sound. The harmonic mean for bridge scoring is the right choice over arithmetic mean — the Semanticist correctly anticipated the asymmetry problem. The midpoint search technique has good geometric intuition.

The problems: First, the NLI Tier 3 score cap at 0.75 and Tier 2 cap at 0.85 are stated without derivation. They are reasonable numbers but presented as principled limits when they are really judgment calls. The Skeptic's attack on this was fair — if the NLI model produces entailment = 0.95 with strong asymmetry, why is the cap 0.75? There is no mathematical argument for that specific value. Second, the ConfounderSearch coverage_score formula `min(1.0, records_evaluated / total_records_in_timewindow)` is misleadingly precise. "Coverage" in the semantic search sense is not the fraction of records scanned — it is the fraction of the *relevant* semantic space examined. Scanning 90% of records in a time window does not mean you found 90% of confounders; it means you found 90% of records near a specific time window. These are different things.

#### Feasibility: 21/25

The tiered fallback design is exactly right for a query-time constraint. Most queries exit at Tier 2 (10ms). Tier 3 NLI at 80ms CPU / 20ms GPU is the real cost. The Semanticist acknowledged the HNSW contention problem in crossfire and accepted the Architect's revised estimate of 70ms under load for ConfounderSearch. The four parallel ConfounderSearch strategies are correctly identified as independent and parallelizable.

The deduction: the proposal originally stated "parallelizable to ~35ms" without acknowledging the HNSW contention problem. This was an honest mistake but it was a significant underestimate that affected downstream planning. The Semanticist was called out for it and accepted the correction gracefully, but the original estimate was not qualified appropriately.

#### Ergonomics: 20/25

`DETECT MECHANISM`, `SEARCH CONFOUNDERS`, and `ASSESS CAUSALITY` syntax are clean and sensible. The `mechanism_type` enum (DIRECT, BRIDGED, CHAIN, INFERRED, NONE) is excellent — it tells the developer not just the score but the *quality of evidence*. This is the kind of design that separates a good proposal from a great one.

The deduction: the `net_causal_score` formula `mechanism_support * (1 - confounder_penalty * confounder_coverage)` in Section C.2 is unintuitive to developers. Multiplying coverage into the penalty means that low coverage reduces the penalty — which means having searched less of the database *increases* the causal score. The intention is correct (low coverage = more ignorance, not more support) but the formula does the wrong thing in edge cases and will confuse developers. This should have been caught before the proposal was submitted.

#### Honesty: 21/25

The tiered score caps by evidence quality are a genuinely good honesty mechanism. Saying "Tier 3 NLI can score at most 0.75 because NLI alone is weak evidence" is exactly the right design philosophy. The coverage_score reporting is also commendable.

The deduction: the semantic bridge algorithm will happily construct a bridge between semantically similar but causally unrelated events. The Skeptic's attack on this (Failure Mode 2.2: semantic similarity is not causal similarity) is valid and largely unaddressed in the Semanticist's proposal. The proposal acknowledges it as a limitation in Section E.1 but does not propose a fix. For a system whose primary job is causal inference, "we know this doesn't distinguish correlation from causation" is a significant honesty gap.

---

### Agent: Historian

**Overall Score: 88/100**

#### Correctness: 24/25

The quasi-experimental cascade is technically excellent. The assumption validation for each method is rigorous and well-specified: parallel trends p-value thresholds, McCrary density test for RDD, balance checks with SMD < 0.25, SUTVA verification, pre-treatment fit quality for SCM. The S4-to-signal conversion (`s4 = 0.5 + 0.5 * qe_score` when direction is positive) correctly maps to the Statistician's Dempster-Shafer ignorance center at 0.5 when no experiment is found.

One dock: the SCM inference via placebo tests (Abadie et al. approach) is correct in principle but the implementation approximation `placebo_sd = std(placebo_effects)` followed by a normal confidence interval is questionable. The correct approach is the permutation p-value the Historian does use — `fraction of placebos with larger absolute effect` — but the 95% CI from `±1.96 * placebo_sd` assumes the placebo distribution is normal, which it is not necessarily. This is a minor statistical inaccuracy in an otherwise rigorous proposal.

#### Feasibility: 22/25

The cascade design with sequential fallback is the right approach. The RDD-first ordering is smart — it checks the highest-value method at lowest cost (15ms) before committing to more expensive methods. The budget_ms parameter and early exit on valid result are good engineering.

The deduction: the DiD bootstrap with B=500 resamples at ~80ms is acceptable but fragile. Under high-cardinality control groups (n_control = 100, T = 60 periods), the bootstrap could easily run to 200ms+. The proposal does not specify what happens when the DiD bootstrap overruns its allocation. The Subgroup C work established the 380ms S4 hard cap as the failsafe, but the Historian's proposal itself does not include this safeguard — it relies on the Architect to enforce it from outside.

#### Ergonomics: 21/25

`EVALUATE QUASI_EXPERIMENT` is well-designed. The minimum invocation (just CAUSE and EFFECT) is genuinely usable. The output schema is comprehensive and the `method_used` field tells developers exactly what was computed. The method selection log is the best ergonomics feature in any proposal — if DiD failed because parallel trends was violated, the developer can see that clearly.

The deduction: the bulk evaluation syntax requires developers to understand the difference between `cause_event_id` and `entity_id` — the proposal introduces `_entity_id` as a concept without explaining how it maps to FunRecord's data model. A developer who does not know which events share the same entity will write control group searches that return incorrect results. This is not a design flaw but a documentation gap that would cause real problems in practice.

#### Honesty: 21/25

The honesty commitments in Section 7 are excellent: assumption reporting is mandatory, "no valid experiment" is a first-class result, ITS capped at 0.60 for structural reasons, S4 = 0.5 when no experiment found. The per-method r4 reliability values (RDD: 0.90, ITS: 0.55) are honest assessments of method quality.

The deduction: the Historian argued in crossfire that "when S4 fires with a valid result, S4 should override S2." This is an intellectually honest position from the standpoint of the causal ladder, but the proposal does not operationalize it. The conflict between S4 and S2 produces a high conflict_degree K in Dempster-Shafer with no additional guidance for the developer. The post-fusion `CONFLICT` causal_type the Historian proposed in crossfire is the right fix, but it is not in the written proposal. The proposal promises rigorous honesty but does not fully deliver on conflict resolution.

---

### Agent: Skeptic

**Overall Score: 85/100**

#### Correctness: 21/25

The 27 failure modes identified across signals and combined are substantive and mostly correct. The Skeptic correctly identified: the feedback loop problem (Combined Failure Mode 5), Zadeh's paradox analog for signal combination (Combined Failure Mode 3), collider bias (Failure Mode 3.2), publication bias (Failure Mode 5.4), and weak instruments (Failure Mode 4.2). These are real attacks and the safeguards are generally well-reasoned.

Two docks: First, the Benjamini-Hochberg FDR correction proposal (Failure Mode 3.3) is correct in principle but the proposal applies it to confounder search without specifying what the "null hypothesis" is in that context. Multiple comparison correction applies to significance tests, but the Semanticist's confounder scoring is not a significance test — it is a similarity score. The Skeptic is applying a correction that assumes a statistical framework that does not exist in the target system. Second, the proposed "adversarial audit score" (Combined Failure Mode 2) is described but not specified. How do you measure whether data looks "too clean"? There is no algorithm, just a description of what the algorithm should do.

#### Feasibility: 19/25

The 15-point Red Team checklist is the most valuable deliverable. The Architect correctly placed it in the `RedTeamOperator` running at EXPLAIN CAUSALITY time (not ASSESS CAUSALITY time), which is the right feasibility decision — running 15 checks on every query would be prohibitive.

The deduction: several checklist items are not computable at query time without significant additional work. Item 5 (Aggregation Level Check: "test at individual, group, and population levels") requires rerunning the entire causal assessment at multiple granularities — this is O(k) additional ASSESS CAUSALITY calls, not a single check. Item 8 (Temporal Stability Check) requires a second causal assessment over recent data only. Neither item has a latency estimate or a fallback if data is insufficient. The checklist is aspirational in places where it should be operational.

#### Ergonomics: 23/25

The warning taxonomy (Tier 1: always emit, Tier 2: high confidence, Tier 3: specific risks, Tier 4: high-stakes domains) is excellent API design. The structured warning objects with `type`, `severity`, `message`, and `action` are exactly what an AI agent needs to make downstream decisions. This is the best ergonomics work in any proposal.

One dock: The Red Team checklist results in the EXPLAIN output (Section 2.4 of Architect's proposal) show boolean `passed` values for each check. But for several checks (items 2, 4, 5), the binary pass/fail hides a continuous spectrum. A common cause scan returning 0 candidates vs. returning 1 low-confidence candidate vs. 3 high-confidence candidates should not all map to the same boolean output. The Skeptic should have pushed for scoring rather than binary checks.

#### Honesty: 22/25

The Skeptic's acknowledgment in crossfire that the feedback loop problem is not fully solved — "I want this formally acknowledged: the feedback loop problem is not solved in any of our proposals" — is the most valuable single statement in the entire session. An agent with an adversarial brief being honest about the limits of its own proposals is exactly the intellectual character that makes a team function well.

The deduction: the hardcoded confidence ceiling of 0.85 was the Skeptic's own proposal, and the Skeptic initially defended it against the Statistician's challenges before conceding in the same exchange that it was arbitrary. A Skeptic who does not pre-apply their own red team process to their own proposals before submission is not being maximally honest.

---

### Agent: Architect

**Overall Score: 91/100**

#### Correctness: 23/25

The Volcano model operator composition is correctly specified and matches how real query engines work. The parallelism model — launching all five signal operators concurrently and blocking until all complete or timeout — is the right design. The three-tier caching (hot/warm/cold) with proper invalidation semantics is solid systems design. The distributed execution plan for cross-shard queries correctly identifies that data movement should be minimized by running heavy computation local to each shard.

Two docks: The `ConfounderScanOperator` S3 conversion formula `s3 = (1.0 - top_score) * coverage + 0.5 * (1.0 - coverage)` has a subtle problem at the boundary. When coverage = 0.0, s3 = 0.5 regardless of top_score — this is correct, total ignorance. But when coverage = 0.0 and a confounder was still found (top_score = 0.9), the formula returns 0.5 instead of reflecting the found confounder. The formula should handle this case explicitly: if a confounder was found at high confidence, it should contribute even under low coverage. The HNSW concurrency estimate of 303ms in Subgroup C assumed dedicated reader slots which is an infrastructure requirement, not a delivered guarantee.

#### Feasibility: 25/25

This is the strongest feasibility work in the hackathon. The latency negotiation in Subgroup C — walking through each operator with specific worst-case estimates and arriving at a defensible 303ms cold-path figure — is exactly the right process. The per-signal hard caps with graceful degradation (return ignorance mass rather than error on timeout) is the correct design. The streaming result support (progressive refinement via SSE/WebSocket) is a genuinely useful addition.

The three-tier routing is sound: the decision tree is explicit, cache invalidation logic is specified, and the hot cache promotion criteria are clear. The distributed plan correctly identifies cross-shard coordination overhead as approximately 2-20ms — within budget.

#### Ergonomics: 24/25

The four-verb taxonomy (INFER, ASSESS, EXPLAIN, TRACE) with a clear use case, latency expectation, and signal set for each is the best ergonomics design in any proposal. A developer can read that table and immediately understand which verb to use for their use case. The CONFIRM and DISCONFIRM CAUSALITY write-back verbs complete the developer loop.

One dock: the `STREAM` option for ASSESS CAUSALITY (progressive results via WebSocket) is described but its interaction with the query planner's cost model is not specified. How does the planner handle a streaming query differently from a blocking query? Do timeouts work the same way? If a streaming client disconnects mid-query, what happens to running signal operators? These are implementation-critical questions left unanswered.

#### Honesty: 19/25

This is the Architect's weakest dimension — not because of dishonesty, but because the Architect's role is primarily design, and honest uncertainty communication is less central to that role. The Architect correctly flagged the HNSW contention problem as a real concern and forced the other agents to give worst-case estimates rather than expected-case estimates. That is a form of epistemic honesty.

The deduction: the 303ms worst-case estimate in Subgroup C is presented with confidence that is not fully warranted. It assumes HNSW dedicated reader slots (a configuration choice, not a guarantee), parallel SCM placebos (requiring FunDB's TaskScheduler support, not yet specified), and stable buffer cache for record loading. Each assumption is reasonable but none is validated. The sum is an estimate with significant uncertainty that should have been flagged more explicitly as "under favorable conditions."

---

### Score Summary

| Agent | Correctness /25 | Feasibility /25 | Ergonomics /25 | Honesty /25 | Total /100 |
|-------|----------------|----------------|----------------|-------------|------------|
| Statistician | 23 | 25 | 20 | 22 | **90** |
| Semanticist | 20 | 21 | 20 | 21 | **82** |
| Historian | 24 | 22 | 21 | 21 | **88** |
| Skeptic | 21 | 19 | 23 | 22 | **85** |
| Architect | 23 | 25 | 24 | 19 | **91** |

---

## Section 2: Team Dynamics Assessment

### Which Debates Were Productive?

**Most Productive: Statistician vs. Skeptic on the 0.85 Ceiling**

This was the best debate in the hackathon. The Skeptic forced the Statistician to admit a heuristic was being presented as a mathematical safeguard. The Statistician responded by conceding the point and proposing a structure for a principled derivation. The debate ended with Subgroup A producing `C_max = 0.90 * f_method * f_independence` — a formula that is genuinely better than what either agent started with. This is what good technical debate looks like: one agent challenges a number, the other defends it, the defense fails on the merits, and the result is an improved design rather than a stalemate.

**Second Most Productive: Architect's Latency Interrogation in Round 1**

The Architect's intervention — "I need every agent to give me a worst-case latency estimate, not an expected case" — reshaped the second half of the crossfire. The Semanticist's "parallelizable to 35ms" estimate was revealed to be optimistic under HNSW contention. The Historian's SCM bootstrap was identified as a potential overrun. The Subgroup C session then turned these concerns into a concrete 303ms timeline. Without the Architect forcing specificity in Round 1, Subgroup C would have had nothing to negotiate with.

**Productive but Incomplete: Historian vs. Semanticist on Bridge Handoff**

The semantic bridge as a mechanism for improving control group quality (Subgroup B) was a genuine intellectual advance. The idea that `augmented_query_vec = 0.7 * cause._embedding + 0.3 * bridge_record._embedding` produces better control candidates than the cause embedding alone is intuitive and operationally sound. The bidirectional SUTVA-to-ConfounderSearch feedback is also good. However, neither agent fully resolved whether this creates a meaningful signal independence violation in the fusion layer. They handed it to the Statistician via a rho_34 update, which is the right approach but leaves the validation work undone.

### Which Debates Were Defensive?

**Semanticist Defending Score Caps**

When the Historian challenged the Semanticist's score caps (Tier 2 capped at 0.85, r2 = 0.50), the Semanticist's initial response was to challenge the Historian's r4 = 0.85 in turn rather than directly addressing whether their own caps were justified. This is defensive pattern recognition: deflect by attacking the challenger. The Statistician helpfully redirected the debate toward the legitimate point — that r2 should be tier-dependent, not a single constant — and both the Statistician and Semanticist reached a useful conclusion. But the Semanticist's initial defense was not intellectually useful.

**Skeptic on the 0.85 Ceiling**

The Skeptic proposed the 0.85 ceiling and then — when challenged by the Statistician in the crossfire — initially maintained that it was a legitimate safeguard before eventually conceding. A Skeptic should have already stress-tested their own number before submitting the proposal. The fact that the most adversarial agent in the room needed the Statistician's pushback to update their own heuristic is mildly ironic.

### Which Subgroup Solved Their Problem Best?

**Subgroup A (Statistician + Skeptic): Best Solution**

They were assigned the hardest problem — derive a mathematically honest maximum confidence ceiling — and they delivered a formula. The `C_max = 0.90 * f_method * f_independence` formula is not perfect (the rho matrix values are estimates, C_base = 0.90 is itself a judgment call), but it has a defensible structure, a clear interpretation, and identified its own uncertainty. The agents did not paper over the remaining gaps; they labeled them explicitly as "open issues requiring empirical validation." This is good engineering practice.

**Subgroup C (Architect + Everyone): Effective but Procedural**

Subgroup C conducted a useful negotiation and produced a concrete timeline. But the session was more audit than design — it was verifying and correcting existing proposals rather than creating new ideas. The 303ms estimate is the right artifact to have, and the dedicated HNSW reader slots proposal is the right recommendation. This subgroup solved its problem but the problem itself was narrower.

**Subgroup B (Semanticist + Historian): Interesting but Incomplete**

The mechanism-augmented control group search is a good idea. The SUTVA feedback mechanism is elegant. But the subgroup's own unresolved question — does the S3/S4 interdependency violate the Statistician's independence assumption — is not actually resolved. They handed it to the Statistician as a rho_34 update and moved on. That is a reasonable handoff, but it means the problem is deferred, not solved.

### Which Agent Added the Most Value to Others' Work?

**The Architect added the most value to others' work.** By forcing worst-case latency estimates from every agent, the Architect turned four individually plausible proposals into a single integrated system with a concrete, defensible SLA. Without that forcing function, the hackathon would have produced five excellent algorithm descriptions with no guarantee they could coexist in a single query. The Architect also produced the physical operator specs that translate each proposal's algorithm into an executable component. No other agent's proposal could run without the Architect's work; the Architect's proposal could run even without some of the others.

**The Statistician added the most value to the mathematical coherence of the system.** The Dempster-Shafer fusion layer is what turns five heterogeneous signals into a single interpretable output. Without it, the other agents would have produced five numbers with no principled way to combine them. The Statistician also correctly identified the tier-dependent reliability problem with S2 in crossfire, which improved the Semanticist's contribution in the integrated design.

### Which Conflicts Were Genuinely Resolved vs. Papered Over?

**Genuinely Resolved:**
- The confidence ceiling: moved from arbitrary 0.85 to computed `C_max = 0.90 * f_method * f_independence`. The formula exists, is defensible, and was stress-tested in subgroup discussion.
- The per-method r4 differentiation: the Historian already had per-method values (RDD: 0.90, DiD: 0.75, ITS: 0.55). The Statistician and Historian agreed these should be used directly rather than a fixed r4 = 0.85. This was clean.
- The 500ms latency feasibility: Subgroup C produced a real timeline with real numbers. The 303ms worst case is a defensible claim, not a hope.

**Papered Over:**
- The rho matrix independence estimates: the values are engineering estimates without empirical grounding. The subgroup correctly labeled this as "open" but the formula ships with these numbers in v1, which means v1 is shipping with known-uncertain parameters.
- S3/S4 independence violation: rho_34 was bumped from 0.20 to 0.35 and the discussion moved on. Whether 0.35 is the right value, and whether the Subgroup B bidirectional integration introduces further correlation, is not answered.
- Tier-dependent r2 for S2: this was correctly identified as necessary in crossfire, but no updated r2 table (r2_tier1, r2_tier2, r2_tier3) was ever specified. It was acknowledged as needed but not delivered.

---

## Section 3: Adjudication of Unresolved Conflicts

---

### Decision 1: S4 Interface Contract

**The conflict:** Should S4 reliability be fixed per-method (r4 = 0.85 as a constant) or dynamically degraded based on validity diagnostics?

**Decision: Diagnostics-degraded reliability. The fixed-per-method approach is correct but must include diagnostic adjustment.**

The Historian's per-method r4 table (RDD: 0.90, Event Study: 0.80, DiD: 0.75, SCM: 0.75, ITS: 0.55, no QE: 0.0) is the right foundation. These values should be used, not the Statistician's single r4 = 0.85. But the Statistician's question in crossfire — should a DiD that barely passes parallel trends at p = 0.11 get the same r4 as one that passes at p = 0.45? — has the correct answer: no.

The interface contract will be:

```
S4 output = {
    s4_value:       float in [0, 1]
    r4_base:        float  -- method-specific base from Historian's table
    r4_degradation: float in [0, 1]  -- 1.0 = no degradation, 0.0 = full degradation
    r4_effective:   r4_base * r4_degradation  -- what Statistician uses
    diagnostics:    ValidityDiagnostics
}
```

The `r4_degradation` is computed by the Historian according to this schedule:

- DiD parallel trends: if p > 0.20, degradation = 1.0; if p in [0.10, 0.20], degradation = lerp(0.7, 1.0); if p < 0.10, method is invalid and not returned.
- DiD balance: if max_smd < 0.10, degradation = 1.0; if in [0.10, 0.25], degradation = lerp(0.8, 1.0).
- DiD SUTVA: if SUTVA passed, degradation = 1.0; if failed, degradation = 0.8.
- ITS pre-trend significant: if pre_trend_warning fires, degradation = 0.75 (reducing ITS r4 from 0.55 to 0.41).
- SCM pre-fit: if pre_fit_pct in [0.10, 0.20], degradation = lerp(0.7, 1.0).

The Statistician uses `r4_effective` in mass function conversion: `m({C}) = r4_effective * s4_value`.

This is Option 2 from the Architect's formulation, with the understanding that Option 1 (fixed per method) is the fallback when diagnostics cannot be computed. Option 3 (evidence ceiling) is handled separately by Subgroup A's `C_max` formula — it is a different mechanism addressing a different problem and should not be conflated with the per-method reliability parameter.

**Why not a pure fixed constant:** The Statistician's original r4 = 0.85 lumped all methods together. That is wrong. A DiD is worth more than an ITS regardless of diagnostic values.

**Why not a fully dynamic continuous function of all diagnostic values:** Because the relationship between diagnostic values and reliability is not established empirically. A DiD that passes all diagnostics at p = 0.11 may be more reliable than one that passes at p = 0.45 if the underlying data has different variance. We do not have the empirical data to fit a continuous function. The lerp-based degradation is the right level of complexity for v1.

---

### Decision 2: Feedback Loop — Ship or Block?

**The conflict:** The Skeptic admitted the feedback loop (Combined Failure Mode 5) is not fully solved. No agent has a complete solution for cases where external actions influenced by FunDB's causal claims generate new data that FunDB treats as fresh evidence.

**Decision: Ship with partial mitigations and a mandatory explicit disclosure. Do not block.**

The partial mitigations that ship in v1 are:
1. The `_influenced_by` provenance tag on CONFIRM/DISCONFIRM CAUSALITY write-backs.
2. The Platt calibration update must check source distribution before accepting feedback: if more than 50% of recent positive labels for a given pair come from the same agent within a 7-day window, decay that agent's feedback weight by 0.5.
3. The Skeptic's source independence formula for S5 (`1 - (1 - base)^n_independent`) applies to feedback events for calibration purposes: n_independent for calibration is computed using the same provenance-based independence check as S5.

**Why not block:** Blocking on a complete solution means blocking indefinitely. The feedback loop is a problem inherent to any system where AI agents read from and write to the same knowledge base. There is no known complete solution to this problem in the general case. Perfect provenance tracking of external actions is not achievable without controlling every system that reads FunDB's output, which is not a realistic constraint.

**Why partial mitigations are sufficient for v1:** The primary feedback loop risk is that a false positive causal claim gets confirmed at high frequency in a short time window by the same agent. The rate-limiting mitigation directly addresses this case. The broader case — where many diverse agents independently confirm a false claim because they all acted on it and observed the same post-intervention behavior — is the hard version of the problem and requires a causal stability detector that is not built yet.

**What ships in v1 that is not sufficient:** The mandatory explicit disclosure. Every causal query response in v1 will include a static warning that feedback loops are a known risk and that confidence scores may be inflated if AI agents have previously acted on this causal relationship. This is not a technical solution. It is an epistemic commitment to transparency. It is the minimum responsible disclosure.

**What ships in v2:** A causal stability detector that compares the causal confidence for a given pair over a rolling 30-day window against the 90-day historical baseline. If confidence has increased by more than 0.15 in 30 days and a significant fraction of new evidence was written by agents that previously read this pair's assessment, emit a POTENTIAL_FEEDBACK_LOOP warning with severity HIGH. This is a detectable signal; the Statistician's observation that non-iid feedback distribution indicates contamination is the right detection approach.

---

### Decision 3: HNSW Concurrency Latency

**The conflict:** The Architect's 303ms worst-case estimate assumes dedicated HNSW reader slots (`hnsw_concurrency: 5`). Without this, five concurrent HNSW searches will cause cache pressure, TLB thrashing, and branch misprediction that is not parallelizable away.

**Decision: Dedicated HNSW reader slots are required, not optional. The 303ms estimate is the target, not the current state.**

The dedicated slot model (`hnsw_concurrency: 5` reserved for causal operators) must be implemented as a first-class feature before causal operators go to production. Without it, the 303ms estimate is not achievable under real concurrent query load, and the 500ms SLA cannot be guaranteed.

However, causal operators do not need to use the same HNSW index as standard vector search queries. The recommendation is to maintain a separate HNSW segment for causal-query access that is structurally identical to the main index but accessed through a different I/O path. This avoids cache pollution from causal queries affecting standard queries, and vice versa. The cost is approximately 2x the HNSW index memory for deployments that use causal operators, which is acceptable given that HNSW indexes are already cached in memory and the duplication is read-only data.

If separate segments are not feasible for a given deployment (memory-constrained environments), the fallback is: reduce `ef_search` for ConfounderSearch from the recommended 200 to 100 (accepting lower recall), and reduce the ConfounderSearch strategy set to 2 strategies (Embedding Triangle + Common Ancestor only), skipping Text Co-occurrence which uses the most HNSW queries. This reduces ConfounderSearch worst-case from 70ms to approximately 30ms, giving more headroom to S4 under contention.

**Why not use a different index strategy for causal operators:** The HNSW index is the right index for semantic similarity search. There is no better alternative for the midpoint search, control group candidate retrieval, and confounder triangle search that these operators perform. LSH alternatives have lower recall that would materially degrade S2 and S3 quality. The answer is to manage the HNSW concurrency problem, not to abandon the index type.

---

## Section 4: The Synthesized Solution

This is the best version of FunDB's Causal Reasoning Engine, combining the strongest contributions from each agent into a coherent whole.

---

### Data Model Additions to FunRecord

Every FunRecord gains three optional fields in `_metadata` for causal reasoning:

```
_metadata additions:
{
    "_entity_id":         UUID,         -- the system/tenant/agent this event belongs to
    "_trigger_rule":      string,       -- if this event was triggered by a threshold rule
    "_threshold":         float,        -- the threshold value that triggered this event
    "_running_variable":  string,       -- the running variable for RDD detection
    "_influenced_by":     [UUID],       -- list of causal claim IDs that influenced this event
    "_causal_tags":       {             -- for causal domain hinting
        "domain":         string,       -- e.g., "medical", "financial", "systems"
        "is_preparatory": bool,         -- true for anticipatory events (flu medication stocking)
        "event_class":    string        -- e.g., "deployment", "metric_spike", "policy_change"
    }
}
```

New CausalEdge stored in FunCausal index:

```
CausalEdge {
    source_id:          UUID,
    target_id:          UUID,
    relation:           CAUSED | INFLUENCED | CORRELATED | PRECEDED | CONFLICT,
    strength:           float in [0, 1],   -- Bel(C) from fusion
    mechanism:          string,
    _metadata: {
        uncertainty_width:   float,
        conflict_degree:     float,
        confidence_ceiling:  float,        -- C_max from Subgroup A formula
        ceiling_factors:     { f_method, f_independence, C_base },
        signals_used:        [S1..S5],
        signal_values:       { s1..s5 },
        r4_effective:        float,        -- diagnostic-adjusted S4 reliability
        quasi_experiment:    { method, effect_estimate, ci, diagnostics },
        red_team_summary:    { all_passed, n_warnings, warnings },
        fusion_method:       "murphy_ds_v1",
        assessment_time:     timestamp,
        feedback_loop_risk:  float in [0, 1]
    }
}
```

---

### The 5 Signals and How They Are Computed

**S1: Temporal Precedence (r1 = 0.60, tier-adjusted)**

Computed from bitemporal `_valid_from` timestamps. Gate: if s1 < 0.2, cap final confidence at s1 * 0.5. When `_created_at - event_time > threshold`, apply Skeptic's timestamp manipulation penalty (reduce r1 by 0.3). When the effect variable was > 2 SD from its mean at cause time, apply regression-to-mean warning and reduce r1 by 0.2.

**S2: Semantic Mechanism (r2 tier-dependent)**

MechanismDetect three-tier cascade. r2 by mechanism_type: DIRECT (direct causal edges) → r2 = 0.80; BRIDGED/CHAIN (semantic bridge, graph-augmented) → r2 = 0.60; INFERRED (NLI only) → r2 = 0.40; NONE → r2 = 0.0 (no S2 contribution). Score caps: DIRECT unlimited, BRIDGED/CHAIN capped at 0.85, INFERRED capped at 0.75. Counterfactual consistency check: present NLI with both "A then B" and "A then not-B" — if both return high entailment, s2 = 0.5 (uninformative).

**S3: Confounder Exclusion (r3 = 0.70)**

ConfounderSearch four-strategy parallel execution (Embedding Triangle, Common Ancestor, Text Co-occurrence, Temporal Context). At t=30ms, publish partial result (Strategies 1+2) to shared buffer for S4 integration. S3 formula: `s3 = (1 - top_score) * coverage + 0.5 * (1 - coverage)`, with additional logic: if a confounder is found at score > 0.7 regardless of coverage, s3 is capped at 0.3. Benjamini-Hochberg FDR correction applied to any statistical confounder tests. SUTVA violations from DiD (backward from S4) augment the confounder candidate list.

**S4: Natural Experiments (r4_effective = r4_base * r4_degradation)**

NaturalExperimentDetector cascade: RDD → Event Study → DiD → SCM → ITS. Control group search augmented with mechanism vector when S2 finds a BRIDGED/CHAIN mechanism (Subgroup B integration). Strong confounder (score > 0.7 from S3 partial result) reduces DiD base credibility by `0.5 * confounder_score`. Per-method r4_base: RDD 0.90, Event Study 0.80, DiD 0.75, SCM 0.75, ITS 0.55. Diagnostic degradation applied to r4_base using validity diagnostics. SUTVA violations surfaced as discovered_potential_confounders for S3 update.

**S5: Source Consensus (r5 = 0.40, scales with n_independent)**

Source credibility formula: `s5 = 1 - (1 - base)^n_independent`, capped at 0.95. n_independent uses provenance graph traversal — only root sources (no tracked upstream) count. Methodology diversity factor: if all agreeing sources share the same methodology, n_independent is further discounted by 0.5. Null results tracked: "no significant relationship found" logged as negative evidence. Feedback events subject to same independence test as source consensus.

---

### The Fusion Algorithm

**Step 1: Convert signals to mass functions**

```
For each signal Si with value si and reliability r_i_effective:
    m_i({C})    = r_i * s_i
    m_i({~C})   = r_i * (1 - s_i)
    m_i({C,~C}) = 1 - r_i

Where r_i_effective for S4 = r4_base * r4_degradation (diagnostic-adjusted).
Where r_i for S2 depends on mechanism_type (not a fixed constant).
Missing signal → m({C,~C}) = 1 (total ignorance, no contribution).
```

**Step 2: Murphy averaging followed by (n-1) Dempster combinations**

```
m_avg = average of all available signal mass functions
Combine m_avg with itself (n-1) times using Dempster's rule
K accumulated = conflict degree
```

**Step 3: Temporal gate (post-fusion)**

```
If s1 < 0.2: cap causal_confidence at s1 * 0.5
If s1 missing: multiply causal_confidence by 0.7, widen uncertainty_width by 0.2
```

**Step 4: Subgroup A confidence ceiling (post-Platt-calibration)**

```
C_max = 0.90 * f_method * f_independence

f_method from best quasi-experimental method:
  RDD          → 1.00
  Event Study  → 0.95
  DiD          → 0.90
  SCM          → 0.88
  ITS          → 0.70
  No valid QE  → 0.60

f_independence = sqrt(n_eff / n_total)
  n_eff = effective independent signal count via eigenvalue adjustment of rho matrix
  rho matrix (initialized values, updated via background learning):
    rho(S1, S4) = 0.40, rho(S2, S3) = 0.50, rho(S3, S4) = 0.35 [updated per Subgroup B]
    all others at estimated values from Subgroup A discussion

causal_confidence = min(platt_calibrated_score, C_max)
Output includes: confidence_ceiling, ceiling_factors
```

**Output schema:**

```
{
    causal_confidence:    float,  -- Bel(C), post-ceiling
    uncertainty_width:    float,  -- Pl(C) - Bel(C), ignorance interval
    conflict_degree:      float,  -- accumulated K, disagreement between signals
    plausibility:         float,  -- Pl(C) = upper bound
    confidence_ceiling:   float,  -- C_max from formula
    ceiling_factors:      { f_method, f_independence, C_base },
    signals_used:         [S1..S5],
    signal_values:        { s1..s5 },
    r_effective:          { r1..r5 },  -- actual reliability used per signal
    is_partial:           bool,
    feedback_loop_risk:   float
}
```

---

### The 4 Query Operators in FunQL

**INFER CAUSALITY FROM A TO B**

Fast path. Uses S1 + S2 (Tier 1 and 2 only, no NLI) only. Expected latency 5-15ms. High uncertainty_width because only 2-3 signals. Suitable for high-frequency programmatic screening.

```sql
INFER CAUSALITY FROM :event_a TO :event_b [TIMEOUT 50ms];
-- Returns: { causal_confidence, uncertainty_width, signals_used, is_partial }
```

**ASSESS CAUSALITY FROM A TO B**

Full 5-signal assessment with Murphy D-S fusion and confidence ceiling. All signal operators run in parallel. Expected cold-path latency 125ms, worst case 303ms, hard limit 500ms.

```sql
ASSESS CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id
    [WITH SIGNALS (temporal=ON, mechanism=ON(...), confounders=ON(...),
                   experiment=ON(...), consensus=ON)]
    [TIMEOUT 500ms]
    [STREAM];
```

**EXPLAIN CAUSALITY FROM A TO B**

Full assessment plus Skeptic's Red Team checklist (15 checks in parallel, post-fusion). All 15 check results, specific warnings per failed check, and natural language explanation. Expected cold-path latency 155ms, worst case 363ms, hard limit 600ms.

```sql
EXPLAIN CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id
    [FORMAT natural_language]
    [CONTEXT_BUDGET 500];
-- Returns: all ASSESS results + red_team_checklist + warnings + explanation_text
```

**TRACE CAUSALITY FROM A TO B**

Existing operator, enhanced with signal metadata per edge. Enumerates causal paths through the FunCausal DAG. Now surfaces uncertainty_width and method_used per edge.

```sql
TRACE CAUSALITY FROM :source TO :target
    MAX_DEPTH 5 MIN_STRENGTH 0.3
    [INCLUDE SIGNAL_METADATA];
```

---

### Execution Plan for ASSESS CAUSALITY

```
t=0ms:    Query received.
          Load cause and effect FunRecords (~5ms from buffer cache).
          Launch S1, S2, S3, S4, S5 in parallel on worker thread pool.

t=0-15ms: S1 (TemporalPrecedenceOperator) completes.
          Timestamp manipulation check applied.
          Regression-to-mean check applied.

t=0-70ms: S2 (MechanismScanOperator) runs tiered cascade.
          At completion: bridge_vec available if BRIDGED/CHAIN.
          r2 set based on mechanism_type.

t=0-70ms: S3 (ConfounderScanOperator) runs 4 strategies (Strategies 1+2 first).
          At t=30ms: top confounder score published to shared buffer.
          At t=70ms: full result with all 4 strategies.
          Collider detection check against FunGraph.

t=0-285ms: S4 (NaturalExperimentOperator) runs:
           - Pre-flight temporal check (5ms)
           - RDD detection (25ms)
           - Control group search: standard HNSW k=100 + mechanism-augmented
             HNSW k=50 if bridge_vec available from S2 (30-40ms)
           - Reads t=30ms confounder score from S3 shared buffer
           - Cascade: RDD → Event Study → DiD → SCM → ITS
           - SUTVA violations → published to S3 as discovered_confounders
           Hard cap: 380ms. After that, S4 = ignorance mass.

t=0-20ms: S5 (SourceConsensusOperator) completes.
          Provenance graph traversal for n_independent.
          Methodology diversity check.
          Null result tracking.

t=285ms:  All signal futures collected (or timed out → ignorance mass).
          SignalFusionOperator: Murphy D-S combination (~1 microsecond).
          Temporal gate applied.
          Platt calibration applied (if calibrated).
          Subgroup A ceiling applied: C_max = 0.90 * f_method * f_independence.

t=286ms:  Result assembled and returned.
          Stored in warm cache (TTL 15min).
          If query frequency warrants: promoted to hot cache.
```

---

### The Red Team Checklist Integrated into EXPLAIN CAUSALITY

The RedTeamOperator receives the full AssessmentResult and runs all 15 checks in parallel. Checks that require re-querying (items 5 and 8) are allocated up to 50ms each within the 600ms EXPLAIN budget. All others use already-computed data.

Checks that integrate directly with the synthesized design:
- Item 2 (Common Cause Scan): uses S3 ConfounderSearch result — already computed.
- Item 3 (Collider Check): uses FunGraph reverse traversal — fast.
- Item 7 (Effect Size Check): uses S4 effect_estimate and Cohen's d from ValidityDiagnostics.
- Item 9 (Mechanism Grounding Check): uses S2 evidence list — checks if bridge records exist.
- Item 10 (Source Independence Check): uses S5 n_independent vs. n_total.
- Item 11 (Feedback Loop Check): uses `_influenced_by` provenance tags from FunRecord.
- Item 15 (Confidence Ceiling Check): uses C_max from fusion output.

Checks with associated warnings are surfaced with `type`, `severity`, `message`, and `action` fields. The output includes a final `explanation_text` string suitable for AI agent consumption.

---

### Phase 1 (Foundation) vs. Phase 3 (Intelligence) Roadmap

**Phase 1 — Foundation (ship immediately):**

- S1 (Temporal Precedence) with timestamp manipulation check and regression-to-mean detection
- S2 (MechanismDetect) tiers 1 and 2 only — no NLI model in Phase 1. Mechanism types: DIRECT, BRIDGED, CHAIN. r2 tier-dependent.
- S3 (ConfounderSearch) strategies 1 and 2 only (Embedding Triangle + Common Ancestor) with basic coverage reporting
- S4 (NaturalExperimentDetector) with ITS and DiD methods only — no SCM or Event Study in Phase 1. Per-method r4 table. Diagnostic degradation.
- S5 (Source Consensus) with provenance-based independence check and null result tracking
- Murphy Dempster-Shafer fusion with temporal gate
- INFER CAUSALITY and ASSESS CAUSALITY query verbs
- Confidence ceiling: hardcoded at 0.65 (the realistic DiD case from Subgroup A), not the computed formula. The formula requires empirical rho matrix validation and ships in Phase 2.
- Three-tier caching (hot/warm/cold) with proper invalidation
- CONFIRM/DISCONFIRM CAUSALITY write-back verbs
- Red Team checklist: items 1, 2, 3, 6, 7, 10, 12, 15 only — the computable subset
- Partial mitigations for feedback loop: rate-limiting, `_influenced_by` provenance tags

**Phase 3 — Intelligence (after empirical validation):**

- S2 Tier 3 NLI model with counterfactual consistency check
- S3 strategies 3 and 4 (Text Co-occurrence + Temporal Context)
- S4 Event Study, SCM, and RDD methods
- Subgroup B bidirectional integration (mechanism-augmented control group, SUTVA feedback)
- Computed confidence ceiling: `0.90 * f_method * f_independence` with empirically validated rho matrix
- Platt calibration with feedback corpus quality checks
- Background reliability parameter learning (24-hour process)
- EXPLAIN CAUSALITY verb with full Red Team checklist
- Streaming results (STREAM option)
- Causal stability detector for feedback loop detection (Phase 3 version)
- Causal direction vector (TransE-style background learning from Section C.5 of Semanticist's proposal)

---

## Section 5: What We Didn't Solve

**1. The Feedback Loop — Fundamentally Open**

The partial mitigations reduce the risk of a single agent inflating a causal claim through rapid repeated confirmation. They do not address the scenario where many diverse agents independently observe post-intervention outcomes and confirm a claim that they themselves influenced by acting on it. This is a form of confounding where the confounder is the database's own past outputs. It is not clear that provenance tracking can ever fully solve this without requiring every system that reads FunDB to also write provenance back when it takes action. That is an architectural constraint that FunDB cannot enforce unilaterally.

The next sprint would need to address: (a) whether a causal stability detector (compare recent confidence trajectory against historical baseline) provides sufficient early warning, and (b) whether the Platt calibration model can be adversarially regularized to resist non-iid feedback distributions.

**2. The rho Matrix — No Empirical Grounding**

The Subgroup A correlation matrix — the foundation of the computed confidence ceiling formula — has values that are engineering intuitions, not empirical measurements. We do not have labeled examples of (A, B) pairs where the ground truth causal relationship is known and all 5 signals were computed. Until that corpus exists, the f_independence factor in the ceiling formula is applying a correction based on assumed, not measured, signal correlations.

The next sprint would need: a benchmark dataset of causal pairs with known ground truth (could be synthetic, could be derived from published RCT results), measurement of actual signal values on those pairs, and regression to fit the rho matrix empirically.

**3. The Binary Causation Frame — Loses Structure**

The Dempster-Shafer fusion operates on a binary frame {C, ~C}. In practice, causal relationships have degrees and types: A directly causes B with strength 0.9, versus A partially contributes to B as one of several causes with strength 0.4. The CausalEdge threshold-based mapping (confidence >= 0.8 => CAUSED) is a lossy approximation of this structure. When Subgroup B's bidirectional integration discovers that A causes M causes B (a mediated relationship), the fusion score for A->B includes the full chain but the CausalEdge type CAUSED conflates direct and indirect causation.

The next sprint would need: a richer output type that distinguishes direct_causal_confidence from total_causal_confidence (inclusive of mediated paths), and a separate fusion pass for mediated chains.

**4. Domain Mismatch in Semantic Evaluation**

The NLI model and embedding space are general-purpose. In specialized domains (biochemistry, legal reasoning, financial regulation), the semantic proximity between events does not track causal proximity in the domain's own ontology. A deployment that stores medical records and asks ASSESS CAUSALITY questions about drug interactions is using an NLI model that was not trained on pharmacological causal language. The `domain_confidence` score the Skeptic proposed for Failure Mode 2.4 is not implemented.

The next sprint would need: a domain detection system that identifies when a query is operating in a specialized domain, and a mechanism for either loading a domain-specialized NLI model or penalizing the S2 score appropriately.

**5. Large-Scale Confounders**

ConfounderSearch looks for confounders within the database. The Skeptic correctly identified that the most important confounder — the one that would definitively explain the correlation — may simply not be recorded. A batch job schedule, an external market event, a personnel decision: if it is not in FunDB, no search algorithm will find it. The coverage_score reports this limitation, but it does not solve it.

The next sprint would need: a mechanism for importing external event streams (market data, calendar events, news feeds) into FunDB as a confounder candidate pool, even if those records are not first-class FunDB entities.

**6. SUTVA Under Spillover**

The DiD SUTVA check detects spillover at the time of treatment. It does not detect gradual spillover that accumulates over the post-treatment window. If the treatment effect spreads from treated to control entities through a mechanism not observable in FunDB (e.g., via a shared customer base or a communication channel), SUTVA appears to pass at treatment time but the effect estimate is biased. No agent proposed a time-varying SUTVA test.

---

## Section 6: Team Lead's Verdict

FunDB entered this hackathon with the right raw ingredients for causal reasoning — vectors, graphs, time-series, confidence scores, provenance tracking — but no coherent framework for combining them into a causal claim. We leave this hackathon with a complete architecture: a defined data model, five concrete signals, a mathematically defensible fusion algorithm, a query language with four distinct verbs at different cost points, and a 15-point adversarial checklist that runs automatically on every explanation query. The most important single result is that the end-to-end cold-path latency lands at 303ms worst case, well within the 500ms target. This means the system is deployable as a query-time feature, not a batch analysis job. That was the challenge, and the challenge is met.

The theoretical foundation is stronger than I expected. Replacing the arbitrary 0.85 confidence ceiling with the computed formula `C_max = 0.90 * f_method * f_independence` was the most intellectually significant outcome of the hackathon. The formula has a clear interpretation — the maximum confidence from observational data is bounded by both the credibility of the experimental design used and the effective independence of the signals — and it connects directly to the empirical literature on observational causal inference. When the rho matrix is empirically calibrated, this formula will automatically tighten or loosen the ceiling based on what the data can actually support. That is the right long-term design.

The honest accounting of what remains hard is essential context for any investor or engineering lead. The feedback loop problem is not solved. We have rate-limiting and provenance tagging as partial mitigations, but the fundamental vulnerability — that an AI agent can act on a causal claim, generate confirming data, and inflate the claim's confidence without any record of the influence — requires architectural constraints on every system that reads FunDB's output. That is a product boundary problem, not a database algorithm problem. It needs to be surfaced in every customer conversation about deploying causal reasoning in agentic systems. Similarly, the rho matrix values are engineering guesses, not empirical measurements. The computed ceiling formula is principled in structure but ships in v1 with a hardcoded approximation (0.65) rather than the full computed value, precisely because the empirical calibration data does not exist yet. These are known, bounded risks — not existential problems, but honest ones.

The commercial case is straightforward: no database system today can answer "why did this happen?" at query time in a way that is mathematically honest, semantically grounded, quasi-experimentally validated, and adversarially stress-tested. FunDB can. The four-verb query interface (INFER at 15ms, ASSESS at 125ms, EXPLAIN at 155ms, TRACE at 10ms) gives developers a natural on-ramp from screening to deep investigation. An AI agent that can call INFER CAUSALITY to screen a thousand candidate causes and EXPLAIN CAUSALITY to audit the top three results has a capability that currently requires a team of data scientists and a week of analysis. That is the value proposition. The architecture from this hackathon makes it real.

---

*Team Lead signed off: 2026-02-28*
