//! HTTP for standalone plugins.
//!
//! ```ignore
//! use renzora_plugin::prelude::*;
//! use renzora_plugin::http::{Http, HttpCommands};
//!
//! const FETCH: u64 = 1;
//!
//! fn kick_off(mut cmds: Commands) {
//!     cmds.http_get(FETCH, "https://example.com/status.json");
//! }
//!
//! fn collect(http: Http) {
//!     if let Some(r) = http.poll(FETCH) {
//!         info(&format!("{} → {} bytes", r.status, r.body.len()));
//!     }
//! }
//! ```
//!
//! A domain module like [`anim`](crate::anim) and [`physics`](crate::physics),
//! riding the generic service channel — [`sys`](crate::sys) knows nothing about
//! HTTP and this does not move the ABI version.
//!
//! ## Why a tag instead of a callback
//!
//! The boundary has no way to call back into a plugin outside a system, and no
//! futures — a function pointer handed over would have to stay valid across a
//! hot reload, which is exactly what generation-gating exists to prevent. So a
//! request carries a `u64` the plugin chose, and the plugin polls for it. That
//! is also what makes reloading safe: a response whose requester was swapped out
//! is simply never collected, rather than dispatched into a dead build.
//!
//! Requests are **not** entity-scoped. Fetching a URL belongs to the plugin, not
//! to anything in the world, so these go through
//! [`Commands::call_service`](crate::ecs::Commands::call_service).

use crate::ecs::Commands;
use crate::sys;
// From `alloc`, not the prelude — a response body is owned data, and this module
// has to compile in a `no_std` plugin where the prelude supplies neither.
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// Identifies this service in the host's queue.
pub const SERVICE: u64 = sys::service_id("renzora.http");

/// Which HTTP verb a request uses.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HttpOp(pub u32);

/// Set on an [`HttpOp`] to say the payload carries a headers block, and so uses
/// [`HttpRequestHeader`] rather than [`HttpHeader`].
///
/// A flag bit rather than four more verbs, and rather than appending a field to
/// [`HttpHeader`]. That struct is allocated by the PLUGIN and read by the host,
/// which is the direction the append-only rule does not cover: a new host
/// reading an old plugin's shorter header would take four bytes of URL as a
/// length. The op is the one thing both sides already agree on before either
/// reads a byte of the payload, so it is what selects the shape.
pub const WITH_HEADERS: u32 = 0x8000_0000;

#[allow(non_upper_case_globals)]
impl HttpOp {
    pub const Get: Self = Self(0);
    pub const Post: Self = Self(1);
    /// `GET`, delivered in pieces — poll with [`Http::poll_stream`].
    pub const GetStream: Self = Self(2);
    /// `POST`, delivered in pieces — poll with [`Http::poll_stream`].
    pub const PostStream: Self = Self(3);

    /// The verb, with [`WITH_HEADERS`] masked off.
    pub const fn verb(self) -> u32 {
        self.0 & !WITH_HEADERS
    }

    /// Whether the payload carries a headers block.
    pub const fn has_headers(self) -> bool {
        self.0 & WITH_HEADERS != 0
    }

    /// The same op, promising a headers block.
    pub const fn with_headers(self) -> Self {
        Self(self.0 | WITH_HEADERS)
    }

    pub const fn is_known(self) -> bool {
        self.verb() < 4
    }

    /// Whether the response arrives as chunks rather than one body.
    pub const fn is_streaming(self) -> bool {
        self.verb() == 2 || self.verb() == 3
    }

    /// The HTTP verb. Streaming is a delivery mode, not a different method, so
    /// the two stream ops name the same verbs the host would otherwise send.
    pub const fn name(self) -> &'static str {
        match self.verb() {
            0 | 2 => "GET",
            1 | 3 => "POST",
            _ => "?",
        }
    }
}

