//! Bevy-native (ember) port of the egui material `GraphGraphPanel` canvas, built
//! on `renzora_ember`'s data-driven `node_graph_view`.
//!
//! WORK IN PROGRESS / not yet registered. This proves `node_graph_view` against
//! the real `MaterialGraph` model: nodes + wires are mounted from the graph
//! (keyed on structure), a toolbar adds nodes / applies, and a sync system drains
//! the view's `GraphEdit`s (node moved / connect / disconnect / select) back into
//! the graph + recompiles. Remaining to wire in: move the egui panel's
//! load-on-selection + autosave orchestration out of `ui()` into systems.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::core::CurrentProject;
use renzora_editor::{DocTabKind, EditorContext, EditorSelection, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{graph_node_view, graph_wire_view, menu_item, node_graph_view, screen_menu, GraphEdit, NodeGraphView};
use renzora_shader::material::graph::PinDir;
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::nodes::{categories, node_def, nodes_in_category};

use crate::graph_editor::category_icon;
use crate::graph_panel::{sync_to_entity, sync_to_file};
use crate::{MaterialEditMode, MaterialEditorState};

pub struct NativeMaterialGraph;

impl Plugin for NativeMaterialGraph {
    fn build(&self, app: &mut App) {
        app.register_panel_content("material_graph", false, build);
        app.add_systems(Update, (apply_click, add_node_open).run_if(in_state(SplashState::Editor)));
        // Orchestration: only while the graph panel is actually mounted (mirrors
        // the egui panel only running its sync inside `ui()`).
        app.add_systems(
            Update,
            (mat_graph_load, mat_graph_sync)
                .chain()
                .run_if(in_state(SplashState::Editor))
                .run_if(any_with_component::<MatGraph>),
        );
    }
}

#[derive(Component)]
struct MatGraph;
#[derive(Component)]
struct ApplyBtn;
#[derive(Component)]
struct AddNodeBtn;

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-material-graph"),
        ))
        .id();

    // Toolbar.
    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::bottom(Val::Px(1.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let add = tool_button(commands, fonts, "plus", "Add Node", accent(), AddNodeBtn);
    let apply = tool_button(commands, fonts, "check", "Apply", text_primary(), ApplyBtn);
    commands.entity(bar).add_children(&[add, apply]);

    // Canvas.
    let handle = node_graph_view(commands, fonts);
    commands.entity(handle.viewport).insert(MatGraph);
    let (canvas, viewport) = (handle.canvas, handle.viewport);

    // Wires draw in viewport space; nodes pan/zoom with the canvas.
    let wires_layer = commands.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }).id();
    commands.entity(viewport).add_child(wires_layer);
    keyed_list(commands, wires_layer, move |w| wire_snapshot(w, viewport));

    let nodes_layer = commands.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }).id();
    commands.entity(canvas).add_child(nodes_layer);
    keyed_list(commands, nodes_layer, move |w| node_snapshot(w, canvas, viewport));

    commands.entity(root).add_children(&[bar, handle.viewport]);
    renzora_editor::mark_drop_zone(commands, root);
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

#[allow(clippy::type_complexity)]
fn node_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(s) = world.get_resource::<MaterialEditorState>() else { return empty() };
    let sel = s.selected_node;
    // (id, title, color, position, inputs, outputs, selected)
    let nodes: Vec<(u64, String, (u8, u8, u8), [f32; 2], Vec<(String, String)>, Vec<(String, String)>, bool)> = s
        .graph
        .nodes
        .iter()
        .map(|n| {
            let def = node_def(&n.node_type);
            let title = def.map(|d| d.display_name.to_string()).unwrap_or_else(|| n.node_type.clone());
            let color = def.map(|d| (d.color[0], d.color[1], d.color[2])).unwrap_or((90, 90, 100));
            let pins = def.map(|d| (d.pins)()).unwrap_or_default();
            let inputs: Vec<(String, String)> = pins.iter().filter(|p| p.direction == PinDir::Input).map(|p| (p.name.clone(), p.label.clone())).collect();
            let outputs: Vec<(String, String)> = pins.iter().filter(|p| p.direction == PinDir::Output).map(|p| (p.name.clone(), p.label.clone())).collect();
            (n.id, title, color, n.position, inputs, outputs, sel == Some(n.id))
        })
        .collect();
    let items: Vec<(u64, u64)> = nodes
        .iter()
        .map(|(id, title, color, _pos, ins, outs, selected)| {
            let mut k = hasher();
            id.hash(&mut k);
            let mut h = hasher();
            // Structure + selection (NOT position) so dragging never rebuilds.
            (title, color, ins, outs, selected).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, title, color, pos, ins, outs, selected) = &nodes[i];
            graph_node_view(c, f, canvas, viewport, *id, title, *color, ins, outs, pos[0], pos[1], *selected)
        }),
    }
}

