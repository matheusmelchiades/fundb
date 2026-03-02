use fundb_sql::{BinaryOp, Expr, Literal, LogicalPlan};
/// Cost-based optimizer for FunDB.
///
/// Translates a `LogicalPlan` into a `PhysicalPlan` by applying cost estimates
/// derived from table statistics (row counts, histograms, index sizes).
use std::collections::HashMap;

// ── Statistics ─────────────────────────────────────────────────────────────────

/// A histogram over a single column.
///
/// The value range [min_val, max_val] is divided into 100 equal-width buckets.
/// Each bucket holds the count of rows whose value falls in that bucket.
pub struct Histogram {
    /// 100 buckets; bucket[i] = row count for the i-th percentile of the value range.
    pub buckets: Vec<u64>,
    pub min_val: f64,
    pub max_val: f64,
}

/// Runtime statistics used by the cost optimizer.
pub struct Statistics {
    /// Estimated row count per collection.
    pub collection_row_count: HashMap<String, u64>,
    /// Per-(collection, column) value histograms.
    pub column_histograms: HashMap<(String, String), Histogram>,
    /// Per-collection confidence histogram (100 buckets over [0.0, 1.0]).
    pub confidence_histograms: HashMap<String, [u32; 100]>,
    /// Index sizes (number of indexed entries) keyed by index name.
    pub index_sizes: HashMap<String, u64>,
}

// ── PhysicalPlan ───────────────────────────────────────────────────────────────

/// A physical execution plan produced by cost-based optimization.
#[derive(Debug, Clone)]
pub enum PhysicalPlan {
    /// Full table scan — always works, O(n).
    SeqScan {
        collection: String,
        estimated_rows: u64,
        estimated_cost: f64,
    },
    /// Index-based scan — faster for selective queries, skip for small tables.
    IndexScan {
        collection: String,
        index_field: String,
        estimated_rows: u64,
        estimated_cost: f64,
    },
    /// ANN vector scan.
    VectorScan {
        collection: String,
        vector_field: String,
        query: Vec<f32>,
        threshold: f32,
        estimated_rows: u64,
    },
    /// Causal trace scan.
    CausalScan {
        from: String,
        to: String,
        max_depth: u32,
        min_strength: f32,
    },
    /// Filter applied on top of another plan.
    Filter {
        input: Box<PhysicalPlan>,
        selectivity: f64,
        estimated_rows: u64,
    },
    /// Row limit.
    Limit { input: Box<PhysicalPlan>, n: usize },
    /// Aggregate.
    Aggregate {
        input: Box<PhysicalPlan>,
        group_count: u64,
    },
    /// Sort.
    Sort {
        input: Box<PhysicalPlan>,
        estimated_cost: f64,
    },
    /// Empty / no-op.
    Empty,
}

impl PhysicalPlan {
    /// Returns the estimated number of output rows for this plan node.
    pub fn estimated_rows(&self) -> u64 {
        match self {
            PhysicalPlan::SeqScan { estimated_rows, .. } => *estimated_rows,
            PhysicalPlan::IndexScan { estimated_rows, .. } => *estimated_rows,
            PhysicalPlan::VectorScan { estimated_rows, .. } => *estimated_rows,
            PhysicalPlan::Filter { estimated_rows, .. } => *estimated_rows,
            PhysicalPlan::Limit { n, .. } => *n as u64,
            PhysicalPlan::Aggregate { .. } => 1,
            PhysicalPlan::Sort { input, .. } => input.estimated_rows(),
            _ => 0,
        }
    }

    /// Returns the estimated execution cost for this plan node.
    pub fn estimated_cost(&self) -> f64 {
        match self {
            PhysicalPlan::SeqScan { estimated_cost, .. } => *estimated_cost,
            PhysicalPlan::IndexScan { estimated_cost, .. } => *estimated_cost,
            PhysicalPlan::Sort { estimated_cost, .. } => *estimated_cost,
            _ => 0.0,
        }
    }

    /// Returns true if this plan uses an index.
    pub fn uses_index(&self) -> bool {
        matches!(self, PhysicalPlan::IndexScan { .. })
    }
}

// ── Cost functions ─────────────────────────────────────────────────────────────

/// Cost of a sequential scan: 1 ms per 1000 rows.
fn scan_cost(row_count: u64) -> f64 {
    row_count as f64 * 0.001
}

/// Cost of an index scan: log2(N) * 0.01 for the B-tree traversal plus
/// 0.001 per selected row.
fn index_scan_cost(row_count: u64, selectivity: f64) -> f64 {
    let selected = (row_count as f64 * selectivity).ceil() as u64;
    (row_count as f64).log2().max(1.0) * 0.01 + selected as f64 * 0.001
}

