//! State machine editor — form-based (states list + transitions list) editor
//! for `.animsm` files. A node-graph visualization can be layered on later
//! using `renzora_ui::widgets::node_graph`; this panel ships the essentials
//! so state machines become authorable today.
//!
//! Panel id: `animator_state_machine`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::{
    state_machine::{AnimCondition, AnimState, AnimTransition, AnimationStateMachine, StateMotion},
    AnimatorComponent,
};
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::AnimationEditorState;

/// Local editor buffer that mirrors the on-disk `.animsm` file while the user
/// is editing. Flushed to disk on Save.
#[derive(Clone)]
struct SmBuffer {
    path: Option<PathBuf>,
    sm: AnimationStateMachine,
    dirty: bool,
    error: Option<String>,
}

impl Default for SmBuffer {
    fn default() -> Self {
        Self {
            path: None,
            sm: Self::fresh(),
            dirty: false,
            error: None,
        }
    }
}

impl SmBuffer {
    fn fresh() -> AnimationStateMachine {
        AnimationStateMachine {
            states: Vec::new(),
            transitions: Vec::new(),
            default_state: String::new(),
        }
    }
}

pub struct StateMachinePanel {
    buffer: Arc<Mutex<SmBuffer>>,
    loaded_for: Arc<Mutex<Option<PathBuf>>>,
}

impl StateMachinePanel {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(SmBuffer::default())),
            loaded_for: Arc::new(Mutex::new(None)),
        }
    }

    /// Read the `.animsm` on disk (or init fresh) for the path currently
    /// referenced by the selected entity's `AnimatorComponent.state_machine`.
    fn ensure_loaded(&self, project_root: &std::path::Path, sm_rel_path: &str) {
        let abs = project_root.join(sm_rel_path);
        let already = { self.loaded_for.lock().unwrap().as_deref() == Some(&abs) };
        if already {
            return;
        }
        let mut buf = self.buffer.lock().unwrap();
        buf.error = None;
        buf.dirty = false;
        buf.path = Some(abs.clone());
        buf.sm = match std::fs::read_to_string(&abs) {
            Ok(s) => match ron::de::from_str::<AnimationStateMachine>(&s) {
                Ok(sm) => sm,
                Err(e) => {
                    buf.error = Some(format!("Parse error: {e}"));
                    SmBuffer::fresh()
                }
            },
            Err(_) => SmBuffer::fresh(),
        };
        *self.loaded_for.lock().unwrap() = Some(abs);
    }

    fn save(&self) -> Result<(), String> {
        let buf = self.buffer.lock().unwrap();
        let Some(path) = buf.path.clone() else {
            return Err("no path".into());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let ron = ron::ser::to_string_pretty(
            &buf.sm,
            ron::ser::PrettyConfig::new().indentor("  ".into()),
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(&path, ron).map_err(|e| e.to_string())?;
        drop(buf);
        self.buffer.lock().unwrap().dirty = false;
        Ok(())
    }
}

impl Default for StateMachinePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorPanel for StateMachinePanel {
    fn id(&self) -> &str {
        "animator_state_machine"
    }

    fn title(&self) -> &str {
        "State Machine"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::GRAPH)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }

    fn min_size(&self) -> [f32; 2] {
        [420.0, 260.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>();
        let (text, muted, accent, surface, border, row_even, row_odd) = if let Some(tm) = theme {
            let t = &tm.active_theme;
            (
                t.text.primary.to_color32(),
                t.text.muted.to_color32(),
                t.semantic.accent.to_color32(),
                t.surfaces.faint.to_color32(),
                t.widgets.border.to_color32(),
                t.panels.inspector_row_even.to_color32(),
                t.panels.inspector_row_odd.to_color32(),
            )
        } else {
            (
                egui::Color32::WHITE,
                egui::Color32::GRAY,
                egui::Color32::from_rgb(100, 149, 237),
                egui::Color32::from_rgb(30, 31, 36),
                egui::Color32::from_rgb(50, 51, 56),
                egui::Color32::from_rgb(30, 31, 36),
                egui::Color32::from_rgb(34, 35, 40),
            )
        };

        let Some(editor_state) = world.get_resource::<AnimationEditorState>() else {
            return;
        };
        let Some(entity) = editor_state.selected_entity else {
            self.empty(ui, muted, "Select an entity with an animator");
            return;
        };
        let Some(animator) = world.get::<AnimatorComponent>(entity) else {
            self.empty(ui, muted, "No AnimatorComponent on selected entity");
            return;
        };

        let clip_names: Vec<String> =
            animator.clips.iter().map(|c| c.name.clone()).collect();

        // ── Header: .animsm path + Load/Save ──
        let sm_path = animator.state_machine.clone();
        let project_root = world
            .get_resource::<renzora::CurrentProject>()
            .map(|p| p.path.clone());

        egui::Frame::new()
            .fill(surface)
            .stroke(egui::Stroke::new(0.5, border))
            .inner_margin(egui::Margin::same(6))
            .corner_radius(3.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::FILE)
                            .size(12.0)
                            .color(muted),
                    );
                    match (&sm_path, &project_root) {
                        (Some(rel), Some(_)) => {
                            ui.label(
                                egui::RichText::new(rel).size(11.0).color(text),
                            );
                        }
                        _ => {
                            ui.label(
                                egui::RichText::new("No .animsm assigned on animator")
                                    .size(11.0)
                                    .color(muted),
                            );
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let dirty = self.buffer.lock().unwrap().dirty;
                        let save_color = if dirty { accent } else { muted };
                        if ui
                            .button(
                                egui::RichText::new(format!(
                                    "{} Save",
                                    egui_phosphor::regular::FLOPPY_DISK
                                ))
                                .size(11.0)
                                .color(save_color),
                            )
                            .clicked()
                        {
                            if let Err(e) = self.save() {
                                self.buffer.lock().unwrap().error = Some(e);
                            }
                        }
                    });
                });
                if let Some(err) = self.buffer.lock().unwrap().error.clone() {
                    ui.colored_label(egui::Color32::from_rgb(220, 100, 100), err);
                }
            });

        // If an .animsm path exists, load it (idempotent).
        if let (Some(rel), Some(root)) = (&sm_path, &project_root) {
            self.ensure_loaded(root, rel);
        } else {
            // No path on animator: show empty prompt.
            self.empty(ui, muted, "Assign a state_machine path on the AnimatorComponent to edit.");
            return;
        }

        ui.add_space(6.0);

        // Split: states (left) | transitions (right)
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            self.states_column(ui, text, muted, accent, surface, border, row_even, row_odd, &clip_names, entity, world);

            let ui = &mut cols[1];
            self.transitions_column(ui, text, muted, accent, surface, border, row_even, row_odd);
        });
    }
}

