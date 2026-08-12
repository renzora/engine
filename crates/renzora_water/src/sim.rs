//! GPU wave simulation: spectrum → inverse FFT → displacement/normal maps.
//!
//! Six compute passes run once per simulation step, for every cascade at once:
//!
//! 1. `water_butterfly` — Stockham butterfly factors. Once per resolution.
//! 2. `water_spectrum`  — the JONSWAP/TMA sea state. Only when parameters change.
//! 3. `water_modulate`  — propagate to the current time, derive gradients.
//! 4. `water_fft`       — inverse FFT along rows.
//! 5. `water_transpose` — transpose, so the next pass walks rows again.
//! 6. `water_fft`       — inverse FFT along the (now transposed) rows.
//! 7. `water_unpack`    — write the displacement/normal maps and grow foam.
//!
//! There is deliberately no second transpose: the result comes out rotated by
//! 90°, which for a wave field with no preferred screen orientation is
//! invisible, and it saves a full pass.
//!
//! Everything runs in one compute pass. WebGPU guarantees dispatches within a
//! pass observe each other's writes in order, so the explicit barriers the
//! Vulkan original needs have no counterpart here.
//!
//! The pass is registered in the `RenderGraph` schedule *before* the camera
//! driver: the simulation is view-independent, so running it per-view would
//! simply do the same work several times.

use std::borrow::Cow;

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, storage_buffer_sized, texture_storage_2d_array, uniform_buffer,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderGraph, RenderGraphSystems};
use bevy::render::texture::GpuImage;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::shader::ShaderDefVal;

use crate::component::MAX_CASCADES;

/// Number of complex fields the FFT carries per cascade: packed displacement
/// (x+iy), (z + i·∂y/∂x), and two more gradient pairs.
const NUM_SPECTRA: u64 = 4;

/// Workgroup size of the tiled kernels (spectrum, modulate, transpose, unpack).
const TILE: u32 = 16;

/// Per-cascade parameters as the compute shaders see them. The field order and
/// padding must match the `Cascade` struct duplicated at the top of each
/// `shaders/water_*.wgsl`.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct CascadeGpu {
    pub tile_length: Vec2,
    pub alpha: f32,
    pub peak_frequency: f32,
    pub wind_speed: f32,
    pub angle: f32,
    pub swell: f32,
    pub detail: f32,
    pub spread: f32,
    pub time: f32,
    pub whitecap: f32,
    pub foam_grow_rate: f32,
    pub foam_decay_rate: f32,
    pub pad: f32,
    pub seed: IVec2,
}

/// The one uniform every compute pass binds.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct WaterSimUniform {
    pub map_size: u32,
    pub num_cascades: u32,
    pub depth: f32,
    pub pad: f32,
    pub cascades: [CascadeGpu; MAX_CASCADES],
}

impl Default for WaterSimUniform {
    fn default() -> Self {
        Self {
            map_size: 256,
            num_cascades: 0,
            depth: 20.0,
            pad: 0.0,
            cascades: [CascadeGpu::default(); MAX_CASCADES],
        }
    }
}

/// The cascade maps the water material samples. Created in the main world so
/// the material can hold ordinary `Handle<Image>`s; written by compute.
#[derive(Resource, Clone, ExtractResource)]
pub struct WaterSimTextures {
    /// `xyz` = displacement, one array layer per cascade.
    pub displacement: Handle<Image>,
    /// `xy` = height gradient, `z` = ∂x/∂x (unused by shading, kept for parity
    /// with the reference), `w` = foam.
    pub normal: Handle<Image>,
    pub map_size: u32,
    pub num_cascades: u32,
}

/// What the main world tells the simulation to do this frame.
#[derive(Resource, Clone, ExtractResource)]
pub struct WaterSimParams {
    pub uniform: WaterSimUniform,
    /// Rebuild the time-independent spectrum (a parameter changed).
    pub regenerate_spectrum: bool,
    /// Advance the simulation this frame — false when throttled by
    /// `updates_per_second`.
    pub step: bool,
}

impl Default for WaterSimParams {
    fn default() -> Self {
        Self {
            uniform: WaterSimUniform::default(),
            regenerate_spectrum: true,
            step: true,
        }
    }
}

