//! Particle Preview system - isolated viewport for particle effect preview
//!
//! Provides a separate render viewport specifically for previewing particle effects
//! in the particle editor panel, with its own camera and lighting.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_egui::egui::TextureId;
use bevy_egui::{EguiContexts, EguiTextureHandle};
use bevy_hanabi::prelude::*;

use crate::scene::EditorOnly;
use crate::core::{AppState, DockingState};
use crate::ui::docking::PanelId;
use crate::particles::{ParticleEditorState, build_complete_effect, HanabiEffectDefinition, HanabiEmitShape};

/// Render layer for particle preview (isolated from main scene, studio preview, and shader preview)
pub const PARTICLE_PREVIEW_LAYER: usize = 7;

/// Resource holding the particle preview render texture
#[derive(Resource)]
pub struct ParticlePreviewImage {
    pub handle: Handle<Image>,
    pub texture_id: Option<TextureId>,
    pub size: (u32, u32),
}

impl Default for ParticlePreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            texture_id: None,
            size: (512, 512),
        }
    }
}

/// Marker component for the particle preview camera
#[derive(Component)]
pub struct ParticlePreviewCamera;

/// Marker component for particle preview lights
#[derive(Component)]
pub struct ParticlePreviewLight;

/// Marker component for the particle preview effect entity
#[derive(Component)]
pub struct ParticlePreviewEffect;

/// State for particle preview orbit camera
#[derive(Resource)]
pub struct ParticlePreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub auto_rotate: bool,
    pub auto_rotate_speed: f32,
}

/// Tracks the currently previewed effect to avoid unnecessary rebuilds
#[derive(Resource, Default)]
pub struct ParticlePreviewTracker {
    /// Hash of the last effect definition that was built
    pub last_effect_hash: Option<u64>,
    /// Path of the last loaded file
    pub last_file_path: Option<String>,
}

impl Default for ParticlePreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.3,
            distance: 5.0,
            target: Vec3::ZERO,
            auto_rotate: false,
            auto_rotate_speed: 0.2,
        }
    }
}

/// Sets up the particle preview render texture and camera
pub fn setup_particle_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut preview_image: ResMut<ParticlePreviewImage>,
) {
    let size = Extent3d {
        width: 512,
        height: 512,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("particle_preview_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);

    let image_handle = images.add(image);

    // Update the resource with the actual handle
    preview_image.handle = image_handle.clone();
    preview_image.size = (512, 512);

    // Spawn the particle preview camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.1, 1.0)),
            order: -4, // Render before main camera, studio preview, and shader preview
            is_active: false,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(PARTICLE_PREVIEW_LAYER),
        ParticlePreviewCamera,
        EditorOnly,
        Name::new("Particle Preview Camera"),
    ));

    // Spawn ambient light for particle preview
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 1.0, 1.0),
            illuminance: 5000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.3, 0.0)),
        RenderLayers::layer(PARTICLE_PREVIEW_LAYER),
        ParticlePreviewLight,
        EditorOnly,
        Name::new("Particle Preview Light"),
    ));

    info!("Particle preview system initialized");
}

