use crate::token::{Token, TokenType, InterpSegment};
use crate::ast::*;
use crate::error::{MoofError, Result};
use crate::lexer::Lexer;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program> {
        let mut expressions = Vec::new();
        while !self.check_eof() {
            expressions.push(self.parse_expression()?);
        }
        Ok(Program { expressions })
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn match_token(&mut self, ty: &TokenType) -> bool {
        if self.check(ty) {
            self.pos += 1;
            true
        } else {
            false
        }
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

    // ── Expression Parsing ───────────────────────────────────────────

    fn parse_expression(&mut self) -> Result<Expr> {
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
                Ok(Expr::Integer(v, Loc::new(tok.line, tok.column)))
            }
            TokenType::Float(v) => {
                let v = *v;
                self.pos += 1;
                Ok(Expr::Float(v, Loc::new(tok.line, tok.column)))
            }
            TokenType::Str(s) => {
                let s = s.clone();
                self.pos += 1;
                Ok(Expr::Str(s, Loc::new(tok.line, tok.column)))
            }
            TokenType::InterpString(segments) => {
                let segments = segments.clone();
                self.pos += 1;
                self.parse_interp_string_token(&segments, tok.line, tok.column)
            }
            TokenType::True => {
                self.pos += 1;
                Ok(Expr::Bool(true, Loc::new(tok.line, tok.column)))
            }
            TokenType::False => {
                self.pos += 1;
                Ok(Expr::Bool(false, Loc::new(tok.line, tok.column)))
            }
            TokenType::Nil => {
                self.pos += 1;
                Ok(Expr::Nil(Loc::new(tok.line, tok.column)))
            }
            TokenType::Identifier(name) => {
                let name = name.clone();
                self.pos += 1;
                Ok(Expr::Identifier(name, Loc::new(tok.line, tok.column)))
            }
            _ => Err(MoofError::syntax(
                format!("Unexpected token {:?}", tok.ty),
                tok.line,
                tok.column,
            )),
        }
    }

    // ── S-Expression ─────────────────────────────────────────────────

    fn parse_s_expression(&mut self) -> Result<Expr> {
        let lparen = self.expect(&TokenType::LParen, "Expected '('")?;
        let ln = lparen.line;
        let col = lparen.column;

        // Empty parens -> nil
        if matches!(self.current().ty, TokenType::RParen) {
            self.pos += 1;
            return Ok(Expr::Nil(Loc::new(ln, col)));
        }

        // Check for special forms
        if let TokenType::Identifier(name) = &self.current().ty {
            match name.as_str() {
                "define" => return self.parse_define(ln, col),
                "lambda" => return self.parse_lambda(ln, col),
                "if" => return self.parse_if(ln, col),
                "let" => return self.parse_let(ln, col),
                "do" => return self.parse_do(ln, col),
                "set!" => return self.parse_set_bang(ln, col),
                "quote" => return self.parse_quote_form(ln, col),
                "try" => return self.parse_try_catch(ln, col),
                "cond" => return self.parse_cond(ln, col),
                "and" => return self.parse_and(ln, col),
                "or" => return self.parse_or(ln, col),
                "class" => return self.parse_class(ln, col),
                "trait" => return self.parse_trait(ln, col),
                "match" => return self.parse_match(ln, col),
                "type" => return self.parse_type_def(ln, col),
                "->" => return self.parse_pipeline(ln, col),
                "protocol" => return self.parse_protocol(ln, col),
                "extend" => return self.parse_extend(ln, col),
                "defmacro" => return self.parse_defmacro(ln, col),
                "module" => return self.parse_module(ln, col),
                "use" => return self.parse_use(ln, col),
                "require" => return self.parse_require(ln, col),
                _ => {}
            }
        }

        self.parse_call(ln, col)
    }

    fn parse_call(&mut self, ln: usize, col: usize) -> Result<Expr> {
        let callee = self.parse_expression()?;
        let mut args = Vec::new();
        while !matches!(self.current().ty, TokenType::RParen) {
            if let TokenType::ColonId(_) = &self.current().ty {
                let kw_tok = self.advance();
                let keyword = extract_colon_id(&kw_tok.ty);
                let value = self.parse_expression()?;
                args.push(Expr::KeywordArg(
                    keyword,
                    Box::new(value),
                    Loc::new(kw_tok.line, kw_tok.column),
                ));
            } else {
                args.push(self.parse_expression()?);
            }
        }
        self.expect(&TokenType::RParen, "Expected ')'")?;
        Ok(Expr::Call(Box::new(callee), args, Loc::new(ln, col)))
    }

    // ── Special Forms ────────────────────────────────────────────────

    fn parse_define(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'define'

        if matches!(self.current().ty, TokenType::LParen) {
            // Function definition: (define (name params...) body)
            self.pos += 1; // skip (
            let (name, _, _) = self.expect_identifier("Expected function name")?;
            let (params, rest_param) = self.parse_params_until(&TokenType::RParen)?;
            self.expect(&TokenType::RParen, "Expected ')' after params")?;
            let body = self.parse_body_until(&TokenType::RParen)?;
            self.expect(&TokenType::RParen, "Expected ')' to close define")?;
            Ok(Expr::DefineFunction(name, params, rest_param, Box::new(body), Loc::new(ln, col)))
        } else {
            let (name, _, _) = self.expect_identifier("Expected variable name")?;
            let value = self.parse_expression()?;
            self.expect(&TokenType::RParen, "Expected ')' to close define")?;
            Ok(Expr::Define(name, Box::new(value), Loc::new(ln, col)))
        }
    }

    fn parse_lambda(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'lambda'
        self.expect(&TokenType::LParen, "Expected '(' for params")?;
        let (params, rest_param) = self.parse_params_until(&TokenType::RParen)?;
        self.expect(&TokenType::RParen, "Expected ')' after params")?;
        let body = self.parse_body_until(&TokenType::RParen)?;
        self.expect(&TokenType::RParen, "Expected ')' to close lambda")?;
        Ok(Expr::Lambda(params, rest_param, Box::new(body), Loc::new(ln, col)))
    }

    fn parse_if(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'if'
        let condition = self.parse_expression()?;
        let then_branch = self.parse_expression()?;
        let else_branch = if matches!(self.current().ty, TokenType::RParen) {
            None
        } else {
            Some(Box::new(self.parse_expression()?))
        };
        self.expect(&TokenType::RParen, "Expected ')' to close if")?;
        Ok(Expr::If(
            Box::new(condition),
            Box::new(then_branch),
            else_branch,
            Loc::new(ln, col),
        ))
    }

    fn parse_let(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'let'
        self.expect(&TokenType::LParen, "Expected '(' for let bindings")?;
        let mut bindings = Vec::new();
        while matches!(self.current().ty, TokenType::LParen) {
            self.pos += 1; // skip (
            let (name, _, _) = self.expect_identifier("Expected binding name")?;
            let value = self.parse_expression()?;
            self.expect(&TokenType::RParen, "Expected ')' to close binding")?;
            bindings.push((name, value));
        }
        self.expect(&TokenType::RParen, "Expected ')' to close bindings")?;
        let body = self.parse_body_until(&TokenType::RParen)?;
        self.expect(&TokenType::RParen, "Expected ')' to close let")?;
        Ok(Expr::Let(bindings, Box::new(body), Loc::new(ln, col)))
    }

    fn parse_do(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'do'
        let mut exprs = Vec::new();
        while !matches!(self.current().ty, TokenType::RParen) {
            exprs.push(self.parse_expression()?);
        }
        self.expect(&TokenType::RParen, "Expected ')' to close do")?;
        Ok(Expr::Do(exprs, Loc::new(ln, col)))
    }

    fn parse_set_bang(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'set!'
        let (name, _, _) = self.expect_identifier("Expected variable name after set!")?;
        let value = self.parse_expression()?;
        self.expect(&TokenType::RParen, "Expected ')' to close set!")?;
        Ok(Expr::SetBang(name, Box::new(value), Loc::new(ln, col)))
    }

    fn parse_quote_form(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'quote'
        let expr = self.parse_expression()?;
        self.expect(&TokenType::RParen, "Expected ')' to close quote")?;
        Ok(Expr::Quote(Box::new(expr), Loc::new(ln, col)))
    }

    fn parse_try_catch(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'try'
        let body = self.parse_expression()?;
        self.expect(&TokenType::LParen, "Expected '(catch ...'")?;
        let (catch_kw, catch_ln, catch_col) = self.expect_identifier("Expected 'catch'")?;
        if catch_kw != "catch" {
            return Err(MoofError::syntax(
                format!("Expected 'catch', got {:?}", catch_kw),
                catch_ln,
                catch_col,
            ));
        }
        let (var_name, _, _) = self.expect_identifier("Expected error variable name")?;
        let handler = self.parse_expression()?;
        self.expect(&TokenType::RParen, "Expected ')' to close catch")?;
        self.expect(&TokenType::RParen, "Expected ')' to close try")?;
        Ok(Expr::TryCatch(
            Box::new(body),
            var_name,
            Box::new(handler),
            Loc::new(ln, col),
        ))
    }

    fn parse_cond(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'cond'
        let mut clauses = Vec::new();
        while matches!(self.current().ty, TokenType::LParen) {
            self.pos += 1; // skip (
            if self.check_ident("else") {
                self.pos += 1; // skip 'else'
                let expr = self.parse_expression()?;
                self.expect(&TokenType::RParen, "Expected ')' to close else clause")?;
                clauses.push((CondTest::Else, expr));
            } else {
                let test = self.parse_expression()?;
                let expr = self.parse_expression()?;
                self.expect(&TokenType::RParen, "Expected ')' to close cond clause")?;
                clauses.push((CondTest::Expr(test), expr));
            }
        }
        self.expect(&TokenType::RParen, "Expected ')' to close cond")?;
        Ok(Expr::Cond(clauses, Loc::new(ln, col)))
    }

    fn parse_and(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'and'
        let left = self.parse_expression()?;
        let right = self.parse_expression()?;
        self.expect(&TokenType::RParen, "Expected ')' to close and")?;
        Ok(Expr::And(Box::new(left), Box::new(right), Loc::new(ln, col)))
    }

    fn parse_or(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'or'
        let left = self.parse_expression()?;
        let right = self.parse_expression()?;
        self.expect(&TokenType::RParen, "Expected ')' to close or")?;
        Ok(Expr::Or(Box::new(left), Box::new(right), Loc::new(ln, col)))
    }

    // ── Class & Trait ────────────────────────────────────────────────

    fn parse_class(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'class'
        let (name, _, _) = self.expect_identifier("Expected class name")?;
        let mut superclass = None;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut traits = Vec::new();

        while !matches!(self.current().ty, TokenType::RParen) {
            self.expect(&TokenType::LParen, "Expected '(' in class body")?;
            let (clause_kw, clause_ln, clause_col) = self.expect_identifier("Expected clause keyword")?;
            match clause_kw.as_str() {
                "extends" => {
                    let (super_name, _, _) = self.expect_identifier("Expected superclass name")?;
                    superclass = Some(super_name);
                    self.expect(&TokenType::RParen, "Expected ')' to close extends")?;
                }
                "fields" => {
                    while let TokenType::Identifier(_) = &self.current().ty {
                        let (f, _, _) = self.expect_identifier("Expected field name")?;
                        fields.push(f);
                    }
                    self.expect(&TokenType::RParen, "Expected ')' to close fields")?;
                }
                "method" => {
                    let m = self.parse_method_body(clause_ln, clause_col)?;
                    methods.push(m);
                    self.expect(&TokenType::RParen, "Expected ')' to close method clause")?;
                }
                "uses" => {
                    let (trait_name, _, _) = self.expect_identifier("Expected trait name")?;
                    traits.push(trait_name);
                    self.expect(&TokenType::RParen, "Expected ')' to close uses")?;
                }
                _ => {
                    return Err(MoofError::syntax(
                        format!("Unknown class clause: {}", clause_kw),
                        clause_ln,
                        clause_col,
                    ));
                }
            }
        }

        self.expect(&TokenType::RParen, "Expected ')' to close class")?;
        Ok(Expr::ClassDef {
            name,
            superclass,
            fields,
            methods,
            traits,
            loc: Loc::new(ln, col),
        })
    }

    fn parse_trait(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'trait'
        let (name, _, _) = self.expect_identifier("Expected trait name")?;
        let mut methods = Vec::new();

        while !matches!(self.current().ty, TokenType::RParen) {
            self.expect(&TokenType::LParen, "Expected '(' in trait body")?;
            let (method_kw, method_ln, method_col) = self.expect_identifier("Expected 'method'")?;
            if method_kw != "method" {
                return Err(MoofError::syntax(
                    format!("Expected 'method', got {}", method_kw),
                    method_ln,
                    method_col,
                ));
            }
            let m = self.parse_method_body(method_ln, method_col)?;
            methods.push(m);
            self.expect(&TokenType::RParen, "Expected ')' to close method in trait")?;
        }

        self.expect(&TokenType::RParen, "Expected ')' to close trait")?;
        Ok(Expr::TraitDef {
            name,
            methods,
            loc: Loc::new(ln, col),
        })
    }

    fn parse_method_body(&mut self, ln: usize, col: usize) -> Result<MethodDef> {
        if let TokenType::Identifier(_) = &self.current().ty {
            // Unary or positional method: name [params...] body
            let (selector, _, _) = self.expect_identifier("Expected selector")?;
            self.expect(&TokenType::LBracket, "Expected '[' for method params")?;
            let (params, _rest) = self.parse_params_until(&TokenType::RBracket)?;
            self.expect(&TokenType::RBracket, "Expected ']' after method params")?;
            let body = self.parse_body_until(&TokenType::RParen)?;
            Ok(MethodDef {
                selector,
                params,
                body: Box::new(body),
                loc: Loc::new(ln, col),
            })
        } else if let TokenType::ColonId(_) = &self.current().ty {
            // Keyword method: key1: [p1] key2: [p2] body
            let mut selector = String::new();
            let mut params = Vec::new();
            while let TokenType::ColonId(_) = &self.current().ty {
                let kw_tok = self.advance();
                selector.push_str(&extract_colon_id_with_colon(&kw_tok.ty));
                self.expect(&TokenType::LBracket, "Expected '[' for keyword param")?;
                let (p, _) = self.parse_params_until(&TokenType::RBracket)?;
                params.extend(p);
                self.expect(&TokenType::RBracket, "Expected ']' after keyword param")?;
            }
            let body = self.parse_body_until(&TokenType::RParen)?;
            Ok(MethodDef {
                selector,
                params,
                body: Box::new(body),
                loc: Loc::new(ln, col),
            })
        } else {
            let tok = self.current();
            Err(MoofError::syntax(
                "Expected method selector",
                tok.line,
                tok.column,
            ))
        }
    }

    // ── Message Send ─────────────────────────────────────────────────

    fn parse_message_send(&mut self) -> Result<Expr> {
        let lbracket = self.expect(&TokenType::LBracket, "Expected '['")?;
        let ln = lbracket.line;
        let col = lbracket.column;
        let receiver = self.parse_expression()?;

        if matches!(self.current().ty, TokenType::RBracket) {
            let tok = self.current();
            return Err(MoofError::syntax(
                "Message send requires a selector",
                tok.line,
                tok.column,
            ));
        }

        let (selector, args) = self.parse_selector_and_args(&TokenType::RBracket)?;
        self.expect(&TokenType::RBracket, "Expected ']' to close message send")?;
        Ok(Expr::MessageSend(
            Box::new(receiver),
            selector,
            args,
            Loc::new(ln, col),
        ))
    }

    /// Parse a selector + args, used by both message send and pipeline message step.
    fn parse_selector_and_args(&mut self, end_token: &TokenType) -> Result<(String, Vec<Expr>)> {
        if let TokenType::ColonId(_) = &self.current().ty {
            self.parse_keyword_message()
        } else if let TokenType::Identifier(_) = &self.current().ty {
            let first_tok = self.advance();
            let first_name = if let TokenType::Identifier(n) = &first_tok.ty {
                n.clone()
            } else {
                unreachable!()
            };

            if self.check(end_token) {
                // Unary message
                Ok((first_name, Vec::new()))
            } else if let TokenType::ColonId(_) = &self.current().ty {
                // Mixed: first_name then keyword parts
                let mut selector = first_name;
                let mut args = Vec::new();
                while let TokenType::ColonId(_) = &self.current().ty {
                    let kw_tok = self.advance();
                    selector.push_str(&extract_colon_id_with_colon(&kw_tok.ty));
                    args.push(self.parse_expression()?);
                }
                Ok((selector, args))
            } else {
                // Positional args
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

    fn parse_keyword_message(&mut self) -> Result<(String, Vec<Expr>)> {
        let mut selector = String::new();
        let mut args = Vec::new();
        while let TokenType::ColonId(_) = &self.current().ty {
            let kw_tok = self.advance();
            selector.push_str(&extract_colon_id_with_colon(&kw_tok.ty));
            args.push(self.parse_expression()?);
        }
        Ok((selector, args))
    }

    // ── Brace expressions (map literal or block) ─────────────────────

    fn parse_brace_expression(&mut self) -> Result<Expr> {
        let lbrace = self.expect(&TokenType::LBrace, "Expected '{'")?;
        let ln = lbrace.line;
        let col = lbrace.column;

        if matches!(self.current().ty, TokenType::Pipe) {
            return self.parse_block(ln, col);
        }

        self.parse_map_literal_body(ln, col)
    }

    fn parse_block(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.expect(&TokenType::Pipe, "Expected '|' to start block params")?;
        let mut params = Vec::new();
        while !matches!(self.current().ty, TokenType::Pipe) {
            let (name, _, _) = self.expect_identifier("Expected block parameter name")?;
            params.push(name);
        }
        self.expect(&TokenType::Pipe, "Expected '|' to end block params")?;

        let mut body_exprs = Vec::new();
        while !matches!(self.current().ty, TokenType::RBrace) {
            body_exprs.push(self.parse_expression()?);
        }
        self.expect(&TokenType::RBrace, "Expected '}' to close block")?;

        let body = if body_exprs.len() == 1 {
            body_exprs.into_iter().next().unwrap()
        } else {
            Expr::Do(body_exprs, Loc::new(ln, col))
        };

        Ok(Expr::Lambda(params, None, Box::new(body), Loc::new(ln, col)))
    }

    fn parse_map_literal_body(&mut self, ln: usize, col: usize) -> Result<Expr> {
        let mut pairs = Vec::new();
        while !matches!(self.current().ty, TokenType::RBrace) {
            let kw_tok = self.expect(&TokenType::ColonId(String::new()), "Expected key: in map literal")?;
            let key = extract_colon_id(&kw_tok.ty);
            let value = self.parse_expression()?;
            pairs.push((key, Box::new(value)));
        }
        self.expect(&TokenType::RBrace, "Expected '}' to close map")?;
        Ok(Expr::MapLiteral(pairs, Loc::new(ln, col)))
    }

    // ── Quote Sugar ──────────────────────────────────────────────────

    fn parse_quote_sugar(&mut self) -> Result<Expr> {
        let qt = self.advance();
        let expr = self.parse_expression()?;
        Ok(Expr::Quote(Box::new(expr), Loc::new(qt.line, qt.column)))
    }

    fn parse_quasiquote_sugar(&mut self) -> Result<Expr> {
        let bt = self.advance();
        let expr = self.parse_expression()?;
        Ok(Expr::Quasiquote(Box::new(expr), Loc::new(bt.line, bt.column)))
    }

    fn parse_unquote_sugar(&mut self) -> Result<Expr> {
        let c = self.advance();
        let expr = self.parse_expression()?;
        Ok(Expr::Unquote(Box::new(expr), Loc::new(c.line, c.column)))
    }

    fn parse_unquote_splice_sugar(&mut self) -> Result<Expr> {
        let ca = self.advance();
        let expr = self.parse_expression()?;
        Ok(Expr::UnquoteSplice(Box::new(expr), Loc::new(ca.line, ca.column)))
    }

    // ── Selector Ref ─────────────────────────────────────────────────

    fn parse_selector_ref(&mut self) -> Result<Expr> {
        let amp = self.advance();
        let ln = amp.line;
        let col = amp.column;

        if matches!(self.current().ty, TokenType::LParen) {
            // &(keyword: arg ...)
            self.pos += 1; // skip (
            let mut selector = String::new();
            let mut partial_args = Vec::new();
            while let TokenType::ColonId(_) = &self.current().ty {
                let kw_tok = self.advance();
                selector.push_str(&extract_colon_id_with_colon(&kw_tok.ty));
                partial_args.push(self.parse_expression()?);
            }
            self.expect(&TokenType::RParen, "Expected ')' to close selector ref")?;
            Ok(Expr::SelectorRef(selector, partial_args, Loc::new(ln, col)))
        } else if let TokenType::Identifier(_) = &self.current().ty {
            let (name, _, _) = self.expect_identifier("Expected selector name")?;
            Ok(Expr::SelectorRef(name, Vec::new(), Loc::new(ln, col)))
        } else {
            let tok = self.current();
            Err(MoofError::syntax(
                "Expected selector name after '&'",
                tok.line,
                tok.column,
            ))
        }
    }

    // ── Match ────────────────────────────────────────────────────────

    fn parse_match(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'match'
        let expr = self.parse_expression()?;
        let mut clauses = Vec::new();
        while matches!(self.current().ty, TokenType::LParen) {
            clauses.push(self.parse_match_clause()?);
        }
        self.expect(&TokenType::RParen, "Expected ')' to close match")?;
        Ok(Expr::Match(Box::new(expr), clauses, Loc::new(ln, col)))
    }

    fn parse_match_clause(&mut self) -> Result<MatchClause> {
        self.expect(&TokenType::LParen, "Expected '(' for match clause")?;
        let pattern = self.parse_pattern()?;

        let guard = if self.check_ident("when") {
            self.pos += 1; // skip 'when'
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        let body = self.parse_expression()?;
        self.expect(&TokenType::RParen, "Expected ')' to close match clause")?;
        Ok(MatchClause {
            pattern,
            guard,
            body: Box::new(body),
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        let tok = self.current().clone();
        match &tok.ty {
            TokenType::Integer(v) => {
                let v = *v;
                self.pos += 1;
                Ok(Pattern::Literal(Box::new(Expr::Integer(v, Loc::new(tok.line, tok.column)))))
            }
            TokenType::Float(v) => {
                let v = *v;
                self.pos += 1;
                Ok(Pattern::Literal(Box::new(Expr::Float(v, Loc::new(tok.line, tok.column)))))
            }
            TokenType::Str(s) => {
                let s = s.clone();
                self.pos += 1;
                Ok(Pattern::Literal(Box::new(Expr::Str(s, Loc::new(tok.line, tok.column)))))
            }
            TokenType::True => {
                self.pos += 1;
                Ok(Pattern::Literal(Box::new(Expr::Bool(true, Loc::new(tok.line, tok.column)))))
            }
            TokenType::False => {
                self.pos += 1;
                Ok(Pattern::Literal(Box::new(Expr::Bool(false, Loc::new(tok.line, tok.column)))))
            }
            TokenType::Nil => {
                self.pos += 1;
                Ok(Pattern::Literal(Box::new(Expr::Nil(Loc::new(tok.line, tok.column)))))
            }
            TokenType::Identifier(name) => {
                let name = name.clone();
                self.pos += 1;
                if name == "_" {
                    Ok(Pattern::Wildcard)
                } else {
                    Ok(Pattern::Bind(name))
                }
            }
            TokenType::LBrace => self.parse_map_pattern(),
            TokenType::LParen => self.parse_list_or_constructor_pattern(),
            _ => Err(MoofError::syntax(
                format!("Unexpected token in pattern: {:?}", tok.ty),
                tok.line,
                tok.column,
            )),
        }
    }

    fn parse_map_pattern(&mut self) -> Result<Pattern> {
        self.expect(&TokenType::LBrace, "Expected '{' for map pattern")?;
        let mut pairs = Vec::new();
        while !matches!(self.current().ty, TokenType::RBrace) {
            let kw_tok = self.expect(&TokenType::ColonId(String::new()), "Expected key: in map pattern")?;
            let key = extract_colon_id(&kw_tok.ty);
            let pat = self.parse_pattern()?;
            pairs.push((key, pat));
        }
        self.expect(&TokenType::RBrace, "Expected '}' to close map pattern")?;
        Ok(Pattern::Map(pairs))
    }

    fn parse_list_or_constructor_pattern(&mut self) -> Result<Pattern> {
        self.expect(&TokenType::LParen, "Expected '(' for pattern")?;

        // Constructor pattern: (ClassName binding1 binding2)
        if let TokenType::Identifier(name) = &self.current().ty {
            if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                let class_name = name.clone();
                self.pos += 1;
                let mut bindings = Vec::new();
                while !matches!(self.current().ty, TokenType::RParen) {
                    bindings.push(self.parse_pattern()?);
                }
                self.expect(&TokenType::RParen, "Expected ')' to close constructor pattern")?;
                return Ok(Pattern::Constructor(class_name, bindings));
            }
        }

        // If starts with "list", skip it
        if self.check_ident("list") {
            self.pos += 1;
        }

        // List pattern: (elem1 elem2 . rest) or (elem1 elem2)
        let mut elements = Vec::new();
        let mut rest = None;
        while !matches!(self.current().ty, TokenType::RParen) {
            if matches!(self.current().ty, TokenType::Dot) {
                self.pos += 1; // skip dot
                rest = Some(Box::new(self.parse_pattern()?));
                break;
            }
            elements.push(self.parse_pattern()?);
        }
        self.expect(&TokenType::RParen, "Expected ')' to close list pattern")?;
        Ok(Pattern::List(elements, rest))
    }

    // ── Type Definition ──────────────────────────────────────────────

    fn parse_type_def(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'type'
        let (name, _, _) = self.expect_identifier("Expected type name")?;
        let mut variants = Vec::new();

        while !matches!(self.current().ty, TokenType::RParen) {
            if matches!(self.current().ty, TokenType::LParen) {
                self.pos += 1; // skip (
                let (variant_name, _, _) = self.expect_identifier("Expected variant name")?;
                let mut fields = Vec::new();
                while let TokenType::Identifier(_) = &self.current().ty {
                    let (f, _, _) = self.expect_identifier("Expected field name")?;
                    fields.push(f);
                }
                self.expect(&TokenType::RParen, "Expected ')' to close variant")?;
                variants.push(TypeVariant { name: variant_name, fields });
            } else if let TokenType::Identifier(_) = &self.current().ty {
                let (variant_name, _, _) = self.expect_identifier("Expected variant name")?;
                variants.push(TypeVariant { name: variant_name, fields: Vec::new() });
            } else {
                let tok = self.current();
                return Err(MoofError::syntax(
                    "Expected variant in type definition",
                    tok.line,
                    tok.column,
                ));
            }
        }

        self.expect(&TokenType::RParen, "Expected ')' to close type")?;
        Ok(Expr::TypeDef(name, variants, Loc::new(ln, col)))
    }

    // ── Pipeline ─────────────────────────────────────────────────────

    fn parse_pipeline(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip '->'
        let value = self.parse_expression()?;
        let mut steps = Vec::new();
        while !matches!(self.current().ty, TokenType::RParen) {
            if matches!(self.current().ty, TokenType::LBracket) {
                steps.push(self.parse_pipeline_message_step()?);
            } else {
                steps.push(self.parse_expression()?);
            }
        }
        self.expect(&TokenType::RParen, "Expected ')' to close pipeline")?;
        Ok(Expr::Pipeline(Box::new(value), steps, Loc::new(ln, col)))
    }

    fn parse_pipeline_message_step(&mut self) -> Result<Expr> {
        let lbracket = self.expect(&TokenType::LBracket, "Expected '['")?;
        let ln = lbracket.line;
        let col = lbracket.column;

        let placeholder = Expr::Identifier(
            "__pipeline_placeholder__".to_string(),
            Loc::new(ln, col),
        );

        let (selector, args) = self.parse_selector_and_args(&TokenType::RBracket)?;
        self.expect(&TokenType::RBracket, "Expected ']' to close pipeline message step")?;

        Ok(Expr::MessageSend(
            Box::new(placeholder),
            selector,
            args,
            Loc::new(ln, col),
        ))
    }

    // ── Protocol ─────────────────────────────────────────────────────

    fn parse_protocol(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'protocol'
        let (name, _, _) = self.expect_identifier("Expected protocol name")?;
        let mut selectors = Vec::new();
        while !matches!(self.current().ty, TokenType::RParen) {
            match &self.current().ty {
                TokenType::Identifier(_) => {
                    let (sel, _, _) = self.expect_identifier("Expected selector")?;
                    selectors.push(sel);
                }
                TokenType::ColonId(_) => {
                    let tok = self.advance();
                    selectors.push(extract_colon_id_with_colon(&tok.ty));
                }
                _ => {
                    let tok = self.current();
                    return Err(MoofError::syntax(
                        "Expected selector in protocol",
                        tok.line,
                        tok.column,
                    ));
                }
            }
        }
        self.expect(&TokenType::RParen, "Expected ')' to close protocol")?;
        Ok(Expr::ProtocolDef(name, selectors, Loc::new(ln, col)))
    }

    // ── Extend ───────────────────────────────────────────────────────

    fn parse_extend(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'extend'
        let (name, _, _) = self.expect_identifier("Expected class name after extend")?;
        let mut methods = Vec::new();

        while !matches!(self.current().ty, TokenType::RParen) {
            self.expect(&TokenType::LParen, "Expected '(' in extend body")?;
            let (method_kw, method_ln, method_col) = self.expect_identifier("Expected 'method'")?;
            if method_kw != "method" {
                return Err(MoofError::syntax(
                    format!("Expected 'method' in extend, got {}", method_kw),
                    method_ln,
                    method_col,
                ));
            }
            let m = self.parse_method_body(method_ln, method_col)?;
            methods.push(m);
            self.expect(&TokenType::RParen, "Expected ')' to close method in extend")?;
        }

        self.expect(&TokenType::RParen, "Expected ')' to close extend")?;
        Ok(Expr::ClassDef {
            name,
            superclass: None,
            fields: Vec::new(),
            methods,
            traits: Vec::new(),
            loc: Loc::new(ln, col),
        })
    }

    // ── DefMacro ─────────────────────────────────────────────────────

    fn parse_defmacro(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'defmacro'
        let (name, _, _) = self.expect_identifier("Expected macro name")?;
        self.expect(&TokenType::LParen, "Expected '(' for macro params")?;
        let mut params = Vec::new();
        while !matches!(self.current().ty, TokenType::RParen) {
            if matches!(self.current().ty, TokenType::Dot) {
                self.pos += 1; // skip dot
                let (_rest_name, _, _) = self.expect_identifier("Expected rest param name after '.'")?;
                // DefMacro AST doesn't have rest_param, so we just consume it
                break;
            }
            let (p, _, _) = self.expect_identifier("Expected parameter name")?;
            params.push(p);
        }
        self.expect(&TokenType::RParen, "Expected ')' after macro params")?;
        let body = self.parse_body_until(&TokenType::RParen)?;
        self.expect(&TokenType::RParen, "Expected ')' to close defmacro")?;
        Ok(Expr::DefMacro(name, params, Box::new(body), Loc::new(ln, col)))
    }

    // ── Module System ────────────────────────────────────────────────

    fn parse_module(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'module'
        let (name, _, _) = self.expect_identifier("Expected module name")?;
        let mut exports = Vec::new();
        let mut body = Vec::new();

        // Check for (export ...) clause
        if matches!(self.current().ty, TokenType::LParen) {
            let saved_pos = self.pos;
            self.pos += 1; // skip (
            if self.check_ident("export") {
                self.pos += 1; // skip 'export'
                while matches!(&self.current().ty, TokenType::Identifier(_) | TokenType::ColonId(_)) {
                    let tok = self.advance();
                    match &tok.ty {
                        TokenType::Identifier(n) => exports.push(n.clone()),
                        TokenType::ColonId(n) => exports.push(n.clone()),
                        _ => unreachable!(),
                    }
                }
                self.expect(&TokenType::RParen, "Expected ')' to close export list")?;
            } else {
                self.pos = saved_pos; // backtrack
            }
        }

        while !matches!(self.current().ty, TokenType::RParen) {
            body.push(self.parse_expression()?);
        }
        self.expect(&TokenType::RParen, "Expected ')' to close module")?;
        Ok(Expr::ModuleDef(name, exports, body, Loc::new(ln, col)))
    }

    fn parse_use(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'use'
        let (module_name, _, _) = self.expect_identifier("Expected module name")?;
        let mut imports = None;
        let mut alias_name = None;

        if !matches!(self.current().ty, TokenType::RParen) {
            if let TokenType::ColonId(kw) = &self.current().ty {
                if kw == "as:" {
                    self.pos += 1; // skip as:
                    let (alias, _, _) = self.expect_identifier("Expected alias name")?;
                    alias_name = Some(alias);
                }
            } else if matches!(self.current().ty, TokenType::LParen) {
                self.pos += 1; // skip (
                let mut imp = Vec::new();
                while matches!(&self.current().ty, TokenType::Identifier(_) | TokenType::ColonId(_)) {
                    let tok = self.advance();
                    match &tok.ty {
                        TokenType::Identifier(n) => imp.push(n.clone()),
                        TokenType::ColonId(n) => imp.push(n.clone()),
                        _ => unreachable!(),
                    }
                }
                self.expect(&TokenType::RParen, "Expected ')' to close import list")?;
                imports = Some(imp);
            }
        }

        self.expect(&TokenType::RParen, "Expected ')' to close use")?;
        Ok(Expr::UseModule(module_name, imports, alias_name, Loc::new(ln, col)))
    }

    fn parse_require(&mut self, ln: usize, col: usize) -> Result<Expr> {
        self.pos += 1; // skip 'require'
        let path_tok = self.expect(&TokenType::Str(String::new()), "Expected file path string")?;
        let path = if let TokenType::Str(s) = &path_tok.ty {
            s.clone()
        } else {
            unreachable!()
        };
        self.expect(&TokenType::RParen, "Expected ')' to close require")?;
        Ok(Expr::Require(path, Loc::new(ln, col)))
    }

    // ── Interpolated Strings ─────────────────────────────────────────

    fn parse_interp_string_token(
        &self,
        segments: &[InterpSegment],
        line: usize,
        col: usize,
    ) -> Result<Expr> {
        let mut exprs = Vec::new();
        for seg in segments {
            match seg {
                InterpSegment::Str(s) => {
                    exprs.push(Expr::Str(s.clone(), Loc::new(line, col)));
                }
                InterpSegment::Expr(src) => {
                    let lexer = Lexer::new(src);
                    let tokens = lexer.tokenize()?;
                    let mut inner_parser = Parser::new(tokens);
                    let expr = inner_parser.parse_expression()?;
                    exprs.push(expr);
                }
            }
        }
        Ok(Expr::StringInterp(exprs, Loc::new(line, col)))
    }

    // ── Utility ──────────────────────────────────────────────────────

    fn parse_params_until(&mut self, end_type: &TokenType) -> Result<(Vec<String>, Option<String>)> {
        let mut params = Vec::new();
        let mut rest_param = None;
        while !self.check(end_type) {
            if matches!(self.current().ty, TokenType::Dot) {
                self.pos += 1; // skip dot
                let (rest_name, _, _) = self.expect_identifier("Expected rest param name after '.'")?;
                rest_param = Some(rest_name);
                break;
            }
            let (name, _, _) = self.expect_identifier("Expected parameter name")?;
            params.push(name);
        }
        Ok((params, rest_param))
    }

    fn parse_body_until(&mut self, end_type: &TokenType) -> Result<Expr> {
        let mut exprs = Vec::new();
        while !self.check(end_type) {
            exprs.push(self.parse_expression()?);
        }
        if exprs.len() == 1 {
            Ok(exprs.into_iter().next().unwrap())
        } else {
            Ok(Expr::Do(exprs, Loc::none()))
        }
    }
}

// ── Helper functions ─────────────────────────────────────────────────

/// Extract keyword name without trailing colon from a ColonId token type.
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
