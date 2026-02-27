# Agent Semanticist -- Algorithm Examples

**Algorithms:** MechanismDetect + ConfounderSearch
**Reference:** PROPOSAL.md, Parts A and B

These five examples trace the full execution path of both algorithms, showing exactly which tiers fire, what scores are computed, and what the output means for the Statistician agent's signal fusion.

---

## Example 1: MechanismDetect Finds a DIRECT Causal Edge (Tier 1, Fast Path)

### Scenario

Pair under examination:
- A: `deploy-tokenizer-v2` (FunRecord, deployed 2025-02-10 09:15 UTC)
- B: `error-spike-cjk-markets` (FunRecord, first observed 2025-02-10 09:24 UTC)

### Tier 1 Execution: DAG Index Lookup

```
FunCausal.query_path(
    source  = deploy-tokenizer-v2._id,
    target  = error-spike-cjk-markets._id,
    max_depth = 3
)
```

The FunCausal index finds:

```
Path found (depth 2):
  deploy-tokenizer-v2
    -[CAUSED, strength=0.92]->
  unicode-handling-regression
    -[CAUSED, strength=0.88]->
  error-spike-cjk-markets
```

This path was written by a previous incident postmortem. Both edges have relation=CAUSED with high strength.

### Score Computation

```
Chain score = product of edge strengths = 0.92 * 0.88 = 0.810
```

### Early Return

Tier 1 fires, algorithm exits immediately. Tiers 2 and 3 are never reached.

```
Latency: < 1ms
```

### Output

```json
{
  "mechanism_score": 0.810,
  "mechanism_type": "DIRECT",
  "mechanism_evidence": [
    {
      "from": "deploy-tokenizer-v2",
      "relation": "CAUSED",
      "strength": 0.92,
      "to": "unicode-handling-regression"
    },
    {
      "from": "unicode-handling-regression",
      "relation": "CAUSED",
      "strength": 0.88,
      "to": "error-spike-cjk-markets"
    }
  ],
  "tiers_executed": ["tier_1"],
  "latency_ms": 0.7
}
```

### Feed to Statistician

This result becomes signal S2 with value 0.810. Because the evidence is DIRECT (existing edges in the causal graph), the Statistician receives a high-quality signal with no cap applied. The reliability parameter r2 = 0.50 will still moderate it in the mass function:

```
m_S2({C}) = 0.50 * 0.810 = 0.405
```

### Note on Score Cap

PROPOSAL.md Section A.5 states Tier 1 has no cap. The score 0.810 is passed through unchanged because it comes from confirmed causal edges, not from similarity heuristics. The only reason it is not higher is that the two edges are not at maximum strength (0.92 and 0.88 rather than 1.0).

---

## Example 2: MechanismDetect Finds a BRIDGED Mechanism (Tier 2, Midpoint Search)

### Scenario

Pair:
- A: `ml-model-retrain-2025-01-20` (description: "Retrained recommendation model on Q4 data")
- B: `mobile-battery-drain-reports` (description: "Users reporting excessive battery drain on iOS app")

No existing causal edges connect these records. Tier 1 returns nothing.

### Tier 2 Execution: Semantic Bridge Search

**Compute midpoint and search radius:**

```
midpoint = (A.vec + B.vec) / 2
         = element-wise average of the two embedding vectors

distance = ||A.vec - B.vec|| = 0.72  (cosine distance)
radius   = 0.72 / 2 * 1.5 = 0.54    (expansion_factor = 1.5)
```

**HNSW search returns 50 candidate records within the hypersphere.** After filtering `_valid_from < B._valid_from`, 47 remain.

**Scoring candidates (top 3 shown):**

