// Complete AST for FunQL — a superset of SQL with vector, graph, temporal,
// confidence, causal, context-aware, and semantic extensions.

// ── Top-level statement ────────────────────────────────────────────────────

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select(SelectStmt),
    Insert(InsertStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
    CreateCollection(CreateCollectionStmt),
    CreateIndex(CreateIndexStmt),
    CreateCausalModel(CreateCausalModelStmt),
    Understand(UnderstandStmt),
    TraceCausality(TraceCausalityStmt),
    EstimateEffect(EstimateEffectStmt),
    Counterfactual(CounterfactualStmt),
    DiscoverCausal(DiscoverCausalStmt),
    Remember(RememberStmt),
    RecallBy(RecallByStmt),
    Forget(ForgetStmt),
}

// ── SELECT ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStmt {
    pub distinct: bool,
    pub projections: Vec<SelectItem>,
    pub from: Option<TableRef>,
    pub joins: Vec<JoinClause>,
    pub where_clause: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<Expr>,
    pub offset: Option<Expr>,
    /// `WITHIN CONTEXT (…)` clause
    pub within_context: Option<ContextOptions>,
    /// `AS OF …` clause
    pub as_of: Option<AsOfClause>,
    /// `TRAVERSE …` clause
    pub traverse: Option<TraverseClause>,
    /// `TRACE CAUSALITY …` clause (embedded in SELECT)
    pub trace_causality: Option<TraceCausalityClause>,
    /// `RETURN …` items (graph / causal)
    pub return_items: Vec<SelectItem>,
    /// `RECALL BY …` clause (embedded in SELECT … FROM AGENT MEMORY)
    pub recall_by: Option<RecallByClause>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    /// `*`
    Wildcard,
    /// `expr [AS alias]`
    Expr { expr: Expr, alias: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
    /// Indicates `FROM AGENT MEMORY <name>`
    pub is_agent_memory: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: TableRef,
    pub on: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Cross,
    Full,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    pub expr: Expr,
    pub asc: bool,
}

// ── Temporal AS OF ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AsOfClause {
    SystemTime(Expr),
    ValidTimeBetween(Expr, Expr),
    ValidTimeAt(Expr),
}

// ── WITHIN CONTEXT ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ContextOptions {
    pub max_tokens: Option<Expr>,
    pub coherence: Option<Expr>,
    pub diversity: Option<Expr>,
    pub include_contradictions: Option<bool>,
    pub priority: Vec<OrderByItem>,
}

// ── TRAVERSE ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct TraverseClause {
    /// Edge relation name, e.g. `follows`
    pub relation: String,
    /// Full chain for multi-hop: `users -> orders -> products`
    pub chain: Vec<String>,
    pub depth_min: Option<Expr>,
    pub depth_max: Option<Expr>,
    /// `-> alias` if present
    pub target_alias: Option<String>,
}

// ── TRACE CAUSALITY (embedded in SELECT) ────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct TraceCausalityClause {
    pub from: Option<Expr>,
    pub to: Option<Expr>,
    pub max_depth: Option<Expr>,
    pub min_strength: Option<Expr>,
    pub min_stability: Option<Expr>,
}

// ── RECALL BY (embedded in SELECT) ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct RecallByClause {
    pub weights: Vec<RecallWeight>,
}

// ── TRACE CAUSALITY (top-level statement) ───────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct TraceCausalityStmt {
    /// The collection to query: `SELECT * FROM <collection>`
    pub collection: String,
    pub from: Expr,
    pub to: Expr,
    pub max_depth: Option<Expr>,
    pub min_strength: Option<Expr>,
    pub min_stability: Option<Expr>,
    pub return_fields: Vec<SelectItem>,
}

// ── UNDERSTAND ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct UnderstandStmt {
    /// The natural language intent string
    pub intent: String,
    pub options: Vec<UnderstandOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnderstandOption {
    /// `WITH confidence > 0.7`
    Confidence(Expr),
    /// `WITHIN last 2 years` — stored as-is from the token stream
    Within(String),
    /// `DEPTH 2`
    Depth(Expr),
    /// `IN COLLECTION <name>`
    InCollection(String),
    /// `USING VECTOR '<field>'`
    UsingVector(String),
    /// `WITH min_similarity 0.7`
    MinSimilarity(Expr),
}

