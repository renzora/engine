//! The runtime **theming system** (foundation for the per-widget Theme).
//!
//! A swappable [`Theme`] resource holds a *style token* per widget [`Role`]
//! (`Button`, `Input`, `Card`, `Tab`, …) — each a [`StyleToken`] of named,
//! individually-targetable style elements (per-state fills, border, geometry,
//! text). Every themeable widget carries a [`Styled`] component (`role` + state)
//! instead of baking in its own colors; [`apply_theme`] paints each `Styled`
//! entity from `theme.token(role)` for its state, so changing the `Theme` (or a
//! widget's state) repaints with no rebuild.
//!
//! The `Theme` is `Reflect` (the editor enumerates every widget + element) and
//! `Serialize`/`Deserialize` (loads from project `themes/*.toml`, runtime-safe so
//! the exported game uses the same Theme). Colors are [`Rgba`] — hex in `.toml`.
//! Defaults cascade from the runtime [`crate::theme::Palette`].

use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

use crate::theme::{
    accent, border, header_bg, hover_bg, panel_bg, popup_bg, section_bg, tab_active, tab_hover,
    text_muted, text_primary,
};

/// Registers the theme resource + the painter. The [`Theme`] is the source of
/// truth (loaded from `themes/*.toml` by the editor bridge / runtime loader, via
/// [`Theme::from_toml`]); [`apply_theme`] repaints every `Styled` widget whenever
/// it changes — so swapping or editing the Theme repaints with no rebuild.
pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Theme>()
            .register_type::<Theme>()
            .register_type::<StyleToken>()
            .register_type::<NodeGraphStyle>()
            .register_type::<Rgba>()
            .add_systems(Update, apply_theme);
    }
}

// ── Color ────────────────────────────────────────────────────────────────────

/// An sRGB color with alpha — the theme's color element type. Serializes as a
/// hex string (`#RRGGBB` / `#RRGGBBAA`) for `.toml`; `Reflect` so the editor can
/// enumerate + edit it.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const NONE: Self = Self { r: 0, g: 0, b: 0, a: 0 };

    /// From an opaque `(r, g, b)` triple (the palette accessor type).
    pub const fn rgb(t: (u8, u8, u8)) -> Self {
        Self { r: t.0, g: t.1, b: t.2, a: 255 }
    }

    /// As a bevy `Color`.
    pub fn color(self) -> Color {
        Color::srgba(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        )
    }

    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }

    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            6 => Some(Self { r: byte(0)?, g: byte(2)?, b: byte(4)?, a: 255 }),
            8 => Some(Self { r: byte(0)?, g: byte(2)?, b: byte(4)?, a: byte(6)? }),
            _ => None,
        }
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgba::from_hex(&s).ok_or_else(|| de::Error::custom(format!("bad hex color: {s}")))
    }
}

// ── State / Role ─────────────────────────────────────────────────────────────

/// The interaction/selection state a styled widget can be in. Each maps to a
/// `bg_*` element in [`StyleToken`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WidgetState {
    #[default]
    Normal,
    Hover,
    Pressed,
    /// Selected / on / focused.
    Active,
    Disabled,
}

/// Which token in the [`Theme`] a widget paints from.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Role {
    Button,
    ButtonAccent,
    /// Fixed-size icon button (no text padding).
    IconButton,
    Input,
    Checkbox,
    Segment,
    Toggle,
    Card,
    Badge,
    Alert,
    Toast,
    Tab,
    Panel,
    Menu,
}

// ── StyleToken: one widget's style elements ──────────────────────────────────

/// A themeable box style: per-state fills, border, geometry, text — each a
/// named, individually-targetable element. `apply_theme` writes these onto a
/// widget's `BackgroundColor` / `BorderColor` / `Node`.
#[derive(Reflect, Clone, Serialize, Deserialize)]
pub struct StyleToken {
    pub bg: Rgba,
    pub bg_hover: Rgba,
    pub bg_pressed: Rgba,
    pub bg_active: Rgba,
    pub bg_disabled: Rgba,
    pub border: Rgba,
    pub border_active: Rgba,
    pub border_width: f32,
    pub radius: f32,
    pub pad_x: f32,
    pub pad_y: f32,
    pub text: Rgba,
    pub text_muted: Rgba,
}

impl StyleToken {
    fn new(bg: Rgba) -> Self {
        Self {
            bg,
            bg_hover: bg,
            bg_pressed: bg,
            bg_active: bg,
            bg_disabled: bg,
            border: Rgba::NONE,
            border_active: Rgba::NONE,
            border_width: 0.0,
            radius: 4.0,
            pad_x: 0.0,
            pad_y: 0.0,
            text: Rgba::rgb(text_primary()),
            text_muted: Rgba::rgb(text_muted()),
        }
    }
    fn hover(mut self, c: Rgba) -> Self {
        self.bg_hover = c;
        self
    }
    fn pressed(mut self, c: Rgba) -> Self {
        self.bg_pressed = c;
        self
    }
    fn active(mut self, c: Rgba) -> Self {
        self.bg_active = c;
        self
    }
    fn border(mut self, c: Rgba, width: f32) -> Self {
        self.border = c;
        self.border_active = c;
        self.border_width = width;
        self
    }
    fn border_active(mut self, c: Rgba) -> Self {
        self.border_active = c;
        self
    }
    fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    fn pad(mut self, x: f32, y: f32) -> Self {
        self.pad_x = x;
        self.pad_y = y;
        self
    }

