# Moof Language Reference

## 1. Getting Started

```bash
moof                    # start the REPL
moof file.moof          # run a file
moof -e '(+ 1 2)'      # evaluate an expression
```

---

## 2. Syntax

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
[42 abs]                         ; unary -- no args
[list at: 0]                     ; keyword -- labeled arg
[dict put: "x" value: 42]       ; multi-keyword
[list sort]                      ; positional
```

### Free Nesting

```moof
(if (> [list length] 0) [list at: 0] nil)
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

## 3. Data Types

| Type    | Literals                          | Class name   |
|---------|-----------------------------------|--------------|
| Integer | `42`, `-7`, `0xFF`                | `Integer`    |
| Float   | `3.14`, `-0.5`, `1e10`            | `Float`      |
| String  | `"hello"`, `"line\nbreak"`        | `String`     |
| Symbol  | `'foo` (quoted identifier)        | `Symbol`     |
| Bool    | `true`, `false`                   | `TrueClass` / `FalseClass` (parent: `Bool`) |
| Nil     | `nil`                             | `NilClass`   |
| Cons    | `'(1 2 3)`, `(list 1 2 3)`       | `Cons`       |
| Table   | `{name: "moof" version: 1}`, `{1 2 3}` | `Table` |
| Range   | `(range 10)`, `(range 1 10)`      | `Range`      |
| Closure | `(lambda (x) (* x x))`, `{ \|x\| (* x x) }` | `Closure` |
| Object  | `(ClassName arg1 arg2)`           | user-defined |

All types respond to messages. All types have classes that can be extended.

### Interpolated Strings

```moof
(define name "world")
$"hello, \(name)!"              ; => "hello, world!"
$"2 + 2 = \((+ 2 2))"          ; => "2 + 2 = 4"
```

### Tables

Tables are a dual array+hash data structure:

```moof
{1, 2, 3}                       ; array table
{name: "moof", version: 1}      ; hash table
{}                               ; empty table
```

### Blocks (Brace Lambdas)

```moof
{ |x| (* x x) }                 ; same as (lambda (x) (* x x))
{ |a b| (+ a b) }               ; multi-param block
{ |x . rest| rest }             ; variadic block
```

---

## 4. Special Forms

### define

```moof
(define x 42)                      ; bind a value
(define (square x) (* x x))       ; shorthand for function definition
; desugars to: (define square (lambda (x) (* x x)))
```

`define` bindings are mutable via `set!`.

### lambda / fn

```moof
(lambda (x) (* x x))              ; anonymous function
(fn (x) (* x x))                  ; alias for lambda
(lambda (a b . rest) rest)         ; variadic -- rest collects extra args
```

### if

```moof
(if condition then-expr else-expr)
(if (> x 0) "positive")           ; else is optional (returns nil)
```

### let

```moof
(let ((x 1) (y 2))
  (+ x y))                        ; => 3
```

`let` bindings are immutable -- `set!` on them raises an error.

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
(set! x 20)                       ; mutate an existing define binding
```

### quote

```moof
(quote (1 2 3))                    ; => unevaluated cons list
'(1 2 3)                           ; sugar for quote
'foo                               ; => Symbol
```

### quasiquote / unquote / unquote-splice

```moof
(define x 42)
`(a b ,x)                         ; => (a b 42)
`(a ,@(list 1 2 3) b)             ; => (a 1 2 3 b)
```

### and / or

Short-circuiting logical operators.

```moof
(and (> x 0) (< x 100))           ; returns false or the right value
(or default-val (compute-val))     ; returns first truthy value
```

### cond

```moof
(cond
  ((= x 1) "one")
  ((= x 2) "two")
  (else     "other"))
```

### match

```moof
(match value
  (pattern1 body1)
  (pattern2 body2)
  (_        default-body))
```

See [Pattern Matching](#7-pattern-matching) for pattern syntax.

### try / catch

```moof
(try
  (dangerous-operation)
  (catch e
    (print [e message])))          ; e is a Moof Error object
```

The catch variable `e` is bound to a Moof Error object (not a string). Use `[e message]` to get the message string, `[e to_s]` for a display form.

### class

```moof
(class Point
  (fields x y)
  (method length []
    (sqrt (+ (* [self x] [self x]) (* [self y] [self y])))))
