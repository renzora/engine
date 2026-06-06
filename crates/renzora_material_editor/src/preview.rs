//! Material preview — renders the compiled material on a preview shape with an
//! orbit camera, displayed via render-to-texture. The panel chrome lives in the
//! native (bevy_ui) `native_preview` module; this file owns the render plugin,
//! resources, and the shader/texture hot-swap systems.

use std::marker::PhantomData;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use uuid::Uuid;

use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_shader::material::runtime::{
    new_graph_material, FallbackTexture, GraphMaterial, GraphMaterialShaderState,
};

use crate::MaterialEditorState;

pub const MATERIAL_PREVIEW_LAYER: usize = 8;

// ── Resources ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct MaterialPreviewImage {
    pub handle: Handle<Image>,
    pub size: (u32, u32),
}

impl Default for MaterialPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            size: (512, 512),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewShape {
    Sphere,
    Cube,
    Cylinder,
    Torus,
    Plane,
}

impl PreviewShape {
    pub const ALL: &[PreviewShape] = &[
        PreviewShape::Sphere,
        PreviewShape::Cube,
        PreviewShape::Cylinder,
        PreviewShape::Torus,
        PreviewShape::Plane,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sphere => "Sphere",
            Self::Cube => "Cube",
            Self::Cylinder => "Cylinder",
            Self::Torus => "Torus",
            Self::Plane => "Plane",
        }
    }

    /// Phosphor icon *name* for this shape (resolved to a glyph by the native UI).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Sphere => "globe-hemisphere-east",
            Self::Cube => "cube",
            Self::Cylinder => "cylinder",
            Self::Torus => "circle-dashed",
            Self::Plane => "square",
        }
    }
}

#[derive(Resource)]
pub struct MaterialPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub shape: PreviewShape,
    pub auto_rotate: bool,
    pub dark_bg: bool,
}

impl Default for MaterialPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.8,
            pitch: 0.3,
            distance: 3.0,
            target: Vec3::ZERO,
            shape: PreviewShape::Sphere,
            auto_rotate: false,
            dark_bg: true,
        }
    }
}

/// Tracks the WGSL hash to detect when preview mesh material needs updating.
#[derive(Resource, Default)]
pub struct MaterialPreviewTracker {
    pub last_wgsl_hash: Option<u64>,
}

// ── Components ──────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct MaterialPreviewCamera;

#[derive(Component)]
pub struct MaterialPreviewLight;

#[derive(Component)]
pub struct MaterialPreviewMesh;

// ── Setup system ────────────────────────────────────────────────────────────

fn setup_material_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GraphMaterial>>,
    fallback: Res<FallbackTexture>,
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

    commands.insert_resource(MaterialPreviewImage {
        handle: image_handle.clone(),
        size: (512, 512),
    });

    // Camera
    commands.spawn((
        Camera3d::default(),
            Hdr,
            NormalPrepass,
            DepthPrepass,
            MotionVectorPrepass,
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.1, 1.0)),
            order: -5,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(0.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
        MaterialPreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Camera"),
    ));

    // Directional light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance: 6000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.4, 0.0)),
        RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
        MaterialPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Light"),
    ));

    // Fill light (softer, from the other side)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.6, 0.7, 0.9),
            illuminance: 2000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.3, -0.8, 0.0)),
        RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
        MaterialPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Fill Light"),
    ));

    // Preview sphere — all texture slots filled with fallback for stable pipeline layout
    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());
    let material = materials.add(new_graph_material(&fallback));

    commands.spawn((
        Mesh3d(sphere_mesh),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
        MaterialPreviewMesh,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Sphere"),
    ));
}

// ── Camera sync ─────────────────────────────────────────────────────────────

fn sync_preview_camera_active(
    editor_state: Res<MaterialEditorState>,
    mut camera: Query<&mut Camera, With<MaterialPreviewCamera>>,
) {
    let should_be_active = editor_state.compiled_wgsl.is_some();
    for mut cam in camera.iter_mut() {
        if cam.is_active != should_be_active {
            cam.is_active = should_be_active;
        }
    }
}

fn update_preview_camera_orbit(
    time: Res<Time>,
    mut orbit: ResMut<MaterialPreviewOrbit>,
    mut camera: Query<(&mut Transform, &mut Camera), With<MaterialPreviewCamera>>,
) {
    if orbit.auto_rotate {
        orbit.yaw += time.delta_secs() * 0.5;
    }

    for (mut transform, mut cam) in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);

        let bg = if orbit.dark_bg {
            Color::srgba(0.08, 0.08, 0.1, 1.0)
        } else {
            Color::srgba(0.45, 0.45, 0.5, 1.0)
        };
        cam.clear_color = ClearColorConfig::Custom(bg);
    }
}

