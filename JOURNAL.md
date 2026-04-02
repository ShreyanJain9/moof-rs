# Moof Development Journal

A record of how the Moof programming language was designed and built, session by session.

---

## Session 1: Language Design & Ruby Prototype

### Starting point

The goal was to create a general-purpose scripting language called Moof that combined Lisp s-expressions `()` with Smalltalk/Objective-C style message passing `[]`. Both syntaxes would nest freely, giving the language two complementary modes of expression.

### Process

The session began with design, not code. When implementation was suggested prematurely, it was rejected outright: "Don't do all that -- first draft a plan and spec for the language in an MD file." A specification was drafted covering syntax, semantics, and the pipeline architecture.

Three models (Codex, Gemini, Claude) reviewed the design in parallel, debating decisions like bracket syntax, dispatch semantics, and how message sends should be lowered to s-expressions. The consensus informed the final spec.

Ruby was chosen as the prototype language ("i'd rather use ruby than python since i know it better"). The implementation used a parallel agent architecture: shared contracts were defined first, then three agents were given non-overlapping file ownership so they could work simultaneously without merge conflicts.

### What was built

The Ruby prototype landed in three incremental commits:

**v0.2** -- Working prototype with classes, traits, open classes, stdlib, and a REPL. The core pipeline was established: Source -> Lexer -> Parser -> Normalizer -> Interpreter. The normalizer's job was to lower `[receiver selector: arg]` message sends into `(__send receiver "selector:" arg)` s-expressions before the interpreter saw them.

**v0.3** -- Pattern matching, algebraic data types, macros, the pipeline operator `->`, blocks `{ |x| body }`, protocols, tail call optimization via trampoline, and string interpolation `$"...\(expr)..."`.

**Module system & self-hosting stdlib** -- Core functionality was moved from Ruby into `stdlib.moof`. The language became partially self-hosting: 24 primitive functions remained in Ruby, with everything else defined in Moof itself. Functional wrappers (map, filter, reduce), ADTs (Option, Result, Pair), and protocol definitions all lived in stdlib.

The Ruby prototype totaled ~4,400 lines across 20 source files in `lib/moof/`, with a test suite in `test/`.

### Key design decisions locked in

- `()` for s-expressions, `[]` for Smalltalk messaging, free nesting between the two
- AST normalization: `[]` lowered to `__send` calls before evaluation
- Open classes: any class reopenable at runtime, including built-in types
- Tail call optimization via trampoline pattern
- Pattern matching, ADTs, macros, pipeline operator, blocks, protocols
- Self-hosting philosophy: minimal primitives in host language, rest in stdlib.moof

---

## Session 2: Rust Rewrite

### Starting point

The Ruby prototype was complete and all tests passed. The instruction was direct: "now it's time for you to move all this prototype stuff into a cute folder somewhere and rewrite it in rust :)"

The Ruby prototype was moved to `prototype/` for reference.

### Process

Three parallel agents built the Rust implementation with non-overlapping file ownership:

- **Agent A**: `lexer.rs`, `parser.rs`, `normalizer.rs`, `token.rs`
- **Agent B**: `interpreter.rs`, `builtins.rs`, `dispatcher.rs`, `pattern_matcher.rs`, `value.rs`
- **Agent C**: `repl.rs`, `main.rs`

A `stdlib.moof` was embedded into the binary via `include_str!`.

### What was built

The Rust rewrite reproduced the full Ruby feature set: lexer with interpolated strings and block comments, recursive-descent parser with early desugaring, AST normalizer, tree-walking interpreter, class system, pattern matching, macros, modules, and a REPL.

After assembly, compilation errors were fixed (notably `value.rs` returning a reference to a temporary in `type_name()`). The binary was ~6,300 lines of Rust plus ~390 lines of stdlib.

### Performance

Fibonacci(30) benchmarked at 8.9 seconds in Rust versus 25.5 seconds in Ruby -- a 2.9x speedup.

### Tested

All six example files in `prototype/examples/` passed: `hello.moof`, `fibonacci.moof`, `messages.moof`, `pattern_matching.moof`, `pipeline.moof`, `adts.moof`.

---

## Session 3: Elegance Overhaul

### Starting point

The Rust implementation worked but was described as "a complete mess." The request was for a plan to make it "much much much more elegant."

### Process

A 9-phase plan was designed and then executed sequentially. Each phase was tested against the full example suite before proceeding.

### Phase 3: Extract TailCall from Value

The `TailCall` variant had been mixed into the `Value` enum, which meant runtime values and control flow lived in the same type. A separate `Eval` enum was created:

```rust
enum Eval { Val(Value), TailCall { func, args } }
```

`TailCall` was removed from the public `Value` type entirely.

### Phase 7: Value accessor methods

Added clean type extraction methods to `Value`: `as_str()`, `as_int()`, `as_float()`, `as_list()`, `as_number()`, `into_list()`, `as_bool()`. These replaced scattered match statements throughout the interpreter and builtins.

### Phase 6: IndexMap for maps

Replaced `Vec<(String, Value)>` with `IndexMap<String, Value>` for map values, gaining O(1) lookups while preserving insertion order. The `indexmap` crate was added to `Cargo.toml`.

### Phase 1: Shrink the AST

Removed five variants from the `Expr` enum: `DefineFunction`, `And`, `Or`, `Cond`, `KeywordArg`. The parser now desugars these inline:

- `(define (f x) body)` becomes `Define(f, Lambda(...))`
- `(and a b)` becomes `If(a, b, false)`
- `(or a b)` becomes `Let(tmp, a, If(tmp, tmp, b))`
- `(cond ...)` becomes nested `If` chains

Also removed `Pipeline`, `StringInterp`, and `SelectorRef` handlers from the interpreter since the normalizer already desugared them.

### Phase 2: Kill the dispatcher monolith

Deleted `dispatcher.rs` -- a 944-line file that was the single largest source file. Replaced it with two focused modules:

