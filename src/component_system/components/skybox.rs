//! Skybox component — registration, inspector, and sync system.
//!
//! Manages sky rendering (Color, Procedural, Panorama modes), HDR panorama loading,
//! and equirectangular-to-cubemap conversion.

use bevy::prelude::*;
use bevy::core_pipeline::Skybox;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use bevy_egui::egui::{self, Color32, RichText, Sense, Vec2};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::path::PathBuf;
use egui_phosphor::regular::{SUN, FILE, FOLDER_OPEN, IMAGE, X_CIRCLE};

use crate::shared::ProceduralSkyData;

use crate::component_system::{ComponentCategory, ComponentRegistry};
use crate::core::{DisabledComponents, EditorEntity, InspectorPanelRenderState, ViewportCamera};
use crate::project::CurrentProject;
use crate::register_component;
use crate::shared::{SkyMode, SkyboxData};
use crate::ui::{inline_property, get_inspector_theme};
use crate::ui::inspectors::sanitize_f32;

// ============================================================================
// Public types (used by core/mod.rs cleanup)
// ============================================================================

/// Resource to track the currently loaded skybox.
#[derive(Resource, Default)]
pub struct SkyboxState {
    pub current_path: Option<String>,
    pub equirect_handle: Option<Handle<Image>>,
    pub cubemap_handle: Option<Handle<Image>>,
    pub conversion_pending: bool,
    /// Handle for the procedural cubemap (kept alive to prevent asset drop)
    pub procedural_cubemap_handle: Option<Handle<Image>>,
    /// Hash of ProceduralSkyData params used to generate the current cubemap
    pub procedural_params_hash: u64,
}

// ============================================================================
// Shared helpers
// ============================================================================

/// Copy a sky texture file into the project's assets/environments folder
/// and return the relative path. Used by the skybox inspector and file drop handler.
pub(crate) fn copy_sky_to_project_assets(source_path: &PathBuf, project_path: &PathBuf) -> Option<String> {
    let file_name = source_path.file_name()?;
    let environments_dir = project_path.join("assets").join("environments");
    if let Err(e) = std::fs::create_dir_all(&environments_dir) {
        bevy::log::error!("Failed to create environments directory: {}", e);
        return None;
    }

    let dest_path = environments_dir.join(file_name);
    if !dest_path.exists() || source_path.canonicalize().ok() != dest_path.canonicalize().ok() {
        if let Err(e) = std::fs::copy(source_path, &dest_path) {
            bevy::log::error!("Failed to copy sky texture to project: {}", e);
            return None;
        }
        bevy::log::info!("Copied sky texture to project: {:?}", dest_path);
    }

    Some(format!("assets/environments/{}", file_name.to_string_lossy()))
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(SkyboxData {
        type_id: "skybox",
        display_name: "Skybox",
        category: ComponentCategory::Rendering,
        icon: SUN,
        priority: 101,
        custom_inspector: inspect_skybox,
    }));
}

// ============================================================================
// Helpers
// ============================================================================

fn rgb_to_color32(r: f32, g: f32, b: f32) -> Color32 {
    Color32::from_rgb(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    )
}

fn color32_to_rgb(color: Color32) -> (f32, f32, f32) {
    (
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
    )
}

fn is_hdr_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(), "hdr" | "exr"))
        .unwrap_or(false)
}

// ============================================================================
// Inspector
// ============================================================================

