//! Hierarchy panel — shows the scene entity tree.

mod state;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{
    icon_button, search_overlay, EditorCommands, EditorPanel, EditorSelection, EntityPreset,
    OverlayAction, OverlayEntry, PanelLocation, PanelRegistry, SpawnRegistry,
};
use renzora_theme::ThemeManager;

use state::{build_entity_tree, filter_tree, HierarchyState};

/// Hierarchy panel — displays all named entities as a tree.
pub struct HierarchyPanel {
    state: RwLock<HierarchyState>,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(HierarchyState::default()),
        }
    }
}

impl EditorPanel for HierarchyPanel {
    fn id(&self) -> &str {
        "hierarchy"
    }

    fn title(&self) -> &str {
        "Hierarchy"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::LIST_BULLETS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let mut state = self.state.write().unwrap();

        // Search bar + "Add Entity" button
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut state.search)
                    .desired_width(ui.available_width() - 30.0)
                    .hint_text(format!("{} Search entities...", regular::MAGNIFYING_GLASS)),
            );
            if icon_button(ui, regular::PLUS, "Add Entity", theme.semantic.accent.to_color32()) {
                state.show_add_overlay = true;
                state.add_search.clear();
            }
        });
        ui.add_space(4.0);

        // Add Entity overlay
        if state.show_add_overlay {
            // Build entries from SpawnRegistry
            let entries: Vec<OverlayEntry> = if let Some(registry) = world.get_resource::<SpawnRegistry>() {
                registry
                    .iter()
                    .map(|p| OverlayEntry {
                        id: p.id,
                        label: p.display_name,
                        icon: p.icon,
                        category: p.category,
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let ctx = ui.ctx().clone();
            match search_overlay(&ctx, "add_entity_overlay", "Add Entity", &entries, &mut state.add_search, &theme) {
                OverlayAction::Selected(id) => {
                    state.show_add_overlay = false;
                    // Find the preset's spawn_fn and push a deferred command
                    if let Some(registry) = world.get_resource::<SpawnRegistry>() {
                        if let Some(preset) = registry.iter().find(|p| p.id == id) {
                            let spawn_fn = preset.spawn_fn;
                            if let Some(commands) = world.get_resource::<EditorCommands>() {
                                commands.push(move |world: &mut World| {
                                    let entity = spawn_fn(world);
                                    if let Some(sel) = world.get_resource::<EditorSelection>() {
                                        sel.set(Some(entity));
                                    }
                                });
                            }
                        }
                    }
                }
                OverlayAction::Closed => {
                    state.show_add_overlay = false;
                }
                OverlayAction::None => {}
            }
        }

        // Build entity tree from ECS
        let nodes = build_entity_tree(world);
        let nodes = if state.search.trim().is_empty() {
            nodes
        } else {
            filter_tree(nodes, state.search.trim())
        };

        if nodes.is_empty() {
            let text_muted = theme.text.muted.to_color32();
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No entities in scene.")
                        .size(11.0)
                        .color(text_muted),
                );
            });
            return;
        }

        // Render the tree (reborrow to allow split field access)
        let state = &mut *state;
        egui::ScrollArea::vertical()
            .id_salt("hierarchy_tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                tree::render_tree(
                    ui,
                    &nodes,
                    &mut state.expanded,
                    &mut state.selected,
                    &theme,
                );
            });

        // Sync local selection → global EditorSelection
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            sel.set(state.selected);
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}

/// Plugin that registers the `HierarchyPanel` and built-in entity presets.
pub struct HierarchyPanelPlugin;

impl Plugin for HierarchyPanelPlugin {
    fn build(&self, app: &mut App) {
        // Register panel
        let world = app.world_mut();
        let mut registry = world
            .remove_resource::<PanelRegistry>()
            .unwrap_or_default();
        registry.register(HierarchyPanel::default());
        world.insert_resource(registry);

        // Register built-in entity presets
        let mut spawn_reg = world
            .remove_resource::<SpawnRegistry>()
            .unwrap_or_default();
        register_builtin_presets(&mut spawn_reg);
        world.insert_resource(spawn_reg);
    }
}

fn register_builtin_presets(registry: &mut SpawnRegistry) {
    // Empty
    registry.register(EntityPreset {
        id: "empty_entity",
        display_name: "Empty Entity",
        icon: regular::CIRCLE,
        category: "general",
        spawn_fn: |world| {
            world
                .spawn((Name::new("Empty Entity"), Transform::default()))
                .id()
        },
    });

    // 3D Objects
    registry.register(EntityPreset {
        id: "cube",
        display_name: "Cube",
        icon: regular::CUBE,
        category: "rendering",
        spawn_fn: |world| {
            let mesh = world
                .resource_mut::<Assets<Mesh>>()
                .add(Cuboid::default());
            let material = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default());
            world
                .spawn((
                    Name::new("Cube"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "sphere",
        display_name: "Sphere",
        icon: regular::GLOBE,
        category: "rendering",
        spawn_fn: |world| {
            let mesh = world
                .resource_mut::<Assets<Mesh>>()
                .add(Sphere::default());
            let material = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default());
            world
                .spawn((
                    Name::new("Sphere"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "plane",
        display_name: "Plane",
        icon: regular::SQUARE,
        category: "rendering",
        spawn_fn: |world| {
            let mesh = world
                .resource_mut::<Assets<Mesh>>()
                .add(Plane3d::default().mesh().size(10.0, 10.0));
            let material = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default());
            world
                .spawn((
                    Name::new("Plane"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "cylinder",
        display_name: "Cylinder",
        icon: regular::CYLINDER,
        category: "rendering",
        spawn_fn: |world| {
            let mesh = world
                .resource_mut::<Assets<Mesh>>()
                .add(Cylinder::default());
            let material = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default());
            world
                .spawn((
                    Name::new("Cylinder"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                ))
                .id()
        },
    });

    // Lights
    registry.register(EntityPreset {
        id: "directional_light",
        display_name: "Directional Light",
        icon: regular::SUN,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Directional Light"),
                    Transform::default(),
                    DirectionalLight::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "point_light",
        display_name: "Point Light",
        icon: regular::LIGHTBULB,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Point Light"),
                    Transform::default(),
                    PointLight::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "spot_light",
        display_name: "Spot Light",
        icon: regular::FLASHLIGHT,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Spot Light"),
                    Transform::default(),
                    SpotLight::default(),
                ))
                .id()
        },
    });

    // Camera
    registry.register(EntityPreset {
        id: "camera_3d",
        display_name: "Camera 3D",
        icon: regular::VIDEO_CAMERA,
        category: "camera",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Camera 3D"),
                    Transform::default(),
                    Camera3d::default(),
                ))
                .id()
        },
    });
}
