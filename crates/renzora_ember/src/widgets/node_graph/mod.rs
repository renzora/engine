//! Node graph — a pannable/zoomable canvas of draggable nodes wired port-to-port
//! with smooth bezier cables (material/blueprint style).
//!
//! Drag a node by its body (not its pins) with the left mouse, pan with the
//! middle mouse, zoom with the wheel (`UiTransform` on the canvas). Cables are
//! cubic beziers with horizontal tangents, painted on the GPU by a [`UiMaterial`]
//! whose fragment shader computes a signed distance to the curve and antialiases
//! it — one full-viewport material node per cable, its control points refreshed
//! each frame from the pins' `UiGlobalTransform`. Click an output pin then an
//! input pin to connect; click a cable to remove it.

use bevy::asset::Asset;
use bevy::input::mouse::MouseWheel;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui::{ComputedNode, RelativeCursorPosition, UiGlobalTransform, UiTransform, Val2};
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;
use bevy::window::SystemCursorIcon;

use crate::font::{ui_font, EmberFonts};
use crate::theme::*;

const NODE_W: f32 = 150.0;
const HEAD_H: f32 = 26.0;
const ROW_H: f32 = 24.0;
const WIRE_W: f32 = 2.5;

mod view;
pub use view::{graph_node_view, graph_wire_view, node_graph_view, GraphEdit, NodeGraphHandle, NodeGraphView};

/// Registers the cable material + shader and the node-graph systems.
pub(crate) struct NodeGraphPlugin;

impl Plugin for NodeGraphPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "cable.wgsl");
        bevy::asset::embedded_asset!(app, "grid.wgsl");
        app.add_plugins(UiMaterialPlugin::<CableMaterial>::default());
        app.add_plugins(UiMaterialPlugin::<GridMaterial>::default());
        app.add_plugins(view::NodeGraphViewPlugin);
        app.add_systems(
            Update,
            (
                graph_drag,
                graph_pan,
                graph_zoom,
                graph_connect,
                graph_remove,
                cable_attach,
                grid_attach,
                apply_node_graph_style,
            ),
        );
        app.add_systems(
            PostUpdate,
            (update_endpoints, update_grid).after(bevy::ui::UiSystems::Layout),
        );
    }
}

/// Which node-graph element a node paints from [`crate::style::NodeGraphStyle`].
#[derive(Component, Clone, Copy)]
pub(crate) enum NgPart {
    Canvas,
    Node,
    Header,
    Port,
}

/// Paint every [`NgPart`] from the live `Theme.node_graph` when the theme changes
/// or a part is added — the node graph's own targetable elements (canvas, node,
/// header, port) follow the theme, independently of every other widget.
pub(crate) fn apply_node_graph_style(
    theme: Res<crate::style::Theme>,
    mut q: Query<(Ref<NgPart>, &mut BackgroundColor, Option<&mut BorderColor>)>,
) {
    let repaint_all = theme.is_changed();
    let ng = &theme.node_graph;
    for (part, mut bg, border) in &mut q {
        if !repaint_all && !part.is_added() {
            continue;
        }
        let (fill, stroke) = match *part {
            NgPart::Canvas => (ng.canvas_bg, Some(ng.canvas_border)),
            NgPart::Node => (ng.node_bg, Some(ng.node_border)),
            NgPart::Header => (ng.node_header, None),
            NgPart::Port => (ng.node_selected_bg, Some(ng.cable)),
        };
        bg.0 = fill.color();
        if let (Some(mut bc), Some(s)) = (border, stroke) {
            *bc = BorderColor::all(s.color());
        }
    }
}

/// GPU-painted bezier cable: control points + color + stroke params.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct CableMaterial {
    #[uniform(0)]
    ab: Vec4,
    #[uniform(0)]
    cd: Vec4,
    #[uniform(0)]
    color: Vec4,
    #[uniform(0)]
    params: Vec4,
}

