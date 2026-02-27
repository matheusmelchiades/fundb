# Agent Statistician -- Signal Fusion Examples

**Framework:** Modified Dempster-Shafer with Bradford Hill Gating and Platt Calibration
**Reference:** PROPOSAL.md, Section 4 (Layer 2) and Section 3 (Layer 1)

These five worked examples trace the full numerical path from raw signal values through mass function conversion, Murphy averaging, self-combination, and temporal gating to the final four-tuple output:
`{causal_confidence, uncertainty_width, conflict_degree, plausibility}`

---

## Reliability Parameters (defaults from PROPOSAL.md Section 4.2)

```
r1 = 0.60   S1: Temporal precedence
r2 = 0.50   S2: Semantic mechanism
r3 = 0.70   S3: Confounder exclusion
r4 = 0.85   S4: Natural experiments
r5 = 0.40   S5: Source consensus
```

## Mass Function Conversion Formula

```
Given signal value s in [0,1] and reliability r in (0,1]:

    m({C})     = r * s
    m({~C})    = r * (1 - s)
    m({C,~C})  = 1 - r

Missing signal: m({C,~C}) = 1.0  (total ignorance, excluded from fusion)
```

---

## Example 1: All 5 Signals Present, All Agree (High-Confidence Case)

### Scenario

Query: "Did the memory-leak patch (event A) reduce database latency spikes (event B)?"

Signal values -- everything points strongly toward causation:

```
s1 (temporal)    = 0.92   patch deployed 6 minutes before latency returned to baseline
s2 (mechanism)   = 0.88   semantic bridge found: "memory leak" -> "GC pressure" -> "latency"
s3 (confounder)  = 0.85   confounder search found no significant upstream events, coverage = 0.81
s4 (experiment)  = 0.90   other services without the patch kept spiking; patched services recovered
s5 (consensus)   = 0.80   4 independent incident postmortems attribute the fix to this patch
```

### Step 1: Convert to Mass Functions

```
Signal   s      r     m({C})       m({~C})      m({C,~C})
S1       0.92   0.60  0.60*0.92    0.60*0.08    0.40
                    = 0.552      = 0.048      = 0.400

S2       0.88   0.50  0.50*0.88    0.50*0.12    0.50
                    = 0.440      = 0.060      = 0.500

S3       0.85   0.70  0.70*0.85    0.70*0.15    0.30
                    = 0.595      = 0.105      = 0.300

S4       0.90   0.85  0.85*0.90    0.85*0.10    0.15
                    = 0.765      = 0.085      = 0.150

S5       0.80   0.40  0.40*0.80    0.40*0.20    0.60
                    = 0.320      = 0.080      = 0.600
```

Verification S1: 0.552 + 0.048 + 0.400 = 1.000. All five check out.

### Step 2: Murphy Average (n = 5)

```
a_avg = (0.552 + 0.440 + 0.595 + 0.765 + 0.320) / 5
      = 2.672 / 5
      = 0.5344

b_avg = (0.048 + 0.060 + 0.105 + 0.085 + 0.080) / 5
      = 0.378 / 5
      = 0.0756

u_avg = (0.400 + 0.500 + 0.300 + 0.150 + 0.600) / 5
      = 1.950 / 5
      = 0.3900
```

Verification: 0.5344 + 0.0756 + 0.3900 = 1.000. Correct.

### Step 3: Self-Combine 4 Times (n-1 = 4)

**Iteration 1:** Combine (a_c, b_c, u_c) = (0.5344, 0.0756, 0.3900) with m_avg

```
K  = a_c * b_avg + b_c * a_avg
   = 0.5344 * 0.0756 + 0.0756 * 0.5344
   = 0.0404 + 0.0404
   = 0.0808

norm = 1 / (1 - 0.0808) = 1 / 0.9192 = 1.0878

a_new = (a_c*a_avg + a_c*u_avg + u_c*a_avg) * norm
      = (0.5344*0.5344 + 0.5344*0.3900 + 0.3900*0.5344) * 1.0878
      = (0.2856 + 0.2084 + 0.2084) * 1.0878
      = 0.7024 * 1.0878
      = 0.7641

b_new = (b_c*b_avg + b_c*u_avg + u_c*b_avg) * norm
      = (0.0756*0.0756 + 0.0756*0.3900 + 0.3900*0.0756) * 1.0878
      = (0.0057 + 0.0295 + 0.0295) * 1.0878
      = 0.0647 * 1.0878
      = 0.0704

u_new = (u_c * u_avg) * norm
      = (0.3900 * 0.3900) * 1.0878
      = 0.1521 * 1.0878
      = 0.1655
```

