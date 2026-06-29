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

use std::collections::{HashMap, HashSet};

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, Outline, RelativeCursorPosition, UiGlobalTransform, UiTransform, Val2};
use bevy::ui_render::prelude::MaterialNode;
use bevy::window::SystemCursorIcon;

use super::{grid_node, CableMaterial, GraphPan, GraphView};
use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::*;
use crate::widgets::text_input::{text_input, EmberTextInput};

const NODE_W: f32 = 160.0;
const HEAD_H: f32 = 26.0;
const ROW_H: f32 = 22.0;
const WIRE_W: f32 = 2.5;
/// Width of a port's connection grab overlay (the interactive slot holding the
/// dot). Kept tight to the dot so the rest of the label is free to drag the node;
/// it's an absolute overlay, so it never shifts the label.
const SLOT_W: f32 = 30.0;
/// Base `GlobalZIndex` for nodes; the selected node is bumped to `NODE_Z + 1` so
/// it draws and picks above overlapping peers (see [`ngv_apply_selection`]).
const NODE_Z: i32 = 5;

/// Registers the data-driven node-graph view systems.
pub(crate) struct NodeGraphViewPlugin;

impl Plugin for NodeGraphViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (ngv_cable_attach, ngv_drag, ngv_connect, ngv_box, ngv_apply_selection, ngv_keys, ngv_port_rmb, ngv_preview, ngv_view_ops, ngv_highlight_slots, ngv_context, ngv_open_key));
        app.add_systems(Update, (ngv_comment_drag, ngv_comment_resize, ngv_comment_title, ngv_comment_delete, ngv_comment_key));
        app.add_systems(PostUpdate, ngv_endpoints.after(bevy::ui::UiSystems::Layout));
    }
}

/// A user edit the view recorded; the caller drains [`NodeGraphView::pending`]
/// and applies these to its own graph model.
pub enum GraphEdit {
    NodeMoved { id: u64, x: f32, y: f32 },
    Connect { from_node: u64, from_pin: String, to_node: u64, to_pin: String },
    Disconnect { from_node: u64, from_pin: String, to_node: u64, to_pin: String },
    Delete { id: u64 },
    /// Primary selection (for the caller's inspector). The widget owns the full
    /// (multi-) selection set itself; this just reports the focused node.
    Select { id: Option<u64> },
    /// Create a comment / group box covering `rect` (`[x, y, w, h]` canvas px).
    /// The caller assigns the id (and persists it).
    AddComment { rect: [f32; 4] },
    /// A comment box was dragged (its own top-left moved; contained nodes report
    /// their own `NodeMoved`).
    CommentMoved { id: u64, x: f32, y: f32 },
    CommentResized { id: u64, w: f32, h: f32 },
    CommentRetitled { id: u64, text: String },
    DeleteComment { id: u64 },
}

/// Lives on the graph viewport; the caller syncs by draining `pending`. The
/// widget owns all interaction — selection, drag, connect, delete, view ops — so
/// any graph editor reuses it by feeding nodes/wires and draining `pending`.
#[derive(Component, Default)]
pub struct NodeGraphView {
    pub pending: Vec<GraphEdit>,
    /// The (multi-) selection set — drives node borders in place (no rebuild, so
    /// a drag-started selection never kills the drag).
    pub selected: HashSet<u64>,
    /// The port a cable is being dragged from: `(node, pin, is_output, colour)`.
    /// Releasing over an opposite-direction, colour-matching port completes it.
    pub pending_connect: Option<(u64, String, bool, (u8, u8, u8))>,
    /// Caller sets to re-frame all nodes; cleared by the widget once applied.
    pub fit_request: bool,
    /// Caller sets to recenter (keep zoom); cleared once applied.
    pub center_request: bool,
    /// Caller sets to multiply the zoom (toolbar +/−); cleared once applied.
    pub zoom_request: Option<f32>,
    /// Set by the widget on right-click over empty canvas: `(screen_pos,
    /// canvas_pos)`. The caller opens its add-node menu at `screen_pos`, spawns
    /// new nodes at `canvas_pos`, and clears this.
    pub context_menu: Option<(Vec2, Vec2)>,
    /// Set by the widget when a cable is dragged from a pin and released on empty
    /// canvas. The caller opens its add-node menu at `screen`, spawns the chosen
    /// node at `canvas`, and auto-wires it to the dragged pin (`node`/`pin`/
    /// `is_output`/`color`). Cleared by the caller.
    pub connect_drag: Option<ConnectDrag>,
    /// A "parked" cable left visibly connecting the source pin to the spot where
    /// the connect-drag menu opened, so the cable doesn't vanish while the user
    /// picks a node. The widget draws it and clears it on the next click (a menu
    /// pick or a dismiss).
    pub(crate) frozen_cable: Option<FrozenCable>,
}

/// A cable parked from a pin to a fixed screen point (the open connect-drag menu),
/// keeping it visible until the user clicks. See [`NodeGraphView::frozen_cable`].
/// The cable's colour is resolved live from the pin, so it isn't stored here.
pub(crate) struct FrozenCable {
    node: u64,
    pin: String,
    is_output: bool,
    /// Logical-screen point the cable runs to (where the menu opened).
    screen: Vec2,
}

/// A cable dragged from a pin and dropped on empty canvas — the caller opens its
/// node palette here and wires the new node back to this source pin.
pub struct ConnectDrag {
    pub screen: Vec2,
    pub canvas: Vec2,
    pub node: u64,
    pub pin: String,
    pub is_output: bool,
    pub color: (u8, u8, u8),
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
    viewport: Entity,
    /// The port's label, whose background is tinted while the grab slot is hovered
    /// (sized to the text, so the highlight tracks the name, not the whole row).
    label: Entity,
}
#[derive(Component)]
struct NgvWire {
    from_node: u64,
    from_pin: String,
    to_node: u64,
    to_pin: String,
    viewport: Entity,
}
/// The small visual dot inside a port row (the connection target); enlarged
/// while connecting / hovered by [`ngv_highlight_slots`].
#[derive(Component)]
struct NgvPortDot {
    is_output: bool,
}
/// The rubber-band selection rectangle (spawned while box-selecting).
#[derive(Component)]
struct NgvBoxRect;
/// The temporary cable drawn from a pending output port to the cursor.
#[derive(Component)]
struct NgvPreview {
    viewport: Entity,
}
/// An inline value-editor row on a node. Marked so node-drag / box-select bail
/// when the press is over it (otherwise scrubbing a field would drag the node).
#[derive(Component)]
struct NgvInputEditor;

