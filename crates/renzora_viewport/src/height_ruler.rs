//! The height ruler — a vertical scale that slides in at the **bottom left** of
//! the viewport while you drag the Zoom button, showing how high the camera is.
//!
//! Dragging to zoom moves the camera through a scene with no other reference to
//! read: the grid fades with distance, and nothing tells you whether you're two
//! metres off the floor or two hundred. The ruler is that reference, in the
//! shape editors have settled on — a fixed strip of ticks whose *labels* scroll
//! past a marked centre line, so the numbers move and the marker stays put.
//!
//! Beside the ticks is a white track that **fills upwards from the floor** as
//! the camera climbs, out of [`EDITOR_ZOOM_MAX`] at the top. A bar that fills
//! is read the way a tank gauge is — level, without a number and without
//! finding a marker first — where the blob that used to ride this track had to
//! be located before it could be read, and reported the *zoom distance* rather
//! than the height the ticks beside it were counting.
//!
//! It hangs in the corner it does because that is where a gauge belongs, and
//! because it is out of the way of the drag: the Zoom button is on the right
//! edge, and a readout under the cursor is a readout with a hand over it. It
//! stacks above the [statistics readout](super::stats_overlay) when that is on,
//! measured rather than assumed — the stats block grows a row when the scene has
//! a terrain.
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

use bevy::ui::ComputedNode;

use renzora::core::viewport_types::{NavOverlayState, EDITOR_ZOOM_MAX};
use renzora::core::EditorCamera;
use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{accent, rgb, text_muted};

use crate::stats_overlay::StatsOverlayRoot;

/// How many ticks the ruler draws. Odd, so one of them is the centre.
const TICKS: usize = 9;
/// Vertical pitch between ticks, in logical px.
const TICK_GAP: f32 = 26.0;
/// How long the ruler lingers after the drag ends, in seconds.
const LINGER: f32 = 0.6;
/// Width of the altitude track, in logical px.
const TRACK_W: f32 = 3.0;
/// Shortest the fill ever draws, in logical px. At ground level the true height
/// is zero, and a bar of nothing reads as a broken widget rather than as the
/// floor.
const FILL_H_MIN: f32 = 4.0;
/// Inset from the viewport's left edge and from whatever it stacks above.
const MARGIN: f32 = 8.0;

/// The ruler's root, hidden until a zoom drag starts.
#[derive(Component)]
pub(crate) struct HeightRuler {
    /// The one label: the live height, on the centre line.
    readout: Entity,
    /// Each tick's dash, top to bottom, hidden below ground.
    marks: Vec<Entity>,
    /// The bar that fills the track from the floor up, by altitude.
    fill: Entity,
}

/// Counts down from [`LINGER`] once the drag ends.
#[derive(Resource, Default)]
struct RulerLinger(f32);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<RulerLinger>();
    app.add_systems(Update, update_height_ruler.run_if(in_state(SplashState::Editor)));
}

/// Build the ruler for a viewport's content node. Absolutely positioned in the
/// bottom-left corner; `update_height_ruler` lifts it above the statistics
/// readout when that is on screen.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(MARGIN),
                bottom: Val::Px(MARGIN),
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
    // Left-aligned, and the track goes *first*: the ruler hangs off the
    // viewport's left edge now, so the scale reads outwards from the bar the
    // way it read inwards from it on the right.
    let scale = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
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
                    justify_content: JustifyContent::FlexStart,
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
            // Dash first, label second: the scale reads against the track on
            // its left, so the marks are the leftmost thing in each row.
            commands.entity(row).add_children(&[mark, label]);
        } else {
            commands.entity(row).add_child(mark);
        }
        commands.entity(scale).add_child(row);
        marks.push(mark);
    }
    let readout = readout.expect("TICKS is odd, so there is always a centre row");

    // The altitude track: an empty channel spanning the ruler, filled from the
    // bottom up. The floor is at the bottom, matching the ticks, where up is
    // further from the ground.
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
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-2.0),
                right: Val::Px(-2.0),
                // Anchored to the floor, so growing the height grows it upwards
                // and the bar's top edge is the reading.
                bottom: Val::Px(0.0),
                height: Val::Px(FILL_H_MIN),
                border_radius: BorderRadius::all(Val::Px(TRACK_W)),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            Name::new("vp-height-fill"),
        ))
        .id();
    commands.entity(track).add_child(fill);

    commands.entity(root).add_children(&[track, scale]);

    commands.entity(root).insert(HeightRuler {
        readout,
        marks,
        fill,
    });
    root
}

/// Show the ruler while the zoom drag is latched (plus a short linger), and
/// relabel its ticks from the camera's height.
fn update_height_ruler(
    time: Res<Time>,
    nav: Res<NavOverlayState>,
    mut linger: ResMut<RulerLinger>,
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    rulers: Query<(Entity, &HeightRuler)>,
    // `Entity` + `ComputedNode`, and the display read through `nodes` below: a
    // `&Node` here would alias the `&mut Node` this system already takes, which
    // bevy refuses at startup rather than at the call site.
    stats: Query<(Entity, &ComputedNode), With<StatsOverlayRoot>>,
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

    // Both live in this corner, so the ruler sits on top of whatever the stats
    // block currently measures. Measured, not assumed: it grows a row when the
    // scene has a terrain, and it is hidden entirely by a Settings toggle and by
    // play mode.
    let stats_h = stats
        .iter()
        .filter(|(e, _)| {
            nodes
                .get(*e)
                .map(|n| n.display != Display::None)
                .unwrap_or(false)
        })
        .map(|(_, cn)| cn.size().y * cn.inverse_scale_factor())
        .fold(0.0f32, f32::max);
    let want_bottom = Val::Px(if stats_h > 0.0 { MARGIN * 2.0 + stats_h } else { MARGIN });

    for (root, ruler) in &rulers {
        if let Ok(mut node) = nodes.get_mut(root) {
            let want = if show { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
            if node.bottom != want_bottom {
                node.bottom = want_bottom;
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
        // Fill from altitude, not from the zoom distance: the two usually move
        // together, but a camera looking along the ground can pull a long way
        // back without climbing, and height is what the ticks beside this bar
        // are counting.
        let h = (height / EDITOR_ZOOM_MAX).clamp(0.0, 1.0);
        let fill_h = (h * TICKS as f32 * TICK_GAP).max(FILL_H_MIN);
        if let Ok(mut node) = nodes.get_mut(ruler.fill) {
            let want_h = Val::Px(fill_h);
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
