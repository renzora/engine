//! An asynchronous leaderboard fetch: `http_get`, then the `Http` hook, then
//! JSON, then the UI. The response only arrives in the hook, so each request is
//! tagged with a callback name and the parsing keys on it. A status of 0 means
//! the request never completed (the body holds the error), so it degrades to a
//! message rather than parsing garbage.
//!
//! Setup: point `URL` at a JSON endpoint returning
//! `{"entries": [{"name": "...", "score": 0}]}`. A UI text named "lb_status"
//! shows the leader.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::serde_json::Value;
use renzora::{ScriptCtx, ScriptHook};

const URL: &str = "https://example.com/api/leaderboard";
/// Seconds between refreshes.
const REFRESH: f32 = 30.0;
const CALLBACK: &str = "leaderboard";

#[derive(Component)]
struct Refresh(f32);

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(mut refresh) = ctx.get_mut::<Refresh>() else { return };
    refresh.0 -= dt;
    if refresh.0 > 0.0 {
        return;
    }
    refresh.0 = REFRESH;
    drop(refresh);
    request(ctx);
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            ctx.insert(Refresh(REFRESH));
            request(ctx);
        }
        ScriptHook::Http { callback: CALLBACK, status, body } => {
            let text = if *status == 0 {
                format!("fetch failed: {body}")
            } else {
                leader(body).unwrap_or_else(|| "bad payload".to_string())
            };
            let me = ctx.entity();
            ScriptAction::new("ui_set_text", me).arg("name", "lb_status").arg("text", text).send(ctx.world());
        }
        _ => {}
    }
}

fn request(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    ScriptAction::new("http_get", me).arg("url", URL).arg("callback", CALLBACK).send(ctx.world());
}

/// "#1 name (score)" for the highest score, without assuming the server sorts.
fn leader(body: &str) -> Option<String> {
    let data: Value = renzora::serde_json::from_str(body).ok()?;
    let best = data
        .get("entries")?
        .as_array()?
        .iter()
        .filter_map(|e| Some((e.get("score")?.as_f64()?, e.get("name").and_then(Value::as_str).unwrap_or("?"))))
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    Some(format!("#1 {} ({})", best.1, best.0 as i64))
}

renzora::script!(update, hooks = hooks);
