//! Physical operator implementations for the FunDB query executor.
//!
//! Each operator is a concrete struct with an `async fn execute(&self) -> Result<RecordBatch>`
//! method.  The executor dispatches to the appropriate operator based on the
//! [`LogicalPlan`](fundb_sql::LogicalPlan) variant it is processing.
//!
//! # Operator catalogue
//!
//! | Module      | Operator(s)                                 |
//! |-------------|---------------------------------------------|
//! | `scan`      | [`ScanOperator`], [`VectorScanOperator`]    |
//! | `filter`    | [`FilterOperator`]                          |
//! | `project`   | [`ProjectOperator`]                         |
//! | `sort`      | [`SortOperator`]                            |

pub mod filter;
pub mod project;
pub mod scan;
pub mod sort;

pub use filter::FilterOperator;
pub use project::ProjectOperator;
pub use scan::{ScanOperator, VectorScanOperator};
pub use sort::SortOperator;
