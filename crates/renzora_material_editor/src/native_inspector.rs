//! Bevy-native (ember) port of the egui `MaterialInspectorPanel`: a Material
//! section (name + domain) and, for the selected graph node, its editable input
//! pin values — float/vector scrub fields, colour swatch, checkbox, and text
//! fields for texture paths / string params. Connected pins show "(connected)".
//!
//! Edits write straight back into `MaterialEditorState.graph` (marking it dirty),
//! reusing ember's `drag_value`/`bind_2way`, `color_field`, `checkbox` and
//! `text_input`/`bind_text_input` editing primitives.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::color_field;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_text, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, text_input, DragRange};
use renzora_shader::material::graph::{PinDir, PinTemplate, PinType, PinValue};
use renzora_shader::material::nodes::node_def;
use renzora_ui::asset_drag::AssetDragPayload;

use crate::graph_editor::category_icon;
use crate::MaterialEditorState;

const LABEL_W: f32 = 88.0;
const AXES: [(&str, (u8, u8, u8)); 4] =
    [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230)), ("W", (200, 200, 90))];
const IMG_EXTS: [&str; 10] = ["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"];

pub struct NativeMaterialInspector;

impl Plugin for NativeMaterialInspector {
    fn build(&self, app: &mut App) {
        app.register_panel_content("material_inspector", true, build);
        app.add_systems(
            Update,
            (tex_drop, tex_clear, tex_browse, tex_drop_highlight).run_if(in_state(SplashState::Editor)),
        );
    }
}