```
Candidate: "on-device-model-inference-added"
  sim_a = cosine_sim(candidate.vec, A.vec) = 0.81  (related to ML model)
  sim_b = cosine_sim(candidate.vec, B.vec) = 0.67  (related to mobile/battery)
  bridge_score = harmonic_mean(0.81, 0.67)
               = 2 * (0.81 * 0.67) / (0.81 + 0.67)
               = 2 * 0.5427 / 1.48
               = 0.733
  temporal_bonus: on-device-model-inference-added._valid_from = 2025-01-20 12:00
                  A._valid_from = 2025-01-20 09:00, B._valid_from = 2025-01-22 10:00
                  A < bridge < B? YES -> temporal_bonus = 1.2
  confidence_weight = bridge._confidence = 0.85
  final_score = 0.733 * 1.2 * 0.85 = 0.747

Candidate: "ios-background-refresh-policy"
  sim_a = 0.41, sim_b = 0.74
  bridge_score = harmonic_mean(0.41, 0.74) = 0.527
  temporal_bonus = 1.0  (precedes A, not between A and B)
  confidence_weight = 0.72
  final_score = 0.527 * 1.0 * 0.72 = 0.379

Candidate: "q4-data-distribution-shift"
  sim_a = 0.76, sim_b = 0.33
  bridge_score = harmonic_mean(0.76, 0.33) = 0.457
  Note: harmonic mean penalizes imbalance -- this candidate is much closer to A than B
  final_score = 0.457 * 1.0 * 0.79 = 0.361
```

**Top bridge:** `on-device-model-inference-added` with score 0.747.

0.747 > BRIDGE_THRESHOLD (0.55). Tier 2 fires.

### Why Harmonic Mean Matters Here

The arithmetic mean of (0.81, 0.67) would be 0.74. The harmonic mean is 0.733 -- close because the values are fairly balanced. But for the third candidate (0.76, 0.33), harmonic = 0.457 vs. arithmetic = 0.545. The harmonic mean correctly identifies that a record semantically close to only one end of the A-B pair is not a good bridge.

### Output

```json
{
  "mechanism_score": 0.747,
  "mechanism_type": "BRIDGED",
  "mechanism_evidence": [
    {
      "record": "on-device-model-inference-added",
      "description": "ML inference migrated from cloud to on-device after model retrain",
      "sim_to_cause": 0.81,
      "sim_to_effect": 0.67,
      "bridge_score": 0.733,
      "temporal_position": "between A and B",
      "confidence": 0.85
    }
  ],
  "score_cap_applied": true,
  "score_before_cap": 0.747,
  "score_after_cap": 0.747,
  "tiers_executed": ["tier_1 (no result)", "tier_2"],
  "latency_ms": 8.3
}
```

Note: 0.747 < 0.85 cap, so the cap is not binding here. Had the raw score been 0.91, it would have been capped at 0.85.

### Interpretation

The mechanism story: the model retrain (A) was deployed as on-device inference, which runs continuously in the background on iOS. This background processing caused the battery drain (B). The bridge record `on-device-model-inference-added` was in the database because an engineer logged the infrastructure change. Importantly, it was created between A and B in time, which added the temporal bonus.

---

## Example 3: MechanismDetect Finds Mechanism via NLI Only (Tier 3, Asymmetry Check)

### Scenario

Pair:
- A: `budget-freeze-2025-q1` (description: "Engineering hiring freeze announced for Q1 2025")
- B: `on-call-incident-rate-increase` (description: "On-call incident count per engineer increased 40% in Q1 2025")

No existing causal edges. No strong semantic bridges (the embedding spaces for "hiring freeze" and "incident rate" are too distant for Tier 2 to find a good midpoint bridge -- the HNSW search returns candidates, but the highest bridge_score is 0.42, below the 0.55 threshold).

Tier 2 also tries graph-augmented bridges (Tier 2b): 10 bridge candidates are checked for A-to-bridge and bridge-to-B paths. None have complete paths at max_depth=2.

Tier 3 is invoked.

### Tier 3 Execution: NLI-Based Inference

**Forward NLI:**

