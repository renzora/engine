#![allow(dead_code)]

mod camera;
mod camera_preview;
pub mod camera2d;
pub mod grid2d;
pub mod model_preview;
pub mod particle_preview;
pub mod render_2d;
pub mod studio_preview;
mod texture;

pub use camera::{apply_orbit_to_camera, camera_controller, camera_focus_selected, update_camera_projection};
pub use camera_preview::{
    setup_camera_preview_texture, update_camera_preview, CameraPreviewImage,
};
pub use camera2d::{
    camera2d_controller, setup_editor_camera_2d, toggle_viewport_cameras,
};
pub use grid2d::draw_grid_2d;
pub use model_preview::{
    capture_model_previews, cleanup_orphaned_preview_entities, process_model_preview_queue,
    register_model_preview_textures, spawn_model_previews, ModelPreviewCache,
};
pub use render_2d::{cleanup_2d_visuals, update_2d_visuals};
pub use studio_preview::{
    StudioPreviewImage, StudioPreviewOrbit, StudioPreviewCamera, StudioPreviewLight,
    StudioPreviewPlugin, STUDIO_RENDER_LAYER,
};
pub use particle_preview::{
    ParticlePreviewImage, ParticlePreviewOrbit, ParticlePreviewCamera,
    ParticlePreviewPlugin, PARTICLE_PREVIEW_LAYER,
};
pub use texture::{resize_viewport_texture, setup_viewport_texture, ViewportTextureSize};

use bevy::prelude::*;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
#[cfg(feature = "solari")]
use bevy::solari::realtime::SolariLighting;
#[cfg(feature = "solari")]
use bevy::solari::scene::RaytracingMesh3d;
#[cfg(feature = "solari")]
use bevy::anti_alias::dlss::{Dlss, DlssRayReconstructionFeature};
use std::collections::HashMap;
use std::marker::PhantomData;

use crate::component_system::components::clouds::CloudDomeMarker;
use crate::console_info;
use crate::core::{AppState, DockingState, EditorSettings, MainCamera, RenderToggles, SelectionState, ViewportState, VisualizationMode};
use crate::gizmo::meshes::GizmoMesh;
use crate::gizmo::GizmoOverlayCamera;
#[cfg(feature = "solari")]
use crate::shared::{DlssQualityMode, SolariLightingData};
use crate::spawn::{EditorSceneRoot, SceneType};
use crate::blueprint::preview::MaterialPreviewCamera;
use crate::ui::docking::PanelId;
use crate::shared::{
    Camera2DData, CameraNodeData, CameraRigData, CollisionShapeData, MeshInstanceData,
    MeshNodeData, PhysicsBodyData, Sprite2DData, UIButtonData, UIImageData, UILabelData,
    UIPanelData,
};

/// Current viewport mode (2D or 3D view)
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum ViewportMode {
    #[default]
    Mode3D,
    Mode2D,
}

/// State for the 2D camera controller
#[derive(Resource)]
pub struct Camera2DState {
    /// Pan offset in world units
    pub pan_offset: Vec2,
    /// Zoom level (1.0 = 100%, 0.5 = 50%, 2.0 = 200%)
    pub zoom: f32,
    /// Whether the camera is currently panning
    pub is_panning: bool,
    /// Last mouse position for delta calculation
    pub last_mouse_pos: Vec2,
}

impl Default for Camera2DState {
    fn default() -> Self {
        Self {
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            is_panning: false,
            last_mouse_pos: Vec2::ZERO,
        }
    }
}

#[derive(Resource)]
pub struct ViewportImage(pub Handle<Image>);

/// Stores original material properties so they can be restored when switching render modes
#[derive(Resource, Default)]
pub struct OriginalMaterialStates {
    states: HashMap<AssetId<StandardMaterial>, MaterialState>,
}

