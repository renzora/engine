//! The bevy-native theme: colors shared by every ember component (and the
//! editor chrome). Values are calibrated to the editor's appearance, derived
//! from `renzora_theme`'s defaults. A live, swappable `Theme` resource will
//! replace these constants later, but the values are the source of truth.

use bevy::prelude::Color;

pub const WINDOW_BG: (u8, u8, u8) = (24, 24, 30); // chrome (top bar/doc tabs/status/dock gaps)
pub const PANEL_BG: (u8, u8, u8) = (33, 33, 39); // leaf content (lighter than chrome)
pub const HEADER_BG: (u8, u8, u8) = (37, 37, 44); // doc tabs + panel tab headers
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

/// An sRGB byte triple as a bevy `Color`.
pub fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::srgb_u8(r, g, b)
}
