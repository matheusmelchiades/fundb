/// All tokens produced by the FunQL lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── Standard SQL keywords ──────────────────────────────────────────────
    Select,
    From,
    Where,
    Insert,
    Into,
    Values,
    Update,
    Set,
    Delete,
    Create,
    Drop,
    Table,
    Index,
    On,
    As,
    By,
    Order,
    Group,
    Having,
    Limit,
    Offset,
    Join,
    Inner,
    Left,
    Right,
    Outer,
    Cross,
    Union,
    All,
    Distinct,
    And,
    Or,
    Not,
    In,
    Is,
    Null,
    True,
    False,
    Asc,
    Desc,
    Between,
    Like,
    Exists,
    Case,
    When,
    Then,
    Else,
    End,
    With,
    Interval,
    Now,
    Explain,
    Value,

    // ── FunQL extensions — vectors ──────────────────────────────────────────
    /// `_vector` keyword
    Vector,

    // ── FunQL extensions — graph ────────────────────────────────────────────
    Traverse,
    Return,
    Depth,
    Start,
    Path,

    // ── FunQL extensions — temporal ─────────────────────────────────────────
    AsOf,
    SystemTime,
    ValidTime,
    TimeTravel,

    // ── FunQL extensions — confidence ───────────────────────────────────────
    Confidence,
    SourceCount,
    ContradictionCount,

    // ── FunQL extensions — causal ────────────────────────────────────────────
    Trace,
    Causality,
    MaxDepth,
    MinStrength,
    MinStability,
    EstimateEffect,
    Intervene,
    Counterfactual,
    Predict,
    Had,
    Given,
    Discover,
    CausalStructure,
    Algorithm,
    CreateCausalModel,
    ModeEquilibrium,
    Structure,
    Equations,
    Variables,
    /// `INFER` keyword (for infer causality)
    Infer,

    // ── FunQL extensions — context retrieval ─────────────────────────────────
    Within,
    Context,
    MaxTokens,
    Coherence,
    Diversity,
    IncludeContradictions,
    Priority,

    // ── FunQL extensions — semantic ───────────────────────────────────────────
    Understand,

    // ── FunQL extensions — agent memory ──────────────────────────────────────
    Remember,
    RecallBy,
    Recall,
    Forget,
    For,
    Agent,
    Memory,
    Importance,
    SemanticSimilarity,
    Recency,
    Decay,
    Collection,
    Consolidate,

    // ── Causal discovery ─────────────────────────────────────────────────────
    MinConfidence,
    StoreAs,
    Store,

    // ── Operators ────────────────────────────────────────────────────────────
    /// `->`
    Arrow,
    /// `<->`
    VectorDist,
    /// `..`
    DotDot,

    // ── Standard operators ───────────────────────────────────────────────────
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    /// `=`
    Assign,

    // ── Punctuation ──────────────────────────────────────────────────────────
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,

    // ── Literals ─────────────────────────────────────────────────────────────
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    /// `:param_name`
    Param(String),

    // ── Identifiers ──────────────────────────────────────────────────────────
    Ident(String),

    // ── Special ──────────────────────────────────────────────────────────────
    Eof,
    Newline,
}

