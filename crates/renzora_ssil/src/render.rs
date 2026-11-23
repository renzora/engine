//! The GPU half of SSIL, in [`RenderPhase::Gi`]: after the main pass has lit
//! the frame and before TAA, so the bounce is part of what TAA accumulates.
//!
//! 1. **Gather**, at half resolution: ASSAO's mirrored disk taps read last
//!    frame's colour from the history's blurred mip, and write the light, the
//!    obscurance and the packed depth edges.
//! 2. **Blur**, four passes at half resolution, two wide then two narrow, each
//!    stopping at the edges the gather found.
//! 3. **Composite**, at full resolution: the blurred result read bilinearly,
//!    the blocked ambient light taken off and the bounce put on, both on the
//!    pixel's diffuse colour from the G-buffer.
//! 4. **History**: the finished frame copied into a full-resolution texture
//!    and downsampled through its mips to 1x1, for the next frame's gather. It
//!    holds this frame's bounce too, so light keeps bouncing over frames.

use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::core_pipeline::FullscreenShader;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    extract_component::ExtractComponentPlugin,
    render_asset::RenderAssets,
    render_resource::{
        binding_types::{sampler, texture_2d, texture_cube, texture_depth_2d, uniform_buffer_sized},
        encase::UniformBuffer,
        AddressMode, BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        Buffer, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState,
        ColorWrites, Extent3d, FilterMode, FragmentState, MipmapFilterMode, Operations,
        PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
        RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
        ShaderType, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
        TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
    },
    renderer::{RenderContext, RenderDevice},
    sync_world::RenderEntity,
    texture::{CachedTexture, GpuImage, TextureCache},
    view::{ExtractedView, ViewTarget},
    Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
};
use renzora_viewport_api::{ViewportSettings, VisualizationMode};
use renzora_render_api::postprocess::{PostProcessCorePlugin, RenderCompositionAppExt, RenderPass};
use renzora_scene_api::{EditorCamera, RenderPhase};

use crate::Ssil;

/// The HDR frame format, and the format of the light and the history.
const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
/// Four depth edges, two bits each.
const EDGES_FORMAT: TextureFormat = TextureFormat::R8Unorm;
/// The history mip the taps read. At 1/32 of the screen it stands for an area
/// rather than a pixel: the taps then sample light that is already smooth,
/// which is most of what keeps them from noise.
const GATHER_MIP: u32 = 5;
/// Blur passes: the first two wide, the rest narrow.
const BLUR_PASSES: usize = 4;

pub(crate) fn build(app: &mut App) {
    bevy::asset::embedded_asset!(app, "ssil.wesl");
    app.add_plugins(ExtractComponentPlugin::<Ssil>::default());

    if !app.is_plugin_added::<PostProcessCorePlugin>() {
        app.add_plugins(PostProcessCorePlugin);
    }
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<SsilHistories>()
        .init_resource::<SsilDebug>()
        .add_systems(ExtractSchedule, extract_debug)
        .add_systems(RenderStartup, init_pipelines)
        .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
        .add_render_pass("ssil", RenderPhase::Gi, 0.0, SsilPass);
}

/// The viewport's SSIL debug view, and the editor cameras it applies to.
///
/// Chosen in the viewport's Display ▸ Visualization ▸ SSIL menu, not on the
/// component, so a debug view can never reach a game camera, a thumbnail or
/// the Play window.
#[derive(Resource, Default)]
struct SsilDebug {
    /// 0 off, 1 light, 2 occlusion, 3 history; the shader's numbering.
    mode: u32,
    views: HashSet<Entity>,
}

fn extract_debug(
    mut debug: ResMut<SsilDebug>,
    settings: Extract<Option<Res<ViewportSettings>>>,
    cameras: Extract<Query<&RenderEntity, With<EditorCamera>>>,
) {
    debug.mode = match settings.as_ref().map(|s| s.visualization_mode) {
        Some(VisualizationMode::SsilLight) => 1,
        Some(VisualizationMode::SsilOcclusion) => 2,
        Some(VisualizationMode::SsilHistory) => 3,
        _ => 0,
    };
    debug.views.clear();
    if debug.mode != 0 {
        debug.views.extend(cameras.iter().map(|e| e.id()));
    }
}

