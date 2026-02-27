//! FunDB query executor — STORY-4-3.
//!
//! The [`Executor`] is the top-level entry point for query execution.  It
//! accepts a [`LogicalPlan`] (produced by the binder), runs the
//! [`fundb_optimizer`] over it, and dispatches to the appropriate physical
//! operator tree.
//!
//! # Architecture
//!
//! ```text
//! LogicalPlan
//!     │  optimize()
//!     ▼
//! LogicalPlan (optimized)
//!     │  execute_plan()
//!     ▼
//! Physical operator(s)   →   RecordBatch
//! ```
//!
//! Physical operators are concrete async structs (no dyn-trait overhead) in
//! the [`operators`] module.  Each operator's `execute` method returns a
//! [`RecordBatch`].

pub mod batch;
pub mod operators;

use std::sync::Arc;

use anyhow::Result;
use fundb_optimizer::optimize;
use fundb_sql::LogicalPlan;
use fundb_storage::LsmTree;

use crate::batch::RecordBatch;
use crate::operators::{FilterOperator, ProjectOperator, ScanOperator, VectorScanOperator};

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Top-level query executor.
///
/// Wraps a shared reference to the storage engine and provides an
/// [`Executor::execute`] method that drives the full pipeline from
/// `LogicalPlan → optimize → physical operators → RecordBatch`.
pub struct Executor {
    lsm: Arc<LsmTree>,
}

impl Executor {
    /// Create a new `Executor` backed by the given [`LsmTree`].
    pub fn new(lsm: Arc<LsmTree>) -> Self {
        Executor { lsm }
    }

    /// Execute a [`LogicalPlan`] and return the resulting [`RecordBatch`].
    ///
    /// Applies the optimizer pass before physical execution.
    pub async fn execute(&self, plan: LogicalPlan) -> Result<RecordBatch> {
        let plan = optimize(plan);
        self.execute_plan(plan).await
    }

    // -----------------------------------------------------------------------
    // Internal dispatch
    // -----------------------------------------------------------------------

    /// Recursively execute a (possibly nested) [`LogicalPlan`] tree.
    async fn execute_plan(&self, plan: LogicalPlan) -> Result<RecordBatch> {
        match plan {
            // ── Full-collection scan ─────────────────────────────────────
            LogicalPlan::Scan {
                collection,
                predicate,
                projections,
            } => {
                let scan_op = ScanOperator::new(collection, Arc::clone(&self.lsm));
                let mut batch = scan_op.execute().await?;

                // Optionally push down a predicate that was not pushed below
                // the scan by the optimizer.
                if let Some(pred) = predicate {
                    batch = FilterOperator::new(batch, pred).execute().await?;
                }

                // Apply projections.
                if !projections.is_empty() {
                    batch = ProjectOperator::new(batch, projections).execute().await?;
                }

                Ok(batch)
            }

            // ── Vector / ANN scan ────────────────────────────────────────
            LogicalPlan::VectorScan {
                collection,
                vector_field,
                query,
                threshold,
            } => {
                VectorScanOperator::new(
                    collection,
                    vector_field,
                    query,
                    threshold,
                    Arc::clone(&self.lsm),
                )
                .execute()
                .await
            }

            // ── Predicate filter ─────────────────────────────────────────
            LogicalPlan::Filter { input, predicate } => {
                let batch = self.execute_plan(*input).await?;
                FilterOperator::new(batch, predicate).execute().await
            }

            // ── Column projection ────────────────────────────────────────
            LogicalPlan::Project { input, exprs } => {
                let batch = self.execute_plan(*input).await?;
                ProjectOperator::new(batch, exprs).execute().await
            }

            // ── Row limit ────────────────────────────────────────────────
            LogicalPlan::Limit { input, n } => {
                let mut batch = self.execute_plan(*input).await?;
                batch.truncate(n);
                Ok(batch)
            }

            // ── Sort (ordering not yet implemented) ──────────────────────
            LogicalPlan::Sort { input, order_by: _ } => {
                // Row order is preserved from the scan; full sort is deferred.
                self.execute_plan(*input).await
            }

            // ── Placeholder / empty result ───────────────────────────────
            LogicalPlan::Empty => Ok(RecordBatch::new()),

            // ── Stubs for future epics ───────────────────────────────────
            // Join, Aggregate, GraphTraverse, CausalTrace, ContextOptimize,
            // Understand, EstimateEffect, Counterfactual all return empty
            // batches until their respective execution stories are implemented.
            _ => Ok(RecordBatch::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_sql::{Expr, Literal, LogicalPlan};
    use tempfile::tempdir;

    // ── Helper: open a fresh LsmTree in a temp directory ─────────────────

    fn open_lsm() -> (tempfile::TempDir, Arc<LsmTree>) {
        let tmp = tempdir().expect("failed to create temp dir");
        let lsm = LsmTree::open(tmp.path(), 1024 * 1024).expect("failed to open LsmTree");
        (tmp, Arc::new(lsm))
    }

    // ── Test 1: scan an empty collection ─────────────────────────────────

    #[tokio::test]
    async fn test_executor_scan_empty() {
        let (_tmp, lsm) = open_lsm();
        let executor = Executor::new(lsm);

        let plan = LogicalPlan::Scan {
            collection: "test".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        };

        let batch = executor.execute(plan).await.expect("execute failed");
        assert!(
            batch.is_empty(),
            "scanning an empty collection must return an empty batch"
        );
    }

    // ── Test 2: filter with a trivially-true predicate ───────────────────

    #[tokio::test]
    async fn test_executor_filter_plan() {
        let (_tmp, lsm) = open_lsm();
        let executor = Executor::new(lsm);

        let plan = LogicalPlan::Filter {
            input: Box::new(LogicalPlan::Scan {
                collection: "test".to_string(),
                predicate: None,
                projections: vec![],
            }),
            predicate: Expr::Literal(Literal::Bool(true)),
        };

        let batch = executor.execute(plan).await.expect("execute failed");
        // No records were written, so even with a pass-through predicate the
        // result must be empty.
        assert!(
            batch.is_empty(),
            "filtering an empty scan must return an empty batch"
        );
    }

    // ── Test 3: limit applied to an empty scan ────────────────────────────

    #[tokio::test]
    async fn test_executor_limit() {
        let (_tmp, lsm) = open_lsm();
        let executor = Executor::new(lsm);

        let plan = LogicalPlan::Limit {
            input: Box::new(LogicalPlan::Scan {
                collection: "test".to_string(),
                predicate: None,
                projections: vec![],
            }),
            n: 5,
        };

        let batch = executor.execute(plan).await.expect("execute failed");
        assert!(
            batch.is_empty(),
            "limiting an empty scan must return an empty batch"
        );
        assert!(
            batch.len() <= 5,
            "batch length must be at most the limit (5)"
        );
    }

    // ── Test 4: Empty plan returns an empty batch ─────────────────────────

    #[tokio::test]
    async fn test_executor_empty_plan() {
        let (_tmp, lsm) = open_lsm();
        let executor = Executor::new(lsm);

        let batch = executor
            .execute(LogicalPlan::Empty)
            .await
            .expect("execute failed");
        assert!(
            batch.is_empty(),
            "LogicalPlan::Empty must produce an empty RecordBatch"
        );
    }
}