impl Token {
    /// Map a lowercase keyword string to a keyword `Token`, or `None` if not a keyword.
    pub fn keyword(s: &str) -> Option<Token> {
        match s {
            // SQL
            "select"    => Some(Token::Select),
            "from"      => Some(Token::From),
            "where"     => Some(Token::Where),
            "insert"    => Some(Token::Insert),
            "into"      => Some(Token::Into),
            "values"    => Some(Token::Values),
            "update"    => Some(Token::Update),
            "set"       => Some(Token::Set),
            "delete"    => Some(Token::Delete),
            "create"    => Some(Token::Create),
            "drop"      => Some(Token::Drop),
            "table"     => Some(Token::Table),
            "index"     => Some(Token::Index),
            "on"        => Some(Token::On),
            "as"        => Some(Token::As),
            "by"        => Some(Token::By),
            "order"     => Some(Token::Order),
            "group"     => Some(Token::Group),
            "having"    => Some(Token::Having),
            "limit"     => Some(Token::Limit),
            "offset"    => Some(Token::Offset),
            "join"      => Some(Token::Join),
            "inner"     => Some(Token::Inner),
            "left"      => Some(Token::Left),
            "right"     => Some(Token::Right),
            "outer"     => Some(Token::Outer),
            "cross"     => Some(Token::Cross),
            "union"     => Some(Token::Union),
            "all"       => Some(Token::All),
            "distinct"  => Some(Token::Distinct),
            "and"       => Some(Token::And),
            "or"        => Some(Token::Or),
            "not"       => Some(Token::Not),
            "in"        => Some(Token::In),
            "is"        => Some(Token::Is),
            "null"      => Some(Token::Null),
            "true"      => Some(Token::True),
            "false"     => Some(Token::False),
            "asc"       => Some(Token::Asc),
            "desc"      => Some(Token::Desc),
            "between"   => Some(Token::Between),
            "like"      => Some(Token::Like),
            "exists"    => Some(Token::Exists),
            "case"      => Some(Token::Case),
            "when"      => Some(Token::When),
            "then"      => Some(Token::Then),
            "else"      => Some(Token::Else),
            "end"       => Some(Token::End),
            "with"      => Some(Token::With),
            "interval"  => Some(Token::Interval),
            "now"       => Some(Token::Now),
            "explain"   => Some(Token::Explain),

            // FunQL — vectors
            "_vector"   => Some(Token::Vector),

            // FunQL — graph
            "traverse"  => Some(Token::Traverse),
            "return"    => Some(Token::Return),
            "depth"     => Some(Token::Depth),
            "start"     => Some(Token::Start),
            "path"      => Some(Token::Path),

            // FunQL — temporal
            "as_of"         => Some(Token::AsOf),
            "systemtime"    => Some(Token::SystemTime),
            "validtime"     => Some(Token::ValidTime),

            // FunQL — confidence
            "confidence"            => Some(Token::Confidence),
            "source_count"          => Some(Token::SourceCount),
            "contradiction_count"   => Some(Token::ContradictionCount),

            // FunQL — causal
            "trace"             => Some(Token::Trace),
            "causality"         => Some(Token::Causality),
            "max_depth"         => Some(Token::MaxDepth),
            "min_strength"      => Some(Token::MinStrength),
            "min_stability"     => Some(Token::MinStability),
            "estimate_effect"   => Some(Token::EstimateEffect),
            "intervene"         => Some(Token::Intervene),
            "counterfactual"    => Some(Token::Counterfactual),
            "predict"           => Some(Token::Predict),
            "had"               => Some(Token::Had),
            "given"             => Some(Token::Given),
            "discover"          => Some(Token::Discover),
            "causal_structure"  => Some(Token::CausalStructure),
            "algorithm"         => Some(Token::Algorithm),
            "equilibrium"       => Some(Token::ModeEquilibrium),
            "structure"         => Some(Token::Structure),
            "equations"         => Some(Token::Equations),
            "variables"         => Some(Token::Variables),
            "infer"             => Some(Token::Infer),

            // FunQL — context
            "within"                    => Some(Token::Within),
            "context"                   => Some(Token::Context),
            "max_tokens"                => Some(Token::MaxTokens),
            "coherence"                 => Some(Token::Coherence),
            "diversity"                 => Some(Token::Diversity),
            "include_contradictions"    => Some(Token::IncludeContradictions),
            "priority"                  => Some(Token::Priority),

            // FunQL — semantic
            "understand"    => Some(Token::Understand),

            // FunQL — agent memory
            "remember"          => Some(Token::Remember),
            "recall"            => Some(Token::Recall),
            "forget"            => Some(Token::Forget),
            "for"               => Some(Token::For),
            "agent"             => Some(Token::Agent),
            "memory"            => Some(Token::Memory),
            "importance"        => Some(Token::Importance),
            "semantic_similarity" => Some(Token::SemanticSimilarity),
            "recency"           => Some(Token::Recency),
            "decay"             => Some(Token::Decay),
            "collection"        => Some(Token::Collection),
            "consolidate"       => Some(Token::Consolidate),

            // Causal discovery
            "min_confidence"    => Some(Token::MinConfidence),
            "store"             => Some(Token::Store),

            _ => None,
        }
    }
}
