//! The Render screen: the render graph as nodes, in the order they ran.
//!
//! Bevy 0.20's render graph is a set of schedules (`Core3d`, `Core2d`, a
//! camera's custom one), run once per camera. Their systems run inside a
//! `schedule: Core3d` span, so the recorded spans already say which pass ran,
//! in what order, for how long, for every camera. Nothing extra is sent: this
//! screen reads the selected frame, finds each camera's schedule span, and
//! lays its passes out as nodes, numbered in run order and coloured by time
//! (GPU time where Bevy measured the pass, CPU time otherwise).
//!
//! Ported from the engine companion's `ui::render_graph`. The finding of the
//! passes is unchanged; the drawing is egui painter calls instead of ember's
//! node graph widget.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke};

use renzora_companion_proto::frame::{ProfileFrame, Span};

use super::profiler::Profiler;
use super::{short, MUTED, WARN};
use crate::capture::Capture;

/// Spacing of passes that ran at once, side by side in a row.
const COL_W: f32 = 250.0;
const ROW_H: f32 = 80.0;
const NODE_W: f32 = 220.0;
const NODE_H: f32 = 44.0;
/// Frames looked back over to decide which systems belong to a render
/// schedule. Enough that a main-world system has run outside the schedule's
/// window at least once, few enough to stay cheap once a second.
const CLASSIFY_FRAMES: usize = 120;
/// Live, the graph is worked out again this often: one that rebuilds every
/// frame cannot be read.
const REFRESH: Duration = Duration::from_secs(1);

/// One pass as the screen shows it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pass {
    pub name: String,
    pub cpu_ms: f64,
    pub gpu_ms: Option<f32>,
    pub start_ns: u64,
    pub end_ns: u64,
}

/// One camera's run of a render schedule.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScheduleRun {
    pub schedule: String,
    pub passes: Vec<Pass>,
}

#[derive(Default)]
pub(super) struct RenderGraph {
    runs: Vec<ScheduleRun>,
    camera: usize,
    selected: Option<usize>,
    /// When the runs were worked out, and for which selected frame.
    refreshed: Option<(Instant, Option<u64>)>,
    /// Scroll the graph to its middle on the next draw: a wide graph opened
    /// at its left edge, showing the first row's empty margin.
    center: bool,
}

