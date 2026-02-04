//! Timeline panel for animation editing
//!
//! Provides a full timeline editor with:
//! - Time ruler with tick marks
//! - Draggable playhead for scrubbing
//! - Playback controls toolbar
//! - Track list showing animation clips
//! - Real-time playhead position from AnimationPlayer

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Rect, RichText, Rounding, Sense, Stroke, Vec2};
use egui_phosphor::regular::{
    CARET_LEFT, CARET_RIGHT, CARET_DOUBLE_LEFT, CARET_DOUBLE_RIGHT,
    PLAY, PAUSE, STOP, ARROWS_CLOCKWISE, PLUS, MINUS, WAVEFORM,
    CARET_DOWN, CARET_RIGHT as CARET_RIGHT_SMALL,
};

use crate::core::{AnimationTimelineState, SelectionState, TimelinePlayState};
use crate::shared::components::animation::{GltfAnimations, GltfAnimationStorage, AnimationData};
use crate::theming::Theme;

/// Height of the toolbar area
const TOOLBAR_HEIGHT: f32 = 32.0;
/// Height of the time ruler
const RULER_HEIGHT: f32 = 24.0;
/// Height of each track row
const TRACK_HEIGHT: f32 = 28.0;
/// Width of the track label area
const TRACK_LABEL_WIDTH: f32 = 200.0;
/// Playhead width
const PLAYHEAD_WIDTH: f32 = 2.0;

/// Render the timeline panel content
pub fn render_timeline_content(
    ui: &mut egui::Ui,
    timeline_state: &mut AnimationTimelineState,
    selection: &SelectionState,
    gltf_query: &Query<&mut GltfAnimations>,
    animation_data_query: &Query<&AnimationData>,
    theme: &Theme,
) {
    let _panel_rect = ui.available_rect_before_wrap();

    // Check if we have an entity selected with animations
    let selected_entity = match selection.selected_entity {
        Some(e) => e,
        None => {
            render_empty_timeline(ui, "Select an entity to edit animations", theme);
            return;
        }
    };

    // Try to get animation data (GLTF or custom)
    let gltf_result = gltf_query.get(selected_entity);
    let has_gltf = gltf_result.as_ref().map(|g| !g.clip_names.is_empty()).unwrap_or(false);
    let has_custom = animation_data_query.get(selected_entity).map(|a| !a.clips.is_empty()).unwrap_or(false);

    if !has_gltf && !has_custom {
        render_empty_timeline(ui, "No animations on selected entity", theme);
        return;
    }

    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let bg_color = theme.surfaces.panel.to_color32();
    let accent = theme.semantic.accent.to_color32();

    // Get clip info for display
    let (clip_names, active_clip, is_playing, clip_duration) = if let Ok(gltf) = gltf_result {
        let duration = 5.0; // Default duration for GLTF (we don't have direct access)
        (gltf.clip_names.clone(), gltf.active_clip, gltf.is_playing, duration)
    } else if let Ok(anim_data) = animation_data_query.get(selected_entity) {
        let names: Vec<String> = anim_data.clips.iter().map(|c| c.name.clone()).collect();
        let duration = anim_data.get_active_clip().map(|c| c.duration()).unwrap_or(5.0);
        (names, anim_data.active_clip, false, duration)
    } else {
        (vec![], None, false, 5.0)
    };

    // Sync timeline state with animation
    if is_playing && timeline_state.play_state != TimelinePlayState::Playing {
        timeline_state.play_state = TimelinePlayState::Playing;
    }

    // Render toolbar
    render_timeline_toolbar(ui, timeline_state, is_playing, theme);

    ui.add_space(2.0);

    // Main timeline area
    let remaining_rect = ui.available_rect_before_wrap();
    let timeline_width = (remaining_rect.width() - TRACK_LABEL_WIDTH).max(100.0);

    // Time ruler area
    let ruler_rect = Rect::from_min_size(
        Pos2::new(remaining_rect.min.x + TRACK_LABEL_WIDTH, remaining_rect.min.y),
        Vec2::new(timeline_width, RULER_HEIGHT),
    );
    render_time_ruler(ui, timeline_state, ruler_rect, theme);

    // Track area
    let track_start_y = remaining_rect.min.y + RULER_HEIGHT;

    egui::ScrollArea::vertical()
        .id_salt("timeline_tracks")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(remaining_rect.width());

            // Render clip tracks
            for (idx, name) in clip_names.iter().enumerate() {
                let is_active = active_clip == Some(idx);
                render_clip_track(ui, idx, name, is_active, is_playing && is_active, timeline_state, timeline_width, clip_duration, theme);
            }

            // If no clips, show placeholder
            if clip_names.is_empty() {
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(TRACK_LABEL_WIDTH + timeline_width, TRACK_HEIGHT * 2.0),
                    Sense::hover(),
                );
                ui.painter().rect_filled(rect, Rounding::ZERO, bg_color);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No animation clips",
                    egui::FontId::proportional(11.0),
                    text_muted,
                );
            }
        });

    // Render playhead overlay
    let playhead_rect = Rect::from_min_max(
        Pos2::new(remaining_rect.min.x + TRACK_LABEL_WIDTH, remaining_rect.min.y),
        Pos2::new(remaining_rect.max.x, remaining_rect.max.y.min(track_start_y + 200.0)),
    );
    render_playhead(ui, timeline_state, playhead_rect, theme);

    // Handle timeline interactions
    handle_timeline_interactions(ui, timeline_state, playhead_rect, clip_duration);
}

