pub mod glare;

// Bevy 0.19: `Atmosphere`/`ScatteringMedium` moved to `bevy::light`;
// `AtmosphereMode`/`AtmosphereSettings` stay in `bevy::pbr`.
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{Atmosphere, AtmosphereEnvironmentMapLight};
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

/// A camera's atmosphere components, held while the sky is off.
///
/// Turning the sky off takes the atmosphere out of the renderer entirely: the
/// planet's `Atmosphere`, and each camera's `AtmosphereSettings` and sky probe
/// (`AtmosphereEnvironmentMapLight`, whose removal also drops the environment
/// lighting Bevy derived from it). What the camera had is kept here and put
/// back when the sky comes on, so a camera's probe size and settings survive
/// the round trip. A camera that gains them while the sky is off (spawned, or
/// rehydrated with a scene) is parked the next frame.
#[derive(Component)]
pub struct ParkedSky {
    settings: Option<AtmosphereSettings>,
    probe: Option<AtmosphereEnvironmentMapLight>,
}

/// Every camera that has, or is holding, atmosphere components.
type SkyCamera = (
    Entity,
    Option<&'static mut AtmosphereSettings>,
    Option<&'static AtmosphereEnvironmentMapLight>,
    Option<&'static ParkedSky>,
);
type SkyCameraFilter = (
    With<Camera>,
    Or<(With<AtmosphereSettings>, With<AtmosphereEnvironmentMapLight>, With<ParkedSky>)>,
);

