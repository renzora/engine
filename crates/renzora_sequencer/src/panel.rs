//! Sequencer panel — multi-track timeline with transport, ruler, clip lanes,
//! and a draggable playhead.
//!
//! Visual structure mirrors the animation `timeline_panel`, but the model is
//! a multi-track Sequence rather than a single AnimClip.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;

use renzora_editor::EditorPanel;
use renzora_theme::ThemeManager;
use renzora_ui::PanelLocation;

use crate::model::TrackKind;
use crate::runtime::{
    push_action, track_clip_views, SequencerAction, SequencerBridge, SequencerState,
};

const HEADER_WIDTH: f32 = 200.0;
const RULER_HEIGHT: f32 = 22.0;
const PLAYHEAD_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 80, 80);

const COLOR_CAMERA: egui::Color32 = egui::Color32::from_rgb(100, 149, 237);
const COLOR_TRANSFORM: egui::Color32 = egui::Color32::from_rgb(120, 200, 120);
const COLOR_MEDIA: egui::Color32 = egui::Color32::from_rgb(220, 140, 60);
const COLOR_MARKER: egui::Color32 = egui::Color32::from_rgb(220, 200, 80);

pub struct SequencerPanel {
    bridge: SequencerBridge,
}

impl SequencerPanel {
    pub fn new(bridge: SequencerBridge) -> Self {
        Self { bridge }
    }
}

impl EditorPanel for SequencerPanel {
    fn id(&self) -> &str {
        "sequencer"
    }

