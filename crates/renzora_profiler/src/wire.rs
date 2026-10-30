//! The profiler's stream: what a game sends the companion, and back.
//!
//! Binary and hand-rolled, because it carries every span of every frame: a
//! busy editor frame is around 5,000 spans, 60 times a second. As JSON that is
//! megabytes a second of text to build and parse; here a span is 19 bytes.
//!
//! Every packet is `u32 length` (of what follows), `u8 kind`, then the body,
//! all little-endian. Names (span, pass and counter names) and thread names
//! are sent once, in their own packets, before the first frame that uses them;
//! frames refer to them by number.
//!
//! [`VERSION`] moves with any change to a packet's shape. The two ends come
//! from one build almost always; when they do not (a phone running last week's
//! export), the hello says so and the companion refuses rather than misreads.

use std::io::{self, Read, Write};

use crate::{Counter, GpuPass, ProfileFrame, Span};

/// Bumped whenever a packet's layout changes. 2 added probes.
pub const VERSION: u16 = 2;

/// A probe as the game lists it; see `renzora::probes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeInfo {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub category: String,
}

/// Largest packet either end accepts. A frame of 100,000 spans is about 2 MB;
/// anything past this is a corrupt length, not a real frame.
pub const MAX_PACKET: u32 = 32 * 1024 * 1024;

/// What the game sends.
#[derive(Debug, Clone, PartialEq)]
pub enum GamePacket {
    /// First packet on a connection, after the companion's [`ControlPacket::Hello`]
    /// passed: who this is.
    Hello {
        version: u16,
        /// The project's name, or "Editor" for the editor itself.
        name: String,
        /// `windows`, `android`, …
        platform: String,
        /// The engine version it was built from.
        build: String,
    },
    /// Interned names, with their ids.
    Names(Vec<(u32, String)>),
    /// Thread ids and their names.
    Threads(Vec<(u32, String)>),
    Frame(ProfileFrame),
    /// Every probe this process offers. Sent on connecting and again if the
    /// list changes (a plugin loaded late).
    Probes(Vec<ProbeInfo>),
    /// One probe's report, as `renzora::probes::ProbeReport` JSON. Reports
    /// are a few kilobytes a few times a second, so JSON's cost is nothing
    /// here, and it lets the report's shape grow without a version bump.
    ProbeReport { id: String, json: String },
}

/// What the companion sends.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlPacket {
    /// First packet on a connection: the pairing key, and the companion's
    /// stream version.
    Hello { version: u16, key: String },
    /// Start or stop sending frames. A connected companion that is not
    /// recording costs the game nothing but the connection.
    Record(bool),
    /// The probes to send reports for, replacing the last list. Empty stops
    /// them all.
    Watch(Vec<String>),
    /// A probe's toggle or button was used: `value` is a toggle's new state.
    ProbeAct { probe: String, item: String, value: Option<bool> },
}

const GAME_HELLO: u8 = 1;
const NAMES: u8 = 2;
const THREADS: u8 = 3;
const FRAME: u8 = 4;
const PROBES: u8 = 5;
const PROBE_REPORT: u8 = 6;
const CONTROL_HELLO: u8 = 10;
const RECORD: u8 = 11;
const WATCH: u8 = 12;
const PROBE_ACT: u8 = 13;

// ── Writing ─────────────────────────────────────────────────────────────────

struct Out(Vec<u8>);

impl Out {
    fn new(kind: u8) -> Self {
        // Room for the length, filled in by `finish`.
        Self(vec![0, 0, 0, 0, kind])
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        let bytes = &s.as_bytes()[..s.len().min(u16::MAX as usize)];
        self.u16(bytes.len() as u16);
        self.0.extend_from_slice(bytes);
    }
    /// A string past `str`'s 64 KB: a probe report with a long table.
    fn long_str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn finish(mut self) -> Vec<u8> {
        let len = (self.0.len() - 4) as u32;
        self.0[..4].copy_from_slice(&len.to_le_bytes());
        self.0
    }
}

