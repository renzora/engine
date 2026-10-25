//! Editor-only half of `renzora_solari`: the "Add Component" inspector entry
//! for [`SolariGi`].
//!
//! `renzora_solari` compiles lean (no `editor` feature). This crate holds the
//! inspector, registered `renzora::add!(SolariEditorPlugin, Editor)` and linked
//! only by the editor bundle. The entry is registered even when GPU ray tracing
//! is unavailable, so the component stays discoverable and serializable; the
//! runtime plugin's systems simply don't run in that case.

use bevy::prelude::*;
use bevy::solari::realtime::SolariLighting;
use renzora::{
    bool_field, AppEditorExt, DlssMode, DlssSupport, FieldDef, FieldType, FieldValue,
    InspectorEntry, SolariGi, SolariQuality,
};

fn solari_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "solari_gi",
        display_name: "Solari Ray-Traced GI",
        icon: "lightning",
        category: "lighting",
        has_fn: |world, entity| world.get::<SolariGi>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SolariGi::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<SolariGi>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<SolariGi>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SolariGi>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            // First, because it is the setting that moves frame rate and noise
            // the most. The labels come from the contract enum so the dropdown
            // and the saved value can't drift apart.
            FieldDef {
                name: "Quality",
                field_type: FieldType::Enum {
                    options: &SolariQuality::LABELS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SolariGi>(entity)
                        .map(|s| FieldValue::Enum(s.quality.label().to_string()))
                },
                set_fn: |world, entity, value| {
                    if let FieldValue::Enum(label) = value {
                        if let (Some(quality), Some(mut s)) = (
                            SolariQuality::from_label(&label),
                            world.get_mut::<SolariGi>(entity),
                        ) {
                            s.quality = quality;
                        }
                    }
                },
            },
            // Renzora's own denoiser, which works on every ray-tracing GPU.
            // Exposed so the raw estimate can be compared against it.
            bool_field!("Denoiser", SolariGi, denoise),
            // DLSS Ray Reconstruction where it can run, the denoiser above
            // where it cannot. On by default, so a project gets DLSS on an RTX
            // machine and still renders cleanly everywhere else.
            bool_field!("DLSS", SolariGi, dlss),
            FieldDef {
                name: "DLSS Mode",
                field_type: FieldType::Enum {
                    options: &DlssMode::LABELS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SolariGi>(entity)
                        .map(|s| FieldValue::Enum(s.dlss_mode.label().to_string()))
                },
                set_fn: |world, entity, value| {
                    if let FieldValue::Enum(label) = value {
                        if let (Some(mode), Some(mut s)) =
                            (DlssMode::from_label(&label), world.get_mut::<SolariGi>(entity))
                        {
                            s.dlss_mode = mode;
                        }
                    }
                },
            },
            // Which denoiser is actually running, and why, because the toggle
            // alone cannot say: DLSS needs a build with the SDK and an RTX GPU
            // on the Vulkan renderer, and falls back silently without them.
            FieldDef {
                name: "Denoising With",
                field_type: FieldType::ReadOnly,
                get_fn: |world, entity| {
                    let gi = world.get::<SolariGi>(entity)?;
                    let support = world.get_resource::<DlssSupport>().copied().unwrap_or_default();
                    let builtin = if gi.denoise { "Built-in denoiser" } else { "Nothing (raw)" };
                    let text = match (gi.dlss, support.compiled, support.ray_reconstruction) {
                        (true, true, true) => "DLSS Ray Reconstruction".to_string(),
                        (true, false, _) => format!("{builtin}: this build has no DLSS SDK"),
                        (true, true, false) => {
                            format!("{builtin}: GPU or renderer has no DLSS Ray Reconstruction")
                        }
                        (false, ..) => builtin.to_string(),
                    };
                    Some(FieldValue::ReadOnly(text))
                },
                set_fn: |_, _, _| {},
            },
            // Solari traces visibility instead of sampling shadow maps, and its
            // camera carries `SkipDeferredLighting`, which removes the only
            // pass that would read one. Every cascade and point-light cubemap is
            // therefore rendered and discarded. Suppression is applied to the
            // EXTRACTED lights in the render world, so toggling this never
            // writes to the user's light components (see `suppress_shadow_maps`
            // in renzora_solari); it is exposed here because keeping raster
            // shadows is a legitimate thing to want while comparing backends.
            bool_field!("Suppress Shadow Maps", SolariGi, suppress_shadow_maps),
            // Solari samples only directional lights and emissive meshes, and
            // its camera's `SkipDeferredLighting` removes the clustered-light
            // pass that would otherwise apply point/spot lights, so without
            // this they contribute nothing whatsoever. Exposed because the
            // stand-in spheres are an approximation (a spot loses its cone), and
            // because seeing Solari's raw behaviour is sometimes the point.
            bool_field!("Point/Spot Light Proxies", SolariGi, light_proxies),
            // Solari's realtime component (`SolariLighting`) exposes exactly one
            // runtime knob: `reset`, which clears the temporal history. Surface
            // it as a button, handy after a camera cut or when ghosting/dark
            // trails appear while the estimate re-converges. It pokes every
            // active Solari camera; `clear_solari_reset` flips the flag back off
            // after the frame is extracted (see renzora_solari).
            FieldDef {
                name: "Reset Temporal History",
                field_type: FieldType::Button { icon: "arrow-counter-clockwise" },
                get_fn: |_w, _e| None,
                set_fn: |world, _e, v| {
                    if let FieldValue::Bool(true) = v {
                        let mut q = world.query::<&mut SolariLighting>();
                        for mut s in q.iter_mut(world) {
                            s.reset = true;
                        }
                    }
                },
            },
        ],
    }
}

/// Editor-scope companion to `renzora_solari::SolariPlugin`.
#[derive(Default)]
pub struct SolariEditorPlugin;

impl Plugin for SolariEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SolariEditorPlugin");
        app.register_inspector(solari_inspector_entry());
    }
}

renzora::add!(SolariEditorPlugin, Editor);
