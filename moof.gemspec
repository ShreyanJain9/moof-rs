require_relative "lib/moof/version"

Gem::Specification.new do |spec|
  spec.name          = "moof"
  spec.version       = Moof::VERSION
  spec.authors       = ["Moof Contributors"]
  spec.summary       = "A Lisp with Smalltalk-style message passing"
  spec.description   = "Moof is a general-purpose scripting language combining Lisp's " \
                        "homoiconic s-expressions with Smalltalk/Objective-C style [] " \
                        "message passing."
  spec.homepage      = "https://github.com/moof-lang/moof"
  spec.license       = "MIT"

  spec.required_ruby_version = ">= 3.2.0"

  spec.files         = Dir["lib/**/*.rb", "exe/*", "LICENSE", "README.md"]
  spec.bindir        = "exe"
  spec.executables   = ["moof"]

  # No runtime dependencies — uses stdlib readline

  spec.add_development_dependency "minitest", "~> 5.0"
end