/// Render a single clip track
fn render_clip_track(
    ui: &mut egui::Ui,
    idx: usize,
    name: &str,
    is_active: bool,
    is_playing: bool,
    state: &AnimationTimelineState,
    timeline_width: f32,
    duration: f32,
    theme: &Theme,
) {
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let bg_even = theme.surfaces.panel.to_color32();
    let bg_odd = Color32::from_rgb(32, 34, 38);

    let bg = if is_active {
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 25)
    } else if idx % 2 == 0 {
        bg_even
    } else {
        bg_odd
    };

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(TRACK_LABEL_WIDTH + timeline_width, TRACK_HEIGHT),
        Sense::click(),
    );

    // Background
    ui.painter().rect_filled(rect, Rounding::ZERO, bg);

    // Label area
    let label_rect = Rect::from_min_size(rect.min, Vec2::new(TRACK_LABEL_WIDTH, TRACK_HEIGHT));

    // Clip icon
    let icon_color = if is_active { accent } else { text_muted };
    ui.painter().text(
        Pos2::new(label_rect.min.x + 12.0, label_rect.center().y),
        egui::Align2::LEFT_CENTER,
        if is_playing { PLAY } else { "\u{f008}" }, // Film or play icon
        egui::FontId::proportional(12.0),
        icon_color,
    );

    // Clip name
    ui.painter().text(
        Pos2::new(label_rect.min.x + 32.0, label_rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(11.0),
        if is_active { text_primary } else { text_secondary },
    );

    // Duration badge
    let duration_str = format!("{:.1}s", duration);
    ui.painter().text(
        Pos2::new(label_rect.max.x - 8.0, label_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        duration_str,
        egui::FontId::proportional(9.0),
        text_muted,
    );

    // Separator line
    ui.painter().line_segment(
        [Pos2::new(rect.min.x + TRACK_LABEL_WIDTH, rect.min.y),
         Pos2::new(rect.min.x + TRACK_LABEL_WIDTH, rect.max.y)],
        Stroke::new(1.0, Color32::from_rgb(50, 52, 58)),
    );

    // Timeline area - draw clip bar
    let timeline_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + TRACK_LABEL_WIDTH, rect.min.y),
        rect.max,
    );

    // Calculate clip bar position based on view
    let clip_start_x = timeline_rect.min.x + state.time_to_x(0.0, timeline_width);
    let clip_end_x = timeline_rect.min.x + state.time_to_x(duration, timeline_width);

    // Clip bar (only if visible in view)
    if clip_end_x > timeline_rect.min.x && clip_start_x < timeline_rect.max.x {
        let bar_start = clip_start_x.max(timeline_rect.min.x);
        let bar_end = clip_end_x.min(timeline_rect.max.x);

        let bar_rect = Rect::from_min_max(
            Pos2::new(bar_start, rect.min.y + 6.0),
            Pos2::new(bar_end, rect.max.y - 6.0),
        );

        // Clip bar background
        let bar_color = if is_active {
            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 120)
        } else {
            Color32::from_rgb(70, 75, 85)
        };
        ui.painter().rect_filled(bar_rect, 4.0, bar_color);

        // Clip bar border
        if is_active {
            ui.painter().rect_stroke(bar_rect, 4.0, Stroke::new(1.0, accent), egui::StrokeKind::Inside);
        }

        // Draw keyframe diamonds along the clip bar
        // For GLTF animations, we simulate keyframes at regular intervals
        // Real keyframe data would come from parsing AnimationClip curves
        let keyframe_interval = 0.25; // Every quarter second
        let num_keyframes = ((duration / keyframe_interval).ceil() as usize).min(50);

        for i in 0..=num_keyframes {
            let t = i as f32 * keyframe_interval;
            if t <= duration {
                let kf_x = timeline_rect.min.x + state.time_to_x(t, timeline_width);
                if kf_x >= bar_start && kf_x <= bar_end {
                    // Draw diamond-shaped keyframe marker
                    let kf_y = bar_rect.center().y;
                    let size = 4.0;

                    // Only show every 4th keyframe as a diamond, others as ticks
                    if i % 4 == 0 {
                        let diamond = [
                            Pos2::new(kf_x, kf_y - size),
                            Pos2::new(kf_x + size, kf_y),
                            Pos2::new(kf_x, kf_y + size),
                            Pos2::new(kf_x - size, kf_y),
                        ];
                        let keyframe_color = if is_active {
                            Color32::from_rgb(255, 200, 100) // Gold for active
                        } else {
                            Color32::from_rgb(180, 180, 180) // Gray for inactive
                        };
                        ui.painter().add(egui::Shape::convex_polygon(
                            diamond.to_vec(),
                            keyframe_color,
                            Stroke::new(1.0, Color32::from_rgb(40, 40, 40)),
                        ));
                    } else {
                        // Small tick mark for sub-keyframes
                        ui.painter().line_segment(
                            [Pos2::new(kf_x, bar_rect.min.y + 2.0), Pos2::new(kf_x, bar_rect.max.y - 2.0)],
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20)),
                        );
                    }
                }
            }
        }
    }

    // Hover effect
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
}

