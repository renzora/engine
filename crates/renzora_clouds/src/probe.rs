//! Clouds in the environment cubemap, so reflections and sky light see them.
//!
//! Solari lights a scene by tracing rays against its meshes, and a ray that
//! leaves the scene reads the sky from the camera's environment cubemap. The
//! cloud deck is neither a mesh nor in that cubemap: the atmosphere bakes a
//! clear sky into it. So under full cloud a Solari scene still reflected a
//! clear blue sky and was lit as brightly as on a cloudless day. Without
//! Solari the ordinary image-based lighting read the same cubemap, with the
//! same result.
//!
//! This keeps a second cubemap per camera holding the atmosphere's sky with
//! the deck marched over it, and points the camera's environment light at it
//! instead. It is a second texture rather than clouds drawn into the
//! atmosphere's own, because the atmosphere rewrites that one every frame from
//! a system Renzora cannot order against: drawn into, the clouds raced the
//! bake and lost. With a texture of its own the order only decides whether the
//! clouds are a frame behind, which nothing can see.
//!
//! The march is the dome's: `probe.wesl` is appended to `clouds.wesl` and
//! calls its `march_clouds`. Under Solari one face is refreshed a frame, so a
//! full cube takes six; without Solari the whole cube is rewritten every frame
//! (see [`CloudSkyProbe::full_refresh`]). The first time a cube is made all six
//! faces are written at once, so it never shows black.

use bevy::asset::RenderAssetUsages;
use bevy::light::{AtmosphereEnvironmentMapLight, EnvironmentMapLight, GeneratedEnvironmentMapLight};
use bevy::prelude::*;
use bevy::render::{
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    render_asset::RenderAssets,
    render_resource::{
        binding_types::{sampler, texture_2d, texture_2d_array, texture_3d, texture_storage_2d_array, uniform_buffer},
        encase::UniformBuffer,
        BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        BufferInitDescriptor, BufferUsages, CachedComputePipelineId, ComputePassDescriptor,
        ComputePipelineDescriptor, Extent3d, PipelineCache, SamplerBindingType, ShaderStages,
        ShaderType, StorageTextureAccess, TextureDimension, TextureFormat, TextureSampleType,
        TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
    },
    renderer::{RenderContext, RenderDevice},
    texture::GpuImage,
    view::ViewTarget,
    Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
};
use renzora::postprocess::{PostProcessCorePlugin, RenderCompositionAppExt, RenderPass};
use renzora::RenderPhase;

use crate::material::{CloudMaterial, CloudsUniform};

/// Workgroup edge in `probe.wesl`.
const WORKGROUP: u32 = 8;

/// A camera's cloudy copy of its sky cubemap.
#[derive(Component, Clone, Debug, PartialEq, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct CloudSkyProbe {
    /// The atmosphere's cubemap: the clear sky.
    source: Handle<Image>,
    /// Ours: that sky with the deck over it. What the camera's environment
    /// light points at while this exists.
    target: Handle<Image>,
    /// The eye the deck is marched from, in world metres.
    eye: Vec3,
    /// Write the whole cube every frame rather than one face.
    ///
    /// Set while Solari is not lighting the camera. Then the cube is not read
    /// live: the environment-map filter turns it into the sky light for a few
    /// frames after the sky changes and freezes the result. Refreshed a face a
    /// frame, the filter froze a cube still half from the old sky, so pulling
    /// the sun down to the horizon left the scene lit as if it were still up.
    /// Solari reads the cube directly every frame, where a face a frame is
    /// enough.
    full_refresh: bool,
}

/// The deck's uniform and noise, as the dome material has them this frame.
/// `None` while there is no deck.
#[derive(Resource, Clone, Default)]
struct CloudProbeInputs(Option<(CloudsUniform, Handle<Image>, Handle<Image>)>);

pub(crate) fn build(app: &mut App) {
    app.init_resource::<CloudProbeInputs>()
        .add_plugins(ExtractComponentPlugin::<CloudSkyProbe>::default())
        .add_systems(Update, sync_cloud_sky_probe.after(crate::sync_clouds));

    if !app.is_plugin_added::<PostProcessCorePlugin>() {
        app.add_plugins(PostProcessCorePlugin);
    }
}

