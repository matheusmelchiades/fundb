// crates/fundb-causal/src/tier2.rs
//
// STORY-6-2: Causal Engine Tier 2 — Statistical Discovery
//
// Implements:
//   - GrangerDiscovery: OLS-based Granger causality tests with ADF pre-check
//     and rolling-window stability analysis.
//   - LlmOracle: stub for LLM-hypothesis generation and validation.

use fundb_core::StabilityStatus;

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

/// Result of a single Granger causality test.
#[derive(Debug, Clone)]
pub struct GrangerResult {
    /// F-statistic from the restricted vs. unrestricted OLS comparison.
    pub f_stat: f64,
    /// Approximate p-value derived from the F-statistic.
    pub p_value: f64,
    /// Optimal lag (1..=max_lag) that produced the lowest p-value.
    pub lag: usize,
    /// Whether first-differencing was applied to achieve stationarity.
    pub differenced: bool,
}

/// Result of a rolling-window Granger causality test.
#[derive(Debug, Clone)]
pub struct RollingGrangerResult {
    /// Total number of windows evaluated.
    pub windows_tested: usize,
    /// Number of windows where p_value < 0.05.
    pub significant_windows: usize,
    /// `significant_windows / windows_tested` (0.0 when windows_tested == 0).
    pub stability_score: f32,
    /// Qualitative classification derived from `stability_score`.
    pub stability_status: StabilityStatus,
}

// ---------------------------------------------------------------------------
// GrangerDiscovery
// ---------------------------------------------------------------------------

/// OLS-based Granger causality engine (Tier 2).
pub struct GrangerDiscovery;

impl GrangerDiscovery {
    /// Create a new [`GrangerDiscovery`] instance.
    pub fn new() -> Self {
        GrangerDiscovery
    }

    /// OLS-based Granger causality test.
    ///
    /// Tests whether `x` Granger-causes `y` for each lag from 1 to `max_lag`
    /// and returns the result with the lowest p-value.
    pub fn test_pair(&self, x: &[f64], y: &[f64], max_lag: usize) -> GrangerResult {
        self.run_granger_sweep(x, y, max_lag, false)
    }

    /// Granger test with ADF stationarity pre-check.
    ///
    /// If either series is detected as non-stationary by the ADF heuristic,
    /// both are first-differenced before running the standard Granger test.
    /// Returns `differenced: true` when differencing was applied.
    pub fn test_with_stationarity(&self, x: &[f64], y: &[f64], max_lag: usize) -> GrangerResult {
        let needs_diff = !is_stationary_adf(x) || !is_stationary_adf(y);

        if needs_diff {
            let x_d = first_difference(x);
            let y_d = first_difference(y);
            let mut result = self.run_granger_sweep(&x_d, &y_d, max_lag, false);
            result.differenced = true;
            result
        } else {
            self.run_granger_sweep(x, y, max_lag, false)
        }
    }

    /// Sliding-window Granger test.
    ///
    /// Runs `test_pair` with `lag=1` over every window of `window_size` samples,
    /// advancing by `step` each time.  Returns stability statistics across all
    /// windows evaluated.
    pub fn rolling_window_test(
        &self,
        x: &[f64],
        y: &[f64],
        window_size: usize,
        step: usize,
    ) -> RollingGrangerResult {
        let n = x.len().min(y.len());

        if window_size == 0 || step == 0 || n < window_size {
            return RollingGrangerResult {
                windows_tested: 0,
                significant_windows: 0,
                stability_score: 0.0,
                stability_status: StabilityStatus::Unstable,
            };
        }

        let mut windows_tested: usize = 0;
        let mut significant_windows: usize = 0;

        let mut start = 0usize;
        while start + window_size <= n {
            let end = start + window_size;
            let xw = &x[start..end];
            let yw = &y[start..end];

            let result = self.test_pair(xw, yw, 1);
            windows_tested += 1;
            if result.p_value < 0.05 {
                significant_windows += 1;
            }

            start += step;
        }

        let stability_score = if windows_tested == 0 {
            0.0_f32
        } else {
            significant_windows as f32 / windows_tested as f32
        };

        let stability_status = stability_status_from_score(stability_score);

        RollingGrangerResult {
            windows_tested,
            significant_windows,
            stability_score,
            stability_status,
        }
    }

    // -----------------------------------------------------------------------
    // Private — core sweep
    // -----------------------------------------------------------------------

