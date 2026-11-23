//! The companion's state and the one thread that changes it.
//!
//! Everything that happens arrives here as an [`Event`] on one channel: a
//! line from the editor, a worker's progress, the watcher's changes, a walk or
//! a measure landing, a game announcing itself, a packet from the game being
//! profiled, a button in the window. The server handles them in order and is
//! the only thing that holds [`State`]; the window, a process of its own, is
//! sent a [`View`] of it after each batch that changed it, and the profile's
//! packets as they arrive. That is the whole concurrency story: no lock,
//! because nothing else reads the state.

use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use renzora_companion_proto::project_files::IndexedFile;
use renzora_companion_proto::protocol::{self, Job, Message};

use crate::analysis;
use crate::cache::{self, ProjectCache, OWN_JOB};
use crate::games::{GameInfo, Incoming, Profiling};
use crate::jobs::{EditorState, JobQueue, Report};
use crate::log;
use crate::render::{RenderWorker, WorkerEvent};
use crate::view::{Command, View};
use crate::window::Window;

/// Something that happened, for the server to act on.
pub(crate) enum Event {
    /// A line from the editor.
    Editor(Message),
    /// The editor closed the link: it quit or crashed.
    EditorClosed,
    /// A worker's progress or result.
    Report(Report),
    /// The watcher saw these project-relative paths change under a root.
    Changed(PathBuf, Vec<String>),
    /// A project walk finished.
    Walked(PathBuf, Vec<IndexedFile>),
    /// The thumbnail cache of a project was measured: files and bytes.
    Measured(PathBuf, (usize, u64)),
    /// A game's beacon.
    Heard(GameInfo),
    /// From the connection with this serial to the profiled game.
    Profile(u64, Incoming),
    /// From the render worker started with this serial.
    Worker(u64, WorkerEvent),
    /// A button in the window.
    Ui(Command),
    /// The window process with this id was closed.
    WindowClosed(u32),
}

/// Everything the companion knows.
#[derive(Default)]
pub(crate) struct State {
    pub queue: JobQueue,
    pub cache: ProjectCache,
    pub editor: EditorState,
    pub profiling: Profiling,
    pub render: RenderWorker,
    /// The footer's message, from the last thing pressed that has one.
    pub notice: Option<String>,
}

/// How long the server sleeps with nothing to do. It wakes on any event long
/// before this; the timeout bounds how late a settled burst of file changes
/// is acted on (which [`ProjectCache::due`] shortens when one waits) and how
/// late a game that went quiet leaves the list.
const IDLE: Duration = Duration::from_secs(1);

/// Run the server until the editor goes. Never returns: the process exits
/// with the link.
pub(crate) fn run(incoming: Receiver<Event>, events: Sender<Event>, mut writer: TcpStream, key: String) -> ! {
    let mut state = State { profiling: Profiling::new(key), ..State::default() };
    state.cache.render_ahead = state.render.available();
    // The window process, while it is open: most of the time, none.
    let mut window: Option<Window> = None;
    // Which capture the window was given; a new one means starting it over.
    let mut window_generation = 0;
    loop {
        let due = [state.cache.due(), state.render.due()].into_iter().flatten().min();
        let timeout =
            due.map_or(IDLE, |due| due.saturating_duration_since(Instant::now()).max(Duration::from_millis(10)).min(IDLE));
        let mut batch = Vec::new();
        match incoming.recv_timeout(timeout) {
            Ok(event) => batch.push(event),
            Err(RecvTimeoutError::Timeout) => {}
            // Every sender is gone, which cannot happen while this thread
            // holds one; treat it as the editor going.
            Err(RecvTimeoutError::Disconnected) => batch.push(Event::EditorClosed),
        }
        batch.extend(incoming.try_iter());

        for event in batch {
            handle(&mut state, &mut window, event, &events);
        }
        state.profiling.forget_quiet();
        state.cache.settle(&events);
        // Thumbnails rendered ahead of time join the queue only while it has
        // nothing the editor asked for.
        state.cache.drain_backlog(state.queue.idle(), &events);
        for (id, job) in state.cache.take_jobs() {
            state.queue.submit(id, job);
        }
        for report in state.render.tick() {
            state.queue.report(report);
        }
        state.queue.start(&state.editor, state.profiling.recording, &events);
        for (id, job) in state.queue.take_render() {
            if let Some(report) = state.render.submit(id, job, &events) {
                state.queue.report(report);
            }
        }
        let mut own_finished = false;
        for report in state.queue.take_reports() {
            let message = match report {
                Report::Progress { id, fraction, note } => Message::Progress { id, fraction, note },
                Report::Done { id, result } => Message::Done { id, result },
            };
            // The companion's own jobs are nobody's in the editor.
            if let Message::Progress { id, .. } | Message::Done { id, .. } = &message {
                if id & OWN_JOB != 0 {
                    own_finished |= matches!(message, Message::Done { .. });
                    continue;
                }
            }
            if let Err(e) = writer.write_all(protocol::encode(&message).as_bytes()) {
                log::warn(format!("could not report to the editor: {e}"));
            }
        }
        if own_finished && state.cache.backlog.is_empty() {
            state.cache.measure(&events);
        }
        // Built only while the window is open, and sent only when it changed.
        if let Some(open) = window.as_mut() {
            if window_generation != state.profiling.generation {
                window_generation = state.profiling.generation;
                open.invalidate();
            }
            let alive = (!open.behind() || open.sync(&state.profiling.capture)) && open.show(View::of(&state));
            if !alive {
                window = None;
            }
        }
    }
}

