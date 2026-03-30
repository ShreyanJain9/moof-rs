module Moof
  TailCall = Struct.new(:func, :args)

  class Function
    attr_reader :params, :rest_param, :body, :closure, :name

    def initialize(params:, body:, closure:, name: nil, rest_param: nil)
      @params     = params
      @rest_param = rest_param
      @body       = body
      @closure    = closure
      @name       = name
    end

    def call(interpreter, args)
      current_args = args
      current_func = self
      loop do
        current_func.send(:check_arity!, current_args)
        call_env = current_func.closure.child
        current_func.params.each_with_index { |p, i| call_env.define(p, current_args[i]) }
        if current_func.rest_param
          call_env.define(current_func.rest_param, current_args[current_func.params.length..] || [])
        end
        result = interpreter.evaluate_tail(current_func.body, call_env)
        if result.is_a?(TailCall)
          current_func = result.func
          current_args = result.args
        else
          return result
        end
      end
    end

    def arity; @params.length; end
    def variadic?; !@rest_param.nil?; end

    def to_s
      n = variadic? ? "#{arity}+" : arity.to_s
      @name ? "<function:#{@name}/#{n}>" : "<lambda/#{n}>"
    end
    def inspect; to_s; end

    private

    def check_arity!(args)
      if variadic?
        if args.length < @params.length
          raise Moof::ArityError.new("#{@params.length}+", args.length, name: @name)
        end
      elsif args.length != @params.length
        raise Moof::ArityError.new(@params.length, args.length, name: @name)
      end
    end
  end
end
