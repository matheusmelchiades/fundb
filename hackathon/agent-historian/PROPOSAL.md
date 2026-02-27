# SP3 Proposal: NaturalExperimentDetector

**Agent:** Historian
**Sub-Problem:** SP3 — Natural Experiment Detection
**Date:** 2026-02-28

---

## 1. Overview

This proposal defines the `NaturalExperimentDetector` algorithm: a cascade of quasi-experimental methods that, given a (cause_event_id, effect_event_id) pair, automatically searches FunDB's historical data for situations that approximate controlled experiments, applies the most credible method the data can support, validates that method's assumptions, and returns a quantitative causal effect estimate with explicit uncertainty bounds.

The output of this algorithm is **Signal S4** in the Statistician's signal fusion framework. When the detector finds a valid quasi-experiment, it provides the strongest possible observational evidence of causation — the closest available approximation to a randomized trial. When it finds nothing valid, it says so explicitly.

---

## 2. Algorithm Design: NaturalExperimentDetector

### 2.1 Input and Output Specification

**Input:**
```
NaturalExperimentDetector(
    cause_event_id:  UUID,        -- FunRecord of the suspected cause
    effect_event_id: UUID,        -- FunRecord of the suspected effect
    config:          DetectorConfig  -- optional tuning parameters
)
```

**DetectorConfig defaults:**
```
DetectorConfig {
    max_control_candidates:     100,     -- HNSW search limit for control group
    min_control_group_size:     10,      -- minimum observations per group
    pre_treatment_window:       30d,     -- look-back for baseline measurement
    post_treatment_window:      7d,      -- look-forward for effect measurement
    parallel_trends_min_periods: 5,      -- minimum periods for the parallel trends test
    rdd_bandwidth:              0.1,     -- fraction of running variable range for RDD
    scm_max_donors:             50,      -- maximum donor units for Synthetic Control
    event_study_min_instances:  5,       -- minimum repeat events for Event Study
    budget_ms:                  400,     -- time budget before cascade must terminate
    similarity_weights: {
        semantic:    0.5,
        structural:  0.3,
        temporal:    0.2
    }
}
```

**Output:**
```
NaturalExperimentResult {
    -- Primary output
    method_used:             MethodEnum | None
    effect_estimate:         float | None        -- point estimate of causal effect
    confidence_interval:     [float, float] | None  -- 95% CI, [lower, upper]
    quasi_experiment_score:  float               -- 0.0 = no valid QE, 1.0 = strong QE

    -- Signal S4 for Statistician fusion
    s4_signal:               float               -- in [0, 1], maps to Statistician's S4

    -- Diagnostics
    validity_diagnostics:    ValidityDiagnostics
    method_selection_log:    list[MethodAttempt]  -- which methods were tried and why skipped
    control_group:           list[UUID] | None    -- IDs of control units used

    -- Honesty metadata
    found_valid_experiment:  bool
    rejection_reason:        string | None        -- if no valid QE found
    warnings:                list[Warning]
}

ValidityDiagnostics {
    -- DiD / Event Study
    parallel_trends_p_value: float | None     -- p-value for pre-trends test (want > 0.1)
    parallel_trends_passed:  bool | None

    -- Balance diagnostics (matching, DiD)
    max_standardized_mean_diff: float | None  -- want < 0.25
    balance_passed:          bool | None
    covariate_balance_table: dict | None

    -- SCM
    pre_treatment_fit_rmse:  float | None     -- want < 10% of mean outcome
    scm_weights:             dict | None      -- donor unit weights

    -- RDD
    mccrary_p_value:         float | None     -- density continuity test, want > 0.05
    f_statistic_first_stage: float | None     -- instrument strength, want > 10

    -- Universal
    sample_sizes:            dict             -- n_treatment, n_control
    effect_size_cohens_d:    float | None
    p_value:                 float | None
    sutva_passed:            bool             -- no spillover to control group
}

MethodEnum: ITS | EVENT_STUDY | DiD | SYNTHETIC_CONTROL | PROPENSITY_MATCHING | RDD
```

### 2.2 The Cascade Algorithm

The cascade is ordered by the **credibility hierarchy** — methods requiring stronger data assumptions are tried later. Critically, each method performs its own assumption validation before reporting a result. If a method's assumptions fail, the cascade moves to the next candidate.

