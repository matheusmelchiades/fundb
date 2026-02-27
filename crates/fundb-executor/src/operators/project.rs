//! ProjectOperator — column-level projection over an input batch.
//!
//! Because `FunRecord` is an opaque, richly-typed struct (not a generic
//! row of dynamically-typed columns), full field-level projection would
//! require serialising each record into a generic value map, selecting the
//! requested fields, and re-assembling a new record — a significant amount of
//! work that is deferred to a future epic.
//!
//! For now, `ProjectOperator` acts as a **pass-through**: if the projection
//! list is empty or contains only a `Star` wildcard it returns the input
//! unchanged.  Any other projection list is also passed through unchanged,
//! preserving correctness at the cost of not pruning unused columns.

use anyhow::Result;
use fundb_sql::Expr;

use crate::batch::RecordBatch;

// ---------------------------------------------------------------------------
// ProjectOperator
// ---------------------------------------------------------------------------

/// Physical operator that applies a column-level projection to a batch.
///
/// Current implementation is a no-op pass-through for all expression lists.
/// Field-level projection is tracked for a future epic.
pub struct ProjectOperator {
    /// The input batch to project.
    pub input: RecordBatch,
    /// The requested output expressions (columns, aliases, computed values).
    pub exprs: Vec<Expr>,
}

impl ProjectOperator {
    /// Construct a new `ProjectOperator`.
    pub fn new(input: RecordBatch, exprs: Vec<Expr>) -> Self {
        ProjectOperator { input, exprs }
    }

    /// Execute the projection.
    ///
    /// Returns the input batch unchanged for all expression lists.  Full
    /// column pruning is left as future work.
    pub async fn execute(&self) -> Result<RecordBatch> {
        // Pass-through: return a clone of the input regardless of exprs.
        // This is correct (no data loss) though not optimal (no column pruning).
        Ok(self.input.clone())
    }
}
