pub mod types;
pub mod errors;
pub mod handlers;
pub mod file_watcher;
pub mod file_sync;
pub mod project_manager;
pub mod update_manager;
pub mod system_monitor;
pub mod model_processor;
pub mod model_converter;
pub mod renscript_compiler;
// Database module removed - using Redis-only caching
// Redis cache module removed - using lightweight memory cache
pub mod memory_cache;
// Embedded Redis removed - using lightweight memory cache
pub mod renscript_mappings;
pub mod renscript_cache;
pub mod project_cache_validator;

// Export only what's needed by main.rs
pub use handlers::{handle_http_request, set_startup_time, set_memory_cache, set_renscript_cache};
pub use file_watcher::{initialize_file_watcher};
pub use project_manager::{get_base_path, get_projects_path};
pub use system_monitor::{initialize_system_monitor};
// DatabaseManager removed - using Redis-only caching
// RedisCache removed - using lightweight MemoryCache
pub use memory_cache::MemoryCache;
// EmbeddedRedisServer removed - using lightweight memory cache
pub use renscript_cache::RenScriptCache;