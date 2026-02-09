//! Pixel Timeline Panel
//!
//! Animation frames row, add/delete/duplicate frame, frame duration,
//! playback controls, onion skin toggle.

use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use crate::pixel_editor::PixelEditorState;
use crate::theming::Theme;

use egui_phosphor::regular::{PLAY, PAUSE, STOP, PLUS, TRASH, COPY};

/// Render the pixel timeline panel
pub fn render_pixel_timeline_content(
    ui: &mut egui::Ui,
    state: &mut PixelEditorState,
    theme: &Theme,
    dt_ms: f32,
) {
    let Some(project) = state.project.as_mut() else {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new("No project open")
                .color(theme.text.muted.to_color32()));
        });
        return;
    };

    let text_color = theme.text.primary.to_color32();
    let muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    // Playback controls row
    ui.horizontal(|ui| {
        // Play/pause
        let play_icon = if state.playing { PAUSE } else { PLAY };
        if ui.small_button(play_icon).on_hover_text(if state.playing { "Pause" } else { "Play" }).clicked() {
            state.playing = !state.playing;
            state.playback_timer_ms = 0.0;
        }

        // Stop
        if ui.small_button(STOP).on_hover_text("Stop").clicked() {
            state.playing = false;
            project.active_frame = 0;
            project.texture_dirty = true;
            state.playback_timer_ms = 0.0;
        }

        ui.separator();

        // Frame info
        ui.label(egui::RichText::new(format!("Frame {}/{}", project.active_frame + 1, project.frames.len()))
            .size(11.0).color(text_color));

        ui.separator();

        // Frame duration
        if project.active_frame < project.frames.len() {
            ui.label(egui::RichText::new("ms:").size(10.0).color(muted));
            let mut dur = project.frames[project.active_frame].duration_ms as i32;
            if ui.add(egui::DragValue::new(&mut dur).range(10..=5000).speed(5)).changed() {
                project.frames[project.active_frame].duration_ms = dur.max(10) as u32;
            }
        }

        ui.separator();

        // Add/delete/duplicate frame
        if ui.small_button(format!("{} Frame", PLUS)).clicked() {
            project.add_frame();
        }
        if ui.small_button(format!("{}", COPY)).on_hover_text("Duplicate Frame").clicked() {
            let idx = project.active_frame;
            project.duplicate_frame(idx);
        }
        if ui.small_button(format!("{}", TRASH)).on_hover_text("Delete Frame").clicked() {
            let idx = project.active_frame;
            project.remove_frame(idx);
        }

        ui.separator();

        // Onion skin toggle
        let onion_text = if state.onion_skin { "Onion: ON" } else { "Onion: OFF" };
        if ui.small_button(onion_text).clicked() {
            state.onion_skin = !state.onion_skin;
        }
    });

    ui.separator();

    // Frame thumbnails row
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            let thumb_size = 48.0;
            let num_frames = project.frames.len();

            for i in 0..num_frames {
                let is_active = i == project.active_frame;

                let (rect, resp) = ui.allocate_exact_size(
                    Vec2::new(thumb_size, thumb_size + 16.0),
                    Sense::click(),
                );

                // Frame thumbnail area
                let thumb_rect = Rect::from_min_size(rect.min, Vec2::splat(thumb_size));

                // Background
                let bg = if is_active {
                    accent.gamma_multiply(0.3)
                } else if resp.hovered() {
                    Color32::from_gray(50)
                } else {
                    Color32::from_gray(35)
                };
                ui.painter().rect_filled(thumb_rect, 4.0, bg);

                if is_active {
                    ui.painter().rect_stroke(thumb_rect, 4.0, Stroke::new(2.0, accent), egui::StrokeKind::Outside);
                }

                // Frame number
                ui.painter().text(
                    Pos2::new(thumb_rect.center().x, thumb_rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(14.0),
                    if is_active { text_color } else { muted },
                );

                // Duration label below
                if i < project.frames.len() {
                    ui.painter().text(
                        Pos2::new(rect.center().x, thumb_rect.max.y + 8.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{}ms", project.frames[i].duration_ms),
                        egui::FontId::proportional(9.0),
                        muted,
                    );
                }

                if resp.clicked() {
                    project.active_frame = i;
                    project.texture_dirty = true;
                }
            }
        });
    });

    // Animation playback logic
    if state.playing && !project.frames.is_empty() {
        state.playback_timer_ms += dt_ms;
        let frame_dur = project.frames[project.active_frame].duration_ms as f32;
        if state.playback_timer_ms >= frame_dur {
            state.playback_timer_ms -= frame_dur;
            project.active_frame = (project.active_frame + 1) % project.frames.len();
            project.texture_dirty = true;
        }
    }
}
