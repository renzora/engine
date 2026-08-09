//! The side of the audio boundary a backend plugin writes.
//!
//! [`Backend`] is one method per [`AudioOp`], with defaults for everything a
//! minimal backend can do without: a mixer that only plays clips implements four
//! methods and leaves capture, feeds and device enumeration alone. What it must
//! then do is tell the truth in [`Caps`] — see [`Backend::init`].
//!
//! Everything below the trait is plumbing an implementor never touches: decoding
//! the request, catching panics, encoding the reply.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::string::String;
use std::vec::Vec;

use super::protocol::{
    read_buses, read_samples, write_samples, BackendInfo, BusState, CaptureInfo, ClipInfo,
    DeviceList, LoadClip, OpenCapture, PlayRequest, StopRequest, UpdateReply, UpdateRequest,
};
use crate::sys::{AudioBackendDesc, AudioCall, AudioEntry, AudioOp, AudioStatus, ByteSink, Str256};
use crate::wire::{Reader, Writer};

/// What an audio backend implements.
///
/// Every method that can fail returns `Result<_, String>` and the message
/// reaches the host's log against the thing that failed — an asset path, a
/// device name. A backend should say what it could not do, not that something
/// went wrong.
pub trait Backend: Default + Send + 'static {
    /// Human-readable, for logs and the editor's audio settings.
    const NAME: &'static str;

    /// Open the device and start mixing.
    ///
    /// The returned [`Caps`](super::Caps) is a promise the host relies on: it
    /// will not ask for capture from a backend that did not claim
    /// [`CAPTURE`](super::Caps::CAPTURE), and it *will* assume positional audio
    /// works if [`SPATIAL`](super::Caps::SPATIAL) is set. Claiming something you
    /// do not do produces a game that is silently wrong rather than one that
    /// reports a missing feature.
    fn init(&mut self) -> Result<BackendInfo, String>;

    /// Stop everything and release the device. Called before unload, and the
    /// last chance to close a stream tidily.
    fn shutdown(&mut self) {}

    /// The whole bus graph, in mixer order.
    ///
    /// Sent on every change rather than diffed, so this must be idempotent —
    /// re-sending an identical board has to leave the mix untouched, not rebuild
    /// it. Buses that vanished from the list are gone; voices on them should
    /// move rather than stop, since a bus disappearing is an authoring action.
    fn set_buses(&mut self, buses: &[BusState]) {
        let _ = buses;
    }

    /// Decode a clip and remember it under `clip`.
    ///
    /// `extension` is a hint from the asset path, not a promise — probe the
    /// bytes and let them win, or a mislabelled file becomes a silent failure
    /// nobody can explain.
    fn load_clip(&mut self, clip: u64, extension: &str, bytes: &[u8]) -> Result<ClipInfo, String>;

    /// Forget a clip. Voices already playing it should finish, not cut.
    fn unload_clip(&mut self, clip: u64) {
        let _ = clip;
    }

    /// Start a voice on a previously loaded clip.
    fn play(&mut self, request: &PlayRequest) -> Result<(), String>;

    /// Stop a voice, a bus's voices, or everything.
    fn stop(&mut self, request: &StopRequest) {
        let _ = request;
    }

    /// The per-frame call: apply what moved, and answer with the meters and the
    /// voices that have finished.
    ///
    /// The `finished` list is what keeps the host from having to poll every
    /// handle it ever created, so a backend that never reports finishes will
    /// leak host-side bookkeeping for the life of the session.
    fn update(&mut self, request: &UpdateRequest) -> UpdateReply;

    /// Open a capture device. Only called when [`Caps::CAPTURE`](super::Caps::CAPTURE)
    /// was claimed.
    fn open_capture(&mut self, capture: u64, device: Option<&str>) -> Result<CaptureInfo, String> {
        let _ = (capture, device);
        Err(String::from("this backend cannot capture"))
    }

    fn close_capture(&mut self, capture: u64) {
        let _ = capture;
    }

    /// Take everything captured since the last call, as interleaved stereo.
    fn read_capture(&mut self, capture: u64) -> Vec<f32> {
        let _ = capture;
        Vec::new()
    }

    /// Mix pushed samples into a bus — mic monitoring, a remote player's voice,
    /// a synth. Only called when [`Caps::FEEDS`](super::Caps::FEEDS) was claimed.
    fn push_frames(&mut self, bus: &str, samples: &[f32]) {
        let _ = (bus, samples);
    }

    /// Enumerate devices for the editor's menus.
    fn list_devices(&mut self) -> DeviceList {
        DeviceList::default()
    }
}

