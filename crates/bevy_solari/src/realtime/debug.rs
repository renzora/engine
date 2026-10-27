//! RENZORA PATCH: diagnostics for the realtime lighting: what it costs in rays,
//! how well the denoiser is converging, where its history is thrown away, which
//! pixels the ray-traced scene does not match, and how far the image is from a
//! converged path-traced reference. The shaders, and what each pass measures,
//! are in `debug.wesl`; the counter layout is `debug_layout.wesl`.
//!
//! Driven by [`SolariDebug`] on a camera. Nothing here runs, and nothing beyond a
//! 20 KB counter buffer is allocated, until a camera has one.

use super::prepare::SolariLightingResources;
use super::SolariLighting;
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, UVec2};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::{
    render_resource::{
        binding_types::{texture_2d, texture_storage_2d, uniform_buffer_sized},
        BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, Buffer,
        BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, MapMode, PipelineCache,
        ShaderStages, StorageTextureAccess, TextureDescriptor, TextureDimension, TextureFormat,
        TextureSampleType, TextureUsages, TextureViewDescriptor,
    },
    renderer::{RenderDevice, RenderQueue},
    texture::CachedTexture,
    view::ExtractedView,
};
use bytemuck::{Pod, Zeroable};
use core::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

// ── Counter layout: must match debug_layout.wesl ────────────────────────────

const DEBUG_LANES: usize = 64;
const SLOT_RAYS_INITIAL: usize = 0;
const SLOT_RAYS_REUSE: usize = 2;
const SLOT_RAYS_WORLD_CACHE: usize = 4;
const SLOT_RAYS_DEBUG: usize = 6;
const SLOT_COVERED: usize = 8;
const SLOT_HISTORY_SUM: usize = 9;
const SLOT_HISTORY_SHORT: usize = 10;
const SLOT_REJECTED: usize = 11;
const SLOT_CONFIDENCE_SUM: usize = 12;
const SLOT_GRAIN_RAW: usize = 13;
const SLOT_GRAIN_DENOISED: usize = 14;
const SLOT_NOT_TRACED: usize = 15;
const SLOT_REFERENCE_ERROR: usize = 16;
const SLOT_REFERENCE_PIXELS: usize = 17;
const SLOT_HISTOGRAM_RAW: usize = 18;
const SLOT_HISTOGRAM_DENOISED: usize = 50;
const SLOT_COUNT: usize = 82;

/// Bins in each brightness histogram.
pub const HISTOGRAM_BINS: usize = 32;
/// The exposed luminance the first histogram bin starts at, as a power of two.
pub const HISTOGRAM_MIN_LOG2: f32 = -12.0;
/// How many stops of exposed luminance each histogram bin spans.
pub const HISTOGRAM_STOPS_PER_BIN: f32 = 0.5;

/// Size of the counter buffer every view carries, which the lighting passes
/// bind whether or not anything reads it.
pub const DEBUG_COUNTERS_SIZE: u64 = (SLOT_COUNT * DEBUG_LANES * 4) as u64;

/// Where `debug_flags` sits in the lighting's uniform (`SolariLightingSettings`
/// in bindings.wesl): after twelve 4-byte fields.
pub const DEBUG_FLAGS_OFFSET: u64 = 12 * 4;

/// `SolariLightingSettings::debug_flags`: count the rays each pass traces.
const DEBUG_COUNT_RAYS: u32 = 1;

// `DebugSettings::flags` in debug.wesl.
const FLAG_DENOISER_ACTIVE: u32 = 1;
const FLAG_REFERENCE_VALID: u32 = 2;

/// Readback buffers in flight. A map completes a frame or two after it is
/// asked for, and a free slot is needed every frame for the counters to be
/// read every frame.
const READBACK_SLOTS: usize = 3;

// ── Main world ──────────────────────────────────────────────────────────────

