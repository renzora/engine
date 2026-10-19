//! Drawing a camera's [`PostEffectChain`]: one render pass that runs each
//! effect in turn, each reading what the one before it wrote.
//!
//! Pipelines are cached by shader and created the first time a chain uses one,
//! so a project that never adds an effect builds none. The settings buffer is
//! made fresh each frame: a handful of 16 to 64 byte uploads per camera, which
//! is less than keeping per-effect buffers in step with a list the user is
//! reordering would cost in bookkeeping.
//!
//! Runs in `RenderPhase::LdrPost`, on the tonemapped frame, where the effect
//! plugins this replaced also ran.

use bevy::core_pipeline::FullscreenShader;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::{
    render_resource::{
        binding_types::{sampler, texture_2d, uniform_buffer_sized},
        BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
        BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState,
        ColorWrites, Extent3d, FragmentState, Operations, PipelineCache,
        RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
        SamplerBindingType, SamplerDescriptor, ShaderStages, TextureDescriptor,
        TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
        TextureViewDescriptor,
    },
    renderer::{RenderContext, RenderDevice},
    view::ViewTarget,
    Render, RenderApp, RenderStartup, RenderSystems,
};
use renzora::postprocess::{PostProcessCorePlugin, RenderCompositionAppExt, RenderPass};
use renzora::RenderPhase;

use crate::PostEffectChain;

/// The smallest settings buffer, for an effect that declares no settings: the
/// layout still has binding 2, and a zero-sized buffer cannot be bound.
const MIN_SETTINGS: usize = 16;

/// The frame format after tonemapping, and the one an HDR camera keeps.
const LDR_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;
const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub(crate) fn build(app: &mut App) {
    if !app.is_plugin_added::<PostProcessCorePlugin>() {
        app.add_plugins(PostProcessCorePlugin);
    }
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .add_systems(RenderStartup, init_pipelines)
        .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
        .add_render_pass("post_effects", RenderPhase::LdrPost, 0.0, PostEffectsPass);
}

#[derive(Resource)]
struct EffectPipelines {
    layout: BindGroupLayoutDescriptor,
    layout_snapshot: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
    /// Copies the live frame into a snapshot texture.
    blit_layout: BindGroupLayoutDescriptor,
    blit: CachedRenderPipelineId,
    /// `[ldr, hdr]` per shader and whether it reads a snapshot.
    cache: HashMap<(AssetId<Shader>, bool), [CachedRenderPipelineId; 2]>,
}

impl EffectPipelines {
    fn pipeline_for(
        &mut self,
        pipeline_cache: &PipelineCache,
        shader: &Handle<Shader>,
        snapshot: bool,
    ) -> [CachedRenderPipelineId; 2] {
        let layout = if snapshot {
            self.layout_snapshot.clone()
        } else {
            self.layout.clone()
        };
        let vertex = self.fullscreen.to_vertex_state();
        *self.cache.entry((shader.id(), snapshot)).or_insert_with(|| {
            let queue = |format: TextureFormat| {
                pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                    label: Some("post_effect_pipeline".into()),
                    layout: vec![layout.clone()],
                    vertex: vertex.clone(),
                    fragment: Some(FragmentState {
                        shader: shader.clone(),
                        targets: vec![Some(ColorTargetState {
                            format,
                            blend: None,
                            write_mask: ColorWrites::ALL,
                        })],
                        ..default()
                    }),
                    ..default()
                })
            };
            [queue(LDR_FORMAT), queue(HDR_FORMAT)]
        })
    }
}

