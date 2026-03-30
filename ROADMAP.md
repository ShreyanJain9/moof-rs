# Moof Roadmap: Prototype to Production

*Synthesized from three independent reviews (Claude, Codex, Gemini). Items marked with consensus strength: [3/3] unanimous, [2/3] strong, [1/3] explore.*

---

## Guiding Principle

> Moof isn't just "Lisp with brackets." It's a language where code, messages, and live objects are all equally composable. Every feature should amplify the `()` + `[]` duality into something greater than either alone.

---

## Phase 2.5: Must-Have Before Rust Port

These shape the VM's execution model, memory layout, and bytecode design. Changing them after Rust means rewriting the VM.

### 1. Pattern Matching + Destructuring [3/3]

The single most impactful missing feature. Affects ADTs, error handling, control flow bytecodes, and future type system.

```moof
(match value
  (42                 "the answer")
  ((Point x y)        (format "(~a, ~a)" x y))
  ((list a b . rest)  (format "starts with ~a ~a" a b))
  ({name: n age: a}   (format "~a is ~a" n a))
  (_                  "something else"))

; Guards
(match point
  ((Point x y) when (> x 0)  "right half")
  ((Point x y)                "left half"))
```

Destructuring in `let`:
```moof
(let (((Point x y) my-point)
      ({name: n} user)
      ((head . tail) my-list))
  ...)
```

**Why before Rust:** Pattern compilation is a core VM operation. The match compiler generates decision trees that become bytecode instructions.

### 2. Module System [3/3]

Affects compilation units, symbol tables, bytecode file format, and how the linker works.

```moof
(module geometry
  (export Point Circle area)

  (class Point (fields x y))

  (define (area shape)
    (match shape
      ((Circle r) (* pi (* r r))))))

; Importing
(use geometry)                    ; import all exports
(use geometry (Point area))       ; selective import
(use geometry :as geo)            ; qualified: (geo.area shape)
```

**Design decisions to lock:**
- Module = file, or explicit `(module ...)` blocks?
- Qualified access syntax: `geo.sin`, `geo:sin`, or `[geo sin]`?
- How do open classes interact across modules?

**Why before Rust:** Module boundaries determine compilation unit isolation, caching, and incremental compilation.

### 3. Protocols (distinct from Traits) [3/3]

Traits = code reuse (method bodies). Protocols = behavioral contracts (selector sets). These are different things.

```moof
(protocol Measurable
  area
  perimeter)

(protocol Sequence
  length
  at:
  map:
  filter:)

; Check at runtime
(implements? my-shape Measurable)  ; => true/false

; Use in type annotations (future)
(: total-area (-> (List (Obj Measurable)) Float))
```

**Why before Rust:** Protocols determine the dispatch table design and how inline caches work. They're also the bridge between the dynamic object system and the static type system (`Obj[selectors...]` from the spec).

### 4. First-Class Selectors / Reified Messages [3/3]

This is where Moof's duality becomes genuinely unique. Messages as composable values.

```moof
; Selector as a value — usable as a function
(map &length strings)              ; => (5 3 7 ...)
(map &name users)                  ; like (map (lambda (u) [u name]) users)

; Keyword selectors too
(map &(replaceAll: "a" with: "b") strings)

; Method references
(define greeter (method-ref dog speak))
(greeter)                          ; => "Rex says woof!"
```

**Why before Rust:** Requires selector interning (atoms, not strings) in the VM. Affects the calling convention and how inline caches are keyed.

### 5. Pipeline / Threading Macro [3/3]

Makes the dual syntax actually sing together instead of creating nesting hell.

```moof
(-> data
    [filter: active?]
    [map: normalize]
    (group-by category)
    [sortBy: score]
    [take: 10])
```

`->` threads the result of each step as the receiver of the next `[]` or the first arg of the next `()`.

**Why before Rust:** This is a macro/special-form that the compiler needs to understand for optimization. Get the semantics right in Ruby.

### 6. Quasiquote + Basic Macros [2/3]

Macros determine AST shape, phase separation, and how the compiler works. Don't need full hygienic macros yet, but need the foundation.

```moof
(defmacro unless (cond body)
  `(if (not ,cond) ,body nil))

