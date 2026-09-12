//! The per-frame conversation with the backend.
//!
//! One system, running once a frame: hand over whatever [`renzora::net::fetch`]
//! queued since the last one, pass on any cancellations, then drain the events
//! the backend has ready and route each to the thread waiting on its tag.
//!
//! Nothing here blocks. The transfers happen on the backend's own threads; this
//! is only the hand-off.

use std::panic::AssertUnwindSafe;

use bevy::prelude::*;

use renzora::net::shared;
use renzora::net_backend::{Backend, BackendInfo, Caps, Event, EventKind, NetBackend};

/// The adopted backend, and what it told us about itself.
#[derive(Resource, Default)]
pub struct NetLink {
    backend: Option<Box<dyn Backend>>,
    info: Option<BackendInfo>,
    /// A backend that panicked. Not retried: it has already shown it will take
    /// the frame down, and calling it sixty times a second is how one bad
    /// request becomes an unusable editor.
    poisoned: bool,
    /// Whether a backend has ever been adopted in this session. Distinguishes
    /// "the backend has not registered yet" from "this build has none", which
    /// need opposite handling — see [`GRACE_FRAMES`].
    ever_had_backend: bool,
    /// Frames elapsed before the first backend appeared.
    startup_frames: u32,
}

/// How long the pump holds queued requests waiting for a first backend.
///
/// Registration happens during the first frames, so a request issued from a
/// startup thread can genuinely precede the client by a frame or two. Failing it
/// immediately made every such caller a startup race — the marketplace panel
/// showing no thumbnails, the splash showing no star count — for no reason other
/// than arriving early.
///
/// Bounded rather than indefinite, because a build that ships no client has to
/// say so instead of parking every request until its timeout. Roughly five
/// seconds at 60 Hz, after which requests fail as `Error::NoBackend` and keep
/// failing.
const GRACE_FRAMES: u32 = 300;

impl NetLink {
    /// The backend's name, for logs and the settings panel.
    pub fn name(&self) -> Option<&str> {
        self.backend.as_deref().map(Backend::name)
    }

    /// Whether a usable backend is loaded.
    pub fn is_active(&self) -> bool {
        self.backend.is_some() && !self.poisoned
    }

    /// Whether the backend claimed `caps`.
    ///
    /// Asked rather than assumed, because backends genuinely differ — a browser
    /// build going through `fetch` cannot set every header, and cannot stream.
    pub fn supports(&self, caps: Caps) -> bool {
        self.info.as_ref().is_some_and(|i| i.caps.contains(caps))
    }

    /// Call the backend, catching a panic rather than letting it reach the
    /// schedule.
    ///
    /// A malformed header should not take the editor down with it, and a backend
    /// that panicked once will do it again, so it is poisoned rather than
    /// retried. This is why the workspace keeps `panic = "unwind"`.
    fn guarded<T>(&mut self, what: &str, f: impl FnOnce(&mut dyn Backend) -> T) -> Option<T> {
        if self.poisoned {
            return None;
        }
        let backend = self.backend.as_deref_mut()?;
        match std::panic::catch_unwind(AssertUnwindSafe(|| f(backend))) {
            Ok(v) => Some(v),
            Err(_) => {
                let name = self.name().unwrap_or("?").to_string();
                error!("[net] backend `{name}` panicked in {what} and has been disabled");
                self.poisoned = true;
                // Failed HERE rather than on the next frame, because the
                // requests that matter are the ones already handed over: they
                // are no longer in the queue, so nothing downstream would find
                // them, and their threads would park until their own timeouts.
                shared().set_available(false);
                shared().fail_all("the network backend panicked and has been disabled");
                None
            }
        }
    }
}

/// Adopt a backend once one has registered.
///
/// Split from [`pump`] because it is the only part that touches
/// [`NetBackend`], and because the two failure modes are different: this one
/// runs `init` and can decide a backend is unusable before any request has been
/// made.
///
/// The backend is *moved* out of the resource into the link. Nothing takes it
/// back: a native client is part of the binary and cannot be unloaded, which is
/// the one thing the plugin version had to keep watching for.
pub(crate) fn adopt_backend(registered: Option<ResMut<NetBackend>>, mut link: ResMut<NetLink>) {
    if link.backend.is_some() {
        return;
    }
    let Some(mut registered) = registered else {
        return;
    };
    let Some(backend) = registered.0.take() else {
        return;
    };

    let name = backend.name().to_string();
    link.backend = Some(backend);
    link.info = None;
    link.poisoned = false;

    match link.guarded("init", |b| b.init()) {
        Some(Ok(info)) => {
            info!("[net] client `{}` ready ({})", name, info.agent);
            link.info = Some(info);
            link.ever_had_backend = true;
            shared().set_available(true);
        }
        Some(Err(e)) => {
            error!("[net] backend `{name}` could not start: {e}");
            link.backend = None;
        }
        // Poisoned during `init` — `guarded` has already reported it and failed
        // whatever was waiting.
        None => link.backend = None,
    }
}

/// Hand over queued requests, then drain whatever came back.
pub(crate) fn pump(mut link: ResMut<NetLink>) {
    // Ticked unconditionally, including when there is no backend: it is what
    // tells a parked thread that the frame loop is alive, and a thread waiting
    // on a request that will never be answered should learn that from
    // `fail_all` rather than from the watchdog.
    shared().tick();

    if !link.is_active() {
        // Still early enough that the client may simply not have registered.
        // Leave the queue untouched — these requests are not failed, they are
        // waiting.
        if !link.ever_had_backend && link.startup_frames < GRACE_FRAMES {
            link.startup_frames += 1;
            return;
        }
        // Nothing can be started, and nothing is coming. Fail what has piled up
        // rather than letting it grow.
        let orphaned = shared().take_queued();
        if !orphaned.is_empty() {
            shared().fail_all(renzora::net::NO_BACKEND);
        }
        let _ = shared().take_cancels();
        return;
    }

    for submission in shared().take_queued() {
        let tag = submission.request.tag;
        let started = link.guarded("start", |b| b.start(&submission.request, &submission.body));
        if let Some(Err(e)) = started {
            // The request never started, so nothing will ever report it. Deliver
            // the failure ourselves or the caller parks until its timeout.
            warn!("[net] request failed to start: {e}");
            shared().deliver(Event {
                tag,
                kind: EventKind::Error,
                status: 0,
                headers: Vec::new(),
                body: e.into_bytes(),
            });
        }
    }

    if link.supports(Caps::CANCEL) {
        for tag in shared().take_cancels() {
            link.guarded("cancel", |b| b.cancel(tag));
        }
    } else {
        // Nothing to send them to. Dropping them is correct — the waiter is
        // already gone, so the only cost is that the transfer runs to
        // completion and its events are discarded on arrival.
        let _ = shared().take_cancels();
    }

    if let Some(events) = link.guarded("poll", |b| b.poll()) {
        for event in events {
            shared().deliver(event);
        }
    }
}