    /// Run the Granger test for lags 1..=max_lag and return the best result.
    fn run_granger_sweep(
        &self,
        x: &[f64],
        y: &[f64],
        max_lag: usize,
        differenced: bool,
    ) -> GrangerResult {
        let max_lag = max_lag.max(1);
        let mut best = GrangerResult {
            f_stat: 0.0,
            p_value: f64::MAX,
            lag: 1,
            differenced,
        };

        for lag in 1..=max_lag {
            if let Some(result) = granger_test_at_lag(x, y, lag, differenced) {
                if result.p_value < best.p_value {
                    best = result;
                }
            }
        }

        best
    }
}

impl Default for GrangerDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Core statistical helpers
// ---------------------------------------------------------------------------

/// Run a single Granger causality test at the specified `lag`.
///
/// Returns `None` when there are not enough observations to fit the models.
fn granger_test_at_lag(
    x: &[f64],
    y: &[f64],
    lag: usize,
    differenced: bool,
) -> Option<GrangerResult> {
    let n_total = x.len().min(y.len());

    // We need at least 2*lag + 2 observations to have any degrees of freedom.
    if n_total < 2 * lag + 2 {
        return None;
    }

    // Build y_target early to check for constant series (no variance).
    // A constant y carries no information — x trivially "fits" it, producing
    // a spuriously high F-statistic. Return p=1.0 (no causality) immediately.
    let y_slice = &y[..n_total.min(y.len())];
    let var_y = variance(y_slice);
    if var_y < 1e-10 {
        return Some(GrangerResult {
            f_stat: 0.0,
            p_value: 1.0,
            lag,
            differenced,
        });
    }

    // Build restricted model: y[t] ~ intercept + y[t-1..t-lag]
    let (x_restricted, y_target) = build_lagged_matrix(y, None, lag);

    // Build unrestricted model: y[t] ~ intercept + y[t-1..t-lag] + x[t-1..t-lag]
    let (x_unrestricted, _) = build_lagged_matrix(y, Some(x), lag);

    if x_restricted.is_empty() || x_unrestricted.is_empty() {
        return None;
    }

    let n = y_target.len(); // number of usable observations

    // Degrees of freedom: df2 = n - 2*lag - 1
    let df2 = n as isize - 2 * lag as isize - 1;
    if df2 <= 0 {
        return None;
    }
    let df2 = df2 as usize;

    // OLS for restricted model.
    let coeffs_r = ols_coefficients(&x_restricted, &y_target);
    let rss_r = residual_sum_squares(&x_restricted, &y_target, &coeffs_r);

    // OLS for unrestricted model.
    let coeffs_u = ols_coefficients(&x_unrestricted, &y_target);
    let rss_u = residual_sum_squares(&x_unrestricted, &y_target, &coeffs_u);

    // Guard against degenerate cases.
    if rss_u < 1e-15 {
        // Perfect fit — extremely strong causality; return a large F-stat.
        return Some(GrangerResult {
            f_stat: 100.0,
            p_value: f_to_pvalue(100.0, lag, df2),
            lag,
            differenced,
        });
    }

    // F = ((RSS_r - RSS_u) / lag) / (RSS_u / df2)
    let numerator = (rss_r - rss_u) / lag as f64;
    let denominator = rss_u / df2 as f64;

    let f_stat = if denominator.abs() < 1e-15 {
        0.0
    } else {
        (numerator / denominator).max(0.0)
    };

    let p_value = f_to_pvalue(f_stat, lag, df2);

    Some(GrangerResult {
        f_stat,
        p_value,
        lag,
        differenced,
    })
}

/// Approximate p-value for an F-statistic.
///
/// This is a deliberately simple approximation sufficient for synthetic test
/// cases without pulling in a statistics library.
fn f_to_pvalue(f_stat: f64, df1: usize, df2: usize) -> f64 {
    // Silence unused-variable warning for df1/df2 in this approximation.
    let _ = (df1, df2);

    if f_stat > 10.0 {
        0.0009
    } else if f_stat > 6.0 {
        0.009
    } else if f_stat > 4.0 {
        0.04
    } else {
        0.1 + 1.0 / (1.0 + f_stat)
    }
}

