//! Runtime graphics-quality enforcement — the shipped-game counterpart to the
//! editor's `renzora_level_presets::graphics_quality`.
//!
//! ## Why this exists
//!
//! The editor's tier enforcement is `Editor`-scoped and only touches
//! `ViewportCamera`s, so a **shipped game had no quality tier at all** — it
//! always ran the full fullscreen-pass stack (SSGI + SSAO + raymarched
//! atmosphere + a 512² per-frame IBL bake + bloom + TAA + auto-exposure),
//! whatever the machine. On a weak / integrated GPU (or a high-DPI display,
//! where the pixel count is ~2–4×) that stack is scene-independent and drops the
//! game to single-digit FPS — see the module doc on
//! `renzora::core::viewport_types::GraphicsQuality`.
//!
//! This module resolves [`RenderingConfig::graphics_quality`](renzora::RenderingConfig)
//! into [`ResolvedGraphicsQuality`] and forces the tier onto the **active play
//! camera** every frame, exactly as the editor forces it onto its viewport
//! cameras. Both paths write the same resource, so downstream renderer crates
//! (clouds, environment-map IBL) read one source of truth.
//!
//! ## Crash-safety
//!
//! Every mutation here is one a router already performs dynamically, so none of
//! it grows a camera's bind-group layout after first render (the spawn-locked
//! atmosphere/prepass bundle is left resident at every tier):
//! - GI: flip `RtLighting.enabled` / `LumenLighting.quality` (same switch the
//!   Render-Toggles panel uses).
//! - Bloom / TAA / auto-exposure / SSAO: remove the routed component (each
//!   router re-adds it from the untouched scene source when routing changes).
//! - Atmosphere: assign `AtmosphereSettings.rendering_method`, which
//!   `renzora_atmosphere::sync_atmosphere` already reassigns at runtime.
//!
//! Because the runtime tier is fixed for a session (it comes from the packed
//! project config, which never changes mid-run), the "restore on a tier raise"
//! path is only exercised if a script mutates the resource — handled the same
//! way the editor does it, by bumping `EffectRouting` on any change so the
//! routers re-apply from source.

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::light::DirectionalLightShadowMap;
use bevy::pbr::{AtmosphereMode, AtmosphereSettings, ScreenSpaceAmbientOcclusion};
use bevy::post_process::auto_exposure::AutoExposure;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use renzora::core::viewport_types::GraphicsQuality;
use renzora::{
    CurrentProject, EffectRouting, LumenLighting, LumenQuality, ResolvedGraphicsQuality, RtLighting,
};

/// Seed [`ResolvedGraphicsQuality`] from the loaded project's `[rendering]`
/// tier. Idempotent and cheap — assigns only when the value actually differs,
/// so it can sit in `Update` and pick the tier up as soon as the project loads.
pub fn sync_runtime_graphics_quality(
    project: Option<Res<CurrentProject>>,
    mut resolved: ResMut<ResolvedGraphicsQuality>,
) {
    let Some(project) = project else {
        return;
    };
    let q = project.config.rendering.graphics_quality;
    if resolved.0 != q {
        resolved.0 = q;
        info!("[runtime] graphics quality tier: {:?}", q);
    }
}

/// Force the resolved tier onto the active play camera. Mirror of
/// `renzora_level_presets::graphics_quality::enforce_graphics_quality`, minus the
/// `ViewportCamera` filter (a game has none — the scene camera renders directly),
/// plus the SSAO / atmosphere / (via the resource) clouds+IBL knobs the editor
/// version predates.
///
/// Runs in `PostUpdate` so it has the last word over the Update-stage effect
/// routers.
#[allow(clippy::too_many_arguments)]
pub fn enforce_runtime_graphics_quality(
    quality: Res<ResolvedGraphicsQuality>,
    mut last: Local<Option<GraphicsQuality>>,
    routing: Option<ResMut<EffectRouting>>,
    mut commands: Commands,
    mut gi_rt: Query<&mut RtLighting, With<Camera3d>>,
    mut gi_lumen: Query<&mut LumenLighting, With<Camera3d>>,
    bloom_cams: Query<Entity, (With<Camera3d>, With<Bloom>)>,
    taa_cams: Query<Entity, (With<Camera3d>, With<TemporalAntiAliasing>)>,
    ae_cams: Query<Entity, (With<Camera3d>, With<AutoExposure>)>,
    ssao_cams: Query<Entity, (With<Camera3d>, With<ScreenSpaceAmbientOcclusion>)>,
    mut atmo: Query<&mut AtmosphereSettings, With<Camera3d>>,
    shadow_map: Option<ResMut<DirectionalLightShadowMap>>,
) {
    let q = quality.0;

    // On a tier change, nudge the routers so an effect a lower tier had stripped
    // is re-applied from its (untouched) scene source; the per-frame force below
    // then re-strips whatever the new tier still forbids.
    if *last != Some(q) {
        if let Some(mut routing) = routing {
            routing.set_changed();
        }
        *last = Some(q);
    }

    // Screen-space GI (the heaviest, most pixel-bound pass). Reads go through
    // Deref; only the assignment hits DerefMut, so we don't re-flag every frame.
    if !q.gi() {
        for mut r in &mut gi_rt {
            if r.enabled {
                r.enabled = false;
            }
        }
        for mut l in &mut gi_lumen {
            if l.quality != LumenQuality::Off {
                l.quality = LumenQuality::Off;
            }
        }
    }

    // Post-process passes — remove the routed component. The `With<…>` filters
    // mean each query only yields cameras that still carry it, so once removed
    // they stop matching and we don't churn.
    if !q.bloom() {
        for e in &bloom_cams {
            commands.entity(e).remove::<Bloom>();
        }
    }
    if !q.taa() {
        for e in &taa_cams {
            commands.entity(e).remove::<TemporalAntiAliasing>();
        }
    }
    if !q.auto_exposure() {
        for e in &ae_cams {
            commands.entity(e).remove::<AutoExposure>();
        }
    }
    if !q.ssao() {
        for e in &ssao_cams {
            commands.entity(e).remove::<ScreenSpaceAmbientOcclusion>();
        }
    }

    // Atmosphere: drop the raymarched sky to the ~40× cheaper LookupTexture path
    // below `High`. `AtmosphereMode` isn't `PartialEq`, so gate the assignment on
    // a `matches!` read (Deref, no change flag) to avoid re-marking every frame.
    if !q.atmosphere_raymarched() {
        for mut s in &mut atmo {
            if matches!(s.rendering_method, AtmosphereMode::Raymarched) {
                s.rendering_method = AtmosphereMode::LookupTexture;
            }
        }
    }

    // Directional shadow-map resolution — a *global* resource (shared by every
    // cascade of every directional light), so set it here rather than per-camera.
    // Read per-frame by Bevy's shadow prep, so a runtime change takes effect next
    // frame; gated on difference so we don't re-flag it every frame.
    if let Some(mut sm) = shadow_map {
        let target = q.shadow_map_size();
        if sm.size != target {
            sm.size = target;
        }
    }
}