fn init_pipelines(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let texture = || texture_2d(TextureSampleType::Float { filterable: true });
    let filtering = || sampler(SamplerBindingType::Filtering);
    let layout = BindGroupLayoutDescriptor::new(
        "post_effect_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture(), filtering(), uniform_buffer_sized(false, None)),
        ),
    );
    let layout_snapshot = BindGroupLayoutDescriptor::new(
        "post_effect_layout_snapshot",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture(),
                filtering(),
                uniform_buffer_sized(false, None),
                texture(),
                filtering(),
            ),
        ),
    );
    let blit_layout = BindGroupLayoutDescriptor::new(
        "post_effect_blit_layout",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (texture(), filtering())),
    );
    // The pass-through shader `PostProcessCorePlugin` embeds for its own
    // snapshots.
    let blit = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("post_effect_blit".into()),
        layout: vec![blit_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: asset_server.load("embedded://renzora/postprocess_copy.wesl"),
            targets: vec![Some(ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(EffectPipelines {
        layout,
        layout_snapshot,
        sampler: render_device.create_sampler(&SamplerDescriptor::default()),
        fullscreen: fullscreen.clone(),
        blit_layout,
        blit,
        cache: HashMap::default(),
    });
}

/// One effect of a view's chain, ready to record.
struct PreparedPass {
    pipeline: [CachedRenderPipelineId; 2],
    settings: Buffer,
    snapshot: bool,
    frozen: bool,
}

#[derive(Component)]
struct PreparedEffects(Vec<PreparedPass>);

/// Snapshot textures by position in the chain. Kept on the view across frames,
/// because a frozen snapshot is only useful if it outlives the frame it was
/// taken in.
#[derive(Component, Default)]
struct EffectSnapshots(HashMap<usize, Snapshot>);

struct Snapshot {
    view: TextureView,
    size: Extent3d,
}

fn prepare(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<EffectPipelines>,
    mut views: Query<(
        Entity,
        &ViewTarget,
        &PostEffectChain,
        Option<&mut EffectSnapshots>,
    )>,
) {
    for (entity, target, chain, snapshots) in &mut views {
        let mut new_snapshots = EffectSnapshots::default();
        let snapshots = match snapshots {
            Some(s) => s.into_inner(),
            None => &mut new_snapshots,
        };
        let size = target.main_texture().size();

        let mut passes = Vec::with_capacity(chain.passes.len());
        for (index, pass) in chain.passes.iter().enumerate() {
            let pipeline = pipelines.pipeline_for(&pipeline_cache, &pass.shader, pass.snapshot);
            let mut bytes = pass.settings.clone();
            bytes.resize(bytes.len().max(MIN_SETTINGS), 0);
            let settings = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("post_effect_settings"),
                contents: &bytes,
                usage: BufferUsages::UNIFORM,
            });
            if pass.snapshot && snapshots.0.get(&index).is_none_or(|s| s.size != size) {
                let texture = render_device.create_texture(&TextureDescriptor {
                    label: Some("post_effect_snapshot"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: HDR_FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                snapshots.0.insert(
                    index,
                    Snapshot {
                        view: texture.create_view(&TextureViewDescriptor::default()),
                        size,
                    },
                );
            }
            passes.push(PreparedPass {
                pipeline,
                settings,
                snapshot: pass.snapshot,
                frozen: pass.frozen,
            });
        }

        let mut ec = commands.entity(entity);
        ec.insert(PreparedEffects(passes));
        if !new_snapshots.0.is_empty() {
            ec.insert(new_snapshots);
        }
    }
}

struct PostEffectsPass;

impl RenderPass for PostEffectsPass {
    fn run(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) {
        // Gate on the chain itself, not only on what `prepare` left: the
        // retained render world keeps `PreparedEffects` on a view after its
        // chain is extracted away, and drawing it would be a ghost of the last
        // effects the camera had.
        if world.get::<PostEffectChain>(view_entity).is_none() {
            return;
        }
        let Some(prepared) = world.get::<PreparedEffects>(view_entity) else {
            return;
        };
        let pipelines = world.resource::<EffectPipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let snapshots = world.get::<EffectSnapshots>(view_entity);
        let hdr = view_target.main_texture_format() == HDR_FORMAT;
        let device = render_context.render_device().clone();

        for (index, pass) in prepared.0.iter().enumerate() {
            // Look the pipeline up before taking the ping-pong pair: taking it
            // flips which texture is current, so skipping the draw afterwards
            // would show a frame nothing was written to.
            let Some(pipeline) = pipeline_cache.get_render_pipeline(pass.pipeline[hdr as usize])
            else {
                continue;
            };
            let frame = view_target.post_process_write();

            let bind_group = if pass.snapshot {
                let snapshot = snapshots.and_then(|s| s.0.get(&index));
                if !pass.frozen {
                    if let (Some(snap), Some(blit)) =
                        (snapshot, pipeline_cache.get_render_pipeline(pipelines.blit))
                    {
                        let blit_group = device.create_bind_group(
                            "post_effect_blit",
                            &pipeline_cache.get_bind_group_layout(&pipelines.blit_layout),
                            &BindGroupEntries::sequential((frame.source, &pipelines.sampler)),
                        );
                        let mut blit_pass =
                            render_context.begin_tracked_render_pass(RenderPassDescriptor {
                                label: Some("post_effect_blit"),
                                color_attachments: &[Some(RenderPassColorAttachment {
                                    view: &snap.view,
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: Operations::default(),
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                                multiview_mask: None,
                            });
                        blit_pass.set_render_pipeline(blit);
                        blit_pass.set_bind_group(0, &blit_group, &[]);
                        blit_pass.draw(0..3, 0..1);
                    }
                }
                // No snapshot yet means the frame stands in for it, so a
                // transition blends the picture with itself rather than with
                // an empty texture.
                let frozen_view = snapshot.map(|s| &s.view).unwrap_or(frame.source);
                device.create_bind_group(
                    "post_effect",
                    &pipeline_cache.get_bind_group_layout(&pipelines.layout_snapshot),
                    &BindGroupEntries::sequential((
                        frame.source,
                        &pipelines.sampler,
                        pass.settings.as_entire_binding(),
                        frozen_view,
                        &pipelines.sampler,
                    )),
                )
            } else {
                device.create_bind_group(
                    "post_effect",
                    &pipeline_cache.get_bind_group_layout(&pipelines.layout),
                    &BindGroupEntries::sequential((
                        frame.source,
                        &pipelines.sampler,
                        pass.settings.as_entire_binding(),
                    )),
                )
            };

            let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("post_effect"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: frame.destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations::default(),
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_render_pipeline(pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}
