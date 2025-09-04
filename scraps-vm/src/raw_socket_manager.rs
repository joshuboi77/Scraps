use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::os::unix::io::RawFd;

// Note: Raw sockets require elevated privileges (root/admin) on most systems
// This implementation provides a cross-platform interface for raw socket operations

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Protocol {
    Icmp = 1,
    Tcp = 6,
    Udp = 17,
    Custom(u8),
}

impl Protocol {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Protocol::Icmp,
            6 => Protocol::Tcp,
            17 => Protocol::Udp,
            other => Protocol::Custom(other),
        }
    }
    
    pub fn to_u8(&self) -> u8 {
        match self {
            Protocol::Icmp => 1,
            Protocol::Tcp => 6,
            Protocol::Udp => 17,
            Protocol::Custom(n) => *n,
        }
    }
    
    pub fn name(&self) -> String {
        match self {
            Protocol::Icmp => "ICMP".to_string(),
            Protocol::Tcp => "TCP".to_string(),
            Protocol::Udp => "UDP".to_string(),
            Protocol::Custom(n) => format!("Protocol({})", n),
        }
    }
}

pub struct RawSocket {
    fd: RawFd,
    protocol: Protocol,
    header_included: bool,
}

impl RawSocket {
    #[cfg(unix)]
    pub fn new(protocol: Protocol) -> Result<Self, String> {
        
        // Create raw socket - requires root privileges
        let socket_type = libc::SOCK_RAW;
        let protocol_num = protocol.to_u8() as i32;
        
        let fd = unsafe {
            libc::socket(libc::AF_INET, socket_type, protocol_num)
        };
        
        if fd < 0 {
            return Err("Failed to create raw socket - requires root privileges".to_string());
        }
        
        Ok(RawSocket {
            fd,
            protocol,
            header_included: false,
        })
    }
    
    #[cfg(not(unix))]
    pub fn new(protocol: Protocol) -> Result<Self, String> {
        // On non-Unix systems, raw sockets may not be available or require different APIs
        Err("Raw sockets not supported on this platform".to_string())
    }
    
    pub fn set_header_included(&mut self, included: bool) -> Result<(), String> {
        self.header_included = included;
        
        #[cfg(unix)]
        {
            let value = if included { 1 } else { 0 };
            let result = unsafe {
                libc::setsockopt(
                    self.fd,
                    libc::IPPROTO_IP,
                    libc::IP_HDRINCL,
                    &value as *const i32 as *const libc::c_void,
                    std::mem::size_of::<i32>() as libc::socklen_t,
                )
            };
            
            if result < 0 {
                return Err("Failed to set IP_HDRINCL option".to_string());
            }
        }
        
        Ok(())
    }
    
    pub fn send_to(&self, data: &[u8], target: &str) -> Result<usize, String> {
        let target_addr: Ipv4Addr = target.parse()
            .map_err(|_| format!("Invalid target address: {}", target))?;
        
        #[cfg(unix)]
        {
            let addr = libc::sockaddr_in {
                #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly"))]
                sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
                sin_family: libc::AF_INET as _,
                sin_port: 0, // Raw sockets don't use ports at IP level
                sin_addr: libc::in_addr {
                    s_addr: u32::from(target_addr).to_be(),
                },
                sin_zero: [0; 8],
            };
            
            let bytes_sent = unsafe {
                libc::sendto(
                    self.fd,
                    data.as_ptr() as *const libc::c_void,
                    data.len(),
                    0,
                    &addr as *const libc::sockaddr_in as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                )
            };
            
            if bytes_sent < 0 {
                return Err("Failed to send raw packet".to_string());
            }
            
            Ok(bytes_sent as usize)
        }
        
        #[cfg(not(unix))]
        {
            Err("Raw socket send not supported on this platform".to_string())
        }
    }
    
    pub fn receive(&self, buffer: &mut [u8]) -> Result<(usize, String), String> {
        #[cfg(unix)]
        {
            let mut addr = libc::sockaddr_in {
                #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly"))]
                sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
                sin_family: libc::AF_INET as _,
                sin_port: 0,
                sin_addr: libc::in_addr { s_addr: 0 },
                sin_zero: [0; 8],
            };
            let mut addr_len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
            
            let bytes_received = unsafe {
                libc::recvfrom(
                    self.fd,
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                    0,
                    &mut addr as *mut libc::sockaddr_in as *mut libc::sockaddr,
                    &mut addr_len,
                )
            };
            
            if bytes_received < 0 {
                return Err("Failed to receive raw packet".to_string());
            }
            
            let source_ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
            Ok((bytes_received as usize, source_ip.to_string()))
        }
        
        #[cfg(not(unix))]
        {
            Err("Raw socket receive not supported on this platform".to_string())
        }
    }
    
    pub fn close(&self) -> Result<(), String> {
        #[cfg(unix)]
        {
            let result = unsafe { libc::close(self.fd) };
            if result < 0 {
                return Err("Failed to close raw socket".to_string());
            }
        }
        Ok(())
    }
    
    pub fn get_protocol(&self) -> Protocol {
        self.protocol
    }
    
    pub fn is_header_included(&self) -> bool {
        self.header_included
    }
}

