# Agent Skeptic -- Proposal: Failure Modes, Attacks, and Safeguards

**Role:** Adversarial Thinker / Red Team
**Scope:** Stress-test each of the 5 causal signals, the combined fusion score, and propose concrete safeguards.

---

## Part A -- Attack Each Signal

---

### Signal 1: Temporal Precedence

**What it claims:** "A happened before B in a relevant context, therefore A may have caused B."

#### Failure Mode 1.1: Common Upstream Cause (Confounded Precedence)

**Attack:** A and B are both effects of an earlier event C, with A having a shorter lag. The system observes A before B and concludes A caused B.

**Concrete example:** A deployment pipeline triggers both a cache flush (Event A, instant) and a database migration (Event B, 10 minutes later). Error rates rise after B completes. FunDB observes "cache flush preceded errors" and infers a causal link, but the real cause is the migration.

**Safeguard:** For every (A, B) pair where temporal precedence is detected, run a **common-cause scan**: search for events C where C precedes both A and B and is correlated with both. If found, demote the temporal precedence score and flag C as a candidate confounder. Additionally, require that the temporal relationship be **consistent across multiple instances**, not just one occurrence.

#### Failure Mode 1.2: Timestamp Manipulation

**Attack:** An adversary (or a misconfigured system) writes events with fabricated timestamps, creating false temporal orderings.

**Concrete example:** A malicious agent backdates a "prediction" event to before an outcome event, making it appear prophetic. FunDB uses this to infer causality.

**Safeguard:** Implement **write-time vs. event-time divergence detection**. FunDB should track both `_created_at` (wall-clock time when the record was inserted, immutable, server-assigned) and `event_time` (user-provided timestamp). If `event_time` significantly precedes `_created_at`, the temporal precedence signal should be penalized. Flag any record where `_created_at - event_time > threshold` as having unreliable temporal ordering.

#### Failure Mode 1.3: Regression to the Mean Masquerading as Causation

**Attack:** An extreme observation triggers an intervention. The subsequent regression toward the mean is attributed to the intervention.

**Concrete example:** Error rates spike to 5x normal (random fluctuation). The team deploys a hotfix. Error rates return to normal. FunDB sees: hotfix preceded error decrease, and infers the hotfix fixed the problem. In reality, the error rate would have decreased anyway.

**Safeguard:** When the candidate cause was **triggered by an extreme value of the effect variable**, flag the relationship for **regression-to-the-mean risk**. Specifically: if the effect variable was more than 2 standard deviations from its mean at the time the cause occurred, emit a warning and reduce the temporal precedence score. Require that the same temporal pattern holds in instances where the effect was not at an extreme.

#### Failure Mode 1.4: Reverse Causation via Anticipatory Behavior

**Attack:** B causes A, but A is observed first because it is a preparatory or anticipatory action.

**Concrete example:** Hospitals stock up on flu medication (A) before flu season peaks (B). FunDB observes "medication stocking precedes flu cases" and infers that stocking medication might cause flu. In reality, expected flu cases drive medication stocking.

**Safeguard:** Implement an **anticipation detector**: if A is a known type of "preparatory" or "response" action (e.g., stocking, booking, planning) and B is the type of event that such preparations are made for, flag the temporal precedence as potentially reversed. This requires a semantic classification of event types -- integration with Signal 2.

---

### Signal 2: Semantic Mechanism

**What it claims:** "There is a plausible causal pathway from A to B based on semantic/domain understanding."

#### Failure Mode 2.1: Hallucinated Mechanisms (LLM Confabulation)

**Attack:** The LLM generates a detailed, confident, and completely wrong causal mechanism.

**Concrete example:** Query: "Did changing the database encoding from UTF-8 to ASCII cause the increase in Chinese user churn?" LLM response: "Yes, ASCII encoding cannot represent Chinese characters, causing display errors, leading to user frustration and churn." This sounds plausible, but if the application transcodes at the API layer, the database encoding is irrelevant to what users see.

**Safeguard:** **Ground mechanism claims in data, not in LLM reasoning.** For each link in the proposed causal chain (A -> X -> Y -> B), require that the intermediate events X and Y are actually observable in the database with appropriate temporal ordering. A mechanism without observable intermediaries should receive a lower score. Additionally, emit a `mechanism_source: "model_inferred"` vs. `mechanism_source: "data_grounded"` tag so consumers know the provenance of the explanation.

