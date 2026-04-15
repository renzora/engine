//! Live animator parameter panel — Unity Animator window-style.
//!
//! Shows the float / bool / trigger parameters from the selected entity's
//! `AnimatorState.params`, lets you add/remove them, edit their values live
//! while in play mode, and fire triggers with a button. Transitions in the
//! state machine reference these parameters by name.
//!
//! Panel id: `animator_params`.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::AnimatorState;
use renzora_editor_framework::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::{AnimEditorAction, AnimationEditorState};

#[derive(Default, Clone, Copy, PartialEq)]
enum ParamKind {
    #[default]
    Float,
    Bool,
    Trigger,
}

pub struct AnimatorParamsPanel {
    bridge: Arc<Mutex<Vec<AnimEditorAction>>>,
    // UI state — scratch for the "add new parameter" row.
    new_param_state: Arc<Mutex<(String, ParamKind)>>,
}

impl AnimatorParamsPanel {
    pub fn new(bridge: Arc<Mutex<Vec<AnimEditorAction>>>) -> Self {
        Self {
            bridge,
            new_param_state: Arc::new(Mutex::new((String::new(), ParamKind::Float))),
        }
    }

    fn push(&self, action: AnimEditorAction) {
        if let Ok(mut q) = self.bridge.lock() {
            q.push(action);
        }
    }
}

impl EditorPanel for AnimatorParamsPanel {
    fn id(&self) -> &str {
        "animator_params"
    }

