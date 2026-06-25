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

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::core::CurrentProject;
use renzora_editor_framework::{DocTabKind, EditorContext, EditorSelection, SplashState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{dropdown, graph_comment_view, graph_node_view, graph_wire_view, icon_button, icon_label_button, node_graph_view, search_menu, GraphEdit, NodeGraphView, SearchEntry};
use renzora_shader::material::codegen;
use renzora_shader::material::graph::{MaterialGraph, PinDir, PinType, PinValue};
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::nodes::{categories, node_def, nodes_in_category};

use crate::{MaterialEditMode, MaterialEditorState, MaterialTab};

/// Phosphor icon name for a material node category (for native ember menus).
fn category_icon(category: &str) -> &'static str {
    match category {
        "Input" => "sign-in",
        "Parameter" => "sliders-horizontal",
        "Texture" => "image",
        "Math" => "calculator",
        "Vector" => "arrows-out-cardinal",
        "Color" => "palette",
        "Procedural" => "waves",
        "Animation" => "timer",
        "Utility" => "wrench",
        "Output" => "sign-out",
        _ => "circle",
    }
}

pub struct NativeMaterialGraph;

impl Plugin for NativeMaterialGraph {
    fn build(&self, app: &mut App) {
        app.register_panel_content("material_graph", false, build);
        app.add_systems(
            Update,
            (apply_click, add_node_open, view_op_click, context_menu_open)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active("material_graph")),
        );
        // Orchestration: only while the graph panel is actually mounted (mirrors
        // the egui panel only running its sync inside `ui()`) AND visible.
        // `mat_graph_load` builds the tab set from the selection; the dropdown is
        // rebuilt from it before the view's edits are drained.
        app.add_systems(
            Update,
            (mat_graph_load, rebuild_material_dropdown, mat_graph_sync)
                .chain()
                .run_if(in_state(SplashState::Editor))
                .run_if(any_with_component::<MatGraph>)
                .run_if(renzora_ember::dock::panel_active("material_graph")),
        );
    }
}

