//! Bevy-native (ember) port of the egui `BlueprintPropertiesPanel`: the selected
//! node's header (coloured name + description) and its editable input-pin values
//! — float/int/vector scrub fields, checkbox, colour swatch, and text fields for
//! string/entity pins — plus a read-only Outputs list. Connected pins show their
//! value comes from the wire.
//!
//! Mirrors the egui dual-mode write-back: scene mode edits the `BlueprintGraph`
//! component on the editing entity; asset mode mutates `file_graph` and persists
//! the `.blueprint` file via `graph_panel::save_blueprint_file`.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_blueprint::graph::{BlueprintGraph, PinDir, PinTemplate, PinType, PinValue};
use renzora_blueprint::nodes::node_def;
use renzora_editor_framework::{DocTabKind, EditorContext};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field, inspector_stripe};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, text_input, DragRange};

use crate::BlueprintEditorState;

const LABEL_W: f32 = 84.0;
const AXES: [(&str, (u8, u8, u8)); 3] =
    [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230))];

pub struct NativeBlueprintProperties;

impl Plugin for NativeBlueprintProperties {
    fn build(&self, app: &mut App) {
        app.register_panel_content("blueprint_properties", true, build);
    }
}

// ── Dual-mode graph access ──────────────────────────────────────────────────────

fn asset_mode(w: &World) -> bool {
    matches!(w.get_resource::<EditorContext>(), Some(EditorContext::Asset { kind: DocTabKind::Blueprint, .. }))
}

fn with_graph<R>(w: &World, f: impl FnOnce(&BlueprintGraph) -> R) -> Option<R> {
    let s = w.get_resource::<BlueprintEditorState>()?;
    if asset_mode(w) {
        s.file_graph.as_ref().map(f)
    } else {
        let e = s.editing_entity?;
        w.get::<BlueprintGraph>(e).map(f)
    }
}

fn selected(w: &World) -> Option<u64> {
    w.get_resource::<BlueprintEditorState>().and_then(|s| s.selected_node)
}

fn node_type_of(w: &World, id: u64) -> Option<String> {
    with_graph(w, |g| g.get_node(id).map(|n| n.node_type.clone())).flatten()
}

fn resolvable(w: &World) -> bool {
    selected(w).and_then(|id| node_type_of(w, id)).and_then(|t| node_def(&t).map(|_| ())).is_some()
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-blueprint-properties"),
        ))
        .id();

    let note = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::all(Val::Px(16.0)), ..default() })
        .id();
    let note_lbl = commands.spawn((Text::new("Select a node to edit its properties"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::new_with_justify(bevy::text::Justify::Center))).id();
    commands.entity(note).add_child(note_lbl);
    bind_display(commands, note, |w| !resolvable(w));

    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, list, props_snapshot);

    commands.entity(root).add_children(&[note, list]);
    root
}

// ── Snapshot ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum Item {
    Header { name: String, desc: String, color: (u8, u8, u8) },
    NoInputs,
    InputPin { node_id: u64, pin: PinTemplate, connected: bool },
    OutputsHeader,
    OutputPin { label: String, ty: &'static str },
}

fn props_snapshot(world: &World) -> KeyedSnapshot {
    let Some(id) = selected(world) else { return empty() };
    let Some(node_type) = node_type_of(world, id) else { return empty() };
    let Some(def) = node_def(&node_type) else { return empty() };

    let pins = (def.pins)();
    let connected: Vec<String> =
        with_graph(world, |g| g.connections.iter().filter(|c| c.to_node == id).map(|c| c.to_pin.clone()).collect::<Vec<_>>()).unwrap_or_default();

    let mut data: Vec<Item> = vec![Item::Header {
        name: def.display_name.to_string(),
        desc: def.description.to_string(),
        color: (def.color[0], def.color[1], def.color[2]),
    }];

    let inputs: Vec<&PinTemplate> = pins.iter().filter(|p| p.direction == PinDir::Input && p.pin_type != PinType::Exec).collect();
    if inputs.is_empty() {
        data.push(Item::NoInputs);
    } else {
        for p in &inputs {
            let connected = connected.contains(&p.name);
            data.push(Item::InputPin { node_id: id, pin: (*p).clone(), connected });
        }
        let outputs: Vec<&PinTemplate> = pins.iter().filter(|p| p.direction == PinDir::Output && p.pin_type != PinType::Exec).collect();
        if !outputs.is_empty() {
            data.push(Item::OutputsHeader);
            for p in &outputs {
                data.push(Item::OutputPin { label: p.label.clone(), ty: pin_type_label(p.pin_type) });
            }
        }
    }

    let items: Vec<(u64, u64)> = data
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match it {
                Item::Header { name, desc, color } => (0u8, name, desc, color).hash(&mut h),
                Item::NoInputs => 1u8.hash(&mut h),
                Item::InputPin { node_id, pin, connected } => (2u8, node_id, &pin.name, pin_disc(pin.pin_type), connected).hash(&mut h),
                Item::OutputsHeader => 3u8.hash(&mut h),
                Item::OutputPin { label, ty } => (4u8, label, ty).hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &data[i] {
            Item::Header { name, desc, color } => node_header(c, f, name, desc, *color),
            Item::NoInputs => {
                let row = prop_row(c, 0);
                let lbl = c.spawn((Text::new("No editable inputs"), ui_font(&f.ui, 11.0), TextColor(rgb(text_muted())))).id();
                c.entity(row).add_child(lbl);
                row
            }
            Item::InputPin { node_id, pin, connected } => input_pin_row(c, f, i, *node_id, pin, *connected),
            Item::OutputsHeader => section_header(c, f, "Outputs"),
            Item::OutputPin { label, ty } => output_pin_row(c, f, i, label, ty),
        }),
    }
}

