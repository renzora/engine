use super::SolariLighting;
use crate::scene::RaytracingSceneNeedsPreviousFrameData;
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy_anti_alias::dlss::{
    Dlss, DlssRayReconstructionFeature, ViewDlssRayReconstructionTextures,
};
use bevy_camera::MainPassResolutionOverride;
use bevy_diagnostic::FrameCount;
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy_ecs::query::Has;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    system::{Commands, Query, Res},
};
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy_image::ToExtents;
use bevy_math::UVec2;
use bevy_render::{
    camera::ExtractedCamera,
    render_resource::{
        Buffer, BufferDescriptor, BufferInitDescriptor, BufferUsages, Extent3d,
        TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    },
    renderer::{RenderDevice, RenderQueue},
    texture::CachedTexture,
};
use bytemuck::{Pod, Zeroable};
use core::sync::atomic::{AtomicU32, Ordering};

/// Size of the `LightSample` shader struct in bytes.
const LIGHT_SAMPLE_STRUCT_SIZE: u64 = 8;

/// Size of the `ResolvedLightSamplePacked` shader struct in bytes.
const RESOLVED_LIGHT_SAMPLE_STRUCT_SIZE: u64 = 24;

/// Size of the `Reservoir` shader struct in bytes.
const RESERVOIR_STRUCT_SIZE: u64 = 48;

pub const LIGHT_TILE_BLOCKS: u64 = 128;
pub const LIGHT_TILE_SAMPLES_PER_BLOCK: u64 = 1024;

/// Amount of entries in the world cache (must be a power of 2, and >= 2^10)
pub const WORLD_CACHE_SIZE: u64 = 2u64.pow(20);
/// Sum of per-cell field sizes in `WorldCache`. Keep in sync with `realtime_bindings.wgsl`.
const WORLD_CACHE_ENTRY_SIZE: u64 = 84;
/// Size of the fixed `b` array (`array<u32, WORLD_CACHE_SIZE / 1024>`).
const WORLD_CACHE_B_SIZE: u64 = (WORLD_CACHE_SIZE / 1024) * size_of::<u32>() as u64;
/// Offset of `active_cells_count`.
pub const WORLD_CACHE_ACTIVE_CELLS_COUNT_OFFSET: u64 =
    WORLD_CACHE_SIZE * WORLD_CACHE_ENTRY_SIZE + WORLD_CACHE_B_SIZE;
/// Must stay under wgpu's default `max_storage_buffer_binding_size` (128 MiB or 2^27 bytes).
pub const WORLD_CACHE_BUFFER_SIZE: u64 =
    (WORLD_CACHE_ACTIVE_CELLS_COUNT_OFFSET + size_of::<u32>() as u64).next_multiple_of(16);

/// GPU representation of the user-configurable [`SolariLighting`] settings, plus
/// per-frame state.
///
/// Field order and types must match the `SolariLightingSettings` struct in
/// `realtime_bindings.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SolariLightingUniforms {
    confidence_weight_cap: f32,
    primary_di_samples: u32,
    secondary_di_samples: u32,
    max_bounces: u32,
    world_cache_max_temporal_samples: f32,
    world_cache_direct_light_sample_count: u32,
    world_cache_max_gi_ray_distance: f32,
    world_cache_cell_updates_soft_target: u32,
    world_cache_position_base_cell_size: f32,
    world_cache_position_lod_scale: f32,
    frame_rng: u32,
    reset: u32,
    /// RENZORA PATCH: written 0 here every frame; the diagnostics set it on the
    /// views they count rays for (`debug::prepare_solari_debug`).
    debug_flags: u32,
    padding: [u32; 3],
}

impl SolariLightingUniforms {
    fn new(settings: &SolariLighting, frame_count: u32, force_reset: bool) -> Self {
        Self {
            confidence_weight_cap: settings.confidence_weight_cap,
            primary_di_samples: settings.primary_di_samples,
            secondary_di_samples: settings.secondary_di_samples,
            max_bounces: settings.max_bounces,
            world_cache_max_temporal_samples: settings.world_cache_max_temporal_samples,
            world_cache_direct_light_sample_count: settings.world_cache_direct_light_sample_count,
            world_cache_max_gi_ray_distance: settings.world_cache_max_gi_ray_distance,
            world_cache_cell_updates_soft_target: settings.world_cache_cell_updates_soft_target,
            world_cache_position_base_cell_size: settings.world_cache_position_base_cell_size,
            world_cache_position_lod_scale: settings.world_cache_position_lod_scale,
            frame_rng: frame_count.wrapping_mul(5782582),
            reset: (settings.reset || force_reset) as u32,
            debug_flags: 0,
            padding: [0; 3],
        }
    }
}

