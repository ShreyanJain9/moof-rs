# Moof Phase 1 — Implementation Plan

Synthesized from three independent reviews (Claude, Codex, Gemini). This is the actionable build plan for the Ruby prototype.

## Project Structure

```
moof/
├── exe/
│   └── moof                    # CLI: moof, moof file.moof, moof -e '...'
├── lib/
│   ├── moof.rb                 # Top-level requires + public API
│   ├── moof/
│   │   ├── version.rb          # Moof::VERSION
│   │   ├── errors.rb           # SyntaxError, RuntimeError, NameError, MessageError, ImmutableBindingError
│   │   ├── token.rb            # Moof::Token = Data.define(:type, :lexeme, :literal, :line, :column)
│   │   ├── ast.rb              # All AST node classes
│   │   ├── lexer.rb            # Moof::Lexer — source → Array[Token]
│   │   ├── parser.rb           # Moof::Parser — tokens → AST::Program
│   │   ├── normalizer.rb       # Moof::Normalizer — lowers MessageSend → __send calls
│   │   ├── environment.rb      # Lexical scope chain
│   │   ├── function.rb         # Moof::Function — closure object, responds to call:
│   │   ├── values.rb           # Moof::SymbolValue + helpers for quoted data
│   │   ├── dispatcher.rb       # Message send protocol for built-in types
│   │   ├── builtins.rb         # Core functions: +, -, *, /, list, format, print, =, eq?, etc.
│   │   ├── interpreter.rb      # Tree-walking evaluator over normalized AST
│   │   ├── printer.rb          # Pretty-printing for REPL output
│   │   ├── meta_commands.rb    # ,help, ,load, ,ast, ,env
│   │   ├── completion.rb       # Tab completion hooks
│   │   └── repl.rb             # REPL loop: readline, _, ,commands
├── test/
│   ├── test_helper.rb
│   ├── lexer_test.rb
│   ├── parser_test.rb
│   ├── normalizer_test.rb
│   ├── interpreter_test.rb
│   ├── dispatcher_test.rb
│   └── integration_test.rb
├── examples/
│   ├── hello.moof
│   ├── fibonacci.moof
│   └── messages.moof
├── Gemfile
└── moof.gemspec
```

## Core Interfaces (Contracts)

### Pipeline

```
Source String
    ↓  Lexer#tokenize
Array[Token]
    ↓  Parser#parse_program
AST::Program  (may contain MessageSend nodes)
    ↓  Normalizer#call
AST::Program  (MessageSend → __send calls)
    ↓  Interpreter#evaluate
Ruby value (Integer, Float, String, Bool, nil, Array, Hash, Function, SymbolValue)
```

### Token

```ruby
Moof::Token = Data.define(:type, :lexeme, :literal, :line, :column)

# Token types:
# :LPAREN :RPAREN :LBRACKET :RBRACKET :LBRACE :RBRACE
# :INTEGER :FLOAT :STRING :TRUE :FALSE :NIL
# :IDENTIFIER :COLON_ID (e.g. "insertValue:")
# :QUOTE :EOF
```

### AST Nodes

All nodes are `Data.define` subclasses under `Moof::AST`:

```ruby
Program(expressions)

# Literals
IntegerLiteral(value)
FloatLiteral(value)
StringLiteral(value)
BoolLiteral(value)
NilLiteral()

# Names
Identifier(name)

# Compound
MapLiteral(pairs)           # pairs: Array[[String, Node]]
Quote(expression)

# Calls
Call(callee, arguments)     # (func arg1 arg2)
MessageSend(receiver, selector, arguments)  # [obj sel: arg] — lowered by normalizer

# Special forms (parsed directly, not generic Call)
Define(name, value)
DefineFunction(name, params, body)
Lambda(params, body)
If(condition, then_branch, else_branch)
Let(bindings, body)         # bindings: Array[[String, Node]]
Do(expressions)
SetBang(name, value)
TryCatch(body, error_name, catch_body)
```

### Normalizer Lowering Rules