// ── CREATE CAUSAL MODEL ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCausalModelStmt {
    pub name: String,
    pub mode: CausalModelMode,
    pub variables: Vec<CausalVariable>,
    /// Directed edges: (from_variable, to_variable)
    pub structure: Vec<(String, String)>,
    pub equations: Vec<CausalEquation>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CausalModelMode {
    Standard,
    Equilibrium,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalVariable {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CausalEquation {
    pub variable: String,
    /// Right-hand side expression (may be a function call / literal)
    pub rhs: Expr,
    /// Optional LEARN FROM <collection>
    pub learn_from: Option<String>,
}

// ── ESTIMATE EFFECT / INTERVENE ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct EstimateEffectStmt {
    /// SELECT items before FROM INTERVENE ON
    pub projections: Vec<SelectItem>,
    pub model: String,
    pub set_vars: Vec<(String, Expr)>,
    pub predict: String,
    pub given: Option<Vec<(String, Expr)>>,
}

// ── COUNTERFACTUAL ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CounterfactualStmt {
    /// SELECT items before FROM COUNTERFACTUAL ON
    pub projections: Vec<SelectItem>,
    pub model: String,
    /// GIVEN observed_data = (subquery)  OR  GIVEN (k=v, …)
    pub given: Option<CounterfactualGiven>,
    pub had: Vec<(String, Expr)>,
    pub predict: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CounterfactualGiven {
    Subquery(Box<Statement>),
    Pairs(Vec<(String, Expr)>),
}

// ── DISCOVER CAUSAL STRUCTURE ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoverCausalStmt {
    pub collection: String,
    /// `'ensemble' | 'pc' | 'notears' | 'granger'`
    pub algorithm: Option<String>,
    pub min_confidence: Option<Expr>,
    pub store_as: Option<String>,
    /// Variables to analyze (from VARIABLES clause)
    pub variables: Vec<String>,
}

// ── REMEMBER ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct RememberStmt {
    pub content: String,
    pub agent_id: Expr,
    pub importance: Option<Expr>,
    pub memory_type: Option<String>,
}

// ── RECALL BY ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct RecallByStmt {
    pub weights: Vec<RecallWeight>,
    pub agent_id: Expr,
    pub limit: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecallWeight {
    SemanticSimilarity {
        query: Expr,
        weight: Option<Expr>,
    },
    Recency {
        decay: Option<String>,
        half_life: Option<String>,
        weight: Option<Expr>,
    },
    Importance {
        weight: Option<Expr>,
    },
}

// ── FORGET ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ForgetStmt {
    pub agent_id: Expr,
    pub filter: Option<Expr>,
}

// ── INSERT ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct InsertStmt {
    pub table: String,
    pub columns: Vec<String>,
    pub values: Vec<Vec<Expr>>,
}

// ── UPDATE ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStmt {
    pub table: String,
    pub alias: Option<String>,
    pub assignments: Vec<(String, Expr)>,
    pub where_clause: Option<Expr>,
}

// ── DELETE ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStmt {
    pub table: String,
    pub where_clause: Option<Expr>,
}

// ── CREATE COLLECTION ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CreateCollectionStmt {
    pub name: String,
    pub if_not_exists: bool,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub ty: String,
    pub nullable: bool,
}

// ── CREATE INDEX ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexStmt {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Ident(String),
    /// `:param_name`
    Param(String),
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
    /// `_vector('field') <-> value` — special syntax for vector distance
    VectorDist {
        field: String,
        value: Box<Expr>,
    },
    /// `expr IN (list)`
    InList {
        expr: Box<Expr>,
        list: Vec<Expr>,
    },
    /// `expr IS NULL`
    IsNull(Box<Expr>),
    /// `expr IS NOT NULL`
    IsNotNull(Box<Expr>),
    /// `expr BETWEEN low AND high`
    Between {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
    },
    /// `(subquery)` or scalar subquery
    Subquery(Box<Statement>),
    /// `*`
    Star,
    /// `[elem, elem, …]` vector literal
    Array(Vec<Expr>),
    /// `table.column`  or  `alias.field`
    Qualified {
        table: String,
        field: String,
    },
    /// `CASE WHEN … THEN … ELSE … END`
    Case {
        operand: Option<Box<Expr>>,
        when_clauses: Vec<(Expr, Expr)>,
        else_clause: Option<Box<Expr>>,
    },
    /// Cast: `CAST(expr AS type)` — represented as a function call
    Cast {
        expr: Box<Expr>,
        ty: String,
    },
    /// `NOT expr`
    Not(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Like,
    NotLike,
    In,
    NotIn,
    Between,
    Arrow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
    IsNull,
    IsNotNull,
}
