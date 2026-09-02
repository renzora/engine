//! The height ruler — a vertical scale that slides in on the right of the
//! viewport while you drag the Zoom button, showing how high the camera is.
//!
//! Dragging to zoom moves the camera through a scene with no other reference to
//! read: the grid fades with distance, and nothing tells you whether you're two
//! metres off the floor or two hundred. The ruler is that reference, in the
//! shape editors have settled on — a fixed strip of ticks whose *labels* scroll
//! past a marked centre line, so the numbers move and the marker stays put.
//!
//! Beside the ticks is a white track showing where the zoom sits between its
//! limits ([`EDITOR_ZOOM_MIN`]..[`EDITOR_ZOOM_MAX`]) — the ticks tell you where
//! you are, the bar tells you how much room is left before the drag stops
//! moving. Without it, hitting the clamp reads as the drag having broken. The
//! marker riding it also **grows taller with altitude**, so height registers
//! peripherally without reading a single number.
//!
//! The scale stops at **0 m**. Ticks that would fall below the ground blank out
//! rather than counting into negatives: the grid plane is the floor everything
//! is built on, and "-8 m" invites the reading that there's something down there
//! to fly to.
//!
//! Only the centre carries a number — your current height, in white. The other
//! ticks are bare dashes: they exist to give the scale a sense of motion as you
//! climb, and nine numbers sliding past at once is harder to read than one that
//! doesn't move.
//!
//! It's deliberately transient: shown while the drag is latched and for a short
//! beat afterwards, then gone. A permanent ruler down the side of the viewport
//! would be one more thing between you and the scene, and the height only
//! matters while you're changing it.

use std::sync::atomic::Ordering;

use bevy::prelude::*;

use renzora::core::viewport_types::{
    CameraOrbitSnapshot, NavOverlayState, EDITOR_ZOOM_MAX, EDITOR_ZOOM_MIN,
};
use renzora::core::EditorCamera;
use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{accent, rgb, text_muted};

/// How many ticks the ruler draws. Odd, so one of them is the centre.
const TICKS: usize = 9;
/// Vertical pitch between ticks, in logical px.
const TICK_GAP: f32 = 26.0;
/// How long the ruler lingers after the drag ends, in seconds.
const LINGER: f32 = 0.6;
/// Width of the zoom-range track, in logical px.
const TRACK_W: f32 = 3.0;
/// Marker length at ground level and at [`EDITOR_ZOOM_MAX`] altitude, in logical
/// px. It stretches between the two, so how high you are is legible from the
/// bar's length alone.
const MARKER_H_MIN: f32 = 4.0;
const MARKER_H_MAX: f32 = 46.0;

/// The ruler's root, hidden until a zoom drag starts.
#[derive(Component)]
pub(crate) struct HeightRuler {
    /// The one label: the live height, on the centre line.
    readout: Entity,
    /// Each tick's dash, top to bottom, hidden below ground.
    marks: Vec<Entity>,
    /// The marker riding the zoom-range track; its length tracks altitude.
    marker: Entity,
}

/// Counts down from [`LINGER`] once the drag ends.
#[derive(Resource, Default)]
struct RulerLinger(f32);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<RulerLinger>();
    app.add_systems(Update, update_height_ruler.run_if(in_state(SplashState::Editor)));
}

/// Build the ruler for a viewport's content node. Absolutely positioned on the
/// left edge, centred vertically.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Right edge, clear of the nav buttons and the axis gizmo above
                // them; vertically centred so the marked line sits at eye level.
                right: Val::Px(14.0),
                top: Val::Percent(50.0),
                margin: UiRect::top(Val::Px(-(TICKS as f32 * TICK_GAP) / 2.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                column_gap: Val::Px(6.0),
                display: Display::None,
                ..default()
            },
            // Pointer-transparent: it's a readout, and it sits right where you'd
            // be dragging.
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-height-ruler"),
        ))
        .id();

    // The ticks live in their own column so the track can sit beside them.
    let scale = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            Name::new("vp-height-scale"),
        ))
        .id();

    let mut readout: Option<Entity> = None;
    let mut marks = Vec::with_capacity(TICKS);
    let centre = TICKS / 2;
    for i in 0..TICKS {
        let row = commands
            .spawn((
                Node {
                    height: Val::Px(TICK_GAP),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                Name::new("vp-height-tick"),
            ))
            .id();
        // The centre tick is the one the camera is actually at, so it's longer
        // and brighter — that's the line you read against.
        let is_centre = i == centre;
        let mark = commands
            .spawn((
                Node {
                    width: Val::Px(if is_centre { 16.0 } else { 8.0 }),
                    height: Val::Px(if is_centre { 2.0 } else { 1.0 }),
                    ..default()
                },
                BackgroundColor(if is_centre {
                    rgb(accent())
                } else {
                    rgb(text_muted()).with_alpha(0.6)
                }),
            ))
            .id();
        // Only the centre row carries a number, and it *is* the readout.
        if is_centre {
            let label = commands
                .spawn((
                    Text::new(""),
                    ui_font(&fonts.ui, 12.0),
                    TextColor(Color::WHITE),
                ))
                .id();
            readout = Some(label);
            // Label first, dash second: the scale reads against the track on
            // the right, so the marks are the rightmost thing in each row.
            commands.entity(row).add_children(&[label, mark]);
        } else {
            commands.entity(row).add_child(mark);
        }
        commands.entity(scale).add_child(row);
        marks.push(mark);
    }
    let readout = readout.expect("TICKS is odd, so there is always a centre row");

    // The zoom-range track: a white bar spanning the ruler, with a marker
    // riding it. Top is fully zoomed out, bottom fully in — matching the ticks,
    // where up is further away.
    let track = commands
        .spawn((
            Node {
                width: Val::Px(TRACK_W),
                height: Val::Px(TICKS as f32 * TICK_GAP),
                position_type: PositionType::Relative,
                border_radius: BorderRadius::all(Val::Px(TRACK_W / 2.0)),
                ..default()
            },
            BackgroundColor(Color::WHITE.with_alpha(0.22)),
            Name::new("vp-height-track"),
        ))
        .id();
    let marker = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-2.0),
                right: Val::Px(-2.0),
                top: Val::Px(0.0),
                height: Val::Px(MARKER_H_MIN),
                border_radius: BorderRadius::all(Val::Px(TRACK_W)),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            Name::new("vp-height-marker"),
        ))
        .id();
    commands.entity(track).add_child(marker);

    commands.entity(root).add_children(&[scale, track]);

    commands.entity(root).insert(HeightRuler {
        readout,
        marks,
        marker,
    });
    root
}

