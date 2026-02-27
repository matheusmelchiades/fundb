/// Confidence propagation rules for FunDB's AI-native query engine.
///
/// All methods are pure functions — no state, no I/O.  `ConfidencePropagator`
/// is a unit struct; every method is static (no `&self` receiver).
///
/// Confidence values are `f32` scalars in `[0.0, 1.0]` where 0.0 means no
/// confidence and 1.0 means certainty.
pub struct ConfidencePropagator;

impl ConfidencePropagator {
    // -----------------------------------------------------------------------
    // Relational-algebra propagation rules
    // -----------------------------------------------------------------------

    /// JOIN confidence: the result can only be as confident as its weakest input.
    ///
    /// Formula: `min(a, b)`
    pub fn propagate_join(a: f32, b: f32) -> f32 {
        a.min(b)
    }

    /// UNION confidence: take the best available evidence.
    ///
    /// Formula: `max(a, b)`
    pub fn propagate_union(a: f32, b: f32) -> f32 {
        a.max(b)
    }

    /// AGGREGATE confidence: weighted average of (confidence, weight) pairs.
    ///
    /// Formula: `Σ(confidence_i × weight_i) / Σ(weight_i)`
    ///
    /// Returns `0.0` if `scores` is empty or all weights sum to zero.
    pub fn propagate_aggregate(scores: &[(f32, f32)]) -> f32 {
        if scores.is_empty() {
            return 0.0;
        }

        let (weighted_sum, weight_sum) = scores
            .iter()
            .fold((0.0_f32, 0.0_f32), |(ws, wt), &(confidence, weight)| {
                (ws + confidence * weight, wt + weight)
            });

        if weight_sum == 0.0 {
            return 0.0;
        }

        weighted_sum / weight_sum
    }

    /// Graph-path confidence: product of all node and edge confidences.
    ///
    /// Formula: `Π(nodes) × Π(edges)`
    ///
    /// Used for causal / provenance chain reasoning.  Returns `1.0` when both
    /// slices are empty (multiplicative identity).
    pub fn propagate_graph_path(nodes: &[f32], edges: &[f32]) -> f32 {
        nodes.iter().chain(edges.iter()).copied().product::<f32>()
    }

    // -----------------------------------------------------------------------
    // Logical / epistemic operators
    // -----------------------------------------------------------------------

    /// Negation confidence: confidence that the complement is true.
    ///
    /// Formula: `1.0 - c`
    pub fn propagate_negation(c: f32) -> f32 {
        1.0 - c
    }

    /// Confidence assigned to a claim that has `n` equal-weight contradictions.
    ///
    /// Formula: `0.5 + 1.0 / (2.0 * n)`
    ///
    /// | n  | result  |
    /// |----|---------|
    /// | 1  | 1.0     |
    /// | 2  | 0.75    |
    /// | 3  | 0.6̄    |
    /// | 10 | 0.55    |
    ///
    /// Returns `0.5` when `n == 0` (maximum uncertainty / coin-flip).
    pub fn confidence_for_n_equal_contradictions(n: u32) -> f32 {
        if n == 0 {
            return 0.5;
        }
        0.5 + 1.0 / (2.0 * n as f32)
    }

    // -----------------------------------------------------------------------
    // Dynamic update rules
    // -----------------------------------------------------------------------

    /// Increase confidence when a new supporting source is added.
    ///
    /// Formula: `min(1.0, current + 0.1)`
    pub fn update_on_corroboration(current: f32) -> f32 {
        (current + 0.1).min(1.0)
    }