impl Default for CableMaterial {
    fn default() -> Self {
        let c = rgb(accent()).to_linear();
        Self {
            ab: Vec4::ZERO,
            cd: Vec4::ZERO,
            color: Vec4::new(c.red, c.green, c.blue, 1.0),
            params: Vec4::new(WIRE_W, 1.0, 0.0, 0.0),
        }
    }
}

impl UiMaterial for CableMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/node_graph/cable.wgsl".into()
    }
}

/// GPU-painted dotted grid background; uniforms refreshed from the canvas pan/zoom.
#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub(crate) struct GridMaterial {
    #[uniform(0)]
    view: Vec4,
    #[uniform(0)]
    size: Vec4,
    #[uniform(0)]
    bg: Vec4,
    #[uniform(0)]
    dot: Vec4,
}

impl UiMaterial for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/node_graph/grid.wgsl".into()
    }
}

/// The dotted background node for a graph; reads pan/zoom from `canvas`.
#[derive(Component)]
pub(crate) struct GraphGrid {
    pub(crate) canvas: Entity,
}

/// Grid spacing in canvas-logical px (matches the egui editor's 20px dot grid).
const GRID_SPACING: f32 = 20.0;

/// Spawn a full-viewport dotted-grid background node that tracks `canvas`'s
/// pan/zoom. Add it behind the canvas in the viewport.
pub(crate) fn grid_node(commands: &mut Commands, canvas: Entity) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GraphGrid { canvas },
            GlobalZIndex(0),
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
            Name::new("node-graph-grid"),
        ))
        .id()
}

/// Give freshly-spawned grid nodes a `GridMaterial`.
fn grid_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<GridMaterial>>,
    grids: Query<Entity, (With<GraphGrid>, Without<MaterialNode<GridMaterial>>)>,
) {
    for e in &grids {
        let handle = materials.add(GridMaterial::default());
        commands.entity(e).insert(MaterialNode(handle));
    }
}

/// Refresh each grid's uniforms from its canvas pan/zoom + the theme colours.
fn update_grid(
    theme: Res<crate::style::Theme>,
    mut materials: ResMut<Assets<GridMaterial>>,
    grids: Query<(&GraphGrid, &ComputedNode, &MaterialNode<GridMaterial>)>,
    views: Query<&GraphView>,
) {
    if grids.is_empty() {
        return;
    }
    let bg_c = theme.node_graph.canvas_bg.color();
    let bg = bg_c.to_linear();
    // Dots a touch lighter than the canvas (egui: 25 → 35).
    let bg_s = bg_c.to_srgba();
    let dot = Color::srgb(bg_s.red + 0.045, bg_s.green + 0.045, bg_s.blue + 0.05).to_linear();
    for (grid, cn, mat) in &grids {
        let Ok(view) = views.get(grid.canvas) else {
            continue;
        };
        let isf = cn.inverse_scale_factor().max(1e-5);
        let pan = view.pan / isf; // logical → physical px
        let spacing = GRID_SPACING / isf;
        let dot_r = (1.5 * view.zoom.sqrt()) / isf;
        if let Some(m) = materials.get_mut(&mat.0) {
            m.view = Vec4::new(view.zoom, pan.x, pan.y, spacing);
            m.size = Vec4::new(dot_r, 0.0, 0.0, 0.0);
            m.bg = Vec4::new(bg.red, bg.green, bg.blue, 1.0);
            m.dot = Vec4::new(dot.red, dot.green, dot.blue, 1.0);
        }
    }
}

/// Per-canvas pan/zoom state (mirrors the canvas `UiTransform`).
#[derive(Component)]
pub(crate) struct GraphView {
    zoom: f32,
    pan: Vec2,
}

#[derive(Component)]
pub(crate) struct GraphPan;

#[derive(Component)]
pub(crate) struct GraphNode {
    canvas: Entity,
}

#[derive(Component)]
pub(crate) struct GraphPort {
    viewport: Entity,
    is_output: bool,
}

#[derive(Component)]
pub(crate) struct GraphWire {
    from: Entity,
    to: Entity,
    viewport: Entity,
}

fn px(v: Val) -> f32 {
    if let Val::Px(p) = v {
        p
    } else {
        0.0
    }
}

