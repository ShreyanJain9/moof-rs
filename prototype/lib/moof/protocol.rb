module Moof
  class Protocol
    attr_reader :name, :selectors

    def initialize(name:, selectors:)
      @name = name
      @selectors = selectors
    end

    def satisfied_by?(obj, interpreter)
      selectors.all? { |sel| responds_to_selector?(obj, sel, interpreter) }
    end

    def to_s; "#<Protocol #{name} [#{selectors.join(" ")}]>"; end
    def inspect; to_s; end

    private

    def responds_to_selector?(obj, sel, interpreter)
      case obj
      when MoofObject
        !obj.klass.lookup(sel).nil? || obj.fields.key?(sel)
      else
        class_name = case obj
          when Integer then "Integer"
          when Float then "Float"
          when String then "String"
          when Array then "List"
          when Hash then "Map"
          when true, false then "Bool"
          when nil then "Nil"
          when Moof::Function then "Function"
          end
        return false unless class_name
        klass = interpreter.class_registry[class_name]
        return true if klass&.lookup(sel)
        BUILTIN_SELECTORS.fetch(class_name, []).include?(sel)
      end
    end

    BUILTIN_SELECTORS = {
      "Integer" => %w[abs to_s to_f to_i zero? positive? negative? nil? class pow: max: min: + - * / % > < >= <=],
      "Float" => %w[abs to_s to_f to_i zero? positive? negative? nil? class pow: max: min: + - * / % > < >= <=],
      "String" => %w[length uppercase lowercase reverse to_s to_i to_f chars trim nil? class at: contains: startsWith: endsWith: replaceAll:with: split: concat: slice:length:],
      "List" => %w[length first last rest reverse sort uniq flatten empty? to_s nil? class at: push: prepend: contains: join: indexOf: take: drop: zip: map: filter: reduce:init: each: any: all: none: sortBy:],
      "Map" => %w[keys values length empty? to_s nil? class at: put:value: remove: contains: merge:],
      "Function" => %w[call: arity nil? class],
      "Bool" => %w[not to_s nil? class and: or:],
      "Nil" => %w[nil? to_s class],
    }.freeze
  end
end
