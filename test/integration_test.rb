require_relative "test_helper"

class IntegrationTest < Minitest::Test
  # Helper: evaluate source through the full pipeline, returning the last value.
  def eval_moof(source)
    Moof.evaluate(source)
  end

  # Helper: evaluate multiple expressions sharing one interpreter (for state).
  def eval_moof_sequence(*sources)
    interp = Moof::Interpreter.new
    result = nil
    sources.each do |src|
      tokens     = Moof::Lexer.new(src, filename: "(test)").tokenize
      program    = Moof::Parser.new(tokens).parse_program
      normalized = Moof::Normalizer.new.call(program)
      result     = interp.evaluate(normalized)
    end
    result
  end

  # ── Arithmetic ────────────────────────────────────────────

  def test_addition
    assert_equal 3, eval_moof("(+ 1 2)")
  end

  def test_subtraction
    assert_equal 7, eval_moof("(- 10 3)")
  end

  def test_multiplication
    assert_equal 20, eval_moof("(* 4 5)")
  end

  def test_division
    assert_equal 3, eval_moof("(/ 9 3)")
  end

  def test_nested_arithmetic
    assert_equal 14, eval_moof("(+ (* 2 3) (+ 4 4))")
  end

  # ── Literals ──────────────────────────────────────────────

  def test_integer_literal
    assert_equal 42, eval_moof("42")
  end

  def test_float_literal
    assert_in_delta 3.14, eval_moof("3.14"), 0.001
  end

  def test_string_literal
    assert_equal "hello", eval_moof('"hello"')
  end

  def test_bool_literals
    assert_equal true, eval_moof("true")
    assert_equal false, eval_moof("false")
  end

  def test_nil_literal
    assert_nil eval_moof("nil")
  end

  # ── Define & Variables ────────────────────────────────────

  def test_define_and_lookup
    result = eval_moof_sequence("(define x 42)", "x")
    assert_equal 42, result
  end

  def test_define_function_shorthand
    result = eval_moof("(do (define (square x) (* x x)) (square 5))")
    assert_equal 25, result
  end

  # ── Lambda ────────────────────────────────────────────────

  def test_lambda_immediate_call
    assert_equal 25, eval_moof("((lambda (x) (* x x)) 5)")
  end

  def test_lambda_as_value
    result = eval_moof("(do (define f (lambda (a b) (+ a b))) (f 3 4))")
    assert_equal 7, result
  end

  # ── Conditionals ──────────────────────────────────────────

  def test_if_true_branch
    assert_equal "yes", eval_moof('(if true "yes" "no")')
  end

  def test_if_false_branch
    assert_equal "no", eval_moof('(if false "yes" "no")')
  end

  def test_if_with_comparison
    assert_equal "big", eval_moof('(if (> 10 5) "big" "small")')
  end

  # ── Let Bindings ──────────────────────────────────────────

  def test_let
    assert_equal 3, eval_moof("(let ((x 1) (y 2)) (+ x y))")
  end

  # ── Do Block ──────────────────────────────────────────────

  def test_do_returns_last
    assert_equal 3, eval_moof("(do 1 2 3)")
  end

  # ── set! ──────────────────────────────────────────────────

  def test_set_bang
    result = eval_moof("(do (define x 1) (set! x 42) x)")
    assert_equal 42, result
  end

  # ── Map Literal ───────────────────────────────────────────

  def test_map_literal
    result = eval_moof("{x: 1 y: 2}")
    assert_instance_of Hash, result
    assert_equal 1, result["x"]
    assert_equal 2, result["y"]
  end

  # ── Message Passing ───────────────────────────────────────

  def test_string_length_message
    assert_equal 5, eval_moof('["hello" length]')
  end

  def test_string_uppercase_message
    assert_equal "HELLO", eval_moof('["hello" uppercase]')
  end

  def test_number_abs_message
    assert_equal 7, eval_moof("[-7 abs]")
    assert_equal 42, eval_moof("[42 abs]")
  end

  def test_string_replace_all_message
    assert_equal "herro", eval_moof('["hello" replaceAll: "l" with: "r"]')
  end

  def test_list_length_message
    assert_equal 3, eval_moof("[(list 1 2 3) length]")
  end

  def test_list_reverse_message
    assert_equal [3, 2, 1], eval_moof("[(list 1 2 3) reverse]")
  end

  def test_list_first_message
    assert_equal 1, eval_moof("[(list 1 2 3) first]")
  end

  # ── Try/Catch ─────────────────────────────────────────────

  def test_try_catch_no_error
    assert_equal 42, eval_moof("(try 42 (catch e 0))")
  end

  def test_try_catch_with_error
    # Use an undefined variable to trigger a Moof::NameError (which is a RuntimeError)
    result = eval_moof('(try undefined_var (catch e "caught"))')
    assert_equal "caught", result
  end

  # ── Error Cases ───────────────────────────────────────────

  def test_undefined_variable_raises
    assert_raises(Moof::NameError) do
      eval_moof("undefined_var")
    end
  end

  def test_message_not_understood_raises
    assert_raises(Moof::MessageError) do
      eval_moof("[42 nonExistentMethod]")
    end
  end

  # ── Comparison / Equality ─────────────────────────────────

  def test_equality
    assert_equal true, eval_moof("(= 1 1)")
    assert_equal false, eval_moof("(= 1 2)")
  end

  def test_greater_than
    assert_equal true, eval_moof("(> 3 2)")
    assert_equal false, eval_moof("(> 2 3)")
  end

  def test_less_than
    assert_equal true, eval_moof("(< 2 3)")
    assert_equal false, eval_moof("(< 3 2)")
  end

  # ── Quote ─────────────────────────────────────────────────

  def test_quote
    result = eval_moof("'(1 2 3)")
    assert_instance_of Array, result
    assert_equal [1, 2, 3], result
  end

  # ── Cond ──────────────────────────────────────────────────

  def test_cond
    result = eval_moof('(cond ((= 1 2) "a") ((= 1 1) "b") (else "c"))')
    assert_equal "b", result
  end

  # ── Recursive Functions ───────────────────────────────────

  def test_fibonacci
    src = <<~MOOF
      (do
        (define (fib n)
          (if (< n 2)
            n
            (+ (fib (- n 1)) (fib (- n 2)))))
        (fib 10))
    MOOF
    assert_equal 55, eval_moof(src)
  end

  # ── Printer ──────────────────────────────────────────────

  def test_printer_integer
    assert_equal "42", Moof::Printer.format(42)
  end

  def test_printer_string
    assert_equal '"hello"', Moof::Printer.format("hello")
  end

  def test_printer_bool
    assert_equal "true", Moof::Printer.format(true)
    assert_equal "false", Moof::Printer.format(false)
  end

  def test_printer_nil
    assert_equal "nil", Moof::Printer.format(nil)
  end

  def test_printer_list
    assert_equal "(1 2 3)", Moof::Printer.format([1, 2, 3])
  end

  def test_printer_map
    result = Moof::Printer.format({"x" => 1})
    assert_equal "{x: 1}", result
  end

  def test_printer_symbol_value
    assert_equal "'foo", Moof::Printer.format(Moof::SymbolValue.new("foo"))
  end
end
