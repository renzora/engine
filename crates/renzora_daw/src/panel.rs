//! Timeline / arrangement panel for the editor.
//!
//! Renders the transport bar, track header column, and the scrolling
//! arrangement view. Drag-drop of audio assets is handled in `drop.rs`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use egui_phosphor::regular as ph;

use renzora_audio::{ClipId, MixerState, TimelineClip, TimelineState, TimelineTrack, TrackId, TransportState};
use renzora_editor::{EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::{Theme, ThemeManager};
use renzora_ui::asset_drag::AssetDragPayload;
use renzora_ui::{empty_state, icon_button};

use crate::drop::AUDIO_EXTENSIONS;
use crate::waveform_cache::WaveformCache;

/// Width of the fixed-position track header column on the left of the panel.
pub const HEADER_W: f32 = 180.0;
/// Height of one track lane in the arrangement view. Sized to comfortably
/// fit the two-row track header (name + bus chip) while still showing
/// readable waveforms in clips.
pub const TRACK_H: f32 = 48.0;
/// Height of the time ruler at the top of the arrangement.
pub const RULER_H: f32 = 22.0;

/// Lightweight intent the panel records during render; applied via
/// `EditorCommands` after the frame so we never alias the world.
#[derive(Clone, Debug)]
pub enum DawIntent {
    Play,
    Stop,
    SeekTo(f64),
    AddTrack { bus: String },
    RemoveTrack(TrackId),
    SetTrackName(TrackId, String),
    SetTrackBus(TrackId, String),
    SetTrackVolume(TrackId, f32),
    SetTrackMute(TrackId, bool),
    SetTrackSolo(TrackId, bool),
    SelectClip(Option<ClipId>),
    MoveClip(ClipId, f64),
    ResizeClipRight(ClipId, f64),
    SetClipName(ClipId, String),
    RemoveClip(ClipId),
    AddClip { track: TrackId, source: PathBuf, start: f64 },
    /// Drop an audio file onto an empty timeline — create a track on the
    /// given bus and place the clip on it in one step.
    AddTrackWithClip { bus: String, source: PathBuf, start: f64 },
    SetBpm(f32),
    SetSnapDiv(u32),
    SetPixelsPerSecond(f32),
}

/// Bridge that funnels intents from `&World`-only panel rendering back into
/// a system that can mutate `TimelineState`.
#[derive(Resource, Default, Clone)]
pub struct DawIntentBuffer {
    inner: Arc<Mutex<Vec<DawIntent>>>,
}

impl DawIntentBuffer {
    pub fn push(&self, i: DawIntent) {
        if let Ok(mut q) = self.inner.lock() {
            q.push(i);
        }
    }
    pub fn drain(&self) -> Vec<DawIntent> {
        self.inner.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
    }
}

/// Drag interaction state held inside the panel via interior mutability.
/// Has to live across frames so we can show a clean drag-grab feel.
#[derive(Default)]
pub struct ClipDragState {
    pub clip: Option<ClipId>,
    /// Cursor x at drag start, in panel-content coordinates.
    pub origin_x: f32,
    /// The clip's `start` value at drag start.
    pub origin_start: f64,
    /// Right-edge resize mode if true; whole-clip move otherwise.
    pub resize: bool,
    /// Original length so a resize can be reverted on Esc.
    pub origin_length: f64,
}

pub struct DawPanel {
    pub drag: Mutex<ClipDragState>,
    /// Local copy of the rename buffer for the track currently being renamed.
    pub renaming_track: Mutex<Option<(TrackId, String)>>,
}

impl Default for DawPanel {
    fn default() -> Self {
        Self {
            drag: Mutex::new(ClipDragState::default()),
            renaming_track: Mutex::new(None),
        }
    }
}

impl EditorPanel for DawPanel {
    fn id(&self) -> &str { "daw" }
    fn title(&self) -> &str { "Audio" }
    fn icon(&self) -> Option<&str> { Some(ph::WAVEFORM) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Center }
    fn min_size(&self) -> [f32; 2] { [520.0, 240.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();

        let Some(timeline) = world.get_resource::<TimelineState>() else {
            ui.label("Timeline state not available");
            return;
        };
        let mixer = world.get_resource::<MixerState>();
        let payload = world.get_resource::<AssetDragPayload>();
        let buffer = world
            .get_resource::<DawIntentBuffer>()
            .cloned()
            .unwrap_or_default();
        let commands = world.get_resource::<EditorCommands>();
        let waveforms = world.get_resource::<WaveformCache>();

        // Top transport bar
        render_transport(ui, timeline, &theme, &buffer);
        ui.separator();

        // Empty-state if no tracks yet — also acts as a drop target so the
        // first audio file dragged in auto-creates a track and places the
        // clip on it (matching the prompt the panel shows).
        if timeline.tracks.is_empty() {
            self.render_empty_state(ui, &theme, mixer, &buffer, payload, timeline);
            return;
        }

        // Main two-column layout: header + arrangement.
        let total_h = ui.available_height().max(120.0);
        let total_w = ui.available_width().max(300.0);
        let arr_w = (total_w - HEADER_W).max(120.0);

        ui.horizontal_top(|ui| {
            // Track header column ----
            ui.allocate_ui_with_layout(
                Vec2::new(HEADER_W, total_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_track_headers(ui, timeline, mixer, &theme, &buffer);
                },
            );

            // Arrangement column ----
            ui.allocate_ui_with_layout(
                Vec2::new(arr_w, total_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_arrangement(
                        ui, world, timeline, &theme, payload, &buffer, commands, waveforms,
                    );
                },
            );
        });

        // Delete-key handler. Only fires when the panel is hovered so a
        // Delete press in the hierarchy / inspector / a text field doesn't
        // wipe the audio clip selection. We also skip if egui has any text
        // edit focused, as a belt-and-braces guard.
        if let Some(clip_id) = timeline.selected_clip {
            let panel_hovered = ui
                .ctx()
                .pointer_hover_pos()
                .map(|p| ui.max_rect().contains(p))
                .unwrap_or(false);
            let text_focused = ui.ctx().wants_keyboard_input();
            if panel_hovered && !text_focused {
                let pressed_delete = ui.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                });
                if pressed_delete {
                    buffer.push(DawIntent::RemoveClip(clip_id));
                    buffer.push(DawIntent::SelectClip(None));
                }
            }
        }
    }
}

