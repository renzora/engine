//! Renzora Lighting — custom lighting components with editor inspectors.
//!
//! Provides `Sun` (directional light via azimuth/elevation), with sync
//! systems that keep the underlying Bevy `DirectionalLight` in lockstep.

use bevy::light::SunDisk;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular::SUN_HORIZON,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

// ============================================================================
// Sun component
// ============================================================================

/// A directional light positioned by azimuth and elevation angles.
///
/// This is a higher-level wrapper around Bevy's `DirectionalLight` that lets
/// you control the sun position with intuitive angle parameters instead of
/// raw quaternion rotation. A sync system automatically updates the
/// `DirectionalLight` and `Transform` each frame.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct Sun {
    /// Azimuth angle in degrees (0–360, compass direction of the sun).
    pub azimuth: f32,
    /// Elevation angle in degrees (−90 to 90, height above horizon).
    pub elevation: f32,
    /// Light color (RGB, 0–1 range).
    pub color: Vec3,
    /// Illuminance in lux.
    pub illuminance: f32,
    /// Whether this light casts shadows.
    pub shadows_enabled: bool,
    /// Angular diameter of the sun disc in degrees (Earth's sun ≈ 0.53°).
    pub angular_diameter: f32,
    /// Brightness multiplier for the sun disk (0 = no disk, 1 = physical, >1 = overexposed).
    pub sun_disk_intensity: f32,
}

impl Default for Sun {
    fn default() -> Self {
        Self {
            azimuth: 90.0,
            elevation: 25.0,
            color: Vec3::new(1.0, 0.95, 0.88),
            illuminance: 40_000.0,
            shadows_enabled: true,
            angular_diameter: 0.53,
            sun_disk_intensity: 1.0,
        }
    }
}

impl Sun {
    /// Compute the direction the light travels (away from the sun toward the scene).
    pub fn direction(&self) -> Vec3 {
        let az = self.azimuth.to_radians();
        let el = self.elevation.to_radians();
        Vec3::new(-el.cos() * az.sin(), -el.sin(), -el.cos() * az.cos())
    }
}

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
    const FADE_START_DEG: f32 = -1.0;
    const FADE_END_DEG: f32 = 5.0;
    ((elevation_deg - FADE_START_DEG) / (FADE_END_DEG - FADE_START_DEG)).clamp(0.0, 1.0)
}

fn sync_sun(
    mut commands: Commands,
    mut query: Query<
        (Entity, &Sun, &mut DirectionalLight, &mut Transform),
        Or<(Changed<Sun>, Without<SunDisk>)>,
    >,
) {
    for (entity, sun, mut light, mut transform) in &mut query {
        // Below the horizon, fade illuminance and disable shadows. Keeps
        // the authored `sun.illuminance` value intact in the inspector
        // while the *effective* light goes to 0 — matches the physical
        // model where the sun is below the horizon and the scene goes
        // dark (until a moon / other key light fills in).
        let factor = sun_horizon_factor(sun.elevation);
        light.color = Color::srgb(sun.color.x, sun.color.y, sun.color.z);
        light.illuminance = sun.illuminance * factor;
        light.shadows_enabled = sun.shadows_enabled && factor > 0.0;
        *transform =
            Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, sun.direction()));
        commands.entity(entity).try_insert(SunDisk {
            angular_size: sun.angular_diameter.to_radians(),
            intensity: sun.sun_disk_intensity * factor,
        });
    }
}

// ============================================================================
// Inspector UI (editor only)
// ============================================================================

#[cfg(feature = "editor")]
fn sun_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(sun) = world.get::<Sun>(entity) else {
        return;
    };

    let mut data = sun.clone();
    let mut changed = false;
    let mut row = 0;

    changed |= inline_property(ui, row, "Azimuth", theme, |ui| {
        ui.add(
            egui::DragValue::new(&mut data.azimuth)
                .speed(1.0)
                .range(0.0..=360.0)
                .suffix("°"),
        )
        .changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Elevation", theme, |ui| {
        ui.add(
            egui::DragValue::new(&mut data.elevation)
                .speed(1.0)
                .range(-90.0..=90.0)
                .suffix("°"),
        )
        .changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Color", theme, |ui| {
        let mut color = egui::Color32::from_rgb(
            (data.color.x * 255.0) as u8,
            (data.color.y * 255.0) as u8,
            (data.color.z * 255.0) as u8,
        );
        let resp = ui.color_edit_button_srgba(&mut color).changed();
        if resp {
            data.color = Vec3::new(
                color.r() as f32 / 255.0,
                color.g() as f32 / 255.0,
                color.b() as f32 / 255.0,
            );
        }
        resp
    });
    row += 1;

    changed |= inline_property(ui, row, "Illuminance", theme, |ui| {
        ui.add(
            egui::DragValue::new(&mut data.illuminance)
                .speed(100.0)
                .range(0.0..=f32::MAX),
        )
        .changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Angular Diameter", theme, |ui| {
        ui.add(
            egui::DragValue::new(&mut data.angular_diameter)
                .speed(0.01)
                .range(0.0..=10.0)
                .suffix("°"),
        )
        .changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Disk Intensity", theme, |ui| {
        ui.add(
            egui::DragValue::new(&mut data.sun_disk_intensity)
                .speed(0.01)
                .range(0.0..=10.0),
        )
        .changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Shadows", theme, |ui| {
        ui.checkbox(&mut data.shadows_enabled, "").changed()
    });

    if changed {
        let new_data = data;
        cmds.push(move |world: &mut World| {
            if let Some(mut sun) = world.get_mut::<Sun>(entity) {
                *sun = new_data;
            }
        });
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "sun",
        display_name: "Sun",
        icon: SUN_HORIZON,
        category: "lighting",
        has_fn: |world, entity| world.get::<Sun>(entity).is_some(),
        add_fn: Some(|world, entity| {
            let data = Sun::default();
            let dir = data.direction();
            world.entity_mut(entity).insert((
                DirectionalLight {
                    color: Color::srgb(data.color.x, data.color.y, data.color.z),
                    illuminance: data.illuminance,
                    shadows_enabled: data.shadows_enabled,
                    ..default()
                },
                Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir)),
                data,
            ));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(Sun, DirectionalLight)>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(sun_custom_ui),
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

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LightingPlugin");
        app.add_systems(Update, sync_sun);
        app.add_observer(handle_sun_script_actions);

        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
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
        // Defaults committed in 420b457 — locking these so a future tweak
        // either updates the test or notices it changed the contract.
        let sun = Sun::default();
        assert_eq!(sun.azimuth, 90.0);
        assert_eq!(sun.elevation, 25.0);
        assert_eq!(sun.illuminance, 40_000.0);
        assert!(sun.shadows_enabled);
        assert_eq!(sun.angular_diameter, 0.53);
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
