//! Import overlay for converting and importing 3D models into Renzora projects.
//!
//! Provides a modal overlay triggered by file drops or the asset browser's
//! import button. All models are converted to GLB and placed in the project's
//! assets directory.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod overlay;

use bevy::prelude::*;

#[derive(Default)]
pub struct ImportPlugin;

impl Plugin for ImportPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] ImportPlugin");
        #[cfg(not(target_arch = "wasm32"))]
        {
            _app.init_resource::<overlay::ImportOverlayState>()
                .add_systems(Update, (collect_dropped_files, import_orchestrate_system).chain());
            native::register(_app);
        }
    }
}

/// Drain Bevy's global file-drop events into the overlay's pending list.
/// Persisted via `EventReader` so each drop is processed exactly once.
#[cfg(not(target_arch = "wasm32"))]
fn collect_dropped_files(
    mut events: MessageReader<bevy::window::FileDragAndDrop>,
    mut state: Option<ResMut<overlay::ImportOverlayState>>,
    settings: Option<Res<renzora_editor::EditorSettings>>,
) {
    let Some(state) = state.as_mut() else {
        events.clear();
        return;
    };
    let dropped: Vec<std::path::PathBuf> = events
        .read()
        .filter_map(|ev| match ev {
            bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } => Some(path_buf.clone()),
            _ => None,
        })
        .filter(|p| renzora_import::formats::is_supported(p))
        .collect();
    if dropped.is_empty() {
        return;
    }
    let was_empty = state.pending_files.is_empty();
    for path in &dropped {
        if !state.pending_files.contains(path) {
            state.pending_files.push(path.clone());
        }
    }
    // Auto-detect unit scale from the first file.
    if was_empty && state.settings.scale == 1.0 {
        if let Some(scale) = renzora_import::units::detect_unit_scale(&dropped[0]) {
            state.settings.scale = scale;
        }
    }
    // When the user hasn't opted into silent auto-import, a drop opens the
    // modal so they can confirm; otherwise the orchestrator imports silently.
    let auto_import = settings.map(|s| s.auto_import_on_drop).unwrap_or(true);
    if !auto_import {
        state.visible = true;
    }
}

/// Backend-agnostic orchestration: opens the overlay when the asset browser
/// fires [`ImportRequested`] and runs the silent auto-import path. Global file
/// drops are gathered upstream by [`collect_dropped_files`]. The native
/// (bevy_ui) modal renders the actual UI and polls the worker while visible.
#[cfg(not(target_arch = "wasm32"))]
fn import_orchestrate_system(world: &mut World) {
    let auto_import = world
        .get_resource::<renzora_editor::EditorSettings>()
        .map(|s| s.auto_import_on_drop)
        .unwrap_or(true);

    // Check for ImportRequested marker from the asset browser
    let import_requested = world
        .remove_resource::<renzora::core::ImportRequested>()
        .is_some();
    let requested_target = world.remove_resource::<renzora::core::ImportTargetDir>();

    if import_requested {
        let mut state = world.resource_mut::<overlay::ImportOverlayState>();
        if let Some(ref target) = requested_target {
            state.target_directory = target.0.clone();
        }
        // An explicit Import click always opens the overlay (the user needs
        // the file picker). `auto_import` only governs drag-and-drop.
        state.visible = true;
    }

    // Auto-import path: kick off the worker silently when files are pending
    // and the user has opted into auto-import. Skipped when the overlay is
    // explicitly visible (the user clicked Import and wants to pick files).
    let overlay_visible = world.resource::<overlay::ImportOverlayState>().visible;
    if auto_import && !overlay_visible {
        overlay::poll_import_task(world);

        let should_start = {
            let state = world.resource::<overlay::ImportOverlayState>();
            !state.pending_files.is_empty() && state.active_task.is_none()
        };
        if should_start {
            overlay::run_import(world);
        }

        // Reset idle terminal state so the next drop starts fresh.
        let done = matches!(
            world.resource::<overlay::ImportOverlayState>().progress,
            overlay::ImportProgress::Done(_) | overlay::ImportProgress::Error(_)
        );
        if done {
            let mut state = world.resource_mut::<overlay::ImportOverlayState>();
            state.pending_files.clear();
            state.progress = overlay::ImportProgress::Idle;
            state.log_entries.clear();
        }
    }
}

renzora::add!(ImportPlugin, Editor);