/// Turn on Solari's diagnostics for a camera that has [`SolariLighting`].
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component, Default, Clone)]
pub struct SolariDebug {
    /// What to show in place of the image.
    pub view: SolariDebugView,
    /// Where [`SolariDebugView::RawSplit`] divides the image, as a fraction of
    /// its width: raw to the left, the result to the right.
    pub split: f32,
    /// Gather the frame's statistics and ray counts into [`SolariDebugOutput`].
    pub stats: bool,
    /// Accumulate the path-traced reference while set. It starts over whenever
    /// the camera moves.
    pub reference: bool,
    /// Throw the reference away and start it over, after a scene change the
    /// camera cannot see (a light moved, a material edited). Cleared after the
    /// frame is extracted.
    pub reset_reference: bool,
}

impl Default for SolariDebug {
    fn default() -> Self {
        Self {
            view: SolariDebugView::None,
            split: 0.5,
            stats: false,
            reference: false,
            reset_reference: false,
        }
    }
}

/// A visualization drawn over a [`SolariDebug`] camera's image.
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Clone)]
#[repr(u32)]
pub enum SolariDebugView {
    /// The image as rendered.
    #[default]
    None = 0,
    /// The raw estimate left of [`SolariDebug::split`], the denoised result
    /// right of it.
    RawSplit = 1,
    /// How many frames each pixel's denoised lighting averages, blue for none
    /// to red for the most the denoiser keeps.
    HistoryLength = 2,
    /// Red where the denoiser threw its history away this frame.
    Rejected = 3,
    /// The denoiser's own estimate of each pixel's noise, relative to its
    /// brightness, which sets how hard it blurs.
    NoiseEstimate = 4,
    /// How many reused samples each pixel's ReSTIR reservoir stands for,
    /// against the cap. Blank without ReSTIR.
    ReservoirConfidence = 5,
    /// The world cache's stored light at each surface: what bounce light is
    /// built from, and where it is coarse.
    WorldCache = 6,
    /// Magenta where the ray-traced scene does not match what was drawn.
    NotTraced = 7,
    /// The path-traced reference.
    Reference = 8,
    /// The image's difference from the reference, relative to the reference.
    ReferenceError = 9,
}

/// One frame's diagnostics, read back from the GPU. Arrives a few frames after
/// the frame it describes.
#[derive(Clone, Debug, Default)]
pub struct SolariDebugStats {
    /// The size the lighting was computed at.
    pub view_size: UVec2,
    /// Pixels with a surface, which the per-pixel figures are averaged over.
    pub covered_pixels: u64,
    /// Rays traced by each pass, and how many of each were shadow rays: the
    /// first path from each pixel, ReSTIR's reuse, the world cache's update,
    /// and the diagnostics themselves.
    pub rays_initial: u64,
    pub shadow_rays_initial: u64,
    pub rays_reuse: u64,
    pub shadow_rays_reuse: u64,
    pub rays_world_cache: u64,
    pub shadow_rays_world_cache: u64,
    pub rays_debug: u64,
    /// Whether the built-in denoiser ran, which the history figures need.
    pub denoiser_active: bool,
    pub restir: bool,
    /// Average frames of history per pixel, the share with under four, and the
    /// share whose history was thrown away this frame.
    pub mean_history: f32,
    pub short_history_fraction: f32,
    pub rejected_fraction: f32,
    /// Average ReSTIR confidence per pixel, and its cap.
    pub mean_confidence: f32,
    pub confidence_cap: f32,
    /// Average difference of each pixel from its neighbours, relative to their
    /// brightness, before and after denoising.
    pub grain_raw: f32,
    pub grain_denoised: f32,
    /// Share of pixels the ray-traced scene does not match.
    pub not_traced_fraction: f32,
    /// Brightness histograms of the exposed image, before and after denoising.
    /// See [`HISTOGRAM_MIN_LOG2`] and [`HISTOGRAM_STOPS_PER_BIN`].
    pub histogram_raw: Vec<u32>,
    pub histogram_denoised: Vec<u32>,
    /// Samples in the reference, and the image's average error against it
    /// once it has any.
    pub reference_samples: u32,
    pub reference_error: Option<f32>,
}

impl SolariDebugStats {
    pub fn total_rays(&self) -> u64 {
        self.rays_initial + self.rays_reuse + self.rays_world_cache
    }
}