- `methods.rs` -- unified message dispatch. ALL messages flow through `send_message` -> class lookup -> `Method` enum.
- `builtin_methods.rs` -- registers built-in type methods on their classes at startup.

A `Method` enum was added to `MoofClass`:

```rust
enum Method { Builtin(BuiltinMethodFn), UserDefined(MoofFunction) }
```

Superclass chain walked by `lookup()`. Levenshtein-based "did you mean?" suggestions added.

### Phase 4: Unify traits and protocols

Merged the separate `trait_registry` and `protocol_registry` into a single system. `MoofProtocol` gained both `selectors` (required) and `default_methods` (provided). One registry replaced two.

### Phase 5: Clean up stdlib

Removed redundant boilerplate from `stdlib.moof`. Kept the functional wrappers (map, filter, reduce) since they bridge `()` and `[]` idiomatically -- `(map fn list)` calls `[list map: fn]` under the hood.

### Phase 8: Thread env as parameter

Removed `self.env` from the `Interpreter` struct. All eval methods now take `env` as a parameter: `eval_expr(&mut self, expr, env)`. This eliminated 10+ save/restore patterns where the interpreter saved the environment, ran something, then restored it.

### Phase 9: Tighten error model

Added `with_loc()` for attaching source locations to errors. Errors auto-decorated from AST nodes. Removed dead `match_char` code from the lexer.

### REPL feature parity

After the elegance phases, REPL parity was addressed. The Ruby REPL had ~17 meta-commands, tab completion, and execution timing. The Rust REPL had 7 commands, 5 of which were stubs.

The Rust REPL was rewritten to include 15 meta-commands: `,help`, `,env`, `,type`, `,doc`, `,methods`, `,classes`, `,protocols`, `,ast`, `,load`, `,time`, `,clear`, `,reset`, `,version`, `,quit`. Tab completion gained keyword, selector, binding, and class awareness. Execution timing was shown automatically on slow evaluations. History deduplication was added.

### Tested throughout

```moof
(+ 1 2)                              ; -> 3
["hello" uppercase]                   ; -> "HELLO"
(and true 42)                        ; -> 42
(or false 99)                        ; -> 99
(cond ((= 1 2) "no") ((= 1 1) "yes") (else "maybe")) ; -> "yes"
(define (fib n) (if (<= n 1) n (+ (fib (- n 1)) (fib (- n 2)))))
(fib 10)                             ; -> 55
(map { |x| (* x x) } (list 1 2 3)) ; -> (1 4 9)
(-> "hello world" [uppercase] [split: " "] [first]) ; -> "HELLO"
[-5 abs]                             ; -> 5
[(list 3 1 2) sort]                 ; -> (1 2 3)
{a: 1 b: 2}                         ; -> {a: 1, b: 2}
(define m {a: 1 b: 2})
[m put: "c" value: 3]               ; -> {a: 1, b: 2, c: 3}
```

All six example files passed after each phase. The net effect was ~2,440 lines added, ~1,670 deleted -- the codebase got smaller while gaining features.

---

## Session 4: v2 Runtime -- Smalltalk VM Foundation

### Starting point

The request was a fundamental architectural shift: "plan a common representation for all objects -- i think it's time to transition to building a genuine smalltalk vm."

### Requirements gathered

Several design questions were resolved through discussion:

- Builtin and user-defined functions should be the same thing (unified closures)
- Lists should be linked lists (cons cells), not arrays
- The AST should be representable as lists (homoiconic -- data IS the AST)
- Maps and arrays should be one type, like Lua tables
- Tables (data structure) and objects/classes are SEPARATE -- tables are a class
- Full Ruby-inspired built-in classes
- Numbers should never overflow (BigInt with auto-promotion)
- Errors should be objects with a class hierarchy
- 0-based indexing
- Smalltalk metaclasses (not Lua metatables)
- Big-bang rewrite in a git worktree

### Phase 1: Foundation types

Three agents built the foundation in parallel:

**symbol.rs** -- `SymbolTable` with intern/lookup and `KnownSymbols` struct pre-interning 42 symbols at startup (special forms like `if`, `define`, `lambda`; internal desugaring symbols like `__send`, `__table`; common names like `self`, `true`, `false`, `nil`; and class-system internals like `__name`, `__super`, `__fields`). Symbols are identified by `SymId` (u32), making comparison a single integer compare.

**moofint.rs** -- `MoofInt` enum with `Small(i64)` and `Big(BigInt)` variants. Arithmetic operations auto-promote from Small to Big on overflow. All standard ops (add, sub, mul, div, rem, neg, pow) plus comparison, GCD, LCM. 600 lines covering every edge case.

**value.rs** -- 10-variant `Value` enum: `Integer(MoofInt)`, `Float(f64)`, `Bool(bool)`, `Nil`, `Symbol(SymId)`, `Str(Rc<str>)`, `Cons(Rc<ConsCell>)`, `Table(Rc<RefCell<IndexMap<Value, Value>>>)`, `Object(...)`, `Closure(...)`, plus `Range(MoofRange)`. Includes `Hash`, `Eq`, `Display`, and `Ord` implementations.

**cons.rs** -- `ConsCell` struct with `car: Value` and `cdr: Value`. Proper Lisp cons cells. Helper functions: `cons_to_vec`, `vec_to_cons`, `cons_length`, `cons_display`. `ListIter` for ergonomic iteration.

**environment.rs** -- `Env` using `SymId` keys instead of string keys. Parent-chain scoping with `Rc<RefCell<...>>`.

**error.rs** -- `MoofError` placeholder with source location tracking.

### Phases 2-4: Parser, Interpreter, Builtins

Built in parallel by three agents. Several files from v1 were deleted entirely: `ast.rs`, `normalizer.rs`, `methods.rs`, `builtin_methods.rs`, `pattern_matcher.rs`. In the homoiconic model, there is no separate AST type -- parsed code IS cons lists of Values.

