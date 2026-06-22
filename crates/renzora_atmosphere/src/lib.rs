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
    // Two persistent `ScatteringMedium`s — `(real, transparent)` — created ONCE
    // and reused for the app's life. Holding them here (not as components on the
    // source) keeps the assets alive so toggling never recreates a buffer (the
    // old flicker + `create_buffer_with_data` "buffer invalid" crash). The
    // transparent one is the sky's "off" (see the selection below).
    mut media: Local<Option<(Handle<ScatteringMedium>, Handle<ScatteringMedium>)>>,
    // The `(source entity, enabled)` last applied to the resident planet. Lets us
    // re-apply the correct medium when the source *reappears* (World Environment
    // removed then re-added) — its `is_changed()` flag has lapsed by the time the
    // routing re-includes it, so a value-change gate alone misses the transition.
    mut applied: Local<Option<(Entity, bool)>>,
    sources: Query<(Entity, Ref<AtmosphereComponentSettings>)>,
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
    // First routed source carrying `AtmosphereComponentSettings` wins —
    // regardless of `enabled`. A *disabled* source still drives the resident
    // atmosphere (to the transparent medium); it is never removed.
    let mut active_src: Option<Entity> = None;
    let mut src_enabled = false;
    let mut targets: Vec<Entity> = Vec::new();
    for (target, source_list) in routing.iter() {
        for &src in source_list {
            let Ok((entity, settings)) = sources.get(src) else {
                continue;
            };
            if active_src.is_none() {
                active_src = Some(entity);
                src_enabled = settings.enabled;
            }
            if active_src == Some(entity) {
                targets.push(*target);
            }
            break;
        }
    }

    let planet_entity = planet.iter().next();
    let have_sky = planet_entity.is_some_and(|(_, has)| has);

    // Two persistent mediums (created once). `render_sky` composites
    // `framebuffer * transmittance + inscattering`, so the transparent medium
    // (zero density → transmittance 1, inscattering 0) makes the resident
    // atmosphere show whatever's behind it (clear color / skybox) = the sky
    // "off", with NO component removal → no crash.
    let (real, transparent) = media
        .get_or_insert_with(|| {
            (
                mediums.add(ScatteringMedium::default()),
                mediums.add(ScatteringMedium::default().with_density_multiplier(0.0)),
            )
        })
        .clone();

    // Resolve the desired sky from the source. A *disabled* source AND **no
    // source at all** (the whole World Environment was deleted) both map to the
    // transparent medium — so deleting the entity turns the sky off exactly like
    // the toggle does, and `Atmosphere` is still never removed (which would
    // crash). `key` is the applied state we reconcile against.
    let source = active_src.and_then(|s| sources.get(s).ok());
    let (handle, inner, outer, ground, mode, settings_changed, key) = match &source {
        Some((e, settings)) => (
            if src_enabled { real } else { transparent },
            settings.bottom_radius,
            settings.top_radius,
            settings.ground_albedo,
            settings.mode,
            settings.is_changed(),
            Some((*e, src_enabled)),
        ),
        // No source: only act if a planet already exists — don't spawn one just
        // to hide it. Default radii are fine; a transparent atmosphere shows the
        // background regardless of geometry.
        None if have_sky => (transparent, 6_360_000.0, 6_460_000.0, 0.3, 0, false, None),
        None => return,
    };

    // Rebuild the planet's `Atmosphere` on first build, a real value change, OR
    // when the applied `(source, enabled)` key differs — the last catches the
    // source reappearing (remove/re-add, whose `is_changed()` lapses before
    // routing re-includes it) AND disappearing (→ transparent/off). NOT gated on
    // `routing_changed` alone: camera-set churn shouldn't re-fire the on-add hook.
    if !have_sky || settings_changed || *applied != key {
        *applied = key;
        let atmosphere = Atmosphere {
            // 0.18 `bottom_radius`/`top_radius` → 0.19 `inner_radius`/`outer_radius`.
            inner_radius: inner,
            outer_radius: outer,
            ground_albedo: Vec3::splat(ground),
            medium: handle,
        };
        // Explicit planet-center transform. No `Transform`, so propagation never
        // overwrites it.
        let xform = planet_transform(inner);

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
    }

    // Mirror the render mode onto routed cameras — only when a source drives it.
    if source.is_some() && (routing_changed || settings_changed) {
        let rendering_method = match mode {
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

#[derive(Default)]
pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AtmospherePlugin");
        app.register_type::<AtmosphereComponentSettings>();
        app.add_systems(Update, sync_atmosphere);
    }
}

renzora::add!(AtmospherePlugin);