/// A comment / group box (the back-layer rectangle). Dragging its body moves the
/// box and every node whose centre it encloses.
#[derive(Component)]
struct NgvComment {
    id: u64,
    canvas: Entity,
    viewport: Entity,
}
/// The editable title field in a comment header.
#[derive(Component)]
struct NgvCommentTitle {
    id: u64,
    viewport: Entity,
}
/// The bottom-right resize grip of a comment box.
#[derive(Component)]
struct NgvCommentResize {
    canvas: Entity,
    viewport: Entity,
    comment: Entity,
    id: u64,
}
/// The ✕ button in a comment header.
#[derive(Component)]
struct NgvCommentDelete {
    id: u64,
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
    // Live connection preview cable (hidden until dragging from an output port).
    let preview = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() },
            NgvPreview { viewport },
            bevy::ui::FocusPolicy::Pass,
            Pickable::IGNORE,
            GlobalZIndex(2),
            Name::new("ngv-preview"),
        ))
        .id();
    commands.entity(viewport).add_children(&[grid, canvas, preview]);
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
    thumbnail: Option<Handle<Image>>,
    // Optional inline value editor entity per input (index-aligned with `inputs`);
    // rendered on its own row under the pin. Pass `&[]` for none.
    input_editors: &[Option<Entity>],
    // Optional widget mounted in the title bar in place of the title text (e.g. a
    // dropdown that switches the node's type). Pass `None` for a plain title.
    header_control: Option<Entity>,
) -> Entity {
    let node = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                // Grow to fit content (e.g. a Vec3 inline editor wider than the
                // base width) so editors never overflow the node's sides; plain
                // nodes stay at the base width.
                min_width: Val::Px(NODE_W),
                flex_direction: FlexDirection::Column,
                // Constant border: selection is drawn as an `Outline` (below), which
                // doesn't affect layout — so selecting never nudges the node.
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            BorderColor::all(rgb(tree_line())),
            Outline {
                width: Val::Px(2.0),
                offset: Val::Px(1.0),
                color: if selected { rgb(accent()) } else { Color::NONE },
            },
            NgvNode { id: node_id, canvas, viewport },
            Interaction::default(),
            RelativeCursorPosition::default(),
            GlobalZIndex(NODE_Z),
            // Drag-to-move node → the 4-arrow Move cursor.
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Move),
            Name::new("ngv-node"),
        ))
        .id();
    let title_bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(HEAD_H), align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(8.0)), border_radius: BorderRadius::top(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(color)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("ngv-node-title"),
        ))
        .id();
    // The title fills the header and is FocusPolicy::Pass, so pressing it drags the
    // node (the press falls through to the node root). An optional caller control
    // (e.g. a type switcher) sits at the trailing edge and captures its own clicks,
    // so it never steals a header drag.
    let label = commands
        .spawn((
            Text::new(title.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(on_accent())),
            bevy::text::TextLayout::no_wrap(),
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(title_bar).add_child(label);
    if let Some(ctrl) = header_control {
        commands.entity(title_bar).add_child(ctrl);
    }
    commands.entity(node).add_child(title_bar);

    for (idx, (pin, label, pin_color)) in inputs.iter().enumerate() {
        let r = port_row(commands, fonts, node_id, viewport, pin, label, false, *pin_color);
        commands.entity(node).add_child(r);
        // Inline value editor for an unconnected input, on its own row.
        if let Some(editor) = input_editors.get(idx).copied().flatten() {
            let erow = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(0.0), Val::Px(3.0)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::FlexEnd,
                        ..default()
                    },
                    // Capture the pointer (don't Pass) + mark so node-drag/box-select
                    // ignore presses here — scrubbing a field must not drag the node.
                    RelativeCursorPosition::default(),
                    NgvInputEditor,
                    Name::new("ngv-input-editor"),
                ))
                .id();
            commands.entity(erow).add_child(editor);
            commands.entity(node).add_child(erow);
        }
    }
    for (pin, label, pin_color) in outputs {
        let r = port_row(commands, fonts, node_id, viewport, pin, label, true, *pin_color);
        commands.entity(node).add_child(r);
    }
    // Optional preview thumbnail (e.g. texture nodes). Output pins hug the right
    // edge, so the left of the node below the inputs is dead space — drop the
    // preview there (absolutely positioned) instead of adding a square row at the
    // bottom. Kept SQUARE: the texture is square, so a square box fills edge to
    // edge (a taller box letterboxes the image, leaving top/bottom gaps). Width
    // is held clear of the right-aligned output labels.
    if let Some(img) = thumbnail {
        let body_top = HEAD_H + inputs.len() as f32 * ROW_H + 2.0;
        let size = NODE_W - 70.0;
        // The preview is absolute (out of flow), so a node with few output rows
        // wouldn't grow to contain it and the image would spill past the bottom.
        // Reserve the shortfall with an in-flow spacer — zero for tall nodes whose
        // pins already exceed the preview (e.g. the 6-output texture node).
        let reserve = (body_top + size + 8.0 - (HEAD_H + (inputs.len() + outputs.len()) as f32 * ROW_H)).max(0.0);
        if reserve > 0.0 {
            let spacer = commands
                .spawn(Node { width: Val::Percent(100.0), height: Val::Px(reserve), ..default() })
                .id();
            commands.entity(node).add_child(spacer);
        }
        let thumb = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    top: Val::Px(body_top),
                    width: Val::Px(size),
                    height: Val::Px(size),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                ImageNode::new(img),
                BorderColor::all(rgb(tree_line())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("ngv-node-thumb"),
            ))
            .id();
        commands.entity(node).add_child(thumb);
    }
    node
}

