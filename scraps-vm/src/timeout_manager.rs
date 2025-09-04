use std::time::Duration;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct NetworkTimeouts {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for NetworkTimeouts {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),  // 10 seconds
            read_timeout: Duration::from_secs(30),     // 30 seconds
            write_timeout: Duration::from_secs(30),    // 30 seconds
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimeoutKey {
    pub protocol: String,  // "tcp", "udp", "tls", "http", "ws"
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl TimeoutKey {
    #[allow(dead_code)]
    pub fn global(protocol: &str) -> Self {
        Self {
            protocol: protocol.to_string(),
            host: None,
            port: None,
        }
    }
    
    pub fn specific(protocol: &str, host: &str, port: u16) -> Self {
        Self {
            protocol: protocol.to_string(),
            host: Some(host.to_string()),
            port: Some(port),
        }
    }
}

pub struct TimeoutManager {
    // Global default timeouts for each protocol
    global_timeouts: HashMap<String, NetworkTimeouts>,
    // Specific timeouts for host:port combinations
    specific_timeouts: HashMap<TimeoutKey, NetworkTimeouts>,
    // Default fallback timeouts
    default_timeouts: NetworkTimeouts,
}

impl TimeoutManager {
    pub fn new() -> Self {
        let mut global_timeouts = HashMap::new();
        
        // Set protocol-specific defaults
        global_timeouts.insert("tcp".to_string(), NetworkTimeouts {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
        });
        
        global_timeouts.insert("udp".to_string(), NetworkTimeouts {
            connect_timeout: Duration::from_secs(5),   // UDP is connectionless, but for bind
            read_timeout: Duration::from_secs(10),     // Shorter for UDP
            write_timeout: Duration::from_secs(10),
        });
        
        global_timeouts.insert("tls".to_string(), NetworkTimeouts {
            connect_timeout: Duration::from_secs(15),  // TLS handshake takes longer
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
        });
        
        global_timeouts.insert("http".to_string(), NetworkTimeouts {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(60),     // HTTP responses can be large
            write_timeout: Duration::from_secs(30),
        });
        
        global_timeouts.insert("ws".to_string(), NetworkTimeouts {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(300),    // WebSocket can have long-lived connections
            write_timeout: Duration::from_secs(30),
        });
        
        Self {
            global_timeouts,
            specific_timeouts: HashMap::new(),
            default_timeouts: NetworkTimeouts::default(),
        }
    }
    
    /// Set global timeout for a protocol
    pub fn set_global_timeout(&mut self, protocol: &str, connect_ms: u64, read_ms: u64, write_ms: u64) {
        let timeouts = NetworkTimeouts {
            connect_timeout: Duration::from_millis(connect_ms),
            read_timeout: Duration::from_millis(read_ms),
            write_timeout: Duration::from_millis(write_ms),
        };
        self.global_timeouts.insert(protocol.to_string(), timeouts);
    }
    
    /// Set specific timeout for a host:port:protocol combination
    pub fn set_specific_timeout(&mut self, protocol: &str, host: &str, port: u16, connect_ms: u64, read_ms: u64, write_ms: u64) {
        let key = TimeoutKey::specific(protocol, host, port);
        let timeouts = NetworkTimeouts {
            connect_timeout: Duration::from_millis(connect_ms),
            read_timeout: Duration::from_millis(read_ms),
            write_timeout: Duration::from_millis(write_ms),
        };
        self.specific_timeouts.insert(key, timeouts);
    }
    
    /// Get timeout for a specific connection (checks specific first, then global, then default)
    pub fn get_timeout(&self, protocol: &str, host: Option<&str>, port: Option<u16>) -> NetworkTimeouts {
        // First, try specific timeout if host and port are provided
        if let (Some(h), Some(p)) = (host, port) {
            let key = TimeoutKey::specific(protocol, h, p);
            if let Some(timeouts) = self.specific_timeouts.get(&key) {
                return timeouts.clone();
            }
        }
        
        // Then try global timeout for the protocol
        if let Some(timeouts) = self.global_timeouts.get(protocol) {
            return timeouts.clone();
        }
        
        // Finally, use default timeouts
        self.default_timeouts.clone()
    }
    
    /// Remove specific timeout
    pub fn remove_specific_timeout(&mut self, protocol: &str, host: &str, port: u16) -> bool {
        let key = TimeoutKey::specific(protocol, host, port);
        self.specific_timeouts.remove(&key).is_some()
    }
    
    /// Get all timeout configurations
    pub fn get_all_timeouts(&self) -> TimeoutSummary {
        TimeoutSummary {
            global_count: self.global_timeouts.len(),
            specific_count: self.specific_timeouts.len(),
            default_timeouts: self.default_timeouts.clone(),
        }
    }
    
    /// Clear all specific timeouts
    pub fn clear_specific_timeouts(&mut self) {
        self.specific_timeouts.clear();
    }
    
    /// Get timeout info for a specific configuration
    pub fn get_timeout_info(&self, protocol: &str, host: Option<&str>, port: Option<u16>) -> TimeoutInfo {
        let timeouts = self.get_timeout(protocol, host, port);
        let source = if let (Some(h), Some(p)) = (host, port) {
            let key = TimeoutKey::specific(protocol, h, p);
            if self.specific_timeouts.contains_key(&key) {
                "specific"
            } else if self.global_timeouts.contains_key(protocol) {
                "global"
            } else {
                "default"
            }
        } else if self.global_timeouts.contains_key(protocol) {
            "global"
        } else {
            "default"
        };
        
        TimeoutInfo {
            timeouts,
            source: source.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeoutSummary {
    pub global_count: usize,
    pub specific_count: usize,
    pub default_timeouts: NetworkTimeouts,
}

#[derive(Debug, Clone)]
pub struct TimeoutInfo {
    pub timeouts: NetworkTimeouts,
    pub source: String,  // "specific", "global", or "default"
}
