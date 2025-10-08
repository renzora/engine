use std::path::Path;
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json;
use crate::modules::memory_cache::MemoryCache;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenScriptEntry {
    pub name: String,
    pub path: String,
    pub directory: String,
    pub full_path: String,
    pub searchable_text: String,
}

pub struct RenScriptCache {
    memory_cache_ref: Option<Arc<tokio::sync::Mutex<MemoryCache>>>,
    cache_key: String,
    local_memory_cache: Arc<RwLock<Vec<RenScriptEntry>>>,
}

impl RenScriptCache {
    pub fn new(memory_cache: Option<Arc<tokio::sync::Mutex<MemoryCache>>>) -> Self {
        Self {
            memory_cache_ref: memory_cache,
            cache_key: "renscripts:cache".to_string(),
            local_memory_cache: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn initialize(&self, renscripts_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔄 Initializing RenScript cache...");
        
        let scripts = Self::scan_directory_recursive(renscripts_path, "")?;
        
        // Store in local memory cache
        {
            let mut cache = self.local_memory_cache.write().await;
            *cache = scripts.clone();
            println!("✅ Cached {} RenScript entries in local memory", scripts.len());
        }
        
        // Also store in shared memory cache if available
        if let Some(memory_cache_ref) = &self.memory_cache_ref {
            let json_data = serde_json::to_string(&scripts)?;
            let memory_cache = memory_cache_ref.lock().await;
            if let Err(e) = memory_cache.set_string(&self.cache_key, &json_data).await {
                println!("⚠️ Failed to cache RenScript entries in shared memory cache: {}", e);
            } else {
                println!("✅ Also cached {} RenScript entries in shared memory cache", scripts.len());
            }
        }
        
        Ok(())
    }

    fn scan_directory_recursive(
        dir_path: &Path,
        relative_path: &str,
    ) -> Result<Vec<RenScriptEntry>, Box<dyn std::error::Error>> {
        let mut scripts = Vec::new();
        
        if !dir_path.exists() || !dir_path.is_dir() {
            return Ok(scripts);
        }

        let entries = fs::read_dir(dir_path)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            
            if path.is_dir() {
                // Recursively scan subdirectory
                let sub_relative = if relative_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", relative_path, name)
                };
                
                let sub_scripts = Self::scan_directory_recursive(&path, &sub_relative)?;
                scripts.extend(sub_scripts);
            } else if name.ends_with(".ren") {
                // Found a RenScript file
                let script_name = name.strip_suffix(".ren").unwrap_or(&name).to_string();
                let full_relative_path = if relative_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", relative_path, name)
                };
                
                let entry = RenScriptEntry {
                    name: script_name.clone(),
                    path: path.to_string_lossy().to_string(),
                    directory: relative_path.to_string(),
                    full_path: full_relative_path.clone(),
                    searchable_text: format!("{} {} {}", relative_path, name, script_name).to_lowercase(),
                };
                
                scripts.push(entry);
            }
        }
        
        Ok(scripts)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<RenScriptEntry>, Box<dyn std::error::Error>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let scripts = self.get_all_scripts().await?;
        let query_lower = query.to_lowercase();
        
        let filtered: Vec<RenScriptEntry> = scripts
            .into_iter()
            .filter(|script| script.searchable_text.contains(&query_lower))
            .collect();
        
        Ok(filtered)
    }

    pub async fn get_all_scripts(&self) -> Result<Vec<RenScriptEntry>, Box<dyn std::error::Error>> {
        // First try local memory cache
        {
            let cache = self.local_memory_cache.read().await;
            if !cache.is_empty() {
                return Ok(cache.clone());
            }
        }
        
        // Try shared memory cache if local cache is empty
        if let Some(memory_cache_ref) = &self.memory_cache_ref {
            let memory_cache = memory_cache_ref.lock().await;
            match memory_cache.get_string(&self.cache_key).await {
                Ok(Some(json_data)) => {
                    match serde_json::from_str::<Vec<RenScriptEntry>>(&json_data) {
                        Ok(scripts) => {
                            // Update local memory cache
                            {
                                let mut cache = self.local_memory_cache.write().await;
                                *cache = scripts.clone();
                            }
                            return Ok(scripts);
                        },
                        Err(e) => {
                            println!("⚠️ Failed to deserialize RenScript cache: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    println!("⚠️ RenScript cache not found in shared memory cache");
                }
                Err(e) => {
                    println!("⚠️ Failed to get RenScript cache from shared memory cache: {}", e);
                }
            }
        }
        
        // Return empty if no cache is available
        Ok(Vec::new())
    }

    pub async fn refresh_cache(&self, renscripts_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.initialize(renscripts_path).await
    }
}