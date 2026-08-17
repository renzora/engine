//! The wire vocabulary, and the framing under it.
//!
//! Every message is length-prefixed bincode. TCP is a byte stream with no
//! record boundaries, so the 4-byte little-endian length is what turns it back
//! into messages; without it a reader has no way to know where one snapshot ends
//! and the next begins.
//!
//! ## Why one enum rather than per-feature channels
//!
//! A collaborative session carries traffic with genuinely different shapes —
//! 20 Hz presence pings, multi-megabyte file chunks, occasional lease requests.
//! The temptation is a socket (or a stream id) per kind. One enum on one ordered
//! stream is chosen instead because **ordering between kinds is load-bearing**:
//! a `SceneReset` must not overtake the `Welcome` that announced it, and an
//! `EntityDespawn` must not overtake the `EntityUpsert` that created the entity.
//! Separate channels would make every such pair a race to be re-serialized by
//! hand.
//!
//! The cost is head-of-line blocking: a large file chunk delays the presence
//! ping behind it. That is why [`MAX_CHUNK`] is small enough that a chunk never
//! occupies the link long enough to be felt — the file transfer is deliberately
//! many small messages rather than few large ones.

use serde::{Deserialize, Serialize};

/// Bumped on any change to [`CollabMsg`] that isn't a pure append at the end of
/// the enum — **and on any change to the framing below**, which is just as
/// incompatible and has no version of its own. Both sides compare in the
/// handshake and refuse a mismatch outright: a session that half-understands its
/// peer corrupts the project on disk, which is a far worse outcome than refusing
/// to connect.
///
/// - 1: initial
/// - 2: frames carry [`FRAME_MAGIC`]; `CollabMsg::Control` added
pub const PROTOCOL_VERSION: u32 = 2;

/// Leads every frame.
///
/// Strictly speaking redundant — a correct stream never loses alignment. It is
/// here because an *incorrect* one did. Accepted sockets inherit the listener's
/// non-blocking flag on Windows, so `read_exact` returned `WouldBlock` part-way
/// through a frame; the reader treated that as retryable and looped, but
/// `read_exact` does not report how much it consumed before failing, so the
/// retry resumed mid-payload and read data as a length prefix.
///
/// What that looked like from the outside was
/// `peer announced a 1560347651-byte frame` — a number with no relation to
/// anything, blamed on the peer, several steps removed from the cause. With a
/// magic word the same fault says "stream desynchronised" at the first bad byte
/// instead. Four bytes per frame is a cheap price for a failure that explains
/// itself.
pub const FRAME_MAGIC: [u8; 4] = *b"RZC1";

/// Hard ceiling on a single frame. A peer that announces a larger one is hung up
/// on rather than trusted — the length prefix arrives before the data, so
/// without this check a hostile (or simply desynced) sender could make us
/// allocate an arbitrary buffer on its say-so.
pub const MAX_FRAME: usize = 64 * 1024 * 1024;

/// Payload bytes per [`CollabMsg::FileChunk`]. Small on purpose — see the
/// head-of-line note in the module docs.
pub const MAX_CHUNK: usize = 256 * 1024;

/// A camera pose, flattened to plain arrays so the protocol owns no Bevy types.
/// Presence is the one message kind sent continuously, so it stays small.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct CamPose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    /// Vertical field of view in radians — a peer's frustum is drawn with it, so
    /// their marker matches what they can actually see.
    pub fov: f32,
}

