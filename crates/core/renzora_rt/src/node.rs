use crate::{
    extract::ExtractedLightDirection,
    prepare::{RtLightingResources, RADIANCE_CACHE_SIZE},
    settings::{RtLighting, RtPushConstants},
};
use bevy::asset::{load_embedded_asset, Handle};
use bevy::core_pipeline::prepass::{
    PreviousViewData, PreviousViewUniformOffset, PreviousViewUniforms, ViewPrepassTextures,
};
use bevy::diagnostic::FrameCount;
use bevy::ecs::{
    query::QueryItem,
    world::{FromWorld, World},
};
use bevy::render::{
    diagnostic::RecordDiagnostics,
    render_graph::{NodeRunError, RenderGraphContext, ViewNode},
    render_resource::{
        binding_types::{
            storage_buffer_sized, texture_2d, texture_depth_2d, texture_storage_2d, uniform_buffer,
        },
        BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, PipelineCache,
        PushConstantRange, ShaderStages, StorageTextureAccess, TextureFormat, TextureSampleType,
    },
    renderer::RenderContext,
    view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
};
use bevy::shader::Shader;
use bevy::utils::default;

pub mod graph {
    use bevy::render::render_graph::RenderLabel;

    #[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
    pub struct RtLightingNode;
}

pub struct RtLightingNode {
    bind_group_layout: BindGroupLayoutDescriptor,
    hi_z_generate_pipeline: CachedComputePipelineId,
    ssgi_trace_pipeline: CachedComputePipelineId,
    radiance_cache_update_pipeline: CachedComputePipelineId,
    ss_reflections_pipeline: CachedComputePipelineId,
    ss_shadows_pipeline: CachedComputePipelineId,
    temporal_denoise_pipeline: CachedComputePipelineId,
    spatial_denoise_pipeline: CachedComputePipelineId,
    composite_pipeline: CachedComputePipelineId,
}

impl ViewNode for RtLightingNode {
    type ViewQuery = (
        &'static RtLighting,
        &'static RtLightingResources,
        &'static ViewTarget,
        &'static ViewPrepassTextures,
        &'static ViewUniformOffset,
        Option<&'static PreviousViewUniformOffset>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (
            rt_lighting,
            resources,
            view_target,
            view_prepass_textures,
            view_uniform_offset,
            previous_view_uniform_offset,
        ): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let view_uniforms = world.resource::<ViewUniforms>();
        let previous_view_uniforms = world.resource::<PreviousViewUniforms>();
        let frame_count = world.resource::<FrameCount>();

        let (
            Some(hi_z_pipeline),
            Some(ssgi_pipeline),
            Some(cache_update_pipeline),
            Some(refl_pipeline),
            Some(shadow_pipeline),
            Some(temporal_pipeline),
            Some(spatial_pipeline),
            Some(composite_pipeline),
            Some(depth_buffer),
            Some(view_uniforms_binding),
            Some(previous_view_uniforms_binding),
        ) = (
            pipeline_cache.get_compute_pipeline(self.hi_z_generate_pipeline),
            pipeline_cache.get_compute_pipeline(self.ssgi_trace_pipeline),
            pipeline_cache.get_compute_pipeline(self.radiance_cache_update_pipeline),
            pipeline_cache.get_compute_pipeline(self.ss_reflections_pipeline),
            pipeline_cache.get_compute_pipeline(self.ss_shadows_pipeline),
            pipeline_cache.get_compute_pipeline(self.temporal_denoise_pipeline),
            pipeline_cache.get_compute_pipeline(self.spatial_denoise_pipeline),
            pipeline_cache.get_compute_pipeline(self.composite_pipeline),
            view_prepass_textures.depth_view(),
            view_uniforms.uniforms.binding(),
            previous_view_uniforms.uniforms.binding(),
        )
        else {
            return Ok(());
        };

        let motion_vectors = view_prepass_textures.motion_vectors_view();
        let deferred = view_prepass_textures.deferred_view();
        let view_target_texture = view_target.get_unsampled_color_attachment();
        let s = resources;

        // Use prepass motion vectors if available, otherwise a 1x1 zero fallback
        let mv_view = motion_vectors.unwrap_or(&s.fallback_motion_vectors_view);
        let deferred_view = deferred.unwrap_or(&s.fallback_deferred_view);

