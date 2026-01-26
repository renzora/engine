#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2, Pos2, Stroke, Sense, CursorIcon};

use crate::commands::{CommandHistory, DeleteEntityCommand, queue_command};
use crate::component_system::{ComponentRegistry, PresetCategory, get_presets_by_category, spawn_preset};
use crate::core::{EditorEntity, SelectionState, HierarchyState, HierarchyDropPosition, HierarchyDropTarget, SceneTabId, AssetBrowserState, DefaultCameraEntity, WorldEnvironmentMarker};
use crate::plugin_core::{ContextMenuLocation, MenuItem as PluginMenuItem, PluginHost, TabLocation};
use crate::scripting::ScriptComponent;
use crate::shared::{
    CameraNodeData, CameraRigData, MeshNodeData, MeshInstanceData, SceneInstanceData,
    Sprite2DData, Camera2DData,
    UIPanelData, UILabelData, UIButtonData, UIImageData,
};
use crate::ui_api::{UiEvent, renderer::UiRenderer};

// Phosphor icons for hierarchy
use egui_phosphor::regular::{
    CUBE, SPHERE, CYLINDER, SQUARE, LIGHTBULB, SUN, FLASHLIGHT,
    VIDEO_CAMERA, TREE_STRUCTURE, DOTS_THREE_OUTLINE,
    PLUS, TRASH, COPY, ARROW_SQUARE_OUT, CODE,
    CARET_DOWN, CARET_RIGHT, CUBE_TRANSPARENT, FRAME_CORNERS, BROWSERS, FOLDER_SIMPLE,
    EYE, EYE_SLASH, LOCK_SIMPLE, LOCK_SIMPLE_OPEN, STAR,
    IMAGE, STACK, TEXTBOX, CURSOR_CLICK,
    GLOBE, PACKAGE, MAGNIFYING_GLASS,
};

/// Queries for component-based icon inference in hierarchy
#[derive(bevy::ecs::system::SystemParam)]
pub struct HierarchyComponentQueries<'w, 's> {
    pub point_lights: Query<'w, 's, Entity, With<PointLight>>,
    pub directional_lights: Query<'w, 's, Entity, With<DirectionalLight>>,
    pub spot_lights: Query<'w, 's, Entity, With<SpotLight>>,
    pub meshes: Query<'w, 's, Entity, With<Mesh3d>>,
    pub cameras: Query<'w, 's, Entity, With<CameraNodeData>>,
    pub camera_rigs: Query<'w, 's, Entity, With<CameraRigData>>,
    pub mesh_data: Query<'w, 's, &'static MeshNodeData>,
    pub mesh_instances: Query<'w, 's, Entity, With<MeshInstanceData>>,
    pub scene_instances: Query<'w, 's, Entity, With<SceneInstanceData>>,
    pub world_environments: Query<'w, 's, Entity, With<WorldEnvironmentMarker>>,
    pub sprites: Query<'w, 's, Entity, With<Sprite2DData>>,
    pub cameras_2d: Query<'w, 's, Entity, With<Camera2DData>>,
    pub ui_panels: Query<'w, 's, Entity, With<UIPanelData>>,
    pub ui_labels: Query<'w, 's, Entity, With<UILabelData>>,
    pub ui_buttons: Query<'w, 's, Entity, With<UIButtonData>>,
    pub ui_images: Query<'w, 's, Entity, With<UIImageData>>,
}

/// Combined hierarchy queries including entities and component checks
#[derive(bevy::ecs::system::SystemParam)]
pub struct HierarchyQueries<'w, 's> {
    pub entities: Query<'w, 's, (Entity, &'static EditorEntity, Option<&'static ChildOf>, Option<&'static Children>, Option<&'static SceneTabId>)>,
    pub components: HierarchyComponentQueries<'w, 's>,
}

