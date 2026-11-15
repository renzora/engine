//! The Profiler screen: a frame graph, a zoomable timeline, the slowest spans.
//!
//! Ported from the engine companion's `ui::profiler`, which drew both graphs
//! as meshes on a camera of their own; here they are egui painter calls. The
//! behaviour is the same: the graph shows the newest frames as bars, a click
//! picks one, and the timeline shows every thread's spans around it, zoomed
//! with the wheel and panned by dragging, following the newest frame twice a
//! second while recording.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke};

use renzora_companion_proto::frame::{ProfileFrame, Span};

use super::{fmt_ms, short, ACCENT, FAINT, HOVER, MUTED, PANEL, WARN};
use crate::capture::{frame_ms, is_wrapper, slowest_in, Capture};

/// The frame-time line on the graph.
const BUDGET_MS: f32 = 16.6;
/// Width of the thread-name column inside the timeline.
const LANE_LABEL_W: f32 = 132.0;
const ROW_H: f32 = 22.0;
const MAX_DEPTH: u16 = 5;
const SLOWEST_ROWS: usize = 12;
/// Bar width and gap on the frame graph.
const BAR: f32 = 4.0;
const GAP: f32 = 1.0;
/// How often a following timeline steps to the newest frame: every frame
/// redrew a different one thirty times a second, which no one can read.
const FOLLOW_EVERY: Duration = Duration::from_millis(500);
/// Vertical drag distance that scrolls the timeline by one thread lane.
const LANE_DRAG_PX: f32 = 40.0;
/// Roughly how wide a character of a span label is, for cutting labels to
/// their bar without measuring thousands of strings a frame.
const CHAR_W: f32 = 7.0;

const TEXT_ON_BAR: Color32 = Color32::from_rgb(244, 246, 250);
const OVERLAPPED: Color32 = Color32::from_rgb(96, 98, 112);
const OVERLAPPED_STRIP: Color32 = Color32::from_rgb(150, 152, 170);
const SLOW: Color32 = Color32::from_rgb(185, 138, 74);
const GPU_BAR: Color32 = Color32::from_rgb(106, 79, 181);

/// The timeline's window onto time, in the recorder's nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelineView {
    pub start_ns: u64,
    pub width_ns: u64,
    /// Move to the newest frame as frames arrive.
    pub follow: bool,
    /// The start of the frame the view was last moved to. Following keeps the
    /// view's offset from it, so a zoom made while following holds as the
    /// view steps from frame to frame.
    pub followed_start_ns: u64,
    /// The first thread lane shown: the timeline scrolls by whole lanes, so
    /// no lane is ever cut in half under the ruler.
    pub first_lane: usize,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self { start_ns: 0, width_ns: 20_000_000, follow: true, followed_start_ns: 0, first_lane: 0 }
    }
}

#[derive(Default)]
pub(super) struct Profiler {
    /// The frame the header and "Fit frame" are about. `None` follows the
    /// newest one.
    pub selected: Option<u64>,
    pub view: TimelineView,
    followed_at: Option<Instant>,
    drag_lanes: f32,
}

