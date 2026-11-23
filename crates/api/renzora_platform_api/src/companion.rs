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
//! given up on), or present but unable to run the kind asked for: it says
//! which kinds it runs when it connects. Either way `submit` returns `None`,
//! and the caller does the work itself the way it always did: moving a job to
//! the companion must never make it impossible.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};

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
    /// Set by the host while a companion has been started and has not
    /// connected yet. Work that can wait a few seconds (thumbnails nobody has
    /// asked to see) waits for it rather than doing the job in the editor
    /// during the very frames a project is loading.
    pub starting: bool,
    next: CompanionJobId,
    pending: Vec<CompanionJobRequest>,
    remote: CompanionRemote,
}

impl CompanionJobs {
    /// Queue `kind` with `args` for the companion. `None` when no companion is
    /// connected, or the one connected cannot run `kind`: do the work yourself.
    pub fn submit(&mut self, kind: &str, target: impl Into<String>, args: &impl Serialize) -> Option<CompanionJobId> {
        if !self.can_run(kind) {
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

    /// Jobs whose submitter wants them stopped. The companion's host calls
    /// this and passes each on.
    pub fn take_cancels(&mut self) -> Vec<CompanionJobId> {
        self.remote.take_cancels()
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

    /// The host: the job kinds the connected companion said it runs. Any
    /// other kind is refused by `submit` and `run`, so its caller does it.
    pub fn set_kinds(&mut self, kinds: impl IntoIterator<Item = String>) {
        *self.remote.0.kinds.write().unwrap_or_else(|e| e.into_inner()) = kinds.into_iter().collect();
    }

    /// Whether the connected companion runs `kind`; false with none connected.
    pub fn can_run(&self, kind: &str) -> bool {
        self.available && self.remote.lists(kind)
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
    /// The kinds the connected companion runs; see [`CompanionJobs::set_kinds`].
    kinds: RwLock<HashSet<String>>,
    state: Mutex<RemoteState>,
    changed: Condvar,
}

#[derive(Default)]
struct RemoteState {
    next: u64,
    requests: Vec<CompanionJobRequest>,
    /// Jobs a thread is waiting on, with what arrived for them.
    waiting: HashMap<CompanionJobId, RemoteSlot>,
    /// Jobs a thread gave up on, for the host to cancel in the companion.
    cancels: Vec<CompanionJobId>,
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

    /// Whether a companion is connected and runs `kind`.
    pub fn can_run(&self, kind: &str) -> bool {
        self.available() && self.lists(kind)
    }

    /// Whether the companion's hello listed `kind`, connected or not.
    fn lists(&self, kind: &str) -> bool {
        self.0.kinds.read().unwrap_or_else(|e| e.into_inner()).contains(kind)
    }

    /// Run `kind` with `args` in the companion and wait for its answer,
    /// passing each progress report to `on_progress` meanwhile. `None` when
    /// no companion is connected, or it cannot run `kind`: do the work
    /// yourself. A companion that stops mid-job answers `Err`.
    ///
    /// Blocks the calling thread. Never call it from a system.
    pub fn run(
        &self,
        kind: &str,
        target: impl Into<String>,
        args: &impl Serialize,
        on_progress: &mut dyn FnMut(Option<f32>, &str),
    ) -> Option<Result<String, String>> {
        self.run_cancellable(kind, target, args, on_progress, &AtomicBool::new(false))
    }

    /// [`CompanionRemote::run`], cancelled in the companion when `cancel` is
    /// set. The answer still arrives (as `Err("cancelled")`, or as whatever
    /// the job finished with first), so the caller hears back exactly once.
    pub fn run_cancellable(
        &self,
        kind: &str,
        target: impl Into<String>,
        args: &impl Serialize,
        on_progress: &mut dyn FnMut(Option<f32>, &str),
        cancel: &AtomicBool,
    ) -> Option<Result<String, String>> {
        if !self.can_run(kind) {
            return None;
        }
        let payload = serde_json::to_string(args).ok()?;
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.next += 1;
        let id = REMOTE_BIT | state.next;
        state.requests.push(CompanionJobRequest { id, kind: kind.to_string(), payload, target: target.into() });
        state.waiting.insert(id, RemoteSlot::default());
        let mut cancel_sent = false;
        loop {
            if !cancel_sent && cancel.load(Ordering::Relaxed) {
                cancel_sent = true;
                state.cancels.push(id);
            }
            let slot = state.waiting.get_mut(&id)?;
            let progress = std::mem::take(&mut slot.progress);
            let result = slot.result.take();
            if result.is_some() {
                state.waiting.remove(&id);
            }
            if progress.is_empty() && result.is_none() {
                // Woken by news, or every quarter second to look at `cancel`.
                state = self
                    .0
                    .changed
                    .wait_timeout(state, std::time::Duration::from_millis(250))
                    .map(|(s, _)| s)
                    .unwrap_or_else(|e| e.into_inner().0);
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

    fn take_cancels(&self) -> Vec<CompanionJobId> {
        std::mem::take(&mut self.0.state.lock().unwrap_or_else(|e| e.into_inner()).cancels)
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
/// that submits builds one, the companion's handler reads it back. Defined in
/// `renzora_companion_proto`, which the companion links without Bevy.
pub use renzora_companion_proto::job_args;

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
        jobs.set_kinds(["texture_thumbnail".to_string()]);
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

    /// A connected companion that did not list a kind is not sent it, from a
    /// system or a thread: the caller does that kind itself.
    #[test]
    fn a_kind_the_companion_did_not_list_stays_in_the_editor() {
        let mut jobs = CompanionJobs::default();
        jobs.set_available(true);
        jobs.set_kinds(["texture_thumbnail".to_string()]);
        assert!(jobs.can_run("texture_thumbnail"));
        assert!(!jobs.can_run("model_thumbnail"));
        assert_eq!(jobs.submit("model_thumbnail", "a.glb", &1), None);
        assert_eq!(jobs.remote().run("model_convert", "a.fbx", &1, &mut |_, _| {}), None);
        assert!(jobs.drain().is_empty());
    }

    /// A worker thread submits and waits; the host drains it with the
    /// systems' jobs and answers it by id, progress first.
    #[test]
    fn a_thread_waits_for_its_answer_and_hears_progress() {
        let mut jobs = CompanionJobs::default();
        jobs.set_available(true);
        jobs.set_kinds(["model_convert".to_string()]);
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