fn inspect_skybox(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let project_path = world.get_resource::<CurrentProject>().map(|p| p.path.clone());

    let dragging_path = world.get_resource::<InspectorPanelRenderState>()
        .and_then(|s| s.dragging_asset_path.clone());

    let os_hovered_hdr = ui.ctx().input(|i| {
        i.raw.hovered_files.iter()
            .any(|f| f.path.as_ref().map_or(false, |p| is_hdr_file(p)))
    });

    let os_dropped_hdr = ui.ctx().input(|i| {
        i.raw.dropped_files.iter()
            .find(|f| f.path.as_ref().map_or(false, |p| is_hdr_file(p)))
            .and_then(|f| f.path.clone())
    });

    let Some(mut skybox) = world.get_mut::<SkyboxData>(entity) else {
        return false;
    };

    let mut changed = false;
    let mut should_clear_drag = false;
    let mut os_import_path: Option<PathBuf> = None;
    let mut row = 0;

    // Sky mode combo
    inline_property(ui, row, "Mode", |ui| {
        let sky_options = ["Color", "Procedural", "Panorama"];
        let mut sky_index = match skybox.sky_mode {
            SkyMode::Color => 0,
            SkyMode::Procedural => 1,
            SkyMode::Panorama => 2,
        };
        egui::ComboBox::from_id_salt("skybox_mode_combo")
            .selected_text(sky_options[sky_index])
            .show_ui(ui, |ui| {
                for (i, option) in sky_options.iter().enumerate() {
                    if ui.selectable_value(&mut sky_index, i, *option).changed() {
                        skybox.sky_mode = match sky_index {
                            0 => SkyMode::Color,
                            1 => SkyMode::Procedural,
                            2 => SkyMode::Panorama,
                            _ => SkyMode::Color,
                        };
                        changed = true;
                    }
                }
            });
    });
    row += 1;

    match skybox.sky_mode {
        SkyMode::Color => {
            changed |= inline_property(ui, row, "Background", |ui| {
                let mut color = rgb_to_color32(skybox.clear_color.0, skybox.clear_color.1, skybox.clear_color.2);
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp {
                    skybox.clear_color = color32_to_rgb(color);
                }
                resp
            });
        }
        SkyMode::Procedural => {
            let sky = &mut skybox.procedural_sky;

            sanitize_f32(&mut sky.sky_curve, 0.01, 1.0, 0.15);
            sanitize_f32(&mut sky.ground_curve, 0.01, 1.0, 0.02);

            changed |= inline_property(ui, row, "Top Color", |ui| {
                let mut color = rgb_to_color32(sky.sky_top_color.0, sky.sky_top_color.1, sky.sky_top_color.2);
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp { sky.sky_top_color = color32_to_rgb(color); }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Horizon Color", |ui| {
                let mut color = rgb_to_color32(sky.sky_horizon_color.0, sky.sky_horizon_color.1, sky.sky_horizon_color.2);
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp { sky.sky_horizon_color = color32_to_rgb(color); }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Sky Curve", |ui| {
                ui.add(egui::DragValue::new(&mut sky.sky_curve).speed(0.01).range(0.01..=1.0)).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Ground Horizon", |ui| {
                let mut color = rgb_to_color32(sky.ground_horizon_color.0, sky.ground_horizon_color.1, sky.ground_horizon_color.2);
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp { sky.ground_horizon_color = color32_to_rgb(color); }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Ground Bottom", |ui| {
                let mut color = rgb_to_color32(sky.ground_bottom_color.0, sky.ground_bottom_color.1, sky.ground_bottom_color.2);
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp { sky.ground_bottom_color = color32_to_rgb(color); }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Ground Curve", |ui| {
                ui.add(egui::DragValue::new(&mut sky.ground_curve).speed(0.01).range(0.01..=1.0)).changed()
            });
        }
        SkyMode::Panorama => {
            let pano = &mut skybox.panorama_sky;
            let theme_colors = get_inspector_theme(ui.ctx());

            sanitize_f32(&mut pano.rotation, 0.0, 360.0, 0.0);
            sanitize_f32(&mut pano.energy, 0.0, 10.0, 1.0);

            ui.horizontal(|ui| {
                ui.label("Sky Texture");
            });
            ui.add_space(4.0);

            let drop_zone_height = 60.0;
            let available_width = ui.available_width();

            ui.horizontal(|ui| {
                let drop_width = available_width - 34.0;
                let (rect, _response) = ui.allocate_exact_size(
                    Vec2::new(drop_width, drop_zone_height),
                    Sense::click_and_drag(),
                );

                ui.painter().rect_filled(rect, 4.0, theme_colors.widget_inactive_bg);
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, theme_colors.widget_border), egui::StrokeKind::Outside);

                let pointer_pos = ui.ctx().pointer_hover_pos();
                let pointer_in_zone = pointer_pos.map_or(false, |p| rect.contains(p));

                if let Some(ref asset_path) = dragging_path {
                    if pointer_in_zone {
                        ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme_colors.semantic_accent), egui::StrokeKind::Inside);
                    }

                    if pointer_in_zone && ui.ctx().input(|i| i.pointer.any_released()) {
                        if is_hdr_file(asset_path) {
                            if let Some(ref project) = project_path {
                                if let Some(rel_path) = copy_sky_to_project_assets(asset_path, project) {
                                    pano.panorama_path = rel_path;
                                } else {
                                    pano.panorama_path = asset_path.to_string_lossy().to_string();
                                }
                            } else {
                                pano.panorama_path = asset_path.to_string_lossy().to_string();
                            }
                            changed = true;
                            should_clear_drag = true;
                        }
                    }
                }

                if os_hovered_hdr && pointer_in_zone {
                    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme_colors.semantic_accent), egui::StrokeKind::Inside);
                }

                if let Some(ref dropped_path) = os_dropped_hdr {
                    if pointer_in_zone {
                        if let Some(ref project) = project_path {
                            if let Some(rel_path) = copy_sky_to_project_assets(dropped_path, project) {
                                pano.panorama_path = rel_path;
                            } else {
                                pano.panorama_path = dropped_path.to_string_lossy().to_string();
                            }
                        } else {
                            pano.panorama_path = dropped_path.to_string_lossy().to_string();
                        }
                        changed = true;
                        os_import_path = Some(dropped_path.clone());
                    }
                }

                if !pano.panorama_path.is_empty() {
                    let file_name = pano.panorama_path.rsplit('/').next()
                        .or_else(|| pano.panorama_path.rsplit('\\').next())
                        .unwrap_or(&pano.panorama_path);
                    let center = rect.center();
                    ui.painter().text(
                        egui::pos2(center.x, center.y - 10.0),
                        egui::Align2::CENTER_CENTER,
                        IMAGE,
                        egui::FontId::proportional(24.0),
                        theme_colors.semantic_warning,
                    );
                    ui.painter().text(
                        egui::pos2(center.x, center.y + 14.0),
                        egui::Align2::CENTER_CENTER,
                        file_name,
                        egui::FontId::proportional(12.0),
                        theme_colors.text_primary,
                    );
                } else {
                    let center = rect.center();
                    ui.painter().text(
                        egui::pos2(center.x, center.y - 8.0),
                        egui::Align2::CENTER_CENTER,
                        FILE,
                        egui::FontId::proportional(20.0),
                        theme_colors.text_disabled,
                    );
                    ui.painter().text(
                        egui::pos2(center.x, center.y + 12.0),
                        egui::Align2::CENTER_CENTER,
                        "Browse or drag HDR/EXR",
                        egui::FontId::proportional(11.0),
                        theme_colors.text_muted,
                    );
                }

                if ui.add_sized([26.0, drop_zone_height], egui::Button::new(FOLDER_OPEN.to_string())).clicked() {
                    if let Some(texture_path) = rfd::FileDialog::new()
                        .add_filter("HDR Images", &["hdr", "exr"])
                        .set_title("Select Sky Texture")
                        .pick_file()
                    {
                        if let Some(ref project) = project_path {
                            if let Some(rel_path) = copy_sky_to_project_assets(&texture_path, project) {
                                pano.panorama_path = rel_path;
                                changed = true;
                            } else {
                                pano.panorama_path = texture_path.to_string_lossy().to_string();
                                changed = true;
                            }
                        } else {
                            pano.panorama_path = texture_path.to_string_lossy().to_string();
                            changed = true;
                        }
                    }
                }
            });

            ui.add_space(4.0);

            if !pano.panorama_path.is_empty() {
                if ui.button(RichText::new(format!("{} Clear", X_CIRCLE)).color(theme_colors.semantic_error)).clicked() {
                    pano.panorama_path.clear();
                    changed = true;
                }
                ui.add_space(4.0);
            }

            row += 1;

            changed |= inline_property(ui, row, "Rotation", |ui| {
                ui.add(egui::DragValue::new(&mut pano.rotation).speed(1.0).range(0.0..=360.0).suffix("\u{00b0}")).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Energy", |ui| {
                ui.add(egui::DragValue::new(&mut pano.energy).speed(0.1).range(0.0..=10.0)).changed()
            });
        }
    }

    // Drop the skybox borrow so we can access world again
    drop(skybox);

    if should_clear_drag {
        if let Some(mut rs) = world.get_resource_mut::<InspectorPanelRenderState>() {
            rs.drag_accepted = true;
        }
    }
    if let Some(import_path) = os_import_path {
        if let Some(mut rs) = world.get_resource_mut::<InspectorPanelRenderState>() {
            rs.pending_file_import = Some(import_path);
        }
    }

    changed
}

