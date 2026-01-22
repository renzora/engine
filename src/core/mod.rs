mod app_state;
mod components;
mod keybindings;
mod state;

pub use app_state::AppState;
pub use components::{AudioListenerMarker, EditorEntity, MainCamera, SceneNode, SceneTabId, ViewportCamera, WorldEnvironmentMarker};
pub use keybindings::{EditorAction, KeyBinding, KeyBindings, bindable_keys, key_name};
pub use state::{AssetViewMode, DragAxis, EditorState, GizmoMode, HierarchyDropPosition, HierarchyDropTarget, OpenScript, SceneTab, ScriptError, TabCameraState};

use bevy::prelude::*;
use crate::scene_file::SkyMode;

/// Marker for the procedural sky sun light
#[derive(Component)]
pub struct ProceduralSkySun;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorState>()
            .init_resource::<KeyBindings>()
            .add_systems(Update, apply_world_environment.run_if(in_state(AppState::Editor)));
    }
}

/// System that applies WorldEnvironment settings to Bevy resources
fn apply_world_environment(
    mut commands: Commands,
    world_envs: Query<&WorldEnvironmentMarker>,
    mut ambient_light: ResMut<AmbientLight>,
    mut cameras: Query<&mut Camera, With<ViewportCamera>>,
    mut sun_query: Query<(Entity, &mut DirectionalLight, &mut Transform), With<ProceduralSkySun>>,
) {
    // Find the first WorldEnvironment in the scene and apply its settings
    if let Some(world_env) = world_envs.iter().next() {
        let data = &world_env.data;

        // Apply ambient light settings
        ambient_light.color = Color::srgb(
            data.ambient_color.0,
            data.ambient_color.1,
            data.ambient_color.2,
        );
        ambient_light.brightness = data.ambient_brightness;

        // Apply sky settings based on mode
        match data.sky_mode {
            SkyMode::Color => {
                // Simple solid color background
                for mut camera in cameras.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(
                        data.clear_color.0,
                        data.clear_color.1,
                        data.clear_color.2,
                    ));
                }
                // Remove procedural sun if exists
                for (entity, _, _) in sun_query.iter() {
                    commands.entity(entity).despawn();
                }
            }
            SkyMode::Procedural => {
                let sky = &data.procedural_sky;

                // Use sky horizon color as clear color (approximation of procedural sky)
                // A proper implementation would use a skybox shader
                for mut camera in cameras.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(
                        sky.sky_horizon_color.0,
                        sky.sky_horizon_color.1,
                        sky.sky_horizon_color.2,
                    ));
                }

                // Calculate sun direction from azimuth and elevation
                let azimuth_rad = sky.sun_angle_azimuth.to_radians();
                let elevation_rad = sky.sun_angle_elevation.to_radians();

                // Convert spherical to cartesian (Y-up coordinate system)
                let sun_dir = Vec3::new(
                    elevation_rad.cos() * azimuth_rad.sin(),
                    elevation_rad.sin(),
                    elevation_rad.cos() * azimuth_rad.cos(),
                ).normalize();

                // Update or create sun directional light
                if let Some((_, mut light, mut transform)) = sun_query.iter_mut().next() {
                    // Update existing sun
                    light.color = Color::srgb(sky.sun_color.0, sky.sun_color.1, sky.sun_color.2);
                    light.illuminance = sky.sun_energy * 10000.0; // Scale to reasonable illuminance
                    *transform = Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, -sun_dir));
                } else {
                    // Create new sun
                    commands.spawn((
                        DirectionalLight {
                            color: Color::srgb(sky.sun_color.0, sky.sun_color.1, sky.sun_color.2),
                            illuminance: sky.sun_energy * 10000.0,
                            shadows_enabled: true,
                            ..default()
                        },
                        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, -sun_dir)),
                        ProceduralSkySun,
                    ));
                }
            }
            SkyMode::Panorama => {
                // HDR panorama - would need to load and apply the HDR texture as skybox
                // For now, use a neutral gray as placeholder
                for mut camera in cameras.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.3, 0.3, 0.35));
                }
                // Remove procedural sun if exists
                for (entity, _, _) in sun_query.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}