/// Render the timeline toolbar with playback controls
fn render_timeline_toolbar(
    ui: &mut egui::Ui,
    state: &mut AnimationTimelineState,
    is_animation_playing: bool,
    theme: &Theme,
) {
    let bg_color = theme.surfaces.popup.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    egui::Frame::NONE
        .fill(bg_color)
        .stroke(Stroke::new(1.0, border_color))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // Skip to start
                if icon_button(ui, CARET_DOUBLE_LEFT, "Go to start", text_muted) {
                    state.current_time = 0.0;
                }

                // Previous frame
                if icon_button(ui, CARET_LEFT, "Previous frame", text_muted) {
                    state.current_time = (state.current_time - 0.033).max(0.0);
                }

                // Play/Pause indicator
                let is_playing = state.play_state == TimelinePlayState::Playing || is_animation_playing;
                let play_icon = if is_playing { PAUSE } else { PLAY };
                let play_color = if is_playing { accent } else { text_primary };
                if icon_button(ui, play_icon, if is_playing { "Pause" } else { "Play" }, play_color) {
                    state.toggle_play();
                }

                // Stop
                if icon_button(ui, STOP, "Stop", text_muted) {
                    state.stop();
                }

                // Next frame
                if icon_button(ui, CARET_RIGHT, "Next frame", text_muted) {
                    state.current_time += 0.033;
                }

                // Skip to end
                if icon_button(ui, CARET_DOUBLE_RIGHT, "Go to end", text_muted) {
                    state.current_time = state.view_end;
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // Current time display
                let minutes = (state.current_time / 60.0).floor() as i32;
                let seconds = state.current_time % 60.0;
                let time_str = format!("{:02}:{:05.2}", minutes, seconds);

                // Time display with background
                egui::Frame::NONE
                    .fill(Color32::from_rgb(25, 27, 32))
                    .inner_margin(egui::Margin::symmetric(8, 2))
                    .corner_radius(3.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(time_str).size(12.0).color(text_primary).monospace());
                    });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // Loop toggle (visual indicator)
                let loop_active = false; // Would come from clip settings
                let loop_color = if loop_active { accent } else { text_muted };
                if icon_button(ui, ARROWS_CLOCKWISE, "Loop", loop_color) {
                    // Toggle loop would go here
                }

                ui.add_space(8.0);

                // Zoom controls
                if icon_button(ui, MINUS, "Zoom out", text_muted) {
                    state.zoom_out();
                }

                let zoom_text = format!("{:.1}s", state.zoom);
                ui.label(RichText::new(zoom_text).size(10.0).color(text_muted));

                if icon_button(ui, PLUS, "Zoom in", text_muted) {
                    state.zoom_in();
                }

                // Status indicator on right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_animation_playing {
                        ui.label(RichText::new("Playing").size(10.0).color(accent));
                        ui.label(RichText::new(PLAY).size(10.0).color(accent));
                    }
                });
            });
        });
}

