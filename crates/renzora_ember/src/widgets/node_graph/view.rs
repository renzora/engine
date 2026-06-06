//! Data-driven node-graph **view** — the reusable engine behind real graph
//! editors (material / blueprint / particle), built on the same pannable/zoomable
//! canvas + GPU bezier cables as the [`super::node_graph`] demo, but driven by the
//! caller's own model instead of a hardcoded scene.
//!
//! The caller mounts its nodes + wires (from its graph model) into the canvas via
//! `keyed_list` using [`graph_node_view`] / [`graph_wire_view`], keyed on the
//! graph *structure* (not node positions) so dragging never triggers a rebuild.
//! Ports and wires carry the model's `(node_id, pin)` ids, so cables resolve
//! their endpoints by tag every frame — robust across rebuilds. User edits (node
//! moved, wire connected/removed, node selected) are recorded as [`GraphEdit`]s in
//! the [`NodeGraphView`] component; the caller drains them and applies them to its
//! model (marking it dirty).

use std::collections::HashMap;

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, UiGlobalTransform, UiTransform};
use bevy::ui_render::prelude::MaterialNode;
use bevy::window::SystemCursorIcon;

use super::{grid_node, CableMaterial, GraphPan, GraphView};
use crate::font::{ui_font, EmberFonts};
use crate::theme::*;

const NODE_W: f32 = 160.0;
const HEAD_H: f32 = 26.0;
const ROW_H: f32 = 22.0;
const WIRE_W: f32 = 2.5;

/// Registers the data-driven node-graph view systems.
pub(crate) struct NodeGraphViewPlugin;

impl Plugin for NodeGraphViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (ngv_cable_attach, ngv_drag, ngv_connect, ngv_remove, ngv_apply_selection));
        app.add_systems(PostUpdate, ngv_endpoints.after(bevy::ui::UiSystems::Layout));
    }
}

/// A user edit the view recorded; the caller drains [`NodeGraphView::pending`]
/// and applies these to its own graph model.
pub enum GraphEdit {
    NodeMoved { id: u64, x: f32, y: f32 },
    Connect { from_node: u64, from_pin: String, to_node: u64, to_pin: String },
    Disconnect { from_node: u64, from_pin: String, to_node: u64, to_pin: String },
    Select { id: Option<u64> },
}

/// Lives on the graph viewport; the caller syncs by draining `pending`.
#[derive(Component, Default)]
pub struct NodeGraphView {
    pub pending: Vec<GraphEdit>,
    /// Currently-selected node id — drives the node border without rebuilding
    /// the node entity (so a drag-started selection doesn't kill the drag). The
    /// caller may also set this to reflect external selection changes.
    pub selected: Option<u64>,
}

/// Entities the caller mounts content into.
pub struct NodeGraphHandle {
    /// Carries [`NodeGraphView`] — add a marker + drain its `pending` each frame.
    pub viewport: Entity,
    /// Pan/zoom canvas — mount nodes (`graph_node_view`) + wires (`graph_wire_view`) here.
    pub canvas: Entity,
}

#[derive(Component)]
struct NgvNode {
    id: u64,
    canvas: Entity,
    viewport: Entity,
}
#[derive(Component)]
struct NgvPort {
    node_id: u64,
    pin: String,
    is_output: bool,
    color: (u8, u8, u8),
}
#[derive(Component)]
struct NgvWire {
    from_node: u64,
    from_pin: String,
    to_node: u64,
    to_pin: String,
    viewport: Entity,
}

// ── Build ────────────────────────────────────────────────────────────────────

/// Build an empty graph view. Mount nodes/wires into `handle.canvas`.
pub fn node_graph_view(commands: &mut Commands, _fonts: &EmberFonts) -> NodeGraphHandle {
    let viewport = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            RelativeCursorPosition::default(),
            NodeGraphView::default(),
            Name::new("node-graph-view"),
        ))
        .id();
    let canvas = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            UiTransform::default(),
            GraphView { zoom: 1.0, pan: Vec2::ZERO },
            GraphPan,
            RelativeCursorPosition::default(),
            Name::new("node-graph-view-canvas"),
        ))
        .id();
    let grid = grid_node(commands, canvas);
    commands.entity(viewport).add_children(&[grid, canvas]);
    NodeGraphHandle { viewport, canvas }
}