impl GamePacket {
    /// The packet, length prefix included, ready to write.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            GamePacket::Hello { version, name, platform, build } => {
                let mut o = Out::new(GAME_HELLO);
                o.u16(*version);
                o.str(name);
                o.str(platform);
                o.str(build);
                o.finish()
            }
            GamePacket::Names(names) | GamePacket::Threads(names) => {
                let kind = if matches!(self, GamePacket::Names(_)) { NAMES } else { THREADS };
                let mut o = Out::new(kind);
                o.u32(names.len() as u32);
                for (id, text) in names {
                    o.u32(*id);
                    o.str(text);
                }
                o.finish()
            }
            GamePacket::Frame(frame) => Self::encode_frame(frame),
            GamePacket::Probes(probes) => {
                let mut o = Out::new(PROBES);
                o.u32(probes.len() as u32);
                for p in probes {
                    o.str(&p.id);
                    o.str(&p.title);
                    o.str(&p.icon);
                    o.str(&p.category);
                }
                o.finish()
            }
            GamePacket::ProbeReport { id, json } => {
                let mut o = Out::new(PROBE_REPORT);
                o.str(id);
                o.long_str(json);
                o.finish()
            }
        }
    }

    /// A [`GamePacket::Frame`] encoded straight from a borrowed frame. The
    /// game's writer threads use this, so a frame shared between companions is
    /// never copied to be sent.
    pub fn encode_frame(frame: &ProfileFrame) -> Vec<u8> {
        let mut o = Out::new(FRAME);
        o.u64(frame.index);
        o.u64(frame.start_ns);
        o.u64(frame.end_ns);
        o.u32(frame.spans.len() as u32);
        for s in &frame.spans {
            o.u32(s.thread);
            o.u32(s.name);
            // Start and duration as u32 nanoseconds relative to the frame:
            // 4.3 s of range, which no frame or span reaches.
            let early = s.start_ns < frame.start_ns;
            let offset = if early {
                frame.start_ns - s.start_ns
            } else {
                s.start_ns - frame.start_ns
            };
            o.u32(offset.min(u32::MAX as u64) as u32);
            o.u32((s.end_ns.saturating_sub(s.start_ns)).min(u32::MAX as u64) as u32);
            o.u16(s.depth);
            // A span that began before the frame mark (a render system still
            // running at it) counts back from the frame start.
            o.u8(u8::from(early));
        }
        o.u16(frame.gpu.len().min(u16::MAX as usize) as u16);
        for g in frame.gpu.iter().take(u16::MAX as usize) {
            o.u32(g.name);
            o.f32(g.ms);
            o.u8(g.depth);
        }
        o.u16(frame.counters.len().min(u16::MAX as usize) as u16);
        for c in frame.counters.iter().take(u16::MAX as usize) {
            o.u32(c.name);
            o.f64(c.value);
        }
        o.finish()
    }

    /// Read one packet. `Ok(None)` means the other end closed cleanly.
    pub fn read(from: &mut impl Read) -> io::Result<Option<Self>> {
        let Some((kind, body)) = read_raw(from)? else { return Ok(None) };
        let mut i = In(&body);
        let packet = match kind {
            GAME_HELLO => GamePacket::Hello {
                version: i.u16()?,
                name: i.str()?,
                platform: i.str()?,
                build: i.str()?,
            },
            NAMES | THREADS => {
                let count = i.u32()? as usize;
                let mut list = Vec::with_capacity(count.min(1 << 16));
                for _ in 0..count {
                    list.push((i.u32()?, i.str()?));
                }
                if kind == NAMES { GamePacket::Names(list) } else { GamePacket::Threads(list) }
            }
            FRAME => {
                let index = i.u64()?;
                let start_ns = i.u64()?;
                let end_ns = i.u64()?;
                let count = i.u32()? as usize;
                let mut spans = Vec::with_capacity(count.min(1 << 20));
                for _ in 0..count {
                    let thread = i.u32()?;
                    let name = i.u32()?;
                    let offset = i.u32()? as u64;
                    let duration = i.u32()? as u64;
                    let depth = i.u16()?;
                    let early = i.u8()? != 0;
                    let start = if early {
                        start_ns.saturating_sub(offset)
                    } else {
                        start_ns + offset
                    };
                    spans.push(Span { thread, name, start_ns: start, end_ns: start + duration, depth });
                }
                let gpu = (0..i.u16()?)
                    .map(|_| Ok(GpuPass { name: i.u32()?, ms: i.f32()?, depth: i.u8()? }))
                    .collect::<io::Result<Vec<_>>>()?;
                let counters = (0..i.u16()?)
                    .map(|_| Ok(Counter { name: i.u32()?, value: i.f64()? }))
                    .collect::<io::Result<Vec<_>>>()?;
                GamePacket::Frame(ProfileFrame { index, start_ns, end_ns, spans, gpu, counters })
            }
            PROBES => {
                let count = i.u32()? as usize;
                let mut list = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    list.push(ProbeInfo { id: i.str()?, title: i.str()?, icon: i.str()?, category: i.str()? });
                }
                GamePacket::Probes(list)
            }
            PROBE_REPORT => GamePacket::ProbeReport { id: i.str()?, json: i.long_str()? },
            other => return Err(bad(format!("unknown game packet kind {other}"))),
        };
        Ok(Some(packet))
    }
}