#### Failure Mode 2.2: Semantic Similarity Is Not Causal Similarity

**Attack:** Two events have high embedding similarity because they involve the same entities or domain, but have no causal relationship.

**Concrete example:** "Company X announces Q4 earnings" and "Company X stock price falls." These events are semantically similar (both about Company X, financial domain) but the earnings announcement may not have caused the price drop -- the drop may have been driven by a broader market selloff. Semantic similarity conflates topical relatedness with causal relatedness.

**Safeguard:** Implement a **causal-specific embedding space** or at minimum a **causal relation classifier** distinct from generic semantic similarity. The classifier should be trained on (cause, effect, non-cause) triples, not on semantic similarity. In the absence of such a classifier, the mechanism score should be explicitly labeled as "topical plausibility" rather than "causal plausibility."

#### Failure Mode 2.3: Narrative Fallacy (Hindsight-Driven Mechanism Construction)

**Attack:** Given that A and B happened, the LLM constructs a plausible-sounding chain. If A and (not B) had happened, the LLM would construct an equally plausible chain explaining why B did not happen.

**Concrete example:** "We changed the pricing model (A) and revenue increased (B)." LLM: "The new pricing model aligned incentives, driving higher conversion." But if revenue had decreased: "The new pricing model confused customers, driving churn." Both explanations are post-hoc rationalizations.

**Safeguard:** Implement a **counterfactual consistency check**: present the LLM with both "A then B" and "A then not-B" and ask it to evaluate the mechanism for each. If it provides high-confidence mechanisms for both outcomes, the mechanism signal is uninformative and should receive a score near 0.5 (maximum uncertainty), not a high score.

#### Failure Mode 2.4: Domain Mismatch

**Attack:** The semantic mechanism model was trained on general knowledge but is applied to a specialized domain where causal pathways differ from common understanding.

**Concrete example:** In biochemistry, inhibiting a gene can either decrease or increase the production of a protein depending on whether the gene encodes the protein directly or encodes an inhibitor of the protein. A general-purpose LLM might get the direction of causation wrong in such specialized contexts.

**Safeguard:** Track **domain metadata** for each collection. When evaluating mechanisms, compare the domain of the data against the known competency of the mechanism evaluator. Emit a `domain_confidence` score that reflects how well the evaluator understands the domain. If the domain is specialized and the evaluator is general-purpose, penalize the mechanism score.

---

### Signal 3: Confounder Exclusion

**What it claims:** "We searched for confounders and did not find any, increasing our confidence in the causal claim."

#### Failure Mode 3.1: Absence of Evidence vs. Evidence of Absence

**Attack:** The confounder search only examines variables present in the database. The most important confounder may simply not be recorded.

**Concrete example:** FunDB stores server metrics (CPU, memory, deploys) and observes that deploys cause error spikes. But the real cause is that the team deploys on Fridays when a batch job also runs, consuming resources. If the batch job schedule is not in FunDB, the confounder will never be found.

**Safeguard:** Every confounder exclusion result must report its **search coverage**: what percentage of the data universe was examined, how many candidate variables were tested, and what types of variables were NOT available. The output should include:
```
confounder_search: {
  variables_tested: 47,
  variables_available_in_db: 52,
  estimated_coverage: "server metrics only",
  domains_NOT_covered: ["human behavior", "external events", "business decisions"],
  confidence_in_completeness: 0.35
}
```

#### Failure Mode 3.2: Conditioning on a Collider (Berkson's Bias)

**Attack:** The confounder search inadvertently conditions on a collider variable, introducing a spurious association.

**Concrete example:** FunDB searches for confounders among "active users" (a collider -- both feature usage and user motivation cause users to be active). By restricting to active users, the system creates a negative association between feature usage and motivation, concluding that feature usage *decreased* motivation, when in fact both independently increase activity.

**Safeguard:** Before conditioning on any variable in the confounder search, run a **collider detection check**: does the candidate conditioning variable have multiple known causes in the causal graph? If so, warn that conditioning on it may introduce bias. Implement a simple DAG check using the existing graph index to detect potential collider structures.

#### Failure Mode 3.3: Overfitting the Confounder Search (Data Dredging)

**Attack:** Testing dozens of potential confounders without multiple comparison correction leads to false "confounders found" or false "no confounders exist."