// ── Selectivity estimation ─────────────────────────────────────────────────────

/// Estimate the fraction of rows in `collection` that satisfy `predicate`,
/// using available statistics.
fn estimate_selectivity(predicate: &Expr, stats: &Statistics, collection: &str) -> f64 {
    match predicate {
        // Equality predicate on a regular column: use the value histogram.
        Expr::BinaryOp {
            op: BinaryOp::Eq,
            left,
            right,
        } => {
            if let (Expr::Column(col), Expr::Literal(Literal::Int(v))) =
                (left.as_ref(), right.as_ref())
            {
                let key = (collection.to_string(), col.clone());
                if let Some(hist) = stats.column_histograms.get(&key) {
                    // Estimate the fraction by locating the bucket the value
                    // falls in and dividing its count by the total row count.
                    let total: u64 = hist.buckets.iter().sum();
                    if total == 0 {
                        return 0.1;
                    }
                    if hist.max_val <= hist.min_val {
                        return 1.0 / hist.buckets.len().max(1) as f64;
                    }
                    let range = hist.max_val - hist.min_val;
                    let bucket_width = range / hist.buckets.len() as f64;
                    let idx = ((*v as f64 - hist.min_val) / bucket_width).floor() as usize;
                    let idx = idx.min(hist.buckets.len().saturating_sub(1));
                    return hist.buckets[idx] as f64 / total as f64;
                }
                return 0.1;
            }
            // Equality on a float literal column
            if let (Expr::Column(col), Expr::Literal(Literal::Float(v))) =
                (left.as_ref(), right.as_ref())
            {
                let key = (collection.to_string(), col.clone());
                if let Some(hist) = stats.column_histograms.get(&key) {
                    let total: u64 = hist.buckets.iter().sum();
                    if total == 0 {
                        return 0.1;
                    }
                    if hist.max_val <= hist.min_val {
                        return 1.0 / hist.buckets.len().max(1) as f64;
                    }
                    let range = hist.max_val - hist.min_val;
                    let bucket_width = range / hist.buckets.len() as f64;
                    let idx = ((*v - hist.min_val) / bucket_width).floor() as usize;
                    let idx = idx.min(hist.buckets.len().saturating_sub(1));
                    return hist.buckets[idx] as f64 / total as f64;
                }
                return 0.1;
            }
            // Default: no histogram available.
            0.1
        }

        // Comparison on _confidence: use the confidence histogram.
        Expr::BinaryOp {
            op: op @ (BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge),
            left,
            right,
        } => {
            if let Expr::Column(col) = left.as_ref() {
                if col == "_confidence" {
                    let threshold = match right.as_ref() {
                        Expr::Literal(Literal::Float(f)) => *f,
                        Expr::Literal(Literal::Int(i)) => *i as f64,
                        _ => return 0.5,
                    };
                    if let Some(conf_hist) = stats.confidence_histograms.get(collection) {
                        let total: u32 = conf_hist.iter().sum();
                        if total == 0 {
                            return threshold.clamp(0.0, 1.0);
                        }
                        // Each bucket covers 1% of the [0.0, 1.0] range.
                        // bucket[i] covers [i/100, (i+1)/100).
                        let cutoff_bucket = (threshold * 100.0).floor() as usize;
                        match op {
                            BinaryOp::Lt | BinaryOp::Le => {
                                // Fraction of rows with confidence < threshold
                                let limit = if matches!(op, BinaryOp::Le) {
                                    (cutoff_bucket + 1).min(100)
                                } else {
                                    cutoff_bucket.min(100)
                                };
                                let below: u32 = conf_hist[..limit].iter().sum();
                                return below as f64 / total as f64;
                            }
                            BinaryOp::Gt | BinaryOp::Ge => {
                                // Fraction of rows with confidence > threshold
                                let start = if matches!(op, BinaryOp::Ge) {
                                    cutoff_bucket.min(100)
                                } else {
                                    (cutoff_bucket + 1).min(100)
                                };
                                let above: u32 = conf_hist[start..].iter().sum();
                                return above as f64 / total as f64;
                            }
                            _ => unreachable!(),
                        }
                    }
                    // No histogram: linear assumption.
                    return threshold.clamp(0.0, 1.0);
                }
            }
            0.5
        }

        // Conjunction: independent selectivity multiplication.
        Expr::BinaryOp {
            op: BinaryOp::And,
            left,
            right,
        } => {
            estimate_selectivity(left, stats, collection)
                * estimate_selectivity(right, stats, collection)
        }

        // Disjunction: inclusion–exclusion (capped at 1.0).
        Expr::BinaryOp {
            op: BinaryOp::Or,
            left,
            right,
        } => {
            let s_left = estimate_selectivity(left, stats, collection);
            let s_right = estimate_selectivity(right, stats, collection);
            (s_left + s_right).min(1.0)
        }

        // Constant booleans.
        Expr::Literal(Literal::Bool(true)) => 1.0,
        Expr::Literal(Literal::Bool(false)) => 0.0,

        // Everything else: 50% selectivity.
        _ => 0.5,
    }
}

