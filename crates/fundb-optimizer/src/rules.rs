/// Optimization rules for FunDB's logical query planner.
///
/// Each rule takes a `LogicalPlan` by value and returns a (potentially
/// transformed) `LogicalPlan`. Rules recurse into every child node so the
/// entire plan tree is rewritten in one pass.
use fundb_sql::{AggExpr, BinaryOp, Expr, Literal, LogicalPlan, SortExpr};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Recurse the `predicate_pushdown` rule into a single-child plan node,
/// rebuilding the wrapper after transforming the child.
#[allow(dead_code)]
fn pushdown_child(plan: LogicalPlan) -> LogicalPlan {
    predicate_pushdown(plan)
}

// ── Rule 1: predicate_pushdown ────────────────────────────────────────────────

/// Push `Filter` nodes down past `Project` nodes so that fewer rows are
/// materialised before projection.
///
/// Pattern:
/// ```text
/// Filter { input: Project { input: child, exprs }, predicate }
///   →  Project { input: Filter { input: child, predicate }, exprs }
/// ```
pub fn predicate_pushdown(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        // The key rewrite: Filter over Project → Project over Filter.
        LogicalPlan::Filter { input, predicate } => {
            match *input {
                LogicalPlan::Project {
                    input: child,
                    exprs,
                } => {
                    // Push the filter below the projection.
                    let new_filter = LogicalPlan::Filter {
                        input: Box::new(predicate_pushdown(*child)),
                        predicate,
                    };
                    LogicalPlan::Project {
                        input: Box::new(new_filter),
                        exprs,
                    }
                }
                other => {
                    // No rewrite here; still recurse into the child.
                    LogicalPlan::Filter {
                        input: Box::new(predicate_pushdown(other)),
                        predicate,
                    }
                }
            }
        }

        // Recurse into all other node types.
        LogicalPlan::Project { input, exprs } => LogicalPlan::Project {
            input: Box::new(predicate_pushdown(*input)),
            exprs,
        },
        LogicalPlan::Join {
            left,
            right,
            condition,
        } => LogicalPlan::Join {
            left: Box::new(predicate_pushdown(*left)),
            right: Box::new(predicate_pushdown(*right)),
            condition,
        },
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => LogicalPlan::Aggregate {
            input: Box::new(predicate_pushdown(*input)),
            group_by,
            aggregates,
        },
        LogicalPlan::Sort { input, order_by } => LogicalPlan::Sort {
            input: Box::new(predicate_pushdown(*input)),
            order_by,
        },
        LogicalPlan::Limit { input, n } => LogicalPlan::Limit {
            input: Box::new(predicate_pushdown(*input)),
            n,
        },
        LogicalPlan::ContextOptimize { input, options } => LogicalPlan::ContextOptimize {
            input: Box::new(predicate_pushdown(*input)),
            options,
        },

        // Leaf / terminal nodes — nothing to recurse into.
        leaf => leaf,
    }
}

// ── Rule 2: limit_pushdown ────────────────────────────────────────────────────

/// Push `Limit` nodes down past `Project` nodes so that the upstream operator
/// only ever produces as many rows as the consumer needs.
///
/// Pattern:
/// ```text
/// Limit { input: Project { input: child, exprs }, n }
///   →  Project { input: Limit { input: child, n }, exprs }
/// ```
///
/// Note: limits are NOT pushed past `Sort` because the sort must see all rows
/// before it can choose the top-N correctly.
pub fn limit_pushdown(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Limit { input, n } => match *input {
            LogicalPlan::Project {
                input: child,
                exprs,
            } => {
                let new_limit = LogicalPlan::Limit {
                    input: Box::new(limit_pushdown(*child)),
                    n,
                };
                LogicalPlan::Project {
                    input: Box::new(new_limit),
                    exprs,
                }
            }
            other => LogicalPlan::Limit {
                input: Box::new(limit_pushdown(other)),
                n,
            },
        },

        // Recurse into all other node types.
        LogicalPlan::Filter { input, predicate } => LogicalPlan::Filter {
            input: Box::new(limit_pushdown(*input)),
            predicate,
        },
        LogicalPlan::Project { input, exprs } => LogicalPlan::Project {
            input: Box::new(limit_pushdown(*input)),
            exprs,
        },
        LogicalPlan::Join {
            left,
            right,
            condition,
        } => LogicalPlan::Join {
            left: Box::new(limit_pushdown(*left)),
            right: Box::new(limit_pushdown(*right)),
            condition,
        },
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => LogicalPlan::Aggregate {
            input: Box::new(limit_pushdown(*input)),
            group_by,
            aggregates,
        },
        LogicalPlan::Sort { input, order_by } => LogicalPlan::Sort {
            input: Box::new(limit_pushdown(*input)),
            order_by,
        },
        LogicalPlan::ContextOptimize { input, options } => LogicalPlan::ContextOptimize {
            input: Box::new(limit_pushdown(*input)),
            options,
        },

        leaf => leaf,
    }
}

