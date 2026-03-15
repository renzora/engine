//! Timeline panel — per-clip collapsible sections with transport and keyframe display.
//!
//! Panel id: `"timeline"` (matches layout_animation() in layouts.rs).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::{AnimatorComponent, AnimatorState};
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::AnimEditorAction;
use crate::AnimationEditorState;

pub struct TimelinePanel {
    bridge: Arc<Mutex<Vec<AnimEditorAction>>>,
}

impl TimelinePanel {
    pub fn new(bridge: Arc<Mutex<Vec<AnimEditorAction>>>) -> Self {
        Self { bridge }
    }

    fn push_action(&self, action: AnimEditorAction) {
        if let Ok(mut pending) = self.bridge.lock() {
            pending.push(action);
        }
    }
}

impl EditorPanel for TimelinePanel {
    fn id(&self) -> &str {
        "timeline"
    }

    fn title(&self) -> &str {
        "Timeline"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::TIMER)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let (text_color, muted_color, accent_color, surface_color, border_color) =
            if let Some(tm) = world.get_resource::<ThemeManager>() {
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

        let editor_state = world.get_resource::<AnimationEditorState>();
        let (scrub_time, is_previewing, preview_speed, preview_looping, selected_entity, selected_clip) =
            if let Some(s) = editor_state {
                (s.scrub_time, s.is_previewing, s.preview_speed, s.preview_looping, s.selected_entity, s.selected_clip.clone())
            } else {
                (0.0, false, 1.0, true, None, None)
            };

        // ── Transport bar ──
        ui.horizontal(|ui| {
            let play_icon = if is_previewing {
                egui_phosphor::regular::PAUSE
            } else {
                egui_phosphor::regular::PLAY
            };
            if ui.button(egui::RichText::new(play_icon).color(text_color)).clicked() {
                self.push_action(AnimEditorAction::TogglePreview);
            }

            if ui.button(egui::RichText::new(egui_phosphor::regular::STOP).color(text_color)).clicked() {
                self.push_action(AnimEditorAction::StopPreview);
            }

            let loop_color = if preview_looping { accent_color } else { muted_color };
            if ui.button(egui::RichText::new(egui_phosphor::regular::REPEAT).color(loop_color)).clicked() {
                self.push_action(AnimEditorAction::SetPreviewLooping(!preview_looping));
            }

            ui.separator();

            ui.label(egui::RichText::new("Speed:").size(11.0).color(muted_color));
            for &speed in &[0.25f32, 0.5, 1.0, 2.0] {
                let label = format!("{:.2}x", speed);
                let color = if (preview_speed - speed).abs() < 0.01 { accent_color } else { text_color };
                if ui.button(egui::RichText::new(&label).size(11.0).color(color)).clicked() {
                    self.push_action(AnimEditorAction::SetPreviewSpeed(speed));
                }
            }

            ui.separator();

            let mins = (scrub_time / 60.0) as u32;
            let secs = scrub_time % 60.0;
            ui.label(egui::RichText::new(format!("{:02}:{:05.2}", mins, secs)).color(text_color).monospace());
        });

        ui.separator();

        // Get clips from entity
        let Some(entity) = selected_entity else {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("Select an entity with animations").size(12.0).color(muted_color));
            });
            return;
        };

        let Some(animator) = world.get::<AnimatorComponent>(entity) else {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No AnimatorComponent").size(12.0).color(muted_color));
            });
            return;
        };

        let state = world.get::<AnimatorState>(entity);
        let current_clip_name = state.and_then(|s| s.current_clip.clone());

        if animator.clips.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No clips assigned").size(12.0).color(muted_color));
            });
            return;
        }

        // ── Per-clip collapsible sections ──
        egui::ScrollArea::vertical().show(ui, |ui| {
            for slot in &animator.clips {
                let is_playing = current_clip_name.as_deref() == Some(&slot.name);
                let is_selected = selected_clip.as_deref() == Some(&slot.name);

                let header_bg = if is_selected {
                    egui::Color32::from_rgba_premultiplied(accent_color.r(), accent_color.g(), accent_color.b(), 30)
                } else {
                    surface_color
                };

                let header_frame = egui::Frame::new()
                    .fill(header_bg)
                    .stroke(egui::Stroke::new(0.5, border_color))
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::same(4));

                header_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // Header row — clickable to select + play
                    let header_response = ui.horizontal(|ui| {
                        // Playing indicator
                        if is_playing {
                            ui.label(egui::RichText::new(egui_phosphor::regular::PLAY_CIRCLE)
                                .size(12.0).color(accent_color));
                        } else {
                            ui.label(egui::RichText::new(egui_phosphor::regular::CIRCLE)
                                .size(12.0).color(muted_color));
                        }

                        // Clip name
                        let name_color = if is_playing { accent_color } else { text_color };
                        ui.label(egui::RichText::new(&slot.name).size(12.0).color(name_color).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Duration
                            // We don't have the .anim file loaded here to get duration,
                            // so show speed + looping info
                            let loop_icon = if slot.looping {
                                egui_phosphor::regular::REPEAT
                            } else {
                                egui_phosphor::regular::ARROW_RIGHT
                            };
                            ui.label(egui::RichText::new(loop_icon).size(10.0).color(muted_color));
                            ui.label(egui::RichText::new(format!("{:.1}x", slot.speed))
                                .size(10.0).color(muted_color));
                        });
                    }).response;

                    // Click to select and play this clip
                    if header_response.clicked() {
                        self.push_action(AnimEditorAction::SelectClip(Some(slot.name.clone())));

                        // Play this clip via EditorCommands
                        let clip_name = slot.name.clone();
                        let looping = slot.looping;
                        let speed = slot.speed;
                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                            cmds.push(move |world: &mut World| {
                                if let Some(mut queue) = world.get_resource_mut::<renzora_animation::AnimationCommandQueue>() {
                                    queue.commands.push(renzora_animation::systems::AnimationCommand::Play {
                                        entity, name: clip_name, looping, speed,
                                    });
                                }
                            });
                        }
                    }

                    // Show bone tracks when selected
                    if is_selected {
                        ui.add_space(2.0);
                        ui.separator();

                        // Read the .anim file to get bone track info
                        // We can read from the clip_handles in AnimatorState
                        let mut bone_info: Vec<(String, bool, bool, bool)> = Vec::new(); // (name, has_t, has_r, has_s)

                        if let Some(anim_state) = state {
                            if let Some(handle) = anim_state.clip_handles.get(&slot.name) {
                                if let Some(clip_assets) = world.get_resource::<Assets<AnimationClip>>() {
                                    if let Some(_clip) = clip_assets.get(handle) {
                                        // Bevy's AnimationClip doesn't expose track names directly,
                                        // so we show the clip is loaded
                                        // For now show a placeholder — real bone info comes from .anim file
                                    }
                                }
                            }
                        }

                        // If we couldn't get bone info from the asset, show clip path
                        if bone_info.is_empty() {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(egui_phosphor::regular::FILE)
                                    .size(10.0).color(muted_color));
                                ui.label(egui::RichText::new(&slot.path)
                                    .size(10.0).color(muted_color));
                            });

                            // Parse bone names from the .anim file if available on disk
                            if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
                                let anim_path = project.path.join("assets").join(&slot.path);
                                if let Ok(content) = std::fs::read_to_string(&anim_path) {
                                    // Quick parse: extract bone_name fields
                                    for line in content.lines() {
                                        let trimmed = line.trim();
                                        if let Some(name) = trimmed.strip_prefix("bone_name: \"") {
                                            if let Some(name) = name.strip_suffix("\",") {
                                                let has_t = true; // simplified — all tracks present
                                                let has_r = true;
                                                let has_s = true;
                                                bone_info.push((name.to_string(), has_t, has_r, has_s));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if !bone_info.is_empty() {
                            ui.label(egui::RichText::new(format!("{} bone tracks", bone_info.len()))
                                .size(10.0).color(muted_color));
                            ui.add_space(2.0);

                            // Show bone tracks in a compact list
                            let max_visible = 8;
                            for (i, (bone_name, has_t, has_r, has_s)) in bone_info.iter().enumerate() {
                                if i >= max_visible {
                                    ui.label(egui::RichText::new(
                                        format!("  ... and {} more", bone_info.len() - max_visible))
                                        .size(10.0).color(muted_color));
                                    break;
                                }

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(egui_phosphor::regular::BONE)
                                        .size(10.0).color(muted_color));
                                    ui.label(egui::RichText::new(bone_name)
                                        .size(10.0).color(text_color));

                                    // Channel indicators
                                    let t_color = if *has_t { accent_color } else { muted_color };
                                    let r_color = if *has_r { egui::Color32::from_rgb(120, 200, 120) } else { muted_color };
                                    let s_color = if *has_s { egui::Color32::from_rgb(200, 120, 120) } else { muted_color };

                                    ui.label(egui::RichText::new("T").size(9.0).color(t_color));
                                    ui.label(egui::RichText::new("R").size(9.0).color(r_color));
                                    ui.label(egui::RichText::new("S").size(9.0).color(s_color));
                                });
                            }
                        }
                    }
                });

                ui.add_space(2.0);
            }
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn min_size(&self) -> [f32; 2] {
        [200.0, 100.0]
    }
}
