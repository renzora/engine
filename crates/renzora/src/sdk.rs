//! Stable helpers for plugin authors.
//!
//! Plugins can reach directly into Bevy resources (`Res<ViewportSettings>`,
//! `Res<ActiveTool>`, etc.) — nothing here prevents that. These helpers
//! exist so the most common gating patterns read as one-liners and so the
//! SDK can absorb internal refactors without breaking every plugin.
//!
//! The typical use is as Bevy `run_if` conditions:
//!
//! ```ignore
//! use renzora::sdk::conditions::*;
//! use renzora::core::viewport_types::ViewportMode;
//!
//! app.add_systems(Update, my_edit_system.run_if(in_mode(ViewportMode::Edit)));
//! app.add_systems(Update, my_overlay.run_if(viewport_hovered));
//! ```

#[cfg(feature = "editor")]
pub mod conditions {
    use bevy::prelude::*;
    use renzora_core::viewport_types::{ViewportMode, ViewportSettings, ViewportState};
    use renzora_editor_framework::{ActiveTool, EditorSelection};

    /// Run only when the viewport is in the given [`ViewportMode`]
    /// (Scene / Edit / Sculpt / Paint / Animate).
    pub fn in_mode(mode: ViewportMode) -> impl FnMut(Option<Res<ViewportSettings>>) -> bool + Clone {
        move |s: Option<Res<ViewportSettings>>| s.map(|s| s.viewport_mode == mode).unwrap_or(false)
    }

    /// Run only when the viewport is NOT in the given mode. Useful for
    /// systems that should pause during Edit / Sculpt / etc.
    pub fn not_in_mode(mode: ViewportMode) -> impl FnMut(Option<Res<ViewportSettings>>) -> bool + Clone {
        move |s: Option<Res<ViewportSettings>>| s.map(|s| s.viewport_mode != mode).unwrap_or(true)
    }

    /// Run only when the given [`ActiveTool`] is the currently selected tool.
    pub fn tool_active(tool: ActiveTool) -> impl FnMut(Option<Res<ActiveTool>>) -> bool + Clone {
        move |t: Option<Res<ActiveTool>>| t.map(|t| *t == tool).unwrap_or(false)
    }

    /// Run only when any of the given tools is active.
    pub fn any_tool_active(
        tools: &'static [ActiveTool],
    ) -> impl FnMut(Option<Res<ActiveTool>>) -> bool + Clone {
        move |t: Option<Res<ActiveTool>>| t.map(|t| tools.contains(&*t)).unwrap_or(false)
    }

    /// Run only while the mouse cursor is over the 3D viewport (not over
    /// any editor panel, overlay, or popup).
    pub fn viewport_hovered(vp: Option<Res<ViewportState>>) -> bool {
        vp.map(|v| v.hovered).unwrap_or(false)
    }

    /// Run only when at least one entity is selected in the editor.
    pub fn has_selection(sel: Option<Res<EditorSelection>>) -> bool {
        sel.map(|s| s.get().is_some()).unwrap_or(false)
    }
}