/// Header of a request that carries HTTP headers: the URL, then the body, then
/// the headers block follow it in the same buffer.
///
/// A separate struct from [`HttpHeader`] rather than a fourth field on it — see
/// [`WITH_HEADERS`] for why that append would be unsound in this direction.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HttpRequestHeader {
    pub tag: u64,
    pub url_len: u32,
    pub body_len: u32,
    pub headers_len: u32,
    pub _pad: u32,
}

/// HTTP headers to send with a request.
///
/// Crosses the boundary as text — `"Authorization:Bearer sk-x\nAccept:text/event-stream"`
/// — rather than as a struct, because the count is variable and a `Vec` of
/// anything cannot cross. Same shape as
/// [`DialogFilter`](crate::dialog::DialogFilter), for the same reason.
#[derive(Clone, Debug, Default)]
pub struct HttpHeaders(String);

impl HttpHeaders {
    pub fn new() -> Self {
        Self(String::new())
    }

    /// Add one header.
    ///
    /// A name containing a separator, or a value containing a newline, would
    /// re-split wrongly on the host side and could smuggle a second header in —
    /// the HTTP equivalent of an injection. Both are filtered rather than
    /// escaped, because a header name with a newline in it is never legitimate
    /// and silently dropping the character is safer than carrying it.
    pub fn add(mut self, name: &str, value: &str) -> Self {
        if !self.0.is_empty() {
            self.0.push('\n');
        }
        for c in name.chars().filter(|c| *c != ':' && *c != '\n' && *c != '\r') {
            self.0.push(c);
        }
        self.0.push(':');
        for c in value.chars().filter(|c| *c != '\n' && *c != '\r') {
            self.0.push(c);
        }
        self
    }

