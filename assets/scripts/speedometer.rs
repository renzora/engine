//! Drives the standalone speedometer demo: an eased 0..220 km/h sweep. The
//! template binds `{{ _speed }}`, which reads this script's variables on the
//! entity, so the value is published there every frame and the needle and
//! readout animate live.
//!
//! Setup: attach to the same entity that holds the HtmlTemplate for
//! `ui/speedometer_demo.html`.

use bevy::prelude::*;
use renzora::component::ScriptComponent;
use renzora::{ScriptCtx, ScriptValue};

const TOP_SPEED: f32 = 220.0;

fn update(ctx: &mut ScriptCtx) {
    let t = ctx.elapsed();
    let s = 0.5 - 0.5 * (t * 0.5).cos();
    let speed = (s * TOP_SPEED + 0.5).floor();
    let Some(mut scripts) = ctx.get_mut::<ScriptComponent>() else { return };
    for entry in scripts.scripts.iter_mut() {
        if entry.script_path.as_ref().is_some_and(|p| p.ends_with("speedometer.rs")) {
            entry.variables.set("_speed", ScriptValue::Float(speed));
        }
    }
}

renzora::script!(update);
