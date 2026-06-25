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
use renzora_blueprint::graph::{PinDir, PinTemplate, PinType};
use renzora_blueprint::{categories, node_def, nodes_in_category, BlueprintGraph};
use renzora_editor_framework::{DocTabKind, EditorContext, EditorSelection, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{graph_comment_view, graph_node_view, graph_wire_view, node_graph_view, search_menu, GraphEdit, NodeGraphView, SearchEntry};

use crate::graph_editor::category_icon;
use crate::graph_panel::{apply_blueprint_to_lua, load_blueprint_file, save_blueprint_file};
use crate::BlueprintEditorState;

pub struct NativeBlueprintGraph;

impl Plugin for NativeBlueprintGraph {
    fn build(&self, app: &mut App) {
        use renzora_ember::toolbar::PanelToolbarExt;
        app.register_panel_content("blueprint_graph", false, build);
        // Toolbar actions live in the shared strip (shown when the blueprint
        // graph is the active panel).
        app.register_panel_toolbar("blueprint_graph", build_toolbar);
        app.add_systems(
            Update,
            (apply_click, add_node_open, bp_context_menu_open, layout_click)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active("blueprint_graph")),
        );
        app.add_systems(
            Update,
            (bp_graph_load, bp_graph_sync)
                .chain()
                .run_if(in_state(SplashState::Editor))
                .run_if(any_with_component::<BpGraph>)
                .run_if(renzora_ember::dock::panel_active("blueprint_graph")),
        );
    }
}

#[derive(Component)]
struct BpGraph;
#[derive(Component)]
struct ApplyBtn;
#[derive(Component)]
struct AddNodeBtn;
#[derive(Component)]
struct LayoutBtn;

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

/// The blueprint graph's toolbar (Add Node / Auto Layout / Apply), mounted in
/// the shared strip while the blueprint graph is the active panel. Wired by the
/// same marker-component systems regardless of where the buttons live.
fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Content-sized — the strip host supplies background + centering.
    let bar = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(8.0)), flex_shrink: 0.0, ..default() },
            Name::new("blueprint-graph-toolbar"),
        ))
        .id();
    let add = tool_button(commands, fonts, "plus", "Add Node", accent(), AddNodeBtn);
    let layout = tool_button(commands, fonts, "tree-structure", "Auto Layout", text_primary(), LayoutBtn);
    let apply = tool_button(commands, fonts, "lightning", "Apply", text_primary(), ApplyBtn);
    commands.entity(bar).add_children(&[add, layout, apply]);
    bar
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-blueprint-graph"),
        ))
        .id();

    // Canvas (the toolbar lives in the shared strip — see `build_toolbar`).
    let handle = node_graph_view(commands, fonts);
    commands.entity(handle.viewport).insert(BpGraph);
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

    commands.entity(root).add_child(handle.viewport);
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

/// Per-input data for the inline editors: (pin template, is-connected). Aligned
/// index-for-index with the node's input `Port`s.
type InputSpec = (PinTemplate, bool);
type NodeData = (u64, String, (u8, u8, u8), [f32; 2], Vec<Port>, Vec<Port>, bool, Vec<InputSpec>);

#[allow(clippy::type_complexity)]
fn node_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let sel = world.get_resource::<BlueprintEditorState>().and_then(|s| s.selected_node);
    let nodes: Vec<NodeData> = with_active_graph(world, |g| {
        g.nodes
            .iter()
            .map(|n| {
                let def = node_def(&n.node_type);
                let title = def.map(|d| d.display_name.to_string()).unwrap_or_else(|| n.node_type.clone());
                let color = def.map(|d| (d.color[0], d.color[1], d.color[2])).unwrap_or((90, 90, 100));
                let pins = def.map(|d| (d.pins)()).unwrap_or_default();
                let inputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Input).map(|p| (p.name.clone(), p.label.clone(), BP_PORT)).collect();
                let outputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Output).map(|p| (p.name.clone(), p.label.clone(), BP_PORT)).collect();
                // Aligned with `inputs`: clone the template + whether a wire feeds it.
                let in_specs: Vec<InputSpec> = pins
                    .iter()
                    .filter(|p| p.direction == PinDir::Input)
                    .map(|p| {
                        let connected = g.connections.iter().any(|c| c.to_node == n.id && c.to_pin == p.name);
                        ((*p).clone(), connected)
                    })
                    .collect();
                (n.id, title, color, n.position, inputs, outputs, sel == Some(n.id), in_specs)
            })
            .collect()
    })
    .unwrap_or_default();
    let items: Vec<(u64, u64)> = nodes
        .iter()
        .map(|(id, title, color, _pos, ins, outs, _selected, specs)| {
            let mut k = hasher();
            id.hash(&mut k);
            let mut h = hasher();
            // Structure + per-input connected state (not selection / values): selecting
            // or editing a value updates in place, but connecting a wire rebuilds the
            // node so its inline editor appears/disappears.
            let connected: Vec<bool> = specs.iter().map(|(_, c)| *c).collect();
            (title, color, ins, outs, &connected).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, title, color, pos, ins, outs, selected, specs) = &nodes[i];
            // Inline editor per unconnected, editable input (index-aligned with `ins`).
            let editors: Vec<Option<Entity>> = specs
                .iter()
                .map(|(pin, connected)| {
                    if *connected || matches!(pin.pin_type, PinType::Exec | PinType::Any) {
                        None
                    } else {
                        Some(crate::native_properties::pin_editor(c, f, *id, pin))
                    }
                })
                .collect();
            graph_node_view(c, f, canvas, viewport, *id, title, *color, ins, outs, pos[0], pos[1], *selected, None, &editors)
        }),
    }
}