**Concrete example:** FunDB tests 100 variables as potential confounders. Five show p < 0.05 correlation with both the cause and effect. These five are flagged as confounders and the causal claim is weakened. But 5 out of 100 at p < 0.05 is exactly what you'd expect by chance alone.

**Safeguard:** Apply **Benjamini-Hochberg false discovery rate (FDR) correction** to all confounder tests conducted in a single search. Report both the raw and adjusted p-values. The system should report: "Tested N candidate confounders with FDR-adjusted significance threshold of X."

#### Failure Mode 3.4: Mediation vs. Confounding

**Attack:** The system finds a variable M that is correlated with both A and B, and labels it a confounder. But M is actually a mediator (A causes M causes B), and "controlling for" M would incorrectly eliminate the real causal effect of A.

**Concrete example:** Marketing spend (A) causes website traffic (M) causes revenue (B). If FunDB controls for website traffic as a "confounder," it will conclude marketing has no effect on revenue, which is wrong -- the effect is real but mediated through traffic.

**Safeguard:** Before labeling a variable as a confounder, check its **temporal ordering** relative to A and B. If A precedes M and M precedes B, and A-M and M-B both have temporal precedence signals, classify M as a **potential mediator** rather than a confounder. Offer the user both the total effect (without controlling for M) and the direct effect (controlling for M), clearly labeled.

---

### Signal 4: Natural Experiments

**What it claims:** "We found a situation where the cause varied for reasons unrelated to the effect, approximating a randomized experiment."

#### Failure Mode 4.1: Instrument Invalidity (Exclusion Restriction Violation)

**Attack:** The "natural experiment" uses a variable Z as an instrument, assuming Z affects the outcome only through the cause. This assumption is wrong.

**Concrete example:** FunDB uses "data center location" as a natural experiment to study the effect of latency on user engagement. The system assumes data center location only affects engagement through latency. But data center location also affects content availability (CDN caching policies differ by region), which independently affects engagement. The exclusion restriction is violated.

**Safeguard:** For each proposed instrument, run a **direct-effect check**: does the instrument (Z) have any direct correlation with the outcome (B) after controlling for the proposed cause (A)? If so, the instrument is invalid. Additionally, maintain a **known-invalid-instruments blacklist** that accumulates over time as violations are discovered.

#### Failure Mode 4.2: Weak Instruments

**Attack:** The natural experiment uses an instrument that has only a tiny correlation with the cause, leading to massive variance in the estimated causal effect (weak instrument bias).

**Concrete example:** FunDB uses "day of the week" as a natural experiment for "deployment frequency causes errors." If the correlation between day-of-week and deployment frequency is weak (R-squared < 0.1), the resulting causal estimate will have enormous confidence intervals and may be biased toward the OLS estimate.

**Safeguard:** Compute the **first-stage F-statistic** for every proposed instrument. If F < 10 (the Stock-Yogo threshold), reject the instrument as too weak and report: "Natural experiment detected but instrument is too weak for reliable causal inference (F-statistic: X, minimum required: 10)."

#### Failure Mode 4.3: Non-Comparable Treatment and Control Groups

**Attack:** The "natural experiment" compares groups that differ in ways beyond the treatment, invalidating the comparison.

**Concrete example:** A configuration change was applied to servers in Region A but not Region B. FunDB compares outcomes between regions as a natural experiment. But Region A serves enterprise customers (high-value, low-volume) while Region B serves consumer customers (low-value, high-volume). The treatment groups are not comparable.

**Safeguard:** When identifying natural experiments, compute a **balance check**: are the treatment and control groups similar on all observable covariates? Report the standardized mean difference for each covariate. If any covariate has SMD > 0.25, warn that the groups are imbalanced and the natural experiment may be invalid. Present the balance table in the output.

#### Failure Mode 4.4: Small Sample Natural Experiments

**Attack:** The natural experiment has very few observations in one or both groups, making the causal estimate highly unreliable.

**Concrete example:** FunDB finds a "natural experiment" -- one day when the system was misconfigured vs. 364 days of normal operation. N=1 in the treatment group is not an experiment; it is an anecdote.

**Safeguard:** Enforce a **minimum sample size** for natural experiments. Both the treatment and control groups must have at least N observations (configurable, default N=30). If either group is smaller, demote the natural experiment score and report: "Natural experiment found but sample size is too small for statistical reliability (treatment n=X, control n=Y)."

---

### Signal 5: Source Consensus

**What it claims:** "Multiple independent sources agree that A causes B, increasing our confidence."

