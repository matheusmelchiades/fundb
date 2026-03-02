//! Integration tests for WHERE filtering and ORDER BY on user data fields.
//!
//! These tests exercise the full pipeline: INSERT data with MessagePack-encoded
//! user fields → SELECT with WHERE / ORDER BY → verify correct results.

use std::collections::HashMap;
use std::sync::Arc;

use fundb_executor::Executor;
use fundb_integration_tests::open_lsm;
use fundb_sql::{BinaryOp, Expr, Literal, LogicalPlan, SortExpr};

async fn insert_person(executor: &Executor, collection: &str, name: &str, age: i64) {
    let plan = LogicalPlan::Insert {
        collection: collection.to_string(),
        columns: vec!["name".to_string(), "age".to_string()],
        values: vec![vec![
            Expr::Literal(Literal::String(name.to_string())),
            Expr::Literal(Literal::Int(age)),
        ]],
    };
    executor.execute(plan).await.unwrap();
}

async fn insert_people(executor: &Executor, collection: &str) {
    insert_person(executor, collection, "Alice", 30).await;
    insert_person(executor, collection, "Bob", 20).await;
    insert_person(executor, collection, "Charlie", 35).await;
    insert_person(executor, collection, "Diana", 25).await;
    insert_person(executor, collection, "Eve", 40).await;
}

fn get_data_field(record: &fundb_core::FunRecord, field: &str) -> Option<String> {
    let map: HashMap<String, serde_json::Value> = rmp_serde::from_slice(&record.data).ok()?;
    map.get(field).map(|v| match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        other => format!("{}", other),
    })
}

// ---------------------------------------------------------------------------
// WHERE tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_insert_then_where_age_gt_25() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    let plan = LogicalPlan::Filter {
        input: Box::new(LogicalPlan::Scan {
            collection: "people".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        predicate: Expr::BinaryOp {
            op: BinaryOp::Gt,
            left: Box::new(Expr::Column("age".to_string())),
            right: Box::new(Expr::Literal(Literal::Int(25))),
        },
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(
        batch.len(),
        3,
        "expected Alice(30), Charlie(35), Eve(40); got {}",
        batch.len()
    );
}

#[tokio::test]
async fn test_insert_then_where_name_eq_alice() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    let plan = LogicalPlan::Filter {
        input: Box::new(LogicalPlan::Scan {
            collection: "people".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        predicate: Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Column("name".to_string())),
            right: Box::new(Expr::Literal(Literal::String("Alice".to_string()))),
        },
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 1, "only Alice should match");

    let name = get_data_field(&batch.records[0], "name").unwrap();
    assert_eq!(name, "Alice");
}

#[tokio::test]
async fn test_insert_then_where_age_le_25() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    let plan = LogicalPlan::Filter {
        input: Box::new(LogicalPlan::Scan {
            collection: "people".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        predicate: Expr::BinaryOp {
            op: BinaryOp::Le,
            left: Box::new(Expr::Column("age".to_string())),
            right: Box::new(Expr::Literal(Literal::Int(25))),
        },
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(
        batch.len(),
        2,
        "expected Bob(20) and Diana(25); got {}",
        batch.len()
    );
}

#[tokio::test]
async fn test_where_nonexistent_field_returns_empty() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    let plan = LogicalPlan::Filter {
        input: Box::new(LogicalPlan::Scan {
            collection: "people".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        predicate: Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Column("email".to_string())),
            right: Box::new(Expr::Literal(Literal::String("x@x.com".to_string()))),
        },
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 0, "nonexistent field should match nothing");
}

// ---------------------------------------------------------------------------
// ORDER BY tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_insert_then_order_by_age_asc() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    let plan = LogicalPlan::Sort {
        input: Box::new(LogicalPlan::Scan {
            collection: "people".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        order_by: vec![SortExpr {
            expr: Expr::Column("age".to_string()),
            asc: true,
        }],
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 5);

    let ages: Vec<String> = batch
        .records
        .iter()
        .map(|r| get_data_field(r, "age").unwrap())
        .collect();
    assert_eq!(ages, vec!["20", "25", "30", "35", "40"]);
}

#[tokio::test]
async fn test_insert_then_order_by_age_desc() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    let plan = LogicalPlan::Sort {
        input: Box::new(LogicalPlan::Scan {
            collection: "people".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        order_by: vec![SortExpr {
            expr: Expr::Column("age".to_string()),
            asc: false,
        }],
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 5);

    let ages: Vec<String> = batch
        .records
        .iter()
        .map(|r| get_data_field(r, "age").unwrap())
        .collect();
    assert_eq!(ages, vec!["40", "35", "30", "25", "20"]);
}

#[tokio::test]
async fn test_insert_then_order_by_name_asc() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    let plan = LogicalPlan::Sort {
        input: Box::new(LogicalPlan::Scan {
            collection: "people".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        order_by: vec![SortExpr {
            expr: Expr::Column("name".to_string()),
            asc: true,
        }],
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 5);

    let names: Vec<String> = batch
        .records
        .iter()
        .map(|r| get_data_field(r, "name").unwrap())
        .collect();
    assert_eq!(names, vec!["Alice", "Bob", "Charlie", "Diana", "Eve"]);
}

// ---------------------------------------------------------------------------
// Combined WHERE + ORDER BY + LIMIT
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_where_and_order_by_combined() {
    let (_tmp, lsm) = open_lsm();
    let executor = Executor::new(Arc::clone(&lsm));

    insert_people(&executor, "people").await;

    // WHERE age > 25 ORDER BY age DESC LIMIT 2
    let plan = LogicalPlan::Limit {
        n: 2,
        input: Box::new(LogicalPlan::Sort {
            order_by: vec![SortExpr {
                expr: Expr::Column("age".to_string()),
                asc: false,
            }],
            input: Box::new(LogicalPlan::Filter {
                predicate: Expr::BinaryOp {
                    op: BinaryOp::Gt,
                    left: Box::new(Expr::Column("age".to_string())),
                    right: Box::new(Expr::Literal(Literal::Int(25))),
                },
                input: Box::new(LogicalPlan::Scan {
                    collection: "people".to_string(),
                    predicate: None,
                    projections: vec![Expr::Star],
                }),
            }),
        }),
    };

    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 2, "LIMIT 2 should return 2 records");

    let ages: Vec<String> = batch
        .records
        .iter()
        .map(|r| get_data_field(r, "age").unwrap())
        .collect();
    // age > 25 → [30, 35, 40], ORDER BY DESC → [40, 35, 30], LIMIT 2 → [40, 35]
    assert_eq!(ages, vec!["40", "35"]);
}
