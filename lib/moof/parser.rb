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
      when LBRACE   then parse_brace_expression
      when QUOTE    then parse_quote_sugar
      when BACKTICK then parse_quasiquote_sugar
      when COMMA_AT then parse_unquote_splice_sugar
      when COMMA    then parse_unquote_sugar
      when AMPERSAND then parse_selector_ref
      when INTEGER
        advance
        AST::IntegerLiteral.new(value: tok.literal, line: tok.line, column: tok.column)
      when FLOAT
        advance
        AST::FloatLiteral.new(value: tok.literal, line: tok.line, column: tok.column)
      when STRING
        advance
        AST::StringLiteral.new(value: tok.literal, line: tok.line, column: tok.column)
      when INTERP_STRING
        advance
        parse_interp_string_token(tok)
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
        when "define"   then return parse_define(ln, col)
        when "lambda"   then return parse_lambda(ln, col)
        when "if"       then return parse_if(ln, col)
        when "let"      then return parse_let(ln, col)
        when "do"       then return parse_do(ln, col)
        when "set!"     then return parse_set_bang(ln, col)
        when "quote"    then return parse_quote(ln, col)
        when "try"      then return parse_try_catch(ln, col)
        when "cond"     then return parse_cond(ln, col)
        when "and"      then return parse_and(ln, col)
        when "or"       then return parse_or(ln, col)
        when "class"    then return parse_class(ln, col)
        when "trait"    then return parse_trait(ln, col)
        when "match"    then return parse_match(ln, col)
        when "type"     then return parse_type_def(ln, col)
        when "->"       then return parse_pipeline(ln, col)
        when "protocol" then return parse_protocol(ln, col)
        when "extend"   then return parse_extend(ln, col)
        when "defmacro" then return parse_defmacro(ln, col)
        when "module"   then return parse_module(ln, col)
        when "use"      then return parse_use(ln, col)
        when "require"  then return parse_require(ln, col)
        end
      end

      parse_call(ln, col)
    end

    def parse_call(ln, col)
      callee = parse_expression
      args = []
      until check(RPAREN)
        # Check for keyword arguments: COLON_ID inside call
        if check(COLON_ID)
          kw_tok = advance
          keyword = kw_tok.lexeme.chomp(":")
          value = parse_expression
          args << AST::KeywordArg.new(keyword: keyword, value: value, line: kw_tok.line, column: kw_tok.column)
        else
          args << parse_expression
        end
      end
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

    # { ... } is either a map or a block depending on whether | follows {
    def parse_brace_expression
      lbrace = expect(LBRACE, "Expected '{'")
      ln, col = lbrace.line, lbrace.column

      # If { is followed by |, it's a block (short lambda)
      if check(PIPE)
        return parse_block(ln, col)
      end

      # Otherwise it's a map literal
      parse_map_literal_body(ln, col)
    end

    # Parse block: { |params| body... }
    def parse_block(ln, col)
      expect(PIPE, "Expected '|' to start block params")
      params = []
      until check(PIPE)
        params << expect(IDENTIFIER, "Expected block parameter name").lexeme
      end
      expect(PIPE, "Expected '|' to end block params")

      body_exprs = []
      body_exprs << parse_expression until check(RBRACE)
      expect(RBRACE, "Expected '}' to close block")

      body = body_exprs.length == 1 ? body_exprs.first : AST::Do.new(expressions: body_exprs, line: ln, column: col)
      AST::Lambda.new(params: params, body: body, line: ln, column: col)
    end

    def parse_map_literal_body(ln, col)
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

    # `expr -> Quasiquote
    def parse_quasiquote_sugar
      bt = advance  # consume backtick
      expr = parse_expression
      AST::Quasiquote.new(expression: expr, line: bt.line, column: bt.column)
    end

    # ,expr -> Unquote
    def parse_unquote_sugar
      c = advance  # consume comma
      expr = parse_expression
      AST::Unquote.new(expression: expr, line: c.line, column: c.column)
    end

    # ,@expr -> UnquoteSplice
    def parse_unquote_splice_sugar
      ca = advance  # consume ,@
      expr = parse_expression
      AST::UnquoteSplice.new(expression: expr, line: ca.line, column: ca.column)
    end

    # &name or &(keyword: arg ...)
    def parse_selector_ref
      amp = advance  # consume &
      ln, col = amp.line, amp.column

      if check(LPAREN)
        # &(keyword: arg ...)
        advance  # skip (
        selector = +""
        partial_args = []
        while check(COLON_ID)
          selector << advance.lexeme
          partial_args << parse_expression
        end
        expect(RPAREN, "Expected ')' to close selector ref")
        AST::SelectorRef.new(selector: selector, partial_args: partial_args, line: ln, column: col)
      elsif check(IDENTIFIER)
        name_tok = advance
        AST::SelectorRef.new(selector: name_tok.lexeme, partial_args: [], line: ln, column: col)
      else
        raise Moof::SyntaxError.new("Expected selector name after '&'",
          line: current.line, column: current.column)
      end
    end

    # (match expr clauses...)
    def parse_match(ln, col)
      advance  # skip 'match'
      expr = parse_expression
      clauses = []
      while check(LPAREN)
        clauses << parse_match_clause
      end
      expect(RPAREN, "Expected ')' to close match")
      AST::Match.new(expr: expr, clauses: clauses, line: ln, column: col)
    end

    def parse_match_clause
      expect(LPAREN, "Expected '(' for match clause")
      pattern = parse_pattern
      guard = nil

      # Check for 'when' guard
      if check(IDENTIFIER) && current.lexeme == "when"
        advance  # skip 'when'
        guard = parse_expression
      end

      body = parse_expression
      expect(RPAREN, "Expected ')' to close match clause")
      AST::MatchClause.new(pattern: pattern, guard: guard, body: body)
    end

    def parse_pattern
      tok = current
      case tok.type
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
        if tok.lexeme == "_"
          AST::MatchWildcard.new(line: tok.line, column: tok.column)
        else
          AST::MatchBind.new(name: tok.lexeme, line: tok.line, column: tok.column)
        end
      when LBRACE
        parse_map_pattern
      when LPAREN
        parse_list_or_constructor_pattern
      else
        raise Moof::SyntaxError.new("Unexpected token in pattern: #{tok.type}(#{tok.lexeme.inspect})",
          line: tok.line, column: tok.column)
      end
    end

    # {key: pattern ...}
    def parse_map_pattern
      expect(LBRACE, "Expected '{' for map pattern")
      pairs = []
      until check(RBRACE)
        key_tok = expect(COLON_ID, "Expected key: in map pattern")
        key = key_tok.lexeme.chomp(":")
        pat = parse_pattern
        pairs << [key, pat]
      end
      expect(RBRACE, "Expected '}' to close map pattern")
      AST::MatchMap.new(pairs: pairs)
    end

    # (ClassName binding1 binding2) or (elem1 elem2 . rest) or (list a b . rest)
    def parse_list_or_constructor_pattern
      expect(LPAREN, "Expected '(' for pattern")

      # Check first token
      if check(IDENTIFIER) && current.lexeme =~ /\A[A-Z]/
        # Constructor pattern: (ClassName binding1 binding2)
        class_tok = advance
        bindings = []
        until check(RPAREN)
          bindings << parse_pattern
        end
        expect(RPAREN, "Expected ')' to close constructor pattern")
        return AST::MatchConstructor.new(class_name: class_tok.lexeme, bindings: bindings,
          line: class_tok.line, column: class_tok.column)
      end

      # If starts with "list", treat as explicit list pattern: (list a b c)
      if check(IDENTIFIER) && current.lexeme == "list"
        advance  # skip "list"
      end

      # List pattern: (elem1 elem2 . rest) or (elem1 elem2 elem3)
      elements = []
      rest = nil
      until check(RPAREN)
        if check(DOT)
          advance  # skip dot
          rest = parse_pattern
          break
        end
        elements << parse_pattern
      end
      expect(RPAREN, "Expected ')' to close list pattern")
      AST::MatchList.new(elements: elements, rest: rest)
    end

    # (type Name (Variant1 field1 field2) Variant2 ...)
    def parse_type_def(ln, col)
      advance  # skip 'type'
      name_tok = expect(IDENTIFIER, "Expected type name")
      variants = []

      until check(RPAREN)
        if check(LPAREN)
          advance  # skip (
          variant_tok = expect(IDENTIFIER, "Expected variant name")
          fields = []
          while check(IDENTIFIER)
            fields << advance.lexeme
          end
          expect(RPAREN, "Expected ')' to close variant")
          variants << AST::TypeVariant.new(name: variant_tok.lexeme, fields: fields,
            line: variant_tok.line, column: variant_tok.column)
        elsif check(IDENTIFIER)
          variant_tok = advance
          variants << AST::TypeVariant.new(name: variant_tok.lexeme, fields: [],
            line: variant_tok.line, column: variant_tok.column)
        else
          raise Moof::SyntaxError.new("Expected variant in type definition",
            line: current.line, column: current.column)
        end
      end

      expect(RPAREN, "Expected ')' to close type")
      AST::TypeDef.new(name: name_tok.lexeme, variants: variants, line: ln, column: col)
    end

    # (-> value step1 step2 ...)
    def parse_pipeline(ln, col)
      advance  # skip '->'
      value = parse_expression
      steps = []
      until check(RPAREN)
        if check(LBRACKET)
          # Parse as receiverless message send — use placeholder receiver
          steps << parse_pipeline_message_step
        else
          steps << parse_expression
        end
      end
      expect(RPAREN, "Expected ')' to close pipeline")
      AST::Pipeline.new(value: value, steps: steps, line: ln, column: col)
    end

    # Parse [selector: arg ...] without a receiver — used in pipeline steps
    def parse_pipeline_message_step
      lbracket = expect(LBRACKET, "Expected '['")
      ln, col = lbracket.line, lbracket.column

      # Use a placeholder that the normalizer will replace with the piped value
      placeholder = AST::Identifier.new(name: "__pipeline_placeholder__", line: ln, column: col)

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
        raise Moof::SyntaxError.new("Expected selector in pipeline message step",
          line: current.line, column: current.column)
      end

      expect(RBRACKET, "Expected ']' to close pipeline message step")
      AST::MessageSend.new(receiver: placeholder, selector: selector, arguments: args, line: ln, column: col)
    end

    # (protocol Name selector1 selector2 ...)
    def parse_protocol(ln, col)
      advance  # skip 'protocol'
      name_tok = expect(IDENTIFIER, "Expected protocol name")
      selectors = []
      until check(RPAREN)
        if check(IDENTIFIER)
          selectors << advance.lexeme
        elsif check(COLON_ID)
          selectors << advance.lexeme
        else
          raise Moof::SyntaxError.new("Expected selector in protocol",
            line: current.line, column: current.column)
        end
      end
      expect(RPAREN, "Expected ')' to close protocol")
      AST::ProtocolDef.new(name: name_tok.lexeme, selectors: selectors, line: ln, column: col)
    end

    # (extend ClassName (method ...) ...)
    def parse_extend(ln, col)
      advance  # skip 'extend'
      name_tok = expect(IDENTIFIER, "Expected class name after extend")
      methods = []

      until check(RPAREN)
        expect(LPAREN, "Expected '(' in extend body")
        method_kw = expect(IDENTIFIER, "Expected 'method'")
        unless method_kw.lexeme == "method"
          raise Moof::SyntaxError.new("Expected 'method' in extend, got #{method_kw.lexeme}",
            line: method_kw.line, column: method_kw.column)
        end
        methods << parse_method_body(method_kw.line, method_kw.column)
        expect(RPAREN, "Expected ')' to close method in extend")
      end

      expect(RPAREN, "Expected ')' to close extend")
      AST::ClassDef.new(name: name_tok.lexeme, fields: [], methods: methods, traits: [],
        line: ln, column: col)
    end

    # (defmacro name (params...) body)
    def parse_defmacro(ln, col)
      advance  # skip 'defmacro'
      name_tok = expect(IDENTIFIER, "Expected macro name")
      expect(LPAREN, "Expected '(' for macro params")
      params = []
      rest_param = nil
      until check(RPAREN)
        if check(DOT)
          advance
          rest_tok = expect(IDENTIFIER, "Expected rest param name after '.'")
          rest_param = rest_tok.lexeme
          break
        end
        params << expect(IDENTIFIER, "Expected parameter name").lexeme
      end
      expect(RPAREN, "Expected ')' after macro params")
      body = parse_body_until(RPAREN)
      expect(RPAREN, "Expected ')' to close defmacro")
      AST::DefMacro.new(name: name_tok.lexeme, params: params, body: body, line: ln, column: col)
    end

    # Parse an INTERP_STRING token into a StringInterp node
    def parse_interp_string_token(tok)
      segments = tok.literal.map do |type, content|
        case type
        when :str
          AST::StringLiteral.new(value: content, line: tok.line, column: tok.column)
        when :expr
          # Lex and parse the expression source
          inner_tokens = Lexer.new(content).tokenize
          inner_parser = Parser.new(inner_tokens)
          inner_parser.send(:parse_expression)
        end
      end
      AST::StringInterp.new(segments: segments, line: tok.line, column: tok.column)
    end

    def parse_body_until(end_type)
      exprs = []
      exprs << parse_expression until check(end_type)
      exprs.length == 1 ? exprs.first : AST::Do.new(expressions: exprs)
    end

    # (module name (export n1 n2 ...) body...)
    def parse_module(ln, col)
      advance # skip 'module'
      name_tok = expect(IDENTIFIER, "Expected module name")
      exports = []
      body = []

      # Check for (export ...) clause
      if check(LPAREN)
        # Peek to see if it's (export ...)
        saved_pos = @pos
        advance # skip (
        if check(IDENTIFIER) && current.lexeme == "export"
          advance # skip 'export'
          exports << advance.lexeme while check(IDENTIFIER) || check(COLON_ID)
          expect(RPAREN, "Expected ')' to close export list")
        else
          @pos = saved_pos # backtrack — it's body, not export
        end
      end

      # Parse body expressions
      body << parse_expression until check(RPAREN)
      expect(RPAREN, "Expected ')' to close module")
      AST::ModuleDef.new(name: name_tok.lexeme, exports: exports, body: body, line: ln, column: col)
    end

    # (use module-name) / (use module-name (name1 name2)) / (use module-name :as alias)
    def parse_use(ln, col)
      advance # skip 'use'
      name_tok = expect(IDENTIFIER, "Expected module name")
      imports = nil
      alias_name = nil

      unless check(RPAREN)
        if check(COLON_ID) && current.lexeme == "as:"
          advance # skip as:
          alias_tok = expect(IDENTIFIER, "Expected alias name")
          alias_name = alias_tok.lexeme
        elsif check(LPAREN)
          advance # skip (
          imports = []
          imports << advance.lexeme while check(IDENTIFIER) || check(COLON_ID)
          expect(RPAREN, "Expected ')' to close import list")
        end
      end

      expect(RPAREN, "Expected ')' to close use")
      AST::UseModule.new(module_name: name_tok.lexeme, imports: imports, alias_name: alias_name, line: ln, column: col)
    end

    # (require "path/to/file")
    def parse_require(ln, col)
      advance # skip 'require'
      path_tok = expect(STRING, "Expected file path string")
      expect(RPAREN, "Expected ')' to close require")
      AST::Require.new(path: path_tok.literal, line: ln, column: col)
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
