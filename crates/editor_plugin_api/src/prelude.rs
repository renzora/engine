//! Prelude module - re-exports commonly used types for plugin authors.
//!
//! ```rust,ignore
//! use editor_plugin_api::prelude::*;
//! ```

// This prelude will re-export types from the main editor crate
// For now, it serves as a placeholder showing what would be exported

pub use crate::declare_plugin;

// When the shared types are properly factored out, this would include:
// pub use crate::abi::{PluginManifest, PluginCapability, PluginDependency, PluginError};
// pub use crate::api::{EditorApi, MenuLocation, MenuItem, PanelDefinition, ...};
// pub use crate::traits::{EditorPlugin, EditorEvent, EditorEventType};
// pub use crate::ui::{Widget, UiEvent, UiId, ...};
