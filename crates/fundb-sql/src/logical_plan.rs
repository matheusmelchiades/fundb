/// Typed logical query plan for FunQL.
///
/// The binder translates an AST `Statement` into a `LogicalPlan` tree that the
/// optimizer can work with without parsing concerns.
use std::collections::HashMap;
use std::ops::RangeInclusive;

// ── Scalar expression tree ────────────────────────────────────────────────────

/// Scalar expression tree used inside logical plan nodes.
#[derive(Debug, Clone)]
pub enum Expr {
    /// A field reference, e.g. `"age"` or `"_confidence"`.
    Column(String),
    /// A constant value.
    Literal(Literal),
    /// A binary operation.
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// A unary operation.
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    /// A function call, e.g. `count(*)`.
    FunctionCall { name: String, args: Vec<Expr> },
    /// A named query parameter, e.g. `:param_name`.
    Param(String),
    /// A vector field reference, e.g. `_vector('field')`.
    VectorRef { field: String },
    /// The wildcard `SELECT *`.
    Star,
}

// ── Literals ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
    Vector(Vec<f32>),
}

// ── Operators ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    VectorDist,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

// ── Sort / aggregate helpers ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SortExpr {
    pub expr: Expr,
    pub asc: bool,
}

#[derive(Debug, Clone)]
pub struct AggExpr {
    pub func: AggFunc,
    pub arg: Box<Expr>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggFunc {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

// ── Clause-specific option structs ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ContextOptions {
    pub max_tokens: Option<u32>,
    pub coherence: Option<f32>,
    pub diversity: Option<f32>,
    pub include_contradictions: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct UnderstandOptions {
    pub min_confidence: Option<f32>,
    pub within_days: Option<u32>,
    pub depth: Option<u32>,
    pub collection: Option<String>,
    pub vector_field: Option<String>,
    pub min_similarity: Option<f32>,
}

// ── LogicalPlan ───────────────────────────────────────────────────────────────

/// The output of the binder: a typed logical query plan tree.
#[derive(Debug, Clone)]
pub enum LogicalPlan {
    /// Full-collection scan with optional predicate and projection list.
    Scan {
        collection: String,
        predicate: Option<Expr>,
        projections: Vec<Expr>,
    },
    /// ANN / vector-distance scan over a single embedding field.
    VectorScan {
        collection: String,
        vector_field: String,
        query: Vec<f32>,
        threshold: f32,
    },
    /// Graph edge traversal.
    GraphTraverse {
        from: Expr,
        predicate: String,
        depth: RangeInclusive<u32>,
    },
    /// Causal-path tracing between two nodes.
    CausalTrace {
        from: Expr,
        to: Expr,
        max_depth: u32,
        min_strength: f32,
        min_stability: Option<f32>,
    },
    /// Predicate filter over an input plan.
    Filter {
        input: Box<LogicalPlan>,
        predicate: Expr,
    },
    /// Column-level projection over an input plan.
    Project {
        input: Box<LogicalPlan>,
        exprs: Vec<Expr>,
    },
    /// Inner join between two sub-plans.
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        condition: Expr,
    },
    /// Aggregation (GROUP BY + aggregate functions).
    Aggregate {
        input: Box<LogicalPlan>,
        group_by: Vec<Expr>,
        aggregates: Vec<AggExpr>,
    },
    /// Ordering.
    Sort {
        input: Box<LogicalPlan>,
        order_by: Vec<SortExpr>,
    },
    /// Row limit.
    Limit {
        input: Box<LogicalPlan>,
        n: usize,
    },
    /// Context-window optimization wrapper.
    ContextOptimize {
        input: Box<LogicalPlan>,
        options: ContextOptions,
    },
    /// Semantic-search / RAG intent node.
    Understand {
        intent: String,
        options: UnderstandOptions,
    },
    /// Causal effect estimation via an intervention model.
    EstimateEffect {
        model: String,
        set_vars: HashMap<String, f64>,
        predict: String,
    },
    /// Counterfactual reasoning over an observed state.
    Counterfactual {
        model: String,
        observed: HashMap<String, f64>,
        had: HashMap<String, f64>,
        predict: String,
    },
    /// INSERT INTO collection (columns) VALUES (row1), (row2), …
    Insert {
        collection: String,
        columns: Vec<String>,
        values: Vec<Vec<Expr>>,
    },
    /// Placeholder for statements that produce no result set
    /// (UPDATE, DELETE, REMEMBER, etc.).
    Empty,
}