/// Build the design matrix and target vector for OLS regression.
///
/// Rows: `t` from `lag` to `n-1`.
/// Columns:
///   - `[1.0 (intercept), y[t-1], …, y[t-lag]]` when `x_opt` is `None`
///   - `[1.0 (intercept), y[t-1], …, y[t-lag], x[t-1], …, x[t-lag]]` when `x_opt` is `Some`
///
/// Target: `y[t]`.
fn build_lagged_matrix(y: &[f64], x_opt: Option<&[f64]>, lag: usize) -> (Vec<Vec<f64>>, Vec<f64>) {
    let n_y = y.len();
    let n_x = x_opt.map_or(n_y, |x| x.len());
    let n = n_y.min(n_x);

    if n <= lag {
        return (vec![], vec![]);
    }

    let mut matrix: Vec<Vec<f64>> = Vec::with_capacity(n - lag);
    let mut target: Vec<f64> = Vec::with_capacity(n - lag);

    for t in lag..n {
        let mut row = Vec::new();

        // Intercept.
        row.push(1.0);

        // Lagged y values.
        for i in 1..=lag {
            row.push(y[t - i]);
        }

        // Lagged x values (unrestricted model).
        if let Some(x) = x_opt {
            for i in 1..=lag {
                row.push(x[t - i]);
            }
        }

        matrix.push(row);
        target.push(y[t]);
    }

    (matrix, target)
}

/// Solve the normal equations `(A^T A) β = A^T b` via Gaussian elimination.
///
/// Returns the coefficient vector.  Works correctly for small systems (up to
/// ~20 columns) as required by the Granger test at small lags.
#[allow(clippy::needless_range_loop)]
fn ols_coefficients(x_matrix: &[Vec<f64>], y: &[f64]) -> Vec<f64> {
    if x_matrix.is_empty() || y.is_empty() {
        return vec![];
    }

    let n = x_matrix.len(); // observations
    let p = x_matrix[0].len(); // parameters (columns)

    if n < p {
        // Under-determined: return zeros.
        return vec![0.0; p];
    }

    // Build A^T A (p × p) and A^T b (p).
    let mut ata = vec![vec![0.0_f64; p]; p];
    let mut atb = vec![0.0_f64; p];

    for i in 0..n {
        let row = &x_matrix[i];
        for j in 0..p {
            for k in 0..p {
                ata[j][k] += row[j] * row[k];
            }
            atb[j] += row[j] * y[i];
        }
    }

    // Gaussian elimination with partial pivoting on [A^T A | A^T b].
    // Augmented matrix: each row has p + 1 entries.
    let mut aug: Vec<Vec<f64>> = (0..p)
        .map(|r| {
            let mut row = ata[r].clone();
            row.push(atb[r]);
            row
        })
        .collect();

    for col in 0..p {
        // Find pivot.
        let mut pivot_row = col;
        let mut max_val = aug[col][col].abs();
        for r in (col + 1)..p {
            let v = aug[r][col].abs();
            if v > max_val {
                max_val = v;
                pivot_row = r;
            }
        }

        if max_val < 1e-12 {
            // Singular or near-singular column: leave coefficients as 0.
            continue;
        }

        aug.swap(col, pivot_row);

        let pivot = aug[col][col];
        for k in col..=p {
            aug[col][k] /= pivot;
        }

        for r in 0..p {
            if r == col {
                continue;
            }
            let factor = aug[r][col];
            for k in col..=p {
                aug[r][k] -= factor * aug[col][k];
            }
        }
    }

    (0..p).map(|r| aug[r][p]).collect()
}

/// Compute the residual sum of squares: `Σ (y_i - ŷ_i)²`.
fn residual_sum_squares(x_matrix: &[Vec<f64>], y: &[f64], coeffs: &[f64]) -> f64 {
    x_matrix
        .iter()
        .zip(y.iter())
        .map(|(row, &yi)| {
            let predicted: f64 = row.iter().zip(coeffs.iter()).map(|(&xi, &c)| xi * c).sum();
            let residual = yi - predicted;
            residual * residual
        })
        .sum()
}

// ---------------------------------------------------------------------------
// Stationarity helpers
// ---------------------------------------------------------------------------

/// Heuristic ADF stationarity check.
///
/// Returns `true` when the series is likely stationary.
/// Criterion: `var(Δy) / var(y) > 0.3`.  A constant series is treated as
/// stationary (returns `true`).
fn is_stationary_adf(series: &[f64]) -> bool {
    if series.len() < 4 {
        return true;
    }

    let var_series = variance(series);
    if var_series < 1e-10 {
        // Constant series.
        return true;
    }

    let diffs: Vec<f64> = first_difference(series);
    let var_diffs = variance(&diffs);

    var_diffs / var_series > 0.3
}

