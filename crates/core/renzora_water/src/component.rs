use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Single Gerstner wave definition.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct GerstnerWave {
    /// Normalized XZ direction of wave travel.
    pub direction: Vec2,
    /// Steepness (Q parameter), 0.0 = sine wave, 1.0 = sharp crests.
    pub steepness: f32,
    /// Distance between wave crests in world units.
    pub wavelength: f32,
    /// Wave height (half the crest-to-trough distance).
    pub amplitude: f32,
}

impl Default for GerstnerWave {
    fn default() -> Self {
        Self {
            direction: Vec2::new(1.0, 0.0),
            steepness: 0.5,
            wavelength: 10.0,
            amplitude: 0.3,
        }
    }
}

/// Tag component for water surface entities.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct WaterSurface {
    pub deep_color: [f32; 3],
    pub shallow_color: [f32; 3],
    pub foam_color: [f32; 3],
    pub foam_threshold: f32,
    pub absorption: f32,
    pub roughness: f32,
    pub subsurface_strength: f32,
    pub mesh_size: f32,
    pub waves: Vec<GerstnerWave>,
    pub subdivisions: u32,
}

impl Default for WaterSurface {
    fn default() -> Self {
        Self {
            deep_color: [0.005, 0.02, 0.08],
            shallow_color: [0.04, 0.22, 0.28],
            foam_color: [0.82, 0.88, 0.92],
            foam_threshold: 0.4,
            absorption: 0.3,
            roughness: 0.15,
            subsurface_strength: 0.3,
            mesh_size: 200.0,
            subdivisions: 256,
            waves: vec![
                GerstnerWave {
                    direction: Vec2::new(0.8, 0.6).normalize(),
                    steepness: 0.4,
                    wavelength: 12.0,
                    amplitude: 0.35,
                },
                GerstnerWave {
                    direction: Vec2::new(-0.3, 0.95).normalize(),
                    steepness: 0.3,
                    wavelength: 8.0,
                    amplitude: 0.2,
                },
                GerstnerWave {
                    direction: Vec2::new(0.5, -0.5).normalize(),
                    steepness: 0.5,
                    wavelength: 5.0,
                    amplitude: 0.12,
                },
                GerstnerWave {
                    direction: Vec2::new(-0.7, -0.3).normalize(),
                    steepness: 0.35,
                    wavelength: 3.5,
                    amplitude: 0.08,
                },
            ],
        }
    }
}

/// Build the manual inspector entry for WaterSurface with color fields.
#[cfg(feature = "editor")]
pub fn water_inspector_entry() -> renzora_editor::InspectorEntry {
    use renzora_editor::{InspectorEntry, FieldDef, FieldType, FieldValue};

    InspectorEntry {
        type_id: "water_surface",
        display_name: "Water Surface",
        icon: egui_phosphor::regular::WAVES,
        category: "rendering",
        has_fn: |world, entity| world.get::<WaterSurface>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(WaterSurface::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<WaterSurface>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        custom_ui_fn: None,
        fields: vec![
            // ── Colors ──
            FieldDef {
                name: "Deep Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Color(s.deep_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.deep_color = v; }
                    }
                },
            },
            FieldDef {
                name: "Shallow Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Color(s.shallow_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.shallow_color = v; }
                    }
                },
            },
            FieldDef {
                name: "Foam Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Color(s.foam_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.foam_color = v; }
                    }
                },
            },
            // ── Float params ──
            FieldDef {
                name: "Foam Threshold",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Float(s.foam_threshold))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.foam_threshold = v; }
                    }
                },
            },
            FieldDef {
                name: "Absorption",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Float(s.absorption))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.absorption = v; }
                    }
                },
            },
            FieldDef {
                name: "Roughness",
                field_type: FieldType::Float { speed: 0.01, min: 0.01, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Float(s.roughness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.roughness = v; }
                    }
                },
            },
            FieldDef {
                name: "Subsurface",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Float(s.subsurface_strength))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.subsurface_strength = v; }
                    }
                },
            },
            FieldDef {
                name: "Mesh Size",
                field_type: FieldType::Float { speed: 1.0, min: 10.0, max: 1000.0 },
                get_fn: |world, entity| {
                    world.get::<WaterSurface>(entity).map(|s| FieldValue::Float(s.mesh_size))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) { s.mesh_size = v; }
                    }
                },
            },
        ],
    }
}