```
Premise:    "Engineering hiring freeze announced for Q1 2025"
Hypothesis: "This could lead to: On-call incident count per engineer increased 40% in Q1 2025"

NLI model output:
  P(entailment)    = 0.71
  P(contradiction) = 0.08
  P(neutral)       = 0.21
```

P(entailment) = 0.71 > 0.6. The reverse check is now triggered.

**Reverse NLI (asymmetry check):**

```
Premise:    "On-call incident count per engineer increased 40% in Q1 2025"
Hypothesis: "This could lead to: Engineering hiring freeze announced for Q1 2025"

NLI model output:
  P(entailment)    = 0.19   (incidents don't usually cause hiring freezes)
  P(contradiction) = 0.31
  P(neutral)       = 0.50
```

### Asymmetry Computation

```
asymmetry = nli_forward.entailment - nli_reverse.entailment
          = 0.71 - 0.19
          = 0.52

direction_score = nli_forward.entailment * (1 + max(0, asymmetry))
                = 0.71 * (1 + 0.52)
                = 0.71 * 1.52
                = 1.079

Capped at Tier 3 max: min(1.079, 0.75) = 0.75
```

The asymmetry bonus boosted the raw entailment from 0.71 toward the cap of 0.75. The logic: a strong forward entailment combined with a weak reverse entailment is a meaningful directional signal. "Fewer engineers causes more incidents" is much more plausible than "more incidents causes fewer engineers."

### What if Asymmetry Were Symmetric?

Suppose the reverse NLI had also returned 0.71 entailment (A and B mutually entail each other). Then:

```
asymmetry = 0.71 - 0.71 = 0.00
direction_score = 0.71 * (1 + 0) = 0.71
```

No boost. And more importantly, high symmetric entailment likely signals a common cause rather than a directed mechanism (both A and B might follow from something like "company downturn").

### Output

```json
{
  "mechanism_score": 0.75,
  "mechanism_type": "INFERRED",
  "mechanism_evidence": {
    "nli_forward": {
      "entailment": 0.71,
      "contradiction": 0.08,
      "neutral": 0.21
    },
    "nli_reverse": {
      "entailment": 0.19,
      "contradiction": 0.31,
      "neutral": 0.50
    },
    "asymmetry": 0.52,
    "direction_score_before_cap": 1.079,
    "direction_score_after_cap": 0.75
  },
  "best_tier2_bridge": {
    "record": "understaffed-teams-q1",
    "score": 0.42
  },
  "tiers_executed": ["tier_1", "tier_2", "tier_2b", "tier_3"],
  "latency_ms": 87.4
}
```

### Limitation Note

Tier 3 evidence is the weakest. The cap of 0.75 enforces this. When the Statistician agent uses this as S2, the mass function will be:

```
m_S2({C}) = 0.50 * 0.75 = 0.375
m_S2({~C}) = 0.50 * 0.25 = 0.125
m_S2({C,~C}) = 0.50
```

Half the mass remains in ignorance because the mechanism evidence is inferential, not data-grounded. PROPOSAL.md Section E.1 is explicit: "A high mechanism score means there is a plausible story for how A causes B, not that A definitely caused B."

---

## Example 4: ConfounderSearch Finds a Strong Confounder Using All 4 Strategies

### Scenario

Suspected causal pair:
- A: `marketing-campaign-launch` (2025-11-28, the day after Thanksgiving)
- B: `revenue-increase-q4` (2025-12-15 through 2025-12-31)

The question is whether the marketing campaign caused the revenue increase, or whether both are explained by the holiday shopping season.

### Strategy 1: Embedding Triangle Search

HNSW search centered on A.vec, cosine radius 0.6, filtered to `_valid_from < A._valid_from` (must precede the cause).

Top results scored with harmonic_mean(sim_zA, sim_zB):

