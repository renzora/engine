//! The Terrain Graph panel: the open `.tgraph` on ember's node graph view.
//!
//! Built the way the material and effect graphs are built
//! (`renzora_material_editor/src/graph.rs`, `renzora_particles/editor/src/graph.rs`):
//! `node_graph_view` for the canvas, `graph_node_view` / `graph_wire_view` /
//! `graph_comment_view` mounted through `keyed_list` keyed on graph
//! *structure*, so dragging, selecting or scrubbing never rebuilds a node, and
//! the widget's recorded `GraphEdit`s drained back into the document by
//! [`graph_sync`]. The add-node palette is ember's `search_menu`, grouped by
//! catalog category.
//!
//! Nodes carry their preview ([`crate::preview`]) and no value fields: a
//! terrain node has up to a dozen parameters, and a dozen rows per node would
//! bury the previews that are the point of the graph. Parameters are edited in
//! the Terrain Graph Inspector beside it ([`crate::inspector_panel`]).

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    comment_search_entry, graph_comment_view, graph_node_view, graph_wire_view, node_graph_view, search_menu, GraphEdit,
    HoverTooltip, NodeGraphView, SearchEntry, TooltipAnchorAbove,
};
use renzora_ember::UiInteraction;
use renzora_terrain_graph::catalog::{categories, node_def, NodeDef, PinKind, NODES};
use renzora_terrain_graph::format::GraphComment;

use crate::preview::PreviewImages;
use crate::state::{save_document, TerrainGraphEditorState};

pub const PANEL: &str = "terrain_graph";

/// Header colour per catalog category.
pub fn category_color(category: &str) -> (u8, u8, u8) {
    match category {
        "Primitives" => (74, 140, 92),
        "Shape" => (62, 120, 196),
        "Simulate" => (176, 112, 58),
        "Data" => (132, 92, 196),
        "Output" => (196, 72, 72),
        _ => (110, 120, 160),
    }
}

pub fn category_icon(category: &str) -> &'static str {
    match category {
        "Primitives" => "mountains",
        "Shape" => "sliders-horizontal",
        "Simulate" => "drop",
        "Data" => "chart-bar",
        "Output" => "sign-out",
        _ => "circle",
    }
}

/// Pin colour per kind: a warm height, a cool mask (the approved mockup's).
fn pin_color(kind: PinKind) -> (u8, u8, u8) {
    match kind {
        PinKind::Height => (232, 178, 92),
        PinKind::Mask => (127, 200, 214),
    }
}

/// Every pin connects to every other: a mask is a height of 0..1 m and a
/// height is a mask past 1, and both are useful (a mask driving a Combine's
/// B, a height warping another).
const PIN_GROUP: u32 = 0;

fn pin_hint(def: &NodeDef) -> String {
    let side = |names: Vec<&str>| names.join(", ");
    let ins = side(def.inputs.iter().map(|p| p.label).collect());
    let outs = side(def.outputs.iter().map(|p| p.label).collect());
    match (ins.is_empty(), outs.is_empty()) {
        (true, true) => String::new(),
        (true, false) => outs,
        (false, true) => ins,
        (false, false) => format!("{ins} \u{203a} {outs}"),
    }
}

/// On the graph viewport.
#[derive(Component)]
pub struct TerrainGraphView;

#[derive(Component, Clone, Copy)]
enum ToolAction {
    AddNode,
    Save,
    Build,
}

#[derive(Clone, Copy)]
enum ViewOp {
    Fit,
    Center,
    ZoomIn,
    ZoomOut,
    Comment,
}

#[derive(Component)]
struct ViewOpBtn {
    op: ViewOp,
    view: Entity,
}

