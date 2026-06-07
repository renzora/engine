//! Editor-only half of `renzora_water` — the inspector entries for the water
//! components (`WaterSurface`, `WaterInteractor`, `Buoyant`), each a renzora
//! editor-contract `InspectorEntry` with a Phosphor icon and `FieldDef` list.
//!
//! `renzora_water` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the inspector entries (which read/write the `pub` runtime
//! components in `renzora_water`), registered
//! `renzora::add!(WaterEditorPlugin, Editor)`, linked only by the editor bundle.

use bevy::prelude::*;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_water::{Buoyant, WaterInteractor, WaterSurface};

// ============================================================================
// WaterSurface inspector entry
// ============================================================================

/// Build the manual inspector entry for WaterSurface with color fields.
pub fn water_inspector_entry() -> InspectorEntry {
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
        fields: vec![
            // ── Colors ──
            FieldDef {
                name: "Deep Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Color(s.deep_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.deep_color = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Shallow Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Color(s.shallow_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.shallow_color = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Foam Color",
                field_type: FieldType::Color,
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Color(s.foam_color))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Color(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.foam_color = v;
                        }
                    }
                },
            },
            // ── Float params ──
            FieldDef {
                name: "Foam Threshold",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.foam_threshold))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.foam_threshold = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Absorption",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.absorption))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.absorption = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Roughness",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.01,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.roughness))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.roughness = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Subsurface",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.subsurface_strength))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.subsurface_strength = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Mesh Size",
                field_type: FieldType::Float {
                    speed: 1.0,
                    min: 10.0,
                    max: 1000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.mesh_size))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.mesh_size = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Refraction Strength",
                field_type: FieldType::Float {
                    speed: 0.005,
                    min: 0.0,
                    max: 0.2,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.refraction_strength))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.refraction_strength = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Max Depth",
                field_type: FieldType::Float {
                    speed: 0.5,
                    min: 1.0,
                    max: 50.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.max_depth))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.max_depth = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Caustic Intensity",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.caustic_intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.caustic_intensity = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Specular Power",
                field_type: FieldType::Float {
                    speed: 100.0,
                    min: 100.0,
                    max: 10000.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.specular_power))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.specular_power = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Foam Depth",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.foam_depth))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.foam_depth = v;
                        }
                    }
                },
            },
            // ── Wind ──
            FieldDef {
                name: "Wind Speed",
                field_type: FieldType::Float {
                    speed: 0.01,
                    min: 0.0,
                    max: 1.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.wind_speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.wind_speed = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Wind Angle",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: std::f32::consts::TAU,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterSurface>(entity)
                        .map(|s| FieldValue::Float(s.wind_angle))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.wind_angle = v;
                        }
                    }
                },
            },
        ],
    }
}

// ============================================================================
// WaterInteractor inspector entry
// ============================================================================

pub fn water_interactor_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "water_interactor",
        display_name: "Water Interactor",
        icon: egui_phosphor::regular::WAVES,
        category: "physics",
        has_fn: |world, entity| world.get::<WaterInteractor>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(WaterInteractor {
                radius: 2.0,
                intensity: 1.0,
            });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<WaterInteractor>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Radius",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.1,
                    max: 20.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterInteractor>(entity)
                        .map(|s| FieldValue::Float(s.radius))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterInteractor>(entity) {
                            s.radius = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.0,
                    max: 2.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<WaterInteractor>(entity)
                        .map(|s| FieldValue::Float(s.intensity))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterInteractor>(entity) {
                            s.intensity = v;
                        }
                    }
                },
            },
        ],
    }
}

// ============================================================================
// Buoyant inspector entry
// ============================================================================

pub fn buoyant_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "buoyant",
        display_name: "Buoyant",
        icon: egui_phosphor::regular::LIFEBUOY,
        category: "physics",
        has_fn: |world, entity| world.get::<Buoyant>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Buoyant::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Buoyant>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Force",
                field_type: FieldType::Float {
                    speed: 0.5,
                    min: 0.0,
                    max: 200.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.force))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.force = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Damping",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.damping))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.damping = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Submerge Depth",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.1,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.submerge_depth))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.submerge_depth = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Wave Push",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.wave_push))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.wave_push = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Drag",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.drag))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.drag = v;
                        }
                    }
                },
            },
        ],
    }
}

// ============================================================================
// Plugin
// ============================================================================

/// Editor-scope companion to `renzora_water::WaterPlugin`. Reproduces the
/// inspector registrations the runtime plugin did under `#[cfg(feature = "editor")]`.
#[derive(Default)]
pub struct WaterEditorPlugin;

impl Plugin for WaterEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] WaterEditorPlugin");
        app.register_inspector(water_inspector_entry());
        app.register_inspector(water_interactor_inspector_entry());
        app.register_inspector(buoyant_inspector_entry());
    }
}

renzora::add!(WaterEditorPlugin, Editor);
