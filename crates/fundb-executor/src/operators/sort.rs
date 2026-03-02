//! SortOperator — sorts a RecordBatch by one or more sort expressions.

use std::cmp::Ordering;
use std::collections::HashMap;

use anyhow::Result;
use fundb_core::FunRecord;
use fundb_sql::{Expr, SortExpr};
use serde::Deserialize;

use crate::batch::RecordBatch;

// ---------------------------------------------------------------------------
// DataValue — deserialized MessagePack field value (shared with filter)
// ---------------------------------------------------------------------------

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
// SortOperator
// ---------------------------------------------------------------------------

/// Physical operator that sorts a RecordBatch according to one or more
/// sort expressions (ORDER BY).
pub struct SortOperator {
    pub input: RecordBatch,
    pub order_by: Vec<SortExpr>,
}

impl SortOperator {
    pub fn new(input: RecordBatch, order_by: Vec<SortExpr>) -> Self {
        SortOperator { input, order_by }
    }

    /// Sort the input batch in-place and return it.
    pub async fn execute(self) -> Result<RecordBatch> {
        if self.order_by.is_empty() || self.input.is_empty() {
            return Ok(self.input);
        }

        // Collect indices and sort them using the comparator.
        let n = self.input.len();
        let mut indices: Vec<usize> = (0..n).collect();

        let order_by = &self.order_by;
        let records = &self.input.records;

        indices.sort_by(|&a, &b| compare_records(&records[a], &records[b], order_by));

        // Reorder keys and records according to sorted indices.
        let mut sorted = RecordBatch::new();
        for &i in &indices {
            sorted.push(self.input.keys[i].clone(), self.input.records[i].clone());
        }

        Ok(sorted)
    }
}

// ---------------------------------------------------------------------------
// Comparison logic
// ---------------------------------------------------------------------------

/// Compare two records using a list of sort expressions.
fn compare_records(a: &FunRecord, b: &FunRecord, order_by: &[SortExpr]) -> Ordering {
    for sort_expr in order_by {
        let col_name = match &sort_expr.expr {
            Expr::Column(name) => name.as_str(),
            _ => continue,
        };

        let val_a = extract_sort_value(a, col_name);
        let val_b = extract_sort_value(b, col_name);

        let cmp = compare_sort_values(&val_a, &val_b);
        if cmp != Ordering::Equal {
            return if sort_expr.asc { cmp } else { cmp.reverse() };
        }
    }
    Ordering::Equal
}

/// Extract a sortable value from a record for a given column name.
fn extract_sort_value(record: &FunRecord, col: &str) -> SortValue {
    match col {
        "_confidence" => SortValue::Float(record._confidence as f64),
        "_collection" => SortValue::Str(record._collection.clone()),
        "_tenant" => SortValue::Float(record._tenant as f64),
        _ => {
            // User data field — deserialize from MessagePack.
            let data_map: HashMap<String, DataValue> = match rmp_serde::from_slice(&record.data) {
                Ok(map) => map,
                Err(_) => return SortValue::Null,
            };
            match data_map.get(col) {
                Some(DataValue::Int(i)) => SortValue::Float(*i as f64),
                Some(DataValue::Float(f)) => SortValue::Float(*f),
                Some(DataValue::Str(s)) => SortValue::Str(s.clone()),
                Some(DataValue::Bool(b)) => SortValue::Float(if *b { 1.0 } else { 0.0 }),
                Some(DataValue::Null) | None => SortValue::Null,
            }
        }
    }
}

/// Internal sortable value type.
#[derive(Debug)]
enum SortValue {
    Float(f64),
    Str(String),
    Null,
}

/// Compare two SortValues. NULLs sort last.
fn compare_sort_values(a: &SortValue, b: &SortValue) -> Ordering {
    match (a, b) {
        (SortValue::Null, SortValue::Null) => Ordering::Equal,
        (SortValue::Null, _) => Ordering::Greater, // NULLs last
        (_, SortValue::Null) => Ordering::Less,
        (SortValue::Float(fa), SortValue::Float(fb)) => {
            fa.partial_cmp(fb).unwrap_or(Ordering::Equal)
        }
        (SortValue::Str(sa), SortValue::Str(sb)) => sa.cmp(sb),
        // Mixed types: floats before strings
        (SortValue::Float(_), SortValue::Str(_)) => Ordering::Less,
        (SortValue::Str(_), SortValue::Float(_)) => Ordering::Greater,
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

    fn get_ages(batch: &RecordBatch) -> Vec<i64> {
        batch
            .records
            .iter()
            .map(|r| {
                let map: HashMap<String, DataValue> = rmp_serde::from_slice(&r.data).unwrap();
                match map.get("age") {
                    Some(DataValue::Int(i)) => *i,
                    _ => -1,
                }
            })
            .collect()
    }

    fn get_names(batch: &RecordBatch) -> Vec<String> {
        batch
            .records
            .iter()
            .map(|r| {
                let map: HashMap<String, DataValue> = rmp_serde::from_slice(&r.data).unwrap();
                match map.get("name") {
                    Some(DataValue::Str(s)) => s.clone(),
                    _ => String::new(),
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn test_sort_by_age_asc() {
        let batch = make_batch(vec![
            ("users", "Charlie", 35),
            ("users", "Alice", 20),
            ("users", "Bob", 30),
        ]);

        let order_by = vec![SortExpr {
            expr: Expr::Column("age".to_string()),
            asc: true,
        }];

        let result = SortOperator::new(batch, order_by).execute().await.unwrap();
        assert_eq!(get_ages(&result), vec![20, 30, 35]);
    }

    #[tokio::test]
    async fn test_sort_by_age_desc() {
        let batch = make_batch(vec![
            ("users", "Alice", 20),
            ("users", "Bob", 30),
            ("users", "Charlie", 35),
        ]);

        let order_by = vec![SortExpr {
            expr: Expr::Column("age".to_string()),
            asc: false,
        }];

        let result = SortOperator::new(batch, order_by).execute().await.unwrap();
        assert_eq!(get_ages(&result), vec![35, 30, 20]);
    }

    #[tokio::test]
    async fn test_sort_by_name_asc() {
        let batch = make_batch(vec![
            ("users", "Charlie", 35),
            ("users", "Alice", 20),
            ("users", "Bob", 30),
        ]);

        let order_by = vec![SortExpr {
            expr: Expr::Column("name".to_string()),
            asc: true,
        }];

        let result = SortOperator::new(batch, order_by).execute().await.unwrap();
        assert_eq!(get_names(&result), vec!["Alice", "Bob", "Charlie"]);
    }

    #[tokio::test]
    async fn test_sort_multi_column() {
        let batch = make_batch(vec![
            ("users", "Bob", 30),
            ("users", "Alice", 30),
            ("users", "Alice", 20),
        ]);

        let order_by = vec![
            SortExpr {
                expr: Expr::Column("name".to_string()),
                asc: true,
            },
            SortExpr {
                expr: Expr::Column("age".to_string()),
                asc: true,
            },
        ];

        let result = SortOperator::new(batch, order_by).execute().await.unwrap();
        let names = get_names(&result);
        let ages = get_ages(&result);
        assert_eq!(names, vec!["Alice", "Alice", "Bob"]);
        assert_eq!(ages, vec![20, 30, 30]);
    }

    #[tokio::test]
    async fn test_sort_empty_batch() {
        let batch = RecordBatch::new();
        let order_by = vec![SortExpr {
            expr: Expr::Column("age".to_string()),
            asc: true,
        }];
        let result = SortOperator::new(batch, order_by).execute().await.unwrap();
        assert!(result.is_empty());
    }
}
