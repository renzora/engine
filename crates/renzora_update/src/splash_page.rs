//! The splash dashboard's **Updates** page.
//!
//! The same updater the editor opens from Help, as a page rather than a modal —
//! literally the same tree, built by [`crate::dialog::build_body`]. Nothing here
//! re-implements the check, the download, the install path, the overwrite
//! confirmation or the sidecar handoff; this module is a frame around them.
//!
//! # Why the splash is the right place for it
//!
//! Updating swaps the files the running editor is executing out from under it,
//! which is why the swap is handed to a sidecar and why it ends in a relaunch.
//! The splash is the one screen where that costs nothing: no project is open, no
//! scene is loaded, and there is nothing to lose to the restart. In the editor
//! the same dialog is an interruption you have to decide about; here it is just
//! the thing you came to do.
//!
//! # Why this lives in `renzora_update` and not in `renzora_splash`
//!
//! The same reason the marketplace's Plugins page does: `renzora_splash` is
//! linked into the *runtime*, so a splash that reached for the updater would put
//! `rfd`, `zip` and the release client in the shipped game binary. The page
//! registers from this side through the registry the splash exposes. See
//! `renzora_splash::launcher::sections`.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_splash::{register_splash_section, SplashSection};

pub(crate) fn register(app: &mut App) {
    register_splash_section(
        app,
        // Between Plugins (40) and Changelog (80): "what version am I on" sits
        // naturally next to "what changed in it".
        SplashSection::new("updates", "arrow-circle-up", "Updates", 60, build),
    );
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let page = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                // Grows into the host rather than asking for 100% of it — see
                // `renzora_splash::launcher::sections::build_page_host`.
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(
                    Val::Px(22.0),
                    Val::Px(22.0),
                    Val::Px(22.0),
                    Val::Px(4.0),
                ),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-page-updates"),
        ))
        .id();

    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let title = commands
        .spawn((
            Text::new("Updates".to_string()),
            ui_font(&fonts.ui, 17.0),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    let sub = commands
        .spawn((
            Text::new(
                "Install a newer engine now, while nothing is open to lose to the restart."
                    .to_string(),
            ),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(header).add_children(&[title, sub]);

    // The updater's own body carries 18px of padding, which is why the page's
    // bottom padding is trimmed to 4 — otherwise the two stack into a gap wide
    // enough to read as a mistake.
    let body = crate::dialog::build_body(commands, fonts, false);
    // Blocks: the splash's rule is that anything holding a press says so, and
    // this subtree is full of buttons and a text field.
    commands.entity(body).insert(FocusPolicy::Block);

    commands.entity(page).add_children(&[header, body]);
    page
}
