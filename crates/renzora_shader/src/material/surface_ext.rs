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
//! Texture slots for procedural graphs live on this extension at bindings 100–117
//! (StandardMaterial reserves 0–99, per Bevy convention). All slots share one
//! sampler at binding 101 to stay under Metal's 16-samplers-per-stage limit.

use std::marker::PhantomData;

use bevy::asset::uuid_handle;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use uuid::Uuid;

/// Number of parameter slots exposed in the per-material uniform buffer.
/// Each slot is one `Vec4`; scalars and bools occupy `.x`, `Vec2` uses `.xy`,
/// `Vec3` uses `.xyz`, `Vec4`/`Color` use the whole slot. 32 slots is enough
/// for any realistic master graph and keeps the UBO under 512 bytes.
pub const SURFACE_GRAPH_PARAM_SLOTS: usize = 32;

/// Well-known handle for the default (unmodified-StandardMaterial) extension
/// fragment shader. Used as the fallback when a material hasn't been compiled
/// yet, so the pipeline layout is always valid.
pub const SURFACE_GRAPH_EXT_DEFAULT_FRAG: Handle<Shader> =
    uuid_handle!("b1c2d3e4-f5a6-4001-aaaa-beefcafebabe");

/// Minimal extension fragment shader: `pbr_input_from_standard_material` →
/// `apply_pbr_lighting` → post-processing, with no mutations.
///
/// Declares the parameter UBO at binding 118 even though the default shader
/// doesn't read from it — the bind group layout has to match the
/// `AsBindGroup` derive on `SurfaceGraphExt`, otherwise wgpu rejects the
/// pipeline at draw time.
pub const DEFAULT_EXT_FRAG_SRC: &str = r#"
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}

struct SurfaceGraphParams {
    slots: array<vec4<f32>, 32>,
}
@group(3) @binding(118) var<uniform> material_params: SurfaceGraphParams;

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
/// Texture slots live at bindings 100–117 alongside StandardMaterial's own
/// bindings (0–~30). Bevy merges both sets into `@group(3)`, filtering
/// duplicates.
///
/// All slots share ONE sampler (binding 101, taken from `texture_0`'s image,
/// or the fallback image's linear sampler when `texture_0` is `None`). Metal
/// caps sampler states at 16 per shader stage; per-slot samplers pushed the
/// fragment stage to 23 (6 mesh-view + 2 mesh + 6 StandardMaterial + 9 here)
/// and the pipeline failed to build on macOS. Sharing brings it to 15.
///
/// The derives mirror Bevy's own `extended_material.rs` example:
/// `Asset + AsBindGroup + Reflect + Debug + Clone + Default` is the full set
/// required by `MaterialPlugin<ExtendedMaterial<_, Self>>`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(SurfaceGraphExtKey)]
pub struct SurfaceGraphExt {
    /// Slot 0 also owns the shared sampler (binding 101) used by every other
    /// slot — see the struct docs for the Metal sampler-limit rationale.
    /// Codegen fills slots in order, so any graph that samples a 2D texture
    /// populates this one first.
    #[texture(100)]
    #[sampler(101)]
    pub texture_0: Option<Handle<Image>>,

    #[texture(102)]
    pub texture_1: Option<Handle<Image>>,

    #[texture(104)]
    pub texture_2: Option<Handle<Image>>,

    #[texture(106)]
    pub texture_3: Option<Handle<Image>>,

    /// Extra D2 slots so a fully-extracted PBR material (base color +
    /// metallic-roughness + normal + emissive + occlusion = 5 maps) can fit
    /// without trimming. Bindings 114 and 116.
    #[texture(114)]
    pub texture_4: Option<Handle<Image>>,

    #[texture(116)]
    pub texture_5: Option<Handle<Image>>,

    /// User cubemap slot (binding 108). Lets a material sample a
    /// user-supplied skybox/IBL-style cube beyond Bevy's built-in env map —
    /// e.g., a baked local reflection cube, a stylized sky, a custom
    /// irradiance probe. `None` falls back to Bevy's `FallbackImage::cube`
    /// (a neutral white cube), so the pipeline layout stays valid.
    #[texture(108, dimension = "cube")]
    pub cube_0: Option<Handle<Image>>,

    /// User 2D array slot (binding 110). Layered texture lookup —
    /// terrain layer stacks, asset variants keyed by layer index, character
    /// body-paint masks. `None` falls back to `FallbackImage::d2_array`.
    #[texture(110, dimension = "2d_array")]
    pub array_0: Option<Handle<Image>>,

    /// User 3D texture slot (binding 112). Volumetric data — volume
    /// fog density, caustics LUT, precomputed scattering tables, 3D noise
    /// bakes. `None` falls back to `FallbackImage::d3`.
    #[texture(112, dimension = "3d")]
    pub volume_0: Option<Handle<Image>>,

    /// Named-parameter uniform buffer. The codegen rewrites every `param/*`
    /// node to read from a fixed slot in this buffer; the resolver writes
    /// authored defaults (for masters) or instance overrides (for material
    /// instances) into the slots. Two material instances of the same master
    /// share one compiled shader and differ only in this buffer's contents,
    /// so wgpu reuses the same specialized pipeline.
    #[uniform(118)]
    pub params: SurfaceGraphParams,

    /// UUID of this material's compiled fragment shader. The resolver inserts
    /// the Shader asset at `Handle::Uuid(shader_uuid, PhantomData)`, and
    /// `specialize()` reconstructs the handle the same way to swap the
    /// pipeline's fragment stage. `Option` because freshly-constructed
    /// materials (default factory) have no compiled shader yet and must fall
    /// back to `SURFACE_GRAPH_EXT_DEFAULT_FRAG`.
    pub shader_uuid: Option<Uuid>,
}

/// Parameter buffer mirrored 1:1 in WGSL (see codegen for the matching
/// struct declaration). Every `param/*` node lives in one slot; scalar
/// types use `.x`, vec2 uses `.xy`, vec3 uses `.xyz`, vec4/color uses the
/// whole slot.
#[derive(ShaderType, Reflect, Debug, Clone)]
pub struct SurfaceGraphParams {
    pub slots: [Vec4; SURFACE_GRAPH_PARAM_SLOTS],
}

impl Default for SurfaceGraphParams {
    fn default() -> Self {
        Self {
            slots: [Vec4::ZERO; SURFACE_GRAPH_PARAM_SLOTS],
        }
    }
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
        Self {
            shader_uuid: ext.shader_uuid,
        }
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
        // Skip the swap on prepass / shadow pipelines. Our generated shader is
        // forward-only — it imports `apply_pbr_lighting`, which is gated on
        // `#ifndef PREPASS_FRAGMENT`, and reads `forward_io::VertexOutput`
        // which differs from `prepass_io::VertexOutput`. Forcing our shader
        // into the prepass triggers naga errors. Letting Bevy keep
        // StandardMaterial's prepass shader handles alpha cutout for `Mask`
        // materials and depth correctly.
        let label = descriptor.label.as_deref().unwrap_or("");
        let is_prepass_or_shadow = label.contains("prepass") || label.contains("shadow");
        if is_prepass_or_shadow {
            return Ok(());
        }
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
            texture_4: Some(fallback.0.clone()),
            texture_5: Some(fallback.0.clone()),
            // cube/array/3d stay None — Bevy's FallbackImage covers the
            // bind-group layout until the user assigns real handles.
            cube_0: None,
            array_0: None,
            volume_0: None,
            params: SurfaceGraphParams::default(),
            shader_uuid: None,
        },
    }
}
