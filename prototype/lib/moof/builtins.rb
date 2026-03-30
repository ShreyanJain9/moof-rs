module Moof
  # Ruby builtins — ONLY things that require host language access.
  # Everything else lives in stdlib.moof (self-hosting Moof).
  module Builtins
    def self.install(env, interpreter)

      # ═══════════════════════════════════════════════════════════════
      # Arithmetic — needs Ruby numeric operators
      # ═══════════════════════════════════════════════════════════════

      env.define("+", ->(interp, args) { args.reduce(0, :+) })
      env.define("*", ->(interp, args) { args.reduce(1, :*) })

      env.define("-", ->(interp, args) {
        case args.length
        when 0 then raise Moof::ArityError.new("1+", 0, name: "-")
        when 1 then -args[0]
        else args.reduce(:-)
        end
      })

      env.define("/", ->(interp, args) {
        raise Moof::ArityError.new("2+", args.length, name: "/") if args.length < 2
        args.reduce(:/)
      })

      env.define("%", ->(interp, args) {
        check_arity!("%", 2, args); args[0] % args[1]
      })

      # ═══════════════════════════════════════════════════════════════
      # Comparison — needs Ruby's <=>
      # ═══════════════════════════════════════════════════════════════

      env.define(">",  ->(interp, args) { check_arity!(">", 2, args);  args[0] > args[1] })
      env.define("<",  ->(interp, args) { check_arity!("<", 2, args);  args[0] < args[1] })
      env.define(">=", ->(interp, args) { check_arity!(">=", 2, args); args[0] >= args[1] })
      env.define("<=", ->(interp, args) { check_arity!("<=", 2, args); args[0] <= args[1] })

      # ═══════════════════════════════════════════════════════════════
      # Equality — needs Ruby's == and .equal?
      # ═══════════════════════════════════════════════════════════════

      env.define("=",   ->(interp, args) { check_arity!("=", 2, args);   args[0] == args[1] })
      env.define("eq?", ->(interp, args) { check_arity!("eq?", 2, args); args[0].equal?(args[1]) })

      # ═══════════════════════════════════════════════════════════════
      # Core data constructors — needs Ruby Array
      # ═══════════════════════════════════════════════════════════════

      env.define("list", ->(interp, args) { args.dup })
      env.define("cons", ->(interp, args) { check_arity!("cons", 2, args); [args[0]] + args[1] })
      env.define("car",  ->(interp, args) { check_arity!("car", 1, args);  args[0].first })
      env.define("cdr",  ->(interp, args) { check_arity!("cdr", 1, args);  args[0][1..] })

      # ═══════════════════════════════════════════════════════════════
      # I/O — needs Ruby's puts, print, $stdin, File
      # ═══════════════════════════════════════════════════════════════

      env.define("print", ->(interp, args) {
        puts args.map { |a| format_value(a) }.join(" "); nil
      })

      env.define("display", ->(interp, args) {
        print args.map { |a| format_value(a) }.join(" "); nil
      })

      env.define("format", ->(interp, args) {
        raise Moof::ArityError.new("1+", 0, name: "format") if args.empty?
        template, values = args[0], args[1..]
        idx = 0
        template.gsub("~a") { val = idx < values.length ? format_value(values[idx]) : ""; idx += 1; val }
      })

      env.define("read-line", ->(interp, args) {
        if args.length == 1 then print args[0]; $stdout.flush end
        $stdin.gets&.chomp
      })

      env.define("read-file",    ->(interp, args) { check_arity!("read-file", 1, args);    File.read(args[0]) })
      env.define("write-file",   ->(interp, args) { check_arity!("write-file", 2, args);   File.write(args[0], args[1]); nil })
      env.define("file-exists?", ->(interp, args) { check_arity!("file-exists?", 1, args); File.exist?(args[0]) })
      env.define("read-lines",   ->(interp, args) { check_arity!("read-lines", 1, args);   File.readlines(args[0], chomp: true) })

      # ═══════════════════════════════════════════════════════════════
      # Message dispatch — the bridge between () and []
      # ═══════════════════════════════════════════════════════════════

      env.define("__send", ->(interp, args) {
        raise Moof::ArityError.new("2+", args.length, name: "__send") if args.length < 2
        interp.dispatcher.send_message(args[0], args[1], args[2..], interpreter: interp)
      })

      # ═══════════════════════════════════════════════════════════════
      # Apply — needs Ruby function invocation
      # ═══════════════════════════════════════════════════════════════

      env.define("apply", ->(interp, args) {
        check_arity!("apply", 2, args)
        func, func_args = args
        call_func(func, interp, func_args)
      })

      # ═══════════════════════════════════════════════════════════════
      # Error / control — needs Ruby exceptions and Kernel
      # ═══════════════════════════════════════════════════════════════

      env.define("error", ->(interp, args) {
        check_arity!("error", 1, args)
        raise Moof::RuntimeError, args[0].to_s
      })

      env.define("exit", ->(interp, args) {
        Kernel.exit(args.length == 1 ? args[0] : 0)
      })

      # ═══════════════════════════════════════════════════════════════
      # Type introspection — needs Ruby is_a? checks
      # ═══════════════════════════════════════════════════════════════

      env.define("type-of", ->(interp, args) {
        check_arity!("type-of", 1, args)
        case args[0]
        when Integer          then "Integer"
        when Float            then "Float"
        when String           then "String"
        when true, false      then "Bool"
        when nil              then "Nil"
        when Array            then "List"
        when Hash             then "Map"
        when Moof::Function   then "Function"
        when Moof::MoofObject then args[0].klass.name
        when Moof::MoofClass  then "Class"
        when Moof::Protocol   then "Protocol"
        else args[0].class.name
        end
      })

      # ═══════════════════════════════════════════════════════════════
      # Protocol — needs Ruby Protocol object
      # ═══════════════════════════════════════════════════════════════

      env.define("implements?", ->(interp, args) {
        check_arity!("implements?", 2, args)
        protocol = args[1]
        raise Moof::RuntimeError, "implements?: second argument must be a Protocol" unless protocol.respond_to?(:satisfied_by?)
        protocol.satisfied_by?(args[0], interp)
      })

      # ═══════════════════════════════════════════════════════════════
      # Timing — needs Ruby clock
      # ═══════════════════════════════════════════════════════════════

      env.define("time", ->(interp, args) {
        check_arity!("time", 1, args)
        t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
        result = call_func(args[0], interp, [])
        elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0
        puts "Elapsed: #{(elapsed * 1000).round(2)}ms"
        result
      })

      # ═══════════════════════════════════════════════════════════════
      # Range — needs Ruby Range
      # ═══════════════════════════════════════════════════════════════

      env.define("range", ->(interp, args) {
        case args.length
        when 1 then (0...args[0]).to_a
        when 2 then (args[0]...args[1]).to_a
        when 3 then (args[0]...args[1]).step(args[2]).to_a
        else raise Moof::ArityError.new("1-3", args.length, name: "range")
        end
      })

      env
    end

    private

    def self.check_arity!(name, expected, args)
      raise Moof::ArityError.new(expected, args.length, name: name) if args.length != expected
    end

    def self.call_func(func, interp, args)
      case func
      when Moof::Function then func.call(interp, args)
      when Proc           then func.call(interp, args)
      else raise Moof::RuntimeError, "Expected a function, got #{func.inspect}"
      end
    end

    def self.format_value(val)
      case val
      when nil              then "nil"
      when true             then "true"
      when false            then "false"
      when String           then val
      when Array            then "(#{val.map { |v| format_value(v) }.join(" ")})"
      when Hash             then "{#{val.map { |k, v| "#{k}: #{format_value(v)}" }.join(" ")}}"
      when Moof::Function   then val.to_s
      when Moof::SymbolValue then val.to_s
      else val.to_s
      end
    end
  end
end
