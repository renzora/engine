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
    // Richer surface/row/border colors used by panels.
    pub border: (u8, u8, u8),
    pub popup_bg: (u8, u8, u8),
    pub row_even: (u8, u8, u8),
    pub row_odd: (u8, u8, u8),
    pub value_text: (u8, u8, u8),
    pub selection: (u8, u8, u8),
    pub section_bg: (u8, u8, u8),
    pub hover_bg: (u8, u8, u8),
    pub card_bg: (u8, u8, u8),
    pub tree_line: (u8, u8, u8),
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
            border: (60, 60, 74),
            popup_bg: (30, 30, 38),
            row_even: (34, 34, 42),
            row_odd: (30, 30, 37),
            value_text: (210, 210, 220),
            selection: (50, 54, 66),
            section_bg: (44, 44, 54),
            hover_bg: (40, 40, 50),
            card_bg: (41, 41, 48),
            tree_line: (64, 64, 76),
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
pub fn border() -> (u8, u8, u8) {
    palette().border
}
pub fn popup_bg() -> (u8, u8, u8) {
    palette().popup_bg
}
pub fn row_even() -> (u8, u8, u8) {
    palette().row_even
}
pub fn row_odd() -> (u8, u8, u8) {
    palette().row_odd
}
pub fn value_text() -> (u8, u8, u8) {
    palette().value_text
}
pub fn selection() -> (u8, u8, u8) {
    palette().selection
}
pub fn section_bg() -> (u8, u8, u8) {
    palette().section_bg
}
pub fn hover_bg() -> (u8, u8, u8) {
    palette().hover_bg
}
pub fn card_bg() -> (u8, u8, u8) {
    palette().card_bg
}
pub fn tree_line() -> (u8, u8, u8) {
    palette().tree_line
}

/// An sRGB byte triple as a bevy `Color`.
pub fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::srgb_u8(r, g, b)
}

// ── Per-widget-type style (geometry + typography + colors) ───────────────────
//
// The full style system: every widget *type* has an editable [`WidgetStyle`]
// (the Settings → Theme tab lists them). Like [`Palette`], the active
// [`StyleSheet`] is process-wide and runtime-safe (no egui) so the exported game
// runtime uses the same theme. Widgets resolve `style(Role::Button)` at spawn.

/// Which font family a widget renders with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontRole {
    #[default]
    Ui,
    Mono,
}

/// A simple drop shadow (logical px). `enabled == false` ⇒ no shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: (u8, u8, u8),
    pub alpha: f32,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            enabled: false,
            x: 0.0,
            y: 2.0,
            blur: 6.0,
            spread: 0.0,
            color: (0, 0, 0),
            alpha: 0.35,
        }
    }
}

/// The full editable style for one widget type: colors + geometry + typography.
/// All lengths are logical px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetStyle {
    pub bg: (u8, u8, u8),
    pub fg: (u8, u8, u8),
    pub border_color: (u8, u8, u8),
    pub border_width: f32,
    pub radius: f32,
    pub pad_x: f32,
    pub pad_y: f32,
    /// Inter-child gap (rows / icon+label spacing).
    pub gap: f32,
    pub margin: f32,
    pub font: FontRole,
    pub font_size: f32,
    pub shadow: Shadow,
}

impl WidgetStyle {
    /// A neutral base other roles tweak (transparent fill, primary text).
    const fn base() -> Self {
        Self {
            bg: PANEL_BG,
            fg: TEXT_PRIMARY,
            border_color: DIVIDER,
            border_width: 0.0,
            radius: 4.0,
            pad_x: 10.0,
            pad_y: 4.0,
            gap: 6.0,
            margin: 0.0,
            font: FontRole::Ui,
            font_size: 12.0,
            shadow: Shadow {
                enabled: false,
                x: 0.0,
                y: 2.0,
                blur: 6.0,
                spread: 0.0,
                color: (0, 0, 0),
                alpha: 0.35,
            },
        }
    }
}

/// The widget types that carry their own [`WidgetStyle`]. Extend as more widgets
/// adopt the style system (each new variant gets a field on [`StyleSheet`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Button,
    Dropdown,
    Input,
    Checkbox,
    Toggle,
    Slider,
    Panel,
    PanelHeader,
    Tab,
    TabActive,
    MenuItem,
    SectionHeader,
    Tooltip,
    Card,
    ScrollThumb,
    Row,
}