impl RenderGraph {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, capture: &Capture, profiler: &Profiler, live: bool) {
        self.refresh(capture, profiler.selected, live);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Render passes in run order").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(self.runs.len() > 1, egui::Button::new("Next camera")).clicked() {
                    self.camera = (self.camera + 1) % self.runs.len();
                    self.selected = None;
                    self.center = true;
                }
            });
        });
        let line = match self.runs.get(self.camera) {
            Some(run) => format!(
                "{} · camera {} of {} · {} passes · fill = GPU time where measured, CPU time otherwise",
                run.schedule,
                self.camera + 1,
                self.runs.len(),
                run.passes.len()
            ),
            None => "No render schedule in the recorded frames yet. Connect to a game and record.".to_string(),
        };
        ui.label(RichText::new(line).color(MUTED));
        let Some(run) = self.runs.get(self.camera) else { return };
        let detail = match self.selected.and_then(|i| run.passes.get(i).map(|p| (i, p))) {
            Some((i, pass)) => format!(
                "{} of {}  ·  {}  ·  CPU {:.3} ms  ·  GPU {}  ·  {}",
                i + 1,
                run.passes.len(),
                short(&pass.name),
                pass.cpu_ms,
                pass.gpu_ms.map_or("not timed".into(), |g| format!("{g:.3} ms")),
                pass.name,
            ),
            None => "Click a node to see its times".into(),
        };
        ui.monospace(detail);
        ui.add_space(4.0);

        let columns = waves(&run.passes);
        // The longest pass of each wave: the one the next wave waited for, so
        // the wires between them trace the run's critical path.
        let longest: Vec<usize> = columns
            .iter()
            .map(|wave| *wave.iter().max_by(|a, b| run.passes[**a].cpu_ms.total_cmp(&run.passes[**b].cpu_ms)).unwrap_or(&wave[0]))
            .collect();
        // Top to bottom: one row per wave, its passes side by side and
        // centred, so the frame reads downward in run order and a wide wave
        // is plainly work that ran at once.
        let widest = columns.iter().map(Vec::len).max().unwrap_or(1) as f32;
        let size = egui::vec2(40.0 + widest * COL_W, 30.0 + columns.len() as f32 * ROW_H);
        let mut clicked = None;
        let mut scroll = egui::ScrollArea::both().auto_shrink([false, false]);
        if std::mem::take(&mut self.center) {
            scroll = scroll.horizontal_scroll_offset(((size.x - ui.available_width()) * 0.5).max(0.0));
        }
        scroll.show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            let painter = ui.painter_at(rect);
            let mut boxes: HashMap<usize, Rect> = HashMap::new();
            for (row, wave) in columns.iter().enumerate() {
                let left = rect.left() + 40.0 + (widest - wave.len() as f32) * COL_W * 0.5;
                let y = rect.top() + 30.0 + row as f32 * ROW_H;
                for (col, &i) in wave.iter().enumerate() {
                    boxes.insert(i, Rect::from_min_size(Pos2::new(left + col as f32 * COL_W, y), egui::vec2(NODE_W, NODE_H)));
                }
            }
            for (row, wave) in columns.iter().enumerate().skip(1) {
                let Some(from) = boxes.get(&longest[row - 1]) else { continue };
                for i in wave {
                    if let Some(to) = boxes.get(i) {
                        painter.line_segment([from.center_bottom(), to.center_top()], Stroke::new(1.5, MUTED));
                    }
                }
            }
            for (i, pass) in run.passes.iter().enumerate() {
                let Some(node) = boxes.get(&i) else { continue };
                let ms = pass.gpu_ms.map_or(pass.cpu_ms, f64::from);
                painter.rect_filled(*node, 6.0, heat(ms));
                if self.selected == Some(i) {
                    painter.rect_stroke(*node, 6.0, Stroke::new(2.0, WARN), egui::StrokeKind::Outside);
                }
                let title: String = format!("{} · {}", i + 1, short(&pass.name)).chars().take(30).collect();
                painter.text(node.left_top() + egui::vec2(8.0, 6.0), Align2::LEFT_TOP, title, FontId::proportional(13.0), Color32::WHITE);
                painter.text(
                    node.left_bottom() + egui::vec2(8.0, -6.0),
                    Align2::LEFT_BOTTOM,
                    format!("{ms:.3} ms {}", if pass.gpu_ms.is_some() { "GPU" } else { "CPU" }),
                    FontId::monospace(11.0),
                    Color32::from_gray(210),
                );
            }
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    clicked = Some(boxes.iter().find(|(_, r)| r.contains(pos)).map(|(i, _)| *i));
                }
            }
        });
        if let Some(pick) = clicked {
            self.selected = pick;
        }
    }

    /// Work the runs out again for the selected frame, or the newest while
    /// live, at most once a [`REFRESH`].
    fn refresh(&mut self, capture: &Capture, selected: Option<u64>, live: bool) {
        let stale = match self.refreshed {
            None => true,
            Some((at, for_frame)) => for_frame != selected || (live && selected.is_none() && at.elapsed() >= REFRESH),
        };
        if !stale {
            return;
        }
        self.refreshed = Some((Instant::now(), selected));
        // The newest frame that ran a render schedule, at or before the
        // selection.
        let newest = capture
            .frames
            .iter()
            .rev()
            .filter(|f| selected.is_none_or(|w| f.index <= w))
            .find(|f| f.spans.iter().any(|s| capture.name(s.name).starts_with("schedule: Core")))
            .map(|f| f.index);
        let runs = match newest {
            Some(newest) => {
                let mut frames: Vec<&ProfileFrame> =
                    capture.frames.iter().rev().filter(|f| f.index <= newest).take(CLASSIFY_FRAMES).collect();
                frames.reverse();
                schedule_runs(capture, &frames)
            }
            None => Vec::new(),
        };
        if runs != self.runs {
            // A different set of passes: an index into the old one means
            // nothing now, and the graph has a new shape to open on.
            if runs.get(self.camera).map(|r| r.passes.len()) != self.runs.get(self.camera).map(|r| r.passes.len()) {
                self.selected = None;
                self.center = true;
            }
            self.runs = runs;
            if self.camera >= self.runs.len() {
                self.camera = 0;
            }
        }
    }
}

