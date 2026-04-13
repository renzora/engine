//! Timeline panel — production-ready animation timeline with scrubber, keyframe
//! display, clip dropdown, transport controls, and per-track keyframe visualization.
//!
//! Panel id: `"timeline"` (matches layout_animation() in layouts.rs).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::{AnimClip, AnimatorComponent};
use renzora_editor_framework::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::AnimEditorAction;
use crate::AnimationEditorState;

/// Color constants for keyframe channels.
const COLOR_TRANSLATION: egui::Color32 = egui::Color32::from_rgb(100, 149, 237); // blue
const COLOR_ROTATION: egui::Color32 = egui::Color32::from_rgb(120, 200, 120); // green
const COLOR_SCALE: egui::Color32 = egui::Color32::from_rgb(200, 120, 120); // red
const COLOR_PLAYHEAD: egui::Color32 = egui::Color32::from_rgb(255, 80, 80); // red playhead

const TRACK_HEIGHT: f32 = 22.0;
const HEADER_WIDTH: f32 = 180.0;
const RULER_HEIGHT: f32 = 24.0;
const KEYFRAME_SIZE: f32 = 6.0;

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
        let theme = world.get_resource::<ThemeManager>();
        let (text_color, muted_color, accent_color, surface_color, border_color, _bg_color,
             row_even, row_odd) =
            if let Some(tm) = theme {
                let t = &tm.active_theme;
                (
                    t.text.primary.to_color32(),
                    t.text.muted.to_color32(),
                    t.semantic.accent.to_color32(),
                    t.surfaces.faint.to_color32(),
                    t.widgets.border.to_color32(),
                    t.surfaces.panel.to_color32(),
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
                    egui::Color32::from_rgb(24, 25, 28),
                    egui::Color32::from_rgb(30, 31, 36),
                    egui::Color32::from_rgb(34, 35, 40),
                )
            };

        let editor_state = world.get_resource::<AnimationEditorState>();
        let (scrub_time, is_previewing, preview_speed, preview_looping, selected_entity,
             selected_clip, timeline_zoom, timeline_scroll, snap_enabled) =
            if let Some(s) = editor_state {
                (s.scrub_time, s.is_previewing, s.preview_speed, s.preview_looping,
                 s.selected_entity, s.selected_clip.clone(), s.timeline_zoom,
                 s.timeline_scroll, s.snap_enabled)
            } else {
                (0.0, false, 1.0, true, None, None, 100.0, 0.0, true)
            };

        // ── Toolbar ──
        self.render_toolbar(
            ui, text_color, muted_color, accent_color, border_color,
            is_previewing, preview_speed, preview_looping, scrub_time,
            snap_enabled, timeline_zoom, selected_entity, selected_clip.as_deref(),
            world,
        );

        ui.add_space(1.0);

        // ── Main content ──
        let Some(entity) = selected_entity else {
            self.empty_state(ui, muted_color, "Select an entity with animations");
            return;
        };

        let Some(animator) = world.get::<AnimatorComponent>(entity) else {
            self.empty_state(ui, muted_color, "No AnimatorComponent on selected entity");
            return;
        };

        if animator.clips.is_empty() {
            self.empty_state(ui, muted_color, "No animation clips assigned");
            return;
        }

        // Load the selected clip's .anim data from disk
        let clip_data = self.load_clip_data(selected_clip.as_deref(), &animator, world);
        let clip_duration = clip_data.as_ref().map(|c| c.duration).unwrap_or(2.0);

        // ── Timeline area (header + ruler + tracks) ──
        let available = ui.available_size();

        // Auto-fit zoom: when a new clip is selected, fit its duration to ~90% of timeline width
        let timeline_area_width = (available.x - HEADER_WIDTH - 2.0).max(100.0);
        let auto_fit_clip = editor_state.and_then(|s| s.auto_fit_clip.clone());
        if clip_data.is_some() && clip_duration > 0.0 {
            let needs_auto_fit = auto_fit_clip.as_deref() != selected_clip.as_deref();
            if needs_auto_fit {
                let ideal_zoom = (timeline_area_width * 0.90) / clip_duration;
                let ideal_zoom = ideal_zoom.clamp(20.0, 500.0);
                self.push_action(AnimEditorAction::SetTimelineZoom(ideal_zoom));
                self.push_action(AnimEditorAction::SetTimelineScroll(0.0));
                self.push_action(AnimEditorAction::AutoFitDone(
                    selected_clip.clone().unwrap_or_default(),
                ));
            }
        }

        // Split: left track headers | right timeline area
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // ── Left: track list headers ──
            ui.vertical(|ui| {
                ui.set_width(HEADER_WIDTH);
                ui.set_min_height(available.y - 4.0);

                // Ruler spacer
                egui::Frame::new()
                    .fill(surface_color)
                    .stroke(egui::Stroke::new(0.5, border_color))
                    .show(ui, |ui| {
                        ui.set_width(HEADER_WIDTH - 2.0);
                        ui.set_height(RULER_HEIGHT);
                        ui.horizontal_centered(|ui| {
                            ui.label(egui::RichText::new("Track").size(10.0).color(muted_color));
                        });
                    });

                // Track headers
                egui::ScrollArea::vertical()
                    .id_salt("timeline_track_headers")
                    .show(ui, |ui| {
                        if let Some(ref clip) = clip_data {
                            for (i, track) in clip.tracks.iter().enumerate() {
                                let row_bg = if i % 2 == 0 { row_even } else { row_odd };
                                egui::Frame::new()
                                    .fill(row_bg)
                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                    .show(ui, |ui| {
                                        ui.set_width(HEADER_WIDTH - 14.0);
                                        ui.set_height(TRACK_HEIGHT - 4.0);
                                        ui.horizontal_centered(|ui| {
                                            ui.label(egui::RichText::new(egui_phosphor::regular::BONE)
                                                .size(10.0).color(muted_color));
                                            ui.add(egui::Label::new(
                                                egui::RichText::new(&track.bone_name)
                                                    .size(11.0).color(text_color)
                                            ).truncate());

                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // Channel indicators
                                                let has_s = !track.scales.is_empty();
                                                let has_r = !track.rotations.is_empty();
                                                let has_t = !track.translations.is_empty();

                                                let s_color = if has_s { COLOR_SCALE } else { muted_color };
                                                let r_color = if has_r { COLOR_ROTATION } else { muted_color };
                                                let t_color = if has_t { COLOR_TRANSLATION } else { muted_color };

                                                ui.label(egui::RichText::new("S").size(9.0).color(s_color));
                                                ui.label(egui::RichText::new("R").size(9.0).color(r_color));
                                                ui.label(egui::RichText::new("T").size(9.0).color(t_color));
                                            });
                                        });
                                    });
                            }
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(egui::RichText::new("No clip selected")
                                    .size(11.0).color(muted_color));
                            });
                        }
                    });
            });

            // ── Vertical separator ──
            let sep_rect = ui.allocate_exact_size(
                egui::vec2(1.0, available.y - 4.0), egui::Sense::hover()
            ).0;
            ui.painter().rect_filled(sep_rect, 0.0, border_color);

            // ── Right: ruler + keyframe lanes ──
            ui.vertical(|ui| {
                let timeline_width = (ui.available_width() - 2.0).max(100.0);

                // ── Time ruler ──
                let (ruler_rect, ruler_response) = ui.allocate_exact_size(
                    egui::vec2(timeline_width, RULER_HEIGHT),
                    egui::Sense::click_and_drag(),
                );

                self.paint_ruler(
                    ui.painter(), ruler_rect, clip_duration, timeline_zoom,
                    timeline_scroll, text_color, muted_color, border_color, surface_color,
                );

                // Scrub on ruler click/drag
                if ruler_response.clicked() || ruler_response.dragged() {
                    if let Some(pos) = ruler_response.interact_pointer_pos() {
                        let local_x = pos.x - ruler_rect.left();
                        let time = (local_x / timeline_zoom) + timeline_scroll;
                        let time = time.clamp(0.0, clip_duration);
                        self.push_action(AnimEditorAction::SetScrubTime(time));
                    }
                }

                // ── Keyframe lanes ──
                egui::ScrollArea::vertical()
                    .id_salt("timeline_keyframe_lanes")
                    .show(ui, |ui| {
                        if let Some(ref clip) = clip_data {
                            for (i, track) in clip.tracks.iter().enumerate() {
                                let row_bg = if i % 2 == 0 { row_even } else { row_odd };

                                let (track_rect, track_response) = ui.allocate_exact_size(
                                    egui::vec2(timeline_width, TRACK_HEIGHT),
                                    egui::Sense::click_and_drag(),
                                );

                                let painter = ui.painter_at(track_rect);

                                // Row background
                                painter.rect_filled(track_rect, 0.0, row_bg);

                                // Draw keyframe diamonds
                                self.paint_keyframes(
                                    &painter, track_rect, track, clip_duration,
                                    timeline_zoom, timeline_scroll,
                                );

                                // Click on track to scrub
                                if track_response.clicked() || track_response.dragged() {
                                    if let Some(pos) = track_response.interact_pointer_pos() {
                                        let local_x = pos.x - track_rect.left();
                                        let time = (local_x / timeline_zoom) + timeline_scroll;
                                        let time = time.clamp(0.0, clip_duration);
                                        self.push_action(AnimEditorAction::SetScrubTime(time));
                                    }
                                }
                            }
                        }
                    });

                // ── Playhead overlay (drawn over everything) ──
                let timeline_top = ruler_rect.top();
                let timeline_bottom = ui.min_rect().bottom();
                let playhead_x = ruler_rect.left()
                    + (scrub_time - timeline_scroll) * timeline_zoom;

                if playhead_x >= ruler_rect.left() && playhead_x <= ruler_rect.right() {
                    let painter = ui.painter();

                    // Playhead line
                    painter.line_segment(
                        [
                            egui::pos2(playhead_x, timeline_top),
                            egui::pos2(playhead_x, timeline_bottom),
                        ],
                        egui::Stroke::new(1.5, COLOR_PLAYHEAD),
                    );

                    // Playhead triangle at top
                    let tri_size = 6.0;
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            egui::pos2(playhead_x, timeline_top + RULER_HEIGHT - 2.0),
                            egui::pos2(playhead_x - tri_size, timeline_top + RULER_HEIGHT - tri_size - 2.0),
                            egui::pos2(playhead_x + tri_size, timeline_top + RULER_HEIGHT - tri_size - 2.0),
                        ],
                        COLOR_PLAYHEAD,
                        egui::Stroke::NONE,
                    ));
                }

                // Scroll with mouse wheel
                let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                if scroll_delta.x.abs() > 0.1 || scroll_delta.y.abs() > 0.1 {
                    // Ctrl+scroll = zoom, regular scroll = pan
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.ctrl || modifiers.command {
                        let new_zoom = (timeline_zoom + scroll_delta.y * 0.5).clamp(20.0, 500.0);
                        self.push_action(AnimEditorAction::SetTimelineZoom(new_zoom));
                    } else {
                        let scroll_amount = -scroll_delta.y / timeline_zoom;
                        let new_scroll = (timeline_scroll + scroll_amount).max(0.0);
                        self.push_action(AnimEditorAction::SetTimelineScroll(new_scroll));
                    }
                }
            });
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn min_size(&self) -> [f32; 2] {
        [400.0, 120.0]
    }
}