pub fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("terrain-graph-panel"),
        ))
        .id();

    let empty = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            Name::new("terrain-graph-empty"),
        ))
        .id();
    let empty_text = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, empty_text, |w| match w.get_resource::<TerrainGraphEditorState>() {
        Some(st) if st.load_error.is_some() => format!("Could not open {}: {}", st.file_name(), st.load_error.clone().unwrap_or_default()),
        _ => "Open a .tgraph from the Assets panel to edit it, or make one with Create > Terrain Graph.".to_string(),
    });
    commands.entity(empty).add_child(empty_text);
    bind_display(commands, empty, |w| !w.get_resource::<TerrainGraphEditorState>().is_some_and(TerrainGraphEditorState::is_open));

    let editor = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    bind_display(commands, editor, |w| w.get_resource::<TerrainGraphEditorState>().is_some_and(TerrainGraphEditorState::is_open));

    let handle = node_graph_view(commands, fonts);
    let (canvas, viewport) = (handle.canvas, handle.viewport);
    commands.entity(viewport).insert(TerrainGraphView);

    let toolbar = build_toolbar(commands, fonts, viewport);
    let errors = error_strip(commands, fonts);

    let layer = |commands: &mut Commands| {
        commands
            .spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() })
            .id()
    };
    let comments_layer = layer(commands);
    commands.entity(canvas).add_child(comments_layer);
    keyed_list(commands, comments_layer, move |w| comment_snapshot(w, canvas, viewport));
    let wires_layer = layer(commands);
    commands.entity(viewport).add_child(wires_layer);
    keyed_list(commands, wires_layer, move |w| wire_snapshot(w, viewport));
    let nodes_layer = layer(commands);
    commands.entity(canvas).add_child(nodes_layer);
    keyed_list(commands, nodes_layer, move |w| node_snapshot(w, canvas, viewport));

    commands.entity(editor).add_children(&[toolbar, errors, viewport]);
    commands.entity(root).add_children(&[empty, editor]);
    renzora_editor_framework::mark_drop_zone(commands, root);
    root
}

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts, view: Entity) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            Name::new("terrain-graph-toolbar"),
        ))
        .id();
    let file = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_shrink: 0.0, ..default() }).id();
    let glyph = icon_text(commands, &fonts.phosphor, "mountains", (150, 190, 110), 13.0);
    let name = commands.spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, name, |w| w.get_resource::<TerrainGraphEditorState>().map(|s| s.file_name()).unwrap_or_default());
    let dirty = commands
        .spawn((
            Node { width: Val::Px(7.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(accent())),
            HoverTooltip::new("Unsaved changes (Ctrl+S saves)"),
        ))
        .id();
    bind_display(commands, dirty, |w| w.get_resource::<TerrainGraphEditorState>().is_some_and(|s| s.dirty));
    commands.entity(file).add_children(&[glyph, name, dirty]);

    let add = tool_button(commands, fonts, "plus", Some("Add Node"), "Add a node (or right-click the graph)", false);
    commands.entity(add).insert((ToolAction::AddNode, RelativeCursorPosition::default()));
    let comment = tool_button(commands, fonts, "chat-text", Some("Comment"), "Group the selected nodes in a comment box (C)", false);
    commands.entity(comment).insert(ViewOpBtn { op: ViewOp::Comment, view });
    let save = tool_button(commands, fonts, "floppy-disk", Some("Save"), "Write the graph to its file (Ctrl+S)", false);
    commands.entity(save).insert(ToolAction::Save);
    let build = tool_button(commands, fonts, "hammer", Some("Build"), "Build the terrain from this graph at full resolution, keeping its sculpting", true);
    commands.entity(build).insert(ToolAction::Build);

    // Preview progress, while a preview is computing.
    let status = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, status, |w| {
        let preview = w.get_resource::<crate::preview::PreviewJob>().and_then(|j| j.progress);
        match preview {
            Some(p) => format!("Previewing {:.0}%", p * 100.0),
            None => String::new(),
        }
    });

    let sep = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let group = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(2.0), flex_shrink: 0.0, ..default() }).id();
    let mut views = Vec::new();
    for (icon, tip, op) in [
        ("arrows-in", "Fit the graph in view", ViewOp::Fit),
        ("crosshair-simple", "Center the view", ViewOp::Center),
        ("magnifying-glass-plus", "Zoom in", ViewOp::ZoomIn),
        ("magnifying-glass-minus", "Zoom out", ViewOp::ZoomOut),
    ] {
        let b = tool_button(commands, fonts, icon, None, tip, false);
        commands.entity(b).insert(ViewOpBtn { op, view });
        views.push(b);
    }
    commands.entity(group).add_children(&views);
    commands.entity(bar).add_children(&[file, add, comment, sep, status, save, build, group]);
    bar
}

