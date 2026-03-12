use crate::settings::RtLighting;
use bevy::prelude::*;
use bevy::image::ToExtents;
use bevy::math::UVec2;
use bevy::render::{
    camera::ExtractedCamera,
    render_resource::{
        Buffer, BufferDescriptor, BufferUsages, Extent3d, Texture, TextureDescriptor,
        TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
    },
    renderer::RenderDevice,
};

/// Amount of entries in the world-space radiance cache (must be power of 2).
pub const RADIANCE_CACHE_SIZE: u64 = 2u64.pow(19); // 512K cells

/// Internal GPU resources allocated per view for the RT pipeline.
#[derive(Component)]
pub struct RtLightingResources {
    // Hi-Z depth pyramid (kept alive so hi_z_views remain valid)
    #[allow(dead_code)]
    pub hi_z_texture: Texture,
    pub hi_z_views: Vec<TextureView>,
    #[allow(dead_code)]
    pub hi_z_mip_count: u32,

    // GI output (current + history for temporal denoise)
    pub gi_output: (Texture, TextureView),
    pub gi_history: (Texture, TextureView),

    // Reflections output + history
    pub refl_output: (Texture, TextureView),
    pub refl_history: (Texture, TextureView),

    // Shadow mask output + history
    pub shadow_output: (Texture, TextureView),
    pub shadow_history: (Texture, TextureView),

    // Denoise ping-pong textures
    pub denoise_ping: (Texture, TextureView),
    pub denoise_pong: (Texture, TextureView),

    // Radiance cache buffers (world-space, persistent)
    pub radiance_cache_checksums: Buffer,
    pub radiance_cache_life: Buffer,
    pub radiance_cache_radiance: Buffer,
    pub radiance_cache_normals: Buffer,
    pub radiance_cache_sample_count: Buffer,

    // Fallback 1x1 zero motion vector texture (used when MotionVectorPrepass unavailable)
    #[allow(dead_code)]
    pub fallback_motion_vectors: Texture,
    pub fallback_motion_vectors_view: TextureView,

    // Fallback 1x1 deferred GBuffer texture (used when DeferredPrepass unavailable)
    #[allow(dead_code)]
    pub fallback_deferred: Texture,
    pub fallback_deferred_view: TextureView,

    pub view_size: UVec2,
    pub trace_size: UVec2,
}

fn create_rt_texture(
    device: &RenderDevice,
    label: &str,
    size: UVec2,
    format: TextureFormat,
) -> (Texture, TextureView) {
    let tex = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: size.to_extents(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&TextureViewDescriptor::default());
    (tex, view)
}

pub fn prepare_rt_lighting_resources(
    query: Query<
        (Entity, &ExtractedCamera, Option<&RtLightingResources>, &RtLighting),
    >,
    render_device: Res<RenderDevice>,
    mut commands: Commands,
) {
    for (entity, camera, existing, settings) in &query {
        let Some(view_size) = camera.physical_viewport_size else {
            continue;
        };

        // Skip re-allocation if size hasn't changed
        if existing.map(|r| r.view_size) == Some(view_size) {
            continue;
        }

        let trace_size = if settings.half_res() {
            UVec2::new(
                (view_size.x + 1) / 2,
                (view_size.y + 1) / 2,
            )
        } else {
            view_size
        };

        // Hi-Z pyramid
        let hi_z_mip_count = (view_size.x.max(view_size.y) as f32).log2().floor() as u32 + 1;
        let hi_z_texture = render_device.create_texture(&TextureDescriptor {
            label: Some("rt_hi_z_depth"),
            size: Extent3d {
                width: view_size.x,
                height: view_size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: hi_z_mip_count,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let hi_z_views: Vec<TextureView> = (0..hi_z_mip_count)
            .map(|mip| {
                hi_z_texture.create_view(&TextureViewDescriptor {
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..default()
                })
            })
            .collect();

        // GI textures (at trace resolution — half or full res)
        let gi_output = create_rt_texture(&render_device, "rt_gi_output", trace_size, TextureFormat::Rgba16Float);
        let gi_history = create_rt_texture(&render_device, "rt_gi_history", trace_size, TextureFormat::Rgba16Float);

        // Reflection textures (at trace resolution)
        let refl_output = create_rt_texture(&render_device, "rt_refl_output", trace_size, TextureFormat::Rgba16Float);
        let refl_history = create_rt_texture(&render_device, "rt_refl_history", trace_size, TextureFormat::Rgba16Float);

        // Shadow textures (full resolution for sharpness)
        let shadow_output = create_rt_texture(&render_device, "rt_shadow_output", view_size, TextureFormat::R16Float);
        let shadow_history = create_rt_texture(&render_device, "rt_shadow_history", view_size, TextureFormat::R16Float);

        // Denoise ping-pong (at trace resolution)
        let denoise_ping = create_rt_texture(&render_device, "rt_denoise_ping", trace_size, TextureFormat::Rgba16Float);
        let denoise_pong = create_rt_texture(&render_device, "rt_denoise_pong", trace_size, TextureFormat::Rgba16Float);

        // Radiance cache buffers (persistent, view-independent size)
        let create_cache_buffer = |label: &str, element_size: u64| {
            render_device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: RADIANCE_CACHE_SIZE * element_size,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            })
        };

        let radiance_cache_checksums = create_cache_buffer("rt_cache_checksums", 4);
        let radiance_cache_life = create_cache_buffer("rt_cache_life", 4);
        let radiance_cache_radiance = create_cache_buffer("rt_cache_radiance", 16); // vec4<f32>
        let radiance_cache_normals = create_cache_buffer("rt_cache_normals", 16);   // vec4<f32>
        let radiance_cache_sample_count = create_cache_buffer("rt_cache_samples", 4);

        // 1x1 zero motion vector texture — fallback when MotionVectorPrepass missing
        let fallback_mv = render_device.create_texture(&TextureDescriptor {
            label: Some("rt_fallback_motion_vectors"),
            size: Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rg16Float,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fallback_mv_view = fallback_mv.create_view(&TextureViewDescriptor::default());

        // 1x1 zero deferred GBuffer texture — fallback when DeferredPrepass missing
        let fallback_deferred = render_device.create_texture(&TextureDescriptor {
            label: Some("rt_fallback_deferred"),
            size: Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Uint,
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let fallback_deferred_view = fallback_deferred.create_view(&TextureViewDescriptor::default());

        commands.entity(entity).insert(RtLightingResources {
            hi_z_texture,
            hi_z_views,
            hi_z_mip_count,
            gi_output,
            gi_history,
            refl_output,
            refl_history,
            shadow_output,
            shadow_history,
            denoise_ping,
            denoise_pong,
            radiance_cache_checksums,
            radiance_cache_life,
            radiance_cache_radiance,
            radiance_cache_normals,
            radiance_cache_sample_count,
            fallback_motion_vectors: fallback_mv,
            fallback_motion_vectors_view: fallback_mv_view,
            fallback_deferred,
            fallback_deferred_view,
            view_size,
            trace_size,
        });
    }
}
