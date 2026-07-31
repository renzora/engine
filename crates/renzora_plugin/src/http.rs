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

/// Identifies this service in the host's queue.
pub const SERVICE: u64 = sys::service_id("renzora.http");

/// Which HTTP verb a request uses.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HttpOp(pub u32);

#[allow(non_upper_case_globals)]
impl HttpOp {
    pub const Get: Self = Self(0);
    pub const Post: Self = Self(1);

    pub const fn is_known(self) -> bool {
        self.0 < 2
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "GET",
            1 => "POST",
            _ => "?",
        }
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
}
