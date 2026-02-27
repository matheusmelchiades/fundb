# SP1 Signal Fusion: Research Notes

**Agent:** Statistician
**Problem:** Combine 5 imperfect causal signals into a single causal confidence score (0.0--1.0)
**Constraint:** Must run at query-time inside a database engine (sub-50ms budget)

---

## 1. Problem Characterization

We have five heterogeneous signals, each providing partial evidence for or against the causal claim "A causes B":

| # | Signal | What it measures | Type of evidence | Pearl ladder rung |
|---|--------|-----------------|------------------|-------------------|
| S1 | Temporal precedence | A preceded B in time | Necessary condition | Rung 1 (association) |
| S2 | Semantic mechanism | An embedding-based explanation exists for WHY A->B | Plausibility of mechanism | Rung 2-adjacent (mechanism) |
| S3 | Confounder exclusion | No known third variable explains both A and B | Absence of alternative explanation | Rung 2 (intervention) |
| S4 | Natural experiments | Historical quasi-experiments where A varied naturally | Approximate intervention | Rung 2 (intervention) |
| S5 | Source consensus | Multiple independent sources agree A->B | Social/epistemic corroboration | Meta-evidence |

Key observations:
- These signals are **not interchangeable**: temporal precedence is a necessary condition, not evidence of the same kind as a natural experiment.
- Some signals are **conditionally dependent**: if a strong confounder is found (S3 fails), then temporal precedence (S1) and co-occurrence patterns become less informative.
- Signals can be **missing**: a query might only have 2 of 5 signals available.
- Signals can **conflict**: S1 and S2 might say "yes" while S4 says "no."

The fusion problem is therefore: **how to combine heterogeneous, partially dependent, potentially conflicting, and partially missing evidence into a calibrated belief**.

---

## 2. Frameworks Considered

### 2.1 Judea Pearl's Structural Causal Models (SCMs) and Do-Calculus

**Core idea:** Causation is defined via a directed acyclic graph (DAG) of structural equations. Causal effects are identified via do-calculus rules (deletion, insertion, exchange of observations and interventions) applied to the DAG.

**Pros for this problem:**
- Gold standard for defining what "causation" means mathematically.
- Clear semantics: P(Y | do(X)) is the quantity we ultimately want.
- The causal hierarchy (association / intervention / counterfactual) maps naturally to our signals.

**Cons for this problem:**
- Do-calculus requires a **pre-specified DAG**, which is exactly what we do NOT have. The whole point of SP1 is to infer causal strength without a prior causal graph.
- Computationally, checking identifiability from a DAG is polynomial, but building the DAG from data is NP-hard in general.
- Do-calculus tells us how to compute causal effects **given** a model, not how to fuse heterogeneous evidence **about** whether a causal relationship exists.

**Verdict:** Pearl's framework is the conceptual foundation (we borrow its causal hierarchy as a lens), but it does not directly solve the signal fusion problem. We use it to **interpret** our signals, not to **combine** them.

---

### 2.2 Rubin's Potential Outcomes Framework

**Core idea:** Define causal effect as Y(1) - Y(0) for each unit, where Y(1) is the outcome under treatment and Y(0) under control. The fundamental problem of causal inference is that we observe at most one of these. Solve via matching, propensity scores, or IV estimation.

**Pros for this problem:**
- Natural fit for signal S4 (natural experiments): the potential outcomes framework is exactly how we would analyze quasi-experimental variation.
- Provides clear estimands (ATE, ATT) that are well understood.
- Strong theory for handling confounders (S3) via ignorability assumptions.

**Cons for this problem:**
- Designed for estimating the **magnitude** of a causal effect from data, not for fusing heterogeneous evidence signals.
- Requires individual-level observational data with clear treatment/outcome variables. Our signals are already pre-processed summaries.
- Does not natively handle semantic or consensus evidence.

**Verdict:** Useful conceptual lens for signals S3 and S4. The natural experiment signal should be interpreted through this framework. But it is not a fusion framework.

---

### 2.3 Bradford Hill Criteria

**Core idea:** Sir Austin Bradford Hill (1965) proposed 9 criteria for assessing whether an observed association is causal:

1. Strength of association
2. Consistency (reproducibility)
3. Specificity
4. Temporality
5. Biological gradient (dose-response)
6. Plausibility
7. Coherence
8. Experiment
9. Analogy

