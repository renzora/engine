//! The job queue: what the editor hands over, run on worker threads.
//!
//! Jobs wait in a queue ordered by priority and run on worker threads, at
//! most [`WORKERS`] at once. Each reports progress, can be cancelled, and ends
//! with exactly one `Done`, which the server forwards to the editor.
//!
//! Ported from the engine companion's `app::jobs`, without the `Resource` and
//! the systems: the server thread owns the queue and calls [`JobQueue::start`]
//! and [`JobQueue::report`] itself.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

use renzora_companion_proto::protocol::{Job, JobId};

use crate::server::Event;

/// Jobs running at once. Most are disk- or CPU-bound and the editor is busy on
/// the same machine, so a few is plenty.
pub(crate) const WORKERS: usize = 3;

/// Finished jobs the window keeps listing.
const HISTORY: usize = 200;

/// What a job tells the editor.
#[derive(Debug)]
pub(crate) enum Report {
    Progress { id: JobId, fraction: Option<f32>, note: String },
    Done { id: JobId, result: Result<String, String> },
}

/// Handed to a running job: how to report, and whether to stop.
#[derive(Clone)]
pub(crate) struct JobCtx {
    id: JobId,
    cancelled: Arc<AtomicBool>,
    events: Sender<Event>,
}

impl JobCtx {
    pub fn progress(&self, fraction: Option<f32>, note: impl Into<String>) {
        let _ = self.events.send(Event::Report(Report::Progress { id: self.id, fraction, note: note.into() }));
    }

    /// Whether the editor cancelled this job. Long jobs check between steps
    /// and return `Err("cancelled")`.
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// A context for running a handler directly in a test.
    #[cfg(test)]
    pub fn for_test() -> (Self, std::sync::mpsc::Receiver<Event>) {
        let (events, rx) = std::sync::mpsc::channel();
        (Self { id: 0, cancelled: Arc::new(AtomicBool::new(false)), events }, rx)
    }
}

/// What the editor said about itself, which decides when GPU jobs may run.
#[derive(Default)]
pub(crate) struct EditorState {
    /// Play is running a game: jobs that render wait so they never compete
    /// with it.
    pub playing: bool,
    /// The editor's Render thumbnails setting is off: no thumbnail is
    /// rendered here, and the render worker is not kept.
    pub thumbnails_off: bool,
}

/// A job waiting or running, as the window shows it.
pub(crate) struct Entry {
    pub id: JobId,
    pub label: &'static str,
    pub target: String,
    pub fraction: Option<f32>,
    pub note: String,
    pub started_at: Option<Instant>,
    /// Started by hand while it would otherwise wait for a recording.
    pub forced: bool,
}

/// A job that ended.
pub(crate) struct Finished {
    pub label: &'static str,
    pub target: String,
    pub result: Result<String, String>,
    pub took_ms: f64,
}

#[derive(Default)]
pub(crate) struct JobQueue {
    pending: VecDeque<(Entry, Job)>,
    pub running: BTreeMap<JobId, (Entry, Arc<AtomicBool>)>,
    pub finished: VecDeque<Finished>,
    /// Reports waiting for the server to forward.
    out: Vec<Report>,
    /// Started jobs that render, for the server to hand to the render worker
    /// (see `crate::render`). They count as running like any other.
    to_render: Vec<(JobId, Job)>,
}

impl JobQueue {
    pub fn submit(&mut self, id: JobId, job: Job) {
        let entry = Entry {
            id,
            label: job.label(),
            target: job.target(),
            fraction: None,
            note: String::new(),
            started_at: None,
            forced: false,
        };
        // Highest priority first; equal priorities keep their order.
        let at = self
            .pending
            .iter()
            .position(|(_, waiting)| waiting.priority() < job.priority())
            .unwrap_or(self.pending.len());
        self.pending.insert(at, (entry, job));
    }

