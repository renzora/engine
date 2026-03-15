//! Animation panel — clip library, state machine view, parameters, layer stack.
//!
//! Panel id: `"animation"` (matches layout_animation() in layouts.rs).

use std::sync::{Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::{AnimatorComponent, AnimatorState};
use renzora_editor::{EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;
use renzora_animation::layers::AnimationLayer;

use crate::AnimEditorAction;
use crate::AnimationEditorState;

/// Snapshot of the currently-selected entity's animation data.
#[derive(Default, Clone)]
struct AnimSnapshot {
    clips: Vec<(String, String, bool, f32)>, // (name, path, looping, speed)
    default_clip: Option<String>,
    selected_clip: Option<String>,
    current_state: Option<String>,
    params_float: Vec<(String, f32)>,
    params_bool: Vec<(String, bool)>,
    triggers: Vec<String>,
    layers: Vec<AnimationLayer>,
    has_state_machine: bool,
    state_names: Vec<String>,
}

pub struct AnimationPanel {
    bridge: Arc<Mutex<Vec<AnimEditorAction>>>,
    local: RwLock<AnimSnapshot>,
}

impl AnimationPanel {
    pub fn new(bridge: Arc<Mutex<Vec<AnimEditorAction>>>) -> Self {
        Self {
            bridge,
            local: RwLock::new(AnimSnapshot::default()),
        }
    }

    fn push_action(&self, action: AnimEditorAction) {
        if let Ok(mut pending) = self.bridge.lock() {
            pending.push(action);
        }
    }
}

impl EditorPanel for AnimationPanel {
    fn id(&self) -> &str {
        "animation"
    }

    fn title(&self) -> &str {
        "Animation"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::FILM_STRIP)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let (text_color, muted_color, accent_color) =
            if let Some(tm) = world.get_resource::<ThemeManager>() {
                let t = &tm.active_theme;
                (
                    t.text.primary.to_color32(),
                    t.text.muted.to_color32(),
                    t.semantic.accent.to_color32(),
                )
            } else {
                (
                    egui::Color32::WHITE,
                    egui::Color32::GRAY,
                    egui::Color32::from_rgb(100, 149, 237),
                )
            };

        // Read state from world
        let editor_state = world.get_resource::<AnimationEditorState>();
        let selected_entity = editor_state.and_then(|s| s.selected_entity);
        let selected_clip = editor_state.and_then(|s| s.selected_clip.clone());

        // Build snapshot from the selected entity's AnimatorComponent
        let mut snap = AnimSnapshot::default();
        snap.selected_clip = selected_clip;

        if let Some(entity) = selected_entity {
            if let Some(animator) = world.get::<AnimatorComponent>(entity) {
                snap.default_clip = animator.default_clip.clone();
                for slot in &animator.clips {
                    snap.clips.push((
                        slot.name.clone(),
                        slot.path.clone(),
                        slot.looping,
                        slot.speed,
                    ));
                }
                snap.has_state_machine = animator.state_machine.is_some();
                snap.layers = animator.layers.clone();
            }
            if let Some(state) = world.get::<AnimatorState>(entity) {
                snap.current_state = state.current_state.clone();
                snap.params_float = state.params.floats.iter().map(|(k, v)| (k.clone(), *v)).collect();
                snap.params_bool = state.params.bools.iter().map(|(k, v)| (k.clone(), *v)).collect();
                snap.triggers = state.params.triggers.keys().cloned().collect();
            }
        }

        if let Ok(mut local) = self.local.write() {
            *local = snap;
        }

        let snap = self.local.read().unwrap();

        if selected_entity.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("Select an entity with AnimatorComponent").color(muted_color));
            });
            return;
        }

        if snap.clips.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No animation clips").color(muted_color));
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            // ── Clip Library ──
            ui.label(egui::RichText::new("Clips").color(text_color).strong());
            ui.separator();

            for (name, path, looping, speed) in &snap.clips {
                let is_selected = snap.selected_clip.as_deref() == Some(name.as_str());
                let is_default = snap.default_clip.as_deref() == Some(name.as_str());

                let label = if is_default {
                    format!("{} (default)", name)
                } else {
                    name.clone()
                };

                let response = ui.selectable_label(
                    is_selected,
                    egui::RichText::new(&label).color(if is_selected { accent_color } else { text_color }),
                );

                if response.clicked() {
                    self.push_action(AnimEditorAction::SelectClip(Some(name.clone())));
                }

                response.on_hover_text(format!("{}\nLoop: {} | Speed: {:.1}", path, looping, speed));
            }

            ui.add_space(8.0);

            // ── State Machine ──
            if snap.has_state_machine {
                ui.label(egui::RichText::new("State Machine").color(text_color).strong());
                ui.separator();

                if let Some(ref current) = snap.current_state {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Current:").color(muted_color));
                        ui.label(egui::RichText::new(current).color(accent_color));
                    });
                }

                ui.add_space(4.0);

                // ── Parameters ──
                if !snap.params_float.is_empty() || !snap.params_bool.is_empty() {
                    ui.label(egui::RichText::new("Parameters").color(text_color).strong());
                    ui.separator();

                    for (name, value) in &snap.params_float {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(name).color(muted_color));
                            ui.label(egui::RichText::new(format!("{:.2}", value)).color(text_color));
                        });
                    }

                    for (name, value) in &snap.params_bool {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(name).color(muted_color));
                            ui.label(egui::RichText::new(if *value { "true" } else { "false" }).color(text_color));
                        });
                    }
                }

                ui.add_space(4.0);
            }

            // ── Layers ──
            if !snap.layers.is_empty() {
                ui.label(egui::RichText::new("Layers").color(text_color).strong());
                ui.separator();

                for layer in &snap.layers {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&layer.name).color(text_color));
                        ui.label(egui::RichText::new(format!("{:.0}%", layer.weight * 100.0)).color(muted_color));
                        if let Some(ref clip) = layer.current_clip {
                            ui.label(egui::RichText::new(format!("({})", clip)).color(muted_color));
                        }
                    });
                }
            }
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }

    fn min_size(&self) -> [f32; 2] {
        [180.0, 120.0]
    }
}
