require_relative "token"
require_relative "ast"
require_relative "errors"

module Moof
  class Parser
    include TokenType

    def initialize(tokens)
      @tokens = tokens
      @pos = 0
    end

    def parse_program
      expressions = []
      expressions << parse_expression until check(EOF)
      AST::Program.new(expressions: expressions)
    end

    private

    def current; @tokens[@pos]; end
    def peek;    @tokens[@pos + 1]; end

    def advance
      tok = @tokens[@pos]
      @pos += 1
      tok
    end

    def check(type); current.type == type; end

    def match(type)
      advance if check(type)
    end

    def expect(type, message)
      if check(type)
        advance
      else
        tok = current
        raise Moof::SyntaxError.new(
          "#{message} — got #{tok.type}(#{tok.lexeme.inspect})",
          line: tok.line, column: tok.column
        )
      end
    end

    def parse_expression
      tok = current
      case tok.type
      when LPAREN   then parse_s_expression
      when LBRACKET then parse_message_send
      when LBRACE   then parse_map_literal
      when QUOTE    then parse_quote_sugar
      when INTEGER
        advance
        AST::IntegerLiteral.new(value: tok.literal, line: tok.line, column: tok.column)
      when FLOAT
        advance
        AST::FloatLiteral.new(value: tok.literal, line: tok.line, column: tok.column)
      when STRING
        advance
        AST::StringLiteral.new(value: tok.literal, line: tok.line, column: tok.column)
      when TRUE
        advance
        AST::BoolLiteral.new(value: true, line: tok.line, column: tok.column)
      when FALSE
        advance
        AST::BoolLiteral.new(value: false, line: tok.line, column: tok.column)
      when NIL
        advance
        AST::NilLiteral.new(line: tok.line, column: tok.column)
      when IDENTIFIER
        advance
        AST::Identifier.new(name: tok.lexeme, line: tok.line, column: tok.column)
      else
        raise Moof::SyntaxError.new(
          "Unexpected token #{tok.type}(#{tok.lexeme.inspect})",
          line: tok.line, column: tok.column
        )
      end
    end

    def parse_s_expression
      lparen = expect(LPAREN, "Expected '('")
      ln, col = lparen.line, lparen.column

      if check(RPAREN)
        advance
        return AST::NilLiteral.new(line: ln, column: col)
      end

      if check(IDENTIFIER)
        case current.lexeme
        when "define"  then return parse_define(ln, col)
        when "lambda"  then return parse_lambda(ln, col)
        when "if"      then return parse_if(ln, col)
        when "let"     then return parse_let(ln, col)
        when "do"      then return parse_do(ln, col)
        when "set!"    then return parse_set_bang(ln, col)
        when "quote"   then return parse_quote(ln, col)
        when "try"     then return parse_try_catch(ln, col)
        when "cond"    then return parse_cond(ln, col)
        when "and"     then return parse_and(ln, col)
        when "or"      then return parse_or(ln, col)
        when "class"   then return parse_class(ln, col)
        when "trait"   then return parse_trait(ln, col)
        end
      end

      parse_call(ln, col)
    end

    def parse_call(ln, col)
      callee = parse_expression
      args = []
      args << parse_expression until check(RPAREN)
      expect(RPAREN, "Expected ')'")
      AST::Call.new(callee: callee, arguments: args, line: ln, column: col)
    end

    def parse_define(ln, col)
      advance # skip define
      if check(LPAREN)
        advance
        name_tok = expect(IDENTIFIER, "Expected function name")
        params, rest_param = parse_params_until(RPAREN)
        expect(RPAREN, "Expected ')' after params")
        body = parse_body_until(RPAREN)
        expect(RPAREN, "Expected ')' to close define")
        AST::DefineFunction.new(name: name_tok.lexeme, params: params, rest_param: rest_param,
          body: body, line: ln, column: col)
      else
        name_tok = expect(IDENTIFIER, "Expected variable name")
        value = parse_expression
        expect(RPAREN, "Expected ')' to close define")
        AST::Define.new(name: name_tok.lexeme, value: value, line: ln, column: col)
      end
    end

    def parse_lambda(ln, col)
      advance
      expect(LPAREN, "Expected '(' for params")
      params, rest_param = parse_params_until(RPAREN)
      expect(RPAREN, "Expected ')' after params")
      body = parse_body_until(RPAREN)
      expect(RPAREN, "Expected ')' to close lambda")
      AST::Lambda.new(params: params, rest_param: rest_param, body: body, line: ln, column: col)
    end

    def parse_if(ln, col)
      advance
      condition   = parse_expression
      then_branch = parse_expression
      else_branch = check(RPAREN) ? nil : parse_expression
      expect(RPAREN, "Expected ')' to close if")
      AST::If.new(condition: condition, then_branch: then_branch, else_branch: else_branch,
        line: ln, column: col)
    end

    def parse_let(ln, col)
      advance
      expect(LPAREN, "Expected '(' for let bindings")
      bindings = []
      while check(LPAREN)
        advance
        name_tok = expect(IDENTIFIER, "Expected binding name")
        value = parse_expression
        expect(RPAREN, "Expected ')' to close binding")
        bindings << [name_tok.lexeme, value]
      end
      expect(RPAREN, "Expected ')' to close bindings")
      body = parse_body_until(RPAREN)
      expect(RPAREN, "Expected ')' to close let")
      AST::Let.new(bindings: bindings, body: body, line: ln, column: col)
    end

    def parse_do(ln, col)
      advance
      exprs = []
      exprs << parse_expression until check(RPAREN)
      expect(RPAREN, "Expected ')' to close do")
      AST::Do.new(expressions: exprs, line: ln, column: col)
    end

    def parse_set_bang(ln, col)
      advance
      name_tok = expect(IDENTIFIER, "Expected variable name after set!")
      value = parse_expression
      expect(RPAREN, "Expected ')' to close set!")
      AST::SetBang.new(name: name_tok.lexeme, value: value, line: ln, column: col)
    end

    def parse_quote(ln, col)
      advance
      expr = parse_expression
      expect(RPAREN, "Expected ')' to close quote")
      AST::Quote.new(expression: expr, line: ln, column: col)
    end

    def parse_try_catch(ln, col)
      advance
      body = parse_expression
      expect(LPAREN, "Expected '(catch ...'")
      catch_tok = expect(IDENTIFIER, "Expected 'catch'")
      unless catch_tok.lexeme == "catch"
        raise Moof::SyntaxError.new("Expected 'catch', got #{catch_tok.lexeme.inspect}",
          line: catch_tok.line, column: catch_tok.column)
      end
      var_tok = expect(IDENTIFIER, "Expected error variable name")
      handler = parse_expression
      expect(RPAREN, "Expected ')' to close catch")
      expect(RPAREN, "Expected ')' to close try")
      AST::TryCatch.new(body: body, error_name: var_tok.lexeme, catch_body: handler,
        line: ln, column: col)
    end

    def parse_cond(ln, col)
      advance
      clauses = []
      while check(LPAREN)
        advance
        if check(IDENTIFIER) && current.lexeme == "else"
          advance
          expr = parse_expression
          expect(RPAREN, "Expected ')' to close else clause")
          clauses << [:else, expr]
        else
          test = parse_expression
          expr = parse_expression
          expect(RPAREN, "Expected ')' to close cond clause")
          clauses << [test, expr]
        end
      end
      expect(RPAREN, "Expected ')' to close cond")
      AST::Cond.new(clauses: clauses, line: ln, column: col)
    end

    def parse_and(ln, col)
      advance
      left  = parse_expression
      right = parse_expression
      expect(RPAREN, "Expected ')' to close and")
      AST::And.new(left: left, right: right, line: ln, column: col)
    end

    def parse_or(ln, col)
      advance
      left  = parse_expression
      right = parse_expression
      expect(RPAREN, "Expected ')' to close or")
      AST::Or.new(left: left, right: right, line: ln, column: col)
    end

    def parse_class(ln, col)
      advance
      name_tok = expect(IDENTIFIER, "Expected class name")
      superclass = nil
      fields = []
      methods = []
      traits = []

      until check(RPAREN)
        expect(LPAREN, "Expected '(' in class body")
        clause_tok = expect(IDENTIFIER, "Expected clause keyword")
        case clause_tok.lexeme
        when "extends"
          super_tok = expect(IDENTIFIER, "Expected superclass name")
          superclass = super_tok.lexeme
          expect(RPAREN, "Expected ')' to close extends")
        when "fields"
          while check(IDENTIFIER)
            fields << advance.lexeme
          end
          expect(RPAREN, "Expected ')' to close fields")
        when "method"
          methods << parse_method_body(clause_tok.line, clause_tok.column)
          expect(RPAREN, "Expected ')' to close method clause")
        when "uses"
          trait_tok = expect(IDENTIFIER, "Expected trait name")
          traits << trait_tok.lexeme
          expect(RPAREN, "Expected ')' to close uses")
        else
          raise Moof::SyntaxError.new("Unknown class clause: #{clause_tok.lexeme}",
            line: clause_tok.line, column: clause_tok.column)
        end
      end

      expect(RPAREN, "Expected ')' to close class")
      AST::ClassDef.new(name: name_tok.lexeme, superclass: superclass, fields: fields,
        methods: methods, traits: traits, line: ln, column: col)
    end

    def parse_trait(ln, col)
      advance
      name_tok = expect(IDENTIFIER, "Expected trait name")
      methods = []
      until check(RPAREN)
        expect(LPAREN, "Expected '(' in trait body")
        method_kw = expect(IDENTIFIER, "Expected 'method'")
        unless method_kw.lexeme == "method"
          raise Moof::SyntaxError.new("Expected 'method', got #{method_kw.lexeme}",
            line: method_kw.line, column: method_kw.column)
        end
        methods << parse_method_body(method_kw.line, method_kw.column)
        expect(RPAREN, "Expected ')' to close method in trait")
      end
      expect(RPAREN, "Expected ')' to close trait")
      AST::TraitDef.new(name: name_tok.lexeme, methods: methods, line: ln, column: col)
    end

    def parse_method_body(ln, col)
      if check(IDENTIFIER)
        sel_tok = advance
        selector = sel_tok.lexeme
        expect(LBRACKET, "Expected '[' for method params")
        params, _rest = parse_params_until(RBRACKET)
        expect(RBRACKET, "Expected ']' after method params")
      elsif check(COLON_ID)
        selector = +""
        params = []
        while check(COLON_ID)
          selector << advance.lexeme
          expect(LBRACKET, "Expected '[' for keyword param")
          p, _ = parse_params_until(RBRACKET)
          params.concat(p)
          expect(RBRACKET, "Expected ']' after keyword param")
        end
      else
        raise Moof::SyntaxError.new("Expected method selector", line: current.line, column: current.column)
      end
      body = parse_body_until(RPAREN)
      AST::MethodDef.new(selector: selector, params: params, body: body, line: ln, column: col)
    end

    def parse_message_send
      lbracket = expect(LBRACKET, "Expected '['")
      ln, col = lbracket.line, lbracket.column
      receiver = parse_expression

      if check(RBRACKET)
        raise Moof::SyntaxError.new("Message send requires a selector",
          line: current.line, column: current.column)
      end

      if check(COLON_ID)
        selector, args = parse_keyword_message
      elsif check(IDENTIFIER)
        first_id = advance
        if check(RBRACKET)
          selector = first_id.lexeme
          args = []
        elsif check(COLON_ID)
          selector = first_id.lexeme
          args = []
          while check(COLON_ID)
            selector += advance.lexeme
            args << parse_expression
          end
        else
          selector = first_id.lexeme
          args = []
          until check(RBRACKET)
            args << parse_expression
          end
        end
      else
        raise Moof::SyntaxError.new("Expected selector after receiver",
          line: current.line, column: current.column)
      end

      expect(RBRACKET, "Expected ']' to close message send")
      AST::MessageSend.new(receiver: receiver, selector: selector, arguments: args, line: ln, column: col)
    end

    def parse_keyword_message
      selector = +""
      args = []
      while check(COLON_ID)
        selector << advance.lexeme
        args << parse_expression
      end
      [selector, args]
    end

    def parse_map_literal
      lbrace = expect(LBRACE, "Expected '{'")
      ln, col = lbrace.line, lbrace.column
      pairs = []
      until check(RBRACE)
        key_tok = expect(COLON_ID, "Expected key: in map literal")
        key = key_tok.lexeme.chomp(":")
        value = parse_expression
        pairs << [key, value]
      end
      expect(RBRACE, "Expected '}' to close map")
      AST::MapLiteral.new(pairs: pairs, line: ln, column: col)
    end

    def parse_quote_sugar
      qt = advance
      expr = parse_expression
      AST::Quote.new(expression: expr, line: qt.line, column: qt.column)
    end

    def parse_body_until(end_type)
      exprs = []
      exprs << parse_expression until check(end_type)
      exprs.length == 1 ? exprs.first : AST::Do.new(expressions: exprs)
    end

    def parse_params_until(end_type)
      params = []
      rest_param = nil
      until check(end_type)
        if check(:DOT)
          advance
          rest_tok = expect(IDENTIFIER, "Expected rest param name after '.'")
          rest_param = rest_tok.lexeme
          break
        end
        params << expect(IDENTIFIER, "Expected parameter name").lexeme
      end
      [params, rest_param]
    end
  end
end
