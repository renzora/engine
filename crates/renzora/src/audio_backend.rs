//! The contract between the engine's audio API and whatever actually opens a
//! device.
//!
//! `renzora_audio` ships the audio *API* — the bus graph, the components, the
//! command queue, the timeline — and no audio. A mixer implements [`Backend`]
//! and hands itself to [`AppAudioBackendExt::add_audio_backend`].
//!
//! Here rather than in `renzora_audio` because the engine's audio link holds
//! these types, and a boundary type has exactly one definition — the rule this
//! crate exists for.

use bevy::prelude::*;

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
    /// [`Backend::push_frames`] is honoured.
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

/// What a backend answers [`Backend::init`] with.
#[derive(Debug, Clone, PartialEq)]
pub struct BackendInfo {
    /// The rate the device negotiated. The engine needs it to convert seconds to
    /// frames the same way the backend does.
    pub sample_rate: u32,
    pub caps: Caps,
    /// Name of the output device that was opened, for the editor to display.
    pub device: String,
}

/// One bus, as the engine describes it.
///
/// The whole board is sent on every change rather than diffed: it is a few dozen
/// entries, and a diff protocol would be a second source of truth about what the
/// mixer looks like — one that can get out of step with the first and produce a
/// board nobody authored.
#[derive(Debug, Clone, PartialEq)]
pub struct BusState {
    /// The permanent routing key, never the display name. See `AudioConfig` for
    /// why those are different things.
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
    /// `0` logarithmic, `1` linear. A number rather than an enum because it used
    /// to cross an ABI; an unknown value means logarithmic.
    pub rolloff: u32,
}

/// Where the ears are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListenerState {
    pub position: [f32; 3],
    /// Unit vector out of the listener's right ear.
    pub right: [f32; 3],
}

/// Reply to [`Backend::load_clip`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipInfo {
    pub duration: f64,
    pub sample_rate: u32,
}

/// Request for [`Backend::play`].
#[derive(Debug, Clone, PartialEq)]
pub struct PlayRequest {
    /// Engine-assigned voice handle, allocated up front so a voice can be named
    /// in a later call before the backend has finished starting it.
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

/// What a [`Backend::stop`] targets.
#[derive(Debug, Clone, PartialEq)]
pub enum StopTarget {
    Voice(u64),
    /// Every voice on a bus, by routing key.
    Bus(String),
    All,
}

/// Request for [`Backend::stop`].
#[derive(Debug, Clone, PartialEq)]
pub struct StopRequest {
    pub target: StopTarget,
    /// Fade-out in seconds. Zero still ramps over a block — cutting a waveform
    /// mid-cycle is a step, and a step is broadband noise.
    pub fade: f32,
}

/// Everything that changes per frame, in one call.
///
/// Batched rather than one call per moved emitter because a scene with two
/// hundred positioned sounds would otherwise make two hundred calls a frame to
/// move them a few centimetres.
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
    pub paused: Vec<(u64, bool)>,
}

/// Reply to [`Backend::update`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateReply {
    /// Peak level per bus, in the order the engine sent them in
    /// [`Backend::set_buses`].
    pub peaks: Vec<f32>,
    /// Voices that have finished since the last update, so the engine can drop
    /// its bookkeeping. Without this it would have to poll every handle it ever
    /// created.
    pub finished: Vec<u64>,
}

/// Reply to [`Backend::open_capture`].
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureInfo {
    /// The capture device's own rate, which need not match the output's — a
    /// caller monitoring through a bus has to resample, one recording to disk
    /// must not.
    pub sample_rate: u32,
    pub device: String,
}

