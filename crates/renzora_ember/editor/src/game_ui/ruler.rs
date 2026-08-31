//! Design-space rulers along the top and left of the canvas.
//!
//! Two strips overlaid inside the canvas area rather than laid out beside it.
//! The design frame is centred in that area and offset by the pan, so putting
//! the rulers in the flow would move the thing they measure every time one
//! appeared or its width changed. Overlaid, they cost the canvas nothing and
//! stay put.
//!
//! **Ticks are a fixed pool, repositioned every frame — never respawned.**
//! Rebuilding a few hundred nodes per frame is what took the editor to 25 FPS
//! once before (see the ember repaint-churn notes); a ruler is the most tempting
//! place to do it again, because "just rebuild the ticks" is the obvious
//! implementation. Unused ticks are hidden, not despawned.
//!
//! Mapping: the frame is centred, so design x maps to
//! `area_w/2 - frame_w/2 + pan.x + x*zoom`, and likewise for y.

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::*;
use renzora_ember::widgets::{fmt_coord, ruler_step};

use crate::game_ui::NativeCanvasState;

/// Thickness of each strip, and the size of the corner where they meet.
const RULER: f32 = 16.0;
/// How many ticks each axis can draw at once. A 4K-wide panel at a 40px spacing
/// needs ~100; the pool is sized past that so the ruler never runs short, and
/// the surplus costs a hidden node each.
const TICKS: usize = 128;

/// The container holding both strips. Positioned over the canvas area.
#[derive(Component)]
pub(crate) struct RulerRoot;

/// One tick: a line, with a label as its child.
#[derive(Component, Clone, Copy)]
struct Tick {
    horizontal: bool,
}

/// Marks the area node the rulers measure, so the systems can read its size.
#[derive(Component)]
pub(crate) struct RulerArea;

/// Build both strips plus their tick pools. Returns the root, to be added to the
/// canvas area.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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
            // Chrome only — clicks belong to the canvas underneath.
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
            RulerRoot,
            Name::new("ui-canvas-rulers"),
        ))
        .id();

    let strip = |commands: &mut Commands, horizontal: bool| {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: if horizontal { Val::Percent(100.0) } else { Val::Px(RULER) },
                    height: if horizontal { Val::Px(RULER) } else { Val::Percent(100.0) },
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(rgb(panel_bg()).with_alpha(0.92)),
                bevy::ui::FocusPolicy::Pass,
                Pickable::IGNORE,
                Name::new(if horizontal { "ruler-h" } else { "ruler-v" }),
            ))
            .id()
    };
    let h = strip(commands, true);
    let v = strip(commands, false);

    // The corner where the two strips cross, so the vertical one does not run
    // under the horizontal one's numbers.
    let corner = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(RULER),
                height: Val::Px(RULER),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
            GlobalZIndex(1),
            Name::new("ruler-corner"),
        ))
        .id();

    for horizontal in [true, false] {
        let parent = if horizontal { h } else { v };
        for _ in 0..TICKS {
            let label = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(if horizontal { 2.0 } else { 3.0 }),
                        top: Val::Px(if horizontal { 1.0 } else { 2.0 }),
                        ..default()
                    },
                    bevy::ui::widget::Text::new(""),
                    ui_font(&fonts.ui, 8.0),
                    TextColor(rgb(text_muted())),
                    bevy::ui::FocusPolicy::Pass,
                    Pickable::IGNORE,
                ))
                .id();
            let tick = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: if horizontal { Val::Px(1.0) } else { Val::Px(RULER) },
                        height: if horizontal { Val::Px(RULER) } else { Val::Px(1.0) },
                        ..default()
                    },
                    BackgroundColor(rgb(border())),
                    Visibility::Hidden,
                    bevy::ui::FocusPolicy::Pass,
                    Pickable::IGNORE,
                    Tick { horizontal },
                ))
                .id();
            commands.entity(tick).add_child(label);
            commands.entity(parent).add_child(tick);
        }
    }

    // Cursor markers — one per strip, tracking the pointer. Accented and drawn
    // over the ticks, because the point of them is to be found at a glance
    // while you are looking somewhere else entirely.
    for horizontal in [true, false] {
        let parent = if horizontal { h } else { v };
        let mark = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: if horizontal { Val::Px(1.0) } else { Val::Px(RULER) },
                    height: if horizontal { Val::Px(RULER) } else { Val::Px(1.0) },
                    ..default()
                },
                BackgroundColor(rgb(accent())),
                GlobalZIndex(2),
                Visibility::Hidden,
                bevy::ui::FocusPolicy::Pass,
                Pickable::IGNORE,
                CursorMark { horizontal },
                Name::new(if horizontal { "ruler-mark-x" } else { "ruler-mark-y" }),
            ))
            .id();
        commands.entity(parent).add_child(mark);
    }

    commands.entity(root).add_children(&[h, v, corner]);
    root
}

