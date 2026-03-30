use crate::token::{Token, TokenType, InterpSegment};
use crate::error::{MoofError, Result};

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    line_start: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            line_start: 0,
            tokens: Vec::new(),
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>> {
        while self.pos < self.source.len() {
            self.skip_whitespace_and_comments()?;
            if self.pos >= self.source.len() { break; }

            let line = self.line;
            let col = self.pos - self.line_start + 1;
            self.scan_token(line, col)?;
        }
        let col = self.pos - self.line_start + 1;
        self.tokens.push(Token::new(TokenType::Eof, self.line, col));
        Ok(self.tokens)
    }

    // ── Helpers ───────────────────────────────────────────────────

    fn ch(&self) -> char { self.source[self.pos] }
    fn peek(&self) -> Option<char> { self.source.get(self.pos + 1).copied() }
    fn advance(&mut self) -> char { let c = self.source[self.pos]; self.pos += 1; c }
    fn at_end(&self) -> bool { self.pos >= self.source.len() }

    fn check(&self, c: char) -> bool { !self.at_end() && self.ch() == c }
    fn match_char(&mut self, c: char) -> bool {
        if self.check(c) { self.pos += 1; true } else { false }
    }

    fn emit(&mut self, ty: TokenType, line: usize, col: usize) {
        self.tokens.push(Token::new(ty, line, col));
    }

    // ── Whitespace & Comments ─────────────────────────────────────

    fn skip_whitespace_and_comments(&mut self) -> Result<()> {
        loop {
            if self.at_end() { break; }
            match self.ch() {
                ' ' | '\t' | '\r' => { self.pos += 1; }
                '\n' => { self.pos += 1; self.line += 1; self.line_start = self.pos; }
                ';' => { while !self.at_end() && self.ch() != '\n' { self.pos += 1; } }
                '#' if self.peek() == Some('|') => { self.skip_block_comment()?; }
                _ => break,
            }
        }
        Ok(())
    }

    fn skip_block_comment(&mut self) -> Result<()> {
        self.pos += 2; // skip #|
        let mut depth = 1;
        while depth > 0 {
            if self.at_end() {
                return Err(MoofError::syntax("Unterminated block comment", self.line, self.pos - self.line_start + 1));
            }
            if self.ch() == '#' && self.peek() == Some('|') { depth += 1; self.pos += 2; }
            else if self.ch() == '|' && self.peek() == Some('#') { depth -= 1; self.pos += 2; }
            else if self.ch() == '\n' { self.pos += 1; self.line += 1; self.line_start = self.pos; }
            else { self.pos += 1; }
        }
        Ok(())
    }

    // ── Token Scanning ────────────────────────────────────────────

    fn scan_token(&mut self, line: usize, col: usize) -> Result<()> {
        let c = self.ch();
        match c {
            '(' => { self.advance(); self.emit(TokenType::LParen, line, col); }
            ')' => { self.advance(); self.emit(TokenType::RParen, line, col); }
            '[' => { self.advance(); self.emit(TokenType::LBracket, line, col); }
            ']' => { self.advance(); self.emit(TokenType::RBracket, line, col); }
            '{' => { self.advance(); self.emit(TokenType::LBrace, line, col); }
            '}' => { self.advance(); self.emit(TokenType::RBrace, line, col); }
            '\'' => { self.advance(); self.emit(TokenType::Quote, line, col); }
            '`' => { self.advance(); self.emit(TokenType::Backtick, line, col); }
            '|' => { self.advance(); self.emit(TokenType::Pipe, line, col); }
            '&' => { self.advance(); self.emit(TokenType::Ampersand, line, col); }
            ',' => {
                self.advance();
                if self.check('@') { self.advance(); self.emit(TokenType::CommaAt, line, col); }
                else { self.emit(TokenType::Comma, line, col); }
            }
            '.' if self.peek().map_or(true, |c| !c.is_ascii_digit()) => {
                self.advance(); self.emit(TokenType::Dot, line, col);
            }
            '$' if self.peek() == Some('"') => { self.scan_interp_string(line, col)?; }
            '"' => { self.scan_string(line, col)?; }
            '0' if self.peek() == Some('x') || self.peek() == Some('X') => { self.scan_hex(line, col)?; }
            c if c == '-' && self.peek().map_or(false, |c| c.is_ascii_digit()) => { self.scan_number(line, col)?; }
            c if c.is_ascii_digit() => { self.scan_number(line, col)?; }
            c if is_ident_start(c) => { self.scan_identifier(line, col)?; }
            _ => {
                return Err(MoofError::syntax(format!("Unexpected character: {:?}", c), line, col));
            }
        }
        Ok(())
    }

    fn scan_string(&mut self, line: usize, col: usize) -> Result<()> {
        self.advance(); // skip opening "
        let mut value = String::new();
        loop {
            if self.at_end() {
                return Err(MoofError::syntax("Unterminated string", line, col));
            }
            match self.ch() {
                '"' => { self.advance(); break; }
                '\\' => {
                    self.advance();
                    if self.at_end() { return Err(MoofError::syntax("Unterminated string escape", line, col)); }
                    match self.advance() {
                        'n' => value.push('\n'),
                        't' => value.push('\t'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        c => { value.push('\\'); value.push(c); }
                    }
                }
                '\n' => { value.push('\n'); self.advance(); self.line += 1; self.line_start = self.pos; }
                c => { value.push(c); self.advance(); }
            }
        }
        self.emit(TokenType::Str(value), line, col);
        Ok(())
    }

    fn scan_interp_string(&mut self, line: usize, col: usize) -> Result<()> {
        self.pos += 2; // skip $"
        let mut segments: Vec<InterpSegment> = Vec::new();
        let mut current_text = String::new();

        loop {
            if self.at_end() {
                return Err(MoofError::syntax("Unterminated interpolated string", line, col));
            }
            if self.ch() == '"' {
                self.advance();
                if !current_text.is_empty() { segments.push(InterpSegment::Str(current_text)); }
                break;
            }
            if self.ch() == '\\' && self.peek() == Some('(') {
                if !current_text.is_empty() {
                    segments.push(InterpSegment::Str(current_text.clone()));
                    current_text.clear();
                }
                self.pos += 2; // skip \(
                let mut depth = 1;
                let mut expr = String::new();
                while depth > 0 {
                    if self.at_end() {
                        return Err(MoofError::syntax("Unterminated interpolation", line, col));
                    }
                    match self.ch() {
                        '(' => { depth += 1; expr.push(self.advance()); }
                        ')' => { depth -= 1; if depth > 0 { expr.push(self.advance()); } else { self.advance(); } }
                        '\n' => { expr.push('\n'); self.advance(); self.line += 1; self.line_start = self.pos; }
                        c => { expr.push(c); self.advance(); }
                    }
                }
                segments.push(InterpSegment::Expr(expr));
                continue;
            }
            if self.ch() == '\\' {
                self.advance();
                if self.at_end() { return Err(MoofError::syntax("Unterminated string escape", line, col)); }
                match self.advance() {
                    'n' => current_text.push('\n'),
                    't' => current_text.push('\t'),
                    '\\' => current_text.push('\\'),
                    '"' => current_text.push('"'),
                    c => { current_text.push('\\'); current_text.push(c); }
                }
            } else if self.ch() == '\n' {
                current_text.push('\n'); self.advance(); self.line += 1; self.line_start = self.pos;
            } else {
                current_text.push(self.advance());
            }
        }
        self.emit(TokenType::InterpString(segments), line, col);
        Ok(())
    }

    fn scan_hex(&mut self, line: usize, col: usize) -> Result<()> {
        self.pos += 2; // skip 0x
        let start = self.pos;
        while !self.at_end() && (self.ch().is_ascii_hexdigit() || self.ch() == '_') { self.pos += 1; }
        let hex_str: String = self.source[start..self.pos].iter().filter(|c| **c != '_').collect();
        let value = i64::from_str_radix(&hex_str, 16)
            .map_err(|_| MoofError::syntax("Invalid hex literal", line, col))?;
        self.emit(TokenType::Integer(value), line, col);
        Ok(())
    }

    fn scan_number(&mut self, line: usize, col: usize) -> Result<()> {
        let start = self.pos;
        if self.ch() == '-' { self.pos += 1; }
        while !self.at_end() && self.ch().is_ascii_digit() { self.pos += 1; }

        let mut is_float = false;
        // Decimal point
        if !self.at_end() && self.ch() == '.' && self.peek().map_or(false, |c| c.is_ascii_digit()) {
            is_float = true;
            self.pos += 1;
            while !self.at_end() && self.ch().is_ascii_digit() { self.pos += 1; }
        }
        // Scientific notation
        if !self.at_end() && (self.ch() == 'e' || self.ch() == 'E') {
            is_float = true;
            self.pos += 1;
            if !self.at_end() && (self.ch() == '+' || self.ch() == '-') { self.pos += 1; }
            while !self.at_end() && self.ch().is_ascii_digit() { self.pos += 1; }
        }

        let text: String = self.source[start..self.pos].iter().collect();
        if is_float {
            let value: f64 = text.parse().map_err(|_| MoofError::syntax("Invalid float", line, col))?;
            self.emit(TokenType::Float(value), line, col);
        } else {
            let value: i64 = text.parse().map_err(|_| MoofError::syntax("Invalid integer", line, col))?;
            self.emit(TokenType::Integer(value), line, col);
        }
        Ok(())
    }

    fn scan_identifier(&mut self, line: usize, col: usize) -> Result<()> {
        let start = self.pos;
        while !self.at_end() && is_ident_char(self.ch()) { self.pos += 1; }
        let text: String = self.source[start..self.pos].iter().collect();

        // Check for colon (keyword selector)
        if !self.at_end() && self.ch() == ':' && self.peek() != Some(':') {
            self.pos += 1;
            let kw = format!("{text}:");
            self.emit(TokenType::ColonId(kw), line, col);
            return Ok(());
        }

        match text.as_str() {
            "true"  => self.emit(TokenType::True, line, col),
            "false" => self.emit(TokenType::False, line, col),
            "nil"   => self.emit(TokenType::Nil, line, col),
            _       => self.emit(TokenType::Identifier(text), line, col),
        }
        Ok(())
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '_' | '!' | '?' | '*' | '/' | '+' | '-' | '<' | '>' | '=' | '%')
}

fn is_ident_char(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}
