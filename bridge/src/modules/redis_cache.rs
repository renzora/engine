use redis::{RedisResult, Connection, Commands, Client};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, warn, debug, error};

pub struct RedisCache {
    connection: Option<Connection>,
    client: Option<Client>,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSearchResult {
    pub name: String,
    pub path: String,
    pub directory: String,
    pub last_modified: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedScriptList {
    pub scripts: Vec<ScriptSearchResult>,
    pub timestamp: u64,
    pub total_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectManifest {
    pub project_name: String,
    pub last_scan: u64,
    pub file_count: usize,
    pub checksum: String,
    pub cache_version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileMetadata {
    pub path: String,
    pub last_modified: u64,
    pub file_size: u64,
    pub hash: String,
    pub processed_at: u64,
    pub file_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessedAsset {
    pub path: String,
    pub file_type: String,
    pub metadata: serde_json::Value,
    pub thumbnail_path: Option<String>,
    pub compressed_path: Option<String>,
    pub extracted_materials: Option<Vec<String>>,
    pub processing_status: String,
    pub processed_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheValidationResult {
    pub cache_status: String, // "valid", "needs_update", "missing"
    pub changes_detected: usize,
    pub estimated_processing_time: u64,
    pub change_summary: ChangeSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeSummary {
    pub new_files: usize,
    pub modified_files: usize,
    pub deleted_files: usize,
    pub moved_files: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CachedAssetNode {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub file_size: Option<u64>,
    pub last_modified: Option<u64>,
    pub extension: Option<String>,
    pub file_type: Option<String>,
    pub thumbnail_url: Option<String>,
    pub children: Option<Vec<CachedAssetNode>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectAssetTree {
    pub project_name: String,
    pub root_path: String,
    pub assets: Vec<CachedAssetNode>,
    pub generated_at: u64,
    pub total_files: usize,
    pub total_directories: usize,
}

impl RedisCache {
    pub fn new() -> Self {
        let mut cache = RedisCache {
            connection: None,
            client: None,
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
        
        // Retry connection a few times in case embedded server is still starting up
        let mut last_error = None;
        for attempt in 1..=3 {
            match client.get_connection() {
                Ok(mut connection) => {
                    // Test the connection
                    match redis::cmd("PING").query::<String>(&mut connection) {
                        Ok(_) => {
                            debug!("🔴 Redis connection established on attempt {}", attempt);
                            self.connection = Some(connection);
                            self.client = Some(client);
                            return Ok(());
                        }
                        Err(e) => {
                            last_error = Some(e);
                            if attempt < 3 {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| {
            redis::RedisError::from((redis::ErrorKind::IoError, "Failed to connect after retries"))
        }))
    }

    // Reconnect if connection is lost
    fn ensure_connection(&mut self) -> RedisResult<()> {
        if let Some(ref mut conn) = self.connection {
            // Test if connection is still alive
            match redis::cmd("PING").query::<String>(conn) {
                Ok(_) => return Ok(()), // Connection is fine
                Err(_) => {
                    warn!("🔴 Redis connection lost, attempting to reconnect...");
                    self.connection = None;
                }
            }
        }

        // Reconnect using stored client
        if let Some(ref client) = self.client {
            match client.get_connection() {
                Ok(connection) => {
                    info!("🔴 Redis connection restored");
                    self.connection = Some(connection);
                    Ok(())
                }
                Err(e) => {
                    error!("🔴 Failed to restore Redis connection: {}", e);
                    self.enabled = false;
                    Err(e)
                }
            }
        } else {
            // Try to reconnect from scratch
            self.try_connect()
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled && self.connection.is_some()
    }

    pub fn cache_script_list(&mut self, scripts: &[ScriptSearchResult]) -> bool {
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

    pub fn get_cached_script_list(&mut self) -> Option<Vec<ScriptSearchResult>> {
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

    pub async fn set_string(&mut self, key: &str, value: &str) -> Result<(), String> {
        if !self.is_enabled() {
            return Err("Redis not enabled".to_string());
        }

        // Ensure connection is alive
        if let Err(e) = self.ensure_connection() {
            return Err(format!("Failed to ensure Redis connection: {}", e));
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let result: RedisResult<()> = conn.set_ex(key, value, 300); // 5 minutes TTL
                result.map_err(|e| format!("Failed to set value in Redis: {}", e))
            }
            None => Err("Redis connection not available".to_string())
        }
    }

    pub async fn get_string(&mut self, key: &str) -> Result<Option<String>, String> {
        if !self.is_enabled() {
            return Ok(None);
        }

        // Ensure connection is alive
        if let Err(e) = self.ensure_connection() {
            warn!("Failed to ensure Redis connection for get: {}", e);
            return Ok(None);
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let result: RedisResult<String> = conn.get(key);
                match result {
                    Ok(value) => Ok(Some(value)),
                    Err(_) => Ok(None), // Key doesn't exist or other error
                }
            }
            None => Ok(None)
        }
    }

    // Project Asset Cache Methods

    pub fn cache_project_manifest(&mut self, manifest: &ProjectManifest) -> bool {
        if !self.is_enabled() {
            return false;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:manifest", manifest.project_name);
                match serde_json::to_string(manifest) {
                    Ok(json) => {
                        let result: RedisResult<()> = conn.set_ex(&key, json, 86400); // 24 hours TTL
                        match result {
                            Ok(_) => {
                                info!("🔴 Cached project manifest for: {}", manifest.project_name);
                                true
                            }
                            Err(e) => {
                                warn!("🔴 Failed to cache project manifest: {}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        warn!("🔴 Failed to serialize project manifest: {}", e);
                        false
                    }
                }
            }
            None => false,
        }
    }

    pub fn get_project_manifest(&mut self, project_name: &str) -> Option<ProjectManifest> {
        if !self.is_enabled() {
            return None;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:manifest", project_name);
                let result: RedisResult<String> = conn.get(&key);
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<ProjectManifest>(&json) {
                            Ok(manifest) => {
                                info!("🔴 Retrieved project manifest from cache: {}", project_name);
                                Some(manifest)
                            }
                            Err(e) => {
                                warn!("🔴 Failed to deserialize project manifest: {}", e);
                                None
                            }
                        }
                    }
                    Err(_) => {
                        debug!("🔴 No cached project manifest found: {}", project_name);
                        None
                    }
                }
            }
            None => None,
        }
    }

    pub fn cache_file_metadata(&mut self, project_name: &str, file_metadata: &[FileMetadata]) -> bool {
        if !self.is_enabled() {
            return false;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:files", project_name);
                
                // Store as hash map for efficient lookups
                for metadata in file_metadata {
                    match serde_json::to_string(metadata) {
                        Ok(json) => {
                            let result: RedisResult<()> = conn.hset(&key, &metadata.path, json);
                            if let Err(e) = result {
                                warn!("🔴 Failed to cache file metadata for {}: {}", metadata.path, e);
                                return false;
                            }
                        }
                        Err(e) => {
                            warn!("🔴 Failed to serialize file metadata: {}", e);
                            return false;
                        }
                    }
                }
                
                // Set TTL on the hash
                let _: RedisResult<()> = conn.expire(&key, 86400); // 24 hours
                info!("🔴 Cached {} file metadata entries for project: {}", file_metadata.len(), project_name);
                true
            }
            None => false,
        }
    }

    pub fn get_file_metadata(&mut self, project_name: &str, file_path: &str) -> Option<FileMetadata> {
        if !self.is_enabled() {
            return None;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:files", project_name);
                let result: RedisResult<String> = conn.hget(&key, file_path);
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<FileMetadata>(&json) {
                            Ok(metadata) => Some(metadata),
                            Err(e) => {
                                warn!("🔴 Failed to deserialize file metadata: {}", e);
                                None
                            }
                        }
                    }
                    Err(_) => None,
                }
            }
            None => None,
        }
    }

    pub fn get_all_file_metadata(&mut self, project_name: &str) -> Vec<FileMetadata> {
        if !self.is_enabled() {
            return Vec::new();
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:files", project_name);
                let result: RedisResult<std::collections::HashMap<String, String>> = conn.hgetall(&key);
                match result {
                    Ok(hash_map) => {
                        let mut metadata_list = Vec::new();
                        for (_path, json) in hash_map {
                            match serde_json::from_str::<FileMetadata>(&json) {
                                Ok(metadata) => metadata_list.push(metadata),
                                Err(e) => warn!("🔴 Failed to deserialize file metadata: {}", e),
                            }
                        }
                        if !metadata_list.is_empty() {
                            info!("🔴 Retrieved {} file metadata entries from cache for project: {}", metadata_list.len(), project_name);
                        }
                        metadata_list
                    }
                    Err(_) => Vec::new(),
                }
            }
            None => Vec::new(),
        }
    }

    pub fn cache_processed_asset(&mut self, project_name: &str, asset: &ProcessedAsset) -> bool {
        if !self.is_enabled() {
            return false;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:processed", project_name);
                match serde_json::to_string(asset) {
                    Ok(json) => {
                        let result: RedisResult<()> = conn.hset(&key, &asset.path, json);
                        match result {
                            Ok(_) => {
                                // Set TTL on the hash
                                let _: RedisResult<()> = conn.expire(&key, 86400); // 24 hours
                                info!("🔴 Cached processed asset: {} (thumbnail: {:?})", asset.path, asset.thumbnail_path);
                                true
                            }
                            Err(e) => {
                                warn!("🔴 Failed to cache processed asset: {}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        warn!("🔴 Failed to serialize processed asset: {}", e);
                        false
                    }
                }
            }
            None => false,
        }
    }

    pub fn get_processed_asset(&mut self, project_name: &str, file_path: &str) -> Option<ProcessedAsset> {
        if !self.is_enabled() {
            return None;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:processed", project_name);
                let result: RedisResult<String> = conn.hget(&key, file_path);
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<ProcessedAsset>(&json) {
                            Ok(asset) => Some(asset),
                            Err(e) => {
                                warn!("🔴 Failed to deserialize processed asset: {}", e);
                                None
                            }
                        }
                    }
                    Err(_) => None,
                }
            }
            None => None,
        }
    }

    pub fn get_all_processed_assets(&mut self, project_name: &str) -> Vec<ProcessedAsset> {
        if !self.is_enabled() {
            return Vec::new();
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:processed", project_name);
                let result: RedisResult<std::collections::HashMap<String, String>> = conn.hgetall(&key);
                match result {
                    Ok(hash_map) => {
                        let mut assets = Vec::new();
                        for (_path, json) in hash_map {
                            match serde_json::from_str::<ProcessedAsset>(&json) {
                                Ok(asset) => {
                                    debug!("🔍 Retrieved asset: {} (thumbnail: {:?})", asset.path, asset.thumbnail_path);
                                    assets.push(asset);
                                }
                                Err(e) => warn!("🔴 Failed to deserialize processed asset: {}", e),
                            }
                        }
                        if !assets.is_empty() {
                            info!("🔴 Retrieved {} processed assets from cache for project: {}", assets.len(), project_name);
                            let with_thumbnails = assets.iter().filter(|a| a.thumbnail_path.is_some()).count();
                            info!("🖼️ Assets with thumbnails: {}/{}", with_thumbnails, assets.len());
                        }
                        assets
                    }
                    Err(_) => Vec::new(),
                }
            }
            None => Vec::new(),
        }
    }

    pub fn cache_project_asset_tree(&mut self, tree: &ProjectAssetTree) -> bool {
        if !self.is_enabled() {
            return false;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:asset_tree", tree.project_name);
                match serde_json::to_string(tree) {
                    Ok(json) => {
                        let result: RedisResult<()> = conn.set_ex(&key, json, 86400); // 24 hours TTL
                        match result {
                            Ok(_) => {
                                info!("🔴 Cached project asset tree for: {} ({} files, {} directories)", 
                                      tree.project_name, tree.total_files, tree.total_directories);
                                true
                            }
                            Err(e) => {
                                warn!("🔴 Failed to cache project asset tree: {}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        warn!("🔴 Failed to serialize project asset tree: {}", e);
                        false
                    }
                }
            }
            None => false,
        }
    }

    pub fn get_project_asset_tree(&mut self, project_name: &str) -> Option<ProjectAssetTree> {
        if !self.is_enabled() {
            return None;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let key = format!("project:{}:asset_tree", project_name);
                let result: RedisResult<String> = conn.get(&key);
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<ProjectAssetTree>(&json) {
                            Ok(tree) => {
                                info!("🔴 Retrieved project asset tree from cache: {} ({} files, {} directories)", 
                                      project_name, tree.total_files, tree.total_directories);
                                Some(tree)
                            }
                            Err(e) => {
                                warn!("🔴 Failed to deserialize project asset tree: {}", e);
                                None
                            }
                        }
                    }
                    Err(_) => {
                        debug!("🔴 No cached project asset tree found: {}", project_name);
                        None
                    }
                }
            }
            None => None,
        }
    }

    pub fn clear_project_cache(&mut self, project_name: &str) -> bool {
        if !self.is_enabled() {
            return false;
        }

        match self.connection.as_mut() {
            Some(conn) => {
                let keys = vec![
                    format!("project:{}:manifest", project_name),
                    format!("project:{}:files", project_name),
                    format!("project:{}:processed", project_name),
                    format!("project:{}:asset_tree", project_name),
                ];
                
                let result: RedisResult<()> = conn.del(keys.clone());
                match result {
                    Ok(_) => {
                        info!("🔴 Cleared project cache for: {}", project_name);
                        true
                    }
                    Err(e) => {
                        warn!("🔴 Failed to clear project cache: {}", e);
                        false
                    }
                }
            }
            None => false,
        }
    }
}