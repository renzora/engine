//! `renzora_ember` UI components — the start of a reusable bevy_ui widget set
//! used by the editor and games. These are plain entity builders + the systems
//! that animate their interaction states. [`build_gallery`] showcases them.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{ui_font, EmberFonts};
use crate::theme::{
    rgb, ACCENT_BLUE, CLOSE_RED, HEADER_BG, PLAY_GREEN, TAB_ACTIVE_BG, TAB_HOVER_BG, TEXT_MUTED,
    TEXT_PRIMARY,
};

/// Registers the widget interaction systems.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (button_interact, toggle_interact));
    }
}

// ── Button ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberButton;

/// A clickable button with hover/press color states.
pub fn button(commands: &mut Commands, font: &Handle<Font>, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Interaction::default(),
            EmberButton,
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("button"),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(b).add_child(t);
    b
}

fn button_interact(
    mut q: Query<(&Interaction, &mut BackgroundColor), (With<EmberButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in &mut q {
        bg.0 = match interaction {
            Interaction::Pressed => rgb(ACCENT_BLUE),
            Interaction::Hovered => rgb((64, 64, 78)),
            Interaction::None => rgb(TAB_ACTIVE_BG),
        };
    }
}

// ── Toggle ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberToggle {
    on: bool,
}

/// An on/off switch — click to flip.
pub fn toggle(commands: &mut Commands, on: bool) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(38.0),
                height: Val::Px(20.0),
                padding: UiRect::all(Val::Px(2.0)),
                align_items: AlignItems::Center,
                justify_content: if on {
                    JustifyContent::FlexEnd
                } else {
                    JustifyContent::FlexStart
                },
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(if on { rgb(ACCENT_BLUE) } else { rgb((60, 60, 70)) }),
            Interaction::default(),
            EmberToggle { on },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("toggle"),
        ))
        .id();
    let knob = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb((240, 240, 245))),
            Name::new("toggle-knob"),
        ))
        .id();
    commands.entity(track).add_child(knob);
    track
}

fn toggle_interact(
    mut q: Query<
        (&Interaction, &mut EmberToggle, &mut BackgroundColor, &mut Node),
        Changed<Interaction>,
    >,
) {
    for (interaction, mut tog, mut bg, mut node) in &mut q {
        if *interaction == Interaction::Pressed {
            tog.on = !tog.on;
            bg.0 = if tog.on {
                rgb(ACCENT_BLUE)
            } else {
                rgb((60, 60, 70))
            };
            node.justify_content = if tog.on {
                JustifyContent::FlexEnd
            } else {
                JustifyContent::FlexStart
            };
        }
    }
}

// ── Slider (visual) ──────────────────────────────────────────────────────────

/// A slider showing `value` in 0..1 (visual for now; drag comes with the
/// interactive widget pass).
pub fn slider(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let track = commands
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(6.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((55, 55, 66))),
            Name::new("slider"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(v * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            Name::new("slider-fill"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(v * 100.0),
                margin: UiRect::left(Val::Px(-6.0)),
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((240, 240, 245))),
            Name::new("slider-thumb"),
        ))
        .id();
    commands.entity(track).add_children(&[fill, thumb]);
    track
}

// ── Swatch + helpers ─────────────────────────────────────────────────────────

/// A small rounded color chip.
pub fn swatch(commands: &mut Commands, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(color)),
            Name::new("swatch"),
        ))
        .id()
}

/// A section heading.
pub fn heading(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(font, 13.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id()
}

/// A horizontal row of widgets with a gap.
pub fn hstack(commands: &mut Commands, gap: f32, children: &[Entity]) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(gap),
                ..default()
            },
            Name::new("hstack"),
        ))
        .id();
    commands.entity(row).add_children(children);
    row
}

// ── Gallery ──────────────────────────────────────────────────────────────────

/// A scrollable showcase of the available ember widgets — the editor's first
/// real bevy_ui panel content.
pub fn build_gallery(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            Name::new("widget-gallery"),
        ))
        .id();

    let title = commands
        .spawn((
            Text::new("renzora_ember — components"),
            ui_font(font, 16.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();

    let h_buttons = heading(commands, font, "Buttons");
    let btns = [
        button(commands, font, "Primary"),
        button(commands, font, "Secondary"),
        button(commands, font, "Cancel"),
    ];
    let buttons = hstack(commands, 8.0, &btns);

    let h_toggle = heading(commands, font, "Toggles");
    let togs = [toggle(commands, true), toggle(commands, false)];
    let toggles = hstack(commands, 10.0, &togs);

    let h_slider = heading(commands, font, "Slider");
    let sld = slider(commands, 0.6);

    let h_swatch = heading(commands, font, "Swatches");
    let chips = [
        swatch(commands, ACCENT_BLUE),
        swatch(commands, PLAY_GREEN),
        swatch(commands, CLOSE_RED),
        swatch(commands, TAB_HOVER_BG),
        swatch(commands, HEADER_BG),
    ];
    let swatches = hstack(commands, 8.0, &chips);

    commands.entity(root).add_children(&[
        title, h_buttons, buttons, h_toggle, toggles, h_slider, sld, h_swatch, swatches,
    ]);
    root
}
