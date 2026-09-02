//! The editable controls for a material node's input pins — float/vector scrub
//! fields, colour swatch, checkbox, and text fields for texture paths / string
//! params — built as standalone entities so the graph can mount them **on the
//! node itself**, directly under the pin they belong to.
//!
//! The same builder also fills the labelled rows of the "Material" panel
//! ([`crate::inspector`]), which lists the selected node's pins with
//! their names and the node's description. Both views bind to the same pin
//! value, so an edit in one shows up in the other; keeping one builder is what
//! stops them drifting apart.
//!
//! Edits write straight back into `MaterialEditorState.graph` (marking it dirty),
//! reusing ember's `drag_value`/`bind_2way`, `color_field`, `checkbox` and
//! `text_input`/`bind_text_input` editing primitives.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::color_field;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_with};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, text_input, DragRange};
use renzora_editor_framework::AssetDragPayload;
use renzora_shader::material::graph::{PinTemplate, PinType, PinValue};

use crate::MaterialEditorState;

const AXES: [(&str, (u8, u8, u8)); 4] =
    [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230)), ("W", (200, 200, 90))];
const IMG_EXTS: [&str; 10] = ["png", "jpg", "jpeg", "ktx2", "tga", "bmp", "dds", "exr", "hdr", "webp"];

/// Carries the systems the texture pin editor needs (asset drop, clear, browse).
/// The editors themselves are built on demand by the graph, so there's no panel
/// here to register — just the handlers those fields talk to.
pub struct MaterialPinEditors;

impl Plugin for MaterialPinEditors {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (tex_drop, tex_clear, tex_browse, tex_drop_highlight).run_if(in_state(SplashState::Editor)),
        );
    }
}

fn state<'w>(w: &Rx<'w>) -> Option<&'w MaterialEditorState> {
    w.get_resource::<MaterialEditorState>()
}

/// Whether [`pin_editor`] produces anything for this pin type. `Sampler` has no
/// editor (it's a plumbing pin), so callers that place editors on their own —
/// the on-node inline editors in `graph` — can skip it rather than mount
/// an empty container that still takes a row of node height.
pub(crate) fn has_pin_editor(pin_type: PinType) -> bool {
    !matches!(pin_type, PinType::Sampler)
}

/// Build the value editor(s) for an unconnected input pin, bound 2-way to the
/// pin's stored value (via [`pin_value`]/[`set_pin`], keyed by `node_id`) so
/// edits write straight into the graph and the field updates in place. Returns a
/// container entity, sized to its content so the node it mounts on can grow to
/// fit it.
pub(crate) fn pin_editor(commands: &mut Commands, fonts: &EmberFonts, node_id: u64, pin: &PinTemplate) -> Entity {
    let cell = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), ..default() })
        .id();
    let name = pin.name.clone();
    let default = pin.default_value.clone();

    match pin.pin_type {
        PinType::Float => {
            let init = scalar(&default);
            let (min, max, step) = float_range(&name);
            let field = num_field(commands, fonts, "", value_text(), init, step, min, max, {
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
                        let mut arr = vec_arr(&pin_value(&Rx::new(&*w), node_id, &n).unwrap_or(d.clone()));
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

    cell
}

#[allow(clippy::too_many_arguments)]
fn num_field<G, S>(commands: &mut Commands, fonts: &EmberFonts, axis: &str, axis_color: (u8, u8, u8), init: f32, step: f32, min: f32, max: f32, get: G, set: S) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
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
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap(), bevy::ui::FocusPolicy::Pass))
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

fn pin_value(w: &Rx, node_id: u64, pin: &str) -> Option<PinValue> {
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

/// Scrub range and step for a float pin, by name: `(min, max, step)`.
///
/// A `drag_value` only draws its slider track when it has a range, and the
/// generic ±1000 fallback puts every 0-to-1 material value in the middle of a
/// 2000-unit track — a slider you cannot aim. The pins below are the ones whose
/// units are fixed by the PBR model, so their real range is known and the track
/// becomes usable. Names are shared across nodes on purpose: a `roughness` pin
/// means the same thing wherever it appears.
///
/// `min == max` means "no range" — no track, and no clamping. That is what
/// `attenuation_distance` wants: its default is a stand-in for infinity, so any
/// slider bound we picked would silently cut it down on the first nudge.
fn float_range(pin: &str) -> (f32, f32, f32) {
    match pin {
        "metallic" | "roughness" | "ao" | "alpha" | "specular_transmission"
        | "diffuse_transmission" | "clearcoat" | "clearcoat_roughness"
        | "anisotropy_strength" | "height" => (0.0, 1.0, 0.005),
        // Bevy stores the anisotropy direction as radians around the tangent.
        "anisotropy_rotation" => (0.0, std::f32::consts::TAU, 0.01),
        // Air 1.0 → water 1.33 → glass 1.5 → diamond 2.42.
        "ior" => (1.0, 3.0, 0.005),
        "attenuation_distance" => (0.0, 0.0, 1.0),
        _ => (-1000.0, 1000.0, 0.01),
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