/// A toolbar button in the material graph's flat style: no fill at rest, a
/// hover wash, and only the primary action (Build) filled in the accent.
pub fn tool_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: Option<&str>, tip: &str, primary: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(28.0),
                min_width: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(if label.is_some() { 12.0 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(tip),
            Name::new("terrain-graph-tool"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hot = matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed));
        match (primary, hot) {
            (true, false) => rgb(accent()),
            (true, true) => renzora_ember::theme::mix(accent(), text_primary(), 0.15),
            (false, true) => rgb(hover_bg()),
            (false, false) => Color::NONE,
        }
    });
    let icon_color = if primary { on_accent() } else if label.is_some() { accent() } else { text_muted() };
    let glyph = icon_text(commands, &fonts.phosphor, icon, icon_color, 14.0);
    commands.entity(glyph).insert(renzora_ember::interaction::PASS_THROUGH);
    let mut kids = vec![glyph];
    if let Some(label) = label {
        let t = commands
            .spawn((
                Text::new(label),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(if primary { on_accent() } else { text_primary() })),
                renzora_ember::interaction::PASS_THROUGH,
            ))
            .id();
        kids.push(t);
    }
    commands.entity(btn).add_children(&kids);
    btn
}

fn error_strip(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let strip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                border: UiRect::vertical(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Name::new("terrain-graph-errors"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "warning", close_red(), 13.0);
    let text = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, text, |w| w.get_resource::<TerrainGraphEditorState>().and_then(|s| s.error_summary()).unwrap_or_default());
    bind_display(commands, strip, |w| w.get_resource::<TerrainGraphEditorState>().is_some_and(|s| s.error_summary().is_some()));
    commands.entity(strip).add_children(&[icon, text]);
    strip
}

// ── Snapshots ────────────────────────────────────────────────────────────────

type Port = (String, String, (u8, u8, u8), u32);

struct NodeSnap {
    id: u64,
    title: String,
    color: (u8, u8, u8),
    pos: [f32; 2],
    inputs: Vec<Port>,
    outputs: Vec<Port>,
    selected: bool,
    thumb: Option<Handle<Image>>,
    error: Option<String>,
    tip: Option<String>,
}

