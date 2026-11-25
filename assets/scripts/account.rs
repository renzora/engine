//! A renzora.com login driven by a UI form. The form's `<input>` fields bind to
//! this script's `email` and `password` variables; the "Log in" button fires
//! the `submit_login` callback, which POSTs them and stores the username.
//!
//! A Rust script has no variables of its own, so this one publishes
//! `email`, `password`, `username` and `login_status` into its own entry of the
//! entity's `ScriptComponent`. That is what `<input bind=...>` writes into and
//! what `{{ account.login_status }}` reads.
//!
//! Setup: put this on an entity named "account" (the name `ui/login_form.html`
//! binds to), and give a UiCanvas that template.

use bevy::prelude::*;
use renzora::component::ScriptComponent;
use renzora::script_bridge::ScriptAction;
use renzora::serde_json::Value;
use renzora::{ScriptCtx, ScriptHook, ScriptValue};

const SCRIPT_FILE: &str = "account.rs";
const LOGIN_URL: &str = "https://renzora.com/api/auth/login";
const CALLBACK: &str = "login";

fn update(_ctx: &mut ScriptCtx) {}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            // Declared up front so the inputs write into this script's entry
            // rather than whichever script happens to be first on the entity.
            for var in ["email", "password", "username"] {
                if read_var(ctx, var).is_none() {
                    publish(ctx, var, String::new());
                }
            }
            publish(ctx, "login_status", "idle".to_string());
        }
        ScriptHook::Ui { name: "submit_login", .. } => {
            let email = read_var(ctx, "email").unwrap_or_default();
            let password = read_var(ctx, "password").unwrap_or_default();
            if email.is_empty() || password.is_empty() {
                publish(ctx, "login_status", "enter email + password".to_string());
                return;
            }
            publish(ctx, "login_status", "logging in...".to_string());
            // Built with serde_json, not by concatenation, so a quote in the
            // password cannot break the body.
            let body = renzora::serde_json::json!({ "email": email, "password": password }).to_string();
            let me = ctx.entity();
            ScriptAction::new("http_post", me)
                .arg("url", LOGIN_URL)
                .arg("body", body)
                .arg("callback", CALLBACK)
                .send(ctx.world());
        }
        ScriptHook::Http { callback: CALLBACK, status, body } => {
            if *status != 200 {
                publish(ctx, "login_status", format!("failed ({status})"));
                renzora::clog_info!("Script", "[account] login failed {status}: {body}");
                return;
            }
            let username = renzora::serde_json::from_str::<Value>(body)
                .ok()
                .and_then(|data| data.get("user")?.get("username")?.as_str().map(str::to_string));
            match username {
                Some(username) => {
                    renzora::clog_info!("Script", "[account] logged in as {username}");
                    publish(ctx, "username", username);
                    publish(ctx, "login_status", "online".to_string());
                }
                None => publish(ctx, "login_status", "ok but no username in response".to_string()),
            }
        }
        _ => {}
    }
}

/// This script's own variable, as text.
fn read_var(ctx: &mut ScriptCtx, name: &str) -> Option<String> {
    let scripts = ctx.get::<ScriptComponent>()?;
    scripts
        .scripts
        .iter()
        .filter(|entry| entry.script_path.as_ref().is_some_and(|p| p.ends_with(SCRIPT_FILE)))
        .find_map(|entry| entry.variables.get_string(name).map(str::to_string))
}

/// Write a variable into this script's entry, where the markup reads it.
fn publish(ctx: &mut ScriptCtx, name: &str, value: String) {
    let Some(mut scripts) = ctx.get_mut::<ScriptComponent>() else { return };
    for entry in scripts.scripts.iter_mut() {
        if entry.script_path.as_ref().is_some_and(|p| p.ends_with(SCRIPT_FILE)) {
            entry.variables.set(name, ScriptValue::String(value.clone()));
        }
    }
}

renzora::script!(update, hooks = hooks);
