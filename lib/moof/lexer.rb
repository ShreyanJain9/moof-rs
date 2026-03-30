require "strscan"
require_relative "token"
require_relative "errors"

module Moof
  class Lexer
    include TokenType

    def initialize(source, filename: "(eval)")
      @source = source
      @filename = filename
      @scanner = StringScanner.new(source)
      @tokens = []
      @line = 1
      @line_start = 0 # byte offset of current line start
    end

    def tokenize
      until @scanner.eos?
        skip_whitespace_and_comments
        break if @scanner.eos?

        start_col = current_column
        start_line = @line

        token = scan_token(start_line, start_col)
        @tokens << token if token
      end

      @tokens << Token.new(type: EOF, lexeme: "", literal: nil, line: @line, column: current_column)
      @tokens
    end

    private

    def current_column
      @scanner.pos - @line_start + 1
    end

    def skip_whitespace_and_comments
      loop do
        # Whitespace
        if @scanner.scan(/[ \t\r]+/)
          next
        end

        # Newlines
        if @scanner.scan(/\n/)
          @line += 1
          @line_start = @scanner.pos
          next
        end

        # Single-line comment
        if @scanner.scan(/;[^\n]*/)
          next
        end

        # Block comment #| ... |# (nestable)
        if @scanner.match?(/#\|/)
          skip_block_comment
          next
        end

        break
      end
    end

    def skip_block_comment
      @scanner.skip(/#\|/)
      depth = 1

      while depth > 0
        if @scanner.eos?
          raise Moof::SyntaxError.new("Unterminated block comment", line: @line, column: current_column)
        end

        if @scanner.scan(/#\|/)
          depth += 1
        elsif @scanner.scan(/\|#/)
          depth -= 1
        elsif @scanner.scan(/\n/)
          @line += 1
          @line_start = @scanner.pos
        else
          @scanner.getch
        end
      end
    end

    def scan_token(line, col)
      # Single-character tokens
      case
      when @scanner.scan(/\(/)
        make_token(LPAREN, "(", nil, line, col)
      when @scanner.scan(/\)/)
        make_token(RPAREN, ")", nil, line, col)
      when @scanner.scan(/\[/)
        make_token(LBRACKET, "[", nil, line, col)
      when @scanner.scan(/\]/)
        make_token(RBRACKET, "]", nil, line, col)
      when @scanner.scan(/\{/)
        make_token(LBRACE, "{", nil, line, col)
      when @scanner.scan(/\}/)
        make_token(RBRACE, "}", nil, line, col)
      when @scanner.scan(/'/)
        make_token(QUOTE, "'", nil, line, col)

      # Backtick (quasiquote)
      when @scanner.scan(/`/)
        make_token(BACKTICK, "`", nil, line, col)

      # Comma-at (unquote-splice) — must check before plain comma
      when @scanner.scan(/,@/)
        make_token(COMMA_AT, ",@", nil, line, col)

      # Comma (unquote)
      when @scanner.scan(/,/)
        make_token(COMMA, ",", nil, line, col)

      # Ampersand (selector ref)
      when @scanner.scan(/&/)
        make_token(AMPERSAND, "&", nil, line, col)

      # Pipe
      when @scanner.scan(/\|/)
        make_token(PIPE, "|", nil, line, col)

      # Dot (rest parameter separator) — only if not followed by digit (which would be a float)
      when @scanner.check(/\.(?![0-9])/)
        @scanner.skip(/\./)
        make_token(DOT, ".", nil, line, col)

      # Interpolated string $"..."
      when @scanner.check(/\$"/)
        scan_interp_string(line, col)

      # String literal
      when @scanner.check(/"/)
        scan_string(line, col)

      # Numbers: hex, float, integer
      when @scanner.check(/0[xX][0-9a-fA-F]/)
        lexeme = @scanner.scan(/0[xX][0-9a-fA-F_]+/)
        make_token(INTEGER, lexeme, lexeme.to_i(16), line, col)

      # Negative numbers: only if followed by a digit (and optionally a dot)
      when @scanner.check(/-[0-9]/)
        scan_number(line, col)

      # Positive numbers
      when @scanner.check(/[0-9]/)
        scan_number(line, col)

      # Identifiers and keywords
      when @scanner.check(/[a-zA-Z_!?\*\/\+\-<>=%]/)
        scan_identifier(line, col)

      else
        ch = @scanner.getch
        raise Moof::SyntaxError.new("Unexpected character: #{ch.inspect}", line: line, column: col)
      end
    end

    def scan_number(line, col)
      # Try float patterns first (with decimal or scientific notation)
      if (lexeme = @scanner.scan(/-?[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?/))
        make_token(FLOAT, lexeme, lexeme.to_f, line, col)
      elsif (lexeme = @scanner.scan(/-?[0-9]+[eE][+-]?[0-9]+/))
        make_token(FLOAT, lexeme, lexeme.to_f, line, col)
      elsif (lexeme = @scanner.scan(/-?[0-9]+/))
        make_token(INTEGER, lexeme, lexeme.to_i, line, col)
      end
    end

    IDENTIFIER_CHARS = /[a-zA-Z0-9_!?\*\/\+\-<>=]/

    def scan_identifier(line, col)
      lexeme = @scanner.scan(/[a-zA-Z_!?\*\/\+\-<>=%][a-zA-Z0-9_!?\*\/\+\-<>=]*/)

      # Check if it ends with a colon (keyword selector like insertValue:)
      if @scanner.check(/:/) && !@scanner.check(/::/)
        @scanner.skip(/:/)
        lexeme += ":"
        return make_token(COLON_ID, lexeme, lexeme, line, col)
      end

      case lexeme
      when "true"
        make_token(TRUE, lexeme, true, line, col)
      when "false"
        make_token(FALSE, lexeme, false, line, col)
      when "nil"
        make_token(NIL, lexeme, nil, line, col)
      else
        make_token(IDENTIFIER, lexeme, lexeme, line, col)
      end
    end

    ESCAPE_MAP = {
      "n" => "\n",
      "t" => "\t",
      "\\" => "\\",
      '"' => '"',
    }.freeze

    def scan_string(line, col)
      @scanner.skip(/"/)
      value = +""
      lexeme_start = @scanner.pos - 1

      loop do
        if @scanner.eos?
          raise Moof::SyntaxError.new("Unterminated string", line: line, column: col)
        end

        if @scanner.scan(/\\(.)/)
          ch = @scanner[1]
          value << (ESCAPE_MAP[ch] || ch)
        elsif @scanner.check(/"/)
          @scanner.skip(/"/)
          break
        elsif @scanner.scan(/\n/)
          @line += 1
          @line_start = @scanner.pos
          value << "\n"
        else
          value << @scanner.getch
        end
      end

      lexeme = @source[lexeme_start..(@scanner.pos - 1)]
      make_token(STRING, lexeme, value, line, col)
    end

    # Scan interpolated string: $"text \(expr) text"
    # Produces INTERP_STRING token with literal = array of [:str, text] / [:expr, source]
    def scan_interp_string(line, col)
      @scanner.skip(/\$"/)
      segments = []
      current_text = +""

      loop do
        if @scanner.eos?
          raise Moof::SyntaxError.new("Unterminated interpolated string", line: line, column: col)
        end

        # End of string
        if @scanner.check(/"/)
          @scanner.skip(/"/)
          segments << [:str, current_text] unless current_text.empty?
          break
        end

        # Interpolation: \( ... )
        if @scanner.check(/\\\(/)
          segments << [:str, current_text] unless current_text.empty?
          current_text = +""
          @scanner.skip(/\\\(/)

          # Read until matching closing paren, tracking nesting
          depth = 1
          expr_source = +""
          while depth > 0
            if @scanner.eos?
              raise Moof::SyntaxError.new("Unterminated interpolation in string", line: line, column: col)
            end
            if @scanner.check(/\(/)
              depth += 1
              expr_source << @scanner.getch
            elsif @scanner.check(/\)/)
              depth -= 1
              if depth > 0
                expr_source << @scanner.getch
              else
                @scanner.skip(/\)/)
              end
            elsif @scanner.scan(/\n/)
              @line += 1
              @line_start = @scanner.pos
              expr_source << "\n"
            else
              expr_source << @scanner.getch
            end
          end

          segments << [:expr, expr_source]
          next
        end

        # Escape sequences (same as regular strings, but \( is interpolation)
        if @scanner.scan(/\\(.)/)
          ch = @scanner[1]
          current_text << (ESCAPE_MAP[ch] || ch)
        elsif @scanner.scan(/\n/)
          @line += 1
          @line_start = @scanner.pos
          current_text << "\n"
        else
          current_text << @scanner.getch
        end
      end

      make_token(INTERP_STRING, "$\"...\"", segments, line, col)
    end

    def make_token(type, lexeme, literal, line, col)
      Token.new(type: type, lexeme: lexeme, literal: literal, line: line, column: col)
    end
  end
end
