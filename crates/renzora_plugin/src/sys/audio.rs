//! The audio boundary: the five types [`Interface`](super::Interface) names to
//! make the audio engine a plugin.
//!
//! Here rather than in the parent for navigability only — everything in this
//! file is part of the same frozen mechanism as the rest of `sys`, and the same
//! append-only rules apply to every one of these declarations. The vocabulary
//! that rides on top (requests, replies, their codec) is
//! [`crate::audio`](crate::audio), which can grow without any of this moving.

use core::ffi::c_void;

use super::{BlobRef, ByteSink, Str256};

// ── Audio ────────────────────────────────────────────────────────────────────
//
// The boundary half of `crate::audio`, here for the same reason the scripting
// half is: anything named in [`Interface`] is part of the frozen mechanism. The
// vocabulary that rides on top — the requests, the replies, their codecs — lives
// in `crate::audio` and can grow without any of this moving.
//
// Audio is shaped like scripting rather than like animation or physics: the host
// calls *into* the plugin. A command queue expresses "do this for me" perfectly
// and cannot express "hand me the next 512 frames of mixed audio", nor "what are
// the meters reading". So the plugin hands over an entry point, which means a
// table entry.

/// Which audio operation the host is invoking.
///
/// Newtype rather than an `enum` for the soundness reason every discriminant
/// here is one: the value crosses between binaries, and materialising an
/// out-of-range discriminant into a Rust enum is undefined behaviour. Unknown
/// values fall to the `_` arm and become [`AudioStatus::UnknownOp`], which is
/// what makes appending an op a non-breaking change — a backend built before an
/// op existed reports not knowing it, and the host treats that exactly as it
/// treats a capability the backend never had.
///
/// **Append only.** Renumbering repoints an already-built backend's `Play` at
/// somebody else's operation.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioOp(pub u32);

#[allow(non_upper_case_globals)]
impl AudioOp {
    /// Open the device and start mixing. Replies with an encoded `BackendInfo`
    /// — sample rate and what this backend can actually do.
    pub const Init: Self = Self(0);
    /// Stop everything and release the device.
    pub const Shutdown: Self = Self(1);
    /// The whole bus graph, in mixer order. Sent whenever it changes rather than
    /// diffed, because the board is a few dozen entries at most and a diff
    /// protocol is a second source of truth to get out of step.
    pub const SetBuses: Self = Self(2);
    /// Decode a clip. `payload` is the encoded request, `blob` the file bytes.
    /// Replies with the clip's duration.
    pub const LoadClip: Self = Self(3);
    /// Drop a decoded clip.
    pub const UnloadClip: Self = Self(4);
    /// Start a voice on a loaded clip.
    pub const Play: Self = Self(5);
    /// Stop a voice, a bus's voices, or everything.
    pub const Stop: Self = Self(6);
    /// Per-frame: the listener, moved emitters, retuned voices. Replies with the
    /// meters and which voices have finished.
    pub const Update: Self = Self(7);
    /// Open a capture device. Replies with its rate and name.
    pub const OpenCapture: Self = Self(8);
    /// Close a capture device.
    pub const CloseCapture: Self = Self(9);
    /// Take everything captured since the last call. Replies with the samples.
    pub const ReadCapture: Self = Self(10);
    /// Push interleaved stereo samples onto a bus — mic monitoring, a remote
    /// player's voice, a synth. See `crate::audio` for why this is one generic
    /// op rather than a family of microphone-shaped ones.
    pub const PushFrames: Self = Self(11);
    /// Enumerate devices. Replies with input and output name lists.
    pub const ListDevices: Self = Self(12);

    pub const fn is_known(self) -> bool {
        self.0 < 13
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Init",
            1 => "Shutdown",
            2 => "SetBuses",
            3 => "LoadClip",
            4 => "UnloadClip",
            5 => "Play",
            6 => "Stop",
            7 => "Update",
            8 => "OpenCapture",
            9 => "CloseCapture",
            10 => "ReadCapture",
            11 => "PushFrames",
            12 => "ListDevices",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for AudioOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "AudioOp({})", self.0)
        }
    }
}

/// How a call into an audio backend went.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AudioStatus(pub i32);

#[allow(non_upper_case_globals)]
impl AudioStatus {
    pub const Ok: Self = Self(0);
    /// This backend does not implement this op. Not an error: a WebAudio
    /// backend has no capture, and a host that asked is expected to carry on.
    /// The same mechanism that makes appending an op safe.
    pub const UnknownOp: Self = Self(1);
    /// The operation failed; the message is in the reply. A device that could
    /// not be opened, a clip that would not decode.
    pub const Error: Self = Self(2);
    /// The plugin panicked and its guard caught it. The host stops calling this
    /// backend rather than aborting a frame at a time.
    pub const Panicked: Self = Self(3);

    pub const fn is_known(self) -> bool {
        self.0 >= 0 && self.0 < 4
    }
}

/// One invocation of an audio backend.
///
/// Two payload slots rather than one, because the large case is genuinely
/// different in kind: `payload` is an encoded request of a few dozen bytes,
/// while `blob` is a whole encoded audio file — megabytes the host already has
/// mapped and must not copy into a codec buffer just to name them. Every op but
/// [`AudioOp::LoadClip`] leaves `blob` empty, which costs two words.
#[repr(C)]
pub struct AudioCall {
    pub op: AudioOp,
    /// Keeps the pointer fields below 8-byte aligned on every target. Explicit
    /// rather than left to the compiler for the same reason [`ScriptCall`] does
    /// it: implicit padding is padding the golden layout test cannot see, and
    /// the two sides compile this struct from separate source trees.
    pub _pad: u32,
    /// Backend state, handed back from [`AudioBackendDesc::state`]. Opaque to
    /// the host.
    pub state: *mut c_void,
    /// The encoded request for this op.
    pub payload: BlobRef,
    /// Bulk bytes — the encoded audio file for [`AudioOp::LoadClip`], the
    /// samples for [`AudioOp::PushFrames`]. Empty otherwise.
    pub blob: BlobRef,
    /// Where the backend writes its encoded reply.
    pub out: *const ByteSink,
}

/// The signature of an audio backend's entry point.
pub type AudioEntry = unsafe extern "C" fn(call: *const AudioCall) -> AudioStatus;

/// What a plugin registers to become the audio backend.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AudioBackendDesc {
    /// Human-readable, for logs and the editor's device panel.
    pub name: Str256,
    /// Opaque backend state, handed back on every call. The host never
    /// dereferences it.
    ///
    /// A pointer rather than a `static` inside the plugin because a backend
    /// that wants two instances — an editor preview mixer alongside the game's
    /// — should not be prevented from having them by the boundary.
    pub state: *mut c_void,
    pub entry: AudioEntry,
}

// SAFETY: as [`ScriptBackendDesc`] — plain data, an opaque pointer the host only
// passes back, and a function pointer. The host keeps the descriptor in a Bevy
// resource, which requires both.
unsafe impl Send for AudioBackendDesc {}
unsafe impl Sync for AudioBackendDesc {}