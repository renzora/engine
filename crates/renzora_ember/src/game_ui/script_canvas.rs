//! Script draw canvas — the render half of the `on_draw(g)` API.
//!
//! A markup node with a `canvas` attribute (`<node canvas width="300px"
//! height="300px">`) becomes a [`ScriptCanvas`] drawing surface. Each frame:
//!
//! 1. [`publish_canvas_surfaces`] tells `renzora_scripting` the surface's px size
//!    (via [`renzora::ScriptDrawSurfaces`]), so the script's `on_draw(g)` gets the
//!    right `g.width`/`g.height`.
//! 2. The script's `on_draw` fills [`renzora::ScriptDrawBuffer`] with `DrawCmd`s.
//! 3. [`render_script_canvas`] reconciles that list into a **pool of the existing
//!    SDF shape entities** ([`ArcShape`], [`CircleShape`], …) parented under the
//!    canvas node — reused in place (no per-frame spawn/despawn churn) and ordered
//!    by draw index via `ZIndex`, so a needle drawn after its dial sits on top.
//!
//! Coordinates are the canvas's local px (top-left origin, y-down).

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use renzora::DrawCmd;

use super::shapes::{ArcShape, CircleShape, LineShape, RectangleShape, UiShapeWidget};

/// A `<node canvas>` surface a script draws into via `on_draw(g)`.
#[derive(Component)]
pub struct ScriptCanvas {
    /// The entity whose `on_draw` feeds this surface — the markup binding host,
    /// i.e. the canvas/entity the script is attached to.
    pub owner: Entity,
    // Per-type child-entity pools, reused across frames.
    arcs: Vec<Entity>,
    lines: Vec<Entity>,
    circles: Vec<Entity>,
    rects: Vec<Entity>,
    texts: Vec<Entity>,
}

impl ScriptCanvas {
    pub fn new(owner: Entity) -> Self {
        Self {
            owner,
            arcs: Vec::new(),
            lines: Vec::new(),
            circles: Vec::new(),
            rects: Vec::new(),
            texts: Vec::new(),
        }
    }
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, publish_canvas_surfaces);
    // Run the reconcile BEFORE the shape material-sync systems so every shape we
    // mutate this frame repaints the same frame. Without this the syncs run in an
    // arbitrary order and a fast-moving needle/fill can update on different frames
    // and visibly separate ("jiggle"); a sync point is inserted between us and the
    // syncs so they observe our just-written shapes.
    app.add_systems(
        Update,
        render_script_canvas
            .after(publish_canvas_surfaces)
            .before(super::shapes::sync_arc_materials)
            .before(super::shapes::sync_line_materials)
            .before(super::shapes::sync_circle_materials)
            .before(super::shapes::sync_rectangle_materials),
    );
}

/// Publish each canvas's size so scripts' `on_draw` is sized correctly. Cleared +
/// rebuilt each frame; scripting reads it (a frame's lag on a resize is harmless).
fn publish_canvas_surfaces(
    canvases: Query<(&ScriptCanvas, &ComputedNode)>,
    mut surfaces: ResMut<renzora::ScriptDrawSurfaces>,
) {
    surfaces.per_entity.clear();
    for (canvas, cn) in &canvases {
        surfaces.per_entity.insert(canvas.owner, cn.size);
    }
}

fn col(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
}

/// An absolutely-positioned child node covering `(left, top, w, h)` in the
/// canvas's local space — the box a shape's SDF material fills.
fn abs_node(left: f32, top: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(w.max(0.0)),
        height: Val::Px(h.max(0.0)),
        ..default()
    }
}