// ============================================================================
// Procedural sky generation
// ============================================================================

/// Compute the sky color for a given vertical direction component `y` in [-1, 1].
/// Returns linear RGB in [0, 1].
fn procedural_sky_color(y: f32, sky: &ProceduralSkyData) -> (f32, f32, f32) {
    if y >= 0.0 {
        // Above horizon: blend from horizon → top using sky_curve
        let t = y.powf(1.0 / sky.sky_curve.max(0.01));
        (
            sky.sky_horizon_color.0 + (sky.sky_top_color.0 - sky.sky_horizon_color.0) * t,
            sky.sky_horizon_color.1 + (sky.sky_top_color.1 - sky.sky_horizon_color.1) * t,
            sky.sky_horizon_color.2 + (sky.sky_top_color.2 - sky.sky_horizon_color.2) * t,
        )
    } else {
        // Below horizon: blend from ground_horizon → ground_bottom using ground_curve
        let t = (-y).powf(1.0 / sky.ground_curve.max(0.01));
        (
            sky.ground_horizon_color.0 + (sky.ground_bottom_color.0 - sky.ground_horizon_color.0) * t,
            sky.ground_horizon_color.1 + (sky.ground_bottom_color.1 - sky.ground_horizon_color.1) * t,
            sky.ground_horizon_color.2 + (sky.ground_bottom_color.2 - sky.ground_horizon_color.2) * t,
        )
    }
}

