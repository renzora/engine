//! The request and event vocabulary, and its codec.
//!
//! Everything here is layered on [`crate::wire`] and can grow without moving
//! [`sys::VERSION_MINOR`](crate::sys::VERSION_MINOR) — appending a field to a
//! struct in this file is a change to `renzora_plugin`'s own semver, and both
//! sides recompile from the same source. See [`crate::sys::net`] for the part
//! that is frozen.

use alloc::string::String;
use alloc::vec::Vec;

use crate::wire::{Reader, WireError, Writer};

/// What a backend can do beyond the basics, answered at
/// [`Init`](crate::sys::NetOp::Init).
///
/// Claimed rather than assumed, for the reason [`Caps`](crate::audio::Caps) is:
/// the backends that matter genuinely differ. A native client streams and
/// uploads; a browser build going through `fetch` cannot set every header and
/// has no control over redirects. A host that assumed otherwise would leave the
/// same editor silently doing nothing on one of them.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Caps(pub u32);

impl Caps {
    /// Bodies can be delivered in pieces — [`Request::stream`] is honoured
    /// rather than ignored. Without it a streaming request still works, it just
    /// arrives as one [`EventKind::Response`] at the end.
    pub const STREAM: Self = Self(1 << 0);
    /// [`Cancel`](crate::sys::NetOp::Cancel) actually abandons the transfer. A
    /// backend without it may still accept the op and simply drop the answer.
    pub const CANCEL: Self = Self(1 << 1);
    /// Requests may carry arbitrary headers. A `fetch`-based backend cannot
    /// promise this — the browser forbids setting several of them.
    pub const HEADERS: Self = Self(1 << 2);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for Caps {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// What a backend answers [`Init`](crate::sys::NetOp::Init) with.
#[derive(Clone, Debug)]
pub struct BackendInfo {
    /// What this client calls itself — the string that goes in logs, and the
    /// basis of the default `User-Agent`.
    pub agent: String,
    pub caps: Caps,
}

impl BackendInfo {
    pub fn encode(&self, w: &mut Writer) {
        w.str(&self.agent);
        w.u32(self.caps.0);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            agent: r.string()?,
            caps: Caps(r.u32()?),
        })
    }
}

/// One HTTP request.
///
/// The body is **not** here: it rides in [`NetCall::blob`](crate::sys::NetCall)
/// so a multipart upload does not get copied through the codec. See that field.
#[derive(Clone, Debug)]
pub struct Request {
    /// The host's identifier for this request, echoed on every event it
    /// produces. The host picks it; the backend only carries it.
    pub tag: u64,
    /// `GET`, `POST`, `PUT`, `DELETE`, … Uppercased by the host before it gets
    /// here, so a backend can compare directly.
    pub method: String,
    pub url: String,
    /// Sent as given, in order. A backend that did not claim
    /// [`Caps::HEADERS`] may drop them.
    pub headers: Vec<(String, String)>,
    /// Deliver the body in pieces rather than in one event. Ignored — not
    /// refused — by a backend without [`Caps::STREAM`].
    pub stream: bool,
    /// Give up after this many milliseconds. `0` means the backend's own
    /// default, which is what almost every caller wants.
    pub timeout_ms: u32,
    /// Stop reading after this many bytes and fail the request. `0` means no
    /// limit.
    ///
    /// Enforced *during* the transfer, not after, which is the whole point: the
    /// callers that want it are fetching images from URLs a server chose — a
    /// marketplace thumbnail, an avatar, an image in a markdown README — and a
    /// cap applied once the bytes are already in memory protects nothing.
    pub max_bytes: u32,
}

impl Request {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.tag);
        w.str(&self.method);
        w.str(&self.url);
        w.count(self.headers.len());
        for (name, value) in &self.headers {
            w.str(name);
            w.str(value);
        }
        w.bool(self.stream);
        w.u32(self.timeout_ms);
        w.u32(self.max_bytes);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            tag: r.u64()?,
            method: r.string()?,
            url: r.string()?,
            headers: r.list(|r| Ok((r.string()?, r.string()?)))?,
            stream: r.bool()?,
            timeout_ms: r.u32()?,
            max_bytes: r.u32()?,
        })
    }
}

/// Which kind of thing happened to a request.
///
/// A `u16` tag on the wire and a plain enum in Rust: unlike the op codes in
/// [`sys`](crate::sys), this is decoded by [`crate::wire`], which returns
/// [`WireError::UnknownTag`] for a value it does not know rather than
/// materialising it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventKind {
    /// The whole body, for a non-streaming request. Terminal.
    Response,
    /// One piece of a streaming body. More follow.
    Chunk,
    /// A streaming body finished. Empty body. Terminal.
    End,
    /// The request failed — DNS, connect, TLS, timeout, a read that died
    /// mid-stream. The body holds the error text. Terminal.
    ///
    /// **A 404 is not this.** A server that answered is a successful request;
    /// the status is on the event and the body is whatever it sent. Reserving
    /// this for transport failure is what lets a caller read the `{"error": …}`
    /// body an API returns with its 400.
    Error,
}

