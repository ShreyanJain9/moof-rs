use std::rc::Rc;

use crate::error::{MoofError, Result};
use crate::lexer::Lexer;
use crate::moofint::MoofInt;
use crate::symbol::SymbolTable;
use crate::token::{InterpSegment, Token, TokenType};
use crate::value::Value;

// ═════════════════════════════════════════════════════════════════════════════
// Parser — converts a token stream into cons-list–based Values.
//
// The parser absorbs ALL desugaring that the normalizer used to do in v1.
// Its output is `Vec<Value>` where each Value is a cons list (s-expression)
// or an atom.  The evaluator operates directly on these Values.
// ═════════════════════════════════════════════════════════════════════════════

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    symbols: &'a mut SymbolTable,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, symbols: &'a mut SymbolTable) -> Self {
        Parser {
            tokens,
            pos: 0,
            symbols,
        }
    }

    // ── Public entry point ──────────────────────────────────────────────

    pub fn parse_program(&mut self) -> Result<Vec<Value>> {
        let mut exprs = Vec::new();
        while !self.check_eof() {
            exprs.push(self.parse_expression()?);
        }
        Ok(exprs)
    }

    // ── Token navigation helpers ────────────────────────────────────────

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos + 1)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        self.pos += 1;
        tok
    }

    fn check_eof(&self) -> bool {
        matches!(self.current().ty, TokenType::Eof)
    }

    fn check(&self, ty: &TokenType) -> bool {
        std::mem::discriminant(&self.current().ty) == std::mem::discriminant(ty)
    }

    fn check_ident(&self, name: &str) -> bool {
        matches!(&self.current().ty, TokenType::Identifier(n) if n == name)
    }

    fn expect(&mut self, ty: &TokenType, message: &str) -> Result<Token> {
        if self.check(ty) {
            Ok(self.advance())
        } else {
            let tok = self.current();
            Err(MoofError::syntax(
                format!("{} -- got {:?}", message, tok.ty),
                tok.line,
                tok.column,
            ))
        }
    }

    fn expect_identifier(&mut self, message: &str) -> Result<(String, usize, usize)> {
        let tok = self.current().clone();
        if let TokenType::Identifier(name) = &tok.ty {
            let name = name.clone();
            self.pos += 1;
            Ok((name, tok.line, tok.column))
        } else {
            Err(MoofError::syntax(
                format!("{} -- got {:?}", message, tok.ty),
                tok.line,
                tok.column,
            ))
        }
    }

    fn syntax_error(&self, msg: &str) -> MoofError {
        let tok = self.current();
        MoofError::syntax(msg, tok.line, tok.column)
    }

    // ── Cons-list construction helpers ───────────────────────────────────

    fn sym(&mut self, name: &str) -> Value {
        Value::Symbol(self.symbols.intern(name))
    }

    /// Build a proper cons list from a Vec of Values.
    fn list(items: Vec<Value>) -> Value {
        Value::from_slice(&items)
    }

    /// Build a proper cons list from a slice of Values.
    fn list_from_slice(items: &[Value]) -> Value {
        Value::from_slice(items)
    }

    // ═════════════════════════════════════════════════════════════════════
    // Expression parsing
    // ═════════════════════════════════════════════════════════════════════

    fn parse_expression(&mut self) -> Result<Value> {
        let tok = self.current().clone();
        match &tok.ty {
            TokenType::LParen => self.parse_s_expression(),
            TokenType::LBracket => self.parse_message_send(),
            TokenType::LBrace => self.parse_brace_expression(),
            TokenType::Quote => self.parse_quote_sugar(),
            TokenType::Backtick => self.parse_quasiquote_sugar(),
            TokenType::CommaAt => self.parse_unquote_splice_sugar(),
            TokenType::Comma => self.parse_unquote_sugar(),
            TokenType::Ampersand => self.parse_selector_ref(),
            TokenType::Integer(v) => {
                let v = *v;
                self.pos += 1;
                Ok(Value::Integer(MoofInt::from(v)))
            }
            TokenType::Float(v) => {
                let v = *v;
                self.pos += 1;
                Ok(Value::Float(v))
            }
            TokenType::Str(s) => {
                let s = s.clone();
                self.pos += 1;
                Ok(Value::Str(Rc::from(s.as_str())))
            }
            TokenType::InterpString(segments) => {
                let segments = segments.clone();
                self.pos += 1;
                self.parse_interp_string(&segments)
            }
            TokenType::True => {
                self.pos += 1;
                Ok(Value::Bool(true))
            }
            TokenType::False => {
                self.pos += 1;
                Ok(Value::Bool(false))
            }
            TokenType::Nil => {
                self.pos += 1;
                Ok(Value::Nil)
            }
            TokenType::Identifier(name) => {
                let name = name.clone();
                self.pos += 1;
                Ok(Value::Symbol(self.symbols.intern(&name)))
            }
            TokenType::Dot => {
                // Bare dot — emit as Symbol(".") so evaluator can detect rest params
                self.pos += 1;
                Ok(self.sym("."))
            }
            _ => {
                Err(MoofError::syntax(
                    format!("Unexpected token {:?}", tok.ty),
                    tok.line,
                    tok.column,
                ))
            }
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // S-expressions  (...)
    // ═════════════════════════════════════════════════════════════════════

    fn parse_s_expression(&mut self) -> Result<Value> {
        self.expect(&TokenType::LParen, "Expected '('")?;

        // Empty parens → nil
        if matches!(self.current().ty, TokenType::RParen) {
            self.pos += 1;
            return Ok(Value::Nil);
        }

        // Check for special-form keywords that need parser-level desugaring
        if let TokenType::Identifier(name) = &self.current().ty {
            match name.as_str() {
                "define" => return self.parse_define(),
                "->" => return self.parse_pipeline(),
                _ => {}
            }
        }

        // Generic s-expression: parse all sub-expressions, skipping ColonId labels
        self.parse_generic_sexp()
    }

    /// Parse a generic s-expression: all elements until `)`, skipping ColonId
    /// labels (keeping only their values).  Returns a cons list.
    fn parse_generic_sexp(&mut self) -> Result<Value> {
        let mut items = Vec::new();
        while !matches!(self.current().ty, TokenType::RParen) {
            if let TokenType::ColonId(_) = &self.current().ty {
                // In the homoiconic model, ColonId tokens become symbols.
                // The evaluator handles keyword arg stripping during function calls.
                let tok = self.advance();
                let name = extract_colon_id_with_colon(&tok.ty);
                items.push(Value::Symbol(self.symbols.intern(&name)));
            } else {
                items.push(self.parse_expression()?);
            }
        }
        self.expect(&TokenType::RParen, "Expected ')'")?;
        Ok(Self::list(items))
    }

    // ── define (only sugar for function shorthand) ───────────────────────

    fn parse_define(&mut self) -> Result<Value> {
        self.pos += 1; // skip 'define'
        let define_sym = self.sym("define");

        if matches!(self.current().ty, TokenType::LParen) {
            // Function sugar: (define (f x y) body...) → (define f (lambda (x y) body...))
            self.pos += 1; // skip inner (
            let (name, _, _) = self.expect_identifier("Expected function name")?;
            let name_sym = self.sym(&name);

            // Parse params (including possible dot for rest param)
            let params = self.parse_param_list_until(&TokenType::RParen)?;
            self.expect(&TokenType::RParen, "Expected ')' after params")?;

            // Parse body expressions until outer )
            let body = self.parse_body_until(&TokenType::RParen)?;
            self.expect(&TokenType::RParen, "Expected ')' to close define")?;

            let lambda_sym = self.sym("lambda");
            let lambda = Self::list(vec![lambda_sym, params, body]);
            Ok(Self::list(vec![define_sym, name_sym, lambda]))
        } else {
            // Variable define: (define name value) — pass through as cons list
            let (name, _, _) = self.expect_identifier("Expected variable name")?;
            let name_sym = self.sym(&name);
            let value = self.parse_expression()?;
            self.expect(&TokenType::RParen, "Expected ')' to close define")?;
            Ok(Self::list(vec![define_sym, name_sym, value]))
        }
    }

    // ── Pipeline (-> ...) ───────────────────────────────────────────────

    fn parse_pipeline(&mut self) -> Result<Value> {
        self.pos += 1; // skip '->'
        let mut current = self.parse_expression()?;

        while !matches!(self.current().ty, TokenType::RParen) {
            current = self.parse_pipeline_step(current)?;
        }
        self.expect(&TokenType::RParen, "Expected ')' to close pipeline")?;
        Ok(current)
    }

    fn parse_pipeline_step(&mut self, prev: Value) -> Result<Value> {
        match &self.current().ty {
            TokenType::LBracket => {
                // Message send step: [selector args...] → (__send prev "selector" args...)
                self.pos += 1; // skip [

                if matches!(self.current().ty, TokenType::RBracket) {
                    return Err(self.syntax_error("Pipeline message step requires a selector"));
                }

                let (selector, args) = self.parse_selector_and_args(&TokenType::RBracket)?;
                self.expect(&TokenType::RBracket, "Expected ']' to close pipeline step")?;

                let send_sym = self.sym("__send");
                let mut items = vec![send_sym, prev, Value::Str(Rc::from(selector.as_str()))];
                items.extend(args);
                Ok(Self::list(items))
            }
            TokenType::LParen => {
                // Function call step: (func args...) → (func prev args...)
                self.pos += 1; // skip (
                let func = self.parse_expression()?;
                let mut items = vec![func, prev];
                while !matches!(self.current().ty, TokenType::RParen) {
                    if let TokenType::ColonId(_) = &self.current().ty {
                        self.advance();
                        items.push(self.parse_expression()?);
                    } else {
                        items.push(self.parse_expression()?);
                    }
                }
                self.expect(&TokenType::RParen, "Expected ')' in pipeline step")?;
                Ok(Self::list(items))
            }
            TokenType::Identifier(_) => {
                // Bare identifier step: func → (func prev)
                let func = self.parse_expression()?;
                Ok(Self::list(vec![func, prev]))
            }
            _ => {
                Err(self.syntax_error("Expected pipeline step"))
            }
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // Message sends  [...]
    // ═════════════════════════════════════════════════════════════════════

    fn parse_message_send(&mut self) -> Result<Value> {
        self.expect(&TokenType::LBracket, "Expected '['")?;

        // Empty brackets [] → nil (used as empty param list in method defs)
        if matches!(self.current().ty, TokenType::RBracket) {
            self.pos += 1;
            return Ok(Value::Nil);
        }

        let receiver = self.parse_expression()?;

        // Check for end — unary messages need at least a selector
        if matches!(self.current().ty, TokenType::RBracket) {
            // This could be [single-expr] which isn't a valid message send.
            // In Moof syntax this shouldn't happen — treat as error or as a list wrapper.
            let tok = self.current();
            return Err(MoofError::syntax(
                "Message send requires a selector after receiver",
                tok.line,
                tok.column,
            ));
        }

        let (selector, args) = self.parse_selector_and_args(&TokenType::RBracket)?;
        self.expect(&TokenType::RBracket, "Expected ']'")?;

        // Check if receiver is the `super` keyword → emit __super-send
        let is_super = matches!(&receiver, Value::Symbol(id) if *id == self.symbols.intern("super"));
        let send_sym = if is_super {
            self.sym("__super-send")
        } else {
            self.sym("__send")
        };
        let mut items = vec![send_sym, receiver, Value::Str(Rc::from(selector.as_str()))];
        items.extend(args);
        Ok(Self::list(items))
    }

    /// Parse selector + args.  Handles unary, keyword, and positional forms.
    fn parse_selector_and_args(
        &mut self,
        end_token: &TokenType,
    ) -> Result<(String, Vec<Value>)> {
        if let TokenType::ColonId(_) = &self.current().ty {
            // Keyword message: key1: val1 key2: val2
            self.parse_keyword_message()
        } else if let TokenType::Identifier(_) = &self.current().ty {
            let first_tok = self.advance();
            let first_name = if let TokenType::Identifier(n) = &first_tok.ty {
                n.clone()
            } else {
                unreachable!()
            };

            if self.check(end_token) {
                // Unary message: [obj method]
                Ok((first_name, Vec::new()))
            } else if let TokenType::ColonId(_) = &self.current().ty {
                // Mixed: first_name then keyword parts
                // e.g. [obj insertValue: 1 atIndex: 2] → selector "insertValue:atIndex:"
                let mut selector = first_name;
                let mut args = Vec::new();
                while let TokenType::ColonId(_) = &self.current().ty {
                    let kw_tok = self.advance();
                    selector.push_str(&extract_colon_id_with_colon(&kw_tok.ty));
                    args.push(self.parse_expression()?);
                }
                Ok((selector, args))
            } else {
                // Positional args: [obj method arg1 arg2]
                let mut args = Vec::new();
                while !self.check(end_token) {
                    args.push(self.parse_expression()?);
                }
                Ok((first_name, args))
            }
        } else {
            let tok = self.current();
            Err(MoofError::syntax(
                "Expected selector after receiver",
                tok.line,
                tok.column,
            ))
        }
    }

    fn parse_keyword_message(&mut self) -> Result<(String, Vec<Value>)> {
        let mut selector = String::new();
        let mut args = Vec::new();
        while let TokenType::ColonId(_) = &self.current().ty {
            let kw_tok = self.advance();
            selector.push_str(&extract_colon_id_with_colon(&kw_tok.ty));
            args.push(self.parse_expression()?);
        }
        Ok((selector, args))
    }

    // ═════════════════════════════════════════════════════════════════════
    // Brace expressions  {...}
    // ═════════════════════════════════════════════════════════════════════

    fn parse_brace_expression(&mut self) -> Result<Value> {
        self.expect(&TokenType::LBrace, "Expected '{'")?;

        // Empty braces → empty table
        if matches!(self.current().ty, TokenType::RBrace) {
            self.pos += 1;
            let table_sym = self.sym("__table");
            return Ok(Self::list(vec![table_sym]));
        }

        // Block: { |params...| body }
        if matches!(self.current().ty, TokenType::Pipe) {
            return self.parse_block();
        }

        // Determine if this is a hash table or array table
        self.parse_table_literal()
    }

    fn parse_block(&mut self) -> Result<Value> {
        self.expect(&TokenType::Pipe, "Expected '|' to start block params")?;
        let mut param_vals = Vec::new();
        while !matches!(self.current().ty, TokenType::Pipe) {
            if matches!(self.current().ty, TokenType::Dot) {
                // Rest param in block
                param_vals.push(self.sym("."));
                self.pos += 1;
                let (name, _, _) = self.expect_identifier("Expected rest param name")?;
                param_vals.push(self.sym(&name));
                break;
            }
            let (name, _, _) = self.expect_identifier("Expected block parameter name")?;
            param_vals.push(self.sym(&name));
        }
        self.expect(&TokenType::Pipe, "Expected '|' to end block params")?;

        let params_list = Self::list(param_vals);

        // Parse body
        let body = self.parse_body_until(&TokenType::RBrace)?;
        self.expect(&TokenType::RBrace, "Expected '}' to close block")?;

        let lambda_sym = self.sym("lambda");
        Ok(Self::list(vec![lambda_sym, params_list, body]))
    }

    fn parse_table_literal(&mut self) -> Result<Value> {
        // Peek to determine hash vs array
        if let TokenType::ColonId(_) = &self.current().ty {
            // Hash table: {key: val key2: val2}
            self.parse_hash_table()
        } else {
            // Could be array or hash — parse first element and check
            // If we see a comma or `}` after the first value, it's array
            // If we see a ColonId, it's hash mixed in
            self.parse_array_table()
        }
    }

    fn parse_hash_table(&mut self) -> Result<Value> {
        let table_sym = self.sym("__table");
        let mut items = vec![table_sym];

        while !matches!(self.current().ty, TokenType::RBrace) {
            if let TokenType::ColonId(_) = &self.current().ty {
                let kw_tok = self.advance();
                let key = extract_colon_id(&kw_tok.ty);
                items.push(Value::Str(Rc::from(key.as_str())));
                items.push(self.parse_expression()?);
            } else {
                return Err(self.syntax_error("Expected key: in table literal"));
            }
            // Skip optional comma
            if matches!(self.current().ty, TokenType::Comma) {
                self.pos += 1;
            }
        }
        self.expect(&TokenType::RBrace, "Expected '}'")?;
        Ok(Self::list(items))
    }

    fn parse_array_table(&mut self) -> Result<Value> {
        let table_array_sym = self.sym("__table-array");
        let mut items = vec![table_array_sym];

        while !matches!(self.current().ty, TokenType::RBrace) {
            // If we encounter a ColonId, switch to treating remaining as hash
            // This handles mixed tables — but for simplicity, once we started
            // as array, we stay array.  The user spec says detect.
            // For now: array mode collects values separated by optional commas.
            items.push(self.parse_expression()?);
            // Skip optional comma
            if matches!(self.current().ty, TokenType::Comma) {
                self.pos += 1;
            }
        }
        self.expect(&TokenType::RBrace, "Expected '}'")?;
        Ok(Self::list(items))
    }

    // ═════════════════════════════════════════════════════════════════════
    // Quote sugar
    // ═════════════════════════════════════════════════════════════════════

    fn parse_quote_sugar(&mut self) -> Result<Value> {
        self.pos += 1; // skip '
        let expr = self.parse_expression()?;
        let quote_sym = self.sym("quote");
        Ok(Self::list(vec![quote_sym, expr]))
    }

    fn parse_quasiquote_sugar(&mut self) -> Result<Value> {
        self.pos += 1; // skip `
        let expr = self.parse_expression()?;
        let qq_sym = self.sym("quasiquote");
        Ok(Self::list(vec![qq_sym, expr]))
    }

    fn parse_unquote_sugar(&mut self) -> Result<Value> {
        self.pos += 1; // skip ,
        let expr = self.parse_expression()?;
        let uq_sym = self.sym("unquote");
        Ok(Self::list(vec![uq_sym, expr]))
    }

    fn parse_unquote_splice_sugar(&mut self) -> Result<Value> {
        self.pos += 1; // skip ,@
        let expr = self.parse_expression()?;
        let uqs_sym = self.sym("unquote-splice");
        Ok(Self::list(vec![uqs_sym, expr]))
    }

    // ═════════════════════════════════════════════════════════════════════
    // Selector ref:  &name  or  &(key: val ...)
    // ═════════════════════════════════════════════════════════════════════

    fn parse_selector_ref(&mut self) -> Result<Value> {
        self.pos += 1; // skip &

        if matches!(self.current().ty, TokenType::LParen) {
            // &(keyword: arg ...) → (lambda (__r) (__send __r "keyword:" arg ...))
            self.pos += 1; // skip (
            let mut selector = String::new();
            let mut partial_args = Vec::new();
            while let TokenType::ColonId(_) = &self.current().ty {
                let kw_tok = self.advance();
                selector.push_str(&extract_colon_id_with_colon(&kw_tok.ty));
                partial_args.push(self.parse_expression()?);
            }
            self.expect(&TokenType::RParen, "Expected ')' to close selector ref")?;

            let lambda_sym = self.sym("lambda");
            let r_sym = self.sym("__r");
            let send_sym = self.sym("__send");

            let params = Self::list(vec![r_sym.clone()]);
            let mut send_items = vec![
                send_sym,
                r_sym,
                Value::Str(Rc::from(selector.as_str())),
            ];
            send_items.extend(partial_args);
            let send_call = Self::list(send_items);

            Ok(Self::list(vec![lambda_sym, params, send_call]))
        } else if let TokenType::Identifier(_) = &self.current().ty {
            // &name → (lambda (__r) (__send __r "name"))
            let (name, _, _) = self.expect_identifier("Expected selector name")?;

            let lambda_sym = self.sym("lambda");
            let r_sym = self.sym("__r");
            let send_sym = self.sym("__send");

            let params = Self::list(vec![r_sym.clone()]);
            let send_call = Self::list(vec![
                send_sym,
                r_sym,
                Value::Str(Rc::from(name.as_str())),
            ]);

            Ok(Self::list(vec![lambda_sym, params, send_call]))
        } else {
            Err(self.syntax_error("Expected selector name after '&'"))
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // String interpolation
    // ═════════════════════════════════════════════════════════════════════

    fn parse_interp_string(&mut self, segments: &[InterpSegment]) -> Result<Value> {
        let interp_sym = self.sym("__str-interp");
        let mut items = vec![interp_sym];

        for seg in segments {
            match seg {
                InterpSegment::Str(s) => {
                    items.push(Value::Str(Rc::from(s.as_str())));
                }
                InterpSegment::Expr(src) => {
                    let lexer = Lexer::new(src);
                    let tokens = lexer.tokenize()?;
                    let mut sub_parser = Parser::new(tokens, self.symbols);
                    let expr = sub_parser.parse_expression()?;
                    items.push(expr);
                }
            }
        }

        Ok(Self::list(items))
    }

    // ═════════════════════════════════════════════════════════════════════
    // Utility: parameter lists and body parsing
    // ═════════════════════════════════════════════════════════════════════

    /// Parse a parameter list (as a cons list of symbols, including `.` for rest).
    /// Consumes tokens until `end_type` is seen (does NOT consume `end_type`).
    fn parse_param_list_until(&mut self, end_type: &TokenType) -> Result<Value> {
        let mut params = Vec::new();
        while !self.check(end_type) {
            if matches!(self.current().ty, TokenType::Dot) {
                // Rest param marker
                params.push(self.sym("."));
                self.pos += 1;
                let (rest_name, _, _) = self.expect_identifier("Expected rest param name after '.'")?;
                params.push(self.sym(&rest_name));
                break;
            }
            let (name, _, _) = self.expect_identifier("Expected parameter name")?;
            params.push(self.sym(&name));
        }
        Ok(Self::list(params))
    }

    /// Parse body expressions until `end_type`.  If multiple, wrap in `(do ...)`.
    /// Does NOT consume `end_type`.
    fn parse_body_until(&mut self, end_type: &TokenType) -> Result<Value> {
        let mut exprs = Vec::new();
        while !self.check(end_type) {
            exprs.push(self.parse_expression()?);
        }
        if exprs.len() == 1 {
            Ok(exprs.into_iter().next().unwrap())
        } else {
            let do_sym = self.sym("do");
            let mut items = vec![do_sym];
            items.extend(exprs);
            Ok(Self::list(items))
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Helper functions
// ═════════════════════════════════════════════════════════════════════════════

/// Extract keyword name WITHOUT trailing colon from a ColonId token type.
fn extract_colon_id(ty: &TokenType) -> String {
    if let TokenType::ColonId(s) = ty {
        s.trim_end_matches(':').to_string()
    } else {
        String::new()
    }
}

/// Extract keyword name WITH trailing colon from a ColonId token type.
fn extract_colon_id_with_colon(ty: &TokenType) -> String {
    if let TokenType::ColonId(s) = ty {
        s.clone()
    } else {
        String::new()
    }
}