```
NaturalExperimentDetector(cause_event_id, effect_event_id, config):

    cause  = FunDB.load(cause_event_id)
    effect = FunDB.load(effect_event_id)
    timer  = Timer(budget_ms = config.budget_ms)
    log    = []

    // ============================================================
    // STAGE 0: Pre-flight checks (< 5ms)
    // ============================================================

    // Temporal ordering check — if effect precedes cause, abort
    IF effect._valid_from <= cause._valid_from:
        RETURN NaturalExperimentResult {
            found_valid_experiment: false,
            rejection_reason: "TEMPORAL_ORDERING_VIOLATED: effect precedes cause"
        }

    treatment_time = cause._valid_from
    outcome_series = extract_time_series(effect, window_before=config.pre_treatment_window,
                                                  window_after=config.post_treatment_window)

    IF outcome_series.length < 5:
        RETURN NaturalExperimentResult {
            found_valid_experiment: false,
            rejection_reason: "INSUFFICIENT_TIME_SERIES: fewer than 5 observations"
        }

    // ============================================================
    // STAGE 1: Check for RDD opportunity (< 10ms)
    // Try this FIRST because when it applies, it is the most
    // credible identification strategy.
    // ============================================================

    rdd_opportunity = detect_threshold_assignment(cause, config)

    IF rdd_opportunity.found AND timer.remaining() > 50ms:
        result = attempt_rdd(cause, effect, rdd_opportunity, config)
        log.append({ method: RDD, attempted: true, outcome: result.status })

        IF result.valid:
            RETURN result.to_output(s4_signal = score_to_s4(result))

    // ============================================================
    // STAGE 2: Search for control group via HNSW (< 30ms)
    // A control group enables DiD, SCM, and Propensity Matching.
    // The search happens once and results are reused across methods.
    // ============================================================

    IF timer.remaining() > 100ms:
        control_candidates = find_control_candidates(cause, effect, config)
    ELSE:
        control_candidates = ControlCandidates.empty()

    // ============================================================
    // STAGE 3: Event Study (< 80ms)
    // Requires: multiple instances of the same type of cause event.
    // Provides built-in pre-treatment placebo test.
    // ============================================================

    similar_causes = find_similar_causes(cause, config)  // uses HNSW on cause embedding

    IF similar_causes.count >= config.event_study_min_instances AND timer.remaining() > 80ms:
        result = attempt_event_study(similar_causes, effect, config)
        log.append({ method: EVENT_STUDY, attempted: true, outcome: result.status })

        IF result.valid:
            RETURN result.to_output(s4_signal = score_to_s4(result))

    // ============================================================
    // STAGE 4: DiD (< 80ms)
    // Requires: control group + pre-treatment period.
    // ============================================================

    IF control_candidates.size >= config.min_control_group_size AND timer.remaining() > 80ms:
        result = attempt_did(cause, effect, control_candidates, config)
        log.append({ method: DiD, attempted: true, outcome: result.status })

        IF result.valid:
            RETURN result.to_output(s4_signal = score_to_s4(result))

    // ============================================================
    // STAGE 5: Synthetic Control (< 150ms)
    // Requires: single treated unit with good pre-treatment data.
    // Computationally heavier (constrained optimization).
    // ============================================================

    IF control_candidates.size >= 3 AND timer.remaining() > 150ms:
        result = attempt_scm(cause, effect, control_candidates, config)
        log.append({ method: SYNTHETIC_CONTROL, attempted: true, outcome: result.status })

        IF result.valid:
            RETURN result.to_output(s4_signal = score_to_s4(result))

    // ============================================================
    // STAGE 6: ITS — always available, last resort (< 20ms)
    // Requires only the treated unit's own time-series.
    // Least credible (no control group) but always computable.
    // ============================================================

    result = attempt_its(cause, effect, outcome_series, config)
    log.append({ method: ITS, attempted: true, outcome: result.status })

    IF result.valid:
        RETURN result.to_output(s4_signal = score_to_s4(result))

    // ============================================================
    // STAGE 7: No valid quasi-experiment found
    // ============================================================

    RETURN NaturalExperimentResult {
        found_valid_experiment:  false,
        quasi_experiment_score:  0.0,
        s4_signal:               0.5,  // total ignorance — not evidence against
        method_selection_log:    log,
        rejection_reason:        "NO_VALID_METHOD: all methods failed assumption checks",
        warnings:                [Warning("Observational association exists but no valid "
                                         "quasi-experiment could be constructed. S4 is "
                                         "set to 0.5 (ignorance), not 0.0 (against causation).")]
    }
```

**Why RDD is checked first:** When treatment assignment is threshold-based (e.g., autoscaling triggers, feature flag rollout percentages, rate limits), units just above and below the threshold are nearly identical. This gives the most credible local causal effect — as close to random assignment as observational data can produce. The cost of checking is low; the payoff when found is high.

**Why ITS is last:** ITS requires no control group and always produces a number. But it cannot distinguish the treatment effect from concurrent events. It is the fallback when nothing better is available, and it receives a lower `quasi_experiment_score` weight.

---

### 2.3 Control Group Construction via HNSW Similarity Search

