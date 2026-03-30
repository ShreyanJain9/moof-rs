require "minitest/autorun"
require_relative "../lib/moof/errors"
require_relative "../lib/moof/token"
require_relative "../lib/moof/ast"
require_relative "../lib/moof/interpreter"

class InterpreterTest < Minitest::Test
  def setup
    @interp = Moof::Interpreter.new
  end

  def eval_program(*exprs)
    program = Moof::AST::Program.new(expressions: exprs)
    @interp.evaluate(program)
  end

  def eval_node(node, env = @interp.global_env)
    @interp.evaluate_node(node, env)
  end

  # ---- Literals ----

  def test_integer_literal
    assert_equal 42, eval_program(Moof::AST::IntegerLiteral.new(value: 42))
  end

  def test_float_literal
    assert_in_delta 3.14, eval_program(Moof::AST::FloatLiteral.new(value: 3.14))
  end

  def test_string_literal
    assert_equal "hello", eval_program(Moof::AST::StringLiteral.new(value: "hello"))
  end

  def test_bool_literal
    assert_equal true, eval_program(Moof::AST::BoolLiteral.new(value: true))
    assert_equal false, eval_program(Moof::AST::BoolLiteral.new(value: false))
  end

  def test_nil_literal
    assert_nil eval_program(Moof::AST::NilLiteral.new)
  end

  # ---- Arithmetic ----

  def test_addition
    # (+ 1 2) => 3
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "+"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 1),
        Moof::AST::IntegerLiteral.new(value: 2)
      ]
    )
    assert_equal 3, eval_program(node)
  end

  def test_variadic_addition
    # (+ 1 2 3 4) => 10
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "+"),
      arguments: [1, 2, 3, 4].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
    )
    assert_equal 10, eval_program(node)
  end

  def test_subtraction
    # (- 10 3) => 7
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "-"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 10),
        Moof::AST::IntegerLiteral.new(value: 3)
      ]
    )
    assert_equal 7, eval_program(node)
  end

  def test_unary_negation
    # (- 5) => -5
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "-"),
      arguments: [Moof::AST::IntegerLiteral.new(value: 5)]
    )
    assert_equal(-5, eval_program(node))
  end

  def test_multiplication
    # (* 3 4) => 12
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "*"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 3),
        Moof::AST::IntegerLiteral.new(value: 4)
      ]
    )
    assert_equal 12, eval_program(node)
  end

  def test_division
    # (/ 10 2) => 5
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "/"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 10),
        Moof::AST::IntegerLiteral.new(value: 2)
      ]
    )
    assert_equal 5, eval_program(node)
  end

  def test_comparison
    gt = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: ">"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 5),
        Moof::AST::IntegerLiteral.new(value: 3)
      ]
    )
    assert_equal true, eval_program(gt)
  end

  # ---- Define and Lookup ----

  def test_define_and_lookup
    define = Moof::AST::Define.new(
      name: "x",
      value: Moof::AST::IntegerLiteral.new(value: 42)
    )
    lookup = Moof::AST::Identifier.new(name: "x")
    assert_equal 42, eval_program(define, lookup)
  end

  def test_undefined_variable_raises
    assert_raises(Moof::NameError) do
      eval_program(Moof::AST::Identifier.new(name: "nonexistent"))
    end
  end

  # ---- Lambda and Closures ----

  def test_lambda_call
    # (define double (lambda (x) (* x 2)))
    # (double 5) => 10
    lam = Moof::AST::Lambda.new(
      params: ["x"],
      body: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "*"),
        arguments: [
          Moof::AST::Identifier.new(name: "x"),
          Moof::AST::IntegerLiteral.new(value: 2)
        ]
      )
    )
    define = Moof::AST::Define.new(name: "double", value: lam)
    call = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "double"),
      arguments: [Moof::AST::IntegerLiteral.new(value: 5)]
    )
    assert_equal 10, eval_program(define, call)
  end

  def test_closure_captures_env
    # (define make-adder (lambda (n) (lambda (x) (+ n x))))
    # (define add5 (make-adder 5))
    # (add5 3) => 8
    inner = Moof::AST::Lambda.new(
      params: ["x"],
      body: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "+"),
        arguments: [
          Moof::AST::Identifier.new(name: "n"),
          Moof::AST::Identifier.new(name: "x")
        ]
      )
    )
    outer = Moof::AST::Lambda.new(params: ["n"], body: inner)

    define_maker = Moof::AST::Define.new(name: "make-adder", value: outer)
    define_add5 = Moof::AST::Define.new(
      name: "add5",
      value: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "make-adder"),
        arguments: [Moof::AST::IntegerLiteral.new(value: 5)]
      )
    )
    call = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "add5"),
      arguments: [Moof::AST::IntegerLiteral.new(value: 3)]
    )
    assert_equal 8, eval_program(define_maker, define_add5, call)
  end

  def test_arity_error
    lam = Moof::AST::Lambda.new(
      params: ["x"],
      body: Moof::AST::Identifier.new(name: "x")
    )
    define = Moof::AST::Define.new(name: "f", value: lam)
    call = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "f"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 1),
        Moof::AST::IntegerLiteral.new(value: 2)
      ]
    )
    assert_raises(Moof::ArityError) do
      eval_program(define, call)
    end
  end

  # ---- DefineFunction ----

  def test_define_function
    # (define (square x) (* x x))
    # (square 6) => 36
    defn = Moof::AST::DefineFunction.new(
      name: "square",
      params: ["x"],
      body: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "*"),
        arguments: [
          Moof::AST::Identifier.new(name: "x"),
          Moof::AST::Identifier.new(name: "x")
        ]
      )
    )
    call = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "square"),
      arguments: [Moof::AST::IntegerLiteral.new(value: 6)]
    )
    assert_equal 36, eval_program(defn, call)
  end

  # ---- Let with immutable bindings ----

  def test_let_bindings
    # (let ((x 10) (y 20)) (+ x y)) => 30
    node = Moof::AST::Let.new(
      bindings: [
        ["x", Moof::AST::IntegerLiteral.new(value: 10)],
        ["y", Moof::AST::IntegerLiteral.new(value: 20)]
      ],
      body: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "+"),
        arguments: [
          Moof::AST::Identifier.new(name: "x"),
          Moof::AST::Identifier.new(name: "y")
        ]
      )
    )
    assert_equal 30, eval_program(node)
  end

  def test_let_immutable_raises_on_set
    # (let ((x 10)) (set! x 20)) => ImmutableBindingError
    node = Moof::AST::Let.new(
      bindings: [["x", Moof::AST::IntegerLiteral.new(value: 10)]],
      body: Moof::AST::SetBang.new(
        name: "x",
        value: Moof::AST::IntegerLiteral.new(value: 20)
      )
    )
    assert_raises(Moof::ImmutableBindingError) do
      eval_program(node)
    end
  end

  # ---- set! on mutable bindings ----

  def test_set_bang
    define = Moof::AST::Define.new(name: "x", value: Moof::AST::IntegerLiteral.new(value: 1))
    set = Moof::AST::SetBang.new(name: "x", value: Moof::AST::IntegerLiteral.new(value: 99))
    lookup = Moof::AST::Identifier.new(name: "x")
    assert_equal 99, eval_program(define, set, lookup)
  end

  # ---- If / Cond Branching ----

  def test_if_true_branch
    node = Moof::AST::If.new(
      condition: Moof::AST::BoolLiteral.new(value: true),
      then_branch: Moof::AST::IntegerLiteral.new(value: 1),
      else_branch: Moof::AST::IntegerLiteral.new(value: 2)
    )
    assert_equal 1, eval_program(node)
  end

  def test_if_false_branch
    node = Moof::AST::If.new(
      condition: Moof::AST::BoolLiteral.new(value: false),
      then_branch: Moof::AST::IntegerLiteral.new(value: 1),
      else_branch: Moof::AST::IntegerLiteral.new(value: 2)
    )
    assert_equal 2, eval_program(node)
  end

  def test_if_nil_is_falsy
    node = Moof::AST::If.new(
      condition: Moof::AST::NilLiteral.new,
      then_branch: Moof::AST::IntegerLiteral.new(value: 1),
      else_branch: Moof::AST::IntegerLiteral.new(value: 2)
    )
    assert_equal 2, eval_program(node)
  end

  def test_if_no_else
    node = Moof::AST::If.new(
      condition: Moof::AST::BoolLiteral.new(value: false),
      then_branch: Moof::AST::IntegerLiteral.new(value: 1),
      else_branch: nil
    )
    assert_nil eval_program(node)
  end

  def test_cond
    # (cond (false 1) (true 2) (else 3)) => 2
    node = Moof::AST::Cond.new(
      clauses: [
        [Moof::AST::BoolLiteral.new(value: false), Moof::AST::IntegerLiteral.new(value: 1)],
        [Moof::AST::BoolLiteral.new(value: true), Moof::AST::IntegerLiteral.new(value: 2)],
        [Moof::AST::Identifier.new(name: "else"), Moof::AST::IntegerLiteral.new(value: 3)]
      ]
    )
    assert_equal 2, eval_program(node)
  end

  def test_cond_else
    node = Moof::AST::Cond.new(
      clauses: [
        [Moof::AST::BoolLiteral.new(value: false), Moof::AST::IntegerLiteral.new(value: 1)],
        [Moof::AST::Identifier.new(name: "else"), Moof::AST::IntegerLiteral.new(value: 99)]
      ]
    )
    assert_equal 99, eval_program(node)
  end

  def test_cond_no_match
    node = Moof::AST::Cond.new(
      clauses: [
        [Moof::AST::BoolLiteral.new(value: false), Moof::AST::IntegerLiteral.new(value: 1)]
      ]
    )
    assert_nil eval_program(node)
  end

  # ---- Do sequencing ----

  def test_do_returns_last
    node = Moof::AST::Do.new(
      expressions: [
        Moof::AST::IntegerLiteral.new(value: 1),
        Moof::AST::IntegerLiteral.new(value: 2),
        Moof::AST::IntegerLiteral.new(value: 3)
      ]
    )
    assert_equal 3, eval_program(node)
  end

  def test_do_with_side_effects
    # (do (define x 10) (+ x 5)) => 15
    node = Moof::AST::Do.new(
      expressions: [
        Moof::AST::Define.new(name: "x", value: Moof::AST::IntegerLiteral.new(value: 10)),
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "+"),
          arguments: [
            Moof::AST::Identifier.new(name: "x"),
            Moof::AST::IntegerLiteral.new(value: 5)
          ]
        )
      ]
    )
    assert_equal 15, eval_program(node)
  end

  # ---- Quote ----

  def test_quote_identifier_produces_symbol
    node = Moof::AST::Quote.new(
      expression: Moof::AST::Identifier.new(name: "foo")
    )
    result = eval_program(node)
    assert_instance_of Moof::SymbolValue, result
    assert_equal "foo", result.name
  end

  def test_quote_literal_returns_value
    node = Moof::AST::Quote.new(
      expression: Moof::AST::IntegerLiteral.new(value: 42)
    )
    assert_equal 42, eval_program(node)
  end

  def test_quote_call_produces_array
    # '(+ 1 2) => [SymbolValue("+"), 1, 2]
    node = Moof::AST::Quote.new(
      expression: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "+"),
        arguments: [
          Moof::AST::IntegerLiteral.new(value: 1),
          Moof::AST::IntegerLiteral.new(value: 2)
        ]
      )
    )
    result = eval_program(node)
    assert_instance_of Array, result
    assert_equal 3, result.length
    assert_equal Moof::SymbolValue.new("+"), result[0]
    assert_equal 1, result[1]
    assert_equal 2, result[2]
  end

  # ---- try/catch ----

  def test_try_catch_no_error
    node = Moof::AST::TryCatch.new(
      body: Moof::AST::IntegerLiteral.new(value: 42),
      error_name: "e",
      catch_body: Moof::AST::IntegerLiteral.new(value: 0)
    )
    assert_equal 42, eval_program(node)
  end

  def test_try_catch_catches_runtime_error
    # try to access undefined var, catch the error
    node = Moof::AST::TryCatch.new(
      body: Moof::AST::Identifier.new(name: "undefined_var"),
      error_name: "e",
      catch_body: Moof::AST::Identifier.new(name: "e")
    )
    result = eval_program(node)
    assert_equal "Undefined variable: undefined_var", result
  end

  # ---- __send and message dispatch ----

  def test_send_number_abs
    # (__send -42 "abs") => 42
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "-"),
          arguments: [Moof::AST::IntegerLiteral.new(value: 42)]
        ),
        Moof::AST::StringLiteral.new(value: "abs")
      ]
    )
    assert_equal 42, eval_program(node)
  end

  def test_send_string_length
    # (__send "hello" "length") => 5
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::StringLiteral.new(value: "hello"),
        Moof::AST::StringLiteral.new(value: "length")
      ]
    )
    assert_equal 5, eval_program(node)
  end

  def test_send_string_uppercase
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::StringLiteral.new(value: "hello"),
        Moof::AST::StringLiteral.new(value: "uppercase")
      ]
    )
    assert_equal "HELLO", eval_program(node)
  end

  def test_send_string_replace_all_with
    # (__send "hello" "replaceAll:with:" "l" "r") => "herro"
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::StringLiteral.new(value: "hello"),
        Moof::AST::StringLiteral.new(value: "replaceAll:with:"),
        Moof::AST::StringLiteral.new(value: "l"),
        Moof::AST::StringLiteral.new(value: "r")
      ]
    )
    assert_equal "herro", eval_program(node)
  end

  def test_send_list_length
    # (define xs (list 1 2 3))
    # (__send xs "length") => 3
    define = Moof::AST::Define.new(
      name: "xs",
      value: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "list"),
        arguments: [1, 2, 3].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
      )
    )
    send_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Identifier.new(name: "xs"),
        Moof::AST::StringLiteral.new(value: "length")
      ]
    )
    assert_equal 3, eval_program(define, send_node)
  end

  def test_send_list_map
    # (define xs (list 1 2 3))
    # (__send xs "map:" (lambda (x) (* x 2))) => [2, 4, 6]
    define = Moof::AST::Define.new(
      name: "xs",
      value: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "list"),
        arguments: [1, 2, 3].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
      )
    )
    mapper = Moof::AST::Lambda.new(
      params: ["x"],
      body: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "*"),
        arguments: [
          Moof::AST::Identifier.new(name: "x"),
          Moof::AST::IntegerLiteral.new(value: 2)
        ]
      )
    )
    send_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Identifier.new(name: "xs"),
        Moof::AST::StringLiteral.new(value: "map:"),
        mapper
      ]
    )
    assert_equal [2, 4, 6], eval_program(define, send_node)
  end

  def test_send_list_filter
    define = Moof::AST::Define.new(
      name: "xs",
      value: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "list"),
        arguments: [1, 2, 3, 4, 5].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
      )
    )
    pred = Moof::AST::Lambda.new(
      params: ["x"],
      body: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: ">"),
        arguments: [
          Moof::AST::Identifier.new(name: "x"),
          Moof::AST::IntegerLiteral.new(value: 3)
        ]
      )
    )
    send_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Identifier.new(name: "xs"),
        Moof::AST::StringLiteral.new(value: "filter:"),
        pred
      ]
    )
    assert_equal [4, 5], eval_program(define, send_node)
  end

  def test_send_hash_at
    # {name: "moof"} -> at: "name" => "moof"
    map_lit = Moof::AST::MapLiteral.new(
      pairs: [["name", Moof::AST::StringLiteral.new(value: "moof")]]
    )
    define = Moof::AST::Define.new(name: "m", value: map_lit)
    send_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Identifier.new(name: "m"),
        Moof::AST::StringLiteral.new(value: "at:"),
        Moof::AST::StringLiteral.new(value: "name")
      ]
    )
    assert_equal "moof", eval_program(define, send_node)
  end

  def test_send_hash_keys
    map_lit = Moof::AST::MapLiteral.new(
      pairs: [
        ["x", Moof::AST::IntegerLiteral.new(value: 1)],
        ["y", Moof::AST::IntegerLiteral.new(value: 2)]
      ]
    )
    define = Moof::AST::Define.new(name: "m", value: map_lit)
    send_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Identifier.new(name: "m"),
        Moof::AST::StringLiteral.new(value: "keys")
      ]
    )
    assert_equal ["x", "y"], eval_program(define, send_node)
  end

  def test_send_boolean_not
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::BoolLiteral.new(value: true),
        Moof::AST::StringLiteral.new(value: "not")
      ]
    )
    assert_equal false, eval_program(node)
  end

  def test_send_nil_nil?
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::NilLiteral.new,
        Moof::AST::StringLiteral.new(value: "nil?")
      ]
    )
    assert_equal true, eval_program(node)
  end

  def test_send_number_nil_is_false
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 42),
        Moof::AST::StringLiteral.new(value: "nil?")
      ]
    )
    assert_equal false, eval_program(node)
  end

  def test_send_unknown_selector_raises
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 42),
        Moof::AST::StringLiteral.new(value: "nonexistent")
      ]
    )
    assert_raises(Moof::MessageError) do
      eval_program(node)
    end
  end

  # ---- Map literals ----

  def test_map_literal
    node = Moof::AST::MapLiteral.new(
      pairs: [
        ["name", Moof::AST::StringLiteral.new(value: "moof")],
        ["version", Moof::AST::IntegerLiteral.new(value: 1)]
      ]
    )
    result = eval_program(node)
    assert_instance_of Hash, result
    assert_equal "moof", result["name"]
    assert_equal 1, result["version"]
  end

  # ---- Nested calls ----

  def test_nested_calls
    # (+ (* 2 3) (- 10 4)) => 12
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "+"),
      arguments: [
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "*"),
          arguments: [
            Moof::AST::IntegerLiteral.new(value: 2),
            Moof::AST::IntegerLiteral.new(value: 3)
          ]
        ),
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "-"),
          arguments: [
            Moof::AST::IntegerLiteral.new(value: 10),
            Moof::AST::IntegerLiteral.new(value: 4)
          ]
        )
      ]
    )
    assert_equal 12, eval_program(node)
  end

  # ---- Builtins: equality ----

  def test_structural_equality
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "="),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 42),
        Moof::AST::IntegerLiteral.new(value: 42)
      ]
    )
    assert_equal true, eval_program(node)
  end

  # ---- Builtins: list ops ----

  def test_list_and_car_cdr
    # (car (list 1 2 3)) => 1
    car_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "car"),
      arguments: [
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "list"),
          arguments: [1, 2, 3].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
        )
      ]
    )
    assert_equal 1, eval_program(car_node)

    # (cdr (list 1 2 3)) => [2, 3]
    cdr_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "cdr"),
      arguments: [
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "list"),
          arguments: [1, 2, 3].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
        )
      ]
    )
    assert_equal [2, 3], eval_program(cdr_node)
  end

  def test_cons
    # (cons 0 (list 1 2)) => [0, 1, 2]
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "cons"),
      arguments: [
        Moof::AST::IntegerLiteral.new(value: 0),
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "list"),
          arguments: [1, 2].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
        )
      ]
    )
    assert_equal [0, 1, 2], eval_program(node)
  end

  # ---- Builtins: type checks ----

  def test_type_checks
    assert_equal true, eval_program(
      Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "number?"),
        arguments: [Moof::AST::IntegerLiteral.new(value: 42)]
      )
    )
    assert_equal true, eval_program(
      Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "string?"),
        arguments: [Moof::AST::StringLiteral.new(value: "hi")]
      )
    )
    assert_equal false, eval_program(
      Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "string?"),
        arguments: [Moof::AST::IntegerLiteral.new(value: 1)]
      )
    )
    assert_equal true, eval_program(
      Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "nil?"),
        arguments: [Moof::AST::NilLiteral.new]
      )
    )
  end

  # ---- Builtins: format ----

  def test_format
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "format"),
      arguments: [
        Moof::AST::StringLiteral.new(value: "hello ~a, you are ~a"),
        Moof::AST::StringLiteral.new(value: "world"),
        Moof::AST::IntegerLiteral.new(value: 42)
      ]
    )
    assert_equal "hello world, you are 42", eval_program(node)
  end

  # ---- Builtins: apply ----

  def test_apply
    # (apply + (list 1 2 3)) => 6
    node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "apply"),
      arguments: [
        Moof::AST::Identifier.new(name: "+"),
        Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "list"),
          arguments: [1, 2, 3].map { |n| Moof::AST::IntegerLiteral.new(value: n) }
        )
      ]
    )
    assert_equal 6, eval_program(node)
  end

  # ---- Function call: via dispatcher ----

  def test_send_function_call
    # Define a function, then use __send to call it via "call:" message
    lam = Moof::AST::Lambda.new(
      params: ["x"],
      body: Moof::AST::Call.new(
        callee: Moof::AST::Identifier.new(name: "*"),
        arguments: [
          Moof::AST::Identifier.new(name: "x"),
          Moof::AST::Identifier.new(name: "x")
        ]
      )
    )
    define = Moof::AST::Define.new(name: "sq", value: lam)
    send_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Identifier.new(name: "sq"),
        Moof::AST::StringLiteral.new(value: "call:"),
        Moof::AST::IntegerLiteral.new(value: 7)
      ]
    )
    assert_equal 49, eval_program(define, send_node)
  end

  # ---- Hash put:value: returns new hash ----

  def test_hash_put_value
    map_lit = Moof::AST::MapLiteral.new(
      pairs: [["x", Moof::AST::IntegerLiteral.new(value: 1)]]
    )
    define = Moof::AST::Define.new(name: "m", value: map_lit)
    send_node = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "__send"),
      arguments: [
        Moof::AST::Identifier.new(name: "m"),
        Moof::AST::StringLiteral.new(value: "put:value:"),
        Moof::AST::StringLiteral.new(value: "y"),
        Moof::AST::IntegerLiteral.new(value: 2)
      ]
    )
    result = eval_program(define, send_node)
    assert_equal({"x" => 1, "y" => 2}, result)
  end

  # ---- Environment introspection ----

  def test_environment_bindings
    env = Moof::Environment.new
    env.define("x", 1)
    env.define("y", 2)
    assert_equal({"x" => 1, "y" => 2}, env.bindings)
  end

  def test_environment_child_scope
    parent = Moof::Environment.new
    parent.define("x", 10)
    child = parent.child
    child.define("y", 20)
    assert_equal 10, child.get("x")
    assert_equal 20, child.get("y")
    assert_raises(Moof::NameError) { parent.get("y") }
  end

  # ---- SymbolValue ----

  def test_symbol_value_equality
    a = Moof::SymbolValue.new("foo")
    b = Moof::SymbolValue.new("foo")
    c = Moof::SymbolValue.new("bar")
    assert_equal a, b
    refute_equal a, c
  end

  def test_symbol_value_to_s
    s = Moof::SymbolValue.new("hello")
    assert_equal "'hello", s.to_s
  end

  # ---- Number messages ----

  def test_number_arithmetic_messages
    dispatcher = Moof::Dispatcher.new
    assert_equal 7, dispatcher.send_message(3, "+", [4], interpreter: nil)
    assert_equal true, dispatcher.send_message(5, ">", [3], interpreter: nil)
    assert_equal true, dispatcher.send_message(0, "zero?", [], interpreter: nil)
    assert_equal false, dispatcher.send_message(5, "negative?", [], interpreter: nil)
  end

  # ---- String messages ----

  def test_string_messages
    d = Moof::Dispatcher.new
    assert_equal "h", d.send_message("hello", "at:", [0], interpreter: nil)
    assert_equal true, d.send_message("hello", "contains:", ["ell"], interpreter: nil)
    assert_equal true, d.send_message("hello", "startsWith:", ["hel"], interpreter: nil)
    assert_equal ["a", "b", "c"], d.send_message("a,b,c", "split:", [","], interpreter: nil)
  end

  # ---- Array messages ----

  def test_array_messages
    d = Moof::Dispatcher.new
    assert_equal 1, d.send_message([1, 2, 3], "first", [], interpreter: nil)
    assert_equal 3, d.send_message([1, 2, 3], "last", [], interpreter: nil)
    assert_equal [3, 2, 1], d.send_message([1, 2, 3], "reverse", [], interpreter: nil)
    assert_equal [1, 2, 3, 4], d.send_message([1, 2, 3], "push:", [4], interpreter: nil)
    assert_equal true, d.send_message([1, 2, 3], "contains:", [2], interpreter: nil)
    assert_equal "1-2-3", d.send_message([1, 2, 3], "join:", ["-"], interpreter: nil)
  end

  # ---- Program evaluates multiple expressions, returns last ----

  def test_program_returns_last
    program = Moof::AST::Program.new(
      expressions: [
        Moof::AST::IntegerLiteral.new(value: 1),
        Moof::AST::IntegerLiteral.new(value: 2),
        Moof::AST::IntegerLiteral.new(value: 3)
      ]
    )
    assert_equal 3, @interp.evaluate(program)
  end

  # ---- Calling non-function raises ----

  def test_call_non_function_raises
    node = Moof::AST::Call.new(
      callee: Moof::AST::IntegerLiteral.new(value: 42),
      arguments: []
    )
    assert_raises(Moof::RuntimeError) do
      eval_program(node)
    end
  end

  # ---- Recursive function ----

  def test_recursive_function
    # (define (fact n) (if (= n 0) 1 (* n (fact (- n 1)))))
    # (fact 5) => 120
    defn = Moof::AST::DefineFunction.new(
      name: "fact",
      params: ["n"],
      body: Moof::AST::If.new(
        condition: Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "="),
          arguments: [
            Moof::AST::Identifier.new(name: "n"),
            Moof::AST::IntegerLiteral.new(value: 0)
          ]
        ),
        then_branch: Moof::AST::IntegerLiteral.new(value: 1),
        else_branch: Moof::AST::Call.new(
          callee: Moof::AST::Identifier.new(name: "*"),
          arguments: [
            Moof::AST::Identifier.new(name: "n"),
            Moof::AST::Call.new(
              callee: Moof::AST::Identifier.new(name: "fact"),
              arguments: [
                Moof::AST::Call.new(
                  callee: Moof::AST::Identifier.new(name: "-"),
                  arguments: [
                    Moof::AST::Identifier.new(name: "n"),
                    Moof::AST::IntegerLiteral.new(value: 1)
                  ]
                )
              ]
            )
          ]
        )
      )
    )
    call = Moof::AST::Call.new(
      callee: Moof::AST::Identifier.new(name: "fact"),
      arguments: [Moof::AST::IntegerLiteral.new(value: 5)]
    )
    assert_equal 120, eval_program(defn, call)
  end
end