fn node_header(commands: &mut Commands, fonts: &EmberFonts, name: &str, desc: &str, color: (u8, u8, u8)) -> Entity {
    let col = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() },
            BorderColor::all(rgb(border())),
        ))
        .id();
    let title = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "flow-arrow", color, 14.0);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 14.0), TextColor(rgb(color)))).id();
    commands.entity(title).add_children(&[ic, lbl]);
    commands.entity(col).add_child(title);
    if !desc.is_empty() {
        let d = commands.spawn((Text::new(desc.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
        commands.entity(col).add_child(d);
    }
    col
}

fn input_pin_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, node_id: u64, pin: &PinTemplate, connected: bool) -> Entity {
    let row = prop_row(commands, idx);

    // Connection icon + label + type badge.
    let conn = icon_text(commands, &fonts.phosphor, if connected { "plugs-connected" } else { "plug" }, if connected { accent() } else { text_muted() }, 11.0);
    let label = commands
        .spawn((Text::new(pin.label.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::new_with_no_wrap(), Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() }))
        .id();
    let cell = editor_cell(commands);

    if connected {
        let lbl = commands.spawn((Text::new("from connection"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
        commands.entity(cell).add_child(lbl);
        commands.entity(row).add_children(&[conn, label, cell]);
        return row;
    }

    let name = pin.name.clone();
    let default = pin.default_value.clone();

    match pin.pin_type {
        PinType::Float => {
            let field = num_field(commands, fonts, "", value_text(), default.as_float(), 0.01, 0.0, 0.0, {
                let n = name.clone();
                let d = default.clone();
                move |w| pin_value(w, node_id, &n).unwrap_or(d.clone()).as_float()
            }, {
                let n = name.clone();
                move |w, v| set_pin(w, node_id, &n, PinValue::Float(*v))
            });
            commands.entity(cell).add_child(field);
        }
        PinType::Int => {
            let field = num_field(commands, fonts, "", value_text(), default.as_int() as f32, 1.0, 0.0, 0.0, {
                let n = name.clone();
                let d = default.clone();
                move |w| pin_value(w, node_id, &n).unwrap_or(d.clone()).as_int() as f32
            }, {
                let n = name.clone();
                move |w, v| set_pin(w, node_id, &n, PinValue::Int(v.round() as i32))
            });
            commands.entity(cell).add_child(field);
        }
        PinType::Bool => {
            let cb = checkbox(commands, default.as_bool());
            bind_2way(commands, cb, {
                let n = name.clone();
                let d = default.clone();
                move |w| pin_value(w, node_id, &n).unwrap_or(d.clone()).as_bool()
            }, {
                let n = name.clone();
                move |w, v: &bool| set_pin(w, node_id, &n, PinValue::Bool(*v))
            });
            commands.entity(cell).add_child(cb);
        }
        PinType::Vec2 | PinType::Vec3 => {
            let comps = if pin.pin_type == PinType::Vec2 { 2 } else { 3 };
            let mut fields = Vec::with_capacity(comps);
            for (i, &(axis, col)) in AXES.iter().take(comps).enumerate() {
                let init = vec_comp(&default, i);
                let field = num_field(commands, fonts, axis, col, init, 0.01, 0.0, 0.0, {
                    let n = name.clone();
                    let d = default.clone();
                    move |w| vec_comp(&pin_value(w, node_id, &n).unwrap_or(d.clone()), i)
                }, {
                    let n = name.clone();
                    let d = default.clone();
                    move |w, v| {
                        let cur = pin_value(w, node_id, &n).unwrap_or(d.clone());
                        let mut a = [vec_comp(&cur, 0), vec_comp(&cur, 1), vec_comp(&cur, 2)];
                        a[i] = *v;
                        let nv = if comps == 2 { PinValue::Vec2([a[0], a[1]]) } else { PinValue::Vec3(a) };
                        set_pin(w, node_id, &n, nv);
                    }
                });
                fields.push(field);
            }
            commands.entity(cell).add_children(&fields);
        }
        PinType::Color => {
            let cf = color_field(commands, {
                let n = name.clone();
                let d = default.clone();
                move |w| {
                    let c = pin_value(w, node_id, &n).unwrap_or(d.clone()).as_color();
                    [c[0], c[1], c[2]]
                }
            }, {
                let n = name.clone();
                let d = default.clone();
                move |w, rgb3| {
                    let a = pin_value(w, node_id, &n).unwrap_or(d.clone()).as_color()[3];
                    set_pin(w, node_id, &n, PinValue::Color([rgb3[0], rgb3[1], rgb3[2], a]));
                }
            });
            commands.entity(cell).add_child(cf);
        }
        PinType::String => {
            let ti = text_input(commands, &fonts.ui, "...", "");
            bind_text_input(commands, ti, {
                let n = name.clone();
                move |w| pin_value(w, node_id, &n).map(|v| v.as_string()).unwrap_or_default()
            }, {
                let n = name.clone();
                move |w, v| set_pin(w, node_id, &n, PinValue::String(v))
            });
            commands.entity(cell).add_child(ti);
        }
        PinType::Entity => {
            let ti = text_input(commands, &fonts.ui, "Entity name...", "");
            bind_text_input(commands, ti, {
                let n = name.clone();
                move |w| pin_value(w, node_id, &n).map(|v| v.as_string()).unwrap_or_default()
            }, {
                let n = name.clone();
                move |w, v| set_pin(w, node_id, &n, PinValue::Entity(v))
            });
            commands.entity(cell).add_child(ti);
        }
        PinType::Any => {
            let lbl = commands.spawn((Text::new("(any)"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
            commands.entity(cell).add_child(lbl);
        }
        PinType::Exec => {}
    }

    commands.entity(row).add_children(&[conn, label, cell]);
    row
}

fn output_pin_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, label: &str, ty: &str) -> Entity {
    let row = prop_row(commands, idx);
    let lbl = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() })).id();
    let badge = commands.spawn((Text::new(ty.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    commands.entity(row).add_children(&[lbl, badge]);
    row
}

// ── Layout helpers ───────────────────────────────────────────────────────────────

fn section_header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(22.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(8.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    let lbl = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    commands.entity(row).add_child(lbl);
    row
}

fn prop_row(commands: &mut Commands, idx: usize) -> Entity {
    commands
        .spawn((
            Node { width: Val::Percent(100.0), min_height: Val::Px(24.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..default() },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id()
}

fn editor_cell(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(3.0), ..default() })
        .id()
}

#[allow(clippy::too_many_arguments)]
fn num_field<G, S>(commands: &mut Commands, fonts: &EmberFonts, axis: &str, axis_color: (u8, u8, u8), init: f32, step: f32, min: f32, max: f32, get: G, set: S) -> Entity
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

// ── State read/write ─────────────────────────────────────────────────────────────

fn pin_value(w: &World, node_id: u64, pin: &str) -> Option<PinValue> {
    with_graph(w, |g| g.get_node(node_id).and_then(|n| n.get_input_value(pin).cloned())).flatten()
}

fn set_pin(w: &mut World, node_id: u64, pin: &str, value: PinValue) {
    if !asset_mode(w) {
        let entity = w.get_resource::<BlueprintEditorState>().and_then(|s| s.editing_entity);
        if let Some(entity) = entity {
            if let Some(mut graph) = w.get_mut::<BlueprintGraph>(entity) {
                if let Some(node) = graph.get_node_mut(node_id) {
                    node.input_values.insert(pin.to_string(), value);
                }
            }
        }
        return;
    }
    // Asset mode: mutate file_graph and persist the .blueprint file.
    let (path, graph) = {
        let Some(mut state) = w.get_resource_mut::<BlueprintEditorState>() else { return };
        let path = state.editing_file_path.clone();
        if let Some(graph) = state.file_graph.as_mut() {
            if let Some(node) = graph.get_node_mut(node_id) {
                node.input_values.insert(pin.to_string(), value);
            }
        }
        let graph = state.file_graph.clone();
        state.is_dirty = true;
        (path, graph)
    };
    if let (Some(path), Some(graph)) = (path, graph) {
        let project = w.get_resource::<CurrentProject>().cloned();
        crate::graph_panel::save_blueprint_file(project.as_ref(), &path, &graph);
        if let Some(mut s) = w.get_resource_mut::<BlueprintEditorState>() {
            s.is_dirty = false;
        }
    }
}

// ── Misc ─────────────────────────────────────────────────────────────────────────

fn vec_comp(v: &PinValue, i: usize) -> f32 {
    match v {
        PinValue::Vec2(a) => a.get(i).copied().unwrap_or(0.0),
        PinValue::Vec3(a) => a.get(i).copied().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn pin_type_label(t: PinType) -> &'static str {
    match t {
        PinType::Exec => "exec",
        PinType::Float => "float",
        PinType::Int => "int",
        PinType::Bool => "bool",
        PinType::String => "string",
        PinType::Vec2 => "vec2",
        PinType::Vec3 => "vec3",
        PinType::Color => "color",
        PinType::Entity => "entity",
        PinType::Any => "any",
    }
}

fn pin_disc(t: PinType) -> u8 {
    match t {
        PinType::Exec => 0,
        PinType::Float => 1,
        PinType::Int => 2,
        PinType::Bool => 3,
        PinType::String => 4,
        PinType::Vec2 => 5,
        PinType::Vec3 => 6,
        PinType::Color => 7,
        PinType::Entity => 8,
        PinType::Any => 9,
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