/// Deferred to `finish`, because it adds to `Assets<Shader>`.
pub(crate) fn finish(app: &mut App) {
    let shader = app
        .world_mut()
        .resource_mut::<Assets<Shader>>()
        .add(renzora::shader_source::shader(probe_source(), "renzora_clouds/probe.wgsl"));
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .insert_resource(ProbeShader(shader))
        .init_resource::<CloudProbeInputs>()
        .add_systems(ExtractSchedule, extract_inputs)
        .add_systems(RenderStartup, init_pipeline)
        .add_systems(Render, prepare.in_set(RenderSystems::PrepareBindGroups))
        // `Gi` runs after the main pass, so Solari and the PBR lighting read
        // the cube this pass wrote last frame. One frame behind, by design: see
        // the module docs.
        .add_render_pass("cloud_sky_probe", RenderPhase::Gi, -100.0, CloudSkyProbePass);
}

/// The compute shader: the cloud shader with its material bindings moved to
/// group 0, stubs for what only its fragment entry reads, and the probe entry.
///
/// Built from the same text the dome compiles, so the probe cannot drift from
/// the sky. The stubs are never bound: the probe entry does not reach them,
/// and a compute pipeline only binds what its entry point uses.
pub(crate) fn probe_source() -> String {
    const PRELUDE: &str = "
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
}
struct ProbeUnusedView { world_position: vec3<f32>, exposure: f32 }
@group(2) @binding(0) var<uniform> view: ProbeUnusedView;
struct ProbeUnusedGlobals { frame_count: u32 }
@group(2) @binding(1) var<uniform> globals: ProbeUnusedGlobals;
";
    let clouds = include_str!("clouds.wesl")
        .lines()
        .filter(|line| !line.trim_start().starts_with("import "))
        .map(|line| line.replace("@group(3)", "@group(0)"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{PRELUDE}\n{clouds}\n{}", include_str!("probe.wesl"))
}

/// Keep each sky camera's probe in step with the deck, and its environment
/// light pointed at it; hand the atmosphere's cube back when the deck goes.
///
/// The atmosphere rebuilds its cube when its settings change (a new size, the
/// filtered/unfiltered switch Solari makes), which puts its own handle back on
/// the environment light. That shows up here as a handle that is neither ours
/// nor the source we knew, and is taken as the new source.
fn sync_cloud_sky_probe(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    state: Res<crate::CloudsState>,
    materials: Res<Assets<CloudMaterial>>,
    mut inputs: ResMut<CloudProbeInputs>,
    mut cameras: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&mut CloudSkyProbe>,
            Option<&mut EnvironmentMapLight>,
            Option<&mut GeneratedEnvironmentMapLight>,
            Has<renzora::SolariLit>,
        ),
        With<AtmosphereEnvironmentMapLight>,
    >,
) {
    let deck = state
        .material_handle
        .as_ref()
        .and_then(|h| materials.get(h))
        .map(|m| (m.uniform, m.base_noise.clone(), m.detail_noise.clone()));
    let active = deck.is_some();
    inputs.0 = deck;

    for (entity, transform, probe, env, generated, solari) in &mut cameras {
        let full_refresh = !solari;
        // With filtering on, `GeneratedEnvironmentMapLight` names the cube and
        // `EnvironmentMapLight` holds its filtered results, which are not ours
        // to touch; without, `EnvironmentMapLight` names the cube directly.
        let current = match (&generated, &env) {
            (Some(g), _) => g.environment_map.clone(),
            (None, Some(e)) => e.specular_map.clone(),
            (None, None) => continue,
        };

        if !active {
            if let Some(probe) = probe {
                point_at(generated, env, &probe.target, &probe.source);
                commands.entity(entity).try_remove::<CloudSkyProbe>();
            }
            continue;
        }

        let source = match &probe {
            Some(p) if current == p.target => p.source.clone(),
            _ => current.clone(),
        };
        let eye = transform.translation();

        match probe {
            Some(mut p) if p.source == source => {
                if p.eye != eye {
                    p.eye = eye;
                }
                if p.full_refresh != full_refresh {
                    p.full_refresh = full_refresh;
                }
                let target = p.target.clone();
                point_at(generated, env, &source, &target);
            }
            _ => {
                let Some(target) = make_target(&mut images, &source) else {
                    continue;
                };
                point_at(generated, env, &source, &target);
                commands.entity(entity).try_insert(CloudSkyProbe {
                    source,
                    target,
                    eye,
                    full_refresh,
                });
            }
        }
    }
}

