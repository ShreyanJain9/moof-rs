# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Moof?

Moof is a programming language that combines Lisp s-expressions `()` with Smalltalk-style message passing `[]`. Both syntaxes nest freely. The Rust implementation in `src/` is the current active codebase; the Ruby prototype lives in `prototype/` for reference only.

## Build & Run

```bash
cargo build --release          # optimized binary at target/release/moof
cargo build                    # debug build
cargo run                      # start REPL
cargo run -- file.moof         # run a file
cargo run -- -e '(+ 1 2)'     # eval expression
```

## Architecture

The pipeline is: **Source -> Lexer -> Parser -> Interpreter**

There is no normalizer or separate AST enum. The parser produces cons lists (Values), and the interpreter evaluates them directly. This is a homoiconic design.

1. **Lexer** (`lexer.rs`) -- tokenizes source. Handles `()[]{}`, interpolated strings `$"...\(expr)..."`, hex literals, block comments `#|...|#`, colon identifiers `name:`.

2. **Parser** (`parser.rs`) -- produces `Vec<Value>` (cons lists). Absorbs all desugaring that the v1 normalizer used to do: `[]` message sends become `(__send receiver "selector" args...)`, `{}` becomes `lambda` (blocks) or `__table`/`__table-array` (table literals), `->` pipeline becomes nested calls, `$""` becomes `__str-interp`, `&sel` becomes a lambda wrapping `__send`. No AST enum -- output is cons-list Values.

3. **Interpreter** (`interpreter.rs`) -- classic Lisp eval on cons lists. Special forms are recognized by interned symbol comparison against `KnownSymbols`. Smalltalk-style metaclass bootstrap: every class has a metaclass, Object is the root, Class's metaclass is itself. TCO via `Eval` enum trampoline (`Eval::Val` | `Eval::TailCall`). Owns registries for classes, protocols, macros, modules, types.

4. **Builtins** (`builtins.rs`) -- all built-in functions AND type methods. Global functions (`+`, `-`, `print`, `cons`, etc.) and per-class methods (`abs`, `length`, `map:`, etc.) are all registered as unified `Closure` values with `ClosureBody::Native`. One file, one registration pattern.

5. **Symbol** (`symbol.rs`) -- symbol interning. `SymId = u32`. `SymbolTable` maps strings to IDs and back. `KnownSymbols` pre-interns all special form names, internal desugaring names, and common identifiers for O(1) comparison.

6. **MoofInt** (`moofint.rs`) -- `Small(i64)` / `Big(BigInt)` auto-promotion. All arithmetic operations (`+`, `-`, `*`, `/`, `%`) promote to BigInt on overflow and shrink back when results fit in i64. Also provides `pow`, `gcd`, `abs`, `checked_div`, `checked_rem`.

7. **Cons** (`cons.rs`) -- `ConsCell { car, cdr }`, `ListIter`, and helpers (`cons_to_vec`, `vec_to_cons`, `cons_length`, `cons_display`). Cons cells are immutable `Rc`-shared linked lists. This is the AST representation.

8. **Value** (`value.rs`) -- 11-variant enum: `Integer(MoofInt)`, `Float(f64)`, `Bool(bool)`, `Nil`, `Symbol(SymId)`, `Str(Rc<str>)`, `Cons(Rc<ConsCell>)`, `Table(Rc<RefCell<MoofTable>>)`, `Object(Rc<RefCell<MoofObject>>)`, `Closure(Rc<MoofClosure>)`, `Range(Rc<MoofRange>)`. Also defines `MoofTable` (array+hash), `MoofObject` (class ref + fields vec), `MoofClass` (name, superclass, metaclass, methods HashMap, field_names), `MoofClosure` (params, rest_param, `ClosureBody::Expr` | `ClosureBody::Native`, captured env).

9. **Environment** (`environment.rs`) -- `Env` with `SymId` keys. Parent-chain scoping. Each binding tracks mutability (`define` is mutable by default, `let` is immutable).

10. **Error** (`error.rs`) -- `MoofError` with `ErrorKind` (Syntax, Runtime, Name, Message, Arity, Type, IO), message, optional line/column, and optional `error_object: Option<Value>` for the Moof-level Error class hierarchy.