/// Generate a cubemap Image from procedural sky parameters.
/// Produces 6 faces at `face_size x face_size` in Rgba8UnormSrgb format.
fn generate_procedural_cubemap(sky: &ProceduralSkyData) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let face_size: usize = 256;
    let bytes_per_pixel = 4usize; // RGBA8
    let face_data_size = face_size * face_size * bytes_per_pixel;
    let mut data = vec![0u8; face_data_size * 6];

    // Face directions: +X, -X, +Y, -Y, +Z, -Z
    let face_directions: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),     // +X (right)
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),     // -X (left)
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),     // +Y (top)
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),     // -Y (bottom)
        (Vec3::Z, Vec3::Y, Vec3::X),         // +Z (front)
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X), // -Z (back)
    ];

    for (face_idx, (forward, up, right)) in face_directions.iter().enumerate() {
        let face_offset = face_idx * face_data_size;

        for y in 0..face_size {
            for x in 0..face_size {
                let u = (x as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;

                let dir = (*forward + *right * u - *up * v).normalize();

                let (r, g, b) = procedural_sky_color(dir.y, sky);

                let dst_idx = face_offset + (y * face_size + x) * bytes_per_pixel;
                // Convert linear [0,1] to sRGB u8 via gamma curve
                data[dst_idx] = linear_to_srgb_u8(r);
                data[dst_idx + 1] = linear_to_srgb_u8(g);
                data[dst_idx + 2] = linear_to_srgb_u8(b);
                data[dst_idx + 3] = 255;
            }
        }
    }

    let mut cubemap = Image::new(
        Extent3d {
            width: face_size as u32,
            height: face_size as u32,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD | bevy::asset::RenderAssetUsages::MAIN_WORLD,
    );

    cubemap.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    cubemap
}

fn linear_to_srgb_u8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

fn hash_procedural_params(sky: &ProceduralSkyData) -> u64 {
    let mut hasher = DefaultHasher::new();
    sky.hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// Sync system
// ============================================================================

pub(crate) fn sync_skybox(
    mut commands: Commands,
    skybox_query: Query<
        (&SkyboxData, &EditorEntity, Option<&DisabledComponents>),
        Or<(Changed<SkyboxData>, Changed<DisabledComponents>, Changed<EditorEntity>)>,
    >,
    cameras: Query<Entity, With<ViewportCamera>>,
    mut camera_query: Query<&mut Camera, With<ViewportCamera>>,
    asset_server: Res<AssetServer>,
    mut skybox_state: ResMut<SkyboxState>,
    current_project: Option<Res<CurrentProject>>,
    mut images: ResMut<Assets<Image>>,
    has_data: Query<(), With<SkyboxData>>,
    mut removed: RemovedComponents<SkyboxData>,
) {
    let had_removals = removed.read().count() > 0;
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<Skybox>();
        }
        skybox_state.current_path = None;
        skybox_state.equirect_handle = None;
        skybox_state.cubemap_handle = None;
        skybox_state.conversion_pending = false;
        skybox_state.procedural_cubemap_handle = None;
        skybox_state.procedural_params_hash = 0;
        return;
    }

    // Always check for pending cubemap conversion (even without changes to data)
    if skybox_state.conversion_pending {
        if let Some(ref equirect_handle) = skybox_state.equirect_handle {
            if let Some(equirect_image) = images.get(equirect_handle) {
                match equirectangular_to_cubemap(equirect_image) {
                    Ok(cubemap) => {
                        let cubemap_handle = images.add(cubemap);
                        skybox_state.cubemap_handle = Some(cubemap_handle);
                        skybox_state.conversion_pending = false;
                        info!("Converted HDR to cubemap successfully");
                    }
                    Err(e) => {
                        error!("Failed to convert HDR to cubemap: {}", e);
                        skybox_state.conversion_pending = false;
                    }
                }
            }
        }
    }

    if skybox_query.is_empty() {
        return;
    }

    let active = skybox_query.iter().find(|(_, editor, _)| editor.visible);

    let Some((skybox, _editor, dc)) = active else {
        return;
    };

    let disabled = dc.map_or(false, |d| d.is_disabled("skybox"));
    let skybox = if disabled { &SkyboxData::default() } else { skybox };

    // Helper to clear skybox state and remove from cameras
    let clear_skybox = |commands: &mut Commands, skybox_state: &mut SkyboxState, cameras: &Query<Entity, With<ViewportCamera>>| {
        for camera_entity in cameras.iter() {
            commands.entity(camera_entity).remove::<Skybox>();
        }
        skybox_state.current_path = None;
        skybox_state.equirect_handle = None;
        skybox_state.cubemap_handle = None;
        skybox_state.conversion_pending = false;
        skybox_state.procedural_cubemap_handle = None;
        skybox_state.procedural_params_hash = 0;
    };

    match skybox.sky_mode {
        SkyMode::Color => {
            clear_skybox(&mut commands, &mut skybox_state, &cameras);
            for mut camera in camera_query.iter_mut() {
                camera.clear_color = ClearColorConfig::Custom(Color::srgb(
                    skybox.clear_color.0,
                    skybox.clear_color.1,
                    skybox.clear_color.2,
                ));
            }
        }
        SkyMode::Procedural => {
            let sky = &skybox.procedural_sky;
            let new_hash = hash_procedural_params(sky);

            // Clear panorama state (not procedural state)
            skybox_state.current_path = None;
            skybox_state.equirect_handle = None;
            skybox_state.cubemap_handle = None;
            skybox_state.conversion_pending = false;

            // Only regenerate cubemap if params changed
            if skybox_state.procedural_params_hash != new_hash || skybox_state.procedural_cubemap_handle.is_none() {
                let cubemap = generate_procedural_cubemap(sky);
                let handle = images.add(cubemap);
                skybox_state.procedural_cubemap_handle = Some(handle);
                skybox_state.procedural_params_hash = new_hash;
            }

            if let Some(ref cubemap_handle) = skybox_state.procedural_cubemap_handle {
                for camera_entity in cameras.iter() {
                    commands.entity(camera_entity).insert(Skybox {
                        image: cubemap_handle.clone(),
                        brightness: 1000.0,
                        rotation: Quat::IDENTITY,
                    });
                }
                for mut camera in camera_query.iter_mut() {
                    camera.clear_color = ClearColorConfig::None;
                }
            }
        }
        SkyMode::Panorama => {
            let pano = &skybox.panorama_sky;

            if !pano.panorama_path.is_empty() {
                let needs_load = skybox_state.current_path.as_ref() != Some(&pano.panorama_path);

                if needs_load {
                    let resolved_path = if std::path::Path::new(&pano.panorama_path).is_absolute() {
                        std::path::PathBuf::from(&pano.panorama_path)
                    } else if let Some(ref project) = current_project {
                        project.path.join(&pano.panorama_path)
                    } else {
                        std::path::PathBuf::from(&pano.panorama_path)
                    };

                    let equirect_handle: Handle<Image> = asset_server.load(resolved_path);
                    skybox_state.equirect_handle = Some(equirect_handle);
                    skybox_state.current_path = Some(pano.panorama_path.clone());
                    skybox_state.conversion_pending = true;
                    skybox_state.cubemap_handle = None;

                    info!("Loading HDR panorama for skybox: {}", pano.panorama_path);
                }

                if let Some(ref cubemap_handle) = skybox_state.cubemap_handle {
                    for camera_entity in cameras.iter() {
                        commands.entity(camera_entity).insert(Skybox {
                            image: cubemap_handle.clone(),
                            brightness: pano.energy * 1000.0,
                            rotation: Quat::from_rotation_y(pano.rotation.to_radians()),
                        });
                    }

                    for mut camera in camera_query.iter_mut() {
                        camera.clear_color = ClearColorConfig::None;
                    }
                } else {
                    for mut camera in camera_query.iter_mut() {
                        camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.15));
                    }
                }
            } else {
                clear_skybox(&mut commands, &mut skybox_state, &cameras);
                for mut camera in camera_query.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.3, 0.3, 0.35));
                }
            }
        }
    }

}

