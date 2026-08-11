//! The per-frame conversation with the backend.
//!
//! One system, running once a frame: hand over whatever [`crate::api::fetch`]
//! queued since the last one, pass on any cancellations, then drain the events
//! the backend has ready and route each to the thread waiting on its tag.
//!
//! Nothing here blocks. The transfers happen inside the plugin, on its own
//! threads; this is only the hand-off.

use std::ffi::c_void;

use bevy::prelude::*;
use renzora_plugin::host::PluginNetBackend;
use renzora_plugin::net::{decode_events, BackendInfo, Caps, Event};
use renzora_plugin::sys::{self, NetOp, NetStatus};
use renzora_plugin::wire::{Reader, Writer};

use crate::api::shared;

/// The backend, and everything needed to call it.
#[derive(Resource, Default)]
pub struct NetLink {
    backend: Option<Loaded>,
    info: Option<BackendInfo>,
    /// A backend that panicked. Not retried: it has already shown it will take
    /// the frame down, and calling it sixty times a second is how one bad
    /// request becomes an unusable editor.
    poisoned: bool,
    /// Whether a backend has ever been adopted in this session. Distinguishes
    /// "the plugin has not loaded yet" from "this build has no plugin", which
    /// need opposite handling — see [`GRACE_FRAMES`].
    ever_had_backend: bool,
    /// Frames elapsed before the first backend appeared.
    startup_frames: u32,
}

/// How long the pump holds queued requests waiting for a first backend.
///
/// The plugin loader runs during the first frames, so a request issued from a
/// startup thread can genuinely precede the HTTP plugin by a frame or two.
/// Failing it immediately made every such caller a startup race — the marketplace
/// panel showing no thumbnails, the splash showing no star count — for no reason
/// other than arriving early.
///
/// Bounded rather than indefinite, because a build that ships no HTTP plugin has
/// to say so instead of parking every request until its timeout. Roughly five
/// seconds at 60 Hz, after which requests fail as `Error::NoBackend` and keep
/// failing.
const GRACE_FRAMES: u32 = 300;

struct Loaded {
    name: String,
    state: usize,
    entry: sys::NetEntry,
}

impl NetLink {
    /// The backend's name, for logs and the settings panel.
    pub fn name(&self) -> Option<&str> {
        self.backend.as_ref().map(|b| b.name.as_str())
    }

    /// Whether a usable backend is loaded.
    pub fn is_active(&self) -> bool {
        self.backend.is_some() && !self.poisoned
    }

    /// Whether the backend claimed `caps`.
    ///
    /// Asked rather than assumed, because backends genuinely differ — a browser
    /// build going through `fetch` cannot set every header, and cannot stream on
    /// older engines. A caller that skips this gets an editor that quietly does
    /// nothing rather than one that reports a missing feature.
    pub fn supports(&self, caps: Caps) -> bool {
        self.info.as_ref().is_some_and(|i| i.caps.contains(caps))
    }

    /// Make one call. `Ok(None)` means the backend does not implement this op.
    fn call(&mut self, op: NetOp, payload: &[u8], blob: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let Some(backend) = self.backend.as_ref() else {
            return Ok(None);
        };
        if self.poisoned {
            return Ok(None);
        }

        let mut out: Vec<u8> = Vec::new();
        // SAFETY: `ctx` is the address of `out`, which outlives this call, and
        // the backend only ever passes it back to this function.
        unsafe extern "C" fn collect(ctx: *mut c_void, bytes: *const u8, len: usize) {
            if ctx.is_null() || bytes.is_null() {
                return;
            }
            let v = &mut *(ctx as *mut Vec<u8>);
            v.extend_from_slice(std::slice::from_raw_parts(bytes, len));
        }
        let sink = sys::ByteSink {
            ctx: &mut out as *mut Vec<u8> as *mut c_void,
            write: collect,
        };
        let call = sys::NetCall {
            op,
            _pad: 0,
            state: backend.state as *mut c_void,
            payload: sys::BlobRef::new(payload),
            blob: sys::BlobRef::new(blob),
            out: &sink,
        };

        // SAFETY: every blob above outlives the call, and the sink writes into
        // `out`, which does too.
        let status = unsafe { (backend.entry)(&call) };

        if !status.is_known() {
            let name = backend.name.clone();
            self.poisoned = true;
            return Err(format!(
                "network backend `{name}` returned status {} — it was built against a newer engine",
                status.0
            ));
        }
        match status {
            NetStatus::Ok => Ok(Some(out)),
            // Not an error, and the host must not log it: it is how a backend
            // says "I was built before this op existed", which is ordinary.
            NetStatus::UnknownOp => Ok(None),
            NetStatus::Error => Err(decode_error(&out)),
            NetStatus::Panicked => {
                self.poisoned = true;
                Err(format!(
                    "network backend `{}` panicked and has been disabled: {}",
                    backend.name,
                    decode_error(&out)
                ))
            }
            _ => Ok(Some(out)),
        }
    }
}

