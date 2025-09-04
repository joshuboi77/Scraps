use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectionKey {
    pub host: String,
    pub port: u16,
    pub protocol: String, // "tcp", "tls", "http", "https"
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PooledConnection {
    pub connection_id: usize,
    pub connection_type: PooledConnectionType,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PooledConnectionType {
    Tcp(usize),
    Tls(usize),
    Http(String), // Store base URL for HTTP connections
}

pub struct ConnectionPoolManager {
    pools: HashMap<ConnectionKey, Vec<PooledConnection>>,
    max_connections_per_pool: usize,
    max_idle_time: Duration,
    max_connection_lifetime: Duration,
    #[allow(dead_code)]
    next_pool_id: usize,
}

impl ConnectionPoolManager {
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            max_connections_per_pool: 10,
            max_idle_time: Duration::from_secs(300), // 5 minutes
            max_connection_lifetime: Duration::from_secs(1800), // 30 minutes
            next_pool_id: 1,
        }
    }
    
    /// Get or create a connection from the pool
    #[allow(dead_code)]
    pub fn get_connection(&mut self, host: &str, port: u16, protocol: &str) -> Option<PooledConnection> {
        let key = ConnectionKey {
            host: host.to_string(),
            port,
            protocol: protocol.to_string(),
        };
        
        // Clean up expired connections first
        self.cleanup_expired_connections(&key);
        
        // Try to get an existing connection
        if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(mut conn) = pool.pop() {
                conn.last_used = Instant::now();
                conn.use_count += 1;
                return Some(conn);
            }
        }
        
        None // No available connection in pool
    }
    
    /// Return a connection to the pool
    #[allow(dead_code)]
    pub fn return_connection(&mut self, host: &str, port: u16, protocol: &str, connection: PooledConnection) -> Result<(), String> {
        let key = ConnectionKey {
            host: host.to_string(),
            port,
            protocol: protocol.to_string(),
        };
        
        // Don't return connection if it's too old or has been used too much
        if connection.created_at.elapsed() > self.max_connection_lifetime {
            return Ok(()); // Just drop the connection
        }
        
        let pool = self.pools.entry(key).or_insert_with(Vec::new);
        
        // Don't exceed max connections per pool
        if pool.len() < self.max_connections_per_pool {
            pool.push(connection);
        }
        
        Ok(())
    }
    
    /// Create a new pooled connection
    #[allow(dead_code)]
    pub fn create_pooled_connection(&mut self, connection_type: PooledConnectionType) -> PooledConnection {
        let id = self.next_pool_id;
        self.next_pool_id += 1;
        
        PooledConnection {
            connection_id: id,
            connection_type,
            created_at: Instant::now(),
            last_used: Instant::now(),
            use_count: 1,
        }
    }
    
    /// Clean up expired connections
    #[allow(dead_code)]
    fn cleanup_expired_connections(&mut self, key: &ConnectionKey) {
        if let Some(pool) = self.pools.get_mut(key) {
            let now = Instant::now();
            pool.retain(|conn| {
                now.duration_since(conn.last_used) <= self.max_idle_time &&
                now.duration_since(conn.created_at) <= self.max_connection_lifetime
            });
        }
    }
    
    /// Get pool statistics
    pub fn get_pool_stats(&self, host: &str, port: u16, protocol: &str) -> PoolStats {
        let key = ConnectionKey {
            host: host.to_string(),
            port,
            protocol: protocol.to_string(),
        };
        
        let pool_size = self.pools.get(&key).map_or(0, |p| p.len());
        
        PoolStats {
            pool_size,
            max_pool_size: self.max_connections_per_pool,
            total_pools: self.pools.len(),
        }
    }
    
    /// Clear all connections from all pools
    #[allow(dead_code)]
    pub fn clear_all_pools(&mut self) {
        self.pools.clear();
    }
    
    /// Clear connections for a specific host/port/protocol
    pub fn clear_pool(&mut self, host: &str, port: u16, protocol: &str) {
        let key = ConnectionKey {
            host: host.to_string(),
            port,
            protocol: protocol.to_string(),
        };
        self.pools.remove(&key);
    }
    
    /// Configure pool settings
    pub fn configure_pool(&mut self, max_connections: usize, max_idle_seconds: u64, max_lifetime_seconds: u64) {
        self.max_connections_per_pool = max_connections;
        self.max_idle_time = Duration::from_secs(max_idle_seconds);
        self.max_connection_lifetime = Duration::from_secs(max_lifetime_seconds);
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub pool_size: usize,
    pub max_pool_size: usize,
    pub total_pools: usize,
}
