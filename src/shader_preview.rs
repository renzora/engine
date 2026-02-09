//! Shader preview rendering pipeline
//!
//! Provides a GPU render-to-texture preview for WGSL shaders.
//! Supports both fragment shaders (via Bevy's Material trait) and
//! compute shaders (via a custom render graph node that writes to
//! a storage texture).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::{ExtendedMaterial, Material, MaterialExtension, MaterialPlugin, StandardMaterial};
use bevy::render::render_resource::{
    AsBindGroup, Extent3d,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::shader::ShaderRef;
use bevy::camera::ScalingMode;
use bevy_egui::egui::TextureId;
use bevy_egui::{EguiContexts, EguiTextureHandle};

use crate::core::{AppState, DockingState};
use crate::scene::EditorOnly;
use crate::ui::docking::PanelId;

/// Render layer for shader preview (isolated from main scene)
pub const SHADER_PREVIEW_RENDER_LAYER: usize = 6;

/// Preview texture resolution
const PREVIEW_SIZE: u32 = 512;

/// UUID handle for the dynamic preview fragment shader
const PREVIEW_FRAG_SHADER_HANDLE: Handle<Shader> = Handle::Uuid(
    bevy::asset::uuid::Uuid::from_u128(0xDEAD_CAFE_BEEF_F00D_1234_5678_ABCD_EF00),
    std::marker::PhantomData,
);

/// UUID handle for the dynamic preview compute shader
const PREVIEW_COMPUTE_SHADER_HANDLE: Handle<Shader> = Handle::Uuid(
    bevy::asset::uuid::Uuid::from_u128(0xDEAD_CAFE_BEEF_F00D_1234_5678_ABCD_EF01),
    std::marker::PhantomData,
);

/// UUID handle for the dynamic PBR preview fragment shader
const PREVIEW_PBR_FRAG_SHADER_HANDLE: Handle<Shader> = Handle::Uuid(
    bevy::asset::uuid::Uuid::from_u128(0xDEAD_CAFE_BEEF_F00D_1234_5678_ABCD_EF02),
    std::marker::PhantomData,
);

// ---------------------------------------------------------------------------
// State types
// ---------------------------------------------------------------------------

/// Detected shader type
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ShaderType {
    #[default]
    Fragment,
    /// Fragment shader that uses PBR lighting (needs StandardMaterial bindings)
    PbrFragment,
    Compute,
}

/// Shader compilation status
#[derive(Debug, Clone, Default)]
pub enum ShaderCompileStatus {
    /// No shader loaded
    #[default]
    Idle,
    /// Shader is being compiled by the GPU pipeline
    Compiling,
    /// Shader compiled successfully
    Compiled,
    /// Shader compilation failed with error message
    Error(String),
}

/// Resource tracking shader preview state
#[derive(Resource, Default)]
pub struct ShaderPreviewState {
    /// Current compilation status
    pub compile_status: ShaderCompileStatus,
    /// Whether to auto-recompile when the active script changes
    pub auto_recompile: bool,
    /// Whether a recompile has been requested
    pub needs_recompile: bool,
    /// Path of the currently previewed shader (if any)
    pub active_shader_path: Option<String>,
    /// Elapsed time for animated shaders (seconds)
    pub elapsed_time: f32,
    /// The WGSL source that was last validated
    pub last_validated_source: String,
    /// Whether the GPU shader needs updating after successful validation
    pub needs_gpu_update: bool,
    /// Detected shader type for the current source
    pub shader_type: ShaderType,
    /// Parsed workgroup size for compute shaders
    pub workgroup_size: [u32; 2],
}

/// Shared pipeline compilation status, communicated from render world to main world
/// via `Arc<Mutex<>>`.
#[derive(Resource, Clone)]
pub struct SharedPipelineStatus {
    pub inner: Arc<Mutex<Option<ShaderCompileStatus>>>,
}

impl Default for SharedPipelineStatus {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

// ---------------------------------------------------------------------------
// Render resources
// ---------------------------------------------------------------------------

/// Resource holding the shader preview render target and entities
#[derive(Resource)]
pub struct ShaderPreviewRender {
    pub image_handle: Handle<Image>,
    pub texture_id: Option<TextureId>,
    pub material_handle: Handle<ShaderPreviewMaterial>,
    pub pbr_material_handle: Handle<ExtendedMaterial<StandardMaterial, PbrPreviewExtension>>,
    /// Storage texture for compute shader output
    pub compute_image_handle: Handle<Image>,
    pub compute_texture_id: Option<TextureId>,
}

impl Default for ShaderPreviewRender {
    fn default() -> Self {
        Self {
            image_handle: Handle::default(),
            texture_id: None,
            material_handle: Handle::default(),
            pbr_material_handle: Handle::default(),
            compute_image_handle: Handle::default(),
            compute_texture_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Material
// ---------------------------------------------------------------------------

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct ShaderPreviewMaterial {
    /// x = elapsed time, yzw = reserved
    #[uniform(0)]
    pub params: Vec4,
}

impl Material for ShaderPreviewMaterial {
    fn fragment_shader() -> ShaderRef {
        PREVIEW_FRAG_SHADER_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

/// Extension material for PBR shader preview.
///
/// Used with `ExtendedMaterial<StandardMaterial, PbrPreviewExtension>` so that
/// PBR shaders get the full StandardMaterial bind group layout while we swap
/// only the fragment shader.
#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct PbrPreviewExtension {}

impl MaterialExtension for PbrPreviewExtension {
    fn fragment_shader() -> ShaderRef {
        PREVIEW_PBR_FRAG_SHADER_HANDLE.into()
    }
}

// ---------------------------------------------------------------------------
// Default preview shaders
// ---------------------------------------------------------------------------

const DEFAULT_PREVIEW_SHADER: &str = r#"
#import bevy_pbr::forward_io::VertexOutput

@group(3) @binding(0)
var<uniform> _preview_params: vec4<f32>;

fn palette(t: f32) -> vec3<f32> {
    let a = vec3<f32>(0.5, 0.5, 0.5);
    let b = vec3<f32>(0.5, 0.5, 0.5);
    let c = vec3<f32>(1.0, 1.0, 1.0);
    let d = vec3<f32>(0.263, 0.416, 0.557);
    return a + b * cos(6.28318 * (c * t + d));
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let time = _preview_params.x;
    let uv = mesh.uv;
    var v = 0.0;
    v += sin(uv.x * 6.0 + time);
    v += sin(uv.y * 4.0 + time * 0.7);
    v += sin((uv.x + uv.y) * 3.0 + time * 0.5);
    v += sin(length(uv - 0.5) * 5.0 - time * 1.3);
    v *= 0.25;
    let color = palette(v + time * 0.1);
    return vec4<f32>(color, 1.0);
}
"#;

const DEFAULT_COMPUTE_SHADER: &str = r#"
@group(0) @binding(0) var<uniform> _preview_params: vec4<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let time = _preview_params.x;
    let res = vec2<f32>(_preview_params.y, _preview_params.z);
    let uv = vec2<f32>(f32(id.x), f32(id.y)) / res;
    let cx = uv.x - 0.5;
    let cy = uv.y - 0.5;
    let d = sqrt(cx * cx + cy * cy);
    let r = 0.5 + 0.5 * sin(d * 20.0 - time * 3.0);
    let g = 0.5 + 0.5 * sin(d * 20.0 - time * 3.0 + 2.094);
    let b = 0.5 + 0.5 * sin(d * 20.0 - time * 3.0 + 4.189);
    let color = vec3<f32>(r, g, b);
    textureStore(output_texture, id.xy, vec4<f32>(color, 1.0));
}
"#;

const DEFAULT_PBR_PREVIEW_SHADER: &str = r#"
#import bevy_pbr::{
    pbr_functions::apply_pbr_lighting,
    pbr_types::PbrInput,
    pbr_types::pbr_input_new,
    mesh_view_bindings::view,
    mesh_view_bindings::globals,
    forward_io::VertexOutput,
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.material.base_color = vec4<f32>(0.8, 0.2, 0.3, 1.0);
    pbr_input.material.metallic = 0.0;
    pbr_input.material.perceptual_roughness = 0.5;
    pbr_input.diffuse_occlusion = vec3<f32>(1.0);
    pbr_input.world_normal = normalize(in.world_normal);
    pbr_input.world_position = in.world_position;
    pbr_input.frag_coord = in.position;

    var color = apply_pbr_lighting(pbr_input);
    color.a = 1.0;
    return color;
}
"#;

// ---------------------------------------------------------------------------
// Shader type detection
// ---------------------------------------------------------------------------

/// Detect whether a WGSL source is a compute, fragment, or PBR fragment shader.
///
/// Returns `None` for vertex-only shaders that cannot be previewed.
pub fn detect_shader_type(source: &str) -> Option<ShaderType> {
    if source.contains("@compute") {
        Some(ShaderType::Compute)
    } else if source.contains("@fragment") {
        // PBR shaders use apply_pbr_lighting (or the old pbr() function)
        if source.contains("apply_pbr_lighting") || source.contains("pbr_functions::pbr") {
            Some(ShaderType::PbrFragment)
        } else {
            Some(ShaderType::Fragment)
        }
    } else {
        // Vertex-only shader or no recognised entry point
        None
    }
}

/// Parse `@workgroup_size(X, Y)` from source, defaulting to [8, 8]
pub fn parse_workgroup_size(source: &str) -> [u32; 2] {
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find("@workgroup_size(") {
            let after = &trimmed[start + "@workgroup_size(".len()..];
            if let Some(end) = after.find(')') {
                let args = &after[..end];
                let parts: Vec<&str> = args.split(',').collect();
                let x = parts.first()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(8);
                let y = parts.get(1)
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(x); // Default Y to X if only one dimension given
                return [x, y];
            }
        }
    }
    [8, 8]
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Marker for the shader preview camera
#[derive(Component)]
struct ShaderPreviewCamera;

/// Marker for the shader preview quad (simple material)
#[derive(Component)]
struct ShaderPreviewQuad;

/// Marker for the PBR shader preview quad (ExtendedMaterial)
#[derive(Component)]
struct ShaderPreviewPbrQuad;

/// Sets up the shader preview render target, camera, and quad
pub fn setup_shader_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut preview_materials: ResMut<Assets<ShaderPreviewMaterial>>,
    mut pbr_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, PbrPreviewExtension>>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut render: ResMut<ShaderPreviewRender>,
) {
    // Create render target image for fragment shaders (512x512)
    let size = Extent3d {
        width: PREVIEW_SIZE,
        height: PREVIEW_SIZE,
        depth_or_array_layers: 1,
    };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("shader_preview_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);
    let image_handle = images.add(image);
    render.image_handle = image_handle.clone();

    // Create storage texture for compute shaders (512x512, Rgba8Unorm)
    let mut compute_image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("shader_preview_compute_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        ..default()
    };
    compute_image.resize(size);
    render.compute_image_handle = images.add(compute_image);

    // Insert default shaders
    let _ = shaders.insert(
        &PREVIEW_FRAG_SHADER_HANDLE,
        Shader::from_wgsl(DEFAULT_PREVIEW_SHADER, file!()),
    );
    let _ = shaders.insert(
        &PREVIEW_COMPUTE_SHADER_HANDLE,
        Shader::from_wgsl(DEFAULT_COMPUTE_SHADER, file!()),
    );
    let _ = shaders.insert(
        &PREVIEW_PBR_FRAG_SHADER_HANDLE,
        Shader::from_wgsl(DEFAULT_PBR_PREVIEW_SHADER, file!()),
    );

    // Create simple preview material
    let material_handle = preview_materials.add(ShaderPreviewMaterial { params: Vec4::ZERO });
    render.material_handle = material_handle.clone();

    // Create PBR preview material (StandardMaterial base + our fragment shader extension)
    let pbr_material_handle = pbr_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            metallic: 0.0,
            perceptual_roughness: 0.5,
            ..default()
        },
        extension: PbrPreviewExtension {},
    });
    render.pbr_material_handle = pbr_material_handle.clone();

    let quad_mesh = meshes.add(Rectangle::new(1.0, 1.0));

    // Spawn orthographic camera looking at the preview quad
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            order: -3,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: 1.0,
                height: 1.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Tonemapping::None,
        RenderLayers::layer(SHADER_PREVIEW_RENDER_LAYER),
        ShaderPreviewCamera,
        EditorOnly,
        Name::new("Shader Preview Camera"),
    ));

    // Spawn a 1x1 rectangle for simple fragment shaders
    commands.spawn((
        Mesh3d(quad_mesh.clone()),
        MeshMaterial3d(material_handle),
        Transform::default(),
        RenderLayers::layer(SHADER_PREVIEW_RENDER_LAYER),
        ShaderPreviewQuad,
        EditorOnly,
        Name::new("Shader Preview Quad"),
    ));

    // Spawn a 1x1 rectangle for PBR fragment shaders (initially hidden)
    commands.spawn((
        Mesh3d(quad_mesh),
        MeshMaterial3d(pbr_material_handle),
        Transform::default(),
        Visibility::Hidden,
        RenderLayers::layer(SHADER_PREVIEW_RENDER_LAYER),
        ShaderPreviewPbrQuad,
        EditorOnly,
        Name::new("Shader Preview PBR Quad"),
    ));

    // Directional light for PBR preview (on preview render layer)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance: 12000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.5, 0.0)),
        RenderLayers::layer(SHADER_PREVIEW_RENDER_LAYER),
        EditorOnly,
        Name::new("Shader Preview Light"),
    ));

    info!("Shader preview system initialized");
}