/// A pin row: a click-through label, plus a **connection slot** (the interactive
/// [`NgvPort`], holding the dot) that's an *absolute overlay* over the dot + the
/// start of the label — so grabbing a cable works right where the dot and its name
/// are, with no layout gap. Being overlaid (not in flow) means it never shifts the
/// label. Its trailing portion of the row stays click-through, so the node still
/// drags from there. The slot hugs the edge, so `port_map`'s `centre ± width/2`
/// lands the cable endpoint at the node edge. Hovering the slot tints the row (see
/// [`ngv_highlight_slots`]) to advertise the grab.
#[allow(clippy::too_many_arguments)]
fn port_row(commands: &mut Commands, fonts: &EmberFonts, node_id: u64, viewport: Entity, pin: &str, label: &str, is_output: bool, color: (u8, u8, u8)) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(ROW_H),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: if is_output { JustifyContent::FlexEnd } else { JustifyContent::FlexStart },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            bevy::ui::FocusPolicy::Pass,
            Name::new("ngv-port-row"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            // Sized to the text; its background is tinted on grab-hover, and the
            // margin clears the dot at the node edge.
            Node {
                margin: if is_output { UiRect::right(Val::Px(14.0)) } else { UiRect::left(Val::Px(14.0)) },
                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    // The grab overlay: absolute, edge-anchored, on top of the dot + label start.
    let slot = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                height: Val::Percent(100.0),
                left: if is_output { Val::Auto } else { Val::Px(0.0) },
                right: if is_output { Val::Px(0.0) } else { Val::Auto },
                width: Val::Px(SLOT_W),
                ..default()
            },
            NgvPort { node_id, pin: pin.to_string(), is_output, color, viewport, label: text },
            Interaction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Crosshair),
            Name::new("ngv-port-slot"),
        ))
        .id();
    let dot = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: if is_output { Val::Auto } else { Val::Px(-5.0) },
                right: if is_output { Val::Px(-5.0) } else { Val::Auto },
                top: Val::Px((ROW_H - 10.0) * 0.5),
                width: Val::Px(10.0),
                height: Val::Px(10.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(color)),
            BorderColor::all(rgb(color)),
            NgvPortDot { is_output },
            bevy::ui::FocusPolicy::Pass,
            Name::new("ngv-port-dot"),
        ))
        .id();
    commands.entity(slot).add_child(dot);
    // Slot is added last so it overlays the label and captures the grab.
    commands.entity(row).add_children(&[text, slot]);
    row
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

/// Build a comment / group box at `rect` (`[x, y, w, h]` canvas px) with an
/// editable `title` and `color` tint. Mount it into `handle.canvas` (ideally in a
/// layer behind the nodes). A translucent body sits behind the nodes (low
/// z-index); the header drags the box + its enclosed nodes, the corner grip
/// resizes it, and the ✕ deletes it.
#[allow(clippy::too_many_arguments)]
pub fn graph_comment_view(
    commands: &mut Commands,
    fonts: &EmberFonts,
    canvas: Entity,
    viewport: Entity,
    id: u64,
    title: &str,
    rect: [f32; 4],
    color: (u8, u8, u8),
) -> Entity {
    let tint = Color::srgba(color.0 as f32 / 255.0, color.1 as f32 / 255.0, color.2 as f32 / 255.0, 0.10);
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(rect[0]),
                top: Val::Px(rect[1]),
                width: Val::Px(rect[2].max(80.0)),
                height: Val::Px(rect[3].max(60.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(tint),
            BorderColor::all(rgb(color)),
            // Behind nodes (z 5) but above the grid (z 0).
            GlobalZIndex(1),
            NgvComment { id, canvas, viewport },
            Interaction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Move),
            Name::new("ngv-comment"),
        ))
        .id();

    // Header: editable title (left, grows) + ✕ delete (right).
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                border_radius: BorderRadius::top(Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(color).with_alpha(0.85)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("ngv-comment-header"),
        ))
        .id();
    let title_input = text_input(commands, &fonts.ui, "Comment", title);
    commands.entity(title_input).insert((
        NgvCommentTitle { id, viewport },
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::NONE),
    ));
    let del = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            NgvCommentDelete { id, viewport },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("ngv-comment-delete"),
        ))
        .id();
    let x_icon = icon_text(commands, &fonts.phosphor, "x", on_accent(), 11.0);
    commands.entity(del).add_child(x_icon);
    commands.entity(header).add_children(&[title_input, del]);

    // Bottom-right resize grip.
    let grip = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(color).with_alpha(0.6)),
            Interaction::default(),
            NgvCommentResize { canvas, viewport, comment: root, id },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::NwseResize),
            Name::new("ngv-comment-resize"),
        ))
        .id();

    commands.entity(root).add_children(&[header, grip]);
    root
}

// ── Systems ──────────────────────────────────────────────────────────────────

#[allow(clippy::type_complexity)]
fn ngv_cable_attach(mut commands: Commands, mut materials: ResMut<Assets<CableMaterial>>, cables: Query<Entity, (Or<(With<NgvWire>, With<NgvPreview>)>, Without<MaterialNode<CableMaterial>>)>) {
    for e in &cables {
        let handle = materials.add(CableMaterial::default());
        // try_insert: the wire entity may be despawned this same frame (panel teardown).
        commands.entity(e).try_insert(MaterialNode(handle));
    }
}

/// Right-click a port → disconnect every wire on it (egui parity).
fn ngv_port_rmb(mouse: Res<ButtonInput<MouseButton>>, ports: Query<(&Interaction, &NgvPort)>, wires: Query<&NgvWire>, mut graphs: Query<&mut NodeGraphView>) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some((_, port)) = ports.iter().find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed)) else { return };
    let Ok(mut g) = graphs.get_mut(port.viewport) else { return };
    for w in &wires {
        if w.viewport != port.viewport {
            continue;
        }
        let hit = if port.is_output {
            w.from_node == port.node_id && w.from_pin == port.pin
        } else {
            w.to_node == port.node_id && w.to_pin == port.pin
        };
        if hit {
            g.pending.push(GraphEdit::Disconnect { from_node: w.from_node, from_pin: w.from_pin.clone(), to_node: w.to_node, to_pin: w.to_pin.clone() });
        }
    }
}

