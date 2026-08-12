//! Editor-only half of `renzora_water` — the inspector entries for the water
//! components (`WaterSurface`, `Buoyant`), each a renzora editor-contract
//! `InspectorEntry` with a Phosphor icon and `FieldDef` list.
//!
//! `renzora_water` compiles lean (no `editor` feature, no egui-phosphor). This
//! crate holds the inspector entries (which read/write the `pub` runtime
//! components in `renzora_water`), registered
//! `renzora::add!(WaterEditorPlugin, Editor)`, linked only by the editor bundle.
//!
//! **Cascade editing.** The reference project's tuning GUI puts each cascade
//! behind a tab, and only one cascade's parameters are on screen at a time. The
//! inspector contract has a flat `Vec<FieldDef>` with no grouping widget, so
//! that shape is reproduced with a "Cascade" selector: one set of per-cascade
//! fields that reads and writes whichever cascade [`WaterInspectorState`] has
//! selected. The alternative — a flat block per cascade — was what this file
//! used to do, and it cost eleven rows times eight cascades of scrolling to
//! reach anything.

use bevy::prelude::*;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_water::{
    Buoyant, WaterMeshMode, WaterMeshQuality, WaterSurface, WaveCascade, MAX_CASCADES,
};

/// Which cascade the per-cascade inspector fields are currently editing.
///
/// Editor-only view state, so it lives in a resource rather than on
/// `WaterSurface` — a scene should not remember which tab someone last had
/// open. Only one water surface drives the simulation, so one selection for the
/// whole editor is the right granularity.
#[derive(Resource, Default)]
pub struct WaterInspectorState {
    pub selected_cascade: usize,
}

/// The selected cascade index, clamped to what this surface actually has.
///
/// Clamping on read rather than on write is what keeps the selector honest when
/// the cascade *count* drops below the selection — the fields fall back to the
/// last cascade instead of reading `None` and rendering as permanently empty.
fn selected(world: &World, entity: Entity) -> Option<usize> {
    let count = world.get::<WaterSurface>(entity)?.cascades.len();
    if count == 0 {
        return None;
    }
    let index = world
        .get_resource::<WaterInspectorState>()
        .map(|s| s.selected_cascade)
        .unwrap_or(0);
    Some(index.min(count - 1))
}

/// One `f32` field on the *selected* cascade.
macro_rules! cascade_float {
    ($name:literal, $field:ident, $speed:expr, $min:expr, $max:expr) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Float {
                speed: $speed,
                min: $min,
                max: $max,
            },
            get_fn: |world, entity| {
                let index = selected(world, entity)?;
                world
                    .get::<WaterSurface>(entity)
                    .and_then(|s| s.cascades.get(index))
                    .map(|c| FieldValue::Float(c.$field))
            },
            set_fn: |world, entity, val| {
                let (FieldValue::Float(v), Some(index)) = (val, selected(world, entity)) else {
                    return;
                };
                if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                    if let Some(c) = s.cascades.get_mut(index) {
                        c.$field = v;
                    }
                }
            },
        }
    };
}

/// One axis of the selected cascade's tile length.
///
/// Both axes are exposed because a deliberately non-square tile is how you
/// stretch a swell along the wind — the reference's GUI shows them as a pair for
/// the same reason. Keeping them equal is still the common case.
macro_rules! cascade_tile {
    ($name:literal, $axis:ident) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Float {
                speed: 0.5,
                min: 1.0,
                max: 1000.0,
            },
            get_fn: |world, entity| {
                let index = selected(world, entity)?;
                world
                    .get::<WaterSurface>(entity)
                    .and_then(|s| s.cascades.get(index))
                    .map(|c| FieldValue::Float(c.tile_length.$axis))
            },
            set_fn: |world, entity, val| {
                let (FieldValue::Float(v), Some(index)) = (val, selected(world, entity)) else {
                    return;
                };
                if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                    if let Some(c) = s.cascades.get_mut(index) {
                        c.tile_length.$axis = v.max(1.0);
                    }
                }
            },
        }
    };
}

/// Simple float field on `WaterSurface` itself.
macro_rules! surface_float {
    ($name:literal, $field:ident, $speed:expr, $min:expr, $max:expr) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Float {
                speed: $speed,
                min: $min,
                max: $max,
            },
            get_fn: |world, entity| {
                world
                    .get::<WaterSurface>(entity)
                    .map(|s| FieldValue::Float(s.$field))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Float(v) = val {
                    if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                        s.$field = v;
                    }
                }
            },
        }
    };
}

