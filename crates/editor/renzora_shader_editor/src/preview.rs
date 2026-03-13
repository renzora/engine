//! Shader preview panel — renders compiled code shaders on a fullscreen quad
//! via render-to-texture in an egui panel.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui::{self, RichText, TextureId};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_core::{IsolatedCamera, HideInHierarchy, EditorLocked};
use renzora_editor::{EditorPanel, PanelLocation};
use renzora_shader::runtime::{CodeShaderMaterial, CodeShaderState, ShaderCache};
use renzora_theme::ThemeManager;

use crate::ShaderEditorState;

pub const SHADER_PREVIEW_LAYER: usize = 9;

// ── Resources ────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ShaderPreviewImage {
    pub handle: Handle<Image>,
    pub texture_id: Option<TextureId>,
    pub size: (u32, u32),
}

impl Default for ShaderPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            texture_id: None,
            size: (512, 512),
        }
    }
}

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct ShaderPreviewCamera;

#[derive(Component)]
pub struct ShaderPreviewQuad;

// ── Setup system ─────────────────────────────────────────────────────────────

fn setup_shader_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
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

    user_textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));
    let texture_id = user_textures.image_id(image_handle.id());

    commands.insert_resource(ShaderPreviewImage {
        handle: image_handle.clone(),
        texture_id,
        size: (512, 512),
    });

    // Orthographic camera looking at the quad
    commands.spawn((
        Camera3d::default(),
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

// ── Camera sync ──────────────────────────────────────────────────────────────

fn sync_shader_preview_camera(
    editor_state: Res<ShaderEditorState>,
    mut camera: Query<&mut Camera, With<ShaderPreviewCamera>>,
) {
    let should_be_active = editor_state.compiled_wgsl.is_some();
    for mut cam in camera.iter_mut() {
        if cam.is_active != should_be_active {
            cam.is_active = should_be_active;
        }
    }
}

// ── Shader hot-swap ──────────────────────────────────────────────────────────

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

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct ShaderPreviewPlugin;

impl Plugin for ShaderPreviewPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShaderPreviewPlugin");
        app.init_resource::<ShaderPreviewImage>()
            .add_systems(PostStartup, setup_shader_preview)
            .add_systems(Update, (
                sync_shader_preview_camera,
                update_shader_preview,
            ));
    }
}

// ── Panel ────────────────────────────────────────────────────────────────────

pub struct ShaderPreviewPanel;

impl EditorPanel for ShaderPreviewPanel {
    fn id(&self) -> &str {
        "shader_preview"
    }

    fn title(&self) -> &str {
        "Shader Preview"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::MONITOR)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => &tm.active_theme,
            None => return,
        };
        let muted = theme.text.muted.to_color32();

        let Some(preview_image) = world.get_resource::<ShaderPreviewImage>() else {
            ui.label("Preview not initialized");
            return;
        };

        let Some(texture_id) = preview_image.texture_id else {
            ui.label("Preview texture not ready");
            return;
        };

        let editor_state = world.get_resource::<ShaderEditorState>();
        let has_shader = editor_state.map_or(false, |s| s.compiled_wgsl.is_some());

        if !has_shader {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No shader compiled").size(12.0).color(muted));
            });
            return;
        }

        // Render preview filling available space
        let available = ui.available_size();
        let size = available.x.min(available.y);

        ui.vertical_centered(|ui| {
            ui.add(
                egui::Image::new(egui::load::SizedTexture::new(texture_id, [size, size]))
                    .fit_to_exact_size(egui::vec2(size, size)),
            );
        });

        // Language info below
        if let Some(state) = editor_state {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&state.shader_file.language)
                        .size(11.0)
                        .color(muted),
                );
                if let Some(ref path) = state.file_path {
                    let name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    ui.label(RichText::new(name).size(10.0).color(muted));
                }
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}
