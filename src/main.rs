use moof::interpreter::Interpreter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check for --bytecode / -b flag
    let use_bytecode = args.iter().any(|a| a == "--bytecode" || a == "-b");
    let args: Vec<&String> = args.iter().filter(|a| *a != "--bytecode" && *a != "-b").collect();

    if args.len() == 1 {
        // REPL mode
        moof::repl::run_repl();
    } else if *args[1] == "-v" || *args[1] == "--version" {
        println!("Moof {VERSION}");
    } else if *args[1] == "-h" || *args[1] == "--help" {
        print_usage();
    } else if *args[1] == "-e" {
        // Eval mode
        if args.len() < 3 {
            eprintln!("Error: -e requires an argument");
            std::process::exit(1);
        }
        if use_bytecode {
            run_bytecode(args[2], "<eval>", true);
        } else {
            run_source(args[2], "<eval>", true);
        }
    } else {
        // File mode
        let path = args[1].as_str();
        match std::fs::read_to_string(path) {
            Ok(source) => {
                if use_bytecode {
                    run_bytecode(&source, path, false);
                } else {
                    run_source(&source, path, false);
                }
            }
            Err(e) => {
                eprintln!("Error: Cannot read '{}': {}", path, e);
                std::process::exit(1);
            }
        }
    }
}

fn print_usage() {
    println!("Usage: moof [options] [file.moof]");
    println!();
    println!("Options:");
    println!("  -e <expr>      Evaluate expression and print result");
    println!("  -b, --bytecode Use bytecode VM instead of tree-walker");
    println!("  -v, --version  Print version");
    println!("  -h, --help     Print this help");
    println!();
    println!("Examples:");
    println!("  moof                       Start the REPL");
    println!("  moof script.moof           Run a file");
    println!("  moof -e '(+ 1 2)'          Evaluate and print");
    println!("  moof -b -e '(+ 1 2)'       Evaluate via bytecode VM");
}

fn run_source(source: &str, filename: &str, print_result: bool) {
    let mut interp = Interpreter::new();
    load_stdlib(&mut interp);
    interp.snapshot_baseline();

    match interp.load_source(source, filename) {
        Ok(result) => {
            if print_result {
                match result {
                    moof::value::Value::Nil => {} // don't print nil for -e
                    ref val => println!("{}", interp.inspect_value(val)),
                }
            }
        }
        Err(e) => {
            print_error(source, &e);
            std::process::exit(1);
        }
    }
}

fn run_bytecode(source: &str, filename: &str, print_result: bool) {
    let mut interp = Interpreter::new();
    load_stdlib(&mut interp);
    interp.snapshot_baseline();

    // Parse
    let exprs = match interp.parse_source(source, filename) {
        Ok(exprs) => exprs,
        Err(e) => {
            print_error(source, &e);
            std::process::exit(1);
        }
    };

    // Compile
    let mut compiler = moof::compiler::Compiler::new(&mut interp);
    let func = match compiler.compile_program(&exprs) {
        Ok(func) => func,
        Err(e) => {
            print_error(source, &e);
            std::process::exit(1);
        }
    };

    // Execute
    let mut vm = moof::vm::VM::new(interp);
    match vm.execute(func) {
        Ok(result) => {
            if print_result {
                match result {
                    moof::value::Value::Nil => {}
                    ref val => println!("{}", vm.interp.inspect_value(val)),
                }
            }
        }
        Err(e) => {
            print_error(source, &e);
            std::process::exit(1);
        }
    }
}

fn print_error(source: &str, e: &moof::error::MoofError) {
    let kind_label = match e.kind {
        moof::error::ErrorKind::Syntax => "SyntaxError",
        moof::error::ErrorKind::Runtime => "RuntimeError",
        moof::error::ErrorKind::Name => "NameError",
        moof::error::ErrorKind::Message => "MessageError",
        moof::error::ErrorKind::Arity => "ArityError",
        moof::error::ErrorKind::Type => "TypeError",
        moof::error::ErrorKind::IO => "IOError",
    };

    eprintln!("\x1b[31m{kind_label}: {}\x1b[0m", e.message);

    if let Some(line_num) = e.line {
        let lines: Vec<&str> = source.lines().collect();
        if line_num > 0 && line_num <= lines.len() {
            eprintln!(
                "\x1b[2m  {} | {}\x1b[0m",
                line_num,
                lines[line_num - 1]
            );
            if let Some(col) = e.column {
                if col > 0 {
                    let padding =
                        " ".repeat(col - 1 + format!("{line_num}").len() + 4);
                    eprintln!("\x1b[31m{padding}^\x1b[0m");
                }
            }
        }
    }
}

fn load_stdlib(interp: &mut Interpreter) {
    if let Err(e) = interp.load_prelude() {
        eprintln!(
            "\x1b[33mWarning: failed to load stdlib: {}\x1b[0m",
            e.message
        );
    }
}
