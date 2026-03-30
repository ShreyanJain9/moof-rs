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

No Rust tests yet. The Ruby prototype has tests in `prototype/test/`.

## Architecture

The pipeline is: **Source → Lexer → Parser → Normalizer → Interpreter**

1. **Lexer** (`lexer.rs`) — tokenizes source. Handles `()[]{}`, interpolated strings `$"...\(expr)..."`, hex literals, block comments `#|...|#`, colon identifiers `name:`.

2. **Parser** (`parser.rs`) — recursive descent producing `Expr` AST. Desugars early: `(define (f x) ...)` → `Define(f, Lambda(...))`, `(and a b)` → `If(a, b, false)`, `(or a b)` → `Let(tmp, If(...))`, `(cond ...)` → nested `If`. Keyword args in calls are stripped to just their values.

3. **Normalizer** (`normalizer.rs`) — AST-to-AST pass that lowers `MessageSend` to `Call(__send, ...)`, desugars `Pipeline`, `StringInterp`, `SelectorRef`. After normalization, the interpreter sees a minimal AST.

4. **Interpreter** (`interpreter.rs`) — tree-walking evaluator. Environment is threaded as a parameter (no save/restore pattern). TCO via trampoline using a separate `Eval` enum (not in `Value`). Owns registries for classes, protocols, macros, modules, types.

5. **Methods** (`methods.rs`) — unified message dispatch. ALL messages go through one path: look up selector in the receiver's class → call `Method::Builtin` or `Method::UserDefined`. Includes Levenshtein-based "did you mean?" suggestions.

6. **Builtin Methods** (`builtin_methods.rs`) — registers all built-in type methods (Integer, Float, String, List, Map, Bool, Nil, Function) on their classes at startup. Methods are `BuiltinMethodFn` closures that receive `(interp, receiver, args)`.

7. **Builtins** (`builtins.rs`) — 24 primitive functions (arithmetic, comparison, equality, list ops, I/O, dispatch, introspection) installed as global bindings.

8. **Stdlib** (`stdlib/stdlib.moof`) — self-hosting standard library embedded via `include_str!`. Provides functional-style wrappers (map, filter, reduce), higher-order utilities, ADTs (Option, Result, Pair), protocols, macros, and type extensions.

## Key Design Decisions

- **Single dispatch path**: All message sends go through `methods::send_message` → class lookup → `Method` enum. No separate hardcoded dispatch tables.
- **Method enum**: `enum Method { Builtin(BuiltinMethodFn), UserDefined(MoofFunction) }` stored in `MoofClass.methods`. Superclass chain walked by `lookup()`.
- **Open classes**: Any class can be reopened at runtime. Built-in types (Integer, String, etc.) are registered as classes and can be extended.
- **Protocols subsume traits**: `MoofProtocol` has both `selectors` (required) and `default_methods` (provided). One registry, not two.
- **Environment threading**: `eval_expr(&mut self, expr, env)` takes env as parameter. No `self.env` save/restore. Closures capture `env.clone()`.
- **Eval enum for TCO**: `enum Eval { Val(Value), TailCall { func, args } }` — control flow is separate from runtime values.
- **IndexMap for maps**: `Value::Map(IndexMap<String, Value>)` — O(1) lookup, preserves insertion order.
- **Value accessors**: `as_str()`, `as_int()`, `as_float()`, `as_number()`, `as_list()`, `into_list()`, `as_bool()` for clean type extraction.

## Example Files

`prototype/examples/` contains runnable `.moof` files: hello, fibonacci, messages, pattern_matching, pipeline, adts.

## Documentation

- `SPEC.md` — language specification
- `REFERENCE.md` — complete language reference
- `ROADMAP.md` — improvement roadmap