```

See [Object System](#6-object-system) for full details.

### trait

```moof
(trait Printable
  (method toString []
    "<object>"))
```

### protocol

```moof
(protocol Countable length)
(protocol Iterable map: filter: each:)
```

### type (ADTs)

```moof
(type Option (Some value) None)
(type Result (Ok value) (Err reason))
```

### defmacro

```moof
(defmacro unless (cond body) `(if (not ,cond) ,body nil))
(defmacro when (cond body) `(if ,cond ,body nil))
```

### module

```moof
(module Math
  (define (square x) (* x x))
  (define pi 3.14159))
```

### use

```moof
(use Math)                         ; import module bindings into scope
```

### require

```moof
(require "path/to/file.moof")     ; load and evaluate a file
```

### -> (Pipeline)

```moof
(-> value
  [method1]
  [method2: arg]
  (function arg))
```

The pipeline threads the previous result as the first argument:

```moof
(-> "hello world"
  [uppercase]
  [split: " "]
  [join: "-"])                     ; => "HELLO-WORLD"
```

Pipeline steps can be:
- `[selector args...]` -- message send, value becomes receiver
- `(func args...)` -- function call, value becomes first arg
- `bare-name` -- unary function call

---

## 5. Message Passing

Messages are sent with `[]` syntax. The parser desugars them to `(__send receiver "selector" args...)`.

### Unary Messages

```moof
[42 abs]                           ; no arguments
["hello" uppercase]
[list first]
```

### Keyword Messages

```moof
[dict put: "key" value: 42]       ; selector is "put:value:"
[str replace_all: "a" with: "b"]  ; selector is "replace_all:with:"
```

### Positional Arguments

```moof
[list at 0]                        ; selector is "at", one arg
```

### Self

Inside method bodies, `self` refers to the message receiver:

```moof
(class Circle
  (fields radius)
  (method area []
    (* pi (* [self radius] [self radius]))))
```

### Keyword Args in S-Expressions

Keyword labels are allowed in s-expression calls and are stripped during evaluation:

```moof
(some-func value: 42 name: "foo")  ; equivalent to (some-func 42 "foo")
```

### Selector References

The `&` operator creates a lambda that sends a message:

```moof
&length                            ; => (lambda (__r) (__send __r "length"))
(map &length (list "hi" "hello"))  ; => (2 5)

&(contains: "x")                   ; partial keyword message
```

---

## 6. Object System

### Defining Classes

```moof
(class Point
  (fields x y)

  (method length []
    (sqrt (+ (* [self x] [self x]) (* [self y] [self y]))))

  (method add: [other]
    (Point (+ [self x] [other x]) (+ [self y] [other y]))))
```

### Creating Instances

```moof
(define p (Point 3 4))
[p x]                              ; => 3 (field access)
[p length]                         ; => 5.0 (method call)
```

### Inheritance

```moof
(class Animal
  (fields name)
  (method speak [] "..."))

(class Dog
  (extends Animal)
  (method speak []
    $"[self name] says woof!"))

(define d (Dog "Rex"))
[d speak]                          ; => "Rex says woof!"
[d name]                           ; => "Rex" (inherited field)
```

### Traits

```moof
(trait Describable
  (method describe []
    $"I am a \([self className])"))

(class Point
  (uses Describable)
  (fields x y))
```

Traits provide method composition. Class methods override trait methods on conflict.

### Open Classes

Classes can be reopened at any time to add new methods:

```moof
(class Dog (fields name))

; later...
(class Dog
  (method speak []
    $"[self name] says woof!"))
```

New methods are merged in. Existing methods can be overridden.

### Extending Built-in Types

All built-in types are proper classes that can be extended:

```moof
(class Integer
  (method double [] (* self 2)))

[42 double]                        ; => 84

(class String
  (method shout [] [self uppercase]))

["hello" shout]                    ; => "HELLO"
```

### Metaclasses

Every class has a metaclass. When you reference a class name like `Point`, you get the metaclass object. Calling it constructs an instance. The metaclass's `is_meta` flag enables constructor behavior.

### Field Mutation

```moof
[obj set: "fieldName" to: newValue]  ; set field on a Table
```

For objects, field access is through message dispatch (field names become unary methods).

---

## 7. Pattern Matching

```moof
(match value
  (pattern body)
  ...)
