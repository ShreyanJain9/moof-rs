# Moof — Improvements & Next Steps

A review of the current Phase 1 prototype and a prioritized roadmap for what to build next. The prototype is solid — 209 tests passing, clean architecture, and a working end-to-end pipeline. These suggestions are about making Moof feel like a *real language* rather than a proof-of-concept.

---

## Phase 1 Polish (low-hanging fruit, high impact)

These are improvements to what already exists. They'll make the prototype dramatically more pleasant to use before tackling the bigger Phase 2 features.

### 1. Better Error Messages with Source Locations

The AST nodes define a `Node` base class with `line` and `column` fields, but none of the actual node types inherit from it — they're all standalone `Data.define` calls. This means runtime errors can't point to where in the source they originated.

**What to do:**
- Add `line` and `column` fields to every AST node (or at least `Call`, `Identifier`, `MessageSend`, `SetBang`)
- Thread source locations through the lexer → parser → normalizer pipeline
- Include them in `RuntimeError`, `NameError`, `MessageError` messages
- Drop the orphaned `Node` base class — it currently does nothing

This is the single highest-value improvement. "Undefined variable: x" is frustrating; "Undefined variable 'x' at line 7, column 3 in fibonacci.moof" is actually useful.

### 2. Tail Call Optimization

Recursive Moof programs (like `fibonacci.moof`) will blow the Ruby call stack on large inputs. A basic TCO for self-tail-calls would fix the most common case.

**What to do:**
- Detect when the last expression in a function body is a call to itself
- Convert to a loop internally in `Function#call`
- This doesn't require changing the language — it's an interpreter optimization

### 3. Variadic Functions (Rest Parameters)

There's no way to write a function that accepts a variable number of arguments. This limits what you can do in Moof itself.

**What to do:**
- Support a syntax like `(lambda (a b . rest) ...)` or `(lambda (a b &rest) ...)`
- Bind the rest parameter to a list of remaining arguments
- Make `DefineFunction` support the same

### 4. `and` / `or` Should Be Short-Circuiting Special Forms

Currently `and` and `or` are builtins, which means both arguments are evaluated eagerly. This is wrong — `(and false (explode))` should not call `explode`.

**What to do:**
- Parse `and` and `or` as special forms (like `if`)
- Evaluate them lazily in the interpreter

### 5. Closures Over Mutable State

`set!` creates surprising behavior with closures. If a closure captures a variable and it gets mutated via `set!`, the closure should see the new value (and currently does, since `Environment` stores by reference). But there are edge cases worth testing — especially around `let` bindings vs `define` bindings inside closures.

**What to do:**
- Add integration tests specifically for closure-over-mutation scenarios
- Verify that `let` immutability is enforced even from within a closure

### 6. `map`, `filter`, `reduce` as First-Class Builtins

These exist on lists as messages (`[list map: f]`, `[list filter: f]`), but not as standalone functions. Lisp programmers expect `(map f list)`.

**What to do:**
- Add `map`, `filter`, `reduce`/`fold` to `builtins.rb`
- These should work with both `Function` and `Proc` callables

### 7. REPL Quality of Life

The REPL works but could feel a lot more polished:

- **Multiline editing**: The `balanced?` check is good, but unclosed strings can hang the REPL
- **History persistence**: Save readline history to `~/.moof_history`
- **,type command**: Show the runtime type of a value (number, string, list, etc.)
- **Error context**: Show the failing expression when an error occurs, not just the error message
- **Colorized output**: Use ANSI colors for `=>` results, errors, and the banner

### 8. String Interpolation

The spec defers this, but `format` with `~a` is clunky. A lightweight interpolation syntax would make strings much more natural.

**Suggested syntax**: `"hello \(name), you are \(age) years old"` — Swift-style, plays well with existing escape handling.

**What to do:**
- Extend the lexer to recognize `\(` inside strings
- Parse interpolated segments as expressions
- Desugar to `format` calls (or a `concat` chain) in the normalizer

---

## Phase 2 — The Object System

This is where Moof goes from "neat scripting toy" to "language with real modeling power." The spec defines this clearly, so the roadmap is about *build order*.

### 9. Class Definitions

The headliner feature. Without user-defined classes, `[]` message passing is only useful for built-in types.

**What to do (in order):**
1. **AST nodes**: `ClassDef(name, fields, methods, traits)`, `MethodDef(name, params, body)`
2. **Parser**: Handle `(class Name (fields ...) (method ...) ...)` syntax
3. **Runtime representation**: A `MoofClass` object that holds field names and a method table (hash of selector → `Function`)
4. **Instance creation**: Calling the class name as a function constructs an instance, e.g. `(Point 3 4)`
5. **Instance representation**: A `MoofObject` with a class pointer and a field-value hash
6. **Dispatcher integration**: When the receiver is a `MoofObject`, look up the selector on its class's method table
7. **`self` binding**: Methods execute with `self` bound to the receiver; field access via `[self fieldName]`

### 10. Inheritance

Single inheritance is enough to start. Keep it simple.

