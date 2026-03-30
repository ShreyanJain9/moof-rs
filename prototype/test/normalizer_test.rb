require "minitest/autorun"
require_relative "../lib/moof/lexer"
require_relative "../lib/moof/parser"
require_relative "../lib/moof/normalizer"

class NormalizerTest < Minitest::Test
  def normalize(source)
    tokens = Moof::Lexer.new(source).tokenize
    ast = Moof::Parser.new(tokens).parse_program
    Moof::Normalizer.new.call(ast)
  end

  def normalize_one(source)
    normalize(source).expressions.first
  end

  # ── Basic Lowering ─────────────────────────────────────

  def test_unary_message
    node = normalize_one("[obj method]")
    assert_instance_of Moof::AST::Call, node
    assert_equal "__send", node.callee.name
    assert_equal 2, node.arguments.length
    assert_instance_of Moof::AST::Identifier, node.arguments[0]
    assert_equal "obj", node.arguments[0].name
    assert_instance_of Moof::AST::StringLiteral, node.arguments[1]
    assert_equal "method", node.arguments[1].value
  end

  def test_positional_message
    node = normalize_one("[list at 0]")
    assert_instance_of Moof::AST::Call, node
    assert_equal "__send", node.callee.name
    assert_equal 3, node.arguments.length
    assert_equal "at", node.arguments[1].value
    assert_equal 0, node.arguments[2].value
  end

  def test_keyword_message
    node = normalize_one('[dict insertValue: 42 forKey: "x"]')
    assert_instance_of Moof::AST::Call, node
    assert_equal "__send", node.callee.name
    assert_equal 4, node.arguments.length
    assert_equal "insertValue:forKey:", node.arguments[1].value
    assert_equal 42, node.arguments[2].value
    assert_equal "x", node.arguments[3].value
  end

  # ── Nested Message Sends ───────────────────────────────

  def test_nested_message_in_message_arg
    node = normalize_one("[obj method: [other value]]")
    assert_instance_of Moof::AST::Call, node
    assert_equal "__send", node.callee.name
    # The inner [other value] should also be lowered
    inner = node.arguments[2]
    assert_instance_of Moof::AST::Call, inner
    assert_equal "__send", inner.callee.name
    assert_equal "value", inner.arguments[1].value
  end

  def test_message_inside_sexpr
    node = normalize_one("(if true [obj method] nil)")
    assert_instance_of Moof::AST::If, node
    then_branch = node.then_branch
    assert_instance_of Moof::AST::Call, then_branch
    assert_equal "__send", then_branch.callee.name
  end

  def test_message_inside_define
    node = normalize_one("(define x [obj method])")
    assert_instance_of Moof::AST::Define, node
    assert_instance_of Moof::AST::Call, node.value
    assert_equal "__send", node.value.callee.name
  end

  def test_message_inside_lambda
    node = normalize_one("(lambda (x) [x length])")
    assert_instance_of Moof::AST::Lambda, node
    body = node.body
    assert_instance_of Moof::AST::Call, body
    assert_equal "__send", body.callee.name
  end

  def test_message_inside_let
    node = normalize_one("(let ((x [obj val])) [x length])")
    assert_instance_of Moof::AST::Let, node
    # Binding value is lowered
    binding_val = node.bindings[0][1]
    assert_instance_of Moof::AST::Call, binding_val
    assert_equal "__send", binding_val.callee.name
    # Body is lowered
    assert_instance_of Moof::AST::Call, node.body
    assert_equal "__send", node.body.callee.name
  end

  def test_message_inside_do
    node = normalize_one("(do [a b] [c d])")
    assert_instance_of Moof::AST::Do, node
    node.expressions.each do |e|
      assert_instance_of Moof::AST::Call, e
      assert_equal "__send", e.callee.name
    end
  end

  def test_message_inside_set_bang
    node = normalize_one("(set! x [obj val])")
    assert_instance_of Moof::AST::SetBang, node
    assert_instance_of Moof::AST::Call, node.value
    assert_equal "__send", node.value.callee.name
  end

  def test_message_inside_try_catch
    node = normalize_one('(try [obj risky] (catch e [e message]))')
    assert_instance_of Moof::AST::TryCatch, node
    assert_instance_of Moof::AST::Call, node.body
    assert_equal "__send", node.body.callee.name
    assert_instance_of Moof::AST::Call, node.catch_body
    assert_equal "__send", node.catch_body.callee.name
  end

  def test_message_inside_cond
    node = normalize_one("(cond ([x positive?] [x abs]) (else 0))")
    assert_instance_of Moof::AST::Cond, node
    # First clause test and body should be lowered
    test_node = node.clauses[0][0]
    assert_instance_of Moof::AST::Call, test_node
    assert_equal "__send", test_node.callee.name
    body_node = node.clauses[0][1]
    assert_instance_of Moof::AST::Call, body_node
    assert_equal "__send", body_node.callee.name
  end

  def test_message_inside_map
    node = normalize_one("{len: [s length]}")
    assert_instance_of Moof::AST::MapLiteral, node
    val = node.pairs[0][1]
    assert_instance_of Moof::AST::Call, val
    assert_equal "__send", val.callee.name
  end

  # ── Quote is NOT normalized ────────────────────────────

  def test_quote_not_normalized
    node = normalize_one("'[obj method]")
    assert_instance_of Moof::AST::Quote, node
    # The inner expression should still be a MessageSend (quotes are data)
    assert_instance_of Moof::AST::MessageSend, node.expression
  end

  # ── Pass-through for non-message nodes ─────────────────

  def test_literals_pass_through
    node = normalize_one("42")
    assert_instance_of Moof::AST::IntegerLiteral, node
    assert_equal 42, node.value
  end

  def test_call_passes_through
    node = normalize_one("(+ 1 2)")
    assert_instance_of Moof::AST::Call, node
    assert_equal "+", node.callee.name
  end

  def test_define_function_body_normalized
    node = normalize_one("(define (f x) [x length])")
    assert_instance_of Moof::AST::DefineFunction, node
    assert_instance_of Moof::AST::Call, node.body
    assert_equal "__send", node.body.callee.name
  end
end