```
Candidate: "black-friday-holiday-season-2025"
  sim_zA (to marketing campaign) = 0.78
  sim_ZB (to revenue increase)   = 0.82
  harmonic_mean(0.78, 0.82)      = 0.799
  balance = 1 - |0.78 - 0.82|   = 0.96   (very balanced: related to both)
  _confidence = 0.91
  triangle_score = 0.799 * 0.96 * 0.91 = 0.698

Candidate: "competitor-price-drop"
  sim_zA = 0.44, sim_ZB = 0.69
  harmonic_mean = 0.540, balance = 0.75, confidence = 0.72
  triangle_score = 0.540 * 0.75 * 0.72 = 0.291

Candidate: "app-store-feature-placement"
  sim_zA = 0.38, sim_ZB = 0.61
  harmonic_mean = 0.469, balance = 0.77, confidence = 0.65
  triangle_score = 0.469 * 0.77 * 0.65 = 0.235
```

`black-friday-holiday-season-2025` scores 0.698 via Strategy 1.

### Strategy 2: Common Ancestor Search

```
ancestors_A = FunGraph.reverse_traverse(marketing-campaign._id, depth=2)
  Returns: "q4-planning-2025", "cmo-budget-approval", "black-friday-holiday-season-2025"

ancestors_B = FunGraph.reverse_traverse(revenue-increase-q4._id, depth=2)
  Returns: "black-friday-holiday-season-2025", "consumer-spending-index-nov2025", "q4-market-conditions"

common_ancestors = intersection = {"black-friday-holiday-season-2025"}
```

Path strength from `black-friday-holiday-season-2025` to marketing-campaign:
```
  holiday-season -[INFLUENCED, 0.85]-> cmo-budget-approval -[CAUSED, 0.90]-> marketing-campaign
  path_strength = 0.85 * 0.90 = 0.765
```

Path strength to revenue-increase:
```
  holiday-season -[INFLUENCED, 0.92]-> revenue-increase-q4
  path_strength = 0.92
```

```
strategy_2_score = harmonic_mean(0.765, 0.92) = 0.836
```

### Strategy 3: Text Co-occurrence Mining

Documents semantically related to A (marketing campaign): contain terms "holiday", "seasonal", "Black Friday", "Q4 spending", "promotional"

Documents semantically related to B (revenue increase): contain terms "holiday", "seasonal", "Q4", "consumer demand", "Black Friday sales"

Shared high-TF-IDF terms: "holiday", "Black Friday", "seasonal demand", "Q4"

Records about "holiday" that predate both A and B include `black-friday-holiday-season-2025`:

```
tfidf in cause-context = 0.81
tfidf in effect-context = 0.88
strategy_3_score = 0.81 * 0.88 * 0.91 = 0.649
```

### Strategy 4: Temporal Context Search

Scan 30 days preceding A (`_valid_from < 2025-11-28`) for records with `_confidence > 0.5`:

```
time_window_start = 2025-10-29
time_window_end   = 2025-11-28
```

`black-friday-holiday-season-2025` was written `2025-10-01` (annual calendar event, logged each year):

```
sim_zA = 0.78, sim_ZB = 0.82 (above 0.3 threshold for both)

recency_weight = exp(-distance / (30/3))
              = exp(-(28 days) / 10)
              = exp(-2.8)
              = 0.061  (28 days before the window end is a long time away)
```

Wait -- this decay is too aggressive for the confounder. The system uses half_life = LOOKBACK_WINDOW / 3 = 10 days, and the holiday season record is 28 days before A, near the edge of the window.

```
strategy_4_score = harmonic_mean(0.78, 0.82) * 0.061 * 0.91 = 0.799 * 0.061 * 0.91 = 0.044
```

Low temporal strategy score because the record is near the boundary of the lookback window. The temporal strategy is better suited to detecting recent events (e.g., a server config change 2 days before both A and B).

### Multi-Strategy Fusion

