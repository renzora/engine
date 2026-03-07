//! Hierarchy panel — shows the scene entity tree.

mod state;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{
    icon_button, search_overlay, AppEditorExt, EditorCommands, EditorPanel, EditorSelection, EntityPreset,
    OverlayAction, OverlayEntry, PanelLocation, SpawnRegistry,
};
use renzora_core::{MeshPrimitive, MeshColor};
use renzora_theme::ThemeManager;

use state::{build_entity_tree, filter_tree, HierarchyState};

/// Label color presets: ([r, g, b], name).
pub const LABEL_COLORS: &[([u8; 3], &str)] = &[
    ([220, 70,  70],  "Red"),
    ([210, 120, 80],  "Coral"),
    ([220, 140, 60],  "Orange"),
    ([210, 175, 55],  "Amber"),
    ([210, 195, 60],  "Yellow"),
    ([160, 210, 60],  "Lime"),
    ([70,  190, 100], "Green"),
    ([55,  185, 155], "Teal"),
    ([60,  200, 200], "Cyan"),
    ([70,  170, 220], "Sky"),
    ([80,  140, 220], "Blue"),
    ([90,  100, 220], "Indigo"),
    ([155, 80,  220], "Purple"),
    ([190, 70,  200], "Violet"),
    ([220, 80,  180], "Pink"),
    ([220, 80,  120], "Rose"),
    ([160, 110, 75],  "Brown"),
    ([130, 130, 140], "Gray"),
    ([200, 200, 200], "White"),
];

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

        let commands = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };

        let selection = match world.get_resource::<EditorSelection>() {
            Some(s) => s,
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
                    if let Some(registry) = world.get_resource::<SpawnRegistry>() {
                        if let Some(preset) = registry.iter().find(|p| p.id == id) {
                            let spawn_fn = preset.spawn_fn;
                            commands.push(move |world: &mut World| {
                                let entity = spawn_fn(world);
                                if let Some(sel) = world.get_resource::<EditorSelection>() {
                                    sel.set(Some(entity));
                                }
                            });
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

        // Reset drop target each frame
        state.drop_target = None;

        // Render the tree
        let state = &mut *state;
        egui::ScrollArea::vertical()
            .id_salt("hierarchy_tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;
                tree::render_tree(
                    ui,
                    &nodes,
                    state,
                    selection,
                    commands,
                    &theme,
                );
            });

        // Swap visible entity order for next frame's range selection
        std::mem::swap(&mut state.visible_entity_order, &mut state.building_entity_order);

        // Handle drag release → apply reparent
        if !state.drag_entities.is_empty() && !ui.ctx().input(|i| i.pointer.any_down()) {
            if let Some((target, zone)) = state.drop_target.take() {
                let drag_entities = std::mem::take(&mut state.drag_entities);
                commands.push(move |world: &mut World| {
                    use renzora_editor::TreeDropZone;
                    for entity in &drag_entities {
                        if *entity == target {
                            continue;
                        }
                        match zone {
                            TreeDropZone::AsChild => {
                                world.entity_mut(*entity).set_parent_in_place(target);
                            }
                            TreeDropZone::Before | TreeDropZone::After => {
                                let parent = world.get::<ChildOf>(target).map(|c| c.parent());
                                world.entity_mut(*entity).remove_parent_in_place();
                                if let Some(p) = parent {
                                    world.entity_mut(*entity).set_parent_in_place(p);
                                }
                            }
                        }
                    }
                });
            } else {
                state.drag_entities.clear();
            }
        }

        // Drag tooltip
        if !state.drag_entities.is_empty() {
            if let Some(pos) = ui.ctx().pointer_latest_pos() {
                let count = state.drag_entities.len();
                let label = if count == 1 {
                    "Moving 1 entity".to_string()
                } else {
                    format!("Moving {} entities", count)
                };
                egui::Area::new(egui::Id::new("hierarchy_drag_tooltip"))
                    .fixed_pos(pos + egui::vec2(12.0, 4.0))
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(egui::RichText::new(label).size(11.0));
                        });
                    });
            }
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
        app.register_panel(HierarchyPanel::default());

        app.init_resource::<SpawnRegistry>();
        register_builtin_presets(
            &mut app.world_mut().resource_mut::<SpawnRegistry>(),
        );
    }
}

fn register_builtin_presets(registry: &mut SpawnRegistry) {
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
                .add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.3, 0.2),
                    ..default()
                });
            let color = Color::srgb(0.8, 0.3, 0.2);
            world
                .spawn((
                    Name::new("Cube"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    MeshPrimitive::Cube,
                    MeshColor(color),
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
                .add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.5, 0.8),
                    ..default()
                });
            let color = Color::srgb(0.2, 0.5, 0.8);
            world
                .spawn((
                    Name::new("Sphere"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    MeshPrimitive::Sphere,
                    MeshColor(color),
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
                .add(StandardMaterial {
                    base_color: Color::srgb(0.35, 0.35, 0.35),
                    ..default()
                });
            let color = Color::srgb(0.35, 0.35, 0.35);
            world
                .spawn((
                    Name::new("Plane"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    MeshPrimitive::Plane { width: 10.0, height: 10.0 },
                    MeshColor(color),
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
                .add(StandardMaterial {
                    base_color: Color::srgb(0.3, 0.7, 0.4),
                    ..default()
                });
            let color = Color::srgb(0.3, 0.7, 0.4);
            world
                .spawn((
                    Name::new("Cylinder"),
                    Transform::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    MeshPrimitive::Cylinder,
                    MeshColor(color),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "sun",
        display_name: "Sun",
        icon: regular::SUN_HORIZON,
        category: "lighting",
        spawn_fn: |world| {
            let data = renzora_lighting::SunData::default();
            let dir = data.direction();
            world
                .spawn((
                    Name::new("Sun"),
                    Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir)),
                    DirectionalLight {
                        color: Color::srgb(data.color.x, data.color.y, data.color.z),
                        illuminance: data.illuminance,
                        shadows_enabled: data.shadows_enabled,
                        ..default()
                    },
                    data,
                ))
                .id()
        },
    });

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

    registry.register(EntityPreset {
        id: "ambient_light",
        display_name: "Ambient Light",
        icon: regular::SUN_DIM,
        category: "lighting",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Ambient Light"),
                    AmbientLight {
                        color: Color::WHITE,
                        brightness: 300.0,
                        ..default()
                    },
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "camera_3d",
        display_name: "Camera 3D",
        icon: regular::VIDEO_CAMERA,
        category: "camera",
        spawn_fn: |world| {
            // Count existing scene cameras to generate a unique name
            let mut count = 0u32;
            let mut q = world.query_filtered::<(), With<renzora_core::SceneCamera>>();
            for _ in q.iter(world) {
                count += 1;
            }
            let name = if count == 0 {
                "Camera 3D".to_string()
            } else {
                format!("Camera 3D ({})", count + 1)
            };
            world
                .spawn((
                    Name::new(name),
                    Transform::default(),
                    Camera3d::default(),
                    Camera {
                        is_active: false,
                        ..default()
                    },
                    renzora_core::SceneCamera,
                ))
                .id()
        },
    });
}