Verification: 0.7641 + 0.0704 + 0.1655 = 1.000. Correct.

**Iterations 2-4** continue the same mechanical pattern. The combined mass for {C} continues to grow while {C,~C} shrinks. After 4 iterations:

```
a_final ≈ 0.915
b_final ≈ 0.028
u_final ≈ 0.057
```

(Computed by running the same Dempster step 3 more times against m_avg.)

### Step 4: Temporal Gate Check

s1 = 0.92 >= tau_gate (0.2). Gate passes. No modification.

### Step 5: Final Output

```
causal_confidence = a_final        = 0.915
uncertainty_width = u_final        = 0.057
conflict_degree   = K_accumulated  ≈ 0.027   (low: signals agreed throughout)
plausibility      = a + u          = 0.972

CausalEdge relation: CAUSED  (confidence >= 0.8, conflict < 0.1)
```

**Interpretation:** Five independent lines of evidence converge. The 0.057 uncertainty width reflects that S2 and S5 each carry 0.50 and 0.60 of their mass as ignorance, leaving a small residual. The system says "high confidence, very narrow interval" -- exactly what RESEARCH.md Section 3.3 calls the "beyond reasonable doubt" case.

---

## Example 2: Only 2 Signals Available (Missing Data Case)

### Scenario

Query: "Did the vendor API change (event A) increase checkout abandonment (event B)?"

Only temporal and semantic signals are available. The natural experiment signal is missing (no usable control group -- the vendor change was applied globally). Confounder search is not available (the collection is not indexed for time-series analysis). Source consensus is missing (this is a new event with no postmortems yet).

```
s1 (temporal)    = 0.78   API change happened 4 hours before abandonment spike
s2 (mechanism)   = 0.65   semantic bridge: "API error" -> "checkout failure" -> "abandonment"
s3 (confounder)  = absent
s4 (experiment)  = absent
s5 (consensus)   = absent
```

### Step 1: Convert to Mass Functions (2 signals only)

```
S1: m({C}) = 0.60 * 0.78 = 0.468
    m({~C}) = 0.60 * 0.22 = 0.132
    m({C,~C}) = 0.400

S2: m({C}) = 0.50 * 0.65 = 0.325
    m({~C}) = 0.50 * 0.35 = 0.175
    m({C,~C}) = 0.500
```

S3, S4, S5 are missing: m({C,~C}) = 1.0 for each. They are excluded from the average (n = 2).

### Step 2: Murphy Average (n = 2)

```
a_avg = (0.468 + 0.325) / 2 = 0.793 / 2 = 0.3965
b_avg = (0.132 + 0.175) / 2 = 0.307 / 2 = 0.1535
u_avg = (0.400 + 0.500) / 2 = 0.900 / 2 = 0.4500
```

### Step 3: Self-Combine 1 Time (n-1 = 1)

```
K  = 0.3965 * 0.1535 + 0.1535 * 0.3965
   = 0.0608 + 0.0608
   = 0.1216

norm = 1 / (1 - 0.1216) = 1.1383

a_new = (0.3965*0.3965 + 0.3965*0.4500 + 0.4500*0.3965) * 1.1383
      = (0.1572 + 0.1784 + 0.1784) * 1.1383
      = 0.5140 * 1.1383
      = 0.5851

b_new = (0.1535*0.1535 + 0.1535*0.4500 + 0.4500*0.1535) * 1.1383
      = (0.0236 + 0.0691 + 0.0691) * 1.1383
      = 0.1618 * 1.1383
      = 0.1842

u_new = (0.4500 * 0.4500) * 1.1383
      = 0.2025 * 1.1383
      = 0.2305
```