```
black-friday-holiday-season-2025 found by:
  Strategy 1: score = 0.698
  Strategy 2: score = 0.836
  Strategy 3: score = 0.649
  Strategy 4: score = 0.044   (too old for temporal decay)

Max score = 0.836
Strategies that found it: 4 out of 4

Multi-strategy boost: 1 + 0.2 * (4 - 1) = 1.6x

Final confounder_score = 0.836 * 1.6 = 1.338 -> capped at 1.0
```

The cap at 1.0 is applied. The confounder is flagged as maximum-strength.

### Output

```json
{
  "confounders": [
    {
      "record": "black-friday-holiday-season-2025",
      "confounder_score": 1.0,
      "strategies_found_by": 4,
      "evidence": {
        "embedding_triangle": { "sim_to_cause": 0.78, "sim_to_effect": 0.82, "score": 0.698 },
        "common_ancestor":    { "path_to_cause_strength": 0.765, "path_to_effect_strength": 0.92, "score": 0.836 },
        "text_cooccurrence":  { "shared_terms": ["holiday", "Black Friday", "Q4"], "score": 0.649 },
        "temporal_context":   { "days_before_cause": 28, "score": 0.044 }
      },
      "interpretation": "The holiday shopping season preceded both the marketing campaign decision and the revenue increase, and has graph edges to both. This is almost certainly a confounder."
    }
  ],
  "coverage_score": 0.62,
  "search_metadata": {
    "candidates_evaluated": 183,
    "records_scanned": 341,
    "total_records_in_timewindow": 551
  }
}
```

### Consequence for S3 Signal

The Statistician agent's S3 signal is computed as:

```
top_confounder_score = 1.0
s3 = 1 - top_confounder_score = 0.0
```

When s3 = 0.0 enters the mass function conversion:

```
m_S3({C}) = 0.70 * 0.0 = 0.000
m_S3({~C}) = 0.70 * 1.0 = 0.700
m_S3({C,~C}) = 0.300
```

The confounder exclusion signal provides strong evidence AGAINST causation. Unless S4 (natural experiments) provides compelling interventional evidence despite the confounder, the combined causal confidence will be low. This is the correct behavior: a strong confounder should make the system skeptical of the causal claim.

---

## Example 5: High Mechanism Score + High Confounder Score -- What Is the Net Semantic Score?

### Scenario

Suspected pair:
- A: `new-ceo-announcement` (2025-09-01)
- B: `stock-price-increase-25pct` (2025-09-01 to 2025-09-15)

MechanismDetect finds a good mechanism. ConfounderSearch also finds a strong confounder. This is a genuinely ambiguous case.

### MechanismDetect Result

Tier 2 BRIDGED fires:

```
Bridge: "market-confidence-in-leadership"
  sim_to_cause = 0.79 (CEO announcement is about leadership)
  sim_to_effect = 0.73 (market confidence drives stock price)
  bridge_score = harmonic_mean(0.79, 0.73) = 0.759
  temporal_bonus = 1.2 (bridge concept exists between A and B)
  confidence_weight = 0.80
  final_score = 0.759 * 1.2 * 0.80 = 0.728 < cap (0.85)

mechanism_score = 0.728
mechanism_type  = BRIDGED
```

### ConfounderSearch Result

Strategy 2 (common ancestor) finds:

```
Candidate confounder: "q3-earnings-beat-2025" (reported 2025-08-29, 3 days before A)
  Path to A:  q3-earnings -> board-approval-of-new-ceo-hire -> new-ceo-announcement
              strength = 0.71 * 0.88 = 0.625
  Path to B:  q3-earnings -> investor-sentiment-boost -> stock-price-increase
              strength = 0.88 * 0.83 = 0.730
  ancestor_score = harmonic_mean(0.625, 0.730) = 0.674
  strategies found by: 3 (embedding_triangle also finds it, text_cooccurrence also finds it)
  multi_strategy_boost: 1 + 0.2*(3-1) = 1.4x
  final_confounder_score = 0.674 * 1.4 = 0.944
```