/// Reply to [`Backend::list_devices`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceList {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// A mixer the engine can drive.
///
/// `Send` but deliberately **not** `Sync`: a real mixer owns the producer end of
/// a lock-free queue into its audio thread, and those are not `Sync` by nature.
/// The engine holds it behind a `Mutex` for that reason — see [`AudioBackend`].
pub trait Backend: Send + 'static {
    /// Human-readable, for logs and the editor's audio settings.
    fn name(&self) -> &str;

    /// Open the device and start mixing.
    ///
    /// The returned [`Caps`] is a promise the engine relies on: it will not ask
    /// for capture from a backend that did not claim [`Caps::CAPTURE`], and it
    /// *will* assume positional audio is applied if [`Caps::SPATIAL`] is set.
    fn init(&mut self) -> Result<BackendInfo, String>;

    /// Close the device and release everything.
    fn shutdown(&mut self) {}

    /// Replace the whole bus board. Sent on every change — see [`BusState`].
    fn set_buses(&mut self, buses: &[BusState]) {
        let _ = buses;
    }

    /// Decode a clip and keep it under `clip`.
    ///
    /// `extension` is a decoding hint only; the bytes are authoritative.
    fn load_clip(&mut self, clip: u64, extension: &str, bytes: &[u8]) -> Result<ClipInfo, String>;

    /// Drop a decoded clip.
    fn unload_clip(&mut self, clip: u64) {
        let _ = clip;
    }

    /// Start a voice.
    fn play(&mut self, request: &PlayRequest) -> Result<(), String>;

    /// Stop one voice, a bus, or everything.
    fn stop(&mut self, request: &StopRequest) {
        let _ = request;
    }

    /// Apply a frame's worth of changes and report what happened.
    fn update(&mut self, request: &UpdateRequest) -> UpdateReply;

    /// Open a capture device. Only called when [`Caps::CAPTURE`] was claimed.
    fn open_capture(&mut self, capture: u64, device: Option<&str>) -> Result<CaptureInfo, String> {
        let _ = (capture, device);
        Err("this backend cannot capture".to_string())
    }

    /// Close a capture device.
    fn close_capture(&mut self, capture: u64) {
        let _ = capture;
    }

    /// Take whatever the capture device has produced since the last call.
    fn read_capture(&mut self, capture: u64) -> Vec<f32> {
        let _ = capture;
        Vec::new()
    }

    /// Push interleaved samples onto a bus. Only called when [`Caps::FEEDS`] was
    /// claimed.
    fn push_frames(&mut self, bus: &str, samples: &[f32]) {
        let _ = (bus, samples);
    }

    /// Enumerate devices. Only called when [`Caps::DEVICE_LIST`] was claimed.
    fn list_devices(&mut self) -> DeviceList {
        DeviceList::default()
    }
}

/// The registered mixer, if one has been installed.
///
/// One, not several: two mixers would each hold a device and the game would be
/// heard twice, slightly out of phase.
///
/// The `Mutex` is what makes this a `Resource` at all — [`Backend`] is `Send`
/// but not `Sync`, and a Bevy resource must be both. It costs nothing: every
/// caller has `&mut` and reaches the mixer through `Mutex::get_mut`, which does
/// not lock.
#[derive(Resource, Default)]
pub struct AudioBackend(pub Option<std::sync::Mutex<Box<dyn Backend>>>);

/// Install an audio backend.
pub trait AppAudioBackendExt {
    /// Register `backend` as the engine's mixer.
    ///
    /// First claim wins, and a second is refused with an error rather than
    /// replacing the first.
    fn add_audio_backend(&mut self, backend: impl Backend) -> &mut Self;
}

impl AppAudioBackendExt for App {
    fn add_audio_backend(&mut self, backend: impl Backend) -> &mut Self {
        let name = backend.name().to_string();
        let mut slot = self
            .world_mut()
            .get_resource_or_insert_with(AudioBackend::default);
        match slot.0.as_mut() {
            Some(existing) => {
                let held = existing
                    .get_mut()
                    .unwrap_or_else(|p| p.into_inner())
                    .name()
                    .to_string();
                error!("audio backend `{name}` is ignored — `{held}` is already registered");
            }
            None => {
                info!("[audio] backend `{name}` registered");
                slot.0 = Some(std::sync::Mutex::new(Box::new(backend)));
            }
        }
        self
    }
}
