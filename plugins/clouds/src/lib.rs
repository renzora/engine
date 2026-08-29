//! Renzora Clouds — volumetric clouds raymarched through a spherical deck.
//!
//! The model is the Horizon Zero Dawn one (Schneider/Vos) with Frostbite's
//! scattering integration (Hillaire): a tileable Perlin-Worley base map defines
//! the silhouette, a 3D Worley volume erodes it, a height profile turns a map
//! with no vertical detail into flat-bottomed billowing cumulus, and a second
//! march toward the sun gives every sample its own shadow.
//!
//! Three pieces:
//!
//! * [`noise`] bakes the two noise fields once, on the GPU.
//! * [`sky`] reads the scene's atmosphere so the deck relights with it.
//! * [`material`] carries them plus one uniform block to the shader.
//! * `sync_clouds` here keeps a camera-centred dome alive and packs the uniform.
//!
//! The dome is *only* a way to get one fragment per sky pixel; `clouds.wgsl`
//! marches from the camera against real shell geometry and never touches the
//! dome surface. That is what lets the deck curve down to the horizon by itself
//! and what lets a camera climb up through it.

use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::Atmosphere;
use bevy::prelude::*;

pub mod inspector;
pub mod material;
pub mod noise;
pub mod sky;

pub use material::{CloudMaterial, CloudsUniform};
pub use noise::CloudNoiseTextures;
pub use sky::SkyTransfer;

// ============================================================================
// Data
// ============================================================================

/// The authored settings now live in the **contract crate**, not here.
///
/// `renzora_level_presets` builds a sky by inserting `CloudsData`, and it is
/// compiled into the editor binary while this renderer is a plugin loaded at
/// runtime — a binary cannot name a type that lives in a plugin. Moving it to
/// `renzora` gives both sides one definition and therefore one `TypeId`; two
/// definitions would mean the preset inserts a component this plugin's queries
/// never match, and the sky silently never appears.
///
/// Re-exported so the rest of this crate (and anything that used
/// `renzora_clouds::CloudsData`) keeps its old path.
pub use renzora::CloudsData;

// ============================================================================
// Tuning constants
// ============================================================================

/// Extinction per km at `density == 1`. Chosen so the default `0.5` lands on the
/// reference model's 0.03 per metre.
const MAX_EXTINCTION_PER_KM: f32 = 60.0;

/// Transmittance below which a ray is called opaque and stops. The reference
/// uses 0.1, which leaves a tenth of the sky bleeding through the thickest
/// clouds; that is only invisible there because it composites against its own
/// sky texture rather than blending over a real one.
const MIN_TRANSMITTANCE: f32 = 0.02;

/// Sun elevation, in degrees, over which the deck fades out into night.
///
/// The window ends *at* the horizon: the deck is at full strength by
/// [`DAY_ELEVATION`], thins steadily as the sun drops through the last few
/// degrees, and is gone once the sun reaches 0. Sunset is the cue.
///
/// This deliberately reverses an earlier choice to sit the whole window below
/// the horizon (-12°..-2°), which kept clouds solid all through golden hour.
/// That looked better in isolation but read as broken in context: the sky and
/// the stars are driven by the atmosphere, which goes to night at 0, so a deck
/// still lit at -1° hung as a bank of bright white cloud over a starfield.
/// Matching the horizon is what keeps the deck and the sky telling the same
/// story. Widen [`DAY_ELEVATION`] for a slower fade.
const NIGHT_ELEVATION: f32 = 0.0;
const DAY_ELEVATION: f32 = 8.0;


/// Step caps below `High`. Both marches are per-pixel, so the tier cap is the
/// difference between clouds costing a slice of the frame and costing the frame.
const MEDIUM_VIEW_STEPS: u32 = 24;
const MEDIUM_SHADOW_STEPS: u32 = 4;

/// The directional-light illuminance the authored colours are calibrated
/// against, in lux. Cloud radiance is scaled by the scene sun's illuminance over
/// this, so brightening or dimming the sun carries the deck with it instead of
/// leaving it lit by a number of its own.
const REFERENCE_ILLUMINANCE: f32 = 40_000.0;

/// Earth's radii, used when a scene has no `Atmosphere` of its own to measure.
/// Match bevy's own defaults.
const FALLBACK_INNER_RADIUS: f32 = 6_360_000.0;
const FALLBACK_OUTER_RADIUS: f32 = 6_460_000.0;

/// How far the warp field runs relative to the base silhouette. **Must match
/// `WARP_SPREAD` in `clouds.wgsl`** — it is only here so the scroll can be
/// wrapped at the field's own period.
const WARP_SPREAD: f32 = 3.0;

