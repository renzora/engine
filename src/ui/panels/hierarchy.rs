use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2, Pos2, Stroke, Sense, CursorIcon};

use crate::commands::{CommandHistory, DeleteEntityCommand, queue_command};
use crate::core::{EditorEntity, SelectionState, HierarchyState, HierarchyDropPosition, HierarchyDropTarget, SceneTabId, AssetBrowserState, DefaultCameraEntity};
use crate::node_system::{NodeRegistry, render_node_menu_as_submenus, SceneRoot, NodeTypeMarker};
use crate::plugin_core::{ContextMenuLocation, MenuItem as PluginMenuItem, PluginHost, TabLocation};
use crate::scripting::ScriptComponent;
use crate::ui_api::{UiEvent, renderer::UiRenderer};

// Phosphor icons for hierarchy
use egui_phosphor::regular::{
    CUBE, SPHERE, CYLINDER, SQUARE, LIGHTBULB, SUN, FLASHLIGHT,
    VIDEO_CAMERA, GLOBE, SPEAKER_HIGH, TREE_STRUCTURE, DOTS_THREE_OUTLINE,
    PLUS, TRASH, COPY, ARROW_SQUARE_OUT, PACKAGE, CODE, ATOM,
    CARET_DOWN, CARET_RIGHT, CUBE_TRANSPARENT, FRAME_CORNERS, BROWSERS, FOLDER_SIMPLE,
    CUBE_FOCUS, FILE_CODE, EYE, EYE_SLASH, LOCK_SIMPLE, LOCK_SIMPLE_OPEN, STAR,
};

// Tree line constants
const INDENT_SIZE: f32 = 20.0;
const ROW_HEIGHT: f32 = 24.0;
const TREE_LINE_COLOR: Color32 = Color32::from_rgb(60, 60, 70);
const DROP_LINE_COLOR: Color32 = Color32::from_rgb(80, 140, 255);

fn row_odd_bg() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, 6)
}

fn drop_child_color() -> Color32 {
    Color32::from_rgba_unmultiplied(80, 140, 255, 50)
}

/// Returns (ui_events, actual_width, scene_changed)
pub fn render_hierarchy(
    ctx: &egui::Context,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>, Option<&SceneRoot>, Option<&NodeTypeMarker>)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node_registry: &NodeRegistry,
    active_tab: usize,
    stored_width: f32,
    plugin_host: &PluginHost,
    assets: &mut AssetBrowserState,
    default_camera: &DefaultCameraEntity,
    command_history: &mut CommandHistory,
    ui_renderer: &mut UiRenderer,
) -> (Vec<UiEvent>, f32, bool) {
    let mut ui_events = Vec::new();
    let mut actual_width = stored_width;
    let mut scene_changed = false;

    // Check if a scene file is being dragged
    let dragging_scene = assets.dragging_asset.as_ref()
        .map(|p| p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase() == "scene").unwrap_or(false))
        .unwrap_or(false);

    // Get plugin tabs for left panel
    let api = plugin_host.api();
    let plugin_tabs = api.get_tabs_for_location(TabLocation::Left);
    let active_plugin_tab = api.get_active_tab(TabLocation::Left);

    egui::SidePanel::left("hierarchy")
        .default_width(stored_width)
        .resizable(true)
        .show(ctx, |ui| {
            // Get actual width from the panel
            actual_width = ui.available_width() + 16.0; // Account for panel padding

            // Render tab bar if there are plugin tabs
            if !plugin_tabs.is_empty() {
                ui.horizontal(|ui| {
                    // Built-in Hierarchy tab
                    let hierarchy_selected = active_plugin_tab.is_none();
                    if ui.selectable_label(hierarchy_selected, RichText::new(format!("{} Hierarchy", TREE_STRUCTURE)).size(12.0)).clicked() {
                        // Clear active tab to show hierarchy
                        ui_events.push(UiEvent::PanelTabSelected { location: 0, tab_id: String::new() });
                    }

                    // Plugin tabs
                    for tab in &plugin_tabs {
                        let is_selected = active_plugin_tab == Some(tab.id.as_str());
                        let tab_label = if let Some(icon) = &tab.icon {
                            format!("{} {}", icon, tab.title)
                        } else {
                            tab.title.clone()
                        };
                        if ui.selectable_label(is_selected, RichText::new(&tab_label).size(12.0)).clicked() {
                            ui_events.push(UiEvent::PanelTabSelected { location: 0, tab_id: tab.id.clone() });
                        }
                    }
                });
                ui.separator();
            }

            // Render content based on active tab
            if let Some(tab_id) = active_plugin_tab {
                // Render plugin tab content
                if let Some(widgets) = api.get_tab_content(tab_id) {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for widget in widgets {
                            ui_renderer.render(ui, widget);
                        }
                    });
                } else {
                    ui.label(RichText::new("No content").color(Color32::GRAY));
                }
            } else {
                // Render normal hierarchy
                let (events, changed) = render_hierarchy_content(ui, ctx, selection, hierarchy, entities, commands, meshes, materials, node_registry, active_tab, plugin_host, assets, dragging_scene, default_camera, command_history);
                ui_events.extend(events);
                scene_changed = changed;
            }
        });

    // Show drag tooltip
    if let Some(drag_entity) = hierarchy.drag_entity {
        if let Ok((_, editor_entity, _, _, _, _, _)) = entities.get(drag_entity) {
            if let Some(pos) = ctx.pointer_hover_pos() {
                egui::Area::new(egui::Id::new("hierarchy_drag_tooltip"))
                    .fixed_pos(pos + Vec2::new(10.0, 10.0))
                    .interactable(false)
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            let (icon, color) = get_node_icon(&editor_entity.name, None);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).color(color));
                                ui.label(&editor_entity.name);
                            });
                        });
                    });
            }
        }
    }

    (ui_events, actual_width, scene_changed)
}