#[derive(Component)]
struct MatGraph;
#[derive(Component)]
struct ApplyBtn;
#[derive(Component)]
struct AddNodeBtn;
#[derive(Clone, Copy)]
enum ViewOp {
    Fit,
    Center,
    ZoomIn,
    ZoomOut,
}
#[derive(Component)]
struct ViewOpBtn(ViewOp);
/// Container for the material picker dropdown; `sig` is the last-built tab-set
/// hash so the dropdown is only rebuilt when the material list changes.
#[derive(Component)]
struct MaterialTabsRoot {
    sig: u64,
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-material-graph"),
        ))
        .id();

    // Toolbar — a single horizontal strip: node/view actions on the left, the
    // material picker pushed to the right. Buttons are ember widgets so they get
    // themed hover/press states (the old hand-rolled chips were flat + tiny).
    let bar = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::bottom(Val::Px(1.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let add = icon_label_button(commands, fonts, "plus", "Add Node");
    // `add_node_open` reads the button's `RelativeCursorPosition` to place the
    // add-node menu under the cursor; the ember button doesn't carry one, so add
    // it here (the old hand-rolled button included it).
    commands.entity(add).insert((AddNodeBtn, RelativeCursorPosition::default()));
    let apply = icon_label_button(commands, fonts, "check", "Apply");
    commands.entity(apply).insert(ApplyBtn);
    let sep = toolbar_divider(commands);
    let fit = icon_label_button(commands, fonts, "arrows-in", "Fit");
    commands.entity(fit).insert(ViewOpBtn(ViewOp::Fit));
    let center = icon_label_button(commands, fonts, "crosshair-simple", "Center");
    commands.entity(center).insert(ViewOpBtn(ViewOp::Center));
    let zin = icon_button(commands, fonts, "magnifying-glass-plus");
    commands.entity(zin).insert(ViewOpBtn(ViewOp::ZoomIn));
    let zout = icon_button(commands, fonts, "magnifying-glass-minus");
    commands.entity(zout).insert(ViewOpBtn(ViewOp::ZoomOut));

    // Material picker — a dropdown of every material in the selected entity's
    // subtree (a model can have many, so a dropdown beats a tab row; it scrolls
    // when long). Sits at the left of the toolbar. Hidden when the selection has
    // none. `dd_holder` is rebuilt by `rebuild_material_dropdown` whenever the
    // set changes; the leading label is static so it survives the rebuild.
    let mat_section = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), flex_shrink: 0.0, ..default() }, Name::new("material-picker")))
        .id();
    let mat_label = commands
        .spawn((Text::new("Material"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    let dd_holder = commands
        .spawn((Node::default(), MaterialTabsRoot { sig: u64::MAX }, Name::new("material-dd")))
        .id();
    commands.entity(mat_section).add_children(&[mat_label, dd_holder]);
    bind_display(commands, mat_section, |w| {
        w.get_resource::<MaterialEditorState>().is_some_and(|s| !s.tabs.is_empty())
    });

    commands.entity(bar).add_children(&[mat_section, add, apply, sep, fit, center, zin, zout]);

    // Canvas.
    let handle = node_graph_view(commands, fonts);
    commands.entity(handle.viewport).insert(MatGraph);
    let (canvas, viewport) = (handle.canvas, handle.viewport);

    // Comment / group boxes mount behind the nodes (their own canvas layer).
    let comments_layer = commands.spawn(Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }).id();
    commands.entity(canvas).add_child(comments_layer);
    keyed_list(commands, comments_layer, move |w| comment_snapshot(w, canvas, viewport));

    // Wires draw in viewport space; nodes pan/zoom with the canvas.
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

/// A thin vertical separator between toolbar button groups.
fn toolbar_divider(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node { width: Val::Px(1.0), height: Val::Px(18.0), margin: UiRect::horizontal(Val::Px(2.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(border())),
        ))
        .id()
}

// ── Snapshots ──────────────────────────────────────────────────────────────────

/// Per-type pin colour (matches the egui editor's `pin_color`).
fn pin_rgb(t: PinType) -> (u8, u8, u8) {
    match t {
        PinType::Float => (0, 212, 170),
        PinType::Vec2 => (127, 204, 25),
        PinType::Vec3 => (255, 215, 0),
        PinType::Vec4 => (255, 102, 255),
        PinType::Color => (255, 200, 60),
        PinType::Bool => (255, 68, 68),
        PinType::Texture2D | PinType::Sampler => (200, 150, 120),
        PinType::String => (180, 110, 200),
    }
}

type Port = (String, String, (u8, u8, u8));

struct NodeSnap {
    id: u64,
    title: String,
    color: (u8, u8, u8),
    pos: [f32; 2],
    inputs: Vec<Port>,
    outputs: Vec<Port>,
    selected: bool,
    tex_path: Option<String>,
    thumb: Option<Handle<Image>>,
}

fn node_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(s) = world.get_resource::<MaterialEditorState>() else { return empty() };
    let assets = world.get_resource::<AssetServer>();
    let sel = s.selected_node;
    let nodes: Vec<NodeSnap> = s
        .graph
        .nodes
        .iter()
        .map(|n| {
            let def = node_def(&n.node_type);
            let title = def.map(|d| d.display_name.to_string()).unwrap_or_else(|| n.node_type.clone());
            let color = def.map(|d| (d.color[0], d.color[1], d.color[2])).unwrap_or((90, 90, 100));
            let pins = def.map(|d| (d.pins)()).unwrap_or_default();
            let inputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Input).map(|p| (p.name.clone(), p.label.clone(), pin_rgb(p.pin_type))).collect();
            let outputs: Vec<Port> = pins.iter().filter(|p| p.direction == PinDir::Output).map(|p| (p.name.clone(), p.label.clone(), pin_rgb(p.pin_type))).collect();
            let tex_path = n.input_values.get("texture").and_then(|v| match v {
                PinValue::TexturePath(p) if !p.is_empty() => Some(p.clone()),
                _ => None,
            });
            let thumb = tex_path.as_ref().and_then(|p| assets.map(|a| a.load::<Image>(p)));
            NodeSnap { id: n.id, title, color, pos: n.position, inputs, outputs, selected: sel == Some(n.id), tex_path, thumb }
        })
        .collect();
    let items: Vec<(u64, u64)> = nodes
        .iter()
        .map(|n| {
            let mut k = hasher();
            n.id.hash(&mut k);
            let mut h = hasher();
            // Structure only (NOT position OR selection) so neither dragging nor
            // selecting rebuilds a node — selection is applied in place by the view.
            (&n.title, n.color, &n.inputs, &n.outputs, &n.tex_path).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let n = &nodes[i];
            graph_node_view(c, f, canvas, viewport, n.id, &n.title, n.color, &n.inputs, &n.outputs, n.pos[0], n.pos[1], n.selected, n.thumb.clone(), &[])
        }),
    }
}

