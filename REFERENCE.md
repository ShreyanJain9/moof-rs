# Moof Language Reference

## Getting Started

```bash
moof                    # start the REPL
moof file.moof          # run a file
moof -e '(+ 1 2)'      # evaluate an expression
```

## Syntax Overview

Moof has two expression forms that nest freely. Both always return a value.

### S-Expressions `()`

Function calls and special forms, Lisp-style.

```moof
(+ 1 2)                         ; => 3
(define x 42)
(if (> x 10) "big" "small")
(print "hello")
```

### Message Sends `[]`

Smalltalk/Objective-C style message passing.

```moof
[42 abs]                         ; unary — no args
[list at 0]                      ; positional — args without keywords
[dict insertValue: 42 forKey: "x"]  ; keyword — labeled args
```

### Free Nesting

```moof
(if (> [list length] 0) [list at 0] nil)
[obj doSomethingWith: (+ 1 2) and: [other value]]
```

### Comments

```moof
; single line comment

#| block comment
   (can be nested #| like this |#)
|#
```

---

## Data Types

| Type    | Literals                      | Class name |
|---------|-------------------------------|------------|
| Integer | `42`, `-7`, `0xFF`            | `Integer`  |
| Float   | `3.14`, `-0.5`, `1e10`        | `Float`    |
| String  | `"hello"`, `"line\nbreak"`    | `String`   |
| Bool    | `true`, `false`               | `Bool`     |
| Nil     | `nil`                         | `Nil`      |
| List    | `'(1 2 3)`, `(list 1 2 3)`    | `List`     |
| Map     | `{name: "moof" version: 1}`   | `Map`      |
| Symbol  | `'foo` (quoted identifier)    | —          |

All types respond to messages. All types except Symbol have classes that can be extended.

---

## Special Forms

### define

```moof
(define x 42)                      ; bind a value
(define (square x) (* x x))       ; shorthand for function definition
```

`define` bindings are mutable via `set!`.

### lambda

```moof
(lambda (x) (* x x))              ; anonymous function
(lambda (a b . rest) rest)         ; variadic — rest collects extra args
```

### if

```moof
(if condition then-expr else-expr)
(if (> x 0) "positive")           ; else is optional (returns nil)
```

### cond

```moof
(cond
  ((= x 1) "one")
  ((= x 2) "two")
  (else     "other"))
```

### let

```moof
(let ((x 1) (y 2))
  (+ x y))                        ; => 3
```

`let` bindings are **immutable** — `set!` on them raises `ImmutableBindingError`.

### do

```moof
(do
  (define x 1)
  (define y 2)
  (+ x y))                        ; => 3, returns last expression
```

### set!

```moof
(define x 10)
(set! x 20)                       ; mutate an existing binding
```

### and / or

Short-circuiting logical operators.

```moof
(and (> x 0) (< x 100))           ; returns false or the right value
(or default-val (compute-val))     ; returns first truthy value
```

### quote

```moof
(quote (1 2 3))                    ; => list without evaluation
'(1 2 3)                           ; sugar for quote
'foo                               ; => SymbolValue "foo"
```

### try / catch

```moof
(try
  (dangerous-operation)
  (catch e
    (print e)))                    ; e is the error message string
```

Catches `RuntimeError` and its subclasses (`NameError`, `MessageError`, etc.).

---

## Scoping & Bindings

- **Lexical scoping** — closures capture their defining environment
- **Immutable by default** for `let`; `define` bindings are mutable via `set!`
- Inside methods, `self` refers to the message receiver

---

## The Object System

### Defining Classes

```moof
(class Point
  (fields x y)

  (method length []
    (sqrt (+ (* [self x] [self x]) (* [self y] [self y]))))

  (method add: [other]
    (+ [self x] [other x])))
```

### Creating Instances

```moof
(define p (Point 3 4))
[p x]                              ; => 3 (field access)
[p length]                         ; => 5 (method call)
```

### Inheritance

```moof
(class Animal
  (fields name)
  (method speak [] "..."))

(class Dog
  (extends Animal)
  (method speak []
    (format "~a says woof!" [self name])))

(define d (Dog "Rex"))
[d speak]                          ; => "Rex says woof!"
[d name]                           ; => "Rex" (inherited field)
```

### Traits

```moof
(trait Printable
  (method toString []
    "<object>"))

(class Point
  (uses Printable)
  (fields x y)

  (method toString []
    (format "(~a, ~a)" [self x] [self y])))
```

Traits provide method composition. Class methods override trait methods on conflict.

### Open Classes

