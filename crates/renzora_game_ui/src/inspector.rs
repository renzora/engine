//! UI Inspector panel — property editor for selected UiWidget / UiCanvas entities.
//! Uses the same collapsible section + inline property pattern as the scene inspector.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor_framework::{
    collapsible_section, collapsible_section_removable, empty_state, inline_property,
    EditorCommands, EditorPanel, EditorSelection, PanelLocation,
};
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
    // Visibility
    is_visible: bool,
    // Layout (from Node)
    has_node: bool,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    margin: [f32; 4],
    position_type: u8,     // 0=Relative, 1=Absolute
    flex_direction: u8,    // 0=Row, 1=Column, 2=RowReverse, 3=ColumnReverse
    justify_content: u8,   // 0=Start, 1=Center, 2=End, 3=SpaceBetween, 4=SpaceAround, 5=SpaceEvenly
    align_items: u8,       // 0=Start, 1=Center, 2=End, 3=Stretch
    // Widget style (individual components, formerly UiWidgetStyle)
    fill: Option<components::UiFill>,
    stroke: Option<components::UiStroke>,
    border_radius: Option<components::UiBorderRadius>,
    shadow: Option<components::UiBoxShadow>,
    opacity: Option<components::UiOpacity>,
    clip_content: Option<components::UiClipContent>,
    cursor: Option<components::UiCursor>,
    text_style: Option<components::UiTextStyle>,
    padding: Option<components::UiPadding>,
    // Text (content lives on bevy Text component, style props on UiTextStyle)
    has_text: bool,
    text_content: String,
    // Interaction style
    interaction_style: Option<components::UiInteractionStyle>,
    transition_duration: Option<f32>,
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

        // Get selected entity
        let selected = selection.get();
        let Some(entity) = selected else {
            empty_state(
                ui,
                regular::CURSOR_CLICK,
                "No widget selected",
                "Select a UI widget or canvas to inspect its properties.",
                &theme,
            );
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

        // Visibility
        snap.is_visible = world
            .get::<Visibility>(entity)
            .map(|v| *v != Visibility::Hidden)
            .unwrap_or(true);

        // If neither canvas nor widget, show nothing
        if !snap.is_canvas && !snap.is_widget {
            empty_state(
                ui,
                regular::WARNING,
                "Not a UI element",
                "The selected entity is not a UI canvas or widget.",
                &theme,
            );
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
            snap.margin = [
                val_px(node.margin.top),
                val_px(node.margin.right),
                val_px(node.margin.bottom),
                val_px(node.margin.left),
            ];
        }

        // Widget style (individual components)
        snap.fill = world.get::<components::UiFill>(entity).cloned();
        snap.stroke = world.get::<components::UiStroke>(entity).cloned();
        snap.border_radius = world.get::<components::UiBorderRadius>(entity).cloned();
        snap.shadow = world.get::<components::UiBoxShadow>(entity).cloned();
        snap.opacity = world.get::<components::UiOpacity>(entity).cloned();
        snap.clip_content = world.get::<components::UiClipContent>(entity).cloned();
        snap.cursor = world.get::<components::UiCursor>(entity).cloned();
        snap.text_style = world.get::<components::UiTextStyle>(entity).cloned();
        snap.padding = world.get::<components::UiPadding>(entity).cloned();

        // Text
        snap.has_text = world.get::<bevy::ui::widget::Text>(entity).is_some();
        if let Some(text) = world.get::<bevy::ui::widget::Text>(entity) {
            snap.text_content = text.0.clone();
        }

        // Interaction style
        snap.interaction_style = world.get::<components::UiInteractionStyle>(entity).cloned();
        snap.transition_duration = world.get::<components::UiTransition>(entity).map(|t| t.duration);

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

        // ── Render sections ──────────────────────────────────────────────
        egui::ScrollArea::vertical()
            .id_salt("ui_inspector_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                // ── Identity section ──
                collapsible_section(
                    ui, regular::TAG, "Identity", "ui", &theme,
                    "ui_insp_identity", true,
                    |ui| {
                        let mut row = 0;
                        inline_property(ui, row, "Name", &theme, |ui| {
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
                        row += 1;
                        inline_property(ui, row, "Type", &theme, |ui| {
                            let type_name = if snap.is_canvas { "Canvas" } else { snap.widget_type.label() };
                            ui.label(egui::RichText::new(type_name).size(11.0));
                        });
                    },
                );

                // ── Visibility section ──
                {
                    let action = collapsible_section_removable(
                        ui, regular::EYE, "Visibility", "ui", &theme,
                        "ui_insp_visibility", true,
                        false, // can't remove
                        !snap.is_visible, // disabled = hidden
                        |ui| {
                            inline_property(ui, 0, "Visible", &theme, |ui| {
                                let mut v = snap.is_visible;
                                if ui.checkbox(&mut v, "").changed() {
                                    let new_vis = v;
                                    commands.push(move |world: &mut World| {
                                        if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
                                            *vis = if new_vis { Visibility::Inherited } else { Visibility::Hidden };
                                        }
                                    });
                                }
                            });
                            if snap.is_widget {
                                inline_property(ui, 1, "Locked", &theme, |ui| {
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
                        },
                    );
                    if action.toggle_clicked {
                        let new_vis = !snap.is_visible;
                        commands.push(move |world: &mut World| {
                            if let Some(mut vis) = world.get_mut::<Visibility>(entity) {
                                *vis = if new_vis { Visibility::Inherited } else { Visibility::Hidden };
                            }
                        });
                    }
                }

                // ── Canvas section ──
                if snap.is_canvas {
                    collapsible_section(
                        ui, regular::FRAME_CORNERS, "Canvas", "ui", &theme,
                        "ui_insp_canvas", true,
                        |ui| {
                            let mut row = 0;
                            inline_property(ui, row, "Sort Order", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Visibility", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Ref Width", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Ref Height", &theme, |ui| {
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
                            row += 1;
                            // Theme selector
                            let current_theme_name = world
                                .get_resource::<components::UiTheme>()
                                .map(|t| t.name.clone())
                                .unwrap_or_default();
                            inline_property(ui, row, "Theme", &theme, |ui| {
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
                        },
                    );
                }

                // ── Layout section ──
                if snap.has_node {
                    collapsible_section(
                        ui, regular::LAYOUT, "Layout", "transform", &theme,
                        "ui_insp_layout", true,
                        |ui| {
                            let mut row = 0;
                            inline_property(ui, row, "Position", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "X", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Y", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Width", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Height", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Direction", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Justify", &theme, |ui| {
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
                            row += 1;
                            inline_property(ui, row, "Align", &theme, |ui| {
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
                        },
                    );

                }

                // ── Fill section ──
                if let Some(ref mut fill) = snap.fill {
                    fill_section(ui, fill, entity, commands, &theme);
                }

                // ── Stroke section ──
                if let Some(ref mut stroke) = snap.stroke {
                    stroke_section(ui, stroke, entity, commands, &theme);
                }

                // ── Border Radius section ──
                if let Some(ref mut border_radius) = snap.border_radius {
                    border_radius_section(ui, border_radius, entity, commands, &theme);
                }

                // ── Text section ──
                if snap.has_text || snap.text_style.is_some() {
                    text_section(ui, snap, entity, commands, &theme);
                }

                // ── Padding section ──
                if let Some(ref mut padding) = snap.padding {
                    padding_section(ui, padding, entity, commands, &theme);
                }

                // ── Effects section (opacity, shadow, clip, cursor) ──
                {
                    let has_effects = snap.opacity.is_some() || snap.clip_content.is_some()
                        || snap.cursor.is_some() || snap.shadow.is_some();
                    if has_effects {
                        effects_section(ui, snap, entity, commands, &theme);
                    }
                }

                // ── Interaction States section ──
                if snap.interaction_style.is_some() || snap.fill.is_some() {
                    interaction_states_section(ui, snap, entity, commands, &theme);
                }

                // ── Widget-specific data sections ──
                widget_data_sections(ui, snap, entity, commands, &theme);

                // ── Delete widget (removable section style) ──
                {
                    let action = collapsible_section_removable(
                        ui, regular::TRASH, "Delete Widget", "ui", &theme,
                        "ui_insp_delete", false,
                        true, // can_remove
                        false,
                        |ui| {
                            ui.label(
                                egui::RichText::new("Click the trash icon to remove this widget.")
                                    .size(11.0)
                                    .color(theme.text.muted.to_color32()),
                            );
                        },
                    );
                    if action.remove_clicked {
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

                ui.add_space(8.0);
            });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }
}

// ── Fill section ─────────────────────────────────────────────────────────────

fn fill_section(
    ui: &mut egui::Ui,
    fill: &mut components::UiFill,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    collapsible_section(
        ui, regular::DROP_HALF, "Fill", "rendering", theme,
        "ui_insp_fill", true,
        |ui| {
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
                                components::GradientStop { position: 0.0, color: Color::WHITE },
                                components::GradientStop { position: 1.0, color: Color::BLACK },
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
                components::UiFill::LinearGradient { ref mut angle, ref mut stops } => {
                    let mut fill_changed = false;
                    inline_property(ui, 1, "Angle", theme, |ui| {
                        let mut v = *angle;
                        if ui.add(egui::DragValue::new(&mut v).speed(1.0).range(0.0..=360.0).suffix("°")).changed() {
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
                components::UiFill::RadialGradient { ref mut center, ref mut stops } => {
                    let mut fill_changed = false;
                    inline_property(ui, 1, "Center X", theme, |ui| {
                        let mut v = center[0];
                        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0)).changed() {
                            center[0] = v;
                            fill_changed = true;
                        }
                    });
                    inline_property(ui, 2, "Center Y", theme, |ui| {
                        let mut v = center[1];
                        if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0)).changed() {
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
        },
    );
}

/// Renders gradient stop editors. Returns true if any stop was modified.
fn gradient_stops_editor(
    ui: &mut egui::Ui,
    stops: &mut Vec<components::GradientStop>,
    start_row: usize,
    theme: &renzora_theme::Theme,
) -> bool {
    let mut changed = false;
    for i in 0..stops.len() {
        let row = start_row + i * 2;
        inline_property(ui, row, &format!("Stop {} Pos", i + 1), theme, |ui| {
            let mut v = stops[i].position;
            if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0)).changed() {
                stops[i].position = v;
                changed = true;
            }
        });
        inline_property(ui, row + 1, &format!("Stop {} Color", i + 1), theme, |ui| {
            let mut arr = color_to_arr(stops[i].color);
            if ui.color_edit_button_rgba_unmultiplied(&mut arr).changed() {
                stops[i].color = arr_to_color(arr);
                changed = true;
            }
        });
    }

    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.small_button(format!("{} Add Stop", regular::PLUS)).clicked() {
            stops.push(components::GradientStop {
                position: 1.0,
                color: Color::WHITE,
            });
            changed = true;
        }
        if stops.len() > 2 {
            if ui.small_button(format!("{} Remove", regular::MINUS)).clicked() {
                stops.pop();
                changed = true;
            }
        }
    });

    changed
}

// ── Stroke section ───────────────────────────────────────────────────────────

fn stroke_section(
    ui: &mut egui::Ui,
    stroke: &mut components::UiStroke,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    collapsible_section(
        ui, regular::BOUNDING_BOX, "Stroke", "rendering", theme,
        "ui_insp_stroke", false,
        |ui| {
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
                if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.0..=50.0).suffix("px")).changed() {
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
                    let side_labels = [("T", &mut stroke.sides.top),
                                       ("R", &mut stroke.sides.right),
                                       ("B", &mut stroke.sides.bottom),
                                       ("L", &mut stroke.sides.left)];
                    for (label, val) in side_labels {
                        if ui.selectable_label(*val, label).clicked() {
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
        },
    );
}

// ── Border Radius section ────────────────────────────────────────────────────

fn border_radius_section(
    ui: &mut egui::Ui,
    border_radius: &mut components::UiBorderRadius,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    collapsible_section(
        ui, regular::FRAME_CORNERS, "Border Radius", "rendering", theme,
        "ui_insp_radius", false,
        |ui| {
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
                    if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.0..=500.0).suffix("px")).changed() {
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
        },
    );
}

// ── Text section ─────────────────────────────────────────────────────────────

fn text_section(
    ui: &mut egui::Ui,
    snap: &mut InspectorSnapshot,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    collapsible_section(
        ui, regular::TEXT_AA, "Text", "ui", theme,
        "ui_insp_text", true,
        |ui| {
            let mut row = 0;
            // Content (writes to bevy Text component)
            if snap.has_text {
                inline_property(ui, row, "Content", theme, |ui| {
                    let mut v = snap.text_content.clone();
                    if ui.add(
                        egui::TextEdit::multiline(&mut v)
                            .desired_width(ui.available_width())
                            .desired_rows(2),
                    ).changed() {
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
                row += 1;
            }

            // Style props from UiTextStyle component
            if let Some(ref mut text_style) = snap.text_style {
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
                    if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(1.0..=200.0).suffix("px")).changed() {
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
        },
    );
}

// ── Padding section ──────────────────────────────────────────────────────────

fn padding_section(
    ui: &mut egui::Ui,
    padding: &mut components::UiPadding,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    collapsible_section(
        ui, regular::COLUMNS, "Padding", "transform", theme,
        "ui_insp_padding", false,
        |ui| {
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
                    if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.0..=500.0).suffix("px")).changed() {
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
        },
    );
}

// ── Effects section (opacity, shadow, clip, cursor) ──────────────────────────

fn effects_section(
    ui: &mut egui::Ui,
    snap: &mut InspectorSnapshot,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    collapsible_section(
        ui, regular::SPARKLE, "Effects", "rendering", theme,
        "ui_insp_effects", false,
        |ui| {
            let mut row = 0;
            // Opacity
            if let Some(ref mut opacity) = snap.opacity {
                inline_property(ui, row, "Opacity", theme, |ui| {
                    let mut v = opacity.0;
                    if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0)).changed() {
                        opacity.0 = v;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut o) = em.get_mut::<components::UiOpacity>() {
                                    o.0 = v;
                                }
                            }
                        });
                    }
                });
                row += 1;
            }

            // Clip Content
            if let Some(ref mut clip) = snap.clip_content {
                inline_property(ui, row, "Clip Content", theme, |ui| {
                    let mut v = clip.0;
                    if ui.checkbox(&mut v, "").changed() {
                        clip.0 = v;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut c) = em.get_mut::<components::UiClipContent>() {
                                    c.0 = v;
                                }
                            }
                        });
                    }
                });
                row += 1;
            }

            // Cursor
            if let Some(ref mut cursor) = snap.cursor {
                inline_property(ui, row, "Cursor", theme, |ui| {
                    let cursor_labels = [
                        "Default", "Pointer", "Text", "Grab", "Grabbing",
                        "Not Allowed", "Crosshair", "EW Resize", "NS Resize", "Move",
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
                        .show_index(ui, &mut idx, cursor_labels.len(), |i| cursor_labels[i].to_string())
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
                        let new_cursor = *cursor;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut c) = em.get_mut::<components::UiCursor>() {
                                    *c = new_cursor;
                                }
                            }
                        });
                    }
                });
                row += 1;
            }

            // Shadow toggle + properties
            let has_shadow = snap.shadow.is_some();
            inline_property(ui, row, "Shadow", theme, |ui| {
                let mut v = has_shadow;
                if ui.checkbox(&mut v, "").changed() {
                    if v {
                        snap.shadow = Some(components::UiBoxShadow::default());
                        let shadow = snap.shadow.clone().unwrap();
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                em.insert(shadow);
                            }
                        });
                    } else {
                        snap.shadow = None;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                em.remove::<components::UiBoxShadow>();
                            }
                        });
                    }
                }
            });
            row += 1;

            if let Some(ref mut shadow) = snap.shadow {
                inline_property(ui, row, "Shadow Color", theme, |ui| {
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
                    if ui.add(egui::DragValue::new(&mut v).speed(0.5).suffix("px")).changed() {
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
                    if ui.add(egui::DragValue::new(&mut v).speed(0.5).suffix("px")).changed() {
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
                    if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.0..=200.0).suffix("px")).changed() {
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
                    if ui.add(egui::DragValue::new(&mut v).speed(0.5).suffix("px")).changed() {
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
        },
    );
}

// ── Interaction States section ────────────────────────────────────────────────

fn interaction_states_section(
    ui: &mut egui::Ui,
    snap: &mut InspectorSnapshot,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    // If no interaction style exists yet, offer to add one
    let has_interaction = snap.interaction_style.is_some();
    collapsible_section(
        ui, regular::CURSOR_CLICK, "Interaction States", "ui", theme,
        "ui_insp_interaction", false,
        |ui| {
            if !has_interaction {
                if ui.small_button(format!("{} Add Interaction Style", regular::PLUS)).clicked() {
                    snap.interaction_style = Some(components::UiInteractionStyle::default());
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            em.insert(components::UiInteractionStyle::default());
                        }
                    });
                }
                return;
            }

            let istyle = snap.interaction_style.as_mut().unwrap();

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
                let has_transition = snap.transition_duration.is_some();
                ui.horizontal(|ui| {
                    let mut enabled = has_transition;
                    if ui.checkbox(&mut enabled, "").changed() {
                        if enabled {
                            snap.transition_duration = Some(0.15);
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    em.insert(components::UiTransition { duration: 0.15 });
                                }
                            });
                        } else {
                            snap.transition_duration = None;
                            commands.push(move |world: &mut World| {
                                if let Ok(mut em) = world.get_entity_mut(entity) {
                                    em.remove::<components::UiTransition>();
                                }
                            });
                        }
                    }
                    if let Some(ref mut dur) = snap.transition_duration {
                        if ui.add(egui::DragValue::new(dur).speed(0.01).range(0.0..=5.0).suffix("s")).changed() {
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

            // Remove button
            ui.add_space(4.0);
            if ui.small_button(format!("{} Remove Interaction Style", regular::MINUS)).clicked() {
                snap.interaction_style = None;
                commands.push(move |world: &mut World| {
                    if let Ok(mut em) = world.get_entity_mut(entity) {
                        em.remove::<components::UiInteractionStyle>();
                    }
                });
            }
        },
    );
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
            } else if matches!(state.fill, Some(components::UiFill::LinearGradient { .. } | components::UiFill::RadialGradient { .. })) {
                ui.label(egui::RichText::new("gradient").size(10.0).color(theme.text.muted.to_color32()));
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
                if ui.add(egui::DragValue::new(&mut w).speed(0.5).range(0.0..=50.0).suffix("px")).changed() {
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
                if ui.add(egui::DragValue::new(opacity).speed(0.01).range(0.0..=1.0)).changed() {
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
                if ui.add(egui::DragValue::new(size).speed(0.5).range(1.0..=200.0).suffix("px")).changed() {
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
                state.cursor = if enabled { Some(components::UiCursor::Pointer) } else { None };
                dirty = true;
            }
            if let Some(ref mut cursor) = state.cursor {
                let cursor_labels = [
                    "Default", "Pointer", "Text", "Grab", "Grabbing",
                    "Not Allowed", "Crosshair", "EW Resize", "NS Resize", "Move",
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
                    .show_index(ui, &mut idx, cursor_labels.len(), |i| cursor_labels[i].to_string())
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
                if ui.add(egui::DragValue::new(scale).speed(0.01).range(0.1..=5.0)).changed() {
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

// ── Widget-specific property sections ────────────────────────────────────────

fn widget_data_sections(
    ui: &mut egui::Ui,
    snap: &mut InspectorSnapshot,
    entity: Entity,
    commands: &EditorCommands,
    theme: &renzora_theme::Theme,
) {
    // Progress Bar
    if let Some(ref mut data) = snap.progress_bar {
        collapsible_section(ui, regular::CHART_BAR, "Progress Bar", "ui", theme, "ui_insp_progress", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Value", theme, |ui| {
                let mut v = data.value;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=data.max)).changed() {
                    data.value = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ProgressBarData>() { d.value = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Max", theme, |ui| {
                let mut v = data.max;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.001..=f32::MAX)).changed() {
                    data.max = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ProgressBarData>() { d.max = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Direction", theme, |ui| {
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
                            if let Some(mut d) = em.get_mut::<components::ProgressBarData>() { d.direction = dir; }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Fill Color", theme, &mut data.fill_color, entity, commands,
                |d, c| d.get_mut::<components::ProgressBarData>().map(|mut p| p.fill_color = c));
            row += 1;
            color_row(ui, row, "Bg Color", theme, &mut data.bg_color, entity, commands,
                |d, c| d.get_mut::<components::ProgressBarData>().map(|mut p| p.bg_color = c));
        });
    }

    // Health Bar
    if let Some(ref mut data) = snap.health_bar {
        collapsible_section(ui, regular::HEART, "Health Bar", "ui", theme, "ui_insp_health", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Current", theme, |ui| {
                let mut v = data.current;
                if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.0..=data.max)).changed() {
                    data.current = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::HealthBarData>() { d.current = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Max", theme, |ui| {
                let mut v = data.max;
                if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(0.001..=f32::MAX)).changed() {
                    data.max = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::HealthBarData>() { d.max = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Low Threshold", theme, |ui| {
                let mut v = data.low_threshold;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=1.0)).changed() {
                    data.low_threshold = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::HealthBarData>() { d.low_threshold = v; }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Fill Color", theme, &mut data.fill_color, entity, commands,
                |d, c| d.get_mut::<components::HealthBarData>().map(|mut p| p.fill_color = c));
            row += 1;
            color_row(ui, row, "Low Color", theme, &mut data.low_color, entity, commands,
                |d, c| d.get_mut::<components::HealthBarData>().map(|mut p| p.low_color = c));
            row += 1;
            color_row(ui, row, "Bg Color", theme, &mut data.bg_color, entity, commands,
                |d, c| d.get_mut::<components::HealthBarData>().map(|mut p| p.bg_color = c));
        });
    }

    // Slider
    if let Some(ref mut data) = snap.slider {
        collapsible_section(ui, regular::SLIDERS_HORIZONTAL, "Slider", "ui", theme, "ui_insp_slider", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Value", theme, |ui| {
                let mut v = data.value;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(data.min..=data.max)).changed() {
                    data.value = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::SliderData>() { d.value = v; }
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
                            if let Some(mut d) = em.get_mut::<components::SliderData>() { d.min = v; }
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
                            if let Some(mut d) = em.get_mut::<components::SliderData>() { d.max = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Step", theme, |ui| {
                let mut v = data.step;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01).range(0.0..=f32::MAX)).changed() {
                    data.step = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::SliderData>() { d.step = v; }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Track Color", theme, &mut data.track_color, entity, commands,
                |d, c| d.get_mut::<components::SliderData>().map(|mut p| p.track_color = c));
            row += 1;
            color_row(ui, row, "Fill Color", theme, &mut data.fill_color, entity, commands,
                |d, c| d.get_mut::<components::SliderData>().map(|mut p| p.fill_color = c));
            row += 1;
            color_row(ui, row, "Thumb Color", theme, &mut data.thumb_color, entity, commands,
                |d, c| d.get_mut::<components::SliderData>().map(|mut p| p.thumb_color = c));
        });
    }

    // Checkbox
    if let Some(ref mut data) = snap.checkbox {
        collapsible_section(ui, regular::CHECK_SQUARE, "Checkbox", "ui", theme, "ui_insp_checkbox", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Checked", theme, |ui| {
                let mut v = data.checked;
                if ui.checkbox(&mut v, "").changed() {
                    data.checked = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::CheckboxData>() { d.checked = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Label", theme, |ui| {
                let mut v = data.label.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.label = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::CheckboxData>() { d.label = v.clone(); }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Check Color", theme, &mut data.check_color, entity, commands,
                |d, c| d.get_mut::<components::CheckboxData>().map(|mut p| p.check_color = c));
            row += 1;
            color_row(ui, row, "Box Color", theme, &mut data.box_color, entity, commands,
                |d, c| d.get_mut::<components::CheckboxData>().map(|mut p| p.box_color = c));
        });
    }

    // Toggle
    if let Some(ref mut data) = snap.toggle {
        collapsible_section(ui, regular::TOGGLE_LEFT, "Toggle", "ui", theme, "ui_insp_toggle", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "On", theme, |ui| {
                let mut v = data.on;
                if ui.checkbox(&mut v, "").changed() {
                    data.on = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ToggleData>() { d.on = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Label", theme, |ui| {
                let mut v = data.label.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.label = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ToggleData>() { d.label = v.clone(); }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "On Color", theme, &mut data.on_color, entity, commands,
                |d, c| d.get_mut::<components::ToggleData>().map(|mut p| p.on_color = c));
            row += 1;
            color_row(ui, row, "Off Color", theme, &mut data.off_color, entity, commands,
                |d, c| d.get_mut::<components::ToggleData>().map(|mut p| p.off_color = c));
            row += 1;
            color_row(ui, row, "Knob Color", theme, &mut data.knob_color, entity, commands,
                |d, c| d.get_mut::<components::ToggleData>().map(|mut p| p.knob_color = c));
        });
    }

    // Radio Button
    if let Some(ref mut data) = snap.radio_button {
        collapsible_section(ui, regular::RADIO_BUTTON, "Radio Button", "ui", theme, "ui_insp_radio", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Group", theme, |ui| {
                let mut v = data.group.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.group = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::RadioButtonData>() { d.group = v.clone(); }
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
                            if let Some(mut d) = em.get_mut::<components::RadioButtonData>() { d.selected = v; }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Label", theme, |ui| {
                let mut v = data.label.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.label = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::RadioButtonData>() { d.label = v.clone(); }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Active Color", theme, &mut data.active_color, entity, commands,
                |d, c| d.get_mut::<components::RadioButtonData>().map(|mut p| p.active_color = c));
        });
    }

    // Dropdown
    if let Some(ref mut data) = snap.dropdown {
        collapsible_section(ui, regular::CARET_CIRCLE_DOWN, "Dropdown", "ui", theme, "ui_insp_dropdown", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Placeholder", theme, |ui| {
                let mut v = data.placeholder.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.placeholder = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::DropdownData>() { d.placeholder = v.clone(); }
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
                {
                    if v != data.selected {
                        data.selected = v;
                        commands.push(move |world: &mut World| {
                            if let Ok(mut em) = world.get_entity_mut(entity) {
                                if let Some(mut d) = em.get_mut::<components::DropdownData>() { d.selected = v; }
                            }
                        });
                    }
                }
            });
            // Options list
            let mut options_changed = false;
            let mut new_options = data.options.clone();
            for i in 0..new_options.len() {
                inline_property(ui, i + 2, &format!("#{}", i + 1), theme, |ui| {
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
                        if let Some(mut d) = em.get_mut::<components::DropdownData>() { d.options = new_options.clone(); }
                    }
                });
            }
        });
    }

    // Text Input
    if let Some(ref mut data) = snap.text_input {
        collapsible_section(ui, regular::TEXTBOX, "Text Input", "ui", theme, "ui_insp_text_input", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Text", theme, |ui| {
                let mut v = data.text.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.text = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() { d.text = v.clone(); }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Placeholder", theme, |ui| {
                let mut v = data.placeholder.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.placeholder = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() { d.placeholder = v.clone(); }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Max Length", theme, |ui| {
                let mut v = data.max_length as i32;
                if ui.add(egui::DragValue::new(&mut v).range(1..=10000)).changed() {
                    data.max_length = v as usize;
                    let len = v as usize;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() { d.max_length = len; }
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
                            if let Some(mut d) = em.get_mut::<components::TextInputData>() { d.password = v; }
                        }
                    });
                }
            });
        });
    }

    // Scroll View
    if let Some(ref mut data) = snap.scroll_view {
        collapsible_section(ui, regular::SCROLL, "Scroll View", "ui", theme, "ui_insp_scroll", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Scroll Speed", theme, |ui| {
                let mut v = data.scroll_speed;
                if ui.add(egui::DragValue::new(&mut v).speed(0.5).range(1.0..=200.0)).changed() {
                    data.scroll_speed = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ScrollViewData>() { d.scroll_speed = v; }
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
                            if let Some(mut d) = em.get_mut::<components::ScrollViewData>() { d.show_horizontal = v; }
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
                            if let Some(mut d) = em.get_mut::<components::ScrollViewData>() { d.show_vertical = v; }
                        }
                    });
                }
            });
        });
    }

    // Tab Bar
    if let Some(ref mut data) = snap.tab_bar {
        collapsible_section(ui, regular::TABS, "Tab Bar", "ui", theme, "ui_insp_tabbar", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Active Tab", theme, |ui| {
                let mut v = data.active as i32;
                if ui.add(egui::DragValue::new(&mut v).range(0..=(data.tabs.len() as i32 - 1).max(0))).changed() {
                    data.active = v as usize;
                    let active = v as usize;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TabBarData>() { d.active = active; }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Tab Color", theme, &mut data.tab_color, entity, commands,
                |d, c| d.get_mut::<components::TabBarData>().map(|mut p| p.tab_color = c));
            row += 1;
            color_row(ui, row, "Active Color", theme, &mut data.active_color, entity, commands,
                |d, c| d.get_mut::<components::TabBarData>().map(|mut p| p.active_color = c));
            // Tab list
            let mut tabs_changed = false;
            let mut new_tabs = data.tabs.clone();
            for i in 0..new_tabs.len() {
                inline_property(ui, i + row + 1, &format!("#{}", i + 1), theme, |ui| {
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
                        if let Some(mut d) = em.get_mut::<components::TabBarData>() { d.tabs = new_tabs.clone(); }
                    }
                });
            }
        });
    }

    // Spinner
    if let Some(ref mut data) = snap.spinner {
        collapsible_section(ui, regular::SPINNER, "Spinner", "ui", theme, "ui_insp_spinner", true, |ui| {
            inline_property(ui, 0, "Speed", theme, |ui| {
                let mut v = data.speed;
                if ui.add(egui::DragValue::new(&mut v).speed(0.1).range(0.1..=20.0)).changed() {
                    data.speed = v;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::SpinnerData>() { d.speed = v; }
                        }
                    });
                }
            });
            color_row(ui, 1, "Color", theme, &mut data.color, entity, commands,
                |d, c| d.get_mut::<components::SpinnerData>().map(|mut p| p.color = c));
        });
    }

    // Tooltip
    if let Some(ref mut data) = snap.tooltip {
        collapsible_section(ui, regular::CHAT_CIRCLE, "Tooltip", "ui", theme, "ui_insp_tooltip", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Text", theme, |ui| {
                let mut v = data.text.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.text = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TooltipData>() { d.text = v.clone(); }
                        }
                    });
                }
            });
            row += 1;
            inline_property(ui, row, "Delay (ms)", theme, |ui| {
                let mut v = data.delay_ms as i32;
                if ui.add(egui::DragValue::new(&mut v).range(0..=5000)).changed() {
                    data.delay_ms = v as u32;
                    let delay = v as u32;
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::TooltipData>() { d.delay_ms = delay; }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Bg Color", theme, &mut data.bg_color, entity, commands,
                |d, c| d.get_mut::<components::TooltipData>().map(|mut p| p.bg_color = c));
            row += 1;
            color_row(ui, row, "Text Color", theme, &mut data.text_color, entity, commands,
                |d, c| d.get_mut::<components::TooltipData>().map(|mut p| p.text_color = c));
        });
    }

    // Modal
    if let Some(ref mut data) = snap.modal {
        collapsible_section(ui, regular::BROWSER, "Modal", "ui", theme, "ui_insp_modal", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Title", theme, |ui| {
                let mut v = data.title.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.title = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::ModalData>() { d.title = v.clone(); }
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
                            if let Some(mut d) = em.get_mut::<components::ModalData>() { d.closable = v; }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Backdrop", theme, &mut data.backdrop_color, entity, commands,
                |d, c| d.get_mut::<components::ModalData>().map(|mut p| p.backdrop_color = c));
        });
    }

    // Draggable Window
    if let Some(ref mut data) = snap.draggable_window {
        collapsible_section(ui, regular::APP_WINDOW, "Draggable Window", "ui", theme, "ui_insp_dragwin", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Title", theme, |ui| {
                let mut v = data.title.clone();
                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).changed() {
                    data.title = v.clone();
                    commands.push(move |world: &mut World| {
                        if let Ok(mut em) = world.get_entity_mut(entity) {
                            if let Some(mut d) = em.get_mut::<components::DraggableWindowData>() { d.title = v.clone(); }
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
                            if let Some(mut d) = em.get_mut::<components::DraggableWindowData>() { d.closable = v; }
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
                            if let Some(mut d) = em.get_mut::<components::DraggableWindowData>() { d.minimizable = v; }
                        }
                    });
                }
            });
            row += 1;
            color_row(ui, row, "Title Bar", theme, &mut data.title_bar_color, entity, commands,
                |d, c| d.get_mut::<components::DraggableWindowData>().map(|mut p| p.title_bar_color = c));
        });
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
