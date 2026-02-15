//! Node Explorer panel - displays the full hierarchy of a selected entity
//!
//! Shows all child nodes, their components, and properties in a tree view.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, ScrollArea, Vec2};
use egui_phosphor::regular::{CUBE, BONE, PLACEHOLDER, CARET_RIGHT, CARET_DOWN};

use renzora_theme::Theme;

/// State for the node explorer panel
#[derive(Default)]
pub struct NodeExplorerState {
    /// Which nodes are expanded in the tree
    pub expanded_nodes: std::collections::HashSet<Entity>,
    /// Currently selected node in the explorer (for highlighting)
    pub selected_node: Option<Entity>,
}

/// Info about a node for display
pub struct NodeInfo {
    pub entity: Entity,
    pub name: String,
    pub has_mesh: bool,
    pub has_skinned_mesh: bool,
    pub transform: Option<Transform>,
    pub global_pos: Option<Vec3>,
    pub joint_count: Option<usize>,
    pub children: Vec<Entity>,
}

/// Render the node explorer panel content
pub fn render_node_explorer_content(
    ui: &mut egui::Ui,
    selected_entity: Option<Entity>,
    state: &mut NodeExplorerState,
    node_infos: &std::collections::HashMap<Entity, NodeInfo>,
    theme: &Theme,
) {
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let surface_raised = theme.surfaces.faint.to_color32();

    // Check if we have a selected entity
    let Some(selected) = selected_entity else {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("Select an entity to explore its nodes").color(text_muted));
        });
        return;
    };

    // Get the root node info
    let Some(root_info) = node_infos.get(&selected) else {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("Loading node data...").color(text_muted));
        });
        return;
    };

    // Header with root entity info
    ui.horizontal(|ui| {
        ui.label(RichText::new("Root:").color(text_secondary));
        ui.label(RichText::new(&root_info.name).color(text_primary).strong());
        ui.label(RichText::new(format!("({:?})", selected)).color(text_muted).small());
    });

    // Count total nodes
    let total_nodes = node_infos.len();
    let mesh_count = node_infos.values().filter(|n| n.has_mesh).count();
    let skinned_count = node_infos.values().filter(|n| n.has_skinned_mesh).count();

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} nodes", total_nodes)).color(text_muted).small());
        if mesh_count > 0 {
            ui.label(RichText::new(format!("• {} meshes", mesh_count)).color(Color32::from_rgb(100, 180, 255)).small());
        }
        if skinned_count > 0 {
            ui.label(RichText::new(format!("• {} skinned", skinned_count)).color(Color32::from_rgb(255, 180, 100)).small());
        }
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Expand all / Collapse all buttons
    ui.horizontal(|ui| {
        if ui.small_button("Expand All").clicked() {
            for entity in node_infos.keys() {
                state.expanded_nodes.insert(*entity);
            }
        }
        if ui.small_button("Collapse All").clicked() {
            state.expanded_nodes.clear();
        }
    });

    ui.add_space(4.0);

    // Scrollable tree view
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_node_tree(
                ui,
                selected,
                0,
                state,
                node_infos,
                text_primary,
                text_secondary,
                text_muted,
                accent,
                surface_raised,
            );
        });
}