/// Repoint whichever component names the cube from `from` to `to`.
fn point_at(
    generated: Option<Mut<GeneratedEnvironmentMapLight>>,
    env: Option<Mut<EnvironmentMapLight>>,
    from: &Handle<Image>,
    to: &Handle<Image>,
) {
    if let Some(mut g) = generated {
        if g.environment_map == *from {
            g.environment_map = to.clone();
        }
        return;
    }
    if let Some(mut e) = env {
        if e.specular_map == *from {
            e.specular_map = to.clone();
        }
        if e.diffuse_map == *from {
            e.diffuse_map = to.clone();
        }
    }
}

/// A cube the size and format of the atmosphere's, writable by the probe.
fn make_target(images: &mut Assets<Image>, source: &Handle<Image>) -> Option<Handle<Image>> {
    let size = images.get(source)?.texture_descriptor.size;
    let mut image = Image::new_fill(
        Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &[0; 8],
        TextureFormat::Rgba16Float,
        // Kept in the main world too: the environment-map filter reads the
        // cube's size from it there.
        RenderAssetUsages::all(),
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC;
    Some(images.add(image))
}

fn extract_inputs(mut commands: Commands, inputs: Extract<Res<CloudProbeInputs>>) {
    commands.insert_resource(inputs.clone());
}

#[derive(Resource)]
struct ProbeShader(Handle<Shader>);

#[derive(ShaderType)]
struct ProbeUniform {
    eye: Vec4,
    first_face: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[derive(Resource)]
struct ProbePipeline {
    clouds_layout: BindGroupLayoutDescriptor,
    probe_layout: BindGroupLayoutDescriptor,
    pipeline: CachedComputePipelineId,
}

fn init_pipeline(mut commands: Commands, shader: Res<ProbeShader>, pipeline_cache: Res<PipelineCache>) {
    let filterable = || TextureSampleType::Float { filterable: true };
    // Bindings 0 to 4 as `CloudMaterial`'s `AsBindGroup` lays them out.
    let clouds_layout = BindGroupLayoutDescriptor::new(
        "cloud_probe_clouds_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<CloudsUniform>(false),
                texture_2d(filterable()),
                sampler(SamplerBindingType::Filtering),
                texture_3d(filterable()),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    );
    let probe_layout = BindGroupLayoutDescriptor::new(
        "cloud_probe_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<ProbeUniform>(false),
                texture_2d_array(filterable()),
                texture_storage_2d_array(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly),
            ),
        ),
    );
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("cloud_sky_probe".into()),
        layout: vec![clouds_layout.clone(), probe_layout.clone()],
        shader: shader.0.clone(),
        entry_point: Some("probe_main".into()),
        ..default()
    });
    commands.insert_resource(ProbePipeline {
        clouds_layout,
        probe_layout,
        pipeline,
    });
}

/// A probe ready to dispatch this frame.
#[derive(Component)]
struct PreparedProbe {
    clouds: BindGroup,
    probe: BindGroup,
    faces: u32,
    size: UVec2,
}

/// Which face a probe writes next, and which cube it was writing, so a new
/// cube is filled whole before it is refreshed a face at a time.
#[derive(Component)]
struct ProbeCursor {
    target: AssetId<Image>,
    next_face: u32,
}

