//! Import overlay for bringing external assets into Renzora projects.
//!
//! Provides a modal overlay triggered by file drops or the asset browser's
//! import button. **3D models** are converted to GLB; **every other permitted
//! kind** (images, audio, `.bsn` scenes, `.particle`, `.material`, fonts,
//! scripts) is copied verbatim into the destination folder. See [`kinds`] for
//! the classification that routes each file.

#[cfg(not(target_arch = "wasm32"))]
pub mod kinds;
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
            use renzora::core::RenzoraShellExt;
            _app.init_resource::<overlay::ImportOverlayState>()
                .init_resource::<renzora::core::FileDragHovering>()
                .add_systems(Update, (collect_dropped_files, import_orchestrate_system).chain());
            native::register(_app);
            // A left-side status-bar item showing live import progress. Its
            // `render` runs each frame, so the `[done/total]` + phase updates as
            // the worker advances; it draws nothing while idle.
            _app.register_shell_status_item(renzora::core::ShellStatusItem {
                id: "import-progress",
                align: renzora::core::ShellStatusAlign::Left,
                order: -10,
                render: import_status_segments,
            });
        }
    }
}

/// Drain Bevy's global file-drop events. Handles three things off the same
/// event stream:
///
/// * **Hover feedback** — `HoveredFile` / `HoveredFileCanceled` flip
///   [`FileDragHovering`](renzora::core::FileDragHovering) so the asset browser
///   can highlight itself as the drop target. The flag is a resource (not
///   recomputed per frame) because winit fires `HoveredFile` once on entry, then
///   nothing until the drop/cancel.
/// * **Target folder** — a drop lands in the folder the asset browser is showing
///   ([`AssetBrowserCwd`](renzora::core::AssetBrowserCwd)), not the importer's
///   last-used/default target. This is what fixes images silently going to
///   `assets/models`.
/// * **Queueing** — importable dropped files are appended to the pending list;
///   `MessageReader` makes each event process exactly once.
#[cfg(not(target_arch = "wasm32"))]
fn collect_dropped_files(
    mut events: MessageReader<bevy::window::FileDragAndDrop>,
    mut state: Option<ResMut<overlay::ImportOverlayState>>,
    mut hovering: Option<ResMut<renzora::core::FileDragHovering>>,
    mut scroll_req: Option<ResMut<renzora::core::AssetDropScrollRequest>>,
    cwd: Option<Res<renzora::core::AssetBrowserCwd>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
) {
    use bevy::window::FileDragAndDrop;

    let mut dropped: Vec<std::path::PathBuf> = Vec::new();
    let mut hover_now: Option<bool> = None;
    for ev in events.read() {
        match ev {
            FileDragAndDrop::HoveredFile { .. } => hover_now = Some(true),
            FileDragAndDrop::HoveredFileCanceled { .. } => hover_now = Some(false),
            FileDragAndDrop::DroppedFile { path_buf, .. } => {
                hover_now = Some(false);
                if crate::kinds::is_importable(path_buf) {
                    dropped.push(path_buf.clone());
                }
            }
        }
    }

    // Publish hover state even when there are no importable files, so the
    // highlight tracks any OS drag over the window.
    if let (Some(h), Some(v)) = (hovering.as_mut(), hover_now) {
        if h.0 != v {
            h.0 = v;
        }
    }

    let Some(state) = state.as_mut() else {
        return;
    };
    if dropped.is_empty() {
        return;
    }

    // Ask the asset browser to scroll its grid to the newly-imported files.
    if let Some(req) = scroll_req.as_mut() {
        req.0 = true;
    }

    // Land the drop in the folder the browser is showing. `Some("")` = project
    // root (a valid target); `None` = no browser, so keep the current target.
    if let Some(dir) = cwd.and_then(|c| c.0.clone()) {
        state.target_directory = dir;
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
        .get_resource::<renzora_editor_framework::EditorSettings>()
        .map(|s| s.auto_import_on_drop)
        .unwrap_or(true);

    // Check for ImportRequested marker from the asset browser
    let import_requested = world
        .remove_resource::<renzora::core::ImportRequested>()
        .is_some();
    let requested_target = world.remove_resource::<renzora::core::ImportTargetDir>();

    if import_requested {
        if let Some(ref target) = requested_target {
            world.resource_mut::<overlay::ImportOverlayState>().target_directory =
                target.0.clone();
        }
        // New workflow: an explicit Import click opens the **OS file picker
        // first**, then shows the overlay pre-loaded with the chosen files —
        // rather than opening an empty overlay the user then has to Browse from.
        // The picker is filtered to every importable kind (models + copyable
        // assets). If the user cancels but files are already queued (e.g. from a
        // prior drop), we still surface the overlay so those aren't stranded.
        let picked = native::pick_and_queue_files(world);
        let has_pending = !world
            .resource::<overlay::ImportOverlayState>()
            .pending_files
            .is_empty();
        if picked || has_pending {
            world.resource_mut::<overlay::ImportOverlayState>().visible = true;
        }
    }

    // Auto-import path: kick off the worker silently when files are pending
    // and the user has opted into auto-import. Skipped when the overlay is
    // explicitly visible (the user clicked Import and wants to pick files).
    let overlay_visible = world.resource::<overlay::ImportOverlayState>().visible;
    // An explicit overlay → toast import owns its own polling + terminal
    // cleanup; staying out keeps the auto-import path from wiping the toast's
    // Done/Error message the moment it finishes.
    let toast_active = world.resource::<overlay::ImportOverlayState>().toast_active;
    if auto_import && !overlay_visible && !toast_active {
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

/// Status-bar segments for the left side: a spinner-style icon + `[done/total]`
/// progress while an import runs, and nothing at all otherwise (idle / done /
/// error are conveyed by the corner toast). Runs every frame via the shell's
/// status registry, so the label tracks the worker live.
#[cfg(not(target_arch = "wasm32"))]
fn import_status_segments(world: &World) -> Vec<renzora::core::ShellStatusSegment> {
    let Some(state) = world.get_resource::<overlay::ImportOverlayState>() else {
        return Vec::new();
    };
    if let overlay::ImportProgress::Working { current, total, label } = &state.progress {
        let (r, g, b) = renzora_ember::theme::accent();
        let text = if label.is_empty() {
            format!("Importing {}/{}", current, total)
        } else {
            format!("Importing [{}/{}] {}", current, total, label)
        };
        vec![renzora::core::ShellStatusSegment::new("circle-notch", text, [r, g, b])]
    } else {
        Vec::new()
    }
}

renzora::add!(ImportPlugin, Editor);
