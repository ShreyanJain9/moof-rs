module Moof
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
      check_arity!(args)
      call_env = @closure.child
      @params.each_with_index { |p, i| call_env.define(p, args[i]) }
      if @rest_param
        call_env.define(@rest_param, args[@params.length..] || [])
      end
      interpreter.evaluate_node(@body, call_env)
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
