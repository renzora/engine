//! The engine side of the audio boundary.
//!
//! [`AudioLink`] is the one place that knows a backend is a `dlopen`'d plugin.
//! Everything else in this crate — the mixer state, the emitters, the timeline —
//! talks to it in ordinary Rust and never sees a function pointer.
//!
//! ## What stays on this side
//!
//! **File I/O**, deliberately, exactly as [`renzora_scripting`] keeps it for
//! scripts. The backend is handed decoded-ready *bytes* and an extension hint;
//! it never opens a path. Exported and Android builds read assets out of an rpak
//! archive, and a backend doing its own `std::fs` would work in the editor and
//! fail in every shipped game.
//!
//! **Handle allocation**, so the engine can name a clip in a `Play` before the
//! backend has finished decoding it, and name a voice before the audio thread
//! has seen the request.
//!
//! ## No backend is a normal state
//!
//! Every method answers sensibly with nothing loaded: calls are dropped, reads
//! come back empty. That is what makes the audio plugin *removable* — delete the
//! file and the same binary runs silent, with the mixer panel still showing a
//! board and every `play_sound` still resolving. The alternative, unwrapping a
//! backend that may not exist, would make audio mandatory in a build system
//! whose entire point is that it is not.

use std::ffi::c_void;

use bevy::prelude::*;

use renzora_plugin::audio::{
    write_buses, write_samples, BackendInfo, BusState, Caps, CaptureInfo, ClipInfo, DeviceList,
    LoadClip, OpenCapture, PlayRequest, StopRequest, UpdateReply, UpdateRequest,
};
use renzora_plugin::sys::{self, AudioOp, AudioStatus};
use renzora_plugin::wire::{Reader, Writer};

/// A backend that has registered and been adopted.
struct Loaded {
    name: String,
    /// The plugin's opaque state, held as a `usize` so this resource stays
    /// `Send + Sync` without an unsafe impl. The engine never dereferences it —
    /// it is handed straight back on every call, so the only requirement is that
    /// it round-trips unchanged.
    state: usize,
    entry: sys::AudioEntry,
}

/// The loaded audio backend, or nothing.
#[derive(Resource, Default)]
pub struct AudioLink {
    backend: Option<Loaded>,
    /// What the backend said it can do. `None` until [`Self::init`] succeeds.
    info: Option<BackendInfo>,
    next_sound: u64,
    next_voice: u64,
    next_capture: u64,
    /// Set once when a call fails in a way that means the backend is gone —
    /// a panic it could not recover from. Stops the engine calling into a
    /// backend that has already proven it will abort.
    poisoned: bool,
}

/// A handle to a sound the backend has decoded.
///
/// Deliberately not `ClipId` — the timeline already has one of those, and it
/// means something else entirely: a region placed on a track. This names a
/// decoded buffer living in the backend. The two get confused the moment they
/// share a name, and they appear within a few lines of each other in the
/// scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoundId(pub u64);

/// A handle to a playing voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VoiceId(pub u64);

/// A handle to an open capture stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CaptureId(pub u64);

impl AudioLink {
    /// Is a backend loaded and working?
    pub fn is_active(&self) -> bool {
        self.backend.is_some() && !self.poisoned
    }

    /// The backend's name, for logs and the editor.
    pub fn name(&self) -> Option<&str> {
        self.backend.as_ref().map(|b| b.name.as_str())
    }

    /// What the backend reported at init.
    pub fn info(&self) -> Option<&BackendInfo> {
        self.info.as_ref()
    }

    /// Does the backend claim this capability?
    ///
    /// Asked rather than assumed, because backends genuinely differ — a browser
    /// build cannot capture. A caller that skips this gets a game that silently
    /// does nothing rather than one that reports a missing feature.
    pub fn supports(&self, caps: Caps) -> bool {
        self.info.as_ref().is_some_and(|i| i.caps.contains(caps))
    }

