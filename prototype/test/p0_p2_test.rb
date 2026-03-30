require_relative "test_helper"

# Tests for P0-P2 improvements:
#   P0: source locations, short-circuit and/or
#   P1: variadic functions, map/filter/reduce builtins, REPL polish verifiable pieces
#   P2: class definitions, inheritance, traits, introspection

class P0ShortCircuitTest < Minitest::Test
  def eval_moof(src)
    Moof.evaluate(src)
  end

  # and/or short-circuit
  def test_and_returns_false_without_evaluating_right
    # if and were eager, the right side (invalid) would be evaluated
    result = eval_moof('(and false true)')
    assert_equal false, result
  end

  def test_and_returns_right_side_if_truthy
    assert_equal 42, eval_moof('(and true 42)')
  end

  def test_or_short_circuits_on_truthy_left
    result = eval_moof('(or 42 false)')
    assert_equal 42, result
  end

  def test_or_evaluates_right_when_left_is_falsy
    assert_equal "yes", eval_moof('(or false "yes")')
  end

  def test_and_nil_is_falsy
    assert_equal false, eval_moof('(and nil 42)')
  end

  def test_nested_and_or
    assert_equal true, eval_moof('(or (and false true) (and true true))')
  end

  # Source location in errors
  def test_name_error_includes_location
    err = assert_raises(Moof::NameError) { eval_moof("undefined_var") }
    # message should contain something useful
    assert_includes err.message, "undefined_var"
  end
end

class P1VariadicTest < Minitest::Test
  def eval_moof(src)
    Moof.evaluate(src)
  end

  def test_variadic_lambda_collects_rest
    result = eval_moof('((lambda (a . rest) rest) 1 2 3)')
    assert_equal [2, 3], result
  end

  def test_variadic_lambda_empty_rest
    result = eval_moof('((lambda (a . rest) rest) 1)')
    assert_equal [], result
  end

  def test_variadic_define_function
    result = eval_moof('(do (define (sum . args) (reduce + 0 args)) (sum 1 2 3 4 5))')
    assert_equal 15, result
  end

  def test_variadic_min_args_enforced
    assert_raises(Moof::ArityError) do
      eval_moof('((lambda (a b . rest) a))')
    end
  end
end

class P1BuiltinsTest < Minitest::Test
  def eval_moof(src)
    Moof.evaluate(src)
  end

  def test_map_builtin
    result = eval_moof('(map (lambda (x) (* x x)) (list 1 2 3))')
    assert_equal [1, 4, 9], result
  end

  def test_filter_builtin
    result = eval_moof('(filter (lambda (x) (> x 2)) (list 1 2 3 4))')
    assert_equal [3, 4], result
  end

  def test_reduce_builtin
    result = eval_moof('(reduce + 0 (list 1 2 3 4 5))')
    assert_equal 15, result
  end

  def test_range_one_arg
    result = eval_moof('(range 5)')
    assert_equal [0, 1, 2, 3, 4], result
  end

  def test_range_two_args
    result = eval_moof('(range 2 6)')
    assert_equal [2, 3, 4, 5], result
  end

  def test_zip_builtin
    result = eval_moof('(zip (list 1 2 3) (list "a" "b" "c"))')
    assert_equal [[1, "a"], [2, "b"], [3, "c"]], result
  end

  def test_flatten_builtin
    result = eval_moof('(flatten (list (list 1 2) (list 3 (list 4 5))))')
    assert_equal [1, 2, 3, 4, 5], result
  end

  def test_take_builtin
    result = eval_moof('(take 3 (list 1 2 3 4 5))')
    assert_equal [1, 2, 3], result
  end

  def test_drop_builtin
    result = eval_moof('(drop 2 (list 1 2 3 4 5))')
    assert_equal [3, 4, 5], result
  end

  def test_for_each_builtin
    result = eval_moof('(do (define sum 0) (for-each (lambda (x) (set! sum (+ sum x))) (list 1 2 3)) sum)')
    assert_equal 6, result
  end

  def test_not_builtin
    assert_equal true,  eval_moof('(not false)')
    assert_equal false, eval_moof('(not true)')
    assert_equal true,  eval_moof('(not nil)')
  end
end

class P2ClassSystemTest < Minitest::Test
  def eval_moof(src)
    Moof.evaluate(src)
  end

  def test_class_definition_and_instantiation
    src = '(do (class Point (fields x y)) (Point 3 4))'
    result = eval_moof(src)
    assert_instance_of Moof::MoofObject, result
    assert_equal "Point", result.klass.name
  end

  def test_field_access_via_message
    src = '(do (class Point (fields x y)) (define p (Point 3 4)) [p x])'
    assert_equal 3, eval_moof(src)
  end

  def test_method_definition_and_dispatch
    src = <<~MOOF
      (do
        (class Point
          (fields x y)
          (method sum [] (+ [self x] [self y])))
        (define p (Point 10 5))
        [p sum])
    MOOF
    assert_equal 15, eval_moof(src)
  end

  def test_method_with_argument
    src = <<~MOOF
      (do
        (class Point
          (fields x y)
          (method scale: [factor]
            (Point (* [self x] factor) (* [self y] factor))))
        (define p (Point 2 3))
        (define q [p scale: 4])
        [q x])
    MOOF
    assert_equal 8, eval_moof(src)
  end

  def test_class_introspection_class_message
    src = '(do (class Dog (fields name)) (define d (Dog "Rex")) [d className])'
    assert_equal "Dog", eval_moof(src)
  end

  def test_class_introspection_responds_to
    src = <<~MOOF
      (do
        (class Cat (fields name) (method speak [] "meow"))
        (define c (Cat "Kitty"))
        [c respondsTo: "speak"])
    MOOF
    assert_equal true, eval_moof(src)
  end

  def test_class_introspection_responds_to_false
    src = <<~MOOF
      (do
        (class Cat (fields name) (method speak [] "meow"))
        (define c (Cat "Kitty"))
        [c respondsTo: "bark"])
    MOOF
    assert_equal false, eval_moof(src)
  end
