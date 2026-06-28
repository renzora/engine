//! Environment map (IBL) component.
//!
//! Image-based lighting drives PBR reflections + ambient diffuse from a
//! cubemap. Right now we use Bevy's atmosphere-derived cubemap
//! (`AtmosphereEnvironmentMapLight`) — the procedural sky gets baked into
//! a cubemap each frame and fed back into the PBR pipeline.
//!
//! Architecturally separate from the atmosphere component because the
//! choice of "should reflections happen" is independent of "should the
//! sky render with scattering." A future HDR-cubemap variant can extend
//! the same component (see `EnvironmentMapKind` placeholder for where
//! that would live).
//!
//! ## Bevy 0.18 caveat
//!
//! Bevy locks the camera's bind group layout the first frame it renders,
//! with IBL slots present iff `AtmosphereEnvironmentMapLight` existed at
//! that moment. Adding/removing it later crashes wgpu. The camera spawn
//! site (in `renzora_engine`) attaches the component at low intensity so
//! the layout is stable; this plugin updates `intensity` in-place via
//! `EffectRouting`. `enabled = false` collapses intensity to 0 — visually
//! "off" without touching the bindings.

use bevy::light::{AtmosphereEnvironmentMapLight, EnvironmentMapLight, GeneratedEnvironmentMapLight};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

mod probe;

/// User-authored settings for sky-driven image-based lighting. Attach to
/// any non-camera entity (typically a "World Environment") and the plugin
/// routes its values onto every active camera via `EffectRouting`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EnvironmentMapComponentSettings {
    /// IBL contribution strength. 1.0 = sky-bright reflections + ambient
    /// (often too strong, washes out direct sun shadows). 0.3 is a good
    /// "modern engine default" — visible reflections, contrast preserved.
    pub intensity: f32,
    pub enabled: bool,
}

impl Default for EnvironmentMapComponentSettings {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            enabled: true,
        }
    }
}

/// Re-fire the whole `EffectRouting` chain for a short window after a
/// `WorldEnvironment` **source** appears, so the IBL + atmosphere apply on
/// scene/project load without the user nudging the sun or env value.
///
/// Why a *window*, not a one-shot: on project load the pieces settle over several
/// frames — the `WorldEnvironment` spawns, then `EffectRouting` rebuilds to
/// include it, then the bake camera's `GeneratedEnvironmentMapLight` (the
/// atmosphere → cubemap bake) appears. The `routing`/`settings`/`sun` `is_changed`
/// flags that gate [`sync_environment_map`] and `renzora_atmosphere::sync_atmosphere`
/// each lapse after a single frame, so a one-frame kick lands before the bake is
/// ready and is missed → the scene loads dark until something is nudged. We arm a
/// countdown when a source `Added`s and `set_changed()` the routing every frame of
/// that window — covering the settle period.
///
/// Triggers, both needed:
/// - a SOURCE appearing (`EnvironmentMapComponentSettings`, on the World
///   Environment entity) — covers switching projects, where the editor bake
///   camera persists and only the source is new.
/// - the bake (`GeneratedEnvironmentMapLight`) appearing on a NON-play camera —
///   the editor bake camera getting its bake is the "everything's ready" moment
///   on a cold load, and it's the kick that actually relit the scene.
///
/// The bake filter `Without<PlayModeCamera>` is load-bearing: the bake is `Added`
/// to the *play camera* every time play starts, so triggering on it unfiltered
/// armed a `set_changed()` burst on every play toggle — forcing SSR/SSAO/etc. to
/// re-specialize while the play camera's pipeline was being rebuilt, which
/// invalidated render buffers ("thickness_buffer is invalid"). Excluding the play
/// camera means the kick only fires for real scene/project loads.
fn kick_routing_on_environment_load(
    added_bake: Query<
        (),
        (
            Added<GeneratedEnvironmentMapLight>,
            Without<renzora::core::PlayModeCamera>,
        ),
    >,
    added_source: Query<(), Added<EnvironmentMapComponentSettings>>,
    mut routing: ResMut<renzora::EffectRouting>,
    mut frames_left: Local<u32>,
) {
    // ~10 frames comfortably covers WorldEnvironment spawn → routing rebuild →
    // bake-camera `GeneratedEnvironmentMapLight` appearing on load.
    const KICK_FRAMES: u32 = 10;
    if !added_bake.is_empty() || !added_source.is_empty() {
        *frames_left = KICK_FRAMES;
    }
    if *frames_left > 0 {
        *frames_left -= 1;
        routing.set_changed();
    }
}

