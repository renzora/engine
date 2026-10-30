//! The editor's side of the companion: start it, keep the link, report on it.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Child;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use renzora::core::console_log::{console_info, console_warn};
use renzora::{RenzoraShellExt, ShellActionInvoked, ShellActionItem};

use crate::protocol::{self, Job, JobId, Message, PROTOCOL_VERSION};

/// The spanner beside the Settings gear: its action id.
const ACTION_ID: &str = "companion.show";

/// How many times a companion that died on its own is started again in one
/// session. A crash on launch would otherwise restart forever.
const MAX_RESTARTS: u32 = 3;

/// Wait between a companion dying and starting the next one, so a crash on
/// launch does not spin.
const RESTART_DELAY_SECS: f32 = 2.0;

/// The companion as the editor sees it. Submit work with [`Companion::submit`].
#[derive(Resource, Default)]
pub struct Companion {
    state: LinkState,
    child: Option<Child>,
    /// Connections that passed the hello check, handed over by the listener
    /// thread. In a `Mutex` because a `Receiver` is not `Sync`.
    accepted: Option<Mutex<Receiver<TcpStream>>>,
    incoming: Option<Mutex<Receiver<Incoming>>>,
    writer: Option<TcpStream>,
    /// Submitted and not yet done, in submission order.
    jobs: BTreeMap<JobId, Running>,
    /// Lines that wait for a connection, sent in order once there is one.
    queued: Vec<Message>,
    next_id: JobId,
    restarts: u32,
    restart_in: Option<f32>,
    /// The echo sent on every connect, and when, to report the round trip.
    probe: Option<(JobId, Instant)>,
    /// Jobs other crates queued through `renzora::CompanionJobs`, by the id
    /// they went out under: their own id and kind, for the answer.
    external: std::collections::HashMap<JobId, (renzora::CompanionJobId, String)>,
    /// Answers for those, waiting for `bridge_jobs` to hand them back.
    external_done: Vec<renzora::CompanionJobDone>,
    external_progress: Vec<renzora::CompanionJobProgress>,
}

#[derive(Default, Clone, PartialEq, Debug)]
enum LinkState {
    #[default]
    Off,
    Starting,
    Connected,
    Stopped(String),
}

/// A job the editor is waiting on.
struct Running {
    label: &'static str,
    fraction: Option<f32>,
}

/// What the reader thread passes to the main thread.
enum Incoming {
    Message(Message),
    Closed,
}

impl Companion {
    /// Queue `job` for the companion and return its id. Sent at once when the
    /// companion is connected, otherwise as soon as it is.
    pub fn submit(&mut self, job: Job) -> JobId {
        self.next_id += 1;
        let id = self.next_id;
        self.jobs.insert(
            id,
            Running {
                label: job.label(),
                fraction: None,
            },
        );
        self.send(Message::Submit { id, job });
        id
    }

    /// Whether the companion is up and linked.
    pub fn is_connected(&self) -> bool {
        self.state == LinkState::Connected
    }

    fn send(&mut self, message: Message) {
        let Some(writer) = self.writer.as_mut() else {
            self.queued.push(message);
            return;
        };
        if let Err(e) = writer.write_all(protocol::encode(&message).as_bytes()) {
            // The reader thread sees the same broken socket and reports the
            // link closed; keep the line for the next companion.
            warn!("[companion] send failed: {e}");
            self.writer = None;
            self.queued.push(message);
        }
    }
}

pub(crate) fn build(app: &mut App) {
    // The editor profiles itself too, under this machine's key: the companion
    // finds it the same way it finds a game, and Play passes the same key on.
    app.insert_resource(renzora::profiling::ProfilingAccess {
        key: renzora::profiling::machine_key(),
        lan: false,
        name: "Editor".into(),
    });
    renzora::init_companion_jobs(app);
    app.init_resource::<Companion>()
        .add_systems(Startup, start_on_launch)
        .add_systems(Update, ((pump, bridge_jobs).chain(), show_on_action, report_play));
    // One quiet spanner beside the gear, and nothing in the status bar: the
    // companion is a tool you open, not something that needs watching.
    app.register_shell_tool(ShellActionItem {
        id: ACTION_ID,
        icon: "wrench",
        label: None,
        color: None,
        tooltip: || "Companion: profiler and background work".to_string(),
        order: 0,
    });
}