fn swap_preview_shape(
    orbit: Res<MaterialPreviewOrbit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut preview_mesh: Query<&mut Mesh3d, With<MaterialPreviewMesh>>,
) {
    if !orbit.is_changed() {
        return;
    }
    for mut mesh3d in preview_mesh.iter_mut() {
        let new_mesh = match orbit.shape {
            PreviewShape::Sphere => Sphere::new(1.0).mesh().ico(5).unwrap(),
            PreviewShape::Cube => Cuboid::new(1.5, 1.5, 1.5).into(),
            PreviewShape::Cylinder => Cylinder::new(0.8, 2.0).into(),
            PreviewShape::Torus => Torus::new(0.5, 1.0).into(),
            PreviewShape::Plane => Plane3d::new(Vec3::Y, Vec2::splat(1.5)).into(),
        };
        mesh3d.0 = meshes.add(new_mesh);
    }
}

// ── Shader hot-swap ─────────────────────────────────────────────────────────

/// Update both the shader AND the material textures atomically in one system.
/// This prevents the pipeline layout mismatch where the shader declares texture
/// bindings but the material hasn't assigned them yet.
///
/// Uses a content hash to skip redundant work when only non-graph fields
/// of MaterialEditorState change (e.g. selected_node).
/// Hash a `PinValue` by its variant + payload bytes. Used to fingerprint
/// `param/*` defaults for the preview's change detector — float/color/
/// vec arrays are hashed via their `to_bits()` so semantically-equal
/// values produce the same hash even though `f32` doesn't impl `Hash`.
fn hash_pin_value<H: std::hash::Hasher>(
    value: &renzora_shader::material::graph::PinValue,
    state: &mut H,
) {
    use renzora_shader::material::graph::PinValue;
    use std::hash::Hash;
    match value {
        PinValue::Float(f) => {
            0u8.hash(state);
            f.to_bits().hash(state);
        }
        PinValue::Vec2(v) => {
            1u8.hash(state);
            for x in v {
                x.to_bits().hash(state);
            }
        }
        PinValue::Vec3(v) => {
            2u8.hash(state);
            for x in v {
                x.to_bits().hash(state);
            }
        }
        PinValue::Vec4(v) => {
            3u8.hash(state);
            for x in v {
                x.to_bits().hash(state);
            }
        }
        PinValue::Color(c) => {
            4u8.hash(state);
            for x in c {
                x.to_bits().hash(state);
            }
        }
        PinValue::Int(i) => {
            5u8.hash(state);
            i.hash(state);
        }
        PinValue::Bool(b) => {
            6u8.hash(state);
            b.hash(state);
        }
        PinValue::TexturePath(s) | PinValue::String(s) => {
            7u8.hash(state);
            s.hash(state);
        }
        PinValue::None => {
            8u8.hash(state);
        }
    }
}