```
find_control_candidates(cause, effect, config):

    // Step 1: Identify the "treated entity"
    // The entity is the service/tenant/system that experienced the cause.
    treated_entity_id = cause._entity_id   // from FunRecord metadata

    // Step 2: Find the treatment time window
    treatment_time = cause._valid_from
    comparison_start = treatment_time - config.pre_treatment_window
    comparison_end   = treatment_time + config.post_treatment_window

    // Step 3: HNSW search for semantically similar events that DID NOT receive treatment
    // We look for events of the same type as the cause, occurring in the same period,
    // but belonging to DIFFERENT entities.
    similar_causes_at_different_entities = HNSW.search(
        vector  = cause._embedding,
        k       = config.max_control_candidates,
        filter  = {
            _entity_id != treated_entity_id,       // different entity (not treated)
            _valid_from BETWEEN comparison_start AND comparison_end,
            _collection = cause._collection        // same event type
        }
    )

    // If not enough results from the same event type, expand search
    IF similar_causes_at_different_entities.count < config.min_control_group_size:
        untreated_entities = HNSW.search(
            vector  = cause._entity_embedding,    // use entity-level embedding
            k       = config.max_control_candidates * 2,
            filter  = {
                _entity_id != treated_entity_id,
                _valid_from BETWEEN comparison_start AND comparison_end
            }
        )
        // Use entities that have the same outcome metric but did NOT experience the cause
        similar_causes_at_different_entities = filter_entities_without_cause(
            untreated_entities, cause, config
        )

    // Step 4: Compute composite similarity score for each candidate
    scored_candidates = []
    FOR candidate IN similar_causes_at_different_entities:
        semantic_sim   = cosine_similarity(candidate._embedding, cause._embedding)
        structural_sim = jaccard_similarity(
            graph_neighborhood(candidate._entity_id, depth=2),
            graph_neighborhood(treated_entity_id,    depth=2)
        )
        temporal_sim   = temporal_context_similarity(candidate._valid_from, cause._valid_from)

        composite_sim = (
            config.similarity_weights.semantic    * semantic_sim  +
            config.similarity_weights.structural  * structural_sim +
            config.similarity_weights.temporal    * temporal_sim
        )

        scored_candidates.append({
            entity_id:   candidate._entity_id,
            similarity:  composite_sim,
            cause_event: candidate
        })

    // Step 5: Sort by similarity and return top candidates
    scored_candidates.sort_by(similarity, DESC)
    RETURN scored_candidates[:config.max_control_candidates]
```

**Computational cost:** One HNSW search is O(log n) with n = total indexed events. Composite scoring is O(k) with k = candidates returned. Total: ~10-20ms for a 10M-event database.

---

### 2.4 Method Implementations

#### 2.4.1 Interrupted Time Series (ITS)

```
attempt_its(cause, effect, outcome_series, config):

    T0 = index of treatment_time in outcome_series
    n_pre  = T0
    n_post = len(outcome_series) - T0

    IF n_pre < 3 OR n_post < 2:
        RETURN { valid: false, reason: "ITS: insufficient pre/post observations" }

    // Segmented regression:
    // Y(t) = b0 + b1*t + b2*D(t) + b3*(t - T0)*D(t) + error
    // D(t) = 1 if t >= T0

    t  = [0, 1, ..., len(outcome_series)-1]
    D  = [0 if t < T0 else 1 for t in t]
    Dt = [(ti - T0) * D[i] for i, ti in enumerate(t)]

    X = column_stack([ones, t, D, Dt])
    Y = outcome_series.values

    beta, residuals, rank, sv = OLS(X, Y)
    // beta[2] = immediate level change (b2)
    // beta[3] = change in slope (b3)

    // Variance estimation with Newey-West HAC for autocorrelation
    se = newey_west_se(X, residuals, lags=min(4, n_pre - 1))

    effect_estimate = beta[2]  // immediate level change
    ci_95 = [effect_estimate - 1.96*se[2], effect_estimate + 1.96*se[2]]
    p_value = two_tailed_t_test(beta[2], se[2], df = len(t) - 4)

    // Check: does CI exclude zero?
    ci_excludes_zero = (ci_95[0] > 0 AND ci_95[1] > 0) OR (ci_95[0] < 0 AND ci_95[1] < 0)

    // ITS-specific validity: the pre-trend must be stable
    pre_trend_se = se[1]  // SE of the pre-intervention slope (b1)
    pre_trend_significant = abs(beta[1]) > 2 * pre_trend_se
    // A highly significant pre-trend makes the post-change harder to interpret.
    // We do not reject on this — we warn.

    its_score = compute_its_score(p_value, ci_excludes_zero, n_pre, n_post)

    RETURN MethodResult {
        valid:          true,  // ITS always produces a result; score reflects quality
        method:         ITS,
        effect_estimate: effect_estimate,
        confidence_interval: ci_95,
        p_value:        p_value,
        quasi_experiment_score: its_score,
        diagnostics: {
            level_change:    beta[2],
            slope_change:    beta[3],
            pre_slope:       beta[1],
            pre_trend_warning: pre_trend_significant
        },
        warnings: ([Warning("Pre-intervention trend is non-flat; effect estimate "
                            "may conflate trend continuation with treatment effect")]
                    IF pre_trend_significant ELSE [])
    }
```

**ITS quasi_experiment_score formula:**
```
its_score = base_score * power_factor * no_control_penalty

base_score     = 1 - p_value                          // higher is better
power_factor   = min(1.0, sqrt(n_pre * n_post) / 20)  // penalize small samples
no_control_penalty = 0.6                               // ITS has no control group

its_score = min(0.60, its_score)  // cap at 0.60 because ITS cannot rule out co-occurring events
```

#### 2.4.2 Difference-in-Differences (DiD)