#[derive(Clone)]
struct MaterialState {
    unlit: bool,
    base_color: Color,
    base_color_texture: Option<Handle<Image>>,
    emissive_texture: Option<Handle<Image>>,
    normal_map_texture: Option<Handle<Image>>,
    metallic_roughness_texture: Option<Handle<Image>>,
    occlusion_texture: Option<Handle<Image>>,
    alpha_mode: AlphaMode,
    metallic: f32,
    perceptual_roughness: f32,
}

/// Tracks the last applied render state to detect changes
#[derive(Resource, Default)]
struct LastRenderState {
    toggles: Option<RenderToggles>,
    visualization: Option<VisualizationMode>,
}

pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WireframePlugin::default())
            .insert_resource(WireframeConfig {
                global: false,
                default_color: bevy::color::Color::WHITE,
            })
            .init_resource::<OriginalMaterialStates>()
            .init_resource::<LastRenderState>()
            .init_resource::<Camera2DState>()
            .init_resource::<ModelPreviewCache>()
            .add_systems(Startup, (setup_viewport_texture, setup_camera_preview_texture))
            .add_systems(
                Update,
                (update_render_toggles, update_shadow_settings).run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (auto_switch_viewport_mode, sync_layout_camera_settings, sync_camera_activity).run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                PostUpdate,
                sync_gizmo_overlay_camera.run_if(in_state(AppState::Editor)),
            )
            // Model preview systems for asset browser thumbnails
            .add_systems(
                Update,
                (
                    process_model_preview_queue,
                    spawn_model_previews,
                    capture_model_previews,
                    cleanup_orphaned_preview_entities,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );

        // Solari raytraced lighting sync (requires solari feature + SDKs)
        #[cfg(feature = "solari")]
        {
            app.init_resource::<SolariState>()
                .add_systems(
                    Update,
                    (
                        sync_rendering_settings,
                        debug_solari_particles,
                    ).run_if(in_state(AppState::Editor)),
                );
        }
    }
}

/// System to automatically switch viewport mode based on selected entity type
fn auto_switch_viewport_mode(
    selection: Res<SelectionState>,
    mut viewport: ResMut<ViewportState>,
    // 2D/UI entities (data components)
    entities_2d: Query<
        (),
        Or<(
            With<Sprite2DData>,
            With<Camera2DData>,
            With<UIPanelData>,
            With<UILabelData>,
            With<UIButtonData>,
            With<UIImageData>,
        )>,
    >,
    // 3D entities (data components)
    entities_3d: Query<
        (),
        Or<(
            With<MeshNodeData>,
            With<MeshInstanceData>,
            With<CameraNodeData>,
            With<CameraRigData>,
            With<PointLight>,
            With<DirectionalLight>,
            With<SpotLight>,
            With<PhysicsBodyData>,
            With<CollisionShapeData>,
        )>,
    >,
    // EditorSceneRoot query to check scene type
    scene_roots: Query<&EditorSceneRoot>,
) {
    // Only check when selection changes
    if !selection.is_changed() {
        return;
    }

    let Some(entity) = selection.selected_entity else {
        return;
    };

    // Check for SceneRoot first - it determines the scene type
    if let Ok(scene_root) = scene_roots.get(entity) {
        match scene_root.scene_type {
            SceneType::Scene2D | SceneType::UI => {
                if viewport.viewport_mode != ViewportMode::Mode2D {
                    viewport.viewport_mode = ViewportMode::Mode2D;
                }
            }
            SceneType::Scene3D | SceneType::Other => {
                if viewport.viewport_mode != ViewportMode::Mode3D {
                    viewport.viewport_mode = ViewportMode::Mode3D;
                }
            }
        }
        return;
    }

    // Check for 2D/UI data components
    let is_2d = entities_2d.get(entity).is_ok();

    // Check for 3D data components
    let is_3d = entities_3d.get(entity).is_ok();

    // Switch viewport mode based on entity type
    if is_2d && viewport.viewport_mode != ViewportMode::Mode2D {
        viewport.viewport_mode = ViewportMode::Mode2D;
    } else if is_3d && viewport.viewport_mode != ViewportMode::Mode3D {
        viewport.viewport_mode = ViewportMode::Mode3D;
    }
}

