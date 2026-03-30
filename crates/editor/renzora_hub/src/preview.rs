//! Hub asset preview — render-to-texture for shader assets in the store overlay.
//!
//! Uses a dedicated render layer + offscreen camera to render a `CodeShaderMaterial`
//! onto a sphere, displayed as an egui texture in the overlay.

use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui::TextureId;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_shader::runtime::{CodeShaderMaterial, ShaderCache};

pub const HUB_PREVIEW_LAYER: usize = 11;

// ── Resources ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct HubPreviewImage {
    pub handle: Handle<Image>,
    pub texture_id: Option<TextureId>,
}

impl Default for HubPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            texture_id: None,
        }
    }
}

/// Set `wgsl` to trigger shader compilation; set `clear` to deactivate.
#[derive(Resource, Default)]
pub struct HubShaderRequest {
    pub wgsl: Option<String>,
    pub active: bool,
    pub clear: bool,
}

// ── Components ──────────────────────────────────────────────────────────────

#[derive(Component)]
struct HubPreviewCamera;

#[derive(Component)]
struct HubPreviewSubject;

// ── Setup ───────────────────────────────────────────────────────────────────

fn setup_hub_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CodeShaderMaterial>>,
) {
    let size = Extent3d {
        width: 256,
        height: 256,
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

    commands.insert_resource(HubPreviewImage {
        handle: image_handle.clone(),
        texture_id,
    });

    // Camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.05, 0.05, 0.08, 1.0)),
            order: -7,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(1.5, 1.0, 2.5).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(HUB_PREVIEW_LAYER),
        HubPreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Hub Preview Camera"),
    ));

    // Key light
    commands.spawn((
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 6000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.4, 0.0)),
        RenderLayers::layer(HUB_PREVIEW_LAYER),
        HideInHierarchy,
        EditorLocked,
        Name::new("Hub Preview Key Light"),
    ));

    // Fill light
    commands.spawn((
        PointLight {
            color: Color::srgb(0.6, 0.7, 1.0),
            intensity: 2000.0,
            range: 20.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-2.0, 1.0, -1.0),
        RenderLayers::layer(HUB_PREVIEW_LAYER),
        HideInHierarchy,
        EditorLocked,
        Name::new("Hub Preview Fill Light"),
    ));

    // Subject sphere
    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());
    let material = materials.add(CodeShaderMaterial::default());

    commands.spawn((
        Mesh3d(sphere_mesh),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        RenderLayers::layer(HUB_PREVIEW_LAYER),
        HubPreviewSubject,
        HideInHierarchy,
        EditorLocked,
        Name::new("Hub Preview Sphere"),
    ));
}

// ── Systems ─────────────────────────────────────────────────────────────────

fn apply_hub_shader(
    mut request: ResMut<HubShaderRequest>,
    mut shaders: ResMut<Assets<Shader>>,
    mut shader_cache: ResMut<ShaderCache>,
    mut materials: ResMut<Assets<CodeShaderMaterial>>,
    subject_q: Query<&MeshMaterial3d<CodeShaderMaterial>, With<HubPreviewSubject>>,
    mut camera_q: Query<&mut Camera, With<HubPreviewCamera>>,
) {
    if request.clear {
        request.clear = false;
        request.active = false;
        for mut cam in camera_q.iter_mut() {
            cam.is_active = false;
        }
        return;
    }

    if let Some(wgsl) = request.wgsl.take() {
        let handle =
            shader_cache.get_or_insert(&wgsl, "code_shader://hub_preview", &mut shaders);

        for mat_handle in subject_q.iter() {
            if let Some(mat) = materials.get_mut(&mat_handle.0) {
                mat.shader_handle = handle.clone();
            }
        }

        for mut cam in camera_q.iter_mut() {
            cam.is_active = true;
        }
        request.active = true;
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct HubPreviewPlugin;

impl Plugin for HubPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HubPreviewImage>()
            .init_resource::<HubShaderRequest>()
            .add_systems(PostStartup, setup_hub_preview)
            .add_systems(Update, apply_hub_shader);
    }
}
