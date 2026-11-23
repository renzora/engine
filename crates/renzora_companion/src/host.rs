//! The editor's side of the companion: start it, keep the link, report on it.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Child;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use renzora_log_api::{console_info, console_warn};
use renzora_shell_api::{RenzoraShellExt, ShellActionInvoked, ShellActionItem};

use crate::protocol::{self, Job, JobId, Message, PROTOCOL_VERSION};

/// The spanner beside the Settings gear: its action id.
const ACTION_ID: &str = "companion.show";

/// How many times a companion that died on its own is started again in one
/// session. A crash on launch would otherwise restart forever.
const MAX_RESTARTS: u32 = 3;

/// Wait between a companion dying and starting the next one, so a crash on
/// launch does not spin.
const RESTART_DELAY_SECS: f32 = 2.0;

/// A connection that passed the hello check, with the job kinds it runs.
type Accepted = (TcpStream, Vec<String>);

/// The companion as the editor sees it. Submit work with [`Companion::submit`].
#[derive(Resource, Default)]
pub struct Companion {
    state: LinkState,
    child: Option<Child>,
    /// Connections that passed the hello check, with the job kinds each said
    /// it runs, handed over by the listener thread. In a `Mutex` because a
    /// `Receiver` is not `Sync`.
    accepted: Option<Mutex<Receiver<Accepted>>>,
    /// The job kinds the connected companion runs, from its hello. Changed
    /// only on a new connection, which is when `bridge_jobs` passes them on.
    kinds: Vec<String>,
    kinds_changed: bool,
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
    /// Jobs other crates queued through `renzora_platform_api::CompanionJobs`, by the id
    /// they went out under: their own id and kind, for the answer.
    external: std::collections::HashMap<JobId, (renzora_platform_api::CompanionJobId, String)>,
    /// Answers for those, waiting for `bridge_jobs` to hand them back.
    external_done: Vec<renzora_platform_api::CompanionJobDone>,
    external_progress: Vec<renzora_platform_api::CompanionJobProgress>,
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
    app.insert_resource(renzora_diagnostics_api::profiling::ProfilingAccess {
        key: renzora_diagnostics_api::profiling::machine_key(),
        lan: false,
        name: "Editor".into(),
    });
    renzora_platform_api::init_companion_jobs(app);
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

/// The environment variable that turns the standalone companion off.
const SERVER_ENV: &str = "RENZORA_COMPANION_SERVER";

/// Which companion to start: the standalone `renzora-companion` beside this
/// executable, or this executable again with `--companion` when
/// [`SERVER_ENV`] is `0` or the file is not there.
///
/// The standalone one has the job queue, the project cache and the profiler
/// (with the Render and Debug screens), at a fraction of the engine
/// companion's memory. It does not yet render model, material or shape
/// thumbnails, import, install from the marketplace or export; while it runs,
/// the editor does those itself, as it did before there was a companion. Both
/// speak the same protocol and each lists the job kinds it runs, so the
/// editor never asks for one the companion cannot do (see
/// `docs/r1-alpha8/editor-dev/standalone-companion.md`).
fn companion_command(exe: &std::path::Path) -> std::process::Command {
    let declined = std::env::var_os(SERVER_ENV).is_some_and(|v| v == "0");
    let server = exe.with_file_name(format!("renzora-companion{}", std::env::consts::EXE_SUFFIX));
    if !declined && server.is_file() {
        return std::process::Command::new(server);
    }
    if declined {
        info!("[companion] {SERVER_ENV}=0: starting the engine companion");
    } else {
        warn!("[companion] {} is not there; starting the engine companion", server.display());
    }
    let mut command = std::process::Command::new(exe);
    command.arg("--companion");
    command
}

/// Listen, then start the companion pointed at the port.
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
    let mut command = companion_command(&exe);
    command
        .arg("--companion-port")
        .arg(port.to_string())
        .arg("--companion-token")
        .arg(&token)
        // What it presents to the games it profiles.
        .arg("--profiling-key")
        .arg(renzora_diagnostics_api::profiling::machine_key())
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
    renzora_platform_api::child_process::tie_to_editor(&child, "the companion");
    if let Some(stdout) = child.stdout.take() {
        renzora_platform_api::child_process::forward_to_console(stdout, "Companion");
    }
    if let Some(stderr) = child.stderr.take() {
        renzora_platform_api::child_process::forward_to_console(stderr, "Companion");
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
fn listen() -> Option<(u16, String, Receiver<Accepted>)> {
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
        renzora_scene_api::SceneEntityId::new_random().0,
        renzora_scene_api::SceneEntityId::new_random().0
    );
    let (tx, rx) = channel();
    let expected = token.clone();
    std::thread::Builder::new()
        .name("companion-listen".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if let Some(kinds) = accepts(&stream, &expected) {
                    if tx.send((stream, kinds)).is_err() {
                        break;
                    }
                }
            }
        })
        .ok()?;
    Some((port, token, rx))
}