    fn title(&self) -> &str {
        "Parameters"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::SLIDERS)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }

    fn min_size(&self) -> [f32; 2] {
        [220.0, 160.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>();
        let (text, muted, accent, surface, border) = if let Some(tm) = theme {
            let t = &tm.active_theme;
            (
                t.text.primary.to_color32(),
                t.text.muted.to_color32(),
                t.semantic.accent.to_color32(),
                t.surfaces.faint.to_color32(),
                t.widgets.border.to_color32(),
            )
        } else {
            (
                egui::Color32::WHITE,
                egui::Color32::GRAY,
                egui::Color32::from_rgb(100, 149, 237),
                egui::Color32::from_rgb(30, 31, 36),
                egui::Color32::from_rgb(50, 51, 56),
            )
        };

        let Some(editor_state) = world.get_resource::<AnimationEditorState>() else {
            return;
        };
        let Some(entity) = editor_state.selected_entity else {
            self.empty(ui, muted, "Select an entity with an animator");
            return;
        };
        let Some(state) = world.get::<AnimatorState>(entity) else {
            self.empty(ui, muted, "No AnimatorState on selected entity");
            return;
        };

        // ── Header: add new param row ──
        egui::Frame::new()
            .fill(surface)
            .stroke(egui::Stroke::new(0.5, border))
            .inner_margin(egui::Margin::same(6))
            .corner_radius(3.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut new_state = self.new_param_state.lock().unwrap();
                    let (name, kind) = &mut *new_state;

                    ui.label(egui::RichText::new("New").size(10.0).color(muted));
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .hint_text("name")
                            .desired_width(120.0),
                    );
                    egui::ComboBox::from_id_salt("anim_param_kind")
                        .selected_text(match kind {
                            ParamKind::Float => "Float",
                            ParamKind::Bool => "Bool",
                            ParamKind::Trigger => "Trigger",
                        })
                        .width(72.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(kind, ParamKind::Float, "Float");
                            ui.selectable_value(kind, ParamKind::Bool, "Bool");
                            ui.selectable_value(kind, ParamKind::Trigger, "Trigger");
                        });

                    let can_add = !name.trim().is_empty();
                    let btn = egui::Button::new(
                        egui::RichText::new(egui_phosphor::regular::PLUS)
                            .size(12.0)
                            .color(if can_add { accent } else { muted }),
                    )
                    .min_size(egui::vec2(22.0, 22.0));
                    if ui.add_enabled(can_add, btn).clicked() {
                        let n = name.trim().to_string();
                        let k = *kind;
                        *name = String::new();
                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                            cmds.push(move |world: &mut World| {
                                let Some(mut s) = world.get_mut::<AnimatorState>(entity)
                                else {
                                    return;
                                };
                                match k {
                                    ParamKind::Float => {
                                        s.params.floats.entry(n).or_insert(0.0);
                                    }
                                    ParamKind::Bool => {
                                        s.params.bools.entry(n).or_insert(false);
                                    }
                                    ParamKind::Trigger => {
                                        s.params.triggers.entry(n).or_insert(false);
                                    }
                                }
                            });
                        }
                    }
                });
            });

        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // ── Floats ──
            let floats: Vec<(String, f32)> = state
                .params
                .floats
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            if !floats.is_empty() {
                ui.label(egui::RichText::new("FLOATS").size(10.0).color(muted));
                for (name, value) in floats {
                    let name_for_set = name.clone();
                    let name_for_del = name.clone();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&name).size(11.0).color(text));
                        let mut v = value;
                        if ui
                            .add(egui::DragValue::new(&mut v).speed(0.05).fixed_decimals(3))
                            .changed()
                        {
                            self.push(AnimEditorAction::SetParam {
                                name: name_for_set,
                                value: v,
                            });
                        }
                        if ui
                            .button(egui::RichText::new(egui_phosphor::regular::TRASH).size(10.0).color(muted))
                            .clicked()
                        {
                            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                cmds.push(move |world: &mut World| {
                                    if let Some(mut s) = world.get_mut::<AnimatorState>(entity) {
                                        s.params.floats.remove(&name_for_del);
                                    }
                                });
                            }
                        }
                    });
                }
                ui.add_space(6.0);
            }

            // ── Bools ──
            let bools: Vec<(String, bool)> = state
                .params
                .bools
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            if !bools.is_empty() {
                ui.label(egui::RichText::new("BOOLS").size(10.0).color(muted));
                for (name, value) in bools {
                    let name_for_set = name.clone();
                    let name_for_del = name.clone();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&name).size(11.0).color(text));
                        let mut v = value;
                        if ui.checkbox(&mut v, "").changed() {
                            self.push(AnimEditorAction::SetBoolParam {
                                name: name_for_set,
                                value: v,
                            });
                        }
                        if ui
                            .button(egui::RichText::new(egui_phosphor::regular::TRASH).size(10.0).color(muted))
                            .clicked()
                        {
                            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                cmds.push(move |world: &mut World| {
                                    if let Some(mut s) = world.get_mut::<AnimatorState>(entity) {
                                        s.params.bools.remove(&name_for_del);
                                    }
                                });
                            }
                        }
                    });
                }
                ui.add_space(6.0);
            }

            // ── Triggers ──
            let triggers: Vec<String> =
                state.params.triggers.keys().cloned().collect();
            if !triggers.is_empty() {
                ui.label(egui::RichText::new("TRIGGERS").size(10.0).color(muted));
                for name in triggers {
                    let name_for_fire = name.clone();
                    let name_for_del = name.clone();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&name).size(11.0).color(text));
                        if ui
                            .button(
                                egui::RichText::new(egui_phosphor::regular::LIGHTNING)
                                    .size(11.0)
                                    .color(accent),
                            )
                            .on_hover_text("Fire trigger")
                            .clicked()
                        {
                            self.push(AnimEditorAction::FireTrigger { name: name_for_fire });
                        }
                        if ui
                            .button(egui::RichText::new(egui_phosphor::regular::TRASH).size(10.0).color(muted))
                            .clicked()
                        {
                            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                cmds.push(move |world: &mut World| {
                                    if let Some(mut s) = world.get_mut::<AnimatorState>(entity) {
                                        s.params.triggers.remove(&name_for_del);
                                    }
                                });
                            }
                        }
                    });
                }
            }

            if state.params.floats.is_empty()
                && state.params.bools.is_empty()
                && state.params.triggers.is_empty()
            {
                self.empty(ui, muted, "No parameters. Add one above.");
            }
        });
    }
}

impl AnimatorParamsPanel {
    fn empty(&self, ui: &mut egui::Ui, color: egui::Color32, msg: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::SLIDERS)
                    .size(22.0)
                    .color(color),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new(msg).size(11.0).color(color));
        });
    }
}