#### Failure Mode 5.1: Non-Independent Sources (Echo Chamber Effect)

**Attack:** Multiple sources that all derive from the same original source. Counting them as independent inflates confidence without adding evidence.

**Concrete example:** A research paper claims "X causes Y." Fifteen blog posts, three news articles, and two Wikipedia edits all cite this paper. FunDB counts 21 sources agreeing, assigns high consensus confidence. But there is really only 1 independent source -- the original paper.

**Safeguard:** Implement **source independence analysis** using provenance tracking. Build a citation/derivation graph among sources. If source B cites source A, B does not count as an independent source. The consensus score should be based on the number of **root sources** (sources with no tracked upstream), not total sources. Report both: "21 total sources, 1 independent root source."

#### Failure Mode 5.2: Shared Methodology Bias

**Attack:** Multiple sources use the same methodology, which has a systematic flaw. They agree not because the claim is correct, but because they all make the same mistake.

**Concrete example:** Five studies all use the same observational design to conclude that coffee consumption causes heart disease. They all fail to control for smoking (coffee drinkers smoke more). The studies "agree" but are all wrong for the same reason.

**Safeguard:** Track **methodology metadata** for sources. If all agreeing sources share the same methodology, reduce the consensus bonus. Diversity of methodology should be a factor in the consensus score: 3 sources using 3 different methods is much stronger than 3 sources using the same method. Implement a `methodology_diversity` sub-score.

#### Failure Mode 5.3: Sybil Attack (Manufactured Consensus)

**Attack:** An adversary inserts many records or sources that all assert the same causal claim, manufacturing artificial consensus.

**Concrete example:** A bot writes 1000 records into FunDB, each from a different "agent_id," all asserting "product_update_v3 caused user_satisfaction_increase." FunDB's consensus signal sees 1000 sources agreeing.

**Safeguard:** Implement **source credibility weighting**. New sources with no track record should receive low weight. Sources whose previous claims have been validated receive higher weight. Rate-limit the consensus impact of any single entity (IP, agent, API key) contributing more than K sources within a time window. Use the existing `_confidence` system to weight sources rather than counting them equally.

#### Failure Mode 5.4: Publication Bias / Confirmation Bias in Sources

**Attack:** Sources preferentially report positive findings. Negative findings ("X does NOT cause Y") are never published or stored.

**Concrete example:** Ten agents query FunDB over time. Three find evidence that "feature_A causes retention" and write this back as confirmed edges. Seven find no evidence and write nothing. FunDB eventually has 3 sources agreeing on the causal claim with zero contradictions, but the true ratio is 3:7 in favor of no effect.

**Safeguard:** Track **null results** explicitly. When a causal query returns "no significant causal relationship found," log this as a negative finding with the same rigor as positive findings. The consensus score should incorporate both confirmations and disconfirmations. Report: "3 sources confirm, 0 sources disconfirm, but 7 queries returned no finding (potential file-drawer problem)."

---

## Part B -- Attack the Combination

When all 5 signals are fused into a single causal confidence score, new failure modes emerge.

### Combined Failure Mode 1: Correlated Errors Across Signals

**Attack:** The 5 signals are not independent. If the temporal precedence is wrong because of a common upstream cause, the semantic mechanism is also likely wrong (the LLM builds its mechanism on the same spurious relationship), and the confounder search may fail for the same reason. Treating the signals as independent and multiplying/averaging their scores leads to overconfidence.

**Example:** Marketing campaign (A) and seasonal demand (confounding C) both cause sales increase (B). C also makes the mechanism sound plausible ("of course marketing increases sales"), makes the confounder search incomplete (seasonal effects are hard to detect with limited data), and makes the temporal precedence strong (campaigns launched before sales peaks by design). All 5 signals agree, but all are wrong for the same root reason.

**Safeguard:** Do NOT assume independence between signals. Model the correlation structure explicitly. At minimum: if the confounder search has low coverage (Signal 3 is weak), then the temporal precedence signal (Signal 1) and mechanism signal (Signal 2) should both be penalized, because their validity depends on the absence of confounders. Implement a **dependency penalty matrix** that reduces the combined score when the signals are known to be correlated in their failure modes.

### Combined Failure Mode 2: Gaming the Score

**Attack:** If the fusion formula is known, an adversary can engineer data that maximizes each signal individually.

