//! UI Inspector — property editor for selected UiWidget / UiCanvas entities.
//!
//! No longer a standalone panel; exposes `render_ui_inspector` which the main
//! inspector calls via an `InspectorEntry` whose `has_fn` matches UI entities.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{inline_property, EditorCommands, EditorSelection};
use renzora_theme::Theme;

use crate::components::{self, UiCanvas, UiWidget};

/// Snapshot of the selected widget's properties for editing.
/// Convert a Val to design-space pixels given a reference dimension.
fn val_to_design_px(v: bevy::ui::Val, reference: f32) -> f32 {
    match v {
        bevy::ui::Val::Percent(p) => p * reference / 100.0,
        bevy::ui::Val::Px(px) => px,
        _ => 0.0,
    }
}

fn val_px(v: bevy::ui::Val) -> f32 {
    match v {
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

/// Helper: convert a `Color` to `[f32; 4]` RGBA.
fn color_to_arr(c: Color) -> [f32; 4] {
    c.to_srgba().to_f32_array()
}

/// Helper: convert `[f32; 4]` RGBA to `Color`.
fn arr_to_color(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
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
pub fn layout_fields() -> Vec<renzora_editor::FieldDef> {
    use bevy::ui::{Node, Val};
    use renzora_editor::{FieldDef, FieldType, FieldValue};

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

/// Inspector for `UiCanvas` — sort order, visibility mode, reference resolution,
/// and a global theme picker. Registered as its own InspectorEntry; the main
/// inspector wraps the body in a collapsible automatically.
pub fn render_canvas_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(canvas) = world.get::<UiCanvas>(entity) else {
        return;
    };
    let theme = theme.clone();
    let sort_order = canvas.sort_order;
    let visibility_mode = canvas.visibility_mode.clone();
    let reference_width = canvas.reference_width;
    let reference_height = canvas.reference_height;

    let mut row = 0;
    inline_property(ui, row, "Sort Order", &theme, |ui| {
        let mut v = sort_order;
        if ui
            .add(egui::DragValue::new(&mut v).range(-100..=100))
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                        canvas.sort_order = v;
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Visibility", &theme, |ui| {
        let modes = ["always", "play_only", "editor_only"];
        let mut idx = modes
            .iter()
            .position(|m| *m == visibility_mode)
            .unwrap_or(0);
        if egui::ComboBox::from_id_salt("vis_mode")
            .width(ui.available_width())
            .show_index(ui, &mut idx, modes.len(), |i| modes[i].to_string())
            .changed()
        {
            let mode = modes[idx].to_string();
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                        canvas.visibility_mode = mode.clone();
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Ref Width", &theme, |ui| {
        let mut v = reference_width;
        if ui
            .add(egui::DragValue::new(&mut v).speed(1.0).range(1.0..=7680.0))
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                        canvas.reference_width = v;
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Ref Height", &theme, |ui| {
        let mut v = reference_height;
        if ui
            .add(egui::DragValue::new(&mut v).speed(1.0).range(1.0..=4320.0))
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                        canvas.reference_height = v;
                    }
                }
            });
        }
    });
    row += 1;
    let current_theme_name = world
        .get_resource::<components::UiTheme>()
        .map(|t| t.name.clone())
        .unwrap_or_default();
    inline_property(ui, row, "Theme", &theme, |ui| {
        let themes = ["Dark", "Light", "High Contrast"];
        let mut idx = themes
            .iter()
            .position(|t| *t == current_theme_name)
            .unwrap_or(0);
        if egui::ComboBox::from_id_salt("ui_theme")
            .width(ui.available_width())
            .show_index(ui, &mut idx, themes.len(), |i| themes[i].to_string())
            .changed()
        {
            let theme_idx = idx;
            commands.push(move |world: &mut World| {
                let new_theme = match theme_idx {
                    1 => components::UiTheme::light(),
                    2 => components::UiTheme::high_contrast(),
                    _ => components::UiTheme::dark(),
                };
                world.insert_resource(new_theme);
            });
        }
    });
}

/// Inspector for `UiWidget` — widget type display, locked toggle, and a delete
/// button that despawns the entity (since removing the UiWidget component alone
/// would leave a stranded entity).
pub fn render_widget_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(widget) = world.get::<UiWidget>(entity) else {
        return;
    };
    let theme = theme.clone();
    let widget_type_label = widget.widget_type.label().to_string();
    let locked = widget.locked;

    let mut row = 0;
    inline_property(ui, row, "Type", &theme, |ui| {
        ui.label(egui::RichText::new(widget_type_label.clone()).size(11.0));
    });
    row += 1;
    inline_property(ui, row, "Locked", &theme, |ui| {
        let mut v = locked;
        if ui.checkbox(&mut v, "").changed() {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut w) = em.get_mut::<UiWidget>() {
                        w.locked = v;
                    }
                }
            });
        }
    });

    ui.add_space(6.0);
    let delete_btn = egui::Button::new(
        egui::RichText::new(format!("{} Delete Widget", regular::TRASH))
            .size(11.0)
            .color(theme.semantic.error.to_color32()),
    )
    .min_size(egui::vec2(ui.available_width(), 22.0));
    if ui.add(delete_btn).clicked() {
        commands.push(move |world: &mut World| {
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
            }
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(None);
            }
        });
    }
}

