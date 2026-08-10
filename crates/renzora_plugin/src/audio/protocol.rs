//! The vocabulary that rides on the audio boundary: one request and one reply
//! type per [`AudioOp`], and the codec for each.
//!
//! None of this is named by [`Interface`](crate::sys::Interface), so it can grow
//! without moving the ABI — the same split [`crate::script`] draws between its
//! commands and [`crate::sys`]. Adding a field to a request here is a
//! `renzora_plugin` semver bump; adding an op is a MINOR; neither is a break.
//!
//! Everything is encoded with [`crate::wire`], which both sides compile from
//! this one source. See that module for why a derive-based format would be the
//! wrong choice at a boundary whose two halves resolve dependencies separately.

use alloc::string::String;
use alloc::vec::Vec;

use crate::wire::{Reader, WireError, Writer};

/// What a backend can actually do.
///
/// A bitfield rather than an assumption, because the two backends that matter
/// differ: a native cpal build captures and decodes locally, while a WebAudio
/// build gets decoding free from the browser and cannot capture through cpal at
/// all. Without this the same game code would silently do nothing on the web —
/// which is exactly the failure a capability answer is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Caps(pub u32);

impl Caps {
    /// Microphone / line input is available.
    pub const CAPTURE: Self = Self(1 << 0);
    /// Positional audio is applied. A backend without it plays every voice 2D.
    pub const SPATIAL: Self = Self(1 << 1);
    /// [`AudioOp::PushFrames`](crate::sys::AudioOp::PushFrames) is honoured.
    pub const FEEDS: Self = Self(1 << 2);
    /// Device enumeration returns something. A browser cannot list devices
    /// before permission is granted, so this can be false at startup and true
    /// later.
    pub const DEVICE_LIST: Self = Self(1 << 3);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Reply to [`AudioOp::Init`](crate::sys::AudioOp::Init).
#[derive(Debug, Clone, PartialEq)]
pub struct BackendInfo {
    /// The rate the device negotiated. The host needs it to convert seconds to
    /// frames the same way the backend does.
    pub sample_rate: u32,
    pub caps: Caps,
    /// Name of the output device that was opened, for the editor to display.
    pub device: String,
}

/// One bus, as the host describes it.
///
/// The whole board is sent on every change rather than diffed: it is a few dozen
/// entries, and a diff protocol would be a second source of truth about what the
/// mixer looks like — one that can get out of step with the first and produce a
/// board nobody authored.
#[derive(Debug, Clone, PartialEq)]
pub struct BusState {
    /// The permanent routing key, never the display name. See the engine's
    /// `AudioConfig` for why those are different things.
    pub key: String,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
}

/// How a positioned voice is heard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmitterState {
    pub position: [f32; 3],
    pub min_distance: f32,
    pub max_distance: f32,
    /// `0` logarithmic, `1` linear. A number rather than an enum because it
    /// crosses the boundary; an unknown value means logarithmic.
    pub rolloff: u32,
}

/// Where the ears are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListenerState {
    pub position: [f32; 3],
    /// Unit vector out of the listener's right ear.
    pub right: [f32; 3],
}

/// Request for [`AudioOp::LoadClip`](crate::sys::AudioOp::LoadClip). The file
/// bytes ride in `AudioCall::blob`, not here.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadClip {
    /// Host-assigned handle. The host allocates it so it can name the clip in a
    /// `Play` before the backend has finished decoding.
    pub clip: u64,
    /// File extension without the dot, as a decoding *hint* only.
    pub extension: String,
}

/// Reply to [`AudioOp::LoadClip`](crate::sys::AudioOp::LoadClip).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipInfo {
    pub duration: f64,
    pub sample_rate: u32,
}

/// Request for [`AudioOp::Play`](crate::sys::AudioOp::Play).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayRequest {
    /// Host-assigned voice handle, for the same reason as [`LoadClip::clip`].
    pub voice: u64,
    pub clip: u64,
    pub bus: String,
    pub gain: f32,
    pub pan: f32,
    pub pitch: f64,
    /// Loop region in seconds.
    pub looping: Option<(f64, f64)>,
    pub fade_in: f32,
    pub start: f64,
    pub emitter: Option<EmitterState>,
    /// How much of this voice reaches the shared reverb and delay, 0..1 —
    /// `AudioPlayer::reverb_send` and `delay_send` verbatim. A backend without
    /// effects ignores them; nothing about the request becomes invalid.
    pub reverb_send: f32,
    pub delay_send: f32,
}