```
attempt_did(cause, effect, control_candidates, config):

    treated_series = extract_time_series(effect, pre=config.pre_treatment_window,
                                                  post=config.post_treatment_window)

    control_series_list = []
    FOR candidate IN control_candidates[:30]:  // limit to 30 for speed
        series = extract_outcome_series_for_entity(
            candidate.entity_id,
            outcome_metric = effect._metric_name,
            pre  = config.pre_treatment_window,
            post = config.post_treatment_window,
            treatment_time = cause._valid_from
        )
        IF series is not None:
            control_series_list.append(series)

    IF len(control_series_list) < config.min_control_group_size:
        RETURN { valid: false, reason: "DiD: insufficient control units with outcome data" }

    control_series = average_series(control_series_list)  // average across control units

    // --- Parallel trends test ---
    // Pre-treatment period only: test if treated - control difference is flat
    pre_treated = treated_series.pre_period
    pre_control = control_series.pre_period

    pre_difference = pre_treated - pre_control  // element-wise
    // Fit linear trend to pre-period difference
    slope, intercept, slope_se = OLS_simple(time_index_pre, pre_difference)
    parallel_trends_p = two_tailed_t_test(slope, slope_se, df = len(pre_difference) - 2)
    parallel_trends_passed = parallel_trends_p > 0.10  // fail if trend is significant at 10%

    IF NOT parallel_trends_passed AND config.parallel_trends_min_periods > 3:
        RETURN {
            valid: false,
            reason: "DiD: parallel trends assumption VIOLATED (p={parallel_trends_p:.3f}). "
                    "Pre-treatment trends diverge significantly."
        }

    // --- Balance check on covariates ---
    balance_table = {}
    max_smd = 0.0
    FOR covariate IN ["mean_load", "event_frequency", "entity_age"]:
        treated_mean = mean_covariate(treated_entity, covariate, pre_window)
        control_mean = mean_covariate(control_group, covariate, pre_window)
        pooled_sd    = pooled_std_dev(treated_entity, control_group, covariate, pre_window)
        smd = abs(treated_mean - control_mean) / pooled_sd
        balance_table[covariate] = smd
        max_smd = max(max_smd, smd)

    balance_passed = max_smd < 0.25

    // --- SUTVA check: did control units' outcomes change at treatment time? ---
    sutva_passed = check_sutva(control_series_list, cause._valid_from)

    // --- DiD estimator ---
    Y_t_after  = mean(treated_series.post_period)
    Y_t_before = mean(treated_series.pre_period)
    Y_c_after  = mean(control_series.post_period)
    Y_c_before = mean(control_series.pre_period)

    effect_estimate = (Y_t_after - Y_t_before) - (Y_c_after - Y_c_before)

    // Variance via clustered bootstrap (entity-level clustering, 500 resamples)
    ci_95 = clustered_bootstrap_ci(treated_series, control_series_list,
                                    estimator=did_estimator, B=500, alpha=0.05)

    p_value = ci_to_p_value(effect_estimate, ci_95)

    did_score = compute_did_score(
        parallel_trends_p, balance_passed, sutva_passed,
        n_treated=len(treated_series), n_control=len(control_series_list), p_value
    )

    RETURN MethodResult {
        valid:               parallel_trends_passed,
        method:              DiD,
        effect_estimate:     effect_estimate,
        confidence_interval: ci_95,
        p_value:             p_value,
        quasi_experiment_score: did_score,
        diagnostics: {
            parallel_trends_p_value:     parallel_trends_p,
            parallel_trends_passed:      parallel_trends_passed,
            max_standardized_mean_diff:  max_smd,
            balance_passed:              balance_passed,
            covariate_balance_table:     balance_table,
            sutva_passed:                sutva_passed,
            sample_sizes:                {n_treatment: 1, n_control: len(control_series_list)}
        }
    }
```

**DiD quasi_experiment_score formula:**
```
did_score = base_credibility * trend_penalty * balance_penalty * sutva_penalty * power_factor

base_credibility = 0.85   // DiD with a good control group is strong evidence
trend_penalty   = 1.0 if parallel_trends_p > 0.20 else
                  lerp(0.5, 1.0, (parallel_trends_p - 0.10) / 0.10)  // degrades near threshold
balance_penalty = 1.0 if max_smd < 0.10 else
                  lerp(0.7, 1.0, (0.25 - max_smd) / 0.15)
sutva_penalty   = 1.0 if sutva_passed else 0.7
power_factor    = min(1.0, sqrt(n_control) / 10)  // more control units = better
```

#### 2.4.3 Synthetic Control Method (SCM)