Classes can be reopened at any time to add new methods:

```moof
(class Dog (fields name))

; later...
(class Dog
  (method speak []
    (format "~a says woof!" [self name])))
```

New methods are merged in. Existing methods can be overridden.

### Extending Built-in Types

All built-in types (`Integer`, `Float`, `String`, `List`, `Map`, `Bool`, `Nil`, `Function`) are proper classes that can be extended:

```moof
(class Integer
  (method double [] (* self 2))
  (method even?  [] (= (% self 2) 0)))

[42 double]                        ; => 84
[7 even?]                          ; => false

(class String
  (method shout [] [self uppercase])
  (method blank? [] (= [self length] 0)))

["hello" shout]                    ; => "HELLO"

(class List
  (method sum [] (reduce + 0 self))
  (method second [] [self at: 1]))

[(list 1 2 3) sum]                 ; => 6
```

User-defined methods on built-in types take priority over hardcoded behavior.

### Introspection

```moof
[obj className]                    ; => "Point"
[obj class]                        ; => the MoofClass object
[obj methods]                      ; => list of selector strings
[obj respondsTo: "length"]         ; => true/false
[ClassName fields]                 ; => list of field names
[ClassName methods]                ; => list of method selectors
```

### Field Mutation

```moof
[obj set: "fieldName" to: newValue]  ; returns the object
```

---

## Built-in Functions

### Arithmetic

| Function | Signature | Description |
|----------|-----------|-------------|
| `+` | `(+ a b ...)` | Addition (variadic, identity 0) |
| `-` | `(- a)` or `(- a b ...)` | Negation or subtraction |
| `*` | `(* a b ...)` | Multiplication (variadic, identity 1) |
| `/` | `(/ a b ...)` | Division (left-to-right) |
| `%` | `(% a b)` | Modulo |

### Comparison

| Function | Signature | Description |
|----------|-----------|-------------|
| `>` | `(> a b)` | Greater than |
| `<` | `(< a b)` | Less than |
| `>=` | `(>= a b)` | Greater than or equal |
| `<=` | `(<= a b)` | Less than or equal |

### Equality

| Function | Signature | Description |
|----------|-----------|-------------|
| `=` | `(= a b)` | Structural/value equality |
| `eq?` | `(eq? a b)` | Reference/identity equality |

### Logic

| Function | Signature | Description |
|----------|-----------|-------------|
| `not` | `(not x)` | Logical negation |
| `and` | `(and a b)` | Short-circuiting AND (special form) |
| `or` | `(or a b)` | Short-circuiting OR (special form) |

Note: `and`/`or` are special forms and short-circuit. `false` and `nil` are falsy; everything else is truthy.

### List Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `list` | `(list a b ...)` | Construct a list |
| `cons` | `(cons x lst)` | Prepend element to list |
| `car` | `(car lst)` | First element |
| `cdr` | `(cdr lst)` | All but first element |
| `length` | `(length x)` | Length of list/string/map |
| `reverse` | `(reverse lst)` | Reverse a list |
| `sort` | `(sort lst)` | Sort a list |
| `flatten` | `(flatten lst)` | Flatten nested lists |
| `concat` | `(concat a b)` | Concatenate two lists |
| `zip` | `(zip a b)` | Zip two lists into pairs |
| `nth` | `(nth i lst)` | Element at index |
| `take` | `(take n lst)` | First n elements |
| `drop` | `(drop n lst)` | Drop first n elements |
| `range` | `(range n)`, `(range a b)`, `(range a b step)` | Generate a range |
| `empty?` | `(empty? x)` | Test if empty |

### Functional

| Function | Signature | Description |
|----------|-----------|-------------|
| `map` | `(map f lst)` | Apply f to each element |
| `filter` | `(filter pred lst)` | Keep elements where pred is truthy |
| `reduce` | `(reduce f init lst)` | Fold left with initial value |
| `for-each` | `(for-each f lst)` | Apply f for side effects, returns nil |
| `apply` | `(apply f args-list)` | Apply function to a list of arguments |

### I/O

| Function | Signature | Description |
|----------|-----------|-------------|
| `print` | `(print a b ...)` | Print with newline, returns nil |
| `display` | `(display a b ...)` | Print without newline, returns nil |
| `format` | `(format template args...)` | String formatting with `~a` placeholders |

### Type Checks