/// Comment boxes, keyed on id only — drag / resize / retitle mutate the box in
/// place (and the model) without rebuilding, so the title field keeps focus.
fn comment_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let comments: Vec<(u64, String, [f32; 4], (u8, u8, u8))> = with_active_graph(world, |g| {
        g.comments.iter().map(|c| (c.id, c.text.clone(), c.rect, (c.color[0], c.color[1], c.color[2]))).collect()
    })
    .unwrap_or_default();
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
            GraphEdit::AddComment { rect } => {
                if let Some(g) = graph.as_mut() {
                    g.add_comment(rect);
                    changed = true;
                }
            }
            GraphEdit::CommentMoved { id, x, y } => {
                if let Some(c) = graph.as_mut().and_then(|g| g.get_comment_mut(id)) {
                    c.rect[0] = x;
                    c.rect[1] = y;
                    changed = true;
                }
            }
            GraphEdit::CommentResized { id, w, h } => {
                if let Some(c) = graph.as_mut().and_then(|g| g.get_comment_mut(id)) {
                    c.rect[2] = w;
                    c.rect[3] = h;
                    changed = true;
                }
            }
            GraphEdit::CommentRetitled { id, text } => {
                if let Some(c) = graph.as_mut().and_then(|g| g.get_comment_mut(id)) {
                    c.text = text;
                    changed = true;
                }
            }
            GraphEdit::DeleteComment { id } => {
                if let Some(g) = graph.as_mut() {
                    g.remove_comment(id);
                    changed = true;
                }
            }
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

fn layout_click(
    q: Query<&Interaction, (With<LayoutBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(bp_auto_layout);
    }
}

