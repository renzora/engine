//! Bevy-native (ember) port of the egui `ShaderPropertiesPanel`: the exposed
//! `@param` editors, grouped by type (Float / Color / Vector / Integer /
//! Boolean) into sections, each row a labelled live editor (scrubbable number
//! field, colour swatch, or checkbox) that writes the value back into the
//! shader file and re-applies it to the preview material.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field, inspector_stripe};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{checkbox, drag_value, DragRange};
use renzora_shader::file::{ParamType, ParamValue, ShaderParam};

use crate::code_panel::reapply_params;
use crate::ShaderEditorState;

const LABEL_W: f32 = 96.0;
const AXES: [(&str, (u8, u8, u8)); 4] =
    [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230)), ("W", (200, 200, 90))];

pub struct NativeShaderProperties;

impl Plugin for NativeShaderProperties {
    fn build(&self, app: &mut App) {
        app.register_panel_content("shader_properties", true, build);
    }
}

fn no_params(w: &World) -> bool {
    w.get_resource::<ShaderEditorState>().is_none_or(|s| s.shader_file.params.is_empty())
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-shader-properties"),
        ))
        .id();

    // Empty state.
    let note = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(3.0), padding: UiRect::vertical(Val::Px(20.0)), ..default() })
        .id();
    let n1 = commands.spawn((Text::new("No parameters defined"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    let n2 = commands.spawn((Text::new("Add // @param annotations to your shader"), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder())))).id();
    commands.entity(note).add_children(&[n1, n2]);
    bind_display(commands, note, no_params);

    // Param list.
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    bind_display(commands, list, |w| !no_params(w));
    keyed_list(commands, list, props_snapshot);

    commands.entity(root).add_children(&[note, list]);
    root
}

/// One row of the flattened, grouped param list.
#[derive(Clone)]
enum Item {
    Header(&'static str, &'static str),
    Param(String, ShaderParam),
}

fn group_of(t: &ParamType) -> usize {
    match t {
        ParamType::Float => 0,
        ParamType::Color => 1,
        ParamType::Vec2 | ParamType::Vec3 | ParamType::Vec4 => 2,
        ParamType::Int => 3,
        ParamType::Bool => 4,
    }
}

const GROUPS: [(&str, &str); 5] = [
    ("Float", "sliders-horizontal"),
    ("Color", "palette"),
    ("Vector", "arrows-out-cardinal"),
    ("Integer", "hash"),
    ("Boolean", "toggle-left"),
];

fn props_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ShaderEditorState>() else { return empty() };
    let mut params: Vec<(String, ShaderParam)> =
        state.shader_file.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    params.sort_by(|a, b| a.0.cmp(&b.0));

    let mut items_data: Vec<Item> = Vec::new();
    for (gi, (label, icon)) in GROUPS.iter().enumerate() {
        let group: Vec<&(String, ShaderParam)> =
            params.iter().filter(|(_, p)| group_of(&p.param_type) == gi).collect();
        if group.is_empty() {
            continue;
        }
        items_data.push(Item::Header(label, icon));
        for (n, p) in group {
            items_data.push(Item::Param(n.clone(), p.clone()));
        }
    }

    let items: Vec<(u64, u64)> = items_data
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match it {
                // Hash structure only (NOT the value) so live edits don't rebuild the row.
                Item::Header(l, _) => (0u8, *l).hash(&mut h),
                Item::Param(n, p) => {
                    (1u8, n, type_disc(&p.param_type), p.min.map(|x| x.to_bits()), p.max.map(|x| x.to_bits())).hash(&mut h)
                }
            }
            (k.finish(), h.finish())
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &items_data[i] {
            Item::Header(label, icon) => section_header(c, f, label, icon),
            Item::Param(name, p) => param_row(c, f, i, name, p),
        }),
    }
}

fn type_disc(t: &ParamType) -> u8 {
    match t {
        ParamType::Float => 0,
        ParamType::Vec2 => 1,
        ParamType::Vec3 => 2,
        ParamType::Vec4 => 3,
        ParamType::Color => 4,
        ParamType::Int => 5,
        ParamType::Bool => 6,
    }
}

fn section_header(commands: &mut Commands, fonts: &EmberFonts, label: &str, icon: &str) -> Entity {
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(22.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 11.0);
    let lbl = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    commands.entity(row).add_children(&[ic, lbl]);
    row
}

fn param_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, param: &ShaderParam) -> Entity {
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), min_height: Val::Px(24.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..default() },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
        ))
        .id();
    let editor = editor(commands, fonts, name, param);
    commands.entity(row).add_children(&[label, editor]);
    row
}

