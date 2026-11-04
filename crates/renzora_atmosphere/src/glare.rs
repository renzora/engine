//! Sun glare: the bright haze around the sun that bloom cannot make.
//!
//! Bevy's bloom caps what any one pixel adds to the glow (its first pass is a
//! firefly filter), so the sun disc, a hundred thousand times brighter than
//! the sky, blooms no more than a bright cloud. The sun came out as a crisp
//! white dot however bloom was set. This draws the glare as its own pass, in
//! the HDR frame before tonemapping, shaped by the scene's `Sun`
//! (`glare_intensity`, `glare_size`).
//!
//! Split the usual way: [`sync_sun_glare`] decides in the main world which
//! cameras get it and with what, as a [`SunGlare`] the render world copies;
//! [`SunGlarePass`] draws it. A camera has glare while it has a sky
//! (`AtmosphereSettings`, which `sync_atmosphere` takes away with the sky), so
//! turning the sky off takes the glare with it.
//!
//! Whether the sun can be seen is measured on screen, in the shader: see
//! `glare.wesl`. That covers clouds, terrain and buildings alike without this
//! pass knowing about any of them.

use bevy::core_pipeline::FullscreenShader;
use bevy::light::SunDisk;
use bevy::pbr::AtmosphereSettings;
use bevy::prelude::*;
use bevy::render::{
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    render_resource::{
        binding_types::{sampler, texture_2d, uniform_buffer_sized},
        encase::UniformBuffer,
        BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendComponent,
        BlendFactor, BlendOperation, BlendState, Buffer, BufferInitDescriptor, BufferUsages,
        CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d, FragmentState, LoadOp,
        Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
        RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
        ShaderType, StoreOp, TextureDescriptor, TextureDimension, TextureFormat,
        TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor,
    },
    renderer::{RenderContext, RenderDevice},
    view::{ExtractedView, ViewTarget},
    Render, RenderApp, RenderStartup, RenderSystems,
};
use renzora::postprocess::{PostProcessCorePlugin, RenderCompositionAppExt, RenderPass};
use renzora::{RenderPhase, Sun};

/// The HDR frame format; the glare is only drawn into an HDR camera.
const HDR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// How much of each frame's measurement goes into the running average. At 60
/// FPS the glare settles in about a tenth of a second: slow enough to swallow
/// the per-frame noise Solari and the camera jitter put on a disc a few pixels
/// wide, fast enough that a cloud crossing the sun still reads as it happens.
const MEASURE_BLEND: f32 = 0.15;

/// What one camera draws, resolved from the scene's sun.
#[derive(Component, Clone, Copy, Debug, PartialEq, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct SunGlare {
    /// Unit vector from the scene toward the sun, in world space.
    pub toward_sun: Vec3,
    /// The sun's colour, linear, brightest channel at 1.
    pub tint: Vec3,
    /// `Sun::glare_intensity`, faded as the sun sets.
    pub strength: f32,
    /// `Sun::glare_size`, in radians.
    pub spread: f32,
    /// Half the disc's angular size, in radians.
    pub disc_radius: f32,
}

pub(crate) fn build(app: &mut App) {
    bevy::asset::embedded_asset!(app, "glare.wesl");
    app.add_plugins(ExtractComponentPlugin::<SunGlare>::default())
        .add_systems(Update, sync_sun_glare);

    if !app.is_plugin_added::<PostProcessCorePlugin>() {
        app.add_plugins(PostProcessCorePlugin);
    }
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .add_systems(RenderStartup, init_pipeline)
        .add_systems(Render, prepare.in_set(RenderSystems::PrepareResources))
        // After TAA and DLSS, before tonemapping (`HdrPost`), so the glare is
        // in the same light as the rest of the frame when auto exposure meters
        // it, and no upscaler treats it as part of its noisy input.
        .add_render_pass("sun_glare", RenderPhase::HdrPost, 0.0, SunGlarePass);
}