impl ControlPacket {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ControlPacket::Hello { version, key } => {
                let mut o = Out::new(CONTROL_HELLO);
                o.u16(*version);
                o.str(key);
                o.finish()
            }
            ControlPacket::Record(on) => {
                let mut o = Out::new(RECORD);
                o.u8(u8::from(*on));
                o.finish()
            }
            ControlPacket::Watch(ids) => {
                let mut o = Out::new(WATCH);
                o.u32(ids.len() as u32);
                for id in ids {
                    o.str(id);
                }
                o.finish()
            }
            ControlPacket::ProbeAct { probe, item, value } => {
                let mut o = Out::new(PROBE_ACT);
                o.str(probe);
                o.str(item);
                // 0 for a button, 1 and 2 for a toggle turned off and on.
                o.u8(match value {
                    None => 0,
                    Some(false) => 1,
                    Some(true) => 2,
                });
                o.finish()
            }
        }
    }

    pub fn read(from: &mut impl Read) -> io::Result<Option<Self>> {
        let Some((kind, body)) = read_raw(from)? else { return Ok(None) };
        let mut i = In(&body);
        Ok(Some(match kind {
            CONTROL_HELLO => ControlPacket::Hello { version: i.u16()?, key: i.str()? },
            RECORD => ControlPacket::Record(i.u8()? != 0),
            WATCH => {
                let count = i.u32()? as usize;
                let mut ids = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    ids.push(i.str()?);
                }
                ControlPacket::Watch(ids)
            }
            PROBE_ACT => ControlPacket::ProbeAct {
                probe: i.str()?,
                item: i.str()?,
                value: match i.u8()? {
                    1 => Some(false),
                    2 => Some(true),
                    _ => None,
                },
            },
            other => return Err(bad(format!("unknown control packet kind {other}"))),
        }))
    }
}

/// Write a packet already encoded with `encode`.
pub fn write(to: &mut impl Write, packet: &[u8]) -> io::Result<()> {
    to.write_all(packet)
}

// ── Reading ─────────────────────────────────────────────────────────────────

fn bad(why: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why)
}

/// One packet's kind and body. `None` on a clean close before a new packet.
fn read_raw(from: &mut impl Read) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut len = [0u8; 4];
    match from.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len);
    if len == 0 || len > MAX_PACKET {
        return Err(bad(format!("packet length {len} out of range")));
    }
    let mut body = vec![0u8; len as usize];
    from.read_exact(&mut body)?;
    let kind = body.remove(0);
    Ok(Some((kind, body)))
}

struct In<'a>(&'a [u8]);

