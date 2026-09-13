//! The engine side of the audio boundary.
//!
//! [`AudioLink`] is the one place that holds the backend. Everything else in
//! this crate (the mixer state, the emitters, the timeline) talks to it in
//! ordinary Rust.
//! The backend is linked into the binary (`renzora_audio_backend` is the one
//! that ships) and registers itself with one call; a platform with a different
//! mixer implements the same trait.
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
//! Every method answers sensibly with nothing registered: calls are dropped,
//! reads come back empty. That is what makes the backend *separable*: a build
//! with `renzora_runtime`'s `audio` feature off, or a platform with no backend
//! written for it yet, runs silent with the mixer panel still showing a board
//! and every `play_sound` still resolving. The alternative, unwrapping a
//! backend that may not exist, would make audio mandatory in a build system
//! whose entire point is that it is not.

use std::panic::AssertUnwindSafe;

use bevy::prelude::*;

use crate::audio_backend::{
    Backend, BackendInfo, BusState, Caps, CaptureInfo, ClipInfo, DeviceList, PlayRequest,
    StopRequest, UpdateReply, UpdateRequest,
};

/// The adopted audio backend, or nothing.
#[derive(Resource, Default)]
pub struct AudioLink {
    backend: Option<std::sync::Mutex<Box<dyn Backend>>>,
    /// What the backend said it can do. `None` until [`Self::init`] succeeds.
    info: Option<BackendInfo>,
    next_sound: u64,
    next_voice: u64,
    next_capture: u64,
    /// Set once a call panics. Stops the engine calling into a backend that has
    /// already proven it will take the frame down.
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
    ///
    /// Cloned rather than borrowed: reaching through the `Mutex` without `&mut`
    /// means locking, and a guard cannot outlive this call to hand back a `&str`.
    pub fn name(&self) -> Option<String> {
        self.backend.as_ref().map(|m| {
            m.lock()
                .unwrap_or_else(|p| p.into_inner())
                .name()
                .to_string()
        })
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

    /// Adopt a registered backend.
    pub fn adopt(&mut self, backend: std::sync::Mutex<Box<dyn Backend>>) {
        self.backend = Some(backend);
        self.info = None;
        self.poisoned = false;
    }

    /// Forget the backend.
    ///
    /// Nothing calls this in a normal build any more — a linked-in mixer cannot
    /// be unloaded — but it stays as the way to put the link back to "no audio"
    /// for a test, and for a future backend that genuinely can be swapped.
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

    /// Call the backend, catching a panic rather than letting it reach the
    /// schedule.
    ///
    /// A backend that panics is poisoned rather than retried: it has already
    /// shown it will take the frame down, and calling it sixty times a second is
    /// how one bad clip becomes an unusable editor.
    ///
    /// `None` means there is no backend, or it has been poisoned — the two cases
    /// every caller already handled, because an op a backend did not implement
    /// used to answer the same way.
    fn with<T>(&mut self, what: &str, f: impl FnOnce(&mut dyn Backend) -> T) -> Option<T> {
        if self.poisoned {
            return None;
        }
        // `get_mut` rather than `lock`: we hold `&mut self`, so there is no
        // contention to resolve and no lock is taken.
        let backend = self
            .backend
            .as_mut()?
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut();
        match std::panic::catch_unwind(AssertUnwindSafe(|| f(backend))) {
            Ok(v) => Some(v),
            Err(_) => {
                error!("[audio] backend panicked in {what} and has been disabled");
                self.poisoned = true;
                self.info = None;
                None
            }
        }
    }

    /// Open the device. Must be called before anything else does anything.
    pub fn init(&mut self) -> Result<Option<BackendInfo>, String> {
        let Some(result) = self.with("init", |b| b.init()) else {
            return Ok(None);
        };
        let info = result?;
        self.info = Some(info.clone());
        Ok(Some(info))
    }

    /// Release the device.
    pub fn shutdown(&mut self) {
        self.with("shutdown", |b| b.shutdown());
        self.info = None;
    }

    /// Send the whole bus graph.
    pub fn set_buses(&mut self, buses: &[BusState]) -> Result<(), String> {
        self.with("set_buses", |b| b.set_buses(buses));
        Ok(())
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
        match self.with("load_clip", |b| b.load_clip(sound.0, extension, bytes)) {
            Some(result) => result.map(Some),
            None => Ok(None),
        }
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
        self.with("unload_clip", |b| b.unload_clip(sound.0));
    }

    /// Start a voice.
    pub fn play(&mut self, request: &PlayRequest) -> Result<(), String> {
        match self.with("play", |b| b.play(request)) {
            Some(result) => result,
            None => Ok(()),
        }
    }

    /// Stop a voice, a bus's voices, or everything.
    pub fn stop(&mut self, request: &StopRequest) {
        self.with("stop", |b| b.stop(request));
    }

    /// The per-frame call. Returns the meters and the voices that finished.
    pub fn update(&mut self, request: &UpdateRequest) -> Result<UpdateReply, String> {
        Ok(self
            .with("update", |b| b.update(request))
            .unwrap_or_default())
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
        match self.with("open_capture", |b| b.open_capture(capture.0, device)) {
            Some(result) => result.map(Some),
            None => Ok(None),
        }
    }

    pub fn close_capture(&mut self, capture: CaptureId) {
        self.with("close_capture", |b| b.close_capture(capture.0));
    }

    /// Take everything captured since the last call, as interleaved stereo.
    pub fn read_capture(&mut self, capture: CaptureId) -> Vec<f32> {
        self.with("read_capture", |b| b.read_capture(capture.0))
            .unwrap_or_default()
    }

    /// Mix samples into a bus. The generic "audio from somewhere that isn't a
    /// file" path.
    pub fn push_frames(&mut self, bus: &str, samples: &[f32]) {
        if !self.supports(Caps::FEEDS) {
            return;
        }
        self.with("push_frames", |b| b.push_frames(bus, samples));
    }

    /// Enumerate devices for the mixer's menus.
    pub fn list_devices(&mut self) -> DeviceList {
        if !self.supports(Caps::DEVICE_LIST) {
            return DeviceList::default();
        }
        self.with("list_devices", |b| b.list_devices())
            .unwrap_or_default()
    }
}

fn alloc_pair(voice: VoiceId, paused: bool) -> Vec<(u64, bool)> {
    vec![(voice.0, paused)]
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// How the fake should answer.
    #[derive(Clone, Copy, PartialEq)]
    enum Answer {
        Ok,
        /// Return an `Err` from every fallible call.
        Error,
        /// Panic, so the link's `catch_unwind` has something to catch.
        Panic,
    }

    /// A backend that answers however the test wants — enough to exercise the
    /// link without a sound card.
    ///
    /// Each test builds its own, rather than sharing one through a `static`. An
    /// earlier version kept the desired answer in a global, and since cargo runs
    /// tests in parallel one test would set "panic" while another was mid-`init`:
    /// it passed or failed depending on scheduling.
    struct Fake {
        answer: Answer,
        /// Set when `list_devices` is reached, so a test can assert that an
        /// unclaimed capability never gets that far.
        listed: Arc<AtomicBool>,
    }

    impl Backend for Fake {
        fn name(&self) -> &str {
            "fake"
        }

        fn init(&mut self) -> Result<BackendInfo, String> {
            match self.answer {
                Answer::Panic => panic!("something broke"),
                Answer::Error => Err("something broke".to_string()),
                Answer::Ok => Ok(BackendInfo {
                    sample_rate: 48_000,
                    caps: Caps::CAPTURE.union(Caps::FEEDS),
                    device: String::from("fake"),
                }),
            }
        }

        fn load_clip(&mut self, _clip: u64, _ext: &str, _bytes: &[u8]) -> Result<ClipInfo, String> {
            Ok(ClipInfo {
                duration: 2.0,
                sample_rate: 44_100,
            })
        }

        fn play(&mut self, _request: &PlayRequest) -> Result<(), String> {
            Ok(())
        }

        fn update(&mut self, _request: &UpdateRequest) -> UpdateReply {
            UpdateReply {
                peaks: vec![0.5],
                finished: vec![3],
            }
        }

        fn list_devices(&mut self) -> DeviceList {
            self.listed.store(true, Ordering::Relaxed);
            DeviceList::default()
        }
    }

    /// A link wired to a fresh fake, and the flag its `list_devices` sets.
    fn fake_link(answer: Answer) -> (AudioLink, Arc<AtomicBool>) {
        let listed = Arc::new(AtomicBool::new(false));
        let mut link = AudioLink::default();
        link.adopt(Mutex::new(Box::new(Fake {
            answer,
            listed: Arc::clone(&listed),
        })));
        (link, listed)
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
        let (mut link, _) = fake_link(Answer::Ok);
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
        let (mut link, listed) = fake_link(Answer::Ok);
        link.init().unwrap();

        assert_eq!(link.list_devices(), DeviceList::default());
        assert!(
            !listed.load(Ordering::Relaxed),
            "DEVICE_LIST was never claimed, so the backend must not be called"
        );
    }

    #[test]
    fn an_update_returns_what_the_backend_reported() {
        let (mut link, _) = fake_link(Answer::Ok);
        link.init().unwrap();
        let reply = link.update(&UpdateRequest::default()).unwrap();
        assert_eq!(reply.peaks, vec![0.5]);
        assert_eq!(reply.finished, vec![3]);
    }

    #[test]
    fn an_error_reply_reaches_the_caller_as_a_message() {
        let (mut link, _) = fake_link(Answer::Error);
        let err = link.init().unwrap_err();
        assert!(err.contains("something broke"), "{err}");
        // An error is not fatal — the backend is still there to try again.
        assert!(link.is_active());
    }

    /// A backend that panicked has shown it will take the frame down. Calling it
    /// sixty times a second is how one bad clip becomes an unusable editor.
    #[test]
    fn a_panicking_backend_is_disabled_rather_than_retried() {
        let (mut link, _) = fake_link(Answer::Panic);
        // Caught, so `init` reports "no backend answered" rather than unwinding
        // into the caller — and the link is poisoned on the way out.
        assert_eq!(link.init().unwrap(), None);
        assert!(!link.is_active());
        // And every later call is a silent no-op rather than another panic.
        assert!(link.play(&play_request()).is_ok());
        assert_eq!(link.update(&UpdateRequest::default()).unwrap(), UpdateReply::default());
    }

    #[test]
    fn releasing_a_backend_leaves_the_link_inert() {
        let (mut link, _) = fake_link(Answer::Ok);
        link.init().unwrap();
        assert!(link.is_active());
        link.release();
        assert!(!link.is_active());
        assert_eq!(link.init().unwrap(), None);
    }
}
