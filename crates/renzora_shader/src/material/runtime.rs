//! Runtime material — Bevy `Material` implementation backed by compiled WGSL.
//!
//! Uses a weak shader handle so the WGSL source can be hot-swapped at runtime
//! (insert a new `Shader` at the same handle to update).
//!
//! All 4 texture slots are always populated (with a 1x1 white fallback when unused)
//! so the pipeline layout is stable and never changes between shader swaps.

use bevy::prelude::*;
use bevy::asset::uuid_handle;
use bevy::pbr::{Material, MaterialPipelineKey, MaterialPlugin as BevyMaterialPlugin};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{AsBindGroup, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;

use super::codegen;
use super::graph::MaterialGraph;

// ── Shader handles ──────────────────────────────────────────────────────────

/// Well-known handle for the graph material fragment shader.
pub const GRAPH_MATERIAL_FRAG_HANDLE: Handle<Shader> =
    uuid_handle!("a1b2c3d4-e5f6-0001-dead-beefcafe0001");

/// Default fragment shader — declares all 4 texture slots (pipeline layout must be stable).
const DEFAULT_FRAG: &str = r#"
#import bevy_pbr::forward_io::VertexOutput

@group(3) @binding(1) var texture_0: texture_2d<f32>;
@group(3) @binding(2) var texture_0_sampler: sampler;
@group(3) @binding(3) var texture_1: texture_2d<f32>;
@group(3) @binding(4) var texture_1_sampler: sampler;
@group(3) @binding(5) var texture_2: texture_2d<f32>;
@group(3) @binding(6) var texture_2_sampler: sampler;
@group(3) @binding(7) var texture_3: texture_2d<f32>;
@group(3) @binding(8) var texture_3_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.8, 0.8, 0.8, 1.0);
}
"#;

// ── Fallback texture ────────────────────────────────────────────────────────

/// 1x1 white fallback image handle, always valid.
#[derive(Resource, Clone)]
pub struct FallbackTexture(pub Handle<Image>);

// ── GraphMaterial ───────────────────────────────────────────────────────────

/// A custom Bevy `Material` whose fragment shader is generated from a
/// `MaterialGraph`. Uses `Option<Handle<Image>>` (Bevy's standard pattern
/// for `AsBindGroup` texture layout generation) — always set to
/// `Some(fallback)` so the pipeline layout is stable across shader hot-swaps.
/// Pipeline key — carries the per-material shader handle so `specialize()`
/// can select the correct compiled WGSL for each material instance.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct GraphMaterialKey {
    pub shader: Option<Handle<Shader>>,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(GraphMaterialKey)]
pub struct GraphMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,

    #[texture(1)]
    #[sampler(2)]
    pub texture_0: Option<Handle<Image>>,

    #[texture(3)]
    #[sampler(4)]
    pub texture_1: Option<Handle<Image>>,

    #[texture(5)]
    #[sampler(6)]
    pub texture_2: Option<Handle<Image>>,

    #[texture(7)]
    #[sampler(8)]
    pub texture_3: Option<Handle<Image>>,

    pub alpha_mode: AlphaMode,

    /// Per-material shader handle — each compiled .material file gets its own
    /// shader so multiple materials can coexist without overwriting each other.
    pub shader: Option<Handle<Shader>>,
}

impl From<&GraphMaterial> for GraphMaterialKey {
    fn from(mat: &GraphMaterial) -> Self {
        Self {
            shader: mat.shader.clone(),
        }
    }
}

impl Material for GraphMaterial {
    fn fragment_shader() -> ShaderRef {
        // Default fallback — overridden per-instance via specialize()
        GRAPH_MATERIAL_FRAG_HANDLE.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Override the fragment shader with this material's compiled shader
        if let Some(ref shader_handle) = key.bind_group_data.shader {
            if let Some(ref mut frag) = descriptor.fragment {
                frag.shader = shader_handle.clone();
            }
        }
        Ok(())
    }
}

// ── Resource tracking compiled shader state ─────────────────────────────────

#[derive(Resource)]
pub struct GraphMaterialShaderState {
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

        // Create fallback texture immediately during build so it's available
        // before any GraphMaterial is created.
        let fallback_image = Image::new(
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![255, 255, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            default(),
        );
        let fallback_handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(fallback_image);
        info!("[runtime] Created fallback texture: {:?}", fallback_handle);
        app.insert_resource(FallbackTexture(fallback_handle));

        app.add_plugins(BevyMaterialPlugin::<GraphMaterial>::default())
            .init_resource::<GraphMaterialShaderState>()
            .add_systems(PostStartup, setup_default_shader);
    }
}

/// Insert the default PBR fragment shader.
fn setup_default_shader(mut shaders: ResMut<Assets<Shader>>) {
    info!("[runtime] Inserting default GraphMaterial shader with {} chars", DEFAULT_FRAG.len());
    let shader = Shader::from_wgsl(DEFAULT_FRAG.to_string(), "graph_material://default");
    let _ = shaders.insert(&GRAPH_MATERIAL_FRAG_HANDLE, shader);
}

// ── Public helpers ──────────────────────────────────────────────────────────

/// Create a `GraphMaterial` with all slots set to the fallback texture.
pub fn new_graph_material(fallback: &FallbackTexture) -> GraphMaterial {
    GraphMaterial {
        base_color: LinearRgba::WHITE,
        texture_0: Some(fallback.0.clone()),
        texture_1: Some(fallback.0.clone()),
        texture_2: Some(fallback.0.clone()),
        texture_3: Some(fallback.0.clone()),
        alpha_mode: AlphaMode::Opaque,
        shader: None,
    }
}

/// Compile a `MaterialGraph` and insert the resulting WGSL into the shader
/// assets, replacing the previous shader at `GRAPH_MATERIAL_FRAG_HANDLE`.
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