/// One message on the link. Guest→host and host→guest share the enum; which
/// direction a variant is legal in is enforced by the handler, not the type.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CollabMsg {
    // ── Handshake ───────────────────────────────────────────────────────────
    /// Guest→host, first message. `project` is the folder name, compared only to
    /// warn about an obvious mismatch — the file sync is what actually
    /// reconciles the two trees.
    Hello {
        protocol: u32,
        display_name: String,
        project: String,
    },
    /// Host→guest, accepting. Carries the full scene so the guest can render the
    /// session immediately, before any file sync has finished.
    Welcome {
        protocol: u32,
        peer_id: u64,
        host_name: String,
        project: String,
    },
    /// Host→guest, refusing. Sent before the socket closes so the guest can say
    /// *why* rather than reporting a bare disconnect.
    Rejected { reason: String },

    // ── Presence ────────────────────────────────────────────────────────────
    /// Where a peer is looking and what they have selected. Sent continuously at
    /// a low rate; dropped freely — it is state, not an event, so a missed one
    /// is corrected by the next.
    Presence {
        peer: u64,
        camera: Option<CamPose>,
        selection: Vec<u64>,
    },
    PeerJoined {
        peer: u64,
        name: String,
        color: [u8; 3],
    },
    PeerLeft {
        peer: u64,
    },
    /// Host→guest: whether guests may currently change the document.
    ///
    /// Sent on admission and on every flip of the host's switch, rather than
    /// left implicit. A guest that does not know it is read-only would keep
    /// making edits that the host silently discards, and the only symptom would
    /// be their work quietly reverting a fraction of a second later.
    Control {
        allowed: bool,
    },

    // ── Scene ───────────────────────────────────────────────────────────────
    /// The whole document. Sent on join, and as the recovery path whenever the
    /// two sides can no longer be reconciled incrementally.
    SceneReset {
        bsn: String,
        ids: Vec<(u64, u64)>,
    },
    /// Entities that changed. `bsn` is a snapshot of exactly those entities;
    /// `ids` maps each entity id *inside that snapshot* to its session-wide
    /// [`crate::identity::CollabId`], which is what lets the receiver patch its
    /// own copy instead of spawning a duplicate.
    ///
    /// `removed` carries what the snapshot cannot: applying a snapshot adds and
    /// overwrites components but never removes them, so a component deleted on
    /// the sender would otherwise live forever on the receiver.
    EntityUpsert {
        bsn: String,
        ids: Vec<(u64, u64)>,
        removed: Vec<(u64, Vec<String>)>,
    },
    EntityDespawn {
        ids: Vec<u64>,
    },

    // ── Leases ──────────────────────────────────────────────────────────────
    /// Guest→host: "I intend to edit these". Granted or not, the guest edits
    /// locally either way — the lease decides whose version *survives*, and
    /// gives the other side something to grey out in the hierarchy.
    LeaseRequest { ids: Vec<u64> },
    /// Host→everyone: authoritative ownership. An id absent from every grant is
    /// unowned and free.
    LeaseGrant { peer: u64, ids: Vec<u64> },
    LeaseRelease { ids: Vec<u64> },

    // ── Files ───────────────────────────────────────────────────────────────
    /// Host→guest on join: every project file with its size and content hash, so
    /// the guest can ask for only what it is missing.
    FileManifest { files: Vec<FileEntry> },
    /// Guest→host: send me these (project-relative paths).
    FileRequest { paths: Vec<String> },
    /// One slice of one file. `offset` is a byte offset; the final chunk of a
    /// file carries `last: true` so the receiver can close and hash it without
    /// having to know the length in advance.
    FileChunk {
        path: String,
        offset: u64,
        bytes: Vec<u8>,
        last: bool,
    },
    /// Host→guest: this file changed on disk (a save, an import). The guest
    /// requests it if it cares.
    FileTouched { entry: FileEntry },

    // ── Liveness ────────────────────────────────────────────────────────────
    Ping,
    Pong,
}

/// One file in the project tree, as the manifest describes it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    /// Project-relative, forward-slashed. Never absolute and never containing
    /// `..` — the receiver writes this path inside its own project and a
    /// traversal here would let a host write anywhere on a guest's disk.
    pub path: String,
    pub size: u64,
    /// Content hash. Cheap and non-cryptographic: it decides whether to transfer
    /// a file, not whether to trust one.
    pub hash: u64,
}

