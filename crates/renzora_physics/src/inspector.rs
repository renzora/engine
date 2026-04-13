//! Inspector entries for physics components.
//!
//! Registered automatically when the `editor` feature is enabled.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor_framework::{
    inline_property, toggle_switch, EditorCommands, InspectorEntry,
};
use renzora_theme::Theme;

use crate::{
    CharacterControllerData, CharacterControllerState, CharacterControllerInput,
    CollisionShapeData, CollisionShapeType,
    PhysicsBodyData, PhysicsBodyType,
};

/// Register all physics inspector entries, spawn presets, icons, and observers via `AppEditorExt`.
pub fn register_physics_inspectors(app: &mut App) {
    // Auto-insert default collider on new MeshPrimitive entities
    app.add_observer(auto_insert_collider_for_shape);
    use renzora_editor_framework::{AppEditorExt, ComponentIconEntry, EntityPreset};

    app.register_inspector(physics_body_entry())
       .register_inspector(collision_shape_entry())
       .register_inspector(character_controller_entry());

    // Spawn presets
    app.register_entity_preset(EntityPreset {
        id: "rigid_body",
        display_name: "Rigid Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("RigidBody3D"),
                    Transform::default(),
                    PhysicsBodyData::default(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "static_body",
        display_name: "Static Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("StaticBody3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "kinematic_body",
        display_name: "Kinematic Body",
        icon: regular::CUBE,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("KinematicBody3D"),
                    Transform::default(),
                    PhysicsBodyData::kinematic_body(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "box_collider",
        display_name: "Box Collider",
        icon: regular::BOUNDING_BOX,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("BoxShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::default(),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "sphere_collider",
        display_name: "Sphere Collider",
        icon: regular::GLOBE_HEMISPHERE_EAST,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("SphereShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::sphere(0.5),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "capsule_collider",
        display_name: "Capsule Collider",
        icon: regular::PILL,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("CapsuleShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::capsule(0.5, 0.5),
                ))
                .id()
        },
    });
    app.register_entity_preset(EntityPreset {
        id: "cylinder_collider",
        display_name: "Cylinder Collider",
        icon: regular::CYLINDER,
        category: "physics",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("CylinderShape3D"),
                    Transform::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::cylinder(0.5, 0.5),
                ))
                .id()
        },
    });
}

/// Map a shape ID to a default collision shape for auto-physics on spawned meshes.
pub fn default_collider_for_shape(id: &str) -> Option<CollisionShapeData> {
    Some(match id {
        "cube"       => CollisionShapeData::cuboid(Vec3::splat(0.5)),
        "sphere"     => CollisionShapeData::sphere(0.5),
        "cylinder"   => CollisionShapeData::cylinder(0.5, 0.5),
        "capsule"    => CollisionShapeData::capsule(0.5, 0.25),
        "cone"       => CollisionShapeData::cylinder(0.5, 0.5),
        "hemisphere" => CollisionShapeData::sphere(0.5),
        "plane"      => CollisionShapeData::cuboid(Vec3::new(0.5, 0.001, 0.5)),
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
        "pipe"   => CollisionShapeData::cylinder(0.5, 0.5),
        "ring"   => CollisionShapeData::cylinder(0.5, 0.1),
        "funnel" => CollisionShapeData::cylinder(0.5, 0.5),
        "gutter" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.25, 0.5)),
        "torus"  => CollisionShapeData::cylinder(0.5, 0.15),
        "prism"   => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        "pyramid" => CollisionShapeData::cuboid(Vec3::new(0.5, 0.5, 0.5)),
        _ => return None,
    })
}

// ── Physics Body ────────────────────────────────────────────────────────────

fn physics_body_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "physics_body",
        display_name: "Physics Body",
        icon: regular::CUBE,
        category: "physics",
        has_fn: |world, entity| world.get::<PhysicsBodyData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PhysicsBodyData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<PhysicsBodyData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(physics_body_ui),
    }
}

