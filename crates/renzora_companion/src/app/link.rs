//! The link to the editor that started this companion.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowCloseRequested};

use super::analysis;
use super::jobs::{JobQueue, Report};
use crate::protocol::{self, Job, Message, PROTOCOL_VERSION};

/// How long to wait for the editor to answer before giving up. The editor
/// started this process a moment ago with the port it is listening on, so a
/// long silence means it is gone.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The link to the editor.
#[derive(Resource)]
pub(crate) struct Link {
    writer: TcpStream,
    /// In a `Mutex` because a `Receiver` is not `Sync`.
    incoming: Mutex<Receiver<Incoming>>,
}

enum Incoming {
    Message(Message),
    Closed,
}

/// What the editor said about itself, which decides when GPU jobs may run.
#[derive(Resource, Default)]
pub(crate) struct EditorState {
    /// Play is running a game: GPU jobs wait so they never compete with it.
    pub playing: bool,
}

/// Connect to the editor and install the link. False when there is no editor
/// to talk to, in which case an exit has been scheduled and nothing else
/// should be built.
pub(crate) fn build(app: &mut App) -> bool {
    let Some((port, token)) = args() else {
        error!("[companion] started without --companion-port and --companion-token; exiting");
        exit_at_startup(app, 2);
        return false;
    };
    let writer = match connect(port, &token) {
        Ok(stream) => stream,
        Err(e) => {
            error!("[companion] could not reach the editor on port {port}: {e}; exiting");
            exit_at_startup(app, 3);
            return false;
        }
    };
    let Ok(reader) = writer.try_clone() else {
        error!("[companion] could not read from the editor; exiting");
        exit_at_startup(app, 4);
        return false;
    };
    let (tx, rx) = channel();
    read_lines(reader, tx);
    app.insert_resource(Link {
        writer,
        incoming: Mutex::new(rx),
    })
    .init_resource::<EditorState>()
    .add_systems(Update, (pump, hide_on_close));
    info!("[companion] connected to the editor on port {port}");
    true
}

fn exit_at_startup(app: &mut App, code: u8) {
    app.add_systems(Startup, move |mut exit: MessageWriter<AppExit>| {
        exit.write(AppExit::from_code(code));
    });
}

/// `--companion-port <n> --companion-token <t>`, as the editor passes them.
fn args() -> Option<(u16, String)> {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    Some((value("--companion-port")?.parse().ok()?, value("--companion-token")?))
}

/// Connect and say hello. Done before the app runs: without the editor there is
/// nothing for the companion to do, and failing here exits before a window or
/// a GPU device is ever made.
pub(crate) fn connect(port: u16, token: &str) -> std::io::Result<TcpStream> {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)?;
    let hello = Message::Hello {
        token: token.to_string(),
        version: PROTOCOL_VERSION,
    };
    stream.write_all(protocol::encode(&hello).as_bytes())?;
    Ok(stream)
}

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

/// Take in the editor's messages, hand jobs to the queue, and pass its
/// reports back.
fn pump(
    mut link: ResMut<Link>,
    mut queue: ResMut<JobQueue>,
    mut questions: ResMut<analysis::Questions>,
    mut cache: ResMut<super::cache::ProjectCache>,
    mut editor: ResMut<EditorState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut received = Vec::new();
    let mut closed = false;
    if let Ok(rx) = link.incoming.lock() {
        while let Ok(incoming) = rx.try_recv() {
            match incoming {
                Incoming::Message(message) => received.push(message),
                Incoming::Closed => closed = true,
            }
        }
    }
    for message in received {
        match message {
            // Questions about the capture are answered from it directly; see
            // `analysis`.
            Message::Submit { id, job: Job::Run { kind, payload, .. } } if analysis::is_question(&kind) => {
                questions.0.push((id, kind, payload));
            }
            // The project listing, which is also how the companion learns
            // which project is open; see `cache`.
            Message::Submit { id, job: Job::Run { kind, payload, .. } } if super::cache::is_listing(&kind) => {
                if let Err(why) = cache.ask(id, &payload) {
                    queue.answer(id, Err(why));
                }
            }
            Message::Submit { id, job } => queue.submit(id, job),
            Message::Cancel { id } => queue.cancel(id),
            Message::EditorState { playing } => editor.playing = playing,
            Message::Show => {
                // Visible alone leaves it wherever it was, usually behind the
                // editor that was just clicked; asking for focus brings it up.
                for mut window in &mut windows {
                    window.visible = true;
                    window.focused = true;
                }
            }
            // Lines only the companion sends.
            Message::Hello { .. } | Message::Progress { .. } | Message::Done { .. } => {}
        }
    }

    for report in queue.take_reports() {
        // The companion's own jobs (thumbnails it renders ahead of time) are
        // nobody's in the editor.
        let id = match &report {
            Report::Progress { id, .. } | Report::Done { id, .. } => *id,
        };
        if id & super::cache::OWN_JOB != 0 {
            continue;
        }
        let message = match report {
            Report::Progress { id, fraction, note } => Message::Progress { id, fraction, note },
            Report::Done { id, result } => Message::Done { id, result },
        };
        if let Err(e) = link.writer.write_all(protocol::encode(&message).as_bytes()) {
            warn!("[companion] could not report to the editor: {e}");
        }
    }

    // The editor is gone, by quitting or crashing. On Windows the job object
    // has usually ended this process already; elsewhere this is what does.
    if closed {
        info!("[companion] the editor closed the link; exiting");
        exit.write(AppExit::Success);
    }
}

/// The window's close button hides it; the companion lives as long as the
/// editor does.
fn hide_on_close(mut requests: MessageReader<WindowCloseRequested>, mut windows: Query<&mut Window>) {
    for request in requests.read() {
        if let Ok(mut window) = windows.get_mut(request.window) {
            window.visible = false;
        }
    }
}
