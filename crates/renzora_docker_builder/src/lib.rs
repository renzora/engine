//! Engine Builder panel — triggers Docker-backed cross-platform builds from the editor.
//!
//! Spawns `docker build` / `docker exec scripts/build-all.sh` on a worker thread,
//! streams stdout/stderr into a Bevy resource, and renders:
//!   * a toolbar with build/stop/clean actions,
//!   * per-target progress bars (platform × {editor, runtime, server}),
//!   * a scrollable log tail.

#[cfg(not(target_arch = "wasm32"))]
mod panel;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(not(target_arch = "wasm32"))]
mod state;

use bevy::prelude::*;

#[derive(Default)]
pub struct DockerBuilderPlugin;

impl Plugin for DockerBuilderPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] DockerBuilderPlugin");
        #[cfg(not(target_arch = "wasm32"))]
        {
            use renzora_editor_framework::{AppEditorExt, SplashState};
            use state::{ActionBridge, DockerBuilderState};

            _app.init_resource::<DockerBuilderState>()
                .init_resource::<ActionBridge>()
                .add_systems(
                    Update,
                    (systems::process_actions, systems::drain_worker)
                        .chain()
                        .run_if(in_state(SplashState::Editor)),
                );

            let bridge = _app
                .world()
                .get_resource::<ActionBridge>()
                .cloned()
                .unwrap_or_default();
            _app.register_panel(panel::DockerBuilderPanel::new(bridge));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod systems {
    use super::runner;
    use super::state::{
        ActionBridge, DockerBuilderState, Stage, TargetStatus, UiAction, WorkerMsg,
    };
    use bevy::prelude::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    pub fn process_actions(
        bridge: Res<ActionBridge>,
        mut state: ResMut<DockerBuilderState>,
    ) {
        let Ok(mut queue) = bridge.actions.lock() else {
            return;
        };
        if queue.is_empty() {
            return;
        }
        let actions: Vec<UiAction> = queue.drain(..).collect();
        drop(queue);

        for action in actions {
            match action {
                UiAction::ClearLogs => {
                    state.logs.clear();
                }
                UiAction::Stop => {
                    if let Some(flag) = state.stop_flag.as_ref() {
                        flag.store(true, Ordering::Relaxed);
                    }
                    state.push_log("[stop requested]".into());
                }
                UiAction::BuildImage => {
                    if state.running {
                        continue;
                    }
                    let repo = match std::env::current_dir() {
                        Ok(p) => p,
                        Err(e) => {
                            state.push_log(format!("cwd error: {}", e));
                            continue;
                        }
                    };
                    let stop = Arc::new(AtomicBool::new(false));
                    let rx = runner::spawn_image_build(repo, stop.clone());
                    state.running = true;
                    state.stage = Stage::BuildingImage;
                    state.rx = Some(Mutex::new(rx));
                    state.stop_flag = Some(stop);
                }
                UiAction::BuildAll => {
                    if state.running {
                        continue;
                    }
                    let repo = match std::env::current_dir() {
                        Ok(p) => p,
                        Err(e) => {
                            state.push_log(format!("cwd error: {}", e));
                            continue;
                        }
                    };
                    state.reset_targets();
                    let stop = Arc::new(AtomicBool::new(false));
                    let rx = runner::spawn_full_build(repo, stop.clone());
                    state.running = true;
                    state.stage = Stage::StartingContainer;
                    state.rx = Some(Mutex::new(rx));
                    state.stop_flag = Some(stop);
                }
                UiAction::CleanCache => {
                    if state.running {
                        continue;
                    }
                    let repo = match std::env::current_dir() {
                        Ok(p) => p,
                        Err(e) => {
                            state.push_log(format!("cwd error: {}", e));
                            continue;
                        }
                    };
                    let rx = runner::spawn_clean(repo);
                    state.running = true;
                    state.stage = Stage::Cleaning;
                    state.rx = Some(Mutex::new(rx));
                    state.stop_flag = Some(Arc::new(AtomicBool::new(false)));
                }
            }
        }
    }

    pub fn drain_worker(mut state: ResMut<DockerBuilderState>) {
        let mut finished = false;
        let mut drained: Vec<WorkerMsg> = Vec::new();

        if let Some(rx_lock) = state.rx.as_ref() {
            if let Ok(rx) = rx_lock.lock() {
                for _ in 0..512 {
                    match rx.try_recv() {
                        Ok(msg) => {
                            if matches!(msg, WorkerMsg::Finished) {
                                finished = true;
                                break;
                            }
                            drained.push(msg);
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            finished = true;
                            break;
                        }
                    }
                }
            }
        }

        for msg in drained {
            apply_msg(&mut state, msg);
        }

        if finished {
            state.running = false;
            state.rx = None;
            state.stop_flag = None;
        }
    }

    fn apply_msg(state: &mut DockerBuilderState, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Stage(stage) => {
                state.stage = stage;
            }
            WorkerMsg::Log(line) => {
                state.push_log(line);
            }
            WorkerMsg::TargetStart(platform, feature) => {
                if let Some(t) = state.target_mut(&platform, &feature) {
                    t.status = TargetStatus::InProgress;
                    t.crates_compiled = 0;
                    t.error = None;
                }
            }
            WorkerMsg::TargetCompileTick(platform, feature) => {
                if let Some(t) = state.target_mut(&platform, &feature) {
                    t.crates_compiled = t.crates_compiled.saturating_add(1);
                }
            }
            WorkerMsg::TargetDone(platform, feature) => {
                if let Some(t) = state.target_mut(&platform, &feature) {
                    t.status = TargetStatus::Done;
                }
            }
            WorkerMsg::TargetFailed(platform, feature, err) => {
                if let Some(t) = state.target_mut(&platform, &feature) {
                    t.status = TargetStatus::Failed;
                    t.error = Some(err);
                }
            }
            WorkerMsg::Finished => { /* handled in drain_worker */ }
        }
    }
}

renzora::add!(DockerBuilderPlugin, Editor);
