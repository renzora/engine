//! The value editors that sit on a node, one per unwired input and one per
//! property, in the same shape as the material editor's (`pin_editors.rs`
//! there): a scrub field per lane, a colour swatch, a checkbox.
//!
//! Every editor is bound two-way to the value in [`VfxEditorState::doc`],
//! keyed by emitter index, node id and pin name. It starts on the catalog's
//! default and writes an inline value only once it is actually changed, so an
//! untouched node keeps falling through to the default exactly as the file
//! says.
//!
//! Colours in a `.vfx` are linear (they are multiplied into additive light),
//! while ember's colour field speaks sRGB, so the field converts both ways.
//! Values above 1 (an HDR flame) survive until the swatch is actually edited.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::color_field_rgba;
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, text_input, DragRange};
use renzora_particles::asset::{NodeId, PinValue};
use renzora_particles::ir::ValueType;
use renzora_particles::nodes::{PinDef, PinDefault, PinKind};

use crate::model;
use crate::state::VfxEditorState;

const AXES: [(&str, (u8, u8, u8)); 3] = [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230))];

/// Which widget a pin gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorKind {
    Float,
    Vec2,
    Vec3,
    Color,
}

/// The editor for an input pin, or `None` when it should have none: a wired
/// pin (the wire supplies the value, and a field there would look editable
/// while doing nothing) and an attribute pin, whose unwired meaning is "this
/// particle's own" (a force's position) and which a constant would override.
///
/// A generic pin edits as whatever its inline value already is (a preset may
/// have written a Vec3 there), else as a Float, which is what an unset generic
/// pin reads as.
pub fn editor_kind(current: Option<&PinValue>, pin: &PinDef, wired: bool) -> Option<EditorKind> {
    if wired || matches!(pin.default, PinDefault::Attr(_)) {
        return None;
    }
    Some(match pin.kind {
        PinKind::Fixed(ValueType::Float) => EditorKind::Float,
        PinKind::Fixed(ValueType::Vec2) => EditorKind::Vec2,
        PinKind::Fixed(ValueType::Vec3) => EditorKind::Vec3,
        PinKind::Fixed(ValueType::Color) => EditorKind::Color,
        PinKind::Generic => match current {
            Some(PinValue::Vec2(_)) => EditorKind::Vec2,
            Some(PinValue::Vec3(_)) => EditorKind::Vec3,
            Some(PinValue::Color(_)) => EditorKind::Color,
            _ => EditorKind::Float,
        },
    })
}

/// The four lanes of a pin's value, falling back to its catalog default.
fn lanes(value: Option<&PinValue>, pin: &PinDef) -> [f32; 4] {
    match value {
        Some(PinValue::Float(v)) => [*v, *v, *v, *v],
        Some(PinValue::Vec2(v)) => [v[0], v[1], 0.0, 0.0],
        Some(PinValue::Vec3(v)) => [v[0], v[1], v[2], 0.0],
        Some(PinValue::Color(c)) => *c,
        _ => match pin.default {
            PinDefault::Value(v) => v,
            PinDefault::Attr(_) => [0.0; 4],
        },
    }
}

fn read(w: &Rx, emitter: usize, node: NodeId, pin: &str) -> Option<PinValue> {
    let st = w.get_resource::<VfxEditorState>()?;
    let g = &st.doc.emitters.get(emitter)?.graph;
    model::node(g, node)?.input_values.get(pin).cloned()
}

/// Write `value` into the document, touching the resource only when it differs
/// so a two-way binding settling on the same number records no edit.
pub fn write(w: &mut World, emitter: usize, node: NodeId, pin: &str, value: PinValue) {
    let same = w
        .get_resource::<VfxEditorState>()
        .and_then(|s| s.doc.emitters.get(emitter))
        .and_then(|e| model::node(&e.graph, node))
        .is_some_and(|n| n.input_values.get(pin) == Some(&value));
    if same {
        return;
    }
    if let Some(mut st) = w.get_resource_mut::<VfxEditorState>() {
        if let Some(e) = st.doc.emitters.get_mut(emitter) {
            model::set_value(&mut e.graph, node, pin, value);
        }
    }
}