**Mapping to our signals:**

| Hill criterion | Our signal |
|---------------|-----------|
| Temporality | S1 (temporal precedence) |
| Plausibility / Coherence | S2 (semantic mechanism) |
| Specificity (absence of alternatives) | S3 (confounder exclusion) |
| Experiment | S4 (natural experiments) |
| Consistency | S5 (source consensus) |

**Pros for this problem:**
- The philosophical alignment is **remarkably strong**. Our 5 signals are essentially a computational instantiation of Bradford Hill's criteria.
- Bradford Hill explicitly stated these are NOT a checklist requiring all items -- they are lines of evidence to be weighed together. This matches our "missing signals" requirement.
- Well-understood in epidemiology and accepted as a practical framework for causal reasoning from imperfect evidence.

**Cons for this problem:**
- Bradford Hill provided no mathematical formula for combining the criteria. He explicitly avoided one, arguing for expert judgment.
- The criteria are qualitative, not quantitative.
- No formal treatment of conflicting evidence.

**Verdict:** Excellent conceptual foundation. Our 5 signals ARE Bradford Hill criteria made computational. But we still need a mathematical combination rule.

---

### 2.4 Dempster-Shafer Theory of Evidence

**Core idea:** Generalization of Bayesian probability that allows explicit representation of **ignorance** (distinct from equal probability). Each source of evidence provides a "mass function" m(A) over subsets of the hypothesis space. Combination via Dempster's rule of combination. Key quantities: belief Bel(A), plausibility Pl(A), with Bel(A) <= P(A) <= Pl(A).

**Pros for this problem:**
- **Handles missing evidence naturally.** If a signal is absent, we simply do not include its mass function -- the remaining ignorance is explicitly tracked. This is strictly better than treating missing signals as neutral (Bayesian with uniform prior) or as zero (frequentist).
- **Handles conflicting evidence.** The normalization factor (1 - K) in Dempster's rule quantifies the degree of conflict between sources. High conflict is a useful diagnostic signal.
- **Does not require prior probabilities.** Unlike Bayesian combination, we do not need to specify P(A causes B) a priori, which would be arbitrary.
- **Computationally efficient.** For our binary hypothesis space {causal, not-causal}, combination is O(n) in the number of signals. Trivially sub-millisecond.
- **Theoretically grounded.** Shafer's "A Mathematical Theory of Evidence" (1976) provides axiomatic foundations. Widely used in sensor fusion, fault diagnosis, and multi-source intelligence.