fn update_preview_material(
    editor_state: Res<MaterialEditorState>,
    asset_server: Res<AssetServer>,
    fallback: Res<FallbackTexture>,
    mut shaders: ResMut<Assets<Shader>>,
    mut shader_state: ResMut<GraphMaterialShaderState>,
    mut tracker: ResMut<MaterialPreviewTracker>,
    preview_mesh: Query<&MeshMaterial3d<GraphMaterial>, With<MaterialPreviewMesh>>,
    mut materials: ResMut<Assets<GraphMaterial>>,
) {
    if !editor_state.is_changed() {
        return;
    }
    // Don't touch the shared shader when no material is being edited —
    // overwriting it with the empty default would break all scene materials.
    if matches!(editor_state.edit_mode, crate::MaterialEditMode::Inactive) {
        return;
    }
    if !editor_state.compile_errors.is_empty() {
        return;
    }

    // Compile once — used for both texture bindings and shader insertion.
    let result = renzora_shader::material::codegen::compile(&editor_state.graph);
    if !result.errors.is_empty() {
        return;
    }

    // Hash shader + texture bindings + param defaults to detect actual
    // graph changes. Skips redundant work when only selection/UI state
    // changed.
    //
    // Param defaults are included because changing a `param/*` node's
    // authored default *doesn't* change the emitted WGSL (defaults
    // live in the params UBO, not in the shader source) — without
    // hashing them, editing a Color Parameter's default in the graph
    // would silently no-op the preview until the user disconnected
    // and reconnected the cable to force a WGSL hash flip.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    result.fragment_shader.hash(&mut hasher);
    for tb in &result.texture_bindings {
        tb.binding.hash(&mut hasher);
        tb.asset_path.hash(&mut hasher);
    }
    for p in &result.parameters {
        p.name.hash(&mut hasher);
        // PinValue's bytes — float/color defaults boil down to f32
        // arrays, so hash via their byte representation. Bools and
        // ints come through their integer hashes naturally.
        hash_pin_value(&p.default, &mut hasher);
    }
    let hash = hasher.finish();

    if tracker.last_wgsl_hash == Some(hash) {
        return;
    }
    tracker.last_wgsl_hash = Some(hash);

    info!(
        "[material_preview] Compiling graph: {} nodes, {} connections",
        editor_state.graph.nodes.len(),
        editor_state.graph.connections.len()
    );

    // Assign textures — unused slots get fallback (never None)
    let fb = &fallback.0;
    for mat_handle in preview_mesh.iter() {
        let Some(material) = materials.get_mut(&mat_handle.0) else {
            warn!("[material_preview] Could not get material asset for preview mesh");
            continue;
        };

        // Texture slots live on the extension half of ExtendedMaterial now.
        material.extension.texture_0 = Some(fb.clone());
        material.extension.texture_1 = Some(fb.clone());
        material.extension.texture_2 = Some(fb.clone());
        material.extension.texture_3 = Some(fb.clone());
        // Cube / array / 3D slots: leave as whatever was last assigned (None
        // until the user points at a real asset; Bevy's FallbackImage
        // covers the layout).
        material.extension.cube_0 = None;
        material.extension.array_0 = None;
        material.extension.volume_0 = None;

        use renzora_shader::material::codegen::TextureKind;
        for tb in &result.texture_bindings {
            if tb.asset_path.is_empty() {
                continue;
            }
            let handle: Handle<Image> = asset_server.load(&tb.asset_path);
            match (tb.kind, tb.binding) {
                (TextureKind::D2, 0) => material.extension.texture_0 = Some(handle),
                (TextureKind::D2, 1) => material.extension.texture_1 = Some(handle),
                (TextureKind::D2, 2) => material.extension.texture_2 = Some(handle),
                (TextureKind::D2, 3) => material.extension.texture_3 = Some(handle),
                (TextureKind::Cube, 0) => material.extension.cube_0 = Some(handle),
                (TextureKind::D2Array, 0) => material.extension.array_0 = Some(handle),
                (TextureKind::D3, 0) => material.extension.volume_0 = Some(handle),
                _ => warn!(
                    "[material_preview] Texture binding slot {:?}/{} not routed!",
                    tb.kind, tb.binding
                ),
            }
        }
    }

    // Create a unique Uuid-addressed shader for the preview material. The
    // preview material's `shader_uuid` points at it; `SurfaceGraphExt::specialize`
    // reconstructs the `Handle::Uuid` and plugs it into the fragment stage.
    let preview_uuid = Uuid::new_v4();
    let preview_handle: Handle<Shader> = Handle::Uuid(preview_uuid, PhantomData);
    let shader = Shader::from_wgsl(result.fragment_shader.clone(), "graph_material://preview");
    let _ = shaders.insert(&preview_handle, shader);

    // Set the per-material shader uuid on the preview sphere AND seed
    // the parameter UBO from the graph's authored defaults. Without
    // the seed, every `param/*` slot reads as zero — a graph whose
    // BaseColor comes from a `param/color` would render as black even
    // though the user authored an orange default. The resolver
    // already does this in `resolve_material_file`; the preview path
    // missed it because Stage 3 (instances) didn't update this code
    // alongside the resolver.
    let default_slots =
        renzora_shader::material::instance::build_default_param_slots(&result.parameters);
    for mat_handle in preview_mesh.iter() {
        if let Some(material) = materials.get_mut(&mat_handle.0) {
            material.extension.shader_uuid = Some(preview_uuid);
            material.extension.params.slots = default_slots;
        }
    }
    shader_state.last_wgsl_hash = Some(hash);
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct MaterialPreviewPlugin;

impl Plugin for MaterialPreviewPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MaterialPreviewPlugin");
        app.init_resource::<MaterialPreviewOrbit>()
            .init_resource::<MaterialPreviewImage>()
            .init_resource::<MaterialPreviewTracker>()
            .add_systems(PostStartup, setup_material_preview)
            .add_systems(
                Update,
                (
                    sync_preview_camera_active,
                    update_preview_camera_orbit,
                    swap_preview_shape,
                    update_preview_material,
                ),
            );
    }
}

