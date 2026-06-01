//! The runtime **theming system**.
//!
//! A swappable [`Theme`] resource holds a *style token* per widget [`Role`]
//! (`Button`, `Input`, `Card`, `Tab`, …). Every themeable widget carries a
//! [`Styled`] component (`role` + interaction `state`) instead of baking in its
//! own colors. [`apply_theme`] paints each `Styled` entity from
//! `theme.token(role)` for its current `state` — so changing the `Theme` (or a
//! widget's `Styled.state`) repaints with no rebuild. This is the seam a future
//! theme-editor panel edits to restyle buttons / tabs / docks / panels at once.

use bevy::prelude::*;

use crate::theme::{
    rgb, ACCENT_BLUE, HEADER_BG, PANEL_BG, TAB_ACTIVE_BG, TAB_HOVER_BG, TEXT_MUTED, TEXT_PRIMARY,
};

/// Registers the theme resource + the painter.
pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Theme>()
            .add_systems(Update, apply_theme);
    }
}

/// The interaction/selection state a styled widget can be in. Each maps to a
/// `bg_*` variant in [`StyleToken`].
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

/// A themeable box style: per-state fills, border, geometry. `apply_theme`
/// writes these onto a widget's `BackgroundColor` / `BorderColor` / `Node`.
#[derive(Clone)]
pub struct StyleToken {
    pub bg: Color,
    pub bg_hover: Color,
    pub bg_pressed: Color,
    pub bg_active: Color,
    pub bg_disabled: Color,
    pub border: Color,
    pub border_active: Color,
    pub border_width: f32,
    pub radius: f32,
    pub padding: UiRect,
    pub text: Color,
    pub text_muted: Color,
}

impl StyleToken {
    fn new(bg: Color) -> Self {
        Self {
            bg,
            bg_hover: bg,
            bg_pressed: bg,
            bg_active: bg,
            bg_disabled: bg,
            border: Color::NONE,
            border_active: Color::NONE,
            border_width: 0.0,
            radius: 4.0,
            padding: UiRect::all(Val::Px(0.0)),
            text: rgb(TEXT_PRIMARY),
            text_muted: rgb(TEXT_MUTED),
        }
    }
    fn hover(mut self, c: Color) -> Self {
        self.bg_hover = c;
        self
    }
    fn pressed(mut self, c: Color) -> Self {
        self.bg_pressed = c;
        self
    }
    fn active(mut self, c: Color) -> Self {
        self.bg_active = c;
        self
    }
    fn border(mut self, c: Color, width: f32) -> Self {
        self.border = c;
        self.border_active = c;
        self.border_width = width;
        self
    }
    fn border_active(mut self, c: Color) -> Self {
        self.border_active = c;
        self
    }
    fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    fn padding(mut self, p: UiRect) -> Self {
        self.padding = p;
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
    }

    /// The border color for a given state (`Active` uses the focus/active color).
    pub fn border_for(&self, state: WidgetState) -> Color {
        if state == WidgetState::Active {
            self.border_active
        } else {
            self.border
        }
    }
}

/// The active theme — one [`StyleToken`] per [`Role`]. Swap or mutate this and
/// every styled widget repaints.
#[derive(Resource, Clone)]
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
}

impl Theme {
    /// The token for a role.
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
        let text_pad = UiRect::axes(Val::Px(12.0), Val::Px(6.0));
        let input_pad = UiRect::axes(Val::Px(8.0), Val::Px(5.0));
        Self {
            button: StyleToken::new(rgb(TAB_ACTIVE_BG))
                .hover(rgb((64, 64, 78)))
                .pressed(rgb(ACCENT_BLUE))
                .active(rgb(ACCENT_BLUE))
                .radius(4.0)
                .padding(text_pad),
            button_accent: StyleToken::new(rgb(ACCENT_BLUE))
                .hover(rgb((100, 158, 255)))
                .pressed(rgb((70, 126, 235)))
                .radius(4.0)
                .padding(text_pad),
            icon_button: StyleToken::new(rgb(TAB_ACTIVE_BG))
                .hover(rgb((64, 64, 78)))
                .pressed(rgb(ACCENT_BLUE))
                .radius(4.0),
            input: StyleToken::new(rgb((28, 28, 34)))
                .border(rgb((70, 70, 82)), 1.0)
                .border_active(rgb(ACCENT_BLUE))
                .radius(4.0)
                .padding(input_pad),
            checkbox: StyleToken::new(Color::NONE)
                .active(rgb(ACCENT_BLUE))
                .border(rgb((92, 92, 104)), 1.0)
                .radius(3.0),
            segment: StyleToken::new(Color::NONE)
                .hover(rgb(TAB_HOVER_BG))
                .active(rgb(ACCENT_BLUE))
                .radius(0.0)
                .padding(UiRect::axes(Val::Px(12.0), Val::Px(5.0))),
            toggle: StyleToken::new(rgb((60, 60, 70)))
                .active(rgb(ACCENT_BLUE))
                .radius(10.0)
                .padding(UiRect::all(Val::Px(2.0))),
            card: StyleToken::new(rgb(PANEL_BG))
                .border(rgb((48, 48, 58)), 1.0)
                .radius(6.0)
                .padding(UiRect::all(Val::Px(12.0))),
            badge: StyleToken::new(rgb(ACCENT_BLUE))
                .radius(8.0)
                .padding(UiRect::axes(Val::Px(7.0), Val::Px(2.0))),
            alert: StyleToken::new(rgb((38, 38, 48)))
                .border(rgb((60, 60, 74)), 1.0)
                .radius(6.0)
                .padding(UiRect::all(Val::Px(10.0))),
            toast: StyleToken::new(rgb((44, 44, 55)))
                .border(rgb((64, 64, 78)), 1.0)
                .radius(6.0)
                .padding(UiRect::all(Val::Px(10.0))),
            tab: StyleToken::new(rgb(HEADER_BG))
                .hover(rgb(TAB_HOVER_BG))
                .active(rgb(TAB_ACTIVE_BG))
                .radius(0.0)
                .padding(UiRect::axes(Val::Px(10.0), Val::Px(5.0))),
            panel: StyleToken::new(rgb(PANEL_BG)).radius(0.0),
            menu: StyleToken::new(rgb((30, 30, 38)))
                .radius(4.0)
                .padding(UiRect::all(Val::Px(2.0))),
        }
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
        node.padding = token.padding;
        if let Some(mut bc) = border {
            *bc = BorderColor::all(token.border_for(styled.state));
        }
    }
}
