//! The side of the network boundary a backend plugin writes.
//!
//! [`Backend`] is one method per [`NetOp`], with defaults for everything a
//! minimal client can do without. Everything below the trait is plumbing an
//! implementor never touches: decoding the request, catching panics, encoding
//! the reply.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::string::String;
use std::vec::Vec;

use super::protocol::{read_events, write_events, BackendInfo, Event, Request};
use crate::sys::{ByteSink, NetBackendDesc, NetCall, NetEntry, NetOp, NetStatus, Str256};
use crate::wire::{Reader, Writer};

/// What a network backend implements.
///
/// ## It must not block
///
/// [`start`](Self::start) is called from the engine's frame, and
/// [`poll`](Self::poll) is called once per frame after it. Doing the transfer
/// inside `start` would stall the editor for the length of a round trip — which
/// is the entire reason this is a queue-and-poll interface rather than a
/// `fetch(url) -> Response` one. Spawn a thread, hand the work to a runtime, do
/// whatever you like; just return.
///
/// The host has its own blocking facade layered on top for callers that want
/// one, and it works by parking the *calling* thread while the frame keeps
/// running. That only holds together if this side stays asynchronous.
pub trait Backend: Default + Send + 'static {
    /// Human-readable, for logs and the editor's network settings.
    const NAME: &'static str;

    /// Bring the client up.
    ///
    /// The returned [`Caps`](super::Caps) is a promise the host relies on: it
    /// will not send [`Cancel`](NetOp::Cancel) to a backend that did not claim
    /// [`CANCEL`](super::Caps::CANCEL), and it *will* assume a streaming request
    /// arrives in pieces if [`STREAM`](super::Caps::STREAM) is set. Claiming
    /// something you do not do produces an editor that hangs waiting for chunks
    /// that never come.
    fn init(&mut self) -> Result<BackendInfo, String>;

    /// Cancel everything in flight and release the client. Called before unload.
    fn shutdown(&mut self) {}

    /// Begin a request. Return immediately — see the trait docs.
    ///
    /// `body` is empty for a GET. An `Err` here means the request could not be
    /// *started* (a URL that would not parse); a request that starts and then
    /// fails reports [`EventKind::Error`](super::EventKind::Error) through
    /// [`poll`](Self::poll) instead, because by then the host is already waiting
    /// on the tag and an error it never hears about is a caller that waits
    /// forever.
    fn start(&mut self, request: &Request, body: &[u8]) -> Result<(), String>;

    /// Take everything that has arrived since the last call.
    ///
    /// Called once per frame. Returning the same event twice delivers it twice;
    /// this is a drain, not a peek.
    fn poll(&mut self) -> Vec<Event>;

    /// Abandon a request. Only called when [`Caps::CANCEL`](super::Caps::CANCEL)
    /// was claimed.
    ///
    /// The host has stopped caring about the answer. Any event still emitted for
    /// this tag is discarded, so the only thing at stake is whether the transfer
    /// keeps consuming bandwidth.
    fn cancel(&mut self, tag: u64) {
        let _ = tag;
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

/// Write an error message as the reply. Paired with [`NetStatus::Error`], so the
/// host has something to log beyond "it failed".
unsafe fn reply_error(sink: *const ByteSink, message: &str) {
    reply(sink, |w| w.str(message));
}

/// Route one call to `backend`.
///
/// # Safety
/// `call` must be a live [`NetCall`] from the host.
pub unsafe fn dispatch<B: Backend>(backend: &mut B, call: *const NetCall) -> NetStatus {
    if call.is_null() {
        return NetStatus::Error;
    }
    let call = &*call;

    // A panic must not unwind out of the `extern "C"` frame the host called us
    // through — that is an abort, and taking the editor down because one URL had
    // a malformed header is not a proportionate response. Catch here and let the
    // host stop calling this backend.
    match catch_unwind(AssertUnwindSafe(|| run(backend, call))) {
        Ok(status) => status,
        Err(_) => {
            reply_error(call.out, &std::format!("{} panicked", B::NAME));
            NetStatus::Panicked
        }
    }
}

unsafe fn run<B: Backend>(backend: &mut B, call: &NetCall) -> NetStatus {
    let payload = call.payload.as_slice();
    let blob = call.blob.as_slice();

    match call.op {
        NetOp::Init => match backend.init() {
            Ok(info) => {
                reply(call.out, |w| info.encode(w));
                NetStatus::Ok
            }
            Err(e) => {
                reply_error(call.out, &e);
                NetStatus::Error
            }
        },
        NetOp::Shutdown => {
            backend.shutdown();
            NetStatus::Ok
        }
        NetOp::Start => {
            let mut r = Reader::new(payload);
            match Request::decode(&mut r) {
                Ok(request) => match backend.start(&request, blob) {
                    Ok(()) => NetStatus::Ok,
                    Err(e) => {
                        reply_error(call.out, &e);
                        NetStatus::Error
                    }
                },
                Err(e) => malformed(call.out, "request", e),
            }
        }
        NetOp::Poll => {
            let events = backend.poll();
            reply(call.out, |w| write_events(w, &events));
            NetStatus::Ok
        }
        NetOp::Cancel => {
            let mut r = Reader::new(payload);
            match r.u64() {
                Ok(tag) => {
                    backend.cancel(tag);
                    NetStatus::Ok
                }
                Err(e) => malformed(call.out, "cancel tag", e),
            }
        }
        // An op this build has never heard of. Not an error: it is what makes
        // appending an op safe for backends built before it existed.
        _ => NetStatus::UnknownOp,
    }
}

/// Report a payload that would not decode.
///
/// Its own path because it means something specific and alarming: the bytes came
/// from another binary, so a failure here is the two sides disagreeing about the
/// format rather than a bad input. Naming which payload is the only clue anyone
/// gets.
unsafe fn malformed(sink: *const ByteSink, what: &str, error: crate::wire::WireError) -> NetStatus {
    reply_error(sink, &std::format!("{what} would not decode: {error}"));
    NetStatus::Error
}

/// Build the descriptor for a backend type.
///
/// `state` is left null by [`net_backend!`], which parks the backend in a
/// `static` instead. The field stays in the descriptor for a hand-written
/// backend that wants more than one instance.
pub fn desc_for<B: Backend>(entry: NetEntry) -> NetBackendDesc {
    NetBackendDesc {
        name: Str256::new_truncating(B::NAME),
        state: core::ptr::null_mut(),
        entry,
    }
}

/// Decode a [`Poll`](NetOp::Poll) reply. The host's half of [`write_events`];
/// here so both sides read the batch through one function.
pub fn decode_events(bytes: &[u8]) -> Result<Vec<Event>, crate::wire::WireError> {
    read_events(&mut Reader::new(bytes))
}

/// Emit the `extern "C"` entry point for a [`Backend`] and the state it needs.
///
/// A macro rather than a generic because the entry point must be a bare function
/// pointer with nowhere to carry state, so it needs a `static` — and a `static`
/// cannot be generic over the backend type.
///
/// ```ignore
/// renzora_plugin::net_backend!(client::Ureq);
///
/// impl Plugin for HttpPlugin {
///     fn build(&self, app: &mut App) {
///         app.add_net_backend(net_backend::desc());
///     }
/// }
/// ```
#[macro_export]
macro_rules! net_backend {
    ($ty:ty) => {
        /// Generated by `renzora_plugin::net_backend!`.
        pub mod net_backend {
            #[allow(unused_imports)]
            use super::*;

            type Backend = $ty;

            static STATE: ::std::sync::Mutex<::std::option::Option<Backend>> =
                ::std::sync::Mutex::new(::std::option::Option::None);

            /// # Safety
            /// Called only by the host, with a live `NetCall`.
            unsafe extern "C" fn entry(
                call: *const $crate::net::NetCall,
            ) -> $crate::net::NetStatus {
                // Recover from poisoning rather than refusing. The lock is
                // poisoned by a panic the dispatcher already caught and
                // reported; treating it as fatal would take the editor offline
                // for the rest of the session over one bad request.
                let mut guard = match STATE.lock() {
                    ::std::result::Result::Ok(g) => g,
                    ::std::result::Result::Err(p) => p.into_inner(),
                };
                let backend = guard.get_or_insert_with(::std::default::Default::default);
                $crate::net::dispatch(backend, call)
            }

            /// The descriptor to hand to `App::add_net_backend`.
            pub fn desc() -> $crate::net::NetBackendDesc {
                $crate::net::desc_for::<Backend>(entry)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Caps, EventKind};
    use crate::sys::BlobRef;
    use std::string::ToString;
    use std::vec;

    /// Run one op against `backend` and collect whatever it writes back.
    fn call(op: NetOp, payload: &[u8], blob: &[u8], backend: &mut Probe) -> (NetStatus, Vec<u8>) {
        let mut out: Vec<u8> = Vec::new();
        unsafe extern "C" fn write_fn(ctx: *mut core::ffi::c_void, bytes: *const u8, len: usize) {
            let v = &mut *(ctx as *mut Vec<u8>);
            v.extend_from_slice(core::slice::from_raw_parts(bytes, len));
        }
        let sink = ByteSink {
            ctx: &mut out as *mut Vec<u8> as *mut core::ffi::c_void,
            write: write_fn,
        };
        let call = NetCall {
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
        started: Vec<(String, String, usize)>,
        cancelled: Vec<u64>,
        queued: Vec<Event>,
        panic_on_start: bool,
    }

    impl Backend for Probe {
        const NAME: &'static str = "probe";

        fn init(&mut self) -> Result<BackendInfo, String> {
            Ok(BackendInfo {
                agent: "probe/1".to_string(),
                caps: Caps::STREAM | Caps::CANCEL,
            })
        }

        fn start(&mut self, request: &Request, body: &[u8]) -> Result<(), String> {
            if self.panic_on_start {
                panic!("deliberate");
            }
            if request.url.is_empty() {
                return Err("empty url".to_string());
            }
            self.started
                .push((request.method.clone(), request.url.clone(), body.len()));
            self.queued.push(Event {
                tag: request.tag,
                kind: EventKind::Response,
                status: 200,
                headers: Vec::new(),
                body: b"ok".to_vec(),
            });
            Ok(())
        }

        fn poll(&mut self) -> Vec<Event> {
            std::mem::take(&mut self.queued)
        }

        fn cancel(&mut self, tag: u64) {
            self.cancelled.push(tag);
        }
    }

    fn encoded(f: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut w = Writer::new();
        f(&mut w);
        w.into_bytes()
    }

    fn request(tag: u64, url: &str) -> Request {
        Request {
            tag,
            method: "GET".to_string(),
            url: url.to_string(),
            headers: Vec::new(),
            stream: false,
            timeout_ms: 0,
            max_bytes: 0,
        }
    }

    #[test]
    fn init_replies_with_the_backend_info() {
        let mut b = Probe::default();
        let (status, out) = call(NetOp::Init, &[], &[], &mut b);
        assert_eq!(status, NetStatus::Ok);
        let info = BackendInfo::decode(&mut Reader::new(&out)).unwrap();
        assert_eq!(info.agent, "probe/1");
        assert!(info.caps.contains(Caps::STREAM));
    }

    /// The reason `blob` exists: an upload body must not go through the codec.
    #[test]
    fn the_request_body_arrives_in_the_blob_rather_than_the_payload() {
        let mut b = Probe::default();
        let payload = encoded(|w| request(7, "https://example.com/upload").encode(w));
        let (status, _) = call(NetOp::Start, &payload, &[0u8; 4096], &mut b);
        assert_eq!(status, NetStatus::Ok);
        assert_eq!(b.started[0].2, 4096);
    }

    #[test]
    fn a_started_request_comes_back_through_poll() {
        let mut b = Probe::default();
        let payload = encoded(|w| request(7, "https://example.com").encode(w));
        call(NetOp::Start, &payload, &[], &mut b);

        let (status, out) = call(NetOp::Poll, &[], &[], &mut b);
        assert_eq!(status, NetStatus::Ok);
        let events = decode_events(&out).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tag, 7);
        assert_eq!(events[0].body, b"ok");

        // A drain, not a peek — the second poll is empty.
        let (_, out) = call(NetOp::Poll, &[], &[], &mut b);
        assert!(decode_events(&out).unwrap().is_empty());
    }

    #[test]
    fn a_request_that_will_not_start_is_an_error_with_a_message() {
        let mut b = Probe::default();
        let payload = encoded(|w| request(1, "").encode(w));
        let (status, out) = call(NetOp::Start, &payload, &[], &mut b);
        assert_eq!(status, NetStatus::Error);
        assert_eq!(Reader::new(&out).string().unwrap(), "empty url");
    }

    #[test]
    fn cancel_reaches_the_backend() {
        let mut b = Probe::default();
        let (status, _) = call(NetOp::Cancel, &encoded(|w| w.u64(9)), &[], &mut b);
        assert_eq!(status, NetStatus::Ok);
        assert_eq!(b.cancelled, vec![9]);
    }

    /// The mechanism that makes appending an op safe for already-built backends.
    #[test]
    fn an_op_from_a_newer_host_is_unknown_rather_than_an_error() {
        let mut b = Probe::default();
        let (status, out) = call(NetOp(9999), &[], &[], &mut b);
        assert_eq!(status, NetStatus::UnknownOp);
        assert!(out.is_empty());
    }

    /// A panic unwinding out of an `extern "C"` frame aborts the process.
    #[test]
    fn a_panicking_backend_is_caught_and_reported() {
        let mut b = Probe {
            panic_on_start: true,
            ..Default::default()
        };
        let payload = encoded(|w| request(1, "https://example.com").encode(w));
        let (status, out) = call(NetOp::Start, &payload, &[], &mut b);
        assert_eq!(status, NetStatus::Panicked);
        assert!(Reader::new(&out).string().unwrap().contains("probe"));
    }

    /// The bytes came from another binary; a malformed payload must be reported,
    /// not read past.
    #[test]
    fn a_malformed_payload_is_an_error_with_a_message() {
        let mut b = Probe::default();
        let (status, out) = call(NetOp::Start, &[0, 1, 2], &[], &mut b);
        assert_eq!(status, NetStatus::Error);
        assert!(Reader::new(&out).string().unwrap().contains("request"));
    }

    #[test]
    fn a_null_call_is_refused_rather_than_dereferenced() {
        let mut b = Probe::default();
        assert_eq!(
            unsafe { dispatch(&mut b, core::ptr::null()) },
            NetStatus::Error
        );
    }
}