```
attempt_scm(cause, effect, control_candidates, config):

    treated_series = extract_time_series(effect, pre=config.pre_treatment_window,
                                                  post=config.post_treatment_window)

    IF len(treated_series.pre_period) < 10:
        RETURN { valid: false, reason: "SCM: insufficient pre-treatment periods (need >= 10)" }

    // Build donor pool
    donor_series = []
    FOR candidate IN control_candidates[:config.scm_max_donors]:
        series = extract_outcome_series_for_entity(candidate.entity_id, ...)
        IF series is not None AND not_affected_by_treatment(series, cause):
            donor_series.append(series)

    IF len(donor_series) < 3:
        RETURN { valid: false, reason: "SCM: insufficient unaffected donor units" }

    // Constrained optimization: find weights w such that
    //   Sigma w_j * Y_j(pre) ~= Y_treated(pre)
    //   Subject to: w_j >= 0, Sigma w_j = 1
    Y_pre_treated = treated_series.pre_period
    Y_pre_donors  = matrix of donor pre-periods

    // Scipy-style constrained minimization (Frank-Wolfe or SLSQP)
    // Cost: O(d * T_pre) where d = donors, T_pre = pre-treatment periods
    // For d=50, T_pre=30: ~1500 operations — trivial
    weights = optimize_scm_weights(Y_pre_treated, Y_pre_donors)

    // Compute synthetic control outcome
    Y_synthetic_pre  = Y_pre_donors.T @ weights
    Y_synthetic_post = [sum(w * donor[t] for w, donor in zip(weights, donor_series))
                        for t in post_period_indices]

    // Pre-treatment fit quality
    pre_fit_rmse = sqrt(mean((Y_pre_treated - Y_synthetic_pre) ** 2))
    pre_fit_pct  = pre_fit_rmse / mean(Y_pre_treated)

    IF pre_fit_pct > 0.20:  // poor pre-fit: synthetic control is not comparable
        RETURN {
            valid: false,
            reason: f"SCM: poor pre-treatment fit (RMSE = {pre_fit_pct*100:.1f}% of mean). "
                    "Synthetic control cannot reproduce treated unit's pre-treatment trajectory."
        }

    // Effect estimate: average post-treatment gap
    effect_estimate = mean(
        treated_series.post_period[t] - Y_synthetic_post[t]
        for t in post_period_indices
    )

    // Inference via placebo tests (Abadie et al. permutation approach)
    placebo_effects = []
    FOR donor IN donor_series:
        placebo_weights = optimize_scm_weights(donor.pre_period, exclude_donor(Y_pre_donors, donor))
        placebo_synthetic = compute_synthetic(placebo_weights, exclude_donor_series)
        placebo_effect = mean(donor.post_period - placebo_synthetic)
        placebo_effects.append(placebo_effect)

    // Approximate 95% CI from placebo distribution
    placebo_sd = std(placebo_effects)
    ci_95 = [effect_estimate - 1.96 * placebo_sd, effect_estimate + 1.96 * placebo_sd]

    // p-value: fraction of placebos with larger absolute effect
    p_value = sum(1 for pe in placebo_effects if abs(pe) >= abs(effect_estimate)) / len(placebo_effects)

    scm_score = compute_scm_score(pre_fit_pct, weights, p_value, len(donor_series))

    RETURN MethodResult {
        valid:               true,
        method:              SYNTHETIC_CONTROL,
        effect_estimate:     effect_estimate,
        confidence_interval: ci_95,
        p_value:             p_value,
        quasi_experiment_score: scm_score,
        diagnostics: {
            pre_treatment_fit_rmse: pre_fit_rmse,
            scm_weights:            dict(zip(donor_ids, weights)),
            n_donors_used:          len(donor_series),
            n_placebo_tests:        len(placebo_effects)
        }
    }
```

#### 2.4.4 Event Study Design

```
attempt_event_study(similar_causes, effect, config):

    // Collect all instances of the same type of cause event
    // Each instance is a (cause_event, outcome_series) pair
    instances = []
    FOR cause_instance IN similar_causes:
        series = extract_normalized_series(
            entity    = cause_instance._entity_id,
            metric    = effect._metric_name,
            center    = cause_instance._valid_from,
            window    = config.pre_treatment_window + config.post_treatment_window
        )
        IF series is not None:
            instances.append((cause_instance, series))

    IF len(instances) < config.event_study_min_instances:
        RETURN { valid: false, reason: "EVENT_STUDY: too few instances of cause type" }

    // Center time at treatment (t=0), normalize to relative periods
    // Estimate: Y(i,t) = alpha_i + gamma_t + Sigma_k beta_k * D(i,t-k) + error
    // beta_k = effect at k periods relative to treatment

    k_range = range(-min_periods_pre, max_periods_post + 1)
    beta_k, se_k = event_study_regression(instances, k_range)

    // --- Pre-treatment placebo test ---
    // Pre-treatment beta_k should be near zero if parallel trends holds
    pre_k_values  = [beta_k[k] for k in k_range if k < 0]
    pre_k_se      = [se_k[k] for k in k_range if k < 0]
    pre_test_stat = sum(abs(b) / se for b, se in zip(pre_k_values, pre_k_se)) / len(pre_k_values)
    pre_test_p    = chi2_p_value(pre_test_stat, df = len(pre_k_values))
    // Large test stat = pre-treatment trends differ = assumption violated

    parallel_trends_passed = pre_test_p > 0.10

    // Effect estimate: average of post-treatment beta_k
    post_k_values = [beta_k[k] for k in k_range if k > 0]
    effect_estimate = mean(post_k_values)

    // CI from event study SE
    post_k_se = [se_k[k] for k in k_range if k > 0]
    pooled_se = sqrt(mean([s**2 for s in post_k_se]))
    ci_95 = [effect_estimate - 1.96 * pooled_se, effect_estimate + 1.96 * pooled_se]

    RETURN MethodResult {
        valid:               parallel_trends_passed,
        method:              EVENT_STUDY,
        effect_estimate:     effect_estimate,
        confidence_interval: ci_95,
        quasi_experiment_score: compute_event_study_score(pre_test_p, len(instances), pooled_se),
        diagnostics: {
            parallel_trends_p_value: pre_test_p,
            parallel_trends_passed:  parallel_trends_passed,
            n_instances:             len(instances),
            beta_k_by_period:        dict(zip(k_range, beta_k)),
            se_k_by_period:          dict(zip(k_range, se_k))
        }
    }
```