    /// The background fill for a given state.
    pub fn bg_for(&self, state: WidgetState) -> Color {
        match state {
            WidgetState::Normal => self.bg,
            WidgetState::Hover => self.bg_hover,
            WidgetState::Pressed => self.bg_pressed,
            WidgetState::Active => self.bg_active,
            WidgetState::Disabled => self.bg_disabled,
        }
        .color()
    }

    /// The border color for a given state (`Active` uses the focus/active color).
    pub fn border_for(&self, state: WidgetState) -> Color {
        if state == WidgetState::Active {
            self.border_active.color()
        } else {
            self.border.color()
        }
    }
}

// ── Bespoke per-widget styles (multi-element widgets) ────────────────────────
//
// Box widgets share [`StyleToken`]; widgets with several distinct, independently
// targetable elements get their own struct so editing one element (e.g. a node
// graph's cable) never smears across the others (its card bg, header, …).

/// Style for the node-graph widget — every element individually targetable.
#[derive(Reflect, Clone, Serialize, Deserialize)]
pub struct NodeGraphStyle {
    pub canvas_bg: Rgba,
    pub canvas_border: Rgba,
    pub node_bg: Rgba,
    pub node_border: Rgba,
    pub node_header: Rgba,
    pub node_selected_bg: Rgba,
    pub node_selected_border: Rgba,
    pub cable: Rgba,
    pub cable_selected: Rgba,
    pub title_text: Rgba,
    pub port_text: Rgba,
}

impl Default for NodeGraphStyle {
    fn default() -> Self {
        let c = Rgba::rgb;
        Self {
            canvas_bg: c(popup_bg()),
            canvas_border: c(border()),
            node_bg: c(section_bg()),
            node_border: c(border()),
            node_header: c(tab_active()),
            node_selected_bg: c(hover_bg()),
            node_selected_border: c(accent()),
            cable: c(accent()),
            cable_selected: c(accent()),
            title_text: c(text_primary()),
            port_text: c(text_muted()),
        }
    }
}

// ── Theme: all per-role tokens ───────────────────────────────────────────────

/// The active theme — one [`StyleToken`] per [`Role`]. Swap or mutate this and
/// every styled widget repaints. `Reflect` (editor enumeration) +
/// `Serialize`/`Deserialize` (project `themes/*.toml`).
#[derive(Resource, Reflect, Clone, Serialize, Deserialize)]
#[reflect(Resource)]
#[serde(default)]
pub struct Theme {
    pub button: StyleToken,
    pub button_accent: StyleToken,
    pub icon_button: StyleToken,
    pub input: StyleToken,
    pub checkbox: StyleToken,
    pub segment: StyleToken,
    pub toggle: StyleToken,
    pub card: StyleToken,
    pub badge: StyleToken,
    pub alert: StyleToken,
    pub toast: StyleToken,
    pub tab: StyleToken,
    pub panel: StyleToken,
    pub menu: StyleToken,
    // Bespoke multi-element widget styles.
    pub node_graph: NodeGraphStyle,
}