    /// `Authorization: Bearer <token>` — what almost every hosted LLM wants.
    pub fn bearer(self, token: &str) -> Self {
        self.add("Authorization", &alloc::format!("Bearer {token}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl core::fmt::Debug for HttpOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// Header of an HTTP service payload; the URL bytes then the body bytes follow
/// it in the same buffer.
///
/// Lengths rather than inline fixed arrays, because a URL with a query string
/// and a JSON body are both genuinely variable — unlike an animation clip name,
/// where a cap is a reasonable constraint rather than an arbitrary one.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HttpHeader {
    /// The plugin's own identifier, echoed back on the response.
    pub tag: u64,
    pub url_len: u32,
    pub body_len: u32,
}

/// A completed response.
#[derive(Clone, Debug)]
pub struct HttpResponse {
    /// HTTP status, or 0 if the request never completed — `body` then holds the
    /// error text.
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    /// Whether the status is 2xx.
    pub fn is_ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Collects completed HTTP responses.
///
/// A system param rather than an [`Interface`](sys::Interface) function, for the
/// same reason [`Meshes`](crate::ecs::Meshes) is: delivery needs host state and
/// `SystemCall::host` is null while a system runs.
pub struct Http<'a> {
    src: *mut sys::HttpSource,
    _p: core::marker::PhantomData<&'a ()>,
}

impl Http<'_> {
    /// Take the next completed response for `tag`, if one is ready.
    ///
    /// `None` is the normal state — a request takes many frames. A response is
    /// delivered exactly once; poll again for the next.
    pub fn poll(&self, tag: u64) -> Option<HttpResponse> {
        if self.src.is_null() {
            return None;
        }
        // The probe must not consume, or a caller that fails to allocate would
        // silently drop the response. The host only removes it on the second
        // pass, which is the one that actually takes the bytes.
        let mut probe = sys::HttpRead::COUNTS_ONLY;
        unsafe {
            if !((*self.src).poll)(self.src, tag, &mut probe) {
                return None;
            }
        }
        let mut body = vec![0u8; probe.body_len];
        let mut fill = sys::HttpRead {
            body_capacity: body.len(),
            body: body.as_mut_ptr(),
            ..sys::HttpRead::COUNTS_ONLY
        };
        unsafe {
            if !((*self.src).poll)(self.src, tag, &mut fill) {
                return None;
            }
        }
        body.truncate(fill.body_len);
        Some(HttpResponse {
            status: fill.status,
            // Lossy rather than refusing: a server that returns malformed UTF-8
            // should not make the response unreachable, and the plugin has no
            // other way to see what came back.
            body: String::from_utf8_lossy(&body).into_owned(),
        })
    }
}

/// One piece of a streaming response.
#[derive(Clone, Debug)]
pub struct HttpChunk {
    /// HTTP status, repeated on every chunk so a plugin that keeps only the
    /// latest still knows it. 0 means the request never reached a response.
    pub status: u16,
    /// Body bytes for this chunk. Empty on [`HttpChunkKind::End`].
    pub data: String,
    pub kind: sys::HttpChunkKind,
}

impl HttpChunk {
    /// Whether this is the last chunk for its tag — stop polling after it.
    pub fn is_last(&self) -> bool {
        self.kind.is_terminal()
    }

    /// Whether the stream ended because something went wrong. `data` then holds
    /// the error text.
    pub fn is_error(&self) -> bool {
        self.kind == sys::HttpChunkKind::Error
    }
}

impl Http<'_> {
    /// Take the next chunk for `tag`, for a request issued with
    /// [`HttpOp::GetStream`] or [`HttpOp::PostStream`].
    ///
    /// Unlike [`poll`](Self::poll), one request produces **many** of these. Call
    /// it in a loop until it returns `None` (nothing more this frame) and stop
    /// for good once a chunk reports [`is_last`](HttpChunk::is_last) — a
    /// terminal chunk is the host's promise that the tag is finished, and
    /// polling past it just returns `None` forever.
    ///
    /// ```ignore
    /// while let Some(chunk) = http.poll_stream(MY_TAG) {
    ///     reply.push_str(&chunk.data);
    ///     if chunk.is_last() { break; }
    /// }
    /// ```
    pub fn poll_stream(&self, tag: u64) -> Option<HttpChunk> {
        if self.src.is_null() {
            return None;
        }
        // Two passes, same as `poll`: the probe must not consume, or a caller
        // that fails to allocate would silently drop a chunk out of the middle
        // of a stream — which, unlike dropping a whole response, would corrupt
        // the reply rather than merely losing it.
        let mut probe = sys::HttpChunkRead::COUNTS_ONLY;
        unsafe {
            if !((*self.src).poll_stream)(self.src, tag, &mut probe) {
                return None;
            }
        }
        // A terminal chunk carries no body, so there is nothing to allocate and
        // nothing to fill — but it still has to be CONSUMED, or the same end
        // marker is returned every frame forever. A one-byte scratch buffer is
        // what makes the second pass a consuming one.
        let mut body = vec![0u8; probe.body_len.max(1)];
        let mut fill = sys::HttpChunkRead {
            body_capacity: body.len(),
            body: body.as_mut_ptr(),
            ..sys::HttpChunkRead::COUNTS_ONLY
        };
        unsafe {
            if !((*self.src).poll_stream)(self.src, tag, &mut fill) {
                return None;
            }
        }
        body.truncate(fill.body_len);
        Some(HttpChunk {
            status: fill.status,
            data: String::from_utf8_lossy(&body).into_owned(),
            kind: fill.kind,
        })
    }
}

unsafe impl crate::ecs::SystemParam for Http<'_> {
    fn declare(_: &mut crate::ecs::InitCtx, _: &mut crate::ecs::SystemBuilder) {}
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        Http {
            src: (*call).http,
            _p: core::marker::PhantomData,
        }
    }
}

/// HTTP methods on [`Commands`].
pub trait HttpCommands {
    /// Issue a request. The others are wrappers.
    fn http(&mut self, op: HttpOp, tag: u64, url: &str, body: Option<&str>) -> &mut Self;

    /// GET `url`; poll [`Http`] for `tag` to collect the response.
    fn http_get(&mut self, tag: u64, url: &str) -> &mut Self;
    /// POST `body` to `url`; poll [`Http`] for `tag` to collect the response.
    fn http_post(&mut self, tag: u64, url: &str, body: &str) -> &mut Self;