#### 2.4.5 Regression Discontinuity Design (RDD)

```
detect_threshold_assignment(cause, config):

    // Look for threshold-based assignment rules that determined the cause
    // Sources to check:
    //   1. Metadata on the cause event (_trigger_rule, _threshold, _running_variable)
    //   2. Related events in the causal graph that describe triggering conditions
    //   3. Pattern detection: are there many similar events at a specific threshold?

    // Check explicit threshold metadata
    IF cause._metadata.contains("threshold") AND cause._metadata.contains("running_variable"):
        RETURN RDDOpportunity {
            found:            true,
            running_variable: cause._metadata.running_variable,
            threshold:        cause._metadata.threshold,
            source:           "EXPLICIT_METADATA"
        }

    // Check graph for triggering rules
    trigger_events = FunGraph.reverse_traverse(cause._id, edge_type="triggered_by", depth=1)
    FOR te IN trigger_events:
        IF te._type == "threshold_rule":
            RETURN RDDOpportunity {
                found:            true,
                running_variable: te._running_variable,
                threshold:        te._threshold,
                source:           "CAUSAL_GRAPH"
            }

    // Pattern detection: look for clustering of cause events at a specific value
    // (e.g., all similar events are triggered when metric X crosses value V)
    similar_events = HNSW.search(cause._embedding, k=200)
    running_var_values = [e._metadata.get("trigger_value") for e in similar_events
                          if e._metadata.get("trigger_value") is not None]

    IF len(running_var_values) > 20:
        threshold_candidate = detect_clustering_threshold(running_var_values)
        IF threshold_candidate is not None:
            RETURN RDDOpportunity {
                found:     true,
                running_variable: "detected",
                threshold: threshold_candidate,
                source:    "PATTERN_DETECTED"
            }

    RETURN RDDOpportunity { found: false }


attempt_rdd(cause, effect, rdd_opportunity, config):

    X = running_var_values  // the continuous assignment variable
    Y = effect_values_at_those_X
    c = rdd_opportunity.threshold

    // Bandwidth selection using the Imbens-Kalyanaraman optimal bandwidth formula
    h = ik_bandwidth(X, Y, c)
    h = max(h, config.rdd_bandwidth * (max(X) - min(X)))  // enforce minimum bandwidth

    // Filter to observations within bandwidth
    X_bw = X where |X - c| <= h
    Y_bw = Y where |X - c| <= h

    IF len(X_bw) < 10:
        RETURN { valid: false, reason: "RDD: insufficient observations within bandwidth" }

    // McCrary density test: check for bunching at threshold
    mccrary_p = mccrary_test(X, c)
    IF mccrary_p < 0.05:
        RETURN {
            valid: false,
            reason: f"RDD: McCrary density test FAILED (p={mccrary_p:.3f}). "
                    "Density discontinuity at threshold suggests manipulation."
        }

    // Local linear regression on each side of the threshold
    left_X  = X_bw where X_bw < c;  left_Y  = Y_bw[left_X]
    right_X = X_bw where X_bw >= c; right_Y = Y_bw[right_X]

    mu_plus  = local_linear_regression(right_X, right_Y, at_x=c)
    mu_minus = local_linear_regression(left_X,  left_Y,  at_x=c)

    effect_estimate = mu_plus - mu_minus
    se = rdd_asymptotic_se(left_X, left_Y, right_X, right_Y, h)
    ci_95 = [effect_estimate - 1.96*se, effect_estimate + 1.96*se]
    p_value = two_tailed_t_test(effect_estimate, se, df = len(X_bw) - 4)

    RETURN MethodResult {
        valid:               mccrary_p >= 0.05,
        method:              RDD,
        effect_estimate:     effect_estimate,
        confidence_interval: ci_95,
        p_value:             p_value,
        quasi_experiment_score: compute_rdd_score(mccrary_p, len(X_bw), p_value),
        diagnostics: {
            mccrary_p_value:     mccrary_p,
            bandwidth_used:      h,
            n_in_bandwidth:      len(X_bw),
            running_variable:    rdd_opportunity.running_variable,
            threshold:           rdd_opportunity.threshold
        }
    }
```

---

