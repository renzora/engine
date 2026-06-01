//! Color picker — a preview swatch driven live by R/G/B sliders.

use bevy::prelude::*;

use crate::theme::rgb;

use super::slider::{slider, EmberSlider};

#[derive(Component)]
pub(crate) struct EmberColorPicker {
    r: Entity,
    g: Entity,
    b: Entity,
    preview: Entity,
}

/// A color picker: a live preview swatch driven by R/G/B sliders.
pub fn color_picker(commands: &mut Commands, color: (u8, u8, u8)) -> Entity {
    let (r0, g0, b0) = color;
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("color-picker"),
        ))
        .id();
    let preview = commands
        .spawn((
            Node {
                width: Val::Px(36.0),
                height: Val::Px(36.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(color)),
            BorderColor::all(rgb((70, 70, 82))),
            Name::new("color-preview"),
        ))
        .id();
    let r = slider(commands, r0 as f32 / 255.0);
    let g = slider(commands, g0 as f32 / 255.0);
    let b = slider(commands, b0 as f32 / 255.0);
    let sliders = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .id();
    commands.entity(sliders).add_children(&[r, g, b]);
    commands.entity(root).add_children(&[preview, sliders]);
    commands
        .entity(root)
        .insert(EmberColorPicker { r, g, b, preview });
    root
}

pub(crate) fn color_picker_sync(
    pickers: Query<&EmberColorPicker>,
    sliders: Query<&EmberSlider>,
    mut bgs: Query<&mut BackgroundColor>,
) {
    for p in &pickers {
        let (Ok(r), Ok(g), Ok(b)) = (sliders.get(p.r), sliders.get(p.g), sliders.get(p.b)) else {
            continue;
        };
        let col = Color::srgb(r.value, g.value, b.value);
        if let Ok(mut bg) = bgs.get_mut(p.preview) {
            if bg.0 != col {
                bg.0 = col;
            }
        }
    }
}
