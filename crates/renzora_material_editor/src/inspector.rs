//! The **Material** panel: the graph's name and domain, plus a labelled list of
//! the selected node's input pins.
//!
//! The pin editors themselves live in [`crate::pin_editors`] and are the same
//! widgets the graph mounts inline on the nodes — this panel only supplies the
//! chrome around them: the node's name and description, one labelled row per
//! pin, and "(connected)" where a wire is already supplying the value. Building
//! the cells from `pin_editor` rather than a second copy is what keeps the two
//! views from drifting apart, and means the texture drop-zone systems that
//! [`crate::pin_editors::MaterialPinEditors`] registers serve both.
//!
//! Edits write straight back into `MaterialEditorState.graph` (marking it
//! dirty), so a value changed here shows up on the node and vice versa.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_display, bind_text, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, text_input};
use renzora_shader::material::graph::{
    resolve_math_ranks, resolved_pin_type, PinDir, PinTemplate, PinType,
};
use renzora_shader::material::nodes::node_def;

use crate::pin_editors::pin_editor;
use crate::MaterialEditorState;

/// Phosphor icon name for a material node category (for native ember headers).
fn category_icon(category: &str) -> &'static str {
    match category {
        "Input" => "sign-in",
        "Parameter" => "sliders-horizontal",
        "Texture" => "image",
        "Math" => "calculator",
        "Vector" => "arrows-out-cardinal",
        "Color" => "palette",
        "Procedural" => "waves",
        "Animation" => "timer",
        "Utility" => "wrench",
        "Output" => "sign-out",
        _ => "circle",
    }
}

const LABEL_W: f32 = 88.0;

pub struct MaterialInspector;

impl Plugin for MaterialInspector {
    fn build(&self, app: &mut App) {
        app.register_panel_content("material_inspector", true, build);
    }
}

fn state<'w>(w: &Rx<'w>) -> Option<&'w MaterialEditorState> {
    w.get_resource::<MaterialEditorState>()
}
fn has_selection(w: &Rx) -> bool {
    state(w).is_some_and(|s| s.selected_node.is_some_and(|id| s.graph.get_node(id).is_some()))
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-material-inspector"),
        ))
        .id();

    // ── Material section ──
    let mat_header = section_header(commands, fonts, "Material", "cube");
    let name_row = prop_row(commands, 0);
    let name_lbl = prop_label(commands, fonts, "Name");
    let name_cell = editor_cell(commands);
    let ti = text_input(commands, &fonts.ui, "Material name", "");
    bind_text_input(
        commands,
        ti,
        |w| state(w).map(|s| s.graph.name.clone()).unwrap_or_default(),
        |w, v| {
            if let Some(mut s) = w.get_resource_mut::<MaterialEditorState>() {
                s.graph.name = v;
                s.is_dirty = true;
            }
        },
    );
    commands.entity(name_cell).add_child(ti);
    commands.entity(name_row).add_children(&[name_lbl, name_cell]);

    let domain_row = prop_row(commands, 1);
    let domain_lbl = prop_label(commands, fonts, "Domain");
    let domain_cell = editor_cell(commands);
    let domain_v = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, domain_v, |w| state(w).map(|s| s.graph.domain.display_name().to_string()).unwrap_or_default());
    commands.entity(domain_cell).add_child(domain_v);
    commands.entity(domain_row).add_children(&[domain_lbl, domain_cell]);

    // ── Selected-node section ──
    let note = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(3.0), padding: UiRect::vertical(Val::Px(18.0)), ..default() })
        .id();
    let n1 = commands.spawn((Text::new("No node selected"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    let n2 = commands.spawn((Text::new("Select a node to edit its properties"), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder())))).id();
    commands.entity(note).add_children(&[n1, n2]);
    bind_display(commands, note, |w| !has_selection(w));

    let node_list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, node_list, node_snapshot);

    commands.entity(root).add_children(&[mat_header, name_row, domain_row, note, node_list]);
    root
}

// ── Node section snapshot ───────────────────────────────────────────────────────

#[derive(Clone)]
enum Item {
    Header { icon: &'static str, name: String, desc: String },
    NoProps,
    Pin { node_id: u64, pin: PinTemplate, connected: bool },
}

fn node_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(s) = state(world) else { return empty() };
    let Some(sel) = s.selected_node else { return empty() };
    let Some(node) = s.graph.get_node(sel) else { return empty() };