/// Render the time ruler with tick marks
fn render_time_ruler(
    ui: &mut egui::Ui,
    state: &AnimationTimelineState,
    rect: Rect,
    theme: &Theme,
) {
    let bg_color = Color32::from_rgb(30, 32, 38);
    let border_color = theme.widgets.border.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let tick_color = Color32::from_rgb(60, 62, 68);
    let tick_major_color = Color32::from_rgb(80, 82, 88);

    // Background
    ui.painter().rect_filled(rect, Rounding::ZERO, bg_color);

    // Bottom border
    ui.painter().line_segment(
        [Pos2::new(rect.min.x, rect.max.y), Pos2::new(rect.max.x, rect.max.y)],
        Stroke::new(1.0, border_color),
    );

    let timeline_width = rect.width();
    let view_duration = state.view_end - state.view_start;
    if view_duration <= 0.0 {
        return;
    }

    // Calculate tick interval based on zoom level
    let tick_interval = calculate_tick_interval(view_duration, timeline_width);
    let major_interval = tick_interval * 5.0;

    // Draw ticks
    let start_tick = (state.view_start / tick_interval).floor() * tick_interval;
    let mut time = start_tick;

    while time <= state.view_end + tick_interval {
        let x = rect.min.x + state.time_to_x(time, timeline_width);

        if x >= rect.min.x - 10.0 && x <= rect.max.x + 10.0 {
            let is_major = (time / major_interval).fract().abs() < 0.001 || time.abs() < 0.001;
            let tick_height = if is_major { 14.0 } else { 7.0 };
            let tick_col = if is_major { tick_major_color } else { tick_color };

            // Draw tick
            ui.painter().line_segment(
                [Pos2::new(x, rect.max.y - tick_height), Pos2::new(x, rect.max.y)],
                Stroke::new(1.0, tick_col),
            );

            // Draw time label for major ticks
            if is_major && x >= rect.min.x && x <= rect.max.x - 20.0 {
                let label = format_time(time);
                ui.painter().text(
                    Pos2::new(x + 4.0, rect.min.y + 10.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(9.0),
                    text_muted,
                );
            }
        }

        time += tick_interval;
    }
}

/// Render the playhead (vertical line at current time)
fn render_playhead(
    ui: &mut egui::Ui,
    state: &AnimationTimelineState,
    rect: Rect,
    theme: &Theme,
) {
    let accent = theme.semantic.accent.to_color32();
    let timeline_width = rect.width();

    // Only draw if current time is in view
    if state.current_time >= state.view_start && state.current_time <= state.view_end {
        let x = rect.min.x + state.time_to_x(state.current_time, timeline_width);

        // Playhead line
        ui.painter().line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            Stroke::new(PLAYHEAD_WIDTH, accent),
        );

        // Playhead head (triangle at top)
        let head_size = 10.0;
        let head_points = vec![
            Pos2::new(x - head_size / 2.0, rect.min.y),
            Pos2::new(x + head_size / 2.0, rect.min.y),
            Pos2::new(x, rect.min.y + head_size),
        ];
        ui.painter().add(egui::Shape::convex_polygon(
            head_points,
            accent,
            Stroke::NONE,
        ));
    }
}

