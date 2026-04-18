use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Sense, Stroke, Vec2};
use egui_phosphor::regular;
use renzora_blueprint::BlueprintGraph;
use renzora_editor_framework::{DockingState, EditorCommands, EditorSelection, TreeRowConfig};
use renzora_theme::Theme;
use renzora_undo::{self, DeleteShapesCmd, DeletedShape, GroupAsChildrenCmd, LockToggleCmd, RenameCmd, UndoContext, VisibilityToggleCmd};

use crate::state::{EntityNode, HierarchyState};
use crate::LABEL_COLORS;

/// Width reserved for visibility + lock icons in the prefix area.
const PREFIX_WIDTH: f32 = 28.0;

/// Render the entity tree recursively using `tree_row`.
pub fn render_tree(
    ui: &mut egui::Ui,
    nodes: &[EntityNode],
    state: &mut HierarchyState,
    selection: &EditorSelection,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let mut row_index = 0;
    let mut parent_lines = Vec::new();
    let count = nodes.len();

    state.building_entity_order.clear();
    state.row_rects.clear();

    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == count - 1;
        render_node(
            ui,
            node,
            0,
            is_last,
            &mut parent_lines,
            &mut row_index,
            state,
            selection,
            commands,
            theme,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_node(
    ui: &mut egui::Ui,
    node: &EntityNode,
    depth: usize,
    is_last: bool,
    parent_lines: &mut Vec<bool>,
    row_index: &mut usize,
    state: &mut HierarchyState,
    selection: &EditorSelection,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let has_children = !node.children.is_empty();
    let is_expanded = state.expanded.contains(&node.entity);
    let is_selected = selection.is_selected(node.entity);
    let is_dragged = state.drag_entities.contains(&node.entity);
    let is_renaming = state.renaming_entity == Some(node.entity);

    // Track entity order for range selection and marquee row rects
    state.building_entity_order.push(node.entity);

    let result = renzora_editor_framework::tree_row(
        ui,
        &TreeRowConfig {
            stable_id: Some(egui::Id::new(("hierarchy_row", node.entity))),
            depth,
            is_last,
            parent_lines,
            row_index: *row_index,
            has_children,
            is_expanded,
            is_selected,
            icon: Some(node.icon),
            icon_color: Some(node.icon_color),
            label: if is_renaming { "" } else { &node.name },
            label_color: node.label_color,
            theme,
            prefix_width: PREFIX_WIDTH,
        },
    );
    *row_index += 1;
    state.row_rects.push((node.entity, result.rect));

    let painter = ui.painter();

    // === Dim dragged / hidden entities (overlay rect, matching legacy) ===
    if is_dragged {
        painter.rect_filled(result.rect, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 120));
    } else if !node.is_visible {
        painter.rect_filled(result.rect, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 60));
    }

    // === Visibility icon (in prefix area, left slot) ===
    {
        let vis_icon = if node.is_visible { regular::EYE } else { regular::EYE_SLASH };
        let vis_color = if node.is_visible {
            Color32::from_rgb(140, 180, 220)
        } else {
            Color32::from_rgb(90, 90, 100)
        };
        let vis_rect = egui::Rect::from_min_size(
            Pos2::new(result.prefix_rect.min.x, result.rect.min.y),
            Vec2::new(14.0, 20.0),
        );
        let vis_id = ui.id().with(("vis_btn", *row_index));
        let vis_resp = ui.interact(vis_rect, vis_id, Sense::click());
        painter.text(
            vis_rect.center(),
            egui::Align2::CENTER_CENTER,
            vis_icon,
            egui::FontId::proportional(10.0),
            vis_color,
        );
        if vis_resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if vis_resp.clicked() {
            let entity = node.entity;
            let currently_visible = node.is_visible;
            commands.push(move |world: &mut World| {
                renzora_undo::execute(world, UndoContext::Scene, Box::new(VisibilityToggleCmd {
                    entity, was_visible: currently_visible,
                }));
            });
        }
        vis_resp.on_hover_text(if node.is_visible { "Hide" } else { "Show" });
    }

    // === Lock icon (in prefix area, right slot) ===
    {
        let lock_icon = if node.is_locked { regular::LOCK_SIMPLE } else { regular::LOCK_SIMPLE_OPEN };
        let lock_color = if node.is_locked {
            Color32::from_rgb(220, 80, 80)
        } else {
            Color32::from_rgb(90, 90, 100)
        };
        let lock_rect = egui::Rect::from_min_size(
            Pos2::new(result.prefix_rect.min.x + 14.0, result.rect.min.y),
            Vec2::new(14.0, 20.0),
        );
        let lock_id = ui.id().with(("lock_btn", *row_index));
        let lock_resp = ui.interact(lock_rect, lock_id, Sense::click());
        painter.text(
            lock_rect.center(),
            egui::Align2::CENTER_CENTER,
            lock_icon,
            egui::FontId::proportional(10.0),
            lock_color,
        );
        if lock_resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if lock_resp.clicked() {
            let entity = node.entity;
            let currently_locked = node.is_locked;
            commands.push(move |world: &mut World| {
                renzora_undo::execute(world, UndoContext::Scene, Box::new(LockToggleCmd {
                    entity, was_locked: currently_locked,
                }));
            });
        }
        lock_resp.on_hover_text(if node.is_locked { "Unlock" } else { "Lock" });
    }

    // === Default camera star indicator ===
    if node.is_default_camera {
        let star_color = Color32::from_rgb(255, 200, 80);
        let label_x = result.prefix_rect.max.x + 20.0 + 4.0;
        // Measure the label width to position the star after it
        let font_id = egui::FontId::proportional(13.0);
        let label_width = painter.layout_no_wrap(node.name.clone(), font_id, Color32::WHITE).rect.width();
        let star_pos = Pos2::new(label_x + label_width + 4.0, result.rect.center().y);
        painter.text(star_pos, egui::Align2::LEFT_CENTER, regular::STAR, egui::FontId::proportional(10.0), star_color);
    }

    // === Inline rename (replaces label area) ===
    if is_renaming {
        // Calculate where label would be: after prefix + icon area
        let label_x = result.prefix_rect.max.x + 20.0 + 4.0; // icon width + gap
        let rename_rect = egui::Rect::from_min_max(
            Pos2::new(label_x, result.rect.min.y + 1.0),
            Pos2::new(result.rect.max.x - 4.0, result.rect.max.y - 1.0),
        );

        let rename_id = egui::Id::new(("hierarchy_rename", node.entity));
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rename_rect));

        let te = egui::TextEdit::singleline(&mut state.rename_buffer)
            .desired_width(rename_rect.width())
            .font(egui::TextStyle::Body)
            .id(rename_id);
        let resp = child_ui.add(te);

        if !state.rename_focus_set {
            resp.request_focus();
            state.rename_focus_set = true;
        }

        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

        let mut should_confirm = false;
        let mut should_cancel = false;

        if enter_pressed {
            should_confirm = true;
        } else if escape_pressed {
            should_cancel = true;
        } else if resp.lost_focus() {
            // Clicked outside — confirm
            should_confirm = true;
        }

        if should_confirm {
            let entity = node.entity;
            let new_name = state.rename_buffer.clone();
            if !new_name.is_empty() {
                commands.push(move |world: &mut World| {
                    let old = world.get::<Name>(entity).map(|n| n.as_str().to_string())
                        .unwrap_or_default();
                    if old == new_name { return; }
                    renzora_undo::execute(world, UndoContext::Scene, Box::new(RenameCmd {
                        entity, old, new: new_name,
                    }));
                });
            }
            state.renaming_entity = None;
            state.rename_buffer.clear();
            state.rename_focus_set = false;
        }
        if should_cancel {
            state.renaming_entity = None;
            state.rename_buffer.clear();
            state.rename_focus_set = false;
        }
    }

    // === Click handling ===
    if result.clicked && state.drag_entities.is_empty() && !node.is_locked {
        if result.ctrl_click {
            selection.toggle(node.entity);
        } else if result.shift_click {
            let visible_order = state.visible_entity_order.clone();
            if let Some(anchor) = selection.get() {
                selection.select_range(&visible_order, anchor, node.entity);
            } else {
                selection.set(Some(node.entity));
            }
        } else {
            selection.set(Some(node.entity));
            // Auto-expand on click if has children and not expanded (legacy behavior)
            if has_children && !is_expanded {
                state.expanded.insert(node.entity);
            }
        }
    }

    // === Double-click → rename (unless locked) ===
    if result.response.double_clicked() && state.drag_entities.is_empty() && !node.is_locked {
        state.renaming_entity = Some(node.entity);
        state.rename_buffer = node.name.clone();
        state.rename_focus_set = false;
    }

    // === Expand/collapse ===
    if result.expand_toggled {
        if state.expanded.contains(&node.entity) {
            state.expanded.remove(&node.entity);
        } else {
            state.expanded.insert(node.entity);
        }
    }

    // === Drag start (unless locked or marquee active) ===
    if result.response.drag_started() && !node.is_locked && state.marquee_origin.is_none() {
        if is_selected && selection.has_multi_selection() {
            state.drag_entities = selection.get_all();
        } else {
            if !result.ctrl_click && !result.shift_click {
                selection.set(Some(node.entity));
            }
            state.drag_entities = vec![node.entity];
        }
    }

    // === Show drag cursor ===
    if !state.drag_entities.is_empty() && result.response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }

    // === Drop zone detection ===
    if !state.drag_entities.is_empty() {
        if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
            let is_self_dragged = state.drag_entities.contains(&node.entity);
            if !is_self_dragged && result.rect.contains(pointer_pos) {
                let relative_y = pointer_pos.y - result.rect.min.y;
                let edge_zone = 20.0 / 3.0;

                let drop_pos = if relative_y < edge_zone {
                    renzora_editor_framework::TreeDropZone::Before
                } else if relative_y > 20.0 - edge_zone {
                    renzora_editor_framework::TreeDropZone::After
                } else {
                    renzora_editor_framework::TreeDropZone::AsChild
                };

                state.drop_target = Some((node.entity, drop_pos));

                // Draw drop indicator on foreground layer
                let fg = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("hierarchy_drop_indicator"),
                ));
                let accent = theme.semantic.accent.to_color32();

                match drop_pos {
                    renzora_editor_framework::TreeDropZone::Before => {
                        let y = result.rect.min.y + 1.0;
                        fg.line_segment(
                            [Pos2::new(result.rect.min.x, y), Pos2::new(result.rect.max.x, y)],
                            Stroke::new(3.0, accent),
                        );
                    }
                    renzora_editor_framework::TreeDropZone::After => {
                        let y = result.rect.max.y - 1.0;
                        fg.line_segment(
                            [Pos2::new(result.rect.min.x, y), Pos2::new(result.rect.max.x, y)],
                            Stroke::new(3.0, accent),
                        );
                    }
                    renzora_editor_framework::TreeDropZone::AsChild => {
                        // Border drawn after children (see group_top handling below)
                    }
                }
            }
        }
    }

    // Track AsChild group border top
    let is_as_child_target = state.drop_target.as_ref().map_or(false, |(e, z)| {
        *e == node.entity && *z == renzora_editor_framework::TreeDropZone::AsChild
    });
    let group_top = if is_as_child_target {
        Some(result.rect.min.y)
    } else {
        None
    };

    // === Context menu ===
    if state.renaming_entity != Some(node.entity) {
        result.response.context_menu(|ui| {
            context_menu(ui, node, state, selection, commands, theme);
        });
    }

    // === Recurse into children ===
    if is_expanded {
        let child_count = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == child_count - 1;
            parent_lines.push(!is_last);
            render_node(
                ui,
                child,
                depth + 1,
                child_is_last,
                parent_lines,
                row_index,
                state,
                selection,
                commands,
                theme,
            );
            parent_lines.pop();
        }
    }

    // Draw AsChild group border after children
    if let Some(top) = group_top {
        let bottom = ui.cursor().top();
        let fg = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("hierarchy_drop_indicator"),
        ));
        let accent = theme.semantic.accent.to_color32();
        let group_rect = egui::Rect::from_min_max(
            Pos2::new(result.rect.min.x, top),
            Pos2::new(result.rect.max.x, bottom),
        );
        fg.rect_stroke(group_rect, 2.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
    }
}