fn physics_body_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(body) = world.get::<PhysicsBodyData>(entity) else { return };
    let body = body.clone();

    // Body type combo
    inline_property(ui, 0, "Body Type", theme, |ui| {
        let current = match body.body_type {
            PhysicsBodyType::RigidBody => "Rigid Body",
            PhysicsBodyType::StaticBody => "Static Body",
            PhysicsBodyType::KinematicBody => "Kinematic Body",
        };
        egui::ComboBox::from_id_salt("physics_body_type")
            .selected_text(current)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (bt, label) in [
                    (PhysicsBodyType::RigidBody, "Rigid Body"),
                    (PhysicsBodyType::StaticBody, "Static Body"),
                    (PhysicsBodyType::KinematicBody, "Kinematic Body"),
                ] {
                    if ui.selectable_label(body.body_type == bt, label).clicked() {
                        cmds.push(move |w: &mut World| {
                            if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                                b.body_type = bt;
                            }
                        });
                    }
                }
            });
    });

    // Mass
    inline_property(ui, 1, "Mass", theme, |ui| {
        let mut v = body.mass;
        if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.001..=f32::MAX)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.mass = v; }
            });
        }
    });

    // Gravity Scale
    inline_property(ui, 0, "Gravity Scale", theme, |ui| {
        let mut v = body.gravity_scale;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(-10.0..=10.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.gravity_scale = v; }
            });
        }
    });

    // Linear Damping
    inline_property(ui, 1, "Linear Damping", theme, |ui| {
        let mut v = body.linear_damping;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=100.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.linear_damping = v; }
            });
        }
    });

    // Angular Damping
    inline_property(ui, 0, "Angular Damping", theme, |ui| {
        let mut v = body.angular_damping;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=100.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) { b.angular_damping = v; }
            });
        }
    });

    // Lock axes
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Lock Axes").size(11.0).color(theme.text.secondary.to_color32()));

    inline_property(ui, 1, "Translation", theme, |ui| {
        ui.horizontal(|ui| {
            let locks = [
                ("X", body.lock_translation_x),
                ("Y", body.lock_translation_y),
                ("Z", body.lock_translation_z),
            ];
            for (i, (label, current)) in locks.iter().enumerate() {
                let mut checked = *current;
                if ui.checkbox(&mut checked, *label).changed() {
                    let val = checked;
                    let axis = i;
                    cmds.push(move |w: &mut World| {
                        if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                            match axis {
                                0 => b.lock_translation_x = val,
                                1 => b.lock_translation_y = val,
                                _ => b.lock_translation_z = val,
                            }
                        }
                    });
                }
            }
        });
    });

    inline_property(ui, 0, "Rotation", theme, |ui| {
        ui.horizontal(|ui| {
            let locks = [
                ("X", body.lock_rotation_x),
                ("Y", body.lock_rotation_y),
                ("Z", body.lock_rotation_z),
            ];
            for (i, (label, current)) in locks.iter().enumerate() {
                let mut checked = *current;
                if ui.checkbox(&mut checked, *label).changed() {
                    let val = checked;
                    let axis = i;
                    cmds.push(move |w: &mut World| {
                        if let Some(mut b) = w.get_mut::<PhysicsBodyData>(entity) {
                            match axis {
                                0 => b.lock_rotation_x = val,
                                1 => b.lock_rotation_y = val,
                                _ => b.lock_rotation_z = val,
                            }
                        }
                    });
                }
            }
        });
    });
}

// ── Collision Shape ─────────────────────────────────────────────────────────

fn collision_shape_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "collision_shape",
        display_name: "Collision Shape",
        icon: regular::BOUNDING_BOX,
        category: "physics",
        has_fn: |world, entity| world.get::<CollisionShapeData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CollisionShapeData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CollisionShapeData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(collision_shape_ui),
    }
}