#[derive(ShaderType)]
struct SsilUniform {
    clip_from_view: Mat4,
    view_from_clip: Mat4,
    view_from_world: Mat4,
    reprojection: Mat4,
    /// Full resolution: width, height, 1 / width, 1 / height.
    full_size: Vec4,
    /// Half resolution, the same way.
    half_size: Vec4,
    /// Radius in metres, bounce intensity times pi, 1 - sharpness, taps.
    params: Vec4,
    /// Normal rejection, 1 / near limit, -1 / radius, ambient scale.
    extra: Vec4,
    /// Fade multiply, fade add, the mip the taps read, the 1x1 average mip.
    fade: Vec4,
    /// The inverse of the environment light's rotation.
    env_rotation: Vec4,
    /// The debug view, occlusion strength times pi, unused, unused.
    misc: Vec4,
}

#[derive(Resource)]
struct SsilPipelines {
    gather_layout: BindGroupLayoutDescriptor,
    blur_layout: BindGroupLayoutDescriptor,
    downsample_layout: BindGroupLayoutDescriptor,
    composite_layout: BindGroupLayoutDescriptor,
    linear: Sampler,
    mip: Sampler,
    mirror: Sampler,
    /// Bound in place of an environment light when the camera has none. The
    /// ambient scale is 0 then, so it is never read.
    no_environment: TextureView,
    gather: CachedRenderPipelineId,
    blur_wide: CachedRenderPipelineId,
    blur_smart: CachedRenderPipelineId,
    downsample: CachedRenderPipelineId,
    composite: CachedRenderPipelineId,
}

