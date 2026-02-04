//! Animation controls panel for managing animation clips
//!
//! Provides animation clip management features:
//! - Clip list with selection
//! - Double-click to rename clips
//! - Create/delete animations
//! - Clip properties (speed, loop, duration)
//!
//! Supports both:
//! - GLTF animations (using Bevy's AnimationPlayer)
//! - Custom editor-created animations (using AnimationData)

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Rounding, Sense, Stroke, Vec2};
use egui_phosphor::regular::{PLAY, PAUSE, STOP, PLUS, TRASH, PENCIL_SIMPLE, ARROWS_CLOCKWISE, COPY};

use crate::core::SelectionState;
use crate::shared::components::animation::{AnimationData, GltfAnimations};
use crate::theming::Theme;

/// State for the animation panel (local UI state only)
#[derive(Default)]
pub struct AnimationPanelState {
    /// Track the last selected entity to detect changes
    pub last_selected_entity: Option<Entity>,
    /// Index of clip currently being renamed (-1 if none)
    pub renaming_clip: Option<usize>,
    /// Buffer for rename text input
    pub rename_buffer: String,
    /// Whether we should focus the rename text input
    pub focus_rename: bool,
    /// Show context menu for clip at index
    pub context_menu_clip: Option<usize>,
}

impl AnimationPanelState {
    pub fn new() -> Self {
        Self {
            last_selected_entity: None,
            renaming_clip: None,
            rename_buffer: String::new(),
            focus_rename: false,
            context_menu_clip: None,
        }
    }

    /// Start renaming a clip
    fn start_rename(&mut self, idx: usize, current_name: &str) {
        self.renaming_clip = Some(idx);
        self.rename_buffer = current_name.to_string();
        self.focus_rename = true;
    }

    /// Cancel any ongoing rename
    fn cancel_rename(&mut self) {
        self.renaming_clip = None;
        self.rename_buffer.clear();
        self.focus_rename = false;
    }
}

/// Render the animation panel content
pub fn render_animation_content(
    ui: &mut egui::Ui,
    selection: &SelectionState,
    gltf_query: &mut Query<&mut GltfAnimations>,
    animation_data_query: &Query<&AnimationData>,
    panel_state: &mut AnimationPanelState,
    theme: &Theme,
) {
    let selected_entity = match selection.selected_entity {
        Some(e) => e,
        None => {
            panel_state.last_selected_entity = None;
            panel_state.cancel_rename();
            render_empty_state(ui, "Select an entity to view animations", theme);
            return;
        }
    };

    // Reset state when entity changes
    if panel_state.last_selected_entity != Some(selected_entity) {
        panel_state.last_selected_entity = Some(selected_entity);
        panel_state.cancel_rename();
        // Reset the active clip on the component when switching entities
        if let Ok(mut gltf_anims) = gltf_query.get_mut(selected_entity) {
            if !gltf_anims.clip_names.is_empty() && gltf_anims.active_clip.is_none() {
                gltf_anims.active_clip = Some(0);
            }
            gltf_anims.is_playing = false;
        }
    }

    // Check for GLTF animations first
    if let Ok(mut gltf_anims) = gltf_query.get_mut(selected_entity) {
        if !gltf_anims.clip_names.is_empty() {
            render_gltf_animation_panel(ui, &mut gltf_anims, panel_state, theme);
            return;
        }
    }

    // Fall back to custom AnimationData
    if let Ok(anim_data) = animation_data_query.get(selected_entity) {
        if !anim_data.clips.is_empty() {
            render_custom_animation_panel(ui, anim_data, panel_state, theme);
            return;
        }
    }

    render_empty_state(ui, "No animations on selected entity", theme);
}