impl StateMachinePanel {
    fn empty(&self, ui: &mut egui::Ui, color: egui::Color32, msg: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::GRAPH)
                    .size(24.0)
                    .color(color),
            );
            ui.add_space(4.0);
            ui.label(egui::RichText::new(msg).size(12.0).color(color));
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn states_column(
        &self,
        ui: &mut egui::Ui,
        text: egui::Color32,
        muted: egui::Color32,
        accent: egui::Color32,
        _surface: egui::Color32,
        _border: egui::Color32,
        row_even: egui::Color32,
        row_odd: egui::Color32,
        clip_names: &[String],
        entity: Entity,
        world: &World,
    ) {
        let _ = (entity, world);
        ui.heading(egui::RichText::new("States").size(13.0).color(text));
        ui.add_space(2.0);

        // Controls row
        ui.horizontal(|ui| {
            if ui
                .button(
                    egui::RichText::new(format!("{} Add State", egui_phosphor::regular::PLUS))
                        .size(11.0)
                        .color(accent),
                )
                .clicked()
            {
                let mut buf = self.buffer.lock().unwrap();
                let name = unique_name("NewState", |n| buf.sm.states.iter().any(|s| s.name == *n));
                buf.sm.states.push(AnimState {
                    name,
                    motion: StateMotion::Clip(
                        clip_names.first().cloned().unwrap_or_default(),
                    ),
                    speed: 1.0,
                    looping: true,
                });
                buf.dirty = true;
            }
        });
        ui.add_space(2.0);

        egui::ScrollArea::vertical().id_salt("sm_states").show(ui, |ui| {
            let mut buf = self.buffer.lock().unwrap();
            let default_state = buf.sm.default_state.clone();
            let mut remove_idx: Option<usize> = None;
            let mut make_default: Option<String> = None;

            for (i, state) in buf.sm.states.iter_mut().enumerate() {
                let bg = if i % 2 == 0 { row_even } else { row_odd };
                egui::Frame::new()
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .corner_radius(2.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let is_default = default_state == state.name;
                            let star_color = if is_default { accent } else { muted };
                            if ui
                                .button(
                                    egui::RichText::new(egui_phosphor::regular::STAR)
                                        .size(11.0)
                                        .color(star_color),
                                )
                                .on_hover_text("Set as entry state")
                                .clicked()
                            {
                                make_default = Some(state.name.clone());
                            }
                            // Name
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut state.name)
                                        .desired_width(100.0),
                                )
                                .changed()
                            {
                                // noop — mutation flags dirty below
                            }
                            // Clip picker
                            let mut clip_name = match &state.motion {
                                StateMotion::Clip(n) => n.clone(),
                                StateMotion::BlendTree(n) => n.clone(),
                            };
                            let before = clip_name.clone();
                            egui::ComboBox::from_id_salt(egui::Id::new(("sm_clip", i)))
                                .selected_text(
                                    egui::RichText::new(&clip_name).size(10.0).color(text),
                                )
                                .width(110.0)
                                .show_ui(ui, |ui| {
                                    for name in clip_names {
                                        ui.selectable_value(
                                            &mut clip_name,
                                            name.clone(),
                                            name,
                                        );
                                    }
                                });
                            if clip_name != before {
                                state.motion = StateMotion::Clip(clip_name);
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(
                                            egui::RichText::new(egui_phosphor::regular::TRASH)
                                                .size(10.0)
                                                .color(muted),
                                        )
                                        .clicked()
                                    {
                                        remove_idx = Some(i);
                                    }
                                },
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("speed").size(10.0).color(muted));
                            ui.add(
                                egui::DragValue::new(&mut state.speed)
                                    .speed(0.05)
                                    .range(0.05..=5.0)
                                    .fixed_decimals(2),
                            );
                            ui.checkbox(&mut state.looping, "loop");
                        });
                    });
            }

            if let Some(i) = remove_idx {
                let removed_name = buf.sm.states[i].name.clone();
                buf.sm.states.remove(i);
                buf.sm.transitions.retain(|t| t.from != removed_name && t.to != removed_name);
                if buf.sm.default_state == removed_name {
                    buf.sm.default_state =
                        buf.sm.states.first().map(|s| s.name.clone()).unwrap_or_default();
                }
                buf.dirty = true;
            }
            if let Some(name) = make_default {
                buf.sm.default_state = name;
                buf.dirty = true;
            }
            // Any mutation above counts as dirty.
            buf.dirty = true;
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn transitions_column(
        &self,
        ui: &mut egui::Ui,
        text: egui::Color32,
        muted: egui::Color32,
        accent: egui::Color32,
        _surface: egui::Color32,
        _border: egui::Color32,
        row_even: egui::Color32,
        row_odd: egui::Color32,
    ) {
        ui.heading(egui::RichText::new("Transitions").size(13.0).color(text));
        ui.add_space(2.0);

        // Snapshot state names for dropdowns (so we don't hold the lock while
        // rendering dropdowns that can call back into it).
        let state_names: Vec<String> = {
            self.buffer
                .lock()
                .unwrap()
                .sm
                .states
                .iter()
                .map(|s| s.name.clone())
                .collect()
        };

        ui.horizontal(|ui| {
            let can_add = state_names.len() >= 1;
            let btn = egui::Button::new(
                egui::RichText::new(format!("{} Add Transition", egui_phosphor::regular::PLUS))
                    .size(11.0)
                    .color(if can_add { accent } else { muted }),
            );
            if ui.add_enabled(can_add, btn).clicked() {
                let mut buf = self.buffer.lock().unwrap();
                let from = state_names[0].clone();
                let to = state_names.get(1).cloned().unwrap_or_else(|| from.clone());
                buf.sm.transitions.push(AnimTransition {
                    from,
                    to,
                    condition: AnimCondition::Always,
                    blend_duration: 0.2,
                });
                buf.dirty = true;
            }
        });
        ui.add_space(2.0);

        egui::ScrollArea::vertical().id_salt("sm_transitions").show(ui, |ui| {
            let mut buf = self.buffer.lock().unwrap();
            let mut remove_idx: Option<usize> = None;

            for (i, tr) in buf.sm.transitions.iter_mut().enumerate() {
                let bg = if i % 2 == 0 { row_even } else { row_odd };
                egui::Frame::new()
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .corner_radius(2.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(egui::Id::new(("sm_from", i)))
                                .selected_text(
                                    egui::RichText::new(&tr.from).size(10.0).color(text),
                                )
                                .width(90.0)
                                .show_ui(ui, |ui| {
                                    for n in &state_names {
                                        ui.selectable_value(&mut tr.from, n.clone(), n);
                                    }
                                });
                            ui.label(
                                egui::RichText::new(egui_phosphor::regular::ARROW_RIGHT)
                                    .size(11.0)
                                    .color(muted),
                            );
                            egui::ComboBox::from_id_salt(egui::Id::new(("sm_to", i)))
                                .selected_text(
                                    egui::RichText::new(&tr.to).size(10.0).color(text),
                                )
                                .width(90.0)
                                .show_ui(ui, |ui| {
                                    for n in &state_names {
                                        ui.selectable_value(&mut tr.to, n.clone(), n);
                                    }
                                });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(
                                            egui::RichText::new(egui_phosphor::regular::TRASH)
                                                .size(10.0)
                                                .color(muted),
                                        )
                                        .clicked()
                                    {
                                        remove_idx = Some(i);
                                    }
                                },
                            );
                        });

                        // Blend duration
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("blend").size(10.0).color(muted));
                            ui.add(
                                egui::DragValue::new(&mut tr.blend_duration)
                                    .speed(0.02)
                                    .range(0.0..=2.0)
                                    .fixed_decimals(2)
                                    .suffix("s"),
                            );
                        });

                        // Condition editor
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("when").size(10.0).color(muted));
                            render_condition_editor(ui, &mut tr.condition, text);
                        });
                    });
            }

            if let Some(i) = remove_idx {
                buf.sm.transitions.remove(i);
                buf.dirty = true;
            }
            buf.dirty = true;
        });
    }
}

