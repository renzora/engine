//! Renzora Skybox — sky rendering with Color, Procedural, and Panorama modes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use bevy::core_pipeline::Skybox;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use bevy_egui::egui;
#[cfg(feature = "editor")]
use egui_phosphor::regular::SUN;
#[cfg(feature = "editor")]
use renzora_editor::{
    file_drop_zone, get_theme_colors, inline_property, sanitize_f32, AppEditorExt,
    EditorCommands, InspectorEntry,
};
#[cfg(feature = "editor")]
use renzora_theme::Theme;


// ============================================================================
// Data types
// ============================================================================

/// Sky rendering mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub enum SkyMode {
    #[default]
    Color,
    Procedural,
    Panorama,
}

/// Procedural sky parameters.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub struct ProceduralSkyData {
    pub sky_top_color: (f32, f32, f32),
    pub sky_horizon_color: (f32, f32, f32),
    pub ground_bottom_color: (f32, f32, f32),
    pub ground_horizon_color: (f32, f32, f32),
    pub sky_curve: f32,
    pub ground_curve: f32,
}

impl Hash for ProceduralSkyData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sky_top_color.0.to_bits().hash(state);
        self.sky_top_color.1.to_bits().hash(state);
        self.sky_top_color.2.to_bits().hash(state);
        self.sky_horizon_color.0.to_bits().hash(state);
        self.sky_horizon_color.1.to_bits().hash(state);
        self.sky_horizon_color.2.to_bits().hash(state);
        self.ground_bottom_color.0.to_bits().hash(state);
        self.ground_bottom_color.1.to_bits().hash(state);
        self.ground_bottom_color.2.to_bits().hash(state);
        self.ground_horizon_color.0.to_bits().hash(state);
        self.ground_horizon_color.1.to_bits().hash(state);
        self.ground_horizon_color.2.to_bits().hash(state);
        self.sky_curve.to_bits().hash(state);
        self.ground_curve.to_bits().hash(state);
    }
}

impl Default for ProceduralSkyData {
    fn default() -> Self {
        Self {
            sky_top_color: (0.15, 0.35, 0.65),
            sky_horizon_color: (0.55, 0.70, 0.85),
            ground_bottom_color: (0.2, 0.17, 0.13),
            ground_horizon_color: (0.55, 0.55, 0.52),
            sky_curve: 0.15,
            ground_curve: 0.02,
        }
    }
}

/// HDR panorama sky parameters.
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub struct PanoramaSkyData {
    pub panorama_path: String,
    pub rotation: f32,
    pub energy: f32,
}

/// Skybox / sky background settings.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SkyboxData {
    pub sky_mode: SkyMode,
    pub clear_color: (f32, f32, f32),
    pub procedural_sky: ProceduralSkyData,
    pub panorama_sky: PanoramaSkyData,
}

impl Default for SkyboxData {
    fn default() -> Self {
        Self {
            sky_mode: SkyMode::default(),
            clear_color: (0.4, 0.6, 0.9),
            procedural_sky: ProceduralSkyData::default(),
            panorama_sky: PanoramaSkyData {
                panorama_path: String::new(),
                rotation: 0.0,
                energy: 1.0,
            },
        }
    }
}

// ============================================================================
// State
// ============================================================================

#[derive(Resource, Default)]
pub struct SkyboxState {
    pub current_path: Option<String>,
    pub equirect_handle: Option<Handle<Image>>,
    pub cubemap_handle: Option<Handle<Image>>,
    pub conversion_pending: bool,
    pub procedural_cubemap_handle: Option<Handle<Image>>,
    pub procedural_params_hash: u64,
}

// ============================================================================
// Helpers (editor-only UI helpers)
// ============================================================================

#[cfg(feature = "editor")]
fn rgb_to_color32(r: f32, g: f32, b: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    )
}

#[cfg(feature = "editor")]
fn color32_to_rgb(color: egui::Color32) -> (f32, f32, f32) {
    (
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
    )
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

// ============================================================================
// Procedural sky generation
// ============================================================================

fn procedural_sky_color(y: f32, sky: &ProceduralSkyData) -> (f32, f32, f32) {
    if y >= 0.0 {
        let t = y.powf(1.0 / sky.sky_curve.max(0.01));
        (
            sky.sky_horizon_color.0 + (sky.sky_top_color.0 - sky.sky_horizon_color.0) * t,
            sky.sky_horizon_color.1 + (sky.sky_top_color.1 - sky.sky_horizon_color.1) * t,
            sky.sky_horizon_color.2 + (sky.sky_top_color.2 - sky.sky_horizon_color.2) * t,
        )
    } else {
        let t = (-y).powf(1.0 / sky.ground_curve.max(0.01));
        (
            sky.ground_horizon_color.0
                + (sky.ground_bottom_color.0 - sky.ground_horizon_color.0) * t,
            sky.ground_horizon_color.1
                + (sky.ground_bottom_color.1 - sky.ground_horizon_color.1) * t,
            sky.ground_horizon_color.2
                + (sky.ground_bottom_color.2 - sky.ground_horizon_color.2) * t,
        )
    }
}

fn generate_procedural_cubemap(sky: &ProceduralSkyData) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let face_size: usize = 256;
    let bytes_per_pixel = 4usize;
    let face_data_size = face_size * face_size * bytes_per_pixel;
    let mut data = vec![0u8; face_data_size * 6];

    let face_directions: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X),
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