fn sync_environment_map(
    mut commands: Commands,
    sources: Query<(
        Ref<EnvironmentMapComponentSettings>,
        Option<Ref<renzora_lighting::Sun>>,
    )>,
    mut env_lights: Query<&mut EnvironmentMapLight>,
    probes: Query<(), With<AtmosphereEnvironmentMapLight>>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        // The IBL probe can't be added at runtime (Bevy specializes the layout
        // at first render). Only *update* cameras that already carry it — the
        // single environment/bake camera. Other routed cameras share its result
        // (the baked cubemap is fanned out as a `Skybox`).
        if probes.get(*target).is_err() {
            continue;
        }
        // Find a source on the routing list that has the settings, and
        // (optionally) a Sun on the same entity for day-night fading.
        let source = source_list.iter().find_map(|&src| sources.get(src).ok());

        match source {
            Some((settings, sun)) => {
                // Re-sync whenever routing, settings, or sun change so
                // the IBL fades smoothly across the horizon.
                let sun_changed = sun.as_ref().map(|s| s.is_changed()).unwrap_or(false);
                if !routing_changed && !settings.is_changed() && !sun_changed {
                    continue;
                }
                // Scale by sun elevation: at night the procedural sky
                // cubemap is dark so IBL is already low, but applying
                // the same horizon fade as the directional light keeps
                // the scene from being "vaguely lit" by residual
                // atmospheric scatter when there's no sun.
                let sun_factor = sun
                    .as_ref()
                    .map(|s| renzora_lighting::sun_horizon_factor(s.elevation))
                    .unwrap_or(1.0);
                let intensity = if settings.enabled {
                    settings.intensity * sun_factor
                } else {
                    0.0
                };
                // Replace the existing component in place — the camera
                // spawn site attached it up front so the bind group
                // layout stays stable across enables/disables.
                commands
                    .entity(*target)
                    .insert(AtmosphereEnvironmentMapLight {
                        intensity,
                        ..default()
                    });
                // The PBR shader reads from `EnvironmentMapLight`, fed by the bake
                // chain (AtmosphereEnvironmentMapLight → GeneratedEnvironmentMapLight
                // → EnvironmentMapLight). Write it directly too so the editor case
                // works, where the camera is spawned long before any WE exists.
                if let Ok(mut env) = env_lights.get_mut(*target) {
                    env.intensity = intensity;
                }
            }
            None => {
                // No source for this target — only push the "off" value
                // when the routing actually changed (e.g. the WE was just
                // removed). Otherwise we'd thrash the camera every frame.
                if routing_changed {
                    commands
                        .entity(*target)
                        .insert(AtmosphereEnvironmentMapLight {
                            intensity: 0.0,
                            ..default()
                        });
                    if let Ok(mut env) = env_lights.get_mut(*target) {
                        env.intensity = 0.0;
                    }
                }
            }
        }
    }
}

/// When the source `EnvironmentMapComponentSettings` is removed (entity
/// despawn or component removed via inspector), zero IBL intensity on
/// every camera the routing currently targets. Without this the camera
/// would keep its last-applied intensity until something else updated it.
fn cleanup_environment_map(
    mut commands: Commands,
    mut removed: RemovedComponents<EnvironmentMapComponentSettings>,
    mut env_lights: Query<&mut EnvironmentMapLight>,
    probes: Query<(), With<AtmosphereEnvironmentMapLight>>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            // Only the bake camera carries the probe; never add it at runtime.
            if probes.get(*target).is_err() {
                continue;
            }
            commands
                .entity(*target)
                .insert(AtmosphereEnvironmentMapLight {
                    intensity: 0.0,
                    ..default()
                });
            if let Ok(mut env) = env_lights.get_mut(*target) {
                env.intensity = 0.0;
            }
        }
    }
}