impl Profiler {
    /// The capture was replaced: nothing selected in it, follow from the start.
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, capture: &Capture, live: bool) {
        if live {
            self.follow_newest(capture);
        }
        egui::Panel::top("frame-graph").resizable(false).exact_size(160.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Frames  ·  click one to inspect it").color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.monospace(self.stats(capture));
                });
            });
            self.frame_graph(ui, capture);
        });
        egui::Panel::bottom("slowest").resizable(false).show(ui, |ui| self.slowest(ui, capture));
        self.toolbar(ui, capture);
        self.timeline(ui, capture);
    }

    /// The frame the view is about: the selected one, or the newest.
    fn focus<'a>(&self, capture: &'a Capture) -> Option<&'a ProfileFrame> {
        self.selected.and_then(|i| capture.frame(i)).or_else(|| capture.frames.back())
    }

    /// While following, step the view to the newest frame every
    /// [`FOLLOW_EVERY`], keeping its zoom and its offset into the frame.
    fn follow_newest(&mut self, capture: &Capture) {
        let Some(frame) = capture.frames.back() else { return };
        if !self.view.follow || frame.start_ns == self.view.followed_start_ns {
            return;
        }
        if self.followed_at.is_some_and(|t| t.elapsed() < FOLLOW_EVERY) {
            return;
        }
        self.followed_at = Some(Instant::now());
        if self.view.followed_start_ns == 0 {
            // The first frame: fit it, which also sets a sensible zoom.
            fit_frame(&mut self.view, frame);
            return;
        }
        let offset = self.view.start_ns as i64 - self.view.followed_start_ns as i64;
        self.view.start_ns = (frame.start_ns as i64 + offset).max(0) as u64;
        self.view.followed_start_ns = frame.start_ns;
    }

    fn stats(&self, capture: &Capture) -> String {
        let Some(frame) = self.focus(capture) else { return String::new() };
        let gpu_ms: f32 = frame.gpu.iter().filter(|p| p.depth == 0).map(|p| p.ms).sum();
        let mut threads: Vec<u32> = frame.spans.iter().map(|s| s.thread).collect();
        threads.sort_unstable();
        threads.dedup();
        let mut parts = vec![
            format!("Frame {}", frame.index),
            format!("CPU {:.2} ms", frame_ms(frame)),
            if gpu_ms > 0.0 { format!("GPU {gpu_ms:.2} ms") } else { "GPU not timed".into() },
            format!("{} spans", frame.spans.len()),
        ];
        if let Some(n) = frame.counters.iter().find(|c| capture.name(c.name) == "Entities").map(|c| c.value) {
            parts.push(format!("{n:.0} entities"));
        }
        parts.push(format!("{} threads", threads.len()));
        parts.join("  ·  ")
    }

    fn frame_graph(&mut self, ui: &mut egui::Ui, capture: &Capture) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, PANEL);
        if capture.frames.is_empty() {
            empty(&painter, rect, "No frames yet. Pick a game to profile, then press Record.");
            return;
        }
        let frames = visible_frames(capture, rect.width());
        let max_ms = frames.iter().map(|f| frame_ms(f)).fold(BUDGET_MS * 1.5, f32::max);
        let scale = rect.height() / max_ms;
        let selected = self.selected.or(frames.last().map(|f| f.index));
        let view_end = self.view.start_ns + self.view.width_ns;
        for (i, frame) in frames.iter().enumerate() {
            let ms = frame_ms(frame);
            let h = (ms * scale).max(1.0);
            let x = rect.left() + i as f32 * (BAR + GAP);
            if frame.end_ns > self.view.start_ns && frame.start_ns < view_end {
                painter.rect_filled(Rect::from_min_size(Pos2::new(x - 1.0, rect.top()), egui::vec2(BAR + 2.0, rect.height())), 0.0, HOVER);
            }
            let overlapped = capture.overlapped.contains(&frame.index);
            let color = if Some(frame.index) == selected {
                WARN
            } else if overlapped {
                // Recorded while a companion job ran: its time is not the
                // game's alone, so it is greyed rather than judged.
                OVERLAPPED
            } else if ms > BUDGET_MS {
                SLOW
            } else {
                ACCENT
            };
            painter.rect_filled(Rect::from_min_size(Pos2::new(x, rect.bottom() - h), egui::vec2(BAR, h)), 0.0, color);
            if overlapped {
                painter.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top()), egui::vec2(BAR, 3.0)), 0.0, OVERLAPPED_STRIP);
            }
        }
        let line_y = rect.bottom() - BUDGET_MS * scale;
        painter.line_segment([Pos2::new(rect.left(), line_y), Pos2::new(rect.right(), line_y)], Stroke::new(1.0, MUTED));
        painter.text(Pos2::new(rect.right() - 6.0, line_y - 2.0), Align2::RIGHT_BOTTOM, format!("{BUDGET_MS} ms"), FontId::proportional(11.0), MUTED);

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let at = ((pos.x - rect.left()) / (BAR + GAP)).max(0.0) as usize;
                if let Some(frame) = frames.get(at.min(frames.len().saturating_sub(1))) {
                    self.selected = Some(frame.index);
                    fit_frame(&mut self.view, frame);
                    self.view.follow = false;
                }
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, capture: &Capture) {
        let origin = capture.frames.front().map_or(0.0, |f| f.start_ns as f64 / 1e6);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Timeline").strong());
            ui.monospace(
                RichText::new(format!(
                    "{} to {}  ·  {} shown",
                    fmt_ms(self.view.start_ns as f64 / 1e6 - origin),
                    fmt_ms((self.view.start_ns + self.view.width_ns) as f64 / 1e6 - origin),
                    fmt_ms(self.view.width_ns as f64 / 1e6)
                ))
                .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let follow = egui::Button::selectable(self.view.follow, "Follow");
                if ui.add(follow).clicked() {
                    self.selected = None;
                    if let Some(frame) = capture.frames.back() {
                        // Onto the newest frame now, at the zoom already chosen.
                        let offset = self.view.start_ns as i64 - self.view.followed_start_ns as i64;
                        self.view.start_ns = (frame.start_ns as i64 + offset).max(0) as u64;
                        self.view.followed_start_ns = frame.start_ns;
                    }
                    self.view.follow = true;
                }
                if ui.button("3 frames").clicked() {
                    if let Some(frame) = self.focus(capture) {
                        let before = capture.frame(frame.index.saturating_sub(1)).unwrap_or(frame);
                        let after = capture.frame(frame.index + 1).unwrap_or(frame);
                        self.view.start_ns = before.start_ns;
                        self.view.width_ns = after.end_ns.saturating_sub(before.start_ns).max(1);
                        self.view.followed_start_ns = before.start_ns;
                        self.view.follow = false;
                    }
                }
                if ui.button("Fit frame").clicked() {
                    if let Some(frame) = self.focus(capture) {
                        fit_frame(&mut self.view, frame);
                        self.view.follow = self.selected.is_none();
                    }
                }
                if ui.button(">").on_hover_text("Pan right").clicked() {
                    pan(&mut self.view, 0.25);
                    self.view.follow = false;
                }
                // Zooming keeps following: it is how you pick how much of
                // each frame to watch.
                if ui.button("+").on_hover_text("Zoom in").clicked() {
                    zoom(&mut self.view, 0.5, 0.5);
                }
                if ui.button("-").on_hover_text("Zoom out").clicked() {
                    zoom(&mut self.view, 2.0, 0.5);
                }
                if ui.button("<").on_hover_text("Pan left").clicked() {
                    pan(&mut self.view, -0.25);
                    self.view.follow = false;
                }
                ui.label(RichText::new("wheel: zoom   drag: pan").color(MUTED));
            });
        });
    }

    fn timeline(&mut self, ui: &mut egui::Ui, capture: &Capture) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.timeline_input(ui, rect, &response);
        let painter = ui.painter_at(rect);
        draw_timeline(&painter, rect, capture, &self.view);
    }

    /// Wheel zoom around the pointer, Shift+wheel and drag to pan, Ctrl+wheel
    /// or the wheel over the thread names to scroll the lanes.
    fn timeline_input(&mut self, ui: &egui::Ui, rect: Rect, response: &egui::Response) {
        let track_w = (rect.width() - LANE_LABEL_W).max(1.0) as f64;
        if let Some(pointer) = response.hover_pos() {
            let anchor = ((pointer.x - rect.left() - LANE_LABEL_W) as f64 / track_w).clamp(0.0, 1.0);
            // Over the thread names there is no time under the pointer to
            // zoom around, so the wheel scrolls the lanes there.
            let over_names = pointer.x < rect.left() + LANE_LABEL_W;
            let wheels: Vec<(egui::Vec2, egui::Modifiers)> = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { delta, modifiers, .. } => Some((*delta, *modifiers)),
                        _ => None,
                    })
                    .collect()
            });
            for (delta, modifiers) in wheels {
                let amount = delta.y + delta.x;
                if amount == 0.0 {
                    continue;
                }
                if modifiers.ctrl || modifiers.command || over_names {
                    if delta.y > 0.0 {
                        self.view.first_lane = self.view.first_lane.saturating_sub(1);
                    } else if delta.y < 0.0 {
                        self.view.first_lane += 1;
                    }
                } else if modifiers.shift {
                    pan(&mut self.view, if amount > 0.0 { -0.15 } else { 0.15 });
                    self.view.follow = false;
                } else {
                    zoom(&mut self.view, if amount > 0.0 { 0.8 } else { 1.25 }, anchor);
                }
            }
        }
        if response.dragged() {
            let delta = response.drag_delta();
            if delta.x != 0.0 {
                let ns_per_px = self.view.width_ns as f64 / track_w;
                self.view.start_ns = (self.view.start_ns as f64 - delta.x as f64 * ns_per_px).max(0.0) as u64;
                self.view.follow = false;
            }
            // Dragging moves up and down too, a lane per LANE_DRAG_PX.
            self.drag_lanes += delta.y;
            while self.drag_lanes >= LANE_DRAG_PX {
                self.drag_lanes -= LANE_DRAG_PX;
                self.view.first_lane = self.view.first_lane.saturating_sub(1);
            }
            while self.drag_lanes <= -LANE_DRAG_PX {
                self.drag_lanes += LANE_DRAG_PX;
                self.view.first_lane += 1;
            }
        } else {
            self.drag_lanes = 0.0;
        }
    }

    fn slowest(&self, ui: &mut egui::Ui, capture: &Capture) {
        ui.label(RichText::new("Slowest in view").color(MUTED));
        let rows = slowest_in(capture, self.view.start_ns, self.view.start_ns + self.view.width_ns, SLOWEST_ROWS);
        ui.columns(3, |columns| {
            for (i, (name, ms, runs)) in rows.iter().enumerate() {
                let column = &mut columns[i / (SLOWEST_ROWS / 3)];
                // Cut to what fits beside the numbers in a third of the window.
                const ROOM: usize = 28;
                let name = short(capture.name(*name));
                let name: String = if name.chars().count() > ROOM {
                    name.chars().take(ROOM - 1).chain(std::iter::once('…')).collect()
                } else {
                    name.to_string()
                };
                column.monospace(RichText::new(format!("{ms:>7.2} ms  x{runs:<3} {name}")).size(12.0));
            }
        });
    }
}