        let bind_group = render_context.render_device().create_bind_group(
            "rt_lighting_bind_group",
            &pipeline_cache.get_bind_group_layout(&self.bind_group_layout),
            &BindGroupEntries::sequential((
                view_target_texture.view,
                depth_buffer,
                mv_view,
                view_uniforms_binding,
                previous_view_uniforms_binding,
                &s.hi_z_views[0],
                &s.gi_output.1,
                &s.gi_history.1,
                &s.refl_output.1,
                &s.refl_history.1,
                &s.shadow_output.1,
                &s.shadow_history.1,
                &s.denoise_ping.1,
                &s.denoise_pong.1,
                s.radiance_cache_checksums.as_entire_binding(),
                s.radiance_cache_life.as_entire_binding(),
                s.radiance_cache_radiance.as_entire_binding(),
                s.radiance_cache_normals.as_entire_binding(),
                s.radiance_cache_sample_count.as_entire_binding(),
                deferred_view,
            )),
        );

        let frame_index = frame_count.0.wrapping_mul(5782582);
        let light_dir = world
            .get_resource::<ExtractedLightDirection>()
            .copied()
            .unwrap_or_default();
        let mut push_constants =
            RtPushConstants::from_settings(rt_lighting, frame_index, light_dir.0);
        let prev_offset = previous_view_uniform_offset
            .map(|o| o.offset)
            .unwrap_or(view_uniform_offset.offset);

        let diagnostics = render_context.diagnostic_recorder();
        let command_encoder = render_context.command_encoder();

        let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("rt_lighting"),
            timestamp_writes: None,
        });

        let dx = s.trace_size.x.div_ceil(8);
        let dy = s.trace_size.y.div_ceil(8);
        let full_dx = s.view_size.x.div_ceil(8);
        let full_dy = s.view_size.y.div_ceil(8);

        pass.set_bind_group(
            0,
            &bind_group,
            &[view_uniform_offset.offset, prev_offset],
        );

        // 1. Hi-Z depth copy (mip 0 from depth buffer)
        {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/hi_z_generate");
            pass.set_pipeline(hi_z_pipeline);
            pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
            pass.dispatch_workgroups(full_dx, full_dy, 1);
            d.end(&mut pass);
        }

        // 2. Screen-space GI probe trace (one probe per 8x8 pixel block)
        if rt_lighting.gi_enabled {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/ssgi_trace");
            pass.set_pipeline(ssgi_pipeline);
            pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
            pass.dispatch_workgroups(dx, dy, 1);
            d.end(&mut pass);
        }

        // 3. Radiance cache update
        if rt_lighting.gi_enabled {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/radiance_cache_update");
            pass.set_pipeline(cache_update_pipeline);
            pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
            pass.dispatch_workgroups((RADIANCE_CACHE_SIZE / 256) as u32, 1, 1);
            d.end(&mut pass);
        }

        // 4. Screen-space reflections
        if rt_lighting.reflections_enabled {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/ss_reflections");
            pass.set_pipeline(refl_pipeline);
            pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
            pass.dispatch_workgroups(dx, dy, 1);
            d.end(&mut pass);
        }

        // 5. Screen-space contact shadows
        if rt_lighting.shadows_enabled {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/ss_shadows");
            pass.set_pipeline(shadow_pipeline);
            pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
            pass.dispatch_workgroups(full_dx, full_dy, 1);
            d.end(&mut pass);
        }

        // 6. Temporal denoise (z=0 GI, z=1 reflections, z=2 shadows)
        if rt_lighting.denoise_temporal {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/temporal_denoise");
            pass.set_pipeline(temporal_pipeline);
            pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
            pass.dispatch_workgroups(full_dx, full_dy, 3);
            d.end(&mut pass);
        }

        // 7. Spatial denoise (A-Trous wavelet, multiple iterations with increasing step)
        if rt_lighting.denoise_spatial_iterations > 0 {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/spatial_denoise");
            pass.set_pipeline(spatial_pipeline);
            for iter in 0..rt_lighting.denoise_spatial_iterations {
                // Encode step_size (1, 2, 4, 8, 16) into the frame_index field
                // The shader decodes: step_size = 1 << (frame_index >> 24)
                push_constants.frame_index =
                    (frame_index & 0x00FFFFFF) | (iter << 24);
                pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
                pass.dispatch_workgroups(full_dx, full_dy, 3);
            }
            // Restore frame_index
            push_constants.frame_index = frame_index;
            d.end(&mut pass);
        }

        // 8. Composite into HDR color buffer
        {
            let d = diagnostics.time_span(&mut pass, "rt_lighting/composite");
            pass.set_pipeline(composite_pipeline);
            pass.set_push_constants(0, bytemuck::bytes_of(&push_constants));
            pass.dispatch_workgroups(full_dx, full_dy, 1);
            d.end(&mut pass);
        }

        Ok(())
    }
}

