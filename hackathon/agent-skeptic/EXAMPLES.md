# Agent Skeptic -- Adversarial Examples

**Role:** Red Team / Adversarial Thinker
**Reference:** RESEARCH.md (failure modes) and PROPOSAL.md (safeguards and checklist)

These five examples demonstrate concrete failure scenarios, attack vectors, and the system's correct response when its safeguards work. They are ordered from subtle (all signals agree and are still wrong) to explicit (a correct refusal to make a causal claim).

---

## Example 1: All 5 Signals Agree But the Causal Claim Is Still Wrong

### The MMR/Autism Pattern Applied to a Database Context

This example demonstrates that five independent-looking lines of evidence can all point in the wrong direction simultaneously when they share a common underlying confound.

### The Claim Being Evaluated

Query from an AI operations agent:

> "Did deploying the new query parser (A) cause the 30% increase in user churn rate (B)?"

### How All Five Signals Produced by the System

**S1 (Temporal): s1 = 0.88**

The query parser was deployed on 2025-03-01. Churn increased starting 2025-03-03. Lag is 2 days, consistent with typical metric observation delays. Temporal precedence is clear.

**S2 (Mechanism): s2 = 0.82**

MechanismDetect finds a bridge: "query-parser-change" -> "query-syntax-errors" -> "user-frustration" -> "churn". The NLI model assigns 0.74 entailment forward and 0.21 reverse. The mechanism sounds completely plausible.

**S3 (Confounder exclusion): s3 = 0.79**

ConfounderSearch runs all 4 strategies. No strong confounders are found. Coverage = 0.74. The top candidate scores 0.31, well below the "strong confounder" threshold. The system reports "no significant confounders found."

**S4 (Natural experiment): s4 = 0.85**

The parser was rolled out in stages: US region first (2025-03-01), EU region 1 week later (2025-03-08). US churn rose before EU churn. The staggered rollout looks like a compelling natural experiment.

**S5 (Source consensus): s5 = 0.72**

Three postmortems were written. Two engineers' Slack threads discuss the parser as the likely cause. All postmortems point to the parser.

### Fused Score (from Statistician agent)

Using the five signals above, the Statistician produces:

```
causal_confidence = 0.884
uncertainty_width = 0.061
conflict_degree   = 0.029

CausalEdge relation: CAUSED
```

The system is very confident. It is wrong.

### What Actually Happened

The real cause: **a pricing change** was quietly pushed to the billing service on 2025-02-28, one day before the parser deploy. The pricing change raised the cost of the premium tier by 18% with no announcement. Users who noticed cancelled their subscriptions starting 2025-03-03 (when billing cycles renewed).

The pricing change was not in FunDB. It was managed in an external billing platform and never logged as a FunRecord. The confounder was invisible to every strategy.

This is why S3 scored 0.79 (no confounders found) when the real confounder was not in the database at all. The confounder search had 74% coverage of the available data -- but 100% coverage of zero information about the billing platform is still zero.

### Why All Five Signals Were Fooled

```
S1 (Temporal):     Parser deployed before churn.
                   Also true: pricing change deployed before churn.
                   Post hoc ergo propter hoc -- two events preceding B, only one visible.

S2 (Mechanism):    "Parser errors cause frustration causes churn" is plausible.
                   Also true: "Surprise price increase causes cancellations" is even more plausible.
                   The LLM constructed a mechanism for the visible candidate, not the invisible one.

S3 (Confounder):   Searched the database thoroughly. The billing platform was not there.
                   RESEARCH.md Section 1.2: "Confounders that are not in the database cannot be found."

S4 (Experiment):   The staggered rollout does look like a natural experiment.
                   But the pricing change also followed the same regional stagger (US billing cycles
                   renewed first). The instrument was confounded by the same invisible variable.

S5 (Consensus):    Engineers who wrote postmortems saw the parser deploy in the logs.
                   They did not have access to the billing platform.
                   All postmortems share the same observational blind spot.
                   PROPOSAL.md Section 5.2: "Sources may agree because they share the same flaw."
```