fn collision_shape_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(shape) = world.get::<CollisionShapeData>(entity) else { return };
    let shape = shape.clone();

    // Shape type combo
    inline_property(ui, 0, "Shape", theme, |ui| {
        let current = match shape.shape_type {
            CollisionShapeType::Box => "Box",
            CollisionShapeType::Sphere => "Sphere",
            CollisionShapeType::Capsule => "Capsule",
            CollisionShapeType::Cylinder => "Cylinder",
        };
        egui::ComboBox::from_id_salt("collision_shape_type")
            .selected_text(current)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (st, label) in [
                    (CollisionShapeType::Box, "Box"),
                    (CollisionShapeType::Sphere, "Sphere"),
                    (CollisionShapeType::Capsule, "Capsule"),
                    (CollisionShapeType::Cylinder, "Cylinder"),
                ] {
                    if ui.selectable_label(shape.shape_type == st, label).clicked() {
                        cmds.push(move |w: &mut World| {
                            if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                                s.shape_type = st;
                            }
                        });
                    }
                }
            });
    });

    // Shape-specific parameters
    match shape.shape_type {
        CollisionShapeType::Box => {
            inline_property(ui, 1, "Half Extents", theme, |ui| {
                let mut v = [shape.half_extents.x, shape.half_extents.y, shape.half_extents.z];
                let mut changed = false;
                ui.horizontal(|ui| {
                    for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                        ui.label(egui::RichText::new(*label).size(10.0).color(theme.text.muted.to_color32()));
                        if ui.add(egui::DragValue::new(&mut v[i]).speed(0.01).range(0.001..=f32::MAX)).changed() {
                            changed = true;
                        }
                    }
                });
                if changed {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                            s.half_extents = Vec3::new(v[0], v[1], v[2]);
                        }
                    });
                }
            });
        }
        CollisionShapeType::Sphere => {
            inline_property(ui, 1, "Radius", theme, |ui| {
                let mut v = shape.radius;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.radius = v; }
                    });
                }
            });
        }
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            inline_property(ui, 1, "Radius", theme, |ui| {
                let mut v = shape.radius;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.radius = v; }
                    });
                }
            });
            inline_property(ui, 0, "Half Height", theme, |ui| {
                let mut v = shape.half_height;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                    cmds.push(move |w: &mut World| {
                        if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.half_height = v; }
                    });
                }
            });
        }
    }

    // Offset
    inline_property(ui, 1, "Offset", theme, |ui| {
        let mut v = [shape.offset.x, shape.offset.y, shape.offset.z];
        let mut changed = false;
        ui.horizontal(|ui| {
            for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
                ui.label(egui::RichText::new(*label).size(10.0).color(theme.text.muted.to_color32()));
                if ui.add(egui::DragValue::new(&mut v[i]).speed(0.01)).changed() {
                    changed = true;
                }
            }
        });
        if changed {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) {
                    s.offset = Vec3::new(v[0], v[1], v[2]);
                }
            });
        }
    });

    // Friction
    inline_property(ui, 0, "Friction", theme, |ui| {
        let mut v = shape.friction;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=2.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.friction = v; }
            });
        }
    });

    // Restitution
    inline_property(ui, 1, "Restitution", theme, |ui| {
        let mut v = shape.restitution;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=2.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.restitution = v; }
            });
        }
    });

    // Is Sensor
    inline_property(ui, 0, "Is Sensor", theme, |ui| {
        let id = ui.id().with("collision_is_sensor");
        if toggle_switch(ui, id, shape.is_sensor) {
            let val = !shape.is_sensor;
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_mut::<CollisionShapeData>(entity) { s.is_sensor = val; }
            });
        }
    });
}

// ── Character Controller ────────────────────────────────────────────────────

fn character_controller_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "character_controller",
        display_name: "Character Controller",
        icon: regular::PERSON_SIMPLE_RUN,
        category: "physics",
        has_fn: |world, entity| world.get::<CharacterControllerData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(CharacterControllerData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<CharacterControllerData>();
            world.entity_mut(entity).remove::<CharacterControllerState>();
            world.entity_mut(entity).remove::<CharacterControllerInput>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(character_controller_ui),
    }
}

