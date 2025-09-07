use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub paths: PathsConfig,
    pub logging: LoggingConfig,
    pub features: FeaturesConfig,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub workers: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PathsConfig {
    pub base_path: Option<String>,
    pub projects_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_watching: bool,
    pub connections: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeaturesConfig {
    pub file_watching: bool,
    pub auto_create_projects: bool,
    pub system_stats: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    pub file_change_buffer: usize,
    pub message_timeout: u64,
    pub max_reconnect_attempts: usize,
    pub reconnect_delay: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                port: 3002,
                host: "127.0.0.1".to_string(),
                workers: None,
            },
            paths: PathsConfig {
                base_path: None,
                projects_path: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file_watching: true,
                connections: true,
            },
            features: FeaturesConfig {
                file_watching: true,
                auto_create_projects: true,
                system_stats: true,
            },
            performance: PerformanceConfig {
                file_change_buffer: 1000,
                message_timeout: 30,
                max_reconnect_attempts: 5,
                reconnect_delay: 2000,
            },
        }
    }
}

impl Config {
    pub fn load() -> Self {
        // Try to load config from multiple locations
        let config_paths = [
            "renzora.toml",
            "server/renzora.toml",
            "../renzora.toml",
            "config/renzora.toml",
        ];
        
        for config_path in &config_paths {
            if let Ok(config) = Self::load_from_file(config_path) {
                info!("📋 Loaded configuration from: {}", config_path);
                return config;
            }
        }
        
        info!("📋 Using default configuration (no renzora.toml found)");
        Self::default()
    }
    
    fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    pub fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        info!("💾 Saved configuration to: {}", path);
        Ok(())
    }
    
    pub fn get_base_path(&self) -> PathBuf {
        if let Some(ref path) = self.paths.base_path {
            return PathBuf::from(path);
        }
        crate::state::get_base_path()
    }
    
    pub fn get_projects_path(&self) -> PathBuf {
        if let Some(ref path) = self.paths.projects_path {
            return PathBuf::from(path);
        }
        self.get_base_path().join("projects")
    }
    
    pub fn get_port(&self) -> String {
        std::env::var("RENZORA_PORT")
            .unwrap_or_else(|_| self.server.port.to_string())
    }
    
    pub fn get_host(&self) -> String {
        std::env::var("RENZORA_HOST")
            .unwrap_or_else(|_| self.server.host.clone())
    }
    
    pub fn get_workers(&self) -> usize {
        if let Ok(workers) = std::env::var("RENZORA_WORKERS") {
            return workers.parse().unwrap_or_else(|_| num_cpus::get());
        }
        
        self.server.workers.unwrap_or_else(num_cpus::get)
    }
}