/// Draw the live cable: while dragging from a port, from it to the cursor; and
/// while a connect-drag menu is open (`frozen_cable`), from the source pin to the
/// menu's anchor so the cable stays connected instead of blinking out.
#[allow(clippy::type_complexity)]
fn ngv_preview(
    windows: Query<&Window>,
    mut materials: ResMut<Assets<CableMaterial>>,
    graphs: Query<&NodeGraphView>,
    ports: Query<(&NgvPort, &UiGlobalTransform, &ComputedNode)>,
    transforms: Query<&UiGlobalTransform>,
    computeds: Query<&ComputedNode>,
    mut previews: Query<(&NgvPreview, &mut Node, &MaterialNode<CableMaterial>)>,
) {
    if previews.is_empty() {
        return;
    }
    let map = port_map(&ports);
    let cur = cursor(&windows);
    for (pv, mut node, mat) in &mut previews {
        // Live drag follows the cursor; otherwise a parked (frozen) cable runs to
        // the open menu's anchor.
        let endpoint = graphs.get(pv.viewport).ok().and_then(|g| {
            if let Some((nid, pin, is_out, _)) = g.pending_connect.clone() {
                cur.map(|c| (nid, pin, is_out, c))
            } else if let Some(f) = g.frozen_cable.as_ref() {
                Some((f.node, f.pin.clone(), f.is_output, f.screen))
            } else {
                None
            }
        });
        let Some((nid, pin, is_out, c)) = endpoint else {
            if node.display != Display::None {
                node.display = Display::None;
            }
            continue;
        };
        let (Some(&(p0, col)), Ok(vgt), Ok(vcn)) = (map.get(&(nid, pin, is_out)), transforms.get(pv.viewport), computeds.get(pv.viewport)) else {
            node.display = Display::None;
            continue;
        };
        let isf = vcn.inverse_scale_factor();
        let top_left = vgt.translation - vcn.size() * 0.5;
        let a = p0 - top_left;
        let b = c / isf - top_left;
        let (c1, c2) = control_points(a, b);
        let lin = rgb(col).to_linear();
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.ab = Vec4::new(a.x, a.y, c1.x, c1.y);
            m.cd = Vec4::new(c2.x, c2.y, b.x, b.y);
            m.color = Vec4::new(lin.red, lin.green, lin.blue, 0.7);
            m.params = Vec4::new(WIRE_W / isf, 1.0, 0.0, 0.0);
        }
        node.display = Display::Flex;
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

/// Press a node → select it (Ctrl toggles/extends, plain click replaces); drag →
/// move every selected node together; on release record `NodeMoved` for each.
/// Selecting in place (via `NodeGraphView.selected`) never rebuilds a node, so
/// the drag survives.
#[allow(clippy::type_complexity)]
fn ngv_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    mut active: Local<Option<(Vec2, bool, Entity, Entity)>>, // (last, moved, viewport, canvas)
    node_picks: Query<(&Interaction, &NgvNode)>,
    port_picks: Query<&Interaction, With<NgvPort>>,
    views: Query<&GraphView>,
    mut nodes: Query<(&NgvNode, &mut Node)>,
    mut graphs: Query<&mut NodeGraphView>,
    editors: Query<&RelativeCursorPosition, With<NgvInputEditor>>,
) {
    if active.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        if port_picks.iter().any(|i| *i == Interaction::Pressed) {
            return; // a port press is a connect, not a drag
        }
        if editors.iter().any(|r| r.cursor_over) {
            return; // pressing an inline value editor must not drag the node
        }
        let Some(c) = cursor(&windows) else { return };
        let Some((_, n)) = node_picks.iter().find(|(i, _)| **i == Interaction::Pressed) else { return };
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if let Ok(mut g) = graphs.get_mut(n.viewport) {
            if ctrl {
                if !g.selected.remove(&n.id) {
                    g.selected.insert(n.id);
                }
            } else if !g.selected.contains(&n.id) {
                g.selected.clear();
                g.selected.insert(n.id);
            }
            let prim = if g.selected.contains(&n.id) { Some(n.id) } else { g.selected.iter().copied().next() };
            g.pending.push(GraphEdit::Select { id: prim });
        }
        *active = Some((c, false, n.viewport, n.canvas));
        return;
    }

    if !mouse.pressed(MouseButton::Left) {
        if let Some((_, moved, vp, _)) = *active {
            if moved {
                let sel: HashSet<u64> = graphs.get(vp).map(|g| g.selected.clone()).unwrap_or_default();
                let moves: Vec<(u64, f32, f32)> = nodes
                    .iter()
                    .filter(|(n, _)| n.viewport == vp && sel.contains(&n.id))
                    .map(|(n, node)| (n.id, px(node.left), px(node.top)))
                    .collect();
                if let Ok(mut g) = graphs.get_mut(vp) {
                    for (id, x, y) in moves {
                        g.pending.push(GraphEdit::NodeMoved { id, x, y });
                    }
                }
            }
        }
        *active = None;
        return;
    }

    let (Some((last, _, vp, canvas)), Some(c)) = (*active, cursor(&windows)) else {
        return;
    };
    let delta = c - last;
    if delta == Vec2::ZERO {
        return;
    }
    let zoom = views.get(canvas).map(|v| v.zoom).unwrap_or(1.0);
    let sel: HashSet<u64> = graphs.get(vp).map(|g| g.selected.clone()).unwrap_or_default();
    for (n, mut node) in &mut nodes {
        if n.viewport == vp && sel.contains(&n.id) {
            node.left = Val::Px(px(node.left) + delta.x / zoom);
            node.top = Val::Px(px(node.top) + delta.y / zoom);
        }
    }
    *active = Some((c, true, vp, canvas));
}

/// Drive each node's border (width + colour) from its viewport's `selected` id,
/// in place — so selecting a node never rebuilds it (which would kill an
/// in-progress drag). Only writes when the selection state actually flips.
///
/// Also raises a selected node's `GlobalZIndex` above its peers so it draws (and
/// picks) on top — without this, clicking where two nodes overlap can land on the
/// one behind. Base nodes sit at `NODE_Z`; the selected one at `NODE_Z + 1`.
fn ngv_apply_selection(views: Query<&NodeGraphView>, mut nodes: Query<(&NgvNode, &mut Outline, &mut GlobalZIndex)>) {
    for (n, mut outline, mut z) in &mut nodes {
        let sel = views.get(n.viewport).map(|v| v.selected.contains(&n.id)).unwrap_or(false);
        let want = if sel { rgb(accent()) } else { Color::NONE };
        if outline.color != want {
            outline.color = want;
        }
        let want_z = if sel { NODE_Z + 1 } else { NODE_Z };
        if z.0 != want_z {
            z.0 = want_z;
        }
    }
}

/// Drag-to-connect: press an output port to start (pending lives on the view so
/// the live preview + Esc see it), then **release** over an input port to record
/// the `Connect`. Releasing elsewhere cancels.
#[allow(clippy::type_complexity)]
fn ngv_connect(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    ports: Query<(&Interaction, &NgvPort)>,
    mut graphs: Query<(Entity, &mut NodeGraphView, &RelativeCursorPosition, &ComputedNode, &Children)>,
    views: Query<&GraphView>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        // A new click ends any parked connect-drag cable (the user is picking a
        // menu node or dismissing the menu).
        for (_, mut g, _, _, _) in &mut graphs {
            if g.frozen_cable.is_some() {
                g.frozen_cable = None;
            }
        }
        if let Some((_, p)) = ports.iter().find(|(i, _)| **i == Interaction::Pressed) {
            if let Ok((_, mut g, _, _, _)) = graphs.get_mut(p.viewport) {
                g.pending_connect = Some((p.node_id, p.pin.clone(), p.is_output, p.color));
            }
        }
    }
    if mouse.just_released(MouseButton::Left) {
        let target = ports
            .iter()
            .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed))
            .map(|(_, p)| (p.viewport, p.node_id, p.pin.clone(), p.is_output, p.color));
        let cur = cursor(&windows);
        for (vp, mut g, rcp, vcn, children) in &mut graphs {
            let Some((snode, spin, s_out, scol)) = g.pending_connect.take() else { continue };
            // 1) Released over a compatible port → make the connection.
            if let Some((tvp, tnode, tpin, t_out, tcol)) = &target {
                if *tvp == vp && *t_out != s_out && *tcol == scol && *tnode != snode {
                    let (from_node, from_pin, to_node, to_pin) = if s_out {
                        (snode, spin, *tnode, tpin.clone())
                    } else {
                        (*tnode, tpin.clone(), snode, spin)
                    };
                    g.pending.push(GraphEdit::Connect { from_node, from_pin, to_node, to_pin });
                    continue;
                }
            }
            // 2) Released on empty canvas (no port under the cursor) → open the
            //    add-node palette so the caller can spawn + auto-wire a node.
            if target.is_none() && rcp.cursor_over {
                let Some(screen) = cur else { continue };
                let (pan, zoom) = children.iter().find_map(|ch| views.get(ch).ok()).map(|v| (v.pan, v.zoom)).unwrap_or((Vec2::ZERO, 1.0));
                let size = vcn.size() * vcn.inverse_scale_factor();
                let center = size * 0.5;
                let canvas = center + (rcp.normalized.unwrap_or(Vec2::ZERO) * size - pan) / zoom.max(0.01);
                g.connect_drag = Some(ConnectDrag { screen, canvas, node: snode, pin: spin.clone(), is_output: s_out, color: scol });
                // Park the cable visibly running to the menu until the next click,
                // so it doesn't blink out while the user picks a node.
                g.frozen_cable = Some(FrozenCable { node: snode, pin: spin, is_output: s_out, screen });
            }
        }
    }
}