/// Render hierarchy content (for use in docking)
/// Returns (ui_events, scene_changed)
pub fn render_hierarchy_content(
    ui: &mut egui::Ui,
    outer_ctx: &egui::Context,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>, Option<&SceneRoot>, Option<&NodeTypeMarker>)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node_registry: &NodeRegistry,
    active_tab: usize,
    plugin_host: &PluginHost,
    assets: &mut AssetBrowserState,
    dragging_scene: bool,
    default_camera: &DefaultCameraEntity,
    command_history: &mut CommandHistory,
) -> (Vec<UiEvent>, bool) {
    let mut ui_events = Vec::new();
    let mut scene_changed = false;
    let ctx = ui.ctx().clone();

    // Find the scene root for current tab
    let scene_root_entity = entities
        .iter()
        .find(|(_, _, _, _, tab_id, scene_root, _)| {
            scene_root.is_some() && tab_id.map_or(false, |t| t.0 == active_tab)
        })
        .map(|(entity, _, _, _, _, _, _)| entity);

    let has_scene_root = scene_root_entity.is_some();

    // Handle scene file drop on hierarchy panel
    if dragging_scene {
        let panel_rect = ui.max_rect();
        if let Some(pos) = outer_ctx.pointer_hover_pos() {
            if panel_rect.contains(pos) {
                // Show drop indicator
                ui.painter().rect_stroke(
                    panel_rect.shrink(4.0),
                    4.0,
                    Stroke::new(2.0, Color32::from_rgb(115, 191, 242)),
                    egui::StrokeKind::Inside,
                );

                // Handle drop on release
                if outer_ctx.input(|i| i.pointer.any_released()) {
                    if let Some(scene_path) = assets.dragging_asset.take() {
                        // Queue the scene drop - parent to scene root
                        assets.pending_scene_drop = Some((scene_path, scene_root_entity));
                    }
                }
            }
        }
    }

    // Compact tab header
    ui.horizontal(|ui| {
        // Tab-style header
        let tab_rect = ui.available_rect_before_wrap();
        let tab_height = 24.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(80.0, tab_height), egui::Sense::hover());

        // Draw tab background
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
            Color32::from_rgb(45, 47, 53),
        );

        // Tab text
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{} Hierarchy", TREE_STRUCTURE),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(200, 200, 210),
        );

        // Add Node button (compact, on the right)
        if has_scene_root {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add_response = ui.add(
                    egui::Button::new(RichText::new(PLUS).size(14.0))
                        .fill(Color32::from_rgb(51, 115, 191))
                        .min_size(Vec2::new(24.0, 20.0)),
                );

                egui::Popup::from_toggle_button_response(&add_response)
                    .show(|ui| {
                        ui.set_min_width(180.0);
                        render_node_menu_as_submenus(ui, node_registry, commands, meshes, materials, scene_root_entity, selection, hierarchy);
                    });
            });
        }
    });

    ui.add_space(4.0);

    // Scene tree
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Collect root entities for current tab (only show entities with matching SceneTabId)
        let root_entities: Vec<_> = entities
            .iter()
            .filter(|(_, _, parent, _, tab_id, _, _)| {
                parent.is_none() && tab_id.map_or(false, |t| t.0 == active_tab)
            })
            .collect();

        if root_entities.is_empty() {
            // No scene root - show scene type selection
            render_scene_type_selection(ui, commands, meshes, materials, selection, hierarchy, active_tab);
        } else {
            // Clear drop target at start of frame
            hierarchy.drop_target = None;

            let root_count = root_entities.len();
            let mut row_index: usize = 0;
            for (i, (entity, editor_entity, _, children, _, _, type_marker)) in root_entities.into_iter().enumerate() {
                let is_last = i == root_count - 1;
                let (events, changed) = render_tree_node(
                    ui,
                    &ctx,
                    selection,
                    hierarchy,
                    entities,
                    commands,
                    meshes,
                    materials,
                    node_registry,
                    entity,
                    editor_entity,
                    children,
                    type_marker,
                    0,
                    is_last,
                    &mut Vec::new(), // No parent lines for root nodes
                    None, // No parent entity for root nodes
                    plugin_host,
                    &mut row_index,
                    default_camera,
                    command_history,
                );
                ui_events.extend(events);
                if changed {
                    scene_changed = true;
                }
            }

            // Handle drop when mouse released
            if ctx.input(|i| i.pointer.any_released()) {
                if let (Some(drag_entity), Some(drop_target)) = (
                    hierarchy.drag_entity.take(),
                    hierarchy.drop_target.take(),
                ) {
                    // Don't drop onto self
                    if drag_entity != drop_target.entity {
                        // Apply the drop and get entity to expand (if any)
                        if let Some(expand_entity) = apply_hierarchy_drop(commands, drag_entity, drop_target, entities) {
                            // Auto-expand parent when dropping as child
                            hierarchy.expanded_entities.insert(expand_entity);
                        }
                    }
                }
            }

            // Clear drag if released without valid target
            if ctx.input(|i| i.pointer.any_released()) {
                hierarchy.drag_entity = None;
                hierarchy.drop_target = None;
            }
        }
    });

    // Render plugin context menu items when right-clicking
    // Get hierarchy context menu items from plugins
    let hierarchy_context_items: Vec<_> = plugin_host.api().context_menus.iter()
        .filter(|(loc, _, _)| *loc == ContextMenuLocation::Hierarchy)
        .map(|(_, item, _)| item)
        .collect();

    // These will be rendered in the tree node context menu, so we just collect them here
    // for now and pass them through. The actual rendering happens in render_tree_node.
    // For simplicity, we store the items in a local to be used by the tree node rendering.
    let _ = hierarchy_context_items; // Used in tree node context menus

    (ui_events, scene_changed)
}

