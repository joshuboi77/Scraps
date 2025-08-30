

use crate::bytecode::OpCode;
use crate::parser::{Expr, Stmt, Block};
use crate::lexer::TokenValue;
use std::collections::HashMap;

pub struct Compiler {
    program: Vec<OpCode>,
    labels: HashMap<String, usize>,
    label_counter: usize,
    jump_patches: Vec<(usize, String)>, // (instruction_index, label_name) pairs to patch
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            program: Vec::new(),
            labels: HashMap::new(),
            label_counter: 0,
            jump_patches: Vec::new(),
        }
    }

    pub fn compile(&mut self, statements: Vec<Stmt>) -> Vec<OpCode> {
        self.program.clear();
        self.labels.clear();
        self.label_counter = 0;
        self.jump_patches.clear();
        
        // First pass: compile all statements and collect labels
        for stmt in statements {
            self.compile_statement(stmt);
        }
        
        // Patch all jump targets with correct positions
        self.patch_jumps();
        
        self.program.push(OpCode::Halt);
        self.program.clone()
    }

    fn compile_statement(&mut self, stmt: Stmt) {
        match stmt {
            Stmt::Print(expr) => {
                self.compile_expression(expr);
                self.program.push(OpCode::Print);
            }
            
            Stmt::Test(expr) => {
                self.compile_expression(expr);
                // TODO: Implement test statement (print TRUE/FALSE)
                self.program.push(OpCode::Print);
            }
            
            Stmt::Expression(expr) => {
                self.compile_expression(expr);
                // Pop the result since we don't need it
                // TODO: Add Pop opcode if needed
            }
            
            Stmt::Assignment { name, value } => {
                self.compile_expression(value);
                self.program.push(OpCode::StoreVar(name));
            }
            
            Stmt::If { condition, then_block, else_block } => {
                let current_counter = self.label_counter;
                self.label_counter += 1;
                
                // Compile condition
                self.compile_expression(condition);
                
                // Jump to else block if condition is false
                let else_label = format!("if_else_{}", current_counter);
                self.program.push(OpCode::JumpIfNot(0)); // Placeholder
                self.jump_patches.push((self.program.len() - 1, else_label.clone()));
                
                // Compile then block
                self.compile_block(then_block);
                
                // Jump to end (skip else block)
                let end_label = format!("if_end_{}", current_counter);
                self.program.push(OpCode::Jump(0)); // Placeholder
                self.jump_patches.push((self.program.len() - 1, end_label.clone()));
                
                // Compile else block
                self.program.push(OpCode::Label(else_label));
                self.compile_block(else_block);
                
                // End label
                self.program.push(OpCode::Label(end_label));
            }
            
            Stmt::While { condition, body } => {
                let current_counter = self.label_counter;
                self.label_counter += 1;
                
                // Condition label
                let cond_label = format!("while_cond_{}", current_counter);
                self.program.push(OpCode::Label(cond_label.clone()));
                
                // Compile condition
                self.compile_expression(condition);
                
                // Jump to end if condition is false
                let end_label = format!("while_end_{}", current_counter);
                self.program.push(OpCode::JumpIfNot(0)); // Placeholder
                self.jump_patches.push((self.program.len() - 1, end_label.clone()));
                
                // Compile body
                let body_label = format!("while_body_{}", current_counter);
                self.program.push(OpCode::Label(body_label));
                self.compile_block(body);
                
                // Jump back to condition
                self.program.push(OpCode::Jump(0)); // Placeholder
                self.jump_patches.push((self.program.len() - 1, cond_label.clone()));
                
                // End label
                self.program.push(OpCode::Label(end_label));
            }
            
            Stmt::Pack { values, target } => {
                // Load target box
                self.compile_expression(target.clone());
                
                // For each value, pack it into the box
                for value in values {
                    self.compile_expression(value);
                    self.program.push(OpCode::Pack);
                }
                
                // Store the updated box back to the variable
                if let Expr::Variable(name) = target {
                    self.program.push(OpCode::StoreVar(name));
                } else {
                    // TODO: Handle non-variable targets
                    panic!("Pack target must be a variable for now");
                }
            }
            
            Stmt::Place { pairs, target } => {
                // Load target box
                self.compile_expression(target);
                
                // For each (index, value) pair
                for (index, value) in pairs {
                    self.compile_expression(index.clone());
                    self.compile_expression(value);
                    // TODO: Implement Place with dynamic index
                    // For now, assume index is constant
                    if let Expr::Number(TokenValue::Int(idx)) = index {
                        self.program.push(OpCode::Place(idx as usize));
                    } else {
                        // TODO: Handle dynamic indices
                        panic!("Dynamic indices not yet implemented");
                    }
                }
            }
            
            Stmt::FunctionDef { name, parameters, body } => {
                // Compile the function body into bytecode
                let mut body_compiler = Compiler::new();
                body_compiler.compile_block(body);
                let function_body = body_compiler.program;
                
                // Use the new MakeFuncWithBody OpCode to store the compiled function body
                self.program.push(OpCode::MakeFuncWithBody {
                    name: name.clone(),
                    parameters: parameters.clone(),
                    body: function_body,
                    rewire_target: None,
                });
            }
            
            Stmt::Rewire { _target, body, destination } => {
                // Compile rewire block into a function body; store as a function with a rewire target
                let mut body_compiler = Compiler::new();
                body_compiler.compile_block(body);
                let function_body = body_compiler.program;
                self.program.push(OpCode::MakeFuncWithBody {
                    name: destination.clone(),
                    parameters: vec![],
                    body: function_body,
                    rewire_target: Some(_target.clone()),
                });
            }
            
            Stmt::Write { content, filename } => {
                // Compile as built-in function call: write(content, filename)
                self.compile_expression(content);
                self.compile_expression(filename);
                self.program.push(OpCode::PushStr("write".to_string()));
                self.program.push(OpCode::Call("".to_string(), 2));
            }
            
            Stmt::Read { filename } => {
                // Compile as built-in function call: read(filename)
                self.compile_expression(filename);
                self.program.push(OpCode::PushStr("read".to_string()));
                self.program.push(OpCode::Call("".to_string(), 1));
            }
            
            Stmt::Fission { delimiter, string } => {
                // Compile as built-in function call: fission(delimiter, string)
                self.compile_expression(delimiter);
                self.compile_expression(string);
                self.program.push(OpCode::PushStr("fission".to_string()));
                self.program.push(OpCode::Call("".to_string(), 2));
            }
            
            Stmt::Fusion { delimiter, strings } => {
                // Compile as built-in function call: fusion(delimiter, strings)
                self.compile_expression(delimiter);
                self.compile_expression(strings);
                self.program.push(OpCode::PushStr("fusion".to_string()));
                self.program.push(OpCode::Call("".to_string(), 2));
            }
        }
    }

    fn compile_block(&mut self, block: Block) {
        for stmt in block.statements {
            self.compile_statement(stmt);
        }
    }

    fn compile_expression(&mut self, expr: Expr) {
        match expr {
            Expr::Number(value) => {
                match value {
                    TokenValue::Int(n) => self.program.push(OpCode::PushInt(n)),
                    TokenValue::Float(f) => self.program.push(OpCode::PushFloat(f)),
                    _ => panic!("Unexpected number type"),
                }
            }
            
            Expr::String(s) => {
                self.program.push(OpCode::PushStr(s));
            }
            
            Expr::Bool(b) => {
                self.program.push(OpCode::PushBool(b));
            }
            
            Expr::Variable(name) => {
                self.program.push(OpCode::LoadVar(name));
            }
            
            Expr::FunctionCall { function, arguments } => {
                let arg_count = arguments.len();
                // Compile arguments first (they go on the stack)
                for arg in arguments {
                    self.compile_expression(arg);
                }
                
                // For function calls, we need to handle the function name specially
                match *function {
                    Expr::Variable(name) => {
                        // Direct function call by name - put the name on the stack
                        self.program.push(OpCode::PushStr(name));
                    }
                    _ => {
                        // Other function expressions - compile normally
                        self.compile_expression(*function);
                    }
                }
                
                // Call function with argument count
                self.program.push(OpCode::Call("".to_string(), arg_count));
            }
            
            Expr::Binary { left, operator, right } => {
                // Compile left and right operands
                self.compile_expression(*left);
                self.compile_expression(*right);
                
                // Add the appropriate operation
                match operator.as_str() {
                    "+" => self.program.push(OpCode::Add),
                    "-" => self.program.push(OpCode::Sub),
                    "*" => self.program.push(OpCode::Mul),
                    "/" => self.program.push(OpCode::Div),
                    "<" => self.program.push(OpCode::Lt),
                    ">" => self.program.push(OpCode::Gt),
                    "<=" => self.program.push(OpCode::Le),
                    ">=" => self.program.push(OpCode::Ge),
                    "==" => self.program.push(OpCode::Eq),
                    "!=" => self.program.push(OpCode::Ne),
                    "&&" => self.program.push(OpCode::And),
                    "||" => self.program.push(OpCode::Or),
                    "|<" => self.program.push(OpCode::And), // Scraps AND operator
                    ">|" => self.program.push(OpCode::Or),  // Scraps OR operator
                    _ => panic!("Unknown binary operator: {}", operator),
                }
            }
            
            Expr::Unary { operator, operand } => {
                // Compile operand
                self.compile_expression(*operand);
                
                // Add the appropriate operation
                match operator.as_str() {
                    "-" => {
                        // Unary minus: multiply by -1
                        self.program.push(OpCode::PushInt(-1));
                        self.program.push(OpCode::Mul);
                    }
                    "!" => {
                        self.program.push(OpCode::Not);
                    }
                    _ => panic!("Unknown unary operator: {}", operator),
                }
            }
            
            Expr::Group(expr) => {
                // Just compile the inner expression
                self.compile_expression(*expr);
            }
            
            Expr::List(elements) => {
                // Create an empty box
                self.program.push(OpCode::MakeBox);
                
                // Pack each element
                for element in elements {
                    self.compile_expression(element);
                    self.program.push(OpCode::Pack);
                }
            }
            
            Expr::Unpack { box_expr, start, end } => {
                // Compile the box expression first (it will be at the bottom of the stack)
                self.compile_expression(*box_expr);
                
                // Compile the start index
                self.compile_expression(*start);
                
                // Compile the end index if provided
                let mut arity: u8 = 1;
                if let Some(end_expr) = end {
                    self.compile_expression(*end_expr);
                    arity = 2;
                }
                
                // Add the Unpack opcode with arity
                self.program.push(OpCode::Unpack(arity));
            }
            
            Expr::Pick { box_expr, indices } => {
                // Compile the box expression first (it will be at the bottom of the stack)
                self.compile_expression(*box_expr);
                
                // Store the number of indices before consuming the vector
                let num_indices = indices.len();
                
                // Compile all the indices
                for index in indices {
                    self.compile_expression(index);
                }
                
                // Add the Pick opcode with the number of indices
                self.program.push(OpCode::Pick(num_indices));
            }
            

            

        }
    }

    fn patch_jumps(&mut self) {
        // Build label map from current program
        let mut label_map = HashMap::new();
        for (i, opcode) in self.program.iter().enumerate() {
            if let OpCode::Label(name) = opcode {
                label_map.insert(name.clone(), i);
            }
        }
        
        // Patch all jump instructions with correct target positions
        for (instruction_index, label_name) in &self.jump_patches {
            if let Some(target_pos) = label_map.get(label_name) {
                // Replace the placeholder jump target with the actual position
                if let OpCode::Jump(ref mut target) = self.program[*instruction_index] {
                    *target = *target_pos;
                } else if let OpCode::JumpIfNot(ref mut target) = self.program[*instruction_index] {
                    *target = *target_pos;

                }
            } else {
                panic!("Label '{}' not found in program", label_name);
            }
        }
    }
}

// Legacy compile() removed — use Parser + Compiler::compile
