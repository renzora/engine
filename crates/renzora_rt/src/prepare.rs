use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_depth_2d, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::{ViewTarget, ViewUniform};
use bevy::utils::default;
use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicU32, Ordering};

use renzora::RtLighting;

/// Per-view uniform fed to the SSGI shader. Padded to 16-byte alignment.
#[derive(Clone, Copy, Debug, Pod, Zeroable, ShaderType)]
#[repr(C)]
pub struct RtConfig {
    pub intensity: f32,
    /// Frame counter, used by the shader to (a) decorrelate per-pixel ray
    /// noise across frames so temporal accumulation has something to
    /// average, and (b) match the CPU-side ping-pong index so the shader
    /// knows which history target it's reading vs writing.
    pub frame_count: u32,
    /// Debug output mode: 0 = composite, 1 = indirect only.
    /// See `RtDebugMode` in the `renzora` contract.
    pub debug_mode: u32,
    pub _pad0: f32,
}

#[derive(Resource)]
pub struct RtPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub pipeline_id: CachedRenderPipelineId,
    pub sampler: Sampler,
    /// Monotonic frame counter shared across views. Wraps at u32::MAX,
    /// which is far past any session length; the shader only cares about
    /// parity for ping-pong and low bits for noise.
    pub frame_count: AtomicU32,
}

#[derive(Component)]
pub struct RtViewResources {
    pub config_buffer: Buffer,
    /// Ping-pong history buffers. Each frame one is read (last frame's
    /// SSGI + linear depth in alpha) and the other is written. Stored in
    /// linear HDR space; alpha holds linear depth at write time so the
    /// shader can do a depth-disocclusion check on reuse.
    pub history_a: TextureView,
    pub history_b: TextureView,
    /// Cached size to detect view-target resizes; we recreate the history
    /// textures when this drifts.
    pub history_size: Extent3d,
    /// Frames the pass still sits out after the history was (re)allocated.
    ///
    /// Running straight through a resize left the sky black until the camera
    /// moved: with SSGI on, every viewport resize lost the sky and it stayed
    /// lost, while with SSGI off no resize ever did. Skipping the pass for a
    /// moment always brought the sky back for good, so a fresh allocation
    /// starts with that pause instead of waiting for someone to orbit.
    pub settle_frames: u32,
}

/// Frames [`RtViewResources::settle_frames`] starts at. A few frames of no
/// bounce light after a resize are invisible; the black sky was not.
const SETTLE_FRAMES: u32 = 4;

impl FromWorld for RtPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = BindGroupLayoutDescriptor::new(
            "rt_ssgi_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    // 0: scene HDR (post-process source)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 1: linear sampler
                    sampler(SamplerBindingType::Filtering),
                    // 2: depth prepass
                    texture_depth_2d(),
                    // 3: normal prepass
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // 4: previous-frame history (RGB = indirect, A = linear depth)
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    // 5: motion vectors prepass (Rg16Float, NDC-space delta)
                    texture_2d(TextureSampleType::Float { filterable: false }),
                    // 6: view uniform
                    uniform_buffer::<ViewUniform>(true),
                    // 7: rt config
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
        let shader = asset_server.load("embedded://renzora_rt/ssgi.wesl");

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
                targets: vec![
                    // Target 0: composited scene+indirect → view target.
                    Some(ColorTargetState {
                        // 0.19: TEXTURE_FORMAT_HDR deprecated.
                        format: TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    }),
                    // Target 1: new history (indirect.rgb, linear_depth).
                    Some(ColorTargetState {
                        format: TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    }),
                ],
                constants: vec![],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            immediate_size: 0,
            zero_initialize_workgroup_memory: false,
        });

        Self {
            layout,
            pipeline_id,
            sampler,
            frame_count: AtomicU32::new(0),
        }
    }
}

/// Allocate / recreate per-view resources, advance the frame counter,
/// and write the per-frame uniform.
pub fn prepare_rt_uniforms(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    pipeline: Res<RtPipeline>,
    mut views: Query<(Entity, &RtLighting, &ViewTarget, Option<&mut RtViewResources>)>,
) {
    let frame_count = pipeline.frame_count.fetch_add(1, Ordering::Relaxed);

    for (entity, settings, view_target, existing) in &mut views {
        let target_size = view_target.main_texture().size();

        let config = RtConfig {
            intensity: if settings.enabled { settings.intensity } else { 0.0 },
            frame_count,
            debug_mode: settings.debug.as_u32(),
            _pad0: 0.0,
        };

        // Reuse existing resources if size still matches; otherwise
        // (re)allocate history textures + config buffer.
        let config_buffer = match existing {
            Some(mut res) if res.history_size == target_size => {
                res.settle_frames = res.settle_frames.saturating_sub(1);
                render_queue.write_buffer(&res.config_buffer, 0, bytemuck::bytes_of(&config));
                continue;
            }
            Some(res) => Some(res.config_buffer.clone()),
            None => None,
        };

        {
            let history_a = render_device.create_texture(&TextureDescriptor {
                label: Some("rt_history_a"),
                size: target_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let history_b = render_device.create_texture(&TextureDescriptor {
                label: Some("rt_history_b"),
                size: target_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let config_buffer = config_buffer.unwrap_or_else(|| {
                render_device.create_buffer(&BufferDescriptor {
                    label: Some("rt_config_buffer"),
                    size: std::mem::size_of::<RtConfig>() as u64,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            });
            render_queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&config));
            commands.entity(entity).insert(RtViewResources {
                config_buffer,
                history_a: history_a.create_view(&TextureViewDescriptor::default()),
                history_b: history_b.create_view(&TextureViewDescriptor::default()),
                history_size: target_size,
                settle_frames: SETTLE_FRAMES,
            });
        }
    }
}
