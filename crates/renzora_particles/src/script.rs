//! Script control: `vfx_play`, `vfx_pause`, `vfx_stop`, `vfx_restart`,
//! `vfx_burst(count)`, `vfx_set_time_scale(scale)`, and the effect's exposed
//! parameters: `vfx_set_param(name, value)`, `vfx_set_param_vec3(name, x, y,
//! z)`, `vfx_set_param_color(name, r, g, b, a)`, `vfx_clear_param(name)`, and
//! the `_on(entity, ...)` forms of the three setters.
//!
//! Declared, not written: [`ParticleScriptExtension`] describes each function
//! and every language backend builds it, firing a [`renzora::ScriptAction`]
//! that [`handle_script_actions`] observes. The declaration lives in the
//! contract crate's `script_extension` module, so this crate links no
//! interpreter and does not depend on `renzora_scripting` at all.
//!
//! The `vfx_` prefix keeps these apart from the Hanabi-backed particle commands
//! while both systems exist.
//!
//! A call acts on the script's own entity when it has a [`ParticlePlayback`],
//! and otherwise on its direct children that do: a script on a character can
//! fire the dust effect parented under its feet without looking it up. The
//! `_on` forms name the entity instead (by its name, as the hierarchy shows
//! it), and so does an action whose `target_entity` is set; an empty name is
//! the script's own entity, as above.
//!
//! A parameter is a Parameter node in the effect's graph, found by its Name.
//! Its value goes into the entity's [`ParticleParams`], which the runtime
//! copies into the simulation every frame, so it holds across a hot reload
//! and applies to every emitter with a node of that name.

use bevy::prelude::*;
use renzora::script_extension::{Bind, Binding, ParamKind, ScriptExtension};
use renzora::{ScriptAction, ScriptActionValue};

use crate::component::{ParticleParams, ParticlePlayback};

pub struct ParticleScriptExtension;

impl ScriptExtension for ParticleScriptExtension {
    fn name(&self) -> &str {
        "particles"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Bind::action("vfx_play", "vfx_play")
                .doc("Play or resume the particle effect.")
                .build(),
            Bind::action("vfx_pause", "vfx_pause")
                .doc("Freeze the particle effect where it is.")
                .build(),
            Bind::action("vfx_stop", "vfx_stop")
                .doc("Clear every particle and stop emitting.")
                .build(),
            Bind::action("vfx_restart", "vfx_restart")
                .doc("Clear every particle and play from the beginning.")
                .build(),
            Bind::action("vfx_burst", "vfx_burst")
                .arg("count", ParamKind::Int)
                .doc("Spawn extra particles from every emitter right now.")
                .build(),
            Bind::action("vfx_set_time_scale", "vfx_set_time_scale")
                .arg("scale", ParamKind::Float)
                .doc("Play faster (above 1) or slower (below 1).")
                .build(),
            Bind::action("vfx_set_param", "vfx_set_param")
                .arg("name", ParamKind::Str)
                .arg("value", ParamKind::Float)
                .doc("Set the effect's Parameter nodes called name to a number.")
                .build(),
            Bind::action("vfx_set_param_vec3", "vfx_set_param_vec3")
                .arg("name", ParamKind::Str)
                .vec3("value")
                .doc("Set the effect's Parameter nodes called name to a vector.")
                .build(),
            Bind::action("vfx_set_param_color", "vfx_set_param_color")
                .arg("name", ParamKind::Str)
                .arg("r", ParamKind::Float)
                .arg("g", ParamKind::Float)
                .arg("b", ParamKind::Float)
                .arg("a", ParamKind::Float)
                .doc("Set the effect's Parameter nodes called name to a linear RGBA colour.")
                .build(),
            Bind::action("vfx_clear_param", "vfx_clear_param")
                .arg("name", ParamKind::Str)
                .doc("Give the Parameter nodes called name back their own value.")
                .build(),
            Bind::action("vfx_set_param_on", "vfx_set_param")
                .arg("entity", ParamKind::Str)
                .arg("name", ParamKind::Str)
                .arg("value", ParamKind::Float)
                .doc("vfx_set_param on the named entity's effect.")
                .build(),
            Bind::action("vfx_set_param_vec3_on", "vfx_set_param_vec3")
                .arg("entity", ParamKind::Str)
                .arg("name", ParamKind::Str)
                .vec3("value")
                .doc("vfx_set_param_vec3 on the named entity's effect.")
                .build(),
            Bind::action("vfx_set_param_color_on", "vfx_set_param_color")
                .arg("entity", ParamKind::Str)
                .arg("name", ParamKind::Str)
                .arg("r", ParamKind::Float)
                .arg("g", ParamKind::Float)
                .arg("b", ParamKind::Float)
                .arg("a", ParamKind::Float)
                .doc("vfx_set_param_color on the named entity's effect.")
                .build(),
        ]
    }
}

