//! Runtime code shader material — Bevy `Material` implementation for code-authored shaders.
//!
//! Supports **per-entity shaders**: each `CodeShaderMaterial` instance carries its own
//! `Handle<Shader>`, and `Material::specialize()` swaps the fragment shader per pipeline key.
//! This allows different entities to use different code shaders simultaneously.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::asset::uuid_handle;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin as BevyMaterialPlugin};
use bevy::render::render_resource::{
    AsBindGroup, ShaderType,
    RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::mesh::MeshVertexBufferLayoutRef;

// ── Default shader handle ───────────────────────────────────────────────────

/// Default fragment shader handle used when no code shader has been compiled yet.
pub const DEFAULT_CODE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("c0de5bad-e000-4001-aaaa-beefcafe0001");

/// Default fragment shader WGSL.
const DEFAULT_FRAG: &str = r#"
#import bevy_pbr::forward_io::VertexOutput

struct ShaderUniforms {
    time: f32,
    delta_time: f32,
    resolution: vec2<f32>,
    mouse: vec4<f32>,
    frame: u32,
    _pad: vec3<f32>,
}

@group(3) @binding(0) var<uniform> uniforms: ShaderUniforms;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.2, 0.2, 0.3, 1.0);
}
"#;

// ── Uniform buffer ──────────────────────────────────────────────────────────

/// Uniforms automatically provided to code shaders each frame.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct ShaderUniforms {
    pub time: f32,
    pub delta_time: f32,
    pub resolution: Vec2,
    pub mouse: Vec4,
    pub frame: u32,
    pub _pad: Vec3,
}

impl Default for ShaderUniforms {
    fn default() -> Self {
        Self {
            time: 0.0,
            delta_time: 0.0,
            resolution: Vec2::new(512.0, 512.0),
            mouse: Vec4::ZERO,
            frame: 0,
            _pad: Vec3::ZERO,
        }
    }
}

// ── Pipeline specialization key ─────────────────────────────────────────────

/// Per-material key that selects which fragment shader to use in the pipeline.
/// Bevy creates a separate pipeline for each unique key value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CodeShaderKey {
    pub shader_handle: Handle<Shader>,
}

impl From<&CodeShaderMaterial> for CodeShaderKey {
    fn from(material: &CodeShaderMaterial) -> Self {
        CodeShaderKey {
            shader_handle: material.shader_handle.clone(),
        }
    }
}

// ── CodeShaderMaterial ──────────────────────────────────────────────────────

/// A custom Bevy `Material` whose fragment shader is authored in code
/// (WGSL, GLSL, ShaderToy, etc.) and compiled to WGSL at runtime.
///
/// Each instance carries its own `shader_handle`, allowing different entities
/// to use different code shaders simultaneously.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(CodeShaderKey)]
pub struct CodeShaderMaterial {
    #[uniform(0)]
    pub uniforms: ShaderUniforms,

    /// The compiled fragment shader for this material instance.
    /// Each unique shader source gets its own `Handle<Shader>`.
    pub shader_handle: Handle<Shader>,

    pub alpha_mode: AlphaMode,
}

impl Default for CodeShaderMaterial {
    fn default() -> Self {
        Self {
            uniforms: ShaderUniforms::default(),
            shader_handle: DEFAULT_CODE_SHADER_HANDLE,
            alpha_mode: AlphaMode::Opaque,
        }
    }
}

