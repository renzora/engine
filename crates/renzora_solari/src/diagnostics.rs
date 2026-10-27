//! The bridge between the editor's Ray Tracing panel and Solari's diagnostics
//! (`bevy_solari`'s `SolariDebug`, see `realtime/debug.rs` in the fork).
//!
//! The panel writes a [`SolariDiagnosticsRequest`]; this puts the matching
//! `SolariDebug` on the focused viewport's camera, and turns what comes back
//! (the fork's per-frame figures, Bevy's GPU pass timings, the coverage tally
//! the mesh mirror keeps) into [`SolariDiagnostics`] for the panel to show.
//! Both are contract types, so the editor needs nothing from the renderer.
//!
//! It also keeps the viewport rendering long enough for what Solari is doing:
//! every frame while the diagnostics measure or build a reference, and long
//! enough after each change for the built-in denoiser to converge.

use bevy::diagnostic::DiagnosticsStore;
use bevy::prelude::*;
use bevy::solari::realtime::{
    SolariDebug, SolariDebugOutput, SolariDebugView, SolariLighting, HISTOGRAM_MIN_LOG2,
    HISTOGRAM_STOPS_PER_BIN,
};
use renzora::{
    EditorCamera, SolariCoverage, SolariDebugMode, SolariDiagnostics, SolariDiagnosticsRequest,
    SolariFrameStats, ViewportRedrawDemand,
};

/// Frames the viewport keeps rendering after a change while the built-in
/// denoiser is the one running: the most history it averages (32 frames), and
/// a margin for ReSTIR and the world cache to settle under it. The viewport's
/// own default stops at 20, which froze a picture still visibly grainy.
const DENOISER_SETTLE_FRAMES: u32 = 48;

/// Where Bevy's render diagnostics keep the GPU timings of Solari's passes:
/// `render/solari_lighting/<pass>/elapsed_gpu`.
const GPU_TIMING_PREFIX: &str = "render/solari_lighting/";
const GPU_TIMING_SUFFIX: &str = "/elapsed_gpu";

/// The world cache's active cell count, which the lighting node records.
const WORLD_CACHE_CELLS: &str = "render/solari_lighting/world_cache_active_cells_count";

pub(crate) fn register(app: &mut App) {
    app.init_resource::<SolariDiagnostics>()
        .init_resource::<ViewportRedrawDemand>()
        .add_systems(Update, (apply_request, collect_diagnostics, demand_redraw));
}

fn debug_view(mode: SolariDebugMode) -> SolariDebugView {
    match mode {
        SolariDebugMode::None => SolariDebugView::None,
        SolariDebugMode::RawSplit => SolariDebugView::RawSplit,
        SolariDebugMode::HistoryLength => SolariDebugView::HistoryLength,
        SolariDebugMode::Rejected => SolariDebugView::Rejected,
        SolariDebugMode::NoiseEstimate => SolariDebugView::NoiseEstimate,
        SolariDebugMode::ReservoirConfidence => SolariDebugView::ReservoirConfidence,
        SolariDebugMode::WorldCache => SolariDebugView::WorldCache,
        SolariDebugMode::NotTraced => SolariDebugView::NotTraced,
        SolariDebugMode::Reference => SolariDebugView::Reference,
        SolariDebugMode::ReferenceError => SolariDebugView::ReferenceError,
    }
}

/// Put the panel's request on the focused viewport's camera as a
/// `SolariDebug`, and take it off every other camera. The focused camera
/// carries [`EditorCamera`], so the diagnostics follow whichever viewport the
/// user is working in.
fn apply_request(
    mut commands: Commands,
    request: Option<ResMut<SolariDiagnosticsRequest>>,
    focused: Query<Entity, (With<EditorCamera>, With<SolariLighting>)>,
    mut debugged: Query<(Entity, &mut SolariDebug)>,
) {
    let wanted = request.as_deref().copied().filter(SolariDiagnosticsRequest::is_active);
    let target = wanted.and(focused.iter().next());

    for (entity, mut debug) in &mut debugged {
        if Some(entity) != target {
            commands.entity(entity).try_remove::<SolariDebug>();
            continue;
        }
        let Some(wanted) = wanted else { continue };
        let next = SolariDebug {
            view: debug_view(wanted.view),
            split: wanted.split,
            stats: wanted.stats,
            reference: wanted.reference,
            reset_reference: wanted.reset_reference,
        };
        if debug.view != next.view
            || debug.split != next.split
            || debug.stats != next.stats
            || debug.reference != next.reference
            || next.reset_reference
        {
            *debug = next;
        }
    }
    if let (Some(entity), Some(wanted)) = (target, wanted) {
        if !debugged.contains(entity) {
            commands.entity(entity).try_insert(SolariDebug {
                view: debug_view(wanted.view),
                split: wanted.split,
                stats: wanted.stats,
                reference: wanted.reference,
                reset_reference: wanted.reset_reference,
            });
        }
    }
    // A reset is one request: passed on once, then cleared.
    if let Some(mut request) = request {
        if request.reset_reference {
            request.reset_reference = false;
        }
    }
}

