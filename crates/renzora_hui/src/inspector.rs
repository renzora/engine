//! Inspector entries for bevy_ui style components, gated on [`MarkupSource`].
//!
//! Every `set_fn` does two things:
//!
//! 1. Update the live bevy component (so the entity in the viewport reflects
//!    the change immediately).
//! 2. Patch the source `.html` via [`crate::writeback::write_attr_to_markup`]
//!    so the markup-as-source-of-truth invariant holds.
//!
//! Field coverage is currently a focused subset of the `StyleAttr` variants
//! the loader handles — the attributes that appear in `<text>` / `<node>` /
//! `<button>` / `<image>` tags in real templates. Expansion (grid_*, flex_basis,
//! min/max_width/height, etc.) is mechanical follow-up.

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::{
    AlignItems, BorderRadius, Display, FlexDirection, JustifyContent, PositionType, UiRect, Val,
};
use egui_phosphor::regular;
use renzora_editor::{
    AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry,
};

use crate::provenance::MarkupSource;
use crate::writeback::write_attr_to_markup;

/// Build a `FieldDef` for a `Val`-typed `Node` field. Macro instead of a
/// helper fn because `FieldDef::set_fn` is a `fn` pointer and so cannot
/// close over `$markup` — the identifier has to be stamped in at expansion.
macro_rules! val_node_field {
    ($display:literal, $markup:literal, $field:ident) => {
        FieldDef {
            name: $display,
            field_type: FieldType::String,
            get_fn: |world, entity| {
                world
                    .get::<Node>(entity)
                    .map(|n| FieldValue::String(val_to_string(n.$field)))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::String(s) = val {
                    if let Some(new) = string_to_val(&s) {
                        if let Some(mut n) = world.get_mut::<Node>(entity) {
                            n.$field = new;
                        }
                        write_attr_to_markup(world, entity, $markup, &s);
                    }
                }
            },
        }
    };
}

/// Counterpart for `UiRect`-typed `Node` fields. Same rationale as
/// [`val_node_field`].
macro_rules! ui_rect_node_field {
    ($display:literal, $markup:literal, $field:ident) => {
        FieldDef {
            name: $display,
            field_type: FieldType::String,
            get_fn: |world, entity| {
                world
                    .get::<Node>(entity)
                    .map(|n| FieldValue::String(ui_rect_to_string(n.$field)))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::String(s) = val {
                    if let Some(new) = string_to_ui_rect(&s) {
                        if let Some(mut n) = world.get_mut::<Node>(entity) {
                            n.$field = new;
                        }
                        write_attr_to_markup(world, entity, $markup, &s);
                    }
                }
            },
        }
    };
}

pub struct HuiInspectorPlugin;

impl Plugin for HuiInspectorPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspector(node_entry());
        app.register_inspector(background_color_entry());
        app.register_inspector(border_color_entry());
        app.register_inspector(text_font_entry());
        app.register_inspector(text_color_entry());
        app.register_inspector(image_node_entry());
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Only show the HUI inspectors on entities that came from markup. Legacy
/// `UiWidget` spawns keep their existing inspectors so we don't double up.
fn has_markup_source(world: &World, entity: Entity) -> bool {
    world.get::<MarkupSource>(entity).is_some()
}