fn cursor(windows: &Query<&Window>) -> Option<Vec2> {
    windows.single().ok().and_then(|w| w.cursor_position())
}

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

/// Control points for a cable between `p0` (output) and `p3` (input): horizontal
/// tangents give the loose hanging-cable curve.
fn control_points(p0: Vec2, p3: Vec2) -> (Vec2, Vec2) {
    let dx = (p3.x - p0.x).abs().max(40.0) * 0.5;
    (p0 + Vec2::new(dx, 0.0), p3 - Vec2::new(dx, 0.0))
}

/// A demo node graph (Texture/Color → Mix → Output), pre-wired and interactive.
pub fn node_graph(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let viewport = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(280.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            RelativeCursorPosition::default(),
            NgPart::Canvas,
            Name::new("node-graph"),
        ))
        .id();
    let canvas = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            UiTransform::default(),
            GraphView {
                zoom: 1.0,
                pan: Vec2::ZERO,
            },
            GraphPan,
            RelativeCursorPosition::default(),
            Name::new("graph-canvas"),
        ))
        .id();
    let grid = grid_node(commands, canvas);
    commands.entity(viewport).add_children(&[grid, canvas]);

    let (n_tex, _, tex_out) =
        graph_node(commands, fonts, canvas, viewport, "Texture", &[], &["RGB"], 20.0, 26.0);
    let (n_col, _, col_out) =
        graph_node(commands, fonts, canvas, viewport, "Color", &[], &["RGB"], 20.0, 150.0);
    let (n_mix, mix_in, mix_out) = graph_node(
        commands, fonts, canvas, viewport, "Mix", &["A", "B"], &["Out"], 220.0, 72.0,
    );
    let (n_out, out_in, _) = graph_node(
        commands, fonts, canvas, viewport, "Output", &["Surface"], &[], 410.0, 100.0,
    );
    commands.entity(canvas).add_children(&[n_tex, n_col, n_mix, n_out]);

    make_wire(commands, viewport, tex_out[0], mix_in[0]);
    make_wire(commands, viewport, col_out[0], mix_in[1]);
    make_wire(commands, viewport, mix_out[0], out_in[0]);
    viewport
}

#[allow(clippy::too_many_arguments)]
fn graph_node(
    commands: &mut Commands,
    fonts: &EmberFonts,
    canvas: Entity,
    viewport: Entity,
    title: &str,
    inputs: &[&str],
    outputs: &[&str],
    x: f32,
    y: f32,
) -> (Entity, Vec<Entity>, Vec<Entity>) {
    let node = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(NODE_W),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            BorderColor::all(rgb(tree_line())),
            NgPart::Node,
            Interaction::default(),
            GraphNode { canvas },
            GlobalZIndex(5),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Move),
            Name::new("graph-node"),
        ))
        .id();
    let title_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(HEAD_H),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::top(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            NgPart::Header,
            bevy::ui::FocusPolicy::Pass,
            Name::new("node-title"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(title),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_primary())),
            ));
        })
        .id();
    commands.entity(node).add_child(title_bar);

    let mut in_ports = Vec::new();
    let mut out_ports = Vec::new();
    let mut row = 0usize;
    for name in inputs {
        let cy = HEAD_H + row as f32 * ROW_H + ROW_H / 2.0;
        let r = graph_row(commands, fonts, name, false);
        let port = port_dot(commands, viewport, Vec2::new(0.0, cy), false);
        commands.entity(node).add_children(&[r, port]);
        in_ports.push(port);
        row += 1;
    }
    for name in outputs {
        let cy = HEAD_H + row as f32 * ROW_H + ROW_H / 2.0;
        let r = graph_row(commands, fonts, name, true);
        let port = port_dot(commands, viewport, Vec2::new(NODE_W, cy), true);
        commands.entity(node).add_children(&[r, port]);
        out_ports.push(port);
        row += 1;
    }
    (node, in_ports, out_ports)
}

