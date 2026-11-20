//! The strip across the top of the splash that offers to install Rust.
//!
//! Scripts and native plugins are compiled on the machine that uses them, so a
//! downloaded editor on a machine with no Rust could open, browse the
//! marketplace and create a project, and then fail the first script or plugin
//! it met. The splash is where everyone lands first and where an install of a
//! few minutes costs nothing, because no project is open and nothing is waiting
//! on it. So the offer is made here, to everyone, before anything fails.
//!
//! This crate only draws and answers. `renzora_rust_script` checks when the
//! editor starts, publishes [`RustToolchain`], and runs the install on a
//! [`RustToolchainAnswer::Install`]; the strip shows whatever state that is in.
//! Neither links the other: both meet in the contract crate. A build without
//! the script crate (the web) never leaves `Unknown`, so the strip never shows.
//!
//! A strip and not a page or a dialog. A page in the rail is somewhere a new
//! user has to think to look, and a dialog would stand between them and
//! everything else on the splash for a question that can wait. A strip is seen
//! on every page and blocks none of them, and it goes away by itself once Rust
//! is there.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora::{RustToolchain, RustToolchainAnswer};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::UiInteraction;

use super::style::*;

/// The strip's button: Install, or Try again after a failure.
#[derive(Component)]
pub(crate) struct RustInstallButton;

fn state(w: &Rx) -> RustToolchain {
    w.get_resource::<RustToolchain>().cloned().unwrap_or_default()
}

/// Shown for every state that has something to say. `Unknown`, `Checking` and
/// `Ready` say nothing: a machine that already has Rust never sees the strip.
fn visible(s: &RustToolchain) -> bool {
    matches!(
        s,
        RustToolchain::Missing { .. }
            | RustToolchain::Declined
            | RustToolchain::Installing { .. }
            | RustToolchain::Installed { .. }
            | RustToolchain::Failed(_)
    )
}

fn message(s: &RustToolchain) -> String {
    match s {
        RustToolchain::Missing { version, rustup_installed: false, .. } => format!(
            "Scripts and plugins are compiled on your machine, and Rust is not installed. \
             Install sets up Rust {version} from rust-lang.org (about 400 MB)."
        ),
        RustToolchain::Missing { version, rustup_installed: true, .. } => format!(
            "Scripts and plugins are compiled on your machine and need Rust {version}, \
             which rustup has not installed yet (about 400 MB)."
        ),
        RustToolchain::Declined => "Scripts and plugins are compiled on your machine and need \
             Rust, which is not installed."
            .to_string(),
        RustToolchain::Installing { version } => format!(
            "Installing Rust {version}. This takes a few minutes, and you can keep using \
             Renzora while it runs."
        ),
        RustToolchain::Installed { version } => format!(
            "Rust {version} is installed. Scripts compile when a project opens, and plugins \
             that were waiting build the next time Renzora starts."
        ),
        // The installer's own words, first line only: rustup can print a page,
        // and the strip is one line high. The Console keeps the rest.
        RustToolchain::Failed(e) => format!(
            "Installing Rust failed: {}",
            elide(e.lines().next().unwrap_or("unknown error"), 160)
        ),
        _ => String::new(),
    }
}

/// Background tint and icon colour for each state.
fn tone(s: &RustToolchain) -> (Color, Color) {
    match s {
        RustToolchain::Installing { .. } => (ca(110, 150, 255, 30), accent()),
        RustToolchain::Installed { .. } => (ca(74, 200, 130, 30), success()),
        RustToolchain::Failed(_) => (ca(239, 68, 68, 34), error_color()),
        _ => (ca(255, 176, 64, 30), c(255, 186, 90)),
    }
}

fn icon(s: &RustToolchain) -> &'static str {
    match s {
        RustToolchain::Installing { .. } => "download-simple",
        RustToolchain::Installed { .. } => "check-circle",
        RustToolchain::Failed(_) => "warning",
        _ => "wrench",
    }
}

fn button_label(s: &RustToolchain) -> Option<String> {
    match s {
        RustToolchain::Missing { .. } | RustToolchain::Declined => {
            Some(renzora::lang::t_or("splash.rust.install", "Install Rust"))
        }
        RustToolchain::Failed(_) => Some(renzora::lang::t_or("splash.rust.retry", "Try again")),
        _ => None,
    }
}

/// Build the strip. Hidden until the check finds something to say.
pub(crate) fn build_strip(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let strip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(border_soft()),
            BLOCK,
            Name::new("splash-rust-strip"),
        ))
        .id();
    bind_display(commands, strip, |w| visible(&state(w)));
    bind_bg(commands, strip, |w| tone(&state(w)).0);

    let glyph = icon_text(commands, &fonts.phosphor, "wrench", ICON_TEXT, 16.0);
    commands.entity(glyph).insert(PASS_THROUGH);
    bind_text(commands, glyph, |w| {
        renzora_ember::phosphor_map::icon_glyph(icon(&state(w)))
            .map(String::from)
            .unwrap_or_default()
    });
    bind_text_color(commands, glyph, |w| tone(&state(w)).1);

    let body = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.5),
            TextColor(text()),
            // Grows into the row and may shrink below its text, so a long
            // message wraps instead of pushing the button off the window.
            Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: Val::Px(0.0), ..default() },
            PASS_THROUGH,
        ))
        .id();
    bind_text(commands, body, |w| message(&state(w)));

    let btn = commands
        .spawn((
            Node {
                height: Val::Px(30.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(accent()),
            UiInteraction::default(),
            BLOCK,
            HoverCursor(SystemCursorIcon::Pointer),
            RustInstallButton,
            Name::new("splash-rust-install"),
        ))
        .id();
    bind_display(commands, btn, |w| button_label(&state(w)).is_some());
    bind_bg(commands, btn, move |w| if is_hovered(w, btn) { accent_hover() } else { accent() });
    let btn_icon = icon_text(commands, &fonts.phosphor, "download-simple", (255, 255, 255), 14.0);
    commands.entity(btn_icon).insert(PASS_THROUGH);
    let btn_text = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.5),
            TextColor(white()),
            PASS_THROUGH,
        ))
        .id();
    bind_text(commands, btn_text, |w| button_label(&state(w)).unwrap_or_default());
    commands.entity(btn).add_children(&[btn_icon, btn_text]);

    commands.entity(strip).add_children(&[glyph, body, btn]);
    strip
}

/// Answer Install when the strip's button is pressed. The script crate decides
/// whether that means anything in the current state.
pub(crate) fn install_click(
    q: Query<&UiInteraction, (With<RustInstallButton>, Changed<UiInteraction>)>,
    mut answers: MessageWriter<RustToolchainAnswer>,
) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        answers.write(RustToolchainAnswer::Install);
    }
}
