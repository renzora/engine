//! Renzora Lighting — custom lighting components with editor inspectors.
//!
//! Provides `Sun` (directional light via azimuth/elevation), with sync
//! systems that keep the underlying Bevy `DirectionalLight` in lockstep.

use bevy::prelude::*;
use bevy::light::SunDisk;
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
            azimuth: 0.0,
            elevation: 50.0,
            color: Vec3::new(1.0, 0.96, 0.90),
            illuminance: 100_000.0,
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
        Vec3::new(
            -el.cos() * az.sin(),
            -el.sin(),
            -el.cos() * az.cos(),
        )
    }
}

// ============================================================================
// Sync system — keeps DirectionalLight + Transform in sync with Sun
// ============================================================================

fn sync_sun(
    mut commands: Commands,
    mut query: Query<(Entity, &Sun, &mut DirectionalLight, &mut Transform), Or<(Changed<Sun>, Without<SunDisk>)>>,
) {
    for (entity, sun, mut light, mut transform) in &mut query {
        light.color = Color::srgb(sun.color.x, sun.color.y, sun.color.z);
        light.illuminance = sun.illuminance;
        light.shadows_enabled = sun.shadows_enabled;
        *transform = Transform::from_rotation(
            Quat::from_rotation_arc(Vec3::NEG_Z, sun.direction()),
        );
        commands.entity(entity).try_insert(SunDisk {
            angular_size: sun.angular_diameter.to_radians(),
            intensity: sun.sun_disk_intensity,
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

pub struct LightingPlugin;

/// Apply pending sun angle commands from scripting.
fn apply_script_sun_commands(
    mut sun_query: Query<&mut Sun>,
    mut env_cmds: ResMut<renzora_scripting::systems::ScriptEnvironmentCommands>,
) {
    if let Some((azimuth, elevation)) = env_cmds.sun_angles.take() {
        for mut sun in &mut sun_query {
            sun.azimuth = azimuth;
            sun.elevation = elevation;
        }
    }
}

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] LightingPlugin");
        app.add_systems(Update, (sync_sun, apply_script_sun_commands));

        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
