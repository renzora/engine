//! Runtime asset-load progress — exposed to scripts so a boot scene can
//! drive a loading bar against actual load state.
//!
//! Every frame, [`tick_asset_load_progress`] walks the scene's
//! `MeshInstanceData` entities and counts how many still carry
//! `PendingMeshInstanceRehydrate` (i.e. haven't had their `Gltf` asset
//! finish loading). The pending paths are looked up in the rpak index to
//! compute byte-level totals; without an rpak the byte counts stay zero
//! and the script can fall back to the file-count ratio.
//!
//! The single [`AssetLoadProgress`] resource is the source of truth.
//! Scripts read it through the `asset_progress()` Lua/Rhai binding (the
//! scripting crate copies it into a thread-local before each script
//! tick — see `renzora_scripting::asset_progress_handler`).

use bevy::prelude::*;

use crate::scene_io;
use crate::Vfs;
use renzora::MeshInstanceData;

/// Lifecycle state for the asset-load progress tracker.
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadProgressState {
    /// Nothing pending — either we never started loading or everything
    /// from the last load completed and the tracker is at rest.
    #[default]
    Idle,
    /// At least one asset is still loading.
    Loading,
    /// Every tracked asset finished. Holds for one frame after the last
    /// pending entity clears so scripts polling on `on_update` get exactly
    /// one tick where they can react to "loading just finished" before
    /// the next scene swap drops us back into `Loading`.
    Done,
}

/// Snapshot of asset-load progress, refreshed each frame by
/// [`tick_asset_load_progress`].
#[derive(Resource, Default, Clone, Debug)]
pub struct AssetLoadProgress {
    pub state: LoadProgressState,
    /// Total `MeshInstanceData` entities in the scene with a `model_path`.
    pub total_files: u32,
    /// Files whose asset has finished loading (no `PendingMeshInstanceRehydrate`).
    pub loaded_files: u32,
    /// Sum of compressed-size for every tracked file, looked up in the
    /// rpak index. Zero when no rpak is mounted (editor or `--project`
    /// runs) — script should fall back to file counts.
    pub total_bytes: u64,
    /// Sum of compressed-size for files that finished loading.
    pub loaded_bytes: u64,
    /// Path of the most recently observed pending file. Useful as
    /// "currently loading: X" UI text.
    pub current_path: Option<String>,
    /// Wall-clock seconds since the tracker last entered `Loading`.
    pub elapsed_secs: f32,
    /// Wall-clock seconds at which `Loading` started. Used to compute
    /// `elapsed_secs`. Internal — scripts should read `elapsed_secs`.
    started_at: Option<f64>,
}

impl AssetLoadProgress {
    /// Best-effort fraction in `[0.0, 1.0]`. Prefers byte-based progress
    /// when an rpak is mounted (smoother across mixed-size assets);
    /// falls back to file-count when bytes are unavailable.
    pub fn fraction(&self) -> f32 {
        if self.total_bytes > 0 {
            (self.loaded_bytes as f32 / self.total_bytes as f32).clamp(0.0, 1.0)
        } else if self.total_files > 0 {
            (self.loaded_files as f32 / self.total_files as f32).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.state, LoadProgressState::Idle)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, LoadProgressState::Loading)
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state, LoadProgressState::Done)
    }
}

/// Refresh [`AssetLoadProgress`] from current scene state.
///
/// Runs every `Update`. Counts files via `MeshInstanceData` + the
/// `PendingMeshInstanceRehydrate` marker; computes byte sums by looking
/// up each entity's `model_path` in the rpak index.
pub fn tick_asset_load_progress(
    instances: Query<&MeshInstanceData>,
    // glTF model loading is `render_3d`-only — a 2D game has no models pending.
    #[cfg(feature = "render_3d")]
    pending: Query<&MeshInstanceData, With<scene_io::PendingMeshInstanceRehydrate>>,
    vfs: Option<Res<Vfs>>,
    time: Res<Time>,
    mut progress: ResMut<AssetLoadProgress>,
) {
    // Count files. Only consider entities whose `model_path` is set —
    // primitives without an external mesh have nothing to load.
    let mut total_files: u32 = 0;
    let mut total_bytes: u64 = 0;
    let archive = vfs.as_ref().and_then(|v| v.archive());

    for data in instances.iter() {
        if let Some(path) = data.model_path.as_deref() {
            total_files += 1;
            if let Some(archive) = archive {
                if let Some(entry) = archive.entry(path) {
                    total_bytes += entry.compressed_size;
                }
            }
        }
    }

    let mut pending_files: u32 = 0;
    let mut pending_bytes: u64 = 0;
    let mut current: Option<String> = None;
    #[cfg(feature = "render_3d")]
    for data in pending.iter() {
        if let Some(path) = data.model_path.as_deref() {
            pending_files += 1;
            if let Some(archive) = archive {
                if let Some(entry) = archive.entry(path) {
                    pending_bytes += entry.compressed_size;
                }
            }
            if current.is_none() {
                current = Some(path.to_string());
            }
        }
    }

    let loaded_files = total_files.saturating_sub(pending_files);
    let loaded_bytes = total_bytes.saturating_sub(pending_bytes);

    // State machine: Idle ↔ Loading ↔ Done.
    let now_secs = time.elapsed_secs_f64();
    let next_state = match progress.state {
        LoadProgressState::Idle => {
            if total_files == 0 {
                LoadProgressState::Idle
            } else if pending_files > 0 {
                progress.started_at = Some(now_secs);
                LoadProgressState::Loading
            } else {
                progress.started_at = Some(now_secs);
                LoadProgressState::Done
            }
        }
        LoadProgressState::Loading => {
            if pending_files == 0 {
                LoadProgressState::Done
            } else {
                LoadProgressState::Loading
            }
        }
        LoadProgressState::Done => {
            if pending_files > 0 {
                progress.started_at = Some(now_secs);
                LoadProgressState::Loading
            } else {
                LoadProgressState::Done
            }
        }
    };

    let elapsed = progress
        .started_at
        .map(|s| (now_secs - s).max(0.0) as f32)
        .unwrap_or(0.0);

    *progress = AssetLoadProgress {
        state: next_state,
        total_files,
        loaded_files,
        total_bytes,
        loaded_bytes,
        current_path: current,
        elapsed_secs: elapsed,
        started_at: progress.started_at,
    };
}

/// Mirror [`AssetLoadProgress`] into the scripting crate's
/// `AssetProgressBridge` so `asset_progress()` reads can find it without
/// the scripting crate depending on `renzora_engine`.
///
/// Runs every `Update` after [`tick_asset_load_progress`].
pub fn publish_asset_progress_to_bridge(
    progress: Res<AssetLoadProgress>,
    mut bridge: Option<ResMut<renzora_scripting::AssetProgressBridge>>,
) {
    let Some(ref mut bridge) = bridge else {
        return;
    };
    let state = match progress.state {
        LoadProgressState::Idle => "idle",
        LoadProgressState::Loading => "loading",
        LoadProgressState::Done => "done",
    };
    bridge.snapshot = Some(renzora_scripting::AssetProgressSnapshot {
        state,
        total_files: progress.total_files,
        loaded_files: progress.loaded_files,
        total_bytes: progress.total_bytes,
        loaded_bytes: progress.loaded_bytes,
        current_path: progress.current_path.clone(),
        elapsed_secs: progress.elapsed_secs,
        fraction: progress.fraction(),
    });
}