fn render_condition_editor(
    ui: &mut egui::Ui,
    condition: &mut AnimCondition,
    text: egui::Color32,
) {
    let current_kind = match condition {
        AnimCondition::FloatGreater(_, _) => "Float >",
        AnimCondition::FloatLess(_, _) => "Float <",
        AnimCondition::BoolTrue(_) => "Bool true",
        AnimCondition::BoolFalse(_) => "Bool false",
        AnimCondition::Trigger(_) => "Trigger",
        AnimCondition::TimeElapsed(_) => "Time >=",
        AnimCondition::Always => "Always",
    };

    let before = current_kind.to_string();
    let mut selected = current_kind.to_string();
    egui::ComboBox::from_id_salt(ui.id().with("sm_cond_kind"))
        .selected_text(egui::RichText::new(&selected).size(10.0).color(text))
        .width(90.0)
        .show_ui(ui, |ui| {
            for k in [
                "Float >",
                "Float <",
                "Bool true",
                "Bool false",
                "Trigger",
                "Time >=",
                "Always",
            ] {
                ui.selectable_value(&mut selected, k.to_string(), k);
            }
        });

    if selected != before {
        *condition = match selected.as_str() {
            "Float >" => AnimCondition::FloatGreater(String::new(), 0.0),
            "Float <" => AnimCondition::FloatLess(String::new(), 0.0),
            "Bool true" => AnimCondition::BoolTrue(String::new()),
            "Bool false" => AnimCondition::BoolFalse(String::new()),
            "Trigger" => AnimCondition::Trigger(String::new()),
            "Time >=" => AnimCondition::TimeElapsed(0.0),
            "Always" => AnimCondition::Always,
            _ => AnimCondition::Always,
        };
    }

    match condition {
        AnimCondition::FloatGreater(name, v) | AnimCondition::FloatLess(name, v) => {
            ui.add(
                egui::TextEdit::singleline(name)
                    .hint_text("param")
                    .desired_width(70.0),
            );
            ui.add(egui::DragValue::new(v).speed(0.05).fixed_decimals(2));
        }
        AnimCondition::BoolTrue(name)
        | AnimCondition::BoolFalse(name)
        | AnimCondition::Trigger(name) => {
            ui.add(
                egui::TextEdit::singleline(name)
                    .hint_text("param")
                    .desired_width(90.0),
            );
        }
        AnimCondition::TimeElapsed(t) => {
            ui.add(
                egui::DragValue::new(t)
                    .speed(0.05)
                    .range(0.0..=60.0)
                    .fixed_decimals(2)
                    .suffix("s"),
            );
        }
        AnimCondition::Always => {}
    }
}

fn unique_name(base: &str, collides: impl Fn(&String) -> bool) -> String {
    let mut candidate = base.to_string();
    let mut n = 1;
    while collides(&candidate) {
        n += 1;
        candidate = format!("{base}{n}");
    }
    candidate
}