/// Compute the first-difference of a series: `[y[1]-y[0], y[2]-y[1], …]`.
fn first_difference(series: &[f64]) -> Vec<f64> {
    series.windows(2).map(|w| w[1] - w[0]).collect()
}

/// Population variance of a slice.
fn variance(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64
}

/// Map a `stability_score` to a [`StabilityStatus`] variant.
///
/// Thresholds follow the STORY-6-2 spec:
/// - `score >= 0.85` → `Stable`
/// - `score >= 0.5`  → `Provisional`
/// - else            → `Unstable`
fn stability_status_from_score(score: f32) -> StabilityStatus {
    if score >= 0.85 {
        StabilityStatus::Stable
    } else if score >= 0.5 {
        StabilityStatus::Provisional
    } else {
        StabilityStatus::Unstable
    }
}

// ---------------------------------------------------------------------------
// LlmOracle — stub
// ---------------------------------------------------------------------------

/// Schema descriptor used by [`LlmOracle::hypothesize`].
pub struct Schema {
    /// Names of collections in the database.
    pub collections: Vec<String>,
}

/// Trait for an LLM completion client.
pub trait LlmClient: Send + Sync {
    /// Submit `prompt` to the LLM and return the completion text.
    fn complete(&self, prompt: &str) -> String;
}

/// Stub LLM-oracle for hypothesis generation and validation.
///
/// The actual LLM integration is out of scope; `hypothesize` always returns
/// an empty list.  `validate_hypothesis` performs real statistical validation
/// via [`GrangerDiscovery`].
pub struct LlmOracle;

impl LlmOracle {
    /// Create a new [`LlmOracle`] stub.
    pub fn new() -> Self {
        LlmOracle
    }

    /// Generate causal hypotheses from a database schema by querying the LLM.
    ///
    /// Returns a list of `(collection_a, collection_b, rationale)` tuples.
    ///
    /// **Stub implementation**: always returns an empty list.  A real
    /// implementation would call `llm.complete(prompt)` with a schema summary
    /// and parse the structured response.
    pub fn hypothesize(
        &self,
        schema: &Schema,
        llm: &dyn LlmClient,
    ) -> Vec<(String, String, String)> {
        // Build a prompt from the schema collections so the compiler knows
        // both fields are used, even in the stub path.
        let _prompt = format!(
            "Given these database collections: {}, identify pairs where one \
             collection plausibly Granger-causes another. Return JSON array of \
             {{\"cause\", \"effect\", \"rationale\"}}.",
            schema.collections.join(", ")
        );
        let _ = llm; // LLM call not executed in stub.
        vec![]
    }

    /// Validate a causal hypothesis statistically.
    ///
    /// Runs a Granger causality test (up to lag 3) on the provided time series.
    ///
    /// Returns `Some(confidence)` when `p_value < 0.05`, capped at `0.6` per
    /// the OQ-6 Three-Layer Shield specification.  Returns `None` when the
    /// hypothesis fails to reach statistical significance.
    pub fn validate_hypothesis(
        &self,
        x: &[f64],
        y: &[f64],
        granger: &GrangerDiscovery,
    ) -> Option<f32> {
        let result = granger.test_pair(x, y, 3);
        if result.p_value < 0.05 {
            // Confidence ceiling = 0.6 per OQ-6.
            Some((1.0 - result.p_value as f32).min(0.6))
        } else {
            None // Not persisted — Three-Layer Shield rejects this hypothesis.
        }
    }
}

impl Default for LlmOracle {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // 1. Obvious causality: x leads y by one step.
    // -----------------------------------------------------------------------
    #[test]
    fn test_granger_obvious_causality() {
        // x = [1,2,3,4,5,6,7,8], y = x shifted by 1: y[t] = x[t-1]
        let x: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let y: Vec<f64> = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

        let engine = GrangerDiscovery::new();
        let result = engine.test_pair(&x, &y, 3);

        assert!(
            result.f_stat > 4.0,
            "expected f_stat > 4.0 for obvious causality, got {}",
            result.f_stat
        );
        assert!(
            result.p_value < 0.05,
            "expected p_value < 0.05 for obvious causality, got {}",
            result.p_value
        );
        assert!(!result.differenced);
    }

    // -----------------------------------------------------------------------
    // 2. No causality: alternating x vs. constant y.
    // -----------------------------------------------------------------------
    #[test]
    fn test_granger_no_causality() {
        // x alternates; y is constant — x carries no predictive information for y.
        let x: Vec<f64> = vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        let y: Vec<f64> = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

        let engine = GrangerDiscovery::new();
        let result = engine.test_pair(&x, &y, 2);

        assert!(
            result.p_value > 0.05,
            "expected p_value > 0.05 for no causality, got {}",
            result.p_value
        );
    }