### Net Semantic Score Computation

From PROPOSAL.md Part C, Section C.2:

```
semantic_causal_score = mechanism_support * (1 - confounder_penalty * confounder_coverage)

  mechanism_support   = 0.728
  confounder_penalty  = max(confounder_scores) = 0.944
  confounder_coverage = 0.71  (good but not exhaustive search)

semantic_causal_score = 0.728 * (1 - 0.944 * 0.71)
                      = 0.728 * (1 - 0.670)
                      = 0.728 * 0.330
                      = 0.240
```

The net semantic score is 0.240. Despite a mechanism_score of 0.728 (which would normally suggest a plausible mechanism exists), the strong confounder nearly cancels it out.

### Why This Is the Right Behavior

Both things can be true simultaneously:
1. A plausible mechanism exists (CEO announcements do boost market confidence, which does affect stock prices).
2. A strong confounder exists (the Q3 earnings beat explains why both the CEO was announced AND the stock rose).

The net score of 0.240 correctly communicates: "There is a real pathway from A to B, but we have found a competing explanation that is almost as strong. The mechanism does not rule out the confounder."

This is exactly what epidemiologists mean when they say "plausibility" is necessary but not sufficient for causation (Bradford Hill criterion 6: plausibility helps but does not establish causation).

### Full Output to Statistician Agent

```json
{
  "mechanism": {
    "score": 0.728,
    "type": "BRIDGED",
    "bridge": "market-confidence-in-leadership"
  },
  "confounders": [
    {
      "record": "q3-earnings-beat-2025",
      "confounder_score": 0.944,
      "strategies": ["embedding_triangle", "common_ancestor", "text_cooccurrence"]
    }
  ],
  "confounder_coverage": 0.71,
  "net_semantic_causal_score": 0.240,
  "signals_for_statistician": {
    "s2_mechanism":  0.728,
    "s3_confounder": 1.0 - 0.944  // = 0.056, i.e. confounder nearly rules out clean causation
  },
  "warnings": [
    "CONFOUNDER_STRONG: q3-earnings-beat-2025 (score=0.944) found by 3 independent strategies. Net semantic score reduced from 0.728 to 0.240.",
    "AMBIGUOUS_CAUSATION: Mechanism exists but confounder offers near-equivalent explanation. Recommend natural experiment (S4) to disambiguate."
  ]
}
```

### What Should Happen Next

The Statistician agent receives s2 = 0.728 and s3 = 0.056. The low s3 (strong confounder found) will dominate the fusion because S3 has reliability r3 = 0.70, the second-highest. Unless S4 (natural experiment) provides strong interventional evidence -- e.g., showing stock movements in response to CEO announcements after controlling for earnings surprises -- the fused causal confidence will remain low despite the plausible mechanism.

This is the system working correctly. Semantic evidence raised a plausible mechanism; semantic evidence also raised a plausible counter-explanation; the statistical layer (S4) is the right arbiter.

---

## Cross-Example Summary

| Example | Tier | Latency | Score | Score Cap Applied | Key Mechanism |
|---------|------|---------|-------|-------------------|---------------|
| 1. Direct edge | Tier 1 | < 1ms | 0.810 | No (Tier 1 uncapped) | Known causal DAG path |
| 2. Bridged | Tier 2 | ~8ms | 0.747 | No (< 0.85) | Harmonic mean favors balanced bridges |
| 3. NLI only | Tier 3 | ~87ms | 0.750 | Yes (capped at 0.75) | Asymmetry bonus: A->B entailment >> B->A |
| 4. ConfounderSearch | All 4 strategies | ~62ms | 1.0 (confounder) | N/A | Multi-strategy convergence; 4x boost |
| 5. High both | Mixed | ~75ms | Net = 0.240 | N/A | Mechanism * (1 - confounder * coverage) |