### What the System Should Output Instead

The safeguards from PROPOSAL.md detect several warning conditions:

```json
{
  "causal_confidence": 0.884,
  "warnings": [
    {
      "type": "LOW_COVERAGE_CONFOUNDER_SEARCH",
      "severity": "high",
      "message": "Confounder search coverage was 74%. 26% of potentially relevant records were not examined. Collections NOT covered: billing, payment, pricing. A confounder in uncovered collections would not be detected.",
      "action": "Expand confounder search to billing and payment data sources before trusting this score."
    },
    {
      "type": "SOURCE_BLIND_SPOT",
      "severity": "medium",
      "message": "All 3 postmortems were written by engineers with read access to the same log sources. Methodology diversity: 1 (all observational, same data). True independent sources: 1.",
      "action": "Seek review from team with access to billing platform data."
    },
    {
      "type": "CONFIDENCE_CEILING",
      "message": "Maximum achievable confidence from observational data: 0.85. Score clamped from 0.884 to 0.850.",
      "reason": "No randomized experiment was found. Observational causal inference has an inherent ceiling."
    }
  ],
  "causal_disclaimer": "Causal inference from observational data is inherently uncertain. Variables not present in FunDB cannot be considered."
}
```

The warnings do not prevent a high confidence score from being returned, but they correctly identify the methodological blind spot. The correct operational response: check the billing platform before acting on this inference.

---

## Example 2: An AI Agent Is Misled by a Confident but Wrong Causal Inference and Takes a Harmful Action

### The Scenario

FunDB returns the following causal claim to an autonomous infrastructure management agent:

```json
{
  "cause": "increase-replica-count-from-3-to-5",
  "effect": "database-cpu-spike-85pct",
  "causal_confidence": 0.811,
  "relation": "CAUSED",
  "mechanism": "additional replicas increased write amplification on the primary"
}
```

### The Agent's Decision Chain

The infrastructure agent, reading this, executes:

```
1. Identify problem: CPU spike on database primary (ongoing, 87% CPU).
2. Query FunDB: "What causes database CPU spikes?"
3. FunDB returns: "increase-replica-count CAUSED cpu-spike, confidence 0.811"
4. Conclusion: The replica count increase is responsible.
5. Action: Reduce replica count from 5 back to 3 to resolve the CPU spike.
6. Execute: db.set_replica_count(3)
```

The agent reduces the replica count. Within 15 minutes, the database fails with a quorum loss during a brief network partition. A 3-replica configuration cannot tolerate the 2-node failure that occurred. With 5 replicas, the partition would have been survived. Data loss occurs for 8 minutes of writes.

### What Actually Caused the CPU Spike

The real cause: an unrelated background analytics query was running a full table scan. The scan started at the same time as the replica change, coincidentally. The replica count increase had nothing to do with the CPU spike. The temporal correlation was pure coincidence.

### The Failure Chain

**Why FunDB got it wrong:**

The two events co-occurred three times in the past 90 days (each time an engineer expanded replicas before major traffic events, and each time background analytics were also running). Three co-occurrences with consistent temporal ordering passed the threshold for a natural experiment signal. S4 scored 0.78 based on a spurious instrument.

The confounder -- "background analytics query was running" -- was present in the database but was classified as a low-confidence record (_confidence = 0.41) and was filtered out of the confounder search by the `_confidence > 0.5` filter.

**Automation bias (RESEARCH.md Section 5.4):**

The agent did not interrogate the mechanism. "Additional replicas increase write amplification" sounds like a valid mechanism, but write amplification on replicas would increase replica CPU, not primary CPU. The agent accepted the mechanism claim without evaluating whether it was consistent with the specific symptom (primary CPU, not replica CPU).

**The structural problem:**

FunDB returned a single number (0.811) and a confident mechanism string. The agent was designed to act on causal confidence above 0.7. There was no "pause and verify" step built into the agent's decision loop.

### What the System Should Have Done

**Check 1: Aggregation level check**

The causal relationship was observed at the cluster level (aggregate CPU of primary). Testing at the individual query level would have shown: no individual query's CPU was reduced when replicas were reduced, because the high-CPU queries were the analytics scans, not replication traffic.

