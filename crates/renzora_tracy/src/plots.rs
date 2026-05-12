//! Renzora-specific Tracy plots — continuous values graphed alongside
//! zones. Always compiled, but `tracy_client::plot!` is a near-no-op
//! when no Tracy GUI is connected (the `ondemand` feature keeps the
//! client dormant).

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Last, push_plots);
}

fn push_plots(diagnostics: Res<DiagnosticsStore>) {
    if let Some(count) = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.smoothed())
    {
        tracy_client::plot!("renzora.entity_count", count);
    }
    if let Some(ms) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
    {
        tracy_client::plot!("renzora.frame_time_ms", ms);
    }
}