/// Create (or resize) the cascade maps. Returns the pair of handles.
pub fn create_cascade_textures(
    images: &mut Assets<Image>,
    map_size: u32,
    num_cascades: u32,
) -> (Handle<Image>, Handle<Image>) {
    let size = Extent3d {
        width: map_size,
        height: map_size,
        depth_or_array_layers: num_cascades.max(1),
    };

    let mut make = |label: &'static str| {
        let mut image = Image::new_uninit(
            size,
            TextureDimension::D2,
            TextureFormat::Rgba16Float,
            // Compute-only content: there is never a main-world copy to keep.
            RenderAssetUsages::RENDER_WORLD,
        );
        image.texture_descriptor.label = Some(label);
        image.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING;
        // A single-cascade sea would otherwise get a plain 2D default view,
        // which does not match the `2d_array` binding the shaders declare.
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..default()
        });
        // The cascades tile across the world, so UVs run far outside 0..1 —
        // anything but Repeat gives one stretched tile and a clamped smear
        // everywhere else.
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: bevy::image::ImageAddressMode::Repeat,
            address_mode_v: bevy::image::ImageAddressMode::Repeat,
            address_mode_w: bevy::image::ImageAddressMode::Repeat,
            mag_filter: bevy::image::ImageFilterMode::Linear,
            min_filter: bevy::image::ImageFilterMode::Linear,
            mipmap_filter: bevy::image::ImageFilterMode::Linear,
            ..default()
        });
        images.add(image)
    };

    (make("water_displacement_map"), make("water_normal_map"))
}

// ── Render-world pipelines ───────────────────────────────────────────────────

#[derive(Resource)]
struct WaterSimPipelines {
    spectrum_layout: BindGroupLayoutDescriptor,
    modulate_layout: BindGroupLayoutDescriptor,
    butterfly_layout: BindGroupLayoutDescriptor,
    fft_layout: BindGroupLayoutDescriptor,
    transpose_layout: BindGroupLayoutDescriptor,
    unpack_layout: BindGroupLayoutDescriptor,

    spectrum: CachedComputePipelineId,
    modulate: CachedComputePipelineId,
    butterfly: CachedComputePipelineId,
    transpose: CachedComputePipelineId,
    unpack: CachedComputePipelineId,

    /// The FFT kernel is specialised per resolution, so its pipeline is queued
    /// lazily in `prepare_water_sim` rather than here.
    fft_shader: Handle<Shader>,
}

impl FromWorld for WaterSimPipelines {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();

        let uniform = || uniform_buffer::<WaterSimUniform>(false);
        let rw = || storage_buffer_sized(false, None);
        let ro = || storage_buffer_read_only_sized(false, None);
        let storage_texture = || {
            texture_storage_2d_array(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly)
        };

        let spectrum_layout = BindGroupLayoutDescriptor::new(
            "water_spectrum_layout",
            &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, (uniform(), rw())),
        );
        let modulate_layout = BindGroupLayoutDescriptor::new(
            "water_modulate_layout",
            &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, (uniform(), ro(), rw())),
        );
        let butterfly_layout = BindGroupLayoutDescriptor::new(
            "water_butterfly_layout",
            &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, (uniform(), rw())),
        );
        let fft_layout = BindGroupLayoutDescriptor::new(
            "water_fft_layout",
            &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, (uniform(), ro(), rw())),
        );
        let transpose_layout = BindGroupLayoutDescriptor::new(
            "water_transpose_layout",
            &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, (uniform(), rw())),
        );
        let unpack_layout = BindGroupLayoutDescriptor::new(
            "water_unpack_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    uniform(),
                    ro(),
                    rw(),
                    storage_texture(),
                    storage_texture(),
                ),
            ),
        );

        let load = |name: &str| asset_server.load(format!("embedded://renzora_water/shaders/{name}.wgsl"));
        let spectrum_shader = load("water_spectrum");
        let modulate_shader = load("water_modulate");
        let butterfly_shader = load("water_butterfly");
        let transpose_shader = load("water_transpose");
        let unpack_shader = load("water_unpack");
        let fft_shader = load("water_fft");

        let pipeline_cache = world.resource::<PipelineCache>();
        let queue = |label: &'static str,
                     layout: &BindGroupLayoutDescriptor,
                     shader: Handle<Shader>| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(Cow::Borrowed(label)),
                layout: vec![layout.clone()],
                shader,
                shader_defs: vec![],
                entry_point: Some(Cow::Borrowed("main")),
                immediate_size: 0,
                zero_initialize_workgroup_memory: false,
            })
        };

        let spectrum = queue("water_spectrum", &spectrum_layout, spectrum_shader);
        let modulate = queue("water_modulate", &modulate_layout, modulate_shader);
        let butterfly = queue("water_butterfly", &butterfly_layout, butterfly_shader);
        let transpose = queue("water_transpose", &transpose_layout, transpose_shader);
        let unpack = queue("water_unpack", &unpack_layout, unpack_shader);

        Self {
            spectrum_layout,
            modulate_layout,
            butterfly_layout,
            fft_layout,
            transpose_layout,
            unpack_layout,
            spectrum,
            modulate,
            butterfly,
            transpose,
            unpack,
            fft_shader,
        }
    }
}

