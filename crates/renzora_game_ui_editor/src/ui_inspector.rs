//! UI Inspector — declarative `FieldDef` builders for selected UiWidget /
//! UiCanvas entities.
//!
//! The egui custom-drawer functions that used to live here have been removed;
//! their native (bevy_ui) replacements live in the `renzora_game_ui_editor`
//! crate. This module now only exposes the declarative `*_fields()` builders
//! the `InspectorEntry` registrations in `lib.rs` consume.

use bevy::prelude::*;

use renzora_game_ui::components::{self, UiCanvas};

/// Snapshot of the selected widget's properties for editing.
/// Convert a Val to design-space pixels given a reference dimension.
fn val_to_design_px(v: bevy::ui::Val, reference: f32) -> f32 {
    match v {
        bevy::ui::Val::Percent(p) => p * reference / 100.0,
        bevy::ui::Val::Px(px) => px,
        _ => 0.0,
    }
}

fn position_type_to_u8(pt: bevy::ui::PositionType) -> u8 {
    match pt {
        bevy::ui::PositionType::Relative => 0,
        bevy::ui::PositionType::Absolute => 1,
    }
}

fn u8_to_position_type(v: u8) -> bevy::ui::PositionType {
    match v {
        1 => bevy::ui::PositionType::Absolute,
        _ => bevy::ui::PositionType::Relative,
    }
}

fn flex_direction_to_u8(fd: bevy::ui::FlexDirection) -> u8 {
    match fd {
        bevy::ui::FlexDirection::Row => 0,
        bevy::ui::FlexDirection::Column => 1,
        bevy::ui::FlexDirection::RowReverse => 2,
        bevy::ui::FlexDirection::ColumnReverse => 3,
    }
}

fn u8_to_flex_direction(v: u8) -> bevy::ui::FlexDirection {
    match v {
        1 => bevy::ui::FlexDirection::Column,
        2 => bevy::ui::FlexDirection::RowReverse,
        3 => bevy::ui::FlexDirection::ColumnReverse,
        _ => bevy::ui::FlexDirection::Row,
    }
}

fn justify_content_to_u8(jc: bevy::ui::JustifyContent) -> u8 {
    match jc {
        bevy::ui::JustifyContent::Start => 0,
        bevy::ui::JustifyContent::Center => 1,
        bevy::ui::JustifyContent::End => 2,
        bevy::ui::JustifyContent::SpaceBetween => 3,
        bevy::ui::JustifyContent::SpaceAround => 4,
        bevy::ui::JustifyContent::SpaceEvenly => 5,
        _ => 0,
    }
}

fn u8_to_justify_content(v: u8) -> bevy::ui::JustifyContent {
    match v {
        1 => bevy::ui::JustifyContent::Center,
        2 => bevy::ui::JustifyContent::End,
        3 => bevy::ui::JustifyContent::SpaceBetween,
        4 => bevy::ui::JustifyContent::SpaceAround,
        5 => bevy::ui::JustifyContent::SpaceEvenly,
        _ => bevy::ui::JustifyContent::Start,
    }
}

fn align_items_to_u8(ai: bevy::ui::AlignItems) -> u8 {
    match ai {
        bevy::ui::AlignItems::Start => 0,
        bevy::ui::AlignItems::Center => 1,
        bevy::ui::AlignItems::End => 2,
        bevy::ui::AlignItems::Stretch => 3,
        _ => 0,
    }
}

fn u8_to_align_items(v: u8) -> bevy::ui::AlignItems {
    match v {
        1 => bevy::ui::AlignItems::Center,
        2 => bevy::ui::AlignItems::End,
        3 => bevy::ui::AlignItems::Stretch,
        _ => bevy::ui::AlignItems::Start,
    }
}

/// Resolve the canvas reference resolution for `entity` (from a `UiCanvas` on it
/// or its parent), used to convert layout `Val`s ↔ design-space pixels.
fn canvas_ref(world: &World, entity: Entity) -> (f32, f32) {
    if let Some(c) = world.get::<UiCanvas>(entity) {
        return (c.reference_width, c.reference_height);
    }
    if let Some(child_of) = world.get::<bevy::prelude::ChildOf>(entity) {
        if let Some(c) = world.get::<UiCanvas>(child_of.parent()) {
            return (c.reference_width, c.reference_height);
        }
    }
    (1280.0, 720.0)
}

