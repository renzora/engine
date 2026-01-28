mod app_state;
mod components;
mod keybindings;
pub mod resources;

pub use app_state::{AppState, AssetLoadingProgress, format_bytes};
pub use components::{AudioListenerMarker, EditorEntity, MainCamera, SceneNode, SceneTabId, ViewportCamera, WorldEnvironmentMarker};
pub use keybindings::{EditorAction, KeyBinding, KeyBindings, bindable_keys};

// Re-export all resources
pub use resources::{
    AnimationTimelineState,
    AssetBrowserState, AssetViewMode, BottomPanelTab, BuildError, BuildState, ColliderImportType,
    CollisionGizmoVisibility, ConsoleState, ConvertAxes, DefaultCameraEntity, DockingState, EditorSettings,
    ExportLogLevel, ExportLogger, ExportState,
    HierarchyDropPosition, HierarchyDropTarget, HierarchyState, InputFocusState, LogEntry, LogLevel, MeshHandling,
    NormalImportMethod, OpenScript, PendingImageDrop,
    OrbitCameraState, PlayModeCamera, PlayModeState, PlayState, RenderToggles, RightPanelTab, SceneManagerState,
    SceneTab, ScriptError, SelectionState, SettingsTab, TabCameraState, TabKind, TangentImportMethod,
    ThumbnailCache, supports_thumbnail, supports_model_preview,
    ViewportMode, ViewportState, VisualizationMode, WindowState, ResizeEdge,
};

// Re-export gizmo types from the gizmo module (they were moved there)
pub use crate::gizmo::GizmoState;

use bevy::prelude::*;
use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping as BevyTonemapping;
use bevy::pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections};
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::post_process::motion_blur::MotionBlur;
use crate::shared::{SkyMode, TonemappingMode};

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
            .init_resource::<ExportState>()
            .init_resource::<DefaultCameraEntity>()
            .init_resource::<PlayModeState>()
            .init_resource::<ConsoleState>()
            .init_resource::<ThumbnailCache>()
            .init_resource::<DockingState>()
            .init_resource::<InputFocusState>()
            .init_resource::<crate::theming::ThemeManager>()
            .insert_resource(AnimationTimelineState::new())
            .add_systems(Update, (
                apply_world_environment,
                track_asset_loading,
                drain_console_buffer,
            ).run_if(in_state(AppState::Editor)));
    }
}

