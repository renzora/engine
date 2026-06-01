//! `renzora_ember` — Renzora's unified **bevy_ui** UI framework.
//!
//! One crate, used by both the editor and exported games, so UI is authored the
//! same way everywhere (Godot-style). Deliberately **no feature flags**: feature
//! deltas would shift `bevy_dylib`'s SVH and re-split the editor/runtime builds
//! that the plugin ABI relies on (see `docs/editor-runtime-plugin-architecture.md`),
//! so everything ships together.
//!
//! Modules:
//! - [`theme`] — the bevy-native palette (raw shared colors).
//! - [`style`] — the runtime theming system (`Theme` tokens + `Styled` + repaint).
//! - [`font`] — fonts + text/icon helpers.
//! - [`dock`] — the dockable panel layout component.
//! - [`widgets`] — reusable UI components (buttons, toggles, …).
//!
//! Add [`EmberPlugin`] to register the theme + dock + widget systems. Migrating
//! in next: `markup` (folds in `renzora_hui`) and `cinder` (particle UI).

use bevy::prelude::*;

pub mod dock;
pub mod font;
pub mod style;
pub mod theme;
pub mod widgets;

/// Registers all of ember's runtime systems (theme + dock + widgets + fonts).
pub struct EmberPlugin;

impl Plugin for EmberPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((style::ThemePlugin, dock::DockPlugin, widgets::WidgetsPlugin));
    }
}
