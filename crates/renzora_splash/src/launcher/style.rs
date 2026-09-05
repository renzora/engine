//! Palette, geometry and the small shared helpers of the splash dashboard.
//!
//! Split out of `launcher.rs` when the launcher grew from one centred column
//! into a dashboard: a rail, three pages and a status strip all draw the same
//! surfaces, and a colour only one of them could see would drift from the rest
//! on the next change.

use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, Gradient, LinearGradient};

use renzora_ember::reactive::Rx;

// ── Palette ──────────────────────────────────────────────────────────────────

pub(crate) fn c(r: u8, g: u8, b: u8) -> Color {
    Color::srgb_u8(r, g, b)
}
pub(crate) fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

/// The window's own ground — what shows when the cinematic isn't running (an
/// integrated GPU; see `post::gate_post_camera`), so it has to stand on its own:
/// a near black with a trace of blue in it, matching the chamber's unlit air.
pub(crate) fn window_bg() -> Color {
    c(4, 5, 9)
}

/// A dashboard panel. Deliberately translucent, and this is the whole reason the
/// dashboard did not simply become an opaque dialog: the Light Chamber cinematic
/// keeps running behind the window, and a solid surface would have thrown away a
/// render nobody would ever see again.
pub(crate) fn surface() -> Color {
    ca(10, 12, 20, 214)
}
/// The navigation rail, a shade deeper than [`surface`] so the two read as
/// separate planes rather than one panel with a line drawn on it.
pub(crate) fn rail_bg() -> Color {
    ca(7, 9, 15, 228)
}
pub(crate) fn panel_hover() -> Color {
    ca(30, 34, 52, 250)
}
pub(crate) fn border_soft() -> Color {
    c(36, 40, 56)
}
pub(crate) fn btn_dark() -> Color {
    ca(12, 14, 22, 235)
}
pub(crate) fn btn_dark_hover() -> Color {
    ca(26, 30, 46, 245)
}
pub(crate) fn text() -> Color {
    c(224, 228, 240)
}
pub(crate) fn text_muted() -> Color {
    c(150, 158, 178)
}
pub(crate) fn accent() -> Color {
    c(110, 150, 255)
}
pub(crate) fn accent_hover() -> Color {
    c(140, 175, 255)
}
pub(crate) fn success() -> Color {
    c(74, 200, 130)
}
pub(crate) fn error_color() -> Color {
    c(239, 68, 68)
}
pub(crate) fn white() -> Color {
    Color::WHITE
}

/// Icon colours, as the `(u8, u8, u8)` triple `icon_text` takes.
pub(crate) const ICON_MUTED: (u8, u8, u8) = (150, 158, 178);
pub(crate) const ICON_TEXT: (u8, u8, u8) = (224, 228, 240);
pub(crate) const ICON_ACCENT: (u8, u8, u8) = (110, 150, 255);

// ── Geometry ─────────────────────────────────────────────────────────────────

/// Width of the navigation rail. Wide enough for an icon, a label and the
/// account row's username without eliding a name of ordinary length.
pub(crate) const RAIL_W: f32 = 214.0;
/// Height of the title bar, which is also the window's drag handle.
pub(crate) const TITLEBAR_H: f32 = 38.0;
/// Height of the status strip along the bottom. Sized around its text rather
/// than the other way round: at 26px the strip fitted only a 10.5px label, which
/// is below the size anything else on the dashboard is set in and read as
/// fine print rather than as the build you are running.
pub(crate) const STATUSBAR_H: f32 = 30.0;
/// Padding inside a page body.
pub(crate) const PAGE_PAD: f32 = 22.0;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A vertical (top → bottom) two-stop background gradient for a card.
pub(crate) fn card_gradient(top: Color, bot: Color) -> BackgroundGradient {
    BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
        std::f32::consts::PI, // 180° → top to bottom
        vec![ColorStop::auto(top), ColorStop::auto(bot)],
    ))])
}

pub(crate) fn is_hovered(w: &Rx, e: Entity) -> bool {
    matches!(
        w.get::<Interaction>(e),
        Some(Interaction::Hovered) | Some(Interaction::Pressed)
    )
}

/// Shorten a path from the left, keeping the tail — the end of a path is the
/// part that identifies the project.
pub(crate) fn elide_path(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let tail: String = s.chars().rev().take(max).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{tail}")
    } else {
        s.to_string()
    }
}

/// Shorten from the right, keeping the head — for a name, where the beginning is
/// what identifies it.
///
/// Counts `char`s rather than bytes: these are user-entered names that routinely
/// carry accents, CJK or emoji, and slicing a `String` by byte index in the
/// middle of one of those panics.
pub(crate) fn elide(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

/// A page heading + one line of subtext, the shape every dashboard page opens
/// with so that switching pages does not also switch layouts.
pub(crate) fn page_header(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    title: &str,
    subtitle: &str,
) -> Entity {
    use bevy::ui::FocusPolicy;
    use renzora_ember::font::ui_font;

    let col = commands
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
    let h = commands
        .spawn((
            Text::new(title.to_string()),
            ui_font(&fonts.ui, 17.0),
            TextColor(text()),
            FocusPolicy::Pass,
        ))
        .id();
    let sub = commands
        .spawn((
            Text::new(subtitle.to_string()),
            ui_font(&fonts.ui, 11.5),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(col).add_children(&[h, sub]);
    col
}
