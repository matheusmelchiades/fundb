use crate::token::Token;

/// Source location for a token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub len: usize,
}

/// Error produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub line: u32,
    pub col: u32,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LexError at {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for LexError {}

/// Hand-written, zero-copy lexer for FunQL.
///
/// Key invariants:
/// - Keywords are case-insensitive (`SELECT` == `select`)
/// - `<->` is a single `VectorDist` token
/// - `->` is a single `Arrow` token
/// - `..` is a single `DotDot` token
/// - `:param_name` is `Param("param_name")`
/// - Single-quoted strings are `StringLiteral`
/// - `--` and `/* */` comments are skipped
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    line: u32,
    col: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input,
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Tokenise the entire input, returning a vector of `(Token, Span)` pairs.
    /// The final token in the vector is always `Token::Eof`.
    pub fn tokenize(&mut self) -> Result<Vec<(Token, Span)>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments()?;
            if self.pos >= self.input.len() {
                tokens.push((Token::Eof, self.span(0)));
                break;
            }
            let (tok, span) = self.next_token()?;
            // Swallow newlines; they are not significant in FunQL
            match tok {
                Token::Newline => {}
                _ => tokens.push((tok, span)),
            }
        }
        Ok(tokens)
    }

    // ── internal helpers ────────────────────────────────────────────────────

    fn current_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn peek_char(&self, offset: usize) -> Option<char> {
        let pos = self.pos + offset;
        if pos >= self.input.len() {
            return None;
        }
        self.input[pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.current_char()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn span(&self, len: usize) -> Span {
        Span {
            line: self.line,
            col: self.col,
            len,
        }
    }

    fn span_at(&self, line: u32, col: u32, len: usize) -> Span {
        Span { line, col, len }
    }

    fn lex_error(&self, msg: impl Into<String>) -> LexError {
        LexError {
            message: msg.into(),
            line: self.line,
            col: self.col,
        }
    }

    // ── comment / whitespace skipping ───────────────────────────────────────

    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            // skip plain whitespace (but not newlines — we produce Newline tokens)
            while let Some(ch) = self.current_char() {
                if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' {
                    self.advance();
                } else {
                    break;
                }
            }

            // check for comments
            if self.pos + 1 < self.input.len() {
                // single-line comment: --
                if &self.input[self.pos..self.pos + 2] == "--" {
                    while let Some(ch) = self.advance() {
                        if ch == '\n' {
                            break;
                        }
                    }
                    continue;
                }
                // block comment: /* ... */
                if &self.input[self.pos..self.pos + 2] == "/*" {
                    self.advance(); // '/'
                    self.advance(); // '*'
                    loop {
                        match self.advance() {
                            None => {
                                return Err(self.lex_error("unterminated block comment"));
                            }
                            Some('*') => {
                                if self.current_char() == Some('/') {
                                    self.advance();
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    continue;
                }
            }

            break;
        }
        Ok(())
    }

    // ── main dispatch ────────────────────────────────────────────────────────

    fn next_token(&mut self) -> Result<(Token, Span), LexError> {
        let line = self.line;
        let col = self.col;
        let start = self.pos;

        let ch = match self.current_char() {
            Some(c) => c,
            None => return Ok((Token::Eof, self.span(0))),
        };

        // Numbers: 42, 3.14, .5
        if ch.is_ascii_digit() || (ch == '.' && self.peek_char(1).map_or(false, |c| c.is_ascii_digit())) {
            return self.lex_number(line, col, start);
        }

        // Identifiers / keywords (including _vector)
        if ch.is_alphabetic() || ch == '_' {
            return self.lex_word(line, col, start);
        }

        // String literal (single-quoted)
        if ch == '\'' {
            return self.lex_string(line, col);
        }

        // Param: :identifier
        if ch == ':' {
            // peek ahead — if next is a letter/underscore it's a param, else Colon
            let next = self.peek_char(1);
            if next.map_or(false, |c| c.is_alphabetic() || c == '_') {
                return self.lex_param(line, col);
            } else {
                self.advance();
                return Ok((Token::Colon, self.span_at(line, col, 1)));
            }
        }

        // Operators & punctuation
        self.advance(); // consume `ch`
        let tok = match ch {
            '+' => Token::Plus,
            '%' => Token::Percent,
            '/' => Token::Slash,
            '*' => Token::Star,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            ',' => Token::Comma,
            ';' => Token::Semicolon,

            // = or ==  (treat both as Eq/Assign depending on context — we emit Assign)
            '=' => {
                if self.current_char() == Some('=') {
                    self.advance();
                    Token::Eq
                } else {
                    Token::Assign
                }
            }

            // ! -> !=
            '!' => {
                if self.current_char() == Some('=') {
                    self.advance();
                    Token::NotEq
                } else {
                    return Err(LexError {
                        message: format!("unexpected character '!'"),
                        line,
                        col,
                    });
                }
            }

            // < -> <-, <=, <->
            '<' => {
                if self.current_char() == Some('-') && self.peek_char(1) == Some('>') {
                    // <->  (VectorDist)
                    self.advance(); // '-'
                    self.advance(); // '>'
                    Token::VectorDist
                } else if self.current_char() == Some('=') {
                    self.advance();
                    Token::LtEq
                } else if self.current_char() == Some('>') {
                    // <> is NotEq in standard SQL
                    self.advance();
                    Token::NotEq
                } else {
                    Token::Lt
                }
            }

            // > -> >=
            '>' => {
                if self.current_char() == Some('=') {
                    self.advance();
                    Token::GtEq
                } else {
                    Token::Gt
                }
            }

            // - -> ->
            '-' => {
                if self.current_char() == Some('>') {
                    self.advance();
                    Token::Arrow
                } else {
                    Token::Minus
                }
            }

            // . -> .. or Dot
            '.' => {
                if self.current_char() == Some('.') {
                    self.advance();
                    Token::DotDot
                } else {
                    Token::Dot
                }
            }

            other => {
                return Err(LexError {
                    message: format!("unexpected character {:?}", other),
                    line,
                    col,
                });
            }
        };

        let len = self.pos - start;
        Ok((tok, self.span_at(line, col, len)))
    }

    // ── number lexer ─────────────────────────────────────────────────────────

    fn lex_number(&mut self, line: u32, col: u32, start: usize) -> Result<(Token, Span), LexError> {
        let mut has_dot = false;

        // leading dot: .5
        if self.current_char() == Some('.') {
            has_dot = true;
            self.advance();
        }

        while let Some(c) = self.current_char() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' && !has_dot {
                // Look ahead: ".." is DotDot, not decimal
                if self.peek_char(1) == Some('.') {
                    break;
                }
                has_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        let raw = &self.input[start..self.pos];
        let len = raw.len();

        let tok = if has_dot {
            let v: f64 = raw.parse().map_err(|_| LexError {
                message: format!("invalid float literal: {}", raw),
                line,
                col,
            })?;
            Token::FloatLiteral(v)
        } else {
            let v: i64 = raw.parse().map_err(|_| LexError {
                message: format!("invalid integer literal: {}", raw),
                line,
                col,
            })?;
            Token::IntLiteral(v)
        };

        Ok((tok, self.span_at(line, col, len)))
    }

    // ── word / keyword lexer ─────────────────────────────────────────────────

    fn lex_word(&mut self, line: u32, col: u32, start: usize) -> Result<(Token, Span), LexError> {
        while let Some(c) = self.current_char() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let raw = &self.input[start..self.pos];
        let lower = raw.to_ascii_lowercase();
        let len = raw.len();

        // Multi-word keyword matching needs to happen at parser level for most cases,
        // but we can handle some compound identifiers here.
        // Single-word keyword lookup:
        let tok = if let Some(kw) = Token::keyword(&lower) {
            kw
        } else {
            Token::Ident(raw.to_string())
        };

        Ok((tok, self.span_at(line, col, len)))
    }

    // ── string literal lexer ─────────────────────────────────────────────────

    fn lex_string(&mut self, line: u32, col: u32) -> Result<(Token, Span), LexError> {
        let start = self.pos;
        self.advance(); // opening '

        let mut s = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(LexError {
                        message: "unterminated string literal".to_string(),
                        line,
                        col,
                    });
                }
                Some('\'') => {
                    // check for escaped quote: ''
                    if self.current_char() == Some('\'') {
                        self.advance();
                        s.push('\'');
                    } else {
                        break;
                    }
                }
                Some(c) => s.push(c),
            }
        }

        let len = self.pos - start;
        Ok((Token::StringLiteral(s), self.span_at(line, col, len)))
    }

    // ── param lexer ──────────────────────────────────────────────────────────

    fn lex_param(&mut self, line: u32, col: u32) -> Result<(Token, Span), LexError> {
        let start = self.pos;
        self.advance(); // ':'

        let name_start = self.pos;
        while let Some(c) = self.current_char() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let name = self.input[name_start..self.pos].to_string();
        let len = self.pos - start;
        Ok((Token::Param(name), self.span_at(line, col, len)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(input: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(input);
        lexer
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .filter(|t| !matches!(t, Token::Eof))
            .collect()
    }

    #[test]
    fn test_vector_dist_is_single_token() {
        let mut lexer = Lexer::new("a <-> b");
        let toks: Vec<Token> = lexer
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        assert!(
            toks.iter().any(|t| matches!(t, Token::VectorDist)),
            "expected VectorDist token, got {:?}",
            toks
        );
        // Must NOT contain standalone Lt or Minus or Gt from the <-> sequence
        let non_eof: Vec<_> = toks.iter().filter(|t| !matches!(t, Token::Eof)).collect();
        // Should be: Ident("a"), VectorDist, Ident("b")
        assert_eq!(non_eof.len(), 3);
    }

    #[test]
    fn test_arrow_is_single_token() {
        let toks = tokens("a -> b");
        assert!(toks.iter().any(|t| matches!(t, Token::Arrow)));
    }

    #[test]
    fn test_dot_dot_is_single_token() {
        let toks = tokens("1..3");
        assert!(toks.iter().any(|t| matches!(t, Token::DotDot)));
    }

    #[test]
    fn test_param() {
        let toks = tokens(":agent_id");
        assert_eq!(toks[0], Token::Param("agent_id".to_string()));
    }

    #[test]
    fn test_string_literal() {
        let toks = tokens("'hello world'");
        assert_eq!(toks[0], Token::StringLiteral("hello world".to_string()));
    }

    #[test]
    fn test_float_literal() {
        let toks = tokens("0.5");
        assert_eq!(toks[0], Token::FloatLiteral(0.5));
    }

    #[test]
    fn test_leading_dot_float() {
        let toks = tokens(".75");
        assert_eq!(toks[0], Token::FloatLiteral(0.75));
    }

    #[test]
    fn test_keywords_case_insensitive() {
        let toks = tokens("SELECT FROM WHERE");
        assert_eq!(toks[0], Token::Select);
        assert_eq!(toks[1], Token::From);
        assert_eq!(toks[2], Token::Where);

        let toks2 = tokens("select from where");
        assert_eq!(toks2, toks);
    }

    #[test]
    fn test_single_line_comment_skipped() {
        let toks = tokens("SELECT -- this is ignored\nFROM");
        assert_eq!(toks, vec![Token::Select, Token::From]);
    }

    #[test]
    fn test_block_comment_skipped() {
        let toks = tokens("SELECT /* ignore */ FROM");
        assert_eq!(toks, vec![Token::Select, Token::From]);
    }

    #[test]
    fn test_number_before_dotdot_does_not_consume_dots() {
        let toks = tokens("1..3");
        assert_eq!(toks[0], Token::IntLiteral(1));
        assert_eq!(toks[1], Token::DotDot);
        assert_eq!(toks[2], Token::IntLiteral(3));
    }
}
