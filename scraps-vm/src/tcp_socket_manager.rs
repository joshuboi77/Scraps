use std::collections::HashMap;
use std::net::{TcpStream, TcpListener};
use std::io::{Read, Write};
use std::time::Duration;

pub struct TcpSocketManager {
    connections: HashMap<usize, TcpStream>,
    listeners: HashMap<usize, TcpListener>,
    next_connection_id: usize,
    next_listener_id: usize,
}

impl TcpSocketManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            listeners: HashMap::new(),
            next_connection_id: 1,
            next_listener_id: 1,
        }
    }
    
    pub fn create_connection(&mut self, stream: TcpStream) -> usize {
        let id = self.next_connection_id;
        self.next_connection_id += 1;
        self.connections.insert(id, stream);
        id
    }
    
    pub fn create_listener(&mut self, listener: TcpListener) -> usize {
        let id = self.next_listener_id;
        self.next_listener_id += 1;
        self.listeners.insert(id, listener);
        id
    }
    
    pub fn get_connection(&mut self, id: usize) -> Option<&mut TcpStream> {
        self.connections.get_mut(&id)
    }
    
    pub fn get_listener(&mut self, id: usize) -> Option<&mut TcpListener> {
        self.listeners.get_mut(&id)
    }
    
    pub fn remove_connection(&mut self, id: usize) -> bool {
        self.connections.remove(&id).is_some()
    }
    
    pub fn remove_listener(&mut self, id: usize) -> bool {
        self.listeners.remove(&id).is_some()
    }
    
    pub fn connect(&mut self, host: &str, port: u16) -> Result<usize, String> {
        match TcpStream::connect(format!("{}:{}", host, port)) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(100))); // short timeout to avoid blocking
                let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                Ok(self.create_connection(stream))
            },
            Err(e) => Err(format!("Failed to connect to {}:{} - {}", host, port, e)),
        }
    }
    
    pub fn listen(&mut self, port: u16) -> Result<usize, String> {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(listener) => Ok(self.create_listener(listener)),
            Err(e) => Err(format!("Failed to bind to port {} - {}", port, e)),
        }
    }
    
    pub fn accept(&mut self, listener_id: usize) -> Result<usize, String> {
        if let Some(listener) = self.get_listener(listener_id) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                    Ok(self.create_connection(stream))
                }
                Err(e) => Err(format!("Failed to accept connection: {}", e)),
            }
        } else {
            Err("Listener not found".to_string())
        }
    }
    
    pub fn send(&mut self, connection_id: usize, data: &str) -> Result<bool, String> {
        if let Some(stream) = self.get_connection(connection_id) {
            match stream.write_all(data.as_bytes()) {
                Ok(_) => Ok(true),
                Err(e) => Err(format!("Failed to send data: {}", e)),
            }
        } else {
            Err("Connection not found".to_string())
        }
    }
    
    pub fn receive(&mut self, connection_id: usize, max_bytes: usize) -> Result<String, String> {
        if let Some(stream) = self.get_connection(connection_id) {
            let mut buffer = vec![0; max_bytes];
            // Avoid indefinite blocking; use a short timeout and treat timeout as empty read
            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
            match stream.read(&mut buffer) {
                Ok(bytes_read) => {
                    buffer.truncate(bytes_read);
                    match String::from_utf8(buffer) {
                        Ok(s) => Ok(s),
                        Err(e) => Err(format!("Invalid UTF-8 data: {}", e)),
                    }
                },
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut {
                        Ok(String::new())
                    } else {
                        Err(format!("Failed to receive data: {}", e))
                    }
                },
            }
        } else {
            Err("Connection not found".to_string())
        }
    }
    
    pub fn close_connection(&mut self, connection_id: usize) -> Result<bool, String> {
        if self.remove_connection(connection_id) {
            Ok(true)
        } else {
            Err("Connection not found".to_string())
        }
    }
    
    pub fn close_listener(&mut self, listener_id: usize) -> Result<bool, String> {
        if self.remove_listener(listener_id) {
            Ok(true)
        } else {
            Err("Listener not found".to_string())
        }
    }
}
