# Moof Language Specification (Draft v0.2)

## Overview

Moof is a general-purpose scripting language that combines Lisp's homoiconic s-expressions with Smalltalk/Objective-C style message passing. It aims to be expressive, flexible, and fun.

## Core Syntax

Moof has two primary expression forms that nest freely. Both are expressions — they always return a value.

### S-Expressions `()`

Used for function calls, special forms, and general Lisp-style evaluation.

```moof
(+ 1 2)              ; => 3
(define x 42)
(if (> x 10) "big" "small")
(lambda (a b) (+ a b))
```

### Message Sends `[]`

Used for Smalltalk-style message passing to objects. Three forms:

**Unary** — no arguments:
```moof
[list length]
["hello" uppercase]
[42 abs]
```

**Positional** — arguments without keywords:
```moof
[list at 0]
[console log "hello"]
```

**Keyword** — Objective-C style labeled arguments:
```moof
[dict insertValue: 42 forKey: "x"]
[string replaceOccurrencesOf: "a" with: "b"]
```

### Nesting

The two forms nest freely — `[]` is an expression everywhere, just like `()`:

```moof
(if (> [list length] 0)
  [list at 0]
  nil)

[obj doSomethingWith: (+ 1 2) and: [other value]]

(define lengths (map (lambda (s) ["s" length]) strings))
```

### AST Normalization

Internally, `[]` message sends lower to a canonical s-expression form before evaluation and macro expansion. This means macros only need to operate on one AST representation:

```
[obj method: arg]  →  (__send obj "method:" arg)
```

This is an implementation detail — users write `[]`, but the evaluator and future macro system see a unified AST.

## Data Types

### Primitives

| Type     | Examples                    |
|----------|-----------------------------|
| Integer  | `42`, `-7`, `0xFF`          |
| Float    | `3.14`, `-0.5`, `1e10`     |
| String   | `"hello"`, `"line\nbreak"` |
| Bool     | `true`, `false`            |
| Nil      | `nil`                      |
| Symbol   | `foo`, `+`, `my-var`       |

All built-in types respond to messages:
```moof
[42 abs]                      ; => 42
["hello" length]              ; => 5
[(list 1 2 3) reverse]        ; => (3 2 1)
["hello" replaceAll: "l" with: "r"]  ; => "herro"
```

### Maps

Map literals use `{}` with `:` separating keys and values:

```moof
{name: "moof" version: 1 stable: false}
```

Inside `{}`, `:` is key-value pair syntax (not keyword-message syntax). Maps are dynamic objects that respond to messages:

```moof
(define m {x: 1 y: 2})
[m at: "x"]          ; => 1
[m keys]             ; => ("x" "y")
```

### Lists

```moof
'(1 2 3)              ; quoted list literal
(list 1 2 3)          ; constructed list
```

## Scoping & Bindings

Moof uses **lexical scoping**. Closures capture their enclosing environment.

Bindings are **immutable by default**:

```moof
(define x 10)
(set! x 20)          ; explicit mutation via set!
```

`let` bindings are always immutable:

```moof
(let ((x 1) (y 2))
  (+ x y))           ; x and y cannot be mutated
```

Inside methods, `self` refers to the receiver. Fields are accessed via `self`:

```moof
(method length []
  (sqrt (+ (* [self x] [self x]) (* [self y] [self y]))))
```

## Special Forms

```moof
; Variable binding
(define name value)
(let ((x 1) (y 2)) (+ x y))

; Conditionals
(if condition then-expr else-expr)
(cond
  (test1 expr1)
  (test2 expr2)
  (else  expr3))

; Functions
(define (square x) (* x x))       ; shorthand
(lambda (x) (* x x))              ; anonymous

; Sequencing
(do
  (define x 1)
  (define y 2)
  (+ x y))

; Quoting
(quote (1 2 3))
'(1 2 3)
```

## Evaluation Rules

- **S-expressions `()`:** Left-to-right evaluation. First element evaluates to a callable; remaining elements are evaluated left-to-right as arguments.
- **Message sends `[]`:** Receiver is evaluated first, then arguments left-to-right. The selector is looked up on the receiver's class.
- **Evaluation order is guaranteed** — important for side effects.

## Object System

Moof uses **class-based inheritance** with **traits** for composition.

### Classes & Messages

Objects respond to messages. The `[]` syntax dispatches a message to a receiver.

```moof
(class Point
  (fields x y)

  (method length []
    (sqrt (+ (* [self x] [self x]) (* [self y] [self y]))))

  (method add: [other]
    (Point (+ [self x] [other x]) (+ [self y] [other y]))))

(define p (Point 3 4))
[p length]            ; => 5
[p add: (Point 1 1)]  ; => (Point 4 5)
```

### Traits

Traits provide reusable behavior without inheritance hierarchies:

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

### Introspection

Objects support runtime introspection:

```moof
[p methods]           ; => (length add: x y toString)
[p class]             ; => Point
[p respondsTo: "length"]  ; => true
```

### Functions and Methods

Functions (defined with `define`/`lambda`) are objects that respond to a `call` message. This unifies the two syntaxes at the protocol level:

```moof
(define f (lambda (x) (* x x)))
(f 5)                 ; => 25  (s-expression call)
[f call: 5]           ; => 25  (message send — same thing)
```

