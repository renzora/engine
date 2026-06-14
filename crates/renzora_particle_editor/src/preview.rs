#![allow(dead_code)] // Public surface area kept for upcoming features.

//! Particle preview system — isolated viewport for particle effect preview.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::ui::ComputedNode;
use bevy_hanabi::prelude::*;
use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_editor_framework::DockingState;

use renzora_hanabi::builder::build_complete_effect;
use renzora_hanabi::data::EditorMode;
use renzora_hanabi::{HanabiEffectDefinition, HanabiEmitShape, ParticleEditorState, ParticleSoftTexture};
use bevy_hanabi::EffectMaterial;

pub const PARTICLE_PREVIEW_LAYER: usize = 7;

#[derive(Resource)]
pub struct ParticlePreviewImage {
    pub handle: Handle<Image>,
    pub current_size: (u32, u32),
    pub requested_size: (u32, u32),
}

impl Default for ParticlePreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            current_size: (512, 512),
            requested_size: (512, 512),
        }
    }
}

#[derive(Component)]
pub struct ParticlePreviewCamera;

#[derive(Component)]
pub struct ParticlePreviewLight;

#[derive(Component)]
pub struct ParticlePreviewEffect;

#[derive(Component)]
pub struct ParticlePreviewFloor;

/// Marker for the preview panel's image node so the orbit-input system can tell
/// when the cursor is over the preview (via its `RelativeCursorPosition`).
#[derive(Component)]
pub struct ParticlePreviewViewport;

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
            target: Vec3::new(0.0, 1.0, 0.0),
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

/// User toggles for the preview viewport.
#[derive(Resource)]
pub struct ParticlePreviewSettings {
    pub show_floor: bool,
}

impl Default for ParticlePreviewSettings {
    fn default() -> Self {
        Self { show_floor: true }
    }
}

/// Apply the floor (checkerboard plane) toggle to its `Visibility`.
fn sync_floor_visibility(
    settings: Res<ParticlePreviewSettings>,
    mut floor: Query<&mut Visibility, With<ParticlePreviewFloor>>,
) {
    if !settings.is_changed() {
        return;
    }
    let want = if settings.show_floor {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut v in floor.iter_mut() {
        if *v != want {
            *v = want;
        }
    }
}

fn setup_particle_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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

    commands.insert_resource(ParticlePreviewImage {
        handle: image_handle.clone(),
        current_size: (512, 512),
        requested_size: (512, 512),
    });

    commands.spawn((
        Camera3d::default(),
            Hdr,
            NormalPrepass,
            DepthPrepass,
            MotionVectorPrepass,
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

    // Checkerboard floor (matches the animation studio preview / terrain look).
    let checker_size = 16u32;
    let checker_tiles = 8u32;
    let tex_dim = checker_size * checker_tiles;
    let mut checker_data = vec![0u8; (tex_dim * tex_dim * 4) as usize];
    for y in 0..tex_dim {
        for x in 0..tex_dim {
            let tx = x / checker_size;
            let ty = y / checker_size;
            let is_light = (tx + ty).is_multiple_of(2);
            let (r, g, b) = if is_light { (55, 55, 60) } else { (35, 35, 40) };
            let idx = ((y * tex_dim + x) * 4) as usize;
            checker_data[idx] = b; // BGRA
            checker_data[idx + 1] = g;
            checker_data[idx + 2] = r;
            checker_data[idx + 3] = 255;
        }
    }
    let mut checker_image = Image {
        data: Some(checker_data),
        ..default()
    };
    checker_image.texture_descriptor.size = Extent3d {
        width: tex_dim,
        height: tex_dim,
        depth_or_array_layers: 1,
    };
    checker_image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    checker_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    let checker_tex = images.add(checker_image);
    let floor_material = materials.add(StandardMaterial {
        base_color_texture: Some(checker_tex),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        reflectance: 0.1,
        ..default()
    });
    let floor_mesh = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(5.0)));
    commands.spawn((
        Mesh3d(floor_mesh),
        MeshMaterial3d(floor_material),
        Transform::from_translation(Vec3::ZERO),
        RenderLayers::layer(PARTICLE_PREVIEW_LAYER),
        ParticlePreviewFloor,
        HideInHierarchy,
        EditorLocked,
        Name::new("Particle Preview Floor"),
    ));
}

