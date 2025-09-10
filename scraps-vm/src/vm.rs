use std::collections::HashMap;
use crate::value::Value;
use crate::bytecode::OpCode;
use std::io::Write;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::compiler::Compiler;
use std::fs;
use std::path::{Path, PathBuf};
use crate::tcp_socket_manager::TcpSocketManager;
use crate::websocket_manager::WebSocketManager;
use crate::udp_socket_manager::UdpSocketManager;
use crate::tls_socket_manager::TlsSocketManager;
use crate::event_loop_manager::{EventLoopManager, SocketType, EventType, EventFilter};
use crate::connection_pool_manager::ConnectionPoolManager;
use crate::timeout_manager::TimeoutManager;
use crate::proxy_manager::ProxyManager;
use crate::raw_socket_manager::RawSocketManager;
use crate::network_interface_manager::{NetworkInterfaceManager, get_best_interface_for_binding, get_interface_by_ip};
use crate::ipv6_manager::{IPv6Manager, DualStackMode, create_dual_stack_socket_addr};
use std::sync::{Mutex, OnceLock};
use ureq;
use serde_json::{self, Value as JsonValue, Number as JsonNumber};
// std::time and std::thread imports removed as they're unused
use base64::{engine::general_purpose, Engine as _};
use urlencoding;

const REWIRED_KEY: &str = "__rewired__";

static TCP_MANAGER_GLOBAL: OnceLock<Mutex<TcpSocketManager>> = OnceLock::new();
static WS_MANAGER_GLOBAL: OnceLock<Mutex<WebSocketManager>> = OnceLock::new();
static UDP_MANAGER_GLOBAL: OnceLock<Mutex<UdpSocketManager>> = OnceLock::new();
static TLS_MANAGER_GLOBAL: OnceLock<Mutex<TlsSocketManager>> = OnceLock::new();
static EVENT_LOOP_GLOBAL: OnceLock<Mutex<EventLoopManager>> = OnceLock::new();
static POOL_MANAGER_GLOBAL: OnceLock<Mutex<ConnectionPoolManager>> = OnceLock::new();
static TIMEOUT_MANAGER_GLOBAL: OnceLock<Mutex<TimeoutManager>> = OnceLock::new();
static PROXY_MANAGER_GLOBAL: OnceLock<Mutex<ProxyManager>> = OnceLock::new();
static RAW_MANAGER_GLOBAL: OnceLock<Mutex<RawSocketManager>> = OnceLock::new();
static INTERFACE_MANAGER_GLOBAL: OnceLock<Mutex<NetworkInterfaceManager>> = OnceLock::new();
static IPV6_MANAGER_GLOBAL: OnceLock<Mutex<IPv6Manager>> = OnceLock::new();

fn tcp_manager_global() -> &'static Mutex<TcpSocketManager> {
    TCP_MANAGER_GLOBAL.get_or_init(|| Mutex::new(TcpSocketManager::new()))
}

fn ws_manager_global() -> &'static Mutex<WebSocketManager> {
    WS_MANAGER_GLOBAL.get_or_init(|| Mutex::new(WebSocketManager::new()))
}

fn udp_manager_global() -> &'static Mutex<UdpSocketManager> {
    UDP_MANAGER_GLOBAL.get_or_init(|| Mutex::new(UdpSocketManager::new()))
}

fn tls_manager_global() -> &'static Mutex<TlsSocketManager> {
    TLS_MANAGER_GLOBAL.get_or_init(|| Mutex::new(TlsSocketManager::new()))
}

fn event_loop_global() -> &'static Mutex<EventLoopManager> {
    EVENT_LOOP_GLOBAL.get_or_init(|| Mutex::new(EventLoopManager::new()))
}

fn pool_manager_global() -> &'static Mutex<ConnectionPoolManager> {
    POOL_MANAGER_GLOBAL.get_or_init(|| Mutex::new(ConnectionPoolManager::new()))
}

fn timeout_manager_global() -> &'static Mutex<TimeoutManager> {
    TIMEOUT_MANAGER_GLOBAL.get_or_init(|| Mutex::new(TimeoutManager::new()))
}

fn proxy_manager_global() -> &'static Mutex<ProxyManager> {
    PROXY_MANAGER_GLOBAL.get_or_init(|| Mutex::new(ProxyManager::new()))
}

fn raw_manager_global() -> &'static Mutex<RawSocketManager> {
    RAW_MANAGER_GLOBAL.get_or_init(|| Mutex::new(RawSocketManager::new()))
}

fn interface_manager_global() -> &'static Mutex<NetworkInterfaceManager> {
    INTERFACE_MANAGER_GLOBAL.get_or_init(|| Mutex::new(NetworkInterfaceManager::new()))
}

fn ipv6_manager_global() -> &'static Mutex<IPv6Manager> {
    IPV6_MANAGER_GLOBAL.get_or_init(|| Mutex::new(IPv6Manager::new()))
}

fn parse_headers_box(val: Value) -> Result<Vec<(String, String)>, String> {
    match val {
        Value::Box(items) => {
            let mut out = Vec::new();
            for pair in items {
                match pair {
                    Value::Box(kv) if kv.len() == 2 => {
                        let k = match &kv[0] { Value::Str(s) => s.clone(), _ => return Err("HTTP headers: key must be string".to_string()) };
                        let v = match &kv[1] { Value::Str(s) => s.clone(), _ => return Err("HTTP headers: value must be string".to_string()) };
                        out.push((k, v));
                    }
                    _ => return Err("HTTP headers must be [[key, value], ...]".to_string()),
                }
            }
            Ok(out)
        }
        _ => Err("HTTP headers must be a box of pairs".to_string()),
    }
}

fn headers_to_value(headers: &[(String, String)]) -> Value {
    let mut pairs = Vec::with_capacity(headers.len());
    for (k, v) in headers {
        pairs.push(Value::Box(vec![Value::Str(k.clone()), Value::Str(v.clone())]));
    }
    Value::Box(pairs)
}

fn http_get_impl(url: &str, headers: Option<&[(String, String)]>) -> Result<(i64, Vec<(String, String)>, String), String> {
    let mut req = ureq::get(url);
    if let Some(hs) = headers { for (k, v) in hs { req = req.set(k, v); } }
    let resp = req.call().map_err(|e| format!("HTTP_GET error: {}", e))?;
    let status = resp.status() as i64;
    let names = resp.headers_names();
    let mut hs = Vec::new();
    for n in names { if let Some(v) = resp.header(&n) { hs.push((n.clone(), v.to_string())); } }
    let body = resp.into_string().map_err(|e| format!("HTTP_GET body error: {}", e))?;
    Ok((status, hs, body))
}

fn http_post_impl(url: &str, body: &str, headers: Option<&[(String, String)]>) -> Result<(i64, Vec<(String, String)>, String), String> {
    let mut req = ureq::post(url);
    let mut has_ct = false;
    if let Some(hs) = headers {
        for (k, v) in hs {
            if k.eq_ignore_ascii_case("content-type") { has_ct = true; }
            req = req.set(k, v);
        }
    }
    if !has_ct { req = req.set("Content-Type", "text/plain; charset=utf-8"); }
    let resp = req.send_string(body).map_err(|e| format!("HTTP_POST error: {}", e))?;
    let status = resp.status() as i64;
    let names = resp.headers_names();
    let mut hs = Vec::new();
    for n in names { if let Some(v) = resp.header(&n) { hs.push((n.clone(), v.to_string())); } }
    let body = resp.into_string().map_err(|e| format!("HTTP_POST body error: {}", e))?;
    Ok((status, hs, body))
}

fn http_put_impl(url: &str, body: &str, headers: Option<&[(String, String)]>) -> Result<(i64, Vec<(String, String)>, String), String> {
    let mut req = ureq::put(url);
    let mut has_ct = false;
    if let Some(hs) = headers {
        for (k, v) in hs {
            if k.eq_ignore_ascii_case("content-type") { has_ct = true; }
            req = req.set(k, v);
        }
    }
    if !has_ct { req = req.set("Content-Type", "text/plain; charset=utf-8"); }
    let resp = req.send_string(body).map_err(|e| format!("HTTP_PUT error: {}", e))?;
    let status = resp.status() as i64;
    let names = resp.headers_names();
    let mut hs = Vec::new();
    for n in names { if let Some(v) = resp.header(&n) { hs.push((n.clone(), v.to_string())); } }
    let body = resp.into_string().map_err(|e| format!("HTTP_PUT body error: {}", e))?;
    Ok((status, hs, body))
}

fn http_delete_impl(url: &str, headers: Option<&[(String, String)]>) -> Result<(i64, Vec<(String, String)>, String), String> {
    let mut req = ureq::delete(url);
    if let Some(hs) = headers { for (k, v) in hs { req = req.set(k, v); } }
    let resp = req.call().map_err(|e| format!("HTTP_DELETE error: {}", e))?;
    let status = resp.status() as i64;
    let names = resp.headers_names();
    let mut hs = Vec::new();
    for n in names { if let Some(v) = resp.header(&n) { hs.push((n.clone(), v.to_string())); } }
    let body = resp.into_string().map_err(|e| format!("HTTP_DELETE body error: {}", e))?;
    Ok((status, hs, body))
}

fn dns_resolve_impl(hostname: &str) -> Result<String, String> {
    use std::net::ToSocketAddrs;
    
    // Try to resolve the hostname to socket addresses
    let mut addrs = format!("{}:80", hostname)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{}': {}", hostname, e))?;
    
    // Get the first resolved address
    if let Some(addr) = addrs.next() {
        Ok(addr.ip().to_string())
    } else {
        Err(format!("No addresses found for hostname '{}'", hostname))
    }
}

fn value_to_json(v: &Value) -> Result<JsonValue, String> {
    match v {
        Value::None => Ok(JsonValue::Null),
        Value::Bool(b) => Ok(JsonValue::Bool(*b)),
        Value::Int(n) => Ok(JsonValue::Number(JsonNumber::from(*n))),
        Value::Float(f) => JsonNumber::from_f64(*f)
            .map(JsonValue::Number)
            .ok_or_else(|| "JSON encode: invalid float".to_string()),
        Value::Str(s) => Ok(JsonValue::String(s.clone())),
        Value::Box(items) => {
            // If all items are [Str key, value] pairs, encode as object; else as array
            let mut all_pairs = true;
            for it in items.iter() {
                match it {
                    Value::Box(kv) if kv.len() == 2 && matches!(kv[0], Value::Str(_)) => {}
                    _ => { all_pairs = false; break; }
                }
            }
            if all_pairs {
                let mut obj = serde_json::Map::new();
                for it in items.iter() {
                    if let Value::Box(kv) = it {
                        let key = if let Value::Str(s) = &kv[0] { s.clone() } else { unreachable!() };
                        let val = value_to_json(&kv[1])?;
                        obj.insert(key, val);
                    }
                }
                Ok(JsonValue::Object(obj))
            } else {
                let mut arr = Vec::with_capacity(items.len());
                for it in items.iter() { arr.push(value_to_json(it)?); }
                Ok(JsonValue::Array(arr))
            }
        }
        Value::Function { .. } => Err("JSON encode: cannot encode function".to_string()),
        Value::TcpConnection(_) | Value::TcpListener(_) | Value::WebSocket(_) | Value::UdpSocket(_) | Value::TlsConnection(_) | Value::TlsListener(_) | Value::RawSocket(_) => Err("JSON encode: cannot encode socket handle".to_string()),
    }
}

fn json_to_value(j: &JsonValue) -> Value {
    match j {
        JsonValue::Null => Value::None,
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else if let Some(f) = n.as_f64() { Value::Float(f) }
            else { Value::Float(0.0) }
        }
        JsonValue::String(s) => Value::Str(s.clone()),
        JsonValue::Array(xs) => {
            let mut vs = Vec::with_capacity(xs.len());
            for x in xs { vs.push(json_to_value(x)); }
            Value::Box(vs)
        }
        JsonValue::Object(map) => {
            // Represent objects as box of [key, value] pairs
            let mut pairs = Vec::with_capacity(map.len());
            for (k, v) in map.iter() {
                pairs.push(Value::Box(vec![Value::Str(k.clone()), json_to_value(v)]));
            }
            Value::Box(pairs)
        }
    }
}

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

fn parse_modules_manifest(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut in_modules = false;
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        if t.starts_with('[') {
            in_modules = t == "[modules]";
            continue;
        }
        if !in_modules { continue; }
        if let Some(eq) = t.find('=') {
            let key = t[..eq].trim().to_string();
            let mut val = t[eq+1..].trim().to_string();
            if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                val = val[1..val.len()-1].to_string();
            }
            if !key.is_empty() && !val.is_empty() { map.insert(key, val); }
        }
    }
    map
}

fn read_modules_manifest_from(path: &Path) -> HashMap<String, String> {
    match fs::read_to_string(path) { Ok(s) => parse_modules_manifest(&s), Err(_) => HashMap::new() }
}

fn find_upwards(start: &Path, filename: &str) -> Option<PathBuf> {
    let mut p = start.to_path_buf();
    let fname = Path::new(filename);
    loop {
        let candidate = p.join(fname);
        if candidate.exists() { return Some(candidate); }
        if !p.pop() { break; }
    }
    None
}

fn read_clanker_manifest() -> Option<(PathBuf, HashMap<String, String>, Option<String>)> {
    // 0) Explicit override via env var IGNITE (file or directory containing clanker.toml)
    if let Ok(path_str) = std::env::var("IGNITE") {
        let p = PathBuf::from(path_str);
        let cfg_path = if p.is_dir() { p.join("clanker.toml") } else { p };
        if let Ok(content) = fs::read_to_string(&cfg_path) {
            let modules = parse_modules_manifest(&content);
            let mut sources_dir: Option<String> = None;
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("sources") && t.contains('=') {
                    if let Some(eq) = t.find('=') {
                        let mut val = t[eq+1..].trim().to_string();
                        if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                            val = val[1..val.len()-1].to_string();
                        }
                        if !val.is_empty() { sources_dir = Some(val); }
                    }
                }
            }
            return Some((cfg_path.parent().unwrap_or(Path::new(".")).to_path_buf(), modules, sources_dir));
        }
    }

    // 1) Search for clanker.toml upwards from CWD
    let cwd = std::env::current_dir().ok()?;
    let cfg_path = find_upwards(&cwd, "clanker.toml")?;
    let content = fs::read_to_string(&cfg_path).ok()?;
    let modules = parse_modules_manifest(&content);
    // Optional [paths] sources = "void"
    let mut sources_dir: Option<String> = None;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("sources") && t.contains('=') {
            if let Some(eq) = t.find('=') {
                let mut val = t[eq+1..].trim().to_string();
                if val.starts_with('"') && val.ends_with('"') && val.len() >= 2 {
                    val = val[1..val.len()-1].to_string();
                }
                if !val.is_empty() { sources_dir = Some(val); }
            }
        }
    }
    Some((cfg_path.parent().unwrap_or(Path::new(".")).to_path_buf(), modules, sources_dir))
}

fn read_scraps_manifest() -> Option<(PathBuf, HashMap<String, String>)> {
    let cwd = std::env::current_dir().ok()?;
    let path = find_upwards(&cwd, "Scraps.toml")?;
    let map = read_modules_manifest_from(&path);
    Some((path.parent().unwrap_or(Path::new(".")).to_path_buf(), map))
}