/// Tidy the active graph (asset file or scene-entity component) with the shared
/// layered auto-layout, then persist it the same way edits are saved.
fn bp_auto_layout(world: &mut World) {
    if is_asset(world) {
        let path = world.resource::<BlueprintEditorState>().editing_file_path.clone();
        let project = world.get_resource::<CurrentProject>().cloned();
        let Some(mut g) = world.resource::<BlueprintEditorState>().file_graph.clone() else {
            return;
        };
        renzora_blueprint::layout::auto_layout(&mut g);
        world.resource_mut::<BlueprintEditorState>().file_graph = Some(g.clone());
        if let Some(path) = path {
            save_blueprint_file(project.as_ref(), &path, &g);
        }
    } else if let Some(e) = world.resource::<BlueprintEditorState>().editing_entity {
        if let Some(mut g) = world.get::<BlueprintGraph>(e).cloned() {
            renzora_blueprint::layout::auto_layout(&mut g);
            world.entity_mut(e).insert(g);
        }
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

/// Mutate the active graph (asset file or scene-entity component), persisting +
/// marking dirty the same way for both modes. Creates the component on demand.
fn bp_apply_graph(world: &mut World, f: impl FnOnce(&mut BlueprintGraph)) {
    if is_asset(world) {
        let path = world.resource::<BlueprintEditorState>().editing_file_path.clone();
        let project = world.get_resource::<CurrentProject>().cloned();
        let mut g = world.resource::<BlueprintEditorState>().file_graph.clone().unwrap_or_default();
        f(&mut g);
        world.resource_mut::<BlueprintEditorState>().file_graph = Some(g.clone());
        if let Some(path) = path {
            save_blueprint_file(project.as_ref(), &path, &g);
        }
    } else if let Some(e) = world.resource::<BlueprintEditorState>().editing_entity {
        if world.get::<BlueprintGraph>(e).is_none() {
            world.entity_mut(e).insert(BlueprintGraph::new());
        }
        if let Some(mut g) = world.get_mut::<BlueprintGraph>(e) {
            f(&mut g);
        }
    }
    world.resource_mut::<BlueprintEditorState>().is_dirty = true;
}

fn add_blueprint_node(world: &mut World, node_type: &str, pos: [f32; 2]) {
    bp_apply_graph(world, |g| {
        g.add_node(node_type, pos);
    });
}

/// Add a node from a dragged cable: spawn it at `base`, then wire `src` to the
/// new node's best-matching opposite-direction pin (exact type > compatible >
/// any). `src` = `(node, pin, is_output)`.
fn bp_add_and_wire(world: &mut World, node_type: &str, base: [f32; 2], src: (u64, String, bool)) {
    bp_apply_graph(world, move |g| {
        let new_id = g.add_node(node_type, base);
        let want_dir = if src.2 { PinDir::Input } else { PinDir::Output };
        let src_ty = g
            .get_node(src.0)
            .and_then(|n| node_def(&n.node_type))
            .map(|d| (d.pins)())
            .and_then(|pins| pins.into_iter().find(|p| p.name == src.1).map(|p| p.pin_type));
        let new_pins = node_def(node_type).map(|d| (d.pins)()).unwrap_or_default();
        let pick = new_pins.iter().filter(|p| p.direction == want_dir).min_by_key(|p| match src_ty {
            Some(t) if p.pin_type == t => 0u8,
            Some(t) if PinType::compatible(t, p.pin_type) || PinType::compatible(p.pin_type, t) => 1,
            _ => 2,
        });
        if let Some(p) = pick {
            if src.2 {
                g.connect(src.0, &src.1, new_id, &p.name);
            } else {
                g.connect(new_id, &p.name, src.0, &src.1);
            }
        }
    });
}

/// Every catalog node as a searchable palette entry, each spawning at `base`
/// (canvas px). Shared by the toolbar button + the right-click/Spacebar menu.
fn bp_node_entries(base: [f32; 2]) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for category in categories() {
        let icon = category_icon(category);
        for def in nodes_in_category(category) {
            let node_type = def.node_type;
            entries.push(SearchEntry::new(icon, def.display_name, category, move |w| add_blueprint_node(w, node_type, base)));
        }
    }
    entries
}

/// Toolbar "Add Node" → open the searchable palette under the button. New nodes
/// land at a default canvas spot (the user can drag them).
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
    search_menu(&mut commands, &fonts, top_left.x, top_left.y + size.y + 2.0, bp_node_entries([80.0, 80.0]));
}

/// Right-click / Spacebar on empty canvas → the shared widget records
/// `context_menu`; open the searchable palette at the cursor and spawn the
/// chosen node at the clicked canvas point.
fn bp_context_menu_open(fonts: Option<Res<EmberFonts>>, mut commands: Commands, mut views: Query<&mut NodeGraphView, With<BpGraph>>) {
    let Some(fonts) = fonts else { return };
    for mut v in &mut views {
        if let Some((screen, canvas)) = v.context_menu.take() {
            search_menu(&mut commands, &fonts, screen.x, screen.y, bp_node_entries([canvas.x, canvas.y]));
        }
        // Cable dragged onto empty canvas → same palette, but each pick auto-wires
        // the new node back to the dragged pin.
        if let Some(cd) = v.connect_drag.take() {
            let src = (cd.node, cd.pin, cd.is_output);
            search_menu(&mut commands, &fonts, cd.screen.x, cd.screen.y, bp_connect_entries([cd.canvas.x, cd.canvas.y], src));
        }
    }
}

/// Catalog entries whose action spawns the node and auto-wires it to `src`.
fn bp_connect_entries(base: [f32; 2], src: (u64, String, bool)) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for category in categories() {
        let icon = category_icon(category);
        for def in nodes_in_category(category) {
            let node_type = def.node_type;
            let src = src.clone();
            entries.push(SearchEntry::new(icon, def.display_name, category, move |w| bp_add_and_wire(w, node_type, base, src.clone())));
        }
    }
    entries
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
