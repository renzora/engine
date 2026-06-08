//! A throwaway "Demo Panel" the tutorial registers but deliberately does NOT add
//! to the dock — the user adds it themselves via a tab bar's **+** picker (the
//! "Add a panel" step) and then re-docks it (the "Rearrange it" step).
//!
//! Registration mirrors any native panel: [`RenzoraShellExt::register_shell_panel`]
//! puts it in the Add-Panel picker, and ember's [`RegisterPanelContent`] supplies
//! the content built when it's first shown.

use bevy::prelude::*;

use renzora::core::RenzoraShellExt;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};

/// The panel id used everywhere (picker entry, content builder, detection).
pub const DEMO_PANEL_ID: &str = "tutorial_demo_panel";

/// Register the panel's picker entry + content builder. Not added to any layout,
/// so it only appears once the user picks it from a **+** menu.
pub fn register(app: &mut App) {
    app.register_shell_panel(DEMO_PANEL_ID, "Demo Panel", "sparkle", "Tutorial");
    app.register_panel_content(DEMO_PANEL_ID, true, build_content);
}

fn build_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(16.0)),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "sparkle", accent(), 30.0);
    let title = commands
        .spawn((
            Text::new("Demo Panel"),
            ui_font(&fonts.ui, 16.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let body = commands
        .spawn((
            Text::new(
                "Nice — you docked a panel! Panels can live anywhere. Next, drag this \
                 panel's tab and drop it over another panel to re-dock it.",
            ),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
            Node { max_width: Val::Px(280.0), ..default() },
        ))
        .id();
    commands.entity(root).add_children(&[icon, title, body]);
    root
}
