//! `renzora-companion`: the editor's companion, with no engine in it.
//!
//! The editor hands slow work to its companion so its own frames stay smooth.
//! The first companion was a second copy of the engine (`renzora --companion`)
//! and held about a gigabyte of memory and a GPU device all the time, for work
//! that is almost all file, CPU and network. This one is a plain program: the
//! job queue, the project cache, the profiler and a small window, and nothing
//! else. See `docs/r1-alpha8/editor-dev/standalone-companion.md` for the plan
//! it is part of.
//!
//! # Shape
//!
//! - **The link** ([`link`]) is the same protocol the engine's companion
//!   speaks (`renzora_companion_proto::protocol`), so the editor's side does
//!   not care which one it started. The hello lists the job kinds this one
//!   runs; the editor does every other kind itself.
//! - **One thread owns the state** ([`server`]): the editor's messages, the
//!   workers' reports, the watcher's changes, the games' beacons and streams,
//!   and the window's buttons all arrive on one channel and are handled in
//!   order, so nothing else ever writes to it.
//! - **Profiling** ([`games`], [`capture`], [`analysis`]): games announce
//!   themselves on the network; the one connected streams every frame's
//!   spans here, where they are kept, saved, and asked about by the editor's
//!   MCP tools.
//! - **Thumbnails that render** ([`render`]) go to a render worker: the
//!   engine, started headless when there is one to render and stopped when
//!   there are none left, so the engine's memory is held only while it works.
//! - **The window is a process of its own** ([`window`], [`ui`]): this same
//!   executable run with `--window`, started when the editor's spanner asks
//!   and gone when it is closed. A GL driver stays loaded until its process
//!   exits, so a window drawn in here left about 90 MB behind; drawn in a
//!   child, closing it gives all of that back.
//! - **Exits with the editor.** On Windows the editor's job object usually
//!   ends it first; everywhere, the link closing ends it, and the window
//!   follows when its pipe closes.

// No console window: the editor reads this process's output through pipes and
// shows it in its Console. The engine's own executable is built the same way.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::sync::mpsc::channel;

mod analysis;
mod cache;
mod capture;
mod games;
mod jobs;
mod link;
mod log;
mod render;
mod server;
mod ui;
mod view;
mod window;
mod work;

fn main() {
    if std::env::args().any(|a| a == window::WINDOW_FLAG) {
        ui::main();
    }
    let Some((port, token)) = link::args() else {
        log::error("started without --companion-port and --companion-token; exiting");
        std::process::exit(2);
    };
    // Thumbnails that render only when the render worker can be started:
    // otherwise the editor keeps rendering them itself.
    let renders: &[&str] = if render::RenderWorker::beside_this_executable().available() { render::KINDS } else { &[] };
    let kinds: Vec<String> =
        [work::KINDS, cache::KINDS, analysis::KINDS, renders].concat().into_iter().map(str::to_string).collect();
    let writer = match link::connect(port, &token, kinds) {
        Ok(stream) => stream,
        Err(e) => {
            log::error(format!("could not reach the editor on port {port}: {e}; exiting"));
            std::process::exit(3);
        }
    };
    let Ok(reader) = writer.try_clone() else {
        log::error("could not read from the editor; exiting");
        std::process::exit(4);
    };
    lower_priority();
    log::info(format!("connected to the editor on port {port}"));

    let (events, incoming) = channel();
    link::read_lines(reader, events.clone());
    games::listen(events.clone());
    server::run(incoming, events, writer, games::pairing_key());
}

/// Ask the OS to schedule the companion below the editor.
///
/// Moving work into another process does not give the machine more cores:
/// both processes share them. What it does buy is that the OS can be told
/// which one matters, so a heavy job here waits for the editor's frame rather
/// than competing with it. Best effort; failing leaves normal priority.
fn lower_priority() {
    #[cfg(target_os = "windows")]
    {
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> isize;
            fn SetPriorityClass(process: isize, class: u32) -> i32;
        }
        // SAFETY: the pseudo-handle for this process, and a documented class.
        let ok = unsafe { SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) };
        if ok == 0 {
            log::warn("could not lower its priority");
        }
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn setpriority(which: i32, who: u32, prio: i32) -> i32;
        }
        const PRIO_PROCESS: i32 = 0;
        // SAFETY: `who = 0` is this process; 10 is a plain "nice" step.
        unsafe { setpriority(PRIO_PROCESS, 0, 10) };
    }
}
