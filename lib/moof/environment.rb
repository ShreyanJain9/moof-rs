module Moof
  class Environment
    attr_reader :parent

    def initialize(parent = nil)
      @parent = parent
      @store = {} # name => { value:, mutable: }
    end

    # Creates a binding in the current scope.
    def define(name, value, mutable: true)
      @store[name] = { value: value, mutable: mutable }
      value
    end

    # Walks the scope chain to find a binding.
    # Raises Moof::NameError if not found.
    def get(name)
      if @store.key?(name)
        @store[name][:value]
      elsif @parent
        @parent.get(name)
      else
        raise Moof::NameError.new(name)
      end
    end

    # Finds a binding in the chain and updates it.
    # Raises ImmutableBindingError if the binding is immutable.
    # Raises NameError if not found.
    def set(name, value)
      if @store.key?(name)
        unless @store[name][:mutable]
          raise Moof::ImmutableBindingError.new(name)
        end
        @store[name][:value] = value
        value
      elsif @parent
        @parent.set(name, value)
      else
        raise Moof::NameError.new(name)
      end
    end

    # Creates a new child scope with self as parent.
    def child
      Environment.new(self)
    end

    # Returns a hash of bindings for REPL introspection.
    def bindings
      @store.transform_values { |entry| entry[:value] }
    end
  end
end