fn node_snapshot(w: &Rx, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(st) = w.get_resource::<TerrainGraphEditorState>() else { return empty() };
    if !st.is_open() {
        return empty();
    }
    let images = w.get_resource::<PreviewImages>();
    let ports = |pins: &[renzora_terrain_graph::catalog::PinDef]| -> Vec<Port> {
        pins.iter().map(|p| (p.name.to_string(), p.label.to_string(), pin_color(p.kind), PIN_GROUP)).collect()
    };
    let nodes: Vec<NodeSnap> = st
        .graph
        .nodes
        .iter()
        .map(|n| {
            let def = node_def(&n.node_type);
            let (inputs, outputs) = match def {
                Some(d) => (ports(d.inputs), ports(d.outputs)),
                None => (Vec::new(), Vec::new()),
            };
            // Mask Output names its layer in the title, so the outputs column
            // of a graph reads as the list of layers it makes.
            let title = match (def, n.node_type.as_str()) {
                (Some(_), "mask_output") => {
                    let name = n.params.get("name").and_then(|v| v.as_text()).unwrap_or("Mask");
                    format!("Mask: {name}")
                }
                (Some(d), _) => d.display_name.to_string(),
                (None, t) => t.to_string(),
            };
            let error = st.node_errors.get(&n.id).cloned().or_else(|| def.is_none().then(|| format!("unknown node type `{}`", n.node_type)));
            NodeSnap {
                id: n.id,
                title,
                color: def.map(|d| category_color(d.category)).unwrap_or((170, 45, 45)),
                pos: n.position,
                inputs,
                outputs,
                selected: st.selected_node == Some(n.id),
                thumb: images.and_then(|i| i.map.get(&n.id).cloned()),
                tip: def.map(|d| d.description.to_string()),
                error,
            }
        })
        .collect();
    let items = nodes
        .iter()
        .map(|n| {
            let mut k = hasher();
            n.id.hash(&mut k);
            let mut h = hasher();
            // Structure only, never position, selection or parameter values,
            // so a drag, a click and a scrub update in place. Whether a
            // preview image exists is in it (the node gains its preview once),
            // the image's pixels are not.
            (&n.title, n.color, &n.inputs, &n.outputs, n.thumb.is_some(), &n.error).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let n = &nodes[i];
            let header = n.error.as_ref().map(|_| {
                let icon = icon_text(c, &f.phosphor, "warning", close_red(), 13.0);
                c.entity(icon).insert(Node { flex_shrink: 0.0, ..default() });
                icon
            });
            let node = graph_node_view(c, f, canvas, viewport, n.id, &n.title, n.color, &n.inputs, &n.outputs, n.pos[0], n.pos[1], n.selected, n.thumb.clone(), &[], header);
            if let Some(tip) = n.error.clone().or_else(|| n.tip.clone()) {
                c.entity(node).insert((HoverTooltip::new(tip), TooltipAnchorAbove));
            }
            node
        }),
    }
}

fn wire_snapshot(w: &Rx, viewport: Entity) -> KeyedSnapshot {
    let Some(st) = w.get_resource::<TerrainGraphEditorState>() else { return empty() };
    if !st.is_open() {
        return empty();
    }
    let wires: Vec<(u64, String, u64, String)> =
        st.graph.connections.iter().map(|c| (c.from_node, c.from_pin.clone(), c.to_node, c.to_pin.clone())).collect();
    let items = wires
        .iter()
        .map(|wire| {
            let mut k = hasher();
            wire.hash(&mut k);
            let h = k.finish();
            (h, h)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let (a, ap, b, bp) = &wires[i];
            graph_wire_view(c, viewport, *a, ap, *b, bp)
        }),
    }
}