fn spawn_preview_effect(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    editor_state: Res<ParticleEditorState>,
    existing: Query<Entity, With<ParticlePreviewEffect>>,
    soft: Res<ParticleSoftTexture>,
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
        EditorMode::Simple => match editor_state.current_effect {
            Some(ref d) => d.clone(),
            None => return,
        },
    };

    let effect_asset = build_complete_effect(&def);
    let effect_handle = effects.add(effect_asset);

    commands.spawn((
        ParticleEffect::new(effect_handle),
        EffectMaterial { images: vec![soft.0.clone()] },
        // Sit the emitter above the floor so the burst's lower hemisphere
        // doesn't clip below the checkerboard.
        Transform::from_xyz(0.0, 1.0, 0.0),
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
    soft: Res<ParticleSoftTexture>,
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
        EditorMode::Simple => match editor_state.current_effect {
            Some(ref d) => d.clone(),
            None => return,
        },
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
        EffectMaterial { images: vec![soft.0.clone()] },
        // Sit the emitter above the floor so the burst's lower hemisphere
        // doesn't clip below the checkerboard.
        Transform::from_xyz(0.0, 1.0, 0.0),
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

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
        HanabiEmitShape::Cone {
            base_radius,
            top_radius,
            height,
            ..
        } => {
            ((base_radius * 1000.0) as u32).hash(&mut hasher);
            ((top_radius * 1000.0) as u32).hash(&mut hasher);
            ((height * 1000.0) as u32).hash(&mut hasher);
        }
        HanabiEmitShape::Rect { half_extents, .. } => {
            for v in half_extents {
                ((v * 1000.0) as u32).hash(&mut hasher);
            }
        }
        HanabiEmitShape::Box { half_extents } => {
            for v in half_extents {
                ((v * 1000.0) as u32).hash(&mut hasher);
            }
        }
    }
    std::mem::discriminant(&def.velocity_mode).hash(&mut hasher);
    ((def.velocity_magnitude * 1000.0) as u32).hash(&mut hasher);
    ((def.velocity_spread * 1000.0) as u32).hash(&mut hasher);
    for v in &def.velocity_direction {
        ((v * 1000.0) as i32).hash(&mut hasher);
    }
    ((def.velocity_speed_min * 1000.0) as u32).hash(&mut hasher);
    ((def.velocity_speed_max * 1000.0) as u32).hash(&mut hasher);
    for v in &def.velocity_axis {
        ((v * 1000.0) as i32).hash(&mut hasher);
    }
    for v in &def.acceleration {
        ((v * 1000.0) as i32).hash(&mut hasher);
    }
    ((def.linear_drag * 1000.0) as u32).hash(&mut hasher);
    ((def.radial_acceleration * 1000.0) as i32).hash(&mut hasher);
    ((def.tangent_acceleration * 1000.0) as i32).hash(&mut hasher);
    for v in &def.tangent_accel_axis {
        ((v * 1000.0) as i32).hash(&mut hasher);
    }
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
        for v in &stop.color {
            ((v * 1000.0) as u32).hash(&mut hasher);
        }
    }
    def.use_flat_color.hash(&mut hasher);
    for v in &def.flat_color {
        ((v * 1000.0) as u32).hash(&mut hasher);
    }
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
            renzora_hanabi::KillZone::Sphere {
                center,
                radius,
                kill_inside,
            } => {
                0u8.hash(&mut hasher);
                for v in center {
                    ((v * 1000.0) as i32).hash(&mut hasher);
                }
                ((radius * 1000.0) as u32).hash(&mut hasher);
                kill_inside.hash(&mut hasher);
            }
            renzora_hanabi::KillZone::Aabb {
                center,
                half_size,
                kill_inside,
            } => {
                1u8.hash(&mut hasher);
                for v in center {
                    ((v * 1000.0) as i32).hash(&mut hasher);
                }
                for v in half_size {
                    ((v * 1000.0) as u32).hash(&mut hasher);
                }
                kill_inside.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// Run condition: `true` when the Particle Preview panel is in the active
/// dock tree. Heavy per-frame work (effect spawn/update) is gated on this.
pub fn particle_preview_panel_mounted(docking: Option<Res<DockingState>>) -> bool {
    docking.is_some_and(|d| d.tree.contains_panel("particle_preview"))
}

fn sync_preview_camera_active(
    editor_state: Res<ParticleEditorState>,
    docking: Option<Res<DockingState>>,
    mut camera: Query<&mut Camera, With<ParticlePreviewCamera>>,
    preview_effects: Query<Entity, With<ParticlePreviewEffect>>,
    mut tracker: ResMut<ParticlePreviewTracker>,
    mut commands: Commands,
) {
    let panel_mounted = docking.is_some_and(|d| d.tree.contains_panel("particle_preview"));
    let should_be_active = panel_mounted && editor_state.current_effect.is_some();
    for mut cam in camera.iter_mut() {
        if cam.is_active != should_be_active {
            cam.is_active = should_be_active;
        }
    }
    if !panel_mounted {
        for entity in preview_effects.iter() {
            commands.entity(entity).despawn();
        }
        // Reset tracker so the next remount rebuilds the effect cleanly.
        if tracker.last_effect_hash.is_some() || tracker.last_file_path.is_some() {
            tracker.last_effect_hash = None;
            tracker.last_file_path = None;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PreviewDrag {
    Rotate,
    Pan,
}

/// Mouse orbit/pan/zoom for the preview camera, active only while the cursor is
/// over (or a drag was started over) the preview panel. Left-drag = rotate,
/// right/middle-drag = pan the focus point, scroll = zoom. Writes the shared
/// `ParticlePreviewOrbit`, which `update_particle_preview_camera` applies.
fn particle_preview_input(
    hover: Query<&Interaction, With<ParticlePreviewViewport>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut orbit: ResMut<ParticlePreviewOrbit>,
    mut drag: Local<Option<PreviewDrag>>,
) {
    // Use the image node's `Interaction` (picking-aware) rather than a geometric
    // cursor test, so dragging a dock splitter that overlaps the preview rect —
    // or clicking the floor toggle overlay — doesn't grab the camera.
    let over = hover
        .iter()
        .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));

    if drag.is_none() && over {
        if mouse.just_pressed(MouseButton::Left) {
            *drag = Some(PreviewDrag::Rotate);
        } else if mouse.just_pressed(MouseButton::Right) || mouse.just_pressed(MouseButton::Middle) {
            *drag = Some(PreviewDrag::Pan);
        }
    }
    // Any button released with nothing held ends the drag.
    if !mouse.pressed(MouseButton::Left)
        && !mouse.pressed(MouseButton::Right)
        && !mouse.pressed(MouseButton::Middle)
    {
        *drag = None;
    }

    if let Some(mode) = *drag {
        let d = motion.delta;
        if d != Vec2::ZERO {
            match mode {
                PreviewDrag::Rotate => {
                    orbit.yaw -= d.x * 0.01;
                    orbit.pitch = (orbit.pitch + d.y * 0.01).clamp(-1.4, 1.4);
                }
                PreviewDrag::Pan => {
                    let yaw = orbit.yaw;
                    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
                    let scale = (orbit.distance * 0.0005).max(0.0002);
                    orbit.target += right * (-d.x * scale) + Vec3::Y * (d.y * scale);
                }
            }
        }
    }

    if over && scroll.delta.y != 0.0 {
        orbit.distance = (orbit.distance * (1.0 - scroll.delta.y * 0.1)).clamp(0.5, 50.0);
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
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

/// Report the preview panel's pixel size so the render target can match it
/// (crisp 1:1 instead of upscaling a fixed 512² image).
fn report_preview_size(
    panel: Query<&ComputedNode, With<ParticlePreviewViewport>>,
    mut img: ResMut<ParticlePreviewImage>,
) {
    if let Some(cn) = panel.iter().next() {
        let s = cn.size(); // physical pixels
        let w = (s.x.round() as u32).max(1);
        let h = (s.y.round() as u32).max(1);
        if img.requested_size != (w, h) {
            img.requested_size = (w, h);
        }
    }
}

/// Recreate the render-target image when the requested (panel) size changes.
fn resize_preview(mut img: ResMut<ParticlePreviewImage>, mut images: ResMut<Assets<Image>>) {
    let (rw, rh) = img.requested_size;
    if (rw, rh) == img.current_size {
        return;
    }
    let w = rw.clamp(64, 3840);
    let h = rh.clamp(64, 2160);
    if let Some(image) = images.get_mut(&img.handle) {
        image.resize(Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        });
        img.current_size = (w, h);
    }
}

pub struct ParticlePreviewPlugin;

impl Plugin for ParticlePreviewPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ParticlePreviewPlugin");
        app.init_resource::<ParticlePreviewOrbit>();
        app.init_resource::<ParticlePreviewImage>();
        app.init_resource::<ParticlePreviewTracker>();
        app.init_resource::<ParticlePreviewSettings>();

        app.add_systems(PostStartup, setup_particle_preview);
        // sync_preview_camera_active runs every frame so close transitions are
        // always caught (mirrors the Studio Preview / Camera Preview pattern).
        app.add_systems(Update, (sync_preview_camera_active, sync_floor_visibility));
        // Heavy work — compiling the effect graph and respawning the preview
        // entity — only when the Particle Preview panel is actually mounted.
        app.add_systems(
            Update,
            (
                report_preview_size.before(resize_preview),
                resize_preview,
                particle_preview_input.before(update_particle_preview_camera),
                update_particle_preview_camera,
                spawn_preview_effect.before(update_preview_effect),
                update_preview_effect,
            )
                .run_if(particle_preview_panel_mounted),
        );
    }
}
