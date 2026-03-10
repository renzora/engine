//! Material preview panel — renders the compiled material on a preview sphere
//! with orbit camera, displayed via render-to-texture in an egui panel.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui::{self, RichText, TextureId};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora_core::{IsolatedCamera, HideInHierarchy, EditorLocked};
use renzora_editor::{EditorPanel, PanelLocation};
use renzora_material::runtime::{GraphMaterial, GraphMaterialShaderState, apply_compiled_shader};
use renzora_theme::ThemeManager;

use crate::MaterialEditorState;

pub const MATERIAL_PREVIEW_LAYER: usize = 8;

// ── Resources ───────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct MaterialPreviewImage {
    pub handle: Handle<Image>,
    pub texture_id: Option<TextureId>,
    pub size: (u32, u32),
}

impl Default for MaterialPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            texture_id: None,
            size: (512, 512),
        }
    }
}

#[derive(Resource)]
pub struct MaterialPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}

impl Default for MaterialPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.8,
            pitch: 0.3,
            distance: 3.0,
            target: Vec3::ZERO,
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
    mut user_textures: ResMut<EguiUserTextures>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GraphMaterial>>,
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

    commands.insert_resource(MaterialPreviewImage {
        handle: image_handle.clone(),
        texture_id,
        size: (512, 512),
    });

    // Camera
    commands.spawn((
        Camera3d::default(),
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

    // Preview sphere
    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());
    let material = materials.add(GraphMaterial::default());

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
    orbit: Res<MaterialPreviewOrbit>,
    mut camera: Query<&mut Transform, With<MaterialPreviewCamera>>,
) {
    for mut transform in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

// ── Shader hot-swap ─────────────────────────────────────────────────────────

fn update_preview_shader(
    editor_state: Res<MaterialEditorState>,
    mut shaders: ResMut<Assets<Shader>>,
    mut shader_state: ResMut<GraphMaterialShaderState>,
) {
    // Only update when the editor state changes
    if !editor_state.is_changed() {
        return;
    }

    if editor_state.compile_errors.is_empty() {
        let _ = apply_compiled_shader(&editor_state.graph, &mut shaders, &mut shader_state);
    }
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
            .add_systems(Update, (
                sync_preview_camera_active,
                update_preview_camera_orbit,
                update_preview_shader,
            ));
    }
}

// ── Panel ───────────────────────────────────────────────────────────────────

pub struct MaterialPreviewPanel;

impl EditorPanel for MaterialPreviewPanel {
    fn id(&self) -> &str {
        "material_preview"
    }

    fn title(&self) -> &str {
        "Material Preview"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::CUBE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => &tm.active_theme,
            None => return,
        };
        let text_muted = theme.text.muted.to_color32();

        let Some(preview_image) = world.get_resource::<MaterialPreviewImage>() else {
            ui.label("Preview not initialized");
            return;
        };

        let Some(texture_id) = preview_image.texture_id else {
            ui.label("Preview texture not ready");
            return;
        };

        let editor_state = world.get_resource::<MaterialEditorState>();
        let has_material = editor_state.map_or(false, |s| s.compiled_wgsl.is_some());

        if !has_material {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No material compiled").size(12.0).color(text_muted));
            });
            return;
        }

        // Render the preview image to fill available space
        let available = ui.available_size();
        let size = available.x.min(available.y);

        ui.vertical_centered(|ui| {
            let response = ui.add(
                egui::Image::new(egui::load::SizedTexture::new(texture_id, [size, size]))
                    .fit_to_exact_size(egui::vec2(size, size))
                    .sense(egui::Sense::click_and_drag()),
            );

            // Orbit interaction — drag to rotate, scroll to zoom
            if let Some(orbit) = world.get_resource::<MaterialPreviewOrbit>() {
                let mut new_yaw = orbit.yaw;
                let mut new_pitch = orbit.pitch;
                let mut new_distance = orbit.distance;

                if response.dragged_by(egui::PointerButton::Primary) {
                    let delta = response.drag_delta();
                    new_yaw += delta.x * 0.01;
                    new_pitch = (new_pitch + delta.y * 0.01).clamp(-1.4, 1.4);
                }

                if response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll.abs() > 0.0 {
                        new_distance = (new_distance - scroll * 0.01).clamp(1.5, 10.0);
                    }
                }

                if (new_yaw - orbit.yaw).abs() > 1e-5
                    || (new_pitch - orbit.pitch).abs() > 1e-5
                    || (new_distance - orbit.distance).abs() > 1e-5
                {
                    if let Some(cmds) = world.get_resource::<renzora_editor::EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            let mut orbit = world.resource_mut::<MaterialPreviewOrbit>();
                            orbit.yaw = new_yaw;
                            orbit.pitch = new_pitch;
                            orbit.distance = new_distance;
                        });
                    }
                }
            }
        });

        // Domain + material name info below preview
        if let Some(state) = editor_state {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(&state.graph.name).size(11.0).color(text_muted));
                ui.label(
                    RichText::new(format!("({})", state.graph.domain.display_name()))
                        .size(10.0)
                        .color(text_muted),
                );
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}
