//! Bakes the two tileable noise volumes the cloud raymarch samples.
//!
//! Both fields are static: the wind moves the *sample* position, not the noise,
//! and every shape knob in `CloudsData` acts on the way they are read rather
//! than on the way they are generated. So this runs one compute pass on the
//! first frame the pipelines are ready and then never touches the GPU again —
//! which is why there is a `baked` latch instead of a dirty check.
//!
//! Generating them on the CPU instead was the obvious alternative and is not
//! viable: the base map is a million texels of 7-octave Perlin FBM multiplied by
//! 8-octave Worley, and each Worley octave visits 27 cells. That is a couple of
//! seconds of startup stall spread over the task pool, versus well under a
//! frame here.

use std::borrow::Cow;

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{texture_storage_2d, texture_storage_3d};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderGraph, RenderGraphSystems};
use bevy::render::texture::GpuImage;
use bevy::render::RenderApp;

/// Side of the square weather/shape atlas. The reference bakes 1920²; the top
/// Worley octave in it runs at frequency 1152, so anything past ~1k is storing
/// sub-texel grain. 1024² of RGBA16F is 8 MB.
pub const BASE_SIZE: u32 = 1024;

/// Side of the cubic erosion volume. The detail scale is tuned against 32³ in
/// the reference, and a proper trilinear sampler (which the reference lacked)
/// gets more out of it than raising the resolution would.
pub const DETAIL_SIZE: u32 = 32;

/// RGBA16F for two reasons: the atlas's green channel is negative — it is the
/// low end of a remap window, in [-1, 0] — and 16-bit float is the narrowest
/// storage-capable format WebGPU guarantees for a 3D texture.
const NOISE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

const BASE_WORKGROUP: u32 = 8;
const DETAIL_WORKGROUP: u32 = 4;

/// The baked fields, held in the main world so the material can reference them
/// as ordinary asset handles.
#[derive(Resource, Clone, ExtractResource)]
pub struct CloudNoiseTextures {
    /// 2D atlas sampled by world XZ. `r` = Perlin-Worley silhouette,
    /// `g` = per-region remap window, `b` = height-gradient modifier.
    pub base: Handle<Image>,
    /// 3D Worley that erodes the base silhouette into wisps.
    pub detail: Handle<Image>,
}

impl FromWorld for CloudNoiseTextures {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.resource_mut::<Assets<Image>>();

        let mut make = |label: &'static str, extent: Extent3d, dimension: TextureDimension| {
            let mut image = Image::new_uninit(
                extent,
                dimension,
                NOISE_FORMAT,
                // Compute-only content: there is never a main-world copy to keep.
                RenderAssetUsages::RENDER_WORLD,
            );
            image.texture_descriptor.label = Some(label);
            image.texture_descriptor.usage =
                TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING;
            // The whole point of baking tileable noise is that a ray can travel
            // any distance and the wind offset can grow without bound; anything
            // but Repeat turns that into one stretched tile and a clamped smear.
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                address_mode_w: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                mipmap_filter: ImageFilterMode::Linear,
                ..default()
            });
            images.add(image)
        };

        Self {
            base: make(
                "cloud_base_noise",
                Extent3d {
                    width: BASE_SIZE,
                    height: BASE_SIZE,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
            ),
            detail: make(
                "cloud_detail_noise",
                Extent3d {
                    width: DETAIL_SIZE,
                    height: DETAIL_SIZE,
                    depth_or_array_layers: DETAIL_SIZE,
                },
                TextureDimension::D3,
            ),
        }
    }
}

#[derive(Resource)]
struct CloudNoisePipelines {
    layout: BindGroupLayoutDescriptor,
    base: CachedComputePipelineId,
    detail: CachedComputePipelineId,
}