const POS_LABELS: &[&str] = &["Relative", "Absolute"];
const DIR_LABELS: &[&str] = &["Row", "Column", "Row Rev", "Col Rev"];
const JUSTIFY_LABELS: &[&str] = &["Start", "Center", "End", "Between", "Around", "Evenly"];
const ALIGN_LABELS: &[&str] = &["Start", "Center", "End", "Stretch"];

fn label_index(labels: &[&str], s: &str) -> u8 {
    labels.iter().position(|l| *l == s).unwrap_or(0) as u8
}

/// Declarative `FieldDef`s for a `Node` layout — the bevy_ui-native equivalent of
/// `render_layout_inspector`. Enums map via the `*_to_u8`/`u8_to_*` helpers; X/Y/
/// Width/Height convert `Val` ↔ design-space pixels using the canvas reference.
pub fn layout_fields() -> Vec<renzora::FieldDef> {
    use bevy::ui::{Node, Val};
    use renzora::{FieldDef, FieldType, FieldValue};

    vec![
        FieldDef {
            name: "Position",
            field_type: FieldType::Enum { options: POS_LABELS },
            get_fn: |w, e| {
                w.get::<Node>(e).map(|n| {
                    FieldValue::Enum(POS_LABELS[position_type_to_u8(n.position_type) as usize].into())
                })
            },
            set_fn: |w, e, v| {
                if let (FieldValue::Enum(s), Some(mut n)) = (v, w.get_mut::<Node>(e)) {
                    n.position_type = u8_to_position_type(label_index(POS_LABELS, &s));
                }
            },
        },
        FieldDef {
            name: "X",
            field_type: FieldType::Float { speed: 1.0, min: f32::MIN, max: f32::MAX },
            get_fn: |w, e| {
                let (crw, _) = canvas_ref(w, e);
                w.get::<Node>(e).map(|n| FieldValue::Float(val_to_design_px(n.left, crw)))
            },
            set_fn: |w, e, v| {
                if let FieldValue::Float(f) = v {
                    let (crw, _) = canvas_ref(w, e);
                    if let Some(mut n) = w.get_mut::<Node>(e) {
                        n.left = Val::Percent(f / crw * 100.0);
                    }
                }
            },
        },
        FieldDef {
            name: "Y",
            field_type: FieldType::Float { speed: 1.0, min: f32::MIN, max: f32::MAX },
            get_fn: |w, e| {
                let (_, crh) = canvas_ref(w, e);
                w.get::<Node>(e).map(|n| FieldValue::Float(val_to_design_px(n.top, crh)))
            },
            set_fn: |w, e, v| {
                if let FieldValue::Float(f) = v {
                    let (_, crh) = canvas_ref(w, e);
                    if let Some(mut n) = w.get_mut::<Node>(e) {
                        n.top = Val::Percent(f / crh * 100.0);
                    }
                }
            },
        },
        FieldDef {
            name: "Width",
            field_type: FieldType::Float { speed: 1.0, min: 0.0, max: f32::MAX },
            get_fn: |w, e| {
                let (crw, _) = canvas_ref(w, e);
                w.get::<Node>(e).map(|n| FieldValue::Float(val_to_design_px(n.width, crw)))
            },
            set_fn: |w, e, v| {
                if let FieldValue::Float(f) = v {
                    let (crw, _) = canvas_ref(w, e);
                    if let Some(mut n) = w.get_mut::<Node>(e) {
                        n.width = Val::Percent(f / crw * 100.0);
                    }
                }
            },
        },
        FieldDef {
            name: "Height",
            field_type: FieldType::Float { speed: 1.0, min: 0.0, max: f32::MAX },
            get_fn: |w, e| {
                let (_, crh) = canvas_ref(w, e);
                w.get::<Node>(e).map(|n| FieldValue::Float(val_to_design_px(n.height, crh)))
            },
            set_fn: |w, e, v| {
                if let FieldValue::Float(f) = v {
                    let (_, crh) = canvas_ref(w, e);
                    if let Some(mut n) = w.get_mut::<Node>(e) {
                        n.height = Val::Percent(f / crh * 100.0);
                    }
                }
            },
        },
        FieldDef {
            name: "Direction",
            field_type: FieldType::Enum { options: DIR_LABELS },
            get_fn: |w, e| {
                w.get::<Node>(e).map(|n| {
                    FieldValue::Enum(DIR_LABELS[flex_direction_to_u8(n.flex_direction) as usize].into())
                })
            },
            set_fn: |w, e, v| {
                if let (FieldValue::Enum(s), Some(mut n)) = (v, w.get_mut::<Node>(e)) {
                    n.flex_direction = u8_to_flex_direction(label_index(DIR_LABELS, &s));
                }
            },
        },
        FieldDef {
            name: "Justify",
            field_type: FieldType::Enum { options: JUSTIFY_LABELS },
            get_fn: |w, e| {
                w.get::<Node>(e).map(|n| {
                    FieldValue::Enum(JUSTIFY_LABELS[justify_content_to_u8(n.justify_content) as usize].into())
                })
            },
            set_fn: |w, e, v| {
                if let (FieldValue::Enum(s), Some(mut n)) = (v, w.get_mut::<Node>(e)) {
                    n.justify_content = u8_to_justify_content(label_index(JUSTIFY_LABELS, &s));
                }
            },
        },
        FieldDef {
            name: "Align",
            field_type: FieldType::Enum { options: ALIGN_LABELS },
            get_fn: |w, e| {
                w.get::<Node>(e).map(|n| {
                    FieldValue::Enum(ALIGN_LABELS[align_items_to_u8(n.align_items) as usize].into())
                })
            },
            set_fn: |w, e, v| {
                if let (FieldValue::Enum(s), Some(mut n)) = (v, w.get_mut::<Node>(e)) {
                    n.align_items = u8_to_align_items(label_index(ALIGN_LABELS, &s));
                }
            },
        },
    ]
}