| Function | Signature | Description |
|----------|-----------|-------------|
| `number?` | `(number? x)` | Integer or Float? |
| `string?` | `(string? x)` | String? |
| `list?` | `(list? x)` | List? |
| `nil?` | `(nil? x)` | Nil? |
| `bool?` | `(bool? x)` | Boolean? |
| `function?` | `(function? x)` | Function or builtin? |
| `hash?` | `(hash? x)` | Map? |
| `symbol?` | `(symbol? x)` | Quoted symbol? |

### Error

| Function | Signature | Description |
|----------|-----------|-------------|
| `error` | `(error msg)` | Raise a RuntimeError |

---

## Built-in Messages (by type)

Messages are sent with `[]` syntax: `[receiver selector]` or `[receiver selector: arg]`.

### Integer / Float

| Selector | Args | Description |
|----------|------|-------------|
| `abs` | — | Absolute value |
| `to_s` | — | Convert to string |
| `to_f` | — | Convert to float |
| `to_i` | — | Convert to integer |
| `zero?` | — | Is zero? |
| `positive?` | — | Greater than 0? |
| `negative?` | — | Less than 0? |
| `nil?` | — | Always false |
| `class` | — | `"Integer"` or `"Float"` |
| `pow:` | exponent | Exponentiation |
| `max:` | other | Larger of two |
| `min:` | other | Smaller of two |
| `+`, `-`, `*`, `/`, `%` | other | Arithmetic (message form) |
| `>`, `<`, `>=`, `<=` | other | Comparison (message form) |

### String

| Selector | Args | Description |
|----------|------|-------------|
| `length` | — | Character count |
| `uppercase` | — | Uppercase copy |
| `lowercase` | — | Lowercase copy |
| `reverse` | — | Reversed copy |
| `to_s` | — | Identity |
| `to_i` | — | Parse as integer |
| `to_f` | — | Parse as float |
| `chars` | — | List of single-char strings |
| `trim` | — | Strip leading/trailing whitespace |
| `nil?` | — | Always false |
| `class` | — | `"String"` |
| `at:` | index | Character at index |
| `contains:` | substr | Contains substring? |
| `startsWith:` | prefix | Starts with prefix? |
| `endsWith:` | suffix | Ends with suffix? |
| `replaceAll:with:` | target, replacement | Replace all occurrences |
| `split:` | delimiter | Split into list |
| `concat:` | other | Concatenate strings |
| `slice:length:` | start, length | Substring |

### List (Array)

| Selector | Args | Description |
|----------|------|-------------|
| `length` | — | Element count |
| `first` | — | First element |
| `last` | — | Last element |
| `rest` | — | All but first |
| `reverse` | — | Reversed copy |
| `sort` | — | Sorted copy |
| `uniq` | — | Unique elements |
| `flatten` | — | Flatten nested lists |
| `empty?` | — | Is empty? |
| `to_s` | — | String representation |
| `nil?` | — | Always false |
| `class` | — | `"List"` |
| `at:` | index | Element at index |
| `push:` | value | New list with value appended |
| `prepend:` | value | New list with value prepended |
| `contains:` | value | Contains value? |
| `join:` | separator | Join elements into string |
| `indexOf:` | value | Index of value (-1 if not found) |
| `take:` | n | First n elements |
| `drop:` | n | Drop first n elements |
| `zip:` | other | Zip with another list |
| `map:` | function | Map function over elements |
| `filter:` | predicate | Filter elements by predicate |
| `reduce:init:` | function, initial | Fold left |
| `each:` | function | Apply for side effects |
| `any:` | predicate | Any element matches? |
| `all:` | predicate | All elements match? |
| `none:` | predicate | No elements match? |
| `sortBy:` | key-function | Sort by key function |

### Map (Hash)

| Selector | Args | Description |
|----------|------|-------------|
| `keys` | — | List of keys |
| `values` | — | List of values |
| `length` | — | Entry count |
| `empty?` | — | Is empty? |
| `to_s` | — | String representation |
| `nil?` | — | Always false |
| `class` | — | `"Map"` |
| `at:` | key | Value for key (nil if missing) |
| `put:value:` | key, value | New map with entry added |
| `remove:` | key | New map with key removed |
| `contains:` | key | Has key? |
| `merge:` | other-map | Merge two maps |

### Function

| Selector | Args | Description |
|----------|------|-------------|
| `call:` | args... | Call the function |
| `arity` | — | Number of required params |
| `nil?` | — | Always false |
| `class` | — | `"Function"` |

### Boolean

| Selector | Args | Description |
|----------|------|-------------|
| `not` | — | Logical negation |
| `to_s` | — | `"true"` or `"false"` |
| `nil?` | — | Always false |
| `class` | — | `"Boolean"` |
| `and:` | other | Logical AND |
| `or:` | other | Logical OR |

