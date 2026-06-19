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
//! Add [`EmberPlugin`] to register the theme + dock + widget systems. The
//! [`markup`] runtime (folded in from the former `renzora_hui`) installs itself
//! via `renzora::add!` so it runs in games and the editor alike. Migrating in
//! next: `cinder` (particle UI).

use bevy::prelude::*;

pub mod cursor_icon;
pub mod dock;
pub mod font;
pub mod icons;
pub mod inspector;
pub mod markup;
pub mod panel;
pub mod phosphor_map;
pub mod reactive;
pub mod settings_sections;
pub mod style;
pub mod theme;
pub mod virtual_scroll;
pub mod widgets;

/// Registers all of ember's runtime systems (theme + dock + widgets + fonts +
/// the reactive bindings/keyed-list drivers).
pub struct EmberPlugin;

impl Plugin for EmberPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            style::ThemePlugin,
            dock::DockPlugin,
            widgets::WidgetsPlugin,
            reactive::ReactivePlugin,
            virtual_scroll::VirtualScrollPlugin,
        ));
    }
}
