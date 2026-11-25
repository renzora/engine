//! An animated data source for the vector component dashboard. The template
//! binds `{{ _speed }}`, `{{ _rpm }}`, `{{ _temp }}`, `{{ _fuel }}`,
//! `{{ _bars }}`, `{{ _line }}` and `{{ _wave }}`, which read this script's
//! variables on the entity, so they are published there every frame and the
//! gauges and charts animate live. Series are comma-separated numbers.
//!
//! Setup: attach to the same entity that holds the HtmlTemplate for
//! `ui/vector_lib_demo.html`.

use bevy::prelude::*;
use renzora::component::ScriptComponent;
use renzora::{ScriptCtx, ScriptValue};
use std::collections::VecDeque;

/// Samples kept in the rolling line chart.
const LINE_LEN: usize = 24;
const BAR_COUNT: usize = 8;
const WAVE_LEN: usize = 32;

#[derive(Component, Default)]
struct Dashboard {
    t: f32,
    /// Rolling history for the line chart, newest last.
    line: VecDeque<f32>,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Dashboard>() {
        ctx.insert(Dashboard::default());
    }
    let Some(mut dash) = ctx.get_mut::<Dashboard>() else { return };
    dash.t += dt;
    let t = dash.t;
    dash.line.push_back(0.5 + 0.4 * (t * 1.5).sin());
    while dash.line.len() > LINE_LEN {
        dash.line.pop_front();
    }
    let line = series(dash.line.iter().copied());
    drop(dash);

    // An eased 0..1 sweep.
    let s = 0.5 - 0.5 * (t * 0.5).cos();
    let bars = series((1..=BAR_COUNT).map(|i| 0.2 + 0.7 * (t * 1.2 + i as f32 * 0.6).sin().abs()));
    let wave = series((1..=WAVE_LEN).map(|i| (t * 4.0 + i as f32 * 0.5).sin().abs() * (0.3 + 0.6 * s)));
    let values = [
        ("_speed", ScriptValue::Float((20.0 + s * 210.0 + 0.5).floor())),
        ("_rpm", ScriptValue::Float(1.0 + s * 7.0)),
        ("_temp", ScriptValue::Float((55.0 + 35.0 * (t * 0.4).sin() + 0.5).floor())),
        ("_fuel", ScriptValue::Float(0.5 + 0.45 * (t * 0.2).sin())),
        ("_bars", ScriptValue::String(bars)),
        ("_line", ScriptValue::String(line)),
        ("_wave", ScriptValue::String(wave)),
    ];

    let Some(mut scripts) = ctx.get_mut::<ScriptComponent>() else { return };
    for entry in scripts.scripts.iter_mut() {
        if entry.script_path.as_ref().is_some_and(|p| p.ends_with("vector_anim.rs")) {
            for (name, value) in &values {
                entry.variables.set(*name, value.clone());
            }
        }
    }
}

fn series(values: impl Iterator<Item = f32>) -> String {
    values.map(|v| format!("{v:.3}")).collect::<Vec<_>>().join(",")
}

renzora::script!(update);
