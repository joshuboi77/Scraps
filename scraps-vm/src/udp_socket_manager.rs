use std::collections::HashMap;
use std::net::{UdpSocket, Ipv4Addr};
use std::time::Duration;

pub struct UdpSocketManager {
    sockets: HashMap<usize, UdpSocket>,
    next_socket_id: usize,
}

impl UdpSocketManager {
    pub fn new() -> Self {
        Self {
            sockets: HashMap::new(),
            next_socket_id: 1,
        }
    }
    
    pub fn bind(&mut self, addr: &str) -> Result<usize, String> {
        let socket = UdpSocket::bind(addr)
            .map_err(|e| format!("Failed to bind UDP socket to {}: {}", addr, e))?;
        
        let id = self.next_socket_id;
        self.next_socket_id += 1;
        self.sockets.insert(id, socket);
        Ok(id)
    }
    
    pub fn send(&mut self, socket_id: usize, data: &[u8], addr: &str) -> Result<usize, String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            socket.send_to(data, addr)
                .map_err(|e| format!("Failed to send UDP data: {}", e))
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    pub fn receive(&mut self, socket_id: usize, max_bytes: usize) -> Result<(Vec<u8>, String), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            let mut buffer = vec![0; max_bytes];
            match socket.recv_from(&mut buffer) {
                Ok((bytes_read, src_addr)) => {
                    buffer.truncate(bytes_read);
                    Ok((buffer, src_addr.to_string()))
                }
                Err(e) => Err(format!("Failed to receive UDP data: {}", e))
            }
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    pub fn try_receive(&mut self, socket_id: usize, max_bytes: usize) -> Result<Option<(Vec<u8>, String)>, String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            // Set socket to non-blocking mode
            socket.set_nonblocking(true)
                .map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;
            
            let mut buffer = vec![0; max_bytes];
            let result = match socket.recv_from(&mut buffer) {
                Ok((bytes_read, src_addr)) => {
                    buffer.truncate(bytes_read);
                    Some((buffer, src_addr.to_string()))
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        None
                    } else {
                        return Err(format!("Failed to try-receive UDP data: {}", e));
                    }
                }
            };
            
            // Reset to blocking mode
            socket.set_nonblocking(false)
                .map_err(|e| format!("Failed to reset blocking mode: {}", e))?;
            
            Ok(result)
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    pub fn close(&mut self, socket_id: usize) -> Result<(), String> {
        if self.sockets.remove(&socket_id).is_some() {
            Ok(())
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    #[allow(dead_code)]
    pub fn set_timeout(&mut self, socket_id: usize, timeout_ms: u64) -> Result<(), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            let timeout = Duration::from_millis(timeout_ms);
            socket.set_read_timeout(Some(timeout))
                .map_err(|e| format!("Failed to set read timeout: {}", e))?;
            socket.set_write_timeout(Some(timeout))
                .map_err(|e| format!("Failed to set write timeout: {}", e))?;
            Ok(())
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    #[allow(dead_code)]
    pub fn get_local_addr(&self, socket_id: usize) -> Result<String, String> {
        if let Some(socket) = self.sockets.get(&socket_id) {
            socket.local_addr()
                .map(|addr| addr.to_string())
                .map_err(|e| format!("Failed to get local address: {}", e))
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    /// Join a multicast group
    pub fn join_multicast_group(&mut self, socket_id: usize, multicast_addr: &str, interface_addr: Option<&str>) -> Result<(), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            let multicast_ip: Ipv4Addr = multicast_addr.parse()
                .map_err(|_| format!("Invalid multicast address: {}", multicast_addr))?;
            
            let interface_ip = if let Some(addr) = interface_addr {
                addr.parse().map_err(|_| format!("Invalid interface address: {}", addr))?
            } else {
                Ipv4Addr::UNSPECIFIED // 0.0.0.0 - use default interface
            };
            
            socket.join_multicast_v4(&multicast_ip, &interface_ip)
                .map_err(|e| format!("Failed to join multicast group {}: {}", multicast_addr, e))
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    /// Leave a multicast group
    pub fn leave_multicast_group(&mut self, socket_id: usize, multicast_addr: &str, interface_addr: Option<&str>) -> Result<(), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            let multicast_ip: Ipv4Addr = multicast_addr.parse()
                .map_err(|_| format!("Invalid multicast address: {}", multicast_addr))?;
            
            let interface_ip = if let Some(addr) = interface_addr {
                addr.parse().map_err(|_| format!("Invalid interface address: {}", addr))?
            } else {
                Ipv4Addr::UNSPECIFIED
            };
            
            socket.leave_multicast_v4(&multicast_ip, &interface_ip)
                .map_err(|e| format!("Failed to leave multicast group {}: {}", multicast_addr, e))
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    /// Set multicast Time-To-Live (TTL)
    pub fn set_multicast_ttl(&mut self, socket_id: usize, ttl: u32) -> Result<(), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            socket.set_multicast_ttl_v4(ttl)
                .map_err(|e| format!("Failed to set multicast TTL: {}", e))
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    /// Set multicast loopback (whether to receive own multicast messages)
    pub fn set_multicast_loopback(&mut self, socket_id: usize, loopback: bool) -> Result<(), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            socket.set_multicast_loop_v4(loopback)
                .map_err(|e| format!("Failed to set multicast loopback: {}", e))
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    /// Enable or disable broadcast
    pub fn set_broadcast(&mut self, socket_id: usize, broadcast: bool) -> Result<(), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            socket.set_broadcast(broadcast)
                .map_err(|e| format!("Failed to set broadcast: {}", e))
        } else {
            Err("UDP socket not found".to_string())
        }
    }
    
    /// Send broadcast message
    pub fn send_broadcast(&mut self, socket_id: usize, data: &[u8], port: u16) -> Result<usize, String> {
        // First enable broadcast if not already enabled
        self.set_broadcast(socket_id, true)?;
        
        // Send to broadcast address
        let broadcast_addr = format!("255.255.255.255:{}", port);
        self.send(socket_id, data, &broadcast_addr)
    }
    
    /// Send multicast message
    pub fn send_multicast(&mut self, socket_id: usize, data: &[u8], multicast_addr: &str, port: u16) -> Result<usize, String> {
        let full_addr = format!("{}:{}", multicast_addr, port);
        self.send(socket_id, data, &full_addr)
    }
    
    /// Check if an address is a multicast address
    pub fn is_multicast_address(addr: &str) -> bool {
        if let Ok(ip) = addr.parse::<Ipv4Addr>() {
            ip.is_multicast()
        } else {
            false
        }
    }
    
    /// Check if an address is a broadcast address
    pub fn is_broadcast_address(addr: &str) -> bool {
        addr == "255.255.255.255" || 
        addr.starts_with("255.255.255.255:") ||
        if let Ok(ip) = addr.parse::<Ipv4Addr>() {
            ip.is_broadcast()
        } else {
            false
        }
    }
}