/// The latest [`SolariDebugStats`], shared between the render world (which
/// writes it) and the main world (which reads it). In both worlds once Solari
/// is running.
#[derive(Resource, Clone, Default)]
pub struct SolariDebugOutput(pub Arc<Mutex<Option<SolariDebugStats>>>);

// ── Render world ────────────────────────────────────────────────────────────

/// Must match `DebugSettings` in debug.wesl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DebugUniforms {
    view: u32,
    flags: u32,
    split: f32,
    reference_samples: u32,
}

/// Per-view state of the diagnostics, while the view has a [`SolariDebug`].
#[derive(Component)]
pub struct SolariDebugResources {
    raw: CachedTexture,
    reference: CachedTexture,
    uniforms: Buffer,
    size: UVec2,
    /// Samples in the reference before this frame's. Counted by the node when
    /// it actually traces one, not here in prepare: a frame whose pipelines
    /// are still compiling traces nothing, and counting it anyway averaged a
    /// zero into the reference for every such frame, which came out about a
    /// tenth too dark.
    reference_samples: AtomicU32,
    /// The camera's transform when the reference was started: moving it makes
    /// the reference describe a different picture.
    reference_view: Option<Mat4>,
    /// What this frame's dispatch needs to know, worked out in prepare.
    pub(super) frame: DebugFrame,
}

/// What [`SolariDebugResources`] asks of the node this frame.
#[derive(Clone, Copy, Default)]
pub struct DebugFrame {
    pub view: SolariDebugView,
    pub stats: bool,
    pub reference: bool,
}

impl SolariDebugResources {
    /// The node traced this frame's reference sample.
    pub(super) fn count_reference_sample(&self) {
        self.reference_samples.fetch_add(1, Ordering::Relaxed);
    }
}

impl DebugFrame {
    /// Whether the raw estimate has to be kept before the denoiser overwrites it.
    pub fn needs_raw(self) -> bool {
        self.stats || self.view == SolariDebugView::RawSplit
    }
}

pub fn debug_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "solari_debug_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::ReadWrite),
                texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::ReadWrite),
                uniform_buffer_sized(false, None),
            ),
        ),
    )
}

/// The bind group of the debug passes for this frame: the denoiser's history
/// and moments as the denoiser just wrote them.
pub fn debug_bind_group(
    render_device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    layout: &BindGroupLayoutDescriptor,
    lighting: &SolariLightingResources,
    debug: &SolariDebugResources,
) -> BindGroup {
    let d = &lighting.denoiser;
    let slot = d.write_slot();
    render_device.create_bind_group(
        "solari_debug_bind_group",
        &pipeline_cache.get_bind_group_layout(layout),
        &BindGroupEntries::sequential((
            &d.history_diffuse[slot].default_view,
            &d.moments[slot].default_view,
            &debug.raw.default_view,
            &debug.reference.default_view,
            debug.uniforms.as_entire_binding(),
        )),
    )
}

fn texture(render_device: &RenderDevice, label: &'static str, size: UVec2, format: TextureFormat) -> CachedTexture {
    let texture = render_device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size.x,
            height: size.y,
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
}

