//! Shader thumbnail system — renders WGSL shader previews as thumbnails for the asset browser.
//!
//! Uses its own render layer (7), camera, and quad, fully isolated from the live
//! shader preview system (layer 6). Thumbnails are processed one at a time, cached
//! to disk as PNGs under `.renzora/thumbnails/`, and displayed in the asset grid.
//! If a shader fails to compile or is not a fragment shader, it falls back to the
//! default icon.

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{Material, MaterialPlugin};
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::gpu_readback::Readback;
use bevy::shader::ShaderRef;
use bevy::camera::ScalingMode;
use bevy_egui::egui::TextureId;
use bevy_egui::{EguiContexts, EguiTextureHandle};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::path::PathBuf;

use crate::core::AppState;
use crate::project::CurrentProject;
use crate::scene::EditorOnly;
use crate::shader_preview::{detect_shader_type, transform_shader_for_preview, ShaderType};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Render layer for shader thumbnails (isolated from main scene and shader preview)
pub const SHADER_THUMBNAIL_RENDER_LAYER: usize = 7;

/// Thumbnail texture resolution
const THUMBNAIL_SIZE: u32 = 128;

/// Cache directory name for thumbnails (relative to project root)
const THUMBNAIL_CACHE_DIR: &str = ".renzora/thumbnails";

/// UUID handle for the dynamic thumbnail fragment shader
const THUMBNAIL_FRAG_SHADER_HANDLE: Handle<Shader> = Handle::Uuid(
    bevy::asset::uuid::Uuid::from_u128(0xDEAD_CAFE_BEEF_F00D_A5AD_7111_B0A1),
    std::marker::PhantomData,
);

/// Default placeholder shader for the thumbnail quad
const DEFAULT_THUMBNAIL_SHADER: &str = r#"
#import bevy_pbr::forward_io::VertexOutput

@group(3) @binding(0)
var<uniform> _preview_params: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.1, 0.1, 0.12, 1.0);
}
"#;

// ---------------------------------------------------------------------------
// Material
// ---------------------------------------------------------------------------

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ShaderThumbnailMaterial {
    /// x = frozen time, yzw = reserved
    #[uniform(0)]
    pub params: Vec4,
}

impl Material for ShaderThumbnailMaterial {
    fn fragment_shader() -> ShaderRef {
        THUMBNAIL_FRAG_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

// ---------------------------------------------------------------------------
// Cache resource
// ---------------------------------------------------------------------------

/// Status of the currently rendering shader thumbnail
#[derive(Debug, Clone)]
enum ShaderThumbnailStatus {
    /// Shader has been inserted, waiting for GPU to render
    Rendering,
}

/// Resource that manages shader thumbnail generation
#[derive(Resource)]
pub struct ShaderThumbnailCache {
    /// Queued shader paths waiting to be rendered
    pub queue: VecDeque<PathBuf>,
    /// Completed preview textures ready for display
    pub textures: HashMap<PathBuf, Handle<Image>>,
    /// Texture IDs registered with egui
    pub texture_ids: HashMap<PathBuf, TextureId>,
    /// Paths that failed (non-fragment, compile errors, etc.)
    pub failed: HashSet<PathBuf>,
    /// Paths that have been requested (to avoid duplicate requests)
    pub requested: HashSet<PathBuf>,
    /// The one shader currently being rendered
    current: Option<(PathBuf, ShaderThumbnailStatus)>,
    /// Render target for the current shader being captured (one per shader, stored as display texture)
    current_render_target: Option<Handle<Image>>,
    /// Frames waited since rendering started
    frames_waited: u32,
}

impl Default for ShaderThumbnailCache {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            textures: HashMap::new(),
            texture_ids: HashMap::new(),
            failed: HashSet::new(),
            requested: HashSet::new(),
            current: None,
            current_render_target: None,
            frames_waited: 0,
        }
    }
}

impl ShaderThumbnailCache {
    /// Request a shader thumbnail to be generated
    pub fn request_thumbnail(&mut self, path: PathBuf) {
        if self.textures.contains_key(&path)
            || self.texture_ids.contains_key(&path)
            || self.queue.contains(&path)
            || self.failed.contains(&path)
            || self.requested.contains(&path)
        {
            return;
        }
        if let Some((ref current_path, _)) = self.current {
            if current_path == &path {
                return;
            }
        }
        self.requested.insert(path.clone());
        self.queue.push_back(path);
    }

    /// Get egui texture ID for a completed shader thumbnail
    pub fn get_texture_id(&self, path: &PathBuf) -> Option<TextureId> {
        self.texture_ids.get(path).copied()
    }

    /// Check if a shader thumbnail is being processed
    pub fn is_loading(&self, path: &PathBuf) -> bool {
        self.queue.contains(path)
            || self.current.as_ref().map(|(p, _)| p == path).unwrap_or(false)
    }