    fn title(&self) -> &str {
        "Sequencer"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FILM_REEL)
    }

    fn min_size(&self) -> [f32; 2] {
        [480.0, 180.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>();
        let (text_color, muted_color, accent_color, surface_color, border_color, row_even, row_odd) =
            if let Some(tm) = theme {
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

        let Some(state) = world.get_resource::<SequencerState>() else {
            ui.label("Sequencer state not initialised yet.");
            return;
        };

        self.toolbar(
            ui,
            state,
            text_color,
            muted_color,
            accent_color,
            border_color,
        );
        ui.add_space(2.0);

        let avail = ui.available_size();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // ── Left: track headers ──
            ui.vertical(|ui| {
                ui.set_width(HEADER_WIDTH);
                ui.set_min_height(avail.y - 4.0);

                egui::Frame::new()
                    .fill(surface_color)
                    .stroke(egui::Stroke::new(0.5, border_color))
                    .show(ui, |ui| {
                        ui.set_width(HEADER_WIDTH - 2.0);
                        ui.set_height(RULER_HEIGHT);
                        ui.horizontal_centered(|ui| {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Tracks").size(10.0).color(muted_color),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button(
                                            egui::RichText::new(regular::PLUS).size(11.0),
                                        )
                                        .on_hover_text("Add track")
                                        .clicked()
                                    {
                                        // Cycle through track types — quick MVP.
                                        let n = state.sequence.tracks.len();
                                        let kind = match n % 3 {
                                            0 => TrackKind::Camera { clips: vec![] },
                                            1 => TrackKind::Transform {
                                                target_tag: "target".into(),
                                                clips: vec![],
                                            },
                                            _ => TrackKind::Marker { clips: vec![] },
                                        };
                                        push_action(&self.bridge, SequencerAction::AddTrack(kind));
                                    }
                                },
                            );
                        });
                    });

                egui::ScrollArea::vertical()
                    .id_salt("seq_track_headers")
                    .show(ui, |ui| {
                        for (i, track) in state.sequence.tracks.iter().enumerate() {
                            let row_bg = if i % 2 == 0 { row_even } else { row_odd };
                            let track_color = track_kind_color(&track.kind);
                            egui::Frame::new()
                                .fill(row_bg)
                                .inner_margin(egui::Margin::symmetric(6, 2))
                                .show(ui, |ui| {
                                    ui.set_width(HEADER_WIDTH - 14.0);
                                    ui.set_height(state.track_height - 4.0);
                                    ui.horizontal_centered(|ui| {
                                        // Color swatch
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(4.0, state.track_height - 10.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(rect, 1.0, track_color);

                                        ui.add_space(4.0);
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&track.name)
                                                    .size(11.0)
                                                    .color(text_color),
                                            )
                                            .truncate(),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let mute_icon = if track.muted {
                                                    regular::SPEAKER_SLASH
                                                } else {
                                                    regular::SPEAKER_HIGH
                                                };
                                                let mute_color = if track.muted {
                                                    muted_color
                                                } else {
                                                    text_color
                                                };
                                                if ui
                                                    .small_button(
                                                        egui::RichText::new(mute_icon)
                                                            .size(11.0)
                                                            .color(mute_color),
                                                    )
                                                    .on_hover_text("Mute track")
                                                    .clicked()
                                                {
                                                    push_action(
                                                        &self.bridge,
                                                        SequencerAction::SetTrackMuted {
                                                            track: i,
                                                            muted: !track.muted,
                                                        },
                                                    );
                                                }
                                                let lock_icon = if track.locked {
                                                    regular::LOCK
                                                } else {
                                                    regular::LOCK_OPEN
                                                };
                                                if ui
                                                    .small_button(
                                                        egui::RichText::new(lock_icon)
                                                            .size(11.0)
                                                            .color(text_color),
                                                    )
                                                    .on_hover_text("Lock track")
                                                    .clicked()
                                                {
                                                    push_action(
                                                        &self.bridge,
                                                        SequencerAction::SetTrackLocked {
                                                            track: i,
                                                            locked: !track.locked,
                                                        },
                                                    );
                                                }
                                                if ui
                                                    .small_button(
                                                        egui::RichText::new(regular::TRASH)
                                                            .size(11.0)
                                                            .color(muted_color),
                                                    )
                                                    .on_hover_text("Delete track")
                                                    .clicked()
                                                {
                                                    push_action(
                                                        &self.bridge,
                                                        SequencerAction::RemoveTrack(i),
                                                    );
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                    });
            });

            // ── Vertical separator ──
            let sep = ui
                .allocate_exact_size(egui::vec2(1.0, avail.y - 4.0), egui::Sense::hover())
                .0;
            ui.painter().rect_filled(sep, 0.0, border_color);

            // ── Right: ruler + clip lanes ──
            ui.vertical(|ui| {
                let timeline_width = (ui.available_width() - 2.0).max(100.0);

                let (ruler_rect, ruler_resp) = ui.allocate_exact_size(
                    egui::vec2(timeline_width, RULER_HEIGHT),
                    egui::Sense::click_and_drag(),
                );
                paint_ruler(
                    ui.painter_at(ruler_rect),
                    ruler_rect,
                    state.sequence.duration,
                    state.timeline_zoom,
                    state.timeline_scroll,
                    text_color,
                    muted_color,
                    border_color,
                    surface_color,
                );

                if ruler_resp.clicked() || ruler_resp.dragged() {
                    if let Some(pos) = ruler_resp.interact_pointer_pos() {
                        let local_x = pos.x - ruler_rect.left();
                        let t = (local_x / state.timeline_zoom) + state.timeline_scroll;
                        push_action(
                            &self.bridge,
                            SequencerAction::SetPlayhead(
                                t.clamp(0.0, state.sequence.duration),
                            ),
                        );
                    }
                }

                // Track lanes
                egui::ScrollArea::vertical()
                    .id_salt("seq_track_lanes")
                    .show(ui, |ui| {
                        for (track_idx, track) in state.sequence.tracks.iter().enumerate() {
                            let row_bg = if track_idx % 2 == 0 { row_even } else { row_odd };
                            let (rect, lane_resp) = ui.allocate_exact_size(
                                egui::vec2(timeline_width, state.track_height),
                                egui::Sense::click_and_drag(),
                            );
                            let painter = ui.painter_at(rect);
                            painter.rect_filled(rect, 0.0, row_bg);

                            // Click on empty lane → scrub.
                            if lane_resp.clicked() {
                                if let Some(pos) = lane_resp.interact_pointer_pos() {
                                    let t = (pos.x - rect.left()) / state.timeline_zoom
                                        + state.timeline_scroll;
                                    push_action(
                                        &self.bridge,
                                        SequencerAction::SetPlayhead(
                                            t.clamp(0.0, state.sequence.duration),
                                        ),
                                    );
                                    push_action(
                                        &self.bridge,
                                        SequencerAction::SelectTrack(Some(track_idx)),
                                    );
                                }
                            }

                            // Draw clips on this lane.
                            self.draw_clip_lane(
                                ui,
                                &painter,
                                rect,
                                track_idx,
                                track,
                                state,
                                accent_color,
                                text_color,
                                muted_color,
                            );
                        }
                    });

                // ── Playhead overlay ──
                let timeline_top = ruler_rect.top();
                let timeline_bottom = ui.min_rect().bottom();
                let playhead_x = ruler_rect.left()
                    + (state.playhead - state.timeline_scroll) * state.timeline_zoom;
                if playhead_x >= ruler_rect.left() && playhead_x <= ruler_rect.right() {
                    let painter = ui.painter();
                    painter.line_segment(
                        [
                            egui::pos2(playhead_x, timeline_top),
                            egui::pos2(playhead_x, timeline_bottom),
                        ],
                        egui::Stroke::new(1.5, PLAYHEAD_COLOR),
                    );
                    let tri = 6.0;
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            egui::pos2(playhead_x, timeline_top + RULER_HEIGHT - 2.0),
                            egui::pos2(
                                playhead_x - tri,
                                timeline_top + RULER_HEIGHT - tri - 2.0,
                            ),
                            egui::pos2(
                                playhead_x + tri,
                                timeline_top + RULER_HEIGHT - tri - 2.0,
                            ),
                        ],
                        PLAYHEAD_COLOR,
                        egui::Stroke::NONE,
                    ));
                }

                // Wheel: ctrl=zoom, shift=pan, plain vertical = (ScrollArea handles).
                let pointer_in = ui
                    .ctx()
                    .pointer_hover_pos()
                    .map(|p| ui.max_rect().contains(p))
                    .unwrap_or(false);
                if pointer_in {
                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.ctrl || modifiers.command {
                        if scroll_delta.y.abs() > 0.1 {
                            push_action(
                                &self.bridge,
                                SequencerAction::SetZoom(
                                    state.timeline_zoom + scroll_delta.y * 0.5,
                                ),
                            );
                        }
                    } else {
                        let dx = if scroll_delta.x.abs() > 0.1 {
                            scroll_delta.x
                        } else if modifiers.shift && scroll_delta.y.abs() > 0.1 {
                            scroll_delta.y
                        } else {
                            0.0
                        };
                        if dx.abs() > 0.1 {
                            let amt = -dx / state.timeline_zoom;
                            push_action(
                                &self.bridge,
                                SequencerAction::SetScroll(state.timeline_scroll + amt),
                            );
                        }
                    }
                }
            });
        });
    }
}