/// Holds a removed [`GeneratedEnvironmentMapLight`] while the environment is
/// inactive, so it can be restored verbatim when IBL switches back on.
#[derive(Component)]
struct DormantGeneratedEnvMap(GeneratedEnvironmentMapLight);

/// Stop the per-frame environment-map (IBL) filtering when no environment is
/// active, and resume it when one is.
///
/// Bevy 0.18 re-filters the atmosphere cubemap into radiance + irradiance maps
/// EVERY frame for any camera carrying a `GeneratedEnvironmentMapLight`, with no
/// bake-once / dirty mode (`bevy_pbr::light_probe::generate`). On a scene with no
/// active `WorldEnvironment` that's pure waste — the `lightprobe_*` passes can be
/// the majority of editor GPU time even though `intensity` is 0 (intensity only
/// scales the lit result, it doesn't gate the generation).
///
/// We use `AtmosphereEnvironmentMapLight.intensity` (kept in sync by
/// [`sync_environment_map`]) as the "is the environment active" signal:
/// - **inactive** (`intensity ~ 0`): stash and remove `GeneratedEnvironmentMapLight`.
///   The generate node then has nothing to do and the `lightprobe_*` passes stop.
/// - **active** (`intensity > 0`): restore the stashed generator (with the live
///   intensity) so IBL regenerates again.
///
/// This is safe w.r.t. the bind-group-layout lock that forces the probe to exist
/// from spawn: the view's IBL *binding* comes from `EnvironmentMapLight` (left
/// untouched, so the layout never changes) — `GeneratedEnvironmentMapLight` only
/// drives the filtering that writes into it. While dormant the filtered maps just
/// freeze (and at intensity 0 they're invisible anyway). `prepare_atmosphere_probe_components`
/// won't re-add it because the camera keeps its `AtmosphereEnvironmentMap`.
fn gate_environment_generation(
    mut commands: Commands,
    active: Query<
        (Entity, &AtmosphereEnvironmentMapLight, &GeneratedEnvironmentMapLight),
        Without<DormantGeneratedEnvMap>,
    >,
    dormant: Query<
        (Entity, &AtmosphereEnvironmentMapLight, &DormantGeneratedEnvMap),
        Without<GeneratedEnvironmentMapLight>,
    >,
) {
    const ACTIVE_EPS: f32 = 1e-4;

    // Active → inactive: pause generation.
    for (entity, probe, generated) in &active {
        if probe.intensity <= ACTIVE_EPS {
            commands
                .entity(entity)
                .insert(DormantGeneratedEnvMap(generated.clone()))
                .remove::<GeneratedEnvironmentMapLight>();
        }
    }

    // Inactive → active: resume generation with the current intensity.
    for (entity, probe, stash) in &dormant {
        if probe.intensity > ACTIVE_EPS {
            let mut generated = stash.0.clone();
            generated.intensity = probe.intensity;
            commands
                .entity(entity)
                .insert(generated)
                .remove::<DormantGeneratedEnvMap>();
        }
    }
}

#[derive(Default)]
pub struct EnvironmentMapPlugin;

impl Plugin for EnvironmentMapPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] EnvironmentMapPlugin");
        app.register_type::<EnvironmentMapComponentSettings>();
        // `gate_environment_generation` runs after `sync_environment_map` so it
        // sees the intensity that was just resolved this frame.
        app.add_systems(
            Update,
            (
                // Runs first so the same frame's `sync_environment_map` sees the
                // forced `routing` change and re-applies intensity once the bake
                // is ready (fixes "scene loads dark until the sun/env is nudged").
                kick_routing_on_environment_load,
                sync_environment_map,
                cleanup_environment_map,
                gate_environment_generation,
            )
                .chain(),
        );
        // Reflection probes: resolve each probe's authored source path into the
        // POT cubemap its `GeneratedEnvironmentMapLight` needs (runs in the
        // editor and the shipped game).
        app.add_systems(Update, probe::apply_reflection_probe_source);
    }
}

renzora::add!(EnvironmentMapPlugin);
