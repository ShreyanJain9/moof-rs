# Moof

A programming language that combines Lisp s-expressions `()` with Smalltalk-style message passing `[]`. Both syntaxes nest freely within each other.

```moof
; Two syntaxes, one language
(define greeting "Hello, Moof!")
(print [greeting uppercase])          ; message passing
(print (length greeting))             ; function call

; Numbers never overflow
(* 999999999999999999 999999999999999999)
; => 999999999999999998000000000000000001

; Pipelines mix both styles
(-> (range 1 20)
  [filter: { |n| [n odd?] }]
  [map: { |n| (* n n) }]
  [take: 5])                          ; => (1 9 25 49 81)
```

## Installation

Requires Rust 2024 edition (1.85+).

```bash
cargo build --release
# Binary at target/release/moof
```

## Usage

```bash
moof                        # start the REPL
moof file.moof              # run a file
moof -e '(+ 1 2)'          # evaluate an expression
```

## Language Overview

### Basics

```moof
(define x 42)                         ; bind a value
(define (square n) (* n n))           ; define a function
(let ((a 1) (b 2)) (+ a b))          ; local bindings
(if (> x 0) "positive" "non-positive")
(lambda (x y) (+ x y))               ; anonymous function
```

### Message Passing

Square brackets send messages to objects, Smalltalk-style. Keyword messages use colons.

```moof
[greeting length]                     ; unary message
[greeting slice: 0 5]                 ; keyword message
["hello" uppercase]                   ; => "HELLO"
[(list 3 1 2) sort]                   ; => (1 2 3)
```

### Classes and Objects

Open classes with single inheritance. Built-in types are extensible.

```moof
(class Point (fields x y)
  (method dist () (sqrt (+ (* x x) (* y y))))
  (method to_s () (format "(~a, ~a)" x y)))

(define p (Point 3 4))
(print [p dist])                      ; => 5
(print [p to_s])                      ; => "(3, 4)"

; Reopen any class, even built-in ones
(extend Integer
  (method even? () (= 0 (% self 2))))
[42 even?]                            ; => true
```

### Pattern Matching

Match on ADTs, cons lists, and tables.

```moof
(type Shape (Circle radius) (Rect w h))

(define (area shape)
  (match shape
    ((Circle r) (* 3.14159 (* r r)))
    ((Rect w h) (* w h))))

(area (Circle 5))                     ; => 78.53975
(area (Rect 3 4))                     ; => 12
```

### Pipelines

Thread a value through a chain of transformations.

```moof
(-> 42 [to_s] [length])              ; => 2

(-> (range 1 100)
  [filter: { |n| [n even?] }]
  [map: { |n| (* n n) }]
  [take: 3])                          ; => (4 16 36)
```

### Macros

Code is data. Macros transform the AST at expansion time.

```moof
(defmacro unless (cond body)
  `(if (not ,cond) ,body nil))

(unless false (print "hello"))        ; => hello
```

### Modules

```moof
(module Math
  (export pi tau)
  (define pi 3.14159)
  (define tau (* 2 pi)))

(import Math)
(print pi)                            ; => 3.14159
```

### Protocols

Behavioral contracts with optional default methods.

```moof
(protocol Printable
  (selector to_s))

(implement Printable for Point)       ; Point already has to_s
```

## Built-in Types

| Type | Example | Notes |
|------|---------|-------|
| Integer | `42`, `0xFF` | Arbitrary precision |
| Float | `3.14` | 64-bit IEEE 754 |
| String | `"hello"`, `$"x = \(x)"` | Interpolation with `$"..."` |
| Symbol | `'foo` | Interned identifiers |
| Bool | `true`, `false` | |
| Nil | `nil` | |
| Cons | `(cons 1 (cons 2 nil))` | Linked list cells |
| List | `(list 1 2 3)` | Built from cons cells |
| Table | `{name: "moof" version: 1}` | Ordered key-value (like Lua) |
| Closure | `(lambda (x) x)`, `{ \|x\| x }` | Unified function type |
| Range | `(range 1 10)` | Lazy integer ranges |
| Error | `(error "oops")` | |

## Examples

See [`prototype/examples/`](prototype/examples/) for runnable programs:

- `hello.moof` -- hello world
- `fibonacci.moof` -- recursive and iterative fibonacci
- `messages.moof` -- message passing and method calls
- `pattern_matching.moof` -- ADTs and match expressions
- `pipeline.moof` -- pipeline operator
- `adts.moof` -- algebraic data types

## Project Structure

```
src/                  Rust implementation
  lexer.rs            Tokenizer
  parser.rs           Recursive descent parser
  normalizer.rs       AST lowering pass
  interpreter.rs      Tree-walking evaluator with TCO
  methods.rs          Unified message dispatch
  builtin_methods.rs  Built-in type methods
  builtins.rs         Primitive functions
stdlib/
  stdlib.moof         Self-hosting standard library (written in Moof)
prototype/            Original Ruby prototype (reference only)
```

## License

MIT
