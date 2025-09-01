use redis::{RedisResult, Connection, Commands};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, warn, debug};

pub struct RedisCache {
    connection: Option<Connection>,
    enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedScriptList {
    pub scripts: Vec<crate::modules::database::ScriptSearchResult>,
    pub timestamp: u64,
    pub total_count: usize,
}

impl RedisCache {
    pub fn new() -> Self {
        let mut cache = RedisCache {
            connection: None,
            enabled: false,
        };
        
        // Try to connect to Redis
        match cache.try_connect() {
            Ok(_) => {
                info!("🔴 Redis cache connected and ready");
                cache.enabled = true;
            }
            Err(e) => {
                warn!("🔴 Redis not available, caching disabled: {}", e);
                cache.enabled = false;
            }
        }
        
        cache
    }

    fn try_connect(&mut self) -> RedisResult<()> {
        let client = redis::Client::open("redis://127.0.0.1:6379/")?;
        let mut connection = client.get_connection()?;
        
        // Test the connection
        let _: String = redis::cmd("PING").query(&mut connection)?;
        
        self.connection = Some(connection);
        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled && self.connection.is_some()
    }

    pub fn cache_script_list(&mut self, scripts: &[crate::modules::database::ScriptSearchResult]) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let cached_data = CachedScriptList {
            scripts: scripts.to_vec(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            total_count: scripts.len(),
        };

        match self.connection.as_mut() {
            Some(conn) => {
                match serde_json::to_string(&cached_data) {
                    Ok(json) => {
                        let result: RedisResult<()> = conn.set_ex("renscripts:list", json, 300); // 5 minutes TTL
                        match result {
                            Ok(_) => {
                                debug!("🔴 Cached {} scripts in Redis", scripts.len());
                                true
                            }
                            Err(e) => {
                                warn!("🔴 Failed to cache script list: {}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        warn!("🔴 Failed to serialize script list: {}", e);
                        false
                    }
                }
            }
            None => false,
        }
    }

    pub fn get_cached_script_list(&mut self) -> Option<Vec<crate::modules::database::ScriptSearchResult>> {
        if !self.is_enabled() {
            return None;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let result: RedisResult<String> = conn.get("renscripts:list");
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<CachedScriptList>(&json) {
                            Ok(cached_data) => {
                                // Check if cache is still fresh (within 5 minutes)
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                
                                if now - cached_data.timestamp < 300 {
                                    debug!("🔴 Retrieved {} scripts from Redis cache", cached_data.scripts.len());
                                    Some(cached_data.scripts)
                                } else {
                                    debug!("🔴 Redis cache expired, returning None");
                                    None
                                }
                            }
                            Err(e) => {
                                warn!("🔴 Failed to deserialize cached script list: {}", e);
                                None
                            }
                        }
                    }
                    Err(_) => {
                        debug!("🔴 No cached script list found in Redis");
                        None
                    }
                }
            }
            None => None,
        }
    }

    pub fn cache_compiled_script(&mut self, script_name: &str, compiled_js: &str) -> bool {
        if !self.is_enabled() {
            return false;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("renscript:compiled:{}", script_name);
                let result: RedisResult<()> = conn.set_ex(key, compiled_js, 600); // 10 minutes TTL
                match result {
                    Ok(_) => {
                        debug!("🔴 Cached compiled script: {}", script_name);
                        true
                    }
                    Err(e) => {
                        warn!("🔴 Failed to cache compiled script {}: {}", script_name, e);
                        false
                    }
                }
            }
            None => false,
        }
    }

    pub fn get_cached_compiled_script(&mut self, script_name: &str) -> Option<String> {
        if !self.is_enabled() {
            return None;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("renscript:compiled:{}", script_name);
                let result: RedisResult<String> = conn.get(key);
                match result {
                    Ok(compiled_js) => {
                        debug!("🔴 Retrieved compiled script from cache: {}", script_name);
                        Some(compiled_js)
                    }
                    Err(_) => {
                        debug!("🔴 No cached compiled script found: {}", script_name);
                        None
                    }
                }
            }
            None => None,
        }
    }

    pub fn clear_all_cache(&mut self) -> bool {
        if !self.is_enabled() {
            return false;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                // Clear all renscript-related keys
                let pattern = "renscript*";
                let result: RedisResult<Vec<String>> = conn.keys(pattern);
                
                match result {
                    Ok(keys) => {
                        if !keys.is_empty() {
                            let del_result: RedisResult<()> = conn.del(keys.clone());
                            match del_result {
                                Ok(_) => {
                                    info!("🔴 Cleared {} Redis cache keys", keys.len());
                                    true
                                }
                                Err(e) => {
                                    warn!("🔴 Failed to clear Redis cache: {}", e);
                                    false
                                }
                            }
                        } else {
                            debug!("🔴 No Redis cache keys to clear");
                            true
                        }
                    }
                    Err(e) => {
                        warn!("🔴 Failed to get Redis keys for clearing: {}", e);
                        false
                    }
                }
            }
            None => false,
        }
    }

    pub fn get_cache_stats(&mut self) -> serde_json::Value {
        if !self.is_enabled() {
            return serde_json::json!({
                "redis_enabled": false,
                "connection_status": "disabled"
            });
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let info_result: RedisResult<String> = redis::cmd("INFO").query(conn);
                let key_count_result: RedisResult<Vec<String>> = conn.keys("renscript*");
                
                let connection_status = match info_result {
                    Ok(_) => "connected",
                    Err(_) => "connection_error",
                };

                let cached_keys = match key_count_result {
                    Ok(keys) => keys.len(),
                    Err(_) => 0,
                };

                serde_json::json!({
                    "redis_enabled": true,
                    "connection_status": connection_status,
                    "cached_keys": cached_keys,
                    "cache_ttl": "5 minutes for script lists, 10 minutes for compiled scripts"
                })
            }
            None => serde_json::json!({
                "redis_enabled": false,
                "connection_status": "no_connection"
            })
        }
    }

    pub async fn set_string(&self, key: &str, value: &str) -> Result<(), String> {
        if !self.is_enabled() {
            return Err("Redis not enabled".to_string());
        }

        // Since we can't modify self.connection (it's not mutable), we'll create a new connection
        let client = redis::Client::open("redis://127.0.0.1:6379/")
            .map_err(|e| format!("Failed to create Redis client: {}", e))?;
        let mut conn = client.get_connection()
            .map_err(|e| format!("Failed to get Redis connection: {}", e))?;
        
        let _: () = conn.set_ex(key, value, 300) // 5 minutes TTL
            .map_err(|e| format!("Failed to set value in Redis: {}", e))?;
        
        Ok(())
    }

    pub async fn get_string(&self, key: &str) -> Result<Option<String>, String> {
        if !self.is_enabled() {
            return Ok(None);
        }

        // Since we can't modify self.connection (it's not mutable), we'll create a new connection
        let client = redis::Client::open("redis://127.0.0.1:6379/")
            .map_err(|e| format!("Failed to create Redis client: {}", e))?;
        let mut conn = client.get_connection()
            .map_err(|e| format!("Failed to get Redis connection: {}", e))?;
        
        let result: RedisResult<String> = conn.get(key);
        match result {
            Ok(value) => Ok(Some(value)),
            Err(_) => Ok(None), // Key doesn't exist or other error
        }
    }
}