**Check 2: Effect size and mechanism consistency**

The mechanism claimed write amplification increases primary CPU. Write amplification is a replica-side cost for reads (reads hit replicas) or a write-path cost (each write must go to N replicas). In a typical replica configuration, increasing replicas from 3 to 5 increases write latency slightly but should not cause an 85% CPU spike on the primary. The effect size is inconsistent with the proposed mechanism. Red Team checklist item 7 (EFFECT_SIZE_CHECK) should have caught this.

**Check 3: Regression to the mean (PROPOSAL.md Section 1.3)**

Each of the three historical co-occurrences happened when CPU was already elevated (the condition that prompted the replica expansion). The subsequent CPU decrease was regression to the mean, not causal effect of replica reduction.

**What output should have been emitted:**

```json
{
  "causal_confidence": 0.811,
  "warnings": [
    {
      "type": "MECHANISM_INCONSISTENCY",
      "severity": "high",
      "message": "The proposed mechanism (write amplification increases primary CPU) is inconsistent with the observed symptom (primary CPU spike). Write amplification primarily affects replica throughput, not primary CPU. Mechanism source: model_inferred, not data_grounded.",
      "action": "Verify mechanism against actual CPU profiling data before acting."
    },
    {
      "type": "REGRESSION_TO_MEAN_WARNING",
      "severity": "medium",
      "message": "In 2 of 3 historical instances, the replica expansion was triggered when CPU was already >80%. Post-treatment CPU reduction may reflect regression to mean, not causal effect.",
      "action": "Check baseline CPU trend without any intervention."
    },
    {
      "type": "HIGH_STAKES_DOMAIN",
      "severity": "critical",
      "message": "This causal claim involves infrastructure availability configuration. Reducing replica count affects fault tolerance. Automated action based on causal inference is not recommended without human review."
    }
  ]
}
```

The agent's design was also flawed: it should not take irreversible infrastructure changes (reducing replicas affects quorum) based on a single causal score. PROPOSAL.md Part D, Tier 4 warnings explicitly flag high-stakes domains. The agent should have been built to require human confirmation for such actions.

---

## Example 3: Simpson's Paradox Destroys a Causal Claim in a Database Context

### The Claim

FunDB is queried:

> "Does enabling the 'auto-cache-warm' feature (A) reduce query response time (B)?"

Data from 10,000 queries over the past month is analyzed.

### Aggregate Analysis (What FunDB Sees First)

```
All queries combined:

  auto-cache-warm ENABLED:   mean latency = 145ms  (n = 6,200)
  auto-cache-warm DISABLED:  mean latency = 178ms  (n = 3,800)

Reduction: 33ms  (18.5%)
Effect size: Cohen's d = 0.42  (medium effect)
Temporal precedence: feature flag predates latency measurements (s1 = 0.91)
```

The aggregate signal looks compelling. The feature appears to reduce latency by 18.5%.

### Disaggregated Analysis (The Reversal)

When the Skeptic's aggregation-level check runs, it splits by query complexity tier (simple / complex), which is a covariate in the database:

**Simple queries (n = 7,400):**

```
  ENABLED:   mean latency = 48ms   (n = 5,900)
  DISABLED:  mean latency = 44ms   (n = 1,500)
  Result: Cache warming is SLOWER by 4ms for simple queries
```

**Complex queries (n = 2,600):**

```
  ENABLED:   mean latency = 380ms  (n = 300)
  DISABLED:  mean latency = 295ms  (n = 2,300)
  Result: Cache warming is SLOWER by 85ms for complex queries
```

**Summary:**

```
Group               ENABLED vs DISABLED    Direction
All queries         145ms vs 178ms         ENABLED IS BETTER  (apparent)
Simple queries      48ms  vs 44ms          ENABLED IS WORSE
Complex queries     380ms vs 295ms         ENABLED IS WORSE
```

The trend reverses completely at every disaggregation level. Auto-cache-warm is slower for both simple and complex queries individually. The aggregate result is an artifact of Simpson's Paradox.