fn character_controller_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(cc) = world.get::<CharacterControllerData>(entity) else { return };
    let cc = cc.clone();

    // Auto Input toggle
    inline_property(ui, 0, "Auto Input", theme, |ui| {
        let id = ui.id().with("cc_auto_input");
        if toggle_switch(ui, id, cc.auto_input) {
            let val = !cc.auto_input;
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.auto_input = val; }
            });
        }
    });

    // Action names (only shown when auto_input is on)
    if cc.auto_input {
        inline_property(ui, 1, "Move Action", theme, |ui| {
            let mut v = cc.move_action.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(100.0)).changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.move_action = v; }
                });
            }
        });
        inline_property(ui, 0, "Jump Action", theme, |ui| {
            let mut v = cc.jump_action.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(100.0)).changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.jump_action = v; }
                });
            }
        });
        inline_property(ui, 1, "Sprint Action", theme, |ui| {
            let mut v = cc.sprint_action.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(100.0)).changed() {
                cmds.push(move |w: &mut World| {
                    if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.sprint_action = v; }
                });
            }
        });
    }

    // Move Speed
    inline_property(ui, 0, "Move Speed", theme, |ui| {
        let mut v = cc.move_speed;
        if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.0..=100.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.move_speed = v; }
            });
        }
    });

    // Sprint Multiplier
    inline_property(ui, 1, "Sprint Multiplier", theme, |ui| {
        let mut v = cc.sprint_multiplier;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(1.0..=5.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.sprint_multiplier = v; }
            });
        }
    });

    // Jump Force
    inline_property(ui, 0, "Jump Force", theme, |ui| {
        let mut v = cc.jump_force;
        if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.0..=50.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.jump_force = v; }
            });
        }
    });

    // Gravity Scale
    inline_property(ui, 1, "Gravity Scale", theme, |ui| {
        let mut v = cc.gravity_scale;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(-10.0..=10.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.gravity_scale = v; }
            });
        }
    });

    // Max Slope Angle
    inline_property(ui, 0, "Max Slope Angle", theme, |ui| {
        let mut v = cc.max_slope_angle;
        if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(0.0..=90.0).suffix("°")).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.max_slope_angle = v; }
            });
        }
    });

    // Ground Distance
    inline_property(ui, 1, "Ground Distance", theme, |ui| {
        let mut v = cc.ground_distance;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.01..=1.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.ground_distance = v; }
            });
        }
    });

    // Air Control
    inline_property(ui, 0, "Air Control", theme, |ui| {
        let mut v = cc.air_control;
        if ui.add(egui::DragValue::new(&mut v).speed(0.05).range(0.0..=1.0)).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.air_control = v; }
            });
        }
    });

    // Coyote Time
    inline_property(ui, 1, "Coyote Time", theme, |ui| {
        let mut v = cc.coyote_time;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0).suffix("s")).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.coyote_time = v; }
            });
        }
    });

    // Jump Buffer Time
    inline_property(ui, 0, "Jump Buffer", theme, |ui| {
        let mut v = cc.jump_buffer_time;
        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0).suffix("s")).changed() {
            cmds.push(move |w: &mut World| {
                if let Some(mut c) = w.get_mut::<CharacterControllerData>(entity) { c.jump_buffer_time = v; }
            });
        }
    });

    // Runtime state (read-only, shown when playing)
    if let Some(state) = world.get::<CharacterControllerState>(entity) {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Runtime State").size(11.0).color(theme.text.muted.to_color32()));
        inline_property(ui, 1, "Grounded", theme, |ui| {
            ui.label(if state.is_grounded { "Yes" } else { "No" });
        });
        inline_property(ui, 0, "Velocity", theme, |ui| {
            ui.label(format!("{:.1}, {:.1}, {:.1}", state.velocity.x, state.velocity.y, state.velocity.z));
        });
        inline_property(ui, 1, "Speed", theme, |ui| {
            let speed = Vec3::new(state.velocity.x, 0.0, state.velocity.z).length();
            ui.label(format!("{:.1}", speed));
        });
    }
}

/// Auto-insert a default static body + collider on newly spawned MeshPrimitive entities.
fn auto_insert_collider_for_shape(
    trigger: On<Insert, renzora::MeshPrimitive>,
    mut commands: Commands,
    query: Query<&renzora::MeshPrimitive, Without<CollisionShapeData>>,
) {
    let entity = trigger.entity;
    if let Ok(prim) = query.get(entity) {
        if let Some(collider) = default_collider_for_shape(&prim.0) {
            commands.entity(entity).insert((
                PhysicsBodyData::static_body(),
                collider,
            ));
        }
    }
}
