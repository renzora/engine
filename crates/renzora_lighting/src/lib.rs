//! Renzora Lighting — custom lighting components with editor inspectors.
//!
//! Provides `Sun` (directional light via azimuth/elevation), with sync
//! systems that keep the underlying Bevy `DirectionalLight` in lockstep.

use bevy::light::{ShadowFilteringMethod, SunDisk};
use bevy::prelude::*;

// ============================================================================
// Sun component
// ============================================================================

/// The `Sun` component now lives in the **contract crate**.
///
/// Six crates read it, and one of them — the `night_stars` plugin, which fades
/// the starfield by sun elevation — is loaded at runtime rather than linked, so
/// a binary and a `dlopen`ed library both have to name the same type. Two
/// definitions would compile and then simply never match each other's queries.
///
/// Re-exported here so every existing `renzora_lighting::Sun` path still works.
pub use renzora::Sun;

// ============================================================================
// Sync system — keeps DirectionalLight + Transform in sync with Sun
// ============================================================================

/// Sun-brightness multiplier as a function of elevation above the horizon.
/// Fades smoothly from 0 at -1° to 1 at +5° so dawn/dusk has a soft
/// transition rather than the directional light snapping on/off. Returns
/// 0 for any negative elevation past -1° — the sun is "set" and shouldn't
/// be casting downward-then-upward light into the scene.
///
/// Public so other crates (e.g. `renzora_environment_map`) can scale
/// atmospheric IBL by the same factor — when the sun is below the
/// horizon the procedural sky cubemap is dark anyway, and applying the
/// same horizon fade ensures no residual ambient leaks in.
pub fn sun_horizon_factor(elevation_deg: f32) -> f32 {
    renzora::sun::horizon_factor(elevation_deg)
}

/// How far the moon has risen, 0 to 1: nothing while the sun is up, rising as
/// the sun sinks from 1 to 7 degrees below the horizon.
///
/// It starts where [`sun_horizon_factor`] has finished, so the two never light
/// the scene at once: the light is the sun's until the sun is gone, then the
/// moon's. Twilight between them is the sky's.
pub fn moon_factor(elevation_deg: f32) -> f32 {
    renzora::sun::moon_factor(elevation_deg)
}

/// What the scene's one directional light should be for this `Sun`: the sun
/// while it is up, the moon once it has set.
///
/// One light rather than a second moon entity, so every scene with a sun
/// already has a moon and nothing new is saved into it. Below the horizon the
/// sun used to fade to nothing and take the sky light with it, leaving a night
/// scene pitch black except for whatever glowed.
pub struct SunLight {
    pub color: Color,
    pub illuminance: f32,
    pub direction: Vec3,
    /// Whether anything is lighting the scene, so shadows are worth casting.
    pub lit: bool,
}

pub fn sun_light(sun: &Sun) -> SunLight {
    let day = sun_horizon_factor(sun.elevation);
    if day > 0.0 {
        return SunLight {
            color: Color::srgb(sun.color.x, sun.color.y, sun.color.z),
            illuminance: sun.illuminance * day,
            direction: sun.direction(),
            lit: true,
        };
    }
    let night = moon_factor(sun.elevation);
    // A thin crescent lights far less than a full moon, but never nothing:
    // a new-moon night still keeps some sky glow to read the scene by.
    let phase = 0.15 + 0.85 * sun.moon_lit_fraction();
    // In the Earth's shadow the moon only gets light bent red through the
    // Earth's atmosphere: dim and coppery.
    let eclipse = sun.moon_eclipse.clamp(0.0, 1.0);
    let dimming = 1.0 - 0.9 * eclipse;
    let red = Vec3::ONE.lerp(Vec3::new(1.0, 0.4, 0.25), eclipse);
    let illuminance = sun.moon_illuminance.max(0.0) * night * phase * dimming;
    let c = sun.moon_color * red;
    SunLight {
        color: Color::srgb(c.x, c.y, c.z),
        illuminance,
        direction: sun.moon_direction(),
        lit: illuminance > 0.0,
    }
}

fn sync_sun(
    mut commands: Commands,
    mut query: Query<
        (Entity, &Sun, &mut DirectionalLight, &mut Transform),
        Or<(Changed<Sun>, Without<SunDisk>)>,
    >,
) {
    for (entity, sun, mut light, mut transform) in &mut query {
        // The authored `sun.illuminance` stays as typed in the inspector; the
        // light carries the effective value, sun or moon. See `sun_light`.
        let state = sun_light(sun);
        light.color = state.color;
        light.illuminance = state.illuminance;
        light.shadow_maps_enabled = sun.shadows_enabled && state.lit;
        light.contact_shadows_enabled = sun.contact_shadows && state.lit;
        *transform =
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, state.direction));
        // The disc is the sun's only. Bevy draws it where the light points, so
        // at night it would sit where the moon is, but as a flat untextured
        // spot; `renzora_atmosphere::moon` draws the moon itself.
        commands.entity(entity).try_insert(SunDisk {
            angular_size: sun.angular_diameter.to_radians(),
            intensity: sun.sun_disk_intensity * sun_horizon_factor(sun.elevation),
        });
    }
}

