//! Canvas scrollbars — the visible half of panning.
//!
//! Panning already worked (middle/right-drag, modifier+wheel), but nothing said
//! *where* you were: zoom into a 1280×720 template inside a 500px panel and the
//! only way to find the part you were not looking at was to drag and see. A
//! scrollbar is the readout as much as the control — its thumb says how much of
//! the design you can currently see and whereabouts that is.
//!
//! Both bars derive from the same numbers the canvas already keeps:
//! `content = reference × zoom`, `viewport = the area's own size`, and
//! `NativeCanvasState::pan` as the offset. There is no second scroll state to
//! keep in step — dragging a thumb writes `pan`, and `nav::apply_view` puts it
//! on the frame exactly as a middle-drag would.
//!
//! Sign: `pan` is the frame's offset from centred, so a *positive* pan moves the
//! content right, which means the thumb moves *left*. Getting that backwards
//! gives a scrollbar that fights the drag, which is worse than none.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_ember::theme::*;

use crate::game_ui::ruler::RulerArea;
use crate::game_ui::NativeCanvasState;

/// Thickness of each bar.
const BAR: f32 = 9.0;
/// Shortest a thumb may get, so it stays grabbable at deep zoom.
const MIN_THUMB: f32 = 24.0;

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) struct ScrollBar {
    horizontal: bool,
}

#[derive(Component, Clone, Copy)]
struct ScrollThumb {
    horizontal: bool,
}

/// A thumb being dragged: which axis, and the pan/cursor it started from.
#[derive(Resource, Default)]
struct ThumbDrag(Option<(bool, f32, f32)>);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ThumbDrag>();
    app.add_systems(
        Update,
        (position_scrollbars, drag_thumbs)
            .after(crate::game_ui::geometry::snapshot_widgets)
            .run_if(in_state(renzora::SplashState::Editor)),
    );
}

/// Build both bars. Returns the container, to be added to the canvas area.
pub(crate) fn build(commands: &mut Commands) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
            Name::new("ui-canvas-scrollbars"),
        ))
        .id();

    let mut bars = Vec::new();
    for horizontal in [true, false] {
        let track = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    // Along the bottom / right, opposite the rulers.
                    left: if horizontal { Val::Px(0.0) } else { Val::Auto },
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    top: if horizontal { Val::Auto } else { Val::Px(0.0) },
                    width: if horizontal { Val::Auto } else { Val::Px(BAR) },
                    height: if horizontal { Val::Px(BAR) } else { Val::Auto },
                    ..default()
                },
                BackgroundColor(rgb(panel_bg()).with_alpha(0.55)),
                Visibility::Hidden,
                ScrollBar { horizontal },
                RelativeCursorPosition::default(),
                Interaction::default(),
                Name::new(if horizontal { "canvas-hscroll" } else { "canvas-vscroll" }),
            ))
            .id();
        let thumb = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    border_radius: BorderRadius::all(Val::Px(BAR * 0.5)),
                    ..default()
                },
                BackgroundColor(rgb(text_muted()).with_alpha(0.45)),
                Interaction::default(),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Grab),
                ScrollThumb { horizontal },
                Name::new("canvas-scroll-thumb"),
            ))
            .id();
        commands.entity(track).add_child(thumb);
        bars.push(track);
    }
    commands.entity(root).add_children(&bars);
    root
}

/// Content extent and viewport extent on one axis, in panel pixels.
fn extents(state: &NativeCanvasState, area: Vec2, horizontal: bool) -> (f32, f32) {
    let content = if horizontal {
        state.canvas_width * state.zoom
    } else {
        state.canvas_height * state.zoom
    };
    let viewport = if horizontal { area.x } else { area.y };
    (content, viewport)
}

