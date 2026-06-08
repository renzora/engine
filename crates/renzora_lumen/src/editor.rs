//! Editor-only half of the GI plugin: the Lumen + RT inspectors and the
//! diagnostics snapshot the debugger's Lumen panel reads.
//!
//! Compiled only with the `editor` feature. `LumenPlugin` loads at Runtime
//! scope, so in a shipped game (no editor framework) these registrations are
//! harmless no-ops; in the editor they wire the "Add Component" inspector
//! entries and feed the debugger's `LumenDiagState`.

use bevy::prelude::*;
use renzora::{
    AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry, LumenBakeSnapshot,
    LumenCameraEntry, LumenDebug, LumenDiagState, LumenLighting, LumenQuality, RtDebugMode,
    RtLighting,
};

use crate::{LumenBakeStats, LumenSkyCubemap, MeshVoxelSamples, VoxelCacheView};

/// Register the Lumen + RT inspector entries with the editor contract.
pub(crate) fn register_inspectors(app: &mut App) {
    info!("[editor] GI inspectors (Lumen + RT)");
    app.register_inspector(lumen_inspector_entry());
    app.register_inspector(rt_inspector_entry());
}

fn lumen_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "lumen_lighting",
        display_name: "Lumen Global Illumination",
        icon: "lightning",
        category: "lighting",
        has_fn: |world, entity| world.get::<LumenLighting>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(LumenLighting::default());
            // Lumen owns the screen-space tier when present — strip any
            // hand-attached `RtLighting` so the two don't double-apply.
            world.entity_mut(entity).remove::<RtLighting>();
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(LumenLighting, RtLighting)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<LumenLighting>(entity)
                .map(|s| s.quality != LumenQuality::Off)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<LumenLighting>(entity) {
                s.quality = if val { LumenQuality::ScreenSpace } else { LumenQuality::Off };
            }
        }),
        fields: vec![
            FieldDef {
                name: "Quality",
                field_type: FieldType::Enum {
                    options: &[
                        "Off",
                        "Screen Space (SSGI)",
                        "SDF Low (Phase 5)",
                        "SDF High (Phase 5)",
                        "Hardware RT (Phase 10)",
                    ],
                },
                get_fn: |w, e| {
                    w.get::<LumenLighting>(e).map(|c| {
                        FieldValue::Enum(
                            match c.quality {
                                LumenQuality::Off => "Off",
                                LumenQuality::ScreenSpace => "Screen Space (SSGI)",
                                LumenQuality::SdfLow => "SDF Low (Phase 5)",
                                LumenQuality::SdfHigh => "SDF High (Phase 5)",
                                LumenQuality::Hwrt => "Hardware RT (Phase 10)",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(s), Some(mut c)) = (v, w.get_mut::<LumenLighting>(e)) {
                        c.quality = match s.as_str() {
                            "Off" => LumenQuality::Off,
                            "SDF Low (Phase 5)" => LumenQuality::SdfLow,
                            "SDF High (Phase 5)" => LumenQuality::SdfHigh,
                            "Hardware RT (Phase 10)" => LumenQuality::Hwrt,
                            _ => LumenQuality::ScreenSpace,
                        };
                    }
                },
            },
            renzora::float_field!("Intensity", LumenLighting, intensity, 0.05, 0.0, 5.0),
            FieldDef {
                name: "Debug",
                field_type: FieldType::Enum {
                    options: &["None", "Indirect Only", "Voxel Cache"],
                },
                get_fn: |w, e| {
                    w.get::<LumenLighting>(e).map(|c| {
                        FieldValue::Enum(
                            match c.debug {
                                LumenDebug::None => "None",
                                LumenDebug::IndirectOnly => "Indirect Only",
                                LumenDebug::VoxelCache => "Voxel Cache",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(s), Some(mut c)) = (v, w.get_mut::<LumenLighting>(e)) {
                        c.debug = match s.as_str() {
                            "Indirect Only" => LumenDebug::IndirectOnly,
                            "Voxel Cache" => LumenDebug::VoxelCache,
                            _ => LumenDebug::None,
                        };
                    }
                },
            },
        ],
    }
}

fn rt_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "rt_lighting",
        display_name: "RT Lighting (SSGI)",
        icon: "lightning",
        category: "lighting",
        has_fn: |world, entity| world.get::<RtLighting>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(RtLighting::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<RtLighting>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<RtLighting>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<RtLighting>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::float_field!("Intensity", RtLighting, intensity, 0.05, 0.0, 5.0),
            FieldDef {
                name: "Debug",
                field_type: FieldType::Enum {
                    options: &["Composite", "Indirect Only"],
                },
                get_fn: |w, e| {
                    w.get::<RtLighting>(e).map(|s| {
                        FieldValue::Enum(
                            match s.debug {
                                RtDebugMode::Composite => "Composite",
                                RtDebugMode::IndirectOnly => "Indirect Only",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(label), Some(mut s)) = (v, w.get_mut::<RtLighting>(e)) {
                        s.debug = match label.as_str() {
                            "Indirect Only" => RtDebugMode::IndirectOnly,
                            _ => RtDebugMode::Composite,
                        };
                    }
                },
            },
        ],
    }
}

/// Copy the GI plugin's internal bake/coverage state into the contract
/// `LumenDiagState` snapshot the debugger's Lumen panel renders.
pub(crate) fn update_lumen_diag_state(
    mut state: ResMut<LumenDiagState>,
    bake_stats: Option<Res<LumenBakeStats>>,
    cameras: Query<(Option<&Name>, &VoxelCacheView)>,
    sample_entities: Query<(), With<MeshVoxelSamples>>,
    // LumenSkyCubemap is a Component on the editor camera (extracted to the
    // render world each frame). Any entity with it = a sky/IBL source is bound.
    sky_cubemaps: Query<(), With<LumenSkyCubemap>>,
) {
    state.cameras.clear();
    state.cameras.extend(cameras.iter().map(|(name, view)| LumenCameraEntry {
        camera_name: name
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| "<unnamed>".into()),
        inject_active: view.inject_active,
        debug_active: view.debug_active,
    }));
    state.mesh_voxel_samples_entities = sample_entities.iter().count();
    state.has_sky_cubemap = !sky_cubemaps.is_empty();
    state.bake = bake_stats
        .map(|s| LumenBakeSnapshot {
            last_bake_dur: s.last_bake_dur,
            avg_bake_dur: s.avg_bake_dur,
            max_bake_dur: s.max_bake_dur,
            bakes_last_frame: s.bakes_last_frame,
            total_bakes: s.total_bakes,
            total_samples_baked: s.total_samples_baked,
            bake_budget_per_frame: s.bake_budget_per_frame,
        })
        .unwrap_or_default();
}