/// Recursively render a node and its children
fn render_node_tree(
    ui: &mut egui::Ui,
    entity: Entity,
    depth: usize,
    state: &mut NodeExplorerState,
    node_infos: &std::collections::HashMap<Entity, NodeInfo>,
    text_primary: Color32,
    text_secondary: Color32,
    text_muted: Color32,
    accent: Color32,
    surface_raised: Color32,
) {
    let Some(node_info) = node_infos.get(&entity) else {
        return;
    };

    let indent = depth as f32 * 16.0;
    let has_children = !node_info.children.is_empty();
    let is_expanded = state.expanded_nodes.contains(&entity);
    let is_selected = state.selected_node == Some(entity);

    // Determine node icon
    let icon = if node_info.has_skinned_mesh {
        BONE
    } else if node_info.has_mesh {
        CUBE
    } else {
        PLACEHOLDER
    };

    // Node row
    ui.horizontal(|ui| {
        ui.add_space(indent);

        // Expand/collapse button
        if has_children {
            let caret = if is_expanded { CARET_DOWN } else { CARET_RIGHT };
            if ui.add(egui::Button::new(RichText::new(caret).size(12.0))
                .frame(false)
                .min_size(Vec2::new(16.0, 16.0))).clicked() {
                if is_expanded {
                    state.expanded_nodes.remove(&entity);
                } else {
                    state.expanded_nodes.insert(entity);
                }
            }
        } else {
            ui.add_space(16.0);
        }

        // Selection highlight background
        let row_rect = ui.available_rect_before_wrap();
        if is_selected {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(row_rect.min, Vec2::new(row_rect.width(), 20.0)),
                2.0,
                surface_raised,
            );
        }

        // Icon
        let icon_color = if node_info.has_skinned_mesh {
            Color32::from_rgb(255, 180, 100) // Orange for bones
        } else if node_info.has_mesh {
            Color32::from_rgb(100, 180, 255) // Blue for meshes
        } else {
            text_muted
        };
        ui.label(RichText::new(icon).color(icon_color).size(14.0));

        // Name (clickable)
        let name_response = ui.add(
            egui::Label::new(RichText::new(&node_info.name).color(if is_selected { accent } else { text_primary }))
                .sense(egui::Sense::click())
        );
        if name_response.clicked() {
            state.selected_node = Some(entity);
        }

        // Entity ID
        ui.label(RichText::new(format!("{:?}", entity)).color(text_muted).small());

        // Component badges
        if node_info.has_mesh && !node_info.has_skinned_mesh {
            ui.label(RichText::new("[Mesh]").color(Color32::from_rgb(100, 180, 255)).small());
        }
        if node_info.has_skinned_mesh {
            ui.label(RichText::new("[Skinned]").color(Color32::from_rgb(255, 180, 100)).small());
        }
        if has_children {
            ui.label(RichText::new(format!("[{} children]", node_info.children.len())).color(text_muted).small());
        }
    });

    // Show node details if selected
    if is_selected {
        ui.horizontal(|ui| {
            ui.add_space(indent + 32.0);
            ui.vertical(|ui| {
                // Transform info
                if let Some(transform) = &node_info.transform {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Position:").color(text_secondary).small());
                        ui.label(RichText::new(format!(
                            "({:.3}, {:.3}, {:.3})",
                            transform.translation.x,
                            transform.translation.y,
                            transform.translation.z
                        )).color(text_muted).small());
                    });

                    let (axis, angle) = transform.rotation.to_axis_angle();
                    if angle.abs() > 0.001 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Rotation:").color(text_secondary).small());
                            ui.label(RichText::new(format!(
                                "{:.1}° around ({:.2}, {:.2}, {:.2})",
                                angle.to_degrees(),
                                axis.x, axis.y, axis.z
                            )).color(text_muted).small());
                        });
                    }

                    if transform.scale != Vec3::ONE {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Scale:").color(text_secondary).small());
                            ui.label(RichText::new(format!(
                                "({:.3}, {:.3}, {:.3})",
                                transform.scale.x,
                                transform.scale.y,
                                transform.scale.z
                            )).color(text_muted).small());
                        });
                    }
                }

                // Global position
                if let Some(global_pos) = node_info.global_pos {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("World:").color(text_secondary).small());
                        ui.label(RichText::new(format!(
                            "({:.3}, {:.3}, {:.3})",
                            global_pos.x, global_pos.y, global_pos.z
                        )).color(text_muted).small());
                    });
                }

                // Joint count for skinned meshes
                if let Some(joint_count) = node_info.joint_count {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Joints:").color(text_secondary).small());
                        ui.label(RichText::new(format!("{}", joint_count)).color(text_muted).small());
                    });
                }
            });
        });
        ui.add_space(4.0);
    }

    // Render children if expanded
    if is_expanded {
        for &child in &node_info.children {
            render_node_tree(
                ui,
                child,
                depth + 1,
                state,
                node_infos,
                text_primary,
                text_secondary,
                text_muted,
                accent,
                surface_raised,
            );
        }
    }
}

/// Collect node info for all descendants of an entity
pub fn collect_node_infos(
    root: Entity,
    names: &Query<&Name>,
    global_transforms: &Query<&GlobalTransform>,
    meshes: &Query<&Mesh3d>,
    skinned_meshes: &Query<&bevy::mesh::skinning::SkinnedMesh>,
    children_query: &Query<&Children>,
) -> std::collections::HashMap<Entity, NodeInfo> {
    let mut infos = std::collections::HashMap::new();
    collect_node_infos_recursive(
        root,
        &mut infos,
        names,
        global_transforms,
        meshes,
        skinned_meshes,
        children_query,
    );
    infos
}

fn collect_node_infos_recursive(
    entity: Entity,
    infos: &mut std::collections::HashMap<Entity, NodeInfo>,
    names: &Query<&Name>,
    global_transforms: &Query<&GlobalTransform>,
    meshes: &Query<&Mesh3d>,
    skinned_meshes: &Query<&bevy::mesh::skinning::SkinnedMesh>,
    children_query: &Query<&Children>,
) {
    let name = names
        .get(entity)
        .map(|n| n.to_string())
        .unwrap_or_else(|_| format!("Entity {:?}", entity));

    // Get global transform and compute local transform from it
    let global_transform = global_transforms.get(entity).ok();
    let transform = global_transform.map(|g| g.compute_transform());
    let global_pos = global_transform.map(|g| g.translation());

    let has_mesh = meshes.get(entity).is_ok();
    let skinned = skinned_meshes.get(entity).ok();
    let has_skinned_mesh = skinned.is_some();
    let joint_count = skinned.map(|s| s.joints.len());

    let children: Vec<Entity> = children_query
        .get(entity)
        .map(|c| c.iter().collect())
        .unwrap_or_default();

    infos.insert(entity, NodeInfo {
        entity,
        name,
        has_mesh,
        has_skinned_mesh,
        transform,
        global_pos,
        joint_count,
        children: children.clone(),
    });

    // Recurse into children
    for child in children {
        collect_node_infos_recursive(
            child,
            infos,
            names,
            global_transforms,
            meshes,
            skinned_meshes,
            children_query,
        );
    }
}