**Example:** To make FunDB conclude "my product cured the disease":
1. Insert events with timestamps showing product use before recovery (temporal precedence).
2. Insert a plausible-sounding mechanism description (semantic mechanism).
3. Ensure no confounders are in the database by not logging other treatments (confounder exclusion).
4. Insert data about a "natural experiment" -- a region that used the product vs. one that did not (natural experiment).
5. Create multiple source accounts that all confirm the claim (source consensus).

**Safeguard:** Implement an **adversarial audit score** for causal claims. Check: (a) Are all 5 signals based on data from the same source/entity? (b) Was the data inserted recently (suggesting coordinated injection)? (c) Is the pattern unusually clean (real causal data is noisy)? If the adversarial audit flags a claim, require human review before it is stored as a confirmed causal edge. Add a `_manipulation_risk` metadata field.

### Combined Failure Mode 3: Overconfidence Through Aggregation

**Attack:** Each individual signal is honestly uncertain (e.g., 0.6 confidence). But the fusion formula, seeing 5 signals all at 0.6, produces a combined score of 0.9+. This is only valid if the signals are independent AND correctly calibrated. Neither is likely.

**Example:** 5 signals each at 0.6 confidence. Naive Bayesian fusion (assuming independence) yields:
```
P(causal | all 5 signals) = very high
```
But if the signals share 80% of their error structure, the true combined confidence may be closer to 0.65, not 0.95.

**Safeguard:** Implement a **confidence ceiling**: no fused causal score from observational data alone should exceed 0.85 (configurable), regardless of how many signals agree. Observational data cannot achieve the certainty of a randomized experiment. The system should emit: `"max_achievable_confidence": 0.85, "reason": "observational data only, no randomized experiment"`. Only interventional data (Tier 3, do-calculus) should be allowed to push confidence above this ceiling.

### Combined Failure Mode 4: Catastrophic Failure on Distributional Shift

**Attack:** The causal model works well on historical data but fails silently when the underlying data-generating process changes.

**Example:** FunDB has learned that "deploy events cause error spikes" with high confidence, based on 2 years of data. The team migrates to a new CI/CD pipeline with blue-green deployments that eliminate error spikes during deploys. FunDB continues to warn about deploy-caused errors for months because the historical evidence overwhelms the recent data.

**Safeguard:** Implement a **recency-weighted evidence decay**. More recent evidence should receive exponentially higher weight than older evidence. Additionally, implement a **drift detector**: if the causal relationship observed in the most recent N% of data differs significantly from the full historical data, emit a warning: `"causal_drift_detected": true, "recent_strength": 0.1, "historical_strength": 0.85, "message": "This causal relationship may no longer hold."` Recalculate the score using only recent data and present both the historical and recent estimates.

### Combined Failure Mode 5: Feedback Loops (Self-Reinforcing False Causation)

**Attack:** FunDB infers a causal relationship and stores it. An agent reads this and takes action based on it. The action generates data that further confirms the (false) causal relationship. The system has created a self-fulfilling prophecy.

**Example:** FunDB infers "user type X causes churn." An agent reads this and stops investing in type X users. Type X users, now receiving worse service, actually do churn. FunDB observes increased churn among type X and raises its confidence in the causal claim. But the causation was created by FunDB's own inference.

**Safeguard:** Implement **causal provenance tracking for actions**. When an agent reads a causal claim and takes an action based on it, that action should be tagged with `_influenced_by: [causal_claim_id]`. When the same causal claim is re-evaluated, data points that were generated by actions influenced by the original claim should be **excluded from the re-evaluation** (or at least flagged). This prevents the feedback loop from inflating confidence.

### Combined Failure Mode 6: Missing Signal Treated as "No Evidence" vs. "Evidence Against"

**Attack:** If one signal cannot be computed (e.g., no time-series data available for Granger causality), the fusion formula must decide whether to treat this as "neutral" or "negative." Treating it as neutral inflates the score (4 out of 4 instead of 4 out of 5). Treating it as negative deflates it unfairly.

**Safeguard:** Clearly distinguish between three states for each signal: SUPPORTS, CONTRADICTS, and NOT_COMPUTABLE. When a signal is NOT_COMPUTABLE, the fusion formula should reduce the **maximum achievable score** but not add negative evidence. The output should report: "4 of 5 signals evaluated (temporal data unavailable), maximum confidence reduced from 0.85 to 0.72."

### Combined Failure Mode 7: Contradictory Signals Masked by Averaging

