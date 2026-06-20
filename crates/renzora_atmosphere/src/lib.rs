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

/// Tracks the `ScatteringMedium` asset for an atmosphere source so we don't
/// recreate it every frame.
#[derive(Component)]
pub struct AtmosphereMediumHandle(Handle<ScatteringMedium>);

/// Marker for the single dedicated entity that carries the bevy `Atmosphere`.
///
/// **Why this can't live on the World Environment / camera (0.19 architecture,
/// load-bearing).** 0.18 had `Atmosphere` as a *camera* component. 0.19 makes
/// the entity's `GlobalTransform` the **planet center**: `extract_atmosphere`
/// stores `world_to_atmosphere = inverse(global_transform)`, and an `on_add`
/// hook drops a fresh atmosphere to `(0, -inner_radius, 0)` so the scene sits on
/// the planet surface. That means the host entity must (a) be *stationary* and
/// (b) have NO `Transform` (a `Transform` would let propagation overwrite the
/// hook's planet-center `GlobalTransform` back to the origin → camera 6,360 km
/// underground → no sky). Renzora's World Environment entity fails both: it
/// carries a rotation `Transform` (it aims the sun's `DirectionalLight`), which
/// would also tilt the whole sky. So the authored `AtmosphereComponentSettings`
/// stays on the World Environment, but the actual `Atmosphere` lives here, on a
/// hidden transform-free entity we own.
#[derive(Component)]
pub struct AtmospherePlanet;

/// Planet-center `GlobalTransform`: `inner_radius` below the origin so world
/// `Y = 0` is the surface (matches bevy's `Atmosphere` on-add default).
fn planet_transform(inner_radius: f32) -> GlobalTransform {
    GlobalTransform::from(Transform::from_translation(Vec3::NEG_Y * inner_radius))
}

/// Drives the sky from the World Environment entity carrying
/// `AtmosphereComponentSettings`: maintains one hidden [`AtmospherePlanet`]
/// entity that holds the real `Atmosphere`, and mirrors the render mode onto the
/// routed cameras' `AtmosphereSettings`.
///
/// `enabled = false` genuinely turns the sky OFF — 0.19 re-extracts `Atmosphere`
/// every frame and the render node `ViewQuery`-skips cameras without it (no
/// bind-group-layout lock like 0.18), so we just strip `Atmosphere` from the
/// planet.
fn sync_atmosphere(
    mut commands: Commands,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
    sources: Query<(
        Entity,
        Ref<AtmosphereComponentSettings>,
        Option<&AtmosphereMediumHandle>,
    )>,
    planet: Query<(Entity, Has<Atmosphere>), With<AtmospherePlanet>>,
    mut cam_settings: Query<&mut AtmosphereSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();

    // Reconcile, don't patch: derive the sky strictly from "is there a routed,
    // still-existing, ENABLED source?". This makes removing the source entity,
    // removing the `AtmosphereComponentSettings` component, AND toggling
    // `enabled` all tear the sky down uniformly — no event bookkeeping that can
    // miss a despawn. The first such source drives the sky; collect every camera
    // routed to it so they share its render mode.
    let mut active_src = None;
    let mut targets: Vec<Entity> = Vec::new();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            let Ok((entity, settings, _)) = sources.get(src) else {
                continue;
            };
            if active_src.is_none() && settings.enabled {
                active_src = Some(entity);
            }
            if active_src == Some(entity) {
                targets.push(*target);
            }
            break;
        }
    }

    let planet_entity = planet.iter().next();
    let have_sky = planet_entity.is_some_and(|(_, has)| has);

    let Some(src) = active_src else {
        // No enabled source (removed / despawned / disabled) → strip the sky.
        // 0.19 re-extracts `Atmosphere` every frame and `ViewQuery`-skips cameras
        // without it, so removing the component is a clean "off".
        if have_sky {
            if let Some((planet_e, _)) = planet_entity {
                commands.entity(planet_e).remove::<Atmosphere>();
            }
        }
        return;
    };

    let Ok((_, settings, existing_handle)) = sources.get(src) else {
        return;
    };

    // Only rebuild on an actual change (or when the sky isn't up yet) — this
    // system otherwise runs every frame for the reconcile above.
    if !have_sky || routing_changed || settings.is_changed() {
        let handle = if let Some(h) = existing_handle {
            h.0.clone()
        } else {
            let h = mediums.add(ScatteringMedium::default());
            commands
                .entity(src)
                .insert(AtmosphereMediumHandle(h.clone()));
            h
        };

        let atmosphere = Atmosphere {
            // 0.18 `bottom_radius`/`top_radius` → 0.19 `inner_radius`/
            // `outer_radius` (meters; 0.19 dropped `scene_units_to_m`).
            inner_radius: settings.bottom_radius,
            outer_radius: settings.top_radius,
            ground_albedo: Vec3::splat(settings.ground_albedo),
            medium: handle,
        };
        // Explicit planet-center transform (tracks radius edits; the on-add hook
        // only fires once). No `Transform`, so propagation never overwrites it.
        let xform = planet_transform(settings.bottom_radius);

        if let Some((planet_e, _)) = planet_entity {
            commands.entity(planet_e).insert((atmosphere, xform));
        } else {
            commands.spawn((
                AtmospherePlanet,
                atmosphere,
                xform,
                Name::new("Sky Atmosphere"),
                renzora::HideInHierarchy,
            ));
        }

        // Mirror the render mode onto every camera routed to this source.
        let rendering_method = match settings.mode {
            1 => AtmosphereMode::Raymarched,
            _ => AtmosphereMode::LookupTexture,
        };
        for &target in &targets {
            if let Ok(mut s) = cam_settings.get_mut(target) {
                // `AtmosphereMode` isn't `PartialEq`; just assign.
                s.rendering_method = rendering_method;
            }
        }
    }
}

/// Drop the medium bookkeeping when `AtmosphereComponentSettings` is removed from
/// a surviving entity (the sky itself is torn down by `sync_atmosphere`'s
/// reconcile). A despawned source takes its components with it, so nothing to do
/// there.
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