fn state(w: &World) -> Option<&MaterialEditorState> {
    w.get_resource::<MaterialEditorState>()
}
fn has_selection(w: &World) -> bool {
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

fn node_snapshot(world: &World) -> KeyedSnapshot {
    let Some(s) = state(world) else { return empty() };
    let Some(sel) = s.selected_node else { return empty() };
    let Some(node) = s.graph.get_node(sel) else { return empty() };

    let def = node_def(&node.node_type);
    let name = def.map(|d| d.display_name).unwrap_or("Unknown").to_string();
    let category = def.map(|d| d.category).unwrap_or("Utility");
    let desc = def.map(|d| d.description).unwrap_or("").to_string();
    let icon = category_icon(category);
    let pins = def.map(|d| (d.pins)()).unwrap_or_default();
    let input_pins: Vec<PinTemplate> = pins.into_iter().filter(|p| p.direction == PinDir::Input).collect();
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
        commands.entity(row).add_children(&[label, cell]);
        return row;
    }

    let name = pin.name.clone();
    let default = pin.default_value.clone();

    match pin.pin_type {
        PinType::Float => {
            let init = scalar(&default);
            let field = num_field(commands, fonts, "", value_text(), init, 0.01, -1000.0, 1000.0, {
                let n = name.clone();
                let d = default.clone();
                move |w| match pin_value(w, node_id, &n).unwrap_or(d.clone()) {
                    PinValue::Float(v) => v,
                    _ => 0.0,
                }
            }, {
                let n = name.clone();
                move |w, v| set_pin(w, node_id, &n, PinValue::Float(*v))
            });
            commands.entity(cell).add_child(field);
        }
        PinType::Vec2 | PinType::Vec3 | PinType::Vec4 => {
            let comps = match pin.pin_type {
                PinType::Vec2 => 2,
                PinType::Vec3 => 3,
                _ => 4,
            };
            let ptype = pin.pin_type;
            let mut fields = Vec::with_capacity(comps);
            for (i, &(axis, col)) in AXES.iter().take(comps).enumerate() {
                let init = vec_arr(&default)[i];
                let field = num_field(commands, fonts, axis, col, init, 0.1, -10000.0, 10000.0, {
                    let n = name.clone();
                    let d = default.clone();
                    move |w| vec_arr(&pin_value(w, node_id, &n).unwrap_or(d.clone()))[i]
                }, {
                    let n = name.clone();
                    let d = default.clone();
                    move |w, v| {
                        let mut arr = vec_arr(&pin_value(w, node_id, &n).unwrap_or(d.clone()));
                        arr[i] = *v;
                        set_pin(w, node_id, &n, vec_value(ptype, arr));
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
                move |w| match pin_value(w, node_id, &n).unwrap_or(d.clone()) {
                    PinValue::Color(a) => [a[0], a[1], a[2]],
                    _ => [1.0, 1.0, 1.0],
                }
            }, {
                let n = name.clone();
                move |w, col| set_pin(w, node_id, &n, PinValue::Color([col[0], col[1], col[2], 1.0]))
            });
            commands.entity(cell).add_child(cf);
        }
        PinType::Bool => {
            let init = matches!(default, PinValue::Bool(true));
            let cb = checkbox(commands, init);
            bind_2way(commands, cb, {
                let n = name.clone();
                move |w| matches!(pin_value(w, node_id, &n), Some(PinValue::Bool(true)))
            }, {
                let n = name.clone();
                move |w, v: &bool| set_pin(w, node_id, &n, PinValue::Bool(*v))
            });
            commands.entity(cell).add_child(cb);
        }
        PinType::Texture2D => {
            let tex = texture_field(commands, fonts, node_id, &name);
            commands.entity(cell).add_child(tex);
        }
        PinType::String => {
            let ti = text_input(commands, &fonts.ui, "ParameterName", "");
            bind_text_input(commands, ti, {
                let n = name.clone();
                move |w| match pin_value(w, node_id, &n) {
                    Some(PinValue::String(s)) => s,
                    _ => String::new(),
                }
            }, {
                let n = name.clone();
                move |w, v| set_pin(w, node_id, &n, PinValue::String(v))
            });
            commands.entity(cell).add_child(ti);
        }
        PinType::Sampler => {
            let lbl = commands.spawn((Text::new("(no editor)"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
            commands.entity(cell).add_child(lbl);
        }
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
            bevy::text::TextLayout::new_with_no_wrap(),
            Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
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

// ── Texture pin: native asset drop-zone + browse + clear (egui parity) ──────────

#[derive(Component)]
struct TexDropZone {
    node_id: u64,
    pin: String,
}
#[derive(Component)]
struct TexClearBtn {
    node_id: u64,
    pin: String,
}

fn tex_display(v: Option<PinValue>) -> (String, bool) {
    match v {
        Some(PinValue::TexturePath(p)) if !p.is_empty() => {
            let name = std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(p);
            (name, true)
        }
        _ => ("Drop texture or click to browse".to_string(), false),
    }
}

fn texture_field(commands: &mut Commands, fonts: &EmberFonts, node_id: u64, pin: &str) -> Entity {
    let path_text = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::new_with_no_wrap(), bevy::ui::FocusPolicy::Pass))
        .id();
    let n = pin.to_string();
    bind_with(commands, path_text, move |w| tex_display(pin_value(w, node_id, &n)), |w, e, (text, has): &(String, bool)| {
        if let Some(mut t) = w.get_mut::<Text>(e) {
            if t.0 != *text {
                t.0 = text.clone();
            }
        }
        if let Some(mut col) = w.get_mut::<TextColor>(e) {
            col.0 = rgb(if *has { text_primary() } else { text_muted() });
        }
    });
    let drop_box = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            TexDropZone { node_id, pin: pin.to_string() },
            Name::new("mat-tex-drop"),
        ))
        .id();
    commands.entity(drop_box).add_child(path_text);
    let clear = commands
        .spawn((Text::new("\u{2715}"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { padding: UiRect::horizontal(Val::Px(2.0)), ..default() }, Interaction::default(), TexClearBtn { node_id, pin: pin.to_string() }, Name::new("mat-tex-clear")))
        .id();
    let row = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
        .id();
    commands.entity(row).add_children(&[drop_box, clear]);
    row
}

/// Drop a dragged image asset onto the hovered zone → set its relative path.
fn tex_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    zones: Query<(&RelativeCursorPosition, &TexDropZone)>,
    state: Option<ResMut<MaterialEditorState>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(mut state)) = (payload, state) else { return };
    if !payload.is_detached || !payload.matches_extensions(&IMG_EXTS) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let path = project.as_ref().map(|p| p.make_asset_relative(&payload.path)).unwrap_or_else(|| payload.path.to_string_lossy().to_string());
        if let Some(node) = state.graph.get_node_mut(zone.node_id) {
            node.input_values.insert(zone.pin.clone(), PinValue::TexturePath(path));
        }
        state.is_dirty = true;
        break;
    }
}

fn tex_clear(q: Query<(&Interaction, &TexClearBtn), Changed<Interaction>>, state: Option<ResMut<MaterialEditorState>>) {
    let Some(mut state) = state else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(node) = state.graph.get_node_mut(btn.node_id) {
            node.input_values.insert(btn.pin.clone(), PinValue::TexturePath(String::new()));
        }
        state.is_dirty = true;
    }
}

