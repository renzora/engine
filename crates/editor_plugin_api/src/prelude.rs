//! Prelude module - re-exports commonly used types for plugin authors.
//!
//! ```rust,ignore
//! use editor_plugin_api::prelude::*;
//! ```

// Macro
pub use crate::declare_plugin;

// ABI types
pub use crate::abi::{
    AssetHandle, AssetStatus, EntityId, PluginCapability, PluginDependency, PluginError,
    PluginManifest, PluginTransform, EDITOR_API_VERSION,
};

// FFI types
pub use crate::ffi::{FFI_API_VERSION, FfiResult, FfiManifest, FfiStatusBarItem, PluginExport, PluginVTable, FfiEditorApi};

// API types (for reference, but plugins should use FfiEditorApi)
pub use crate::api::{StatusBarAlign, StatusBarItem};

// Event types
pub use crate::events::{EditorEvent, EditorEventType, UiEvent};

// UI types
pub use crate::ui::{Align, Size, Tab, TableColumn, TableRow, TextStyle, UiId, Widget};

// Icons - re-export egui_phosphor for easy access
pub use crate::egui_phosphor;
