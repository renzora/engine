mod app_state;
mod components;
mod keybindings;
pub mod resources;

pub use app_state::{AppState, AssetLoadingProgress, TrackedAsset, format_bytes};
pub use components::{AudioListenerMarker, EditorEntity, MainCamera, SceneNode, SceneTabId, ViewportCamera, WorldEnvironmentMarker};
pub use keybindings::{EditorAction, KeyBinding, KeyBindings, bindable_keys, key_name};

// Re-export all resources
pub use resources::{
    AssetBrowserState, AssetViewMode, EditorSettings, HierarchyDropPosition, HierarchyDropTarget,
    HierarchyState, OpenScript, OrbitCameraState, SceneManagerState, SceneTab, ScriptError,
    SelectionState, TabCameraState, ViewportState, WindowState,
};

// Re-export gizmo types from the gizmo module (they were moved there)
pub use crate::gizmo::{DragAxis, GizmoMode, GizmoState};

use bevy::prelude::*;
use crate::scene_file::SkyMode;

/// Marker for the procedural sky sun light
#[derive(Component)]
pub struct ProceduralSkySun;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        // Initialize split resources
        app.init_resource::<SelectionState>()
            .init_resource::<ViewportState>()
            .init_resource::<OrbitCameraState>()
            .init_resource::<HierarchyState>()
            .init_resource::<WindowState>()
            .init_resource::<SceneManagerState>()
            .init_resource::<AssetBrowserState>()
            .init_resource::<EditorSettings>()
            .init_resource::<KeyBindings>()
            .init_resource::<AssetLoadingProgress>()
            .add_systems(Update, (
                apply_world_environment,
                track_asset_loading,
            ).run_if(in_state(AppState::Editor)));
    }
}

/// System that tracks asset loading progress via AssetServer
fn track_asset_loading(
    asset_server: Res<AssetServer>,
    mut loading_progress: ResMut<AssetLoadingProgress>,
) {
    use bevy::asset::LoadState;

    if loading_progress.tracking.is_empty() {
        loading_progress.loading = false;
        return;
    }

    // Find assets that have finished loading this frame
    let mut finished_ids = Vec::new();
    let mut newly_loaded_bytes = 0u64;

    for (id, info) in loading_progress.tracking.iter() {
        match asset_server.get_load_state(*id) {
            Some(LoadState::Loaded) => {
                newly_loaded_bytes += info.size_bytes;
                finished_ids.push(*id);
            }
            Some(LoadState::Failed(_)) => {
                // Count failed as "loaded" for progress purposes
                newly_loaded_bytes += info.size_bytes;
                finished_ids.push(*id);
            }
            _ => {
                // Still loading or not loaded
            }
        }
    }

    // Update loaded counts with newly finished assets
    loading_progress.loaded += finished_ids.len();
    loading_progress.loaded_bytes += newly_loaded_bytes;

    // Remove finished assets from tracking
    for id in finished_ids {
        loading_progress.tracking.remove(&id);
    }

    // Update loading state
    loading_progress.loading = !loading_progress.tracking.is_empty();

    // Reset when done loading
    if !loading_progress.loading {
        loading_progress.loaded_bytes = 0;
        loading_progress.total_bytes = 0;
        loading_progress.loaded = 0;
        loading_progress.total = 0;
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