fn fit_frame(view: &mut TimelineView, frame: &ProfileFrame) {
    let len = frame.end_ns.saturating_sub(frame.start_ns).max(1);
    let pad = len / 40;
    view.start_ns = frame.start_ns.saturating_sub(pad);
    view.width_ns = len + pad * 2;
    view.followed_start_ns = frame.start_ns;
}

/// Zoom by `factor` keeping the time under `anchor` (0..1 across) in place.
fn zoom(view: &mut TimelineView, factor: f64, anchor: f64) {
    let at = view.start_ns as f64 + view.width_ns as f64 * anchor;
    // 10 µs to 10 s across.
    let width = (view.width_ns as f64 * factor).clamp(10_000.0, 10_000_000_000.0);
    view.start_ns = (at - width * anchor).max(0.0) as u64;
    view.width_ns = width as u64;
}

fn pan(view: &mut TimelineView, fraction: f64) {
    let delta = view.width_ns as f64 * fraction;
    view.start_ns = (view.start_ns as f64 + delta).max(0.0) as u64;
}

/// The newest frames that fit `width` points of graph, oldest first.
fn visible_frames(capture: &Capture, width: f32) -> Vec<&ProfileFrame> {
    let n = ((width / (BAR + GAP)).floor() as usize).max(1);
    let skip = capture.frames.len().saturating_sub(n);
    capture.frames.iter().skip(skip).collect()
}