### 2.5 Signal S4 Conversion

The `quasi_experiment_score` (0-1) maps to the Statistician's Signal S4:

```
score_to_s4(method_result):

    // S4 is in [0, 1] where:
    //   0.5 = no quasi-experiment found (ignorance, not evidence against)
    //   > 0.5 = quasi-experiment supports causation
    //   < 0.5 = quasi-experiment contradicts causation

    qe_score = method_result.quasi_experiment_score  // quality of the QE design
    effect   = method_result.effect_estimate
    ci_lower = method_result.confidence_interval[0]
    ci_upper = method_result.confidence_interval[1]

    // Determine direction of effect relative to prior
    // If effect is in the expected direction (positive), treat as supporting
    // If the CI includes zero, reduce S4 toward 0.5
    IF ci_upper <= 0 AND ci_lower < 0:
        direction_factor = -1  // effect is significantly negative (contradicts causation)
    ELIF ci_lower >= 0 AND ci_upper > 0:
        direction_factor = +1  // effect is significantly positive (supports causation)
    ELSE:
        direction_factor = 0   // CI includes zero, ambiguous

    // Map to S4
    IF direction_factor == 0:
        s4 = 0.5  // ambiguous: CI includes zero
    ELIF direction_factor > 0:
        s4 = 0.5 + 0.5 * qe_score  // supporting: ranges from 0.5 to 1.0
    ELSE:
        s4 = 0.5 - 0.5 * qe_score  // contradicting: ranges from 0.0 to 0.5

    RETURN s4
```

**When no valid QE is found:** S4 = 0.5 (Dempster-Shafer ignorance mass). This is honest: absence of a natural experiment is not evidence against causation — it is simply no evidence.

---

## 3. FunQL Syntax: EVALUATE QUASI_EXPERIMENT

The `NaturalExperimentDetector` is exposed through a new FunQL operator. The design follows the ergonomics principle: a developer should be able to invoke it without knowing which method will be selected.

### 3.1 Basic Syntax

```sql
-- Minimum invocation: let the system choose everything
EVALUATE QUASI_EXPERIMENT
    CAUSE  :deploy_event_id
    EFFECT :error_spike_id;

-- Returns:
-- {
--   method_used:            "DiD",
--   effect_estimate:        0.034,
--   confidence_interval:    [0.018, 0.050],
--   quasi_experiment_score: 0.74,
--   s4_signal:              0.87,
--   found_valid_experiment: true,
--   validity_diagnostics: {
--     parallel_trends_p_value: 0.43,
--     parallel_trends_passed:  true,
--     max_standardized_mean_diff: 0.12,
--     balance_passed: true,
--     ...
--   }
-- }
```

### 3.2 Extended Syntax with Options

```sql
-- With explicit configuration
EVALUATE QUASI_EXPERIMENT
    CAUSE  :deploy_event_id
    EFFECT :error_spike_id
    OPTIONS (
        pre_treatment_window     = '14 days',
        post_treatment_window    = '3 days',
        min_control_group_size   = 5,
        prefer_method            = 'DiD',   -- hint, not a mandate; cascade still validates
        budget_ms                = 500,
        include_method_log       = true
    );
```

### 3.3 Bulk Evaluation

```sql
-- Evaluate quasi-experiments for all candidate causes of an effect
SELECT
    c.event_id,
    c.description,
    qe.method_used,
    qe.effect_estimate,
    qe.quasi_experiment_score,
    qe.s4_signal
FROM events c
CROSS APPLY (
    EVALUATE QUASI_EXPERIMENT
        CAUSE  c.event_id
        EFFECT :target_effect_id
        OPTIONS (budget_ms = 200)
) qe
WHERE c._valid_from < :target_effect_time
  AND qe.found_valid_experiment = true
ORDER BY qe.quasi_experiment_score DESC
LIMIT 10;
```

### 3.4 Integration with ASSESS CAUSALITY

```sql
-- Use EVALUATE QUASI_EXPERIMENT as one input to full causal assessment
ASSESS CAUSALITY
    FROM :cause_event_id
    TO   :effect_event_id
    WITH SIGNALS (
        temporal    = ON,
        mechanism   = ON (max_tiers: 3),
        confounders = ON (lookback: '90 days'),
        experiment  = ON (budget_ms: 400),   -- invokes NaturalExperimentDetector
        consensus   = ON
    )
    RETURN full_assessment;
-- The experiment signal's output is automatically used as S4 in Dempster-Shafer fusion.
```

---

## 4. Computational Complexity Analysis

The 500ms cold-path budget is allocated across the cascade stages:

| Stage | Operations | Expected Latency | Budget Allocation |
|-------|-----------|-----------------|-------------------|
| Pre-flight checks | Load 2 records, check timestamps | ~2ms | 5ms |
| RDD detection | HNSW search (k=200) + pattern analysis | ~15ms | 25ms |
| Control group search | 1-2 HNSW searches (k=100), composite scoring | ~25ms | 50ms |
| Event Study | HNSW search (k=100), panel regression on ~500 obs | ~60ms | 100ms |
| DiD | Bootstrapped estimator (B=500), parallel trends test | ~80ms | 120ms |
| SCM | Constrained optimization (d=50, T=30) + placebo tests | ~120ms | 180ms |
| ITS | Segmented regression (n~100), Newey-West SE | ~10ms | 20ms |
| **Total worst case** | All stages attempted, no early exit | **~312ms** | **500ms** |

