//! Tracy profiler bridge — distribution plugin.
//!
//! Streams live engine telemetry to a running [Tracy] profiler over Tracy's
//! native protocol: a frame mark per app frame, plus every Bevy diagnostic —
//! frame time, FPS, entity count, per-render-pass GPU/CPU span times, and
//! system CPU/memory where the platform supports it — as a named Tracy plot.
//!
//! [Tracy]: https://github.com/wolfpld/tracy

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use tracy_client::{Client, PlotName};

/// Keeps the Tracy client alive for the app's lifetime — it's refcounted,
/// and dropping the last handle would tear the connection down.
#[derive(Resource)]
struct TracyClient(Client);

/// Tracy `PlotName`s require `'static` storage, but diagnostic paths are
/// dynamic strings — each unique path is leaked once and cached here. The
/// set is small and stable (a few dozen paths), so the leak is bounded.
#[derive(Resource, Default)]
struct PlotNames(HashMap<String, PlotName>);

#[derive(Default)]
pub struct TracyPlugin;

impl Plugin for TracyPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TracyPlugin (Tracy profiler bridge)");
        app.insert_resource(TracyClient(Client::start()))
            .init_resource::<PlotNames>();

        // Make sure the standard diagnostic sources exist. The editor's
        // debugger plugin adds some of these already (duplicates panic), and
        // a bare exported game has none of them.
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        if !app.is_plugin_added::<EntityCountDiagnosticsPlugin>() {
            app.add_plugins(EntityCountDiagnosticsPlugin::default());
        }
        if !app.is_plugin_added::<SystemInformationDiagnosticsPlugin>() {
            app.add_plugins(SystemInformationDiagnosticsPlugin);
        }
        // Per-render-pass GPU/CPU span timings (`render/<pass>/elapsed_*`).
        if !app.is_plugin_added::<bevy::render::diagnostic::RenderDiagnosticsPlugin>() {
            app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
        }

        // `Last`, chained: plots carry this frame's values, then the frame
        // mark closes the frame on Tracy's timeline.
        app.add_systems(Last, (plot_diagnostics, frame_mark).chain());
    }
}

fn frame_mark(client: Res<TracyClient>) {
    client.0.frame_mark();
}

fn plot_diagnostics(
    client: Res<TracyClient>,
    store: Res<DiagnosticsStore>,
    mut names: ResMut<PlotNames>,
) {
    for diag in store.iter() {
        let Some(value) = diag.value() else { continue };
        if !value.is_finite() {
            continue;
        }
        let path = diag.path().as_str();
        let name = names
            .0
            .entry(path.to_string())
            .or_insert_with(|| PlotName::new_leak(path.to_string()));
        client.0.plot(*name, value);
    }
}

renzora::add!(TracyPlugin);