/// Build the editor for one input pin. Returns a content-sized row the graph
/// node stretches across the rest of the pin's line.
pub fn pin_editor(
    commands: &mut Commands,
    fonts: &EmberFonts,
    emitter: usize,
    node: NodeId,
    pin: &'static PinDef,
    kind: EditorKind,
    init: Option<&PinValue>,
) -> Entity {
    // The cell is also the right-click target for "Animate" (see
    // `crate::animate`), and shows a keyed pin with the accent wash.
    let marker = crate::animate::AnimatablePin { emitter, node, pin: pin.name };
    let cell = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(3.0),
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            bevy::ui::RelativeCursorPosition::default(),
            marker,
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, cell, move |w| {
        let keyed = w.get_resource::<VfxEditorState>().is_some_and(|st| crate::animate::is_keyed(st, &marker));
        if keyed {
            rgb(accent()).with_alpha(0.18)
        } else {
            Color::NONE
        }
    });
    let start = lanes(init, pin);
    let name = pin.name;
    match kind {
        EditorKind::Float => {
            let field = num_field(commands, fonts, "", value_text(), start[0], move |w| {
                lanes(read(w, emitter, node, name).as_ref(), pin)[0]
            }, move |w, v| write(w, emitter, node, name, PinValue::Float(v)));
            if name == "angle" {
                commands.entity(field).insert(DragRange { min: 0.0, max: 180.0 });
            }
            commands.entity(cell).add_child(field);
        }
        EditorKind::Vec2 | EditorKind::Vec3 => {
            let n = if kind == EditorKind::Vec2 { 2 } else { 3 };
            let mut kids = Vec::with_capacity(n);
            for (i, &(axis, color)) in AXES.iter().take(n).enumerate() {
                let field = num_field(commands, fonts, axis, color, start[i], move |w| {
                    lanes(read(w, emitter, node, name).as_ref(), pin)[i]
                }, move |w, v| {
                    let mut l = lanes(read(&Rx::new(&*w), emitter, node, name).as_ref(), pin);
                    l[i] = v;
                    let value = if n == 2 { PinValue::Vec2([l[0], l[1]]) } else { PinValue::Vec3([l[0], l[1], l[2]]) };
                    write(w, emitter, node, name, value);
                });
                kids.push(field);
            }
            commands.entity(cell).add_children(&kids);
        }
        EditorKind::Color => {
            let field = color_field_rgba(
                commands,
                move |w| linear_to_srgb(lanes(read(w, emitter, node, name).as_ref(), pin)),
                move |w, c| write(w, emitter, node, name, PinValue::Color(srgb_to_linear(c))),
            );
            commands.entity(cell).add_child(field);
        }
    }
    cell
}

/// A Bool property's checkbox (Sphere "Surface Only", Circle "Edge Only").
pub fn bool_prop_editor(commands: &mut Commands, emitter: usize, node: NodeId, prop: &'static str, default: bool) -> Entity {
    let init = default;
    let cb = checkbox(commands, init);
    bind_2way(
        commands,
        cb,
        move |w: &Rx| match read(w, emitter, node, prop) {
            Some(PinValue::Bool(b)) => b,
            _ => default,
        },
        move |w: &mut World, v: &bool| write(w, emitter, node, prop, PinValue::Bool(*v)),
    );
    cb
}

/// A Text property's field (a Parameter node's Name). It writes on every
/// keystroke, like the scrub fields do, so the preview follows the name as it
/// is typed; an untouched field keeps the catalog default without writing it.
/// It starts on the default and the binding puts the stored name in on its
/// first run, while the field is not focused.
pub fn text_prop_editor(commands: &mut Commands, fonts: &EmberFonts, emitter: usize, node: NodeId, prop: &'static str, default: &'static str) -> Entity {
    let field = text_input(commands, &fonts.ui, default, default);
    commands.entity(field).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(60.0);
    });
    bind_text_input(
        commands,
        field,
        move |w: &Rx| match read(w, emitter, node, prop) {
            Some(PinValue::Text(s)) => s,
            _ => default.to_string(),
        },
        move |w: &mut World, v: String| write(w, emitter, node, prop, PinValue::Text(v)),
    );
    field
}

/// A labelled row for a property, built to sit under a node's pins: the label
/// where a pin's would be, the control filling the rest.
pub fn prop_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(18.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::new(Val::Px(13.0), Val::Px(8.0), Val::Px(1.0), Val::Px(3.0)),
                ..default()
            },
            renzora_ember::interaction::PASS_THROUGH,
            Name::new("vfx-prop-row"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node { flex_shrink: 0.0, ..default() },
            renzora_ember::interaction::PASS_THROUGH,
        ))
        .id();
    commands.entity(control).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
    });
    commands.entity(row).add_children(&[text, control]);
    row
}