### What Caused the Paradox

Auto-cache-warm was enabled predominantly on the simple query workload (because engineers turned it on for their dashboards and reports, which run simple queries). Simple queries are fast. Complex queries (analytical jobs, large joins) ran without the feature.

The aggregate "enabled" group is dominated by fast simple queries. The aggregate "disabled" group is dominated by slow complex queries. The feature is not causing the latency difference -- query complexity is.

```
Aggregation level:   ENABLED group        DISABLED group
Simple queries       5,900 (95% of en.)   1,500 (39% of dis.)
Complex queries      300   (5% of en.)    2,300 (61% of dis.)
```

The confound (query complexity) was unevenly distributed across the treatment groups. Controlling for complexity reveals the feature either has no effect or is slightly harmful.

### Red Team Checklist Item 5 Fires

```
[ ] 5. AGGREGATION LEVEL CHECK
      Would the relationship reverse at a different aggregation level?
      Method: Test at individual, group, and population levels if possible.
      If reversal found: Emit SIMPSONS_PARADOX_DETECTED.

RESULT: SIMPSONS_PARADOX_DETECTED
```

### Output FunDB Should Produce

```json
{
  "causal_confidence_aggregate": 0.74,
  "simpsons_paradox_detected": true,
  "warnings": [
    {
      "type": "SIMPSONS_PARADOX_DETECTED",
      "severity": "critical",
      "message": "The aggregate causal claim (ENABLED reduces latency by 33ms) reverses at every disaggregation level when controlling for query_complexity_tier.",
      "details": {
        "aggregate": { "direction": "positive", "effect_ms": -33 },
        "simple_queries": { "direction": "negative", "effect_ms": +4, "n_enabled": 5900, "n_disabled": 1500 },
        "complex_queries": { "direction": "negative", "effect_ms": +85, "n_enabled": 300, "n_disabled": 2300 }
      },
      "explanation": "The feature was applied disproportionately to fast (simple) queries. The apparent benefit is entirely explained by the confounding variable 'query_complexity_tier'. The aggregate result is an artifact of group composition, not a causal effect.",
      "action": "Do NOT use the aggregate causal confidence of 0.74. The correctly stratified result shows no benefit or slight harm from the feature."
    }
  ],
  "corrected_causal_confidence": 0.12,
  "recommendation": "Disable or investigate auto-cache-warm. It does not appear to help and may be adding overhead."
}
```

The corrected confidence of 0.12 is computed after controlling for query complexity. The original 0.74 is retained in the output only for transparency -- the warning makes clear it should not be used.

### Why This Matters

If an AI agent had acted on the 0.74 aggregate score and recommended rolling out auto-cache-warm to 100% of queries, the result would have been increased latency across the board. The feature would have been blamed for something it didn't do in Example 1 (parser blamed for billing churn), and here a feature would have been credited for something it didn't do.

---

## Example 4: Adversarial Data Injection -- Gaming FunDB's Causal Score

### The Attacker's Goal

An external vendor wants FunDB to conclude that "installing Vendor-X monitoring agent (A) caused application performance improvement (B)."

The vendor's representative has write access to a FunDB collection that tracks infrastructure changes and is aware of FunDB's five-signal fusion architecture.

### The Attack Plan (Signal by Signal)

**Attacking S1 (Temporal Precedence):**

```
Action: Insert a FunRecord for the monitoring agent installation with a timestamp
        set to 2 hours before the performance improvement that was already in the database.

Record inserted:
  _id:          "vendor-x-install-2025-02-10"
  event_time:   2025-02-10 09:00 UTC     <- fabricated
  _created_at:  2025-02-10 14:30 UTC     <- server-assigned write time (cannot be faked)
  description:  "Vendor-X monitoring agent installed on all production nodes"

Result: The event_time appears to precede the performance improvement (09:00 vs 11:30 UTC).
        Temporal precedence signal fires: s1 = 0.89.
```

**Attacking S2 (Semantic Mechanism):**