/// One muted line in the middle: what a view shows before there is any data.
fn empty(painter: &egui::Painter, rect: Rect, message: &str) {
    painter.text(rect.center(), Align2::CENTER_CENTER, message, FontId::proportional(14.0), MUTED);
}

/// What colour a span is, from its name: a rough grouping so the eye can find
/// physics or rendering in a busy frame.
fn span_color(name: &str) -> Color32 {
    let n = name.to_ascii_lowercase();
    if ["render", "pass", "extract", "queue", "prepare", "pipeline", "command_buffer"].iter().any(|k| n.contains(k)) {
        Color32::from_rgb(106, 79, 181)
    } else if ["avian", "rapier", "physics", "collision", "solver"].iter().any(|k| n.contains(k)) {
        Color32::from_rgb(35, 120, 110)
    } else if n.contains("script") {
        Color32::from_rgb(154, 101, 22)
    } else if ["ui", "ember", "text", "layout"].iter().any(|k| n.contains(k)) {
        Color32::from_rgb(148, 65, 111)
    } else {
        Color32::from_rgb(59, 93, 170)
    }
}

fn draw_timeline(painter: &egui::Painter, rect: Rect, capture: &Capture, view: &TimelineView) {
    painter.rect_filled(rect, 4.0, PANEL);
    if capture.frames.is_empty() {
        empty(painter, rect, "Each thread's systems appear here, frame by frame.");
        return;
    }
    let label = |pos: Pos2, text: String, color: Color32| {
        painter.text(pos, Align2::LEFT_TOP, text, FontId::proportional(12.0), color);
    };
    let track_x = rect.left() + LANE_LABEL_W;
    let track_w = (rect.width() - LANE_LABEL_W).max(1.0);
    let end_ns = view.start_ns + view.width_ns;
    let px = |ns: u64| track_x + ((ns as f64 - view.start_ns as f64) / view.width_ns as f64 * track_w as f64) as f32;
    let origin = capture.frames.front().map_or(0, |f| f.start_ns);

    // Ruler, counted from the start of the frame the view begins in. Counted
    // from the start of the capture, a minute in, every tick of a 20 ms view
    // read "98.73 s" and the ruler said nothing.
    let anchor = capture.frames.iter().rev().find(|f| f.start_ns <= view.start_ns).map_or(origin, |f| f.start_ns);
    let step = tick_step(view.width_ns as f64 / 1e6);
    let mut t = ((view.start_ns as f64 - anchor as f64) / 1e6 / step).ceil() * step;
    while anchor as f64 + t * 1e6 <= end_ns as f64 {
        let x = px((anchor as f64 + t * 1e6).max(0.0) as u64);
        let text = if t.abs() < step / 2.0 { "0".to_string() } else { format!("+{}", fmt_ms(t)) };
        painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(1.0, FAINT));
        label(Pos2::new(x + 3.0, rect.top() + 2.0), text, MUTED);
        t += step;
    }

    // Frames in range, and the spans that overlap the view.
    const SLACK_NS: u64 = 1_000_000_000;
    let frames: Vec<&ProfileFrame> = capture
        .frames
        .iter()
        .filter(|f| f.end_ns > view.start_ns && f.start_ns < end_ns.saturating_add(SLACK_NS))
        .collect();
    for frame in &frames {
        if frame.start_ns >= view.start_ns && frame.start_ns <= end_ns {
            let x = px(frame.start_ns);
            painter.line_segment([Pos2::new(x, rect.top() + 16.0), Pos2::new(x, rect.bottom())], Stroke::new(1.0, MUTED));
            label(
                Pos2::new(x + 4.0, rect.bottom() - 16.0),
                format!("frame {} · {:.1} ms", frame.index, frame_ms(frame)),
                MUTED,
            );
        }
    }

    // One lane per thread with work in view: main, then render, then the
    // rest by name. Wrappers (the app update, `schedule: Main`, the render
    // sub-app) are left out: they span the whole frame, which the frame
    // markers already show, and stacked five deep they took most of every
    // lane before any work appeared.
    let mut threads: Vec<u32> = frames
        .iter()
        .flat_map(|f| f.spans.iter())
        .filter(|s| s.end_ns > view.start_ns && s.start_ns < end_ns)
        .map(|s| s.thread)
        .collect();
    threads.sort_unstable();
    threads.dedup();
    threads.sort_by_cached_key(|t| lane_order(capture.thread_name(*t)));
    let lanes: Vec<(u32, Vec<&Span>)> = threads
        .into_iter()
        .map(|thread| {
            let spans: Vec<&Span> = frames
                .iter()
                .flat_map(|f| f.spans.iter())
                .filter(|s| s.thread == thread && s.end_ns > view.start_ns && s.start_ns < end_ns)
                .filter(|s| !is_wrapper(capture.name(s.name)))
                .collect();
            (thread, spans)
        })
        .filter(|(_, spans)| !spans.is_empty())
        .collect();
    let first = view.first_lane.min(lanes.len().saturating_sub(1));
    let mut y = rect.top() + 22.0;
    if first > 0 {
        label(Pos2::new(rect.left() + 6.0, y), format!("{first} threads above: wheel over the names, or Ctrl + wheel"), MUTED);
        y += 18.0;
    }
    // Kept free at the bottom for the GPU lane and the "more threads" line.
    let gpu_room = ROW_H + 8.0 + 18.0 + 4.0;
    let lane_count = lanes.len();
    for (shown, (thread, spans)) in lanes.into_iter().enumerate().skip(first) {
        let base = spans.iter().map(|s| s.depth).min().unwrap_or(0);
        let depth = spans.iter().map(|s| (s.depth - base).min(MAX_DEPTH)).max().unwrap_or(0);
        let lane_h = ROW_H * (depth as f32 + 1.0) + 6.0;
        if y + lane_h > rect.bottom() - 18.0 - gpu_room {
            label(
                Pos2::new(rect.left() + 6.0, y),
                format!("{} threads below: wheel over the names, or Ctrl + wheel", lane_count - shown),
                MUTED,
            );
            y += 18.0;
            break;
        }
        painter.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), y), egui::vec2(rect.width(), lane_h)), 0.0, FAINT);
        painter.text(
            Pos2::new(rect.left() + 6.0, y + 3.0),
            Align2::LEFT_TOP,
            capture.thread_name(thread),
            FontId::proportional(13.0),
            Color32::WHITE,
        );
        // Where the last label in each row ends: spans can overlap in one row
        // (work stolen onto this thread inside a system), and their labels
        // printed over each other into an unreadable smear.
        let mut label_end = [f32::MIN; MAX_DEPTH as usize + 1];
        for span in spans {
            let d = (span.depth - base).min(MAX_DEPTH) as f32;
            let x0 = px(span.start_ns.max(view.start_ns)).max(track_x);
            let x1 = px(span.end_ns.min(end_ns));
            let w = x1 - x0;
            // Narrower than a third of a point: invisible either way, and at
            // wide zooms there are thousands of these.
            if w < 0.33 {
                continue;
            }
            let name = capture.name(span.name);
            let ry = y + 3.0 + d * ROW_H;
            painter.rect_filled(Rect::from_min_size(Pos2::new(x0, ry), egui::vec2(w.max(1.0), ROW_H - 2.0)), 0.0, span_color(name));
            if w > 56.0 {
                let ms = span.end_ns.saturating_sub(span.start_ns) as f64 / 1e6;
                let text = if w > 160.0 { format!("{}  {}", short(name), fmt_ms(ms)) } else { short(name).to_string() };
                let max_chars = ((w - 8.0) / CHAR_W).max(1.0) as usize;
                let text: String = text.chars().take(max_chars).collect();
                let row = d as usize;
                if x0 + 4.0 >= label_end[row] {
                    label_end[row] = x0 + 4.0 + text.chars().count() as f32 * CHAR_W;
                    label(Pos2::new(x0 + 4.0, ry + 3.0), text, TEXT_ON_BAR);
                }
            }
        }
        y += lane_h + 4.0;
    }

    // The GPU, as passes laid end to end: Bevy reports how long each pass
    // took, not when it started.
    if let Some(frame) = frames.iter().rev().find(|f| !f.gpu.is_empty() && f.start_ns <= end_ns) {
        if y + ROW_H + 8.0 < rect.bottom() - 18.0 {
            painter.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), y), egui::vec2(rect.width(), ROW_H + 6.0)), 0.0, FAINT);
            painter.text(Pos2::new(rect.left() + 6.0, y + 3.0), Align2::LEFT_TOP, "GPU (in order)", FontId::proportional(13.0), Color32::WHITE);
            let mut at = frame.start_ns;
            for pass in frame.gpu.iter().filter(|p| p.depth == 0) {
                let len = (pass.ms as f64 * 1e6) as u64;
                let x0 = px(at.max(view.start_ns)).max(track_x);
                let x1 = px((at + len).min(end_ns));
                if x1 - x0 >= 0.33 {
                    painter.rect_filled(Rect::from_min_size(Pos2::new(x0, y + 3.0), egui::vec2((x1 - x0).max(1.0), ROW_H - 2.0)), 0.0, GPU_BAR);
                    if x1 - x0 > 48.0 {
                        let max_chars = ((x1 - x0 - 8.0) / CHAR_W).max(1.0) as usize;
                        let text: String = short(capture.name(pass.name)).chars().take(max_chars).collect();
                        label(Pos2::new(x0 + 4.0, y + 5.0), text, TEXT_ON_BAR);
                    }
                }
                at += len;
            }
        }
    }
}