fn load_module_from_key(env: &mut HashMap<String, Value>, key: &str) -> Result<(), String> {
    // 1) clanker.toml support (preferred)
    if let Some((base_dir, modules, sources_dir)) = read_clanker_manifest() {
        if let Some(rel) = modules.get(key) {
            let candidate = if let Some(sdir) = &sources_dir { base_dir.join(sdir).join(rel) } else { base_dir.join(rel) };
            let code = fs::read_to_string(&candidate)
                .map_err(|e| format!("SOURCE error: {} (path: {:?})", e, candidate))?;
            let _ = eval_snippet(env, &code)?;
            return Ok(());
        }
    }
    // 2) Scraps.toml fallback
    if let Some((base_dir, manifest)) = read_scraps_manifest() {
        if let Some(rel) = manifest.get(key) {
            let candidate = base_dir.join(rel);
            let code = fs::read_to_string(&candidate)
                .map_err(|e| format!("SOURCE error: {} (path: {:?})", e, candidate))?;
            let _ = eval_snippet(env, &code)?;
            return Ok(());
        }
    }
    // 3) Direct filepath
    let code = fs::read_to_string(key).map_err(|_| format!("IMPORT: module key '{}' not found and file '{}' unreadable", key, key))?;
    let _ = eval_snippet(env, &code)?;
    Ok(())
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
                    // Ensure deep cloning of boxes to maintain immutability
                    let cloned_val = match val {
                        Value::Box(contents) => {
                            // Deep clone the box contents to prevent shared mutable references
                            Value::Box(contents.clone())
                        }
                        other => other.clone()
                    };
                    local_stack.push(cloned_val);
                } else {
                    return Err(format!("Undefined variable '{}' in function", name));
                }
            }
            OpCode::MakeFuncWithBody { name, parameters, body, rewire_target } => {
                // Create function within the local (function) environment
                let func_value = Value::Function {
                    name: name.clone(),
                    params: parameters.clone(),
                    body: body.clone(),
                    rewire_target: rewire_target.clone(),
                };
                local_env.insert(name.clone(), func_value);
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
                let func = local_env.get(&func_name).cloned().ok_or("Function not found")?;
                match func_name.as_str() {
                    "box" => { if *arg_count != 0 { return Err("BOX expects 0 arguments".to_string()); } local_stack.push(Value::Box(vec![])); }
                    "http_get" => {
                        if *arg_count < 1 || *arg_count > 2 { return Err("HTTP_GET expects 1 or 2 arguments (url[, headers])".to_string()); }
                        let url_val = local_stack.pop().expect("Expected URL for HTTP_GET");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_GET url must be a string".to_string()) };
                        let headers = if *arg_count == 2 {
                            let headers_val = local_stack.pop().expect("Expected headers for HTTP_GET");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let (status, hs, body) = http_get_impl(&url, headers.as_deref())?;
                        let resp = Value::Box(vec![
                            Value::Int(status),
                            headers_to_value(&hs),
                            Value::Str(body),
                        ]);
                        local_stack.push(resp);
                    }
                    "json_encode" => {
                        if *arg_count != 1 { return Err("JSON_ENCODE expects exactly 1 argument".to_string()); }
                        let val = local_stack.pop().expect("Expected value for JSON_ENCODE");
                        let j = value_to_json(&val)?;
                        let s = serde_json::to_string(&j).map_err(|e| format!("JSON_ENCODE error: {}", e))?;
                        local_stack.push(Value::Str(s));
                    }
                    "json_decode" => {
                        if *arg_count != 1 { return Err("JSON_DECODE expects exactly 1 argument".to_string()); }
                        let sval = local_stack.pop().expect("Expected string for JSON_DECODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("JSON_DECODE argument must be a string".to_string()) };
                        let j: JsonValue = serde_json::from_str(&s).map_err(|e| format!("JSON_DECODE error: {}", e))?;
                        let v = json_to_value(&j);
                        local_stack.push(v);
                    }
                    "http_post" => {
                        if *arg_count < 2 || *arg_count > 3 { return Err("HTTP_POST expects 2 or 3 arguments (url, data[, headers])".to_string()); }
                        let headers = if *arg_count == 3 {
                            let headers_val = local_stack.pop().expect("Expected headers for HTTP_POST");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let data_val = local_stack.pop().expect("Expected data for HTTP_POST");
                        let url_val = local_stack.pop().expect("Expected URL for HTTP_POST");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_POST url must be a string".to_string()) };
                        let data = match data_val { Value::Str(s) => s, _ => return Err("HTTP_POST data must be a string".to_string()) };
                        let (status, hs, body) = http_post_impl(&url, &data, headers.as_deref())?;
                        let resp = Value::Box(vec![
                            Value::Int(status),
                            headers_to_value(&hs),
                            Value::Str(body),
                        ]);
                        local_stack.push(resp);
                    }
                    "http_put" => {
                        if *arg_count < 2 || *arg_count > 3 { return Err("HTTP_PUT expects 2 or 3 arguments (url, data[, headers])".to_string()); }
                        let headers = if *arg_count == 3 {
                            let headers_val = local_stack.pop().expect("Expected headers for HTTP_PUT");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let data_val = local_stack.pop().expect("Expected data for HTTP_PUT");
                        let url_val = local_stack.pop().expect("Expected URL for HTTP_PUT");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_PUT url must be a string".to_string()) };
                        let data = match data_val { Value::Str(s) => s, _ => return Err("HTTP_PUT data must be a string".to_string()) };
                        let (status, hs, body) = http_put_impl(&url, &data, headers.as_deref())?;
                        let resp = Value::Box(vec![ Value::Int(status), headers_to_value(&hs), Value::Str(body) ]);
                        local_stack.push(resp);
                    }
                    "base64_encode" => {
                        if *arg_count != 1 { return Err("BASE64_ENCODE expects exactly 1 argument".to_string()); }
                        let sval = local_stack.pop().expect("Expected string for BASE64_ENCODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("BASE64_ENCODE argument must be a string".to_string()) };
                        let out = general_purpose::STANDARD.encode(s.as_bytes());
                        local_stack.push(Value::Str(out));
                    }
                    "base64_decode" => {
                        if *arg_count != 1 { return Err("BASE64_DECODE expects exactly 1 argument".to_string()); }
                        let sval = local_stack.pop().expect("Expected string for BASE64_DECODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("BASE64_DECODE argument must be a string".to_string()) };
                        let bytes = general_purpose::STANDARD.decode(s.as_bytes()).map_err(|e| format!("BASE64_DECODE error: {}", e))?;
                        let out = String::from_utf8(bytes).map_err(|e| format!("BASE64_DECODE UTF-8 error: {}", e))?;
                        local_stack.push(Value::Str(out));
                    }
                    "url_encode" => {
                        if *arg_count != 1 { return Err("URL_ENCODE expects exactly 1 argument".to_string()); }
                        let sval = local_stack.pop().expect("Expected string for URL_ENCODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("URL_ENCODE argument must be a string".to_string()) };
                        let out = urlencoding::encode(&s).into_owned();
                        local_stack.push(Value::Str(out));
                    }
                    "url_decode" => {
                        if *arg_count != 1 { return Err("URL_DECODE expects exactly 1 argument".to_string()); }
                        let sval = local_stack.pop().expect("Expected string for URL_DECODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("URL_DECODE argument must be a string".to_string()) };
                        let out = urlencoding::decode(&s).map_err(|e| format!("URL_DECODE error: {}", e))?.into_owned();
                        local_stack.push(Value::Str(out));
                    }
                    "http_delete" => {
                        if *arg_count < 1 || *arg_count > 2 { return Err("HTTP_DELETE expects 1 or 2 arguments (url[, headers])".to_string()); }
                        let url_val = local_stack.pop().expect("Expected URL for HTTP_DELETE");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_DELETE url must be a string".to_string()) };
                        let headers = if *arg_count == 2 {
                            let headers_val = local_stack.pop().expect("Expected headers for HTTP_DELETE");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let (status, hs, body) = http_delete_impl(&url, headers.as_deref())?;
                        let resp = Value::Box(vec![ Value::Int(status), headers_to_value(&hs), Value::Str(body) ]);
                        local_stack.push(resp);
                    }
                    
                    "source" => {
                        if *arg_count != 1 { return Err("SOURCE expects exactly 1 argument".to_string()); }
                        let filename = local_stack.pop().expect("Expected filename for SOURCE");
                        let path = match filename { Value::Str(s) => s, _ => return Err("SOURCE filename must be a string".to_string()) };
                        let content = match std::fs::read_to_string(&path) { Ok(s) => s, Err(e) => return Err(format!("SOURCE error: {}", e)) };
                        let result = eval_snippet(local_env, &content)?;
                        local_stack.push(result);
                    }
                    "import" => {
                        if *arg_count == 1 {
                            let arg = local_stack.pop().expect("Expected argument for IMPORT");
                            match arg {
                                Value::Function { name, params, .. } => {
                                    if !params.is_empty() { return Err("IMPORT: function argument must take 0 parameters".to_string()); }
                                    let module_key = format!("__module_{}", name);
                                    let module_val = local_env.get(&module_key).ok_or_else(|| format!("IMPORT: module '{}' not found", name))?;
                                    let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                    let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); local_env.insert(key.clone(), val); count+=1; } } } }
                                    local_stack.push(Value::Str(format!("Module '{}' imported with {} exports", name, count)));
                                }
                                Value::Str(s) => {
                                    let module_key = format!("__module_{}", s);
                                    if let Some(module_val) = local_env.get(&module_key) {
                                        let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                        let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); local_env.insert(key.clone(), val); count+=1; } } } }
                                        local_stack.push(Value::Str(format!("Module '{}' imported with {} exports", s, count)));
                                    } else {
                                        let pre: Vec<String> = local_env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                                        load_module_from_key(local_env, &s)?;
                                        let post: Vec<String> = local_env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                                        let new_modules: Vec<String> = post.into_iter().filter(|k| !pre.contains(k)).collect();
                                        let mut total = 0;
                                        for mk in new_modules {
                                            if let Some(module_val) = local_env.get(&mk) {
                                                let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                                for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); local_env.insert(key.clone(), val); total+=1; } } } }
                                            }
                                        }
                                        local_stack.push(Value::Str(format!("Imported {} exports from '{}'", total, s)));
                                    }
                                }
                                _ => return Err("IMPORT: argument must be a string or function".to_string()),
                            }
                        } else if *arg_count >= 2 {
                            let src_key_val = local_stack.pop().expect("Expected source key for IMPORT");
                            let src_key = match src_key_val { Value::Str(s)=> s, _=> return Err("IMPORT: source key must be a string".to_string()) };
                            let mut selectors: Vec<String> = Vec::new();
                            for _ in 0..(*arg_count - 1) { match local_stack.pop().expect("Expected selector") { Value::Str(s)=> selectors.push(s), _=> return Err("IMPORT: selectors must be strings".to_string()) } }
                            selectors.reverse();
                            let pre: Vec<String> = local_env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                            load_module_from_key(local_env, &src_key)?;
                            let post: Vec<String> = local_env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                            let new_modules: Vec<String> = post.into_iter().filter(|k| !pre.contains(k)).collect();
                            if selectors.len() == 1 {
                                let module_name = &selectors[0];
                                let mk = format!("__module_{}", module_name);
                                if local_env.contains_key(&mk) {
                                    let module_val = local_env.get(&mk).unwrap();
                                    let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                    let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); local_env.insert(key.clone(), val); count+=1; } } } }
                                    local_stack.push(Value::Str(format!("Module '{}' imported with {} exports", module_name, count)));
                                    return Ok(Value::None);
                                } else if !new_modules.is_empty() {
                                    // Alias the first newly loaded module to the requested name, then import
                                    let old_mk = new_modules[0].clone();
                                    if let Some(mut module_val) = local_env.remove(&old_mk) {
                                        if let Value::Box(ref mut items) = module_val { if !items.is_empty() { items[0] = Value::Str(module_name.clone()); } }
                                        local_env.insert(mk.clone(), module_val);
                                        if let Some(module_val2) = local_env.get(&mk) {
                                            let exports_box = match module_val2 { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                            let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); local_env.insert(key.clone(), val); count+=1; } } } }
                                            local_stack.push(Value::Str(format!("Module '{}' imported with {} exports", module_name, count)));
                                            return Ok(Value::None);
                                        }
                                    }
                                }
                            }
                            let module_keys = if new_modules.is_empty() { pre } else { new_modules };
                            let mut count = 0;
                            for mk in module_keys {
                                if let Some(module_val) = local_env.get(&mk) {
                                    let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                    for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { if selectors.contains(key) { let val = kv[1].clone(); local_env.insert(key.clone(), val); count+=1; } } } } }
                                }
                            }
                            local_stack.push(Value::Str(format!("Imported {} selected exports from '{}'", count, src_key)));
                        } else {
                            return Err("IMPORT expects at least 1 argument".to_string());
                        }
                    }
                    "ship" => {
                        if *arg_count != 0 { return Err("SHIP expects 0 arguments".to_string()); }
                        // Get the function name from the environment
                        let func_name = func_name.clone();
                        let func = local_env.get(&func_name).cloned().ok_or("Function not found")?;
                        match func {
                            Value::Function { name, params, body, .. } => {
                                if !params.is_empty() { return Err("SHIP: function argument must take 0 parameters".to_string()); }
                                // Execute the factory in a cloned environment to collect its definitions
                                let mut module_env = local_env.clone();
                                let mut tmp_stack: Vec<Value> = Vec::new();
                                let _ = execute_function(&body, &mut tmp_stack, &mut module_env)?;
                                // Diff module_env against env to get new/changed definitions
                                let mut exports = HashMap::new();
                                for (key, value) in module_env.iter() {
                                    if !key.starts_with("__") &&
                                       !["box","pack","place","unpack","pick","count","print",
                                         "result","string","read","write","fission","fusion",
                                         "rewire_symbol","ship","import"].contains(&key.as_str()) {
                                        match local_env.get(key) {
                                            Some(old) if old == value => { /* unchanged; skip */ }
                                            _ => { exports.insert(key.clone(), value.clone()); }
                                        }
                                    }
                                }
                                // Convert exports to a boxed representation
                                let mut pairs: Vec<Value> = Vec::new();
                                for (k, v) in exports.iter() {
                                    pairs.push(Value::Box(vec![Value::Str(k.clone()), v.clone()]));
                                }
                                let module_key = format!("__module_{}", name);
                                local_env.insert(module_key, Value::Box(vec![
                                    Value::Str(name.clone()),
                                    Value::Box(pairs.clone()),
                                ]));
                                local_stack.push(Value::Str(format!("Module '{}' shipped with {} exports", name, exports.len())));
                            }
                            _ => return Err("SHIP: module must be a function".to_string()),
                        }
                    }
                    
                    // Core-8 Hardware Functions
                    "time_counter" => {
                        if *arg_count != 0 { return Err("TIME_COUNTER expects no arguments".to_string()); }
                        
                        // Get high-resolution monotonic time counter
                        let counter = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_err(|_| "Failed to get system time")?
                            .as_nanos() as u64;
                        
                        local_stack.push(Value::Int(counter as i64));
                    }
                    "time_freq" => {
                        if *arg_count != 0 { return Err("TIME_FREQ expects no arguments".to_string()); }
                        
                        // Return frequency in Hz - nanosecond counter runs at 1 billion Hz
                        local_stack.push(Value::Int(1_000_000_000));
                    }
                    "cpu_halt" => {
                        if *arg_count != 0 { return Err("CPU_HALT expects no arguments".to_string()); }
                        
                        // Low-power halt until interrupt - simulate with short sleep to yield CPU
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        
                        local_stack.push(Value::Bool(true));
                    }
                    "mem_fence" => {
                        if *arg_count != 0 { return Err("MEM_FENCE expects no arguments".to_string()); }
                        
                        // Full memory barrier - prevents reordering of loads/stores across it
                        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
                        
                        local_stack.push(Value::Bool(true));
                    }
                    "mem_load" => {
                        if *arg_count != 2 { return Err("MEM_LOAD expects exactly 2 arguments (ptr, width)".to_string()); }
                        let width_val = local_stack.pop().expect("Expected width for MEM_LOAD");
                        let ptr_val = local_stack.pop().expect("Expected ptr for MEM_LOAD");
                        
                        let ptr: u64 = match ptr_val { 
                            Value::Int(n) => n as u64, 
                            _ => return Err("MEM_LOAD ptr must be an integer".to_string()) 
                        };
                        let width: u8 = match width_val { 
                            Value::Int(n) => {
                                if n < 1 || n > 8 || (n != 1 && n != 2 && n != 4 && n != 8) {
                                    return Err("MEM_LOAD width must be 1, 2, 4, or 8".to_string());
                                }
                                n as u8
                            }, 
                            _ => return Err("MEM_LOAD width must be an integer".to_string()) 
                        };
                        
                        // Simulate memory load - in real implementation this would be actual memory access
                        // For simulation, return a value based on ptr and width for predictable testing
                        let value = match width {
                            1 => (ptr & 0xFF) as i64,                    // 8-bit
                            2 => ((ptr & 0xFFFF) + 0x100) as i64,       // 16-bit  
                            4 => ((ptr & 0xFFFFFFFF) + 0x10000) as i64, // 32-bit
                            8 => (ptr + 0x100000000) as i64,            // 64-bit
                            _ => unreachable!()
                        };
                        
                        local_stack.push(Value::Int(value));
                    }
                    "mem_store" => {
                        if *arg_count != 3 { return Err("MEM_STORE expects exactly 3 arguments (ptr, val, width)".to_string()); }
                        let width_val = local_stack.pop().expect("Expected width for MEM_STORE");
                        let val_val = local_stack.pop().expect("Expected val for MEM_STORE");
                        let ptr_val = local_stack.pop().expect("Expected ptr for MEM_STORE");
                        
                        let _ptr: u64 = match ptr_val { 
                            Value::Int(n) => n as u64, 
                            _ => return Err("MEM_STORE ptr must be an integer".to_string()) 
                        };
                        let val: i64 = match val_val { 
                            Value::Int(n) => n, 
                            _ => return Err("MEM_STORE val must be an integer".to_string()) 
                        };
                        let width: u8 = match width_val { 
                            Value::Int(n) => {
                                if n < 1 || n > 8 || (n != 1 && n != 2 && n != 4 && n != 8) {
                                    return Err("MEM_STORE width must be 1, 2, 4, or 8".to_string());
                                }
                                n as u8
                            }, 
                            _ => return Err("MEM_STORE width must be an integer".to_string()) 
                        };
                        
                        // Simulate memory store - in real implementation this would be actual memory write
                        // For simulation, we just validate parameters and return success
                        // In a real implementation, this would write val to memory at ptr with specified width
                        
                        // Validate value fits in specified width
                        let max_val = match width {
                            1 => 255i64,                    // 8-bit: 0-255
                            2 => 65535i64,                  // 16-bit: 0-65535  
                            4 => 4294967295i64,             // 32-bit: 0-4294967295
                            8 => i64::MAX,                  // 64-bit: full range
                            _ => unreachable!()
                        };
                        
                        if val < 0 || val > max_val {
                            return Err(format!("MEM_STORE value {} out of range for width {}", val, width));
                        }
                        
                        local_stack.push(Value::Bool(true));
                    }
                    "mem_cmpxchg" => {
                        if *arg_count != 3 { return Err("MEM_CMPXCHG expects exactly 3 arguments (ptr, expect, val)".to_string()); }
                        let val_val = local_stack.pop().expect("Expected val for MEM_CMPXCHG");
                        let expect_val = local_stack.pop().expect("Expected expect for MEM_CMPXCHG");
                        let ptr_val = local_stack.pop().expect("Expected ptr for MEM_CMPXCHG");
                        
                        let ptr: u64 = match ptr_val { 
                            Value::Int(n) => n as u64, 
                            _ => return Err("MEM_CMPXCHG ptr must be an integer".to_string()) 
                        };
                        let expect: i64 = match expect_val { 
                            Value::Int(n) => n, 
                            _ => return Err("MEM_CMPXCHG expect must be an integer".to_string()) 
                        };
                        let _val: i64 = match val_val { 
                            Value::Int(n) => n, 
                            _ => return Err("MEM_CMPXCHG val must be an integer".to_string()) 
                        };
                        
                        // Simulate atomic compare-exchange operation
                        // In real implementation, this would be actual atomic CAS with acquire-release semantics
                        // For simulation, we'll simulate the behavior based on ptr value
                        
                        // Simulate current value at memory location (based on ptr for predictable testing)
                        let current_value = ((ptr & 0xFFFFFFFF) + 0x20000) as i64;
                        
                        // Perform compare-and-swap logic
                        let old_value = if current_value == expect {
                            // Values match - would write new value in real implementation
                            // Return the old value (which equals expect)
                            current_value
                        } else {
                            // Values don't match - no write occurs
                            // Return the actual current value
                            current_value
                        };
                        
                        local_stack.push(Value::Int(old_value));
                    }
                    "int_disable" => {
                        if *arg_count != 0 { return Err("INT_DISABLE expects no arguments".to_string()); }
                        
                        // Simulate interrupt disable operation
                        // In real implementation, this would disable maskable interrupts
                        // and return the previous interrupt flag state
                        
                        // For simulation, we'll return a consistent "previous state" value
                        // In real hardware, this would be the actual interrupt flag from CPU status register
                        let previous_interrupt_state = 1i64; // Simulate interrupts were enabled
                        
                        local_stack.push(Value::Int(previous_interrupt_state));
                    }
                    "int_enable" => {
                        if *arg_count != 1 { return Err("INT_ENABLE expects exactly 1 argument (interrupt_state)".to_string()); }
                        let state_val = local_stack.pop().expect("Expected interrupt_state for INT_ENABLE");
                        
                        let interrupt_state: i64 = match state_val { 
                            Value::Int(n) => n, 
                            _ => return Err("INT_ENABLE interrupt_state must be an integer".to_string()) 
                        };
                        
                        // Simulate interrupt enable operation
                        // In real implementation, this would restore the interrupt flag state
                        // and re-enable maskable interrupts if the state indicates they were enabled
                        
                        // For simulation, we validate the state and return success
                        // In real hardware, this would restore the CPU interrupt flag
                        let success = if interrupt_state == 0 || interrupt_state == 1 {
                            true  // Valid interrupt state (0=disabled, 1=enabled)
                        } else {
                            false // Invalid interrupt state
                        };
                        
                        local_stack.push(Value::Bool(success));
                    }
                    
                    // Character classification functions
                    "is_digit" => {
                        if *arg_count != 1 { return Err("IS_DIGIT expects exactly 1 argument (char)".to_string()); }
                        let char_val = local_stack.pop().expect("Expected char for IS_DIGIT");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_digit()
                                } else {
                                    return Err("IS_DIGIT requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_DIGIT argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Bool(result));
                    }
                    "is_alpha" => {
                        if *arg_count != 1 { return Err("IS_ALPHA expects exactly 1 argument (char)".to_string()); }
                        let char_val = local_stack.pop().expect("Expected char for IS_ALPHA");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_alphabetic()
                                } else {
                                    return Err("IS_ALPHA requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_ALPHA argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Bool(result));
                    }
                    "is_space" => {
                        if *arg_count != 1 { return Err("IS_SPACE expects exactly 1 argument (char)".to_string()); }
                        let char_val = local_stack.pop().expect("Expected char for IS_SPACE");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_whitespace()
                                } else {
                                    return Err("IS_SPACE requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_SPACE argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Bool(result));
                    }
                    "is_alnum" => {
                        if *arg_count != 1 { return Err("IS_ALNUM expects exactly 1 argument (char)".to_string()); }
                        let char_val = local_stack.pop().expect("Expected char for IS_ALNUM");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_alphanumeric()
                                } else {
                                    return Err("IS_ALNUM requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_ALNUM argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Bool(result));
                    }
                    "char_code" => {
                        if *arg_count != 1 { return Err("CHAR_CODE expects exactly 1 argument (char)".to_string()); }
                        let char_val = local_stack.pop().expect("Expected char for CHAR_CODE");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap() as u32 as i64
                                } else {
                                    return Err("CHAR_CODE requires a single character string".to_string());
                                }
                            }
                            _ => return Err("CHAR_CODE argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Int(result));
                    }
                    "char_from_code" => {
                        if *arg_count != 1 { return Err("CHAR_FROM_CODE expects exactly 1 argument (code)".to_string()); }
                        let code_val = local_stack.pop().expect("Expected code for CHAR_FROM_CODE");
                        
                        let result = match code_val {
                            Value::Int(code) => {
                                if code >= 0 && code <= 127 {
                                    let ch = code as u8 as char;
                                    ch.to_string()
                                } else {
                                    return Err("CHAR_FROM_CODE requires ASCII code 0-127".to_string());
                                }
                            }
                            _ => return Err("CHAR_FROM_CODE argument must be an integer".to_string())
                        };
                        
                        local_stack.push(Value::Str(result));
                    }
                    
                    // Number parsing functions
                    "parse_int" => {
                        if *arg_count != 1 { return Err("PARSE_INT expects exactly 1 argument (string)".to_string()); }
                        let string_val = local_stack.pop().expect("Expected string for PARSE_INT");
                        
                        let result = match string_val {
                            Value::Str(s) => {
                                match s.trim().parse::<i64>() {
                                    Ok(n) => n,
                                    Err(_) => return Err(format!("PARSE_INT: '{}' is not a valid integer", s))
                                }
                            }
                            _ => return Err("PARSE_INT argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Int(result));
                    }
                    "parse_float" => {
                        if *arg_count != 1 { return Err("PARSE_FLOAT expects exactly 1 argument (string)".to_string()); }
                        let string_val = local_stack.pop().expect("Expected string for PARSE_FLOAT");
                        
                        let result = match string_val {
                            Value::Str(s) => {
                                match s.trim().parse::<f64>() {
                                    Ok(f) => f,
                                    Err(_) => return Err(format!("PARSE_FLOAT: '{}' is not a valid float", s))
                                }
                            }
                            _ => return Err("PARSE_FLOAT argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Float(result));
                    }
                    
                    // String manipulation functions
                    "to_upper" => {
                        if *arg_count != 1 { return Err("TO_UPPER expects exactly 1 argument (string)".to_string()); }
                        let string_val = local_stack.pop().expect("Expected string for TO_UPPER");
                        
                        let result = match string_val {
                            Value::Str(s) => s.to_uppercase(),
                            _ => return Err("TO_UPPER argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Str(result));
                    }
                    "to_lower" => {
                        if *arg_count != 1 { return Err("TO_LOWER expects exactly 1 argument (string)".to_string()); }
                        let string_val = local_stack.pop().expect("Expected string for TO_LOWER");
                        
                        let result = match string_val {
                            Value::Str(s) => s.to_lowercase(),
                            _ => return Err("TO_LOWER argument must be a string".to_string())
                        };
                        
                        local_stack.push(Value::Str(result));
                    }
                    "substring" => {
                        if *arg_count != 3 { return Err("SUBSTRING expects exactly 3 arguments (string, start, end)".to_string()); }
                        let end_val = local_stack.pop().expect("Expected end for SUBSTRING");
                        let start_val = local_stack.pop().expect("Expected start for SUBSTRING");
                        let string_val = local_stack.pop().expect("Expected string for SUBSTRING");
                        
                        let result = match (string_val, start_val, end_val) {
                            (Value::Str(s), Value::Int(start), Value::Int(end)) => {
                                if start < 0 || end < 0 {
                                    return Err("SUBSTRING indices must be non-negative".to_string());
                                }
                                let start_idx = start as usize;
                                let end_idx = end as usize;
                                let chars: Vec<char> = s.chars().collect();
                                
                                if start_idx > chars.len() {
                                    return Err(format!("SUBSTRING start index {} out of bounds (string length {})", start_idx, chars.len()));
                                }
                                if end_idx > chars.len() {
                                    return Err(format!("SUBSTRING end index {} out of bounds (string length {})", end_idx, chars.len()));
                                }
                                if start_idx > end_idx {
                                    return Err("SUBSTRING start index must be <= end index".to_string());
                                }
                                
                                chars[start_idx..end_idx].iter().collect::<String>()
                            }
                            _ => return Err("SUBSTRING requires string, int, int arguments".to_string())
                        };
                        
                        local_stack.push(Value::Str(result));
                    }
                    "index_of" => {
                        if *arg_count != 2 { return Err("INDEX_OF expects exactly 2 arguments (string, substring)".to_string()); }
                        let substring_val = local_stack.pop().expect("Expected substring for INDEX_OF");
                        let string_val = local_stack.pop().expect("Expected string for INDEX_OF");
                        
                        let result = match (string_val, substring_val) {
                            (Value::Str(s), Value::Str(sub)) => {
                                match s.find(&sub) {
                                    Some(index) => index as i64,
                                    None => -1
                                }
                            }
                            _ => return Err("INDEX_OF requires two string arguments".to_string())
                        };
                        
                        local_stack.push(Value::Int(result));
                    }
                    "starts_with" => {
                        if *arg_count != 2 { return Err("STARTS_WITH expects exactly 2 arguments (string, prefix)".to_string()); }
                        let prefix_val = local_stack.pop().expect("Expected prefix for STARTS_WITH");
                        let string_val = local_stack.pop().expect("Expected string for STARTS_WITH");
                        
                        let result = match (string_val, prefix_val) {
                            (Value::Str(s), Value::Str(prefix)) => s.starts_with(&prefix),
                            _ => return Err("STARTS_WITH requires two string arguments".to_string())
                        };
                        
                        local_stack.push(Value::Bool(result));
                    }
                    "ends_with" => {
                        if *arg_count != 2 { return Err("ENDS_WITH expects exactly 2 arguments (string, suffix)".to_string()); }
                        let suffix_val = local_stack.pop().expect("Expected suffix for ENDS_WITH");
                        let string_val = local_stack.pop().expect("Expected string for ENDS_WITH");
                        
                        let result = match (string_val, suffix_val) {
                            (Value::Str(s), Value::Str(suffix)) => s.ends_with(&suffix),
                            _ => return Err("ENDS_WITH requires two string arguments".to_string())
                        };
                        
                        local_stack.push(Value::Bool(result));
                    }
                    
                    // TCP Network I/O Functions
                    "tcp_connect" => {
                        if *arg_count != 2 { return Err("TCP_CONNECT expects exactly 2 arguments (host, port)".to_string()); }
                        let port_val = local_stack.pop().expect("Expected port for TCP_CONNECT");
                        let host_val = local_stack.pop().expect("Expected host for TCP_CONNECT");
                        let host = match host_val { Value::Str(s) => s, _ => return Err("TCP_CONNECT host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TCP_CONNECT port must be a number".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.connect(&host, port) { Ok(id) => local_stack.push(Value::TcpConnection(id)), Err(e) => return Err(e) }
                    }
                    "tcp_listen" => {
                        if *arg_count != 1 { return Err("TCP_LISTEN expects exactly 1 argument (port)".to_string()); }
                        let port_val = local_stack.pop().expect("Expected port for TCP_LISTEN");
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TCP_LISTEN port must be a number".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.listen(port) { Ok(id) => local_stack.push(Value::TcpListener(id)), Err(e) => return Err(e) }
                    }
                    "tcp_send" => {
                        if *arg_count != 2 { return Err("TCP_SEND expects exactly 2 arguments (connection, data)".to_string()); }
                        let data_val = local_stack.pop().expect("Expected data for TCP_SEND");
                        let conn_val = local_stack.pop().expect("Expected connection for TCP_SEND");
                        let data = match data_val { Value::Str(s) => s, _ => return Err("TCP_SEND data must be a string".to_string()) };
                        let id = match conn_val { Value::TcpConnection(id) => id, _ => return Err("TCP_SEND connection must be a TCP connection".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.send(id, &data) { Ok(_) => local_stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "tcp_receive" => {
                        if *arg_count != 2 { return Err("TCP_RECEIVE expects exactly 2 arguments (connection, max_bytes)".to_string()); }
                        let max_bytes_val = local_stack.pop().expect("Expected max_bytes for TCP_RECEIVE");
                        let conn_val = local_stack.pop().expect("Expected connection for TCP_RECEIVE");
                        let max_bytes: usize = match max_bytes_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("TCP_RECEIVE max_bytes must be a number".to_string()) };
                        let id = match conn_val { Value::TcpConnection(id) => id, _ => return Err("TCP_RECEIVE connection must be a TCP connection".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.receive(id, max_bytes) { Ok(s) => local_stack.push(Value::Str(s)), Err(e) => return Err(e) }
                    }
                    "tcp_close" => {
                        if *arg_count != 1 { return Err("TCP_CLOSE expects exactly 1 argument (connection or listener)".to_string()); }
                        let val = local_stack.pop().expect("Expected argument for TCP_CLOSE");
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match val {
                            Value::TcpConnection(id) => match mgr.close_connection(id) { Ok(_) => local_stack.push(Value::Bool(true)), Err(e) => return Err(e) },
                            Value::TcpListener(id) => match mgr.close_listener(id) { Ok(_) => local_stack.push(Value::Bool(true)), Err(e) => return Err(e) },
                            _ => return Err("TCP_CLOSE expects TCP connection or listener".to_string()),
                        }
                    }
                    "tcp_accept" => {
                        if *arg_count != 1 { return Err("TCP_ACCEPT expects exactly 1 argument (listener)".to_string()); }
                        let val = local_stack.pop().expect("Expected listener for TCP_ACCEPT");
                        let id = match val { Value::TcpListener(id) => id, _ => return Err("TCP_ACCEPT listener must be a TCP listener".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.accept(id) { Ok(conn_id) => local_stack.push(Value::TcpConnection(conn_id)), Err(e) => return Err(e) }
                    }
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
                    // Math built-ins (inside functions)
                    "abs" => { if *arg_count != 1 { return Err("ABS expects 1 argument".to_string()); } let x = local_stack.pop().unwrap(); let v = match x { Value::Int(n) => (n as f64).abs(), Value::Float(f)=> f.abs(), _=> return Err("ABS: type".to_string())}; local_stack.push(Value::Float(v)); }
                    "sign" => { if *arg_count != 1 { return Err("SIGN expects 1 argument".to_string()); } let x = local_stack.pop().unwrap(); let f = match x { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("SIGN: type".to_string())}; let s = if f>0.0 {1.0} else if f<0.0 {-1.0} else {0.0}; local_stack.push(Value::Float(s)); }
                    "floor" => { if *arg_count != 1 { return Err("FLOOR expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("FLOOR: type".to_string())}; local_stack.push(Value::Float(f.floor())); }
                    "ceil" => { if *arg_count != 1 { return Err("CEIL expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("CEIL: type".to_string())}; local_stack.push(Value::Float(f.ceil())); }
                    "round" => { if *arg_count != 1 { return Err("ROUND expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ROUND: type".to_string())}; local_stack.push(Value::Float(f.round())); }
                    "trunc" => { if *arg_count != 1 { return Err("TRUNC expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("TRUNC: type".to_string())}; local_stack.push(Value::Float(f.trunc())); }
                    "sqrt" => { if *arg_count != 1 { return Err("SQRT expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("SQRT: type".to_string())}; if f<0.0 { return Err("SQRT: domain error".to_string()); } local_stack.push(Value::Float(f.sqrt())); }
                    "cbrt" => { if *arg_count != 1 { return Err("CBRT expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("CBRT: type".to_string())}; local_stack.push(Value::Float(f.cbrt())); }
                    "pow" => { if *arg_count != 2 { return Err("POW expects 2 arguments".to_string()); } let y = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("POW: type".to_string())}; let x = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("POW: type".to_string())}; local_stack.push(Value::Float(x.powf(y))); }
                    "sin" => { if *arg_count != 1 { return Err("SIN expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("SIN: type".to_string())}; local_stack.push(Value::Float(f.sin())); }
                    "cos" => { if *arg_count != 1 { return Err("COS expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("COS: type".to_string())}; local_stack.push(Value::Float(f.cos())); }
                    "tan" => { if *arg_count != 1 { return Err("TAN expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("TAN: type".to_string())}; local_stack.push(Value::Float(f.tan())); }
                    "asin" => { if *arg_count != 1 { return Err("ASIN expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ASIN: type".to_string())}; if f < -1.0 || f > 1.0 { return Err("ASIN: domain error".to_string()); } local_stack.push(Value::Float(f.asin())); }
                    "acos" => { if *arg_count != 1 { return Err("ACOS expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ACOS: type".to_string())}; if f < -1.0 || f > 1.0 { return Err("ACOS: domain error".to_string()); } local_stack.push(Value::Float(f.acos())); }
                    "atan" => { if *arg_count != 1 { return Err("ATAN expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ATAN: type".to_string())}; local_stack.push(Value::Float(f.atan())); }
                    "atan2" => { if *arg_count != 2 { return Err("ATAN2 expects 2 arguments".to_string()); } let x = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ATAN2: type".to_string())}; let y = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ATAN2: type".to_string())}; local_stack.push(Value::Float(y.atan2(x))); }
                    "exp" => { if *arg_count != 1 { return Err("EXP expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("EXP: type".to_string())}; local_stack.push(Value::Float(f.exp())); }
                    "ln" => { if *arg_count != 1 { return Err("LN expects 1 argument".to_string()); } let f = match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("LN: type".to_string())}; if f<=0.0 { return Err("LN: domain error".to_string()); } local_stack.push(Value::Float(f.ln())); }
                    "log10" => { if *arg_count != 1 { return Err("LOG10 expects 1 argument".to_string()); } let f=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("LOG10: type".to_string())}; if f<=0.0 { return Err("LOG10: domain error".to_string()); } local_stack.push(Value::Float(f.log10())); }
                    "log2" => { if *arg_count != 1 { return Err("LOG2 expects 1 argument".to_string()); } let f=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("LOG2: type".to_string())}; if f<=0.0 { return Err("LOG2: domain error".to_string()); } local_stack.push(Value::Float(f.log2())); }
                    "min" => { if *arg_count != 2 { return Err("MIN expects 2 arguments".to_string()); } let b=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MIN: type".to_string())}; let a=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MIN: type".to_string())}; local_stack.push(Value::Float(a.min(b))); }
                    "max" => { if *arg_count != 2 { return Err("MAX expects 2 arguments".to_string()); } let b=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MAX: type".to_string())}; let a=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MAX: type".to_string())}; local_stack.push(Value::Float(a.max(b))); }
                    "clamp" => { if *arg_count != 3 { return Err("CLAMP expects 3 arguments".to_string()); } let hi=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("CLAMP: type".to_string())}; let lo=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("CLAMP: type".to_string())}; let x=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("CLAMP: type".to_string())}; if lo>hi { return Err("CLAMP: domain error".to_string()); } local_stack.push(Value::Float(x.max(lo).min(hi))); }
                    "hypot" => { if *arg_count != 2 { return Err("HYPOT expects 2 arguments".to_string()); } let b=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("HYPOT: type".to_string())}; let a=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("HYPOT: type".to_string())}; local_stack.push(Value::Float(a.hypot(b))); }
                    "deg" => { if *arg_count != 1 { return Err("DEG expects 1 argument".to_string()); } let f=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("DEG: type".to_string())}; local_stack.push(Value::Float(f*180.0/std::f64::consts::PI)); }
                    "rad" => { if *arg_count != 1 { return Err("RAD expects 1 argument".to_string()); } let f=match local_stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("RAD: type".to_string())}; local_stack.push(Value::Float(f*std::f64::consts::PI/180.0)); }
                    // Extra helpers (inside functions)
                    "mod" => { if *arg_count != 2 { return Err("MOD expects 2 arguments".to_string()); } let b=local_stack.pop().unwrap(); let a=local_stack.pop().unwrap(); match (a,b){(Value::Int(x),Value::Int(y))=>{ if y==0 { return Err("MOD: division by zero".to_string()); } local_stack.push(Value::Int(x.rem_euclid(y))); }, (Value::Float(x),Value::Float(y))=>{ local_stack.push(Value::Float(x.rem_euclid(y))); }, (Value::Int(x),Value::Float(y))=>{ local_stack.push(Value::Float((x as f64).rem_euclid(y))); }, (Value::Float(x),Value::Int(y))=>{ local_stack.push(Value::Float(x.rem_euclid(y as f64))); }, _=> return Err("MOD: type".to_string()) } }
                    "div" => { if *arg_count != 2 { return Err("DIV expects 2 arguments".to_string()); } let b=local_stack.pop().unwrap(); let a=local_stack.pop().unwrap(); match (a,b){(Value::Int(x),Value::Int(y))=>{ if y==0 { return Err("DIV: division by zero".to_string()); } local_stack.push(Value::Int(x.div_euclid(y))); }, _=> return Err("DIV: int type".to_string()) } }
                    "divmod" => { if *arg_count != 2 { return Err("DIVMOD expects 2 arguments".to_string()); } let b=local_stack.pop().unwrap(); let a=local_stack.pop().unwrap(); match (a,b){(Value::Int(x),Value::Int(y))=>{ if y==0 { return Err("DIVMOD: division by zero".to_string()); } let q = x.div_euclid(y); let r = x.rem_euclid(y); local_stack.push(Value::Box(vec![Value::Int(q), Value::Int(r)])); }, _=> return Err("DIVMOD: int type".to_string()) } }
                    "pow_int" => { if *arg_count != 2 { return Err("POW_INT expects 2 arguments".to_string()); } let n_v=local_stack.pop().unwrap(); let x_v=local_stack.pop().unwrap(); let n = match n_v { Value::Int(k)=>k, _=> return Err("POW_INT: exponent must be int".to_string())}; match x_v { Value::Int(x)=>{ if n<0 { local_stack.push(Value::Float((x as f64).powi(n as i32))); } else { let mut res:i64=1; let mut base=x; let mut exp=n; while exp>0 { if (exp & 1)==1 { res = res.saturating_mul(base); } base = base.saturating_mul(base); exp >>= 1; } local_stack.push(Value::Int(res)); } }, Value::Float(x)=>{ local_stack.push(Value::Float(x.powi(n as i32))); }, _=> return Err("POW_INT: base type".to_string()) }
                    }
                    "frac" => { if *arg_count != 1 { return Err("FRAC expects 1 argument".to_string()); } let v=local_stack.pop().unwrap(); match v { Value::Int(_)=> local_stack.push(Value::Float(0.0)), Value::Float(f)=> local_stack.push(Value::Float(f.fract())), _=> return Err("FRAC: type".to_string()) } }
                    "nearly_equal" => { if *arg_count != 3 { return Err("NEARLY_EQUAL expects 3 arguments".to_string()); } let eps=match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("NEARLY_EQUAL: type".to_string())}; let b=match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("NEARLY_EQUAL: type".to_string())}; let a=match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("NEARLY_EQUAL: type".to_string())}; local_stack.push(Value::Bool((a-b).abs() <= eps)); }
                    "sum" => { if *arg_count != 1 { return Err("SUM expects 1 argument".to_string()); } let xs = local_stack.pop().unwrap(); match xs { Value::Box(vs)=>{ let mut acc=0.0; for v in vs { match v { Value::Int(n)=> acc += n as f64, Value::Float(f)=> acc += f, _=> return Err("SUM: element type".to_string()) } } local_stack.push(Value::Float(acc)); }, _=> return Err("SUM: expects box".to_string()) } }
                    "mean" => { 
                        if *arg_count != 1 { return Err("MEAN expects 1 argument".to_string()); } 
                        let xs = local_stack.pop().unwrap(); 
                        match xs { 
                            Value::Box(vs)=>{ 
                                if vs.is_empty() { return Err("MEAN: empty".to_string()); } 
                                let len = vs.len();
                                let mut acc=0.0; 
                                for v in vs { 
                                    match v { 
                                        Value::Int(n)=> acc += n as f64, 
                                        Value::Float(f)=> acc += f, 
                                        _=> return Err("MEAN: element type".to_string()) 
                                    } 
                                } 
                                local_stack.push(Value::Float(acc / len as f64)); 
                            }, 
                            _=> return Err("MEAN: expects box".to_string()) 
                        } 
                    }
                    "dot" => { if *arg_count != 2 { return Err("DOT expects 2 arguments".to_string()); } let b=local_stack.pop().unwrap(); let a=local_stack.pop().unwrap(); match (a,b){(Value::Box(as_), Value::Box(bs))=>{ if as_.len()!=bs.len(){ return Err("DOT: length mismatch".to_string()); } let mut acc=0.0; for (va,vb) in as_.into_iter().zip(bs.into_iter()) { let fa = match va { Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("DOT: element type".to_string())}; let fb = match vb { Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("DOT: element type".to_string())}; acc += fa*fb; } local_stack.push(Value::Float(acc)); }, _=> return Err("DOT: expects boxes".to_string()) } }
                    "length" => { if *arg_count != 1 { return Err("LENGTH expects 1 argument".to_string()); } let a=local_stack.pop().unwrap(); match a { Value::Box(vs)=>{ let mut acc=0.0; for v in vs { let f=match v{Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("LENGTH: element type".to_string())}; acc += f*f; } local_stack.push(Value::Float(acc.sqrt())); }, _=> return Err("LENGTH: expects box".to_string()) } }
                    "sin_deg" => { if *arg_count != 1 { return Err("SIN_DEG expects 1 argument".to_string()); } let d=match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("SIN_DEG: type".to_string())}; local_stack.push(Value::Float((d*std::f64::consts::PI/180.0).sin())); }
                    "cos_deg" => { if *arg_count != 1 { return Err("COS_DEG expects 1 argument".to_string()); } let d=match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("COS_DEG: type".to_string())}; local_stack.push(Value::Float((d*std::f64::consts::PI/180.0).cos())); }
                    "tan_deg" => { if *arg_count != 1 { return Err("TAN_DEG expects 1 argument".to_string()); } let d=match local_stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("TAN_DEG: type".to_string())}; local_stack.push(Value::Float((d*std::f64::consts::PI/180.0).tan())); }
                    "linspace" => { if *arg_count != 3 { return Err("LINSPACE expects 3 arguments".to_string()); } let n_v = local_stack.pop().unwrap(); let end_v = local_stack.pop().unwrap(); let start_v = local_stack.pop().unwrap(); let n = match n_v { Value::Int(k)=>k, _=> return Err("LINSPACE: n must be int".to_string())}; if n < 2 { return Err("LINSPACE: n >= 2".to_string()); } let start = match start_v { Value::Int(x)=> x as f64, Value::Float(f)=>f, _=> return Err("LINSPACE: type".to_string())}; let end = match end_v { Value::Int(x)=> x as f64, Value::Float(f)=>f, _=> return Err("LINSPACE: type".to_string())}; let step = (end-start) / (n-1) as f64; let mut out = Vec::with_capacity(n as usize); let mut i=0; while i < n { out.push(Value::Float(start + step * i as f64)); i += 1; } local_stack.push(Value::Box(out)); }
                    "range" => { 
                        if *arg_count != 3 { return Err("RANGE expects 3 arguments".to_string()); } 
                        let step_v=local_stack.pop().unwrap(); 
                        let end_v=local_stack.pop().unwrap(); 
                        let start_v=local_stack.pop().unwrap(); 
                        let mut max_iter: i64 = 1_000_000; 
                        match (start_v, end_v, step_v) { 
                            (Value::Int(mut a), Value::Int(b), Value::Int(s))=>{ 
                                if s==0 { return Err("RANGE: step=0".to_string()); } 
                                let mut out=Vec::new(); 
                                if s>0 { 
                                    while a < b && max_iter>0 { 
                                        out.push(Value::Int(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } else { 
                                    while a > b && max_iter>0 { 
                                        out.push(Value::Int(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } 
                                local_stack.push(Value::Box(out)); 
                            }, 
                            (a_v, b_v, s_v)=>{ 
                                let mut a = match a_v { 
                                    Value::Int(x)=> x as f64, 
                                    Value::Float(f)=>f, 
                                    _=> return Err("RANGE: type".to_string())
                                }; 
                                let b = match b_v { 
                                    Value::Int(x)=> x as f64, 
                                    Value::Float(f)=>f, 
                                    _=> return Err("RANGE: type".to_string())
                                }; 
                                let s = match s_v { 
                                    Value::Int(x)=> x as f64, 
                                    Value::Float(f)=>f, 
                                    _=> return Err("RANGE: type".to_string())
                                }; 
                                if s==0.0 { return Err("RANGE: step=0".to_string()); } 
                                let mut out=Vec::new(); 
                                if s>0.0 { 
                                    while a < b && max_iter>0 { 
                                        out.push(Value::Float(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } else { 
                                    while a > b && max_iter>0 { 
                                        out.push(Value::Float(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } 
                                local_stack.push(Value::Box(out)); 
                            } 
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
                    "rename" => {
                        if *arg_count != 2 { return Err("RENAME expects exactly 2 arguments".to_string()); }
                        let to_val = local_stack.pop().expect("Expected destination name for RENAME");
                        let from_val = local_stack.pop().expect("Expected source for RENAME");
                        let from_name = match from_val { Value::Str(s) => s, Value::Function { name, .. } => name, _ => return Err("RENAME: first argument must be a string or function".to_string()) };
                        let to_name = match to_val { Value::Str(s) => s, _ => return Err("RENAME: destination must be a string".to_string()) };
                        let old_key = format!("__module_{}", from_name);
                        let mut module_val = match local_env.remove(&old_key) { Some(v) => v, None => return Err(format!("RENAME: module '{}' not found", from_name)) };
                        if let Value::Box(ref mut items) = module_val { if !items.is_empty() { items[0] = Value::Str(to_name.clone()); } }
                        let new_key = format!("__module_{}", to_name);
                        local_env.insert(new_key, module_val);
                        local_stack.push(Value::Str("OK".to_string()));
                    }
                    "count" => {
                        if *arg_count != 1 { return Err("COUNT expects exactly 1 argument".to_string()); }
                        let arg = local_stack.pop().expect("Expected argument for COUNT");
                        match arg { Value::Box(contents) => local_stack.push(Value::Int(contents.len() as i64)), Value::Str(s) => local_stack.push(Value::Int(s.chars().count() as i64)), Value::Function { name: _n, params, body: _b, .. } => local_stack.push(Value::Int(params.len() as i64)), _ => return Err("COUNT expects box, string, or function".to_string()) }
                    }
                    _ => {
                        match func {
                            Value::Function { name, params, body, .. } => {
                                if params.len() != *arg_count { return Err(format!("Function '{}' expects {} arguments, got {}", name, params.len(), arg_count)); }
                                // Pop args and bind to params in a cloned env
                                let mut args = Vec::with_capacity(*arg_count);
                                for _ in 0..*arg_count { args.push(local_stack.pop().expect("Missing argument")); }
                                args.reverse();
                                let mut env_copy = local_env.clone();
                                for (i, p) in params.iter().enumerate() { env_copy.insert(p.clone(), args[i].clone()); }
                                let body_clone = body.clone();
                                let result = execute_function(&body_clone, &mut local_stack, &mut env_copy)?;
                                local_stack.push(result);
                            }
                            _ => return Err(format!("'{}' is not a function", func_name)),
                        }
                    }
                }
            }
            
            OpCode::StoreVar(name) => {
                let val = local_stack.pop().expect("Nothing to store");
                // Ensure deep cloning of boxes to maintain immutability
                let cloned_val = match val {
                    Value::Box(contents) => {
                        // Deep clone the box contents to prevent shared mutable references
                        Value::Box(contents.clone())
                    }
                    other => other
                };
                local_env.insert(name.clone(), cloned_val);
            }
            
            OpCode::Print => {
                if let Some(val) = local_stack.pop() {
                    let out_val = match val {
                        Value::Str(s) => {
                            // If rewired symbol, print its bound value
                            if is_rewired(&local_env, &s) {
                                local_env.get(&s).cloned().unwrap_or(Value::Str(s))
                            } else {
                                // Print strings literally - do not evaluate as code
                                Value::Str(s)
                            }
                        }
                        other => other,
                    };
                    println!("{}", out_val.format_for_display());
                }
            }
            
            // Control flow operations
            OpCode::Jump(offset) => {
                ip = *offset;
                continue; // Skip the normal increment
            }
            
            OpCode::JumpIfNot(offset) => {
                let condition = local_stack.pop().expect("Expected condition for JumpIfNot");
                match condition {
                    Value::Bool(false) => {
                        ip = *offset;
                        continue; // Skip the normal increment
                    }
                    Value::Bool(true) => {
                        // Continue to next instruction
                    }
                    _ => return Err("JumpIfNot condition must be boolean".to_string()),
                }
            }
            
            OpCode::Label(_) => {
                // Labels are no-ops during execution
            }
            
            OpCode::MakeBox => {
                local_stack.push(Value::Box(vec![]));
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
    let mut tcp_manager = TcpSocketManager::new();
    
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
    env.insert("http_get".to_string(), Value::Function {
        name: "http_get".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("http_post".to_string(), Value::Function {
        name: "http_post".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("http_put".to_string(), Value::Function {
        name: "http_put".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("http_delete".to_string(), Value::Function {
        name: "http_delete".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("dns_resolve".to_string(), Value::Function {
        name: "dns_resolve".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("json_encode".to_string(), Value::Function {
        name: "json_encode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("json_decode".to_string(), Value::Function {
        name: "json_decode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("base64_encode".to_string(), Value::Function {
        name: "base64_encode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("base64_decode".to_string(), Value::Function {
        name: "base64_decode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("url_encode".to_string(), Value::Function {
        name: "url_encode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("url_decode".to_string(), Value::Function {
        name: "url_decode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("source".to_string(), Value::Function {
        name: "source".to_string(),
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
    env.insert("rename".to_string(), Value::Function {
        name: "rename".to_string(),
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
    // Math built-ins (stubs; dispatch handled in Call)
    for name in [
        "abs","sign","floor","ceil","round","trunc",
        "sqrt","cbrt","pow",
        "sin","cos","tan","asin","acos","atan","atan2",
        "exp","ln","log10","log2",
        "min","max","clamp","hypot","deg","rad",
        "mod","div","divmod","pow_int","frac","nearly_equal","sum","mean","dot","length","sin_deg","cos_deg","tan_deg","linspace","range"
    ] {
        env.insert(name.to_string(), Value::Function {
            name: name.to_string(),
            params: vec![],
            body: vec![],
            rewire_target: None,
        });
    }
    
    // Add ship function
    env.insert("ship".to_string(), Value::Function {
        name: "ship".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    // Add import function
    env.insert("import".to_string(), Value::Function {
        name: "import".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Core-8 Hardware Functions
    env.insert("time_counter".to_string(), Value::Function {
        name: "time_counter".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("time_freq".to_string(), Value::Function {
        name: "time_freq".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("cpu_halt".to_string(), Value::Function {
        name: "cpu_halt".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("mem_fence".to_string(), Value::Function {
        name: "mem_fence".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("mem_load".to_string(), Value::Function {
        name: "mem_load".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("mem_store".to_string(), Value::Function {
        name: "mem_store".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("mem_cmpxchg".to_string(), Value::Function {
        name: "mem_cmpxchg".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("int_disable".to_string(), Value::Function {
        name: "int_disable".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("int_enable".to_string(), Value::Function {
        name: "int_enable".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Character classification functions
    env.insert("is_digit".to_string(), Value::Function {
        name: "is_digit".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("is_alpha".to_string(), Value::Function {
        name: "is_alpha".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("is_space".to_string(), Value::Function {
        name: "is_space".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("is_alnum".to_string(), Value::Function {
        name: "is_alnum".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("is_operator".to_string(), Value::Function {
        name: "is_operator".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("is_punctuation".to_string(), Value::Function {
        name: "is_punctuation".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("is_symbol".to_string(), Value::Function {
        name: "is_symbol".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_char_category".to_string(), Value::Function {
        name: "get_char_category".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("char_code".to_string(), Value::Function {
        name: "char_code".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("char_from_code".to_string(), Value::Function {
        name: "char_from_code".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Number parsing functions
    env.insert("parse_int".to_string(), Value::Function {
        name: "parse_int".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("parse_float".to_string(), Value::Function {
        name: "parse_float".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // String manipulation functions
    env.insert("to_upper".to_string(), Value::Function {
        name: "to_upper".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("to_lower".to_string(), Value::Function {
        name: "to_lower".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("substring".to_string(), Value::Function {
        name: "substring".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("index_of".to_string(), Value::Function {
        name: "index_of".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("starts_with".to_string(), Value::Function {
        name: "starts_with".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ends_with".to_string(), Value::Function {
        name: "ends_with".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // TCP Network I/O Functions
    env.insert("tcp_connect".to_string(), Value::Function {
        name: "tcp_connect".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tcp_listen".to_string(), Value::Function {
        name: "tcp_listen".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tcp_send".to_string(), Value::Function {
        name: "tcp_send".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tcp_receive".to_string(), Value::Function {
        name: "tcp_receive".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tcp_close".to_string(), Value::Function {
        name: "tcp_close".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tcp_accept".to_string(), Value::Function {
        name: "tcp_accept".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // UDP Network I/O Functions
    env.insert("udp_bind".to_string(), Value::Function {
        name: "udp_bind".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_send".to_string(), Value::Function {
        name: "udp_send".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_receive".to_string(), Value::Function {
        name: "udp_receive".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_try_receive".to_string(), Value::Function {
        name: "udp_try_receive".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_close".to_string(), Value::Function {
        name: "udp_close".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // UDP Multicast and Broadcast Functions
    env.insert("udp_join_multicast".to_string(), Value::Function {
        name: "udp_join_multicast".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_leave_multicast".to_string(), Value::Function {
        name: "udp_leave_multicast".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_set_multicast_ttl".to_string(), Value::Function {
        name: "udp_set_multicast_ttl".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_set_multicast_loopback".to_string(), Value::Function {
        name: "udp_set_multicast_loopback".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_set_broadcast".to_string(), Value::Function {
        name: "udp_set_broadcast".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_send_broadcast".to_string(), Value::Function {
        name: "udp_send_broadcast".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_send_multicast".to_string(), Value::Function {
        name: "udp_send_multicast".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_is_multicast".to_string(), Value::Function {
        name: "udp_is_multicast".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("udp_is_broadcast".to_string(), Value::Function {
        name: "udp_is_broadcast".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // TLS Network I/O Functions
    env.insert("tls_connect".to_string(), Value::Function {
        name: "tls_connect".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tls_listen".to_string(), Value::Function {
        name: "tls_listen".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tls_accept".to_string(), Value::Function {
        name: "tls_accept".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tls_send".to_string(), Value::Function {
        name: "tls_send".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tls_receive".to_string(), Value::Function {
        name: "tls_receive".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tls_try_receive".to_string(), Value::Function {
        name: "tls_try_receive".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("tls_close".to_string(), Value::Function {
        name: "tls_close".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Event Loop / Async Network I/O Functions
    env.insert("event_register".to_string(), Value::Function {
        name: "event_register".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("event_unregister".to_string(), Value::Function {
        name: "event_unregister".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("event_poll".to_string(), Value::Function {
        name: "event_poll".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("event_wait".to_string(), Value::Function {
        name: "event_wait".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("event_wait_any".to_string(), Value::Function {
        name: "event_wait_any".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Connection Pool Functions
    env.insert("pool_get_connection".to_string(), Value::Function {
        name: "pool_get_connection".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("pool_return_connection".to_string(), Value::Function {
        name: "pool_return_connection".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("pool_stats".to_string(), Value::Function {
        name: "pool_stats".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("pool_clear".to_string(), Value::Function {
        name: "pool_clear".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("pool_configure".to_string(), Value::Function {
        name: "pool_configure".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Timeout Management Functions
    env.insert("timeout_set_global".to_string(), Value::Function {
        name: "timeout_set_global".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("timeout_set_specific".to_string(), Value::Function {
        name: "timeout_set_specific".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("timeout_get_info".to_string(), Value::Function {
        name: "timeout_get_info".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("timeout_remove".to_string(), Value::Function {
        name: "timeout_remove".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("timeout_clear".to_string(), Value::Function {
        name: "timeout_clear".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("timeout_summary".to_string(), Value::Function {
        name: "timeout_summary".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Proxy Management Functions
    env.insert("proxy_set_global".to_string(), Value::Function {
        name: "proxy_set_global".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_set_specific".to_string(), Value::Function {
        name: "proxy_set_specific".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_set_default".to_string(), Value::Function {
        name: "proxy_set_default".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_get_info".to_string(), Value::Function {
        name: "proxy_get_info".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_remove_global".to_string(), Value::Function {
        name: "proxy_remove_global".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_remove_specific".to_string(), Value::Function {
        name: "proxy_remove_specific".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_clear_all".to_string(), Value::Function {
        name: "proxy_clear_all".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_add_bypass".to_string(), Value::Function {
        name: "proxy_add_bypass".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_remove_bypass".to_string(), Value::Function {
        name: "proxy_remove_bypass".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_get_bypass_list".to_string(), Value::Function {
        name: "proxy_get_bypass_list".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("proxy_stats".to_string(), Value::Function {
        name: "proxy_stats".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Raw Socket Functions
    env.insert("raw_socket_create".to_string(), Value::Function {
        name: "raw_socket_create".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("raw_socket_set_header_included".to_string(), Value::Function {
        name: "raw_socket_set_header_included".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("raw_socket_send".to_string(), Value::Function {
        name: "raw_socket_send".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("raw_socket_receive".to_string(), Value::Function {
        name: "raw_socket_receive".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("raw_socket_close".to_string(), Value::Function {
        name: "raw_socket_close".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("raw_socket_info".to_string(), Value::Function {
        name: "raw_socket_info".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("packet_build_icmp_echo".to_string(), Value::Function {
        name: "packet_build_icmp_echo".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("packet_build_ipv4_header".to_string(), Value::Function {
        name: "packet_build_ipv4_header".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("packet_calculate_checksum".to_string(), Value::Function {
        name: "packet_calculate_checksum".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // Network Interface Functions
    env.insert("get_interfaces".to_string(), Value::Function {
        name: "get_interfaces".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_interface_info".to_string(), Value::Function {
        name: "get_interface_info".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_interface_stats".to_string(), Value::Function {
        name: "get_interface_stats".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_primary_interface".to_string(), Value::Function {
        name: "get_primary_interface".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_loopback_interface".to_string(), Value::Function {
        name: "get_loopback_interface".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_interfaces_by_type".to_string(), Value::Function {
        name: "get_interfaces_by_type".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_up_interfaces".to_string(), Value::Function {
        name: "get_up_interfaces".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_interface_by_ip".to_string(), Value::Function {
        name: "get_interface_by_ip".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("get_best_interface".to_string(), Value::Function {
        name: "get_best_interface".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // IPv6 and Dual-Stack Functions
    env.insert("ipv6_set_dual_stack_mode".to_string(), Value::Function {
        name: "ipv6_set_dual_stack_mode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_get_dual_stack_mode".to_string(), Value::Function {
        name: "ipv6_get_dual_stack_mode".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_resolve_dual_stack".to_string(), Value::Function {
        name: "ipv6_resolve_dual_stack".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_parse_address".to_string(), Value::Function {
        name: "ipv6_parse_address".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_get_multicast_address".to_string(), Value::Function {
        name: "ipv6_get_multicast_address".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_is_ipv6_address".to_string(), Value::Function {
        name: "ipv6_is_ipv6_address".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_is_ipv4_address".to_string(), Value::Function {
        name: "ipv6_is_ipv4_address".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_get_address_info".to_string(), Value::Function {
        name: "ipv6_get_address_info".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_get_config".to_string(), Value::Function {
        name: "ipv6_get_config".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    env.insert("ipv6_create_dual_stack_socket".to_string(), Value::Function {
        name: "ipv6_create_dual_stack_socket".to_string(),
        params: vec![],
        body: vec![],
        rewire_target: None,
    });
    
    // WebSocket built-ins
    env.insert("ws_connect".to_string(), Value::Function { name: "ws_connect".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("ws_send".to_string(), Value::Function { name: "ws_send".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("ws_receive".to_string(), Value::Function { name: "ws_receive".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("ws_close".to_string(), Value::Function { name: "ws_close".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("ws_try_receive".to_string(), Value::Function { name: "ws_try_receive".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("ws_send_binary".to_string(), Value::Function { name: "ws_send_binary".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("ws_receive_bytes".to_string(), Value::Function { name: "ws_receive_bytes".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("ws_try_receive_bytes".to_string(), Value::Function { name: "ws_try_receive_bytes".to_string(), params: vec![], body: vec![], rewire_target: None });
    
    env.insert("tcp_try_receive".to_string(), Value::Function { name: "tcp_try_receive".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("dns_lookup".to_string(), Value::Function { name: "dns_lookup".to_string(), params: vec![], body: vec![], rewire_target: None });
    env.insert("net_wait_any".to_string(), Value::Function { name: "net_wait_any".to_string(), params: vec![], body: vec![], rewire_target: None });
    
    // Math constants
    env.insert("PI".to_string(), Value::Float(std::f64::consts::PI));
    env.insert("TAU".to_string(), Value::Float(std::f64::consts::TAU));
    env.insert("E".to_string(), Value::Float(std::f64::consts::E));
    
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
                                // Print strings literally - do not evaluate as code
                                Value::Str(s)
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
                    // Ensure deep cloning of boxes to maintain immutability
                    let cloned_val = match val {
                        Value::Box(contents) => {
                            // Deep clone the box contents to prevent shared mutable references
                            if std::env::var("SCRAPS_DEBUG").is_ok() {
                                println!("DEBUG LOAD_VAR: Loading box '{}' with {} contents: {:?}", name, contents.len(), contents.iter().map(|v| v.format_for_display()).collect::<Vec<_>>());
                            }
                            Value::Box(contents.clone())
                        }
                        other => other.clone()
                    };
                    stack.push(cloned_val);
                } else {
                    return Err(format!("Undefined variable '{}'", name));
                }
            }
            OpCode::StoreVar(name) => {
                let val = stack.pop().expect("Nothing to store");
                // Ensure deep cloning of boxes to maintain immutability
                let cloned_val = match val {
                    Value::Box(contents) => {
                        // Deep clone the box contents to prevent shared mutable references
                        if std::env::var("SCRAPS_DEBUG").is_ok() {
                            println!("DEBUG STORE_VAR: Storing box '{}' with {} contents: {:?}", name, contents.len(), contents.iter().map(|v| v.format_for_display()).collect::<Vec<_>>());
                        }
                        Value::Box(contents.clone())
                    }
                    other => other
                };
                env.insert(name.clone(), cloned_val);
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
                let func = env.get(&func_name).cloned().expect("Function not found");
                
                // Handle built-in functions
                match func_name.as_str() {
                    "box" => {
                        if *arg_count != 0 {
                            return Err("BOX expects 0 arguments".to_string());
                        }
                        stack.push(Value::Box(vec![]));
                    }
                    "source" => {
                        if *arg_count != 1 { return Err("SOURCE expects exactly 1 argument".to_string()); }
                        let filename = stack.pop().expect("Expected filename for SOURCE");
                        let path = match filename { Value::Str(s) => s, _ => return Err("SOURCE filename must be a string".to_string()) };
                        let content = match std::fs::read_to_string(&path) { Ok(s) => s, Err(e) => return Err(format!("SOURCE error: {}", e)) };
                        let result = eval_snippet(&mut env, &content)?;
                        stack.push(result);
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
                    "http_get" => {
                        if *arg_count < 1 || *arg_count > 2 { return Err("HTTP_GET expects 1 or 2 arguments (url[, headers])".to_string()); }
                        let url_val = stack.pop().expect("Expected URL for HTTP_GET");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_GET url must be a string".to_string()) };
                        let headers = if *arg_count == 2 {
                            let headers_val = stack.pop().expect("Expected headers for HTTP_GET");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let (status, hs, body) = http_get_impl(&url, headers.as_deref())?;
                        let resp = Value::Box(vec![
                            Value::Int(status),
                            headers_to_value(&hs),
                            Value::Str(body),
                        ]);
                        stack.push(resp);
                    }
                    // WebSocket built-ins (top-level)
                    "ws_connect" => {
                        if *arg_count != 1 { return Err("WS_CONNECT expects 1 argument (url)".to_string()); }
                        let url_val = stack.pop().expect("Expected URL for WS_CONNECT");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("WS_CONNECT url must be a string".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.connect(&url) { Ok(id) => stack.push(Value::WebSocket(id)), Err(e) => return Err(e) }
                    }
                    "ws_send" => {
                        if *arg_count != 2 { return Err("WS_SEND expects 2 arguments (ws, text)".to_string()); }
                        let text_val = stack.pop().expect("Expected text for WS_SEND");
                        let ws_val = stack.pop().expect("Expected ws handle for WS_SEND");
                        let text = match text_val { Value::Str(s) => s, _ => return Err("WS_SEND text must be a string".to_string()) };
                        let id = match ws_val { Value::WebSocket(id) => id, _ => return Err("WS_SEND expects WebSocket handle".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.send_text(id, &text) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "ws_receive" => {
                        if *arg_count != 1 { return Err("WS_RECEIVE expects 1 argument (ws)".to_string()); }
                        let ws_val = stack.pop().expect("Expected ws handle for WS_RECEIVE");
                        let id = match ws_val { Value::WebSocket(id) => id, _ => return Err("WS_RECEIVE expects WebSocket handle".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.receive_text(id) { Ok(s) => stack.push(Value::Str(s)), Err(e) => return Err(e) }
                    }
                    "ws_send_binary" => {
                        if *arg_count != 2 { return Err("WS_SEND_BINARY expects 2 arguments (ws, data)".to_string()); }
                        let data_val = stack.pop().expect("Expected data for WS_SEND_BINARY");
                        let ws_val = stack.pop().expect("Expected ws handle for WS_SEND_BINARY");
                        let bytes = match data_val { Value::Str(s) => s.into_bytes(), _ => return Err("WS_SEND_BINARY data must be a string".to_string()) };
                        let id = match ws_val { Value::WebSocket(id) => id, _ => return Err("WS_SEND_BINARY expects WebSocket handle".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.send_binary(id, &bytes) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "ws_receive_bytes" => {
                        if *arg_count != 1 { return Err("WS_RECEIVE_BYTES expects 1 argument (ws)".to_string()); }
                        let ws_val = stack.pop().expect("Expected ws handle for WS_RECEIVE_BYTES");
                        let id = match ws_val { Value::WebSocket(id) => id, _ => return Err("WS_RECEIVE_BYTES expects WebSocket handle".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.receive_bytes(id) { Ok(v) => {
                            let mut out = Vec::with_capacity(v.len());
                            for b in v { out.push(Value::Int(b as i64)); }
                            stack.push(Value::Box(out));
                        }, Err(e) => return Err(e) }
                    }
                    "ws_try_receive_bytes" => {
                        if *arg_count != 1 { return Err("WS_TRY_RECEIVE_BYTES expects 1 argument (ws)".to_string()); }
                        let ws_val = stack.pop().expect("Expected ws handle for WS_TRY_RECEIVE_BYTES");
                        let id = match ws_val { Value::WebSocket(id) => id, _ => return Err("WS_TRY_RECEIVE_BYTES expects WebSocket handle".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.try_receive_bytes(id) { Ok(Some(v)) => { let mut out=Vec::new(); for b in v { out.push(Value::Int(b as i64)); } stack.push(Value::Box(out)); }, Ok(None) => stack.push(Value::Box(vec![])), Err(e) => return Err(e) }
                    }
                    "tcp_try_receive" => {
                        if *arg_count != 2 { return Err("TCP_TRY_RECEIVE expects 2 arguments (connection, max_bytes)".to_string()); }
                        let max_val = stack.pop().expect("Expected max_bytes for TCP_TRY_RECEIVE");
                        let conn_val = stack.pop().expect("Expected connection for TCP_TRY_RECEIVE");
                        let max_bytes: usize = match max_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("TCP_TRY_RECEIVE max_bytes must be a number".to_string()) };
                        let id = match conn_val { Value::TcpConnection(id) => id, _ => return Err("TCP_TRY_RECEIVE expects TCP connection".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.try_receive(id, max_bytes) { Ok(Some(v)) => {
                            match String::from_utf8(v) { Ok(s) => stack.push(Value::Str(s)), Err(_e)=> stack.push(Value::Str(String::new())) }
                        }, Ok(None) => stack.push(Value::Str(String::new())), Err(e) => return Err(e) }
                    }
                    "ws_try_receive" => {
                        if *arg_count != 1 { return Err("WS_TRY_RECEIVE expects 1 argument (ws)".to_string()); }
                        let ws_val = stack.pop().expect("Expected ws handle for WS_TRY_RECEIVE");
                        let id = match ws_val { Value::WebSocket(id) => id, _ => return Err("WS_TRY_RECEIVE expects WebSocket handle".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.try_receive_text(id) { Ok(Some(s)) => stack.push(Value::Str(s)), Ok(None) => stack.push(Value::Str(String::new())), Err(e) => return Err(e) }
                    }
                    "ws_close" => {
                        if *arg_count != 1 { return Err("WS_CLOSE expects 1 argument (ws)".to_string()); }
                        let ws_val = stack.pop().expect("Expected ws handle for WS_CLOSE");
                        let id = match ws_val { Value::WebSocket(id) => id, _ => return Err("WS_CLOSE expects WebSocket handle".to_string()) };
                        let mut mgr = ws_manager_global().lock().map_err(|_| "WS manager poisoned".to_string())?;
                        match mgr.close(id) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "http_post" => {
                        if *arg_count < 2 || *arg_count > 3 { return Err("HTTP_POST expects 2 or 3 arguments (url, data[, headers])".to_string()); }
                        let headers = if *arg_count == 3 {
                            let headers_val = stack.pop().expect("Expected headers for HTTP_POST");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let data_val = stack.pop().expect("Expected data for HTTP_POST");
                        let url_val = stack.pop().expect("Expected URL for HTTP_POST");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_POST url must be a string".to_string()) };
                        let data = match data_val { Value::Str(s) => s, _ => return Err("HTTP_POST data must be a string".to_string()) };
                        let (status, hs, body) = http_post_impl(&url, &data, headers.as_deref())?;
                        let resp = Value::Box(vec![
                            Value::Int(status),
                            headers_to_value(&hs),
                            Value::Str(body),
                        ]);
                        stack.push(resp);
                    }
                    "http_put" => {
                        if *arg_count < 2 || *arg_count > 3 { return Err("HTTP_PUT expects 2 or 3 arguments (url, data[, headers])".to_string()); }
                        let headers = if *arg_count == 3 {
                            let headers_val = stack.pop().expect("Expected headers for HTTP_PUT");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let data_val = stack.pop().expect("Expected data for HTTP_PUT");
                        let url_val = stack.pop().expect("Expected URL for HTTP_PUT");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_PUT url must be a string".to_string()) };
                        let data = match data_val { Value::Str(s) => s, _ => return Err("HTTP_PUT data must be a string".to_string()) };
                        let (status, hs, body) = http_put_impl(&url, &data, headers.as_deref())?;
                        let resp = Value::Box(vec![
                            Value::Int(status),
                            headers_to_value(&hs),
                            Value::Str(body),
                        ]);
                        stack.push(resp);
                    }
                    "http_delete" => {
                        if *arg_count < 1 || *arg_count > 2 { return Err("HTTP_DELETE expects 1 or 2 arguments (url[, headers])".to_string()); }
                        let url_val = stack.pop().expect("Expected URL for HTTP_DELETE");
                        let url = match url_val { Value::Str(s) => s, _ => return Err("HTTP_DELETE url must be a string".to_string()) };
                        let headers = if *arg_count == 2 {
                            let headers_val = stack.pop().expect("Expected headers for HTTP_DELETE");
                            Some(parse_headers_box(headers_val)?)
                        } else { None };
                        let (status, hs, body) = http_delete_impl(&url, headers.as_deref())?;
                        let resp = Value::Box(vec![
                            Value::Int(status),
                            headers_to_value(&hs),
                            Value::Str(body),
                        ]);
                        stack.push(resp);
                    }
                    "dns_resolve" => {
                        if *arg_count != 1 { return Err("DNS_RESOLVE expects exactly 1 argument".to_string()); }
                        let hostname_val = stack.pop().expect("Expected hostname for DNS_RESOLVE");
                        let hostname = match hostname_val { Value::Str(s) => s, _ => return Err("DNS_RESOLVE hostname must be a string".to_string()) };
                        match dns_resolve_impl(&hostname) {
                            Ok(ip_address) => stack.push(Value::Str(ip_address)),
                            Err(e) => return Err(e),
                        }
                    }
                    "json_encode" => {
                        if *arg_count != 1 { return Err("JSON_ENCODE expects exactly 1 argument".to_string()); }
                        let val = stack.pop().expect("Expected value for JSON_ENCODE");
                        let j = value_to_json(&val)?;
                        let s = serde_json::to_string(&j).map_err(|e| format!("JSON_ENCODE error: {}", e))?;
                        stack.push(Value::Str(s));
                    }
                    "json_decode" => {
                        if *arg_count != 1 { return Err("JSON_DECODE expects exactly 1 argument".to_string()); }
                        let sval = stack.pop().expect("Expected string for JSON_DECODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("JSON_DECODE argument must be a string".to_string()) };
                        let j: JsonValue = serde_json::from_str(&s).map_err(|e| format!("JSON_DECODE error: {}", e))?;
                        let v = json_to_value(&j);
                        stack.push(v);
                    }
                    "base64_encode" => {
                        if *arg_count != 1 { return Err("BASE64_ENCODE expects exactly 1 argument".to_string()); }
                        let sval = stack.pop().expect("Expected string for BASE64_ENCODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("BASE64_ENCODE argument must be a string".to_string()) };
                        let out = general_purpose::STANDARD.encode(s.as_bytes());
                        stack.push(Value::Str(out));
                    }
                    "base64_decode" => {
                        if *arg_count != 1 { return Err("BASE64_DECODE expects exactly 1 argument".to_string()); }
                        let sval = stack.pop().expect("Expected string for BASE64_DECODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("BASE64_DECODE argument must be a string".to_string()) };
                        let bytes = general_purpose::STANDARD.decode(s.as_bytes()).map_err(|e| format!("BASE64_DECODE error: {}", e))?;
                        let out = String::from_utf8(bytes).map_err(|e| format!("BASE64_DECODE UTF-8 error: {}", e))?;
                        stack.push(Value::Str(out));
                    }
                    "url_encode" => {
                        if *arg_count != 1 { return Err("URL_ENCODE expects exactly 1 argument".to_string()); }
                        let sval = stack.pop().expect("Expected string for URL_ENCODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("URL_ENCODE argument must be a string".to_string()) };
                        let out = urlencoding::encode(&s).into_owned();
                        stack.push(Value::Str(out));
                    }
                    "url_decode" => {
                        if *arg_count != 1 { return Err("URL_DECODE expects exactly 1 argument".to_string()); }
                        let sval = stack.pop().expect("Expected string for URL_DECODE");
                        let s = match sval { Value::Str(s) => s, _ => return Err("URL_DECODE argument must be a string".to_string()) };
                        let out = urlencoding::decode(&s).map_err(|e| format!("URL_DECODE error: {}", e))?.into_owned();
                        stack.push(Value::Str(out));
                    }
                    // Core-8 Hardware Functions (top-level)
                    "time_counter" => {
                        if *arg_count != 0 { return Err("TIME_COUNTER expects no arguments".to_string()); }
                        
                        // Get high-resolution monotonic time counter
                        let counter = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_err(|_| "Failed to get system time")?
                            .as_nanos() as u64;
                        
                        stack.push(Value::Int(counter as i64));
                    }
                    "time_freq" => {
                        if *arg_count != 0 { return Err("TIME_FREQ expects no arguments".to_string()); }
                        
                        // Return frequency in Hz - nanosecond counter runs at 1 billion Hz
                        stack.push(Value::Int(1_000_000_000));
                    }
                    "cpu_halt" => {
                        if *arg_count != 0 { return Err("CPU_HALT expects no arguments".to_string()); }
                        
                        // Low-power halt until interrupt - simulate with short sleep to yield CPU
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        
                        stack.push(Value::Bool(true));
                    }
                    "mem_fence" => {
                        if *arg_count != 0 { return Err("MEM_FENCE expects no arguments".to_string()); }
                        
                        // Full memory barrier - prevents reordering of loads/stores across it
                        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
                        
                        stack.push(Value::Bool(true));
                    }
                    "mem_load" => {
                        if *arg_count != 2 { return Err("MEM_LOAD expects exactly 2 arguments (ptr, width)".to_string()); }
                        let width_val = stack.pop().expect("Expected width for MEM_LOAD");
                        let ptr_val = stack.pop().expect("Expected ptr for MEM_LOAD");
                        
                        let ptr: u64 = match ptr_val { 
                            Value::Int(n) => n as u64, 
                            _ => return Err("MEM_LOAD ptr must be an integer".to_string()) 
                        };
                        let width: u8 = match width_val { 
                            Value::Int(n) => {
                                if n < 1 || n > 8 || (n != 1 && n != 2 && n != 4 && n != 8) {
                                    return Err("MEM_LOAD width must be 1, 2, 4, or 8".to_string());
                                }
                                n as u8
                            }, 
                            _ => return Err("MEM_LOAD width must be an integer".to_string()) 
                        };
                        
                        // Simulate memory load - in real implementation this would be actual memory access
                        // For simulation, return a value based on ptr and width for predictable testing
                        let value = match width {
                            1 => (ptr & 0xFF) as i64,                    // 8-bit
                            2 => ((ptr & 0xFFFF) + 0x100) as i64,       // 16-bit  
                            4 => ((ptr & 0xFFFFFFFF) + 0x10000) as i64, // 32-bit
                            8 => (ptr + 0x100000000) as i64,            // 64-bit
                            _ => unreachable!()
                        };
                        
                        stack.push(Value::Int(value));
                    }
                    "mem_store" => {
                        if *arg_count != 3 { return Err("MEM_STORE expects exactly 3 arguments (ptr, val, width)".to_string()); }
                        let width_val = stack.pop().expect("Expected width for MEM_STORE");
                        let val_val = stack.pop().expect("Expected val for MEM_STORE");
                        let ptr_val = stack.pop().expect("Expected ptr for MEM_STORE");
                        
                        let _ptr: u64 = match ptr_val { 
                            Value::Int(n) => n as u64, 
                            _ => return Err("MEM_STORE ptr must be an integer".to_string()) 
                        };
                        let val: i64 = match val_val { 
                            Value::Int(n) => n, 
                            _ => return Err("MEM_STORE val must be an integer".to_string()) 
                        };
                        let width: u8 = match width_val { 
                            Value::Int(n) => {
                                if n < 1 || n > 8 || (n != 1 && n != 2 && n != 4 && n != 8) {
                                    return Err("MEM_STORE width must be 1, 2, 4, or 8".to_string());
                                }
                                n as u8
                            }, 
                            _ => return Err("MEM_STORE width must be an integer".to_string()) 
                        };
                        
                        // Simulate memory store - in real implementation this would be actual memory write
                        // For simulation, we just validate parameters and return success
                        // In a real implementation, this would write val to memory at ptr with specified width
                        
                        // Validate value fits in specified width
                        let max_val = match width {
                            1 => 255i64,                    // 8-bit: 0-255
                            2 => 65535i64,                  // 16-bit: 0-65535  
                            4 => 4294967295i64,             // 32-bit: 0-4294967295
                            8 => i64::MAX,                  // 64-bit: full range
                            _ => unreachable!()
                        };
                        
                        if val < 0 || val > max_val {
                            return Err(format!("MEM_STORE value {} out of range for width {}", val, width));
                        }
                        
                        stack.push(Value::Bool(true));
                    }
                    "mem_cmpxchg" => {
                        if *arg_count != 3 { return Err("MEM_CMPXCHG expects exactly 3 arguments (ptr, expect, val)".to_string()); }
                        let val_val = stack.pop().expect("Expected val for MEM_CMPXCHG");
                        let expect_val = stack.pop().expect("Expected expect for MEM_CMPXCHG");
                        let ptr_val = stack.pop().expect("Expected ptr for MEM_CMPXCHG");
                        
                        let ptr: u64 = match ptr_val { 
                            Value::Int(n) => n as u64, 
                            _ => return Err("MEM_CMPXCHG ptr must be an integer".to_string()) 
                        };
                        let expect: i64 = match expect_val { 
                            Value::Int(n) => n, 
                            _ => return Err("MEM_CMPXCHG expect must be an integer".to_string()) 
                        };
                        let _val: i64 = match val_val { 
                            Value::Int(n) => n, 
                            _ => return Err("MEM_CMPXCHG val must be an integer".to_string()) 
                        };
                        
                        // Simulate atomic compare-exchange operation
                        // In real implementation, this would be actual atomic CAS with acquire-release semantics
                        // For simulation, we'll simulate the behavior based on ptr value
                        
                        // Simulate current value at memory location (based on ptr for predictable testing)
                        let current_value = ((ptr & 0xFFFFFFFF) + 0x20000) as i64;
                        
                        // Perform compare-and-swap logic
                        let old_value = if current_value == expect {
                            // Values match - would write new value in real implementation
                            // Return the old value (which equals expect)
                            current_value
                        } else {
                            // Values don't match - no write occurs
                            // Return the actual current value
                            current_value
                        };
                        
                        stack.push(Value::Int(old_value));
                    }
                    "int_disable" => {
                        if *arg_count != 0 { return Err("INT_DISABLE expects no arguments".to_string()); }
                        
                        // Simulate interrupt disable operation
                        // In real implementation, this would disable maskable interrupts
                        // and return the previous interrupt flag state
                        
                        // For simulation, we'll return a consistent "previous state" value
                        // In real hardware, this would be the actual interrupt flag from CPU status register
                        let previous_interrupt_state = 1i64; // Simulate interrupts were enabled
                        
                        stack.push(Value::Int(previous_interrupt_state));
                    }
                    "int_enable" => {
                        if *arg_count != 1 { return Err("INT_ENABLE expects exactly 1 argument (interrupt_state)".to_string()); }
                        let state_val = stack.pop().expect("Expected interrupt_state for INT_ENABLE");
                        
                        let interrupt_state: i64 = match state_val { 
                            Value::Int(n) => n, 
                            _ => return Err("INT_ENABLE interrupt_state must be an integer".to_string()) 
                        };
                        
                        // Simulate interrupt enable operation
                        // In real implementation, this would restore the interrupt flag state
                        // and re-enable maskable interrupts if the state indicates they were enabled
                        
                        // For simulation, we validate the state and return success
                        // In real hardware, this would restore the CPU interrupt flag
                        let success = if interrupt_state == 0 || interrupt_state == 1 {
                            true  // Valid interrupt state (0=disabled, 1=enabled)
                        } else {
                            false // Invalid interrupt state
                        };
                        
                        stack.push(Value::Bool(success));
                    }
                    
                    // Character classification functions (top-level)
                    "is_digit" => {
                        if *arg_count != 1 { return Err("IS_DIGIT expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for IS_DIGIT");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_digit()
                                } else {
                                    return Err("IS_DIGIT requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_DIGIT argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "is_alpha" => {
                        if *arg_count != 1 { return Err("IS_ALPHA expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for IS_ALPHA");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_alphabetic()
                                } else {
                                    return Err("IS_ALPHA requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_ALPHA argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "is_space" => {
                        if *arg_count != 1 { return Err("IS_SPACE expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for IS_SPACE");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_whitespace()
                                } else {
                                    return Err("IS_SPACE requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_SPACE argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "is_alnum" => {
                        if *arg_count != 1 { return Err("IS_ALNUM expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for IS_ALNUM");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap().is_ascii_alphanumeric()
                                } else {
                                    return Err("IS_ALNUM requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_ALNUM argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "is_operator" => {
                        if *arg_count != 1 { return Err("IS_OPERATOR expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for IS_OPERATOR");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    let c = s.chars().next().unwrap();
                                    matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '^' | '~')
                                } else {
                                    return Err("IS_OPERATOR requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_OPERATOR argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "is_punctuation" => {
                        if *arg_count != 1 { return Err("IS_PUNCTUATION expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for IS_PUNCTUATION");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    let c = s.chars().next().unwrap();
                                    matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | ':' | '.' | '?' | '"' | '\'' | '`')
                                } else {
                                    return Err("IS_PUNCTUATION requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_PUNCTUATION argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "is_symbol" => {
                        if *arg_count != 1 { return Err("IS_SYMBOL expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for IS_SYMBOL");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    let c = s.chars().next().unwrap();
                                    matches!(c, '@' | '#' | '$' | '\\' | '_')
                                } else {
                                    return Err("IS_SYMBOL requires a single character string".to_string());
                                }
                            }
                            _ => return Err("IS_SYMBOL argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "get_char_category" => {
                        if *arg_count != 1 { return Err("GET_CHAR_CATEGORY expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for GET_CHAR_CATEGORY");
                        
                        let category = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    let c = s.chars().next().unwrap();
                                    if c.is_ascii_alphabetic() {
                                        "alpha"
                                    } else if c.is_ascii_digit() {
                                        "digit"
                                    } else if c.is_ascii_whitespace() {
                                        "space"
                                    } else if matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '^' | '~') {
                                        "operator"
                                    } else if matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | ',' | ';' | ':' | '.' | '?' | '"' | '\'' | '`') {
                                        "punctuation"
                                    } else if matches!(c, '@' | '#' | '$' | '\\' | '_') {
                                        "symbol"
                                    } else {
                                        "other"
                                    }
                                } else {
                                    return Err("GET_CHAR_CATEGORY requires a single character string".to_string());
                                }
                            }
                            _ => return Err("GET_CHAR_CATEGORY argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Str(category.to_string()));
                    }
                    "char_code" => {
                        if *arg_count != 1 { return Err("CHAR_CODE expects exactly 1 argument (char)".to_string()); }
                        let char_val = stack.pop().expect("Expected char for CHAR_CODE");
                        
                        let result = match char_val {
                            Value::Str(s) => {
                                if s.len() == 1 {
                                    s.chars().next().unwrap() as u32 as i64
                                } else {
                                    return Err("CHAR_CODE requires a single character string".to_string());
                                }
                            }
                            _ => return Err("CHAR_CODE argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Int(result));
                    }
                    "char_from_code" => {
                        if *arg_count != 1 { return Err("CHAR_FROM_CODE expects exactly 1 argument (code)".to_string()); }
                        let code_val = stack.pop().expect("Expected code for CHAR_FROM_CODE");
                        
                        let result = match code_val {
                            Value::Int(code) => {
                                if code >= 0 && code <= 127 {
                                    let ch = code as u8 as char;
                                    ch.to_string()
                                } else {
                                    return Err("CHAR_FROM_CODE requires ASCII code 0-127".to_string());
                                }
                            }
                            _ => return Err("CHAR_FROM_CODE argument must be an integer".to_string())
                        };
                        
                        stack.push(Value::Str(result));
                    }
                    
                    // Number parsing functions (top-level)
                    "parse_int" => {
                        if *arg_count != 1 { return Err("PARSE_INT expects exactly 1 argument (string)".to_string()); }
                        let string_val = stack.pop().expect("Expected string for PARSE_INT");
                        
                        let result = match string_val {
                            Value::Str(s) => {
                                match s.trim().parse::<i64>() {
                                    Ok(n) => n,
                                    Err(_) => return Err(format!("PARSE_INT: '{}' is not a valid integer", s))
                                }
                            }
                            _ => return Err("PARSE_INT argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Int(result));
                    }
                    "parse_float" => {
                        if *arg_count != 1 { return Err("PARSE_FLOAT expects exactly 1 argument (string)".to_string()); }
                        let string_val = stack.pop().expect("Expected string for PARSE_FLOAT");
                        
                        let result = match string_val {
                            Value::Str(s) => {
                                match s.trim().parse::<f64>() {
                                    Ok(f) => f,
                                    Err(_) => return Err(format!("PARSE_FLOAT: '{}' is not a valid float", s))
                                }
                            }
                            _ => return Err("PARSE_FLOAT argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Float(result));
                    }
                    
                    // String manipulation functions (top-level)
                    "to_upper" => {
                        if *arg_count != 1 { return Err("TO_UPPER expects exactly 1 argument (string)".to_string()); }
                        let string_val = stack.pop().expect("Expected string for TO_UPPER");
                        
                        let result = match string_val {
                            Value::Str(s) => s.to_uppercase(),
                            _ => return Err("TO_UPPER argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Str(result));
                    }
                    "to_lower" => {
                        if *arg_count != 1 { return Err("TO_LOWER expects exactly 1 argument (string)".to_string()); }
                        let string_val = stack.pop().expect("Expected string for TO_LOWER");
                        
                        let result = match string_val {
                            Value::Str(s) => s.to_lowercase(),
                            _ => return Err("TO_LOWER argument must be a string".to_string())
                        };
                        
                        stack.push(Value::Str(result));
                    }
                    "substring" => {
                        if *arg_count != 3 { return Err("SUBSTRING expects exactly 3 arguments (string, start, end)".to_string()); }
                        let end_val = stack.pop().expect("Expected end for SUBSTRING");
                        let start_val = stack.pop().expect("Expected start for SUBSTRING");
                        let string_val = stack.pop().expect("Expected string for SUBSTRING");
                        
                        let result = match (string_val, start_val, end_val) {
                            (Value::Str(s), Value::Int(start), Value::Int(end)) => {
                                if start < 0 || end < 0 {
                                    return Err("SUBSTRING indices must be non-negative".to_string());
                                }
                                let start_idx = start as usize;
                                let end_idx = end as usize;
                                let chars: Vec<char> = s.chars().collect();
                                
                                if start_idx > chars.len() {
                                    return Err(format!("SUBSTRING start index {} out of bounds (string length {})", start_idx, chars.len()));
                                }
                                if end_idx > chars.len() {
                                    return Err(format!("SUBSTRING end index {} out of bounds (string length {})", end_idx, chars.len()));
                                }
                                if start_idx > end_idx {
                                    return Err("SUBSTRING start index must be <= end index".to_string());
                                }
                                
                                chars[start_idx..end_idx].iter().collect::<String>()
                            }
                            _ => return Err("SUBSTRING requires string, int, int arguments".to_string())
                        };
                        
                        stack.push(Value::Str(result));
                    }
                    "index_of" => {
                        if *arg_count != 2 { return Err("INDEX_OF expects exactly 2 arguments (string, substring)".to_string()); }
                        let substring_val = stack.pop().expect("Expected substring for INDEX_OF");
                        let string_val = stack.pop().expect("Expected string for INDEX_OF");
                        
                        let result = match (string_val, substring_val) {
                            (Value::Str(s), Value::Str(sub)) => {
                                match s.find(&sub) {
                                    Some(index) => index as i64,
                                    None => -1
                                }
                            }
                            _ => return Err("INDEX_OF requires two string arguments".to_string())
                        };
                        
                        stack.push(Value::Int(result));
                    }
                    "starts_with" => {
                        if *arg_count != 2 { return Err("STARTS_WITH expects exactly 2 arguments (string, prefix)".to_string()); }
                        let prefix_val = stack.pop().expect("Expected prefix for STARTS_WITH");
                        let string_val = stack.pop().expect("Expected string for STARTS_WITH");
                        
                        let result = match (string_val, prefix_val) {
                            (Value::Str(s), Value::Str(prefix)) => s.starts_with(&prefix),
                            _ => return Err("STARTS_WITH requires two string arguments".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    "ends_with" => {
                        if *arg_count != 2 { return Err("ENDS_WITH expects exactly 2 arguments (string, suffix)".to_string()); }
                        let suffix_val = stack.pop().expect("Expected suffix for ENDS_WITH");
                        let string_val = stack.pop().expect("Expected string for ENDS_WITH");
                        
                        let result = match (string_val, suffix_val) {
                            (Value::Str(s), Value::Str(suffix)) => s.ends_with(&suffix),
                            _ => return Err("ENDS_WITH requires two string arguments".to_string())
                        };
                        
                        stack.push(Value::Bool(result));
                    }
                    
                    // TCP Network I/O built-ins (top-level)
                    "tcp_connect" => {
                        if *arg_count != 2 { return Err("TCP_CONNECT expects exactly 2 arguments (host, port)".to_string()); }
                        let port_val = stack.pop().expect("Expected port for TCP_CONNECT");
                        let host_val = stack.pop().expect("Expected host for TCP_CONNECT");
                        let host = match host_val { Value::Str(s) => s, _ => return Err("TCP_CONNECT host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TCP_CONNECT port must be a number".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.connect(&host, port) { Ok(id) => stack.push(Value::TcpConnection(id)), Err(e) => return Err(e) }
                    }
                    "tcp_listen" => {
                        if *arg_count != 1 { return Err("TCP_LISTEN expects exactly 1 argument (port)".to_string()); }
                        let port_val = stack.pop().expect("Expected port for TCP_LISTEN");
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TCP_LISTEN port must be a number".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.listen(port) { Ok(id) => stack.push(Value::TcpListener(id)), Err(e) => return Err(e) }
                    }
                    "tcp_send" => {
                        if *arg_count != 2 { return Err("TCP_SEND expects exactly 2 arguments (connection, data)".to_string()); }
                        let data_val = stack.pop().expect("Expected data for TCP_SEND");
                        let conn_val = stack.pop().expect("Expected connection for TCP_SEND");
                        let data = match data_val { Value::Str(s) => s, _ => return Err("TCP_SEND data must be a string".to_string()) };
                        let id = match conn_val { Value::TcpConnection(id) => id, _ => return Err("TCP_SEND expects TCP connection".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.send(id, &data) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "tcp_receive" => {
                        if *arg_count != 2 { return Err("TCP_RECEIVE expects exactly 2 arguments (connection, max_bytes)".to_string()); }
                        let max_val = stack.pop().expect("Expected max_bytes for TCP_RECEIVE");
                        let conn_val = stack.pop().expect("Expected connection for TCP_RECEIVE");
                        let max_bytes: usize = match max_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("TCP_RECEIVE max_bytes must be a number".to_string()) };
                        let id = match conn_val { Value::TcpConnection(id) => id, _ => return Err("TCP_RECEIVE expects TCP connection".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.receive(id, max_bytes) { Ok(s) => stack.push(Value::Str(s)), Err(e) => return Err(e) }
                    }
                    "tcp_close" => {
                        if *arg_count != 1 { return Err("TCP_CLOSE expects exactly 1 argument (connection or listener)".to_string()); }
                        let val = stack.pop().expect("Expected argument for TCP_CLOSE");
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match val {
                            Value::TcpConnection(id) => match mgr.close_connection(id) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) },
                            Value::TcpListener(id) => match mgr.close_listener(id) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) },
                            _ => return Err("TCP_CLOSE expects TCP connection or listener".to_string()),
                        }
                    }
                    "tcp_accept" => {
                        if *arg_count != 1 { return Err("TCP_ACCEPT expects exactly 1 argument (listener)".to_string()); }
                        let val = stack.pop().expect("Expected listener for TCP_ACCEPT");
                        let id = match val { Value::TcpListener(id) => id, _ => return Err("TCP_ACCEPT expects TCP listener".to_string()) };
                        let mut mgr = tcp_manager_global().lock().map_err(|_| "TCP manager poisoned".to_string())?;
                        match mgr.accept(id) { Ok(conn_id) => stack.push(Value::TcpConnection(conn_id)), Err(e) => return Err(e) }
                    }
                    
                    // UDP Network I/O built-ins (top-level)
                    "udp_bind" => {
                        if *arg_count != 1 { return Err("UDP_BIND expects exactly 1 argument (address)".to_string()); }
                        let addr_val = stack.pop().expect("Expected address for UDP_BIND");
                        let addr = match addr_val { Value::Str(s) => s, _ => return Err("UDP_BIND address must be a string".to_string()) };
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        match mgr.bind(&addr) { Ok(id) => stack.push(Value::UdpSocket(id)), Err(e) => return Err(e) }
                    }
                    "udp_send" => {
                        if *arg_count != 3 { return Err("UDP_SEND expects exactly 3 arguments (socket, data, target_addr)".to_string()); }
                        let addr_val = stack.pop().expect("Expected target address for UDP_SEND");
                        let data_val = stack.pop().expect("Expected data for UDP_SEND");
                        let socket_val = stack.pop().expect("Expected socket for UDP_SEND");
                        let addr = match addr_val { Value::Str(s) => s, _ => return Err("UDP_SEND target address must be a string".to_string()) };
                        let data = match data_val { Value::Str(s) => s, _ => return Err("UDP_SEND data must be a string".to_string()) };
                        let id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_SEND expects UDP socket".to_string()) };
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        match mgr.send(id, data.as_bytes(), &addr) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "udp_receive" => {
                        if *arg_count != 2 { return Err("UDP_RECEIVE expects exactly 2 arguments (socket, max_bytes)".to_string()); }
                        let max_val = stack.pop().expect("Expected max_bytes for UDP_RECEIVE");
                        let socket_val = stack.pop().expect("Expected socket for UDP_RECEIVE");
                        let max_bytes: usize = match max_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("UDP_RECEIVE max_bytes must be a number".to_string()) };
                        let id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_RECEIVE expects UDP socket".to_string()) };
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        match mgr.receive(id, max_bytes) {
                            Ok((data, src_addr)) => {
                                let data_str = String::from_utf8_lossy(&data).to_string();
                                stack.push(Value::Box(vec![Value::Str(data_str), Value::Str(src_addr)]));
                            }
                            Err(e) => return Err(e)
                        }
                    }
                    "udp_try_receive" => {
                        if *arg_count != 2 { return Err("UDP_TRY_RECEIVE expects exactly 2 arguments (socket, max_bytes)".to_string()); }
                        let max_val = stack.pop().expect("Expected max_bytes for UDP_TRY_RECEIVE");
                        let socket_val = stack.pop().expect("Expected socket for UDP_TRY_RECEIVE");
                        let max_bytes: usize = match max_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("UDP_TRY_RECEIVE max_bytes must be a number".to_string()) };
                        let id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_TRY_RECEIVE expects UDP socket".to_string()) };
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        match mgr.try_receive(id, max_bytes) {
                            Ok(Some((data, src_addr))) => {
                                let data_str = String::from_utf8_lossy(&data).to_string();
                                stack.push(Value::Box(vec![Value::Str(data_str), Value::Str(src_addr)]));
                            }
                            Ok(None) => stack.push(Value::None),
                            Err(e) => return Err(e)
                        }
                    }
                    "udp_close" => {
                        if *arg_count != 1 { return Err("UDP_CLOSE expects exactly 1 argument (socket)".to_string()); }
                        let socket_val = stack.pop().expect("Expected socket for UDP_CLOSE");
                        let id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_CLOSE expects UDP socket".to_string()) };
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        match mgr.close(id) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    
                    // UDP Multicast and Broadcast built-ins (top-level)
                    "udp_join_multicast" => {
                        if *arg_count < 2 || *arg_count > 3 { return Err("UDP_JOIN_MULTICAST expects 2-3 arguments (socket, multicast_addr, [interface_addr])".to_string()); }
                        
                        let interface_addr = if *arg_count >= 3 {
                            let addr_val = stack.pop().expect("Expected interface_addr for UDP_JOIN_MULTICAST");
                            match addr_val { Value::Str(s) => Some(s), _ => return Err("UDP_JOIN_MULTICAST interface_addr must be a string".to_string()) }
                        } else { None };
                        
                        let multicast_addr_val = stack.pop().expect("Expected multicast_addr for UDP_JOIN_MULTICAST");
                        let socket_val = stack.pop().expect("Expected socket for UDP_JOIN_MULTICAST");
                        
                        let multicast_addr = match multicast_addr_val { Value::Str(s) => s, _ => return Err("UDP_JOIN_MULTICAST multicast_addr must be a string".to_string()) };
                        let socket_id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_JOIN_MULTICAST socket must be a UdpSocket".to_string()) };
                        
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        mgr.join_multicast_group(socket_id, &multicast_addr, interface_addr.as_deref())?;
                        stack.push(Value::Bool(true));
                    }
                    "udp_leave_multicast" => {
                        if *arg_count < 2 || *arg_count > 3 { return Err("UDP_LEAVE_MULTICAST expects 2-3 arguments (socket, multicast_addr, [interface_addr])".to_string()); }
                        
                        let interface_addr = if *arg_count >= 3 {
                            let addr_val = stack.pop().expect("Expected interface_addr for UDP_LEAVE_MULTICAST");
                            match addr_val { Value::Str(s) => Some(s), _ => return Err("UDP_LEAVE_MULTICAST interface_addr must be a string".to_string()) }
                        } else { None };
                        
                        let multicast_addr_val = stack.pop().expect("Expected multicast_addr for UDP_LEAVE_MULTICAST");
                        let socket_val = stack.pop().expect("Expected socket for UDP_LEAVE_MULTICAST");
                        
                        let multicast_addr = match multicast_addr_val { Value::Str(s) => s, _ => return Err("UDP_LEAVE_MULTICAST multicast_addr must be a string".to_string()) };
                        let socket_id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_LEAVE_MULTICAST socket must be a UdpSocket".to_string()) };
                        
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        mgr.leave_multicast_group(socket_id, &multicast_addr, interface_addr.as_deref())?;
                        stack.push(Value::Bool(true));
                    }
                    "udp_set_multicast_ttl" => {
                        if *arg_count != 2 { return Err("UDP_SET_MULTICAST_TTL expects exactly 2 arguments (socket, ttl)".to_string()); }
                        let ttl_val = stack.pop().expect("Expected ttl for UDP_SET_MULTICAST_TTL");
                        let socket_val = stack.pop().expect("Expected socket for UDP_SET_MULTICAST_TTL");
                        
                        let ttl: u32 = match ttl_val { Value::Int(n) => n as u32, Value::Float(f) => f as u32, _ => return Err("UDP_SET_MULTICAST_TTL ttl must be a number".to_string()) };
                        let socket_id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_SET_MULTICAST_TTL socket must be a UdpSocket".to_string()) };
                        
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        mgr.set_multicast_ttl(socket_id, ttl)?;
                        stack.push(Value::Bool(true));
                    }
                    "udp_set_multicast_loopback" => {
                        if *arg_count != 2 { return Err("UDP_SET_MULTICAST_LOOPBACK expects exactly 2 arguments (socket, loopback)".to_string()); }
                        let loopback_val = stack.pop().expect("Expected loopback for UDP_SET_MULTICAST_LOOPBACK");
                        let socket_val = stack.pop().expect("Expected socket for UDP_SET_MULTICAST_LOOPBACK");
                        
                        let loopback = match loopback_val { Value::Bool(b) => b, _ => return Err("UDP_SET_MULTICAST_LOOPBACK loopback must be a boolean".to_string()) };
                        let socket_id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_SET_MULTICAST_LOOPBACK socket must be a UdpSocket".to_string()) };
                        
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        mgr.set_multicast_loopback(socket_id, loopback)?;
                        stack.push(Value::Bool(true));
                    }
                    "udp_set_broadcast" => {
                        if *arg_count != 2 { return Err("UDP_SET_BROADCAST expects exactly 2 arguments (socket, broadcast)".to_string()); }
                        let broadcast_val = stack.pop().expect("Expected broadcast for UDP_SET_BROADCAST");
                        let socket_val = stack.pop().expect("Expected socket for UDP_SET_BROADCAST");
                        
                        let broadcast = match broadcast_val { Value::Bool(b) => b, _ => return Err("UDP_SET_BROADCAST broadcast must be a boolean".to_string()) };
                        let socket_id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_SET_BROADCAST socket must be a UdpSocket".to_string()) };
                        
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        mgr.set_broadcast(socket_id, broadcast)?;
                        stack.push(Value::Bool(true));
                    }
                    "udp_send_broadcast" => {
                        if *arg_count != 3 { return Err("UDP_SEND_BROADCAST expects exactly 3 arguments (socket, data, port)".to_string()); }
                        let port_val = stack.pop().expect("Expected port for UDP_SEND_BROADCAST");
                        let data_val = stack.pop().expect("Expected data for UDP_SEND_BROADCAST");
                        let socket_val = stack.pop().expect("Expected socket for UDP_SEND_BROADCAST");
                        
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("UDP_SEND_BROADCAST port must be a number".to_string()) };
                        let data = match data_val { Value::Str(s) => s.into_bytes(), _ => return Err("UDP_SEND_BROADCAST data must be a string".to_string()) };
                        let socket_id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_SEND_BROADCAST socket must be a UdpSocket".to_string()) };
                        
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        let bytes_sent = mgr.send_broadcast(socket_id, &data, port)?;
                        stack.push(Value::Int(bytes_sent as i64));
                    }
                    "udp_send_multicast" => {
                        if *arg_count != 4 { return Err("UDP_SEND_MULTICAST expects exactly 4 arguments (socket, data, multicast_addr, port)".to_string()); }
                        let port_val = stack.pop().expect("Expected port for UDP_SEND_MULTICAST");
                        let multicast_addr_val = stack.pop().expect("Expected multicast_addr for UDP_SEND_MULTICAST");
                        let data_val = stack.pop().expect("Expected data for UDP_SEND_MULTICAST");
                        let socket_val = stack.pop().expect("Expected socket for UDP_SEND_MULTICAST");
                        
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("UDP_SEND_MULTICAST port must be a number".to_string()) };
                        let multicast_addr = match multicast_addr_val { Value::Str(s) => s, _ => return Err("UDP_SEND_MULTICAST multicast_addr must be a string".to_string()) };
                        let data = match data_val { Value::Str(s) => s.into_bytes(), _ => return Err("UDP_SEND_MULTICAST data must be a string".to_string()) };
                        let socket_id = match socket_val { Value::UdpSocket(id) => id, _ => return Err("UDP_SEND_MULTICAST socket must be a UdpSocket".to_string()) };
                        
                        let mut mgr = udp_manager_global().lock().map_err(|_| "UDP manager poisoned".to_string())?;
                        let bytes_sent = mgr.send_multicast(socket_id, &data, &multicast_addr, port)?;
                        stack.push(Value::Int(bytes_sent as i64));
                    }
                    "udp_is_multicast" => {
                        if *arg_count != 1 { return Err("UDP_IS_MULTICAST expects exactly 1 argument (address)".to_string()); }
                        let addr_val = stack.pop().expect("Expected address for UDP_IS_MULTICAST");
                        let addr = match addr_val { Value::Str(s) => s, _ => return Err("UDP_IS_MULTICAST address must be a string".to_string()) };
                        
                        use crate::udp_socket_manager::UdpSocketManager;
                        let is_multicast = UdpSocketManager::is_multicast_address(&addr);
                        stack.push(Value::Bool(is_multicast));
                    }
                    "udp_is_broadcast" => {
                        if *arg_count != 1 { return Err("UDP_IS_BROADCAST expects exactly 1 argument (address)".to_string()); }
                        let addr_val = stack.pop().expect("Expected address for UDP_IS_BROADCAST");
                        let addr = match addr_val { Value::Str(s) => s, _ => return Err("UDP_IS_BROADCAST address must be a string".to_string()) };
                        
                        use crate::udp_socket_manager::UdpSocketManager;
                        let is_broadcast = UdpSocketManager::is_broadcast_address(&addr);
                        stack.push(Value::Bool(is_broadcast));
                    }
                    
                    // TLS Network I/O built-ins (top-level)
                    "tls_connect" => {
                        if *arg_count != 2 { return Err("TLS_CONNECT expects exactly 2 arguments (host, port)".to_string()); }
                        let port_val = stack.pop().expect("Expected port for TLS_CONNECT");
                        let host_val = stack.pop().expect("Expected host for TLS_CONNECT");
                        let host = match host_val { Value::Str(s) => s, _ => return Err("TLS_CONNECT host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TLS_CONNECT port must be a number".to_string()) };
                        let mut mgr = tls_manager_global().lock().map_err(|_| "TLS manager poisoned".to_string())?;
                        match mgr.connect(&host, port) { Ok(id) => stack.push(Value::TlsConnection(id)), Err(e) => return Err(e) }
                    }
                    "tls_listen" => {
                        if *arg_count != 3 { return Err("TLS_LISTEN expects exactly 3 arguments (port, cert_path, key_path)".to_string()); }
                        let key_val = stack.pop().expect("Expected key_path for TLS_LISTEN");
                        let cert_val = stack.pop().expect("Expected cert_path for TLS_LISTEN");
                        let port_val = stack.pop().expect("Expected port for TLS_LISTEN");
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TLS_LISTEN port must be a number".to_string()) };
                        let cert_path = match cert_val { Value::Str(s) => s, _ => return Err("TLS_LISTEN cert_path must be a string".to_string()) };
                        let key_path = match key_val { Value::Str(s) => s, _ => return Err("TLS_LISTEN key_path must be a string".to_string()) };
                        let mut mgr = tls_manager_global().lock().map_err(|_| "TLS manager poisoned".to_string())?;
                        match mgr.listen(port, &cert_path, &key_path) { Ok(id) => stack.push(Value::TlsListener(id)), Err(e) => return Err(e) }
                    }
                    "tls_accept" => {
                        if *arg_count != 1 { return Err("TLS_ACCEPT expects exactly 1 argument (listener)".to_string()); }
                        let listener_val = stack.pop().expect("Expected listener for TLS_ACCEPT");
                        let id = match listener_val { Value::TlsListener(id) => id, _ => return Err("TLS_ACCEPT expects TLS listener".to_string()) };
                        let mut mgr = tls_manager_global().lock().map_err(|_| "TLS manager poisoned".to_string())?;
                        match mgr.accept(id) { Ok(conn_id) => stack.push(Value::TlsConnection(conn_id)), Err(e) => return Err(e) }
                    }
                    "tls_send" => {
                        if *arg_count != 2 { return Err("TLS_SEND expects exactly 2 arguments (connection, data)".to_string()); }
                        let data_val = stack.pop().expect("Expected data for TLS_SEND");
                        let conn_val = stack.pop().expect("Expected connection for TLS_SEND");
                        let data = match data_val { Value::Str(s) => s, _ => return Err("TLS_SEND data must be a string".to_string()) };
                        let id = match conn_val { Value::TlsConnection(id) => id, _ => return Err("TLS_SEND expects TLS connection".to_string()) };
                        let mut mgr = tls_manager_global().lock().map_err(|_| "TLS manager poisoned".to_string())?;
                        match mgr.send(id, &data) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "tls_receive" => {
                        if *arg_count != 2 { return Err("TLS_RECEIVE expects exactly 2 arguments (connection, max_bytes)".to_string()); }
                        let max_val = stack.pop().expect("Expected max_bytes for TLS_RECEIVE");
                        let conn_val = stack.pop().expect("Expected connection for TLS_RECEIVE");
                        let max_bytes: usize = match max_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("TLS_RECEIVE max_bytes must be a number".to_string()) };
                        let id = match conn_val { Value::TlsConnection(id) => id, _ => return Err("TLS_RECEIVE expects TLS connection".to_string()) };
                        let mut mgr = tls_manager_global().lock().map_err(|_| "TLS manager poisoned".to_string())?;
                        match mgr.receive(id, max_bytes) { Ok(s) => stack.push(Value::Str(s)), Err(e) => return Err(e) }
                    }
                    "tls_try_receive" => {
                        if *arg_count != 2 { return Err("TLS_TRY_RECEIVE expects exactly 2 arguments (connection, max_bytes)".to_string()); }
                        let max_val = stack.pop().expect("Expected max_bytes for TLS_TRY_RECEIVE");
                        let conn_val = stack.pop().expect("Expected connection for TLS_TRY_RECEIVE");
                        let max_bytes: usize = match max_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("TLS_TRY_RECEIVE max_bytes must be a number".to_string()) };
                        let id = match conn_val { Value::TlsConnection(id) => id, _ => return Err("TLS_TRY_RECEIVE expects TLS connection".to_string()) };
                        let mut mgr = tls_manager_global().lock().map_err(|_| "TLS manager poisoned".to_string())?;
                        match mgr.try_receive(id, max_bytes) {
                            Ok(Some(data)) => stack.push(Value::Str(data)),
                            Ok(None) => stack.push(Value::None),
                            Err(e) => return Err(e)
                        }
                    }
                    "tls_close" => {
                        if *arg_count != 1 { return Err("TLS_CLOSE expects exactly 1 argument (connection or listener)".to_string()); }
                        let val = stack.pop().expect("Expected argument for TLS_CLOSE");
                        let mut mgr = tls_manager_global().lock().map_err(|_| "TLS manager poisoned".to_string())?;
                        match val {
                            Value::TlsConnection(id) => match mgr.close_connection(id) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) },
                            Value::TlsListener(id) => match mgr.close_listener(id) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) },
                            _ => return Err("TLS_CLOSE expects TLS connection or listener".to_string()),
                        }
                    }
                    
                    // Event Loop / Async Network I/O built-ins (top-level)
                    "event_register" => {
                        if *arg_count != 2 { return Err("EVENT_REGISTER expects exactly 2 arguments (socket, event_types)".to_string()); }
                        let events_val = stack.pop().expect("Expected event_types for EVENT_REGISTER");
                        let socket_val = stack.pop().expect("Expected socket for EVENT_REGISTER");
                        
                        // Convert socket value to SocketType
                        let socket_type = match socket_val {
                            Value::TcpConnection(id) => SocketType::TcpConnection(id),
                            Value::TcpListener(id) => SocketType::TcpListener(id),
                            Value::UdpSocket(id) => SocketType::UdpSocket(id),
                            Value::TlsConnection(id) => SocketType::TlsConnection(id),
                            Value::TlsListener(id) => SocketType::TlsListener(id),
                            Value::WebSocket(id) => SocketType::WebSocket(id),
                            _ => return Err("EVENT_REGISTER expects a socket".to_string()),
                        };
                        
                        // Convert event types (expecting a box of strings)
                        let event_types = match events_val {
                            Value::Box(items) => {
                                let mut events = Vec::new();
                                for item in items {
                                    match item {
                                        Value::Str(s) => {
                                            match s.as_str() {
                                                "read" => events.push(EventType::Read),
                                                "write" => events.push(EventType::Write),
                                                "accept" => events.push(EventType::Accept),
                                                "connect" => events.push(EventType::Connect),
                                                _ => return Err(format!("Unknown event type: {}", s)),
                                            }
                                        }
                                        _ => return Err("Event types must be strings".to_string()),
                                    }
                                }
                                events
                            }
                            Value::Str(s) => {
                                // Single event type
                                vec![match s.as_str() {
                                    "read" => EventType::Read,
                                    "write" => EventType::Write,
                                    "accept" => EventType::Accept,
                                    "connect" => EventType::Connect,
                                    _ => return Err(format!("Unknown event type: {}", s)),
                                }]
                            }
                            _ => return Err("Event types must be a string or box of strings".to_string()),
                        };
                        
                        let mut mgr = event_loop_global().lock().map_err(|_| "Event loop manager poisoned".to_string())?;
                        let event_id = mgr.register_socket(socket_type, event_types);
                        stack.push(Value::Str(event_id));
                    }
                    "event_unregister" => {
                        if *arg_count != 1 { return Err("EVENT_UNREGISTER expects exactly 1 argument (event_id)".to_string()); }
                        let event_id_val = stack.pop().expect("Expected event_id for EVENT_UNREGISTER");
                        let event_id = match event_id_val { Value::Str(s) => s, _ => return Err("EVENT_UNREGISTER event_id must be a string".to_string()) };
                        
                        let mut mgr = event_loop_global().lock().map_err(|_| "Event loop manager poisoned".to_string())?;
                        match mgr.unregister_socket(&event_id) { Ok(_) => stack.push(Value::Bool(true)), Err(e) => return Err(e) }
                    }
                    "event_poll" => {
                        if *arg_count != 0 { return Err("EVENT_POLL expects no arguments".to_string()); }
                        
                        let mut mgr = event_loop_global().lock().map_err(|_| "Event loop manager poisoned".to_string())?;
                        let events = mgr.poll_events();
                        
                        // Convert events to Scraps format
                        let mut event_boxes = Vec::new();
                        for event in events {
                            let socket_str = match event.socket {
                                SocketType::TcpConnection(id) => format!("tcp_connection_{}", id),
                                SocketType::TcpListener(id) => format!("tcp_listener_{}", id),
                                SocketType::UdpSocket(id) => format!("udp_socket_{}", id),
                                SocketType::TlsConnection(id) => format!("tls_connection_{}", id),
                                SocketType::TlsListener(id) => format!("tls_listener_{}", id),
                                SocketType::WebSocket(id) => format!("websocket_{}", id),
                            };
                            let event_str = match event.event_type {
                                EventType::Read => "read",
                                EventType::Write => "write",
                                EventType::Accept => "accept",
                                EventType::Connect => "connect",
                            };
                            event_boxes.push(Value::Box(vec![
                                Value::Str(socket_str),
                                Value::Str(event_str.to_string()),
                            ]));
                        }
                        stack.push(Value::Box(event_boxes));
                    }
                    "event_wait" => {
                        if *arg_count != 1 { return Err("EVENT_WAIT expects exactly 1 argument (timeout_ms)".to_string()); }
                        let timeout_val = stack.pop().expect("Expected timeout_ms for EVENT_WAIT");
                        let timeout_ms: Option<u64> = match timeout_val {
                            Value::Int(n) if n >= 0 => Some(n as u64),
                            Value::Float(f) if f >= 0.0 => Some(f as u64),
                            Value::Int(-1) => None, // -1 means no timeout
                            Value::Float(f) if f < 0.0 => None, // negative means no timeout
                            _ => return Err("EVENT_WAIT timeout_ms must be a non-negative number or -1".to_string()),
                        };
                        
                        let mut mgr = event_loop_global().lock().map_err(|_| "Event loop manager poisoned".to_string())?;
                        let events = mgr.wait_for_events(timeout_ms);
                        
                        // Convert events to Scraps format (same as event_poll)
                        let mut event_boxes = Vec::new();
                        for event in events {
                            let socket_str = match event.socket {
                                SocketType::TcpConnection(id) => format!("tcp_connection_{}", id),
                                SocketType::TcpListener(id) => format!("tcp_listener_{}", id),
                                SocketType::UdpSocket(id) => format!("udp_socket_{}", id),
                                SocketType::TlsConnection(id) => format!("tls_connection_{}", id),
                                SocketType::TlsListener(id) => format!("tls_listener_{}", id),
                                SocketType::WebSocket(id) => format!("websocket_{}", id),
                            };
                            let event_str = match event.event_type {
                                EventType::Read => "read",
                                EventType::Write => "write",
                                EventType::Accept => "accept",
                                EventType::Connect => "connect",
                            };
                            event_boxes.push(Value::Box(vec![
                                Value::Str(socket_str),
                                Value::Str(event_str.to_string()),
                            ]));
                        }
                        stack.push(Value::Box(event_boxes));
                    }
                    "event_wait_any" => {
                        if *arg_count < 1 || *arg_count > 3 { return Err("EVENT_WAIT_ANY expects 1-3 arguments (socket_types, [event_types], [timeout_ms])".to_string()); }
                        
                        // Get timeout (optional, last argument)
                        let timeout_ms = if *arg_count >= 3 {
                            let timeout_val = stack.pop().expect("Expected timeout_ms");
                            match timeout_val {
                                Value::Int(n) if n >= 0 => Some(n as u64),
                                Value::Float(f) if f >= 0.0 => Some(f as u64),
                                Value::Int(-1) => None,
                                Value::Float(f) if f < 0.0 => None,
                                _ => return Err("EVENT_WAIT_ANY timeout_ms must be a non-negative number or -1".to_string()),
                            }
                        } else { None };
                        
                        // Get event types (optional, middle argument)
                        let event_types = if *arg_count >= 2 {
                            let events_val = stack.pop().expect("Expected event_types");
                            match events_val {
                                Value::Box(items) => {
                                    let mut events = Vec::new();
                                    for item in items {
                                        match item {
                                            Value::Str(s) => {
                                                match s.as_str() {
                                                    "read" => events.push(EventType::Read),
                                                    "write" => events.push(EventType::Write),
                                                    "accept" => events.push(EventType::Accept),
                                                    "connect" => events.push(EventType::Connect),
                                                    _ => return Err(format!("Unknown event type: {}", s)),
                                                }
                                            }
                                            _ => return Err("Event types must be strings".to_string()),
                                        }
                                    }
                                    events
                                }
                                _ => Vec::new(), // Empty means all event types
                            }
                        } else { Vec::new() };
                        
                        // Get socket types (required, first argument)
                        let socket_types_val = stack.pop().expect("Expected socket_types");
                        let socket_types = match socket_types_val {
                            Value::Box(items) => {
                                let mut sockets = Vec::new();
                                for item in items {
                                    match item {
                                        Value::TcpConnection(id) => sockets.push(SocketType::TcpConnection(id)),
                                        Value::TcpListener(id) => sockets.push(SocketType::TcpListener(id)),
                                        Value::UdpSocket(id) => sockets.push(SocketType::UdpSocket(id)),
                                        Value::TlsConnection(id) => sockets.push(SocketType::TlsConnection(id)),
                                        Value::TlsListener(id) => sockets.push(SocketType::TlsListener(id)),
                                        Value::WebSocket(id) => sockets.push(SocketType::WebSocket(id)),
                                        _ => return Err("Socket types must be socket objects".to_string()),
                                    }
                                }
                                sockets
                            }
                            _ => Vec::new(), // Empty means all socket types
                        };
                        
                        let filter = EventFilter { socket_types, event_types, timeout_ms };
                        let mut mgr = event_loop_global().lock().map_err(|_| "Event loop manager poisoned".to_string())?;
                        
                        match mgr.wait_for_any(filter) {
                            Some(event) => {
                                let socket_str = match event.socket {
                                    SocketType::TcpConnection(id) => format!("tcp_connection_{}", id),
                                    SocketType::TcpListener(id) => format!("tcp_listener_{}", id),
                                    SocketType::UdpSocket(id) => format!("udp_socket_{}", id),
                                    SocketType::TlsConnection(id) => format!("tls_connection_{}", id),
                                    SocketType::TlsListener(id) => format!("tls_listener_{}", id),
                                    SocketType::WebSocket(id) => format!("websocket_{}", id),
                                };
                                let event_str = match event.event_type {
                                    EventType::Read => "read",
                                    EventType::Write => "write",
                                    EventType::Accept => "accept",
                                    EventType::Connect => "connect",
                                };
                                stack.push(Value::Box(vec![
                                    Value::Str(socket_str),
                                    Value::Str(event_str.to_string()),
                                ]));
                            }
                            None => stack.push(Value::None),
                        }
                    }
                    
                    // Connection Pool built-ins (top-level)
                    "pool_stats" => {
                        if *arg_count != 3 { return Err("POOL_STATS expects exactly 3 arguments (host, port, protocol)".to_string()); }
                        let protocol_val = stack.pop().expect("Expected protocol for POOL_STATS");
                        let port_val = stack.pop().expect("Expected port for POOL_STATS");
                        let host_val = stack.pop().expect("Expected host for POOL_STATS");
                        
                        let host = match host_val { Value::Str(s) => s, _ => return Err("POOL_STATS host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("POOL_STATS port must be a number".to_string()) };
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("POOL_STATS protocol must be a string".to_string()) };
                        
                        let mgr = pool_manager_global().lock().map_err(|_| "Pool manager poisoned".to_string())?;
                        let stats = mgr.get_pool_stats(&host, port, &protocol);
                        
                        // Return stats as a box: [pool_size, max_pool_size, total_pools]
                        stack.push(Value::Box(vec![
                            Value::Int(stats.pool_size as i64),
                            Value::Int(stats.max_pool_size as i64),
                            Value::Int(stats.total_pools as i64),
                        ]));
                    }
                    "pool_clear" => {
                        if *arg_count != 3 { return Err("POOL_CLEAR expects exactly 3 arguments (host, port, protocol)".to_string()); }
                        let protocol_val = stack.pop().expect("Expected protocol for POOL_CLEAR");
                        let port_val = stack.pop().expect("Expected port for POOL_CLEAR");
                        let host_val = stack.pop().expect("Expected host for POOL_CLEAR");
                        
                        let host = match host_val { Value::Str(s) => s, _ => return Err("POOL_CLEAR host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("POOL_CLEAR port must be a number".to_string()) };
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("POOL_CLEAR protocol must be a string".to_string()) };
                        
                        let mut mgr = pool_manager_global().lock().map_err(|_| "Pool manager poisoned".to_string())?;
                        mgr.clear_pool(&host, port, &protocol);
                        stack.push(Value::Bool(true));
                    }
                    "pool_configure" => {
                        if *arg_count != 3 { return Err("POOL_CONFIGURE expects exactly 3 arguments (max_connections, max_idle_seconds, max_lifetime_seconds)".to_string()); }
                        let lifetime_val = stack.pop().expect("Expected max_lifetime_seconds for POOL_CONFIGURE");
                        let idle_val = stack.pop().expect("Expected max_idle_seconds for POOL_CONFIGURE");
                        let max_val = stack.pop().expect("Expected max_connections for POOL_CONFIGURE");
                        
                        let max_connections: usize = match max_val { Value::Int(n) => n as usize, Value::Float(f) => f as usize, _ => return Err("POOL_CONFIGURE max_connections must be a number".to_string()) };
                        let max_idle_seconds: u64 = match idle_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("POOL_CONFIGURE max_idle_seconds must be a number".to_string()) };
                        let max_lifetime_seconds: u64 = match lifetime_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("POOL_CONFIGURE max_lifetime_seconds must be a number".to_string()) };
                        
                        let mut mgr = pool_manager_global().lock().map_err(|_| "Pool manager poisoned".to_string())?;
                        mgr.configure_pool(max_connections, max_idle_seconds, max_lifetime_seconds);
                        stack.push(Value::Bool(true));
                    }
                    
                    // Timeout Management built-ins (top-level)
                    "timeout_set_global" => {
                        if *arg_count != 4 { return Err("TIMEOUT_SET_GLOBAL expects exactly 4 arguments (protocol, connect_ms, read_ms, write_ms)".to_string()); }
                        let write_val = stack.pop().expect("Expected write_ms for TIMEOUT_SET_GLOBAL");
                        let read_val = stack.pop().expect("Expected read_ms for TIMEOUT_SET_GLOBAL");
                        let connect_val = stack.pop().expect("Expected connect_ms for TIMEOUT_SET_GLOBAL");
                        let protocol_val = stack.pop().expect("Expected protocol for TIMEOUT_SET_GLOBAL");
                        
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("TIMEOUT_SET_GLOBAL protocol must be a string".to_string()) };
                        let connect_ms: u64 = match connect_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("TIMEOUT_SET_GLOBAL connect_ms must be a number".to_string()) };
                        let read_ms: u64 = match read_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("TIMEOUT_SET_GLOBAL read_ms must be a number".to_string()) };
                        let write_ms: u64 = match write_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("TIMEOUT_SET_GLOBAL write_ms must be a number".to_string()) };
                        
                        let mut mgr = timeout_manager_global().lock().map_err(|_| "Timeout manager poisoned".to_string())?;
                        mgr.set_global_timeout(&protocol, connect_ms, read_ms, write_ms);
                        stack.push(Value::Bool(true));
                    }
                    "timeout_set_specific" => {
                        if *arg_count != 6 { return Err("TIMEOUT_SET_SPECIFIC expects exactly 6 arguments (protocol, host, port, connect_ms, read_ms, write_ms)".to_string()); }
                        let write_val = stack.pop().expect("Expected write_ms for TIMEOUT_SET_SPECIFIC");
                        let read_val = stack.pop().expect("Expected read_ms for TIMEOUT_SET_SPECIFIC");
                        let connect_val = stack.pop().expect("Expected connect_ms for TIMEOUT_SET_SPECIFIC");
                        let port_val = stack.pop().expect("Expected port for TIMEOUT_SET_SPECIFIC");
                        let host_val = stack.pop().expect("Expected host for TIMEOUT_SET_SPECIFIC");
                        let protocol_val = stack.pop().expect("Expected protocol for TIMEOUT_SET_SPECIFIC");
                        
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("TIMEOUT_SET_SPECIFIC protocol must be a string".to_string()) };
                        let host = match host_val { Value::Str(s) => s, _ => return Err("TIMEOUT_SET_SPECIFIC host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TIMEOUT_SET_SPECIFIC port must be a number".to_string()) };
                        let connect_ms: u64 = match connect_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("TIMEOUT_SET_SPECIFIC connect_ms must be a number".to_string()) };
                        let read_ms: u64 = match read_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("TIMEOUT_SET_SPECIFIC read_ms must be a number".to_string()) };
                        let write_ms: u64 = match write_val { Value::Int(n) => n as u64, Value::Float(f) => f as u64, _ => return Err("TIMEOUT_SET_SPECIFIC write_ms must be a number".to_string()) };
                        
                        let mut mgr = timeout_manager_global().lock().map_err(|_| "Timeout manager poisoned".to_string())?;
                        mgr.set_specific_timeout(&protocol, &host, port, connect_ms, read_ms, write_ms);
                        stack.push(Value::Bool(true));
                    }
                    "timeout_get_info" => {
                        if *arg_count < 1 || *arg_count > 3 { return Err("TIMEOUT_GET_INFO expects 1-3 arguments (protocol, [host], [port])".to_string()); }
                        
                        let (host, port) = if *arg_count >= 3 {
                            let port_val = stack.pop().expect("Expected port for TIMEOUT_GET_INFO");
                            let host_val = stack.pop().expect("Expected host for TIMEOUT_GET_INFO");
                            let host = match host_val { Value::Str(s) => Some(s), _ => return Err("TIMEOUT_GET_INFO host must be a string".to_string()) };
                            let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TIMEOUT_GET_INFO port must be a number".to_string()) };
                            (host, Some(port))
                        } else if *arg_count == 2 {
                            let host_val = stack.pop().expect("Expected host for TIMEOUT_GET_INFO");
                            let host = match host_val { Value::Str(s) => Some(s), _ => return Err("TIMEOUT_GET_INFO host must be a string".to_string()) };
                            (host, None)
                        } else {
                            (None, None)
                        };
                        
                        let protocol_val = stack.pop().expect("Expected protocol for TIMEOUT_GET_INFO");
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("TIMEOUT_GET_INFO protocol must be a string".to_string()) };
                        
                        let mgr = timeout_manager_global().lock().map_err(|_| "Timeout manager poisoned".to_string())?;
                        let info = mgr.get_timeout_info(&protocol, host.as_deref(), port);
                        
                        // Return [connect_ms, read_ms, write_ms, source]
                        stack.push(Value::Box(vec![
                            Value::Int(info.timeouts.connect_timeout.as_millis() as i64),
                            Value::Int(info.timeouts.read_timeout.as_millis() as i64),
                            Value::Int(info.timeouts.write_timeout.as_millis() as i64),
                            Value::Str(info.source),
                        ]));
                    }
                    "timeout_remove" => {
                        if *arg_count != 3 { return Err("TIMEOUT_REMOVE expects exactly 3 arguments (protocol, host, port)".to_string()); }
                        let port_val = stack.pop().expect("Expected port for TIMEOUT_REMOVE");
                        let host_val = stack.pop().expect("Expected host for TIMEOUT_REMOVE");
                        let protocol_val = stack.pop().expect("Expected protocol for TIMEOUT_REMOVE");
                        
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("TIMEOUT_REMOVE protocol must be a string".to_string()) };
                        let host = match host_val { Value::Str(s) => s, _ => return Err("TIMEOUT_REMOVE host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("TIMEOUT_REMOVE port must be a number".to_string()) };
                        
                        let mut mgr = timeout_manager_global().lock().map_err(|_| "Timeout manager poisoned".to_string())?;
                        let removed = mgr.remove_specific_timeout(&protocol, &host, port);
                        stack.push(Value::Bool(removed));
                    }
                    "timeout_clear" => {
                        if *arg_count != 0 { return Err("TIMEOUT_CLEAR expects no arguments".to_string()); }
                        let mut mgr = timeout_manager_global().lock().map_err(|_| "Timeout manager poisoned".to_string())?;
                        mgr.clear_specific_timeouts();
                        stack.push(Value::Bool(true));
                    }
                    "timeout_summary" => {
                        if *arg_count != 0 { return Err("TIMEOUT_SUMMARY expects no arguments".to_string()); }
                        let mgr = timeout_manager_global().lock().map_err(|_| "Timeout manager poisoned".to_string())?;
                        let summary = mgr.get_all_timeouts();
                        
                        // Return [global_count, specific_count, default_connect_ms, default_read_ms, default_write_ms]
                        stack.push(Value::Box(vec![
                            Value::Int(summary.global_count as i64),
                            Value::Int(summary.specific_count as i64),
                            Value::Int(summary.default_timeouts.connect_timeout.as_millis() as i64),
                            Value::Int(summary.default_timeouts.read_timeout.as_millis() as i64),
                            Value::Int(summary.default_timeouts.write_timeout.as_millis() as i64),
                        ]));
                    }
                    
                    // Proxy Management built-ins (top-level)
                    "proxy_set_global" => {
                        if *arg_count < 4 || *arg_count > 6 { return Err("PROXY_SET_GLOBAL expects 4-6 arguments (protocol, proxy_type, host, port, [username], [password])".to_string()); }
                        
                        let (username, password) = if *arg_count >= 6 {
                            let pass_val = stack.pop().expect("Expected password for PROXY_SET_GLOBAL");
                            let user_val = stack.pop().expect("Expected username for PROXY_SET_GLOBAL");
                            let password = match pass_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_GLOBAL password must be a string".to_string()) };
                            let username = match user_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_GLOBAL username must be a string".to_string()) };
                            (username, password)
                        } else if *arg_count == 5 {
                            let user_val = stack.pop().expect("Expected username for PROXY_SET_GLOBAL");
                            let username = match user_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_GLOBAL username must be a string".to_string()) };
                            (username, None)
                        } else {
                            (None, None)
                        };
                        
                        let port_val = stack.pop().expect("Expected port for PROXY_SET_GLOBAL");
                        let host_val = stack.pop().expect("Expected host for PROXY_SET_GLOBAL");
                        let proxy_type_val = stack.pop().expect("Expected proxy_type for PROXY_SET_GLOBAL");
                        let protocol_val = stack.pop().expect("Expected protocol for PROXY_SET_GLOBAL");
                        
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("PROXY_SET_GLOBAL protocol must be a string".to_string()) };
                        let proxy_type = match proxy_type_val { Value::Str(s) => s, _ => return Err("PROXY_SET_GLOBAL proxy_type must be a string".to_string()) };
                        let host = match host_val { Value::Str(s) => s, _ => return Err("PROXY_SET_GLOBAL host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("PROXY_SET_GLOBAL port must be a number".to_string()) };
                        
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        match mgr.set_global_proxy(&protocol, &proxy_type, &host, port, username, password) {
                            Ok(()) => stack.push(Value::Bool(true)),
                            Err(e) => return Err(e),
                        }
                    }
                    "proxy_set_specific" => {
                        if *arg_count < 6 || *arg_count > 8 { return Err("PROXY_SET_SPECIFIC expects 6-8 arguments (protocol, target_host, target_port, proxy_type, proxy_host, proxy_port, [username], [password])".to_string()); }
                        
                        let (username, password) = if *arg_count >= 8 {
                            let pass_val = stack.pop().expect("Expected password for PROXY_SET_SPECIFIC");
                            let user_val = stack.pop().expect("Expected username for PROXY_SET_SPECIFIC");
                            let password = match pass_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_SPECIFIC password must be a string".to_string()) };
                            let username = match user_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_SPECIFIC username must be a string".to_string()) };
                            (username, password)
                        } else if *arg_count == 7 {
                            let user_val = stack.pop().expect("Expected username for PROXY_SET_SPECIFIC");
                            let username = match user_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_SPECIFIC username must be a string".to_string()) };
                            (username, None)
                        } else {
                            (None, None)
                        };
                        
                        let proxy_port_val = stack.pop().expect("Expected proxy_port for PROXY_SET_SPECIFIC");
                        let proxy_host_val = stack.pop().expect("Expected proxy_host for PROXY_SET_SPECIFIC");
                        let proxy_type_val = stack.pop().expect("Expected proxy_type for PROXY_SET_SPECIFIC");
                        let target_port_val = stack.pop().expect("Expected target_port for PROXY_SET_SPECIFIC");
                        let target_host_val = stack.pop().expect("Expected target_host for PROXY_SET_SPECIFIC");
                        let protocol_val = stack.pop().expect("Expected protocol for PROXY_SET_SPECIFIC");
                        
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("PROXY_SET_SPECIFIC protocol must be a string".to_string()) };
                        let target_host = match target_host_val { Value::Str(s) => s, _ => return Err("PROXY_SET_SPECIFIC target_host must be a string".to_string()) };
                        let target_port: u16 = match target_port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("PROXY_SET_SPECIFIC target_port must be a number".to_string()) };
                        let proxy_type = match proxy_type_val { Value::Str(s) => s, _ => return Err("PROXY_SET_SPECIFIC proxy_type must be a string".to_string()) };
                        let proxy_host = match proxy_host_val { Value::Str(s) => s, _ => return Err("PROXY_SET_SPECIFIC proxy_host must be a string".to_string()) };
                        let proxy_port: u16 = match proxy_port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("PROXY_SET_SPECIFIC proxy_port must be a number".to_string()) };
                        
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        match mgr.set_specific_proxy(&protocol, &target_host, target_port, &proxy_type, &proxy_host, proxy_port, username, password) {
                            Ok(()) => stack.push(Value::Bool(true)),
                            Err(e) => return Err(e),
                        }
                    }
                    "proxy_set_default" => {
                        if *arg_count < 3 || *arg_count > 5 { return Err("PROXY_SET_DEFAULT expects 3-5 arguments (proxy_type, host, port, [username], [password])".to_string()); }
                        
                        let (username, password) = if *arg_count >= 5 {
                            let pass_val = stack.pop().expect("Expected password for PROXY_SET_DEFAULT");
                            let user_val = stack.pop().expect("Expected username for PROXY_SET_DEFAULT");
                            let password = match pass_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_DEFAULT password must be a string".to_string()) };
                            let username = match user_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_DEFAULT username must be a string".to_string()) };
                            (username, password)
                        } else if *arg_count == 4 {
                            let user_val = stack.pop().expect("Expected username for PROXY_SET_DEFAULT");
                            let username = match user_val { Value::Str(s) => Some(s), _ => return Err("PROXY_SET_DEFAULT username must be a string".to_string()) };
                            (username, None)
                        } else {
                            (None, None)
                        };
                        
                        let port_val = stack.pop().expect("Expected port for PROXY_SET_DEFAULT");
                        let host_val = stack.pop().expect("Expected host for PROXY_SET_DEFAULT");
                        let proxy_type_val = stack.pop().expect("Expected proxy_type for PROXY_SET_DEFAULT");
                        
                        let proxy_type = match proxy_type_val { Value::Str(s) => s, _ => return Err("PROXY_SET_DEFAULT proxy_type must be a string".to_string()) };
                        let host = match host_val { Value::Str(s) => s, _ => return Err("PROXY_SET_DEFAULT host must be a string".to_string()) };
                        let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("PROXY_SET_DEFAULT port must be a number".to_string()) };
                        
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        match mgr.set_default_proxy(&proxy_type, &host, port, username, password) {
                            Ok(()) => stack.push(Value::Bool(true)),
                            Err(e) => return Err(e),
                        }
                    }
                    "proxy_get_info" => {
                        if *arg_count < 1 || *arg_count > 3 { return Err("PROXY_GET_INFO expects 1-3 arguments (protocol, [target_host], [target_port])".to_string()); }
                        
                        let (target_host, target_port) = if *arg_count >= 3 {
                            let port_val = stack.pop().expect("Expected target_port for PROXY_GET_INFO");
                            let host_val = stack.pop().expect("Expected target_host for PROXY_GET_INFO");
                            let host = match host_val { Value::Str(s) => Some(s), _ => return Err("PROXY_GET_INFO target_host must be a string".to_string()) };
                            let port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("PROXY_GET_INFO target_port must be a number".to_string()) };
                            (host, Some(port))
                        } else if *arg_count == 2 {
                            let host_val = stack.pop().expect("Expected target_host for PROXY_GET_INFO");
                            let host = match host_val { Value::Str(s) => Some(s), _ => return Err("PROXY_GET_INFO target_host must be a string".to_string()) };
                            (host, None)
                        } else {
                            (None, None)
                        };
                        
                        let protocol_val = stack.pop().expect("Expected protocol for PROXY_GET_INFO");
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("PROXY_GET_INFO protocol must be a string".to_string()) };
                        
                        let mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        let info = mgr.get_proxy_info(&protocol, target_host.as_deref(), target_port);
                        
                        if let Some(config) = info.proxy_config {
                            // Return [proxy_type, host, port, username, source]
                            let username = config.username.unwrap_or_else(|| "".to_string());
                            stack.push(Value::Box(vec![
                                Value::Str(config.proxy_type.to_string()),
                                Value::Str(config.host),
                                Value::Int(config.port as i64),
                                Value::Str(username),
                                Value::Str(info.source),
                            ]));
                        } else {
                            stack.push(Value::None);
                        }
                    }
                    "proxy_remove_global" => {
                        if *arg_count != 1 { return Err("PROXY_REMOVE_GLOBAL expects exactly 1 argument (protocol)".to_string()); }
                        let protocol_val = stack.pop().expect("Expected protocol for PROXY_REMOVE_GLOBAL");
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("PROXY_REMOVE_GLOBAL protocol must be a string".to_string()) };
                        
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        let removed = mgr.remove_global_proxy(&protocol);
                        stack.push(Value::Bool(removed));
                    }
                    "proxy_remove_specific" => {
                        if *arg_count != 3 { return Err("PROXY_REMOVE_SPECIFIC expects exactly 3 arguments (protocol, target_host, target_port)".to_string()); }
                        let port_val = stack.pop().expect("Expected target_port for PROXY_REMOVE_SPECIFIC");
                        let host_val = stack.pop().expect("Expected target_host for PROXY_REMOVE_SPECIFIC");
                        let protocol_val = stack.pop().expect("Expected protocol for PROXY_REMOVE_SPECIFIC");
                        
                        let protocol = match protocol_val { Value::Str(s) => s, _ => return Err("PROXY_REMOVE_SPECIFIC protocol must be a string".to_string()) };
                        let target_host = match host_val { Value::Str(s) => s, _ => return Err("PROXY_REMOVE_SPECIFIC target_host must be a string".to_string()) };
                        let target_port: u16 = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("PROXY_REMOVE_SPECIFIC target_port must be a number".to_string()) };
                        
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        let removed = mgr.remove_specific_proxy(&protocol, &target_host, target_port);
                        stack.push(Value::Bool(removed));
                    }
                    "proxy_clear_all" => {
                        if *arg_count != 0 { return Err("PROXY_CLEAR_ALL expects no arguments".to_string()); }
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        mgr.clear_all();
                        stack.push(Value::Bool(true));
                    }
                    "proxy_add_bypass" => {
                        if *arg_count != 1 { return Err("PROXY_ADD_BYPASS expects exactly 1 argument (host)".to_string()); }
                        let host_val = stack.pop().expect("Expected host for PROXY_ADD_BYPASS");
                        let host = match host_val { Value::Str(s) => s, _ => return Err("PROXY_ADD_BYPASS host must be a string".to_string()) };
                        
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        mgr.add_bypass(&host);
                        stack.push(Value::Bool(true));
                    }
                    "proxy_remove_bypass" => {
                        if *arg_count != 1 { return Err("PROXY_REMOVE_BYPASS expects exactly 1 argument (host)".to_string()); }
                        let host_val = stack.pop().expect("Expected host for PROXY_REMOVE_BYPASS");
                        let host = match host_val { Value::Str(s) => s, _ => return Err("PROXY_REMOVE_BYPASS host must be a string".to_string()) };
                        
                        let mut mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        let removed = mgr.remove_bypass(&host);
                        stack.push(Value::Bool(removed));
                    }
                    "proxy_get_bypass_list" => {
                        if *arg_count != 0 { return Err("PROXY_GET_BYPASS_LIST expects no arguments".to_string()); }
                        let mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        let bypass_list = mgr.get_bypass_list();
                        
                        let bypass_values: Vec<Value> = bypass_list.into_iter().map(Value::Str).collect();
                        stack.push(Value::Box(bypass_values));
                    }
                    "proxy_stats" => {
                        if *arg_count != 0 { return Err("PROXY_STATS expects no arguments".to_string()); }
                        let mgr = proxy_manager_global().lock().map_err(|_| "Proxy manager poisoned".to_string())?;
                        let stats = mgr.get_stats();
                        
                        // Return [global_count, specific_count, has_default, bypass_count]
                        stack.push(Value::Box(vec![
                            Value::Int(stats.global_count as i64),
                            Value::Int(stats.specific_count as i64),
                            Value::Bool(stats.has_default),
                            Value::Int(stats.bypass_count as i64),
                        ]));
                    }
                    
                    // Raw Socket built-ins (top-level)
                    "raw_socket_create" => {
                        if *arg_count != 1 { return Err("RAW_SOCKET_CREATE expects exactly 1 argument (protocol)".to_string()); }
                        let protocol_val = stack.pop().expect("Expected protocol for RAW_SOCKET_CREATE");
                        let protocol: u8 = match protocol_val { 
                            Value::Int(n) => n as u8, 
                            Value::Float(f) => f as u8, 
                            _ => return Err("RAW_SOCKET_CREATE protocol must be a number".to_string()) 
                        };
                        
                        let mut mgr = raw_manager_global().lock().map_err(|_| "Raw socket manager poisoned".to_string())?;
                        match mgr.create_raw_socket(protocol) {
                            Ok(id) => stack.push(Value::RawSocket(id)),
                            Err(e) => {
                                // Return error string instead of failing - raw sockets need root
                                stack.push(Value::Str(format!("Error: {}", e)));
                            }
                        }
                    }
                    "raw_socket_set_header_included" => {
                        if *arg_count != 2 { return Err("RAW_SOCKET_SET_HEADER_INCLUDED expects exactly 2 arguments (socket, included)".to_string()); }
                        let included_val = stack.pop().expect("Expected included for RAW_SOCKET_SET_HEADER_INCLUDED");
                        let socket_val = stack.pop().expect("Expected socket for RAW_SOCKET_SET_HEADER_INCLUDED");
                        
                        let included = match included_val { Value::Bool(b) => b, _ => return Err("RAW_SOCKET_SET_HEADER_INCLUDED included must be a boolean".to_string()) };
                        let socket_id = match socket_val { Value::RawSocket(id) => id, _ => return Err("RAW_SOCKET_SET_HEADER_INCLUDED socket must be a RawSocket".to_string()) };
                        
                        let mut mgr = raw_manager_global().lock().map_err(|_| "Raw socket manager poisoned".to_string())?;
                        match mgr.set_header_included(socket_id, included) {
                            Ok(()) => stack.push(Value::Bool(true)),
                            Err(e) => return Err(e),
                        }
                    }
                    "raw_socket_send" => {
                        if *arg_count != 3 { return Err("RAW_SOCKET_SEND expects exactly 3 arguments (socket, data, target)".to_string()); }
                        let target_val = stack.pop().expect("Expected target for RAW_SOCKET_SEND");
                        let data_val = stack.pop().expect("Expected data for RAW_SOCKET_SEND");
                        let socket_val = stack.pop().expect("Expected socket for RAW_SOCKET_SEND");
                        
                        let target = match target_val { Value::Str(s) => s, _ => return Err("RAW_SOCKET_SEND target must be a string".to_string()) };
                        let data = match data_val { Value::Str(s) => s.into_bytes(), _ => return Err("RAW_SOCKET_SEND data must be a string".to_string()) };
                        let socket_id = match socket_val { Value::RawSocket(id) => id, _ => return Err("RAW_SOCKET_SEND socket must be a RawSocket".to_string()) };
                        
                        let mut mgr = raw_manager_global().lock().map_err(|_| "Raw socket manager poisoned".to_string())?;
                        match mgr.send_raw(socket_id, &data, &target) {
                            Ok(bytes_sent) => stack.push(Value::Int(bytes_sent as i64)),
                            Err(e) => return Err(e),
                        }
                    }
                    "raw_socket_receive" => {
                        if *arg_count != 2 { return Err("RAW_SOCKET_RECEIVE expects exactly 2 arguments (socket, max_bytes)".to_string()); }
                        let max_bytes_val = stack.pop().expect("Expected max_bytes for RAW_SOCKET_RECEIVE");
                        let socket_val = stack.pop().expect("Expected socket for RAW_SOCKET_RECEIVE");
                        
                        let max_bytes: usize = match max_bytes_val { 
                            Value::Int(n) => n as usize, 
                            Value::Float(f) => f as usize, 
                            _ => return Err("RAW_SOCKET_RECEIVE max_bytes must be a number".to_string()) 
                        };
                        let socket_id = match socket_val { Value::RawSocket(id) => id, _ => return Err("RAW_SOCKET_RECEIVE socket must be a RawSocket".to_string()) };
                        
                        let mut mgr = raw_manager_global().lock().map_err(|_| "Raw socket manager poisoned".to_string())?;
                        match mgr.receive_raw(socket_id, max_bytes) {
                            Ok((data, source)) => {
                                let data_str = String::from_utf8_lossy(&data).to_string();
                                stack.push(Value::Box(vec![Value::Str(data_str), Value::Str(source)]));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "raw_socket_close" => {
                        if *arg_count != 1 { return Err("RAW_SOCKET_CLOSE expects exactly 1 argument (socket)".to_string()); }
                        let socket_val = stack.pop().expect("Expected socket for RAW_SOCKET_CLOSE");
                        let socket_id = match socket_val { Value::RawSocket(id) => id, _ => return Err("RAW_SOCKET_CLOSE socket must be a RawSocket".to_string()) };
                        
                        let mut mgr = raw_manager_global().lock().map_err(|_| "Raw socket manager poisoned".to_string())?;
                        match mgr.close_raw(socket_id) {
                            Ok(()) => stack.push(Value::Bool(true)),
                            Err(e) => return Err(e),
                        }
                    }
                    "raw_socket_info" => {
                        if *arg_count != 1 { return Err("RAW_SOCKET_INFO expects exactly 1 argument (socket)".to_string()); }
                        let socket_val = stack.pop().expect("Expected socket for RAW_SOCKET_INFO");
                        let socket_id = match socket_val { Value::RawSocket(id) => id, _ => return Err("RAW_SOCKET_INFO socket must be a RawSocket".to_string()) };
                        
                        let mgr = raw_manager_global().lock().map_err(|_| "Raw socket manager poisoned".to_string())?;
                        if let Some((protocol, header_included)) = mgr.get_socket_info(socket_id) {
                            stack.push(Value::Box(vec![
                                Value::Int(protocol.to_u8() as i64),
                                Value::Str(protocol.name()),
                                Value::Bool(header_included),
                            ]));
                        } else {
                            return Err("Raw socket not found".to_string());
                        }
                    }
                    "packet_build_icmp_echo" => {
                        if *arg_count != 3 { return Err("PACKET_BUILD_ICMP_ECHO expects exactly 3 arguments (id, sequence, data)".to_string()); }
                        let data_val = stack.pop().expect("Expected data for PACKET_BUILD_ICMP_ECHO");
                        let sequence_val = stack.pop().expect("Expected sequence for PACKET_BUILD_ICMP_ECHO");
                        let id_val = stack.pop().expect("Expected id for PACKET_BUILD_ICMP_ECHO");
                        
                        let data = match data_val { Value::Str(s) => s.into_bytes(), _ => return Err("PACKET_BUILD_ICMP_ECHO data must be a string".to_string()) };
                        let sequence: u16 = match sequence_val { 
                            Value::Int(n) => n as u16, 
                            Value::Float(f) => f as u16, 
                            _ => return Err("PACKET_BUILD_ICMP_ECHO sequence must be a number".to_string()) 
                        };
                        let id: u16 = match id_val { 
                            Value::Int(n) => n as u16, 
                            Value::Float(f) => f as u16, 
                            _ => return Err("PACKET_BUILD_ICMP_ECHO id must be a number".to_string()) 
                        };
                        
                        use crate::raw_socket_manager::PacketBuilder;
                        let packet = PacketBuilder::icmp_echo_request(id, sequence, &data);
                        let packet_str = String::from_utf8_lossy(&packet).to_string();
                        stack.push(Value::Str(packet_str));
                    }
                    "packet_build_ipv4_header" => {
                        if *arg_count != 4 { return Err("PACKET_BUILD_IPV4_HEADER expects exactly 4 arguments (source, dest, protocol, data_len)".to_string()); }
                        let data_len_val = stack.pop().expect("Expected data_len for PACKET_BUILD_IPV4_HEADER");
                        let protocol_val = stack.pop().expect("Expected protocol for PACKET_BUILD_IPV4_HEADER");
                        let dest_val = stack.pop().expect("Expected dest for PACKET_BUILD_IPV4_HEADER");
                        let source_val = stack.pop().expect("Expected source for PACKET_BUILD_IPV4_HEADER");
                        
                        let data_len: u16 = match data_len_val { 
                            Value::Int(n) => n as u16, 
                            Value::Float(f) => f as u16, 
                            _ => return Err("PACKET_BUILD_IPV4_HEADER data_len must be a number".to_string()) 
                        };
                        let protocol: u8 = match protocol_val { 
                            Value::Int(n) => n as u8, 
                            Value::Float(f) => f as u8, 
                            _ => return Err("PACKET_BUILD_IPV4_HEADER protocol must be a number".to_string()) 
                        };
                        let dest = match dest_val { Value::Str(s) => s, _ => return Err("PACKET_BUILD_IPV4_HEADER dest must be a string".to_string()) };
                        let source = match source_val { Value::Str(s) => s, _ => return Err("PACKET_BUILD_IPV4_HEADER source must be a string".to_string()) };
                        
                        use crate::raw_socket_manager::PacketBuilder;
                        match PacketBuilder::ipv4_header(&source, &dest, protocol, data_len) {
                            Ok(header) => {
                                let header_str = String::from_utf8_lossy(&header).to_string();
                                stack.push(Value::Str(header_str));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "packet_calculate_checksum" => {
                        if *arg_count != 1 { return Err("PACKET_CALCULATE_CHECKSUM expects exactly 1 argument (data)".to_string()); }
                        let data_val = stack.pop().expect("Expected data for PACKET_CALCULATE_CHECKSUM");
                        let data = match data_val { Value::Str(s) => s.into_bytes(), _ => return Err("PACKET_CALCULATE_CHECKSUM data must be a string".to_string()) };
                        
                        use crate::raw_socket_manager::PacketBuilder;
                        let checksum = PacketBuilder::calculate_checksum(&data);
                        stack.push(Value::Int(checksum as i64));
                    }
                    
                    // Network Interface built-ins (top-level)
                    "get_interfaces" => {
                        if *arg_count != 0 { return Err("GET_INTERFACES expects no arguments".to_string()); }
                        let mut mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        match mgr.enumerate_interfaces() {
                            Ok(interface_names) => {
                                let interface_values: Vec<Value> = interface_names.into_iter().map(Value::Str).collect();
                                stack.push(Value::Box(interface_values));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "get_interface_info" => {
                        if *arg_count != 1 { return Err("GET_INTERFACE_INFO expects exactly 1 argument (interface_name)".to_string()); }
                        let name_val = stack.pop().expect("Expected interface_name for GET_INTERFACE_INFO");
                        let name = match name_val { Value::Str(s) => s, _ => return Err("GET_INTERFACE_INFO interface_name must be a string".to_string()) };
                        
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        if let Some(interface) = mgr.get_interface(&name) {
                            let mut addresses = Vec::new();
                            for addr in &interface.addresses {
                                let mut addr_info = Vec::new();
                                addr_info.push(Value::Str(addr.ip.to_string()));
                                addr_info.push(Value::Str(addr.netmask.to_string()));
                                if let Some(broadcast) = &addr.broadcast {
                                    addr_info.push(Value::Str(broadcast.to_string()));
                                } else {
                                    addr_info.push(Value::Str("None".to_string()));
                                }
                                addresses.push(Value::Box(addr_info));
                            }
                            
                            stack.push(Value::Box(vec![
                                Value::Str(interface.name.clone()),
                                Value::Str(interface.interface_type.to_string()),
                                Value::Bool(interface.is_up),
                                Value::Bool(interface.is_loopback),
                                Value::Bool(interface.is_multicast),
                                Value::Int(interface.mtu as i64),
                                Value::Box(addresses),
                                Value::Str(interface.mac_address.clone().unwrap_or_else(|| "Unknown".to_string())),
                            ]));
                        } else {
                            return Err(format!("Interface '{}' not found", name));
                        }
                    }
                    "get_interface_stats" => {
                        if *arg_count != 1 { return Err("GET_INTERFACE_STATS expects exactly 1 argument (interface_name)".to_string()); }
                        let name_val = stack.pop().expect("Expected interface_name for GET_INTERFACE_STATS");
                        let name = match name_val { Value::Str(s) => s, _ => return Err("GET_INTERFACE_STATS interface_name must be a string".to_string()) };
                        
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        if let Some(stats) = mgr.get_interface_stats(&name) {
                            stack.push(Value::Box(vec![
                                Value::Str(stats.name),
                                Value::Bool(stats.is_up),
                                Value::Int(stats.mtu as i64),
                                Value::Int(stats.address_count as i64),
                                Value::Bool(stats.has_ipv4),
                                Value::Bool(stats.has_ipv6),
                            ]));
                        } else {
                            return Err(format!("Interface '{}' not found", name));
                        }
                    }
                    "get_primary_interface" => {
                        if *arg_count != 0 { return Err("GET_PRIMARY_INTERFACE expects no arguments".to_string()); }
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        if let Some(interface) = mgr.get_primary_interface() {
                            stack.push(Value::Str(interface.name.clone()));
                        } else {
                            stack.push(Value::None);
                        }
                    }
                    "get_loopback_interface" => {
                        if *arg_count != 0 { return Err("GET_LOOPBACK_INTERFACE expects no arguments".to_string()); }
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        if let Some(interface) = mgr.get_loopback_interface() {
                            stack.push(Value::Str(interface.name.clone()));
                        } else {
                            stack.push(Value::None);
                        }
                    }
                    "get_interfaces_by_type" => {
                        if *arg_count != 1 { return Err("GET_INTERFACES_BY_TYPE expects exactly 1 argument (type)".to_string()); }
                        let type_val = stack.pop().expect("Expected type for GET_INTERFACES_BY_TYPE");
                        let type_str = match type_val { Value::Str(s) => s, _ => return Err("GET_INTERFACES_BY_TYPE type must be a string".to_string()) };
                        
                        use crate::network_interface_manager::InterfaceType;
                        let interface_type = match type_str.to_lowercase().as_str() {
                            "ethernet" => InterfaceType::Ethernet,
                            "wireless" => InterfaceType::Wireless,
                            "loopback" => InterfaceType::Loopback,
                            "tunnel" => InterfaceType::Tunnel,
                            "virtual" => InterfaceType::Virtual,
                            "unknown" => InterfaceType::Unknown,
                            _ => return Err("GET_INTERFACES_BY_TYPE type must be one of: ethernet, wireless, loopback, tunnel, virtual, unknown".to_string()),
                        };
                        
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        let interfaces = mgr.get_interfaces_by_type(interface_type);
                        let interface_names: Vec<Value> = interfaces.into_iter().map(|iface| Value::Str(iface.name.clone())).collect();
                        stack.push(Value::Box(interface_names));
                    }
                    "get_up_interfaces" => {
                        if *arg_count != 0 { return Err("GET_UP_INTERFACES expects no arguments".to_string()); }
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        let interfaces = mgr.get_up_interfaces();
                        let interface_names: Vec<Value> = interfaces.into_iter().map(|iface| Value::Str(iface.name.clone())).collect();
                        stack.push(Value::Box(interface_names));
                    }
                    "get_interface_by_ip" => {
                        if *arg_count != 1 { return Err("GET_INTERFACE_BY_IP expects exactly 1 argument (ip_address)".to_string()); }
                        let ip_val = stack.pop().expect("Expected ip_address for GET_INTERFACE_BY_IP");
                        let ip_str = match ip_val { Value::Str(s) => s, _ => return Err("GET_INTERFACE_BY_IP ip_address must be a string".to_string()) };
                        
                        let target_ip: std::net::IpAddr = ip_str.parse()
                            .map_err(|_| format!("Invalid IP address: {}", ip_str))?;
                        
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        let all_interfaces: Vec<_> = mgr.interfaces.values().collect();
                        if let Some(interface) = get_interface_by_ip(&all_interfaces, target_ip) {
                            stack.push(Value::Str(interface.name.clone()));
                        } else {
                            stack.push(Value::None);
                        }
                    }
                    "get_best_interface" => {
                        if *arg_count != 1 { return Err("GET_BEST_INTERFACE expects exactly 1 argument (prefer_ipv4)".to_string()); }
                        let prefer_ipv4_val = stack.pop().expect("Expected prefer_ipv4 for GET_BEST_INTERFACE");
                        let prefer_ipv4 = match prefer_ipv4_val { Value::Bool(b) => b, _ => return Err("GET_BEST_INTERFACE prefer_ipv4 must be a boolean".to_string()) };
                        
                        let mgr = interface_manager_global().lock().map_err(|_| "Interface manager poisoned".to_string())?;
                        let all_interfaces: Vec<_> = mgr.interfaces.values().collect();
                        if let Some(interface) = get_best_interface_for_binding(&all_interfaces, prefer_ipv4) {
                            stack.push(Value::Str(interface.name.clone()));
                        } else {
                            stack.push(Value::None);
                        }
                    }
                    
                    // IPv6 and Dual-Stack built-ins (top-level)
                    "ipv6_set_dual_stack_mode" => {
                        if *arg_count != 1 { return Err("IPV6_SET_DUAL_STACK_MODE expects exactly 1 argument (mode)".to_string()); }
                        let mode_val = stack.pop().expect("Expected mode for IPV6_SET_DUAL_STACK_MODE");
                        let mode_str = match mode_val { Value::Str(s) => s, _ => return Err("IPV6_SET_DUAL_STACK_MODE mode must be a string".to_string()) };
                        
                        let mode = DualStackMode::from_string(&mode_str)
                            .map_err(|e| e)?;
                        
                        let mut mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        mgr.set_dual_stack_mode(mode);
                        stack.push(Value::Bool(true));
                    }
                    "ipv6_get_dual_stack_mode" => {
                        if *arg_count != 0 { return Err("IPV6_GET_DUAL_STACK_MODE expects no arguments".to_string()); }
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        let mode = mgr.get_dual_stack_mode();
                        stack.push(Value::Str(mode.to_string()));
                    }
                    "ipv6_resolve_dual_stack" => {
                        if *arg_count != 1 { return Err("IPV6_RESOLVE_DUAL_STACK expects exactly 1 argument (hostname)".to_string()); }
                        let hostname_val = stack.pop().expect("Expected hostname for IPV6_RESOLVE_DUAL_STACK");
                        let hostname = match hostname_val { Value::Str(s) => s, _ => return Err("IPV6_RESOLVE_DUAL_STACK hostname must be a string".to_string()) };
                        
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        match mgr.resolve_dual_stack(&hostname) {
                            Ok(result) => {
                                let mut addresses = Vec::new();
                                if let Some(ipv4) = result.ipv4_address {
                                    addresses.push(Value::Str(ipv4.to_string()));
                                } else {
                                    addresses.push(Value::None);
                                }
                                if let Some(ipv6) = result.ipv6_address {
                                    addresses.push(Value::Str(ipv6.to_string()));
                                } else {
                                    addresses.push(Value::None);
                                }
                                
                                stack.push(Value::Box(vec![
                                    Value::Box(addresses),
                                    Value::Str(result.preferred_address.to_string()),
                                    Value::Str(result.preferred_family),
                                ]));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "ipv6_parse_address" => {
                        if *arg_count != 1 { return Err("IPV6_PARSE_ADDRESS expects exactly 1 argument (address)".to_string()); }
                        let addr_val = stack.pop().expect("Expected address for IPV6_PARSE_ADDRESS");
                        let addr_str = match addr_val { Value::Str(s) => s, _ => return Err("IPV6_PARSE_ADDRESS address must be a string".to_string()) };
                        
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        match mgr.parse_ipv6_address(&addr_str) {
                            Ok(ipv6_addr) => {
                                stack.push(Value::Box(vec![
                                    Value::Str(ipv6_addr.to_string()),
                                    Value::Int(ipv6_addr.scope_id.unwrap_or(0) as i64),
                                    Value::Bool(ipv6_addr.is_link_local),
                                    Value::Bool(ipv6_addr.is_site_local),
                                    Value::Bool(ipv6_addr.is_unique_local),
                                    Value::Bool(ipv6_addr.is_multicast),
                                    Value::Bool(ipv6_addr.is_loopback),
                                    Value::Bool(ipv6_addr.is_unspecified),
                                    Value::Bool(ipv6_addr.is_global()),
                                ]));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "ipv6_get_multicast_address" => {
                        if *arg_count != 1 { return Err("IPV6_GET_MULTICAST_ADDRESS expects exactly 1 argument (group)".to_string()); }
                        let group_val = stack.pop().expect("Expected group for IPV6_GET_MULTICAST_ADDRESS");
                        let group = match group_val { Value::Str(s) => s, _ => return Err("IPV6_GET_MULTICAST_ADDRESS group must be a string".to_string()) };
                        
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        match mgr.get_ipv6_multicast_address(&group) {
                            Ok(ipv6_addr) => {
                                stack.push(Value::Box(vec![
                                    Value::Str(ipv6_addr.to_string()),
                                    Value::Int(ipv6_addr.scope_id.unwrap_or(0) as i64),
                                    Value::Bool(ipv6_addr.is_multicast),
                                ]));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "ipv6_is_ipv6_address" => {
                        if *arg_count != 1 { return Err("IPV6_IS_IPV6_ADDRESS expects exactly 1 argument (address)".to_string()); }
                        let addr_val = stack.pop().expect("Expected address for IPV6_IS_IPV6_ADDRESS");
                        let addr_str = match addr_val { Value::Str(s) => s, _ => return Err("IPV6_IS_IPV6_ADDRESS address must be a string".to_string()) };
                        
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        let is_ipv6 = mgr.is_ipv6_address(&addr_str);
                        stack.push(Value::Bool(is_ipv6));
                    }
                    "ipv6_is_ipv4_address" => {
                        if *arg_count != 1 { return Err("IPV6_IS_IPV4_ADDRESS expects exactly 1 argument (address)".to_string()); }
                        let addr_val = stack.pop().expect("Expected address for IPV6_IS_IPV4_ADDRESS");
                        let addr_str = match addr_val { Value::Str(s) => s, _ => return Err("IPV6_IS_IPV4_ADDRESS address must be a string".to_string()) };
                        
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        let is_ipv4 = mgr.is_ipv4_address(&addr_str);
                        stack.push(Value::Bool(is_ipv4));
                    }
                    "ipv6_get_address_info" => {
                        if *arg_count != 1 { return Err("IPV6_GET_ADDRESS_INFO expects exactly 1 argument (address)".to_string()); }
                        let addr_val = stack.pop().expect("Expected address for IPV6_GET_ADDRESS_INFO");
                        let addr_str = match addr_val { Value::Str(s) => s, _ => return Err("IPV6_GET_ADDRESS_INFO address must be a string".to_string()) };
                        
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        match mgr.get_ipv6_info(&addr_str) {
                            Ok(ipv6_addr) => {
                                stack.push(Value::Box(vec![
                                    Value::Str(ipv6_addr.to_string()),
                                    Value::Int(ipv6_addr.scope_id.unwrap_or(0) as i64),
                                    Value::Bool(ipv6_addr.is_link_local),
                                    Value::Bool(ipv6_addr.is_site_local),
                                    Value::Bool(ipv6_addr.is_unique_local),
                                    Value::Bool(ipv6_addr.is_multicast),
                                    Value::Bool(ipv6_addr.is_loopback),
                                    Value::Bool(ipv6_addr.is_unspecified),
                                    Value::Bool(ipv6_addr.is_global()),
                                ]));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    "ipv6_get_config" => {
                        if *arg_count != 0 { return Err("IPV6_GET_CONFIG expects no arguments".to_string()); }
                        let mgr = ipv6_manager_global().lock().map_err(|_| "IPv6 manager poisoned".to_string())?;
                        let (mode, ipv4_enabled, ipv6_enabled) = mgr.get_config();
                        stack.push(Value::Box(vec![
                            Value::Str(mode),
                            Value::Bool(ipv4_enabled),
                            Value::Bool(ipv6_enabled),
                        ]));
                    }
                    "ipv6_create_dual_stack_socket" => {
                        if *arg_count != 3 { return Err("IPV6_CREATE_DUAL_STACK_SOCKET expects exactly 3 arguments (host, port, prefer_ipv6)".to_string()); }
                        let prefer_ipv6_val = stack.pop().expect("Expected prefer_ipv6 for IPV6_CREATE_DUAL_STACK_SOCKET");
                        let port_val = stack.pop().expect("Expected port for IPV6_CREATE_DUAL_STACK_SOCKET");
                        let host_val = stack.pop().expect("Expected host for IPV6_CREATE_DUAL_STACK_SOCKET");
                        
                        let host = match host_val { Value::Str(s) => s, _ => return Err("IPV6_CREATE_DUAL_STACK_SOCKET host must be a string".to_string()) };
                        let port = match port_val { Value::Int(n) => n as u16, Value::Float(f) => f as u16, _ => return Err("IPV6_CREATE_DUAL_STACK_SOCKET port must be a number".to_string()) };
                        let prefer_ipv6 = match prefer_ipv6_val { Value::Bool(b) => b, _ => return Err("IPV6_CREATE_DUAL_STACK_SOCKET prefer_ipv6 must be a boolean".to_string()) };
                        
                        match create_dual_stack_socket_addr(&host, port, prefer_ipv6) {
                            Ok(socket_addr) => {
                                stack.push(Value::Box(vec![
                                    Value::Str(socket_addr.to_string()),
                                    Value::Str(if socket_addr.is_ipv4() { "IPv4".to_string() } else { "IPv6".to_string() }),
                                    Value::Int(port as i64),
                                ]));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    
                    // Math built-ins (top-level)
                    "abs" => { if *arg_count != 1 { return Err("ABS expects 1 argument".to_string()); } let x = stack.pop().unwrap(); let v = match x { Value::Int(n) => (n as f64).abs(), Value::Float(f)=> f.abs(), _=> return Err("ABS: type".to_string())}; stack.push(Value::Float(v)); }
                    "sign" => { if *arg_count != 1 { return Err("SIGN expects 1 argument".to_string()); } let x = stack.pop().unwrap(); let f = match x { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("SIGN: type".to_string())}; let s = if f>0.0 {1.0} else if f<0.0 {-1.0} else {0.0}; stack.push(Value::Float(s)); }
                    "floor" => { if *arg_count != 1 { return Err("FLOOR expects 1 argument".to_string()); } let f = match stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("FLOOR: type".to_string())}; stack.push(Value::Float(f.floor())); }
                    "ceil" => { if *arg_count != 1 { return Err("CEIL expects 1 argument".to_string()); } let f = match stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("CEIL: type".to_string())}; stack.push(Value::Float(f.ceil())); }
                    "round" => { if *arg_count != 1 { return Err("ROUND expects 1 argument".to_string()); } let f = match stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ROUND: type".to_string())}; stack.push(Value::Float(f.round())); }
                    "trunc" => { if *arg_count != 1 { return Err("TRUNC expects 1 argument".to_string()); } let f = match stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("TRUNC: type".to_string())}; stack.push(Value::Float(f.trunc())); }
                    "sqrt" => { if *arg_count != 1 { return Err("SQRT expects 1 argument".to_string()); } let f = match stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("SQRT: type".to_string())}; if f<0.0 { return Err("SQRT: domain error".to_string()); } stack.push(Value::Float(f.sqrt())); }
                    "cbrt" => { if *arg_count != 1 { return Err("CBRT expects 1 argument".to_string()); } let f = match stack.pop().unwrap() { Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("CBRT: type".to_string())}; stack.push(Value::Float(f.cbrt())); }
                    "pow" => { if *arg_count != 2 { return Err("POW expects 2 arguments".to_string()); } let y = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("POW: type".to_string())}; let x = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("POW: type".to_string())}; stack.push(Value::Float(x.powf(y))); }
                    "sin" => { if *arg_count != 1 { return Err("SIN expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("SIN: type".to_string())}; stack.push(Value::Float(f.sin())); }
                    "cos" => { if *arg_count != 1 { return Err("COS expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("COS: type".to_string())}; stack.push(Value::Float(f.cos())); }
                    "tan" => { if *arg_count != 1 { return Err("TAN expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("TAN: type".to_string())}; stack.push(Value::Float(f.tan())); }
                    "asin" => { if *arg_count != 1 { return Err("ASIN expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ASIN: type".to_string())}; if f < -1.0 || f > 1.0 { return Err("ASIN: domain error".to_string()); } stack.push(Value::Float(f.asin())); }
                    "acos" => { if *arg_count != 1 { return Err("ACOS expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ACOS: type".to_string())}; if f < -1.0 || f > 1.0 { return Err("ACOS: domain error".to_string()); } stack.push(Value::Float(f.acos())); }
                    "atan" => { if *arg_count != 1 { return Err("ATAN expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ATAN: type".to_string())}; stack.push(Value::Float(f.atan())); }
                    "atan2" => { if *arg_count != 2 { return Err("ATAN2 expects 2 arguments".to_string()); } let x = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ATAN2: type".to_string())}; let y = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("ATAN2: type".to_string())}; stack.push(Value::Float(y.atan2(x))); }
                    "exp" => { if *arg_count != 1 { return Err("EXP expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("EXP: type".to_string())}; stack.push(Value::Float(f.exp())); }
                    "ln" => { if *arg_count != 1 { return Err("LN expects 1 argument".to_string()); } let f = match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=> f, _=> return Err("LN: type".to_string())}; if f<=0.0 { return Err("LN: domain error".to_string()); } stack.push(Value::Float(f.ln())); }
                    "log10" => { if *arg_count != 1 { return Err("LOG10 expects 1 argument".to_string()); } let f=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("LOG10: type".to_string())}; if f<=0.0 { return Err("LOG10: domain error".to_string()); } stack.push(Value::Float(f.log10())); }
                    "log2" => { if *arg_count != 1 { return Err("LOG2 expects 1 argument".to_string()); } let f=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("LOG2: type".to_string())}; if f<=0.0 { return Err("LOG2: domain error".to_string()); } stack.push(Value::Float(f.log2())); }
                    "min" => { if *arg_count != 2 { return Err("MIN expects 2 arguments".to_string()); } let b=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MIN: type".to_string())}; let a=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MIN: type".to_string())}; stack.push(Value::Float(a.min(b))); }
                    "max" => { if *arg_count != 2 { return Err("MAX expects 2 arguments".to_string()); } let b=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MAX: type".to_string())}; let a=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("MAX: type".to_string())}; stack.push(Value::Float(a.max(b))); }
                    "clamp" => { if *arg_count != 3 { return Err("CLAMP expects 3 arguments".to_string()); } let hi=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("CLAMP: type".to_string())}; let lo=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("CLAMP: type".to_string())}; let x=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("CLAMP: type".to_string())}; if lo>hi { return Err("CLAMP: domain error".to_string()); } stack.push(Value::Float(x.max(lo).min(hi))); }
                    "hypot" => { if *arg_count != 2 { return Err("HYPOT expects 2 arguments".to_string()); } let b=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("HYPOT: type".to_string())}; let a=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=>return Err("HYPOT: type".to_string())}; stack.push(Value::Float(a.hypot(b))); }
                    "deg" => { if *arg_count != 1 { return Err("DEG expects 1 argument".to_string()); } let f=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("DEG: type".to_string())}; stack.push(Value::Float(f*180.0/std::f64::consts::PI)); }
                    "rad" => { if *arg_count != 1 { return Err("RAD expects 1 argument".to_string()); } let f=match stack.pop().unwrap(){Value::Int(n)=>n as f64, Value::Float(f)=>f, _=> return Err("RAD: type".to_string())}; stack.push(Value::Float(f*std::f64::consts::PI/180.0)); }
                    // Extra helpers (top-level)
                    "mod" => { if *arg_count != 2 { return Err("MOD expects 2 arguments".to_string()); } let b=stack.pop().unwrap(); let a=stack.pop().unwrap(); match (a,b){(Value::Int(x),Value::Int(y))=>{ if y==0 { return Err("MOD: division by zero".to_string()); } stack.push(Value::Int(x.rem_euclid(y))); }, (Value::Float(x),Value::Float(y))=>{ stack.push(Value::Float(x.rem_euclid(y))); }, (Value::Int(x),Value::Float(y))=>{ stack.push(Value::Float((x as f64).rem_euclid(y))); }, (Value::Float(x),Value::Int(y))=>{ stack.push(Value::Float(x.rem_euclid(y as f64))); }, _=> return Err("MOD: type".to_string()) } }
                    "div" => { if *arg_count != 2 { return Err("DIV expects 2 arguments".to_string()); } let b=stack.pop().unwrap(); let a=stack.pop().unwrap(); match (a,b){(Value::Int(x),Value::Int(y))=>{ if y==0 { return Err("DIV: division by zero".to_string()); } stack.push(Value::Int(x.div_euclid(y))); }, _=> return Err("DIV: int type".to_string()) } }
                    "divmod" => { if *arg_count != 2 { return Err("DIVMOD expects 2 arguments".to_string()); } let b=stack.pop().unwrap(); let a=stack.pop().unwrap(); match (a,b){(Value::Int(x),Value::Int(y))=>{ if y==0 { return Err("DIVMOD: division by zero".to_string()); } let q = x.div_euclid(y); let r = x.rem_euclid(y); stack.push(Value::Box(vec![Value::Int(q), Value::Int(r)])); }, _=> return Err("DIVMOD: int type".to_string()) } }
                    "pow_int" => { if *arg_count != 2 { return Err("POW_INT expects 2 arguments".to_string()); } let n_v=stack.pop().unwrap(); let x_v=stack.pop().unwrap(); let n = match n_v { Value::Int(k)=>k, _=> return Err("POW_INT: exponent must be int".to_string())}; match x_v { Value::Int(x)=>{ if n<0 { stack.push(Value::Float((x as f64).powi(n as i32))); } else { let mut res:i64=1; let mut base=x; let mut exp=n; while exp>0 { if (exp & 1)==1 { res = res.saturating_mul(base); } base = base.saturating_mul(base); exp >>= 1; } stack.push(Value::Int(res)); } }, Value::Float(x)=>{ stack.push(Value::Float(x.powi(n as i32))); }, _=> return Err("POW_INT: base type".to_string()) } }
                    "frac" => { if *arg_count != 1 { return Err("FRAC expects 1 argument".to_string()); } let v=stack.pop().unwrap(); match v { Value::Int(_)=> stack.push(Value::Float(0.0)), Value::Float(f)=> stack.push(Value::Float(f.fract())), _=> return Err("FRAC: type".to_string()) } }
                    "nearly_equal" => { if *arg_count != 3 { return Err("NEARLY_EQUAL expects 3 arguments".to_string()); } let eps=match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("NEARLY_EQUAL: type".to_string())}; let b=match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("NEARLY_EQUAL: type".to_string())}; let a=match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("NEARLY_EQUAL: type".to_string())}; stack.push(Value::Bool((a-b).abs() <= eps)); }
                    "sum" => { if *arg_count != 1 { return Err("SUM expects 1 argument".to_string()); } let xs = stack.pop().unwrap(); match xs { Value::Box(vs)=>{ let mut acc=0.0; for v in vs { match v { Value::Int(n)=> acc += n as f64, Value::Float(f)=> acc += f, _=> return Err("SUM: element type".to_string()) } } stack.push(Value::Float(acc)); }, _=> return Err("SUM: expects box".to_string()) } }
                    "mean" => { 
                        if *arg_count != 1 { return Err("MEAN expects 1 argument".to_string()); } 
                        let xs = stack.pop().unwrap(); 
                        match xs { 
                            Value::Box(vs)=>{ 
                                if vs.is_empty() { return Err("MEAN: empty".to_string()); } 
                                let len = vs.len();
                                let mut acc=0.0; 
                                for v in vs { 
                                    match v { 
                                        Value::Int(n)=> acc += n as f64, 
                                        Value::Float(f)=> acc += f, 
                                        _=> return Err("MEAN: element type".to_string()) 
                                    } 
                                } 
                                stack.push(Value::Float(acc / len as f64)); 
                            }, 
                            _=> return Err("MEAN: expects box".to_string()) 
                        } 
                    }
                    "dot" => { if *arg_count != 2 { return Err("DOT expects 2 arguments".to_string()); } let b=stack.pop().unwrap(); let a=stack.pop().unwrap(); match (a,b){(Value::Box(as_), Value::Box(bs))=>{ if as_.len()!=bs.len(){ return Err("DOT: length mismatch".to_string()); } let mut acc=0.0; for (va,vb) in as_.into_iter().zip(bs.into_iter()) { let fa = match va { Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("DOT: element type".to_string())}; let fb = match vb { Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("DOT: element type".to_string())}; acc += fa*fb; } stack.push(Value::Float(acc)); }, _=> return Err("DOT: expects boxes".to_string()) } }
                    "length" => { if *arg_count != 1 { return Err("LENGTH expects 1 argument".to_string()); } let a=stack.pop().unwrap(); match a { Value::Box(vs)=>{ let mut acc=0.0; for v in vs { let f=match v{Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("LENGTH: element type".to_string())}; acc += f*f; } stack.push(Value::Float(acc.sqrt())); }, _=> return Err("LENGTH: expects box".to_string()) } }
                    "sin_deg" => { if *arg_count != 1 { return Err("SIN_DEG expects 1 argument".to_string()); } let d=match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("SIN_DEG: type".to_string())}; stack.push(Value::Float((d*std::f64::consts::PI/180.0).sin())); }
                    "cos_deg" => { if *arg_count != 1 { return Err("COS_DEG expects 1 argument".to_string()); } let d=match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("COS_DEG: type".to_string())}; stack.push(Value::Float((d*std::f64::consts::PI/180.0).cos())); }
                    "tan_deg" => { if *arg_count != 1 { return Err("TAN_DEG expects 1 argument".to_string()); } let d=match stack.pop().unwrap(){Value::Int(n)=> n as f64, Value::Float(f)=>f, _=> return Err("TAN_DEG: type".to_string())}; stack.push(Value::Float((d*std::f64::consts::PI/180.0).tan())); }
                    "linspace" => { if *arg_count != 3 { return Err("LINSPACE expects 3 arguments".to_string()); } let n_v = stack.pop().unwrap(); let end_v = stack.pop().unwrap(); let start_v = stack.pop().unwrap(); let n = match n_v { Value::Int(k)=>k, _=> return Err("LINSPACE: n must be int".to_string())}; if n < 2 { return Err("LINSPACE: n >= 2".to_string()); } let start = match start_v { Value::Int(x)=> x as f64, Value::Float(f)=>f, _=> return Err("LINSPACE: type".to_string())}; let end = match end_v { Value::Int(x)=> x as f64, Value::Float(f)=>f, _=> return Err("LINSPACE: type".to_string())}; let step = (end-start) / (n-1) as f64; let mut out = Vec::with_capacity(n as usize); let mut i=0; while i < n { out.push(Value::Float(start + step * i as f64)); i += 1; } stack.push(Value::Box(out)); }
                    "range" => { 
                        if *arg_count != 3 { return Err("RANGE expects 3 arguments".to_string()); } 
                        let step_v=stack.pop().unwrap(); 
                        let end_v=stack.pop().unwrap(); 
                        let start_v=stack.pop().unwrap(); 
                        let mut max_iter: i64 = 1_000_000; 
                        match (start_v, end_v, step_v) { 
                            (Value::Int(mut a), Value::Int(b), Value::Int(s))=>{ 
                                if s==0 { return Err("RANGE: step=0".to_string()); } 
                                let mut out=Vec::new(); 
                                if s>0 { 
                                    while a < b && max_iter>0 { 
                                        out.push(Value::Int(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } else { 
                                    while a > b && max_iter>0 { 
                                        out.push(Value::Int(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } 
                                stack.push(Value::Box(out)); 
                            }, 
                            (a_v, b_v, s_v)=>{ 
                                let mut a = match a_v { 
                                    Value::Int(x)=> x as f64, 
                                    Value::Float(f)=>f, 
                                    _=> return Err("RANGE: type".to_string())
                                }; 
                                let b = match b_v { 
                                    Value::Int(x)=> x as f64, 
                                    Value::Float(f)=>f, 
                                    _=> return Err("RANGE: type".to_string())
                                }; 
                                let s = match s_v { 
                                    Value::Int(x)=> x as f64, 
                                    Value::Float(f)=>f, 
                                    _=> return Err("RANGE: type".to_string())
                                }; 
                                if s==0.0 { return Err("RANGE: step=0".to_string()); } 
                                let mut out=Vec::new(); 
                                if s>0.0 { 
                                    while a < b && max_iter>0 { 
                                        out.push(Value::Float(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } else { 
                                    while a > b && max_iter>0 { 
                                        out.push(Value::Float(a)); 
                                        a += s; 
                                        max_iter-=1; 
                                    } 
                                } 
                                stack.push(Value::Box(out)); 
                            } 
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
                        if std::env::var("SCRAPS_DEBUG").is_ok() {
                            println!("DEBUG FISSION: Created box with {} parts: {:?}", parts.len(), parts.iter().map(|v| v.format_for_display()).collect::<Vec<_>>());
                        }
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
                    "unpack" => {
                        if *arg_count != 2 { return Err("UNPACK expects exactly 2 arguments (box, index)".to_string()); }
                        let index_val = stack.pop().expect("Expected index for UNPACK");
                        let box_val = stack.pop().expect("Expected box for UNPACK");
                        
                        let index = match index_val {
                            Value::Int(n) => n,
                            _ => return Err("UNPACK: index must be an integer".to_string()),
                        };
                        
                        match box_val {
                            Value::Box(contents) => {
                                if index < 0 || index >= contents.len() as i64 {
                                    return Err("UNPACK: index out of bounds".to_string());
                                }
                                let extracted = contents[index as usize].clone();
                                if std::env::var("SCRAPS_DEBUG").is_ok() {
                                    println!("DEBUG UNPACK: Extracting index {} from box with {} contents. Box contents: {:?}. Extracted: {:?}", 
                                        index, contents.len(), 
                                        contents.iter().map(|v| v.format_for_display()).collect::<Vec<_>>(),
                                        extracted.format_for_display());
                                }
                                stack.push(extracted);
                            }
                            Value::Str(s) => {
                                if index < 0 || index >= s.len() as i64 {
                                    return Err("UNPACK: index out of bounds".to_string());
                                }
                                let ch = s.chars().nth(index as usize).unwrap();
                                stack.push(Value::Str(ch.to_string()));
                            }
                            _ => return Err("UNPACK: expected box or string".to_string()),
                        }
                    }
                    "count" => {
                        if *arg_count != 1 { return Err("COUNT expects exactly 1 argument".to_string()); }
                        let val = stack.pop().expect("Expected value for COUNT");
                        
                        match val {
                            Value::Box(contents) => {
                                stack.push(Value::Int(contents.len() as i64));
                            }
                            Value::Str(s) => {
                                stack.push(Value::Int(s.len() as i64));
                            }
                            _ => return Err("COUNT: expected box or string".to_string()),
                        }
                    }
                    "rename" => {
                        if *arg_count != 2 { return Err("RENAME expects exactly 2 arguments".to_string()); }
                        let to_val = stack.pop().expect("Expected destination name for RENAME");
                        let from_val = stack.pop().expect("Expected source for RENAME");
                        let from_name = match from_val { Value::Str(s) => s, Value::Function { name, .. } => name, _ => return Err("RENAME: first argument must be a string or function".to_string()) };
                        let to_name = match to_val { Value::Str(s) => s, _ => return Err("RENAME: destination must be a string".to_string()) };
                        let old_key = format!("__module_{}", from_name);
                        let mut module_val = match env.remove(&old_key) { Some(v) => v, None => return Err(format!("RENAME: module '{}' not found", from_name)) };
                        if let Value::Box(ref mut items) = module_val { if !items.is_empty() { items[0] = Value::Str(to_name.clone()); } }
                        let new_key = format!("__module_{}", to_name);
                        env.insert(new_key, module_val);
                        stack.push(Value::Str("OK".to_string()));
                    }
                    "ship" => {
                        if *arg_count != 0 { return Err("SHIP expects 0 arguments".to_string()); }
                        // Get the function name from the environment
                        let func_name = func_name.clone();
                        let func = env.get(&func_name).cloned().ok_or("Function not found")?;
                        match func {
                            Value::Function { name, params, body, .. } => {
                                if !params.is_empty() { return Err("SHIP: function argument must take 0 parameters".to_string()); }
                                // Execute the factory in a cloned environment to collect its definitions
                                let mut module_env = env.clone();
                                let mut tmp_stack: Vec<Value> = Vec::new();
                                let _ = execute_function(&body, &mut tmp_stack, &mut module_env)?;
                                // Diff module_env against env to get new/changed definitions
                                let mut exports = HashMap::new();
                                for (key, value) in module_env.iter() {
                                    if !key.starts_with("__") &&
                                       !["box","pack","place","unpack","pick","count","print",
                                         "result","string","read","write","fission","fusion",
                                         "rewire_symbol","ship","import"].contains(&key.as_str()) {
                                        match env.get(key) {
                                            Some(old) if old == value => { /* unchanged; skip */ }
                                            _ => { exports.insert(key.clone(), value.clone()); }
                                        }
                                    }
                                }
                                // Convert exports to a boxed representation
                                let mut pairs: Vec<Value> = Vec::new();
                                for (k, v) in exports.iter() {
                                    pairs.push(Value::Box(vec![Value::Str(k.clone()), v.clone()]));
                                }
                                let module_key = format!("__module_{}", name);
                                env.insert(module_key, Value::Box(vec![
                                    Value::Str(name.clone()),
                                    Value::Box(pairs.clone()),
                                ]));
                                stack.push(Value::Str(format!("Module '{}' shipped with {} exports", name, exports.len())));
                            }
                            _ => return Err("SHIP: module must be a function".to_string()),
                        }
                    }
                    

                    "import" => {
                        if *arg_count == 1 {
                            // import(arg): if arg matches a module record, import it; else treat as source key and import all from loaded modules
                            let arg = stack.pop().expect("Expected argument for IMPORT");
                            match arg {
                                Value::Function { name, params, .. } => {
                                    if !params.is_empty() { return Err("IMPORT: function argument must take 0 parameters".to_string()); }
                                    let module_key = format!("__module_{}", name);
                                    let module_val = env.get(&module_key).ok_or_else(|| format!("IMPORT: module '{}' not found", name))?;
                                    let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                    let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); env.insert(key.clone(), val); count+=1; } } } }
                                    stack.push(Value::Str(format!("Module '{}' imported with {} exports", name, count)));
                                }
                                Value::Str(s) => {
                                    // Try existing module first
                                    let module_key = format!("__module_{}", s);
                                    if let Some(module_val) = env.get(&module_key) {
                                        let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                        let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); env.insert(key.clone(), val); count+=1; } } } }
                                        stack.push(Value::Str(format!("Module '{}' imported with {} exports", s, count)));
                                    } else {
                                        // Treat as source key; load and import all new modules
                                        let pre: Vec<String> = env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                                        load_module_from_key(&mut env, &s)?;
                                        let post: Vec<String> = env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                                        let mut new_modules: Vec<String> = post.into_iter().filter(|k| !pre.contains(k)).collect();
                                        if new_modules.is_empty() { new_modules = pre; /* fallback: import existing if none new */ }
                                        let mut total = 0;
                                        for mk in new_modules {
                                            if let Some(module_val) = env.get(&mk) {
                                                let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                                for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); env.insert(key.clone(), val); total+=1; } } } }
                                            }
                                        }
                                        stack.push(Value::Str(format!("Imported {} exports from '{}'", total, s)));
                                    }
                                }
                                _ => return Err("IMPORT: argument must be a string or function".to_string()),
                            }
                        } else if *arg_count >= 2 {
                            // import(symbol1, symbol2, ..., src_key)
                            let src_key_val = stack.pop().expect("Expected source key for IMPORT");
                            let src_key = match src_key_val { Value::Str(s)=> s, _=> return Err("IMPORT: source key must be a string".to_string()) };
                            // Collect selectors
                            let mut selectors: Vec<String> = Vec::new();
                            for _ in 0..(*arg_count - 1) {
                                match stack.pop().expect("Expected selector") {
                                    Value::Str(s) => selectors.push(s),
                                    _ => return Err("IMPORT: selectors must be strings".to_string()),
                                }
                            }
                            selectors.reverse();
                            let pre: Vec<String> = env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                            load_module_from_key(&mut env, &src_key)?;
                            let post: Vec<String> = env.keys().filter(|k| k.starts_with("__module_")).cloned().collect();
                            let new_modules: Vec<String> = post.into_iter().filter(|k| !pre.contains(k)).collect();
                            // If given exactly one selector matching a module name, import that module
                            if selectors.len() == 1 {
                                let module_name = &selectors[0];
                                let mk = format!("__module_{}", module_name);
                                if env.contains_key(&mk) {
                                    let module_val = env.get(&mk).unwrap();
                                    let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                    let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); env.insert(key.clone(), val); count+=1; } } } }
                                    stack.push(Value::Str(format!("Module '{}' imported with {} exports", module_name, count)));
                                    return Ok(());
                                } else if !new_modules.is_empty() {
                                    // Alias the first newly loaded module to the requested name, then import
                                    let old_mk = new_modules[0].clone();
                                    if let Some(mut module_val) = env.remove(&old_mk) {
                                        if let Value::Box(ref mut items) = module_val { if !items.is_empty() { items[0] = Value::Str(module_name.clone()); } }
                                        env.insert(mk.clone(), module_val);
                                        if let Some(module_val2) = env.get(&mk) {
                                            let exports_box = match module_val2 { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                            let mut count = 0; for pair in exports_box.into_iter() { if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] { let val = kv[1].clone(); env.insert(key.clone(), val); count+=1; } } } }
                                            stack.push(Value::Str(format!("Module '{}' imported with {} exports", module_name, count)));
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                            // Otherwise, import only selected symbols from newly loaded modules (or all modules if none detected)
                            let module_keys = if new_modules.is_empty() { pre } else { new_modules };
                            let mut count = 0;
                            for mk in module_keys {
                                if let Some(module_val) = env.get(&mk) {
                                    let exports_box = match module_val { Value::Box(items) if items.len()>=2 => { match &items[1] { Value::Box(v)=> v.clone(), _=> Vec::new() } }, _=> Vec::new() };
                                    for pair in exports_box.into_iter() {
                                        if let Value::Box(kv) = pair { if kv.len()==2 { if let Value::Str(key)=&kv[0] {
                                            if selectors.contains(key) { let val = kv[1].clone(); env.insert(key.clone(), val); count+=1; }
                                        } } }
                                    }
                                }
                            }
                            stack.push(Value::Str(format!("Imported {} selected exports from '{}'", count, src_key)));
                        } else {
                            return Err("IMPORT expects at least 1 argument".to_string());
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
                        // Handle user-defined functions (pure by default): bind args, run in cloned env
                        match func {
                            Value::Function { name, params, body, .. } => {
                                if params.len() != *arg_count {
                                    return Err(format!("Function '{}' expects {} arguments, got {}", 
                                        name, params.len(), arg_count));
                                }
                                // Pop arguments (reverse order) then bind to parameter names
                                let mut args = Vec::with_capacity(*arg_count);
                                for _ in 0..*arg_count { args.push(stack.pop().expect("Missing argument")); }
                                args.reverse();
                                let mut env_clone = env.clone();
                                for (i, p) in params.iter().enumerate() { env_clone.insert(p.clone(), args[i].clone()); }
                                let body_clone = body.clone();
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
            
            // TCP Socket Operations
            OpCode::TcpConnect(host, port) => {
                match tcp_manager.connect(host, *port) {
                    Ok(connection_id) => stack.push(Value::TcpConnection(connection_id)),
                    Err(e) => return Err(e),
                }
            }
            
            OpCode::TcpListen(port) => {
                match tcp_manager.listen(*port) {
                    Ok(listener_id) => stack.push(Value::TcpListener(listener_id)),
                    Err(e) => return Err(e),
                }
            }
            
            OpCode::TcpSend(_max_bytes) => {
                let data = stack.pop().expect("Expected data to send");
                let connection = stack.pop().expect("Expected connection");
                
                let data_str = match data {
                    Value::Str(s) => s,
                    _ => return Err("TCP_SEND expects string data".to_string()),
                };
                
                let connection_id = match connection {
                    Value::TcpConnection(id) => id,
                    _ => return Err("TCP_SEND expects TCP connection".to_string()),
                };
                
                match tcp_manager.send(connection_id, &data_str) {
                    Ok(_) => stack.push(Value::Bool(true)),
                    Err(e) => return Err(e),
                }
            }
            
            OpCode::TcpReceive(max_bytes) => {
                let connection = stack.pop().expect("Expected connection");
                
                let connection_id = match connection {
                    Value::TcpConnection(id) => id,
                    _ => return Err("TCP_RECEIVE expects TCP connection".to_string()),
                };
                
                match tcp_manager.receive(connection_id, *max_bytes) {
                    Ok(data) => stack.push(Value::Str(data)),
                    Err(e) => return Err(e),
                }
            }
            
            OpCode::TcpClose(connection_id) => {
                let connection = stack.pop().expect("Expected connection");
                
                let id = match connection {
                    Value::TcpConnection(id) => id,
                    Value::TcpListener(id) => id,
                    _ => return Err("TCP_CLOSE expects TCP connection or listener".to_string()),
                };
                
                let result = if connection_id == &0 {
                    // Close connection
                    tcp_manager.close_connection(id)
                } else {
                    // Close listener
                    tcp_manager.close_listener(id)
                };
                
                match result {
                    Ok(_) => stack.push(Value::Bool(true)),
                    Err(e) => return Err(e),
                }
            }
            
            OpCode::TcpAccept(_listener_id) => {
                let listener = stack.pop().expect("Expected listener");
                
                let _id = match listener {
                    Value::TcpListener(id) => id,
                    _ => return Err("TCP_ACCEPT expects TCP listener".to_string()),
                };
                
                // For now, we'll return an error since accept is blocking
                // In a real implementation, this would need to be non-blocking
                return Err("TCP_ACCEPT not yet implemented (blocking operation)".to_string());
            }
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
