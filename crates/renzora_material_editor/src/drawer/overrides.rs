//! Instance overrides: the master's named parameters, edited per instance.
//!
//! These live in the `.material` instance file rather than in ECS data, so they
//! are loaded into [`MatCache`] on selection change, edited there, and written
//! back by [`flush_overrides`] — which also invalidates the resolver so every
//! entity bound to the same file re-renders.

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::inspector::{color_field_rgba, inspector_row};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{checkbox, drag_value};

use renzora_shader::material::codegen::{MaterialParam, ParamKind};
use renzora_shader::material::material_ref::{MaterialRef, ParamValue};
use renzora_shader::material::resolver::{MaterialCache, MaterialResolved};

use crate::material_inspector::{default_param_value, pin_to_param};

use super::slot::icon_btn;
use super::{ov_get, ov_set, MatCache, MatRevertBtn};

pub(super) fn param_row(commands: &mut Commands, fonts: &EmberFonts, param: &MaterialParam) -> Entity {
    let name = param.name.clone();
    let kind = param.kind;
    let default_param = pin_to_param(&param.default).unwrap_or(default_param_value(kind));

    let ctrl = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
        .id();

    let editor = build_param_editor(commands, fonts, name.clone(), kind, default_param);
    let revert = icon_btn(commands, fonts, "arrow-counter-clockwise", "Revert to the master's value");
    commands.entity(revert).insert(MatRevertBtn { name: name.clone() });
    commands.entity(ctrl).add_children(&[editor, revert]);

    inspector_row(commands, &fonts.ui, &param.name, ctrl)
}

fn build_param_editor(commands: &mut Commands, fonts: &EmberFonts, name: String, kind: ParamKind, default_param: ParamValue) -> Entity {
    match kind {
        ParamKind::Float => {
            let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.0, 0.01);
            let (n1, d1) = (name.clone(), default_param.clone());
            bind_2way(
                commands,
                dv,
                move |w| match ov_get(&Rx::new(w.untracked()), &n1, kind, &d1) {
                    ParamValue::Float(f) => f,
                    _ => 0.0,
                },
                move |w, v: &f32| ov_set(w, &name, ParamValue::Float(*v)),
            );
            dv
        }
        ParamKind::Bool => {
            let cb = checkbox(commands, false);
            let (n1, d1) = (name.clone(), default_param.clone());
            bind_2way(
                commands,
                cb,
                move |w| matches!(ov_get(&Rx::new(w.untracked()), &n1, kind, &d1), ParamValue::Bool(true)),
                move |w, v: &bool| ov_set(w, &name, ParamValue::Bool(*v)),
            );
            cb
        }
        ParamKind::Color => {
            let n1 = name.clone();
            let d1 = default_param.clone();
            color_field_rgba(
                commands,
                move |w| match ov_get(&Rx::new(w.untracked()), &n1, kind, &d1) {
                    ParamValue::Color(c) => c,
                    _ => [1.0; 4],
                },
                move |w, a: [f32; 4]| ov_set(w, &name, ParamValue::Color(a)),
            )
        }
        ParamKind::Vec2 | ParamKind::Vec3 | ParamKind::Vec4 => {
            let n = match kind {
                ParamKind::Vec2 => 2,
                ParamKind::Vec3 => 3,
                _ => 4,
            };
            let group = commands
                .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), flex_grow: 1.0, ..default() })
                .id();
            let axes = ["x", "y", "z", "w"];
            let mut cells = Vec::new();
            for (i, axis) in axes.iter().enumerate().take(n) {
                let dv = drag_value(commands, &fonts.ui, axis, (210, 210, 220), 0.0, 0.01);
                let (n1, d1) = (name.clone(), default_param.clone());
                let (n2, kind2) = (name.clone(), kind);
                bind_2way(
                    commands,
                    dv,
                    move |w| vec_component(&ov_get(&Rx::new(w.untracked()), &n1, kind, &d1), i),
                    move |w, v: &f32| {
                        let cur = ov_get(&Rx::new(&*w), &n2, kind2, &default_param_value(kind2));
                        let updated = set_vec_component(cur, kind2, i, *v);
                        ov_set(w, &n2, updated);
                    },
                );
                cells.push(dv);
            }
            commands.entity(group).add_children(&cells);
            group
        }
    }
}

fn vec_component(v: &ParamValue, i: usize) -> f32 {
    match v {
        ParamValue::Vec2(a) => *a.get(i).unwrap_or(&0.0),
        ParamValue::Vec3(a) => *a.get(i).unwrap_or(&0.0),
        ParamValue::Vec4(a) => *a.get(i).unwrap_or(&0.0),
        _ => 0.0,
    }
}

fn set_vec_component(mut v: ParamValue, kind: ParamKind, i: usize, val: f32) -> ParamValue {
    match (&mut v, kind) {
        (ParamValue::Vec2(a), ParamKind::Vec2) => {
            if i < 2 {
                a[i] = val;
            }
        }
        (ParamValue::Vec3(a), ParamKind::Vec3) => {
            if i < 3 {
                a[i] = val;
            }
        }
        (ParamValue::Vec4(a), ParamKind::Vec4) => {
            if i < 4 {
                a[i] = val;
            }
        }
        _ => {
            // Type drifted (override stored a different kind) — reset to the kind's default.
            let mut d = default_param_value(kind);
            d = set_vec_component(d, kind, i, val);
            return d;
        }
    }
    v
}

pub(super) fn mat_revert_click(q: Query<(&Interaction, &MatRevertBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let name = b.name.clone();
        commands.queue(move |w: &mut World| {
            if let Some(mut cache) = w.get_resource_mut::<MatCache>() {
                if let Some(inst) = &mut cache.instance {
                    if inst.overrides.remove(&name).is_some() {
                        cache.dirty = true;
                    }
                }
            }
        });
    }
}

/// Write the edited overrides back to disk + invalidate the resolver so every
/// entity bound to this `.material` re-renders.
pub(super) fn flush_overrides(world: &mut World) {
    let dirty = world.get_resource::<MatCache>().map(|c| c.dirty).unwrap_or(false);
    if !dirty {
        return;
    }
    let (instance, instance_abs, asset_path) = {
        let cache = world.resource::<MatCache>();
        (cache.instance.clone(), cache.instance_abs.clone(), cache.path.clone())
    };
    world.resource_mut::<MatCache>().dirty = false;
    let Some(inst) = instance else { return };

    if let Ok(json) = serde_json::to_string_pretty(&inst) {
        if let Err(e) = std::fs::write(&instance_abs, json) {
            bevy::log::warn!("[material] couldn't write {}: {}", instance_abs.display(), e);
            return;
        }
    }
    if let Some(mut cache) = world.get_resource_mut::<MaterialCache>() {
        cache.invalidate(&asset_path);
    }
    let mut to_invalidate: Vec<Entity> = Vec::new();
    let mut q = world.query::<(Entity, &MaterialRef)>();
    for (e, mr) in q.iter(world) {
        if mr.0 == asset_path {
            to_invalidate.push(e);
        }
    }
    for e in to_invalidate {
        world.entity_mut(e).remove::<MaterialResolved>();
    }
}
