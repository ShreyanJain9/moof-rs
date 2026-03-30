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
      else
        node
      end
    end
  end
end