/// Make and update each [`SolariDebug`] view's resources, and turn the ray
/// counts on in its lighting uniform. Runs after the lighting's own prepare,
/// which rewrites that uniform with the counts off every frame.
#[allow(clippy::type_complexity)]
pub fn prepare_solari_debug(
    mut views: Query<(
        Entity,
        &ExtractedView,
        &SolariLighting,
        &SolariLightingResources,
        Option<&SolariDebug>,
        Option<&mut SolariDebugResources>,
    )>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut commands: Commands,
) {
    for (entity, view, lighting, resources, debug, debug_resources) in &mut views {
        let Some(debug) = debug else {
            if debug_resources.is_some() {
                commands.entity(entity).remove::<SolariDebugResources>();
            }
            continue;
        };
        let size = resources.view_size;
        // A view new to the diagnostics, or resized, gets new resources,
        // inserted once this frame's state is in them.
        let mut created = None;
        let debug_resources = match debug_resources {
            Some(r) if r.size == size => r.into_inner(),
            _ => created.insert(SolariDebugResources {
                raw: texture(&render_device, "solari_debug_raw", size, TextureFormat::Rgba16Float),
                reference: texture(&render_device, "solari_debug_reference", size, TextureFormat::Rgba32Float),
                uniforms: render_device.create_buffer(&BufferDescriptor {
                    label: Some("solari_debug_uniforms"),
                    size: size_of::<DebugUniforms>() as u64,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                size,
                reference_samples: AtomicU32::new(0),
                reference_view: None,
                frame: DebugFrame::default(),
            }),
        };

        // The reference describes one picture: start it over when the camera
        // moves or a reset is asked for.
        let camera = view.world_from_view.to_matrix();
        if debug.reset_reference || debug_resources.reference_view != Some(camera) {
            debug_resources.reference_samples.store(0, Ordering::Relaxed);
            debug_resources.reference_view = Some(camera);
        }
        let samples = debug_resources.reference_samples.load(Ordering::Relaxed);
        let reference_valid = samples > 0 || debug.reference;

        let denoiser_active = lighting.denoise && resources.denoiser.frame.load(Ordering::Relaxed) > 0;
        let uniforms = DebugUniforms {
            view: debug.view as u32,
            flags: (denoiser_active as u32 * FLAG_DENOISER_ACTIVE) | (reference_valid as u32 * FLAG_REFERENCE_VALID),
            split: debug.split.clamp(0.0, 1.0),
            reference_samples: samples,
        };
        render_queue.write_buffer(&debug_resources.uniforms, 0, bytemuck::bytes_of(&uniforms));
        if debug.stats {
            render_queue.write_buffer(&resources.constants, DEBUG_FLAGS_OFFSET, bytemuck::bytes_of(&DEBUG_COUNT_RAYS));
        }
        debug_resources.frame = DebugFrame {
            view: debug.view,
            stats: debug.stats,
            reference: debug.reference,
        };

        if let Some(created) = created {
            commands.entity(entity).insert(created);
        }
    }
}

// ── Readback ────────────────────────────────────────────────────────────────

struct ReadbackSlot {
    staging: Buffer,
    busy: bool,
    /// The counters, once the map completes.
    result: Arc<Mutex<Option<Vec<u32>>>>,
    /// What the CPU needs to make sense of them, as of the frame they describe.
    context: StatsContext,
}

#[derive(Clone, Copy, Default)]
struct StatsContext {
    view_size: UVec2,
    denoiser_active: bool,
    restir: bool,
    confidence_cap: f32,
    reference_samples: u32,
}

/// The ring of buffers the counters are copied into and read from.
#[derive(Resource, Default)]
pub struct SolariDebugReadback {
    slots: Vec<ReadbackSlot>,
    next: usize,
}

/// After the frame's work is submitted: hand the finished readbacks to the main
/// world, then copy this frame's counters into a free slot, zero them for the
/// next frame, and ask for the copy to be mapped.
///
/// Only the first view gathering statistics is read; the editor asks it of one
/// viewport at a time.
#[allow(clippy::type_complexity)]
pub fn read_back_solari_debug(
    views: Query<(&SolariLighting, &SolariLightingResources, &SolariDebug, &SolariDebugResources)>,
    mut readback: ResMut<SolariDebugReadback>,
    output: Res<SolariDebugOutput>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for slot in &mut readback.slots {
        let words = slot.result.lock().ok().and_then(|mut r| r.take());
        if let Some(words) = words {
            slot.busy = false;
            if words.len() == SLOT_COUNT * DEBUG_LANES {
                if let Ok(mut out) = output.0.lock() {
                    *out = Some(parse_stats(&words, slot.context));
                }
            }
        }
    }

    let Some((lighting, resources, _, debug_resources)) = views.iter().find(|(_, _, d, _)| d.stats) else {
        return;
    };
    if readback.slots.is_empty() {
        readback.slots = (0..READBACK_SLOTS)
            .map(|_| ReadbackSlot {
                staging: render_device.create_buffer(&BufferDescriptor {
                    label: Some("solari_debug_readback"),
                    size: DEBUG_COUNTERS_SIZE,
                    usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                busy: false,
                result: Arc::default(),
                context: StatsContext::default(),
            })
            .collect();
    }

    let mut encoder = render_device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("solari_debug_readback"),
    });
    let free = (0..READBACK_SLOTS)
        .map(|i| (readback.next + i) % READBACK_SLOTS)
        .find(|&i| !readback.slots[i].busy);
    if let Some(i) = free {
        encoder.copy_buffer_to_buffer(&resources.debug_counters, 0, &readback.slots[i].staging, 0, DEBUG_COUNTERS_SIZE);
    }
    // Zeroed every frame, copied or not: counts left in place would be read
    // again later as if they were that frame's.
    encoder.clear_buffer(&resources.debug_counters, 0, None);
    render_queue.submit([encoder.finish()]);

    let Some(i) = free else {
        return;
    };
    readback.next = (i + 1) % READBACK_SLOTS;
    let slot = &mut readback.slots[i];
    slot.busy = true;
    slot.context = StatsContext {
        view_size: resources.view_size,
        denoiser_active: lighting.denoise && resources.denoiser.frame.load(Ordering::Relaxed) > 0,
        restir: resources.reservoirs.is_some(),
        confidence_cap: lighting.confidence_weight_cap,
        // The reference has this many samples once the frame's own is in.
        reference_samples: debug_resources.reference_samples.load(Ordering::Relaxed),
    };
    let buffer = slot.staging.clone();
    let result = slot.result.clone();
    slot.staging.slice(..).map_async(MapMode::Read, move |mapped| {
        let words = mapped.ok().and_then(|()| {
            let words = buffer
                .slice(..)
                .get_mapped_range()
                .ok()
                .map(|data| bytemuck::cast_slice::<u8, u32>(&data).to_vec());
            buffer.unmap();
            words
        });
        if let Ok(mut slot) = result.lock() {
            // A failed map still frees the slot, with nothing to report.
            *slot = Some(words.unwrap_or_default());
        }
    });
}

/// Sum each counter's lanes and turn the sums into the figures shown.
fn parse_stats(words: &[u32], context: StatsContext) -> SolariDebugStats {
    let sum = |slot: usize| -> u64 {
        words[slot * DEBUG_LANES..(slot + 1) * DEBUG_LANES]
            .iter()
            .map(|&w| w as u64)
            .sum()
    };
    let covered = sum(SLOT_COVERED);
    let per_pixel = |total: u64| total as f32 / covered.max(1) as f32;
    let histogram = |first: usize| -> Vec<u32> {
        (first..first + HISTOGRAM_BINS)
            .map(|slot| sum(slot).min(u32::MAX as u64) as u32)
            .collect()
    };
    let reference_pixels = sum(SLOT_REFERENCE_PIXELS);
    SolariDebugStats {
        view_size: context.view_size,
        covered_pixels: covered,
        rays_initial: sum(SLOT_RAYS_INITIAL),
        shadow_rays_initial: sum(SLOT_RAYS_INITIAL + 1),
        rays_reuse: sum(SLOT_RAYS_REUSE),
        shadow_rays_reuse: sum(SLOT_RAYS_REUSE + 1),
        rays_world_cache: sum(SLOT_RAYS_WORLD_CACHE),
        shadow_rays_world_cache: sum(SLOT_RAYS_WORLD_CACHE + 1),
        rays_debug: sum(SLOT_RAYS_DEBUG),
        denoiser_active: context.denoiser_active,
        restir: context.restir,
        mean_history: per_pixel(sum(SLOT_HISTORY_SUM)),
        short_history_fraction: per_pixel(sum(SLOT_HISTORY_SHORT)),
        rejected_fraction: per_pixel(sum(SLOT_REJECTED)),
        mean_confidence: per_pixel(sum(SLOT_CONFIDENCE_SUM)) / 16.0,
        confidence_cap: context.confidence_cap,
        grain_raw: per_pixel(sum(SLOT_GRAIN_RAW)) / 1024.0,
        grain_denoised: per_pixel(sum(SLOT_GRAIN_DENOISED)) / 1024.0,
        not_traced_fraction: per_pixel(sum(SLOT_NOT_TRACED)),
        histogram_raw: histogram(SLOT_HISTOGRAM_RAW),
        histogram_denoised: histogram(SLOT_HISTOGRAM_DENOISED),
        reference_samples: context.reference_samples,
        reference_error: (reference_pixels > 0)
            .then(|| sum(SLOT_REFERENCE_ERROR) as f32 / 1024.0 / reference_pixels as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_are_summed_and_averaged_per_covered_pixel() {
        let mut words = vec![0u32; SLOT_COUNT * DEBUG_LANES];
        // 100 pixels over two lanes, 300 frames of history between them.
        words[SLOT_COVERED * DEBUG_LANES] = 60;
        words[SLOT_COVERED * DEBUG_LANES + 5] = 40;
        words[SLOT_HISTORY_SUM * DEBUG_LANES + 1] = 300;
        words[SLOT_RAYS_INITIAL * DEBUG_LANES + 63] = 7;
        words[(SLOT_HISTOGRAM_DENOISED + 3) * DEBUG_LANES] = 9;
        let stats = parse_stats(&words, StatsContext::default());
        assert_eq!(stats.covered_pixels, 100);
        assert_eq!(stats.mean_history, 3.0);
        assert_eq!(stats.rays_initial, 7);
        assert_eq!(stats.histogram_denoised[3], 9);
        assert_eq!(stats.histogram_raw.len(), HISTOGRAM_BINS);
        assert_eq!(stats.reference_error, None);
    }

    #[test]
    fn the_counter_layout_matches_the_shader() {
        // debug_layout.wesl holds the same numbers; a slot moved on one side
        // only would read every figure from the wrong counter.
        let wesl = include_str!("debug_layout.wesl");
        for (name, value) in [
            ("DEBUG_LANES", DEBUG_LANES),
            ("SLOT_RAYS_INITIAL", SLOT_RAYS_INITIAL),
            ("SLOT_RAYS_REUSE", SLOT_RAYS_REUSE),
            ("SLOT_RAYS_WORLD_CACHE", SLOT_RAYS_WORLD_CACHE),
            ("SLOT_RAYS_DEBUG", SLOT_RAYS_DEBUG),
            ("SLOT_COVERED", SLOT_COVERED),
            ("SLOT_HISTORY_SUM", SLOT_HISTORY_SUM),
            ("SLOT_HISTORY_SHORT", SLOT_HISTORY_SHORT),
            ("SLOT_REJECTED", SLOT_REJECTED),
            ("SLOT_CONFIDENCE_SUM", SLOT_CONFIDENCE_SUM),
            ("SLOT_GRAIN_RAW", SLOT_GRAIN_RAW),
            ("SLOT_GRAIN_DENOISED", SLOT_GRAIN_DENOISED),
            ("SLOT_NOT_TRACED", SLOT_NOT_TRACED),
            ("SLOT_REFERENCE_ERROR", SLOT_REFERENCE_ERROR),
            ("SLOT_REFERENCE_PIXELS", SLOT_REFERENCE_PIXELS),
            ("SLOT_HISTOGRAM_RAW", SLOT_HISTOGRAM_RAW),
            ("SLOT_HISTOGRAM_DENOISED", SLOT_HISTOGRAM_DENOISED),
            ("SLOT_COUNT", SLOT_COUNT),
            ("HISTOGRAM_BINS", HISTOGRAM_BINS),
        ] {
            let line = format!("const {name}: u32 = {value}u;");
            assert!(wesl.contains(&line), "debug_layout.wesl lacks `{line}`");
        }
        assert!(wesl.contains(&format!("const HISTOGRAM_MIN_LOG2: f32 = {HISTOGRAM_MIN_LOG2:?};")));
        assert!(wesl.contains(&format!("const HISTOGRAM_STOPS_PER_BIN: f32 = {HISTOGRAM_STOPS_PER_BIN:?};")));
    }
}