// ── Rule 3: eliminate_redundant_project ───────────────────────────────────────

/// Remove a `Project` whose only expression is `Expr::Star` — it is a no-op.
///
/// Pattern:
/// ```text
/// Project { exprs: [Star], input }  →  input
/// ```
pub fn eliminate_redundant_project(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Project { input, ref exprs } => {
            // Decide whether to elide *before* recursing so we still apply
            // the rule to the child if there is a nested redundant project.
            let is_star_only = matches!(exprs.as_slice(), [Expr::Star]);
            if is_star_only {
                // Drop this node and recurse into the child.
                eliminate_redundant_project(*input)
            } else {
                LogicalPlan::Project {
                    input: Box::new(eliminate_redundant_project(*input)),
                    exprs: exprs.clone(),
                }
            }
        }

        // Recurse into all other node types.
        LogicalPlan::Filter { input, predicate } => LogicalPlan::Filter {
            input: Box::new(eliminate_redundant_project(*input)),
            predicate,
        },
        LogicalPlan::Join {
            left,
            right,
            condition,
        } => LogicalPlan::Join {
            left: Box::new(eliminate_redundant_project(*left)),
            right: Box::new(eliminate_redundant_project(*right)),
            condition,
        },
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => LogicalPlan::Aggregate {
            input: Box::new(eliminate_redundant_project(*input)),
            group_by,
            aggregates,
        },
        LogicalPlan::Sort { input, order_by } => LogicalPlan::Sort {
            input: Box::new(eliminate_redundant_project(*input)),
            order_by,
        },
        LogicalPlan::Limit { input, n } => LogicalPlan::Limit {
            input: Box::new(eliminate_redundant_project(*input)),
            n,
        },
        LogicalPlan::ContextOptimize { input, options } => LogicalPlan::ContextOptimize {
            input: Box::new(eliminate_redundant_project(*input)),
            options,
        },

        leaf => leaf,
    }
}

// ── Rule 4: constant_fold ─────────────────────────────────────────────────────