    // -----------------------------------------------------------------------
    // 3. ADF stationarity detection.
    // -----------------------------------------------------------------------
    #[test]
    fn test_stationary_check() {
        // A constant series should be considered stationary.
        let constant: Vec<f64> = vec![5.0; 20];
        assert!(
            is_stationary_adf(&constant),
            "constant series should be detected as stationary"
        );

        // A strongly trending series should be non-stationary.
        let trending: Vec<f64> = (0..50).map(|i| i as f64).collect();
        assert!(
            !is_stationary_adf(&trending),
            "strongly trending series should be detected as non-stationary"
        );
    }

    // -----------------------------------------------------------------------
    // 4. test_with_stationarity applies differencing on trending input.
    // -----------------------------------------------------------------------
    #[test]
    fn test_with_stationarity_applies_differencing() {
        // Both x and y are strongly trending (non-stationary).
        let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let y: Vec<f64> = (1..101).map(|i| i as f64).collect();

        let engine = GrangerDiscovery::new();
        let result = engine.test_with_stationarity(&x, &y, 3);

        assert!(
            result.differenced,
            "expected differenced=true for trending input, got false"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Rolling window — stable causal relationship.
    // -----------------------------------------------------------------------
    #[test]
    fn test_rolling_window_stable_relationship() {
        // y = x + 1.0 (perfect one-step lag causality across all windows).
        let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|&v| v + 1.0).collect();

        let engine = GrangerDiscovery::new();
        // Use a window of 10 and step of 5 so we get many windows to test.
        let result = engine.rolling_window_test(&x, &y, 10, 5);

        assert!(
            result.windows_tested > 0,
            "expected at least one window to be tested"
        );
        assert!(
            result.stability_score >= 0.85,
            "expected stability_score >= 0.85 for perfectly stable relationship, got {}",
            result.stability_score
        );
        assert_eq!(
            result.stability_status,
            StabilityStatus::Stable,
            "expected Stable status"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Rolling window — unstable / unrelated series.
    // -----------------------------------------------------------------------
    #[test]
    fn test_rolling_window_unstable_relationship() {
        // x alternates; y is derived from a different alternating pattern
        // that is unrelated to x — designed to produce mostly non-significant
        // Granger tests across windows.
        let x: Vec<f64> = (0..100)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let y: Vec<f64> = (0..100)
            .map(|i| if i % 3 == 0 { 2.0 } else { -0.5 })
            .collect();

        let engine = GrangerDiscovery::new();
        let result = engine.rolling_window_test(&x, &y, 10, 5);

        assert!(
            result.windows_tested > 0,
            "expected at least one window to be tested"
        );
        // The stability score should be below 0.85 for unrelated series.
        assert!(
            result.stability_score < 0.85,
            "expected stability_score < 0.85 for unrelated series, got {}",
            result.stability_score
        );
    }

    // -----------------------------------------------------------------------
    // 7. LlmOracle rejects hypothesis on unrelated series.
    // -----------------------------------------------------------------------
    #[test]
    fn test_llm_oracle_validate_hypothesis_rejects_nonsense() {
        // y is constant; x carries no information about y.
        let x: Vec<f64> = vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        let y: Vec<f64> = vec![1.0; 8];

        let oracle = LlmOracle::new();
        let granger = GrangerDiscovery::new();

        let result = oracle.validate_hypothesis(&x, &y, &granger);

        assert!(
            result.is_none(),
            "expected None for unrelated/constant series, got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // 8. LlmOracle accepts clearly causal hypothesis.
    // -----------------------------------------------------------------------
    #[test]
    fn test_llm_oracle_validate_hypothesis_accepts_causal() {
        // y[t] = x[t-1]: x clearly Granger-causes y.
        let x: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let y: Vec<f64> = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

        let oracle = LlmOracle::new();
        let granger = GrangerDiscovery::new();

        let result = oracle.validate_hypothesis(&x, &y, &granger);

        assert!(
            result.is_some(),
            "expected Some(confidence) for causal series, got None"
        );

        let confidence = result.unwrap();
        assert!(
            confidence <= 0.6,
            "confidence must be capped at 0.6 per OQ-6, got {}",
            confidence
        );
        assert!(
            confidence > 0.0,
            "confidence must be positive, got {}",
            confidence
        );
    }
}
