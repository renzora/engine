// Bevy 0.19: `Atmosphere`/`ScatteringMedium` moved to `bevy::light`;
// `AtmosphereMode`/`AtmosphereSettings` stay in `bevy::pbr`.
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::Atmosphere;
use bevy::pbr::{AtmosphereMode, AtmosphereSettings};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AtmosphereComponentSettings {
    pub bottom_radius: f32,
    pub top_radius: f32,
    pub ground_albedo: f32,
    pub scene_units_to_m: f32,
    /// 0 = LookupTexture, 1 = Raymarched
    pub mode: u32,
    pub enabled: bool,
}

impl Default for AtmosphereComponentSettings {
    fn default() -> Self {
        Self {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            ground_albedo: 0.3,
            scene_units_to_m: 1.0,
            mode: 1,
            enabled: true,
        }
    }
}

/// Stores the ScatteringMedium handle so we don't recreate it every frame.
#[derive(Component)]
pub struct AtmosphereMediumHandle(Handle<ScatteringMedium>);

/// Sync atmosphere settings from a `WorldEnvironment`-style source entity
/// onto every camera the routing table targets.
///
/// Bevy 0.18 freezes the camera's bind group layout the first frame the
/// camera renders, with atmosphere bindings present iff the `Atmosphere`
/// component existed at that moment. This function therefore *replaces
/// values* rather than adding/removing components — adding atmosphere at
/// runtime crashes wgpu with a 20-vs-23 binding mismatch, and removing it
/// breaks any subsequent re-add. The camera spawn site is responsible for
/// attaching the components up front (see `renzora_engine::camera`); we
/// just keep them in sync with whatever the user authored.
///
/// `enabled = false` becomes a no-op — there's no clean "disable" path in
/// Bevy 0.18's atmosphere. The user-facing toggle effectively means "stop
/// updating from this source," and the camera retains its last-known
/// values (or its spawn defaults if no source ever drove it).
fn sync_atmosphere(
    mut commands: Commands,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
    sources: Query<(
        Entity,
        Ref<AtmosphereComponentSettings>,
        Option<&AtmosphereMediumHandle>,
    )>,
    existing: Query<&Atmosphere>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        // `Atmosphere` can't be added at runtime (Bevy specializes the bind
        // group layout at first render). So only *update* cameras that already
        // carry it — i.e. the dedicated environment/bake camera. Other routed
        // cameras (extra viewports, previews) share that camera's baked sky via
        // a `Skybox` instead of getting their own atmosphere pass.
        if existing.get(*target).is_err() {
            continue;
        }
        for &src in source_list {
            if let Ok((entity, settings, existing_handle)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    break;
                }

                // Reuse the camera's existing atmosphere medium handle
                // when present so the GPU resource stays valid; only
                // allocate a fresh one when this is a brand-new source.
                let handle = if let Some(h) = existing_handle {
                    h.0.clone()
                } else if let Ok(atmo) = existing.get(*target) {
                    atmo.medium.clone()
                } else {
                    let h = mediums.add(ScatteringMedium::default());
                    commands
                        .entity(entity)
                        .insert(AtmosphereMediumHandle(h.clone()));
                    h
                };

                let rendering_method = match settings.mode {
                    1 => AtmosphereMode::Raymarched,
                    _ => AtmosphereMode::LookupTexture,
                };

                // `enabled = false` collapses the atmosphere to a sliver
                // and zeroes ground albedo — Bevy 0.18 won't let us strip
                // the components without crashing the deferred pipeline,
                // so this is the closest thing we have to "atmosphere
                // off." IBL is handled separately by EnvironmentMapPlugin.
                let (ground_albedo, top_radius) = if settings.enabled {
                    (settings.ground_albedo, settings.top_radius)
                } else {
                    (0.0, settings.bottom_radius + 1.0)
                };

                // `insert` replaces the existing components in place — no
                // bind group layout change because the camera already had
                // these slots from spawn-time.
                commands.entity(*target).insert((
                    Atmosphere {
                        // 0.19: bottom/top_radius → inner/outer_radius.
                        inner_radius: settings.bottom_radius,
                        outer_radius: top_radius,
                        ground_albedo: Vec3::splat(ground_albedo),
                        medium: handle,
                    },
                    AtmosphereSettings {
                        // 0.19 removed `scene_units_to_m` (scale is now implicit
                        // in the meter-based radii above).
                        rendering_method,
                        ..default()
                    },
                ));
                break;
            }
        }
    }
}

/// When the source `AtmosphereComponentSettings` is removed (entity
/// despawn or component removed via inspector), drop our medium-handle
/// bookkeeping. We deliberately do NOT remove `Atmosphere` /
/// `AtmosphereSettings` / `AtmosphereEnvironmentMapLight` / `Msaa` from
/// the target cameras — see `sync_atmosphere` for the why. The camera
/// keeps rendering with its last-applied (or spawn-default) atmosphere.
fn cleanup_atmosphere(
    mut commands: Commands,
    mut removed: RemovedComponents<AtmosphereComponentSettings>,
) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<AtmosphereMediumHandle>();
        }
    }
}

#[derive(Default)]
pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AtmospherePlugin");
        app.register_type::<AtmosphereComponentSettings>();
        app.add_systems(Update, (sync_atmosphere, cleanup_atmosphere));
    }
}

renzora::add!(AtmospherePlugin);