/// Write `bytes` into the host's sink.
///
/// # Safety
/// `sink` must be the one the host passed with this call.
unsafe fn write(sink: *const ByteSink, bytes: &[u8]) {
    if let Some(sink) = sink.as_ref() {
        (sink.write)(sink.ctx, bytes.as_ptr(), bytes.len());
    }
}

/// Encode `value` and write it to the sink.
unsafe fn reply(sink: *const ByteSink, encode: impl FnOnce(&mut Writer)) {
    let mut w = Writer::new();
    encode(&mut w);
    write(sink, w.bytes());
}

/// Write an error message as the reply. Paired with [`AudioStatus::Error`], so
/// the host has something to log beyond "it failed".
unsafe fn reply_error(sink: *const ByteSink, message: &str) {
    reply(sink, |w| w.str(message));
}

/// Route one call to `backend`.
///
/// # Safety
/// `call` must be a live [`AudioCall`] from the host.
pub unsafe fn dispatch<B: Backend>(backend: &mut B, call: *const AudioCall) -> AudioStatus {
    if call.is_null() {
        return AudioStatus::Error;
    }
    let call = &*call;

    // A panic must not unwind out of the `extern "C"` frame the host called us
    // through — that is an abort, and taking a game down because one clip had a
    // bad header is not a proportionate response. Catch here and let the host
    // stop calling this backend.
    match catch_unwind(AssertUnwindSafe(|| run(backend, call))) {
        Ok(status) => status,
        Err(_) => {
            reply_error(call.out, &std::format!("{} panicked", B::NAME));
            AudioStatus::Panicked
        }
    }
}

