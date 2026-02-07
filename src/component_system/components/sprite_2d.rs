//! Sprite 2D component definition

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::json;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::register_component;
use crate::shared::Sprite2DData;
use crate::ui::property_row;

use egui_phosphor::regular::IMAGE;

// ============================================================================
// Custom Add/Remove/Serialize/Deserialize
// ============================================================================

fn add_sprite_2d(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        Sprite {
            color: Color::WHITE,
            ..default()
        },
        Sprite2DData::default(),
    ));
}

fn remove_sprite_2d(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Sprite>()
        .remove::<Sprite2DData>();
}

fn serialize_sprite_2d(world: &World, entity: Entity) -> Option<serde_json::Value> {
    let data = world.get::<Sprite2DData>(entity)?;
    let sprite = world.get::<Sprite>(entity)?;
    let srgba = sprite.color.to_srgba();

    Some(json!({
        "texture_path": data.texture_path,
        "color": [srgba.red, srgba.green, srgba.blue, srgba.alpha],
        "flip_x": data.flip_x,
        "flip_y": data.flip_y,
        "anchor": [data.anchor.x, data.anchor.y]
    }))
}

fn deserialize_sprite_2d(
    entity_commands: &mut EntityCommands,
    data: &serde_json::Value,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    let texture_path = data
        .get("texture_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let color = data
        .get("color")
        .and_then(|c| c.as_array())
        .map(|arr| {
            Color::srgba(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            )
        })
        .unwrap_or(Color::WHITE);

    let flip_x = data
        .get("flip_x")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let flip_y = data
        .get("flip_y")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let anchor = data
        .get("anchor")
        .and_then(|a| a.as_array())
        .map(|arr| {
            Vec2::new(
                arr.first().and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.5) as f32,
            )
        })
        .unwrap_or(Vec2::new(0.5, 0.5));

    entity_commands.insert((
        Sprite {
            color,
            flip_x,
            flip_y,
            ..default()
        },
        Sprite2DData {
            texture_path,
            color: Vec4::new(
                color.to_srgba().red,
                color.to_srgba().green,
                color.to_srgba().blue,
                color.to_srgba().alpha,
            ),
            flip_x,
            flip_y,
            anchor,
        },
    ));
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_sprite_2d(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    // Get mutable references to components
    let Some(mut sprite) = world.get_mut::<Sprite>(entity) else {
        return false;
    };

    // Color
    property_row(ui, 0, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let srgba = sprite.color.to_srgba();
                let mut color = egui::Color32::from_rgba_unmultiplied(
                    (srgba.red * 255.0) as u8,
                    (srgba.green * 255.0) as u8,
                    (srgba.blue * 255.0) as u8,
                    (srgba.alpha * 255.0) as u8,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    sprite.color = Color::srgba(
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    );
                    changed = true;
                }
            });
        });
    });

    // Flip X
    property_row(ui, 1, |ui| {
        ui.horizontal(|ui| {
            ui.label("Flip X");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut sprite.flip_x, "").changed() {
                    changed = true;
                }
            });
        });
    });

    // Flip Y
    property_row(ui, 2, |ui| {
        ui.horizontal(|ui| {
            ui.label("Flip Y");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut sprite.flip_y, "").changed() {
                    changed = true;
                }
            });
        });
    });

    // Update Sprite2DData to match - copy values first, then update
    drop(sprite);

    // Get sprite values
    let sprite_values = world.get::<Sprite>(entity).map(|sprite| {
        let srgba = sprite.color.to_srgba();
        (sprite.flip_x, sprite.flip_y, Vec4::new(srgba.red, srgba.green, srgba.blue, srgba.alpha))
    });

    if let (Some((flip_x, flip_y, color)), Some(mut sprite_data)) =
        (sprite_values, world.get_mut::<Sprite2DData>(entity))
    {
        sprite_data.flip_x = flip_x;
        sprite_data.flip_y = flip_y;
        sprite_data.color = color;
    }

    changed
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(Sprite2DData {
        type_id: "sprite_2d",
        display_name: "Sprite 2D",
        category: ComponentCategory::Rendering,
        icon: IMAGE,
        priority: 1,
        conflicts_with: ["mesh_renderer"],
        custom_inspector: inspect_sprite_2d,
        custom_add: add_sprite_2d,
        custom_remove: remove_sprite_2d,
        custom_serialize: serialize_sprite_2d,
        custom_deserialize: deserialize_sprite_2d,
    }));
}