**Parser** -- Produces `Vec<Value>` (cons lists). All desugaring happens inline during parsing:

- `[receiver selector: arg]` becomes `(__send receiver "selector:" arg)`
- `{ |x| body }` becomes `(lambda (x) body)`
- `{a: 1 b: 2}` becomes `(__table "a" 1 "b" 2)`
- `{1 2 3}` becomes `(__table-array 1 2 3)`
- `-> expr [msg]` becomes nested send calls
- `$"hello \(name)"` becomes `(__str-interp "hello " name)`
- `&selector` becomes a lambda wrapping `__send`
- `(define (f x) body)` becomes `(define f (lambda (x) body))`
- `ColonId` tokens become Symbols

**Interpreter** -- Classic Lisp eval on cons lists. The core dispatch is ~50 lines: extract the head of a cons cell, compare its symbol ID against `KnownSymbols`, and branch. 19+ special forms handled. The class system uses Smalltalk metaclasses. Pattern matching operates directly on cons lists and constructor patterns. Macro expansion is trivial because data IS the AST.

**Builtins** -- Unified closures registered on type classes at startup:

| Type | Method count | Highlights |
|------|-------------|------------|
| Integer | 23 | Arithmetic, bit ops, `times:`, `upto:`, `downto:` |
| Float | 20 | Trig, rounding, `finite?`, `round:` |
| String | 19 | `split:`, `replace:with:`, `each_char:`, `*` |
| Cons | 25 | `map:`, `filter:`, `reduce:`, `sort_by:`, `uniq`, `nth:` |
| Table | 17 | `put:value:`, `remove:`, `each_pair:`, `select:`, `reject:` |
| Bool | 5 | `not`, `and:`, `or:`, `to_s`, `class` |
| Nil | 4 | `nil?`, `to_s`, `class`, `empty?` |
| Closure | 3+ | `call`, `call:`, `curry:` |

20+ global functions installed as bindings (arithmetic, comparison, equality, list ops, I/O, dispatch, introspection).

### Key fix: ColonId handling

The parser initially stripped `ColonId` tokens as keyword labels, carrying over v1 behavior. This broke protocol definitions like `(protocol Iterable map: filter: each:)` and method definitions like `(method times: (f) body)` where the colon-suffixed selector names need to be preserved as Symbols. The fix was to make `ColonId` tokens become `Value::Symbol` in the homoiconic model, with the evaluator skipping keyword labels during argument evaluation.

### Key fix: [] in method params

The v1 stdlib used `[]` for empty method parameter lists: `(method sum [] body)`. The v2 parser interpreted `[]` as a message send expression. The fix was twofold: make `[]` return `Value::Nil` in the parser (representing an empty parameter list), and update the stdlib to use `()` for method params.

### Tested

```moof
(+ 1 2)                                          ; -> 3
(* 999999999999999999 999999999999999999)          ; -> 999999999999999998000000000000000001
["hello" uppercase]                               ; -> "HELLO"
(car (list 1 2 3))                               ; -> 1
(cdr (list 1 2 3))                               ; -> (2 3)
(cons 0 (list 1 2))                              ; -> (0 1 2)
{a: 1 b: 2}                                     ; -> {a: 1, b: 2}
(define (fib n) (if (<= n 1) n (+ (fib (- n 1)) (fib (- n 2)))))
(fib 10)                                         ; -> 55
(map { |x| (* x x) } (list 1 2 3))              ; -> (1 4 9)
(filter odd? (list 1 2 3 4 5))                   ; -> (1 3 5)
(class Point (fields x y) (method sum () (+ x y)))
(define p (Point 3 4)) [p sum]                   ; -> 7
(match 42 (42 "yes") (_ "no"))                   ; -> "yes"
(-> "hello world" [uppercase] [split: " "] [first]) ; -> "HELLO"
(try (error "boom") (catch e e))                 ; -> "boom"
(defmacro unless (c b) (list (quote if) (list (quote not) c) b nil))
(unless false 42)                                ; -> 42
(module M (export x) (define x 42)) (use M) x   ; -> 42
```

All six example files passed.

### Completing the v2 plan

After the initial v2 merge, the remaining items from the plan were implemented:

**Tail call optimization** -- Added `Eval` enum with `TailCall` variant. An `eval_tail` method detects tail positions in `if`, `do`, `let`, `match`, `cond`, `and`, `or`. A trampoline in `call_closure` with a `setup_call_env` helper handles the loop. Verified:

```moof
(define (loop n) (if (= n 0) "done" (loop (- n 1))))
(loop 1000000)  ; -> "done", no stack overflow
```

**Object and Symbol class methods** -- 10 Object methods inherited by all types: `class`, `to_s`, `inspect`, `nil?`, `is_a:`, `responds_to:`, `==`, `!=`, `hash`, `send:`. 6 Symbol methods: `to_s`, `to_sym`, `inspect`, `length`, `nil?`, `class`.

**Error objects** -- Error class hierarchy: `SyntaxError`, `RuntimeError`, `NameError`, `TypeError`, `ArityError`, `MessageError`, `IOError`. `MoofError` carries an optional `error_object` Value. `try`/`catch` receives real Moof objects:

```moof
(try (error "boom") (catch e [e message]))  ; -> "boom"
(try (error "boom") (catch e [e class]))    ; -> "RuntimeError"
(try (error "boom") (catch e [e to_s]))     ; -> "RuntimeError: boom"
(try undefined_var (catch e [e class]))     ; -> "NameError"
```

**Protocols** -- `(protocol Name sel1 sel2)` with a registry. Protocol membership tracked via a marker in the environment's Table representation.

**Traits** -- `(trait Name (method sel (params) body))` with default method implementations. Classes adopt traits via `(uses TraitName)` in their body, gaining all the trait's methods.

**Modules** -- `(module Name (export ...) body)`, `(use Name)`, `(use Name (as Alias))`, `(use Name (only sym1 sym2))`.