/// Gather what the panel shows, while it asks for figures.
fn collect_diagnostics(
    request: Option<Res<SolariDiagnosticsRequest>>,
    output: Option<Res<SolariDebugOutput>>,
    store: Option<Res<DiagnosticsStore>>,
    coverage: Res<CoverageTally>,
    dlss: Query<(), (With<EditorCamera>, crate::dlss::DlssAttached)>,
    mut diagnostics: ResMut<SolariDiagnostics>,
) {
    if !request.is_some_and(|r| r.stats) {
        return;
    }
    if let Some(stats) = output.and_then(|o| o.0.lock().ok().and_then(|mut s| s.take())) {
        diagnostics.frame = Some(SolariFrameStats {
            width: stats.view_size.x,
            height: stats.view_size.y,
            covered_pixels: stats.covered_pixels,
            rays_initial: stats.rays_initial,
            shadow_rays_initial: stats.shadow_rays_initial,
            rays_reuse: stats.rays_reuse,
            shadow_rays_reuse: stats.shadow_rays_reuse,
            rays_world_cache: stats.rays_world_cache,
            shadow_rays_world_cache: stats.shadow_rays_world_cache,
            rays_debug: stats.rays_debug,
            denoiser_active: stats.denoiser_active,
            restir: stats.restir,
            mean_history: stats.mean_history,
            short_history_fraction: stats.short_history_fraction,
            rejected_fraction: stats.rejected_fraction,
            mean_confidence: stats.mean_confidence,
            confidence_cap: stats.confidence_cap,
            grain_raw: stats.grain_raw,
            grain_denoised: stats.grain_denoised,
            not_traced_fraction: stats.not_traced_fraction,
            histogram_raw: stats.histogram_raw,
            histogram_denoised: stats.histogram_denoised,
            histogram_min_log2: HISTOGRAM_MIN_LOG2,
            histogram_stops_per_bin: HISTOGRAM_STOPS_PER_BIN,
            reference_samples: stats.reference_samples,
            reference_error: stats.reference_error,
        });
    }
    if let Some(store) = store {
        let mut gpu_ms: Vec<(String, f64)> = store
            .iter()
            .filter_map(|d| {
                let path = d.path().as_str();
                let pass = path.strip_prefix(GPU_TIMING_PREFIX)?.strip_suffix(GPU_TIMING_SUFFIX)?;
                Some((pass.to_string(), d.smoothed()?))
            })
            .collect();
        gpu_ms.sort_by(|a, b| a.0.cmp(&b.0));
        diagnostics.gpu_ms = gpu_ms;
        diagnostics.world_cache_cells = store
            .iter()
            .find(|d| d.path().as_str() == WORLD_CACHE_CELLS)
            .and_then(|d| d.smoothed());
    }
    diagnostics.coverage = coverage.0;
    diagnostics.dlss_running = !dlss.is_empty();
}

/// Ask the viewport for the rendering Solari needs: every frame while the
/// diagnostics measure or accumulate, and a longer settle after each change
/// while the built-in denoiser runs.
fn demand_redraw(
    request: Option<Res<SolariDiagnosticsRequest>>,
    solari: Query<&SolariLighting>,
    mut demand: ResMut<ViewportRedrawDemand>,
) {
    let measuring = request.is_some_and(|r| r.stats || r.reference);
    let continuous = measuring.then_some("ray tracing diagnostics");
    let settle_frames = if solari.iter().any(|s| s.denoise) {
        DENOISER_SETTLE_FRAMES
    } else {
        0
    };
    if demand.continuous != continuous || demand.settle_frames != settle_frames {
        *demand = ViewportRedrawDemand {
            continuous,
            settle_frames,
        };
    }
}

/// The mesh mirror's coverage tally, kept by `log_solari_coverage` alongside
/// its log line.
#[derive(Resource, Default)]
pub(crate) struct CoverageTally(pub SolariCoverage);