/// Declares to the raytracing scene whether any view needs last frame's TLAS and light ids.
pub fn setup_raytracing_scene_needs_previous_frame_data(
    views: Query<&SolariLighting>,
    needs_previous_frame_data: Option<Res<RaytracingSceneNeedsPreviousFrameData>>,
    mut commands: Commands,
) {
    let restir_used = views.iter().any(|solari_lighting| solari_lighting.restir);
    match (restir_used, needs_previous_frame_data.is_some()) {
        (true, false) => commands.insert_resource(RaytracingSceneNeedsPreviousFrameData),
        (false, true) => commands.remove_resource::<RaytracingSceneNeedsPreviousFrameData>(),
        _ => {}
    }
}

/// Internal rendering resources used for Solari lighting.
#[derive(Component)]
pub struct SolariLightingResources {
    pub constants: Buffer,
    pub light_tile_samples: Buffer,
    pub light_tile_resolved_samples: Buffer,
    pub reservoirs: Option<SolariReservoirBuffers>,
    pub world_cache: Buffer,
    pub world_cache_active_cells_dispatch: Buffer,
    pub view_size: UVec2,
    /// RENZORA PATCH: the built-in denoiser's buffers. See denoise.wesl.
    pub denoiser: SolariDenoiserResources,
    /// RENZORA PATCH: the diagnostics' counters (debug_layout.wesl). Always
    /// there, because the lighting passes bind it, but only written to while a
    /// `SolariDebug` asks.
    pub debug_counters: Buffer,
}

/// RENZORA PATCH: per-view state of the built-in denoiser (denoise.wesl).
///
/// Always allocated, because the lighting passes write the split signal into
/// `noisy_*` whether or not the denoiser then runs: keeping one layout avoids
/// compiling every lighting pipeline twice.
pub struct SolariDenoiserResources {
    /// The lighting split into diffuse and specular, demodulated and exposed.
    pub noisy_diffuse: CachedTexture,
    pub noisy_specular: CachedTexture,
    /// The other half of the a-trous ping-pong, which `noisy_*` is reused for
    /// once the temporal pass has read it. rgb is the signal, a the variance.
    pub filtered_diffuse: CachedTexture,
    pub filtered_specular: CachedTexture,
    /// Accumulated signal (rgb) and history length in frames (a), one per frame
    /// parity: the temporal pass reads last frame's and writes this frame's.
    pub history_diffuse: [CachedTexture; 2],
    pub history_specular: [CachedTexture; 2],
    /// Luminance moments for the noise estimate: diffuse in xy, specular in zw.
    pub moments: [CachedTexture; 2],
    /// What the a-trous edge-stops need of each pixel, packed once by the
    /// temporal pass: world position in xyz (f32 bits), and the normal,
    /// roughness and a coverage bit in w. See `pack_guide` in denoise.wesl.
    pub guide: CachedTexture,
    pub constants: Buffer,
    /// Frames this view has been denoised for in a row; its parity picks the
    /// history slots, and 0 means there is no history yet.
    pub frame: AtomicU32,
}

impl SolariDenoiserResources {
    /// Which history slot this frame writes; it reads the other.
    pub fn write_slot(&self) -> usize {
        (self.frame.load(Ordering::Relaxed) % 2) as usize
    }
}

/// RENZORA PATCH: GPU side of the denoiser's per-frame state. Must match
/// `DenoiserSettings` in denoise.wesl, padded to the 16 bytes a uniform needs.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DenoiserUniforms {
    reset: u32,
    padding: [u32; 3],
}