fn hash_procedural_params(sky: &ProceduralSkyData) -> u64 {
    let mut hasher = DefaultHasher::new();
    sky.hash(&mut hasher);
    hasher.finish()
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
    let bytes_per_pixel = equirect
        .texture_descriptor
        .format
        .block_copy_size(None)
        .unwrap_or(4) as usize;
    let face_data_size = face_size * face_size * bytes_per_pixel;
    let mut cubemap_data = vec![0u8; face_data_size * 6];

    let src_data = equirect
        .data
        .as_ref()
        .ok_or_else(|| "Image has no data".to_string())?;

    let face_directions: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X),
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

                if src_idx + bytes_per_pixel <= src_data.len()
                    && dst_idx + bytes_per_pixel <= cubemap_data.len()
                {
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

// ============================================================================
// Sync system
// ============================================================================

fn sync_skybox(
    mut commands: Commands,
    skybox_query: Query<&SkyboxData>,
    cameras: Query<Entity, (With<Camera3d>, Without<renzora_core::IsolatedCamera>)>,
    mut camera_query: Query<&mut Camera, (With<Camera3d>, Without<renzora_core::IsolatedCamera>)>,
    asset_server: Res<AssetServer>,
    mut skybox_state: ResMut<SkyboxState>,
    mut images: ResMut<Assets<Image>>,
    has_data: Query<(), With<SkyboxData>>,
    mut removed: RemovedComponents<SkyboxData>,
) {
    let had_removals = removed.read().count() > 0;
    if had_removals && has_data.is_empty() {
        for cam in cameras.iter() {
            commands.entity(cam).remove::<Skybox>();
        }
        for mut camera in camera_query.iter_mut() {
            camera.clear_color = ClearColorConfig::Default;
        }
        *skybox_state = SkyboxState::default();
        return;
    }

    // Check pending cubemap conversion
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

    let Some(skybox) = skybox_query.iter().next() else {
        return;
    };

    match skybox.sky_mode {
        SkyMode::Color => {
            for cam in cameras.iter() {
                commands.entity(cam).remove::<Skybox>();
            }
            for mut camera in camera_query.iter_mut() {
                camera.clear_color = ClearColorConfig::Custom(Color::srgb(
                    skybox.clear_color.0,
                    skybox.clear_color.1,
                    skybox.clear_color.2,
                ));
            }
            skybox_state.current_path = None;
            skybox_state.equirect_handle = None;
            skybox_state.cubemap_handle = None;
            skybox_state.conversion_pending = false;
            skybox_state.procedural_cubemap_handle = None;
            skybox_state.procedural_params_hash = 0;
        }
        SkyMode::Procedural => {
            let sky = &skybox.procedural_sky;
            let new_hash = hash_procedural_params(sky);

            skybox_state.current_path = None;
            skybox_state.equirect_handle = None;
            skybox_state.cubemap_handle = None;
            skybox_state.conversion_pending = false;

            if skybox_state.procedural_params_hash != new_hash
                || skybox_state.procedural_cubemap_handle.is_none()
            {
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
                let needs_load =
                    skybox_state.current_path.as_ref() != Some(&pano.panorama_path);

                if needs_load {
                    let resolved_path = std::path::PathBuf::from(&pano.panorama_path);
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
                        camera.clear_color =
                            ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.15));
                    }
                }
            } else {
                for cam in cameras.iter() {
                    commands.entity(cam).remove::<Skybox>();
                }
                for mut camera in camera_query.iter_mut() {
                    camera.clear_color =
                        ClearColorConfig::Custom(Color::srgb(0.3, 0.3, 0.35));
                }
                *skybox_state = SkyboxState::default();
            }
        }
    }
}

// ============================================================================
// Custom inspector UI (editor only)
// ============================================================================