/// Returns (ui_events, scene_changed)
fn render_tree_node(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>, Option<&SceneRoot>, Option<&NodeTypeMarker>)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node_registry: &NodeRegistry,
    entity: Entity,
    editor_entity: &EditorEntity,
    children: Option<&Children>,
    type_marker: Option<&NodeTypeMarker>,
    depth: usize,
    is_last: bool,
    parent_lines: &mut Vec<bool>, // true = draw vertical line at this depth
    _parent_entity: Option<Entity>,
    plugin_host: &PluginHost,
    row_index: &mut usize,
    default_camera: &DefaultCameraEntity,
    command_history: &mut CommandHistory,
) -> (Vec<UiEvent>, bool) {
    let mut ui_events = Vec::new();
    let mut scene_changed = false;
    let is_selected = selection.selected_entity == Some(entity);
    // Only count children that are EditorEntity (not internal Bevy children like mesh handles)
    let has_children = children.map_or(false, |c| {
        c.iter().any(|child| entities.get(child).is_ok())
    });
    let is_expanded = hierarchy.expanded_entities.contains(&entity);
    let is_being_dragged = hierarchy.drag_entity == Some(entity);

    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click_and_drag());
    let painter = ui.painter();

    // Draw odd/even row background
    if *row_index % 2 == 1 {
        painter.rect_filled(rect, 0.0, row_odd_bg());
    }
    *row_index += 1;

    let base_x = rect.min.x + 4.0;
    let center_y = rect.center().y;

    // Handle drag start (unless locked)
    if response.drag_started() && !editor_entity.locked {
        hierarchy.drag_entity = Some(entity);
    }

    // Show drag cursor when dragging
    if hierarchy.drag_entity.is_some() && response.hovered() {
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }

    // Determine drop target based on mouse position
    let mut current_drop_target: Option<HierarchyDropPosition> = None;

    if let Some(drag_entity) = hierarchy.drag_entity {
        if drag_entity != entity && response.hovered() {
            if let Some(pointer_pos) = ctx.pointer_hover_pos() {
                let relative_y = pointer_pos.y - rect.min.y;
                let drop_zone_size = ROW_HEIGHT / 4.0;

                if relative_y < drop_zone_size {
                    // Top zone - insert before
                    current_drop_target = Some(HierarchyDropPosition::Before);
                    hierarchy.drop_target = Some(HierarchyDropTarget {
                        entity,
                        position: HierarchyDropPosition::Before,
                    });
                } else if relative_y > ROW_HEIGHT - drop_zone_size {
                    // Bottom zone - insert after (or as first child if has children and expanded)
                    if has_children && is_expanded {
                        current_drop_target = Some(HierarchyDropPosition::AsChild);
                        hierarchy.drop_target = Some(HierarchyDropTarget {
                            entity,
                            position: HierarchyDropPosition::AsChild,
                        });
                    } else {
                        current_drop_target = Some(HierarchyDropPosition::After);
                        hierarchy.drop_target = Some(HierarchyDropTarget {
                            entity,
                            position: HierarchyDropPosition::After,
                        });
                    }
                } else {
                    // Middle zone - insert as child
                    current_drop_target = Some(HierarchyDropPosition::AsChild);
                    hierarchy.drop_target = Some(HierarchyDropTarget {
                        entity,
                        position: HierarchyDropPosition::AsChild,
                    });
                }
            }
        }
    }

    // Draw drop indicators
    if let Some(drop_pos) = current_drop_target {
        let indent_x = base_x + (depth as f32 * INDENT_SIZE);
        match drop_pos {
            HierarchyDropPosition::Before => {
                // Horizontal line at top with circle indicator
                let y = rect.min.y;
                painter.circle_filled(Pos2::new(indent_x, y), 3.0, DROP_LINE_COLOR);
                painter.line_segment(
                    [Pos2::new(indent_x, y), Pos2::new(rect.max.x - 4.0, y)],
                    Stroke::new(2.0, DROP_LINE_COLOR),
                );
            }
            HierarchyDropPosition::After => {
                // Horizontal line at bottom with circle indicator
                let y = rect.max.y;
                painter.circle_filled(Pos2::new(indent_x, y), 3.0, DROP_LINE_COLOR);
                painter.line_segment(
                    [Pos2::new(indent_x, y), Pos2::new(rect.max.x - 4.0, y)],
                    Stroke::new(2.0, DROP_LINE_COLOR),
                );
            }
            HierarchyDropPosition::AsChild => {
                // Highlight entire row with border
                painter.rect_filled(rect, 3.0, drop_child_color());
                painter.rect_stroke(rect, 3.0, Stroke::new(1.5, DROP_LINE_COLOR), egui::StrokeKind::Outside);
            }
        }
    }

    // Dim the row if it's being dragged or hidden
    if is_being_dragged {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 120));
    } else if !editor_entity.visible {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 60));
    }

    // Draw tree guide lines (draw before content so they appear behind)
    let line_stroke = Stroke::new(1.5, TREE_LINE_COLOR); // Thicker lines for better visibility
    let line_x_offset = INDENT_SIZE / 2.0 - 1.0; // Center the line in the indent area
    let line_overlap = 3.0; // Larger overlap between rows to ensure seamless connections

    // Draw vertical continuation lines for parent levels
    for (level, &has_more_siblings) in parent_lines.iter().enumerate() {
        if has_more_siblings {
            let x = base_x + (level as f32 * INDENT_SIZE) + line_x_offset;
            // Extend beyond row bounds for seamless connection
            painter.line_segment(
                [Pos2::new(x, rect.min.y - line_overlap), Pos2::new(x, rect.max.y + line_overlap)],
                line_stroke,
            );
        }
    }

    // Draw connector for this node (if not root)
    if depth > 0 {
        let x = base_x + ((depth - 1) as f32 * INDENT_SIZE) + line_x_offset;
        let h_end_x = base_x + (depth as f32 * INDENT_SIZE) - 2.0;

        if is_last {
            // └ shape - vertical line from top edge to center
            painter.line_segment(
                [Pos2::new(x, rect.min.y - line_overlap), Pos2::new(x, center_y)],
                line_stroke,
            );
        } else {
            // ├ shape - vertical line full height (extended for seamless connection)
            painter.line_segment(
                [Pos2::new(x, rect.min.y - line_overlap), Pos2::new(x, rect.max.y + line_overlap)],
                line_stroke,
            );
        }

        // Horizontal line from vertical to content
        painter.line_segment(
            [Pos2::new(x, center_y), Pos2::new(h_end_x, center_y)],
            line_stroke,
        );
    }

    // Content starts after tree lines
    let content_x = base_x + (depth as f32 * INDENT_SIZE);

    // Create a child ui for the content
    let content_rect = egui::Rect::from_min_max(
        Pos2::new(content_x, rect.min.y),
        rect.max,
    );

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));

    child_ui.horizontal(|ui| {
        // Expand/collapse button
        if has_children {
            let (icon, icon_color) = if is_expanded {
                (CARET_DOWN, Color32::from_rgb(150, 150, 160))
            } else {
                (CARET_RIGHT, Color32::from_rgb(110, 110, 120))
            };

            let expand_btn = ui.add(
                egui::Button::new(RichText::new(icon).size(11.0).color(icon_color))
                    .frame(false)
                    .min_size(Vec2::new(18.0, 18.0))
            );

            if expand_btn.clicked() {
                if is_expanded {
                    hierarchy.expanded_entities.remove(&entity);
                } else {
                    hierarchy.expanded_entities.insert(entity);
                }
            }
        } else {
            // Empty space for alignment
            ui.add_space(20.0);
        }

        // Icon based on node type or name
        let type_id = type_marker.map(|m| m.type_id);
        let (icon, icon_color) = get_node_icon(&editor_entity.name, type_id);
        ui.label(RichText::new(icon).color(icon_color).size(15.0));

        // Show default camera indicator
        if default_camera.entity == Some(entity) {
            ui.label(RichText::new(STAR).color(Color32::from_rgb(255, 200, 80)).size(11.0));
        }

        // Check if this entity is being renamed
        let is_renaming = hierarchy.renaming_entity == Some(entity);

        if is_renaming {
            // Show text input for renaming
            let text_edit = egui::TextEdit::singleline(&mut hierarchy.rename_buffer)
                .desired_width(120.0)
                .font(egui::TextStyle::Body);

            let response = ui.add(text_edit);

            // Only request focus once when renaming starts
            if !hierarchy.rename_focus_set {
                response.request_focus();
                hierarchy.rename_focus_set = true;
            }

            // Check for Enter key to confirm
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            // Check for Escape key to cancel
            let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

            let mut should_confirm = false;
            let mut should_cancel = false;

            if enter_pressed {
                should_confirm = true;
            } else if escape_pressed {
                should_cancel = true;
            } else if response.lost_focus() {
                // Clicked outside - confirm rename
                should_confirm = true;
            }

            if should_confirm {
                let new_name = hierarchy.rename_buffer.clone();
                if !new_name.is_empty() {
                    commands.entity(entity).insert(EditorEntity {
                        name: new_name,
                        visible: editor_entity.visible,
                        locked: editor_entity.locked,
                    });
                }
                hierarchy.renaming_entity = None;
                hierarchy.rename_buffer.clear();
                hierarchy.rename_focus_set = false;
            } else if should_cancel {
                hierarchy.renaming_entity = None;
                hierarchy.rename_buffer.clear();
                hierarchy.rename_focus_set = false;
            }
        } else {
            // Allocate space for the name and handle interactions manually
            let text = RichText::new(&editor_entity.name).size(13.5);
            let galley = ui.fonts(|f| f.layout_no_wrap(
                editor_entity.name.clone(),
                egui::FontId::proportional(13.5),
                Color32::WHITE,
            ));

            let desired_size = galley.size() + egui::vec2(8.0, 4.0); // padding
            let (rect, name_response) = ui.allocate_exact_size(desired_size, Sense::click());

            // Determine colors based on state
            let (bg_color, text_color) = if is_selected {
                (Color32::from_rgb(51, 115, 191), Color32::WHITE)
            } else if name_response.hovered() {
                (Color32::from_rgba_unmultiplied(255, 255, 255, 20), Color32::from_rgb(218, 218, 225))
            } else {
                (Color32::TRANSPARENT, Color32::from_rgb(218, 218, 225))
            };

            // Draw background
            if bg_color != Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, 2.0, bg_color);
            }

            // Draw text
            let text_pos = rect.min + egui::vec2(4.0, 2.0);
            ui.painter().text(
                text_pos,
                egui::Align2::LEFT_TOP,
                &editor_entity.name,
                egui::FontId::proportional(13.5),
                text_color,
            );

            // Single click to select (unless locked)
            if name_response.clicked() && hierarchy.drag_entity.is_none() && !editor_entity.locked {
                selection.selected_entity = Some(entity);
            }

            // Double click to rename (unless locked)
            if name_response.double_clicked() && hierarchy.drag_entity.is_none() && !editor_entity.locked {
                hierarchy.renaming_entity = Some(entity);
                hierarchy.rename_buffer = editor_entity.name.clone();
                hierarchy.rename_focus_set = false;
            }

            // Right-click context menu (only when not renaming)
            name_response.context_menu(|ui| {
                ui.set_min_width(180.0);

                // Rename option
                if ui.button("✏ Rename").clicked() {
                    hierarchy.renaming_entity = Some(entity);
                    hierarchy.rename_buffer = editor_entity.name.clone();
                    hierarchy.rename_focus_set = false;
                    ui.close();
                }

                ui.separator();

                // Add Child submenu with categories
                ui.menu_button(RichText::new(format!("{} Add Child", PLUS)), |ui| {
                    render_node_menu_as_submenus(ui, node_registry, commands, meshes, materials, Some(entity), selection, hierarchy);
                });

                // Add Script
                if ui.button(format!("{} Add Script", CODE)).clicked() {
                    commands.entity(entity).insert(ScriptComponent {
                        script_id: String::new(),
                        script_path: None,
                        enabled: true,
                        variables: Default::default(),
                        runtime_state: Default::default(),
                    });
                    ui.close();
                }

                // Camera-specific options
                if let Some(marker) = type_marker {
                    if marker.type_id == "camera.camera3d" || marker.type_id == "camera.camera_rig" {
                        ui.separator();
                        if ui.button(format!("{} Make Default Camera", STAR)).clicked() {
                            hierarchy.pending_make_default_camera = Some(entity);
                            ui.close();
                        }
                    }
                }

                ui.separator();

                // Duplicate
                if ui.button(format!("{} Duplicate", COPY)).clicked() {
                    // TODO: Implement duplicate
                    ui.close();
                }

                // Reparent to root
                if ui.button(format!("{} Unparent", ARROW_SQUARE_OUT)).clicked() {
                    commands.entity(entity).remove::<ChildOf>();
                    ui.close();
                }

                ui.separator();

                // Delete
                if ui.button(RichText::new(format!("{} Delete", TRASH)).color(Color32::from_rgb(230, 100, 100))).clicked() {
                    // Queue delete command for undo support
                    queue_command(command_history, Box::new(DeleteEntityCommand::new(entity)));
                    // Remove from expanded set
                    hierarchy.expanded_entities.remove(&entity);
                    scene_changed = true;
                    ui.close();
                }

                // Plugin context menu items
                let hierarchy_items: Vec<_> = plugin_host.api().context_menus.iter()
                    .filter(|(loc, _, _)| *loc == ContextMenuLocation::Hierarchy)
                    .map(|(_, item, _)| item)
                    .collect();

                if !hierarchy_items.is_empty() {
                    ui.separator();
                    for item in hierarchy_items {
                        if render_plugin_context_menu_item(ui, item) {
                            ui_events.push(UiEvent::ButtonClicked(crate::ui_api::UiId(item.id.0)));
                        }
                    }
                }
            });
        }

        // Visibility and Lock icons (right-aligned)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Lock icon
            let lock_icon = if editor_entity.locked { LOCK_SIMPLE } else { LOCK_SIMPLE_OPEN };
            let lock_color = if editor_entity.locked {
                Color32::from_rgb(230, 180, 100)
            } else {
                Color32::from_rgb(90, 90, 100)
            };
            let lock_btn = ui.add(
                egui::Button::new(RichText::new(lock_icon).size(13.0).color(lock_color))
                    .frame(false)
                    .min_size(Vec2::new(18.0, 18.0))
            );
            if lock_btn.clicked() {
                commands.entity(entity).insert(EditorEntity {
                    name: editor_entity.name.clone(),
                    visible: editor_entity.visible,
                    locked: !editor_entity.locked,
                });
            }
            lock_btn.on_hover_text(if editor_entity.locked { "Unlock" } else { "Lock" });

            // Visibility icon
            let vis_icon = if editor_entity.visible { EYE } else { EYE_SLASH };
            let vis_color = if editor_entity.visible {
                Color32::from_rgb(140, 180, 220)
            } else {
                Color32::from_rgb(90, 90, 100)
            };
            let vis_btn = ui.add(
                egui::Button::new(RichText::new(vis_icon).size(13.0).color(vis_color))
                    .frame(false)
                    .min_size(Vec2::new(18.0, 18.0))
            );
            if vis_btn.clicked() {
                let new_visible = !editor_entity.visible;
                commands.entity(entity).insert(EditorEntity {
                    name: editor_entity.name.clone(),
                    visible: new_visible,
                    locked: editor_entity.locked,
                });
                // Also update the Bevy Visibility component
                if new_visible {
                    commands.entity(entity).insert(Visibility::Inherited);
                } else {
                    commands.entity(entity).insert(Visibility::Hidden);
                }
            }
            vis_btn.on_hover_text(if editor_entity.visible { "Hide" } else { "Show" });
        });
    });

    // Render children if expanded
    if has_children && is_expanded {
        if let Some(children) = children {
            let child_entities: Vec<_> = children.iter().collect();
            let child_count = child_entities.len();

            for (i, child_entity) in child_entities.into_iter().enumerate() {
                if let Ok((child, child_editor, _, grandchildren, _, _, child_type_marker)) = entities.get(child_entity) {
                    let child_is_last = i == child_count - 1;

                    // Update parent_lines for children
                    parent_lines.push(!is_last); // Continue vertical line if current node is not last

                    let (child_events, child_changed) = render_tree_node(
                        ui,
                        ctx,
                        selection,
                        hierarchy,
                        entities,
                        commands,
                        meshes,
                        materials,
                        node_registry,
                        child,
                        child_editor,
                        grandchildren,
                        child_type_marker,
                        depth + 1,
                        child_is_last,
                        parent_lines,
                        Some(entity),
                        plugin_host,
                        row_index,
                        default_camera,
                        command_history,
                    );
                    ui_events.extend(child_events);
                    if child_changed {
                        scene_changed = true;
                    }

                    parent_lines.pop();
                }
            }
        }
    }

    (ui_events, scene_changed)
}