fn graph_row(commands: &mut Commands, fonts: &EmberFonts, name: &str, output: bool) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(ROW_H),
                align_items: AlignItems::Center,
                justify_content: if output {
                    JustifyContent::FlexEnd
                } else {
                    JustifyContent::FlexStart
                },
                padding: if output {
                    UiRect::right(Val::Px(12.0))
                } else {
                    UiRect::left(Val::Px(12.0))
                },
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            Name::new("node-row"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(name),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_muted())),
            ));
        })
        .id()
}

fn port_dot(commands: &mut Commands, viewport: Entity, offset: Vec2, is_output: bool) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(offset.x - 5.0),
                top: Val::Px(offset.y - 5.0),
                width: Val::Px(10.0),
                height: Val::Px(10.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(accent())),
            NgPart::Port,
            Interaction::default(),
            GraphPort {
                viewport,
                is_output,
            },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Crosshair),
            Name::new("graph-port"),
        ))
        .id()
}

fn make_wire(commands: &mut Commands, viewport: Entity, from: Entity, to: Entity) {
    let cable = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GraphWire { from, to, viewport },
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
            GlobalZIndex(1),
            Name::new("cable"),
        ))
        .id();
    commands.entity(viewport).add_child(cable);
}

/// Give freshly-spawned cables a `CableMaterial` (needs `Assets` access).
fn cable_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<CableMaterial>>,
    cables: Query<Entity, (With<GraphWire>, Without<MaterialNode<CableMaterial>>)>,
) {
    for e in &cables {
        let handle = materials.add(CableMaterial::default());
        commands.entity(e).insert(MaterialNode(handle));
    }
}

pub(crate) fn graph_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: Local<Option<(Entity, Vec2)>>,
    picks: Query<(Entity, &Interaction, &GraphNode)>,
    views: Query<&GraphView>,
    mut nodes: Query<&mut Node, With<GraphNode>>,
) {
    if active.is_none() {
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(c) = cursor(&windows) {
                for (e, interaction, _) in &picks {
                    if *interaction == Interaction::Pressed {
                        *active = Some((e, c));
                        break;
                    }
                }
            }
        }
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *active = None;
        return;
    }
    let (Some((node, last)), Some(c)) = (*active, cursor(&windows)) else {
        return;
    };
    let delta = c - last;
    *active = Some((node, c));
    let zoom = picks
        .get(node)
        .ok()
        .and_then(|(_, _, gn)| views.get(gn.canvas).ok())
        .map(|v| v.zoom)
        .unwrap_or(1.0);
    if let Ok(mut n) = nodes.get_mut(node) {
        n.left = Val::Px(px(n.left) + delta.x / zoom);
        n.top = Val::Px(px(n.top) + delta.y / zoom);
    }
}

pub(crate) fn graph_pan(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut last: Local<Option<Vec2>>,
    mut canvases: Query<(&RelativeCursorPosition, &mut GraphView, &mut UiTransform), With<GraphPan>>,
) {
    if !mouse.pressed(MouseButton::Middle) {
        *last = None;
        return;
    }
    let Some(c) = cursor(&windows) else {
        return;
    };
    if let Some(prev) = *last {
        let delta = c - prev;
        for (rcp, mut view, mut tf) in &mut canvases {
            if rcp.cursor_over {
                view.pan += delta;
                tf.translation = Val2::px(view.pan.x, view.pan.y);
                break;
            }
        }
    }
    *last = Some(c);
}