#[cfg(feature = "editor")]
fn skybox_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let theme_colors = get_theme_colors(ui.ctx());

    let Some(skybox) = world.get::<SkyboxData>(entity) else {
        return;
    };

    // Clone data to avoid borrowing issues
    let mut data = skybox.clone();
    let mut changed = false;
    let mut row = 0;

    // Sky mode combo
    inline_property(ui, row, "Type", theme, |ui| {
        let sky_options = ["Color", "Procedural", "Panorama"];
        let mut sky_index = match data.sky_mode {
            SkyMode::Color => 0,
            SkyMode::Procedural => 1,
            SkyMode::Panorama => 2,
        };
        egui::ComboBox::from_id_salt("skybox_mode_combo")
            .selected_text(sky_options[sky_index])
            .show_ui(ui, |ui| {
                for (i, option) in sky_options.iter().enumerate() {
                    if ui
                        .selectable_value(&mut sky_index, i, *option)
                        .changed()
                    {
                        data.sky_mode = match sky_index {
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

    match data.sky_mode {
        SkyMode::Color => {
            changed |= inline_property(ui, row, "Background", theme, |ui| {
                let mut color = rgb_to_color32(
                    data.clear_color.0,
                    data.clear_color.1,
                    data.clear_color.2,
                );
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp {
                    data.clear_color = color32_to_rgb(color);
                }
                resp
            });
        }
        SkyMode::Procedural => {
            let sky = &mut data.procedural_sky;
            sanitize_f32(&mut sky.sky_curve, 0.01, 1.0, 0.15);
            sanitize_f32(&mut sky.ground_curve, 0.01, 1.0, 0.02);

            changed |= inline_property(ui, row, "Top Color", theme, |ui| {
                let mut color = rgb_to_color32(
                    sky.sky_top_color.0,
                    sky.sky_top_color.1,
                    sky.sky_top_color.2,
                );
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp {
                    sky.sky_top_color = color32_to_rgb(color);
                }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Horizon Color", theme, |ui| {
                let mut color = rgb_to_color32(
                    sky.sky_horizon_color.0,
                    sky.sky_horizon_color.1,
                    sky.sky_horizon_color.2,
                );
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp {
                    sky.sky_horizon_color = color32_to_rgb(color);
                }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Sky Curve", theme, |ui| {
                ui.add(
                    egui::DragValue::new(&mut sky.sky_curve)
                        .speed(0.01)
                        .range(0.01..=1.0),
                )
                .changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Ground Horizon", theme, |ui| {
                let mut color = rgb_to_color32(
                    sky.ground_horizon_color.0,
                    sky.ground_horizon_color.1,
                    sky.ground_horizon_color.2,
                );
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp {
                    sky.ground_horizon_color = color32_to_rgb(color);
                }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Ground Bottom", theme, |ui| {
                let mut color = rgb_to_color32(
                    sky.ground_bottom_color.0,
                    sky.ground_bottom_color.1,
                    sky.ground_bottom_color.2,
                );
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp {
                    sky.ground_bottom_color = color32_to_rgb(color);
                }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Ground Curve", theme, |ui| {
                ui.add(
                    egui::DragValue::new(&mut sky.ground_curve)
                        .speed(0.01)
                        .range(0.01..=1.0),
                )
                .changed()
            });
        }
        SkyMode::Panorama => {
            let pano = &mut data.panorama_sky;
            sanitize_f32(&mut pano.rotation, 0.0, 360.0, 0.0);
            sanitize_f32(&mut pano.energy, 0.0, 10.0, 1.0);

            ui.horizontal(|ui| {
                ui.label("Image");
            });
            ui.add_space(4.0);

            let current = if pano.panorama_path.is_empty() {
                None
            } else {
                Some(pano.panorama_path.as_str())
            };

            let drop_result = file_drop_zone(
                ui,
                egui::Id::new("skybox_panorama_drop"),
                current,
                &["hdr", "exr"],
                "Browse or drag HDR/EXR",
                &theme_colors,
            );

            if drop_result.cleared {
                pano.panorama_path.clear();
                changed = true;
            }

            if let Some(dropped_path) = drop_result.dropped_path {
                pano.panorama_path = dropped_path.to_string_lossy().to_string();
                changed = true;
            }

            if drop_result.browse_clicked {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(texture_path) = rfd::FileDialog::new()
                    .add_filter("HDR Images", &["hdr", "exr"])
                    .set_title("Select Sky Texture")
                    .pick_file()
                {
                    pano.panorama_path = texture_path.to_string_lossy().to_string();
                    changed = true;
                }
            }

            ui.add_space(4.0);
            row += 1;

            changed |= inline_property(ui, row, "Rotation", theme, |ui| {
                ui.add(
                    egui::DragValue::new(&mut pano.rotation)
                        .speed(1.0)
                        .range(0.0..=360.0)
                        .suffix("\u{00b0}"),
                )
                .changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Energy", theme, |ui| {
                ui.add(
                    egui::DragValue::new(&mut pano.energy)
                        .speed(0.1)
                        .range(0.0..=10.0),
                )
                .changed()
            });
        }
    }

    if changed {
        let new_data = data;
        cmds.push(move |world: &mut World| {
            if let Some(mut skybox) = world.get_mut::<SkyboxData>(entity) {
                *skybox = new_data;
            }
        });
    }
}

// ============================================================================
// Inspector entry (editor only)
// ============================================================================

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "skybox",
        display_name: "Skybox",
        icon: SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<SkyboxData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SkyboxData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<SkyboxData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(skybox_custom_ui),
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct SkyboxPlugin;

impl Plugin for SkyboxPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SkyMode>()
            .register_type::<ProceduralSkyData>()
            .register_type::<PanoramaSkyData>()
            .register_type::<SkyboxData>()
            .init_resource::<SkyboxState>()
            .add_systems(Update, sync_skybox);

        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
