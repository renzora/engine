//! Renzora RT — screen-space global illumination.
//!
//! Single-pass depth+normal-aware SSGI wired between `EndMainPass` and
//! `Tonemapping`. Not the 9-pass beast the old crate was; this is the
//! `ScreenSpace` tier Lumen delegates to. Phases 5+ of the Lumen plan extend
//! this with Hi-Z and denoise.
//!
//! `RtLighting` / `RtDebugMode` / `RtLightingExternallyManaged` live in the
//! shared `renzora` contract (so the GI plugin, presets and inspectors share
//! one `TypeId`). This crate is a *library* linked into the `renzora_lumen` GI
//! distribution plugin, which installs `RtPlugin`; it is not a plugin on its
//! own and is never statically linked into the host.

use bevy::core_pipeline::Core3d;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponentPlugin;
use bevy::render::{Render, RenderApp, RenderSystems};
use renzora::{RtLighting, RtLightingExternallyManaged};

mod node;
mod prepare;

use node::rt_pass;
use prepare::RtPipeline;

#[derive(Default)]
pub struct RtPlugin;

impl Plugin for RtPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "ssgi.wgsl");

        app.register_type::<RtLighting>();
        app.add_systems(Update, (sync_rt_lighting, cleanup_rt_lighting));
        app.add_plugins(ExtractComponentPlugin::<RtLighting>::default());

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .add_systems(
                    Render,
                    prepare::prepare_rt_uniforms.in_set(RenderSystems::PrepareResources),
                )
                // RT-SSGI is global illumination → the shared `RenderPhase::Gi`
                // (HDR, after the main pass, before TAA — the framework anchors it;
                // this crate doesn't reference TAA/tonemapping). It and Lumen are
                // mutually-exclusive GI backends, so sharing the phase is fine.
                .add_systems(Core3d, rt_pass.in_set(renzora::RenderPhase::Gi));
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<RtPipeline>();
        }
    }
}

/// Routing-driven sync. `EffectRouting` maps source entities (where the
/// user authored `RtLighting`) onto target cameras. We mirror the
/// component to the camera so the render world sees it on the view entity.
///
/// Prepass components (`DepthPrepass`, `NormalPrepass`) are *not* touched
/// here — they're attached at camera spawn (`renzora_engine::camera`,
/// `renzora_viewport::play_mode`). Bevy 0.18 specializes the prepass
/// pipeline at first render and can't grow its attachment list later
/// without a wgpu validation crash, so SSGI relies on those being permanent.
fn sync_rt_lighting(
    mut commands: Commands,
    sources: Query<(Entity, Ref<RtLighting>)>,
    externally_managed: Query<(), With<RtLightingExternallyManaged>>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        // Skip cameras whose RtLighting is owned by another plugin
        // (e.g. `renzora_lumen` ScreenSpace tier).
        if externally_managed.contains(*target) {
            continue;
        }
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                // Bevy 0.19: `settings` is `Ref<RtLighting>`; clone the inner
                // component, not the `Ref` (which isn't a Bundle).
                commands.entity(*target).insert((*settings).clone());
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<RtLighting>();
            }
        }
    }
}

fn cleanup_rt_lighting(
    mut commands: Commands,
    mut removed: RemovedComponents<RtLighting>,
    externally_managed: Query<(), With<RtLightingExternallyManaged>>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if externally_managed.contains(*target) {
                continue;
            }
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<RtLighting>();
            }
        }
    }
}
