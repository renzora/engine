//! RENZORA PATCH: pipelines and dispatch for the built-in denoiser. The shaders,
//! and the reasoning behind each stage, are in `denoise.wesl`.

use super::prepare::SolariLightingResources;
use bevy_asset::{load_embedded_asset, AssetServer};
use bevy_core_pipeline::prepass::PreviousViewData;
use bevy_render::{
    render_resource::{
        binding_types::{texture_2d, texture_depth_2d, texture_storage_2d, uniform_buffer, uniform_buffer_sized},
        BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BindingResource,
        CachedComputePipelineId, ComputePass, ComputePipelineDescriptor, PipelineCache, ShaderStages,
        StorageTextureAccess, TextureFormat, TextureSampleType, TextureView,
    },
    renderer::RenderDevice,
    view::ViewUniform,
};
use bevy_shader::ShaderDefVal;
use bevy_utils::default;

/// Spacing of each a-trous iteration, in pixels. Five doublings give the blur a
/// reach of 2 * (1 + 2 + 4 + 8 + 16) = 62 pixels for 125 taps per pixel.
const ATROUS_STEPS: [u32; 5] = [1, 2, 4, 8, 16];

pub struct DenoiserPipelines {
    temporal_layout: BindGroupLayoutDescriptor,
    atrous_layout: BindGroupLayoutDescriptor,
    atrous_feedback_layout: BindGroupLayoutDescriptor,
    composite_layout: BindGroupLayoutDescriptor,
    temporal: CachedComputePipelineId,
    /// One per entry of [`ATROUS_STEPS`]; the first also writes the history.
    atrous: [CachedComputePipelineId; 5],
    composite: CachedComputePipelineId,
}

pub fn init_denoiser_pipelines(pipeline_cache: &PipelineCache, asset_server: &AssetServer) -> DenoiserPipelines {
    let read = || texture_2d(TextureSampleType::Float { filterable: false });
    let write = || texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly);

    let temporal_layout = BindGroupLayoutDescriptor::new(
        "solari_denoiser_temporal_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<ViewUniform>(true),
                texture_2d(TextureSampleType::Uint),
                texture_depth_2d(),
                texture_2d(TextureSampleType::Uint),
                texture_depth_2d(),
                read(),
                uniform_buffer::<PreviousViewData>(true),
                read(),
                read(),
                read(),
                read(),
                read(),
                write(),
                write(),
                write(),
                write(),
                write(),
                uniform_buffer_sized(false, None),
            ),
        ),
    );
    let atrous_layout = BindGroupLayoutDescriptor::new(
        "solari_denoiser_atrous_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<ViewUniform>(true),
                texture_2d(TextureSampleType::Uint),
                texture_depth_2d(),
                read(),
                read(),
                write(),
                write(),
            ),
        ),
    );
    let atrous_feedback_layout = BindGroupLayoutDescriptor::new(
        "solari_denoiser_atrous_feedback_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<ViewUniform>(true),
                texture_2d(TextureSampleType::Uint),
                texture_depth_2d(),
                read(),
                read(),
                write(),
                write(),
                texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
            ),
        ),
    );
    let composite_layout = BindGroupLayoutDescriptor::new(
        "solari_denoiser_composite_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<ViewUniform>(true),
                texture_2d(TextureSampleType::Uint),
                texture_depth_2d(),
                read(),
                read(),
                write(),
            ),
        ),
    );

    let pipeline = |label: &'static str, entry_point: &'static str, layout: &BindGroupLayoutDescriptor, shader_defs: Vec<ShaderDefVal>| {
        pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some(label.into()),
            layout: vec![layout.clone()],
            shader: load_embedded_asset!(asset_server, "denoise.wesl"),
            shader_defs,
            entry_point: Some(entry_point.into()),
            ..default()
        })
    };

    let temporal = pipeline("solari_denoiser_temporal", "temporal", &temporal_layout, vec!["DENOISE_TEMPORAL".into()]);
    let atrous = ATROUS_STEPS.map(|step| {
        let mut defs: Vec<ShaderDefVal> = vec!["DENOISE_ATROUS".into(), ShaderDefVal::UInt("DENOISE_STEP".into(), step)];
        let layout = if step == ATROUS_STEPS[0] {
            defs.push("DENOISE_FEEDBACK".into());
            &atrous_feedback_layout
        } else {
            &atrous_layout
        };
        pipeline("solari_denoiser_atrous", "atrous", layout, defs)
    });
    let composite = pipeline("solari_denoiser_composite", "composite", &composite_layout, vec!["DENOISE_COMPOSITE".into()]);

    DenoiserPipelines {
        temporal_layout,
        atrous_layout,
        atrous_feedback_layout,
        composite_layout,
        temporal,
        atrous,
        composite,
    }
}

/// What the denoiser binds that it does not own.
pub struct DenoiserInputs<'a> {
    pub view_uniforms: BindingResource<'a>,
    pub previous_view_uniforms: BindingResource<'a>,
    pub view_uniform_offset: u32,
    pub previous_view_uniform_offset: u32,
    pub gbuffer: &'a TextureView,
    pub depth: &'a TextureView,
    pub previous_gbuffer: &'a TextureView,
    pub previous_depth: &'a TextureView,
    pub motion_vectors: &'a TextureView,
    /// The view target's raw texture view, which is not a Bevy `TextureView`.
    pub view_output: BindingResource<'a>,
}

