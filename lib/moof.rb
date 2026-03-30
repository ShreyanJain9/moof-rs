require_relative "moof/version"
require_relative "moof/errors"
require_relative "moof/token"
require_relative "moof/ast"
require_relative "moof/lexer"
require_relative "moof/parser"
require_relative "moof/normalizer"
require_relative "moof/environment"
require_relative "moof/values"
require_relative "moof/function"
require_relative "moof/object_system"
require_relative "moof/dispatcher"
require_relative "moof/builtins"
require_relative "moof/interpreter"
require_relative "moof/printer"
require_relative "moof/meta_commands"
require_relative "moof/completion"
require_relative "moof/repl"

module Moof
  STDLIB_PATH = File.expand_path("../moof/stdlib.moof", __FILE__)

  def self.evaluate(source, filename: "(eval)", env: nil)
    tokens     = Lexer.new(source, filename: filename).tokenize
    program    = Parser.new(tokens).parse_program
    normalized = Normalizer.new.call(program)
    interp     = Interpreter.new
    load_stdlib(interp)
    interp.evaluate(normalized)
  end

  def self.load_stdlib(interp)
    return unless File.exist?(STDLIB_PATH)
    source     = File.read(STDLIB_PATH)
    tokens     = Lexer.new(source, filename: "stdlib.moof").tokenize
    program    = Parser.new(tokens).parse_program
    normalized = Normalizer.new.call(program)
    interp.evaluate(normalized)
  rescue => e
    $stderr.puts "Warning: stdlib failed to load: #{e.message}" if ENV["MOOF_DEBUG"]
  end

  def self.repl
    Repl.new.run
  end
end
