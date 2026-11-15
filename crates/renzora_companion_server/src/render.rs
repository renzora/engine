//! The render worker: the engine, started only to render thumbnails.
//!
//! Model, material and Shape Library thumbnails are drawn by the engine's
//! renderer with the editor's own capture code, so a thumbnail looks the same
//! wherever it was made. This program has no renderer, so it starts one when
//! it needs one: `renzora --companion --render-worker`, the engine companion's
//! app with only its link, its queue and its capture code (see
//! `renzora_companion::app`). The worker speaks the same protocol to this
//! program that this program speaks to the editor: it says hello with the
//! kinds it renders, and each job goes over as a `Submit`.
//!
//! Each job gets a number of the worker's own (counting from 1), and its
//! progress and result are put back under the queue's id on the way in. The
//! queue's ids cannot go over as they are: the companion's own jobs (the
//! thumbnails it renders ahead of time) carry `cache::OWN_JOB`, and so do the
//! engine companion's, which is what the worker is, so it took them for its
//! own and never reported them. The first such job then counted as running
//! forever and the backlog behind it never moved.
//!
//! The worker is the expensive part of the companion: the engine's memory and
//! a GPU device. It exists only while there are thumbnails to render, and is
//! stopped once it has had nothing to do for [`IDLE_EXIT`]. A job it cannot
//! do (it failed to start, or stopped) fails, and the editor renders that
//! thumbnail itself, as it does with no companion.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use renzora_companion_proto::protocol::{self, Job, JobId, Message};

use crate::jobs::Report;
use crate::log;
use crate::server::Event;

/// The kinds the worker renders, announced to the editor in the hello when a
/// worker can be started at all.
pub(crate) const KINDS: &[&str] = &["model_thumbnail", "material_thumbnail", "shape_thumbnail"];

/// How long the worker may sit with nothing to render before it is stopped.
/// Long enough that a folder opened a few seconds after another does not pay
/// the engine's start again; short enough that its memory is not held for
/// nothing. A guess, to be tuned.
const IDLE_EXIT: Duration = Duration::from_secs(30);

/// How long a started worker has to connect: the engine's start, and its
/// first look at the plugins.
const START_TIMEOUT: Duration = Duration::from_secs(90);

/// Thumbnails one worker renders before it is replaced, at the next moment it
/// has nothing in hand. Its memory grew with every thumbnail while the
/// capture code kept each image (fixed in the worker), and still grows about
/// 2 MB a material whose textures the material system's caches keep (which
/// the editor wants: a material opened again is quick). Starting a fresh one
/// every 200 bounds that to a few hundred megabytes, for about two seconds of
/// engine start every couple of minutes of rendering.
const RECYCLE_AFTER: u32 = 200;

/// What the worker's threads report.
pub(crate) enum WorkerEvent {
    Connected(TcpStream),
    Message(Message),
    Closed,
}

enum State {
    Off,
    Starting { child: Child, since: Instant },
    Ready { child: Child, writer: TcpStream },
}

pub(crate) struct RenderWorker {
    exe: Option<PathBuf>,
    state: State,
    /// Counts starts, so a stopped worker's late events are not taken for the
    /// current one's.
    serial: u64,
    /// Handed to the worker and not yet done: the worker's number for each,
    /// and the queue's id it stands for.
    sent: BTreeMap<JobId, JobId>,
    /// The worker's number for the next job handed over.
    next_number: JobId,
    /// Waiting for the worker to connect, by the queue's id.
    waiting: VecDeque<(JobId, Job)>,
    idle_since: Option<Instant>,
    /// Jobs the running worker has finished (see [`RECYCLE_AFTER`]).
    finished: u32,
}

impl Default for RenderWorker {
    fn default() -> Self {
        Self::beside_this_executable()
    }
}

impl RenderWorker {
    /// A worker that would run the engine executable beside this one.
    pub(crate) fn beside_this_executable() -> Self {
        let exe = std::env::current_exe()
            .ok()
            .map(|exe| exe.with_file_name(format!("renzora{}", std::env::consts::EXE_SUFFIX)))
            .filter(|exe| exe.is_file());
        Self {
            exe,
            state: State::Off,
            serial: 0,
            sent: BTreeMap::new(),
            next_number: 0,
            waiting: VecDeque::new(),
            idle_since: None,
            finished: 0,
        }
    }

    /// Whether a worker can be started: the engine is beside this program.
    pub(crate) fn available(&self) -> bool {
        self.exe.is_some()
    }

    /// In words, for the Jobs screen.
    pub(crate) fn status(&self) -> String {
        match &self.state {
            State::Off => "not running".into(),
            State::Starting { .. } => "starting".into(),
            State::Ready { child, .. } => match self.idle_since {
                Some(since) => format!(
                    "idle (pid {}), stops in {} s",
                    child.id(),
                    IDLE_EXIT.saturating_sub(since.elapsed()).as_secs()
                ),
                None => format!("rendering (pid {})", child.id()),
            },
        }
    }

