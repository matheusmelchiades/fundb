//! FilterOperator — evaluates a scalar predicate row-by-row.

use std::collections::HashMap;

use anyhow::Result;
use fundb_core::FunRecord;
use fundb_sql::{BinaryOp, Expr, Literal, UnaryOp};
use serde::Deserialize;

use crate::batch::RecordBatch;

// ---------------------------------------------------------------------------
// DataValue — deserialized MessagePack field value
// ---------------------------------------------------------------------------

/// A value extracted from a record's MessagePack `data` payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum DataValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Null,
}

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
        for (key, record) in self.input.keys.iter().zip(self.input.records.iter()) {
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
            // Binary comparison (delegates to eval_binary)
            // ----------------------------------------------------------------
            Expr::BinaryOp { op, left, right } => self.eval_binary(op, left, right, record),

            // ----------------------------------------------------------------
            // Everything else → pass-through (return true)
            // ----------------------------------------------------------------
            _ => true,
        }
    }

    /// Evaluate a binary comparison expression against a record.
    ///
    /// Handles built-in fields (`_confidence`, `_collection`, `_tenant`) and
    /// user data fields stored as MessagePack in `FunRecord.data`.
    fn eval_binary(&self, op: &BinaryOp, left: &Expr, right: &Expr, record: &FunRecord) -> bool {
        // ── Numeric comparisons on _confidence ────────────────────────────
        if let (Expr::Column(col), Some(rhs_f)) = (left, extract_float(right)) {
            if col == "_confidence" {
                return compare_f32(record._confidence, op, rhs_f as f32);
            }
        }
        if let (Some(lhs_f), Expr::Column(col)) = (extract_float(left), right) {
            if col == "_confidence" {
                return compare_f32(lhs_f as f32, &flip_op(op), record._confidence);
            }
        }

        // ── Column <op> Literal — resolve via built-in or user data ──────
        if let (Expr::Column(col), lit_expr) = (left, right) {
            return self.eval_column_vs_literal(col, op, lit_expr, record);
        }
        // Swapped: Literal <op> Column → flip operator
        if let (lit_expr, Expr::Column(col)) = (left, right) {
            return self.eval_column_vs_literal(col, &flip_op(op), lit_expr, record);
        }

        // Unsupported → pass-through
        true
    }

    /// Compare a column value (built-in or user data) against a literal expression.
    fn eval_column_vs_literal(
        &self,
        col: &str,
        op: &BinaryOp,
        lit_expr: &Expr,
        record: &FunRecord,
    ) -> bool {
        // ── Built-in fields ──────────────────────────────────────────────
        match col {
            "_collection" => {
                if let Expr::Literal(Literal::String(s)) = lit_expr {
                    return compare_str(&record._collection, op, s);
                }
                return true;
            }
            "_tenant" => {
                if let Some(rhs) = extract_float(lit_expr) {
                    return compare_f64(record._tenant as f64, op, rhs);
                }
                return true;
            }
            "_confidence" => {
                // Already handled above, but catch any remaining patterns.
                if let Some(rhs) = extract_float(lit_expr) {
                    return compare_f32(record._confidence, op, rhs as f32);
                }
                return true;
            }
            _ => {}
        }

        // ── User data fields (MessagePack) ───────────────────────────────
        let data_map: HashMap<String, DataValue> = match rmp_serde::from_slice(&record.data) {
            Ok(map) => map,
            Err(_) => return true, // can't deserialize → pass-through
        };

        let Some(field_val) = data_map.get(col) else {
            // Field doesn't exist in this record → treat as NULL → false
            return false;
        };

        match (field_val, lit_expr) {
            // Numeric field vs numeric literal
            (DataValue::Int(v), _) if extract_float(lit_expr).is_some() => {
                compare_f64(*v as f64, op, extract_float(lit_expr).unwrap())
            }
            (DataValue::Float(v), _) if extract_float(lit_expr).is_some() => {
                compare_f64(*v, op, extract_float(lit_expr).unwrap())
            }
            // String field vs string literal
            (DataValue::Str(v), Expr::Literal(Literal::String(s))) => compare_str(v, op, s),
            // Bool field vs bool literal
            (DataValue::Bool(v), Expr::Literal(Literal::Bool(b))) => match op {
                BinaryOp::Eq => v == b,
                BinaryOp::Ne => v != b,
                _ => true,
            },
            // NULL field → false for any comparison
            (DataValue::Null, _) => false,
            // Type mismatch → false
            _ => false,
        }
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
fn compare_f32(lhs: f32, op: &BinaryOp, rhs: f32) -> bool {
    match op {
        BinaryOp::Eq => (lhs - rhs).abs() < f32::EPSILON,
        BinaryOp::Ne => (lhs - rhs).abs() >= f32::EPSILON,
        BinaryOp::Lt => lhs < rhs,
        BinaryOp::Le => lhs <= rhs,
        BinaryOp::Gt => lhs > rhs,
        BinaryOp::Ge => lhs >= rhs,
        _ => true,
    }
}

/// Compare two `f64` values using the given binary operator.
fn compare_f64(lhs: f64, op: &BinaryOp, rhs: f64) -> bool {
    match op {
        BinaryOp::Eq => (lhs - rhs).abs() < f64::EPSILON,
        BinaryOp::Ne => (lhs - rhs).abs() >= f64::EPSILON,
        BinaryOp::Lt => lhs < rhs,
        BinaryOp::Le => lhs <= rhs,
        BinaryOp::Gt => lhs > rhs,
        BinaryOp::Ge => lhs >= rhs,
        _ => true,
    }
}

/// Compare two strings using the given binary operator.
fn compare_str(lhs: &str, op: &BinaryOp, rhs: &str) -> bool {
    match op {
        BinaryOp::Eq => lhs == rhs,
        BinaryOp::Ne => lhs != rhs,
        BinaryOp::Lt => lhs < rhs,
        BinaryOp::Le => lhs <= rhs,
        BinaryOp::Gt => lhs > rhs,
        BinaryOp::Ge => lhs >= rhs,
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_core::{FunRecordBuilder, RecordKey};
    use serde::Serialize;

    /// Serialize user data the same way the executor does: as a HashMap.
    fn make_record(collection: &str, name: &str, age: i64) -> (RecordKey, FunRecord) {
        #[derive(Serialize)]
        #[serde(untagged)]
        enum MsgVal {
            Str(String),
            Int(i64),
        }
        let mut data_map = HashMap::new();
        data_map.insert("name".to_string(), MsgVal::Str(name.to_string()));
        data_map.insert("age".to_string(), MsgVal::Int(age));
        let data_bytes = rmp_serde::to_vec(&data_map).unwrap();
        let record = FunRecordBuilder::new(collection).data(data_bytes).build();
        let key = RecordKey {
            collection: collection.to_string(),
            id: *record._id.as_bytes(),
        };
        (key, record)
    }

    fn make_batch(records: Vec<(&str, &str, i64)>) -> RecordBatch {
        let mut batch = RecordBatch::new();
        for (col, name, age) in records {
            let (key, rec) = make_record(col, name, age);
            batch.push(key, rec);
        }
        batch
    }

    #[tokio::test]
    async fn test_filter_where_age_gt_25() {
        let batch = make_batch(vec![
            ("users", "Alice", 30),
            ("users", "Bob", 20),
            ("users", "Charlie", 35),
        ]);

        let predicate = Expr::BinaryOp {
            op: BinaryOp::Gt,
            left: Box::new(Expr::Column("age".to_string())),
            right: Box::new(Expr::Literal(Literal::Int(25))),
        };

        let result = FilterOperator::new(batch, predicate)
            .execute()
            .await
            .unwrap();
        assert_eq!(
            result.len(),
            2,
            "only Alice(30) and Charlie(35) match age > 25"
        );
    }

    #[tokio::test]
    async fn test_filter_where_name_eq_alice() {
        let batch = make_batch(vec![("users", "Alice", 30), ("users", "Bob", 20)]);

        let predicate = Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Column("name".to_string())),
            right: Box::new(Expr::Literal(Literal::String("Alice".to_string()))),
        };

        let result = FilterOperator::new(batch, predicate)
            .execute()
            .await
            .unwrap();
        assert_eq!(result.len(), 1, "only Alice matches name = 'Alice'");
    }

    #[tokio::test]
    async fn test_filter_where_age_le_20() {
        let batch = make_batch(vec![
            ("users", "Alice", 30),
            ("users", "Bob", 20),
            ("users", "Charlie", 15),
        ]);

        let predicate = Expr::BinaryOp {
            op: BinaryOp::Le,
            left: Box::new(Expr::Column("age".to_string())),
            right: Box::new(Expr::Literal(Literal::Int(20))),
        };

        let result = FilterOperator::new(batch, predicate)
            .execute()
            .await
            .unwrap();
        assert_eq!(result.len(), 2, "Bob(20) and Charlie(15) match age <= 20");
    }

    #[tokio::test]
    async fn test_filter_where_missing_field() {
        let batch = make_batch(vec![("users", "Alice", 30)]);

        let predicate = Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Column("nonexistent".to_string())),
            right: Box::new(Expr::Literal(Literal::String("x".to_string()))),
        };

        let result = FilterOperator::new(batch, predicate)
            .execute()
            .await
            .unwrap();
        assert_eq!(result.len(), 0, "missing field should not match");
    }

    #[tokio::test]
    async fn test_filter_builtin_confidence() {
        let mut batch = RecordBatch::new();
        let rec = FunRecordBuilder::new("test").confidence(0.9).build();
        let key = RecordKey {
            collection: "test".to_string(),
            id: *rec._id.as_bytes(),
        };
        batch.push(key, rec);

        let predicate = Expr::BinaryOp {
            op: BinaryOp::Gt,
            left: Box::new(Expr::Column("_confidence".to_string())),
            right: Box::new(Expr::Literal(Literal::Float(0.5))),
        };

        let result = FilterOperator::new(batch, predicate)
            .execute()
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_filter_and_compound() {
        let batch = make_batch(vec![
            ("users", "Alice", 30),
            ("users", "Bob", 20),
            ("users", "Charlie", 35),
        ]);

        // age > 25 AND name != 'Charlie'
        let predicate = Expr::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(Expr::BinaryOp {
                op: BinaryOp::Gt,
                left: Box::new(Expr::Column("age".to_string())),
                right: Box::new(Expr::Literal(Literal::Int(25))),
            }),
            right: Box::new(Expr::BinaryOp {
                op: BinaryOp::Ne,
                left: Box::new(Expr::Column("name".to_string())),
                right: Box::new(Expr::Literal(Literal::String("Charlie".to_string()))),
            }),
        };

        let result = FilterOperator::new(batch, predicate)
            .execute()
            .await
            .unwrap();
        assert_eq!(
            result.len(),
            1,
            "only Alice matches age > 25 AND name != Charlie"
        );
    }
}
