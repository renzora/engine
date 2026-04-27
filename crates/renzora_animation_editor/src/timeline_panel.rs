//! Timeline panel — production-ready animation timeline with scrubber, keyframe
//! display, clip dropdown, transport controls, and per-track keyframe visualization.
//!
//! Panel id: `"timeline"` (matches layout_animation() in layouts.rs).

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_animation::{AnimClip, AnimatorComponent};
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::AnimEditorAction;
use crate::AnimationEditorState;

/// Color constants for keyframe channels.
const COLOR_TRANSLATION: egui::Color32 = egui::Color32::from_rgb(100, 149, 237); // blue
const COLOR_ROTATION: egui::Color32 = egui::Color32::from_rgb(120, 200, 120); // green
const COLOR_SCALE: egui::Color32 = egui::Color32::from_rgb(200, 120, 120); // red
const COLOR_PLAYHEAD: egui::Color32 = egui::Color32::from_rgb(255, 80, 80); // red playhead

const HEADER_WIDTH: f32 = 180.0;
const RULER_HEIGHT: f32 = 24.0;

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
             selected_clip, timeline_zoom, timeline_scroll, snap_enabled, track_height) =
            if let Some(s) = editor_state {
                (s.scrub_time, s.is_previewing, s.preview_speed, s.preview_looping,
                 s.selected_entity, s.selected_clip.clone(), s.timeline_zoom,
                 s.timeline_scroll, s.snap_enabled, s.track_height)
            } else {
                (0.0, false, 1.0, true, None, None, 100.0, 0.0, true, 22.0)
            };

        // ── Toolbar ──
        self.render_toolbar(
            ui, text_color, muted_color, accent_color, border_color,
            is_previewing, preview_speed, preview_looping, scrub_time,
            snap_enabled, timeline_zoom, track_height, selected_entity,
            selected_clip.as_deref(), world,
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

        // ── Range overview bar (DAW-style mini-map) ──
        self.render_range_overview(
            ui,
            border_color,
            surface_color,
            accent_color,
            muted_color,
            clip_duration,
            timeline_zoom,
            timeline_scroll,
            scrub_time,
            clip_data.as_ref(),
        );

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
                                        ui.set_height((track_height - 4.0).max(10.0));
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
                                    egui::vec2(timeline_width, track_height),
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

                // Scroll with mouse wheel — only when the pointer is actually
                // over the timeline area. Reading `smooth_scroll_delta`
                // unconditionally consumed scroll events from anywhere in the
                // app, and let vertical scroll momentum leak horizontally into
                // the timeline pan.
                let pointer_in_timeline = ui
                    .ctx()
                    .pointer_hover_pos()
                    .map(|p| ui.max_rect().contains(p))
                    .unwrap_or(false);
                if pointer_in_timeline {
                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.ctrl || modifiers.command {
                        // Ctrl+wheel = zoom.
                        if scroll_delta.y.abs() > 0.1 {
                            let new_zoom =
                                (timeline_zoom + scroll_delta.y * 0.5).clamp(20.0, 2000.0);
                            self.push_action(AnimEditorAction::SetTimelineZoom(new_zoom));
                        }
                    } else {
                        // Only horizontal deltas pan. Shift+wheel converts a
                        // vertical wheel into horizontal pan. A plain vertical
                        // wheel is reserved for the ScrollArea beneath and
                        // MUST NOT leak into the timeline scroll.
                        let effective_dx = if scroll_delta.x.abs() > 0.1 {
                            scroll_delta.x
                        } else if modifiers.shift && scroll_delta.y.abs() > 0.1 {
                            scroll_delta.y
                        } else {
                            0.0
                        };
                        if effective_dx.abs() > 0.1 {
                            let scroll_amount = -effective_dx / timeline_zoom;
                            let new_scroll = (timeline_scroll + scroll_amount).max(0.0);
                            self.push_action(AnimEditorAction::SetTimelineScroll(new_scroll));
                        }
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
        track_height: f32,
        selected_entity: Option<Entity>,
        selected_clip: Option<&str>,
        world: &World,
    ) {
        let _ = timeline_zoom; // kept for future use
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
                    // Preset buttons — now real framed buttons so they feel clickable.
                    for &speed in &[0.25f32, 0.5, 1.0, 2.0, 4.0] {
                        let label = format!("{:.2}x", speed);
                        let is_active = (preview_speed - speed).abs() < 0.01;
                        let color = if is_active { accent_color } else { text_color };
                        let mut btn = egui::Button::new(
                            egui::RichText::new(&label).size(10.0).color(color),
                        )
                        .min_size(egui::vec2(34.0, 18.0));
                        if is_active {
                            btn = btn.fill(egui::Color32::from_rgba_unmultiplied(
                                accent_color.r(),
                                accent_color.g(),
                                accent_color.b(),
                                40,
                            ));
                        }
                        if ui.add(btn).clicked() {
                            self.push_action(AnimEditorAction::SetPreviewSpeed(speed));
                        }
                    }
                    // Manual speed entry.
                    let mut custom = preview_speed;
                    if ui
                        .add(
                            egui::DragValue::new(&mut custom)
                                .speed(0.05)
                                .range(0.05..=10.0)
                                .suffix("x")
                                .fixed_decimals(2),
                        )
                        .changed()
                    {
                        self.push_action(AnimEditorAction::SetPreviewSpeed(custom));
                    }

                    ui.separator();

                    // ── Row-height slider ──
                    ui.label(egui::RichText::new("Row").size(10.0).color(muted_color));
                    let mut h = track_height;
                    if ui
                        .add(
                            egui::DragValue::new(&mut h)
                                .speed(0.5)
                                .range(14.0..=96.0)
                                .fixed_decimals(0)
                                .suffix("px"),
                        )
                        .changed()
                    {
                        self.push_action(AnimEditorAction::SetTrackHeight(h));
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

    /// Render a compact "mini-map" above the timeline: shows the full clip
    /// duration with a draggable window representing the visible range. Drag
    /// the middle to pan; drag either end handle to zoom; double-click to fit.
    #[allow(clippy::too_many_arguments)]
    fn render_range_overview(
        &self,
        ui: &mut egui::Ui,
        border_color: egui::Color32,
        surface_color: egui::Color32,
        accent_color: egui::Color32,
        muted_color: egui::Color32,
        clip_duration: f32,
        timeline_zoom: f32,
        timeline_scroll: f32,
        scrub_time: f32,
        clip_data: Option<&AnimClip>,
    ) {
        const OVERVIEW_HEIGHT: f32 = 26.0;
        const HANDLE_WIDTH: f32 = 10.0;
        // Inset from the side of the panel so the end-handles don't collide
        // with the dockable panel's own resize gutter.
        const SIDE_INSET: f32 = 12.0;

        if clip_duration <= 0.0 {
            return;
        }

        let avail = ui.available_width();
        let (rect, _bg_response) = ui.allocate_exact_size(
            egui::vec2(avail, OVERVIEW_HEIGHT),
            egui::Sense::hover(),
        );

        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 3.0, surface_color);
        painter.rect_stroke(rect, 3.0, egui::Stroke::new(0.5, border_color), egui::StrokeKind::Inside);

        // Inner rect used for time → pixel math (insets keep handles clear of
        // the panel's edge resize gutter).
        let inner = egui::Rect::from_min_max(
            egui::pos2(rect.left() + SIDE_INSET, rect.top() + 3.0),
            egui::pos2(rect.right() - SIDE_INSET, rect.bottom() - 3.0),
        );
        let pps_overview = (inner.width() / clip_duration).max(0.001);

        // Faint keyframe ticks across the full clip.
        if let Some(clip) = clip_data {
            let tick_color = egui::Color32::from_rgba_unmultiplied(
                muted_color.r(), muted_color.g(), muted_color.b(), 80,
            );
            for track in &clip.tracks {
                for (t, _) in &track.translations {
                    let x = inner.left() + *t * pps_overview;
                    painter.line_segment(
                        [egui::pos2(x, inner.top() + 2.0), egui::pos2(x, inner.bottom() - 2.0)],
                        egui::Stroke::new(0.5, tick_color),
                    );
                }
            }
        }

        // Visible window in pixel coords.
        let visible_start_s = timeline_scroll.max(0.0);
        let visible_duration_s = if timeline_zoom > 0.0 {
            (inner.width() / timeline_zoom).min(clip_duration)
        } else {
            clip_duration
        };
        let visible_end_s = (visible_start_s + visible_duration_s).min(clip_duration);
        let win_left = inner.left() + visible_start_s * pps_overview;
        let win_right = inner.left() + visible_end_s * pps_overview;
        let window_rect = egui::Rect::from_min_max(
            egui::pos2(win_left, inner.top()),
            egui::pos2(win_right, inner.bottom()),
        );

        // Playhead line (behind the window).
        let ph_x = inner.left() + scrub_time.clamp(0.0, clip_duration) * pps_overview;
        painter.line_segment(
            [egui::pos2(ph_x, rect.top() + 2.0), egui::pos2(ph_x, rect.bottom() - 2.0)],
            egui::Stroke::new(1.0, COLOR_PLAYHEAD),
        );

        // Window body.
        let fill = egui::Color32::from_rgba_unmultiplied(
            accent_color.r(), accent_color.g(), accent_color.b(), 48,
        );
        painter.rect_filled(window_rect, 2.0, fill);
        painter.rect_stroke(
            window_rect,
            2.0,
            egui::Stroke::new(1.0, accent_color),
            egui::StrokeKind::Inside,
        );

        // Three separate interactable rects: left handle, right handle, center.
        let left_handle = egui::Rect::from_min_max(
            egui::pos2(window_rect.left() - HANDLE_WIDTH * 0.5, window_rect.top()),
            egui::pos2(window_rect.left() + HANDLE_WIDTH * 0.5, window_rect.bottom()),
        );
        let right_handle = egui::Rect::from_min_max(
            egui::pos2(window_rect.right() - HANDLE_WIDTH * 0.5, window_rect.top()),
            egui::pos2(window_rect.right() + HANDLE_WIDTH * 0.5, window_rect.bottom()),
        );
        let center_rect = egui::Rect::from_min_max(
            egui::pos2(left_handle.right(), window_rect.top()),
            egui::pos2(right_handle.left(), window_rect.bottom()),
        );

        // Paint handle bars.
        painter.rect_filled(left_handle, 2.0, accent_color);
        painter.rect_filled(right_handle, 2.0, accent_color);
        // Inner grip dots on handles for visual affordance.
        for handle in [left_handle, right_handle] {
            let cx = handle.center().x;
            let cy = handle.center().y;
            for dy in [-4.0, 0.0, 4.0] {
                painter.circle_filled(egui::pos2(cx, cy + dy), 1.0, egui::Color32::WHITE);
            }
        }

        // Left handle
        let left_id = ui.id().with("overview_left_handle");
        let left_resp = ui.interact(left_handle, left_id, egui::Sense::drag());
        if left_resp.hovered() || left_resp.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
        }
        if left_resp.dragged() {
            let ds = left_resp.drag_delta().x / pps_overview;
            let new_start = (visible_start_s + ds).clamp(0.0, visible_end_s - 0.05);
            let new_duration = (visible_end_s - new_start).max(0.05);
            let new_zoom = (inner.width() / new_duration).clamp(20.0, 2000.0);
            self.push_action(AnimEditorAction::SetTimelineScroll(new_start));
            self.push_action(AnimEditorAction::SetTimelineZoom(new_zoom));
        }

        // Right handle
        let right_id = ui.id().with("overview_right_handle");
        let right_resp = ui.interact(right_handle, right_id, egui::Sense::drag());
        if right_resp.hovered() || right_resp.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
        }
        if right_resp.dragged() {
            let ds = right_resp.drag_delta().x / pps_overview;
            let new_end = (visible_end_s + ds).clamp(visible_start_s + 0.05, clip_duration);
            let new_duration = (new_end - visible_start_s).max(0.05);
            let new_zoom = (inner.width() / new_duration).clamp(20.0, 2000.0);
            self.push_action(AnimEditorAction::SetTimelineZoom(new_zoom));
        }

        // Center (pan)
        let center_id = ui.id().with("overview_window_center");
        let center_resp = ui.interact(center_rect, center_id, egui::Sense::click_and_drag());
        if center_resp.hovered() || center_resp.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Move);
        }
        if center_resp.dragged() {
            let ds = center_resp.drag_delta().x / pps_overview;
            let new_start = (visible_start_s + ds)
                .clamp(0.0, (clip_duration - visible_duration_s).max(0.0));
            self.push_action(AnimEditorAction::SetTimelineScroll(new_start));
        }

        // Click outside the window → jump scrub to that time.
        let outside_id = ui.id().with("overview_outside_click");
        let outside_resp = ui.interact(rect, outside_id, egui::Sense::click());
        if outside_resp.clicked() {
            if let Some(pos) = outside_resp.interact_pointer_pos() {
                if !window_rect.contains(pos) {
                    let time = ((pos.x - inner.left()) / pps_overview).clamp(0.0, clip_duration);
                    self.push_action(AnimEditorAction::SetScrubTime(time));
                }
            }
        }
        if outside_resp.double_clicked() {
            let fit_zoom = (inner.width() / clip_duration).clamp(20.0, 2000.0);
            self.push_action(AnimEditorAction::SetTimelineZoom(fit_zoom));
            self.push_action(AnimEditorAction::SetTimelineScroll(0.0));
        }

        ui.add_space(2.0);
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
        // Scale keyframes + vertical spacing with the row height so a bigger
        // track height actually gives bigger, more readable markers.
        let kf_size = (rect.height() * 0.38).clamp(4.0, 18.0);
        let row_offset = (rect.height() * 0.26).min(14.0);

        for &(time, _) in &track.translations {
            self.paint_diamond(painter, rect, time, cy - row_offset, zoom, scroll, kf_size, COLOR_TRANSLATION);
        }
        for &(time, _) in &track.rotations {
            self.paint_diamond(painter, rect, time, cy, zoom, scroll, kf_size, COLOR_ROTATION);
        }
        for &(time, _) in &track.scales {
            self.paint_diamond(painter, rect, time, cy + row_offset, zoom, scroll, kf_size, COLOR_SCALE);
        }
    }

    /// Paint a single keyframe diamond.
    #[allow(clippy::too_many_arguments)]
    fn paint_diamond(
        &self,
        painter: &egui::Painter,
        clip_rect: egui::Rect,
        time: f32,
        cy: f32,
        zoom: f32,
        scroll: f32,
        size: f32,
        color: egui::Color32,
    ) {
        let x = clip_rect.left() + (time - scroll) * zoom;
        if x < clip_rect.left() - size || x > clip_rect.right() + size {
            return;
        }

        let half = size * 0.5;
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(x, cy - half),
                egui::pos2(x + half, cy),
                egui::pos2(x, cy + half),
                egui::pos2(x - half, cy),
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