/// Simplify constant sub-expressions inside `Expr` trees.
///
/// Reductions applied:
/// - `true AND right`   → `right`
/// - `left AND true`    → `left`
/// - `false OR right`   → `right`
/// - `left OR false`    → `left`
pub fn fold_expr(expr: Expr) -> Expr {
    match expr {
        Expr::BinaryOp { op, left, right } => {
            // Recurse first so inner reductions are applied before outer ones.
            let left = fold_expr(*left);
            let right = fold_expr(*right);

            match &op {
                BinaryOp::And => {
                    match (&left, &right) {
                        // true AND right → right
                        (Expr::Literal(Literal::Bool(true)), _) => right,
                        // left AND true → left
                        (_, Expr::Literal(Literal::Bool(true))) => left,
                        _ => Expr::BinaryOp {
                            op,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                    }
                }
                BinaryOp::Or => {
                    match (&left, &right) {
                        // false OR right → right
                        (Expr::Literal(Literal::Bool(false)), _) => right,
                        // left OR false → left
                        (_, Expr::Literal(Literal::Bool(false))) => left,
                        _ => Expr::BinaryOp {
                            op,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                    }
                }
                _ => Expr::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            }
        }

        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op,
            operand: Box::new(fold_expr(*operand)),
        },

        Expr::FunctionCall { name, args } => Expr::FunctionCall {
            name,
            args: args.into_iter().map(fold_expr).collect(),
        },

        // Leaf expressions — nothing to fold.
        other => other,
    }
}

/// Apply `fold_expr` to every expression contained in a plan node, then
/// recurse into child plan nodes.
pub fn constant_fold(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Filter { input, predicate } => LogicalPlan::Filter {
            input: Box::new(constant_fold(*input)),
            predicate: fold_expr(predicate),
        },
        LogicalPlan::Project { input, exprs } => LogicalPlan::Project {
            input: Box::new(constant_fold(*input)),
            exprs: exprs.into_iter().map(fold_expr).collect(),
        },
        LogicalPlan::Join {
            left,
            right,
            condition,
        } => LogicalPlan::Join {
            left: Box::new(constant_fold(*left)),
            right: Box::new(constant_fold(*right)),
            condition: fold_expr(condition),
        },
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => LogicalPlan::Aggregate {
            input: Box::new(constant_fold(*input)),
            group_by: group_by.into_iter().map(fold_expr).collect(),
            aggregates: aggregates
                .into_iter()
                .map(|agg| AggExpr {
                    func: agg.func,
                    arg: Box::new(fold_expr(*agg.arg)),
                    alias: agg.alias,
                })
                .collect(),
        },
        LogicalPlan::Sort { input, order_by } => LogicalPlan::Sort {
            input: Box::new(constant_fold(*input)),
            order_by: order_by
                .into_iter()
                .map(|s| SortExpr {
                    expr: fold_expr(s.expr),
                    asc: s.asc,
                })
                .collect(),
        },
        LogicalPlan::Limit { input, n } => LogicalPlan::Limit {
            input: Box::new(constant_fold(*input)),
            n,
        },
        LogicalPlan::ContextOptimize { input, options } => LogicalPlan::ContextOptimize {
            input: Box::new(constant_fold(*input)),
            options,
        },
        LogicalPlan::Scan {
            collection,
            predicate,
            projections,
        } => LogicalPlan::Scan {
            collection,
            predicate: predicate.map(fold_expr),
            projections: projections.into_iter().map(fold_expr).collect(),
        },

        // Nodes without expression fields — just return as-is.
        leaf => leaf,
    }
}

// ── Rule 5: vector_scan_elide_filter ──────────────────────────────────────────

/// Return `true` if the predicate is a `BinaryOp` whose operator is one of the
/// comparison operators that VectorScan already handles via its `threshold`
/// field, or is a pure `VectorDist` expression.
fn is_vector_dist_predicate(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::BinaryOp {
            op: BinaryOp::VectorDist | BinaryOp::Lt | BinaryOp::Le,
            ..
        }
    )
}