fn context_menu(
    ui: &mut egui::Ui,
    node: &EntityNode,
    state: &mut HierarchyState,
    selection: &EditorSelection,
    commands: &EditorCommands,
    theme: &Theme,
) {
    ui.set_min_width(180.0);

    // Rename
    if ui.button(format!("{} Rename", regular::PENCIL_SIMPLE)).clicked() {
        state.renaming_entity = Some(node.entity);
        state.rename_buffer = node.name.clone();
        state.rename_focus_set = false;
        ui.close();
    }

    ui.separator();

    // Add Child Entity
    if ui.button(format!("{} Add Child Entity", regular::PLUS)).clicked() {
        let parent = node.entity;
        commands.push(move |world: &mut World| {
            let child = world.spawn((Name::new("New Entity"), Transform::default())).id();
            world.entity_mut(child).set_parent_in_place(parent);
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(Some(child));
            }
        });
        ui.close();
    }

    // Add Component — selects this entity and asks the context-menu plugin
    // to open its component picker overlay at the cursor.
    if ui.button(format!("{} Add Component…", regular::PLUS_CIRCLE)).clicked() {
        let entity = node.entity;
        let pos = ui
            .ctx()
            .pointer_latest_pos()
            .unwrap_or(egui::pos2(100.0, 100.0));
        let screen_pos = bevy::math::Vec2::new(pos.x, pos.y);
        commands.push(move |world: &mut World| {
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(Some(entity));
            }
            world.insert_resource(
                renzora_editor_framework::OpenAddComponentMenuRequest { screen_pos },
            );
        });
        ui.close();
    }

    // ── Blueprint ──
    ui.separator();
    if node.has_blueprint {
        if ui.button(format!("{} Edit Blueprint", regular::FLOW_ARROW)).clicked() {
            let entity = node.entity;
            commands.push(move |world: &mut World| {
                if let Some(sel) = world.get_resource::<EditorSelection>() {
                    sel.set(Some(entity));
                }
                if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
                    docking.tree.set_active_tab("blueprint_graph");
                }
            });
            ui.close();
        }
    } else {
        if ui.button(format!("{} Add Blueprint", regular::FLOW_ARROW)).clicked() {
            let entity = node.entity;
            commands.push(move |world: &mut World| {
                let mut graph = BlueprintGraph::new();
                graph.add_node("event/on_update", [0.0, 0.0]);
                world.entity_mut(entity).insert(graph);
                if let Some(sel) = world.get_resource::<EditorSelection>() {
                    sel.set(Some(entity));
                }
                if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
                    docking.tree.set_active_tab("blueprint_graph");
                }
            });
            ui.close();
        }
    }

    ui.separator();

    // Duplicate
    if ui.button(format!("{} Duplicate", regular::COPY)).clicked() {
        let entity = node.entity;
        let name = format!("{} (Copy)", node.name);
        commands.push(move |world: &mut World| {
            let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
            let new_entity = world.spawn((Name::new(name), Transform::default())).id();
            if let Some(p) = parent {
                world.entity_mut(new_entity).set_parent_in_place(p);
            }
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(Some(new_entity));
            }
        });
        ui.close();
    }

    // Unparent
    if ui.button(format!("{} Unparent", regular::ARROW_SQUARE_OUT)).clicked() {
        let entity = node.entity;
        commands.push(move |world: &mut World| {
            world.entity_mut(entity).remove::<ChildOf>();
        });
        ui.close();
    }

    // Multi-selection actions
    if selection.has_multi_selection() {
        if ui.button(format!("{} Group as Children", regular::FOLDER_SIMPLE)).clicked() {
            let selected = selection.get_all();
            commands.push(move |world: &mut World| {
                let members: Vec<(Entity, Option<Entity>)> = selected.iter()
                    .map(|e| (*e, world.get::<ChildOf>(*e).map(|c| c.parent())))
                    .collect();
                renzora_undo::execute(world, UndoContext::Scene, Box::new(GroupAsChildrenCmd {
                    parent: Entity::PLACEHOLDER,
                    group_name: "Group".to_string(),
                    members,
                }));
            });
            ui.close();
        }

        if ui.button(format!("{} Batch Rename…", regular::TEXT_AA)).clicked() {
            state.batch_rename_active = true;
            state.batch_rename_base = node.name.clone();
            state.batch_rename_start = 1;
            state.batch_rename_entities = selection.get_all();
            ui.close();
        }
    }

    ui.separator();

    // Label Color inline picker (swatch grid, matching legacy)
    let current_label_color = node.label_color;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(format!("{} Label Color", regular::PALETTE)).size(13.0));
        if current_label_color.is_some() {
            ui.add_space(4.0);
            let clear_resp = ui.add(
                egui::Button::new(egui::RichText::new(regular::X).size(10.0))
                    .frame(false)
                    .min_size(Vec2::new(14.0, 14.0)),
            );
            if clear_resp.on_hover_text("Clear").clicked() {
                let entity = node.entity;
                commands.push(move |world: &mut World| {
                    world.entity_mut(entity).remove::<renzora_editor_framework::EntityLabelColor>();
                });
                ui.close();
            }
        }
    });
    // Render swatches in 2 rows (10 + 9)
    for row in [&LABEL_COLORS[..10], &LABEL_COLORS[10..]] {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
            for &(color, name) in row {
                let c32 = Color32::from_rgb(color[0], color[1], color[2]);
                let is_current = current_label_color == Some(color);
                let (swatch_rect, swatch_resp) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::click());
                ui.painter().rect_filled(swatch_rect.shrink(1.0), 2.0, c32);
                if is_current {
                    ui.painter().rect_stroke(swatch_rect, 2.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside);
                } else if swatch_resp.hovered() {
                    ui.painter().rect_stroke(swatch_rect, 2.0, Stroke::new(1.0, Color32::from_rgb(200, 200, 200)), egui::StrokeKind::Outside);
                }
                if swatch_resp.on_hover_text(name).clicked() {
                    let entity = node.entity;
                    commands.push(move |world: &mut World| {
                        world.entity_mut(entity).insert(renzora_editor_framework::EntityLabelColor(color));
                    });
                    ui.close();
                }
            }
        });
    }

    // Camera-specific options
    if node.is_camera {
        ui.separator();
        let label = if node.is_default_camera {
            format!("{} Default Camera", regular::STAR)
        } else {
            format!("{} Set as Default Camera", regular::STAR)
        };
        let button = ui.add_enabled(!node.is_default_camera, egui::Button::new(label));
        if button.clicked() {
            let entity = node.entity;
            commands.push(move |world: &mut World| {
                // Remove DefaultCamera from all other entities
                let mut to_remove = Vec::new();
                for archetype in world.archetypes().iter() {
                    for arch_entity in archetype.entities() {
                        let e = arch_entity.id();
                        if e != entity && world.get::<renzora::core::DefaultCamera>(e).is_some() {
                            to_remove.push(e);
                        }
                    }
                }
                for e in to_remove {
                    world.entity_mut(e).remove::<renzora::core::DefaultCamera>();
                }
                world.entity_mut(entity).insert(renzora::core::DefaultCamera);
            });
            ui.close();
        }

        // Snap to Viewport — copy editor camera transform to this scene camera
        if ui.button(format!("{} Snap to Viewport", regular::FRAME_CORNERS)).clicked() {
            let entity = node.entity;
            commands.push(move |world: &mut World| {
                // Read the editor camera's transform
                let editor_transform = {
                    let mut q = world.query_filtered::<&Transform, With<renzora::core::EditorCamera>>();
                    q.iter(world).next().copied()
                };
                if let Some(t) = editor_transform {
                    if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                        *transform = t;
                    }
                }
            });
            ui.close();
        }
    }

    ui.separator();

    // Instance Scene… — pick a .ron scene file and spawn it as a nested
    // scene instance under this entity.
    if ui.button(format!("{} Instance Scene…", regular::FILM_STRIP)).clicked() {
        let parent_entity = node.entity;
        commands.push(move |world: &mut World| {
            let scenes_dir = world
                .get_resource::<renzora::core::CurrentProject>()
                .map(|p| p.resolve_path("scenes"));

            #[cfg(not(target_arch = "wasm32"))]
            let file = {
                let mut dlg = rfd::FileDialog::new()
                    .set_title("Instance Scene")
                    .add_filter("Scene File", &["ron"]);
                if let Some(ref dir) = scenes_dir {
                    dlg = dlg.set_directory(dir);
                }
                dlg.pick_file()
            };
            #[cfg(target_arch = "wasm32")]
            let file: Option<std::path::PathBuf> = None;

            let Some(path) = file else { return };
            let host_abs = world
                .get_resource::<renzora::core::CurrentProject>()
                .and_then(|p| {
                    world.get_resource::<renzora_ui::DocumentTabState>()
                        .and_then(|t| t.tabs.get(t.active_tab)
                            .and_then(|tab| tab.scene_path.clone()))
                        .map(|rel| p.resolve_path(&rel))
                });
            if let (Some(host_abs), Some(project_root)) = (
                host_abs,
                world.get_resource::<renzora::core::CurrentProject>()
                    .map(|p| p.path.clone()),
            ) {
                let mut cache = world
                    .remove_resource::<renzora_engine::scene_io::SceneReferenceCache>()
                    .unwrap_or_default();
                let cycle = renzora_engine::scene_io::would_create_reference_cycle(
                    &mut cache, &project_root, &host_abs, &path,
                );
                world.insert_resource(cache);
                if cycle {
                    if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
                        toasts.warning("You cannot add a scene to itself");
                    }
                    return;
                }
            }
            let transform = Transform::default();
            if let Some(entity) = renzora_engine::scene_io::spawn_scene_instance(
                world,
                &path,
                Some(parent_entity),
                transform,
            ) {
                if let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() {
                    sel.set(Some(entity));
                }
            }
        });
        ui.close();
    }

    // Unpack Scene Instance — only shown when this entity IS an instance root.
    // Removes the `SceneInstance` marker; its descendants stay in the world
    // and become scene-owned entities (saved directly in the host scene from
    // then on, no longer pulled from the source file).
    if node.is_scene_instance {
        if ui.button(format!("{} Unpack Scene Instance", regular::LINK_BREAK)).clicked() {
            let entity = node.entity;
            commands.push(move |world: &mut World| {
                world.entity_mut(entity).remove::<renzora::SceneInstance>();
                renzora::core::console_log::console_info(
                    "Scene",
                    format!("Unpacked scene instance {:?}", entity),
                );
            });
            ui.close();
        }
    }

    ui.separator();

    // Delete
    if ui.button(egui::RichText::new(format!("{} Delete", regular::TRASH)).color(
        theme.semantic.error.to_color32(),
    )).clicked() {
        let entities: Vec<Entity> = if selection.is_selected(node.entity) {
            selection.get_all()
        } else {
            vec![node.entity]
        };
        // Remove from expanded set before moving into closure
        for entity in &entities {
            state.expanded.remove(entity);
        }
        commands.push(move |world: &mut World| {
            let mut items = Vec::new();
            let mut other = Vec::new();
            for entity in &entities {
                let shape = world.get_entity(*entity).ok().and_then(|e| {
                    let shape_id = e.get::<renzora::core::MeshPrimitive>()?.0.clone();
                    let name = e.get::<Name>()?.as_str().to_string();
                    let transform = *e.get::<Transform>()?;
                    let color = e.get::<renzora::core::MeshColor>()?.0;
                    Some(DeletedShape { entity: *entity, shape_id, name, transform, color })
                });
                match shape {
                    Some(item) => items.push(item),
                    None => other.push(*entity),
                }
            }
            // Non-shape entities: despawn directly, not undoable for now.
            for e in other {
                if let Ok(em) = world.get_entity_mut(e) { em.despawn(); }
            }
            if let Some(sel) = world.get_resource::<EditorSelection>() { sel.clear(); }
            if items.is_empty() { return; }
            renzora_undo::execute(world, UndoContext::Scene, Box::new(DeleteShapesCmd { items }));
        });
        ui.close();
    }
}