/// While dragging a cable, show only the **valid drop slots** (opposite direction
/// with matching colour) enlarged; hide every incompatible dot. With no drag, dots
/// rest at base size and just grow on hover. Also tints the hovered port's row to
/// advertise that the area is a cable grab handle.
fn ngv_highlight_slots(graphs: Query<&NodeGraphView>, ports: Query<(&NgvPort, &Interaction, &Children)>, mut dots: Query<(&NgvPortDot, &mut Node)>, mut bgs: Query<&mut BackgroundColor>) {
    for (port, interaction, children) in &ports {
        let pending = graphs.get(port.viewport).ok().and_then(|g| g.pending_connect.clone());
        let hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        // A port is connectable in the current context: always when no cable is
        // being dragged, else only if it's a compatible drop target (opposite
        // direction, matching colour, a different node).
        let connectable = match &pending {
            Some((snode, _, s_out, scol)) => port.is_output != *s_out && port.color == *scol && port.node_id != *snode,
            None => true,
        };
        // Tint the label background (sized to the text) only when hovering a port
        // that can actually take a connection — so dragging a cable doesn't light up
        // ports you can't drop on.
        if let Ok(mut bg) = bgs.get_mut(port.label) {
            let want = if hovered && connectable { Color::srgba(1.0, 1.0, 1.0, 0.10) } else { Color::NONE };
            if bg.0 != want {
                bg.0 = want;
            }
        }
        let (visible, size) = match &pending {
            Some((snode, spin, s_out, scol)) => {
                let is_source = port.node_id == *snode && &port.pin == spin && port.is_output == *s_out;
                let valid = port.is_output != *s_out && port.color == *scol && port.node_id != *snode;
                if is_source {
                    (true, 13.0)
                } else if valid {
                    (true, if hovered { 16.0 } else { 13.0 })
                } else {
                    (false, 10.0)
                }
            }
            None => (true, if hovered { 13.0 } else { 10.0 }),
        };
        for &c in children {
            if let Ok((dot, mut node)) = dots.get_mut(c) {
                let disp = if visible { Display::Flex } else { Display::None };
                if node.display != disp {
                    node.display = disp;
                }
                let want = Val::Px(size);
                if node.width != want {
                    node.width = want;
                    node.height = want;
                    node.top = Val::Px((ROW_H - size) * 0.5);
                    let off = Val::Px(-size * 0.5);
                    if dot.is_output {
                        node.right = off;
                    } else {
                        node.left = off;
                    }
                }
            }
        }
    }
}

/// Right-*click* (press + release without a drag) over empty canvas → record
/// `(screen, canvas)` for the caller's add-node menu. A right-*drag* pans instead
/// (see [`super::graph_pan`]), so we open the menu on release only when the cursor
/// barely moved. (Right-click on a port is handled by [`ngv_port_rmb`].)
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn ngv_context(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    node_blockers: Query<&Interaction, With<NgvNode>>,
    port_blockers: Query<&Interaction, With<NgvPort>>,
    comment_blockers: Query<&Interaction, Or<(With<NgvComment>, With<NgvCommentTitle>, With<NgvCommentResize>, With<NgvCommentDelete>)>>,
    mut graphs: Query<(&mut NodeGraphView, &RelativeCursorPosition, &ComputedNode, &Children)>,
    views: Query<&GraphView>,
    mut press: Local<Option<Vec2>>,
) {
    if mouse.just_pressed(MouseButton::Right) {
        *press = cursor(&windows);
        return;
    }
    if !mouse.just_released(MouseButton::Right) {
        return;
    }
    let Some(start) = press.take() else { return };
    let Some(c) = cursor(&windows) else { return };
    // Moved more than a few px → that was a pan, not a context click.
    if (c - start).length() > 5.0 {
        return;
    }
    let hot = |i: &Interaction| matches!(i, Interaction::Hovered | Interaction::Pressed);
    if node_blockers.iter().any(hot) || port_blockers.iter().any(hot) || comment_blockers.iter().any(hot) {
        return;
    }
    for (mut g, rcp, vcn, children) in &mut graphs {
        if !rcp.cursor_over {
            continue;
        }
        let (pan, zoom) = children.iter().find_map(|ch| views.get(ch).ok()).map(|v| (v.pan, v.zoom)).unwrap_or((Vec2::ZERO, 1.0));
        let size = vcn.size() * vcn.inverse_scale_factor();
        let center = size * 0.5;
        // screen→canvas (inverse of the canvas transform).
        let canvas = center + (rcp.normalized.unwrap_or(Vec2::ZERO) * size - pan) / zoom.max(0.01);
        g.context_menu = Some((c, canvas));
    }
}