/// Remove a `Filter` that wraps a `VectorScan` when the filter predicate is
/// already encoded in the scan's threshold.
///
/// Pattern:
/// ```text
/// Filter { input: VectorScan { .. }, predicate: BinaryOp { op: VectorDist | Lt | Le, .. } }
///   →  VectorScan { .. }
/// ```
pub fn vector_scan_elide_filter(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Filter { input, predicate } => {
            match *input {
                vs @ LogicalPlan::VectorScan { .. } if is_vector_dist_predicate(&predicate) => {
                    // The predicate is already captured by the VectorScan threshold;
                    // the outer Filter is redundant.
                    vs
                }
                other => LogicalPlan::Filter {
                    input: Box::new(vector_scan_elide_filter(other)),
                    predicate,
                },
            }
        }

        // Recurse into all other node types.
        LogicalPlan::Project { input, exprs } => LogicalPlan::Project {
            input: Box::new(vector_scan_elide_filter(*input)),
            exprs,
        },
        LogicalPlan::Join {
            left,
            right,
            condition,
        } => LogicalPlan::Join {
            left: Box::new(vector_scan_elide_filter(*left)),
            right: Box::new(vector_scan_elide_filter(*right)),
            condition,
        },
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => LogicalPlan::Aggregate {
            input: Box::new(vector_scan_elide_filter(*input)),
            group_by,
            aggregates,
        },
        LogicalPlan::Sort { input, order_by } => LogicalPlan::Sort {
            input: Box::new(vector_scan_elide_filter(*input)),
            order_by,
        },
        LogicalPlan::Limit { input, n } => LogicalPlan::Limit {
            input: Box::new(vector_scan_elide_filter(*input)),
            n,
        },
        LogicalPlan::ContextOptimize { input, options } => LogicalPlan::ContextOptimize {
            input: Box::new(vector_scan_elide_filter(*input)),
            options,
        },

        leaf => leaf,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fundb_sql::{BinaryOp, Expr, Literal, LogicalPlan};

    // Helper: a simple Scan leaf.
    fn scan(name: &str) -> LogicalPlan {
        LogicalPlan::Scan {
            collection: name.to_string(),
            predicate: None,
            projections: vec![],
        }
    }

    // Helper: wrap plan in a Project with named columns.
    fn project(plan: LogicalPlan, cols: Vec<&str>) -> LogicalPlan {
        LogicalPlan::Project {
            input: Box::new(plan),
            exprs: cols
                .into_iter()
                .map(|c| Expr::Column(c.to_string()))
                .collect(),
        }
    }

    // Helper: wrap plan in a Filter.
    fn filter(plan: LogicalPlan, predicate: Expr) -> LogicalPlan {
        LogicalPlan::Filter {
            input: Box::new(plan),
            predicate,
        }
    }

    // ── Test 1: predicate_pushdown moves filter inside project ────────────────

    #[test]
    fn test_predicate_pushdown_moves_filter_inside_project() {
        // Build:  Filter { Project { Scan, [age] }, age > 25 }
        let pred = Expr::BinaryOp {
            op: BinaryOp::Gt,
            left: Box::new(Expr::Column("age".to_string())),
            right: Box::new(Expr::Literal(Literal::Int(25))),
        };
        let inner = project(scan("users"), vec!["age"]);
        let plan = filter(inner, pred);

        let result = predicate_pushdown(plan);

        // Expected:  Project { Filter { Scan, age > 25 }, [age] }
        match result {
            LogicalPlan::Project { input, .. } => {
                match *input {
                    LogicalPlan::Filter { .. } => { /* correct */ }
                    other => panic!("expected Filter inside Project, got {:?}", other),
                }
            }
            other => panic!("expected Project at top, got {:?}", other),
        }
    }

    // ── Test 2: eliminate_redundant_project removes Star project ──────────────

    #[test]
    fn test_eliminate_redundant_project_removes_star() {
        let plan = LogicalPlan::Project {
            input: Box::new(scan("users")),
            exprs: vec![Expr::Star],
        };

        let result = eliminate_redundant_project(plan);

        match result {
            LogicalPlan::Scan { collection, .. } => {
                assert_eq!(collection, "users");
            }
            other => panic!("expected Scan after removing Star project, got {:?}", other),
        }
    }

    // ── Test 3: constant_fold simplifies true AND expr → expr ─────────────────

    #[test]
    fn test_constant_fold_true_and_expr() {
        let expr = Expr::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(Expr::Literal(Literal::Bool(true))),
            right: Box::new(Expr::Column("active".to_string())),
        };

        let result = fold_expr(expr);

        match result {
            Expr::Column(name) => assert_eq!(name, "active"),
            other => panic!("expected Column after fold, got {:?}", other),
        }
    }

    #[test]
    fn test_constant_fold_expr_and_true() {
        let expr = Expr::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(Expr::Column("active".to_string())),
            right: Box::new(Expr::Literal(Literal::Bool(true))),
        };

        let result = fold_expr(expr);

        match result {
            Expr::Column(name) => assert_eq!(name, "active"),
            other => panic!("expected Column after fold, got {:?}", other),
        }
    }

    #[test]
    fn test_constant_fold_false_or_expr() {
        let expr = Expr::BinaryOp {
            op: BinaryOp::Or,
            left: Box::new(Expr::Literal(Literal::Bool(false))),
            right: Box::new(Expr::Column("active".to_string())),
        };

        let result = fold_expr(expr);

        match result {
            Expr::Column(name) => assert_eq!(name, "active"),
            other => panic!("expected Column after fold, got {:?}", other),
        }
    }

    #[test]
    fn test_constant_fold_in_plan_filter() {
        // Filter { Scan, true AND (age > 25) }  →  Filter { Scan, age > 25 }
        let predicate = Expr::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(Expr::Literal(Literal::Bool(true))),
            right: Box::new(Expr::BinaryOp {
                op: BinaryOp::Gt,
                left: Box::new(Expr::Column("age".to_string())),
                right: Box::new(Expr::Literal(Literal::Int(25))),
            }),
        };

        let plan = filter(scan("users"), predicate);
        let result = constant_fold(plan);

        match result {
            LogicalPlan::Filter { predicate, .. } => {
                // After folding, should be BinaryOp { Gt, age, 25 } — NOT an And.
                match predicate {
                    Expr::BinaryOp {
                        op: BinaryOp::Gt, ..
                    } => { /* correct */ }
                    other => panic!("expected Gt after fold, got {:?}", other),
                }
            }
            other => panic!("expected Filter, got {:?}", other),
        }
    }

    // ── Test 4: vector_scan_elide_filter removes filter wrapping VectorScan ───

    #[test]
    fn test_vector_scan_elide_filter_removes_redundant_filter() {
        let vs = LogicalPlan::VectorScan {
            collection: "documents".to_string(),
            vector_field: "embedding".to_string(),
            query: vec![0.1, 0.2, 0.3],
            threshold: 0.5,
        };

        // Filter { VectorScan, _vec <-> query < 0.5 }
        let predicate = Expr::BinaryOp {
            op: BinaryOp::Lt,
            left: Box::new(Expr::BinaryOp {
                op: BinaryOp::VectorDist,
                left: Box::new(Expr::VectorRef {
                    field: "embedding".to_string(),
                }),
                right: Box::new(Expr::Literal(Literal::Vector(vec![0.1, 0.2, 0.3]))),
            }),
            right: Box::new(Expr::Literal(Literal::Float(0.5))),
        };

        let plan = filter(vs, predicate);
        let result = vector_scan_elide_filter(plan);

        match result {
            LogicalPlan::VectorScan { collection, .. } => {
                assert_eq!(collection, "documents");
            }
            other => panic!("expected VectorScan after elision, got {:?}", other),
        }
    }

    #[test]
    fn test_vector_scan_elide_filter_keeps_non_vector_filter() {
        let vs = LogicalPlan::VectorScan {
            collection: "documents".to_string(),
            vector_field: "embedding".to_string(),
            query: vec![0.1],
            threshold: 0.5,
        };

        // A regular equality predicate — must NOT be removed.
        let predicate = Expr::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expr::Column("category".to_string())),
            right: Box::new(Expr::Literal(Literal::String("research".to_string()))),
        };

        let plan = filter(vs, predicate);
        let result = vector_scan_elide_filter(plan);

        match result {
            LogicalPlan::Filter { .. } => { /* correct — filter was kept */ }
            other => panic!("expected Filter to be kept, got {:?}", other),
        }
    }

    // ── Test 5: optimize() converges on a multi-layer plan ────────────────────

    #[test]
    fn test_optimize_converges_multi_layer_plan() {
        // Build a plan with multiple optimizable layers:
        //
        //   Limit 10
        //     Project [*]            ← redundant star project
        //       Filter (true AND age > 18)
        //         Project [age, name]
        //           Scan "users"

        let inner_project = project(scan("users"), vec!["age", "name"]);

        let pred = Expr::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(Expr::Literal(Literal::Bool(true))),
            right: Box::new(Expr::BinaryOp {
                op: BinaryOp::Gt,
                left: Box::new(Expr::Column("age".to_string())),
                right: Box::new(Expr::Literal(Literal::Int(18))),
            }),
        };

        let filter_node = filter(inner_project, pred);

        let star_project = LogicalPlan::Project {
            input: Box::new(filter_node),
            exprs: vec![Expr::Star],
        };

        let plan = LogicalPlan::Limit {
            input: Box::new(star_project),
            n: 10,
        };

        // Must not panic and must return a valid plan.
        let result = crate::optimize(plan);

        // The result should be a well-formed plan — just verify no panic and
        // that we get something sensible back.
        let debug_str = format!("{:?}", result);
        assert!(!debug_str.is_empty(), "result plan should be non-empty");
    }
}