/// Start the companion with the editor. `RENZORA_NO_COMPANION` skips it, for
/// debugging the editor on its own.
fn start_on_launch(mut companion: ResMut<Companion>) {
    if std::env::var_os("RENZORA_NO_COMPANION").is_some() {
        info!("[companion] not started: RENZORA_NO_COMPANION is set");
        return;
    }
    launch(&mut companion);
}

/// Listen, then start `renzora --companion` pointed at the port.
fn launch(companion: &mut Companion) {
    let Some((port, token, accepted)) = listen() else {
        companion.state = LinkState::Stopped("could not open a port".into());
        return;
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            companion.state = LinkState::Stopped(format!("could not find the executable: {e}"));
            return;
        }
    };
    let mut command = std::process::Command::new(exe);
    command
        .arg("--companion")
        .arg("--companion-port")
        .arg(port.to_string())
        .arg("--companion-token")
        .arg(&token)
        // What it presents to the games it profiles.
        .arg("--profiling-key")
        .arg(renzora::profiling::machine_key())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            companion.state = LinkState::Stopped(format!("could not start: {e}"));
            warn!("[companion] could not start: {e}");
            return;
        }
    };
    renzora::child_process::tie_to_editor(&child, "the companion");
    if let Some(stdout) = child.stdout.take() {
        renzora::child_process::forward_to_console(stdout, "Companion");
    }
    if let Some(stderr) = child.stderr.take() {
        renzora::child_process::forward_to_console(stderr, "Companion");
    }
    companion.child = Some(child);
    companion.accepted = Some(Mutex::new(accepted));
    companion.incoming = None;
    companion.writer = None;
    companion.state = LinkState::Starting;
}

/// Bind a loopback port and accept the companion on a thread of its own.
///
/// Port 0 lets the OS pick a free one. Only a connection whose first line is a
/// hello with the right token and protocol version reaches the main thread.
fn listen() -> Option<(u16, String, Receiver<TcpStream>)> {
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(l) => l,
        Err(e) => {
            warn!("[companion] could not listen: {e}");
            return None;
        }
    };
    let port = listener.local_addr().ok()?.port();
    let token = format!(
        "{:016x}{:016x}",
        renzora::SceneEntityId::new_random().0,
        renzora::SceneEntityId::new_random().0
    );
    let (tx, rx) = channel();
    let expected = token.clone();
    std::thread::Builder::new()
        .name("companion-listen".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if accepts(&stream, &expected) && tx.send(stream).is_err() {
                    break;
                }
            }
        })
        .ok()?;
    Some((port, token, rx))
}

/// Whether `stream`'s first line is a hello with the right token and version.
/// Bounded by a timeout so a connection that never speaks cannot hold the
/// listener.
fn accepts(stream: &TcpStream, token: &str) -> bool {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Ok(reader) = stream.try_clone() else { return false };
    let mut line = String::new();
    if BufReader::new(reader).read_line(&mut line).is_err() {
        return false;
    }
    let _ = stream.set_read_timeout(None);
    match protocol::decode(&line) {
        Some(Message::Hello { token: t, version }) if t == token => {
            if version != PROTOCOL_VERSION {
                warn!(
                    "[companion] refused: it speaks protocol {version}, this editor {PROTOCOL_VERSION}"
                );
                return false;
            }
            true
        }
        _ => false,
    }
}

/// Read a connection's lines on a thread, ending with `Closed`.
fn read_lines(stream: TcpStream, tx: Sender<Incoming>) {
    std::thread::Builder::new()
        .name("companion-read".into())
        .spawn(move || {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                if let Some(message) = protocol::decode(&line) {
                    if tx.send(Incoming::Message(message)).is_err() {
                        return;
                    }
                }
            }
            let _ = tx.send(Incoming::Closed);
        })
        .ok();
}