    pub fn cancel(&mut self, id: JobId) {
        if let Some(at) = self.pending.iter().position(|(e, _)| e.id == id) {
            if let Some((entry, _)) = self.pending.remove(at) {
                self.finish(entry, Err("cancelled".into()));
            }
        } else if let Some((_, flag)) = self.running.get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn waiting(&self) -> impl Iterator<Item = (&Entry, &Job)> {
        self.pending.iter().map(|(e, j)| (e, j))
    }

    /// Start a waiting job despite a recording, from the Jobs screen.
    pub fn run_anyway(&mut self, id: JobId) {
        if let Some((entry, _)) = self.pending.iter_mut().find(|(e, _)| e.id == id) {
            entry.forced = true;
        }
    }

    /// Whether a job is running now: frames recorded meanwhile shared the
    /// machine with it.
    pub fn busy(&self) -> bool {
        !self.running.is_empty()
    }

    /// Answer a request that was handled on the spot rather than queued (the
    /// project listing). It goes back like any job's result but is not
    /// listed: it did no work worth showing.
    pub fn answer(&mut self, id: JobId, result: Result<String, String>) {
        self.out.push(Report::Done { id, result });
    }

    pub fn take_reports(&mut self) -> Vec<Report> {
        std::mem::take(&mut self.out)
    }

    /// Rendering jobs [`Self::start`] started, for the render worker.
    pub fn take_render(&mut self) -> Vec<(JobId, Job)> {
        std::mem::take(&mut self.to_render)
    }

    /// Whether nothing runs and nothing waits: when work the companion gives
    /// itself may start without delaying anything the editor asked for.
    pub fn idle(&self) -> bool {
        self.running.is_empty() && self.pending.is_empty()
    }

    /// Start as many waiting jobs as there are free workers. A worker reports
    /// through `events`, which is how its progress and result reach
    /// [`Self::report`].
    pub fn start(&mut self, editor: &EditorState, recording: bool, events: &Sender<Event>) {
        while self.running.len() < WORKERS {
            let Some(at) = self.pending.iter().position(|(entry, job)| held(entry, job, editor, recording).is_none())
            else {
                break;
            };
            let Some((mut entry, job)) = self.pending.remove(at) else { break };
            entry.started_at = Some(Instant::now());
            let cancelled = Arc::new(AtomicBool::new(false));
            let id = entry.id;
            let ctx = JobCtx { id, cancelled: cancelled.clone(), events: events.clone() };
            self.running.insert(id, (entry, cancelled));
            if job.uses_gpu() {
                self.to_render.push((id, job));
                continue;
            }
            let spawned = std::thread::Builder::new().name(format!("companion-job-{id}")).spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::work::run(job, &ctx)))
                    .unwrap_or_else(|_| Err("the job crashed".into()));
                let result = if ctx.cancelled() && result.is_ok() { Err("cancelled".into()) } else { result };
                let _ = ctx.events.send(Event::Report(Report::Done { id, result }));
            });
            if let Err(e) = spawned {
                if let Some((entry, _)) = self.running.remove(&id) {
                    self.finish(entry, Err(format!("could not start a worker: {e}")));
                }
            }
        }
    }

    /// Fold a worker's report into the queue, and queue it for the editor.
    pub fn report(&mut self, report: Report) {
        match report {
            Report::Progress { id, fraction, note } => {
                if let Some((entry, _)) = self.running.get_mut(&id) {
                    entry.fraction = fraction;
                    entry.note.clone_from(&note);
                }
                self.out.push(Report::Progress { id, fraction, note });
            }
            Report::Done { id, result } => {
                if let Some((entry, _)) = self.running.remove(&id) {
                    self.finish(entry, result);
                }
            }
        }
    }

    fn finish(&mut self, entry: Entry, result: Result<String, String>) {
        let took_ms = entry.started_at.map(|t| t.elapsed().as_secs_f64() * 1000.0).unwrap_or(0.0);
        self.out.push(Report::Done { id: entry.id, result: result.clone() });
        if self.finished.len() == HISTORY {
            self.finished.pop_back();
        }
        self.finished.push_front(Finished { label: entry.label, target: entry.target, result, took_ms });
    }
}