impl In<'_> {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        if self.0.len() < n {
            return Err(bad("packet ended early".into()));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap_or_default()))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap_or_default()))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap_or_default()))
    }
    fn f32(&mut self) -> io::Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap_or_default()))
    }
    fn f64(&mut self) -> io::Result<f64> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap_or_default()))
    }
    fn str(&mut self) -> io::Result<String> {
        let len = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
    fn long_str(&mut self) -> io::Result<String> {
        let len = self.u32()? as usize;
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip_game(packet: GamePacket) {
        let bytes = packet.encode();
        let back = GamePacket::read(&mut bytes.as_slice()).unwrap().unwrap();
        assert_eq!(back, packet);
    }

    #[test]
    fn every_game_packet_survives() {
        round_trip_game(GamePacket::Hello {
            version: VERSION,
            name: "forest_demo".into(),
            platform: "android".into(),
            build: "r1-alpha8".into(),
        });
        round_trip_game(GamePacket::Names(vec![(0, "physics::step".into()), (7, "ünïcode".into())]));
        round_trip_game(GamePacket::Threads(vec![(3, "Compute Task Pool (2)".into())]));
        round_trip_game(GamePacket::Frame(ProfileFrame {
            index: 42,
            start_ns: 1_000_000,
            end_ns: 17_600_000,
            spans: vec![
                Span { thread: 0, name: 1, start_ns: 1_200_000, end_ns: 5_000_000, depth: 0 },
                // Began before the frame mark: its true start must survive.
                Span { thread: 2, name: 3, start_ns: 900_000, end_ns: 3_100_000, depth: 1 },
            ],
            gpu: vec![GpuPass { name: 9, ms: 6.9, depth: 0 }],
            counters: vec![Counter { name: 4, value: 41288.0 }],
        }));
        round_trip_game(GamePacket::Probes(vec![ProbeInfo {
            id: "camera_debug".into(),
            title: "Camera Debug".into(),
            icon: "video-camera".into(),
            category: "Rendering".into(),
        }]));
        // Past the 64 KB a plain string can carry.
        round_trip_game(GamePacket::ProbeReport { id: "ecs_stats".into(), json: "x".repeat(100_000) });
    }

    #[test]
    fn control_packets_survive() {
        for packet in [
            ControlPacket::Hello { version: VERSION, key: "rz-7f3a".into() },
            ControlPacket::Record(true),
            ControlPacket::Record(false),
            ControlPacket::Watch(vec!["performance".into(), "memory_profiler".into()]),
            ControlPacket::Watch(Vec::new()),
            ControlPacket::ProbeAct { probe: "render_toggles".into(), item: "ssao".into(), value: Some(false) },
            ControlPacket::ProbeAct { probe: "camera_debug".into(), item: "select:3".into(), value: None },
        ] {
            let bytes = packet.encode();
            assert_eq!(ControlPacket::read(&mut bytes.as_slice()).unwrap(), Some(packet));
        }
    }

    #[test]
    fn several_packets_read_back_in_order_then_end_cleanly() {
        let mut stream = Vec::new();
        stream.extend(GamePacket::Names(vec![(0, "a".into())]).encode());
        stream.extend(GamePacket::Threads(vec![(0, "main".into())]).encode());
        let mut reader = stream.as_slice();
        assert!(matches!(GamePacket::read(&mut reader).unwrap(), Some(GamePacket::Names(_))));
        assert!(matches!(GamePacket::read(&mut reader).unwrap(), Some(GamePacket::Threads(_))));
        assert_eq!(GamePacket::read(&mut reader).unwrap(), None);
    }

    #[test]
    fn garbage_is_an_error_not_a_hang_or_a_huge_allocation() {
        let huge = [0xFF, 0xFF, 0xFF, 0x7F, FRAME];
        assert!(GamePacket::read(&mut huge.as_slice()).is_err());
        // A frame cut off half way.
        let mut cut = GamePacket::Frame(ProfileFrame::default()).encode();
        cut.truncate(cut.len() - 3);
        assert!(GamePacket::read(&mut cut.as_slice()).is_err());
    }

    /// The size the module docs promise: a span is 19 bytes.
    #[test]
    fn a_span_costs_19_bytes() {
        let empty = GamePacket::Frame(ProfileFrame::default()).encode().len();
        let one = GamePacket::Frame(ProfileFrame {
            spans: vec![Span { thread: 0, name: 0, start_ns: 5, end_ns: 9, depth: 0 }],
            ..Default::default()
        })
        .encode()
        .len();
        assert_eq!(one - empty, 19);
    }
}
