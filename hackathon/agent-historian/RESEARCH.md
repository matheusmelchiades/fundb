# SP3 Research: Quasi-Experimental Methods for Automated Natural Experiment Detection

**Agent:** Historian
**Sub-Problem:** SP3 — Natural Experiment Detection
**Date:** 2026-02-28

---

## 1. The Core Question

Given a candidate (cause, effect) pair stored in FunDB, how do we **automatically** find historical situations that function as quasi-experiments -- situations where the cause occurred naturally in some cases but not in others, allowing us to estimate the causal effect without a designed randomized experiment?

This is fundamentally the problem that observational causal inference has been wrestling with across epidemiology, economics, political science, and program evaluation for the past 50 years. The difference here is that we must do it **automatically** (no human specifying control groups), **at query time** (not after weeks of analysis), and **within a database engine** (not in R or Stata).

---

## 2. Survey of Quasi-Experimental Methods

### 2.1 Difference-in-Differences (DiD)

**How it works:** Compare the change in outcomes before vs. after treatment in a treatment group versus a control group. The key insight is that the *difference of differences* removes time-invariant confounders.

```
Effect = (Y_treatment_after - Y_treatment_before) - (Y_control_after - Y_control_before)
```

**Key assumption:** Parallel trends -- in the absence of treatment, the treatment and control groups would have followed the same trajectory. This is testable using pre-treatment data.

**Applicability to FunDB:** Excellent. FunDB's bitemporal data gives us exact before/after measurements. Vector similarity can find control groups (similar entities that did not receive the treatment). Multi-tenant data provides natural variation -- some tenants experienced the cause while others did not.

**Strengths:**
- Intuitive and widely understood
- Controls for all time-invariant unobservable confounders
- Parallel trends assumption is partially testable
- Computationally simple (means and differences)