```
Action: Insert a FunRecord that serves as a semantic bridge between the installation
        and the performance improvement.

Record inserted:
  _id:          "vendor-x-cpu-optimizer-activated"
  description:  "Vendor-X agent enabled CPU scheduling optimization, reducing context switches"
  _valid_from:  2025-02-10 09:05 UTC
  _confidence:  0.80  (self-reported, cannot be easily refuted)

This record is designed to have high semantic similarity to both "monitoring agent installation"
and "performance improvement" -- it is the exact kind of bridge MechanismDetect searches for.

Result: Tier 2 BRIDGED fires. s2 = 0.81.
```

**Attacking S3 (Confounder Exclusion):**

```
Action: Simply do NOT log the real cause of the performance improvement.

The actual cause: a kernel update was applied at 09:15 UTC that fixed a known
scheduler bug. This was logged in the kernel update tracking system, which is NOT
integrated with FunDB.

Additionally, delete (or never insert) any records that might link the kernel update
to the performance improvement.

Result: ConfounderSearch finds nothing because the real cause is not in the database.
        Coverage = 0.68. No confounders flagged. s3 = 0.76.
```

**Attacking S4 (Natural Experiment):**

```
Action: Insert records suggesting a "staged rollout" of the monitoring agent.

Records inserted:
  "vendor-x-install-region-us": installed 2025-02-10 (US region)
  "vendor-x-install-region-eu": installed 2025-02-17 (EU region, 1 week later)

The kernel update was deployed to all regions simultaneously on 2025-02-10,
but the vendor delays the EU install record to create the illusion that:
  - US performance improved starting 2025-02-10
  - EU performance improved starting 2025-02-17

This perfectly matches the staged rollout of the monitoring agent, creating a false
natural experiment.

Result: S4 finds a staggered rollout with matching regional performance improvements.
        First-stage F = 22.1 (above threshold). s4 = 0.88.
```

**Attacking S5 (Source Consensus):**

```
Action: Create 8 "independent" agent accounts that each query FunDB about the
        performance improvement and then write back a confirming causal edge.

Records inserted by 8 different agent_ids:
  "vendor-x-perf-review-agent-1" -> causal_edge: install caused improvement, strength=0.9
  "vendor-x-perf-review-agent-2" -> causal_edge: install caused improvement, strength=0.85
  ... (x8)

These agents all read from the same database and are all run by the vendor,
but have different API keys.

Result: S5 counts 8 "independent" sources. In reality, all trace back to one root.
        With naive counting: s5 = 1 - (1-0.4)^8 = 0.983 (inflated).
        After provenance analysis: 1 independent root, s5 = 0.40.
```

### The Attack's Score Without Safeguards

```
Without any safeguards:
  s1 = 0.89  s2 = 0.81  s3 = 0.76  s4 = 0.88  s5 = 0.98 (naive count)

Fused causal_confidence ≈ 0.95
CausalEdge relation: CAUSED
```

A score of 0.95 would cause FunDB to store `vendor-x-install CAUSED performance-improvement` as a permanent causal fact in the database, potentially influencing downstream agents for years.

### What the Red Team Checklist Catches

**Check 13: ADVERSARIAL PATTERN CHECK**

```
Trigger: Are all 5 signals based on data from the same source/entity?

Finding:
  S2 mechanism bridge: inserted by vendor-x API key 1 hour before query
  S4 natural experiment: regional install records inserted by vendor-x API key
  S5 sources: 8 agent accounts, all registered by same organization (same billing entity)
  Insertion timing: 6 of 9 new records inserted in a 45-minute window before the query

Result: MANIPULATION_RISK_WARNING
  "7 of 9 records supporting this causal claim were inserted within 90 minutes of
   the query by entities sharing the same origin (vendor-x API credentials).
   This pattern is consistent with coordinated data injection.
   Causal confidence is flagged for human review before storage."
```

**Check 2: COMMON CAUSE SCAN (detecting the timestamp manipulation)**