## Equality

- **`=`** — structural/value equality (like Elixir's `==`)
- **`eq?`** — reference/identity equality
- Classes can override `=` by implementing an `equals:` method

```moof
(= (Point 1 2) (Point 1 2))   ; => true  (same values)
(eq? (Point 1 2) (Point 1 2)) ; => false  (different objects)
```

## Error Handling

### Phase 1: Exceptions

Simple exception-based error handling for the dynamic/scripting side:

```moof
(try
  [file open: "missing.txt"]
  (catch e
    (print [e message])))
```

"Message not understood" errors raise exceptions on the dynamic side.

### Future: Result Types

When the static type system is added, `Result` types handle expected failures:

```moof
(type Result
  (Ok value)
  (Err reason))

(match [file tryOpen: path]
  ((Ok f)   (print [f contents]))
  ((Err e)  (print e)))
```

**Boundary rule:** Exceptions are for bugs and runtime faults. `Result` is for expected, recoverable failures.

## Static Type System (Future)

Inspired by Haskell/Elixir — details to be designed. Rough direction:

```moof
; Algebraic data types
(type Option
  (Some value)
  None)

; Pattern matching
(match my-option
  ((Some x) (print x))
  (None     (print "nothing")))

; Type annotations (optional)
(: square (-> Int Int))
(define (square x) (* x x))
```

### Static/Dynamic Bridge

Typed code interacts with dynamic objects via structural selector types:

```moof
; Typed code declares what selectors it expects
(: process-shape (-> (Obj [area perimeter]) Float))
(define (process-shape s)
  (+ [s area] [s perimeter]))
```

`Obj[selectors...]` means "any object that responds to these messages" — structural typing for the dynamic side, checked at compile time where possible, at runtime otherwise.

## Message Resolution

When `[receiver selector args...]` is evaluated:

1. Evaluate `receiver` to get an object
2. Look up `selector` on the object's class (walking the inheritance chain)
3. Evaluate arguments left-to-right
4. Invoke the method with `self` bound to the receiver

For keyword messages, the full selector is the concatenation of all keywords:
`[obj insertValue: v forKey: k]` has selector `insertValue:forKey:`.

## Comments

```moof
; single line comment

#|
  block comment
  (can be nested)
|#
```

## String Interpolation

**Deferred.** Use `format` in v1:

```moof
(format "hello ~a, you are ~a" name age)
```

Interpolation syntax will be chosen when added (likely `\()` or `$""`).

## Module System (Future)

```moof
(import math)
(import (io read write))
```

Details TBD — needs namespace/module-qualified name resolution design.

## Macros (Future)

Deferred until the core evaluator, quoting model, and module system are stable. When added:

- **Hygienic** macros operating on the canonical s-expression AST
- `[]` is lowered to s-expression form before macro expansion
- Macros only need to understand one AST representation

## Concurrency (Future)

**Actor/process model** — deferred until after the object system and type system exist.

The `[]` message-send syntax naturally extends to actor communication:

```moof
[local-obj doWork: data]     ; synchronous, local
[remote-actor doWork: data]  ; async, same syntax
```

Typed message protocols between actors are a future direction.

## REPL

The REPL is a first-class part of the language experience:

- `_` — last result
- Tab completion — context-aware (`()` shows functions, `[]` shows methods)
- `,` prefix for meta-commands: `,help`, `,type expr`, `,load file`
- `[obj methods]` for runtime introspection
- Pretty-printed output

## Implementation Plan

### Phase 1 — Ruby Prototype
- Lexer (tokenizer)
- Parser (s-exprs + message sends + `{}` maps)
- AST normalization (`[]` → canonical s-expression form)
- Tree-walking interpreter
- REPL with `_`, tab completion, `,` meta-commands
- Core special forms: `define`, `if`, `lambda`, `let`, `do`, `quote`, `set!`, `try`/`catch`
- Built-in types responding to messages (numbers, strings, lists, maps)
- Lexical scoping with closures

### Phase 2 — Object System
- Class definitions with fields and methods
- Traits for composition
- `self` binding and field access
- Inheritance and method lookup
- Introspection (`methods`, `class`, `respondsTo:`)
- Functions as callable objects

### Phase 3 — Type System
- Algebraic data types
- Pattern matching (`match`)
- Optional type annotations
- `Obj[selectors...]` structural typing for dynamic objects
- Result types for error handling

### Phase 4 — Production Runtime
- Port to Rust, or target BEAM VM, or both — TBD
- Bytecode compiler + VM (replaces tree-walking)

### Phase 5 — Advanced Features
- Hygienic macros
- Actor-based concurrency
- Module system with namespaces

## Open Design Questions (Remaining)

1. **Concurrency details** — pure actors, or also lightweight tasks/async? Backpressure?
2. **Macro hygiene model** — Scheme-style `syntax-rules`? Racket-style `syntax-parse`? Elixir-style?
3. **Module system** — how are names qualified? `math.sin` vs `math:sin` vs `[math sin]`?
4. **Runtime target** — Rust native? BEAM VM? Both via compilation backends?
5. **Python/Ruby FFI** — should the prototype allow calling host-language objects via `[]`?
6. **Metaclasses** — should classes themselves be objects that respond to messages?
