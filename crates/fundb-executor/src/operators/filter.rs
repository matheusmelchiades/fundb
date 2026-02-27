//! FilterOperator — evaluates a scalar predicate row-by-row.

use anyhow::Result;
use fundb_core::FunRecord;
use fundb_sql::{BinaryOp, Expr, Literal, UnaryOp};

use crate::batch::RecordBatch;

// ---------------------------------------------------------------------------
// FilterOperator
// ---------------------------------------------------------------------------

/// Physical operator that keeps only rows for which a scalar predicate
/// evaluates to `true`.
///
/// Unsupported expression forms return `true` (pass-through) so that partial
/// pushdown does not silently drop rows whose predicates cannot yet be
/// evaluated at the physical layer.
pub struct FilterOperator {
    /// The input batch to filter.
    pub input: RecordBatch,
    /// The scalar predicate to evaluate against each row.
    pub predicate: Expr,
}

impl FilterOperator {
    /// Construct a new `FilterOperator`.
    pub fn new(input: RecordBatch, predicate: Expr) -> Self {
        FilterOperator { input, predicate }
    }

    /// Apply the predicate to every row in `input`, returning a new batch that
    /// contains only the rows for which the predicate is `true`.
    pub async fn execute(&self) -> Result<RecordBatch> {
        let mut result = RecordBatch::new();
        for (key, record) in self
            .input
            .keys
            .iter()
            .zip(self.input.records.iter())
        {
            if self.eval_predicate(record) {
                result.push(key.clone(), record.clone());
            }
        }
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Predicate evaluation
    // -----------------------------------------------------------------------

    /// Evaluate `self.predicate` against a single record.
    ///
    /// Returns `true` for predicate forms that are not yet supported so that
    /// the operator acts as a no-op pass-through for unsupported expressions.
    fn eval_predicate(&self, record: &FunRecord) -> bool {
        self.eval_expr(&self.predicate, record)
    }

    /// Recursively evaluate an [`Expr`] against a single record.
    fn eval_expr(&self, expr: &Expr, record: &FunRecord) -> bool {
        match expr {
            // ----------------------------------------------------------------
            // Literals
            // ----------------------------------------------------------------
            Expr::Literal(Literal::Bool(b)) => *b,
            Expr::Literal(Literal::Null) => false,

            // ----------------------------------------------------------------
            // Logical connectives
            // ----------------------------------------------------------------
            Expr::BinaryOp {
                op: BinaryOp::And,
                left,
                right,
            } => self.eval_expr(left, record) && self.eval_expr(right, record),

            Expr::BinaryOp {
                op: BinaryOp::Or,
                left,
                right,
            } => self.eval_expr(left, record) || self.eval_expr(right, record),

            // ----------------------------------------------------------------
            // Unary NOT
            // ----------------------------------------------------------------
            Expr::UnaryOp {
                op: UnaryOp::Not,
                operand,
            } => !self.eval_expr(operand, record),

            // ----------------------------------------------------------------
            // Equality / inequality on _confidence (numeric)
            // ----------------------------------------------------------------
            Expr::BinaryOp {
                op,
                left,
                right,
            } => self.eval_binary(op, left, right, record),

            // ----------------------------------------------------------------
            // Everything else → pass-through (return true)
            // ----------------------------------------------------------------
            _ => true,
        }
    }

    /// Evaluate a binary comparison expression against a record.
    ///
    /// Supported patterns:
    /// - `Column("_confidence") <op> Literal(Float | Int)` — compares the
    ///   record's `_confidence` field.
    /// - `Column("_confidence") <op> Literal(Float | Int)` with swapped
    ///   operands (literal on the left).
    /// - `Column(<name>) = Literal(String(s))` — equality check against a
    ///   built-in string field (`_collection`, `_tenant` string form, etc.).
    ///
    /// Returns `true` for any pattern that is not explicitly handled.
    fn eval_binary(
        &self,
        op: &BinaryOp,
        left: &Expr,
        right: &Expr,
        record: &FunRecord,
    ) -> bool {
        // ── Numeric comparisons on _confidence ────────────────────────────
        // Pattern: Column("_confidence") <op> Literal(numeric)
        if let (Expr::Column(col), Some(rhs_f)) = (left, extract_float(right)) {
            if col == "_confidence" {
                return compare_f32(record._confidence, op, rhs_f as f32);
            }
        }
        // Swapped: Literal(numeric) <op> Column("_confidence")
        if let (Some(lhs_f), Expr::Column(col)) = (extract_float(left), right) {
            if col == "_confidence" {
                // Flip the operator direction.
                return compare_f32(lhs_f as f32, &flip_op(op), record._confidence);
            }
        }

        // ── String equality on built-in fields ───────────────────────────
        if let (Expr::Column(col), Expr::Literal(Literal::String(s))) = (left, right) {
            return match col.as_str() {
                "_collection" => &record._collection == s,
                // Unknown / unsupported field → pass-through
                _ => true,
            };
        }
        // Swapped literal / column
        if let (Expr::Literal(Literal::String(s)), Expr::Column(col)) = (left, right) {
            if let BinaryOp::Eq | BinaryOp::Ne = op {
                return match col.as_str() {
                    "_collection" => {
                        let eq = &record._collection == s;
                        if *op == BinaryOp::Ne { !eq } else { eq }
                    }
                    _ => true,
                };
            }
        }

        // ── Int equality on _tenant ───────────────────────────────────────
        if let (Expr::Column(col), Expr::Literal(Literal::Int(i))) = (left, right) {
            if col == "_tenant" {
                return match op {
                    BinaryOp::Eq => record._tenant == *i as u32,
                    BinaryOp::Ne => record._tenant != *i as u32,
                    _ => true,
                };
            }
        }

        // Unsupported → pass-through
        true
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a numeric value from a Literal as `f64`, returning `None` for
/// non-numeric literals.
fn extract_float(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Literal(Literal::Float(f)) => Some(*f),
        Expr::Literal(Literal::Int(i)) => Some(*i as f64),
        _ => None,
    }
}

/// Compare two `f32` values using the given binary operator.
///
/// Returns `true` for non-comparison operators (e.g. `Add`, `VectorDist`).
fn compare_f32(lhs: f32, op: &BinaryOp, rhs: f32) -> bool {
    match op {
        BinaryOp::Eq => (lhs - rhs).abs() < f32::EPSILON,
        BinaryOp::Ne => (lhs - rhs).abs() >= f32::EPSILON,
        BinaryOp::Lt => lhs < rhs,
        BinaryOp::Le => lhs <= rhs,
        BinaryOp::Gt => lhs > rhs,
        BinaryOp::Ge => lhs >= rhs,
        // Non-comparison operators → pass-through
        _ => true,
    }
}

/// Reverse the direction of a comparison operator for when operands are
/// swapped (e.g. `5.0 > _confidence` → `_confidence < 5.0`).
fn flip_op(op: &BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::Lt => BinaryOp::Gt,
        BinaryOp::Le => BinaryOp::Ge,
        BinaryOp::Gt => BinaryOp::Lt,
        BinaryOp::Ge => BinaryOp::Le,
        other => other.clone(),
    }
}