```
[obj length]                    → Call(Identifier("__send"), [obj, "length"])
[list at 0]                     → Call(Identifier("__send"), [list, "at", 0])
[dict insertValue: 42 forKey: "x"] → Call(Identifier("__send"), [dict, "insertValue:forKey:", 42, "x"])
```

After normalization, the interpreter never sees `MessageSend` nodes.

### Environment

```ruby
Environment#define(name, value, mutable: true) → value
Environment#get(name) → value               # raises NameError
Environment#set(name, value) → value         # raises ImmutableBindingError for let bindings
Environment#child → Environment              # new scope with self as parent
```

### Dispatcher

```ruby
Dispatcher#send_message(receiver, selector, args, interpreter:) → value
# Handles: numbers, strings, lists, maps, functions (call:), booleans, nil
# Raises MessageError for unknown selectors
```

### Public API

```ruby
Moof.evaluate(source, filename: "(eval)", env: nil) → value
Moof.repl → void
```

## Dependency Graph & Complexity

```
                    ┌─────────┐
                    │  AST (S) │
                    └────┬────┘
           ┌─────────────┼─────────────┐
           ▼             ▼             ▼
     ┌──────────┐  ┌──────────┐  ┌───────────┐
     │ Token (S) │  │Errors (S)│  │ Values (S)│
     └─────┬────┘  └────┬─────┘  └─────┬─────┘
           ▼             ▼              ▼
     ┌──────────┐  ┌───────────────────────────┐
     │ Lexer (M) │  │ Environment + Function (M) │
     └─────┬────┘  └────────────┬──────────────┘
           ▼                    ▼
     ┌──────────┐  ┌───────────────────────────┐
     │Parser (L)│  │   Dispatcher/Builtins (L)  │
     └─────┬────┘  └────────────┬──────────────┘
           ▼                    ▼
     ┌─────────────┐  ┌──────────────┐
     │Normalizer(S)│  │Interpreter(L)│
     └──────┬──────┘  └──────┬───────┘
            └────────┬───────┘
                     ▼
              ┌────────────┐
              │  REPL (M)  │
              └────────────┘
```

## Agent Work Split

### Agent A (Claude) — Syntax Frontend

**Files:** `token.rb`, `ast.rb`, `lexer.rb`, `parser.rb`, `normalizer.rb`, `errors.rb` + tests

**Deliverables:**
- `Lexer.new(source).tokenize` → `Array[Token]`
- `Parser.new(tokens).parse_program` → `AST::Program`
- `Normalizer.new.call(program)` → normalized `AST::Program`

**Key challenges:**
- Colon disambiguation: `:` is keyword-arg separator in `[]`, key-value separator in `{}`
- Keyword selector accumulation: `[obj k1: a1 k2: a2]` → selector `"k1:k2:"`, args `[a1, a2]`
- Nested block comments `#| ... |#`
- Quote sugar: `'(1 2 3)` → `Quote(Call(Identifier("list"), [...]))`

**Can start immediately.** No dependencies on other agents.

---

### Agent B (Codex) — Runtime & Evaluator

**Files:** `environment.rb`, `function.rb`, `values.rb`, `dispatcher.rb`, `builtins.rb`, `interpreter.rb` + tests

**Deliverables:**
- `Environment` with define/get/set, immutability enforcement, child scopes
- `Function` closure object with `call(interpreter, args)`
- `Dispatcher.send_message(receiver, selector, args, interpreter:)`
- `Interpreter.new(env).evaluate(program)` → value
- All builtins: `+`, `-`, `*`, `/`, `>`, `<`, `=`, `eq?`, `list`, `format`, `print`, `__send`
- Built-in type methods: `[42 abs]`, `["hello" length]`, `[(list 1 2 3) reverse]`, `[{} keys]`, etc.

**Key challenges:**
- `__send` as a normal builtin that delegates to `Dispatcher`
- Function objects responding to `call:` message via dispatcher
- Immutable `let` bindings vs mutable `define` bindings
- `try/catch` wrapping Ruby exceptions from dispatcher

