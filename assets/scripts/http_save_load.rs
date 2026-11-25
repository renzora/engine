//! Cloud save and load: post this entity's pose to a server as JSON, and
//! restore it on start. Persistence needs a snapshot of live component data,
//! so the Transform is encoded on save and written back on load. A status of 0
//! means the request never completed, and is reported rather than parsed, so a
//! failed fetch cannot corrupt the pose.
//!
//! Setup: attach to a saveable entity with a name; saves are keyed by that name
//! on the server. Saves every 30 seconds and on F5.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::serde_json::{json, Value};
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const BASE: &str = "https://example.com/saves/";
/// Seconds between autosaves.
const AUTOSAVE: f32 = 30.0;

#[derive(Component)]
struct Autosave(f32);

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let manual = ctx.get_resource::<ScriptInput>().is_some_and(|i| i.is_key_just_pressed(KeyCode::F5));
    let Some(mut timer) = ctx.get_mut::<Autosave>() else { return };
    timer.0 -= dt;
    let due = timer.0 <= 0.0;
    if due {
        timer.0 += AUTOSAVE;
    }
    drop(timer);
    if due || manual {
        save(ctx);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            ctx.insert(Autosave(AUTOSAVE));
            let me = ctx.entity();
            let url = url(ctx);
            ScriptAction::new("http_get", me).arg("url", url).arg("callback", "load").send(ctx.world());
        }
        ScriptHook::Http { callback, status, body } => {
            if *callback != "load" && *callback != "save" {
                return;
            }
            let label = ctx.name().unwrap_or("?").to_string();
            if *status == 0 {
                renzora::clog_warn!("Script", "[save] {} failed: {}", callback, body);
                return;
            }
            if *callback == "save" {
                renzora::clog_info!("Script", "[save] persisted {}", label);
                return;
            }
            let Ok(d) = renzora::serde_json::from_str::<Value>(body) else { return };
            let num = |k: &str| d.get(k).and_then(Value::as_f64).map(|v| v as f32);
            let Some(mut t) = ctx.get_mut::<Transform>() else { return };
            let p = t.translation;
            t.translation = Vec3::new(num("x").unwrap_or(p.x), num("y").unwrap_or(p.y), num("z").unwrap_or(p.z));
            let (rx, ry, rz) = (num("rx").unwrap_or(0.0), num("ry").unwrap_or(0.0), num("rz").unwrap_or(0.0));
            t.rotation = Quat::from_euler(EulerRot::YXZ, ry.to_radians(), rx.to_radians(), rz.to_radians());
            drop(t);
            renzora::clog_info!("Script", "[save] restored {}", label);
        }
        _ => {}
    }
}

fn url(ctx: &ScriptCtx) -> String {
    format!("{BASE}{}", ctx.name().unwrap_or("unnamed"))
}

fn save(ctx: &mut ScriptCtx) {
    let Some(t) = ctx.get::<Transform>() else { return };
    let p = t.translation;
    let (y, x, z) = t.rotation.to_euler(EulerRot::YXZ);
    let body = json!({
        "x": p.x, "y": p.y, "z": p.z,
        "rx": x.to_degrees(), "ry": y.to_degrees(), "rz": z.to_degrees(),
    })
    .to_string();
    let me = ctx.entity();
    let url = url(ctx);
    ScriptAction::new("http_post", me)
        .arg("url", url)
        .arg("body", body)
        .arg("callback", "save")
        .send(ctx.world());
}

renzora::script!(update, hooks = hooks);
