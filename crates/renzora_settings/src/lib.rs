//! Renzora Settings — the editor's settings overlay.
//!
//! A centered modal with a vertical category sidebar and a scrollable content
//! pane, driven by `EditorSettings::show_settings`. It reads from the live
//! resources (`EditorSettings`, `KeyBindings`, `ViewportSettings`,
//! `ThemeManager`, `CurrentProject`) and writes straight back to them via
//! `bind_2way`, so an edit lands the same frame — there is no apply step and no
//! model of its own.
//!
//! - [`overlay`] owns the shell and the spawn/despawn/rebuild loop
//! - [`sidebar`] owns the category list, its search filter and its click systems
//! - [`rows`] is the form-row vocabulary the tab builders are written in
//! - [`tabs`] holds one module per sidebar page

use bevy::prelude::*;

mod fonts;
mod lang;
mod overlay;
mod prefs;
mod rows;
mod sidebar;
mod state;
mod tabs;

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SettingsPlugin");
        app.init_resource::<state::OverlayState>();
        app.init_resource::<state::InputUi>();
        // Seed the auto-save setting from disk so the Editor tab shows the
        // persisted value even if the `renzora_autosave` plugin (its real owner)
        // isn't present. `insert_resource` from that plugin wins over this when
        // it is.
        app.insert_resource(renzora::load_autosave());
        // Seed the shared log-buffer cap from the persisted pref up front, so
        // logs emitted during startup (before `sync_console_log_limit` first
        // runs) are already bounded by the user's chosen limit.
        renzora::core::console_log::set_max_log_entries(renzora::load_console_log_limit());
        app.add_systems(
            Update,
            (
                overlay::manage_overlay,
                overlay::settings_close_click,
                sidebar::settings_tab_click,
                sidebar::settings_plugin_click,
                sidebar::filter_sidebar,
                fonts::refresh_settings_on_font_change,
                fonts::apply_font_settings,
                tabs::plugins::plugin_toggle_click,
                tabs::theme::theme_save_click,
                tabs::theme::ember_theme_save_click,
                prefs::sync_drag_value_rail_sweep,
                prefs::sync_scroll_speed,
                prefs::sync_console_log_limit,
            )
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
        // The Input tab's structural edits — each marks the overlay dirty so it
        // rebuilds with the new set of rows.
        app.add_systems(
            Update,
            (
                tabs::input::add_action_click,
                tabs::input::delete_action_click,
                tabs::input::expand_action_click,
                tabs::input::add_binding_click,
                tabs::input::cancel_listen_click,
                tabs::input::remove_binding_click,
                tabs::input::composite_click,
                tabs::input::input_listen_capture,
            )
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
        // Key/mouse-rebind capture.
        app.add_systems(
            Update,
            (
                tabs::shortcuts::rebind_btn_click,
                tabs::shortcuts::rebind_capture,
                tabs::shortcuts::reset_bindings_click,
            )
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
    }
}

renzora::add!(SettingsPlugin, Editor);
