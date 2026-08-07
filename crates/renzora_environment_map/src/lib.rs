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

use bevy::light::{
    Atmosphere, AtmosphereEnvironmentMapLight, EnvironmentMapLight, GeneratedEnvironmentMapLight,
};
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
    probe_holders: Query<Entity, With<AtmosphereEnvironmentMapLight>>,
    routing: Res<renzora::EffectRouting>,
    quality: Option<Res<renzora::ResolvedGraphicsQuality>>,
) {
    let routing_changed = routing.is_changed();
    // The probe face size MUST match what `renzora_engine::scene_io::rehydrate_cameras`
    // (and the editor camera spawn) attached, or re-inserting this component here
    // re-allocates the cubemap. Both derive it from the same tier. See the field's
    // cost note in `GraphicsQuality::ibl_face_size`.
    let ibl_size = UVec2::splat(quality.as_ref().map(|q| q.0.ibl_face_size()).unwrap_or(128));

    // Address the probe by the component that IS the probe, not by `EffectRouting`
    // target. The holder differs by mode — the primary viewport camera in the
    // editor, the active game camera in a shipped runtime — and routing doesn't
    // always name it (a shipped game has no viewport routing at all, which left the
    // probe unwritten and the runtime IBL stuck at its spawn intensity). So resolve
    // the intensity from the routed `WorldEnvironment` source, then write it onto
    // the probe holder(s) directly. The probe is only ever *updated* here, never
    // added or removed, so the spawn-time bind-group layout stays stable.
    let source = routing
        .iter()
        .flat_map(|(_, srcs)| srcs.iter())
        .find_map(|&src| sources.get(src).ok());

    let (intensity, changed) = match source {
        Some((settings, sun)) => {
            // Re-sync whenever routing, settings, or sun change so the IBL fades
            // smoothly across the horizon.
            let sun_changed = sun.as_ref().map(|s| s.is_changed()).unwrap_or(false);
            let changed = routing_changed || settings.is_changed() || sun_changed;
            // Scale by sun elevation: at night the procedural sky cubemap is dark
            // so IBL is already low, but applying the same horizon fade as the
            // directional light keeps the scene from being "vaguely lit" by
            // residual atmospheric scatter when there's no sun.
            let sun_factor = sun
                .as_ref()
                .map(|s| renzora_lighting::sun_horizon_factor(s.elevation))
                .unwrap_or(1.0);
            let intensity = if settings.enabled {
                settings.intensity * sun_factor
            } else {
                0.0
            };
            (intensity, changed)
        }
        // No source — only push the "off" value when the routing actually changed
        // (e.g. the WE was just removed); otherwise we'd thrash every frame.
        None => (0.0, routing_changed),
    };

    if !changed {
        return;
    }

    for entity in &probe_holders {
        commands
            .entity(entity)
            .insert(AtmosphereEnvironmentMapLight {
                intensity,
                size: ibl_size,
                ..default()
            });
        // The PBR shader reads from `EnvironmentMapLight`, fed by the bake chain
        // (AtmosphereEnvironmentMapLight → GeneratedEnvironmentMapLight →
        // EnvironmentMapLight). Write it directly too so the editor case works,
        // where the camera is spawned long before any WE exists.
        if let Ok(mut env) = env_lights.get_mut(entity) {
            env.intensity = intensity;
        }
    }
}

/// When the source `EnvironmentMapComponentSettings` is removed (entity
/// despawn or component removed via inspector), zero IBL intensity on the probe
/// holder. Without this it would keep its last-applied intensity until something
/// else updated it. Same targeting as [`sync_environment_map`]: whoever carries
/// the probe, not whatever the routing happens to target.
fn cleanup_environment_map(
    mut commands: Commands,
    mut removed: RemovedComponents<EnvironmentMapComponentSettings>,
    mut env_lights: Query<&mut EnvironmentMapLight>,
    probe_holders: Query<Entity, With<AtmosphereEnvironmentMapLight>>,
    quality: Option<Res<renzora::ResolvedGraphicsQuality>>,
) {
    if removed.read().next().is_some() {
        // Keep the face size identical to the spawn/sync value — re-inserting with
        // a different `size` would re-allocate the cubemap.
        let ibl_size = UVec2::splat(quality.as_ref().map(|q| q.0.ibl_face_size()).unwrap_or(128));
        for entity in &probe_holders {
            commands
                .entity(entity)
                .insert(AtmosphereEnvironmentMapLight {
                    intensity: 0.0,
                    size: ibl_size,
                    ..default()
                });
            if let Ok(mut env) = env_lights.get_mut(entity) {
                env.intensity = 0.0;
            }
        }
    }
}

/// Frames to keep re-baking the IBL after the sky last changed. The
/// atmosphere→cubemap→prefilter chain produces a complete filtered map in a
/// frame, so a small window comfortably covers convergence and the load-time
/// routing kick; a static sky then freezes.
const SKY_SETTLE_FRAMES: u32 = 8;