// ── Collection name helper ─────────────────────────────────────────────────────

/// Return the collection name of the innermost scan node in `plan`, if any.
fn collection_of(plan: &LogicalPlan) -> &str {
    match plan {
        LogicalPlan::Scan { collection, .. } => collection.as_str(),
        LogicalPlan::VectorScan { collection, .. } => collection.as_str(),
        LogicalPlan::Filter { input, .. } => collection_of(input),
        LogicalPlan::Project { input, .. } => collection_of(input),
        LogicalPlan::Limit { input, .. } => collection_of(input),
        LogicalPlan::Sort { input, .. } => collection_of(input),
        LogicalPlan::Aggregate { input, .. } => collection_of(input),
        LogicalPlan::ContextOptimize { input, .. } => collection_of(input),
        _ => "",
    }
}

/// Extract the column name referenced by a simple predicate of the form
/// `Column(name) op Literal(...)`.  Returns `"_id"` as a fallback.
fn predicate_index_field(predicate: &Expr) -> String {
    match predicate {
        Expr::BinaryOp { left, .. } => match left.as_ref() {
            Expr::Column(col) => col.clone(),
            _ => "_id".to_string(),
        },
        _ => "_id".to_string(),
    }
}

// ── CostOptimizer ──────────────────────────────────────────────────────────────

/// Entry point for cost-based query optimization.
pub struct CostOptimizer;