/// Bind groups and pipelines for one view's frame, built before the compute
/// pass opens.
pub struct DenoiserDispatch<'a> {
    temporal: BindGroup,
    /// One per a-trous iteration, alternating which buffer is read and written.
    atrous: [BindGroup; 5],
    composite: BindGroup,
    pipelines: [&'a bevy_render::render_resource::ComputePipeline; 7],
    view_uniform_offset: u32,
    previous_view_uniform_offset: u32,
}

/// `None` while a pipeline is still compiling, in which case the frame shows the
/// undenoised image.
pub fn prepare_denoiser_dispatch<'a>(
    pipelines: &DenoiserPipelines,
    pipeline_cache: &'a PipelineCache,
    render_device: &RenderDevice,
    resources: &SolariLightingResources,
    inputs: DenoiserInputs,
) -> Option<DenoiserDispatch<'a>> {
    let temporal_pipeline = pipeline_cache.get_compute_pipeline(pipelines.temporal)?;
    let composite_pipeline = pipeline_cache.get_compute_pipeline(pipelines.composite)?;
    let mut atrous_pipelines = Vec::with_capacity(ATROUS_STEPS.len());
    for id in pipelines.atrous {
        atrous_pipelines.push(pipeline_cache.get_compute_pipeline(id)?);
    }

    let d = &resources.denoiser;
    let write = d.write_slot();
    let read = 1 - write;

    let temporal = render_device.create_bind_group(
        "solari_denoiser_temporal",
        &pipeline_cache.get_bind_group_layout(&pipelines.temporal_layout),
        &BindGroupEntries::sequential((
            inputs.view_uniforms.clone(),
            inputs.gbuffer,
            inputs.depth,
            inputs.previous_gbuffer,
            inputs.previous_depth,
            inputs.motion_vectors,
            inputs.previous_view_uniforms,
            &d.noisy_diffuse.default_view,
            &d.noisy_specular.default_view,
            &d.history_diffuse[read].default_view,
            &d.history_specular[read].default_view,
            &d.moments[read].default_view,
            &d.history_diffuse[write].default_view,
            &d.history_specular[write].default_view,
            &d.moments[write].default_view,
            &d.filtered_diffuse.default_view,
            &d.filtered_specular.default_view,
            d.constants.as_entire_binding(),
        )),
    );

    // The temporal pass leaves its result in `filtered_*`; each a-trous
    // iteration reads one pair and writes the other, so after five the result
    // is in `noisy_*`.
    let filtered = (&d.filtered_diffuse.default_view, &d.filtered_specular.default_view);
    let noisy = (&d.noisy_diffuse.default_view, &d.noisy_specular.default_view);
    let atrous_group = |source: (&TextureView, &TextureView), destination: (&TextureView, &TextureView)| {
        render_device.create_bind_group(
            "solari_denoiser_atrous",
            &pipeline_cache.get_bind_group_layout(&pipelines.atrous_layout),
            &BindGroupEntries::sequential((
                inputs.view_uniforms.clone(),
                inputs.gbuffer,
                inputs.depth,
                source.0,
                source.1,
                destination.0,
                destination.1,
            )),
        )
    };
    let atrous_feedback = render_device.create_bind_group(
        "solari_denoiser_atrous_feedback",
        &pipeline_cache.get_bind_group_layout(&pipelines.atrous_feedback_layout),
        &BindGroupEntries::sequential((
            inputs.view_uniforms.clone(),
            inputs.gbuffer,
            inputs.depth,
            filtered.0,
            filtered.1,
            noisy.0,
            noisy.1,
            &d.history_diffuse[write].default_view,
            &d.history_specular[write].default_view,
        )),
    );
    let atrous = [
        atrous_feedback,
        atrous_group(noisy, filtered),
        atrous_group(filtered, noisy),
        atrous_group(noisy, filtered),
        atrous_group(filtered, noisy),
    ];

    let composite = render_device.create_bind_group(
        "solari_denoiser_composite",
        &pipeline_cache.get_bind_group_layout(&pipelines.composite_layout),
        &BindGroupEntries::sequential((
            inputs.view_uniforms,
            inputs.gbuffer,
            inputs.depth,
            noisy.0,
            noisy.1,
            inputs.view_output,
        )),
    );

    Some(DenoiserDispatch {
        temporal,
        atrous,
        composite,
        pipelines: [
            temporal_pipeline,
            atrous_pipelines[0],
            atrous_pipelines[1],
            atrous_pipelines[2],
            atrous_pipelines[3],
            atrous_pipelines[4],
            composite_pipeline,
        ],
        view_uniform_offset: inputs.view_uniform_offset,
        previous_view_uniform_offset: inputs.previous_view_uniform_offset,
    })
}

impl DenoiserDispatch<'_> {
    /// Record the passes. The caller has already dispatched the lighting, so
    /// `noisy_*` holds this frame's signal.
    pub fn dispatch(&self, pass: &mut ComputePass, dx: u32, dy: u32) {
        // Bind group 0 is the denoiser's own, and the lighting's groups 1 and 2
        // are not part of these pipelines' layouts.
        pass.set_bind_group(1, None, &[]);
        pass.set_bind_group(2, None, &[]);

        pass.set_pipeline(self.pipelines[0]);
        pass.set_bind_group(0, &self.temporal, &[self.view_uniform_offset, self.previous_view_uniform_offset]);
        pass.dispatch_workgroups(dx, dy, 1);

        for (i, group) in self.atrous.iter().enumerate() {
            pass.set_pipeline(self.pipelines[1 + i]);
            pass.set_bind_group(0, group, &[self.view_uniform_offset]);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        pass.set_pipeline(self.pipelines[6]);
        pass.set_bind_group(0, &self.composite, &[self.view_uniform_offset]);
        pass.dispatch_workgroups(dx, dy, 1);
    }
}
