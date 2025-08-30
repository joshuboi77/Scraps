#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Box(Vec<Value>),
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<crate::bytecode::OpCode>,
        rewire_target: Option<String>,
    },
    None,
}

impl Value {
    /// Format a value for display, with special handling for functions
    pub fn format_for_display(&self) -> String {
        match self {
            Value::Function { name: _name, params, body, rewire_target: _ } => {
                let param_list = params.join(", ");
                let body_str = if body.is_empty() { "..." } else { "..." };
                format!("fn(use({})) {{ {} }}", param_list, body_str)
            }
            Value::Int(n) => n.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => if *b { "TRUE".to_string() } else { "FALSE".to_string() },
            Value::Str(s) => s.clone(),
            Value::Box(contents) => {
                if contents.is_empty() {
                    "[]".to_string()
                } else {
                    let items: Vec<String> = contents.iter().map(|v| v.format_for_display()).collect();
                    format!("[{}]", items.join(", "))
                }
            }
            Value::None => "None".to_string(),
        }
    }
}