/// Render a plugin context menu item, returns true if clicked
fn render_plugin_context_menu_item(ui: &mut egui::Ui, item: &PluginMenuItem) -> bool {
    if item.children.is_empty() {
        let mut text = String::new();
        if let Some(icon) = &item.icon {
            text.push_str(icon);
            text.push(' ');
        }
        text.push_str(&item.label);

        let button = egui::Button::new(&text);
        let response = ui.add_enabled(item.enabled, button);

        if response.clicked() {
            ui.close();
            return true;
        }
    } else {
        let label = if let Some(icon) = &item.icon {
            format!("{} {}", icon, item.label)
        } else {
            item.label.clone()
        };

        ui.menu_button(label, |ui| {
            for child in &item.children {
                render_plugin_context_menu_item(ui, child);
            }
        });
    }

    false
}

/// Apply hierarchy drag and drop - reparent or reorder entity
/// Returns the entity to expand (if dropping as child)
fn apply_hierarchy_drop(
    commands: &mut Commands,
    drag_entity: Entity,
    drop_target: HierarchyDropTarget,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>, Option<&SceneRoot>, Option<&NodeTypeMarker>)>,
) -> Option<Entity> {
    // Get the target's parent
    let target_parent = entities
        .get(drop_target.entity)
        .ok()
        .and_then(|(_, _, parent, _, _, _, _)| parent.map(|p| p.0));

    match drop_target.position {
        HierarchyDropPosition::Before | HierarchyDropPosition::After => {
            // Make sibling of target (same parent)
            if let Some(parent) = target_parent {
                commands.entity(drag_entity).insert(ChildOf(parent));
            } else {
                // Target is at root level, make dragged entity also root
                commands.entity(drag_entity).remove::<ChildOf>();
            }
            None
        }
        HierarchyDropPosition::AsChild => {
            // Make child of target
            commands.entity(drag_entity).insert(ChildOf(drop_target.entity));
            // Return the target entity so it can be expanded
            Some(drop_target.entity)
        }
    }
}

