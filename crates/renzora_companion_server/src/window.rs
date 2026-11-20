//! The window, from the server's side: a child process that draws it.
//!
//! The window runs as `renzora-companion --window`, started when the editor's
//! spanner asks for it and ended when it is closed. It is a process rather
//! than a thread because its GL driver cannot be unloaded: drawn in the
//! companion itself, the driver kept about 90 MB and a handful of threads in
//! the companion for the rest of the session after the window first opened.
//! A process takes all of that with it when it exits, so a companion nobody
//! is looking at is back to a few megabytes.
//!
//! The two talk over the child's standard streams (see [`crate::view`] for
//! what goes over them): messages in on its stdin, [`Command`] lines out on
//! its stdout. Its stderr is the companion's own, which the editor already
//! shows in its Console. Both ends notice the other going by the pipe
//! closing, and the editor's job object ends both with the editor.
//!
//! # Never waiting on the window
//!
//! The server thread does not write to the pipe itself: a window that stops
//! reading (dragged, minimised on some systems, or simply slower than a game
//! sending 60 large frames a second) would fill the pipe and stall the server,
//! and with it the editor's jobs. A writer thread takes everything from a
//! queue instead, and frames are dropped, not queued, once
//! [`FRAMES_QUEUED`] are waiting: the window loses some frames of a busy game
//! and nothing else does.

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use renzora_companion_proto::wire::GamePacket;

use crate::capture::Capture;
use crate::log;
use crate::server::Event;
use crate::view::{self, Command, ToWindow, View};

/// The argument that makes this executable the window.
pub(crate) const WINDOW_FLAG: &str = "--window";

/// Frames the window keeps to draw from: six seconds at 60 fps. The companion
/// keeps the whole recording; the window shows the newest couple of hundred
/// bars and the timeline around them. Profiling the editor (about 9,000
/// spans a frame) ten seconds of frames cost the window 200 MB, and holding
/// the whole recording twice would double what profiling costs.
pub(crate) const WINDOW_FRAMES: usize = 360;

/// Frames waiting for the writer before new ones are dropped.
const FRAMES_QUEUED: usize = 30;

/// What the writer thread is handed.
enum Out {
    Line(ToWindow),
    /// An encoded packet, whether it is a frame recorded while a job ran,
    /// and whether it is a frame (counted in `frames_queued`, and uncounted
    /// once written). Only frames are counted: uncounting every packet took
    /// the count below zero, where it wrapped to the largest number there is
    /// and every frame after was dropped as if the window were behind.
    Packet { bytes: Vec<u8>, overlapped: bool, frame: bool },
}

pub(crate) struct Window {
    child: Child,
    out: Sender<Out>,
    /// Frames handed to the writer and not yet written.
    frames_queued: Arc<AtomicUsize>,
    /// The last view sent, so an unchanged one is not sent again.
    last: Option<View>,
    /// Whether the window holds the current capture. False when it has just
    /// opened and whenever the server replaces the capture (a new game, an
    /// opened file), until [`Self::sync`] catches it up.
    synced: bool,
}

