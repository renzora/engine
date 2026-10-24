//! Spin-slider — a Godot-style inline value field: a label + value with a fill
//! bar showing the value in `[min, max]`; drag horizontally to scrub.
//!
//! The value lives in `Bound<f32>`, as a fader's does, so `bind_2way` drives
//! it both ways: a drag writes the model, and [`spin_apply`] draws whatever the
//! model holds, a drag or a state change alike. It used to keep the value in a
//! private field, which a binding could not see, so every bound spin-slider
//! moved its bar and changed nothing else.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::interaction::PASS_THROUGH;
use crate::reactive::Bound;
use crate::style::{Role, Styled};
use crate::theme::*;
use crate::UiInteraction;

use super::common::format_num;

#[derive(Component)]
pub(crate) struct EmberSpin {
    min: f32,
    max: f32,
    step: f32,
    fill: Entity,
    text: Entity,
    last_x: Option<f32>,
}

fn frac(value: f32, min: f32, max: f32) -> f32 {
    ((value - min) / (max - min).max(1e-4)).clamp(0.0, 1.0)
}

/// A spin-slider showing `value` in `[min, max]`. Drag to scrub by `step`/px.
pub fn spin_slider(commands: &mut Commands, font: &bevy::text::FontSource, label: &str, value: f32, min: f32, max: f32) -> Entity {
    let row = commands
        .spawn((
            Node {
                min_width: Val::Px(150.0),
                height: Val::Px(22.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Styled::new(Role::Input),
            UiInteraction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::EwResize),
            Name::new("spin-slider"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                height: Val::Percent(100.0),
                width: Val::Percent(frac(value, min, max) * 100.0),
                ..default()
            },
            BackgroundColor(rgb(accent()).with_alpha(0.30)),
            PASS_THROUGH,
            Name::new("spin-fill"),
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(8.0)),
                ..default()
            },
            PASS_THROUGH,
            Name::new("spin-content"),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(font, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(format_num(value)),
            ui_font(font, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(content).add_children(&[lbl, text]);
    let step = (max - min) / 200.0;
    commands.entity(row).insert((Bound(value), EmberSpin {
        min,
        max,
        step: if step > 0.0 { step } else { 0.01 },
        fill,
        text,
        last_x: None,
    }));
    commands.entity(row).add_children(&[fill, content]);
    row
}

/// Drag → the model. The visuals follow in [`spin_apply`].
pub(crate) fn spin_drag(
    windows: Query<&Window>,
    mut spins: Query<(&UiInteraction, &mut EmberSpin, &mut Bound<f32>)>,
) {
    let cursor_x = windows.iter().find_map(|w| w.cursor_position())
        .map(|p| p.x);
    for (interaction, mut s, mut bound) in &mut spins {
        if *interaction == UiInteraction::Pressed {
            if let (Some(cx), Some(last)) = (cursor_x, s.last_x) {
                let d = cx - last;
                if d != 0.0 {
                    let v = (bound.0 + d * s.step).clamp(s.min, s.max);
                    if v != bound.0 {
                        bound.0 = v;
                    }
                }
            }
            s.last_x = cursor_x;
        } else if s.last_x.is_some() {
            s.last_x = None;
        }
    }
}

/// The model → the fill and the number, for a drag and for a change made to
/// the bound state elsewhere (a reset button, an undo, a scrub).
pub(crate) fn spin_apply(
    spins: Query<(&EmberSpin, &Bound<f32>), Changed<Bound<f32>>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (s, bound) in &spins {
        let v = bound.0.clamp(s.min, s.max);
        if let Ok(mut n) = nodes.get_mut(s.fill) {
            n.width = Val::Percent(frac(v, s.min, s.max) * 100.0);
        }
        if let Ok(mut t) = texts.get_mut(s.text) {
            *t = Text::new(format_num(v));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn spin(world: &mut World, value: f32) -> (Entity, Entity, Entity) {
        let fill = world.spawn(Node::default()).id();
        let text = world.spawn(Text::new(format_num(value))).id();
        let row = world
            .spawn((
                UiInteraction::Pressed,
                Bound::<f32>(value),
                EmberSpin { min: 0.0, max: 1.0, step: 0.005, fill, text, last_x: None },
            ))
            .id();
        (row, fill, text)
    }

    /// A drag has to reach the model, or `bind_2way` never hears of it: the
    /// bug this widget had, where only the bar moved.
    #[test]
    fn drag_writes_the_model() {
        let mut world = World::new();
        let (row, ..) = spin(&mut world, 0.0);
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(10.0, 10.0)));
        let w = world.spawn(window).id();
        world.run_system_once(spin_drag).unwrap();
        world.get_mut::<Window>(w).unwrap().set_cursor_position(Some(Vec2::new(110.0, 10.0)));
        world.run_system_once(spin_drag).unwrap();
        assert!((world.get::<Bound<f32>>(row).unwrap().0 - 0.5).abs() < 1e-5);
    }

    /// A change made to the model from outside (a reset button) shows.
    #[test]
    fn model_change_redraws() {
        let mut world = World::new();
        let (row, fill, text) = spin(&mut world, 0.5);
        world.get_mut::<Bound<f32>>(row).unwrap().0 = 0.25;
        world.run_system_once(spin_apply).unwrap();
        assert_eq!(world.get::<Text>(text).unwrap().0, format_num(0.25));
        assert_eq!(world.get::<Node>(fill).unwrap().width, Val::Percent(25.0));
    }
}