unsafe fn run<B: Backend>(backend: &mut B, call: &AudioCall) -> AudioStatus {
    let payload = call.payload.as_slice();
    let blob = call.blob.as_slice();

    match call.op {
        AudioOp::Init => match backend.init() {
            Ok(info) => {
                reply(call.out, |w| info.encode(w));
                AudioStatus::Ok
            }
            Err(e) => {
                reply_error(call.out, &e);
                AudioStatus::Error
            }
        },
        AudioOp::Shutdown => {
            backend.shutdown();
            AudioStatus::Ok
        }
        AudioOp::SetBuses => {
            let mut r = Reader::new(payload);
            match read_buses(&mut r) {
                Ok(buses) => {
                    backend.set_buses(&buses);
                    AudioStatus::Ok
                }
                Err(e) => malformed(call.out, "bus graph", e),
            }
        }
        AudioOp::LoadClip => {
            let mut r = Reader::new(payload);
            match LoadClip::decode(&mut r) {
                Ok(request) => match backend.load_clip(request.clip, &request.extension, blob) {
                    Ok(info) => {
                        reply(call.out, |w| info.encode(w));
                        AudioStatus::Ok
                    }
                    Err(e) => {
                        reply_error(call.out, &e);
                        AudioStatus::Error
                    }
                },
                Err(e) => malformed(call.out, "clip request", e),
            }
        }
        AudioOp::UnloadClip => {
            let mut r = Reader::new(payload);
            match r.u64() {
                Ok(clip) => {
                    backend.unload_clip(clip);
                    AudioStatus::Ok
                }
                Err(e) => malformed(call.out, "clip handle", e),
            }
        }
        AudioOp::Play => {
            let mut r = Reader::new(payload);
            match PlayRequest::decode(&mut r) {
                Ok(request) => match backend.play(&request) {
                    Ok(()) => AudioStatus::Ok,
                    Err(e) => {
                        reply_error(call.out, &e);
                        AudioStatus::Error
                    }
                },
                Err(e) => malformed(call.out, "play request", e),
            }
        }
        AudioOp::Stop => {
            let mut r = Reader::new(payload);
            match StopRequest::decode(&mut r) {
                Ok(request) => {
                    backend.stop(&request);
                    AudioStatus::Ok
                }
                Err(e) => malformed(call.out, "stop request", e),
            }
        }
        AudioOp::Update => {
            let mut r = Reader::new(payload);
            match UpdateRequest::decode(&mut r) {
                Ok(request) => {
                    let out = backend.update(&request);
                    reply(call.out, |w| out.encode(w));
                    AudioStatus::Ok
                }
                Err(e) => malformed(call.out, "update", e),
            }
        }
        AudioOp::OpenCapture => {
            let mut r = Reader::new(payload);
            match OpenCapture::decode(&mut r) {
                Ok(request) => {
                    match backend.open_capture(request.capture, request.device.as_deref()) {
                        Ok(info) => {
                            reply(call.out, |w| info.encode(w));
                            AudioStatus::Ok
                        }
                        Err(e) => {
                            reply_error(call.out, &e);
                            AudioStatus::Error
                        }
                    }
                }
                Err(e) => malformed(call.out, "capture request", e),
            }
        }
        AudioOp::CloseCapture => {
            let mut r = Reader::new(payload);
            match r.u64() {
                Ok(capture) => {
                    backend.close_capture(capture);
                    AudioStatus::Ok
                }
                Err(e) => malformed(call.out, "capture handle", e),
            }
        }
        AudioOp::ReadCapture => {
            let mut r = Reader::new(payload);
            match r.u64() {
                Ok(capture) => {
                    let samples = backend.read_capture(capture);
                    reply(call.out, |w| write_samples(w, &samples));
                    AudioStatus::Ok
                }
                Err(e) => malformed(call.out, "capture handle", e),
            }
        }
        AudioOp::PushFrames => {
            let mut r = Reader::new(payload);
            match r.string() {
                Ok(bus) => {
                    // The samples ride in `blob` rather than the payload: a block
                    // of audio is thousands of floats and the host already has
                    // them contiguous, so copying them through the codec to name
                    // a bus alongside would be the expensive half of the call.
                    let mut br = Reader::new(blob);
                    match read_samples(&mut br) {
                        Ok(samples) => {
                            backend.push_frames(&bus, &samples);
                            AudioStatus::Ok
                        }
                        Err(e) => malformed(call.out, "pushed samples", e),
                    }
                }
                Err(e) => malformed(call.out, "bus key", e),
            }
        }
        AudioOp::ListDevices => {
            let devices = backend.list_devices();
            reply(call.out, |w| devices.encode(w));
            AudioStatus::Ok
        }
        // An op this build has never heard of. Not an error: it is what makes
        // appending an op safe for backends built before it existed, and the
        // host treats it exactly as it treats a capability never claimed.
        _ => AudioStatus::UnknownOp,
    }
}

/// Report a payload that would not decode.
///
/// Its own path because it means something specific and alarming: the bytes came
/// from another binary, so a failure here is the two sides disagreeing about the
/// format rather than a bad asset. Naming which payload is the only clue anyone
/// gets.
unsafe fn malformed(
    sink: *const ByteSink,
    what: &str,
    error: crate::wire::WireError,
) -> AudioStatus {
    reply_error(sink, &std::format!("{what} would not decode: {error}"));
    AudioStatus::Error
}

/// Build the descriptor for a backend type.
///
/// `state` is left null by [`audio_backend!`], which parks the backend in a
/// `static` instead. The field stays in the descriptor for a hand-written
/// backend that wants more than one instance — see [`AudioBackendDesc::state`].
pub fn desc_for<B: Backend>(entry: AudioEntry) -> AudioBackendDesc {
    AudioBackendDesc {
        name: Str256::new_truncating(B::NAME),
        state: core::ptr::null_mut(),
        entry,
    }
}

