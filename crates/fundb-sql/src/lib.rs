pub mod ast;
pub mod binder;
pub mod lexer;
pub mod logical_plan;
pub mod parser;
pub mod token;

pub use ast::Statement;
pub use parser::ParseError;

pub use binder::{bind, BindError, Catalog};
pub use logical_plan::{
    AggExpr, AggFunc, BinaryOp, ContextOptions, Expr, Literal, LogicalPlan, SortExpr, UnaryOp,
    UnderstandOptions,
};

/// Parse a FunQL query string into a typed AST.
///
/// Returns `Err(ParseError)` on any lexical or syntactic error — never panics.
pub fn parse(input: &str) -> Result<Statement, ParseError> {
    let mut lex = lexer::Lexer::new(input);
    let tokens = lex.tokenize().map_err(|e| ParseError {
        message: e.message,
        line: e.line,
        col: e.col,
    })?;
    let mut parser = parser::Parser::new(tokens);
    parser.parse_statement()
}

#[cfg(test)]
mod tests {
    use super::{parse, Statement};
    use crate::ast::{AsOfClause, Expr, UnderstandOption};
    use crate::lexer::Lexer;
    use crate::token::Token;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn tokens_no_eof(input: &str) -> Vec<Token> {
        let mut lex = Lexer::new(input);
        lex.tokenize()
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .filter(|t| !matches!(t, Token::Eof))
            .collect()
    }

    // ── lexer unit tests ─────────────────────────────────────────────────────

    #[test]
    fn test_lexer_vector_dist_operator() {
        // <-> must be a single token, NOT three separate tokens
        let mut lexer = Lexer::new("a <-> b");
        let toks: Vec<Token> = lexer
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        assert!(
            toks.iter().any(|t| matches!(t, Token::VectorDist)),
            "expected VectorDist token in {:?}",
            toks
        );
        let non_eof: Vec<_> = toks
            .into_iter()
            .filter(|t| !matches!(t, Token::Eof))
            .collect();
        assert_eq!(
            non_eof.len(),
            3,
            "expected [Ident, VectorDist, Ident], got {:?}",
            non_eof
        );
    }

    #[test]
    fn test_lexer_arrow_is_single_token() {
        let toks = tokens_no_eof("a -> b");
        assert!(toks.iter().any(|t| matches!(t, Token::Arrow)));
    }

    #[test]
    fn test_lexer_dotdot_is_single_token() {
        let toks = tokens_no_eof("1..3");
        assert!(toks.iter().any(|t| matches!(t, Token::DotDot)));
        assert_eq!(toks[0], Token::IntLiteral(1));
        assert_eq!(toks[1], Token::DotDot);
        assert_eq!(toks[2], Token::IntLiteral(3));
    }

    #[test]
    fn test_lexer_param() {
        let toks = tokens_no_eof(":agent_id");
        assert_eq!(toks[0], Token::Param("agent_id".to_string()));
    }

    #[test]
    fn test_lexer_string_literal() {
        let toks = tokens_no_eof("'hello world'");
        assert_eq!(toks[0], Token::StringLiteral("hello world".to_string()));
    }

    #[test]
    fn test_lexer_float_literal() {
        let toks = tokens_no_eof("3.14");
        #[allow(clippy::approx_constant)]
        let expected = 3.14;
        assert_eq!(toks[0], Token::FloatLiteral(expected));
    }

    #[test]
    fn test_lexer_leading_dot_float() {
        let toks = tokens_no_eof(".5");
        assert_eq!(toks[0], Token::FloatLiteral(0.5));
    }

    #[test]
    fn test_lexer_keywords_case_insensitive() {
        let upper = tokens_no_eof("SELECT FROM WHERE");
        let lower = tokens_no_eof("select from where");
        assert_eq!(upper, lower);
        assert_eq!(upper[0], Token::Select);
        assert_eq!(upper[1], Token::From);
        assert_eq!(upper[2], Token::Where);
    }

    #[test]
    fn test_lexer_single_line_comment_skipped() {
        let toks = tokens_no_eof("SELECT -- this is ignored\nFROM");
        assert_eq!(toks, vec![Token::Select, Token::From]);
    }

