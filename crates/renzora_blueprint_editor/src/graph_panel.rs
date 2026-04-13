//! Blueprint Graph Panel — shows the node graph for the selected entity's blueprint.

use std::cell::RefCell;

use bevy::prelude::*;
use renzora::bevy_egui::egui::{self, RichText};
use renzora::egui_phosphor::regular::{
    PLUS, CROSSHAIR, MAGNIFYING_GLASS_PLUS, MAGNIFYING_GLASS_MINUS,
    FLOW_ARROW, CUBE, LIGHTNING, EXPORT,
};

use renzora::editor::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora::theme::ThemeManager;
use renzora_blueprint::BlueprintGraph;

use crate::BlueprintEditorState;
use crate::graph_editor::{self, GraphEditorState};

thread_local! {
    static GRAPH_EDITOR_STATE: RefCell<GraphEditorState> = RefCell::new(GraphEditorState::default());
}

#[derive(Default)]
pub struct BlueprintGraphPanel;

impl EditorPanel for BlueprintGraphPanel {
    fn id(&self) -> &str {
        "blueprint_graph"
    }

    fn title(&self) -> &str {
        "Blueprint"
    }

    fn icon(&self) -> Option<&str> {
        Some(FLOW_ARROW)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let cmds = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let Some(bp_state) = world.get_resource::<BlueprintEditorState>() else {
            return;
        };
        let selection = world.get_resource::<EditorSelection>();
        let project_path = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.path.clone());

        let selected_entity = selection.and_then(|s| s.get());

        // Track if the selected entity changed
        if selected_entity != bp_state.editing_entity {
            let new_entity = selected_entity;
            cmds.push(move |world: &mut World| {
                world.resource_mut::<BlueprintEditorState>().editing_entity = new_entity;
            });
            GRAPH_EDITOR_STATE.with(|cell| {
                cell.borrow_mut().needs_sync = true;
            });
        }

        let Some(entity) = selected_entity else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("Select an entity to edit its blueprint")
                        .size(14.0)
                        .color(theme.text.muted.to_color32()),
                );
            });
            return;
        };

        // Get the entity's name
        let entity_name = world
            .get::<Name>(entity)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| format!("Entity {}", entity.index()));

        // Get or create the blueprint graph
        let has_blueprint = world.get::<BlueprintGraph>(entity).is_some();
        let mut graph = if let Some(bp) = world.get::<BlueprintGraph>(entity) {
            bp.clone()
        } else {
            BlueprintGraph::new()
        };

        // ── Toolbar ──
        let toolbar_modified = GRAPH_EDITOR_STATE.with(|cell| {
            let mut gs = cell.borrow_mut();
            render_toolbar(ui, &mut graph, &mut gs, cmds, &entity_name, has_blueprint, entity, &theme, &project_path)
        });

        ui.separator();

        if !has_blueprint && !toolbar_modified {
            // No blueprint yet — show prompt
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(format!("{LIGHTNING}"))
                        .size(48.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("\"{}\" has no blueprint", entity_name))
                        .size(14.0)
                        .color(theme.text.secondary.to_color32()),
                );
                ui.add_space(8.0);
                if ui
                    .button(
                        RichText::new(format!("{PLUS} Create Blueprint"))
                            .size(13.0)
                            .color(theme.semantic.accent.to_color32()),
                    )
                    .clicked()
                {
                    let mut new_graph = BlueprintGraph::new();
                    // Add a default OnUpdate event node
                    new_graph.add_node("event/on_update", [0.0, 0.0]);
                    cmds.push(move |world: &mut World| {
                        world.entity_mut(entity).insert(new_graph);
                    });
                    GRAPH_EDITOR_STATE.with(|cell| {
                        cell.borrow_mut().needs_sync = true;
                    });
                }
            });
            return;
        }

        // ── Graph canvas ──
        let (graph_modified, selected) = GRAPH_EDITOR_STATE.with(|cell| {
            let mut gs = cell.borrow_mut();
            let m = graph_editor::render_graph_editor(ui, &mut graph, &mut gs, &theme);
            let sel = if gs.widget_state.selected.len() == 1 {
                Some(gs.widget_state.selected[0])
            } else {
                None
            };
            (m, sel)
        });

        // Sync selection
        if selected != bp_state.selected_node {
            let sel = selected;
            cmds.push(move |world: &mut World| {
                world.resource_mut::<BlueprintEditorState>().selected_node = sel;
            });
        }

        if graph_modified || toolbar_modified {
            let updated_graph = graph.clone();
            cmds.push(move |world: &mut World| {
                world.entity_mut(entity).insert(updated_graph);
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

/// Render toolbar. Returns true if graph was modified.
fn render_toolbar(
    ui: &mut egui::Ui,
    graph: &mut BlueprintGraph,
    state: &mut GraphEditorState,
    cmds: &EditorCommands,
    entity_name: &str,
    has_blueprint: bool,
    entity: Entity,
    theme: &renzora::theme::Theme,
    project_path: &Option<std::path::PathBuf>,
) -> bool {
    let mut modified = false;
    let text_color = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.style_mut().spacing.button_padding = egui::vec2(6.0, 2.0);

        // ── Entity name ──
        ui.label(
            RichText::new(format!("{CUBE} {entity_name}"))
                .size(12.0)
                .color(text_color),
        );

        ui.separator();

        if !has_blueprint {
            return;
        }

        // ── Add Node ──
        let add_btn = ui.add(egui::Button::new(
            RichText::new(format!("{PLUS} Add Node")).size(12.0).color(accent),
        ));
        let add_id = ui.make_persistent_id("bp_add_node_popup");
        if add_btn.clicked() {
            #[allow(deprecated)]
            ui.memory_mut(|m| m.toggle_popup(add_id));
        }
        #[allow(deprecated)]
        egui::popup_below_widget(
            ui,
            add_id,
            &add_btn,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(160.0);
                for &category in &renzora_blueprint::categories() {
                    let icon = graph_editor::category_icon(category);
                    ui.menu_button(format!("{icon} {category}"), |ui| {
                        let defs = renzora_blueprint::nodes_in_category(category);
                        for def in &defs {
                            let btn = ui.button(format!("{icon} {}", def.display_name))
                                .on_hover_text(def.description);
                            if btn.clicked() {
                                let canvas_pos = if let Some(cr) = state.canvas_rect {
                                    graph_editor::screen_to_canvas(
                                        cr.center(),
                                        state.widget_state.offset,
                                        state.widget_state.zoom,
                                        cr,
                                    )
                                } else {
                                    [0.0, 0.0]
                                };
                                graph.add_node(def.node_type, canvas_pos);
                                state.needs_sync = true;
                                modified = true;
                                #[allow(deprecated)]
                                ui.memory_mut(|m| m.toggle_popup(add_id));
                            }
                        }
                    });
                }
            },
        );

        ui.separator();

        // ── Center ──
        if ui
            .add(egui::Button::new(
                RichText::new(format!("{CROSSHAIR}")).size(12.0).color(text_muted),
            ))
            .on_hover_text("Center Graph")
            .clicked()
        {
            if !graph.nodes.is_empty() {
                let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
                let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
                for node in &graph.nodes {
                    min_x = min_x.min(node.position[0]);
                    max_x = max_x.max(node.position[0]);
                    min_y = min_y.min(node.position[1]);
                    max_y = max_y.max(node.position[1]);
                }
                state.widget_state.offset = [-(min_x + max_x) / 2.0, -(min_y + max_y) / 2.0];
            } else {
                state.widget_state.offset = [0.0, 0.0];
            }
        }

        // ── Zoom ──
        if ui
            .add(egui::Button::new(
                RichText::new(format!("{MAGNIFYING_GLASS_MINUS}"))
                    .size(12.0)
                    .color(text_muted),
            ))
            .on_hover_text("Zoom Out")
            .clicked()
        {
            state.widget_state.zoom = (state.widget_state.zoom * 0.8).max(0.25);
        }

        let zoom_pct = (state.widget_state.zoom * 100.0) as u32;
        ui.label(RichText::new(format!("{zoom_pct}%")).size(11.0).color(text_muted));

        if ui
            .add(egui::Button::new(
                RichText::new(format!("{MAGNIFYING_GLASS_PLUS}"))
                    .size(12.0)
                    .color(text_muted),
            ))
            .on_hover_text("Zoom In")
            .clicked()
        {
            state.widget_state.zoom = (state.widget_state.zoom * 1.25).min(4.0);
        }

        // ── Node count + Apply ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // ── Apply (compile to Lua) ──
            if project_path.is_some() {
                let apply_btn = ui.add(egui::Button::new(
                    RichText::new(format!("{EXPORT} Apply")).size(12.0).color(accent),
                ));
                if apply_btn.on_hover_text("Compile blueprint to Lua script and attach to entity").clicked() {
                    let compiled_graph = graph.clone();
                    let proj = project_path.clone().unwrap();
                    let ent_name = entity_name.to_string();
                    cmds.push(move |world: &mut World| {
                        apply_blueprint_to_lua(world, entity, &compiled_graph, &proj, &ent_name);
                    });
                }
                ui.separator();
            }

            let node_count = graph.nodes.len();
            let conn_count = graph.connections.len();
            ui.label(
                RichText::new(format!("{FLOW_ARROW} {node_count} nodes, {conn_count} connections"))
                    .size(11.0)
                    .color(text_muted),
            );
        });
    });

    modified
}