impl Material for CodeShaderMaterial {
    fn fragment_shader() -> ShaderRef {
        // This is the "base" shader used for the default pipeline.
        // `specialize()` overrides it per-instance via the key.
        DEFAULT_CODE_SHADER_HANDLE.into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Swap the fragment shader to the per-material handle
        if let Some(ref mut frag) = descriptor.fragment {
            frag.shader = key.bind_group_data.shader_handle.clone();
        }
        Ok(())
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

// ── Shader cache ────────────────────────────────────────────────────────────

/// Cache of compiled shader handles, keyed by source hash.
/// Prevents creating duplicate `Handle<Shader>` assets for the same WGSL source.
#[derive(Resource, Default)]
pub struct ShaderCache {
    /// source hash → shader handle
    entries: HashMap<u64, Handle<Shader>>,
}

impl ShaderCache {
    /// Get or create a shader handle for the given WGSL source.
    pub fn get_or_insert(
        &mut self,
        wgsl: &str,
        label: &str,
        shaders: &mut Assets<Shader>,
    ) -> Handle<Shader> {
        let hash = hash_source(wgsl);

        if let Some(handle) = self.entries.get(&hash) {
            return handle.clone();
        }

        let shader = Shader::from_wgsl(wgsl.to_string(), label.to_string());
        let handle = shaders.add(shader);
        self.entries.insert(hash, handle.clone());
        handle
    }

    /// Update an existing cached shader's source (for hot-reload in the editor).
    /// Returns the handle if it was updated, None if no entry existed.
    pub fn update(
        &mut self,
        old_hash: u64,
        wgsl: &str,
        label: &str,
        shaders: &mut Assets<Shader>,
    ) -> Handle<Shader> {
        // Remove old entry
        self.entries.remove(&old_hash);
        // Insert new
        self.get_or_insert(wgsl, label, shaders)
    }
}

/// Legacy state — kept for editor preview compatibility.
#[derive(Resource, Default)]
pub struct CodeShaderState {
    pub last_wgsl_hash: Option<u64>,
    /// The handle for the editor's currently previewed shader.
    pub preview_handle: Option<Handle<Shader>>,
}

// ── Systems ─────────────────────────────────────────────────────────────────

fn setup_default_code_shader(mut shaders: ResMut<Assets<Shader>>) {
    let shader = Shader::from_wgsl(DEFAULT_FRAG.to_string(), "code_shader://default");
    let _ = shaders.insert(&DEFAULT_CODE_SHADER_HANDLE, shader);
}

/// Update time/resolution/mouse uniforms on all `CodeShaderMaterial` instances each frame.
fn update_code_shader_uniforms(
    time: Res<Time>,
    mut materials: ResMut<Assets<CodeShaderMaterial>>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();

    for (_id, mat) in materials.iter_mut() {
        mat.uniforms.time = t;
        mat.uniforms.delta_time = dt;
        mat.uniforms.frame = mat.uniforms.frame.wrapping_add(1);
    }
}

// ── Public helpers ──────────────────────────────────────────────────────────

/// Compile WGSL and cache a shader handle. Returns the handle for use in materials.
pub fn compile_and_cache(
    wgsl: &str,
    label: &str,
    cache: &mut ShaderCache,
    shaders: &mut Assets<Shader>,
) -> Handle<Shader> {
    cache.get_or_insert(wgsl, label, shaders)
}

/// Apply a compiled shader for the editor preview.
/// Updates the preview material's shader handle and caches the result.
/// Returns `true` if the shader was actually updated (different hash).
pub fn apply_code_shader(
    wgsl: &str,
    shaders: &mut Assets<Shader>,
    state: &mut CodeShaderState,
) -> bool {
    let hash = hash_source(wgsl);

    if state.last_wgsl_hash == Some(hash) {
        return false;
    }

    // For the editor preview, we update the default handle in-place
    // so the preview quad's material picks it up immediately.
    let shader = Shader::from_wgsl(wgsl.to_string(), "code_shader://preview");
    let _ = shaders.insert(&DEFAULT_CODE_SHADER_HANDLE, shader);
    state.last_wgsl_hash = Some(hash);
    true
}

fn hash_source(source: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct CodeShaderMaterialPlugin;

impl Plugin for CodeShaderMaterialPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] CodeShaderMaterialPlugin");
        app.add_plugins(BevyMaterialPlugin::<CodeShaderMaterial>::default())
            .init_resource::<CodeShaderState>()
            .init_resource::<ShaderCache>()
            .add_systems(PostStartup, setup_default_code_shader)
            .add_systems(Update, update_code_shader_uniforms);
    }
}