/// Comment boxes, keyed on id only — drag / resize / retitle update in place.
fn comment_snapshot(world: &World, canvas: Entity, viewport: Entity) -> KeyedSnapshot {
    let Some(s) = world.get_resource::<MaterialEditorState>() else { return empty() };
    let comments: Vec<(u64, String, [f32; 4], (u8, u8, u8))> =
        s.graph.comments.iter().map(|c| (c.id, c.text.clone(), c.rect, (c.color[0], c.color[1], c.color[2]))).collect();
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
            // Persist unsaved edits to the outgoing material before switching.
            save_if_dirty(world);
            sync_to_file(world, path.clone());
            // Reflect the standalone document as a single tab.
            let label = file_stem_label(&path);
            let mut st = world.resource_mut::<MaterialEditorState>();
            st.tabs = vec![MaterialTab { entity: None, path: Some(path), label }];
            st.active_tab = Some(0);
        }
        return;
    }

    // Scene mode: the tab set is every distinct material in the selected entity's
    // subtree. A bare model/group root has no material of its own — its meshes
    // (and their `MaterialRef`s) are children — so we walk descendants too.
    let selected = world.get_resource::<EditorSelection>().and_then(|s| s.get());
    let desired = collect_material_tabs(world, selected);

    // Only rebuild when the tab SET actually changes (by entity + path), so
    // re-selecting the same entity or editing the active graph doesn't reset the
    // tabs or the active selection. Leaving asset mode always rebuilds.
    let changed = {
        let st = world.resource::<MaterialEditorState>();
        matches!(st.edit_mode, MaterialEditMode::EditingFile { .. })
            || tab_keys(&st.tabs) != tab_keys(&desired)
    };
    if !changed {
        return;
    }

    // Persist unsaved edits to the outgoing material before swapping it out.
    save_if_dirty(world);

    if desired.is_empty() {
        // Nothing editable under this selection — go idle (matches the editor's
        // prior behaviour for non-mesh selections).
        let mut st = world.resource_mut::<MaterialEditorState>();
        st.tabs.clear();
        st.active_tab = None;
        st.editing_entity = selected;
        st.selected_node = None;
        st.is_dirty = false;
        st.edit_mode = MaterialEditMode::Inactive;
        st.graph = MaterialGraph::new("New Material", renzora_shader::material::graph::MaterialDomain::Surface);
        st.compiled_wgsl = None;
        st.compile_errors.clear();
        return;
    }

    {
        let mut st = world.resource_mut::<MaterialEditorState>();
        st.tabs = desired;
        st.active_tab = Some(0);
    }
    load_material_tab(world, 0);
}

/// Walk `root` and its descendants, collecting one [`MaterialTab`] per distinct
/// material. Meshes that share a `.material` collapse to a single tab (deduped by
/// path); a mesh with no `MaterialRef` yet becomes a `None`-path tab that will
/// save on first edit. No cap — the picker is a scrolling dropdown, so a model
/// with many materials just gets a long (scrollable) list.
fn collect_material_tabs(world: &World, root: Option<Entity>) -> Vec<MaterialTab> {
    let Some(root) = root else { return Vec::new() };
    let mut out: Vec<MaterialTab> = Vec::new();
    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(e) = queue.pop_front() {
        if world.get::<Mesh3d>(e).is_some() {
            match world.get::<MaterialRef>(e).map(|m| m.0.clone()) {
                Some(path) => {
                    if seen_paths.insert(path.clone()) {
                        let label = file_stem_label(&path);
                        out.push(MaterialTab { entity: Some(e), path: Some(path), label });
                    }
                }
                None => {
                    let label = world
                        .get::<Name>(e)
                        .map(|n| n.as_str().to_string())
                        .unwrap_or_else(|| format!("material_{}", e.index()));
                    out.push(MaterialTab { entity: Some(e), path: None, label });
                }
            }
        }
        if let Some(children) = world.get::<Children>(e) {
            for c in children.iter() {
                queue.push_back(c);
            }
        }
    }
    out
}

/// Identity of a tab set used to detect selection changes: the (entity, path)
/// pairs, order-sensitive.
fn tab_keys(tabs: &[MaterialTab]) -> Vec<(Option<Entity>, Option<String>)> {
    tabs.iter().map(|t| (t.entity, t.path.clone())).collect()
}

