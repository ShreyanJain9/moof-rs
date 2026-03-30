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
  end
end
