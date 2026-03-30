require_relative "environment"
require_relative "function"
require_relative "values"
require_relative "dispatcher"
require_relative "builtins"
require_relative "object_system"
require_relative "pattern_matcher"
require_relative "protocol"

module Moof
  class Interpreter
    attr_reader :dispatcher, :global_env, :class_registry, :trait_registry,
                :protocol_registry, :macro_registry, :type_registry

    def initialize
      @dispatcher        = Dispatcher.new(self)
      @global_env        = Environment.new
      @class_registry    = {}
      @trait_registry    = {}
      @protocol_registry = {}
      @macro_registry    = {}
      @type_registry     = {}
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

      when AST::Quasiquote
        quasiquote_eval(node.expression, env)

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
        # Check for macro expansion first
        if node.callee.is_a?(AST::Identifier) && @macro_registry.key?(node.callee.name)
          return expand_and_eval_macro(node, env)
        end

        callee = evaluate_node(node.callee, env)
        args = evaluate_call_args(node.arguments, env)
        invoke_callee(callee, args, node)

      when AST::Match
        val = evaluate_node(node.expr, env)
        node.clauses.each do |clause|
          result = PatternMatcher.match(clause.pattern, val, self)
          if result[:success]
            match_env = env.child
            result[:bindings].each { |name, v| match_env.define(name, v) }
            if clause.guard
              next unless truthy?(evaluate_node(clause.guard, match_env))
            end
            return evaluate_node(clause.body, match_env)
          end
        end
        nil

      when AST::TypeDef
        variant_classes = []
        node.variants.each do |variant|
          if variant.fields.empty?
            # Singleton variant (like None)
            klass = MoofClass.new(name: variant.name, fields: [])
            @class_registry[variant.name] = klass
            singleton = klass.instantiate([])
            env.define(variant.name, singleton)
          else
            # Constructor variant (like Some)
            klass = MoofClass.new(name: variant.name, fields: variant.fields)
            @class_registry[variant.name] = klass
            env.define(variant.name, klass)
          end
          variant_classes << variant.name
        end
        @type_registry[node.name] = variant_classes
        node.name

      when AST::ProtocolDef
        proto = Protocol.new(name: node.name, selectors: node.selectors)
        @protocol_registry[node.name] = proto
        env.define(node.name, proto)
        proto

      when AST::DefMacro
        @macro_registry[node.name] = node
        env.define(node.name, node)
        node.name

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

      when AST::ModuleDef
        evaluate_module(node, env)

      when AST::UseModule
        import_module(node, env)

      when AST::Require
        require_file(node, env)

      else
        raise Moof::RuntimeError, "Unknown AST node type: #{node.class}"
      end
    end

    # evaluate_tail is like evaluate_node but returns TailCall for function calls
    # in tail position, enabling tail call optimization via the trampoline in Function#call.
    def evaluate_tail(node, env)
      case node
      when AST::Call
        # Check for macro expansion first
        if node.callee.is_a?(AST::Identifier) && @macro_registry.key?(node.callee.name)
          return expand_and_eval_macro(node, env)
        end

        callee = evaluate_node(node.callee, env)
        args = evaluate_call_args(node.arguments, env)
        if callee.is_a?(Moof::Function)
          TailCall.new(callee, args)
        else
          invoke_callee(callee, args, node)
        end

      when AST::If
        cond = evaluate_node(node.condition, env)
        if truthy?(cond)
          evaluate_tail(node.then_branch, env)
        elsif node.else_branch
          evaluate_tail(node.else_branch, env)
        end

      when AST::Do
        return nil if node.expressions.empty?
        node.expressions[0...-1].each { |e| evaluate_node(e, env) }
        evaluate_tail(node.expressions.last, env)

      when AST::Let
        let_env = env.child
        node.bindings.each { |name, vn| let_env.define(name, evaluate_node(vn, env), mutable: false) }
        evaluate_tail(node.body, let_env)

      when AST::Match
        val = evaluate_node(node.expr, env)
        node.clauses.each do |clause|
          result = PatternMatcher.match(clause.pattern, val, self)
          if result[:success]
            match_env = env.child
            result[:bindings].each { |n, v| match_env.define(n, v) }
            if clause.guard
              next unless truthy?(evaluate_node(clause.guard, match_env))
            end
            return evaluate_tail(clause.body, match_env)
          end
        end
        nil

      when AST::Cond
        node.clauses.each do |test_node, body_node|
          if test_node == :else || (test_node.is_a?(AST::Identifier) && test_node.name == "else")
            return evaluate_tail(body_node, env)
          end
          return evaluate_tail(body_node, env) if truthy?(evaluate_node(test_node, env))
        end
        nil

      else
        evaluate_node(node, env)
      end
    end

    attr_reader :module_registry

    private

    def evaluate_module(node, env)
      @module_registry ||= {}
      mod_env = Environment.new(@global_env)  # modules see globals but get their own scope

      # Evaluate all body expressions in the module env
      node.body.each { |expr| evaluate_node(expr, mod_env) }

      # Collect exports
      exported = {}
      if node.exports.empty?
        # Export everything defined in the module
        mod_env.bindings.each { |name, val| exported[name] = val }
      else
        node.exports.each do |name|
          begin
            exported[name] = mod_env.get(name)
          rescue Moof::NameError
            raise Moof::RuntimeError, "Module '#{node.name}' exports '#{name}' but it is not defined"
          end
        end
      end

      @module_registry[node.name] = exported
      env.define(node.name, exported)  # module itself is available as a map-like value
      exported
    end

    def import_module(node, env)
      @module_registry ||= {}
      mod = @module_registry[node.module_name]
      raise Moof::RuntimeError, "Unknown module: #{node.module_name}" unless mod

      if node.alias_name
        # (use foo :as f) — define alias as a hash for qualified access
        env.define(node.alias_name, mod)
      elsif node.imports
        # (use foo (bar baz)) — import specific names
        node.imports.each do |name|
          raise Moof::RuntimeError, "Module '#{node.module_name}' does not export '#{name}'" unless mod.key?(name)
          env.define(name, mod[name])
        end
      else
        # (use foo) — import all exports
        mod.each { |name, val| env.define(name, val) }
      end
      nil
    end

    def require_file(node, env)
      path = node.path
      path += ".moof" unless path.end_with?(".moof")

      # Search relative to the current working directory and the stdlib directory
      full_path = if File.exist?(path)
        path
      elsif File.exist?(File.join(File.dirname(Moof::STDLIB_PATH), path))
        File.join(File.dirname(Moof::STDLIB_PATH), path)
      else
        raise Moof::RuntimeError, "Cannot find file: #{path}"
      end

      source = File.read(full_path)
      tokens = Lexer.new(source, filename: full_path).tokenize
      program = Parser.new(tokens).parse_program
      normalized = Normalizer.new.call(program)
      evaluate(normalized)
    end

    def evaluate_call_args(arguments, env)
      arguments.map do |arg|
        if arg.is_a?(AST::KeywordArg)
          evaluate_node(arg.value, env)
        else
          evaluate_node(arg, env)
        end
      end
    end

    def invoke_callee(callee, args, node)
      if callee.is_a?(Moof::Function)
        callee.call(self, args)
      elsif callee.is_a?(Proc)
        callee.call(self, args)
      elsif callee.is_a?(Moof::MoofClass)
        callee.instantiate(args)
      else
        loc = node.respond_to?(:line) && node.line ? " at line #{node.line}" : ""
        raise Moof::RuntimeError, "Cannot call #{callee.inspect}#{loc}: not a function"
      end
    end

    def expand_and_eval_macro(call_node, env)
      macro_def = @macro_registry[call_node.callee.name]
      macro_env = env.child
      macro_def.params.each_with_index do |param, i|
        macro_env.define(param, call_node.arguments[i] || nil)
      end
      expanded = evaluate_node(macro_def.body, macro_env)
      ast = data_to_ast(expanded)
      evaluate_node(ast, env)
    end

    def data_to_ast(data)
      case data
      when Integer then AST::IntegerLiteral.new(value: data)
      when Float then AST::FloatLiteral.new(value: data)
      when String then AST::StringLiteral.new(value: data)
      when true, false then AST::BoolLiteral.new(value: data)
      when nil then AST::NilLiteral.new
      when SymbolValue then AST::Identifier.new(name: data.name)
      when Array
        return AST::NilLiteral.new if data.empty?
        elements = data.map { |el| data_to_ast(el) }
        # Reconstruct special forms from their list representation
        if elements.first.is_a?(AST::Identifier)
          case elements.first.name
          when "if"
            return AST::If.new(
              condition: elements[1],
              then_branch: elements[2],
              else_branch: elements[3]
            )
          when "do"
            return AST::Do.new(expressions: elements[1..])
          when "define"
            return AST::Define.new(name: elements[1].name, value: elements[2]) if elements[1].is_a?(AST::Identifier)
          when "set!"
            return AST::SetBang.new(name: elements[1].name, value: elements[2]) if elements[1].is_a?(AST::Identifier)
          when "and"
            return AST::And.new(left: elements[1], right: elements[2])
          when "or"
            return AST::Or.new(left: elements[1], right: elements[2])
          when "not"
            return AST::Call.new(callee: elements.first, arguments: elements[1..])
          when "quote"
            return AST::Quote.new(expression: elements[1])
          end
        end
        AST::Call.new(callee: elements.first, arguments: elements[1..])
      else
        # If it's already an AST node, pass through unchanged
        if data.is_a?(Data) && data.class.name&.start_with?("Moof::AST")
          data
        else
          AST::NilLiteral.new
        end
      end
    end

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

    def quasiquote_eval(node, env)
      case node
      when AST::Unquote
        evaluate_node(node.expression, env)
      when AST::UnquoteSplice
        raise Moof::RuntimeError, "Unquote-splice ,@ not valid outside of a list context"
      when AST::Call
        qq_list([node.callee] + node.arguments, env)
      when AST::Identifier
        SymbolValue.new(node.name)
      when AST::IntegerLiteral, AST::FloatLiteral then node.value
      when AST::StringLiteral then node.value
      when AST::BoolLiteral then node.value
      when AST::NilLiteral then nil
      # Special forms get decomposed back into list form for quasiquote
      when AST::If
        elements = [AST::Identifier.new(name: "if"), node.condition, node.then_branch]
        elements << node.else_branch if node.else_branch
        qq_list(elements, env)
      when AST::Do
        qq_list([AST::Identifier.new(name: "do")] + node.expressions, env)
      when AST::Define
        qq_list([AST::Identifier.new(name: "define"), AST::Identifier.new(name: node.name), node.value], env)
      when AST::Let
        bindings_list = node.bindings.map { |n, v| [AST::Identifier.new(name: n), v] }
        # Simplified: just produce the structure
        qq_list([AST::Identifier.new(name: "let")] + [node] , env)
      when AST::SetBang
        qq_list([AST::Identifier.new(name: "set!"), AST::Identifier.new(name: node.name), node.value], env)
      when AST::And
        qq_list([AST::Identifier.new(name: "and"), node.left, node.right], env)
      when AST::Or
        qq_list([AST::Identifier.new(name: "or"), node.left, node.right], env)
      when AST::Quasiquote
        # Nested quasiquote — don't evaluate inner unquotes
        quote_value(node)
      else
        quote_value(node)
      end
    end

    # Process a list of AST elements for quasiquote, handling splicing
    def qq_list(elements, env)
      result = []
      elements.each do |el|
        if el.is_a?(AST::UnquoteSplice)
          spliced = evaluate_node(el.expression, env)
          raise Moof::RuntimeError, ",@ value must be a list" unless spliced.is_a?(Array)
          result.concat(spliced)
        else
          result << quasiquote_eval(el, env)
        end
      end
      result
    end
  end
end
