module Moof
  class MetaCommands
    COMMANDS = {
      "help"    => "Show this help message",
      "env"     => "Print all user-defined bindings",
      "ast"     => "Parse and display AST for an expression",
      "load"    => "Load and evaluate a .moof file",
      "type"    => "Show the type and value summary of an expression",
      "doc"     => "Show documentation for a function or type",
      "methods" => "List methods on a type or value",
      "time"    => "Evaluate an expression and show elapsed time",
      "classes" => "List all defined classes",
      "traits"  => "List all defined traits",
      "clear"   => "Clear the screen",
      "reset"   => "Reset the interpreter to a fresh state",
      "version" => "Print the Moof version",
      "quit"    => "Exit the REPL",
      "exit"    => "Exit the REPL",
    }.freeze

    def initialize(repl)
      @repl = repl
    end

    def handle(input)
      return false unless input.start_with?(",")

      parts = input[1..].strip.split(/\s+/, 2)
      command = parts[0]
      arg = parts[1]

      case command
      when "help"    then print_help
      when "env"     then print_env(arg)
      when "ast"     then print_ast(arg)
      when "load"    then load_file(arg)
      when "type"    then show_type(arg)
      when "doc"     then show_doc(arg)
      when "methods" then show_methods(arg)
      when "time"    then time_expr(arg)
      when "classes" then list_classes
      when "traits"  then list_traits
      when "clear"   then clear_screen
      when "reset"   then reset_interpreter
      when "quit", "exit"
        puts Printer::Colors.dim("Goodbye!")
        @repl.send(:save_history) rescue nil
        exit 0
      when "version"
        puts "Moof #{Moof::VERSION}"
      else
        puts Printer::Colors.red("Unknown command: ,#{command}")
        puts Printer::Colors.dim("Type ,help for available commands.")
      end

      true
    end

    private

    def interp; @repl.interpreter; end
    def env; interp.global_env; end

    def print_help
      puts Printer::Colors.bold("REPL Commands")
      puts
      COMMANDS.each do |name, desc|
        next if name == "exit" # don't double-list quit/exit
        puts "  #{Printer::Colors.cyan(",#{name.ljust(10)}")} #{desc}"
      end
      puts
      puts Printer::Colors.bold("Keyboard")
      puts "  #{Printer::Colors.cyan("Ctrl-D".ljust(12))} Exit the REPL"
      puts "  #{Printer::Colors.cyan("Tab".ljust(12))} Auto-complete"
      puts "  #{Printer::Colors.cyan("Up/Down".ljust(12))} History navigation"
      puts
      puts Printer::Colors.bold("Quick Syntax")
      puts Printer.highlight('  (+ 1 2)                  ; function call')
      puts Printer.highlight('  [obj method: arg]        ; message send')
      puts Printer.highlight('  {key: "value"}           ; map literal')
      puts Printer.highlight('  (define (f x) (* x x))  ; function')
      puts Printer.highlight('  (class Foo (fields x))   ; class')
      puts "  #{Printer::Colors.dim("_")}                          #{Printer::Colors.dim("; last result")}"
    end

    def print_env(filter)
      bindings = env.bindings
      # Filter out builtins (Procs) and internal names unless asked
      user_bindings = bindings.reject { |name, val| val.is_a?(Proc) || name.start_with?("__") }

      if filter && !filter.empty?
        pattern = Regexp.new(Regexp.escape(filter), Regexp::IGNORECASE)
        user_bindings = user_bindings.select { |name, _| name.match?(pattern) }
      end

      if user_bindings.empty?
        puts Printer::Colors.dim("(no user-defined bindings#{" matching '#{filter}'" if filter})")
        return
      end

      # Group by type
      groups = user_bindings.group_by { |_, v| category(v) }
      order = ["Class", "Function", "Value"]
      order.each do |cat|
        items = groups[cat]
        next unless items
        puts Printer::Colors.bold(cat == "Value" ? "Variables" : cat.end_with?("s") ? cat : "#{cat}s")
        items.sort_by(&:first).each do |name, value|
          formatted = Printer.format(value)
          type = Printer::Colors.dim(" : #{Printer.type_of(value)}")
          puts "  #{Printer::Colors.cyan(name.ljust(20))} #{formatted}#{type}"
        end
        puts
      end
    end

    def category(value)
      case value
      when Moof::MoofClass then "Class"
      when Moof::Function  then "Function"
      else "Value"
      end
    end

    def print_ast(arg)
      if arg.nil? || arg.strip.empty?
        puts "Usage: #{Printer::Colors.cyan(",ast <expression>")}"
        return
      end

      tokens = Lexer.new(arg, filename: "(ast)").tokenize
      program = Parser.new(tokens).parse_program
      normalized = Normalizer.new.call(program)

      puts Printer::Colors.bold("Parsed:")
      pretty_print_ast(program.expressions.first, indent: 1)
      puts
      puts Printer::Colors.bold("Normalized:")
      pretty_print_ast(normalized.expressions.first, indent: 1)
    rescue Moof::MoofError => e
      puts Printer::Colors.red("Error: #{e.message}")
    end

    def pretty_print_ast(node, indent: 0)
      pad = "  " * indent
      case node
      when nil
        puts "#{pad}#{Printer::Colors.dim("nil")}"
      when AST::Program
        node.expressions.each { |e| pretty_print_ast(e, indent: indent) }
      when AST::Call
        puts "#{pad}#{Printer::Colors.yellow("Call")}"
        pretty_print_ast(node.callee, indent: indent + 1)
        node.arguments.each { |a| pretty_print_ast(a, indent: indent + 1) }
      when AST::MessageSend
        puts "#{pad}#{Printer::Colors.magenta("Send")} #{Printer::Colors.bold(node.selector)}"
        puts "#{pad}  receiver:"
        pretty_print_ast(node.receiver, indent: indent + 2)
        node.arguments.each_with_index do |a, i|
          puts "#{pad}  arg#{i}:"
          pretty_print_ast(a, indent: indent + 2)
        end
      when AST::Identifier
        puts "#{pad}#{Printer::Colors.cyan(node.name)}"
      when AST::IntegerLiteral
        puts "#{pad}#{Printer::Colors.number(node.value.to_s)}"
      when AST::FloatLiteral
        puts "#{pad}#{Printer::Colors.number(node.value.to_s)}"
      when AST::StringLiteral
        puts "#{pad}#{Printer::Colors.string(node.value.inspect)}"
      when AST::BoolLiteral
        puts "#{pad}#{Printer::Colors.keyword(node.value.to_s)}"
      when AST::NilLiteral
        puts "#{pad}#{Printer::Colors.keyword("nil")}"
      when AST::Define
        puts "#{pad}#{Printer::Colors.keyword("define")} #{Printer::Colors.cyan(node.name)}"
        pretty_print_ast(node.value, indent: indent + 1)
      when AST::DefineFunction
        params = node.params.join(" ")
        params += " . #{node.rest_param}" if node.rest_param
        puts "#{pad}#{Printer::Colors.keyword("defn")} #{Printer::Colors.cyan(node.name)}(#{params})"
        pretty_print_ast(node.body, indent: indent + 1)
      when AST::Lambda
        params = node.params.join(" ")
        params += " . #{node.rest_param}" if node.rest_param
        puts "#{pad}#{Printer::Colors.keyword("lambda")}(#{params})"
        pretty_print_ast(node.body, indent: indent + 1)
      when AST::If
        puts "#{pad}#{Printer::Colors.keyword("if")}"
        pretty_print_ast(node.condition, indent: indent + 1)
        puts "#{pad}  then:"
        pretty_print_ast(node.then_branch, indent: indent + 2)
        if node.else_branch
          puts "#{pad}  else:"
          pretty_print_ast(node.else_branch, indent: indent + 2)
        end
      when AST::Do
        puts "#{pad}#{Printer::Colors.keyword("do")}"
        node.expressions.each { |e| pretty_print_ast(e, indent: indent + 1) }
      when AST::Let
        puts "#{pad}#{Printer::Colors.keyword("let")}"
        node.bindings.each do |name, val|
          puts "#{pad}  #{Printer::Colors.cyan(name)} ="
          pretty_print_ast(val, indent: indent + 2)
        end
        puts "#{pad}  body:"
        pretty_print_ast(node.body, indent: indent + 2)
      when AST::MapLiteral
        puts "#{pad}#{Printer::Colors.yellow("Map")}"
        node.pairs.each do |key, val|
          puts "#{pad}  #{Printer::Colors.cyan("#{key}:")} "
          pretty_print_ast(val, indent: indent + 2)
        end
      when AST::Quote
        puts "#{pad}#{Printer::Colors.keyword("quote")}"
        pretty_print_ast(node.expression, indent: indent + 1)
      else
        puts "#{pad}#{Printer::Colors.dim(node.class.name.split("::").last)}"
      end
    end

    def load_file(arg)
      if arg.nil? || arg.strip.empty?
        puts "Usage: #{Printer::Colors.cyan(",load <filename>")}"
        return
      end

      filename = arg.strip
      # Try with .moof extension
      filename += ".moof" unless File.exist?(filename) || filename.end_with?(".moof")

      unless File.exist?(filename)
        puts Printer::Colors.red("File not found: #{filename}")
        return
      end

      source = File.read(filename)
      tokens = Lexer.new(source, filename: filename).tokenize
      program = Parser.new(tokens).parse_program
      normalized = Normalizer.new.call(program)
      t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      result = interp.evaluate(normalized)
      elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0

      puts Printer::Colors.green("Loaded #{filename}") +
           Printer::Colors.dim(" (#{format_time(elapsed)})")
    rescue Moof::MoofError => e
      puts Printer.format_error(e, source: source)
    rescue => e
      puts Printer::Colors.red("Error: #{e.class}: #{e.message}")
    end

    def show_type(arg)
      if arg.nil? || arg.strip.empty?
        puts "Usage: #{Printer::Colors.cyan(",type <expression>")}"
        return
      end

      tokens = Lexer.new(arg, filename: "(type)").tokenize
      program = Parser.new(tokens).parse_program
      normalized = Normalizer.new.call(program)
      result = interp.evaluate(normalized)

      type_str = Printer::Colors.type_hl(Printer.type_of(result))
      val_str  = Printer.format(result)
      puts "#{type_str} #{val_str}"
    rescue Moof::MoofError => e
      puts Printer::Colors.red("Error: #{e.message}")
    end

    def show_doc(arg)
      if arg.nil? || arg.strip.empty?
        puts "Usage: #{Printer::Colors.cyan(",doc <name>")}"
        return
      end

      name = arg.strip

      # Check if it's a class
      klass = interp.class_registry[name]
      if klass
        puts Printer::Colors.class_hl("class #{klass.name}")
        if klass.superclass
          puts "  extends #{Printer::Colors.class_hl(klass.superclass.name)}"
        end
        unless klass.own_fields.empty?
          puts "  fields: #{klass.own_fields.map { |f| Printer::Colors.cyan(f) }.join(", ")}"
        end
        all_fields = klass.fields
        inherited = all_fields - klass.own_fields
        unless inherited.empty?
          puts "  inherited fields: #{inherited.map { |f| Printer::Colors.dim(f) }.join(", ")}"
        end
        unless klass.method_table.empty?
          puts "  methods:"
          klass.method_table.each do |sel, func|
            params = func.params.join(" ")
            params += " . #{func.rest_param}" if func.respond_to?(:rest_param) && func.rest_param
            puts "    #{Printer::Colors.cyan(sel)}#{params.empty? ? "" : " [#{params}]"}"
          end
        end
        # Show inherited methods
        if klass.superclass
          inherited_methods = collect_inherited_methods(klass.superclass, klass.method_table.keys)
          unless inherited_methods.empty?
            puts "  inherited methods:"
            inherited_methods.each do |sel, from_class|
              puts "    #{Printer::Colors.dim(sel)} #{Printer::Colors.dim("(from #{from_class})")}"
            end
          end
        end
        return
      end

      # Check if it's a trait
      trait = interp.trait_registry[name]
      if trait
        puts Printer::Colors.magenta("trait #{name}")
        trait.each do |sel, func|
          params = func.params.join(" ")
          puts "  #{Printer::Colors.cyan(sel)}#{params.empty? ? "" : " [#{params}]"}"
        end
        return
      end

      # Check if it's a binding
      begin
        value = env.get(name)
        if value.is_a?(Moof::Function)
          params = value.params.join(" ")
          params += " . #{value.rest_param}" if value.rest_param
          fn_name = value.name || name
          puts Printer::Colors.fn_hl("(define (#{fn_name} #{params}))")
          puts "  arity: #{value.variadic? ? "#{value.arity}+" : value.arity}"
        elsif value.is_a?(Proc)
          puts Printer::Colors.fn_hl("#<builtin #{name}>")
          puts Printer::Colors.dim("  (built-in function)")
        else
          puts "#{Printer::Colors.cyan(name)} : #{Printer::Colors.type_hl(Printer.type_of(value))}"
          puts "  = #{Printer.format(value)}"
        end
      rescue Moof::NameError
        puts Printer::Colors.dim("Unknown: '#{name}' is not a class, trait, or binding")
      end
    end

    def collect_inherited_methods(klass, exclude, result = [])
      return result if klass.nil?
      klass.method_table.each do |sel, _|
        next if exclude.include?(sel)
        result << [sel, klass.name]
      end
      collect_inherited_methods(klass.superclass, exclude + klass.method_table.keys, result)
      result
    end

    def show_methods(arg)
      if arg.nil? || arg.strip.empty?
        puts "Usage: #{Printer::Colors.cyan(",methods <type-or-expression>")}"
        puts Printer::Colors.dim("  Examples: ,methods String  ,methods (list 1 2 3)  ,methods Point")
        return
      end

      name = arg.strip

      # Check if it's a class name
      klass = interp.class_registry[name]
      if klass
        show_class_methods(klass)
        return
      end

      # Try evaluating as expression
      tokens = Lexer.new(name, filename: "(methods)").tokenize
      program = Parser.new(tokens).parse_program
      normalized = Normalizer.new.call(program)
      result = interp.evaluate(normalized)

      type = Printer.type_of(result)
      puts Printer::Colors.bold("Methods on #{Printer::Colors.type_hl(type)}:")
      puts

      # If it's a MoofObject, show its class methods
      if result.is_a?(Moof::MoofObject)
        show_class_methods(result.klass)
        return
      end

      # For built-in types, show hardcoded + user-defined methods
      class_name = case result
                   when Integer then "Integer"
                   when Float then "Float"
                   when String then "String"
                   when Array then "List"
                   when Hash then "Map"
                   when true, false then "Bool"
                   when nil then "Nil"
                   when Moof::Function then "Function"
                   end

      if class_name
        builtin_msgs = builtin_messages(class_name)
        user_klass = interp.class_registry[class_name]
        user_methods = user_klass ? user_klass.method_table.keys : []

        unless user_methods.empty?
          puts Printer::Colors.bold("  User-defined:")
          user_methods.sort.each { |m| puts "    #{Printer::Colors.cyan(m)}" }
          puts
        end
        puts Printer::Colors.bold("  Built-in:")
        builtin_msgs.sort.each { |m| puts "    #{Printer::Colors.dim(m)}" }
      end

    rescue Moof::MoofError => e
      puts Printer::Colors.red("Error: #{e.message}")
    end

    def show_class_methods(klass)
      puts Printer::Colors.bold("Methods on #{Printer::Colors.class_hl(klass.name)}:")
      puts
      k = klass
      while k
        unless k.method_table.empty?
          label = k == klass ? "Own:" : "Inherited from #{k.name}:"
          puts Printer::Colors.bold("  #{label}")
          k.method_table.each do |sel, func|
            params = func.params.join(" ")
            puts "    #{Printer::Colors.cyan(sel)}#{params.empty? ? "" : "(#{params})"}"
          end
          puts
        end
        k = k.superclass
      end
    end

    def builtin_messages(class_name)
      case class_name
      when "Integer", "Float"
        %w[abs to_s to_f to_i zero? positive? negative? nil? class pow: max: min: + - * / % > < >= <=]
      when "String"
        %w[length uppercase lowercase reverse to_s to_i to_f chars trim nil? class at: contains: startsWith: endsWith: replaceAll:with: split: concat: slice:length:]
      when "List"
        %w[length first last rest reverse sort uniq flatten empty? to_s nil? class at: push: prepend: contains: join: indexOf: take: drop: zip: map: filter: reduce:init: each: any: all: none: sortBy:]
      when "Map"
        %w[keys values length empty? to_s nil? class at: put:value: remove: contains: merge:]
      when "Function"
        %w[call: arity nil? class]
      when "Bool"
        %w[not to_s nil? class and: or:]
      when "Nil"
        %w[nil? to_s class]
      else
        []
      end
    end

    def time_expr(arg)
      if arg.nil? || arg.strip.empty?
        puts "Usage: #{Printer::Colors.cyan(",time <expression>")}"
        return
      end

      tokens = Lexer.new(arg, filename: "(time)").tokenize
      program = Parser.new(tokens).parse_program
      normalized = Normalizer.new.call(program)

      t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      result = interp.evaluate(normalized)
      elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0

      interp.global_env.define("_", result)

      formatted = Printer.format(result)
      type_hint = Printer::Colors.dim(" : #{Printer.type_of(result)}")
      time_str  = Printer::Colors.dim(" [#{format_time(elapsed)}]")
      puts Printer::Colors.green("=> ") + formatted + type_hint + time_str
    rescue Moof::MoofError => e
      puts Printer::Colors.red("Error: #{e.message}")
    end

    def list_classes
      registry = interp.class_registry
      if registry.empty?
        puts Printer::Colors.dim("(no classes defined)")
        return
      end

      builtin = %w[Integer Float String List Map Bool Nil Function]
      user_classes = registry.reject { |name, _| builtin.include?(name) }
      builtin_classes = registry.select { |name, _| builtin.include?(name) }

      unless user_classes.empty?
        puts Printer::Colors.bold("User-defined classes:")
        user_classes.each do |name, klass|
          extras = []
          extras << "extends #{klass.superclass.name}" if klass.superclass
          extras << "#{klass.own_fields.length} fields" unless klass.own_fields.empty?
          extras << "#{klass.method_table.length} methods" unless klass.method_table.empty?
          desc = extras.empty? ? "" : Printer::Colors.dim(" (#{extras.join(", ")})")
          puts "  #{Printer::Colors.class_hl(name)}#{desc}"
        end
        puts
      end

      # Show built-in classes that have been extended
      extended = builtin_classes.select { |_, k| !k.method_table.empty? }
      unless extended.empty?
        puts Printer::Colors.bold("Extended built-in classes:")
        extended.each do |name, klass|
          methods = klass.method_table.keys.sort.join(", ")
          puts "  #{Printer::Colors.class_hl(name)} #{Printer::Colors.dim("+ #{methods}")}"
        end
        puts
      end

      unextended = builtin_classes.select { |_, k| k.method_table.empty? }
      unless unextended.empty?
        names = unextended.keys.sort.join(", ")
        puts Printer::Colors.dim("Built-in (not extended): #{names}")
      end
    end

    def list_traits
      registry = interp.trait_registry
      if registry.empty?
        puts Printer::Colors.dim("(no traits defined)")
        return
      end

      registry.each do |name, methods|
        selectors = methods.keys.sort.join(", ")
        puts "  #{Printer::Colors.magenta(name)} #{Printer::Colors.dim("(#{selectors})")}"
      end
    end

    def clear_screen
      print "\e[2J\e[H"
    end

    def reset_interpreter
      @repl.instance_variable_set(:@interpreter, Moof::Interpreter.new)
      Moof.load_stdlib(@repl.interpreter)
      puts Printer::Colors.green("Interpreter reset.")
    end

    def format_time(seconds)
      if seconds < 0.001
        "#{(seconds * 1_000_000).round}us"
      elsif seconds < 1
        "#{(seconds * 1_000).round(1)}ms"
      else
        "#{seconds.round(3)}s"
      end
    end
  end
end