impl SequencerPanel {
    #[allow(clippy::too_many_arguments)]
    fn toolbar(
        &self,
        ui: &mut egui::Ui,
        state: &SequencerState,
        text_color: egui::Color32,
        muted_color: egui::Color32,
        accent_color: egui::Color32,
        border_color: egui::Color32,
    ) {
        egui::Frame::new()
            .stroke(egui::Stroke::new(0.5, border_color))
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.style_mut().spacing.button_padding = egui::vec2(4.0, 2.0);

                    if ui
                        .button(
                            egui::RichText::new(regular::SKIP_BACK)
                                .size(14.0)
                                .color(text_color),
                        )
                        .clicked()
                    {
                        push_action(&self.bridge, SequencerAction::SetPlayhead(0.0));
                    }
                    let icon = if state.playing {
                        regular::PAUSE
                    } else {
                        regular::PLAY
                    };
                    let icon_color = if state.playing { accent_color } else { text_color };
                    if ui
                        .button(egui::RichText::new(icon).size(14.0).color(icon_color))
                        .clicked()
                    {
                        push_action(&self.bridge, SequencerAction::TogglePlay);
                    }
                    if ui
                        .button(
                            egui::RichText::new(regular::STOP)
                                .size(14.0)
                                .color(text_color),
                        )
                        .clicked()
                    {
                        push_action(&self.bridge, SequencerAction::Stop);
                    }
                    if ui
                        .button(
                            egui::RichText::new(regular::SKIP_FORWARD)
                                .size(14.0)
                                .color(text_color),
                        )
                        .clicked()
                    {
                        push_action(
                            &self.bridge,
                            SequencerAction::SetPlayhead(state.sequence.duration),
                        );
                    }

                    ui.separator();

                    let loop_color = if state.looping { accent_color } else { muted_color };
                    if ui
                        .button(
                            egui::RichText::new(regular::REPEAT)
                                .size(13.0)
                                .color(loop_color),
                        )
                        .on_hover_text("Loop")
                        .clicked()
                    {
                        push_action(
                            &self.bridge,
                            SequencerAction::SetLooping(!state.looping),
                        );
                    }

                    ui.separator();

                    // Add camera key at playhead
                    if ui
                        .button(
                            egui::RichText::new(format!("{} Key", regular::DIAMOND))
                                .size(11.0)
                                .color(text_color),
                        )
                        .on_hover_text("Add a camera keyframe at the playhead from the live editor camera")
                        .clicked()
                    {
                        push_action(&self.bridge, SequencerAction::AddCameraKeyAtPlayhead);
                    }

                    // Add marker at playhead
                    if ui
                        .button(
                            egui::RichText::new(format!("{} Mark", regular::FLAG))
                                .size(11.0)
                                .color(text_color),
                        )
                        .on_hover_text("Drop a marker at the playhead")
                        .clicked()
                    {
                        push_action(
                            &self.bridge,
                            SequencerAction::AddMarkerAtPlayhead(format!(
                                "M{:02}",
                                marker_count(state) + 1
                            )),
                        );
                    }

                    ui.separator();

                    if ui
                        .button(
                            egui::RichText::new(format!("{} Bake", regular::FILM_STRIP))
                                .size(11.0)
                                .color(COLOR_MEDIA),
                        )
                        .on_hover_text("Bake the full sequence to a media clip (stub — wire to recorder)")
                        .clicked()
                    {
                        push_action(
                            &self.bridge,
                            SequencerAction::StubBakeRange {
                                from: 0.0,
                                to: state.sequence.duration,
                            },
                        );
                    }

                    ui.separator();

                    // Speed
                    ui.label(egui::RichText::new("Speed").size(10.0).color(muted_color));
                    let mut rate = state.play_rate;
                    if ui
                        .add(
                            egui::DragValue::new(&mut rate)
                                .speed(0.05)
                                .range(0.05..=10.0)
                                .suffix("x")
                                .fixed_decimals(2),
                        )
                        .changed()
                    {
                        push_action(&self.bridge, SequencerAction::SetPlayRate(rate));
                    }

                    ui.separator();

                    // Duration
                    ui.label(egui::RichText::new("Length").size(10.0).color(muted_color));
                    let mut dur = state.sequence.duration;
                    if ui
                        .add(
                            egui::DragValue::new(&mut dur)
                                .speed(0.1)
                                .range(0.1..=600.0)
                                .suffix("s")
                                .fixed_decimals(1),
                        )
                        .changed()
                    {
                        push_action(&self.bridge, SequencerAction::SetSequenceDuration(dur));
                    }

                    // Right side
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let secs = state.playhead;
                            let mins = (secs / 60.0) as u32;
                            let frame = (secs * state.sequence.fps as f32) as u32;
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:02}:{:05.2}  f{}",
                                    mins,
                                    secs % 60.0,
                                    frame
                                ))
                                .color(text_color)
                                .monospace()
                                .size(11.0),
                            );

                            ui.separator();

                            if ui
                                .button(
                                    egui::RichText::new(regular::MAGNIFYING_GLASS_PLUS)
                                        .size(12.0)
                                        .color(muted_color),
                                )
                                .on_hover_text("Zoom in")
                                .clicked()
                            {
                                push_action(
                                    &self.bridge,
                                    SequencerAction::SetZoom(state.timeline_zoom * 1.25),
                                );
                            }
                            ui.label(
                                egui::RichText::new(format!("{:.0}px/s", state.timeline_zoom))
                                    .size(10.0)
                                    .color(muted_color),
                            );
                            if ui
                                .button(
                                    egui::RichText::new(regular::MAGNIFYING_GLASS_MINUS)
                                        .size(12.0)
                                        .color(muted_color),
                                )
                                .on_hover_text("Zoom out")
                                .clicked()
                            {
                                push_action(
                                    &self.bridge,
                                    SequencerAction::SetZoom(state.timeline_zoom * 0.8),
                                );
                            }
                        },
                    );
                });
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_clip_lane(
        &self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        rect: egui::Rect,
        track_idx: usize,
        track: &crate::model::Track,
        state: &SequencerState,
        accent_color: egui::Color32,
        text_color: egui::Color32,
        muted_color: egui::Color32,
    ) {
        let track_color = track_kind_color(&track.kind);
        let zoom = state.timeline_zoom;
        let scroll = state.timeline_scroll;
        let views = track_clip_views(track);

        for (clip_idx, view) in views.iter().enumerate() {
            let x0 = rect.left() + (view.start - scroll) * zoom;
            let x1 = rect.left() + (view.start + view.duration - scroll) * zoom;
            // Cull off-screen clips.
            if x1 < rect.left() || x0 > rect.right() {
                continue;
            }
            let clip_rect = egui::Rect::from_min_max(
                egui::pos2(x0.max(rect.left()), rect.top() + 2.0),
                egui::pos2(x1.min(rect.right()), rect.bottom() - 2.0),
            );

            let is_marker = matches!(track.kind, TrackKind::Marker { .. });
            let selected = state.selected_clip == Some((track_idx, clip_idx));

            if is_marker {
                // Markers are point-in-time → draw a flag.
                let flag_x = x0;
                if flag_x >= rect.left() && flag_x <= rect.right() {
                    painter.line_segment(
                        [
                            egui::pos2(flag_x, rect.top() + 1.0),
                            egui::pos2(flag_x, rect.bottom() - 1.0),
                        ],
                        egui::Stroke::new(1.5, track_color),
                    );
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            egui::pos2(flag_x, rect.top() + 1.0),
                            egui::pos2(flag_x + 12.0, rect.top() + 5.0),
                            egui::pos2(flag_x, rect.top() + 9.0),
                        ],
                        track_color,
                        egui::Stroke::NONE,
                    ));
                    painter.text(
                        egui::pos2(flag_x + 14.0, rect.top() + 4.0),
                        egui::Align2::LEFT_TOP,
                        &view.name,
                        egui::FontId::proportional(9.5),
                        text_color,
                    );
                }
                continue;
            }

            // Body
            let body_color = with_alpha(track_color, 90);
            painter.rect_filled(clip_rect, 3.0, body_color);
            let stroke_color = if selected { accent_color } else { track_color };
            painter.rect_stroke(
                clip_rect,
                3.0,
                egui::Stroke::new(if selected { 1.6 } else { 1.0 }, stroke_color),
                egui::StrokeKind::Inside,
            );

            // Label
            if clip_rect.width() > 32.0 {
                painter.text(
                    egui::pos2(clip_rect.left() + 5.0, clip_rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    &view.name,
                    egui::FontId::proportional(10.5),
                    text_color,
                );
                if view.key_count > 0 && clip_rect.width() > 70.0 {
                    painter.text(
                        egui::pos2(clip_rect.right() - 4.0, clip_rect.bottom() - 2.0),
                        egui::Align2::RIGHT_BOTTOM,
                        format!("{} keys", view.key_count),
                        egui::FontId::proportional(9.0),
                        muted_color,
                    );
                }
            }

            // Keyframe diamonds (camera/transform clips).
            if let TrackKind::Camera { clips } = &track.kind {
                if let Some(c) = clips.get(clip_idx) {
                    for k in &c.keys {
                        let kx = clip_rect.left() + k.t * zoom;
                        if kx >= clip_rect.left() && kx <= clip_rect.right() {
                            paint_diamond(painter, kx, clip_rect.center().y, 5.0, track_color);
                        }
                    }
                }
            }
            if let TrackKind::Transform { clips, .. } = &track.kind {
                if let Some(c) = clips.get(clip_idx) {
                    for k in &c.keys {
                        let kx = clip_rect.left() + k.t * zoom;
                        if kx >= clip_rect.left() && kx <= clip_rect.right() {
                            paint_diamond(painter, kx, clip_rect.center().y, 5.0, track_color);
                        }
                    }
                }
            }

            // ── Interactions: drag body to move, drag edges to resize ──
            let edge_w = 5.0;
            let body_rect = egui::Rect::from_min_max(
                egui::pos2(clip_rect.left() + edge_w, clip_rect.top()),
                egui::pos2(clip_rect.right() - edge_w, clip_rect.bottom()),
            );
            let left_edge = egui::Rect::from_min_max(
                egui::pos2(clip_rect.left(), clip_rect.top()),
                egui::pos2(clip_rect.left() + edge_w, clip_rect.bottom()),
            );
            let right_edge = egui::Rect::from_min_max(
                egui::pos2(clip_rect.right() - edge_w, clip_rect.top()),
                egui::pos2(clip_rect.right(), clip_rect.bottom()),
            );

            let body_id = ui.id().with(("clip_body", track_idx, clip_idx));
            let body_resp = ui.interact(body_rect, body_id, egui::Sense::click_and_drag());
            if body_resp.clicked() {
                push_action(
                    &self.bridge,
                    SequencerAction::SelectClip(Some((track_idx, clip_idx))),
                );
            }
            if body_resp.dragged() {
                let dx = body_resp.drag_delta().x / zoom;
                push_action(
                    &self.bridge,
                    SequencerAction::MoveClip {
                        track: track_idx,
                        clip: clip_idx,
                        new_start: view.start + dx,
                    },
                );
            }
            if body_resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
            }

            let left_id = ui.id().with(("clip_left", track_idx, clip_idx));
            let left_resp = ui.interact(left_edge, left_id, egui::Sense::drag());
            if left_resp.dragged() {
                let dx = left_resp.drag_delta().x / zoom;
                let new_start = view.start + dx;
                let new_dur = (view.duration - dx).max(0.05);
                push_action(
                    &self.bridge,
                    SequencerAction::ResizeClip {
                        track: track_idx,
                        clip: clip_idx,
                        new_start,
                        new_duration: new_dur,
                    },
                );
            }
            if left_resp.hovered() || left_resp.dragged() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
            }

            let right_id = ui.id().with(("clip_right", track_idx, clip_idx));
            let right_resp = ui.interact(right_edge, right_id, egui::Sense::drag());
            if right_resp.dragged() {
                let dx = right_resp.drag_delta().x / zoom;
                let new_dur = (view.duration + dx).max(0.05);
                push_action(
                    &self.bridge,
                    SequencerAction::ResizeClip {
                        track: track_idx,
                        clip: clip_idx,
                        new_start: view.start,
                        new_duration: new_dur,
                    },
                );
            }
            if right_resp.hovered() || right_resp.dragged() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
            }
        }
    }
}

