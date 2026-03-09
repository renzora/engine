//! Hierarchy panel — shows the scene entity tree.

mod state;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{
    icon_button, search_overlay, AppEditorExt, EditorCommands, EditorPanel, EditorSelection, EntityPreset,
    InspectorRegistry, OverlayAction, OverlayEntry, PanelLocation, SpawnRegistry,
};
use renzora_core::{MeshPrimitive, MeshColor, ShapeRegistry};
use renzora_physics::{CollisionShapeData, PhysicsBodyData};
use renzora_scripting::ScriptComponent;
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
            let mut entries: Vec<OverlayEntry> = Vec::new();

            // Add SpawnRegistry presets (lights, cameras, etc.)
            if let Some(registry) = world.get_resource::<SpawnRegistry>() {
                entries.extend(registry.iter().map(|p| OverlayEntry {
                    id: p.id,
                    label: p.display_name,
                    icon: p.icon,
                    category: p.category,
                }));
            }

            // Add ShapeRegistry shapes (meshes)
            if let Some(shape_reg) = world.get_resource::<ShapeRegistry>() {
                entries.extend(shape_reg.iter().map(|s| OverlayEntry {
                    id: s.id,
                    label: s.name,
                    icon: s.icon,
                    category: s.category,
                }));
            }

            // Add components from InspectorRegistry (post-processing, rendering, effects, audio)
            if let Some(inspector_reg) = world.get_resource::<InspectorRegistry>() {
                let component_categories = &["rendering", "post_process", "effects", "Audio"];
                for entry in inspector_reg.iter() {
                    if entry.add_fn.is_some() && component_categories.contains(&entry.category) {
                        entries.push(OverlayEntry {
                            id: entry.type_id,
                            label: entry.display_name,
                            icon: entry.icon,
                            category: entry.category,
                        });
                    }
                }
            }

            let ctx = ui.ctx().clone();
            match search_overlay(&ctx, "add_entity_overlay", "Add Entity", &entries, &mut state.add_search, &theme) {
                OverlayAction::Selected(id) => {
                    state.show_add_overlay = false;

                    // Try SpawnRegistry first (lights, cameras, etc.)
                    let mut handled = false;
                    if let Some(registry) = world.get_resource::<SpawnRegistry>() {
                        if let Some(preset) = registry.iter().find(|p| p.id == id) {
                            let spawn_fn = preset.spawn_fn;
                            commands.push(move |world: &mut World| {
                                let entity = spawn_fn(world);
                                world.entity_mut(entity).insert(ScriptComponent::new());
                                if let Some(sel) = world.get_resource::<EditorSelection>() {
                                    sel.set(Some(entity));
                                }
                            });
                            handled = true;
                        }
                    }

                    // Fall back to ShapeRegistry (meshes)
                    if !handled {
                        if let Some(shape_reg) = world.get_resource::<ShapeRegistry>() {
                            if let Some(entry) = shape_reg.get(&id) {
                                let create_mesh = entry.create_mesh;
                                let color = entry.default_color;
                                let name = entry.name;
                                let shape_id = entry.id;
                                commands.push(move |world: &mut World| {
                                    let mesh = create_mesh(&mut world.resource_mut::<Assets<Mesh>>());
                                    let material = world
                                        .resource_mut::<Assets<StandardMaterial>>()
                                        .add(StandardMaterial {
                                            base_color: color,
                                            perceptual_roughness: 0.9,
                                            ..default()
                                        });
                                    let mut entity_cmds = world.spawn((
                                        Name::new(name),
                                        Transform::default(),
                                        Mesh3d(mesh),
                                        MeshMaterial3d(material),
                                        MeshPrimitive(shape_id.to_string()),
                                        MeshColor(color),
                                    ));
                                    entity_cmds.insert(ScriptComponent::new());
                                    if let Some(collider) = default_collider_for_shape(shape_id) {
                                        entity_cmds.insert((
                                            PhysicsBodyData::static_body(),
                                            collider,
                                        ));
                                    }
                                    let entity = entity_cmds.id();
                                    if let Some(sel) = world.get_resource::<EditorSelection>() {
                                        sel.set(Some(entity));
                                    }
                                });
                                handled = true;
                            }
                        }
                    }

                    // Fall back to InspectorRegistry (components as entities)
                    if !handled {
                        if let Some(inspector_reg) = world.get_resource::<InspectorRegistry>() {
                            if let Some(entry) = inspector_reg.iter().find(|e| e.type_id == id) {
                                if let Some(add_fn) = entry.add_fn {
                                    let display_name = entry.display_name;
                                    commands.push(move |world: &mut World| {
                                        let entity = world
                                            .spawn((Name::new(display_name), Transform::default(), ScriptComponent::new()))
                                            .id();
                                        add_fn(world, entity);
                                        if let Some(sel) = world.get_resource::<EditorSelection>() {
                                            sel.set(Some(entity));
                                        }
                                    });
                                }
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
        id: "world_environment",
        display_name: "World Environment",
        icon: regular::GLOBE,
        category: "general",
        spawn_fn: |world| {
            let sun = renzora_lighting::Sun::default();
            let dir = sun.direction();
            world
                .spawn((
                    Name::new("World Environment"),
                    Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir)),
                    DirectionalLight {
                        color: Color::srgb(sun.color.x, sun.color.y, sun.color.z),
                        illuminance: sun.illuminance,
                        shadows_enabled: sun.shadows_enabled,
                        ..default()
                    },
                    sun,
                    renzora_bloom_effect::BloomSettings::default(),
                    renzora_atmosphere::AtmosphereComponentSettings::default(),
                    renzora_clouds::CloudsData::default(),
                    renzora_distance_fog::DistanceFogSettings::default(),
                ))
                .id()
        },
    });

    // Note: rendering shapes (cube, sphere, etc.) are registered via ShapeRegistry
    // and shown in both the shape library panel and this overlay.

    registry.register(EntityPreset {
        id: "sun",
        display_name: "Sun",
        icon: regular::SUN_HORIZON,
        category: "lighting",
        spawn_fn: |world| {
            let data = renzora_lighting::Sun::default();
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

    // ── Physics ──────────────────────────────────────────────────────────────

    registry.register(EntityPreset {
        id: "rigid_body",
        display_name: "Rigid Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("RigidBody3D"),
                    Transform::default(),
                    renzora_physics::PhysicsBodyData::default(),
                    renzora_physics::CollisionShapeData::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "static_body",
        display_name: "Static Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("StaticBody3D"),
                    Transform::default(),
                    renzora_physics::PhysicsBodyData::static_body(),
                    renzora_physics::CollisionShapeData::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "kinematic_body",
        display_name: "Kinematic Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("KinematicBody3D"),
                    Transform::default(),
                    renzora_physics::PhysicsBodyData::kinematic_body(),
                    renzora_physics::CollisionShapeData::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "box_collider",
        display_name: "Box Collider",
        icon: regular::BOUNDING_BOX,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("BoxShape3D"),
                    Transform::default(),
                    renzora_physics::PhysicsBodyData::static_body(),
                    renzora_physics::CollisionShapeData::default(),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "sphere_collider",
        display_name: "Sphere Collider",
        icon: regular::GLOBE_HEMISPHERE_EAST,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("SphereShape3D"),
                    Transform::default(),
                    renzora_physics::PhysicsBodyData::static_body(),
                    renzora_physics::CollisionShapeData::sphere(0.5),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "capsule_collider",
        display_name: "Capsule Collider",
        icon: regular::PILL,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("CapsuleShape3D"),
                    Transform::default(),
                    renzora_physics::PhysicsBodyData::static_body(),
                    renzora_physics::CollisionShapeData::capsule(0.5, 0.5),
                ))
                .id()
        },
    });

    registry.register(EntityPreset {
        id: "cylinder_collider",
        display_name: "Cylinder Collider",
        icon: regular::CYLINDER,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("CylinderShape3D"),
                    Transform::default(),
                    renzora_physics::PhysicsBodyData::static_body(),
                    renzora_physics::CollisionShapeData::cylinder(0.5, 0.5),
                ))
                .id()
        },
    });

    // ── Camera ──────────────────────────────────────────────────────────────

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