fn create_denoiser_resources(render_device: &RenderDevice, view_size: UVec2) -> SolariDenoiserResources {
    let texture_of = |label: &'static str, format: TextureFormat| {
        let texture = render_device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width: view_size.x,
                height: view_size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let default_view = texture.create_view(&TextureViewDescriptor::default());
        CachedTexture {
            texture,
            default_view,
        }
    };
    let texture = |label: &'static str| texture_of(label, TextureFormat::Rgba16Float);
    SolariDenoiserResources {
        noisy_diffuse: texture("solari_denoiser_noisy_diffuse"),
        noisy_specular: texture("solari_denoiser_noisy_specular"),
        filtered_diffuse: texture("solari_denoiser_filtered_diffuse"),
        filtered_specular: texture("solari_denoiser_filtered_specular"),
        history_diffuse: [
            texture("solari_denoiser_history_diffuse_a"),
            texture("solari_denoiser_history_diffuse_b"),
        ],
        history_specular: [
            texture("solari_denoiser_history_specular_a"),
            texture("solari_denoiser_history_specular_b"),
        ],
        moments: [
            texture("solari_denoiser_moments_a"),
            texture("solari_denoiser_moments_b"),
        ],
        guide: texture_of("solari_denoiser_guide", TextureFormat::Rgba32Uint),
        constants: render_device.create_buffer(&BufferDescriptor {
            label: Some("solari_denoiser_constants"),
            size: size_of::<DenoiserUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        frame: AtomicU32::new(0),
    }
}

/// RENZORA PATCH: advance the denoiser one frame and upload its state. History
/// is unusable on a frame Solari resets, on the frame the resources were made,
/// and on the first frame after the denoiser was off, whether switched off or
/// standing aside for DLSS.
fn update_denoiser(render_queue: &RenderQueue, denoiser: &SolariDenoiserResources, active: bool, reset: bool) {
    if !active {
        denoiser.frame.store(0, Ordering::Relaxed);
        return;
    }
    let previous = denoiser.frame.fetch_add(1, Ordering::Relaxed);
    let uniforms = DenoiserUniforms {
        reset: (reset || previous == 0) as u32,
        padding: [0; 3],
    };
    render_queue.write_buffer(&denoiser.constants, 0, bytemuck::bytes_of(&uniforms));
}

pub struct SolariReservoirBuffers {
    pub a: Buffer,
    pub b: Buffer,
}

pub fn prepare_solari_lighting_resources(
    #[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))] query: Query<(
        Entity,
        &ExtractedCamera,
        &SolariLighting,
        Option<&SolariLightingResources>,
        Option<&MainPassResolutionOverride>,
    )>,
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))] query: Query<(
        Entity,
        &ExtractedCamera,
        &SolariLighting,
        Option<&SolariLightingResources>,
        Option<&MainPassResolutionOverride>,
        Has<Dlss<DlssRayReconstructionFeature>>,
        Has<ViewDlssRayReconstructionTextures>,
    )>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    frame_count: Res<FrameCount>,
    mut commands: Commands,
) {
    for query_item in &query {
        #[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))]
        let (entity, camera, solari_lighting, solari_lighting_resources, resolution_override) =
            query_item;
        #[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))]
        let has_dlss_rr = false;
        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        let (
            entity,
            camera,
            solari_lighting,
            solari_lighting_resources,
            resolution_override,
            has_dlss_rr,
            has_dlss_rr_textures,
        ) = query_item;
        // RENZORA PATCH: the built-in denoiser steps aside while DLSS Ray
        // Reconstruction denoises (see the node), and must start over when it
        // takes back over: its history is from before DLSS was switched on.
        let denoiser_active = solari_lighting.denoise && !has_dlss_rr;

        let Some(mut view_size) = camera.physical_viewport_size else {
            continue;
        };
        if let Some(MainPassResolutionOverride(resolution_override)) = resolution_override {
            view_size = *resolution_override;
        }

        let reusable = solari_lighting_resources.filter(|r| {
            r.view_size == view_size && r.reservoirs.is_some() == solari_lighting.restir
        });
        let uniforms =
            SolariLightingUniforms::new(solari_lighting, frame_count.0, reusable.is_none());

        if let Some(solari_lighting_resources) = reusable {
            // The constants uniform can change every frame, so always upload it.
            render_queue.write_buffer(
                &solari_lighting_resources.constants,
                0,
                bytemuck::bytes_of(&uniforms),
            );
            update_denoiser(
                &render_queue,
                &solari_lighting_resources.denoiser,
                denoiser_active,
                uniforms.reset != 0,
            );
            #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
            sync_dlss_rr_textures(
                &mut commands,
                &render_device,
                entity,
                view_size,
                has_dlss_rr,
                has_dlss_rr_textures,
                false,
            );
            continue;
        }

        let constants = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("solari_lighting_constants"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let light_tile_samples = render_device.create_buffer(&BufferDescriptor {
            label: Some("solari_lighting_light_tile_samples"),
            size: LIGHT_TILE_BLOCKS * LIGHT_TILE_SAMPLES_PER_BLOCK * LIGHT_SAMPLE_STRUCT_SIZE,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let light_tile_resolved_samples = render_device.create_buffer(&BufferDescriptor {
            label: Some("solari_lighting_light_tile_resolved_samples"),
            size: LIGHT_TILE_BLOCKS
                * LIGHT_TILE_SAMPLES_PER_BLOCK
                * RESOLVED_LIGHT_SAMPLE_STRUCT_SIZE,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let reservoirs = solari_lighting.restir.then(|| {
            let reservoirs_buffer = |name| {
                render_device.create_buffer(&BufferDescriptor {
                    label: Some(name),
                    size: (view_size.x * view_size.y) as u64 * RESERVOIR_STRUCT_SIZE,
                    usage: BufferUsages::STORAGE,
                    mapped_at_creation: false,
                })
            };
            SolariReservoirBuffers {
                a: reservoirs_buffer("solari_lighting_reservoirs_a"),
                b: reservoirs_buffer("solari_lighting_reservoirs_b"),
            }
        });

        let world_cache = render_device.create_buffer(&BufferDescriptor {
            label: Some("solari_lighting_world_cache"),
            size: WORLD_CACHE_BUFFER_SIZE,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let world_cache_active_cells_dispatch = render_device.create_buffer(&BufferDescriptor {
            label: Some("solari_lighting_world_cache_active_cells_dispatch"),
            size: size_of::<[u32; 3]>() as u64,
            usage: BufferUsages::INDIRECT | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let denoiser = create_denoiser_resources(&render_device, view_size);
        update_denoiser(&render_queue, &denoiser, denoiser_active, true);

        commands.entity(entity).insert(SolariLightingResources {
            constants,
            light_tile_samples,
            light_tile_resolved_samples,
            reservoirs,
            world_cache,
            world_cache_active_cells_dispatch,
            view_size,
            denoiser,
            debug_counters: render_device.create_buffer(&BufferDescriptor {
                label: Some("solari_debug_counters"),
                size: super::debug::DEBUG_COUNTERS_SIZE,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        });

        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        sync_dlss_rr_textures(
            &mut commands,
            &render_device,
            entity,
            view_size,
            has_dlss_rr,
            has_dlss_rr_textures,
            true,
        );
    }
}

/// RENZORA PATCH: give a view DLSS Ray Reconstruction's guide textures exactly
/// while it has DLSS RR, every frame rather than only when the view's other
/// resources are rebuilt.
///
/// Upstream made them only alongside the rest, which are rebuilt only when the
/// view changes size, and never took them away. Switching DLSS off at an
/// unchanged size (DLAA renders at full size, so always) left them on the view,
/// and the node reads their presence as "DLSS is denoising": the built-in
/// denoiser stood aside, DLSS no longer ran, and the raw image reached the
/// screen. Switching DLSS on later at an unchanged size never made them, so DLSS
/// had nothing to read.
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
fn sync_dlss_rr_textures(
    commands: &mut Commands,
    render_device: &RenderDevice,
    entity: Entity,
    view_size: UVec2,
    wanted: bool,
    present: bool,
    resized: bool,
) {
    if !wanted {
        if present {
            commands.entity(entity).remove::<ViewDlssRayReconstructionTextures>();
        }
        return;
    }
    if present && !resized {
        return;
    }
    {
            let diffuse_albedo = render_device.create_texture(&TextureDescriptor {
                label: Some("solari_lighting_diffuse_albedo"),
                size: view_size.to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let diffuse_albedo_view = diffuse_albedo.create_view(&TextureViewDescriptor::default());

            let specular_albedo = render_device.create_texture(&TextureDescriptor {
                label: Some("solari_lighting_specular_albedo"),
                size: view_size.to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let specular_albedo_view =
                specular_albedo.create_view(&TextureViewDescriptor::default());

            let normal_roughness = render_device.create_texture(&TextureDescriptor {
                label: Some("solari_lighting_normal_roughness"),
                size: view_size.to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let normal_roughness_view =
                normal_roughness.create_view(&TextureViewDescriptor::default());

            let dlss_rr_depth = render_device.create_texture(&TextureDescriptor {
                label: Some("solari_lighting_dlss_rr_depth"),
                size: view_size.to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R32Float,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let dlss_rr_depth_view = dlss_rr_depth.create_view(&TextureViewDescriptor::default());

            let specular_motion_vectors = render_device.create_texture(&TextureDescriptor {
                label: Some("solari_lighting_specular_motion_vectors"),
                size: view_size.to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rg16Float,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let specular_motion_vectors_view =
                specular_motion_vectors.create_view(&TextureViewDescriptor::default());

            commands
                .entity(entity)
                .insert(ViewDlssRayReconstructionTextures {
                    diffuse_albedo: CachedTexture {
                        texture: diffuse_albedo,
                        default_view: diffuse_albedo_view,
                    },
                    specular_albedo: CachedTexture {
                        texture: specular_albedo,
                        default_view: specular_albedo_view,
                    },
                    normal_roughness: CachedTexture {
                        texture: normal_roughness,
                        default_view: normal_roughness_view,
                    },
                    depth: CachedTexture {
                        texture: dlss_rr_depth,
                        default_view: dlss_rr_depth_view,
                    },
                    specular_motion_vectors: CachedTexture {
                        texture: specular_motion_vectors,
                        default_view: specular_motion_vectors_view,
                    },
                });
    }
}
