//! Handing work to the companion process, from any editor crate.
//!
//! The companion (`renzora_companion`) runs heavy work off the editor: in
//! another process, at lower priority, with no access to the editor's frame.
//! A crate asks for it here, by kind and serialized arguments, without
//! depending on the companion: [`CompanionJobs::submit`] queues the request
//! and the companion's host sends it; [`CompanionJobDone`] comes back with the
//! result, and [`CompanionJobProgress`] while it runs.
//!
//! The companion may be absent (switched off, still starting, or crashed and
//! given up on). `submit` then returns `None`, and the caller does the work
//! itself the way it always did: moving a job to the companion must never
//! make it impossible.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use bevy::prelude::*;
use serde::Serialize;

/// Identifies one job; unique within an editor session.
pub type CompanionJobId = u64;

/// A request on its way to the companion.
#[derive(Clone, Debug)]
pub struct CompanionJobRequest {
    pub id: CompanionJobId,
    /// Which handler runs it, e.g. `"texture_thumbnail"`. The companion knows
    /// every kind; an unknown one fails with a message saying so.
    pub kind: String,
    /// The arguments, as JSON, in the shape that kind's handler expects.
    pub payload: String,
    /// Shown in the companion's job list and the status bar.
    pub target: String,
}

/// The queue of requests, and whether a companion is there to take them.
#[derive(Resource, Default)]
pub struct CompanionJobs {
    /// Set by the companion's host while one is connected.
    pub available: bool,
    next: CompanionJobId,
    pending: Vec<CompanionJobRequest>,
    remote: CompanionRemote,
}

impl CompanionJobs {
    /// Queue `kind` with `args` for the companion. `None` when no companion is
    /// connected: do the work yourself.
    pub fn submit(&mut self, kind: &str, target: impl Into<String>, args: &impl Serialize) -> Option<CompanionJobId> {
        if !self.available {
            return None;
        }
        let payload = serde_json::to_string(args).ok()?;
        self.next += 1;
        let id = self.next;
        self.pending.push(CompanionJobRequest {
            id,
            kind: kind.to_string(),
            payload,
            target: target.into(),
        });
        Some(id)
    }

    /// Take the queued requests, from systems and from worker threads alike.
    /// The companion's host calls this.
    pub fn drain(&mut self) -> Vec<CompanionJobRequest> {
        let mut all = std::mem::take(&mut self.pending);
        all.extend(self.remote.take_requests());
        all
    }

    /// A handle for a worker thread to run jobs through; see
    /// [`CompanionRemote`].
    pub fn remote(&self) -> CompanionRemote {
        self.remote.clone()
    }

    /// The host: say whether a companion is there, for systems and threads.
    pub fn set_available(&mut self, available: bool) {
        self.available = available;
        self.remote.0.available.store(available, Ordering::Relaxed);
    }

    /// The host: a job finished. True when it was a worker thread's, which is
    /// then woken with the result; false when it belongs to a system, which
    /// hears through [`CompanionJobDone`] instead.
    pub fn finish_remote(&self, id: CompanionJobId, result: &Result<String, String>) -> bool {
        self.remote.finish(id, result)
    }

    /// The host: a job reported progress. True when it was a worker thread's.
    pub fn progress_remote(&self, id: CompanionJobId, fraction: Option<f32>, note: &str) -> bool {
        self.remote.progress(id, fraction, note)
    }
}

/// Runs companion jobs from a worker thread, which has no `World` to reach
/// [`CompanionJobs`] through.
///
/// Import and export do their heavy work on threads of their own. Handing it
/// to the companion from there means submitting and then waiting, on that
/// thread, for the answer; [`CompanionRemote::run`] does both. Clone one out
/// of [`CompanionJobs::remote`] before starting the thread.
///
/// Ids from here have the top bit set, so they never meet a system's.
#[derive(Clone, Default)]
pub struct CompanionRemote(Arc<RemoteShared>);

#[derive(Default)]
struct RemoteShared {
    available: AtomicBool,
    state: Mutex<RemoteState>,
    changed: Condvar,
}

#[derive(Default)]
struct RemoteState {
    next: u64,
    requests: Vec<CompanionJobRequest>,
    /// Jobs a thread is waiting on, with what arrived for them.
    waiting: HashMap<CompanionJobId, RemoteSlot>,
}

