//! The network boundary: the five types [`Interface`](super::Interface) names to
//! make the HTTP client a plugin.
//!
//! Here rather than in the parent for navigability only — everything in this
//! file is part of the same frozen mechanism as the rest of `sys`, and the same
//! append-only rules apply to every one of these declarations. The vocabulary
//! that rides on top (requests, events, their codec) is [`crate::net`], which
//! can grow without any of this moving.
//!
//! ## Why this is boundary surface and [`crate::http`] is not
//!
//! The two are the same protocol pointed in opposite directions, and it is worth
//! being clear about which is which before reading either.
//!
//! [`crate::http`] is a plugin asking the *engine* to fetch something. That is a
//! command — "do this for me, I will collect the answer later" — so it rides
//! [`CommandKind::Service`](super::CommandKind) like animation and physics, and
//! costs no table entry.
//!
//! This module is the reverse: the **engine** asking a plugin to fetch
//! something, because the engine no longer contains an HTTP client. The editor's
//! marketplace browser, the asset thumbnails, the login flow and the update
//! check all need an answer back, and a command queue cannot say "call me and
//! tell me what you got". So the plugin hands over an entry point, which means a
//! table entry — the same reasoning that put `add_script_backend` and
//! `add_audio_backend` here.
//!
//! Both directions coexist happily: a plugin's [`crate::http`] request reaches
//! the host, which hands it straight back out to whichever plugin registered
//! itself here.

use core::ffi::c_void;

use super::{BlobRef, ByteSink, Str256};

/// Which network operation the host is invoking.
///
/// Newtype rather than an `enum` for the soundness reason every discriminant
/// here is one: the value crosses between binaries, and materialising an
/// out-of-range discriminant into a Rust enum is undefined behaviour. Unknown
/// values fall to the `_` arm and become [`NetStatus::UnknownOp`], which is what
/// makes appending an op a non-breaking change.
///
/// **Append only.** Renumbering repoints an already-built backend's `Start` at
/// somebody else's operation.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetOp(pub u32);

#[allow(non_upper_case_globals)]
impl NetOp {
    /// Bring the client up. Replies with an encoded `BackendInfo` — what this
    /// backend is and what it can do.
    pub const Init: Self = Self(0);
    /// Cancel everything in flight and release the client.
    pub const Shutdown: Self = Self(1);
    /// Begin a request. `payload` is the encoded request, `blob` the body bytes.
    /// Replies with nothing: the answer arrives through [`Poll`](Self::Poll).
    pub const Start: Self = Self(2);
    /// Take everything that has arrived since the last call. Replies with a list
    /// of encoded events.
    pub const Poll: Self = Self(3);
    /// Abandon a request. The host has stopped caring about the answer — a
    /// cancelled download, a panel closed mid-fetch.
    pub const Cancel: Self = Self(4);

    pub const fn is_known(self) -> bool {
        self.0 < 5
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Init",
            1 => "Shutdown",
            2 => "Start",
            3 => "Poll",
            4 => "Cancel",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for NetOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "NetOp({})", self.0)
        }
    }
}

/// How a call into a network backend went.
///
/// Note what is **not** here: an HTTP status. A 404 is a perfectly successful
/// call — the backend was asked to fetch a URL and it did, and the server said
/// no. That belongs on the event, not on the call, and conflating the two is how
/// a client ends up unable to read the error body a server sent with its 400.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NetStatus(pub i32);

#[allow(non_upper_case_globals)]
impl NetStatus {
    pub const Ok: Self = Self(0);
    /// This backend does not implement this op. Not an error: a backend with no
    /// cancellation support is still a usable backend.
    pub const UnknownOp: Self = Self(1);
    /// The call itself failed — the client could not be brought up, a request
    /// could not be queued. The message is in the reply.
    pub const Error: Self = Self(2);
    /// The plugin panicked and its guard caught it. The host stops calling this
    /// backend rather than aborting a frame at a time.
    pub const Panicked: Self = Self(3);

    pub const fn is_known(self) -> bool {
        self.0 >= 0 && self.0 < 4
    }
}

/// One invocation of a network backend.
///
/// Two payload slots rather than one, for the same reason [`AudioCall`](super::AudioCall)
/// has two: `payload` is an encoded request of a few hundred bytes, while `blob`
/// is a whole request body — a multipart upload carrying a `.glb` is megabytes
/// the host already has contiguous and must not copy through a codec buffer just
/// to name a URL alongside. Every op but [`NetOp::Start`] leaves `blob` empty.
#[repr(C)]
pub struct NetCall {
    pub op: NetOp,
    /// Keeps the pointer fields below 8-byte aligned on every target. Explicit
    /// rather than left to the compiler for the same reason [`AudioCall`](super::AudioCall)
    /// does it: implicit padding is padding the golden layout test cannot see,
    /// and the two sides compile this struct from separate source trees.
    pub _pad: u32,
    /// Backend state, handed back from [`NetBackendDesc::state`]. Opaque to the
    /// host.
    pub state: *mut c_void,
    /// The encoded request for this op.
    pub payload: BlobRef,
    /// Bulk bytes — the request body for [`NetOp::Start`]. Empty otherwise.
    pub blob: BlobRef,
    /// Where the backend writes its encoded reply.
    pub out: *const ByteSink,
}

/// The signature of a network backend's entry point.
pub type NetEntry = unsafe extern "C" fn(call: *const NetCall) -> NetStatus;

/// What a plugin registers to become the network backend.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetBackendDesc {
    /// Human-readable, for logs and the editor's network settings.
    pub name: Str256,
    /// Opaque backend state, handed back on every call. The host never
    /// dereferences it.
    pub state: *mut c_void,
    pub entry: NetEntry,
}

// SAFETY: as [`AudioBackendDesc`](super::AudioBackendDesc) — plain data, an
// opaque pointer the host only passes back, and a function pointer. The host
// keeps the descriptor in a Bevy resource, which requires both.
unsafe impl Send for NetBackendDesc {}
unsafe impl Sync for NetBackendDesc {}