**Can start immediately** by coding against the AST contract (frozen before work begins). Uses stub/mock ASTs for testing until parser lands.

---

### Agent C (Gemini) — CLI, REPL & Integration

**Files:** `lib/moof.rb`, `exe/moof`, `printer.rb`, `meta_commands.rb`, `completion.rb`, `repl.rb`, `Gemfile`, `moof.gemspec`, `version.rb`, integration tests, example `.moof` files

**Deliverables:**
- `Moof.evaluate(source)` — wires lexer → parser → normalizer → interpreter
- `Moof.repl` — full REPL with readline
- `exe/moof` CLI: `moof` (REPL), `moof file.moof` (run file), `moof -e '...'` (eval)
- `Printer.format(value)` — pretty-prints Moof values
- Meta-commands: `,help`, `,load file`, `,ast expr`, `,env`
- Tab completion: context-aware for functions and `,` commands
- `_` for last result
- Integration test suite
- Example programs: `hello.moof`, `fibonacci.moof`, `messages.moof`

**Key challenges:**
- Stable public API that wires all components without leaking internals
- Tab completion that introspects the environment for function names
- Pretty-printing nested structures and Moof-specific types

**Can start immediately** on skeleton (Gemfile, CLI, REPL shell). Wiring to real components happens after A and B deliver.

---

## Integration Order

```
Step 0: Freeze AST contract (all three agents agree on ast.rb node shapes)
        ↓
Step 1: Agent A delivers ast.rb + token.rb + errors.rb
        (shared foundation — merge first)
        ↓
Step 2: In parallel:
        - Agent A continues: lexer.rb, parser.rb, normalizer.rb
        - Agent B builds: environment, function, values, interpreter, dispatcher, builtins
        - Agent C builds: Gemfile, exe/moof, repl shell, printer, meta_commands
        ↓
Step 3: Agent A's parser lands → Agent C can wire Moof.evaluate
        ↓
Step 4: Agent B's interpreter lands → full pipeline works
        ↓
Step 5: Agent C runs integration tests, wires REPL to real evaluator
        ↓
Step 6: End-to-end: `echo '(print "hello world")' | exe/moof` works
```

## Fastest Hello World

Minimum vertical slice to prove the pipeline:

```moof
(print "hello world")
```

Requires only:
- Lexer: tokenize `(`, `print`, `"hello world"`, `)`
- Parser: parse as `Call(Identifier("print"), [StringLiteral("hello world")])`
- Normalizer: pass-through (no `[]` to lower)
- Interpreter: look up `print` in builtins, call it
- Builtin `print`: puts the string

**Do not wait for `[]`, `{}`, `quote`, `try/catch`, or REPL to land before proving this works.**

## Tricky Integration Points

1. **Colon disambiguation** — `:` means different things in `[]` (keyword arg) vs `{}` (key-value pair). Parser must track context. Lexer can emit `COLON_ID` tokens (e.g., `insertValue:`) inside `[]`, and plain `IDENTIFIER` + implicit colon inside `{}`.

2. **`__send` as the bridge** — After normalization, all message sends are `Call(Identifier("__send"), ...)`. The interpreter treats `__send` as a normal builtin. The builtin delegates to `Dispatcher.send_message`. This keeps the interpreter clean.

3. **Quote semantics** — Quoted identifiers → `SymbolValue`. Quoted lists → arrays of quoted values (not evaluated). Quoted `()` must not trigger function calls.

4. **Map key consistency** — `{name: "moof"}` stores key as the string `"name"`. Do not leak Ruby symbols.

5. **Mutation rules** — `define` bindings are mutable via `set!`. `let` bindings are immutable — `set!` on a `let` binding raises `ImmutableBindingError`.

6. **`try/catch` parsing** — Parser must expect `(try BODY (catch VAR HANDLER))` shape and produce a clean `TryCatch` node. Don't leave this as raw list surgery for the interpreter.

7. **Function `call:` message** — `[f call: 5]` should work the same as `(f 5)`. Dispatcher must recognize `Function` objects and delegate to their `call` method.
