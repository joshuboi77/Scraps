

use std::collections::HashMap;
use crate::value::Value;
use crate::bytecode::OpCode;
use std::io::Write;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::compiler::Compiler;

const REWIRED_KEY: &str = "__rewired__";

fn is_rewired(env: &HashMap<String, Value>, name: &str) -> bool {
    if let Some(Value::Box(list)) = env.get(REWIRED_KEY) {
        for v in list {
            if let Value::Str(s) = v { if s == name { return true; } }
        }
    }
    false
}

fn add_rewired(env: &mut HashMap<String, Value>, name: &str) {
    let mut list = match env.remove(REWIRED_KEY) {
        Some(Value::Box(vs)) => vs,
        _ => Vec::new(),
    };
    if !list.iter().any(|v| matches!(v, Value::Str(s) if s == name)) {
        list.push(Value::Str(name.to_string()));
    }
    env.insert(REWIRED_KEY.to_string(), Value::Box(list));
}

fn eval_snippet(env: &mut HashMap<String, Value>, src: &str) -> Result<Value, String> {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse().map_err(|e| e.to_string())?;
    let mut compiler = Compiler::new();
    let bytecode = compiler.compile(program.statements);
    // Execute in the provided environment, capture the last value on stack
    let mut stack: Vec<Value> = Vec::new();
    execute_function(&bytecode, &mut stack, env)
}

