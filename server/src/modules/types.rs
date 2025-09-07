use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    // Connection management
    Connected { server_version: String, timestamp: DateTime<Utc> },
    Ping,
    Pong,
    
    // File operations (paths can be absolute or relative to provided base)
    FileRead { path: String, base_path: Option<String> },
    FileReadResponse { path: String, content: Option<String>, error: Option<String> },
    FileBinaryRead { path: String, base_path: Option<String> },
    FileBinaryReadResponse { path: String, content: Option<String>, error: Option<String> },
    FileWrite { path: String, content: String, base_path: Option<String> },
    FileWriteResponse { path: String, success: bool, error: Option<String> },
    FileBinaryWrite { path: String, data: String, create_dirs: Option<bool>, base_path: Option<String> },
    FileBinaryWriteResponse { path: String, success: bool, error: Option<String> },
    FileDelete { path: String, base_path: Option<String> },
    FileDeleteResponse { path: String, success: bool, error: Option<String> },
    
    // Directory operations
    ListDirectory { path: String, base_path: Option<String> },
    ListDirectoryResponse { path: String, items: Vec<FileInfo>, error: Option<String> },
    
    // File watching
    FileChanges { changes: Vec<FileChange> },
    StartWatching { project_name: Option<String> },
    StopWatching,
    
    // Project management
    ListProjects,
    ListProjectsResponse { projects: Vec<ProjectInfo>, error: Option<String> },
    CreateProject { name: String, template: String },
    CreateProjectResponse { project: Option<ProjectInfo>, error: Option<String> },
    
    // Health and status
    HealthCheck,
    HealthCheckResponse { status: ServerHealth },
    SystemStats,
    SystemStatsResponse { stats: SystemStats },
    
    // Configuration management
    GetConfig,
    GetConfigResponse { config: ServerConfig, error: Option<String> },
    SetBasePath { path: String },
    SetBasePathResponse { success: bool, error: Option<String> },
    SetProjectsPath { path: String },
    SetProjectsPathResponse { success: bool, error: Option<String> },
    ScanForEngineRoot,
    ScanForEngineRootResponse { found_paths: Vec<String>, current_path: String, error: Option<String> },
    
    // Error handling
    Error { message: String, code: Option<u32> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: Option<u64>,
    pub modified: Option<DateTime<Utc>>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub event_type: String, // "create", "modify", "delete", "rename"
    pub paths: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub files: Vec<FileInfo>,
    pub settings: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealth {
    pub status: String,
    pub uptime_seconds: u64,
    pub connections: u32,
    pub memory_usage: u64,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub base_path: String,
    pub projects_path: String,
    pub port: u16,
    pub host: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub disk_free: u64,
    pub disk_total: u64,
    pub network_rx: u64,
    pub network_tx: u64,
    pub gpu_usage: Option<f32>,
    pub gpu_memory: Option<u64>,
}

// Internal server types
#[derive(Clone)]
pub struct ClientConnection {
    pub id: uuid::Uuid,
    pub connected_at: DateTime<Utc>,
    pub watching_project: Option<String>,
    pub session: actix_ws::Session,
}

// File operation requests (internal)
#[derive(Debug, Clone)]
pub struct WriteFileRequest {
    pub content: String,
    pub create_dirs: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct WriteBinaryFileRequest {
    pub data: String, // base64 encoded
    pub create_dirs: Option<bool>,
}