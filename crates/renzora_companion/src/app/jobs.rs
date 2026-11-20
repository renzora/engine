//! The companion's job queue: what the editor hands over, run off its thread.
//!
//! Jobs wait in a queue ordered by priority and run on worker threads, at
//! most [`WORKERS`] at once. A job that uses the GPU waits while Play is
//! running a game or a profile is recording, so it never competes with what
//! you are playing or measuring. Each job reports progress, can be cancelled,
//! and ends with exactly one `Done`, which the link forwards to the editor.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bevy::prelude::*;

use super::games::Connection;
use super::link::EditorState;
use crate::protocol::{Job, JobId};

/// Jobs running at once. Most are disk- or CPU-bound and the editor is busy on
/// the same machine, so a few is plenty.
pub(crate) const WORKERS: usize = 3;

/// Finished jobs the window keeps listing.
const HISTORY: usize = 200;

/// What a job tells the editor.
pub(crate) enum Report {
    Progress { id: JobId, fraction: Option<f32>, note: String },
    Done { id: JobId, result: Result<String, String> },
}

/// Handed to a running job: how to report, and whether to stop.
#[derive(Clone)]
pub(crate) struct JobCtx {
    id: JobId,
    cancelled: Arc<AtomicBool>,
    reports: Sender<Report>,
}

impl JobCtx {
    /// Finish the job. A worker thread's job finishes by returning; a
    /// main-thread job (one that renders) calls this when it is done.
    pub fn finish(&self, result: Result<String, String>) {
        let result = if self.cancelled() && result.is_ok() { Err("cancelled".into()) } else { result };
        let _ = self.reports.send(Report::Done { id: self.id, result });
    }

    pub fn progress(&self, fraction: Option<f32>, note: impl Into<String>) {
        let _ = self.reports.send(Report::Progress { id: self.id, fraction, note: note.into() });
    }

    /// Whether the editor cancelled this job. Long jobs check between steps
    /// and return `Err("cancelled")`.
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// A context for running a handler directly in a test.
    #[cfg(test)]
    pub fn for_test() -> (Self, Receiver<Report>) {
        let (reports, rx) = channel();
        (Self { id: 0, cancelled: Arc::new(AtomicBool::new(false)), reports }, rx)
    }
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
    pub id: JobId,
    pub label: &'static str,
    pub target: String,
    pub result: Result<String, String>,
    pub took_ms: f64,
}

#[derive(Resource)]
pub(crate) struct JobQueue {
    pending: VecDeque<(Entry, Job)>,
    pub running: BTreeMap<JobId, (Entry, Arc<AtomicBool>)>,
    pub finished: VecDeque<Finished>,
    reports_tx: Sender<Report>,
    reports_rx: Mutex<Receiver<Report>>,
    /// Reports waiting for the link to forward.
    out: Vec<Report>,
    /// Started jobs that run on the main thread (they render), waiting for
    /// `app::render` to take them.
    pub main_thread: Vec<(Job, JobCtx)>,
}

impl Default for JobQueue {
    fn default() -> Self {
        let (reports_tx, rx) = channel();
        Self {
            pending: VecDeque::new(),
            running: BTreeMap::new(),
            finished: VecDeque::new(),
            reports_tx,
            reports_rx: Mutex::new(rx),
            out: Vec::new(),
            main_thread: Vec::new(),
        }
    }
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

    /// Whether nothing runs and nothing waits: when work the companion gives
    /// itself may start without delaying anything the editor asked for.
    pub fn idle(&self) -> bool {
        self.running.is_empty() && self.pending.is_empty() && self.main_thread.is_empty()
    }

    /// Answer a request that was handled on the spot rather than queued (a
    /// question about the capture). It goes back like any job's result but is
    /// not listed: it took no time and did no work worth showing.
    pub fn answer(&mut self, id: JobId, result: Result<String, String>) {
        self.out.push(Report::Done { id, result });
    }

    pub fn take_reports(&mut self) -> Vec<Report> {
        std::mem::take(&mut self.out)
    }

