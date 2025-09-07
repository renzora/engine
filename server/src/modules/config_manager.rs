use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::types::ServerConfig;
use crate::config::Config;

// Global configuration state
static CONFIG_STATE: std::sync::OnceLock<Arc<RwLock<Config>>> = std::sync::OnceLock::new();

pub fn init_config_manager(config: Config) {
    let _ = CONFIG_STATE.set(Arc::new(RwLock::new(config)));
    info!("🔧 Configuration manager initialized");
}

pub async fn get_current_server_config() -> ServerConfig {
    let config_lock = CONFIG_STATE.get().expect("Config manager not initialized");
    let config = config_lock.read().await;
    
    ServerConfig {
        base_path: config.get_base_path().to_string_lossy().to_string(),
        projects_path: config.get_projects_path().to_string_lossy().to_string(),
        port: config.server.port,
        host: config.server.host.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

pub async fn set_base_path(new_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(new_path);
    
    // Validate that the path exists
    if !path.exists() {
        return Err(format!("Path does not exist: {}", new_path).into());
    }
    
    // Validate that it looks like an engine root
    if !is_engine_root(&path) {
        warn!("⚠️ Path doesn't appear to be an engine root: {}", new_path);
        // Don't fail, just warn - user might know what they're doing
    }
    
    let config_lock = CONFIG_STATE.get().expect("Config manager not initialized");
    let mut config = config_lock.write().await;
    
    // Update the configuration
    config.paths.base_path = Some(new_path.to_string());
    
    // Save to config file if possible
    if let Err(e) = config.save("renzora.toml") {
        warn!("Failed to save configuration: {}", e);
    }
    
    info!("📂 Base path updated to: {}", new_path);
    Ok(())
}

pub async fn set_projects_path(new_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(new_path);
    
    // Create the directory if it doesn't exist
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
        info!("📁 Created projects directory: {}", new_path);
    }
    
    let config_lock = CONFIG_STATE.get().expect("Config manager not initialized");
    let mut config = config_lock.write().await;
    
    // Update the configuration
    config.paths.projects_path = Some(new_path.to_string());
    
    // Save to config file if possible
    if let Err(e) = config.save("renzora.toml") {
        warn!("Failed to save configuration: {}", e);
    }
    
    info!("📂 Projects path updated to: {}", new_path);
    Ok(())
}

pub async fn scan_for_engine_roots() -> Result<(Vec<String>, String), Box<dyn std::error::Error>> {
    // Get current configuration
    let current_path = {
        let config_lock = CONFIG_STATE.get().expect("Config manager not initialized");
        let config = config_lock.read().await;
        config.get_base_path().to_string_lossy().to_string()
    };
    
    // No need to scan - frontend will tell us where things are
    // Just return current path for now
    info!("📍 Current engine path: {}", current_path);
    
    Ok((vec![current_path.clone()], current_path))
}

fn is_engine_root(path: &PathBuf) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }
    
    // Check for key files/directories that indicate the engine root
    let required_indicators = [
        "package.json",
        "src",
    ];
    
    let optional_indicators = [
        "bridge",
        "server", 
        "rspack.config.js",
        "projects",
        "assets",
    ];
    
    // Must have all required indicators
    let has_required = required_indicators.iter().all(|indicator| {
        path.join(indicator).exists()
    });
    
    if !has_required {
        return false;
    }
    
    // Should have at least one optional indicator
    let has_optional = optional_indicators.iter().any(|indicator| {
        path.join(indicator).exists()
    });
    
    has_optional
}