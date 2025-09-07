use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{info, error};

use crate::types::{ClientConnection, FileChange};

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<Uuid, ClientConnection>>>,
    pub file_change_tx: broadcast::Sender<FileChange>,
    pub startup_time: DateTime<Utc>,
    pub server_version: String,
}

impl AppState {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (file_change_tx, _) = broadcast::channel(1000); // Buffer up to 1000 file changes
        
        let state = Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            file_change_tx,
            startup_time: Utc::now(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        
        info!("🏗️  Server state initialized");
        Ok(state)
    }
    
    pub async fn add_client(&self, client: ClientConnection) {
        let client_id = client.id;
        let mut clients = self.clients.write().await;
        clients.insert(client.id, client);
        info!("👤 Client connected: {} (total: {})", client_id, clients.len());
    }
    
    pub async fn remove_client(&self, client_id: &Uuid) {
        let mut clients = self.clients.write().await;
        if clients.remove(client_id).is_some() {
            info!("👋 Client disconnected: {} (total: {})", client_id, clients.len());
        }
    }
    
    pub async fn get_client_count(&self) -> usize {
        self.clients.read().await.len()
    }
    
    pub async fn broadcast_file_change(&self, change: FileChange) {
        if let Err(e) = self.file_change_tx.send(change.clone()) {
            error!("Failed to broadcast file change: {}", e);
        } else {
            info!("📡 Broadcasting file change: {:?}", change.event_type);
        }
    }
    
    pub fn subscribe_to_file_changes(&self) -> broadcast::Receiver<FileChange> {
        self.file_change_tx.subscribe()
    }
}

pub fn get_base_path() -> PathBuf {
    // Check environment variable first
    if let Ok(base_path) = std::env::var("RENZORA_BASE_PATH") {
        return PathBuf::from(base_path);
    }
    
    // Default to current directory - frontend will tell us the real path
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn get_projects_path() -> PathBuf {
    // Check environment variable first  
    if let Ok(projects_path) = std::env::var("RENZORA_PROJECTS_PATH") {
        return PathBuf::from(projects_path);
    }
    
    // Default to projects subdirectory - frontend will update this
    get_base_path().join("projects")
}