fn comment_snapshot(w: &Rx, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(st) = w.get_resource::<TerrainGraphEditorState>() else { return empty() };
    if !st.is_open() {
        return empty();
    }
    let comments: Vec<(u64, String, [f32; 4])> = st.graph.comments.iter().map(|c| (c.id, c.text.clone(), c.rect)).collect();
    let items = comments
        .iter()
        .map(|(id, ..)| {
            let mut k = hasher();
            id.hash(&mut k);
            let h = k.finish();
            (h, h)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, text, rect) = &comments[i];
            graph_comment_view(c, f, canvas, viewport, *id, text, *rect, (150, 190, 110))
        }),
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Drain the view's recorded edits into the document.
pub fn graph_sync(world: &mut World) {
    let mut edits: Vec<GraphEdit> = Vec::new();
    let mut q = world.query_filtered::<&mut NodeGraphView, With<TerrainGraphView>>();
    for mut view in q.iter_mut(world) {
        if !view.pending.is_empty() {
            edits.append(&mut view.pending);
        }
    }
    if edits.is_empty() {
        return;
    }
    let mut notes: Vec<String> = Vec::new();
    {
        let mut st = world.resource_mut::<TerrainGraphEditorState>();
        if !st.is_open() {
            return;
        }
        let mut selected = st.selected_node;
        let graph = &mut st.graph;
        for edit in edits {
            match edit {
                GraphEdit::NodeMoved { id, x, y } => {
                    if let Some(n) = graph.node_mut(id) {
                        if n.position != [x, y] {
                            n.position = [x, y];
                        }
                    }
                }
                GraphEdit::Connect { from_node, from_pin, to_node, to_pin } => {
                    if let Err(e) = graph.connect(from_node, &from_pin, to_node, &to_pin) {
                        notes.push(format!("Connection refused: {e}"));
                    }
                }
                GraphEdit::Disconnect { to_node, to_pin, .. } => {
                    graph.disconnect(to_node, &to_pin);
                }
                GraphEdit::Delete { id } => {
                    if graph.remove_node(id) && selected == Some(id) {
                        selected = None;
                    }
                }
                GraphEdit::Select { id } => selected = id,
                GraphEdit::AddComment { rect } => {
                    let id = graph.next_id();
                    graph.comments.push(GraphComment { id, text: "Comment".into(), rect });
                }
                GraphEdit::CommentMoved { id, x, y } => {
                    if let Some(c) = graph.comments.iter_mut().find(|c| c.id == id) {
                        c.rect[0] = x;
                        c.rect[1] = y;
                    }
                }
                GraphEdit::CommentResized { id, w, h } => {
                    if let Some(c) = graph.comments.iter_mut().find(|c| c.id == id) {
                        c.rect[2] = w;
                        c.rect[3] = h;
                    }
                }
                GraphEdit::CommentRetitled { id, text } => {
                    if let Some(c) = graph.comments.iter_mut().find(|c| c.id == id) {
                        c.text = text;
                    }
                }
                GraphEdit::DeleteComment { id } => graph.comments.retain(|c| c.id != id),
            }
        }
        if st.selected_node != selected {
            st.selected_node = selected;
        }
    }
    for note in notes {
        renzora::console_log::console_info("Terrain Graph", note);
    }
}

/// Frame the graph when another one opens.
pub fn fit_on_switch(state: Res<TerrainGraphEditorState>, mut views: Query<&mut NodeGraphView, With<TerrainGraphView>>, mut last: Local<Option<Option<String>>>) {
    if last.as_ref() == Some(&state.path) {
        return;
    }
    *last = Some(state.path.clone());
    for mut v in &mut views {
        v.fit_request = true;
    }
}

fn view_op_click(q: Query<(&UiInteraction, &ViewOpBtn), Changed<UiInteraction>>, mut views: Query<&mut NodeGraphView, With<TerrainGraphView>>) {
    for (i, btn) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        let Ok(mut v) = views.get_mut(btn.view) else { continue };
        match btn.op {
            ViewOp::Fit => v.fit_request = true,
            ViewOp::Center => v.center_request = true,
            ViewOp::ZoomIn => v.zoom_request = Some(1.25),
            ViewOp::ZoomOut => v.zoom_request = Some(0.8),
            ViewOp::Comment => v.request_comment(None),
        }
    }
}

fn toolbar_click(q: Query<(&UiInteraction, &ToolAction), Changed<UiInteraction>>, mut commands: Commands) {
    for (i, action) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        match action {
            ToolAction::Save => commands.queue(save_document),
            ToolAction::Build => commands.queue(crate::build::build_open_graph),
            ToolAction::AddNode => {}
        }
    }
}

fn add_node_open(
    q: Query<(&UiInteraction, &ToolAction, &RelativeCursorPosition, &ComputedNode), Changed<UiInteraction>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, _, rcp, cn)) = q.iter().find(|(i, a, ..)| **i == UiInteraction::Pressed && matches!(a, ToolAction::AddNode)) else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    search_menu(&mut commands, &fonts, top_left.x, top_left.y + size.y + 2.0, node_entries([60.0, 60.0], None));
}

