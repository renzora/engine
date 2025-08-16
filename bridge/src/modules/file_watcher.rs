use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::broadcast;
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event, EventKind};
use crate::types::FileChangeEvent;

// Global file change broadcaster
static FILE_CHANGE_SENDER: OnceLock<broadcast::Sender<String>> = OnceLock::new();

pub fn get_file_change_sender() -> Option<&'static broadcast::Sender<String>> {
    FILE_CHANGE_SENDER.get()
}

pub fn initialize_file_watcher(projects_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the broadcaster
    let (tx, _) = broadcast::channel(100);
    FILE_CHANGE_SENDER.set(tx.clone()).ok();
    
    // Start file watching in background
    tokio::spawn(async move {
        if let Err(e) = watch_files(projects_path, tx).await {
            eprintln!("❌ File watcher error: {}", e);
        }
    });
    
    Ok(())
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
        
        if !paths.is_empty() {
            let change_event = FileChangeEvent {
                event_type: event_type.to_string(),
                paths: paths.clone(),
            };
            
            let frontend_message = serde_json::json!({
                "type": "file-changes",
                "changes": [change_event]
            });
            
            if let Ok(json) = serde_json::to_string(&frontend_message) {
                let _ = tx.send(json);
                println!("📁 File {}: {:?}", event_type, paths);
            }
        }
    }
    
    Ok(())
}

pub fn create_file_change_receiver() -> Option<broadcast::Receiver<String>> {
    FILE_CHANGE_SENDER.get().map(|sender| sender.subscribe())
}