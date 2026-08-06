//! The HTTP half of the standalone-plugin boundary.
//!
//! Same construction as the animation and physics bridges: `renzora_plugin::sys`
//! carries opaque bytes, `renzora_plugin::http` is the Bevy-free vocabulary both
//! sides compile, and this module is the engine side that claims the service.
//!
//! It lives in `renzora_scripting` because that is where the HTTP client
//! already is — [`HttpInbox`] runs the blocking `ureq` call on a background
//! thread so the game loop never waits on the network. A plugin's request takes
//! exactly the same path a script's `http_get` does; there is one client and one
//! thread pool, not two.
//!
//! ## Requests in, responses out
//!
//! Requests arrive as parked service calls and are decoded here — a
//! [`HttpHeader`] followed by the URL and body bytes, both length-prefixed
//! because a URL with a query string and a JSON payload are genuinely
//! variable-length.
//!
//! Responses go back through [`PluginHttpInbox`], which the plugin drains via
//! its `Http` system param. The plugin's own `tag` is what pairs the two — the
//! boundary has no callbacks, and a function pointer handed over would have to
//! survive a hot reload, which is exactly what generation-gating prevents. A
//! response whose requester was swapped out is simply never collected.

use bevy::prelude::*;

use renzora_plugin::host::{PluginHttpInbox, PluginHttpResponse, PluginServiceCalls};
use renzora_plugin::http::{HttpHeader, HttpOp, HttpRequestHeader};
use renzora_plugin::sys;

use crate::http::{ChunkKind, HttpInbox};


/// Callback name used for plugin requests inside [`HttpInbox`].
///
/// The inbox is shared with scripts, whose callbacks are script-chosen names.
/// A prefix nothing else uses keeps the two populations apart, and the plugin's
/// numeric tag is appended so the response can be routed back to it.
const PLUGIN_CALLBACK: &str = "\u{0}plugin-http:";

/// Decode parked plugin service calls into real HTTP requests.
pub fn drain_plugin_http_requests(
    mut parked: ResMut<PluginServiceCalls>,
    inbox: Option<Res<HttpInbox>>,
) {
    let calls = parked.take(renzora_plugin::http::SERVICE);
    if calls.is_empty() {
        return;
    }
    let Some(inbox) = inbox else {
        warn!("[http] a plugin made a request but this build has no HTTP client");
        return;
    };

    for call in calls {
        let op = HttpOp(call.op);
        // The op selects the payload shape, because it is the one thing both
        // sides agree on before either reads a byte — see `http::WITH_HEADERS`
        // for why this is not a fourth field on `HttpHeader`.
        if op.has_headers() {
            match decode_with_headers(&call.payload) {
                Some((tag, url, body, headers)) => {
                    let callback = format!("{PLUGIN_CALLBACK}{tag}");
                    if op.is_streaming() {
                        inbox.request_stream_with(
                            op.name().to_string(),
                            url,
                            body,
                            callback,
                            headers,
                        );
                    } else {
                        inbox.request_with(op.name().to_string(), url, body, callback, headers);
                    }
                }
                None => warn!("[http] plugin sent a malformed request-with-headers payload"),
            }
            continue;
        }

        let hdr_len = size_of::<HttpHeader>();
        if call.payload.len() < hdr_len {
            warn!("[http] plugin sent {} bytes for a request header", call.payload.len());
            continue;
        }
        // SAFETY: length checked, and `HttpHeader` is `#[repr(C)]` plain data.
        let hdr = unsafe { call.payload.as_ptr().cast::<HttpHeader>().read_unaligned() };

        // Lengths are untrusted — they crossed from another compilation unit,
        // and a bad pair would slice past the end of the payload.
        let url_end = hdr_len.saturating_add(hdr.url_len as usize);
        let body_end = url_end.saturating_add(hdr.body_len as usize);
        // Exact rather than "not past the end": trailing bytes mean the sender
        // and this bridge disagree about the payload's shape, and the header
        // alone cannot tell a longer string from a reordered struct. Unlike the
        // anim and physics commands, the header check above stays a minimum,
        // because this payload genuinely has a variable-length tail.
        if body_end != call.payload.len() {
            warn!(
                "[http] plugin request claims {} + {} bytes but sent {}",
                hdr.url_len,
                hdr.body_len,
                call.payload.len() - hdr_len
            );
            continue;
        }

        let url = String::from_utf8_lossy(&call.payload[hdr_len..url_end]).into_owned();
        let body = if hdr.body_len == 0 {
            None
        } else {
            Some(String::from_utf8_lossy(&call.payload[url_end..body_end]).into_owned())
        };

        if !op.is_known() {
            warn!("[http] plugin used verb {}, which this build does not have", call.op);
            continue;
        }

        let callback = format!("{PLUGIN_CALLBACK}{}", hdr.tag);
        // The verb is the same either way — streaming is a delivery mode, not a
        // different method — so the only thing the op decides is which inbox
        // entry point runs the request.
        if op.is_streaming() {
            inbox.request_stream(op.name().to_string(), url, body, callback);
        } else {
            inbox.request(op.name().to_string(), url, body, callback);
        }
    }
}