fn wire_snapshot(world: &World, viewport: Entity) -> KeyedSnapshot {
    let Some(s) = world.get_resource::<MaterialEditorState>() else { return empty() };
    let wires: Vec<(u64, String, u64, String)> = s.graph.connections.iter().map(|c| (c.from_node, c.from_pin.clone(), c.to_node, c.to_pin.clone())).collect();
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

/// Load the right material into `MaterialEditorState` when the active document
/// (asset mode) or the selected entity (scene mode) changes — the orchestration
/// the egui panel did inside `ui()`.
fn mat_graph_load(world: &mut World) {
    // Asset mode: a standalone .material document tab.
    let asset_path: Option<String> = world.get_resource::<EditorContext>().and_then(|ctx| match ctx {
        EditorContext::Asset { path, kind: DocTabKind::Material } => Some(path.clone()),
        _ => None,
    });
    if let Some(path) = asset_path {
        let needs = !matches!(&world.resource::<MaterialEditorState>().edit_mode, MaterialEditMode::EditingFile { path: p } if *p == path);
        if needs {
            sync_to_file(world, path);
        }
        return;
    }

    // Scene mode: follow the selected entity's MaterialRef.
    let selected_entity = world.get_resource::<EditorSelection>().and_then(|s| s.get());
    let mat_ref_path = selected_entity.and_then(|e| world.get::<MaterialRef>(e).map(|m| m.0.clone()));
    let (entity_changed, path_changed, leaving) = {
        let st = world.resource::<MaterialEditorState>();
        let cur = match &st.edit_mode {
            MaterialEditMode::Existing { path, .. } => Some(path.clone()),
            _ => None,
        };
        let ec = selected_entity != st.editing_entity;
        let pc = !ec && mat_ref_path != cur;
        let lv = matches!(st.edit_mode, MaterialEditMode::EditingFile { .. });
        (ec, pc, lv)
    };
    if entity_changed || path_changed || leaving {
        let has_mesh = selected_entity.is_some_and(|e| world.get::<Mesh3d>(e).is_some());
        let entity_name = selected_entity.and_then(|e| world.get::<Name>(e).map(|n| n.as_str().to_string()));
        sync_to_entity(world, selected_entity, has_mesh, mat_ref_path, entity_name);
    }
}

/// Apply the view's recorded edits to the graph, recompile, and (for a brand-new
/// material-less entity) create + link the `.material` file on first edit.
fn mat_graph_sync(world: &mut World) {
    let mut edits: Vec<GraphEdit> = Vec::new();
    let mut q = world.query_filtered::<&mut NodeGraphView, With<MatGraph>>();
    for mut view in q.iter_mut(world) {
        if !view.pending.is_empty() {
            edits.append(&mut view.pending);
        }
    }
    if edits.is_empty() {
        return;
    }

    let mut structural = false;
    let mut dirty = false;
    {
        let mut st = world.resource_mut::<MaterialEditorState>();
        for edit in edits {
            match edit {
                GraphEdit::NodeMoved { id, x, y } => {
                    if let Some(n) = st.graph.nodes.iter_mut().find(|n| n.id == id) {
                        n.position = [x, y];
                        dirty = true;
                    }
                }
                GraphEdit::Connect { from_node, from_pin, to_node, to_pin } => {
                    st.graph.connect(from_node, &from_pin, to_node, &to_pin);
                    structural = true;
                }
                GraphEdit::Disconnect { to_node, to_pin, .. } => {
                    st.graph.disconnect(to_node, &to_pin);
                    structural = true;
                }
                GraphEdit::Select { id } => {
                    if st.selected_node != id {
                        st.selected_node = id;
                    }
                }
            }
        }
    }

    if structural {
        let graph = world.resource::<MaterialEditorState>().graph.clone();
        let result = renzora_shader::material::codegen::compile(&graph);
        let mut st = world.resource_mut::<MaterialEditorState>();
        st.compiled_wgsl = Some(result.fragment_shader);
        st.compile_errors = result.errors;
    }
    if structural || dirty {
        world.resource_mut::<MaterialEditorState>().is_dirty = true;
        let pending_entity = match world.resource::<MaterialEditorState>().edit_mode {
            MaterialEditMode::Pending { entity } => Some(entity),
            _ => None,
        };
        if let Some(entity) = pending_entity {
            pending_first_save(world, entity);
        }
    }
}

/// First edit of a material-less entity: write `materials/<name>.material`, link
/// it via `MaterialRef`, and transition to `Existing`.
fn pending_first_save(world: &mut World, entity: Entity) {
    let graph_name = world.resource::<MaterialEditorState>().graph.name.clone();
    let asset_path = format!("materials/{}.material", graph_name);
    if let Some(project_root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) {
        let dir = project_root.join("materials");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join(format!("{}.material", graph_name));
        let mut graph_to_save = world.resource::<MaterialEditorState>().graph.clone();
        if let Ok((json, _errors)) = renzora_shader::material::precompiled::save_compiled_and_serialize(&mut graph_to_save, &project_root, &file) {
            let _ = std::fs::write(&file, &json);
            world.resource_mut::<MaterialEditorState>().graph = graph_to_save;
        }
    }
    world.entity_mut(entity).remove::<renzora_shader::material::resolver::MaterialResolved>();
    world.entity_mut(entity).insert(MaterialRef(asset_path.clone()));
    world.resource_mut::<MaterialEditorState>().edit_mode = MaterialEditMode::Existing { path: asset_path, entity };
}

fn apply_click(q: Query<&Interaction, (With<ApplyBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(crate::apply_material);
    }
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
        if category == "Output" {
            continue;
        }
        let icon = category_icon(category);
        for def in nodes_in_category(category) {
            let node_type = def.node_type;
            let pos = [60.0 + offset, 60.0 + offset];
            offset += 6.0;
            kids.push(menu_item(&mut commands, &fonts, icon, def.display_name, move |w| {
                if let Some(mut s) = w.get_resource_mut::<MaterialEditorState>() {
                    s.graph.add_node(node_type, pos);
                    let graph = s.graph.clone();
                    let result = renzora_shader::material::codegen::compile(&graph);
                    s.compiled_wgsl = Some(result.fragment_shader);
                    s.compile_errors = result.errors;
                    s.is_dirty = true;
                }
            }));
        }
    }
    commands.entity(menu).add_children(&kids);
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
