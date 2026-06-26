//! Loading overlay — the phase between project selection and the editor.
//!
//! Plugins register `LoadingTask`s that the overlay renders with a progress
//! bar. When every task reports complete (`completed >= total`), the state
//! advances to [`SplashState::Editor`][super::SplashState::Editor] and the
//! overlay disappears.

use bevy::prelude::*;

use crate::SplashState;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LoadingTaskHandle(u32);

#[derive(Clone, Debug)]
pub struct LoadingTask {
    pub label: String,
    pub total: u32,
    pub completed: u32,
    pub detail: Option<String>,
}

impl LoadingTask {
    pub fn is_done(&self) -> bool {
        self.completed >= self.total
    }
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            1.0
        } else {
            (self.completed as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }
}

#[derive(Resource, Default)]
pub struct LoadingTasks {
    next_id: u32,
    tasks: Vec<(LoadingTaskHandle, LoadingTask)>,
    pub(crate) min_frames_remaining: u32,
}

/// Real byte totals for the loading bar, populated by `renzora_scene` from the
/// on-disk sizes of the scene's GLB files (loose-file editor loads). Lets the
/// loader show genuine MB-loaded rather than a step count. `total == 0` means no
/// byte data — the UI falls back to the task-count fraction.
#[derive(Resource, Default)]
pub struct LoadingBytes {
    pub loaded: u64,
    pub total: u64,
}

/// Real sub-asset progress: how many of the scene's `.rmip` textures have
/// finished decoding (vs total referenced by the spawned models' materials).
/// Populated by `renzora_scene::tick_texture_progress`. This is the genuine
/// "still decoding" signal — textures are the heavy part of a scene load.
#[derive(Resource, Default)]
pub struct TextureLoadProgress {
    pub loaded: u32,
    pub total: u32,
}

impl LoadingTasks {
    pub fn register(&mut self, label: impl Into<String>, total: u32) -> LoadingTaskHandle {
        let h = LoadingTaskHandle(self.next_id);
        self.next_id += 1;
        self.tasks.push((
            h,
            LoadingTask {
                label: label.into(),
                total,
                completed: 0,
                detail: None,
            },
        ));
        // Minimum display time to prevent flicker when tasks register with zero work.
        self.min_frames_remaining = self.min_frames_remaining.max(30);
        h
    }

    pub fn advance(&mut self, handle: LoadingTaskHandle, by: u32) {
        if let Some(task) = self.task_mut(handle) {
            task.completed = task.completed.saturating_add(by).min(task.total);
        }
    }

    pub fn set_detail(&mut self, handle: LoadingTaskHandle, detail: impl Into<String>) {
        if let Some(task) = self.task_mut(handle) {
            task.detail = Some(detail.into());
        }
    }

    pub fn complete(&mut self, handle: LoadingTaskHandle) {
        if let Some(task) = self.task_mut(handle) {
            task.completed = task.total;
            task.detail = None;
        }
    }

    pub fn tasks(&self) -> &[(LoadingTaskHandle, LoadingTask)] {
        &self.tasks
    }

    pub fn all_done(&self) -> bool {
        self.tasks.iter().all(|(_, t)| t.is_done())
    }

    /// Drop every registered task. Used when transitioning between
    /// loading sessions (e.g. splash → editor, or one editor overlay
    /// session ending so the next can start) — without this, the
    /// fraction calculation would still see the previous session's
    /// completed totals and start the next bar partway full.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// Drop only finished tasks; in-flight ones survive. Lets the editor
    /// overlay tear down its session at the end while still being safe
    /// to call mid-session if anything's still ticking.
    pub fn clear_completed(&mut self) {
        self.tasks.retain(|(_, t)| !t.is_done());
    }

    pub fn tick_and_can_advance(&mut self) -> bool {
        if self.min_frames_remaining > 0 {
            self.min_frames_remaining -= 1;
        }
        self.min_frames_remaining == 0 && self.all_done()
    }

    fn task_mut(&mut self, handle: LoadingTaskHandle) -> Option<&mut LoadingTask> {
        self.tasks
            .iter_mut()
            .find(|(h, _)| *h == handle)
            .map(|(_, t)| t)
    }
}

/// Resource toggled by `renzora_scene::tick_editor_load_progress`.
/// While `true`, the native editor loading overlay (`native_loading`) paints
/// the modal over the editor; while `false` (the steady state) it is hidden.
#[derive(Resource, Default)]
pub struct EditorLoadingOverlayActive(pub bool);

pub(crate) fn auto_advance_to_editor(
    time: Res<Time<bevy::time::Real>>,
    mut tasks: ResMut<LoadingTasks>,
    textures: Res<TextureLoadProgress>,
    mut tex_wait: Local<f32>,
    mut next_state: ResMut<NextState<SplashState>>,
) {
    // First the GLB/scene phase (tasks) must be done.
    if !tasks.tick_and_can_advance() {
        *tex_wait = 0.0;
        return;
    }
    // Then hold the loading screen until the scene's textures have actually
    // finished decoding — they load *after* the GLB spawns and are the real
    // remaining work. Guarded by a timeout so a stuck/failed texture can never
    // hang the editor open.
    const TEXTURE_TIMEOUT: f32 = 12.0;
    *tex_wait += time.delta_secs();
    let textures_pending = textures.total > 0 && textures.loaded < textures.total;
    if textures_pending && *tex_wait < TEXTURE_TIMEOUT {
        return;
    }
    next_state.set(SplashState::Editor);
}

pub(crate) fn log_loading_entered() {
    bevy::log::info!("[loading] entered Loading state");
}
