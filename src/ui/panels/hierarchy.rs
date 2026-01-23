use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2, Pos2, Stroke, Sense, CursorIcon};

use crate::core::{EditorEntity, SelectionState, HierarchyState, HierarchyDropPosition, HierarchyDropTarget, SceneTabId};
use crate::node_system::{NodeRegistry, render_node_menu_items};
use crate::plugin_core::{ContextMenuLocation, MenuItem as PluginMenuItem, PluginHost};
use crate::scripting::ScriptComponent;
use crate::ui_api::UiEvent;

// Phosphor icons for hierarchy
use egui_phosphor::regular::{
    CUBE, SPHERE, CYLINDER, SQUARE, LIGHTBULB, SUN, FLASHLIGHT,
    VIDEO_CAMERA, GLOBE, SPEAKER_HIGH, TREE_STRUCTURE, DOTS_THREE_OUTLINE,
    PLUS, TRASH, COPY, ARROW_SQUARE_OUT, PACKAGE, CODE,
};

// Tree line constants
const INDENT_SIZE: f32 = 18.0;
const TREE_LINE_COLOR: Color32 = Color32::from_rgb(70, 70, 80);
const DROP_LINE_COLOR: Color32 = Color32::from_rgb(100, 160, 255);

fn drop_child_color() -> Color32 {
    Color32::from_rgba_unmultiplied(100, 160, 255, 40)
}

pub fn render_hierarchy(
    ctx: &egui::Context,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node_registry: &NodeRegistry,
    active_tab: usize,
    _left_panel_width: f32,
    _content_start_y: f32,
    _content_height: f32,
    plugin_host: &PluginHost,
) -> Vec<UiEvent> {
    let mut ui_events = Vec::new();

    egui::SidePanel::left("hierarchy")
        .default_width(260.0)
        .resizable(true)
        .show(ctx, |ui| {
            let events = render_hierarchy_content(ui, selection, hierarchy, entities, commands, meshes, materials, node_registry, active_tab, plugin_host);
            ui_events.extend(events);
        });

    // Show drag tooltip
    if let Some(drag_entity) = hierarchy.drag_entity {
        if let Ok((_, editor_entity, _, _, _)) = entities.get(drag_entity) {
            if let Some(pos) = ctx.pointer_hover_pos() {
                egui::Area::new(egui::Id::new("hierarchy_drag_tooltip"))
                    .fixed_pos(pos + Vec2::new(10.0, 10.0))
                    .interactable(false)
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            let (icon, color) = get_node_icon(&editor_entity.name);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).color(color));
                                ui.label(&editor_entity.name);
                            });
                        });
                    });
            }
        }
    }

    ui_events
}

