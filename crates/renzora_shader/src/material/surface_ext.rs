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
//! Texture slots for procedural graphs live on this extension at bindings 100 to 133
//! (StandardMaterial reserves 0–99, per Bevy convention). All slots share one
//! sampler at binding 101 to stay under Metal's 16-samplers-per-stage limit.
//! Bindings 122 and 123 are the terrain's landscape bounds and weights, which
//! only a terrain's own instance fills in (`LandscapeParams`).

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
use bevy::shader::{ShaderDefVal, ShaderRef};
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
import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::pbr_functions;
import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};

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

    /// Slots 6..15, at bindings 124..133: past the landscape bindings, and
    /// clear of the particle extension's own 119..121. Declared by a shader
    /// only when its graph samples that many images (a four-layer landscape
    /// samples sixteen), so every other shader is unchanged; the layout has
    /// them either way, bound to the fallback white.
    #[texture(124)]
    pub texture_6: Option<Handle<Image>>,
    #[texture(125)]
    pub texture_7: Option<Handle<Image>>,
    #[texture(126)]
    pub texture_8: Option<Handle<Image>>,
    #[texture(127)]
    pub texture_9: Option<Handle<Image>>,
    #[texture(128)]
    pub texture_10: Option<Handle<Image>>,
    #[texture(129)]
    pub texture_11: Option<Handle<Image>>,
    #[texture(130)]
    pub texture_12: Option<Handle<Image>>,
    #[texture(131)]
    pub texture_13: Option<Handle<Image>>,
    #[texture(132)]
    pub texture_14: Option<Handle<Image>>,
    #[texture(133)]
    pub texture_15: Option<Handle<Image>>,

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

    /// Where the terrain wearing this material is, and how its weight
    /// channels map onto the material's layers. All zero (not a terrain) on
    /// every material except a terrain's own instance, which
    /// `renzora_terrain::landscape` makes per terrain and fills in.
    #[uniform(122)]
    pub landscape: LandscapeParams,

    /// The terrain's painted layer weights: an RGBA8 array of two slices, the
    /// eight channels over the whole terrain. `None` (Bevy's fallback array)
    /// everywhere but a terrain's instance, where `landscape.info.x` is 0
    /// anyway and the shader never reads it.
    #[texture(123, dimension = "2d_array")]
    pub landscape_weights: Option<Handle<Image>>,

    /// UUID of this material's compiled fragment shader. The resolver inserts
    /// the Shader asset at `Handle::Uuid(shader_uuid, PhantomData)`, and
    /// `specialize()` reconstructs the handle the same way to swap the
    /// pipeline's fragment stage. `Option` because freshly-constructed
    /// materials (default factory) have no compiled shader yet and must fall
    /// back to `SURFACE_GRAPH_EXT_DEFAULT_FRAG`.
    pub shader_uuid: Option<Uuid>,

    /// Whether the compiled shader can also write the deferred G-buffer
    /// (`codegen::supports_deferred`). When it can, `specialize()` swaps it
    /// into the deferred prepass too, so a node graph looks the same under
    /// deferred rendering (and Solari, which shades from the G-buffer) as it
    /// does forward. When it can't, that pass falls back to the base
    /// `StandardMaterial` approximation.
    pub writes_gbuffer: bool,

    /// Whether the compiled shader can also shade virtual geometry
    /// (`codegen::supports_virtual_geometry`). Read by
    /// `renzora_virtual_geometry` when deciding which meshes to convert; it
    /// changes nothing about ordinary pipelines, so it stays out of the key.
    pub virtual_geometry: bool,
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

/// The terrain a landscape material is drawn on, mirrored 1:1 by
/// `LandscapeParams` in the generated WGSL (`codegen::wgsl`).
#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default, PartialEq)]
pub struct LandscapeParams {
    /// xy: world X/Z of the weight map's (0, 0) corner. zw: the world size the
    /// whole map covers. Landscape Layer Coords measures from the corner too.
    pub bounds: Vec4,
    /// x: 1 when this is a terrain's instance and the weights are real, 0
    /// otherwise (the first layer then shows everywhere). yzw unused.
    pub info: Vec4,
    /// The weight channel holding material layer 0..3.
    pub channels_a: UVec4,
    /// The weight channel holding material layer 4..7.
    pub channels_b: UVec4,
}