    #[test]
    fn test_lexer_block_comment_skipped() {
        let toks = tokens_no_eof("SELECT /* ignore this */ FROM");
        assert_eq!(toks, vec![Token::Select, Token::From]);
    }

    // ── parser: standard SQL ─────────────────────────────────────────────────

    #[test]
    fn test_parse_simple_select() {
        let stmt = parse("SELECT * FROM users WHERE age > 25").unwrap();
        match stmt {
            Statement::Select(s) => {
                assert!(s.from.is_some());
                assert_eq!(s.from.unwrap().name, "users");
                assert!(s.where_clause.is_some());
            }
            other => panic!("expected Select, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_select_with_order_limit() {
        let sql = "SELECT * FROM users ORDER BY age DESC LIMIT 10";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Select(s) => {
                assert_eq!(s.order_by.len(), 1);
                assert!(!s.order_by[0].asc);
                assert!(s.limit.is_some());
            }
            _ => panic!("expected Select"),
        }
    }

    // ── parser: vector search ────────────────────────────────────────────────

    #[test]
    fn test_parse_vector_search() {
        let sql = "SELECT * FROM documents \
            WHERE _vector('content_embedding') <-> [0.1, 0.2] < 0.3 \
            ORDER BY _vector_distance ASC \
            LIMIT 10";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Select(s) => {
                assert!(s.where_clause.is_some(), "expected WHERE clause");
                assert!(s.limit.is_some());
            }
            other => panic!("expected Select, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_confidence_aware_query() {
        let sql = "SELECT title, abstract, _confidence, _sources \
            FROM research_papers \
            WHERE _vector('embedding') <-> :query < 0.5 \
              AND _confidence > 0.7 \
              AND _source_count >= 2 \
              AND _contradiction_count = 0 \
            ORDER BY _confidence * (1 - _vector_distance) DESC \
            LIMIT 10";
        parse(sql).unwrap();
    }

    // ── parser: graph traversal ──────────────────────────────────────────────

    #[test]
    fn test_parse_graph_traverse() {
        let sql = "SELECT * FROM entities \
            TRAVERSE follows(depth: 1..3) \
            WHERE start.type = 'user' \
            RETURN path";
        // The RETURN is parsed at the end
        parse(sql).unwrap();
    }

    // ── parser: temporal ─────────────────────────────────────────────────────

    #[test]
    fn test_parse_as_of_system_time() {
        let sql = "SELECT * FROM agent_memory \
            AS OF SYSTEM TIME '2025-06-01' \
            WHERE agent_id = 'agent-42'";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Select(s) => {
                assert!(s.as_of.is_some(), "expected AS OF clause");
                match s.as_of.unwrap() {
                    AsOfClause::SystemTime(_) => {}
                    other => panic!("expected SystemTime, got {:?}", other),
                }
            }
            _ => panic!("expected Select"),
        }
    }

    #[test]
    fn test_parse_as_of_valid_time_between() {
        let sql = "SELECT * FROM agent_memory \
            AS OF VALID TIME BETWEEN '2025-01-01' AND '2025-06-01' \
            WHERE agent_id = 'agent-42'";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Select(s) => {
                assert!(matches!(s.as_of, Some(AsOfClause::ValidTimeBetween(_, _))));
            }
            _ => panic!("expected Select"),
        }
    }

    // ── parser: TRACE CAUSALITY ──────────────────────────────────────────────

