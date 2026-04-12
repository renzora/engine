//! Renzora Plugin SDK — the single dependency for all plugin development.
//!
//! ```toml
//! [dependencies]
//! bevy = { workspace = true }
//! renzora = { path = "..." }
//! ```
//!
//! ```rust
//! use bevy::prelude::*;
//! use renzora::prelude::*;
//!
//! renzora::add!(MyPlugin);
//!
//! impl Plugin for MyPlugin {
//!     fn build(&self, app: &mut App) {
//!         app.register_panel(MyPanel);
//!         app.add_systems(Update, my_system);
//!     }
//! }
//! ```

// ── Plugin macro ────────────────────────────────────────────────────────
pub use dynamic_plugin_meta::add;

// ── Proc macros ─────────────────────────────────────────────────────────
pub use renzora_macros::{Inspectable, post_process};

// ── Core types (always available) ───────────────────────────────────────
pub use renzora_core as core;
pub use renzora_postprocess as postprocess;
pub use renzora_theme as theme;

// ── Editor framework (only with editor feature) ────────────────────────
#[cfg(feature = "editor")]
pub use renzora_editor_framework as editor;
#[cfg(feature = "editor")]
pub use renzora_undo as undo;
#[cfg(feature = "editor")]
pub use bevy_egui;
#[cfg(feature = "editor")]
pub use egui_phosphor;

// ── Prelude ─────────────────────────────────────────────────────────────

pub mod prelude {
    // Core types
    pub use renzora_core::{
        CurrentProject, DefaultCamera, EditorCamera, EditorLocked, EntityTag,
        HideInHierarchy, MeshColor, MeshPrimitive, PlayModeState, PlayState,
        SceneCamera, ViewportRenderTarget, ShapeRegistry,
        // Decoupling events
        PausePhysics, UnpausePhysics, ResetScriptStates, SaveCurrentScene,
        ScriptAction, ScriptActionValue, ScriptsReloaded,
        CharacterCommand, CharacterCommandQueue,
    };

    // Plugin macro
    pub use dynamic_plugin_meta::add;

    // Proc macros
    pub use renzora_macros::{Inspectable, post_process};

    // Postprocess
    pub use renzora_postprocess::PostProcessEffect;

    // Editor framework (panel traits, registries, commands)
    #[cfg(feature = "editor")]
    pub use renzora_editor_framework::{
        // Extension trait
        AppEditorExt, InspectableComponent,
        // Panel system
        EditorPanel, PanelLocation,
        // Status bar
        StatusBarItem, StatusBarAlignment, StatusBarRegistry,
        // Inspector
        InspectorEntry, InspectorRegistry, FieldDef, FieldType, FieldValue,
        // Spawn & icons
        EntityPreset, SpawnRegistry, ComponentIconEntry, ComponentIconRegistry,
        // Selection & commands
        EditorCommands, EditorSelection,
        // Toolbar
        ToolEntry, ToolSection, ToolbarRegistry,
        // Shortcuts
        ShortcutEntry, ShortcutRegistry,
        // Settings
        EditorSettings,
        // State
        SplashState,
        // UI re-exports (from renzora_ui)
        DockingState,
    };
}
