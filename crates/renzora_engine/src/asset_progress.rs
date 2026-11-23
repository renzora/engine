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

// Only for `PendingMeshInstanceRehydrate`, which is itself `gltf`-gated.
#[cfg(feature = "gltf")]
use crate::scene_io;
use crate::Vfs;
use renzora_scene_api::MeshInstanceData;

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

/// After every counted thing is done, how long nothing more may finish
/// loading before the scene counts as loaded. Textures and shaders arrive one
/// after another for a while after the last model, and nothing counts those
/// in flight; a quiet spell is the sign they have stopped.
pub const SETTLE_SECONDS: f64 = 0.5;

/// How long a scene load can be held before the log says what it is still
/// waiting for. Not a limit: a scene takes as long as it takes, and a loading
/// screen that came down early would show it unfinished. The line is there so
/// a load that never ends (a material whose file is missing) can be found.
pub const SLOW_LOAD_SECONDS: f64 = 30.0;

/// What [`tick_asset_load_progress`] remembers of the scene load it is holding
/// open.
#[derive(Default)]
pub struct LoadHold {
    /// A scene loaded and is not ready to be seen yet.
    active: bool,
    started: f64,
    /// When something last finished loading.
    last_activity: f64,
    /// Whether the slow-load line has been written for this load.
    reported_slow: bool,
}

/// What still has to happen after a scene's models are in, before it is ready
/// to be seen.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SceneSettling<'w, 's> {
    scene: Option<Res<'w, crate::scene_io::SceneLoadState>>,
    work: Option<Res<'w, renzora_platform_api::SceneLoadWork>>,
    unresolved_materials: Query<'w, 's, (), (With<renzora_material_api::MaterialRef>, Without<renzora_material_api::MaterialResolved>)>,
    // Optional, not `MessageReader`s: a headless server registers neither
    // asset type, and a reader of an unregistered message panics.
    images: Option<Res<'w, Messages<AssetEvent<Image>>>>,
    shaders: Option<Res<'w, Messages<AssetEvent<bevy::shader::Shader>>>>,
    image_cursor: Local<'s, bevy::ecs::message::MessageCursor<AssetEvent<Image>>>,
    shader_cursor: Local<'s, bevy::ecs::message::MessageCursor<AssetEvent<bevy::shader::Shader>>>,
}

impl SceneSettling<'_, '_> {
    /// Whether a scene load is still settling at `now`, starting a hold when a
    /// scene has just loaded and ending it when everything is in.
    ///
    /// Only a scene load starts one. Chunks streaming in as the player drives,
    /// or a texture loaded mid-game, extend nothing, so a loading screen never
    /// comes back over a game in progress.
    fn holding(&mut self, hold: &mut LoadHold, now: f64) -> bool {
        // Scene loading is synchronous: a load leaves the phase at `Ready` in
        // the frame it happens, so the resource changing to it is the signal.
        let loaded = self
            .scene
            .as_ref()
            .is_some_and(|s| s.is_changed() && s.phase == crate::scene_io::SceneLoadPhase::Ready);
        if loaded {
            *hold = LoadHold { active: true, started: now, last_activity: now, reported_slow: false };
        }
        let mut activity = 0;
        if let Some(images) = self.images.as_deref() {
            activity += self
                .image_cursor
                .read(images)
                .filter(|e| matches!(e, AssetEvent::LoadedWithDependencies { .. } | AssetEvent::Modified { .. }))
                .count();
        }
        if let Some(shaders) = self.shaders.as_deref() {
            activity += self
                .shader_cursor
                .read(shaders)
                .filter(|e| matches!(e, AssetEvent::LoadedWithDependencies { .. } | AssetEvent::Modified { .. }))
                .count();
        }
        if !hold.active {
            return false;
        }
        let pending = self.work.as_ref().map_or(0, |w| w.total()) + self.unresolved_materials.iter().count() as u32;
        // Work still under way counts as activity, so the quiet spell is timed
        // from when the last of it finished, not from the last texture.
        if activity > 0 || pending > 0 {
            hold.last_activity = now;
        }
        let settling = now - hold.last_activity < SETTLE_SECONDS;
        if !settling {
            hold.active = false;
            info!("[load] scene ready {:.1}s after it loaded", now - hold.started);
        } else if now - hold.started > SLOW_LOAD_SECONDS && !hold.reported_slow {
            hold.reported_slow = true;
            warn!(
                "[load] the scene is still loading after {SLOW_LOAD_SECONDS}s: {} still building, {} materials unresolved",
                self.work.as_ref().map_or(0, |w| w.total()),
                self.unresolved_materials.iter().count()
            );
        }
        hold.active
    }
}