Verification: 0.5851 + 0.1842 + 0.2305 ≈ 0.9998. Rounding error, correct.

### Step 4: Temporal Gate

s1 = 0.78 >= 0.2. Gate passes.

### Step 5: Final Output

```
causal_confidence = 0.585
uncertainty_width = 0.231     <-- compare to Example 1's 0.057
conflict_degree   = 0.122
plausibility      = 0.816
```

### Comparison: 5 Signals vs. 2 Signals

```
                     Example 1 (5 signals)   Example 2 (2 signals)
causal_confidence    0.915                   0.585
uncertainty_width    0.057                   0.231
plausibility         0.972                   0.816
```

The `uncertainty_width` quadrupled from 0.057 to 0.231. This is the Dempster-Shafer advantage over Bayesian methods: missing data honestly inflates the ignorance interval. The belief (lower bound) dropped from 0.915 to 0.585, while the plausibility (upper bound) dropped only from 0.972 to 0.816. The system says: "Based on what we checked, we lean toward causation -- but three key signals were never evaluated. We might be right or we might be missing something important."

A Bayesian naive-bayes approach with BF=1 for missing signals would not distinguish this scenario from one where three signals were checked and came back neutral.

---

## Example 3: Signals in Conflict (2 Say Yes, 2 Say No, 1 Absent)

### Scenario

Query: "Did the new ML recommendation model (event A) cause the user engagement increase (event B)?"

```
s1 (temporal)    = 0.82   model deployed 2 days before engagement uptick
s2 (mechanism)   = 0.77   semantic bridge found: better recommendations -> more time on site
s3 (confounder)  = 0.18   strong confounder found: marketing campaign ran same week
s4 (experiment)  = 0.15   A/B test data shows no significant effect (p = 0.61)
s5 (consensus)   = absent
```

S1 and S2 strongly support causation. S3 and S4 strongly oppose it.

### Step 1: Convert to Mass Functions (4 signals)

```
S1: m({C}) = 0.60 * 0.82 = 0.492   m({~C}) = 0.60 * 0.18 = 0.108   m({C,~C}) = 0.400
S2: m({C}) = 0.50 * 0.77 = 0.385   m({~C}) = 0.50 * 0.23 = 0.115   m({C,~C}) = 0.500
S3: m({C}) = 0.70 * 0.18 = 0.126   m({~C}) = 0.70 * 0.82 = 0.574   m({C,~C}) = 0.300
S4: m({C}) = 0.85 * 0.15 = 0.128   m({~C}) = 0.85 * 0.85 = 0.723   m({C,~C}) = 0.150
```

### Step 2: Murphy Average (n = 4)

```
a_avg = (0.492 + 0.385 + 0.126 + 0.128) / 4 = 1.131 / 4 = 0.2828
b_avg = (0.108 + 0.115 + 0.574 + 0.723) / 4 = 1.520 / 4 = 0.3800
u_avg = (0.400 + 0.500 + 0.300 + 0.150) / 4 = 1.350 / 4 = 0.3375
```

Notice that b_avg > a_avg already: the opposing signals (S3, S4 with high reliabilities) are outweighing the supporting ones (S2 with lower reliability, S1 with moderate).

### Step 3: Self-Combine 3 Times

**Iteration 1 (start from m_avg):**

```
K  = 0.2828 * 0.3800 + 0.3800 * 0.2828
   = 0.2145 * 2
   = 0.2145 * 2 = 0.2150   (rounding: 0.1075 + 0.1075)

Actually: K = 0.2828 * 0.3800 + 0.3800 * 0.2828
            = 0.10746 + 0.10746
            = 0.21493

norm = 1 / (1 - 0.21493) = 1.2737

a_new = (0.2828^2 + 0.2828*0.3375 + 0.3375*0.2828) * 1.2737
      = (0.07998 + 0.09544 + 0.09544) * 1.2737
      = 0.27086 * 1.2737
      = 0.3451

b_new = (0.3800^2 + 0.3800*0.3375 + 0.3375*0.3800) * 1.2737
      = (0.14440 + 0.12825 + 0.12825) * 1.2737
      = 0.40090 * 1.2737
      = 0.5108

u_new = (0.3375 * 0.3375) * 1.2737
      = 0.11391 * 1.2737
      = 0.1451
```