/// Register the shader preview texture with egui
pub fn register_shader_preview_texture(
    mut contexts: EguiContexts,
    mut render: ResMut<ShaderPreviewRender>,
) {
    if render.texture_id.is_none() && render.image_handle != Handle::default() {
        let texture_id = contexts.add_image(EguiTextureHandle::Weak(render.image_handle.id()));
        render.texture_id = Some(texture_id);
    }
    if render.compute_texture_id.is_none() && render.compute_image_handle != Handle::default() {
        let texture_id = contexts.add_image(EguiTextureHandle::Weak(render.compute_image_handle.id()));
        render.compute_texture_id = Some(texture_id);
    }
}

/// Update shader preview: tick time uniform and apply shader changes
pub fn update_shader_preview(
    time: Res<Time>,
    mut preview_state: ResMut<ShaderPreviewState>,
    render: Res<ShaderPreviewRender>,
    mut materials: ResMut<Assets<ShaderPreviewMaterial>>,
    mut shaders: ResMut<Assets<Shader>>,
    shared_status: Res<SharedPipelineStatus>,
    mut simple_quads: Query<&mut Visibility, (With<ShaderPreviewQuad>, Without<ShaderPreviewPbrQuad>)>,
    mut pbr_quads: Query<&mut Visibility, (With<ShaderPreviewPbrQuad>, Without<ShaderPreviewQuad>)>,
    mut cameras: Query<(&mut Camera, &mut Tonemapping), With<ShaderPreviewCamera>>,
    docking: Res<DockingState>,
) {
    // Deactivate camera when the shader preview panel is closed
    let panel_open = docking.dock_tree.contains_panel(&PanelId::ShaderPreview);
    if !panel_open {
        for (mut camera, _) in cameras.iter_mut() {
            camera.is_active = false;
        }
        return;
    }

    // Update elapsed time
    preview_state.elapsed_time += time.delta_secs();

    // Update material time uniform (params.x = time)
    if let Some(mat) = materials.get_mut(&render.material_handle) {
        mat.params.x = preview_state.elapsed_time;
    }

    // Apply shader update if flagged
    if preview_state.needs_gpu_update {
        preview_state.needs_gpu_update = false;

        // Clear any previous pipeline status — render world will write the new result
        if let Ok(mut status) = shared_status.inner.lock() {
            *status = None;
        }

        // Toggle entity visibility and camera tonemapping based on shader type
        let show_simple = preview_state.shader_type == ShaderType::Fragment;
        let show_pbr = preview_state.shader_type == ShaderType::PbrFragment;
        for mut vis in simple_quads.iter_mut() {
            *vis = if show_simple { Visibility::Visible } else { Visibility::Hidden };
        }
        for mut vis in pbr_quads.iter_mut() {
            *vis = if show_pbr { Visibility::Visible } else { Visibility::Hidden };
        }
        for (mut camera, mut tonemapping) in cameras.iter_mut() {
            // Activate camera when a shader is submitted for preview
            camera.is_active = true;
            *tonemapping = if show_pbr {
                Tonemapping::TonyMcMapface
            } else {
                Tonemapping::None
            };
        }

        match preview_state.shader_type {
            ShaderType::Fragment => {
                let transformed = transform_shader_for_preview(&preview_state.last_validated_source);
                let _ = shaders.insert(
                    &PREVIEW_FRAG_SHADER_HANDLE,
                    Shader::from_wgsl(transformed, file!()),
                );
            }
            ShaderType::PbrFragment => {
                // Push source directly — no transform needed, Bevy's naga_oil
                // resolves #import bevy_pbr::... and ExtendedMaterial provides
                // the StandardMaterial bind groups.
                let _ = shaders.insert(
                    &PREVIEW_PBR_FRAG_SHADER_HANDLE,
                    Shader::from_wgsl(
                        preview_state.last_validated_source.clone(),
                        file!(),
                    ),
                );
            }
            ShaderType::Compute => {
                let transformed = transform_shader_for_compute(&preview_state.last_validated_source);
                let _ = shaders.insert(
                    &PREVIEW_COMPUTE_SHADER_HANDLE,
                    Shader::from_wgsl(transformed, file!()),
                );
            }
        }
    }
}