/// Take in the connection, what arrived on it, and the process's own fate.
fn pump(time: Res<Time>, mut companion: ResMut<Companion>) {
    let companion = &mut *companion;

    let stream = companion
        .accepted
        .as_ref()
        .and_then(|rx| rx.lock().ok()?.try_recv().ok());
    if let Some(stream) = stream {
        connected(companion, stream);
    }

    let mut messages = Vec::new();
    let mut closed = false;
    if let Some(rx) = companion.incoming.as_ref().and_then(|rx| rx.lock().ok()) {
        while let Ok(incoming) = rx.try_recv() {
            match incoming {
                Incoming::Message(message) => messages.push(message),
                Incoming::Closed => closed = true,
            }
        }
    }
    for message in messages {
        received(companion, message);
    }
    if closed && companion.state == LinkState::Connected {
        companion.writer = None;
        companion.incoming = None;
        companion.state = LinkState::Stopped("lost the connection".into());
    }

    let exited = companion
        .child
        .as_mut()
        .and_then(|child| child.try_wait().ok().flatten());
    if let Some(status) = exited {
        companion.child = None;
        companion.accepted = None;
        companion.incoming = None;
        companion.writer = None;
        // Its jobs died with it. They are not resubmitted: a job that crashed
        // the companion would crash the next one too. Whoever submitted one is
        // told, so nothing waits forever.
        companion.jobs.clear();
        companion.probe = None;
        let lost: Vec<(renzora::CompanionJobId, String)> =
            companion.external.drain().map(|(_, job)| job).collect();
        for (id, kind) in lost {
            companion.external_done.push(renzora::CompanionJobDone {
                id,
                kind,
                result: Err("the companion stopped while running it".into()),
            });
        }
        let why = match status.code() {
            Some(code) => format!("exited with code {code}"),
            None => "was ended".to_string(),
        };
        if companion.restarts < MAX_RESTARTS {
            companion.restarts += 1;
            companion.restart_in = Some(RESTART_DELAY_SECS);
            console_warn("Companion", format!("The companion {why}; starting it again"));
        } else {
            console_warn(
                "Companion",
                format!("The companion {why}, and has stopped {MAX_RESTARTS} times; not starting it again"),
            );
        }
        companion.state = LinkState::Stopped(why);
    }

    if let Some(wait) = companion.restart_in.as_mut() {
        *wait -= time.delta_secs();
        if *wait <= 0.0 {
            companion.restart_in = None;
            launch(companion);
        }
    }
}

/// The companion passed the hello: start reading, flush the queue, and send the
/// echo that proves the round trip.
fn connected(companion: &mut Companion, stream: TcpStream) {
    let Ok(reader) = stream.try_clone() else {
        warn!("[companion] could not read from the connection");
        return;
    };
    let (tx, rx) = channel();
    read_lines(reader, tx);
    companion.incoming = Some(Mutex::new(rx));
    companion.writer = Some(stream);
    companion.state = LinkState::Connected;
    for message in std::mem::take(&mut companion.queued) {
        companion.send(message);
    }
    let id = companion.submit(Job::Echo {
        text: "ready".into(),
        millis: 0,
    });
    companion.probe = Some((id, Instant::now()));
}

fn received(companion: &mut Companion, message: Message) {
    match message {
        Message::Progress { id, fraction, note } => {
            if let Some(job) = companion.jobs.get_mut(&id) {
                job.fraction = fraction;
            }
            if let Some((external, _)) = companion.external.get(&id) {
                let external = *external;
                companion.external_progress.push(renzora::CompanionJobProgress { id: external, fraction, note });
            }
        }
        Message::Done { id, result } => {
            let label = companion.jobs.remove(&id).map(|j| j.label).unwrap_or("Job");
            if let Some((external, kind)) = companion.external.remove(&id) {
                companion.external_done.push(renzora::CompanionJobDone { id: external, kind, result });
                return;
            }
            if let Some((probe, sent)) = companion.probe {
                if probe == id {
                    companion.probe = None;
                    console_info(
                        "Companion",
                        format!(
                            "Companion ready (round trip {:.1} ms)",
                            sent.elapsed().as_secs_f64() * 1000.0
                        ),
                    );
                    return;
                }
            }
            if let Err(e) = result {
                console_warn("Companion", format!("{label} failed: {e}"));
            }
        }
        // Lines only the editor sends; a companion echoing them is ignored.
        Message::Hello { .. }
        | Message::Submit { .. }
        | Message::Show
        | Message::Cancel { .. }
        | Message::EditorState { .. } => {}
    }
}

