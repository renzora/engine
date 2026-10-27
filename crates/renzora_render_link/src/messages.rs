//! Scene messages from the editor to the render host, carried on the host's
//! stdin.
//!
//! The shared file carries what changes every frame (camera, frames); this
//! carries what changes when you edit. A pipe suits it: messages are variable
//! length (a full scene can be megabytes, a gizmo drag is one entity), they
//! must arrive in order, and none may be dropped, which a "latest value" slot
//! in the file cannot promise.
//!
//! Framing is a little-endian `u32` length, then the body: a one-byte tag and
//! the fields. Hand-rolled rather than serde, to keep this crate free of
//! dependencies both processes would have to agree on the version of.

use std::io::{self, Read, Write};

/// The largest message accepted. A scene snapshot is text; this is far above
/// any real one and only guards against reading garbage as a length.
const MAX_MESSAGE: usize = 512 * 1024 * 1024;

/// Entities are keyed by the editor's `Entity::to_bits()`: stable for the
/// entity's life, and the host keeps a map from it to its own entities.
pub type EditorEntity = u64;

#[derive(Clone, Debug, PartialEq)]
pub enum SceneMessage {
    /// Replace the whole scene with this snapshot (BSN). Sent once when the
    /// host starts, so it shows the editor's state, unsaved edits included.
    Reset { bsn: String },
    /// Entities that changed, components removed from entities, and entities
    /// despawned, since the last message.
    Patch {
        bsn: String,
        removed: Vec<(EditorEntity, Vec<String>)>,
        despawned: Vec<EditorEntity>,
    },
}

const TAG_RESET: u8 = 1;
const TAG_PATCH: u8 = 2;

impl SceneMessage {
    /// Encode with its length prefix, ready to write to the pipe.
    pub fn encode(&self) -> Vec<u8> {
        let mut body = Vec::new();
        match self {
            SceneMessage::Reset { bsn } => {
                body.push(TAG_RESET);
                put_str(&mut body, bsn);
            }
            SceneMessage::Patch { bsn, removed, despawned } => {
                body.push(TAG_PATCH);
                put_str(&mut body, bsn);
                put_u32(&mut body, removed.len() as u32);
                for (entity, paths) in removed {
                    body.extend_from_slice(&entity.to_le_bytes());
                    put_u32(&mut body, paths.len() as u32);
                    for path in paths {
                        put_str(&mut body, path);
                    }
                }
                put_u32(&mut body, despawned.len() as u32);
                for entity in despawned {
                    body.extend_from_slice(&entity.to_le_bytes());
                }
            }
        }
        let mut out = Vec::with_capacity(4 + body.len());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Write one framed message.
    pub fn write_to(&self, w: &mut impl Write) -> io::Result<()> {
        w.write_all(&self.encode())?;
        w.flush()
    }

    /// Read one framed message. `Ok(None)` at a clean end of stream, which is
    /// how the host learns the editor has gone.
    pub fn read_from(r: &mut impl Read) -> io::Result<Option<Self>> {
        let mut len = [0u8; 4];
        match r.read_exact(&mut len) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
        let len = u32::from_le_bytes(len) as usize;
        if len == 0 || len > MAX_MESSAGE {
            return Err(bad("message length out of range"));
        }
        let mut body = vec![0u8; len];
        r.read_exact(&mut body)?;
        let mut cur = Cursor { bytes: &body, at: 0 };
        let message = match cur.u8()? {
            TAG_RESET => SceneMessage::Reset { bsn: cur.string()? },
            TAG_PATCH => {
                let bsn = cur.string()?;
                let mut removed = Vec::new();
                for _ in 0..cur.u32()? {
                    let entity = cur.u64()?;
                    let mut paths = Vec::new();
                    for _ in 0..cur.u32()? {
                        paths.push(cur.string()?);
                    }
                    removed.push((entity, paths));
                }
                let mut despawned = Vec::new();
                for _ in 0..cur.u32()? {
                    despawned.push(cur.u64()?);
                }
                SceneMessage::Patch { bsn, removed, despawned }
            }
            _ => return Err(bad("unknown message tag")),
        };
        Ok(Some(message))
    }
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_string())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len());
        let end = end.ok_or_else(|| bad("message is truncated"))?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap_or_default()))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap_or_default()))
    }
    fn string(&mut self) -> io::Result<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| bad("string is not UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip_in_order() {
        let a = SceneMessage::Reset { bsn: "(scene)".into() };
        let b = SceneMessage::Patch {
            bsn: "(patch)".into(),
            removed: vec![(7, vec!["bevy_light::PointLight".into()])],
            despawned: vec![3, 9],
        };
        let mut pipe = Vec::new();
        a.write_to(&mut pipe).unwrap();
        b.write_to(&mut pipe).unwrap();
        let mut r = &pipe[..];
        assert_eq!(SceneMessage::read_from(&mut r).unwrap(), Some(a));
        assert_eq!(SceneMessage::read_from(&mut r).unwrap(), Some(b));
        assert_eq!(SceneMessage::read_from(&mut r).unwrap(), None, "clean end of stream");
    }

    #[test]
    fn garbage_is_an_error_not_a_hang() {
        let mut r = &[0xFF, 0xFF, 0xFF, 0xFF, 1][..];
        assert!(SceneMessage::read_from(&mut r).is_err());
        let truncated = SceneMessage::Reset { bsn: "abc".into() }.encode();
        let mut r = &truncated[..truncated.len() - 1];
        assert!(SceneMessage::read_from(&mut r).is_err());
    }
}