/// Countdown that keeps the per-frame IBL prefilter running for a settle window
/// after the sky last changed, then lets [`gate_environment_generation`] freeze
/// it for a static sky.
#[derive(Resource, Default)]
struct SkyBakeDirty {
    frames: u32,
}

/// Re-arm [`SkyBakeDirty`] whenever anything that determines the baked sky
/// changes — the sun (day/night), the atmosphere params (`Atmosphere`, which
/// `renzora_atmosphere::sync_atmosphere` rewrites only on change), the env-map
/// intensity, or the effect routing (scene/source load, incl. the load-time
/// kick). For a truly static sky none of these fire, the countdown reaches 0,
/// and the dominant per-frame IBL cost — Bevy re-prefiltering the atmosphere
/// cubemap into radiance/irradiance maps EVERY frame (`bevy_pbr::light_probe::generate`,
/// which has no upstream bake-once, see bevyengine/bevy#24517) — stops until the
/// sky next changes.
fn mark_sky_dirty(
    mut dirty: ResMut<SkyBakeDirty>,
    routing: Res<renzora::EffectRouting>,
    changed_sun: Query<(), Changed<renzora_lighting::Sun>>,
    changed_env: Query<(), Changed<EnvironmentMapComponentSettings>>,
    changed_atmosphere: Query<(), Changed<Atmosphere>>,
) {
    let sky_changed = routing.is_changed()
        || !changed_sun.is_empty()
        || !changed_env.is_empty()
        || !changed_atmosphere.is_empty();
    if sky_changed {
        dirty.frames = SKY_SETTLE_FRAMES;
    } else {
        dirty.frames = dirty.frames.saturating_sub(1);
    }
}

/// Holds a removed [`GeneratedEnvironmentMapLight`] while the environment is
/// inactive, so it can be restored verbatim when IBL switches back on.
#[derive(Component)]
struct DormantGeneratedEnvMap(GeneratedEnvironmentMapLight);

/// Stop the per-frame environment-map (IBL) filtering when it isn't earning its
/// cost — when no environment is active **OR the sky is static** — and resume it
/// while the environment is active and the sky is changing.
///
/// Bevy re-filters the atmosphere cubemap into radiance + irradiance maps EVERY
/// frame for any camera carrying a `GeneratedEnvironmentMapLight`, with no
/// bake-once / dirty mode (`bevy_pbr::light_probe::generate`; bevyengine/bevy#24517).
/// That's the dominant per-frame IBL cost, and for a static sky it recomputes an
/// identical map forever. Two independent triggers pause it:
/// - **environment inactive** (`intensity ~ 0`): scaling the lit result to zero
///   doesn't gate the generation, so we do — stash + remove the generator.
/// - **sky static** ([`SkyBakeDirty`]`.frames == 0`, i.e. nothing that determines
///   the baked sky has changed for `SKY_SETTLE_FRAMES` — see [`mark_sky_dirty`]):
///   the last-baked map is already correct, so freeze it. This is the win for a
///   normal scene at full intensity, where the old intensity-only gate never fired.
///
/// Restore (re-bake) as soon as the environment is active AND the sky is settling
/// again after a change.
///
/// Safe w.r.t. the bind-group-layout lock that forces the probe to exist from
/// spawn: the view's IBL *binding* comes from `EnvironmentMapLight` (left
/// untouched, so the layout never changes) — `GeneratedEnvironmentMapLight` only
/// drives the filtering that writes into it. While dormant the filtered maps just
/// freeze at their last-baked (correct-for-a-static-sky) state.
fn gate_environment_generation(
    mut commands: Commands,
    dirty: Res<SkyBakeDirty>,
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
    // Generate only while the environment is on AND the sky is still settling
    // after a change; an off or static sky freezes the prefilter.
    let settling = dirty.frames > 0;

    // Active → dormant: pause generation when off or static.
    for (entity, probe, generated) in &active {
        if probe.intensity <= ACTIVE_EPS || !settling {
            commands
                .entity(entity)
                .insert(DormantGeneratedEnvMap(generated.clone()))
                .remove::<GeneratedEnvironmentMapLight>();
        }
    }

    // Dormant → active: resume generation with the current intensity.
    for (entity, probe, stash) in &dormant {
        if probe.intensity > ACTIVE_EPS && settling {
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
        app.init_resource::<SkyBakeDirty>();
        // `gate_environment_generation` runs after `sync_environment_map` so it
        // sees the intensity that was just resolved this frame, and after
        // `mark_sky_dirty` so it sees this frame's sky-dirty countdown.
        app.add_systems(
            Update,
            (
                // Runs first so the same frame's `sync_environment_map` sees the
                // forced `routing` change and re-applies intensity once the bake
                // is ready (fixes "scene loads dark until the sun/env is nudged").
                kick_routing_on_environment_load,
                sync_environment_map,
                cleanup_environment_map,
                mark_sky_dirty,
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
