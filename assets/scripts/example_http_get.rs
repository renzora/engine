//! Fetches an endpoint once and exposes the response for UI binding as
//! `{{ EntityName.result }}`. The reply only arrives in the `Http` hook, tagged
//! with the callback name the request carried; a status of 0 means the request
//! never completed.
//!
//! A Rust script has no variables of its own, so `result` is published into
//! this script's entry of the entity's `ScriptComponent`, which is what the
//! markup binding reads.
//!
//! Setup: name the entity (for example "api"), then in a template:
//! `<text>{{ api.result }}</text>`.

use bevy::prelude::*;
use renzora::component::ScriptComponent;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptValue};

const SCRIPT_FILE: &str = "example_http_get.rs";
const URL: &str = "https://renzora.com/api/health";
const CALLBACK: &str = "fetch";

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            publish(ctx, "(loading)".to_string());
            let me = ctx.entity();
            ScriptAction::new("http_get", me).arg("url", URL).arg("callback", CALLBACK).send(ctx.world());
        }
        ScriptHook::Http { callback: CALLBACK, status, body } => {
            // The raw body; to pull one field instead, parse it with
            // `renzora::serde_json::from_str::<Value>(body)` and read `["status"]`.
            let result = if *status == 200 { body.to_string() } else { format!("error {status}") };
            publish(ctx, result);
        }
        _ => {}
    }
}

fn publish(ctx: &mut ScriptCtx, result: String) {
    let Some(mut scripts) = ctx.get_mut::<ScriptComponent>() else { return };
    for entry in scripts.scripts.iter_mut() {
        if entry.script_path.as_ref().is_some_and(|p| p.ends_with(SCRIPT_FILE)) {
            entry.variables.set("result", ScriptValue::String(result.clone()));
        }
    }
}

renzora::script!(update, hooks = hooks);