/// Decode a `WITH_HEADERS` payload into `(tag, url, body, headers)`.
///
/// Every length is untrusted — they crossed from another compilation unit — so
/// each offset is checked before it is used, and the total must match exactly.
/// Trailing bytes mean the sender and this bridge disagree about the shape, and
/// no header alone can tell a longer string from a reordered struct.
#[allow(clippy::type_complexity)]
fn decode_with_headers(
    payload: &[u8],
) -> Option<(u64, String, Option<String>, Vec<(String, String)>)> {
    let hdr_len = size_of::<HttpRequestHeader>();
    if payload.len() < hdr_len {
        return None;
    }
    // SAFETY: length checked, and `HttpRequestHeader` is `#[repr(C)]` plain data.
    let hdr = unsafe { payload.as_ptr().cast::<HttpRequestHeader>().read_unaligned() };

    let url_end = hdr_len.checked_add(hdr.url_len as usize)?;
    let body_end = url_end.checked_add(hdr.body_len as usize)?;
    let headers_end = body_end.checked_add(hdr.headers_len as usize)?;
    if headers_end != payload.len() {
        return None;
    }

    let url = String::from_utf8_lossy(&payload[hdr_len..url_end]).into_owned();
    let body = (hdr.body_len > 0)
        .then(|| String::from_utf8_lossy(&payload[url_end..body_end]).into_owned());

    // `"Name:value\nName2:value2"` — see `http::HttpHeaders`, which filters the
    // separators out of both halves so a value cannot smuggle in a second header.
    let blob = String::from_utf8_lossy(&payload[body_end..headers_end]).into_owned();
    let headers = blob
        .split('\n')
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            (!k.is_empty()).then(|| (k.to_string(), v.to_string()))
        })
        .collect();

    Some((hdr.tag, url, body, headers))
}

/// Move completed plugin responses from the shared inbox to the plugin one.
///
/// Claims **only** entries carrying the plugin prefix. Scripts and plugins share
/// one client and one queue — there is no reason to run two — but they have
/// separate consumers, and a plain drain by either would swallow the other's
/// responses.
pub fn route_plugin_http_responses(
    inbox: Option<Res<HttpInbox>>,
    mut plugin_inbox: ResMut<PluginHttpInbox>,
) {
    let Some(inbox) = inbox else { return };
    for result in inbox.drain_matching(PLUGIN_CALLBACK) {
        let Some(tag) = result.callback.strip_prefix(PLUGIN_CALLBACK) else {
            continue;
        };
        // A tag that will not parse cannot be routed to anyone. Dropping it with
        // a message beats holding it forever in a queue nothing will match.
        match tag.parse::<u64>() {
            Ok(tag) => plugin_inbox.0.push(PluginHttpResponse {
                tag,
                status: result.status,
                body: result.body,
                // The client's ChunkKind and the ABI's are deliberately separate
                // types — the script client predates the plugin boundary and
                // must keep working in a build with no plugin host — so the
                // mapping happens here, at the one place they meet.
                chunk: result.chunk.map(|k| match k {
                    ChunkKind::Data => sys::HttpChunkKind::Data,
                    ChunkKind::End => sys::HttpChunkKind::End,
                    ChunkKind::Error => sys::HttpChunkKind::Error,
                }),
            }),
            Err(_) => warn!("[http] plugin response has an unparseable tag `{tag}`"),
        }
    }
}

/// Wires both directions up.
pub fn install(app: &mut App) {
    app.init_resource::<PluginHttpInbox>();
    // `PostUpdate`: a plugin's service call is a DEFERRED command and only
    // reaches `PluginServiceCalls` when `Update`'s queue flushes, i.e. after
    // every `Update` system. Draining from `Update` therefore claimed the queue
    // before the call was in it, and the `Last` sweep
    // (`discard_unhandled_service_calls`) binned it before the next frame could
    // look — so calls were silently lost, every frame, with nothing logged.
    app.add_systems(
        PostUpdate,
        (drain_plugin_http_requests, route_plugin_http_responses).chain(),
    );
}

/// Installs the bridge. A plugin so it can be added inline where the shared
/// [`HttpInbox`] is initialised, keeping the two together.
pub struct PluginHttpBridge;

impl Plugin for PluginHttpBridge {
    fn build(&self, app: &mut App) {
        install(app);
    }
}