/// Right-click on empty canvas opens the palette at the cursor; a cable
/// dropped on empty canvas opens it to wire the new node to that pin.
fn context_menu_open(fonts: Option<Res<EmberFonts>>, mut commands: Commands, mut views: Query<(Entity, &mut NodeGraphView), With<TerrainGraphView>>) {
    let Some(fonts) = fonts else { return };
    for (view, mut v) in &mut views {
        if let Some((screen, canvas)) = v.context_menu.take() {
            let mut entries = vec![comment_search_entry(view, canvas)];
            entries.extend(node_entries([canvas.x, canvas.y], None));
            search_menu(&mut commands, &fonts, screen.x, screen.y, entries);
        }
        if let Some(cd) = v.connect_drag.take() {
            let src = (cd.node, cd.pin, cd.is_output);
            search_menu(&mut commands, &fonts, cd.screen.x, cd.screen.y, node_entries([cd.canvas.x, cd.canvas.y], Some(src)));
        }
    }
}

/// The palette: every catalog node, by category. From a dropped cable, only
/// nodes with a pin on the side the cable needs.
pub fn node_entries(at: [f32; 2], src: Option<(u64, String, bool)>) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for category in categories() {
        let icon = category_icon(category);
        for def in NODES.iter().filter(|d| d.category == category) {
            if let Some((_, _, from_output)) = &src {
                let fits = if *from_output { !def.inputs.is_empty() } else { !def.outputs.is_empty() };
                if !fits {
                    continue;
                }
            }
            let node_type = def.node_type;
            let src = src.clone();
            entries.push(
                SearchEntry::new(icon, def.display_name, category, move |w| add_node(w, node_type, at, src.clone()))
                    .with_detail(pin_hint(def))
                    .with_description(def.description),
            );
        }
    }
    entries
}

/// Add a node, select it, and (from a dropped cable) wire it to the pin the
/// cable came from: the first pin of the same kind, else the first.
fn add_node(w: &mut World, node_type: &str, at: [f32; 2], src: Option<(u64, String, bool)>) {
    let mut st = w.resource_mut::<TerrainGraphEditorState>();
    if !st.is_open() {
        return;
    }
    let id = st.graph.add_node(node_type, at);
    if let Some((src_node, src_pin, src_is_output)) = src {
        let src_kind = st.graph.node(src_node).and_then(|n| node_def(&n.node_type)).and_then(|d| {
            if src_is_output { d.output(&src_pin).map(|p| p.kind) } else { d.input(&src_pin).map(|p| p.kind) }
        });
        if let Some(def) = node_def(node_type) {
            let pins = if src_is_output { def.inputs } else { def.outputs };
            let pick = pins.iter().find(|p| Some(p.kind) == src_kind).or(pins.first()).map(|p| p.name);
            if let Some(pin) = pick {
                let _ = if src_is_output {
                    st.graph.connect(src_node, &src_pin, id, pin)
                } else {
                    st.graph.connect(id, pin, src_node, &src_pin)
                };
            }
        }
    }
    st.selected_node = Some(id);
}

pub struct TerrainGraphPanel;

impl Plugin for TerrainGraphPanel {
    fn build(&self, app: &mut App) {
        use renzora_ember::panel::RegisterPanelContent;
        app.register_panel_content(PANEL, false, build).systems(
            Update,
            (toolbar_click, view_op_click, add_node_open, context_menu_open, fit_on_switch, graph_sync)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_node_is_in_the_palette_with_a_colour() {
        assert_eq!(node_entries([0.0, 0.0], None).len(), NODES.len());
        for c in categories() {
            assert_ne!(category_icon(c), "circle", "{c} has no icon");
            assert_ne!(category_color(c), (110, 120, 160), "{c} has no colour");
        }
    }
}
