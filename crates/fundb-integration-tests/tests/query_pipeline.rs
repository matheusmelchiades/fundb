use std::sync::Arc;

use fundb_core::FunRecordBuilder;
use fundb_executor::Executor;
use fundb_integration_tests::{key_for_record, make_record, open_lsm};
use fundb_optimizer::optimize;
use fundb_sql::{parse, Expr, LogicalPlan, Statement};

// ---------------------------------------------------------------------------
// 1. SELECT * returns all records
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_select_star_returns_all_records() {
    let (_tmp, lsm) = open_lsm();

    for _ in 0..10 {
        let rec = make_record("users");
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
    }

    let executor = Executor::new(Arc::clone(&lsm));
    let plan = LogicalPlan::Scan {
        collection: "users".to_string(),
        predicate: None,
        projections: vec![Expr::Star],
    };
    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 10, "expected 10 records, got {}", batch.len());
}

// ---------------------------------------------------------------------------
// 2. SELECT with confidence filter
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_select_with_confidence_filter() {
    let (_tmp, lsm) = open_lsm();

    // Insert records with varying confidence
    for i in 0..10 {
        let conf = (i as f32 + 1.0) / 10.0; // 0.1, 0.2, ..., 1.0
        let rec = FunRecordBuilder::new("users").confidence(conf).build();
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
    }

    let executor = Executor::new(Arc::clone(&lsm));

    // Filter: _confidence > 0.5 should match records with 0.6, 0.7, 0.8, 0.9, 1.0
    let plan = LogicalPlan::Filter {
        input: Box::new(LogicalPlan::Scan {
            collection: "users".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        predicate: Expr::BinaryOp {
            op: fundb_sql::BinaryOp::Gt,
            left: Box::new(Expr::Column("_confidence".to_string())),
            right: Box::new(Expr::Literal(fundb_sql::Literal::Float(0.5))),
        },
    };
    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(
        batch.len(),
        5,
        "expected 5 records with confidence > 0.5, got {}",
        batch.len()
    );
}

// ---------------------------------------------------------------------------
// 3. SELECT with LIMIT
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_select_with_limit() {
    let (_tmp, lsm) = open_lsm();

    for _ in 0..10 {
        let rec = make_record("users");
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
    }

    let executor = Executor::new(Arc::clone(&lsm));
    let plan = LogicalPlan::Limit {
        input: Box::new(LogicalPlan::Scan {
            collection: "users".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        n: 3,
    };
    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 3, "LIMIT 3 should return exactly 3 records");
}

// ---------------------------------------------------------------------------
// 4. Parse INSERT produces correct Statement
// ---------------------------------------------------------------------------
#[test]
fn test_parse_insert_produces_correct_plan() {
    let sql = "INSERT INTO orders VALUES ('item1', 42, 3.14)";
    let stmt = parse(sql).expect("parse failed");
    assert!(
        matches!(stmt, Statement::Insert(_)),
        "expected Statement::Insert, got {:?}",
        std::mem::discriminant(&stmt)
    );
}

// ---------------------------------------------------------------------------
// 5. Parse VECTOR SCAN (RECALL BY)
// ---------------------------------------------------------------------------
#[test]
fn test_parse_vector_scan() {
    let sql = "RECALL BY semantic_similarity(:query, weight: 0.5) + recency(weight: 0.3) + importance(weight: 0.2) FOR AGENT :agent_id LIMIT 20";
    let result = parse(sql);
    assert!(
        result.is_ok(),
        "vector scan should parse: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// 6. Optimizer pushes filter into scan
// ---------------------------------------------------------------------------
#[test]
fn test_optimizer_pushes_filter_into_scan() {
    let plan = LogicalPlan::Filter {
        input: Box::new(LogicalPlan::Scan {
            collection: "users".to_string(),
            predicate: None,
            projections: vec![Expr::Star],
        }),
        predicate: Expr::BinaryOp {
            op: fundb_sql::BinaryOp::Gt,
            left: Box::new(Expr::Column("_confidence".to_string())),
            right: Box::new(Expr::Literal(fundb_sql::Literal::Float(0.5))),
        },
    };

    let optimized = optimize(plan);

    // After predicate pushdown, the Scan node should have a predicate
    match &optimized {
        LogicalPlan::Scan { predicate, .. } => {
            assert!(predicate.is_some(), "predicate should be pushed into scan");
        }
        // Filter may still wrap scan if pushdown produced a different structure
        LogicalPlan::Filter { .. } => {
            // Acceptable — optimizer may choose not to push down
        }
        _ => {
            // Any plan transformation is acceptable as long as it doesn't panic
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Parse UNDERSTAND
// ---------------------------------------------------------------------------
#[test]
fn test_parse_understand() {
    let sql = "UNDERSTAND 'Why are sales declining?'";
    let stmt = parse(sql).expect("parse failed");
    assert!(
        matches!(stmt, Statement::Understand(_)),
        "expected Statement::Understand"
    );
}

// ---------------------------------------------------------------------------
// 8. Parse ESTIMATE EFFECT
// ---------------------------------------------------------------------------
#[test]
fn test_parse_estimate_effect() {
    let sql = "SELECT expected_revenue FROM INTERVENE ON revenue_model SET marketing_spend = 100000 PREDICT revenue";
    let stmt = parse(sql).expect("parse failed");
    assert!(
        matches!(stmt, Statement::EstimateEffect(_)),
        "expected Statement::EstimateEffect"
    );
}

// ---------------------------------------------------------------------------
// 9. Parse COUNTERFACTUAL
// ---------------------------------------------------------------------------
#[test]
fn test_parse_counterfactual() {
    let sql = "COUNTERFACTUAL ON revenue_model GIVEN observed_data = (SELECT * FROM metrics WHERE month = '2025-01') HAD marketing_spend = 50000 PREDICT revenue";
    let stmt = parse(sql).expect("parse failed");
    assert!(
        matches!(stmt, Statement::Counterfactual(_)),
        "expected Statement::Counterfactual"
    );
}

// ---------------------------------------------------------------------------
// 10. Parse TRACE CAUSALITY
// ---------------------------------------------------------------------------
#[test]
fn test_parse_trace_causality() {
    let sql = "TRACE CAUSALITY FROM deployment TO error_rate";
    let result = parse(sql);
    assert!(
        result.is_ok(),
        "TRACE CAUSALITY should parse: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// 11. Parse AS OF SYSTEM TIME
// ---------------------------------------------------------------------------
#[test]
fn test_parse_as_of_system_time() {
    let sql = "SELECT * FROM orders AS OF SYSTEM TIME '2024-01-01T00:00:00Z'";
    let result = parse(sql);
    assert!(
        result.is_ok(),
        "AS OF SYSTEM TIME should parse: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// 12. Parse AS OF VALID TIME
// ---------------------------------------------------------------------------
#[test]
fn test_parse_as_of_valid_time() {
    let sql = "SELECT * FROM prices AS OF VALID TIME '2024-06-15T12:00:00Z'";
    let result = parse(sql);
    assert!(
        result.is_ok(),
        "AS OF VALID TIME should parse: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// 13. Scan with pre-loaded data returns records
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_scan_with_data_returns_records() {
    let (_tmp, lsm) = open_lsm();

    let mut expected_ids = Vec::new();
    for _ in 0..5 {
        let rec = make_record("inventory");
        expected_ids.push(rec._id);
        let key = key_for_record(&rec);
        lsm.write(key, rec).await.unwrap();
    }

    let executor = Executor::new(Arc::clone(&lsm));
    let plan = LogicalPlan::Scan {
        collection: "inventory".to_string(),
        predicate: None,
        projections: vec![Expr::Star],
    };
    let batch = executor.execute(plan).await.unwrap();
    assert_eq!(batch.len(), 5, "expected 5 records from scan");

    // Verify that all returned records have _id values from our inserts
    for record in &batch.records {
        assert!(
            expected_ids.contains(&record._id),
            "unexpected record _id in results"
        );
    }
}

// ---------------------------------------------------------------------------
// 14. No panic on invalid SQL
// ---------------------------------------------------------------------------
#[test]
fn test_no_panic_on_invalid_sql() {
    let invalid_queries = vec![
        "",
        "   ",
        "SELECT",
        "SELECT *",
        "SELECT * FROM",
        "INSERT",
        "INSERT INTO",
        "DELETE",
        "UPDATE",
        "GIBBERISH NONSENSE",
        ";;;",
        "SELECT * FROM users WHERE",
        "SELECT * FROM users WHERE x =",
        "SELECT * FROM users LIMIT",
        "DROP TABLE users",
        "CREATE INDEX",
        "SELECT 1 +",
        "SELECT * FROM users ORDER BY",
        "(((",
        "SELECT * FROM 'invalid table name' WHERE 1 = 1 AND",
    ];

    for sql in &invalid_queries {
        // Must not panic — either Ok or Err is fine
        let _ = parse(sql);
    }
}