**Key properties:**
- The cascade exits early on the first valid method. Typical latency is 30-80ms (control group search + one method attempt).
- SCM is the only stage with non-trivial optimization. With d=50 donors and T=30 pre-treatment periods, the SLSQP solver performs ~5000 scalar operations — trivial for modern hardware.
- Placebo tests for SCM (inference by permutation) are parallelizable across FunDB's worker pool. 50 placebos at 2ms each = 100ms sequential, or ~15ms parallel on 8 threads.
- If the full 500ms budget is exhausted before the cascade completes, the current best partial result is returned with `is_partial: true` and a wider confidence interval reflecting the incomplete analysis.

**Big-O summary:**
```
Control group search:      O(log n + k * d)     -- n events, k candidates, d dimensions
DiD:                       O(k * T * B)         -- k controls, T time points, B bootstrap samples
SCM:                       O(d * T^2)           -- d donors, T pre-treatment periods
Event Study:               O(m * T * K)         -- m instances, T periods, K time bins
RDD:                       O(k_bw * log k_bw)   -- k_bw obs in bandwidth
ITS:                       O(T)                 -- T time points, trivial
```

For the largest realistic FunDB workload (n=10M events, k=100 candidates, d=768-dim embeddings, T=30 periods, B=500 bootstrap, d_scm=50 donors): all stages fit within 500ms on standard hardware.

---

## 5. Assumption Validation Summary

Every method reports its assumption validation status. The output is consumed by the Skeptic's Red Team checklist.

| Method | Assumption | Test | Threshold to Fail |
|--------|-----------|------|-------------------|
| **ITS** | Stable pre-trend | OLS slope significance | p < 0.05 on pre-trend slope |
| **ITS** | No co-occurring events | Checked via confounder scan output | External |
| **DiD** | Parallel trends | Pre-period difference trend | p < 0.10 on slope |
| **DiD** | Balance | Standardized mean difference | Max SMD > 0.25 |
| **DiD** | SUTVA | Control outcome shift at treatment time | > 1 SD change |
| **SCM** | Good pre-fit | RMSE as % of mean | > 20% |
| **SCM** | No spillover | Donor outcome check | > 1 SD change |
| **Event Study** | Parallel trends | Joint test of pre-period coefficients | Chi-sq p < 0.10 |
| **Event Study** | Sufficient instances | Count of similar events | < 5 instances |
| **RDD** | No manipulation | McCrary density test | p < 0.05 |
| **RDD** | Sufficient bandwidth obs | Count in bandwidth | < 10 |

---

## 6. Integration with Agent Statistician (Signal S4)

The connection to the Statistician's Dempster-Shafer fusion is direct:

```
Statistician receives from Historian:

s4_signal: float in [0, 1]       -- maps directly to si in fuse_signals()
s4_reliability: float in (0, 1]  -- method-specific reliability

    Method reliabilities (r4 values for Statistician's framework):
    RDD:               r4 = 0.90  // near-random assignment, very credible
    Event Study:       r4 = 0.80  // multiple instances, built-in placebo test
    DiD:               r4 = 0.75  // control group, assumption-tested
    SCM:               r4 = 0.75  // good pre-fit, transparent weights
    ITS:               r4 = 0.55  // no control group, weakest of the five
    No valid QE found: r4 = 0.0   // s4 = 0.5 and r4 = 0.0 => pure ignorance mass

The Statistician uses these as: m_i({C}) = r4 * s4, m_i({~C}) = r4 * (1 - s4)
```

The Historian provides the `(s4_signal, s4_reliability)` tuple. The Statistician handles the fusion. Neither agent needs to know the other's internal logic.

---

## 7. Honesty Commitments

The `NaturalExperimentDetector` makes the following honesty commitments that cannot be overridden by configuration:

1. **Assumption reporting is mandatory.** Every result includes the full `validity_diagnostics` object. Callers cannot suppress this.

2. **"No valid experiment" is a first-class result.** It is not an error or exception — it is a legitimate answer with `found_valid_experiment: false` and a clear `rejection_reason`.

3. **Method score caps are enforced.** ITS cannot exceed a `quasi_experiment_score` of 0.60. This reflects its inherent limitation (no control group), regardless of statistical significance.

4. **S4 = 0.5 when no QE found.** Never 0.0. Absence of a natural experiment is not evidence against causation; it is ignorance. The Dempster-Shafer framework handles this correctly.

5. **Sample size is always reported.** A quasi-experiment with n_treatment = 3 is reported, not hidden — but the wide confidence intervals and low power automatically depress the `quasi_experiment_score`.

6. **The method selection log is always available.** Every method attempted and the reason for rejection or acceptance is logged. Callers can inspect why DiD failed and ITS was used instead.
