#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.

//! Material preview panel — renders the compiled material on a preview sphere
//! with orbit camera, displayed via render-to-texture in an egui panel.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui::{self, RichText, TextureId};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use renzora::core::{IsolatedCamera, HideInHierarchy, EditorLocked};
use renzora_editor_framework::{EditorPanel, PanelLocation};
use renzora_shader::material::runtime::{FallbackTexture, GraphMaterial, GraphMaterialShaderState, GRAPH_MATERIAL_FRAG_HANDLE, new_graph_material};
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

    pub fn icon(self) -> &'static str {
        match self {
            Self::Sphere => egui_phosphor::regular::GLOBE_HEMISPHERE_EAST,
            Self::Cube => egui_phosphor::regular::CUBE,
            Self::Cylinder => egui_phosphor::regular::CYLINDER,
            Self::Torus => egui_phosphor::regular::CIRCLE_DASHED,
            Self::Plane => egui_phosphor::regular::SQUARE,
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
    mut user_textures: ResMut<EguiUserTextures>,
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

    // Hash shader + texture bindings to detect actual graph changes.
    // Skips redundant work when only selection/UI state changed.
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    result.fragment_shader.hash(&mut hasher);
    for tb in &result.texture_bindings {
        tb.binding.hash(&mut hasher);
        tb.asset_path.hash(&mut hasher);
    }
    let hash = hasher.finish();

    if tracker.last_wgsl_hash == Some(hash) {
        return;
    }
    tracker.last_wgsl_hash = Some(hash);

    info!("[material_preview] Compiling graph: {} nodes, {} connections",
        editor_state.graph.nodes.len(), editor_state.graph.connections.len());

    // Assign textures — unused slots get fallback (never None)
    let fb = &fallback.0;
    for mat_handle in preview_mesh.iter() {
        let Some(material) = materials.get_mut(&mat_handle.0) else {
            warn!("[material_preview] Could not get material asset for preview mesh");
            continue;
        };

        material.texture_0 = Some(fb.clone());
        material.texture_1 = Some(fb.clone());
        material.texture_2 = Some(fb.clone());
        material.texture_3 = Some(fb.clone());

        for tb in &result.texture_bindings {
            if tb.asset_path.is_empty() {
                continue;
            }
            let handle: Handle<Image> = asset_server.load(&tb.asset_path);
            match tb.binding {
                0 => material.texture_0 = Some(handle),
                1 => material.texture_1 = Some(handle),
                2 => material.texture_2 = Some(handle),
                3 => material.texture_3 = Some(handle),
                _ => warn!("[material_preview] Texture binding slot {} exceeds max 3!", tb.binding),
            }
        }
    }

    // Create a unique shader for the preview and assign it to the preview material
    let shader = Shader::from_wgsl(
        result.fragment_shader.clone(),
        "graph_material://preview",
    );
    let preview_shader_handle = shaders.add(shader);

    // Also update the shared handle (used as fallback)
    let shared = Shader::from_wgsl(
        result.fragment_shader.clone(),
        "graph_material://compiled",
    );
    let _ = shaders.insert(&GRAPH_MATERIAL_FRAG_HANDLE, shared);

    // Set the per-material shader on the preview sphere
    for mat_handle in preview_mesh.iter() {
        if let Some(material) = materials.get_mut(&mat_handle.0) {
            material.shader = Some(preview_shader_handle.clone());
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
            .add_systems(Update, (
                sync_preview_camera_active,
                update_preview_camera_orbit,
                swap_preview_shape,
                update_preview_material,
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

        let orbit = world.get_resource::<MaterialPreviewOrbit>();

        // ── Toolbar ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.style_mut().spacing.button_padding = egui::vec2(4.0, 1.0);

            if let Some(orbit) = orbit {
                // Shape dropdown
                let shape_label = format!("{} {}", orbit.shape.icon(), orbit.shape.label());
                let shape_btn = ui.add(egui::Button::new(
                    RichText::new(shape_label).size(10.0).color(text_muted),
                ));
                let shape_id = ui.make_persistent_id("preview_shape");
                if shape_btn.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(shape_id));
                }
                egui::popup_below_widget(ui, shape_id, &shape_btn, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                    for &s in PreviewShape::ALL {
                        if ui.button(format!("{} {}", s.icon(), s.label())).clicked() {
                            let shape = s;
                            if let Some(cmds) = world.get_resource::<renzora_editor_framework::EditorCommands>() {
                                cmds.push(move |world: &mut World| {
                                    world.resource_mut::<MaterialPreviewOrbit>().shape = shape;
                                });
                            }
                        }
                    }
                });

                ui.separator();

                // Auto-rotate toggle
                let rotate_icon = if orbit.auto_rotate {
                    egui_phosphor::regular::ARROWS_CLOCKWISE
                } else {
                    egui_phosphor::regular::ARROW_CLOCKWISE
                };
                let rotate_color = if orbit.auto_rotate {
                    egui::Color32::from_rgb(80, 200, 120)
                } else {
                    text_muted
                };
                if ui.add(egui::Button::new(RichText::new(rotate_icon).size(11.0).color(rotate_color)))
                    .on_hover_text("Auto-rotate")
                    .clicked()
                {
                    let new_val = !orbit.auto_rotate;
                    if let Some(cmds) = world.get_resource::<renzora_editor_framework::EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            world.resource_mut::<MaterialPreviewOrbit>().auto_rotate = new_val;
                        });
                    }
                }

                // Background toggle
                let bg_icon = if orbit.dark_bg {
                    egui_phosphor::regular::MOON
                } else {
                    egui_phosphor::regular::SUN
                };
                if ui.add(egui::Button::new(RichText::new(bg_icon).size(11.0).color(text_muted)))
                    .on_hover_text("Toggle background")
                    .clicked()
                {
                    let new_val = !orbit.dark_bg;
                    if let Some(cmds) = world.get_resource::<renzora_editor_framework::EditorCommands>() {
                        cmds.push(move |world: &mut World| {
                            world.resource_mut::<MaterialPreviewOrbit>().dark_bg = new_val;
                        });
                    }
                }
            }
        });

        ui.separator();

        // ── Preview image ──
        let available = ui.available_size();
        let size = available.x.min(available.y);

        ui.vertical_centered(|ui| {
            let response = ui.add(
                egui::Image::new(egui::load::SizedTexture::new(texture_id, [size, size]))
                    .fit_to_exact_size(egui::vec2(size, size))
                    .sense(egui::Sense::click_and_drag()),
            );

            // Orbit interaction — drag to rotate, scroll to zoom
            if let Some(orbit) = orbit {
                let mut new_yaw = orbit.yaw;
                let mut new_pitch = orbit.pitch;
                let mut new_distance = orbit.distance;

                if response.dragged_by(egui::PointerButton::Primary) {
                    let delta = response.drag_delta();
                    new_yaw += delta.x * 0.01;
                    new_pitch = (new_pitch - delta.y * 0.01).clamp(-1.4, 1.4);
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
                    if let Some(cmds) = world.get_resource::<renzora_editor_framework::EditorCommands>() {
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
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}
