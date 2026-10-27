//! The pipelines that do not depend on a material: cull, raster and the
//! material-depth pass, plus every bind group layout the crate uses.

use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, storage_buffer_sized, texture_2d, uniform_buffer,
    uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::view::ViewUniform;
use bevy::shader::ShaderDefVal;

#[derive(Resource)]
pub(crate) struct VgPipelines {
    pub cull: CachedComputePipelineId,
    pub skin: CachedComputePipelineId,
    pub skin_layout: BindGroupLayoutDescriptor,
    pub raster_single: CachedRenderPipelineId,
    pub raster_double: CachedRenderPipelineId,
    pub material_depth: CachedRenderPipelineId,
    pub material_vertex: Handle<Shader>,
    pub standard_prepass: Handle<Shader>,
    pub raster_masked: Handle<Shader>,
    pub masked_scene_layout: BindGroupLayoutDescriptor,
    pub bucket_layout: BindGroupLayoutDescriptor,
    pub cull_layout: BindGroupLayoutDescriptor,
    pub raster_view_layout: BindGroupLayoutDescriptor,
    pub raster_scene_layout: BindGroupLayoutDescriptor,
    pub material_depth_layout: BindGroupLayoutDescriptor,
    /// Group 2 of every material pass: what `resolve.wesl` reads.
    pub shade_layout: BindGroupLayoutDescriptor,
    /// Group 2 of the transparent draws (see `transparent.wesl`).
    pub transparent_layout: BindGroupLayoutDescriptor,
    pub transparent_vertex: Handle<Shader>,
}

/// The cull parameters, matching `VgCullParams`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CullParams {
    pub pair_count: u32,
    pub instance_count: u32,
    pub visible_capacity: u32,
    pub lod_threshold: f32,
    pub view_layers: [u32; 2],
    pub masked_offset: u32,
    pub _pad: u32,
}

/// Where the transparent pass starts the scene's bindings in group 2, past
/// Bevy's mesh bindings (0 for the `Mesh` uniforms, 9 for metadata), which
/// its material's fragment shader declares there.
pub(crate) const TRANSPARENT_VG_BASE: u32 = 100;

/// The shader defs that place `bindings.wesl` in bind group `group`, with its
/// bindings starting at `base`.
pub(crate) fn vg_bindings(group: u32, base: u32) -> [ShaderDefVal; 2] {
    [
        ShaderDefVal::UInt("VG_GROUP".into(), group),
        ShaderDefVal::UInt("VG_BASE".into(), base),
    ]
}

