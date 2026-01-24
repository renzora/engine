use bevy::prelude::*;
use bevy_egui::egui::{self, RichText, Color32};

use crate::core::{SelectionState, HierarchyState};
use crate::console_success;
use super::nodes::is_scene_root_type;
use super::registry::NodeRegistry;
use super::definition::NodeCategory;

// Phosphor icons for node menu
use egui_phosphor::regular::{
    CUBE, SPHERE, CYLINDER, SQUARE, LIGHTBULB, SUN, FLASHLIGHT,
    VIDEO_CAMERA, GLOBE, SPEAKER_HIGH, DOTS_THREE_OUTLINE, PACKAGE, ATOM,
    TREE_STRUCTURE,
};

/// Render the add node popup menu using the node registry
/// This generates the menu dynamically from registered node definitions
#[allow(deprecated)]
pub fn render_add_node_popup(
    ui: &mut egui::Ui,
    popup_id: egui::Id,
    registry: &NodeRegistry,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    has_scene_root: bool,
) {
    // Use popup_below_widget for dropdown menu
    egui::popup_below_widget(
        ui,
        popup_id,
        &ui.response(),
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui: &mut egui::Ui| {
            ui.set_min_width(180.0);
            render_node_menu_items(ui, registry, commands, meshes, materials, parent, selection, hierarchy, has_scene_root);
        },
    );
}

/// Render node menu items (used in both popup and submenu contexts)
/// When `has_scene_root` is true, scene root nodes are filtered out
pub fn render_node_menu_items(
    ui: &mut egui::Ui,
    registry: &NodeRegistry,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    has_scene_root: bool,
) {
    let categories = registry.categories_with_nodes();

    for (i, category) in categories.iter().enumerate() {
        // Get filtered nodes for this category
        let nodes: Vec<_> = registry.get_by_category(*category)
            .iter()
            .filter(|def| !has_scene_root || !is_scene_root_type(def.type_id))
            .copied()
            .collect();

        // Skip empty categories
        if nodes.is_empty() {
            continue;
        }

        // Add spacing between categories (except before the first)
        if i > 0 {
            ui.add_space(4.0);
        }

        // Category header with icon
        let (cat_icon, cat_color) = get_category_icon(*category);
        ui.horizontal(|ui| {
            ui.label(RichText::new(cat_icon).color(cat_color));
            ui.label(RichText::new(category.display_name()).weak());
        });
        ui.separator();

        // Node items in this category
        for definition in nodes {
            let (icon, _color) = get_node_icon_for_type(definition.type_id);
            let label = format!("{} {}", icon, definition.display_name);
            if ui.selectable_label(false, RichText::new(label).color(Color32::from_rgb(220, 220, 230))).clicked() {
                // Spawn the node using the registered spawn function
                let entity = (definition.spawn_fn)(commands, meshes, materials, parent);
                // Auto-select the newly created node
                selection.selected_entity = Some(entity);
                // Auto-expand parent if adding as child
                if let Some(parent_entity) = parent {
                    hierarchy.expanded_entities.insert(parent_entity);
                }
                console_success!("Node", "Created {}", definition.display_name);
                ui.close();
            }
        }
    }
}

/// Get icon and color for a node category
fn get_category_icon(category: NodeCategory) -> (&'static str, Color32) {
    match category {
        NodeCategory::Nodes3D => (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190)),
        NodeCategory::Meshes => (CUBE, Color32::from_rgb(242, 166, 115)),
        NodeCategory::Lights => (LIGHTBULB, Color32::from_rgb(255, 230, 140)),
        NodeCategory::Physics => (ATOM, Color32::from_rgb(166, 242, 200)),
        NodeCategory::Environment => (GLOBE, Color32::from_rgb(140, 217, 191)),
        NodeCategory::Cameras => (VIDEO_CAMERA, Color32::from_rgb(140, 191, 242)),
        NodeCategory::Custom => (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190)),
    }
}