/// GPU buffers, sized for the current resolution and cascade count.
#[derive(Resource)]
struct WaterSimResources {
    map_size: u32,
    num_cascades: u32,
    uniform: UniformBuffer<WaterSimUniform>,
    spectrum: Buffer,
    butterfly: Buffer,
    /// Scratch for the transform: `cascades × map² × 4 spectra × 2` complex
    /// values — the second half is the Stockham ping-pong target.
    fft: Buffer,
    /// Foam accumulator, one f32 per texel per cascade. It has to persist
    /// across frames (foam grows and decays over time), and it lives in a
    /// buffer rather than the normal map's alpha because read-write storage
    /// textures of this format are not portable.
    foam: Buffer,
    /// Butterfly factors only depend on `map_size`; computed on the first pass
    /// after a resize.
    butterfly_ready: bool,
    fft_pipeline: CachedComputePipelineId,
}

fn prepare_water_sim(
    mut commands: Commands,
    params: Res<WaterSimParams>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipelines: Res<WaterSimPipelines>,
    existing: Option<ResMut<WaterSimResources>>,
) {
    let map_size = params.uniform.map_size;
    let num_cascades = params.uniform.num_cascades;
    if num_cascades == 0 || map_size == 0 {
        return;
    }

    let needs_rebuild = existing
        .as_ref()
        .map(|r| r.map_size != map_size || r.num_cascades != num_cascades)
        .unwrap_or(true);

    if !needs_rebuild {
        let mut resources = existing.expect("checked above");
        resources.uniform.set(params.uniform);
        return;
    }

    let map = map_size as u64;
    let cascades = num_cascades as u64;
    let stages = map.trailing_zeros() as u64;

    let storage = |label: &'static str, size: u64| {
        device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };

    let spectrum = storage("water_spectrum_buffer", cascades * map * map * 16);
    let butterfly = storage("water_butterfly_buffer", stages * map * 16);
    let fft = storage(
        "water_fft_buffer",
        cascades * map * map * NUM_SPECTRA * 2 * 8,
    );
    let foam = storage("water_foam_buffer", cascades * map * map * 4);

    // The FFT kernel bakes its resolution in: workgroup memory is sized to one
    // row, and the thread count is capped at WebGPU's 256.
    let threads = map_size.min(256);
    let fft_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(Cow::Borrowed("water_fft")),
        layout: vec![pipelines.fft_layout.clone()],
        shader: pipelines.fft_shader.clone(),
        shader_defs: vec![
            ShaderDefVal::UInt("MAP_SIZE".into(), map_size),
            ShaderDefVal::UInt("FFT_THREADS".into(), threads),
        ],
        entry_point: Some(Cow::Borrowed("main")),
        immediate_size: 0,
        zero_initialize_workgroup_memory: false,
    });

    let mut uniform = UniformBuffer::from(params.uniform);
    uniform.set_label(Some("water_sim_uniform"));

    commands.insert_resource(WaterSimResources {
        map_size,
        num_cascades,
        uniform,
        spectrum,
        butterfly,
        fft,
        foam,
        butterfly_ready: false,
        fft_pipeline,
    });
}

fn write_water_uniform(
    device: Res<RenderDevice>,
    queue: Res<bevy::render::renderer::RenderQueue>,
    resources: Option<ResMut<WaterSimResources>>,
) {
    let Some(mut resources) = resources else {
        return;
    };
    resources.uniform.write_buffer(&device, &queue);
}