fn prepare(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipelines: Option<Res<ProbePipeline>>,
    inputs: Res<CloudProbeInputs>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    mut probes: Query<(Entity, &CloudSkyProbe, Option<&mut ProbeCursor>)>,
) {
    let (Some(pipelines), Some((uniform, base, detail))) = (pipelines, inputs.0.as_ref()) else {
        return;
    };
    let (Some(base), Some(detail)) = (gpu_images.get(base), gpu_images.get(detail)) else {
        return;
    };

    let mut clouds_bytes = UniformBuffer::new(Vec::<u8>::new());
    if clouds_bytes.write(uniform).is_err() {
        return;
    }
    let clouds_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("cloud_probe_clouds"),
        contents: clouds_bytes.as_ref(),
        usage: BufferUsages::UNIFORM,
    });
    let clouds = render_device.create_bind_group(
        "cloud_probe_clouds",
        &pipeline_cache.get_bind_group_layout(&pipelines.clouds_layout),
        &BindGroupEntries::sequential((
            clouds_buffer.as_entire_binding(),
            &base.texture_view,
            &base.sampler,
            &detail.texture_view,
            &detail.sampler,
        )),
    );

    for (entity, probe, cursor) in &mut probes {
        let (Some(source), Some(target)) = (gpu_images.get(&probe.source), gpu_images.get(&probe.target)) else {
            continue;
        };
        let layers = |image: &GpuImage| -> TextureView {
            image.texture.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2Array),
                ..default()
            })
        };

        // A cube seen for the first time is written whole, so the camera
        // never lights the scene from a black sky; after that one face a frame.
        let (first_face, faces) = match cursor {
            Some(_) if probe.full_refresh => (0, 6),
            Some(mut c) if c.target == probe.target.id() => {
                let face = c.next_face;
                c.next_face = (face + 1) % 6;
                (face, 1)
            }
            _ => {
                commands.entity(entity).insert(ProbeCursor {
                    target: probe.target.id(),
                    next_face: 0,
                });
                (0, 6)
            }
        };

        let mut probe_bytes = UniformBuffer::new(Vec::<u8>::new());
        let probe_uniform = ProbeUniform {
            eye: probe.eye.extend(0.0),
            first_face,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        if probe_bytes.write(&probe_uniform).is_err() {
            continue;
        }
        let probe_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("cloud_probe"),
            contents: probe_bytes.as_ref(),
            usage: BufferUsages::UNIFORM,
        });
        let probe_group = render_device.create_bind_group(
            "cloud_probe",
            &pipeline_cache.get_bind_group_layout(&pipelines.probe_layout),
            &BindGroupEntries::sequential((
                probe_buffer.as_entire_binding(),
                &layers(source),
                &layers(target),
            )),
        );
        commands.entity(entity).insert(PreparedProbe {
            clouds: clouds.clone(),
            probe: probe_group,
            faces,
            size: target.size_2d(),
        });
    }
}

struct CloudSkyProbePass;

impl RenderPass for CloudSkyProbePass {
    fn run(
        &self,
        world: &World,
        render_context: &mut RenderContext,
        _view_target: &ViewTarget,
        view_entity: Entity,
    ) {
        // Gate on the extracted probe as well: the retained render world keeps
        // `PreparedProbe` on a view after its probe is taken away.
        if world.get::<CloudSkyProbe>(view_entity).is_none() {
            return;
        }
        let Some(prepared) = world.get::<PreparedProbe>(view_entity) else {
            return;
        };
        let pipelines = world.resource::<ProbePipeline>();
        let Some(pipeline) = world
            .resource::<PipelineCache>()
            .get_compute_pipeline(pipelines.pipeline)
        else {
            return;
        };
        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("cloud_sky_probe"),
                timestamp_writes: None,
            });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &prepared.clouds, &[]);
        pass.set_bind_group(1, &prepared.probe, &[]);
        pass.dispatch_workgroups(
            prepared.size.x.div_ceil(WORKGROUP),
            prepared.size.y.div_ceil(WORKGROUP),
            prepared.faces,
        );
    }
}

#[cfg(test)]
mod tests {
    /// The generated compute shader compiles as wgpu will compile it. It is
    /// text spliced from two files, and nothing else checks it before the
    /// editor builds the pipeline, where a failure only logs.
    #[test]
    fn the_probe_shader_compiles() {
        renzora::wgsl::check(&super::probe_source())
            .unwrap_or_else(|err| panic!("cloud probe: {err}"));
    }
}