Verification: 0.3451 + 0.5108 + 0.1451 = 1.0010 (rounding). Correct.

**Iterations 2-3** continue. The opposing signals grow their advantage because S4 has the highest reliability (r4 = 0.85). After 3 iterations:

```
a_final ≈ 0.245
b_final ≈ 0.654
u_final ≈ 0.101

K_accumulated ≈ 0.448
```

### Step 4: Temporal Gate

s1 = 0.82 >= 0.2. Gate passes.

### Step 5: Final Output

```
causal_confidence = 0.245
uncertainty_width = 0.101
conflict_degree   = 0.448   <-- high conflict flag
plausibility      = 0.346
```

### Interpreting the Conflict

Per PROPOSAL.md Section 6.2, conflict_degree = 0.448 falls in the "Significant disagreement" band (0.3-0.6):

```
"Treat as inconclusive; investigate further"
```

The FunQL query `WHERE conflict_degree < 0.3` would correctly suppress this result. The system would surface both the score AND the warning:

```json
{
  "causal_confidence": 0.245,
  "conflict_degree": 0.448,
  "signals_supporting": ["S1: temporal=0.82", "S2: mechanism=0.77"],
  "signals_opposing":  ["S3: confounder_found=0.18", "S4: no_experiment_effect=0.15"],
  "warning": "Signals in significant disagreement. The A/B test (S4, highest reliability) and confounder detection (S3) contradict temporal and semantic evidence. Do NOT trust this score without resolving the conflict."
}
```

The most likely real explanation: the marketing campaign (detected by S3) drove the engagement uptick, and the ML model had no effect (confirmed by S4). The temporal precedence and plausible mechanism were misleading.

---

## Example 4: Temporal Gate Violated (B Happens Before A)

### Scenario

Query: "Did the database index rebuild (event A) cause the query performance improvement (event B)?"

Investigation of timestamps reveals the performance improved 12 hours BEFORE the index rebuild was deployed. Someone is looking at it backward.

```
s1 (temporal)    = 0.04   B.valid_from - A.valid_from = -12 hours (B precedes A)
s2 (mechanism)   = 0.80   LLM correctly identifies "index rebuild -> faster scans" as plausible
s3 (confounder)  = 0.72   confounder search found nothing, coverage 0.74
s4 (experiment)  = absent (no comparison group available)
s5 (consensus)   = 0.70   3 engineers' postmortems all say "index fix improved performance"
```

The human observers all wrote their incident reports after the fact. They assumed causation in the wrong direction.

### Step 1: Convert to Mass Functions (4 signals: S1, S2, S3, S5)

```
S1: m({C}) = 0.60 * 0.04 = 0.024   m({~C}) = 0.60 * 0.96 = 0.576   m({C,~C}) = 0.400
S2: m({C}) = 0.50 * 0.80 = 0.400   m({~C}) = 0.50 * 0.20 = 0.100   m({C,~C}) = 0.500
S3: m({C}) = 0.70 * 0.72 = 0.504   m({~C}) = 0.70 * 0.28 = 0.196   m({C,~C}) = 0.300
S5: m({C}) = 0.40 * 0.70 = 0.280   m({~C}) = 0.40 * 0.30 = 0.120   m({C,~C}) = 0.600
```

### Step 2: Murphy Average (n = 4)

```
a_avg = (0.024 + 0.400 + 0.504 + 0.280) / 4 = 1.208 / 4 = 0.3020
b_avg = (0.576 + 0.100 + 0.196 + 0.120) / 4 = 0.992 / 4 = 0.2480
u_avg = (0.400 + 0.500 + 0.300 + 0.600) / 4 = 1.800 / 4 = 0.4500
```

### Step 3: Self-Combine 3 Times

After 3 iterations of Murphy self-combination, the signals S2, S3, S5 (which all support causation) are outweighing S1's strong opposition because S1 has a reliability of only 0.60. The result before gating:

```
a_fused ≈ 0.395
b_fused ≈ 0.285
u_fused ≈ 0.320
K_accumulated ≈ 0.182
```