/// Render panel for GLTF animations (using Bevy's animation system)
fn render_gltf_animation_panel(
    ui: &mut egui::Ui,
    gltf_anims: &mut GltfAnimations,
    panel_state: &mut AnimationPanelState,
    theme: &Theme,
) {
    let bg_color = theme.surfaces.popup.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    // Panel header
    render_panel_header(ui, "Animation Clips", gltf_anims.clip_names.len(), theme);

    ui.add_space(4.0);

    // Playback controls toolbar
    egui::Frame::NONE
        .fill(bg_color)
        .stroke(Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // Play/Pause toggle
                let play_icon = if gltf_anims.is_playing { PAUSE } else { PLAY };
                let play_color = if gltf_anims.is_playing { accent } else { text_primary };
                if icon_button(ui, play_icon, if gltf_anims.is_playing { "Pause" } else { "Play" }, play_color) {
                    gltf_anims.is_playing = !gltf_anims.is_playing;
                }

                // Stop
                if icon_button(ui, STOP, "Stop", text_muted) {
                    gltf_anims.is_playing = false;
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Speed control
                ui.label(RichText::new("Speed:").size(10.0).color(text_secondary));
                let mut speed = gltf_anims.speed;
                if ui.add(egui::DragValue::new(&mut speed)
                    .range(0.1..=4.0)
                    .speed(0.05)
                    .suffix("x")
                    .min_decimals(1)
                    .max_decimals(1)).changed() {
                    gltf_anims.speed = speed;
                }

                // Playing indicator
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if gltf_anims.is_playing {
                        ui.label(RichText::new("Playing").size(10.0).color(accent));
                    }
                });
            });
        });

    ui.add_space(4.0);

    // Clip list section
    egui::Frame::NONE
        .fill(bg_color)
        .stroke(Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            // Clip list header
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("Clips").size(10.0).color(text_muted));
            });

            ui.add_space(2.0);

            // Animation list
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(200.0)
                .show(ui, |ui| {
                    let mut clicked_idx: Option<usize> = None;
                    let mut double_clicked_idx: Option<usize> = None;
                    let mut context_menu_idx: Option<usize> = None;

                    for (idx, name) in gltf_anims.clip_names.iter().enumerate() {
                        let is_selected = gltf_anims.active_clip == Some(idx);
                        let is_renaming = panel_state.renaming_clip == Some(idx);
                        let row_bg = if is_selected {
                            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 30)
                        } else if idx % 2 == 0 {
                            theme.surfaces.panel.to_color32()
                        } else {
                            bg_color
                        };

                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 28.0),
                            Sense::click(),
                        );

                        ui.painter().rect_filled(rect, Rounding::ZERO, row_bg);

                        // Clip icon
                        let icon_color = if is_selected { accent } else { text_muted };
                        ui.painter().text(
                            egui::pos2(rect.min.x + 12.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "\u{f008}", // Film icon
                            egui::FontId::proportional(12.0),
                            icon_color,
                        );

                        if is_renaming {
                            // Inline rename text input
                            let text_edit_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.min.x + 30.0, rect.min.y + 4.0),
                                Vec2::new(rect.width() - 60.0, 20.0),
                            );

                            let text_edit = egui::TextEdit::singleline(&mut panel_state.rename_buffer)
                                .desired_width(text_edit_rect.width())
                                .font(egui::FontId::proportional(11.0));

                            let te_response = ui.put(text_edit_rect, text_edit);

                            // Focus on first frame
                            if panel_state.focus_rename {
                                te_response.request_focus();
                                panel_state.focus_rename = false;
                            }

                            // Confirm on Enter, cancel on Escape
                            if te_response.lost_focus() {
                                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    // GLTF clip names can't actually be renamed (they come from the file)
                                    // Just cancel for now
                                }
                                panel_state.cancel_rename();
                            }

                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                panel_state.cancel_rename();
                            }
                        } else {
                            // Clip name
                            ui.painter().text(
                                egui::pos2(rect.min.x + 30.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                name,
                                egui::FontId::proportional(11.0),
                                if is_selected { text_primary } else { text_secondary },
                            );

                            // Playing indicator
                            if is_selected && gltf_anims.is_playing {
                                ui.painter().text(
                                    egui::pos2(rect.max.x - 24.0, rect.center().y),
                                    egui::Align2::CENTER_CENTER,
                                    PLAY,
                                    egui::FontId::proportional(10.0),
                                    accent,
                                );
                            }
                        }

                        if response.clicked() && !is_renaming {
                            clicked_idx = Some(idx);
                        }

                        if response.hovered() && !is_renaming {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }

                        if response.double_clicked() && !is_renaming {
                            double_clicked_idx = Some(idx);
                        }

                        // Context menu on right-click
                        response.context_menu(|ui| {
                            if ui.button("Play").clicked() {
                                gltf_anims.active_clip = Some(idx);
                                gltf_anims.is_playing = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            // GLTF clips can't be renamed or deleted
                            ui.add_enabled(false, egui::Button::new("Rename"));
                            ui.add_enabled(false, egui::Button::new("Duplicate"));
                            ui.add_enabled(false, egui::Button::new("Delete"));
                        });
                    }

                    // Apply changes after iteration
                    if let Some(idx) = clicked_idx {
                        gltf_anims.active_clip = Some(idx);
                    }
                    if let Some(idx) = double_clicked_idx {
                        gltf_anims.active_clip = Some(idx);
                        gltf_anims.is_playing = true;
                    }
                });
        });

    ui.add_space(8.0);

    // Clip properties section (for selected clip)
    if let Some(active_idx) = gltf_anims.active_clip {
        if let Some(name) = gltf_anims.clip_names.get(active_idx) {
            render_clip_properties_section(ui, name, gltf_anims.speed, true, theme);
        }
    }
}

