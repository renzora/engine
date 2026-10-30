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
use renzora::job_args::{RvgBake, RvgBakeOutcome};
use renzora_rvg::bake::{ensure_current, find_models, Ensured};
use renzora_virtual_geometry::RvgUpdated;

/// A build in flight: on this process's compute pool, or in the companion.
///
/// The companion when there is one. A build holds a model's whole vertex data
/// and cluster hierarchy, a few times the GLB's size, for seconds; in the
/// companion that memory and CPU are another process's, scheduled below the
/// editor's frame.
enum Pending {
    Local(Task<Result<Ensured, String>>),
    /// The job id, and its answer once it arrives.
    Remote(renzora::CompanionJobId, Option<Result<String, String>>),
}

#[derive(Resource, Default)]
pub struct Converter {
    root: Option<PathBuf>,
    scan: Option<Task<Vec<PathBuf>>>,
    queue: VecDeque<PathBuf>,
    queued: HashSet<PathBuf>,
    running: Option<(PathBuf, Pending)>,
    built_this_batch: usize,
}

/// What a bake did, the same shape whichever process ran it.
pub(crate) fn outcome_of(ensured: Ensured) -> RvgBakeOutcome {
    match ensured {
        Ensured::Built { primitives, skipped, bytes, seconds } => RvgBakeOutcome {
            built: true,
            primitives,
            bytes,
            seconds,
            skipped: skipped
                .into_iter()
                .map(|s| format!("mesh {} primitive {} stays ordinary geometry ({})", s.mesh_index, s.primitive_index, s.reason))
                .collect(),
        },
        Ensured::UpToDate | Ensured::NothingToConvert { .. } | Ensured::Model => RvgBakeOutcome::default(),
    }
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
        .add_message::<renzora::CompanionJobDone>()
        .add_systems(Update, (follow_project, watch, finish_scan, collect_answer, pump).chain());
}

/// The companion's answer for the build handed to it.
fn collect_answer(mut answers: MessageReader<renzora::CompanionJobDone>, mut converter: ResMut<Converter>) {
    for answer in answers.read() {
        if let Some((_, Pending::Remote(id, slot))) = converter.running.as_mut() {
            if *id == answer.id {
                *slot = Some(answer.result.clone());
            }
        }
    }
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
    mut jobs: Option<ResMut<renzora::CompanionJobs>>,
) {
    if let Some((model, pending)) = converter.running.as_mut() {
        let result: Option<Result<RvgBakeOutcome, String>> = match pending {
            Pending::Local(task) => block_on(future::poll_once(task)).map(|r| r.map(outcome_of)),
            Pending::Remote(_, answer) => answer.take().map(|r| {
                r.and_then(|json| serde_json::from_str(&json).map_err(|e| format!("unreadable answer: {e}")))
            }),
        };
        let Some(result) = result else {
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
            Ok(done) if done.built => {
                converter.built_this_batch += 1;
                info!(
                    "[virtual geometry] built {name}: {} mesh(es), {:.1} MB, {:.1}s",
                    done.primitives,
                    done.bytes as f64 / 1e6,
                    done.seconds
                );
                for line in done.skipped {
                    info!("[virtual geometry] {name}: {line}");
                }
                if let Some(relative) = relative {
                    updated.write(RvgUpdated {
                        model: AssetPath::from(relative),
                    });
                }
            }
            Ok(_) => {}
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
    let target = next.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(id) = jobs
        .as_deref_mut()
        .and_then(|jobs| jobs.submit("rvg_bake", target, &RvgBake { model: next.clone() }))
    {
        converter.running = Some((next, Pending::Remote(id, None)));
        return;
    }
    let path = next.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move { ensure_current(&path) });
    converter.running = Some((next, Pending::Local(task)));
}
