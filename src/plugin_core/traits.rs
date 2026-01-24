//! Plugin traits that plugins must implement.
//!
//! Re-exports types from editor_plugin_api.

// Re-export types from the shared crate
pub use editor_plugin_api::traits::EditorPlugin;
pub use editor_plugin_api::events::{EditorEvent, EditorEventType};
