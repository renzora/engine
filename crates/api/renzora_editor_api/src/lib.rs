//! The editor contract: the thin editor types every editor crate and editor
//! plugin agrees on. Selection (`EditorSelection`), the inspector registry and
//! field definitions (`FieldDef`, the `*_field!` macros, the `Inspectable`
//! derive), the toolbar, spawn, shortcut and GPU-pass registries, gizmo
//! targets, the timeline bridge, and `AppEditorExt`, which registers into all
//! of them.
//!
//! A layer of the contract hierarchy, moved out of `renzora`'s `editor_contract`.
//! Editor-only by linking rather than by a feature: a crate that ships in a game
//! does not depend on this, and one with an optional `editor` feature makes it
//! an optional dependency of that feature. The ungated editor vocabulary that
//! games also use (shell panels, keybindings, undo, ...) is `renzora_shell_api`.
//!
//! Plugins reach every item at the path it always had (`renzora::EditorSelection`,
//! `renzora::FieldDef`, `renzora::float_field!`, `renzora::Inspectable`) through
//! `renzora`'s `editor` feature.

mod ext;
mod gizmo_target;
mod gpu_pass_registry;
mod inspector_registry;
mod selection;
mod shortcut_registry;
mod spawn_registry;
mod timeline_bridge;
mod toolbar_registry;
mod tools;
mod types;

pub use ext::*;
pub use gizmo_target::*;
pub use gpu_pass_registry::*;
pub use inspector_registry::*;
pub use selection::*;
pub use shortcut_registry::*;
pub use spawn_registry::*;
pub use timeline_bridge::*;
pub use toolbar_registry::*;
pub use tools::*;
pub use types::*;

/// The `Inspectable` derive. What it generates names this crate in an engine
/// crate and `renzora` in a plugin; see the macro crate.
pub use renzora_editor_api_macros::Inspectable;