/// Show the ruler while the zoom drag is latched (plus a short linger), and
/// relabel its ticks from the camera's height.
fn update_height_ruler(
    time: Res<Time>,
    nav: Res<NavOverlayState>,
    orbit: Option<Res<CameraOrbitSnapshot>>,
    mut linger: ResMut<RulerLinger>,
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    rulers: Query<(Entity, &HeightRuler)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    if rulers.is_empty() {
        return;
    }
    if nav.zoom_dragging.load(Ordering::Relaxed) {
        linger.0 = LINGER;
    } else if linger.0 > 0.0 {
        linger.0 = (linger.0 - time.delta_secs()).max(0.0);
    }
    let show = linger.0 > 0.0;

    for (root, ruler) in &rulers {
        if let Ok(mut node) = nodes.get_mut(root) {
            let want = if show { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
        if !show {
            continue;
        }
        let Ok(cam) = camera.single() else { continue };
        // The ruler is a height-above-ground readout, so it bottoms out at the
        // floor: a camera dropped below y=0 reads 0 m rather than counting down.
        let height = cam.translation().y.max(0.0);
        // Pick a step that keeps the labels readable at any altitude: the ruler
        // spans roughly the camera's own height, so it stays useful whether
        // you're a metre up or a kilometre.
        let step = nice_step(height.abs().max(1.0) / (TICKS as f32 * 0.5));
        let centre = (TICKS / 2) as i32;
        for (i, mark) in ruler.marks.iter().enumerate() {
            // Top of the ruler is the *highest* value, so ticks count down, and
            // any that would fall below the floor hide — the scale ends at 0 m
            // without the centre line drifting off-centre.
            let value = height + (centre - i as i32) as f32 * step;
            if let Ok(mut node) = nodes.get_mut(*mark) {
                let want = if value < 0.0 { Display::None } else { Display::Flex };
                if node.display != want {
                    node.display = want;
                }
            }
        }
        if let Ok(mut text) = texts.get_mut(ruler.readout) {
            let next = format_height(height, step);
            if text.0 != next {
                text.0 = next;
            }
        }
        // Marker: fully zoomed OUT sits at the top, fully in at the bottom, so
        // it travels the same way the tick numbers do.
        let distance = orbit
            .as_ref()
            .map(|o| o.distance)
            .unwrap_or(EDITOR_ZOOM_MIN)
            .clamp(EDITOR_ZOOM_MIN, EDITOR_ZOOM_MAX);
        let span = (EDITOR_ZOOM_MAX - EDITOR_ZOOM_MIN).max(f32::EPSILON);
        let t = 1.0 - (distance - EDITOR_ZOOM_MIN) / span;
        // Marker length from altitude, not from the zoom: they usually move
        // together, but a camera looking along the ground can pull a long way
        // back without climbing, and it's the height the bar is reporting.
        let h = (height.max(0.0) / EDITOR_ZOOM_MAX).clamp(0.0, 1.0);
        let marker_h = MARKER_H_MIN + (MARKER_H_MAX - MARKER_H_MIN) * h;
        // Travel is shortened by the marker's own length, so a long marker at
        // the bottom of the range doesn't hang off the end of the track.
        let travel = (TICKS as f32 * TICK_GAP - marker_h).max(0.0);
        if let Ok(mut node) = nodes.get_mut(ruler.marker) {
            let want_top = Val::Px(t * travel);
            if node.top != want_top {
                node.top = want_top;
            }
            let want_h = Val::Px(marker_h);
            if node.height != want_h {
                node.height = want_h;
            }
        }
    }
}

/// Round a raw spacing up to the nearest 1/2/5×10ⁿ, so tick labels land on
/// numbers a person would have chosen.
fn nice_step(raw: f32) -> f32 {
    let mag = 10f32.powf(raw.max(0.001).log10().floor());
    let norm = raw / mag;
    let step = if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * mag
}

/// One decimal while the step is fine enough for it to mean anything, whole
/// numbers once the ruler is spanning tens of units. World units are metres, and
/// the suffix says so — a bare number beside a scene is ambiguous.
fn format_height(value: f32, step: f32) -> String {
    if step < 1.0 {
        format!("{value:.1} m")
    } else {
        format!("{:.0} m", value.round())
    }
}
