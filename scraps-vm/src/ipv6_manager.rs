use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum DualStackMode {
    IPv4Only,
    IPv6Only,
    DualStack, // Prefer IPv6, fallback to IPv4
    DualStackPreferIPv4, // Prefer IPv4, fallback to IPv6
}

impl DualStackMode {
    pub fn from_string(mode: &str) -> Result<Self, String> {
        match mode.to_lowercase().as_str() {
            "ipv4" | "ipv4_only" => Ok(DualStackMode::IPv4Only),
            "ipv6" | "ipv6_only" => Ok(DualStackMode::IPv6Only),
            "dual" | "dual_stack" | "dualstack" => Ok(DualStackMode::DualStack),
            "dual_ipv4" | "dual_stack_ipv4" => Ok(DualStackMode::DualStackPreferIPv4),
            _ => Err(format!("Invalid dual-stack mode: {}. Valid modes: ipv4, ipv6, dual, dual_ipv4", mode)),
        }
    }
    
    pub fn to_string(&self) -> String {
        match self {
            DualStackMode::IPv4Only => "IPv4Only".to_string(),
            DualStackMode::IPv6Only => "IPv6Only".to_string(),
            DualStackMode::DualStack => "DualStack".to_string(),
            DualStackMode::DualStackPreferIPv4 => "DualStackPreferIPv4".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IPv6Address {
    pub address: Ipv6Addr,
    pub scope_id: Option<u32>,
    pub is_link_local: bool,
    pub is_site_local: bool,
    pub is_unique_local: bool,
    pub is_multicast: bool,
    pub is_loopback: bool,
    pub is_unspecified: bool,
}

impl IPv6Address {
    pub fn new(address: Ipv6Addr, scope_id: Option<u32>) -> Self {
        Self {
            address,
            scope_id,
            is_link_local: address.is_loopback(),
            is_site_local: address.is_unicast_link_local(),
            is_unique_local: address.is_unique_local(),
            is_multicast: address.is_multicast(),
            is_loopback: address.is_loopback(),
            is_unspecified: address.is_unspecified(),
        }
    }
    
    pub fn to_string(&self) -> String {
        if let Some(scope_id) = self.scope_id {
            format!("{}%{}", self.address, scope_id)
        } else {
            self.address.to_string()
        }
    }
    
    #[allow(dead_code)]
    pub fn get_scope_id(&self) -> Option<u32> {
        self.scope_id
    }
    
    pub fn is_global(&self) -> bool {
        !self.is_link_local && !self.is_site_local && !self.is_unique_local && 
        !self.is_multicast && !self.is_loopback && !self.is_unspecified
    }
}

#[derive(Debug, Clone)]
pub struct DualStackResult {
    pub ipv4_address: Option<Ipv4Addr>,
    pub ipv6_address: Option<IPv6Address>,
    pub preferred_address: IpAddr,
    pub preferred_family: String,
}

impl DualStackResult {
    pub fn new(ipv4: Option<Ipv4Addr>, ipv6: Option<IPv6Address>, preferred: IpAddr) -> Self {
        let preferred_family = match preferred {
            IpAddr::V4(_) => "IPv4".to_string(),
            IpAddr::V6(_) => "IPv6".to_string(),
        };
        
        Self {
            ipv4_address: ipv4,
            ipv6_address: ipv6,
            preferred_address: preferred,
            preferred_family,
        }
    }
}

pub struct IPv6Manager {
    dual_stack_mode: DualStackMode,
    ipv6_enabled: bool,
    ipv4_enabled: bool,
}

impl IPv6Manager {
    pub fn new() -> Self {
        Self {
            dual_stack_mode: DualStackMode::DualStack,
            ipv6_enabled: true,
            ipv4_enabled: true,
        }
    }
    
    /// Set the dual-stack mode
    pub fn set_dual_stack_mode(&mut self, mode: DualStackMode) {
        self.dual_stack_mode = mode;
    }
    
    /// Get the current dual-stack mode
    pub fn get_dual_stack_mode(&self) -> &DualStackMode {
        &self.dual_stack_mode
    }
    
    /// Enable or disable IPv6
    #[allow(dead_code)]
    pub fn set_ipv6_enabled(&mut self, enabled: bool) {
        self.ipv6_enabled = enabled;
    }
    
    /// Enable or disable IPv4
    #[allow(dead_code)]
    pub fn set_ipv4_enabled(&mut self, enabled: bool) {
        self.ipv4_enabled = enabled;
    }
    
    /// Check if IPv6 is enabled
    #[allow(dead_code)]
    pub fn is_ipv6_enabled(&self) -> bool {
        self.ipv6_enabled
    }
    
    /// Check if IPv4 is enabled
    #[allow(dead_code)]
    pub fn is_ipv4_enabled(&self) -> bool {
        self.ipv4_enabled
    }
    
    /// Parse an IPv6 address with optional scope ID
    pub fn parse_ipv6_address(&self, addr_str: &str) -> Result<IPv6Address, String> {
        if addr_str.contains('%') {
            let parts: Vec<&str> = addr_str.split('%').collect();
            if parts.len() != 2 {
                return Err("Invalid IPv6 address with scope ID format".to_string());
            }
            
            let address = Ipv6Addr::from_str(parts[0])
                .map_err(|_| "Invalid IPv6 address".to_string())?;
            let scope_id = parts[1].parse::<u32>()
                .map_err(|_| "Invalid scope ID".to_string())?;
            
            Ok(IPv6Address::new(address, Some(scope_id)))
        } else {
            let address = Ipv6Addr::from_str(addr_str)
                .map_err(|_| "Invalid IPv6 address".to_string())?;
            Ok(IPv6Address::new(address, None))
        }
    }
    
    /// Resolve a hostname to both IPv4 and IPv6 addresses
    pub fn resolve_dual_stack(&self, hostname: &str) -> Result<DualStackResult, String> {
        // This is a simplified implementation
        // In a real implementation, you would use the system's DNS resolver
        
        let mut ipv4_address = None;
        let mut ipv6_address = None;
        
        // Try to resolve as IPv4
        if self.ipv4_enabled {
            match self.resolve_ipv4(hostname) {
                Ok(addr) => ipv4_address = Some(addr),
                Err(_) => {}, // IPv4 resolution failed
            }
        }
        
        // Try to resolve as IPv6
        if self.ipv6_enabled {
            match self.resolve_ipv6(hostname) {
                Ok(addr) => ipv6_address = Some(addr),
                Err(_) => {}, // IPv6 resolution failed
            }
        }
        
        // Determine preferred address based on dual-stack mode
        let preferred_address = match self.dual_stack_mode {
            DualStackMode::IPv4Only => {
                if let Some(ipv4) = ipv4_address {
                    IpAddr::V4(ipv4)
                } else {
                    return Err("IPv4 resolution failed and IPv4-only mode is enabled".to_string());
                }
            }
            DualStackMode::IPv6Only => {
                if let Some(ipv6) = &ipv6_address {
                    IpAddr::V6(ipv6.address)
                } else {
                    return Err("IPv6 resolution failed and IPv6-only mode is enabled".to_string());
                }
            }
            DualStackMode::DualStack => {
                if let Some(ipv6) = &ipv6_address {
                    IpAddr::V6(ipv6.address)
                } else if let Some(ipv4) = ipv4_address {
                    IpAddr::V4(ipv4)
                } else {
                    return Err("Both IPv4 and IPv6 resolution failed".to_string());
                }
            }
            DualStackMode::DualStackPreferIPv4 => {
                if let Some(ipv4) = ipv4_address {
                    IpAddr::V4(ipv4)
                } else if let Some(ipv6) = &ipv6_address {
                    IpAddr::V6(ipv6.address)
                } else {
                    return Err("Both IPv4 and IPv6 resolution failed".to_string());
                }
            }
        };
        
        Ok(DualStackResult::new(ipv4_address, ipv6_address, preferred_address))
    }
    
    /// Resolve hostname to IPv4 address (simplified)
    fn resolve_ipv4(&self, hostname: &str) -> Result<Ipv4Addr, String> {
        // This is a simplified implementation
        // In a real implementation, you would use the system's DNS resolver
        
        match hostname {
            "localhost" => Ok(Ipv4Addr::new(127, 0, 0, 1)),
            "google.com" => Ok(Ipv4Addr::new(8, 8, 8, 8)),
            "cloudflare.com" => Ok(Ipv4Addr::new(1, 1, 1, 1)),
            _ => {
                // Try to parse as direct IPv4 address
                Ipv4Addr::from_str(hostname)
                    .map_err(|_| format!("Failed to resolve IPv4 address for: {}", hostname))
            }
        }
    }
    
    /// Resolve hostname to IPv6 address (simplified)
    fn resolve_ipv6(&self, hostname: &str) -> Result<IPv6Address, String> {
        // This is a simplified implementation
        // In a real implementation, you would use the system's DNS resolver
        
        match hostname {
            "localhost" => Ok(IPv6Address::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), None)),
            "google.com" => Ok(IPv6Address::new(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888), None)),
            "cloudflare.com" => Ok(IPv6Address::new(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111), None)),
            _ => {
                // Try to parse as direct IPv6 address
                let addr = Ipv6Addr::from_str(hostname)
                    .map_err(|_| format!("Failed to resolve IPv6 address for: {}", hostname))?;
                Ok(IPv6Address::new(addr, None))
            }
        }
    }
    
    /// Get IPv6 multicast address for a given group
    pub fn get_ipv6_multicast_address(&self, group: &str) -> Result<IPv6Address, String> {
        match group.to_lowercase().as_str() {
            "all_nodes" => Ok(IPv6Address::new(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1), None)),
            "all_routers" => Ok(IPv6Address::new(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 2), None)),
            "all_hosts" => Ok(IPv6Address::new(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1), None)),
            _ => {
                // Try to parse as direct IPv6 multicast address
                let addr = Ipv6Addr::from_str(group)
                    .map_err(|_| format!("Invalid IPv6 multicast group: {}", group))?;
                if !addr.is_multicast() {
                    return Err(format!("Address {} is not a multicast address", group));
                }
                Ok(IPv6Address::new(addr, None))
            }
        }
    }
    
    /// Check if an address is IPv6
    pub fn is_ipv6_address(&self, addr_str: &str) -> bool {
        Ipv6Addr::from_str(addr_str).is_ok()
    }
    
    /// Check if an address is IPv4
    pub fn is_ipv4_address(&self, addr_str: &str) -> bool {
        Ipv4Addr::from_str(addr_str).is_ok()
    }
    
    /// Convert IPv4-mapped IPv6 address to IPv4
    #[allow(dead_code)]
    pub fn ipv4_mapped_to_ipv4(&self, ipv6_addr: &Ipv6Addr) -> Option<Ipv4Addr> {
        let octets = ipv6_addr.octets();
        // Check if this is an IPv4-mapped IPv6 address (::ffff:0:0/96)
        if octets[0..10] == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] && octets[10..12] == [0xff, 0xff] {
            Some(Ipv4Addr::new(octets[12], octets[13], octets[14], octets[15]))
        } else {
            None
        }
    }
    
    /// Convert IPv4 address to IPv4-mapped IPv6
    #[allow(dead_code)]
    pub fn ipv4_to_ipv4_mapped(&self, ipv4_addr: &Ipv4Addr) -> Ipv6Addr {
        let octets = ipv4_addr.octets();
        Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 
                     ((octets[0] as u16) << 8) | (octets[1] as u16),
                     ((octets[2] as u16) << 8) | (octets[3] as u16))
    }
    
    /// Get IPv6 address information
    pub fn get_ipv6_info(&self, addr_str: &str) -> Result<IPv6Address, String> {
        self.parse_ipv6_address(addr_str)
    }
    
    /// Get dual-stack configuration
    pub fn get_config(&self) -> (String, bool, bool) {
        (
            self.dual_stack_mode.to_string(),
            self.ipv4_enabled,
            self.ipv6_enabled,
        )
    }
}

/// Helper function to create a dual-stack socket address
pub fn create_dual_stack_socket_addr(host: &str, port: u16, prefer_ipv6: bool) -> Result<SocketAddr, String> {
    let manager = IPv6Manager::new();
    
    match manager.resolve_dual_stack(host) {
        Ok(result) => {
            if prefer_ipv6 {
                if let Some(ipv6) = result.ipv6_address {
                    Ok(SocketAddr::V6(SocketAddrV6::new(ipv6.address, port, 0, ipv6.scope_id.unwrap_or(0))))
                } else if let Some(ipv4) = result.ipv4_address {
                    Ok(SocketAddr::V4(SocketAddrV4::new(ipv4, port)))
                } else {
                    Err("No valid address found".to_string())
                }
            } else {
                if let Some(ipv4) = result.ipv4_address {
                    Ok(SocketAddr::V4(SocketAddrV4::new(ipv4, port)))
                } else if let Some(ipv6) = result.ipv6_address {
                    Ok(SocketAddr::V6(SocketAddrV6::new(ipv6.address, port, 0, ipv6.scope_id.unwrap_or(0))))
                } else {
                    Err("No valid address found".to_string())
                }
            }
        }
        Err(e) => Err(e),
    }
}