/// The pointer's position on a ruler strip.
#[derive(Component, Clone, Copy)]
struct CursorMark {
    horizontal: bool,
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (position_ticks, position_cursor_marks)
            .after(crate::game_ui::geometry::snapshot_widgets)
            .run_if(in_state(renzora::SplashState::Editor)),
    );
}

/// Slide each strip's marker to the pointer.
///
/// Measured against the canvas *area*, not the design frame: the cursor is
/// still somewhere meaningful when it is out over the dark surround, and a
/// marker that vanished the moment you left the frame would drop out exactly
/// when you were reaching for an edge.
fn position_cursor_marks(
    state: Res<NativeCanvasState>,
    area: Query<(&ComputedNode, &bevy::ui::RelativeCursorPosition), With<RulerArea>>,
    mut marks: Query<(&CursorMark, &mut Node, &mut Visibility)>,
) {
    let show = state.show_rulers && state.active_canvas.is_some();
    let at = area.single().ok().filter(|_| show).and_then(|(cn, rcp)| {
        let n = rcp.normalized?;
        rcp.cursor_over
            .then(|| Vec2::new((n.x + 0.5) * cn.size.x, (n.y + 0.5) * cn.size.y))
    });
    for (mark, mut node, mut vis) in &mut marks {
        match at {
            Some(p) => {
                if mark.horizontal {
                    node.left = Val::Px(p.x);
                } else {
                    node.top = Val::Px(p.y);
                }
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
            None => {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

/// Lay the ticks out from the current pan/zoom, and hide the ones not needed.
fn position_ticks(
    state: Res<NativeCanvasState>,
    area: Query<&ComputedNode, With<RulerArea>>,
    mut roots: Query<&mut Visibility, (With<RulerRoot>, Without<Tick>)>,
    mut ticks: Query<(&Tick, &mut Node, &mut Visibility, &Children), Without<RulerRoot>>,
    mut labels: Query<&mut bevy::ui::widget::Text>,
) {
    let show = state.show_rulers && state.active_canvas.is_some();
    for mut vis in &mut roots {
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
    if !show {
        return;
    }
    let Ok(area) = area.single() else { return };
    let (aw, ah) = (area.size.x, area.size.y);
    let zoom = state.zoom.max(0.0001);
    // Design (0,0) in area-local pixels: the frame is centred, then panned.
    let origin = Vec2::new(
        aw * 0.5 - state.canvas_width * zoom * 0.5 + state.pan.x,
        ah * 0.5 - state.canvas_height * zoom * 0.5 + state.pan.y,
    );
    // Aim for a tick roughly every 64 screen pixels, snapped to the grid when
    // one is showing so the numbers sit on grid lines.
    let grid = state.show_grid.then_some(state.grid_size);
    let step = ruler_step(64.0 / zoom, grid);

    // First visible design coordinate on each axis, rounded down to a step.
    let first = |origin: f32| -> f32 {
        let design_at_zero = -origin / zoom;
        (design_at_zero / step).floor() * step
    };
    let (mut hx, mut vy) = (first(origin.x), first(origin.y));

    let mut used_h = 0usize;
    let mut used_v = 0usize;
    for (tick, mut node, mut vis, kids) in &mut ticks {
        let (pos, coord, used, extent) = if tick.horizontal {
            let p = origin.x + hx * zoom;
            let c = hx;
            hx += step;
            used_h += 1;
            (p, c, used_h, aw)
        } else {
            let p = origin.y + vy * zoom;
            let c = vy;
            vy += step;
            used_v += 1;
            (p, c, used_v, ah)
        };
        // Off the end of the strip, or past the pool's share — park it.
        if pos < -RULER || pos > extent || used > TICKS {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        if tick.horizontal {
            node.left = Val::Px(pos);
        } else {
            node.top = Val::Px(pos);
        }
        let text = fmt_coord(coord, step);
        for kid in kids.iter() {
            if let Ok(mut t) = labels.get_mut(kid) {
                if t.0 != text {
                    t.0 = text.clone();
                }
            }
        }
    }
}