**Range class** -- Lazy `Value::Range` with `MoofRange` struct (start, end, step). 13 methods: `each:`, `map:`, `filter:`, `to_list`, `contains:`, `size`, `length`, `first`, `last`, `reverse`, `to_s`, `class`, `nil?`.

```moof
(range 5)                           ; -> 0..5
[(range 5) to_list]                 ; -> (0 1 2 3 4)
[(range 1 10) map: { |x| (* x x) }] ; -> (1 4 9 16 25 36 49 64 81)
[(range 1 100) size]                ; -> 99
```

**Table pattern matching** -- `(__table "key" pattern)` recognized in `match_pattern`, enabling destructuring of table values.

**40+ missing methods across all types:**

- Integer: `**`, `gcd:`, `lcm:`, `times:`, `upto:`, `downto:`, `between:and:`, `bit_and`/`bit_or`/`bit_xor`
- Float: `**`, `truncate`, `finite?`, `round:`
- String: `capitalize`, `strip`, `index_of:`, `to_sym`, `each_char:`, `each_line:`, `*`, `bytes`
- Cons: `sort_by:`, `uniq`, `nth:`, `append:`, `cons:`, `empty?`
- Table: `shift`, `unshift:`, `compact`, `find:`, `count:`, `entries`, `has_value:`, `each_pair:`, `each_with_index:`, `select:`, `reject:`
- Closure: `curry:`
- Nil: `empty?`, `length`

```moof
[5 times: { |i| (print i) }]       ; -> 0 1 2 3 4
[3 upto: 7]                        ; -> (3 4 5 6 7)
["hello world" capitalize]          ; -> "Hello world"
["abc" * 3]                        ; -> "abcabcabc"
[(list 3 1 2) sort_by: { |x| (- x) }] ; -> (3 2 1)
[(list 1 2 2 3 3 3) uniq]          ; -> (1 2 3)
(define add (lambda (a b c) (+ a (+ b c))))
(define add5 [add curry: 5])
(add5 10 20)                        ; -> 35
```

---

---

## Session 5: Better Smalltalk — Phase 0

### Starting point

The v2 runtime had a working Smalltalk-style class hierarchy with metaclasses, message dispatch, and open classes, but several key Smalltalk features were missing: no `super` sends, no class methods, no `doesNotUnderstand:`, no `new`/`initialize` protocol, and no control-flow-as-messages. A plan was designed for both Smalltalk improvements (Phase 0) and a future bytecode VM (Phases 1-5), with benchmarks at each phase boundary.

### Baseline benchmarks

Before any changes, baseline timings were captured:

| Benchmark | Time |
|-----------|------|
| fib(30) | 1850ms |
| map 10k squares | 3.84ms |
| 5k Points create | 7.39ms |
| 5k Point.sum | 4.17ms |
| 5k Shapes create | 10.13ms |
| 5k area matches | 4.20ms |

### What was built

**`super` sends** -- `[super method]` syntax. Parser detects `super` as receiver and emits `(__super-send "method" args...)`. The interpreter binds `__current_class` in method call environments (via `send_message` tracking which class owns the resolved method). `eval_super_send` reads `__current_class`, gets its superclass, and starts method lookup there. Tested with 3-level inheritance chains: `C -> B -> Hello from A`.

**Class methods via metaclass** -- `(classmethod selector (params) body)` inside class definitions. Methods are installed on the metaclass, making them callable on the class-as-Object. Works for new classes and reopened classes. Example: `(class Foo (classmethod greet () "hello"))` then `[Foo greet]`.

**`new`/`initialize` protocol** -- `[ClassName new]` and `[ClassName new arg1 arg2]`. Implemented as a `new` method on `class_class` (inherited by all metaclasses) that allocates an instance with nil-filled fields, sends `[instance initialize ...]`, and returns the instance. Default `initialize` on Object does nothing. **Key fix**: `set!` inside methods now writes back to the object's actual fields (not just the local env copy), and field bindings in methods are now mutable. The old `(ClassName arg1 arg2)` shorthand continues to work.

**`doesNotUnderstand:` hook** -- When method lookup fails and field access fails, `send_message` tries `doesNotUnderstand:` on the receiver's class before raising an error. The message argument is a table `{ selector: "name", args: (...) }`. Infinite recursion avoided by checking that the selector isn't `doesNotUnderstand:` itself. Enables proxy and method-missing patterns.

**Control-flow-as-messages** -- Smalltalk-style block-based control flow:

- `TrueClass`: `ifTrue:` evaluates block, `ifFalse:` returns nil, `ifTrue:ifFalse:` evaluates first block
- `FalseClass`: mirror opposites
- `NilClass`: `ifNil:` evaluates block, `ifNotNil:` returns nil, `ifTrue:ifFalse:` evaluates second block
- `Object`: `ifNil:` returns nil, `ifNotNil:` evaluates block with self
- `Closure`: `value`, `value:`, `value:value:` for evaluation; `whileTrue:` and `whileFalse:` for loops

Note: zero-arg blocks require `{ || body }` syntax since `{ body }` is parsed as a table.

**Bug fix**: `define` bindings were incorrectly marked as immutable (`false`), preventing `set!` from working on them. Fixed to `true` per the language spec.

### Post-Phase 0 benchmarks

| Benchmark | Baseline | Post-Phase 0 | Change |
|-----------|----------|--------------|--------|
| fib(30) | 1850ms | 1868ms | ~same |
| map 10k | 3.84ms | 4.20ms | ~same |
| 5k Points create | 7.39ms | 7.98ms | ~same |
| 5k Point.sum | 4.17ms | 6.32ms | +52% (lookup_owner overhead) |
| 5k Shapes create | 10.13ms | 10.27ms | ~same |
| 5k area matches | 4.20ms | 4.48ms | ~same |

The Point.sum regression is from `lookup_owner` (now returns method + defining class name for super send support) and `current_method_class` save/restore in `send_message`. This overhead will be eliminated by the bytecode VM's inline caching.

### Tested

All six example files pass. New features tested:

```moof
;; super sends
(class A (method greet () "Hello from A"))
(class B (extends A) (method greet () $"B -> \([super greet])"))
(class C (extends B) (method greet () $"C -> \([super greet])"))
[(C) greet]  ;; => "C -> B -> Hello from A"

;; Class methods
(class Foo (classmethod greet () "Hello from Foo class"))
[Foo greet]  ;; => "Hello from Foo class"

;; new/initialize
(class Counter (fields count)
  (method initialize () (set! count 0))
  (method increment () (set! count (+ count 1)))
  (method value () count))
(define c [Counter new])
[c increment] [c increment] [c increment]
[c value]  ;; => 3

;; doesNotUnderstand:
(class Logger (fields prefix)
  (method doesNotUnderstand: (msg)
    $"\([self prefix]): received \([msg at: "selector"])"))
[(Logger "LOG") anything]  ;; => "LOG: received anything"

;; Control flow as messages
[true ifTrue: { || 42 } ifFalse: { || 0 }]  ;; => 42
(define x 5) (define sum 0)
[{ || (> x 0) } whileTrue: { || (do (set! sum (+ sum x)) (set! x (- x 1))) }]
sum  ;; => 15
```

---

---

## Session 5 (continued): Bytecode VM — Phase 1

### What was built

Three new files totaling ~1,700 lines:

**bytecode.rs** (~250 lines) — 31 opcodes covering stack manipulation, constants, variable access (local/global/upvalue), message sends (Send, TailSend, SendSuper), control flow (jumps, conditional jumps), function calls (Call, TailCall, MakeClosure, Return), object construction (MakeList, MakeTable, MakeTableArray), and special ops (StringInterp, Eval). `CompiledFunction` struct holds bytecode, constant pool, upvalue descriptors. `BytecodeBuilder` provides emit/patch helpers.

**compiler.rs** (~500 lines) — Compiles parsed AST (cons lists from parser) into `CompiledFunction` bytecode. Handles: literals, `if`, `do`, `let`, `define`, `set!`, `lambda`/`fn`, `and`, `or`, `cond`, `quote`, `__send`, `__str-interp`, `__table`, `__table-array`. Uncompiled forms (`class`, `type`, `match`, `try`, `trait`, `protocol`, `module`, `use`, `require`, `defmacro`, `quasiquote`, `__super-send`) fall through to an `Eval` opcode that calls the tree-walker. Macros are expanded at compile time via tree-walker.

**vm.rs** (~450 lines) — Stack-based VM with call frame stack. Main `run()` loop dispatches on opcodes. `dispatch_call()` handles bytecode closures (push new frame), native closures (call directly), and expr closures (fall back to tree-walker). `dispatch_send()` delegates to `interp.send_message()`. Standalone `execute_bytecode()` function enables tree-walker↔bytecode interop — when a native method like `map:` invokes a bytecode-compiled block, it works.

**Key design**: `ClosureBody::Bytecode(Rc<CompiledFunction>)` added to the Value enum. Bytecode closures coexist with tree-walker closures. The VM calls native builtins directly and falls back to the tree-walker for uncompiled features. The `--bytecode` / `-b` CLI flag selects the bytecode path.

### Phase 1 benchmarks

| Benchmark | Baseline (tree-walk) | Phase 1 (bytecode VM) | Speedup |
|-----------|---------------------|----------------------|---------|
| fib(30) | 1850ms | 1299ms | **30% faster** |
| map 10k | 3.84ms | 2.73ms | **29% faster** |

Remaining benchmarks (Points, Shapes) crash on stack overflow because TCO is not yet implemented in the VM — deferred to Phase 2.

### Phase 2: TCO + Inline Caching + Match

**TailCall frame reuse** -- `TailCall` opcode now overwrites the current frame's locals with new args and resets IP to 0, instead of pushing a new frame. No stack growth for tail-recursive functions. Works in both the main VM loop and the `execute_bytecode` interop path. `(loop 1000000)` completes without stack overflow.

**Monomorphic inline caching** -- 1024-slot cache keyed by bytecode offset. Each `Send` instruction checks: does the receiver's class pointer match the cached entry? If yes, use the cached method directly (skipping the superclass chain walk). On miss, do full lookup and update the cache. `dispatch_send_cached` calls the method via `call_closure` on cache hit, bypassing `send_message`.

**Match compilation** -- `match` expressions can't use the simple `Eval` fallback because the scrutinee is on the bytecode stack, invisible to the tree-walker's global env. Solution: compile the scrutinee, store it in a well-known global (`__vm_match_scrutinee`), reconstruct the match form with that global as the scrutinee, and Eval it. This bridges bytecode values into the tree-walker for pattern matching.

**Call dispatch optimization** -- Bytecode-to-bytecode `Call` now rearranges the stack in-place (shifts args down over the callee slot) instead of collecting args into a `Vec` and pushing them back.

### Final benchmark comparison (median of 3 runs)

| Benchmark | Tree-walker | Bytecode VM | Speedup |
|-----------|-----------|-------------|---------|
| fib(30) | 2157ms | **1371ms** | **36% faster** |
| map 10k | 3.81ms | 4.30ms | -13% (interop overhead) |
| 5k Points create | 7.42ms | **5.47ms** | **26% faster** |
| 5k Point.sum | 6.27ms | **5.63ms** | **10% faster** |
| 5k Shapes create | 10.57ms | **7.95ms** | **25% faster** |
| 5k area matches | 4.61ms | **4.53ms** | **2% faster** |

The map 10k regression is due to the `execute_bytecode` interop path: when native `map:` invokes a compiled block, the mini-executor has overhead from being a flat function (no persistent frame stack). This will improve when more of the stdlib is compiled to bytecode.

### Phase 3: Upvalue Capture