pub(crate) fn init(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
    fullscreen: Res<FullscreenShader>,
) {
    let read = || storage_buffer_read_only_sized(false, None);
    let params = || {
        uniform_buffer_sized(
            false,
            std::num::NonZeroU64::new(std::mem::size_of::<CullParams>() as u64),
        )
    };
    let cull_layout = BindGroupLayoutDescriptor::new(
        "vg_cull_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<ViewUniform>(true),
                params(),
                read(),
                read(),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
                read(),
                storage_buffer_sized(false, None),
                // Residency, then the requested and drawn bits.
                read(),
                storage_buffer_sized(false, None),
                storage_buffer_sized(false, None),
            ),
        ),
    );
    let skin_layout = BindGroupLayoutDescriptor::new(
        "vg_skin_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer_sized(false, std::num::NonZeroU64::new(16)),
                read(),
                read(),
                read(),
                read(),
                read(),
                read(),
                storage_buffer_sized(false, None),
            ),
        ),
    );
    let masked_scene_layout = BindGroupLayoutDescriptor::new(
        "vg_masked_scene_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                (1, read()),
                (2, read()),
                (3, read()),
                (4, read()),
                (5, read()),
                (6, read()),
                (7, read()),
            ),
        ),
    );
    let bucket_layout = BindGroupLayoutDescriptor::new(
        "vg_masked_bucket_layout",
        &BindGroupLayoutEntries::single(
            ShaderStages::VERTEX,
            uniform_buffer_sized(true, std::num::NonZeroU64::new(4)),
        ),
    );
    let raster_view_layout = BindGroupLayoutDescriptor::new(
        "vg_raster_view_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX,
            (uniform_buffer::<ViewUniform>(true), params()),
        ),
    );
    let raster_scene_layout = BindGroupLayoutDescriptor::new(
        "vg_raster_scene_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX,
            (
                (1, read()),
                (2, read()),
                (3, read()),
                (4, read()),
                (5, read()),
                (7, read()),
            ),
        ),
    );
    let material_depth_layout = BindGroupLayoutDescriptor::new(
        "vg_material_depth_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(TextureSampleType::Uint), read(), read()),
        ),
    );
    let shade_layout = BindGroupLayoutDescriptor::new(
        "vg_shade_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Uint),
                read(),
                read(),
                read(),
                read(),
                read(),
                read(),
                read(),
            ),
        ),
    );

    let base = TRANSPARENT_VG_BASE;
    let transparent_layout = BindGroupLayoutDescriptor::new(
        "vg_transparent_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                (0, read()),
                (base + 1, read()),
                (base + 2, read()),
                (base + 3, read()),
                (base + 4, read()),
                (base + 5, read()),
                (base + 6, read()),
                (base + 7, read()),
                (base + 8, uniform_buffer_sized(true, std::num::NonZeroU64::new(8))),
            ),
        ),
    );

    // Embedded by the plugin (`lib.rs`), so the paths are the crate's.
    let shader = |name: &str| -> Handle<Shader> {
        asset_server.load(format!("embedded://renzora_virtual_geometry/shaders/{name}.wesl"))
    };
    let cull_shader = shader("cull");
    let raster_shader = shader("raster");
    let depth_shader = shader("material_depth");
    let material_vertex = shader("material");
    let standard_prepass = shader("standard_prepass");
    let raster_masked = shader("raster_masked");
    let transparent_vertex = shader("transparent");

    let cull = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("vg_cull".into()),
        layout: vec![cull_layout.clone()],
        shader: cull_shader,
        entry_point: Some("cull".into()),
        ..default()
    });
    let skin = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("vg_skin".into()),
        layout: vec![skin_layout.clone()],
        shader: shader("skin"),
        entry_point: Some("skin_vertices".into()),
        ..default()
    });

    let raster = |double_sided: bool| {
        let mut defs = vg_bindings(1, 0).to_vec();
        if double_sided {
            defs.push("DOUBLE_SIDED".into());
        }
        RenderPipelineDescriptor {
            label: Some(if double_sided { "vg_raster_double" } else { "vg_raster_single" }.into()),
            layout: vec![raster_view_layout.clone(), raster_scene_layout.clone()],
            vertex: VertexState {
                shader: raster_shader.clone(),
                shader_defs: defs.clone(),
                ..default()
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: if double_sided { None } else { Some(Face::Back) },
                ..default()
            },
            // The view's own depth buffer, written exactly as Bevy's prepass
            // would: reversed-z, nearest wins. Ordinary meshes drawn after
            // this test against it, so the two kinds of geometry occlude each
            // other correctly.
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: default(),
            }),
            fragment: Some(FragmentState {
                shader: raster_shader.clone(),
                shader_defs: defs,
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::R32Uint,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        }
    };
    let raster_single = pipeline_cache.queue_render_pipeline(raster(false));
    let raster_double = pipeline_cache.queue_render_pipeline(raster(true));

    let material_depth = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("vg_material_depth".into()),
        layout: vec![material_depth_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth16Unorm,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::Always),
            stencil: default(),
            bias: default(),
        }),
        fragment: Some(FragmentState {
            shader: depth_shader,
            shader_defs: vg_bindings(0, 0).to_vec(),
            targets: vec![],
            ..default()
        }),
        ..default()
    });

    commands.insert_resource(VgPipelines {
        cull,
        skin,
        skin_layout,
        raster_single,
        raster_double,
        material_depth,
        material_vertex,
        standard_prepass,
        raster_masked,
        masked_scene_layout,
        bucket_layout,
        cull_layout,
        raster_view_layout,
        raster_scene_layout,
        material_depth_layout,
        shade_layout,
        transparent_layout,
        transparent_vertex,
    });
}