/// The job kinds a companion runs, when `stream`'s first line is a hello with
/// the right token and version; `None` otherwise. Bounded by a timeout so a
/// connection that never speaks cannot hold the listener.
fn accepts(stream: &TcpStream, token: &str) -> Option<Vec<String>> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let reader = stream.try_clone().ok()?;
    let mut line = String::new();
    BufReader::new(reader).read_line(&mut line).ok()?;
    let _ = stream.set_read_timeout(None);
    match protocol::decode(&line) {
        Some(Message::Hello { token: t, version, kinds }) if t == token => {
            if version != PROTOCOL_VERSION {
                warn!(
                    "[companion] refused: it speaks protocol {version}, this editor {PROTOCOL_VERSION}"
                );
                return None;
            }
            Some(kinds)
        }
        _ => None,
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
    if let Some((stream, kinds)) = stream {
        companion.kinds = kinds;
        companion.kinds_changed = true;
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
        let lost: Vec<(renzora_platform_api::CompanionJobId, String)> =
            companion.external.drain().map(|(_, job)| job).collect();
        for (id, kind) in lost {
            companion.external_done.push(renzora_platform_api::CompanionJobDone {
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
                companion.external_progress.push(renzora_platform_api::CompanionJobProgress { id: external, fraction, note });
            }
        }
        Message::Done { id, result } => {
            let label = companion.jobs.remove(&id).map(|j| j.label).unwrap_or("Job");
            if let Some((external, kind)) = companion.external.remove(&id) {
                companion.external_done.push(renzora_platform_api::CompanionJobDone { id: external, kind, result });
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
    mut jobs: ResMut<renzora_platform_api::CompanionJobs>,
    mut done: MessageWriter<renzora_platform_api::CompanionJobDone>,
    mut progress: MessageWriter<renzora_platform_api::CompanionJobProgress>,
) {
    let connected = companion.is_connected();
    if std::mem::take(&mut companion.kinds_changed) {
        jobs.set_kinds(companion.kinds.iter().cloned());
    }
    if jobs.available != connected {
        jobs.set_available(connected);
    }
    // Launched and not linked yet, or about to be started again.
    let starting = companion.state == LinkState::Starting || companion.restart_in.is_some();
    if jobs.starting != starting {
        jobs.starting = starting;
    }
    for request in jobs.drain() {
        let id = companion.submit(Job::Run {
            kind: request.kind.clone(),
            payload: request.payload,
            target: request.target,
        });
        companion.external.insert(id, (request.id, request.kind));
    }
    for cancelled in jobs.take_cancels() {
        let ours = companion.external.iter().find(|(_, (theirs, _))| *theirs == cancelled).map(|(id, _)| *id);
        if let Some(id) = ours {
            companion.send(Message::Cancel { id });
        }
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
/// With it goes the Render thumbnails setting, so turning that off stops the
/// companion's render worker.
fn report_play(
    session: Option<Res<renzora_project_api::LivePlaySession>>,
    thumbnails: Option<Res<renzora_project_api::ThumbnailRendering>>,
    mut companion: ResMut<Companion>,
    mut was: Local<Option<(bool, bool)>>,
) {
    let playing = session.is_some();
    let thumbnails = thumbnails.is_none_or(|t| t.0);
    // Sent again after a reconnect, which clears `was`.
    if !companion.is_connected() {
        *was = None;
        return;
    }
    if *was != Some((playing, thumbnails)) {
        *was = Some((playing, thumbnails));
        companion.send(Message::EditorState { playing, thumbnails });
    }
}

/// The spanner: bring the window up, or start a companion that gave up.
fn show_on_action(mut actions: MessageReader<ShellActionInvoked>, mut companion: ResMut<Companion>) {
    if !actions.read().any(|a| a.0 == ACTION_ID) {
        return;
    }
    allow_companion_foreground();
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

/// Let the companion's window come to the front when it opens.
///
/// Windows keeps a background process from taking the foreground, and the
/// companion (and the window process it starts) is always in the background:
/// the click was the editor's. Only the process that got the click can lend
/// that right, so the editor lends it here, just before asking. Elsewhere the
/// window manager decides, and there is nothing to lend.
fn allow_companion_foreground() {
    #[cfg(target_os = "windows")]
    {
        /// Any process may take the foreground, until the next input event.
        const ASFW_ANY: u32 = u32::MAX;
        #[link(name = "user32")]
        unsafe extern "system" {
            fn AllowSetForegroundWindow(process_id: u32) -> i32;
        }
        // SAFETY: a documented constant; failing only leaves the window behind.
        unsafe { AllowSetForegroundWindow(ASFW_ANY) };
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
            kinds: vec!["texture_thumbnail".into()],
        })
    }

    #[test]
    fn only_the_right_token_and_version_get_in() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        assert_eq!(
            accepts(&dial(&listener, &hello("t0k", PROTOCOL_VERSION)), "t0k"),
            Some(vec!["texture_thumbnail".to_string()])
        );
        assert_eq!(accepts(&dial(&listener, &hello("guess", PROTOCOL_VERSION)), "t0k"), None);
        assert_eq!(accepts(&dial(&listener, &hello("t0k", PROTOCOL_VERSION + 1)), "t0k"), None);
        assert_eq!(accepts(&dial(&listener, "GET / HTTP/1.1\n"), "t0k"), None);
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
