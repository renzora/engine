//! Shared layout/text helpers used across the ember widgets.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::{rgb, TEXT_MUTED};

/// A `Text` entity in the UI font at `size` and `color`.
pub(crate) fn text_node(
    commands: &mut Commands,
    font: &Handle<Font>,
    text: &str,
    size: f32,
    color: (u8, u8, u8),
) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(font, size), TextColor(rgb(color))))
        .id()
}

/// Format a number compactly (integers without a trailing decimal).
pub(crate) fn format_num(v: f32) -> String {
    if v.fract().abs() < 0.001 {
        format!("{}", v as i32)
    } else {
        format!("{v:.1}")
    }
}

/// A labeled row: a fixed-width label on the left, a control on the right.
pub fn field(commands: &mut Commands, font: &Handle<Font>, label: &str, control: Entity) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                ..default()
            },
            Name::new("field"),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                min_width: Val::Px(90.0),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[lbl, control]);
    row
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