/// System to drain the global console buffer into the ConsoleState resource
fn drain_console_buffer(
    mut console: ResMut<ConsoleState>,
    time: Res<Time>,
) {
    console.drain_shared_buffer(time.elapsed_secs_f64());
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
    world_envs: Query<(&WorldEnvironmentMarker, &EditorEntity)>,
    mut ambient_light: ResMut<GlobalAmbientLight>,
    cameras: Query<Entity, With<ViewportCamera>>,
    mut camera_query: Query<&mut Camera, With<ViewportCamera>>,
    mut sun_query: Query<(Entity, &mut DirectionalLight, &mut Transform), With<ProceduralSkySun>>,
) {
    // Find the first visible WorldEnvironment in the scene and apply its settings
    if let Some((world_env, _)) = world_envs.iter().find(|(_, editor)| editor.visible) {
        let data = &world_env.data;

        // Apply ambient light settings
        ambient_light.color = Color::srgb(
            data.ambient_color.0,
            data.ambient_color.1,
            data.ambient_color.2,
        );
        ambient_light.brightness = data.ambient_brightness;

        // Apply post-processing to cameras
        for camera_entity in cameras.iter() {
            // MSAA (component-based in Bevy 0.17)
            let msaa = match data.msaa_samples {
                1 => Msaa::Off,
                2 => Msaa::Sample2,
                4 => Msaa::Sample4,
                8 => Msaa::Sample8,
                _ => Msaa::Sample4,
            };
            commands.entity(camera_entity).insert(msaa);
            // Fog
            if data.fog_enabled {
                commands.entity(camera_entity).insert(DistanceFog {
                    color: Color::srgba(data.fog_color.0, data.fog_color.1, data.fog_color.2, 1.0),
                    falloff: FogFalloff::Linear {
                        start: data.fog_start,
                        end: data.fog_end,
                    },
                    ..default()
                });
            } else {
                commands.entity(camera_entity).remove::<DistanceFog>();
            }

            // FXAA
            if data.fxaa_enabled {
                commands.entity(camera_entity).insert(Fxaa::default());
            } else {
                commands.entity(camera_entity).remove::<Fxaa>();
            }

            // Bloom
            if data.bloom_enabled {
                commands.entity(camera_entity).insert(Bloom {
                    intensity: data.bloom_intensity,
                    low_frequency_boost: data.bloom_threshold * 0.5,
                    ..default()
                });
            } else {
                commands.entity(camera_entity).remove::<Bloom>();
            }

            // Tonemapping
            let bevy_tonemap = match data.tonemapping {
                TonemappingMode::None => BevyTonemapping::None,
                TonemappingMode::Reinhard => BevyTonemapping::Reinhard,
                TonemappingMode::ReinhardLuminance => BevyTonemapping::ReinhardLuminance,
                TonemappingMode::AcesFitted => BevyTonemapping::AcesFitted,
                TonemappingMode::AgX => BevyTonemapping::AgX,
                TonemappingMode::SomewhatBoringDisplayTransform => BevyTonemapping::SomewhatBoringDisplayTransform,
                TonemappingMode::TonyMcMapface => BevyTonemapping::TonyMcMapface,
                TonemappingMode::BlenderFilmic => BevyTonemapping::BlenderFilmic,
            };
            commands.entity(camera_entity).insert(bevy_tonemap);

            // Exposure
            commands.entity(camera_entity).insert(Exposure { ev100: data.exposure });

            // SSAO
            if data.ssao_enabled {
                commands.entity(camera_entity).insert(ScreenSpaceAmbientOcclusion::default());
            } else {
                commands.entity(camera_entity).remove::<ScreenSpaceAmbientOcclusion>();
            }

            // SSR
            if data.ssr_enabled {
                commands.entity(camera_entity).insert(ScreenSpaceReflections::default());
            } else {
                commands.entity(camera_entity).remove::<ScreenSpaceReflections>();
            }

            // Depth of Field
            if data.dof_enabled {
                commands.entity(camera_entity).insert(DepthOfField {
                    focal_distance: data.dof_focal_distance,
                    aperture_f_stops: data.dof_aperture,
                    mode: DepthOfFieldMode::Bokeh,
                    ..default()
                });
            } else {
                commands.entity(camera_entity).remove::<DepthOfField>();
            }

            // Motion Blur
            if data.motion_blur_enabled {
                commands.entity(camera_entity).insert(MotionBlur {
                    shutter_angle: data.motion_blur_intensity * 360.0,
                    samples: 4,
                });
            } else {
                commands.entity(camera_entity).remove::<MotionBlur>();
            }
        }

        // Apply sky settings based on mode
        match data.sky_mode {
            SkyMode::Color => {
                // Simple solid color background
                for mut camera in camera_query.iter_mut() {
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
                for mut camera in camera_query.iter_mut() {
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
                // Note: Full panorama skybox support requires loading KTX2/HDR files
                // and using the Skybox component, which is complex due to asset loading
                for mut camera in camera_query.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.3, 0.3, 0.35));
                }
                // Remove procedural sun if exists
                for (entity, _, _) in sun_query.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    } else {
        // No WorldEnvironment - reset to defaults
        ambient_light.color = Color::WHITE;
        ambient_light.brightness = 200.0;

        // Reset camera settings to defaults
        for camera_entity in cameras.iter() {
            // Reset MSAA to default
            commands.entity(camera_entity).insert(Msaa::Sample4);
            // Remove all post-processing components
            commands.entity(camera_entity).remove::<DistanceFog>();
            commands.entity(camera_entity).remove::<Fxaa>();
            commands.entity(camera_entity).remove::<Bloom>();
            commands.entity(camera_entity).remove::<ScreenSpaceAmbientOcclusion>();
            commands.entity(camera_entity).remove::<ScreenSpaceReflections>();
            commands.entity(camera_entity).remove::<DepthOfField>();
            commands.entity(camera_entity).remove::<MotionBlur>();

            // Reset tonemapping and exposure to defaults
            commands.entity(camera_entity).insert(BevyTonemapping::Reinhard);
            commands.entity(camera_entity).insert(Exposure::default());
        }

        // Reset camera clear color to default dark gray
        for mut camera in camera_query.iter_mut() {
            camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12));
        }

        // Remove procedural sun if exists
        for (entity, _, _) in sun_query.iter() {
            commands.entity(entity).despawn();
        }
    }
}