/// Why `job` must wait right now, if it must.
///
/// Recording holds every job, not only the ones that render: an import or a
/// compile competes with the game for cores, caches and memory bandwidth, and
/// a profile is only worth reading if nothing else was running. Play holds
/// only the jobs that render (none of this companion's own kinds do yet; that
/// is the render worker's, still to come), since a game being played is not
/// being measured. A job started anyway ([`JobQueue::run_anyway`]) is not
/// held, and frames recorded while it runs are marked (`Capture::overlapped`).
pub(crate) fn held(entry: &Entry, job: &Job, editor: &EditorState, recording: bool) -> Option<&'static str> {
    if entry.forced {
        None
    } else if recording {
        Some("recording to stop")
    } else if editor.playing && job.uses_gpu() {
        Some("Play to stop")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn echo(text: &str) -> Job {
        Job::Echo { text: text.into(), millis: 0 }
    }

    fn run(kind: &str) -> Job {
        Job::Run { kind: kind.into(), payload: "{}".into(), target: String::new() }
    }

    fn entry() -> Entry {
        Entry { id: 0, label: "", target: String::new(), fraction: None, note: String::new(), started_at: None, forced: false }
    }

    #[test]
    fn play_holds_only_jobs_that_render() {
        let playing = EditorState { playing: true, ..EditorState::default() };
        assert_eq!(held(&entry(), &run("model_thumbnail"), &playing, false), Some("Play to stop"));
        assert_eq!(held(&entry(), &run("texture_thumbnail"), &playing, false), None);
        assert_eq!(held(&entry(), &run("model_thumbnail"), &EditorState::default(), false), None);
    }

    #[test]
    fn recording_holds_every_job_until_one_is_run_anyway() {
        let idle = EditorState::default();
        assert_eq!(held(&entry(), &run("rust_script"), &idle, true), Some("recording to stop"));
        let forced = Entry { forced: true, ..entry() };
        assert_eq!(held(&forced, &run("rust_script"), &idle, true), None);

        let (events, _rx) = std::sync::mpsc::channel();
        let mut queue = JobQueue::default();
        queue.submit(1, echo("a"));
        queue.start(&idle, true, &events);
        assert!(queue.running.is_empty());
        queue.run_anyway(1);
        queue.start(&idle, true, &events);
        assert_eq!(queue.running.len(), 1);
    }

    #[test]
    fn higher_priority_jobs_wait_in_front() {
        let mut queue = JobQueue::default();
        queue.submit(1, run("rvg_bake"));
        queue.submit(2, run("rust_script"));
        queue.submit(3, run("rvg_bake"));
        let order: Vec<JobId> = queue.waiting().map(|(e, _)| e.id).collect();
        assert_eq!(order, [2, 1, 3]);
    }

    #[test]
    fn a_queued_job_can_be_cancelled_and_still_reports_done() {
        let mut queue = JobQueue::default();
        queue.submit(1, echo("a"));
        queue.cancel(1);
        let reports = queue.take_reports();
        assert!(matches!(reports.as_slice(), [Report::Done { id: 1, result: Err(e) }] if e == "cancelled"));
        assert_eq!(queue.waiting().count(), 0);
    }

    #[test]
    fn jobs_run_on_workers_and_finish() {
        let (events, rx) = std::sync::mpsc::channel();
        let mut queue = JobQueue::default();
        queue.submit(7, echo("pong"));
        queue.start(&EditorState::default(), false, &events);
        assert_eq!(queue.running.len(), 1);
        loop {
            match rx.recv_timeout(Duration::from_secs(5)).expect("the worker reported") {
                Event::Report(report) => queue.report(report),
                _ => continue,
            }
            if queue.running.is_empty() {
                break;
            }
        }
        assert_eq!(queue.finished.front().map(|f| f.result.clone()), Some(Ok("pong".to_string())));
        assert!(queue.take_reports().iter().any(|r| matches!(r, Report::Done { id: 7, result: Ok(_) })));
    }

    /// A job that renders is started like any other, but handed out for the
    /// render worker instead of to a thread here.
    #[test]
    fn rendering_jobs_go_to_the_render_worker() {
        let (events, _rx) = std::sync::mpsc::channel();
        let mut queue = JobQueue::default();
        queue.submit(4, run("material_thumbnail"));
        queue.start(&EditorState::default(), false, &events);
        assert_eq!(queue.running.len(), 1);
        let handed: Vec<JobId> = queue.take_render().into_iter().map(|(id, _)| id).collect();
        assert_eq!(handed, [4]);
        queue.report(Report::Done { id: 4, result: Ok("x.png".into()) });
        assert!(queue.idle());
    }
}
