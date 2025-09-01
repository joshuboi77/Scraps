use crate::lexer::{Token, TokenKind, TokenValue};

// Debug macro: prints only when SCRAPS_DEBUG is set
macro_rules! debug {
    ($($arg:tt)*) => {
        if crate::parser::is_debug_enabled() {
            println!($($arg)*);
        }
    };
}

use std::sync::atomic::{AtomicBool, Ordering};
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);
pub fn set_debug_enabled(on: bool) { DEBUG_ENABLED.store(on, Ordering::Relaxed); }
pub fn is_debug_enabled() -> bool { DEBUG_ENABLED.load(Ordering::Relaxed) || std::env::var("SCRAPS_DEBUG").is_ok() }

// ============================================================================
// AST Nodes
// ============================================================================

#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    Number(TokenValue),
    String(String),
    Bool(bool),
    
    // Variables and calls
    Variable(String),
    FunctionCall {
        function: Box<Expr>,
        arguments: Vec<Expr>,
    },
    
    // Binary operations
    Binary {
        left: Box<Expr>,
        operator: String,
        right: Box<Expr>,
    },
    
    // Unary operations
    Unary {
        operator: String,
        operand: Box<Expr>,
    },
    
    // Grouping
    Group(Box<Expr>),
    
    // List literal
    List(Vec<Expr>),
    
    // Box operations
    Unpack {
        box_expr: Box<Expr>,
        start: Box<Expr>,
        end: Option<Box<Expr>>,
    },
    
    Pick {
        box_expr: Box<Expr>,
        indices: Vec<Expr>,
    },
    

    

}

#[derive(Debug, Clone)]
pub enum Stmt {
    // Basic statements
    Print(Expr),
    Test(Expr),
    Expression(Expr),
    Assignment {
        name: String,
        value: Expr,
    },
    
    // Control flow
    If {
        condition: Expr,
        then_block: Block,
        else_block: Block,
    },
    
    While {
        condition: Expr,
        body: Block,
    },
    
    // Box operations
    Pack {
        values: Vec<Expr>,
        target: Expr,
    },
    
    Place {
        pairs: Vec<(Expr, Expr)>, // (index, value) pairs
        target: Expr,
    },
    
    // Function definition
    FunctionDef {
        name: String,
        parameters: Vec<String>,
        body: Block,
    },
    
    // Rewire
    Rewire {
        _target: String,
        body: Block,
        destination: String,
    },
    
    // I/O operations
    Write {
        content: Expr,
        filename: Expr,
    },
    
    Read {
        filename: Expr,
    },
    
    // String operations
    Fission {
        delimiter: Expr,
        string: Expr,
    },
    
    Fusion {
        delimiter: Expr,
        strings: Expr,
    },

    // Module operations
    Rename {
        from: Expr,
        to: Expr,
    },
}

#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

