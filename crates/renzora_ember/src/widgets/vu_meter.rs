//! VU meter — a vertical level meter with green/amber/red zones and a peak-hold
//! marker. Self-animates a demo signal so it's lively in the gallery; set
//! [`VuMeter::level`] each frame to drive it from real audio.

use bevy::prelude::*;

use crate::theme::rgb;

const GREEN: (u8, u8, u8) = (90, 200, 110);
const AMBER: (u8, u8, u8) = (225, 180, 70);
const RED: (u8, u8, u8) = (225, 90, 80);
const AMBER_AT: f32 = 0.6;
const RED_AT: f32 = 0.85;

#[derive(Component)]
pub(crate) struct VuMeter {
    pub level: f32,
    peak: f32,
    t: f32,
    auto: bool,
    fill: Entity,
    peak_marker: Entity,
}

fn zone_color(level: f32) -> Color {
    if level >= RED_AT {
        rgb(RED)
    } else if level >= AMBER_AT {
        rgb(AMBER)
    } else {
        rgb(GREEN)
    }
}

/// A self-animating VU meter (demo signal). Use [`vu_meter_driven`] to feed levels.
pub fn vu_meter(commands: &mut Commands) -> Entity {
    build_vu(commands, true)
}

/// A VU meter you drive by writing [`VuMeter::level`] (no self-animation).
pub fn vu_meter_driven(commands: &mut Commands) -> Entity {
    build_vu(commands, false)
}

fn build_vu(commands: &mut Commands, auto: bool) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(14.0),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((20, 20, 26))),
            BorderColor::all(rgb((48, 48, 58))),
            Name::new("vu-meter"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(0.0),
                ..default()
            },
            BackgroundColor(rgb(GREEN)),
            Name::new("vu-fill"),
        ))
        .id();
    let peak_marker = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Percent(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(rgb((235, 235, 245))),
            Name::new("vu-peak"),
        ))
        .id();
    commands.entity(track).add_children(&[fill, peak_marker]);
    commands.entity(track).insert(VuMeter {
        level: 0.0,
        peak: 0.0,
        t: 0.0,
        auto,
        fill,
        peak_marker,
    });
    track
}

pub(crate) fn vu_animate(
    time: Res<Time>,
    mut meters: Query<&mut VuMeter>,
    mut nodes: Query<&mut Node>,
    mut colors: Query<&mut BackgroundColor>,
) {
    let dt = time.delta_secs();
    for mut m in &mut meters {
        m.t += dt;
        if m.auto {
            // A faux signal from layered sines — lively but deterministic.
            let s = (m.t * 6.3).sin() * 0.5 + 0.5;
            let s2 = (m.t * 2.1 + 1.0).sin() * 0.5 + 0.5;
            m.level = (s * 0.7 + s2 * 0.3).clamp(0.0, 1.0);
        }
        let level = m.level.clamp(0.0, 1.0);
        // Peak holds, then decays.
        if level > m.peak {
            m.peak = level;
        } else {
            m.peak = (m.peak - dt * 0.4).max(level);
        }
        let (fill, marker, peak) = (m.fill, m.peak_marker, m.peak);
        if let Ok(mut n) = nodes.get_mut(fill) {
            n.height = Val::Percent(level * 100.0);
        }
        if let Ok(mut c) = colors.get_mut(fill) {
            c.0 = zone_color(level);
        }
        if let Ok(mut n) = nodes.get_mut(marker) {
            n.bottom = Val::Percent(peak * 100.0);
        }
        if let Ok(mut c) = colors.get_mut(marker) {
            c.0 = zone_color(peak);
        }
    }
}
