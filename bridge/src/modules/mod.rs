pub mod types;
pub mod handlers;
pub mod file_watcher;
pub mod file_sync;
pub mod project_manager;
pub mod thumbnail_generator;
pub mod thumbnail_generators;
pub mod update_manager;
pub mod system_monitor;
pub mod model_processor;
pub mod model_converter;
pub mod renscript_compiler;
pub mod database;
pub mod redis_cache;
pub mod embedded_redis;
pub mod renscript_mappings;
pub mod renscript_cache;
pub mod project_cache_validator;

// Export only what's needed by main.rs
pub use handlers::{handle_http_request, set_startup_time, set_database, set_redis_cache, set_renscript_cache};
pub use file_watcher::{initialize_file_watcher};
pub use project_manager::{get_base_path, get_projects_path};
pub use system_monitor::{initialize_system_monitor};
pub use database::DatabaseManager;
pub use redis_cache::RedisCache;
pub use embedded_redis::EmbeddedRedisServer;
pub use renscript_cache::RenScriptCache;