/// Syncs viewport settings based on the active layout.
/// Disables left-click camera drag in terrain layout to allow brush tools to work.
pub fn sync_layout_camera_settings(
    docking: Res<DockingState>,
    mut viewport: ResMut<ViewportState>,
) {
    if !docking.is_changed() {
        return;
    }

    // Disable left-click camera drag in terrain layout for brush tools
    viewport.disable_left_click_drag = docking.active_layout == "Terrain";
}

/// System to update rendering based on render toggles and visualization mode
fn update_render_toggles(
    settings: Res<EditorSettings>,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut original_states: ResMut<OriginalMaterialStates>,
    mut last_state: ResMut<LastRenderState>,
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,
) {
    let current_toggles = settings.render_toggles;
    let current_viz = settings.visualization_mode;

    let toggles_changed = last_state.toggles != Some(current_toggles);
    let viz_changed = last_state.visualization != Some(current_viz);
    let state_changed = toggles_changed || viz_changed;

    // Check if new materials were added
    let new_materials_added = material_events.read().any(|e| matches!(e, AssetEvent::Added { .. }));

    // Determine if we're in a "modified" state (not default rendering)
    let is_modified_state = !current_toggles.textures
        || !current_toggles.lighting
        || current_toggles.wireframe
        || current_viz != VisualizationMode::None;

    // Only update if state changed or new materials were added in a modified state
    if !state_changed && !(new_materials_added && is_modified_state) {
        return;
    }

    if state_changed {
        last_state.toggles = Some(current_toggles);
        last_state.visualization = Some(current_viz);

        // Update wireframe config
        wireframe_config.global = current_toggles.wireframe;

        // If this is the first change from default state, capture all material states
        if original_states.states.is_empty() && is_modified_state {
            capture_material_states(&materials, &mut original_states);
        }
    }

    // If we're back to default state, restore original materials
    let is_default = current_toggles.textures
        && current_toggles.lighting
        && !current_toggles.wireframe
        && current_viz == VisualizationMode::None;

    if is_default {
        restore_material_states(&mut materials, &original_states);
        return;
    }

    // Apply current render state to all materials
    for (id, material) in materials.iter_mut() {
        // Capture state if not already captured
        if !original_states.states.contains_key(&id) {
            original_states.states.insert(id, MaterialState {
                unlit: material.unlit,
                base_color: material.base_color,
                base_color_texture: material.base_color_texture.clone(),
                emissive_texture: material.emissive_texture.clone(),
                normal_map_texture: material.normal_map_texture.clone(),
                metallic_roughness_texture: material.metallic_roughness_texture.clone(),
                occlusion_texture: material.occlusion_texture.clone(),
                alpha_mode: material.alpha_mode,
                metallic: material.metallic,
                perceptual_roughness: material.perceptual_roughness,
            });
        }

        // Get the original state for this material
        let original = original_states.states.get(&id).cloned();

        // Start from original values
        if let Some(ref orig) = original {
            material.base_color = orig.base_color;
            material.base_color_texture = orig.base_color_texture.clone();
            material.emissive_texture = orig.emissive_texture.clone();
            material.normal_map_texture = orig.normal_map_texture.clone();
            material.metallic_roughness_texture = orig.metallic_roughness_texture.clone();
            material.occlusion_texture = orig.occlusion_texture.clone();
            material.alpha_mode = orig.alpha_mode;
            material.unlit = orig.unlit;
            material.metallic = orig.metallic;
            material.perceptual_roughness = orig.perceptual_roughness;
        }

        // Apply visualization mode (takes priority)
        match current_viz {
            VisualizationMode::None => {
                // Apply toggles only
            }
            VisualizationMode::Normals => {
                // Show a placeholder color - proper normals need a custom shader
                // For now, use a neutral gray to indicate this mode is active
                material.base_color = Color::srgb(0.5, 0.5, 1.0);
                material.base_color_texture = None;
                material.unlit = true;
            }
            VisualizationMode::Roughness => {
                // Visualize roughness as grayscale
                let roughness = if let Some(ref orig) = original {
                    orig.perceptual_roughness
                } else {
                    material.perceptual_roughness
                };
                material.base_color = Color::srgb(roughness, roughness, roughness);
                material.base_color_texture = None;
                material.unlit = true;
            }
            VisualizationMode::Metallic => {
                // Visualize metallic as grayscale
                let metallic = if let Some(ref orig) = original {
                    orig.metallic
                } else {
                    material.metallic
                };
                material.base_color = Color::srgb(metallic, metallic, metallic);
                material.base_color_texture = None;
                material.unlit = true;
            }
            VisualizationMode::Depth => {
                // Depth visualization needs a custom shader
                // Placeholder: dark blue tint
                material.base_color = Color::srgb(0.1, 0.1, 0.3);
                material.base_color_texture = None;
                material.unlit = true;
            }
            VisualizationMode::UvChecker => {
                // UV checker needs a checker texture
                // Placeholder: magenta to indicate missing texture
                material.base_color = Color::srgb(1.0, 0.0, 1.0);
                material.base_color_texture = None;
                material.unlit = true;
            }
        }

        // Apply toggles (if no visualization mode active, or for wireframe)
        if current_viz == VisualizationMode::None {
            // Textures toggle
            if !current_toggles.textures {
                material.base_color_texture = None;
                material.emissive_texture = None;
                material.normal_map_texture = None;
                material.metallic_roughness_texture = None;
                material.occlusion_texture = None;
            }

            // Lighting toggle
            if !current_toggles.lighting {
                material.unlit = true;
            }
        }

        // Wireframe: make materials transparent when wireframe is on and textures are off
        if current_toggles.wireframe && !current_toggles.textures && current_viz == VisualizationMode::None {
            material.base_color = Color::srgba(0.0, 0.0, 0.0, 0.0);
            material.alpha_mode = AlphaMode::Blend;
        }
    }
}