**Attack:** Two signals strongly support causation (0.9) and two strongly oppose it (0.1), but the average looks moderate (0.5). The system reports moderate confidence instead of flagging the deep disagreement.

**Safeguard:** Compute the **variance across signals** in addition to the mean. If the variance exceeds a threshold, do NOT report a single fused score. Instead, report: "Causal signals are in strong disagreement. Signals supporting: [temporal precedence: 0.9, mechanism: 0.85]. Signals opposing: [natural experiment: 0.15, confounder found: 0.1]. No consensus reached." Disagreement between signals is itself informative and should not be hidden.

---

## Part C -- Red Team Checklist

Every causal claim produced by FunDB's Causal Engine should be automatically subjected to the following checks before being returned to the user or downstream agent.

### Checklist (ordered by priority)

```
RED TEAM CHECKLIST FOR CAUSAL CLAIMS
=====================================

[ ] 1. DIRECTION CHECK
      Could the causal direction be reversed? (B causes A instead of A causes B)
      Method: Check if B also temporally precedes A in some instances.
      If yes: Emit BIDIRECTIONAL_WARNING.

[ ] 2. COMMON CAUSE SCAN
      Is there a variable C that precedes both A and B and correlates with both?
      Method: Automated search across all available variables.
      If found: Emit CONFOUNDER_DETECTED, report C, reduce score.

[ ] 3. COLLIDER CHECK
      Are we conditioning on a collider variable?
      Method: Check filter/WHERE conditions against known causal structure.
      If yes: Emit COLLIDER_BIAS_RISK.

[ ] 4. SAMPLE BIAS CHECK
      Does the data represent the full population or a biased sample?
      Method: Check for survivorship bias indicators, filter conditions.
      If biased: Emit SAMPLE_BIAS_WARNING with description.

[ ] 5. AGGREGATION LEVEL CHECK
      Would the relationship reverse at a different aggregation level?
      Method: Test at individual, group, and population levels if possible.
      If reversal found: Emit SIMPSONS_PARADOX_DETECTED.

[ ] 6. MULTIPLE COMPARISONS CHECK
      How many hypotheses were tested to find this relationship?
      Method: Track all tests in current session, apply FDR correction.
      If uncorrected: Emit MULTIPLE_COMPARISONS_WARNING.

[ ] 7. EFFECT SIZE CHECK
      Even if statistically significant, is the effect practically meaningful?
      Method: Compute Cohen's d or equivalent effect size measure.
      If effect is trivially small: Emit TRIVIAL_EFFECT_WARNING.

[ ] 8. TEMPORAL STABILITY CHECK
      Does the relationship hold in recent data, not just historical data?
      Method: Compare causal strength in recent window vs. full history.
      If divergent: Emit CAUSAL_DRIFT_WARNING.

[ ] 9. MECHANISM GROUNDING CHECK
      Is the proposed mechanism supported by observable intermediate events?
      Method: Check if intermediate nodes in the causal chain exist in the DB.
      If ungrounded: Emit UNGROUNDED_MECHANISM_WARNING.

[ ] 10. SOURCE INDEPENDENCE CHECK
       Are the supporting sources truly independent?
       Method: Check provenance/citation graph for shared origins.
       If dependent: Emit SOURCE_DEPENDENCE_WARNING, report root count.

[ ] 11. FEEDBACK LOOP CHECK
       Was any evidence in this claim influenced by a previous version
       of the same claim?
       Method: Check _influenced_by provenance tags.
       If circular: Emit FEEDBACK_LOOP_DETECTED, exclude tainted evidence.

[ ] 12. SAMPLE SIZE CHECK
       Is there enough data to make this inference with the claimed confidence?
       Method: Compute statistical power for the observed effect size.
       If underpowered: Emit LOW_POWER_WARNING.

[ ] 13. ADVERSARIAL PATTERN CHECK
       Does the evidence look "too clean" or come from a concentrated source?
       Method: Check source diversity, insertion timing, data consistency.
       If suspicious: Emit MANIPULATION_RISK_WARNING.

[ ] 14. REGRESSION TO MEAN CHECK
       Was the "cause" triggered by an extreme value of the "effect"?
       Method: Check if effect variable was >2 SD from mean at cause time.
       If yes: Emit REGRESSION_TO_MEAN_WARNING.

[ ] 15. CONFIDENCE CEILING CHECK
       Is the final score exceeding what observational data can support?
       Method: Enforce max confidence ceiling for non-experimental evidence.
       If exceeded: Clamp score and report ceiling reason.
```

