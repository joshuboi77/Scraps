use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, PartialEq)]
pub enum InterfaceType {
    Ethernet,
    Wireless,
    Loopback,
    Tunnel,
    Virtual,
    Unknown,
}

impl InterfaceType {
    pub fn from_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();
        if name_lower.contains("eth") || name_lower.contains("en") {
            InterfaceType::Ethernet
        } else if name_lower.contains("wlan") || name_lower.contains("wifi") || name_lower.contains("wi") {
            InterfaceType::Wireless
        } else if name_lower.contains("lo") || name_lower.contains("loopback") {
            InterfaceType::Loopback
        } else if name_lower.contains("tun") || name_lower.contains("tap") {
            InterfaceType::Tunnel
        } else if name_lower.contains("veth") || name_lower.contains("docker") || name_lower.contains("br") {
            InterfaceType::Virtual
        } else {
            InterfaceType::Unknown
        }
    }
    
    pub fn to_string(&self) -> String {
        match self {
            InterfaceType::Ethernet => "Ethernet".to_string(),
            InterfaceType::Wireless => "Wireless".to_string(),
            InterfaceType::Loopback => "Loopback".to_string(),
            InterfaceType::Tunnel => "Tunnel".to_string(),
            InterfaceType::Virtual => "Virtual".to_string(),
            InterfaceType::Unknown => "Unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkAddress {
    pub ip: IpAddr,
    pub netmask: IpAddr,
    pub broadcast: Option<IpAddr>,
}

impl NetworkAddress {
    pub fn new(ip: IpAddr, netmask: IpAddr, broadcast: Option<IpAddr>) -> Self {
        Self {
            ip,
            netmask,
            broadcast,
        }
    }
    
    pub fn is_ipv4(&self) -> bool {
        self.ip.is_ipv4()
    }
    
    pub fn is_ipv6(&self) -> bool {
        self.ip.is_ipv6()
    }
    
    #[allow(dead_code)]
    pub fn network_address(&self) -> IpAddr {
        match (self.ip, self.netmask) {
            (IpAddr::V4(ip), IpAddr::V4(mask)) => {
                let network = Ipv4Addr::from(u32::from(ip) & u32::from(mask));
                IpAddr::V4(network)
            }
            (IpAddr::V6(ip), IpAddr::V6(mask)) => {
                let ip_bytes = ip.octets();
                let mask_bytes = mask.octets();
                let mut network_bytes = [0u8; 16];
                for i in 0..16 {
                    network_bytes[i] = ip_bytes[i] & mask_bytes[i];
                }
                IpAddr::V6(Ipv6Addr::from(network_bytes))
            }
            _ => self.ip, // Fallback for mixed types
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub interface_type: InterfaceType,
    pub is_up: bool,
    pub is_loopback: bool,
    pub is_multicast: bool,
    pub mtu: u32,
    pub addresses: Vec<NetworkAddress>,
    pub mac_address: Option<String>,
}

impl NetworkInterface {
    pub fn new(name: String) -> Self {
        let interface_type = InterfaceType::from_name(&name);
        let is_loopback = interface_type == InterfaceType::Loopback;
        
        Self {
            name,
            interface_type,
            is_up: false,
            is_loopback,
            is_multicast: true, // Most interfaces support multicast
            mtu: 1500, // Default MTU
            addresses: Vec::new(),
            mac_address: None,
        }
    }
    
    pub fn add_address(&mut self, address: NetworkAddress) {
        self.addresses.push(address);
    }
    
    pub fn get_primary_ipv4(&self) -> Option<&NetworkAddress> {
        self.addresses.iter()
            .find(|addr| addr.is_ipv4() && !addr.ip.is_loopback())
    }
    
    pub fn get_primary_ipv6(&self) -> Option<&NetworkAddress> {
        self.addresses.iter()
            .find(|addr| addr.is_ipv6() && !addr.ip.is_loopback())
    }
    
    #[allow(dead_code)]
    pub fn get_loopback_ipv4(&self) -> Option<&NetworkAddress> {
        self.addresses.iter()
            .find(|addr| addr.is_ipv4() && addr.ip.is_loopback())
    }
    
    #[allow(dead_code)]
    pub fn get_loopback_ipv6(&self) -> Option<&NetworkAddress> {
        self.addresses.iter()
            .find(|addr| addr.is_ipv6() && addr.ip.is_loopback())
    }
}

pub struct NetworkInterfaceManager {
    pub interfaces: HashMap<String, NetworkInterface>,
}

impl NetworkInterfaceManager {
    pub fn new() -> Self {
        Self {
            interfaces: HashMap::new(),
        }
    }
    
    /// Enumerate all network interfaces on the system
    pub fn enumerate_interfaces(&mut self) -> Result<Vec<String>, String> {
        self.interfaces.clear();
        
        #[cfg(unix)]
        {
            use std::fs;
            
            // Read from /proc/net/dev on Linux or use ifconfig on other Unix systems
            if let Ok(proc_net_dev) = fs::read_to_string("/proc/net/dev") {
                self.parse_proc_net_dev(&proc_net_dev)?;
            } else {
                // Fallback: try to get basic interface info
                self.add_basic_interfaces()?;
            }
        }
        
        #[cfg(windows)]
        {
            // Windows implementation would use GetAdaptersInfo or similar
            self.add_basic_interfaces()?;
        }
        
        #[cfg(not(any(unix, windows)))]
        {
            self.add_basic_interfaces()?;
        }
        
        Ok(self.interfaces.keys().cloned().collect())
    }
    
    #[cfg(unix)]
    fn parse_proc_net_dev(&mut self, content: &str) -> Result<(), String> {
        for line in content.lines() {
            if line.contains(':') && !line.starts_with("Inter-") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 {
                    let name = parts[0].trim().to_string();
                    if !name.is_empty() {
                        let mut interface = NetworkInterface::new(name.clone());
                        interface.is_up = true; // Assume up if in /proc/net/dev
                        self.interfaces.insert(name, interface);
                    }
                }
            }
        }
        
        // Add common addresses for known interfaces
        self.add_common_addresses()?;
        Ok(())
    }
    
    fn add_basic_interfaces(&mut self) -> Result<(), String> {
        // Add common interfaces that most systems have
        let common_interfaces = vec![
            "lo",      // Loopback
            "eth0",    // Ethernet
            "wlan0",   // Wireless
            "en0",     // macOS Ethernet
            "en1",     // macOS Wireless
        ];
        
        for name in common_interfaces {
            let mut interface = NetworkInterface::new(name.to_string());
            interface.is_up = true; // Assume available
            self.interfaces.insert(name.to_string(), interface);
        }
        
        self.add_common_addresses()?;
        Ok(())
    }
    
    fn add_common_addresses(&mut self) -> Result<(), String> {
        // Add common loopback address
        if let Some(lo_interface) = self.interfaces.get_mut("lo") {
            let loopback_ipv4 = NetworkAddress::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                IpAddr::V4(Ipv4Addr::new(255, 0, 0, 0)),
                Some(IpAddr::V4(Ipv4Addr::new(127, 255, 255, 255))),
            );
            lo_interface.add_address(loopback_ipv4);
            
            let loopback_ipv6 = NetworkAddress::new(
                IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                IpAddr::V6(Ipv6Addr::new(0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff)),
                None,
            );
            lo_interface.add_address(loopback_ipv6);
        }
        
        // Add common private network addresses for other interfaces
        for (name, interface) in self.interfaces.iter_mut() {
            if name != "lo" {
                // Add a common private IPv4 address
                let private_ip = match name.as_str() {
                    "eth0" | "en0" => Ipv4Addr::new(192, 168, 1, 100),
                    "wlan0" | "en1" => Ipv4Addr::new(192, 168, 1, 101),
                    _ => Ipv4Addr::new(192, 168, 1, 102),
                };
                
                let private_netmask = Ipv4Addr::new(255, 255, 255, 0);
                let private_broadcast = Ipv4Addr::new(192, 168, 1, 255);
                
                let private_addr = NetworkAddress::new(
                    IpAddr::V4(private_ip),
                    IpAddr::V4(private_netmask),
                    Some(IpAddr::V4(private_broadcast)),
                );
                interface.add_address(private_addr);
            }
        }
        
        Ok(())
    }
    
    /// Get information about a specific interface
    pub fn get_interface(&self, name: &str) -> Option<&NetworkInterface> {
        self.interfaces.get(name)
    }
    
    /// Get all interface names
    #[allow(dead_code)]
    pub fn get_interface_names(&self) -> Vec<String> {
        self.interfaces.keys().cloned().collect()
    }
    
    /// Get interfaces by type
    pub fn get_interfaces_by_type(&self, interface_type: InterfaceType) -> Vec<&NetworkInterface> {
        self.interfaces.values()
            .filter(|iface| iface.interface_type == interface_type)
            .collect()
    }
    
    /// Get interfaces that are currently up
    pub fn get_up_interfaces(&self) -> Vec<&NetworkInterface> {
        self.interfaces.values()
            .filter(|iface| iface.is_up)
            .collect()
    }
    
    /// Get the primary interface (first non-loopback interface)
    pub fn get_primary_interface(&self) -> Option<&NetworkInterface> {
        self.interfaces.values()
            .find(|iface| !iface.is_loopback && iface.is_up)
    }
    
    /// Get loopback interface
    pub fn get_loopback_interface(&self) -> Option<&NetworkInterface> {
        self.interfaces.values()
            .find(|iface| iface.is_loopback)
    }
    
    /// Get interface statistics (simplified)
    pub fn get_interface_stats(&self, name: &str) -> Option<InterfaceStats> {
        if let Some(interface) = self.interfaces.get(name) {
            Some(InterfaceStats {
                name: interface.name.clone(),
                is_up: interface.is_up,
                mtu: interface.mtu,
                address_count: interface.addresses.len(),
                has_ipv4: interface.get_primary_ipv4().is_some(),
                has_ipv6: interface.get_primary_ipv6().is_some(),
            })
        } else {
            None
        }
    }
    
    /// Get all interface statistics
    #[allow(dead_code)]
    pub fn get_all_interface_stats(&self) -> Vec<InterfaceStats> {
        self.interfaces.values()
            .map(|interface| InterfaceStats {
                name: interface.name.clone(),
                is_up: interface.is_up,
                mtu: interface.mtu,
                address_count: interface.addresses.len(),
                has_ipv4: interface.get_primary_ipv4().is_some(),
                has_ipv6: interface.get_primary_ipv6().is_some(),
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceStats {
    pub name: String,
    pub is_up: bool,
    pub mtu: u32,
    pub address_count: usize,
    pub has_ipv4: bool,
    pub has_ipv6: bool,
}

/// Helper function to get the best interface for binding
pub fn get_best_interface_for_binding<'a>(interfaces: &'a [&'a NetworkInterface], prefer_ipv4: bool) -> Option<&'a NetworkInterface> {
    // First, try to find a non-loopback interface with the preferred IP version
    for interface in interfaces {
        if !interface.is_loopback && interface.is_up {
            if prefer_ipv4 && interface.get_primary_ipv4().is_some() {
                return Some(interface);
            } else if !prefer_ipv4 && interface.get_primary_ipv6().is_some() {
                return Some(interface);
            }
        }
    }
    
    // Fallback: any non-loopback interface
    for interface in interfaces {
        if !interface.is_loopback && interface.is_up {
            return Some(interface);
        }
    }
    
    // Last resort: loopback interface
    interfaces.iter().find(|iface| iface.is_loopback).map(|v| *v)
}

/// Helper function to get interface by IP address
pub fn get_interface_by_ip<'a>(interfaces: &'a [&'a NetworkInterface], target_ip: IpAddr) -> Option<&'a NetworkInterface> {
    for interface in interfaces {
        for address in &interface.addresses {
            if address.ip == target_ip {
                return Some(interface);
            }
        }
    }
    None
}
