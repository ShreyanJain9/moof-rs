module Moof
  # Runtime representation of a class (user-defined or built-in).
  # Classes are OPEN — re-evaluating (class Foo ...) merges new methods.
  class MoofClass
    attr_reader :name, :superclass, :own_fields
    attr_accessor :method_table

    def initialize(name:, superclass: nil, fields: [], method_table: {})
      @name         = name
      @superclass   = superclass   # MoofClass or nil
      @own_fields   = fields       # Array[String] — this class's own fields
      @method_table = method_table # Hash[String, Function]
    end

    # All fields including inherited ones (parent first).
    def fields
      if @superclass
        @superclass.fields + @own_fields
      else
        @own_fields
      end
    end

    # Look up a method selector, walking the inheritance chain.
    def lookup(selector)
      @method_table[selector] || (@superclass&.lookup(selector))
    end

    # Merge new methods and fields into this class (open class support).
    def reopen(new_fields: [], new_methods: {}, new_superclass: nil, new_traits: [])
      @own_fields = (@own_fields + new_fields).uniq
      @superclass = new_superclass if new_superclass && @superclass.nil?
      new_traits.each { |trait_methods| trait_methods.each { |sel, f| @method_table[sel] ||= f } }
      new_methods.each { |sel, f| @method_table[sel] = f }
    end

    # Create a new instance with positional field values.
    def instantiate(args)
      all_fields = fields
      if args.length != all_fields.length
        raise Moof::ArityError.new(all_fields.length, args.length, name: @name)
      end
      field_values = {}
      all_fields.each_with_index { |f, i| field_values[f] = args[i] }
      MoofObject.new(klass: self, fields: field_values)
    end

    def to_s; "<class #{@name}>"; end
    def inspect; to_s; end
  end

  # Runtime representation of a user-defined object instance.
  class MoofObject
    attr_reader :klass, :fields

    def initialize(klass:, fields: {})
      @klass  = klass
      @fields = fields
    end

    def get_field(name)
      unless @fields.key?(name)
        raise Moof::MessageError.new(@klass.name, name)
      end
      @fields[name]
    end

    def set_field(name, value)
      unless @fields.key?(name)
        raise Moof::MessageError.new(@klass.name, "#{name}=")
      end
      @fields[name] = value
    end

    def to_s
      field_str = @fields.map { |k, v| "#{k}: #{v.inspect}" }.join(", ")
      "(#{@klass.name} #{field_str})"
    end
    def inspect; to_s; end
  end
end
