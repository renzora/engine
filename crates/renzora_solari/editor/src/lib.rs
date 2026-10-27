//! Editor-only half of `renzora_solari`: the "Add Component" inspector entry
//! for [`SolariGi`], and the Ray Tracing debug panel (`panel.rs`).
//!
//! `renzora_solari` compiles lean (no `editor` feature). This crate holds the
//! inspector, registered `renzora::add!(SolariEditorPlugin, Editor)` and linked
//! only by the editor bundle. The entry is registered even when GPU ray tracing
//! is unavailable, so the component stays discoverable and serializable; the
//! runtime plugin's systems simply don't run in that case.

mod panel;

use bevy::prelude::*;
use bevy::solari::realtime::SolariLighting;
use renzora::{
    bool_field, AppEditorExt, DlssMode, DlssSupport, FieldDef, FieldType, FieldValue,
    InspectorEntry, SolariDenoiser, SolariGi, SolariQuality,
};

/// Whether DLSS Ray Reconstruction is what actually denoises `gi` on this run,
/// given what this build and GPU support: the same decision `renzora_solari`'s
/// `dlss.rs` makes.
fn dlss_running(world: &World, gi: &SolariGi) -> bool {
    let support = world.get_resource::<DlssSupport>().copied().unwrap_or_default();
    gi.denoiser_choice().wants_dlss() && support.ray_reconstruction
}

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
                            // A preset brings its own ReSTIR setting back.
                            s.restir = None;
                        }
                    }
                },
            },
            // ReSTIR reuses light samples across neighbouring pixels and past
            // frames: far less noise for a few more rays. The quality preset
            // picks it (off on Low); this overrides the preset either way.
            FieldDef {
                name: "ReSTIR",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<SolariGi>(entity)
                        .map(|s| FieldValue::Bool(s.restir_enabled()))
                },
                set_fn: |world, entity, value| {
                    if let (FieldValue::Bool(on), Some(mut s)) = (value, world.get_mut::<SolariGi>(entity)) {
                        s.restir = Some(on);
                    }
                },
            },
            // Glass, clear coats and lenses traced so they reflect the scene;
            // off, they reflect only the sky, as before Solari traced them.
            FieldDef {
                name: "Glass Reflections",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<SolariGi>(entity)
                        .map(|s| FieldValue::Bool(s.glass_reflections))
                },
                set_fn: |world, entity, value| {
                    if let (FieldValue::Bool(on), Some(mut s)) = (value, world.get_mut::<SolariGi>(entity)) {
                        s.glass_reflections = on;
                    }
                },
            },
            // One choice of what removes the noise. DLSS Ray Reconstruction is
            // itself a denoiser, so a DLSS toggle beside a denoiser toggle read
            // as two settings where there was one, and the denoiser toggle did
            // nothing while DLSS ran. "Off" shows the raw estimate in any mode.
            FieldDef {
                name: "Denoiser",
                field_type: FieldType::Enum {
                    options: &SolariDenoiser::LABELS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<SolariGi>(entity)
                        .map(|s| FieldValue::Enum(s.denoiser_choice().label().to_string()))
                },
                set_fn: |world, entity, value| {
                    if let FieldValue::Enum(label) = value {
                        if let (Some(choice), Some(mut s)) = (
                            SolariDenoiser::from_label(&label),
                            world.get_mut::<SolariGi>(entity),
                        ) {
                            s.set_denoiser(choice);
                        }
                    }
                },
            },
            // Only while DLSS is the denoiser actually running: anywhere else
            // the frame renders at full size and the setting does nothing.
            FieldDef {
                name: "DLSS Mode",
                field_type: FieldType::Enum {
                    options: &DlssMode::LABELS,
                },
                get_fn: |world, entity| {
                    let gi = world.get::<SolariGi>(entity)?;
                    dlss_running(world, gi)
                        .then(|| FieldValue::Enum(gi.dlss_mode.label().to_string()))
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
        use renzora::RenzoraShellExt;
        use renzora_ember::panel::RegisterPanelContent;

        info!("[editor] SolariEditorPlugin");
        app.register_inspector(solari_inspector_entry());

        // The Ray Tracing panel (panel.rs).
        app.init_resource::<renzora::SolariDiagnosticsRequest>()
            .init_resource::<panel::PanelVisible>()
            .init_resource::<panel::RayHistory>()
            .register_shell_panel(panel::PANEL, "Ray Tracing", "lightning", "Debug");
        app.register_panel_content(panel::PANEL, true, panel::build)
            .systems(
                Update,
                (
                    panel::mark_visible,
                    panel::reset_reference_pressed,
                    panel::record_ray_history,
                ),
            )
            // Always: it is what turns measuring off once the panel is hidden.
            .always(Last, panel::follow_visibility);
    }
}

renzora::add!(SolariEditorPlugin, Editor);