/// The compute pass itself.
fn water_sim_pass(
    mut render_context: RenderContext,
    pipeline_cache: Res<PipelineCache>,
    pipelines: Res<WaterSimPipelines>,
    resources: Option<ResMut<WaterSimResources>>,
    params: Res<WaterSimParams>,
    textures: Option<Res<WaterSimTextures>>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let (Some(mut resources), Some(textures)) = (resources, textures) else {
        return;
    };
    if !params.step && !params.regenerate_spectrum && resources.butterfly_ready {
        return;
    }

    let (Some(displacement), Some(normal)) = (
        images.get(&textures.displacement),
        images.get(&textures.normal),
    ) else {
        return;
    };
    let Some(uniform_binding) = resources.uniform.binding() else {
        return;
    };

    // Every pipeline has to be compiled before any of them may run: a partial
    // sequence would leave the scratch buffer half-transformed and the maps
    // full of garbage until the next frame.
    let (
        Some(spectrum_pl),
        Some(modulate_pl),
        Some(butterfly_pl),
        Some(fft_pl),
        Some(transpose_pl),
        Some(unpack_pl),
    ) = (
        pipeline_cache.get_compute_pipeline(pipelines.spectrum),
        pipeline_cache.get_compute_pipeline(pipelines.modulate),
        pipeline_cache.get_compute_pipeline(pipelines.butterfly),
        pipeline_cache.get_compute_pipeline(resources.fft_pipeline),
        pipeline_cache.get_compute_pipeline(pipelines.transpose),
        pipeline_cache.get_compute_pipeline(pipelines.unpack),
    )
    else {
        return;
    };

    let device = render_context.render_device().clone();
    let map_size = resources.map_size;
    let cascades = resources.num_cascades;
    let tiles = map_size.div_ceil(TILE);
    let stages = map_size.trailing_zeros();

    let spectrum_bg = device.create_bind_group(
        "water_spectrum_bg",
        &pipeline_cache.get_bind_group_layout(&pipelines.spectrum_layout),
        &BindGroupEntries::sequential((
            uniform_binding.clone(),
            resources.spectrum.as_entire_binding(),
        )),
    );
    let modulate_bg = device.create_bind_group(
        "water_modulate_bg",
        &pipeline_cache.get_bind_group_layout(&pipelines.modulate_layout),
        &BindGroupEntries::sequential((
            uniform_binding.clone(),
            resources.spectrum.as_entire_binding(),
            resources.fft.as_entire_binding(),
        )),
    );
    let butterfly_bg = device.create_bind_group(
        "water_butterfly_bg",
        &pipeline_cache.get_bind_group_layout(&pipelines.butterfly_layout),
        &BindGroupEntries::sequential((
            uniform_binding.clone(),
            resources.butterfly.as_entire_binding(),
        )),
    );
    let fft_bg = device.create_bind_group(
        "water_fft_bg",
        &pipeline_cache.get_bind_group_layout(&pipelines.fft_layout),
        &BindGroupEntries::sequential((
            uniform_binding.clone(),
            resources.butterfly.as_entire_binding(),
            resources.fft.as_entire_binding(),
        )),
    );
    let transpose_bg = device.create_bind_group(
        "water_transpose_bg",
        &pipeline_cache.get_bind_group_layout(&pipelines.transpose_layout),
        &BindGroupEntries::sequential((
            uniform_binding.clone(),
            resources.fft.as_entire_binding(),
        )),
    );
    let unpack_bg = device.create_bind_group(
        "water_unpack_bg",
        &pipeline_cache.get_bind_group_layout(&pipelines.unpack_layout),
        &BindGroupEntries::sequential((
            uniform_binding.clone(),
            resources.fft.as_entire_binding(),
            resources.foam.as_entire_binding(),
            &displacement.texture_view,
            &normal.texture_view,
        )),
    );

    let regenerate = params.regenerate_spectrum || !resources.butterfly_ready;
    let butterfly_needed = !resources.butterfly_ready;

    {
        let _span = info_span!("water.simulate").entered();
        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("water_wave_simulation"),
                timestamp_writes: None,
            });

        if butterfly_needed {
            pass.set_pipeline(butterfly_pl);
            pass.set_bind_group(0, &butterfly_bg, &[]);
            // Half the columns, 64 per workgroup — each invocation writes the
            // two butterfly entries of one pair.
            pass.dispatch_workgroups((map_size / 2).div_ceil(64), stages, 1);
        }

        if regenerate {
            pass.set_pipeline(spectrum_pl);
            pass.set_bind_group(0, &spectrum_bg, &[]);
            pass.dispatch_workgroups(tiles, tiles, cascades);
        }

        if params.step {
            pass.set_pipeline(modulate_pl);
            pass.set_bind_group(0, &modulate_bg, &[]);
            pass.dispatch_workgroups(tiles, tiles, cascades);

            // One workgroup per row, per spectrum, per cascade.
            pass.set_pipeline(fft_pl);
            pass.set_bind_group(0, &fft_bg, &[]);
            pass.dispatch_workgroups(1, map_size, NUM_SPECTRA as u32 * cascades);

            pass.set_pipeline(transpose_pl);
            pass.set_bind_group(0, &transpose_bg, &[]);
            pass.dispatch_workgroups(tiles, tiles, NUM_SPECTRA as u32 * cascades);

            pass.set_pipeline(fft_pl);
            pass.set_bind_group(0, &fft_bg, &[]);
            pass.dispatch_workgroups(1, map_size, NUM_SPECTRA as u32 * cascades);

            pass.set_pipeline(unpack_pl);
            pass.set_bind_group(0, &unpack_bg, &[]);
            pass.dispatch_workgroups(tiles, tiles, cascades);
        }
    }

    resources.butterfly_ready = true;
}

