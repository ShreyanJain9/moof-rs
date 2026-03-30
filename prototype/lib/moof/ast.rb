module Moof
  module AST
    # Helper: create a Data subclass with optional fields (default nil).
    # `optional` fields default to nil if not provided.
    # `line` and `column` are always optional.
    def self.node(*required_fields, optional: [])
      all_fields = [*required_fields, *optional, :line, :column]
      klass = Data.define(*all_fields)
      optional_with_loc = [*optional, :line, :column]
      klass.define_method(:initialize) do |**kwargs|
        defaults = optional_with_loc.each_with_object({}) { |f, h| h[f] = nil }
        super(**defaults.merge(kwargs))
      end
      klass
    end

    # -- Program (top-level) --
    Program = Data.define(:expressions)

    # -- Literals --
    IntegerLiteral = node(:value)
    FloatLiteral   = node(:value)
    StringLiteral  = node(:value)
    BoolLiteral    = node(:value)
    NilLiteral     = node()

    # -- Names --
    Identifier = node(:name)

    # -- Compound --
    MapLiteral = node(:pairs)          # pairs: Array of [String, Node]
    Quote      = node(:expression)

    # -- Calls --
    # Generic function call: (func arg1 arg2)
    Call = node(:callee, :arguments)

    # Message send: [receiver selector: arg]
    # Only exists before normalization — Normalizer lowers these to Call(__send, ...)
    MessageSend = node(:receiver, :selector, :arguments)

    # -- Special Forms --
    # (define name value) — mutable binding
    Define = node(:name, :value)

    # (define (name params...) body) — function shorthand; rest_param is optional
    DefineFunction = node(:name, :params, :body, optional: [:rest_param])

    # (lambda (params...) body); rest_param is optional
    Lambda = node(:params, :body, optional: [:rest_param])

    # (if condition then else)
    If = node(:condition, :then_branch, :else_branch)

    # (let ((x 1) (y 2)) body) — immutable bindings
    Let = node(:bindings, :body)  # bindings: Array of [String, Node]

    # (do expr1 expr2 ...)
    Do = node(:expressions)

    # (set! name value) — mutate existing binding
    SetBang = node(:name, :value)

    # (try body (catch var handler))
    TryCatch = node(:body, :error_name, :catch_body)

    # (cond (test1 expr1) (test2 expr2) (else expr3))
    Cond = node(:clauses)  # Array of [Node, Node]

    # (and expr1 expr2) — short-circuit logical and (special form)
    And = node(:left, :right)

    # (or expr1 expr2) — short-circuit logical or (special form)
    Or = node(:left, :right)

    # (class Name (extends Parent) (fields ...) (method ...) (uses Trait))
    ClassDef = node(:name, :fields, :methods, :traits, optional: [:superclass])

    # (method selector [params...] body)
    MethodDef = node(:selector, :params, :body)

    # (trait Name (method ...))
    TraitDef = node(:name, :methods)

    # ============ New AST nodes ============

    # -- Pattern Matching --
    # (match expr clauses...)
    Match = node(:expr, :clauses)  # clauses: Array of MatchClause

    # Each clause in a match expression
    MatchClause = node(:pattern, :guard, :body)  # guard is nil if no when

    # Pattern nodes
    MatchWildcard    = node()                        # _ pattern
    MatchBind        = node(:name)                   # bind a name
    MatchList        = node(:elements, :rest)         # (elem1 elem2 . rest)
    MatchMap         = node(:pairs)                   # {key: pattern ...}
    MatchConstructor = node(:class_name, :bindings)   # (Point x y)

    # -- Algebraic Data Types --
    # (type Option (Some value) None)
    TypeDef     = node(:name, :variants)  # variants: Array of TypeVariant
    TypeVariant = node(:name, :fields)    # fields: Array of String

    # -- Pipeline --
    # (-> value step1 step2 step3)
    Pipeline = node(:value, :steps)

    # -- Quasiquote / Unquote --
    Quasiquote  = node(:expression)
    Unquote     = node(:expression)
    UnquoteSplice = node(:expression)  # ,@

    # -- String Interpolation --
    # $"hello \(name)"
    StringInterp = node(:segments)  # Array of AST nodes (StringLiteral for text, any for exprs)

    # -- Protocol Definitions --
    # (protocol Measurable area perimeter)
    ProtocolDef = node(:name, :selectors)  # selectors: Array of String

    # -- First-Class Selector --
    # &name or &(keyword: arg ...)
    SelectorRef = node(:selector, :partial_args)

    # -- Keyword Arguments --
    # port: 443 inside a call
    KeywordArg = node(:keyword, :value)

    # -- DefMacro --
    # (defmacro name (params...) body)
    DefMacro = node(:name, :params, :body)

    # -- Module System --
    # (module name (export ...) body...)
    ModuleDef = node(:name, :exports, :body)  # exports: Array of String, body: Array of nodes

    # (use module-name) / (use module-name (name1 name2)) / (use module-name :as alias)
    UseModule = node(:module_name, :imports, :alias_name)  # imports: nil=all, Array=selective; alias_name: nil or String

    # (require "path/to/file")
    Require = node(:path)
  end
end