fn number(action: &ScriptAction, key: &str) -> Option<f64> {
    match action.args.get(key) {
        Some(ScriptActionValue::Float(f)) => Some(*f as f64),
        Some(ScriptActionValue::Int(i)) => Some(*i as f64),
        _ => None,
    }
}

fn text<'a>(action: &'a ScriptAction, key: &str) -> Option<&'a str> {
    match action.args.get(key) {
        Some(ScriptActionValue::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The value a parameter call carries, as a slot's four lanes
/// ([`ParticleParams`]): a number fills every lane, a vector gets alpha 1,
/// and a colour is its four components. `None` for a call missing its
/// value or carrying a non-finite one.
fn param_value(action: &ScriptAction) -> Option<Vec4> {
    let lanes = match action.name.as_str() {
        "vfx_set_param" => Vec4::splat(number(action, "value")? as f32),
        "vfx_set_param_vec3" => match action.args.get("value") {
            Some(ScriptActionValue::Vec3(v)) => Vec3::from_array(*v).extend(1.0),
            // A number for a vector fills it, as a Float into a vector pin does.
            _ => Vec4::splat(number(action, "value")? as f32),
        },
        "vfx_set_param_color" => {
            let lane = |k: &str| number(action, k).map(|v| v as f32);
            Vec4::new(lane("r")?, lane("g")?, lane("b")?, lane("a").unwrap_or(1.0))
        }
        _ => return None,
    };
    lanes.is_finite().then_some(lanes)
}

pub fn handle_script_actions(
    trigger: On<ScriptAction>,
    mut commands: Commands,
    mut playbacks: Query<&mut ParticlePlayback>,
    mut params: Query<&mut ParticleParams>,
    children: Query<&Children>,
    names: Query<(Entity, &Name)>,
) {
    let action = trigger.event();
    if !action.name.starts_with("vfx_") {
        return;
    }
    // Whose effect: a named entity (the `_on` forms, or the action's own
    // target), else the script's.
    let named = text(action, "entity").filter(|n| !n.trim().is_empty()).or(action.target_entity.as_deref());
    let owner = match named {
        Some(name) => {
            let name = name.trim();
            match names.iter().find(|(_, n)| n.as_str() == name) {
                Some((e, _)) => e,
                None => {
                    renzora::console_log::console_warn(
                        "Particles",
                        format!("{}: no entity is named `{name}`", action.name),
                    );
                    return;
                }
            }
        }
        None => action.entity,
    };
    let mut targets: Vec<Entity> = Vec::new();
    if playbacks.contains(owner) {
        targets.push(owner);
    } else if let Ok(kids) = children.get(owner) {
        targets.extend(kids.iter().filter(|c| playbacks.contains(*c)));
    }
    for target in targets {
        match action.name.as_str() {
            "vfx_set_param" | "vfx_set_param_vec3" | "vfx_set_param_color" | "vfx_clear_param" => {
                let Some(name) = text(action, "name").map(str::trim).filter(|n| !n.is_empty()) else {
                    continue;
                };
                let value = if action.name == "vfx_clear_param" {
                    None
                } else {
                    match param_value(action) {
                        Some(v) => Some(v),
                        None => continue,
                    }
                };
                match (params.get_mut(target), value) {
                    (Ok(mut p), Some(v)) => p.set(name, v),
                    (Ok(mut p), None) => p.clear(name),
                    // Through the entry, not a plain insert: two calls before
                    // the component exists must both land, not the second
                    // replace the first.
                    (Err(_), Some(v)) => {
                        let name = name.to_string();
                        commands
                            .entity(target)
                            .entry::<ParticleParams>()
                            .or_default()
                            .and_modify(move |mut p| p.set(name, v));
                    }
                    (Err(_), None) => {}
                }
                continue;
            }
            _ => {}
        }
        let Ok(mut playback) = playbacks.get_mut(target) else {
            continue;
        };
        match action.name.as_str() {
            "vfx_play" => playback.play(),
            "vfx_pause" => playback.pause(),
            "vfx_stop" => playback.stop(),
            "vfx_restart" => playback.restart(),
            "vfx_burst" => {
                let count = number(action, "count").unwrap_or(0.0).clamp(0.0, u32::MAX as f64);
                playback.burst(count as u32);
            }
            "vfx_set_time_scale" => {
                if let Some(scale) = number(action, "scale").filter(|s| s.is_finite()) {
                    playback.time_scale = (scale as f32).max(0.0);
                }
            }
            _ => {}
        }
    }
}
