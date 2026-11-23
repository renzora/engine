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

/// The argument that starts this app as the standalone companion's render
/// worker rather than as a companion of its own.
pub(crate) const RENDER_WORKER_FLAG: &str = "--render-worker";

/// Whether this process is a render worker: started by the standalone
/// companion (`renzora-companion`), which has no renderer, to render the
/// thumbnails it is asked for, and stopped by it once there are none left.
pub(crate) fn is_render_worker() -> bool {
    std::env::args().any(|a| a == RENDER_WORKER_FLAG)
}

pub(crate) fn build(app: &mut App) {
    let worker = is_render_worker();
    if !link::build(app, worker) {
        return;
    }
    lower_priority();
    if worker {
        // Only what rendering needs: the link (to the standalone companion,
        // which plays the editor's part), the queue and the capture code. No
        // window to show, no profiler, no project watcher: the companion
        // does those itself. `link::pump` and `jobs::held` read these three,
        // so they exist, empty.
        app.init_resource::<games::Connection>()
            .init_resource::<analysis::Questions>()
            .init_resource::<cache::ProjectCache>();
        jobs::build(app);
        render::build(app);
        return;
    }
    let key = renzora_diagnostics_api::profiling::ProfilingAccess::from_args("")
        .map(|a| a.key)
        .unwrap_or_else(renzora_diagnostics_api::profiling::machine_key);
    app.insert_resource(PairingKey(key));
    app.add_plugins(renzora_ember::EmberPlugin);
    // Ember's cursor system reads the markup layer's drag state, which the
    // engine's markup plugin normally creates; the companion has no markup.
    app.init_resource::<renzora_ember::markup::dnd::DragState>();
    // The companion is booted from `add_companion_rendering`, not the engine's
    // plugin list, so nothing else puts these in. Without them every job that
    // fetches (marketplace images, installs, uploads) parked for thirty seconds
    // and failed with "the frame loop is not running".
    app.add_plugins((renzora_net::NetPlugin, renzora_http::RenzoraHttpPlugin));
    games::build(app);
    capture::build(app);
    jobs::build(app);
    render::build(app);
    cache::build(app);
    analysis::build(app);
    ui::build(app);
}
