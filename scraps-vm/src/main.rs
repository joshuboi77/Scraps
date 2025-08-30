mod vm;
mod bytecode;
mod value;
mod compiler;
mod lexer;
mod parser;

use compiler::Compiler;
use lexer::Lexer;
use parser::Parser;

use std::fs;
use std::env;

fn main() {
    // Collect command-line arguments
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: scraps-vm <source.scraps>");
        eprintln!("Or: scraps-vm --lex <source.scraps> to test lexer");
        eprintln!("Or: scraps-vm --parse <source.scraps> to test parser");
        std::process::exit(1);
    }
    
    if args[1] == "--lex" {
        if args.len() < 3 {
            eprintln!("Usage: scraps-vm --lex <source.scraps>");
            std::process::exit(1);
        }
        
        // Test lexer
        let filename = &args[2];
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
    
    if args[1] == "--parse" {
        if args.len() < 3 {
            eprintln!("Usage: scraps-vm --parse <source.scraps>");
            std::process::exit(1);
        }
        
        // Test parser
        let filename = &args[2];
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
    
    // Normal execution
    let filename = &args[1];
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
                    let bytecode = compiler.compile(program.statements);
                    
                    if let Err(e) = vm::run(&bytecode) {
                        eprintln!("Runtime error: {}", e);
                    }
                }
                Err(e) => eprintln!("Parser error: {}", e),
            }
        }
        Err(e) => eprintln!("Lexer error: {}", e),
    }
}