/// System to update shadow settings on lights
fn update_shadow_settings(
    settings: Res<EditorSettings>,
    mut directional_lights: Query<&mut DirectionalLight>,
    mut point_lights: Query<&mut PointLight>,
    mut spot_lights: Query<&mut SpotLight>,
) {
    if !settings.is_changed() {
        return;
    }

    let shadows_enabled = settings.render_toggles.shadows;

    for mut light in directional_lights.iter_mut() {
        light.shadows_enabled = shadows_enabled;
    }

    for mut light in point_lights.iter_mut() {
        light.shadows_enabled = shadows_enabled;
    }

    for mut light in spot_lights.iter_mut() {
        light.shadows_enabled = shadows_enabled;
    }
}

/// Capture the current state of all materials
fn capture_material_states(
    materials: &Assets<StandardMaterial>,
    original_states: &mut OriginalMaterialStates,
) {
    original_states.states.clear();
    for (id, material) in materials.iter() {
        original_states.states.insert(id, MaterialState {
            unlit: material.unlit,
            base_color: material.base_color,
            base_color_texture: material.base_color_texture.clone(),
            emissive_texture: material.emissive_texture.clone(),
            normal_map_texture: material.normal_map_texture.clone(),
            metallic_roughness_texture: material.metallic_roughness_texture.clone(),
            occlusion_texture: material.occlusion_texture.clone(),
            alpha_mode: material.alpha_mode,
            metallic: material.metallic,
            perceptual_roughness: material.perceptual_roughness,
        });
    }
}