    /// Render `job` for the editor's job `id`, starting the worker if it is
    /// not running. A job that cannot be handed over fails at once.
    pub(crate) fn submit(&mut self, id: JobId, job: Job, events: &Sender<Event>) -> Option<Report> {
        self.idle_since = None;
        if matches!(self.state, State::Off) {
            if let Err(why) = self.start(events) {
                log::warn(format!("could not start the render worker: {why}"));
                return Some(Report::Done { id, result: Err(format!("the render worker could not start: {why}")) });
            }
        }
        match &mut self.state {
            State::Ready { writer, .. } => {
                self.next_number += 1;
                let number = self.next_number;
                if send(writer, &Message::Submit { id: number, job }) {
                    self.sent.insert(number, id);
                    None
                } else {
                    Some(Report::Done { id, result: Err("the render worker stopped".into()) })
                }
            }
            _ => {
                self.waiting.push_back((id, job));
                None
            }
        }
    }

    /// Stop a job. One still waiting for the worker ends here; one the worker
    /// has is cancelled there, and its `Done` comes back from it.
    pub(crate) fn cancel(&mut self, id: JobId) -> Option<Report> {
        if let Some(at) = self.waiting.iter().position(|(waiting, _)| *waiting == id) {
            self.waiting.remove(at);
            return Some(Report::Done { id, result: Err("cancelled".into()) });
        }
        if let Some(number) = self.sent.iter().find(|(_, ours)| **ours == id).map(|(number, _)| *number) {
            if let State::Ready { writer, .. } = &mut self.state {
                send(writer, &Message::Cancel { id: number });
            }
        }
        None
    }

    /// Something from worker `serial`. Returns what the queue should hear.
    pub(crate) fn handle(&mut self, serial: u64, event: WorkerEvent, events: &Sender<Event>) -> Vec<Report> {
        if serial != self.serial {
            return Vec::new();
        }
        match event {
            WorkerEvent::Connected(stream) => {
                let State::Starting { .. } = &self.state else { return Vec::new() };
                let Ok(reader) = stream.try_clone() else {
                    return self.stopped("its connection could not be read");
                };
                read_lines(reader, serial, events.clone());
                let State::Starting { child, .. } = std::mem::replace(&mut self.state, State::Off) else {
                    return Vec::new();
                };
                log::info(format!("the render worker is ready (pid {})", child.id()));
                self.state = State::Ready { child, writer: stream };
                let mut failed = Vec::new();
                for (id, job) in std::mem::take(&mut self.waiting) {
                    if let Some(report) = self.submit(id, job, events) {
                        failed.push(report);
                    }
                }
                failed
            }
            WorkerEvent::Message(Message::Progress { id: number, fraction, note }) => match self.sent.get(&number) {
                Some(&id) => vec![Report::Progress { id, fraction, note }],
                None => Vec::new(),
            },
            WorkerEvent::Message(Message::Done { id: number, result }) => match self.sent.remove(&number) {
                Some(id) => {
                    self.finished += 1;
                    // Between jobs: the moment to replace a worker that has
                    // done its share. The next job starts a fresh one.
                    if self.finished >= RECYCLE_AFTER && self.sent.is_empty() && self.waiting.is_empty() {
                        log::info(format!("the render worker rendered {} thumbnails; starting a fresh one", self.finished));
                        self.shut_down();
                    }
                    vec![Report::Done { id, result }]
                }
                None => Vec::new(),
            },
            WorkerEvent::Message(_) => Vec::new(),
            WorkerEvent::Closed => self.stopped("it closed the connection"),
        }
    }

    /// Stop a worker that has been idle long enough, and give up on one that
    /// never connected.
    pub(crate) fn tick(&mut self) -> Vec<Report> {
        match &mut self.state {
            State::Off => Vec::new(),
            State::Starting { child, since } => {
                if let Ok(Some(status)) = child.try_wait() {
                    return self.stopped(&format!("it exited while starting ({status})"));
                }
                if since.elapsed() > START_TIMEOUT {
                    return self.stopped("it did not connect in time");
                }
                Vec::new()
            }
            State::Ready { .. } => {
                if !self.sent.is_empty() || !self.waiting.is_empty() {
                    self.idle_since = None;
                    return Vec::new();
                }
                let since = *self.idle_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= IDLE_EXIT {
                    log::info("the render worker had nothing to render; stopping it");
                    self.shut_down();
                }
                Vec::new()
            }
        }
    }