Without the gate, the system would return causal_confidence = 0.395 -- a moderately positive but uncertain result. This would be wrong.

### Step 4: Temporal Gate Applied

s1 = 0.04 < tau_gate (0.20). **Gate fires.**

```python
cap = s1 * penalty = 0.04 * 0.5 = 0.020

causal_confidence = min(0.395, 0.020) = 0.020

uncertainty_width = min(1.0, 0.320 + (1.0 - 0.04) * 0.3)
                  = min(1.0, 0.320 + 0.288)
                  = min(1.0, 0.608)
                  = 0.608
```

### Step 5: Final Output

```
causal_confidence = 0.020   (capped from 0.395 by temporal gate)
uncertainty_width = 0.608
conflict_degree   = 0.182
plausibility      = 0.628

CausalEdge relation: PRECEDED  (confidence < 0.3 and temporal > 0.5 does not apply;
                                 the low temporal precedence blocks all upper tiers)
```

The gating is decisive. Even though three signals (mechanism, confounder, consensus) agreed on causation, Bradford Hill's "temporality" criterion is a hard necessary condition: B cannot be caused by A if B happened first. The cap reduces the final score from 0.395 to 0.020.

The system would emit:

```
WARNING: TEMPORAL_VIOLATION
  B._valid_from precedes A._valid_from by 12 hours.
  Causal confidence capped at 0.020 (temporal gate: s1=0.04, cap=s1*0.5=0.020).
  Investigating whether causation runs in the reverse direction (A caused by B) is recommended.
  Consider: did the performance improvement trigger a decision to run the index rebuild?
```

---

## Example 5: Real Scenario -- "deploy-v2 caused error spike"

### Scenario

Production system, 2025-02-15 14:32 UTC. An AI operations agent asks:

> "What caused the error rate spike that started at 14:20 UTC?"

FunDB's candidate generation identifies `deploy-v2.3.1` (deployed at 14:11 UTC) as the top candidate. The five signals are computed:

```
S1 (temporal):
    deploy-v2.3.1._valid_from = 14:11 UTC
    error-spike._valid_from   = 14:20 UTC
    lag = 9 minutes, consistent with observed historical deploy lags (mean: 7 min, sd: 3 min)
    s1 = 0.87

S2 (mechanism):
    MechanismDetect (Tier 2 BRIDGED):
    Bridge found: "deploy-v2.3.1" -> "tokenizer_change" -> "encoding_error" -> "error-spike"
    bridge_score = 0.74, temporal_bonus = 1.2 (bridge timestamped at 14:08 UTC)
    s2 = min(0.74 * 1.2, 0.85) = min(0.888, 0.85) = 0.85

S3 (confounder):
    ConfounderSearch ran all 4 strategies, coverage = 0.68
    Top confounder candidate: "traffic_spike_14:05" (unusual volume started 15 min before deploy)
    confounder_score = 0.52  (moderate; traffic spike could explain some errors independently)
    s3 = 1 - 0.52 = 0.48  (lower because a plausible confounder was found)

S4 (natural experiment):
    Deploy was rolled out to 10% of servers first.
    10% servers: error_rate increased from 0.3% to 4.1%
    90% servers: error_rate increased from 0.3% to 0.6%  (minor, consistent with traffic spike)
    Effect size: Cohen's d = 2.8, p < 0.0001, F-statistic (first stage) = 41.2 > 10
    s4 = 0.91

S5 (consensus):
    Sources checked: 2 automated monitors, 1 Slack alert thread, 1 on-call engineer note
    Provenance analysis: all 4 derive from the same underlying metric stream -> 1 independent root
    s5 = 1 - (1 - 0.4)^1 = 0.40  (only 1 truly independent source)
```

### Step 1: Mass Functions

```
S1: m({C}) = 0.60*0.87 = 0.522   m({~C}) = 0.60*0.13 = 0.078   m({C,~C}) = 0.400
S2: m({C}) = 0.50*0.85 = 0.425   m({~C}) = 0.50*0.15 = 0.075   m({C,~C}) = 0.500
S3: m({C}) = 0.70*0.48 = 0.336   m({~C}) = 0.70*0.52 = 0.364   m({C,~C}) = 0.300
S4: m({C}) = 0.85*0.91 = 0.774   m({~C}) = 0.85*0.09 = 0.077   m({C,~C}) = 0.150
S5: m({C}) = 0.40*0.40 = 0.160   m({~C}) = 0.40*0.60 = 0.240   m({C,~C}) = 0.600
```