/// Render hierarchy content (for use in docking)
pub fn render_hierarchy_content(
    ui: &mut egui::Ui,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node_registry: &NodeRegistry,
    active_tab: usize,
    plugin_host: &PluginHost,
) -> Vec<UiEvent> {
    let mut ui_events = Vec::new();
    let ctx = ui.ctx().clone();

    ui.horizontal(|ui| {
        ui.label(RichText::new(TREE_STRUCTURE).size(18.0).color(Color32::from_rgb(140, 191, 242)));
        ui.heading("Hierarchy");
    });

    ui.add_space(8.0);

    // Add button with popup
    let add_response = ui.add_sized(
        Vec2::new(ui.available_width() - 8.0, 26.0),
        egui::Button::new(format!("{} Add Node", PLUS)).fill(Color32::from_rgb(51, 115, 191)),
    );

    egui::Popup::from_toggle_button_response(&add_response)
        .show(|ui| {
            ui.set_min_width(180.0);
            render_node_menu_items(ui, node_registry, commands, meshes, materials, None, selection, hierarchy);
        });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Scene tree
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Collect root entities for current tab (only show entities with matching SceneTabId)
        let root_entities: Vec<_> = entities
            .iter()
            .filter(|(_, _, parent, _, tab_id)| {
                parent.is_none() && tab_id.map_or(false, |t| t.0 == active_tab)
            })
            .collect();

        if root_entities.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Empty scene").weak());
            ui.label(RichText::new("Click '+ Add Node' to begin").weak());
        } else {
            // Clear drop target at start of frame
            hierarchy.drop_target = None;

            let root_count = root_entities.len();
            for (i, (entity, editor_entity, _, children, _)) in root_entities.into_iter().enumerate() {
                let is_last = i == root_count - 1;
                let events = render_tree_node(
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
                    0,
                    is_last,
                    &mut Vec::new(), // No parent lines for root nodes
                    None, // No parent entity for root nodes
                    plugin_host,
                );
                ui_events.extend(events);
            }

            // Handle drop when mouse released
            if ctx.input(|i| i.pointer.any_released()) {
                if let (Some(drag_entity), Some(drop_target)) = (
                    hierarchy.drag_entity.take(),
                    hierarchy.drop_target.take(),
                ) {
                    // Don't drop onto self
                    if drag_entity != drop_target.entity {
                        apply_hierarchy_drop(commands, drag_entity, drop_target, entities);
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
        .filter(|(loc, _)| *loc == ContextMenuLocation::Hierarchy)
        .map(|(_, item)| item)
        .collect();

    // These will be rendered in the tree node context menu, so we just collect them here
    // for now and pass them through. The actual rendering happens in render_tree_node.
    // For simplicity, we store the items in a local to be used by the tree node rendering.
    let _ = hierarchy_context_items; // Used in tree node context menus

    ui_events
}

fn render_tree_node(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>)>,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    node_registry: &NodeRegistry,
    entity: Entity,
    editor_entity: &EditorEntity,
    children: Option<&Children>,
    depth: usize,
    is_last: bool,
    parent_lines: &mut Vec<bool>, // true = draw vertical line at this depth
    _parent_entity: Option<Entity>,
    plugin_host: &PluginHost,
) -> Vec<UiEvent> {
    let mut ui_events = Vec::new();
    let is_selected = selection.selected_entity == Some(entity);
    let has_children = children.map_or(false, |c| !c.is_empty());
    let is_expanded = hierarchy.expanded_entities.contains(&entity);
    let is_being_dragged = hierarchy.drag_entity == Some(entity);

    let row_height = 22.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), Sense::click_and_drag());
    let painter = ui.painter();

    let base_x = rect.min.x + 4.0;
    let center_y = rect.center().y;

    // Handle drag start
    if response.drag_started() {
        hierarchy.drag_entity = Some(entity);
    }

    // Show drag cursor when dragging
    if hierarchy.drag_entity.is_some() && response.hovered() {
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }

    // Determine drop target based on mouse position
    let mut current_drop_target: Option<(HierarchyDropPosition, bool)> = None; // (position, show_indicator)

    if let Some(drag_entity) = hierarchy.drag_entity {
        if drag_entity != entity && response.hovered() {
            if let Some(pointer_pos) = ctx.pointer_hover_pos() {
                let relative_y = pointer_pos.y - rect.min.y;
                let drop_zone_size = row_height / 4.0;

                if relative_y < drop_zone_size {
                    // Top zone - insert before
                    current_drop_target = Some((HierarchyDropPosition::Before, true));
                    hierarchy.drop_target = Some(HierarchyDropTarget {
                        entity,
                        position: HierarchyDropPosition::Before,
                    });
                } else if relative_y > row_height - drop_zone_size {
                    // Bottom zone - insert after (or as first child if has children and expanded)
                    if has_children && is_expanded {
                        current_drop_target = Some((HierarchyDropPosition::AsChild, true));
                        hierarchy.drop_target = Some(HierarchyDropTarget {
                            entity,
                            position: HierarchyDropPosition::AsChild,
                        });
                    } else {
                        current_drop_target = Some((HierarchyDropPosition::After, true));
                        hierarchy.drop_target = Some(HierarchyDropTarget {
                            entity,
                            position: HierarchyDropPosition::After,
                        });
                    }
                } else {
                    // Middle zone - insert as child
                    current_drop_target = Some((HierarchyDropPosition::AsChild, true));
                    hierarchy.drop_target = Some(HierarchyDropTarget {
                        entity,
                        position: HierarchyDropPosition::AsChild,
                    });
                }
            }
        }
    }

    // Draw drop indicators
    if let Some((drop_pos, _)) = current_drop_target {
        let content_x = base_x + (depth as f32 * INDENT_SIZE);
        match drop_pos {
            HierarchyDropPosition::Before => {
                // Line at top
                painter.line_segment(
                    [Pos2::new(content_x, rect.min.y), Pos2::new(rect.max.x, rect.min.y)],
                    Stroke::new(2.0, DROP_LINE_COLOR),
                );
            }
            HierarchyDropPosition::After => {
                // Line at bottom
                painter.line_segment(
                    [Pos2::new(content_x, rect.max.y), Pos2::new(rect.max.x, rect.max.y)],
                    Stroke::new(2.0, DROP_LINE_COLOR),
                );
            }
            HierarchyDropPosition::AsChild => {
                // Highlight entire row
                painter.rect_filled(rect, 2.0, drop_child_color());
            }
        }
    }

    // Dim the row if it's being dragged
    if is_being_dragged {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 100));
    }

    // Draw tree guide lines
    let line_stroke = Stroke::new(1.0, TREE_LINE_COLOR);

    // Draw vertical continuation lines for parent levels
    for (level, &has_more_siblings) in parent_lines.iter().enumerate() {
        if has_more_siblings {
            let x = base_x + (level as f32 * INDENT_SIZE) + INDENT_SIZE / 2.0;
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                line_stroke,
            );
        }
    }

    // Draw connector for this node (if not root)
    if depth > 0 {
        let x = base_x + ((depth - 1) as f32 * INDENT_SIZE) + INDENT_SIZE / 2.0;

        // Vertical line from top to center (or full height if not last)
        if is_last {
            // └ shape - vertical line from top to center
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, center_y)],
                line_stroke,
            );
        } else {
            // ├ shape - vertical line full height
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                line_stroke,
            );
        }

        // Horizontal line from vertical to content
        let h_end_x = base_x + (depth as f32 * INDENT_SIZE);
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
            let arrow = if is_expanded { "▼" } else { "▶" };
            if ui.add(egui::Button::new(RichText::new(arrow).size(10.0)).frame(false)).clicked() {
                if is_expanded {
                    hierarchy.expanded_entities.remove(&entity);
                } else {
                    hierarchy.expanded_entities.insert(entity);
                }
            }
        } else {
            ui.add_space(16.0);
        }

        // Icon based on node name/type
        let (icon, icon_color) = get_node_icon(&editor_entity.name);
        ui.label(RichText::new(icon).color(icon_color).size(14.0));

        // Name - selectable
        let text_color = if is_selected {
            Color32::WHITE
        } else {
            Color32::from_rgb(217, 217, 224)
        };

        let name_response = ui.selectable_label(is_selected, RichText::new(&editor_entity.name).color(text_color));

        if name_response.clicked() && hierarchy.drag_entity.is_none() {
            selection.selected_entity = Some(entity);
        }

        // Right-click context menu
        name_response.context_menu(|ui| {
            ui.set_min_width(160.0);

            // Add Child submenu
            ui.menu_button(format!("{} Add Child", PLUS), |ui| {
                render_node_menu_items(ui, node_registry, commands, meshes, materials, Some(entity), selection, hierarchy);
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
                // Despawn entity and its children
                commands.entity(entity).despawn();
                // Clear selection if this was selected
                if selection.selected_entity == Some(entity) {
                    selection.selected_entity = None;
                }
                // Remove from expanded set
                hierarchy.expanded_entities.remove(&entity);
                ui.close();
            }

            // Plugin context menu items
            let hierarchy_items: Vec<_> = plugin_host.api().context_menus.iter()
                .filter(|(loc, _)| *loc == ContextMenuLocation::Hierarchy)
                .map(|(_, item)| item)
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
    });

    // Render children if expanded
    if has_children && is_expanded {
        if let Some(children) = children {
            let child_entities: Vec<_> = children.iter().collect();
            let child_count = child_entities.len();

            for (i, child_entity) in child_entities.into_iter().enumerate() {
                if let Ok((child, child_editor, _, grandchildren, _)) = entities.get(child_entity) {
                    let child_is_last = i == child_count - 1;

                    // Update parent_lines for children
                    parent_lines.push(!is_last); // Continue vertical line if current node is not last

                    let child_events = render_tree_node(
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
                        depth + 1,
                        child_is_last,
                        parent_lines,
                        Some(entity),
                        plugin_host,
                    );
                    ui_events.extend(child_events);

                    parent_lines.pop();
                }
            }
        }
    }

    ui_events
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
fn apply_hierarchy_drop(
    commands: &mut Commands,
    drag_entity: Entity,
    drop_target: HierarchyDropTarget,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>)>,
) {
    // Get the target's parent
    let target_parent = entities
        .get(drop_target.entity)
        .ok()
        .and_then(|(_, _, parent, _, _)| parent.map(|p| p.0));

    match drop_target.position {
        HierarchyDropPosition::Before | HierarchyDropPosition::After => {
            // Make sibling of target (same parent)
            if let Some(parent) = target_parent {
                commands.entity(drag_entity).insert(ChildOf(parent));
            } else {
                // Target is at root level, make dragged entity also root
                commands.entity(drag_entity).remove::<ChildOf>();
            }
        }
        HierarchyDropPosition::AsChild => {
            // Make child of target
            commands.entity(drag_entity).insert(ChildOf(drop_target.entity));
        }
    }
}

/// Get an icon and color for a node based on its name
fn get_node_icon(name: &str) -> (&'static str, Color32) {
    let name_lower = name.to_lowercase();

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

    // Default - generic node
    (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190))
}