/// Build one node at `(x, y)` (canvas px) with typed input/output pins
/// (`(pin_id, label)`). Returns the node entity (add it to `handle.canvas`).
#[allow(clippy::too_many_arguments)]
pub fn graph_node_view(
    commands: &mut Commands,
    fonts: &EmberFonts,
    canvas: Entity,
    viewport: Entity,
    node_id: u64,
    title: &str,
    color: (u8, u8, u8),
    inputs: &[(String, String, (u8, u8, u8))],
    outputs: &[(String, String, (u8, u8, u8))],
    x: f32,
    y: f32,
    selected: bool,
) -> Entity {
    let node = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(NODE_W),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 })),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            BorderColor::all(rgb(if selected { accent() } else { tree_line() })),
            NgvNode { id: node_id, canvas, viewport },
            Interaction::default(),
            RelativeCursorPosition::default(),
            GlobalZIndex(5),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Move),
            Name::new("ngv-node"),
        ))
        .id();
    let title_bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(HEAD_H), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::top(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(color)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("ngv-node-title"),
        ))
        .with_children(|p| {
            p.spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(on_accent())), bevy::text::TextLayout::new_with_no_wrap()));
        })
        .id();
    commands.entity(node).add_child(title_bar);

    let mut row = 0usize;
    for (pin, label, pin_color) in inputs {
        let cy = HEAD_H + row as f32 * ROW_H + ROW_H / 2.0;
        let r = graph_row(commands, fonts, label, false);
        let port = port_dot(commands, node_id, pin, false, 0.0, cy, *pin_color);
        commands.entity(node).add_children(&[r, port]);
        row += 1;
    }
    for (pin, label, pin_color) in outputs {
        let cy = HEAD_H + row as f32 * ROW_H + ROW_H / 2.0;
        let r = graph_row(commands, fonts, label, true);
        let port = port_dot(commands, node_id, pin, true, NODE_W, cy, *pin_color);
        commands.entity(node).add_children(&[r, port]);
        row += 1;
    }
    node
}

fn graph_row(commands: &mut Commands, fonts: &EmberFonts, name: &str, output: bool) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(ROW_H),
                align_items: AlignItems::Center,
                justify_content: if output { JustifyContent::FlexEnd } else { JustifyContent::FlexStart },
                padding: if output { UiRect::right(Val::Px(12.0)) } else { UiRect::left(Val::Px(12.0)) },
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|p| {
            p.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::new_with_no_wrap()));
        })
        .id()
}

fn port_dot(commands: &mut Commands, node_id: u64, pin: &str, is_output: bool, x: f32, cy: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(x - 5.0), top: Val::Px(cy - 5.0), width: Val::Px(10.0), height: Val::Px(10.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(color)),
            BorderColor::all(rgb(color)),
            NgvPort { node_id, pin: pin.to_string(), is_output, color },
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Crosshair),
            Name::new("ngv-port"),
        ))
        .id()
}