11. **Stdlib** (`stdlib/stdlib.moof`) -- self-hosting standard library embedded via `include_str!`. Provides functional-style wrappers (`map`, `filter`, `reduce`), higher-order utilities (`compose`, `pipe`, `partial`), type checks, numeric utilities, ADTs (`Option`, `Result`, `Pair`), protocols, macros (`when`, `unless`), and built-in type extensions.

12. **REPL** (`repl.rs`) -- rustyline-based with tab completion, meta-commands (`,help`, `,env`, `,type`, `,doc`, `,methods`, `,classes`, `,load`, `,time`, `,clear`, `,reset`, `,version`, `,quit`), multi-line input, `_` for last result, startup tips.

## Key Design Decisions

- **Homoiconic**: Parser produces cons lists, evaluator consumes them. No `Expr` enum anywhere. The AST IS the data structure.
- **Smalltalk metaclasses**: Every class has a metaclass. Object is the root. Class's metaclass is itself. Method dispatch walks class -> superclass chain via `MoofClass::lookup()`.
- **Unified closures**: `ClosureBody::Expr(Value)` (cons list body) or `ClosureBody::Native(NativeFn)` (Rust fn). No separate Function/Builtin/Method types. Both globals and methods are the same `MoofClosure` type.
- **Cons cells**: Immutable `Rc`-shared linked lists. The AST representation. Proper lists terminate with `Nil`.
- **Tables**: Array+hash data structure (`MoofTable`), separate from the object system. `{}` syntax creates tables, not objects.
- **BigInt**: `MoofInt` auto-promotes on overflow, shrinks back when possible. Arithmetic is safe for arbitrary precision.
- **Symbol interning**: All identifiers, selectors, and special form names are interned to `SymId` (u32). O(1) comparison everywhere.
- **TCO**: `Eval` enum with `TailCall { func, args }`, trampoline loop in `call_closure`. Tail-position variants exist for `if`, `do`, `let`, `match`, `cond`, `and`, `or`, `try`.
- **Error objects**: Moof has an Error class hierarchy (Error > RuntimeError > NameError/TypeError/...). `MoofError` wraps an optional `Value` error object for catch blocks.
- **Environment threading**: `eval(&mut self, expr, env)` takes env as parameter. No `self.env` save/restore. Closures capture `env.clone()`.
- **Open classes**: Any class can be reopened at runtime via `(class Name ...)`. Built-in types are proper classes and can be extended with new methods.

## Class Hierarchy

```
Object
  Numeric
    Integer
    Float
  String
  Symbol
  Cons
  Table
  Closure
  Range
  Bool
    TrueClass
    FalseClass
  NilClass
  Error
    SyntaxError
    RuntimeError
      NameError
      TypeError
      ArityError
      MessageError
    IOError
  Class (metaclass of itself)
```

## Smalltalk Features (Phase 0)

- **`super` sends**: `[super method]` in method bodies starts lookup from superclass of defining class. Parser emits `__super-send`, interpreter tracks `__current_class` via `current_method_class` on Interpreter struct.
- **Class methods**: `(classmethod selector (params) body)` in class definitions installs methods on the metaclass. Callable via `[ClassName method]`.
- **`new`/`initialize`**: `[ClassName new]` or `[ClassName new arg1 arg2]` allocates instance with nil fields, sends `initialize` with args. Default `initialize` on Object does nothing. Old `(ClassName arg1 arg2)` shorthand still works.
- **`doesNotUnderstand:`**: When method lookup fails, tries `doesNotUnderstand:` on the class before raising error. Message arg is `{ selector: "name", args: (...) }`.
- **Control-flow-as-messages**: `ifTrue:`, `ifFalse:`, `ifTrue:ifFalse:` on Bool; `ifNil:`, `ifNotNil:` on Nil/Object; `value`, `value:`, `whileTrue:`, `whileFalse:` on Closure. Zero-arg blocks use `{ || body }` syntax.
- **Mutable field bindings**: `set!` inside methods writes back to the object's actual fields. Field bindings in methods are mutable.

## Example Files

`prototype/examples/` contains runnable `.moof` files: hello, fibonacci, messages, pattern_matching, pipeline, adts.

## Documentation

- `README.md` — project overview with examples
- `REFERENCE.md` — complete language reference
- `JOURNAL.md` — development history and session logs