**Weaknesses:**
- Parallel trends assumption is untestable in the counterfactual period
- Sensitive to composition changes in groups
- Requires a clear "before" and "after" period
- Does not handle staggered treatment timing well (though recent advances like Callaway & Sant'Anna 2021 address this)

**Best when:** There is a clear intervention time, a comparable untreated group, and pre-treatment data shows parallel trajectories.

---

### 2.2 Propensity Score Matching (PSM)

**How it works:** Estimate the probability (propensity) of receiving the treatment given observed covariates. Match treated units to untreated units with similar propensity scores. Compare outcomes between matched pairs.

```
Propensity score: e(X) = P(Treatment = 1 | X)
Match: For each treated unit i, find untreated unit j where |e(Xi) - e(Xj)| is minimized
Effect: Average of [Y_treated(i) - Y_control(j)] across matched pairs
```

**Key assumption:** Conditional independence (no unmeasured confounders) -- given the observed covariates, treatment assignment is as-if random. Also known as "selection on observables."

**Applicability to FunDB:** Strong. FunDB's vector embeddings are essentially high-dimensional covariate summaries. Matching on embedding similarity is conceptually equivalent to matching on a rich set of covariates. The HNSW index makes nearest-neighbor matching O(log n) per unit.

**Strengths:**
- Can handle many covariates (dimensionality reduction via the propensity score)
- Vector similarity in FunDB is a natural generalization
- Produces individual-level treatment effect estimates
- Can check covariate balance after matching

**Weaknesses:**
- Cannot control for unobserved confounders (the fundamental limitation)
- Requires a model for treatment assignment
- Matching discards unmatched units, reducing sample size
- The "curse of dimensionality" applies when matching on high-dimensional vectors (though HNSW mitigates the computational cost)

**Best when:** Rich covariate data is available, and the key confounders are observable and captured in the embedding.

---

### 2.3 Synthetic Control Method (SCM - Abadie, Diamond & Hainmueller 2010)

**How it works:** For a single treated unit, construct a "synthetic" control as a weighted combination of untreated units that best reproduces the treated unit's pre-treatment trajectory. The post-treatment divergence between the real unit and the synthetic control is the estimated effect.

```
Synthetic control: Y_synthetic(t) = Σ w_j * Y_j(t), where Σ w_j = 1, w_j >= 0
Weights chosen to minimize: Σ_pre |Y_treated(t) - Y_synthetic(t)|^2
Effect at time t: Y_treated(t) - Y_synthetic(t)
```

**Key assumption:** The treated unit can be well approximated by a convex combination of control units in the pre-treatment period. If the pre-treatment fit is good, the post-treatment divergence is plausibly causal.

**Applicability to FunDB:** Extremely relevant. FunDB stores time-series data natively, and the multi-tenant architecture means there are often multiple parallel entities (tenants, services, instances) that can serve as donor units. Vector similarity can pre-filter the donor pool to semantically similar entities.

**Strengths:**
- Works with a single treated unit (common in database incident analysis)
- Transparent weights reveal which donors contribute to the control
- Pre-treatment fit quality is a built-in diagnostic
- Inference via placebo tests (apply the method to each control unit as if it were treated)

**Weaknesses:**
- Requires substantial pre-treatment periods
- Cannot extrapolate beyond the convex hull of the donor pool
- Donor pool must not be affected by the treatment (no spillovers)
- Computationally involves constrained optimization (but small-scale: typically < 100 donors)

**Best when:** There is one treated entity and several similar but untreated entities, with good pre-treatment time-series data.

---

### 2.4 Regression Discontinuity Design (RDD)

**How it works:** When treatment is assigned based on whether a "running variable" exceeds a threshold, units just above and just below the threshold are nearly identical except for treatment. Comparing outcomes right at the threshold yields a local causal effect.

```
Effect = lim(x→c+) E[Y|X=x] - lim(x→c-) E[Y|X=x]
where c is the threshold and X is the running variable
```

**Key assumption:** Units cannot precisely manipulate their position relative to the threshold. The density of the running variable is continuous at the threshold (McCrary test).

**Applicability to FunDB:** Moderate to high. Many database-native scenarios involve thresholds: rate limits, autoscaling triggers, alert thresholds, quota boundaries, feature flag rollout percentages. FunDB can detect these discontinuities automatically by looking for threshold-based rules in the data.

**Strengths:**
- Very credible identification strategy (as-good-as-random near the threshold)
- Local treatment effect is well-defined
- Graphically intuitive (scatter plot with discontinuity)

**Weaknesses:**
- Only estimates a local effect (at the threshold), not the average effect
- Requires a clear running variable and threshold
- Needs many observations near the threshold for precision
- Bandwidth selection affects results

**Best when:** Treatment assignment is rule-based with a clear cutoff in a continuous variable.

---

### 2.5 Interrupted Time Series (ITS)

**How it works:** Model the trend of an outcome variable before an intervention, then check whether the intervention caused a change in level (immediate effect) or slope (trend effect) of the outcome.

```
Y(t) = beta_0 + beta_1*t + beta_2*D(t) + beta_3*(t - T_0)*D(t) + error
where D(t) = 1 if t >= T_0 (post-intervention)
beta_2 = immediate level change
beta_3 = change in trend
```

**Key assumption:** In the absence of the intervention, the pre-intervention trend would have continued unchanged. No other event coincided with the intervention that could explain the change.

**Applicability to FunDB:** Very high. This is the simplest method to apply in a database context -- it only requires the treated unit's own time-series. FunDB's time-series storage and bitemporal data make it straightforward to extract pre/post segments. The causal graph metadata can identify intervention points.

**Strengths:**
- Does not require a control group
- Works well with regularly sampled time-series (metrics data)
- Can distinguish level changes from trend changes
- Computationally trivial (segmented regression)

**Weaknesses:**
- No control for co-occurring events (the main threat)
- Assumes a stable pre-intervention trend
- Sensitive to autocorrelation in the time-series
- Seasonality must be handled

**Best when:** There is a clear intervention point, good pre-intervention data, and no plausible co-occurring changes.

---

### 2.6 Event Study Design

**How it works:** A generalization of DiD that estimates the treatment effect at multiple time points relative to the event, producing a dynamic treatment effect curve. This reveals whether effects are immediate, gradual, or temporary.

```
Y(i,t) = alpha_i + gamma_t + Σ_k beta_k * D(i,t-k) + error
where D(i,t-k) = 1 if unit i was treated k periods ago
beta_k = effect at k periods relative to treatment
```

**Key assumption:** Same as DiD (parallel trends), but the event study format makes the assumption visually testable -- pre-treatment coefficients should be near zero.

**Applicability to FunDB:** Strong. When the same type of event (e.g., "deploy") occurs at different times for different entities, an event study can pool all instances and estimate the typical dynamic effect. FunDB's embedding similarity can find "the same type of event" across the database.

**Strengths:**
- Reveals temporal dynamics of the effect
- Pre-treatment coefficients serve as a built-in placebo test
- Can handle staggered treatment timing
- Powerful when many instances of the cause exist

**Weaknesses:**
- Requires many treated units for stable estimation
- Binning of time periods is somewhat arbitrary
- All the same limitations as DiD for the parallel trends assumption

**Best when:** The same type of cause occurs repeatedly at different times, and we want to characterize the dynamic shape of the effect.

---

### 2.7 Instrumental Variables (IV)

**How it works:** Find a variable (the "instrument") that affects the treatment but has no direct effect on the outcome except through the treatment. Use the instrument to isolate the causal component of the treatment.

```
First stage:  Treatment = alpha + pi*Z + error_1   (Z predicts treatment)
Second stage: Y = beta_0 + beta_1*Treatment_hat + error_2  (predicted treatment → outcome)
```

**Key assumption:** The instrument (Z) is relevant (correlated with treatment), exogenous (uncorrelated with the outcome error), and satisfies the exclusion restriction (affects outcome only through treatment).

**Applicability to FunDB:** Lower for automatic use. Finding valid instruments typically requires domain knowledge. However, FunDB could potentially detect instrument candidates: variables that are correlated with the cause timing but not with the effect's pre-treatment trajectory. Cross-tenant variation is a natural source of instruments (e.g., different tenants' deployment schedules may be driven by their own internal calendars, which are plausibly exogenous to other tenants' error rates).

**Strengths:**
- Can handle unmeasured confounders (the only method in this list that can)
- Well-understood theoretical foundation

**Weaknesses:**
- Valid instruments are extremely hard to find automatically
- Weak instruments cause severe bias
- The exclusion restriction is untestable
- Two-stage estimation is computationally more complex

**Best when:** A plausible instrument is available, typically requiring domain knowledge. Not ideal for fully automated use.

---

## 3. Which Methods Work Best for Automated Database Use?

### 3.1 Ranking by Automability

| Method | Automability | Computational Cost | Data Requirements | Credibility |
|--------|-------------|-------------------|-------------------|-------------|
| ITS | Very High | O(n) per series | Single time-series | Moderate (no control) |
| Event Study | High | O(k*n) events | Multiple events of same type | High (built-in tests) |
| DiD | High | O(n) per comparison | Treatment + control group time-series | High (with parallel trends test) |
| Synthetic Control | High | O(d*T) optimization | Multiple donor series | High (fit quality as diagnostic) |
| Propensity Matching | Medium-High | O(n log n) via HNSW | Rich covariates (embeddings) | Medium (unobservables) |
| RDD | Medium | O(n) near threshold | Running variable + threshold | Very High (local) |
| IV | Low | O(n) per stage | Valid instrument needed | Very High (if valid) |

### 3.2 Recommended Strategy for FunDB

The optimal approach is **not to pick one method but to apply a cascade of methods** ordered by their data requirements, using each method's diagnostics to assess its own validity:

1. **Start with ITS** (always available -- only needs the effect time-series) to establish whether the effect timing is consistent with the cause timing.

2. **Upgrade to Event Study** if multiple instances of the same cause exist (found via vector similarity). This pools evidence across instances and provides a pre-treatment placebo check.

3. **Upgrade to DiD / Synthetic Control** if comparable untreated units exist (found via vector similarity on entities, with temporal filtering). This controls for time-varying confounders that affect both treated and untreated units.

4. **Apply Propensity Matching** as a robustness check, using embedding distance as a high-dimensional covariate match.

5. **Check for RDD opportunities** when threshold-based assignment rules are detected in the data.

6. **Report an "insufficient data" result** when no method's assumptions can be validated.

Each method produces its own confidence/validity score, and the final estimate combines them (see Agent Statistician's SP1 signal fusion framework).

---

## 4. Defining "Similar" Events for Automatic Control Group Construction

The hardest part of automating natural experiments is answering: "What counts as a comparable control unit?"

### 4.1 Three Dimensions of Similarity

**Semantic similarity (embedding distance):** Events with similar vector representations are semantically similar. "Deploy tokenizer v2" is semantically close to "Deploy tokenizer v1.9" and "Deploy NER model v3." FunDB's HNSW index provides O(log n) nearest-neighbor retrieval.

**Structural similarity (graph neighborhood):** Events that share graph relationships are structurally similar. Two deploys in the same service, triggered by the same CI/CD pipeline, affecting the same downstream dependencies. FunDB's graph traversal enables this.

**Temporal similarity (time-series shape):** Events occurring in similar temporal contexts (same day-of-week, similar load levels, similar seasonal patterns). FunDB's time-series storage and temporal indexes enable this.

### 4.2 Composite Similarity Score

For automatic control group construction, we define:

```
similarity(event_a, event_b) =
    w_semantic * cosine_sim(embed(a), embed(b))
  + w_structural * jaccard(graph_neighbors(a), graph_neighbors(b))
  + w_temporal * temporal_context_sim(a, b)
```

Where weights are configurable but default to emphasizing semantic similarity (w_semantic = 0.5, w_structural = 0.3, w_temporal = 0.2).

### 4.3 The "Never Identical" Problem

Similar events are never identical. "Deploy tokenizer v2" differs from "Deploy tokenizer v1.9" in version, code changes, timing, and context. The key insight from the matching literature is that **we do not need identical events -- we need events that are similar enough that their differences are not systematically correlated with the outcome.**

This is formalized as the "conditional independence assumption" in the Rubin causal model:

```
Y(0) ⊥ T | X
```

That is, potential outcomes under no-treatment are independent of treatment assignment, conditional on observable covariates X. In FunDB terms: if two deploys have similar embeddings, similar graph context, and similar temporal context, then any difference in their outcomes is attributable to the specific treatment, not to the context.

### 4.4 Balance Checking

After constructing a control group, we must verify that it is actually comparable. Standard balance diagnostics include:

- **Standardized mean difference** for each covariate: |mean(treated) - mean(control)| / pooled_SD < 0.1
- **Kolmogorov-Smirnov test** for distributional similarity
- **Pre-treatment outcome trajectory comparison** (the most powerful check)

In FunDB, this translates to:
1. Compare mean embedding distances within vs. between groups
2. Compare pre-treatment metrics trajectories (visual parallel trends check, formalized as a test statistic)
3. Compare graph neighborhood overlap

---

## 5. Key Assumptions and When They Break

### 5.1 No Interference (SUTVA)

**Assumption:** One unit's treatment does not affect another unit's outcome.

**When it breaks in databases:** Service A's deploy causes errors in Service B (cascading failures). Tenant A's usage spike affects Tenant B's performance (shared infrastructure).

**Detection:** Check whether control units' outcomes change at the treatment time. If they do, SUTVA is violated and the effect estimate is biased.

### 5.2 No Anticipation

**Assumption:** Units do not change behavior before the treatment based on knowledge that it is coming.

**When it breaks:** Teams prepare for a deploy by reducing traffic, scaling up infrastructure, or freezing other changes. The pre-treatment period is already affected by the upcoming treatment.

**Detection:** Look for outcome changes in the pre-treatment period for treated units (event study pre-treatment coefficients).

### 5.3 Parallel Trends (for DiD)

**Assumption:** Treated and control units would have followed the same trajectory without treatment.

**When it breaks:** Control units were selected *because* they are different (e.g., they never deploy risky changes because they are more conservative services). The trends diverge for reasons unrelated to the specific treatment.

**Detection:** Test for parallel pre-treatment trends. Longer pre-treatment periods give more power to detect violations.

### 5.4 No Unmeasured Confounders (for matching)

**Assumption:** All variables that jointly affect treatment and outcome are observed and included in the matching.

**When it breaks:** An unobserved factor (e.g., a team's internal stress level) simultaneously causes both the rushed deploy and the errors. The deploy and errors are correlated but not causally linked -- the stress is the common cause.

**Detection:** Sensitivity analysis (Rosenbaum bounds) -- how large would an unobserved confounder need to be to explain the result? If the result is sensitive to small confounders, it is less credible.

---

## 6. How Other Fields Solve This

### 6.1 Epidemiology

Epidemiologists face the same fundamental problem: they cannot randomize exposure to diseases or pollutants. Their solutions:

- **Case-control studies:** Find people with the disease (cases) and match them to similar people without the disease (controls). Compare exposure rates. Directly applicable to FunDB: find "incident" records and match to "non-incident" records.
- **Cohort studies:** Follow a group over time, comparing those who are exposed to those who are not. This is the event study / DiD approach.
- **Bradford Hill criteria (1965):** A set of nine criteria for evaluating causal claims from observational data: strength of association, consistency, specificity, temporality, biological gradient, plausibility, coherence, experiment, and analogy. These can be adapted as a multi-criteria scoring system for FunDB's causal assessments.

### 6.2 Econometrics

Economists have pioneered most of the methods listed above. Key insights for FunDB:

- **Credibility revolution (Angrist, Imbens, Card):** The shift from structural models to design-based approaches. The quality of the comparison group matters more than the sophistication of the statistical model. For FunDB, this means investing in control group construction (vector similarity, temporal matching) rather than complex models.
- **Local average treatment effect (LATE):** Most methods estimate effects for a specific subgroup, not the whole population. FunDB should report which subgroup the estimate applies to.
- **Sensitivity analysis:** Altonji, Elder & Taber (2005) and Oster (2019) provide methods to assess how robust results are to unobserved confounders. These can be automated.

### 6.3 Tech Industry (A/B Testing Infrastructure)

Companies like Netflix, Uber, and Microsoft have built massive experimentation platforms. Relevant insights:

- **Synthetic control for metrics monitoring:** Netflix uses synthetic control methods to detect metric changes when A/B tests are not possible (Brodersen et al., "CausalImpact" at Google, 2015).
- **Variance reduction via CUPED:** Controlled-experiment Using Pre-Experiment Data. Uses pre-treatment covariates to reduce variance in treatment effect estimates. Directly applicable when FunDB has pre-treatment metrics.
- **Always-valid confidence intervals:** Sequential testing methods that remain valid as more data arrives. Important for a database that continuously accumulates data.

### 6.4 Causal Inference in Machine Learning

Recent ML approaches to causal inference are relevant:

- **Causal forests (Athey & Imbens, 2016):** Estimate heterogeneous treatment effects using random forests. Can estimate how the effect varies across different types of units. Computationally expensive but could be offered as an advanced mode.
- **Double/debiased machine learning (Chernozhukov et al., 2018):** Uses ML for nuisance parameter estimation while maintaining valid inference for the causal parameter. Combines the flexibility of ML with the rigor of causal inference.
- **Representation learning for causal inference:** Learn representations that balance treated and control groups (Shalit et al., 2017, "CEVAE"). FunDB's embeddings are essentially learned representations -- can we check whether they are balanced?

---

## 7. Critical Insight for FunDB

The fundamental insight is that FunDB already possesses the three key ingredients that make automated quasi-experiments feasible:

1. **Bitemporal data** provides the exact before/after measurements needed for every method.
2. **Vector embeddings + HNSW** provide the similarity search needed to construct control groups automatically.
3. **Multi-tenant / multi-entity data** provides the natural variation needed -- some entities experienced the cause while others did not.

What is missing is the **orchestration layer**: the algorithm that takes a (cause, effect) pair, searches the database for quasi-experimental situations, applies the appropriate methods, validates assumptions, and returns a quantitative effect estimate with honest uncertainty bounds.

That is the subject of the PROPOSAL.

---

## 8. References

1. Angrist, J. D., & Pischke, J.-S. (2009). *Mostly Harmless Econometrics*. Princeton University Press.
2. Imbens, G. W., & Rubin, D. B. (2015). *Causal Inference for Statistics, Social, and Biomedical Sciences*. Cambridge University Press.
3. Abadie, A., Diamond, A., & Hainmueller, J. (2010). "Synthetic Control Methods for Comparative Case Studies." *JASA*, 105(490), 493-505.
4. Brodersen, K. H., et al. (2015). "Inferring Causal Impact Using Bayesian Structural Time-Series Models." *Annals of Applied Statistics*, 9(1), 247-274.
5. Callaway, B., & Sant'Anna, P. H. C. (2021). "Difference-in-Differences with Multiple Time Periods." *Journal of Econometrics*, 225(2), 200-230.
6. Rosenbaum, P. R., & Rubin, D. B. (1983). "The Central Role of the Propensity Score in Observational Studies." *Biometrika*, 70(1), 41-55.
7. Athey, S., & Imbens, G. W. (2016). "Recursive Partitioning for Heterogeneous Causal Effects." *PNAS*, 113(27), 7353-7360.
8. Hill, A. B. (1965). "The Environment and Disease: Association or Causation?" *Proceedings of the Royal Society of Medicine*, 58(5), 295-300.
9. Oster, E. (2019). "Unobservable Selection and Coefficient Stability: Theory and Evidence." *Journal of Business & Economic Statistics*, 37(2), 187-204.
10. Chernozhukov, V., et al. (2018). "Double/Debiased Machine Learning for Treatment and Structural Parameters." *Econometrics Journal*, 21(1), C1-C68.
