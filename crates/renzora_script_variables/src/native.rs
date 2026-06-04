//! Bevy-native (ember) Script Variables panel — a read-only list of the active
//! script's `props()` variables (type badge + name + default value + hint). The
//! active script comes from `CodeEditorState`; the props from `ScriptEngine`.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_code_editor::CodeEditorState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_scripting::{ScriptEngine, ScriptValue, ScriptVariableDefinition};

const DISABLED: (u8, u8, u8) = (110, 110, 122);
const ROW_EVEN: (u8, u8, u8) = (28, 28, 34);
const ROW_ODD: (u8, u8, u8) = (24, 24, 30);
const EXTREME: (u8, u8, u8) = (16, 16, 21);

pub fn register_native_script_variables(app: &mut App) {
    app.register_panel_content("script_variables", true, build);
}

/// Name of the active script tab (empty if none open).
fn script_name(w: &World) -> String {
    w.get_resource::<CodeEditorState>()
        .and_then(|s| s.active_tab.and_then(|i| s.open_files.get(i)))
        .map(|f| f.name.clone())
        .unwrap_or_default()
}

/// The active script's variable definitions.
fn active_props(w: &World) -> Vec<ScriptVariableDefinition> {
    let (Some(state), Some(engine)) = (
        w.get_resource::<CodeEditorState>(),
        w.get_resource::<ScriptEngine>(),
    ) else {
        return Vec::new();
    };
    state
        .active_tab
        .and_then(|i| state.open_files.get(i))
        .map(|f| engine.get_script_props(&f.path))
        .unwrap_or_default()
}

fn type_color(name: &str) -> (u8, u8, u8) {
    match name {
        "Float" => (120, 200, 120),
        "Int" => (100, 180, 220),
        "Bool" => (220, 160, 100),
        "String" => (200, 140, 180),
        "Entity" => (100, 210, 200),
        "Vec2" => (180, 140, 220),
        "Vec3" => (140, 180, 220),
        "Color" => (220, 180, 100),
        _ => (150, 150, 150),
    }
}

fn format_default(v: &ScriptValue) -> String {
    match v {
        ScriptValue::Float(x) => format!("{:.2}", x),
        ScriptValue::Int(x) => format!("{}", x),
        ScriptValue::Bool(x) => format!("{}", x),
        ScriptValue::String(x) => format!("\"{}\"", x),
        ScriptValue::Entity(x) => {
            if x.is_empty() {
                "(none)".to_string()
            } else {
                x.clone()
            }
        }
        ScriptValue::Vec2(v) => format!("({:.1}, {:.1})", v.x, v.y),
        ScriptValue::Vec3(v) => format!("({:.1}, {:.1}, {:.1})", v.x, v.y, v.z),
        ScriptValue::Color(c) => format!("rgba({:.2}, {:.2}, {:.2}, {:.2})", c.x, c.y, c.z, c.w),
    }
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            min_height: Val::Px(0.0),
            ..default()
        })
        .id();

    // Empty state (no script open).
    let empty = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(Val::Px(40.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("script-vars-empty"),
        ))
        .id();
    let e_icon = icon_text(commands, &fonts.phosphor, "code", DISABLED, 32.0);
    let e_text = commands
        .spawn((Text::new("Open a script to see its variables"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(empty).add_children(&[e_icon, e_text]);
    bind_display(commands, empty, |w| script_name(w).is_empty());

    // Content (script open).
    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            min_height: Val::Px(0.0),
            padding: UiRect::all(Val::Px(8.0)),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, content, |w| !script_name(w).is_empty());

    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let h_icon = icon_text(commands, &fonts.phosphor, "code", accent(), 14.0);
    let h_name = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, h_name, script_name);
    commands.entity(header).add_children(&[h_icon, h_name]);

    let divider = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb((60, 60, 72))),
        ))
        .id();

    let list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, props_snapshot);

    let footer = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), margin: UiRect::top(Val::Px(12.0)), ..default() })
        .id();
    let f_icon = icon_text(commands, &fonts.phosphor, "info", DISABLED, 12.0);
    let f_text = commands
        .spawn((Text::new("Edit values in the Inspector panel"), ui_font(&fonts.ui, 10.0), TextColor(rgb(DISABLED))))
        .id();
    commands.entity(footer).add_children(&[f_icon, f_text]);

    commands.entity(content).add_children(&[header, divider, list, footer]);
    commands.entity(root).add_children(&[empty, content]);
    root
}

fn props_snapshot(world: &World) -> KeyedSnapshot {
    let props = active_props(world);
    if props.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| empty_vars(c, f)),
        };
    }
    let items: Vec<(u64, u64)> = props
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&p.display_name, p.default_value.type_name(), format_default(&p.default_value), &p.hint).hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| prop_row(c, f, i, &props[i])),
    }
}

fn prop_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, p: &ScriptVariableDefinition) -> Entity {
    let bg = if idx.is_multiple_of(2) { ROW_EVEN } else { ROW_ODD };
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(bg)),
        ))
        .id();
    let line = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, width: Val::Percent(100.0), column_gap: Val::Px(6.0), ..default() })
        .id();
    let ty = p.default_value.type_name();
    let badge = commands
        .spawn((Text::new(ty), ui_font(&fonts.mono, 10.0), TextColor(rgb(type_color(ty)))))
        .id();
    let name = commands
        .spawn((Text::new(p.display_name.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let val = commands
        .spawn((Text::new(format_default(&p.default_value)), ui_font(&fonts.mono, 10.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(line).add_children(&[badge, name, gap, val]);
    let mut kids = vec![line];
    if let Some(hint) = &p.hint {
        kids.push(
            commands
                .spawn((Text::new(hint.clone()), ui_font(&fonts.ui, 9.0), TextColor(rgb(DISABLED))))
                .id(),
        );
    }
    commands.entity(row).add_children(&kids);
    row
}

fn empty_vars(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            padding: UiRect::top(Val::Px(20.0)),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let t1 = commands
        .spawn((Text::new("No variables defined"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted()))))
        .id();
    let t2 = commands
        .spawn((Text::new("Add a props() function to define variables:"), ui_font(&fonts.ui, 11.0), TextColor(rgb(DISABLED))))
        .id();
    let example = "fn props() {\n    #{\n        speed: #{ value: 5.0, hint: \"Movement speed\" },\n        health: #{ value: 100 },\n        name: #{ value: \"Player\" },\n        active: #{ value: true },\n    }\n}";
    let frame = commands
        .spawn((
            Node { padding: UiRect::all(Val::Px(8.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(EXTREME)),
        ))
        .id();
    let code = commands
        .spawn((Text::new(example), ui_font(&fonts.mono, 10.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(frame).add_child(code);
    commands.entity(col).add_children(&[t1, t2, frame]);
    col
}