```

### Pattern Types

**Literals** -- match by equality:

```moof
(match x
  (42 "the answer")
  (true "yes")
  ("hello" "greeting")
  (nil "nothing"))
```

**Wildcards** -- `_` matches anything, binds nothing:

```moof
(match x
  (_ "anything"))
```

**Variable Bindings** -- lowercase symbols bind the matched value:

```moof
(match x
  (n (* n 2)))                     ; n binds to x
```

**Cons Patterns** -- destructure lists:

```moof
(match lst
  ((head . tail) head))            ; destructure into first and rest
```

**Constructor Patterns** -- destructure ADT instances:

```moof
(match opt
  ((Some v) v)                     ; destructure Some, bind inner value
  (None "nothing"))                ; match None (no fields)
```

**Table Patterns** -- match hash table entries:

```moof
(match tbl
  ({name: n age: a} (list n a)))   ; destructure hash keys
```

**Uppercase Symbols** -- treated as constructor names (not variable bindings):

```moof
(match x
  (None "nothing"))                ; matches an object whose class is None
```

---

## 8. Macros

Macros operate on the homoiconic cons-list representation. Since the AST is just Values, macros receive unevaluated cons lists and return cons lists to be evaluated.

```moof
(defmacro unless (cond body)
  `(if (not ,cond) ,body nil))

(defmacro when (cond body)
  `(if ,cond ,body nil))
```

- `` ` `` (backtick) -- quasiquote: templates a cons list
- `,` (comma) -- unquote: evaluate and splice a value in
- `,@` (comma-at) -- unquote-splice: splice a list's elements in

```moof
(defmacro swap! (a b)
  `(let ((tmp ,a))
     (set! ,a ,b)
     (set! ,b tmp)))
```

---

## 9. Modules

```moof
(module MyLib
  (define (helper x) (* x 2))
  (define greeting "hello"))