    /// GET `url`, delivered in pieces — poll [`Http::poll_stream`] for `tag`.
    fn http_get_stream(&mut self, tag: u64, url: &str) -> &mut Self;
    /// POST `body` to `url`, delivered in pieces — poll [`Http::poll_stream`].
    ///
    /// This is the shape a token-streaming chat API wants: the reply arrives as
    /// NDJSON or SSE over one long-lived response, and waiting for the whole
    /// body would defeat the point of streaming it.
    fn http_post_stream(&mut self, tag: u64, url: &str, body: &str) -> &mut Self;

    /// Any of the above, with HTTP headers.
    ///
    /// The one that unlocks hosted APIs: an `Authorization: Bearer` is the
    /// difference between reaching a local Ollama and reaching anything that
    /// charges for tokens.
    ///
    /// ```ignore
    /// commands.http_with(
    ///     HttpOp::PostStream, TAG, url, Some(body),
    ///     &HttpHeaders::new().bearer(&key),
    /// );
    /// ```
    fn http_with(
        &mut self,
        op: HttpOp,
        tag: u64,
        url: &str,
        body: Option<&str>,
        headers: &HttpHeaders,
    ) -> &mut Self;
}

impl HttpCommands for Commands<'_> {
    fn http(&mut self, op: HttpOp, tag: u64, url: &str, body: Option<&str>) -> &mut Self {
        let body = body.unwrap_or("");
        let header = HttpHeader {
            tag,
            url_len: url.len() as u32,
            body_len: body.len() as u32,
        };
        let mut payload = Vec::with_capacity(
            core::mem::size_of::<HttpHeader>() + url.len() + body.len(),
        );
        // SAFETY: `#[repr(C)]`, no pointers, no `Drop`.
        payload.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                (&header as *const HttpHeader).cast::<u8>(),
                core::mem::size_of::<HttpHeader>(),
            )
        });
        payload.extend_from_slice(url.as_bytes());
        payload.extend_from_slice(body.as_bytes());
        self.call_service(SERVICE, op.0, &payload)
    }

    fn http_get(&mut self, tag: u64, url: &str) -> &mut Self {
        self.http(HttpOp::Get, tag, url, None)
    }

    fn http_post(&mut self, tag: u64, url: &str, body: &str) -> &mut Self {
        self.http(HttpOp::Post, tag, url, Some(body))
    }

    fn http_get_stream(&mut self, tag: u64, url: &str) -> &mut Self {
        self.http(HttpOp::GetStream, tag, url, None)
    }

    fn http_post_stream(&mut self, tag: u64, url: &str, body: &str) -> &mut Self {
        self.http(HttpOp::PostStream, tag, url, Some(body))
    }

    fn http_with(
        &mut self,
        op: HttpOp,
        tag: u64,
        url: &str,
        body: Option<&str>,
        headers: &HttpHeaders,
    ) -> &mut Self {
        // No headers to send means the plain payload, so an empty `HttpHeaders`
        // costs nothing and a caller need not branch on it.
        if headers.is_empty() {
            return self.http(op, tag, url, body);
        }
        let body = body.unwrap_or("");
        let headers = headers.as_str();
        let header = HttpRequestHeader {
            tag,
            url_len: url.len() as u32,
            body_len: body.len() as u32,
            headers_len: headers.len() as u32,
            _pad: 0,
        };
        let mut payload = Vec::with_capacity(
            core::mem::size_of::<HttpRequestHeader>() + url.len() + body.len() + headers.len(),
        );
        // SAFETY: `#[repr(C)]`, no pointers, no `Drop`.
        payload.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                (&header as *const HttpRequestHeader).cast::<u8>(),
                core::mem::size_of::<HttpRequestHeader>(),
            )
        });
        payload.extend_from_slice(url.as_bytes());
        payload.extend_from_slice(body.as_bytes());
        payload.extend_from_slice(headers.as_bytes());
        self.call_service(SERVICE, op.with_headers().0, &payload)
    }
}
