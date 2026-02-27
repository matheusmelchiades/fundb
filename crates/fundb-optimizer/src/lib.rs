/// FunDB query optimizer.
///
/// Applies a chain of algebraic rewriting rules to a `LogicalPlan` tree until
/// the plan reaches a fixed point (no further rule fires), or until the maximum
/// iteration limit is reached.
pub mod rules;
pub mod cost;

use fundb_sql::LogicalPlan;

/// Apply all optimization rules in a fixed-point loop (max 8 iterations).
///
/// The loop terminates early when a full rule pass produces no change in the
/// plan (detected via `Debug` representation equality).
pub fn optimize(plan: LogicalPlan) -> LogicalPlan {
    let mut current = plan;
    for _ in 0..8 {
        let next = apply_all_rules(current.clone());
        if plans_equal(&next, &current) {
            break;
        }
        current = next;
    }
    current
}

/// Run every rule once over the plan in a defined order.
fn apply_all_rules(plan: LogicalPlan) -> LogicalPlan {
    use rules::*;
    let plan = predicate_pushdown(plan);
    let plan = limit_pushdown(plan);
    let plan = eliminate_redundant_project(plan);
    let plan = constant_fold(plan);
    let plan = vector_scan_elide_filter(plan);
    plan
}

/// Structural equality check used to detect fixed-point convergence.
///
/// Uses the `Debug` format as a cheap structural fingerprint — this is
/// intentionally simple: real production use would employ a proper `PartialEq`
/// derivation, but that requires all contained types to implement `PartialEq`.
fn plans_equal(a: &LogicalPlan, b: &LogicalPlan) -> bool {
    format!("{:?}", a) == format!("{:?}", b)
}

pub use cost::{CostOptimizer, Histogram, PhysicalPlan, Statistics};
