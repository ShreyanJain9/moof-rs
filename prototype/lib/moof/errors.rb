module Moof
  class MoofError < StandardError; end

  class SyntaxError < MoofError
    attr_reader :line, :column

    def initialize(message, line: nil, column: nil)
      @line = line
      @column = column
      loc = line ? " at line #{line}" : ""
      loc += ":#{column}" if column
      super("#{message}#{loc}")
    end
  end

  class RuntimeError < MoofError; end

  class NameError < RuntimeError
    attr_reader :line, :column
    def initialize(name, line: nil, column: nil)
      @line, @column = line, column
      loc = line ? " at line #{line}" : ""
      loc += ":#{column}" if column && line
      super("Undefined variable: #{name}#{loc}")
    end
  end

  class MessageError < RuntimeError
    def initialize(receiver_class, selector)
      super("Message not understood: #{receiver_class} does not respond to '#{selector}'")
    end
  end

  class ImmutableBindingError < RuntimeError
    def initialize(name)
      super("Cannot mutate immutable binding: #{name}")
    end
  end

  class ArityError < RuntimeError
    def initialize(expected, got, name: nil)
      fn = name ? " for '#{name}'" : ""
      super("Wrong number of arguments#{fn}: expected #{expected}, got #{got}")
    end
  end
end
