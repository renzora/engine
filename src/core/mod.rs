mod app_state;
mod components;
mod keybindings;
pub mod resources;

pub use app_state::{AppState, AssetLoadingProgress, format_bytes};
pub use components::{AudioListenerMarker, EditorEntity, MainCamera, SceneNode, SceneTabId, ViewportCamera, WorldEnvironmentMarker};
pub use keybindings::{EditorAction, KeyBinding, KeyBindings, bindable_keys};

// Re-export all resources
pub use resources::{
    AnimationTimelineState, TimelinePlayState,
    AssetBrowserState, AssetViewMode, BottomPanelTab, BuildError, BuildState, ColliderImportType,
    CollisionGizmoVisibility, ConsoleState, ConvertAxes, DefaultCameraEntity, DiagnosticsPlugin, DiagnosticsState,
    DockingState, EditorSettings, RenderStats, CameraSettings,
    ExportLogLevel, ExportLogger, ExportState, GamepadDebugState, GamepadInfo, GamepadButtonState, update_gamepad_debug_state,
    HierarchyDropPosition, HierarchyDropTarget, HierarchyState, InputFocusState, LogEntry, LogLevel, MeshHandling,
    NormalImportMethod, OpenScript, PendingImageDrop, PendingMaterialDrop,
    OrbitCameraState, PlayModeCamera, PlayModeState, PlayState, ProjectionMode, RenderToggles, RightPanelTab, SceneManagerState,
    SceneTab, ScriptError, SelectionState, SettingsTab, TabCameraState, TabKind, TangentImportMethod,
    ThumbnailCache, supports_thumbnail, supports_model_preview,
    ViewportMode, ViewportState, VisualizationMode, WindowState, ResizeEdge,
    // New debug/profiler resources
    EcsStatsState, MemoryProfilerState, MemoryTrend,
    SystemTimingState,
    PhysicsDebugState, ColliderShapeType,
    CameraDebugState, CameraProjectionType,
};

// Re-export gizmo types from the gizmo module (they were moved there)
pub use crate::gizmo::GizmoState;

use bevy::prelude::*;
use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::Exposure;
use bevy::core_pipeline::tonemapping::Tonemapping as BevyTonemapping;
use bevy::core_pipeline::Skybox;
use bevy::pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections};
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::post_process::motion_blur::MotionBlur;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use crate::shared::{SkyMode, TonemappingMode};
use crate::project::CurrentProject;

/// Marker for the procedural sky sun light
#[derive(Component)]
pub struct ProceduralSkySun;

