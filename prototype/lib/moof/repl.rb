require "readline"

module Moof
  class Repl
    attr_reader :interpreter

    PROMPT      = "moof> "
    CONT_PROMPT = " ...> "
    HISTORY_FILE = File.expand_path("~/.moof_history")
    HISTORY_SIZE = 1000

    TIPS = [
      'Try [\"hello\" uppercase] to send a message to a string',
      "Use ,doc <name> to look up any function, class, or trait",
      "Use ,methods String to see all available String messages",
      "Extend built-in types: (class Integer (method double [] (* self 2)))",
      "Use (pipe f g h) to compose functions left-to-right",
      "_ always holds the last result",
      "Use ,time <expr> to benchmark an expression",
      "Use ,ast <expr> to see how your code is parsed",
      "(map f lst), (filter pred lst), (reduce f init lst) — your functional toolkit",
      "Classes are open! Re-evaluate (class Foo ...) to add methods anytime",
    ].freeze

    def initialize
      @interpreter   = nil
      @meta_commands = nil
      @eval_count    = 0
      @start_time    = nil
    end

    def run
      @start_time    = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      @interpreter   = Moof::Interpreter.new
      @meta_commands = MetaCommands.new(self)
      Moof.load_stdlib(@interpreter)

      print_banner
      setup_readline
      load_history

      buffer = +""
      loop do
        prompt = buffer.empty? ? colorize_prompt(PROMPT) : colorize_prompt(CONT_PROMPT)
        line = Readline.readline(prompt, false)  # we manage history ourselves

        if line.nil?
          puts
          break
        end

        stripped = line.strip

        # Skip empty lines when no buffer
        next if stripped.empty? && buffer.empty?

        # Add to readline history (dedup consecutive)
        add_to_history(line) unless stripped.empty?

        # Meta commands — only when not accumulating multi-line
        if buffer.empty? && stripped.start_with?(",")
          @meta_commands.handle(stripped)
          next
        end

        buffer << line << "\n"
        next unless balanced?(buffer)

        input = buffer.strip
        buffer = +""
        next if input.empty?

        evaluate_input(input)
      end

      print_farewell
      save_history
    end

    private

    def colorize_prompt(prompt)
      Printer::Colors.enabled? ? Printer::Colors.bold(Printer::Colors.cyan(prompt)) : prompt
    end

    def print_banner
      puts
      puts Printer::Colors.bold(Printer::Colors.cyan("  Moof #{Moof::VERSION}"))
      puts Printer::Colors.dim("  A Lisp with Smalltalk-style message passing")
      puts
      puts Printer::Colors.dim("  Type ,help for commands, Ctrl-D to exit")
      puts Printer::Colors.dim("  Tip: #{TIPS.sample}")
      puts
    end

    def print_farewell
      elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - @start_time
      stats = "#{@eval_count} expression#{"s" unless @eval_count == 1}"
      time = format_session_time(elapsed)
      puts Printer::Colors.dim("Session: #{stats} in #{time}. Goodbye!")
    end

    def format_session_time(seconds)
      if seconds < 60
        "#{seconds.round(1)}s"
      elsif seconds < 3600
        "#{(seconds / 60).round(1)}m"
      else
        "#{(seconds / 3600).round(1)}h"
      end
    end

    def setup_readline
      Readline.completion_append_character = " "
      Readline.completion_proc = Completion.build(
        @interpreter.global_env,
        interpreter: @interpreter
      )
    end

    def refresh_completion
      Readline.completion_proc = Completion.build(
        @interpreter.global_env,
        interpreter: @interpreter
      )
    end

    def load_history
      return unless File.exist?(HISTORY_FILE)
      File.readlines(HISTORY_FILE, chomp: true).last(HISTORY_SIZE).each do |line|
        Readline::HISTORY.push(line) unless line.strip.empty?
      end
    rescue; end

    def save_history
      lines = Readline::HISTORY.to_a.last(HISTORY_SIZE)
      File.write(HISTORY_FILE, lines.join("\n") + "\n")
    rescue; end

    def add_to_history(line)
      return if line.strip.empty?
      # Dedup consecutive
      if Readline::HISTORY.length > 0 && Readline::HISTORY[Readline::HISTORY.length - 1] == line
        return
      end
      Readline::HISTORY.push(line)
    end

    def balanced?(source)
      depth_p = depth_b = depth_br = 0
      in_string = escape = in_comment = in_block_comment = false
      block_depth = 0
      chars = source.chars
      i = 0
      while i < chars.length
        ch = chars[i]
        nch = chars[i + 1]

        if in_block_comment
          if ch == "#" && nch == "|"
            block_depth += 1
            i += 2; next
          elsif ch == "|" && nch == "#"
            block_depth -= 1
            in_block_comment = false if block_depth == 0
            i += 2; next
          end
          i += 1; next
        end

        if in_comment
          in_comment = false if ch == "\n"
          i += 1; next
        end

        if escape
          escape = false
          i += 1; next
        end

        if in_string
          case ch
          when "\\" then escape = true
          when '"'  then in_string = false
          end
          i += 1; next
        end

        case ch
        when ";"  then in_comment = true
        when "#"
          if nch == "|"
            in_block_comment = true
            block_depth = 1
            i += 2; next
          end
        when '"'  then in_string  = true
        when '('  then depth_p  += 1
        when ')'  then depth_p  -= 1
        when '['  then depth_b  += 1
        when ']'  then depth_b  -= 1
        when '{'  then depth_br += 1
        when '}'  then depth_br -= 1
        end
        i += 1
      end

      # Still in string or block comment = not balanced
      return false if in_string || in_block_comment

      depth_p <= 0 && depth_b <= 0 && depth_br <= 0
    end

    def evaluate_input(input)
      tokens     = Lexer.new(input, filename: "(repl)").tokenize
      program    = Parser.new(tokens).parse_program
      normalized = Normalizer.new.call(program)

      t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      result = @interpreter.evaluate(normalized)
      elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0

      @eval_count += 1
      @interpreter.global_env.define("_", result)
      refresh_completion

      unless result.nil?
        formatted = Printer.format(result)
        type_hint = Printer::Colors.dim(" : #{Printer.type_of(result)}")
        time_hint = elapsed > 0.1 ? Printer::Colors.dim(" [#{format_time(elapsed)}]") : ""
        puts Printer::Colors.green("=> ") + formatted + type_hint + time_hint
      end
    rescue Moof::MoofError => e
      $stderr.puts Printer.format_error(e, source: input)
    rescue ZeroDivisionError
      $stderr.puts Printer::Colors.red("Error: Division by zero")
    rescue => e
      $stderr.puts Printer::Colors.red("Internal error: #{e.class}: #{e.message}")
      if ENV["MOOF_DEBUG"]
        e.backtrace.first(5).each { |l| $stderr.puts Printer::Colors.dim("  #{l}") }
      end
    end

    def format_time(seconds)
      if seconds < 0.001
        "#{(seconds * 1_000_000).round}us"
      elsif seconds < 1
        "#{(seconds * 1_000).round(1)}ms"
      else
        "#{seconds.round(3)}s"
      end
    end
  end
end
