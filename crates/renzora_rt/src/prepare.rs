use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_depth_2d, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::{ViewTarget, ViewUniform};
use bevy::utils::default;
use bytemuck::{Pod, Zeroable};

use crate::RtLighting;

/// Per-view uniform fed to the SSGI shader. Padded to 16-byte alignment.
#[derive(Clone, Copy, Debug, Pod, Zeroable, ShaderType)]
#[repr(C)]
pub struct RtConfig {
    pub intensity: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

#[derive(Resource)]
pub struct RtPipeline {
    /// `BindGroupLayoutDescriptor` is what the pipeline cache + bind-group
    /// builder consume in Bevy 0.18; the actual `BindGroupLayout` is
    /// resolved lazily via `pipeline_cache.get_bind_group_layout(&desc)`.
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedRenderPipelineId,
    pub sampler: Sampler,
}

/// Per-view config buffer. Bind group is built inside the render node
/// because `ViewTarget::post_process_write()` returns a different source
/// view every frame (ping-pong) and can't be cached here.
#[derive(Component)]
pub struct RtViewResources {
    pub config_buffer: Buffer,
}

impl FromWorld for RtPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "rt_ssgi_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // 0: scene HDR (post-process source — built per-frame in node)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 1: linear sampler
                    sampler(SamplerBindingType::Filtering),
                    // 2: depth prepass
                    texture_depth_2d(),
                    // 3: normal prepass (world-space, packed in [0,1])
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // 4: view uniform (dynamic offset)
                    uniform_buffer::<ViewUniform>(true),
                    // 5: rt config
                    uniform_buffer::<RtConfig>(false),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });

        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("embedded://renzora_rt/ssgi.wgsl");

        let fullscreen = world.resource::<bevy::core_pipeline::FullscreenShader>();

        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("rt_ssgi_pipeline".into()),
            layout: vec![layout.clone()],
            vertex: fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader,
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: ViewTarget::TEXTURE_FORMAT_HDR,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: false,
        });

        Self { layout, pipeline_id, sampler }
    }
}

/// Allocate per-view config buffers and write the latest settings.
pub fn prepare_rt_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    views: Query<(Entity, &RtLighting, Option<&RtViewResources>)>,
) {
    for (entity, settings, existing) in &views {
        let config = RtConfig {
            intensity: if settings.enabled { settings.intensity } else { 0.0 },
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };

        let buffer = if let Some(res) = existing {
            res.config_buffer.clone()
        } else {
            let buffer = render_device.create_buffer(&BufferDescriptor {
                label: Some("rt_config_buffer"),
                size: std::mem::size_of::<RtConfig>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            commands
                .entity(entity)
                .insert(RtViewResources { config_buffer: buffer.clone() });
            buffer
        };

        render_queue.write_buffer(&buffer, 0, bytemuck::bytes_of(&config));
    }
}
