#![allow(dead_code)]

mod camera;
mod camera_preview;
pub mod camera2d;
pub mod grid2d;
pub mod render_2d;
mod texture;

pub use camera::camera_controller;
pub use camera_preview::{
    setup_camera_preview_texture, update_camera_preview, CameraPreviewImage,
};
pub use camera2d::{
    camera2d_controller, setup_editor_camera_2d, toggle_viewport_cameras,
};
pub use grid2d::draw_grid_2d;
pub use render_2d::{cleanup_2d_visuals, update_2d_visuals};
pub use texture::{resize_viewport_texture, setup_viewport_texture};

use bevy::prelude::*;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use std::collections::HashMap;

use crate::core::{AppState, EditorSettings, RenderToggles, SelectionState, ViewportState, VisualizationMode};
use crate::spawn::{EditorSceneRoot, SceneType};
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
            .add_systems(Startup, (setup_viewport_texture, setup_camera_preview_texture))
            .add_systems(
                Update,
                (update_render_toggles, update_shadow_settings).run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                auto_switch_viewport_mode.run_if(in_state(AppState::Editor)),
            );
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
