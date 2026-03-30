require "minitest/autorun"
require_relative "../lib/moof/lexer"
require_relative "../lib/moof/parser"

class ParserTest < Minitest::Test
  def parse(source)
    tokens = Moof::Lexer.new(source).tokenize
    Moof::Parser.new(tokens).parse_program
  end

  def parse_one(source)
    parse(source).expressions.first
  end

  # ── Literals ───────────────────────────────────────────

  def test_integer_literal
    node = parse_one("42")
    assert_instance_of Moof::AST::IntegerLiteral, node
    assert_equal 42, node.value
  end

  def test_float_literal
    node = parse_one("3.14")
    assert_instance_of Moof::AST::FloatLiteral, node
    assert_in_delta 3.14, node.value
  end

  def test_string_literal
    node = parse_one('"hello"')
    assert_instance_of Moof::AST::StringLiteral, node
    assert_equal "hello", node.value
  end

  def test_bool_literal
    assert_instance_of Moof::AST::BoolLiteral, parse_one("true")
    assert_equal true, parse_one("true").value
    assert_equal false, parse_one("false").value
  end

  def test_nil_literal
    assert_instance_of Moof::AST::NilLiteral, parse_one("nil")
  end

  def test_identifier
    node = parse_one("foo")
    assert_instance_of Moof::AST::Identifier, node
    assert_equal "foo", node.name
  end

  # ── S-Expressions / Calls ──────────────────────────────

  def test_simple_call
    node = parse_one("(+ 1 2)")
    assert_instance_of Moof::AST::Call, node
    assert_equal "+", node.callee.name
    assert_equal 2, node.arguments.length
    assert_equal 1, node.arguments[0].value
    assert_equal 2, node.arguments[1].value
  end

  def test_nested_call
    node = parse_one("(+ (* 2 3) 4)")
    assert_instance_of Moof::AST::Call, node
    inner = node.arguments[0]
    assert_instance_of Moof::AST::Call, inner
    assert_equal "*", inner.callee.name
  end

  def test_empty_parens_is_nil
    node = parse_one("()")
    assert_instance_of Moof::AST::NilLiteral, node
  end

  # ── Define ─────────────────────────────────────────────

  def test_define_variable
    node = parse_one("(define x 42)")
    assert_instance_of Moof::AST::Define, node
    assert_equal "x", node.name
    assert_equal 42, node.value.value
  end

  def test_define_function
    node = parse_one("(define (square x) (* x x))")
    assert_instance_of Moof::AST::DefineFunction, node
    assert_equal "square", node.name
    assert_equal ["x"], node.params
    assert_instance_of Moof::AST::Call, node.body
  end

  def test_define_function_multi_body
    node = parse_one("(define (f x) (print x) (+ x 1))")
    assert_instance_of Moof::AST::DefineFunction, node
    # Multi-body wraps in Do
    assert_instance_of Moof::AST::Do, node.body
    assert_equal 2, node.body.expressions.length
  end

  # ── Lambda ─────────────────────────────────────────────

  def test_lambda
    node = parse_one("(lambda (x y) (+ x y))")
    assert_instance_of Moof::AST::Lambda, node
    assert_equal ["x", "y"], node.params
    assert_instance_of Moof::AST::Call, node.body
  end

  # ── If ─────────────────────────────────────────────────

  def test_if_with_else
    node = parse_one('(if true "yes" "no")')
    assert_instance_of Moof::AST::If, node
    assert_instance_of Moof::AST::BoolLiteral, node.condition
    assert_equal "yes", node.then_branch.value
    assert_equal "no", node.else_branch.value
  end

  def test_if_without_else
    node = parse_one('(if true "yes")')
    assert_instance_of Moof::AST::If, node
    assert_nil node.else_branch
  end

  # ── Let ────────────────────────────────────────────────

  def test_let
    node = parse_one("(let ((x 1) (y 2)) (+ x y))")
    assert_instance_of Moof::AST::Let, node
    assert_equal 2, node.bindings.length
    assert_equal "x", node.bindings[0][0]
    assert_equal 1, node.bindings[0][1].value
    assert_equal "y", node.bindings[1][0]
    assert_equal 2, node.bindings[1][1].value
  end

  # ── Do ─────────────────────────────────────────────────

  def test_do
    node = parse_one("(do 1 2 3)")
    assert_instance_of Moof::AST::Do, node
    assert_equal 3, node.expressions.length
  end

  # ── SetBang ────────────────────────────────────────────

  def test_set_bang
    node = parse_one("(set! x 99)")
    assert_instance_of Moof::AST::SetBang, node
    assert_equal "x", node.name
    assert_equal 99, node.value.value
  end

  # ── Quote ──────────────────────────────────────────────

  def test_quote_form
    node = parse_one("(quote (1 2 3))")
    assert_instance_of Moof::AST::Quote, node
  end

  def test_quote_sugar
    node = parse_one("'(1 2 3)")
    assert_instance_of Moof::AST::Quote, node
  end

  def test_quote_sugar_identifier
    node = parse_one("'foo")
    assert_instance_of Moof::AST::Quote, node
    assert_instance_of Moof::AST::Identifier, node.expression
    assert_equal "foo", node.expression.name
  end

  # ── TryCatch ───────────────────────────────────────────

  def test_try_catch
    node = parse_one('(try (/ 1 0) (catch e (print "error")))')
    assert_instance_of Moof::AST::TryCatch, node
    assert_instance_of Moof::AST::Call, node.body
    assert_equal "e", node.error_name
    assert_instance_of Moof::AST::Call, node.catch_body
  end

  # ── Cond ───────────────────────────────────────────────

  def test_cond
    node = parse_one('(cond ((> x 0) "positive") ((< x 0) "negative") (else "zero"))')
    assert_instance_of Moof::AST::Cond, node
    assert_equal 3, node.clauses.length
    assert_equal :else, node.clauses[2][0]
  end

  # ── Message Sends ──────────────────────────────────────

  def test_unary_message
    node = parse_one("[obj method]")
    assert_instance_of Moof::AST::MessageSend, node
    assert_equal "obj", node.receiver.name
    assert_equal "method", node.selector
    assert_empty node.arguments
  end

  def test_positional_message
    node = parse_one("[list at 0]")
    assert_instance_of Moof::AST::MessageSend, node
    assert_equal "list", node.receiver.name
    assert_equal "at", node.selector
    assert_equal 1, node.arguments.length
    assert_equal 0, node.arguments[0].value
  end

  def test_positional_message_multiple_args
    node = parse_one("[console log 1 2 3]")
    assert_instance_of Moof::AST::MessageSend, node
    assert_equal "log", node.selector
    assert_equal 3, node.arguments.length
  end

  def test_keyword_message_single
    node = parse_one("[m at: 0]")
    assert_instance_of Moof::AST::MessageSend, node
    assert_equal "at:", node.selector
    assert_equal 1, node.arguments.length
  end

  def test_keyword_message_multiple
    node = parse_one('[dict insertValue: 42 forKey: "x"]')
    assert_instance_of Moof::AST::MessageSend, node
    assert_equal "insertValue:forKey:", node.selector
    assert_equal 2, node.arguments.length
    assert_equal 42, node.arguments[0].value
    assert_equal "x", node.arguments[1].value
  end

  def test_message_with_nested_sexpr_arg
    node = parse_one("[obj method: (+ 1 2)]")
    assert_instance_of Moof::AST::MessageSend, node
    assert_equal "method:", node.selector
    assert_instance_of Moof::AST::Call, node.arguments[0]
  end

  def test_message_with_nested_message_arg
    node = parse_one("[obj method: [other value]]")
    assert_instance_of Moof::AST::MessageSend, node
    inner = node.arguments[0]
    assert_instance_of Moof::AST::MessageSend, inner
    assert_equal "value", inner.selector
  end

  # ── Nesting [] inside () ───────────────────────────────

  def test_brackets_inside_parens
    node = parse_one("(if (> [list length] 0) [list at 0] nil)")
    assert_instance_of Moof::AST::If, node
    # condition is (> [list length] 0)
    cond_call = node.condition
    assert_instance_of Moof::AST::Call, cond_call
    assert_instance_of Moof::AST::MessageSend, cond_call.arguments[0]
  end

  def test_parens_inside_brackets
    node = parse_one("[obj doSomethingWith: (+ 1 2) and: [other value]]")
    assert_instance_of Moof::AST::MessageSend, node
    assert_equal "doSomethingWith:and:", node.selector
    assert_instance_of Moof::AST::Call, node.arguments[0]
    assert_instance_of Moof::AST::MessageSend, node.arguments[1]
  end

  # ── Map Literals ───────────────────────────────────────

  def test_map_literal
    node = parse_one('{name: "moof" version: 1}')
    assert_instance_of Moof::AST::MapLiteral, node
    assert_equal 2, node.pairs.length
    assert_equal "name", node.pairs[0][0]
    assert_equal "moof", node.pairs[0][1].value
    assert_equal "version", node.pairs[1][0]
    assert_equal 1, node.pairs[1][1].value
  end

  def test_empty_map
    node = parse_one("{}")
    assert_instance_of Moof::AST::MapLiteral, node
    assert_empty node.pairs
  end

  def test_map_with_nested_expressions
    node = parse_one("{total: (+ 1 2)}")
    assert_instance_of Moof::AST::MapLiteral, node
    assert_equal "total", node.pairs[0][0]
    assert_instance_of Moof::AST::Call, node.pairs[0][1]
  end

  # ── Multiple Expressions ───────────────────────────────

  def test_program_multiple_expressions
    prog = parse("(define x 1) (define y 2) (+ x y)")
    assert_equal 3, prog.expressions.length
  end

  # ── Error Cases ────────────────────────────────────────

  def test_missing_closing_paren
    assert_raises(Moof::SyntaxError) { parse("(+ 1 2") }
  end

  def test_missing_closing_bracket
    assert_raises(Moof::SyntaxError) { parse("[obj method") }
  end

  def test_empty_message_send
    assert_raises(Moof::SyntaxError) { parse("[obj]") }
  end
end