---

## Part D -- Mandatory Warnings and Disclaimers

FunDB should emit these warnings at different confidence levels and contexts.

### Tier 1: Always Emit (Every Causal Query Response)

```json
{
  "causal_disclaimer": "Causal inference from observational data is inherently uncertain. This assessment is based on statistical patterns and semantic analysis, not controlled experiments. Confidence scores represent the strength of available evidence, not the probability that the causal claim is true."
}
```

### Tier 2: Emit When Confidence Is High (score > 0.7)

```
"high_confidence_warning": "High causal confidence scores can still be wrong due to
unmeasured confounders, distributional shift, or correlated errors across signals.
Consider whether a randomized experiment or domain expert review is feasible before
making high-stakes decisions based on this assessment."
```

### Tier 3: Emit When Specific Risks Are Detected

Each checklist item that fails should produce a specific, actionable warning. Examples:

```
"warnings": [
  {
    "type": "CONFOUNDER_DETECTED",
    "severity": "high",
    "message": "Variable 'batch_job_schedule' correlates with both the proposed cause and effect. This may be the true cause.",
    "action": "Review whether 'batch_job_schedule' should be controlled for."
  },
  {
    "type": "LOW_POWER",
    "severity": "medium",
    "message": "Only 12 observations support this causal claim. Statistical power is 0.35 (recommended: >0.8).",
    "action": "Collect more data before relying on this inference."
  }
]
```

### Tier 4: Emit on High-Stakes Domains

When the collection is tagged as high-stakes (medical, legal, financial, safety), add:

```
"high_stakes_warning": "This causal claim involves a domain flagged as high-stakes.
Automated causal inference should NOT be the sole basis for decisions in this domain.
Human expert review is strongly recommended. FunDB's causal engine has not been
validated for [medical/legal/financial/safety] decision-making."
```

---

## Summary Table of All Failure Modes and Safeguards

| # | Signal | Failure Mode | Safeguard |
|---|--------|-------------|-----------|
| 1.1 | Temporal | Common upstream cause | Common-cause scan + require multiple instances |
| 1.2 | Temporal | Timestamp manipulation | Write-time vs event-time divergence check |
| 1.3 | Temporal | Regression to mean | Extreme-value trigger detection |
| 1.4 | Temporal | Reverse causation (anticipation) | Semantic event-type classification |
| 2.1 | Mechanism | LLM confabulation | Ground mechanisms in observable data |
| 2.2 | Mechanism | Semantic != causal similarity | Causal-specific classifier or explicit label |
| 2.3 | Mechanism | Narrative fallacy | Counterfactual consistency check |
| 2.4 | Mechanism | Domain mismatch | Domain competency scoring |
| 3.1 | Confounder | Unmeasured confounders | Search coverage report with gaps |
| 3.2 | Confounder | Collider bias | DAG-based collider detection |
| 3.3 | Confounder | Multiple comparisons | Benjamini-Hochberg FDR correction |
| 3.4 | Confounder | Mediator misidentified | Temporal ordering + mediator vs. confounder classification |
| 4.1 | Natural exp. | Invalid instrument | Direct-effect check + blacklist |
| 4.2 | Natural exp. | Weak instrument | First-stage F-statistic > 10 |
| 4.3 | Natural exp. | Non-comparable groups | Balance check with SMD < 0.25 |
| 4.4 | Natural exp. | Small sample | Minimum sample size enforcement |
| 5.1 | Consensus | Non-independent sources | Provenance graph, count root sources only |
| 5.2 | Consensus | Shared methodology bias | Methodology diversity score |
| 5.3 | Consensus | Sybil attack | Source credibility weighting + rate limiting |
| 5.4 | Consensus | Publication/confirmation bias | Track and count null results |
| C1 | Combined | Correlated errors | Dependency penalty matrix |
| C2 | Combined | Gaming the score | Adversarial audit score |
| C3 | Combined | Overconfidence | Hard confidence ceiling (0.85) |
| C4 | Combined | Distributional shift | Recency weighting + drift detector |
| C5 | Combined | Feedback loops | Causal provenance tracking |
| C6 | Combined | Missing signals mishandled | Three-state signal (support/contradict/N/A) |
| C7 | Combined | Disagreement masked | Variance check, refuse single score when high |