/// Spacebar over the graph → open the add-node palette at the cursor, exactly
/// like a right-click: records `context_menu` so the caller's
/// search_menu opens there and new nodes spawn at the cursor's canvas point.
/// Ignored while a text field is focused (so typing a space in the palette's
/// own search box doesn't reopen it).
#[allow(clippy::type_complexity)]
fn ngv_open_key(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    text_inputs: Query<&crate::widgets::text_input::EmberTextInput>,
    mut graphs: Query<(&mut NodeGraphView, &RelativeCursorPosition, &ComputedNode, &Children)>,
    views: Query<&GraphView>,
) {
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }
    if text_inputs.iter().any(|t| t.focused) {
        return;
    }
    let Some(c) = cursor(&windows) else { return };
    for (mut g, rcp, vcn, children) in &mut graphs {
        if !rcp.cursor_over {
            continue;
        }
        let (pan, zoom) = children.iter().find_map(|ch| views.get(ch).ok()).map(|v| (v.pan, v.zoom)).unwrap_or((Vec2::ZERO, 1.0));
        let size = vcn.size() * vcn.inverse_scale_factor();
        let center = size * 0.5;
        let canvas = center + (rcp.normalized.unwrap_or(Vec2::ZERO) * size - pan) / zoom.max(0.01);
        g.context_menu = Some((c, canvas));
    }
}

/// Keyboard ops over the graph under the cursor: Delete/Backspace removes the
/// selection, Ctrl+A selects all, Esc cancels a pending connection then clears
/// the selection.
fn ngv_keys(keys: Res<ButtonInput<KeyCode>>, all_nodes: Query<&NgvNode>, mut graphs: Query<(Entity, &mut NodeGraphView, &RelativeCursorPosition)>) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let del = keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace);
    let all = ctrl && keys.just_pressed(KeyCode::KeyA);
    let esc = keys.just_pressed(KeyCode::Escape);
    if !del && !all && !esc {
        return;
    }
    for (vp, mut g, rcp) in &mut graphs {
        if !rcp.cursor_over {
            continue;
        }
        if del && !g.selected.is_empty() {
            let ids: Vec<u64> = g.selected.iter().copied().collect();
            for id in ids {
                g.pending.push(GraphEdit::Delete { id });
            }
            g.selected.clear();
            g.pending.push(GraphEdit::Select { id: None });
        }
        if all {
            g.selected = all_nodes.iter().filter(|n| n.viewport == vp).map(|n| n.id).collect();
        }
        if esc {
            if g.pending_connect.is_some() {
                g.pending_connect = None;
            } else if !g.selected.is_empty() {
                g.selected.clear();
                g.pending.push(GraphEdit::Select { id: None });
            }
        }
    }
}

/// Empty-canvas left interaction: drag → rubber-band box select (Ctrl extends);
/// click (no drag) over a cable → disconnect it, else clear the selection.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn ngv_box(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    mut commands: Commands,
    node_blockers: Query<&Interaction, With<NgvNode>>,
    port_blockers: Query<&Interaction, With<NgvPort>>,
    mut active: Local<Option<(Vec2, Entity, Entity, bool)>>, // (start, viewport, rect, moved)
    mut vps: Query<(Entity, &mut NodeGraphView, &RelativeCursorPosition, &UiGlobalTransform, &ComputedNode)>,
    node_rects: Query<(&NgvNode, &UiGlobalTransform, &ComputedNode)>,
    mut box_nodes: Query<&mut Node, With<NgvBoxRect>>,
    wires: Query<&NgvWire>,
    ports: Query<(&NgvPort, &UiGlobalTransform, &ComputedNode)>,
    editors: Query<&RelativeCursorPosition, With<NgvInputEditor>>,
    comment_blockers: Query<&Interaction, Or<(With<NgvComment>, With<NgvCommentTitle>, With<NgvCommentResize>, With<NgvCommentDelete>)>>,
) {
    if active.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        if node_blockers.iter().any(|i| *i == Interaction::Pressed) || port_blockers.iter().any(|i| *i == Interaction::Pressed) || comment_blockers.iter().any(|i| *i == Interaction::Pressed) {
            return; // press landed on a node/port/comment → not a box
        }
        if editors.iter().any(|r| r.cursor_over) {
            return; // pressing an inline value editor is not a box-select
        }
        let Some(c) = cursor(&windows) else { return };
        let Some(vp) = vps.iter().find(|(_, _, rcp, _, _)| rcp.cursor_over).map(|(e, _, _, _, _)| e) else { return };
        let a = rgb(accent());
        let rect = commands
            .spawn((
                Node { position_type: PositionType::Absolute, border: UiRect::all(Val::Px(1.0)), ..default() },
                BackgroundColor(a.with_alpha(0.12)),
                BorderColor::all(a),
                GlobalZIndex(10),
                bevy::ui::FocusPolicy::Pass,
                Pickable::IGNORE,
                NgvBoxRect,
                Name::new("ngv-box"),
            ))
            .id();
        commands.entity(vp).add_child(rect);
        *active = Some((c, vp, rect, false));
        return;
    }

    let (start, vp, rect, moved) = active.unwrap();
    let Some(c) = cursor(&windows) else { return };
    let Some((_, mut g, _, vgt, vcn)) = vps.iter_mut().find(|(e, _, _, _, _)| *e == vp) else {
        commands.entity(rect).try_despawn();
        *active = None;
        return;
    };
    let isf = vcn.inverse_scale_factor();
    let top_left = vgt.translation - vcn.size() * 0.5;

    if mouse.pressed(MouseButton::Left) {
        let tl_logical = top_left * isf;
        let s = start - tl_logical;
        let e = c - tl_logical;
        let mn = s.min(e);
        let sz = (e - s).abs();
        if let Ok(mut bn) = box_nodes.get_mut(rect) {
            bn.left = Val::Px(mn.x);
            bn.top = Val::Px(mn.y);
            bn.width = Val::Px(sz.x);
            bn.height = Val::Px(sz.y);
        }
        let now_moved = moved || (c - start).length() > 3.0;
        *active = Some((start, vp, rect, now_moved));
        return;
    }

    commands.entity(rect).try_despawn();
    if moved {
        let (bmin, bmax) = ((start / isf).min(c / isf), (start / isf).max(c / isf));
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if !ctrl {
            g.selected.clear();
        }
        for (n, ngt, ncn) in &node_rects {
            if n.viewport != vp {
                continue;
            }
            let half = ncn.size() * 0.5;
            let (nmin, nmax) = (ngt.translation - half, ngt.translation + half);
            if nmin.x <= bmax.x && nmax.x >= bmin.x && nmin.y <= bmax.y && nmax.y >= bmin.y {
                g.selected.insert(n.id);
            }
        }
        let prim = g.selected.iter().copied().next();
        g.pending.push(GraphEdit::Select { id: prim });
    } else {
        // A plain click: cut a cable under the cursor, else clear selection.
        let map = port_map(&ports);
        let cur = c / isf - top_left;
        let mut best: Option<(u64, String, u64, String, f32)> = None;
        for w in &wires {
            if w.viewport != vp {
                continue;
            }
            let (Some(&(p0, _)), Some(&(p3, _))) = (map.get(&(w.from_node, w.from_pin.clone(), true)), map.get(&(w.to_node, w.to_pin.clone(), false))) else {
                continue;
            };
            let (a, b) = (p0 - top_left, p3 - top_left);
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
            g.pending.push(GraphEdit::Disconnect { from_node: fnode, from_pin: fpin, to_node: tnode, to_pin: tpin });
        } else if !g.selected.is_empty() {
            g.selected.clear();
            g.pending.push(GraphEdit::Select { id: None });
        }
    }
    *active = None;
}