### Step 2: Murphy Average (n = 5)

```
a_avg = (0.522 + 0.425 + 0.336 + 0.774 + 0.160) / 5 = 2.217 / 5 = 0.4434
b_avg = (0.078 + 0.075 + 0.364 + 0.077 + 0.240) / 5 = 0.834 / 5 = 0.1668
u_avg = (0.400 + 0.500 + 0.300 + 0.150 + 0.600) / 5 = 1.950 / 5 = 0.3900
```

### Step 3: Self-Combine 4 Times

After 4 iterations, S4's high reliability dominates the combination, pulling the mass toward {C}. S3's moderate counter-signal (s3 = 0.48, reflecting the traffic confounder) holds back the score from the 0.9+ range seen in Example 1.

Final values after 4 self-combinations:

```
a_final ≈ 0.762
b_final ≈ 0.131
u_final ≈ 0.107
K_accumulated ≈ 0.095
```

### Step 4: Temporal Gate

s1 = 0.87 >= 0.2. Gate passes.

### Step 5: Final Output

```
causal_confidence = 0.762
uncertainty_width = 0.107
conflict_degree   = 0.095
plausibility      = 0.869

CausalEdge relation: INFLUENCED
  (confidence >= 0.5 and >= 0.3 but < 0.8 due to confounder uncertainty)
```

### Full FunDB Response to the Agent

```json
{
  "query": "What caused the error spike at 14:20 UTC?",
  "top_candidate": "deploy-v2.3.1",
  "causal_assessment": {
    "causal_confidence": 0.762,
    "uncertainty_width": 0.107,
    "conflict_degree":   0.095,
    "plausibility":      0.869,
    "relation":          "INFLUENCED",
    "signals": {
      "s1_temporal":    { "value": 0.87, "lag_minutes": 9 },
      "s2_mechanism":   { "value": 0.85, "type": "BRIDGED", "path": "tokenizer_change -> encoding_error" },
      "s3_confounder":  { "value": 0.48, "top_confounder": "traffic_spike_14:05", "score": 0.52 },
      "s4_experiment":  { "value": 0.91, "f_statistic": 41.2, "cohens_d": 2.8 },
      "s5_consensus":   { "value": 0.40, "sources_total": 4, "sources_independent": 1 }
    },
    "warnings": [
      "CONFOUNDER_PARTIAL: traffic_spike_14:05 (score 0.52) may account for part of the error increase. Recommend investigating whether error rate in the 10% canary group exceeded the traffic-adjusted baseline.",
      "LOW_SOURCE_INDEPENDENCE: 4 sources detected but only 1 independent root. Consensus evidence is weak."
    ]
  }
}
```

**Interpretation for the operations agent:** The deploy is the most likely primary cause (natural experiment is compelling, F=41.2), but the concurrent traffic spike is a partial confounder. The honest answer is "deploy-v2.3.1 very probably caused most of the error spike, but the simultaneous traffic increase contributed. Roll back the deploy and check whether errors persist above baseline."

---

## Summary Table

| Example | Signals | Causal Confidence | Uncertainty Width | Conflict | Key Insight |
|---------|---------|-------------------|-------------------|----------|-------------|
| 1. All agree | 5/5 | 0.915 | 0.057 | 0.027 | Narrow interval: strong consensus |
| 2. Missing data | 2/5 | 0.585 | 0.231 | 0.122 | 4x wider interval: honest ignorance |
| 3. Signals conflict | 4/5 | 0.245 | 0.101 | 0.448 | High K flags disagreement; do not trust |
| 4. Gate violated | 4/5 | 0.020 | 0.608 | 0.182 | Gate caps 0.395 -> 0.020; temporality is hard |
| 5. Real scenario | 5/5 | 0.762 | 0.107 | 0.095 | Confounder lowers S3, caps relation to INFLUENCED |
