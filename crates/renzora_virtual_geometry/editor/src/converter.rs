//! The background converter: a queue of models, worked one at a time on the
//! async compute pool.
//!
//! One at a time on purpose. Building holds a model's whole vertex data plus
//! its cluster hierarchy in memory, a few times the GLB's size for a large
//! scene, and a project opening with dozens of models queued should not ask
//! for all of that at once. A large model (Bistro, 2.8 million triangles)
//! takes about five seconds.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

use bevy::asset::AssetPath;
use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use renzora::core::project_files::{is_ignored, FileChange, ProjectFileChanged};
use renzora::core::CurrentProject;
use renzora_rvg::bake::{ensure_current, find_models, Ensured};
use renzora_virtual_geometry::RvgUpdated;

#[derive(Resource, Default)]
pub struct Converter {
    root: Option<PathBuf>,
    scan: Option<Task<Vec<PathBuf>>>,
    queue: VecDeque<PathBuf>,
    queued: HashSet<PathBuf>,
    running: Option<(PathBuf, Task<Result<Ensured, String>>)>,
    built_this_batch: usize,
}

impl Converter {
    /// Queue a model (absolute path). Already queued or running: no-op.
    pub fn enqueue(&mut self, glb: PathBuf) {
        let running = self.running.as_ref().is_some_and(|(p, _)| *p == glb);
        if !running && self.queued.insert(glb.clone()) {
            self.queue.push_back(glb);
        }
    }

    /// Forget everything waiting. A build already running finishes.
    pub(crate) fn clear_queue(&mut self) {
        self.queue.clear();
        self.queued.clear();
    }

    /// Whether a build is in flight, which would write an `.rvg` when it
    /// finishes.
    pub(crate) fn is_running(&self) -> bool {
        self.running.is_some()
    }
}

pub(crate) fn build(app: &mut App) {
    // Registered by the runtime plugin too; repeated here so the converter
    // still works in a build that leaves the renderer out.
    app.add_message::<RvgUpdated>()
        .init_resource::<Converter>()
        .add_systems(Update, (follow_project, watch, finish_scan, pump).chain());
}

fn enabled(project: &CurrentProject) -> bool {
    project.config.rendering.virtual_geometry
}

/// Look at every model when a project opens, or when the switch turns on.
fn follow_project(mut converter: ResMut<Converter>, project: Option<Res<CurrentProject>>) {
    let Some(project) = project else {
        return;
    };
    let wanted = enabled(&project).then(|| project.path.clone());
    if converter.root == wanted {
        return;
    }
    // A different project, or the switch flipped: whatever was queued belongs
    // to the old state. A build already running finishes on its own.
    converter.queue.clear();
    converter.queued.clear();
    converter.root = wanted.clone();
    converter.scan = wanted.map(|root| {
        AsyncComputeTaskPool::get().spawn(async move { find_models(&root, is_ignored) })
    });
}

fn finish_scan(mut converter: ResMut<Converter>) {
    let Some(task) = converter.scan.as_mut() else {
        return;
    };
    let Some(models) = block_on(future::poll_once(task)) else {
        return;
    };
    converter.scan = None;
    info!("[virtual geometry] checking {} model(s) for current virtual geometry", models.len());
    for model in models {
        converter.enqueue(model);
    }
}

/// A model added or re-imported while the project is open.
fn watch(mut converter: ResMut<Converter>, mut events: MessageReader<ProjectFileChanged>) {
    for event in events.read() {
        let Some(root) = converter.root.clone() else {
            continue;
        };
        if event.root != root || event.is_dir || !event.has_extension("glb") {
            continue;
        }
        if matches!(event.change, FileChange::Added | FileChange::Modified | FileChange::Renamed { .. }) {
            converter.enqueue(event.path.clone());
        }
    }
}

/// Collect a finished build and start the next. Nothing new starts while
/// models are being converted to `.rvg` (see `migrate`): a sidecar written
/// over a model just converted would leave it with no scene.
fn pump(
    mut converter: ResMut<Converter>,
    migration: Res<crate::migrate::Migration>,
    mut updated: MessageWriter<RvgUpdated>,
) {
    if let Some((model, task)) = converter.running.as_mut() {
        let Some(result) = block_on(future::poll_once(task)) else {
            return;
        };
        let model = model.clone();
        converter.running = None;
        let relative = converter
            .root
            .as_ref()
            .and_then(|root| model.strip_prefix(root).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"));
        let name = relative.clone().unwrap_or_else(|| model.display().to_string());
        match result {
            Ok(Ensured::UpToDate) => {}
            Ok(Ensured::Built {
                primitives,
                skipped,
                bytes,
                seconds,
            }) => {
                converter.built_this_batch += 1;
                info!(
                    "[virtual geometry] built {name}: {primitives} mesh(es), {:.1} MB, {seconds:.1}s",
                    bytes as f64 / 1e6
                );
                for s in skipped {
                    info!(
                        "[virtual geometry] {name}: mesh {} primitive {} stays ordinary geometry ({})",
                        s.mesh_index, s.primitive_index, s.reason
                    );
                }
                if let Some(relative) = relative {
                    updated.write(RvgUpdated {
                        model: AssetPath::from(relative),
                    });
                }
            }
            Ok(Ensured::NothingToConvert { .. }) | Ok(Ensured::Model) => {}
            Err(e) => warn!("[virtual geometry] could not convert {name}: {e}"),
        }
    }

    if migration.busy() {
        return;
    }
    let Some(next) = converter.queue.pop_front() else {
        if converter.built_this_batch > 0 {
            info!(
                "[virtual geometry] up to date ({} model(s) converted)",
                converter.built_this_batch
            );
            converter.built_this_batch = 0;
        }
        return;
    };
    converter.queued.remove(&next);
    let path = next.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move { ensure_current(&path) });
    converter.running = Some((next, task));
}