/// Whole-number field on `WaterSurface` (values still travel as floats).
macro_rules! surface_int {
    ($name:literal, $field:ident, $min:expr, $max:expr) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Int {
                min: $min,
                max: $max,
            },
            get_fn: |world, entity| {
                world
                    .get::<WaterSurface>(entity)
                    .map(|s| FieldValue::Float(s.$field as f32))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Float(v) = val {
                    if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                        s.$field = v.round().max(0.0) as u32;
                    }
                }
            },
        }
    };
}

macro_rules! surface_bool {
    ($name:literal, $field:ident) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Bool,
            get_fn: |world, entity| {
                world
                    .get::<WaterSurface>(entity)
                    .map(|s| FieldValue::Bool(s.$field))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Bool(v) = val {
                    if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                        s.$field = v;
                    }
                }
            },
        }
    };
}

macro_rules! surface_color {
    ($name:literal, $field:ident) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Color,
            get_fn: |world, entity| {
                world
                    .get::<WaterSurface>(entity)
                    .map(|s| FieldValue::Color(s.$field))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Color(v) = val {
                    if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                        s.$field = v;
                    }
                }
            },
        }
    };
}

// ============================================================================
// WaterSurface inspector entry
// ============================================================================

/// Build the manual inspector entry for `WaterSurface`.
pub fn water_inspector_entry() -> InspectorEntry {
    let fields = vec![
        // ── Appearance ──
        surface_color!("Water Color", water_color),
        surface_color!("Foam Color", foam_color),
        surface_float!("Roughness", roughness, 0.01, 0.0, 1.0),
        surface_float!("Normal Strength", normal_strength, 0.01, 0.0, 1.0),
        // ── Sea state ──
        surface_float!("Sea Depth", sea_depth, 0.5, 0.5, 500.0),
        surface_int!("Seed", seed, 0.0, 100000.0),
        // ── Performance ──
        FieldDef {
            name: "Wave Resolution",
            field_type: FieldType::Enum {
                options: &["128", "256", "512", "1024"],
            },
            get_fn: |world, entity| {
                world
                    .get::<WaterSurface>(entity)
                    .map(|s| FieldValue::Enum(s.clamped_map_size().to_string()))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Enum(v) = val {
                    if let Ok(size) = v.parse::<u32>() {
                        if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                            s.map_size = size;
                        }
                    }
                }
            },
        },
        surface_float!("Updates / Second", updates_per_second, 1.0, 0.0, 60.0),
        surface_bool!("Enable Sea Spray", enable_sea_spray),
        // ── Mesh ──
        FieldDef {
            name: "Mesh Mode",
            field_type: FieldType::Enum {
                options: &["grid", "clipmap"],
            },
            get_fn: |world, entity| {
                world.get::<WaterSurface>(entity).map(|s| {
                    FieldValue::Enum(
                        match s.mesh_mode {
                            WaterMeshMode::Grid => "grid",
                            WaterMeshMode::Clipmap => "clipmap",
                        }
                        .to_string(),
                    )
                })
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Enum(v) = val {
                    if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                        s.mesh_mode = if v == "clipmap" {
                            WaterMeshMode::Clipmap
                        } else {
                            WaterMeshMode::Grid
                        };
                    }
                }
            },
        },
        FieldDef {
            name: "Wave Mesh Quality",
            field_type: FieldType::Enum {
                options: &["low", "medium", "high", "custom"],
            },
            get_fn: |world, entity| {
                world.get::<WaterSurface>(entity).map(|s| {
                    FieldValue::Enum(
                        match s.mesh_quality {
                            WaterMeshQuality::Low => "low",
                            WaterMeshQuality::Medium => "medium",
                            WaterMeshQuality::High => "high",
                            WaterMeshQuality::Custom => "custom",
                        }
                        .to_string(),
                    )
                })
            },
            set_fn: |world, entity, val| {
                let FieldValue::Enum(v) = val else { return };
                let quality = match v.as_str() {
                    "low" => WaterMeshQuality::Low,
                    "medium" => WaterMeshQuality::Medium,
                    "custom" => WaterMeshQuality::Custom,
                    _ => WaterMeshQuality::High,
                };
                if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                    // Seed the raw fields from the preset on the way into
                    // Custom, so "custom" starts from what was on screen rather
                    // than snapping to whatever the fields last held.
                    if quality == WaterMeshQuality::Custom
                        && s.mesh_quality != WaterMeshQuality::Custom
                    {
                        let (rings, resolution, quad_size) = s.clipmap_params();
                        s.clipmap_rings = rings;
                        s.clipmap_resolution = resolution;
                        s.clipmap_quad_size = quad_size;
                    }
                    s.mesh_quality = quality;
                }
            },
        },
        surface_float!("Grid Size", mesh_size, 1.0, 1.0, 5000.0),
        surface_int!("Grid Subdivisions", subdivisions, 1.0, 512.0),
        surface_int!("Clipmap Rings", clipmap_rings, 0.0, 16.0),
        surface_int!("Clipmap Resolution", clipmap_resolution, 4.0, 256.0),
        surface_float!("Clipmap Quad Size", clipmap_quad_size, 0.05, 0.1, 16.0),
        // ── Wave parameters ──
        FieldDef {
            name: "Cascades",
            field_type: FieldType::Int {
                min: 1.0,
                max: MAX_CASCADES as f32,
            },
            get_fn: |world, entity| {
                world
                    .get::<WaterSurface>(entity)
                    .map(|s| FieldValue::Float(s.cascades.len() as f32))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Float(v) = val {
                    let count = (v.round() as usize).clamp(1, MAX_CASCADES);
                    if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                        // Added cascades start from the default (a mid-scale
                        // wind sea) rather than a copy of cascade 0, which
                        // would double one frequency band instead of adding a
                        // new one.
                        s.cascades.resize_with(count, WaveCascade::default);
                    }
                }
            },
        },
        // The "tab bar": every field below edits whichever cascade this picks.
        // 1-based, because the reference's tabs read "Cascade 1".
        FieldDef {
            name: "Cascade",
            field_type: FieldType::Int {
                min: 1.0,
                max: MAX_CASCADES as f32,
            },
            get_fn: |world, entity| {
                selected(world, entity).map(|i| FieldValue::Float(i as f32 + 1.0))
            },
            set_fn: |world, entity, val| {
                let FieldValue::Float(v) = val else { return };
                let count = world
                    .get::<WaterSurface>(entity)
                    .map(|s| s.cascades.len())
                    .unwrap_or(0);
                if count == 0 {
                    return;
                }
                let index = (v.round().max(1.0) as usize - 1).min(count - 1);
                if let Some(mut state) = world.get_resource_mut::<WaterInspectorState>() {
                    state.selected_cascade = index;
                }
            },
        },
        cascade_tile!("Tile Length X", x),
        cascade_tile!("Tile Length Y", y),
        cascade_float!("Time Scale", time_scale, 0.01, 0.0, 4.0),
        cascade_float!("Wind Speed", wind_speed, 0.1, 0.0, 40.0),
        // Authored in degrees, stored in radians — the component is what the
        // shader and the spectrum read, and a heading is a thing artists type
        // in degrees.
        FieldDef {
            name: "Wind Direction (deg)",
            field_type: FieldType::Float {
                speed: 1.0,
                min: -360.0,
                max: 360.0,
            },
            get_fn: |world, entity| {
                let index = selected(world, entity)?;
                world
                    .get::<WaterSurface>(entity)
                    .and_then(|s| s.cascades.get(index))
                    .map(|c| FieldValue::Float(c.wind_direction.to_degrees()))
            },
            set_fn: |world, entity, val| {
                let (FieldValue::Float(v), Some(index)) = (val, selected(world, entity)) else {
                    return;
                };
                if let Some(mut s) = world.get_mut::<WaterSurface>(entity) {
                    if let Some(c) = s.cascades.get_mut(index) {
                        c.wind_direction = v.to_radians();
                    }
                }
            },
        },
        cascade_float!("Fetch Length (km)", fetch_length, 1.0, 0.1, 1000.0),
        cascade_float!("Swell", swell, 0.01, 0.0, 2.0),
        cascade_float!("Detail", detail, 0.01, 0.0, 1.0),
        cascade_float!("Spread", spread, 0.01, 0.0, 1.0),
        cascade_float!("Whitecap", whitecap, 0.01, 0.0, 2.0),
        cascade_float!("Foam Amount", foam_amount, 0.05, 0.0, 10.0),
        cascade_float!("Displacement Scale", displacement_scale, 0.01, 0.0, 2.0),
        cascade_float!("Normal Scale", normal_scale, 0.01, 0.0, 2.0),
    ];

    InspectorEntry {
        type_id: "water_surface",
        display_name: "Water Surface",
        icon: "waves",
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
        fields,
    }
}

// ============================================================================
// Buoyant inspector entry
// ============================================================================

pub fn buoyant_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "buoyant",
        display_name: "Buoyant",
        icon: "lifebuoy",
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

/// Editor-scope companion to `renzora_water::WaterPlugin`.
#[derive(Default)]
pub struct WaterEditorPlugin;

impl Plugin for WaterEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] WaterEditorPlugin");
        app.init_resource::<WaterInspectorState>();
        app.register_inspector(water_inspector_entry());
        app.register_inspector(buoyant_inspector_entry());
    }
}

renzora::add!(WaterEditorPlugin, Editor);
