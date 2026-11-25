//! A batched telemetry uploader. Posting one event per frame would hammer the
//! endpoint, so samples are buffered and flushed on an interval as one JSON
//! body. The `Http` hook confirms the batch, or on status 0 (the request never
//! completed) puts it back in the buffer, so a transient network blip loses no
//! data.
//!
//! A failed batch is re-queued as its individual events, where the Lua
//! original nested it as one blob inside the next batch.
//!
//! Setup: set `URL`. The sample metric is how far this entity moved each
//! frame; each peer reports on its own.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::serde_json::{json, Value};
use renzora::{ScriptCtx, ScriptHook};

const URL: &str = "https://example.com/api/telemetry";
/// Seconds between uploads.
const FLUSH: f32 = 5.0;
const CALLBACK: &str = "telemetry";
/// Movement below this per frame is not worth a sample.
const MIN_STEP: f32 = 0.01;

#[derive(Component)]
struct Telemetry {
    buffer: Vec<Value>,
    /// The batch awaiting its response, kept so a failure can re-queue it.
    inflight: Vec<Value>,
    last: Vec3,
    flush: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let elapsed = ctx.elapsed();
    let Some(here) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let Some(mut tele) = ctx.get_mut::<Telemetry>() else { return };
    let dist = (here - tele.last).xz().length();
    if dist > MIN_STEP {
        tele.buffer.push(json!({ "t": round(elapsed, 100.0), "k": "move", "v": round(dist, 1000.0) }));
    }
    tele.last = here;

    tele.flush -= dt;
    if tele.flush > 0.0 {
        return;
    }
    tele.flush += FLUSH;
    // Holding one batch at a time keeps a slow endpoint from piling requests.
    if tele.buffer.is_empty() || !tele.inflight.is_empty() {
        return;
    }
    let batch = std::mem::take(&mut tele.buffer);
    tele.inflight = batch.clone();
    drop(tele);

    let session = ctx.name().unwrap_or("unnamed").to_string();
    let body = json!({ "session": session, "events": batch }).to_string();
    let me = ctx.entity();
    ScriptAction::new("http_post", me)
        .arg("url", URL)
        .arg("body", body)
        .arg("callback", CALLBACK)
        .send(ctx.world());
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            let last = ctx.get::<Transform>().map_or(Vec3::ZERO, |t| t.translation);
            ctx.insert(Telemetry { buffer: Vec::new(), inflight: Vec::new(), last, flush: FLUSH });
        }
        ScriptHook::Http { callback: CALLBACK, status, .. } => {
            let Some(mut tele) = ctx.get_mut::<Telemetry>() else { return };
            let sent = std::mem::take(&mut tele.inflight);
            if *status == 0 {
                // Older samples first, so the stream stays in time order.
                let newer = std::mem::replace(&mut tele.buffer, sent);
                tele.buffer.extend(newer);
            }
        }
        _ => {}
    }
}

/// `v` rounded to `1 / scale`, as the Lua `%.2f` / `%.3f` formatting did.
fn round(v: f32, scale: f32) -> f64 {
    (f64::from(v) * f64::from(scale)).round() / f64::from(scale)
}

renzora::script!(update, hooks = hooks);