Closures that capture outer variables now work in the bytecode VM. The compiler walks enclosing scopes to find captured variables, marks them as `is_captured`, and emits `GetUpvalue`/`SetUpvalue` opcodes. `MakeClosure` bytecode carries upvalue descriptors that the VM reads to capture values from the enclosing frame. Upvalues are `Rc<RefCell<Value>>` cells, enabling mutable shared state between closures.

Key bug found and fixed: `dispatch_tail_call` was reusing frames without updating the `upvalues` field, causing panics when tail-calling closures with captured variables.

```moof
;; Mutable closure state works
(define (make-counter)
  (define count 0)
  (lambda ()
    (set! count (+ count 1))
    count))
(define c (make-counter))
(c) ; => 1
(c) ; => 2
(c) ; => 3
```

### Finishing Touches: try/catch + Interop Fix

**try/catch compilation** -- `try` is compiled as `__vm_try(body_thunk, catch_thunk)` where both thunks are bytecode lambdas that capture outer variables via upvalues. The `__vm_try` native function calls the body thunk, and on error, calls the catch thunk with the error value. This fixed `(try (/ a b) (catch e "error"))` inside functions where `a` and `b` are parameters.

**execute_bytecode upvalue fix** -- The standalone bytecode executor (used when native code like `map:` or `__vm_try` invokes a compiled block) now receives and uses the closure's upvalues. This fixed the map 10k regression and all cases where bytecode closures called from native code needed to access captured outer variables. MakeClosure in the mini-executor now also properly captures upvalues from the enclosing scope.

### Final benchmark (median of 3 runs)

| Benchmark | Tree-walker | Bytecode VM | Speedup |
|-----------|-----------|-------------|---------|
| fib(30) | 2248ms | **1520ms** | **32%** |
| map 10k | 4.32ms | **3.06ms** | **29%** |
| 5k Points create | 9.83ms | **5.86ms** | **40%** |
| 5k Point.sum | 7.23ms | **6.27ms** | **13%** |
| 5k Shapes create | 12.00ms | **9.22ms** | **23%** |
| 5k area matches | 4.74ms | 5.18ms | -9% (match→Eval bridge overhead) |

All 6 example files produce identical output between tree-walker and bytecode VM.

---

## Session 6: Philosophy Rework & Self-Hosting

### Starting point

Moof had grown into a working language with a bytecode VM, but felt like "Lisp with Smalltalk bolted on." The interpreter had 26 special forms. `builtins.rs` was 2,556 lines of Rust-registered methods — many trivial (`nil?` returning false, `even?` doing `% 2`, `map:` iterating a list). The stdlib was 311 lines embedded via `include_str!`. The session's goal was to establish a coherent philosophy and self-host as much as possible in Moof.

### The Moof Philosophy (established this session)

1. **Everything is an object.** Numbers, strings, closures, classes, modules.
2. **Everything is a message.** `(f x y)` means "send `call:` to f." Control flow is messages to booleans and blocks.
3. **The evaluator is minimal.** ~12 true special forms, not 26.
4. **Rust provides primitives, Moof provides methods.** A small `__primitive` FFI exposes ~50 raw operations. The stdlib wraps these into the full object protocol.
5. **The stdlib is Moof files on disk.** Loaded via a real import system.

### What was built

**Phase 1 — Primitive FFI System** (`primitives.rs`, 990 lines)

New `__primitive` special form that dispatches to a `HashMap<SymId, NativeFn>` registry on the Interpreter. ~50 primitives registered covering: polymorphic numeric ops (`num_add`, `num_sqrt`), integer-specific (`int_gcd`, `int_bit_and`), float-specific (`float_round`, `float_nan`), string ops (`str_length`, `str_upper`, `str_split`), cons/table ops, object introspection, symbol ops, I/O, and range field access. Numeric helpers (`numeric_add`, `numeric_sub`, etc.) are shared between primitives and the variadic global functions — no duplication.

```moof
(__primitive num_add 1 2)      ;; => 3
(__primitive str_upper "hello") ;; => "HELLO"
```

**Phase 2 — Import System Overhaul**

Rewrote `require` with:
- **Path resolution**: relative to current file's directory, then CWD, then `MOOF_PATH` entries, then `stdlib_dir`
- **Extension inference**: `(require "core")` finds `core.moof`
- **Module caching**: `loaded_modules: HashMap<PathBuf, Value>` — each file eval'd once
- **Circular detection**: `loading_stack: Vec<PathBuf>` — error if a file is already being loaded
- **Current file tracking**: `current_file: Option<PathBuf>` — enables relative require from within loaded files

`load_prelude()` finds the stdlib directory via `MOOF_STDLIB` env var, CWD, or binary-relative paths, and loads `stdlib/prelude.moof`.

**Phase 3 — External Stdlib** (13 .moof files, 932 lines)

Removed `include_str!` embedding. Created modular stdlib:

```
stdlib/
  prelude.moof      — boot entry point, requires everything in order
  core.moof         — Object base methods (to_s, nil?, ==, hash)
  bool.moof         — TrueClass/FalseClass control flow + NilClass (list terminators)
  numeric.moof      — Integer + Float methods via __primitive
  string.moof       — String methods via __primitive
  collections.moof  — Cons + Table methods (many pure Moof, some via __primitive)
  closure.moof      — whileTrue:, whileFalse: (pure Moof, recursive)
  range.moof        — Range iteration via __primitive field access
  error.moof        — placeholder (Error methods need field access, stay in Rust)
  symbol.moof       — Symbol methods via __primitive
  functional.moof   — map, filter, reduce, compose, pipe, type checks, etc.
  math.moof         — pi, e, factorial, power, trig conversions
  adt.moof          — Option, Result, Pair ADTs
  macros.moof       — when, unless, protocols
```

**Boot order matters**: NilClass must load before Cons because `[[self cdr] map: f]` terminates at nil — NilClass needs `map:` returning `(list)`, `filter:` returning `(list)`, `each:` returning nil, etc.