/// Refresh [`AssetLoadProgress`] from current scene state.
///
/// Runs every `Update`. Counts files via `MeshInstanceData` + the
/// `PendingMeshInstanceRehydrate` marker; computes byte sums by looking
/// up each entity's `model_path` in the rpak index.
///
/// Then, after a scene loads, holds the state at `Loading` until the rest of
/// it is in too ([`SceneSettling`]): the terrain built, every material
/// resolved, and textures and shaders stopped arriving. Counting only model
/// files, a loading screen came down over a level still filling in.
pub fn tick_asset_load_progress(
    instances: Query<&MeshInstanceData>,
    // Only models loaded through bevy_gltf are ever "pending" — see the `gltf`
    // feature. A project built from engine primitives has none.
    #[cfg(feature = "gltf")]
    pending: Query<&MeshInstanceData, With<scene_io::PendingMeshInstanceRehydrate>>,
    vfs: Option<Res<Vfs>>,
    time: Res<Time>,
    mut progress: ResMut<AssetLoadProgress>,
    mut settling: SceneSettling,
    mut hold: Local<LoadHold>,
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

    // `mut` is only exercised by the glTF-pending loop below; without models
    // nothing is ever pending, so the three stay at their initial values.
    #[cfg_attr(not(feature = "gltf"), allow(unused_mut))]
    let mut pending_files: u32 = 0;
    #[cfg_attr(not(feature = "gltf"), allow(unused_mut))]
    let mut pending_bytes: u64 = 0;
    #[cfg_attr(not(feature = "gltf"), allow(unused_mut))]
    let mut current: Option<String> = None;
    #[cfg(feature = "gltf")]
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
    let holding = settling.holding(&mut hold, now_secs);
    let busy = pending_files > 0 || holding;
    let next_state = match progress.state {
        LoadProgressState::Idle => {
            if total_files == 0 && !holding {
                LoadProgressState::Idle
            } else if busy {
                progress.started_at = Some(now_secs);
                LoadProgressState::Loading
            } else {
                progress.started_at = Some(now_secs);
                LoadProgressState::Done
            }
        }
        LoadProgressState::Loading => {
            if !busy {
                LoadProgressState::Done
            } else {
                LoadProgressState::Loading
            }
        }
        LoadProgressState::Done => {
            if busy {
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

/// No-op stand-in when scripting is stripped — the bridge exists only so
/// scripts can call `asset_progress()`, and there are none.
#[cfg(not(feature = "scripting"))]
pub fn publish_asset_progress_to_bridge() {}

/// Mirror [`AssetLoadProgress`] into the scripting crate's
/// `AssetProgressBridge` so `asset_progress()` reads can find it without
/// the scripting crate depending on `renzora_engine`.
///
/// Runs every `Update` after [`tick_asset_load_progress`].
#[cfg(feature = "scripting")]
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

/// No-op stand-in when scripting is stripped; see
/// [`publish_asset_progress_to_bridge`].
#[cfg(not(feature = "scripting"))]
pub fn publish_scene_load_to_bridge() {}

/// Mirror [`crate::scene_io::SceneLoadState`] into the scripting crate's
/// `SceneLoadBridge` so `scene_load_state()` reads can find it.
///
/// Separate from [`publish_asset_progress_to_bridge`] because the two track
/// different things: this is how far through *spawning the scene* we are,
/// that one is how many of its **models** have finished loading. A scene
/// reaches `ready` while its meshes are still streaming in.
#[cfg(feature = "scripting")]
pub fn publish_scene_load_to_bridge(
    state: Option<Res<crate::scene_io::SceneLoadState>>,
    bridge: Option<ResMut<renzora_scripting::SceneLoadBridge>>,
) {
    use crate::scene_io::SceneLoadPhase;

    let (Some(state), Some(mut bridge)) = (state, bridge) else {
        return;
    };
    let phase = match state.phase {
        SceneLoadPhase::Idle => "idle",
        SceneLoadPhase::Loading => "loading",
        SceneLoadPhase::Ready => "ready",
        SceneLoadPhase::Failed => "failed",
    };
    bridge.snapshot = Some(renzora_scripting::SceneLoadSnapshot {
        phase,
        current_path: state.current_path.clone(),
        progress: state.progress,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_io::{SceneLoadPhase, SceneLoadState};
    use bevy::time::TimeUpdateStrategy;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)))
            .init_resource::<AssetLoadProgress>()
            .init_resource::<SceneLoadState>()
            .init_resource::<renzora_platform_api::SceneLoadWork>()
            .add_systems(Update, tick_asset_load_progress);
        app.update();
        app
    }

    fn load_scene(app: &mut App) {
        app.world_mut().resource_mut::<SceneLoadState>().phase = SceneLoadPhase::Ready;
    }

    fn state(app: &App) -> LoadProgressState {
        app.world().resource::<AssetLoadProgress>().state
    }

    /// A scene loads with its terrain still building: loading holds until the
    /// terrain is done and a settle period has passed, then ends.
    #[test]
    fn loading_waits_for_reported_work_and_a_quiet_spell() {
        let mut app = app();
        app.world_mut().resource_mut::<renzora_platform_api::SceneLoadWork>().set("terrain", 40);
        load_scene(&mut app);
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(state(&app), LoadProgressState::Loading, "terrain still building");

        app.world_mut().resource_mut::<renzora_platform_api::SceneLoadWork>().set("terrain", 0);
        app.update();
        assert_eq!(state(&app), LoadProgressState::Loading, "settling");
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(state(&app), LoadProgressState::Done);
    }

    /// Work reported mid-game, terrain streaming in as the player drives,
    /// does not bring loading back.
    #[test]
    fn work_without_a_scene_load_does_not_start_loading() {
        let mut app = app();
        load_scene(&mut app);
        for _ in 0..10 {
            app.update();
        }
        let before = state(&app);
        assert_ne!(before, LoadProgressState::Loading);
        app.world_mut().resource_mut::<renzora_platform_api::SceneLoadWork>().set("terrain", 5);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(state(&app), before);
    }

    /// A slow scene is waited for, however long it takes.
    #[test]
    fn a_slow_scene_is_waited_for() {
        let mut app = app();
        app.world_mut().resource_mut::<renzora_platform_api::SceneLoadWork>().set("terrain", 1);
        load_scene(&mut app);
        for _ in 0..((SLOW_LOAD_SECONDS * 10.0) as usize + 50) {
            app.update();
        }
        assert_eq!(state(&app), LoadProgressState::Loading);
    }
}