/// Get icon and color for a specific node type
fn get_node_icon_for_type(type_id: &str) -> (&'static str, Color32) {
    match type_id {
        // Scene Roots
        "scene.3d" => (TREE_STRUCTURE, Color32::from_rgb(140, 191, 242)),
        "scene.2d" => (TREE_STRUCTURE, Color32::from_rgb(191, 140, 242)),
        "scene.ui" => (TREE_STRUCTURE, Color32::from_rgb(242, 191, 140)),
        "scene.other" => (TREE_STRUCTURE, Color32::from_rgb(180, 180, 190)),

        // 3D Nodes
        "node.empty" => (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190)),

        // Meshes
        "mesh.cube" => (CUBE, Color32::from_rgb(242, 166, 115)),
        "mesh.sphere" => (SPHERE, Color32::from_rgb(242, 166, 115)),
        "mesh.cylinder" => (CYLINDER, Color32::from_rgb(242, 166, 115)),
        "mesh.plane" => (SQUARE, Color32::from_rgb(242, 166, 115)),
        "mesh.instance" => (PACKAGE, Color32::from_rgb(166, 217, 242)),

        // Lights
        "light.point" => (LIGHTBULB, Color32::from_rgb(255, 230, 140)),
        "light.directional" => (SUN, Color32::from_rgb(255, 230, 140)),
        "light.spot" => (FLASHLIGHT, Color32::from_rgb(255, 230, 140)),

        // Environment
        "env.world" => (GLOBE, Color32::from_rgb(140, 217, 191)),
        "env.audio_listener" => (SPEAKER_HIGH, Color32::from_rgb(217, 140, 217)),

        // Cameras
        "camera.3d" => (VIDEO_CAMERA, Color32::from_rgb(140, 191, 242)),

        // Physics Bodies
        "physics.rigidbody3d" => (ATOM, Color32::from_rgb(166, 242, 200)),
        "physics.staticbody3d" => (ATOM, Color32::from_rgb(166, 242, 200)),
        "physics.kinematicbody3d" => (ATOM, Color32::from_rgb(166, 242, 200)),

        // Collision Shapes
        "physics.collision_box" => (CUBE, Color32::from_rgb(166, 242, 200)),
        "physics.collision_sphere" => (SPHERE, Color32::from_rgb(166, 242, 200)),
        "physics.collision_capsule" => (CYLINDER, Color32::from_rgb(166, 242, 200)),
        "physics.collision_cylinder" => (CYLINDER, Color32::from_rgb(166, 242, 200)),

        // Default
        _ => (DOTS_THREE_OUTLINE, Color32::from_rgb(180, 180, 190)),
    }
}

/// Render the add child submenu using the node registry
#[allow(dead_code)]
pub fn render_add_child_menu(
    ui: &mut egui::Ui,
    registry: &NodeRegistry,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
) {
    ui.menu_button("Add Child", |ui| {
        // Always filter scene roots when adding children (we must have a scene root already)
        render_node_menu_items(ui, registry, commands, meshes, materials, Some(parent), selection, hierarchy, true);
    });
}

/// Render node menu with categories as submenus (for context menus)
/// Scene root nodes are always filtered out since they can only be created from empty scene
pub fn render_node_menu_as_submenus(
    ui: &mut egui::Ui,
    registry: &NodeRegistry,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
) {
    let categories = registry.categories_with_nodes();
    let mut first_category = true;

    for category in categories.iter() {
        // Filter out scene root nodes - they can only be created from empty scene prompt
        let nodes: Vec<_> = registry.get_by_category(*category)
            .iter()
            .filter(|def| !is_scene_root_type(def.type_id))
            .copied()
            .collect();

        // Skip empty categories
        if nodes.is_empty() {
            continue;
        }

        // Add spacing between categories (except before the first)
        if !first_category {
            ui.add_space(2.0);
        }
        first_category = false;

        let (cat_icon, cat_color) = get_category_icon(*category);
        let label = format!("{} {}", cat_icon, category.display_name());

        ui.menu_button(RichText::new(label).color(cat_color), |ui| {
            ui.set_min_width(160.0);

            // Node items in this category
            for definition in nodes {
                let (icon, _) = get_node_icon_for_type(definition.type_id);
                let item_label = format!("{} {}", icon, definition.display_name);

                if ui.selectable_label(false, RichText::new(item_label).color(Color32::from_rgb(220, 220, 230))).clicked() {
                    // Spawn the node using the registered spawn function
                    let entity = (definition.spawn_fn)(commands, meshes, materials, parent);
                    // Auto-select the newly created node
                    selection.selected_entity = Some(entity);
                    // Auto-expand parent if adding as child
                    if let Some(parent_entity) = parent {
                        hierarchy.expanded_entities.insert(parent_entity);
                    }
                    console_success!("Node", "Created {}", definition.display_name);
                    ui.close();
                }
            }
        });
    }
}

/// Get icon and color - exported for use in hierarchy
pub fn get_category_icon_public(category: NodeCategory) -> (&'static str, Color32) {
    get_category_icon(category)
}

/// Get node icon by type - exported for use in hierarchy
pub fn get_node_icon_for_type_public(type_id: &str) -> (&'static str, Color32) {
    get_node_icon_for_type(type_id)
}
