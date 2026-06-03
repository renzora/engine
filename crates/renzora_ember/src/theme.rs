//! The bevy-native theme: colors shared by every ember component (and the
//! editor chrome).
//!
//! Colors are read through a process-wide [`Palette`] so spawn functions (which
//! only have `&mut Commands`, not the world) can resolve the *current* theme
//! without threading a resource through every signature. A bridge maps the
//! editor's `ThemeManager` into this palette (see `set_palette`) and rebuilds
//! the UI on change, so swapping themes re-spawns widgets with the new colors.
//!
//! Widgets call the lowercase accessors (`window_bg()`, `accent()`, …). The
//! SCREAMING_CASE constants are the built-in defaults (the dark theme) and the
//! `Palette::default()` values — kept public for the rare spot that needs a
//! literal tuple (e.g. an alpha tweak).

use std::sync::{LazyLock, RwLock};

use bevy::prelude::Color;

// ── Default (dark) palette constants ─────────────────────────────────────────

pub const WINDOW_BG: (u8, u8, u8) = (24, 24, 30); // chrome (top bar/doc tabs/status/dock gaps)
pub const PANEL_BG: (u8, u8, u8) = (26, 26, 31); // leaf content (matches egui surfaces.panel)
pub const HEADER_BG: (u8, u8, u8) = (30, 30, 36); // doc tabs + panel tab headers
pub const TAB_ACTIVE_BG: (u8, u8, u8) = (50, 50, 62); // active tab
pub const TAB_HOVER_BG: (u8, u8, u8) = (42, 42, 52); // hovered (inactive) tab
pub const CLOSE_RED: (u8, u8, u8) = (216, 84, 84); // close × on hover
pub const DIVIDER: (u8, u8, u8) = (14, 14, 20); // split dividers (not black)
pub const TEXT_PRIMARY: (u8, u8, u8) = (230, 230, 240);
pub const TEXT_MUTED: (u8, u8, u8) = (148, 148, 160);
pub const PLACEHOLDER: (u8, u8, u8) = (110, 110, 122); // dim placeholder
pub const PLAY_GREEN: (u8, u8, u8) = (89, 191, 115); // semantic.success
pub const WARN_AMBER: (u8, u8, u8) = (224, 170, 72); // semantic.warning
pub const ACCENT_BLUE: (u8, u8, u8) = (80, 140, 255); // accent / active underline

// ── Runtime palette ──────────────────────────────────────────────────────────

/// The live set of theme colors every ember widget resolves against. Mapped from
/// the editor's `ThemeManager` by the theme bridge; defaults to the dark theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub window_bg: (u8, u8, u8),
    pub panel_bg: (u8, u8, u8),
    pub header_bg: (u8, u8, u8),
    pub tab_active: (u8, u8, u8),
    pub tab_hover: (u8, u8, u8),
    pub close_red: (u8, u8, u8),
    pub divider: (u8, u8, u8),
    pub text_primary: (u8, u8, u8),
    pub text_muted: (u8, u8, u8),
    pub placeholder: (u8, u8, u8),
    pub play_green: (u8, u8, u8),
    pub warn_amber: (u8, u8, u8),
    pub accent: (u8, u8, u8),
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            window_bg: WINDOW_BG,
            panel_bg: PANEL_BG,
            header_bg: HEADER_BG,
            tab_active: TAB_ACTIVE_BG,
            tab_hover: TAB_HOVER_BG,
            close_red: CLOSE_RED,
            divider: DIVIDER,
            text_primary: TEXT_PRIMARY,
            text_muted: TEXT_MUTED,
            placeholder: PLACEHOLDER,
            play_green: PLAY_GREEN,
            warn_amber: WARN_AMBER,
            accent: ACCENT_BLUE,
        }
    }
}

static CURRENT: LazyLock<RwLock<Palette>> = LazyLock::new(|| RwLock::new(Palette::default()));

/// Replace the live palette (called by the theme bridge when the active theme
/// changes). Widgets spawned afterward resolve the new colors.
pub fn set_palette(p: Palette) {
    if let Ok(mut g) = CURRENT.write() {
        *g = p;
    }
}

/// The current live palette.
pub fn palette() -> Palette {
    CURRENT.read().map(|g| *g).unwrap_or_default()
}

// Accessors — what widgets call. Each reads the live palette.
pub fn window_bg() -> (u8, u8, u8) {
    palette().window_bg
}
pub fn panel_bg() -> (u8, u8, u8) {
    palette().panel_bg
}
pub fn header_bg() -> (u8, u8, u8) {
    palette().header_bg
}
pub fn tab_active() -> (u8, u8, u8) {
    palette().tab_active
}
pub fn tab_hover() -> (u8, u8, u8) {
    palette().tab_hover
}
pub fn close_red() -> (u8, u8, u8) {
    palette().close_red
}
pub fn divider() -> (u8, u8, u8) {
    palette().divider
}
pub fn text_primary() -> (u8, u8, u8) {
    palette().text_primary
}
pub fn text_muted() -> (u8, u8, u8) {
    palette().text_muted
}
pub fn placeholder() -> (u8, u8, u8) {
    palette().placeholder
}
pub fn play_green() -> (u8, u8, u8) {
    palette().play_green
}
pub fn warn_amber() -> (u8, u8, u8) {
    palette().warn_amber
}
pub fn accent() -> (u8, u8, u8) {
    palette().accent
}

/// An sRGB byte triple as a bevy `Color`.
pub fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::srgb_u8(r, g, b)
}
