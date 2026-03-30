require_relative "ast"

module Moof
  class Normalizer
    def call(program)
      AST::Program.new(expressions: program.expressions.map { |e| normalize(e) })
    end

    private

    def normalize(node)
      case node
      when AST::MessageSend
        receiver = normalize(node.receiver)
        args = node.arguments.map { |a| normalize(a) }
        AST::Call.new(
          callee: AST::Identifier.new(name: "__send", line: node.line, column: node.column),
          arguments: [receiver, AST::StringLiteral.new(value: node.selector, line: node.line, column: node.column), *args],
          line: node.line, column: node.column
        )
      when AST::Call
        AST::Call.new(callee: normalize(node.callee),
          arguments: node.arguments.map { |a| normalize(a) },
          line: node.line, column: node.column)
      when AST::Define
        AST::Define.new(name: node.name, value: normalize(node.value), line: node.line, column: node.column)
      when AST::DefineFunction
        AST::DefineFunction.new(name: node.name, params: node.params, rest_param: node.rest_param,
          body: normalize(node.body), line: node.line, column: node.column)
      when AST::Lambda
        AST::Lambda.new(params: node.params, rest_param: node.rest_param,
          body: normalize(node.body), line: node.line, column: node.column)
      when AST::If
        AST::If.new(condition: normalize(node.condition),
          then_branch: normalize(node.then_branch),
          else_branch: node.else_branch ? normalize(node.else_branch) : nil,
          line: node.line, column: node.column)
      when AST::Let
        bindings = node.bindings.map { |name, val| [name, normalize(val)] }
        AST::Let.new(bindings: bindings, body: normalize(node.body), line: node.line, column: node.column)
      when AST::Do
        AST::Do.new(expressions: node.expressions.map { |e| normalize(e) }, line: node.line, column: node.column)
      when AST::SetBang
        AST::SetBang.new(name: node.name, value: normalize(node.value), line: node.line, column: node.column)
      when AST::Quote
        node
      when AST::TryCatch
        AST::TryCatch.new(body: normalize(node.body), error_name: node.error_name,
          catch_body: normalize(node.catch_body), line: node.line, column: node.column)
      when AST::Cond
        clauses = node.clauses.map do |test_node, body_node|
          if test_node == :else || (test_node.is_a?(AST::Identifier) && test_node.name == "else")
            [:else, normalize(body_node)]
          else
            [normalize(test_node), normalize(body_node)]
          end
        end
        AST::Cond.new(clauses: clauses, line: node.line, column: node.column)
      when AST::And
        AST::And.new(left: normalize(node.left), right: normalize(node.right), line: node.line, column: node.column)
      when AST::Or
        AST::Or.new(left: normalize(node.left), right: normalize(node.right), line: node.line, column: node.column)
      when AST::MapLiteral
        pairs = node.pairs.map { |key, val| [key, normalize(val)] }
        AST::MapLiteral.new(pairs: pairs, line: node.line, column: node.column)
      when AST::Program
        AST::Program.new(expressions: node.expressions.map { |e| normalize(e) })
      when AST::ClassDef
        methods = node.methods.map { |m|
          AST::MethodDef.new(selector: m.selector, params: m.params, body: normalize(m.body),
            line: m.line, column: m.column)
        }
        AST::ClassDef.new(name: node.name, superclass: node.superclass, fields: node.fields,
          methods: methods, traits: node.traits, line: node.line, column: node.column)
      when AST::TraitDef
        methods = node.methods.map { |m|
          AST::MethodDef.new(selector: m.selector, params: m.params, body: normalize(m.body),
            line: m.line, column: m.column)
        }
        AST::TraitDef.new(name: node.name, methods: methods, line: node.line, column: node.column)

      # ============ New node normalization ============

      # Pipeline: desugar (-> val step1 step2) into nested calls
      when AST::Pipeline
        desugar_pipeline(node)

      # StringInterp: desugar into nested concat calls
      when AST::StringInterp
        desugar_string_interp(node)

      # SelectorRef: desugar &name into lambda
      when AST::SelectorRef
        desugar_selector_ref(node)

      # KeywordArg: normalize the value
      when AST::KeywordArg
        AST::KeywordArg.new(keyword: node.keyword, value: normalize(node.value),
          line: node.line, column: node.column)

      # Match: recursively normalize children
      when AST::Match
        clauses = node.clauses.map do |clause|
          AST::MatchClause.new(
            pattern: normalize_pattern(clause.pattern),
            guard: clause.guard ? normalize(clause.guard) : nil,
            body: normalize(clause.body)
          )
        end
        AST::Match.new(expr: normalize(node.expr), clauses: clauses, line: node.line, column: node.column)

      # TypeDef: pass through (no desugaring needed)
      when AST::TypeDef
        node

      # ProtocolDef: pass through
      when AST::ProtocolDef
        node

      # DefMacro: recursively normalize body
      when AST::DefMacro
        AST::DefMacro.new(name: node.name, params: node.params,
          body: normalize(node.body), line: node.line, column: node.column)

      # Quasiquote: recursively normalize
      when AST::Quasiquote
        AST::Quasiquote.new(expression: normalize(node.expression), line: node.line, column: node.column)

      when AST::Unquote
        AST::Unquote.new(expression: normalize(node.expression), line: node.line, column: node.column)

      when AST::UnquoteSplice
        AST::UnquoteSplice.new(expression: normalize(node.expression), line: node.line, column: node.column)

      when AST::ModuleDef
        AST::ModuleDef.new(name: node.name, exports: node.exports,
          body: node.body.map { |e| normalize(e) }, line: node.line, column: node.column)

      when AST::UseModule, AST::Require
        node

      else
        node
      end
    end

    # Normalize pattern nodes (most are leaf nodes, but some contain sub-patterns)
    def normalize_pattern(pat)
      case pat
      when AST::MatchList
        elements = pat.elements.map { |e| normalize_pattern(e) }
        rest = pat.rest ? normalize_pattern(pat.rest) : nil
        AST::MatchList.new(elements: elements, rest: rest, line: pat.line, column: pat.column)
      when AST::MatchMap
        pairs = pat.pairs.map { |k, p| [k, normalize_pattern(p)] }
        AST::MatchMap.new(pairs: pairs, line: pat.line, column: pat.column)
      when AST::MatchConstructor
        bindings = pat.bindings.map { |b| normalize_pattern(b) }
        AST::MatchConstructor.new(class_name: pat.class_name, bindings: bindings,
          line: pat.line, column: pat.column)
      else
        pat
      end
    end

    # Desugar (-> val step1 step2) into nested calls
    # For each step:
    #   - MessageSend: inject previous as receiver
    #   - Call: inject previous as first argument
    #   - Identifier: wrap in Call(step, [previous])
    def desugar_pipeline(node)
      result = normalize(node.value)
      node.steps.each do |step|
        result = pipeline_inject(step, result, node.line, node.column)
      end
      result
    end

    def pipeline_inject(step, prev, ln, col)
      case step
      when AST::MessageSend
        # Inject prev as receiver, then normalize
        injected = AST::MessageSend.new(
          receiver: prev, selector: step.selector,
          arguments: step.arguments, line: step.line, column: step.column
        )
        normalize(injected)
      when AST::Call
        # Inject prev as first argument
        normalized_callee = normalize(step.callee)
        normalized_args = step.arguments.map { |a| normalize(a) }
        AST::Call.new(
          callee: normalized_callee,
          arguments: [prev, *normalized_args],
          line: step.line, column: step.column
        )
      when AST::Identifier
        # Wrap in Call(step, [prev])
        AST::Call.new(
          callee: step,
          arguments: [prev],
          line: step.line, column: step.column
        )
      else
        # For any other expression type, wrap as a call with prev as argument
        normalized_step = normalize(step)
        AST::Call.new(
          callee: normalized_step,
          arguments: [prev],
          line: ln, column: col
        )
      end
    end

    # Desugar $"hello \(name)" into nested concat calls via __send
    def desugar_string_interp(node)
      ln, col = node.line, node.column
      parts = node.segments.map do |seg|
        normalized = normalize(seg)
        if seg.is_a?(AST::StringLiteral)
          normalized
        else
          # Call to_s on expression: (__send expr "to_s")
          make_send(normalized, "to_s", [], ln, col)
        end
      end

      return AST::StringLiteral.new(value: "", line: ln, column: col) if parts.empty?
      return parts.first if parts.length == 1

      # Chain concat calls: (__send (__send part1 "concat:" part2) "concat:" part3)
      result = parts.first
      parts[1..].each do |part|
        result = make_send(result, "concat:", [part], ln, col)
      end
      result
    end

    # Desugar &name into lambda
    def desugar_selector_ref(node)
      ln, col = node.line, node.column
      obj_param = "__obj"
      obj_id = AST::Identifier.new(name: obj_param, line: ln, column: col)

      body = make_send(obj_id, node.selector, node.partial_args.map { |a| normalize(a) }, ln, col)
      AST::Lambda.new(params: [obj_param], body: body, line: ln, column: col)
    end

    # Helper: make a __send call
    def make_send(receiver, selector, args, ln, col)
      AST::Call.new(
        callee: AST::Identifier.new(name: "__send", line: ln, column: col),
        arguments: [receiver, AST::StringLiteral.new(value: selector, line: ln, column: col), *args],
        line: ln, column: col
      )
    end
  end
end