/// Carry other crates' jobs to the companion and their answers back.
fn bridge_jobs(
    mut companion: ResMut<Companion>,
    mut jobs: ResMut<renzora::CompanionJobs>,
    mut done: MessageWriter<renzora::CompanionJobDone>,
    mut progress: MessageWriter<renzora::CompanionJobProgress>,
) {
    let connected = companion.is_connected();
    if jobs.available != connected {
        jobs.set_available(connected);
    }
    for request in jobs.drain() {
        let id = companion.submit(Job::Run {
            kind: request.kind.clone(),
            payload: request.payload,
            target: request.target,
        });
        companion.external.insert(id, (request.id, request.kind));
    }
    // A worker thread's job is answered through its handle; a system's
    // through the messages.
    for message in std::mem::take(&mut companion.external_progress) {
        if !jobs.progress_remote(message.id, message.fraction, &message.note) {
            progress.write(message);
        }
    }
    for message in std::mem::take(&mut companion.external_done) {
        if !jobs.finish_remote(message.id, &message.result) {
            done.write(message);
        }
    }
}

/// Tell the companion when Play starts and stops, so its GPU jobs wait while a
/// game is running. `LivePlaySession` exists exactly while one is.
fn report_play(
    session: Option<Res<renzora::LivePlaySession>>,
    mut companion: ResMut<Companion>,
    mut was: Local<Option<bool>>,
) {
    let playing = session.is_some();
    // Sent again after a reconnect, which clears `was`.
    if !companion.is_connected() {
        *was = None;
        return;
    }
    if *was != Some(playing) {
        *was = Some(playing);
        companion.send(Message::EditorState { playing });
    }
}

/// The spanner: bring the window up, or start a companion that gave up.
fn show_on_action(mut actions: MessageReader<ShellActionInvoked>, mut companion: ResMut<Companion>) {
    if !actions.read().any(|a| a.0 == ACTION_ID) {
        return;
    }
    match companion.state {
        LinkState::Connected => companion.send(Message::Show),
        LinkState::Starting => companion.queued.push(Message::Show),
        LinkState::Off | LinkState::Stopped(_) => {
            if companion.child.is_none() {
                companion.restarts = 0;
                companion.restart_in = None;
                launch(&mut companion);
                companion.queued.push(Message::Show);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Connect to `listener` and send `first_line`, returning the accepted end.
    fn dial(listener: &TcpListener, first_line: &str) -> TcpStream {
        let port = listener.local_addr().unwrap().port();
        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.write_all(first_line.as_bytes()).unwrap();
        // Kept open until the check has read the line.
        std::mem::forget(client);
        listener.accept().unwrap().0
    }

    fn hello(token: &str, version: u32) -> String {
        protocol::encode(&Message::Hello {
            token: token.into(),
            version,
        })
    }

    #[test]
    fn only_the_right_token_and_version_get_in() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        assert!(accepts(&dial(&listener, &hello("t0k", PROTOCOL_VERSION)), "t0k"));
        assert!(!accepts(&dial(&listener, &hello("guess", PROTOCOL_VERSION)), "t0k"));
        assert!(!accepts(&dial(&listener, &hello("t0k", PROTOCOL_VERSION + 1)), "t0k"));
        assert!(!accepts(&dial(&listener, "GET / HTTP/1.1\n"), "t0k"));
    }

    /// Work submitted before the companion connects waits, in order, and is not
    /// lost.
    #[test]
    fn jobs_submitted_early_are_queued_in_order() {
        let mut companion = Companion::default();
        let a = companion.submit(Job::Echo {
            text: "a".into(),
            millis: 0,
        });
        let b = companion.submit(Job::Echo {
            text: "b".into(),
            millis: 0,
        });
        assert!(a < b);
        let ids: Vec<JobId> = companion
            .queued
            .iter()
            .filter_map(|m| match m {
                Message::Submit { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec![a, b]);
        assert_eq!(companion.jobs.len(), 2);
    }
}
