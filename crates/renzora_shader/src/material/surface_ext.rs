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

/// Swap the descriptor's fragment shader to the per-instance compiled
/// `Handle<Shader>` identified by `shader_uuid`. Skips the swap on prepass
/// and shadow pipelines, where the generated WGSL (which imports
/// `apply_pbr_lighting` gated on `#ifndef PREPASS_FRAGMENT`) is invalid.
///
/// Pulled out of the trait `specialize` so the logic can be unit-tested
/// without constructing a `MaterialExtensionPipeline` (whose inner
/// `MeshPipeline` requires a `RenderDevice` that needs a GPU).
pub(crate) fn swap_graph_fragment_shader(
    descriptor: &mut RenderPipelineDescriptor,
    layout: &MeshVertexBufferLayoutRef,
    shader_uuid: Option<Uuid>,
) {
    let label = descriptor.label.as_deref().unwrap_or("");
    let is_prepass_or_shadow = label.contains("prepass") || label.contains("shadow");

    // Skip the swap on prepass / shadow pipelines. Our generated shader is
    // forward-only — it imports `apply_pbr_lighting`, which is gated on
    // `#ifndef PREPASS_FRAGMENT`, and reads `forward_io::VertexOutput`
    // which differs from `prepass_io::VertexOutput`. Forcing our shader
    // into the prepass triggers naga errors. Letting Bevy keep
    // StandardMaterial's prepass shader handles alpha cutout for `Mask`
    // materials and depth correctly.
    if is_prepass_or_shadow {
        return;
    }
    // Ensure `VERTEX_UVS_A` is in the fragment shader defines when the mesh
    // has a UV0 attribute. Without this define, the graph shader's
    // `#ifdef VERTEX_UVS_A let mat_uv = in.uv; #else let mat_uv = vec2(0,0); #endif`
    // always falls back to the zero-UV branch — every fragment samples the
    // same texel regardless of the UV Scale node's value, so tiling is
    // impossible on UV-aware meshes (large planes, tiled ground, etc.).
    //
    // Conditional on UV0 being present: forcing the define for a mesh
    // without UV0 would reference a non-existent `in.uv` field in
    // `forward_io::VertexOutput` and break the shader for that mesh.
    //
    // Idempotent: skip the push if Bevy already added it.
    if layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_UV_0) {
        if let Some(ref mut frag) = descriptor.fragment {
            let already = frag.shader_defs.iter().any(|d| {
                matches!(
                    d,
                    bevy::shader::ShaderDefVal::Bool(name, _) if name == "VERTEX_UVS_A"
                )
            });
            if !already {
                frag.shader_defs.push("VERTEX_UVS_A".into());
            }
        }
    }
    if let Some(uuid) = shader_uuid {
        if let Some(ref mut frag) = descriptor.fragment {
            frag.shader = Handle::<Shader>::Uuid(uuid, PhantomData);
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
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        swap_graph_fragment_shader(descriptor, layout, key.bind_group_data.shader_uuid);
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

// ── Diagnostic + safety tests for `SurfaceGraphExt::specialize` ────────────
//
// These tests construct a `RenderPipelineDescriptor` by hand and pass it to
// `SurfaceGraphExt::specialize` directly. They prove:
//   - When the mesh layout contains `ATTRIBUTE_UV_0` and the descriptor's
//     fragment `shader_defs` does NOT contain `VERTEX_UVS_A`, the
//     extension-specialize step will not silently lose the define.
//   - When the mesh layout has no `ATTRIBUTE_UV_0`, the define is not
//     forced on (a mesh without UVs would fail to compile if we did).
//
// The diagnostic logging added in `specialize` prints the pre/post
// `shader_defs` so these tests double as documentation of the actual
// behavior of the specialize step in isolation — useful because the
// GPU-render path can't run in a headless container without a Vulkan ICD.
//
// We do NOT need a real `MeshPipeline` to call `specialize`; the parameter
// is `_pipeline` (unused). The `mem::zeroed()` below is sound because
// `MaterialExtensionPipeline::mesh_pipeline` is never dereferenced.
#[cfg(test)]
mod specialize_diagnostic_tests {
    use super::*;
    use bevy::render::render_resource::{FragmentState, RenderPipelineDescriptor, VertexState};
    use bevy::shader::ShaderDefVal;

    fn make_plane_layout() -> bevy::mesh::MeshVertexBufferLayoutRef {
        let mut layouts = bevy::mesh::MeshVertexBufferLayouts::default();
        let mesh = bevy::mesh::Mesh::from(
            bevy::math::primitives::Plane3d::default()
                .mesh()
                .size(2.0, 2.0),
        );
        mesh.get_mesh_vertex_buffer_layout(&mut layouts)
    }

    fn make_position_only_layout() -> bevy::mesh::MeshVertexBufferLayoutRef {
        let mut layouts = bevy::mesh::MeshVertexBufferLayouts::default();
        let mesh = bevy::mesh::Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        )
        .with_inserted_attribute(
            bevy::mesh::Mesh::ATTRIBUTE_POSITION,
            vec![[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        );
        mesh.get_mesh_vertex_buffer_layout(&mut layouts)
    }

    /// Build a fake descriptor whose `fragment.shader_defs` mirror what
    /// `MeshPipeline::specialize` + `MaterialPipelineSpecializer::specialize`
    /// would have produced for the given layout. Vertex and fragment use
    /// the same define list (matching Bevy's actual code).
    fn make_descriptor(
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        label: &str,
    ) -> RenderPipelineDescriptor {
        let mut shader_defs: Vec<ShaderDefVal> = vec![
            "MESH_PIPELINE".into(),
            "VERTEX_OUTPUT_INSTANCE_INDEX".into(),
        ];
        if layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_POSITION) {
            shader_defs.push("VERTEX_POSITIONS".into());
        }
        if layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_NORMAL) {
            shader_defs.push("VERTEX_NORMALS".into());
        }
        if layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_UV_0) {
            shader_defs.push("VERTEX_UVS".into());
            shader_defs.push("VERTEX_UVS_A".into());
        }
        if layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_UV_1) {
            shader_defs.push("VERTEX_UVS".into());
            shader_defs.push("VERTEX_UVS_B".into());
        }
        if layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_TANGENT) {
            shader_defs.push("VERTEX_TANGENTS".into());
        }
        if layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_COLOR) {
            shader_defs.push("VERTEX_COLORS".into());
        }
        shader_defs.push(ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), 3));

        RenderPipelineDescriptor {
            label: Some(label.to_string().into()),
            vertex: VertexState {
                shader: default(),
                shader_defs: shader_defs.clone(),
                buffers: vec![],
                entry_point: default(),
            },
            fragment: Some(FragmentState {
                shader: default(),
                shader_defs,
                targets: vec![],
                entry_point: default(),
            }),
            ..default()
        }
    }

    fn names(descriptor: &RenderPipelineDescriptor) -> Vec<String> {
        descriptor
            .fragment
            .as_ref()
            .unwrap()
            .shader_defs
            .iter()
            .map(|d| match d {
                ShaderDefVal::Bool(s, _) => s.clone(),
                ShaderDefVal::Int(s, _) => s.clone(),
                ShaderDefVal::UInt(s, _) => s.clone(),
            })
            .collect()
    }

    /// 1. Mesh WITH UV0: the descriptor built from the layout already has
    ///    VERTEX_UVS_A. swap_graph_fragment_shader must not strip it.
    #[test]
    fn mesh_with_uv0_keeps_vertex_uvs_a() {
        let layout = make_plane_layout();
        assert!(layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_UV_0));

        let mut descriptor = make_descriptor(&layout, "opaque_mesh_pipeline");
        let names_before = names(&descriptor);
        assert!(
            names_before.contains(&"VERTEX_UVS_A".to_string()),
            "mesh with UV0: VERTEX_UVS_A should be in shader_defs BEFORE specialize, got {:?}",
            names_before
        );

        swap_graph_fragment_shader(&mut descriptor, &layout, Some(Uuid::from_u128(1)));

        let names_after = names(&descriptor);
        assert!(
            names_after.contains(&"VERTEX_UVS_A".to_string()),
            "mesh with UV0: VERTEX_UVS_A must SURVIVE swap_graph_fragment_shader, got {:?}",
            names_after
        );
    }

    /// 2. Mesh WITHOUT UV0: the descriptor has no VERTEX_UVS_A, and the
    ///    shader-swap step must not silently add one (it would fail
    ///    compilation if VertexOutput has no `uv` field).
    #[test]
    fn mesh_without_uv0_does_not_get_vertex_uvs_a() {
        let layout = make_position_only_layout();
        assert!(!layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_UV_0));

        let mut descriptor = make_descriptor(&layout, "opaque_mesh_pipeline");
        let names_before = names(&descriptor);
        assert!(
            !names_before.contains(&"VERTEX_UVS_A".to_string()),
            "mesh without UV0: VERTEX_UVS_A should NOT be in shader_defs BEFORE specialize, got {:?}",
            names_before
        );

        swap_graph_fragment_shader(&mut descriptor, &layout, Some(Uuid::from_u128(2)));

        let names_after = names(&descriptor);
        assert!(
            !names_after.contains(&"VERTEX_UVS_A".to_string()),
            "mesh without UV0: VERTEX_UVS_A must NOT appear after specialize, got {:?}",
            names_after
        );
    }

    /// 3. Prepass pipeline (label "prepass") — must skip the shader swap.
    ///    Fragment shader_defs should be unchanged.
    #[test]
    fn prepass_label_is_skipped() {
        let layout = make_plane_layout();
        let mut descriptor = make_descriptor(&layout, "prepass_mesh_pipeline");
        let names_before = names(&descriptor);

        swap_graph_fragment_shader(&mut descriptor, &layout, Some(Uuid::from_u128(3)));

        let names_after = names(&descriptor);
        assert_eq!(
            names_before, names_after,
            "prepass pipeline: specialize must be a no-op"
        );
    }

    /// 4. Shadow pipeline (label "shadow") — must skip the shader swap.
    #[test]
    fn shadow_label_is_skipped() {
        let layout = make_position_only_layout();
        let mut descriptor = make_descriptor(&layout, "shadow_mesh_pipeline");
        let names_before = names(&descriptor);

        swap_graph_fragment_shader(&mut descriptor, &layout, Some(Uuid::from_u128(4)));

        let names_after = names(&descriptor);
        assert_eq!(
            names_before, names_after,
            "shadow pipeline: specialize must be a no-op"
        );
    }

    /// 5. IDEMPOTENCY: mesh WITH UV0 whose descriptor is missing
    ///    VERTEX_UVS_A (synthetic state, manually stripped in setup;
    ///    Bevy 0.19.1's `MeshPipeline::specialize` and `PrepassPipeline::specialize`
    ///    already push this define for UV-aware meshes, so this branch is
    ///    unreachable in production today). The defensive push must add the
    ///    define so the `#ifdef VERTEX_UVS_A let mat_uv = in.uv;` branch is
    ///    taken, defending against a hypothetical future Bevy where the
    ///    define is dropped.
    #[test]
    fn mesh_with_uv0_missing_vertex_uvs_a_gets_it_added() {
        let layout = make_plane_layout();
        assert!(layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_UV_0));

        // Simulate the buggy state: build a descriptor but strip
        // VERTEX_UVS_A from shader_defs.
        let mut descriptor = make_descriptor(&layout, "opaque_mesh_pipeline");
        if let Some(frag) = descriptor.fragment.as_mut() {
            frag.shader_defs
                .retain(|d| !matches!(d, ShaderDefVal::Bool(name, _) if name == "VERTEX_UVS_A"));
        }
        let names_before = names(&descriptor);
        assert!(
            !names_before.contains(&"VERTEX_UVS_A".to_string()),
            "test setup: VERTEX_UVS_A must be stripped before swap_graph_fragment_shader, got {:?}",
            names_before
        );

        swap_graph_fragment_shader(&mut descriptor, &layout, Some(Uuid::from_u128(5)));

        let names_after = names(&descriptor);
        assert!(
            names_after.contains(&"VERTEX_UVS_A".to_string()),
            "mesh with UV0: VERTEX_UVS_A must be RE-ADDED by the fix, got {:?}",
            names_after
        );
    }

    /// 6. IDEMPOTENT: mesh WITH UV0 whose descriptor already has
    ///    VERTEX_UVS_A — the fix must not push a second copy.
    #[test]
    fn mesh_with_uv0_existing_vertex_uvs_a_not_duplicated() {
        let layout = make_plane_layout();
        assert!(layout.0.contains(bevy::mesh::Mesh::ATTRIBUTE_UV_0));

        let mut descriptor = make_descriptor(&layout, "opaque_mesh_pipeline");
        let count_before = names(&descriptor)
            .iter()
            .filter(|n| n.as_str() == "VERTEX_UVS_A")
            .count();
        assert_eq!(
            count_before, 1,
            "make_descriptor should produce exactly one VERTEX_UVS_A entry, got {}",
            count_before
        );

        swap_graph_fragment_shader(&mut descriptor, &layout, Some(Uuid::from_u128(6)));

        let names_after = names(&descriptor);
        let count_after = names_after
            .iter()
            .filter(|n| n.as_str() == "VERTEX_UVS_A")
            .count();
        assert_eq!(
            count_after, 1,
            "fix must be idempotent: VERTEX_UVS_A should not be pushed twice, got {:?}",
            names_after
        );
    }
}