// ── Widget-specific data fields ──────────────────────────────────────────────
//
// Each widget data component (SliderData, CheckboxData, …) is its own
// InspectorEntry now. The main inspector wraps each in a collapsible
// automatically; these fns supply its declarative `FieldDef` rows.

/// Declarative fields for `SliderData` (native bevy_ui inspector).
pub fn slider_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::float_field!("Value", components::SliderData, value, 0.01, f32::MIN, f32::MAX),
        renzora::float_field!("Min", components::SliderData, min, 0.1, f32::MIN, f32::MAX),
        renzora::float_field!("Max", components::SliderData, max, 0.1, f32::MIN, f32::MAX),
        renzora::float_field!("Step", components::SliderData, step, 0.01, 0.0, f32::MAX),
        renzora::color_rgba_field!("Track Color", components::SliderData, track_color),
        renzora::color_rgba_field!("Fill Color", components::SliderData, fill_color),
        renzora::color_rgba_field!("Thumb Color", components::SliderData, thumb_color),
    ]
}

/// Declarative fields for `UiTextStyle` (+ Content on the bevy `Text`).
pub fn text_fields() -> Vec<renzora::FieldDef> {
    use renzora::{FieldDef, FieldType, FieldValue};
    const ALIGN: &[&str] = &["Left", "Center", "Right"];
    vec![
        FieldDef {
            name: "Content",
            field_type: FieldType::String,
            get_fn: |w, e| w.get::<bevy::ui::widget::Text>(e).map(|t| FieldValue::String(t.0.clone())),
            set_fn: |w, e, v| {
                if let (FieldValue::String(s), Some(mut t)) = (v, w.get_mut::<bevy::ui::widget::Text>(e)) {
                    t.0 = s;
                }
            },
        },
        renzora::color_rgba_field!("Color", components::UiTextStyle, color),
        renzora::float_field!("Size", components::UiTextStyle, size, 0.5, 1.0, 200.0),
        renzora::bool_field!("Bold", components::UiTextStyle, bold),
        renzora::bool_field!("Italic", components::UiTextStyle, italic),
        FieldDef {
            name: "Align",
            field_type: FieldType::Enum { options: ALIGN },
            get_fn: |w, e| {
                w.get::<components::UiTextStyle>(e).map(|s| {
                    FieldValue::Enum(
                        match s.align {
                            components::UiTextAlign::Left => "Left",
                            components::UiTextAlign::Center => "Center",
                            components::UiTextAlign::Right => "Right",
                        }
                        .to_string(),
                    )
                })
            },
            set_fn: |w, e, v| {
                if let (FieldValue::Enum(s), Some(mut st)) = (v, w.get_mut::<components::UiTextStyle>(e)) {
                    st.align = match s.as_str() {
                        "Left" => components::UiTextAlign::Left,
                        "Right" => components::UiTextAlign::Right,
                        _ => components::UiTextAlign::Center,
                    };
                }
            },
        },
    ]
}

