module Moof
  module Builtins
    def self.install(env, interpreter)
      # ---- Arithmetic (variadic) ----

      env.define("+", ->(interp, args) {
        args.reduce(0, :+)
      })

      env.define("-", ->(interp, args) {
        if args.length == 1
          -args[0]
        elsif args.length >= 2
          args[0..-1].reduce(:-)
        else
          raise Moof::ArityError.new("1+", 0, name: "-")
        end
      })

      env.define("*", ->(interp, args) {
        args.reduce(1, :*)
      })

      env.define("/", ->(interp, args) {
        if args.length < 2
          raise Moof::ArityError.new("2+", args.length, name: "/")
        end
        args.reduce(:/)
      })

      env.define("%", ->(interp, args) {
        if args.length != 2
          raise Moof::ArityError.new(2, args.length, name: "%")
        end
        args[0] % args[1]
      })

      # ---- Comparison ----

      env.define(">", ->(interp, args) {
        check_arity!(">", 2, args); args[0] > args[1]
      })

      env.define("<", ->(interp, args) {
        check_arity!("<", 2, args); args[0] < args[1]
      })

      env.define(">=", ->(interp, args) {
        check_arity!(">=", 2, args); args[0] >= args[1]
      })

      env.define("<=", ->(interp, args) {
        check_arity!("<=", 2, args); args[0] <= args[1]
      })

      # ---- Equality ----

      env.define("=", ->(interp, args) {
        check_arity!("=", 2, args); args[0] == args[1]
      })

      env.define("eq?", ->(interp, args) {
        check_arity!("eq?", 2, args); args[0].equal?(args[1])
      })

      # ---- Logic ----

      env.define("not", ->(interp, args) {
        check_arity!("not", 1, args)
        val = args[0]
        val == false || val.nil? ? true : false
      })

      env.define("and", ->(interp, args) {
        check_arity!("and", 2, args)
        args[0] && args[1]
      })

      env.define("or", ->(interp, args) {
        check_arity!("or", 2, args)
        args[0] || args[1]
      })

      # ---- List ----

      env.define("list", ->(interp, args) {
        args.dup
      })

      env.define("cons", ->(interp, args) {
        check_arity!("cons", 2, args)
        [args[0]] + args[1]
      })

      env.define("car", ->(interp, args) {
        check_arity!("car", 1, args)
        args[0].first
      })

      env.define("cdr", ->(interp, args) {
        check_arity!("cdr", 1, args)
        args[0][1..]
      })

      # ---- I/O ----

      env.define("print", ->(interp, args) {
        puts args.map { |a| format_value(a) }.join(" ")
        nil
      })

      env.define("display", ->(interp, args) {
        print args.map { |a| format_value(a) }.join(" ")
        nil
      })

      # ---- Formatting ----

      env.define("format", ->(interp, args) {
        if args.empty?
          raise Moof::ArityError.new("1+", 0, name: "format")
        end
        template = args[0]
        values = args[1..]
        idx = 0
        template.gsub("~a") {
          val = idx < values.length ? format_value(values[idx]) : ""
          idx += 1
          val
        }
      })

      # ---- Type checks ----

      env.define("number?", ->(interp, args) {
        check_arity!("number?", 1, args)
        args[0].is_a?(Integer) || args[0].is_a?(Float)
      })

      env.define("string?", ->(interp, args) {
        check_arity!("string?", 1, args)
        args[0].is_a?(String)
      })

      env.define("list?", ->(interp, args) {
        check_arity!("list?", 1, args)
        args[0].is_a?(Array)
      })

      env.define("nil?", ->(interp, args) {
        check_arity!("nil?", 1, args)
        args[0].nil?
      })

      env.define("bool?", ->(interp, args) {
        check_arity!("bool?", 1, args)
        args[0] == true || args[0] == false
      })

      env.define("function?", ->(interp, args) {
        check_arity!("function?", 1, args)
        args[0].is_a?(Moof::Function) || args[0].is_a?(Proc)
      })

      # ---- Misc ----

      env.define("apply", ->(interp, args) {
        check_arity!("apply", 2, args)
        func = args[0]
        func_args = args[1]
        if func.is_a?(Moof::Function)
          func.call(interp, func_args)
        elsif func.is_a?(Proc)
          func.call(interp, func_args)
        else
          raise Moof::RuntimeError, "apply: first argument must be a function"
        end
      })

      env.define("length", ->(interp, args) {
        check_arity!("length", 1, args)
        args[0].length
      })

      # ---- Functional (standalone versions of list messages) ----

      env.define("map", ->(interp, args) {
        check_arity!("map", 2, args)
        func, lst = args
        lst.map { |el| call_func(func, interp, [el]) }
      })

      env.define("filter", ->(interp, args) {
        check_arity!("filter", 2, args)
        func, lst = args
        lst.select { |el| truthy?(call_func(func, interp, [el])) }
      })

      env.define("reduce", ->(interp, args) {
        check_arity!("reduce", 3, args)
        func, init, lst = args
        lst.reduce(init) { |acc, el| call_func(func, interp, [acc, el]) }
      })

      env.define("for-each", ->(interp, args) {
        check_arity!("for-each", 2, args)
        func, lst = args
        lst.each { |el| call_func(func, interp, [el]) }
        nil
      })

      env.define("range", ->(interp, args) {
        case args.length
        when 1 then (0...args[0]).to_a
        when 2 then (args[0]...args[1]).to_a
        when 3 then (args[0]...args[1]).step(args[2]).to_a
        else raise Moof::ArityError.new("1-3", args.length, name: "range")
        end
      })

      env.define("take", ->(interp, args) {
        check_arity!("take", 2, args); args[1].first(args[0])
      })

      env.define("drop", ->(interp, args) {
        check_arity!("drop", 2, args); args[1].drop(args[0])
      })

      env.define("flatten", ->(interp, args) {
        check_arity!("flatten", 1, args); args[0].flatten
      })

      env.define("reverse", ->(interp, args) {
        check_arity!("reverse", 1, args); args[0].reverse
      })

      env.define("sort", ->(interp, args) {
        check_arity!("sort", 1, args); args[0].sort
      })

      env.define("concat", ->(interp, args) {
        check_arity!("concat", 2, args); args[0] + args[1]
      })

      env.define("nth", ->(interp, args) {
        check_arity!("nth", 2, args); args[1][args[0]]
      })

      env.define("empty?", ->(interp, args) {
        check_arity!("empty?", 1, args); args[0].empty?
      })

      env.define("hash?", ->(interp, args) {
        check_arity!("hash?", 1, args); args[0].is_a?(Hash)
      })

      env.define("symbol?", ->(interp, args) {
        check_arity!("symbol?", 1, args); args[0].is_a?(Moof::SymbolValue)
      })

      env.define("zip", ->(interp, args) {
        check_arity!("zip", 2, args)
        args[0].zip(args[1]).map { |pair| pair.compact }
      })

      env.define("error", ->(interp, args) {
        check_arity!("error", 1, args)
        raise Moof::RuntimeError, args[0].to_s
      })

      # ---- Message dispatch ----

      env.define("__send", ->(interp, args) {
        if args.length < 2
          raise Moof::ArityError.new("2+", args.length, name: "__send")
        end
        receiver = args[0]
        selector = args[1]
        msg_args = args[2..]
        interp.dispatcher.send_message(receiver, selector, msg_args, interpreter: interp)
      })

      # ---- File I/O ----

      env.define("read-file", ->(interp, args) {
        check_arity!("read-file", 1, args)
        File.read(args[0])
      })

      env.define("write-file", ->(interp, args) {
        check_arity!("write-file", 2, args)
        File.write(args[0], args[1])
        nil
      })

      env.define("file-exists?", ->(interp, args) {
        check_arity!("file-exists?", 1, args)
        File.exist?(args[0])
      })

      env.define("read-lines", ->(interp, args) {
        check_arity!("read-lines", 1, args)
        File.readlines(args[0], chomp: true)
      })

      # ---- Protocol support ----

      env.define("implements?", ->(interp, args) {
        check_arity!("implements?", 2, args)
        obj = args[0]
        protocol = args[1]
        if protocol.respond_to?(:satisfied_by?)
          protocol.satisfied_by?(obj, interp)
        else
          raise Moof::RuntimeError, "implements?: second argument must be a Protocol"
        end
      })

      # ---- Dynamic send ----

      env.define("send", ->(interp, args) {
        if args.length < 2
          raise Moof::ArityError.new("2+", args.length, name: "send")
        end
        receiver = args[0]
        selector = args[1]
        msg_args = args[2..] || []
        interp.dispatcher.send_message(receiver, selector, msg_args, interpreter: interp)
      })

      # ---- Type introspection ----

      env.define("type-of", ->(interp, args) {
        check_arity!("type-of", 1, args)
        val = args[0]
        case val
        when Integer          then "Integer"
        when Float            then "Float"
        when String           then "String"
        when true, false      then "Bool"
        when nil              then "Nil"
        when Array            then "List"
        when Hash             then "Map"
        when Moof::Function   then "Function"
        when Moof::MoofObject then val.klass.name
        when Moof::MoofClass  then "Class"
        when Moof::Protocol   then "Protocol"
        else val.class.name
        end
      })

      # ---- Conversion ----

      env.define("to-string", ->(interp, args) {
        check_arity!("to-string", 1, args)
        format_value(args[0])
      })

      # ---- User input ----

      env.define("read-line", ->(interp, args) {
        if args.length > 1
          raise Moof::ArityError.new("0-1", args.length, name: "read-line")
        end
        if args.length == 1
          print args[0]
          $stdout.flush
        end
        $stdin.gets&.chomp
      })

      # ---- Assertions ----

      env.define("assert", ->(interp, args) {
        check_arity!("assert", 1, args)
        unless args[0] != false && !args[0].nil?
          raise Moof::RuntimeError, "Assertion failed"
        end
        true
      })

      env.define("assert-equal", ->(interp, args) {
        check_arity!("assert-equal", 2, args)
        unless args[0] == args[1]
          raise Moof::RuntimeError, "Assertion failed: expected #{format_value(args[0])}, got #{format_value(args[1])}"
        end
        true
      })

      # ---- Process control ----

      env.define("exit", ->(interp, args) {
        if args.length > 1
          raise Moof::ArityError.new("0-1", args.length, name: "exit")
        end
        code = args.length == 1 ? args[0] : 0
        Kernel.exit(code)
      })

      # ---- Timing ----

      env.define("time", ->(interp, args) {
        check_arity!("time", 1, args)
        thunk = args[0]
        t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
        result = if thunk.is_a?(Moof::Function)
          thunk.call(interp, [])
        elsif thunk.is_a?(Proc)
          thunk.call(interp, [])
        else
          raise Moof::RuntimeError, "time: argument must be a function/lambda"
        end
        elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0
        puts "Elapsed: #{(elapsed * 1000).round(2)}ms"
        result
      })

      # ---- String operations ----

      env.define("string-concat", ->(interp, args) {
        args.map { |a| a.to_s }.join
      })

      env
    end

    private

    def self.check_arity!(name, expected, args)
      if args.length != expected
        raise Moof::ArityError.new(expected, args.length, name: name)
      end
    end

    def self.call_func(func, interp, args)
      if func.is_a?(Moof::Function)
        func.call(interp, args)
      elsif func.is_a?(Proc)
        func.call(interp, args)
      else
        raise Moof::RuntimeError, "Expected a function, got #{func.inspect}"
      end
    end

    def self.truthy?(val)
      val != false && !val.nil?
    end

    def self.format_value(val)
      case val
      when nil    then "nil"
      when true   then "true"
      when false  then "false"
      when String then val
      when Array  then "(#{val.map { |v| format_value(v) }.join(" ")})"
      when Hash
        pairs = val.map { |k, v| "#{k}: #{format_value(v)}" }.join(" ")
        "{#{pairs}}"
      when Moof::Function then val.to_s
      when Moof::SymbolValue then val.to_s
      else val.to_s
      end
    end
  end
end
