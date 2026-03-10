//! Runtime material — Bevy `Material` implementation backed by compiled WGSL.
//!
//! Uses a weak shader handle so the WGSL source can be hot-swapped at runtime
//! (insert a new `Shader` at the same handle to update).

use bevy::prelude::*;
use bevy::asset::uuid_handle;
use bevy::pbr::{Material, MaterialPlugin as BevyMaterialPlugin};
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::codegen;
use crate::graph::MaterialGraph;

// ── Shader handles ──────────────────────────────────────────────────────────

/// Well-known handle for the graph material fragment shader.
/// Insert a `Shader` at this handle to update the material's fragment stage.
pub const GRAPH_MATERIAL_FRAG_HANDLE: Handle<Shader> =
    uuid_handle!("a1b2c3d4-e5f6-0001-dead-beefcafe0001");

/// Default PBR fragment shader used when no graph has been compiled yet.
const DEFAULT_FRAG: &str = r#"
#import bevy_pbr::forward_io::VertexOutput

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.8, 0.8, 0.8, 1.0);
}
"#;

// ── GraphMaterial ───────────────────────────────────────────────────────────

/// A custom Bevy `Material` whose fragment shader is generated from a
/// `MaterialGraph`. Supports up to 4 dynamically-bound textures.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GraphMaterial {
    /// Base color tint (multiplied in shader if desired).
    #[uniform(0)]
    pub base_color: LinearRgba,

    /// Texture slot 0 (e.g. base color / albedo).
    #[texture(1)]
    #[sampler(2)]
    pub texture_0: Option<Handle<Image>>,

    /// Texture slot 1 (e.g. normal map).
    #[texture(3)]
    #[sampler(4)]
    pub texture_1: Option<Handle<Image>>,

    /// Texture slot 2 (e.g. metallic/roughness).
    #[texture(5)]
    #[sampler(6)]
    pub texture_2: Option<Handle<Image>>,

    /// Texture slot 3 (e.g. emissive / AO).
    #[texture(7)]
    #[sampler(8)]
    pub texture_3: Option<Handle<Image>>,

    pub alpha_mode: AlphaMode,
}

impl Default for GraphMaterial {
    fn default() -> Self {
        Self {
            base_color: LinearRgba::WHITE,
            texture_0: None,
            texture_1: None,
            texture_2: None,
            texture_3: None,
            alpha_mode: AlphaMode::Opaque,
        }
    }
}

impl Material for GraphMaterial {
    fn fragment_shader() -> ShaderRef {
        GRAPH_MATERIAL_FRAG_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

// ── Resource tracking compiled shader state ─────────────────────────────────

/// Tracks the latest compiled WGSL so systems can detect changes and
/// re-insert the shader asset.
#[derive(Resource)]
pub struct GraphMaterialShaderState {
    /// Hash of the last WGSL source that was inserted into the shader assets.
    pub last_wgsl_hash: Option<u64>,
}

impl Default for GraphMaterialShaderState {
    fn default() -> Self {
        Self { last_wgsl_hash: None }
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct GraphMaterialPlugin;

impl Plugin for GraphMaterialPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] GraphMaterialPlugin");
        app.add_plugins(BevyMaterialPlugin::<GraphMaterial>::default())
            .init_resource::<GraphMaterialShaderState>()
            .add_systems(PostStartup, setup_default_shader);
    }
}

/// Insert the default PBR fragment shader so the material is valid from the start.
fn setup_default_shader(mut shaders: ResMut<Assets<Shader>>) {
    let shader = Shader::from_wgsl(DEFAULT_FRAG.to_string(), "graph_material://default");
    let _ = shaders.insert(&GRAPH_MATERIAL_FRAG_HANDLE, shader);
}

// ── Public helpers ──────────────────────────────────────────────────────────

/// Compile a `MaterialGraph` and insert the resulting WGSL into the shader
/// assets, replacing the previous shader at `GRAPH_MATERIAL_FRAG_HANDLE`.
///
/// Returns the `CompileResult` for error reporting.
pub fn apply_compiled_shader(
    graph: &MaterialGraph,
    shaders: &mut Assets<Shader>,
    state: &mut GraphMaterialShaderState,
) -> codegen::CompileResult {
    let result = codegen::compile(graph);

    if result.errors.is_empty() {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        result.fragment_shader.hash(&mut hasher);
        let hash = hasher.finish();

        if state.last_wgsl_hash != Some(hash) {
            let shader = Shader::from_wgsl(
                result.fragment_shader.clone(),
                "graph_material://compiled",
            );
            let _ = shaders.insert(&GRAPH_MATERIAL_FRAG_HANDLE, shader);
            state.last_wgsl_hash = Some(hash);
        }
    }

    result
}
