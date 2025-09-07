use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, error, warn};
use chrono::Utc;

use crate::types::FileChange;
use crate::state::AppState;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    state: AppState,
}

impl FileWatcher {
    pub fn new(state: AppState) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let state_clone = state.clone();
        
        // Spawn task to handle file events
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let Err(e) = handle_file_event(event, &state_clone).await {
                    error!("Error handling file event: {}", e);
                }
            }
        });
        
        let watcher = RecommendedWatcher::new(
            move |res| {
                match res {
                    Ok(event) => {
                        if let Err(e) = tx.send(event) {
                            error!("Failed to send file event: {}", e);
                        }
                    }
                    Err(e) => error!("File watcher error: {:?}", e),
                }
            },
            Config::default(),
        )?;
        
        Ok(Self {
            _watcher: watcher,
            state,
        })
    }
    
    pub fn watch_path<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        self._watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;
        info!("📁 Watching path: {}", path.as_ref().display());
        Ok(())
    }
}

async fn handle_file_event(event: Event, state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    // Filter out temporary files and system files
    let paths: Vec<String> = event.paths
        .iter()
        .filter_map(|p| {
            let path_str = p.to_string_lossy();
            
            // Skip temporary files, hidden files, and build artifacts
            if path_str.contains(".tmp") || 
               path_str.contains("~") || 
               path_str.starts_with('.') ||
               path_str.contains("/target/") ||
               path_str.contains("\\target\\") ||
               path_str.contains("/node_modules/") ||
               path_str.contains("\\node_modules\\") ||
               path_str.contains(".git") {
                return None;
            }
            
            Some(path_str.to_string())
        })
        .collect();
    
    if paths.is_empty() {
        return Ok(());
    }
    
    let event_type = match event.kind {
        EventKind::Create(_) => "create",
        EventKind::Modify(_) => "modify", 
        EventKind::Remove(_) => "delete",
        _ => "other",
    };
    
    let file_change = FileChange {
        event_type: event_type.to_string(),
        paths,
        timestamp: Utc::now(),
    };
    
    // Broadcast to all connected clients
    state.broadcast_file_change(file_change).await;
    
    Ok(())
}

pub async fn initialize_file_watcher(
    projects_path: std::path::PathBuf,
    state: AppState,
) -> Result<FileWatcher, Box<dyn std::error::Error>> {
    info!("🔍 Initializing file watcher");
    
    let mut watcher = FileWatcher::new(state)?;
    
    // Watch the projects directory
    if projects_path.exists() {
        watcher.watch_path(&projects_path)?;
        info!("👀 File watcher initialized for: {}", projects_path.display());
    } else {
        warn!("Projects directory does not exist: {}", projects_path.display());
    }
    
    Ok(watcher)
}