/// Render panel for custom editor-created animations
fn render_custom_animation_panel(
    ui: &mut egui::Ui,
    anim_data: &AnimationData,
    panel_state: &mut AnimationPanelState,
    theme: &Theme,
) {
    let bg_color = theme.surfaces.popup.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    // Panel header with add button
    render_panel_header_with_action(ui, "Animation Clips", anim_data.clips.len(), PLUS, "New Animation", theme);

    ui.add_space(4.0);

    // Clip list
    egui::Frame::NONE
        .fill(bg_color)
        .stroke(Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(200.0)
                .show(ui, |ui| {
                    for (idx, clip) in anim_data.clips.iter().enumerate() {
                        let is_selected = anim_data.active_clip == Some(idx);
                        let is_renaming = panel_state.renaming_clip == Some(idx);
                        let row_bg = if is_selected {
                            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 30)
                        } else if idx % 2 == 0 {
                            theme.surfaces.panel.to_color32()
                        } else {
                            bg_color
                        };

                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 28.0),
                            Sense::click(),
                        );

                        ui.painter().rect_filled(rect, Rounding::ZERO, row_bg);

                        // Clip icon
                        let icon_color = if is_selected { accent } else { text_muted };
                        ui.painter().text(
                            egui::pos2(rect.min.x + 12.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "\u{f008}",
                            egui::FontId::proportional(12.0),
                            icon_color,
                        );

                        if is_renaming {
                            // Inline rename
                            let text_edit_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.min.x + 30.0, rect.min.y + 4.0),
                                Vec2::new(rect.width() - 80.0, 20.0),
                            );

                            let text_edit = egui::TextEdit::singleline(&mut panel_state.rename_buffer)
                                .desired_width(text_edit_rect.width())
                                .font(egui::FontId::proportional(11.0));

                            let te_response = ui.put(text_edit_rect, text_edit);

                            if panel_state.focus_rename {
                                te_response.request_focus();
                                panel_state.focus_rename = false;
                            }

                            if te_response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                panel_state.cancel_rename();
                            }
                        } else {
                            // Clip name
                            ui.painter().text(
                                egui::pos2(rect.min.x + 30.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &clip.name,
                                egui::FontId::proportional(11.0),
                                if is_selected { text_primary } else { text_secondary },
                            );

                            // Duration
                            let duration_str = format!("{:.1}s", clip.duration());
                            ui.painter().text(
                                egui::pos2(rect.max.x - 40.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                duration_str,
                                egui::FontId::proportional(9.0),
                                text_muted,
                            );

                            // Loop indicator
                            if clip.looping {
                                ui.painter().text(
                                    egui::pos2(rect.max.x - 12.0, rect.center().y),
                                    egui::Align2::CENTER_CENTER,
                                    ARROWS_CLOCKWISE,
                                    egui::FontId::proportional(10.0),
                                    text_muted,
                                );
                            }
                        }

                        if response.hovered() && !is_renaming {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }

                        if response.double_clicked() && !is_renaming {
                            panel_state.start_rename(idx, &clip.name);
                        }

                        // Context menu
                        response.context_menu(|ui| {
                            if ui.button(format!("{} Rename", PENCIL_SIMPLE)).clicked() {
                                panel_state.start_rename(idx, &clip.name);
                                ui.close_menu();
                            }
                            if ui.button(format!("{} Duplicate", COPY)).clicked() {
                                // TODO: Duplicate clip
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button(RichText::new(format!("{} Delete", TRASH)).color(Color32::from_rgb(220, 80, 80))).clicked() {
                                // TODO: Delete clip
                                ui.close_menu();
                            }
                        });
                    }
                });
        });

    ui.add_space(8.0);

    // Properties for active clip
    if let Some(active_idx) = anim_data.active_clip {
        if let Some(clip) = anim_data.clips.get(active_idx) {
            render_clip_properties_section(ui, &clip.name, clip.speed, clip.looping, theme);
        }
    }
}

