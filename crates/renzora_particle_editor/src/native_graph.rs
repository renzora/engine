//! Bevy-native (ember) port of the egui particle `GraphPanel` canvas, built on
//! `renzora_ember`'s data-driven `node_graph_view`.
//!
//! Simpler than the material/blueprint graphs: the model is a single resource
//! field (`ParticleEditorState.node_graph`), generated from the current effect.
//! Nodes + wires are mounted from it (keyed on structure); a sync system drains
//! the view's `GraphEdit`s back into it. Toolbar: Add Node (auto-wires Spawn/
//! Init/Update/Render modules into the Emitter) + Presets.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_editor_framework::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{graph_comment_view, graph_node_view, graph_wire_view, menu_item, node_graph_view, screen_menu, search_menu, GraphEdit, NodeGraphView, SearchEntry};
use renzora_hanabi::node_graph::{ParticleNodeGraph, ParticleNodeType, PinDir};
use renzora_hanabi::{load_effect_from_file, ParticleEditorState};

pub struct NativeParticleGraph;

impl Plugin for NativeParticleGraph {
    fn build(&self, app: &mut App) {
        app.register_panel_content("particle_graph", false, build);
        app.add_systems(
            Update,
            (add_node_open, part_context_menu_open, presets_open)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active("particle_graph")),
        );
        app.add_systems(
            Update,
            (ensure_node_graph, part_graph_sync)
                .chain()
                .run_if(in_state(SplashState::Editor))
                .run_if(any_with_component::<PartGraph>)
                .run_if(renzora_ember::dock::panel_active("particle_graph")),
        );
    }
}

#[derive(Component)]
struct PartGraph;
#[derive(Component)]
struct AddNodeBtn;
#[derive(Component)]
struct PresetsBtn;

/// Phosphor icon name for a particle node category (for native ember menus).
fn category_icon(category: &str) -> &'static str {
    match category {
        "Spawn" => "sparkle",
        "Init" => "arrows-out",
        "Update" => "wind",
        "Render" => "palette",
        "Math" => "calculator",
        "Constants" => "hash",
        _ => "circle",
    }
}

fn cat_color(category: &str) -> (u8, u8, u8) {
    match category {
        "Emitter" => (60, 60, 60),
        "Spawn" => (50, 100, 200),
        "Init" => (50, 150, 50),
        "Update" => (200, 100, 50),
        "Render" => (150, 50, 200),
        "Constants" => (80, 80, 80),
        _ => (100, 100, 100),
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-particle-graph"),
        ))
        .id();

    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::bottom(Val::Px(1.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let presets = tool_button(commands, fonts, "sparkle", "Presets", text_muted(), PresetsBtn);
    let add = tool_button(commands, fonts, "plus", "Add Node", accent(), AddNodeBtn);
    commands.entity(bar).add_children(&[presets, add]);

    let handle = node_graph_view(commands, fonts);
    commands.entity(handle.viewport).insert(PartGraph);
    let (canvas, viewport) = (handle.canvas, handle.viewport);

    // Comment / group boxes mount behind the nodes (their own canvas layer).
    let comments_layer = commands.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }).id();
    commands.entity(canvas).add_child(comments_layer);
    keyed_list(commands, comments_layer, move |w| comment_snapshot(w, canvas, viewport));

    let wires_layer = commands.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }).id();
    commands.entity(viewport).add_child(wires_layer);
    keyed_list(commands, wires_layer, move |w| wire_snapshot(w, viewport));

    let nodes_layer = commands.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }).id();
    commands.entity(canvas).add_child(nodes_layer);
    keyed_list(commands, nodes_layer, move |w| node_snapshot(w, canvas, viewport));

    commands.entity(root).add_children(&[bar, handle.viewport]);
    renzora_editor_framework::mark_drop_zone(commands, root);
    root
}

fn tool_button<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, color: (u8, u8, u8), marker: M) -> Entity {
    let btn = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(card_bg())), Interaction::default(), RelativeCursorPosition::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(color)))).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

// ── Snapshots ──────────────────────────────────────────────────────────────────

type Port = (String, String, (u8, u8, u8));
/// Neutral port colour for particle pins.
const PART_PORT: (u8, u8, u8) = (140, 150, 175);