/// A heat colour for a pass's time: dark blue for nothing to rust for a lot.
fn heat(ms: f64) -> Color32 {
    match ms {
        m if m < 0.1 => Color32::from_rgb(36, 44, 62),
        m if m < 0.5 => Color32::from_rgb(36, 54, 95),
        m if m < 1.5 => Color32::from_rgb(47, 76, 143),
        m if m < 4.0 => Color32::from_rgb(122, 90, 38),
        _ => Color32::from_rgb(154, 58, 42),
    }
}

/// The render schedule runs in `frame`: one per camera per schedule.
fn runs_in(capture: &Capture, frame: &ProfileFrame) -> Vec<(Span, String)> {
    let mut runs: Vec<(Span, String)> = frame
        .spans
        .iter()
        .filter_map(|s| {
            let schedule = capture.name(s.name).strip_prefix("schedule: ")?;
            matches!(schedule, "Core3d" | "Core2d").then(|| (*s, schedule.to_string()))
        })
        .collect();
    runs.sort_by_key(|(s, _)| s.start_ns);
    runs
}

/// Bookkeeping spans that are not passes: the executor, its condition checks
/// and the command flushes between systems.
fn is_machinery(name: &str) -> bool {
    name == "apply"
        || name.contains("executor")
        || name.starts_with("check_conditions")
        || name.ends_with("(commands)")
        || name.contains("apply_deferred")
        || name == "par_for_each"
}

/// The span names that belong to a render schedule: across `frames`, every run
/// of them fell inside a `Core3d`/`Core2d` window, and at least one did.
///
/// The schedule's systems run on the task pool's threads, not the thread the
/// schedule span is on, and nothing in a span says which world or schedule its
/// system belongs to. With pipelined rendering the main world's systems run on
/// the same threads at the same time, so overlapping the window in one frame
/// proves nothing. Over a hundred frames a main-world system also runs outside
/// every window, and a render system never does.
fn render_schedule_names(capture: &Capture, frames: &[&ProfileFrame]) -> HashSet<u32> {
    let mut windows: Vec<(u64, u64)> =
        frames.iter().flat_map(|f| runs_in(capture, f)).map(|(s, _)| (s.start_ns, s.end_ns)).collect();
    windows.sort_unstable();
    let inside = |s: &Span| {
        let at = windows.partition_point(|w| w.0 <= s.start_ns);
        at > 0 && s.end_ns <= windows[at - 1].1
    };
    let mut seen: HashMap<u32, (u32, u32)> = HashMap::new();
    for span in frames.iter().flat_map(|f| f.spans.iter()) {
        let entry = seen.entry(span.name).or_default();
        if inside(span) {
            entry.0 += 1;
        } else {
            entry.1 += 1;
        }
    }
    seen.into_iter()
        .filter(|(name, (inn, out))| {
            *inn > 0 && *out == 0 && !is_machinery(capture.name(*name)) && !capture.name(*name).starts_with("schedule: ")
        })
        .map(|(name, _)| name)
        .collect()
}