/// `[r, g, b]` (sRGB linear floats, alpha-less) → `"#RRGGBB"` — the form
/// `parse_color` accepts and matches the user's example
/// (`font_color="#8A93A2"`).
fn color_to_hex(srgb: [f32; 3]) -> String {
    let r = (srgb[0].clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (srgb[1].clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (srgb[2].clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

fn color_to_srgb_array(c: Color) -> [f32; 3] {
    let s = c.to_srgba();
    [s.red, s.green, s.blue]
}

/// `"100%"` / `"50px"` / `"auto"` / `"10vw"` etc. — the same format the
/// bevy_hui parser accepts via `parse_val`. We re-implement here so the
/// inspector doesn't depend on parser internals.
fn val_to_string(v: Val) -> String {
    match v {
        Val::Auto => "auto".to_string(),
        Val::Px(n) => format!("{}px", trim_float(n)),
        Val::Percent(n) => format!("{}%", trim_float(n)),
        Val::Vw(n) => format!("{}vw", trim_float(n)),
        Val::Vh(n) => format!("{}vh", trim_float(n)),
        Val::VMin(n) => format!("{}vmin", trim_float(n)),
        Val::VMax(n) => format!("{}vmax", trim_float(n)),
    }
}

fn string_to_val(s: &str) -> Option<Val> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("auto") {
        return Some(Val::Auto);
    }
    let (num_str, unit): (&str, &str) = if let Some(rest) = s.strip_suffix("px") {
        (rest, "px")
    } else if let Some(rest) = s.strip_suffix('%') {
        (rest, "%")
    } else if let Some(rest) = s.strip_suffix("vw") {
        (rest, "vw")
    } else if let Some(rest) = s.strip_suffix("vh") {
        (rest, "vh")
    } else if let Some(rest) = s.strip_suffix("vmin") {
        (rest, "vmin")
    } else if let Some(rest) = s.strip_suffix("vmax") {
        (rest, "vmax")
    } else {
        // Bare number → pixels (parser convention).
        (s, "px")
    };
    let n: f32 = num_str.trim().parse().ok()?;
    Some(match unit {
        "px" => Val::Px(n),
        "%" => Val::Percent(n),
        "vw" => Val::Vw(n),
        "vh" => Val::Vh(n),
        "vmin" => Val::VMin(n),
        "vmax" => Val::VMax(n),
        _ => unreachable!(),
    })
}

/// Strip trailing zeros from a float for prettier serialization
/// (`12.0` → `"12"`, `12.5` → `"12.5"`). Matters because the user originally
/// wrote `font_size="12"` — round-tripping to `"12.0"` would feel noisy.
fn trim_float(n: f32) -> String {
    if n.fract().abs() < f32::EPSILON {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// UiRect → string in the form `parse_ui_rect` accepts. Matches the user's
/// `padding="16px"` (all sides equal) shorthand when applicable, falls back
/// to two-axis or four-side forms otherwise. Border-radius corner mapping
/// follows the loader: top→top_left, right→top_right, bottom→bottom_right,
/// left→bottom_left.
fn ui_rect_to_string(rect: UiRect) -> String {
    let t = val_to_string(rect.top);
    let r = val_to_string(rect.right);
    let b = val_to_string(rect.bottom);
    let l = val_to_string(rect.left);
    if t == r && r == b && b == l {
        t
    } else if t == b && l == r {
        format!("{} {}", t, l)
    } else {
        format!("{} {} {} {}", t, r, b, l)
    }
}

fn string_to_ui_rect(s: &str) -> Option<UiRect> {
    let parts: Vec<Val> = s.split_whitespace().filter_map(string_to_val).collect();
    match parts.as_slice() {
        [all] => Some(UiRect::all(*all)),
        [v, h] => Some(UiRect::axes(*h, *v)),
        [t, r, b, l] => Some(UiRect {
            top: *t,
            right: *r,
            bottom: *b,
            left: *l,
        }),
        _ => None,
    }
}

/// Same border-radius corner mapping the loader uses (see `loader::apply_xnode_to`),
/// expressed back out as a `UiRect` shape so we can reuse `ui_rect_to_string`.
fn border_radius_to_string(br: BorderRadius) -> String {
    ui_rect_to_string(UiRect {
        top: br.top_left,
        right: br.top_right,
        bottom: br.bottom_right,
        left: br.bottom_left,
    })
}

fn string_to_border_radius(s: &str) -> Option<BorderRadius> {
    let r = string_to_ui_rect(s)?;
    Some(BorderRadius {
        top_left: r.top,
        top_right: r.right,
        bottom_right: r.bottom,
        bottom_left: r.left,
    })
}

// Enum string labels match the bevy_hui parser's `tag(...)` values exactly
// so a roundtrip (read string → set bevy enum → write string) is lossless.

const FLEX_DIRECTION_OPTIONS: &[&str] = &["row", "column", "row_reverse", "column_reverse"];
fn flex_direction_to_str(d: FlexDirection) -> &'static str {
    match d {
        FlexDirection::Row => "row",
        FlexDirection::Column => "column",
        FlexDirection::RowReverse => "row_reverse",
        FlexDirection::ColumnReverse => "column_reverse",
    }
}
fn str_to_flex_direction(s: &str) -> Option<FlexDirection> {
    match s {
        "row" => Some(FlexDirection::Row),
        "column" => Some(FlexDirection::Column),
        "row_reverse" => Some(FlexDirection::RowReverse),
        "column_reverse" => Some(FlexDirection::ColumnReverse),
        _ => None,
    }
}

const POSITION_TYPE_OPTIONS: &[&str] = &["absolute", "relative"];
fn position_type_to_str(p: PositionType) -> &'static str {
    match p {
        PositionType::Absolute => "absolute",
        PositionType::Relative => "relative",
    }
}
fn str_to_position_type(s: &str) -> Option<PositionType> {
    match s {
        "absolute" => Some(PositionType::Absolute),
        "relative" => Some(PositionType::Relative),
        _ => None,
    }
}

const DISPLAY_OPTIONS: &[&str] = &["flex", "grid", "block", "none"];
fn display_to_str(d: Display) -> &'static str {
    match d {
        Display::Flex => "flex",
        Display::Grid => "grid",
        Display::Block => "block",
        Display::None => "none",
    }
}
fn str_to_display(s: &str) -> Option<Display> {
    match s {
        "flex" => Some(Display::Flex),
        "grid" => Some(Display::Grid),
        "block" => Some(Display::Block),
        "none" => Some(Display::None),
        _ => None,
    }
}

const ALIGN_ITEMS_OPTIONS: &[&str] = &[
    "default",
    "start",
    "end",
    "flex_start",
    "flex_end",
    "center",
    "stretch",
    "baseline",
];
fn align_items_to_str(a: AlignItems) -> &'static str {
    match a {
        AlignItems::Default => "default",
        AlignItems::Start => "start",
        AlignItems::End => "end",
        AlignItems::FlexStart => "flex_start",
        AlignItems::FlexEnd => "flex_end",
        AlignItems::Center => "center",
        AlignItems::Stretch => "stretch",
        AlignItems::Baseline => "baseline",
    }
}
fn str_to_align_items(s: &str) -> Option<AlignItems> {
    match s {
        "default" => Some(AlignItems::Default),
        "start" => Some(AlignItems::Start),
        "end" => Some(AlignItems::End),
        "flex_start" => Some(AlignItems::FlexStart),
        "flex_end" => Some(AlignItems::FlexEnd),
        "center" => Some(AlignItems::Center),
        "stretch" => Some(AlignItems::Stretch),
        "baseline" => Some(AlignItems::Baseline),
        _ => None,
    }
}

const JUSTIFY_CONTENT_OPTIONS: &[&str] = &[
    "start",
    "end",
    "flex_start",
    "flex_end",
    "center",
    "stretch",
    "space_between",
    "space_around",
    "space_evenly",
];
fn justify_content_to_str(j: JustifyContent) -> &'static str {
    match j {
        JustifyContent::Start => "start",
        JustifyContent::End => "end",
        JustifyContent::FlexStart => "flex_start",
        JustifyContent::FlexEnd => "flex_end",
        JustifyContent::Center => "center",
        JustifyContent::Stretch => "stretch",
        JustifyContent::SpaceBetween => "space_between",
        JustifyContent::SpaceAround => "space_around",
        JustifyContent::SpaceEvenly => "space_evenly",
        JustifyContent::Default => "start",
    }
}
fn str_to_justify_content(s: &str) -> Option<JustifyContent> {
    match s {
        "start" => Some(JustifyContent::Start),
        "end" => Some(JustifyContent::End),
        "flex_start" => Some(JustifyContent::FlexStart),
        "flex_end" => Some(JustifyContent::FlexEnd),
        "center" => Some(JustifyContent::Center),
        "stretch" => Some(JustifyContent::Stretch),
        "space_between" => Some(JustifyContent::SpaceBetween),
        "space_around" => Some(JustifyContent::SpaceAround),
        "space_evenly" => Some(JustifyContent::SpaceEvenly),
        _ => None,
    }
}