#[derive(Default)]
struct RemoteSlot {
    progress: Vec<(Option<f32>, String)>,
    result: Option<Result<String, String>>,
}

const REMOTE_BIT: u64 = 1 << 63;

impl CompanionRemote {
    /// Whether a companion is connected right now.
    pub fn available(&self) -> bool {
        self.0.available.load(Ordering::Relaxed)
    }

    /// Run `kind` with `args` in the companion and wait for its answer,
    /// passing each progress report to `on_progress` meanwhile. `None` when
    /// no companion is connected: do the work yourself. A companion that
    /// stops mid-job answers `Err`.
    ///
    /// Blocks the calling thread. Never call it from a system.
    pub fn run(
        &self,
        kind: &str,
        target: impl Into<String>,
        args: &impl Serialize,
        on_progress: &mut dyn FnMut(Option<f32>, &str),
    ) -> Option<Result<String, String>> {
        if !self.available() {
            return None;
        }
        let payload = serde_json::to_string(args).ok()?;
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.next += 1;
        let id = REMOTE_BIT | state.next;
        state.requests.push(CompanionJobRequest { id, kind: kind.to_string(), payload, target: target.into() });
        state.waiting.insert(id, RemoteSlot::default());
        loop {
            let slot = state.waiting.get_mut(&id)?;
            let progress = std::mem::take(&mut slot.progress);
            let result = slot.result.take();
            if result.is_some() {
                state.waiting.remove(&id);
            }
            if progress.is_empty() && result.is_none() {
                state = self.0.changed.wait(state).unwrap_or_else(|e| e.into_inner());
                continue;
            }
            // Reported without the lock held: the callback may take a while.
            drop(state);
            for (fraction, note) in &progress {
                on_progress(*fraction, note);
            }
            if let Some(result) = result {
                return Some(result);
            }
            state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        }
    }

    fn take_requests(&self) -> Vec<CompanionJobRequest> {
        std::mem::take(&mut self.0.state.lock().unwrap_or_else(|e| e.into_inner()).requests)
    }

    fn finish(&self, id: CompanionJobId, result: &Result<String, String>) -> bool {
        if id & REMOTE_BIT == 0 {
            return false;
        }
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = state.waiting.get_mut(&id) {
            slot.result = Some(result.clone());
            self.0.changed.notify_all();
        }
        true
    }

    fn progress(&self, id: CompanionJobId, fraction: Option<f32>, note: &str) -> bool {
        if id & REMOTE_BIT == 0 {
            return false;
        }
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = state.waiting.get_mut(&id) {
            slot.progress.push((fraction, note.to_string()));
            self.0.changed.notify_all();
        }
        true
    }
}

/// A companion job finished. `result` is the handler's answer: usually a path
/// or a short summary on success, a reason in words on failure. A job the
/// companion lost (it crashed mid-job) arrives as `Err`, so every submitter
/// hears back exactly once.
#[derive(Message, Clone, Debug)]
pub struct CompanionJobDone {
    pub id: CompanionJobId,
    pub kind: String,
    pub result: Result<String, String>,
}

/// How a running companion job is getting on.
#[derive(Message, Clone, Debug)]
pub struct CompanionJobProgress {
    pub id: CompanionJobId,
    pub fraction: Option<f32>,
    pub note: String,
}

/// The arguments of each job kind. Both ends need one definition: the crate
/// that submits builds one, the companion's handler reads it back.
pub mod job_args {
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;

