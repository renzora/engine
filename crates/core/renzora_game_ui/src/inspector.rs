//! UI Inspector panel — property editor for selected UiWidget / UiCanvas entities.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::ThemeManager;

use crate::components::{UiCanvas, UiWidget, UiWidgetType};

/// Snapshot of the selected widget's properties for editing.
#[derive(Default, Clone)]
struct InspectorSnapshot {
    entity: Option<Entity>,
    name: String,
    // Canvas props (if entity has UiCanvas)
    is_canvas: bool,
    sort_order: i32,
    visibility_mode: String,
    reference_width: f32,
    reference_height: f32,
    // Reference resolution (from parent canvas or defaults)
    canvas_ref_w: f32,
    canvas_ref_h: f32,
    // Widget props (if entity has UiWidget)
    is_widget: bool,
    widget_type: UiWidgetType,
    locked: bool,
    // Layout (from Node)
    has_node: bool,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    padding: [f32; 4],    // top, right, bottom, left
    margin: [f32; 4],
    position_type: u8,     // 0=Relative, 1=Absolute
    flex_direction: u8,    // 0=Row, 1=Column, 2=RowReverse, 3=ColumnReverse
    justify_content: u8,   // 0=Start, 1=Center, 2=End, 3=SpaceBetween, 4=SpaceAround, 5=SpaceEvenly
    align_items: u8,       // 0=Start, 1=Center, 2=End, 3=Stretch
    // Style
    has_bg: bool,
    bg_color: [f32; 4],
    has_border_color: bool,
    border_color: [f32; 4],
    // Text
    has_text: bool,
    text_content: String,
    text_size: f32,
    text_color: [f32; 4],
}

#[derive(Default)]
struct UiInspectorState {
    snap: InspectorSnapshot,
}

pub struct UiInspectorPanel {
    state: RwLock<UiInspectorState>,
}

impl Default for UiInspectorPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(UiInspectorState::default()),
        }
    }
}

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

impl EditorPanel for UiInspectorPanel {
    fn id(&self) -> &str {
        "ui_inspector"
    }

    fn title(&self) -> &str {
        "UI Inspector"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::SLIDERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let commands = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };
        let selection = match world.get_resource::<EditorSelection>() {
            Some(s) => s,
            None => return,
        };

        let mut state = self.state.write().unwrap();
        let text_muted = theme.text.muted.to_color32();
        let _text_primary = theme.text.primary.to_color32();
        let accent = theme.semantic.accent.to_color32();