/// Every render schedule run in the newest of `frames`, in the order they
/// started, with the passes that ran in each, on whatever thread.
pub(crate) fn schedule_runs(capture: &Capture, frames: &[&ProfileFrame]) -> Vec<ScheduleRun> {
    let Some(frame) = frames.last() else { return Vec::new() };
    let render = render_schedule_names(capture, frames);
    // Per camera (the k-th run of a frame), every run's passes across the
    // frames, as offsets from the run's start.
    let cameras = runs_in(capture, frame);
    let mut seen: Vec<HashMap<u32, Vec<(u64, u64)>>> = vec![HashMap::new(); cameras.len()];
    let mut run_counts = vec![0usize; cameras.len()];
    for (index, f) in frames.iter().enumerate() {
        // A span finishing just after the frame mark is stored in the next
        // frame, so each frame's runs look at its own spans and the next's.
        let nearby: Vec<&Span> = frames[index..frames.len().min(index + 2)].iter().flat_map(|f| f.spans.iter()).collect();
        for (k, (run, _)) in runs_in(capture, f).into_iter().enumerate().take(cameras.len()) {
            run_counts[k] += 1;
            for span in outer_passes(&nearby, &render, &run) {
                seen[k].entry(span.name).or_default().push((span.start_ns - run.start_ns, span.end_ns - run.start_ns));
            }
        }
    }
    cameras
        .into_iter()
        .enumerate()
        .map(|(k, (_, schedule))| {
            // In at least half the runs: a pass that ran once is noise, and
            // one flickering in and out would rebuild the graph each time.
            let mut passes: Vec<(u64, u64, u32, f64)> = seen[k]
                .iter()
                .filter(|(_, runs)| runs.len() * 2 >= run_counts[k].max(1))
                .map(|(name, runs)| {
                    let mut starts: Vec<u64> = runs.iter().map(|r| r.0).collect();
                    let mut ends: Vec<u64> = runs.iter().map(|r| r.1).collect();
                    starts.sort_unstable();
                    ends.sort_unstable();
                    let cpu = runs.iter().map(|r| r.1.saturating_sub(r.0)).sum::<u64>() as f64 / runs.len() as f64 / 1e6;
                    (starts[starts.len() / 2], ends[ends.len() / 2], *name, cpu)
                })
                .collect();
            // By median start, rounded so two passes a few microseconds apart
            // do not trade places from one refresh to the next; then by name.
            const STEADY_NS: u64 = 50_000;
            passes.sort_by_key(|(start, _, name, _)| (start / STEADY_NS, capture.name(*name).to_string()));
            ScheduleRun {
                schedule,
                passes: passes
                    .into_iter()
                    .map(|(start_ns, end_ns, name, cpu_ms)| {
                        let name = capture.name(name).to_string();
                        Pass { gpu_ms: gpu_for(capture, frame, &name), cpu_ms, start_ns, end_ns, name }
                    })
                    .collect(),
            }
        })
        .collect()
}

/// The passes of one schedule `run`: render-schedule spans inside its time,
/// without the spans nested inside them (a span inside a system is part of
/// that pass, not a pass of its own).
fn outer_passes<'a>(spans: &[&'a Span], render: &HashSet<u32>, run: &Span) -> Vec<&'a Span> {
    let mut inside: Vec<&Span> = spans
        .iter()
        .copied()
        .filter(|s| render.contains(&s.name) && s.start_ns >= run.start_ns && s.end_ns <= run.end_ns)
        .collect();
    inside.sort_by_key(|s| (s.start_ns, s.depth));
    let mut outer: Vec<&Span> = Vec::new();
    for span in inside {
        let nested = outer.iter().any(|o| o.thread == span.thread && o.start_ns <= span.start_ns && span.end_ns <= o.end_ns);
        if !nested {
            outer.push(span);
        }
    }
    outer
}

/// Group passes into waves that ran at the same time: a pass joins the
/// current wave while it starts before every pass in the wave has finished.
pub(crate) fn waves(passes: &[Pass]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut wave_end = 0u64;
    for (i, pass) in passes.iter().enumerate() {
        match out.last_mut() {
            Some(wave) if pass.start_ns < wave_end => {
                wave.push(i);
                wave_end = wave_end.max(pass.end_ns);
            }
            _ => {
                out.push(vec![i]);
                wave_end = pass.end_ns;
            }
        }
    }
    out
}