(defmacro when (cond . body)
  `(if ,cond (do ,@body) nil))

(unless false (print "yes"))       ; expands to (if (not false) (print "yes") nil)
```

**Key decision:** Macros operate on the normalized AST (after `[]` lowering), but source-form metadata is preserved for tooling and diagnostics.

**Why before Rust:** The compiler's IR and phase model depend entirely on how macros work.

### 7. Tail Call Optimization [3/3]

A Lisp without TCO is a broken Lisp. Must be a semantic guarantee, not an optional optimization.

```moof
(define (loop n acc)
  (if (= n 0) acc
    (loop (- n 1) (+ acc n))))

(loop 1000000 0)                   ; must not stack overflow
```

**Why before Rust:** TCO affects whether the VM is register-based with explicit tail-call instructions or stack-based with trampolining.

### 8. Non-Local Control Flow [2/3]

Decide now: exceptions only, or also named blocks and guaranteed cleanup?

```moof
(block done
  (for-each (lambda (x)
    (when (= x target)
      (return-from done x)))
    items)
  nil)

(unwind-protect
  (dangerous-work)
  (cleanup))   ; always runs, even on exception
```

**Why before Rust:** Shapes stack frames, unwinding semantics, closure implementation, and optimizer assumptions.

---

## Phase 2.5b: High-Value Quality of Life

These make Moof delightful and inform what the Rust runtime needs to support.

### 9. Block / Short Lambda Syntax [2/3]

`(lambda (x) ...)` is too verbose for inline use with `[]`.

```moof
; Proposed: { |args| body }
[nums map: { |x| (* x x) }]
[nums filter: { |x| (> x 0) }]
[nums reduce: { |acc x| (+ acc x) } init: 0]

; Shorthand for single-arg blocks
[nums map: &square]               ; selector-as-function (from #4)
```

### 10. String Interpolation [2/3]

```moof
$"Hello \([user name]), you have \([inbox count]) messages"
$"2 + 2 = \((+ 2 2))"
```

Desugars to `format` calls. `$"..."` prefix distinguishes interpolated strings from regular ones.

### 11. Keyword Arguments for `()` Functions [2/3]

Currently only `[]` has keywords. Unify:

```moof
(define (connect host port: 8080 timeout: 30 tls: true)
  ...)

(connect "example.com" port: 443 tls: false)
```

**Key design rule (Codex):** For `[]`, labels are part of selector identity (`insertValue:forKey:`). For `()`, keywords are named parameter binding metadata. Don't conflate the two.

### 12. Error Suggestions [2/3]

```
Error: String does not respond to 'upppercase'
  Did you mean: uppercase?

Error: Undefined variable: lenght at line 3:5
  Did you mean: length?
```

Levenshtein distance on selector tables and environment bindings. Dynamic languages earn trust through diagnostics.

### 13. Documentation Strings + Metadata [1/3]

```moof
(define (square x)
  "Returns the square of x"
  (* x x))

; Or Clojure-style metadata
(define ^{:doc "Square a number" :since "0.3"} square
  (lambda (x) (* x x)))
```

Accessible via `,doc square` in the REPL. Rust runtime should preserve this metadata.

### 14. File I/O [3/3]

Practical necessity for real scripts:

```moof
(define contents (read-file "data.txt"))
(write-file "out.txt" "hello world")
(file-exists? "data.txt")          ; => true
(define lines (lines (read-file "data.txt")))
```

---

## Phase 3: Signature Features

These make Moof genuinely unique — not just another Lisp.

### 15. The Selector-Function Bridge [2/3] (Gemini's killer idea)

Quoted symbols as getter functions:

```moof
(map 'name users)
; equivalent to:
(map (lambda (u) [u name]) users)
```

This makes Moof the ultimate "data scripting" language. It bridges functional Lisp with OO message passing in one clean gesture.

### 16. Protocol-Oriented Structural Typing [3/3]

The `Obj[selectors...]` idea from the spec, made real:

```moof
(protocol Drawable
  draw
  color:)

; Any class that implements these methods auto-conforms — duck typing with a name
(define (render shape)
  (assert (implements? shape Drawable))
  [shape color: "red"]
  [shape draw])
```

### 17. Message Cascading [2/3]

Smalltalk's cascade, adapted for Moof:

```moof
(define canvas
  (-> [Canvas new]
      [; setWidth: 800]
      [; setHeight: 600]
      [; setBackground: "white"]))
```

The `;` inside `[]` means "send this message, but return the receiver, not the result."

### 18. Implicit Self Shorthand [1/3] (Gemini)

Inside methods, `.name` as sugar for `[self name]`:

```moof
(method area []
  (* pi (* (.radius) (.radius))))

; Instead of:
(method area []
  (* pi (* [self radius] [self radius])))
```

### 19. Live Introspection [2/3]

Source code and runtime equally inspectable:

```moof
[Point methodSource: "length"]     ; => the source code as a string
(source-of square)                 ; => the define form
(ast-of '(+ 1 2))                 ; => the AST representation
,who-added slug                    ; REPL: which module/file added this method?
```

---

## Explicitly DEFER to Rust [3/3 agreement]

| Feature | Why defer |
|---------|-----------|
| Bytecode VM + optimizer | Ruby tree-walker is fine for semantics; real perf is Rust work |
| Concurrency / actors | Needs real lightweight processes, not Ruby threads |
| GC strategy | Ruby's GC is fine; Rust forces the real decisions (generational, NaN boxing, etc.) |
| JIT / inline caches | Method dispatch optimization is the Rust VM's job |
| Serious FFI | Ruby FFI would be throwaway; design the API shape only |
| Full static typechecker | Design the type model now, build the checker in Rust |
| Package distribution | Irrelevant until core semantics are settled |

---

## Critical Design Decisions to Lock Down [Before Any Rust]

| # | Decision | Recommendation | Impact |
|---|----------|----------------|--------|
| 1 | **Value representation** | Tagged pointers: `00` heap, `01` SmallInt, `10` constants, `11` float | Every VM instruction |
| 2 | **Selector identity** | Interned atoms (not strings). Source spelling preserved separately for diagnostics | Dispatch, caches, reflection |
| 3 | **Classes vs Traits vs Protocols** | Classes = state + methods. Traits = code reuse. Protocols = behavioral contracts. Don't conflate. | Type system, dispatch tables |
| 4 | **Open class semantics** | Always mutable, but explicit `(extend ClassName ...)` syntax. Cache invalidation on change. | Optimizer, module loading |
| 5 | **Core IR** | Surface AST retains syntax identity + spans. Core IR lowers `()` and `[]` into one form. Macros operate on syntax objects. | Compiler architecture |
| 6 | **TCO** | Semantic guarantee, not optimization. VM must support it in the instruction loop. | Stack frames, bytecode design |
| 7 | **Keyword args in `()` vs `[]`** | `[]`: labels are selector identity. `()`: keywords are named param binding. Different mechanisms. | Call frames, dispatch, ABI |
| 8 | **Mutation story** | Bindings: `define` mutable, `let` immutable. Fields: mutable via message. Consider immutable-by-default for fields in future. | Optimizer, type system |
| 9 | **Equality** | `=` structural (deep, recursive). `eq?` identity. Functions compare by identity only. User classes override via `equals:`. Total equality on all types. | Hashing, pattern matching, stdlib |
| 10 | **Nil behavior** | `[nil foo]` raises MessageError (no nil-punning). Explicit `nil?` check required. | Error model, type safety |
| 11 | **Runtime target** | Design for native Rust VM first. Keep semantics BEAM-compatible but don't contort for it. | Everything |

---

## Implementation Sequence

```
Phase 2.5a — Core Semantics (prototype in Ruby)
  1. Pattern matching + destructuring
  2. Tail call optimization
  3. Protocols (distinct from traits)
  4. Module system
  5. First-class selectors / &name syntax
  6. Pipeline operator (->)
  7. Quasiquote + basic macros

Phase 2.5b — Ergonomics (prototype in Ruby)
  8. Block syntax { |x| ... }
  9. String interpolation $"...\(...)"
  10. Keyword args for () functions
  11. Non-local control flow (block/return-from)
  12. Error suggestions (did-you-mean)
  13. File I/O
  14. Docstrings

Phase 3 — Lock Down Design Decisions
  15. Formalize value representation
  16. Formalize selector interning
  17. Formalize class/trait/protocol split
  18. Write the Moof Language Specification v1.0
  19. Write example programs that exercise every feature

Phase 4 — Rust Port
  20. Bytecode ISA design
  21. Register-based VM with TCO
  22. Garbage collector (likely generational + NaN boxing)
  23. Selector interning + inline caches
  24. Module loader + bytecode serialization
  25. Actor runtime (lightweight processes)
  26. FFI layer
```

---

## The One-Line Vision

**Moof is where Lisp's composability meets Smalltalk's live objects, and both feel like they were always meant to be together.**

Everything on this roadmap should serve that sentence. If a feature doesn't make the `()` + `[]` duality stronger, it doesn't belong.
