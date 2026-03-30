require_relative "environment"
require_relative "function"
require_relative "values"
require_relative "dispatcher"
require_relative "builtins"
require_relative "object_system"

module Moof
  class Interpreter
    attr_reader :dispatcher, :global_env, :class_registry, :trait_registry

    def initialize
      @dispatcher     = Dispatcher.new(self)
      @global_env     = Environment.new
      @class_registry = {}
      @trait_registry = {}
      register_builtin_classes
      Builtins.install(@global_env, self)
    end

    def evaluate(program)
      result = nil
      program.expressions.each { |expr| result = evaluate_node(expr, @global_env) }
      result
    end

    def evaluate_node(node, env)
      case node
      when AST::IntegerLiteral then node.value
      when AST::FloatLiteral   then node.value
      when AST::StringLiteral  then node.value
      when AST::BoolLiteral    then node.value
      when AST::NilLiteral     then nil

      when AST::Identifier
        begin
          env.get(node.name)
        rescue Moof::NameError
          raise Moof::NameError.new(node.name, line: node.line, column: node.column)
        end

      when AST::Quote
        quote_value(node.expression)

      when AST::MapLiteral
        result = {}
        node.pairs.each { |key, vn| result[key] = evaluate_node(vn, env) }
        result

      when AST::Define
        value = evaluate_node(node.value, env)
        env.define(node.name, value)
        value

      when AST::DefineFunction
        func = Function.new(params: node.params, rest_param: node.rest_param,
          body: node.body, closure: env, name: node.name)
        env.define(node.name, func)
        func

      when AST::Lambda
        Function.new(params: node.params, rest_param: node.rest_param,
          body: node.body, closure: env)

      when AST::If
        condition = evaluate_node(node.condition, env)
        if truthy?(condition)
          evaluate_node(node.then_branch, env)
        elsif node.else_branch
          evaluate_node(node.else_branch, env)
        end

      when AST::Let
        let_env = env.child
        node.bindings.each do |name, value_node|
          let_env.define(name, evaluate_node(value_node, env), mutable: false)
        end
        evaluate_node(node.body, let_env)

      when AST::Do
        result = nil
        node.expressions.each { |expr| result = evaluate_node(expr, env) }
        result

      when AST::SetBang
        value = evaluate_node(node.value, env)
        env.set(node.name, value)
        value

      when AST::TryCatch
        begin
          evaluate_node(node.body, env)
        rescue Moof::RuntimeError => e
          catch_env = env.child
          catch_env.define(node.error_name, e.message)
          evaluate_node(node.catch_body, catch_env)
        end

      when AST::Cond
        node.clauses.each do |test_node, body_node|
          if test_node == :else || (test_node.is_a?(AST::Identifier) && test_node.name == "else")
            return evaluate_node(body_node, env)
          end
          return evaluate_node(body_node, env) if truthy?(evaluate_node(test_node, env))
        end
        nil

      when AST::And
        left = evaluate_node(node.left, env)
        return false unless truthy?(left)
        right = evaluate_node(node.right, env)
        truthy?(right) ? right : false

      when AST::Or
        left = evaluate_node(node.left, env)
        return left if truthy?(left)
        evaluate_node(node.right, env)

      when AST::Call
        callee = evaluate_node(node.callee, env)
        args = node.arguments.map { |arg| evaluate_node(arg, env) }
        if callee.is_a?(Moof::Function)
          callee.call(self, args)
        elsif callee.is_a?(Proc)
          callee.call(self, args)
        elsif callee.is_a?(Moof::MoofClass)
          callee.instantiate(args)
        else
          loc = node.line ? " at line #{node.line}" : ""
          raise Moof::RuntimeError, "Cannot call #{callee.inspect}#{loc}: not a function"
        end

      when AST::ClassDef
        superklass = nil
        if node.superclass
          superklass = @class_registry[node.superclass]
          raise Moof::NameError.new(node.superclass, line: node.line) unless superklass
        end
        # Build trait methods
        trait_methods_list = node.traits.map do |trait_name|
          tmethods = @trait_registry[trait_name]
          raise Moof::RuntimeError, "Unknown trait: #{trait_name}" unless tmethods
          tmethods
        end
        # Build new methods
        new_methods = {}
        node.methods.each do |mdef|
          func = Function.new(params: mdef.params, body: mdef.body, closure: env,
            name: "#{node.name}##{mdef.selector}")
          new_methods[mdef.selector] = func
        end
        # Open class: if class already exists, merge into it
        existing = @class_registry[node.name]
        if existing
          existing.reopen(
            new_fields: node.fields,
            new_methods: new_methods,
            new_superclass: superklass,
            new_traits: trait_methods_list
          )
          klass = existing
        else
          method_table = {}
          trait_methods_list.each { |tm| tm.each { |sel, f| method_table[sel] = f } }
          new_methods.each { |sel, f| method_table[sel] = f }
          klass = MoofClass.new(name: node.name, superclass: superklass, fields: node.fields,
            method_table: method_table)
          @class_registry[node.name] = klass
          env.define(node.name, klass)
        end
        klass

      when AST::TraitDef
        method_table = {}
        node.methods.each do |mdef|
          func = Function.new(params: mdef.params, body: mdef.body, closure: env,
            name: "#{node.name}##{mdef.selector}")
          method_table[mdef.selector] = func
        end
        @trait_registry[node.name] = method_table
        method_table

      else
        raise Moof::RuntimeError, "Unknown AST node type: #{node.class}"
      end
    end

    private

    # Register built-in types as MoofClass so they can be extended in Moof.
    # These start with empty method_tables — hardcoded dispatch in Dispatcher
    # handles the defaults. User-defined methods are checked FIRST by Dispatcher.
    def register_builtin_classes
      %w[Integer Float String List Map Bool Nil Function].each do |name|
        klass = MoofClass.new(name: name)
        @class_registry[name] = klass
        @global_env.define(name, klass)
      end
    end

    def truthy?(value)
      value != false && !value.nil?
    end

    def quote_value(node)
      case node
      when AST::Identifier      then SymbolValue.new(node.name)
      when AST::IntegerLiteral  then node.value
      when AST::FloatLiteral    then node.value
      when AST::StringLiteral   then node.value
      when AST::BoolLiteral     then node.value
      when AST::NilLiteral      then nil
      when AST::Call
        [quote_value(node.callee)] + node.arguments.map { |a| quote_value(a) }
      else
        node.respond_to?(:expressions) ? node.expressions.map { |e| quote_value(e) } : node
      end
    end
  end
end