**Key fix — Bootstrap class registration**: All 22 bootstrap classes (Object, Integer, String, etc.) are now registered as named bindings in the global env during `Interpreter::new()`. Without this, `(class Object ...)` in stdlib files created a NEW Object class instead of reopening the bootstrap one.

**Key fix — Multi-keyword selector parsing**: Added `parse_method_selector()` to merge consecutive colon-terminated symbols in method definitions. The parser splits `replace_all:with:` into two tokens; the new method merges them before registering. This enabled multi-keyword methods like `(method replace_all:with: (from to) ...)` in .moof files.

**builtins.rs shrunk from 2,556 → 540 lines** — kept only: variadic global functions (+, -, cons, print, etc.), Class.new, Object introspection (class, is_a:, responds_to:, send:), Closure invoke (value, value:, call:, curry:), and Error field access (message, to_s).

**Phase 4 — Callable Protocol**

Any object responding to `call:` can now be called with `(obj args...)`. When `invoke()` encounters a non-closure, non-metaclass value, it checks if the object's class has a `call:` method and dispatches `[obj call: args-as-list]`. ~15 lines in the evaluator.

```moof
(class Counter (fields count)
  (method initialize (n) (set! count n))
  (method call: (args) (set! count (+ count 1)) count))
(define c [Counter new 0])
(c) ;; => 1
(c) ;; => 2
(c) ;; => 3
```

**Phase 5 — Macro-ization** (deferred)

Investigated converting `cond`, `type`, `protocol`, `trait` from special forms to macros. Found that `type` needs class creation + type registry, `protocol`/`trait` need interpreter registries, and `cond` would lose TCO. These need a proper meta-object protocol before they can be macro-ized. Kept as special forms.

### LOC impact

| Component | Before | After | Change |
|-----------|--------|-------|--------|
| `builtins.rs` | 2,556 | 540 | **-79%** |
| `primitives.rs` | — | 990 | New |
| `interpreter.rs` | 2,632 | 2,942 | +310 (import system, bootstrap, selectors) |
| `stdlib/*.moof` | 311 (1 embedded file) | 932 (13 external files) | Self-hosted |
| **Net Rust** | **~5,200** | **~4,470** | **-14%** |

### Tested

All 21 unit tests pass. All 6 example files produce identical output. New features tested:

```moof
;; Primitives
(__primitive num_add 1 2)          ;; => 3
(__primitive str_upper "hello")    ;; => "HELLO"

;; Import system
(require "stdlib/core")            ;; loads from disk, cached
(require "/tmp/test.moof")         ;; absolute path
(require "mylib")                  ;; .moof inferred, circular detection

;; Self-hosted methods
[3 even?]                          ;; => false (defined in stdlib/numeric.moof)
["hello" uppercase]                ;; => "HELLO" (defined in stdlib/string.moof)
[(list 3 1 2) sort]                ;; => (1 2 3) (pure Moof insertion sort)
[{a: 1 b: 2} keys]                ;; => (a b) (via __primitive table_keys)

;; Callable protocol
(class Adder (fields n)
  (method initialize (x) (set! n x))
  (method call: (args) (+ n (car args))))
(define add5 [Adder new 5])
(add5 10)                          ;; => 15
```

### Classes as First-Class Objects (Session 6 continued)

Classes were identified by strings everywhere — `[3 class]` returned `"Integer"`, `is_a:` took string arguments, type predicates compared strings. This was reworked so classes are proper first-class objects.

**class_objects registry** — `HashMap<SymId, Value>` on Interpreter maps class name → the `Value::Object` representing that class. Both `register_bootstrap_classes()` and `eval_class()` populate it. New `class_object_of(val)` method returns the class object for any value.

**`.class` returns the class object** — `[3 class]` now returns the Integer class object (not `"Integer"`). Class objects have `name`, `to_s`, `superclass`, and `methods` methods via `class_class`. `real_class_from_class_object()` resolves class objects back to `Rc<RefCell<MoofClass>>` by pointer identity in the registry, avoiding the fragile " meta" suffix stripping.

**`is_a:` accepts class objects** — `[3 is_a: Integer]`, `[3 is_a: Numeric]`, `[3 is_a: Object]` all work. Still accepts strings for backwards compatibility.

**`type-of` returns class objects** — `(= (type-of 3) Integer)` → `true`.

**Type predicates rewritten** — `(integer? x)` is now `[x is_a: Integer]` instead of string comparison.

**Anonymous classes** — `(class (fields x y) (method ...))` without a name returns the class object directly. Gets an auto-generated internal name like `<anon-class-22>`.

```moof
;; Classes are objects
(= [3 class] Integer)           ;; => true
[[3 class] name]                ;; => "Integer"
[Integer superclass]            ;; => Numeric (the class object)
(= [Integer superclass] Numeric) ;; => true
[3 is_a: Integer]               ;; => true
[3 is_a: Numeric]               ;; => true

;; Pass classes as values
(define MyClass Integer)
[3 is_a: MyClass]               ;; => true

;; Anonymous classes
(define Point (class (fields x y)
  (method to_s () (format "Point(~a, ~a)" x y))))
(define p (Point 3 4))
[p to_s]                        ;; => "Point(3, 4)"
```

---

## Session 7: Image-Based REPL

### Starting point

Moof had classes as first-class objects, a primitive FFI, external stdlib, and callable protocol. The next step: make the REPL a true development environment where you experiment live and save the running state.

### Philosophy

The REPL IS the development environment. Since Moof is homoiconic, the cons list IS the source — no source strings needed. A pretty-printer reconstructs readable Moof from the AST. An "image" is just a .moof file that recreates the running state when loaded.

### What was built