/// The glare a camera should draw for `sun`, or `None` for none at all.
fn glare_for(sun: &Sun, light: &DirectionalLight, disc: &SunDisk, transform: &GlobalTransform) -> Option<SunGlare> {
    // `sync_sun` fades the light's illuminance out below the horizon while the
    // authored value stays put; the ratio is that fade.
    let horizon = if sun.illuminance > 0.0 {
        (light.illuminance / sun.illuminance).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // No glare is still a pass while the disc is in the sky: the pass also
    // brings the disc's overflowing pixels back into range before bloom (see
    // `sanitize` in `glare.wesl`), and that is needed with or without a glare.
    if horizon <= 0.0 || disc.intensity <= 0.0 || disc.angular_size <= 0.0 {
        return None;
    }
    let strength = sun.glare_intensity.max(0.0) * horizon;
    let c = light.color.to_linear();
    let rgb = Vec3::new(c.red, c.green, c.blue).max(Vec3::ZERO);
    let peak = rgb.max_element();
    let tint = if peak > 0.0 { rgb / peak } else { Vec3::ONE };
    Some(SunGlare {
        // The light travels along its forward axis, so the sun is behind it.
        toward_sun: transform.back().as_vec3(),
        tint,
        strength,
        spread: sun.glare_size.max(0.1).to_radians(),
        disc_radius: disc.angular_size * 0.5,
    })
}

/// Give every camera with a sky the glare of the scene's sun, and take it off
/// cameras that lost the sky or the sun.
///
/// The brightest `Sun` wins, as the cloud deck picks its light. A camera's
/// glare is only rewritten when it changes, so a still scene does not
/// re-extract it every frame.
fn sync_sun_glare(
    mut commands: Commands,
    suns: Query<(&Sun, &DirectionalLight, &SunDisk, &GlobalTransform)>,
    cameras: Query<(Entity, Option<&SunGlare>), (With<Camera3d>, With<AtmosphereSettings>)>,
    stale: Query<Entity, (With<SunGlare>, Without<AtmosphereSettings>)>,
) {
    let glare = suns
        .iter()
        .max_by(|a, b| a.1.illuminance.total_cmp(&b.1.illuminance))
        .and_then(|(sun, light, disc, transform)| glare_for(sun, light, disc, transform));

    for (camera, current) in &cameras {
        match glare {
            Some(g) if current != Some(&g) => {
                commands.entity(camera).try_insert(g);
            }
            None if current.is_some() => {
                commands.entity(camera).try_remove::<SunGlare>();
            }
            _ => {}
        }
    }
    for camera in &stale {
        commands.entity(camera).try_remove::<SunGlare>();
    }
}

#[derive(ShaderType)]
struct GlareUniform {
    clip_from_view: Mat4,
    view_from_clip: Mat4,
    sun: Vec4,
    tint: Vec4,
    params: Vec4,
}

#[derive(Resource)]
struct GlarePipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline: CachedRenderPipelineId,
    /// Bindings 0 and 2 only: the measure pass writes the history texture, so
    /// it cannot also have it bound for reading.
    measure_layout: BindGroupLayoutDescriptor,
    measure: CachedRenderPipelineId,
}

fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let texture = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new(
        "sun_glare_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture(),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer_sized(false, None),
                texture(),
            ),
        ),
    );
    let measure_layout = BindGroupLayoutDescriptor::new(
        "sun_glare_measure_layout",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::FRAGMENT,
            ((0, texture()), (2, uniform_buffer_sized(false, None))),
        ),
    );
    let shader: Handle<Shader> = asset_server.load("embedded://renzora_atmosphere/glare.wesl");
    let pipeline = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("sun_glare".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: shader.clone(),
            entry_point: Some("fragment".into()),
            targets: vec![Some(ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    // `new * c + old * (1 - c)`, with `c` set per draw as the blend constant:
    // the running average is the blend itself, so no second texture has to
    // be ping-ponged to keep last frame's value.
    let average = BlendComponent {
        src_factor: BlendFactor::Constant,
        dst_factor: BlendFactor::OneMinusConstant,
        operation: BlendOperation::Add,
    };
    let measure = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("sun_glare_measure".into()),
        layout: vec![measure_layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader,
            entry_point: Some("measure".into()),
            targets: vec![Some(ColorTargetState {
                format: HDR_FORMAT,
                blend: Some(BlendState {
                    color: average,
                    alpha: average,
                }),
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(GlarePipeline {
        layout,
        sampler: render_device.create_sampler(&SamplerDescriptor::default()),
        pipeline,
        measure_layout,
        measure,
    });
}

#[derive(Component)]
struct PreparedGlare(Buffer);

/// A view's running measurement of the sun: one texel, `x` = how much of the
/// disc is visible, `y` = the sky's brightness around it. Kept on the view
/// across frames, because the average is only useful if it outlives the frame.
#[derive(Component)]
struct GlareHistory(TextureView);

fn prepare(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedView, &SunGlare, Has<GlareHistory>)>,
) {
    for (entity, view, glare, has_history) in &views {
        if !has_history {
            // wgpu zero-fills a new texture, so the glare fades in from
            // nothing over its first few frames rather than reading garbage.
            let texture = render_device.create_texture(&TextureDescriptor {
                label: Some("sun_glare_history"),
                size: Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: HDR_FORMAT,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            commands
                .entity(entity)
                .insert(GlareHistory(texture.create_view(&TextureViewDescriptor::default())));
        }
        let sun_view = view.world_from_view.rotation().inverse() * glare.toward_sun;
        let uniform = GlareUniform {
            clip_from_view: view.clip_from_view,
            view_from_clip: view.clip_from_view.inverse(),
            sun: sun_view.normalize_or_zero().extend(glare.disc_radius),
            tint: glare.tint.extend(glare.strength),
            params: Vec4::new(glare.spread, 0.0, 0.0, 0.0),
        };
        let mut bytes = UniformBuffer::new(Vec::<u8>::new());
        if bytes.write(&uniform).is_err() {
            continue;
        }
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("sun_glare_uniform"),
            contents: bytes.as_ref(),
            usage: BufferUsages::UNIFORM,
        });
        commands.entity(entity).insert(PreparedGlare(buffer));
    }
}

struct SunGlarePass;

impl RenderPass for SunGlarePass {
    fn run(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        view_target: &ViewTarget,
        view_entity: Entity,
    ) {
        // Gate on the extracted glare, not only on what `prepare` left: the
        // retained render world keeps `PreparedGlare` on a view after its
        // glare is taken away, and drawing it would leave a ghost sun.
        if world.get::<SunGlare>(view_entity).is_none() {
            return;
        }
        let (Some(prepared), Some(history)) = (
            world.get::<PreparedGlare>(view_entity),
            world.get::<GlareHistory>(view_entity),
        ) else {
            return;
        };
        if view_target.main_texture_format() != HDR_FORMAT {
            return;
        }
        let pipelines = world.resource::<GlarePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        // Look the pipelines up before taking the ping-pong pair: taking it
        // flips which texture is current, and skipping the draw after that
        // would show a frame nothing was written to.
        let (Some(measure), Some(pipeline)) = (
            pipeline_cache.get_render_pipeline(pipelines.measure),
            pipeline_cache.get_render_pipeline(pipelines.pipeline),
        ) else {
            return;
        };
        let device = render_context.render_device().clone();

        // Fold this frame's look at the sun into the running average.
        let measure_group = device.create_bind_group(
            "sun_glare_measure",
            &pipeline_cache.get_bind_group_layout(&pipelines.measure_layout),
            &BindGroupEntries::with_indices((
                (0, view_target.main_texture_view()),
                (2, prepared.0.as_entire_binding()),
            )),
        );
        {
            let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("sun_glare_measure"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &history.0,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_render_pipeline(measure);
            pass.set_bind_group(0, &measure_group, &[]);
            pass.set_blend_constant(LinearRgba::new(
                MEASURE_BLEND,
                MEASURE_BLEND,
                MEASURE_BLEND,
                MEASURE_BLEND,
            ));
            pass.draw(0..3, 0..1);
        }

        let frame = view_target.post_process_write();
        let bind_group = device.create_bind_group(
            "sun_glare",
            &pipeline_cache.get_bind_group_layout(&pipelines.layout),
            &BindGroupEntries::sequential((
                frame.source,
                &pipelines.sampler,
                prepared.0.as_entire_binding(),
                &history.0,
            )),
        );
        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("sun_glare"),
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
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sun_at(elevation: f32) -> (Sun, DirectionalLight, SunDisk, GlobalTransform) {
        let sun = Sun {
            elevation,
            ..default()
        };
        let light = DirectionalLight {
            illuminance: sun.illuminance,
            ..default()
        };
        let transform = GlobalTransform::from(Transform::from_rotation(Quat::from_rotation_arc(
            Vec3::NEG_Z,
            sun.direction(),
        )));
        let disc = SunDisk {
            angular_size: sun.angular_diameter.to_radians(),
            intensity: 1.0,
        };
        (sun, light, disc, transform)
    }

    /// The glare points at the sun, not along the light: the light travels
    /// away from it.
    #[test]
    fn the_glare_points_at_the_sun() {
        let (sun, light, disc, t) = sun_at(30.0);
        let g = glare_for(&sun, &light, &disc, &t).unwrap();
        assert!((g.toward_sun + sun.direction()).length() < 1e-4, "{:?}", g.toward_sun);
        assert!(g.toward_sun.y > 0.0);
    }

    /// Below the horizon `sync_sun` has faded the light to nothing, and the
    /// glare goes with it.
    #[test]
    fn a_set_sun_has_no_glare() {
        let (sun, mut light, disc, t) = sun_at(-5.0);
        light.illuminance = 0.0;
        assert!(glare_for(&sun, &light, &disc, &t).is_none());
    }

    /// Zero glare keeps the pass, at zero strength, because the disc still has
    /// to be brought into range for bloom. Switching the disc off drops it: a
    /// glare with no sun in it would be light coming from nowhere, and there
    /// is no disc left to overflow.
    #[test]
    fn zero_glare_keeps_the_pass_and_no_disc_drops_it() {
        let (mut sun, light, mut disc, t) = sun_at(30.0);
        sun.glare_intensity = 0.0;
        let g = glare_for(&sun, &light, &disc, &t).expect("the disc still needs taming");
        assert_eq!(g.strength, 0.0);
        disc.intensity = 0.0;
        assert!(glare_for(&sun, &light, &disc, &t).is_none());
    }

    /// The shader compiles as wgpu will compile it, with the one import
    /// replaced by the struct it brings in. Nothing else checks it before the
    /// editor loads it, and a shader that fails there only logs.
    #[test]
    fn the_glare_shader_compiles() {
        const STUB: &str = "
            struct FullscreenVertexOutput {
                @builtin(position) position: vec4<f32>,
                @location(0) uv: vec2<f32>,
            }
        ";
        let source: String = std::iter::once(STUB.to_string())
            .chain(
                include_str!("glare.wesl")
                    .lines()
                    .filter(|line| !line.trim_start().starts_with("import "))
                    .map(str::to_string),
            )
            .collect::<Vec<_>>()
            .join("\n");
        renzora::wgsl::check(&source).unwrap_or_else(|err| panic!("glare.wesl: {err}"));
    }

    /// The tint is the light's colour at full brightness, so the glare's
    /// strength is set by the sky around the sun and the intensity alone.
    #[test]
    fn the_tint_is_normalised() {
        let (sun, mut light, disc, t) = sun_at(30.0);
        light.color = Color::linear_rgb(2.0, 1.0, 0.5);
        let g = glare_for(&sun, &light, &disc, &t).unwrap();
        assert!((g.tint - Vec3::new(1.0, 0.5, 0.25)).length() < 1e-5);
    }
}