/// Turns of the detail volume per second, per metre-per-second of morph speed.
/// The default 50 m/s morph therefore rolls the erosion right through once every
/// 50 seconds, which reads as evolving rather than boiling.
const DETAIL_MORPH_RATE: f32 = 0.0004;

/// Degrees the morph scroll is offset from the wind. Sharing the wind's bearing
/// would make the deformation read as more translation in the same direction;
/// crossing it is what makes shapes look like they are changing rather than
/// moving.
const MORPH_BEARING_OFFSET: f32 = 55.0;

/// Fallback dome radius when no camera reports a usable far plane.
const DEFAULT_DOME_RADIUS: f32 = 4000.0;

// ============================================================================
// Marker & State
// ============================================================================

#[derive(Component)]
struct CloudDomeMarker;

/// Change-only diagnostics for the cloud dome's lifecycle.
///
/// The three explanations for a flickering deck each leave a different trace,
/// and they are hard to tell apart by eye:
///
/// * **A respawn loop** — something despawns the dome and `sync_clouds` builds a
///   new one next frame. Shows up as repeated `dome spawned` / `dome gone`.
/// * **The wrong source or camera** — two `CloudsData` or two active `Camera3d`,
///   picked from an unordered query, alternating frame to frame. Shows up as
///   `source ->` / `camera ->` lines repeating between the same two ids.
/// * **Re-centre lag** — the dome only follows the camera once it has moved more
///   than a unit, so a continuously moving player camera leaves the deck
///   trailing and snapping. Shows up as a high `recentres/s` with a large
///   `max jump`, and is the one that would be invisible in an editor whose
///   camera mostly sits still.
///
/// None of it fires on a steady scene, so this is safe to leave in while the
/// question is open.
#[derive(Default)]
struct Diag {
    source: Option<Option<Entity>>,
    camera: Option<Option<Entity>>,
    allowed: Option<bool>,
    spawned: bool,
    recentres: u32,
    max_jump: f32,
    next_report: f32,
}

impl Diag {
    /// Log `what -> value` the first time it is seen and on every change after.
    fn track<T: PartialEq + std::fmt::Debug>(slot: &mut Option<T>, what: &str, now: T) {
        if slot.as_ref() != Some(&now) {
            info!("[clouds] {what} -> {now:?}");
            *slot = Some(now);
        }
    }
}

#[derive(Resource, Default)]
struct CloudsState {
    entity: Option<Entity>,
    material_handle: Option<Handle<CloudMaterial>>,
    mesh_handle: Option<Handle<Mesh>>,
    /// Last camera position the dome was re-centred on. The dome is a sphere
    /// centred on the camera, so it only needs re-centring when the camera
    /// actually moves — re-inserting `Transform` every frame otherwise re-marks
    /// it changed and forces transform propagation + a mesh re-extract for
    /// nothing.
    last_cam_pos: Option<Vec3>,
    last_radius: f32,
    /// Diagnostics for the "flickers in an exported game, fine in the editor"
    /// report. Everything here logs on **change** only, plus one throttled
    /// summary a second — a per-frame log line in this system would itself
    /// stall frames and change what is being measured.
    diag: Diag,
    /// Accumulated wind displacement in km, wrapped to the noise period.
    wind_offset: Vec3,
    /// Accumulated scroll of the warp field in km, wrapped to its own period.
    morph_offset: Vec3,
    /// Phase through the detail volume, in whole turns.
    detail_phase: f32,
    /// Cached noon reference for the atmosphere coupling, with the key it was
    /// built from: `(medium, inner radius, outer radius, deck mid-altitude)`.
    /// `None` for the medium means the scene had no `Atmosphere`. The `bool` is
    /// whether the fallback medium ended up being the one measured.
    ///
    /// Only the medium *asset* is keyed, not its contents — nothing in the
    /// engine mutates a `ScatteringMedium` in place, and `renzora_atmosphere`
    /// swaps between two persistent handles to turn the sky on and off, which
    /// this does catch.
    #[allow(clippy::type_complexity)]
    sky_reference: Option<(
        (Option<AssetId<ScatteringMedium>>, f32, f32, f32),
        bool,
        sky::SkyReference,
    )>,
}

// ============================================================================
// Sync System
// ============================================================================

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Whether a `DirectionalLight` can light the scene the cloud dome is in.
///
/// The dome carries no `RenderLayers`, so it sits on the default layer; a light
/// that shares no layer with it cannot illuminate it and must not be mistaken
/// for the sun. A light with no `RenderLayers` is on the default layer too, so
/// `None` counts as scene lighting.
///
/// This exists because the editor keeps several preview rigs alive in the same
/// World, each with its own key and fill lights on its own layer. See the call
/// site for what that cost us.
fn lights_the_scene(layers: Option<&bevy::camera::visibility::RenderLayers>) -> bool {
    let scene = bevy::camera::visibility::RenderLayers::default();
    layers.is_none_or(|layers| layers.intersects(&scene))
}