/// File stem of a `.material` path for a tab label (e.g. `dirt` from
/// `materials/dirt.material`).
fn file_stem_label(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Save the active graph to disk if it has unsaved edits and a backing file.
/// The editor only persists on Apply, so this is called before swapping the
/// loaded material (tab/selection change) to avoid silently dropping edits.
fn save_if_dirty(world: &mut World) {
    let st = world.resource::<MaterialEditorState>();
    let has_path = matches!(
        st.edit_mode,
        MaterialEditMode::Existing { .. } | MaterialEditMode::EditingFile { .. }
    );
    if st.is_dirty && has_path {
        crate::apply_material(world);
    }
}

/// Load `tabs[idx]` into `MaterialEditorState::graph` and set the edit mode.
fn load_material_tab(world: &mut World, idx: usize) {
    let Some(tab) = world.resource::<MaterialEditorState>().tabs.get(idx).cloned() else {
        return;
    };
    {
        let mut st = world.resource_mut::<MaterialEditorState>();
        st.active_tab = Some(idx);
        st.editing_entity = tab.entity;
        st.selected_node = None;
        st.is_dirty = false;
    }

    let Some(path) = tab.path else {
        // Mesh with no MaterialRef yet — start an empty graph that saves on the
        // first edit (drives `MaterialEditMode::Pending`).
        let graph = MaterialGraph::new(&tab.label, renzora_shader::material::graph::MaterialDomain::Surface);
        let result = codegen::compile(&graph);
        let mut st = world.resource_mut::<MaterialEditorState>();
        st.compiled_wgsl = Some(result.fragment_shader);
        st.compile_errors = result.errors;
        st.graph = graph;
        st.edit_mode = match tab.entity {
            Some(entity) => MaterialEditMode::Pending { entity },
            None => MaterialEditMode::Inactive,
        };
        return;
    };

    let fs_path = world
        .get_resource::<CurrentProject>()
        .map(|p| p.resolve_path(&path).to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let loaded = std::fs::read_to_string(&fs_path)
        .ok()
        .and_then(|j| serde_json::from_str::<MaterialGraph>(&j).ok());

    let mut st = world.resource_mut::<MaterialEditorState>();
    match loaded {
        Some(graph) => {
            let result = codegen::compile(&graph);
            st.compiled_wgsl = Some(result.fragment_shader);
            st.compile_errors = result.errors;
            st.graph = graph;
            st.edit_mode = match tab.entity {
                Some(entity) => MaterialEditMode::Existing { path, entity },
                None => MaterialEditMode::EditingFile { path },
            };
        }
        None => {
            warn!("[material_editor] Failed to load '{}', starting fresh", path);
            let name = file_stem_label(&path);
            let graph = MaterialGraph::new(&name, renzora_shader::material::graph::MaterialDomain::Surface);
            let result = codegen::compile(&graph);
            st.compiled_wgsl = Some(result.fragment_shader);
            st.compile_errors = result.errors;
            st.graph = graph;
            st.edit_mode = match tab.entity {
                Some(entity) => MaterialEditMode::Pending { entity },
                None => MaterialEditMode::EditingFile { path },
            };
        }
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
                GraphEdit::Delete { id } => {
                    st.graph.remove_node(id);
                    if st.selected_node == Some(id) {
                        st.selected_node = None;
                    }
                    structural = true;
                }
                GraphEdit::Select { id } => {
                    if st.selected_node != id {
                        st.selected_node = id;
                    }
                }
                // Comments are visual only — persist (dirty) but never recompile.
                GraphEdit::AddComment { rect } => {
                    st.graph.add_comment(rect);
                    dirty = true;
                }
                GraphEdit::CommentMoved { id, x, y } => {
                    if let Some(c) = st.graph.get_comment_mut(id) {
                        c.rect[0] = x;
                        c.rect[1] = y;
                        dirty = true;
                    }
                }
                GraphEdit::CommentResized { id, w, h } => {
                    if let Some(c) = st.graph.get_comment_mut(id) {
                        c.rect[2] = w;
                        c.rect[3] = h;
                        dirty = true;
                    }
                }
                GraphEdit::CommentRetitled { id, text } => {
                    if let Some(c) = st.graph.get_comment_mut(id) {
                        c.text = text;
                        dirty = true;
                    }
                }
                GraphEdit::DeleteComment { id } => {
                    st.graph.remove_comment(id);
                    dirty = true;
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

/// Toolbar view ops just set request flags on the shared widget, which acts on them.
fn view_op_click(q: Query<(&Interaction, &ViewOpBtn), Changed<Interaction>>, mut views: Query<&mut NodeGraphView, With<MatGraph>>) {
    for (i, op) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        for mut v in &mut views {
            match op.0 {
                ViewOp::Fit => v.fit_request = true,
                ViewOp::Center => v.center_request = true,
                ViewOp::ZoomIn => v.zoom_request = Some(1.25),
                ViewOp::ZoomOut => v.zoom_request = Some(0.8),
            }
        }
    }
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
    open_add_menu(&mut commands, &fonts, top_left.x, top_left.y + size.y + 2.0, [60.0, 60.0]);
}

/// Right-click on empty canvas → the shared widget records `context_menu`; open
/// the add-node menu at the cursor and spawn nodes at the clicked canvas point.
fn context_menu_open(fonts: Option<Res<EmberFonts>>, mut commands: Commands, mut views: Query<&mut NodeGraphView, With<MatGraph>>) {
    let Some(fonts) = fonts else { return };
    for mut v in &mut views {
        if let Some((screen, canvas)) = v.context_menu.take() {
            open_add_menu(&mut commands, &fonts, screen.x, screen.y, [canvas.x, canvas.y]);
        }
        // Cable dragged onto empty canvas → palette that auto-wires to the pin.
        if let Some(cd) = v.connect_drag.take() {
            let src = (cd.node, cd.pin, cd.is_output);
            search_menu(&mut commands, &fonts, cd.screen.x, cd.screen.y, mat_connect_entries([cd.canvas.x, cd.canvas.y], src));
        }
    }
}

/// Catalog entries (minus Output) whose action spawns the node and wires it to `src`.
fn mat_connect_entries(base: [f32; 2], src: (u64, String, bool)) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for category in categories() {
        if category == "Output" {
            continue;
        }
        let icon = category_icon(category);
        for def in nodes_in_category(category) {
            let node_type = def.node_type;
            let src = src.clone();
            entries.push(SearchEntry::new(icon, def.display_name, category, move |w| mat_add_and_wire(w, node_type, base, src.clone())));
        }
    }
    entries
}

/// Spawn `node_type` at `base`, wire `src` to its best-matching opposite pin, recompile.
fn mat_add_and_wire(world: &mut World, node_type: &str, base: [f32; 2], src: (u64, String, bool)) {
    let Some(mut s) = world.get_resource_mut::<MaterialEditorState>() else { return };
    let new_id = s.graph.add_node(node_type, base);
    let src_ty = s
        .graph
        .nodes
        .iter()
        .find(|n| n.id == src.0)
        .and_then(|n| node_def(&n.node_type))
        .and_then(|d| (d.pins)().into_iter().find(|p| p.name == src.1).map(|p| p.pin_type));
    let want_dir = if src.2 { PinDir::Input } else { PinDir::Output };
    let new_pins = node_def(node_type).map(|d| (d.pins)()).unwrap_or_default();
    let pick = new_pins
        .iter()
        .filter(|p| p.direction == want_dir)
        .min_by_key(|p| match src_ty {
            Some(t) if p.pin_type == t => 0u8,
            Some(t) if PinType::compatible(t, p.pin_type) || PinType::compatible(p.pin_type, t) => 1,
            _ => 2,
        })
        .map(|p| p.name.clone());
    if let Some(pin) = pick {
        if src.2 {
            s.graph.connect(src.0, &src.1, new_id, &pin);
        } else {
            s.graph.connect(new_id, &pin, src.0, &src.1);
        }
    }
    let graph = s.graph.clone();
    let result = renzora_shader::material::codegen::compile(&graph);
    s.compiled_wgsl = Some(result.fragment_shader);
    s.compile_errors = result.errors;
    s.is_dirty = true;
}

/// Open the searchable add-node palette at `(x, y)`, spawning the chosen node at
/// `base` (canvas px).
fn open_add_menu(commands: &mut Commands, fonts: &EmberFonts, x: f32, y: f32, base: [f32; 2]) {
    search_menu(commands, fonts, x, y, mat_node_entries(base));
}

/// Every catalog node (minus the singleton Output) as a palette entry, each
/// adding the node at `base` and recompiling the shader.
fn mat_node_entries(base: [f32; 2]) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for category in categories() {
        if category == "Output" {
            continue;
        }
        let icon = category_icon(category);
        for def in nodes_in_category(category) {
            let node_type = def.node_type;
            entries.push(SearchEntry::new(icon, def.display_name, category, move |w| {
                if let Some(mut s) = w.get_resource_mut::<MaterialEditorState>() {
                    s.graph.add_node(node_type, base);
                    let graph = s.graph.clone();
                    let result = renzora_shader::material::codegen::compile(&graph);
                    s.compiled_wgsl = Some(result.fragment_shader);
                    s.compile_errors = result.errors;
                    s.is_dirty = true;
                }
            }));
        }
    }
    entries
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Material picker dropdown ─────────────────────────────────────────────────

/// Hash of the tab *labels* — changes when the material set changes, but not when
/// only the active selection changes (that updates the dropdown label in place).
fn material_tabs_sig(state: &MaterialEditorState) -> u64 {
    let mut h = hasher();
    state.tabs.len().hash(&mut h);
    for t in &state.tabs {
        t.label.hash(&mut h);
    }
    h.finish()
}

/// Rebuild the material picker dropdown when the tab set changes. The dropdown
/// bakes its options at build time, so a changed selection (new model → new
/// materials) despawns and rebuilds it; switching the active material within an
/// unchanged set is handled in place by the dropdown's own two-way binding.
fn rebuild_material_dropdown(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut q = world.query::<(Entity, &MaterialTabsRoot)>();
    let roots: Vec<(Entity, u64)> = q.iter(world).map(|(e, r)| (e, r.sig)).collect();

    let (labels, sig, active) = {
        let st = world.resource::<MaterialEditorState>();
        let labels: Vec<String> = st.tabs.iter().map(|t| t.label.clone()).collect();
        (labels, material_tabs_sig(st), st.active_tab.unwrap_or(0))
    };

    for (root, old_sig) in roots {
        if old_sig == sig {
            continue;
        }
        let existing: Vec<Entity> = world
            .get::<Children>(root)
            .map(|c| c.iter().collect())
            .unwrap_or_default();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            if !labels.is_empty() {
                let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                let dd = dropdown(&mut commands, &fonts, &label_refs, active);
                bind_2way(
                    &mut commands,
                    dd,
                    |w: &World| w.get_resource::<MaterialEditorState>().and_then(|s| s.active_tab).unwrap_or(0),
                    |w: &mut World, v: &usize| {
                        let v = *v;
                        if w.resource::<MaterialEditorState>().active_tab == Some(v) {
                            return;
                        }
                        save_if_dirty(w);
                        load_material_tab(w, v);
                    },
                );
                commands.entity(root).add_child(dd);
            }
        }
        queue.apply(world);
        if let Some(mut r) = world.get_mut::<MaterialTabsRoot>(root) {
            r.sig = sig;
        }
    }
}

// ── Selection / document orchestration ───────────────────────────────────────

/// Load (or create) a material graph for a standalone `.material` document tab.
fn sync_to_file(world: &mut World, path: String) {
    let fs_path = if let Some(project) = world.get_resource::<CurrentProject>() {
        project.resolve_path(&path).to_string_lossy().to_string()
    } else {
        path.clone()
    };

    let mut state = world.resource_mut::<MaterialEditorState>();
    state.editing_entity = None;
    state.selected_node = None;
    state.is_dirty = false;

    if let Ok(json) = std::fs::read_to_string(&fs_path) {
        if let Ok(graph) = serde_json::from_str::<MaterialGraph>(&json) {
            let result = codegen::compile(&graph);
            state.compiled_wgsl = Some(result.fragment_shader);
            state.compile_errors = result.errors;
            state.graph = graph;
            state.edit_mode = MaterialEditMode::EditingFile { path };
            return;
        }
    }

    // File missing or unparseable — start a fresh graph named after the file
    // so the user can save it back into place.
    warn!("[material_editor] Failed to load asset '{}', starting fresh", path);
    let name = std::path::Path::new(&path).file_stem().and_then(|s| s.to_str()).unwrap_or("material").to_string();
    let graph = MaterialGraph::new(&name, renzora_shader::material::graph::MaterialDomain::Surface);
    let result = codegen::compile(&graph);
    state.compiled_wgsl = Some(result.fragment_shader);
    state.compile_errors = result.errors;
    state.graph = graph;
    state.edit_mode = MaterialEditMode::EditingFile { path };
}
