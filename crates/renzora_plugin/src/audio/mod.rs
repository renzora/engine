//! Implement the audio engine as a plugin.
//!
//! The engine ships an audio *API* — the bus graph, the components scenes carry,
//! the command queue, the timeline — and no audio. This module is the contract
//! between that and whatever actually makes sound, so a mixer is an ordinary
//! `plugins/<name>/` cdylib built with no engine checkout.
//!
//! That is the whole point of the split, and it generalises the same way
//! scripting's does. A native backend is cpal plus a mixer; a browser backend is
//! WebAudio, where the graph and the decoders come free and cpal cannot capture
//! at all. Both implement [`Backend`], and neither the engine nor the other one
//! needs to know the other exists.
//!
//! ## The shape of a call
//!
//! The host owns everything with a Bevy type in it. It walks the emitters,
//! resolves and reads the audio files, tracks which entity owns which voice, and
//! applies what comes back. The plugin owns exactly one thing: turning that into
//! samples.
//!
//! ```text
//!   host                                   plugin
//!   ────                                   ──────
//!   Init                     ────────────▶  open a device
//!                            ◀──────────── BackendInfo (rate + capabilities)
//!   SetBuses (whole board)   ────────────▶
//!   read file, hand over bytes ──────────▶  decode, cache by clip id
//!   Play                     ────────────▶  start a voice
//!   Update (listener, moves) ─── /frame ──▶
//!                            ◀──────────── peaks + finished voices
//! ```
//!
//! ### The host keeps file I/O, deliberately
//!
//! A backend never opens a file. It is handed the bytes and an extension hint.
//! Exported and Android builds read assets out of an rpak archive through a
//! closure the engine owns, so a backend doing its own `std::fs` would work in
//! the editor and fail in every shipped game — the identical trap
//! [`crate::script`] avoids for identical reasons.
//!
//! ### Capabilities, not assumptions
//!
//! [`Caps`] is answered at [`Init`](crate::sys::AudioOp::Init) rather than
//! assumed, because the two backends that matter genuinely differ — a WebAudio
//! build cannot capture through cpal, and a host that assumed otherwise would
//! leave the same game code silently doing nothing on the web.
//!
//! ## Ops, not one function pointer per operation
//!
//! A backend registers a single `extern "C"` entry point and the operation is
//! selected by [`AudioOp`](crate::sys::AudioOp). A struct of thirteen named
//! function pointers would make adding a fourteenth an ABI change — every
//! prebuilt backend would need rebuilding to add, say, a reverb send. With an op
//! code, a backend that does not know an op returns
//! [`AudioStatus::UnknownOp`](crate::sys::AudioStatus::UnknownOp) and the host
//! treats it exactly as it treats a capability that backend never had.

mod backend;
pub mod protocol;

pub use backend::{desc_for, dispatch, Backend};

pub use protocol::{
    read_buses, read_samples, write_buses, write_samples, BackendInfo, BusState, Caps, CaptureInfo,
    ClipInfo, DeviceList, EmitterState, ListenerState, LoadClip, OpenCapture, PlayRequest,
    StopRequest, StopTarget, UpdateReply, UpdateRequest,
};

/// The boundary half, re-exported so a plugin author names one module.
///
/// These live in [`sys`](crate::sys) because they are named by
/// [`Interface`](crate::sys::Interface), and anything in that table is part of
/// the frozen mechanism. Everything else here — the requests, the replies, the
/// codec — is vocabulary layered on top and can grow without moving the ABI.
pub use crate::sys::{AudioBackendDesc, AudioCall, AudioEntry, AudioOp, AudioStatus};