impl LandscapeParams {
    /// Material layer `i` read from channel `i`: what a terrain with no
    /// layer names recorded yet uses.
    pub const IDENTITY_CHANNELS: [u32; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

    /// The params for a terrain whose weight map starts at world `corner`
    /// (X/Z) and covers `size`, reading material layer `i` from channel
    /// `channels[i]`.
    pub fn for_terrain(corner: Vec2, size: Vec2, channels: [u32; 8]) -> Self {
        Self {
            bounds: Vec4::new(corner.x, corner.y, size.x, size.y),
            info: Vec4::new(1.0, 0.0, 0.0, 0.0),
            channels_a: UVec4::new(channels[0], channels[1], channels[2], channels[3]),
            channels_b: UVec4::new(channels[4], channels[5], channels[6], channels[7]),
        }
    }
}

impl SurfaceGraphExt {
    /// The sixteen 2D slots, in slot order.
    pub fn d2_slots(&self) -> [&Option<Handle<Image>>; super::codegen::MAX_D2_TEXTURES] {
        [
            &self.texture_0, &self.texture_1, &self.texture_2, &self.texture_3,
            &self.texture_4, &self.texture_5, &self.texture_6, &self.texture_7,
            &self.texture_8, &self.texture_9, &self.texture_10, &self.texture_11,
            &self.texture_12, &self.texture_13, &self.texture_14, &self.texture_15,
        ]
    }

    /// 2D slot `slot`, for writing.
    pub fn d2_slot_mut(&mut self, slot: usize) -> Option<&mut Option<Handle<Image>>> {
        Some(match slot {
            0 => &mut self.texture_0,
            1 => &mut self.texture_1,
            2 => &mut self.texture_2,
            3 => &mut self.texture_3,
            4 => &mut self.texture_4,
            5 => &mut self.texture_5,
            6 => &mut self.texture_6,
            7 => &mut self.texture_7,
            8 => &mut self.texture_8,
            9 => &mut self.texture_9,
            10 => &mut self.texture_10,
            11 => &mut self.texture_11,
            12 => &mut self.texture_12,
            13 => &mut self.texture_13,
            14 => &mut self.texture_14,
            15 => &mut self.texture_15,
            _ => return None,
        })
    }

    /// Bind `handle` where a compiled shader's [`super::codegen::TextureBinding`]
    /// of `kind` at `binding` reads it. `false` for a slot the extension does
    /// not have.
    pub fn set_texture(&mut self, kind: super::codegen::TextureKind, binding: u32, handle: Handle<Image>) -> bool {
        use super::codegen::TextureKind;
        match (kind, binding) {
            (TextureKind::D2, slot) => match self.d2_slot_mut(slot as usize) {
                Some(s) => *s = Some(handle),
                None => return false,
            },
            (TextureKind::Cube, 0) => self.cube_0 = Some(handle),
            (TextureKind::D2Array, 0) => self.array_0 = Some(handle),
            (TextureKind::D3, 0) => self.volume_0 = Some(handle),
            _ => return false,
        }
        true
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
    pub writes_gbuffer: bool,
}

impl From<&SurfaceGraphExt> for SurfaceGraphExtKey {
    fn from(ext: &SurfaceGraphExt) -> Self {
        Self {
            shader_uuid: ext.shader_uuid,
            writes_gbuffer: ext.writes_gbuffer,
        }
    }
}

/// Whether `descriptor` is the deferred G-buffer pass: the prepass pipeline
/// with `DEFERRED_PREPASS` set, which Bevy only sets for a camera that has a
/// `DeferredPrepass` and a material that renders deferred.
fn is_deferred_prepass(descriptor: &RenderPipelineDescriptor) -> bool {
    descriptor.fragment.as_ref().is_some_and(|frag| {
        frag.shader_defs
            .iter()
            .any(|def| matches!(def, ShaderDefVal::Bool(name, true) if name == "DEFERRED_PREPASS"))
    })
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
        // Prepass and shadow pipelines keep StandardMaterial's own shader,
        // which handles depth and `Mask` alpha cutout from the base
        // approximation. The one exception is the deferred G-buffer pass for a
        // shader that can write it: that pass decides what the surface looks
        // like (deferred lighting and Solari both read only the G-buffer), so
        // leaving the base there drew every node graph as its approximation.
        // A shader without a deferred branch would fail to compile in that
        // pipeline (different vertex input, different view bindings), so it
        // still gets the base.
        let label = descriptor.label.as_deref().unwrap_or("");
        let is_prepass_or_shadow = label.contains("prepass") || label.contains("shadow");
        if is_prepass_or_shadow
            && !(key.bind_group_data.writes_gbuffer && is_deferred_prepass(descriptor))
        {
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
            texture_6: Some(fallback.0.clone()),
            texture_7: Some(fallback.0.clone()),
            texture_8: Some(fallback.0.clone()),
            texture_9: Some(fallback.0.clone()),
            texture_10: Some(fallback.0.clone()),
            texture_11: Some(fallback.0.clone()),
            texture_12: Some(fallback.0.clone()),
            texture_13: Some(fallback.0.clone()),
            texture_14: Some(fallback.0.clone()),
            texture_15: Some(fallback.0.clone()),
            // cube/array/3d stay None — Bevy's FallbackImage covers the
            // bind-group layout until the user assigns real handles.
            cube_0: None,
            array_0: None,
            volume_0: None,
            params: SurfaceGraphParams::default(),
            landscape: LandscapeParams::default(),
            landscape_weights: None,
            shader_uuid: None,
            writes_gbuffer: false,
            virtual_geometry: false,
        },
    }
}