**Pretty-printer** (`src/pretty.rs`, ~330 lines) — Reverses the parser's desugaring to produce readable Moof source from cons list ASTs:
- `(__send receiver "selector" args)` → `[receiver selector args]`
- `(__send obj "key:val:" a b)` → `[obj key: a val: b]`
- `(lambda (x) body)` → `{ |x| body }`
- `(__table "k" v)` → `{k: v}`
- `(__table-array v1 v2)` → `{v1, v2}`
- `(__str-interp parts)` → `$"...\(expr)..."`
- `(quote x)` → `'x`, `(quasiquote x)` → `` `x ``
- `(define name (lambda (x) body))` → `(define (name x) body)`
- Multi-line formatting with 2-space indentation for long expressions

**Baseline snapshot** — `snapshot_baseline()` captures interpreter state after stdlib loads: method sets, global bindings, macros, types, protocols. Everything present at baseline is "stdlib"; anything added later is "user code."

**Image serialization** (`src/image.rs`, ~300 lines) — `save_image()` walks interpreter state and emits a single .moof file containing only user-defined content:
- User-defined classes (with fields, methods, superclass)
- Methods added to bootstrap classes (e.g., `factorial` on Integer)
- Global functions and variables
- Macros, ADTs, protocols

**REPL `,save` command** — `,save [path]` saves the current image. Default path: `image.moof`. Also available as `(save-image "path")` from code.

**`pretty_print` primitive** — `(__primitive pretty_print expr)` pretty-prints any expression from Moof code.

### Round-trip test

```moof
;; In the REPL, define things:
(define (my-fib n) (if (<= n 1) n (+ (my-fib (- n 1)) (my-fib (- n 2)))))
(class Dog (fields name breed)
  (method to_s () (format "~a the ~a" name breed)))
(class Integer
  (method factorial () (if (<= self 1) 1 (* self [(- self 1) factorial]))))

;; Save the image
,save mywork.moof

;; Start a new REPL, load the image:
(require "mywork.moof")
(my-fib 10)              ;; => 55
[(Dog "Rex" "Lab") to_s] ;; => "Rex the Lab"
[5 factorial]            ;; => 120
```

### DRY Features (Session 7 continued)

**Auto-generated field reader methods** — `(class Point (fields x y))` automatically generates `[p x]` and `[p y]` reader methods. No need to write boilerplate accessors.

**`delegates-to` directive** — `(class Proxy (fields target) (delegates-to target))` generates a `doesNotUnderstand:` method that forwards all unknown messages to the target field. One-line proxy/delegation pattern.

**Default parameter values** — `(define (connect host (port 8080) (timeout 30)) ...)`. Optional params with defaults after required params. Parser, interpreter, and arity check all updated. Defaults evaluated eagerly at definition time.

**`with-fields` macro** — `(with-fields (x y) point body)` binds field values to local variables.

**`ComparableDefaults` trait** — Define `>` on a class, `(uses ComparableDefaults)` gives you `<`, `>=`, `<=`, `min:`, `max:` for free.

### Condition/Restart System (Session 7 continued)

Common Lisp-style condition/restart system. Errors become conversations, not crashes.

**Core forms:**
- `(signal condition)` — signal a condition through the handler stack without unwinding
- `(handler-bind ((Type handler) ...) body)` — dynamically bind handlers for condition types
- `(restart-case expr (name (params) body) ...)` — offer named restart points
- `(invoke-restart name args...)` — invoke a restart from within a handler

**Key design:** `restart-case` integrates with regular errors, not just `(signal ...)`. When an error occurs inside `restart-case`, it runs the error through the handler stack before unwinding. This lets handlers invoke restarts even for regular `(error "...")` calls.

```moof
;; Safe division with recovery options
(define (safe-div a b)
  (restart-case (/ a b)
    (use-value (v) v)
    (retry-with (new-b) (/ a new-b))))

;; Handler: use 0 on division error
(handler-bind ((Error { |e| (invoke-restart use-value 0) }))
  (safe-div 10 0))  ;; => 0

;; Handler: retry with different divisor
(handler-bind ((Error { |e| (invoke-restart retry-with 2) }))
  (safe-div 10 0))  ;; => 5
```

---

## Final state

The codebase at the end of these sessions:

| File | Lines | Purpose |
|------|-------|---------|
| `src/interpreter.rs` | 3,096 | Tree-walking evaluator, class system, macros, modules, import system, baseline snapshot |
| `src/repl.rs` | 1,232 | 16 meta-commands (incl. `,save`), tab completion, inspect via message sends |
| `src/primitives.rs` | 998 | Primitive FFI registry (~50 raw operations + pretty_print) |
| `src/compiler.rs` | 927 | AST → bytecode compiler, upvalue resolution |
| `src/vm.rs` | 732 | Stack-based bytecode VM, inline caching, TCO |
| `src/parser.rs` | 701 | Recursive descent, all desugaring inline |
| `src/builtins.rs` | 604 | Variadic globals, Class introspection, Object/Closure/Error intrinsics |
| `src/moofint.rs` | 600 | BigInt with auto-promotion |
| `src/value.rs` | 560 | 11-variant Value enum, Range, display, hashing |
| `src/pretty.rs` | 469 | Pretty-printer: cons list AST → readable Moof source |
| `src/image.rs` | 369 | Image serialization: save running state as .moof file |
| `src/lexer.rs` | 272 | Tokenizer |
| `src/bytecode.rs` | 259 | Op enum (31 opcodes), CompiledFunction, builder |
| `src/symbol.rs` | 207 | Symbol table, pre-interned known symbols |
| `src/main.rs` | 167 | CLI entry point (`--bytecode` flag) |
| `src/error.rs` | 152 | Error type with class hierarchy |
| `src/cons.rs` | 114 | Cons cells, list iteration |
| `src/environment.rs` | 95 | Scoped environments with SymId keys |
| `src/token.rs` | 48 | Token enum |
| `src/lib.rs` | 18 | Module declarations |
| `stdlib/*.moof` | 931 | External self-hosting standard library (13 files) |
| **Total** | **~12,551** | |

The language went from spec to Ruby prototype to Rust rewrite to Smalltalk-inspired VM to bytecode compiler to self-hosting philosophy rework to image-based REPL in seven sessions. The running system IS the program — experiment in the REPL, `,save` your work, pick up where you left off.
