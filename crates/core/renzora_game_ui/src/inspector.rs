//! UI Inspector panel — property editor for selected UiWidget / UiCanvas entities.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::ThemeManager;

use crate::components::{self, UiCanvas, UiWidget, UiWidgetType};

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
    // Widget-specific data snapshots
    progress_bar: Option<components::ProgressBarData>,
    health_bar: Option<components::HealthBarData>,
    slider: Option<components::SliderData>,
    checkbox: Option<components::CheckboxData>,
    toggle: Option<components::ToggleData>,
    radio_button: Option<components::RadioButtonData>,
    dropdown: Option<components::DropdownData>,
    text_input: Option<components::TextInputData>,
    scroll_view: Option<components::ScrollViewData>,
    tab_bar: Option<components::TabBarData>,
    spinner: Option<components::SpinnerData>,
    tooltip: Option<components::TooltipData>,
    modal: Option<components::ModalData>,
    draggable_window: Option<components::DraggableWindowData>,
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

/// Helper: convert a `Color` to `[f32; 4]` RGBA.
fn color_to_arr(c: Color) -> [f32; 4] {
    c.to_srgba().to_f32_array()
}

/// Helper: convert `[f32; 4]` RGBA to `Color`.
fn arr_to_color(c: [f32; 4]) -> Color {
    Color::srgba(c[0], c[1], c[2], c[3])
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

        // Widget-specific data snapshots
        snap.progress_bar = world.get::<components::ProgressBarData>(entity).cloned();
        snap.health_bar = world.get::<components::HealthBarData>(entity).cloned();
        snap.slider = world.get::<components::SliderData>(entity).cloned();
        snap.checkbox = world.get::<components::CheckboxData>(entity).cloned();
        snap.toggle = world.get::<components::ToggleData>(entity).cloned();
        snap.radio_button = world.get::<components::RadioButtonData>(entity).cloned();
        snap.dropdown = world.get::<components::DropdownData>(entity).cloned();
        snap.text_input = world.get::<components::TextInputData>(entity).cloned();
        snap.scroll_view = world.get::<components::ScrollViewData>(entity).cloned();
        snap.tab_bar = world.get::<components::TabBarData>(entity).cloned();
        snap.spinner = world.get::<components::SpinnerData>(entity).cloned();
        snap.tooltip = world.get::<components::TooltipData>(entity).cloned();
        snap.modal = world.get::<components::ModalData>(entity).cloned();
        snap.draggable_window = world.get::<components::DraggableWindowData>(entity).cloned();

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
                    // Theme selector (global, shown in canvas inspector)
                    section_header(ui, "Theme", text_muted);
                    let current_theme_name = world
                        .get_resource::<components::UiTheme>()
                        .map(|t| t.name.clone())
                        .unwrap_or_default();
                    property_row(ui, "Preset", text_muted, |ui| {
                        let themes = ["Dark", "Light", "High Contrast"];
                        let mut idx = themes.iter().position(|t| *t == current_theme_name).unwrap_or(0);
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

                // ── Widget-specific properties ────────────────────────────
                widget_data_section(ui, snap, entity, commands, text_muted);

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

// ── Widget-specific property sections ────────────────────────────────────────

fn widget_data_section(
    ui: &mut egui::Ui,
    snap: &mut InspectorSnapshot,
    entity: Entity,
    commands: &EditorCommands,
    text_muted: egui::Color32,
) {
    // Progress Bar
    if let Some(ref mut data) = snap.progress_bar {
        section_header(ui, "Progress Bar", text_muted);
        property_row(ui, "Value", text_muted, |ui| {
            let mut v = data.value;
            if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=data.max)).changed() {
                data.value = v;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::ProgressBarData>() {
                            d.value = v;
                        }
                    }
                });
            }
        });
        property_row(ui, "Max", text_muted, |ui| {
            let mut v = data.max;
            if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                data.max = v;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::ProgressBarData>() {
                            d.max = v;
                        }
                    }
                });
            }
        });
        property_row(ui, "Direction", text_muted, |ui| {
            let labels = ["Left→Right", "Right→Left", "Bottom→Top", "Top→Bottom"];
            let mut idx = data.direction as usize;
            if egui::ComboBox::from_id_salt("prog_dir")
                .width(ui.available_width())
                .show_index(ui, &mut idx, labels.len(), |i| labels[i].to_string())
                .changed()
            {
                let dir = match idx {
                    1 => components::ProgressDirection::RightToLeft,
                    2 => components::ProgressDirection::BottomToTop,
                    3 => components::ProgressDirection::TopToBottom,
                    _ => components::ProgressDirection::LeftToRight,
                };
                data.direction = dir;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::ProgressBarData>() {
                            d.direction = dir;
                        }
                    }
                });
            }
        });
        color_property(ui, "Fill Color", text_muted, &mut data.fill_color, entity, commands,
            |d, c| d.get_mut::<components::ProgressBarData>().map(|mut p| p.fill_color = c));
        color_property(ui, "Bg Color", text_muted, &mut data.bg_color, entity, commands,
            |d, c| d.get_mut::<components::ProgressBarData>().map(|mut p| p.bg_color = c));
    }

    // Health Bar
    if let Some(ref mut data) = snap.health_bar {
        section_header(ui, "Health Bar", text_muted);
        property_row(ui, "Current", text_muted, |ui| {
            let mut v = data.current;
            if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.0..=data.max)).changed() {
                data.current = v;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::HealthBarData>() {
                            d.current = v;
                        }
                    }
                });
            }
        });
        property_row(ui, "Max", text_muted, |ui| {
            let mut v = data.max;
            if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.001..=f32::MAX)).changed() {
                data.max = v;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::HealthBarData>() {
                            d.max = v;
                        }
                    }
                });
            }
        });
        property_row(ui, "Low Threshold", text_muted, |ui| {
            let mut v = data.low_threshold;
            if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0)).changed() {
                data.low_threshold = v;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::HealthBarData>() {
                            d.low_threshold = v;
                        }
                    }
                });
            }
        });
        color_property(ui, "Fill Color", text_muted, &mut data.fill_color, entity, commands,
            |d, c| d.get_mut::<components::HealthBarData>().map(|mut p| p.fill_color = c));
        color_property(ui, "Low Color", text_muted, &mut data.low_color, entity, commands,
            |d, c| d.get_mut::<components::HealthBarData>().map(|mut p| p.low_color = c));
        color_property(ui, "Bg Color", text_muted, &mut data.bg_color, entity, commands,
            |d, c| d.get_mut::<components::HealthBarData>().map(|mut p| p.bg_color = c));
    }

    // Slider
    if let Some(ref mut data) = snap.slider {
        section_header(ui, "Slider", text_muted);
        property_row(ui, "Value", text_muted, |ui| {
            let mut v = data.value;
            if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(data.min..=data.max)).changed() {
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
        property_row(ui, "Min", text_muted, |ui| {
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
        property_row(ui, "Max", text_muted, |ui| {
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
        property_row(ui, "Step", text_muted, |ui| {
            let mut v = data.step;
            if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=f32::MAX)).changed() {
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
        color_property(ui, "Track Color", text_muted, &mut data.track_color, entity, commands,
            |d, c| d.get_mut::<components::SliderData>().map(|mut p| p.track_color = c));
        color_property(ui, "Fill Color", text_muted, &mut data.fill_color, entity, commands,
            |d, c| d.get_mut::<components::SliderData>().map(|mut p| p.fill_color = c));
        color_property(ui, "Thumb Color", text_muted, &mut data.thumb_color, entity, commands,
            |d, c| d.get_mut::<components::SliderData>().map(|mut p| p.thumb_color = c));
    }

    // Checkbox
    if let Some(ref mut data) = snap.checkbox {
        section_header(ui, "Checkbox", text_muted);
        property_row(ui, "Checked", text_muted, |ui| {
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
        property_row(ui, "Label", text_muted, |ui| {
            let mut v = data.label.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        color_property(ui, "Check Color", text_muted, &mut data.check_color, entity, commands,
            |d, c| d.get_mut::<components::CheckboxData>().map(|mut p| p.check_color = c));
        color_property(ui, "Box Color", text_muted, &mut data.box_color, entity, commands,
            |d, c| d.get_mut::<components::CheckboxData>().map(|mut p| p.box_color = c));
    }

    // Toggle
    if let Some(ref mut data) = snap.toggle {
        section_header(ui, "Toggle", text_muted);
        property_row(ui, "On", text_muted, |ui| {
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
        property_row(ui, "Label", text_muted, |ui| {
            let mut v = data.label.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        color_property(ui, "On Color", text_muted, &mut data.on_color, entity, commands,
            |d, c| d.get_mut::<components::ToggleData>().map(|mut p| p.on_color = c));
        color_property(ui, "Off Color", text_muted, &mut data.off_color, entity, commands,
            |d, c| d.get_mut::<components::ToggleData>().map(|mut p| p.off_color = c));
        color_property(ui, "Knob Color", text_muted, &mut data.knob_color, entity, commands,
            |d, c| d.get_mut::<components::ToggleData>().map(|mut p| p.knob_color = c));
    }

    // Radio Button
    if let Some(ref mut data) = snap.radio_button {
        section_header(ui, "Radio Button", text_muted);
        property_row(ui, "Group", text_muted, |ui| {
            let mut v = data.group.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        property_row(ui, "Selected", text_muted, |ui| {
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
        property_row(ui, "Label", text_muted, |ui| {
            let mut v = data.label.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        color_property(ui, "Active Color", text_muted, &mut data.active_color, entity, commands,
            |d, c| d.get_mut::<components::RadioButtonData>().map(|mut p| p.active_color = c));
    }

    // Dropdown
    if let Some(ref mut data) = snap.dropdown {
        section_header(ui, "Dropdown", text_muted);
        property_row(ui, "Placeholder", text_muted, |ui| {
            let mut v = data.placeholder.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        property_row(ui, "Selected", text_muted, |ui| {
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
            {
                if v != data.selected {
                    data.selected = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::DropdownData>() {
                                d.selected = v;
                            }
                        }
                    });
                }
            }
        });
        // Options list editor
        let mut options_changed = false;
        let mut new_options = data.options.clone();
        section_header(ui, "Options", text_muted);
        for i in 0..new_options.len() {
            property_row(ui, &format!("#{}", i + 1), text_muted, |ui| {
                if ui.add(egui::TextEdit::singleline(&mut new_options[i]).desired_width(ui.available_width())).changed() {
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
            if new_options.len() > 1 {
                if ui.small_button(format!("{} Remove", regular::MINUS)).clicked() {
                    new_options.pop();
                    options_changed = true;
                }
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

    // Text Input
    if let Some(ref mut data) = snap.text_input {
        section_header(ui, "Text Input", text_muted);
        property_row(ui, "Text", text_muted, |ui| {
            let mut v = data.text.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        property_row(ui, "Placeholder", text_muted, |ui| {
            let mut v = data.placeholder.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        property_row(ui, "Max Length", text_muted, |ui| {
            let mut v = data.max_length as i32;
            if ui.add(egui::DragValue::new(&mut v).range(1..=10000)).changed() {
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
        property_row(ui, "Password", text_muted, |ui| {
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

    // Scroll View
    if let Some(ref mut data) = snap.scroll_view {
        section_header(ui, "Scroll View", text_muted);
        property_row(ui, "Scroll Speed", text_muted, |ui| {
            let mut v = data.scroll_speed;
            if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(1.0..=200.0)).changed() {
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
        property_row(ui, "Horizontal", text_muted, |ui| {
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
        property_row(ui, "Vertical", text_muted, |ui| {
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

    // Tab Bar
    if let Some(ref mut data) = snap.tab_bar {
        section_header(ui, "Tab Bar", text_muted);
        property_row(ui, "Active Tab", text_muted, |ui| {
            let mut v = data.active as i32;
            if ui.add(egui::DragValue::new(&mut v).range(0..=(data.tabs.len() as i32 - 1).max(0))).changed() {
                data.active = v as usize;
                let active = v as usize;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::TabBarData>() {
                            d.active = active;
                        }
                    }
                });
            }
        });
        color_property(ui, "Tab Color", text_muted, &mut data.tab_color, entity, commands,
            |d, c| d.get_mut::<components::TabBarData>().map(|mut p| p.tab_color = c));
        color_property(ui, "Active Color", text_muted, &mut data.active_color, entity, commands,
            |d, c| d.get_mut::<components::TabBarData>().map(|mut p| p.active_color = c));
        // Tab list editor
        let mut tabs_changed = false;
        let mut new_tabs = data.tabs.clone();
        section_header(ui, "Tabs", text_muted);
        for i in 0..new_tabs.len() {
            property_row(ui, &format!("#{}", i + 1), text_muted, |ui| {
                if ui.add(egui::TextEdit::singleline(&mut new_tabs[i]).desired_width(ui.available_width())).changed() {
                    tabs_changed = true;
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if ui.small_button(format!("{} Add", regular::PLUS)).clicked() {
                new_tabs.push(format!("Tab {}", new_tabs.len() + 1));
                tabs_changed = true;
            }
            if new_tabs.len() > 1 {
                if ui.small_button(format!("{} Remove", regular::MINUS)).clicked() {
                    new_tabs.pop();
                    tabs_changed = true;
                }
            }
        });
        if tabs_changed {
            data.tabs = new_tabs.clone();
            commands.push(move |world: &mut World| {
                if let Ok(mut em) = world.get_entity_mut(entity) {
                    if let Some(mut d) = em.get_mut::<components::TabBarData>() {
                        d.tabs = new_tabs.clone();
                    }
                }
            });
        }
    }

    // Spinner
    if let Some(ref mut data) = snap.spinner {
        section_header(ui, "Spinner", text_muted);
        property_row(ui, "Speed", text_muted, |ui| {
            let mut v = data.speed;
            if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.1..=20.0)).changed() {
                data.speed = v;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        if let Some(mut d) = em.get_mut::<components::SpinnerData>() {
                            d.speed = v;
                        }
                    }
                });
            }
        });
        color_property(ui, "Color", text_muted, &mut data.color, entity, commands,
            |d, c| d.get_mut::<components::SpinnerData>().map(|mut p| p.color = c));
    }

    // Tooltip
    if let Some(ref mut data) = snap.tooltip {
        section_header(ui, "Tooltip", text_muted);
        property_row(ui, "Text", text_muted, |ui| {
            let mut v = data.text.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        property_row(ui, "Delay (ms)", text_muted, |ui| {
            let mut v = data.delay_ms as i32;
            if ui.add(egui::DragValue::new(&mut v).range(0..=5000)).changed() {
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
        color_property(ui, "Bg Color", text_muted, &mut data.bg_color, entity, commands,
            |d, c| d.get_mut::<components::TooltipData>().map(|mut p| p.bg_color = c));
        color_property(ui, "Text Color", text_muted, &mut data.text_color, entity, commands,
            |d, c| d.get_mut::<components::TooltipData>().map(|mut p| p.text_color = c));
    }

    // Modal
    if let Some(ref mut data) = snap.modal {
        section_header(ui, "Modal", text_muted);
        property_row(ui, "Title", text_muted, |ui| {
            let mut v = data.title.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        property_row(ui, "Closable", text_muted, |ui| {
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
        color_property(ui, "Backdrop", text_muted, &mut data.backdrop_color, entity, commands,
            |d, c| d.get_mut::<components::ModalData>().map(|mut p| p.backdrop_color = c));
    }

    // Draggable Window
    if let Some(ref mut data) = snap.draggable_window {
        section_header(ui, "Draggable Window", text_muted);
        property_row(ui, "Title", text_muted, |ui| {
            let mut v = data.title.clone();
            if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
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
        property_row(ui, "Closable", text_muted, |ui| {
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
        property_row(ui, "Minimizable", text_muted, |ui| {
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
        color_property(ui, "Title Bar", text_muted, &mut data.title_bar_color, entity, commands,
            |d, c| d.get_mut::<components::DraggableWindowData>().map(|mut p| p.title_bar_color = c));
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Color property editor that writes back to a component field via closure.
fn color_property(
    ui: &mut egui::Ui,
    label: &str,
    label_color: egui::Color32,
    color: &mut Color,
    entity: Entity,
    commands: &EditorCommands,
    apply: impl Fn(&mut bevy::ecs::world::EntityWorldMut, Color) -> Option<()> + Send + Sync + 'static,
) {
    property_row(ui, label, label_color, |ui| {
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
