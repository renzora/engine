//! Renzora RT — screen-space global illumination.
//!
//! Single-pass depth+normal-aware SSGI wired between `EndMainPass` and
//! `Tonemapping`. Not the 9-pass beast the old crate was; this is the
//! `ScreenSpace` tier `renzora_lumen` delegates to. Phases 5+ of the
//! Lumen plan extend this with Hi-Z and denoise.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_graph::{RenderGraphExt, RenderLabel, ViewNodeRunner};
use bevy::render::{Render, RenderApp, RenderSystems};
use serde::{Deserialize, Serialize};

mod node;
mod prepare;

use node::RtNode;
use prepare::RtPipeline;

/// Output mode for the SSGI pass. Drives a uniform that the shader
/// branches on at composite time. Reusable for future debug views;
/// new variants append at the end so existing serialized values stay valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum RtDebugMode {
    /// scene + indirect — normal output.
    #[default]
    Composite,
    /// Indirect contribution only — no scene. Useful for tuning intensity
    /// and seeing where bounce light is / isn't.
    IndirectOnly,
}

impl RtDebugMode {
    pub fn as_u32(self) -> u32 {
        match self {
            RtDebugMode::Composite => 0,
            RtDebugMode::IndirectOnly => 1,
        }
    }
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RtLighting {
    pub enabled: bool,
    pub intensity: f32,
    pub debug: RtDebugMode,
}

/// Marker placed on a target camera to tell `sync_rt_lighting` and
/// `cleanup_rt_lighting` to leave its `RtLighting` alone.
///
/// Used by `renzora_lumen` when its `ScreenSpace` tier owns the camera —
/// without this, RT's routing-based sync would see no `RtLighting` on the
/// authored source entity and clobber what Lumen just inserted on the
/// target. Insert alongside `RtLighting`; remove together when releasing
/// control.
#[derive(Component, Clone, Debug, Default)]
pub struct RtLightingExternallyManaged;

impl Default for RtLighting {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
            debug: RtDebugMode::Composite,
        }
    }
}

impl ExtractComponent for RtLighting {
    type QueryData = &'static RtLighting;
    type QueryFilter = ();
    type Out = RtLighting;

    fn extract_component(
        item: bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(item.clone())
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct RtLabel;

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
                .add_render_graph_node::<ViewNodeRunner<RtNode>>(Core3d, RtLabel)
                .add_render_graph_edges(
                    Core3d,
                    (Node3d::EndMainPass, RtLabel, Node3d::Tonemapping),
                );
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
                commands.entity(*target).insert(settings.clone());
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

renzora::add!(RtPlugin);
