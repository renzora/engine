//! Particle preview system — isolated viewport for particle effect preview.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui::TextureId;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use bevy_hanabi::prelude::*;
use renzora_core::IsolatedCamera;
use renzora_runtime::{EditorLocked, HideInHierarchy};

use renzora_hanabi::{ParticleEditorState, HanabiEffectDefinition, HanabiEmitShape};
use renzora_hanabi::builder::build_complete_effect;
use renzora_hanabi::data::EditorMode;

pub const PARTICLE_PREVIEW_LAYER: usize = 7;

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

#[derive(Component)]
pub struct ParticlePreviewCamera;

#[derive(Component)]
pub struct ParticlePreviewLight;

#[derive(Component)]
pub struct ParticlePreviewEffect;

#[derive(Resource)]
pub struct ParticlePreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub auto_rotate: bool,
    pub auto_rotate_speed: f32,
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

#[derive(Resource, Default)]
pub struct ParticlePreviewTracker {
    pub last_effect_hash: Option<u64>,
    pub last_file_path: Option<String>,
}

fn setup_particle_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
) {
    let size = Extent3d {
        width: 512,
        height: 512,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        data: Some(vec![0u8; (size.width * size.height * 4) as usize]),
        ..default()
    };
    image.texture_descriptor.size = size;
    image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;

    let image_handle = images.add(image);

    user_textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));
    let texture_id = user_textures.image_id(image_handle.id());

    commands.insert_resource(ParticlePreviewImage {
        handle: image_handle.clone(),
        texture_id,
        size: (512, 512),
    });

    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.1, 1.0)),
            order: -4,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(PARTICLE_PREVIEW_LAYER),
        ParticlePreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Particle Preview Camera"),
    ));

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
        HideInHierarchy,
        EditorLocked,
        Name::new("Particle Preview Light"),
    ));
}

fn spawn_preview_effect(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    editor_state: Res<ParticleEditorState>,
    existing: Query<Entity, With<ParticlePreviewEffect>>,
) {
    if !existing.is_empty() {
        return;
    }

    let def = match editor_state.editor_mode {
        EditorMode::Graph => {
            if let Some(ref graph) = editor_state.node_graph {
                graph.compile_to_definition()
            } else if let Some(ref d) = editor_state.current_effect {
                d.clone()
            } else {
                return;
            }
        }
        EditorMode::Simple => {
            match editor_state.current_effect {
                Some(ref d) => d.clone(),
                None => return,
            }
        }
    };

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
        HideInHierarchy,
        EditorLocked,
        Name::new("Particle Preview Effect"),
    ));
}

fn update_preview_effect(
    mut commands: Commands,
    editor_state: Res<ParticleEditorState>,
    mut tracker: ResMut<ParticlePreviewTracker>,
    mut effects: ResMut<Assets<EffectAsset>>,
    existing: Query<Entity, With<ParticlePreviewEffect>>,
) {
    // In graph mode, compile the node graph; in simple mode, use the definition directly
    let def = match editor_state.editor_mode {
        EditorMode::Graph => {
            if let Some(ref graph) = editor_state.node_graph {
                graph.compile_to_definition()
            } else if let Some(ref d) = editor_state.current_effect {
                d.clone()
            } else {
                return;
            }
        }
        EditorMode::Simple => {
            match editor_state.current_effect {
                Some(ref d) => d.clone(),
                None => return,
            }
        }
    };

    let current_path = editor_state.current_file_path.clone();
    let path_changed = current_path != tracker.last_file_path;
    let effect_hash = compute_effect_hash(&def);
    let hash_changed = tracker.last_effect_hash != Some(effect_hash);

    if !path_changed && !hash_changed {
        return;
    }

    tracker.last_file_path = current_path;
    tracker.last_effect_hash = Some(effect_hash);

    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

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
        HideInHierarchy,
        EditorLocked,
        Name::new("Particle Preview Effect"),
    ));
}

