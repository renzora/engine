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

use crate::RtLighting;

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
    pub _pad0: f32,
    pub _pad1: f32,
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
}

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
                    // 5: view uniform
                    uniform_buffer::<ViewUniform>(true),
                    // 6: rt config
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
                targets: vec![
                    // Target 0: composited scene+indirect → view target.
                    Some(ColorTargetState {
                        format: ViewTarget::TEXTURE_FORMAT_HDR,
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
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: vec![],
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
    views: Query<(Entity, &RtLighting, &ViewTarget, Option<&RtViewResources>)>,
) {
    let frame_count = pipeline.frame_count.fetch_add(1, Ordering::Relaxed);

    for (entity, settings, view_target, existing) in &views {
        let target_size = view_target.main_texture().size();

        let config = RtConfig {
            intensity: if settings.enabled { settings.intensity } else { 0.0 },
            frame_count,
            _pad0: 0.0,
            _pad1: 0.0,
        };

        // Reuse existing resources if size still matches; otherwise
        // (re)allocate history textures + config buffer.
        let needs_alloc = match existing {
            Some(res) => res.history_size != target_size,
            None => true,
        };

        let res = if needs_alloc {
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
            let config_buffer = if let Some(prev) = existing {
                prev.config_buffer.clone()
            } else {
                render_device.create_buffer(&BufferDescriptor {
                    label: Some("rt_config_buffer"),
                    size: std::mem::size_of::<RtConfig>() as u64,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            };
            let res = RtViewResources {
                config_buffer,
                history_a: history_a.create_view(&TextureViewDescriptor::default()),
                history_b: history_b.create_view(&TextureViewDescriptor::default()),
                history_size: target_size,
            };
            commands.entity(entity).insert(res.clone());
            res
        } else {
            existing.unwrap().clone()
        };

        render_queue.write_buffer(&res.config_buffer, 0, bytemuck::bytes_of(&config));
    }
}

// `RtViewResources` needs Clone for the reuse-or-alloc path above. The
// inner `Buffer`/`TextureView` are Arc-wrapped by wgpu so this is cheap.
impl Clone for RtViewResources {
    fn clone(&self) -> Self {
        Self {
            config_buffer: self.config_buffer.clone(),
            history_a: self.history_a.clone(),
            history_b: self.history_b.clone(),
            history_size: self.history_size,
        }
    }
}