### Nil

| Selector | Args | Description |
|----------|------|-------------|
| `nil?` | — | Always true |
| `to_s` | — | `"nil"` |
| `class` | — | `"Nil"` |

### User-Defined Objects (MoofObject)

| Selector | Args | Description |
|----------|------|-------------|
| `class` | — | The MoofClass object |
| `className` | — | Class name as string |
| `methods` | — | List of method selectors |
| `respondsTo:` | selector | Can handle this message? |
| `nil?` | — | Always false |
| `to_s` | — | String representation |
| `set:to:` | field-name, value | Mutate a field |
| *fieldName* | — | Access field by name |
| *any method* | varies | Dispatched via class hierarchy |

---

## Standard Library (stdlib.moof)

The standard library is written in Moof itself and loaded automatically.

### Function Utilities

| Function | Signature | Description |
|----------|-----------|-------------|
| `identity` | `(identity x)` | Returns its argument |
| `compose` | `(compose f g)` | Returns `(lambda (x) (f (g x)))` |
| `flip` | `(flip f)` | Swap argument order: `(lambda (a b) (f b a))` |
| `constantly` | `(constantly x)` | Returns a function that always returns `x` |
| `complement` | `(complement pred)` | Negates a predicate |
| `pipe` | `(pipe f g h ...)` | Left-to-right function composition (variadic) |
| `juxt` | `(juxt f g h ...)` | Apply multiple functions, return list of results |
| `memoize` | `(memoize f)` | Cache results by argument |

### Numeric Utilities

| Function | Signature | Description |
|----------|-----------|-------------|
| `square` | `(square x)` | `(* x x)` |
| `cube` | `(cube x)` | `(* x x x)` |
| `abs` | `(abs x)` | Absolute value |
| `max` | `(max a b)` | Larger of two |
| `min` | `(min a b)` | Smaller of two |
| `even?` | `(even? n)` | Is even? |
| `odd?` | `(odd? n)` | Is odd? |
| `between?` | `(between? n lo hi)` | Is n in [lo, hi]? |
| `clamp` | `(clamp n lo hi)` | Clamp n to range |
| `gcd` | `(gcd a b)` | Greatest common divisor |
| `lcm` | `(lcm a b)` | Least common multiple |
| `factorial` | `(factorial n)` | n! |
| `power` | `(power base exp)` | Exponentiation (integer exp, fast) |
| `sum` | `(sum lst)` | Sum of a list |
| `product` | `(product lst)` | Product of a list |

### Math Constants

| Name | Value |
|------|-------|
| `pi` | `3.141592653589793` |
| `e` | `2.718281828459045` |
| `degrees->radians` | `(degrees->radians d)` |
| `radians->degrees` | `(radians->degrees r)` |

### List Utilities

| Function | Signature | Description |
|----------|-----------|-------------|
| `second` | `(second lst)` | Second element |
| `third` | `(third lst)` | Third element |
| `fourth` | `(fourth lst)` | Fourth element |
| `last` | `(last lst)` | Last element |
| `append` | `(append lst1 lst2 ...)` | Concatenate multiple lists (variadic) |
| `zip-with` | `(zip-with f lst1 lst2)` | Zip with a combining function |
| `repeat` | `(repeat x n)` | List of n copies of x |
| `partition` | `(partition pred lst)` | Split into `(matches non-matches)` |
| `frequencies` | `(frequencies lst)` | Map of element counts |
| `sort-by` | `(sort-by key lst)` | Sort by key function |
| `sum-by` | `(sum-by key lst)` | Sum after applying key |
| `max-by` | `(max-by key lst)` | Element with max key |
| `min-by` | `(min-by key lst)` | Element with min key |
| `chunk-by` | `(chunk-by n lst)` | Split into chunks of size n |
| `iterate` | `(iterate f x n)` | `(x (f x) (f (f x)) ...)` n times |

### Predicate Wrappers

| Function | Signature | Description |
|----------|-----------|-------------|
| `any?` | `(any? pred lst)` | Any element matches predicate? |
| `all?` | `(all? pred lst)` | All elements match? |
| `none?` | `(none? pred lst)` | No elements match? |
| `find` | `(find pred lst)` | First matching element or nil |

### String Utilities

| Function | Signature | Description |
|----------|-----------|-------------|
| `string-empty?` | `(string-empty? s)` | Is empty string? |
| `string-join` | `(string-join sep lst)` | Join list with separator |
| `words` | `(words s)` | Split string into words |
| `lines` | `(lines s)` | Split string into lines |

