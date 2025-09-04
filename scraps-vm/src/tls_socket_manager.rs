use std::collections::HashMap;
use std::net::{TcpStream, TcpListener};
use std::io::{Read, Write};
use native_tls::{TlsConnector, TlsAcceptor, TlsStream, Identity};

pub struct TlsSocketManager {
    connections: HashMap<usize, TlsStream<TcpStream>>,
    listeners: HashMap<usize, TcpListener>,
    acceptors: HashMap<usize, TlsAcceptor>,
    next_connection_id: usize,
    next_listener_id: usize,
}

impl TlsSocketManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            listeners: HashMap::new(),
            acceptors: HashMap::new(),
            next_connection_id: 1,
            next_listener_id: 1,
        }
    }
    
    pub fn connect(&mut self, host: &str, port: u16) -> Result<usize, String> {
        let connector = TlsConnector::new()
            .map_err(|e| format!("Failed to create TLS connector: {}", e))?;
        
        let stream = TcpStream::connect(format!("{}:{}", host, port))
            .map_err(|e| format!("Failed to connect to {}:{}: {}", host, port, e))?;
        
        let tls_stream = connector.connect(host, stream)
            .map_err(|e| format!("TLS handshake failed: {}", e))?;
        
        let id = self.next_connection_id;
        self.next_connection_id += 1;
        self.connections.insert(id, tls_stream);
        Ok(id)
    }
    
    pub fn listen(&mut self, port: u16, cert_path: &str, key_path: &str) -> Result<usize, String> {
        // Read certificate and key files
        let cert_data = std::fs::read(cert_path)
            .map_err(|e| format!("Failed to read certificate file: {}", e))?;
        let _key_data = std::fs::read(key_path)
            .map_err(|e| format!("Failed to read key file: {}", e))?;
        
        // Create PKCS#12 identity (this is a simplified approach)
        // In a real implementation, you'd want to support different certificate formats
        let identity = Identity::from_pkcs12(&cert_data, "")
            .map_err(|e| format!("Failed to create identity from certificate: {}", e))?;
        
        let acceptor = TlsAcceptor::new(identity)
            .map_err(|e| format!("Failed to create TLS acceptor: {}", e))?;
        
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
            .map_err(|e| format!("Failed to bind to port {}: {}", port, e))?;
        
        let id = self.next_listener_id;
        self.next_listener_id += 1;
        self.listeners.insert(id, listener);
        self.acceptors.insert(id, acceptor);
        Ok(id)
    }
    
    pub fn accept(&mut self, listener_id: usize) -> Result<usize, String> {
        let listener = self.listeners.get(&listener_id)
            .ok_or("TLS listener not found")?;
        let acceptor = self.acceptors.get(&listener_id)
            .ok_or("TLS acceptor not found")?;
        
        let (stream, _addr) = listener.accept()
            .map_err(|e| format!("Failed to accept TLS connection: {}", e))?;
        
        let tls_stream = acceptor.accept(stream)
            .map_err(|e| format!("TLS handshake failed during accept: {}", e))?;
        
        let id = self.next_connection_id;
        self.next_connection_id += 1;
        self.connections.insert(id, tls_stream);
        Ok(id)
    }
    
    pub fn send(&mut self, connection_id: usize, data: &str) -> Result<(), String> {
        if let Some(stream) = self.connections.get_mut(&connection_id) {
            stream.write_all(data.as_bytes())
                .map_err(|e| format!("Failed to send TLS data: {}", e))?;
            stream.flush()
                .map_err(|e| format!("Failed to flush TLS stream: {}", e))?;
            Ok(())
        } else {
            Err("TLS connection not found".to_string())
        }
    }
    
    pub fn receive(&mut self, connection_id: usize, max_bytes: usize) -> Result<String, String> {
        if let Some(stream) = self.connections.get_mut(&connection_id) {
            let mut buffer = vec![0; max_bytes];
            let bytes_read = stream.read(&mut buffer)
                .map_err(|e| format!("Failed to receive TLS data: {}", e))?;
            buffer.truncate(bytes_read);
            String::from_utf8(buffer)
                .map_err(|e| format!("Failed to decode TLS data as UTF-8: {}", e))
        } else {
            Err("TLS connection not found".to_string())
        }
    }
    
    pub fn try_receive(&mut self, connection_id: usize, max_bytes: usize) -> Result<Option<String>, String> {
        if let Some(stream) = self.connections.get_mut(&connection_id) {
            // Set to non-blocking mode
            stream.get_ref().set_nonblocking(true)
                .map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;
            
            let mut buffer = vec![0; max_bytes];
            let result = match stream.read(&mut buffer) {
                Ok(bytes_read) => {
                    buffer.truncate(bytes_read);
                    match String::from_utf8(buffer) {
                        Ok(s) => Some(s),
                        Err(e) => return Err(format!("Failed to decode TLS data as UTF-8: {}", e))
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        None
                    } else {
                        return Err(format!("Failed to try-receive TLS data: {}", e));
                    }
                }
            };
            
            // Reset to blocking mode
            stream.get_ref().set_nonblocking(false)
                .map_err(|e| format!("Failed to reset blocking mode: {}", e))?;
            
            Ok(result)
        } else {
            Err("TLS connection not found".to_string())
        }
    }
    
    pub fn close_connection(&mut self, connection_id: usize) -> Result<(), String> {
        if self.connections.remove(&connection_id).is_some() {
            Ok(())
        } else {
            Err("TLS connection not found".to_string())
        }
    }
    
    pub fn close_listener(&mut self, listener_id: usize) -> Result<(), String> {
        let listener_removed = self.listeners.remove(&listener_id).is_some();
        let acceptor_removed = self.acceptors.remove(&listener_id).is_some();
        
        if listener_removed || acceptor_removed {
            Ok(())
        } else {
            Err("TLS listener not found".to_string())
        }
    }
}