/// Drives the sky from the World Environment entity carrying
/// `AtmosphereComponentSettings`: maintains one hidden [`AtmospherePlanet`]
/// entity that holds the real `Atmosphere`, and mirrors the render mode onto the
/// routed cameras' `AtmosphereSettings`.
///
/// Turning the sky off removes the atmosphere completely: `Atmosphere` from the
/// planet, and the cameras' atmosphere components (see [`ParkedSky`]), so none
/// of its passes, bindings or bakes exist. It used to stay on with a
/// zero-density medium, because removing it crashed under Bevy 0.18 and 0.19
/// (the camera's bind-group layout was locked to the atmosphere bindings it
/// started with). Bevy 0.20 drops a camera's atmosphere state the frame its
/// `Atmosphere` goes (`extract_atmosphere`), and removing all three, then
/// adding them back, runs clean. Until then the lookup tables, the sky draw and
/// the sky bake ran every frame for a sky nobody could see.
fn sync_atmosphere(
    mut commands: Commands,
    mut mediums: ResMut<Assets<ScatteringMedium>>,
    // The one `ScatteringMedium`, created once and held for the app's life, so
    // it outlives every `Atmosphere` that points at it. The render world runs a
    // frame behind: a medium freed with the component it was on was still being
    // read there ("ScatteringMedium missing"), and recreating it on each toggle
    // was the old flicker and `create_buffer_with_data` "buffer invalid" crash.
    mut media: Local<Option<Handle<ScatteringMedium>>>,
    // The `(source entity, enabled)` last applied to the resident planet. Lets us
    // re-apply the correct medium when the source *reappears* (World Environment
    // removed then re-added) — its `is_changed()` flag has lapsed by the time the
    // routing re-includes it, so a value-change gate alone misses the transition.
    mut applied: Local<Option<(Entity, bool)>>,
    sources: Query<(Entity, Ref<AtmosphereComponentSettings>)>,
    planet: Query<(Entity, Has<Atmosphere>), With<AtmospherePlanet>>,
    mut cameras: Query<SkyCamera, SkyCameraFilter>,
    routing: Res<renzora::EffectRouting>,
    suppressed: Option<Res<renzora::core::EnvironmentSuppressed>>,
) {
    let routing_changed = routing.is_changed();

    // Reconcile, don't patch: derive the sky strictly from "is there a routed,
    // still-existing, ENABLED source?". This makes removing the source entity,
    // removing the `AtmosphereComponentSettings` component, AND toggling
    // `enabled` all tear the sky down uniformly — no event bookkeeping that can
    // miss a despawn. The first routed source carrying
    // `AtmosphereComponentSettings` wins, enabled or not (a disabled one is what
    // turns the sky off); collect every camera routed to it so they share its
    // render mode.
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

    // A shading mode that shows no environment (wireframe, solid) turns the
    // sky off the same way a *disabled* source does. Folding it into
    // `src_enabled` also means it flows through `key`, so flipping the flag
    // reconciles like any other change to the source.
    if suppressed.as_deref().is_some_and(|s| s.active) {
        src_enabled = false;
    }

    let planet_entity = planet.iter().next();
    let have_sky = planet_entity.is_some_and(|(_, has)| has);
    let medium = media.get_or_insert_with(|| mediums.add(ScatteringMedium::default())).clone();

    // A *disabled* source and **no source at all** (the whole World Environment
    // was deleted) both turn the sky off. `key` is the applied state we
    // reconcile against.
    let source = active_src.and_then(|s| sources.get(s).ok());
    let (inner, outer, ground, mode, settings_changed, key) = match &source {
        Some((e, settings)) if src_enabled => (
            settings.bottom_radius,
            settings.top_radius,
            settings.ground_albedo,
            settings.mode,
            settings.is_changed(),
            Some((*e, true)),
        ),
        // Off: take the `Atmosphere` away and keep the (now empty) planet, so
        // turning the sky back on is a component insert, not a respawn. Park
        // every camera's atmosphere components, including any that gained them
        // since (a camera spawned while the sky is off).
        _ => {
            if have_sky {
                if let Some((planet_e, _)) = planet_entity {
                    commands.entity(planet_e).remove::<Atmosphere>();
                }
            }
            for (entity, settings, probe, parked) in &cameras {
                if parked.is_some() {
                    continue;
                }
                commands
                    .entity(entity)
                    .remove::<(AtmosphereSettings, AtmosphereEnvironmentMapLight)>()
                    .insert(ParkedSky {
                        settings: settings.cloned(),
                        probe: probe.cloned(),
                    });
            }
            *applied = source.as_ref().map(|(e, _)| (*e, false));
            return;
        }
    };
    let rendering_method = match mode {
        1 => AtmosphereMode::Raymarched,
        _ => AtmosphereMode::LookupTexture,
    };

    // On: give parked cameras back what they had, in the current render mode.
    for (entity, _, _, parked) in &cameras {
        let Some(parked) = parked else { continue };
        let mut e = commands.entity(entity);
        e.remove::<ParkedSky>();
        if let Some(mut settings) = parked.settings.clone() {
            settings.rendering_method = rendering_method;
            e.insert(settings);
        }
        if let Some(probe) = parked.probe.clone() {
            e.insert(probe);
        }
    }

    // Rebuild the planet's `Atmosphere` on first build, a real value change, OR
    // when the applied `(source, enabled)` key differs — the last catches the
    // source reappearing (remove/re-add, whose `is_changed()` lapses before
    // routing re-includes it) and the sky coming back on. NOT gated on
    // `routing_changed` alone: camera-set churn shouldn't re-fire the on-add hook.
    let rebuilt = !have_sky || settings_changed || *applied != key;
    if rebuilt {
        *applied = key;
        let atmosphere = Atmosphere {
            // 0.18 `bottom_radius`/`top_radius` → 0.19 `inner_radius`/`outer_radius`.
            inner_radius: inner,
            outer_radius: outer,
            ground_albedo: Vec3::splat(ground),
            medium,
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

    // Mirror the render mode onto routed cameras. Also on `rebuilt`: a mode
    // changed while the sky was off was skipped then, and has to land when it
    // comes back on.
    if routing_changed || rebuilt {
        for &target in &targets {
            if let Ok((_, Some(mut s), _, _)) = cameras.get_mut(target) {
                // `AtmosphereMode` isn't `PartialEq`; just assign.
                s.rendering_method = rendering_method;
            }
        }
    }
}

/// Marks a camera whose clear colour [`sync_space_backdrop`] set to black, so
/// it is only ever handed back what it had.
#[derive(Component)]
struct SpaceBackdrop;

/// Clear the cameras an enabled sky renders for to black.
///
/// `render_sky` composites `framebuffer * transmittance + inscattering`, so
/// whatever the camera cleared to shows through the sky wherever the air
/// scatters little. By day that is invisible under the inscattering. At night
/// there is almost none, and Bevy's default `ClearColor`, an editor grey,
/// came through as a flat brown-grey overcast that no sun angle, cloud or
/// exposure setting changed. Behind a real sky is space, so black is right.
///
/// Only a camera on `ClearColorConfig::Default` is changed, and it is marked,
/// so a colour the user chose is never touched and turning the sky off (or
/// deleting it) hands back exactly the default it had.
fn sync_space_backdrop(
    mut commands: Commands,
    routing: Res<renzora::EffectRouting>,
    sources: Query<&AtmosphereComponentSettings>,
    suppressed: Option<Res<renzora::core::EnvironmentSuppressed>>,
    mut cameras: Query<(Entity, &mut Camera, Has<SpaceBackdrop>)>,
) {
    let sky_hidden = suppressed.as_deref().is_some_and(|s| s.active);
    for (entity, mut camera, marked) in &mut cameras {
        let has_sky = !sky_hidden
            && routing.iter().any(|(target, source_list)| {
                *target == entity
                    && source_list
                        .iter()
                        .find_map(|&src| sources.get(src).ok())
                        .is_some_and(|settings| settings.enabled)
            });
        match (has_sky, marked) {
            (true, false) if matches!(camera.clear_color, ClearColorConfig::Default) => {
                camera.clear_color = ClearColorConfig::Custom(Color::BLACK);
                commands.entity(entity).insert(SpaceBackdrop);
            }
            (false, true) => {
                camera.clear_color = ClearColorConfig::Default;
                commands.entity(entity).remove::<SpaceBackdrop>();
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AtmospherePlugin");
        app.register_type::<AtmosphereComponentSettings>();
        app.add_systems(Update, (sync_atmosphere, sync_space_backdrop));
        glare::build(app);
    }
}

renzora::add!(AtmospherePlugin);
