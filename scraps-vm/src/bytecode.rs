

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    // Stack operations
    PushInt(i64),
    PushFloat(f64),
    PushBool(bool),
    PushStr(String),
    
    // Box operations
    MakeBox,
    Pack,
    Place(usize),
    Unpack(u8),   // unpack operation: arity 1 = box,start; arity 2 = box,start,end
    Pick(usize),  // Pick multiple elements from a box
    
    // Arithmetic operations
    Add,
    Sub,
    Mul,
    Div,
    
    // Comparison operations
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    
    // Logical operations
    And,
    Or,
    Not,
    
    // Control flow
    Jump(usize),           // Absolute jump to instruction index

    JumpIfNot(usize),      // Conditional jump if false

    Label(String),          // Label for jump targets (no-op during execution)
    
    // Variables
    LoadVar(String),
    StoreVar(String),
    
    // Functions
    Call(String, usize), // function name, arg count

    MakeFuncWithBody {
        name: String,
        parameters: Vec<String>,
        body: Vec<OpCode>,
        rewire_target: Option<String>,
    },
    
    // I/O and utilities
    Print,
    Halt,
    
    // Network I/O - TCP Sockets
    TcpConnect(String, u16),    // host, port -> connection_id
    TcpSend(usize),             // connection_id, data -> success
    TcpReceive(usize),          // connection_id, max_bytes -> data
    TcpClose(usize),            // connection_id -> success
    TcpListen(u16),             // port -> listener_id
    TcpAccept(usize),           // listener_id -> connection_id
}