/// Declarative fields for `UiBoxShadow`.
pub fn shadow_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::color_rgba_field!("Color", components::UiBoxShadow, color),
        renzora::float_field!("Offset X", components::UiBoxShadow, offset_x, 0.5, f32::MIN, f32::MAX),
        renzora::float_field!("Offset Y", components::UiBoxShadow, offset_y, 0.5, f32::MIN, f32::MAX),
        renzora::float_field!("Blur", components::UiBoxShadow, blur, 0.5, 0.0, 200.0),
        renzora::float_field!("Spread", components::UiBoxShadow, spread, 0.5, f32::MIN, f32::MAX),
    ]
}

/// Declarative fields for `UiWidget` (read-only Type + Locked toggle).
pub fn widget_fields() -> Vec<renzora::FieldDef> {
    use renzora::{FieldDef, FieldType, FieldValue};
    vec![
        FieldDef {
            name: "Type",
            field_type: FieldType::ReadOnly,
            get_fn: |w, e| {
                w.get::<components::UiWidget>(e)
                    .map(|wd| FieldValue::ReadOnly(wd.widget_type.label().to_string()))
            },
            set_fn: |_, _, _| {},
        },
        renzora::bool_field!("Locked", components::UiWidget, locked),
    ]
}

/// Declarative fields for `UiPadding` (Top/Right/Bottom/Left).
pub fn padding_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::float_field!("Top", components::UiPadding, top, 0.5, 0.0, 500.0),
        renzora::float_field!("Right", components::UiPadding, right, 0.5, 0.0, 500.0),
        renzora::float_field!("Bottom", components::UiPadding, bottom, 0.5, 0.0, 500.0),
        renzora::float_field!("Left", components::UiPadding, left, 0.5, 0.0, 500.0),
    ]
}

/// Declarative fields for `UiBorderRadius` (per-corner).
pub fn border_radius_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::float_field!("Top Left", components::UiBorderRadius, top_left, 0.5, 0.0, 500.0),
        renzora::float_field!("Top Right", components::UiBorderRadius, top_right, 0.5, 0.0, 500.0),
        renzora::float_field!("Bottom Right", components::UiBorderRadius, bottom_right, 0.5, 0.0, 500.0),
        renzora::float_field!("Bottom Left", components::UiBorderRadius, bottom_left, 0.5, 0.0, 500.0),
    ]
}

/// Declarative field for `UiOpacity` (a `f32` tuple component).
pub fn opacity_fields() -> Vec<renzora::FieldDef> {
    use renzora::{FieldDef, FieldType, FieldValue};
    vec![FieldDef {
        name: "Opacity",
        field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
        get_fn: |w, e| w.get::<components::UiOpacity>(e).map(|o| FieldValue::Float(o.0)),
        set_fn: |w, e, v| {
            if let (FieldValue::Float(f), Some(mut o)) = (v, w.get_mut::<components::UiOpacity>(e)) {
                o.0 = f;
            }
        },
    }]
}

/// Declarative field for `UiClipContent` (a `bool` tuple component).
pub fn clip_content_fields() -> Vec<renzora::FieldDef> {
    use renzora::{FieldDef, FieldType, FieldValue};
    vec![FieldDef {
        name: "Clip Content",
        field_type: FieldType::Bool,
        get_fn: |w, e| w.get::<components::UiClipContent>(e).map(|c| FieldValue::Bool(c.0)),
        set_fn: |w, e, v| {
            if let (FieldValue::Bool(b), Some(mut c)) = (v, w.get_mut::<components::UiClipContent>(e)) {
                c.0 = b;
            }
        },
    }]
}