/// Radius to give the dome: just inside the tightest active far plane.
///
/// This has to track the camera rather than be a constant, because the dome is
/// depth-tested against the scene. Too small and terrain further away than the
/// dome still gets clouds painted over it; too large and the dome is clipped by
/// the far plane and the sky goes empty.
fn dome_radius(projection: Option<&Projection>) -> f32 {
    let far = match projection {
        Some(Projection::Perspective(p)) => p.far,
        Some(Projection::Orthographic(p)) => p.far,
        _ => return DEFAULT_DOME_RADIUS,
    };
    if far.is_finite() && far > 1.0 {
        far * 0.9
    } else {
        DEFAULT_DOME_RADIUS
    }
}

/// Resolve what the sky is currently doing to the cloud lighting, rebuilding the
/// cached noon reference whenever the atmosphere it came from changes.
///
/// Returns the identity only when the coupling is switched off.
///
/// Two situations look like "no atmosphere" and are deliberately *not* treated
/// as one: a scene that never spawned an `Atmosphere` because it is lit by a
/// skybox or an HDRI, and a scene that switched its sky off (which
/// `renzora_atmosphere` represents with a zero-density medium). Both still have
/// a sun in them, and their clouds still have to know what time of day it is — a
/// deck that stays noon-white while everything underneath it goes to dusk is the
/// single most conspicuous way for a sky to look wrong. Both measure Earth's
/// medium instead.
fn atmosphere_transfer(
    state: &mut CloudsState,
    atmospheres: &Query<&Atmosphere>,
    media: &Assets<ScatteringMedium>,
    fallback: &ScatteringMedium,
    clouds_data: &CloudsData,
    sun_dir: Vec3,
) -> SkyTransfer {
    if !clouds_data.atmosphere_lighting {
        return SkyTransfer::NONE;
    }

    let scene_atmosphere = atmospheres
        .iter()
        .next()
        .and_then(|a| media.get(&a.medium).map(|medium| (a, medium)));
    let (medium, inner_radius, outer_radius, medium_id) = match scene_atmosphere {
        Some((atmosphere, medium)) => (
            medium,
            atmosphere.inner_radius,
            atmosphere.outer_radius,
            Some(atmosphere.medium.id()),
        ),
        None => (
            fallback,
            FALLBACK_INNER_RADIUS,
            FALLBACK_OUTER_RADIUS,
            None,
        ),
    };

    let deck_altitude = (clouds_data.bottom_height + clouds_data.top_height) * 0.5;
    let key = (medium_id, inner_radius, outer_radius, deck_altitude);

    let earth = || sky::Sky::new(fallback, FALLBACK_INNER_RADIUS, FALLBACK_OUTER_RADIUS);

    // The reference is three ray integrations and depends on nothing that moves,
    // so only the four live ones are paid for per frame.
    let (use_fallback, reference) = match &state.sky_reference {
        Some((cached, used_fallback, reference)) if *cached == key => {
            (*used_fallback, *reference)
        }
        _ => {
            // Measure the scene's own medium, and fall back to Earth's if it
            // turns out not to scatter — which is how a switched-off sky reads.
            // A scene that dropped the procedural sky for a skybox still has a
            // sun in it, and its clouds still have to follow that sun down.
            let scene = sky::Sky::new(medium, inner_radius, outer_radius);
            let scene_reference = scene.reference(deck_altitude);
            let resolved = if scene_reference.is_usable() {
                (false, scene_reference)
            } else {
                (true, earth().reference(deck_altitude))
            };
            state.sky_reference = Some((key, resolved.0, resolved.1));
            resolved
        }
    };

    if use_fallback {
        earth().transfer(deck_altitude, sun_dir, &reference)
    } else {
        sky::Sky::new(medium, inner_radius, outer_radius)
            .transfer(deck_altitude, sun_dir, &reference)
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_clouds(
    mut commands: Commands,
    time: Res<Time>,
    mut clouds_state: ResMut<CloudsState>,
    // `Entity` on both purely so the diagnostics can say *which* source and
    // *which* camera were picked — an alternating id is the whole signal.
    clouds_query: Query<(Entity, &CloudsData)>,
    camera_query: Query<(Entity, &GlobalTransform, &Camera, Option<&Projection>), With<Camera3d>>,
    sun_query: Query<(
        &GlobalTransform,
        &DirectionalLight,
        Option<&bevy::camera::visibility::RenderLayers>,
    )>,
    atmosphere_query: Query<&Atmosphere>,
    media: Res<Assets<ScatteringMedium>>,
    // Earth's medium, built once, for scenes with no `Atmosphere` to measure.
    fallback_medium: Local<ScatteringMedium>,
    quality: Option<Res<renzora::ResolvedGraphicsQuality>>,
    wind: Option<Res<renzora::WindState>>,
    noise: Option<Res<CloudNoiseTextures>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cloud_materials: ResMut<Assets<CloudMaterial>>,
) {
    // Two per-pixel marches make this the single largest scene-independent
    // raster cost on a weak GPU, so the graphics-quality tier can switch it off
    // entirely (the `Low` tier does). Treated exactly like the inspector toggle:
    // no active clouds ⇒ the dome is despawned below.
    let tier = quality.as_ref().map(|q| q.0);
    let clouds_allowed = tier.map(|t| t.clouds()).unwrap_or(true);

    // First *enabled* clouds component. Honors the inspector toggle — without
    // the `enabled` check, switching clouds off left the dome rendering.
    let active_clouds = clouds_query
        .iter()
        .find(|(_, c)| c.enabled)
        .filter(|_| clouds_allowed);

    // How many candidates exist at all — more than one is itself the bug in the
    // "alternating source" case, and `find` would hide it.
    let enabled_sources = clouds_query.iter().filter(|(_, c)| c.enabled).count();
    Diag::track(&mut clouds_state.diag.allowed, "quality allows clouds", clouds_allowed);
    Diag::track(
        &mut clouds_state.diag.source,
        if enabled_sources > 1 { "source (MULTIPLE enabled!)" } else { "source" },
        active_clouds.map(|(e, _)| e),
    );

    let Some((_, clouds_data)) = active_clouds else {
        // No active clouds — despawn dome if it exists.
        if let Some(dome_entity) = clouds_state.entity.take() {
            info!("[clouds] dome despawned (no enabled CloudsData, or quality gate)");
            commands.entity(dome_entity).despawn();
            clouds_state.material_handle = None;
            clouds_state.mesh_handle = None;
            clouds_state.last_cam_pos = None;
        }
        return;
    };

    // The material cannot exist before the noise handles do, and they are built
    // in `CloudNoisePlugin::finish`.
    let Some(noise) = noise else {
        return;
    };

    let Some((camera_entity, camera_transform, _, projection)) = camera_query
        .iter()
        .find(|(_, _, cam, _)| cam.is_active)
        .or_else(|| camera_query.iter().next())
    else {
        return;
    };

    // An id that alternates frame to frame means two active `Camera3d` and an
    // unordered `find` — the dome would be re-centred on a different eye each
    // frame, which reads exactly as flicker.
    let active_cameras = camera_query.iter().filter(|(_, _, c, _)| c.is_active).count();
    Diag::track(
        &mut clouds_state.diag.camera,
        if active_cameras > 1 { "camera (MULTIPLE active!)" } else { "camera" },
        Some(camera_entity),
    );

    // `GlobalTransform`, not `Transform`: a camera parented to a rig — which is
    // what a flying player camera usually is — has a local translation relative
    // to that rig, and centring the dome on it would leave the deck somewhere
    // else entirely the moment the rig moved. The shader reads the true eye
    // position from the view uniform, so the two have to agree.
    let camera_pos = camera_transform.translation();
    let radius = dome_radius(projection);

    // ── Wind ──
    // Bearing and drift come from the world wind unless the deck opts out. The
    // floor of 0.35 is not a fudge: air aloft is always moving, so a deck that
    // froze solid in a dead-calm scene would look more wrong than one that
    // keeps drifting.
    let world_wind = wind.as_deref().copied().unwrap_or_default();
    let (wind_bearing, wind_speed) = if clouds_data.follow_world_wind {
        (
            world_wind.direction_degrees(),
            clouds_data.speed * (0.35 + 0.65 * world_wind.strength01()),
        )
    } else {
        (clouds_data.wind_direction, clouds_data.speed)
    };
    let wind_rad = wind_bearing.to_radians();
    let wind_dir = Vec3::new(wind_rad.cos(), 0.0, wind_rad.sin());
    clouds_state.wind_offset += wind_dir * (wind_speed * 0.001 * time.delta_secs());

    // Wrap to the base map's world period. An offset that only ever grew would
    // eventually coarsen to metres of f32 resolution and the deck would visibly
    // jitter. The detail volume repeats `detail_scale` times inside one base
    // period, so subtracting a whole base period leaves both samples untouched
    // whenever `detail_scale` is a whole number — which is why its default is.
    let base_period_km = 1.0 / (0.05 * clouds_data.scale.max(0.01));
    clouds_state.wind_offset.x %= base_period_km;
    clouds_state.wind_offset.z %= base_period_km;

    // ── Morph ──
    // The warp field and the detail volume evolve on their own clock, crossing
    // the wind rather than following it.
    let morph_rad = (wind_bearing + MORPH_BEARING_OFFSET).to_radians();
    let morph_dir = Vec3::new(morph_rad.cos(), 0.0, morph_rad.sin());
    let morph_step = clouds_data.morph_speed * 0.001 * time.delta_secs();
    clouds_state.morph_offset += morph_dir * morph_step;
    let warp_period_km = base_period_km * WARP_SPREAD;
    clouds_state.morph_offset.x %= warp_period_km;
    clouds_state.morph_offset.z %= warp_period_km;
    clouds_state.detail_phase = (clouds_state.detail_phase
        + clouds_data.morph_speed * DETAIL_MORPH_RATE * time.delta_secs())
        % 1.0;

    // ── Sun ──
    // Read from the `DirectionalLight` rather than from a `Sun` component, so
    // this works in any scene that has a sun at all — `renzora_lighting` mirrors
    // `Sun` onto the light anyway, and plenty of scenes carry only the light.
    // Brightest wins: a scene with fill lights should still be read by its key.
    //
    // Restricted to lights that share the dome's render layer, which is the
    // default one. The editor keeps several *preview* rigs alive in the same
    // World — the material, model-thumbnail, particle and animation-studio
    // previews each spawn their own key and fill lights at 2000–12000 lux, on
    // their own layers. Those cannot light this scene, but they were winning
    // `max_by` the moment the real sun dimmed: `renzora_lighting::sync_sun`
    // takes the sun's illuminance to exactly 0 at -1° elevation, so from there
    // down the brightest `DirectionalLight` in the World was a preview light
    // pointing wherever its little rig points. That read back as a sun roughly
    // 30° *up*, and the deck faded out at 0° and then snapped back at -1°.
    let sun = sun_query
        .iter()
        .filter(|(_, _, layers)| lights_the_scene(*layers))
        .max_by(|(_, a, _), (_, b, _)| a.illuminance.total_cmp(&b.illuminance));

    let sun_dir = sun
        .map(|(transform, _, _)| -transform.forward().as_vec3())
        .unwrap_or_else(|| Vec3::new(0.5, 0.7, 0.5).normalize())
        .normalize_or(Vec3::Y);

    // Elevation comes from where the light points, not from an authored field,
    // for the same reason.
    let elevation = sun_dir.y.clamp(-1.0, 1.0).asin().to_degrees();
    let sun_tint = sun
        .map(|(_, light, _)| {
            let c = light.color.to_linear();
            Vec3::new(c.red, c.green, c.blue)
        })
        .unwrap_or(Vec3::ONE);
    // Both the direct light and the skylight filling the shadows come from the
    // sun in the end, so both track its illuminance.
    let sun_power = sun
        .map(|(_, light, _)| (light.illuminance / REFERENCE_ILLUMINANCE).clamp(0.0, 4.0))
        .unwrap_or(1.0);
    let day = smoothstep(NIGHT_ELEVATION, DAY_ELEVATION, elevation);

    // ── Atmosphere ──
    // Every authored colour below is a *noon* value; the transfer says what the
    // sky is doing to it right now. Identity when the coupling is off, when there
    // is no atmosphere in the scene, or when the sky has been switched off (which
    // `renzora_atmosphere` does with a zero-density medium).
    let transfer = atmosphere_transfer(
        &mut clouds_state,
        &atmosphere_query,
        &media,
        &fallback_medium,
        clouds_data,
        sun_dir,
    );

    let tint = Vec3::new(clouds_data.color.0, clouds_data.color.1, clouds_data.color.2);
    let sun_color = tint * sun_tint * clouds_data.brightness * sun_power * transfer.sun;

    // No night term here: `day_factor` fades the deck out in the shader, and the
    // atmosphere transfer has already darkened these colours on the way down.
    let ambient = clouds_data.ambient_brightness * sun_power;
    let ambient_top = Vec3::new(
        clouds_data.ambient_color.0,
        clouds_data.ambient_color.1,
        clouds_data.ambient_color.2,
    ) * ambient
        * transfer.zenith;
    // The base of the deck sees the whole sky ring, not one bearing of it, so
    // the two horizon samples are averaged rather than picking a side.
    let ambient_bottom = Vec3::new(
        clouds_data.shadow_color.0,
        clouds_data.shadow_color.1,
        clouds_data.shadow_color.2,
    ) * ambient
        * (transfer.horizon_sunward + transfer.horizon_away)
        * 0.5;

    let horizon = Vec3::new(
        clouds_data.horizon_color.0,
        clouds_data.horizon_color.1,
        clouds_data.horizon_color.2,
    );

    // ── Step budget ──
    let (view_steps, shadow_steps) = match tier {
         Some(renzora::core::viewport_types::GraphicsQuality::High) | None => {
            (clouds_data.raymarch_steps, clouds_data.shadow_steps)
        }
        _ => (
            clouds_data.raymarch_steps.min(MEDIUM_VIEW_STEPS),
            clouds_data.shadow_steps.min(MEDIUM_SHADOW_STEPS),
        ),
    };

    // Keep the deck at least a metre thick: the shader divides by its thickness.
    let bottom_km = clouds_data.bottom_height * 0.001;
    let top_km = (clouds_data.top_height * 0.001).max(bottom_km + 0.001);

    let uniform = CloudsUniform {
        sun_direction: sun_dir.extend(0.0),
        sun_color: sun_color.extend(0.0),
        ambient_top: ambient_top.extend(0.0),
        ambient_bottom: ambient_bottom.extend(0.0),
        haze_sunward: (horizon * transfer.horizon_sunward)
            .extend(clouds_data.atmosphere_strength),
        haze_away: (horizon * transfer.horizon_away).extend(0.0),
        wind_offset: clouds_state.wind_offset.extend(0.0),
        morph_offset: Vec4::new(
            clouds_state.morph_offset.x,
            clouds_state.morph_offset.z,
            clouds_state.detail_phase,
            0.0,
        ),

        planet_radius: (clouds_data.planet_radius * 0.001).max(1.0),
        bottom_height: bottom_km,
        top_height: top_km,
        base_scale: clouds_data.scale.max(0.01),
        detail_scale: clouds_data.detail_scale.max(0.01),
        coverage: clouds_data.coverage,
        extinction: clouds_data.density * MAX_EXTINCTION_PER_KM * clouds_data.absorption,
        detail_strength: clouds_data.detail_strength,
        edge_softness: clouds_data.edge_softness.max(1e-3),
        base_softness: clouds_data.base_softness.max(1e-3),
        powder_strength: clouds_data.powder_strength,
        min_transmittance: MIN_TRANSMITTANCE,
        forward_scattering: clouds_data.forward_scattering,
        backward_scattering: clouds_data.backward_scattering,
        scattering_blend: clouds_data.scattering_blend,
        view_steps: view_steps.max(1),
        shadow_steps,
        day_factor: day,
    };

    if let Some(dome_entity) = clouds_state.entity {
        if commands.get_entity(dome_entity).is_ok() {
            if let Some(ref mat_handle) = clouds_state.material_handle {
                // Only touch the material when a uniform actually changed. A bare
                // `get_mut` marks the asset `Modified` every frame, which rebuilds
                // the bind group + re-uploads the uniforms — pure waste on a still
                // sky. The wind offset does move every frame while `speed` is
                // non-zero, which is exactly when a re-upload is earned.
                let changed = cloud_materials
                    .get(mat_handle)
                    .map(|m| m.uniform != uniform)
                    .unwrap_or(true);
                if changed {
                    if let Some(mut mat) = cloud_materials.get_mut(mat_handle) {
                        mat.uniform = uniform;
                    }
                }
            }
            // Re-centre the dome only when the camera has actually moved, or the
            // far plane changed under it.
            let moved = clouds_state
                .last_cam_pos
                .map(|p| p.distance_squared(camera_pos) > 1.0)
                .unwrap_or(true)
                || (clouds_state.last_radius - radius).abs() > 1.0;
            if moved {
                // How far the dome had drifted from the eye before it caught up.
                // The threshold is 1 unit, so anything approaching that is the
                // deck visibly trailing the camera and then snapping.
                let jump = clouds_state
                    .last_cam_pos
                    .map(|p| p.distance(camera_pos))
                    .unwrap_or(0.0);
                clouds_state.diag.recentres += 1;
                clouds_state.diag.max_jump = clouds_state.diag.max_jump.max(jump);

                let transform =
                    Transform::from_translation(camera_pos).with_scale(Vec3::splat(radius));
                commands.entity(dome_entity).insert(transform);
                clouds_state.last_cam_pos = Some(camera_pos);
                clouds_state.last_radius = radius;
            }
        } else {
            // The dome we built is gone and we did not despawn it. Something
            // else in the app removed it, and the block below will build another
            // — that pair repeating every frame IS the flicker.
            warn!("[clouds] dome entity vanished (despawned by something else) — respawning");
            clouds_state.entity = None;
            clouds_state.material_handle = None;
            clouds_state.mesh_handle = None;
            clouds_state.last_cam_pos = None;
        }
    }

    if clouds_state.entity.is_none() {
        // A fine tessellation, because the view direction is interpolated from
        // the dome's vertices: a coarse sphere is a faceted approximation of one,
        // and the resulting directions warp the sky along the facet seams.
        let mesh_handle = meshes.add(Sphere::new(1.0).mesh().uv(96, 48));
        let material_handle = cloud_materials.add(CloudMaterial {
            uniform,
            base_noise: noise.base.clone(),
            detail_noise: noise.detail.clone(),
        });

        let transform = Transform::from_translation(camera_pos).with_scale(Vec3::splat(radius));

        let dome_entity = commands
            .spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(material_handle.clone()),
                transform,
                CloudDomeMarker,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ))
            .id();

        // First spawn is expected and says so once; any *later* one means the
        // dome is being rebuilt, which should not happen on a steady scene.
        if clouds_state.diag.spawned {
            warn!("[clouds] dome RE-spawned (radius {radius:.0}) — this should not repeat");
        } else {
            info!("[clouds] dome spawned (radius {radius:.0}, camera {camera_entity})");
            clouds_state.diag.spawned = true;
        }

        clouds_state.entity = Some(dome_entity);
        clouds_state.mesh_handle = Some(mesh_handle);
        clouds_state.material_handle = Some(material_handle);
        clouds_state.last_cam_pos = Some(camera_pos);
        clouds_state.last_radius = radius;
    }

    // One throttled line a second, only while the dome is actually moving.
    // `recentres` near the frame rate with a `max jump` close to the 1.0
    // threshold is the re-centre-lag explanation; a low count is not.
    let now = time.elapsed_secs();
    if now >= clouds_state.diag.next_report {
        if clouds_state.diag.recentres > 0 {
            info!(
                "[clouds] {} recentres/s, max jump {:.2} units (threshold 1.00)",
                clouds_state.diag.recentres, clouds_state.diag.max_jump
            );
        }
        clouds_state.diag.recentres = 0;
        clouds_state.diag.max_jump = 0.0;
        clouds_state.diag.next_report = now + 1.0;
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct CloudsPlugin;

impl Plugin for CloudsPlugin {
    fn build(&self, app: &mut App) {
        info!("[clouds] native plugin");
        bevy::asset::embedded_asset!(app, "clouds.wgsl");
        bevy::asset::embedded_asset!(app, "clouds_bake.wgsl");
        app.register_type::<CloudsData>()
            .add_plugins((
                MaterialPlugin::<CloudMaterial>::default(),
                noise::CloudNoisePlugin,
            ))
            .init_resource::<CloudsState>()
            .add_systems(Update, sync_clouds);

        inspector::register(app);
    }
}

// `Runtime`, explicitly: `plugin!` defaults to `Editor` where `add!` defaulted
// to `Runtime`, so omitting it would stop shipping clouds to games.
renzora::plugin!(CloudsPlugin, Runtime);

#[cfg(test)]
mod tests {
    use super::CloudsData;

    /// The editor keeps preview rigs (material, model thumbnail, particle,
    /// animation studio) alive in the same World, each with its own key and
    /// fill lights at 2000-12000 lux on its own render layer. Those must never
    /// be mistaken for the scene's sun.
    ///
    /// This is a real regression, not a hypothetical: `sync_sun` drops the
    /// sun's illuminance to exactly 0 at -1 degree of elevation, so below that
    /// the brightest `DirectionalLight` in the World was a preview light. It
    /// pointed roughly 30 degrees up, so the deck correctly vanished at 0 and
    /// then snapped back to full daylight at -1.
    #[test]
    fn preview_rig_lights_are_not_mistaken_for_the_sun() {
        use super::lights_the_scene;
        use bevy::camera::visibility::RenderLayers;

        // The scene's sun: usually no RenderLayers at all, sometimes an
        // explicit default layer.
        assert!(lights_the_scene(None));
        assert!(lights_the_scene(Some(&RenderLayers::default())));
        assert!(lights_the_scene(Some(&RenderLayers::layer(0))));

        // Preview rigs live on their own layers and light nothing here.
        for layer in [1, 2, 3, 17, 20] {
            assert!(
                !lights_the_scene(Some(&RenderLayers::layer(layer))),
                "a light confined to layer {layer} cannot light the dome"
            );
        }

        // A light on its own layer *and* the default one does light the scene.
        assert!(lights_the_scene(Some(&RenderLayers::from_layers(&[0, 17]))));
    }

    /// The deck must be *fully* gone at and below the horizon, and must get
    /// there gradually rather than snapping. Sunset is the cue, so a partially
    /// lit deck at 0 is the bug this guards: the sky and stars are driven by
    /// the atmosphere, which is already night by then.
    #[test]
    fn the_deck_fades_out_by_the_time_the_sun_reaches_the_horizon() {
        use super::{smoothstep, DAY_ELEVATION, NIGHT_ELEVATION};
        let day = |elev: f32| smoothstep(NIGHT_ELEVATION, DAY_ELEVATION, elev);

        // At and below the horizon: nothing. The shader early-outs under
        // 0.002, so anything at or under that reads as fully gone.
        for elev in [0.0, -0.5, -2.0, -12.0, -45.0, -90.0] {
            assert!(
                day(elev) <= 0.002,
                "sun at {elev}° should leave no cloud, got {}",
                day(elev)
            );
        }

        // Well up in the sky: full strength.
        for elev in [DAY_ELEVATION, 20.0, 60.0, 90.0] {
            assert!(
                day(elev) > 0.999,
                "sun at {elev}° should be full daylight, got {}",
                day(elev)
            );
        }

        // In between: monotonically increasing, and actually partial — a fade
        // that jumped straight from 0 to 1 would pass the two checks above.
        let mut previous = 0.0;
        let mut saw_partial = false;
        for step in 0..=64 {
            let elev = NIGHT_ELEVATION
                + (DAY_ELEVATION - NIGHT_ELEVATION) * (step as f32 / 64.0);
            let d = day(elev);
            assert!(d >= previous, "fade reversed at {elev}°");
            if d > 0.05 && d < 0.95 {
                saw_partial = true;
            }
            previous = d;
        }
        assert!(saw_partial, "fade must pass through partial coverage");
    }

    /// Compile a WGSL module exactly as wgpu will.
    fn validate(name: &str, source: &str) {
        renzora::wgsl::check(source).unwrap_or_else(|err| panic!("{name}: {err}"));
    }

    /// Reads a `const NAME: f32 = ...;` out of the shader. Handles a bare literal
    /// and a single division, which is all the shader uses.
    fn wgsl_const(source: &str, name: &str) -> f32 {
        let prefix = format!("const {name}: f32 =");
        let line = source
            .lines()
            .find(|line| line.trim_start().starts_with(&prefix))
            .unwrap_or_else(|| panic!("`{name}` is not declared in clouds.wgsl"));
        let value = line.split('=').nth(1).unwrap().trim().trim_end_matches(';');
        match value.split_once('/') {
            Some((numerator, denominator)) => {
                numerator.trim().parse::<f32>().unwrap()
                    / denominator.trim().parse::<f32>().unwrap()
            }
            None => value.parse().unwrap(),
        }
    }

    /// The erosion detail's frequency is tied to `scale` and `detail_scale`, and
    /// the march samples the deck `1 / FINE_STEP_FRACTION` times. Let those two
    /// drift apart and consecutive samples land about a cycle apart, at which
    /// point the detail stops being wisps and starts beating against the steps.
    ///
    /// It is worth pinning because of how the failure looks: not like noise, but
    /// like combed vertical streaks hanging off the underside of every cloud —
    /// which reads as something wrong with the cloud *shapes*, and sends you
    /// looking in the wrong place. It has already happened twice, once when the
    /// deck was made thicker and once when `scale` went up.
    ///
    /// Checked against the shader's own constants rather than a remembered
    /// number, so tuning one side cannot silently invalidate the other.
    #[test]
    fn the_default_detail_stays_inside_what_the_default_march_resolves() {
        let shader = include_str!("clouds.wgsl");
        let fine_fraction = wgsl_const(shader, "FINE_STEP_FRACTION");
        let safe = wgsl_const(shader, "DETAIL_NYQUIST_SAFE");

        let clouds = CloudsData::default();
        let thickness_km = (clouds.top_height - clouds.bottom_height) * 0.001;
        let fine_step_km = thickness_km * fine_fraction;

        // Mirrors `detail_resolved` in the shader.
        let cycles_per_step =
            fine_step_km * 1.6 * clouds.scale * clouds.detail_scale / 32.0;

        assert!(
            cycles_per_step < safe,
            "the default erosion detail runs at {cycles_per_step:.3} cycles per \
             march step, past the {safe} the shader keeps all of it below; it \
             will be faded out, and near the limit it combs. Lower `detail_scale` \
             or `scale`, or take smaller steps.",
        );
    }

    #[test]
    fn bake_shader_compiles() {
        validate("clouds_bake", include_str!("clouds_bake.wgsl"));
    }

    /// The dome shader imports two Bevy modules that naga cannot resolve on its
    /// own, so they are swapped for the minimum this shader actually reads. That
    /// leaves the raymarch itself — the part with the arithmetic in it — under
    /// exactly the validation wgpu would apply.
    #[test]
    fn dome_shader_compiles() {
        const STUB: &str = "
            struct View { world_position: vec3<f32> }
            @group(0) @binding(0) var<uniform> view: View;
            struct VertexOutput {
                @builtin(position) position: vec4<f32>,
                @location(0) world_position: vec4<f32>,
            }
        ";
        let source: String = std::iter::once(STUB.to_string())
            .chain(
                include_str!("clouds.wgsl")
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("#import"))
                    .map(|line| line.to_string()),
            )
            .collect::<Vec<_>>()
            .join("
");
        validate("clouds", &source);
    }
}