/// Inspector for the bevy_ui `Node` (layout) on a UI entity. Position type,
/// X/Y/W/H in design pixels (stored as percent of the resolved canvas
/// reference resolution), and flex direction/justify/align.
pub fn render_layout_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(node) = world.get::<Node>(entity) else {
        return;
    };
    let theme = theme.clone();

    // Resolve canvas reference resolution from parent canvas (or self).
    let (crw, crh) = {
        let mut w = 1280.0_f32;
        let mut h = 720.0_f32;
        if let Some(canvas) = world.get::<UiCanvas>(entity) {
            w = canvas.reference_width;
            h = canvas.reference_height;
        } else if let Some(child_of) = world.get::<ChildOf>(entity) {
            if let Some(canvas) = world.get::<UiCanvas>(child_of.parent()) {
                w = canvas.reference_width;
                h = canvas.reference_height;
            }
        }
        (w, h)
    };

    let position_type = position_type_to_u8(node.position_type);
    let left = val_to_design_px(node.left, crw);
    let top = val_to_design_px(node.top, crh);
    let width = val_to_design_px(node.width, crw);
    let height = val_to_design_px(node.height, crh);
    let flex_direction = flex_direction_to_u8(node.flex_direction);
    let justify_content = justify_content_to_u8(node.justify_content);
    let align_items = align_items_to_u8(node.align_items);

    let mut row = 0;
    inline_property(ui, row, "Position", &theme, |ui| {
        let labels = ["Relative", "Absolute"];
        let mut idx = position_type as usize;
        if egui::ComboBox::from_id_salt("pos_type")
            .width(ui.available_width())
            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.position_type = u8_to_position_type(idx as u8);
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "X", &theme, |ui| {
        let mut v = left;
        if ui.add(egui::DragValue::new(&mut v).speed(1.0)).changed() {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.left = bevy::ui::Val::Percent(v / crw * 100.0);
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Y", &theme, |ui| {
        let mut v = top;
        if ui.add(egui::DragValue::new(&mut v).speed(1.0)).changed() {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.top = bevy::ui::Val::Percent(v / crh * 100.0);
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Width", &theme, |ui| {
        let mut v = width;
        if ui
            .add(
                egui::DragValue::new(&mut v)
                    .speed(1.0)
                    .range(0.0..=f32::MAX),
            )
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.width = bevy::ui::Val::Percent(v / crw * 100.0);
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Height", &theme, |ui| {
        let mut v = height;
        if ui
            .add(
                egui::DragValue::new(&mut v)
                    .speed(1.0)
                    .range(0.0..=f32::MAX),
            )
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.height = bevy::ui::Val::Percent(v / crh * 100.0);
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Direction", &theme, |ui| {
        let labels = ["Row", "Column", "Row Rev", "Col Rev"];
        let mut idx = flex_direction as usize;
        if egui::ComboBox::from_id_salt("flex_dir")
            .width(ui.available_width())
            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.flex_direction = u8_to_flex_direction(idx as u8);
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Justify", &theme, |ui| {
        let labels = ["Start", "Center", "End", "Between", "Around", "Evenly"];
        let mut idx = justify_content as usize;
        if egui::ComboBox::from_id_salt("justify")
            .width(ui.available_width())
            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.justify_content = u8_to_justify_content(idx as u8);
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Align", &theme, |ui| {
        let labels = ["Start", "Center", "End", "Stretch"];
        let mut idx = align_items as usize;
        if egui::ComboBox::from_id_salt("align")
            .width(ui.available_width())
            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut node) = em.get_mut::<Node>() {
                        node.align_items = u8_to_align_items(idx as u8);
                    }
                }
            });
        }
    });
}

// ── Fill section ─────────────────────────────────────────────────────────────

pub fn render_fill_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut fill_owned) = world.get::<components::UiFill>(entity).cloned() else {
        return;
    };
    let fill: &mut components::UiFill = &mut fill_owned;
    {
        {
            let fill_type_idx = match fill {
                components::UiFill::None => 0,
                components::UiFill::Solid(_) => 1,
                components::UiFill::LinearGradient { .. } => 2,
                components::UiFill::RadialGradient { .. } => 3,
            };
            let mut idx = fill_type_idx;
            inline_property(ui, 0, "Type", theme, |ui| {
                let labels = ["None", "Solid", "Linear", "Radial"];
                if egui::ComboBox::from_id_salt("fill_type")
                    .width(ui.available_width())
                    .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
                    .changed()
                {
                    *fill = match idx {
                        0 => components::UiFill::None,
                        1 => components::UiFill::Solid(Color::srgba(0.2, 0.2, 0.2, 1.0)),
                        2 => components::UiFill::linear(
                            0.0,
                            Color::srgba(0.2, 0.2, 0.8, 1.0),
                            Color::srgba(0.8, 0.2, 0.2, 1.0),
                        ),
                        3 => components::UiFill::RadialGradient {
                            center: [0.5, 0.5],
                            stops: vec![
                                components::GradientStop {
                                    position: 0.0,
                                    color: Color::WHITE,
                                },
                                components::GradientStop {
                                    position: 1.0,
                                    color: Color::BLACK,
                                },
                            ],
                        },
                        _ => components::UiFill::None,
                    };
                    let new_fill = fill.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut f) = em.get_mut::<components::UiFill>() {
                                *f = new_fill;
                            }
                        }
                    });
                }
            });

            match fill {
                components::UiFill::Solid(ref mut color) => {
                    inline_property(ui, 1, "Color", theme, |ui| {
                        let mut arr = color_to_arr(*color);
                        if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                            *color = arr_to_color(arr);
                            let c = *color;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut f) = em.get_mut::<components::UiFill>() {
                                        *f = components::UiFill::Solid(c);
                                    }
                                }
                            });
                        }
                    });
                }
                components::UiFill::LinearGradient {
                    ref mut angle,
                    ref mut stops,
                } => {
                    let mut fill_changed = false;
                    inline_property(ui, 1, "Angle", theme, |ui| {
                        let mut v = *angle;
                        if ui
                            .add(
                                egui::DragValue::new(&mut v)
                                    .speed(1.0)
                                    .range(0.0..=360.0)
                                    .suffix("°"),
                            )
                            .changed()
                        {
                            *angle = v;
                            fill_changed = true;
                        }
                    });
                    if gradient_stops_editor(ui, stops, 2, theme) {
                        fill_changed = true;
                    }
                    if fill_changed {
                        let new_fill = fill.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut f) = em.get_mut::<components::UiFill>() {
                                    *f = new_fill;
                                }
                            }
                        });
                    }
                }
                components::UiFill::RadialGradient {
                    ref mut center,
                    ref mut stops,
                } => {
                    let mut fill_changed = false;
                    inline_property(ui, 1, "Center X", theme, |ui| {
                        let mut v = center[0];
                        if ui
                            .add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0))
                            .changed()
                        {
                            center[0] = v;
                            fill_changed = true;
                        }
                    });
                    inline_property(ui, 2, "Center Y", theme, |ui| {
                        let mut v = center[1];
                        if ui
                            .add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0))
                            .changed()
                        {
                            center[1] = v;
                            fill_changed = true;
                        }
                    });
                    if gradient_stops_editor(ui, stops, 3, theme) {
                        fill_changed = true;
                    }
                    if fill_changed {
                        let new_fill = fill.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut f) = em.get_mut::<components::UiFill>() {
                                    *f = new_fill;
                                }
                            }
                        });
                    }
                }
                _ => {}
            }
        }
    }
}