/// Cursor-anchored zoom: scaling the canvas around its centre, then adjusting the
/// pan so the canvas point under the cursor stays put. `q` is the cursor offset
/// from the viewport centre (logical px); `pan' = pan*r + q*(1-r)` keeps it fixed.
pub(crate) fn graph_zoom(
    mut wheel: MessageReader<MouseWheel>,
    mut canvases: Query<(&mut GraphView, &mut UiTransform, &ChildOf), With<GraphPan>>,
    viewports: Query<(&RelativeCursorPosition, &ComputedNode)>,
    over_overlay: Option<Res<crate::widgets::popup::PointerOverOverlay>>,
) {
    // Don't zoom while an overlay (e.g. the add-node menu) is under the cursor —
    // that wheel scrolls the overlay, not the graph.
    if over_overlay.is_some_and(|o| o.0) {
        return;
    }
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    for (mut view, mut tf, child_of) in &mut canvases {
        let Ok((rcp, cn)) = viewports.get(child_of.parent()) else {
            continue;
        };
        if !rcp.cursor_over {
            continue;
        }
        let old = view.zoom;
        let new = (old * (1.0 + dy * 0.12)).clamp(0.4, 2.5);
        if (new - old).abs() < 1e-5 {
            continue;
        }
        let r = new / old;
        let size = cn.size() * cn.inverse_scale_factor(); // logical px
        let q = rcp.normalized.unwrap_or(Vec2::ZERO) * size; // cursor − centre
        view.pan = view.pan * r + q * (1.0 - r);
        view.zoom = new;
        tf.translation = Val2::px(view.pan.x, view.pan.y);
        tf.scale = Vec2::splat(new);
    }
}

pub(crate) fn graph_connect(
    mut pending: Local<Option<Entity>>,
    pressed: Query<(Entity, &Interaction, &GraphPort), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (entity, interaction, port) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if port.is_output {
            *pending = Some(entity);
        } else if let Some(out) = pending.take() {
            make_wire(&mut commands, port.viewport, out, entity);
        }
    }
}

pub(crate) fn graph_remove(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    blockers: Query<&Interaction, Or<(With<GraphNode>, With<GraphPort>)>>,
    wires: Query<(Entity, &GraphWire)>,
    transforms: Query<&UiGlobalTransform>,
    computeds: Query<&ComputedNode>,
    mut commands: Commands,
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
    let mut best: Option<(Entity, f32)> = None;
    for (we, w) in &wires {
        let (Ok(fgt), Ok(tgt), Ok(vgt), Ok(vcn)) = (
            transforms.get(w.from),
            transforms.get(w.to),
            transforms.get(w.viewport),
            computeds.get(w.viewport),
        ) else {
            continue;
        };
        let isf = vcn.inverse_scale_factor();
        let top_left = vgt.translation - vcn.size() * 0.5;
        let cur = cl / isf - top_left;
        let p0 = fgt.translation - top_left;
        let p3 = tgt.translation - top_left;
        let (p1, p2) = control_points(p0, p3);
        let mut d = f32::MAX;
        let mut prev = p0;
        for i in 1..=24 {
            let pt = bezier(p0, p1, p2, p3, i as f32 / 24.0);
            d = d.min(seg_dist(cur, prev, pt));
            prev = pt;
        }
        let thresh = 8.0 / isf;
        if d < thresh && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((we, d));
        }
    }
    if let Some((we, _)) = best {
        commands.entity(we).despawn();
    }
}

pub(crate) fn update_endpoints(
    mut materials: ResMut<Assets<CableMaterial>>,
    wires: Query<(&GraphWire, &MaterialNode<CableMaterial>)>,
    transforms: Query<&UiGlobalTransform>,
    computeds: Query<&ComputedNode>,
) {
    let accent = rgb(accent()).to_linear();
    for (w, mat) in &wires {
        let (Ok(fgt), Ok(tgt), Ok(vgt), Ok(vcn)) = (
            transforms.get(w.from),
            transforms.get(w.to),
            transforms.get(w.viewport),
            computeds.get(w.viewport),
        ) else {
            continue;
        };
        let isf = vcn.inverse_scale_factor();
        let top_left = vgt.translation - vcn.size() * 0.5;
        let p0 = fgt.translation - top_left;
        let p3 = tgt.translation - top_left;
        let (p1, p2) = control_points(p0, p3);
        if let Some(m) = materials.get_mut(&mat.0) {
            m.ab = Vec4::new(p0.x, p0.y, p1.x, p1.y);
            m.cd = Vec4::new(p2.x, p2.y, p3.x, p3.y);
            m.color = Vec4::new(accent.red, accent.green, accent.blue, 1.0);
            // Stroke width / feather in physical px (constant logical thickness).
            m.params = Vec4::new(WIRE_W / isf, 1.0, 0.0, 0.0);
        }
    }
}