/// Restore materials to their original states
fn restore_material_states(
    materials: &mut Assets<StandardMaterial>,
    original_states: &OriginalMaterialStates,
) {
    for (id, state) in &original_states.states {
        if let Some(material) = materials.get_mut(*id) {
            material.unlit = state.unlit;
            material.base_color = state.base_color;
            material.base_color_texture = state.base_color_texture.clone();
            material.emissive_texture = state.emissive_texture.clone();
            material.normal_map_texture = state.normal_map_texture.clone();
            material.metallic_roughness_texture = state.metallic_roughness_texture.clone();
            material.occlusion_texture = state.occlusion_texture.clone();
            material.alpha_mode = state.alpha_mode;
            material.metallic = state.metallic;
            material.perceptual_roughness = state.perceptual_roughness;
        }
    }
}

/// Resource to track whether Solari is currently active
#[cfg(feature = "solari")]
#[derive(Resource, Default)]
pub struct SolariState {
    pub enabled: bool,
}

/// System to sync SolariLightingData component to the main camera.
/// Handles adding/removing SolariLighting, DLSS, and RaytracingMesh3d.
/// The viewport texture format and Hdr/CameraMainTextureUsages are set at startup
/// (conditionally compiled via `cfg(feature = "solari")`) and do NOT change at runtime.
#[cfg(feature = "solari")]
fn sync_rendering_settings(
    mut commands: Commands,
    solari_query: Query<&SolariLightingData, Or<(Changed<SolariLightingData>, Changed<crate::core::DisabledComponents>)>>,
    solari_all: Query<(&SolariLightingData, Option<&crate::core::DisabledComponents>)>,
    camera_query: Query<
        (Entity, Option<&SolariLighting>, Option<&Dlss<DlssRayReconstructionFeature>>),
        With<MainCamera>,
    >,
    meshes_without_rt: Query<(Entity, &Mesh3d), (Without<RaytracingMesh3d>, Without<GizmoMesh>, Without<CloudDomeMarker>)>,
    meshes_with_rt: Query<Entity, With<RaytracingMesh3d>>,
    mut solari_state: ResMut<SolariState>,
    new_meshes: Query<Entity, Added<Mesh3d>>,
    mut logged_startup: Local<bool>,
) {
    // Log startup state once
    if !*logged_startup {
        console_info!("Solari", "=== RENDERING SETTINGS SYNC INITIALIZED ===");
        let solari_data_count = solari_all.iter().count();
        let enabled_count = solari_all.iter().filter(|(_, dc)| !dc.map_or(false, |d| d.is_disabled("solari_lighting"))).count();
        let mesh_count = meshes_without_rt.iter().count() + meshes_with_rt.iter().count();
        console_info!("Solari", "SolariLightingData entities: {} (enabled: {})", solari_data_count, enabled_count);
        console_info!("Solari", "Total meshes in scene: {}", mesh_count);
        console_info!("Solari", "Current Solari state: {}", solari_state.enabled);
        *logged_startup = true;
    }

    // Early out: if no SolariLightingData changed and no new meshes, skip most work
    let has_changes = !solari_query.is_empty();
    let has_new_meshes = !new_meshes.is_empty();

    if has_new_meshes {
        let new_mesh_count = new_meshes.iter().count();
        console_info!("Solari", "Detected {} new Mesh3d entities", new_mesh_count);
    }

    // If nothing changed and Solari is disabled, skip entirely
    if !has_changes && !has_new_meshes && !solari_state.enabled {
        return;
    }

    let Ok((camera_entity, has_solari, has_dlss)) = camera_query.single() else {
        console_info!("Solari", "WARNING: No MainCamera found!");
        return;
    };

    // Find the first enabled SolariLightingData in the scene
    let active_settings = solari_all.iter().find(|(_, dc)| {
        !dc.map_or(false, |d| d.is_disabled("solari_lighting"))
    }).map(|(s, _)| s);
    let should_enable = active_settings.is_some();

    let state_changed = solari_state.enabled != should_enable;

    if has_changes && state_changed {
        console_info!("Solari", "SolariLightingData changed:");
        console_info!("Solari", "  should_enable={} state_changed={}", should_enable, state_changed);
        console_info!("Solari", "  camera has SolariLighting: {}", has_solari.is_some());
        console_info!("Solari", "  camera has DLSS: {}", has_dlss.is_some());
    }

    match active_settings {
        Some(settings) => {
            // Solari should be enabled
            if state_changed {
                console_info!("Solari", "=== ENABLING SOLARI RAYTRACED LIGHTING ===");
                console_info!("Solari", "DLSS settings: enabled={} quality={:?}", settings.dlss_enabled, settings.dlss_quality);

                // Add SolariLighting to the camera (Hdr + CameraMainTextureUsages already present from setup)
                console_info!("Solari", "Adding SolariLighting to camera");
                commands.entity(camera_entity).insert(SolariLighting::default());

                // Add RaytracingMesh3d to all existing meshes
                let mut count = 0;
                for (entity, mesh3d) in meshes_without_rt.iter() {
                    commands.entity(entity).insert(RaytracingMesh3d(mesh3d.0.clone()));
                    count += 1;
                }
                console_info!("Solari", "Added RaytracingMesh3d to {} meshes", count);
                console_info!("Solari", "=== SOLARI ENABLED ===");
            } else if has_new_meshes && solari_state.enabled {
                // Solari already enabled, just add RaytracingMesh3d to new meshes
                let mut count = 0;
                for (entity, mesh3d) in meshes_without_rt.iter() {
                    commands.entity(entity).insert(RaytracingMesh3d(mesh3d.0.clone()));
                    count += 1;
                }
                if count > 0 {
                    console_info!("Solari", "Added RaytracingMesh3d to {} new meshes (Solari already active)", count);
                }
            }

            // Handle DLSS (only check when SolariLightingData changed)
            if has_changes {
                if settings.dlss_enabled {
                    let dlss_quality = match settings.dlss_quality {
                        DlssQualityMode::Auto => bevy::anti_alias::dlss::DlssPerfQualityMode::Auto,
                        DlssQualityMode::Dlaa => bevy::anti_alias::dlss::DlssPerfQualityMode::Dlaa,
                        DlssQualityMode::Quality => bevy::anti_alias::dlss::DlssPerfQualityMode::Quality,
                        DlssQualityMode::Balanced => bevy::anti_alias::dlss::DlssPerfQualityMode::Balanced,
                        DlssQualityMode::Performance => bevy::anti_alias::dlss::DlssPerfQualityMode::Performance,
                        DlssQualityMode::UltraPerformance => bevy::anti_alias::dlss::DlssPerfQualityMode::UltraPerformance,
                    };

                    if has_dlss.is_none() {
                        console_info!("Solari", "Enabling DLSS Ray Reconstruction: quality={:?}", dlss_quality);
                        commands.entity(camera_entity).insert(Dlss::<DlssRayReconstructionFeature> {
                            perf_quality_mode: dlss_quality,
                            reset: false,
                            _phantom_data: PhantomData,
                        });
                    }
                } else if has_dlss.is_some() {
                    console_info!("Solari", "Disabling DLSS Ray Reconstruction");
                    commands.entity(camera_entity).remove::<Dlss<DlssRayReconstructionFeature>>();
                }
            }

            solari_state.enabled = true;
        }
        None => {
            // No enabled SolariLightingData - disable Solari
            if state_changed && solari_state.enabled {
                console_info!("Solari", "=== DISABLING SOLARI RAYTRACED LIGHTING ===");

                // Remove Solari-specific camera components (keep Hdr + CameraMainTextureUsages - standard PBR works fine with HDR)
                console_info!("Solari", "Removing SolariLighting and DLSS from camera");
                commands.entity(camera_entity)
                    .remove::<SolariLighting>()
                    .remove::<Dlss<DlssRayReconstructionFeature>>();

                // Remove RaytracingMesh3d from all meshes
                let mut count = 0;
                for entity in meshes_with_rt.iter() {
                    commands.entity(entity).remove::<RaytracingMesh3d>();
                    count += 1;
                }
                console_info!("Solari", "Removed RaytracingMesh3d from {} meshes", count);
                console_info!("Solari", "=== SOLARI DISABLED (STANDARD RENDERING WITH HDR) ===");
            }

            solari_state.enabled = false;
        }
    }
}