/// Size and place each thumb, and hide a bar whose axis fits.
fn position_scrollbars(
    state: Res<NativeCanvasState>,
    area: Query<&ComputedNode, With<RulerArea>>,
    mut bars: Query<(&ScrollBar, &ComputedNode, &mut Visibility)>,
    mut thumbs: Query<(&ScrollThumb, &mut Node)>,
) {
    let Ok(area_node) = area.single() else { return };
    let inv = area_node.inverse_scale_factor();
    let area_size = area_node.size() * inv;
    let has_canvas = state.active_canvas.is_some();

    for (bar, bar_node, mut vis) in &mut bars {
        let (content, viewport) = extents(&state, area_size, bar.horizontal);
        // A bar for an axis that already fits is noise — and worse, a thumb
        // filling its whole track invites a drag that cannot do anything.
        let needed = has_canvas && content > viewport + 1.0;
        let want = if needed { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if !needed {
            continue;
        }
        let track = if bar.horizontal {
            bar_node.size().x * inv
        } else {
            bar_node.size().y * inv
        };
        let frac = (viewport / content).clamp(0.0, 1.0);
        let thumb_len = (track * frac).max(MIN_THUMB).min(track);
        // Pan is the frame's offset from centred; at the extremes it is
        // ±(content − viewport)/2, so shift into 0..1 before scaling.
        let span = (content - viewport).max(1.0);
        let pan = if bar.horizontal { state.pan.x } else { state.pan.y };
        let t = (0.5 - pan / span).clamp(0.0, 1.0);
        let offset = (track - thumb_len) * t;
        for (thumb, mut node) in &mut thumbs {
            if thumb.horizontal != bar.horizontal {
                continue;
            }
            if bar.horizontal {
                node.left = Val::Px(offset);
                node.top = Val::Px(1.0);
                node.width = Val::Px(thumb_len);
                node.height = Val::Px(BAR - 2.0);
            } else {
                node.left = Val::Px(1.0);
                node.top = Val::Px(offset);
                node.width = Val::Px(BAR - 2.0);
                node.height = Val::Px(thumb_len);
            }
        }
    }
}

/// Drag a thumb to pan.
///
/// The cursor is read from the *track's* `RelativeCursorPosition` rather than
/// the window, so the mapping is in track space and needs no knowledge of where
/// the panel sits on screen.
fn drag_thumbs(
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<ThumbDrag>,
    mut state: ResMut<NativeCanvasState>,
    area: Query<&ComputedNode, With<RulerArea>>,
    bars: Query<(&ScrollBar, &ComputedNode, &RelativeCursorPosition)>,
    thumbs: Query<(&ScrollThumb, &Interaction)>,
) {
    if !mouse.pressed(MouseButton::Left) {
        drag.0 = None;
        return;
    }
    let Ok(area_node) = area.single() else { return };
    let inv = area_node.inverse_scale_factor();
    let area_size = area_node.size() * inv;

    // Begin: a press on a thumb latches that axis and remembers where from.
    if mouse.just_pressed(MouseButton::Left) && drag.0.is_none() {
        for (thumb, interaction) in &thumbs {
            if *interaction != Interaction::Pressed {
                continue;
            }
            let at = bars.iter().find(|(b, _, _)| b.horizontal == thumb.horizontal).and_then(
                |(b, _, rcp)| {
                    rcp.normalized
                        .map(|n| if b.horizontal { n.x } else { n.y })
                },
            );
            if let Some(at) = at {
                let pan = if thumb.horizontal { state.pan.x } else { state.pan.y };
                drag.0 = Some((thumb.horizontal, pan, at));
            }
        }
        return;
    }

    let Some((horizontal, start_pan, start_at)) = drag.0 else {
        return;
    };
    let Some((_, _, rcp)) = bars.iter().find(|(b, _, _)| b.horizontal == horizontal) else {
        return;
    };
    let Some(n) = rcp.normalized else { return };
    let at = if horizontal { n.x } else { n.y };
    let (content, viewport) = extents(&state, area_size, horizontal);
    let span = (content - viewport).max(1.0);
    // `normalized` is a fraction of the track, so a delta in it is a fraction of
    // the scrollable span. Negated because a thumb moving right shows content
    // further right, which is the frame moving left.
    let delta = (at - start_at) * span;
    let next = (start_pan - delta).clamp(-span * 0.5, span * 0.5);
    if horizontal {
        if state.pan.x != next {
            state.pan.x = next;
        }
    } else if state.pan.y != next {
        state.pan.y = next;
    }
}