/// Spawn initial preview effect (only if none exists and no editor effect is active)
pub fn spawn_preview_effect(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    editor_state: Res<ParticleEditorState>,
    existing: Query<Entity, With<ParticlePreviewEffect>>,
) {
    // Don't spawn if already exists
    if !existing.is_empty() {
        return;
    }

    // Use the editor's current effect if available, otherwise use default
    let def = editor_state.current_effect.as_ref()
        .cloned()
        .unwrap_or_default();
    let effect_asset = build_complete_effect(&def);
    let effect_handle = effects.add(effect_asset);

    commands.spawn((
        ParticleEffect::new(effect_handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        RenderLayers::layer(PARTICLE_PREVIEW_LAYER),
        ParticlePreviewEffect,
        EditorOnly,
        Name::new("Particle Preview Effect"),
    ));
}

/// Update the preview effect when the editor state changes
pub fn update_preview_effect(
    mut commands: Commands,
    editor_state: Res<ParticleEditorState>,
    mut tracker: ResMut<ParticlePreviewTracker>,
    mut effects: ResMut<Assets<EffectAsset>>,
    existing: Query<Entity, With<ParticlePreviewEffect>>,
) {
    // Only update if we have an effect being edited
    let Some(ref def) = editor_state.current_effect else {
        return;
    };

    // Check if the file path changed (new file loaded)
    let current_path = editor_state.current_file_path.clone();
    let path_changed = current_path != tracker.last_file_path;

    // Check if the effect definition changed (user edited values)
    // Use a simple hash of key fields to detect changes
    let effect_hash = compute_effect_hash(def);
    let hash_changed = tracker.last_effect_hash != Some(effect_hash);

    // Only rebuild if something actually changed
    if !path_changed && !hash_changed {
        return;
    }

    // Update tracker
    tracker.last_file_path = current_path.clone();
    tracker.last_effect_hash = Some(effect_hash);

    // Despawn old effect entity completely so bevy_hanabi's CompiledParticleEffect
    // doesn't get out of sync with a replaced ParticleEffect handle
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    // Build new effect asset and spawn fresh entity
    let effect_asset = build_complete_effect(def);
    let effect_handle = effects.add(effect_asset);

    commands.spawn((
        ParticleEffect::new(effect_handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        RenderLayers::layer(PARTICLE_PREVIEW_LAYER),
        ParticlePreviewEffect,
        EditorOnly,
        Name::new("Particle Preview Effect"),
    ));
}

/// Compute a hash of all effect parameters to detect changes
fn compute_effect_hash(def: &HanabiEffectDefinition) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    // Basic
    def.name.hash(&mut hasher);
    def.capacity.hash(&mut hasher);

    // Spawning
    ((def.spawn_rate * 100.0) as u32).hash(&mut hasher);
    def.spawn_count.hash(&mut hasher);
    ((def.spawn_duration * 100.0) as u32).hash(&mut hasher);
    def.spawn_cycle_count.hash(&mut hasher);
    def.spawn_starts_active.hash(&mut hasher);

    // Lifetime
    ((def.lifetime_min * 1000.0) as u32).hash(&mut hasher);
    ((def.lifetime_max * 1000.0) as u32).hash(&mut hasher);

    // Shape (hash discriminant + key fields)
    std::mem::discriminant(&def.emit_shape).hash(&mut hasher);
    match &def.emit_shape {
        HanabiEmitShape::Point => {}
        HanabiEmitShape::Circle { radius, .. } |
        HanabiEmitShape::Sphere { radius, .. } => {
            ((radius * 1000.0) as u32).hash(&mut hasher);
        }
        HanabiEmitShape::Cone { base_radius, top_radius, height, .. } => {
            ((base_radius * 1000.0) as u32).hash(&mut hasher);
            ((top_radius * 1000.0) as u32).hash(&mut hasher);
            ((height * 1000.0) as u32).hash(&mut hasher);
        }
        HanabiEmitShape::Rect { half_extents, .. } => {
            for v in half_extents { ((v * 1000.0) as u32).hash(&mut hasher); }
        }
        HanabiEmitShape::Box { half_extents } => {
            for v in half_extents { ((v * 1000.0) as u32).hash(&mut hasher); }
        }
    }

    // Velocity
    std::mem::discriminant(&def.velocity_mode).hash(&mut hasher);
    ((def.velocity_magnitude * 1000.0) as u32).hash(&mut hasher);
    ((def.velocity_spread * 1000.0) as u32).hash(&mut hasher);
    for v in &def.velocity_direction { ((v * 1000.0) as i32).hash(&mut hasher); }
    ((def.velocity_speed_min * 1000.0) as u32).hash(&mut hasher);
    ((def.velocity_speed_max * 1000.0) as u32).hash(&mut hasher);
    for v in &def.velocity_axis { ((v * 1000.0) as i32).hash(&mut hasher); }

    // Forces
    for v in &def.acceleration { ((v * 1000.0) as i32).hash(&mut hasher); }
    ((def.linear_drag * 1000.0) as u32).hash(&mut hasher);
    ((def.radial_acceleration * 1000.0) as i32).hash(&mut hasher);
    ((def.tangent_acceleration * 1000.0) as i32).hash(&mut hasher);
    for v in &def.tangent_accel_axis { ((v * 1000.0) as i32).hash(&mut hasher); }
    def.conform_to_sphere.is_some().hash(&mut hasher);
    if let Some(ref c) = def.conform_to_sphere {
        ((c.radius * 1000.0) as u32).hash(&mut hasher);
        ((c.attraction_accel * 1000.0) as u32).hash(&mut hasher);
    }

    // Size
    ((def.size_start * 1000.0) as u32).hash(&mut hasher);
    ((def.size_end * 1000.0) as u32).hash(&mut hasher);
    ((def.size_start_min * 1000.0) as u32).hash(&mut hasher);
    ((def.size_start_max * 1000.0) as u32).hash(&mut hasher);
    def.size_non_uniform.hash(&mut hasher);
    ((def.size_start_x * 1000.0) as u32).hash(&mut hasher);
    ((def.size_start_y * 1000.0) as u32).hash(&mut hasher);
    ((def.size_end_x * 1000.0) as u32).hash(&mut hasher);
    ((def.size_end_y * 1000.0) as u32).hash(&mut hasher);
    def.screen_space_size.hash(&mut hasher);
    ((def.roundness * 1000.0) as u32).hash(&mut hasher);

    // Color
    def.color_gradient.len().hash(&mut hasher);
    for stop in &def.color_gradient {
        ((stop.position * 1000.0) as u32).hash(&mut hasher);
        for v in &stop.color { ((v * 1000.0) as u32).hash(&mut hasher); }
    }
    def.use_flat_color.hash(&mut hasher);
    for v in &def.flat_color { ((v * 1000.0) as u32).hash(&mut hasher); }
    def.use_hdr_color.hash(&mut hasher);
    ((def.hdr_intensity * 100.0) as u32).hash(&mut hasher);
    std::mem::discriminant(&def.color_blend_mode).hash(&mut hasher);

    // Rendering
    std::mem::discriminant(&def.alpha_mode).hash(&mut hasher);
    ((def.alpha_mask_threshold * 1000.0) as u32).hash(&mut hasher);
    std::mem::discriminant(&def.orient_mode).hash(&mut hasher);
    ((def.rotation_speed * 1000.0) as i32).hash(&mut hasher);
    def.flipbook.is_some().hash(&mut hasher);
    if let Some(ref fb) = def.flipbook {
        fb.grid_columns.hash(&mut hasher);
        fb.grid_rows.hash(&mut hasher);
    }
    def.texture_path.hash(&mut hasher);

    // Simulation
    std::mem::discriminant(&def.simulation_space).hash(&mut hasher);
    std::mem::discriminant(&def.simulation_condition).hash(&mut hasher);
    std::mem::discriminant(&def.motion_integration).hash(&mut hasher);

    // Kill zones
    def.kill_zones.len().hash(&mut hasher);
    for zone in &def.kill_zones {
        match zone {
            crate::particles::KillZone::Sphere { center, radius, kill_inside } => {
                0u8.hash(&mut hasher);
                for v in center { ((v * 1000.0) as i32).hash(&mut hasher); }
                ((radius * 1000.0) as u32).hash(&mut hasher);
                kill_inside.hash(&mut hasher);
            }
            crate::particles::KillZone::Aabb { center, half_size, kill_inside } => {
                1u8.hash(&mut hasher);
                for v in center { ((v * 1000.0) as i32).hash(&mut hasher); }
                for v in half_size { ((v * 1000.0) as u32).hash(&mut hasher); }
                kill_inside.hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

/// Register the particle preview texture with egui
pub fn register_particle_preview_texture(
    mut contexts: EguiContexts,
    mut preview_image: ResMut<ParticlePreviewImage>,
) {
    // Only register once we have a valid handle
    if preview_image.texture_id.is_none() && preview_image.handle != Handle::default() {
        let texture_id = contexts.add_image(EguiTextureHandle::Weak(preview_image.handle.id()));
        preview_image.texture_id = Some(texture_id);
    }
}

/// Update the particle preview camera based on orbit controls
pub fn update_particle_preview_camera(
    time: Res<Time>,
    mut orbit: ResMut<ParticlePreviewOrbit>,
    mut camera: Query<&mut Transform, With<ParticlePreviewCamera>>,
) {
    // Auto-rotate when enabled
    if orbit.auto_rotate {
        orbit.yaw += orbit.auto_rotate_speed * time.delta_secs();
    }

    // Update camera transform
    for mut transform in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin() + 1.0; // Offset up a bit
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

/// Plugin for the particle preview system
pub struct ParticlePreviewPlugin;

impl Plugin for ParticlePreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticlePreviewOrbit>();
        app.init_resource::<ParticlePreviewImage>();
        app.init_resource::<ParticlePreviewTracker>();

        // Setup when entering Editor state
        app.add_systems(OnEnter(AppState::Editor), setup_particle_preview);

        // Register texture always (needed before panel opens)
        app.add_systems(Update,
            register_particle_preview_texture.run_if(in_state(AppState::Editor))
        );
        // Only run expensive update systems when a particle panel is visible
        app.add_systems(Update, (
            update_particle_preview_camera,
            // spawn must run before update so a despawn+respawn cycle works correctly
            spawn_preview_effect.before(update_preview_effect),
            update_preview_effect,
        ).run_if(in_state(AppState::Editor))
         .run_if(|docking: Res<DockingState>| {
             docking.is_panel_visible(&PanelId::ParticlePreview)
                 || docking.is_panel_visible(&PanelId::ParticleEditor)
         }));
    }
}