impl Drop for RawSocket {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

pub struct RawSocketManager {
    sockets: HashMap<usize, RawSocket>,
    next_socket_id: usize,
}

impl RawSocketManager {
    pub fn new() -> Self {
        Self {
            sockets: HashMap::new(),
            next_socket_id: 1,
        }
    }
    
    pub fn create_raw_socket(&mut self, protocol: u8) -> Result<usize, String> {
        let proto = Protocol::from_u8(protocol);
        let socket = RawSocket::new(proto)?;
        
        let id = self.next_socket_id;
        self.next_socket_id += 1;
        self.sockets.insert(id, socket);
        Ok(id)
    }
    
    pub fn set_header_included(&mut self, socket_id: usize, included: bool) -> Result<(), String> {
        if let Some(socket) = self.sockets.get_mut(&socket_id) {
            socket.set_header_included(included)
        } else {
            Err("Raw socket not found".to_string())
        }
    }
    
    pub fn send_raw(&mut self, socket_id: usize, data: &[u8], target: &str) -> Result<usize, String> {
        if let Some(socket) = self.sockets.get(&socket_id) {
            socket.send_to(data, target)
        } else {
            Err("Raw socket not found".to_string())
        }
    }
    
    pub fn receive_raw(&mut self, socket_id: usize, max_bytes: usize) -> Result<(Vec<u8>, String), String> {
        if let Some(socket) = self.sockets.get(&socket_id) {
            let mut buffer = vec![0; max_bytes];
            let (bytes_received, source) = socket.receive(&mut buffer)?;
            buffer.truncate(bytes_received);
            Ok((buffer, source))
        } else {
            Err("Raw socket not found".to_string())
        }
    }
    
    pub fn close_raw(&mut self, socket_id: usize) -> Result<(), String> {
        if let Some(socket) = self.sockets.remove(&socket_id) {
            socket.close()
        } else {
            Err("Raw socket not found".to_string())
        }
    }
    
    pub fn get_socket_info(&self, socket_id: usize) -> Option<(Protocol, bool)> {
        self.sockets.get(&socket_id).map(|s| (s.get_protocol(), s.is_header_included()))
    }
    
    #[allow(dead_code)]
    pub fn list_sockets(&self) -> Vec<(usize, Protocol, bool)> {
        self.sockets
            .iter()
            .map(|(&id, socket)| (id, socket.get_protocol(), socket.is_header_included()))
            .collect()
    }
}

// Helper functions for common packet crafting
pub struct PacketBuilder;

impl PacketBuilder {
    /// Create a basic ICMP echo request packet
    pub fn icmp_echo_request(id: u16, sequence: u16, data: &[u8]) -> Vec<u8> {
        let mut packet = Vec::new();
        
        // ICMP Header
        packet.push(8); // Type: Echo Request
        packet.push(0); // Code: 0
        packet.extend_from_slice(&[0, 0]); // Checksum (calculated later)
        packet.extend_from_slice(&id.to_be_bytes()); // Identifier
        packet.extend_from_slice(&sequence.to_be_bytes()); // Sequence number
        
        // Data payload
        packet.extend_from_slice(data);
        
        // Calculate checksum
        let checksum = Self::calculate_checksum(&packet);
        packet[2..4].copy_from_slice(&checksum.to_be_bytes());
        
        packet
    }
    
    /// Calculate Internet checksum
    pub fn calculate_checksum(data: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        
        // Sum all 16-bit words
        for chunk in data.chunks(2) {
            if chunk.len() == 2 {
                sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
            } else {
                sum += (chunk[0] as u32) << 8;
            }
        }
        
        // Add carry bits
        while (sum >> 16) > 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
        
        // One's complement
        !(sum as u16)
    }
    
    /// Create a basic IPv4 header
    pub fn ipv4_header(source: &str, dest: &str, protocol: u8, data_len: u16) -> Result<Vec<u8>, String> {
        let source_ip: Ipv4Addr = source.parse()
            .map_err(|_| format!("Invalid source IP: {}", source))?;
        let dest_ip: Ipv4Addr = dest.parse()
            .map_err(|_| format!("Invalid destination IP: {}", dest))?;
        
        let mut header = Vec::new();
        
        // Version (4) + IHL (5) = 0x45
        header.push(0x45);
        // Type of Service
        header.push(0);
        // Total Length
        let total_len = 20 + data_len; // 20 bytes IP header + data
        header.extend_from_slice(&total_len.to_be_bytes());
        // Identification
        header.extend_from_slice(&0u16.to_be_bytes());
        // Flags + Fragment Offset
        header.extend_from_slice(&0u16.to_be_bytes());
        // TTL
        header.push(64);
        // Protocol
        header.push(protocol);
        // Header Checksum (calculated later)
        header.extend_from_slice(&[0, 0]);
        // Source IP
        header.extend_from_slice(&source_ip.octets());
        // Destination IP
        header.extend_from_slice(&dest_ip.octets());
        
        // Calculate header checksum
        let checksum = Self::calculate_checksum(&header);
        header[10..12].copy_from_slice(&checksum.to_be_bytes());
        
        Ok(header)
    }
}
