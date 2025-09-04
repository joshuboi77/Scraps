use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum ProxyType {
    Http,
    Https,
    Socks4,
    Socks5,
}

impl ProxyType {
    pub fn from_string(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "http" => Ok(ProxyType::Http),
            "https" => Ok(ProxyType::Https),
            "socks4" => Ok(ProxyType::Socks4),
            "socks5" => Ok(ProxyType::Socks5),
            _ => Err(format!("Unknown proxy type: {}", s)),
        }
    }
    
    pub fn to_string(&self) -> String {
        match self {
            ProxyType::Http => "http".to_string(),
            ProxyType::Https => "https".to_string(),
            ProxyType::Socks4 => "socks4".to_string(),
            ProxyType::Socks5 => "socks5".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub proxy_type: ProxyType,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub enabled: bool,
}

impl ProxyConfig {
    pub fn new(proxy_type: ProxyType, host: String, port: u16) -> Self {
        Self {
            proxy_type,
            host,
            port,
            username: None,
            password: None,
            enabled: true,
        }
    }
    
    pub fn with_auth(mut self, username: String, password: String) -> Self {
        self.username = Some(username);
        self.password = Some(password);
        self
    }
    
    #[allow(dead_code)]
    pub fn url(&self) -> String {
        let auth = match (&self.username, &self.password) {
            (Some(user), Some(pass)) => format!("{}:{}@", user, pass),
            _ => String::new(),
        };
        format!("{}://{}{}:{}", self.proxy_type.to_string(), auth, self.host, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProxyKey {
    pub protocol: String,  // "http", "https", "tcp", "tls", "ws"
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
}

impl ProxyKey {
    #[allow(dead_code)]
    pub fn global(protocol: &str) -> Self {
        Self {
            protocol: protocol.to_string(),
            target_host: None,
            target_port: None,
        }
    }
    
    pub fn specific(protocol: &str, host: &str, port: u16) -> Self {
        Self {
            protocol: protocol.to_string(),
            target_host: Some(host.to_string()),
            target_port: Some(port),
        }
    }
}

pub struct ProxyManager {
    // Global proxy configs for each protocol
    global_proxies: HashMap<String, ProxyConfig>,
    // Specific proxy configs for host:port combinations
    specific_proxies: HashMap<ProxyKey, ProxyConfig>,
    // Default proxy (if set, applies to all unless overridden)
    default_proxy: Option<ProxyConfig>,
    // Proxy bypass list (hosts that should not use proxy)
    bypass_list: Vec<String>,
}

impl ProxyManager {
    pub fn new() -> Self {
        Self {
            global_proxies: HashMap::new(),
            specific_proxies: HashMap::new(),
            default_proxy: None,
            bypass_list: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ],
        }
    }
    
    /// Set global proxy for a protocol
    pub fn set_global_proxy(&mut self, protocol: &str, proxy_type: &str, host: &str, port: u16, username: Option<String>, password: Option<String>) -> Result<(), String> {
        let ptype = ProxyType::from_string(proxy_type)?;
        let mut config = ProxyConfig::new(ptype, host.to_string(), port);
        if let (Some(user), Some(pass)) = (username, password) {
            config = config.with_auth(user, pass);
        }
        self.global_proxies.insert(protocol.to_string(), config);
        Ok(())
    }
    
    /// Set specific proxy for a target host:port:protocol combination
    pub fn set_specific_proxy(&mut self, protocol: &str, target_host: &str, target_port: u16, proxy_type: &str, proxy_host: &str, proxy_port: u16, username: Option<String>, password: Option<String>) -> Result<(), String> {
        let ptype = ProxyType::from_string(proxy_type)?;
        let mut config = ProxyConfig::new(ptype, proxy_host.to_string(), proxy_port);
        if let (Some(user), Some(pass)) = (username, password) {
            config = config.with_auth(user, pass);
        }
        let key = ProxyKey::specific(protocol, target_host, target_port);
        self.specific_proxies.insert(key, config);
        Ok(())
    }
    
    /// Set default proxy for all connections
    pub fn set_default_proxy(&mut self, proxy_type: &str, host: &str, port: u16, username: Option<String>, password: Option<String>) -> Result<(), String> {
        let ptype = ProxyType::from_string(proxy_type)?;
        let mut config = ProxyConfig::new(ptype, host.to_string(), port);
        if let (Some(user), Some(pass)) = (username, password) {
            config = config.with_auth(user, pass);
        }
        self.default_proxy = Some(config);
        Ok(())
    }
    
    /// Get proxy config for a specific connection
    pub fn get_proxy(&self, protocol: &str, target_host: Option<&str>, target_port: Option<u16>) -> Option<ProxyConfig> {
        // Check if target should bypass proxy
        if let Some(host) = target_host {
            if self.should_bypass(host) {
                return None;
            }
        }
        
        // Check specific proxy first
        if let (Some(host), Some(port)) = (target_host, target_port) {
            let key = ProxyKey::specific(protocol, host, port);
            if let Some(config) = self.specific_proxies.get(&key) {
                if config.enabled {
                    return Some(config.clone());
                }
            }
        }
        
        // Check global proxy for protocol
        if let Some(config) = self.global_proxies.get(protocol) {
            if config.enabled {
                return Some(config.clone());
            }
        }
        
        // Check default proxy
        if let Some(config) = &self.default_proxy {
            if config.enabled {
                return Some(config.clone());
            }
        }
        
        None
    }
    
    /// Check if a host should bypass proxy
    fn should_bypass(&self, host: &str) -> bool {
        for bypass_host in &self.bypass_list {
            if host.eq_ignore_ascii_case(bypass_host) || 
               (bypass_host.starts_with('.') && host.ends_with(bypass_host)) {
                return true;
            }
        }
        false
    }
    
    /// Add host to proxy bypass list
    pub fn add_bypass(&mut self, host: &str) {
        if !self.bypass_list.contains(&host.to_string()) {
            self.bypass_list.push(host.to_string());
        }
    }
    
    /// Remove host from proxy bypass list
    pub fn remove_bypass(&mut self, host: &str) -> bool {
        if let Some(pos) = self.bypass_list.iter().position(|x| x == host) {
            self.bypass_list.remove(pos);
            true
        } else {
            false
        }
    }
    
    /// Clear all proxy configurations
    pub fn clear_all(&mut self) {
        self.global_proxies.clear();
        self.specific_proxies.clear();
        self.default_proxy = None;
    }
    
    /// Remove specific proxy
    pub fn remove_specific_proxy(&mut self, protocol: &str, target_host: &str, target_port: u16) -> bool {
        let key = ProxyKey::specific(protocol, target_host, target_port);
        self.specific_proxies.remove(&key).is_some()
    }
    
    /// Remove global proxy for protocol
    pub fn remove_global_proxy(&mut self, protocol: &str) -> bool {
        self.global_proxies.remove(protocol).is_some()
    }
    
    /// Get proxy statistics
    pub fn get_stats(&self) -> ProxyStats {
        ProxyStats {
            global_count: self.global_proxies.len(),
            specific_count: self.specific_proxies.len(),
            has_default: self.default_proxy.is_some(),
            bypass_count: self.bypass_list.len(),
        }
    }
    
    /// Get all bypass hosts
    pub fn get_bypass_list(&self) -> Vec<String> {
        self.bypass_list.clone()
    }
    
    /// Get proxy info for a specific connection
    pub fn get_proxy_info(&self, protocol: &str, target_host: Option<&str>, target_port: Option<u16>) -> ProxyInfo {
        let proxy_config = self.get_proxy(protocol, target_host, target_port);
        let source = if let (Some(host), Some(port)) = (target_host, target_port) {
            let key = ProxyKey::specific(protocol, host, port);
            if self.specific_proxies.contains_key(&key) {
                "specific"
            } else if self.global_proxies.contains_key(protocol) {
                "global"
            } else if self.default_proxy.is_some() {
                "default"
            } else {
                "none"
            }
        } else if self.global_proxies.contains_key(protocol) {
            "global"
        } else if self.default_proxy.is_some() {
            "default"
        } else {
            "none"
        };
        
        ProxyInfo {
            proxy_config,
            source: source.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyStats {
    pub global_count: usize,
    pub specific_count: usize,
    pub has_default: bool,
    pub bypass_count: usize,
}

#[derive(Debug, Clone)]
pub struct ProxyInfo {
    pub proxy_config: Option<ProxyConfig>,
    pub source: String,  // "specific", "global", "default", or "none"
}
