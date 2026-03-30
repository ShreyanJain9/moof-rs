require_relative "object_system"

module Moof
  class Dispatcher
    def initialize(interpreter = nil)
      @interpreter = interpreter
    end

    def send_message(receiver, selector, args, interpreter: nil)
      interp = interpreter || @interpreter

      # For non-MoofObject receivers, check if the built-in class has been
      # extended with a Moof-defined method for this selector FIRST.
      unless receiver.is_a?(Moof::MoofObject) || receiver.is_a?(Moof::MoofClass)
        class_name = builtin_class_name(receiver)
        if class_name && interp
          klass = interp.class_registry[class_name]
          if klass
            method = klass.lookup(selector)
            if method
              return invoke_method(method, receiver, args, interp)
            end
          end
        end
      end

      # Fall back to hardcoded dispatch
      case receiver
      when Integer, Float        then dispatch_number(receiver, selector, args)
      when String                then dispatch_string(receiver, selector, args)
      when Array                 then dispatch_array(receiver, selector, args, interp)
      when Hash                  then dispatch_hash(receiver, selector, args)
      when Moof::Function        then dispatch_function(receiver, selector, args, interp)
      when Moof::MoofObject      then dispatch_moof_object(receiver, selector, args, interp)
      when Moof::MoofClass       then dispatch_moof_class(receiver, selector, args)
      when true, false           then dispatch_boolean(receiver, selector, args)
      when nil                   then dispatch_nil(receiver, selector, args)
      else                            dispatch_universal(receiver, selector, args)
      end
    end

    private

    # Map Ruby types to Moof built-in class names
    def builtin_class_name(receiver)
      case receiver
      when Integer       then "Integer"
      when Float         then "Float"
      when String        then "String"
      when Array         then "List"
      when Hash          then "Map"
      when true, false   then "Bool"
      when nil           then "Nil"
      when Moof::Function then "Function"
      end
    end

    # ── Levenshtein distance (simple DP) ──────────────────────────

    def levenshtein(a, b)
      n = a.length
      m = b.length
      return m if n == 0
      return n if m == 0

      d = Array.new(n + 1) { |i| i }
      (1..m).each do |j|
        prev = d[0]
        d[0] = j
        (1..n).each do |i|
          cost = a[i - 1] == b[j - 1] ? 0 : 1
          temp = d[i]
          d[i] = [d[i] + 1, d[i - 1] + 1, prev + cost].min
          prev = temp
        end
      end
      d[n]
    end

    # ── Collect known selectors for a receiver ────────────────────

    def collect_known_selectors(receiver)
      selectors = []

      case receiver
      when Integer, Float
        selectors.concat(%w[abs to_s to_f to_i zero? positive? negative? nil? class sqrt pow: max: min: + - * / % > < >= <=])
      when String
        selectors.concat(%w[length uppercase lowercase reverse to_s to_i to_f chars trim nil? class at: contains: startsWith: endsWith: replaceAll:with: split: concat: slice:length:])
      when Array
        selectors.concat(%w[length first last rest reverse sort uniq flatten empty? to_s nil? class at: push: prepend: contains: join: indexOf: take: drop: zip: map: filter: reduce:init: each: any: all: none: sortBy:])
      when Hash
        selectors.concat(%w[keys values length empty? to_s nil? class at: put:value: remove: contains: merge:])
      when Moof::Function
        selectors.concat(%w[call: arity nil? class])
      when Moof::MoofObject
        klass = receiver.klass
        k = klass
        while k
          selectors.concat(k.method_table.keys)
          k = k.superclass
        end
        selectors.concat(receiver.fields.keys)
        selectors.concat(%w[class className methods respondsTo: nil? to_s set:to:])
      when Moof::MoofClass
        selectors.concat(%w[name fields methods nil? class])
      when true, false
        selectors.concat(%w[not to_s nil? class and: or:])
      when nil
        selectors.concat(%w[nil? to_s class])
      end

      # Also check user-defined class extensions
      class_name = builtin_class_name(receiver)
      if class_name && @interpreter
        klass = @interpreter.class_registry[class_name]
        selectors.concat(klass.method_table.keys) if klass
      end

      selectors.uniq
    end

    # ── Suggest closest selector ──────────────────────────────────

    def suggest_selector(receiver, failed_selector)
      known = collect_known_selectors(receiver)
      return nil if known.empty?

      best = nil
      best_dist = Float::INFINITY

      known.each do |sel|
        dist = levenshtein(failed_selector, sel)
        if dist < best_dist
          best_dist = dist
          best = sel
        end
      end

      # Only suggest if reasonably close (distance <= 3 and < half the selector length)
      max_dist = [3, (failed_selector.length / 2.0).ceil].min
      max_dist = 1 if failed_selector.length <= 2
      best_dist <= max_dist ? best : nil
    end

    # ── Raise with suggestion ─────────────────────────────────────

    def raise_message_error(receiver, receiver_desc, selector)
      suggestion = suggest_selector(receiver, selector)
      if suggestion
        raise Moof::RuntimeError, "#{receiver_desc} does not respond to '#{selector}'\n  Did you mean: #{suggestion}?"
      else
        raise Moof::MessageError.new(receiver_desc, selector)
      end
    end

    def dispatch_number(receiver, selector, args)
      case selector
      when "abs"       then receiver.abs
      when "to_s"      then receiver.to_s
      when "to_f"      then receiver.to_f
      when "to_i"      then receiver.to_i
      when "zero?"     then receiver.zero?
      when "positive?" then receiver > 0
      when "negative?" then receiver < 0
      when "nil?"      then false
      when "class"     then receiver.is_a?(Integer) ? "Integer" : "Float"
      when "+"   then check_args!(receiver, selector, args, 1); receiver + args[0]
      when "-"   then check_args!(receiver, selector, args, 1); receiver - args[0]
      when "*"   then check_args!(receiver, selector, args, 1); receiver * args[0]
      when "/"   then check_args!(receiver, selector, args, 1); receiver / args[0]
      when "%"   then check_args!(receiver, selector, args, 1); receiver % args[0]
      when ">"   then check_args!(receiver, selector, args, 1); receiver > args[0]
      when "<"   then check_args!(receiver, selector, args, 1); receiver < args[0]
      when ">="  then check_args!(receiver, selector, args, 1); receiver >= args[0]
      when "<="  then check_args!(receiver, selector, args, 1); receiver <= args[0]
      when "sqrt"
        result = Math.sqrt(receiver.to_f)
        receiver.is_a?(Integer) && result == result.floor ? result.to_i : result
      when "pow:" then check_args!(receiver, selector, args, 1); receiver ** args[0]
      when "max:" then check_args!(receiver, selector, args, 1); receiver >= args[0] ? receiver : args[0]
      when "min:" then check_args!(receiver, selector, args, 1); receiver <= args[0] ? receiver : args[0]
      else
        desc = receiver.is_a?(Integer) ? "Integer" : "Float"
        raise_message_error(receiver, desc, selector)
      end
    end

    def dispatch_string(receiver, selector, args)
      case selector
      when "length"      then receiver.length
      when "uppercase"   then receiver.upcase
      when "lowercase"   then receiver.downcase
      when "reverse"     then receiver.reverse
      when "to_s"        then receiver
      when "to_i"        then receiver.to_i
      when "to_f"        then receiver.to_f
      when "nil?"        then false
      when "class"       then "String"
      when "chars"       then receiver.chars
      when "trim"        then receiver.strip
      when "at:"
        check_args!(receiver, selector, args, 1); receiver[args[0]]
      when "contains:"
        check_args!(receiver, selector, args, 1); receiver.include?(args[0])
      when "startsWith:"
        check_args!(receiver, selector, args, 1); receiver.start_with?(args[0])
      when "endsWith:"
        check_args!(receiver, selector, args, 1); receiver.end_with?(args[0])
      when "replaceAll:with:"
        check_args!(receiver, selector, args, 2); receiver.gsub(args[0], args[1])
      when "split:"
        check_args!(receiver, selector, args, 1); receiver.split(args[0])
      when "concat:"
        check_args!(receiver, selector, args, 1); receiver + args[0].to_s
      when "slice:length:"
        check_args!(receiver, selector, args, 2); receiver[args[0], args[1]]
      else raise_message_error(receiver, "String", selector)
      end
    end

    def dispatch_array(receiver, selector, args, interpreter)
      case selector
      when "length"  then receiver.length
      when "first"   then receiver.first
      when "last"    then receiver.last
      when "reverse" then receiver.reverse
      when "empty?"  then receiver.empty?
      when "to_s"    then receiver.to_s
      when "nil?"    then false
      when "class"   then "List"
      when "rest"    then receiver[1..] || []
      when "sort"    then receiver.sort
      when "uniq"    then receiver.uniq
      when "flatten" then receiver.flatten
      when "at:"
        check_args!(receiver, selector, args, 1); receiver[args[0]]
      when "push:"
        check_args!(receiver, selector, args, 1); receiver + [args[0]]
      when "prepend:"
        check_args!(receiver, selector, args, 1); [args[0]] + receiver
      when "contains:"
        check_args!(receiver, selector, args, 1); receiver.include?(args[0])
      when "join:"
        check_args!(receiver, selector, args, 1); receiver.map(&:to_s).join(args[0])
      when "map:"
        check_args!(receiver, selector, args, 1)
        receiver.map { |el| call_func(args[0], interpreter, [el]) }
      when "filter:"
        check_args!(receiver, selector, args, 1)
        receiver.select { |el| truthy_result?(call_func(args[0], interpreter, [el])) }
      when "reduce:init:"
        check_args!(receiver, selector, args, 2)
        receiver.reduce(args[1]) { |acc, el| call_func(args[0], interpreter, [acc, el]) }
      when "each:"
        check_args!(receiver, selector, args, 1)
        receiver.each { |el| call_func(args[0], interpreter, [el]) }
        nil
      when "any:"
        check_args!(receiver, selector, args, 1)
        receiver.any? { |el| truthy_result?(call_func(args[0], interpreter, [el])) }
      when "all:"
        check_args!(receiver, selector, args, 1)
        receiver.all? { |el| truthy_result?(call_func(args[0], interpreter, [el])) }
      when "none:"
        check_args!(receiver, selector, args, 1)
        receiver.none? { |el| truthy_result?(call_func(args[0], interpreter, [el])) }
      when "take:"
        check_args!(receiver, selector, args, 1); receiver.first(args[0])
      when "drop:"
        check_args!(receiver, selector, args, 1); receiver.drop(args[0])
      when "zip:"
        check_args!(receiver, selector, args, 1); receiver.zip(args[0])
      when "indexOf:"
        check_args!(receiver, selector, args, 1); receiver.index(args[0]) || -1
      when "sortBy:"
        check_args!(receiver, selector, args, 1)
        receiver.sort_by { |el| call_func(args[0], interpreter, [el]) }
      else raise_message_error(receiver, "List", selector)
      end
    end

    def dispatch_hash(receiver, selector, args)
      case selector
      when "keys"    then receiver.keys
      when "values"  then receiver.values
      when "length"  then receiver.length
      when "empty?"  then receiver.empty?
      when "to_s"    then receiver.to_s
      when "nil?"    then false
      when "class"   then "Map"
      when "at:"
        check_args!(receiver, selector, args, 1); receiver[args[0]]
      when "put:value:"
        check_args!(receiver, selector, args, 2); receiver.merge(args[0] => args[1])
      when "remove:"
        check_args!(receiver, selector, args, 1); receiver.reject { |k, _| k == args[0] }
      when "contains:"
        check_args!(receiver, selector, args, 1); receiver.key?(args[0])
      when "merge:"
        check_args!(receiver, selector, args, 1); receiver.merge(args[0])
      else raise_message_error(receiver, "Map", selector)
      end
    end

    def dispatch_function(receiver, selector, args, interpreter)
      case selector
      when "call:"  then receiver.call(interpreter, args)
      when "nil?"   then false
      when "class"  then "Function"
      when "arity"  then receiver.arity
      else raise_message_error(receiver, "Function", selector)
      end
    end

    def dispatch_moof_object(receiver, selector, args, interpreter)
      klass = receiver.klass
      case selector
      when "class"     then klass
      when "className" then klass.name
      when "methods"
        all_methods = []
        k = klass
        while k
          all_methods.concat(k.method_table.keys)
          k = k.superclass
        end
        all_methods.uniq
      when "respondsTo:"
        check_args!(receiver, selector, args, 1)
        !klass.lookup(args[0]).nil?
      when "nil?"  then false
      when "to_s"  then receiver.to_s
      when "set:to:"
        check_args!(receiver, selector, args, 2)
        receiver.set_field(args[0], args[1])
        receiver
      else
        method = klass.lookup(selector)
        if method.nil?
          return receiver.get_field(selector) if receiver.fields.key?(selector)
          raise_message_error(receiver, klass.name, selector)
        end
        invoke_method(method, receiver, args, interpreter)
      end
    end

    def dispatch_moof_class(receiver, selector, args)
      case selector
      when "name"    then receiver.name
      when "fields"  then receiver.fields
      when "methods" then receiver.method_table.keys
      when "nil?"    then false
      when "class"   then "Class"
      else raise_message_error(receiver, "Class(#{receiver.name})", selector)
      end
    end

    def dispatch_boolean(receiver, selector, args)
      case selector
      when "not"   then !receiver
      when "to_s"  then receiver.to_s
      when "nil?"  then false
      when "class" then "Boolean"
      when "and:"
        check_args!(receiver, selector, args, 1); receiver && args[0]
      when "or:"
        check_args!(receiver, selector, args, 1); receiver || args[0]
      else raise_message_error(receiver, receiver.class.to_s, selector)
      end
    end

    def dispatch_nil(_receiver, selector, args)
      case selector
      when "nil?"  then true
      when "to_s"  then "nil"
      when "class" then "Nil"
      else raise_message_error(nil, "nil", selector)
      end
    end

    def dispatch_universal(receiver, selector, args)
      case selector
      when "nil?" then false
      else raise_message_error(receiver, receiver.class.to_s, selector)
      end
    end

    def invoke_method(func, self_obj, args, interpreter)
      # Variadic method support
      if func.respond_to?(:rest_param) && func.rest_param
        if args.length < func.params.length
          raise Moof::ArityError.new("#{func.params.length}+", args.length, name: func.name)
        end
      elsif args.length != func.params.length
        raise Moof::ArityError.new(func.params.length, args.length, name: func.name)
      end
      call_env = func.closure.child
      call_env.define("self", self_obj)
      func.params.each_with_index { |p, i| call_env.define(p, args[i]) }
      if func.respond_to?(:rest_param) && func.rest_param
        call_env.define(func.rest_param, args[func.params.length..] || [])
      end
      interpreter.evaluate_node(func.body, call_env)
    end

    def call_func(func, interpreter, args)
      if func.is_a?(Moof::Function)
        func.call(interpreter, args)
      elsif func.is_a?(Proc)
        func.call(interpreter, args)
      else
        raise Moof::RuntimeError, "Expected a function, got #{func.inspect}"
      end
    end

    def truthy_result?(val)
      val != false && !val.nil?
    end

    def check_args!(receiver, selector, args, expected)
      if args.length != expected
        raise Moof::ArityError.new(expected, args.length, name: "#{receiver.class}##{selector}")
      end
    end
  end
end