/// Renders gradient stop editors. Returns true if any stop was modified.
fn gradient_stops_editor(
    ui: &mut egui::Ui,
    stops: &mut Vec<components::GradientStop>,
    start_row: usize,
    theme: &renzora_theme::Theme,
) -> bool {
    let mut changed = false;
    for (i, stop) in stops.iter_mut().enumerate() {
        let row = start_row + i * 2;
        inline_property(ui, row, &format!("Stop {} Pos", i + 1), theme, |ui| {
            let mut v = stop.position;
            if ui
                .add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0))
                .changed()
            {
                stop.position = v;
                changed = true;
            }
        });
        inline_property(ui, row + 1, &format!("Stop {} Color", i + 1), theme, |ui| {
            let mut arr = color_to_arr(stop.color);
            if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                stop.color = arr_to_color(arr);
                changed = true;
            }
        });
    }

    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui
            .small_button(format!("{} Add Stop", regular::PLUS))
            .clicked()
        {
            stops.push(components::GradientStop {
                position: 1.0,
                color: Color::WHITE,
            });
            changed = true;
        }
        if stops.len() > 2
            && ui
                .small_button(format!("{} Remove", regular::MINUS))
                .clicked()
            {
                stops.pop();
                changed = true;
            }
    });

    changed
}

// ── Stroke section ───────────────────────────────────────────────────────────

pub fn render_stroke_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut stroke_owned) = world.get::<components::UiStroke>(entity).cloned() else {
        return;
    };
    let stroke: &mut components::UiStroke = &mut stroke_owned;
    {
        {
            inline_property(ui, 0, "Color", theme, |ui| {
                let mut arr = color_to_arr(stroke.color);
                if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                    stroke.color = arr_to_color(arr);
                    let new_stroke = stroke.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut s) = em.get_mut::<components::UiStroke>() {
                                *s = new_stroke;
                            }
                        }
                    });
                }
            });
            inline_property(ui, 1, "Width", theme, |ui| {
                let mut v = stroke.width;
                if ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .speed(0.5)
                            .range(0.0..=50.0)
                            .suffix("px"),
                    )
                    .changed()
                {
                    stroke.width = v;
                    let new_stroke = stroke.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut s) = em.get_mut::<components::UiStroke>() {
                                *s = new_stroke;
                            }
                        }
                    });
                }
            });
            inline_property(ui, 2, "Sides", theme, |ui| {
                ui.horizontal(|ui| {
                    let mut changed = false;
                    // Arrow-to-line icons indicate which edge the border
                    // applies to. Up = top, Down = bottom, etc. Tooltip
                    // labels make the meaning explicit on hover.
                    let side_buttons = [
                        (regular::ARROW_LINE_UP, "Top", &mut stroke.sides.top),
                        (regular::ARROW_LINE_RIGHT, "Right", &mut stroke.sides.right),
                        (regular::ARROW_LINE_DOWN, "Bottom", &mut stroke.sides.bottom),
                        (regular::ARROW_LINE_LEFT, "Left", &mut stroke.sides.left),
                    ];
                    for (icon, tooltip, val) in side_buttons {
                        let resp = ui.selectable_label(*val, icon).on_hover_text(tooltip);
                        if resp.clicked() {
                            *val = !*val;
                            changed = true;
                        }
                    }
                    if changed {
                        let new_stroke = stroke.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut s) = em.get_mut::<components::UiStroke>() {
                                    *s = new_stroke;
                                }
                            }
                        });
                    }
                });
            });
        }
    }
}

// ── Border Radius section ────────────────────────────────────────────────────

pub fn render_border_radius_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut border_radius_owned) = world.get::<components::UiBorderRadius>(entity).cloned()
    else {
        return;
    };
    let border_radius: &mut components::UiBorderRadius = &mut border_radius_owned;
    {
        {
            let labels = ["Top Left", "Top Right", "Bottom Right", "Bottom Left"];
            for (i, label) in labels.iter().enumerate() {
                inline_property(ui, i, label, theme, |ui| {
                    let cur = match i {
                        0 => border_radius.top_left,
                        1 => border_radius.top_right,
                        2 => border_radius.bottom_right,
                        _ => border_radius.bottom_left,
                    };
                    let mut v = cur;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .speed(0.5)
                                .range(0.0..=500.0)
                                .suffix("px"),
                        )
                        .changed()
                    {
                        match i {
                            0 => border_radius.top_left = v,
                            1 => border_radius.top_right = v,
                            2 => border_radius.bottom_right = v,
                            _ => border_radius.bottom_left = v,
                        }
                        let new_radius = *border_radius;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut r) = em.get_mut::<components::UiBorderRadius>() {
                                    *r = new_radius;
                                }
                            }
                        });
                    }
                });
            }
        }
    }
}