fn handle(state: &mut State, window: &mut Option<Window>, event: Event, events: &Sender<Event>) {
    match event {
        Event::Editor(message) => match message {
            // Questions about the capture are answered from it directly; see
            // `analysis`.
            Message::Submit { id, job: Job::Run { kind, payload, .. } } if analysis::is_question(&kind) => {
                let answer = analysis::answer(&kind, &payload, &mut state.profiling, events);
                state.queue.answer(id, answer);
            }
            // The project listing, which is also how the companion learns
            // which project is open; see `cache`.
            Message::Submit { id, job: Job::Run { kind, payload, .. } } if cache::is_listing(&kind) => {
                match state.cache.ask(id, &payload, events) {
                    Ok(Some(answer)) => state.queue.answer(id, Ok(answer)),
                    Ok(None) => {}
                    Err(why) => state.queue.answer(id, Err(why)),
                }
            }
            Message::Submit { id, job } if state.editor.thumbnails_off && job.uses_gpu() => {
                state.queue.answer(id, Err(THUMBNAILS_OFF.into()));
            }
            Message::Submit { id, job } => state.queue.submit(id, job),
            Message::Cancel { id } => cancel(state, id),
            Message::EditorState { playing, thumbnails } => {
                state.editor.playing = playing;
                set_thumbnails(state, thumbnails);
            }
            Message::Show => show(window, events),
            // Lines only the companion sends.
            Message::Hello { .. } | Message::Progress { .. } | Message::Done { .. } => {}
        },
        Event::EditorClosed => {
            // On Windows the editor's job object has usually ended this
            // process already; elsewhere this is what does. The window sees
            // its stdin close and goes too.
            log::info("the editor closed the link; exiting");
            std::process::exit(0);
        }
        Event::Report(report) => state.queue.report(report),
        Event::Changed(root, paths) => state.cache.changed(&root, paths),
        Event::Walked(root, files) => {
            for (id, answer) in state.cache.walked(&root, files) {
                state.queue.answer(id, answer);
            }
        }
        Event::Measured(root, size) => state.cache.measured(&root, size),
        Event::Heard(game) => state.profiling.heard(game),
        Event::Profile(serial, incoming) => {
            if let Some(packet) = state.profiling.incoming(serial, incoming) {
                // A frame that arrives while a job runs shared the machine
                // with it. The game's clock is not ours, so arrival is the
                // measure: a local stream arrives within a millisecond.
                let overlapped = state.queue.busy();
                if let Some(open) = window.as_mut().filter(|w| !w.behind()) {
                    open.forward(&packet, overlapped);
                }
                state.profiling.capture.receive(packet, overlapped);
            }
        }
        Event::Worker(serial, event) => {
            for report in state.render.handle(serial, event, events) {
                state.queue.report(report);
            }
        }
        Event::Ui(command) => match command {
            Command::CancelJob { id } => cancel(state, id),
            Command::RunAnyway { id } => state.queue.run_anyway(id),
            Command::ClearThumbnails => state.cache.clear_thumbnails(events),
            Command::RenderMissing => state.cache.render_missing(),
            Command::Connect { game } => state.profiling.connect(game, events),
            Command::Disconnect => state.profiling.disconnect(),
            Command::Record { on } => state.profiling.record(on),
            Command::Watch { probes } => state.profiling.watch(probes),
            Command::ProbeAct { probe, item, value } => state.profiling.probe_act(probe, item, value),
            Command::SaveCapture => state.notice = Some(state.profiling.capture.save_to_folder()),
            Command::OpenCapture { path } => state.notice = Some(state.profiling.open_capture(path.as_ref())),
        },
        // Only the current window's: an earlier one's close can land after a
        // new one opened.
        Event::WindowClosed(pid) => {
            if window.as_ref().is_some_and(|w| w.pid() == pid) {
                *window = None;
                // Nobody is looking at the probes any more; the game stops
                // building their reports.
                state.profiling.watch(Vec::new());
            }
        }
    }
}

/// Stop a job, wherever it is: waiting in the queue, on a worker thread, or
/// with the render worker.
/// Why a thumbnail job fails while the editor has thumbnails turned off.
const THUMBNAILS_OFF: &str = "thumbnail rendering is turned off in the editor's settings";

/// Apply the editor's Render thumbnails setting. Off: forget the render-ahead
/// backlog, fail every thumbnail job waiting or running, and stop the render
/// worker, which is what gives its memory and GPU back. On: render ahead again
/// (when a worker can be started at all), from a fresh look at the project.
fn set_thumbnails(state: &mut State, on: bool) {
    let off = !on;
    if state.editor.thumbnails_off == off {
        return;
    }
    state.editor.thumbnails_off = off;
    if off {
        state.cache.render_ahead = false;
        state.cache.backlog.clear();
        let waiting: Vec<protocol::JobId> =
            state.queue.waiting().filter(|(_, job)| job.uses_gpu()).map(|(entry, _)| entry.id).collect();
        for id in waiting {
            cancel(state, id);
        }
        for report in state.render.stop(THUMBNAILS_OFF) {
            state.queue.report(report);
        }
    } else {
        state.cache.render_ahead = state.render.available();
        state.cache.render_missing();
    }
}

fn cancel(state: &mut State, id: protocol::JobId) {
    state.queue.cancel(id);
    if let Some(report) = state.render.cancel(id) {
        state.queue.report(report);
    }
}

/// The spanner: bring the open window to the front, or open one.
fn show(window: &mut Option<Window>, events: &Sender<Event>) {
    if let Some(open) = window.as_mut() {
        if open.focus() {
            return;
        }
    }
    *window = match Window::open(events) {
        Ok(opened) => Some(opened),
        Err(e) => {
            log::warn(e);
            None
        }
    };
}