impl Window {
    /// Start the window process. Its commands arrive on `events` as
    /// [`Event::Ui`], then [`Event::WindowClosed`] once it has gone.
    pub(crate) fn open(events: &Sender<Event>) -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| format!("could not find this executable: {e}"))?;
        let mut child = std::process::Command::new(exe)
            .arg(WINDOW_FLAG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("could not start the window: {e}"))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err("the window's pipes were not made".into());
        };
        let pid = child.id();

        let tx = events.clone();
        let reader = std::thread::Builder::new().name("companion-window-in".into()).spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<Command>(&line) {
                    Ok(command) => {
                        if tx.send(Event::Ui(command)).is_err() {
                            return;
                        }
                    }
                    Err(e) => log::warn(format!("the window sent a line that is not a command ({e}): {line}")),
                }
            }
            let _ = tx.send(Event::WindowClosed(pid));
        });

        let (out, queue) = channel::<Out>();
        let frames_queued = Arc::new(AtomicUsize::new(0));
        let counted = frames_queued.clone();
        let writer = std::thread::Builder::new().name("companion-window-out".into()).spawn(move || {
            let mut pipe = BufWriter::new(stdin);
            while let Ok(first) = queue.recv() {
                // Everything already waiting goes in one flush.
                for message in std::iter::once(first).chain(queue.try_iter()) {
                    let written = match message {
                        Out::Line(line) => view::write_line(&mut pipe, &line),
                        Out::Packet { bytes, overlapped, frame } => {
                            if frame {
                                counted.fetch_sub(1, Ordering::Relaxed);
                            }
                            view::write_packet(&mut pipe, &bytes, overlapped)
                        }
                    };
                    if written.is_err() {
                        return;
                    }
                }
                if pipe.flush().is_err() {
                    return;
                }
            }
        });
        if let Err(e) = reader.as_ref().map(|_| ()).and(writer.as_ref().map(|_| ())) {
            let _ = child.kill();
            return Err(format!("could not talk to the window: {e}"));
        }
        Ok(Self { child, out, frames_queued, last: None, synced: false })
    }

    /// The process id, to tell this window's [`Event::WindowClosed`] from an
    /// earlier one's.
    pub(crate) fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Bring the window to the front. False when it is already gone.
    pub(crate) fn focus(&mut self) -> bool {
        self.out.send(Out::Line(ToWindow::Focus)).is_ok()
    }

    /// Send `view` if it differs from the last one sent. False when the
    /// window is gone.
    pub(crate) fn show(&mut self, view: View) -> bool {
        if self.last.as_ref() == Some(&view) {
            return true;
        }
        self.last = Some(view.clone());
        self.out.send(Out::Line(ToWindow::View(Box::new(view)))).is_ok()
    }

    /// Pass on a packet that just arrived from the game, `encoded` as it goes
    /// over the wire. A frame is dropped while the window is behind.
    pub(crate) fn forward(&mut self, packet: &GamePacket, overlapped: bool) -> bool {
        let frame = matches!(packet, GamePacket::Frame(_));
        if frame && self.frames_queued.load(Ordering::Relaxed) >= FRAMES_QUEUED {
            return true;
        }
        self.queue(packet, overlapped && frame)
    }

    /// Hand a packet to the writer, counting it first if it is a frame (the
    /// writer may write it before this returns).
    fn queue(&mut self, packet: &GamePacket, overlapped: bool) -> bool {
        let frame = matches!(packet, GamePacket::Frame(_));
        if frame {
            self.frames_queued.fetch_add(1, Ordering::Relaxed);
        }
        self.out.send(Out::Packet { bytes: packet.encode(), overlapped, frame }).is_ok()
    }

    /// Whether the window must be brought up to date with [`Self::sync`].
    pub(crate) fn behind(&self) -> bool {
        !self.synced
    }

    /// Start the window's capture over and send it the newest frames.
    pub(crate) fn sync(&mut self, capture: &Capture) -> bool {
        let reset = ToWindow::CaptureReset {
            source: capture.source.clone(),
            from_file: capture.from_file.as_ref().map(|p| p.display().to_string()),
        };
        if self.out.send(Out::Line(reset)).is_err() {
            return false;
        }
        for (packet, overlapped) in capture.packets(WINDOW_FRAMES) {
            if !self.queue(&packet, overlapped) {
                return false;
            }
        }
        self.synced = true;
        true
    }

    /// Mark the window as needing a full [`Self::sync`]: the capture it holds
    /// was replaced.
    pub(crate) fn invalidate(&mut self) {
        self.synced = false;
    }
}

impl Drop for Window {
    /// Reap the process. By the time a window is dropped it has usually
    /// exited (it was closed); if not, the server is replacing one that
    /// stopped listening, and it is ended rather than left behind.
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