#[allow(clippy::type_complexity)]
fn node_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(s) = world.get_resource::<ParticleEditorState>() else { return empty() };
    let Some(graph) = s.node_graph.as_ref() else { return empty() };
    let sel = s.selected_node;
    let nodes: Vec<(u64, String, (u8, u8, u8), [f32; 2], Vec<Port>, Vec<Port>, bool)> = graph
        .nodes
        .iter()
        .map(|n| {
            let title = n.node_type.display_name().to_string();
            let color = cat_color(n.node_type.category());
            let pins = n.node_type.pins();
            let inputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Input).map(|p| (p.name.clone(), p.label.clone(), PART_PORT)).collect();
            let outputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Output).map(|p| (p.name.clone(), p.label.clone(), PART_PORT)).collect();
            (n.id, title, color, n.position, inputs, outputs, sel == Some(n.id))
        })
        .collect();
    let items: Vec<(u64, u64)> = nodes
        .iter()
        .map(|(id, title, color, _pos, ins, outs, _selected)| {
            let mut k = hasher();
            id.hash(&mut k);
            let mut h = hasher();
            // Structure only (not selection) so selecting never rebuilds a node.
            (title, color, ins, outs).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, title, color, pos, ins, outs, selected) = &nodes[i];
            graph_node_view(c, f, canvas, viewport, *id, title, *color, ins, outs, pos[0], pos[1], *selected, None, &[], None)
        }),
    }
}

/// Comment boxes, keyed on id only — drag / resize / retitle update in place.
fn comment_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(s) = world.get_resource::<ParticleEditorState>() else { return empty() };
    let Some(graph) = s.node_graph.as_ref() else { return empty() };
    let comments: Vec<(u64, String, [f32; 4], (u8, u8, u8))> =
        graph.comments.iter().map(|c| (c.id, c.text.clone(), c.rect, (c.color[0], c.color[1], c.color[2]))).collect();
    let items: Vec<(u64, u64)> = comments
        .iter()
        .map(|(id, _, _, _)| {
            let mut k = hasher();
            id.hash(&mut k);
            let h = k.finish();
            (h, h)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, text, rect, color) = &comments[i];
            graph_comment_view(c, f, canvas, viewport, *id, text, *rect, *color)
        }),
    }
}

fn wire_snapshot(world: &World, viewport: Entity) -> KeyedSnapshot {
    let Some(s) = world.get_resource::<ParticleEditorState>() else { return empty() };
    let Some(graph) = s.node_graph.as_ref() else { return empty() };
    let wires: Vec<(u64, String, u64, String)> = graph.connections.iter().map(|c| (c.from_node, c.from_pin.clone(), c.to_node, c.to_pin.clone())).collect();
    let items: Vec<(u64, u64)> = wires
        .iter()
        .map(|(fnode, fpin, tnode, tpin)| {
            let mut k = hasher();
            (fnode, fpin, tnode, tpin).hash(&mut k);
            (k.finish(), k.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let (fnode, fpin, tnode, tpin) = &wires[i];
            graph_wire_view(c, viewport, *fnode, fpin, *tnode, tpin)
        }),
    }
}

// ── Systems ────────────────────────────────────────────────────────────────────

/// Generate `node_graph` from the current effect if it isn't built yet.
fn ensure_node_graph(state: Option<ResMut<ParticleEditorState>>) {
    let Some(mut s) = state else { return };
    if s.node_graph.is_none() {
        if let Some(g) = s.current_effect.as_ref().map(ParticleNodeGraph::from_effect) {
            s.node_graph = Some(g);
        }
    }
}

fn part_graph_sync(mut views: Query<&mut NodeGraphView, With<PartGraph>>, state: Option<ResMut<ParticleEditorState>>) {
    let Some(mut s) = state else { return };
    let mut changed = false;
    for mut view in &mut views {
        for edit in view.pending.drain(..) {
            match edit {
                GraphEdit::NodeMoved { id, x, y } => {
                    if let Some(g) = s.node_graph.as_mut() {
                        if let Some(n) = g.get_node_mut(id) {
                            n.position = [x, y];
                            changed = true;
                        }
                    }
                }
                GraphEdit::Connect { from_node, from_pin, to_node, to_pin } => {
                    if let Some(g) = s.node_graph.as_mut() {
                        g.connect(from_node, &from_pin, to_node, &to_pin);
                        changed = true;
                    }
                }
                GraphEdit::Disconnect { to_node, to_pin, .. } => {
                    if let Some(g) = s.node_graph.as_mut() {
                        g.disconnect(to_node, &to_pin);
                        changed = true;
                    }
                }
                GraphEdit::Delete { id } => {
                    if let Some(g) = s.node_graph.as_mut() {
                        g.remove_node(id);
                        changed = true;
                    }
                    if s.selected_node == Some(id) {
                        s.selected_node = None;
                    }
                }
                GraphEdit::Select { id } => s.selected_node = id,
                GraphEdit::AddComment { rect } => {
                    if let Some(g) = s.node_graph.as_mut() {
                        g.add_comment(rect);
                        changed = true;
                    }
                }
                GraphEdit::CommentMoved { id, x, y } => {
                    if let Some(c) = s.node_graph.as_mut().and_then(|g| g.get_comment_mut(id)) {
                        c.rect[0] = x;
                        c.rect[1] = y;
                        changed = true;
                    }
                }
                GraphEdit::CommentResized { id, w, h } => {
                    if let Some(c) = s.node_graph.as_mut().and_then(|g| g.get_comment_mut(id)) {
                        c.rect[2] = w;
                        c.rect[3] = h;
                        changed = true;
                    }
                }
                GraphEdit::CommentRetitled { id, text } => {
                    if let Some(c) = s.node_graph.as_mut().and_then(|g| g.get_comment_mut(id)) {
                        c.text = text;
                        changed = true;
                    }
                }
                GraphEdit::DeleteComment { id } => {
                    if let Some(g) = s.node_graph.as_mut() {
                        g.remove_comment(id);
                        changed = true;
                    }
                }
            }
        }
    }
    if changed {
        s.is_modified = true;
    }
}

