//! Fader — a vertical slider.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::reactive::Bound;
use crate::theme::*;

/// References to the fader's fill/thumb so the model→visuals system can move
/// them. The value itself lives in `Bound<f32>` (so [`bind_2way`] can drive it).
#[derive(Component)]
pub(crate) struct EmberFader {
    fill: Entity,
    thumb: Entity,
}

/// Width of the fader's column. The cap centres itself in it.
const COL_W: f32 = 24.0;

/// A vertical fader (drag to change `value` 0..1).
pub fn fader(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let col = commands
        .spawn((
            Node {
                width: Val::Px(COL_W),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("fader"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0),
                width: Val::Px(6.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0),
                bottom: Val::Px(0.0),
                width: Val::Px(6.0),
                height: Val::Percent(v * 100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-fill"),
        ))
        .id();
    let thumb = fader_cap(commands, v);
    commands.entity(col).add_children(&[track, fill, thumb]);
    commands
        .entity(col)
        .insert((EmberFader { fill, thumb }, Bound::<f32>(v)));
    col
}

/// Height of the fader cap. It sits on `bottom: <value>%` with a negative
/// bottom margin of half this, so the cap's *centre* — where the index line is —
/// marks the value rather than its lower edge.
const CAP_H: f32 = 28.0;
const CAP_W: f32 = 20.0;

/// The grip: a cap with ribbing and a centre index line, rather than the flat
/// 18×10 lozenge this used to be.
///
/// The lozenge was the wrong shape for the job in two ways. It was wider than it
/// was tall, so nothing about it said "slide me up and down"; and with no mark on
/// it there was no answer to "which pixel of this thing is the value?" — on a
/// 10px block you can guess, but the taller a fader gets (and this one now
/// stretches to fill the strip) the more that guess costs. The ribs are what a
/// real cap has under the thumb, and the index line in the fill's own colour is
/// the one pixel that reads the scale.
fn fader_cap(commands: &mut Commands, v: f32) -> Entity {
    let cap = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((COL_W - CAP_W) / 2.0),
                bottom: Val::Percent(v * 100.0),
                margin: UiRect::bottom(Val::Px(-CAP_H / 2.0)),
                width: Val::Px(CAP_W),
                height: Val::Px(CAP_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            BorderColor::all(rgb(border())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-thumb"),
        ))
        .id();

    // Two ribs, the index line, two ribs — symmetrical about the centre so the
    // index line lands exactly on the value however the cap is laid out.
    let rib = |commands: &mut Commands| {
        commands
            .spawn((
                Node {
                    width: Val::Px(10.0),
                    height: Val::Px(1.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(rgb(placeholder())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("fader-thumb-rib"),
            ))
            .id()
    };
    let index = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                flex_shrink: 0.0,
                ..default()
            },
            // The fill's colour: cap and fill are reading the same number, so
            // they should be saying it in the same voice.
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-thumb-index"),
        ))
        .id();
    let parts = [rib(commands), rib(commands), index, rib(commands), rib(commands)];
    commands.entity(cap).add_children(&parts);
    cap
}

/// User drag → write the model (`Bound<f32>`). Visuals follow via [`fader_apply`].
pub(crate) fn fader_drag(
    mut faders: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut Bound<f32>), With<EmberFader>>,
) {
    for (interaction, rcp, mut b) in &mut faders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - b.0).abs() >= 0.001 {
            b.0 = v;
        }
    }
}

/// Model (`Bound<f32>`) → fill/thumb position. Runs whenever the model changes,
/// whether the user dragged or [`bind_2way`] pushed a new value from state.
pub(crate) fn fader_apply(
    faders: Query<(&EmberFader, &Bound<f32>), Changed<Bound<f32>>>,
    mut nodes: Query<&mut Node>,
) {
    for (f, b) in &faders {
        let v = b.0.clamp(0.0, 1.0);
        if let Ok(mut node) = nodes.get_mut(f.fill) {
            node.height = Val::Percent(v * 100.0);
        }
        if let Ok(mut node) = nodes.get_mut(f.thumb) {
            node.bottom = Val::Percent(v * 100.0);
        }
    }
}
