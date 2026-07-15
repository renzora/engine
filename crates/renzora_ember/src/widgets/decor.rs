//! Decorative primitives — gradients, glow, and elevation shadows, plus a few
//! gradient-forward widgets (CTA button, badge pill, icon tile).
//!
//! These wrap bevy_ui's `BackgroundGradient` + `BoxShadow` so panels get the
//! "app-store" polish (soft colored glows, gradient call-to-actions) without
//! every caller re-deriving angles and alphas. Everything is hue-driven so it
//! adapts to the active theme's accent.

use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, BoxShadow, ColorStop, Gradient, LinearGradient};

use crate::cursor_icon::HoverCursor;
use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::{rgb, rgba};

// ── Gradients ────────────────────────────────────────────────────────────────

/// A linear gradient at `angle_deg` (CSS convention: 180° = top→bottom) through
/// the given auto-spaced color stops.
pub fn linear_gradient(angle_deg: f32, stops: impl IntoIterator<Item = Color>) -> BackgroundGradient {
    let stops: Vec<ColorStop> = stops.into_iter().map(ColorStop::auto).collect();
    BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(angle_deg.to_radians(), stops))])
}

/// Two-stop vertical (top → bottom) gradient.
pub fn gradient_v(top: Color, bottom: Color) -> BackgroundGradient {
    linear_gradient(180.0, [top, bottom])
}

/// Two-stop diagonal (top-left → bottom-right) gradient — the default "sheen".
pub fn gradient_diag(from: Color, to: Color) -> BackgroundGradient {
    linear_gradient(135.0, [from, to])
}

/// A single-hue diagonal gradient (subtle light-top → deeper-bottom) for
/// CTAs/tiles that adopt the area's accent hue and still adapt to any theme.
/// Deep enough to read as a rich fill (not washed toward white).
pub fn hue_gradient(hue: (u8, u8, u8)) -> BackgroundGradient {
    gradient_diag(lighten(rgb(hue), 0.10), darken(rgb(hue), 0.16))
}

/// A readable text/icon color for content sitting ON a hue fill — dark on light
/// hues (amber/gold), white on dark ones. Keeps gradient buttons legible in any
/// theme.
pub fn on_hue(hue: (u8, u8, u8)) -> (u8, u8, u8) {
    let lum = 0.299 * hue.0 as f32 + 0.587 * hue.1 as f32 + 0.114 * hue.2 as f32;
    if lum > 150.0 {
        (24, 18, 8)
    } else {
        (255, 255, 255)
    }
}

// ── Glow & elevation ─────────────────────────────────────────────────────────

/// A soft colored glow (no offset, slight spread, blurred). Insert on a node to
/// make it feel lit — use the accent/hue color at ~0.4 alpha.
pub fn glow(color: Color, blur: f32) -> BoxShadow {
    BoxShadow::new(color, Val::Px(0.0), Val::Px(0.0), Val::Px(2.0), Val::Px(blur))
}

/// A subtle downward elevation shadow (dark, offset, blurred) for lifted cards.
pub fn elevation(y: f32, blur: f32) -> BoxShadow {
    BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.35), Val::Px(0.0), Val::Px(y), Val::Px(0.0), Val::Px(blur))
}

// ── Gradient widgets ─────────────────────────────────────────────────────────

/// A rounded gradient tile with a centered white icon (the "app icon" look).
/// `size` is the square side in px.
pub fn gradient_icon_tile(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    icon: &str,
    size: f32,
) -> Entity {
    let tile = commands
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(size * 0.28)),
                flex_shrink: 0.0,
                ..default()
            },
            hue_gradient(hue),
            glow(rgba([hue.0, hue.1, hue.2, 95]), size * 0.55),
            Name::new("gradient-icon-tile"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, on_hue(hue), size * 0.5);
    commands.entity(tile).add_child(ic);
    tile
}

/// A small gradient pill (e.g. "POPULAR", "BEST VALUE"). Dark text over the
/// gradient reads best.
pub fn gradient_badge(
    commands: &mut Commands,
    fonts: &EmberFonts,
    from: Color,
    to: Color,
    label: &str,
) -> Entity {
    let badge = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            gradient_diag(from, to),
            Name::new("gradient-badge"),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 8.0), TextColor(Color::srgba(0.08, 0.06, 0.02, 1.0))))
        .id();
    commands.entity(badge).add_child(t);
    badge
}

/// Marks a [`gradient_button`] so its glow can pulse on hover.
#[derive(Component)]
pub struct GradientButton {
    glow_color: Color,
}

/// A prominent gradient call-to-action with a soft glow that intensifies on
/// hover. Insert your own click marker on the returned entity.
pub fn gradient_button(commands: &mut Commands, fonts: &EmberFonts, hue: (u8, u8, u8), label: &str) -> Entity {
    // A tight, low-alpha glow — enough to feel lit, not a hazy halo.
    let g = rgba([hue.0, hue.1, hue.2, 55]);
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            hue_gradient(hue),
            glow(g, 10.0),
            Interaction::default(),
            GradientButton { glow_color: g },
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("gradient-button"),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(on_hue(hue)))))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

/// Brighten a [`GradientButton`]'s glow on hover/press.
pub(crate) fn gradient_button_interact(
    mut q: Query<(&Interaction, &GradientButton, &mut BoxShadow), Changed<Interaction>>,
) {
    for (i, gb, mut shadow) in &mut q {
        let (alpha_mul, blur) = match i {
            Interaction::Hovered => (1.9, 16.0),
            Interaction::Pressed => (1.3, 8.0),
            Interaction::None => (1.0, 10.0),
        };
        let base = gb.glow_color.to_srgba();
        *shadow = glow(
            Color::srgba(base.red, base.green, base.blue, (base.alpha * alpha_mul).min(1.0)),
            blur,
        );
    }
}

/// Mix a color toward white by `amt` (0..1).
fn lighten(c: Color, amt: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba(
        s.red + (1.0 - s.red) * amt,
        s.green + (1.0 - s.green) * amt,
        s.blue + (1.0 - s.blue) * amt,
        s.alpha,
    )
}

/// Mix a color toward black by `amt` (0..1).
fn darken(c: Color, amt: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba(s.red * (1.0 - amt), s.green * (1.0 - amt), s.blue * (1.0 - amt), s.alpha)
}