// ============================================================================
// Soft shadows → temporal shadow filtering
// ============================================================================

/// Switch every 3D camera to temporal shadow filtering while any light casts
/// soft shadows, and back to Bevy's default once none does.
///
/// Soft shadows sample the shadow map with a sparse, rotating pattern, so a
/// single frame of them is stippled. Only `ShadowFilteringMethod::Temporal`
/// varies that pattern from frame to frame, which is what lets TAA average it
/// into a smooth penumbra; under the default filter the stipple is fixed and
/// TAA has nothing to smooth. Leaving this to the user meant soft shadows
/// looked broken the moment they were switched on.
///
/// Nothing else in the engine sets `ShadowFilteringMethod`, so this owns it.
/// It writes only when the wanted value differs, so a steady scene costs a
/// scan of its lights and no change detection churn.
fn match_shadow_filtering(
    mut commands: Commands,
    directional: Query<&DirectionalLight>,
    points: Query<&PointLight>,
    spots: Query<&SpotLight>,
    mut cameras: Query<(Entity, Option<&mut ShadowFilteringMethod>), With<Camera3d>>,
) {
    let soft = directional
        .iter()
        .any(|l| l.shadow_maps_enabled && l.soft_shadow_size.is_some())
        || points
            .iter()
            .any(|l| l.shadow_maps_enabled && l.soft_shadows_enabled)
        || spots
            .iter()
            .any(|l| l.shadow_maps_enabled && l.soft_shadows_enabled);
    let wanted = if soft {
        ShadowFilteringMethod::Temporal
    } else {
        ShadowFilteringMethod::default()
    };
    for (entity, method) in &mut cameras {
        match method {
            Some(mut method) if *method != wanted => *method = wanted,
            Some(_) => {}
            // A camera without the component renders with the default, so it
            // only needs one when soft shadows want something else.
            None if wanted != ShadowFilteringMethod::default() => {
                commands.entity(entity).try_insert(wanted);
            }
            None => {}
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct LightingPlugin;

/// Observer: handle sun angle changes from scripts via ScriptAction.
fn handle_sun_script_actions(trigger: On<renzora::ScriptAction>, mut sun_query: Query<&mut Sun>) {
    let action = trigger.event();
    if action.name != "set_sun_angles" {
        return;
    }
    use renzora::ScriptActionValue;
    let azimuth = match action.args.get("azimuth") {
        Some(ScriptActionValue::Float(v)) => *v,
        _ => return,
    };
    let elevation = match action.args.get("elevation") {
        Some(ScriptActionValue::Float(v)) => *v,
        _ => return,
    };
    for mut sun in &mut sun_query {
        sun.azimuth = azimuth;
        sun.elevation = elevation;
    }
}

/// Start Bevy's GPU light-clustering lists big enough for a scene with many
/// point lights.
///
/// Bevy sizes them small (1024 Z slices, 65536 light indices), and only grows
/// them after the GPU has overflowed one and the overflow was read back, so a
/// scene with dozens of lamps (the Bistro has 90) rendered a few frames of
/// wrong lighting on load and again whenever the camera turned to face more of
/// them, each time with a warning. A view's lists are created at these sizes
/// the first time it renders, so this has to run before then. At four bytes an
/// index the larger list is 1 MiB per active view.
fn size_light_clusters(settings: Option<ResMut<bevy::light::cluster::GlobalClusterSettings>>) {
    const Z_SLICES: usize = 4096;
    const INDICES: usize = 262_144;
    let Some(gpu) = settings.and_then(|s| s.into_inner().gpu_clustering.as_mut()) else {
        return;
    };
    gpu.initial_z_slice_list_capacity = gpu.initial_z_slice_list_capacity.max(Z_SLICES);
    gpu.initial_index_list_capacity = gpu.initial_index_list_capacity.max(INDICES);
}

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LightingPlugin");
        app.add_systems(Startup, size_light_clusters);
        app.add_systems(Update, (sync_sun, match_shadow_filtering));
        app.add_observer(handle_sun_script_actions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: compare two Vec3s within float tolerance.
    fn approx_eq(a: Vec3, b: Vec3) {
        let eps = 1e-5;
        assert!(
            (a - b).length() < eps,
            "expected {:?}, got {:?} (diff {})",
            b,
            a,
            (a - b).length(),
        );
    }

    #[test]
    fn direction_north_horizon_points_south() {
        // Azimuth 0 = sun in the north on the horizon. Light travels away
        // from the sun toward -Z.
        let sun = Sun {
            azimuth: 0.0,
            elevation: 0.0,
            ..Sun::default()
        };
        approx_eq(sun.direction(), Vec3::new(0.0, 0.0, -1.0));
    }

    #[test]
    fn direction_overhead_points_straight_down() {
        let sun = Sun {
            azimuth: 0.0,
            elevation: 90.0,
            ..Sun::default()
        };
        approx_eq(sun.direction(), Vec3::new(0.0, -1.0, 0.0));
    }

    #[test]
    fn direction_below_horizon_points_up() {
        let sun = Sun {
            azimuth: 0.0,
            elevation: -90.0,
            ..Sun::default()
        };
        approx_eq(sun.direction(), Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn direction_east_horizon_points_west() {
        // Azimuth 90 = sun in the east on the horizon. Light travels to -X.
        let sun = Sun {
            azimuth: 90.0,
            elevation: 0.0,
            ..Sun::default()
        };
        approx_eq(sun.direction(), Vec3::new(-1.0, 0.0, 0.0));
    }

    #[test]
    fn direction_south_horizon_points_north() {
        // Azimuth 180 = sun in the south on the horizon. Light travels to +Z.
        let sun = Sun {
            azimuth: 180.0,
            elevation: 0.0,
            ..Sun::default()
        };
        approx_eq(sun.direction(), Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn direction_is_always_unit_length() {
        // Spot-check a spread of (az, el) pairs — the spherical
        // construction should always produce a unit vector.
        for az in [0.0, 37.5, 90.0, 175.0, 245.0, 359.0] {
            for el in [-89.9, -45.0, 0.0, 22.5, 60.0, 89.9] {
                let sun = Sun {
                    azimuth: az,
                    elevation: el,
                    ..Sun::default()
                };
                let len = sun.direction().length();
                assert!(
                    (len - 1.0).abs() < 1e-5,
                    "az={} el={} produced direction with length {}",
                    az,
                    el,
                    len,
                );
            }
        }
    }

    #[test]
    fn default_sun_matches_documented_values() {
        // Locked so a future tweak either updates the test or notices it
        // changed the contract. Mid-morning since 2026-11-04 (was 90/25).
        let sun = Sun::default();
        assert_eq!(sun.azimuth, 245.5);
        assert_eq!(sun.elevation, 38.5);
        assert_eq!(sun.illuminance, 120_000.0);
        assert!(sun.shadows_enabled);
        assert!(sun.contact_shadows);
        assert_eq!(sun.angular_diameter, 0.53);
    }

    /// The light is the sun's while it is up and the moon's once it has set,
    /// never both, and a night is lit rather than black.
    #[test]
    fn the_light_hands_over_from_sun_to_moon() {
        let at = |elevation| sun_light(&Sun { elevation, ..Sun::default() });

        let day = at(40.0);
        assert_eq!(day.illuminance, 120_000.0);
        assert!(day.direction.y < 0.0, "sunlight comes down");

        // Just set: the sun has faded and the moon has not yet risen.
        let dusk = at(-1.0);
        assert_eq!(dusk.illuminance, 0.0);
        assert!(!dusk.lit);

        // A full moon, so the phase does not scale the light.
        let night = sun_light(&Sun {
            elevation: -30.0,
            moon_follows_calendar: false,
            moon_day: renzora::sun::SYNODIC_MONTH_DAYS * 0.5,
            ..Sun::default()
        });
        assert!((night.illuminance - Sun::default().moon_illuminance).abs() < 0.01);
        assert!(night.lit);
        assert!(night.direction.y < 0.0, "moonlight comes down, not up from below the ground");

        let unlit = sun_light(&Sun { elevation: -30.0, moon_illuminance: 0.0, ..Sun::default() });
        assert!(!unlit.lit, "a moonlight of 0 keeps nights unlit");
    }

    fn filtering_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, match_shadow_filtering);
        let camera = app.world_mut().spawn(Camera3d::default()).id();
        (app, camera)
    }

    fn filtering(app: &App, camera: Entity) -> ShadowFilteringMethod {
        app.world()
            .get::<ShadowFilteringMethod>(camera)
            .copied()
            .unwrap_or_default()
    }

    #[test]
    fn soft_shadows_switch_cameras_to_temporal_filtering_and_back() {
        let (mut app, camera) = filtering_app();
        let light = app
            .world_mut()
            .spawn(DirectionalLight {
                shadow_maps_enabled: true,
                ..default()
            })
            .id();
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::default());

        app.world_mut()
            .get_mut::<DirectionalLight>(light)
            .unwrap()
            .soft_shadow_size = Some(0.5);
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::Temporal);

        app.world_mut().despawn(light);
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::default());
    }

    /// A light with its shadow maps off casts no shadow at all, soft or not.
    #[test]
    fn a_light_without_shadow_maps_does_not_count() {
        let (mut app, camera) = filtering_app();
        app.world_mut().spawn(PointLight {
            shadow_maps_enabled: false,
            soft_shadows_enabled: true,
            ..default()
        });
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::default());

        app.world_mut().spawn(SpotLight {
            shadow_maps_enabled: true,
            soft_shadows_enabled: true,
            ..default()
        });
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::Temporal);
    }

    #[test]
    fn azimuth_360_equivalent_to_zero() {
        // Wraps because the construction is pure trig — 360° must produce
        // the same direction as 0° within float precision.
        let a = Sun {
            azimuth: 0.0,
            elevation: 30.0,
            ..Sun::default()
        };
        let b = Sun {
            azimuth: 360.0,
            elevation: 30.0,
            ..Sun::default()
        };
        approx_eq(a.direction(), b.direction());
    }
}

renzora::add!(LightingPlugin);