impl DawPanel {
    fn render_empty_state(
        &self,
        ui: &mut egui::Ui,
        theme: &Theme,
        mixer: Option<&MixerState>,
        buffer: &DawIntentBuffer,
        payload: Option<&AssetDragPayload>,
        timeline: &TimelineState,
    ) {
        // Reserve the rest of the panel for the empty-state drop target. We
        // need the rect up-front so the audio-drag highlight can be painted
        // *behind* the empty_state content and the drop hit-test sees the
        // whole area, not just the bit under the centered label.
        let drop_rect = ui.available_rect_before_wrap();
        let pp = ui.ctx().pointer_latest_pos();
        let audio_drag = payload
            .filter(|p| p.is_detached && p.matches_extensions(AUDIO_EXTENSIONS));
        let pointer_in_rect = pp.map_or(false, |p| drop_rect.contains(p));
        let is_hovering = audio_drag.is_some() && pointer_in_rect;

        if let (Some(p), true) = (audio_drag, is_hovering) {
            ui.painter().rect_filled(
                drop_rect,
                4.0,
                Color32::from_rgba_unmultiplied(p.color.r(), p.color.g(), p.color.b(), 30),
            );
            ui.painter().rect_stroke(
                drop_rect,
                4.0,
                Stroke::new(1.5, p.color),
                egui::StrokeKind::Inside,
            );
        }

        let (title, body) = if is_hovering {
            ("Drop to create track", "An audio track will be added on the default bus.")
        } else {
            ("No tracks yet", "Drop an audio file here, or add a track to get started.")
        };
        empty_state(ui, ph::WAVEFORM, title, body, theme);

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 90.0);
            let default_bus = pick_default_bus(mixer);
            if ui
                .button(egui::RichText::new(format!("{}  Add audio track", ph::PLUS)).size(12.0))
                .clicked()
            {
                buffer.push(DawIntent::AddTrack { bus: default_bus });
            }
        });

        // Drop completion: pointer released over the empty timeline.
        if is_hovering {
            let pointer_released = !ui.ctx().input(|i| i.pointer.any_down());
            if pointer_released {
                if let (Some(p), Some(pos)) = (audio_drag, pp) {
                    let default_bus = pick_default_bus(mixer);
                    // Map the cursor x onto the timeline so the clip drops
                    // where the user released, not always at t=0. The empty
                    // state has no ruler, so we approximate by treating the
                    // panel rect as the visible timeline at the current zoom.
                    let pps = timeline.pixels_per_second.max(20.0);
                    let drop_secs = ((pos.x - drop_rect.min.x) / pps).max(0.0) as f64;
                    let snapped = timeline.transport.snap_seconds(drop_secs);
                    for path in &p.paths {
                        buffer.push(DawIntent::AddTrackWithClip {
                            bus: default_bus.clone(),
                            source: path.clone(),
                            start: snapped,
                        });
                    }
                }
            }
        }
    }

    fn render_track_headers(
        &self,
        ui: &mut egui::Ui,
        timeline: &TimelineState,
        mixer: Option<&MixerState>,
        theme: &Theme,
        buffer: &DawIntentBuffer,
    ) {
        // Spacer so headers line up with the arrangement's ruler row.
        ui.add_space(RULER_H);

        let row_h = TRACK_H;
        for track in &timeline.tracks {
            let (rect, _resp) = ui.allocate_exact_size(
                Vec2::new(HEADER_W - 4.0, row_h),
                Sense::click(),
            );
            let bg = if track.id == timeline.tracks[0].id {
                theme.panels.inspector_row_even.to_color32()
            } else {
                theme.panels.inspector_row_odd.to_color32()
            };
            ui.painter().rect_filled(rect, 3.0, bg);
            let accent = Color32::from_rgba_unmultiplied(
                track.color[0], track.color[1], track.color[2], track.color[3],
            );
            // Left accent stripe
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                0.0, accent,
            );

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(
                        Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0),
                        Vec2::new(rect.width() - 12.0, rect.height() - 8.0),
                    ))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            self.render_track_header_row(&mut child, track, mixer, theme, buffer);
        }

        ui.add_space(8.0);
        let default_bus = pick_default_bus(mixer);
        if ui
            .button(egui::RichText::new(format!("{}  Add track", ph::PLUS)).size(11.0))
            .clicked()
        {
            buffer.push(DawIntent::AddTrack { bus: default_bus });
        }
    }

    fn render_track_header_row(
        &self,
        ui: &mut egui::Ui,
        track: &TimelineTrack,
        mixer: Option<&MixerState>,
        theme: &Theme,
        buffer: &DawIntentBuffer,
    ) {
        // Row 1: name (click-to-rename) · M · S · trash. Volume control is
        // intentionally absent — that lives on the mixer strip the track is
        // routed to, matching the convention in pro DAWs (Live, Reaper,
        // Logic) where the arrangement-side header carries identity +
        // routing while the mixer carries gain.
        ui.horizontal(|ui| {
            let mut renaming = self.renaming_track.lock().unwrap();
            let is_renaming = renaming.as_ref().map(|(t, _)| *t == track.id).unwrap_or(false);
            if is_renaming {
                let buf = renaming.as_mut().map(|(_, s)| s).unwrap();
                let resp = ui.add(
                    egui::TextEdit::singleline(buf)
                        .desired_width(90.0)
                        .font(egui::FontId::proportional(11.0)),
                );
                let commit = resp.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let cancel = resp.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Escape));
                if commit {
                    let new_name = buf.trim().to_string();
                    if !new_name.is_empty() {
                        buffer.push(DawIntent::SetTrackName(track.id, new_name));
                    }
                    *renaming = None;
                } else if cancel {
                    *renaming = None;
                }
                if !resp.has_focus() && !commit && !cancel {
                    resp.request_focus();
                }
            } else {
                let label = egui::Label::new(
                    egui::RichText::new(&track.name)
                        .size(11.5)
                        .strong()
                        .color(theme.text.primary.to_color32()),
                )
                .truncate()
                .sense(Sense::click());
                let resp = ui
                    .add_sized(Vec2::new(74.0, 16.0), label)
                    .on_hover_text("Double-click to rename");
                if resp.double_clicked() {
                    *renaming = Some((track.id, track.name.clone()));
                }
            }

            // Right-aligned cluster: trash · S · M (read right-to-left so
            // they appear M S 🗑 visually).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let trash = egui::Button::new(
                    egui::RichText::new(ph::TRASH)
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                )
                .frame(false);
                if ui.add(trash).on_hover_text("Delete track").clicked() {
                    buffer.push(DawIntent::RemoveTrack(track.id));
                }
                ui.add_space(2.0);

                let solo_chip = mini_state_chip(
                    ui,
                    "S",
                    track.soloed,
                    theme.semantic.warning.to_color32(),
                    theme,
                );
                if solo_chip {
                    buffer.push(DawIntent::SetTrackSolo(track.id, !track.soloed));
                }

                let mute_chip = mini_state_chip(
                    ui,
                    "M",
                    track.muted,
                    theme.semantic.error.to_color32(),
                    theme,
                );
                if mute_chip {
                    buffer.push(DawIntent::SetTrackMute(track.id, !track.muted));
                }
            });
        });

        // Row 2: bus chip — small accent-tinted pill, much less visually
        // heavy than the previous full-width combobox.
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let buses = available_buses(mixer);
            let chip_id = ui.id().with(("daw_bus_chip", track.id.0));
            let chip_label = format!("→ {}", track.bus_name);
            let chip = egui::Button::new(
                egui::RichText::new(chip_label)
                    .size(10.0)
                    .color(theme.text.muted.to_color32()),
            )
            .fill(theme.surfaces.faint.to_color32())
            .stroke(Stroke::new(0.5, theme.widgets.border.to_color32()))
            .corner_radius(2.0)
            .min_size(Vec2::new(0.0, 16.0));
            let resp = ui.add(chip).on_hover_text("Mixer bus this track plays into");
            let popup_id = chip_id.with("popup");
            if resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(popup_id));
            }
            egui::popup_below_widget(
                ui,
                popup_id,
                &resp,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(120.0);
                    for bus in &buses {
                        let selected = bus == &track.bus_name;
                        if ui.selectable_label(selected, bus).clicked() {
                            buffer.push(DawIntent::SetTrackBus(track.id, bus.clone()));
                            ui.memory_mut(|m| m.close_popup(popup_id));
                        }
                    }
                },
            );
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn render_arrangement(
        &self,
        ui: &mut egui::Ui,
        world: &World,
        timeline: &TimelineState,
        theme: &Theme,
        payload: Option<&AssetDragPayload>,
        buffer: &DawIntentBuffer,
        commands: Option<&EditorCommands>,
        waveforms: Option<&WaveformCache>,
    ) {
        let pps = timeline.pixels_per_second.max(20.0);
        let content_w =
            (timeline.view_duration as f32 * pps).max(ui.available_width().max(200.0));
        let track_count = timeline.tracks.len() as f32;
        let content_h = RULER_H + track_count * TRACK_H + 4.0;

        let scroll_resp = egui::ScrollArea::both()
            .id_salt("daw_arrangement")
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, _viewport| {
                ui.set_min_size(Vec2::new(content_w, content_h));
                let origin = ui.cursor().min;

                self.draw_arrangement_content(
                    ui, world, origin, content_w, timeline, theme, payload, buffer, commands,
                    waveforms,
                );
            });
        let _ = scroll_resp;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_arrangement_content(
        &self,
        ui: &mut egui::Ui,
        world: &World,
        origin: Pos2,
        content_w: f32,
        timeline: &TimelineState,
        theme: &Theme,
        payload: Option<&AssetDragPayload>,
        buffer: &DawIntentBuffer,
        commands: Option<&EditorCommands>,
        waveforms: Option<&WaveformCache>,
    ) {
        let painter = ui.painter().clone();
        let pps = timeline.pixels_per_second.max(20.0);

        // Background
        let total_rect = Rect::from_min_size(
            origin,
            Vec2::new(content_w, RULER_H + timeline.tracks.len() as f32 * TRACK_H),
        );
        painter.rect_filled(total_rect, 0.0, theme.surfaces.faint.to_color32());

        // Ruler
        let ruler_rect = Rect::from_min_size(origin, Vec2::new(content_w, RULER_H));
        painter.rect_filled(ruler_rect, 0.0, theme.surfaces.faint.to_color32().gamma_multiply(0.6));
        let bpm = timeline.transport.bpm.max(1.0);
        let beat_secs = 60.0 / bpm as f64;
        // Choose major step so we get <~6 labels per 200px to avoid clutter
        let max_seconds = content_w as f64 / pps as f64;
        let mut beat = 0.0;
        let muted_color = theme.text.muted.to_color32();
        let primary_color = theme.text.primary.to_color32();
        let beat_count = (max_seconds / beat_secs).ceil() as i32 + 1;
        for b in 0..beat_count {
            let t = b as f64 * beat_secs;
            let x = origin.x + (t * pps as f64) as f32;
            let major = b % 4 == 0;
            painter.line_segment(
                [
                    Pos2::new(x, ruler_rect.max.y - if major { 10.0 } else { 5.0 }),
                    Pos2::new(x, ruler_rect.max.y),
                ],
                Stroke::new(1.0, if major { primary_color } else { muted_color }),
            );
            if major {
                painter.text(
                    Pos2::new(x + 3.0, ruler_rect.min.y + 1.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}", b / 4 + 1),
                    egui::FontId::proportional(9.5),
                    primary_color,
                );
            }
            beat += 1.0;
        }
        let _ = beat;

        // Track lanes + grid
        for (i, track) in timeline.tracks.iter().enumerate() {
            let lane_y = origin.y + RULER_H + i as f32 * TRACK_H;
            let lane_rect =
                Rect::from_min_size(Pos2::new(origin.x, lane_y), Vec2::new(content_w, TRACK_H));
            let bg = if i % 2 == 0 {
                theme.panels.inspector_row_even.to_color32()
            } else {
                theme.panels.inspector_row_odd.to_color32()
            };
            painter.rect_filled(lane_rect, 0.0, bg);

            // Beat grid
            for b in 0..beat_count {
                let t = b as f64 * beat_secs;
                let x = origin.x + (t * pps as f64) as f32;
                let major = b % 4 == 0;
                let stroke = if major {
                    Stroke::new(1.0, theme.widgets.border.to_color32().gamma_multiply(0.6))
                } else {
                    Stroke::new(0.5, theme.widgets.border.to_color32().gamma_multiply(0.3))
                };
                painter.line_segment(
                    [Pos2::new(x, lane_rect.min.y), Pos2::new(x, lane_rect.max.y)],
                    stroke,
                );
            }

            // Drop highlight if a compatible audio file is being dragged here.
            if let Some(p) = payload {
                if p.is_detached && p.matches_extensions(AUDIO_EXTENSIONS) {
                    if let Some(pp) = ui.ctx().pointer_hover_pos() {
                        if lane_rect.contains(pp) {
                            painter.rect_filled(
                                lane_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(
                                    p.color.r(), p.color.g(), p.color.b(), 30,
                                ),
                            );
                            painter.rect_stroke(
                                lane_rect,
                                0.0,
                                Stroke::new(1.5, p.color),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                }
            }

            // Empty-lane hint: faint "Drag audio here" centred in the
            // visible portion of the lane (not the entire timeline) so it
            // stays readable as the user scrolls. Only drawn when the
            // track has zero clips, so existing audio is never obscured.
            let has_clips = timeline.clips.iter().any(|c| c.track == track.id);
            if !has_clips {
                let visible_left = ui.clip_rect().min.x.max(lane_rect.min.x);
                let visible_right = ui.clip_rect().max.x.min(lane_rect.max.x);
                if visible_right > visible_left + 80.0 {
                    let center = Pos2::new(
                        (visible_left + visible_right) * 0.5,
                        lane_rect.center().y,
                    );
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        "Drag audio here",
                        egui::FontId::proportional(10.5),
                        theme.text.muted.to_color32().gamma_multiply(0.6),
                    );
                }
            }

            // Clips on this lane
            self.render_lane_clips(
                ui, &painter, origin, lane_rect, track, timeline, pps, theme, buffer, waveforms,
            );
        }

        // Playhead
        let px = origin.x + (timeline.transport.position * pps as f64) as f32;
        let arrangement_bottom = origin.y + RULER_H + timeline.tracks.len() as f32 * TRACK_H;
        painter.line_segment(
            [Pos2::new(px, origin.y), Pos2::new(px, arrangement_bottom)],
            Stroke::new(1.5, theme.semantic.accent.to_color32()),
        );

        // Ruler interaction: click to seek
        let ruler_resp = ui.interact(
            ruler_rect,
            egui::Id::new(("daw_ruler_seek", timeline.tracks.len())),
            Sense::click_and_drag(),
        );
        if ruler_resp.clicked() || ruler_resp.dragged() {
            if let Some(p) = ruler_resp.interact_pointer_pos() {
                let t = ((p.x - origin.x) / pps as f32).max(0.0) as f64;
                let snapped = timeline.transport.snap_seconds(t);
                buffer.push(DawIntent::SeekTo(snapped));
            }
        }

        // Drop completion (audio file → new clip on a lane)
        self.handle_audio_drop(ui, world, origin, timeline, payload, buffer, commands);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_lane_clips(
        &self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        origin: Pos2,
        lane_rect: Rect,
        track: &TimelineTrack,
        timeline: &TimelineState,
        pps: f32,
        theme: &Theme,
        buffer: &DawIntentBuffer,
        waveforms: Option<&WaveformCache>,
    ) {
        let track_color = Color32::from_rgba_unmultiplied(
            track.color[0], track.color[1], track.color[2], 255,
        );
        let track_color_fill = Color32::from_rgba_unmultiplied(
            track.color[0], track.color[1], track.color[2], 110,
        );
        let track_color_dim = Color32::from_rgba_unmultiplied(
            track.color[0], track.color[1], track.color[2], 45,
        );

        for clip in timeline.clips.iter().filter(|c| c.track == track.id) {
            let x0 = origin.x + (clip.start * pps as f64) as f32;
            let x1 = origin.x + ((clip.start + clip.length) * pps as f64) as f32;
            let clip_rect = Rect::from_min_max(
                Pos2::new(x0, lane_rect.min.y + 4.0),
                Pos2::new(x1.max(x0 + 4.0), lane_rect.max.y - 4.0),
            );

            // Body
            let body_color = if clip.muted { track_color_dim } else { track_color_fill };
            painter.rect_filled(clip_rect, 4.0, body_color);
            painter.rect_stroke(
                clip_rect,
                4.0,
                Stroke::new(
                    if Some(clip.id) == timeline.selected_clip { 2.0 } else { 1.0 },
                    track_color,
                ),
                egui::StrokeKind::Inside,
            );
            // Waveform fills the body below the title strip.
            if let Some(cache) = waveforms {
                if let Some(peaks) = cache.get(&clip.source) {
                    draw_clip_waveform(painter, &clip_rect, &peaks.peaks, track_color);
                }
            }

            // Title
            let title_pos = Pos2::new(clip_rect.min.x + 6.0, clip_rect.min.y + 3.0);
            painter.text(
                title_pos,
                egui::Align2::LEFT_TOP,
                &clip.name,
                egui::FontId::proportional(10.0),
                theme.text.primary.to_color32(),
            );

            // Interaction area for drag/move
            let id = egui::Id::new(("daw_clip", clip.id.0));
            let resp = ui.interact(clip_rect, id, Sense::click_and_drag());

            // Right-edge resize handle (last 6px)
            let handle_rect = Rect::from_min_max(
                Pos2::new((clip_rect.max.x - 6.0).max(clip_rect.min.x + 4.0), clip_rect.min.y),
                clip_rect.max,
            );
            let handle_id = id.with("rhandle");
            let handle_resp = ui.interact(handle_rect, handle_id, Sense::click_and_drag());

            if resp.clicked() || handle_resp.clicked() {
                buffer.push(DawIntent::SelectClip(Some(clip.id)));
            }

            // Begin drag
            let mut drag = self.drag.lock().unwrap();
            if handle_resp.drag_started() {
                drag.clip = Some(clip.id);
                drag.resize = true;
                drag.origin_x = handle_resp
                    .interact_pointer_pos()
                    .map(|p| p.x)
                    .unwrap_or(clip_rect.max.x);
                drag.origin_start = clip.start;
                drag.origin_length = clip.length;
            } else if resp.drag_started() {
                drag.clip = Some(clip.id);
                drag.resize = false;
                drag.origin_x = resp
                    .interact_pointer_pos()
                    .map(|p| p.x)
                    .unwrap_or(clip_rect.min.x);
                drag.origin_start = clip.start;
                drag.origin_length = clip.length;
            }

            // Continue drag
            if drag.clip == Some(clip.id) && (resp.dragged() || handle_resp.dragged()) {
                if let Some(pp) = resp
                    .interact_pointer_pos()
                    .or_else(|| handle_resp.interact_pointer_pos())
                {
                    let dx_secs = ((pp.x - drag.origin_x) / pps) as f64;
                    if drag.resize {
                        let new_len = (drag.origin_length + dx_secs).max(0.05);
                        let snapped_end = timeline
                            .transport
                            .snap_seconds(drag.origin_start + new_len);
                        let new_len_snapped = (snapped_end - drag.origin_start).max(0.05);
                        buffer.push(DawIntent::ResizeClipRight(clip.id, new_len_snapped));
                    } else {
                        let new_start = (drag.origin_start + dx_secs).max(0.0);
                        let snapped = timeline.transport.snap_seconds(new_start);
                        buffer.push(DawIntent::MoveClip(clip.id, snapped));
                    }
                }
            }

            if resp.drag_stopped() || handle_resp.drag_stopped() {
                drag.clip = None;
            }

            // Right-click menu
            resp.context_menu(|ui| {
                if ui.button(format!("{}  Delete clip", ph::TRASH)).clicked() {
                    buffer.push(DawIntent::RemoveClip(clip.id));
                    ui.close();
                }
            });

            // Cursor hint
            if handle_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            } else if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
    }

    fn handle_audio_drop(
        &self,
        ui: &mut egui::Ui,
        _world: &World,
        origin: Pos2,
        timeline: &TimelineState,
        payload: Option<&AssetDragPayload>,
        buffer: &DawIntentBuffer,
        _commands: Option<&EditorCommands>,
    ) {
        let Some(payload) = payload else { return };
        if !payload.is_detached || !payload.matches_extensions(AUDIO_EXTENSIONS) {
            return;
        }
        let Some(pp) = ui.ctx().pointer_latest_pos() else { return };

        // Determine which lane (if any) the cursor is over.
        let pps = timeline.pixels_per_second.max(20.0);
        let mut hit_track: Option<TrackId> = None;
        for (i, track) in timeline.tracks.iter().enumerate() {
            let lane_y = origin.y + RULER_H + i as f32 * TRACK_H;
            let lane_rect = Rect::from_min_size(
                Pos2::new(origin.x, lane_y),
                Vec2::new(timeline.view_duration as f32 * pps, TRACK_H),
            );
            if lane_rect.contains(pp) {
                hit_track = Some(track.id);
                break;
            }
        }

        let pointer_released = !ui.ctx().input(|i| i.pointer.any_down());
        if !pointer_released {
            return;
        }
        let Some(track) = hit_track else { return };

        let drop_secs = ((pp.x - origin.x) / pps).max(0.0) as f64;
        let snapped = timeline.transport.snap_seconds(drop_secs);

        for path in &payload.paths {
            buffer.push(DawIntent::AddClip {
                track,
                source: path.clone(),
                start: snapped,
            });
        }
    }
}

/// Pro-DAW transport header.
///
/// Layout (left → right):
///   [Transport cluster] │ [LCD time display] │ [TEMPO] │ [GRID] │ … │ [ZOOM]
///
/// Each cluster is its own framed surface so the bar reads as a row of
/// dedicated instrument-style modules instead of a strip of bare buttons.
/// Visual cues mirror Logic / Bitwig: a chunky filled play pill, an inset
/// dark "LCD" panel for time, and small-caps labels above each chip value.
fn render_transport(
    ui: &mut egui::Ui,
    timeline: &TimelineState,
    theme: &Theme,
    buffer: &DawIntentBuffer,
) {
    let panel_bg = theme.surfaces.panel.to_color32();
    let border = theme.widgets.border.to_color32();
    let border_light = theme.widgets.border_light.to_color32();
    let muted = theme.text.muted.to_color32();
    let primary = theme.text.primary.to_color32();
    let secondary = theme.text.secondary.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let lcd_bg = theme.surfaces.extreme.to_color32();
    let cluster_bg = theme.widgets.noninteractive_bg.to_color32();

    // Transport tinted slightly darker than the surrounding panel so the
    // header reads as its own bar even on themes where panel and surface are
    // close in luminance.
    let bar_bg = darken(panel_bg, 0.10);

    // Outer frame draws the bar background. The inner UI then allocates a
    // fixed-height horizontal row so the bar doesn't stretch to fill the
    // panel's full vertical space (which would scatter clusters across
    // hundreds of pixels and make them wrap onto separate rows).
    const BAR_ROW_H: f32 = 40.0;

    egui::Frame::NONE
        .fill(bar_bg)
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 6,
            bottom: 6,
        })
        .stroke(Stroke::NONE)
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), BAR_ROW_H),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                ui.set_min_height(BAR_ROW_H);
                ui.spacing_mut().item_spacing.x = 8.0;

                // ── Transport cluster ───────────────────────────────────
                let is_playing = timeline.transport.is_playing();
                cluster(ui, cluster_bg, border_light, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    if icon_button(ui, ph::SKIP_BACK, "Return to start", primary) {
                        buffer.push(DawIntent::SeekTo(0.0));
                    }

                    // Play pill — the visual anchor of the bar.
                    let (play_icon, play_fill, play_fg, play_tip) = if is_playing {
                        (ph::PAUSE, accent, Color32::WHITE, "Pause")
                    } else {
                        (
                            ph::PLAY,
                            Color32::from_rgb(220, 60, 60),
                            Color32::WHITE,
                            "Play",
                        )
                    };
                    if play_pill(ui, play_icon, play_fill, play_fg)
                        .on_hover_text(play_tip)
                        .clicked()
                    {
                        if is_playing {
                            buffer.push(DawIntent::Stop);
                        } else {
                            buffer.push(DawIntent::Play);
                        }
                    }

                    if icon_button(ui, ph::STOP, "Stop", primary) {
                        buffer.push(DawIntent::Stop);
                    }
                });

                divider(ui, border);

                // ── LCD time display ────────────────────────────────────
                let secs = timeline.transport.position;
                let beats = timeline.transport.seconds_to_beats(secs);
                let bar_n = (beats / 4.0).floor() as i32 + 1;
                let beat_in_bar = (beats % 4.0).floor() as i32 + 1;
                let ticks = ((beats - beats.floor()) * 1000.0) as i32;
                let mins = (secs / 60.0) as u32;
                let sec_part = secs % 60.0;

                lcd_panel(ui, lcd_bg, border, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            egui::RichText::new(format!(
                                "{:>3}.{:>1}.{:03}",
                                bar_n, beat_in_bar, ticks
                            ))
                            .monospace()
                            .size(15.0)
                            .strong()
                            .color(accent),
                        );
                        ui.label(
                            egui::RichText::new(format!("{:02}:{:05.2}s", mins, sec_part))
                                .monospace()
                                .size(10.0)
                                .color(secondary),
                        );
                    });
                });

                divider(ui, border);

                // ── Tempo ───────────────────────────────────────────────
                labelled_chip(ui, "TEMPO", muted, |ui| {
                    let mut bpm = timeline.transport.bpm;
                    if ui
                        .add(
                            egui::DragValue::new(&mut bpm)
                                .speed(0.5)
                                .range(20.0..=999.0)
                                .fixed_decimals(1)
                                .suffix(" BPM"),
                        )
                        .changed()
                    {
                        buffer.push(DawIntent::SetBpm(bpm));
                    }
                });

                divider(ui, border);

                // ── Grid / snap ─────────────────────────────────────────
                labelled_chip(ui, "GRID", muted, |ui| {
                    let mut snap = timeline.transport.snap_div;
                    let snap_label = match snap {
                        0 => "off".to_string(),
                        1 => "1/4".to_string(),
                        2 => "1/8".to_string(),
                        4 => "1/16".to_string(),
                        8 => "1/32".to_string(),
                        other => format!("1/{}", 4 * other),
                    };
                    egui::ComboBox::from_id_salt("daw_snap")
                        .selected_text(snap_label)
                        .width(60.0)
                        .show_ui(ui, |ui| {
                            for (label, value) in [
                                ("off", 0u32),
                                ("1/4", 1),
                                ("1/8", 2),
                                ("1/16", 4),
                                ("1/32", 8),
                            ] {
                                if ui.selectable_value(&mut snap, value, label).clicked() {
                                    buffer.push(DawIntent::SetSnapDiv(snap));
                                }
                            }
                        });
                });

                // Zoom on the right edge.
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        labelled_chip(ui, "ZOOM", muted, |ui| {
                            let mut pps = timeline.pixels_per_second;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut pps)
                                        .speed(2.0)
                                        .range(20.0..=600.0)
                                        .suffix(" px/s"),
                                )
                                .changed()
                            {
                                buffer.push(DawIntent::SetPixelsPerSecond(pps));
                            }
                        });
                    },
                );
            });

            // Hairline under the bar to separate it cleanly from the
            // arrangement / track header rows below.
            let r = ui.max_rect();
            ui.painter().line_segment(
                [
                    Pos2::new(r.left(), r.bottom() + 0.5),
                    Pos2::new(r.right(), r.bottom() + 0.5),
                ],
                Stroke::new(1.0, border),
            );
        });
}