/// Reconcile each canvas's draw list into its pooled shape children.
fn render_script_canvas(
    mut commands: Commands,
    draws: Res<renzora::ScriptDrawBuffer>,
    mut canvases: Query<(Entity, &mut ScriptCanvas)>,
) {
    let empty: Vec<DrawCmd> = Vec::new();
    for (canvas_entity, mut canvas) in &mut canvases {
        let cmds = draws.per_entity.get(&canvas.owner).unwrap_or(&empty);
        let (mut ai, mut li, mut ci, mut ri, mut ti) = (0usize, 0usize, 0usize, 0usize, 0usize);

        for (z, cmd) in cmds.iter().enumerate() {
            let z = ZIndex(z as i32);
            match cmd {
                DrawCmd::Arc {
                    cx,
                    cy,
                    r,
                    start,
                    end,
                    color,
                    thickness,
                } => {
                    let e = slot(&mut commands, &mut canvas.arcs, ai, canvas_entity);
                    commands.entity(e).try_insert((
                        abs_node(cx - r, cy - r, 2.0 * r, 2.0 * r),
                        z,
                        ArcShape {
                            color: col(*color),
                            start_angle: *start,
                            end_angle: *end,
                            thickness: (thickness / r.max(0.001)).clamp(0.01, 1.0),
                        },
                    ));
                    ai += 1;
                }
                DrawCmd::Circle { cx, cy, r, color } => {
                    let e = slot(&mut commands, &mut canvas.circles, ci, canvas_entity);
                    commands.entity(e).try_insert((
                        abs_node(cx - r, cy - r, 2.0 * r, 2.0 * r),
                        z,
                        CircleShape {
                            color: col(*color),
                            ..default()
                        },
                    ));
                    ci += 1;
                }
                DrawCmd::Rect { x, y, w, h, color } => {
                    let e = slot(&mut commands, &mut canvas.rects, ri, canvas_entity);
                    commands.entity(e).try_insert((
                        abs_node(*x, *y, *w, *h),
                        z,
                        RectangleShape {
                            color: col(*color),
                            ..default()
                        },
                    ));
                    ri += 1;
                }
                DrawCmd::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    thickness,
                } => {
                    // LineShape draws an infinite line through the node CENTRE at
                    // `angle`, clipped to the rect — so centre the node on the
                    // segment's midpoint (not its bbox corner) and pad symmetrically
                    // to the stroke width. Corner-anchoring + padding would shift the
                    // centre off the segment near vertical/horizontal, wobbling the
                    // line as it rotates through those angles.
                    let midx = (x1 + x2) * 0.5;
                    let midy = (y1 + y2) * 0.5;
                    let w = (x2 - x1).abs().max(*thickness);
                    let h = (y2 - y1).abs().max(*thickness);
                    let angle = (y2 - y1).atan2(x2 - x1).to_degrees();
                    let e = slot(&mut commands, &mut canvas.lines, li, canvas_entity);
                    commands.entity(e).try_insert((
                        abs_node(midx - w * 0.5, midy - h * 0.5, w, h),
                        z,
                        LineShape {
                            color: col(*color),
                            thickness: *thickness,
                            angle,
                        },
                    ));
                    li += 1;
                }
                DrawCmd::Text {
                    x,
                    y,
                    text,
                    size,
                    color,
                } => {
                    let e = slot_text(&mut commands, &mut canvas.texts, ti, canvas_entity);
                    commands.entity(e).try_insert((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(*x),
                            top: Val::Px(*y),
                            ..default()
                        },
                        z,
                        Text::new(text.clone()),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(*size),
                            ..default()
                        },
                        TextColor(col(*color)),
                    ));
                    ti += 1;
                }
            }
        }

        // Hide pool entries the frame didn't use (Display::None keeps them cheap
        // and reusable instead of despawning + respawning next frame).
        hide_from(&mut commands, &canvas.arcs, ai);
        hide_from(&mut commands, &canvas.circles, ci);
        hide_from(&mut commands, &canvas.rects, ri);
        hide_from(&mut commands, &canvas.lines, li);
        hide_from(&mut commands, &canvas.texts, ti);
    }
}

/// Get pool slot `index`, spawning a fresh shape child under `parent` if the pool
/// is exhausted. Shape children carry `UiShapeWidget` (so the style pass leaves
/// them alone) and are hidden from the scene hierarchy.
fn slot(commands: &mut Commands, pool: &mut Vec<Entity>, index: usize, parent: Entity) -> Entity {
    if let Some(&e) = pool.get(index) {
        return e;
    }
    let e = commands
        .spawn((UiShapeWidget, renzora::HideInHierarchy, ChildOf(parent)))
        .id();
    pool.push(e);
    e
}

/// Text slot — like [`slot`] but without `UiShapeWidget` (it's a `Text` node).
fn slot_text(
    commands: &mut Commands,
    pool: &mut Vec<Entity>,
    index: usize,
    parent: Entity,
) -> Entity {
    if let Some(&e) = pool.get(index) {
        return e;
    }
    let e = commands
        .spawn((renzora::HideInHierarchy, ChildOf(parent)))
        .id();
    pool.push(e);
    e
}

/// Hide every pool entry from `used` onward this frame.
fn hide_from(commands: &mut Commands, pool: &[Entity], used: usize) {
    for &e in pool.iter().skip(used) {
        commands.entity(e).try_insert(Node {
            display: Display::None,
            ..default()
        });
    }
}