/// Emit the `extern "C"` entry point for a [`Backend`] and the state it needs.
///
/// A macro rather than a generic because the entry point must be a bare function
/// pointer with nowhere to carry state, so it needs a `static` — and a `static`
/// cannot be generic over the backend type.
///
/// ```ignore
/// renzora_plugin::audio_backend!(mixer::MyBackend);
///
/// impl Plugin for MyPlugin {
///     fn build(&self, app: &mut App) {
///         app.add_audio_backend(audio_backend::desc());
///     }
/// }
/// ```
#[macro_export]
macro_rules! audio_backend {
    ($ty:ty) => {
        /// Generated by `renzora_plugin::audio_backend!`.
        pub mod audio_backend {
            #[allow(unused_imports)]
            use super::*;

            type Backend = $ty;

            static STATE: ::std::sync::Mutex<::std::option::Option<Backend>> =
                ::std::sync::Mutex::new(::std::option::Option::None);

            /// # Safety
            /// Called only by the host, with a live `AudioCall`.
            unsafe extern "C" fn entry(
                call: *const $crate::audio::AudioCall,
            ) -> $crate::audio::AudioStatus {
                // Recover from poisoning rather than refusing. The lock is
                // poisoned by a panic the dispatcher already caught and
                // reported; treating it as fatal would silence the game for the
                // rest of the session over one bad frame.
                let mut guard = match STATE.lock() {
                    ::std::result::Result::Ok(g) => g,
                    ::std::result::Result::Err(p) => p.into_inner(),
                };
                let backend = guard.get_or_insert_with(::std::default::Default::default);
                $crate::audio::dispatch(backend, call)
            }

            /// The descriptor to hand to `App::add_audio_backend`.
            pub fn desc() -> $crate::audio::AudioBackendDesc {
                $crate::audio::desc_for::<Backend>(entry)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::BlobRef;
    use std::vec;

    /// Run one op against `backend` and collect whatever it writes back.
    fn call(op: AudioOp, payload: &[u8], blob: &[u8], backend: &mut Probe) -> (AudioStatus, Vec<u8>) {
            let mut out: Vec<u8> = Vec::new();
            unsafe extern "C" fn write_fn(ctx: *mut core::ffi::c_void, bytes: *const u8, len: usize) {
                let v = &mut *(ctx as *mut Vec<u8>);
                v.extend_from_slice(core::slice::from_raw_parts(bytes, len));
            }
            let sink = ByteSink {
                ctx: &mut out as *mut Vec<u8> as *mut core::ffi::c_void,
                write: write_fn,
            };
            let call = AudioCall {
                op,
                _pad: 0,
                state: core::ptr::null_mut(),
                payload: BlobRef::new(payload),
                blob: BlobRef::new(blob),
                out: &sink,
            };
        let status = unsafe { dispatch(backend, &call) };
        (status, out)
    }

    #[derive(Default)]
    struct Probe {
        buses: usize,
        played: usize,
        panic_on_play: bool,
    }

    impl Backend for Probe {
        const NAME: &'static str = "probe";

        fn init(&mut self) -> Result<BackendInfo, String> {
            Ok(BackendInfo {
                sample_rate: 48_000,
                caps: super::super::Caps::SPATIAL,
                device: String::from("test"),
            })
        }

        fn set_buses(&mut self, buses: &[BusState]) {
            self.buses = buses.len();
        }

        fn load_clip(&mut self, _clip: u64, _ext: &str, bytes: &[u8]) -> Result<ClipInfo, String> {
            if bytes.is_empty() {
                return Err(String::from("empty file"));
            }
            Ok(ClipInfo {
                duration: 1.5,
                sample_rate: 48_000,
            })
        }

        fn play(&mut self, _request: &PlayRequest) -> Result<(), String> {
            if self.panic_on_play {
                panic!("deliberate");
            }
            self.played += 1;
            Ok(())
        }

        fn update(&mut self, _request: &UpdateRequest) -> UpdateReply {
            UpdateReply {
                peaks: vec![0.25],
                finished: vec![9],
            }
        }
    }

    fn encoded(f: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut w = Writer::new();
        f(&mut w);
        w.into_bytes()
    }

    #[test]
    fn init_replies_with_the_backend_info() {
        let mut b = Probe::default();
        let (status, out) = call(AudioOp::Init, &[], &[], &mut b);
        assert_eq!(status, AudioStatus::Ok);
        let info = BackendInfo::decode(&mut Reader::new(&out)).unwrap();
        assert_eq!(info.sample_rate, 48_000);
        assert_eq!(info.device, "test");
    }

    #[test]
    fn a_bus_graph_reaches_the_backend() {
        let mut b = Probe::default();
        let payload = encoded(|w| {
            super::super::write_buses(
                w,
                &[
                    BusState { key: String::from("Master"), gain: 1.0, pan: 0.0, muted: false, soloed: false },
                    BusState { key: String::from("Sfx"), gain: 0.5, pan: 0.0, muted: false, soloed: false },
                ],
            )
        });
        let (status, _) = call(AudioOp::SetBuses, &payload, &[], &mut b);
        assert_eq!(status, AudioStatus::Ok);
        assert_eq!(b.buses, 2);
    }

    #[test]
    fn clip_bytes_arrive_in_the_blob_rather_than_the_payload() {
        let mut b = Probe::default();
        let payload = encoded(|w| LoadClip { clip: 1, extension: String::from("wav") }.encode(w));
        let (status, out) = call(AudioOp::LoadClip, &payload, &[1, 2, 3], &mut b);
        assert_eq!(status, AudioStatus::Ok);
        assert_eq!(ClipInfo::decode(&mut Reader::new(&out)).unwrap().duration, 1.5);

        // Empty blob = the backend's own error path, not a decode failure.
        let (status, _) = call(AudioOp::LoadClip, &payload, &[], &mut b);
        assert_eq!(status, AudioStatus::Error);
    }

    #[test]
    fn an_update_round_trips_through_the_boundary() {
        let mut b = Probe::default();
        let payload = encoded(|w| UpdateRequest::default().encode(w));
        let (status, out) = call(AudioOp::Update, &payload, &[], &mut b);
        assert_eq!(status, AudioStatus::Ok);
        let reply = UpdateReply::decode(&mut Reader::new(&out)).unwrap();
        assert_eq!(reply.peaks, vec![0.25]);
        assert_eq!(reply.finished, vec![9]);
    }

    /// The mechanism that makes appending an op safe for already-built backends.
    #[test]
    fn an_op_from_a_newer_host_is_unknown_rather_than_an_error() {
        let mut b = Probe::default();
        let (status, out) = call(AudioOp(9999), &[], &[], &mut b);
        assert_eq!(status, AudioStatus::UnknownOp);
        assert!(out.is_empty());
    }

    /// A panic unwinding out of an `extern "C"` frame aborts the process.
    #[test]
    fn a_panicking_backend_is_caught_and_reported() {
        let mut b = Probe { panic_on_play: true, ..Default::default() };
        let payload = encoded(|w| {
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
            .encode(w)
        });
        let (status, out) = call(AudioOp::Play, &payload, &[], &mut b);
        assert_eq!(status, AudioStatus::Panicked);
        let message = Reader::new(&out).string().unwrap();
        assert!(message.contains("probe"), "{message}");
    }

    /// The bytes came from another binary; a malformed payload must be reported,
    /// not read past.
    #[test]
    fn a_malformed_payload_is_an_error_with_a_message() {
        let mut b = Probe::default();
        let (status, out) = call(AudioOp::Play, &[0, 1, 2], &[], &mut b);
        assert_eq!(status, AudioStatus::Error);
        let message = Reader::new(&out).string().unwrap();
        assert!(message.contains("play request"), "{message}");
    }

    #[test]
    fn a_null_call_is_refused_rather_than_dereferenced() {
        let mut b = Probe::default();
        assert_eq!(
            unsafe { dispatch(&mut b, core::ptr::null()) },
            AudioStatus::Error
        );
    }

    /// Defaults exist so a minimal backend implements four methods; a host that
    /// asks anyway must get a clean answer.
    #[test]
    fn unimplemented_optional_ops_answer_cleanly() {
        let mut b = Probe::default();
        let payload = encoded(|w| OpenCapture { capture: 1, device: None }.encode(w));
        let (status, out) = call(AudioOp::OpenCapture, &payload, &[], &mut b);
        assert_eq!(status, AudioStatus::Error);
        assert!(Reader::new(&out).string().unwrap().contains("cannot capture"));

        let payload = encoded(|w| w.u64(1));
        let (status, out) = call(AudioOp::ReadCapture, &payload, &[], &mut b);
        assert_eq!(status, AudioStatus::Ok);
        assert!(read_samples(&mut Reader::new(&out)).unwrap().is_empty());

        let (status, out) = call(AudioOp::ListDevices, &[], &[], &mut b);
        assert_eq!(status, AudioStatus::Ok);
        assert_eq!(DeviceList::decode(&mut Reader::new(&out)).unwrap(), DeviceList::default());
    }
}