        // Get selected entity
        let selected = selection.get();
        let Some(entity) = selected else {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Select a UI widget to inspect")
                        .size(11.0)
                        .color(text_muted),
                );
            });
            return;
        };

        // Snapshot entity data
        let snap = &mut state.snap;
        snap.entity = Some(entity);

        // Name
        snap.name = world
            .get::<Name>(entity)
            .map(|n| n.as_str().to_string())
            .unwrap_or_default();

        // Canvas
        snap.is_canvas = world.get::<UiCanvas>(entity).is_some();
        if let Some(canvas) = world.get::<UiCanvas>(entity) {
            snap.sort_order = canvas.sort_order;
            snap.visibility_mode = canvas.visibility_mode.clone();
            snap.reference_width = canvas.reference_width;
            snap.reference_height = canvas.reference_height;
        }

        // Widget
        snap.is_widget = world.get::<UiWidget>(entity).is_some();
        if let Some(widget) = world.get::<UiWidget>(entity) {
            snap.widget_type = widget.widget_type.clone();
            snap.locked = widget.locked;
        }

        // If neither canvas nor widget, show nothing
        if !snap.is_canvas && !snap.is_widget {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Selected entity is not a UI element")
                        .size(11.0)
                        .color(text_muted),
                );
            });
            return;
        }

        // Resolve reference resolution from parent canvas (or self if canvas)
        snap.canvas_ref_w = 1280.0;
        snap.canvas_ref_h = 720.0;
        if snap.is_canvas {
            snap.canvas_ref_w = snap.reference_width;
            snap.canvas_ref_h = snap.reference_height;
        } else if let Some(child_of) = world.get::<ChildOf>(entity) {
            if let Some(canvas) = world.get::<UiCanvas>(child_of.parent()) {
                snap.canvas_ref_w = canvas.reference_width;
                snap.canvas_ref_h = canvas.reference_height;
            }
        }
        let crw = snap.canvas_ref_w;
        let crh = snap.canvas_ref_h;

        // Node
        snap.has_node = world.get::<Node>(entity).is_some();
        if let Some(node) = world.get::<Node>(entity) {
            snap.left = val_to_design_px(node.left, crw);
            snap.top = val_to_design_px(node.top, crh);
            snap.width = val_to_design_px(node.width, crw);
            snap.height = val_to_design_px(node.height, crh);
            snap.position_type = position_type_to_u8(node.position_type);
            snap.flex_direction = flex_direction_to_u8(node.flex_direction);
            snap.justify_content = justify_content_to_u8(node.justify_content);
            snap.align_items = align_items_to_u8(node.align_items);
            snap.padding = [
                val_px(node.padding.top),
                val_px(node.padding.right),
                val_px(node.padding.bottom),
                val_px(node.padding.left),
            ];
            snap.margin = [
                val_px(node.margin.top),
                val_px(node.margin.right),
                val_px(node.margin.bottom),
                val_px(node.margin.left),
            ];
        }

        // Background
        snap.has_bg = world.get::<BackgroundColor>(entity).is_some();
        if let Some(bg) = world.get::<BackgroundColor>(entity) {
            snap.bg_color = bg.0.to_srgba().to_f32_array();
        }

        // Border
        snap.has_border_color = world.get::<BorderColor>(entity).is_some();
        if let Some(bc) = world.get::<BorderColor>(entity) {
            snap.border_color = bc.top.to_srgba().to_f32_array();
        }

        // Text
        snap.has_text = world.get::<bevy::ui::widget::Text>(entity).is_some();
        if let Some(text) = world.get::<bevy::ui::widget::Text>(entity) {
            snap.text_content = text.0.clone();
        }
        if let Some(font) = world.get::<TextFont>(entity) {
            snap.text_size = font.font_size;
        }
        if let Some(tc) = world.get::<TextColor>(entity) {
            snap.text_color = tc.0.to_srgba().to_f32_array();
        }

        // ── Render property sections ─────────────────────────────────────
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Header
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    let icon = if snap.is_canvas {
                        regular::FRAME_CORNERS
                    } else {
                        snap.widget_type.icon()
                    };
                    let type_name = if snap.is_canvas {
                        "Canvas"
                    } else {
                        snap.widget_type.label()
                    };
                    ui.label(
                        egui::RichText::new(format!("{} {}", icon, type_name))
                            .size(13.0)
                            .color(accent),
                    );
                });
                ui.add_space(4.0);

                // Name
                section_header(ui, "Identity", text_muted);
                property_row(ui, "Name", text_muted, |ui| {
                    let mut name = snap.name.clone();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut name)
                            .desired_width(ui.available_width()),
                    );
                    if resp.changed() {
                        snap.name = name.clone();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut n) = em.get_mut::<Name>() {
                                    n.set(name.clone());
                                }
                            }
                        });
                    }
                });

                // Canvas properties
                if snap.is_canvas {
                    section_header(ui, "Canvas", text_muted);
                    property_row(ui, "Sort Order", text_muted, |ui| {
                        let mut v = snap.sort_order;
                        if ui.add(egui::DragValue::new(&mut v).range(-100..=100)).changed() {
                            snap.sort_order = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                                        canvas.sort_order = v;
                                    }
                                }
                            });
                        }
                    });
                    property_row(ui, "Visibility", text_muted, |ui| {
                        let modes = ["always", "play_only", "editor_only"];
                        let mut idx = modes.iter().position(|m| *m == snap.visibility_mode).unwrap_or(0);
                        if egui::ComboBox::from_id_salt("vis_mode")
                            .width(ui.available_width())
                            .show_index(ui, &mut idx, modes.len(), |i| modes[i].to_string())
                            .changed()
                        {
                            let mode = modes[idx].to_string();
                            snap.visibility_mode = mode.clone();
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                                        canvas.visibility_mode = mode.clone();
                                    }
                                }
                            });
                        }
                    });

                    section_header(ui, "Reference Resolution", text_muted);
                    property_row(ui, "Width", text_muted, |ui| {
                        let mut v = snap.reference_width;
                        if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(1.0..=7680.0)).changed() {
                            snap.reference_width = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                                        canvas.reference_width = v;
                                    }
                                }
                            });
                        }
                    });
                    property_row(ui, "Height", text_muted, |ui| {
                        let mut v = snap.reference_height;
                        if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(1.0..=4320.0)).changed() {
                            snap.reference_height = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut canvas) = em.get_mut::<UiCanvas>() {
                                        canvas.reference_height = v;
                                    }
                                }
                            });
                        }
                    });
                }

                // Widget properties
                if snap.is_widget {
                    property_row(ui, "Locked", text_muted, |ui| {
                        let mut v = snap.locked;
                        if ui.checkbox(&mut v, "").changed() {
                            snap.locked = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut w) = em.get_mut::<UiWidget>() {
                                        w.locked = v;
                                    }
                                }
                            });
                        }
                    });
                }

                // Layout
                if snap.has_node {
                    section_header(ui, "Layout", text_muted);

                    // Position type
                    property_row(ui, "Position", text_muted, |ui| {
                        let labels = ["Relative", "Absolute"];
                        let mut idx = snap.position_type as usize;
                        if egui::ComboBox::from_id_salt("pos_type")
                            .width(ui.available_width())
                            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
                            .changed()
                        {
                            snap.position_type = idx as u8;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.position_type = u8_to_position_type(idx as u8);
                                    }
                                }
                            });
                        }
                    });

                    // X / Y
                    property_row(ui, "X", text_muted, |ui| {
                        let mut v = snap.left;
                        if ui.add(egui::DragValue::new(&mut v).speed(1.0)).changed() {
                            snap.left = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.left = bevy::ui::Val::Percent(v / crw * 100.0);
                                    }
                                }
                            });
                        }
                    });
                    property_row(ui, "Y", text_muted, |ui| {
                        let mut v = snap.top;
                        if ui.add(egui::DragValue::new(&mut v).speed(1.0)).changed() {
                            snap.top = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.top = bevy::ui::Val::Percent(v / crh * 100.0);
                                    }
                                }
                            });
                        }
                    });

                    // Width / Height
                    property_row(ui, "Width", text_muted, |ui| {
                        let mut v = snap.width;
                        if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(0.0..=f32::MAX)).changed() {
                            snap.width = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.width = bevy::ui::Val::Percent(v / crw * 100.0);
                                    }
                                }
                            });
                        }
                    });
                    property_row(ui, "Height", text_muted, |ui| {
                        let mut v = snap.height;
                        if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(0.0..=f32::MAX)).changed() {
                            snap.height = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.height = bevy::ui::Val::Percent(v / crh * 100.0);
                                    }
                                }
                            });
                        }
                    });

                    // Flex direction
                    property_row(ui, "Direction", text_muted, |ui| {
                        let labels = ["Row", "Column", "Row Rev", "Col Rev"];
                        let mut idx = snap.flex_direction as usize;
                        if egui::ComboBox::from_id_salt("flex_dir")
                            .width(ui.available_width())
                            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
                            .changed()
                        {
                            snap.flex_direction = idx as u8;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.flex_direction = u8_to_flex_direction(idx as u8);
                                    }
                                }
                            });
                        }
                    });

                    // Justify content
                    property_row(ui, "Justify", text_muted, |ui| {
                        let labels = ["Start", "Center", "End", "Between", "Around", "Evenly"];
                        let mut idx = snap.justify_content as usize;
                        if egui::ComboBox::from_id_salt("justify")
                            .width(ui.available_width())
                            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
                            .changed()
                        {
                            snap.justify_content = idx as u8;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.justify_content = u8_to_justify_content(idx as u8);
                                    }
                                }
                            });
                        }
                    });

                    // Align items
                    property_row(ui, "Align", text_muted, |ui| {
                        let labels = ["Start", "Center", "End", "Stretch"];
                        let mut idx = snap.align_items as usize;
                        if egui::ComboBox::from_id_salt("align")
                            .width(ui.available_width())
                            .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
                            .changed()
                        {
                            snap.align_items = idx as u8;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut node) = em.get_mut::<Node>() {
                                        node.align_items = u8_to_align_items(idx as u8);
                                    }
                                }
                            });
                        }
                    });

                    // Padding
                    section_header(ui, "Padding", text_muted);
                    for (i, label) in ["Top", "Right", "Bottom", "Left"].iter().enumerate() {
                        property_row(ui, label, text_muted, |ui| {
                            let mut v = snap.padding[i];
                            if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.0..=f32::MAX)).changed() {
                                snap.padding[i] = v;
                                let padding = snap.padding;
                                commands.push(move |world: &mut World| {
                                    if let Ok(mut em) = world.get_entity_mut(entity) {
                                        if let Some(mut node) = em.get_mut::<Node>() {
                                            node.padding = bevy::ui::UiRect {
                                                top: bevy::ui::Val::Px(padding[0]),
                                                right: bevy::ui::Val::Px(padding[1]),
                                                bottom: bevy::ui::Val::Px(padding[2]),
                                                left: bevy::ui::Val::Px(padding[3]),
                                            };
                                        }
                                    }
                                });
                            }
                        });
                    }
                }

                // Style
                if snap.has_bg || snap.has_border_color {
                    section_header(ui, "Style", text_muted);
                }

                if snap.has_bg {
                    property_row(ui, "Background", text_muted, |ui| {
                        let mut c = snap.bg_color;
                        if ui.color_edit_button_rgba_unmultiplied(&mut c).changed() {
                            snap.bg_color = c;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut bg) = em.get_mut::<BackgroundColor>() {
                                        bg.0 = Color::srgba(c[0], c[1], c[2], c[3]);
                                    }
                                }
                            });
                        }
                    });
                }

                if snap.has_border_color {
                    property_row(ui, "Border", text_muted, |ui| {
                        let mut c = snap.border_color;
                        if ui.color_edit_button_rgba_unmultiplied(&mut c).changed() {
                            snap.border_color = c;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut bc) = em.get_mut::<BorderColor>() {
                                        *bc = BorderColor::all(Color::srgba(c[0], c[1], c[2], c[3]));
                                    }
                                }
                            });
                        }
                    });
                }

                // Text
                if snap.has_text {
                    section_header(ui, "Text", text_muted);

                    property_row(ui, "Content", text_muted, |ui| {
                        let mut v = snap.text_content.clone();
                        if ui.add(
                            egui::TextEdit::multiline(&mut v)
                                .desired_width(ui.available_width())
                                .desired_rows(2),
                        )
                        .changed()
                        {
                            snap.text_content = v.clone();
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut text) = em.get_mut::<bevy::ui::widget::Text>() {
                                        text.0 = v.clone();
                                    }
                                }
                            });
                        }
                    });

                    property_row(ui, "Size", text_muted, |ui| {
                        let mut v = snap.text_size;
                        if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(1.0..=200.0)).changed() {
                            snap.text_size = v;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut font) = em.get_mut::<TextFont>() {
                                        font.font_size = v;
                                    }
                                }
                            });
                        }
                    });

                    property_row(ui, "Color", text_muted, |ui| {
                        let mut c = snap.text_color;
                        if ui.color_edit_button_rgba_unmultiplied(&mut c).changed() {
                            snap.text_color = c;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    if let Some(mut tc) = em.get_mut::<TextColor>() {
                                        tc.0 = Color::srgba(c[0], c[1], c[2], c[3]);
                                    }
                                }
                            });
                        }
                    });
                }

                // Delete button
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    let del = ui.add(
                        egui::Button::new(
                            egui::RichText::new(format!("{} Delete", regular::TRASH))
                                .size(12.0)
                                .color(egui::Color32::from_rgb(220, 70, 70)),
                        )
                        .min_size(egui::vec2(ui.available_width() - 8.0, 28.0)),
                    );
                    if del.clicked() {
                        commands.push(move |world: &mut World| {
                            if world.get_entity(entity).is_ok() {
                                world.despawn(entity);
                            }
                            if let Some(sel) = world.get_resource::<EditorSelection>() {
                                sel.set(None);
                            }
                        });
                    }
                });
            });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

fn section_header(ui: &mut egui::Ui, label: &str, color: egui::Color32) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(label.to_uppercase())
                .size(10.0)
                .color(color),
        );
    });
    ui.separator();
    ui.add_space(2.0);
}

fn property_row(
    ui: &mut egui::Ui,
    label: &str,
    label_color: egui::Color32,
    content: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(label)
                .size(11.0)
                .color(label_color),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            content(ui);
        });
    });
    ui.add_space(1.0);
}