/// Handle timeline mouse interactions (scrubbing, selection)
fn handle_timeline_interactions(
    ui: &mut egui::Ui,
    state: &mut AnimationTimelineState,
    rect: Rect,
    clip_duration: f32,
) {
    let response = ui.interact(rect, ui.id().with("timeline_interact"), Sense::click_and_drag());

    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if rect.contains(pos) {
                let local_x = pos.x - rect.min.x;
                let new_time = state.x_to_time(local_x, rect.width());
                state.set_time(new_time, clip_duration.max(state.view_end));
                state.scrubbing = true;
            }
        }
    }

    if response.drag_stopped() {
        state.scrubbing = false;
    }

    // Change cursor when hovering
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Text);
    }

    // Handle scroll to pan/zoom
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta);

        // Horizontal scroll for panning
        if scroll_delta.x.abs() > 0.1 {
            state.scroll(-scroll_delta.x * 0.005 * state.zoom);
        }

        // Vertical scroll for zooming (or with Ctrl)
        if ui.input(|i| i.modifiers.ctrl) || scroll_delta.y.abs() > scroll_delta.x.abs() {
            if scroll_delta.y > 0.0 {
                state.zoom_in();
            } else if scroll_delta.y < 0.0 {
                state.zoom_out();
            }
        }
    }
}

/// Render empty state for timeline
fn render_empty_timeline(ui: &mut egui::Ui, message: &str, theme: &Theme) {
    let text_muted = theme.text.muted.to_color32();
    let text_disabled = theme.text.disabled.to_color32();

    ui.vertical_centered(|ui| {
        ui.add_space(30.0);

        ui.label(RichText::new(WAVEFORM).size(32.0).color(text_disabled));

        ui.add_space(8.0);
        ui.label(RichText::new("Timeline").size(13.0).color(text_muted));

        ui.add_space(4.0);
        ui.label(RichText::new(message).size(10.0).color(text_disabled));
    });
}

/// Calculate appropriate tick interval based on zoom level
fn calculate_tick_interval(view_duration: f32, width: f32) -> f32 {
    if width <= 0.0 || view_duration <= 0.0 {
        return 1.0;
    }

    let pixels_per_second = width / view_duration;
    let target_pixels = 50.0;
    let ideal_interval = target_pixels / pixels_per_second;

    let intervals = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 60.0];

    for &interval in &intervals {
        if interval >= ideal_interval {
            return interval;
        }
    }

    60.0
}

/// Format time value for display
fn format_time(seconds: f32) -> String {
    if seconds < 0.0 {
        return "0.0s".to_string();
    }
    if seconds < 60.0 {
        format!("{:.1}s", seconds)
    } else {
        let mins = (seconds / 60.0).floor() as i32;
        let secs = seconds % 60.0;
        format!("{}:{:04.1}", mins, secs)
    }
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