impl CostOptimizer {
    /// Translate a `LogicalPlan` into a `PhysicalPlan` using cost estimates
    /// derived from `stats`.
    pub fn optimize(plan: LogicalPlan, stats: &Statistics) -> PhysicalPlan {
        match plan {
            // ── Scan ───────────────────────────────────────────────────────────
            LogicalPlan::Scan {
                collection,
                predicate,
                ..
            } => {
                let row_count = stats
                    .collection_row_count
                    .get(&collection)
                    .copied()
                    .unwrap_or(10_000);

                if row_count < 1_000 {
                    // Small table: sequential scan is never worth the index overhead.
                    PhysicalPlan::SeqScan {
                        estimated_cost: scan_cost(row_count),
                        estimated_rows: row_count,
                        collection,
                    }
                } else if let Some(pred) = predicate {
                    let selectivity = estimate_selectivity(&pred, stats, &collection);
                    if selectivity < 0.01 {
                        // Very selective: index scan wins.
                        let index_field = predicate_index_field(&pred);
                        let selected = (row_count as f64 * selectivity).ceil() as u64;
                        PhysicalPlan::IndexScan {
                            estimated_cost: index_scan_cost(row_count, selectivity),
                            estimated_rows: selected,
                            collection,
                            index_field,
                        }
                    } else {
                        // Moderate or high selectivity: sequential scan is cheaper.
                        PhysicalPlan::SeqScan {
                            estimated_cost: scan_cost(row_count),
                            estimated_rows: row_count,
                            collection,
                        }
                    }
                } else {
                    // No predicate: always sequential.
                    PhysicalPlan::SeqScan {
                        estimated_cost: scan_cost(row_count),
                        estimated_rows: row_count,
                        collection,
                    }
                }
            }

            // ── VectorScan ─────────────────────────────────────────────────────
            LogicalPlan::VectorScan {
                collection,
                vector_field,
                query,
                threshold,
            } => {
                let row_count = stats
                    .collection_row_count
                    .get(&collection)
                    .copied()
                    .unwrap_or(10_000);
                let estimated_rows = (row_count as f64 * threshold as f64 * 0.1) as u64;
                PhysicalPlan::VectorScan {
                    collection,
                    vector_field,
                    query,
                    threshold,
                    estimated_rows,
                }
            }

            // ── Filter ─────────────────────────────────────────────────────────
            LogicalPlan::Filter { input, predicate } => {
                let collection = collection_of(&input).to_string();
                let selectivity = estimate_selectivity(&predicate, stats, &collection);
                let physical_input = CostOptimizer::optimize(*input, stats);
                let input_rows = physical_input.estimated_rows();
                let estimated_rows = (input_rows as f64 * selectivity) as u64;
                PhysicalPlan::Filter {
                    input: Box::new(physical_input),
                    selectivity,
                    estimated_rows,
                }
            }

            // ── Limit ──────────────────────────────────────────────────────────
            LogicalPlan::Limit { input, n } => PhysicalPlan::Limit {
                input: Box::new(CostOptimizer::optimize(*input, stats)),
                n,
            },

            // ── Sort ───────────────────────────────────────────────────────────
            LogicalPlan::Sort { input, .. } => {
                let physical_input = CostOptimizer::optimize(*input, stats);
                let rows = physical_input.estimated_rows();
                PhysicalPlan::Sort {
                    estimated_cost: scan_cost(rows) * 10.0,
                    input: Box::new(physical_input),
                }
            }

            // ── Aggregate ──────────────────────────────────────────────────────
            LogicalPlan::Aggregate { input, .. } => PhysicalPlan::Aggregate {
                input: Box::new(CostOptimizer::optimize(*input, stats)),
                group_count: 1,
            },

            // ── CausalTrace ───────────────────────────────────────────────────
            LogicalPlan::CausalTrace {
                from,
                to,
                max_depth,
                min_strength,
                ..
            } => PhysicalPlan::CausalScan {
                from: format!("{:?}", from),
                to: format!("{:?}", to),
                max_depth,
                min_strength,
            },

            // ── Empty and everything else ─────────────────────────────────────
            LogicalPlan::Empty => PhysicalPlan::Empty,
            _ => PhysicalPlan::Empty,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_sql::{BinaryOp, Expr, Literal, LogicalPlan};

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Build an empty Statistics struct with the given row count for one collection.
    fn stats_with_rows(collection: &str, rows: u64) -> Statistics {
        let mut m = HashMap::new();
        m.insert(collection.to_string(), rows);
        Statistics {
            collection_row_count: m,
            column_histograms: HashMap::new(),
            confidence_histograms: HashMap::new(),
            index_sizes: HashMap::new(),
        }
    }

    /// Build a Scan with an equality predicate: `col = value`.
    fn scan_with_eq_pred(collection: &str, col: &str, value: i64) -> LogicalPlan {
        LogicalPlan::Scan {
            collection: collection.to_string(),
            predicate: Some(Expr::BinaryOp {
                op: BinaryOp::Eq,
                left: Box::new(Expr::Column(col.to_string())),
                right: Box::new(Expr::Literal(Literal::Int(value))),
            }),
            projections: vec![],
        }
    }

    // ── Test 1: small table always uses SeqScan ───────────────────────────────

    #[test]
    fn test_small_table_uses_seq_scan() {
        let stats = stats_with_rows("orders", 500);
        let plan = LogicalPlan::Scan {
            collection: "orders".to_string(),
            predicate: None,
            projections: vec![],
        };
        let physical = CostOptimizer::optimize(plan, &stats);
        assert!(
            matches!(physical, PhysicalPlan::SeqScan { .. }),
            "expected SeqScan for table with 500 rows, got {:?}",
            physical
        );
    }

    // ── Test 2: very selective predicate triggers IndexScan ───────────────────

    #[test]
    fn test_selective_predicate_uses_index_scan() {
        // 100,000 rows; histogram makes selectivity 0.005 (< 0.01).
        let mut stats = stats_with_rows("events", 100_000);

        // Build a histogram where only bucket 50 (out of 100) has entries.
        // Total = 1000; bucket 50 = 5  →  selectivity ≈ 0.005.
        let mut buckets = vec![0u64; 100];
        buckets[50] = 5;
        let total_rows = 1000u64;
        // Fill remaining rows into a different bucket so totals add up.
        buckets[0] = total_rows - 5;

        stats.column_histograms.insert(
            ("events".to_string(), "user_id".to_string()),
            Histogram {
                buckets,
                min_val: 0.0,
                max_val: 100.0,
            },
        );

        // user_id = 50  →  falls in bucket 50  →  selectivity = 5/1000 = 0.005
        let plan = scan_with_eq_pred("events", "user_id", 50);
        let physical = CostOptimizer::optimize(plan, &stats);

        assert!(
            physical.uses_index(),
            "expected IndexScan for highly selective predicate, got {:?}",
            physical
        );
    }

    // ── Test 3: high-selectivity predicate keeps SeqScan ─────────────────────

    #[test]
    fn test_high_selectivity_uses_seq_scan() {
        // 100,000 rows; no histogram → default selectivity 0.1 (> 0.01).
        let stats = stats_with_rows("products", 100_000);
        let plan = scan_with_eq_pred("products", "category", 1);
        let physical = CostOptimizer::optimize(plan, &stats);

        assert!(
            matches!(physical, PhysicalPlan::SeqScan { .. }),
            "expected SeqScan for high-selectivity predicate, got {:?}",
            physical
        );
    }

    // ── Test 4: VectorScan logical → VectorScan physical ─────────────────────

    #[test]
    fn test_vector_scan_plan() {
        let stats = stats_with_rows("documents", 50_000);
        let plan = LogicalPlan::VectorScan {
            collection: "documents".to_string(),
            vector_field: "embedding".to_string(),
            query: vec![0.1, 0.2, 0.3],
            threshold: 0.5,
        };
        let physical = CostOptimizer::optimize(plan, &stats);
        assert!(
            matches!(physical, PhysicalPlan::VectorScan { .. }),
            "expected VectorScan physical plan, got {:?}",
            physical
        );
    }

    // ── Test 5: Limit wraps SeqScan ───────────────────────────────────────────

    #[test]
    fn test_limit_wraps_scan() {
        let stats = stats_with_rows("users", 5_000);
        let plan = LogicalPlan::Limit {
            input: Box::new(LogicalPlan::Scan {
                collection: "users".to_string(),
                predicate: None,
                projections: vec![],
            }),
            n: 10,
        };
        let physical = CostOptimizer::optimize(plan, &stats);
        match &physical {
            PhysicalPlan::Limit { input, n } => {
                assert_eq!(*n, 10);
                assert!(
                    matches!(input.as_ref(), PhysicalPlan::SeqScan { .. }),
                    "expected SeqScan inside Limit, got {:?}",
                    input
                );
            }
            other => panic!("expected Limit {{ SeqScan }}, got {:?}", other),
        }
    }

    // ── Test 6: Filter propagates over SeqScan ────────────────────────────────

    #[test]
    fn test_filter_propagates() {
        let stats = stats_with_rows("logs", 10_000);
        let predicate = Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Column("level".to_string())),
            right: Box::new(Expr::Literal(Literal::Int(2))),
        };
        let plan = LogicalPlan::Filter {
            input: Box::new(LogicalPlan::Scan {
                collection: "logs".to_string(),
                predicate: None,
                projections: vec![],
            }),
            predicate,
        };
        let physical = CostOptimizer::optimize(plan, &stats);
        match &physical {
            PhysicalPlan::Filter { input, .. } => {
                assert!(
                    matches!(input.as_ref(), PhysicalPlan::SeqScan { .. }),
                    "expected SeqScan inside Filter, got {:?}",
                    input
                );
            }
            other => panic!("expected Filter {{ SeqScan }}, got {:?}", other),
        }
    }

