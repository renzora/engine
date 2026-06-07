//! Bevy-native (ember) port of the egui blueprint `GraphPanel` canvas, built on
//! `renzora_ember`'s data-driven `node_graph_view` (same engine as the material
//! graph).
//!
//! Dual-mode like the properties panel: scene mode edits the `BlueprintGraph`
//! component on the editing entity; asset mode mutates `file_graph` + persists
//! the `.blueprint`. Nodes + wires are mounted from the active graph (keyed on
//! structure); a sync system drains the view's `GraphEdit`s (move/connect/
//! disconnect/select) back into it. Toolbar: Add Node (creates the blueprint if
//! the entity has none) + Apply (compile to Lua, scene mode).

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::core::CurrentProject;
use renzora_blueprint::graph::PinDir;
use renzora_blueprint::{categories, node_def, nodes_in_category, BlueprintGraph};
use renzora_editor_framework::{DocTabKind, EditorContext, EditorSelection, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{graph_node_view, graph_wire_view, menu_item, node_graph_view, screen_menu, GraphEdit, NodeGraphView};

use crate::graph_editor::category_icon;
use crate::graph_panel::{apply_blueprint_to_lua, load_blueprint_file, save_blueprint_file};
use crate::BlueprintEditorState;

pub struct NativeBlueprintGraph;

impl Plugin for NativeBlueprintGraph {
    fn build(&self, app: &mut App) {
        app.register_panel_content("blueprint_graph", false, build);
        app.add_systems(Update, (apply_click, add_node_open).run_if(in_state(SplashState::Editor)));
        app.add_systems(
            Update,
            (bp_graph_load, bp_graph_sync)
                .chain()
                .run_if(in_state(SplashState::Editor))
                .run_if(any_with_component::<BpGraph>),
        );
    }
}

#[derive(Component)]
struct BpGraph;
#[derive(Component)]
struct ApplyBtn;
#[derive(Component)]
struct AddNodeBtn;

// ── Dual-mode graph access ──────────────────────────────────────────────────────

fn is_asset(w: &World) -> bool {
    matches!(w.get_resource::<EditorContext>(), Some(EditorContext::Asset { kind: DocTabKind::Blueprint, .. }))
}

fn with_active_graph<R>(w: &World, f: impl FnOnce(&BlueprintGraph) -> R) -> Option<R> {
    let s = w.get_resource::<BlueprintEditorState>()?;
    if is_asset(w) {
        s.file_graph.as_ref().map(f)
    } else {
        let e = s.editing_entity?;
        w.get::<BlueprintGraph>(e).map(f)
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-blueprint-graph"),
        ))
        .id();

    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::bottom(Val::Px(1.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let add = tool_button(commands, fonts, "plus", "Add Node", accent(), AddNodeBtn);
    let apply = tool_button(commands, fonts, "lightning", "Apply", text_primary(), ApplyBtn);
    commands.entity(bar).add_children(&[add, apply]);

    let handle = node_graph_view(commands, fonts);
    commands.entity(handle.viewport).insert(BpGraph);
    let (canvas, viewport) = (handle.canvas, handle.viewport);

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
/// Neutral port colour for blueprint pins (material pins are typed/coloured).
const BP_PORT: (u8, u8, u8) = (140, 150, 175);

#[allow(clippy::type_complexity)]
fn node_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let sel = world.get_resource::<BlueprintEditorState>().and_then(|s| s.selected_node);
    let nodes: Vec<(u64, String, (u8, u8, u8), [f32; 2], Vec<Port>, Vec<Port>, bool)> = with_active_graph(world, |g| {
        g.nodes
            .iter()
            .map(|n| {
                let def = node_def(&n.node_type);
                let title = def.map(|d| d.display_name.to_string()).unwrap_or_else(|| n.node_type.clone());
                let color = def.map(|d| (d.color[0], d.color[1], d.color[2])).unwrap_or((90, 90, 100));
                let pins = def.map(|d| (d.pins)()).unwrap_or_default();
                let inputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Input).map(|p| (p.name.clone(), p.label.clone(), BP_PORT)).collect();
                let outputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Output).map(|p| (p.name.clone(), p.label.clone(), BP_PORT)).collect();
                (n.id, title, color, n.position, inputs, outputs, sel == Some(n.id))
            })
            .collect()
    })
    .unwrap_or_default();
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
            graph_node_view(c, f, canvas, viewport, *id, title, *color, ins, outs, pos[0], pos[1], *selected, None)
        }),
    }
}

