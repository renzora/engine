//! Overlay geometry, the accent palette, and the two resources that hold the
//! settings overlay's own transient state.

use bevy::prelude::*;

use renzora_editor_framework::SettingsTab;
use renzora_input::InputAction;

pub(crate) const PANEL_W: f32 = 880.0;
pub(crate) const PANEL_H: f32 = 620.0;
// Wide enough that no category label wraps to a second line. The usable text
// width is this minus the icon (14), the two gaps (10 + 10) and the horizontal
// padding (8 + 8) — at 160px that left ~110px, which "2D Rendering" and
// "UI Workspace" overflowed.
pub(crate) const SIDEBAR_W: f32 = 200.0;

// Accent colors per category — matches the egui `CategoryStyle` palette.
pub(crate) const A_BLUE: (u8, u8, u8) = (80, 140, 255);
pub(crate) const A_PURPLE: (u8, u8, u8) = (170, 130, 240);
pub(crate) const A_ORANGE: (u8, u8, u8) = (235, 150, 70);
pub(crate) const A_GREEN: (u8, u8, u8) = (110, 200, 120);
pub(crate) const A_TEAL: (u8, u8, u8) = (80, 200, 200);
pub(crate) const A_VIOLET: (u8, u8, u8) = (180, 130, 230);
pub(crate) const A_YELLOW: (u8, u8, u8) = (225, 200, 70);

#[derive(Resource, Default)]
pub(crate) struct OverlayState {
    pub(crate) root: Option<Entity>,
    pub(crate) built_tab: Option<SettingsTab>,
    /// Set by dynamic tabs (Input) to force a rebuild after a structural change
    /// (add/remove action, expand a row, enter listen mode).
    pub(crate) dirty: bool,
    /// Active theme name at last build — the overlay rebuilds on a theme switch
    /// so it re-spawns with the new palette (it's a separate root from the chrome
    /// and wouldn't otherwise pick up the change while open).
    pub(crate) built_theme: Option<String>,
    /// `renzora::lang::revision()` at last build — same idea as `built_theme`, so
    /// switching language from the overlay's own picker re-localizes it live.
    pub(crate) built_lang_rev: u64,
    /// Sub-selection within the active tab — a section focus key for a split tab
    /// (e.g. `"grid"` under Viewport) or a plugin section id under `Plugins`. The
    /// tab disambiguates which, so one field serves both. `None` = whole tab.
    pub(crate) active_sub: Option<String>,
    /// The `active_sub` at last build, for the rebuild comparison.
    pub(crate) built_sub: Option<String>,
}

/// Transient UI state for the Input tab (which action is expanded, whether a
/// binding capture is in progress, and the new-action name field).
#[derive(Resource, Default)]
pub(crate) struct InputUi {
    pub(crate) selected: Option<usize>,
    pub(crate) listening: bool,
    pub(crate) new_name: String,
}

/// Snapshot of the Input tab's data, read once per (re)build.
pub(crate) struct InputTabData {
    pub(crate) actions: Vec<InputAction>,
    pub(crate) selected: Option<usize>,
    pub(crate) listening: bool,
}