fn compute_effect_hash(def: &HanabiEffectDefinition) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    def.name.hash(&mut hasher);
    def.capacity.hash(&mut hasher);
    ((def.spawn_rate * 100.0) as u32).hash(&mut hasher);
    def.spawn_count.hash(&mut hasher);
    ((def.spawn_duration * 100.0) as u32).hash(&mut hasher);
    def.spawn_cycle_count.hash(&mut hasher);
    def.spawn_starts_active.hash(&mut hasher);
    ((def.lifetime_min * 1000.0) as u32).hash(&mut hasher);
    ((def.lifetime_max * 1000.0) as u32).hash(&mut hasher);
    std::mem::discriminant(&def.emit_shape).hash(&mut hasher);
    match &def.emit_shape {
        HanabiEmitShape::Point => {}
        HanabiEmitShape::Circle { radius, .. } | HanabiEmitShape::Sphere { radius, .. } => {
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
    std::mem::discriminant(&def.velocity_mode).hash(&mut hasher);
    ((def.velocity_magnitude * 1000.0) as u32).hash(&mut hasher);
    ((def.velocity_spread * 1000.0) as u32).hash(&mut hasher);
    for v in &def.velocity_direction { ((v * 1000.0) as i32).hash(&mut hasher); }
    ((def.velocity_speed_min * 1000.0) as u32).hash(&mut hasher);
    ((def.velocity_speed_max * 1000.0) as u32).hash(&mut hasher);
    for v in &def.velocity_axis { ((v * 1000.0) as i32).hash(&mut hasher); }
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
    std::mem::discriminant(&def.simulation_space).hash(&mut hasher);
    std::mem::discriminant(&def.simulation_condition).hash(&mut hasher);
    std::mem::discriminant(&def.motion_integration).hash(&mut hasher);
    def.kill_zones.len().hash(&mut hasher);
    for zone in &def.kill_zones {
        match zone {
            renzora_hanabi::KillZone::Sphere { center, radius, kill_inside } => {
                0u8.hash(&mut hasher);
                for v in center { ((v * 1000.0) as i32).hash(&mut hasher); }
                ((radius * 1000.0) as u32).hash(&mut hasher);
                kill_inside.hash(&mut hasher);
            }
            renzora_hanabi::KillZone::Aabb { center, half_size, kill_inside } => {
                1u8.hash(&mut hasher);
                for v in center { ((v * 1000.0) as i32).hash(&mut hasher); }
                for v in half_size { ((v * 1000.0) as u32).hash(&mut hasher); }
                kill_inside.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

fn sync_preview_camera_active(
    editor_state: Res<ParticleEditorState>,
    mut camera: Query<&mut Camera, With<ParticlePreviewCamera>>,
) {
    let should_be_active = editor_state.current_effect.is_some();
    for mut cam in camera.iter_mut() {
        if cam.is_active != should_be_active {
            cam.is_active = should_be_active;
        }
    }
}

fn update_particle_preview_camera(
    time: Res<Time>,
    mut orbit: ResMut<ParticlePreviewOrbit>,
    mut camera: Query<&mut Transform, With<ParticlePreviewCamera>>,
) {
    if orbit.auto_rotate {
        orbit.yaw += orbit.auto_rotate_speed * time.delta_secs();
    }

    for mut transform in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin() + 1.0;
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

pub struct ParticlePreviewPlugin;

impl Plugin for ParticlePreviewPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ParticlePreviewPlugin");
        app.init_resource::<ParticlePreviewOrbit>();
        app.init_resource::<ParticlePreviewImage>();
        app.init_resource::<ParticlePreviewTracker>();

        app.add_systems(PostStartup, setup_particle_preview);
        app.add_systems(Update, (
            sync_preview_camera_active,
            update_particle_preview_camera,
            spawn_preview_effect.before(update_preview_effect),
            update_preview_effect,
        ));
    }
}