/// Read the error string a backend writes alongside [`NetStatus::Error`].
fn decode_error(bytes: &[u8]) -> String {
    Reader::new(bytes)
        .string()
        .unwrap_or_else(|_| "backend reported an error it could not describe".to_string())
}

/// Adopt a backend the plugin host registered, or let go of one that vanished.
///
/// Split from [`pump`] because it is the only part that touches
/// [`PluginNetBackend`], and because the two failure modes are different: this
/// one runs `Init` and can decide a backend is unusable before any request has
/// been made.
pub(crate) fn adopt_backend(
    registered: Option<Res<PluginNetBackend>>,
    mut link: ResMut<NetLink>,
) {
    let registered = registered.and_then(|r| {
        r.0.as_ref()
            .map(|b| (b.name.clone(), b.state, b.entry))
    });

    match (registered, link.backend.is_some()) {
        // A backend appeared. Bring it up before anything queues a request.
        (Some((name, state, entry)), false) => {
            link.backend = Some(Loaded {
                name: name.clone(),
                state,
                entry,
            });
            link.info = None;
            link.poisoned = false;
            match link.call(NetOp::Init, &[], &[]) {
                Ok(Some(reply)) => match BackendInfo::decode(&mut Reader::new(&reply)) {
                    Ok(info) => {
                        info!("[net] client `{}` ready ({})", name, info.agent);
                        link.info = Some(info);
                        link.ever_had_backend = true;
                        shared().set_available(true);
                    }
                    Err(e) => {
                        error!("[net] backend `{name}` sent an info block that would not decode: {e}");
                        link.backend = None;
                    }
                },
                // A backend that does not implement `Init` is not a backend.
                Ok(None) => {
                    error!("[net] backend `{name}` does not implement Init");
                    link.backend = None;
                }
                Err(e) => {
                    error!("[net] backend `{name}` could not start: {e}");
                    link.backend = None;
                }
            }
        }
        // The plugin was unloaded or hot-reloaded. `entry` and `state` now point
        // into an unmapped image, so this must happen before anything calls
        // through them again — and every parked thread has to be told, or it
        // waits out its full timeout for an answer that can no longer come.
        (None, true) => {
            let name = link.name().unwrap_or("?").to_string();
            info!("[net] client `{name}` went away");
            link.backend = None;
            link.info = None;
            shared().set_available(false);
            shared().fail_all("the network backend was unloaded");
        }
        _ => {}
    }

    // A poisoned backend is still registered but no longer usable. Say so, so a
    // caller fails immediately rather than queueing into a void.
    if link.poisoned && crate::is_available() {
        shared().set_available(false);
        shared().fail_all("the network backend panicked and has been disabled");
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
        // Still early enough that the plugin may simply not have loaded. Leave
        // the queue untouched — these requests are not failed, they are waiting.
        if !link.ever_had_backend && link.startup_frames < GRACE_FRAMES {
            link.startup_frames += 1;
            return;
        }
        // Nothing can be started, and nothing is coming. Fail what has piled up
        // rather than letting it grow; a backend that went away already failed
        // its own in-flight requests in `adopt_backend`.
        let orphaned = shared().take_queued();
        if !orphaned.is_empty() {
            shared().fail_all(crate::api::NO_BACKEND);
        }
        let _ = shared().take_cancels();
        return;
    }

    let mut w = Writer::new();
    for submission in shared().take_queued() {
        w.clear();
        submission.request.encode(&mut w);
        let tag = submission.request.tag;
        if let Err(e) = link.call(NetOp::Start, w.bytes(), &submission.body) {
            // The request never started, so nothing will ever report it. Deliver
            // the failure ourselves or the caller parks until its timeout.
            warn!("[net] request failed to start: {e}");
            shared().deliver(Event {
                tag,
                kind: renzora_plugin::net::EventKind::Error,
                status: 0,
                headers: Vec::new(),
                body: e.into_bytes(),
            });
        }
    }

    if link.supports(Caps::CANCEL) {
        for tag in shared().take_cancels() {
            w.clear();
            w.u64(tag);
            if let Err(e) = link.call(NetOp::Cancel, w.bytes(), &[]) {
                warn!("[net] cancel failed: {e}");
            }
        }
    } else {
        // Nothing to send them to. Dropping them is correct — the waiter is
        // already gone, so the only cost is that the transfer runs to
        // completion and its events are discarded on arrival.
        let _ = shared().take_cancels();
    }

    match link.call(NetOp::Poll, &[], &[]) {
        Ok(Some(reply)) => match decode_events(&reply) {
            Ok(events) => {
                for event in events {
                    shared().deliver(event);
                }
            }
            Err(e) => error!("[net] backend sent events that would not decode: {e}"),
        },
        // A backend that does not implement `Poll` can never answer anything.
        Ok(None) => {}
        Err(e) => error!("[net] {e}"),
    }
}
