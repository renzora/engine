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

mod filter_gate;
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
    mut probe_holders: Query<ProbeHolder>,
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

    for holder in &mut probe_holders {
        set_probe_intensity(&mut commands, holder, intensity, ibl_size);
    }
}

/// A camera carrying the atmosphere probe, with the two components Bevy derives
/// from it.
type ProbeHolder = (
    Entity,
    &'static mut AtmosphereEnvironmentMapLight,
    Option<&'static mut GeneratedEnvironmentMapLight>,
    Option<&'static mut EnvironmentMapLight>,
);

/// Write `intensity` onto a probe holder without re-inserting the probe.
///
/// Bevy 0.20 treats inserting `AtmosphereEnvironmentMapLight` on an entity that
/// already has one as a rebuild: its observer strips the derived
/// `GeneratedEnvironmentMapLight` and `EnvironmentMapLight` and starts again
/// from a black cubemap. Re-inserting here on every routing change also re-added
/// `GeneratedEnvironmentMapLight`, which `kick_routing_on_environment_load` reads
/// as a fresh probe and answers by changing the routing again, so the sky
/// lighting was rebuilt from black on every frame: about 5 ms of CPU, new GPU
/// images each frame, and black patches in the environment.
///
/// So the intensity is written in place on all three components (each copies it
/// only when created), and the probe is re-inserted only when its cubemap size
/// really changes, which does need new textures.
fn set_probe_intensity(
    commands: &mut Commands,
    (entity, mut probe, generated, derived): (
        Entity,
        Mut<AtmosphereEnvironmentMapLight>,
        Option<Mut<GeneratedEnvironmentMapLight>>,
        Option<Mut<EnvironmentMapLight>>,
    ),
    intensity: f32,
    size: UVec2,
) {
    if probe.size != size {
        commands
            .entity(entity)
            .insert(AtmosphereEnvironmentMapLight {
                intensity,
                size,
                ..default()
            });
        return;
    }
    if probe.intensity != intensity {
        probe.intensity = intensity;
    }
    if let Some(mut generated) = generated {
        if generated.intensity != intensity {
            generated.intensity = intensity;
        }
    }
    // The PBR shader reads from `EnvironmentMapLight`. Written directly too so
    // the editor case works, where the camera is spawned long before any World
    // Environment exists.
    if let Some(mut derived) = derived {
        if derived.intensity != intensity {
            derived.intensity = intensity;
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
    mut probe_holders: Query<ProbeHolder>,
    quality: Option<Res<renzora::ResolvedGraphicsQuality>>,
) {
    if removed.read().next().is_some() {
        // Keep the face size identical to the spawn/sync value; a different
        // `size` would re-allocate the cubemap.
        let ibl_size = UVec2::splat(quality.as_ref().map(|q| q.0.ibl_face_size()).unwrap_or(128));
        for holder in &mut probe_holders {
            set_probe_intensity(&mut commands, holder, 0.0, ibl_size);
        }
    }
}

/// Frames to keep re-baking the IBL after the sky last changed. The
/// atmosphere→cubemap→prefilter chain produces a complete filtered map in a
/// frame, so a small window comfortably covers convergence and the load-time
/// routing kick; a static sky then freezes.
const SKY_SETTLE_FRAMES: u32 = 8;

/// Countdown that keeps the sky probe's IBL prefilter running for a settle
/// window after the sky last changed, then lets the filter gate
/// ([`filter_gate`]) freeze it for a static sky.
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
    mut last_reasons: Local<Vec<&'static str>>,
) {
    let mut reasons = Vec::new();
    if routing.is_changed() {
        reasons.push("effect routing");
    }
    if !changed_sun.is_empty() {
        reasons.push("sun");
    }
    if !changed_env.is_empty() {
        reasons.push("environment settings");
    }
    if !changed_atmosphere.is_empty() {
        reasons.push("atmosphere");
    }
    // A sky that is "changing" on every frame never freezes, and costs the sky
    // probe's full re-filter each frame. Name the signal whenever the set of
    // signals changes, so a writer that touches one of these every frame is
    // visible in the log instead of only in a profile.
    if *last_reasons != reasons {
        info!("[env-map] sky re-bake triggered by: {:?}", reasons);
        *last_reasons = reasons.clone();
    }
    let sky_changed = !reasons.is_empty();
    if sky_changed {
        dirty.frames = SKY_SETTLE_FRAMES;
    } else {
        dirty.frames = dirty.frames.saturating_sub(1);
    }
}

#[derive(Default)]
pub struct EnvironmentMapPlugin;

impl Plugin for EnvironmentMapPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] EnvironmentMapPlugin");
        app.register_type::<EnvironmentMapComponentSettings>();
        app.init_resource::<SkyBakeDirty>();
        app.init_resource::<filter_gate::EnvMapFilterGate>();
        filter_gate::register_render(app);
        // `update_env_map_filter_gate` runs after `sync_environment_map` so it
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
                filter_gate::update_env_map_filter_gate,
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