impl FromWorld for RtLightingNode {
    fn from_world(world: &mut World) -> Self {
        let pipeline_cache = world.resource::<PipelineCache>();

        let bind_group_layout = BindGroupLayoutDescriptor::new(
            "rt_lighting_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    // 0: HDR color buffer (read/write)
                    texture_storage_2d(
                        ViewTarget::TEXTURE_FORMAT_HDR,
                        StorageTextureAccess::ReadWrite,
                    ),
                    // 1: depth buffer
                    texture_depth_2d(),
                    // 2: motion vectors (or depth fallback)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 3: view uniforms
                    uniform_buffer::<ViewUniform>(true),
                    // 4: previous view uniforms
                    uniform_buffer::<PreviousViewData>(true),
                    // 5: Hi-Z texture mip 0 (R32Float)
                    texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::ReadWrite),
                    // 6: GI output
                    texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                    // 7: GI history
                    texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                    // 8: reflections output
                    texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                    // 9: reflections history
                    texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                    // 10: shadow output
                    texture_storage_2d(TextureFormat::R16Float, StorageTextureAccess::ReadWrite),
                    // 11: shadow history
                    texture_storage_2d(TextureFormat::R16Float, StorageTextureAccess::ReadWrite),
                    // 12: denoise ping
                    texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                    // 13: denoise pong
                    texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                    // 14-18: radiance cache
                    storage_buffer_sized(false, None),
                    storage_buffer_sized(false, None),
                    storage_buffer_sized(false, None),
                    storage_buffer_sized(false, None),
                    storage_buffer_sized(false, None),
                    // 19: deferred GBuffer (Rgba32Uint) for roughness/metallic
                    texture_2d(TextureSampleType::Uint),
                ),
            ),
        );

        let push_constant_ranges = vec![PushConstantRange {
            stages: ShaderStages::COMPUTE,
            range: 0..size_of::<RtPushConstants>() as u32,
        }];

        let create_pipeline =
            |label: &'static str, entry_point: &'static str, shader: Handle<Shader>| {
                pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                    label: Some(label.into()),
                    layout: vec![bind_group_layout.clone()],
                    push_constant_ranges: push_constant_ranges.clone(),
                    shader,
                    shader_defs: vec![],
                    entry_point: Some(entry_point.into()),
                    ..default()
                })
            };

        Self {
            bind_group_layout: bind_group_layout.clone(),
            hi_z_generate_pipeline: create_pipeline(
                "rt_hi_z_generate",
                "hi_z_generate",
                load_embedded_asset!(world, "shaders/hi_z_generate.wgsl"),
            ),
            ssgi_trace_pipeline: create_pipeline(
                "rt_ssgi_trace",
                "ssgi_trace",
                load_embedded_asset!(world, "shaders/ssgi_trace.wgsl"),
            ),
            radiance_cache_update_pipeline: create_pipeline(
                "rt_radiance_cache_update",
                "radiance_cache_update",
                load_embedded_asset!(world, "shaders/radiance_cache.wgsl"),
            ),
            ss_reflections_pipeline: create_pipeline(
                "rt_ss_reflections",
                "ss_reflections",
                load_embedded_asset!(world, "shaders/ss_reflections.wgsl"),
            ),
            ss_shadows_pipeline: create_pipeline(
                "rt_ss_shadows",
                "ss_shadows",
                load_embedded_asset!(world, "shaders/ss_shadows.wgsl"),
            ),
            temporal_denoise_pipeline: create_pipeline(
                "rt_temporal_denoise",
                "temporal_denoise",
                load_embedded_asset!(world, "shaders/temporal_denoise.wgsl"),
            ),
            spatial_denoise_pipeline: create_pipeline(
                "rt_spatial_denoise",
                "spatial_denoise",
                load_embedded_asset!(world, "shaders/spatial_denoise.wgsl"),
            ),
            composite_pipeline: create_pipeline(
                "rt_composite",
                "composite",
                load_embedded_asset!(world, "shaders/composite.wgsl"),
            ),
        }
    }
}