---

## REPL

### Special Variables

- `_` — the result of the last evaluated expression

### Meta-Commands

| Command | Description |
|---------|-------------|
| `,help` | Show available commands |
| `,env` | Print all bindings in current environment |
| `,ast expr` | Parse and display AST without evaluating |
| `,load file` | Load and evaluate a .moof file |
| `,type expr` | Show the runtime type of a value |
| `,version` | Print Moof version |
| `,quit` / `,exit` | Exit the REPL |

### Multi-line Input

If parentheses or brackets are unbalanced, the REPL prompts for continuation:

```
moof> (define (fib n)
..>    (if (< n 2) n
..>      (+ (fib (- n 1)) (fib (- n 2)))))
moof> (fib 10)
=> 55
```

### Tab Completion

Completes function names, variable names, keywords, and meta-commands.

---

## Variadic Functions

Use `.` before the rest parameter in the parameter list:

```moof
(define (sum-all . nums)
  (reduce + 0 nums))

(sum-all 1 2 3 4 5)               ; => 15

(lambda (first . rest)
  (list first rest))
```

The rest parameter collects all extra arguments into a list.

---

## Truthiness

- `false` and `nil` are **falsy**
- Everything else is **truthy** (including `0`, `""`, and `'()`)

---

## Error Hierarchy

```
MoofError
  SyntaxError         ; parse errors (line, column)
  RuntimeError        ; base runtime error
    NameError         ; undefined variable (line, column)
    MessageError      ; message not understood
    ImmutableBindingError  ; set! on let binding
    ArityError        ; wrong number of arguments
```

All errors include source location (line:column) when available.

---

## AST Normalization

Internally, `[]` message sends are lowered to `__send` calls before evaluation:

```
[obj method: arg]  =>  (__send obj "method:" arg)
```

This means the interpreter only deals with s-expressions. Future macros will only need to understand one AST form.

---

## Evaluation Rules

1. **S-expressions `()`** — left-to-right: callee first, then arguments
2. **Message sends `[]`** — receiver first, then arguments left-to-right
3. **Evaluation order is guaranteed** — important for side effects
4. **Functions are first-class objects** — they respond to the `call:` message:
   ```moof
   (define f (lambda (x) (* x x)))
   (f 5)                           ; => 25
   [f call: 5]                     ; => 25 (same thing)
   ```

---

## Quick Examples

### Fibonacci

```moof
(define (fib n)
  (if (< n 2) n
    (+ (fib (- n 1)) (fib (- n 2)))))

(print (fib 10))                   ; => 55
```

### FizzBuzz

```moof
(for-each
  (lambda (n)
    (cond
      ((= (% n 15) 0) (print "FizzBuzz"))
      ((= (% n 3) 0)  (print "Fizz"))
      ((= (% n 5) 0)  (print "Buzz"))
      (else            (print n))))
  (range 1 101))
```

### Message Passing

```moof
(define greeting "Hello, Moof!")
(print [greeting length])          ; => 12
(print [greeting uppercase])       ; => "HELLO, MOOF!"
(print [greeting replaceAll: "Moof" with: "World"])

(define nums (list 3 1 4 1 5 9))
(print [nums sort])                ; => (1 1 3 4 5 9)
(print [nums map: (lambda (x) (* x x))])  ; => (9 1 16 1 25 81)
```

### Classes & Traits

```moof
(trait Describable
  (method describe []
    (format "I am a ~a" [self className])))

(class Circle
  (uses Describable)
  (fields radius)

  (method area []
    (* pi (* [self radius] [self radius])))

  (method circumference []
    (* 2 pi [self radius])))

(define c (Circle 5))
(print [c area])                   ; => 78.53981633974483
(print [c describe])               ; => "I am a Circle"
```

### Extending Built-in Types

```moof
(class List
  (method sum [] (reduce + 0 self))
  (method average [] (/ [self sum] [self length])))

(class Integer
  (method times: [f] (for-each f (range self))))

(print [(list 10 20 30) average])  ; => 20
[5 times: (lambda (i) (print i))] ; prints 0 1 2 3 4
```

### Higher-Order Functions

```moof
(define (make-adder n)
  (lambda (x) (+ x n)))

(define add5 (make-adder 5))
(print (add5 10))                  ; => 15
(print (map add5 (list 1 2 3)))    ; => (6 7 8)

(define process
  (pipe
    (lambda (x) (* x 2))
    (lambda (x) (+ x 1))
    (lambda (x) (* x x))))

(print (process 3))                ; => 49  ((3*2 + 1)^2)
```