/// Map a shape ID to a default collision shape. Returns `None` for complex shapes
/// where an automatic collider wouldn't be a good fit.
fn default_collider_for_shape(id: &str) -> Option<CollisionShapeData> {
    Some(match id {
        // Basic primitives
        "cube"       => CollisionShapeData::cuboid(Vec3::splat(0.5)),
        "sphere"     => CollisionShapeData::sphere(0.5),
        "cylinder"   => CollisionShapeData::cylinder(0.5, 0.5),
        "capsule"    => CollisionShapeData::capsule(0.5, 0.25),
        "cone"       => CollisionShapeData::cylinder(0.5, 0.5),
        "hemisphere" => CollisionShapeData::sphere(0.5),

        // Flat surfaces
        "plane"      => CollisionShapeData::cuboid(Vec3::new(0.5, 0.001, 0.5)),

        // Level geometry — box approximations
        "wedge"       => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "wall"        => CollisionShapeData::cuboid(Vec3::new(0.5, 1.0, 0.05)),
        "ramp"        => CollisionShapeData::cuboid(Vec3::new(0.5, 0.25, 1.0)),
        "doorway"     => CollisionShapeData::cuboid(Vec3::new(0.5, 1.0, 0.05)),
        "window_wall" => CollisionShapeData::cuboid(Vec3::new(0.5, 1.0, 0.05)),
        "pillar"      => CollisionShapeData::cylinder(0.15, 1.0),
        "l_shape"     => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "t_shape"     => CollisionShapeData::cuboid(Vec3::new(0.75, 0.5, 0.5)),
        "cross_shape" => CollisionShapeData::cuboid(Vec3::new(0.75, 0.5, 0.75)),
        "corner"      => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "stairs"      => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "half_cylinder" => CollisionShapeData::cylinder(0.5, 0.5),
        "quarter_pipe"  => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "curved_wall"   => CollisionShapeData::cylinder(0.5, 1.0),
        "spiral_stairs" => CollisionShapeData::cylinder(0.5, 1.0),

        // Curved shapes
        "pipe"   => CollisionShapeData::cylinder(0.5, 0.5),
        "ring"   => CollisionShapeData::cylinder(0.5, 0.1),
        "funnel" => CollisionShapeData::cylinder(0.5, 0.5),
        "gutter" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.25, 0.5)),
        "torus"  => CollisionShapeData::cylinder(0.5, 0.15),

        // Advanced
        "prism"   => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "pyramid" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),

        _ => return None,
    })
}
