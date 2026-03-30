module Moof
  module Printer
    module Colors
      def self.enabled?; $stdout.tty?; end

      def self.ansi(code, s)
        enabled? ? "\e[#{code}m#{s}\e[0m" : s
      end

      def self.green(s)   = ansi(32, s)
      def self.red(s)     = ansi(31, s)
      def self.cyan(s)    = ansi(36, s)
      def self.yellow(s)  = ansi(33, s)
      def self.blue(s)    = ansi(34, s)
      def self.magenta(s) = ansi(35, s)
      def self.bold(s)    = ansi(1, s)
      def self.dim(s)     = ansi(2, s)
      def self.italic(s)  = ansi(3, s)

      # Compound styles
      def self.keyword(s)  = ansi(35, s)  # magenta
      def self.string(s)   = ansi(32, s)  # green
      def self.number(s)   = ansi(33, s)  # yellow
      def self.comment(s)  = ansi(2, s)   # dim
      def self.error_hl(s) = ansi(31, s)  # red
      def self.type_hl(s)  = ansi(36, s)  # cyan
      def self.fn_hl(s)    = ansi(34, s)  # blue
      def self.class_hl(s) = bold(ansi(33, s))  # bold yellow
    end

    # Format a Moof value for display — with optional color
    def self.format(value, color: Colors.enabled?, indent: 0, max_depth: 8)
      return "..." if indent > max_depth
      case value
      when Integer
        s = value.to_s
        color ? Colors.number(s) : s
      when Float
        s = value.to_s
        color ? Colors.number(s) : s
      when String
        s = value.inspect
        color ? Colors.string(s) : s
      when true
        s = "true"
        color ? Colors.keyword(s) : s
      when false
        s = "false"
        color ? Colors.keyword(s) : s
      when nil
        s = "nil"
        color ? Colors.keyword(s) : s
      when Array
        format_list(value, color: color, indent: indent, max_depth: max_depth)
      when Hash
        format_map(value, color: color, indent: indent, max_depth: max_depth)
      when Moof::SymbolValue
        s = "'#{value.name}"
        color ? Colors.cyan(s) : s
      when Moof::Function
        format_function(value, color: color)
      when Moof::MoofObject
        format_object(value, color: color, indent: indent, max_depth: max_depth)
      when Moof::MoofClass
        s = "#<Class #{value.name}>"
        color ? Colors.class_hl(s) : s
      when Moof::Protocol
        s = "#<Protocol #{value.name} [#{value.selectors.join(" ")}]>"
        color ? Colors.magenta(s) : s
      when Moof::TailCall
        s = "#<TailCall>"
        color ? Colors.dim(s) : s
      else
        value.to_s
      end
    end

    def self.format_list(arr, color:, indent:, max_depth:)
      if arr.empty?
        color ? Colors.dim("()") : "()"
      elsif arr.length <= 8 && !arr.any? { |e| e.is_a?(Array) || e.is_a?(Hash) || e.is_a?(MoofObject) }
        # Short list — inline
        inner = arr.map { |e| format(e, color: color, indent: indent + 1, max_depth: max_depth) }.join(" ")
        "(#{inner})"
      else
        # Long or nested list — one per line
        pad = "  " * (indent + 1)
        items = arr.map { |e| "#{pad}#{format(e, color: color, indent: indent + 1, max_depth: max_depth)}" }
        "(\n#{items.join("\n")}\n#{"  " * indent})"
      end
    end

    def self.format_map(hash, color:, indent:, max_depth:)
      if hash.empty?
        color ? Colors.dim("{}") : "{}"
      elsif hash.length <= 4
        pairs = hash.map do |k, v|
          key = color ? Colors.cyan("#{k}:") : "#{k}:"
          "#{key} #{format(v, color: color, indent: indent + 1, max_depth: max_depth)}"
        end
        "{#{pairs.join(" ")}}"
      else
        pad = "  " * (indent + 1)
        pairs = hash.map do |k, v|
          key = color ? Colors.cyan("#{k}:") : "#{k}:"
          "#{pad}#{key} #{format(v, color: color, indent: indent + 1, max_depth: max_depth)}"
        end
        "{\n#{pairs.join("\n")}\n#{"  " * indent}}"
      end
    end

    def self.format_function(func, color:)
      if func.name
        params_str = func.params.join(" ")
        params_str += " . #{func.rest_param}" if func.rest_param
        s = "#<fn #{func.name}(#{params_str})>"
        color ? Colors.fn_hl(s) : s
      else
        params_str = func.params.join(" ")
        params_str += " . #{func.rest_param}" if func.rest_param
        s = "#<lambda(#{params_str})>"
        color ? Colors.fn_hl(s) : s
      end
    end

    def self.format_object(obj, color:, indent:, max_depth:)
      klass = obj.klass
      name = color ? Colors.class_hl(klass.name) : klass.name
      if obj.fields.empty?
        name
      elsif klass.own_fields.length <= 3
        pairs = obj.fields.map do |k, v|
          key = color ? Colors.dim("#{k}:") : "#{k}:"
          "#{key} #{format(v, color: color, indent: indent + 1, max_depth: max_depth)}"
        end
        "(#{name} #{pairs.join(" ")})"
      else
        pad = "  " * (indent + 1)
        pairs = obj.fields.map do |k, v|
          key = color ? Colors.dim("#{k}:") : "#{k}:"
          "#{pad}#{key} #{format(v, color: color, indent: indent + 1, max_depth: max_depth)}"
        end
        "(#{name}\n#{pairs.join("\n")}\n#{"  " * indent})"
      end
    end

    # Plain format (no colors, for print/display)
    def self.plain(value)
      format(value, color: false)
    end

    def self.type_of(value)
      case value
      when Integer          then "Integer"
      when Float            then "Float"
      when String           then "String"
      when true, false      then "Bool"
      when nil              then "Nil"
      when Array            then "List(#{value.length})"
      when Hash             then "Map(#{value.length})"
      when Moof::SymbolValue  then "Symbol"
      when Moof::Function
        value.name ? "Function" : "Lambda"
      when Moof::MoofObject   then value.klass.name
      when Moof::MoofClass    then "Class"
      when Moof::Protocol     then "Protocol"
      when Moof::TailCall     then "TailCall"
      else value.class.name
      end
    end

    # Format an error with source context
    def self.format_error(err, source: nil)
      msg = Colors.red("Error: #{err.message}")
      if source && err.respond_to?(:line) && err.line
        lines = source.split("\n")
        line_idx = err.line - 1
        if line_idx >= 0 && line_idx < lines.length
          msg += "\n"
          # Show surrounding context
          start = [line_idx - 1, 0].max
          stop  = [line_idx + 1, lines.length - 1].min
          (start..stop).each do |i|
            ln = Colors.dim("#{(i + 1).to_s.rjust(4)} | ")
            if i == line_idx
              msg += ln + Colors.bold(lines[i]) + "\n"
              if err.respond_to?(:column) && err.column
                pointer = " " * (7 + err.column - 1) + Colors.red("^")
                msg += pointer + "\n"
              end
            else
              msg += ln + Colors.dim(lines[i]) + "\n"
            end
          end
        end
      end
      msg
    end

    # Syntax-highlight Moof source code for display
    def self.highlight(source)
      return source unless Colors.enabled?
      # Simple regex-based highlighter
      result = source.dup
      # We do this token-by-token to avoid overlapping replacements
      tokens = []
      scanner = StringScanner.new(source)
      out = +""
      until scanner.eos?
        if scanner.scan(/;[^\n]*/)
          out << Colors.comment(scanner.matched)
        elsif scanner.scan(/"(?:[^"\\]|\\.)*"/)
          out << Colors.string(scanner.matched)
        elsif scanner.scan(/\b(define|lambda|if|let|do|set!|quote|try|catch|cond|and|or|class|trait|method|fields|extends|uses)\b/)
          out << Colors.keyword(scanner.matched)
        elsif scanner.scan(/\b(true|false|nil)\b/)
          out << Colors.keyword(scanner.matched)
        elsif scanner.scan(/-?0[xX][0-9a-fA-F]+|-?[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?|-?[0-9]+[eE][+-]?[0-9]+|-?[0-9]+/)
          out << Colors.number(scanner.matched)
        else
          out << scanner.getch
        end
      end
      out
    end
  end
end