    /// When the server should next look at the worker: an idle one is due to
    /// stop.
    pub(crate) fn due(&self) -> Option<Instant> {
        match self.state {
            State::Ready { .. } => self.idle_since.map(|t| t + IDLE_EXIT),
            State::Starting { .. } => Some(Instant::now() + Duration::from_secs(1)),
            State::Off => None,
        }
    }

    fn start(&mut self, events: &Sender<Event>) -> Result<(), String> {
        let exe = self.exe.clone().ok_or("the engine is not beside the companion")?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("no port to listen on: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let token = token();
        let child = Command::new(&exe)
            .args(["--companion", crate::render::WORKER_FLAG, "--companion-port"])
            .arg(port.to_string())
            .arg("--companion-token")
            .arg(&token)
            // Its log is read here, and only what is worth seeing goes on to
            // the editor's Console (see `pass_on`).
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("could not run {}: {e}", exe.display()));
        let mut child = child?;
        if let Some(out) = child.stdout.take() {
            pass_on(out);
        }
        if let Some(err) = child.stderr.take() {
            pass_on(err);
        }
        self.serial += 1;
        accept(listener, token, self.serial, events.clone());
        log::info(format!("starting the render worker (pid {})", child.id()));
        self.state = State::Starting { child, since: Instant::now() };
        Ok(())
    }

    /// The worker went: everything it had, or was waiting for, fails.
    fn stopped(&mut self, why: &str) -> Vec<Report> {
        if !matches!(self.state, State::Off) {
            log::warn(format!("the render worker stopped: {why}"));
        }
        self.shut_down();
        let ids: Vec<JobId> =
            std::mem::take(&mut self.sent).into_values().chain(self.waiting.drain(..).map(|(id, _)| id)).collect();
        ids.into_iter().map(|id| Report::Done { id, result: Err(format!("the render worker stopped: {why}")) }).collect()
    }

    /// Close the link, which ends the worker, and reap it off this thread.
    fn shut_down(&mut self) {
        self.idle_since = None;
        self.finished = 0;
        let child = match std::mem::replace(&mut self.state, State::Off) {
            State::Off => return,
            State::Starting { child, .. } => child,
            State::Ready { child, writer } => {
                let _ = writer.shutdown(Shutdown::Both);
                child
            }
        };
        reap(child);
    }
}

/// The argument that makes the engine a render worker. Must match
/// `renzora_companion::app::RENDER_WORKER_FLAG`.
pub(crate) const WORKER_FLAG: &str = "--render-worker";

/// Read the worker's output on a thread and pass on its warnings, errors and
/// crashes, as they are, to this program's output, which the editor shows.
///
/// Everything it logged went to the Console at first, and the capture code
/// logs four lines a thumbnail: a project's backlog was thousands of lines,
/// enough to push everything else out of the Console's history.
fn pass_on(output: impl std::io::Read + Send + 'static) {
    let _ = std::thread::Builder::new().name("companion-worker-log".into()).spawn(move || {
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { break };
            if worth_passing_on(&line) {
                let _ = writeln!(std::io::stdout(), "{line}");
            }
        }
    });
}

/// A warning, an error, or a panic: Bevy's log puts the level first or after
/// the time, and a panic is plain text on stderr.
fn worth_passing_on(line: &str) -> bool {
    line.split_whitespace().take(2).any(|word| word == "WARN" || word == "ERROR") || line.contains("panicked")
}

fn send(writer: &mut TcpStream, message: &Message) -> bool {
    writer.write_all(protocol::encode(message).as_bytes()).is_ok()
}

/// A token for one worker's connection, from the OS's random hash keys. Only
/// a process started with it on its command line can present it.
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// Accept the worker's connection on a thread, checking its hello's token.
fn accept(listener: TcpListener, token: String, serial: u64, events: Sender<Event>) {
    let spawned = std::thread::Builder::new().name("companion-worker-accept".into()).spawn(move || {
        // Anything else on the machine that finds the port is turned away;
        // only the right hello is passed on.
        while let Ok((stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(START_TIMEOUT));
            let Ok(reader) = stream.try_clone() else { continue };
            let mut line = String::new();
            if BufReader::new(reader).read_line(&mut line).is_err() {
                continue;
            }
            match protocol::decode(&line) {
                Some(Message::Hello { token: theirs, .. }) if theirs == token => {
                    let _ = stream.set_read_timeout(None);
                    let _ = events.send(Event::Worker(serial, WorkerEvent::Connected(stream)));
                    return;
                }
                _ => continue,
            }
        }
    });
    if let Err(e) = spawned {
        log::warn(format!("cannot listen for the render worker: {e}"));
    }
}

