//! The Render screen: the render graph as nodes, in the order they ran.
//!
//! Bevy 0.20's render graph is a set of schedules (`Core3d`, `Core2d`, a
//! camera's custom one), run once per camera. Their systems run inside a
//! `schedule: Core3d` span, so the recorded spans already say which pass ran,
//! in what order, for how long, for every camera. Nothing extra is sent: this
//! screen reads the selected frame, finds each camera's schedule span, and
//! lays its passes out as a chain of nodes, numbered in run order and coloured
//! by time (GPU time where Bevy measured the pass, CPU time otherwise).

use bevy::prelude::*;
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;
use renzora_ember::widgets::{graph_node_view, graph_wire_view, node_graph_view, GraphEdit, NodeGraphView};
use renzora_profiler::{ProfileFrame, Span};

use super::profiler::SelectedFrame;
use super::{card, mono, text, LiveView, Page};
use crate::app::capture::Capture;

/// Spacing of passes that ran at once, side by side in a row.
const COL_W: f32 = 250.0;
const ROW_H: f32 = 80.0;

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

/// What is on screen, so the graph is rebuilt only when it changes.
#[derive(Resource, Default)]
struct Shown {
    runs: Vec<ScheduleRun>,
    camera: usize,
    signature: u64,
    selected: Option<u64>,
}

#[derive(Component)]
struct GraphHost;
/// The node graph currently shown, despawned when the graph is rebuilt.
#[derive(Component)]
struct GraphViewport;
#[derive(Component)]
struct CameraLine;
/// The "Next camera" button.
#[derive(Component)]
struct CameraCycle;
#[derive(Component)]
struct DetailText;

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Shown>()
        .add_systems(Update, (cycle_camera, collect_runs, rebuild_graph, follow_selection, details).chain());
}

/// Step to the next camera's run of the schedule.
fn cycle_camera(
    pressed: Query<&renzora_ember::UiInteraction, (Changed<renzora_ember::UiInteraction>, With<CameraCycle>)>,
    mut shown: ResMut<Shown>,
) {
    if pressed.iter().any(|i| *i == renzora_ember::UiInteraction::Pressed) && !shown.runs.is_empty() {
        shown.camera = (shown.camera + 1) % shown.runs.len();
        shown.selected = None;
    }
}

pub(crate) fn spawn(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let graph_card = card(commands);
    commands.entity(graph_card).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(10.0)),
        row_gap: Val::Px(8.0),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    });
    let title = text(commands, fonts, "Render passes in run order", 13.0, text_primary());
    let camera = text(commands, fonts, "", 11.0, text_muted());
    commands.entity(camera).insert(CameraLine);
    let next_camera = super::button(commands, fonts, Some("video-camera"), "Next camera", super::UiAction::Page(Page::Render));
    commands.entity(next_camera).insert(CameraCycle);
    let host = commands
        .spawn((Node { flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }, GraphHost))
        .id();
    let header = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() })
        .id();
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    commands.entity(header).add_children(&[title, spacer, next_camera]);
    // The selected pass, one line under the header: a side column for it took
    // a quarter of the window to show seven short lines.
    let detail = mono(commands, fonts, "Click a node to see its times", 12.0, text_muted());
    commands.entity(detail).insert(DetailText);
    commands.entity(graph_card).add_children(&[header, camera, detail, host]);

    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .id();
    commands.entity(root).add_child(graph_card);
    root
}