/// Map `(node, pin, is_output) → (cable endpoint, colour)`. The port row spans
/// the node width, so the endpoint is its outer edge (`centre.x ± width/2`), not
/// the row centre — cables attach at the dot on the node's edge.
fn port_map(ports: &Query<(&NgvPort, &UiGlobalTransform, &ComputedNode)>) -> HashMap<(u64, String, bool), (Vec2, (u8, u8, u8))> {
    let mut map = HashMap::default();
    for (p, gt, cn) in ports {
        let hw = cn.size().x * 0.5;
        let x = if p.is_output { gt.translation.x + hw } else { gt.translation.x - hw };
        map.insert((p.node_id, p.pin.clone(), p.is_output), (Vec2::new(x, gt.translation.y), p.color));
    }
    map
}

/// Refresh every cable's control points from its endpoints' live transforms.
fn ngv_endpoints(mut materials: ResMut<Assets<CableMaterial>>, wires: Query<(&NgvWire, &MaterialNode<CableMaterial>)>, ports: Query<(&NgvPort, &UiGlobalTransform, &ComputedNode)>, transforms: Query<&UiGlobalTransform>, computeds: Query<&ComputedNode>) {
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
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.ab = Vec4::new(a.x, a.y, c1.x, c1.y);
            m.cd = Vec4::new(c2.x, c2.y, b.x, b.y);
            m.color = Vec4::new(lin.red, lin.green, lin.blue, 1.0);
            m.params = Vec4::new(WIRE_W / isf, 1.0, 0.0, 0.0);
        }
    }
}

/// Apply the caller's view requests: `fit_request` frames all nodes,
/// `center_request` recenters at the current zoom, `zoom_request` multiplies the
/// zoom (centre-anchored). Drives the canvas `GraphView` + `UiTransform`.
#[allow(clippy::type_complexity)]
fn ngv_view_ops(
    mut graphs: Query<(Entity, &mut NodeGraphView, &ComputedNode, &Children)>,
    mut canvases: Query<(&mut GraphView, &mut UiTransform)>,
    nodes: Query<(&NgvNode, &Node, &ComputedNode)>,
) {
    for (vp, mut g, vcn, children) in &mut graphs {
        if !g.fit_request && !g.center_request && g.zoom_request.is_none() {
            continue;
        }
        let Some(canvas) = children.iter().find(|&c| canvases.contains(c)) else {
            g.fit_request = false;
            g.center_request = false;
            g.zoom_request = None;
            continue;
        };
        let vp_isf = vcn.inverse_scale_factor();
        let vp_size = vcn.size() * vp_isf;
        let center = vp_size * 0.5;
        let Ok((mut view, mut tf)) = canvases.get_mut(canvas) else { continue };

        // Toolbar zoom ± (centre-anchored): pan scales with the zoom ratio.
        if let Some(factor) = g.zoom_request.take() {
            let cur = view.zoom.max(0.01);
            let new = (cur * factor).clamp(super::MIN_ZOOM, super::MAX_ZOOM);
            let r = new / cur;
            view.pan *= r;
            view.zoom = new;
        }

        if g.fit_request || g.center_request {
            let z = view.zoom.max(0.01);
            let (mut mn, mut mx, mut any) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN), false);
            for (n, node, ncn) in &nodes {
                if n.viewport != vp {
                    continue;
                }
                let pos = Vec2::new(px(node.left), px(node.top));
                let size = ncn.size() * vp_isf / z; // canvas-local logical
                mn = mn.min(pos);
                mx = mx.max(pos + size);
                any = true;
            }
            if any {
                let bbox_c = (mn + mx) * 0.5;
                let bbox_s = (mx - mn).max(Vec2::splat(1.0));
                if g.fit_request {
                    let pad = 80.0;
                    let zoom = (vp_size.x / (bbox_s.x + pad)).min(vp_size.y / (bbox_s.y + pad)).clamp(0.25, 1.5);
                    view.zoom = zoom;
                    view.pan = (center - bbox_c) * zoom;
                } else {
                    view.pan = (center - bbox_c) * view.zoom;
                }
            }
            g.fit_request = false;
            g.center_request = false;
        }
        tf.translation = Val2::px(view.pan.x, view.pan.y);
        tf.scale = Vec2::splat(view.zoom);
    }
}

// ── Comment / group boxes ─────────────────────────────────────────────────────

/// In-flight comment drag: the box, which nodes it carries, and the running
/// cursor anchor.
struct CommentDrag {
    comment: Entity,
    id: u64,
    vp: Entity,
    canvas: Entity,
    last: Vec2,
    contained: Vec<(Entity, u64)>,
    moved: bool,
}