// ── Text section ─────────────────────────────────────────────────────────────

pub fn render_text_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut text_style_owned) = world.get::<components::UiTextStyle>(entity).cloned() else {
        return;
    };
    let has_text = world.get::<bevy::ui::widget::Text>(entity).is_some();
    let text_content_initial = world
        .get::<bevy::ui::widget::Text>(entity)
        .map(|t| t.0.clone())
        .unwrap_or_default();
    {
        {
            let mut row = 0;
            // Content (writes to bevy Text component)
            if has_text {
                inline_property(ui, row, "Content", theme, |ui| {
                    let mut v = text_content_initial.clone();
                    if ui
                        .add(
                            egui::TextEdit::multiline(&mut v)
                                .desired_width(ui.available_width())
                                .desired_rows(2),
                        )
                        .changed()
                    {
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut text) = em.get_mut::<bevy::ui::widget::Text>() {
                                    text.0 = v.clone();
                                }
                            }
                        });
                    }
                });
                row += 1;
            }

            // Style props from UiTextStyle component
            {
                let text_style: &mut components::UiTextStyle = &mut text_style_owned;
                inline_property(ui, row, "Color", theme, |ui| {
                    let mut arr = color_to_arr(text_style.color);
                    if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                        text_style.color = arr_to_color(arr);
                        let ts = text_style.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut s) = em.get_mut::<components::UiTextStyle>() {
                                    *s = ts;
                                }
                            }
                        });
                    }
                });
                row += 1;
                inline_property(ui, row, "Size", theme, |ui| {
                    let mut v = text_style.size;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .speed(0.5)
                                .range(1.0..=200.0)
                                .suffix("px"),
                        )
                        .changed()
                    {
                        text_style.size = v;
                        let ts = text_style.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut s) = em.get_mut::<components::UiTextStyle>() {
                                    *s = ts;
                                }
                            }
                        });
                    }
                });
                row += 1;
                inline_property(ui, row, "Bold", theme, |ui| {
                    let mut v = text_style.bold;
                    if ui.checkbox(&mut v, "").changed() {
                        text_style.bold = v;
                        let ts = text_style.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut s) = em.get_mut::<components::UiTextStyle>() {
                                    *s = ts;
                                }
                            }
                        });
                    }
                });
                row += 1;
                inline_property(ui, row, "Italic", theme, |ui| {
                    let mut v = text_style.italic;
                    if ui.checkbox(&mut v, "").changed() {
                        text_style.italic = v;
                        let ts = text_style.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut s) = em.get_mut::<components::UiTextStyle>() {
                                    *s = ts;
                                }
                            }
                        });
                    }
                });
                row += 1;
                inline_property(ui, row, "Align", theme, |ui| {
                    let labels = ["Left", "Center", "Right"];
                    let mut idx = match text_style.align {
                        components::UiTextAlign::Left => 0,
                        components::UiTextAlign::Center => 1,
                        components::UiTextAlign::Right => 2,
                    };
                    if egui::ComboBox::from_id_salt("text_align")
                        .width(ui.available_width())
                        .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
                        .changed()
                    {
                        text_style.align = match idx {
                            0 => components::UiTextAlign::Left,
                            2 => components::UiTextAlign::Right,
                            _ => components::UiTextAlign::Center,
                        };
                        let ts = text_style.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut s) = em.get_mut::<components::UiTextStyle>() {
                                    *s = ts;
                                }
                            }
                        });
                    }
                });
            }
        }
    }
}

// ── Padding section ──────────────────────────────────────────────────────────

pub fn render_padding_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut padding_owned) = world.get::<components::UiPadding>(entity).cloned() else {
        return;
    };
    let padding: &mut components::UiPadding = &mut padding_owned;
    {
        {
            let labels = ["Top", "Right", "Bottom", "Left"];
            for (i, label) in labels.iter().enumerate() {
                inline_property(ui, i, label, theme, |ui| {
                    let cur = match i {
                        0 => padding.top,
                        1 => padding.right,
                        2 => padding.bottom,
                        _ => padding.left,
                    };
                    let mut v = cur;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .speed(0.5)
                                .range(0.0..=500.0)
                                .suffix("px"),
                        )
                        .changed()
                    {
                        match i {
                            0 => padding.top = v,
                            1 => padding.right = v,
                            2 => padding.bottom = v,
                            _ => padding.left = v,
                        }
                        let new_padding = *padding;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut p) = em.get_mut::<components::UiPadding>() {
                                    *p = new_padding;
                                }
                            }
                        });
                    }
                });
            }
        }
    }
}

// ── Effects (opacity / shadow / clip / cursor) — split into per-component
//    inspectors below. The old combined `effects_section` is retired.

pub fn render_opacity_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(opacity) = world.get::<components::UiOpacity>(entity).copied() else {
        return;
    };
    inline_property(ui, 0, "Opacity", theme, |ui| {
        let mut v = opacity.0;
        if ui
            .add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0))
            .changed()
        {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut o) = em.get_mut::<components::UiOpacity>() {
                        o.0 = v;
                    }
                }
            });
        }
    });
}

pub fn render_clip_content_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(clip) = world.get::<components::UiClipContent>(entity).copied() else {
        return;
    };
    inline_property(ui, 0, "Clip Content", theme, |ui| {
        let mut v = clip.0;
        if ui.checkbox(&mut v, "").changed() {
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut c) = em.get_mut::<components::UiClipContent>() {
                        c.0 = v;
                    }
                }
            });
        }
    });
}