/// The active per-widget-type style table. Loaded from a theme `.toml`; defaults
/// reproduce the built-in dark look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleSheet {
    pub button: WidgetStyle,
    pub dropdown: WidgetStyle,
    pub input: WidgetStyle,
    pub checkbox: WidgetStyle,
    pub toggle: WidgetStyle,
    pub slider: WidgetStyle,
    pub panel: WidgetStyle,
    pub panel_header: WidgetStyle,
    pub tab: WidgetStyle,
    pub tab_active: WidgetStyle,
    pub menu_item: WidgetStyle,
    pub section_header: WidgetStyle,
    pub tooltip: WidgetStyle,
    pub card: WidgetStyle,
    pub scroll_thumb: WidgetStyle,
    pub row: WidgetStyle,
}

impl StyleSheet {
    pub fn get(&self, role: Role) -> WidgetStyle {
        match role {
            Role::Button => self.button,
            Role::Dropdown => self.dropdown,
            Role::Input => self.input,
            Role::Checkbox => self.checkbox,
            Role::Toggle => self.toggle,
            Role::Slider => self.slider,
            Role::Panel => self.panel,
            Role::PanelHeader => self.panel_header,
            Role::Tab => self.tab,
            Role::TabActive => self.tab_active,
            Role::MenuItem => self.menu_item,
            Role::SectionHeader => self.section_header,
            Role::Tooltip => self.tooltip,
            Role::Card => self.card,
            Role::ScrollThumb => self.scroll_thumb,
            Role::Row => self.row,
        }
    }
}

impl Default for StyleSheet {
    fn default() -> Self {
        let base = WidgetStyle::base();
        Self {
            button: WidgetStyle { bg: TAB_ACTIVE_BG, pad_x: 12.0, pad_y: 5.0, ..base },
            dropdown: WidgetStyle { bg: TAB_ACTIVE_BG, pad_x: 10.0, pad_y: 5.0, ..base },
            input: WidgetStyle { bg: (30, 30, 38), border_color: (60, 60, 74), border_width: 1.0, pad_x: 8.0, pad_y: 4.0, ..base },
            checkbox: WidgetStyle { bg: (0, 0, 0), border_color: (92, 92, 104), border_width: 1.0, radius: 3.0, ..base },
            toggle: WidgetStyle { radius: 7.0, ..base },
            slider: WidgetStyle { bg: (46, 46, 56), radius: 3.0, ..base },
            panel: WidgetStyle { bg: PANEL_BG, radius: 0.0, pad_x: 0.0, pad_y: 0.0, ..base },
            panel_header: WidgetStyle { bg: HEADER_BG, fg: TEXT_MUTED, radius: 0.0, pad_x: 8.0, pad_y: 5.0, ..base },
            tab: WidgetStyle { bg: HEADER_BG, fg: TEXT_MUTED, radius: 0.0, pad_x: 10.0, pad_y: 6.0, ..base },
            tab_active: WidgetStyle { bg: TAB_ACTIVE_BG, fg: TEXT_PRIMARY, radius: 0.0, pad_x: 10.0, pad_y: 6.0, ..base },
            menu_item: WidgetStyle { bg: (0, 0, 0), radius: 3.0, pad_x: 8.0, pad_y: 4.0, ..base },
            section_header: WidgetStyle { bg: (40, 40, 50), radius: 4.0, pad_x: 6.0, pad_y: 5.0, ..base },
            tooltip: WidgetStyle { bg: (30, 30, 38), border_color: (60, 60, 74), border_width: 1.0, radius: 4.0, pad_x: 8.0, pad_y: 4.0, ..base },
            card: WidgetStyle { bg: (28, 28, 34), border_color: (60, 60, 74), border_width: 1.0, radius: 8.0, pad_x: 12.0, pad_y: 12.0, ..base },
            scroll_thumb: WidgetStyle { bg: (96, 96, 112), radius: 5.0, ..base },
            row: WidgetStyle { bg: PANEL_BG, radius: 3.0, pad_x: 8.0, pad_y: 5.0, ..base },
        }
    }
}

static SHEET: LazyLock<RwLock<StyleSheet>> = LazyLock::new(|| RwLock::new(StyleSheet::default()));

/// Replace the active style sheet (called by the theme bridge / loader).
pub fn set_stylesheet(s: StyleSheet) {
    if let Ok(mut g) = SHEET.write() {
        *g = s;
    }
}

/// The current style sheet.
pub fn stylesheet() -> StyleSheet {
    SHEET.read().map(|g| *g).unwrap_or_default()
}

/// Resolve one widget type's style from the active sheet.
pub fn style(role: Role) -> WidgetStyle {
    stylesheet().get(role)
}
