//! Animation Properties panel — clip properties, state machine info, parameters,
//! layers, and bone list using the renzora_ui section/collapsible design.
//!
//! Panel id: `"animation"` (matches layout_animation() in layouts.rs).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::{AnimClip, AnimatorComponent, AnimatorState};
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;
use renzora_ui::widgets::{collapsible_section, inline_property};

use crate::AnimEditorAction;
use crate::AnimationEditorState;

pub struct AnimationPanel {
    bridge: Arc<Mutex<Vec<AnimEditorAction>>>,
}

impl AnimationPanel {
    pub fn new(bridge: Arc<Mutex<Vec<AnimEditorAction>>>) -> Self {
        Self { bridge }
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
        "Properties"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::SLIDERS_HORIZONTAL)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>();
        let (text_color, muted_color, accent_color) =
            if let Some(tm) = theme {
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

        let theme_ref = theme.map(|tm| &tm.active_theme);

        let editor_state = world.get_resource::<AnimationEditorState>();
        let selected_entity = editor_state.and_then(|s| s.selected_entity);
        let selected_clip = editor_state.and_then(|s| s.selected_clip.clone());

        // ── Empty states ──
        let Some(entity) = selected_entity else {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(egui::RichText::new(egui_phosphor::regular::FILM_STRIP)
                    .size(24.0).color(muted_color));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Select an entity with")
                    .size(12.0).color(muted_color));
                ui.label(egui::RichText::new("AnimatorComponent")
                    .size(12.0).color(muted_color));
            });
            return;
        };

        let Some(animator) = world.get::<AnimatorComponent>(entity) else {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(egui::RichText::new("No AnimatorComponent").size(12.0).color(muted_color));
            });
            return;
        };

        let state = world.get::<AnimatorState>(entity);

        if animator.clips.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.3);
                ui.label(egui::RichText::new("No animation clips").size(12.0).color(muted_color));
            });
            return;
        }

        // Load clip data if a clip is selected
        let clip_data = selected_clip.as_deref().and_then(|name| {
            let slot = animator.clips.iter().find(|s| s.name == name)?;
            let project = world.get_resource::<renzora_core::CurrentProject>()?;
            let anim_path = project.path.join(&slot.path);
            let content = std::fs::read_to_string(&anim_path).ok()?;
            ron::from_str::<AnimClip>(&content).ok()
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(ref t) = theme_ref {
                // ── Clip Properties ──
                if let Some(ref clip_name) = selected_clip {
                    if let Some(slot) = animator.clips.iter().find(|s| &s.name == clip_name) {
                        collapsible_section(
                            ui,
                            egui_phosphor::regular::FILM_STRIP,
                            "Clip Properties",
                            "rendering",
                            t,
                            "anim_clip_props",
                            true,
                            |ui| {
                                let mut row = 0;

                                inline_property(ui, row, "Name", t, |ui| {
                                    ui.label(egui::RichText::new(&slot.name)
                                        .size(11.0).color(text_color));
                                });
                                row += 1;

                                inline_property(ui, row, "Path", t, |ui| {
                                    ui.add(egui::Label::new(
                                        egui::RichText::new(&slot.path)
                                            .size(11.0).color(muted_color)
                                    ).truncate());
                                });
                                row += 1;

                                inline_property(ui, row, "Speed", t, |ui| {
                                    ui.label(egui::RichText::new(format!("{:.2}x", slot.speed))
                                        .size(11.0).color(text_color));
                                });
                                row += 1;

                                inline_property(ui, row, "Looping", t, |ui| {
                                    let icon = if slot.looping {
                                        egui_phosphor::regular::CHECK_CIRCLE
                                    } else {
                                        egui_phosphor::regular::CIRCLE
                                    };
                                    let color = if slot.looping { accent_color } else { muted_color };
                                    ui.label(egui::RichText::new(icon).size(12.0).color(color));
                                });
                                row += 1;

                                if let Some(ref clip) = clip_data {
                                    inline_property(ui, row, "Duration", t, |ui| {
                                        ui.label(egui::RichText::new(format!("{:.2}s", clip.duration))
                                            .size(11.0).color(text_color));
                                    });
                                    row += 1;

                                    inline_property(ui, row, "Tracks", t, |ui| {
                                        ui.label(egui::RichText::new(format!("{} bones", clip.tracks.len()))
                                            .size(11.0).color(text_color));
                                    });
                                    row += 1;

                                    // Count total keyframes
                                    let total_kf: usize = clip.tracks.iter().map(|t| {
                                        t.translations.len() + t.rotations.len() + t.scales.len()
                                    }).sum();
                                    inline_property(ui, row, "Keyframes", t, |ui| {
                                        ui.label(egui::RichText::new(format!("{}", total_kf))
                                            .size(11.0).color(text_color));
                                    });
                                }
                            },
                        );

                        ui.add_space(2.0);
                    }
                }

                // ── Clip Library ──
                collapsible_section(
                    ui,
                    egui_phosphor::regular::LIST_BULLETS,
                    "Clip Library",
                    "rendering",
                    t,
                    "anim_clip_library",
                    true,
                    |ui| {
                        let current_clip_name = state.and_then(|s| s.current_clip.as_deref());

                        for slot in &animator.clips {
                            let is_selected = selected_clip.as_deref() == Some(&slot.name);
                            let is_playing = current_clip_name == Some(&slot.name);
                            let is_default = animator.default_clip.as_deref() == Some(&slot.name);

                            let bg = if is_selected {
                                egui::Color32::from_rgba_premultiplied(
                                    accent_color.r(), accent_color.g(), accent_color.b(), 25
                                )
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            egui::Frame::new()
                                .fill(bg)
                                .corner_radius(3.0)
                                .inner_margin(egui::Margin::symmetric(6, 3))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    let resp = ui.horizontal(|ui| {
                                        // Status icon
                                        if is_playing {
                                            ui.label(egui::RichText::new(egui_phosphor::regular::PLAY_CIRCLE)
                                                .size(12.0).color(accent_color));
                                        } else {
                                            ui.label(egui::RichText::new(egui_phosphor::regular::CIRCLE)
                                                .size(12.0).color(muted_color));
                                        }

                                        // Name
                                        let name_color = if is_selected || is_playing { accent_color } else { text_color };
                                        ui.label(egui::RichText::new(&slot.name)
                                            .size(11.0).color(name_color).strong());

                                        if is_default {
                                            ui.label(egui::RichText::new("(default)")
                                                .size(9.0).color(muted_color));
                                        }

                                        // Right side info
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let loop_icon = if slot.looping {
                                                egui_phosphor::regular::REPEAT
                                            } else {
                                                egui_phosphor::regular::ARROW_RIGHT
                                            };
                                            ui.label(egui::RichText::new(loop_icon)
                                                .size(10.0).color(muted_color));
                                            ui.label(egui::RichText::new(format!("{:.1}x", slot.speed))
                                                .size(10.0).color(muted_color));
                                        });
                                    }).response;

                                    if resp.clicked() {
                                        self.push_action(AnimEditorAction::SelectClip(Some(slot.name.clone())));

                                        // Play on select
                                        let clip_name = slot.name.clone();
                                        let looping = slot.looping;
                                        let speed = slot.speed;
                                        if let Some(cmds) = world.get_resource::<EditorCommands>() {
                                            cmds.push(move |world: &mut World| {
                                                if let Some(mut queue) = world.get_resource_mut::<renzora_animation::AnimationCommandQueue>() {
                                                    queue.commands.push(renzora_animation::AnimationCommand::Play {
                                                        entity, name: clip_name, looping, speed,
                                                    });
                                                }
                                            });
                                        }
                                    }
                                });
                        }
                    },
                );

                ui.add_space(2.0);

                // ── Bone Tracks ──
                if let Some(ref clip) = clip_data {
                    collapsible_section(
                        ui,
                        egui_phosphor::regular::BONE,
                        &format!("Bone Tracks ({})", clip.tracks.len()),
                        "transform",
                        t,
                        "anim_bone_tracks",
                        false, // collapsed by default
                        |ui| {
                            for (i, track) in clip.tracks.iter().enumerate() {
                                let has_t = !track.translations.is_empty();
                                let has_r = !track.rotations.is_empty();
                                let has_s = !track.scales.is_empty();

                                let row = i;
                                inline_property(ui, row, &track.bone_name, t, |ui| {
                                    let t_color = if has_t {
                                        egui::Color32::from_rgb(100, 149, 237)
                                    } else { muted_color };
                                    let r_color = if has_r {
                                        egui::Color32::from_rgb(120, 200, 120)
                                    } else { muted_color };
                                    let s_color = if has_s {
                                        egui::Color32::from_rgb(200, 120, 120)
                                    } else { muted_color };

                                    ui.label(egui::RichText::new("T").size(9.0).color(t_color));
                                    ui.label(egui::RichText::new(
                                        format!("{}", track.translations.len())
                                    ).size(9.0).color(t_color));
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("R").size(9.0).color(r_color));
                                    ui.label(egui::RichText::new(
                                        format!("{}", track.rotations.len())
                                    ).size(9.0).color(r_color));
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("S").size(9.0).color(s_color));
                                    ui.label(egui::RichText::new(
                                        format!("{}", track.scales.len())
                                    ).size(9.0).color(s_color));
                                });
                            }
                        },
                    );

                    ui.add_space(2.0);
                }

                // ── State Machine ──
                if animator.state_machine.is_some() {
                    collapsible_section(
                        ui,
                        egui_phosphor::regular::GRAPH,
                        "State Machine",
                        "scripting",
                        t,
                        "anim_state_machine",
                        true,
                        |ui| {
                            let mut row = 0;

                            if let Some(ref sm_path) = animator.state_machine {
                                inline_property(ui, row, "File", t, |ui| {
                                    ui.add(egui::Label::new(
                                        egui::RichText::new(sm_path)
                                            .size(11.0).color(muted_color)
                                    ).truncate());
                                });
                                row += 1;
                            }

                            if let Some(anim_state) = state {
                                if let Some(ref current) = anim_state.current_state {
                                    inline_property(ui, row, "State", t, |ui| {
                                        ui.label(egui::RichText::new(current)
                                            .size(11.0).color(accent_color).strong());
                                    });
                                    row += 1;
                                }

                                inline_property(ui, row, "Time", t, |ui| {
                                    ui.label(egui::RichText::new(format!("{:.2}s", anim_state.state_time))
                                        .size(11.0).color(text_color));
                                });
                            }
                        },
                    );

                    ui.add_space(2.0);
                }

                // ── Parameters ──
                if let Some(anim_state) = state {
                    let has_params = !anim_state.params.floats.is_empty()
                        || !anim_state.params.bools.is_empty()
                        || !anim_state.params.triggers.is_empty();

                    if has_params {
                        collapsible_section(
                            ui,
                            egui_phosphor::regular::FADERS,
                            "Parameters",
                            "scripting",
                            t,
                            "anim_parameters",
                            true,
                            |ui| {
                                let mut row = 0;

                                for (name, value) in &anim_state.params.floats {
                                    inline_property(ui, row, name, t, |ui| {
                                        ui.label(egui::RichText::new(format!("{:.3}", value))
                                            .size(11.0).color(text_color));
                                    });
                                    row += 1;
                                }

                                for (name, value) in &anim_state.params.bools {
                                    inline_property(ui, row, name, t, |ui| {
                                        let icon = if *value {
                                            egui_phosphor::regular::CHECK_CIRCLE
                                        } else {
                                            egui_phosphor::regular::CIRCLE
                                        };
                                        let color = if *value { accent_color } else { muted_color };
                                        ui.label(egui::RichText::new(icon).size(12.0).color(color));
                                        ui.label(egui::RichText::new(if *value { "true" } else { "false" })
                                            .size(11.0).color(text_color));
                                    });
                                    row += 1;
                                }

                                for name in anim_state.params.triggers.keys() {
                                    inline_property(ui, row, name, t, |ui| {
                                        if ui.button(egui::RichText::new("Fire")
                                            .size(10.0).color(accent_color)).clicked() {
                                            self.push_action(AnimEditorAction::FireTrigger {
                                                name: name.clone(),
                                            });
                                        }
                                    });
                                    row += 1;
                                }
                            },
                        );

                        ui.add_space(2.0);
                    }
                }

                // ── Layers ──
                if !animator.layers.is_empty() {
                    collapsible_section(
                        ui,
                        egui_phosphor::regular::STACK,
                        &format!("Layers ({})", animator.layers.len()),
                        "rendering",
                        t,
                        "anim_layers",
                        true,
                        |ui| {
                            for (i, layer) in animator.layers.iter().enumerate() {
                                let mut row = i * 4; // each layer uses ~4 rows

                                inline_property(ui, row, "Layer", t, |ui| {
                                    ui.label(egui::RichText::new(&layer.name)
                                        .size(11.0).color(text_color).strong());
                                });
                                row += 1;

                                inline_property(ui, row, "Weight", t, |ui| {
                                    ui.label(egui::RichText::new(format!("{:.0}%", layer.weight * 100.0))
                                        .size(11.0).color(text_color));
                                });
                                row += 1;

                                inline_property(ui, row, "Blend", t, |ui| {
                                    let mode = format!("{:?}", layer.blend_mode);
                                    ui.label(egui::RichText::new(mode)
                                        .size(11.0).color(muted_color));
                                });
                                row += 1;

                                if let Some(ref clip) = layer.current_clip {
                                    inline_property(ui, row, "Clip", t, |ui| {
                                        ui.label(egui::RichText::new(clip)
                                            .size(11.0).color(accent_color));
                                    });
                                }

                                if let Some(ref mask) = layer.mask {
                                    inline_property(ui, row + 1, "Mask", t, |ui| {
                                        ui.label(egui::RichText::new(
                                            format!("{} bones", mask.len())
                                        ).size(11.0).color(muted_color));
                                    });
                                }
                            }
                        },
                    );

                    ui.add_space(2.0);
                }

                // ── Animator Settings ──
                collapsible_section(
                    ui,
                    egui_phosphor::regular::GEAR,
                    "Animator Settings",
                    "rendering",
                    t,
                    "anim_settings",
                    false,
                    |ui| {
                        let mut row = 0;

                        inline_property(ui, row, "Default Clip", t, |ui| {
                            let label = animator.default_clip.as_deref().unwrap_or("None");
                            ui.label(egui::RichText::new(label)
                                .size(11.0).color(if animator.default_clip.is_some() { text_color } else { muted_color }));
                        });
                        row += 1;

                        inline_property(ui, row, "Blend Time", t, |ui| {
                            ui.label(egui::RichText::new(format!("{:.2}s", animator.blend_duration))
                                .size(11.0).color(text_color));
                        });
                        row += 1;

                        inline_property(ui, row, "Clips", t, |ui| {
                            ui.label(egui::RichText::new(format!("{}", animator.clips.len()))
                                .size(11.0).color(text_color));
                        });
                        row += 1;

                        inline_property(ui, row, "State Machine", t, |ui| {
                            let label = if animator.state_machine.is_some() { "Yes" } else { "No" };
                            let color = if animator.state_machine.is_some() { accent_color } else { muted_color };
                            ui.label(egui::RichText::new(label).size(11.0).color(color));
                        });
                        row += 1;

                        if let Some(anim_state) = state {
                            inline_property(ui, row, "Initialized", t, |ui| {
                                let color = if anim_state.initialized { accent_color } else { muted_color };
                                let icon = if anim_state.initialized {
                                    egui_phosphor::regular::CHECK_CIRCLE
                                } else {
                                    egui_phosphor::regular::WARNING_CIRCLE
                                };
                                ui.label(egui::RichText::new(icon).size(12.0).color(color));
                            });
                        }
                    },
                );
            } else {
                // Fallback without theme — simple labels
                ui.label(egui::RichText::new("Theme not loaded").color(muted_color));
            }
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }

    fn min_size(&self) -> [f32; 2] {
        [200.0, 120.0]
    }
}
