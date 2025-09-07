pub mod websocket;
pub mod state;
pub mod file_watcher;
pub mod project_manager;
pub mod file_sync;
pub mod types;
pub mod config;
pub mod config_manager;

// Re-export commonly used items
pub use state::{AppState, get_base_path, get_projects_path};
pub use config::Config;
pub use config_manager::*;
pub use types::*;