end

class P2InheritanceTest < Minitest::Test
  def eval_moof(src)
    Moof.evaluate(src)
  end

  def test_inheritance_inherits_fields
    src = <<~MOOF
      (do
        (class Animal (fields name))
        (class Dog (extends Animal) (fields breed))
        (define d (Dog "Rex" "Labrador"))
        [d name])
    MOOF
    assert_equal "Rex", eval_moof(src)
  end

  def test_inheritance_inherits_methods
    src = <<~MOOF
      (do
        (class Animal (fields name) (method speak [] "..."))
        (class Cat (extends Animal))
        (define c (Cat "Whiskers"))
        [c speak])
    MOOF
    assert_equal "...", eval_moof(src)
  end

  def test_inheritance_overrides_methods
    src = <<~MOOF
      (do
        (class Animal (fields name) (method speak [] "..."))
        (class Dog (extends Animal) (method speak [] "woof"))
        (define d (Dog "Rex"))
        [d speak])
    MOOF
    assert_equal "woof", eval_moof(src)
  end
end

class P2TraitTest < Minitest::Test
  def eval_moof(src)
    Moof.evaluate(src)
  end

  def test_trait_methods_applied
    src = <<~MOOF
      (do
        (trait Printable
          (method describe [] "I am printable"))
        (class Widget (uses Printable) (fields name))
        (define w (Widget "button"))
        [w describe])
    MOOF
    assert_equal "I am printable", eval_moof(src)
  end

  def test_class_overrides_trait_method
    src = <<~MOOF
      (do
        (trait Printable
          (method describe [] "from trait"))
        (class Widget (uses Printable) (fields name)
          (method describe [] "from class"))
        (define w (Widget "button"))
        [w describe])
    MOOF
    assert_equal "from class", eval_moof(src)
  end
end

class P2StdlibTest < Minitest::Test
  def eval_moof(src)
    Moof.evaluate(src)
  end

  def test_stdlib_square
    assert_equal 25, eval_moof('(square 5)')
  end

  def test_stdlib_abs
    assert_equal 7, eval_moof('(abs -7)')
    assert_equal 3, eval_moof('(abs 3)')
  end

  def test_stdlib_max
    assert_equal 7, eval_moof('(max 3 7)')
  end

  def test_stdlib_min
    assert_equal 3, eval_moof('(min 3 7)')
  end

  def test_stdlib_even_odd
    assert_equal true,  eval_moof('(even? 4)')
    assert_equal false, eval_moof('(even? 3)')
    assert_equal true,  eval_moof('(odd? 7)')
  end

  def test_stdlib_gcd
    assert_equal 6, eval_moof('(gcd 42 18)')
  end

  def test_stdlib_factorial
    assert_equal 120, eval_moof('(factorial 5)')
  end

  def test_stdlib_sum
    assert_equal 15, eval_moof('(sum (list 1 2 3 4 5))')
  end

  def test_stdlib_any
    assert_equal true,  eval_moof('(any? even? (list 1 2 3))')
    assert_equal false, eval_moof('(any? even? (list 1 3 5))')
  end

  def test_stdlib_all
    assert_equal true,  eval_moof('(all? even? (list 2 4 6))')
    assert_equal false, eval_moof('(all? even? (list 2 3 6))')
  end

  def test_stdlib_find
    result = eval_moof('(find even? (list 1 3 4 5))')
    assert_equal 4, result
  end

  def test_stdlib_identity
    assert_equal 42, eval_moof('(identity 42)')
  end

  def test_stdlib_compose
    result = eval_moof('(do (define inc (lambda (x) (+ x 1))) (define double (lambda (x) (* x 2))) ((compose inc double) 5))')
    assert_equal 11, result
  end

  def test_stdlib_complement
    result = eval_moof('(filter (complement even?) (list 1 2 3 4 5))')
    assert_equal [1, 3, 5], result
  end

  def test_stdlib_flip
    result = eval_moof('((flip -) 3 10)')
    assert_equal 7, result
  end

  def test_stdlib_pi
    pi = eval_moof('pi')
    assert_in_delta 3.14159, pi, 0.001
  end

  def test_stdlib_string_join
    result = eval_moof('(string-join ", " (list "a" "b" "c"))')
    assert_equal "a, b, c", result
  end

  def test_stdlib_reverse
    result = eval_moof('(reverse (list 1 2 3))')
    assert_equal [3, 2, 1], result
  end
end