/// What a [`AudioOp::Stop`](crate::sys::AudioOp::Stop) targets.
#[derive(Debug, Clone, PartialEq)]
pub enum StopTarget {
    Voice(u64),
    /// Every voice on a bus, by routing key.
    Bus(String),
    All,
}

/// Request for [`AudioOp::Stop`](crate::sys::AudioOp::Stop).
#[derive(Debug, Clone, PartialEq)]
pub struct StopRequest {
    pub target: StopTarget,
    /// Fade-out in seconds. Zero still ramps over a block — cutting a waveform
    /// mid-cycle is a step, and a step is broadband noise.
    pub fade: f32,
}

/// Request for [`AudioOp::Update`](crate::sys::AudioOp::Update) — everything
/// that changes per frame, in one call.
///
/// Batched rather than one call per moved emitter because the boundary crossing
/// is the expensive part: a scene with two hundred positioned sounds would
/// otherwise make two hundred FFI calls a frame to move them a few centimetres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateRequest {
    pub listener: Option<ListenerState>,
    /// `(voice, position)` for every emitter that moved.
    pub moved: Vec<(u64, [f32; 3])>,
    /// `(voice, gain)` for every voice whose level changed.
    pub gains: Vec<(u64, f32)>,
    /// `(voice, pitch)` for every voice that was retuned.
    pub pitches: Vec<(u64, f64)>,
    /// `(voice, bus key)` for every voice re-routed.
    ///
    /// A voice moves between buses rather than being restarted for it — changing
    /// where a sound goes is not a reason to hear it from the top again.
    pub buses: Vec<(u64, String)>,
    /// `(voice, pan)` for every voice re-panned.
    pub pans: Vec<(u64, f32)>,
    /// `(voice, emitter)` for every positioned voice whose spatial parameters
    /// changed — distances or rolloff, not just position.
    ///
    /// Separate from `moved` because that one carries a position per frame for
    /// every live emitter and wants to stay three floats; this one is rare and
    /// replaces the whole thing.
    pub emitters: Vec<(u64, EmitterState)>,
    /// `(voice, paused)` for every voice held or released.
    ///
    /// Batched here with the rest rather than given ops of their own: pausing is
    /// rare, but a separate op would be a boundary crossing for something that
    /// is already crossing this frame anyway.
    pub paused: Vec<(u64, bool)>,
}

/// Reply to [`AudioOp::Update`](crate::sys::AudioOp::Update).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateReply {
    /// Peak level per bus, in the order the host sent them in `SetBuses`.
    pub peaks: Vec<f32>,
    /// Voices that have finished since the last update, so the host can drop
    /// its bookkeeping. Without this the host would have to poll every handle
    /// it ever created.
    pub finished: Vec<u64>,
}

/// Request for [`AudioOp::OpenCapture`](crate::sys::AudioOp::OpenCapture).
#[derive(Debug, Clone, PartialEq)]
pub struct OpenCapture {
    pub capture: u64,
    /// Device name, or `None` for the system default.
    pub device: Option<String>,
}

/// Reply to [`AudioOp::OpenCapture`](crate::sys::AudioOp::OpenCapture).
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureInfo {
    /// The capture device's own rate, which need not match the output's — a
    /// caller monitoring through a bus has to resample, one recording to disk
    /// must not.
    pub sample_rate: u32,
    pub device: String,
}

/// Reply to [`AudioOp::ListDevices`](crate::sys::AudioOp::ListDevices).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceList {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

// ── Codecs ───────────────────────────────────────────────────────────────────
//
// Written out by hand, one pair per type. Verbose, and deliberately so: see
// `crate::wire`'s module doc for why a derive would put the format in a
// dependency that the two sides resolve separately.

/// Append interleaved `f32` samples as a length-prefixed byte field.
///
/// Bytes rather than a per-sample loop because a block of audio is thousands of
/// samples and this runs every frame — `bytes_field` is one length and one
/// `memcpy`. Little-endian is explicit, matching the rest of the codec.
pub fn write_samples(w: &mut Writer, samples: &[f32]) {
    let mut bytes = Vec::with_capacity(samples.len() * 4);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    w.bytes_field(&bytes);
}