/// Debug system to log Solari + particle state when Solari is active
#[cfg(feature = "solari")]
fn debug_solari_particles(
    time: Res<Time>,
    mut last_log: Local<f32>,
    solari_state: Res<SolariState>,
    camera_query: Query<
        (Entity, &Camera, Option<&bevy::core_pipeline::prepass::DeferredPrepass>, Option<&bevy::core_pipeline::prepass::DepthPrepass>),
        (With<MainCamera>, With<SolariLighting>),
    >,
    particles: Query<
        (Entity, &bevy_hanabi::prelude::ParticleEffect, Option<&bevy_hanabi::prelude::EffectSpawner>, &Visibility, &InheritedVisibility, &GlobalTransform),
    >,
    mut logged_once: Local<bool>,
) {
    if !solari_state.enabled {
        *logged_once = false;
        return;
    }

    // Log once when Solari first enables, then every 10 seconds
    let elapsed = time.elapsed_secs();
    let should_log = !*logged_once || (elapsed - *last_log >= 10.0);
    if !should_log {
        return;
    }
    *last_log = elapsed;
    *logged_once = true;

    console_info!("Particles+Solari", "=== DIAGNOSTIC: Solari active, checking particles ===");

    // Camera state
    if let Ok((entity, camera, has_deferred, has_depth)) = camera_query.single() {
        console_info!("Particles+Solari", "MainCamera {:?}: active={}, deferred_prepass={}, depth_prepass={}",
            entity, camera.is_active, has_deferred.is_some(), has_depth.is_some());
    } else {
        console_info!("Particles+Solari", "WARNING: No MainCamera with SolariLighting found!");
    }

    // Particle state
    let particle_count = particles.iter().count();
    console_info!("Particles+Solari", "ParticleEffect entities: {}", particle_count);

    for (entity, _effect, spawner, vis, inherited_vis, transform) in particles.iter() {
        let spawner_active = spawner.map_or(false, |s| s.active);
        let spawner_alive = spawner.is_some();
        let pos = transform.translation();
        console_info!("Particles+Solari",
            "  {:?}: vis={:?}, inherited_visible={}, spawner={}, spawner_active={}, pos=({:.1},{:.1},{:.1})",
            entity, vis, inherited_vis.get(), spawner_alive, spawner_active,
            pos.x, pos.y, pos.z);
    }

    if particle_count == 0 {
        console_info!("Particles+Solari", "No particle effects in scene - nothing to render");
    }
}