/// Render scene type selection when no scene root exists
fn render_scene_type_selection(
    ui: &mut egui::Ui,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    active_tab: usize,
) {
    use crate::node_system::nodes::{SCENE3D, SCENE2D, UI_ROOT, OTHER_ROOT};
    use crate::core::SceneTabId;

    ui.add_space(20.0);

    ui.vertical_centered(|ui| {
        ui.label(RichText::new("Create New Scene").size(16.0).strong());
        ui.add_space(4.0);
        ui.label(RichText::new("Choose a scene type to begin").weak());
        ui.add_space(16.0);

        let button_width = ui.available_width() - 40.0;
        let button_height = 42.0;

        // 3D Scene button
        let scene3d_btn = ui.add_sized(
            Vec2::new(button_width, button_height),
            egui::Button::new(
                RichText::new(format!("{}  3D Scene", CUBE_TRANSPARENT))
                    .size(14.0)
                    .color(Color32::from_rgb(140, 191, 242))
            )
        );
        if scene3d_btn.clicked() {
            let entity = (SCENE3D.spawn_fn)(commands, meshes, materials, None);
            commands.entity(entity).insert(SceneTabId(active_tab));
            selection.selected_entity = Some(entity);
            hierarchy.expanded_entities.insert(entity);
        }
        ui.add_space(4.0);
        ui.label(RichText::new("For 3D games and applications").weak().size(11.0));

        ui.add_space(12.0);

        // 2D Scene button
        let scene2d_btn = ui.add_sized(
            Vec2::new(button_width, button_height),
            egui::Button::new(
                RichText::new(format!("{}  2D Scene", FRAME_CORNERS))
                    .size(14.0)
                    .color(Color32::from_rgb(191, 140, 242))
            )
        );
        if scene2d_btn.clicked() {
            let entity = (SCENE2D.spawn_fn)(commands, meshes, materials, None);
            commands.entity(entity).insert(SceneTabId(active_tab));
            selection.selected_entity = Some(entity);
            hierarchy.expanded_entities.insert(entity);
        }
        ui.add_space(4.0);
        ui.label(RichText::new("For 2D games and sprites").weak().size(11.0));

        ui.add_space(12.0);

        // UI button
        let ui_btn = ui.add_sized(
            Vec2::new(button_width, button_height),
            egui::Button::new(
                RichText::new(format!("{}  UI", BROWSERS))
                    .size(14.0)
                    .color(Color32::from_rgb(242, 191, 140))
            )
        );
        if ui_btn.clicked() {
            let entity = (UI_ROOT.spawn_fn)(commands, meshes, materials, None);
            commands.entity(entity).insert(SceneTabId(active_tab));
            selection.selected_entity = Some(entity);
            hierarchy.expanded_entities.insert(entity);
        }
        ui.add_space(4.0);
        ui.label(RichText::new("For user interface layouts").weak().size(11.0));

        ui.add_space(12.0);

        // Other button
        let other_btn = ui.add_sized(
            Vec2::new(button_width, button_height),
            egui::Button::new(
                RichText::new(format!("{}  Other", FOLDER_SIMPLE))
                    .size(14.0)
                    .color(Color32::from_rgb(180, 180, 190))
            )
        );
        if other_btn.clicked() {
            let entity = (OTHER_ROOT.spawn_fn)(commands, meshes, materials, None);
            commands.entity(entity).insert(SceneTabId(active_tab));
            selection.selected_entity = Some(entity);
            hierarchy.expanded_entities.insert(entity);
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Generic container for any content").weak().size(11.0));
    });
}

/// Get an icon and color for a node based on its type_id and name
fn get_node_icon(name: &str, type_id: Option<&str>) -> (&'static str, Color32) {
    // First check type_id for accurate icon matching
    if let Some(type_id) = type_id {
        match type_id {
            // Scene roots
            "scene.3d" => return (CUBE_TRANSPARENT, Color32::from_rgb(140, 191, 242)),
            "scene.2d" => return (FRAME_CORNERS, Color32::from_rgb(191, 140, 242)),
            "scene.ui" => return (BROWSERS, Color32::from_rgb(242, 191, 140)),
            "scene.other" => return (FOLDER_SIMPLE, Color32::from_rgb(180, 180, 190)),
            // Scene instance (instanced scene reference)
            "scene.instance" => return (FILE_CODE, Color32::from_rgb(140, 220, 191)),

            // Mesh types
            "mesh.instance" => return (CUBE_FOCUS, Color32::from_rgb(200, 180, 230)),
            "mesh.cube" => return (CUBE, Color32::from_rgb(242, 166, 115)),
            "mesh.sphere" => return (SPHERE, Color32::from_rgb(242, 166, 115)),
            "mesh.cylinder" => return (CYLINDER, Color32::from_rgb(242, 166, 115)),
            "mesh.plane" => return (SQUARE, Color32::from_rgb(242, 166, 115)),

            // Lights
            "light.point" => return (LIGHTBULB, Color32::from_rgb(255, 230, 140)),
            "light.directional" => return (SUN, Color32::from_rgb(255, 230, 140)),
            "light.spot" => return (FLASHLIGHT, Color32::from_rgb(255, 230, 140)),

            // Physics bodies
            "physics.rigidbody3d" => return (ATOM, Color32::from_rgb(166, 242, 200)),
            "physics.staticbody3d" => return (ATOM, Color32::from_rgb(140, 220, 180)),
            "physics.kinematicbody3d" => return (ATOM, Color32::from_rgb(120, 200, 160)),

            // Collision shapes
            "physics.collision_box" => return (CUBE, Color32::from_rgb(166, 242, 200)),
            "physics.collision_sphere" => return (SPHERE, Color32::from_rgb(166, 242, 200)),
            "physics.collision_capsule" => return (CYLINDER, Color32::from_rgb(166, 242, 200)),
            "physics.collision_cylinder" => return (CYLINDER, Color32::from_rgb(166, 242, 200)),

            // Environment
            "env.world" => return (GLOBE, Color32::from_rgb(140, 217, 191)),
            "env.audio_listener" => return (SPEAKER_HIGH, Color32::from_rgb(217, 140, 217)),

            // Camera
            "camera.3d" => return (VIDEO_CAMERA, Color32::from_rgb(140, 191, 242)),

            // Empty node
            "node.empty" => return (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190)),

            _ => {} // Fall through to name-based detection
        }
    }

    // Fallback to name-based detection
    let name_lower = name.to_lowercase();

    // Scene roots
    if name_lower == "scene3d" {
        return (CUBE_TRANSPARENT, Color32::from_rgb(140, 191, 242));
    }
    if name_lower == "scene2d" {
        return (FRAME_CORNERS, Color32::from_rgb(191, 140, 242));
    }
    if name_lower == "ui" {
        return (BROWSERS, Color32::from_rgb(242, 191, 140));
    }
    if name_lower == "root" {
        return (FOLDER_SIMPLE, Color32::from_rgb(180, 180, 190));
    }

    // Physics bodies (check before mesh types since some share words)
    if name_lower.contains("rigidbody") || name_lower.contains("rigid") && name_lower.contains("body") {
        return (ATOM, Color32::from_rgb(166, 242, 200));
    }
    if name_lower.contains("staticbody") || name_lower.contains("static") && name_lower.contains("body") {
        return (ATOM, Color32::from_rgb(140, 220, 180));
    }
    if name_lower.contains("kinematicbody") || name_lower.contains("kinematic") {
        return (ATOM, Color32::from_rgb(120, 200, 160));
    }

    // Collision shapes
    if name_lower.contains("collision") || name_lower.contains("shape3d") {
        if name_lower.contains("box") {
            return (CUBE, Color32::from_rgb(166, 242, 200));
        }
        if name_lower.contains("sphere") {
            return (SPHERE, Color32::from_rgb(166, 242, 200));
        }
        if name_lower.contains("capsule") {
            return (CYLINDER, Color32::from_rgb(166, 242, 200));
        }
        if name_lower.contains("cylinder") {
            return (CYLINDER, Color32::from_rgb(166, 242, 200));
        }
        // Generic collision shape
        return (ATOM, Color32::from_rgb(166, 242, 200));
    }

    // Mesh types
    if name_lower.contains("meshinstance") || (name_lower.contains("mesh") && name_lower.contains("instance")) {
        return (PACKAGE, Color32::from_rgb(166, 217, 242));
    }
    if name_lower.contains("cube") {
        return (CUBE, Color32::from_rgb(242, 166, 115));
    }
    if name_lower.contains("sphere") {
        return (SPHERE, Color32::from_rgb(242, 166, 115));
    }
    if name_lower.contains("cylinder") {
        return (CYLINDER, Color32::from_rgb(242, 166, 115));
    }
    if name_lower.contains("plane") {
        return (SQUARE, Color32::from_rgb(242, 166, 115));
    }

    // Light types
    if name_lower.contains("point") && name_lower.contains("light") {
        return (LIGHTBULB, Color32::from_rgb(255, 230, 140));
    }
    if name_lower.contains("directional") || name_lower.contains("sun") {
        return (SUN, Color32::from_rgb(255, 230, 140));
    }
    if name_lower.contains("spot") && name_lower.contains("light") {
        return (FLASHLIGHT, Color32::from_rgb(255, 230, 140));
    }

    // Camera
    if name_lower.contains("camera") {
        return (VIDEO_CAMERA, Color32::from_rgb(140, 191, 242));
    }

    // Environment
    if name_lower.contains("world") || name_lower.contains("environment") {
        return (GLOBE, Color32::from_rgb(140, 217, 191));
    }
    if name_lower.contains("audio") || name_lower.contains("listener") {
        return (SPEAKER_HIGH, Color32::from_rgb(217, 140, 217));
    }

    // 3D Models (gltf, obj, fbx imports)
    if name_lower.contains("model") || name_lower.ends_with(".glb") || name_lower.ends_with(".gltf")
        || name_lower.ends_with(".obj") || name_lower.ends_with(".fbx") {
        return (CUBE_FOCUS, Color32::from_rgb(200, 180, 230));
    }

    // Default - generic node
    (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190))
}
