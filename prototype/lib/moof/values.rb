module Moof
  class SymbolValue
    attr_reader :name

    def initialize(name)
      @name = name
    end

    def ==(other)
      other.is_a?(SymbolValue) && other.name == @name
    end

    def eql?(other)
      self == other
    end

    def hash
      @name.hash
    end

    def to_s
      "'#{@name}"
    end

    def inspect
      "SymbolValue(#{@name})"
    end
  end
end