    /// Decrease confidence when a contradicting record is linked.
    ///
    /// Formula: `max(0.0, current - 0.2 * contradiction_confidence)`
    pub fn update_on_contradiction(current: f32, contradiction_confidence: f32) -> f32 {
        (current - 0.2 * contradiction_confidence).max(0.0)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::ConfidencePropagator as CP;

    /// Convenience: assert two f32 values are within 0.001 of each other.
    fn approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.001,
            "expected {expected} but got {actual} (diff = {})",
            (actual - expected).abs()
        );
    }

    // --- propagate_join -----------------------------------------------------

    #[test]
    fn join_takes_minimum() {
        approx_eq(CP::propagate_join(0.9, 0.8), 0.8);
    }

    #[test]
    fn join_is_commutative() {
        let a = 0.9_f32;
        let b = 0.8_f32;
        approx_eq(CP::propagate_join(a, b), CP::propagate_join(b, a));
    }

    #[test]
    fn join_equal_inputs() {
        approx_eq(CP::propagate_join(0.5, 0.5), 0.5);
    }

    #[test]
    fn join_extreme_values() {
        approx_eq(CP::propagate_join(1.0, 0.0), 0.0);
        approx_eq(CP::propagate_join(0.0, 1.0), 0.0);
    }

    // --- propagate_union ----------------------------------------------------

    #[test]
    fn union_takes_maximum() {
        approx_eq(CP::propagate_union(0.7, 0.9), 0.9);
    }

    #[test]
    fn union_is_commutative() {
        let a = 0.3_f32;
        let b = 0.6_f32;
        approx_eq(CP::propagate_union(a, b), CP::propagate_union(b, a));
    }

    #[test]
    fn union_extreme_values() {
        approx_eq(CP::propagate_union(1.0, 0.0), 1.0);
        approx_eq(CP::propagate_union(0.0, 1.0), 1.0);
    }

    // --- propagate_aggregate ------------------------------------------------

    #[test]
    fn aggregate_weighted_average() {
        // (0.8 × 1.0 + 0.6 × 2.0) / (1.0 + 2.0) = 2.0 / 3.0 ≈ 0.667
        let scores = [(0.8_f32, 1.0_f32), (0.6_f32, 2.0_f32)];
        approx_eq(CP::propagate_aggregate(&scores), 0.6667);
    }

    #[test]
    fn aggregate_empty_returns_zero() {
        approx_eq(CP::propagate_aggregate(&[]), 0.0);
    }

    #[test]
    fn aggregate_single_entry_returns_that_confidence() {
        let scores = [(0.75_f32, 3.0_f32)];
        approx_eq(CP::propagate_aggregate(&scores), 0.75);
    }

    #[test]
    fn aggregate_equal_weights_is_simple_average() {
        let scores = [(0.4_f32, 1.0_f32), (0.6_f32, 1.0_f32)];
        approx_eq(CP::propagate_aggregate(&scores), 0.5);
    }

    // --- propagate_graph_path -----------------------------------------------

    #[test]
    fn graph_path_causal_chain() {
        // 0.9 × 0.85 × 0.7 = 0.5355
        approx_eq(CP::propagate_graph_path(&[0.9, 0.85, 0.7], &[]), 0.5355);
    }

    #[test]
    fn graph_path_empty_slices_returns_one() {
        approx_eq(CP::propagate_graph_path(&[], &[]), 1.0);
    }

    #[test]
    fn graph_path_nodes_and_edges_multiplied_together() {
        // nodes: 0.9, 0.8 → 0.72; edges: 0.5 → 0.36
        approx_eq(CP::propagate_graph_path(&[0.9, 0.8], &[0.5]), 0.36);
    }

    #[test]
    fn graph_path_only_edges() {
        approx_eq(CP::propagate_graph_path(&[], &[0.8, 0.5]), 0.4);
    }

    // --- propagate_negation -------------------------------------------------

    #[test]
    fn negation_complement() {
        approx_eq(CP::propagate_negation(0.3), 0.7);
    }

    #[test]
    fn negation_of_zero_is_one() {
        approx_eq(CP::propagate_negation(0.0), 1.0);
    }

    #[test]
    fn negation_of_one_is_zero() {
        approx_eq(CP::propagate_negation(1.0), 0.0);
    }

    #[test]
    fn negation_involutory() {
        let c = 0.6_f32;
        approx_eq(CP::propagate_negation(CP::propagate_negation(c)), c);
    }

    // --- confidence_for_n_equal_contradictions ------------------------------

    #[test]
    fn contradictions_n1_is_one() {
        approx_eq(CP::confidence_for_n_equal_contradictions(1), 1.0);
    }

    #[test]
    fn contradictions_n2_is_0_75() {
        approx_eq(CP::confidence_for_n_equal_contradictions(2), 0.75);
    }

    #[test]
    fn contradictions_n3_approx_0_667() {
        approx_eq(CP::confidence_for_n_equal_contradictions(3), 0.6667);
    }

    #[test]
    fn contradictions_n10_approx_0_55() {
        approx_eq(CP::confidence_for_n_equal_contradictions(10), 0.55);
    }

    #[test]
    fn contradictions_n0_is_0_5() {
        approx_eq(CP::confidence_for_n_equal_contradictions(0), 0.5);
    }

    // --- update_on_corroboration --------------------------------------------

    #[test]
    fn corroboration_basic_increase() {
        approx_eq(CP::update_on_corroboration(0.8), 0.9);
    }

    #[test]
    fn corroboration_clamped_at_one() {
        approx_eq(CP::update_on_corroboration(0.95), 1.0);
    }

    #[test]
    fn corroboration_exactly_one_stays_one() {
        approx_eq(CP::update_on_corroboration(1.0), 1.0);
    }

    // --- update_on_contradiction --------------------------------------------

    #[test]
    fn contradiction_reduces_confidence() {
        // 0.9 - 0.2 * 1.0 = 0.7
        approx_eq(CP::update_on_contradiction(0.9, 1.0), 0.7);
    }

    #[test]
    fn contradiction_clamped_at_zero() {
        // 0.1 - 0.2 * 1.0 = -0.1 → clamped to 0.0
        approx_eq(CP::update_on_contradiction(0.1, 1.0), 0.0);
    }

    #[test]
    fn contradiction_partial_confidence_reducer() {
        // 0.8 - 0.2 * 0.5 = 0.8 - 0.1 = 0.7
        approx_eq(CP::update_on_contradiction(0.8, 0.5), 0.7);
    }

    #[test]
    fn contradiction_zero_confidence_contradiction_is_noop() {
        approx_eq(CP::update_on_contradiction(0.6, 0.0), 0.6);
    }

    // --- property-based style: commutativity of join ------------------------

    #[test]
    fn join_commutative_property_multiple_values() {
        let pairs: &[(f32, f32)] = &[
            (0.0, 1.0),
            (0.5, 0.5),
            (0.3, 0.9),
            (1.0, 0.0),
            (0.7, 0.2),
            (0.85, 0.85),
            (0.1, 0.99),
        ];
        for &(a, b) in pairs {
            approx_eq(CP::propagate_join(a, b), CP::propagate_join(b, a));
        }
    }
}