pub fn render_cursor_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(cursor) = world.get::<components::UiCursor>(entity).copied() else {
        return;
    };
    inline_property(ui, 0, "Cursor", theme, |ui| {
        let cursor_labels = [
            "Default",
            "Pointer",
            "Text",
            "Grab",
            "Grabbing",
            "Not Allowed",
            "Crosshair",
            "EW Resize",
            "NS Resize",
            "Move",
        ];
        let mut idx = match cursor {
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
        if egui::ComboBox::from_id_salt("cursor_type")
            .width(ui.available_width())
            .show_index(ui, &mut idx, cursor_labels.len(), |i| {
                cursor_labels[i].to_string()
            })
            .changed()
        {
            let new_cursor = match idx {
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
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut c) = em.get_mut::<components::UiCursor>() {
                        *c = new_cursor;
                    }
                }
            });
        }
    });
}

pub fn render_shadow_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut shadow_owned) = world.get::<components::UiBoxShadow>(entity).cloned() else {
        return;
    };
    let shadow: &mut components::UiBoxShadow = &mut shadow_owned;
    let mut row = 0;
    inline_property(ui, row, "Color", theme, |ui| {
        let mut arr = color_to_arr(shadow.color);
        if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
            shadow.color = arr_to_color(arr);
            let sh = shadow.clone();
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut s) = em.get_mut::<components::UiBoxShadow>() {
                        *s = sh;
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Offset X", theme, |ui| {
        let mut v = shadow.offset_x;
        if ui
            .add(egui::DragValue::new(&mut v).speed(0.5).suffix("px"))
            .changed()
        {
            shadow.offset_x = v;
            let sh = shadow.clone();
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut s) = em.get_mut::<components::UiBoxShadow>() {
                        *s = sh;
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Offset Y", theme, |ui| {
        let mut v = shadow.offset_y;
        if ui
            .add(egui::DragValue::new(&mut v).speed(0.5).suffix("px"))
            .changed()
        {
            shadow.offset_y = v;
            let sh = shadow.clone();
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut s) = em.get_mut::<components::UiBoxShadow>() {
                        *s = sh;
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Blur", theme, |ui| {
        let mut v = shadow.blur;
        if ui
            .add(
                egui::DragValue::new(&mut v)
                    .speed(0.5)
                    .range(0.0..=200.0)
                    .suffix("px"),
            )
            .changed()
        {
            shadow.blur = v;
            let sh = shadow.clone();
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut s) = em.get_mut::<components::UiBoxShadow>() {
                        *s = sh;
                    }
                }
            });
        }
    });
    row += 1;
    inline_property(ui, row, "Spread", theme, |ui| {
        let mut v = shadow.spread;
        if ui
            .add(egui::DragValue::new(&mut v).speed(0.5).suffix("px"))
            .changed()
        {
            shadow.spread = v;
            let sh = shadow.clone();
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut s) = em.get_mut::<components::UiBoxShadow>() {
                        *s = sh;
                    }
                }
            });
        }
    });
}

// ── Interaction States — see render_interaction_inspector below.

pub fn render_interaction_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut istyle_owned) = world.get::<components::UiInteractionStyle>(entity).cloned()
    else {
        return;
    };
    let mut transition_duration_initial = world
        .get::<components::UiTransition>(entity)
        .map(|t| t.duration);
    {
        {
            let istyle: &mut components::UiInteractionStyle = &mut istyle_owned;

            let states = ["Normal", "Hovered", "Pressed", "Disabled"];
            for (state_idx, state_name) in states.iter().enumerate() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(*state_name)
                        .size(11.0)
                        .strong()
                        .color(theme.text.primary.to_color32()),
                );
                ui.add_space(2.0);

                let state_style = match state_idx {
                    0 => &mut istyle.normal,
                    1 => &mut istyle.hovered,
                    2 => &mut istyle.pressed,
                    _ => &mut istyle.disabled,
                };

                state_style_editor(ui, state_style, state_idx, entity, commands, theme);
            }

            // ── Transition Duration ──
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Transition")
                    .size(11.0)
                    .strong()
                    .color(theme.text.primary.to_color32()),
            );
            ui.add_space(2.0);
            inline_property(ui, 50, "Duration", theme, |ui| {
                let has_transition = transition_duration_initial.is_some();
                ui.horizontal(|ui| {
                    let mut enabled = has_transition;
                    if ui.checkbox(&mut enabled, "").changed() {
                        if enabled {
                            transition_duration_initial = Some(0.15);
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    em.insert(components::UiTransition { duration: 0.15 });
                                }
                            });
                        } else {
                            transition_duration_initial = None;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    em.remove::<components::UiTransition>();
                                }
                            });
                        }
                    }
                    if let Some(ref mut dur) = transition_duration_initial {
                        if ui
                            .add(
                                egui::DragValue::new(dur)
                                    .speed(0.01)
                                    .range(0.0..=5.0)
                                    .suffix("s"),
                            )
                            .changed()
                        {
                            let d = *dur;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut t) = em.get_mut::<components::UiTransition>() {
                                        t.duration = d;
                                    }
                                }
                            });
                        }
                    }
                });
            });
        }
    }
}