// ============================================================================
// Equirectangular to cubemap conversion
// ============================================================================

fn equirectangular_to_cubemap(equirect: &Image) -> Result<Image, String> {
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    use std::f32::consts::PI;

    let src_width = equirect.width() as usize;
    let src_height = equirect.height() as usize;

    if src_width == 0 || src_height == 0 {
        return Err("Invalid image dimensions".to_string());
    }

    let face_size = (src_height / 2).max(256).min(2048);

    let bytes_per_pixel = equirect.texture_descriptor.format.block_copy_size(None).unwrap_or(4) as usize;
    let face_data_size = face_size * face_size * bytes_per_pixel;
    let mut cubemap_data = vec![0u8; face_data_size * 6];

    let src_data = equirect.data.as_ref()
        .ok_or_else(|| "Image has no data".to_string())?;

    // Face directions: +X, -X, +Y, -Y, +Z, -Z
    let face_directions: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),     // +X (right)
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),     // -X (left)
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),     // +Y (top)
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),     // -Y (bottom)
        (Vec3::Z, Vec3::Y, Vec3::X),         // +Z (front)
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X), // -Z (back)
    ];

    for (face_idx, (forward, up, right)) in face_directions.iter().enumerate() {
        let face_offset = face_idx * face_data_size;

        for y in 0..face_size {
            for x in 0..face_size {
                let u = (x as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / face_size as f32 * 2.0 - 1.0;

                let dir = (*forward + *right * u - *up * v).normalize();

                let theta = dir.z.atan2(dir.x);
                let phi = dir.y.asin();

                let eq_u = (theta + PI) / (2.0 * PI);
                let eq_v = (phi + PI / 2.0) / PI;

                let src_x = ((eq_u * src_width as f32) as usize).min(src_width - 1);
                let src_y = (((1.0 - eq_v) * src_height as f32) as usize).min(src_height - 1);

                let src_idx = (src_y * src_width + src_x) * bytes_per_pixel;
                let dst_idx = face_offset + (y * face_size + x) * bytes_per_pixel;

                if src_idx + bytes_per_pixel <= src_data.len() && dst_idx + bytes_per_pixel <= cubemap_data.len() {
                    cubemap_data[dst_idx..dst_idx + bytes_per_pixel]
                        .copy_from_slice(&src_data[src_idx..src_idx + bytes_per_pixel]);
                }
            }
        }
    }

    let mut cubemap = Image::new(
        Extent3d {
            width: face_size as u32,
            height: face_size as u32,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        cubemap_data,
        equirect.texture_descriptor.format,
        equirect.asset_usage,
    );

    cubemap.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });

    Ok(cubemap)
}