impl EventKind {
    /// Whether nothing more will arrive for this tag. A consumer stops polling
    /// once it sees one.
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Chunk)
    }

    const fn tag(self) -> u16 {
        match self {
            Self::Response => 0,
            Self::Chunk => 1,
            Self::End => 2,
            Self::Error => 3,
        }
    }

    const fn from_tag(tag: u16) -> Option<Self> {
        match tag {
            0 => Some(Self::Response),
            1 => Some(Self::Chunk),
            2 => Some(Self::End),
            3 => Some(Self::Error),
            _ => None,
        }
    }
}

/// Something that happened to a request, on its way back to the host.
#[derive(Clone, Debug)]
pub struct Event {
    /// The [`Request::tag`] this belongs to.
    pub tag: u64,
    pub kind: EventKind,
    /// HTTP status, or `0` when the request never reached a response. Repeated
    /// on every chunk of a stream so a consumer that keeps only the latest still
    /// knows it.
    pub status: u16,
    /// Response headers. Empty on [`EventKind::Chunk`] after the first — they
    /// are sent once, with the first event that has them.
    pub headers: Vec<(String, String)>,
    /// The bytes. `Vec<u8>` rather than `String` because half the callers are
    /// fetching PNGs, and a lossy UTF-8 conversion at the boundary would corrupt
    /// every one of them.
    pub body: Vec<u8>,
}

impl Event {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.tag);
        w.u16(self.kind.tag());
        w.u16(self.status);
        w.count(self.headers.len());
        for (name, value) in &self.headers {
            w.str(name);
            w.str(value);
        }
        w.bytes_field(&self.body);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let tag = r.u64()?;
        let kind_tag = r.u16()?;
        let kind = EventKind::from_tag(kind_tag).ok_or(WireError::UnknownTag(kind_tag as u32))?;
        Ok(Self {
            tag,
            kind,
            status: r.u16()?,
            headers: r.list(|r| Ok((r.string()?, r.string()?)))?,
            body: r.bytes_field()?.to_vec(),
        })
    }
}

/// Encode a batch of events as the reply to [`Poll`](crate::sys::NetOp::Poll).
pub fn write_events(w: &mut Writer, events: &[Event]) {
    w.count(events.len());
    for event in events {
        event.encode(w);
    }
}

/// Decode what [`write_events`] wrote.
pub fn read_events(r: &mut Reader) -> Result<Vec<Event>, WireError> {
    r.list(Event::decode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn a_request_round_trips() {
        let request = Request {
            tag: 42,
            method: "POST".to_string(),
            url: "https://renzora.com/api/assets?page=2".to_string(),
            headers: vec![
                ("Authorization".to_string(), "Bearer xyz".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            stream: true,
            timeout_ms: 30_000,
            max_bytes: 0,
        };
        let mut w = Writer::new();
        request.encode(&mut w);
        let bytes = w.into_bytes();

        let out = Request::decode(&mut Reader::new(&bytes)).unwrap();
        assert_eq!(out.tag, 42);
        assert_eq!(out.method, "POST");
        assert_eq!(out.headers.len(), 2);
        assert!(out.stream);
        assert_eq!(out.timeout_ms, 30_000);
    }

    /// The reason `body` is bytes: a thumbnail is not UTF-8, and a boundary that
    /// converted lossily would corrupt every image the asset browser fetches.
    #[test]
    fn a_binary_body_survives_the_codec() {
        let body = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0xff, 0xfe, 0x00];
        let event = Event {
            tag: 1,
            kind: EventKind::Response,
            status: 200,
            headers: vec![("Content-Type".to_string(), "image/png".to_string())],
            body: body.clone(),
        };
        let mut w = Writer::new();
        write_events(&mut w, &[event]);
        let bytes = w.into_bytes();

        let out = read_events(&mut Reader::new(&bytes)).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].body, body);
        assert_eq!(out[0].status, 200);
    }

    #[test]
    fn an_empty_poll_round_trips() {
        let mut w = Writer::new();
        write_events(&mut w, &[]);
        let bytes = w.into_bytes();
        assert!(read_events(&mut Reader::new(&bytes)).unwrap().is_empty());
    }

    /// A newer backend inventing a fifth event kind must be reported, not
    /// materialised into an enum that has no such variant.
    #[test]
    fn an_unknown_event_kind_is_refused() {
        let mut w = Writer::new();
        w.count(1);
        w.u64(1);
        w.u16(99);
        w.u16(200);
        w.count(0);
        w.bytes_field(&[]);
        let bytes = w.into_bytes();

        assert_eq!(
            read_events(&mut Reader::new(&bytes)).unwrap_err(),
            WireError::UnknownTag(99)
        );
    }

    #[test]
    fn only_a_chunk_is_non_terminal() {
        assert!(!EventKind::Chunk.is_terminal());
        assert!(EventKind::Response.is_terminal());
        assert!(EventKind::End.is_terminal());
        assert!(EventKind::Error.is_terminal());
    }
}
