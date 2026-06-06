//! Shader preview â€” renders compiled code shaders on a mesh via
//! render-to-texture. The image is consumed by the bevy-native preview panel.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};

use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_shader::runtime::{CodeShaderMaterial, ShaderCache};

use crate::ShaderEditorState;

pub const SHADER_PREVIEW_LAYER: usize = 9;

// ---- Preview mesh selection -------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMesh {
    Quad,
    #[default]
    Sphere,
    Cube,
    Cylinder,
    Capsule,
    Torus,
    Cone,
    Tetrahedron,
    Plane,
}

impl PreviewMesh {
    pub const ALL: &[PreviewMesh] = &[
        PreviewMesh::Quad,
        PreviewMesh::Sphere,
        PreviewMesh::Cube,
        PreviewMesh::Cylinder,
        PreviewMesh::Capsule,
        PreviewMesh::Torus,
        PreviewMesh::Cone,
        PreviewMesh::Tetrahedron,
        PreviewMesh::Plane,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            PreviewMesh::Quad => "Quad",
            PreviewMesh::Sphere => "Sphere",
            PreviewMesh::Cube => "Cube",
            PreviewMesh::Cylinder => "Cylinder",
            PreviewMesh::Capsule => "Capsule",
            PreviewMesh::Torus => "Torus",
            PreviewMesh::Cone => "Cone",
            PreviewMesh::Tetrahedron => "Tetrahedron",
            PreviewMesh::Plane => "Plane",
        }
    }

    pub fn to_mesh(&self) -> Mesh {
        match self {
            PreviewMesh::Quad => Mesh::from(Rectangle::new(2.0, 2.0)),
            PreviewMesh::Sphere => Sphere::new(1.0).mesh().ico(5).unwrap(),
            PreviewMesh::Cube => Mesh::from(Cuboid::new(1.5, 1.5, 1.5)),
            PreviewMesh::Cylinder => Mesh::from(Cylinder::new(0.8, 1.5)),
            PreviewMesh::Capsule => Mesh::from(Capsule3d::new(0.5, 1.0)),
            PreviewMesh::Torus => Mesh::from(Torus::new(0.4, 1.0)),
            PreviewMesh::Cone => Mesh::from(Cone {
                radius: 0.8,
                height: 1.5,
            }),
            PreviewMesh::Tetrahedron => Mesh::from(Tetrahedron::default()),
            PreviewMesh::Plane => Plane3d::default().mesh().size(2.0, 2.0).build(),
        }
    }
}

// ---- Resources --------------------------------------------------------------

#[derive(Resource, Default)]
pub struct ShaderPreviewImage {
    pub handle: Handle<Image>,
}

// ---- Components -------------------------------------------------------------

#[derive(Component)]
pub struct ShaderPreviewCamera;

#[derive(Component)]
pub struct ShaderPreviewQuad;

// ---- Setup system -----------------------------------------------------------

fn setup_shader_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CodeShaderMaterial>>,
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

    commands.insert_resource(ShaderPreviewImage {
        handle: image_handle.clone(),
    });

    // Orthographic camera looking at the quad
    commands.spawn((
        Camera3d::default(),
            Hdr,
            NormalPrepass,
            DepthPrepass,
            MotionVectorPrepass,
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.05, 0.05, 0.08, 1.0)),
            order: -6,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(0.0, 0.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(SHADER_PREVIEW_LAYER),
        ShaderPreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Shader Preview Camera"),
    ));

    // Ambient light so the quad is visible
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 5000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.3, 0.0)),
        RenderLayers::layer(SHADER_PREVIEW_LAYER),
        HideInHierarchy,
        EditorLocked,
        Name::new("Shader Preview Light"),
    ));

    // Fullscreen quad
    let quad_mesh = meshes.add(Rectangle::new(2.0, 2.0));
    let material = materials.add(CodeShaderMaterial::default());

    commands.spawn((
        Mesh3d(quad_mesh),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        RenderLayers::layer(SHADER_PREVIEW_LAYER),
        ShaderPreviewQuad,
        HideInHierarchy,
        EditorLocked,
        Name::new("Shader Preview Quad"),
    ));
}

// ---- Camera sync ------------------------------------------------------------

fn sync_shader_preview_camera(
    editor_state: Res<ShaderEditorState>,
    mut camera: Query<&mut Camera, With<ShaderPreviewCamera>>,
) {
    let should_be_active = editor_state.compiled_wgsl.is_some() && editor_state.preview_compatible;
    for mut cam in camera.iter_mut() {
        if cam.is_active != should_be_active {
            cam.is_active = should_be_active;
        }
    }
}

// ---- Shader hot-swap --------------------------------------------------------

fn update_shader_preview(
    editor_state: Res<ShaderEditorState>,
    mut shaders: ResMut<Assets<Shader>>,
    mut shader_cache: ResMut<ShaderCache>,
    mut materials: ResMut<Assets<CodeShaderMaterial>>,
    preview_quad: Query<&MeshMaterial3d<CodeShaderMaterial>, With<ShaderPreviewQuad>>,
) {
    if !editor_state.is_changed() {
        return;
    }

    // Only apply to preview if the shader is compatible with CodeShaderMaterial's
    // bind group layout. Shaders with custom textures/samplers would crash the pipeline.
    if !editor_state.preview_compatible {
        return;
    }

    if let Some(ref wgsl) = editor_state.compiled_wgsl {
        let handle = shader_cache.get_or_insert(wgsl, "code_shader://preview", &mut shaders);

        // Update the preview quad's material with the new shader handle
        for mat_handle in preview_quad.iter() {
            if let Some(mat) = materials.get_mut(&mat_handle.0) {
                mat.shader_handle = handle.clone();
            }
        }
    }
}

// ---- Mesh swap --------------------------------------------------------------

fn swap_preview_mesh(
    editor_state: Res<ShaderEditorState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut preview_quad: Query<&mut Mesh3d, With<ShaderPreviewQuad>>,
    mut preview_camera: Query<&mut Transform, With<ShaderPreviewCamera>>,
    mut current_mesh: Local<Option<PreviewMesh>>,
) {
    let wanted = editor_state.preview_mesh;
    if *current_mesh == Some(wanted) {
        return;
    }
    *current_mesh = Some(wanted);

    let mesh = meshes.add(wanted.to_mesh());
    for mut mesh3d in preview_quad.iter_mut() {
        mesh3d.0 = mesh.clone();
    }

    // Adjust camera for 3D meshes vs flat quad
    let (pos, look_at) = match wanted {
        PreviewMesh::Quad => (Vec3::new(0.0, 0.0, 2.0), Vec3::ZERO),
        PreviewMesh::Plane => (Vec3::new(0.0, 2.0, 1.5), Vec3::ZERO),
        _ => (Vec3::new(1.5, 1.0, 2.5), Vec3::ZERO),
    };
    for mut transform in preview_camera.iter_mut() {
        *transform = Transform::from_translation(pos).looking_at(look_at, Vec3::Y);
    }
}

// ---- Plugin -----------------------------------------------------------------

pub struct ShaderPreviewPlugin;

impl Plugin for ShaderPreviewPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShaderPreviewPlugin");
        app.init_resource::<ShaderPreviewImage>()
            .add_systems(PostStartup, setup_shader_preview)
            .add_systems(
                Update,
                (
                    sync_shader_preview_camera,
                    update_shader_preview,
                    swap_preview_mesh,
                ),
            );
    }
}