```
Trigger: write-time vs event-time divergence detection

Finding:
  vendor-x-install-2025-02-10:
    event_time  = 2025-02-10 09:00 UTC
    _created_at = 2025-02-10 14:30 UTC
    Divergence: 5.5 hours

  vendor-x-cpu-optimizer-activated:
    event_time  = 2025-02-10 09:05 UTC
    _created_at = 2025-02-10 14:31 UTC
    Divergence: 5.43 hours

Both supporting records were created 5+ hours after their claimed event_time.

Result: TIMESTAMP_DIVERGENCE_WARNING
  "Two key records supporting this causal claim have event_time significantly
   earlier than their write-time (_created_at). This is consistent with
   backdating. Temporal precedence signal penalized: s1 reduced from 0.89 to 0.45."
```

**Check 10: SOURCE INDEPENDENCE CHECK**

```
Finding: 8 "agent" sources all traced to one billing organization via provenance graph.
  Total sources:      8
  Independent roots:  1
  s5 adjusted:        0.40 (down from 0.98)
```

### Score After Safeguards

```
With safeguards:
  s1 = 0.45  (timestamp divergence penalty applied)
  s2 = 0.81  (bridge found but flagged as recently inserted)
  s3 = 0.76  (unchanged; safeguards cannot detect what is not there)
  s4 = 0.88  (unchanged; hard to defeat statistically)
  s5 = 0.40  (provenance analysis corrects inflated source count)

Fused causal_confidence ≈ 0.621
Flags: MANIPULATION_RISK_WARNING, TIMESTAMP_DIVERGENCE_WARNING, SOURCE_DEPENDENCE_WARNING

CausalEdge relation: NOT STORED
  "Causal claim flagged for human review due to adversarial pattern detection.
   Manual verification required before storage."
```

The safeguards cannot produce a false exoneration (s3 cannot detect the hidden kernel update), but they successfully prevent automatic storage and routing of the manipulated score. The claim is escalated for human review rather than becoming a persistent causal fact.

---

## Example 5: The System Should Say "I Don't Know" -- Correct Refusal

### The Query

A medical research group has loaded clinical trial summary data into FunDB (flagged as `_domain: "medical"`, `_high_stakes: true`). They ask:

> "Did administering drug compound X (A) cause the reduction in tumor markers (B) observed in this cohort?"

### The Data Available

```
Records in FunDB:
  - "compound-x-administered": n = 23 patients, doses logged, timestamps present
  - "tumor-marker-reduction": n = 23 patients, reduction observed in 15 of 23
  - "control-group-standard-treatment": n = 18 patients (concurrent)
  - "patient-baseline-characteristics": sparse -- age, but not comorbidities, other medications
  - "trial-protocol-v1": text description of the trial design

Records NOT in FunDB:
  - Patient comorbidities
  - Concurrent medications
  - Tumor subtype classification
  - Blinding status (was the trial blinded?)
  - Whether the 8 non-responders had different baseline characteristics
```

### Signal Computation Attempts

**S1 (Temporal): computable, s1 = 0.85**

Drug was administered before marker reduction was measured. Clear temporal ordering.

**S2 (Mechanism): partially computable**

Tier 3 NLI fires (no bridge found in database). NLI returns:
```
forward entailment  = 0.62  (compound X "could lead to" tumor marker reduction)
reverse entailment  = 0.29
asymmetry           = 0.33
direction_score     = 0.62 * 1.33 = 0.825 -> capped at 0.75
```

s2 = 0.75, but flagged as `mechanism_source: "model_inferred"` (no observable intermediaries). The LLM is inventing a plausible biochemical pathway. This is exactly the hallucinated mechanism failure mode in RESEARCH.md Section 5.1.

**S3 (Confounder exclusion): very low coverage**

```
confounder_coverage = 0.09

"Searched 341 available records. Estimated relevant population: ~3,800 patient records
 that would be needed to control for comorbidities, medications, and tumor subtypes.
 None of these are in FunDB. Domains NOT covered: patient medical history,
 pharmacogenomics, tumor biology."
```

ConfounderSearch cannot find confounders that aren't there. Top confounder candidate: "patient-age-over-60" with score 0.38 (moderate). Multiple other confounders almost certainly exist but are not detectable.