/// Editor for a single `UiStateStyle` (override fields for one interaction state).
/// Each override has a checkbox to enable/disable it + the value editor when enabled.
fn state_style_editor(
    ui: &mut egui::Ui,
    state: &mut components::UiStateStyle,
    state_idx: usize,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    let salt = format!("is_{}", state_idx);
    // Track whether anything in this state changed so we push once at the end.
    let mut dirty = false;

    // ── Fill override ──
    inline_property(ui, state_idx * 10, "Fill", theme, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = state.fill.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                state.fill = if enabled {
                    Some(components::UiFill::Solid(Color::srgba(0.3, 0.3, 0.3, 1.0)))
                } else {
                    None
                };
                dirty = true;
            }
            if let Some(components::UiFill::Solid(ref mut color)) = state.fill {
                let mut arr = color_to_arr(*color);
                if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                    *color = arr_to_color(arr);
                    dirty = true;
                }
            } else if matches!(
                state.fill,
                Some(
                    components::UiFill::LinearGradient { .. }
                        | components::UiFill::RadialGradient { .. }
                )
            ) {
                ui.label(
                    egui::RichText::new("gradient")
                        .size(10.0)
                        .color(theme.text.muted.to_color32()),
                );
            }
        });
    });

    // ── Stroke override ──
    inline_property(ui, state_idx * 10 + 1, "Stroke", theme, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = state.stroke.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                state.stroke = if enabled {
                    Some(components::UiStroke::new(Color::WHITE, 1.0))
                } else {
                    None
                };
                dirty = true;
            }
            if let Some(ref mut stroke) = state.stroke {
                let mut arr = color_to_arr(stroke.color);
                if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                    stroke.color = arr_to_color(arr);
                    dirty = true;
                }
                let mut w = stroke.width;
                if ui
                    .add(
                        egui::DragValue::new(&mut w)
                            .speed(0.5)
                            .range(0.0..=50.0)
                            .suffix("px"),
                    )
                    .changed()
                {
                    stroke.width = w;
                    dirty = true;
                }
            }
        });
    });

    // ── Opacity override ──
    inline_property(ui, state_idx * 10 + 2, "Opacity", theme, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = state.opacity.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                state.opacity = if enabled { Some(1.0) } else { None };
                dirty = true;
            }
            if let Some(ref mut opacity) = state.opacity {
                if ui
                    .add(egui::DragValue::new(opacity).speed(0.01).range(0.0..=1.0))
                    .changed()
                {
                    dirty = true;
                }
            }
        });
    });

    // ── Text Color override ──
    inline_property(ui, state_idx * 10 + 3, "Text Color", theme, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = state.text_color.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                state.text_color = if enabled { Some(Color::WHITE) } else { None };
                dirty = true;
            }
            if let Some(ref mut color) = state.text_color {
                let mut arr = color_to_arr(*color);
                if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                    *color = arr_to_color(arr);
                    dirty = true;
                }
            }
        });
    });

    // ── Text Size override ──
    inline_property(ui, state_idx * 10 + 4, "Text Size", theme, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = state.text_size.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                state.text_size = if enabled { Some(14.0) } else { None };
                dirty = true;
            }
            if let Some(ref mut size) = state.text_size {
                if ui
                    .add(
                        egui::DragValue::new(size)
                            .speed(0.5)
                            .range(1.0..=200.0)
                            .suffix("px"),
                    )
                    .changed()
                {
                    dirty = true;
                }
            }
        });
    });

    // ── Cursor override ──
    inline_property(ui, state_idx * 10 + 5, "Cursor", theme, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = state.cursor.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                state.cursor = if enabled {
                    Some(components::UiCursor::Pointer)
                } else {
                    None
                };
                dirty = true;
            }
            if let Some(ref mut cursor) = state.cursor {
                let cursor_labels = [
                    "Default",
                    "Pointer",
                    "Text",
                    "Grab",
                    "Grabbing",
                    "Not Allowed",
                    "Crosshair",
                    "EW Resize",
                    "NS Resize",
                    "Move",
                ];
                let mut idx = match cursor {
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
                if egui::ComboBox::from_id_salt(format!("cursor_{}", salt))
                    .width(80.0)
                    .show_index(ui, &mut idx, cursor_labels.len(), |i| {
                        cursor_labels[i].to_string()
                    })
                    .changed()
                {
                    *cursor = match idx {
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
                    dirty = true;
                }
            }
        });
    });

    // ── Scale override ──
    inline_property(ui, state_idx * 10 + 6, "Scale", theme, |ui| {
        ui.horizontal(|ui| {
            let mut enabled = state.scale.is_some();
            if ui.checkbox(&mut enabled, "").changed() {
                state.scale = if enabled { Some(1.0) } else { None };
                dirty = true;
            }
            if let Some(ref mut scale) = state.scale {
                if ui
                    .add(egui::DragValue::new(scale).speed(0.01).range(0.1..=5.0))
                    .changed()
                {
                    dirty = true;
                }
            }
        });
    });

    // Push once if anything changed
    if dirty {
        push_interaction_style(state_idx, entity, commands, state);
    }
}

/// Push updated `UiInteractionStyle` to the world for a specific state.
fn push_interaction_style(
    state_idx: usize,
    entity: Entity,
    commands: &EditorCommands,
    state: &components::UiStateStyle,
) {
    let new_state = state.clone();
    commands.push(move |world: &mut World| {
        if let Ok(mut em) = world.get_entity_mut(entity) {
            if let Some(mut is) = em.get_mut::<components::UiInteractionStyle>() {
                match state_idx {
                    0 => is.normal = new_state,
                    1 => is.hovered = new_state,
                    2 => is.pressed = new_state,
                    _ => is.disabled = new_state,
                }
            }
        }
    });
}

// ── Widget-specific data inspectors ──────────────────────────────────────────
//
// Each widget data component (SliderData, CheckboxData, …) is its own
// InspectorEntry now. The main inspector wraps each in a collapsible
// automatically; these fns just render the inline_property rows.

/// Declarative fields for `SliderData` (native bevy_ui inspector). Mirrors
/// `render_slider_data_inspector`; egui keeps the custom_ui_fn.
pub fn slider_fields() -> Vec<renzora_editor::FieldDef> {
    vec![
        renzora_editor::float_field!("Value", components::SliderData, value, 0.01, f32::MIN, f32::MAX),
        renzora_editor::float_field!("Min", components::SliderData, min, 0.1, f32::MIN, f32::MAX),
        renzora_editor::float_field!("Max", components::SliderData, max, 0.1, f32::MIN, f32::MAX),
        renzora_editor::float_field!("Step", components::SliderData, step, 0.01, 0.0, f32::MAX),
        renzora_editor::color_rgba_field!("Track Color", components::SliderData, track_color),
        renzora_editor::color_rgba_field!("Fill Color", components::SliderData, fill_color),
        renzora_editor::color_rgba_field!("Thumb Color", components::SliderData, thumb_color),
    ]
}

