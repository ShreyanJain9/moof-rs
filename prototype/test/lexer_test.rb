require "minitest/autorun"
require_relative "../lib/moof/lexer"

class LexerTest < Minitest::Test
  include Moof::TokenType

  def tokenize(source)
    Moof::Lexer.new(source).tokenize
  end

  def types(source)
    tokenize(source).map(&:type)
  end

  # ── Delimiters ─────────────────────────────────────────

  def test_parens
    assert_equal [LPAREN, RPAREN, EOF], types("()")
  end

  def test_brackets
    assert_equal [LBRACKET, RBRACKET, EOF], types("[]")
  end

  def test_braces
    assert_equal [LBRACE, RBRACE, EOF], types("{}")
  end

  def test_quote
    assert_equal [QUOTE, LPAREN, INTEGER, INTEGER, RPAREN, EOF], types("'(1 2)")
  end

  # ── Integers ───────────────────────────────────────────

  def test_integer
    tok = tokenize("42").first
    assert_equal INTEGER, tok.type
    assert_equal 42, tok.literal
  end

  def test_negative_integer
    tok = tokenize("-7").first
    assert_equal INTEGER, tok.type
    assert_equal(-7, tok.literal)
  end

  def test_hex_integer
    tok = tokenize("0xFF").first
    assert_equal INTEGER, tok.type
    assert_equal 255, tok.literal
  end

  def test_hex_lowercase
    tok = tokenize("0xff").first
    assert_equal INTEGER, tok.type
    assert_equal 255, tok.literal
  end

  # ── Floats ─────────────────────────────────────────────

  def test_float
    tok = tokenize("3.14").first
    assert_equal FLOAT, tok.type
    assert_in_delta 3.14, tok.literal
  end

  def test_negative_float
    tok = tokenize("-0.5").first
    assert_equal FLOAT, tok.type
    assert_in_delta(-0.5, tok.literal)
  end

  def test_scientific_notation
    tok = tokenize("1e10").first
    assert_equal FLOAT, tok.type
    assert_in_delta 1e10, tok.literal
  end

  def test_scientific_with_decimal
    tok = tokenize("2.5e3").first
    assert_equal FLOAT, tok.type
    assert_in_delta 2500.0, tok.literal
  end

  # ── Strings ────────────────────────────────────────────

  def test_simple_string
    tok = tokenize('"hello"').first
    assert_equal STRING, tok.type
    assert_equal "hello", tok.literal
  end

  def test_string_escape_newline
    tok = tokenize('"line\nbreak"').first
    assert_equal "line\nbreak", tok.literal
  end

  def test_string_escape_tab
    tok = tokenize('"col\there"').first
    assert_equal "col\there", tok.literal
  end

  def test_string_escape_backslash
    tok = tokenize('"back\\\\slash"').first
    assert_equal "back\\slash", tok.literal
  end

  def test_string_escape_quote
    tok = tokenize('"say \\"hi\\""').first
    assert_equal 'say "hi"', tok.literal
  end

  def test_unterminated_string
    assert_raises(Moof::SyntaxError) { tokenize('"oops') }
  end

  # ── Booleans & Nil ─────────────────────────────────────

  def test_true
    tok = tokenize("true").first
    assert_equal TRUE, tok.type
    assert_equal true, tok.literal
  end

  def test_false
    tok = tokenize("false").first
    assert_equal FALSE, tok.type
    assert_equal false, tok.literal
  end

  def test_nil
    tok = tokenize("nil").first
    assert_equal NIL, tok.type
    assert_nil tok.literal
  end

  # ── Identifiers ────────────────────────────────────────

  def test_simple_identifier
    tok = tokenize("foo").first
    assert_equal IDENTIFIER, tok.type
    assert_equal "foo", tok.lexeme
  end

  def test_operator_identifier
    tok = tokenize("+").first
    assert_equal IDENTIFIER, tok.type
    assert_equal "+", tok.lexeme
  end

  def test_comparison_identifier
    tok = tokenize(">=").first
    assert_equal IDENTIFIER, tok.type
    assert_equal ">=", tok.lexeme
  end

  def test_predicate_identifier
    tok = tokenize("eq?").first
    assert_equal IDENTIFIER, tok.type
    assert_equal "eq?", tok.lexeme
  end

  def test_bang_identifier
    tok = tokenize("set!").first
    assert_equal IDENTIFIER, tok.type
    assert_equal "set!", tok.lexeme
  end

  def test_hyphenated_identifier
    tok = tokenize("my-var").first
    assert_equal IDENTIFIER, tok.type
    assert_equal "my-var", tok.lexeme
  end

  # ── Colon IDs ──────────────────────────────────────────

  def test_colon_id
    tok = tokenize("insertValue:").first
    assert_equal COLON_ID, tok.type
    assert_equal "insertValue:", tok.lexeme
  end

  def test_multiple_colon_ids
    toks = tokenize("insertValue: forKey:")
    assert_equal COLON_ID, toks[0].type
    assert_equal "insertValue:", toks[0].lexeme
    assert_equal COLON_ID, toks[1].type
    assert_equal "forKey:", toks[1].lexeme
  end

  # ── Comments ───────────────────────────────────────────

  def test_single_line_comment
    toks = tokenize("; this is a comment\n42")
    assert_equal [INTEGER, EOF], toks.map(&:type)
  end

  def test_block_comment
    toks = tokenize("#| block comment |# 42")
    assert_equal [INTEGER, EOF], toks.map(&:type)
  end

  def test_nested_block_comment
    toks = tokenize("#| outer #| inner |# still comment |# 42")
    assert_equal [INTEGER, EOF], toks.map(&:type)
  end

  def test_unterminated_block_comment
    assert_raises(Moof::SyntaxError) { tokenize("#| oops") }
  end

  # ── Line/Column Tracking ───────────────────────────────

  def test_line_tracking
    toks = tokenize("42\n\"hi\"")
    assert_equal 1, toks[0].line
    assert_equal 2, toks[1].line
  end

  def test_column_tracking
    toks = tokenize("  42")
    assert_equal 3, toks[0].column
  end

  # ── Full Expression ────────────────────────────────────

  def test_s_expression
    expected = [LPAREN, IDENTIFIER, INTEGER, INTEGER, RPAREN, EOF]
    assert_equal expected, types("(+ 1 2)")
  end

  def test_message_send
    expected = [LBRACKET, IDENTIFIER, IDENTIFIER, RBRACKET, EOF]
    assert_equal expected, types("[obj method]")
  end

  def test_keyword_message
    expected = [LBRACKET, IDENTIFIER, COLON_ID, INTEGER, COLON_ID, STRING, RBRACKET, EOF]
    assert_equal expected, types('[dict insertValue: 42 forKey: "x"]')
  end

  def test_map_literal
    expected = [LBRACE, COLON_ID, STRING, COLON_ID, INTEGER, RBRACE, EOF]
    assert_equal expected, types('{name: "moof" version: 1}')
  end

  def test_unexpected_character
    assert_raises(Moof::SyntaxError) { tokenize("@") }
  end
end