// Tree line constants
const INDENT_SIZE: f32 = 18.0;
const ROW_HEIGHT: f32 = 20.0;
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
    hierarchy_queries: &HierarchyQueries,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    component_registry: &ComponentRegistry,
    active_tab: usize,
    stored_width: f32,
    plugin_host: &PluginHost,
    assets: &mut AssetBrowserState,
    default_camera: &DefaultCameraEntity,
    command_history: &mut CommandHistory,
    ui_renderer: &mut UiRenderer,
) -> (Vec<UiEvent>, f32, bool) {
    let mut ui_events = Vec::new();
    let mut scene_changed = false;

    // Check if a scene file is being dragged (.ron format)
    let dragging_scene = assets.dragging_asset.as_ref()
        .map(|p| p.to_string_lossy().to_lowercase().ends_with(".ron"))
        .unwrap_or(false);

    // Get plugin tabs for left panel
    let api = plugin_host.api();
    let plugin_tabs = api.get_tabs_for_location(TabLocation::Left);
    let active_plugin_tab = api.get_active_tab(TabLocation::Left);

    // Calculate max width based on screen size (max 500px to match load-time clamping)
    let screen_width = ctx.screen_rect().width();
    let min_viewport_width = 200.0;
    let max_width = ((screen_width - min_viewport_width) / 2.0).max(100.0).min(500.0);
    let display_width = stored_width.clamp(100.0, max_width);
    let mut actual_width = display_width;

    egui::SidePanel::left("hierarchy")
        .exact_width(display_width)
        .resizable(false)
        .frame(egui::Frame::new().fill(Color32::from_rgb(30, 32, 36)).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {

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
                let (events, changed) = render_hierarchy_content(ui, ctx, selection, hierarchy, hierarchy_queries, commands, meshes, materials, component_registry, active_tab, plugin_host, assets, dragging_scene, default_camera, command_history);
                ui_events.extend(events);
                scene_changed = changed;
            }
        });

    // Custom resize handle at the right edge of the panel (full height)
    let resize_x = display_width - 3.0;
    let resize_height = ctx.screen_rect().height();

    egui::Area::new(egui::Id::new("hierarchy_resize"))
        .fixed_pos(Pos2::new(resize_x, 0.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let (resize_rect, resize_response) = ui.allocate_exact_size(
                Vec2::new(6.0, resize_height),
                Sense::drag(),
            );

            if resize_response.hovered() || resize_response.dragged() {
                ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
            }

            // Use pointer position for smooth resizing
            if resize_response.dragged() {
                if let Some(pointer_pos) = ctx.pointer_interact_pos() {
                    actual_width = pointer_pos.x.clamp(100.0, max_width);
                }
            }

            // Invisible resize handle - just show cursor change
            let _ = resize_rect; // Still need the rect for interaction
        });

    // Show drag tooltip
    if !hierarchy.drag_entities.is_empty() {
        if let Some(pos) = ctx.pointer_hover_pos() {
            egui::Area::new(egui::Id::new("hierarchy_drag_tooltip"))
                .fixed_pos(pos + Vec2::new(10.0, 10.0))
                .interactable(false)
                .order(egui::Order::Tooltip)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let drag_count = hierarchy.drag_entities.len();
                        if drag_count == 1 {
                            // Single entity drag
                            if let Ok((_, editor_entity, _, _, _)) = hierarchy_queries.entities.get(hierarchy.drag_entities[0]) {
                                let (icon, color) = get_entity_icon(hierarchy.drag_entities[0], &editor_entity.name, &hierarchy_queries.components);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(icon).color(color));
                                    ui.label(&editor_entity.name);
                                });
                            }
                        } else {
                            // Multi entity drag
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(CUBE).color(Color32::from_rgb(140, 191, 242)));
                                ui.label(format!("{} items", drag_count));
                            });
                        }
                    });
                });
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
    hierarchy_queries: &HierarchyQueries,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    component_registry: &ComponentRegistry,
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

    // Scene root is no longer required - entities can be root-level directly
    let scene_root_entity: Option<Entity> = None;

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

    // Panel bar with search and add button
    ui.horizontal(|ui| {
        ui.add_space(4.0);

        // Search bar
        let search_width = (ui.available_width() - 60.0).max(40.0);
        ui.add_sized(
            [search_width, 20.0],
            egui::TextEdit::singleline(&mut hierarchy.search)
                .hint_text(format!("{} Search...", MAGNIFYING_GLASS))
        );

        // Add button with centered dropdown menu
        ui.menu_button(
            RichText::new(format!("{} Add", PLUS)).color(Color32::from_rgb(100, 180, 255)),
            |ui| {
                ui.set_min_width(180.0);
                render_preset_menu(ui, commands, meshes, materials, component_registry, scene_root_entity, selection, hierarchy);
            }
        );

        ui.add_space(4.0);
    });

    ui.add_space(2.0);

    // Content area with padding
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(0, 2))
        .show(ui, |ui| {
        ui.style_mut().spacing.item_spacing.y = 0.0;

    // Scene tree
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Remove vertical spacing between rows
        ui.style_mut().spacing.item_spacing.y = 0.0;

        // Collect root entities for current tab (only show entities with matching SceneTabId)
        let search_lower = hierarchy.search.to_lowercase();
        let has_search = !hierarchy.search.is_empty();

        let mut root_entities: Vec<_> = hierarchy_queries.entities
            .iter()
            .filter(|(_, editor_entity, parent, _, tab_id)| {
                let in_tab = parent.is_none() && tab_id.map_or(false, |t| t.0 == active_tab);
                if !in_tab {
                    return false;
                }
                // If searching, only show if name matches
                if has_search {
                    editor_entity.name.to_lowercase().contains(&search_lower)
                } else {
                    true
                }
            })
            .collect();

        // Sort by entity ID to maintain stable order (older entities first)
        root_entities.sort_by_key(|(entity, _, _, _, _)| *entity);

        if root_entities.is_empty() {
            // Empty scene - show add entity prompt
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Empty Scene").size(14.0).color(Color32::from_rgb(150, 150, 160)));
                ui.add_space(8.0);
                ui.label(RichText::new("Use the + button above to add entities").size(12.0).weak());
            });
        } else {
            // Clear drop target at start of frame
            hierarchy.drop_target = None;

            // Clear the building order for this frame (we'll swap at the end)
            hierarchy.building_entity_order.clear();

            let root_count = root_entities.len();
            let mut row_index: usize = 0;
            for (i, (entity, editor_entity, _, children, _)) in root_entities.into_iter().enumerate() {
                let is_last = i == root_count - 1;
                let (events, changed) = render_tree_node(
                    ui,
                    &ctx,
                    selection,
                    hierarchy,
                    hierarchy_queries,
                    commands,
                    meshes,
                    materials,
                    component_registry,
                    entity,
                    editor_entity,
                    children,
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
                let drag_entities = std::mem::take(&mut hierarchy.drag_entities);
                let drop_target = hierarchy.drop_target.take();

                if !drag_entities.is_empty() {
                    if let Some(drop_target) = drop_target {
                        // Build a set of all dragged entities for quick lookup
                        let drag_set: std::collections::HashSet<Entity> = drag_entities.iter().copied().collect();

                        // Check if drop target is a descendant of any dragged entity (would create cycle)
                        let is_descendant_of_drag = is_descendant_of_any(drop_target.entity, &drag_set, &hierarchy_queries.entities);

                        if !is_descendant_of_drag {
                            // Find "root" entities in the selection - those whose parent is NOT also selected
                            // This preserves parent-child relationships within the selection
                            let root_drags: Vec<_> = drag_entities
                                .into_iter()
                                .filter(|&e| {
                                    // Don't drop onto self
                                    if e == drop_target.entity {
                                        return false;
                                    }
                                    // Check if this entity's parent is also being dragged
                                    if let Ok((_, _, parent, _, _)) = hierarchy_queries.entities.get(e) {
                                        if let Some(parent) = parent {
                                            // If parent is in the drag set, this is not a root
                                            if drag_set.contains(&parent.0) {
                                                return false;
                                            }
                                        }
                                    }
                                    true
                                })
                                .collect();

                            for drag_entity in root_drags {
                                // Apply the drop and get entity to expand (if any)
                                if let Some(expand_entity) = apply_hierarchy_drop(commands, drag_entity, drop_target, &hierarchy_queries.entities) {
                                    // Auto-expand parent when dropping as child
                                    hierarchy.expanded_entities.insert(expand_entity);
                                }
                            }
                        }
                    }
                }
            }

            // Clear drag if released without valid target
            if ctx.input(|i| i.pointer.any_released()) {
                hierarchy.clear_drag();
            }
        }
    });

    }); // End content frame

    // Swap the building order to visible order for next frame's click handling
    std::mem::swap(&mut hierarchy.visible_entity_order, &mut hierarchy.building_entity_order);

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
    hierarchy_queries: &HierarchyQueries,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    component_registry: &ComponentRegistry,
    entity: Entity,
    editor_entity: &EditorEntity,
    children: Option<&Children>,
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
    let is_selected = selection.is_selected(entity);
    // Only count children that are EditorEntity (not internal Bevy children like mesh handles)
    let has_children = children.map_or(false, |c| {
        c.iter().any(|child| hierarchy_queries.entities.get(child).is_ok())
    });
    let is_expanded = hierarchy.expanded_entities.contains(&entity);
    let is_being_dragged = hierarchy.is_being_dragged(entity);

    // Track visible entity order for Shift+click range selection (building for next frame)
    hierarchy.building_entity_order.push(entity);

    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click_and_drag());
    let painter = ui.painter();

    // Draw odd/even row background
    if *row_index % 2 == 1 {
        painter.rect_filled(rect, 0.0, row_odd_bg());
    }
    *row_index += 1;

    // Draw full-row selection highlight
    if is_selected {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(51, 115, 191, 80));
    } else if response.hovered() && hierarchy.drag_entities.is_empty() {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(255, 255, 255, 15));
    }

    let base_x = rect.min.x + 4.0;
    let center_y = rect.center().y;

    // Get modifier states
    let ctrl_held = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let shift_held = ctx.input(|i| i.modifiers.shift);

    // Handle drag start (unless locked)
    if response.drag_started() && !editor_entity.locked {
        // If the entity is already selected and part of multi-selection, drag all selected
        if selection.is_selected(entity) && selection.has_multi_selection() {
            hierarchy.start_drag(selection.get_all_selected());
        } else {
            // Otherwise, select this entity and start dragging just it
            if !ctrl_held && !shift_held {
                selection.select(entity);
            }
            hierarchy.start_drag(vec![entity]);
        }
    }

    // Click anywhere on row to select (unless locked or dragging)
    if response.clicked() && hierarchy.drag_entities.is_empty() && !editor_entity.locked {
        if ctrl_held {
            // Ctrl+click: toggle selection
            selection.toggle_selection(entity);
        } else if shift_held {
            // Shift+click: select range from anchor to this entity
            let visible_order = hierarchy.visible_entity_order.clone();
            selection.select_range(entity, &visible_order);
        } else {
            // Regular click: select single entity
            selection.select(entity);
        }
    }

    // Show drag cursor when dragging
    if !hierarchy.drag_entities.is_empty() && response.hovered() {
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }

    // Determine drop target based on mouse position
    let mut current_drop_target: Option<HierarchyDropPosition> = None;

    if !hierarchy.drag_entities.is_empty() {
        // Check if pointer is over this row (don't rely on response.hovered() during drag)
        if let Some(pointer_pos) = ctx.pointer_hover_pos() {
            let is_pointer_over_row = rect.contains(pointer_pos);
            let is_self_being_dragged = hierarchy.is_being_dragged(entity);

            if !is_self_being_dragged && is_pointer_over_row {
                let relative_y = pointer_pos.y - rect.min.y;
                let edge_zone = ROW_HEIGHT / 3.0; // Top and bottom third for line dividers

                if relative_y < edge_zone {
                    // Top zone - insert before (show line)
                    current_drop_target = Some(HierarchyDropPosition::Before);
                    hierarchy.drop_target = Some(HierarchyDropTarget {
                        entity,
                        position: HierarchyDropPosition::Before,
                    });
                } else if relative_y > ROW_HEIGHT - edge_zone {
                    // Bottom zone - insert after (show line)
                    current_drop_target = Some(HierarchyDropPosition::After);
                    hierarchy.drop_target = Some(HierarchyDropTarget {
                        entity,
                        position: HierarchyDropPosition::After,
                    });
                } else {
                    // Middle zone - insert as child (highlight parent)
                    current_drop_target = Some(HierarchyDropPosition::AsChild);
                    hierarchy.drop_target = Some(HierarchyDropTarget {
                        entity,
                        position: HierarchyDropPosition::AsChild,
                    });
                }
            }
        }
    }

    // Draw drop indicators on foreground layer so they appear on top of content
    // (AsChild border is drawn after children are rendered, so it encompasses the whole group)
    if let Some(drop_pos) = current_drop_target {
        let fg_painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("hierarchy_drop_indicator")));
        match drop_pos {
            HierarchyDropPosition::Before => {
                // Full-width horizontal line at top
                let y = rect.min.y + 1.0;
                fg_painter.line_segment(
                    [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                    Stroke::new(3.0, DROP_LINE_COLOR),
                );
            }
            HierarchyDropPosition::After => {
                // Full-width horizontal line at bottom
                let y = rect.max.y - 1.0;
                fg_painter.line_segment(
                    [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                    Stroke::new(3.0, DROP_LINE_COLOR),
                );
            }
            HierarchyDropPosition::AsChild => {
                // Border will be drawn after children - just store the top position
            }
        }
    }

    // Remember the top of this row if it's an AsChild target (for group border)
    let group_top = if current_drop_target == Some(HierarchyDropPosition::AsChild) {
        Some(rect.min.y)
    } else {
        None
    };

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

    // Create a child ui for the content - vertically centered
    let content_rect = egui::Rect::from_min_max(
        Pos2::new(content_x, rect.min.y),
        rect.max,
    );

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
    child_ui.style_mut().spacing.item_spacing = Vec2::new(2.0, 0.0);

    child_ui.horizontal_centered(|ui| {
        ui.style_mut().spacing.item_spacing = Vec2::new(2.0, 0.0);

        // Expand/collapse button
        if has_children {
            let (icon, icon_color) = if is_expanded {
                (CARET_DOWN, Color32::from_rgb(150, 150, 160))
            } else {
                (CARET_RIGHT, Color32::from_rgb(110, 110, 120))
            };

            let expand_btn = ui.add(
                egui::Button::new(RichText::new(icon).size(10.0).color(icon_color))
                    .frame(false)
                    .min_size(Vec2::new(16.0, ROW_HEIGHT))
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
            ui.add_space(16.0);
        }

        // Visibility icon (left side)
        let vis_icon = if editor_entity.visible { EYE } else { EYE_SLASH };
        let vis_color = if editor_entity.visible {
            Color32::from_rgb(140, 180, 220)
        } else {
            Color32::from_rgb(90, 90, 100)
        };
        let vis_btn = ui.add(
            egui::Button::new(RichText::new(vis_icon).size(10.0).color(vis_color))
                .frame(false)
                .min_size(Vec2::new(14.0, ROW_HEIGHT))
        );
        if vis_btn.clicked() {
            let new_visible = !editor_entity.visible;
            commands.entity(entity).insert(EditorEntity {
                name: editor_entity.name.clone(),
                tag: editor_entity.tag.clone(),
                visible: new_visible,
                locked: editor_entity.locked,
            });
            if new_visible {
                commands.entity(entity).insert(Visibility::Inherited);
            } else {
                commands.entity(entity).insert(Visibility::Hidden);
            }
        }
        vis_btn.on_hover_text(if editor_entity.visible { "Hide" } else { "Show" });

        // Lock icon (left side)
        let lock_icon = if editor_entity.locked { LOCK_SIMPLE } else { LOCK_SIMPLE_OPEN };
        let lock_color = if editor_entity.locked {
            Color32::from_rgb(230, 180, 100)
        } else {
            Color32::from_rgb(90, 90, 100)
        };
        let lock_btn = ui.add(
            egui::Button::new(RichText::new(lock_icon).size(10.0).color(lock_color))
                .frame(false)
                .min_size(Vec2::new(14.0, ROW_HEIGHT))
        );
        if lock_btn.clicked() {
            commands.entity(entity).insert(EditorEntity {
                name: editor_entity.name.clone(),
                tag: editor_entity.tag.clone(),
                visible: editor_entity.visible,
                locked: !editor_entity.locked,
            });
        }
        lock_btn.on_hover_text(if editor_entity.locked { "Unlock" } else { "Lock" });

        // Icon based on components
        let (icon, icon_color) = get_entity_icon(entity, &editor_entity.name, &hierarchy_queries.components);
        ui.label(RichText::new(icon).color(icon_color).size(13.0));

        // Show default camera indicator
        if default_camera.entity == Some(entity) {
            ui.label(RichText::new(STAR).color(Color32::from_rgb(255, 200, 80)).size(10.0));
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
                        tag: editor_entity.tag.clone(),
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
            let font_size = 12.0;
            let galley = ui.fonts(|f| f.layout_no_wrap(
                editor_entity.name.clone(),
                egui::FontId::proportional(font_size),
                Color32::WHITE,
            ));

            let desired_size = Vec2::new(galley.size().x + 4.0, ROW_HEIGHT);
            let (rect, name_response) = ui.allocate_exact_size(desired_size, Sense::click());

            // Determine text color based on state (row background handles selection highlight)
            let text_color = if is_selected {
                Color32::WHITE
            } else if name_response.hovered() {
                Color32::from_rgb(240, 240, 245)
            } else {
                Color32::from_rgb(218, 218, 225)
            };

            // Draw text centered vertically
            let text_pos = Pos2::new(rect.min.x + 2.0, rect.center().y - galley.size().y / 2.0);
            ui.painter().text(
                text_pos,
                egui::Align2::LEFT_TOP,
                &editor_entity.name,
                egui::FontId::proportional(font_size),
                text_color,
            );

            // Single click to select (unless locked)
            if name_response.clicked() && hierarchy.drag_entities.is_empty() && !editor_entity.locked {
                if ctrl_held {
                    // Ctrl+click: toggle selection
                    selection.toggle_selection(entity);
                } else if shift_held {
                    // Shift+click: select range from anchor to this entity
                    let visible_order = hierarchy.visible_entity_order.clone();
                    selection.select_range(entity, &visible_order);
                } else {
                    // Regular click: select single entity
                    selection.select(entity);
                }
            }

            // Double click to rename (unless locked)
            if name_response.double_clicked() && hierarchy.drag_entities.is_empty() && !editor_entity.locked {
                hierarchy.renaming_entity = Some(entity);
                hierarchy.rename_buffer = editor_entity.name.clone();
                hierarchy.rename_focus_set = false;
            }

            // Right-click context menu on name
            name_response.context_menu(|ui| {
                render_hierarchy_context_menu(ui, entity, editor_entity, hierarchy, hierarchy_queries, commands, meshes, materials, component_registry, selection, plugin_host, &mut ui_events, &mut scene_changed, command_history);
            });
        }

    });

    // Right-click context menu on row background (only when not renaming)
    if hierarchy.renaming_entity != Some(entity) {
        response.context_menu(|ui| {
            render_hierarchy_context_menu(ui, entity, editor_entity, hierarchy, hierarchy_queries, commands, meshes, materials, component_registry, selection, plugin_host, &mut ui_events, &mut scene_changed, command_history);
        });
    }

    // Placeholder for the actual context menu - moved to a separate function
    fn render_hierarchy_context_menu(
        ui: &mut egui::Ui,
        entity: Entity,
        editor_entity: &EditorEntity,
        hierarchy: &mut HierarchyState,
        hierarchy_queries: &HierarchyQueries,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        component_registry: &ComponentRegistry,
        selection: &mut SelectionState,
        plugin_host: &PluginHost,
        ui_events: &mut Vec<UiEvent>,
        scene_changed: &mut bool,
        command_history: &mut CommandHistory,
    ) {
        ui.set_min_width(180.0);

        // Rename option
        if ui.button("✏ Rename").clicked() {
            hierarchy.renaming_entity = Some(entity);
            hierarchy.rename_buffer = editor_entity.name.clone();
            hierarchy.rename_focus_set = false;
            ui.close();
        }

        ui.separator();

        // Add Child Entity submenu with categories
        ui.menu_button(RichText::new(format!("{} Add Child Entity", PLUS)), |ui| {
            render_preset_menu(ui, commands, meshes, materials, component_registry, Some(entity), selection, hierarchy);
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
        let is_camera = hierarchy_queries.components.cameras.get(entity).is_ok()
            || hierarchy_queries.components.camera_rigs.get(entity).is_ok();
        if is_camera {
            ui.separator();
            if ui.button(format!("{} Make Default Camera", STAR)).clicked() {
                hierarchy.pending_make_default_camera = Some(entity);
                ui.close();
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
            *scene_changed = true;
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
    }

    // Track the bottom of the group for AsChild border
    let mut group_bottom = rect.max.y;

    // Render children if expanded
    if has_children && is_expanded {
        if let Some(children) = children {
            let mut child_entities: Vec<_> = children.iter().collect();
            // Sort children by entity ID to maintain stable order
            child_entities.sort();
            let child_count = child_entities.len();

            for (i, child_entity) in child_entities.into_iter().enumerate() {
                if let Ok((child, child_editor, _, grandchildren, _)) = hierarchy_queries.entities.get(child_entity) {
                    let child_is_last = i == child_count - 1;

                    // Update parent_lines for children
                    parent_lines.push(!is_last); // Continue vertical line if current node is not last

                    let (child_events, child_changed) = render_tree_node(
                        ui,
                        ctx,
                        selection,
                        hierarchy,
                        hierarchy_queries,
                        commands,
                        meshes,
                        materials,
                        component_registry,
                        child,
                        child_editor,
                        grandchildren,
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

            // Update group bottom to current cursor position (after all children)
            group_bottom = ui.cursor().top();
        }
    }

    // Draw AsChild group border after children are rendered (so it encompasses the whole group)
    if let Some(top) = group_top {
        let fg_painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("hierarchy_drop_indicator")));
        let group_rect = egui::Rect::from_min_max(
            Pos2::new(rect.min.x, top),
            Pos2::new(rect.max.x, group_bottom),
        );
        fg_painter.rect_stroke(group_rect, 2.0, Stroke::new(2.0, DROP_LINE_COLOR), egui::StrokeKind::Inside);
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

/// Check if an entity is a descendant of any entity in the given set
/// Used to prevent creating cycles when dragging (can't drop parent onto child)
fn is_descendant_of_any(
    entity: Entity,
    ancestors: &std::collections::HashSet<Entity>,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>)>,
) -> bool {
    // Walk up the parent chain and check if any parent is in the ancestors set
    let mut current = entity;
    while let Ok((_, _, parent, _, _)) = entities.get(current) {
        if let Some(parent) = parent {
            if ancestors.contains(&parent.0) {
                return true;
            }
            current = parent.0;
        } else {
            break;
        }
    }
    false
}

/// Apply hierarchy drag and drop - reparent or reorder entity
/// Returns the entity to expand (if dropping as child)
fn apply_hierarchy_drop(
    commands: &mut Commands,
    drag_entity: Entity,
    drop_target: HierarchyDropTarget,
    entities: &Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>)>,
) -> Option<Entity> {
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


/// Get an icon and color for an entity based on its components
fn get_entity_icon(entity: Entity, name: &str, queries: &HierarchyComponentQueries) -> (&'static str, Color32) {
    // Check components in priority order

    // Cameras (highest priority)
    if queries.cameras.get(entity).is_ok() {
        return (VIDEO_CAMERA, Color32::from_rgb(140, 191, 242));
    }
    if queries.camera_rigs.get(entity).is_ok() {
        return (VIDEO_CAMERA, Color32::from_rgb(140, 191, 242));
    }
    if queries.cameras_2d.get(entity).is_ok() {
        return (VIDEO_CAMERA, Color32::from_rgb(242, 140, 191));
    }

    // Lights
    if queries.point_lights.get(entity).is_ok() {
        return (LIGHTBULB, Color32::from_rgb(255, 230, 140));
    }
    if queries.directional_lights.get(entity).is_ok() {
        return (SUN, Color32::from_rgb(255, 230, 140));
    }
    if queries.spot_lights.get(entity).is_ok() {
        return (FLASHLIGHT, Color32::from_rgb(255, 230, 140));
    }

    // World Environment
    if queries.world_environments.get(entity).is_ok() {
        return (GLOBE, Color32::from_rgb(140, 220, 200));
    }

    // 3D Meshes - check mesh data for specific type
    if queries.meshes.get(entity).is_ok() {
        if let Ok(mesh_data) = queries.mesh_data.get(entity) {
            use crate::shared::MeshPrimitiveType;
            return match mesh_data.mesh_type {
                MeshPrimitiveType::Cube => (CUBE, Color32::from_rgb(242, 166, 115)),
                MeshPrimitiveType::Sphere => (SPHERE, Color32::from_rgb(242, 166, 115)),
                MeshPrimitiveType::Cylinder => (CYLINDER, Color32::from_rgb(242, 166, 115)),
                MeshPrimitiveType::Plane => (SQUARE, Color32::from_rgb(242, 166, 115)),
            };
        }
        return (CUBE, Color32::from_rgb(242, 166, 115));
    }

    // Mesh and Scene Instances (imported GLB/GLTF)
    if queries.mesh_instances.get(entity).is_ok() {
        return (PACKAGE, Color32::from_rgb(200, 180, 140));
    }
    if queries.scene_instances.get(entity).is_ok() {
        return (PACKAGE, Color32::from_rgb(200, 180, 140));
    }

    // 2D Sprites
    if queries.sprites.get(entity).is_ok() {
        return (IMAGE, Color32::from_rgb(242, 140, 191));
    }

    // UI Elements
    if queries.ui_panels.get(entity).is_ok() {
        return (STACK, Color32::from_rgb(191, 166, 242));
    }
    if queries.ui_labels.get(entity).is_ok() {
        return (TEXTBOX, Color32::from_rgb(191, 166, 242));
    }
    if queries.ui_buttons.get(entity).is_ok() {
        return (CURSOR_CLICK, Color32::from_rgb(191, 166, 242));
    }
    if queries.ui_images.get(entity).is_ok() {
        return (IMAGE, Color32::from_rgb(191, 166, 242));
    }

    // Fallback to name-based detection for scene roots and special cases
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

    // Default - empty entity
    (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190))
}

/// Render the Create menu with presets organized by category
fn render_preset_menu(
    ui: &mut egui::Ui,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    registry: &ComponentRegistry,
    parent: Option<Entity>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
) {
    for category in PresetCategory::all_in_order() {
        // Skip Empty category at top level - we'll show it separately
        if *category == PresetCategory::Empty {
            continue;
        }

        let presets = get_presets_by_category(*category);
        if presets.is_empty() {
            continue;
        }

        let cat_icon = category.icon();
        let cat_color = get_category_color(*category);
        let label = format!("{} {}", cat_icon, category.display_name());

        ui.menu_button(RichText::new(label).color(cat_color), |ui| {
            ui.set_min_width(160.0);

            for preset in presets {
                let item_label = format!("{} {}", preset.icon, preset.display_name);

                if ui.button(RichText::new(item_label).color(Color32::from_rgb(220, 220, 230))).clicked() {
                    let entity = spawn_preset(commands, meshes, materials, registry, preset, parent);
                    selection.selected_entity = Some(entity);
                    if let Some(parent_entity) = parent {
                        hierarchy.expanded_entities.insert(parent_entity);
                    }
                    ui.close();
                }
            }
        });
    }

    ui.separator();

    // Empty Entity at the bottom
    let empty_presets = get_presets_by_category(PresetCategory::Empty);
    for preset in empty_presets {
        let item_label = format!("{} {}", preset.icon, preset.display_name);

        if ui.button(RichText::new(item_label).color(Color32::from_rgb(180, 180, 190))).clicked() {
            let entity = spawn_preset(commands, meshes, materials, registry, preset, parent);
            selection.selected_entity = Some(entity);
            if let Some(parent_entity) = parent {
                hierarchy.expanded_entities.insert(parent_entity);
            }
            ui.close();
        }
    }
}

/// Render the Create menu with presets organized by category (with close signal for custom popups)
fn render_preset_menu_with_close(
    ui: &mut egui::Ui,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    registry: &ComponentRegistry,
    parent: Option<Entity>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    close_menu: &mut bool,
) {
    for category in PresetCategory::all_in_order() {
        if *category == PresetCategory::Empty {
            continue;
        }

        let presets = get_presets_by_category(*category);
        if presets.is_empty() {
            continue;
        }

        let cat_icon = category.icon();
        let cat_color = get_category_color(*category);
        let label = format!("{} {}", cat_icon, category.display_name());

        ui.menu_button(RichText::new(label).color(cat_color), |ui| {
            ui.set_min_width(160.0);

            for preset in presets {
                let item_label = format!("{} {}", preset.icon, preset.display_name);

                if ui.button(RichText::new(item_label).color(Color32::from_rgb(220, 220, 230))).clicked() {
                    let entity = spawn_preset(commands, meshes, materials, registry, preset, parent);
                    selection.selected_entity = Some(entity);
                    if let Some(parent_entity) = parent {
                        hierarchy.expanded_entities.insert(parent_entity);
                    }
                    *close_menu = true;
                    ui.close();
                }
            }
        });
    }

    ui.separator();

    let empty_presets = get_presets_by_category(PresetCategory::Empty);
    for preset in empty_presets {
        let item_label = format!("{} {}", preset.icon, preset.display_name);

        if ui.button(RichText::new(item_label).color(Color32::from_rgb(180, 180, 190))).clicked() {
            let entity = spawn_preset(commands, meshes, materials, registry, preset, parent);
            selection.selected_entity = Some(entity);
            if let Some(parent_entity) = parent {
                hierarchy.expanded_entities.insert(parent_entity);
            }
            *close_menu = true;
            ui.close();
        }
    }
}

/// Get color for a preset category
fn get_category_color(category: PresetCategory) -> Color32 {
    match category {
        PresetCategory::Empty => Color32::from_rgb(180, 180, 190),
        PresetCategory::Objects3D => Color32::from_rgb(242, 166, 115),
        PresetCategory::Lights => Color32::from_rgb(255, 230, 140),
        PresetCategory::Cameras => Color32::from_rgb(140, 191, 242),
        PresetCategory::Physics => Color32::from_rgb(166, 242, 200),
        PresetCategory::Objects2D => Color32::from_rgb(242, 140, 191),
        PresetCategory::UI => Color32::from_rgb(191, 166, 242),
        PresetCategory::Environment => Color32::from_rgb(140, 220, 180),
    }
}