fn wire_snapshot(world: &World, viewport: Entity) -> KeyedSnapshot {
    let wires: Vec<(u64, String, u64, String)> = with_active_graph(world, |g| g.connections.iter().map(|c| (c.from_node, c.from_pin.clone(), c.to_node, c.to_pin.clone())).collect()).unwrap_or_default();
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

/// Track the active document / selected entity and (re)load the graph.
fn bp_graph_load(world: &mut World) {
    let asset_path: Option<String> = world.get_resource::<EditorContext>().and_then(|ctx| match ctx {
        EditorContext::Asset { path, kind: DocTabKind::Blueprint } => Some(path.clone()),
        _ => None,
    });
    if let Some(path) = asset_path {
        let needs = world.resource::<BlueprintEditorState>().editing_file_path.as_deref() != Some(path.as_str());
        if needs {
            let project = world.get_resource::<CurrentProject>().cloned();
            let graph = load_blueprint_file(project.as_ref(), &path).unwrap_or_default();
            let mut s = world.resource_mut::<BlueprintEditorState>();
            s.editing_file_path = Some(path);
            s.file_graph = Some(graph);
            s.editing_entity = None;
            s.selected_node = None;
            s.is_dirty = false;
        }
        return;
    }
    let selected_entity = world.get_resource::<EditorSelection>().and_then(|s| s.get());
    let (changed, leftover) = {
        let st = world.resource::<BlueprintEditorState>();
        (selected_entity != st.editing_entity, st.editing_file_path.is_some())
    };
    if changed || leftover {
        let mut s = world.resource_mut::<BlueprintEditorState>();
        s.editing_entity = selected_entity;
        s.editing_file_path = None;
        s.file_graph = None;
        s.is_dirty = false;
    }
}

/// Drain the view's edits into the active graph + persist.
fn bp_graph_sync(world: &mut World) {
    let mut edits: Vec<GraphEdit> = Vec::new();
    let mut q = world.query_filtered::<&mut NodeGraphView, With<BpGraph>>();
    for mut view in q.iter_mut(world) {
        if !view.pending.is_empty() {
            edits.append(&mut view.pending);
        }
    }
    if edits.is_empty() {
        return;
    }

    let asset = is_asset(world);
    let mut graph = with_active_graph(world, |g| g.clone());
    let mut changed = false;
    let mut new_sel: Option<Option<u64>> = None;
    for edit in edits {
        match edit {
            GraphEdit::NodeMoved { id, x, y } => {
                if let Some(g) = graph.as_mut() {
                    if let Some(n) = g.get_node_mut(id) {
                        n.position = [x, y];
                        changed = true;
                    }
                }
            }
            GraphEdit::Connect { from_node, from_pin, to_node, to_pin } => {
                if let Some(g) = graph.as_mut() {
                    g.connect(from_node, &from_pin, to_node, &to_pin);
                    changed = true;
                }
            }
            GraphEdit::Disconnect { to_node, to_pin, .. } => {
                if let Some(g) = graph.as_mut() {
                    g.disconnect(to_node, &to_pin);
                    changed = true;
                }
            }
            GraphEdit::Delete { id } => {
                if let Some(g) = graph.as_mut() {
                    g.remove_node(id);
                    changed = true;
                    if world.resource::<BlueprintEditorState>().selected_node == Some(id) {
                        new_sel = Some(None);
                    }
                }
            }
            GraphEdit::Select { id } => new_sel = Some(id),
        }
    }
    if let Some(sel) = new_sel {
        world.resource_mut::<BlueprintEditorState>().selected_node = sel;
    }
    if changed {
        let Some(g) = graph else { return };
        if asset {
            let path = world.resource::<BlueprintEditorState>().editing_file_path.clone();
            let project = world.get_resource::<CurrentProject>().cloned();
            world.resource_mut::<BlueprintEditorState>().file_graph = Some(g.clone());
            if let Some(path) = path {
                save_blueprint_file(project.as_ref(), &path, &g);
            }
            world.resource_mut::<BlueprintEditorState>().is_dirty = false;
        } else if let Some(e) = world.resource::<BlueprintEditorState>().editing_entity {
            world.entity_mut(e).insert(g);
        }
    }
}

fn apply_click(q: Query<&Interaction, (With<ApplyBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(bp_apply);
    }
}

fn bp_apply(world: &mut World) {
    if is_asset(world) {
        return; // compile-to-Lua needs an entity context
    }
    let Some(entity) = world.resource::<BlueprintEditorState>().editing_entity else { return };
    let Some(graph) = world.get::<BlueprintGraph>(entity).cloned() else { return };
    let Some(project) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else { return };
    let name = world.get::<Name>(entity).map(|n| n.as_str().to_string()).unwrap_or_else(|| format!("Entity {}", entity.index()));
    apply_blueprint_to_lua(world, entity, &graph, &project, &name);
}

fn add_blueprint_node(world: &mut World, node_type: &str, pos: [f32; 2]) {
    if is_asset(world) {
        let path = world.resource::<BlueprintEditorState>().editing_file_path.clone();
        let project = world.get_resource::<CurrentProject>().cloned();
        let mut g = world.resource::<BlueprintEditorState>().file_graph.clone().unwrap_or_default();
        g.add_node(node_type, pos);
        world.resource_mut::<BlueprintEditorState>().file_graph = Some(g.clone());
        if let Some(path) = path {
            save_blueprint_file(project.as_ref(), &path, &g);
        }
    } else if let Some(e) = world.resource::<BlueprintEditorState>().editing_entity {
        if world.get::<BlueprintGraph>(e).is_none() {
            world.entity_mut(e).insert(BlueprintGraph::new());
        }
        if let Some(mut g) = world.get_mut::<BlueprintGraph>(e) {
            g.add_node(node_type, pos);
        }
    }
    world.resource_mut::<BlueprintEditorState>().is_dirty = true;
}

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
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let mut kids: Vec<Entity> = Vec::new();
    let mut offset = 0.0f32;
    for category in categories() {
        let icon = category_icon(category);
        for def in nodes_in_category(category) {
            let node_type = def.node_type;
            let pos = [60.0 + offset, 60.0 + offset];
            offset += 6.0;
            kids.push(menu_item(&mut commands, &fonts, icon, def.display_name, move |w| add_blueprint_node(w, node_type, pos)));
        }
    }
    commands.entity(menu).add_children(&kids);
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
