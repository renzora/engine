pub mod types;
pub mod handlers;
pub mod file_watcher;
pub mod file_sync;
pub mod project_manager;
pub mod thumbnail_generator;
pub mod update_manager;

// Export only what's needed by main.rs
pub use handlers::{handle_http_request, set_startup_time};
pub use file_watcher::initialize_file_watcher;
pub use project_manager::{get_base_path, get_projects_path};