/// Frames looked back over to decide which systems belong to a render
/// schedule. Enough that a main-world system has run outside the schedule's
/// window at least once, few enough to stay cheap once a second.
const CLASSIFY_FRAMES: usize = 120;

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
fn render_schedule_names(capture: &Capture, frames: &[&ProfileFrame]) -> std::collections::HashSet<u32> {
    use std::collections::HashMap;
    let mut windows: Vec<(u64, u64)> = frames
        .iter()
        .flat_map(|f| runs_in(capture, f))
        .map(|(s, _)| (s.start_ns, s.end_ns))
        .collect();
    windows.sort_unstable();
    let inside = |s: &Span| {
        // The last window starting at or before the span.
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
        .filter(|(name, (inn, out))| *inn > 0 && *out == 0 && !is_machinery(capture.name(*name)) && !capture.name(*name).starts_with("schedule: "))
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
    let mut seen: Vec<std::collections::HashMap<u32, Vec<(u64, u64)>>> = vec![Default::default(); cameras.len()];
    let mut run_counts = vec![0usize; cameras.len()];
    for (index, f) in frames.iter().enumerate() {
        // A span finishing just after the frame mark is stored in the next
        // frame, so each frame's runs look at its own spans and the next's.
        let nearby: Vec<&Span> = frames[index..frames.len().min(index + 2)].iter().flat_map(|f| f.spans.iter()).collect();
        for (k, (run, _)) in runs_in(capture, f).into_iter().enumerate().take(cameras.len()) {
            run_counts[k] += 1;
            for span in outer_passes(&nearby, &render, &run) {
                seen[k]
                    .entry(span.name)
                    .or_default()
                    .push((span.start_ns - run.start_ns, span.end_ns - run.start_ns));
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
fn outer_passes<'a>(spans: &[&'a Span], render: &std::collections::HashSet<u32>, run: &Span) -> Vec<&'a Span> {
    let mut inside: Vec<&Span> = spans
        .iter()
        .copied()
        .filter(|s| render.contains(&s.name) && s.start_ns >= run.start_ns && s.end_ns <= run.end_ns)
        .collect();
    inside.sort_by_key(|s| (s.start_ns, s.depth));
    let mut outer: Vec<&Span> = Vec::new();
    for span in inside {
        let nested = outer
            .iter()
            .any(|o| o.thread == span.thread && o.start_ns <= span.start_ns && span.end_ns <= o.end_ns);
        if !nested {
            outer.push(span);
        }
    }
    outer
}

/// Group passes into waves that ran at the same time: a pass joins the
/// current wave while it starts before every pass in the wave has finished.
/// Waves are the graph's columns, left to right in run order.
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
/// `…::main_opaque_pass_3d`). Passes and systems are named independently, so
/// an unmatched system is simply not timed on the GPU here.
fn gpu_for(capture: &Capture, frame: &ProfileFrame, system: &str) -> Option<f32> {
    let leaf = super::short(system);
    frame
        .gpu
        .iter()
        .find(|p| {
            let pass = capture.name(p.name);
            let pass_leaf = pass.rsplit('/').next().unwrap_or(pass);
            pass_leaf == leaf
        })
        .map(|p| p.ms)
}

fn collect_runs(capture: Res<Capture>, selected: Res<SelectedFrame>, live: Res<LiveView>, page: Res<Page>, mut shown: ResMut<Shown>, time: Res<Time<Real>>, mut last: Local<f32>) {
    if *page != Page::Render {
        return;
    }
    let refresh = selected.is_changed() || page.is_changed() || (capture.is_changed() && live.0);
    let now = time.elapsed_secs();
    // Live, once a second: a graph that rebuilds every frame cannot be read.
    if !refresh || (!selected.is_changed() && !page.is_changed() && now - *last < 1.0) {
        return;
    }
    *last = now;
    // The newest frame that ran a render schedule, at or before the selection.
    let wanted = selected.0;
    let newest = capture
        .frames
        .iter()
        .rev()
        .filter(|f| wanted.is_none_or(|w| f.index <= w))
        .find(|f| f.spans.iter().any(|s| capture.name(s.name).starts_with("schedule: Core")))
        .map(|f| f.index);
    let runs = match newest {
        Some(newest) => {
            let mut frames: Vec<&ProfileFrame> = capture
                .frames
                .iter()
                .rev()
                .filter(|f| f.index <= newest)
                .take(CLASSIFY_FRAMES)
                .collect();
            frames.reverse();
            schedule_runs(&capture, &frames)
        }
        None => Vec::new(),
    };
    if runs != shown.runs {
        shown.runs = runs;
        if shown.camera >= shown.runs.len() {
            shown.camera = 0;
        }
    }
}

/// A heat colour for a pass's time: dark blue for nothing to rust for a lot.
fn heat(ms: f64) -> (u8, u8, u8) {
    match ms {
        m if m < 0.1 => (36, 44, 62),
        m if m < 0.5 => (36, 54, 95),
        m if m < 1.5 => (47, 76, 143),
        m if m < 4.0 => (122, 90, 38),
        _ => (154, 58, 42),
    }
}

/// Rebuild the node graph when the passes on screen changed.
fn rebuild_graph(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut shown: ResMut<Shown>,
    host: Query<Entity, With<GraphHost>>,
    viewports: Query<Entity, With<GraphViewport>>,
    mut camera_line: Query<&mut Text, With<CameraLine>>,
) {
    let Some(fonts) = fonts else { return };
    let Ok(host) = host.single() else { return };
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    shown.camera.hash(&mut hasher);
    for run in &shown.runs {
        run.schedule.hash(&mut hasher);
        // The passes and their order only. A rebuild makes a new view, which
        // opens at the top: keyed on each pass's colour band as well, a time
        // crossing a band threw the reader back to the top mid-scroll. Colours
        // follow the next time the passes themselves change (or Next camera).
        for pass in &run.passes {
            pass.name.hash(&mut hasher);
        }
    }
    let signature = hasher.finish();
    if signature == shown.signature && !viewports.is_empty() {
        return;
    }
    shown.signature = signature;

    for entity in &viewports {
        commands.entity(entity).despawn();
    }
    let line = match shown.runs.get(shown.camera) {
        Some(run) => format!(
            "{} · camera {} of {} · {} passes · fill = GPU time where measured, CPU time otherwise",
            run.schedule,
            shown.camera + 1,
            shown.runs.len(),
            run.passes.len()
        ),
        None => "No render schedule in the recorded frames yet. Connect to a game and record.".to_string(),
    };
    for mut t in &mut camera_line {
        t.0.clone_from(&line);
    }
    let Some(run) = shown.runs.get(shown.camera).cloned() else { return };

    let handle = node_graph_view(&mut commands, &fonts);
    commands.entity(handle.viewport).insert(GraphViewport);
    // Open at the top, as wide as the widest wave allows; the opening fit
    // would shrink the whole tall graph into the view.
    commands
        .entity(handle.viewport)
        .entry::<NodeGraphView>()
        .and_modify(|mut view| view.fit_width_request = true);
    commands.entity(host).add_child(handle.viewport);
    let port = |id: &str| (id.to_string(), String::new(), text_muted(), 0u32);
    let input = [port("in")];
    let output = [port("out")];
    let columns = waves(&run.passes);
    // The longest pass of each wave: the one the next wave waited for, so the
    // wires between them trace the run's critical path.
    let longest: Vec<usize> = columns
        .iter()
        .map(|wave| {
            *wave
                .iter()
                .max_by(|a, b| run.passes[**a].cpu_ms.total_cmp(&run.passes[**b].cpu_ms))
                .unwrap_or(&wave[0])
        })
        .collect();
    // Top to bottom: one row per wave, its passes side by side and centred,
    // so the frame reads downward in run order and a wide wave is plainly
    // work that ran at once. Laid out across, thirty-odd columns fitted to the
    // view at a size no label could be read at.
    let widest = columns.iter().map(Vec::len).max().unwrap_or(1) as f32;
    for (col, wave) in columns.iter().enumerate() {
        let left = 40.0 + (widest - wave.len() as f32) * COL_W * 0.5;
        let y = 30.0 + col as f32 * ROW_H;
        for (row, &i) in wave.iter().enumerate() {
            let pass = &run.passes[i];
            let ms = pass.gpu_ms.map_or(pass.cpu_ms, f64::from);
            let title = format!("{} · {}", i + 1, super::short(&pass.name));
            let node = graph_node_view(
                &mut commands,
                &fonts,
                handle.canvas,
                handle.viewport,
                i as u64,
                &title,
                heat(ms),
                if col == 0 { &[] } else { &input },
                if col + 1 == columns.len() { &[] } else { &output },
                left + row as f32 * COL_W,
                y,
                false,
                None,
                &[],
                None,
            );
            commands.entity(handle.canvas).add_child(node);
            if col > 0 {
                let from = longest[col - 1];
                let wire = graph_wire_view(&mut commands, handle.viewport, from as u64, "out", i as u64, "in");
                // On the viewport, not the canvas: a cable is drawn in the
                // viewport's own space from its ports' screen positions, and
                // under the canvas its pan and zoom were applied a second time,
                // piling every wire up to one side of the graph.
                commands.entity(handle.viewport).add_child(wire);
            }
        }
    }
}

/// Follow the graph's selection into the side panel.
fn follow_selection(mut views: Query<&mut NodeGraphView>, mut shown: ResMut<Shown>) {
    for mut view in &mut views {
        for edit in view.pending.drain(..) {
            if let GraphEdit::Select { id } = edit {
                shown.selected = id;
            }
        }
    }
}

fn details(shown: Res<Shown>, mut lines: Query<&mut Text, With<DetailText>>) {
    if !shown.is_changed() {
        return;
    }
    let run = shown.runs.get(shown.camera);
    let pass = shown.selected.and_then(|i| run.and_then(|r| r.passes.get(i as usize)).map(|p| (i as usize, p)));
    let line = match (run, pass) {
        (Some(run), Some((i, pass))) => format!(
            "{} of {}  ·  {}  ·  CPU {:.3} ms  ·  GPU {}  ·  {}",
            i + 1,
            run.passes.len(),
            super::short(&pass.name),
            pass.cpu_ms,
            pass.gpu_ms.map_or("not timed".into(), |g| format!("{g:.3} ms")),
            pass.name,
        ),
        _ => "Click a node to see its times".into(),
    };
    for mut t in &mut lines {
        if t.0 != line {
            t.0.clone_from(&line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_profiler::wire::GamePacket;
    use renzora_profiler::GpuPass;

    fn span(thread: u32, name: u32, start_ns: u64, end_ns: u64, depth: u16) -> Span {
        Span { thread, name, start_ns, end_ns, depth }
    }

    /// The schedule span is on the render thread; its systems run on pool
    /// threads, beside a main-world system that overlaps the window in one
    /// frame and runs outside it in the other.
    #[test]
    fn passes_are_found_on_any_thread_and_main_world_systems_are_not() {
        let mut capture = Capture::default();
        capture.receive(GamePacket::Names(vec![
            (0, "schedule: Core3d".into()),
            (1, "bevy_core_pipeline::core_3d::main_opaque_pass_3d".into()),
            (2, "bevy_core_pipeline::tonemapping::tonemapping".into()),
            (3, "apply".into()),
            (4, "main_opaque_pass_3d".into()),
            (5, "game::ai".into()),
            (6, "multithreaded executor".into()),
        ]));
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
        let names: Vec<&str> = runs[0].passes.iter().map(|p| super::super::short(&p.name)).collect();
        assert_eq!(names, vec!["main_opaque_pass_3d", "tonemapping"]);
        assert_eq!(runs[0].passes[0].gpu_ms, Some(6.9));
        assert_eq!(runs[0].passes[1].gpu_ms, None);
    }

    /// Two passes that start microseconds apart swap places from frame to
    /// frame. The graph built from them must not swap with them, or it
    /// rebuilds, and a rebuilt graph's cables flicker.
    #[test]
    fn near_simultaneous_passes_keep_one_order() {
        let mut capture = Capture::default();
        capture.receive(GamePacket::Names(vec![
            (0, "schedule: Core3d".into()),
            (1, "game::render::b_pass".into()),
            (2, "game::render::a_pass".into()),
        ]));
        let frame = |base: u64, a_first: bool| {
            let (a, b) = if a_first { (base + 10_000, base + 12_000) } else { (base + 12_000, base + 10_000) };
            ProfileFrame {
                spans: vec![
                    span(1, 0, base, base + 100_000, 2),
                    span(2, 2, a, a + 5_000, 0),
                    span(3, 1, b, b + 5_000, 0),
                ],
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