    /// `"texture_thumbnail"`: decode `source`, shrink it to fit `size` square
    /// (Lanczos3), write it as a PNG to `cache`.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    pub struct TextureThumbnail {
        pub source: PathBuf,
        pub cache: PathBuf,
        pub size: u32,
    }

    /// `"rust_script"`: compile the script `source` of `project` against the
    /// SDK in `sdk_dir`. Answers with the built library's path, which the
    /// editor then loads itself (a library is loaded by the process that runs
    /// it).
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    pub struct RustScriptBuild {
        pub sdk_dir: PathBuf,
        pub project: PathBuf,
        pub source: PathBuf,
    }

    /// `"rvg_bake"`: bring the virtual geometry beside the GLB `model` up to
    /// date. Answers with an [`RvgBakeOutcome`] as JSON.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    pub struct RvgBake {
        pub model: PathBuf,
    }

    /// `"model_thumbnail"`: render `model` (a `.glb`, `.gltf` or `.rvg` in
    /// `project`) to its cached thumbnail PNG, with the editor's own capture
    /// code. Answers with the PNG's path; the editor then loads it as it
    /// loads any cached thumbnail.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    pub struct ModelThumbnail {
        pub project: PathBuf,
        pub model: PathBuf,
    }

    /// `"material_thumbnail"`: render `material` (a `.material` in `project`)
    /// to its cached thumbnail PNG, with the material editor's own capture
    /// code. Answers with the PNG's path.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    pub struct MaterialThumbnail {
        pub project: PathBuf,
        pub material: PathBuf,
    }

    /// `"project_index"`: walk `root` and answer with its files as JSON
    /// (`Vec<IndexedFile>`, see `renzora::core::project_index`). Also tells the
    /// companion which project is open, so it can watch it and keep its
    /// caches (thumbnails) in step with it.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    pub struct ProjectIndexWalk {
        pub root: PathBuf,
    }

    /// `"profiler.record"`: start or stop recording. With `game`, connect to
    /// the game whose name contains it first (the editor itself is "Editor").
    /// Answers in words.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
    pub struct ProfilerRecord {
        pub on: bool,
        #[serde(default)]
        pub game: Option<String>,
    }

    /// `"profiler.status"`, `"profiler.summary"` and `"profiler.spikes"`: a
    /// question about the last `seconds` of the companion's capture. Answers
    /// with JSON.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
    pub struct ProfilerQuery {
        #[serde(default)]
        pub seconds: Option<f32>,
        /// How many rows to list.
        #[serde(default)]
        pub limit: Option<usize>,
        /// `"profiler.spikes"` only: frames slower than this. Without it, the
        /// slowest frames in the window.
        #[serde(default)]
        pub over_ms: Option<f32>,
    }

    /// What an `"rvg_bake"` did.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
    pub struct RvgBakeOutcome {
        /// A new `.rvg` was written; false when it was already current or
        /// there was nothing to convert.
        pub built: bool,
        pub primitives: usize,
        pub bytes: usize,
        pub seconds: f32,
        /// Why each primitive that stayed ordinary geometry did, one line each.
        pub skipped: Vec<String>,
    }
}

/// Registers the queue and the messages. Every crate that submits jobs adds
/// it; adding it twice is harmless.
pub fn init_companion_jobs(app: &mut App) {
    app.init_resource::<CompanionJobs>()
        .add_message::<CompanionJobDone>()
        .add_message::<CompanionJobProgress>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_queued_without_a_companion() {
        let mut jobs = CompanionJobs::default();
        assert_eq!(jobs.submit("texture_thumbnail", "a.png", &("a.png", 256)), None);
        jobs.available = true;
        let id = jobs.submit("texture_thumbnail", "a.png", &("a.png", 256)).unwrap();
        let queued = jobs.drain();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, id);
        assert_eq!(queued[0].payload, r#"["a.png",256]"#);
        assert!(jobs.drain().is_empty());
    }

    #[test]
    fn a_thread_without_a_companion_is_told_to_do_the_work() {
        let jobs = CompanionJobs::default();
        assert_eq!(jobs.remote().run("model_convert", "a.fbx", &1, &mut |_, _| {}), None);
    }

    /// A worker thread submits and waits; the host drains it with the
    /// systems' jobs and answers it by id, progress first.
    #[test]
    fn a_thread_waits_for_its_answer_and_hears_progress() {
        let mut jobs = CompanionJobs::default();
        jobs.set_available(true);
        let remote = jobs.remote();
        let worker = std::thread::spawn(move || {
            let mut notes = Vec::new();
            let result = remote.run("model_convert", "a.fbx", &"args", &mut |_, note| notes.push(note.to_string()));
            (result, notes)
        });
        let request = loop {
            if let Some(r) = jobs.drain().pop() {
                break r;
            }
            std::thread::yield_now();
        };
        assert_eq!(request.kind, "model_convert");
        assert!(jobs.progress_remote(request.id, Some(0.5), "converting"));
        assert!(jobs.finish_remote(request.id, &Ok("done".into())));
        let (result, notes) = worker.join().unwrap();
        assert_eq!(result, Some(Ok("done".into())));
        // Reported even though the answer came straight after it.
        assert_eq!(notes, ["converting"]);
        // A system's job id is not a thread's.
        assert!(!jobs.finish_remote(1, &Ok(String::new())));
    }
}