**What to do:**
- Add an optional `(extends ParentClass)` clause in `class` definitions
- Method lookup walks the class hierarchy (child → parent → grandparent → ...)
- Field inheritance: child classes inherit parent fields
- `super` for calling the parent's implementation of a method

### 11. Traits

Traits are the composition mechanism. They're simpler than mixins because they don't deal with state.

**What to do:**
- `(trait TraitName (method ...))` defines a trait
- `(uses TraitName)` inside a class copies trait methods into the class's method table
- Conflict resolution: if a class defines a method that a trait also defines, the class wins
- No trait state (traits don't have `fields`)

### 12. Introspection

Make objects queryable at runtime — this is essential for the REPL to feel alive.

**What to do:**
- `[obj class]` → returns the class name (or the class object)
- `[obj methods]` → returns a list of selector strings
- `[obj respondsTo: "selector"]` → returns `true`/`false`
- `[ClassName fields]` → returns a list of field names

---

## Phase 2.5 — Making the Language Self-Hosting

These are features that sit between the object system and the type system. They make Moof powerful enough to write non-trivial programs.

### 13. Pattern Matching (`match`)

Even before the full type system, pattern matching is incredibly useful with lists, maps, and literal values.

**What to do:**
- Add a `match` special form: `(match expr (pattern1 body1) (pattern2 body2) ...)`
- Support literal patterns, variable binding, list destructuring, and a wildcard `_`
- Later extend to work with algebraic data types

### 14. Module System (`import` / `export`)

Without modules, every `.moof` file pollutes the global namespace. This blocks writing real programs.

**What to do:**
- `(import "path/to/file")` loads and evaluates a file, returning its exports as a map (or namespace)
- `(export name1 name2 ...)` at the top of a file declares what's public
- Qualified access: `[math sin: x]` or `(math.sin x)` — pick one, commit
- Circular import detection

### 15. Iterators and `for` / `each`

The message-passing `[list each: f]` works, but a dedicated iteration construct would be more natural for imperative-style code:

```moof
(for x in (list 1 2 3)
  (print x))
```

### 16. File I/O

The spec mentions `[file open: "path"]` but doesn't implement it. Even basic read/write would make Moof practical for scripting.

**What to do:**
- `(read-file "path")` → string contents
- `(write-file "path" content)` → nil
- `(file-exists? "path")` → bool
- Later: wrap in objects for the `[]` syntax

---

## Phase 3 — Type System (Future, Needs Design)

These are noted for completeness. They require significant design work before implementation.

### 17. Algebraic Data Types

```moof
(type Option (Some value) None)
```

Needs: new AST nodes, constructor functions, pattern matching integration.

### 18. Optional Type Annotations

```moof
(: square (-> Int Int))
```

Needs: type representation, inference engine (or just checking), error reporting.

### 19. Structural Typing for Message Sends

```moof
(: process (-> (Obj [area perimeter]) Float))
```

This is the most interesting and novel part of Moof's type system design. It bridges the dynamic message-passing world with static safety.

---

## Implementation Priority (Recommended Order)

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| 🔴 P0 | Error messages with source locations | Medium | Critical — unusable for real code without this |
| 🔴 P0 | `and`/`or` short-circuiting | Small | Correctness bug |
| 🟡 P1 | Tail call optimization | Medium | Removes a hard crash on recursive code |
| 🟡 P1 | `map`/`filter`/`reduce` builtins | Small | Table stakes for a Lisp |
| 🟡 P1 | REPL polish (history, colors, ,type) | Small | Makes daily use pleasant |
| 🟡 P1 | Variadic functions | Medium | Unblocks lots of user-defined abstractions |
| 🟢 P2 | Class definitions | Large | The Phase 2 headliner |
| 🟢 P2 | Inheritance + traits | Large | Completes the object system |
| 🟢 P2 | Introspection | Medium | Makes the object system usable in the REPL |
| 🟢 P2 | String interpolation | Medium | Quality-of-life for real programs |
| 🔵 P3 | Pattern matching | Large | Bridges to the type system |
| 🔵 P3 | Module system | Large | Prerequisite for multi-file programs |
| 🔵 P3 | File I/O | Medium | Makes Moof practical for scripting |
| ⚪ P4 | Algebraic data types | Very Large | Needs design work first |
| ⚪ P4 | Type annotations + inference | Very Large | Research-level work |

---

## What's Already Good

Worth calling out what doesn't need fixing:

- **The pipeline architecture** (source → tokens → AST → normalized AST → values) is clean and well-separated
- **The normalizer** is a smart design — `[]` lowering to `__send` keeps the interpreter simple
- **Error hierarchy** is well-structured with proper subclassing
- **Test coverage** is solid at 209 tests / 391 assertions, and they all pass
- **The REPL** already has history, tab completion, meta-commands, and continuation support — that's above average for a prototype
- **The dispatcher** handles all built-in types correctly and has proper arity checking

The foundation is genuinely strong. The improvements above are about building on it, not fixing it.