impl TimelinePanel {
    fn empty_state(&self, ui: &mut egui::Ui, color: egui::Color32, msg: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.label(egui::RichText::new(egui_phosphor::regular::FILM_STRIP)
                .size(24.0).color(color));
            ui.add_space(4.0);
            ui.label(egui::RichText::new(msg).size(12.0).color(color));
        });
    }

    /// Render the top toolbar with transport, clip dropdown, speed, snap, zoom.
    #[allow(clippy::too_many_arguments)]
    fn render_toolbar(
        &self,
        ui: &mut egui::Ui,
        text_color: egui::Color32,
        muted_color: egui::Color32,
        accent_color: egui::Color32,
        border_color: egui::Color32,
        is_previewing: bool,
        preview_speed: f32,
        preview_looping: bool,
        scrub_time: f32,
        snap_enabled: bool,
        timeline_zoom: f32,
        selected_entity: Option<Entity>,
        selected_clip: Option<&str>,
        world: &World,
    ) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(0.5, border_color))
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.style_mut().spacing.button_padding = egui::vec2(4.0, 2.0);

                    // ── Transport controls ──
                    // Skip to start
                    if ui.button(egui::RichText::new(egui_phosphor::regular::SKIP_BACK)
                        .size(14.0).color(text_color)).clicked() {
                        self.push_action(AnimEditorAction::SetScrubTime(0.0));
                    }

                    // Step back
                    if ui.button(egui::RichText::new(egui_phosphor::regular::CARET_LEFT)
                        .size(14.0).color(text_color)).clicked() {
                        let new_time = (scrub_time - 1.0 / 30.0).max(0.0);
                        self.push_action(AnimEditorAction::SetScrubTime(new_time));
                    }

                    // Play/Pause
                    let play_icon = if is_previewing {
                        egui_phosphor::regular::PAUSE
                    } else {
                        egui_phosphor::regular::PLAY
                    };
                    let play_color = if is_previewing { accent_color } else { text_color };
                    if ui.button(egui::RichText::new(play_icon).size(14.0).color(play_color)).clicked() {
                        self.push_action(AnimEditorAction::TogglePreview);
                    }

                    // Stop
                    if ui.button(egui::RichText::new(egui_phosphor::regular::STOP)
                        .size(14.0).color(text_color)).clicked() {
                        self.push_action(AnimEditorAction::StopPreview);
                    }

                    // Step forward
                    if ui.button(egui::RichText::new(egui_phosphor::regular::CARET_RIGHT)
                        .size(14.0).color(text_color)).clicked() {
                        let new_time = scrub_time + 1.0 / 30.0;
                        self.push_action(AnimEditorAction::SetScrubTime(new_time));
                    }

                    // Skip to end
                    if ui.button(egui::RichText::new(egui_phosphor::regular::SKIP_FORWARD)
                        .size(14.0).color(text_color)).clicked() {
                        // Will be clamped to clip duration
                        self.push_action(AnimEditorAction::SetScrubTime(f32::MAX));
                    }

                    ui.separator();

                    // ── Loop toggle ──
                    let loop_color = if preview_looping { accent_color } else { muted_color };
                    if ui.button(egui::RichText::new(egui_phosphor::regular::REPEAT)
                        .size(13.0).color(loop_color)).on_hover_text("Toggle loop").clicked() {
                        self.push_action(AnimEditorAction::SetPreviewLooping(!preview_looping));
                    }

                    ui.separator();

                    // ── Clip dropdown ──
                    ui.label(egui::RichText::new(egui_phosphor::regular::FILM_STRIP)
                        .size(12.0).color(muted_color));

                    let clip_label = selected_clip.unwrap_or("Select clip...");
                    let _combo_response = egui::ComboBox::from_id_salt("timeline_clip_selector")
                        .selected_text(egui::RichText::new(clip_label).size(11.0).color(text_color))
                        .width(140.0)
                        .show_ui(ui, |ui| {
                            if let Some(entity) = selected_entity {
                                if let Some(animator) = world.get::<AnimatorComponent>(entity) {
                                    for slot in &animator.clips {
                                        let is_selected = selected_clip == Some(slot.name.as_str());
                                        let label = if animator.default_clip.as_deref() == Some(&slot.name) {
                                            format!("{} (default)", slot.name)
                                        } else {
                                            slot.name.clone()
                                        };
                                        if ui.selectable_label(is_selected,
                                            egui::RichText::new(&label).size(11.0)
                                        ).clicked() {
                                            self.push_action(AnimEditorAction::SelectClip(Some(slot.name.clone())));

                                            // Auto-play the selected clip
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
                                    }
                                }
                            }
                        });

                    ui.separator();

                    // ── Speed selector ──
                    ui.label(egui::RichText::new("Speed").size(10.0).color(muted_color));
                    for &speed in &[0.25f32, 0.5, 1.0, 2.0] {
                        let label = format!("{:.2}x", speed);
                        let color = if (preview_speed - speed).abs() < 0.01 { accent_color } else { muted_color };
                        if ui.add(egui::Button::new(
                            egui::RichText::new(&label).size(10.0).color(color)
                        ).frame(false)).clicked() {
                            self.push_action(AnimEditorAction::SetPreviewSpeed(speed));
                        }
                    }

                    ui.separator();

                    // ── Snap toggle ──
                    let snap_color = if snap_enabled { accent_color } else { muted_color };
                    if ui.button(egui::RichText::new(egui_phosphor::regular::MAGNET_STRAIGHT)
                        .size(13.0).color(snap_color)).on_hover_text("Snap to frames").clicked() {
                        self.push_action(AnimEditorAction::SetSnapEnabled(!snap_enabled));
                    }

                    // ── Spacer → right side ──
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Time display
                        let mins = (scrub_time / 60.0) as u32;
                        let secs = scrub_time % 60.0;
                        let frame = (scrub_time * 30.0) as u32;
                        ui.label(egui::RichText::new(
                            format!("{:02}:{:05.2}  f{}", mins, secs, frame)
                        ).color(text_color).monospace().size(11.0));

                        ui.separator();

                        // Zoom controls
                        if ui.button(egui::RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS_PLUS)
                            .size(12.0).color(muted_color)).on_hover_text("Zoom in").clicked() {
                            self.push_action(AnimEditorAction::SetTimelineZoom(
                                (timeline_zoom * 1.25).min(500.0)
                            ));
                        }
                        ui.label(egui::RichText::new(format!("{:.0}%", timeline_zoom))
                            .size(10.0).color(muted_color));
                        if ui.button(egui::RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS_MINUS)
                            .size(12.0).color(muted_color)).on_hover_text("Zoom out").clicked() {
                            self.push_action(AnimEditorAction::SetTimelineZoom(
                                (timeline_zoom * 0.8).max(20.0)
                            ));
                        }
                    });
                });
            });
    }

    /// Paint the time ruler with tick marks and labels.
    fn paint_ruler(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        duration: f32,
        zoom: f32,
        scroll: f32,
        text_color: egui::Color32,
        muted_color: egui::Color32,
        border_color: egui::Color32,
        bg_color: egui::Color32,
    ) {
        // Background
        painter.rect_filled(rect, 0.0, bg_color);
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(0.5, border_color),
        );

        // Determine tick spacing based on zoom
        let base_interval = if zoom >= 200.0 {
            0.1 // 100ms ticks at high zoom
        } else if zoom >= 80.0 {
            0.5 // 500ms ticks
        } else {
            1.0 // 1s ticks
        };

        let start_time = (scroll / base_interval).floor() as i32;
        let end_time = ((scroll + rect.width() / zoom) / base_interval).ceil() as i32 + 1;

        for i in start_time..end_time {
            let time = i as f32 * base_interval;
            if time < 0.0 || time > duration {
                continue;
            }

            let x = rect.left() + (time - scroll) * zoom;
            if x < rect.left() || x > rect.right() {
                continue;
            }

            let is_major = if base_interval < 1.0 {
                (time * 10.0).round() % 10.0 < 0.01 // Major at whole seconds
            } else {
                (time % 5.0).abs() < 0.01 // Major every 5 seconds
            };

            let tick_height = if is_major { 10.0 } else { 5.0 };
            let tick_color = if is_major { text_color } else { muted_color };

            // Tick line
            painter.line_segment(
                [
                    egui::pos2(x, rect.bottom() - tick_height),
                    egui::pos2(x, rect.bottom()),
                ],
                egui::Stroke::new(0.5, tick_color),
            );

            // Label on major ticks
            if is_major {
                let label = if time >= 60.0 {
                    format!("{}:{:04.1}", (time / 60.0) as u32, time % 60.0)
                } else if time == time.floor() {
                    format!("{:.0}s", time)
                } else {
                    format!("{:.1}s", time)
                };
                painter.text(
                    egui::pos2(x + 2.0, rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::proportional(9.0),
                    muted_color,
                );
            }
        }

        // Duration end marker
        let end_x = rect.left() + (duration - scroll) * zoom;
        if end_x >= rect.left() && end_x <= rect.right() {
            painter.line_segment(
                [egui::pos2(end_x, rect.top()), egui::pos2(end_x, rect.bottom())],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 40)),
            );
        }
    }

    /// Paint keyframe diamonds on a track row.
    fn paint_keyframes(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        track: &renzora_animation::BoneTrack,
        _duration: f32,
        zoom: f32,
        scroll: f32,
    ) {
        let cy = rect.center().y;

        // Draw translation keyframes
        for &(time, _) in &track.translations {
            self.paint_diamond(painter, rect, time, cy - 4.0, zoom, scroll, COLOR_TRANSLATION);
        }

        // Draw rotation keyframes
        for &(time, _) in &track.rotations {
            self.paint_diamond(painter, rect, time, cy, zoom, scroll, COLOR_ROTATION);
        }

        // Draw scale keyframes
        for &(time, _) in &track.scales {
            self.paint_diamond(painter, rect, time, cy + 4.0, zoom, scroll, COLOR_SCALE);
        }
    }

    /// Paint a single keyframe diamond.
    fn paint_diamond(
        &self,
        painter: &egui::Painter,
        clip_rect: egui::Rect,
        time: f32,
        cy: f32,
        zoom: f32,
        scroll: f32,
        color: egui::Color32,
    ) {
        let x = clip_rect.left() + (time - scroll) * zoom;
        if x < clip_rect.left() - KEYFRAME_SIZE || x > clip_rect.right() + KEYFRAME_SIZE {
            return;
        }

        let half = KEYFRAME_SIZE * 0.5;
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(x, cy - half),        // top
                egui::pos2(x + half, cy),         // right
                egui::pos2(x, cy + half),         // bottom
                egui::pos2(x - half, cy),         // left
            ],
            color,
            egui::Stroke::new(0.5, egui::Color32::from_white_alpha(60)),
        ));
    }

    /// Load the .anim clip data from disk for the selected clip.
    fn load_clip_data(
        &self,
        selected_clip: Option<&str>,
        animator: &AnimatorComponent,
        world: &World,
    ) -> Option<AnimClip> {
        let clip_name = selected_clip?;
        let slot = animator.clips.iter().find(|s| s.name == clip_name)?;
        let project = world.get_resource::<renzora::core::CurrentProject>()?;
        let anim_path = project.path.join(&slot.path);
        let content = std::fs::read_to_string(&anim_path).ok()?;
        ron::from_str::<AnimClip>(&content).ok()
    }
}