/// System to enable/disable cameras based on whether their panels are visible.
/// This prevents rendering to textures that aren't being displayed, improving performance.
fn sync_camera_activity(
    docking: Res<DockingState>,
    mut main_cameras: Query<&mut Camera, With<MainCamera>>,
    mut gizmo_overlay_cameras: Query<&mut Camera, (With<GizmoOverlayCamera>, Without<MainCamera>, Without<StudioPreviewCamera>, Without<ParticlePreviewCamera>, Without<MaterialPreviewCamera>)>,
    mut studio_cameras: Query<&mut Camera, (With<StudioPreviewCamera>, Without<MainCamera>, Without<ParticlePreviewCamera>)>,
    mut particle_cameras: Query<&mut Camera, (With<ParticlePreviewCamera>, Without<MainCamera>, Without<StudioPreviewCamera>)>,
    mut material_cameras: Query<&mut Camera, (With<MaterialPreviewCamera>, Without<MainCamera>, Without<StudioPreviewCamera>, Without<ParticlePreviewCamera>)>,
    mut scene_cameras_3d: Query<&mut Camera, (With<CameraNodeData>, Without<MainCamera>, Without<GizmoOverlayCamera>, Without<StudioPreviewCamera>, Without<ParticlePreviewCamera>, Without<MaterialPreviewCamera>)>,
    mut scene_rigs: Query<&mut Camera, (With<CameraRigData>, Without<MainCamera>, Without<GizmoOverlayCamera>, Without<StudioPreviewCamera>, Without<ParticlePreviewCamera>, Without<MaterialPreviewCamera>, Without<CameraNodeData>)>,
    mut scene_cameras_2d: Query<&mut Camera, (With<Camera2DData>, Without<MainCamera>, Without<GizmoOverlayCamera>, Without<StudioPreviewCamera>, Without<ParticlePreviewCamera>, Without<MaterialPreviewCamera>, Without<CameraNodeData>, Without<CameraRigData>)>,
) {
    // Main viewport camera - always active if Viewport panel is visible
    let viewport_visible = docking.is_panel_visible(&PanelId::Viewport);
    for mut camera in main_cameras.iter_mut() {
        if camera.is_active != viewport_visible {
            camera.is_active = viewport_visible;
        }
    }

    // Gizmo overlay camera - synced with main viewport camera
    for mut camera in gizmo_overlay_cameras.iter_mut() {
        if camera.is_active != viewport_visible {
            camera.is_active = viewport_visible;
        }
    }

    // Studio preview camera - only active if StudioPreview panel is visible
    let studio_visible = docking.is_panel_visible(&PanelId::StudioPreview);
    for mut camera in studio_cameras.iter_mut() {
        if camera.is_active != studio_visible {
            camera.is_active = studio_visible;
        }
    }

    // Particle preview camera - active if ParticlePreview or ParticleEditor panel is visible
    let particle_visible = docking.is_panel_visible(&PanelId::ParticlePreview)
        || docking.is_panel_visible(&PanelId::ParticleEditor);
    for mut camera in particle_cameras.iter_mut() {
        if camera.is_active != particle_visible {
            camera.is_active = particle_visible;
        }
    }

    // Material preview camera - only active if MaterialPreview panel is visible
    let material_visible = docking.is_panel_visible(&PanelId::MaterialPreview);
    for mut camera in material_cameras.iter_mut() {
        if camera.is_active != material_visible {
            camera.is_active = material_visible;
        }
    }

    // Scene cameras must stay inactive - they exist for data/preview only,
    // the camera preview system spawns its own camera to render from them
    for mut camera in scene_cameras_3d.iter_mut() {
        if camera.is_active {
            camera.is_active = false;
        }
    }
    for mut camera in scene_rigs.iter_mut() {
        if camera.is_active {
            camera.is_active = false;
        }
    }
    for mut camera in scene_cameras_2d.iter_mut() {
        if camera.is_active {
            camera.is_active = false;
        }
    }
}

/// Sync gizmo overlay camera transform and projection with the main camera.
/// Runs in PostUpdate to ensure it picks up all camera changes from Update.
fn sync_gizmo_overlay_camera(
    main_camera: Query<(&Transform, &Projection), With<MainCamera>>,
    mut gizmo_camera: Query<(&mut Transform, &mut Projection), (With<GizmoOverlayCamera>, Without<MainCamera>)>,
) {
    let Ok((main_transform, main_projection)) = main_camera.single() else { return };
    let Ok((mut gizmo_transform, mut gizmo_projection)) = gizmo_camera.single_mut() else { return };

    *gizmo_transform = *main_transform;
    *gizmo_projection = main_projection.clone();
}