/// A scrub field. The step follows the starting magnitude, because one effect
/// is in metres and the next in pixels: a 0.01 step that suits a 3D size makes
/// a 2D velocity of 600 take a mile of mouse.
fn num_field<G, S>(commands: &mut Commands, fonts: &EmberFonts, axis: &str, axis_color: (u8, u8, u8), init: f32, get: G, set: S) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, f32) + Send + Sync + 'static,
{
    let step = (init.abs() * 0.01).max(0.01);
    let dv = drag_value(commands, &fonts.ui, axis, axis_color, init, step);
    commands.entity(dv).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.flex_basis = Val::Px(0.0);
        n.min_width = Val::Px(36.0);
    });
    bind_2way(commands, dv, get, move |w: &mut World, v: &f32| set(w, *v));
    dv
}

pub fn linear_to_srgb(c: [f32; 4]) -> [f32; 4] {
    let s = Color::linear_rgba(c[0], c[1], c[2], c[3]).to_srgba();
    [s.red, s.green, s.blue, s.alpha]
}

pub fn srgb_to_linear(c: [f32; 4]) -> [f32; 4] {
    let l = Color::srgba(c[0], c[1], c[2], c[3]).to_linear();
    [l.red, l.green, l.blue, l.alpha]
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_particles::nodes::node_def;

    fn pin(node: &str, name: &str) -> &'static PinDef {
        node_def(node).unwrap().input(name).unwrap()
    }

    #[test]
    fn wired_and_attribute_pins_get_no_editor() {
        let p = pin("output/particle", "lifetime");
        assert_eq!(editor_kind(None, p, false), Some(EditorKind::Float));
        assert_eq!(editor_kind(None, p, true), None);
        // The curl noise position means "this particle" while unwired.
        assert_eq!(editor_kind(None, pin("force/curl_noise", "position"), false), None);
    }

    #[test]
    fn the_wind_and_blackbody_nodes_get_editors_for_their_values_only() {
        // The wind force's velocity and wind mean "this particle" and "the
        // world's wind" while unwired; a constant there would override them.
        for p in ["position", "velocity", "wind"] {
            assert_eq!(editor_kind(None, pin("force/wind", p), false), None, "{p}");
        }
        for p in ["coupling", "height_falloff"] {
            assert_eq!(editor_kind(None, pin("force/wind", p), false), Some(EditorKind::Float), "{p}");
        }
        assert_eq!(editor_kind(None, pin("input/wind", "scale"), false), Some(EditorKind::Float));
        assert_eq!(editor_kind(None, pin("color/blackbody", "temperature"), false), Some(EditorKind::Float));
        assert_eq!(lanes(None, pin("color/blackbody", "temperature"))[0], 1800.0);
    }

    #[test]
    fn fixed_pins_edit_as_their_type() {
        assert_eq!(editor_kind(None, pin("output/particle", "size"), false), Some(EditorKind::Vec2));
        assert_eq!(editor_kind(None, pin("output/particle", "velocity"), false), Some(EditorKind::Vec3));
        assert_eq!(editor_kind(None, pin("output/particle", "color"), false), Some(EditorKind::Color));
    }

    #[test]
    fn a_generic_pin_edits_as_its_stored_value() {
        let p = pin("math/multiply", "b");
        assert_eq!(editor_kind(None, p, false), Some(EditorKind::Float));
        assert_eq!(editor_kind(Some(&PinValue::Vec3([1.0, 2.0, 3.0])), p, false), Some(EditorKind::Vec3));
    }

    #[test]
    fn lanes_fall_back_to_the_catalog_default() {
        let p = pin("output/particle", "spawn_rate");
        assert_eq!(lanes(None, p)[0], 10.0);
        assert_eq!(lanes(Some(&PinValue::Float(3.0)), p)[0], 3.0);
    }

    #[test]
    fn colour_conversion_round_trips() {
        let c = [0.2, 0.5, 0.9, 0.7];
        let back = srgb_to_linear(linear_to_srgb(c));
        for i in 0..4 {
            assert!((back[i] - c[i]).abs() < 1e-4);
        }
    }
}
