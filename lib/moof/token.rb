module Moof
  Token = Data.define(:type, :lexeme, :literal, :line, :column) do
    def to_s
      "#{type}(#{lexeme.inspect})"
    end
  end

  # Token type constants
  module TokenType
    LPAREN    = :LPAREN      # (
    RPAREN    = :RPAREN      # )
    LBRACKET  = :LBRACKET    # [
    RBRACKET  = :RBRACKET    # ]
    LBRACE    = :LBRACE      # {
    RBRACE    = :RBRACE      # }

    INTEGER   = :INTEGER     # 42, 0xFF
    FLOAT     = :FLOAT       # 3.14, 1e10
    STRING    = :STRING      # "hello"
    TRUE      = :TRUE        # true
    FALSE     = :FALSE       # false
    NIL       = :NIL         # nil

    IDENTIFIER = :IDENTIFIER # foo, +, my-var
    COLON_ID   = :COLON_ID   # insertValue: (keyword in message sends)

    QUOTE     = :QUOTE       # '
    DOT       = :DOT         # . (rest parameter separator)

    # New tokens
    PIPE          = :PIPE          # |
    BACKTICK      = :BACKTICK      # `
    COMMA         = :COMMA         # ,
    COMMA_AT      = :COMMA_AT      # ,@
    AMPERSAND     = :AMPERSAND     # &
    INTERP_STRING = :INTERP_STRING # $"..."

    EOF       = :EOF
  end
end