/// Execute a function's bytecode body
fn execute_function(
    body: &[OpCode], 
    _stack: &mut Vec<Value>, 
    env: &mut HashMap<String, Value>
) -> Result<Value, String> {
    let mut local_stack = Vec::new();
    let local_env = env; // operate on provided environment
    let mut ip: usize = 0;
    
    while ip < body.len() {
        let instr = &body[ip];
        
        match instr {
            // Stack operations
            OpCode::PushInt(n) => local_stack.push(Value::Int(*n)),
            OpCode::PushFloat(f) => local_stack.push(Value::Float(*f)),
            OpCode::PushBool(b) => local_stack.push(Value::Bool(*b)),
            OpCode::PushStr(s) => local_stack.push(Value::Str(s.clone())),
            
            // Variables
            OpCode::LoadVar(name) => {
                if let Some(val) = local_env.get(name) {
                    local_stack.push(val.clone());
                } else {
                    return Err(format!("Undefined variable '{}' in function", name));
                }
            }

            // Box operations inside functions
            OpCode::Unpack(arity) => {
                // Mirror main VM unpack behavior
                let (start, end_opt, box_val) = if *arity == 2 {
                    let end_val = local_stack.pop().expect("Expected end index for UNPACK");
                    let start_val = local_stack.pop().expect("Expected start index for UNPACK");
                    let box_val = local_stack.pop().expect("Expected box for UNPACK");
                    (start_val, Some(end_val), box_val)
                } else {
                    let start_val = local_stack.pop().expect("Expected start index for UNPACK");
                    let box_val = local_stack.pop().expect("Expected box for UNPACK");
                    (start_val, None, box_val)
                };

                let start_idx = match start {
                    Value::Int(n) => n,
                    _ => return Err("UNPACK start index must be integer".to_string()),
                };
                let end_idx = if let Some(end_val) = end_opt {
                    match end_val {
                        Value::Int(n) => Some(n),
                        _ => return Err("UNPACK end index must be integer".to_string()),
                    }
                } else { None };

                match box_val {
                    Value::Box(contents) => {
                        if end_idx.is_none() {
                            if start_idx < 0 || start_idx >= contents.len() as i64 {
                                return Err("UNPACK: index out of bounds".to_string());
                            }
                            local_stack.push(contents[start_idx as usize].clone());
                        } else {
                            let end = end_idx.unwrap();
                            if start_idx < 0 || end < start_idx || end > contents.len() as i64 {
                                return Err("UNPACK: slice out of bounds".to_string());
                            }
                            let slice = contents[start_idx as usize..end as usize].to_vec();
                            local_stack.push(Value::Box(slice));
                        }
                    }
                    Value::Str(s) => {
                        if end_idx.is_none() {
                            if start_idx < 0 || start_idx >= s.len() as i64 {
                                return Err("UNPACK: index out of bounds".to_string());
                            }
                            let ch = s.chars().nth(start_idx as usize).unwrap();
                            local_stack.push(Value::Str(ch.to_string()));
                        } else {
                            let end = end_idx.unwrap();
                            if start_idx < 0 || end < start_idx || end > s.len() as i64 {
                                return Err("UNPACK: slice out of bounds".to_string());
                            }
                            let slice: String = s.chars().skip(start_idx as usize).take((end - start_idx) as usize).collect();
                            local_stack.push(Value::Str(slice));
                        }
                    }
                    _ => return Err("UNPACK: expected box, string, or function".to_string()),
                }
            }
            OpCode::Pick(num_indices) => {
                // Multi-select within one layer
                let mut indices = Vec::new();
                for _ in 0..*num_indices {
                    let val = local_stack.pop().ok_or("PICK: not enough indices on stack")?;
                    match val {
                        Value::Int(idx) => indices.push(idx),
                        _ => return Err("PICK: index must be integer".to_string()),
                    }
                }
                indices.reverse();

                let box_val = local_stack.pop().ok_or("PICK: no box on stack")?;
                match box_val {
                    Value::Box(list) => {
                        if indices.is_empty() { return Err("PICK: expects at least one index".to_string()); }
                        if indices.len() == 1 {
                            let idx = indices[0];
                            if idx < 0 || idx >= list.len() as i64 { return Err(format!("PICK: index {} out of bounds", idx)); }
                            local_stack.push(list[idx as usize].clone());
                        } else {
                            let mut out = Vec::with_capacity(indices.len());
                            for idx in indices {
                                if idx < 0 || idx >= list.len() as i64 { return Err(format!("PICK: index {} out of bounds", idx)); }
                                out.push(list[idx as usize].clone());
                            }
                            local_stack.push(Value::Box(out));
                        }
                    }
                    Value::Str(s) => {
                        if indices.is_empty() { return Err("PICK: expects at least one index".to_string()); }
                        if indices.len() == 1 {
                            let idx = indices[0];
                            if idx < 0 || idx >= s.len() as i64 { return Err(format!("PICK: index {} out of bounds", idx)); }
                            let ch = s.chars().nth(idx as usize).unwrap();
                            local_stack.push(Value::Str(ch.to_string()));
                        } else {
                            let mut out = String::new();
                            for idx in indices {
                                if idx < 0 || idx >= s.len() as i64 { return Err(format!("PICK: index {} out of bounds", idx)); }
                                if let Some(ch) = s.chars().nth(idx as usize) { out.push(ch); }
                            }
                            local_stack.push(Value::Str(out));
                        }
                    }
                    _ => return Err("PICK: box must be list or string".to_string()),
                }
            }
            // Box operations inside functions (Pack/Place)
            OpCode::Pack => {
                let item = local_stack.pop().expect("Expected value to pack");
                let target = local_stack.pop().expect("Expected target box");
                match target {
                    Value::Box(mut contents) => {
                        contents.push(item);
                        local_stack.push(Value::Box(contents));
                    }
                    _ => return Err("Pack target was not a box".to_string()),
                }
            }
            OpCode::Place(index) => {
                let value = local_stack.pop().expect("Expected value to place");
                let mut target = local_stack.pop().expect("Expected target box");
                if let Value::Box(ref mut inner) = target {
                    let idx = *index;
                    if idx >= inner.len() {
                        inner.resize(idx + 1, Value::None);
                    }
                    inner[idx] = value;
                    local_stack.push(target);
                } else {
                    return Err("PLACE target is not a box".to_string());
                }
            }
            // Built-in and function calls inside functions
            OpCode::Call(_func_name, arg_count) => {
                let func_name_value = local_stack.pop().expect("Expected function name on stack");
                let func_name = match func_name_value { Value::Str(s) => s, _ => return Err("Function name must be a string".to_string()) };
                let func = local_env.get(&func_name).ok_or("Function not found")?;
                match func_name.as_str() {
                    "box" => { if *arg_count != 0 { return Err("BOX expects 0 arguments".to_string()); } local_stack.push(Value::Box(vec![])); }
                    "result" => {
                        if *arg_count != 1 { return Err("RESULT expects exactly 1 argument".to_string()); }
                        let func_value = local_stack.pop().expect("Expected function for RESULT");
                        match func_value {
                            Value::Function { name: _n, params, body, rewire_target } => {
                                let mut func_env = local_env.clone();
                                for param_name in params.iter() {
                                    if let Some(value) = local_env.get(param_name) {
                                        func_env.insert(param_name.clone(), value.clone());
                                    }
                                }
                                let result = execute_function(&body, &mut local_stack, &mut func_env)?;
                                if let Some(target) = rewire_target {
                                    if let Some(new_val) = func_env.get(&target) {
                                        local_env.insert(target.clone(), new_val.clone());
                                    }
                                }
                                local_stack.push(result);
                            }
                            _ => return Err("RESULT expects a function".to_string()),
                        }
                    }
                    "read" => { if *arg_count != 1 { return Err("READ expects exactly 1 argument".to_string()); }
                        let filename = local_stack.pop().expect("Expected filename for READ");
                        let path = match filename { Value::Str(s) => s, _ => return Err("READ filename must be a string".to_string()) };
                        match std::fs::read_to_string(&path) { Ok(content) => local_stack.push(Value::Str(content)), Err(e) => return Err(format!("READ error: {}", e)) }
                    }
                    "write" => { if *arg_count != 2 { return Err("WRITE expects exactly 2 arguments".to_string()); }
                        let filename = local_stack.pop().expect("Expected filename for WRITE");
                        let content = local_stack.pop().expect("Expected content for WRITE");
                        let path = match filename { Value::Str(s) => s, _ => return Err("WRITE filename must be a string".to_string()) };
                        let data = match content { Value::Str(s) => s, _ => return Err("WRITE content must be a string".to_string()) };
                        match std::fs::write(&path, data.as_bytes()) { Ok(_) => local_stack.push(Value::Str(data)), Err(e) => return Err(format!("WRITE error: {}", e)) }
                    }
                    "fission" => { if *arg_count != 2 { return Err("FISSION expects exactly 2 arguments".to_string()); }
                        let string_val = local_stack.pop().expect("Expected string for FISSION");
                        let delim_val = local_stack.pop().expect("Expected delimiter for FISSION");
                        let s = match string_val { Value::Str(s) => s, _ => return Err("FISSION: string must be a string".to_string()) };
                        let delim = match delim_val { Value::Str(s) => s, _ => return Err("FISSION: delimiter must be a string".to_string()) };
                        let parts: Vec<Value> = if delim.is_empty() { s.chars().map(|c| Value::Str(c.to_string())).collect() } else { s.split(&delim).map(|p| Value::Str(p.to_string())).collect() };
                        local_stack.push(Value::Box(parts));
                    }
                    "fusion" => { if *arg_count != 2 { return Err("FUSION expects exactly 2 arguments".to_string()); }
                        let strings_val = local_stack.pop().expect("Expected strings for FUSION");
                        let delim_val = local_stack.pop().expect("Expected delimiter for FUSION");
                        let delim = match delim_val { Value::Str(s) => s, _ => return Err("FUSION: delimiter must be a string".to_string()) };
                        let items = match strings_val {
                            Value::Box(vs) => {
                                let mut out = Vec::new();
                                for v in vs {
                                    match v {
                                        Value::Str(s) => out.push(s),
                                        _ => return Err("FUSION: all elements must be strings".to_string()),
                                    }
                                }
                                out
                            }
                            _ => return Err("FUSION: second argument must be a box of strings".to_string()),
                        };
                        let joined = items.join(&delim); local_stack.push(Value::Str(joined));
                    }
                    "count" => {
                        if *arg_count != 1 { return Err("COUNT expects exactly 1 argument".to_string()); }
                        let arg = local_stack.pop().expect("Expected argument for COUNT");
                        match arg { Value::Box(contents) => local_stack.push(Value::Int(contents.len() as i64)), Value::Str(s) => local_stack.push(Value::Int(s.chars().count() as i64)), Value::Function { name: _n, params, body: _b, .. } => local_stack.push(Value::Int(params.len() as i64)), _ => return Err("COUNT expects box, string, or function".to_string()) }
                    }
                    _ => {
                        match func { Value::Function { name, params, body, .. } => { if params.len() != *arg_count { return Err(format!("Function '{}' expects {} arguments, got {}", name, params.len(), arg_count)); } let body_clone = body.clone(); let mut env_copy = local_env.clone(); let result = execute_function(&body_clone, &mut local_stack, &mut env_copy)?; local_stack.push(result); }
                            _ => return Err(format!("'{}' is not a function", func_name)),
                        }
                    }
                }
            }
            
            OpCode::StoreVar(name) => {
                let val = local_stack.pop().expect("Nothing to store");
                local_env.insert(name.clone(), val);
            }
            
            // Arithmetic operations
            OpCode::Add => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Int(x + y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Float(x + y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Float(x as f64 + y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Float(x + y as f64)),
                    _ => return Err("Type error in addition".to_string()),
                }
            }
            OpCode::Sub => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Int(x - y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Float(x - y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Float(x as f64 - y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Float(x - y as f64)),
                    _ => return Err("Type error in subtraction".to_string()),
                }
            }
            OpCode::Mul => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Int(x * y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Float(x * y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Float(x as f64 * y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Float(x * y as f64)),
                    _ => return Err("Type error in multiplication".to_string()),
                }
            }
            OpCode::Div => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => {
                        if y == 0 { return Err("Division by zero".to_string()); }
                        local_stack.push(Value::Float(x as f64 / y as f64));
                    }
                    (Value::Float(x), Value::Float(y)) => {
                        if y == 0.0 { return Err("Division by zero".to_string()); }
                        local_stack.push(Value::Float(x / y));
                    }
                    (Value::Int(x), Value::Float(y)) => {
                        if y == 0.0 { return Err("Division by zero".to_string()); }
                        local_stack.push(Value::Float(x as f64 / y));
                    }
                    (Value::Float(x), Value::Int(y)) => {
                        if y == 0 { return Err("Division by zero".to_string()); }
                        local_stack.push(Value::Float(x / y as f64));
                    }
                    _ => return Err("Type error in division".to_string()),
                }
            }
            
            // Comparison operations
            OpCode::Lt => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Bool(x < y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Bool(x < y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Bool((x as f64) < y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Bool(x < (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Gt => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Bool(x > y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Bool(x > y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Bool((x as f64) > y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Bool(x > (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Le => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Bool(x <= y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Bool(x <= y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Bool((x as f64) <= y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Bool(x <= (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Ge => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Bool(x >= y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Bool(x >= y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Bool((x as f64) >= y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Bool(x >= (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Eq => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Bool(x == y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Bool(x == y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Bool((x as f64) == y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Bool(x == (y as f64))),
                    (Value::Bool(x), Value::Bool(y)) => local_stack.push(Value::Bool(x == y)),
                    (Value::Str(ref x), Value::Str(ref y)) => local_stack.push(Value::Bool(x == y)),
                    _ => local_stack.push(Value::Bool(false)),
                }
            }
            OpCode::Ne => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => local_stack.push(Value::Bool(x != y)),
                    (Value::Float(x), Value::Float(y)) => local_stack.push(Value::Bool(x != y)),
                    (Value::Int(x), Value::Float(y)) => local_stack.push(Value::Bool((x as f64) != y)),
                    (Value::Float(x), Value::Int(y)) => local_stack.push(Value::Bool(x != (y as f64))),
                    (Value::Bool(x), Value::Bool(y)) => local_stack.push(Value::Bool(x != y)),
                    (Value::Str(ref x), Value::Str(ref y)) => local_stack.push(Value::Bool(x != y)),
                    _ => local_stack.push(Value::Bool(true)),
                }
            }
            
            // Logical operations
            OpCode::And => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Bool(x), Value::Bool(y)) => local_stack.push(Value::Bool(x && y)),
                    _ => return Err("Type error in logical AND".to_string()),
                }
            }
            OpCode::Or => {
                let b = local_stack.pop().expect("Expected second operand");
                let a = local_stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Bool(x), Value::Bool(y)) => local_stack.push(Value::Bool(x || y)),
                    _ => return Err("Type error in logical OR".to_string()),
                }
            }
            OpCode::Not => {
                let a = local_stack.pop().expect("Expected operand");
                match a {
                    Value::Bool(x) => local_stack.push(Value::Bool(!x)),
                    _ => return Err("Type error in logical NOT".to_string()),
                }
            }
            

            
            // For now, skip other operations in functions
            _ => {
                // Skip unsupported operations in function bodies
                ip += 1;
                continue;
            }
        }
        
        ip += 1;
    }
    
    // Return the last value on the stack, or None if empty
    Ok(local_stack.pop().unwrap_or(Value::None))
}

pub fn run(program: &[OpCode]) -> Result<(), String> {
    let mut stack: Vec<Value> = Vec::new();
    let mut env: HashMap<String, Value> = HashMap::new();
    
    // Initialize built-in functions
    env.insert("box".to_string(), Value::Function {
        name: "box".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("pack".to_string(), Value::Function {
        name: "pack".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("place".to_string(), Value::Function {
        name: "place".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("unpack".to_string(), Value::Function {
        name: "unpack".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("pick".to_string(), Value::Function {
        name: "pick".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("count".to_string(), Value::Function {
        name: "count".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("print".to_string(), Value::Function {
        name: "print".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("result".to_string(), Value::Function {
        name: "result".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("string".to_string(), Value::Function {
        name: "string".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("read".to_string(), Value::Function {
        name: "read".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("write".to_string(), Value::Function {
        name: "write".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("fission".to_string(), Value::Function {
        name: "fission".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("fusion".to_string(), Value::Function {
        name: "fusion".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("rewire_symbol".to_string(), Value::Function {
        name: "rewire_symbol".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    let mut ip: usize = 0; // Instruction pointer
    
    // Build label map for jump targets (used by jump instructions)
    let _label_map = build_label_map(program)?;

    while ip < program.len() {
        let instr = &program[ip];
        
        match instr {
            // Stack operations
            OpCode::PushInt(n) => stack.push(Value::Int(*n)),
            OpCode::PushFloat(f) => stack.push(Value::Float(*f)),
            OpCode::PushBool(b) => stack.push(Value::Bool(*b)),
            OpCode::PushStr(s) => stack.push(Value::Str(s.clone())),
            
            // Box operations
            OpCode::MakeBox => stack.push(Value::Box(vec![])),
            OpCode::Pack => {
                // Simple Pack: pop item and target, push updated box.
                let item = stack.pop().expect("Expected value to pack");
                let target = stack.pop().expect("Expected target box");

                match target {
                    Value::Box(mut contents) => {
                        contents.push(item);
                        stack.push(Value::Box(contents));
                    },
                    _ => panic!("Pack target was not a box"),
                }
            }
            OpCode::Place(index) => {
                let value = stack.pop().expect("Expected value to place");
                let mut target = stack.pop().expect("Expected target box");
                
                if let Value::Box(ref mut inner) = target {
                    let idx = *index;
                    if idx >= inner.len() {
                        inner.resize(idx + 1, Value::None);
                    }
                    inner[idx] = value;
                    stack.push(target);
                } else {
                    panic!("PLACE target is not a box");
                }
            }
            OpCode::Unpack(arity) => {
                // UNPACK operation: arity 1 expects box, start; arity 2 expects box, start, end on stack
                // Stack: [box, start, end?] -> [result]
                
                let (start, end_opt, box_val) = if *arity == 2 {
                    let end_val = stack.pop().expect("Expected end index for UNPACK");
                    let start_val = stack.pop().expect("Expected start index for UNPACK");
                    let box_val = stack.pop().expect("Expected box for UNPACK");
                    (start_val, Some(end_val), box_val)
                } else {
                    let start_val = stack.pop().expect("Expected start index for UNPACK");
                    let box_val = stack.pop().expect("Expected box for UNPACK");
                    (start_val, None, box_val)
                };
                
                // Convert start and end to integers
                let start_idx = match start {
                    Value::Int(n) => n,
                    _ => return Err("UNPACK start index must be integer".to_string()),
                };
                
                let end_idx = if let Some(end_val) = end_opt {
                    match end_val {
                        Value::Int(n) => Some(n),
                        _ => return Err("UNPACK end index must be integer".to_string()),
                    }
                } else {
                    None
                };
                
                // Handle different box types
                match box_val {
                    Value::Box(contents) => {
                        if end_idx.is_none() {
                            // Single element extraction
                            if start_idx < 0 || start_idx >= contents.len() as i64 {
                                return Err("UNPACK: index out of bounds".to_string());
                            }
                            stack.push(contents[start_idx as usize].clone());
                        } else {
                            // Slice extraction
                            let end = end_idx.unwrap();
                            if start_idx < 0 || end < start_idx || end > contents.len() as i64 {
                                return Err("UNPACK: slice out of bounds".to_string());
                            }
                            let slice = contents[start_idx as usize..end as usize].to_vec();
                            stack.push(Value::Box(slice));
                        }
                    }
                    Value::Str(s) => {
                        if end_idx.is_none() {
                            // Single character extraction
                            if start_idx < 0 || start_idx >= s.len() as i64 {
                                return Err("UNPACK: index out of bounds".to_string());
                            }
                            let ch = s.chars().nth(start_idx as usize).unwrap();
                            stack.push(Value::Str(ch.to_string()));
                        } else {
                            // String slice extraction
                            let end = end_idx.unwrap();
                            if start_idx < 0 || end < start_idx || end > s.len() as i64 {
                                return Err("UNPACK: slice out of bounds".to_string());
                            }
                            let slice: String = s.chars().skip(start_idx as usize).take((end - start_idx) as usize).collect();
                            stack.push(Value::Str(slice));
                        }
                    }
                    Value::Function { name: _name, params, body: _body, .. } => {
                        // Convert function to box representation
                        let func_box = Value::Box(params.iter().map(|p| Value::Str(p.clone())).collect());
                        // Recursively call UNPACK on the function box
                        stack.push(func_box);
                        stack.push(Value::Int(start_idx));
                        if let Some(end_val) = end_idx {
                            stack.push(Value::Int(end_val));
                            // In a full implementation, we'd invoke an internal unpack with arity=2
                        }
                        return Err("UNPACK on functions not yet implemented".to_string());
                    }
                    _ => return Err("UNPACK: expected box, string, or function".to_string()),
                }
            }
            OpCode::Pick(num_indices) => {
                // PICK operation (multi-select within one layer): expects box, index1, index2, ... on stack
                // If one index provided: return the single element/char.
                // If multiple indices: return a box (for lists) or concatenated string (for strings).

                // Gather indices
                let mut indices = Vec::new();
                for _ in 0..*num_indices {
                    let val = stack.pop().ok_or("PICK: not enough indices on stack")?;
                    match val {
                        Value::Int(idx) => indices.push(idx),
                        _ => return Err("PICK: index must be integer".to_string()),
                    }
                }
                indices.reverse();

                // Get the target container
                let box_val = stack.pop().ok_or("PICK: no box on stack")?;

                match box_val {
                    Value::Box(list) => {
                        if indices.is_empty() {
                            return Err("PICK: expects at least one index".to_string());
                        }
                        if indices.len() == 1 {
                            let idx = indices[0];
                            if idx < 0 || idx >= list.len() as i64 {
                                return Err(format!("PICK: index {} out of bounds", idx));
                            }
                            stack.push(list[idx as usize].clone());
                        } else {
                            let mut out = Vec::with_capacity(indices.len());
                            for idx in indices {
                                if idx < 0 || idx >= list.len() as i64 {
                                    return Err(format!("PICK: index {} out of bounds", idx));
                                }
                                out.push(list[idx as usize].clone());
                            }
                            stack.push(Value::Box(out));
                        }
                    }
                    Value::Str(s) => {
                        if indices.is_empty() {
                            return Err("PICK: expects at least one index".to_string());
                        }
                        if indices.len() == 1 {
                            let idx = indices[0];
                            if idx < 0 || idx >= s.len() as i64 {
                                return Err(format!("PICK: index {} out of bounds", idx));
                            }
                            let ch = s.chars().nth(idx as usize).unwrap();
                            stack.push(Value::Str(ch.to_string()));
                        } else {
                            let mut out = String::new();
                            for idx in indices {
                                if idx < 0 || idx >= s.len() as i64 {
                                    return Err(format!("PICK: index {} out of bounds", idx));
                                }
                                if let Some(ch) = s.chars().nth(idx as usize) {
                                    out.push(ch);
                                } else {
                                    return Err(format!("PICK: index {} out of bounds", idx));
                                }
                            }
                            stack.push(Value::Str(out));
                        }
                    }
                    _ => return Err("PICK: box must be list or string".to_string()),
                }
            }
            

            
            OpCode::Print => {
                if let Some(val) = stack.pop() {
                    let out_val = match val {
                        Value::Str(s) => {
                            // If rewired symbol, print its bound value
                            if is_rewired(&env, &s) {
                                env.get(&s).cloned().unwrap_or(Value::Str(s))
                            } else {
                                // Evaluate string as code (best-effort)
                                match eval_snippet(&mut env, &s) {
                                    Ok(v) => v,
                                    Err(_) => Value::Str(s),
                                }
                            }
                        }
                        other => other,
                    };
                    if let Err(e) = writeln!(std::io::stdout(), "{}", out_val.format_for_display()) {
                        return Err(format!("Failed to write output: {}", e));
                    }
                }
            }
            OpCode::LoadVar(name) => {
                if let Some(val) = env.get(name) {
                    stack.push(val.clone());
                } else {
                    return Err(format!("Undefined variable '{}'", name));
                }
            }
            OpCode::StoreVar(name) => {
                let val = stack.pop().expect("Nothing to store");
                env.insert(name.clone(), val);
            }
            
            // Arithmetic operations
            OpCode::Add => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Int(x + y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Float(x + y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Float(x as f64 + y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Float(x + y as f64)),
                    _ => return Err("Type error in addition".to_string()),
                }
            }
            OpCode::Sub => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Int(x - y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Float(x - y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Float(x as f64 - y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Float(x - y as f64)),
                    _ => return Err("Type error in subtraction".to_string()),
                }
            }
            OpCode::Mul => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Int(x * y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Float(x * y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Float(x as f64 * y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Float(x * y as f64)),
                    _ => return Err("Type error in multiplication".to_string()),
                }
            }
            OpCode::Div => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => {
                        if y == 0 { return Err("Division by zero".to_string()); }
                        stack.push(Value::Float(x as f64 / y as f64));
                    }
                    (Value::Float(x), Value::Float(y)) => {
                        if y == 0.0 { return Err("Division by zero".to_string()); }
                        stack.push(Value::Float(x / y));
                    }
                    (Value::Int(x), Value::Float(y)) => {
                        if y == 0.0 { return Err("Division by zero".to_string()); }
                        stack.push(Value::Float(x as f64 / y));
                    }
                    (Value::Float(x), Value::Int(y)) => {
                        if y == 0 { return Err("Division by zero".to_string()); }
                        stack.push(Value::Float(x / y as f64));
                    }
                    _ => return Err("Type error in division".to_string()),
                }
            }
            
            // Comparison operations
            OpCode::Lt => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Bool(x < y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Bool(x < y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Bool((x as f64) < y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Bool(x < (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Gt => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Bool(x > y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Bool(x > y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Bool((x as f64) > y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Bool(x > (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Le => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Bool(x <= y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Bool(x <= y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Bool((x as f64) <= y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Bool(x <= (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Ge => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Bool(x >= y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Bool(x >= y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Bool((x as f64) >= y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Bool(x >= (y as f64))),
                    _ => return Err("Type error in comparison".to_string()),
                }
            }
            OpCode::Eq => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Bool(x == y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Bool(x == y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Bool((x as f64) == y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Bool(x == (y as f64))),
                    (Value::Bool(x), Value::Bool(y)) => stack.push(Value::Bool(x == y)),
                    (Value::Str(ref x), Value::Str(ref y)) => stack.push(Value::Bool(x == y)),
                    _ => stack.push(Value::Bool(false)),
                }
            }
            OpCode::Ne => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => stack.push(Value::Bool(x != y)),
                    (Value::Float(x), Value::Float(y)) => stack.push(Value::Bool(x != y)),
                    (Value::Int(x), Value::Float(y)) => stack.push(Value::Bool((x as f64) != y)),
                    (Value::Float(x), Value::Int(y)) => stack.push(Value::Bool(x != (y as f64))),
                    (Value::Bool(x), Value::Bool(y)) => stack.push(Value::Bool(x != y)),
                    (Value::Str(ref x), Value::Str(ref y)) => stack.push(Value::Bool(x != y)),
                    _ => stack.push(Value::Bool(true)),
                }
            }
            
            // Logical operations
            OpCode::And => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Bool(x), Value::Bool(y)) => stack.push(Value::Bool(x && y)),
                    _ => return Err("Type error in logical AND".to_string()),
                }
            }
            OpCode::Or => {
                let b = stack.pop().expect("Expected second operand");
                let a = stack.pop().expect("Expected first operand");
                match (a, b) {
                    (Value::Bool(x), Value::Bool(y)) => stack.push(Value::Bool(x || y)),
                    _ => return Err("Type error in logical OR".to_string()),
                }
            }
            OpCode::Not => {
                let a = stack.pop().expect("Expected operand");
                match a {
                    Value::Bool(x) => stack.push(Value::Bool(!x)),
                    _ => return Err("Type error in logical NOT".to_string()),
                }
            }
            
            // Control flow - now properly implemented!
            OpCode::Jump(offset) => {
                // Absolute jump to instruction index
                ip = *offset;
                continue; // Skip the normal increment
            }

            OpCode::JumpIfNot(offset) => {
                // Conditional jump if false
                let condition = stack.pop().expect("Expected condition for JumpIfNot");
                match condition {
                    Value::Bool(false) => {
                        ip = *offset;
                        continue; // Skip the normal increment
                    }
                    Value::Bool(true) => {
                        // Don't jump, continue to next instruction
                    }
                    _ => return Err("JumpIf condition must be boolean".to_string()),
                }
            }

            OpCode::Label(_) => {
                // Labels are no-ops during execution
                // They're just used for jump targets
            }
            
            // Functions (basic stubs)
            OpCode::Call(_func_name, arg_count) => {
                // Get the function name from the stack (it was compiled as an expression)
                let func_name_value = stack.pop().expect("Expected function name on stack");
                let func_name = match func_name_value {
                    Value::Str(s) => s,
                    _ => return Err("Function name must be a string".to_string()),
                };
                
                // Get the function from the environment
                let func = env.get(&func_name).expect("Function not found");
                
                // Handle built-in functions
                match func_name.as_str() {
                    "box" => {
                        if *arg_count != 0 {
                            return Err("BOX expects 0 arguments".to_string());
                        }
                        stack.push(Value::Box(vec![]));
                    }
                    "rewire_symbol" => {
                        if *arg_count != 1 { return Err("REWIRE expects exactly 1 argument".to_string()); }
                        let name_val = stack.pop().expect("Expected symbol name for REWIRE");
                        let sym = match name_val { Value::Str(s) => s, _ => return Err("REWIRE: symbol must be a string".to_string()) };
                        add_rewired(&mut env, &sym);
                        stack.push(Value::None);
                    }
                    "read" => {
                        if *arg_count != 1 { return Err("READ expects exactly 1 argument".to_string()); }
                        let filename = stack.pop().expect("Expected filename for READ");
                        let path = match filename { Value::Str(s) => s, _ => return Err("READ filename must be a string".to_string()) };
                        match std::fs::read_to_string(&path) {
                            Ok(content) => stack.push(Value::Str(content)),
                            Err(e) => return Err(format!("READ error: {}", e)),
                        }
                    }
                    "write" => {
                        if *arg_count != 2 { return Err("WRITE expects exactly 2 arguments".to_string()); }
                        let filename = stack.pop().expect("Expected filename for WRITE");
                        let content = stack.pop().expect("Expected content for WRITE");
                        let path = match filename { Value::Str(s) => s, _ => return Err("WRITE filename must be a string".to_string()) };
                        let data = match content { Value::Str(s) => s, _ => return Err("WRITE content must be a string".to_string()) };
                        match std::fs::write(&path, data.as_bytes()) {
                            Ok(_) => stack.push(Value::Str(data)),
                            Err(e) => return Err(format!("WRITE error: {}", e)),
                        }
                    }
                    "fission" => {
                        if *arg_count != 2 { return Err("FISSION expects exactly 2 arguments".to_string()); }
                        let string_val = stack.pop().expect("Expected string for FISSION");
                        let delim_val = stack.pop().expect("Expected delimiter for FISSION");
                        let s = match string_val { Value::Str(s) => s, _ => return Err("FISSION: string must be a string".to_string()) };
                        let delim = match delim_val { Value::Str(s) => s, _ => return Err("FISSION: delimiter must be a string".to_string()) };
                        let parts: Vec<Value> = if delim.is_empty() {
                            s.chars().map(|c| Value::Str(c.to_string())).collect()
                        } else {
                            s.split(&delim).map(|p| Value::Str(p.to_string())).collect()
                        };
                        stack.push(Value::Box(parts));
                    }
                    "fusion" => {
                        if *arg_count != 2 { return Err("FUSION expects exactly 2 arguments".to_string()); }
                        let strings_val = stack.pop().expect("Expected strings for FUSION");
                        let delim_val = stack.pop().expect("Expected delimiter for FUSION");
                        let delim = match delim_val { Value::Str(s) => s, _ => return Err("FUSION: delimiter must be a string".to_string()) };
                        let items = match strings_val {
                            Value::Box(vs) => {
                                let mut out = Vec::new();
                                for v in vs {
                                    match v {
                                        Value::Str(s) => out.push(s),
                                        _ => return Err("FUSION: all elements must be strings".to_string()),
                                    }
                                }
                                out
                            }
                            _ => return Err("FUSION: second argument must be a box of strings".to_string()),
                        };
                        let joined = items.join(&delim);
                        stack.push(Value::Str(joined));
                    }
                    "count" => {
                        if *arg_count != 1 {
                            return Err("COUNT expects exactly 1 argument".to_string());
                        }
                        let arg = stack.pop().expect("Expected argument for COUNT");
                        match arg {
                            Value::Box(contents) => stack.push(Value::Int(contents.len() as i64)),
                            Value::Str(s) => stack.push(Value::Int(s.chars().count() as i64)),
                            Value::Function { name: _n, params, body: _b, .. } => stack.push(Value::Int(params.len() as i64)),
                            _ => return Err("COUNT expects box, string, or function".to_string()),
                        }
                    }
                    "result" => {
                        if *arg_count != 1 {
                            return Err("RESULT expects exactly 1 argument".to_string());
                        }
                        let func_value = stack.pop().expect("Expected function for RESULT");
                        match func_value {
                            Value::Function { name: _name, params, body, rewire_target } => {
                                // Execute the function with its captured parameters
                                // For now, we'll execute it in the current environment
                                // TODO: Implement proper closure environment
                                
                                // Create a new environment that includes the current environment
                                // This allows functions to access global variables
                                let mut func_env = env.clone();
                                
                                // Now bind the function parameters to their values from the current environment
                                // This simulates what would happen in a real function call
                                for param_name in params.iter() {
                                    if let Some(value) = env.get(param_name) {
                                        func_env.insert(param_name.clone(), value.clone());
                                    }
                                }
                                
                                let mut func_env_mut = func_env;
                                let result = execute_function(&body, &mut stack, &mut func_env_mut)?;
                                // If this is a rewire function, propagate the target back to env
                                if let Some(target) = rewire_target {
                                    if let Some(new_val) = func_env_mut.get(&target) {
                                        env.insert(target.clone(), new_val.clone());
                                    }
                                }
                                stack.push(result);
                            }
                            _ => return Err("RESULT expects a function".to_string()),
                        }
                    }
                    _ => {
                        // Handle user-defined functions (pure by default): run in cloned env
                        match func {
                            Value::Function { name, params, body, .. } => {
                                if params.len() != *arg_count {
                                    return Err(format!("Function '{}' expects {} arguments, got {}", 
                                        name, params.len(), arg_count));
                                }
                                let body_clone = body.clone();
                                let mut env_clone = env.clone();
                                let result = execute_function(&body_clone, &mut stack, &mut env_clone)?;
                                stack.push(result);
                            }
                            _ => return Err(format!("'{}' is not a function", func_name)),
                        }
                    }
                }
            }


            OpCode::MakeFuncWithBody { name, parameters, body, rewire_target } => {
                // Create a function value with the actual compiled body
                let func_value = Value::Function {
                    name: name.clone(),
                    params: parameters.clone(),
                    body: body.clone(),
                    rewire_target: rewire_target.clone(),
                };
                env.insert(name.clone(), func_value);
            }
            
            OpCode::Halt => break,
        }
        
        // Normal instruction progression
        ip += 1;
    }
    Ok(())
}

/// Build a map from label names to instruction indices
fn build_label_map(program: &[OpCode]) -> Result<HashMap<String, usize>, String> {
    let mut label_map = HashMap::new();
    
    for (index, opcode) in program.iter().enumerate() {
        if let OpCode::Label(name) = opcode {
            if label_map.contains_key(name) {
                return Err(format!("Duplicate label: {}", name));
            }
            label_map.insert(name.clone(), index);
        }
    }
    
    Ok(label_map)
}