fn init_pipelines(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    // One shader holds all five entry points, so each binding has one index
    // across them; a pass's layout lists only the ones it reads.
    let depth = || texture_depth_2d();
    let gbuffer = || texture_2d(TextureSampleType::Uint);
    let color = || texture_2d(TextureSampleType::Float { filterable: true });
    let filtering = || sampler(SamplerBindingType::Filtering);
    let uniform = || uniform_buffer_sized(false, None);
    let layout = BindGroupLayoutDescriptor::new;
    let gather_layout = layout(
        "ssil_gather_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::FRAGMENT,
            ((0, depth()), (1, gbuffer()), (4, uniform()), (7, color()), (8, filtering())),
        ),
    );
    let blur_layout = layout(
        "ssil_blur_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::FRAGMENT,
            ((4, uniform()), (5, color()), (9, color()), (10, filtering())),
        ),
    );
    let downsample_layout = layout(
        "ssil_downsample_layout",
        &BindGroupLayoutEntries::with_indices(ShaderStages::FRAGMENT, ((3, filtering()), (5, color()))),
    );
    let composite_layout = layout(
        "ssil_composite_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::FRAGMENT,
            (
                (0, depth()),
                (1, gbuffer()),
                (2, color()),
                (3, filtering()),
                (4, uniform()),
                (6, color()),
                (7, color()),
                (8, filtering()),
                (11, texture_cube(TextureSampleType::Float { filterable: true })),
            ),
        ),
    );
    let shader: Handle<Shader> = asset_server.load("embedded://renzora_ssil/ssil.wesl");
    let target = |format: TextureFormat| {
        Some(ColorTargetState {
            format,
            blend: None,
            write_mask: ColorWrites::ALL,
        })
    };
    let pipeline = |label: &'static str,
                    layout: &BindGroupLayoutDescriptor,
                    entry: &'static str,
                    targets: Vec<Option<ColorTargetState>>| {
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some(label.into()),
            layout: vec![layout.clone()],
            vertex: fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: shader.clone(),
                entry_point: Some(entry.into()),
                targets,
                ..default()
            }),
            ..default()
        })
    };
    let gather = pipeline(
        "ssil_gather",
        &gather_layout,
        "gather",
        vec![target(HDR_FORMAT), target(EDGES_FORMAT)],
    );
    let blur_wide = pipeline("ssil_blur_wide", &blur_layout, "blur_wide", vec![target(HDR_FORMAT)]);
    let blur_smart = pipeline("ssil_blur_smart", &blur_layout, "blur_smart", vec![target(HDR_FORMAT)]);
    let downsample = pipeline(
        "ssil_downsample",
        &downsample_layout,
        "downsample",
        vec![target(HDR_FORMAT)],
    );
    let composite = pipeline(
        "ssil_composite",
        &composite_layout,
        "composite",
        vec![target(HDR_FORMAT)],
    );

    let sampler_with = |label: &'static str, address: AddressMode, mipmap: MipmapFilterMode| {
        render_device.create_sampler(&SamplerDescriptor {
            label: Some(label),
            address_mode_u: address,
            address_mode_v: address,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: mipmap,
            ..default()
        })
    };
    let no_environment = render_device
        .create_texture(&TextureDescriptor {
            label: Some("ssil_no_environment"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: HDR_FORMAT,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
    commands.insert_resource(SsilPipelines {
        gather_layout,
        blur_layout,
        downsample_layout,
        composite_layout,
        linear: sampler_with("ssil_linear", AddressMode::ClampToEdge, MipmapFilterMode::Nearest),
        mip: sampler_with("ssil_mip", AddressMode::ClampToEdge, MipmapFilterMode::Linear),
        mirror: sampler_with("ssil_mirror", AddressMode::MirrorRepeat, MipmapFilterMode::Nearest),
        no_environment,
        gather,
        blur_wide,
        blur_smart,
        downsample,
        composite,
    });
}

/// One view's last frame, kept across frames: the finished image with its
/// mips, and the matrix it was rendered with, which the next frame's taps
/// reproject through.
struct History {
    size: Extent3d,
    /// All mips, for the gather to sample.
    sampled: TextureView,
    /// One view per mip, for writing them.
    mips: Vec<TextureView>,
    clip_from_world: Mat4,
    /// Held so the texture lives as long as its views.
    _texture: Texture,
}

impl History {
    fn new(render_device: &RenderDevice, size: Extent3d, clip_from_world: Mat4) -> Self {
        // The whole chain down to 1x1, whose one texel is the frame's average:
        // the gather compresses its taps relative to it.
        let mip_count = size.max_mips(TextureDimension::D2);
        // New textures start zeroed, so the first frame gathers no light
        // rather than garbage.
        let texture = render_device.create_texture(&TextureDescriptor {
            label: Some("ssil_history"),
            size,
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: HDR_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let mips = (0..mip_count)
            .map(|mip| {
                texture.create_view(&TextureViewDescriptor {
                    label: Some("ssil_history_mip"),
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..default()
                })
            })
            .collect();
        Self {
            size,
            sampled: texture.create_view(&TextureViewDescriptor::default()),
            mips,
            clip_from_world,
            _texture: texture,
        }
    }
}

/// Every SSIL view's [`History`]. A view that stops drawing SSIL loses its
/// entry the next frame.
#[derive(Resource, Default)]
struct SsilHistories(HashMap<Entity, History>);

/// A view's per-frame SSIL resources.
#[derive(Component)]
struct PreparedSsil {
    uniform: Buffer,
    light: CachedTexture,
    scratch: CachedTexture,
    edges: CachedTexture,
    /// The camera's diffuse environment light as a cube, if it has one.
    environment: Option<TextureView>,
    /// The debug view this view draws, 0 for none. See [`SsilDebug`].
    debug_view: u32,
}

#[allow(clippy::type_complexity)]
fn prepare(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    mut histories: ResMut<SsilHistories>,
    debug: Res<SsilDebug>,
    images: Res<RenderAssets<GpuImage>>,
    views: Query<(Entity, &ExtractedView, &ViewTarget, &Ssil, Option<&ExtractedCamera>)>,
) {
    let mut seen = Vec::new();
    for (entity, view, target, ssil, camera) in &views {
        let full = target.main_texture().size();
        if full.width == 0 || full.height == 0 {
            continue;
        }
        seen.push(entity);
        let half = Extent3d {
            width: full.width.div_ceil(2),
            height: full.height.div_ceil(2),
            depth_or_array_layers: 1,
        };
        let mut half_texture = |label: &'static str, format: TextureFormat| {
            texture_cache.get(
                &render_device,
                TextureDescriptor {
                    label: Some(label),
                    size: half,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };
        let light = half_texture("ssil_light", HDR_FORMAT);
        let scratch = half_texture("ssil_scratch", HDR_FORMAT);
        let edges = half_texture("ssil_edges", EDGES_FORMAT);

        let world_from_view = view.world_from_view.to_matrix();
        let view_from_world = world_from_view.inverse();
        let clip_from_world = view.clip_from_view * view_from_world;
        let history = histories.0.entry(entity).or_insert_with(|| History::new(&render_device, full, clip_from_world));
        if history.size != full {
            *history = History::new(&render_device, full, clip_from_world);
        }
        // Last frame's matrix, taken before this frame's replaces it.
        let reprojection = history.clip_from_world * world_from_view;
        history.clip_from_world = clip_from_world;
        let average_mip = (history.mips.len() - 1) as u32;
        let gather_mip = GATHER_MIP.min(average_mip) as f32;

        // The near limit keeps the disk from outgrowing the screen up close.
        let tan_half_fov_y = 1.0 / view.clip_from_view.y_axis.y;
        let radius_near_limit = ssil.radius * 1.2 / tan_half_fov_y;
        let fade_span = ssil.fade_end - ssil.fade_start;

        let exposure = camera.map_or(1.0, |c| c.exposure);
        let (environment, ambient_scale, env_rotation) = match ssil.ambient.as_ref().and_then(|ambient| {
            let image = images.get(&ambient.diffuse_map)?;
            let is_cube = image
                .texture_view_descriptor
                .as_ref()
                .and_then(|d| d.dimension)
                == Some(TextureViewDimension::Cube);
            is_cube.then(|| (image.texture_view.clone(), ambient))
        }) {
            Some((view, ambient)) => (
                Some(view),
                ambient.intensity * exposure,
                Vec4::from(ambient.rotation.inverse()),
            ),
            None => (None, 0.0, Vec4::from(Quat::IDENTITY)),
        };

        let debug_view = if debug.views.contains(&entity) { debug.mode } else { 0 };
        let size = |w: u32, h: u32| Vec4::new(w as f32, h as f32, 1.0 / w as f32, 1.0 / h as f32);
        let uniform = SsilUniform {
            clip_from_view: view.clip_from_view,
            view_from_clip: view.clip_from_view.inverse(),
            view_from_world,
            reprojection,
            full_size: size(full.width, full.height),
            half_size: size(half.width, half.height),
            params: Vec4::new(
                ssil.radius,
                ssil.intensity * std::f32::consts::PI,
                1.0 - ssil.sharpness,
                ssil.samples as f32,
            ),
            extra: Vec4::new(
                ssil.normal_rejection,
                1.0 / radius_near_limit,
                -1.0 / ssil.radius,
                ambient_scale,
            ),
            fade: Vec4::new(
                -1.0 / fade_span,
                ssil.fade_start / fade_span + 1.0,
                gather_mip,
                average_mip as f32,
            ),
            env_rotation,
            misc: Vec4::new(
                debug_view as f32,
                ssil.occlusion * std::f32::consts::PI,
                0.0,
                0.0,
            ),
        };
        let mut bytes = UniformBuffer::new(Vec::<u8>::new());
        if bytes.write(&uniform).is_err() {
            continue;
        }
        let uniform = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("ssil_uniform"),
            contents: bytes.as_ref(),
            usage: BufferUsages::UNIFORM,
        });
        commands.entity(entity).insert(PreparedSsil {
            uniform,
            light,
            scratch,
            edges,
            environment,
            debug_view,
        });
    }
    histories.0.retain(|entity, _| seen.contains(entity));
}

struct SsilPass;

impl RenderPass for SsilPass {
    fn run(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) {
        // Gate on the extracted `Ssil`, not only on what `prepare` left: the
        // retained render world keeps `PreparedSsil` on a view after SSIL is
        // taken away, and drawing it would keep a bounce nobody asked for.
        if world.get::<Ssil>(view_entity).is_none() {
            return;
        }
        let (Some(prepared), Some(prepass)) = (
            world.get::<PreparedSsil>(view_entity),
            world.get::<ViewPrepassTextures>(view_entity),
        ) else {
            return;
        };
        let Some(history) = world.resource::<SsilHistories>().0.get(&view_entity) else {
            return;
        };
        let (Some(depth), Some(gbuffer)) = (prepass.depth_only_view(), prepass.deferred_view()) else {
            return;
        };
        if view_target.main_texture_format() != HDR_FORMAT {
            return;
        }
        let pipelines = world.resource::<SsilPipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();
        // Look every pipeline up before taking the ping-pong pair: taking it
        // flips which texture is current, and skipping the composite after
        // that would show a frame nothing was written to.
        let (Some(gather), Some(blur_wide), Some(blur_smart), Some(downsample), Some(composite)) = (
            pipeline_cache.get_render_pipeline(pipelines.gather),
            pipeline_cache.get_render_pipeline(pipelines.blur_wide),
            pipeline_cache.get_render_pipeline(pipelines.blur_smart),
            pipeline_cache.get_render_pipeline(pipelines.downsample),
            pipeline_cache.get_render_pipeline(pipelines.composite),
        ) else {
            return;
        };
        let device = render_context.render_device().clone();
        let uniform = prepared.uniform.as_entire_binding();
        let light = &prepared.light.default_view;
        let scratch = &prepared.scratch.default_view;
        let edges = &prepared.edges.default_view;

        let gather_group = device.create_bind_group(
            "ssil_gather",
            &pipeline_cache.get_bind_group_layout(&pipelines.gather_layout),
            &BindGroupEntries::with_indices((
                (0, depth),
                (1, gbuffer),
                (4, uniform.clone()),
                (7, &history.sampled),
                (8, &pipelines.mip),
            )),
        );
        draw(render_context, "ssil_gather", &[light, edges], gather, &gather_group);

        let blur_layout = pipeline_cache.get_bind_group_layout(&pipelines.blur_layout);
        let blur_group = |input: &TextureView| {
            device.create_bind_group(
                "ssil_blur",
                &blur_layout,
                &BindGroupEntries::with_indices((
                    (4, uniform.clone()),
                    (5, input),
                    (9, edges),
                    (10, &pipelines.mirror),
                )),
            )
        };
        // Ping-pong, ending back in `light` for an even pass count.
        for pass in 0..BLUR_PASSES {
            let (input, output) = if pass % 2 == 0 { (light, scratch) } else { (scratch, light) };
            let pipeline = if pass + 2 < BLUR_PASSES { blur_wide } else { blur_smart };
            draw(render_context, "ssil_blur", &[output], pipeline, &blur_group(input));
        }
        let blurred = if BLUR_PASSES.is_multiple_of(2) { light } else { scratch };

        let frame = view_target.post_process_write();
        let composite_group = device.create_bind_group(
            "ssil_composite",
            &pipeline_cache.get_bind_group_layout(&pipelines.composite_layout),
            &BindGroupEntries::with_indices((
                (0, depth),
                (1, gbuffer),
                (2, frame.source),
                (3, &pipelines.linear),
                (4, uniform.clone()),
                (6, blurred),
                (7, &history.sampled),
                (8, &pipelines.mip),
                (11, prepared.environment.as_ref().unwrap_or(&pipelines.no_environment)),
            )),
        );
        draw(render_context, "ssil_composite", &[frame.destination], composite, &composite_group);

        // Keep the finished frame for the next one's taps: a copy into mip 0,
        // then each mip from the one above it.
        let downsample_layout = pipeline_cache.get_bind_group_layout(&pipelines.downsample_layout);
        let downsample_group = |input: &TextureView| {
            device.create_bind_group(
                "ssil_downsample",
                &downsample_layout,
                &BindGroupEntries::with_indices(((3, &pipelines.linear), (5, input))),
            )
        };
        // A debug view would otherwise be kept and fed back into the taps, so
        // while one is on the history keeps the lit frame from before SSIL.
        let mut source = if prepared.debug_view == 0 {
            frame.destination
        } else {
            frame.source
        };
        for mip in &history.mips {
            draw(render_context, "ssil_history", &[mip], downsample, &downsample_group(source));
            source = mip;
        }
    }
}

/// One fullscreen triangle into `targets`.
fn draw(
    render_context: &mut RenderContext,
    label: &'static str,
    targets: &[&TextureView],
    pipeline: &RenderPipeline,
    bind_group: &BindGroup,
) {
    let attachments: Vec<_> = targets
        .iter()
        .map(|view| {
            Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })
        })
        .collect();
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some(label),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}

#[cfg(test)]
mod tests {
    /// The shader compiles as wgpu will compile it, with its one import
    /// replaced by a stand-in. Nothing else checks it before the editor loads
    /// it, and a shader that fails there only logs while the pass draws
    /// nothing.
    #[test]
    fn the_ssil_shader_compiles() {
        const STUB: &str = "
            struct FullscreenVertexOutput {
                @builtin(position) position: vec4<f32>,
                @location(0) uv: vec2<f32>,
            }
        ";
        let source: String = std::iter::once(STUB.to_string())
            .chain(
                include_str!("ssil.wesl")
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("import "))
                    .map(str::to_string),
            )
            .collect::<Vec<_>>()
            .join("\n");
        renzora_shader_api::wgsl::check(&source).unwrap_or_else(|err| panic!("ssil.wesl: {err}"));
    }
}