/// Read what [`write_samples`] wrote.
///
/// A trailing partial sample is dropped rather than treated as an error: it can
/// only mean a truncated payload, and half a sample is not recoverable either
/// way.
pub fn read_samples(r: &mut Reader) -> Result<Vec<f32>, WireError> {
    let bytes = r.bytes_field()?;
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

impl BackendInfo {
    pub fn encode(&self, w: &mut Writer) {
        w.u32(self.sample_rate);
        w.u32(self.caps.0);
        w.str(&self.device);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            sample_rate: r.u32()?,
            caps: Caps(r.u32()?),
            device: r.string()?,
        })
    }
}

impl BusState {
    pub fn encode(&self, w: &mut Writer) {
        w.str(&self.key);
        w.f32(self.gain);
        w.f32(self.pan);
        w.bool(self.muted);
        w.bool(self.soloed);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            key: r.string()?,
            gain: r.f32()?,
            pan: r.f32()?,
            muted: r.bool()?,
            soloed: r.bool()?,
        })
    }
}

/// Encode the whole board.
pub fn write_buses(w: &mut Writer, buses: &[BusState]) {
    w.count(buses.len());
    for bus in buses {
        bus.encode(w);
    }
}

/// Decode the whole board.
pub fn read_buses(r: &mut Reader) -> Result<Vec<BusState>, WireError> {
    let n = r.count()?;
    let mut out = Vec::with_capacity(n.min(1024));
    for _ in 0..n {
        out.push(BusState::decode(r)?);
    }
    Ok(out)
}

impl EmitterState {
    pub(crate) fn encode(&self, w: &mut Writer) {
        w.f32x3(self.position);
        w.f32(self.min_distance);
        w.f32(self.max_distance);
        w.u32(self.rolloff);
    }

    pub(crate) fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            position: [r.f32()?, r.f32()?, r.f32()?],
            min_distance: r.f32()?,
            max_distance: r.f32()?,
            rolloff: r.u32()?,
        })
    }
}

impl ListenerState {
    fn encode(&self, w: &mut Writer) {
        w.f32x3(self.position);
        w.f32x3(self.right);
    }

    fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            position: [r.f32()?, r.f32()?, r.f32()?],
            right: [r.f32()?, r.f32()?, r.f32()?],
        })
    }
}

impl LoadClip {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.clip);
        w.str(&self.extension);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            clip: r.u64()?,
            extension: r.string()?,
        })
    }
}

impl ClipInfo {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.duration.to_bits());
        w.u32(self.sample_rate);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            duration: f64::from_bits(r.u64()?),
            sample_rate: r.u32()?,
        })
    }
}

impl PlayRequest {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.voice);
        w.u64(self.clip);
        w.str(&self.bus);
        w.f32(self.gain);
        w.f32(self.pan);
        w.u64(self.pitch.to_bits());
        match self.looping {
            Some((a, b)) => {
                w.bool(true);
                w.u64(a.to_bits());
                w.u64(b.to_bits());
            }
            None => w.bool(false),
        }
        w.f32(self.fade_in);
        w.u64(self.start.to_bits());
        match &self.emitter {
            Some(e) => {
                w.bool(true);
                e.encode(w);
            }
            None => w.bool(false),
        }
        w.f32(self.reverb_send);
        w.f32(self.delay_send);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let voice = r.u64()?;
        let clip = r.u64()?;
        let bus = r.string()?;
        let gain = r.f32()?;
        let pan = r.f32()?;
        let pitch = f64::from_bits(r.u64()?);
        let looping = if r.bool()? {
            Some((f64::from_bits(r.u64()?), f64::from_bits(r.u64()?)))
        } else {
            None
        };
        let fade_in = r.f32()?;
        let start = f64::from_bits(r.u64()?);
        let emitter = if r.bool()? {
            Some(EmitterState::decode(r)?)
        } else {
            None
        };
        Ok(Self {
            voice,
            clip,
            bus,
            gain,
            pan,
            pitch,
            looping,
            fade_in,
            start,
            emitter,
            reverb_send: r.f32()?,
            delay_send: r.f32()?,
        })
    }
}