/// Compile a blueprint graph to Lua, save it to the project scripts folder,
/// and attach a ScriptComponent pointing to the generated file.
fn apply_blueprint_to_lua(
    world: &mut World,
    entity: Entity,
    graph: &BlueprintGraph,
    project_path: &std::path::Path,
    entity_name: &str,
) {
    use renzora_scripting::ScriptComponent;

    // Compile
    let lua_source = renzora_blueprint::compiler::compile_to_lua(graph);

    // Sanitize entity name for filename
    let safe_name: String = entity_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    let file_name = format!("bp_{}.lua", safe_name.to_lowercase());

    // Ensure scripts directory exists
    let scripts_dir = project_path.join("scripts");
    if let Err(e) = std::fs::create_dir_all(&scripts_dir) {
        warn!("Failed to create scripts dir: {}", e);
        return;
    }

    // Write the file
    let file_path = scripts_dir.join(&file_name);
    if let Err(e) = std::fs::write(&file_path, &lua_source) {
        warn!("Failed to write compiled blueprint: {}", e);
        return;
    }

    info!(
        "Blueprint compiled to Lua: {} ({} bytes)",
        file_path.display(),
        lua_source.len()
    );

    // Attach or update ScriptComponent
    let script_rel_path = std::path::PathBuf::from(format!("scripts/{}", file_name));
    if let Some(mut sc) = world.get_mut::<ScriptComponent>(entity) {
        // Check if this blueprint script is already attached
        let existing = sc.scripts.iter().position(|e| {
            e.script_path.as_ref().map(|p| p.ends_with(&file_name)).unwrap_or(false)
        });
        if let Some(idx) = existing {
            // Update path (forces reload) and reset runtime state
            sc.scripts[idx].script_path = Some(script_rel_path);
            sc.scripts[idx].runtime_state = Default::default();
        } else {
            sc.add_file_script(script_rel_path);
        }
    } else {
        world
            .entity_mut(entity)
            .insert(ScriptComponent::from_file(script_rel_path));
    }
}
