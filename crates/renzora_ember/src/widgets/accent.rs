//! Accent widgets — a hue-tinted family for surfaces that carry an identity
//! color (the editor's Community panels, game hubs, anything that should feel
//! warmer than tool chrome).
//!
//! The hue is used only as low-alpha tints, icon color, and accents over the
//! active theme's surfaces, so accent UI adapts to any theme. Includes two
//! small interaction systems: [`hover_tint`] (free-color hover feedback for
//! custom-colored elements the role stylesheet can't drive) and
//! [`pulse_dots`] (gently breathing status dots).

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::{card_bg, hover_bg, placeholder, rgb, rgba, text_muted, text_primary};

/// An accent hue at `alpha` (0-255) — the family's tinting primitive.
pub fn tint((r, g, b): (u8, u8, u8), alpha: u8) -> Color {
    rgba([r, g, b, alpha])
}

// ── Hover feedback for free-colored elements ─────────────────────────────────

/// Live background feedback for any `Interaction` entity whose colors are
/// chosen by the caller (accent-tinted cards/buttons) rather than by a
/// stylesheet role. Driven by [`hover_tint`].
#[derive(Component)]
pub struct HoverTint {
    pub base: Color,
    pub hover: Color,
    pub pressed: Color,
}

impl HoverTint {
    /// Feedback over a solid base color.
    pub fn solid(base: Color, hover: Color, pressed: Color) -> Self {
        Self { base, hover, pressed }
    }

    /// Feedback for a hue-tinted surface: base/hover alpha steps.
    pub fn tinted(hue: (u8, u8, u8), base_alpha: u8, hover_alpha: u8) -> Self {
        Self {
            base: tint(hue, base_alpha),
            hover: tint(hue, hover_alpha),
            pressed: tint(hue, hover_alpha.saturating_add(30)),
        }
    }
}

pub(crate) fn hover_tint(
    mut q: Query<(&Interaction, &HoverTint, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, h, mut bg) in &mut q {
        bg.0 = match interaction {
            Interaction::None => h.base,
            Interaction::Hovered => h.hover,
            Interaction::Pressed => h.pressed,
        };
    }
}

// ── Presence pulse ───────────────────────────────────────────────────────────

/// A gently breathing dot — status that feels alive rather than labeled.
/// Vary `phase` per dot so a list doesn't blink in lockstep.
#[derive(Component)]
pub struct PulseDot {
    pub hue: (u8, u8, u8),
    pub phase: f32,
}

pub(crate) fn pulse_dots(time: Res<Time>, mut q: Query<(&PulseDot, &mut BackgroundColor)>) {
    let t = time.elapsed_secs();
    for (dot, mut bg) in &mut q {
        // 0.55..1.0 alpha, ~2.4s cycle.
        let a = 0.775 + 0.225 * (t * 2.6 + dot.phase).sin();
        bg.0 = tint(dot.hue, (a * 255.0) as u8);
    }
}

/// A small status dot driven by [`pulse_dots`] when `pulse` is true, or a
/// static muted dot when false (offline).
pub fn status_dot(commands: &mut Commands, hue: (u8, u8, u8), size: f32, pulse: bool) -> Entity {
    let mut e = commands.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border_radius: BorderRadius::all(Val::Px(size / 2.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(if pulse { tint(hue, 255) } else { rgba([120, 120, 134, 255]) }),
    ));
    let id = e.id();
    if pulse {
        e.insert(PulseDot { hue, phase: (id.to_bits() % 7) as f32 });
    }
    id
}

// ── Building blocks ──────────────────────────────────────────────────────────

/// The accent icon in a softly glowing rounded badge.
pub fn icon_badge(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    icon: &str,
    size: f32,
) -> Entity {
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(size * 0.3)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(tint(hue, 38)),
            BorderColor::all(tint(hue, 70)),
            Name::new("icon-badge"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, hue, size * 0.52);
    commands.entity(badge).add_child(ic);
    badge
}

/// The pieces of an [`accent_banner`], exposed so callers can bind live text
/// to the title/subtitle or append buttons to the actions row.
pub struct AccentBanner {
    pub root: Entity,
    pub title: Entity,
    pub subtitle: Entity,
    pub actions: Entity,
}