    let def = node_def(&node.node_type);
    let name = def.map(|d| d.display_name).unwrap_or("Unknown").to_string();
    let category = def.map(|d| d.category).unwrap_or("Utility");
    let desc = def.map(|d| d.description).unwrap_or("").to_string();
    let icon = category_icon(category);
    let pins = def.map(|d| (d.pins)()).unwrap_or_default();
    // Resolved (latch-aware) pin types, so a latched Vec4 math pin shows a
    // vec4 editor here just like the one on the node.
    let ranks = resolve_math_ranks(&s.graph);
    let input_pins: Vec<PinTemplate> = pins
        .into_iter()
        .filter(|p| p.direction == PinDir::Input)
        .map(|mut p| {
            if let Some(t) = resolved_pin_type(&ranks, node, &p.name, p.direction) {
                p.pin_type = t;
            }
            p
        })
        .collect();
    let connected: Vec<String> =
        s.graph.connections.iter().filter(|c| c.to_node == sel).map(|c| c.to_pin.clone()).collect();

    let mut data: Vec<Item> = vec![Item::Header { icon, name, desc }];
    if input_pins.is_empty() {
        data.push(Item::NoProps);
    }
    for p in input_pins {
        let connected = connected.contains(&p.name);
        data.push(Item::Pin { node_id: sel, pin: p, connected });
    }

    let items: Vec<(u64, u64)> = data
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match it {
                Item::Header { name, desc, .. } => (0u8, name, desc).hash(&mut h),
                Item::NoProps => 1u8.hash(&mut h),
                // Structure only (NOT the value) so live edits don't rebuild the row.
                Item::Pin { node_id, pin, connected } => {
                    (2u8, node_id, &pin.name, pin_disc(&pin.pin_type), connected).hash(&mut h)
                }
            }
            (k.finish(), h.finish())
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &data[i] {
            Item::Header { icon, name, desc } => node_header(c, f, icon, name, desc),
            Item::NoProps => {
                let row = prop_row(c, 0);
                let lbl = c.spawn((Text::new("No editable properties"), ui_font(&f.ui, 11.0), TextColor(rgb(text_muted())))).id();
                c.entity(row).add_child(lbl);
                row
            }
            Item::Pin { node_id, pin, connected } => pin_row(c, f, i, *node_id, pin, *connected),
        }),
    }
}

fn node_header(commands: &mut Commands, fonts: &EmberFonts, icon: &str, name: &str, desc: &str) -> Entity {
    let col = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    let title = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, accent(), 12.0);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    commands.entity(title).add_children(&[ic, lbl]);
    commands.entity(col).add_child(title);
    if !desc.is_empty() {
        let d = commands.spawn((Text::new(desc.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
        commands.entity(col).add_child(d);
    }
    col
}

fn pin_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, node_id: u64, pin: &PinTemplate, connected: bool) -> Entity {
    let row = prop_row(commands, idx);
    let label = prop_label(commands, fonts, &pin.label);
    let cell = editor_cell(commands);

    if connected {
        let lbl = commands.spawn((Text::new("(connected)"), ui_font(&fonts.ui, 10.0), TextColor(rgb((100, 150, 255))))).id();
        commands.entity(cell).add_child(lbl);
    } else {
        // The very same editor the graph mounts under the pin — bound to the
        // same value, so whichever one you touch, both follow.
        let editor = pin_editor(commands, fonts, node_id, pin);
        commands.entity(cell).add_child(editor);
    }

    commands.entity(row).add_children(&[label, cell]);
    row
}

// ── Small layout helpers ────────────────────────────────────────────────────────

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

fn prop_row(commands: &mut Commands, idx: usize) -> Entity {
    commands
        .spawn((
            Node { width: Val::Percent(100.0), min_height: Val::Px(24.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..default() },
            BackgroundColor(renzora_ember::inspector::inspector_stripe(idx)),
        ))
        .id()
}

fn prop_label(commands: &mut Commands, fonts: &EmberFonts, name: &str) -> Entity {
    commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
        ))
        .id()
}

fn editor_cell(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(3.0), ..default() })
        .id()
}

fn pin_disc(t: &PinType) -> u8 {
    match t {
        PinType::Float => 0,
        PinType::Vec2 => 1,
        PinType::Vec3 => 2,
        PinType::Vec4 => 3,
        PinType::Color => 4,
        PinType::Bool => 5,
        PinType::Texture2D => 6,
        PinType::Sampler => 7,
        PinType::String => 8,
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