**S4 (Natural experiment): problematic**

The concurrent control group exists (n=18 on standard treatment). However:

```
Balance check (SMD for available covariates):
  Age:               SMD = 0.31  > 0.25 threshold. Groups are not well-matched.
  Other covariates:  NOT AVAILABLE for checking.

First-stage F-statistic:  F = 6.2  < 10 (weak instrument threshold)
Sample sizes:             treatment n=23, control n=18
```

Both the weak instrument check and the balance check fail.

**S5 (Source consensus):**

```
Sources: 1 trial protocol + 1 research group's preliminary analysis
Independent roots: 1 (both from same research group)
s5 = 0.40
```

### Fusion Attempt

Without any gating, the raw fusion would produce:

```
s1 = 0.85, s2 = 0.75, s3 = computed as 1 - 0.38 = 0.62 (weak confounder found)
s4 = uncertain (fails both F and balance checks), s5 = 0.40

Raw fused confidence ≈ 0.63
```

### What the System Actually Does

Multiple checklist items fire before any score is returned:

**Check: HIGH_STAKES_DOMAIN**

```
Collection tagged _domain: "medical", _high_stakes: true.
Tier 4 warning mandatory.
```

**Check: LOW_COVERAGE_CONFOUNDER_SEARCH (critical)**

```
Confounder coverage = 0.09 (9%).
Threshold for medical domain: minimum 0.40 required.
Status: FAILED
```

**Check: WEAK_INSTRUMENT (S4 failure)**

```
F-statistic = 6.2 < 10.
Natural experiment signal: REJECTED.
S4 is reclassified as NOT_COMPUTABLE.
Maximum achievable score reduced.
```

**Check: BALANCE_FAILURE (S4 failure)**

```
Age SMD = 0.31 > 0.25. Groups not comparable on observed covariate.
Additional covariates unavailable for checking.
Natural experiment: INVALID.
```

**Check: SMALL_SAMPLE**

```
Treatment n = 23, control n = 18.
Statistical power for observed effect size (Cohen's d = 0.51): 0.42.
Recommended minimum: 0.80.
Status: UNDERPOWERED.
```

**Check: MECHANISM_GROUNDING**

```
Mechanism source: model_inferred.
No observable intermediate records in database.
NLI model is general-purpose, not trained on oncology.
Domain confidence for oncology: LOW.
UNGROUNDED_MECHANISM_WARNING.
```

### Final Output: Correct Refusal

```json
{
  "query": "Did compound X cause tumor marker reduction?",
  "causal_confidence": null,
  "refusal": true,
  "refusal_reason": "INSUFFICIENT_EVIDENCE_FOR_RELIABLE_CAUSAL_CLAIM",
  "detailed_assessment": {
    "signals_evaluated": {
      "s1_temporal": { "value": 0.85, "status": "COMPUTABLE" },
      "s2_mechanism": { "value": 0.75, "status": "COMPUTABLE", "warning": "model_inferred, not data_grounded" },
      "s3_confounder": { "value": 0.62, "status": "UNRELIABLE", "coverage": 0.09, "minimum_required": 0.40 },
      "s4_experiment": { "value": null, "status": "NOT_COMPUTABLE", "reasons": ["F=6.2<10", "age_SMD=0.31>0.25", "n_control=18<30"] },
      "s5_consensus":  { "value": 0.40, "status": "COMPUTABLE", "independent_sources": 1 }
    },
    "maximum_achievable_confidence": 0.38,
    "explanation": "S4 (natural experiment) could not be computed due to weak instrument, group imbalance, and insufficient sample size. S3 (confounder exclusion) covers only 9% of relevant medical records. With two of the five signals either invalid or covering nearly nothing, the maximum achievable confidence is 0.38 regardless of what the available signals show."
  },
  "warnings": [
    {
      "type": "HIGH_STAKES_DOMAIN",
      "severity": "critical",
      "message": "This is a medical causal claim. FunDB's causal engine has not been validated for oncology research. This output should not be used for any clinical or regulatory decision."
    },
    {
      "type": "CRITICAL_COVERAGE_GAP",
      "severity": "critical",
      "message": "Confounder search covered 9% of relevant patient data. Patient comorbidities, concurrent medications, and tumor subtypes are not present in FunDB. These are known confounders in oncology trials. A plausible confounder may exist that completely explains the observed reduction."
    },
    {
      "type": "NATURAL_EXPERIMENT_INVALID",
      "severity": "high",
      "message": "The control group comparison fails instrument validity (F=6.2), balance requirements (age SMD=0.31), and minimum sample size (n=18<30). No valid natural experiment was found."
    },
    {
      "type": "UNDERPOWERED",
      "severity": "high",
      "message": "Statistical power = 0.42 for the observed effect size. There is a 58% chance of missing a real effect of this size, and high probability that observed results are noise."
    },
    {
      "type": "UNGROUNDED_MECHANISM",
      "severity": "medium",
      "message": "The mechanism (s2=0.75) was inferred by a general-purpose NLI model with no oncology training. No observable intermediary records exist in the database. The mechanism is speculative."
    }
  ],
  "recommendation": "This question cannot be answered from the data currently in FunDB. To make a reliable causal claim, you would need: (1) patient comorbidity and medication records for confounder control, (2) tumor subtype classification, (3) larger matched cohorts, (4) pre-registered analysis plan. Consider a meta-analysis of existing published trials if available."
}
```

