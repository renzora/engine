//! What the window shows, and what it asks for: the lines between the
//! companion and its window process.
//!
//! The window is a process of its own (see [`crate::window`]) because a GL
//! driver, once loaded, stays loaded until its process exits: drawing the
//! window inside the companion left about 90 MB behind after it closed. So the
//! server tells the window what it needs, and the window sends back a
//! [`Command`] when something is pressed. The window never sees the server's
//! own types, only these.
//!
//! # The two streams
//!
//! - **Server to window** (the window's stdin) is a sequence of messages, each
//!   one tag byte and then a body: [`TAG_LINE`], a JSON [`ToWindow`] ending in
//!   a newline; [`TAG_PACKET`], one profiler wire packet as the game sent it;
//!   [`TAG_OVERLAPPED`], a frame packet recorded while a job ran. The profile
//!   goes over in the wire's own binary form because it is most of the
//!   traffic: as JSON it was megabytes a second to build and parse.
//! - **Window to server** (its stdout) is JSON lines of [`Command`]s.

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use renzora_companion_proto::protocol::JobId;
use renzora_companion_proto::wire::GamePacket;

use crate::cache::{thumbnailed, Thumbnailed};
use crate::games::{ConnState, GameInfo};
use crate::jobs::held;
use crate::server::State;

/// A JSON [`ToWindow`] line follows.
pub(crate) const TAG_LINE: u8 = 0;
/// A profiler packet follows.
pub(crate) const TAG_PACKET: u8 = 1;
/// A frame packet follows, recorded while the companion ran a job.
pub(crate) const TAG_OVERLAPPED: u8 = 2;

/// Waiting jobs listed by name; the rest are counted.
const WAITING_SHOWN: usize = 50;
/// Finished jobs listed.
const FINISHED_SHOWN: usize = 30;

/// Everything the window draws apart from the profile itself.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub(crate) struct View {
    /// What the held jobs wait for, when any job is held.
    pub held: Option<String>,
    pub running: Vec<Row>,
    /// The first [`WAITING_SHOWN`], in the order they will run.
    pub waiting: Vec<Row>,
    pub waiting_total: usize,
    /// Newest first.
    pub finished: Vec<Done>,
    pub cache: CacheView,
    pub profiling: ProfilingView,
    /// The render worker, in words: not running, starting, rendering, idle.
    /// `None` when there is no engine beside the companion to start.
    pub render_worker: Option<String>,
    /// A short message for the footer: where a capture was saved, why one
    /// could not be opened.
    pub notice: Option<String>,
}