// ─── Transport bar building blocks ──────────────────────────────────────────

/// Frame a horizontal cluster of widgets in a subtly raised pill so the
/// transport buttons read as a single unit.
fn cluster(
    ui: &mut egui::Ui,
    fill: Color32,
    stroke: Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(28.0);
                add_contents(ui);
            });
        });
}

/// Inset darker panel for the time readout — evokes a hardware LCD.
fn lcd_panel(
    ui: &mut egui::Ui,
    fill: Color32,
    stroke: Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 3,
            bottom: 3,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(28.0);
                add_contents(ui);
            });
        });
}

/// Small-caps zone label stacked above its control. Used for TEMPO / GRID /
/// ZOOM so each chip reads as a labelled module rather than a free-floating
/// input.
fn labelled_chip(
    ui: &mut egui::Ui,
    label: &str,
    label_color: Color32,
    add_value: impl FnOnce(&mut egui::Ui),
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(
            egui::RichText::new(label)
                .size(8.5)
                .strong()
                .extra_letter_spacing(0.8)
                .color(label_color),
        );
        add_value(ui);
    });
}

/// Thin vertical divider between transport zones.
fn divider(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 28.0), Sense::hover());
    ui.painter().line_segment(
        [
            Pos2::new(rect.center().x, rect.top() + 2.0),
            Pos2::new(rect.center().x, rect.bottom() - 2.0),
        ],
        Stroke::new(1.0, color),
    );
}