/// An identity banner: tinted surface, glowing icon badge, title + subtitle,
/// and a right-aligned actions row.
pub fn accent_banner(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    icon: &str,
    title: &str,
    subtitle: &str,
) -> AccentBanner {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(tint(hue, 22)),
            BorderColor::all(tint(hue, 52)),
            Name::new("accent-banner"),
        ))
        .id();
    let badge = icon_badge(commands, fonts, hue, icon, 34.0);
    let col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), flex_grow: 1.0, ..default() })
        .id();
    let t = commands
        .spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 14.5), TextColor(rgb(text_primary()))))
        .id();
    let s = commands
        .spawn((Text::new(subtitle.to_string()), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(col).add_children(&[t, s]);
    let actions = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    commands.entity(root).add_children(&[badge, col, actions]);
    AccentBanner { root, title: t, subtitle: s, actions }
}

/// A content card row: theme surface, rounded, hairline border. `interactive`
/// adds pointer cursor + hover feedback + `Interaction` (insert your own
/// marker to catch clicks).
pub fn accent_card(commands: &mut Commands, interactive: bool) -> Entity {
    let base = rgb(card_bg());
    let mut e = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(base),
        BorderColor::all(rgba([255, 255, 255, 10])),
        Name::new("accent-card"),
    ));
    if interactive {
        e.insert((
            Interaction::default(),
            HoverTint::solid(base, rgb(hover_bg()), rgb(hover_bg())),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
        ));
    }
    e.id()
}

/// A filled call-to-action pill in the accent hue: white label, hover-brightens.
pub fn accent_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    label: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(11.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(tint(hue, 210)),
            Interaction::default(),
            HoverTint { base: tint(hue, 210), hover: tint(hue, 255), pressed: tint(hue, 170) },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("accent-button"),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb((255, 255, 255)))))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

/// A quiet secondary button: hairline border, transparent until hovered,
/// hue-tinted label.
pub fn accent_ghost(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    label: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgba([255, 255, 255, 26])),
            Interaction::default(),
            HoverTint { base: Color::NONE, hover: tint(hue, 34), pressed: tint(hue, 60) },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("accent-ghost"),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(hue))))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

/// An icon-only button in the accent hue (refresh, close, back…).
pub fn accent_icon_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    icon: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            HoverTint { base: Color::NONE, hover: tint(hue, 40), pressed: tint(hue, 70) },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("accent-icon-button"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, hue, 12.5);
    commands.entity(btn).add_child(ic);
    btn
}

/// A small tinted chip: optional icon + label in the hue. For counts, roles,
/// statuses. (The plain themed `chip` widget lives in `chip.rs`.)
pub fn accent_chip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    icon: Option<&str>,
    label: &str,
) -> Entity {
    let e = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(tint(hue, 34)),
            Name::new("accent-chip"),
        ))
        .id();
    if let Some(icon) = icon {
        let ic = icon_text(commands, &fonts.phosphor, icon, hue, 9.5);
        commands.entity(e).add_child(ic);
    }
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 9.0), TextColor(rgb(hue))))
        .id();
    commands.entity(e).add_child(t);
    e
}

/// An inviting empty state: glowing icon halo, friendly headline, and a
/// pointer at what to do next. Empty screens are invitations, not shrugs.
pub fn empty_state(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    icon: &str,
    title: &str,
    sub: Option<&str>,
) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::top(Val::Px(44.0)),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let halo = commands
        .spawn((
            Node {
                width: Val::Px(56.0),
                height: Val::Px(56.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(18.0)),
                ..default()
            },
            BackgroundColor(tint(hue, 30)),
            BorderColor::all(tint(hue, 64)),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, hue, 26.0);
    commands.entity(halo).add_child(ic);
    let t = commands
        .spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary()))))
        .id();
    let mut kids = vec![halo, t];
    if let Some(s) = sub {
        kids.push(
            commands
                .spawn((Text::new(s.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder()))))
                .id(),
        );
    }
    commands.entity(col).add_children(&kids);
    col
}