(use MyLib)
(helper 21)                        ; => 42
```

### require

```moof
(require "path/to/file.moof")     ; load and execute a file
```

---

## 10. Built-in Functions

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

### List Operations

| Function | Signature | Description |
|----------|-----------|-------------|
| `list` | `(list a b ...)` | Construct a cons list |
| `cons` | `(cons x lst)` | Prepend element to cons list |
| `car` | `(car lst)` | First element (head) |
| `cdr` | `(cdr lst)` | Rest of list (tail) |

### I/O

| Function | Signature | Description |
|----------|-----------|-------------|
| `print` | `(print a b ...)` | Print with newline, returns nil |
| `display` | `(display a b ...)` | Print without newline, returns nil |
| `format` | `(format template args...)` | String formatting with `~a` placeholders |
| `read-line` | `(read-line)` | Read a line from stdin |

### File I/O

| Function | Signature | Description |
|----------|-----------|-------------|
| `read-file` | `(read-file path)` | Read entire file as string |
| `write-file` | `(write-file path content)` | Write string to file |
| `file-exists?` | `(file-exists? path)` | Check if file exists |
| `read-lines` | `(read-lines path)` | Read file as list of lines |

### Other

| Function | Signature | Description |
|----------|-----------|-------------|
| `apply` | `(apply f args-list)` | Apply function to a list of arguments |
| `error` | `(error msg)` | Raise a RuntimeError |
| `exit` | `(exit code)` | Exit the process |
| `not` | `(not x)` | Logical negation |
| `type-of` | `(type-of x)` | Type name as string |
| `range` | `(range n)`, `(range a b)`, `(range a b step)` | Create a Range value |
| `time` | `(time f)` | Call f and print elapsed time |

---

## 11. Built-in Methods by Type

Messages are sent with `[]` syntax: `[receiver selector]` or `[receiver selector: arg]`.

### Object (inherited by all classes)

| Selector | Args | Description |
|----------|------|-------------|
| `class` | -- | The class object (for user objects: via metaclass) |
| `to_s` | -- | String representation |
| `inspect` | -- | Detailed string representation |
| `nil?` | -- | Always false (overridden by NilClass) |
| `is_a:` | class-name | Is the object an instance of this class or a subclass? |
| `responds_to:` | selector | Can handle this message? |
| `==` | other | Structural equality |
| `!=` | other | Structural inequality |
| `hash` | -- | Hash code |
| `send:` | selector-str | Dynamic message send |

### Integer

Inherits from Numeric > Object.

| Selector | Args | Description |
|----------|------|-------------|
| `abs` | -- | Absolute value |
| `to_s` | -- | Convert to string |
| `to_f` | -- | Convert to float |
| `to_i` | -- | Identity (returns self) |
| `zero?` | -- | Is zero? |
| `positive?` | -- | Greater than 0? |
| `negative?` | -- | Less than 0? |
| `even?` | -- | Is even? |
| `odd?` | -- | Is odd? |
| `nil?` | -- | Always false |
| `class` | -- | `"Integer"` |
| `sqrt` | -- | Square root (returns Float) |
| `pow:` | exponent | Exponentiation |
| `max:` | other | Larger of two |
| `min:` | other | Smaller of two |
| `**` | other | Exponentiation (alias) |
| `gcd:` | other | Greatest common divisor |
| `lcm:` | other | Least common multiple |
| `times:` | function | Call function n times (0..self) |
| `upto:` | n | Range from self to n (inclusive, as list) |
| `downto:` | n | Range from self down to n (inclusive, as list) |
| `between:and:` | lo, hi | Is self between lo and hi? |
| `bit_and:` | other | Bitwise AND |
| `bit_or:` | other | Bitwise OR |
| `bit_xor:` | other | Bitwise XOR |
| `+`, `-`, `*`, `/`, `%` | other | Arithmetic (message form) |
| `>`, `<`, `>=`, `<=` | other | Comparison (message form) |

Stdlib extensions:

| Selector | Args | Description |
|----------|------|-------------|
| `even?` | -- | `(= (% self 2) 0)` |
| `odd?` | -- | `(not [self even?])` |
| `times:` | function | `(for-each f (range self))` |
| `to:` | n | Range from self to n (inclusive) |
| `downto:` | n | Cons list from self down to n |

### Float

Inherits from Numeric > Object.

| Selector | Args | Description |
|----------|------|-------------|
| `abs` | -- | Absolute value |
| `to_s` | -- | Convert to string |
| `to_f` | -- | Identity |
| `to_i` | -- | Truncate to integer |
| `zero?` | -- | Is zero? |
| `positive?` | -- | Greater than 0? |
| `negative?` | -- | Less than 0? |
| `nil?` | -- | Always false |
| `class` | -- | `"Float"` |
| `round` | -- | Round to nearest integer |
| `floor` | -- | Floor to integer |
| `ceil` | -- | Ceiling to integer |
| `sqrt` | -- | Square root |
| `pow:` | exponent | Exponentiation |
| `max:` | other | Larger of two |
| `min:` | other | Smaller of two |
| `nan?` | -- | Is NaN? |
| `infinite?` | -- | Is infinite? |
| `finite?` | -- | Is finite? |
| `**` | other | Exponentiation (alias) |
| `truncate` | -- | Truncate toward zero |
| `round:` | places | Round to n decimal places |
| `+`, `-`, `*`, `/` | other | Arithmetic (message form) |
| `>`, `<`, `>=`, `<=` | other | Comparison (message form) |

Stdlib extensions:

| Selector | Args | Description |
|----------|------|-------------|
| `round:` | places | Round to n decimal places (via power) |
| `floor` | -- | `[self to_i]` |
| `ceil` | -- | Ceiling via to_i |

### String

| Selector | Args | Description |
|----------|------|-------------|
| `length` | -- | Character count |
| `uppercase` | -- | Uppercase copy |
| `lowercase` | -- | Lowercase copy |
| `reverse` | -- | Reversed copy |
| `trim` | -- | Strip leading/trailing whitespace |
| `strip` | -- | Alias for trim |
| `capitalize` | -- | Capitalize first character |
| `to_s` | -- | Identity |
| `to_i` | -- | Parse as integer |
| `to_f` | -- | Parse as float |
| `to_sym` | -- | Convert to symbol |
| `chars` | -- | List of single-char strings |
| `bytes` | -- | List of byte values |
| `nil?` | -- | Always false |
| `class` | -- | `"String"` |
| `empty?` | -- | Is empty string? |
| `at:` | index | Character at index |
| `contains:` | substr | Contains substring? |
| `starts_with:` | prefix | Starts with prefix? |
| `ends_with:` | suffix | Ends with suffix? |
| `split:` | delimiter | Split into cons list |
| `concat:` | other | Concatenate strings |
| `replace_all:with:` | target, replacement | Replace all occurrences |
| `slice:length:` | start, length | Substring |
| `index_of:` | substr | Index of substring (-1 if not found) |
| `each_char:` | function | Call function on each character |
| `each_line:` | function | Call function on each line |
| `*` | n | Repeat string n times |

Stdlib extensions:

| Selector | Args | Description |
|----------|------|-------------|
| `empty?` | -- | `(= [self length] 0)` |
| `blank?` | -- | Same as empty? |
| `words` | -- | Split into words (trimmed, no empties) |
| `lines` | -- | Split on newlines |
| `repeat:` | n | Repeat string n times |
| `includes:` | s | Alias for contains: |
| `first:` | n | First n characters |
| `last:` | n | Last n characters |

### Symbol

| Selector | Args | Description |
|----------|------|-------------|
| `to_s` | -- | Symbol name as string |
| `to_sym` | -- | Identity |
| `inspect` | -- | Quoted representation |
| `length` | -- | Length of symbol name |
| `nil?` | -- | Always false |
| `class` | -- | `"Symbol"` |

### Cons (Lists)

| Selector | Args | Description |
|----------|------|-------------|
| `car` | -- | First element |
| `cdr` | -- | Rest of list |
| `empty?` | -- | Always false (Cons is never empty; nil is) |
| `length` | -- | Element count |
| `first` | -- | First element (alias for car) |
| `rest` | -- | Rest of list (alias for cdr) |
| `last` | -- | Last element |
| `reverse` | -- | Reversed copy |
| `nil?` | -- | Always false |
| `class` | -- | `"Cons"` |
| `to_s` | -- | String representation `(1 2 3)` |
| `map:` | function | Map function over elements |
| `filter:` | predicate | Filter elements by predicate |
| `each:` | function | Apply for side effects |
| `any:` | predicate | Any element matches? |
| `all:` | predicate | All elements match? |
| `none:` | predicate | No elements match? |
| `at:` | index | Element at index |
| `contains:` | value | Contains value? |
| `push:` | value | New list with value appended |
| `flatten` | -- | Flatten nested lists |
| `sort` | -- | Sorted copy |
| `take:` | n | First n elements |
| `drop:` | n | Drop first n elements |
| `zip:` | other | Zip with another list |
| `join:` | separator | Join elements into string |
| `reduce:init:` | function, initial | Fold left |
| `sort_by:` | key-function | Sort by key function |
| `uniq` | -- | Remove duplicate elements |
| `nth:` | index | Element at index |
| `append:` | other | Concatenate with another list |
| `cons:` | value | Prepend value (returns new list) |

Stdlib extensions:

| Selector | Args | Description |
|----------|------|-------------|
| `sum` | -- | Sum of elements |
| `product` | -- | Product of elements |
| `min` | -- | Minimum element |
| `max` | -- | Maximum element |
| `compact` | -- | Remove nil elements |
| `enumerate` | -- | Zip with indices |
| `second` | -- | Element at index 1 |
| `third` | -- | Element at index 2 |
| `flat-map:` | function | Map then flatten |
| `count:` | predicate | Count matching elements |

### Table

| Selector | Args | Description |
|----------|------|-------------|
| `length` | -- | Total entry count (array + hash) |
| `empty?` | -- | Is empty? |
| `nil?` | -- | Always false |
| `class` | -- | `"Table"` |
| `to_s` | -- | String representation |
| `keys` | -- | List of keys (indices for array, strings for hash) |
| `values` | -- | List of values |
| `at:` | key | Value for key (integer index or string key) |
| `get:` | key | Alias for at: |
| `put:value:` | key, value | New table with entry added/updated |
| `set:to:` | key, value | Mutate in place, return table |
| `remove:` | key | New table with key removed |
| `has_key:` | key | Has key? |
| `merge:` | other-table | Merge two tables |
| `push:` | value | Append to array portion |
| `pop` | -- | Remove and return last array element |
| `first` | -- | First array element |
| `last` | -- | Last array element |
| `shift` | -- | Remove and return first array element |
| `unshift:` | value | Prepend to array portion |
| `compact` | -- | Remove nil values |
| `each:` | function | Iterate over all entries |
| `map:` | function | Map over all entries |
| `filter:` | predicate | Filter entries |
| `find:` | predicate | Find first matching entry |
| `count:` | predicate | Count matching entries |
| `entries` | -- | List of [key, value] pairs |
| `has_value:` | value | Contains this value? |
| `each_pair:` | function | Call f(key, value) for hash entries |
| `each_with_index:` | function | Call f(value, index) for array entries |
| `select:` | predicate | Filter hash entries by pred(key, value) |
| `reject:` | predicate | Reject hash entries by pred(key, value) |

Stdlib extensions (on `Map` class, applies to hash-like tables):

| Selector | Args | Description |
|----------|------|-------------|
| `each:` | function | Call f on each key |
| `map-values:` | function | Map over values, keeping keys |
| `select:` | predicate | Filter by pred(key, value) |
| `reject:` | predicate | Reject by pred(key, value) |

### Range

| Selector | Args | Description |
|----------|------|-------------|
| `each:` | function | Iterate over range elements |
| `map:` | function | Map function, return list |
| `filter:` | predicate | Filter, return list |
| `to_list` | -- | Materialize as cons list |
| `contains:` | value | Is value in range? |
| `size` | -- | Number of elements |
| `length` | -- | Alias for size |
| `first` | -- | First element |
| `last` | -- | Last element |
| `reverse` | -- | Reversed list |
| `to_s` | -- | String representation `1..10` |
| `class` | -- | `"Range"` |
| `nil?` | -- | Always false |

### Closure

| Selector | Args | Description |
|----------|------|-------------|
| `call:` | args... | Call the closure |
| `arity` | -- | Number of required params |
| `nil?` | -- | Always false |
| `class` | -- | `"Closure"` |
| `to_s` | -- | String representation `<name/arity>` |
| `curry:` | partial-arg | Return a new closure with one arg pre-applied |

### Bool (TrueClass / FalseClass)

| Selector | Args | Description |
|----------|------|-------------|
| `not` | -- | Logical negation |
| `to_s` | -- | `"true"` or `"false"` |
| `nil?` | -- | Always false |
| `class` | -- | `"Bool"` |
| `and:` | other | Logical AND |
| `or:` | other | Logical OR |

### Nil (NilClass)

| Selector | Args | Description |
|----------|------|-------------|
| `nil?` | -- | Always true |
| `empty?` | -- | Always true |
| `length` | -- | Always 0 |
| `to_s` | -- | `"nil"` |
| `class` | -- | `"Nil"` |

### Error

| Selector | Args | Description |
|----------|------|-------------|
| `message` | -- | Error message string |
| `to_s` | -- | `"ClassName: message"` |
| `inspect` | -- | Same as to_s |

---

## 12. Standard Library

The standard library (`stdlib/stdlib.moof`) is written in Moof and loaded automatically.

### Functional-Style Wrappers

These bridge `()` and `[]` by letting you write `(map f lst)` instead of `[lst map: f]`:

| Function | Signature | Description |
|----------|-----------|-------------|
| `length` | `(length x)` | `[x length]` |
| `reverse` | `(reverse lst)` | `[lst reverse]` |
| `sort` | `(sort lst)` | `[lst sort]` |
| `flatten` | `(flatten lst)` | `[lst flatten]` |
| `empty?` | `(empty? x)` | `[x empty?]` |
| `first` | `(first lst)` | `[lst first]` |
| `last` | `(last lst)` | `[lst last]` |
| `map` | `(map f lst)` | `[lst map: f]` |
| `filter` | `(filter pred lst)` | `[lst filter: pred]` |
| `reduce` | `(reduce f init lst)` | `[lst reduce: f init: init]` |
| `for-each` | `(for-each f lst)` | `[lst each: f]` |
| `take` | `(take n lst)` | `[lst take: n]` |
| `drop` | `(drop n lst)` | `[lst drop: n]` |
| `zip` | `(zip a b)` | `[a zip: b]` |
| `nth` | `(nth i lst)` | `[lst at: i]` |

### Type Checks

| Function | Signature | Description |
|----------|-----------|-------------|
| `number?` | `(number? x)` | Integer or Float? |
| `integer?` | `(integer? x)` | Integer? |
| `float?` | `(float? x)` | Float? |
| `string?` | `(string? x)` | String? |
| `list?` | `(list? x)` | List (Cons)? |
| `nil?` | `(nil? x)` | Nil? |
| `bool?` | `(bool? x)` | Boolean? |
| `function?` | `(function? x)` | Closure? |
| `symbol?` | `(symbol? x)` | Symbol? |

### Identity and Composition

| Function | Signature | Description |
|----------|-----------|-------------|
| `identity` | `(identity x)` | Returns its argument |
| `compose` | `(compose f g)` | `(lambda (x) (f (g x)))` |
| `flip` | `(flip f)` | `(lambda (a b) (f b a))` |
| `constantly` | `(constantly x)` | Function that always returns x |
| `complement` | `(complement pred)` | Negates a predicate |
| `pipe` | `(pipe f g h ...)` | Left-to-right function composition (variadic) |
| `juxt` | `(juxt f g h ...)` | Apply multiple functions, return list of results |
| `partial` | `(partial f arg1 ...)` | Partially apply arguments |

### Dynamic Dispatch

| Function | Signature | Description |
|----------|-----------|-------------|
| `send` | `(send receiver selector . args)` | Dynamic message send |
| `to-string` | `(to-string x)` | `[x to_s]` |

### Assertions

| Function | Signature | Description |
|----------|-----------|-------------|
| `assert` | `(assert condition)` | Error if false |
| `assert-equal` | `(assert-equal expected actual)` | Error if not equal |

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
| `clamp` | `(clamp n lo hi)` | Clamp to range |
| `gcd` | `(gcd a b)` | Greatest common divisor |
| `lcm` | `(lcm a b)` | Least common multiple |
| `factorial` | `(factorial n)` | n! |
| `power` | `(power base exp)` | Integer exponentiation (fast) |
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
| `any?` | `(any? pred lst)` | Any element matches? |
| `all?` | `(all? pred lst)` | All elements match? |
| `none?` | `(none? pred lst)` | No elements match? |
| `find` | `(find pred lst)` | First matching element or nil |
| `append` | `(append lst1 lst2 ...)` | Concatenate multiple lists (variadic) |
| `zip-with` | `(zip-with f lst1 lst2)` | Zip with combining function |
| `repeat` | `(repeat x n)` | List of n copies of x |
| `partition` | `(partition pred lst)` | Split into (matches non-matches) |
| `frequencies` | `(frequencies lst)` | Table of element counts |
| `sort-by` | `(sort-by key lst)` | Sort by key function |
| `sum-by` | `(sum-by key lst)` | Sum after applying key |
| `max-by` | `(max-by key lst)` | Element with max key |
| `min-by` | `(min-by key lst)` | Element with min key |
| `chunk-by` | `(chunk-by n lst)` | Split into chunks of size n |
| `iterate` | `(iterate f x n)` | `(x (f x) (f (f x)) ...)` n times |
| `index-of` | `(index-of pred lst)` | Index of first match (-1 if none) |
| `flat-map` | `(flat-map f lst)` | Map then flatten |

### String Utilities

| Function | Signature | Description |
|----------|-----------|-------------|
| `string-empty?` | `(string-empty? s)` | Is empty string? |
| `string-join` | `(string-join sep lst)` | Join list with separator |
| `words` | `(words s)` | Split string into words |
| `lines` | `(lines s)` | Split string into lines |
| `string-concat` | `(string-concat s1 s2 ...)` | Concatenate strings (variadic) |
| `string-repeat` | `(string-repeat s n)` | Repeat string n times |

### Memoization

| Function | Signature | Description |
|----------|-----------|-------------|
| `memoize` | `(memoize f)` | Cache results by argument |

### ADTs

```moof
(type Option (Some value) None)
(type Result (Ok value) (Err reason))
(type Pair   (Pair fst snd))
```

### Option Helpers

| Function | Signature | Description |
|----------|-----------|-------------|
| `some?` | `(some? x)` | Is it `Some`? |
| `unwrap` | `(unwrap opt default)` | Extract value or default |
| `option-map` | `(option-map f opt)` | Map over Some |
| `option-flat-map` | `(option-flat-map f opt)` | FlatMap over Some |
| `option-filter` | `(option-filter pred opt)` | Filter Some by predicate |

### Result Helpers

| Function | Signature | Description |
|----------|-----------|-------------|
| `ok?` | `(ok? x)` | Is it `Ok`? |
| `err?` | `(err? x)` | Is it `Err`? |
| `unwrap-ok` | `(unwrap-ok res default)` | Extract Ok or default |
| `unwrap-err` | `(unwrap-err res default)` | Extract Err or default |
| `result-map` | `(result-map f res)` | Map over Ok |
| `result-flat-map` | `(result-flat-map f res)` | FlatMap over Ok |
| `try-call` | `(try-call f args...)` | Call f, wrap in Ok/Err |

### Pair Helpers

| Function | Signature | Description |
|----------|-----------|-------------|
| `fst` | `(fst p)` | First of pair |
| `snd` | `(snd p)` | Second of pair |
| `map-fst` | `(map-fst f p)` | Map over first |
| `map-snd` | `(map-snd f p)` | Map over second |

### Protocols

```moof
(protocol Countable  length)
(protocol Iterable   map: filter: each:)
(protocol Comparable >)
(protocol Showable   to_s)
```

### Macros

```moof
(defmacro unless (cond body) `(if (not ,cond) ,body nil))
(defmacro when (cond body) `(if ,cond ,body nil))
```

---

## 13. REPL

### Special Variables

- `_` -- the result of the last evaluated expression

### Meta-Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `,help` | `,h` | Show available commands |
| `,env` | `,e` | Print all bindings (`,env filter` to search) |
| `,type expr` | `,t` | Show the runtime type of an expression |
| `,doc name` | `,d` | Show documentation/signature for a binding |
| `,methods ClassName` | `,m` | List methods for a class |
| `,classes` | -- | List all defined classes |
| `,load file` | -- | Load and evaluate a .moof file |
| `,time expr` | -- | Benchmark an expression |
| `,clear` | -- | Clear the screen |
| `,reset` | -- | Reset the interpreter |
| `,version` | `,v` | Print Moof version |
| `,quit` | `,q` | Exit the REPL |

### Multi-line Input

If parentheses or brackets are unbalanced, the REPL prompts for continuation:

```
moof> (define (fib n)
 ...>   (if (< n 2) n
 ...>     (+ (fib (- n 1)) (fib (- n 2)))))