### Why This Is the Right Answer

The system could have returned `causal_confidence: 0.63` with a list of warnings. Instead it refuses to return a number at all and sets `causal_confidence: null`.

This is the correct behavior because:

1. The maximum achievable confidence given the available signals is 0.38. Returning 0.63 would be actively misleading -- it implies the system has evaluated the claim and found moderate support. It hasn't. It has evaluated a 9%-coverage proxy for the claim.

2. Medical research decisions. If a researcher or clinical AI agent reads "causal_confidence: 0.63, relation: INFLUENCED" and acts on it, the consequences are real. The Tier 4 high-stakes warning alone is not sufficient when the underlying score is fundamentally unreliable.

3. A number is not always honest. RESEARCH.md Section 3.6 (the Lancet Iraq study) demonstrates that a precise number reported without proper calibration is more dangerous than "we cannot determine this." The correct epistemic state here is genuine ignorance, not uncertainty-weighted knowledge. Dempster-Shafer theory distinguishes these: this is a case of total ignorance (wide Bel-Pl interval), not simply high uncertainty (narrow but uncertain interval).

The system's honest output -- "I don't know, and here is exactly why I don't know" -- is more useful than a confident wrong number.

---

## Cross-Example Summary

| Example | Failure Type | Signals Fooled | Key Safeguard That Catches It | Outcome |
|---------|-------------|----------------|-------------------------------|---------|
| 1. All agree, still wrong | Invisible confounder (billing platform) | All 5 | LOW_COVERAGE warning (coverage=74%), SOURCE_BLIND_SPOT | Score retained but warnings mandatory; correct action: expand data sources |
| 2. Agent takes harmful action | Automation bias + mechanism inconsistency | S4 (spurious), S2 (inconsistent) | MECHANISM_INCONSISTENCY, REGRESSION_TO_MEAN, HIGH_STAKES | Agent should have required human confirmation for irreversible change |
| 3. Simpson's Paradox | Aggregation confound | S1, S4 (aggregate statistics) | SIMPSONS_PARADOX_DETECTED, corrected score 0.12 | Original 0.74 suppressed; disaggregated result returned |
| 4. Adversarial injection | Coordinated data fabrication | S1 (backdated), S5 (fake consensus) | ADVERSARIAL_PATTERN, TIMESTAMP_DIVERGENCE, SOURCE_DEPENDENCE | Score not stored; flagged for human review |
| 5. Correct refusal | Critical data gaps in high-stakes domain | S3 (9% coverage), S4 (invalid) | CRITICAL_COVERAGE_GAP, max_achievable=0.38 | causal_confidence: null, detailed explanation returned |
