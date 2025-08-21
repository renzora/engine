use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::broadcast;
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event, EventKind};
use crate::types::FileChangeEvent;
use log::info;

// Global file change broadcaster
static FILE_CHANGE_SENDER: OnceLock<broadcast::Sender<String>> = OnceLock::new();
// Global current project being watched
static CURRENT_PROJECT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

pub fn get_file_change_receiver() -> Option<broadcast::Receiver<String>> {
    FILE_CHANGE_SENDER.get().map(|sender| sender.subscribe())
}

pub fn initialize_file_watcher(projects_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the broadcaster
    let (tx, _) = broadcast::channel(100);
    FILE_CHANGE_SENDER.set(tx.clone()).ok();
    
    // Initialize current project tracker
    CURRENT_PROJECT.set(Arc::new(Mutex::new(None))).ok();
    
    // For now, watch all projects but filter by current project
    // In the future, we can dynamically update the watcher
    tokio::spawn(async move {
        if let Err(e) = watch_files(projects_path, tx).await {
            eprintln!("❌ File watcher error: {}", e);
        }
    });
    
    Ok(())
}

pub fn set_current_project(project_name: Option<String>) {
    if let Some(current) = CURRENT_PROJECT.get() {
        let mut current_project = current.lock().unwrap();
        *current_project = project_name.clone();
        if let Some(name) = project_name {
            info!("🔍 File watcher now focused on project: {}", name);
        } else {
            info!("🔍 File watcher now watching all projects");
        }
    }
}

pub async fn watch_files(projects_path: PathBuf, tx: broadcast::Sender<String>) -> Result<(), Box<dyn std::error::Error>> {
    use notify::Result as NotifyResult;
    
    let (watch_tx, mut watch_rx) = tokio::sync::mpsc::channel(100);
    
    let mut watcher = RecommendedWatcher::new(
        move |result: NotifyResult<Event>| {
            if let Ok(event) = result {
                let _ = watch_tx.blocking_send(event);
            }
        },
        notify::Config::default()
    )?;
    
    watcher.watch(&projects_path, RecursiveMode::Recursive)?;
    println!("🔍 File watcher started for: {}", projects_path.display());
    
    while let Some(event) = watch_rx.recv().await {
        let event_type = match event.kind {
            EventKind::Create(_) => "create",
            EventKind::Modify(_) => "modify", 
            EventKind::Remove(_) => "delete",
            _ => continue,
        };
        
        let paths: Vec<String> = event.paths.iter()
            .filter_map(|p| p.strip_prefix(&projects_path).ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        
        // Filter by current project if one is set
        let filtered_paths = if let Some(current) = CURRENT_PROJECT.get() {
            if let Some(ref project_name) = *current.lock().unwrap() {
                // Only include paths that start with the current project name
                paths.into_iter()
                    .filter(|p| p.starts_with(project_name))
                    .collect()
            } else {
                paths
            }
        } else {
            paths
        };
        
        if !filtered_paths.is_empty() {
            let change_event = FileChangeEvent {
                event_type: event_type.to_string(),
                paths: filtered_paths.clone(),
            };
            
            let frontend_message = serde_json::json!({
                "type": "file-changes",
                "changes": [change_event]
            });
            
            if let Ok(json) = serde_json::to_string(&frontend_message) {
                let _ = tx.send(json);
                println!("📁 File {}: {:?}", event_type, filtered_paths);
            }
        }
    }
    
    Ok(())
}