// ── InspectorEntry constructors ────────────────────────────────────────────

fn node_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "hui_node",
        display_name: "Node",
        icon: regular::FRAME_CORNERS,
        category: "ui",
        // Show on every HUI-built entity (they all have a Node). Don't offer
        // add/remove — Node is required infrastructure, removing it would
        // strip layout entirely.
        has_fn: |world, entity| {
            has_markup_source(world, entity) && world.get::<Node>(entity).is_some()
        },
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            // -- Enum-pickered layout knobs --
            FieldDef {
                name: "Flex Direction",
                field_type: FieldType::Enum {
                    options: FLEX_DIRECTION_OPTIONS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Node>(entity)
                        .map(|n| FieldValue::Enum(flex_direction_to_str(n.flex_direction).into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(s) = val {
                        if let Some(new) = str_to_flex_direction(&s) {
                            if let Some(mut n) = world.get_mut::<Node>(entity) {
                                n.flex_direction = new;
                            }
                            write_attr_to_markup(world, entity, "flex_direction", &s);
                        }
                    }
                },
            },
            FieldDef {
                name: "Position",
                field_type: FieldType::Enum {
                    options: POSITION_TYPE_OPTIONS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Node>(entity)
                        .map(|n| FieldValue::Enum(position_type_to_str(n.position_type).into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(s) = val {
                        if let Some(new) = str_to_position_type(&s) {
                            if let Some(mut n) = world.get_mut::<Node>(entity) {
                                n.position_type = new;
                            }
                            write_attr_to_markup(world, entity, "position", &s);
                        }
                    }
                },
            },
            FieldDef {
                name: "Display",
                field_type: FieldType::Enum {
                    options: DISPLAY_OPTIONS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Node>(entity)
                        .map(|n| FieldValue::Enum(display_to_str(n.display).into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(s) = val {
                        if let Some(new) = str_to_display(&s) {
                            if let Some(mut n) = world.get_mut::<Node>(entity) {
                                n.display = new;
                            }
                            write_attr_to_markup(world, entity, "display", &s);
                        }
                    }
                },
            },
            FieldDef {
                name: "Align Items",
                field_type: FieldType::Enum {
                    options: ALIGN_ITEMS_OPTIONS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Node>(entity)
                        .map(|n| FieldValue::Enum(align_items_to_str(n.align_items).into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(s) = val {
                        if let Some(new) = str_to_align_items(&s) {
                            if let Some(mut n) = world.get_mut::<Node>(entity) {
                                n.align_items = new;
                            }
                            write_attr_to_markup(world, entity, "align_items", &s);
                        }
                    }
                },
            },
            FieldDef {
                name: "Justify Content",
                field_type: FieldType::Enum {
                    options: JUSTIFY_CONTENT_OPTIONS,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Node>(entity)
                        .map(|n| FieldValue::Enum(justify_content_to_str(n.justify_content).into()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Enum(s) = val {
                        if let Some(new) = str_to_justify_content(&s) {
                            if let Some(mut n) = world.get_mut::<Node>(entity) {
                                n.justify_content = new;
                            }
                            write_attr_to_markup(world, entity, "justify_content", &s);
                        }
                    }
                },
            },
            // -- Val-typed sizing/position, edited as String --
            //
            // Each one is fully inlined because `FieldDef::set_fn` is a `fn`
            // pointer, not a closure — the markup identifier can't be
            // captured at runtime so we stamp it as a literal in each arm.
            val_node_field!("Width", "width", width),
            val_node_field!("Height", "height", height),
            val_node_field!("Left", "left", left),
            val_node_field!("Right", "right", right),
            val_node_field!("Top", "top", top),
            val_node_field!("Bottom", "bottom", bottom),
            val_node_field!("Row Gap", "row_gap", row_gap),
            val_node_field!("Column Gap", "column_gap", column_gap),
            // -- UiRect-typed --
            ui_rect_node_field!("Padding", "padding", padding),
            ui_rect_node_field!("Margin", "margin", margin),
            ui_rect_node_field!("Border", "border", border),
            FieldDef {
                name: "Border Radius",
                field_type: FieldType::String,
                get_fn: |world, entity| {
                    world
                        .get::<Node>(entity)
                        .map(|n| FieldValue::String(border_radius_to_string(n.border_radius)))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::String(s) = val {
                        if let Some(new) = string_to_border_radius(&s) {
                            if let Some(mut n) = world.get_mut::<Node>(entity) {
                                n.border_radius = new;
                            }
                            write_attr_to_markup(world, entity, "border_radius", &s);
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

fn background_color_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "hui_background_color",
        display_name: "Background",
        icon: regular::PAINT_BUCKET,
        category: "ui",
        has_fn: |world, entity| {
            has_markup_source(world, entity) && world.get::<BackgroundColor>(entity).is_some()
        },
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(BackgroundColor(Color::WHITE));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<BackgroundColor>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Color",
            field_type: FieldType::Color,
            get_fn: |world, entity| {
                world
                    .get::<BackgroundColor>(entity)
                    .map(|c| FieldValue::Color(color_to_srgb_array(c.0)))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Color(rgb) = val {
                    let color = Color::srgb(rgb[0], rgb[1], rgb[2]);
                    if let Some(mut bg) = world.get_mut::<BackgroundColor>(entity) {
                        bg.0 = color;
                    }
                    write_attr_to_markup(world, entity, "background", &color_to_hex(rgb));
                }
            },
        }],
        custom_ui_fn: None,
    }
}

fn border_color_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "hui_border_color",
        display_name: "Border Color",
        icon: regular::SQUARE,
        category: "ui",
        has_fn: |world, entity| {
            has_markup_source(world, entity) && world.get::<BorderColor>(entity).is_some()
        },
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(BorderColor::all(Color::WHITE));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<BorderColor>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Color",
            field_type: FieldType::Color,
            // BorderColor in 0.18 carries per-side colors; we expose the top
            // side (which matches `BorderColor::all` symmetry) and write all
            // four sides together so the markup `border_color="..."` shorthand
            // round-trips.
            get_fn: |world, entity| {
                world
                    .get::<BorderColor>(entity)
                    .map(|c| FieldValue::Color(color_to_srgb_array(c.top)))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Color(rgb) = val {
                    let color = Color::srgb(rgb[0], rgb[1], rgb[2]);
                    if let Some(mut bc) = world.get_mut::<BorderColor>(entity) {
                        *bc = BorderColor::all(color);
                    }
                    write_attr_to_markup(world, entity, "border_color", &color_to_hex(rgb));
                }
            },
        }],
        custom_ui_fn: None,
    }
}

fn text_font_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "hui_text_font",
        display_name: "Text Font",
        icon: regular::TEXT_AA,
        category: "ui",
        has_fn: |world, entity| {
            has_markup_source(world, entity) && world.get::<TextFont>(entity).is_some()
        },
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(TextFont::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<TextFont>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Font Size",
            field_type: FieldType::Float {
                speed: 0.5,
                min: 1.0,
                max: 512.0,
            },
            get_fn: |world, entity| {
                world
                    .get::<TextFont>(entity)
                    .map(|f| FieldValue::Float(f.font_size))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Float(v) = val {
                    if let Some(mut f) = world.get_mut::<TextFont>(entity) {
                        f.font_size = v;
                    }
                    write_attr_to_markup(world, entity, "font_size", &trim_float(v));
                }
            },
        }],
        custom_ui_fn: None,
    }
}

fn text_color_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "hui_text_color",
        display_name: "Text Color",
        icon: regular::PALETTE,
        category: "ui",
        has_fn: |world, entity| {
            has_markup_source(world, entity) && world.get::<TextColor>(entity).is_some()
        },
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(TextColor(Color::WHITE));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<TextColor>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Color",
            field_type: FieldType::Color,
            get_fn: |world, entity| {
                world
                    .get::<TextColor>(entity)
                    .map(|c| FieldValue::Color(color_to_srgb_array(c.0)))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Color(rgb) = val {
                    let color = Color::srgb(rgb[0], rgb[1], rgb[2]);
                    if let Some(mut tc) = world.get_mut::<TextColor>(entity) {
                        tc.0 = color;
                    }
                    write_attr_to_markup(world, entity, "font_color", &color_to_hex(rgb));
                }
            },
        }],
        custom_ui_fn: None,
    }
}

