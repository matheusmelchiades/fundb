use crate::ast::*;
use crate::lexer::Span;
use crate::token::Token;

/// Parse error produced by the recursive-descent parser.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: u32,
    pub col: u32,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ParseError at {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

// ── Parser ────────────────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<(Token, Span)>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<(Token, Span)>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ── token navigation ─────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].0
    }

    fn peek2(&self) -> Option<&Token> {
        self.tokens.get(self.pos + 1).map(|(t, _)| t)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].0.clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn current_span(&self) -> Span {
        self.tokens[self.pos].1
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.peek()
            )))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let span = self.current_span();
        match self.peek().clone() {
            Token::Ident(name) => {
                self.advance();
                Ok(name)
            }
            other => {
                if let Some(s) = keyword_as_ident(&other) {
                    self.advance();
                    Ok(s)
                } else {
                    Err(ParseError {
                        message: format!("expected identifier, found {:?}", other),
                        line: span.line,
                        col: span.col,
                    })
                }
            }
        }
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == tok {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Eat a keyword that may appear as either its keyword token or as Ident("keyword")
    fn eat_keyword_or_ident(&mut self, keyword: &str) -> bool {
        match self.peek() {
            Token::Ident(s) if s.eq_ignore_ascii_case(keyword) => {
                self.advance();
                true
            }
            _ => false,
        }
    }

    fn error(&self, msg: impl Into<String>) -> ParseError {
        let span = self.current_span();
        ParseError {
            message: msg.into(),
            line: span.line,
            col: span.col,
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    // ── top-level dispatch ───────────────────────────────────────────────────

    pub fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        while self.eat(&Token::Semicolon) {}

        let stmt = match self.peek().clone() {
            Token::Select   => self.parse_select_stmt()?,
            Token::Insert   => Statement::Insert(self.parse_insert()?),
            Token::Update   => Statement::Update(self.parse_update()?),
            Token::Delete   => Statement::Delete(self.parse_delete()?),
            Token::Create   => self.parse_create()?,
            Token::Understand => Statement::Understand(self.parse_understand()?),
            Token::Remember => Statement::Remember(self.parse_remember()?),
            Token::Recall   => Statement::RecallBy(self.parse_recall_by()?),
            Token::Forget   => Statement::Forget(self.parse_forget()?),
            Token::Discover => Statement::DiscoverCausal(self.parse_discover_causal()?),
            Token::Trace    => Statement::TraceCausality(self.parse_trace_causality_top()?),
            Token::Intervene => Statement::EstimateEffect(self.parse_intervene()?),
            Token::Counterfactual => self.parse_counterfactual_dispatch()?,
            Token::Traverse => Statement::Select(self.parse_traverse_stmt()?),
            // ESTIMATE EFFECT OF ... ON ... FROM ...
            Token::Ident(ref s) if s.eq_ignore_ascii_case("estimate") => {
                Statement::EstimateEffect(self.parse_estimate_effect_natural()?)
            }
            Token::Eof => {
                return Err(self.error("unexpected end of input"));
            }
            other => {
                return Err(self.error(format!("unexpected token {:?}", other)));
            }
        };

        self.eat(&Token::Semicolon);
        Ok(stmt)
    }

    // ── SELECT top-level dispatch ────────────────────────────────────────────

    fn parse_select_stmt(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::Select)?;
        let distinct = self.eat(&Token::Distinct);
        let projections = self.parse_projection_list()?;

        // FROM INTERVENE ON …  →  EstimateEffect
        if self.eat(&Token::From) {
            if self.peek() == &Token::Intervene {
                return Ok(Statement::EstimateEffect(
                    self.parse_intervene_after_from(projections)?,
                ));
            }
            if self.peek() == &Token::Counterfactual {
                return Ok(Statement::Counterfactual(
                    self.parse_counterfactual_after_from(projections)?,
                ));
            }
            // FROM TRAVERSE … — graph traversal in FROM position
            if self.peek() == &Token::Traverse {
                self.advance(); // consume TRAVERSE
                let first = self.expect_ident()?;
                let mut chain = vec![first.clone()];
                while self.eat(&Token::Arrow) {
                    chain.push(self.expect_ident()?);
                }
                // optional AS alias
                let target_alias = if self.eat(&Token::As) {
                    Some(self.expect_ident()?)
                } else {
                    None
                };
                let traverse = TraverseClause {
                    relation: first,
                    chain,
                    depth_min: None,
                    depth_max: None,
                    target_alias,
                };
                let mut stmt = self.finish_select(distinct, projections, None)?;
                stmt.traverse = Some(traverse);
                return Ok(Statement::Select(stmt));
            }
            // regular FROM
            let from = Some(self.parse_table_ref()?);
            return Ok(Statement::Select(
                self.finish_select(distinct, projections, from)?,
            ));
        }

        // SELECT without FROM
        Ok(Statement::Select(
            self.finish_select(distinct, projections, None)?,
        ))
    }

    fn finish_select(
        &mut self,
        distinct: bool,
        projections: Vec<SelectItem>,
        from: Option<TableRef>,
    ) -> Result<SelectStmt, ParseError> {
        let mut joins: Vec<JoinClause> = Vec::new();
        let mut where_clause: Option<Expr> = None;
        let mut group_by: Vec<Expr> = Vec::new();
        let mut having: Option<Expr> = None;
        let mut order_by: Vec<OrderByItem> = Vec::new();
        let mut limit: Option<Expr> = None;
        let mut offset: Option<Expr> = None;
        let mut within_context: Option<ContextOptions> = None;
        let mut as_of: Option<AsOfClause> = None;
        let mut traverse: Option<TraverseClause> = None;
        let mut trace_causality: Option<TraceCausalityClause> = None;
        let mut return_items: Vec<SelectItem> = Vec::new();
        let mut recall_by: Option<RecallByClause> = None;

        loop {
            match self.peek().clone() {
                Token::Join | Token::Inner | Token::Left | Token::Right | Token::Cross | Token::Outer => {
                    joins.push(self.parse_join()?);
                }
                Token::Traverse => {
                    traverse = Some(self.parse_traverse_clause()?);
                }
                Token::Trace => {
                    trace_causality = Some(self.parse_trace_causality_clause()?);
                }
                Token::Where => {
                    self.advance();
                    where_clause = Some(self.parse_expr()?);
                }
                Token::Recall => {
                    recall_by = Some(self.parse_recall_by_clause()?);
                }
                Token::Group => {
                    self.advance();
                    self.expect(&Token::By)?;
                    group_by = self.parse_expr_list()?;
                }
                Token::Having => {
                    self.advance();
                    having = Some(self.parse_expr()?);
                }
                Token::Order => {
                    self.advance();
                    self.expect(&Token::By)?;
                    order_by = self.parse_order_by_list()?;
                }
                Token::Limit => {
                    self.advance();
                    limit = Some(self.parse_expr()?);
                }
                Token::Offset => {
                    self.advance();
                    offset = Some(self.parse_expr()?);
                }
                Token::Within => {
                    within_context = Some(self.parse_within_context()?);
                }
                Token::AsOf => {
                    self.advance();
                    as_of = Some(self.parse_as_of_body()?);
                }
                Token::As => {
                    // AS OF … — "OF" would be an Ident
                    if matches!(self.peek2(), Some(Token::Ident(s)) if s.eq_ignore_ascii_case("of")) {
                        self.advance(); // AS
                        self.advance(); // OF (ident)
                        as_of = Some(self.parse_as_of_body()?);
                    } else {
                        break;
                    }
                }
                Token::Return => {
                    self.advance();
                    return_items = self.parse_projection_list()?;
                }
                Token::MaxDepth => {
                    if let Some(ref mut tc) = trace_causality {
                        self.advance();
                        tc.max_depth = Some(self.parse_expr()?);
                    } else {
                        break;
                    }
                }
                Token::MinStrength => {
                    if let Some(ref mut tc) = trace_causality {
                        self.advance();
                        tc.min_strength = Some(self.parse_expr()?);
                    } else {
                        break;
                    }
                }
                Token::MinStability => {
                    if let Some(ref mut tc) = trace_causality {
                        self.advance();
                        tc.min_stability = Some(self.parse_expr()?);
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(SelectStmt {
            distinct,
            projections,
            from,
            joins,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
            within_context,
            as_of,
            traverse,
            trace_causality,
            return_items,
            recall_by,
        })
    }

    fn parse_projection_list(&mut self) -> Result<Vec<SelectItem>, ParseError> {
        let mut items = vec![self.parse_select_item()?];
        while self.eat(&Token::Comma) {
            items.push(self.parse_select_item()?);
        }
        Ok(items)
    }

    fn parse_select_item(&mut self) -> Result<SelectItem, ParseError> {
        if self.eat(&Token::Star) {
            return Ok(SelectItem::Wildcard);
        }
        let expr = self.parse_expr()?;
        let alias = if self.eat(&Token::As) {
            Some(self.expect_ident()?)
        } else if matches!(self.peek(), Token::Ident(_)) && !self.is_clause_keyword(self.peek()) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        Ok(SelectItem::Expr { expr, alias })
    }

    fn parse_table_ref(&mut self) -> Result<TableRef, ParseError> {
        if self.eat(&Token::Agent) {
            self.expect(&Token::Memory)?;
            let name = self.expect_ident()?;
            return Ok(TableRef {
                name,
                alias: None,
                is_agent_memory: true,
            });
        }

        // Subquery in FROM: (SELECT ...) AS alias
        if self.peek() == &Token::LParen {
            self.advance();
            // parse the inner select, discarding the result (we just need to not error)
            let _sub = self.parse_select_inner()?;
            self.expect(&Token::RParen)?;
            let alias = if self.eat(&Token::As) {
                Some(self.expect_ident()?)
            } else if matches!(self.peek(), Token::Ident(_)) && !self.is_clause_keyword(self.peek()) {
                Some(self.expect_ident()?)
            } else {
                None
            };
            return Ok(TableRef {
                name: "<subquery>".to_string(),
                alias,
                is_agent_memory: false,
            });
        }

        let name = self.parse_qualified_name()?;
        // Consume AS as a table alias only when it is NOT followed by "of"
        // (which would indicate an AS OF temporal clause, not an alias).
        let alias = if matches!(self.peek(), Token::As)
            && !matches!(self.peek2(), Some(Token::Ident(s)) if s.eq_ignore_ascii_case("of"))
        {
            self.advance(); // eat As
            Some(self.expect_ident()?)
        } else if matches!(self.peek(), Token::Ident(_)) && !self.is_clause_keyword(self.peek()) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        Ok(TableRef { name, alias, is_agent_memory: false })
    }

    fn parse_qualified_name(&mut self) -> Result<String, ParseError> {
        let mut name = self.expect_ident()?;
        if self.eat(&Token::Dot) {
            let sub = self.expect_ident()?;
            name = format!("{}.{}", name, sub);
        }
        Ok(name)
    }

    fn is_clause_keyword(&self, tok: &Token) -> bool {
        matches!(
            tok,
            Token::Where
                | Token::Join | Token::Inner | Token::Left | Token::Right | Token::Cross | Token::Outer
                | Token::Order | Token::Group | Token::Having
                | Token::Limit | Token::Offset
                | Token::Within | Token::AsOf | Token::As
                | Token::Traverse | Token::Trace | Token::Return
                | Token::Recall
                | Token::Eof | Token::Semicolon
        )
    }

    fn parse_join(&mut self) -> Result<JoinClause, ParseError> {
        let join_type = match self.peek().clone() {
            Token::Inner => {
                self.advance();
                self.eat(&Token::Join);
                JoinType::Inner
            }
            Token::Left => {
                self.advance();
                self.eat(&Token::Outer);
                self.eat(&Token::Join);
                JoinType::Left
            }
            Token::Right => {
                self.advance();
                self.eat(&Token::Outer);
                self.eat(&Token::Join);
                JoinType::Right
            }
            Token::Cross => {
                self.advance();
                self.eat(&Token::Join);
                JoinType::Cross
            }
            Token::Outer => {
                self.advance();
                self.eat(&Token::Join);
                JoinType::Full
            }
            Token::Join => {
                self.advance();
                JoinType::Inner
            }
            other => {
                return Err(self.error(format!("expected JOIN keyword, found {:?}", other)));
            }
        };
        let table = self.parse_table_ref()?;
        let on = if self.eat(&Token::On) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(JoinClause { join_type, table, on })
    }

    // ── TRAVERSE ────────────────────────────────────────────────────────────

    /// Parse top-level: `TRAVERSE a -> b -> c [WHERE ...] [LIMIT ...]`
    fn parse_traverse_stmt(&mut self) -> Result<SelectStmt, ParseError> {
        self.expect(&Token::Traverse)?;
        let first = self.expect_ident()?;
        let mut chain = vec![first.clone()];
        while self.eat(&Token::Arrow) {
            chain.push(self.expect_ident()?);
        }

        let traverse = TraverseClause {
            relation: first,
            chain,
            depth_min: None,
            depth_max: None,
            target_alias: None,
        };

        let mut where_clause = None;
        let mut limit = None;
        let mut order_by = Vec::new();

        loop {
            match self.peek().clone() {
                Token::Where => {
                    self.advance();
                    where_clause = Some(self.parse_expr()?);
                }
                Token::Limit => {
                    self.advance();
                    limit = Some(self.parse_expr()?);
                }
                Token::Order => {
                    self.advance();
                    self.expect(&Token::By)?;
                    order_by = self.parse_order_by_list()?;
                }
                Token::And => {
                    // Allow `AND depth <= 3` as continuation of WHERE
                    if where_clause.is_some() {
                        self.advance();
                        let extra = self.parse_expr()?;
                        where_clause = Some(Expr::BinaryOp {
                            op: BinaryOp::And,
                            left: Box::new(where_clause.unwrap()),
                            right: Box::new(extra),
                        });
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(SelectStmt {
            distinct: false,
            projections: vec![SelectItem::Wildcard],
            from: None,
            joins: Vec::new(),
            where_clause,
            group_by: Vec::new(),
            having: None,
            order_by,
            limit,
            offset: None,
            within_context: None,
            as_of: None,
            traverse: Some(traverse),
            trace_causality: None,
            return_items: Vec::new(),
            recall_by: None,
        })
    }

    fn parse_traverse_clause(&mut self) -> Result<TraverseClause, ParseError> {
        self.expect(&Token::Traverse)?;
        let relation = self.expect_ident()?;

        let mut depth_min = None;
        let mut depth_max = None;

        if self.eat(&Token::LParen) {
            // optional "depth:" prefix
            if matches!(self.peek(), Token::Depth) {
                self.advance();
                self.eat(&Token::Colon);
            } else if matches!(self.peek(), Token::Ident(s) if s.eq_ignore_ascii_case("depth")) {
                self.advance();
                self.eat(&Token::Colon);
            }
            let low = self.parse_primary_expr()?;
            if self.eat(&Token::DotDot) {
                depth_min = Some(low);
                depth_max = Some(self.parse_primary_expr()?);
            } else {
                depth_min = Some(low.clone());
                depth_max = Some(low);
            }
            self.expect(&Token::RParen)?;
        }

        // Parse chain: relation -> ident -> ident ...
        let mut chain = vec![relation.clone()];
        while self.eat(&Token::Arrow) {
            chain.push(self.expect_ident()?);
        }

        let target_alias = None;

        Ok(TraverseClause { relation, chain, depth_min, depth_max, target_alias })
    }

    // ── TRACE CAUSALITY clause (inside SELECT) ───────────────────────────────

    fn parse_trace_causality_clause(&mut self) -> Result<TraceCausalityClause, ParseError> {
        self.expect(&Token::Trace)?;
        self.expect(&Token::Causality)?;

        let mut from = None;
        let mut to = None;
        let mut max_depth = None;
        let mut min_strength = None;
        let mut min_stability = None;

        loop {
            match self.peek().clone() {
                Token::From => {
                    self.advance();
                    from = Some(self.parse_primary_expr()?);
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("to") => {
                    self.advance();
                    to = Some(self.parse_primary_expr()?);
                }
                Token::MaxDepth => {
                    self.advance();
                    max_depth = Some(self.parse_expr()?);
                }
                Token::MinStrength => {
                    self.advance();
                    min_strength = Some(self.parse_expr()?);
                }
                Token::MinStability => {
                    self.advance();
                    min_stability = Some(self.parse_expr()?);
                }
                _ => break,
            }
        }

        Ok(TraceCausalityClause { from, to, max_depth, min_strength, min_stability })
    }

    // ── TRACE CAUSALITY (top-level) ──────────────────────────────────────────

    fn parse_trace_causality_top(&mut self) -> Result<TraceCausalityStmt, ParseError> {
        self.expect(&Token::Trace)?;
        self.expect(&Token::Causality)?;

        let mut from_expr = None;
        let mut to_expr = None;
        let mut max_depth = None;
        let mut min_strength = None;
        let mut min_stability = None;
        let mut return_fields = Vec::new();

        loop {
            match self.peek().clone() {
                Token::From => {
                    self.advance();
                    let base = self.parse_primary_expr()?;
                    // Optional WHERE condition after FROM <ident>
                    if self.eat(&Token::Where) {
                        let cond = self.parse_expr()?;
                        from_expr = Some(Expr::BinaryOp {
                            op: BinaryOp::And,
                            left: Box::new(base),
                            right: Box::new(cond),
                        });
                    } else {
                        from_expr = Some(base);
                    }
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("to") => {
                    self.advance();
                    let base = self.parse_primary_expr()?;
                    // Optional WHERE condition after TO <ident>
                    if self.eat(&Token::Where) {
                        let cond = self.parse_expr()?;
                        to_expr = Some(Expr::BinaryOp {
                            op: BinaryOp::And,
                            left: Box::new(base),
                            right: Box::new(cond),
                        });
                    } else {
                        to_expr = Some(base);
                    }
                }
                Token::MaxDepth => {
                    self.advance();
                    max_depth = Some(self.parse_expr()?);
                }
                Token::MinStrength => {
                    self.advance();
                    min_strength = Some(self.parse_expr()?);
                }
                Token::MinStability => {
                    self.advance();
                    min_stability = Some(self.parse_expr()?);
                }
                Token::Return => {
                    self.advance();
                    return_fields = self.parse_projection_list()?;
                }
                Token::Order => {
                    self.advance();
                    self.expect(&Token::By)?;
                    self.parse_order_by_list()?;
                }
                _ => break,
            }
        }

        let from_e = from_expr.ok_or_else(|| self.error("TRACE CAUSALITY requires FROM <expr>"))?;
        let to_e = to_expr.ok_or_else(|| self.error("TRACE CAUSALITY requires TO <expr>"))?;

        Ok(TraceCausalityStmt {
            collection: String::new(),
            from: from_e,
            to: to_e,
            max_depth,
            min_strength,
            min_stability,
            return_fields,
        })
    }

    // ── WITHIN CONTEXT ───────────────────────────────────────────────────────

    fn parse_within_context(&mut self) -> Result<ContextOptions, ParseError> {
        self.expect(&Token::Within)?;
        self.expect(&Token::Context)?;
        self.expect(&Token::LParen)?;

        let mut max_tokens = None;
        let mut coherence = None;
        let mut diversity = None;
        let mut include_contradictions = None;
        let mut priority = Vec::new();

        while !matches!(self.peek(), Token::RParen | Token::Eof) {
            let key = self.expect_ident()?;
            self.expect(&Token::Colon)?;

            match key.to_ascii_lowercase().as_str() {
                "max_tokens" => {
                    max_tokens = Some(self.parse_expr()?);
                }
                "coherence" => {
                    coherence = Some(self.parse_expr()?);
                }
                "diversity" => {
                    diversity = Some(self.parse_expr()?);
                }
                "include_contradictions" => {
                    include_contradictions = Some(match self.peek().clone() {
                        Token::True => { self.advance(); true }
                        Token::False => { self.advance(); false }
                        other => {
                            return Err(self.error(format!(
                                "expected true/false for include_contradictions, found {:?}", other
                            )));
                        }
                    });
                }
                "priority" => {
                    if self.eat(&Token::LBracket) {
                        priority = self.parse_order_by_list()?;
                        self.expect(&Token::RBracket)?;
                    } else {
                        priority = self.parse_order_by_list()?;
                    }
                }
                other => {
                    return Err(self.error(format!("unknown WITHIN CONTEXT option: {}", other)));
                }
            }
            self.eat(&Token::Comma);
        }

        self.expect(&Token::RParen)?;

        Ok(ContextOptions { max_tokens, coherence, diversity, include_contradictions, priority })
    }

    // ── AS OF ────────────────────────────────────────────────────────────────

    fn parse_as_of_body(&mut self) -> Result<AsOfClause, ParseError> {
        // Next token is SYSTEM, VALID, or legacy identifiers
        let first = self.peek().clone();
        match first {
            Token::SystemTime => {
                self.advance();
                let expr = self.parse_primary_expr()?;
                Ok(AsOfClause::SystemTime(expr))
            }
            Token::ValidTime => {
                self.advance();
                if self.eat(&Token::Between) {
                    let low = self.parse_primary_expr()?;
                    self.expect(&Token::And)?;
                    let high = self.parse_primary_expr()?;
                    Ok(AsOfClause::ValidTimeBetween(low, high))
                } else {
                    let expr = self.parse_primary_expr()?;
                    Ok(AsOfClause::ValidTimeAt(expr))
                }
            }
            Token::Ident(s) if s.eq_ignore_ascii_case("system") => {
                self.advance();
                // consume TIME if present as ident
                if matches!(self.peek(), Token::Ident(t) if t.eq_ignore_ascii_case("time")) {
                    self.advance();
                }
                let expr = self.parse_primary_expr()?;
                Ok(AsOfClause::SystemTime(expr))
            }
            Token::Ident(s) if s.eq_ignore_ascii_case("valid") => {
                self.advance();
                if matches!(self.peek(), Token::Ident(t) if t.eq_ignore_ascii_case("time")) {
                    self.advance();
                }
                if self.eat(&Token::Between) {
                    let low = self.parse_primary_expr()?;
                    self.expect(&Token::And)?;
                    let high = self.parse_primary_expr()?;
                    Ok(AsOfClause::ValidTimeBetween(low, high))
                } else {
                    let expr = self.parse_primary_expr()?;
                    Ok(AsOfClause::ValidTimeAt(expr))
                }
            }
            other => Err(self.error(format!(
                "expected SYSTEM TIME or VALID TIME after AS OF, found {:?}", other
            ))),
        }
    }

    // ── RECALL BY clause (inside SELECT) ────────────────────────────────────

    fn parse_recall_by_clause(&mut self) -> Result<RecallByClause, ParseError> {
        self.expect(&Token::Recall)?;
        self.expect(&Token::By)?;
        let weights = self.parse_recall_weights()?;
        Ok(RecallByClause { weights })
    }

    // ── INTERVENE / ESTIMATE EFFECT ──────────────────────────────────────────

    /// Parse: ESTIMATE EFFECT OF <ident> ON <ident> FROM <table> [JOIN ...] [WHERE ...]
    fn parse_estimate_effect_natural(&mut self) -> Result<EstimateEffectStmt, ParseError> {
        // consume "estimate"
        self.advance();
        // consume "effect" (ident)
        self.eat_keyword_or_ident("effect");
        // consume "of" (ident)
        self.eat_keyword_or_ident("of");
        let treatment = self.expect_ident()?;
        // consume ON
        self.expect(&Token::On)?;
        let outcome = self.expect_ident()?;

        // FROM <table_ref> [JOIN ...] [WHERE ...]
        let model = if self.eat(&Token::From) {
            let table = self.parse_table_ref()?;
            // Consume optional JOINs
            while matches!(self.peek(), Token::Join | Token::Inner | Token::Left | Token::Right | Token::Cross | Token::Outer) {
                let _join = self.parse_join()?;
            }
            // Consume optional WHERE
            if self.eat(&Token::Where) {
                let _filter = self.parse_expr()?;
            }
            table.name
        } else {
            String::new()
        };

        Ok(EstimateEffectStmt {
            projections: vec![SelectItem::Wildcard],
            model,
            set_vars: vec![(treatment, Expr::Literal(Literal::Int(1)))],
            predict: outcome,
            given: None,
        })
    }

    fn parse_intervene(&mut self) -> Result<EstimateEffectStmt, ParseError> {
        self.expect(&Token::Intervene)?;
        self.expect(&Token::On)?;
        let model = self.expect_ident()?;
        self.expect(&Token::Set)?;
        let set_vars = self.parse_assignment_list()?;
        self.expect(&Token::Predict)?;
        let predict = self.expect_ident()?;
        Ok(EstimateEffectStmt {
            projections: vec![SelectItem::Wildcard],
            model,
            set_vars,
            predict,
            given: None,
        })
    }

    fn parse_intervene_after_from(
        &mut self,
        projections: Vec<SelectItem>,
    ) -> Result<EstimateEffectStmt, ParseError> {
        self.expect(&Token::Intervene)?;
        self.expect(&Token::On)?;
        let model = self.expect_ident()?;
        self.expect(&Token::Set)?;
        let set_vars = self.parse_assignment_list()?;
        self.expect(&Token::Predict)?;
        let predict = self.expect_ident()?;
        Ok(EstimateEffectStmt { projections, model, set_vars, predict, given: None })
    }

    // ── COUNTERFACTUAL ────────────────────────────────────────────────────────

    /// Dispatch: COUNTERFACTUAL ON model ... | COUNTERFACTUAL SELECT ... INTERVENE SET ...
    fn parse_counterfactual_dispatch(&mut self) -> Result<Statement, ParseError> {
        // Peek past COUNTERFACTUAL to see what follows
        if matches!(self.peek2(), Some(Token::Select)) {
            Ok(Statement::Counterfactual(self.parse_counterfactual_with_select()?))
        } else {
            Ok(Statement::Counterfactual(self.parse_counterfactual()?))
        }
    }

    /// Parse: COUNTERFACTUAL SELECT ... FROM ... WHERE ... INTERVENE SET key = val;
    fn parse_counterfactual_with_select(&mut self) -> Result<CounterfactualStmt, ParseError> {
        self.expect(&Token::Counterfactual)?;
        let sub = self.parse_select_inner()?;

        // Parse INTERVENE SET assignments
        let mut had = Vec::new();
        if self.eat(&Token::Intervene) {
            self.expect(&Token::Set)?;
            had = self.parse_assignment_list()?;
        }

        let projections = sub.projections.clone();
        Ok(CounterfactualStmt {
            projections,
            model: String::new(),
            given: Some(CounterfactualGiven::Subquery(Box::new(Statement::Select(sub)))),
            had,
            predict: String::new(),
        })
    }

    fn parse_counterfactual(&mut self) -> Result<CounterfactualStmt, ParseError> {
        self.parse_counterfactual_body(vec![SelectItem::Wildcard])
    }

    fn parse_counterfactual_after_from(
        &mut self,
        projections: Vec<SelectItem>,
    ) -> Result<CounterfactualStmt, ParseError> {
        self.parse_counterfactual_body(projections)
    }

    fn parse_counterfactual_body(
        &mut self,
        projections: Vec<SelectItem>,
    ) -> Result<CounterfactualStmt, ParseError> {
        self.expect(&Token::Counterfactual)?;
        self.expect(&Token::On)?;
        let model = self.expect_ident()?;

        let mut given: Option<CounterfactualGiven> = None;
        let mut had = Vec::new();
        let mut predict = String::new();

        loop {
            match self.peek().clone() {
                Token::Given => {
                    self.advance();
                    let key = self.expect_ident()?;
                    self.eat(&Token::Assign);
                    self.eat(&Token::Eq);
                    if self.eat(&Token::LParen) {
                        let sub = self.parse_select_inner()?;
                        self.expect(&Token::RParen)?;
                        given = Some(CounterfactualGiven::Subquery(Box::new(
                            Statement::Select(sub),
                        )));
                    } else {
                        let val = self.parse_expr()?;
                        let mut pairs = vec![(key, val)];
                        while self.eat(&Token::Comma) {
                            let k = self.expect_ident()?;
                            self.eat(&Token::Assign);
                            self.eat(&Token::Eq);
                            let v = self.parse_expr()?;
                            pairs.push((k, v));
                        }
                        given = Some(CounterfactualGiven::Pairs(pairs));
                    }
                }
                Token::Had => {
                    self.advance();
                    let k = self.expect_ident()?;
                    self.eat(&Token::Assign);
                    self.eat(&Token::Eq);
                    let v = self.parse_expr()?;
                    had.push((k, v));
                    while self.eat(&Token::Comma) {
                        let k2 = self.expect_ident()?;
                        self.eat(&Token::Assign);
                        self.eat(&Token::Eq);
                        let v2 = self.parse_expr()?;
                        had.push((k2, v2));
                    }
                }
                Token::Predict => {
                    self.advance();
                    predict = self.expect_ident()?;
                }
                _ => break,
            }
        }

        Ok(CounterfactualStmt { projections, model, given, had, predict })
    }

    // ── DISCOVER CAUSAL ───────────────────────────────────────────────────────

    fn parse_discover_causal(&mut self) -> Result<DiscoverCausalStmt, ParseError> {
        self.expect(&Token::Discover)?;

        // eat "CAUSAL" — may be Token::Causality or Ident("causal")
        if self.peek() == &Token::Causality {
            self.advance();
        } else {
            self.eat_keyword_or_ident("causal");
        }

        // eat "STRUCTURE"
        if self.peek() == &Token::Structure {
            self.advance();
        } else {
            self.eat_keyword_or_ident("structure");
        }

        // Accept either IN [COLLECTION] name or FROM name [JOIN ...]
        let collection = if self.eat(&Token::In) {
            // eat optional "COLLECTION"
            if self.peek() == &Token::Collection {
                self.advance();
            } else {
                self.eat_keyword_or_ident("collection");
            }
            self.expect_ident()?
        } else if self.eat(&Token::From) {
            let table = self.parse_table_ref()?;
            // Consume optional JOINs
            while matches!(self.peek(), Token::Join | Token::Inner | Token::Left | Token::Right | Token::Cross | Token::Outer) {
                let _join = self.parse_join()?;
            }
            table.name
        } else {
            return Err(self.error("expected IN or FROM after DISCOVER CAUSAL STRUCTURE"));
        };

        let mut algorithm = None;
        let mut min_confidence = None;
        let mut store_as = None;
        let mut variables = Vec::new();

        loop {
            match self.peek().clone() {
                Token::Variables => {
                    self.advance();
                    // Parse comma-separated variable names
                    variables.push(self.expect_ident()?);
                    while self.eat(&Token::Comma) {
                        variables.push(self.expect_ident()?);
                    }
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("variables") => {
                    self.advance();
                    variables.push(self.expect_ident()?);
                    while self.eat(&Token::Comma) {
                        variables.push(self.expect_ident()?);
                    }
                }
                Token::Algorithm => {
                    self.advance();
                    algorithm = Some(self.expect_string_or_ident()?);
                }
                Token::MinConfidence => {
                    self.advance();
                    min_confidence = Some(self.parse_expr()?);
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("min_confidence") => {
                    self.advance();
                    min_confidence = Some(self.parse_expr()?);
                }
                Token::Store => {
                    self.advance();
                    // AS
                    if self.peek() == &Token::As {
                        self.advance();
                    } else {
                        self.eat_keyword_or_ident("as");
                    }
                    store_as = Some(self.expect_string_or_ident()?);
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("store") => {
                    self.advance();
                    if self.peek() == &Token::As {
                        self.advance();
                    } else {
                        self.eat_keyword_or_ident("as");
                    }
                    store_as = Some(self.expect_string_or_ident()?);
                }
                _ => break,
            }
        }

        Ok(DiscoverCausalStmt { collection, algorithm, min_confidence, store_as, variables })
    }

    fn expect_string_or_ident(&mut self) -> Result<String, ParseError> {
        match self.peek().clone() {
            Token::StringLiteral(s) => {
                self.advance();
                Ok(s)
            }
            _ => self.expect_ident(),
        }
    }

    // ── UNDERSTAND ───────────────────────────────────────────────────────────

    fn parse_understand(&mut self) -> Result<UnderstandStmt, ParseError> {
        self.expect(&Token::Understand)?;

        let intent = match self.peek().clone() {
            Token::StringLiteral(s) => {
                self.advance();
                s
            }
            other => {
                return Err(self.error(format!(
                    "UNDERSTAND expects a string literal, found {:?}", other
                )));
            }
        };

        let mut options = Vec::new();

        loop {
            match self.peek().clone() {
                Token::With => {
                    self.advance();
                    // peek at key to determine option type, then parse the full expression
                    let key_peek = match self.peek().clone() {
                        Token::Ident(s) => s.to_ascii_lowercase(),
                        Token::Confidence => "confidence".to_string(),
                        Token::MinConfidence => "min_confidence".to_string(),
                        other => {
                            return Err(self.error(format!(
                                "expected option name after WITH in UNDERSTAND, found {:?}", other
                            )));
                        }
                    };
                    match key_peek.as_str() {
                        "confidence" => {
                            // Parse `confidence > 0.7` as a full expression
                            let expr = self.parse_expr()?;
                            options.push(UnderstandOption::Confidence(expr));
                        }
                        "min_similarity" => {
                            self.advance(); // consume the key
                            let expr = self.parse_expr()?;
                            options.push(UnderstandOption::MinSimilarity(expr));
                        }
                        other => {
                            return Err(self.error(format!(
                                "unknown UNDERSTAND WITH option: {}", other
                            )));
                        }
                    }
                }
                Token::Within => {
                    self.advance();
                    let mut parts = Vec::new();
                    loop {
                        match self.peek().clone() {
                            Token::Depth | Token::In | Token::With | Token::Semicolon | Token::Eof => break,
                            Token::Ident(s) => { parts.push(s); self.advance(); }
                            Token::IntLiteral(n) => { parts.push(n.to_string()); self.advance(); }
                            Token::StringLiteral(s) => { parts.push(s); self.advance(); }
                            _ => break,
                        }
                    }
                    options.push(UnderstandOption::Within(parts.join(" ")));
                }
                Token::Depth => {
                    self.advance();
                    let expr = self.parse_expr()?;
                    options.push(UnderstandOption::Depth(expr));
                }
                Token::In => {
                    self.advance();
                    // eat COLLECTION
                    if self.peek() == &Token::Collection {
                        self.advance();
                    } else {
                        self.eat_keyword_or_ident("collection");
                    }
                    let name = self.expect_ident()?;
                    options.push(UnderstandOption::InCollection(name));
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("using") => {
                    self.advance();
                    // eat VECTOR
                    if self.peek() == &Token::Vector {
                        self.advance();
                    } else {
                        self.eat_keyword_or_ident("vector");
                    }
                    let field = self.expect_string_or_ident()?;
                    options.push(UnderstandOption::UsingVector(field));
                }
                _ => break,
            }
        }

        Ok(UnderstandStmt { intent, options })
    }

    // ── REMEMBER ─────────────────────────────────────────────────────────────

    fn parse_remember(&mut self) -> Result<RememberStmt, ParseError> {
        self.expect(&Token::Remember)?;

        let content = match self.peek().clone() {
            Token::StringLiteral(s) => { self.advance(); s }
            other => {
                return Err(self.error(format!(
                    "REMEMBER expects a string literal, found {:?}", other
                )));
            }
        };

        // Accept either FOR AGENT <expr> or WITHIN CONTEXT <expr>
        let agent_id = if self.eat(&Token::For) {
            self.expect(&Token::Agent)?;
            self.parse_primary_expr()?
        } else if self.eat(&Token::Within) {
            self.expect(&Token::Context)?;
            self.parse_primary_expr()?
        } else {
            return Err(self.error("expected FOR AGENT or WITHIN CONTEXT after REMEMBER string"));
        };

        let mut importance = None;
        let mut memory_type = None;

        loop {
            match self.peek().clone() {
                Token::With => {
                    self.advance();
                    let key = self.expect_ident()?;
                    match key.to_ascii_lowercase().as_str() {
                        "importance" => {
                            importance = Some(self.parse_primary_expr()?);
                        }
                        "_confidence" => {
                            // WITH _confidence = 0.95 — map to importance
                            self.eat(&Token::Assign);
                            self.eat(&Token::Eq);
                            importance = Some(self.parse_primary_expr()?);
                        }
                        other => {
                            return Err(self.error(format!(
                                "unknown REMEMBER WITH option: {}", other
                            )));
                        }
                    }
                }
                Token::Importance => {
                    self.advance();
                    importance = Some(self.parse_primary_expr()?);
                }
                Token::As => {
                    self.advance();
                    let ty = self.expect_ident()?;
                    memory_type = Some(ty);
                }
                _ => break,
            }
        }

        Ok(RememberStmt { content, agent_id, importance, memory_type })
    }

    // ── RECALL BY (top-level) ────────────────────────────────────────────────

    fn parse_recall_by(&mut self) -> Result<RecallByStmt, ParseError> {
        self.expect(&Token::Recall)?;
        self.expect(&Token::By)?;

        // String shorthand: RECALL BY 'query string' ...
        let weights = if matches!(self.peek(), Token::StringLiteral(_)) {
            let query = self.parse_primary_expr()?;
            vec![RecallWeight::SemanticSimilarity { query, weight: None }]
        } else {
            self.parse_recall_weights()?
        };

        // Accept either FOR AGENT <expr> or WITHIN CONTEXT <expr>
        let agent_id = if self.eat(&Token::For) {
            self.expect(&Token::Agent)?;
            self.parse_primary_expr()?
        } else if self.eat(&Token::Within) {
            self.expect(&Token::Context)?;
            self.parse_primary_expr()?
        } else {
            return Err(self.error("expected FOR AGENT or WITHIN CONTEXT after RECALL BY weights"));
        };

        let mut limit = None;

        // Optional WHERE, ORDER BY, LIMIT
        loop {
            match self.peek().clone() {
                Token::Where => {
                    self.advance();
                    // Parse and discard WHERE for now (binder doesn't use it)
                    let _filter = self.parse_expr()?;
                }
                Token::Order => {
                    self.advance();
                    self.expect(&Token::By)?;
                    let _order = self.parse_order_by_list()?;
                }
                Token::Limit => {
                    self.advance();
                    limit = Some(self.parse_expr()?);
                }
                _ => break,
            }
        }

        Ok(RecallByStmt { weights, agent_id, limit })
    }

    fn parse_recall_weights(&mut self) -> Result<Vec<RecallWeight>, ParseError> {
        let mut weights = vec![self.parse_recall_weight()?];
        while self.eat(&Token::Plus) {
            weights.push(self.parse_recall_weight()?);
        }
        Ok(weights)
    }

    fn parse_recall_weight(&mut self) -> Result<RecallWeight, ParseError> {
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;

        match name.to_ascii_lowercase().as_str() {
            "semantic_similarity" => {
                let query = self.parse_primary_expr()?;
                let mut w = None;
                if self.eat(&Token::Comma) {
                    let key = self.expect_ident()?;
                    if key.eq_ignore_ascii_case("weight") {
                        self.expect(&Token::Colon)?;
                        w = Some(self.parse_primary_expr()?);
                    }
                }
                self.expect(&Token::RParen)?;
                Ok(RecallWeight::SemanticSimilarity { query, weight: w })
            }
            "recency" => {
                let mut decay = None;
                let mut half_life = None;
                let mut w = None;
                while !matches!(self.peek(), Token::RParen | Token::Eof) {
                    let k = self.expect_ident()?;
                    self.expect(&Token::Colon)?;
                    match k.to_ascii_lowercase().as_str() {
                        "decay" => {
                            decay = Some(self.expect_string_or_ident()?);
                        }
                        "half_life" => {
                            half_life = Some(self.expect_string_or_ident()?);
                        }
                        "weight" => {
                            w = Some(self.parse_primary_expr()?);
                        }
                        _ => { self.parse_primary_expr()?; }
                    }
                    self.eat(&Token::Comma);
                }
                self.expect(&Token::RParen)?;
                Ok(RecallWeight::Recency { decay, half_life, weight: w })
            }
            "importance" => {
                let mut w = None;
                if !matches!(self.peek(), Token::RParen) {
                    let k = self.expect_ident()?;
                    if k.eq_ignore_ascii_case("weight") {
                        self.expect(&Token::Colon)?;
                        w = Some(self.parse_primary_expr()?);
                    }
                }
                self.expect(&Token::RParen)?;
                Ok(RecallWeight::Importance { weight: w })
            }
            other => Err(self.error(format!("unknown recall weight function: {}", other))),
        }
    }

    // ── FORGET ───────────────────────────────────────────────────────────────

    fn parse_forget(&mut self) -> Result<ForgetStmt, ParseError> {
        self.expect(&Token::Forget)?;

        // Accept either FOR AGENT <expr> or WITHIN CONTEXT <expr>
        let agent_id = if self.eat(&Token::For) {
            self.expect(&Token::Agent)?;
            self.parse_primary_expr()?
        } else if self.eat(&Token::Within) {
            self.expect(&Token::Context)?;
            self.parse_primary_expr()?
        } else {
            return Err(self.error("expected FOR AGENT or WITHIN CONTEXT after FORGET"));
        };

        let filter = if self.eat(&Token::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(ForgetStmt { agent_id, filter })
    }

    // ── INSERT ────────────────────────────────────────────────────────────────

    fn parse_insert(&mut self) -> Result<InsertStmt, ParseError> {
        self.expect(&Token::Insert)?;
        self.expect(&Token::Into)?;
        let table = self.parse_qualified_name()?;

        let mut columns = Vec::new();
        if self.eat(&Token::LParen) {
            columns.push(self.expect_ident()?);
            while self.eat(&Token::Comma) {
                columns.push(self.expect_ident()?);
            }
            self.expect(&Token::RParen)?;
        }

        self.expect(&Token::Values)?;
        let mut all_values = Vec::new();

        loop {
            self.expect(&Token::LParen)?;
            let mut row = vec![self.parse_expr()?];
            while self.eat(&Token::Comma) {
                row.push(self.parse_expr()?);
            }
            self.expect(&Token::RParen)?;
            all_values.push(row);
            if !self.eat(&Token::Comma) {
                break;
            }
        }

        Ok(InsertStmt { table, columns, values: all_values })
    }

    // ── UPDATE ────────────────────────────────────────────────────────────────

    fn parse_update(&mut self) -> Result<UpdateStmt, ParseError> {
        self.expect(&Token::Update)?;
        let table = self.parse_qualified_name()?;
        let alias = if self.eat(&Token::As) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect(&Token::Set)?;
        let assignments = self.parse_assignment_list()?;
        let where_clause = if self.eat(&Token::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(UpdateStmt { table, alias, assignments, where_clause })
    }

    fn parse_assignment_list(&mut self) -> Result<Vec<(String, Expr)>, ParseError> {
        let mut list = vec![self.parse_assignment()?];
        while self.eat(&Token::Comma) {
            list.push(self.parse_assignment()?);
        }
        Ok(list)
    }

    fn parse_assignment(&mut self) -> Result<(String, Expr), ParseError> {
        let name = self.expect_ident()?;
        if !self.eat(&Token::Assign) {
            self.eat(&Token::Eq);
        }
        let val = self.parse_expr()?;
        Ok((name, val))
    }

    // ── DELETE ────────────────────────────────────────────────────────────────

    fn parse_delete(&mut self) -> Result<DeleteStmt, ParseError> {
        self.expect(&Token::Delete)?;
        self.expect(&Token::From)?;
        let table = self.parse_qualified_name()?;
        let where_clause = if self.eat(&Token::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(DeleteStmt { table, where_clause })
    }

    // ── CREATE ────────────────────────────────────────────────────────────────

    fn parse_create(&mut self) -> Result<Statement, ParseError> {
        self.expect(&Token::Create)?;

        match self.peek().clone() {
            Token::Table => {
                self.advance();
                Ok(Statement::CreateCollection(self.parse_create_table_body()?))
            }
            Token::Collection => {
                self.advance();
                Ok(Statement::CreateCollection(self.parse_create_table_body()?))
            }
            Token::Index => {
                self.advance();
                Ok(Statement::CreateIndex(self.parse_create_index_body()?))
            }
            Token::Ident(s) if s.eq_ignore_ascii_case("causal") => {
                self.advance();
                // eat MODEL
                if matches!(self.peek(), Token::Ident(t) if t.eq_ignore_ascii_case("model")) {
                    self.advance();
                }
                Ok(Statement::CreateCausalModel(self.parse_create_causal_model_body()?))
            }
            other => Err(self.error(format!("unexpected CREATE target: {:?}", other))),
        }
    }

    fn parse_create_table_body(&mut self) -> Result<CreateCollectionStmt, ParseError> {
        let name = self.expect_ident()?;
        let mut columns = Vec::new();
        if self.eat(&Token::LParen) {
            while !matches!(self.peek(), Token::RParen | Token::Eof) {
                let col_name = self.expect_ident()?;
                let ty = self.expect_ident()?;
                let nullable = if self.peek() == &Token::Not {
                    self.advance();
                    self.expect(&Token::Null)?;
                    false
                } else {
                    true
                };
                columns.push(ColumnDef { name: col_name, ty, nullable });
                self.eat(&Token::Comma);
            }
            self.expect(&Token::RParen)?;
        }
        Ok(CreateCollectionStmt { name, if_not_exists: false, columns })
    }

    fn parse_create_index_body(&mut self) -> Result<CreateIndexStmt, ParseError> {
        let name = self.expect_ident()?;
        self.expect(&Token::On)?;
        let table = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let mut cols = vec![self.expect_ident()?];
        while self.eat(&Token::Comma) {
            cols.push(self.expect_ident()?);
        }
        self.expect(&Token::RParen)?;
        Ok(CreateIndexStmt { name, table, columns: cols, unique: false })
    }

    fn parse_create_causal_model_body(&mut self) -> Result<CreateCausalModelStmt, ParseError> {
        let name = self.expect_ident()?;
        let mode = if self.peek() == &Token::ModeEquilibrium {
            self.advance();
            CausalModelMode::Equilibrium
        } else {
            CausalModelMode::Standard
        };

        self.eat(&Token::As);
        self.expect(&Token::LParen)?;

        let mut variables = Vec::new();
        let mut structure = Vec::new();
        let mut equations = Vec::new();

        while !matches!(self.peek(), Token::RParen | Token::Eof) {
            let section = match self.peek().clone() {
                Token::Variables => { self.advance(); "variables" }
                Token::Structure => { self.advance(); "structure" }
                Token::Equations => { self.advance(); "equations" }
                Token::Ident(s) if s.eq_ignore_ascii_case("variables") => {
                    self.advance(); "variables"
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("structure") => {
                    self.advance(); "structure"
                }
                Token::Ident(s) if s.eq_ignore_ascii_case("equations") => {
                    self.advance(); "equations"
                }
                _ => {
                    // Skip unknown tokens gracefully
                    self.advance();
                    continue;
                }
            };
            self.eat(&Token::Colon);

            match section {
                "variables" => {
                    while !self.is_next_section_or_end() {
                        let vname = self.expect_ident()?;
                        let vty = self.expect_ident()?;
                        variables.push(CausalVariable { name: vname, ty: vty });
                        self.eat(&Token::Comma);
                    }
                }
                "structure" => {
                    while !self.is_next_section_or_end() {
                        let from = self.expect_ident()?;
                        self.expect(&Token::Arrow)?;
                        let to = self.expect_ident()?;
                        structure.push((from, to));
                        self.eat(&Token::Comma);
                    }
                }
                "equations" => {
                    while !matches!(self.peek(), Token::RParen | Token::Eof) {
                        if self.is_next_section_or_end() {
                            break;
                        }
                        let vname = self.expect_ident()?;
                        self.eat(&Token::Assign);
                        self.eat(&Token::Eq);
                        let rhs = self.parse_primary_expr()?;
                        let learn_from = if matches!(self.peek(), Token::Ident(s) if s.eq_ignore_ascii_case("learn")) {
                            self.advance();
                            if matches!(self.peek(), Token::From) {
                                self.advance();
                            } else if matches!(self.peek(), Token::Ident(s) if s.eq_ignore_ascii_case("from")) {
                                self.advance();
                            }
                            Some(self.expect_ident()?)
                        } else {
                            None
                        };
                        equations.push(CausalEquation { variable: vname, rhs, learn_from });
                        self.eat(&Token::Comma);
                    }
                }
                _ => {}
            }
        }

        self.expect(&Token::RParen)?;

        Ok(CreateCausalModelStmt { name, mode, variables, structure, equations })
    }

    fn is_next_section_or_end(&self) -> bool {
        matches!(
            self.peek(),
            Token::Variables | Token::Structure | Token::Equations | Token::RParen | Token::Eof
        ) || matches!(self.peek(), Token::Ident(s) if {
            s.eq_ignore_ascii_case("variables")
                || s.eq_ignore_ascii_case("structure")
                || s.eq_ignore_ascii_case("equations")
        })
    }

    // ── SELECT inner (for subqueries) ────────────────────────────────────────

    fn parse_select_inner(&mut self) -> Result<SelectStmt, ParseError> {
        self.expect(&Token::Select)?;
        let distinct = self.eat(&Token::Distinct);
        let projections = self.parse_projection_list()?;
        let from = if self.eat(&Token::From) {
            Some(self.parse_table_ref()?)
        } else {
            None
        };
        self.finish_select(distinct, projections, from)
    }

    // ── order by ────────────────────────────────────────────────────────────

    fn parse_order_by_list(&mut self) -> Result<Vec<OrderByItem>, ParseError> {
        let mut items = vec![self.parse_order_by_item()?];
        while self.eat(&Token::Comma) {
            // stop at RBracket (when used inside priority: [...])
            if matches!(self.peek(), Token::RBracket) {
                break;
            }
            items.push(self.parse_order_by_item()?);
        }
        Ok(items)
    }

    fn parse_order_by_item(&mut self) -> Result<OrderByItem, ParseError> {
        let expr = self.parse_expr()?;
        let asc = if self.eat(&Token::Desc) {
            false
        } else {
            self.eat(&Token::Asc);
            true
        };
        Ok(OrderByItem { expr, asc })
    }

    // ── expression parsing ───────────────────────────────────────────────────

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut items = vec![self.parse_expr()?];
        while self.eat(&Token::Comma) {
            items.push(self.parse_expr()?);
        }
        Ok(items)
    }

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and_expr()?;
        while self.eat(&Token::Or) {
            let right = self.parse_and_expr()?;
            left = Expr::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not_expr()?;
        while self.eat(&Token::And) {
            let right = self.parse_not_expr()?;
            left = Expr::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_not_expr(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&Token::Not) {
            let expr = self.parse_not_expr()?;
            return Ok(Expr::Not(Box::new(expr)));
        }
        self.parse_comparison_expr()
    }

    pub fn parse_comparison_expr(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_additive_expr()?;

        let op = match self.peek().clone() {
            Token::Eq | Token::Assign => {
                self.advance();
                BinaryOp::Eq
            }
            Token::NotEq => { self.advance(); BinaryOp::NotEq }
            Token::Lt => { self.advance(); BinaryOp::Lt }
            Token::LtEq => { self.advance(); BinaryOp::LtEq }
            Token::Gt => { self.advance(); BinaryOp::Gt }
            Token::GtEq => { self.advance(); BinaryOp::GtEq }
            Token::Like => { self.advance(); BinaryOp::Like }
            Token::Not => {
                self.advance();
                if self.eat(&Token::Like) {
                    BinaryOp::NotLike
                } else if self.eat(&Token::In) {
                    let list = self.parse_in_list()?;
                    return Ok(Expr::InList { expr: Box::new(left), list });
                } else {
                    return Err(self.error("expected LIKE or IN after NOT"));
                }
            }
            Token::Is => {
                self.advance();
                if self.eat(&Token::Not) {
                    self.expect(&Token::Null)?;
                    return Ok(Expr::IsNotNull(Box::new(left)));
                } else {
                    self.expect(&Token::Null)?;
                    return Ok(Expr::IsNull(Box::new(left)));
                }
            }
            Token::In => {
                self.advance();
                let list = self.parse_in_list()?;
                return Ok(Expr::InList { expr: Box::new(left), list });
            }
            Token::Between => {
                self.advance();
                let low = self.parse_additive_expr()?;
                self.expect(&Token::And)?;
                let high = self.parse_additive_expr()?;
                return Ok(Expr::Between {
                    expr: Box::new(left),
                    low: Box::new(low),
                    high: Box::new(high),
                });
            }
            _ => return Ok(left),
        };

        let right = self.parse_additive_expr()?;
        Ok(Expr::BinaryOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_additive_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative_expr()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative_expr()?;
            left = Expr::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_multiplicative_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_vector_dist_expr()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                Token::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_vector_dist_expr()?;
            left = Expr::BinaryOp { op, left: Box::new(left), right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_vector_dist_expr(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_unary_expr()?;
        if self.eat(&Token::VectorDist) {
            let value = self.parse_unary_expr()?;
            return Ok(Expr::BinaryOp {
                op: BinaryOp::Arrow,
                left: Box::new(expr),
                right: Box::new(value),
            });
        }
        Ok(expr)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&Token::Minus) {
            let expr = self.parse_primary_expr()?;
            return Ok(Expr::UnaryOp { op: UnaryOp::Neg, expr: Box::new(expr) });
        }
        self.parse_primary_expr()
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::IntLiteral(n) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int(n)))
            }
            Token::FloatLiteral(f) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(f)))
            }
            Token::StringLiteral(s) => {
                self.advance();
                Ok(Expr::Literal(Literal::String(s)))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            Token::Null => {
                self.advance();
                Ok(Expr::Literal(Literal::Null))
            }
            Token::Param(name) => {
                self.advance();
                Ok(Expr::Param(name))
            }
            Token::Star => {
                self.advance();
                Ok(Expr::Star)
            }

            // _vector('field') <-> value  OR  bare _vector <-> value
            Token::Vector => {
                self.advance();
                // Bare _vector <-> [...] syntax (no parenthesized field)
                if self.peek() == &Token::VectorDist {
                    self.advance(); // consume <->
                    let value = self.parse_primary_expr()?;
                    return Ok(Expr::VectorDist {
                        field: "_default".to_string(),
                        value: Box::new(value),
                    });
                }
                self.expect(&Token::LParen)?;
                let field = self.expect_string_or_ident()?;
                self.expect(&Token::RParen)?;
                if self.eat(&Token::VectorDist) {
                    let value = self.parse_primary_expr()?;
                    return Ok(Expr::VectorDist { field, value: Box::new(value) });
                }
                Ok(Expr::FunctionCall {
                    name: "_vector".to_string(),
                    args: vec![Expr::Literal(Literal::String(field))],
                })
            }

            // subquery or grouped expr
            Token::LParen => {
                self.advance();
                if matches!(self.peek(), Token::Select) {
                    let sub = self.parse_select_inner()?;
                    self.expect(&Token::RParen)?;
                    return Ok(Expr::Subquery(Box::new(Statement::Select(sub))));
                }
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }

            // array literal [0.1, 0.2, ...]
            Token::LBracket => {
                self.advance();
                if self.eat(&Token::RBracket) {
                    return Ok(Expr::Array(vec![]));
                }
                let mut elems = vec![self.parse_expr()?];
                while self.eat(&Token::Comma) {
                    if matches!(self.peek(), Token::RBracket) {
                        break;
                    }
                    elems.push(self.parse_expr()?);
                }
                self.expect(&Token::RBracket)?;
                Ok(Expr::Array(elems))
            }

            // EXISTS (SELECT ...) — subquery predicate
            Token::Exists => {
                self.advance();
                self.expect(&Token::LParen)?;
                let sub = self.parse_select_inner()?;
                self.expect(&Token::RParen)?;
                Ok(Expr::Subquery(Box::new(Statement::Select(sub))))
            }

            Token::Case => self.parse_case_expr(),

            // INTERVAL literal: `interval '7 days'`
            Token::Interval => {
                self.advance();
                match self.peek().clone() {
                    Token::StringLiteral(s) => {
                        self.advance();
                        Ok(Expr::FunctionCall {
                            name: "interval".to_string(),
                            args: vec![Expr::Literal(Literal::String(s))],
                        })
                    }
                    _ => Ok(Expr::Ident("interval".to_string())),
                }
            }

            // Identifiers and keywords-as-identifiers
            tok => {
                if let Some(name) = keyword_as_ident(&tok) {
                    self.advance();
                    // function call?
                    if self.eat(&Token::LParen) {
                        let mut args = Vec::new();
                        if !matches!(self.peek(), Token::RParen) {
                            args.push(self.parse_expr()?);
                            while self.eat(&Token::Comma) {
                                if matches!(self.peek(), Token::RParen) {
                                    break;
                                }
                                // handle named args like `key: value`
                                if matches!(self.peek(), Token::Ident(_)) && matches!(self.peek2(), Some(Token::Colon)) {
                                    // key: value pair — consume both as an expression pair
                                    let _key = self.expect_ident()?;
                                    self.expect(&Token::Colon)?;
                                    args.push(self.parse_expr()?);
                                } else {
                                    args.push(self.parse_expr()?);
                                }
                            }
                        }
                        self.expect(&Token::RParen)?;
                        return Ok(Expr::FunctionCall { name, args });
                    }
                    // qualified name (table.field) or table._vector('field') <-> value
                    if self.eat(&Token::Dot) {
                        // table.* — qualified wildcard
                        if self.peek() == &Token::Star {
                            self.advance();
                            return Ok(Expr::Qualified { table: name, field: "*".to_string() });
                        }
                        // Check if next is _vector keyword
                        if self.peek() == &Token::Vector {
                            self.advance(); // consume _vector
                            // Bare table._vector <-> [...] syntax
                            if self.peek() == &Token::VectorDist {
                                self.advance();
                                let value = self.parse_primary_expr()?;
                                return Ok(Expr::VectorDist {
                                    field: "_default".to_string(),
                                    value: Box::new(value),
                                });
                            }
                            self.expect(&Token::LParen)?;
                            let field = self.expect_string_or_ident()?;
                            self.expect(&Token::RParen)?;
                            // Check for <->
                            if self.eat(&Token::VectorDist) {
                                let value = self.parse_primary_expr()?;
                                return Ok(Expr::VectorDist {
                                    field,
                                    value: Box::new(value),
                                });
                            }
                            return Ok(Expr::FunctionCall {
                                name: "_vector".to_string(),
                                args: vec![Expr::Literal(Literal::String(field))],
                            });
                        }
                        let field = self.expect_ident()?;
                        // Check for method calls: table.field(args)
                        if self.eat(&Token::LParen) {
                            let mut args = Vec::new();
                            if !matches!(self.peek(), Token::RParen) {
                                args.push(self.parse_expr()?);
                                while self.eat(&Token::Comma) {
                                    if matches!(self.peek(), Token::RParen) { break; }
                                    args.push(self.parse_expr()?);
                                }
                            }
                            self.expect(&Token::RParen)?;
                            return Ok(Expr::FunctionCall {
                                name: format!("{}.{}", name, field),
                                args,
                            });
                        }
                        return Ok(Expr::Qualified { table: name, field });
                    }
                    Ok(Expr::Ident(name))
                } else {
                    Err(self.error(format!("unexpected token in expression: {:?}", tok)))
                }
            }
        }
    }

    fn parse_case_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect(&Token::Case)?;
        let operand = if !matches!(self.peek(), Token::When) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let mut when_clauses = Vec::new();
        while self.eat(&Token::When) {
            let cond = self.parse_expr()?;
            self.expect(&Token::Then)?;
            let result = self.parse_expr()?;
            when_clauses.push((cond, result));
        }
        let else_clause = if self.eat(&Token::Else) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect(&Token::End)?;
        Ok(Expr::Case { operand, when_clauses, else_clause })
    }

    fn parse_in_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        self.expect(&Token::LParen)?;
        let mut list = vec![self.parse_expr()?];
        while self.eat(&Token::Comma) {
            list.push(self.parse_expr()?);
        }
        self.expect(&Token::RParen)?;
        Ok(list)
    }
}

// ── free function: keyword → ident string ────────────────────────────────────

/// Returns Some(name) if `tok` is a keyword that can double as an identifier.
fn keyword_as_ident(tok: &Token) -> Option<String> {
    match tok {
        Token::Ident(s) => Some(s.clone()),
        Token::Algorithm   => Some("algorithm".into()),
        Token::Structure   => Some("structure".into()),
        Token::Equations   => Some("equations".into()),
        Token::Variables   => Some("variables".into()),
        Token::Confidence  => Some("confidence".into()),
        Token::Depth       => Some("depth".into()),
        Token::Path        => Some("path".into()),
        Token::Causality   => Some("causality".into()),
        Token::Recency     => Some("recency".into()),
        Token::Importance  => Some("importance".into()),
        Token::Memory      => Some("memory".into()),
        Token::Agent       => Some("agent".into()),
        Token::Priority    => Some("priority".into()),
        Token::Coherence   => Some("coherence".into()),
        Token::Diversity   => Some("diversity".into()),
        Token::Context     => Some("context".into()),
        Token::Collection  => Some("collection".into()),
        Token::Start       => Some("start".into()),
        Token::Return      => Some("return".into()),
        Token::Predict     => Some("predict".into()),
        Token::Now         => Some("now".into()),
        Token::Interval    => Some("interval".into()),
        Token::Explain     => Some("explain".into()),
        Token::Set         => Some("set".into()),
        Token::Value       => Some("value".into()),
        Token::MinConfidence => Some("min_confidence".into()),
        Token::Store       => Some("store".into()),
        Token::Decay       => Some("decay".into()),
        Token::SemanticSimilarity => Some("semantic_similarity".into()),
        Token::Consolidate => Some("consolidate".into()),
        Token::Infer       => Some("infer".into()),
        Token::StoreAs     => Some("store_as".into()),
        Token::Vector      => Some("_vector".into()),
        Token::Traverse    => Some("traverse".into()),
        Token::Trace       => Some("trace".into()),
        Token::With        => Some("with".into()),
        Token::MaxTokens   => Some("max_tokens".into()),
        Token::IncludeContradictions => Some("include_contradictions".into()),
        _ => None,
    }
}
