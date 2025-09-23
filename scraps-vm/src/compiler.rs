

use crate::bytecode::OpCode;
use crate::parser::{Expr, Stmt, Block};
use crate::lexer::TokenValue;
use std::collections::HashMap;

pub struct Compiler {
    program: Vec<OpCode>,
    labels: HashMap<String, usize>,
    label_counter: usize,
    jump_patches: Vec<(usize, String)>, // (instruction_index, label_name) pairs to patch
    // Simple compile-time type checking (best-effort)
    type_env: HashMap<String, CType>,
    // When compiling a rewire block, allow this variable to change type
    rewire_ok: Option<String>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            program: Vec::new(),
            labels: HashMap::new(),
            label_counter: 0,
            jump_patches: Vec::new(),
            type_env: HashMap::new(),
            rewire_ok: None,
        }
    }

    pub fn compile(&mut self, statements: Vec<Stmt>) -> Result<Vec<OpCode>, String> {
        self.program.clear();
        self.labels.clear();
        self.label_counter = 0;
        self.jump_patches.clear();
        
        // First pass: compile all statements and collect labels
        for stmt in statements {
            self.compile_statement(stmt)?;
        }
        
        // Patch all jump targets with correct positions
        self.patch_jumps();
        
        self.program.push(OpCode::Halt);
        Ok(self.program.clone())
    }

    fn compile_statement(&mut self, stmt: Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Line(n) => {
                // Insert a SetLine marker for runtime context
                self.program.push(OpCode::SetLine(n));
            }
            Stmt::Print(expr) => {
                self.compile_expression(expr);
                self.program.push(OpCode::Print);
            }
            
            Stmt::Test(expr) => {
                // Compile the condition expression, then assert it
                self.compile_expression(expr);
                self.program.push(OpCode::Assert);
            }
            
            Stmt::Expression(expr) => {
                self.compile_expression(expr);
                // Pop the result since we don't need it
                // TODO: Add Pop opcode if needed
            }
            
            Stmt::Assignment { name, value } => {
                // Compile-time type check (best-effort)
                if let Some(rhs_ty) = self.infer_expr_type(&value) {
                    if let Some(prev) = self.type_env.get(&name).cloned() {
                        if prev != rhs_ty {
                            let allowed = self.rewire_ok.as_ref().map(|s| s == &name).unwrap_or(false);
                            if !allowed {
                                return Err(format!(
                                    "Compile-time TYPE error: '{}' was {:?}, assigned {:?}. Use rewire to change type",
                                    name, prev, rhs_ty
                                ));
                            }
                        }
                    }
                    self.type_env.insert(name.clone(), rhs_ty);
                }
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
                self.compile_block(then_block)?;
                
                // Jump to end (skip else block)
                let end_label = format!("if_end_{}", current_counter);
                self.program.push(OpCode::Jump(0)); // Placeholder
                self.jump_patches.push((self.program.len() - 1, end_label.clone()));
                
                // Compile else block
                self.program.push(OpCode::Label(else_label));
                self.compile_block(else_block)?;
                
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
                self.compile_block(body)?;
                
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
                let mut inferred_elems: Vec<CType> = Vec::new();
                for value in values {
                    if let Some(t) = self.infer_expr_type(&value) { inferred_elems.push(t); }
                    self.compile_expression(value);
                    self.program.push(OpCode::Pack);
                }
                
                // Store the updated box back to the variable
                if let Expr::Variable(name) = target {
                    // Compile-time check: enforce int_box/str_box element types when known
                    let prev = self.type_env.get(&name).cloned();
                    let elem_kind = if inferred_elems.iter().all(|t| matches!(t, CType::Int)) {
                        Some(CType::Int)
                    } else if inferred_elems.iter().all(|t| matches!(t, CType::Str)) {
                        Some(CType::Str)
                    } else { None };

                    match prev {
                        Some(CType::BoxInt) => {
                            if let Some(CType::Int) = elem_kind { /* ok */ } else if !inferred_elems.is_empty() {
                                return Err(format!("Compile-time TYPE error: '{}' is BoxInt, but pack contains non-Int element(s)", name));
                            }
                        }
                        Some(CType::BoxStr) => {
                            if let Some(CType::Str) = elem_kind { /* ok */ } else if !inferred_elems.is_empty() {
                                return Err(format!("Compile-time TYPE error: '{}' is BoxStr, but pack contains non-Str element(s)", name));
                            }
                        }
                        Some(CType::Box) | None => {
                            // If not typed yet, specialize if all elements are consistent
                            if let Some(CType::Int) = elem_kind { self.type_env.insert(name.clone(), CType::BoxInt); }
                            else if let Some(CType::Str) = elem_kind { self.type_env.insert(name.clone(), CType::BoxStr); }
                            else { self.type_env.insert(name.clone(), CType::Box); }
                        }
                        Some(other) => {
                            // If variable had a scalar type, using it as box is an error
                            return Err(format!("Compile-time TYPE error: '{}' is {:?}, but used as Box with pack()", name, other));
                        }
                    }
                    self.program.push(OpCode::StoreVar(name));
                } else {
                    // TODO: Handle non-variable targets
                    panic!("Pack target must be a variable for now");
                }
            }
            
            Stmt::Place { pairs, target } => {
                // Load target box
                self.compile_expression(target.clone());
                
                // For each (index, value) pair
                for (index, value) in pairs {
                    // Infer value type for element-type checking
                    let val_ty = self.infer_expr_type(&value);
                    // Don't put index on stack - it's passed as OpCode parameter
                    self.compile_expression(value);
                    // TODO: Implement Place with dynamic index
                    // For now, assume index is constant
                    if let Expr::Number(TokenValue::Int(idx)) = index {
                        self.program.push(OpCode::Place(idx as usize));
                    } else {
                        // TODO: Handle dynamic indices
                        panic!("Dynamic indices not yet implemented");
                    }
                    // After each place value, update/check the box variable type
                    if let Expr::Variable(ref name) = target {
                        let prev = self.type_env.get(name).cloned();
                        match prev {
                            Some(CType::BoxInt) => {
                                if let Some(CType::Str) = val_ty {
                                    return Err(format!("Compile-time TYPE error: '{}' is BoxInt, but place sets Str", name));
                                }
                            }
                            Some(CType::BoxStr) => {
                                if let Some(CType::Int) = val_ty {
                                    return Err(format!("Compile-time TYPE error: '{}' is BoxStr, but place sets Int", name));
                                }
                            }
                            Some(CType::Box) => { /* ok, dynamic box */ }
                            None => {
                                // Specialize if known
                                if let Some(CType::Int) = val_ty { self.type_env.insert(name.clone(), CType::BoxInt); }
                                else if let Some(CType::Str) = val_ty { self.type_env.insert(name.clone(), CType::BoxStr); }
                                else { self.type_env.insert(name.clone(), CType::Box); }
                            }
                            Some(other) => {
                                return Err(format!("Compile-time TYPE error: '{}' is {:?}, but used as Box with place()", name, other));
                            }
                        }
                    }
                }
                
                // Store the updated box back to the variable
                if let Expr::Variable(name) = target {
                    // Ensure final type is some Box variant
                    let prev = self.type_env.get(&name).cloned();
                    match prev {
                        Some(CType::BoxInt) | Some(CType::BoxStr) | Some(CType::Box) => {}
                        None => { self.type_env.insert(name.clone(), CType::Box); }
                        Some(other) => return Err(format!("Compile-time TYPE error: '{}' is {:?}, but used as Box with place()", name, other)),
                    }
                    self.program.push(OpCode::StoreVar(name));
                } else {
                    // TODO: Handle non-variable targets
                    panic!("Place target must be a variable for now");
                }
            }
            
            Stmt::FunctionDef { name, parameters, body } => {
                // Compile the function body into bytecode
                let mut body_compiler = Compiler::new();
                // Inherit type environment (best-effort)
                body_compiler.type_env = self.type_env.clone();
                body_compiler.compile_block(body)?;
                body_compiler.patch_jumps(); // CRITICAL: Patch jumps for function body
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
                body_compiler.type_env = self.type_env.clone();
                body_compiler.rewire_ok = Some(_target.clone());
                body_compiler.compile_block(body)?;
                // Ensure function returns the rewired variable to make the ceremony output be the variable itself
                body_compiler.program.push(OpCode::LoadVar(_target.clone()));
                body_compiler.patch_jumps(); // CRITICAL: Patch jumps for rewire body
                let function_body = body_compiler.program;
                self.program.push(OpCode::MakeFuncWithBody {
                    name: destination.clone(),
                    parameters: vec![],
                    body: function_body,
                    rewire_target: Some(_target.clone()),
                });
                // Immediately apply the rewire: result(destination)
                // Push function value (LoadVar), then call result with 1 arg
                self.program.push(OpCode::LoadVar(destination.clone()));
                self.program.push(OpCode::PushStr("result".to_string()));
                self.program.push(OpCode::Call("".to_string(), 1));
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

            Stmt::Rename { from, to } => {
                // Compile as built-in function call: rename(from, to)
                self.compile_expression(from);
                self.compile_expression(to);
                self.program.push(OpCode::PushStr("rename".to_string()));
                self.program.push(OpCode::Call("".to_string(), 2));
            }
        }
        Ok(())
    }

    fn compile_block(&mut self, block: Block) -> Result<(), String> {
        for stmt in block.statements {
            self.compile_statement(stmt)?;
        }
        Ok(())
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
            
            Expr::If { condition, then_block, else_block } => {
                // Compile condition
                self.compile_expression(*condition);
                
                // Create jump labels
                let else_label = self.create_label();
                let end_label = self.create_label();
                
                // Jump to else block if condition is false (placeholder target)
                let jump_if_not_index = self.program.len();
                self.program.push(OpCode::JumpIfNot(0)); // placeholder
                self.jump_patches.push((jump_if_not_index, else_label.clone()));
                
                // Compile then block directly (no separate compiler)
                let _ = self.compile_block(then_block);
                
                // Jump to end (skip else block) (placeholder target)
                let jump_index = self.program.len();
                self.program.push(OpCode::Jump(0)); // placeholder
                self.jump_patches.push((jump_index, end_label.clone()));
                
                // Else block
                self.set_label(else_label);
                let _ = self.compile_block(else_block);
                
                // End label
                self.set_label(end_label);
            }
            

        }
    }

    fn create_label(&mut self) -> String {
        let label = format!("label_{}", self.label_counter);
        self.label_counter += 1;
        label
    }
    
    fn set_label(&mut self, label: String) {
        self.program.push(OpCode::Label(label));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CType {
    Int,
    Float,
    Bool,
    Str,
    Box,
    BoxInt,
    BoxStr,
}

impl Compiler {
    fn infer_expr_type(&self, expr: &Expr) -> Option<CType> {
        match expr {
            Expr::Number(TokenValue::Int(_)) => Some(CType::Int),
            Expr::Number(TokenValue::Float(_)) => Some(CType::Float),
            Expr::String(_) => Some(CType::Str),
            Expr::Bool(_) => Some(CType::Bool),
            Expr::List(_) => Some(CType::Box),
            Expr::Variable(_name) => None, // unknown at compile-time
            Expr::Group(inner) => self.infer_expr_type(inner),
            Expr::FunctionCall { function, .. } => {
                if let Expr::Variable(name) = &**function {
                    match name.as_str() {
                        "int" | "INT" => Some(CType::Int),
                        "str" | "STR" => Some(CType::Str),
                        "string" | "STRING" => Some(CType::Str),
                        "input_str" | "INPUT_STR" => Some(CType::Str),
                        "input_int" | "INPUT_INT" => Some(CType::Int),
                        "int_box" | "INT_BOX" => Some(CType::BoxInt),
                        "str_box" | "STR_BOX" => Some(CType::BoxStr),
                        "box" | "BOX" => Some(CType::Box),
                        _ => None,
                    }
                } else { None }
            }
            _ => None,
        }
    }
}
