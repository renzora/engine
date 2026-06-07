//! Renzora Skybox — sky rendering with Color, Procedural, and Panorama modes.

use bevy::core_pipeline::Skybox;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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
    Tiled,
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
            sky_top_color: (0.12, 0.30, 0.60),
            sky_horizon_color: (0.60, 0.75, 0.90),
            ground_bottom_color: (0.18, 0.15, 0.12),
            ground_horizon_color: (0.50, 0.52, 0.50),
            sky_curve: 0.12,
            ground_curve: 0.02,
        }
    }
}

/// Tiled level-design backdrop parameters.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub struct TiledSkyData {
    pub tile_color_a: (f32, f32, f32),
    pub tile_color_b: (f32, f32, f32),
    pub line_color: (f32, f32, f32),
    pub tile_count: u32,
    pub line_width: f32,
}

impl Hash for TiledSkyData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tile_color_a.0.to_bits().hash(state);
        self.tile_color_a.1.to_bits().hash(state);
        self.tile_color_a.2.to_bits().hash(state);
        self.tile_color_b.0.to_bits().hash(state);
        self.tile_color_b.1.to_bits().hash(state);
        self.tile_color_b.2.to_bits().hash(state);
        self.line_color.0.to_bits().hash(state);
        self.line_color.1.to_bits().hash(state);
        self.line_color.2.to_bits().hash(state);
        self.tile_count.hash(state);
        self.line_width.to_bits().hash(state);
    }
}

impl Default for TiledSkyData {
    fn default() -> Self {
        Self {
            tile_color_a: (1.0, 1.0, 1.0),
            tile_color_b: (1.0, 1.0, 1.0),
            line_color: (0.75, 0.75, 0.75),
            tile_count: 16,
            line_width: 0.015,
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
    pub tiled_sky: TiledSkyData,
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
            tiled_sky: TiledSkyData::default(),
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
    pub tiled_cubemap_handle: Option<Handle<Image>>,
    pub tiled_params_hash: u64,
}

// ============================================================================
// Helpers
// ============================================================================

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
// Tiled sky generation
// ============================================================================

fn generate_tiled_cubemap(tiled: &TiledSkyData) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let face_size: usize = 256;
    let bytes_per_pixel = 4usize;
    let face_data_size = face_size * face_size * bytes_per_pixel;
    let mut data = vec![0u8; face_data_size * 6];

    let tc = tiled.tile_count as f32;
    let half_line = tiled.line_width * 0.5;

    // Generate each face using flat 2D UVs so tiles look like flat panels on a box
    // Face order: 0=+X, 1=-X, 2=+Y(top), 3=-Y(bottom), 4=+Z, 5=-Z
    for face_idx in 0..6usize {
        let face_offset = face_idx * face_data_size;
        for y in 0..face_size {
            for x in 0..face_size {
                let (r, g, b) = {
                    let u = (x as f32 + 0.5) / face_size as f32;
                    let v = (y as f32 + 0.5) / face_size as f32;

                    // Scale to tile count
                    let su = u * tc;
                    let sv = v * tc;

                    let fu = su.fract();
                    let fv = sv.fract();

                    // Checkerboard pattern
                    let checker = ((su.floor() as i32) + (sv.floor() as i32)) & 1;

                    // Grid lines
                    let on_line = fu < half_line
                        || fu > 1.0 - half_line
                        || fv < half_line
                        || fv > 1.0 - half_line;

                    if on_line {
                        tiled.line_color
                    } else if checker == 0 {
                        tiled.tile_color_a
                    } else {
                        tiled.tile_color_b
                    }
                };

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

fn hash_tiled_params(tiled: &TiledSkyData) -> u64 {
    let mut hasher = DefaultHasher::new();
    tiled.hash(&mut hasher);
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

    let face_size = (src_height / 2).clamp(256, 2048);
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
    cameras: Query<Entity, (With<Camera3d>, Without<renzora::IsolatedCamera>)>,
    mut camera_query: Query<&mut Camera, (With<Camera3d>, Without<renzora::IsolatedCamera>)>,
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
            skybox_state.tiled_cubemap_handle = None;
            skybox_state.tiled_params_hash = 0;
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
                    commands.entity(camera_entity).try_insert(Skybox {
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
                        commands.entity(camera_entity).try_insert(Skybox {
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
                for cam in cameras.iter() {
                    commands.entity(cam).remove::<Skybox>();
                }
                for mut camera in camera_query.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(Color::srgb(0.3, 0.3, 0.35));
                }
                *skybox_state = SkyboxState::default();
            }
        }
        SkyMode::Tiled => {
            let tiled = &skybox.tiled_sky;
            let new_hash = hash_tiled_params(tiled);

            skybox_state.current_path = None;
            skybox_state.equirect_handle = None;
            skybox_state.cubemap_handle = None;
            skybox_state.conversion_pending = false;
            skybox_state.procedural_cubemap_handle = None;
            skybox_state.procedural_params_hash = 0;

            if skybox_state.tiled_params_hash != new_hash
                || skybox_state.tiled_cubemap_handle.is_none()
            {
                let cubemap = generate_tiled_cubemap(tiled);
                let handle = images.add(cubemap);
                skybox_state.tiled_cubemap_handle = Some(handle);
                skybox_state.tiled_params_hash = new_hash;
            }

            if let Some(ref cubemap_handle) = skybox_state.tiled_cubemap_handle {
                for camera_entity in cameras.iter() {
                    commands.entity(camera_entity).try_insert(Skybox {
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
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct SkyboxPlugin;

impl Plugin for SkyboxPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SkyboxPlugin");
        app.register_type::<SkyMode>()
            .register_type::<ProceduralSkyData>()
            .register_type::<PanoramaSkyData>()
            .register_type::<TiledSkyData>()
            .register_type::<SkyboxData>()
            .init_resource::<SkyboxState>()
            .add_systems(Update, sync_skybox);
    }
}

renzora::add!(SkyboxPlugin);