/// Build a wire between two model pins. Returns the cable entity (add it to
/// `handle.canvas`). The cable resolves its endpoints by `(node_id, pin)` tag.
#[allow(clippy::too_many_arguments)]
pub fn graph_wire_view(commands: &mut Commands, viewport: Entity, from_node: u64, from_pin: &str, to_node: u64, to_pin: &str) -> Entity {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            NgvWire { from_node, from_pin: from_pin.to_string(), to_node, to_pin: to_pin.to_string(), viewport },
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
            GlobalZIndex(1),
            Name::new("ngv-cable"),
        ))
        .id()
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn ngv_cable_attach(mut commands: Commands, mut materials: ResMut<Assets<CableMaterial>>, cables: Query<Entity, (With<NgvWire>, Without<MaterialNode<CableMaterial>>)>) {
    for e in &cables {
        let handle = materials.add(CableMaterial::default());
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn cursor(windows: &Query<&Window>) -> Option<Vec2> {
    windows.single().ok().and_then(|w| w.cursor_position())
}

fn px(v: Val) -> f32 {
    if let Val::Px(p) = v {
        p
    } else {
        0.0
    }
}

/// Drag a node body → move it; on release record `NodeMoved`. Press → `Select`.
fn ngv_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: Local<Option<(Entity, Vec2, bool)>>,
    picks: Query<(Entity, &Interaction, &NgvNode)>,
    views: Query<&GraphView>,
    mut nodes: Query<&mut Node>,
    mut graphs: Query<&mut NodeGraphView>,
) {
    if active.is_none() {
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(c) = cursor(&windows) {
                for (e, interaction, n) in &picks {
                    if *interaction == Interaction::Pressed {
                        *active = Some((e, c, false));
                        if let Ok(mut g) = graphs.get_mut(n.viewport) {
                            // Select immediately (visual updates without a rebuild,
                            // so the drag entity stays alive) + record the edit.
                            g.selected = Some(n.id);
                            g.pending.push(GraphEdit::Select { id: Some(n.id) });
                        }
                        break;
                    }
                }
            }
        }
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        if let Some((e, _, moved)) = *active {
            if moved {
                if let (Ok((_, _, n)), Ok(node)) = (picks.get(e), nodes.get(e)) {
                    let (x, y) = (px(node.left), px(node.top));
                    let vp = n.viewport;
                    if let Ok(mut g) = graphs.get_mut(vp) {
                        g.pending.push(GraphEdit::NodeMoved { id: n.id, x, y });
                    }
                }
            }
        }
        *active = None;
        return;
    }
    let (Some((node, last, moved)), Some(c)) = (*active, cursor(&windows)) else {
        return;
    };
    let delta = c - last;
    let zoom = picks.get(node).ok().and_then(|(_, _, n)| views.get(n.canvas).ok()).map(|v| v.zoom).unwrap_or(1.0);
    let mut now_moved = moved;
    if delta != Vec2::ZERO {
        if let Ok(mut n) = nodes.get_mut(node) {
            n.left = Val::Px(px(n.left) + delta.x / zoom);
            n.top = Val::Px(px(n.top) + delta.y / zoom);
        }
        now_moved = true;
    }
    *active = Some((node, c, now_moved));
}

/// Drive each node's border (width + colour) from its viewport's `selected` id,
/// in place — so selecting a node never rebuilds it (which would kill an
/// in-progress drag). Only writes when the selection state actually flips.
fn ngv_apply_selection(views: Query<&NodeGraphView>, mut nodes: Query<(&NgvNode, &mut Node, &mut BorderColor)>) {
    for (n, mut node, mut bc) in &mut nodes {
        let sel = views.get(n.viewport).map(|v| v.selected == Some(n.id)).unwrap_or(false);
        let want = UiRect::all(Val::Px(if sel { 2.0 } else { 1.0 }));
        if node.border != want {
            node.border = want;
            *bc = BorderColor::all(rgb(if sel { accent() } else { tree_line() }));
        }
    }
}

/// Output-port click then input-port click → record a `Connect`.
fn ngv_connect(mut pending: Local<Option<(u64, String)>>, pressed: Query<(&Interaction, &NgvPort, &ChildOf), Changed<Interaction>>, parents: Query<&NgvNode>, mut graphs: Query<&mut NodeGraphView>) {
    for (interaction, port, child_of) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if port.is_output {
            *pending = Some((port.node_id, port.pin.clone()));
        } else if let Some((from_node, from_pin)) = pending.take() {
            // The viewport is reachable via the owning node.
            if let Ok(node) = parents.get(child_of.parent()) {
                if let Ok(mut g) = graphs.get_mut(node.viewport) {
                    g.pending.push(GraphEdit::Connect { from_node, from_pin, to_node: port.node_id, to_pin: port.pin.clone() });
                }
            }
        }
    }
}