    /// Check if shader thumbnail generation failed
    pub fn has_failed(&self, path: &PathBuf) -> bool {
        self.failed.contains(path)
    }
}

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct ShaderThumbnailCamera;

#[derive(Component)]
struct ShaderThumbnailQuad;

// ---------------------------------------------------------------------------
// Disk cache helpers
// ---------------------------------------------------------------------------

fn get_cache_path(shader_path: &PathBuf, project: Option<&CurrentProject>) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    shader_path.hash(&mut hasher);
    // Add a discriminator so shader thumbnails don't collide with model thumbnails
    "shader_thumb".hash(&mut hasher);
    let hash = hasher.finish();

    let base = match project {
        Some(p) => p.path.join(THUMBNAIL_CACHE_DIR),
        None => PathBuf::from(THUMBNAIL_CACHE_DIR),
    };
    base.join(format!("{:016x}.png", hash))
}

fn is_cache_valid(shader_path: &PathBuf, cache_path: &PathBuf) -> bool {
    let cache_meta = match fs::metadata(cache_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let source_meta = match fs::metadata(shader_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    match (cache_meta.modified(), source_meta.modified()) {
        (Ok(cache_time), Ok(source_time)) => cache_time >= source_time,
        _ => false,
    }
}

fn save_thumbnail_data_to_cache(
    data: &[u8],
    width: u32,
    height: u32,
    cache_path: &PathBuf,
) -> Result<(), String> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Convert BGRA to RGBA for PNG
    let mut rgba_data = Vec::with_capacity(data.len());
    for chunk in data.chunks(4) {
        if chunk.len() == 4 {
            rgba_data.push(chunk[2]); // R (was B)
            rgba_data.push(chunk[1]); // G
            rgba_data.push(chunk[0]); // B (was R)
            rgba_data.push(chunk[3]); // A
        }
    }

    let img = image::RgbaImage::from_raw(width, height, rgba_data)
        .ok_or("Failed to create image buffer")?;

    img.save(cache_path).map_err(|e| e.to_string())?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Create a fresh 128x128 render target image
fn create_render_target(images: &mut Assets<Image>) -> Handle<Image> {
    let size = Extent3d {
        width: THUMBNAIL_SIZE,
        height: THUMBNAIL_SIZE,
        depth_or_array_layers: 1,
    };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("shader_thumbnail_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);
    images.add(image)
}

/// Set up the shader thumbnail camera and quad
fn setup_shader_thumbnails(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShaderThumbnailMaterial>>,
    mut shaders: ResMut<Assets<Shader>>,
) {
    // Create an initial dummy render target so the camera has something to point at
    let initial_target = create_render_target(&mut images);

    // Insert default placeholder shader
    let _ = shaders.insert(
        &THUMBNAIL_FRAG_SHADER_HANDLE,
        Shader::from_wgsl(DEFAULT_THUMBNAIL_SHADER, file!()),
    );

    let material_handle = materials.add(ShaderThumbnailMaterial {
        params: Vec4::new(0.5, 0.0, 0.0, 0.0),
    });

    let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));

    // Spawn orthographic camera — starts inactive (not rendering)
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            order: -4,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(initial_target.into()),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 1.0,
                height: 1.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::None,
        RenderLayers::layer(SHADER_THUMBNAIL_RENDER_LAYER),
        ShaderThumbnailCamera,
        EditorOnly,
        Name::new("Shader Thumbnail Camera"),
    ));

    // Spawn 1x1 quad with material
    commands.spawn((
        Mesh3d(quad_mesh),
        MeshMaterial3d(material_handle),
        Transform::default(),
        RenderLayers::layer(SHADER_THUMBNAIL_RENDER_LAYER),
        ShaderThumbnailQuad,
        EditorOnly,
        Name::new("Shader Thumbnail Quad"),
    ));

    info!("Shader thumbnail system initialized");
}