/// Declarative field for `UiCursor` (enum → dropdown).
pub fn cursor_fields() -> Vec<renzora::FieldDef> {
    use renzora::{FieldDef, FieldType, FieldValue};
    const CURSOR_LABELS: &[&str] = &[
        "Default", "Pointer", "Text", "Grab", "Grabbing", "Not Allowed", "Crosshair", "EW Resize", "NS Resize", "Move",
    ];
    vec![FieldDef {
        name: "Cursor",
        field_type: FieldType::Enum { options: CURSOR_LABELS },
        get_fn: |w, e| {
            w.get::<components::UiCursor>(e).map(|c| {
                let i = match c {
                    components::UiCursor::Default => 0,
                    components::UiCursor::Pointer => 1,
                    components::UiCursor::Text => 2,
                    components::UiCursor::Grab => 3,
                    components::UiCursor::Grabbing => 4,
                    components::UiCursor::NotAllowed => 5,
                    components::UiCursor::Crosshair => 6,
                    components::UiCursor::EwResize => 7,
                    components::UiCursor::NsResize => 8,
                    components::UiCursor::Move => 9,
                };
                FieldValue::Enum(CURSOR_LABELS[i].to_string())
            })
        },
        set_fn: |w, e, v| {
            if let (FieldValue::Enum(s), Some(mut c)) = (v, w.get_mut::<components::UiCursor>(e)) {
                *c = match CURSOR_LABELS.iter().position(|l| *l == s).unwrap_or(0) {
                    1 => components::UiCursor::Pointer,
                    2 => components::UiCursor::Text,
                    3 => components::UiCursor::Grab,
                    4 => components::UiCursor::Grabbing,
                    5 => components::UiCursor::NotAllowed,
                    6 => components::UiCursor::Crosshair,
                    7 => components::UiCursor::EwResize,
                    8 => components::UiCursor::NsResize,
                    9 => components::UiCursor::Move,
                    _ => components::UiCursor::Default,
                };
            }
        },
    }]
}

/// Declarative fields for `CheckboxData`.
pub fn checkbox_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::bool_field!("Checked", components::CheckboxData, checked),
        renzora::string_field!("Label", components::CheckboxData, label),
        renzora::color_rgba_field!("Check Color", components::CheckboxData, check_color),
        renzora::color_rgba_field!("Box Color", components::CheckboxData, box_color),
    ]
}

/// Declarative fields for `ToggleData`.
pub fn toggle_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::bool_field!("On", components::ToggleData, on),
        renzora::string_field!("Label", components::ToggleData, label),
        renzora::color_rgba_field!("On Color", components::ToggleData, on_color),
        renzora::color_rgba_field!("Off Color", components::ToggleData, off_color),
        renzora::color_rgba_field!("Knob Color", components::ToggleData, knob_color),
    ]
}

/// Declarative fields for `RadioButtonData`.
pub fn radio_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::string_field!("Group", components::RadioButtonData, group),
        renzora::bool_field!("Selected", components::RadioButtonData, selected),
        renzora::string_field!("Label", components::RadioButtonData, label),
        renzora::color_rgba_field!("Active Color", components::RadioButtonData, active_color),
    ]
}

/// Declarative fields for `TextInputData`.
pub fn text_input_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::string_field!("Text", components::TextInputData, text),
        renzora::string_field!("Placeholder", components::TextInputData, placeholder),
        renzora::int_field!("Max Length", components::TextInputData, max_length, usize, 1.0, 1.0, 10000.0),
        renzora::bool_field!("Password", components::TextInputData, password),
    ]
}

/// Declarative fields for `ScrollViewData`.
pub fn scroll_view_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::float_field!("Scroll Speed", components::ScrollViewData, scroll_speed, 0.5, 1.0, 200.0),
        renzora::bool_field!("Horizontal", components::ScrollViewData, show_horizontal),
        renzora::bool_field!("Vertical", components::ScrollViewData, show_vertical),
    ]
}

/// Declarative fields for `TooltipData`.
pub fn tooltip_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::string_field!("Text", components::TooltipData, text),
        renzora::int_field!("Delay (ms)", components::TooltipData, delay_ms, u32, 1.0, 0.0, 5000.0),
        renzora::color_rgba_field!("Bg Color", components::TooltipData, bg_color),
        renzora::color_rgba_field!("Text Color", components::TooltipData, text_color),
    ]
}

/// Declarative fields for `ModalData`.
pub fn modal_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::string_field!("Title", components::ModalData, title),
        renzora::bool_field!("Closable", components::ModalData, closable),
        renzora::color_rgba_field!("Backdrop", components::ModalData, backdrop_color),
    ]
}

/// Declarative fields for `DraggableWindowData`.
pub fn draggable_window_fields() -> Vec<renzora::FieldDef> {
    vec![
        renzora::string_field!("Title", components::DraggableWindowData, title),
        renzora::bool_field!("Closable", components::DraggableWindowData, closable),
        renzora::bool_field!("Minimizable", components::DraggableWindowData, minimizable),
        renzora::color_rgba_field!("Title Bar", components::DraggableWindowData, title_bar_color),
    ]
}