fn add_particle_node(world: &mut World, node_type: ParticleNodeType, pos: [f32; 2]) {
    let Some(mut s) = world.get_resource_mut::<ParticleEditorState>() else { return };
    if s.node_graph.is_none() {
        let g = s.current_effect.as_ref().map(ParticleNodeGraph::from_effect);
        s.node_graph = g;
    }
    let module_pin = match node_type.category() {
        "Spawn" => Some("spawn"),
        "Init" => Some("init"),
        "Update" => Some("update"),
        "Render" => Some("render"),
        _ => None,
    };
    if let Some(g) = s.node_graph.as_mut() {
        let new_id = g.add_node(node_type, pos);
        if let Some(pin) = module_pin {
            if let Some(emitter) = g.nodes.iter().find(|n| n.node_type == ParticleNodeType::Emitter) {
                let eid = emitter.id;
                g.connect(new_id, "module", eid, pin);
            }
        }
        s.is_modified = true;
    }
}

/// Every particle node type as a searchable palette entry, each spawning at
/// `base` (canvas px). Shared by the toolbar button + the right-click/Spacebar menu.
fn part_node_entries(base: [f32; 2]) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for &category in ParticleNodeType::categories() {
        let icon = category_icon(category);
        for node_type in ParticleNodeType::nodes_in_category(category) {
            let label = node_type.display_name();
            entries.push(SearchEntry::new(icon, label, category, move |w| add_particle_node(w, node_type, base)));
        }
    }
    entries
}

/// Toolbar "Add Node" → open the searchable palette under the button.
fn add_node_open(
    q: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode), (With<AddNodeBtn>, Changed<Interaction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else { return };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    search_menu(&mut commands, &fonts, top_left.x, top_left.y + size.y + 2.0, part_node_entries([60.0, 60.0]));
}

/// Right-click / Spacebar on empty canvas → open the searchable palette at the
/// cursor, spawning the chosen node at the clicked canvas point.
fn part_context_menu_open(fonts: Option<Res<EmberFonts>>, mut commands: Commands, mut views: Query<&mut NodeGraphView, With<PartGraph>>) {
    let Some(fonts) = fonts else { return };
    for mut v in &mut views {
        if let Some((screen, canvas)) = v.context_menu.take() {
            search_menu(&mut commands, &fonts, screen.x, screen.y, part_node_entries([canvas.x, canvas.y]));
        }
        // Cable dragged onto empty canvas → same palette. Particle modules are
        // auto-wired into the Emitter by `add_particle_node`, so dropping a cable
        // and picking a module connects it the same sensible way.
        if let Some(cd) = v.connect_drag.take() {
            search_menu(&mut commands, &fonts, cd.screen.x, cd.screen.y, part_node_entries([cd.canvas.x, cd.canvas.y]));
        }
    }
}

fn presets_open(
    q: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode), (With<PresetsBtn>, Changed<Interaction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else { return };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir("assets/particles")
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "particle"))
        .collect();
    files.sort();
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kids: Vec<Entity> = files
        .into_iter()
        .map(|path| {
            let label = path.file_stem().and_then(|s| s.to_str()).unwrap_or("preset").to_string();
            menu_item(&mut commands, &fonts, "sparkle", &label, move |w| {
                if let Some(effect) = load_effect_from_file(&path) {
                    if let Some(mut s) = w.get_resource_mut::<ParticleEditorState>() {
                        s.node_graph = Some(ParticleNodeGraph::from_effect(&effect));
                        s.current_file_path = Some(path.to_string_lossy().to_string());
                        s.current_effect = Some(effect);
                        s.is_modified = false;
                    }
                }
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