    /// Adopt a backend the plugin host registered.
    pub fn adopt(&mut self, name: String, state: usize, entry: sys::AudioEntry) {
        self.backend = Some(Loaded { name, state, entry });
        self.info = None;
        self.poisoned = false;
    }

    /// Forget the backend. Called when its plugin is unloaded — both `entry` and
    /// `state` point into an image about to be unmapped.
    pub fn release(&mut self) {
        self.backend = None;
        self.info = None;
    }

    /// Allocate the next sound handle.
    pub fn next_sound(&mut self) -> SoundId {
        self.next_sound += 1;
        SoundId(self.next_sound)
    }

    /// Allocate the next voice handle.
    pub fn next_voice(&mut self) -> VoiceId {
        self.next_voice += 1;
        VoiceId(self.next_voice)
    }

    /// Allocate the next capture handle.
    pub fn next_capture(&mut self) -> CaptureId {
        self.next_capture += 1;
        CaptureId(self.next_capture)
    }

    /// Make one call. `Ok(None)` means the backend does not implement this op.
    ///
    /// Returns the reply bytes, which most callers then decode. A backend that
    /// panics is poisoned rather than retried: it has already shown it will take
    /// the frame down, and calling it sixty times a second is how one bad clip
    /// becomes an unusable editor.
    fn call(&mut self, op: AudioOp, payload: &[u8], blob: &[u8]) -> Result<Option<Vec<u8>>, String> {
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
        let call = sys::AudioCall {
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
                "audio backend `{name}` returned status {} — it was built against a newer engine",
                status.0
            ));
        }
        match status {
            AudioStatus::Ok => Ok(Some(out)),
            // Not an error, and the host must not log it: it is how a backend
            // says "I was built before this op existed" or "I never claimed this
            // capability", which are both ordinary.
            AudioStatus::UnknownOp => Ok(None),
            AudioStatus::Error => Err(decode_error(&out)),
            AudioStatus::Panicked => {
                self.poisoned = true;
                Err(format!(
                    "audio backend `{}` panicked and has been disabled: {}",
                    backend.name,
                    decode_error(&out)
                ))
            }
            _ => Ok(Some(out)),
        }
    }

    /// Open the device. Must be called before anything else does anything.
    pub fn init(&mut self) -> Result<Option<BackendInfo>, String> {
        let Some(bytes) = self.call(AudioOp::Init, &[], &[])? else {
            return Ok(None);
        };
        let info = BackendInfo::decode(&mut Reader::new(&bytes))
            .map_err(|e| format!("backend's init reply would not decode: {e}"))?;
        self.info = Some(info.clone());
        Ok(Some(info))
    }

    /// Release the device.
    pub fn shutdown(&mut self) {
        let _ = self.call(AudioOp::Shutdown, &[], &[]);
        self.info = None;
    }

    /// Send the whole bus graph.
    pub fn set_buses(&mut self, buses: &[BusState]) -> Result<(), String> {
        let mut w = Writer::new();
        write_buses(&mut w, buses);
        self.call(AudioOp::SetBuses, w.bytes(), &[]).map(|_| ())
    }

    /// Hand over an encoded audio file to decode.
    ///
    /// `bytes` is the whole file, read by the *engine* — see the module doc.
    pub fn load_clip(
        &mut self,
        sound: SoundId,
        extension: &str,
        bytes: &[u8],
    ) -> Result<Option<ClipInfo>, String> {
        let mut w = Writer::new();
        LoadClip {
            clip: sound.0,
            extension: extension.to_string(),
        }
        .encode(&mut w);
        let payload = w.into_bytes();
        let Some(reply) = self.call(AudioOp::LoadClip, &payload, bytes)? else {
            return Ok(None);
        };
        ClipInfo::decode(&mut Reader::new(&reply))
            .map(Some)
            .map_err(|e| format!("backend's clip reply would not decode: {e}"))
    }

    /// Decode bytes that did not come from an asset path.
    ///
    /// For audio the engine has in hand rather than on disk — a marketplace
    /// preview downloaded over HTTP, say. `SoundCache` is the right door for
    /// anything with a path, because it deduplicates; this one is for bytes that
    /// have no stable name to deduplicate by.
    pub fn load_bytes(&mut self, extension: &str, bytes: &[u8]) -> Option<SoundId> {
        let sound = self.next_sound();
        match self.load_clip(sound, extension, bytes) {
            Ok(Some(_)) => Some(sound),
            Ok(None) => None,
            Err(e) => {
                warn!("[audio] {e}");
                None
            }
        }
    }

    /// Hold a voice silent, or let it carry on.
    ///
    /// A convenience over [`Self::update`] for the one-off case. Systems that
    /// change several voices a frame should batch them into a single update
    /// instead — the boundary crossing is the expensive part.
    pub fn set_paused(&mut self, voice: VoiceId, paused: bool) {
        let request = UpdateRequest {
            paused: alloc_pair(voice, paused),
            ..Default::default()
        };
        if let Err(e) = self.update(&request) {
            warn!("[audio] {e}");
        }
    }

    /// Drop a decoded clip. Voices already playing it finish rather than cut.
    pub fn unload_clip(&mut self, sound: SoundId) {
        let mut w = Writer::new();
        w.u64(sound.0);
        let payload = w.into_bytes();
        let _ = self.call(AudioOp::UnloadClip, &payload, &[]);
    }

    /// Start a voice.
    pub fn play(&mut self, request: &PlayRequest) -> Result<(), String> {
        let mut w = Writer::new();
        request.encode(&mut w);
        self.call(AudioOp::Play, w.bytes(), &[]).map(|_| ())
    }

    /// Stop a voice, a bus's voices, or everything.
    pub fn stop(&mut self, request: &StopRequest) {
        let mut w = Writer::new();
        request.encode(&mut w);
        let _ = self.call(AudioOp::Stop, w.bytes(), &[]);
    }

    /// The per-frame call. Returns the meters and the voices that finished.
    pub fn update(&mut self, request: &UpdateRequest) -> Result<UpdateReply, String> {
        let mut w = Writer::new();
        request.encode(&mut w);
        let Some(reply) = self.call(AudioOp::Update, w.bytes(), &[])? else {
            return Ok(UpdateReply::default());
        };
        UpdateReply::decode(&mut Reader::new(&reply))
            .map_err(|e| format!("backend's update reply would not decode: {e}"))
    }

    /// Open a capture device.
    pub fn open_capture(
        &mut self,
        capture: CaptureId,
        device: Option<&str>,
    ) -> Result<Option<CaptureInfo>, String> {
        if !self.supports(Caps::CAPTURE) {
            return Ok(None);
        }
        let mut w = Writer::new();
        OpenCapture {
            capture: capture.0,
            device: device.map(str::to_string),
        }
        .encode(&mut w);
        let payload = w.into_bytes();
        let Some(reply) = self.call(AudioOp::OpenCapture, &payload, &[])? else {
            return Ok(None);
        };
        CaptureInfo::decode(&mut Reader::new(&reply))
            .map(Some)
            .map_err(|e| format!("backend's capture reply would not decode: {e}"))
    }

    pub fn close_capture(&mut self, capture: CaptureId) {
        let mut w = Writer::new();
        w.u64(capture.0);
        let payload = w.into_bytes();
        let _ = self.call(AudioOp::CloseCapture, &payload, &[]);
    }

    /// Take everything captured since the last call, as interleaved stereo.
    pub fn read_capture(&mut self, capture: CaptureId) -> Vec<f32> {
        let mut w = Writer::new();
        w.u64(capture.0);
        let payload = w.into_bytes();
        match self.call(AudioOp::ReadCapture, &payload, &[]) {
            Ok(Some(reply)) => renzora_plugin::audio::read_samples(&mut Reader::new(&reply))
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// Mix samples into a bus. The generic "audio from somewhere that isn't a
    /// file" path — see [`renzora_plugin::audio`].
    pub fn push_frames(&mut self, bus: &str, samples: &[f32]) {
        if !self.supports(Caps::FEEDS) {
            return;
        }
        let mut w = Writer::new();
        w.str(bus);
        let payload = w.into_bytes();
        // The samples ride in the blob rather than the payload: a block of audio
        // is thousands of floats, and copying them through the codec alongside a
        // bus key would be the expensive half of the call.
        let mut b = Writer::new();
        write_samples(&mut b, samples);
        let blob = b.into_bytes();
        let _ = self.call(AudioOp::PushFrames, &payload, &blob);
    }

    /// Enumerate devices for the mixer's menus.
    pub fn list_devices(&mut self) -> DeviceList {
        if !self.supports(Caps::DEVICE_LIST) {
            return DeviceList::default();
        }
        match self.call(AudioOp::ListDevices, &[], &[]) {
            Ok(Some(reply)) => {
                DeviceList::decode(&mut Reader::new(&reply)).unwrap_or_default()
            }
            _ => DeviceList::default(),
        }
    }
}

fn alloc_pair(voice: VoiceId, paused: bool) -> Vec<(u64, bool)> {
    vec![(voice.0, paused)]
}

/// Read an error reply, or say so when even that would not decode.
fn decode_error(bytes: &[u8]) -> String {
    Reader::new(bytes)
        .string()
        .unwrap_or_else(|_| String::from("(the backend's error message would not decode)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend that answers however the test wants — enough to exercise the
    /// link without a sound card.
    ///
    /// Its state lives behind the call's own `state` pointer rather than in a
    /// `static`, and that is not incidental: an earlier version kept the desired
    /// status in a global, and since cargo runs tests in parallel one test would
    /// set `Panicked` while another was mid-`init`. It passed or failed
    /// depending on scheduling. Per-instance state is also what the `state`
    /// field on the descriptor exists for, so this is the boundary being used as
    /// intended rather than a test-only trick.
    mod fake {
        use super::*;

        pub struct State {
            pub status: AudioStatus,
            pub last_op: u32,
        }

        impl State {
            pub fn new(status: AudioStatus) -> Box<Self> {
                Box::new(Self {
                    status,
                    last_op: u32::MAX,
                })
            }
        }

        pub unsafe extern "C" fn entry(call: *const sys::AudioCall) -> AudioStatus {
            let call = &*call;
            let state = &mut *(call.state as *mut State);
            state.last_op = call.op.0;
            let status = state.status;

            let mut w = Writer::new();
            match (status, call.op) {
                (AudioStatus::Ok, AudioOp::Init) => BackendInfo {
                    sample_rate: 48_000,
                    caps: Caps::CAPTURE.union(Caps::FEEDS),
                    device: String::from("fake"),
                }
                .encode(&mut w),
                (AudioStatus::Ok, AudioOp::Update) => UpdateReply {
                    peaks: vec![0.5],
                    finished: vec![3],
                }
                .encode(&mut w),
                (AudioStatus::Ok, AudioOp::LoadClip) => ClipInfo {
                    duration: 2.0,
                    sample_rate: 44_100,
                }
                .encode(&mut w),
                (AudioStatus::Error, _) | (AudioStatus::Panicked, _) => w.str("something broke"),
                _ => {}
            }
            if let Some(sink) = call.out.as_ref() {
                let bytes = w.bytes();
                (sink.write)(sink.ctx, bytes.as_ptr(), bytes.len());
            }
            status
        }

        /// A link wired to a fresh fake. The returned `State` must outlive the
        /// link — the link holds its address.
        pub fn link(status: AudioStatus) -> (AudioLink, Box<State>) {
            let mut state = State::new(status);
            let mut link = AudioLink::default();
            link.adopt(String::from("fake"), state.as_mut() as *mut State as usize, entry);
            (link, state)
        }
    }

    /// The property that makes the plugin removable: with nothing loaded,
    /// everything is a quiet no-op rather than a panic.
    #[test]
    fn a_link_with_no_backend_answers_everything_harmlessly() {
        let mut link = AudioLink::default();
        assert!(!link.is_active());
        assert_eq!(link.init().unwrap(), None);
        assert!(link.set_buses(&[]).is_ok());
        assert_eq!(link.load_clip(SoundId(1), "wav", &[1, 2, 3]).unwrap(), None);
        assert!(link.play(&play_request()).is_ok());
        assert_eq!(link.update(&UpdateRequest::default()).unwrap(), UpdateReply::default());
        assert!(link.read_capture(CaptureId(1)).is_empty());
        assert_eq!(link.list_devices(), DeviceList::default());
        link.push_frames("Sfx", &[0.0; 4]);
        link.unload_clip(SoundId(1));
        link.shutdown();
    }

    fn play_request() -> PlayRequest {
        PlayRequest {
            voice: 1,
            clip: 1,
            bus: String::from("Sfx"),
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
    }

    #[test]
    fn handles_are_unique_and_start_at_one() {
        let mut link = AudioLink::default();
        assert_eq!(link.next_sound(), SoundId(1));
        assert_eq!(link.next_sound(), SoundId(2));
        assert_eq!(link.next_voice(), VoiceId(1));
        assert_eq!(link.next_capture(), CaptureId(1));
    }

    #[test]
    fn init_records_what_the_backend_reported() {
        let (mut link, _state) = fake::link(AudioStatus::Ok);
        let info = link.init().unwrap().expect("should report");
        assert_eq!(info.sample_rate, 48_000);
        assert!(link.supports(Caps::CAPTURE));
        assert!(link.supports(Caps::FEEDS));
        assert!(!link.supports(Caps::SPATIAL));
    }

    /// Capability gating is the point of `Caps` — an op the backend never
    /// claimed must not even reach it.
    #[test]
    fn an_unclaimed_capability_is_not_called() {
        let (mut link, mut state) = fake::link(AudioStatus::Ok);
        link.init().unwrap();

        state.last_op = u32::MAX;
        assert_eq!(link.list_devices(), DeviceList::default());
        assert_eq!(
            state.last_op,
            u32::MAX,
            "DEVICE_LIST was never claimed, so the backend must not be called"
        );
    }

    #[test]
    fn an_update_decodes_its_reply() {
        let (mut link, _state) = fake::link(AudioStatus::Ok);
        link.init().unwrap();
        let reply = link.update(&UpdateRequest::default()).unwrap();
        assert_eq!(reply.peaks, vec![0.5]);
        assert_eq!(reply.finished, vec![3]);
    }

    #[test]
    fn an_error_reply_reaches_the_caller_as_a_message() {
        let (mut link, _state) = fake::link(AudioStatus::Error);
        let err = link.init().unwrap_err();
        assert!(err.contains("something broke"), "{err}");
        // An error is not fatal — the backend is still there to try again.
        assert!(link.is_active());
    }

    /// A backend that panicked has shown it will take the frame down. Calling it
    /// sixty times a second is how one bad clip becomes an unusable editor.
    #[test]
    fn a_panicking_backend_is_disabled_rather_than_retried() {
        let (mut link, _state) = fake::link(AudioStatus::Panicked);
        let err = link.init().unwrap_err();
        assert!(err.contains("panicked"), "{err}");
        assert!(!link.is_active());
        // And every later call is a silent no-op rather than another panic.
        assert!(link.play(&play_request()).is_ok());
    }

    /// A status this build has no variant for means the plugin is newer than the
    /// engine — the one case where continuing is guessing.
    #[test]
    fn an_unknown_status_disables_the_backend() {
        let (mut link, _state) = fake::link(AudioStatus(99));
        let err = link.init().unwrap_err();
        assert!(err.contains("newer engine"), "{err}");
        assert!(!link.is_active());
    }

    #[test]
    fn releasing_a_backend_leaves_the_link_inert() {
        let (mut link, _state) = fake::link(AudioStatus::Ok);
        link.init().unwrap();
        assert!(link.is_active());
        link.release();
        assert!(!link.is_active());
        assert_eq!(link.init().unwrap(), None);
    }
}