/// Click a zone (when no asset is being dragged) → open a file picker.
fn tex_browse(
    q: Query<(&Interaction, &TexDropZone), Changed<Interaction>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    state: Option<ResMut<MaterialEditorState>>,
) {
    if payload.as_ref().is_some_and(|p| p.is_detached) {
        return;
    }
    let Some(mut state) = state else { return };
    for (interaction, zone) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(file) = rfd::FileDialog::new().add_filter("Image", &IMG_EXTS).pick_file() {
            let path = project.as_ref().map(|p| p.make_asset_relative(&file)).unwrap_or_else(|| file.to_string_lossy().to_string());
            if let Some(node) = state.graph.get_node_mut(zone.node_id) {
                node.input_values.insert(zone.pin.clone(), PinValue::TexturePath(path));
            }
            state.is_dirty = true;
        }
    }
}

/// Accent the zone border while a compatible asset is dragged over it.
fn tex_drop_highlight(payload: Option<Res<AssetDragPayload>>, mut zones: Query<(&RelativeCursorPosition, &mut BorderColor), With<TexDropZone>>) {
    for (rcp, mut bc) in &mut zones {
        let active = payload.as_ref().is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(&IMG_EXTS));
        let want = BorderColor::all(rgb(if active { accent() } else { border() }));
        if *bc != want {
            *bc = want;
        }
    }
}

// ── State helpers ────────────────────────────────────────────────────────────────

fn pin_value(w: &World, node_id: u64, pin: &str) -> Option<PinValue> {
    state(w).and_then(|s| s.graph.get_node(node_id)).and_then(|n| n.input_values.get(pin).cloned())
}

fn set_pin(w: &mut World, node_id: u64, pin: &str, val: PinValue) {
    if let Some(mut s) = w.get_resource_mut::<MaterialEditorState>() {
        if let Some(n) = s.graph.get_node_mut(node_id) {
            n.input_values.insert(pin.to_string(), val);
        }
        s.is_dirty = true;
    }
}

fn scalar(v: &PinValue) -> f32 {
    match v {
        PinValue::Float(f) => *f,
        PinValue::Int(i) => *i as f32,
        _ => 0.0,
    }
}

fn vec_arr(v: &PinValue) -> [f32; 4] {
    match v {
        PinValue::Vec2(a) => [a[0], a[1], 0.0, 0.0],
        PinValue::Vec3(a) => [a[0], a[1], a[2], 0.0],
        PinValue::Vec4(a) | PinValue::Color(a) => *a,
        PinValue::Float(f) => [*f, *f, *f, *f],
        _ => [0.0; 4],
    }
}

fn vec_value(ptype: PinType, a: [f32; 4]) -> PinValue {
    match ptype {
        PinType::Vec2 => PinValue::Vec2([a[0], a[1]]),
        PinType::Vec3 => PinValue::Vec3([a[0], a[1], a[2]]),
        _ => PinValue::Vec4(a),
    }
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