/// Poll the shared pipeline status from the render world and update `ShaderPreviewState`
pub fn poll_pipeline_status(
    mut preview_state: ResMut<ShaderPreviewState>,
    shared_status: Res<SharedPipelineStatus>,
) {
    if let Ok(mut status) = shared_status.inner.lock() {
        if let Some(new_status) = status.take() {
            preview_state.compile_status = new_status;
        }
    }
}

// ---------------------------------------------------------------------------
// Shader transformation (Fragment)
// ---------------------------------------------------------------------------

/// Transform user WGSL into a Bevy-compatible fragment shader.
///
/// Removes the user's `struct VertexOutput/VertexInput` and `@vertex fn vertex`
/// definitions, replacing them with Bevy's standard vertex output import.
/// Replaces `@group(2)` bindings with the material's `@group(2) @binding(0)` params
/// uniform and provides a `time` alias. Helper functions and the `@fragment`
/// function are preserved.
pub fn transform_shader_for_preview(source: &str) -> String {
    // Check whether the source already imports VertexOutput (single-line,
    // grouped `#import bevy_pbr::{ ... forward_io::VertexOutput ... }`, or
    // module-level `#import bevy_pbr::forward_io`).  If so, we keep the
    // user's imports and skip injecting our own to avoid duplicates.
    let already_imports_vertex_output = source.contains("forward_io::VertexOutput")
        || source.contains("#import bevy_pbr::forward_io\n")
        || source.contains("#import bevy_pbr::forward_io ");

    let mut result = String::new();
    if !already_imports_vertex_output {
        result.push_str("#import bevy_pbr::forward_io::VertexOutput\n\n");
    }
    result.push_str("@group(3) @binding(0)\nvar<uniform> _preview_params: vec4<f32>;\n\n");

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    let mut injected_time_alias = false;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Skip struct VertexOutput { ... }; and struct VertexInput { ... };
        if trimmed.starts_with("struct VertexOutput")
            || trimmed.starts_with("struct VertexInput")
        {
            let mut brace_depth: i32 = 0;
            loop {
                for ch in lines[i].chars() {
                    if ch == '{' { brace_depth += 1; }
                    if ch == '}' { brace_depth -= 1; }
                }
                i += 1;
                if brace_depth <= 0 || i >= lines.len() { break; }
            }
            continue;
        }

        // Skip @vertex fn vertex(...) { ... }
        if trimmed.starts_with("@vertex") {
            let mut brace_depth: i32 = 0;
            let mut found_open = false;
            loop {
                for ch in lines[i].chars() {
                    if ch == '{' { brace_depth += 1; found_open = true; }
                    if ch == '}' { brace_depth -= 1; }
                }
                i += 1;
                if (found_open && brace_depth <= 0) || i >= lines.len() { break; }
            }
            continue;
        }

        // Skip @group(0), @group(1), @group(2), and @group(3) bindings (we provide our own)
        if trimmed.starts_with("@group(0)")
            || trimmed.starts_with("@group(1)")
            || trimmed.starts_with("@group(2)")
            || trimmed.starts_with("@group(3)")
        {
            // Skip the binding line and the next line (var declaration)
            i += 1;
            if i < lines.len() && lines[i].trim().starts_with("var") {
                i += 1;
            }
            continue;
        }

        // Skip struct View/Mesh (custom view/mesh types conflict with Bevy)
        if trimmed.starts_with("struct View")
            || trimmed.starts_with("struct Mesh")
        {
            let mut brace_depth: i32 = 0;
            loop {
                for ch in lines[i].chars() {
                    if ch == '{' { brace_depth += 1; }
                    if ch == '}' { brace_depth -= 1; }
                }
                i += 1;
                if brace_depth <= 0 || i >= lines.len() { break; }
            }
            continue;
        }

        // Inject `let time = _preview_params.x;` at the start of the @fragment function body,
        // but only if the shader doesn't already define its own `time` variable.
        if trimmed.starts_with("@fragment") && !injected_time_alias {
            let already_defines_time = source.contains("let time ")
                || source.contains("let time=")
                || source.contains("var time ")
                || source.contains("var time=");

            // Write the @fragment line
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
            // Write lines until we find the opening brace
            while i < lines.len() {
                result.push_str(lines[i]);
                result.push('\n');
                if lines[i].contains('{') {
                    i += 1;
                    if !already_defines_time {
                        result.push_str("    let time = _preview_params.x;\n");
                    }
                    injected_time_alias = true;
                    break;
                }
                i += 1;
            }
            continue;
        }

        result.push_str(lines[i]);
        result.push('\n');
        i += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// Shader transformation (Compute)
// ---------------------------------------------------------------------------

/// Transform user WGSL into a compute shader with preview bindings.
///
/// Strips user `@group` bindings, injects the preview params uniform and
/// output storage texture at group(0), and injects `let time = ...` at
/// the start of the `@compute` function body.
pub fn transform_shader_for_compute(source: &str) -> String {
    let mut result = String::from(
        "@group(0) @binding(0) var<uniform> _preview_params: vec4<f32>;\n\
         @group(0) @binding(1) var output_texture: texture_storage_2d<rgba8unorm, write>;\n\n",
    );

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    let mut injected_time_alias = false;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Skip @group bindings (we provide our own)
        if trimmed.starts_with("@group(") {
            i += 1;
            if i < lines.len() && lines[i].trim().starts_with("var") {
                i += 1;
            }
            continue;
        }

        // Inject `let time = _preview_params.x;` at the start of the @compute function body
        if trimmed.starts_with("@compute") && !injected_time_alias {
            // Write the @compute line (may include @workgroup_size on same line)
            result.push_str(lines[i]);
            result.push('\n');
            i += 1;
            // Write lines until we find the opening brace
            while i < lines.len() {
                let line_trimmed = lines[i].trim();
                result.push_str(lines[i]);
                result.push('\n');
                if lines[i].contains('{') {
                    i += 1;
                    result.push_str("    let time = _preview_params.x;\n");
                    result.push_str("    let _resolution = vec2<f32>(_preview_params.y, _preview_params.z);\n");
                    injected_time_alias = true;
                    break;
                }
                // If we see @workgroup_size on a separate line, keep going
                if line_trimmed.starts_with("@workgroup_size") || line_trimmed.starts_with("fn ") {
                    i += 1;
                    continue;
                }
                i += 1;
            }
            continue;
        }

        result.push_str(lines[i]);
        result.push('\n');
        i += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// Compute shader render infrastructure
// ---------------------------------------------------------------------------

mod compute {
    use bevy::prelude::*;
    use bevy::render::render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
        BindGroupLayoutEntry,
        BindingType, BufferBindingType, BufferInitDescriptor, BufferUsages,
        CachedComputePipelineId, CachedPipelineState, ComputePipelineDescriptor,
        PipelineCache, PipelineDescriptor, ShaderStages, StorageTextureAccess,
        TextureFormat, TextureViewDimension,
    };
    use bevy::render::renderer::{RenderDevice, RenderQueue};
    use bevy::render::render_asset::RenderAssets;
    use bevy::render::texture::GpuImage;
    use bevy::render::render_graph::{Node, NodeRunError, RenderGraphContext};
    use bevy::render::renderer::RenderContext;
    use bevy::render::Extract;

    use super::{
        ShaderPreviewState, ShaderPreviewRender, SharedPipelineStatus, ShaderCompileStatus,
        PREVIEW_COMPUTE_SHADER_HANDLE, PREVIEW_FRAG_SHADER_HANDLE,
        PREVIEW_PBR_FRAG_SHADER_HANDLE, PREVIEW_SIZE,
    };

    /// Helper to convert f32 slice to bytes
    fn f32_slice_as_bytes(data: &[f32]) -> &[u8] {
        unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) }
    }

    /// Extracted compute preview data (main world -> render world)
    #[derive(Resource, Default)]
    pub struct ComputePreviewExtracted {
        pub shader_type: super::ShaderType,
        pub time: f32,
        pub width: u32,
        pub height: u32,
        pub compute_image_id: Option<bevy::asset::AssetId<Image>>,
        pub workgroup_size: [u32; 2],
    }

    /// Compute preview pipeline resources (render world)
    #[derive(Resource)]
    pub struct ComputePreviewPipeline {
        pub bind_group_layout: BindGroupLayout,
        pub pipeline_id: CachedComputePipelineId,
        pub params_buffer: bevy::render::render_resource::Buffer,
    }

    impl FromWorld for ComputePreviewPipeline {
        fn from_world(world: &mut World) -> Self {
            let render_device = world.resource::<RenderDevice>();

            let layout_entries = [
                // binding 0: uniform params
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(std::num::NonZero::new(16).unwrap()), // vec4<f32>
                    },
                    count: None,
                },
                // binding 1: storage texture
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ];

            let bind_group_layout = render_device.create_bind_group_layout(
                "compute_preview_bind_group_layout",
                &layout_entries,
            );

            let bind_group_layout_descriptor = BindGroupLayoutDescriptor::new(
                "compute_preview_bind_group_layout",
                &layout_entries,
            );

            // Create initial params buffer
            let initial_data = [0.0f32, PREVIEW_SIZE as f32, PREVIEW_SIZE as f32, 0.0f32];
            let params_buffer = render_device.create_buffer_with_data(
                &BufferInitDescriptor {
                    label: Some("compute_preview_params"),
                    contents: f32_slice_as_bytes(&initial_data),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                },
            );

            let pipeline_cache = world.resource::<PipelineCache>();
            let pipeline_id = pipeline_cache.queue_compute_pipeline(
                ComputePipelineDescriptor {
                    label: Some("compute_preview_pipeline".into()),
                    layout: vec![bind_group_layout_descriptor],
                    shader: PREVIEW_COMPUTE_SHADER_HANDLE,
                    shader_defs: vec![],
                    entry_point: Some("main".into()),
                    push_constant_ranges: vec![],
                    zero_initialize_workgroup_memory: true,
                },
            );

            Self {
                bind_group_layout,
                pipeline_id,
                params_buffer,
            }
        }
    }

    /// Bind group for compute preview (render world)
    #[derive(Resource, Default)]
    pub struct ComputePreviewBindGroup {
        pub bind_group: Option<BindGroup>,
    }

    /// Extract compute preview data from main world to render world
    pub fn extract_compute_preview(
        preview_state: Extract<Res<ShaderPreviewState>>,
        preview_render: Extract<Res<ShaderPreviewRender>>,
        mut extracted: ResMut<ComputePreviewExtracted>,
    ) {
        extracted.shader_type = preview_state.shader_type;
        extracted.time = preview_state.elapsed_time;
        extracted.width = PREVIEW_SIZE;
        extracted.height = PREVIEW_SIZE;
        extracted.workgroup_size = preview_state.workgroup_size;
        extracted.compute_image_id = if preview_render.compute_image_handle != Handle::default() {
            Some(preview_render.compute_image_handle.id())
        } else {
            None
        };
    }

    /// Prepare compute preview bind groups and update params buffer
    pub fn prepare_compute_preview(
        render_device: Res<RenderDevice>,
        render_queue: Res<RenderQueue>,
        gpu_images: Res<RenderAssets<GpuImage>>,
        extracted: Res<ComputePreviewExtracted>,
        pipeline: Res<ComputePreviewPipeline>,
        mut bind_group_res: ResMut<ComputePreviewBindGroup>,
    ) {
        if extracted.shader_type != super::ShaderType::Compute {
            bind_group_res.bind_group = None;
            return;
        }

        // Update params buffer
        let params = [extracted.time, extracted.width as f32, extracted.height as f32, 0.0f32];
        render_queue.write_buffer(&pipeline.params_buffer, 0, f32_slice_as_bytes(&params));

        // Get GPU image for the compute storage texture
        let Some(image_id) = extracted.compute_image_id else {
            bind_group_res.bind_group = None;
            return;
        };
        let Some(gpu_image) = gpu_images.get(image_id) else {
            bind_group_res.bind_group = None;
            return;
        };

        // Create bind group
        let bind_group = render_device.create_bind_group(
            "compute_preview_bind_group",
            &pipeline.bind_group_layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: pipeline.params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: bevy::render::render_resource::BindingResource::TextureView(
                        &gpu_image.texture_view,
                    ),
                },
            ],
        );

        bind_group_res.bind_group = Some(bind_group);
    }

    /// Render graph node that dispatches the compute shader
    pub struct ComputePreviewNode;

    impl FromWorld for ComputePreviewNode {
        fn from_world(_world: &mut World) -> Self {
            Self
        }
    }

    impl Node for ComputePreviewNode {
        fn run<'w>(
            &self,
            _graph: &mut RenderGraphContext,
            render_context: &mut RenderContext<'w>,
            world: &'w World,
        ) -> Result<(), NodeRunError> {
            let extracted = world.resource::<ComputePreviewExtracted>();
            if extracted.shader_type != super::ShaderType::Compute {
                return Ok(());
            }

            let bind_group_res = world.resource::<ComputePreviewBindGroup>();
            let Some(ref bind_group) = bind_group_res.bind_group else {
                return Ok(());
            };

            let pipeline_res = world.resource::<ComputePreviewPipeline>();
            let pipeline_cache = world.resource::<PipelineCache>();

            let Some(compute_pipeline) = pipeline_cache.get_compute_pipeline(pipeline_res.pipeline_id) else {
                return Ok(());
            };

            let mut pass = render_context
                .command_encoder()
                .begin_compute_pass(&bevy::render::render_resource::ComputePassDescriptor {
                    label: Some("compute_preview_pass"),
                    timestamp_writes: None,
                });

            pass.set_pipeline(compute_pipeline);
            pass.set_bind_group(0, bind_group, &[]);

            let wg = extracted.workgroup_size;
            let dispatch_x = (extracted.width + wg[0] - 1) / wg[0];
            let dispatch_y = (extracted.height + wg[1] - 1) / wg[1];
            pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);

            Ok(())
        }
    }

    /// Render-world system that checks pipeline compilation status and writes
    /// results to `SharedPipelineStatus` so the main world can read them.
    pub fn check_pipeline_status(
        extracted: Res<ComputePreviewExtracted>,
        pipeline_cache: Res<PipelineCache>,
        compute_pipeline: Option<Res<ComputePreviewPipeline>>,
        shared_status: Res<SharedPipelineStatus>,
    ) {
        let result = match extracted.shader_type {
            super::ShaderType::Compute => {
                let Some(ref pipe) = compute_pipeline else { return };
                match pipeline_cache.get_compute_pipeline_state(pipe.pipeline_id) {
                    CachedPipelineState::Ok(_) => Some(ShaderCompileStatus::Compiled),
                    CachedPipelineState::Err(err) => {
                        Some(ShaderCompileStatus::Error(format!("{}", err)))
                    }
                    // Queued or Creating — still compiling
                    _ => None,
                }
            }
            super::ShaderType::Fragment | super::ShaderType::PbrFragment => {
                // Find the render pipeline whose fragment shader matches our handle
                let target_id = if extracted.shader_type == super::ShaderType::PbrFragment {
                    PREVIEW_PBR_FRAG_SHADER_HANDLE.id()
                } else {
                    PREVIEW_FRAG_SHADER_HANDLE.id()
                };
                let mut found = None;
                for cached in pipeline_cache.pipelines() {
                    if let PipelineDescriptor::RenderPipelineDescriptor(ref desc) = cached.descriptor {
                        let matches = desc.fragment.as_ref()
                            .map(|f| f.shader.id() == target_id)
                            .unwrap_or(false);
                        if matches {
                            found = Some(&cached.state);
                            break;
                        }
                    }
                }
                match found {
                    Some(CachedPipelineState::Ok(_)) => Some(ShaderCompileStatus::Compiled),
                    Some(CachedPipelineState::Err(err)) => {
                        Some(ShaderCompileStatus::Error(format!("{}", err)))
                    }
                    // Queued, Creating, or not yet queued
                    _ => None,
                }
            }
        };

        if let Some(status) = result {
            if let Ok(mut shared) = shared_status.inner.lock() {
                *shared = Some(status);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ShaderPreviewPlugin;

impl Plugin for ShaderPreviewPlugin {
    fn build(&self, app: &mut App) {
        use bevy::render::render_graph::RenderGraphExt;
        use bevy::render::{Render, RenderApp, RenderSystems, ExtractSchedule};

        let shared_status = SharedPipelineStatus::default();

        app.add_plugins((
                MaterialPlugin::<ShaderPreviewMaterial>::default(),
                MaterialPlugin::<ExtendedMaterial<StandardMaterial, PbrPreviewExtension>>::default(),
            ))
            .init_resource::<ShaderPreviewState>()
            .init_resource::<ShaderPreviewRender>()
            .insert_resource(shared_status.clone())
            .add_systems(OnEnter(AppState::Editor), setup_shader_preview)
            .add_systems(
                Update,
                (
                    update_shader_preview,
                    poll_pipeline_status,
                ).chain().run_if(in_state(AppState::Editor)),
            );

        // Configure render app for compute shader support
        // Note: ComputePreviewPipeline requires RenderDevice, which is only
        // available in finish(), so we init the non-dependent resources here.
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(shared_status)
                .init_resource::<compute::ComputePreviewExtracted>()
                .init_resource::<compute::ComputePreviewBindGroup>()
                .add_systems(ExtractSchedule, compute::extract_compute_preview)
                .add_systems(
                    Render,
                    (
                        compute::prepare_compute_preview.in_set(RenderSystems::Prepare),
                        compute::check_pipeline_status.in_set(RenderSystems::Prepare),
                    ),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
        use bevy::render::render_graph::{RenderGraphExt, RenderLabel};
        use bevy::render::RenderApp;

        // RenderDevice is now available, so we can init ComputePreviewPipeline
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            #[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
            struct ComputePreviewPass;

            render_app
                .init_resource::<compute::ComputePreviewPipeline>()
                .add_render_graph_node::<compute::ComputePreviewNode>(
                    Core3d,
                    ComputePreviewPass,
                )
                .add_render_graph_edge(Core3d, ComputePreviewPass, Node3d::StartMainPass);
        }
    }
}