pub fn render_slider_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::SliderData>(entity).cloned() else {
        return;
    };
    let data: &mut components::SliderData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Value", theme, |ui| {
                let mut v = data.value;
                if ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .speed(0.01)
                            .range(data.min..=data.max),
                    )
                    .changed()
                {
                    data.value = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::SliderData>() {
                                d.value = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Min", theme, |ui| {
                let mut v = data.min;
                if ui.add(egui::DragValue::new(&mut v).speed(0.1)).changed() {
                    data.min = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::SliderData>() {
                                d.min = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Max", theme, |ui| {
                let mut v = data.max;
                if ui.add(egui::DragValue::new(&mut v).speed(0.1)).changed() {
                    data.max = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::SliderData>() {
                                d.max = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Step", theme, |ui| {
                let mut v = data.step;
                if ui
                    .add(
                        egui::DragValue::new(&mut v)
                            .speed(0.01)
                            .range(0.0..=f32::MAX),
                    )
                    .changed()
                {
                    data.step = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::SliderData>() {
                                d.step = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            color_row(
                ui,
                row,
                "Track Color",
                theme,
                &mut data.track_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::SliderData>()
                        .map(|mut p| p.track_color = c)
                },
            );
            row += 1;
            color_row(
                ui,
                row,
                "Fill Color",
                theme,
                &mut data.fill_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::SliderData>()
                        .map(|mut p| p.fill_color = c)
                },
            );
            row += 1;
            color_row(
                ui,
                row,
                "Thumb Color",
                theme,
                &mut data.thumb_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::SliderData>()
                        .map(|mut p| p.thumb_color = c)
                },
            );
        }
    }
}

pub fn render_checkbox_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::CheckboxData>(entity).cloned() else {
        return;
    };
    let data: &mut components::CheckboxData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Checked", theme, |ui| {
                let mut v = data.checked;
                if ui.checkbox(&mut v, "").changed() {
                    data.checked = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::CheckboxData>() {
                                d.checked = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Label", theme, |ui| {
                let mut v = data.label.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.label = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::CheckboxData>() {
                                d.label = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            color_row(
                ui,
                row,
                "Check Color",
                theme,
                &mut data.check_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::CheckboxData>()
                        .map(|mut p| p.check_color = c)
                },
            );
            row += 1;
            color_row(
                ui,
                row,
                "Box Color",
                theme,
                &mut data.box_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::CheckboxData>()
                        .map(|mut p| p.box_color = c)
                },
            );
        }
    }
}

pub fn render_toggle_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::ToggleData>(entity).cloned() else {
        return;
    };
    let data: &mut components::ToggleData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "On", theme, |ui| {
                let mut v = data.on;
                if ui.checkbox(&mut v, "").changed() {
                    data.on = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ToggleData>() {
                                d.on = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Label", theme, |ui| {
                let mut v = data.label.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.label = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ToggleData>() {
                                d.label = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            color_row(
                ui,
                row,
                "On Color",
                theme,
                &mut data.on_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::ToggleData>()
                        .map(|mut p| p.on_color = c)
                },
            );
            row += 1;
            color_row(
                ui,
                row,
                "Off Color",
                theme,
                &mut data.off_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::ToggleData>()
                        .map(|mut p| p.off_color = c)
                },
            );
            row += 1;
            color_row(
                ui,
                row,
                "Knob Color",
                theme,
                &mut data.knob_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::ToggleData>()
                        .map(|mut p| p.knob_color = c)
                },
            );
        }
    }
}

pub fn render_radio_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::RadioButtonData>(entity).cloned() else {
        return;
    };
    let data: &mut components::RadioButtonData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Group", theme, |ui| {
                let mut v = data.group.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.group = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::RadioButtonData>() {
                                d.group = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Selected", theme, |ui| {
                let mut v = data.selected;
                if ui.checkbox(&mut v, "").changed() {
                    data.selected = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::RadioButtonData>() {
                                d.selected = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Label", theme, |ui| {
                let mut v = data.label.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.label = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::RadioButtonData>() {
                                d.label = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            color_row(
                ui,
                row,
                "Active Color",
                theme,
                &mut data.active_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::RadioButtonData>()
                        .map(|mut p| p.active_color = c)
                },
            );
        }
    }
}

pub fn render_dropdown_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::DropdownData>(entity).cloned() else {
        return;
    };
    let data: &mut components::DropdownData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Placeholder", theme, |ui| {
                let mut v = data.placeholder.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.placeholder = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::DropdownData>() {
                                d.placeholder = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Selected", theme, |ui| {
                let mut v = data.selected;
                let options = &data.options;
                let label = if v >= 0 && (v as usize) < options.len() {
                    options[v as usize].clone()
                } else {
                    data.placeholder.clone()
                };
                if egui::ComboBox::from_id_salt("dropdown_sel")
                    .width(ui.available_width())
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (i, opt) in options.iter().enumerate() {
                            ui.selectable_value(&mut v, i as i32, opt);
                        }
                    })
                    .inner
                    .is_some()
                    && v != data.selected {
                        data.selected = v;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut d) = em.get_mut::<components::DropdownData>() {
                                    d.selected = v;
                                }
                            }
                        });
                    }
            });
            // Options list
            let mut options_changed = false;
            let mut new_options = data.options.clone();
            for (i, option) in new_options.iter_mut().enumerate() {
                inline_property(ui, i + 2, &format!("#{}", i + 1), theme, |ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(option)
                                .desired_width(ui.available_width()),
                        )
                        .changed()
                    {
                        options_changed = true;
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                if ui.small_button(format!("{} Add", regular::PLUS)).clicked() {
                    new_options.push(format!("Option {}", new_options.len() + 1));
                    options_changed = true;
                }
                if new_options.len() > 1
                    && ui
                        .small_button(format!("{} Remove", regular::MINUS))
                        .clicked()
                    {
                        new_options.pop();
                        options_changed = true;
                    }
            });
            if options_changed {
                data.options = new_options.clone();
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::DropdownData>() {
                            d.options = new_options.clone();
                        }
                    }
                });
            }
        }
    }
}