**Cons for this problem:**
- Dempster's rule assumes **independence** of evidence sources. Our signals are NOT fully independent (e.g., temporal precedence and natural experiments share temporal data).
- The classic combination rule can produce counterintuitive results under high conflict (Zadeh's paradox). Various modifications exist (Yager, Murphy, Zhang) but add complexity.
- Choosing the mass functions for each signal requires careful calibration.
- Less familiar to most engineers than Bayesian methods.

**Verdict:** Strong candidate. The explicit handling of ignorance and missing data is exactly what we need. The independence assumption is a concern but manageable with careful signal design.

---

### 2.5 Bayesian Evidence Combination

**Core idea:** Start with a prior P(C) for the causal hypothesis. Each signal provides a likelihood ratio (Bayes factor). Update sequentially: posterior odds = prior odds * BF1 * BF2 * ... * BFn. Convert back to probability.

**Pros for this problem:**
- Extremely well understood and widely implemented.
- Clear interpretation: each signal's contribution is a multiplicative update to odds.
- Natural handling of missing evidence: simply omit the corresponding Bayes factor (equivalent to BF = 1, no update).
- Handles conflicting evidence gracefully: opposing signals partially cancel.
- Computationally trivial: n multiplications and one logistic transform.

**Cons for this problem:**
- **Requires a prior.** Choosing P(C) = 0.5 (maximum ignorance) is defensible but not neutral -- it assumes equal prior odds of causation vs. non-causation for any arbitrary pair of events.
- **Assumes conditional independence of signals given the hypothesis.** Same concern as Dempster-Shafer, but the Naive Bayes literature shows this works well in practice even with moderate violations.
- **Does not explicitly represent ignorance.** With all signals missing, Bayesian combination returns the prior. This conflates "we checked and found nothing" with "we haven't checked." Dempster-Shafer distinguishes these.
- **Requires calibrated likelihood ratios.** Each signal must provide P(signal | causal) / P(signal | not causal), which requires empirical calibration.

**Verdict:** Strong candidate. Simpler than Dempster-Shafer, well-understood by engineers, and the conditional independence assumption is the same.

---

### 2.6 Granger Causality

**Core idea:** Time series X "Granger-causes" Y if past values of X improve the prediction of Y beyond what past values of Y alone provide. Tested via F-statistic on nested autoregressive models.

**Pros for this problem:**
- Directly applicable to our signal S1 (temporal precedence) and partially to S4 (natural experiments with time-series data).
- Well-defined statistical test with p-values and effect sizes.

**Cons for this problem:**
- Only applicable to time-series data. Not a general fusion framework.
- "Granger causality" is a misnomer: it detects predictive precedence, not true causation.
- Cannot incorporate semantic or consensus evidence.

**Verdict:** Useful as a specific method for computing signal S1, not as the fusion framework.

---

### 2.7 Meta-Analysis / Evidence Synthesis

**Core idea:** Statistical methods (fixed-effects, random-effects models) for combining effect size estimates from multiple studies. Weighted combination where weights are inverse variances.

**Pros for this problem:**
- Purpose-built for combining evidence from heterogeneous sources.
- Handles heterogeneity via random-effects models (DerSimonian-Laird).
- Well-developed methods for assessing publication bias, funnel plots, etc.

**Cons for this problem:**
- Designed for combining **effect size estimates** (means, odds ratios) from comparable studies. Our signals are not effect sizes -- they are heterogeneous types of evidence.
- Assumes each study estimates the SAME underlying quantity. Our signals measure different ASPECTS of causation.
- Overkill for 5 signals; meta-analysis is designed for dozens to hundreds of studies.

**Verdict:** The philosophical idea of "weighing evidence by quality" is relevant, but the specific statistical machinery does not fit.

---

## 3. Framework Selection: Hybrid Dempster-Shafer + Bradford Hill Architecture

### 3.1 Rationale

After analyzing all frameworks, the optimal approach is a **hierarchical hybrid**:

**Layer 1 -- Conceptual Structure: Bradford Hill**
Our 5 signals map directly to Bradford Hill's criteria. This gives us the conceptual architecture: what each signal means and how it relates to the others. Crucially, Bradford Hill tells us that **temporality is a necessary condition** (not just one more piece of evidence) -- if A did not precede B, no amount of other evidence should yield high causal confidence.

**Layer 2 -- Mathematical Combination: Modified Dempster-Shafer**
For the actual numerical fusion, Dempster-Shafer provides the best fit because:
1. It explicitly represents **ignorance** when signals are missing (critical requirement).
2. It quantifies **conflict** between signals (critical for honesty).
3. It does not require an arbitrary prior on causation.
4. It is computationally trivial for a binary hypothesis space.

We use Murphy's modified combination rule (averaging mass functions before combining) to handle the Zadeh paradox and reduce sensitivity to the independence assumption.

**Layer 3 -- Calibration: Bayesian interpretation**
The final belief value Bel(causal) is interpreted as a calibrated probability. We apply a logistic calibration function (Platt scaling) trained on known causal/non-causal pairs from the database's feedback loop, ensuring the output is well-calibrated (a score of 0.8 means "causal" 80% of the time in practice).

### 3.2 Why Not Pure Bayesian?

The deciding factor is the **missing data semantics**. Consider:
- Scenario A: All 5 signals checked, all support causation. Score should be high.
- Scenario B: Only 1 signal checked (temporal precedence), it supports causation. Score should be moderate.
- Scenario C: All 5 signals checked, only temporal precedence supports causation. Score should be low.

In Bayesian combination, Scenario B and Scenario C both yield the same result if the 4 missing/negative signals have BF = 1 (neutral). But intuitively, B and C are VERY different: in B we are ignorant; in C we have evidence against.

Dempster-Shafer distinguishes these via the belief-plausibility interval:
- Scenario B: Bel(causal) = 0.4, Pl(causal) = 0.9 -- wide interval reflecting ignorance.
- Scenario C: Bel(causal) = 0.15, Pl(causal) = 0.25 -- narrow interval reflecting negative evidence.

This distinction is exactly what FunDB needs for honest uncertainty communication.

### 3.3 Why Not Pure Dempster-Shafer?

Pure D-S has two practical issues:
1. The independence assumption is violated (our signals share underlying data).
2. Engineers find mass functions and belief/plausibility intervals less intuitive than probabilities.

The Murphy averaging modification addresses (1), and the final Platt calibration step addresses (2) by mapping to a single probability while preserving the interval width as a separate "uncertainty" output.

---

## 4. Key References

1. **Pearl, J.** (2009). *Causality: Models, Reasoning, and Inference*. Cambridge University Press. -- Foundational framework for causal hierarchy.

2. **Shafer, G.** (1976). *A Mathematical Theory of Evidence*. Princeton University Press. -- Axiomatic foundation for Dempster-Shafer theory.

3. **Hill, A.B.** (1965). "The Environment and Disease: Association or Causation?" *Proceedings of the Royal Society of Medicine*, 58(5), 295-300. -- The original Bradford Hill criteria.

4. **Murphy, C.K.** (2000). "Combining Belief Functions When Evidence Conflicts." *Decision Support Systems*, 29(1), 1-9. -- Modified combination rule for handling conflict.

5. **Smets, P. & Kennes, R.** (1994). "The Transferable Belief Model." *Artificial Intelligence*, 66(2), 191-234. -- Open-world extension of D-S theory.

6. **Rubin, D.B.** (1974). "Estimating Causal Effects of Treatments in Randomized and Nonrandomized Studies." *Journal of Educational Psychology*, 66(5), 688-701. -- Potential outcomes framework.

7. **Platt, J.** (1999). "Probabilistic Outputs for Support Vector Machines." *Advances in Large Margin Classifiers*, 61-74. -- Platt scaling for probability calibration.

8. **Granger, C.W.J.** (1969). "Investigating Causal Relations by Econometric Models and Cross-Spectral Methods." *Econometrica*, 37(3), 424-438. -- Granger causality.

9. **Yager, R.R.** (1987). "On the Dempster-Shafer Framework and New Combination Rules." *Information Sciences*, 41(2), 93-137. -- Alternative combination rules.

10. **Fedorov, V., Mannino, F., & Zhang, R.** (2009). "Consequences of Dichotomization." *Pharmaceutical Statistics*, 8, 50-61. -- Analysis of information loss in discretization, relevant to mass function design.

---

## 5. Design Decisions for FunDB Context

### 5.1 Computational Budget

FunDB's causal path query target is <15ms p99 (from the architecture doc). Signal fusion adds to this budget. Our framework must:
- Accept pre-computed signal values (each signal is computed by its respective engine: temporal by the time-series engine, semantic by the vector engine, etc.)
- Combine them in O(n) time where n = number of signals (n <= 5)
- The combination itself should take <1ms

This is trivially achievable. The D-S combination for a binary frame with 5 mass functions requires ~20 floating-point operations.

### 5.2 Integration with FunDB Data Model

Each signal maps to existing FunDB infrastructure:
- S1 (Temporal): `_sys_from`, `_valid_from` timestamps + bitemporal MVCC
- S2 (Semantic): `_vectors` map + HNSW similarity search
- S3 (Confounder): `_edges` graph + `_timeseries` data
- S4 (Natural experiments): historical `_timeseries` + `_valid_time` ranges
- S5 (Source consensus): `_sources` array + `_supports` / `_contradicts` refs

The `CausalEdge` type in FunRecord already has a `strength` field (float32, 0.0-1.0). Our fusion score maps directly to this field.

### 5.3 Output Semantics

The fusion produces:
1. **Causal confidence** (float32, 0.0-1.0): maps to `CausalEdge.strength`
2. **Uncertainty width** (float32, 0.0-1.0): Pl(causal) - Bel(causal), communicates how much ignorance remains
3. **Conflict degree** (float32, 0.0-1.0): the D-S conflict factor K, flags when signals strongly disagree
4. **Signal breakdown** (array): individual contribution of each signal, for explainability

These four outputs together give FunDB the tools it needs for honest uncertainty communication, which is one of the evaluation criteria.
