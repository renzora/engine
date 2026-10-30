//! The companion's own app: the link to the editor, the games it can profile,
//! the jobs it runs, and its window.
//!
//! - [`link`]: the editor connection (`protocol`), and nothing else.
//! - [`games`]: finding games on this machine and the network, and the
//!   connection to the one being profiled.
//! - [`capture`]: what that connection delivered, kept and analysed.
//! - [`jobs`]: the work queue the editor hands over.
//! - [`render`]: the jobs that render (model thumbnails), with the editor's
//!   own capture code.
//! - [`cache`]: the open project's files, its watcher, and the caches kept in
//!   step with them.
//! - [`analysis`]: questions about the capture, answered for the editor and
//!   its MCP tools.
//! - [`ui`]: the window, in ember.

use bevy::prelude::*;

mod analysis;
mod cache;
mod capture;
mod games;
mod jobs;
mod link;
mod render;
mod ui;
mod work;

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
            warn!("[companion] could not lower its priority");
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

/// This companion's pairing key, from `--profiling-key`. What it presents to
/// every game it profiles; the editor passes its machine key.
#[derive(Resource, Clone)]
pub(crate) struct PairingKey(pub String);

pub(crate) fn build(app: &mut App) {
    if !link::build(app) {
        return;
    }
    lower_priority();
    let key = renzora::profiling::ProfilingAccess::from_args("")
        .map(|a| a.key)
        .unwrap_or_else(renzora::profiling::machine_key);
    app.insert_resource(PairingKey(key));
    app.add_plugins(renzora_ember::EmberPlugin);
    // Ember's cursor system reads the markup layer's drag state, which the
    // engine's markup plugin normally creates; the companion has no markup.
    app.init_resource::<renzora_ember::markup::dnd::DragState>();
    games::build(app);
    capture::build(app);
    jobs::build(app);
    render::build(app);
    cache::build(app);
    analysis::build(app);
    ui::build(app);
}
