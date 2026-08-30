//! Progress — determinate and indeterminate progress bars.

use bevy::prelude::*;

use crate::theme::*;

/// A determinate progress bar (`value` 0..1).
pub fn progress(commands: &mut Commands, value: f32) -> Entity {
    progress_sized(commands, value, 180.0, 8.0)
}

/// A determinate progress bar at an explicit size, for callers that aren't a
/// panel body — a status-bar segment wants something far smaller than the
/// 180×8 default.
pub fn progress_sized(commands: &mut Commands, value: f32, width: f32, height: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let track = track_node(commands, width, height);
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(v * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(height * 0.5)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Name::new("progress-fill"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    track
}

/// Marks the sliding block of an indeterminate bar, animated by
/// [`progress_indeterminate_anim`].
#[derive(Component)]
pub(crate) struct EmberIndeterminateFill;

/// A bar for work whose total isn't known: a short block that sweeps the track.
///
/// The animation lives in its own system rather than in whatever rebuilds the
/// bar, so a caller can spawn this once and leave it. That matters most where
/// this is most useful — the status bar reconciles its rows by content hash, so
/// a bar that animated by respawning would despawn and respawn a row every
/// frame for the whole of a download.
pub fn progress_indeterminate(commands: &mut Commands, width: f32, height: f32) -> Entity {
    let track = track_node(commands, width, height);
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(0.0),
                width: Val::Percent(SWEEP_WIDTH * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(height * 0.5)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            EmberIndeterminateFill,
            Name::new("progress-indeterminate-fill"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    track
}

/// How much of the track the sweeping block covers.
const SWEEP_WIDTH: f32 = 0.35;
/// Seconds for one there-and-back sweep.
const SWEEP_PERIOD: f32 = 1.6;

pub(crate) fn progress_indeterminate_anim(
    time: Res<Time>,
    mut fills: Query<&mut Node, With<EmberIndeterminateFill>>,
) {
    if fills.is_empty() {
        return;
    }
    // Ping-pong rather than wrapping: a block that vanishes off one edge and
    // reappears at the other reads as two blocks at this size, where the whole
    // sweep is a couple of centimetres.
    let phase = (time.elapsed_secs() / SWEEP_PERIOD).fract();
    let t = 1.0 - (phase * 2.0 - 1.0).abs();
    // Eased, so it settles at each end instead of bouncing off it.
    let eased = t * t * (3.0 - 2.0 * t);
    let left = eased * (1.0 - SWEEP_WIDTH) * 100.0;
    for mut node in &mut fills {
        node.left = Val::Percent(left);
    }
}

fn track_node(commands: &mut Commands, width: f32, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
                border_radius: BorderRadius::all(Val::Px(height * 0.5)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Name::new("progress"),
        ))
        .id()
}