/// Registers the simulation. Main-world state lives in `systems.rs`; this only
/// owns the render-world half.
pub struct WaterSimPlugin;

impl Plugin for WaterSimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaterSimParams>().add_plugins((
            ExtractResourcePlugin::<WaterSimParams>::default(),
            ExtractResourcePlugin::<WaterSimTextures>::default(),
        ));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(
                Render,
                (
                    prepare_water_sim.in_set(RenderSystems::PrepareResources),
                    write_water_uniform.in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(
                RenderGraph,
                water_sim_pass
                    .in_set(RenderGraphSystems::Render)
                    // View-independent work: once per frame, before any camera
                    // starts drawing with the maps it produces.
                    .before(bevy::core_pipeline::schedule::camera_driver),
            );
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<WaterSimPipelines>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile a compute kernel exactly as wgpu will. A shader error otherwise
    /// shows up only as a pipeline that never becomes ready — the ocean simply
    /// stays flat, with the reason buried in the log.
    fn validate(name: &str, source: &str) {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|err| panic!("{name}: {}", err.emit_to_string(source)));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        if let Err(err) = validator.validate(&module) {
            panic!("{name}: {}", err.emit_to_string(source));
        }
    }

    #[test]
    fn compute_shaders_compile() {
        validate(
            "water_spectrum",
            include_str!("shaders/water_spectrum.wgsl"),
        );
        validate(
            "water_modulate",
            include_str!("shaders/water_modulate.wgsl"),
        );
        validate(
            "water_butterfly",
            include_str!("shaders/water_butterfly.wgsl"),
        );
        validate(
            "water_transpose",
            include_str!("shaders/water_transpose.wgsl"),
        );
        validate("water_unpack", include_str!("shaders/water_unpack.wgsl"));
    }

    #[test]
    fn fft_shader_compiles_at_every_resolution() {
        // The FFT kernel is the one specialised by shader def, so it has to be
        // checked at each size the inspector offers — including 1024, where the
        // ping-pong row buffer sits exactly on WebGPU's 16 KiB workgroup-memory
        // limit.
        let source = include_str!("shaders/water_fft.wgsl");
        for map_size in [128u32, 256, 512, 1024] {
            let threads = map_size.min(256);
            let expanded = source
                .replace("#{MAP_SIZE}", &map_size.to_string())
                .replace("#{FFT_THREADS}", &threads.to_string());
            validate(&format!("water_fft({map_size})"), &expanded);

            let workgroup_bytes = 2 * map_size as usize * 8;
            assert!(
                workgroup_bytes <= 16384,
                "map_size {map_size} needs {workgroup_bytes} B of workgroup memory"
            );
            assert!(threads <= 256, "workgroup size {threads} exceeds WebGPU's cap");
        }
    }

    #[test]
    fn cascade_uniform_layout_matches_the_shaders() {
        // The `Cascade`/`WaterSim` structs are duplicated at the top of every
        // compute shader. If this side's layout drifts, the shaders read
        // garbage parameters and the sea quietly goes wrong rather than
        // failing — so pin the sizes the WGSL assumes.
        assert_eq!(CascadeGpu::min_size().get(), 64);
        assert_eq!(
            WaterSimUniform::min_size().get(),
            16 + 64 * MAX_CASCADES as u64
        );
    }
}