/// Drag a comment by its body → move the box and every node it encloses (the
/// "group" behaviour). On release, record `CommentMoved` for the box and
/// `NodeMoved` for each carried node so the caller persists the new layout.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn ngv_comment_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: Local<Option<CommentDrag>>,
    comment_picks: Query<(Entity, &Interaction, &NgvComment)>,
    block_picks: Query<&Interaction, Or<(With<NgvNode>, With<NgvPort>, With<NgvCommentResize>, With<NgvCommentTitle>, With<NgvCommentDelete>)>>,
    node_rects: Query<(Entity, &NgvNode, &ComputedNode)>,
    views: Query<&GraphView>,
    mut all_nodes: Query<&mut Node>,
    mut graphs: Query<&mut NodeGraphView>,
) {
    if active.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        // Nodes / ports / a comment's own controls take priority over body-drag.
        if block_picks.iter().any(|i| *i == Interaction::Pressed) {
            return;
        }
        let Some(c) = cursor(&windows) else { return };
        let Some((ce, _, cm)) = comment_picks.iter().find(|(_, i, _)| **i == Interaction::Pressed) else { return };
        let Ok(cnode) = all_nodes.get(ce) else { return };
        let crect = [px(cnode.left), px(cnode.top), px(cnode.width), px(cnode.height)];
        let zoom = views.get(cm.canvas).map(|v| v.zoom).unwrap_or(1.0);
        let mut contained = Vec::new();
        for (ne, nn, ncn) in &node_rects {
            if nn.viewport != cm.viewport {
                continue;
            }
            let Ok(nnode) = all_nodes.get(ne) else { continue };
            let pos = Vec2::new(px(nnode.left), px(nnode.top));
            let size = ncn.size() * ncn.inverse_scale_factor() / zoom.max(0.01);
            let center = pos + size * 0.5;
            if center.x >= crect[0] && center.x <= crect[0] + crect[2] && center.y >= crect[1] && center.y <= crect[1] + crect[3] {
                contained.push((ne, nn.id));
            }
        }
        *active = Some(CommentDrag { comment: ce, id: cm.id, vp: cm.viewport, canvas: cm.canvas, last: c, contained, moved: false });
        return;
    }

    if !mouse.pressed(MouseButton::Left) {
        if let Some(cd) = active.take() {
            if cd.moved {
                let mut moves: Vec<GraphEdit> = Vec::new();
                if let Ok(n) = all_nodes.get(cd.comment) {
                    moves.push(GraphEdit::CommentMoved { id: cd.id, x: px(n.left), y: px(n.top) });
                }
                for (e, id) in &cd.contained {
                    if let Ok(n) = all_nodes.get(*e) {
                        moves.push(GraphEdit::NodeMoved { id: *id, x: px(n.left), y: px(n.top) });
                    }
                }
                if let Ok(mut g) = graphs.get_mut(cd.vp) {
                    g.pending.extend(moves);
                }
            }
        }
        return;
    }

    let Some(cd) = active.as_mut() else { return };
    let Some(c) = cursor(&windows) else { return };
    let delta = c - cd.last;
    if delta == Vec2::ZERO {
        return;
    }
    let zoom = views.get(cd.canvas).map(|v| v.zoom).unwrap_or(1.0);
    let d = delta / zoom.max(0.01);
    if let Ok(mut n) = all_nodes.get_mut(cd.comment) {
        n.left = Val::Px(px(n.left) + d.x);
        n.top = Val::Px(px(n.top) + d.y);
    }
    for (e, _) in &cd.contained {
        if let Ok(mut n) = all_nodes.get_mut(*e) {
            n.left = Val::Px(px(n.left) + d.x);
            n.top = Val::Px(px(n.top) + d.y);
        }
    }
    cd.last = c;
    cd.moved = true;
}

/// Drag the corner grip → resize the comment box; record `CommentResized` on release.
#[allow(clippy::type_complexity)]
fn ngv_comment_resize(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: Local<Option<(Entity, u64, Entity, Entity, Vec2, bool)>>, // (comment, id, canvas, vp, last, moved)
    grips: Query<(&Interaction, &NgvCommentResize)>,
    views: Query<&GraphView>,
    mut all_nodes: Query<&mut Node>,
    mut graphs: Query<&mut NodeGraphView>,
) {
    if active.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        let Some((_, g)) = grips.iter().find(|(i, _)| **i == Interaction::Pressed) else { return };
        let Some(c) = cursor(&windows) else { return };
        *active = Some((g.comment, g.id, g.canvas, g.viewport, c, false));
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        if let Some((comment, id, _, vp, _, moved)) = active.take() {
            if moved {
                if let Ok(n) = all_nodes.get(comment) {
                    if let Ok(mut gr) = graphs.get_mut(vp) {
                        gr.pending.push(GraphEdit::CommentResized { id, w: px(n.width), h: px(n.height) });
                    }
                }
            }
        }
        return;
    }
    let Some((comment, _, canvas, _, last, moved)) = active.as_mut() else { return };
    let Some(c) = cursor(&windows) else { return };
    let delta = c - *last;
    if delta == Vec2::ZERO {
        return;
    }
    let zoom = views.get(*canvas).map(|v| v.zoom).unwrap_or(1.0);
    let d = delta / zoom.max(0.01);
    if let Ok(mut n) = all_nodes.get_mut(*comment) {
        n.width = Val::Px((px(n.width) + d.x).max(80.0));
        n.height = Val::Px((px(n.height) + d.y).max(60.0));
    }
    *last = c;
    *moved = true;
}

/// Edit a comment title → record `CommentRetitled` (every keystroke; comments
/// don't affect compilation so this is cheap to apply).
fn ngv_comment_title(changed: Query<(&EmberTextInput, &NgvCommentTitle), Changed<EmberTextInput>>, mut graphs: Query<&mut NodeGraphView>) {
    for (inp, t) in &changed {
        if let Ok(mut g) = graphs.get_mut(t.viewport) {
            g.pending.push(GraphEdit::CommentRetitled { id: t.id, text: inp.value.clone() });
        }
    }
}

/// Click a comment's ✕ → record `DeleteComment`.
fn ngv_comment_delete(changed: Query<(&Interaction, &NgvCommentDelete), Changed<Interaction>>, mut graphs: Query<&mut NodeGraphView>) {
    for (interaction, d) in &changed {
        if *interaction == Interaction::Pressed {
            if let Ok(mut g) = graphs.get_mut(d.viewport) {
                g.pending.push(GraphEdit::DeleteComment { id: d.id });
            }
        }
    }
}

/// `C` over the graph with nodes selected → wrap them in a comment box. Ignored
/// while typing in a field (so 'c' in the search/title box is text).
#[allow(clippy::type_complexity)]
fn ngv_comment_key(
    keys: Res<ButtonInput<KeyCode>>,
    text_inputs: Query<&EmberTextInput>,
    mut graphs: Query<(Entity, &mut NodeGraphView, &RelativeCursorPosition, &Children)>,
    views: Query<&GraphView>,
    node_rects: Query<(&NgvNode, &Node, &ComputedNode)>,
) {
    if !keys.just_pressed(KeyCode::KeyC) {
        return;
    }
    if text_inputs.iter().any(|t| t.focused) {
        return;
    }
    for (vp, mut g, rcp, children) in &mut graphs {
        if !rcp.cursor_over || g.selected.is_empty() {
            continue;
        }
        let zoom = children.iter().find_map(|ch| views.get(ch).ok()).map(|v| v.zoom).unwrap_or(1.0);
        let (mut mn, mut mx, mut any) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN), false);
        for (n, node, ncn) in &node_rects {
            if n.viewport != vp || !g.selected.contains(&n.id) {
                continue;
            }
            let pos = Vec2::new(px(node.left), px(node.top));
            let size = ncn.size() * ncn.inverse_scale_factor() / zoom.max(0.01);
            mn = mn.min(pos);
            mx = mx.max(pos + size);
            any = true;
        }
        if any {
            let pad = 28.0;
            let header = 26.0;
            let rect = [mn.x - pad, mn.y - pad - header, (mx.x - mn.x) + pad * 2.0, (mx.y - mn.y) + pad * 2.0 + header];
            g.pending.push(GraphEdit::AddComment { rect });
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