moof> (fib 10)
=> 55 : Integer
```

### Tab Completion

Completes function names, variable names, keywords, selectors, and meta-commands. Refreshes after each evaluation to include new bindings.

### Tips

The REPL shows a random tip on startup, such as:
- Try `[\"hello\" uppercase]` to send a message to a string
- Use `(-> value [step1] [step2])` for pipelines
- `_` always holds the last result

---

## 14. Truthiness, Equality, Errors

### Truthiness

- `false` and `nil` are **falsy**
- Everything else is **truthy** (including `0`, `""`, empty lists, and empty tables)

### Equality

- `(= a b)` -- structural/value equality. Numbers cross-compare (Integer vs Float).
- `(eq? a b)` -- reference identity. Objects compared by pointer.
- `[a == b]` -- method form of structural equality.
- `[a != b]` -- method form of structural inequality.
- Closures are never equal to each other (even the same closure).

### Error Hierarchy

```
Error (fields: message)
  SyntaxError
  RuntimeError
    NameError
    TypeError
    ArityError
    MessageError
  IOError
```

All errors include source location (line:column) when available. In `catch` blocks, the caught value is a Moof Error object with a `message` method.

### Variadic Functions

Use `.` before the rest parameter:

```moof
(define (sum-all . nums)
  (reduce + 0 nums))

(sum-all 1 2 3 4 5)               ; => 15
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

### Pipeline

```moof
(-> (list 1 2 3 4 5)
  (filter even?)
  (map square)
  sum)                             ; => 20
```

### Pattern Matching & ADTs

```moof
(type Option (Some value) None)

(define (safe-div a b)
  (if (= b 0) None (Some (/ a b))))

(match (safe-div 10 3)
  ((Some v) (print $"Result: \(v)"))
  (None     (print "Division by zero")))
```

### Classes

```moof
(class Point
  (fields x y)

  (method length []
    (sqrt (+ (* [self x] [self x]) (* [self y] [self y]))))

  (method + [other]
    (Point (+ [self x] [other x]) (+ [self y] [other y]))))

(define p1 (Point 3 4))
(define p2 (Point 1 2))
(print [[p1 + p2] length])        ; => 5.830...
```

### Macros

```moof
(defmacro unless (cond body)
  `(if (not ,cond) ,body nil))

(unless false (print "this runs"))
```
