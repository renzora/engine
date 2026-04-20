//! Surface graph extension for Bevy's `ExtendedMaterial<StandardMaterial, _>`.
//!
//! This replaces the old custom `GraphMaterial` (a standalone `Material`) with
//! an extension that **rides the full StandardMaterial pipeline**. By living on
//! top of StandardMaterial rather than parallel to it, graphs automatically get:
//!
//!   * PBR direct + indirect lighting (diffuse, specular, IBL, AO)
//!   * Image-based lighting from `EnvironmentMapLight` / `AtmosphereEnvironmentMapLight`
//!   * Scene-color refraction via Bevy's transmission pipeline when the base
//!     material has `specular_transmission > 0`
//!   * Screen-space reflections via `ScreenSpaceReflections` on the camera
//!   * Shadows, fog, atmosphere blending, tonemapping
//!
//! Per-material shaders use `Handle::Uuid(uuid, PhantomData)` — a stable id-based
//! handle that survives the `#[repr(C, packed)]` constraint on
//! `MaterialExtensionBindGroupData<B, E>`. `Handle<Shader>` contains an `Arc`
//! (non-Copy) and can't be stored in the pipeline-key struct; `Uuid` is `Copy`
//! and its derived `Clone` works inside a packed struct.
//!
//! Texture slots for procedural graphs live on this extension at bindings 100–107
//! (StandardMaterial reserves 0–99, per Bevy convention).

use std::marker::PhantomData;

use bevy::prelude::*;
use bevy::asset::uuid_handle;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use uuid::Uuid;

/// Well-known handle for the default (unmodified-StandardMaterial) extension
/// fragment shader. Used as the fallback when a material hasn't been compiled
/// yet, so the pipeline layout is always valid.
pub const SURFACE_GRAPH_EXT_DEFAULT_FRAG: Handle<Shader> =
    uuid_handle!("b1c2d3e4-f5a6-4001-aaaa-beefcafebabe");

/// Minimal extension fragment shader: `pbr_input_from_standard_material` →
/// `apply_pbr_lighting` → post-processing, with no mutations.
pub const DEFAULT_EXT_FRAG_SRC: &str = r#"
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var out: FragmentOutput;
    out.color = pbr_functions::apply_pbr_lighting(pbr_input);
    out.color = pbr_functions::main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
"#;

/// Extension half of the graph material. Holds per-material texture slots and
/// a UUID identifying the compiled shader (the actual `Handle<Shader>` is
/// reconstructed at specialize-time as `Handle::Uuid(shader_uuid, PhantomData)`).
///
/// Bindings 100–107 reserve 4 slots of (texture, sampler) alongside
/// StandardMaterial's own bindings (0–~30). Bevy merges both sets into
/// `@group(3)`, filtering duplicates.
///
/// The derives mirror Bevy's own `extended_material.rs` example:
/// `Asset + AsBindGroup + Reflect + Debug + Clone + Default` is the full set
/// required by `MaterialPlugin<ExtendedMaterial<_, Self>>`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(SurfaceGraphExtKey)]
pub struct SurfaceGraphExt {
    #[texture(100)]
    #[sampler(101)]
    pub texture_0: Option<Handle<Image>>,

    #[texture(102)]
    #[sampler(103)]
    pub texture_1: Option<Handle<Image>>,

    #[texture(104)]
    #[sampler(105)]
    pub texture_2: Option<Handle<Image>>,

    #[texture(106)]
    #[sampler(107)]
    pub texture_3: Option<Handle<Image>>,

    /// User cubemap slot (bindings 108/109). Lets a material sample a
    /// user-supplied skybox/IBL-style cube beyond Bevy's built-in env map —
    /// e.g., a baked local reflection cube, a stylized sky, a custom
    /// irradiance probe. `None` falls back to Bevy's `FallbackImage::cube`
    /// (a neutral white cube), so the pipeline layout stays valid.
    #[texture(108, dimension = "cube")]
    #[sampler(109)]
    pub cube_0: Option<Handle<Image>>,

    /// User 2D array slot (bindings 110/111). Layered texture lookup —
    /// terrain layer stacks, asset variants keyed by layer index, character
    /// body-paint masks. `None` falls back to `FallbackImage::d2_array`.
    #[texture(110, dimension = "2d_array")]
    #[sampler(111)]
    pub array_0: Option<Handle<Image>>,

    /// User 3D texture slot (bindings 112/113). Volumetric data — volume
    /// fog density, caustics LUT, precomputed scattering tables, 3D noise
    /// bakes. `None` falls back to `FallbackImage::d3`.
    #[texture(112, dimension = "3d")]
    #[sampler(113)]
    pub volume_0: Option<Handle<Image>>,

    /// UUID of this material's compiled fragment shader. The resolver inserts
    /// the Shader asset at `Handle::Uuid(shader_uuid, PhantomData)`, and
    /// `specialize()` reconstructs the handle the same way to swap the
    /// pipeline's fragment stage. `Option` because freshly-constructed
    /// materials (default factory) have no compiled shader yet and must fall
    /// back to `SURFACE_GRAPH_EXT_DEFAULT_FRAG`.
    pub shader_uuid: Option<Uuid>,
}

/// Pipeline key carried across extraction. Everything that affects the
/// compiled pipeline must live here. `Uuid` is `Copy`, which is what lets this
/// survive `MaterialExtensionBindGroupData`'s packed layout — a `Handle<Shader>`
/// (containing a non-Copy `Arc`) would make the combined Data fail to derive
/// `Clone`, which in turn breaks the `MaterialPlugin<ExtendedMaterial<_, _>>`
/// trait bound (`M::Data: PartialEq + Eq + Hash + Clone`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SurfaceGraphExtKey {
    pub shader_uuid: Option<Uuid>,
}

impl From<&SurfaceGraphExt> for SurfaceGraphExtKey {
    fn from(ext: &SurfaceGraphExt) -> Self {
        Self { shader_uuid: ext.shader_uuid }
    }
}

impl MaterialExtension for SurfaceGraphExt {
    fn fragment_shader() -> ShaderRef {
        // Default — overridden per-instance via `specialize()` when the
        // material carries a compiled shader UUID.
        SURFACE_GRAPH_EXT_DEFAULT_FRAG.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(uuid) = key.bind_group_data.shader_uuid {
            if let Some(ref mut frag) = descriptor.fragment {
                frag.shader = Handle::<Shader>::Uuid(uuid, PhantomData);
            }
        }
        Ok(())
    }
}

/// Top-level asset type users refer to. Kept as `GraphMaterial` so downstream
/// code (`MeshMaterial3d<GraphMaterial>`, handles, caches) doesn't need to change.
pub type GraphMaterial = ExtendedMaterial<StandardMaterial, SurfaceGraphExt>;

/// Convenience factory: a white StandardMaterial base + empty extension with
/// fallback-white textures in every slot. The resolver fills in textures and
/// the shader UUID after compilation; preview code does the same.
pub fn new_graph_material(fallback: &super::runtime::FallbackTexture) -> GraphMaterial {
    GraphMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Opaque,
            ..default()
        },
        extension: SurfaceGraphExt {
            texture_0: Some(fallback.0.clone()),
            texture_1: Some(fallback.0.clone()),
            texture_2: Some(fallback.0.clone()),
            texture_3: Some(fallback.0.clone()),
            // cube/array/3d stay None — Bevy's FallbackImage covers the
            // bind-group layout until the user assigns real handles.
            cube_0: None,
            array_0: None,
            volume_0: None,
            shader_uuid: None,
        },
    }
}