// ── Framing ─────────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod framing {
    use super::{CollabMsg, FRAME_MAGIC, MAX_FRAME};
    use std::io::{Read, Write};

    /// Encode one message as `[magic: 4][len: u32 LE][bincode payload]`.
    ///
    /// Assembled into one buffer and written once, rather than a write per part.
    /// With Nagle disabled a multi-part write is several segments on the wire for
    /// no benefit, and — the reason that matters — a failure between two of them
    /// would leave a header on the stream with no payload behind it,
    /// desynchronising the receiver permanently. One `write_all` either places a
    /// whole frame or fails with the connection already broken.
    pub fn write_frame<W: Write>(w: &mut W, msg: &CollabMsg) -> std::io::Result<()> {
        let payload = bincode::serde::encode_to_vec(msg, bincode::config::standard())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        if payload.len() > MAX_FRAME {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame of {} bytes exceeds MAX_FRAME", payload.len()),
            ));
        }
        let mut frame = Vec::with_capacity(payload.len() + 8);
        frame.extend_from_slice(&FRAME_MAGIC);
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&payload);
        w.write_all(&frame)?;
        w.flush()
    }

    /// Read exactly one message, blocking until it is complete.
    ///
    /// **Never call this on a stream with a read timeout.** `read_exact` does not
    /// report how many bytes it consumed before failing, so a timeout landing
    /// mid-frame silently eats part of the stream and every frame after it is
    /// misparsed. The reader is meant to block indefinitely and be interrupted by
    /// closing the transport instead — see `Link`'s `Drop`.
    ///
    /// The length is validated *before* the buffer is allocated. A frame header
    /// is the one thing a peer can make us act on before it has proved anything,
    /// so an absurd length is a disconnect rather than a 4 GB allocation.
    pub fn read_frame<R: Read>(r: &mut R) -> std::io::Result<CollabMsg> {
        let mut header = [0u8; 8];
        r.read_exact(&mut header)?;
        if header[..4] != FRAME_MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "stream desynchronised (frame magic missing) — a fault on this side                  of the link, not a bad peer",
            ));
        }
        let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if len > MAX_FRAME {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("peer announced a {len}-byte frame"),
            ));
        }
        let mut payload = vec![0u8; len];
        r.read_exact(&mut payload)?;
        bincode::serde::decode_from_slice(&payload, bincode::config::standard())
            .map(|(m, _)| m)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use framing::{read_frame, write_frame};

impl CollabMsg {
    /// A short label for logs and the session panel's activity line. Deliberately
    /// not `Debug` — a `SceneReset`'s payload is the entire scene, and printing
    /// one into a log has already been a way to freeze the editor.
    pub fn label(&self) -> &'static str {
        match self {
            CollabMsg::Hello { .. } => "hello",
            CollabMsg::Welcome { .. } => "welcome",
            CollabMsg::Rejected { .. } => "rejected",
            CollabMsg::Presence { .. } => "presence",
            CollabMsg::PeerJoined { .. } => "peer-joined",
            CollabMsg::PeerLeft { .. } => "peer-left",
            CollabMsg::Control { .. } => "control",
            CollabMsg::SceneReset { .. } => "scene-reset",
            CollabMsg::EntityUpsert { .. } => "upsert",
            CollabMsg::EntityDespawn { .. } => "despawn",
            CollabMsg::LeaseRequest { .. } => "lease-request",
            CollabMsg::LeaseGrant { .. } => "lease-grant",
            CollabMsg::LeaseRelease { .. } => "lease-release",
            CollabMsg::FileManifest { .. } => "file-manifest",
            CollabMsg::FileRequest { .. } => "file-request",
            CollabMsg::FileChunk { .. } => "file-chunk",
            CollabMsg::FileTouched { .. } => "file-touched",
            CollabMsg::Ping => "ping",
            CollabMsg::Pong => "pong",
        }
    }
}