pub fn render_text_input_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::TextInputData>(entity).cloned() else {
        return;
    };
    let data: &mut components::TextInputData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Text", theme, |ui| {
                let mut v = data.text.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.text = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() {
                                d.text = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Placeholder", theme, |ui| {
                let mut v = data.placeholder.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.placeholder = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() {
                                d.placeholder = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Max Length", theme, |ui| {
                let mut v = data.max_length as i32;
                if ui
                    .add(egui::DragValue::new(&mut v).range(1..=10000))
                    .changed()
                {
                    data.max_length = v as usize;
                    let len = v as usize;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() {
                                d.max_length = len;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Password", theme, |ui| {
                let mut v = data.password;
                if ui.checkbox(&mut v, "").changed() {
                    data.password = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() {
                                d.password = v;
                            }
                        }
                    });
                }
            });
        }
    }
}

pub fn render_scroll_view_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::ScrollViewData>(entity).cloned() else {
        return;
    };
    let data: &mut components::ScrollViewData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Scroll Speed", theme, |ui| {
                let mut v = data.scroll_speed;
                if ui
                    .add(egui::DragValue::new(&mut v).speed(0.5).range(1.0..=200.0))
                    .changed()
                {
                    data.scroll_speed = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ScrollViewData>() {
                                d.scroll_speed = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Horizontal", theme, |ui| {
                let mut v = data.show_horizontal;
                if ui.checkbox(&mut v, "").changed() {
                    data.show_horizontal = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ScrollViewData>() {
                                d.show_horizontal = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Vertical", theme, |ui| {
                let mut v = data.show_vertical;
                if ui.checkbox(&mut v, "").changed() {
                    data.show_vertical = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ScrollViewData>() {
                                d.show_vertical = v;
                            }
                        }
                    });
                }
            });
        }
    }
}

pub fn render_tooltip_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::TooltipData>(entity).cloned() else {
        return;
    };
    let data: &mut components::TooltipData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Text", theme, |ui| {
                let mut v = data.text.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.text = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TooltipData>() {
                                d.text = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Delay (ms)", theme, |ui| {
                let mut v = data.delay_ms as i32;
                if ui
                    .add(egui::DragValue::new(&mut v).range(0..=5000))
                    .changed()
                {
                    data.delay_ms = v as u32;
                    let delay = v as u32;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TooltipData>() {
                                d.delay_ms = delay;
                            }
                        }
                    });
                }
            });
            row += 1;
            color_row(
                ui,
                row,
                "Bg Color",
                theme,
                &mut data.bg_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::TooltipData>()
                        .map(|mut p| p.bg_color = c)
                },
            );
            row += 1;
            color_row(
                ui,
                row,
                "Text Color",
                theme,
                &mut data.text_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::TooltipData>()
                        .map(|mut p| p.text_color = c)
                },
            );
        }
    }
}

pub fn render_modal_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world.get::<components::ModalData>(entity).cloned() else {
        return;
    };
    let data: &mut components::ModalData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Title", theme, |ui| {
                let mut v = data.title.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.title = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ModalData>() {
                                d.title = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Closable", theme, |ui| {
                let mut v = data.closable;
                if ui.checkbox(&mut v, "").changed() {
                    data.closable = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ModalData>() {
                                d.closable = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            color_row(
                ui,
                row,
                "Backdrop",
                theme,
                &mut data.backdrop_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::ModalData>()
                        .map(|mut p| p.backdrop_color = c)
                },
            );
        }
    }
}

pub fn render_draggable_window_data_inspector(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    commands: &EditorCommands,
    theme: &Theme,
) {
    let Some(mut data_owned) = world
        .get::<components::DraggableWindowData>(entity)
        .cloned()
    else {
        return;
    };
    let data: &mut components::DraggableWindowData = &mut data_owned;
    {
        {
            let mut row = 0;
            inline_property(ui, row, "Title", theme, |ui| {
                let mut v = data.title.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width()))
                    .changed()
                {
                    data.title = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::DraggableWindowData>() {
                                d.title = v.clone();
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Closable", theme, |ui| {
                let mut v = data.closable;
                if ui.checkbox(&mut v, "").changed() {
                    data.closable = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::DraggableWindowData>() {
                                d.closable = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Minimizable", theme, |ui| {
                let mut v = data.minimizable;
                if ui.checkbox(&mut v, "").changed() {
                    data.minimizable = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::DraggableWindowData>() {
                                d.minimizable = v;
                            }
                        }
                    });
                }
            });
            row += 1;
            color_row(
                ui,
                row,
                "Title Bar",
                theme,
                &mut data.title_bar_color,
                entity,
                commands,
                |d, c| {
                    d.get_mut::<components::DraggableWindowData>()
                        .map(|mut p| p.title_bar_color = c)
                },
            );
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Color property row using inline_property with alternating background.
fn color_row(
    ui: &mut egui::Ui,
    row_index: usize,
    label: &str,
    theme: &renzora_theme::Theme,
    color: &mut Color,
    entity: Entity,
    commands: &EditorCommands,
    apply: impl Fn(&mut bevy::ecs::world::EntityWorldMut, Color) -> Option<()> + Send + Sync + 'static,
) {
    inline_property(ui, row_index, label, theme, |ui| {
        let mut arr = color_to_arr(*color);
        if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
            let new_color = arr_to_color(arr);
            *color = new_color;
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    apply(&mut em, new_color);
                }
            });
        }
    });
}