/// A running or waiting job. For a waiting one, `note` says what it waits for.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub(crate) struct Row {
    pub id: JobId,
    pub label: String,
    pub target: String,
    pub fraction: Option<f32>,
    pub note: String,
    /// Waiting only for a recording, so "Run anyway" can start it.
    pub forceable: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub(crate) struct Done {
    pub label: String,
    pub target: String,
    pub result: Result<String, String>,
    pub took_ms: f64,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub(crate) struct CacheView {
    pub root: Option<String>,
    pub walking: bool,
    pub files: usize,
    pub bytes: u64,
    pub models: usize,
    pub materials: usize,
    pub watch_error: Option<String>,
    /// Thumbnail cache: files and bytes, once measured.
    pub thumbnails: Option<(usize, u64)>,
    pub pruned: usize,
    /// Thumbnails are rendered ahead of time (there is a render worker).
    pub render_ahead: bool,
    /// Files still to look at for a missing thumbnail.
    pub backlog: usize,
    /// Thumbnails queued ahead of time this session.
    pub prewarmed: usize,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
pub(crate) struct ProfilingView {
    /// Every game heard from lately, by address.
    pub games: Vec<GameInfo>,
    /// The game connected to (or last tried).
    pub game: Option<GameInfo>,
    pub state: ConnState,
    pub recording: bool,
    /// Frames the companion holds, which can be more than the window does.
    pub frames: usize,
}

/// Server to window, as a [`TAG_LINE`].
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(tag = "to_window", rename_all = "snake_case")]
pub(crate) enum ToWindow {
    View(Box<View>),
    /// The spanner was pressed with the window already open.
    Focus,
    /// The capture was replaced (a new game, an opened file): start over.
    /// The packets that follow fill it again.
    CaptureReset { source: String, from_file: Option<String> },
}

/// Window to server.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "command", rename_all = "snake_case")]
pub(crate) enum Command {
    CancelJob { id: JobId },
    /// Start a job a recording is holding.
    RunAnyway { id: JobId },
    ClearThumbnails,
    /// Look at every model and material again for a missing thumbnail.
    RenderMissing,
    Connect { game: GameInfo },
    Disconnect,
    Record { on: bool },
    /// The probes the Debug screen is showing; empty when it is not shown.
    Watch { probes: Vec<String> },
    /// A probe's toggle or button was used.
    ProbeAct { probe: String, item: String, value: Option<bool> },
    SaveCapture,
    OpenCapture { path: String },
}

/// One message from the server, as the window reads it.
pub(crate) enum Received {
    Line(ToWindow),
    Packet(GamePacket, bool),
}

/// Write a [`ToWindow`] line.
pub(crate) fn write_line(out: &mut impl Write, message: &ToWindow) -> io::Result<()> {
    let line = serde_json::to_string(message).map_err(io::Error::other)?;
    out.write_all(&[TAG_LINE])?;
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")
}

/// Write an encoded profiler packet.
pub(crate) fn write_packet(out: &mut impl Write, encoded: &[u8], overlapped: bool) -> io::Result<()> {
    out.write_all(&[if overlapped { TAG_OVERLAPPED } else { TAG_PACKET }])?;
    out.write_all(encoded)
}

/// Read one message. `Ok(None)` when the server closed the stream.
pub(crate) fn read(from: &mut impl BufRead) -> io::Result<Option<Received>> {
    let mut tag = [0u8; 1];
    loop {
        match from.read(&mut tag) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    match tag[0] {
        TAG_LINE => {
            let mut line = String::new();
            if from.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            serde_json::from_str(line.trim_end()).map(|m| Some(Received::Line(m))).map_err(io::Error::other)
        }
        TAG_PACKET | TAG_OVERLAPPED => match GamePacket::read(from)? {
            Some(packet) => Ok(Some(Received::Packet(packet, tag[0] == TAG_OVERLAPPED))),
            None => Ok(None),
        },
        other => Err(io::Error::other(format!("unknown message tag {other}"))),
    }
}

impl View {
    pub(crate) fn of(state: &State) -> Self {
        let queue = &state.queue;
        let recording = state.profiling.recording;
        let running = queue
            .running
            .values()
            .map(|(e, _)| Row {
                id: e.id,
                label: e.label.to_string(),
                target: e.target.clone(),
                fraction: e.fraction,
                note: e.note.clone(),
                forceable: false,
            })
            .collect();
        let waiting = queue
            .waiting()
            .take(WAITING_SHOWN)
            .map(|(e, job)| {
                let why = held(e, job, &state.editor, recording);
                Row {
                    id: e.id,
                    label: e.label.to_string(),
                    target: e.target.clone(),
                    fraction: None,
                    note: match why {
                        Some(why) => format!("waits for {why}"),
                        None => "waits for a free worker".to_string(),
                    },
                    forceable: why == Some("recording to stop"),
                }
            })
            .collect();
        let finished = queue
            .finished
            .iter()
            .take(FINISHED_SHOWN)
            .map(|f| Done { label: f.label.to_string(), target: f.target.clone(), result: f.result.clone(), took_ms: f.took_ms })
            .collect();

        let cache = &state.cache;
        let count = |kind| cache.files.keys().filter(|k| thumbnailed(k) == Some(kind)).count();
        let profiling = &state.profiling;
        Self {
            held: queue.waiting().find_map(|(e, job)| held(e, job, &state.editor, recording)).map(str::to_string),
            running,
            waiting,
            waiting_total: queue.waiting().count(),
            finished,
            cache: CacheView {
                root: cache.root.as_ref().map(|r| r.display().to_string()),
                walking: cache.is_walking(),
                files: cache.files.len(),
                bytes: cache.files.values().map(|f| f.size).sum(),
                models: count(Thumbnailed::Model),
                materials: count(Thumbnailed::Material),
                watch_error: cache.watch_error.clone(),
                thumbnails: cache.thumbnails,
                pruned: cache.pruned,
                render_ahead: cache.render_ahead,
                backlog: cache.backlog.len(),
                prewarmed: cache.prewarmed,
            },
            profiling: ProfilingView {
                games: profiling.games.values().map(|(g, _)| g.clone()).collect(),
                game: profiling.game.clone(),
                state: profiling.state.clone(),
                recording,
                frames: profiling.capture.frames.len(),
            },
            render_worker: state.render.available().then(|| state.render.status()),
            notice: state.notice.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_companion_proto::frame::ProfileFrame;
    use renzora_companion_proto::protocol::Job;

    #[test]
    fn a_view_lists_waiting_jobs_with_why() {
        let mut state = State::default();
        state.editor.playing = true;
        state.queue.submit(1, Job::Run { kind: "model_thumbnail".into(), payload: "{}".into(), target: "a.glb".into() });
        state.queue.submit(2, Job::Echo { text: "x".into(), millis: 0 });
        let view = View::of(&state);
        assert_eq!(view.waiting_total, 2);
        assert_eq!(view.held.as_deref(), Some("Play to stop"));
        let notes: Vec<&str> = view.waiting.iter().map(|r| r.note.as_str()).collect();
        assert!(notes.contains(&"waits for Play to stop"));
        assert!(notes.contains(&"waits for a free worker"));
        assert!(view.waiting.iter().all(|r| !r.forceable));
    }

    /// Lines and packets interleave on one stream and read back in order.
    #[test]
    fn the_stream_reads_back_in_order() {
        let mut out = Vec::new();
        write_line(&mut out, &ToWindow::Focus).unwrap();
        let frame = GamePacket::Frame(ProfileFrame { index: 9, ..Default::default() });
        write_packet(&mut out, &frame.encode(), true).unwrap();
        write_line(&mut out, &ToWindow::CaptureReset { source: "Editor".into(), from_file: None }).unwrap();
        let mut input = std::io::BufReader::new(out.as_slice());
        assert!(matches!(read(&mut input).unwrap(), Some(Received::Line(ToWindow::Focus))));
        match read(&mut input).unwrap() {
            Some(Received::Packet(GamePacket::Frame(f), true)) => assert_eq!(f.index, 9),
            _ => panic!("expected the overlapped frame"),
        }
        assert!(matches!(read(&mut input).unwrap(), Some(Received::Line(ToWindow::CaptureReset { .. }))));
        assert!(read(&mut input).unwrap().is_none());
    }

    #[test]
    fn commands_survive_the_pipe() {
        for command in [
            Command::CancelJob { id: 4 },
            Command::ClearThumbnails,
            Command::Record { on: true },
            Command::Watch { probes: vec!["performance".into()] },
            Command::OpenCapture { path: "C:\\a b\\c.rzprof".into() },
        ] {
            let line = serde_json::to_string(&command).unwrap();
            assert!(!line.contains('\n'));
            assert_eq!(serde_json::from_str::<Command>(&line).unwrap(), command);
        }
    }
}
