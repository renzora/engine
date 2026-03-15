//! Timeline panel — transport controls, time ruler, scrubber, track lanes.
//!
//! Panel id: `"timeline"` (matches layout_animation() in layouts.rs).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::{AnimatorComponent, AnimatorState};
use renzora_editor::{EditorPanel, PanelLocation};
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
        let (text_color, muted_color, accent_color, bg_color) =
            if let Some(tm) = world.get_resource::<ThemeManager>() {
                let t = &tm.active_theme;
                (
                    t.text.primary.to_color32(),
                    t.text.muted.to_color32(),
                    t.semantic.accent.to_color32(),
                    t.surfaces.panel.to_color32(),
                )
            } else {
                (
                    egui::Color32::WHITE,
                    egui::Color32::GRAY,
                    egui::Color32::from_rgb(100, 149, 237),
                    egui::Color32::from_rgb(24, 25, 30),
                )
            };

        let editor_state = world.get_resource::<AnimationEditorState>();
        let (scrub_time, is_previewing, preview_speed, preview_looping, timeline_zoom, selected_entity) =
            if let Some(s) = editor_state {
                (s.scrub_time, s.is_previewing, s.preview_speed, s.preview_looping, s.timeline_zoom, s.selected_entity)
            } else {
                (0.0, false, 1.0, true, 100.0, None)
            };

        // ── Transport bar ──
        ui.horizontal(|ui| {
            // Play/Pause
            let play_icon = if is_previewing {
                egui_phosphor::regular::PAUSE
            } else {
                egui_phosphor::regular::PLAY
            };
            if ui.button(egui::RichText::new(play_icon).color(text_color)).clicked() {
                self.push_action(AnimEditorAction::TogglePreview);
            }

            // Stop
            if ui.button(egui::RichText::new(egui_phosphor::regular::STOP).color(text_color)).clicked() {
                self.push_action(AnimEditorAction::StopPreview);
            }

            // Loop toggle
            let loop_color = if preview_looping { accent_color } else { muted_color };
            if ui.button(egui::RichText::new(egui_phosphor::regular::REPEAT).color(loop_color)).clicked() {
                self.push_action(AnimEditorAction::SetPreviewLooping(!preview_looping));
            }

            ui.separator();

            // Speed
            ui.label(egui::RichText::new("Speed:").color(muted_color));
            for &speed in &[0.25f32, 0.5, 1.0, 2.0] {
                let label = format!("{:.0}x", speed);
                let color = if (preview_speed - speed).abs() < 0.01 { accent_color } else { text_color };
                if ui.button(egui::RichText::new(&label).color(color).small()).clicked() {
                    self.push_action(AnimEditorAction::SetPreviewSpeed(speed));
                }
            }

            ui.separator();

            // Time display
            let mins = (scrub_time / 60.0) as u32;
            let secs = scrub_time % 60.0;
            ui.label(egui::RichText::new(format!("{:02}:{:05.2}", mins, secs)).color(text_color).monospace());
        });

        ui.separator();

        // ── Timeline area ──
        let available = ui.available_rect_before_wrap();
        let track_height = 24.0;
        let ruler_height = 20.0;
        let scrubber_color = egui::Color32::from_rgb(255, 80, 80);

        // Get clip info for track lanes
        let mut clip_names: Vec<String> = Vec::new();
        let mut duration = 2.0f32; // default visible duration

        if let Some(entity) = selected_entity {
            if let Some(animator) = world.get::<AnimatorComponent>(entity) {
                for slot in &animator.clips {
                    clip_names.push(slot.name.clone());
                }
            }
        }

        let total_width = duration * timeline_zoom;
        let time_rect = egui::Rect::from_min_size(
            available.min + egui::vec2(0.0, ruler_height),
            egui::vec2(available.width(), available.height() - ruler_height),
        );

        // Draw ruler
        let ruler_rect = egui::Rect::from_min_size(available.min, egui::vec2(available.width(), ruler_height));
        let painter = ui.painter_at(ruler_rect);

        // Time markers
        let step = (0.5 / (timeline_zoom / 100.0)).max(0.1);
        let mut t = 0.0f32;
        while t <= duration {
            let x = ruler_rect.min.x + t * timeline_zoom;
            if x <= ruler_rect.max.x {
                painter.line_segment(
                    [egui::pos2(x, ruler_rect.min.y + 12.0), egui::pos2(x, ruler_rect.max.y)],
                    egui::Stroke::new(1.0, muted_color),
                );
                painter.text(
                    egui::pos2(x + 2.0, ruler_rect.min.y),
                    egui::Align2::LEFT_TOP,
                    format!("{:.1}s", t),
                    egui::FontId::proportional(10.0),
                    muted_color,
                );
            }
            t += step;
        }

        // Draw track lanes
        let track_painter = ui.painter_at(time_rect);
        for (i, name) in clip_names.iter().enumerate() {
            let y = time_rect.min.y + i as f32 * track_height;
            let lane_rect = egui::Rect::from_min_size(
                egui::pos2(time_rect.min.x, y),
                egui::vec2(time_rect.width(), track_height),
            );

            // Alternating background
            if i % 2 == 0 {
                track_painter.rect_filled(
                    lane_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(255, 255, 255, 5),
                );
            }

            // Track label
            track_painter.text(
                egui::pos2(lane_rect.min.x + 4.0, lane_rect.center().y),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(11.0),
                text_color,
            );
        }

        // Draw scrubber line
        let scrub_x = available.min.x + scrub_time * timeline_zoom;
        if scrub_x >= available.min.x && scrub_x <= available.max.x {
            let painter = ui.painter_at(available);
            painter.line_segment(
                [egui::pos2(scrub_x, available.min.y), egui::pos2(scrub_x, available.max.y)],
                egui::Stroke::new(2.0, scrubber_color),
            );
            // Scrubber handle
            painter.circle_filled(egui::pos2(scrub_x, available.min.y + 4.0), 5.0, scrubber_color);
        }

        // Handle click-to-scrub on the timeline area
        let response = ui.allocate_rect(available, egui::Sense::click_and_drag());
        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((pos.x - available.min.x) / timeline_zoom).max(0.0);
                self.push_action(AnimEditorAction::SetScrubTime(t));
            }
        }

        // Zoom with scroll
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let new_zoom = (timeline_zoom + scroll * 0.5).clamp(20.0, 500.0);
                self.push_action(AnimEditorAction::SetTimelineZoom(new_zoom));
            }
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn min_size(&self) -> [f32; 2] {
        [200.0, 100.0]
    }
}