/// Process the shader thumbnail queue — one at a time
fn process_shader_thumbnail_queue(
    mut cache: ResMut<ShaderThumbnailCache>,
    mut images: ResMut<Assets<Image>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget), With<ShaderThumbnailCamera>>,
    current_project: Option<Res<CurrentProject>>,
) {
    // If currently rendering, don't start a new one
    if cache.current.is_some() {
        return;
    }

    let project_ref = current_project.as_deref();

    // Process from queue
    while let Some(path) = cache.queue.pop_front() {
        // Check disk cache first
        let cache_path = get_cache_path(&path, project_ref);
        if is_cache_valid(&path, &cache_path) {
            // Load cached PNG
            if let Ok(data) = fs::read(&cache_path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let rgba = img.to_rgba8();
                    let (width, height) = rgba.dimensions();

                    let mut bgra_data = Vec::with_capacity((width * height * 4) as usize);
                    for pixel in rgba.pixels() {
                        bgra_data.push(pixel[2]); // B
                        bgra_data.push(pixel[1]); // G
                        bgra_data.push(pixel[0]); // R
                        bgra_data.push(pixel[3]); // A
                    }

                    let image = Image::new(
                        Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        TextureDimension::D2,
                        bgra_data,
                        TextureFormat::Bgra8UnormSrgb,
                        default(),
                    );

                    let handle = images.add(image);
                    cache.textures.insert(path, handle);
                    continue;
                }
            }
            // Cache read failed, fall through to render
        }

        // Read shader source from disk
        let source = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                cache.failed.insert(path);
                continue;
            }
        };

        // Detect shader type — only Fragment shaders get thumbnails
        let shader_type = detect_shader_type(&source);
        match shader_type {
            Some(ShaderType::Fragment) => {}
            _ => {
                // PBR, Compute, or vertex-only — skip
                cache.failed.insert(path);
                continue;
            }
        }

        // Transform the shader for preview rendering
        let transformed = transform_shader_for_preview(&source);

        // Insert the transformed shader into the thumbnail handle
        let _ = shaders.insert(
            &THUMBNAIL_FRAG_SHADER_HANDLE,
            Shader::from_wgsl(transformed, file!()),
        );

        // Create a fresh render target for this shader — the GPU will render
        // directly into this image, and we'll store the handle as the display texture
        let render_target = create_render_target(&mut images);
        cache.current_render_target = Some(render_target.clone());

        // Update camera to point at the new render target and activate it
        for (mut camera, mut target) in cameras.iter_mut() {
            *target = RenderTarget::Image(render_target.clone().into());
            camera.is_active = true;
        }

        // Mark as rendering
        cache.current = Some((path, ShaderThumbnailStatus::Rendering));
        cache.frames_waited = 0;
        return;
    }
}

/// Capture the rendered shader thumbnail after enough frames
fn capture_shader_thumbnail(
    mut commands: Commands,
    mut cache: ResMut<ShaderThumbnailCache>,
    mut cameras: Query<&mut Camera, With<ShaderThumbnailCamera>>,
    current_project: Option<Res<CurrentProject>>,
) {
    let Some((ref path, ShaderThumbnailStatus::Rendering)) = cache.current else {
        return;
    };
    let path = path.clone();

    cache.frames_waited += 1;

    // Timeout — shader probably failed to compile
    if cache.frames_waited > 15 {
        warn!("Shader thumbnail timed out for: {}", path.display());
        cache.failed.insert(path);
        cache.current = None;
        cache.current_render_target = None;
        for mut camera in cameras.iter_mut() {
            camera.is_active = false;
        }
        return;
    }

    // Wait 5 frames for GPU pipeline to compile and render
    if cache.frames_waited < 5 {
        return;
    }

    // Take the render target handle — the GPU has rendered into this texture,
    // so it can be displayed directly by egui
    let Some(render_target) = cache.current_render_target.take() else {
        return;
    };

    // Spawn readback to save PNG to disk cache (async, doesn't block)
    let cache_path = get_cache_path(&path, current_project.as_deref());
    commands
        .spawn(Readback::texture(render_target.clone()))
        .observe(
            move |trigger: On<bevy::render::gpu_readback::ReadbackComplete>,
                  mut commands: Commands| {
                let data = &trigger.event().data;
                if let Err(e) = save_thumbnail_data_to_cache(
                    data,
                    THUMBNAIL_SIZE,
                    THUMBNAIL_SIZE,
                    &cache_path,
                ) {
                    warn!("Failed to cache shader thumbnail: {}", e);
                }
                commands.entity(trigger.observer()).despawn();
            },
        );

    // Store the GPU render target directly as the display texture
    cache.textures.insert(path, render_target);

    // Deactivate camera and clear current
    cache.current = None;
    for mut camera in cameras.iter_mut() {
        camera.is_active = false;
    }
}

/// Register completed shader thumbnail textures with egui
pub fn register_shader_thumbnail_textures(
    mut contexts: EguiContexts,
    mut cache: ResMut<ShaderThumbnailCache>,
    images: Res<Assets<Image>>,
) {
    let Ok(_ctx) = contexts.ctx_mut() else {
        return;
    };

    let to_register: Vec<(PathBuf, Handle<Image>)> = cache
        .textures
        .iter()
        .filter(|(path, _)| !cache.texture_ids.contains_key(*path))
        .map(|(path, handle)| (path.clone(), handle.clone()))
        .collect();

    for (path, handle) in to_register {
        if images.contains(&handle) {
            let texture_id = contexts.add_image(EguiTextureHandle::Weak(handle.id()));
            cache.texture_ids.insert(path, texture_id);
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ShaderThumbnailPlugin;

impl Plugin for ShaderThumbnailPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShaderThumbnailMaterial>::default())
            .init_resource::<ShaderThumbnailCache>()
            .add_systems(OnEnter(AppState::Editor), setup_shader_thumbnails)
            .add_systems(
                Update,
                (process_shader_thumbnail_queue, capture_shader_thumbnail)
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}