    fn finish(&mut self, entry: Entry, result: Result<String, String>) {
        let took_ms = entry
            .started_at
            .map(|t| t.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        self.out.push(Report::Done { id: entry.id, result: result.clone() });
        if self.finished.len() == HISTORY {
            self.finished.pop_back();
        }
        self.finished.push_front(Finished {
            id: entry.id,
            label: entry.label,
            target: entry.target,
            result,
            took_ms,
        });
    }
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<JobQueue>().add_systems(Update, (start_jobs, collect_reports).chain());
}

/// Why `job` must wait right now, if it must.
///
/// Recording holds every job, not only the ones that render: an import or a
/// compile competes with the game for cores, caches and memory bandwidth, and
/// a profile is only worth reading if nothing else was running. Play holds
/// only the jobs that render, since a game being played is not being measured.
/// A job the user started anyway ([`JobQueue::run_anyway`]) is not held, and
/// the frames recorded while it runs are marked (see `Capture::overlapped`).
pub(crate) fn held(entry: &Entry, job: &Job, editor: &EditorState, conn: &Connection) -> Option<&'static str> {
    if entry.forced {
        None
    } else if conn.recording {
        Some("recording to stop")
    } else if editor.playing && job.uses_gpu() {
        Some("Play to stop")
    } else {
        None
    }
}

/// Start as many waiting jobs as there are free workers.
fn start_jobs(mut queue: ResMut<JobQueue>, editor: Res<EditorState>, conn: Res<Connection>) {
    while queue.running.len() < WORKERS {
        let Some(at) = queue
            .pending
            .iter()
            .position(|(entry, job)| held(entry, job, &editor, &conn).is_none())
        else {
            break;
        };
        let Some((mut entry, job)) = queue.pending.remove(at) else { break };
        entry.started_at = Some(Instant::now());
        let cancelled = Arc::new(AtomicBool::new(false));
        let ctx = JobCtx { id: entry.id, cancelled: cancelled.clone(), reports: queue.reports_tx.clone() };
        let id = entry.id;
        queue.running.insert(id, (entry, cancelled));
        // A rendering job runs on this thread, in the `World`: handed to
        // `app::render`, which reports through the same context.
        if job.runs_on_main() {
            queue.main_thread.push((job, ctx));
            continue;
        }
        let spawned = std::thread::Builder::new()
            .name(format!("companion-job-{id}"))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::work::run(job, &ctx)))
                    .unwrap_or_else(|_| Err("the job crashed".into()));
                let result = if ctx.cancelled() && result.is_ok() { Err("cancelled".into()) } else { result };
                let _ = ctx.reports.send(Report::Done { id, result });
            });
        if let Err(e) = spawned {
            if let Some((entry, _)) = queue.running.remove(&id) {
                queue.finish(entry, Err(format!("could not start a worker: {e}")));
            }
        }
    }
}

/// Fold workers' reports into the queue, and queue them for the editor.
fn collect_reports(mut queue: ResMut<JobQueue>) {
    let reports: Vec<Report> = match queue.reports_rx.lock() {
        Ok(rx) => rx.try_iter().collect(),
        Err(_) => Vec::new(),
    };
    for report in reports {
        match report {
            Report::Progress { id, fraction, note } => {
                if let Some((entry, _)) = queue.running.get_mut(&id) {
                    entry.fraction = fraction;
                    entry.note.clone_from(&note);
                }
                queue.out.push(Report::Progress { id, fraction, note });
            }
            Report::Done { id, result } => {
                if let Some((entry, _)) = queue.running.remove(&id) {
                    queue.finish(entry, result);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo(text: &str) -> Job {
        Job::Echo { text: text.into(), millis: 0 }
    }

    #[test]
    fn recording_holds_every_job_until_one_is_run_anyway() {
        let mut queue = JobQueue::default();
        queue.submit(1, echo("a"));
        let (entry, job) = queue.waiting().next().unwrap();
        let mut recording = Connection::default();
        recording.recording = true;
        let idle = Connection::default();
        let playing = EditorState { playing: true };
        assert_eq!(held(entry, job, &EditorState::default(), &recording), Some("recording to stop"));
        // Play holds only jobs that render, and an echo does not.
        assert_eq!(held(entry, job, &playing, &idle), None);
        queue.run_anyway(1);
        let (entry, job) = queue.waiting().next().unwrap();
        assert_eq!(held(entry, job, &EditorState::default(), &recording), None);
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
    fn jobs_run_and_finish() {
        let mut app = App::new();
        app.init_resource::<EditorState>().init_resource::<Connection>();
        build(&mut app);
        app.world_mut().resource_mut::<JobQueue>().submit(7, echo("pong"));
        for _ in 0..200 {
            app.update();
            if !app.world().resource::<JobQueue>().finished.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let queue = app.world().resource::<JobQueue>();
        let done = queue.finished.front().expect("the job finished");
        assert_eq!(done.result, Ok("pong".to_string()));
    }
}
