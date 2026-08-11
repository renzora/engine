//! Fader — a slider that travels along one axis.
//!
//! Vertical by default (the desk fader everyone pictures), horizontal on
//! request. The two are one control with its geometry mirrored rather than two
//! widgets: every dimension below is picked from `horizontal`, so a change to
//! the cap or the track can't land on one orientation and miss the other.

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
    /// Travel along x rather than y — read by [`fader_drag`] to decide which
    /// cursor axis is the value, and by [`fader_apply`] to decide which edge the
    /// fill grows from.
    horizontal: bool,
}

/// Thickness of the fader across its travel axis. The cap centres itself in it.
const COL_W: f32 = 24.0;
/// Default travel length. Callers that want the fader to fill its parent
/// override it — the mixer's strips do, because a longer fader is a finer one.
const LEN: f32 = 120.0;
/// The track/fill bar's thickness, and the inset that centres it in `COL_W`.
const TRACK_T: f32 = 6.0;
const TRACK_INSET: f32 = (COL_W - TRACK_T) / 2.0;

/// A vertical fader (drag to change `value` 0..1).
pub fn fader(commands: &mut Commands, value: f32) -> Entity {
    build_fader(commands, value, false)
}

/// The same fader on its side: travel left→right, cap ribs turned through 90°.
///
/// For layouts where height is the scarce axis rather than width — a mixer drawn
/// as rows can spare 24px of a strip's height but not the 120px a standing
/// fader wants.
pub fn fader_horizontal(commands: &mut Commands, value: f32) -> Entity {
    build_fader(commands, value, true)
}

fn build_fader(commands: &mut Commands, value: f32, horizontal: bool) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let col = commands
        .spawn((
            Node {
                width: Val::Px(if horizontal { LEN } else { COL_W }),
                height: Val::Px(if horizontal { COL_W } else { LEN }),
                position_type: PositionType::Relative,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            crate::cursor_icon::HoverCursor(if horizontal {
                SystemCursorIcon::EwResize
            } else {
                SystemCursorIcon::NsResize
            }),
            Name::new("fader"),
        ))
        .id();

    // Track and fill are the same bar, one full-length and one cut to the value.
    // Both are absolute so the cap can overhang them at either end.
    let mut track_node = Node {
        position_type: PositionType::Absolute,
        border_radius: BorderRadius::all(Val::Px(TRACK_T / 2.0)),
        ..default()
    };
    let mut fill_node = track_node.clone();
    if horizontal {
        track_node.top = Val::Px(TRACK_INSET);
        track_node.left = Val::Px(0.0);
        track_node.width = Val::Percent(100.0);
        track_node.height = Val::Px(TRACK_T);
        fill_node.top = Val::Px(TRACK_INSET);
        fill_node.left = Val::Px(0.0);
        fill_node.width = Val::Percent(v * 100.0);
        fill_node.height = Val::Px(TRACK_T);
    } else {
        track_node.left = Val::Px(TRACK_INSET);
        track_node.width = Val::Px(TRACK_T);
        track_node.height = Val::Percent(100.0);
        fill_node.left = Val::Px(TRACK_INSET);
        fill_node.bottom = Val::Px(0.0);
        fill_node.width = Val::Px(TRACK_T);
        fill_node.height = Val::Percent(v * 100.0);
    }

    let track = commands
        .spawn((
            track_node,
            BackgroundColor(rgb(card_bg())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            fill_node,
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-fill"),
        ))
        .id();
    let thumb = fader_cap(commands, v, horizontal);
    commands.entity(col).add_children(&[track, fill, thumb]);
    commands.entity(col).insert((
        EmberFader {
            fill,
            thumb,
            horizontal,
        },
        Bound::<f32>(v),
    ));
    col
}

/// Cap size along the travel axis. It sits on `<value>%` with a negative margin
/// of half this, so the cap's *centre* — where the index line is — marks the
/// value rather than its trailing edge.
const CAP_LONG: f32 = 28.0;
/// Cap size across the travel axis.
const CAP_SHORT: f32 = 20.0;

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
fn fader_cap(commands: &mut Commands, v: f32, horizontal: bool) -> Entity {
    let mut node = Node {
        position_type: PositionType::Absolute,
        width: Val::Px(if horizontal { CAP_LONG } else { CAP_SHORT }),
        height: Val::Px(if horizontal { CAP_SHORT } else { CAP_LONG }),
        // The ribs run across the travel axis in both orientations, so the cap
        // stacks them along it.
        flex_direction: if horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(3.0)),
        ..default()
    };
    if horizontal {
        node.top = Val::Px((COL_W - CAP_SHORT) / 2.0);
        node.left = Val::Percent(v * 100.0);
        node.margin = UiRect::left(Val::Px(-CAP_LONG / 2.0));
        node.column_gap = Val::Px(4.0);
    } else {
        node.left = Val::Px((COL_W - CAP_SHORT) / 2.0);
        node.bottom = Val::Percent(v * 100.0);
        node.margin = UiRect::bottom(Val::Px(-CAP_LONG / 2.0));
        node.row_gap = Val::Px(4.0);
    }
    let cap = commands
        .spawn((
            node,
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
                    width: Val::Px(if horizontal { 1.0 } else { 10.0 }),
                    height: Val::Px(if horizontal { 10.0 } else { 1.0 }),
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
                width: if horizontal {
                    Val::Px(1.0)
                } else {
                    Val::Percent(100.0)
                },
                height: if horizontal {
                    Val::Percent(100.0)
                } else {
                    Val::Px(1.0)
                },
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
    mut faders: Query<(
        &Interaction,
        &bevy::ui::RelativeCursorPosition,
        &EmberFader,
        &mut Bound<f32>,
    )>,
) {
    for (interaction, rcp, f, mut b) in &mut faders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        // `normalized` is centred on the node (-0.5..0.5), and y grows downward.
        let along = if f.horizontal { n.x + 0.5 } else { 0.5 - n.y };
        let v = along.clamp(0.0, 1.0);
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
            if f.horizontal {
                node.width = Val::Percent(v * 100.0);
            } else {
                node.height = Val::Percent(v * 100.0);
            }
        }
        if let Ok(mut node) = nodes.get_mut(f.thumb) {
            if f.horizontal {
                node.left = Val::Percent(v * 100.0);
            } else {
                node.bottom = Val::Percent(v * 100.0);
            }
        }
    }
}
