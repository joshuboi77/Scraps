
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Number,
    String,
    Identifier,
    
    // Operators
    Plus,      // +
    Minus,     // -
    Star,      // *
    Slash,     // /
    Equal,     // =
    Less,      // <
    Greater,   // >
    NotEqual,  // !=
    LessEqual, // <=
    GreaterEqual, // >=
    EqualEqual,    // ==
    Or,        // >|
    And,       // |<
    Not,       // !
    
    // STEP operator
    Step,      // :.
    
    // Arrows
    ArrowRight, // ->
    ArrowLeft,  // <-
    

    
    // Groupers
    LeftParen,   // (
    RightParen,  // )
    LeftBrace,   // {
    RightBrace,  // }
    LeftBracket, // [
    RightBracket, // ]
    Comma,       // ,
    Colon,       // :
    
    // Special
    Newline,
    EOF,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub value: Option<TokenValue>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub enum TokenValue {
    Int(i64),
    Float(f64),
    String(String),
}

pub struct Lexer {
    source: String,
    position: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        Self {
            source,
            position: 0,
            line: 1,
            column: 1,
        }
    }
    
    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        
        while self.position < self.source.len() {
            let ch = self.peek_char(0)?;
            
            // Skip whitespace (but keep newlines)
            if ch.is_whitespace() && ch != '\n' {
                self.advance()?;
                continue;
            }
            
            // Handle comments
            if ch == '#' {
                self.tokenize_comment(&mut tokens)?;
                continue;
            }
            
            // Handle newlines
            if ch == '\n' {
                tokens.push(Token {
                    kind: TokenKind::Newline,
                    lexeme: "\n".to_string(),
                    value: None,
                    line: self.line,
                    column: self.column,
                });
                self.advance()?;
                self.line += 1;
                self.column = 1;
                continue;
            }
            
            // Handle two-character operators first
            if let Some(token) = self.try_two_char_operator()? {
                tokens.push(token);
                continue;
            }
            
            // Handle single character operators and groupers
            if let Some(token) = self.try_single_char()? {
                tokens.push(token);
                continue;
            }
            
            // Handle strings
            if ch == '"' {
                let token = self.tokenize_string()?;
                tokens.push(token);
                continue;
            }
            
            // Handle numbers
            if ch.is_ascii_digit() {
                let token = self.tokenize_number()?;
                tokens.push(token);
                continue;
            }
            
            // Handle identifiers
            if self.is_identifier_start(ch) {
                let token = self.tokenize_identifier()?;
                tokens.push(token);
                continue;
            }
            
            return Err(format!("Unexpected character '{}' at line {}:{}", ch, self.line, self.column));
        }
        
        // Add EOF token
        tokens.push(Token {
            kind: TokenKind::EOF,
            lexeme: "".to_string(),
            value: None,
            line: self.line,
            column: self.column,
        });
        
        Ok(tokens)
    }
    
    fn try_two_char_operator(&mut self) -> Result<Option<Token>, String> {
        if self.position + 1 >= self.source.len() {
            return Ok(None);
        }
        
        let ch1 = self.peek_char(0)?;
        let ch2 = self.peek_char(1)?;
        let two_char = format!("{}{}", ch1, ch2);
        
        let token = match two_char.as_str() {
            ":." => Some(Token {
                kind: TokenKind::Step,
                lexeme: ":.".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            "->" => Some(Token {
                kind: TokenKind::ArrowRight,
                lexeme: "->".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            "<-" => Some(Token {
                kind: TokenKind::ArrowLeft,
                lexeme: "<-".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),

            
            "!=" => Some(Token {
                kind: TokenKind::NotEqual,
                lexeme: "!=".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            "<=" => Some(Token {
                kind: TokenKind::LessEqual,
                lexeme: "<=".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            ">=" => Some(Token {
                kind: TokenKind::GreaterEqual,
                lexeme: ">=".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            "==" => Some(Token {
                kind: TokenKind::EqualEqual,
                lexeme: "==".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            ">|" => Some(Token {
                kind: TokenKind::Or,
                lexeme: ">|".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            "|<" => Some(Token {
                kind: TokenKind::And,
                lexeme: "|<".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),

            _ => None,
        };
        
        if token.is_some() {
            self.advance()?;
            self.advance()?;
        }
        
        Ok(token)
    }
    
    fn try_single_char(&mut self) -> Result<Option<Token>, String> {
        let ch = self.peek_char(0)?;
        
        let token = match ch {
            '+' => Some(Token {
                kind: TokenKind::Plus,
                lexeme: "+".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '-' => Some(Token {
                kind: TokenKind::Minus,
                lexeme: "-".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '*' => Some(Token {
                kind: TokenKind::Star,
                lexeme: "*".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '/' => Some(Token {
                kind: TokenKind::Slash,
                lexeme: "/".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '=' => Some(Token {
                kind: TokenKind::Equal,
                lexeme: "=".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '<' => Some(Token {
                kind: TokenKind::Less,
                lexeme: "<".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '>' => Some(Token {
                kind: TokenKind::Greater,
                lexeme: ">".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '!' => Some(Token {
                kind: TokenKind::Not,
                lexeme: "!".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '(' => Some(Token {
                kind: TokenKind::LeftParen,
                lexeme: "(".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            ')' => Some(Token {
                kind: TokenKind::RightParen,
                lexeme: ")".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '{' => Some(Token {
                kind: TokenKind::LeftBrace,
                lexeme: "{".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '}' => Some(Token {
                kind: TokenKind::RightBrace,
                lexeme: "}".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            '[' => Some(Token {
                kind: TokenKind::LeftBracket,
                lexeme: "[".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            ']' => Some(Token {
                kind: TokenKind::RightBracket,
                lexeme: "]".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            ',' => Some(Token {
                kind: TokenKind::Comma,
                lexeme: ",".to_string(),
                value: None,
                line: self.line,
                column: self.column,
            }),
            ':' => {
                {
                    Some(Token {
                        kind: TokenKind::Colon,
                        lexeme: ":".to_string(),
                        value: None,
                        line: self.line,
                        column: self.column,
                    })
                }
            },
            _ => None,
        };
        
        if token.is_some() {
            self.advance()?;
        }
        
        Ok(token)
    }
    
    fn tokenize_comment(&mut self, _tokens: &mut Vec<Token>) -> Result<(), String> {
        // Skip until newline or EOF
        while self.position < self.source.len() {
            let ch = self.peek_char(0)?;
            if ch == '\n' {
                break;
            }
            self.advance()?;
        }
        Ok(())
    }
    
    fn tokenize_string(&mut self) -> Result<Token, String> {
        let start_line = self.line;
        let start_column = self.column;
        
        self.advance()?; // consume opening quote
        
        let mut value = String::new();
        let mut lexeme = String::from("\"");
        
        while self.position < self.source.len() {
            let ch = self.peek_char(0)?;
            
            if ch == '"' {
                lexeme.push(ch);
                self.advance()?;
                break;
            }
            
            if ch == '\\' {
                self.advance()?; // consume backslash
                let esc = self.peek_char(0)?;
                let escaped = match esc {
                    'n' => '\n',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    _ => esc, // unknown escape, keep as-is
                };
                value.push(escaped);
                lexeme.push('\\');
                lexeme.push(esc);
                self.advance()?;
                continue;
            }
            
            value.push(ch);
            lexeme.push(ch);
            self.advance()?;
        }
        
        if self.position >= self.source.len() {
            return Err(format!("Unterminated string at line {}:{}", start_line, start_column));
        }
        
        Ok(Token {
            kind: TokenKind::String,
            lexeme,
            value: Some(TokenValue::String(value)),
            line: start_line,
            column: start_column,
        })
    }
    
    fn tokenize_number(&mut self) -> Result<Token, String> {
        let start_line = self.line;
        let start_column = self.column;
        
        let mut lexeme = String::new();
        let mut has_dot = false;
        
        while self.position < self.source.len() {
            let ch = self.peek_char(0)?;
            
            if ch.is_ascii_digit() {
                lexeme.push(ch);
                self.advance()?;
            } else if ch == '.' && !has_dot {
                // Check if next character is a digit
                if self.position + 1 < self.source.len() && self.peek_char(1)?.is_ascii_digit() {
                    has_dot = true;
                    lexeme.push(ch);
                    self.advance()?;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        
        let value = if has_dot {
            lexeme.parse::<f64>()
                .map(TokenValue::Float)
                .map_err(|_| format!("Invalid float literal '{}'", lexeme))?
        } else {
            lexeme.parse::<i64>()
                .map(TokenValue::Int)
                .map_err(|_| format!("Invalid integer literal '{}'", lexeme))?
        };
        
        Ok(Token {
            kind: TokenKind::Number,
            lexeme,
            value: Some(value),
            line: start_line,
            column: start_column,
        })
    }
    
    fn tokenize_identifier(&mut self) -> Result<Token, String> {
        let start_line = self.line;
        let start_column = self.column;
        
        let mut lexeme = String::new();
        
        while self.position < self.source.len() {
            let ch = self.peek_char(0)?;
            
            if self.is_identifier_part(ch) {
                lexeme.push(ch);
                self.advance()?;
            } else {
                break;
            }
        }
        
        Ok(Token {
            kind: TokenKind::Identifier,
            lexeme,
            value: None,
            line: start_line,
            column: start_column,
        })
    }
    
    fn is_identifier_start(&self, ch: char) -> bool {
        ch.is_alphabetic() || ch == '_'
    }
    
    fn is_identifier_part(&self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }
    
    fn peek_char(&self, offset: usize) -> Result<char, String> {
        let pos = self.position + offset;
        if pos >= self.source.len() {
            return Err("Unexpected end of input".to_string());
        }
        Ok(self.source.chars().nth(pos).unwrap())
    }
    
    fn advance(&mut self) -> Result<char, String> {
        let ch = self.peek_char(0)?;
        self.position += 1;
        if ch != '\n' {
            self.column += 1;
        }
        Ok(ch)
    }
}