/// Click empty canvas over a cable → record a `Disconnect`.
fn ngv_remove(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    blockers: Query<&Interaction, Or<(With<NgvNode>, With<NgvPort>)>>,
    wires: Query<&NgvWire>,
    ports: Query<(&NgvPort, &UiGlobalTransform)>,
    transforms: Query<&UiGlobalTransform>,
    computeds: Query<&ComputedNode>,
    mut graphs: Query<&mut NodeGraphView>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if blockers.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(cl) = cursor(&windows) else {
        return;
    };
    let map = port_map(&ports);
    let mut best: Option<(u64, String, u64, String, f32)> = None;
    for w in &wires {
        let (Some(&(p0, _)), Some(&(p3, _))) = (map.get(&(w.from_node, w.from_pin.clone(), true)), map.get(&(w.to_node, w.to_pin.clone(), false))) else {
            continue;
        };
        let (Ok(vgt), Ok(vcn)) = (transforms.get(w.viewport), computeds.get(w.viewport)) else {
            continue;
        };
        let isf = vcn.inverse_scale_factor();
        let top_left = vgt.translation - vcn.size() * 0.5;
        let cur = cl / isf - top_left;
        let a = p0 - top_left;
        let b = p3 - top_left;
        let (c1, c2) = control_points(a, b);
        let mut d = f32::MAX;
        let mut prev = a;
        for i in 1..=24 {
            let pt = bezier(a, c1, c2, b, i as f32 / 24.0);
            d = d.min(seg_dist(cur, prev, pt));
            prev = pt;
        }
        if d < 8.0 / isf && best.as_ref().is_none_or(|(_, _, _, _, bd)| d < *bd) {
            best = Some((w.from_node, w.from_pin.clone(), w.to_node, w.to_pin.clone(), d));
        }
    }
    if let Some((fnode, fpin, tnode, tpin, _)) = best {
        // Record on the first viewport (single graph per view).
        if let Some(mut g) = graphs.iter_mut().next() {
            g.pending.push(GraphEdit::Disconnect { from_node: fnode, from_pin: fpin, to_node: tnode, to_pin: tpin });
        }
    }
}

fn port_map(ports: &Query<(&NgvPort, &UiGlobalTransform)>) -> HashMap<(u64, String, bool), (Vec2, (u8, u8, u8))> {
    let mut map = HashMap::default();
    for (p, gt) in ports {
        map.insert((p.node_id, p.pin.clone(), p.is_output), (gt.translation, p.color));
    }
    map
}

/// Refresh every cable's control points from its endpoints' live transforms.
fn ngv_endpoints(mut materials: ResMut<Assets<CableMaterial>>, wires: Query<(&NgvWire, &MaterialNode<CableMaterial>)>, ports: Query<(&NgvPort, &UiGlobalTransform)>, transforms: Query<&UiGlobalTransform>, computeds: Query<&ComputedNode>) {
    if wires.is_empty() {
        return;
    }
    let map = port_map(&ports);
    for (w, mat) in &wires {
        let (Some(&(p0, wire_col)), Some(&(p3, _))) = (map.get(&(w.from_node, w.from_pin.clone(), true)), map.get(&(w.to_node, w.to_pin.clone(), false))) else {
            continue;
        };
        let (Ok(vgt), Ok(vcn)) = (transforms.get(w.viewport), computeds.get(w.viewport)) else {
            continue;
        };
        let isf = vcn.inverse_scale_factor();
        let top_left = vgt.translation - vcn.size() * 0.5;
        let a = p0 - top_left;
        let b = p3 - top_left;
        let (c1, c2) = control_points(a, b);
        let lin = rgb(wire_col).to_linear(); // wire takes the output pin's colour
        if let Some(m) = materials.get_mut(&mat.0) {
            m.ab = Vec4::new(a.x, a.y, c1.x, c1.y);
            m.cd = Vec4::new(c2.x, c2.y, b.x, b.y);
            m.color = Vec4::new(lin.red, lin.green, lin.blue, 1.0);
            m.params = Vec4::new(WIRE_W / isf, 1.0, 0.0, 0.0);
        }
    }
}

// ── Local copies of the demo's curve math ────────────────────────────────────

fn bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let m = 1.0 - t;
    p0 * (m * m * m) + p1 * (3.0 * m * m * t) + p2 * (3.0 * m * t * t) + p3 * (t * t * t)
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(ba) / ba.dot(ba).max(1e-5)).clamp(0.0, 1.0);
    (pa - ba * h).length()
}

fn control_points(p0: Vec2, p3: Vec2) -> (Vec2, Vec2) {
    let dx = (p3.x - p0.x).abs().max(40.0) * 0.5;
    (p0 + Vec2::new(dx, 0.0), p3 - Vec2::new(dx, 0.0))
}
