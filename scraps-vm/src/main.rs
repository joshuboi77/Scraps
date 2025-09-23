mod vm;
mod bytecode;
mod value;
mod compiler;
mod lexer;
mod parser;
mod tcp_socket_manager;
mod websocket_manager;
mod udp_socket_manager;
mod tls_socket_manager;
mod event_loop_manager;
mod connection_pool_manager;
mod timeout_manager;
mod proxy_manager;
mod raw_socket_manager;
mod network_interface_manager;
mod ipv6_manager;

use compiler::Compiler;
use lexer::Lexer;
use parser::Parser;

use std::fs;
use std::env;
use std::path::PathBuf;

fn main() {
    // Collect command-line arguments
    let args: Vec<String> = env::args().collect();
    let mut i = 1;

    if args.len() < 2 {
        eprintln!("Usage: scraps <source.scraps>");
        eprintln!("Or: scraps --lex <source.scraps> to test lexer");
        eprintln!("Or: scraps --parse <source.scraps> to test parser");
        eprintln!("Flags: --debug to enable verbose parser logging");
        std::process::exit(1);
    }

    // Optional debug flag first
    if i < args.len() && args[i] == "--debug" {
        parser::set_debug_enabled(true);
        i += 1;
    }

    if i < args.len() && args[i] == "--lex" {
        if args.len() < i + 2 {
            eprintln!("Usage: scraps --lex <source.scraps>");
            std::process::exit(1);
        }
        
        // Test lexer
        let filename = &args[i + 1];
        let source = fs::read_to_string(filename)
            .expect("Failed to read source file");
        
        let mut lexer = Lexer::new(source);
        match lexer.tokenize() {
            Ok(tokens) => {
                println!("Lexer output:");
                for token in tokens {
                    println!("  {:?} at {}:{}", token.kind, token.line, token.column);
                }
            }
            Err(e) => eprintln!("Lexer error: {}", e),
        }
        return;
    }
    
    // Scaffold a new Scraps project
    if i < args.len() && (args[i] == "new" || args[i] == "NEW") {
        if args.len() < i + 2 {
            eprintln!("Usage: scraps new <project_dir>");
            std::process::exit(1);
        }
        let proj = PathBuf::from(&args[i + 1]);
        if let Err(e) = fs::create_dir_all(proj.join("void")) { eprintln!("Failed to create project dirs: {}", e); std::process::exit(1); }
        // .gitignore
        let gi = proj.join(".gitignore");
        let gitignore = b"# Scraps
target/
*.log
.DS_Store
";
        if fs::write(&gi, gitignore).is_err() {}
        // clanker.toml
        let clanker = proj.join("clanker.toml");
        let clanker_contents = "# Scraps manifest\n\n[modules]\nmain = \"main.scraps\"\n\n[paths]\nsources = \"void\"\n";
        if let Err(e) = fs::write(&clanker, clanker_contents) { eprintln!("Failed to write clanker.toml: {}", e); }
        // void/main.scraps
        let main_src = proj.join("void").join("main.scraps");
        let hello = "print \"Hello World!\"\n";
        if let Err(e) = fs::write(&main_src, hello) { eprintln!("Failed to write main.scraps: {}", e); }
        println!("Created Scraps project at {}", args[i + 1]);
        println!("Try: cd {} && scraps void/main.scraps", args[i + 1]);
        return;
    }

    // Build: simple dependency and module check
    if i < args.len() && (args[i] == "build" || args[i] == "BUILD") {
        let root = if args.len() > i + 1 { PathBuf::from(&args[i + 1]) } else { env::current_dir().expect("cwd") };
        let manifest = root.join("clanker.toml");
        if !manifest.exists() { eprintln!("No clanker.toml found at {}", manifest.display()); std::process::exit(1); }
        let content = fs::read_to_string(&manifest).expect("read clanker.toml");
        // Naive parse
        let mut in_modules = false;
        let mut modules: Vec<(String, String)> = Vec::new();
        let mut sources_dir: Option<String> = None;
        for line in content.lines() {
            let t = line.trim();
            if t.starts_with('[') { in_modules = t == "[modules]"; continue; }
            if in_modules {
                if let Some(eq) = t.find('=') { let key = t[..eq].trim().to_string(); let mut val = t[eq+1..].trim().to_string(); if val.starts_with('"') && val.ends_with('"') && val.len()>=2 { val = val[1..val.len()-1].to_string(); } if !key.is_empty() && !val.is_empty() { modules.push((key, val)); } }
            }
            if t.starts_with("sources") && t.contains('=') {
                if let Some(eq) = t.find('=') { let mut val = t[eq+1..].trim().to_string(); if val.starts_with('"') && val.ends_with('"') && val.len()>=2 { val = val[1..val.len()-1].to_string(); } if !val.is_empty() { sources_dir = Some(val); } }
            }
        }
        let srcdir = sources_dir.unwrap_or_else(|| "void".to_string());
        let srcroot = root.join(&srcdir);
        let mut ok = true;
        for (name, rel) in modules.iter() {
            let p = srcroot.join(rel);
            if !p.exists() {
                eprintln!("Missing module '{}': {}", name, p.display());
                ok = false;
            }
        }
        if ok { println!("Build ok ({} modules, sources='{}')", modules.len(), srcdir); std::process::exit(0); } else { std::process::exit(1); }
    }

    if i < args.len() && args[i] == "--parse" {
        if args.len() < i + 2 {
            eprintln!("Usage: scraps --parse <source.scraps>");
            std::process::exit(1);
        }
        
        // Test parser
        let filename = &args[i + 1];
        let source = fs::read_to_string(filename)
            .expect("Failed to read source file");
        
        let mut lexer = Lexer::new(source);
        match lexer.tokenize() {
            Ok(tokens) => {
                let mut parser = parser::Parser::new(tokens);
                match parser.parse() {
                    Ok(program) => {
                        println!("Parser output:");
                        println!("  Program with {} statements", program.statements.len());
                        for (i, stmt) in program.statements.iter().enumerate() {
                            println!("  Statement {}: {:?}", i, stmt);
                        }
                    }
                    Err(e) => eprintln!("Parser error: {}", e),
                }
            }
            Err(e) => eprintln!("Lexer error: {}", e),
        }
        return;
    }
    
    // Verify mode: run in safe profile (disallow IO/network/rewire side-effects)
    if i < args.len() && args[i] == "--verify" {
        if args.len() < i + 2 {
            eprintln!("Usage: scraps --verify <source.scraps>");
            std::process::exit(1);
        }
        let filename = &args[i + 1];
        let source = fs::read_to_string(filename)
            .expect("Failed to read source file");
        let mut lexer = Lexer::new(source);
        match lexer.tokenize() {
            Ok(tokens) => {
                let mut parser = Parser::new(tokens);
                match parser.parse() {
                    Ok(program) => {
                        let mut compiler = Compiler::new();
                        match compiler.compile(program.statements) {
                            Ok(bytecode) => {
                                vm::set_safe_mode(true);
                                vm::reset_verify_counters();
                                if let Err(e) = vm::run_with_context(&bytecode, Some(filename), 0) {
                                    eprintln!("\x1b[31mVerify failed:\x1b[0m {}", e);
                                    std::process::exit(1);
                                } else {
                                    let n = vm::get_verify_assert_count();
                                    println!("Verify passed (tests: {})", n);
                                }
                            }
                            Err(e) => { eprintln!("Compile error: {}", e); std::process::exit(1); }
                        }
                    }
                    Err(e) => eprintln!("Parser error: {}", e),
                }
            }
            Err(e) => eprintln!("Lexer error: {}", e),
        }
        return;
    }
    
    // Normal execution
    if i >= args.len() {
        eprintln!("Usage: scraps [--debug] <source.scraps>");
        std::process::exit(1);
    }
    let filename = &args[i];
    let source = fs::read_to_string(filename)
        .expect("Failed to read source file");

    // Use the new compiler with parser
    let mut lexer = Lexer::new(source);
    match lexer.tokenize() {
        Ok(tokens) => {
            let mut parser = Parser::new(tokens);
            match parser.parse() {
                Ok(program) => {
                    let mut compiler = Compiler::new();
                    match compiler.compile(program.statements) {
                        Ok(bytecode) => {
                            if let Err(e) = vm::run_with_context(&bytecode, Some(filename), 0) {
                                eprintln!("\x1b[31mRuntime error:\x1b[0m {}", e);
                                // Context-aware hints (first pass)
                                let hint = if e.contains("UNPACK") || e.contains("index out of bounds") {
                                    Some("Check indices/slice bounds for boxes and strings")
                                } else if e.starts_with("HTTP_") || e.starts_with("URL_") || e.contains("DNS_") {
                                    Some("Verify arguments and network connectivity (URL/headers/DNS)")
                                } else if e.starts_with("JSON_") {
                                    Some("Ensure value is JSON-encodable or input is valid JSON")
                                } else if e.starts_with("READ") || e.starts_with("WRITE") || e.starts_with("SOURCE") {
                                    Some("Check file path and permissions")
                                } else if e.contains("TEST failed") {
                                    Some("Log intermediate values; confirm the condition is boolean and true")
                                } else { None };
                                if let Some(h) = hint { eprintln!("Tip: {}", h); }
                            }
                        }
                        Err(e) => {
                            eprintln!("\x1b[31mCompile error:\x1b[0m {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => eprintln!("Parser error: {}", e),
            }
        }
        Err(e) => eprintln!("Lexer error: {}", e),
    }
}
