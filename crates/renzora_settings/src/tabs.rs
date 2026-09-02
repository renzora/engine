//! Tab content dispatch — one module per sidebar page.
//!
//! Every builder here takes the same shape: it is handed the content column and
//! stacks ember `section`s under it. A tab that is split into several sidebar
//! categories also takes a `focus` key and calls [`crate::rows::focus_hide`] on
//! each section, so all of its sections are built and all but one hidden — that
//! is what lets the categories share one page without rebuilding per category.

use bevy::prelude::*;

use renzora_editor_framework::{EditorSettings, SettingsTab};
use renzora_ember::font::EmberFonts;
use renzora_ember::settings_sections::SettingsSectionRegistry;
use renzora_viewport::settings::ViewportSettings;

use crate::state::InputTabData;

pub(crate) mod editor;
pub(crate) mod input;
pub(crate) mod interface;
pub(crate) mod plugins;
pub(crate) mod project;
pub(crate) mod scripting;
pub(crate) mod shortcuts;
pub(crate) mod theme;
pub(crate) mod viewport;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_tab_content(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tab: SettingsTab,
    settings: &EditorSettings,
    viewport_settings: &ViewportSettings,
    custom: &[String],
    themes: &[String],
    scenes: &[String],
    has_project: bool,
    input_data: &InputTabData,
    sections: Option<&SettingsSectionRegistry>,
    active_sub: Option<&str>,
) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("tab-content"),
        ))
        .id();

    match tab {
        SettingsTab::Project => {
            project::tab_project(commands, fonts, col, scenes, custom, has_project, active_sub)
        }
        SettingsTab::Interface => {
            interface::tab_interface(commands, fonts, col, settings, custom)
        }
        SettingsTab::Editor => editor::tab_editor(commands, fonts, col, active_sub),
        SettingsTab::Viewport => {
            viewport::tab_viewport(commands, fonts, col, viewport_settings, active_sub)
        }
        SettingsTab::Scripting => scripting::tab_scripting(commands, fonts, col),
        SettingsTab::Theme => theme::tab_theme(commands, fonts, col, themes),
        SettingsTab::Shortcuts => shortcuts::tab_shortcuts(commands, fonts, col),
        SettingsTab::Input => input::tab_input(commands, fonts, col, input_data),
        SettingsTab::Plugins => plugins::tab_plugins(commands, fonts, col, sections, active_sub),
    }
    col
}