/// Where a thread's lane goes: main, render, then by name with the number in
/// it compared as a number, so "Pool (2)" comes before "Pool (10)".
fn lane_order(name: &str) -> (u8, String, u32) {
    let rank = match name {
        "main" => 0,
        n if n.starts_with("Render") => 1,
        _ => 2,
    };
    let number = name
        .split(|c: char| !c.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
        .unwrap_or(0);
    let text: String = name.chars().filter(|c| !c.is_ascii_digit()).collect();
    (rank, text, number)
}

/// A ruler step giving 5 to 10 ticks across `width_ms`.
fn tick_step(width_ms: f64) -> f64 {
    const STEPS: [f64; 13] = [0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0];
    STEPS.into_iter().find(|s| width_ms / s <= 10.0).unwrap_or(250.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_companion_proto::wire::GamePacket;

    #[test]
    fn zoom_keeps_the_anchor_in_place() {
        let mut view = TimelineView { start_ns: 1_000_000, width_ns: 20_000_000, follow: true, followed_start_ns: 1_000_000, first_lane: 0 };
        let anchor_ns = view.start_ns as f64 + view.width_ns as f64 * 0.25;
        zoom(&mut view, 0.5, 0.25);
        let after = view.start_ns as f64 + view.width_ns as f64 * 0.25;
        assert!((after - anchor_ns).abs() < 2.0);
        assert_eq!(view.width_ns, 10_000_000);
        assert!(view.follow, "zooming keeps following");
    }

    /// Following moves the view frame to frame without touching its zoom or
    /// where in the frame it is looking.
    #[test]
    fn following_keeps_zoom_and_offset() {
        let mut profiler = Profiler {
            view: TimelineView { start_ns: 1_004_000, width_ns: 2_000_000, follow: true, followed_start_ns: 1_000_000, first_lane: 0 },
            ..Profiler::default()
        };
        let mut capture = Capture::default();
        capture.receive(GamePacket::Frame(ProfileFrame { index: 0, start_ns: 9_000_000, end_ns: 25_000_000, ..Default::default() }), false);
        profiler.follow_newest(&capture);
        assert_eq!(profiler.view.width_ns, 2_000_000);
        assert_eq!(profiler.view.start_ns, 9_004_000);
        assert_eq!(profiler.view.followed_start_ns, 9_000_000);
    }

    #[test]
    fn lanes_put_main_and_render_first_and_count_pools_numerically() {
        let mut names = vec!["Compute Task Pool (10)", "Render thread", "Compute Task Pool (2)", "main", "IO Task Pool (0)"];
        names.sort_by_key(|n| lane_order(n));
        assert_eq!(names, ["main", "Render thread", "Compute Task Pool (2)", "Compute Task Pool (10)", "IO Task Pool (0)"]);
    }

    #[test]
    fn ticks_stay_readable_at_any_zoom() {
        for width in [0.05, 0.3, 2.0, 16.6, 60.0, 900.0] {
            let n = width / tick_step(width);
            assert!((0.5..=10.0).contains(&n), "width {width} ms gave {n} ticks");
        }
    }
}