impl Theme {
    pub fn token(&self, role: Role) -> &StyleToken {
        match role {
            Role::Button => &self.button,
            Role::ButtonAccent => &self.button_accent,
            Role::IconButton => &self.icon_button,
            Role::Input => &self.input,
            Role::Checkbox => &self.checkbox,
            Role::Segment => &self.segment,
            Role::Toggle => &self.toggle,
            Role::Card => &self.card,
            Role::Badge => &self.badge,
            Role::Alert => &self.alert,
            Role::Toast => &self.toast,
            Role::Tab => &self.tab,
            Role::Panel => &self.panel,
            Role::Menu => &self.menu,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        let c = Rgba::rgb;
        Self {
            button: StyleToken::new(c(tab_active()))
                .hover(c(hover_bg()))
                .pressed(c(accent()))
                .active(c(accent()))
                .radius(4.0)
                .pad(12.0, 6.0),
            button_accent: StyleToken::new(c(accent()))
                .hover(c(accent()))
                .pressed(c(accent()))
                .radius(4.0)
                .pad(12.0, 6.0),
            icon_button: StyleToken::new(c(tab_active()))
                .hover(c(hover_bg()))
                .pressed(c(accent()))
                .radius(4.0),
            input: StyleToken::new(c(popup_bg()))
                .border(c(border()), 1.0)
                .border_active(c(accent()))
                .radius(4.0)
                .pad(8.0, 5.0),
            checkbox: StyleToken::new(Rgba::NONE)
                .active(c(accent()))
                .border(c(border()), 1.0)
                .radius(3.0),
            segment: StyleToken::new(Rgba::NONE)
                .hover(c(tab_hover()))
                .active(c(accent()))
                .radius(0.0)
                .pad(12.0, 5.0),
            toggle: StyleToken::new(c(tab_hover()))
                .active(c(accent()))
                .radius(10.0)
                .pad(2.0, 2.0),
            card: StyleToken::new(c(panel_bg()))
                .border(c(border()), 1.0)
                .radius(6.0)
                .pad(12.0, 12.0),
            badge: StyleToken::new(c(accent()))
                .radius(8.0)
                .pad(7.0, 2.0),
            alert: StyleToken::new(c(section_bg()))
                .border(c(border()), 1.0)
                .radius(6.0)
                .pad(10.0, 10.0),
            toast: StyleToken::new(c(section_bg()))
                .border(c(border()), 1.0)
                .radius(6.0)
                .pad(10.0, 10.0),
            tab: StyleToken::new(c(header_bg()))
                .hover(c(tab_hover()))
                .active(c(tab_active()))
                .radius(0.0)
                .pad(10.0, 5.0),
            panel: StyleToken::new(c(panel_bg())).radius(0.0),
            menu: StyleToken::new(c(popup_bg())).radius(4.0).pad(2.0, 2.0),
            node_graph: NodeGraphStyle::default(),
        }
    }
}

impl Theme {
    /// Load a theme from TOML, **cascading over the palette-derived defaults** —
    /// a theme only specifies the elements it overrides (any depth: a whole
    /// `[button]` table, or just `button.bg`). Built on a deep merge of the
    /// loaded TOML over the serialized default.
    pub fn from_toml(s: &str) -> Result<Theme, toml::de::Error> {
        let base = toml::Value::try_from(Theme::default())
            .expect("Theme always serializes to a TOML value");
        let over: toml::Value = toml::from_str(s)?;
        deep_merge(base, over).try_into()
    }
}

/// Recursively merge `over` onto `base`: tables merge key-by-key, leaves are
/// replaced by `over`. Lets a theme override individual elements.
fn deep_merge(base: toml::Value, over: toml::Value) -> toml::Value {
    match (base, over) {
        (toml::Value::Table(mut b), toml::Value::Table(o)) => {
            for (k, ov) in o {
                let merged = match b.remove(&k) {
                    Some(bv) => deep_merge(bv, ov),
                    None => ov,
                };
                b.insert(k, merged);
            }
            toml::Value::Table(b)
        }
        (_, over) => over,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_cascade_overrides_only_specified() {
        // A theme that only sets the button background.
        let t = Theme::from_toml("[button]\nbg = \"#FF0000\"\n").unwrap();
        assert_eq!(t.button.bg, Rgba { r: 255, g: 0, b: 0, a: 255 });
        // Everything else keeps the palette-derived default.
        let d = Theme::default();
        assert_eq!(t.button.bg_hover, d.button.bg_hover);
        assert_eq!(t.input.bg, d.input.bg);
        assert_eq!(t.node_graph.cable, d.node_graph.cable);
    }

    #[test]
    fn toml_overrides_a_bespoke_element() {
        let t = Theme::from_toml("[node_graph]\ncable = \"#8CD0D3\"\n").unwrap();
        assert_eq!(t.node_graph.cable, Rgba { r: 0x8C, g: 0xD0, b: 0xD3, a: 255 });
        // The node's card bg is untouched — no smearing.
        assert_eq!(t.node_graph.node_bg, Theme::default().node_graph.node_bg);
    }

    #[test]
    fn rgba_hex_roundtrips() {
        let c = Rgba { r: 18, g: 52, b: 86, a: 255 };
        assert_eq!(c.to_hex(), "#123456");
        assert_eq!(Rgba::from_hex("#123456"), Some(c));
    }
}

/// Marks a widget as themeable: it paints from `theme.token(role)` for `state`.
#[derive(Component, Clone, Copy)]
pub struct Styled {
    pub role: Role,
    pub state: WidgetState,
}

impl Styled {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            state: WidgetState::Normal,
        }
    }
    pub fn with_state(role: Role, state: WidgetState) -> Self {
        Self { role, state }
    }
}

/// Paint every styled widget whose `Styled` (state/role) or the `Theme` changed.
pub fn apply_theme(
    theme: Res<Theme>,
    mut q: Query<(
        Ref<Styled>,
        &mut BackgroundColor,
        Option<&mut BorderColor>,
        &mut Node,
    )>,
) {
    let repaint_all = theme.is_changed();
    for (styled, mut bg, border, mut node) in &mut q {
        if !repaint_all && !styled.is_changed() {
            continue;
        }
        let token = theme.token(styled.role);
        bg.0 = token.bg_for(styled.state);
        node.border = UiRect::all(Val::Px(token.border_width));
        node.border_radius = BorderRadius::all(Val::Px(token.radius));
        node.padding = UiRect::axes(Val::Px(token.pad_x), Val::Px(token.pad_y));
        if let Some(mut bc) = border {
            *bc = BorderColor::all(token.border_for(styled.state));
        }
    }
}