impl StopRequest {
    pub fn encode(&self, w: &mut Writer) {
        match &self.target {
            StopTarget::Voice(id) => {
                w.u8(0);
                w.u64(*id);
            }
            StopTarget::Bus(key) => {
                w.u8(1);
                w.str(key);
            }
            StopTarget::All => w.u8(2),
        }
        w.f32(self.fade);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let tag = r.u8()?;
        let target = match tag {
            0 => StopTarget::Voice(r.u64()?),
            1 => StopTarget::Bus(r.string()?),
            2 => StopTarget::All,
            // An unknown tag means the writer is newer than this build. Reading
            // on would misinterpret every byte after it, so this stops here.
            other => return Err(WireError::UnknownTag(other as u32)),
        };
        Ok(Self {
            target,
            fade: r.f32()?,
        })
    }
}

impl UpdateRequest {
    pub fn encode(&self, w: &mut Writer) {
        match &self.listener {
            Some(l) => {
                w.bool(true);
                l.encode(w);
            }
            None => w.bool(false),
        }
        w.count(self.moved.len());
        for (voice, position) in &self.moved {
            w.u64(*voice);
            w.f32x3(*position);
        }
        w.count(self.gains.len());
        for (voice, gain) in &self.gains {
            w.u64(*voice);
            w.f32(*gain);
        }
        w.count(self.pitches.len());
        for (voice, pitch) in &self.pitches {
            w.u64(*voice);
            w.u64(pitch.to_bits());
        }
        w.count(self.paused.len());
        for (voice, paused) in &self.paused {
            w.u64(*voice);
            w.bool(*paused);
        }
        w.count(self.buses.len());
        for (voice, bus) in &self.buses {
            w.u64(*voice);
            w.str(bus);
        }
        w.count(self.pans.len());
        for (voice, pan) in &self.pans {
            w.u64(*voice);
            w.f32(*pan);
        }
        w.count(self.emitters.len());
        for (voice, emitter) in &self.emitters {
            w.u64(*voice);
            emitter.encode(w);
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let listener = if r.bool()? {
            Some(ListenerState::decode(r)?)
        } else {
            None
        };
        let n = r.count()?;
        let mut moved = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            moved.push((r.u64()?, [r.f32()?, r.f32()?, r.f32()?]));
        }
        let n = r.count()?;
        let mut gains = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            gains.push((r.u64()?, r.f32()?));
        }
        let n = r.count()?;
        let mut pitches = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            pitches.push((r.u64()?, f64::from_bits(r.u64()?)));
        }
        let n = r.count()?;
        let mut paused = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            paused.push((r.u64()?, r.bool()?));
        }
        let n = r.count()?;
        let mut buses = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            buses.push((r.u64()?, r.string()?));
        }
        let n = r.count()?;
        let mut pans = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            pans.push((r.u64()?, r.f32()?));
        }
        let n = r.count()?;
        let mut emitters = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            emitters.push((r.u64()?, EmitterState::decode(r)?));
        }
        Ok(Self {
            listener,
            moved,
            gains,
            pitches,
            paused,
            buses,
            pans,
            emitters,
        })
    }
}

impl UpdateReply {
    pub fn encode(&self, w: &mut Writer) {
        w.count(self.peaks.len());
        for p in &self.peaks {
            w.f32(*p);
        }
        w.count(self.finished.len());
        for v in &self.finished {
            w.u64(*v);
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let n = r.count()?;
        let mut peaks = Vec::with_capacity(n.min(1024));
        for _ in 0..n {
            peaks.push(r.f32()?);
        }
        let n = r.count()?;
        let mut finished = Vec::with_capacity(n.min(65_536));
        for _ in 0..n {
            finished.push(r.u64()?);
        }
        Ok(Self { peaks, finished })
    }
}

impl OpenCapture {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.capture);
        w.opt_str(self.device.as_deref());
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            capture: r.u64()?,
            device: r.opt_string()?,
        })
    }
}

impl CaptureInfo {
    pub fn encode(&self, w: &mut Writer) {
        w.u32(self.sample_rate);
        w.str(&self.device);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            sample_rate: r.u32()?,
            device: r.string()?,
        })
    }
}