    // ── Test 7: Empty logical plan → Empty physical plan ─────────────────────

    #[test]
    fn test_empty_plan() {
        let stats = Statistics {
            collection_row_count: HashMap::new(),
            column_histograms: HashMap::new(),
            confidence_histograms: HashMap::new(),
            index_sizes: HashMap::new(),
        };
        let physical = CostOptimizer::optimize(LogicalPlan::Empty, &stats);
        assert!(
            matches!(physical, PhysicalPlan::Empty),
            "expected Empty physical plan, got {:?}",
            physical
        );
    }

    // ── Test 8: 1M rows + 0.005 selectivity → uses_index() == true ───────────

    #[test]
    fn test_explain_index_scan() {
        // 1,000,000 rows; histogram makes the predicate hit only 0.5% of rows.
        let mut stats = stats_with_rows("metrics", 1_000_000);

        // Histogram: total = 2000 rows, target bucket = 10 rows → sel = 0.005.
        let mut buckets = vec![0u64; 100];
        buckets[42] = 10;
        buckets[0] = 1990;

        stats.column_histograms.insert(
            ("metrics".to_string(), "sensor_id".to_string()),
            Histogram {
                buckets,
                min_val: 0.0,
                max_val: 100.0,
            },
        );

        // sensor_id = 42  →  falls in bucket 42  →  selectivity = 10/2000 = 0.005
        let plan = scan_with_eq_pred("metrics", "sensor_id", 42);
        let physical = CostOptimizer::optimize(plan, &stats);

        assert!(
            physical.uses_index(),
            "expected uses_index() == true for 1M rows with 0.005 selectivity, got {:?}",
            physical
        );
    }
}