fn image_node_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "hui_image_node",
        display_name: "UI Image",
        icon: regular::IMAGE,
        category: "ui",
        has_fn: |world, entity| {
            has_markup_source(world, entity) && world.get::<ImageNode>(entity).is_some()
        },
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(ImageNode {
                image_mode: NodeImageMode::Auto,
                ..default()
            });
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<ImageNode>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Source",
            field_type: FieldType::Asset {
                extensions: vec![
                    "png".into(),
                    "jpg".into(),
                    "jpeg".into(),
                    "webp".into(),
                    "ktx2".into(),
                ],
            },
            get_fn: |_world, _entity| {
                // Image handle → asset path lookup is one-way without holding
                // the AssetServer; the asset field's drag-drop UX is the
                // primary path for setting this so we surface an empty slot
                // here. Future: track the source string in a sidecar
                // component (`UiImagePath` already exists in game_ui for the
                // non-markup case).
                Some(FieldValue::Asset(None))
            },
            set_fn: |world, entity, val| {
                if let FieldValue::Asset(Some(path)) = val {
                    let handle: Handle<Image> = world.resource::<AssetServer>().load(&path);
                    if let Some(mut img) = world.get_mut::<ImageNode>(entity) {
                        img.image = handle;
                    }
                    write_attr_to_markup(world, entity, "src", &path);
                }
            },
        }],
        custom_ui_fn: None,
    }
}
