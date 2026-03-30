module Moof
  module PatternMatcher
    # Returns {success: true/false, bindings: {name => value}}
    def self.match(pattern, value, interpreter)
      case pattern
      when AST::MatchWildcard
        {success: true, bindings: {}}
      when AST::MatchBind
        {success: true, bindings: {pattern.name => value}}
      when AST::IntegerLiteral, AST::FloatLiteral, AST::StringLiteral, AST::BoolLiteral
        {success: value == pattern.value, bindings: {}}
      when AST::NilLiteral
        {success: value.nil?, bindings: {}}
      when AST::MatchList
        match_list(pattern, value, interpreter)
      when AST::MatchMap
        match_map(pattern, value, interpreter)
      when AST::MatchConstructor
        match_constructor(pattern, value, interpreter)
      when AST::Identifier
        if pattern.name == "_"
          {success: true, bindings: {}}
        else
          {success: true, bindings: {pattern.name => value}}
        end
      else
        {success: false, bindings: {}}
      end
    end

    private

    def self.match_list(pattern, value, _interpreter)
      return {success: false, bindings: {}} unless value.is_a?(Array)

      elements = pattern.elements
      rest = pattern.rest

      if rest
        # With rest pattern: need at least as many elements as non-rest patterns
        return {success: false, bindings: {}} if value.length < elements.length
      else
        # Without rest: exact length match
        return {success: false, bindings: {}} if value.length != elements.length
      end

      bindings = {}
      elements.each_with_index do |el_pattern, i|
        result = match(el_pattern, value[i], _interpreter)
        return {success: false, bindings: {}} unless result[:success]
        bindings.merge!(result[:bindings])
      end

      if rest
        bindings[rest] = value[elements.length..] || []
      end

      {success: true, bindings: bindings}
    end

    def self.match_map(pattern, value, _interpreter)
      return {success: false, bindings: {}} unless value.is_a?(Hash)

      bindings = {}
      pattern.pairs.each do |key, sub_pattern|
        return {success: false, bindings: {}} unless value.key?(key)
        result = match(sub_pattern, value[key], _interpreter)
        return {success: false, bindings: {}} unless result[:success]
        bindings.merge!(result[:bindings])
      end

      {success: true, bindings: bindings}
    end

    def self.match_constructor(pattern, value, _interpreter)
      return {success: false, bindings: {}} unless value.is_a?(MoofObject)
      return {success: false, bindings: {}} unless value.klass.name == pattern.class_name

      bindings = {}
      field_names = value.klass.fields
      pattern.bindings.each_with_index do |bind_pattern, i|
        field_name = field_names[i]
        return {success: false, bindings: {}} unless field_name
        field_value = value.fields[field_name]
        result = match(bind_pattern, field_value, _interpreter)
        return {success: false, bindings: {}} unless result[:success]
        bindings.merge!(result[:bindings])
      end

      {success: true, bindings: bindings}
    end
  end
end