// ============================================================================
// Parser
// ============================================================================

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
        }
    }
    
    pub fn parse(&mut self) -> Result<Program, String> {
        let mut statements = Vec::new();
        
        while !self.is_at_end() {
            // Skip newlines
            while self.match_token(TokenKind::Newline) {
                // consume newline
            }
            
            if self.is_at_end() {
                break;
            }
            
            let stmt = self.statement()?;
            statements.push(stmt);
            
            // Skip newlines after statement (don't require them)
            while self.match_token(TokenKind::Newline) {
                // consume newline
            }
        }
        
        Ok(Program { statements })
    }
    
    fn statement(&mut self) -> Result<Stmt, String> {
        let token = self.peek();
        debug!("DEBUG: Parsing statement, token: {:?} '{}' at line {}", token.kind, token.lexeme, token.line);
        
        match token.kind {
            // Keywords
            TokenKind::Identifier => {
                let name = token.lexeme.clone();
                // debug removed
                match name.as_str() {
                    "print" | "PRINT" => {
                        self.advance(); // consume print
                        let expr = self.expression()?;
                        Ok(Stmt::Print(expr))
                    }
                    
                    "test" | "TEST" => {
                        self.advance(); // consume test
                        let expr = self.expression()?;
                        Ok(Stmt::Test(expr))
                    }
                    
                    "if" | "IF" => {
                        self.advance(); // consume if
                        let condition = self.expression()?;
                        let then_block = self.block()?;
                        
                        // Expect ELSE
                        if !self.match_keyword("ELSE") && !self.match_keyword("else") {
                            return Err("Expected 'ELSE' after if block".to_string());
                        }
                        
                        let else_block = self.block()?;
                        Ok(Stmt::If {
                            condition,
                            then_block,
                            else_block,
                        })
                    }
                    
                    "while" | "WHILE" => {
                        self.advance(); // consume while
                        let condition = self.expression()?;
                        let body = self.block()?;
                        Ok(Stmt::While { condition, body })
                    }
                    
                    "fn" | "FN" => {
                        self.advance(); // consume fn
                        self.function_definition()
                    }
                    
                    "rewire" | "REWIRE" => {
                        self.advance(); // consume rewire
                        // Support two forms:
                        // 1) rewire variable { ... } -> destination
                        // 2) rewire "symbol"        (dynamic symbol binding)
                        match self.peek().kind.clone() {
                            TokenKind::Identifier => self.rewire_statement(),
                            TokenKind::String => {
                                // rewire "symbol" => call builtin rewire_symbol("symbol")
                                let tok = self.advance().clone();
                                let sym = match tok.value {
                                    Some(TokenValue::String(s)) => s,
                                    _ => tok.lexeme.clone(),
                                };
                                Ok(Stmt::Expression(Expr::FunctionCall {
                                    function: Box::new(Expr::Variable("rewire_symbol".to_string())),
                                    arguments: vec![Expr::String(sym)],
                                }))
                            }
                            _ => Err("Expected identifier or string after rewire".to_string()),
                        }
                    }
                    
                    "write" | "WRITE" => {
                        self.advance(); // consume write
                        self.write_statement()
                    }
                    
                    "read" | "READ" => {
                        self.advance(); // consume read
                        self.read_statement()
                    }
                    
                    "fission" | "FISSION" => {
                        self.advance(); // consume fission
                        self.fission_statement()
                    }
                    
                    "fusion" | "FUSION" => {
                        self.advance(); // consume fusion
                        self.fusion_statement()
                    }
                    
                    "rename" | "RENAME" => {
                        self.advance(); // consume rename
                        self.rename_statement()
                    }
                    
                    "pack" | "PACK" => {
                        self.advance(); // consume pack
                        self.pack_statement()
                    }
                    
                    "place" | "PLACE" => {
                        self.advance(); // consume place
                        self.place_statement()
                    }
                    
                    "unpack" | "UNPACK" => {
                        self.advance(); // consume unpack
                        self.unpack_statement()
                    }
                    
                    "pick" | "PICK" => {
                        self.advance(); // consume pick
                        self.pick_statement()
                    }
                    
                    _ => {
                        debug!("DEBUG: Identifier '{}' not a keyword, checking if assignment", name);
                        // Check if it's an assignment
                        if self.peek_next().kind == TokenKind::Equal {
                            debug!("DEBUG: It's an assignment");
                            self.assignment_statement()
                        } else {
                            debug!("DEBUG: It's an expression statement");
                            // Expression statement
                            let expr = self.expression()?;
                            Ok(Stmt::Expression(expr))
                        }
                    }
                }
            }
            
            TokenKind::String => {
                // String literal - could be assignment target or expression
                if self.peek_next().kind == TokenKind::Equal {
                    self.assignment_statement()
                } else {
                    let expr = self.expression()?;
                    Ok(Stmt::Expression(expr))
                }
            }
            
            TokenKind::LeftBrace => {
                // Block as statement
                let _block = self.block()?;
                Ok(Stmt::Expression(Expr::Group(Box::new(
                    Expr::Binary {
                        left: Box::new(Expr::Number(TokenValue::Int(0))), // dummy
                        operator: "block".to_string(),
                        right: Box::new(Expr::Number(TokenValue::Int(0))), // dummy
                    }
                )))) // TODO: Fix this - blocks should be statements, not expressions
            }
            
            _ => {
                // Expression statement
                let expr = self.expression()?;
                Ok(Stmt::Expression(expr))
            }
        }
    }
    
    fn expression(&mut self) -> Result<Expr, String> {
        self.parse_expression(0)
    }
    
    fn parse_expression(&mut self, min_precedence: u8) -> Result<Expr, String> {
        debug!("DEBUG: parse_expression() - min_precedence: {}", min_precedence);
        let mut left = self.parse_prefix()?;
        
        // Handle postfix arrow operations (like unpack(...) <- box)
        left = self.parse_postfix_arrows(left)?;
        
        while !self.is_at_end() && self.is_operator() && self.get_precedence() >= min_precedence {
            let operator = self.peek().lexeme.clone();
            let precedence = self.get_precedence();
            
            self.advance(); // consume operator
            
            let right = self.parse_expression(precedence + 1)?;
            
            left = Expr::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }
        
        Ok(left)
    }
    
    fn parse_prefix(&mut self) -> Result<Expr, String> {
        let token = self.peek();
        debug!("DEBUG: parse_prefix() - token: {:?} '{}'", token.kind, token.lexeme);
        
        match token.kind {
            // STEP prefix: :.(head) used as either an UNPACK expression or sugar for stepped PICK
            crate::lexer::TokenKind::Step => {
                self.advance(); // consume :.
                self.expect(crate::lexer::TokenKind::LeftParen, "Expected '(' after :.")?;
                let head_expr = self.expression()?;
                self.expect(crate::lexer::TokenKind::RightParen, "Expected ')' after step head")?;

                // Sugar: :.(head) pick(i, ...) <- box  => Pick(indices) <- (Unpack(head) <- box)
                if self.check(crate::lexer::TokenKind::Identifier)
                    && (self.peek().lexeme == "pick" || self.peek().lexeme == "PICK")
                {
                    self.advance(); // consume 'pick'
                    self.expect(crate::lexer::TokenKind::LeftParen, "Expected '(' after pick")?;
                    let indices = self.parse_arguments()?; // parses until ')'
                    if !self.match_token(crate::lexer::TokenKind::ArrowLeft) {
                        return Err("Expected '<-' after pick indices".to_string());
                    }
                    let box_expr = self.expression()?;
                    let stepped_box = Expr::Unpack {
                        box_expr: Box::new(box_expr),
                        start: Box::new(head_expr),
                        end: None,
                    };
                    return Ok(Expr::Pick {
                        box_expr: Box::new(stepped_box),
                        indices,
                    });
                }

                // Expression form: :.(head) <- box  => UNPACK(box, head)
                if self.match_token(crate::lexer::TokenKind::ArrowLeft) {
                    let box_expr = self.expression()?;
                    return Ok(Expr::Unpack {
                        box_expr: Box::new(box_expr),
                        start: Box::new(head_expr),
                        end: None,
                    });
                }

                Err("Expected '<-' or 'pick(...) <-' after :.(...)".to_string())
            }
            TokenKind::Number => {
                let value = token.value.clone().unwrap();
                self.advance();
                Ok(Expr::Number(value))
            }
            
            TokenKind::String => {
                let value = match &token.value {
                    Some(TokenValue::String(s)) => s.clone(),
                    _ => token.lexeme.clone(),
                };
                self.advance();
                Ok(Expr::String(value))
            }
            
            TokenKind::Identifier => {
                let name = token.lexeme.clone();
                
                // Check for boolean literals
                match name.as_str() {
                    "TRUE" => {
                        self.advance();
                        Ok(Expr::Bool(true))
                    }
                    "FALSE" => {
                        self.advance();
                        Ok(Expr::Bool(false))
                    }
                    "string" => {
                        self.advance();
                        Ok(Expr::String("".to_string()))
                    }
                    _ => {
                        self.advance();
                        
                        // Special handling for PICK expressions
                        if name == "pick" || name == "PICK" {
                            if self.match_token(TokenKind::LeftParen) {
                                let mut indices = Vec::new();
                                
                                if !self.match_token(TokenKind::RightParen) {
                                    loop {
                                        // Parse the index expression
                                        let index_expr = self.expression()?;
                                        
                                        // This is a regular index expression
                                        indices.push(index_expr);
                                        
                                        if self.match_token(TokenKind::RightParen) {
                                            break;
                                        }
                                        
                                        // Allow commas between different pick operations
                                        if !self.match_token(TokenKind::Comma) {
                                            return Err("Expected ',' or ')' in pick indices".to_string());
                                        }
                                    }
                                }
                                
                                if !self.match_token(TokenKind::ArrowLeft) {
                                    return Err("Expected '<-' after pick indices".to_string());
                                }
                                
                                let box_expr = self.expression()?;
                                
                                Ok(Expr::Pick {
                                    box_expr: Box::new(box_expr),
                                    indices,
                                })
                            } else {
                                // Not a PICK call, treat as regular variable
                                Ok(Expr::Variable(name))
                            }
                        } else {
                            // Check for function call
                            if self.match_token(TokenKind::LeftParen) {
                                let mut arguments = self.parse_arguments()?;
                                // Special-case: import(...) treats bare identifiers as strings for convenience
                                if name == "import" || name == "IMPORT" {
                                    for arg in arguments.iter_mut() {
                                        if let Expr::Variable(var_name) = arg.clone() {
                                            *arg = Expr::String(var_name);
                                        }
                                    }
                                }
                                Ok(Expr::FunctionCall { function: Box::new(Expr::Variable(name)), arguments })
                            } else {
                                Ok(Expr::Variable(name))
                            }
                        }
                    }
                }
            }
            
            TokenKind::LeftParen => {
                self.advance(); // consume (
                let expr = self.expression()?;
                self.expect(TokenKind::RightParen, "Expected ')' after expression")?;
                Ok(Expr::Group(Box::new(expr)))
            }
            
            TokenKind::LeftBracket => {
                self.advance(); // consume [
                let elements = self.parse_list_elements()?;
                self.expect(TokenKind::RightBracket, "Expected ']' after list")?;
                Ok(Expr::List(elements))
            }
            
            TokenKind::Minus => {
                self.advance(); // consume -
                let operand = self.parse_expression(4)?; // higher precedence than *
                Ok(Expr::Unary {
                    operator: "-".to_string(),
                    operand: Box::new(operand),
                })
            }
            
            TokenKind::Not => {
                self.advance(); // consume !
                let operand = self.parse_expression(4)?; // high precedence
                Ok(Expr::Unary {
                    operator: "!".to_string(),
                    operand: Box::new(operand),
                })
            }
            
            _ => {
                Err(format!("Unexpected token {:?} at line {}", token.kind, token.line))
            }
        }
    }
    
    fn parse_postfix_arrows(&mut self, mut left: Expr) -> Result<Expr, String> {
        // Handle postfix arrow operations like unpack(...) <- box
        while !self.is_at_end() {
            match self.peek().kind {
                TokenKind::ArrowLeft => {
                    // Handle read-from arrow: unpack(...) <- box
                    if let Expr::FunctionCall { function, arguments } = &left {
                        if let Expr::Variable(name) = &**function {
                            match name.as_str() {
                                "import" | "IMPORT" => {
                                    // Support: import(name1, name2, ...) <- src
                                    self.advance(); // consume '<-'
                                    let mut src_expr = self.expression()?;

                                    // If src is a bare identifier, treat it as a string key
                                    if let Expr::Variable(var_name) = src_expr {
                                        src_expr = Expr::String(var_name);
                                    }

                                    // Convert any bare identifiers on left to strings
                                    let mut new_args: Vec<Expr> = Vec::new();
                                    for a in arguments.iter() {
                                        match a {
                                            Expr::Variable(n) => new_args.push(Expr::String(n.clone())),
                                            _ => new_args.push(a.clone()),
                                        }
                                    }
                                    // Append source key as last argument
                                    new_args.push(src_expr);

                                    left = Expr::FunctionCall {
                                        function: Box::new(Expr::Variable("import".to_string())),
                                        arguments: new_args,
                                    };
                                    continue;
                                }
                                "unpack" | "UNPACK" => {
                                    self.advance(); // consume '<-'
                                    let box_expr = self.expression()?;
                                    
                                    // Extract start and end from arguments
                                    let start = if arguments.len() > 0 {
                                        arguments[0].clone()
                                    } else {
                                        return Err("UNPACK expects at least one argument".to_string());
                                    };
                                    
                                    let end = if arguments.len() > 1 {
                                        Some(arguments[1].clone())
                                    } else {
                                        None
                                    };
                                    
                                    left = Expr::Unpack {
                                        box_expr: Box::new(box_expr),
                                        start: Box::new(start),
                                        end: end.map(|e| Box::new(e)),
                                    };
                                    continue;
                                }
                                "count" | "COUNT" => {
                                    self.advance(); // consume '<-'
                                    let box_expr = self.expression()?;

                                    // Build the argument to count based on provided args
                                    let count_arg = match arguments.len() {
                                        0 => box_expr, // count(box)
                                        1 => Expr::Unpack {
                                            box_expr: Box::new(box_expr),
                                            start: Box::new(arguments[0].clone()),
                                            end: None,
                                        },
                                        2 => Expr::Unpack {
                                            box_expr: Box::new(box_expr),
                                            start: Box::new(arguments[0].clone()),
                                            end: Some(Box::new(arguments[1].clone())),
                                        },
                                        _ => return Err("COUNT expects 0, 1, or 2 indices before '<-'".to_string()),
                                    };

                                    left = Expr::FunctionCall {
                                        function: Box::new(Expr::Variable("count".to_string())),
                                        arguments: vec![count_arg],
                                    };
                                    continue;
                                }
                                "pick" | "PICK" => {
                                    self.advance(); // consume '<-'
                                    let box_expr = self.expression()?;
                                    
                                    left = Expr::Pick {
                                        box_expr: Box::new(box_expr),
                                        indices: arguments.clone(),
                                    };
                                    continue;
                                }
                                "fission" | "FISSION" => {
                                    // FISSION(delim) <- string
                                    self.advance(); // consume '<-'
                                    let string_expr = self.expression()?;
                                    left = Expr::FunctionCall {
                                        function: Box::new(Expr::Variable("fission".to_string())),
                                        arguments: vec![
                                            if arguments.len() > 0 { arguments[0].clone() } else { return Err("FISSION expects a delimiter argument".to_string()) },
                                            string_expr,
                                        ],
                                    };
                                    continue;
                                }
                                _ => {
                                    // Not a valid arrow operation, break
                                    // fall through to check for READ <- filename when left is a variable
                                }
                            }
                        } else {
                            // Not a variable function call, break
                            // fall through to check for READ <- filename when left is a variable
                        }
                    }
                    // Support READ <- filename when left is a variable named READ/read
                    if let Expr::Variable(var_name) = &left {
                        if var_name == "READ" || var_name == "read" {
                            self.advance(); // consume '<-'
                            let filename = self.expression()?;
                            left = Expr::FunctionCall {
                                function: Box::new(Expr::Variable("read".to_string())),
                                arguments: vec![filename],
                            };
                            continue;
                        }
                    }
                    // Otherwise, not an arrow form we support here
                    break;
                }
                TokenKind::ArrowRight => {
                    // Handle write-to arrow for fusion(... ) -> strings
                    if let Expr::FunctionCall { function, arguments } = &left {
                        if let Expr::Variable(name) = &**function {
                            match name.as_str() {
                                "fusion" | "FUSION" => {
                                    self.advance(); // consume '->'
                                    let strings_expr = self.expression()?;
                                    left = Expr::FunctionCall {
                                        function: Box::new(Expr::Variable("fusion".to_string())),
                                        arguments: vec![
                                            if arguments.len() > 0 { arguments[0].clone() } else { return Err("FUSION expects a delimiter argument".to_string()) },
                                            strings_expr,
                                        ],
                                    };
                                    continue;
                                }
                                "write" | "WRITE" => {
                                    // WRITE(content) -> filename in expression context
                                    self.advance(); // consume '->'
                                    let filename_expr = self.expression()?;
                                    let mut args = arguments.clone();
                                    if args.len() != 1 { return Err("WRITE expects exactly one content argument before '->'".to_string()); }
                                    args.push(filename_expr);
                                    left = Expr::FunctionCall {
                                        function: Box::new(Expr::Variable("write".to_string())),
                                        arguments: args,
                                    };
                                    continue;
                                }
                                _ => {}
                            }
                        }
                    }
                    // For other write arrows, not handled as expressions
                    break;
                }
                _ => {
                    // Not an arrow, break
                    break;
                }
            }
        }
        
        Ok(left)
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr>, String> {
        let mut arguments = Vec::new();
        
        if !self.match_token(TokenKind::RightParen) {
            loop {
                arguments.push(self.expression()?);
                
                if self.match_token(TokenKind::RightParen) {
                    break;
                }
                
                if !self.match_token(TokenKind::Comma) {
                    return Err("Expected ',' or ')' in argument list".to_string());
                }
            }
        }
        
        Ok(arguments)
    }
    
    fn parse_list_elements(&mut self) -> Result<Vec<Expr>, String> {
        let mut elements = Vec::new();
        
        if !self.match_token(TokenKind::RightBracket) {
            loop {
                elements.push(self.expression()?);
                
                if self.match_token(TokenKind::RightBracket) {
                    break;
                }
                
                if !self.match_token(TokenKind::Comma) {
                    return Err("Expected ',' or ']' in list".to_string());
                }
            }
        }
        
        Ok(elements)
    }
    
    fn block(&mut self) -> Result<Block, String> {
        debug!("DEBUG: Starting block parsing");
        self.expect(TokenKind::LeftBrace, "Expected '{' to start block")?;

        let mut statements = Vec::new();

        // Parse statements until we see a closing '}'
        while !self.is_at_end() && !self.check(TokenKind::RightBrace) {
            debug!("DEBUG: Block parsing - current token: {:?} '{}'", self.peek().kind, self.peek().lexeme);
            // Skip leading newlines between statements
            while self.match_token(TokenKind::Newline) {
                // consume newline
            }

            if self.is_at_end() {
                return Err("Unterminated block - missing '}'".to_string());
            }

            // If we arrived at a '}', end the block (don't consume here)
            if self.check(TokenKind::RightBrace) {
                break;
            }

            let stmt = self.statement()?;
            statements.push(stmt);

            // Consume any trailing newlines after the statement; do not consume '}' here.
            while self.match_token(TokenKind::Newline) {
                // consume newline(s)
            }
        }

        // Now consume the closing '}' for this block
        self.expect(TokenKind::RightBrace, "Expected '}' to end block")?;

        Ok(Block { statements })
    }
    
    // ============================================================================
    // Statement Parsers
    // ============================================================================
    
    fn assignment_statement(&mut self) -> Result<Stmt, String> {
        let lhs_kind = self.peek().kind.clone();
        let lhs_lexeme = self.peek().lexeme.clone();
        let lhs_value = self.peek().value.clone();
        self.advance(); // consume LHS
        
        self.expect(TokenKind::Equal, "Expected '=' in assignment")?;
        
        // Check if the right-hand side is a pick statement
        let value = if self.peek().kind == TokenKind::Identifier && 
                      (self.peek().lexeme == "pick" || self.peek().lexeme == "PICK") {
            // Parse as pick statement
            self.advance(); // consume pick
            let pick_stmt = self.pick_statement()?;
            // Convert pick statement to expression
            match pick_stmt {
                Stmt::Expression(expr) => expr,
                _ => return Err("Pick statement must be an expression".to_string()),
            }
        } else {
            self.expression()?
        };
        
        let name = match lhs_kind {
            TokenKind::Identifier => lhs_lexeme,
            TokenKind::String => match lhs_value {
                Some(TokenValue::String(s)) => s,
                _ => lhs_lexeme,
            },
            _ => return Err("Invalid assignment target".to_string()),
        };
        
        Ok(Stmt::Assignment { name, value })
    }
    
    fn function_definition(&mut self) -> Result<Stmt, String> {
        debug!("DEBUG: Starting function definition");
        self.expect(TokenKind::LeftParen, "Expected '(' after fn")?;
        
        // Parse use(params)
        self.expect_keyword("use")?;
        self.expect(TokenKind::LeftParen, "Expected '(' after use")?;
        
        let mut parameters = Vec::new();
        if !self.match_token(TokenKind::RightParen) {
            loop {
                let param = self.expect(TokenKind::Identifier, "Expected parameter name")?;
                parameters.push(param.lexeme.clone());
                
                if self.match_token(TokenKind::RightParen) {
                    break;
                }
                
                if !self.match_token(TokenKind::Comma) {
                    return Err("Expected ',' or ')' in parameter list".to_string());
                }
            }
        }
        
        self.expect(TokenKind::RightParen, "Expected ')' after function parameters")?;
        
        let body = self.block()?;
        
        debug!("DEBUG: Checking for arrow after function body");
        let name = if self.match_token(TokenKind::ArrowRight) {
            debug!("DEBUG: Found arrow, expecting function name");
            let dest = self.expect(TokenKind::Identifier, "Expected function name after ->")?;
            debug!("DEBUG: Function name: {}", dest.lexeme);
            dest.lexeme.clone()
        } else {
            debug!("DEBUG: No arrow found, using anonymous name");
            "_anon".to_string()
        };
        
        Ok(Stmt::FunctionDef {
            name,
            parameters,
            body,
        })
    }
    
    fn rewire_statement(&mut self) -> Result<Stmt, String> {
        let target_lexeme = self.expect(TokenKind::Identifier, "Expected target variable after rewire")?.lexeme.clone();
        let body = self.block()?;
        
        if !self.match_token(TokenKind::ArrowRight) {
            return Err("Expected '->' after rewire block".to_string());
        }
        
        let destination_lexeme = self.expect(TokenKind::Identifier, "Expected destination after ->")?.lexeme.clone();
        
        Ok(Stmt::Rewire {
            _target: target_lexeme,
            body,
            destination: destination_lexeme,
        })
    }
    
    fn write_statement(&mut self) -> Result<Stmt, String> {
        self.expect(TokenKind::LeftParen, "Expected '(' after write")?;
        let content = self.expression()?;
        self.expect(TokenKind::RightParen, "Expected ')' after write content")?;
        
        if !self.match_token(TokenKind::ArrowRight) {
            return Err("Expected '->' after write content".to_string());
        }
        
        let filename = self.expression()?;
        
        Ok(Stmt::Write { content, filename })
    }
    
    fn read_statement(&mut self) -> Result<Stmt, String> {
        // read <- filename
        if !self.match_token(TokenKind::ArrowLeft) {
            return Err("Expected '<-' after read".to_string());
        }
        
        let filename = self.expression()?;
        
        Ok(Stmt::Read { filename })
    }
    
    fn fission_statement(&mut self) -> Result<Stmt, String> {
        self.expect(TokenKind::LeftParen, "Expected '(' after fission")?;
        let delimiter = self.expression()?;
        self.expect(TokenKind::RightParen, "Expected ')' after fission delimiter")?;
        
        if !self.match_token(TokenKind::ArrowLeft) {
            return Err("Expected '<-' after fission delimiter".to_string());
        }
        
        let string = self.expression()?;
        
        Ok(Stmt::Fission { delimiter, string })
    }
    
    fn fusion_statement(&mut self) -> Result<Stmt, String> {
        self.expect(TokenKind::LeftParen, "Expected '(' after fusion")?;
        let delimiter = self.expression()?;
        self.expect(TokenKind::RightParen, "Expected ')' after fusion delimiter")?;
        
        if !self.match_token(TokenKind::ArrowRight) {
            return Err("Expected '->' after fusion delimiter".to_string());
        }
        
        let strings = self.expression()?;
        
        Ok(Stmt::Fusion { delimiter, strings })
    }

    fn rename_statement(&mut self) -> Result<Stmt, String> {
        // Syntax: rename(from) -> to
        self.expect(TokenKind::LeftParen, "Expected '(' after rename")?;
        let mut from = self.expression()?;
        // Treat bare identifiers as string names for source
        if let Expr::Variable(name) = from { from = Expr::String(name); }
        self.expect(TokenKind::RightParen, "Expected ')' after rename source")?;

        if !self.match_token(TokenKind::ArrowRight) {
            return Err("Expected '->' after rename source".to_string());
        }

        let mut to = self.expression()?;
        // Treat bare identifiers as string names for destination
        if let Expr::Variable(name) = to {
            to = Expr::String(name);
        }

        Ok(Stmt::Rename { from, to })
    }
    
    fn pack_statement(&mut self) -> Result<Stmt, String> {
        self.expect(TokenKind::LeftParen, "Expected '(' after pack")?;
        
        let mut values = Vec::new();
        if !self.match_token(TokenKind::RightParen) {
            loop {
                values.push(self.expression()?);
                
                if self.match_token(TokenKind::RightParen) {
                    break;
                }
                
                if !self.match_token(TokenKind::Comma) {
                    return Err("Expected ',' or ')' in pack values".to_string());
                }
            }
        }
        
        if !self.match_token(TokenKind::ArrowRight) {
            return Err("Expected '->' after pack values".to_string());
        }
        
        let target = self.expression()?;
        
        Ok(Stmt::Pack { values, target })
    }
    
    fn place_statement(&mut self) -> Result<Stmt, String> {
        self.expect(TokenKind::LeftParen, "Expected '(' after place")?;
        
        let mut pairs = Vec::new();
        if !self.match_token(TokenKind::RightParen) {
            loop {
                let index = self.expression()?;
                self.expect(TokenKind::Colon, "Expected ':' in place statement")?;
                let value = self.expression()?;
                
                pairs.push((index, value));
                
                if self.match_token(TokenKind::RightParen) {
                    break;
                }
                
                if !self.match_token(TokenKind::Comma) {
                    return Err("Expected ',' or ')' in place pairs".to_string());
                }
            }
        }
        
        if !self.match_token(TokenKind::ArrowRight) {
            return Err("Expected '->' after place pairs".to_string());
        }
        
        let target = self.expression()?;
        
        Ok(Stmt::Place { pairs, target })
    }
    
    fn unpack_statement(&mut self) -> Result<Stmt, String> {
        self.expect(TokenKind::LeftParen, "Expected '(' after unpack")?;
        
        let start = self.expression()?;
        let end = if self.match_token(TokenKind::Comma) {
            Some(self.expression()?)
        } else {
            None
        };
        
        self.expect(TokenKind::RightParen, "Expected ')' after unpack indices")?;
        
        if !self.match_token(TokenKind::ArrowLeft) {
            return Err("Expected '<-' after unpack indices".to_string());
        }
        
        let box_expr = self.expression()?;
        
        Ok(Stmt::Expression(Expr::Unpack {
            box_expr: Box::new(box_expr),
            start: Box::new(start),
            end: end.map(|e| Box::new(e)),
        }))
    }
    
    fn pick_statement(&mut self) -> Result<Stmt, String> {
        // debug removed
        self.expect(TokenKind::LeftParen, "Expected '(' after pick")?;
        // debug removed
        
        let mut indices = Vec::new();
        if !self.match_token(TokenKind::RightParen) {
            loop {
                // debug removed
                
                // Parse the index expression first
                let index_expr = self.expression()?;
                // debug removed
                
                // This is a regular index expression
                indices.push(index_expr);
                

                
                // debug removed
                if self.match_token(TokenKind::RightParen) {
                    // debug removed
                    break;
                }
                
                // Expect comma between indices
                // debug removed
                if !self.match_token(TokenKind::Comma) {
                    return Err(format!("Expected ',' or ')' in pick indices, got {:?}", self.peek()));
                }
            }
        }
        
        // debug removed
        if !self.match_token(TokenKind::ArrowLeft) {
            return Err("Expected '<-' after pick indices".to_string());
        }
        // debug removed
        
        let box_expr = self.expression()?;
        // debug removed
        
        Ok(Stmt::Expression(Expr::Pick {
            box_expr: Box::new(box_expr),
            indices,
        }))
    }
    

    
    // ============================================================================
    // Helper Methods
    // ============================================================================
    
    fn get_precedence(&self) -> u8 {
        if self.is_at_end() {
            return 0;
        }
        
        match self.peek().lexeme.as_str() {
            "*" | "/" => 3,
            "+" | "-" => 2,
            "<" | ">" | "<=" | ">=" | "==" | "!=" => 1,
            ">|" | "|<" => 0,
            _ => 0,
        }
    }
    
    fn is_operator(&self) -> bool {
        if self.is_at_end() {
            return false;
        }
        
        matches!(self.peek().lexeme.as_str(), 
            "*" | "/" | "+" | "-" | "<" | ">" | "<=" | ">=" | "==" | "!=" | ">|" | "|<")
    }
    
    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }
    
    fn match_keyword(&mut self, keyword: &str) -> bool {
        if self.check(TokenKind::Identifier) && self.peek().lexeme == keyword {
            self.advance();
            true
        } else {
            false
        }
    }
    
    fn expect_keyword(&mut self, keyword: &str) -> Result<&Token, String> {
        if self.check(TokenKind::Identifier) && self.peek().lexeme == keyword {
            Ok(self.advance())
        } else {
            Err(format!("Expected keyword '{}'", keyword))
        }
    }
    
    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<&Token, String> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(format!("{} at line {}", message, self.peek().line))
        }
    }
    
    fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() {
            false
        } else {
            self.peek().kind == kind
        }
    }
    
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }
    
    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::EOF
    }
    
    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }
    
    fn peek_next(&self) -> &Token {
        if self.current + 1 < self.tokens.len() {
            &self.tokens[self.current + 1]
        } else {
            &self.tokens[self.current] // fallback to current
        }
    }
    
    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }
}