    #[test]
    fn test_parse_trace_causality() {
        let sql = "SELECT * FROM events \
            TRACE CAUSALITY FROM :event_a TO :event_b \
            MAX_DEPTH 5 \
            MIN_STRENGTH 0.3 \
            RETURN causal_path, total_strength";
        // This SELECT has a TRACE CAUSALITY clause
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Select(s) => {
                assert!(
                    s.trace_causality.is_some(),
                    "expected TRACE CAUSALITY clause"
                );
                let tc = s.trace_causality.unwrap();
                assert!(tc.from.is_some());
                assert!(tc.to.is_some());
                assert!(tc.max_depth.is_some());
                assert!(tc.min_strength.is_some());
            }
            _ => panic!("expected Select with trace causality"),
        }
    }

    // ── parser: WITHIN CONTEXT ───────────────────────────────────────────────

    #[test]
    fn test_parse_within_context() {
        let sql = "SELECT * FROM knowledge_base \
            WHERE _vector('embedding') <-> :query < 0.6 \
            WITHIN CONTEXT (max_tokens: 4000, coherence: 0.6, diversity: 0.3, include_contradictions: true)";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Select(s) => {
                assert!(s.within_context.is_some(), "expected WITHIN CONTEXT");
                let ctx = s.within_context.unwrap();
                assert!(ctx.max_tokens.is_some());
                assert!(ctx.coherence.is_some());
                assert!(ctx.diversity.is_some());
                assert_eq!(ctx.include_contradictions, Some(true));
            }
            _ => panic!("expected Select"),
        }
    }

    // ── parser: UNDERSTAND ───────────────────────────────────────────────────

    #[test]
    fn test_parse_understand() {
        let sql = "UNDERSTAND 'papers about attention mechanisms cited by Vaswani' \
            WITH confidence > 0.7 \
            WITHIN last 2 years \
            DEPTH 2";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Understand(u) => {
                assert_eq!(
                    u.intent,
                    "papers about attention mechanisms cited by Vaswani"
                );
                assert!(!u.options.is_empty());
            }
            other => panic!("expected Understand, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_understand_with_collection_and_vector() {
        let sql = "UNDERSTAND 'users who behave similarly to user-42' \
            IN COLLECTION users \
            USING VECTOR 'behavior_embedding' \
            WITH min_similarity 0.7";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Understand(u) => {
                assert_eq!(u.intent, "users who behave similarly to user-42");
                let has_collection = u
                    .options
                    .iter()
                    .any(|o| matches!(o, UnderstandOption::InCollection(_)));
                let has_vector = u
                    .options
                    .iter()
                    .any(|o| matches!(o, UnderstandOption::UsingVector(_)));
                assert!(has_collection, "expected InCollection option");
                assert!(has_vector, "expected UsingVector option");
            }
            _ => panic!("expected Understand"),
        }
    }

    // ── parser: ESTIMATE EFFECT / INTERVENE ─────────────────────────────────

    #[test]
    fn test_parse_estimate_effect() {
        // ARCHITECTURE §5.1 form: SELECT … FROM INTERVENE ON … SET … PREDICT …
        let sql = "SELECT expected_revenue FROM INTERVENE ON revenue_model \
            SET marketing_spend = 100000 \
            PREDICT revenue";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::EstimateEffect(e) => {
                assert_eq!(e.model, "revenue_model");
                assert_eq!(e.set_vars.len(), 1);
                assert_eq!(e.set_vars[0].0, "marketing_spend");
                assert_eq!(e.predict, "revenue");
            }
            _ => panic!("expected EstimateEffect"),
        }
    }

    // ── parser: COUNTERFACTUAL ───────────────────────────────────────────────

    #[test]
    fn test_parse_counterfactual() {
        let sql = "COUNTERFACTUAL ON revenue_model \
            GIVEN observed_data = (SELECT * FROM metrics WHERE month = '2025-01') \
            HAD marketing_spend = 50000 \
            PREDICT revenue";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Counterfactual(c) => {
                assert_eq!(c.model, "revenue_model");
                assert!(c.given.is_some());
                assert_eq!(c.had.len(), 1);
                assert_eq!(c.had[0].0, "marketing_spend");
                assert_eq!(c.predict, "revenue");
            }
            _ => panic!("expected Counterfactual"),
        }
    }

    // ── parser: REMEMBER ─────────────────────────────────────────────────────

    #[test]
    fn test_parse_remember() {
        let sql = "REMEMBER 'user prefers Python' FOR AGENT :agent_id \
            WITH importance 0.8 AS semantic";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Remember(r) => {
                assert_eq!(r.content, "user prefers Python");
                assert!(matches!(r.agent_id, Expr::Param(_)));
                assert!(r.importance.is_some());
                assert_eq!(r.memory_type, Some("semantic".to_string()));
            }
            other => panic!("expected Remember, got {:?}", other),
        }
    }

    // ── parser: RECALL BY ────────────────────────────────────────────────────

    #[test]
    fn test_parse_recall_by() {
        let sql = "RECALL BY semantic_similarity(:query, weight: 0.5) \
            + recency(weight: 0.3) \
            + importance(weight: 0.2) \
            FOR AGENT :agent_id \
            LIMIT 20";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::RecallBy(r) => {
                assert_eq!(r.weights.len(), 3);
                assert!(matches!(r.agent_id, Expr::Param(_)));
                assert!(r.limit.is_some());
            }
            other => panic!("expected RecallBy, got {:?}", other),
        }
    }

    // ── parser: DISCOVER CAUSAL ──────────────────────────────────────────────

    #[test]
    fn test_parse_discover_causal() {
        let sql = "DISCOVER CAUSAL STRUCTURE IN COLLECTION metrics \
            ALGORITHM 'ensemble' \
            MIN_CONFIDENCE 0.55 \
            STORE AS 'my_system'";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::DiscoverCausal(d) => {
                assert_eq!(d.collection, "metrics");
                assert_eq!(d.algorithm, Some("ensemble".to_string()));
                assert!(d.min_confidence.is_some());
                assert_eq!(d.store_as, Some("my_system".to_string()));
            }
            other => panic!("expected DiscoverCausal, got {:?}", other),
        }
    }

    // ── parser: error reporting ──────────────────────────────────────────────

    #[test]
    fn test_parse_invalid_returns_error_with_position() {
        let result = parse("SELECT FROM WHERE");
        // SELECT FROM WHERE: after SELECT the parser tries to parse a projection,
        // hits FROM which is not a valid projection start → parse error
        assert!(result.is_err(), "expected parse error");
        let err = result.unwrap_err();
        assert!(err.line > 0, "line should be > 0, got {}", err.line);
    }

    #[test]
    fn test_parse_empty_input_is_error() {
        assert!(parse("").is_err());
    }

    #[test]
    fn test_parse_unterminated_string_is_lex_error() {
        assert!(parse("SELECT 'unterminated").is_err());
    }

    // ── parser: hybrid query (ARCHITECTURE §5.1 full example) ────────────────

    #[test]
    fn test_parse_hybrid_vector_graph_confidence_temporal() {
        let sql = "SELECT d.title, d.summary, d._confidence \
            FROM documents d \
            WHERE d._vector('embedding') <-> :query_vector < 0.5 \
              AND d.category = 'research' \
              AND d._confidence > 0.6 \
            AS OF SYSTEM TIME '2025-06-01' \
            ORDER BY _vector_distance ASC \
            LIMIT 20";
        parse(sql).unwrap();
    }

    #[test]
    fn test_parse_time_series_aggregation() {
        let sql = "SELECT time_bucket('1 hour', ts) AS bucket, avg(value) AS avg_metric \
            FROM metrics \
            WHERE ts BETWEEN now() - interval '7 days' AND now() \
            GROUP BY bucket";
        parse(sql).unwrap();
    }

    // ── parser: INSERT / UPDATE / DELETE ────────────────────────────────────

    #[test]
    fn test_parse_insert() {
        let sql = "INSERT INTO users (name, age) VALUES ('Alice', 30)";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Insert(ins) => {
                assert_eq!(ins.table, "users");
                assert_eq!(ins.columns, vec!["name", "age"]);
                assert_eq!(ins.values.len(), 1);
            }
            _ => panic!("expected Insert"),
        }
    }

    #[test]
    fn test_parse_update() {
        let sql = "UPDATE users SET age = 31 WHERE name = 'Alice'";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Update(u) => {
                assert_eq!(u.table, "users");
                assert_eq!(u.assignments.len(), 1);
                assert!(u.where_clause.is_some());
            }
            _ => panic!("expected Update"),
        }
    }

    #[test]
    fn test_parse_delete() {
        let sql = "DELETE FROM users WHERE age < 18";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Delete(d) => {
                assert_eq!(d.table, "users");
                assert!(d.where_clause.is_some());
            }
            _ => panic!("expected Delete"),
        }
    }

    // ── parser: trace causality (top-level standalone) ───────────────────────

    #[test]
    fn test_parse_trace_causality_standalone() {
        // top-level form: TRACE CAUSALITY FROM :x TO :y …
        let sql = "SELECT * FROM events \
            TRACE CAUSALITY FROM :event_a TO :event_b \
            MAX_DEPTH 5 \
            MIN_STRENGTH 0.3 \
            RETURN causal_path, total_strength";
        parse(sql).unwrap();
    }

    // ── parser: WITHIN CONTEXT with priority ────────────────────────────────

    #[test]
    fn test_parse_within_context_full() {
        let sql = "SELECT * FROM knowledge_base \
            WHERE _vector('embedding') <-> :query < 0.6 \
            WITHIN CONTEXT ( \
                max_tokens: 4000, \
                coherence: 0.6, \
                diversity: 0.3, \
                include_contradictions: true \
            )";
        let stmt = parse(sql).unwrap();
        match stmt {
            Statement::Select(s) => {
                let ctx = s.within_context.unwrap();
                assert_eq!(ctx.include_contradictions, Some(true));
            }
            _ => panic!("expected Select"),
        }
    }

    // ── parser: vector literal in WHERE ─────────────────────────────────────

    #[test]
    fn test_parse_vector_array_literal() {
        let sql = "SELECT * FROM documents WHERE _vector('emb') <-> [0.1, 0.2, 0.3] < 0.5";
        parse(sql).unwrap();
    }

    // ── parser: no panics on arbitrary input (basic fuzz guard) ──────────────

    #[test]
    fn test_no_panic_on_random_tokens() {
        let inputs = vec![
            "SELECT",
            "FROM WHERE",
            "SELECT SELECT SELECT",
            "999 999 999",
            "TRACE",
            "UNDERSTAND",
            "RECALL BY",
            "CREATE CAUSAL MODEL",
            "!!! ???",
            "SELECT * FROM",
            "INSERT INTO",
        ];
        for input in inputs {
            // Must not panic — only return Ok or Err
            let _ = parse(input);
        }
    }

    // ── parser: ARCHITECTURE §5.1 standalone TRACE CAUSALITY (nested SELECT) ─

    #[test]
    fn test_parse_trace_causality_in_subquery() {
        let sql = "SELECT total_confidence \
            FROM ( \
                SELECT * FROM events \
                TRACE CAUSALITY FROM 'vaswani-attention-paper' TO 'transformer-adoption' \
                MAX_DEPTH 5 \
                MIN_STRENGTH 0.2 \
                RETURN causal_path, total_strength \
            )";
        parse(sql).unwrap();
    }

    // ── example file integration: every .funsql query must parse ─────────────

    #[test]
    fn test_all_example_queries_parse() {
        // ── 01-smoke-test.funsql ─────────────────────────────────────────
        let queries_01: &[&str] = &[
            "SELECT 1",
            "SELECT VERSION()",
            "SELECT 1",
            "SELECT 2",
            "SELECT 3",
        ];

        // ── 02-seed-data.funsql ──────────────────────────────────────────
        let queries_02: &[&str] = &[
            "CREATE COLLECTION users",
            "CREATE COLLECTION products",
            "CREATE COLLECTION orders",
            "CREATE COLLECTION reviews",
            "CREATE COLLECTION articles",
            "INSERT INTO users (name, email, role, created_at) VALUES ('Alice Silva', 'alice@example.com', 'admin', '2025-01-15T10:00:00Z')",
            "INSERT INTO users (name, email, role, created_at) VALUES ('Bob Santos', 'bob@example.com', 'user', '2025-02-20T14:30:00Z')",
            "INSERT INTO users (name, email, role, created_at) VALUES ('Carol Souza', 'carol@example.com', 'user', '2025-03-10T09:15:00Z')",
            "INSERT INTO users (name, email, role, created_at) VALUES ('Dave Oliveira', 'dave@example.com', 'moderator', '2025-04-05T16:45:00Z')",
            "INSERT INTO users (name, email, role, created_at) VALUES ('Eva Lima', 'eva@example.com', 'user', '2025-05-12T11:20:00Z')",
            "INSERT INTO products (name, category, price, description, _vector) VALUES ('Neural Keyboard', 'hardware', 299.99, 'Mechanical keyboard with AI-powered adaptive key mapping', [0.12, 0.85, 0.33, 0.67, 0.91, 0.22, 0.45, 0.78])",
            "INSERT INTO products (name, category, price, description, _vector) VALUES ('Quantum Mouse', 'hardware', 149.99, 'Precision mouse with predictive tracking', [0.88, 0.15, 0.42, 0.71, 0.29, 0.63, 0.51, 0.37])",
            "INSERT INTO products (name, category, price, description, _vector) VALUES ('Holo Monitor', 'display', 1299.99, '32-inch holographic display with eye tracking', [0.55, 0.92, 0.18, 0.44, 0.76, 0.31, 0.89, 0.60])",
            "INSERT INTO products (name, category, price, description, _vector) VALUES ('Cloud Dock', 'accessories', 89.99, 'Universal docking station with auto-config', [0.33, 0.47, 0.81, 0.19, 0.55, 0.72, 0.28, 0.94])",
            "INSERT INTO products (name, category, price, description, _vector) VALUES ('Smart Cable Kit', 'accessories', 39.99, 'Self-organizing cable management system', [0.21, 0.68, 0.53, 0.87, 0.14, 0.46, 0.79, 0.35])",
            "INSERT INTO orders (user_id, product_id, quantity, total, status, ordered_at) VALUES (1, 1, 1, 299.99, 'delivered', '2025-06-01T08:00:00Z')",
            "INSERT INTO orders (user_id, product_id, quantity, total, status, ordered_at) VALUES (2, 3, 1, 1299.99, 'shipped', '2025-06-15T12:00:00Z')",
            "INSERT INTO orders (user_id, product_id, quantity, total, status, ordered_at) VALUES (1, 2, 2, 299.98, 'delivered', '2025-06-20T15:30:00Z')",
            "INSERT INTO orders (user_id, product_id, quantity, total, status, ordered_at) VALUES (3, 5, 3, 119.97, 'pending', '2025-07-01T09:00:00Z')",
            "INSERT INTO orders (user_id, product_id, quantity, total, status, ordered_at) VALUES (4, 4, 1, 89.99, 'delivered', '2025-07-10T17:00:00Z')",
            "INSERT INTO reviews (user_id, product_id, rating, text, _confidence) VALUES (1, 1, 5, 'Best keyboard I have ever used. The AI mapping is incredible.', 0.95)",
            "INSERT INTO reviews (user_id, product_id, rating, text, _confidence) VALUES (2, 3, 4, 'Great display but takes a while to calibrate.', 0.82)",
            "INSERT INTO reviews (user_id, product_id, rating, text, _confidence) VALUES (3, 5, 3, 'Works fine, nothing special about the smart features.', 0.71)",
            "INSERT INTO reviews (user_id, product_id, rating, text, _confidence) VALUES (4, 4, 5, 'Plug and play, auto-detected everything instantly.', 0.93)",
            "INSERT INTO articles (title, body, author, published_at, _vector) VALUES ('Introduction to FunDB', 'FunDB is an AI-native database designed for the next generation of applications...', 'Alice Silva', '2025-08-01T10:00:00Z', [0.45, 0.78, 0.12, 0.93, 0.34, 0.67, 0.55, 0.21])",
            "INSERT INTO articles (title, body, author, published_at, _vector) VALUES ('Vector Search Deep Dive', 'Understanding how vector similarity search works under the hood...', 'Bob Santos', '2025-08-15T14:00:00Z', [0.82, 0.19, 0.64, 0.37, 0.91, 0.28, 0.73, 0.46])",
            "INSERT INTO articles (title, body, author, published_at, _vector) VALUES ('Graph Databases vs FunDB', 'Comparing traditional graph databases with FunDB integrated graph traversal...', 'Carol Souza', '2025-09-01T09:00:00Z', [0.56, 0.41, 0.88, 0.23, 0.69, 0.52, 0.17, 0.84])",
        ];

        // ── 03-vectors.funsql ────────────────────────────────────────────
        let queries_03: &[&str] = &[
            "SELECT name, price, description FROM products WHERE _vector <-> [0.10, 0.80, 0.30, 0.65, 0.90, 0.20, 0.40, 0.75] < 0.5 ORDER BY _vector <-> [0.10, 0.80, 0.30, 0.65, 0.90, 0.20, 0.40, 0.75] LIMIT 3",
            "SELECT title, author, published_at FROM articles WHERE _vector <-> [0.50, 0.75, 0.15, 0.90, 0.40, 0.60, 0.50, 0.25] < 0.8 ORDER BY _vector <-> [0.50, 0.75, 0.15, 0.90, 0.40, 0.60, 0.50, 0.25]",
            "SELECT name, category, price FROM products WHERE category = 'hardware' AND _vector <-> [0.12, 0.85, 0.33, 0.67, 0.91, 0.22, 0.45, 0.78] < 0.3 ORDER BY price ASC",
            "SELECT name, description FROM products ORDER BY _vector <-> [0.50, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50, 0.50] LIMIT 5",
            "SELECT title, body, _confidence FROM articles WHERE _vector <-> [0.82, 0.19, 0.64, 0.37, 0.91, 0.28, 0.73, 0.46] < 0.4 AND _confidence > 0.7",
        ];

        // ── 04-temporal.funsql ───────────────────────────────────────────
        let queries_04: &[&str] = &[
            "SELECT name, price FROM products AS OF SYSTEM TIME '2025-07-01T00:00:00Z'",
            "SELECT name, price AS current_price FROM products ORDER BY name",
            "SELECT name, price AS old_price FROM products AS OF SYSTEM TIME '2025-06-01T00:00:00Z' ORDER BY name",
            "SELECT name, email, role FROM users AS OF VALID TIME '2025-03-01T00:00:00Z'",
            "SELECT name, role, _valid_from, _valid_to FROM users WHERE email = 'alice@example.com' ORDER BY _valid_from ASC",
            "SELECT o.*, u.name AS user_name FROM orders o JOIN users u ON o.user_id = u.id WHERE o.ordered_at >= '2025-06-01T00:00:00Z' AND o.ordered_at < '2025-07-01T00:00:00Z' ORDER BY o.ordered_at",
            "SELECT * FROM products AS OF SYSTEM TIME '2025-06-15T00:00:00Z' ORDER BY name",
        ];

        // ── 05-graph.funsql ──────────────────────────────────────────────
        let queries_05: &[&str] = &[
            "TRAVERSE users -> orders -> products WHERE users.name = 'Alice Silva'",
            "TRAVERSE products -> orders -> users WHERE products.name = 'Neural Keyboard'",
            "TRAVERSE users -> orders -> products -> orders -> users WHERE users.name = 'Alice Silva' AND depth <= 3",
            "TRAVERSE users -> orders -> products -> reviews WHERE users.email = 'alice@example.com'",
            "TRAVERSE products -> orders -> users -> orders -> products WHERE products.name = 'Neural Keyboard' AND products.category = 'hardware' LIMIT 5",
            "SELECT target.name, COUNT(*) AS purchase_count FROM TRAVERSE users -> orders -> products AS target GROUP BY target.name ORDER BY purchase_count DESC LIMIT 10",
        ];

        // ── 06-causal-agents.funsql ──────────────────────────────────────
        let queries_06: &[&str] = &[
            "TRACE CAUSALITY FROM users WHERE name = 'Alice Silva' TO orders WHERE status = 'delivered'",
            "ESTIMATE EFFECT OF price ON quantity FROM products JOIN orders ON products.id = orders.product_id WHERE category = 'hardware'",
            "COUNTERFACTUAL SELECT SUM(total) FROM orders JOIN products ON orders.product_id = products.id WHERE products.name = 'Holo Monitor' INTERVENE SET price = 999.99",
            "DISCOVER CAUSAL STRUCTURE FROM orders JOIN products ON orders.product_id = products.id VARIABLES price, quantity, rating, total",
            "REMEMBER 'User Alice prefers hardware products under $300' WITHIN CONTEXT 'shopping_assistant'",
            "RECALL BY 'product recommendations for Alice' WITHIN CONTEXT 'shopping_assistant' WHERE _confidence > 0.5",
            "UNDERSTAND 'Show me the best reviewed products from last month'",
            "REMEMBER 'Neural Keyboard has 95% satisfaction rate' WITHIN CONTEXT 'product_analytics' WITH _confidence = 0.95",
            "RECALL BY 'customer preferences' WITHIN CONTEXT 'shopping_assistant' ORDER BY _confidence DESC LIMIT 10",
            "FORGET WITHIN CONTEXT 'shopping_assistant' WHERE _confidence < 0.3",
        ];

        // ── 07-full-demo.funsql ──────────────────────────────────────────
        let queries_07: &[&str] = &[
            "SELECT 1",
            "SELECT VERSION()",
            "SELECT name, email, role FROM users WHERE role = 'admin' ORDER BY name ASC LIMIT 10",
            "SELECT category, COUNT(*) AS total_products, AVG(price) AS avg_price, MIN(price) AS min_price, MAX(price) AS max_price FROM products GROUP BY category HAVING COUNT(*) > 1 ORDER BY avg_price DESC",
            "SELECT u.name AS customer, p.name AS product, o.quantity, o.total, o.status FROM orders o INNER JOIN users u ON o.user_id = u.id INNER JOIN products p ON o.product_id = p.id ORDER BY o.ordered_at DESC",
            "SELECT u.name, u.email, COUNT(o.id) AS order_count FROM users u LEFT JOIN orders o ON u.id = o.user_id GROUP BY u.name, u.email ORDER BY order_count DESC",
            "SELECT name, price, description FROM products WHERE _vector <-> [0.15, 0.82, 0.30, 0.70, 0.88, 0.25, 0.42, 0.80] < 0.5 ORDER BY _vector <-> [0.15, 0.82, 0.30, 0.70, 0.88, 0.25, 0.42, 0.80] LIMIT 3",
            "SELECT title, author, _confidence FROM articles WHERE _vector <-> [0.50, 0.70, 0.20, 0.85, 0.40, 0.55, 0.50, 0.30] < 0.6 AND _confidence > 0.7 ORDER BY _confidence DESC",
            "SELECT name, price FROM products AS OF SYSTEM TIME '2025-06-01T00:00:00Z'",
            "SELECT name, role FROM users AS OF VALID TIME '2025-04-01T00:00:00Z'",
            "TRAVERSE users -> orders -> products WHERE users.name = 'Alice Silva'",
            "TRAVERSE products -> orders -> users -> orders -> products WHERE products.name = 'Neural Keyboard' LIMIT 5",
            "TRACE CAUSALITY FROM users WHERE name = 'Bob Santos' TO orders WHERE total > 1000",
            "COUNTERFACTUAL SELECT COUNT(*) AS order_count FROM orders JOIN products ON orders.product_id = products.id WHERE products.name = 'Holo Monitor' INTERVENE SET price = 799.99",
            "REMEMBER 'Top seller in hardware category is Neural Keyboard with 5-star average' WITHIN CONTEXT 'sales_dashboard'",
            "RECALL BY 'what are our best selling products?' WITHIN CONTEXT 'sales_dashboard' WHERE _confidence > 0.6",
            "UNDERSTAND 'Show me customers who might churn based on recent activity'",
            "SELECT DISTINCT u.name, u.email FROM users u JOIN orders o ON u.id = o.user_id JOIN products p ON o.product_id = p.id WHERE p.price > (SELECT AVG(price) FROM products)",
            "SELECT p.name, p.category, p.price FROM products p WHERE NOT EXISTS (SELECT 1 FROM orders o WHERE o.product_id = p.id)",
            "SELECT u.name, o.ordered_at, o.total, SUM(o.total) AS running_total FROM orders o JOIN users u ON o.user_id = u.id ORDER BY u.name, o.ordered_at",
            "SELECT name, price, description FROM products WHERE category = :category AND price BETWEEN :min_price AND :max_price ORDER BY price ASC LIMIT :limit",
        ];

        let all_groups: &[(&str, &[&str])] = &[
            ("01-smoke-test.funsql", queries_01),
            ("02-seed-data.funsql", queries_02),
            ("03-vectors.funsql", queries_03),
            ("04-temporal.funsql", queries_04),
            ("05-graph.funsql", queries_05),
            ("06-causal-agents.funsql", queries_06),
            ("07-full-demo.funsql", queries_07),
        ];

        let mut total = 0;
        for (file, queries) in all_groups {
            for query in *queries {
                parse(query).unwrap_or_else(|e| {
                    panic!(
                        "failed to parse query from {}: {:?}\nQuery: {}",
                        file, e, query
                    )
                });
                total += 1;
            }
        }

        // Sanity check: we tested a meaningful number of queries
        assert!(
            total >= 70,
            "expected at least 70 example queries, but only tested {}",
            total
        );
    }
}