/// Render section header
fn render_panel_header(ui: &mut egui::Ui, title: &str, count: usize, theme: &Theme) {
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(11.0).color(text_secondary));
        ui.label(RichText::new(format!("({})", count)).size(10.0).color(text_muted));
    });
}

/// Render section header with action button
fn render_panel_header_with_action(
    ui: &mut egui::Ui,
    title: &str,
    count: usize,
    icon: &str,
    tooltip: &str,
    theme: &Theme,
) -> bool {
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let mut clicked = false;

    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(11.0).color(text_secondary));
        ui.label(RichText::new(format!("({})", count)).size(10.0).color(text_muted));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, icon, tooltip, text_muted) {
                clicked = true;
            }
        });
    });

    clicked
}

/// Render clip properties section
fn render_clip_properties_section(
    ui: &mut egui::Ui,
    name: &str,
    speed: f32,
    looping: bool,
    theme: &Theme,
) {
    let bg_color = theme.surfaces.popup.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_primary = theme.text.primary.to_color32();

    egui::Frame::NONE
        .fill(bg_color)
        .stroke(Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.label(RichText::new("Properties").size(10.0).color(text_muted));
            ui.add_space(4.0);

            // Clip name
            ui.horizontal(|ui| {
                ui.label(RichText::new("Name:").size(10.0).color(text_secondary));
                ui.add_space(4.0);
                ui.label(RichText::new(name).size(10.0).color(text_primary));
            });

            // Speed
            ui.horizontal(|ui| {
                ui.label(RichText::new("Speed:").size(10.0).color(text_secondary));
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{:.1}x", speed)).size(10.0).color(text_primary));
            });

            // Loop
            ui.horizontal(|ui| {
                ui.label(RichText::new("Loop:").size(10.0).color(text_secondary));
                ui.add_space(4.0);
                ui.label(RichText::new(if looping { "Yes" } else { "No" }).size(10.0).color(text_primary));
            });
        });
}

/// Render empty state when no animation is available
fn render_empty_state(ui: &mut egui::Ui, message: &str, theme: &Theme) {
    let text_muted = theme.text.muted.to_color32();
    let text_disabled = theme.text.disabled.to_color32();

    ui.vertical_centered(|ui| {
        ui.add_space(40.0);

        // Film strip icon
        ui.label(RichText::new("\u{f008}").size(36.0).color(text_disabled));

        ui.add_space(8.0);
        ui.label(RichText::new("Animation").size(14.0).color(text_muted));

        ui.add_space(4.0);
        ui.label(RichText::new(message).size(11.0).color(text_disabled));

        ui.add_space(16.0);
        ui.label(RichText::new("Import a GLTF/GLB model with\nanimations to get started").size(10.0).color(text_disabled));
    });
}

/// Helper: Create an icon button
fn icon_button(ui: &mut egui::Ui, icon: &str, tooltip: &str, color: Color32) -> bool {
    let btn = ui.add(
        egui::Button::new(RichText::new(icon).size(12.0).color(color))
            .fill(Color32::TRANSPARENT)
            .min_size(Vec2::new(22.0, 22.0))
    );

    if btn.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    btn.on_hover_text(tooltip).clicked()
}
