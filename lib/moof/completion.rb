module Moof
  module Completion
    KEYWORDS = %w[
      true false nil
      define lambda if let do set! quote try catch cond and or
      class trait method fields extends uses
    ].freeze

    META_COMMANDS = %w[
      ,help ,env ,ast ,load ,quit ,exit ,version
      ,type ,doc ,methods ,time ,classes ,traits
      ,clear ,reset
    ].freeze

    SPECIAL_FORMS = %w[
      define lambda if let do set! quote try catch cond and or
      class trait
    ].freeze

    # Returns a Proc suitable for Readline.completion_proc=.
    def self.build(env, interpreter: nil)
      proc do |input|
        candidates = []

        # Meta commands
        if input.start_with?(",")
          candidates.concat(META_COMMANDS)
          next candidates.select { |c| c.start_with?(input) }.sort
        end

        # After "[" — suggest message selectors
        # After "(" — suggest functions and special forms

        # Environment binding names
        collect_bindings(env, candidates) if env

        # Class names from registry
        if interpreter
          candidates.concat(interpreter.class_registry.keys)
          candidates.concat(interpreter.trait_registry.keys)
        end

        # Keywords
        candidates.concat(KEYWORDS)

        # Common message selectors (for [] context)
        candidates.concat(common_selectors)

        candidates.uniq.select { |c| c.start_with?(input) }.sort
      end
    end

    # Common message selectors users might want to complete
    def self.common_selectors
      %w[
        length first last reverse sort empty? to_s to_i to_f
        abs zero? positive? negative?
        uppercase lowercase chars trim
        keys values
        at: push: contains: map: filter: each: reduce:init:
        join: split: replaceAll:with: startsWith: endsWith:
        class className methods respondsTo:
        call: arity nil?
      ]
    end

    def self.collect_bindings(env, result)
      result.concat(env.bindings.keys)
      collect_bindings(env.parent, result) if env.parent
    end

    private_class_method :collect_bindings
  end
end
