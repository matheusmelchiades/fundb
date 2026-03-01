pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod logical_plan;
pub mod binder;

pub use ast::Statement;
pub use parser::ParseError;

pub use logical_plan::{
    AggExpr, AggFunc, BinaryOp, ContextOptions, Expr, Literal, LogicalPlan, SortExpr,
    UnaryOp, UnderstandOptions,
};
pub use binder::{bind, BindError, Catalog};

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
    use super::{parse, ParseError, bind, BindError, Catalog, LogicalPlan, Statement};
    use crate::ast::{self, AsOfClause, Expr, Statement as AstStatement, UnderstandOption};
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
        let non_eof: Vec<_> = toks.into_iter().filter(|t| !matches!(t, Token::Eof)).collect();
        assert_eq!(non_eof.len(), 3, "expected [Ident, VectorDist, Ident], got {:?}", non_eof);
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
        assert_eq!(toks[0], Token::FloatLiteral(3.14));
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
                assert!(s.trace_causality.is_some(), "expected TRACE CAUSALITY clause");
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
                let has_collection = u.options.iter().any(|o| {
                    matches!(o, UnderstandOption::InCollection(_))
                });
                let has_vector = u.options.iter().any(|o| {
                    matches!(o, UnderstandOption::UsingVector(_))
                });
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
}