/// Read the worker's lines on a thread, ending with [`WorkerEvent::Closed`].
fn read_lines(stream: TcpStream, serial: u64, events: Sender<Event>) {
    let _ = std::thread::Builder::new().name("companion-worker-read".into()).spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            if let Some(message) = protocol::decode(&line) {
                if events.send(Event::Worker(serial, WorkerEvent::Message(message))).is_err() {
                    return;
                }
            }
        }
        let _ = events.send(Event::Worker(serial, WorkerEvent::Closed));
    });
}

/// Wait for a stopped worker to exit, and end it if it does not: an engine
/// that missed its link closing must not stay behind holding a GPU device.
fn reap(mut child: Child) {
    let _ = std::thread::Builder::new().name("companion-worker-reap".into()).spawn(move || {
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let _ = child.kill();
        let _ = child.wait();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_without_engine() -> RenderWorker {
        RenderWorker { exe: None, ..RenderWorker::beside_this_executable() }
    }

    #[test]
    fn with_no_engine_a_job_fails_at_once_for_the_editor_to_render() {
        let (events, _rx) = std::sync::mpsc::channel();
        let mut worker = worker_without_engine();
        assert!(!worker.available());
        let job = Job::Run { kind: "model_thumbnail".into(), payload: "{}".into(), target: "a.glb".into() };
        match worker.submit(7, job, &events) {
            Some(Report::Done { id: 7, result: Err(why) }) => assert!(why.contains("could not start")),
            _ => panic!("expected an immediate failure"),
        }
    }

    /// A job waiting for the worker to connect is cancelled where it waits,
    /// and a worker that stops fails what it had.
    #[test]
    fn waiting_jobs_cancel_here_and_fail_when_the_worker_stops() {
        let mut worker = worker_without_engine();
        let job = || Job::Run { kind: "material_thumbnail".into(), payload: "{}".into(), target: String::new() };
        worker.waiting.push_back((1, job()));
        worker.waiting.push_back((2, job()));
        worker.sent.insert(1, 3);
        assert!(matches!(worker.cancel(1), Some(Report::Done { id: 1, result: Err(_) })));
        let failed: Vec<JobId> = worker
            .stopped("test")
            .into_iter()
            .map(|r| match r {
                Report::Done { id, result: Err(_) } => id,
                _ => panic!("expected failures"),
            })
            .collect();
        assert_eq!(failed, [3, 2]);
    }

    /// The worker sees its own small numbers, never the queue's ids (the
    /// companion's own jobs carry a bit the worker reads as "mine, do not
    /// report"), and what it sends back is put under the queue's id.
    #[test]
    fn jobs_go_over_under_the_workers_numbers_and_come_back_under_ours() {
        let (events, _rx) = std::sync::mpsc::channel();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let theirs = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (ours, _) = listener.accept().unwrap();
        let child = Command::new(std::env::current_exe().unwrap()).arg("--list").stdout(Stdio::null()).spawn().unwrap();
        let mut worker = worker_without_engine();
        worker.state = State::Ready { child, writer: ours };

        let own = crate::cache::OWN_JOB | 5;
        let job = Job::Run { kind: "material_thumbnail".into(), payload: "{}".into(), target: String::new() };
        assert!(worker.submit(own, job, &events).is_none());
        let mut line = String::new();
        BufReader::new(theirs).read_line(&mut line).unwrap();
        let Some(Message::Submit { id: number, .. }) = protocol::decode(&line) else { panic!("expected a submit: {line}") };
        assert_eq!(number & crate::cache::OWN_JOB, 0);

        let done = WorkerEvent::Message(Message::Done { id: number, result: Ok("x.png".into()) });
        let reports = worker.handle(worker.serial, done, &events);
        assert!(matches!(reports.as_slice(), [Report::Done { id, result: Ok(_) }] if *id == own));
        worker.shut_down();
    }

    #[test]
    fn a_stale_worker_is_not_heard() {
        let (events, _rx) = std::sync::mpsc::channel();
        let mut worker = worker_without_engine();
        worker.serial = 2;
        let done = WorkerEvent::Message(Message::Done { id: 1, result: Ok(String::new()) });
        assert!(worker.handle(1, done, &events).is_empty());
    }

    #[test]
    fn only_warnings_errors_and_panics_reach_the_console() {
        assert!(!worth_passing_on("2026-11-15T14:34:56.295Z  INFO renzora_material_editor: Queued 'a.material'"));
        assert!(worth_passing_on("2026-11-15T14:34:56.295Z  WARN bevy_render: No previous asset to copy from"));
        assert!(worth_passing_on("ERROR renzora: could not open the project"));
        assert!(worth_passing_on("thread 'main' panicked at crates/x.rs:1:1:"));
        assert!(!worth_passing_on("a note mentioning WARN later on in the line"));
    }

    #[test]
    fn tokens_differ() {
        assert_ne!(token(), token());
        assert_eq!(token().len(), 32);
    }
}
