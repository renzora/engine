//! VU meter — a level meter with green/amber/red zones and a peak-hold marker.
//! Self-animates a demo signal so it's lively in the gallery; set
//! [`VuMeter::level`] each frame to drive it from real audio.
//!
//! Vertical by default, horizontal on request — the same mirroring the fader
//! does, and for the same caller (a mixer laid out in rows has height to spare
//! nowhere and width to spare everywhere).

use bevy::prelude::*;

use crate::reactive::Rx;
use crate::reactive::tracked::bind_with;
use crate::theme::*;

const GREEN: (u8, u8, u8) = (90, 200, 110);
const AMBER: (u8, u8, u8) = (225, 180, 70);
const RED: (u8, u8, u8) = (220, 90, 80);
const AMBER_AT: f32 = 0.6;
const RED_AT: f32 = 0.85;

/// Meter thickness across its travel axis, and length along it.
const BAR_T: f32 = 14.0;
const BAR_LEN: f32 = 120.0;

#[derive(Component)]
pub(crate) struct VuMeter {
    pub level: f32,
    peak: f32,
    t: f32,
    auto: bool,
    fill: Entity,
    peak_marker: Entity,
    /// Fill grows left→right rather than bottom→up.
    horizontal: bool,
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
    build_vu(commands, true, false)
}

/// A VU meter you drive by writing [`VuMeter::level`] (no self-animation).
pub fn vu_meter_driven(commands: &mut Commands) -> Entity {
    build_vu(commands, false, false)
}

/// A VU meter whose level is driven (one-way) from `get` each frame — e.g. a
/// channel's `peak_level`. The `VuMeter` is crate-private, so this is the public
/// way to feed it from another crate.
pub fn vu_meter_bound<G>(commands: &mut Commands, get: G) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
{
    let meter = build_vu(commands, false, false);
    bind_level(commands, meter, get)
}

/// [`vu_meter_bound`] lying on its side: the fill runs left→right and the
/// peak-hold marker is a vertical hairline.
pub fn vu_meter_bound_horizontal<G>(commands: &mut Commands, get: G) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
{
    let meter = build_vu(commands, false, true);
    bind_level(commands, meter, get)
}

fn bind_level<G>(commands: &mut Commands, meter: Entity, get: G) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
{
    bind_with(commands, meter, get, |world, e, v: &f32| {
        if let Some(mut vu) = world.get_mut::<VuMeter>(e) {
            vu.level = *v;
        }
    });
    meter
}

fn build_vu(commands: &mut Commands, auto: bool, horizontal: bool) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(if horizontal { BAR_LEN } else { BAR_T }),
                height: Val::Px(if horizontal { BAR_T } else { BAR_LEN }),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            Name::new("vu-meter"),
        ))
        .id();

    // Both the fill and the marker are anchored at the quiet end and sized (or
    // offset) along the travel axis; the other axis is always the full width of
    // the bar.
    let mut fill_node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        bottom: Val::Px(0.0),
        ..default()
    };
    let mut peak_node = fill_node.clone();
    if horizontal {
        fill_node.width = Val::Percent(0.0);
        fill_node.height = Val::Percent(100.0);
        peak_node.left = Val::Percent(0.0);
        peak_node.width = Val::Px(2.0);
        peak_node.height = Val::Percent(100.0);
    } else {
        fill_node.width = Val::Percent(100.0);
        fill_node.height = Val::Percent(0.0);
        peak_node.bottom = Val::Percent(0.0);
        peak_node.width = Val::Percent(100.0);
        peak_node.height = Val::Px(2.0);
    }

    let fill = commands
        .spawn((fill_node, BackgroundColor(rgb(GREEN)), Name::new("vu-fill")))
        .id();
    let peak_marker = commands
        .spawn((
            peak_node,
            BackgroundColor(rgb(text_primary())),
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
        horizontal,
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
        let (fill, marker, peak, horizontal) = (m.fill, m.peak_marker, m.peak, m.horizontal);
        if let Ok(mut n) = nodes.get_mut(fill) {
            if horizontal {
                n.width = Val::Percent(level * 100.0);
            } else {
                n.height = Val::Percent(level * 100.0);
            }
        }
        if let Ok(mut c) = colors.get_mut(fill) {
            c.0 = zone_color(level);
        }
        if let Ok(mut n) = nodes.get_mut(marker) {
            if horizontal {
                n.left = Val::Percent(peak * 100.0);
            } else {
                n.bottom = Val::Percent(peak * 100.0);
            }
        }
        if let Ok(mut c) = colors.get_mut(marker) {
            c.0 = zone_color(peak);
        }
    }
}