impl DeviceList {
    pub fn encode(&self, w: &mut Writer) {
        w.count(self.inputs.len());
        for s in &self.inputs {
            w.str(s);
        }
        w.count(self.outputs.len());
        for s in &self.outputs {
            w.str(s);
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let n = r.count()?;
        let mut inputs = Vec::with_capacity(n.min(512));
        for _ in 0..n {
            inputs.push(r.string()?);
        }
        let n = r.count()?;
        let mut outputs = Vec::with_capacity(n.min(512));
        for _ in 0..n {
            outputs.push(r.string()?);
        }
        Ok(Self { inputs, outputs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn roundtrip<T>(value: &T, enc: impl Fn(&T, &mut Writer), dec: impl Fn(&mut Reader) -> Result<T, WireError>) -> T {
        let mut w = Writer::new();
        enc(value, &mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        let out = dec(&mut r).expect("should decode");
        assert!(r.is_empty(), "decoder left {} bytes unread", r.remaining());
        out
    }

    #[test]
    fn backend_info_round_trips() {
        let v = BackendInfo {
            sample_rate: 48_000,
            caps: Caps::CAPTURE.union(Caps::SPATIAL),
            device: "Speakers".to_string(),
        };
        assert_eq!(roundtrip(&v, BackendInfo::encode, BackendInfo::decode), v);
    }

    #[test]
    fn caps_report_only_what_was_set() {
        let c = Caps::CAPTURE.union(Caps::FEEDS);
        assert!(c.contains(Caps::CAPTURE));
        assert!(c.contains(Caps::FEEDS));
        assert!(!c.contains(Caps::SPATIAL));
        assert!(!Caps::default().contains(Caps::CAPTURE));
    }

    #[test]
    fn a_whole_board_round_trips() {
        let buses = vec![
            BusState {
                key: "Master".to_string(),
                gain: 1.0,
                pan: 0.0,
                muted: false,
                soloed: false,
            },
            BusState {
                key: "Bus 1".to_string(),
                gain: 0.5,
                pan: -0.25,
                muted: true,
                soloed: true,
            },
        ];
        let mut w = Writer::new();
        write_buses(&mut w, &buses);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(read_buses(&mut r).unwrap(), buses);
    }

    #[test]
    fn a_play_request_round_trips_with_every_option_set() {
        let v = PlayRequest {
            voice: 7,
            clip: 3,
            bus: "Sfx".to_string(),
            gain: 0.8,
            pan: -0.5,
            pitch: 1.25,
            looping: Some((0.5, 2.5)),
            fade_in: 0.1,
            start: 0.25,
            emitter: Some(EmitterState {
                position: [1.0, 2.0, 3.0],
                min_distance: 1.0,
                max_distance: 50.0,
                rolloff: 1,
            }),
            reverb_send: 0.4,
            delay_send: 0.15,
        };
        assert_eq!(roundtrip(&v, PlayRequest::encode, PlayRequest::decode), v);
    }

    #[test]
    fn a_play_request_round_trips_with_every_option_unset() {
        let v = PlayRequest {
            voice: 1,
            clip: 1,
            bus: String::new(),
            gain: 1.0,
            pan: 0.0,
            pitch: 1.0,
            looping: None,
            fade_in: 0.0,
            start: 0.0,
            emitter: None,
            reverb_send: 0.0,
            delay_send: 0.0,
        };
        assert_eq!(roundtrip(&v, PlayRequest::encode, PlayRequest::decode), v);
    }

    /// `f64` fields go over as bits rather than through an `f64` writer the
    /// codec does not have — a lossy round trip here would retune every voice.
    #[test]
    fn f64_fields_survive_exactly() {
        let v = PlayRequest {
            voice: 0,
            clip: 0,
            bus: String::new(),
            gain: 0.0,
            pan: 0.0,
            pitch: core::f64::consts::PI,
            looping: Some((core::f64::consts::E, 1.0 / 3.0)),
            fade_in: 0.0,
            start: f64::MIN_POSITIVE,
            emitter: None,
            reverb_send: 0.0,
            delay_send: 0.0,
        };
        let out = roundtrip(&v, PlayRequest::encode, PlayRequest::decode);
        assert_eq!(out.pitch, core::f64::consts::PI);
        assert_eq!(out.looping, Some((core::f64::consts::E, 1.0 / 3.0)));
        assert_eq!(out.start, f64::MIN_POSITIVE);
    }

    #[test]
    fn every_stop_target_round_trips() {
        for target in [
            StopTarget::Voice(42),
            StopTarget::Bus("Music".to_string()),
            StopTarget::All,
        ] {
            let v = StopRequest {
                target: target.clone(),
                fade: 0.25,
            };
            assert_eq!(roundtrip(&v, StopRequest::encode, StopRequest::decode), v);
        }
    }

    /// A tag from a newer build must stop the decode rather than have every
    /// following byte read as something else.
    #[test]
    fn an_unknown_stop_tag_is_an_error_rather_than_a_misread() {
        let bytes = [9u8, 0, 0, 0, 0];
        let mut r = Reader::new(&bytes);
        assert_eq!(StopRequest::decode(&mut r), Err(WireError::UnknownTag(9)));
    }

    #[test]
    fn an_update_round_trips_in_both_directions() {
        let req = UpdateRequest {
            listener: Some(ListenerState {
                position: [1.0, 0.0, -1.0],
                right: [1.0, 0.0, 0.0],
            }),
            moved: vec![(1, [0.0, 1.0, 2.0]), (2, [3.0, 4.0, 5.0])],
            gains: vec![(1, 0.5)],
            pitches: vec![(2, core::f64::consts::PI)],
            paused: vec![(1, true), (2, false)],
            buses: vec![(1, String::from("Music"))],
            pans: vec![(1, -0.75)],
            emitters: vec![(
                2,
                EmitterState {
                    position: [1.0, 2.0, 3.0],
                    min_distance: 2.0,
                    max_distance: 40.0,
                    rolloff: 1,
                },
            )],
        };
        assert_eq!(
            roundtrip(&req, UpdateRequest::encode, UpdateRequest::decode),
            req
        );

        let reply = UpdateReply {
            peaks: vec![0.5, 0.25, 0.0],
            finished: vec![7, 8, 9],
        };
        assert_eq!(
            roundtrip(&reply, UpdateReply::encode, UpdateReply::decode),
            reply
        );
    }

    #[test]
    fn empty_updates_round_trip() {
        let req = UpdateRequest::default();
        assert_eq!(
            roundtrip(&req, UpdateRequest::encode, UpdateRequest::decode),
            req
        );
        let reply = UpdateReply::default();
        assert_eq!(
            roundtrip(&reply, UpdateReply::encode, UpdateReply::decode),
            reply
        );
    }

    #[test]
    fn samples_round_trip_bit_for_bit() {
        let samples = vec![0.0, 1.0, -1.0, 0.333_333_34, f32::MIN_POSITIVE];
        let mut w = Writer::new();
        write_samples(&mut w, &samples);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(read_samples(&mut r).unwrap(), samples);
    }

    #[test]
    fn no_samples_round_trips() {
        let mut w = Writer::new();
        write_samples(&mut w, &[]);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert!(read_samples(&mut r).unwrap().is_empty());
    }

    #[test]
    fn capture_and_device_types_round_trip() {
        let open = OpenCapture {
            capture: 3,
            device: Some("Mic".to_string()),
        };
        assert_eq!(roundtrip(&open, OpenCapture::encode, OpenCapture::decode), open);

        let default_device = OpenCapture {
            capture: 4,
            device: None,
        };
        assert_eq!(
            roundtrip(&default_device, OpenCapture::encode, OpenCapture::decode),
            default_device
        );

        let info = CaptureInfo {
            sample_rate: 44_100,
            device: "Mic".to_string(),
        };
        assert_eq!(roundtrip(&info, CaptureInfo::encode, CaptureInfo::decode), info);

        let list = DeviceList {
            inputs: vec!["Mic".to_string()],
            outputs: vec!["Speakers".to_string(), "Headphones".to_string()],
        };
        assert_eq!(roundtrip(&list, DeviceList::encode, DeviceList::decode), list);
    }

    /// The bytes come from another binary, so a truncated payload is ordinary
    /// untrusted input — and a panic unwinding out of an `extern "C"` frame
    /// aborts the process.
    #[test]
    fn truncated_payloads_are_errors_rather_than_panics() {
        let mut w = Writer::new();
        PlayRequest {
            voice: 1,
            clip: 1,
            bus: "Sfx".to_string(),
            gain: 1.0,
            pan: 0.0,
            pitch: 1.0,
            looping: None,
            fade_in: 0.0,
            start: 0.0,
            emitter: None,
            reverb_send: 0.0,
            delay_send: 0.0,
        }
        .encode(&mut w);
        let full = w.into_bytes();
        for cut in 0..full.len() {
            let mut r = Reader::new(&full[..cut]);
            let _ = PlayRequest::decode(&mut r);
        }
    }

    /// A count field is a number some other binary wrote. Reserving on it
    /// directly would let a corrupt payload ask for gigabytes.
    #[test]
    fn an_absurd_count_does_not_reserve_on_trust() {
        let mut w = Writer::new();
        w.count(usize::MAX);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert!(read_buses(&mut r).is_err());
    }
}
