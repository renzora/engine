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

// API types
pub use crate::api::{
    Command, ContextMenuLocation, CustomEvent, EditorApi, EntityDefinition, EntityQuery,
    InspectorDefinition, MenuItem, MenuLocation, PanelDefinition, PanelLocation, SettingValue,
    ToolbarItem,
};

// Event types
pub use crate::events::{EditorEvent, EditorEventType, UiEvent};

// Trait
pub use crate::traits::{CreatePluginFn, EditorPlugin};

// UI types
pub use crate::ui::{Align, Size, Tab, TableColumn, TableRow, TextStyle, UiId, Widget};