fn marker_count(state: &SequencerState) -> usize {
    state
        .sequence
        .tracks
        .iter()
        .filter_map(|t| match &t.kind {
            TrackKind::Marker { clips } => Some(clips.len()),
            _ => None,
        })
        .sum()
}

fn track_kind_color(kind: &TrackKind) -> egui::Color32 {
    match kind {
        TrackKind::Camera { .. } => COLOR_CAMERA,
        TrackKind::Transform { .. } => COLOR_TRANSFORM,
        TrackKind::Media { .. } => COLOR_MEDIA,
        TrackKind::Marker { .. } => COLOR_MARKER,
    }
}

fn with_alpha(c: egui::Color32, a: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

#[allow(clippy::too_many_arguments)]
fn paint_ruler(
    painter: egui::Painter,
    rect: egui::Rect,
    duration: f32,
    zoom: f32,
    scroll: f32,
    text_color: egui::Color32,
    muted_color: egui::Color32,
    border_color: egui::Color32,
    bg_color: egui::Color32,
) {
    painter.rect_filled(rect, 0.0, bg_color);
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        egui::Stroke::new(0.5, border_color),
    );

    let base_interval = if zoom >= 200.0 {
        0.1
    } else if zoom >= 80.0 {
        0.5
    } else if zoom >= 30.0 {
        1.0
    } else {
        5.0
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
            (time * 10.0).round() % 10.0 < 0.01
        } else {
            (time % 5.0).abs() < 0.01
        };
        let h = if is_major { 9.0 } else { 4.0 };
        let c = if is_major { text_color } else { muted_color };
        painter.line_segment(
            [
                egui::pos2(x, rect.bottom() - h),
                egui::pos2(x, rect.bottom()),
            ],
            egui::Stroke::new(0.5, c),
        );
        if is_major {
            let label = if time >= 60.0 {
                format!("{}:{:04.1}", (time / 60.0) as u32, time % 60.0)
            } else if time == time.floor() {
                format!("{:.0}s", time)
            } else {
                format!("{:.1}s", time)
            };
            painter.text(
                egui::pos2(x + 2.0, rect.top() + 1.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(9.0),
                muted_color,
            );
        }
    }

    // Sequence end marker
    let end_x = rect.left() + (duration - scroll) * zoom;
    if end_x >= rect.left() && end_x <= rect.right() {
        painter.line_segment(
            [egui::pos2(end_x, rect.top()), egui::pos2(end_x, rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(255, 255, 255, 50)),
        );
    }
}

fn paint_diamond(painter: &egui::Painter, x: f32, y: f32, size: f32, color: egui::Color32) {
    let h = size * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(x, y - h),
            egui::pos2(x + h, y),
            egui::pos2(x, y + h),
            egui::pos2(x - h, y),
        ],
        color,
        egui::Stroke::new(0.5, egui::Color32::from_white_alpha(80)),
    ));
}