/// The big filled play / pause pill at the centre of the transport cluster.
fn play_pill(ui: &mut egui::Ui, glyph: &str, fill: Color32, fg: Color32) -> egui::Response {
    let btn = egui::Button::new(
        egui::RichText::new(glyph)
            .size(15.0)
            .color(fg)
            .strong(),
    )
    .min_size(Vec2::new(34.0, 24.0))
    .fill(fill)
    .stroke(Stroke::NONE)
    .corner_radius(egui::CornerRadius::same(5));
    let resp = ui.add(btn);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Multiply each channel by `1 - amount` to darken the colour. Cheap helper
/// so we can sit the transport bar a tick darker than the surrounding panel
/// on any theme.
fn darken(c: Color32, amount: f32) -> Color32 {
    let f = (1.0 - amount).clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(
        (c.r() as f32 * f) as u8,
        (c.g() as f32 * f) as u8,
        (c.b() as f32 * f) as u8,
        c.a(),
    )
}

/// Compact ON/OFF chip used for M/S buttons in the track header. Returns
/// `true` if clicked. Active state fills with `accent`; inactive shows a
/// faint outlined rect — pro-DAW look (Live, Logic) where mute/solo are
/// always visible but visually quiet until engaged.
fn mini_state_chip(
    ui: &mut egui::Ui,
    label: &str,
    active: bool,
    accent: Color32,
    theme: &Theme,
) -> bool {
    let size = Vec2::new(16.0, 16.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let bg = if active {
        accent
    } else if resp.hovered() {
        theme.widgets.hovered_bg.to_color32()
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if active {
        Stroke::NONE
    } else {
        Stroke::new(0.8, theme.widgets.border.to_color32())
    };
    painter.rect(rect, 2.0, bg, stroke, egui::StrokeKind::Inside);
    let text_color = if active {
        Color32::WHITE
    } else {
        theme.text.muted.to_color32()
    };
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(9.5),
        text_color,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn pick_default_bus(mixer: Option<&MixerState>) -> String {
    if let Some(m) = mixer {
        if !m.custom_buses.is_empty() {
            return m.custom_buses[0].0.clone();
        }
    }
    "Music".to_string()
}

fn available_buses(mixer: Option<&MixerState>) -> Vec<String> {
    let mut out = vec![
        "Master".to_string(),
        "Music".to_string(),
        "Sfx".to_string(),
        "Ambient".to_string(),
    ];
    if let Some(m) = mixer {
        for (name, _) in &m.custom_buses {
            out.push(name.clone());
        }
    }
    out
}

/// Apply intents queued during the last frame to `TimelineState`. Runs after
/// panel rendering, before the scheduler.
pub fn apply_intents(
    buffer: Res<DawIntentBuffer>,
    mut timeline: ResMut<TimelineState>,
) {
    let intents = buffer.drain();
    if intents.is_empty() {
        return;
    }
    for intent in intents {
        apply_one_intent(&mut timeline, intent);
    }
}

fn apply_one_intent(timeline: &mut TimelineState, intent: DawIntent) {
    match intent {
        DawIntent::Play => timeline.transport.state = TransportState::Playing,
        DawIntent::Stop => {
            timeline.transport.state = TransportState::Stopped;
        }
        DawIntent::SeekTo(t) => {
            timeline.transport.position = t.max(0.0);
        }
        DawIntent::AddTrack { bus } => {
            let n = timeline.tracks.len() + 1;
            timeline.add_track(format!("Track {}", n), bus);
        }
        DawIntent::RemoveTrack(id) => timeline.remove_track(id),
        DawIntent::SetTrackName(id, name) => {
            if let Some(t) = timeline.track_mut(id) { t.name = name; }
        }
        DawIntent::SetTrackBus(id, bus) => {
            if let Some(t) = timeline.track_mut(id) { t.bus_name = bus; }
        }
        DawIntent::SetTrackVolume(id, v) => {
            if let Some(t) = timeline.track_mut(id) { t.volume = v.max(0.0); }
        }
        DawIntent::SetTrackMute(id, m) => {
            if let Some(t) = timeline.track_mut(id) { t.muted = m; }
        }
        DawIntent::SetTrackSolo(id, s) => {
            if let Some(t) = timeline.track_mut(id) { t.soloed = s; }
        }
        DawIntent::SelectClip(id) => timeline.selected_clip = id,
        DawIntent::MoveClip(id, t) => {
            if let Some(c) = timeline.clip_mut(id) { c.start = t.max(0.0); }
        }
        DawIntent::ResizeClipRight(id, len) => {
            if let Some(c) = timeline.clip_mut(id) { c.length = len.max(0.05); }
        }
        DawIntent::SetClipName(id, name) => {
            if let Some(c) = timeline.clip_mut(id) { c.name = name; }
        }
        DawIntent::RemoveClip(id) => timeline.remove_clip(id),
        DawIntent::AddClip { track, source, start } => {
            // Default placeholder length; the scheduler will trim to real
            // duration the first time the file gets loaded.
            timeline.add_clip(track, source, start, 600.0);
        }
        DawIntent::AddTrackWithClip { bus, source, start } => {
            let n = timeline.tracks.len() + 1;
            let track = timeline.add_track(format!("Track {}", n), bus);
            timeline.add_clip(track, source, start, 600.0);
        }
        DawIntent::SetBpm(b) => timeline.transport.bpm = b.clamp(20.0, 999.0),
        DawIntent::SetSnapDiv(d) => timeline.transport.snap_div = d,
        DawIntent::SetPixelsPerSecond(p) => {
            timeline.pixels_per_second = p.clamp(20.0, 600.0);
        }
    }
}

/// Helper used by `TimelineClip` rendering elsewhere — kept here so the
/// bridge can dehydrate ids when serialising.
#[allow(dead_code)]
pub fn clip_filename(c: &TimelineClip) -> String {
    c.source
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&c.name)
        .to_string()
}

/// Draw a downsampled waveform inside a clip body, leaving room at the top
/// for the clip title. `peaks[i] = (min, max)` in `-1.0..=1.0`.
fn draw_clip_waveform(
    painter: &egui::Painter,
    clip_rect: &Rect,
    peaks: &[(f32, f32)],
    color: Color32,
) {
    if peaks.is_empty() || clip_rect.width() < 8.0 {
        return;
    }
    // Body area below the title bar.
    let body = Rect::from_min_max(
        Pos2::new(clip_rect.min.x + 2.0, clip_rect.min.y + 16.0),
        Pos2::new(clip_rect.max.x - 2.0, clip_rect.max.y - 2.0),
    );
    if body.height() < 4.0 {
        return;
    }
    let cy = body.center().y;
    let hh = body.height() * 0.5;
    let n = peaks.len();
    let step = body.width() / n as f32;
    let stroke_color = color.gamma_multiply(0.85);
    for (i, (mn, mx)) in peaks.iter().enumerate() {
        let x = body.min.x + i as f32 * step + step * 0.5;
        let y_top = cy - hh * mx.clamp(-1.0, 1.0);
        let y_bot = cy - hh * mn.clamp(-1.0, 1.0);
        painter.line_segment(
            [Pos2::new(x, y_top), Pos2::new(x, y_bot)],
            Stroke::new(step.max(1.0), stroke_color),
        );
    }
}

/// System: ensure every clip on the timeline has a peaks request in flight.
/// Cheap to run every frame — `request` is idempotent and fingerprinted.
pub fn request_clip_waveforms(
    timeline: Res<TimelineState>,
    cache: Res<WaveformCache>,
) {
    for clip in &timeline.clips {
        if cache.needs_request(&clip.source) {
            cache.request(clip.source.clone());
        }
    }
}