/// Resource to track the currently loaded skybox
#[derive(Resource, Default)]
pub struct SkyboxState {
    /// The path of the currently loaded skybox (to avoid reloading)
    pub current_path: Option<String>,
    /// Handle to the original equirectangular image
    pub equirect_handle: Option<Handle<Image>>,
    /// Handle to the converted cubemap image
    pub cubemap_handle: Option<Handle<Image>>,
    /// Whether the cubemap conversion is pending
    pub conversion_pending: bool,
}

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
            .init_resource::<GamepadDebugState>()
            .init_resource::<SkyboxState>()
            .init_resource::<crate::theming::ThemeManager>()
            .insert_resource(AnimationTimelineState::new())
            .add_systems(Update, (
                apply_world_environment,
                track_asset_loading,
                drain_console_buffer,
                update_gamepad_debug_state,
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
    asset_server: Res<AssetServer>,
    mut skybox_state: ResMut<SkyboxState>,
    current_project: Option<Res<CurrentProject>>,
    mut images: ResMut<Assets<Image>>,
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
            commands.entity(camera_entity).insert(Exposure { ev100: data.ev100 });

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

        // Helper to clear skybox state and remove from cameras
        fn clear_skybox(commands: &mut Commands, skybox_state: &mut SkyboxState, cameras: &Query<Entity, With<ViewportCamera>>) {
            for camera_entity in cameras.iter() {
                commands.entity(camera_entity).remove::<Skybox>();
            }
            skybox_state.current_path = None;
            skybox_state.equirect_handle = None;
            skybox_state.cubemap_handle = None;
            skybox_state.conversion_pending = false;
        }

        // Apply sky/background settings based on mode
        match data.sky_mode {
            SkyMode::Color => {
                // Simple solid color background - remove skybox if present
                clear_skybox(&mut commands, &mut skybox_state, &cameras);
                for mut camera in camera_query.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(
                        data.clear_color.0,
                        data.clear_color.1,
                        data.clear_color.2,
                    ));
                }
            }
            SkyMode::Procedural => {
                let sky = &data.procedural_sky;
                // Use sky horizon color as clear color (approximation of procedural sky)
                clear_skybox(&mut commands, &mut skybox_state, &cameras);
                for mut camera in camera_query.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(
                        sky.sky_horizon_color.0,
                        sky.sky_horizon_color.1,
                        sky.sky_horizon_color.2,
                    ));
                }
            }
            SkyMode::Panorama => {
                let pano = &data.panorama_sky;

                if !pano.panorama_path.is_empty() {
                    let needs_load = skybox_state.current_path.as_ref() != Some(&pano.panorama_path);

                    if needs_load {
                        // Resolve the path - if relative, make it absolute using project path
                        let resolved_path = if std::path::Path::new(&pano.panorama_path).is_absolute() {
                            std::path::PathBuf::from(&pano.panorama_path)
                        } else if let Some(ref project) = current_project {
                            project.path.join(&pano.panorama_path)
                        } else {
                            std::path::PathBuf::from(&pano.panorama_path)
                        };

                        // Load the HDR image
                        let equirect_handle: Handle<Image> = asset_server.load(resolved_path);
                        skybox_state.equirect_handle = Some(equirect_handle);
                        skybox_state.current_path = Some(pano.panorama_path.clone());
                        skybox_state.conversion_pending = true;
                        skybox_state.cubemap_handle = None;

                        info!("Loading HDR panorama for skybox: {}", pano.panorama_path);
                    }

                    // Check if the equirectangular image is loaded and needs conversion
                    if skybox_state.conversion_pending {
                        if let Some(ref equirect_handle) = skybox_state.equirect_handle {
                            if let Some(equirect_image) = images.get(equirect_handle) {
                                // Convert equirectangular to cubemap
                                match equirectangular_to_cubemap(equirect_image) {
                                    Ok(cubemap) => {
                                        let cubemap_handle = images.add(cubemap);
                                        skybox_state.cubemap_handle = Some(cubemap_handle);
                                        skybox_state.conversion_pending = false;
                                        info!("Converted HDR to cubemap successfully");
                                    }
                                    Err(e) => {
                                        error!("Failed to convert HDR to cubemap: {}", e);
                                        skybox_state.conversion_pending = false;
                                    }
                                }
                            }
                        }
                    }

                    // Apply cubemap skybox to cameras if available
                    if let Some(ref cubemap_handle) = skybox_state.cubemap_handle {
                        for camera_entity in cameras.iter() {
                            commands.entity(camera_entity).insert(Skybox {
                                image: cubemap_handle.clone(),
                                brightness: pano.energy * 1000.0,
                                rotation: Quat::from_rotation_y(pano.rotation.to_radians()),
                            });
                        }

                        // Set camera clear color to none (skybox will fill background)
                        for mut camera in camera_query.iter_mut() {
                            camera.clear_color = ClearColorConfig::None;
                        }
                    } else {
                        // Still loading/converting - use dark background
                        for mut camera in camera_query.iter_mut() {
                            camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.15));
                        }
                    }
                } else {
                    // No panorama path set - clear skybox and use neutral gray
                    clear_skybox(&mut commands, &mut skybox_state, &cameras);
                    for mut camera in camera_query.iter_mut() {
                        camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.3, 0.3, 0.35));
                    }
                }
            }
        }

        // Sun directional light - works in ALL sky modes
        // The sun provides scene lighting regardless of background type
        let sky = &data.procedural_sky;
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
            light.illuminance = sky.sun_energy * 10000.0;
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
    } else {
        // No WorldEnvironment - reset to defaults
        ambient_light.color = Color::WHITE;
        ambient_light.brightness = 200.0;

        // Clear skybox state
        skybox_state.current_path = None;
        skybox_state.equirect_handle = None;
        skybox_state.cubemap_handle = None;
        skybox_state.conversion_pending = false;

        // Reset camera settings to defaults
        for camera_entity in cameras.iter() {
            // Reset MSAA to default
            commands.entity(camera_entity).insert(Msaa::Sample4);
            // Remove skybox
            commands.entity(camera_entity).remove::<Skybox>();
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

/// Convert an equirectangular panorama image to a cubemap
///
/// This function takes an equirectangular (lat-long) HDR image and converts it
/// to a cubemap with 6 faces that can be used with Bevy's Skybox component.
fn equirectangular_to_cubemap(equirect: &Image) -> Result<Image, String> {
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    use std::f32::consts::PI;

    // Get the source image dimensions
    let src_width = equirect.width() as usize;
    let src_height = equirect.height() as usize;

    if src_width == 0 || src_height == 0 {
        return Err("Invalid image dimensions".to_string());
    }

    // Determine cubemap face size (typically height/2 for equirectangular)
    let face_size = (src_height / 2).max(256).min(2048);

    // Get bytes per pixel from format
    let bytes_per_pixel = equirect.texture_descriptor.format.block_copy_size(None).unwrap_or(4) as usize;
    let face_data_size = face_size * face_size * bytes_per_pixel;
    let mut cubemap_data = vec![0u8; face_data_size * 6];

    // Get source data - handle Option<Vec<u8>>
    let src_data = equirect.data.as_ref()
        .ok_or_else(|| "Image has no data".to_string())?;

    // Face directions: +X, -X, +Y, -Y, +Z, -Z
    let face_directions: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),   // +X (right)
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),   // -X (left)
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),   // +Y (top)
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),   // -Y (bottom)
        (Vec3::Z, Vec3::Y, Vec3::X),       // +Z (front)
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X), // -Z (back)
    ];

    for (face_idx, (forward, up, right)) in face_directions.iter().enumerate() {
        let face_offset = face_idx * face_data_size;

        for y in 0..face_size {
            for x in 0..face_size {
                // Convert pixel coordinates to [-1, 1] range
                let u = (x as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;

                // Calculate direction vector for this pixel
                let dir = (*forward + *right * u - *up * v).normalize();

                // Convert direction to equirectangular UV coordinates
                let theta = dir.z.atan2(dir.x); // Longitude: -PI to PI
                let phi = dir.y.asin();          // Latitude: -PI/2 to PI/2

                let eq_u = (theta + PI) / (2.0 * PI);
                let eq_v = (phi + PI / 2.0) / PI;

                // Sample the equirectangular image
                let src_x = ((eq_u * src_width as f32) as usize).min(src_width - 1);
                let src_y = (((1.0 - eq_v) * src_height as f32) as usize).min(src_height - 1);

                let src_idx = (src_y * src_width + src_x) * bytes_per_pixel;
                let dst_idx = face_offset + (y * face_size + x) * bytes_per_pixel;

                // Copy pixel data
                if src_idx + bytes_per_pixel <= src_data.len() && dst_idx + bytes_per_pixel <= cubemap_data.len() {
                    cubemap_data[dst_idx..dst_idx + bytes_per_pixel]
                        .copy_from_slice(&src_data[src_idx..src_idx + bytes_per_pixel]);
                }
            }
        }
    }

    // Create the cubemap image using Image::new
    let mut cubemap = Image::new(
        Extent3d {
            width: face_size as u32,
            height: face_size as u32,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        cubemap_data,
        equirect.texture_descriptor.format,
        equirect.asset_usage,
    );

    // Set the texture view descriptor to treat it as a cube
    cubemap.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    Ok(cubemap)
}