/// The GPU time Bevy measured for the pass a system records, if its pass name
/// matches the system's own (`main_opaque_pass_3d` for
/// `...::main_opaque_pass_3d`). Passes and systems are named independently,
/// so an unmatched system is simply not timed on the GPU here.
fn gpu_for(capture: &Capture, frame: &ProfileFrame, system: &str) -> Option<f32> {
    let leaf = short(system);
    frame
        .gpu
        .iter()
        .find(|p| {
            let pass = capture.name(p.name);
            pass.rsplit('/').next().unwrap_or(pass) == leaf
        })
        .map(|p| p.ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_companion_proto::frame::GpuPass;
    use renzora_companion_proto::wire::GamePacket;

    fn span(thread: u32, name: u32, start_ns: u64, end_ns: u64, depth: u16) -> Span {
        Span { thread, name, start_ns, end_ns, depth }
    }

    /// The schedule span is on the render thread; its systems run on pool
    /// threads, beside a main-world system that overlaps the window in one
    /// frame and runs outside it in the other.
    #[test]
    fn passes_are_found_on_any_thread_and_main_world_systems_are_not() {
        let mut capture = Capture::default();
        capture.receive(
            GamePacket::Names(vec![
                (0, "schedule: Core3d".into()),
                (1, "bevy_core_pipeline::core_3d::main_opaque_pass_3d".into()),
                (2, "bevy_core_pipeline::tonemapping::tonemapping".into()),
                (3, "apply".into()),
                (4, "main_opaque_pass_3d".into()),
                (5, "game::ai".into()),
                (6, "multithreaded executor".into()),
            ]),
            false,
        );
        let frame = |base: u64, ai_at: u64| ProfileFrame {
            spans: vec![
                span(1, 0, base, base + 100, 2),
                span(1, 6, base, base + 100, 3),
                span(1, 3, base + 52, base + 53, 4),
                span(2, 1, base + 10, base + 50, 0),
                span(3, 2, base + 60, base + 80, 0),
                span(4, 5, ai_at, ai_at + 3, 0),
            ],
            gpu: vec![GpuPass { name: 4, ms: 6.9, depth: 0 }],
            ..Default::default()
        };
        let a = frame(0, 55);
        let b = frame(1000, 1200);
        let runs = schedule_runs(&capture, &[&a, &b]);
        assert_eq!(runs.len(), 1);
        let names: Vec<&str> = runs[0].passes.iter().map(|p| short(&p.name)).collect();
        assert_eq!(names, vec!["main_opaque_pass_3d", "tonemapping"]);
        assert_eq!(runs[0].passes[0].gpu_ms, Some(6.9));
        assert_eq!(runs[0].passes[1].gpu_ms, None);
    }

    /// Two passes that start microseconds apart swap places from frame to
    /// frame; the graph built from them must not swap with them.
    #[test]
    fn near_simultaneous_passes_keep_one_order() {
        let mut capture = Capture::default();
        capture.receive(
            GamePacket::Names(vec![
                (0, "schedule: Core3d".into()),
                (1, "game::render::b_pass".into()),
                (2, "game::render::a_pass".into()),
            ]),
            false,
        );
        let frame = |base: u64, a_first: bool| {
            let (a, b) = if a_first { (base + 10_000, base + 12_000) } else { (base + 12_000, base + 10_000) };
            ProfileFrame {
                spans: vec![span(1, 0, base, base + 100_000, 2), span(2, 2, a, a + 5_000, 0), span(3, 1, b, b + 5_000, 0)],
                ..Default::default()
            }
        };
        let order = |frames: &[ProfileFrame]| {
            let refs: Vec<&ProfileFrame> = frames.iter().collect();
            schedule_runs(&capture, &refs)[0].passes.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
        };
        let one = order(&[frame(0, true), frame(1_000_000, false), frame(2_000_000, true)]);
        let two = order(&[frame(0, false), frame(1_000_000, true), frame(2_000_000, false)]);
        assert_eq!(one, two);
        assert_eq!(one, ["game::render::a_pass", "game::render::b_pass"]);
    }

    #[test]
    fn passes_that_overlap_share_a_wave() {
        let pass = |start_ns, end_ns| Pass { name: String::new(), cpu_ms: 0.0, gpu_ms: None, start_ns, end_ns };
        let passes = [pass(0, 10), pass(2, 6), pass(8, 12), pass(12, 20), pass(21, 22)];
        assert_eq!(waves(&passes), vec![vec![0, 1, 2], vec![3], vec![4]]);
    }
}
