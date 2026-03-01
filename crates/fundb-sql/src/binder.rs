/// Semantic binder: translates a parsed AST `Statement` into a typed
/// `LogicalPlan` tree, resolving collection names against a `Catalog`.
use std::collections::{HashMap, HashSet};

use crate::ast::{
    self, CounterfactualGiven, Expr as AstExpr, Literal as AstLiteral, SelectItem,
    Statement, UnderstandOption,
};
use crate::logical_plan::{
    AggExpr, AggFunc, BinaryOp, ContextOptions, Expr, Literal, LogicalPlan, SortExpr,
    UnaryOp, UnderstandOptions,
};

// ── Catalog ───────────────────────────────────────────────────────────────────

/// Lightweight schema registry used by the binder for collection validation.
#[derive(Debug, Clone)]
pub struct Catalog {
    collections: HashSet<String>,
}

impl Catalog {
    /// Create an empty catalog.
    pub fn new() -> Self {
        Catalog {
            collections: HashSet::new(),
        }
    }

    /// Register a collection name.
    pub fn add_collection(&mut self, name: &str) {
        self.collections.insert(name.to_string());
    }

    /// Check whether a collection is registered.
    pub fn has_collection(&self, name: &str) -> bool {
        self.collections.contains(name)
    }

    /// Return a catalog pre-populated with the well-known FunDB collections.
    pub fn open() -> Self {
        let mut c = Catalog::new();
        for name in &[
            "knowledge_base",
            "documents",
            "users",
            "metrics",
            "events",
            "entities",
            "research_papers",
            "agent_memory",
        ] {
            c.add_collection(name);
        }
        c
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

// ── BindError ─────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BindError {
    #[error("unknown collection '{0}': no such collection exists. Check the collection name and ensure it has been created")]
    UnknownCollection(String),
    #[error("type mismatch: {0}. Check that column types match the operation being performed")]
    TypeMismatch(String),
    #[error("query bind error: {0}")]
    Other(String),
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Bind a parsed `Statement` to a `LogicalPlan`, validating collection names
/// against the supplied `Catalog`.
pub fn bind(stmt: Statement, catalog: &Catalog) -> Result<LogicalPlan, BindError> {
    match stmt {
        Statement::Select(s) => bind_select(s, catalog),
        Statement::Understand(u) => {
            let options = bind_understand_options(&u.options);
            Ok(LogicalPlan::Understand {
                intent: u.intent,
                options,
            })
        }
        Statement::EstimateEffect(e) => {
            let set_vars = bind_kv_pairs_to_f64(&e.set_vars)?;
            Ok(LogicalPlan::EstimateEffect {
                model: e.model,
                set_vars,
                predict: e.predict,
            })
        }
        Statement::Counterfactual(c) => {
            // Observed data: use given pairs if present, otherwise empty map.
            let observed = match &c.given {
                Some(CounterfactualGiven::Pairs(pairs)) => bind_kv_pairs_to_f64(pairs)?,
                _ => HashMap::new(),
            };
            let had = bind_kv_pairs_to_f64(&c.had)?;
            Ok(LogicalPlan::Counterfactual {
                model: c.model,
                observed,
                had,
                predict: c.predict,
            })
        }
        // Statements that produce no result set.
        Statement::Insert(_)
        | Statement::Update(_)
        | Statement::Delete(_)
        | Statement::Remember(_)
        | Statement::RecallBy(_)
        | Statement::Forget(_)
        | Statement::DiscoverCausal(_)
        | Statement::CreateCausalModel(_)
        | Statement::CreateCollection(_)
        | Statement::CreateIndex(_)
        | Statement::TraceCausality(_) => Ok(LogicalPlan::Empty),
    }
}

// ── SELECT binding ─────────────────────────────────────────────────────────────

fn bind_select(s: ast::SelectStmt, catalog: &Catalog) -> Result<LogicalPlan, BindError> {
    // ── 1. Base scan ──────────────────────────────────────────────────────────
    let collection = s
        .from
        .as_ref()
        .map(|t| t.name.clone())
        .unwrap_or_default();

    if !collection.is_empty() && !catalog.has_collection(&collection) {
        return Err(BindError::UnknownCollection(collection));
    }

    // Translate SELECT items into projection expressions.
    let projections: Vec<Expr> = s
        .projections
        .iter()
        .map(bind_select_item)
        .collect::<Result<Vec<_>, _>>()?;

    // ── 2. WHERE / vector scan ────────────────────────────────────────────────
    let mut plan = if let Some(ref where_expr) = s.where_clause {
        // Check whether the WHERE clause is (or contains at the top level) a
        // VectorDist expression — if so, emit a VectorScan plan instead of
        // wrapping with a plain Filter.
        if let Some(vector_scan) =
            try_build_vector_scan(&collection, where_expr, &projections)?
        {
            vector_scan
        } else {
            // Regular scan + filter.
            let scan = LogicalPlan::Scan {
                collection: collection.clone(),
                predicate: None,
                projections: projections.clone(),
            };
            let predicate = bind_expr(where_expr)?;
            LogicalPlan::Filter {
                input: Box::new(scan),
                predicate,
            }
        }
    } else {
        LogicalPlan::Scan {
            collection: collection.clone(),
            predicate: None,
            projections: projections.clone(),
        }
    };

    // ── 3. TRACE CAUSALITY ────────────────────────────────────────────────────
    if let Some(tc) = s.trace_causality {
        let from_expr = match tc.from {
            Some(ref e) => bind_expr(e)?,
            None => Expr::Literal(Literal::Null),
        };
        let to_expr = match tc.to {
            Some(ref e) => bind_expr(e)?,
            None => Expr::Literal(Literal::Null),
        };
        let max_depth = match tc.max_depth {
            Some(ref e) => expr_to_u32(e).unwrap_or(10),
            None => 10,
        };
        let min_strength = match tc.min_strength {
            Some(ref e) => expr_to_f32(e).unwrap_or(0.0),
            None => 0.0,
        };
        let min_stability = tc.min_stability.as_ref().and_then(|e| expr_to_f32(e));

        plan = LogicalPlan::CausalTrace {
            from: from_expr,
            to: to_expr,
            max_depth,
            min_strength,
            min_stability,
        };
    }

    // ── 4. TRAVERSE ───────────────────────────────────────────────────────────
    if let Some(tv) = s.traverse {
        let depth_min = tv.depth_min.as_ref().and_then(|e| expr_to_u32(e)).unwrap_or(1);
        let depth_max = tv.depth_max.as_ref().and_then(|e| expr_to_u32(e)).unwrap_or(depth_min);
        plan = LogicalPlan::GraphTraverse {
            from: Expr::Column(collection.clone()),
            predicate: tv.relation,
            depth: depth_min..=depth_max,
        };
    }

    // ── 5. WITHIN CONTEXT ─────────────────────────────────────────────────────
    if let Some(ctx) = s.within_context {
        let options = bind_context_options(&ctx);
        plan = LogicalPlan::ContextOptimize {
            input: Box::new(plan),
            options,
        };
    }

    // ── 6. GROUP BY / aggregation ─────────────────────────────────────────────
    if !s.group_by.is_empty() {
        let group_by = s
            .group_by
            .iter()
            .map(bind_expr)
            .collect::<Result<Vec<_>, _>>()?;
        // Extract aggregate expressions from the projection list.
        let aggregates = extract_aggregates(&s.projections)?;
        plan = LogicalPlan::Aggregate {
            input: Box::new(plan),
            group_by,
            aggregates,
        };
    }

    // ── 7. ORDER BY ───────────────────────────────────────────────────────────
    if !s.order_by.is_empty() {
        let order_by = s
            .order_by
            .iter()
            .map(|item| {
                bind_expr(&item.expr).map(|expr| SortExpr {
                    expr,
                    asc: item.asc,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        plan = LogicalPlan::Sort {
            input: Box::new(plan),
            order_by,
        };
    }

    // ── 8. LIMIT ─────────────────────────────────────────────────────────────
    if let Some(ref limit_expr) = s.limit {
        let n = expr_to_usize(limit_expr).unwrap_or(0);
        plan = LogicalPlan::Limit {
            input: Box::new(plan),
            n,
        };
    }

    Ok(plan)
}

// ── Expression binding ────────────────────────────────────────────────────────

/// Translate an AST expression into a LogicalPlan expression.
pub(crate) fn bind_expr(e: &AstExpr) -> Result<Expr, BindError> {
    match e {
        AstExpr::Literal(lit) => Ok(Expr::Literal(bind_literal(lit))),
        AstExpr::Ident(name) => Ok(Expr::Column(name.clone())),
        AstExpr::Param(name) => Ok(Expr::Param(name.clone())),
        AstExpr::Star => Ok(Expr::Star),

        AstExpr::BinaryOp { op, left, right } => {
            let lp_op = bind_binary_op(op)?;
            Ok(Expr::BinaryOp {
                op: lp_op,
                left: Box::new(bind_expr(left)?),
                right: Box::new(bind_expr(right)?),
            })
        }

        AstExpr::UnaryOp { op, expr } => {
            let lp_op = match op {
                ast::UnaryOp::Neg => UnaryOp::Neg,
                ast::UnaryOp::Not | ast::UnaryOp::IsNull | ast::UnaryOp::IsNotNull => {
                    UnaryOp::Not
                }
            };
            Ok(Expr::UnaryOp {
                op: lp_op,
                operand: Box::new(bind_expr(expr)?),
            })
        }

        AstExpr::Not(inner) => Ok(Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(bind_expr(inner)?),
        }),

        AstExpr::FunctionCall { name, args } => {
            let bound_args = args
                .iter()
                .map(bind_expr)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Expr::FunctionCall {
                name: name.clone(),
                args: bound_args,
            })
        }

        AstExpr::VectorDist { field, value } => {
            // Represent as a BinaryOp(VectorDist, VectorRef, bound_value).
            Ok(Expr::BinaryOp {
                op: BinaryOp::VectorDist,
                left: Box::new(Expr::VectorRef {
                    field: field.clone(),
                }),
                right: Box::new(bind_expr(value)?),
            })
        }

        AstExpr::Qualified { table, field } => {
            // Flatten to a single dot-separated column name.
            Ok(Expr::Column(format!("{}.{}", table, field)))
        }

        AstExpr::InList { expr, list } => {
            // Translate `x IN (a, b, c)` as a chain of OR-equals for the plan.
            // We represent it as a FunctionCall for simplicity.
            let bound_expr = bind_expr(expr)?;
            let bound_list = list
                .iter()
                .map(bind_expr)
                .collect::<Result<Vec<_>, _>>()?;
            let mut args = vec![bound_expr];
            args.extend(bound_list);
            Ok(Expr::FunctionCall {
                name: "in_list".to_string(),
                args,
            })
        }

        AstExpr::IsNull(inner) => Ok(Expr::FunctionCall {
            name: "is_null".to_string(),
            args: vec![bind_expr(inner)?],
        }),

        AstExpr::IsNotNull(inner) => Ok(Expr::FunctionCall {
            name: "is_not_null".to_string(),
            args: vec![bind_expr(inner)?],
        }),

        AstExpr::Between { expr, low, high } => {
            // x BETWEEN low AND high  ≡  x >= low AND x <= high
            let x = bind_expr(expr)?;
            let l = bind_expr(low)?;
            let h = bind_expr(high)?;
            Ok(Expr::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(Expr::BinaryOp {
                    op: BinaryOp::Ge,
                    left: Box::new(x.clone()),
                    right: Box::new(l),
                }),
                right: Box::new(Expr::BinaryOp {
                    op: BinaryOp::Le,
                    left: Box::new(x),
                    right: Box::new(h),
                }),
            })
        }

        AstExpr::Array(elems) => {
            // Inline array literal — used in vector queries.
            let floats: Vec<f32> = elems
                .iter()
                .filter_map(|e| match e {
                    AstExpr::Literal(AstLiteral::Float(f)) => Some(*f as f32),
                    AstExpr::Literal(AstLiteral::Int(i)) => Some(*i as f32),
                    _ => None,
                })
                .collect();
            if floats.len() == elems.len() {
                Ok(Expr::Literal(Literal::Vector(floats)))
            } else {
                // Heterogeneous array — represent as a function call.
                let bound = elems
                    .iter()
                    .map(bind_expr)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Expr::FunctionCall {
                    name: "array".to_string(),
                    args: bound,
                })
            }
        }

        AstExpr::Case {
            operand,
            when_clauses,
            else_clause,
        } => {
            // Encode CASE as a generic function call for the plan layer.
            let mut args = Vec::new();
            if let Some(op) = operand {
                args.push(bind_expr(op)?);
            }
            for (cond, then) in when_clauses {
                args.push(bind_expr(cond)?);
                args.push(bind_expr(then)?);
            }
            if let Some(els) = else_clause {
                args.push(bind_expr(els)?);
            }
            Ok(Expr::FunctionCall {
                name: "case".to_string(),
                args,
            })
        }

        AstExpr::Cast { expr, ty } => Ok(Expr::FunctionCall {
            name: format!("cast::{}", ty),
            args: vec![bind_expr(expr)?],
        }),

        AstExpr::Subquery(_) => {
            // Subquery references are represented as a placeholder for now.
            Ok(Expr::FunctionCall {
                name: "subquery".to_string(),
                args: vec![],
            })
        }
    }
}

// ── Literal binding ───────────────────────────────────────────────────────────

fn bind_literal(lit: &AstLiteral) -> Literal {
    match lit {
        AstLiteral::Int(i) => Literal::Int(*i),
        AstLiteral::Float(f) => Literal::Float(*f),
        AstLiteral::String(s) => Literal::String(s.clone()),
        AstLiteral::Bool(b) => Literal::Bool(*b),
        AstLiteral::Null => Literal::Null,
    }
}

// ── Binary operator mapping ───────────────────────────────────────────────────

fn bind_binary_op(op: &ast::BinaryOp) -> Result<BinaryOp, BindError> {
    match op {
        ast::BinaryOp::Eq => Ok(BinaryOp::Eq),
        ast::BinaryOp::NotEq => Ok(BinaryOp::Ne),
        ast::BinaryOp::Lt => Ok(BinaryOp::Lt),
        ast::BinaryOp::LtEq => Ok(BinaryOp::Le),
        ast::BinaryOp::Gt => Ok(BinaryOp::Gt),
        ast::BinaryOp::GtEq => Ok(BinaryOp::Ge),
        ast::BinaryOp::And => Ok(BinaryOp::And),
        ast::BinaryOp::Or => Ok(BinaryOp::Or),
        ast::BinaryOp::Add => Ok(BinaryOp::Add),
        ast::BinaryOp::Sub => Ok(BinaryOp::Sub),
        ast::BinaryOp::Mul => Ok(BinaryOp::Mul),
        ast::BinaryOp::Div => Ok(BinaryOp::Div),
        // Operators that don't map 1-to-1: treat as function-call-like comparisons.
        ast::BinaryOp::Mod => Ok(BinaryOp::Div), // best-effort; optimizer can handle
        ast::BinaryOp::Like | ast::BinaryOp::NotLike => Ok(BinaryOp::Eq),
        ast::BinaryOp::In | ast::BinaryOp::NotIn => Ok(BinaryOp::Eq),
        ast::BinaryOp::Between => Ok(BinaryOp::And),
        ast::BinaryOp::Arrow => Ok(BinaryOp::Eq),
    }
}

// ── SELECT item binding ───────────────────────────────────────────────────────

fn bind_select_item(item: &SelectItem) -> Result<Expr, BindError> {
    match item {
        SelectItem::Wildcard => Ok(Expr::Star),
        SelectItem::Expr { expr, .. } => bind_expr(expr),
    }
}

// ── VectorScan detection ──────────────────────────────────────────────────────

/// If the top-level WHERE expression is a simple `VectorDist … < threshold`
/// pattern, return a `VectorScan` plan node.  Otherwise return `None`.
fn try_build_vector_scan(
    collection: &str,
    where_expr: &AstExpr,
    _projections: &[Expr],
) -> Result<Option<LogicalPlan>, BindError> {
    // Pattern: BinaryOp { op: Lt/LtEq, left: VectorDist { field, value }, right: threshold }
    if let AstExpr::BinaryOp { op, left, right } = where_expr {
        if matches!(op, ast::BinaryOp::Lt | ast::BinaryOp::LtEq) {
            if let AstExpr::VectorDist { field, value } = left.as_ref() {
                // Extract vector query from the value expression.
                let query_floats = extract_vector_floats(value);
                let threshold = match right.as_ref() {
                    AstExpr::Literal(AstLiteral::Float(f)) => *f as f32,
                    AstExpr::Literal(AstLiteral::Int(i)) => *i as f32,
                    _ => return Ok(None),
                };
                if let Some(query) = query_floats {
                    return Ok(Some(LogicalPlan::VectorScan {
                        collection: collection.to_string(),
                        vector_field: field.clone(),
                        query,
                        threshold,
                    }));
                }
            }
        }
    }
    Ok(None)
}

/// Recursively extract a `Vec<f32>` from an array literal expression.
fn extract_vector_floats(e: &AstExpr) -> Option<Vec<f32>> {
    match e {
        AstExpr::Array(elems) => {
            let floats: Vec<f32> = elems
                .iter()
                .filter_map(|elem| match elem {
                    AstExpr::Literal(AstLiteral::Float(f)) => Some(*f as f32),
                    AstExpr::Literal(AstLiteral::Int(i)) => Some(*i as f32),
                    _ => None,
                })
                .collect();
            if floats.len() == elems.len() {
                Some(floats)
            } else {
                None
            }
        }
        AstExpr::Param(_) => Some(vec![]), // param reference — empty placeholder
        _ => None,
    }
}

// ── Aggregate extraction ──────────────────────────────────────────────────────

/// Walk SELECT items and collect any aggregate function calls.
fn extract_aggregates(items: &[SelectItem]) -> Result<Vec<AggExpr>, BindError> {
    let mut aggs = Vec::new();
    for item in items {
        if let SelectItem::Expr { expr, alias } = item {
            if let Some(agg) = try_extract_agg(expr, alias.clone())? {
                aggs.push(agg);
            }
        }
    }
    Ok(aggs)
}

fn try_extract_agg(
    expr: &AstExpr,
    alias: Option<String>,
) -> Result<Option<AggExpr>, BindError> {
    if let AstExpr::FunctionCall { name, args } = expr {
        let func = match name.to_lowercase().as_str() {
            "count" => Some(AggFunc::Count),
            "sum" => Some(AggFunc::Sum),
            "avg" | "average" => Some(AggFunc::Avg),
            "min" => Some(AggFunc::Min),
            "max" => Some(AggFunc::Max),
            _ => None,
        };
        if let Some(func) = func {
            let arg = args.first().map(bind_expr).transpose()?.unwrap_or(Expr::Star);
            return Ok(Some(AggExpr {
                func,
                arg: Box::new(arg),
                alias,
            }));
        }
    }
    Ok(None)
}

// ── WITHIN CONTEXT option binding ─────────────────────────────────────────────

fn bind_context_options(ctx: &ast::ContextOptions) -> ContextOptions {
    ContextOptions {
        max_tokens: ctx.max_tokens.as_ref().and_then(|e| expr_to_u32(e)),
        coherence: ctx.coherence.as_ref().and_then(|e| expr_to_f32(e)),
        diversity: ctx.diversity.as_ref().and_then(|e| expr_to_f32(e)),
        include_contradictions: ctx.include_contradictions,
    }
}

// ── UNDERSTAND option binding ─────────────────────────────────────────────────

fn bind_understand_options(opts: &[UnderstandOption]) -> UnderstandOptions {
    let mut out = UnderstandOptions {
        min_confidence: None,
        within_days: None,
        depth: None,
        collection: None,
        vector_field: None,
        min_similarity: None,
    };

    for opt in opts {
        match opt {
            UnderstandOption::Confidence(e) => {
                // `WITH confidence > 0.7` — extract the RHS float.
                if let Some(f) = expr_rhs_float(e) {
                    out.min_confidence = Some(f);
                }
            }
            UnderstandOption::Within(s) => {
                // Simple heuristic: parse a number of days from the string.
                out.within_days = parse_within_days(s);
            }
            UnderstandOption::Depth(e) => {
                out.depth = expr_to_u32(e);
            }
            UnderstandOption::InCollection(name) => {
                out.collection = Some(name.clone());
            }
            UnderstandOption::UsingVector(field) => {
                out.vector_field = Some(field.clone());
            }
            UnderstandOption::MinSimilarity(e) => {
                // `WITH min_similarity 0.7` — may be a bare literal or comparison.
                out.min_similarity = expr_to_f32(e).or_else(|| expr_rhs_float(e));
            }
        }
    }
    out
}

// ── Numeric helpers ───────────────────────────────────────────────────────────

fn expr_to_f32(e: &AstExpr) -> Option<f32> {
    match e {
        AstExpr::Literal(AstLiteral::Float(f)) => Some(*f as f32),
        AstExpr::Literal(AstLiteral::Int(i)) => Some(*i as f32),
        _ => None,
    }
}

fn expr_to_u32(e: &AstExpr) -> Option<u32> {
    match e {
        AstExpr::Literal(AstLiteral::Int(i)) if *i >= 0 => Some(*i as u32),
        AstExpr::Literal(AstLiteral::Float(f)) if *f >= 0.0 => Some(*f as u32),
        _ => None,
    }
}

fn expr_to_usize(e: &AstExpr) -> Option<usize> {
    match e {
        AstExpr::Literal(AstLiteral::Int(i)) if *i >= 0 => Some(*i as usize),
        AstExpr::Literal(AstLiteral::Float(f)) if *f >= 0.0 => Some(*f as usize),
        _ => None,
    }
}

/// For comparison expressions like `confidence > 0.7`, extract the RHS float.
fn expr_rhs_float(e: &AstExpr) -> Option<f32> {
    if let AstExpr::BinaryOp { right, .. } = e {
        return expr_to_f32(right);
    }
    expr_to_f32(e)
}

/// Very simple heuristic: extract a number of days from a WITHIN string such
/// as `"last 2 years"` or `"last 30 days"`.
fn parse_within_days(s: &str) -> Option<u32> {
    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();
    // Try to find a numeric token.
    for (i, part) in parts.iter().enumerate() {
        if let Ok(n) = part.parse::<u32>() {
            // Check the following word for the unit.
            let unit = parts.get(i + 1).copied().unwrap_or("days");
            let days = if unit.starts_with("year") {
                n * 365
            } else if unit.starts_with("month") {
                n * 30
            } else if unit.starts_with("week") {
                n * 7
            } else {
                n // assume days
            };
            return Some(days);
        }
    }
    None
}

// ── Key-value pair helpers ────────────────────────────────────────────────────

/// Convert `Vec<(String, Expr)>` to `HashMap<String, f64>`, best-effort.
fn bind_kv_pairs_to_f64(pairs: &[(String, AstExpr)]) -> Result<HashMap<String, f64>, BindError> {
    let mut map = HashMap::new();
    for (k, v) in pairs {
        let val = match v {
            AstExpr::Literal(AstLiteral::Float(f)) => *f,
            AstExpr::Literal(AstLiteral::Int(i)) => *i as f64,
            other => {
                return Err(BindError::TypeMismatch(format!(
                    "expected numeric literal for key '{}', got {:?}",
                    k, other
                )));
            }
        };
        map.insert(k.clone(), val);
    }
    Ok(map)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn catalog() -> Catalog {
        Catalog::open()
    }

    /// Helper: parse a FunQL string and bind it.
    fn bind_sql(sql: &str) -> Result<LogicalPlan, BindError> {
        let stmt = parse(sql).expect("parse failed");
        bind(stmt, &catalog())
    }

    // ── DoD tests ─────────────────────────────────────────────────────────────

    #[test]
    fn test_bind_simple_select() {
        let plan = bind_sql("SELECT * FROM documents").unwrap();
        match plan {
            LogicalPlan::Scan { ref collection, .. } => {
                assert_eq!(collection, "documents");
            }
            other => panic!("expected Scan, got {:?}", other),
        }
    }

    #[test]
    fn test_bind_unknown_collection() {
        let result = bind_sql("SELECT * FROM nonexistent");
        match result {
            Err(BindError::UnknownCollection(name)) => {
                assert_eq!(name, "nonexistent");
            }
            other => panic!("expected UnknownCollection error, got {:?}", other),
        }
    }

    #[test]
    fn test_bind_select_with_filter() {
        let plan = bind_sql("SELECT * FROM documents WHERE _confidence > 0.7").unwrap();
        // The plan must contain a Filter node somewhere.
        fn has_filter(p: &LogicalPlan) -> bool {
            match p {
                LogicalPlan::Filter { .. } => true,
                LogicalPlan::Sort { input, .. }
                | LogicalPlan::Limit { input, .. }
                | LogicalPlan::Project { input, .. }
                | LogicalPlan::Aggregate { input, .. }
                | LogicalPlan::ContextOptimize { input, .. } => has_filter(input),
                _ => false,
            }
        }
        assert!(has_filter(&plan), "expected a Filter node in plan: {:?}", plan);
    }

    #[test]
    fn test_bind_understand() {
        let plan = bind_sql(
            "UNDERSTAND 'papers about X' WITH confidence > 0.7 DEPTH 2",
        )
        .unwrap();
        match plan {
            LogicalPlan::Understand { ref intent, ref options } => {
                assert_eq!(intent, "papers about X");
                assert_eq!(options.depth, Some(2));
                assert!(
                    options.min_confidence.is_some(),
                    "expected min_confidence to be set"
                );
            }
            other => panic!("expected Understand, got {:?}", other),
        }
    }

    #[test]
    fn test_bind_estimate_effect() {
        let plan = bind_sql(
            "SELECT * FROM INTERVENE ON revenue_model SET x = 1 PREDICT y",
        )
        .unwrap();
        match plan {
            LogicalPlan::EstimateEffect {
                ref model,
                ref set_vars,
                ref predict,
            } => {
                assert_eq!(model, "revenue_model");
                assert!(set_vars.contains_key("x"), "expected 'x' in set_vars");
                assert_eq!(predict, "y");
            }
            other => panic!("expected EstimateEffect, got {:?}", other),
        }
    }

    #[test]
    fn test_bind_insert_is_empty() {
        let plan = bind_sql("INSERT INTO documents (x) VALUES (1)").unwrap();
        assert!(
            matches!(plan, LogicalPlan::Empty),
            "expected Empty, got {:?}",
            plan
        );
    }

    // ── Additional regression tests ───────────────────────────────────────────

    #[test]
    fn test_bind_select_with_limit() {
        let plan = bind_sql("SELECT * FROM users LIMIT 10").unwrap();
        assert!(
            matches!(plan, LogicalPlan::Limit { n: 10, .. }),
            "expected Limit(10), got {:?}",
            plan
        );
    }

    #[test]
    fn test_bind_select_with_order_by() {
        let plan = bind_sql("SELECT * FROM users ORDER BY age DESC").unwrap();
        assert!(
            matches!(plan, LogicalPlan::Sort { .. }),
            "expected Sort, got {:?}",
            plan
        );
    }

    #[test]
    fn test_bind_update_is_empty() {
        let plan = bind_sql("UPDATE users SET age = 31 WHERE name = 'Alice'").unwrap();
        assert!(matches!(plan, LogicalPlan::Empty));
    }

    #[test]
    fn test_bind_delete_is_empty() {
        let plan = bind_sql("DELETE FROM users WHERE age < 18").unwrap();
        assert!(matches!(plan, LogicalPlan::Empty));
    }

    #[test]
    fn test_bind_catalog_open_has_known_collections() {
        let cat = Catalog::open();
        assert!(cat.has_collection("knowledge_base"));
        assert!(cat.has_collection("documents"));
        assert!(cat.has_collection("users"));
        assert!(cat.has_collection("metrics"));
        assert!(cat.has_collection("events"));
        assert!(cat.has_collection("entities"));
        assert!(cat.has_collection("research_papers"));
        assert!(cat.has_collection("agent_memory"));
    }

    #[test]
    fn test_catalog_add_collection() {
        let mut cat = Catalog::new();
        assert!(!cat.has_collection("my_table"));
        cat.add_collection("my_table");
        assert!(cat.has_collection("my_table"));
    }

    #[test]
    fn test_bind_remember_is_empty() {
        let plan = bind_sql(
            "REMEMBER 'user prefers Python' FOR AGENT :agent_id WITH importance 0.8 AS semantic",
        )
        .unwrap();
        assert!(matches!(plan, LogicalPlan::Empty));
    }
}