/// The right-hand editor cell for a param (flex-grow, right-aligned content).
fn editor(commands: &mut Commands, fonts: &EmberFonts, name: &str, param: &ShaderParam) -> Entity {
    let cell = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(3.0), ..default() })
        .id();

    match param.param_type {
        ParamType::Float => {
            let init = match param.default_value {
                ParamValue::Float(v) => v,
                _ => 0.0,
            };
            let min = param.min.unwrap_or(0.0);
            let max = param.max.unwrap_or(1.0);
            let step = ((max - min) / 100.0).max(0.001);
            let n = name.to_string();
            let field = num_field(commands, fonts, "", value_text(), init, step, min, max, move |w, v| set_param(w, &n, ParamValue::Float(*v)), {
                let n = name.to_string();
                move |w| match param_value(w, &n) {
                    Some(ParamValue::Float(v)) => v,
                    _ => 0.0,
                }
            });
            commands.entity(cell).add_child(field);
        }
        ParamType::Int => {
            let init = match param.default_value {
                ParamValue::Int(v) => v as f32,
                _ => 0.0,
            };
            let n = name.to_string();
            let field = num_field(commands, fonts, "", value_text(), init, 1.0, 0.0, 0.0, move |w, v| set_param(w, &n, ParamValue::Int(v.round() as i32)), {
                let n = name.to_string();
                move |w| match param_value(w, &n) {
                    Some(ParamValue::Int(v)) => v as f32,
                    _ => 0.0,
                }
            });
            commands.entity(cell).add_child(field);
        }
        ParamType::Vec2 | ParamType::Vec3 | ParamType::Vec4 => {
            let comps = match param.param_type {
                ParamType::Vec2 => 2,
                ParamType::Vec3 => 3,
                _ => 4,
            };
            let mut fields = Vec::with_capacity(comps);
            for (i, &(axis, col)) in AXES.iter().take(comps).enumerate() {
                let init = vec_comp(&param.default_value, i);
                let n_set = name.to_string();
                let n_get = name.to_string();
                let field = num_field(commands, fonts, axis, col, init, 0.01, 0.0, 0.0, move |w, v| set_vec_comp(w, &n_set, i, *v), move |w| vec_comp(&param_value(w, &n_get).unwrap_or(ParamValue::Float(0.0)), i));
                fields.push(field);
            }
            commands.entity(cell).add_children(&fields);
        }
        ParamType::Color => {
            let n_get = name.to_string();
            let n_set = name.to_string();
            let cf = color_field(
                commands,
                move |w| match param_value(w, &n_get) {
                    Some(ParamValue::Color(a)) => [a[0], a[1], a[2]],
                    _ => [0.0; 3],
                },
                move |w, col| set_param(w, &n_set, ParamValue::Color([col[0], col[1], col[2], 1.0])),
            );
            commands.entity(cell).add_child(cf);
        }
        ParamType::Bool => {
            let init = matches!(param.default_value, ParamValue::Bool(true));
            let cb = checkbox(commands, init);
            let n_get = name.to_string();
            let n_set = name.to_string();
            bind_2way(commands, cb, move |w| matches!(param_value(w, &n_get), Some(ParamValue::Bool(true))), move |w, v: &bool| set_param(w, &n_set, ParamValue::Bool(*v)));
            commands.entity(cell).add_child(cb);
        }
    }
    cell
}

#[allow(clippy::too_many_arguments)]
fn num_field<G, S>(commands: &mut Commands, fonts: &EmberFonts, axis: &str, axis_color: (u8, u8, u8), init: f32, step: f32, min: f32, max: f32, set: S, get: G) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let dv = drag_value(commands, &fonts.ui, axis, axis_color, init, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, set);
    dv
}

// ── State helpers ──────────────────────────────────────────────────────────────

fn param_value(w: &World, name: &str) -> Option<ParamValue> {
    w.get_resource::<ShaderEditorState>().and_then(|s| s.shader_file.params.get(name)).map(|p| p.default_value.clone())
}

fn vec_comp(v: &ParamValue, i: usize) -> f32 {
    match v {
        ParamValue::Vec2(a) => a.get(i).copied().unwrap_or(0.0),
        ParamValue::Vec3(a) => a.get(i).copied().unwrap_or(0.0),
        ParamValue::Vec4(a) => a.get(i).copied().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn set_vec_comp(w: &mut World, name: &str, i: usize, val: f32) {
    let Some(pv) = param_value(w, name) else { return };
    let nv = match pv {
        ParamValue::Vec2(mut a) => {
            if let Some(c) = a.get_mut(i) {
                *c = val;
            }
            ParamValue::Vec2(a)
        }
        ParamValue::Vec3(mut a) => {
            if let Some(c) = a.get_mut(i) {
                *c = val;
            }
            ParamValue::Vec3(a)
        }
        ParamValue::Vec4(mut a) => {
            if let Some(c) = a.get_mut(i) {
                *c = val;
            }
            ParamValue::Vec4(a)
        }
        other => other,
    };
    set_param(w, name, nv);
}

fn set_param(w: &mut World, name: &str, val: ParamValue) {
    if let Some(mut s) = w.get_resource_mut::<ShaderEditorState>() {
        if let Some(p) = s.shader_file.params.get_mut(name) {
            p.default_value = val;
        }
        s.is_modified = true;
    }
    reapply_params(w);
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