impl FromWorld for CloudNoisePipelines {
    fn from_world(world: &mut World) -> Self {
        let shader: Handle<Shader> = world
            .resource::<AssetServer>()
            // Crate name in the path — `clouds`, not `renzora_clouds`.
            .load("embedded://clouds/clouds_bake.wgsl");

        let layout = BindGroupLayoutDescriptor::new(
            "cloud_noise_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_storage_2d(NOISE_FORMAT, StorageTextureAccess::WriteOnly),
                    texture_storage_3d(NOISE_FORMAT, StorageTextureAccess::WriteOnly),
                ),
            ),
        );

        let pipeline_cache = world.resource::<PipelineCache>();
        let queue = |label: &'static str, entry_point: &'static str| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(Cow::Borrowed(label)),
                layout: vec![layout.clone()],
                shader: shader.clone(),
                shader_defs: vec![],
                entry_point: Some(Cow::Borrowed(entry_point)),
                immediate_size: 0,
                zero_initialize_workgroup_memory: false,
            })
        };

        let base = queue("cloud_bake_base", "bake_base");
        let detail = queue("cloud_bake_detail", "bake_detail");

        Self {
            layout,
            base,
            detail,
        }
    }
}

/// Latch: set once the single bake has been recorded.
#[derive(Resource, Default)]
struct CloudNoiseBaked(bool);

fn bake_cloud_noise(
    mut render_context: RenderContext,
    mut baked: ResMut<CloudNoiseBaked>,
    pipeline_cache: Res<PipelineCache>,
    pipelines: Res<CloudNoisePipelines>,
    textures: Option<Res<CloudNoiseTextures>>,
    images: Res<RenderAssets<GpuImage>>,
) {
    if baked.0 {
        return;
    }
    let Some(textures) = textures else {
        return;
    };
    let (Some(base_image), Some(detail_image)) =
        (images.get(&textures.base), images.get(&textures.detail))
    else {
        return;
    };
    // Both kernels share one bind group, so neither may run until both compile —
    // otherwise the latch would trip on a half-baked pair and the detail volume
    // would stay uninitialised for the rest of the session.
    let (Some(base_pipeline), Some(detail_pipeline)) = (
        pipeline_cache.get_compute_pipeline(pipelines.base),
        pipeline_cache.get_compute_pipeline(pipelines.detail),
    ) else {
        return;
    };

    let device = render_context.render_device().clone();
    let bind_group = device.create_bind_group(
        "cloud_noise_bg",
        &pipeline_cache.get_bind_group_layout(&pipelines.layout),
        &BindGroupEntries::sequential((&base_image.texture_view, &detail_image.texture_view)),
    );

    {
        let _span = info_span!("clouds.bake_noise").entered();
        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("cloud_noise_bake"),
                timestamp_writes: None,
            });
        pass.set_bind_group(0, &bind_group, &[]);

        pass.set_pipeline(base_pipeline);
        pass.dispatch_workgroups(
            BASE_SIZE.div_ceil(BASE_WORKGROUP),
            BASE_SIZE.div_ceil(BASE_WORKGROUP),
            1,
        );

        pass.set_pipeline(detail_pipeline);
        let detail_groups = DETAIL_SIZE.div_ceil(DETAIL_WORKGROUP);
        pass.dispatch_workgroups(detail_groups, detail_groups, detail_groups);
    }

    baked.0 = true;
}

/// Owns the render-world half of the bake. The handles themselves live in
/// [`CloudNoiseTextures`], which the dome material reads from the main world.
pub struct CloudNoisePlugin;

impl Plugin for CloudNoisePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractResourcePlugin::<CloudNoiseTextures>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(
            RenderGraph,
            bake_cloud_noise
                .in_set(RenderGraphSystems::Render)
                // View-independent, and the first camera to draw already wants
                // to sample the result.
                .before(bevy::core_pipeline::schedule::camera_driver),
        );
    }

    fn finish(&self, app: &mut App) {
        // Deferred to `finish` because building the images needs `Assets<Image>`,
        // which only exists once every plugin has had its `build` run.
        app.init_resource::<CloudNoiseTextures>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<CloudNoisePipelines>()
                .init_resource::<CloudNoiseBaked>();
        }
    }
}
