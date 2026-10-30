//! Questions about the capture, answered for the editor (and through it, an
//! MCP client): what is being profiled, where the time goes, and what made
//! the slow frames slow.
//!
//! These read the live [`Capture`] the window draws from, so they run here on
//! the main thread rather than on the job workers, and they answer at once.
//! A request arrives as an ordinary job whose kind starts with `profiler.`;
//! [`link`](super::link) sends those here instead of to the queue.
//!
//! Answers are JSON, with times in milliseconds rounded to a microsecond:
//! meant to be read by a person or a model, not fed back into a tool.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora::job_args::{ProfilerQuery, ProfilerRecord};
use renzora_profiler::ProfileFrame;
use serde::Serialize;
use serde_json::{json, Value};

use super::capture::{frame_ms, is_wrapper, Capture};
use super::games::{ConnRequest, ConnState, Connection, Games};
use super::jobs::JobQueue;
use crate::protocol::JobId;

/// The window a question covers when it does not say.
const DEFAULT_SECONDS: f32 = 5.0;
const DEFAULT_LIMIT: usize = 15;
/// What a 60 Hz frame may take.
const BUDGET_MS: f64 = 1000.0 / 60.0;

/// Questions waiting for this frame's answer.
#[derive(Resource, Default)]
pub(crate) struct Questions(pub Vec<(JobId, String, String)>);

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Questions>().add_systems(Update, answer);
}

/// Whether a job kind is a question for this module.
pub(crate) fn is_question(kind: &str) -> bool {
    kind.starts_with("profiler.")
}

fn answer(
    mut questions: ResMut<Questions>,
    mut queue: ResMut<JobQueue>,
    capture: Res<Capture>,
    conn: Res<Connection>,
    games: Res<Games>,
    mut requests: MessageWriter<ConnRequest>,
) {
    for (id, kind, payload) in questions.0.drain(..) {
        let result = match kind.as_str() {
            "profiler.record" => serde_json::from_str::<ProfilerRecord>(&payload)
                .map_err(|e| e.to_string())
                .and_then(|args| record(&args, &conn, &games, &mut requests)),
            "profiler.status" => Ok(status(&capture, &conn, &games).to_string()),
            "profiler.summary" | "profiler.spikes" => serde_json::from_str::<ProfilerQuery>(&payload)
                .map_err(|e| e.to_string())
                .and_then(|q| {
                    let window = window(&capture, q.seconds.unwrap_or(DEFAULT_SECONDS));
                    if window.is_empty() {
                        return Err(no_frames(&conn));
                    }
                    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, 100);
                    Ok(if kind == "profiler.summary" {
                        summary(&capture, &window, limit)
                    } else {
                        spikes(&capture, &window, limit.min(10), q.over_ms)
                    }
                    .to_string())
                }),
            other => Err(format!("the companion has no question called {other}")),
        };
        queue.answer(id, result);
    }
}

fn no_frames(conn: &Connection) -> String {
    match (&conn.state, conn.recording) {
        (ConnState::Connected, true) => "recording, but no frames have arrived yet: ask again in a second".into(),
        (ConnState::Connected, false) => "connected but not recording: start with profiler_record".into(),
        _ => "nothing is being profiled: start with profiler_record (the editor itself is the game \"Editor\")".into(),
    }
}

/// Start or stop recording, connecting first when asked to or when nothing is
/// connected. Connecting starts recording by itself once the game answers.
fn record(
    args: &ProfilerRecord,
    conn: &Connection,
    games: &Games,
    requests: &mut MessageWriter<ConnRequest>,
) -> Result<String, String> {
    if !args.on {
        requests.write(ConnRequest::Record(false));
        return Ok("recording stopped; the capture is kept for questions".into());
    }
    let connected = conn.state == ConnState::Connected;
    let wanted = args.game.as_deref().map(str::to_lowercase);
    let current = conn.game.as_ref().map(|g| g.name.to_lowercase());
    let switch = match (&wanted, &current) {
        (Some(want), Some(cur)) => !cur.contains(want.as_str()),
        (Some(_), None) => true,
        (None, _) => !connected,
    };
    if !switch {
        requests.write(ConnRequest::Record(true));
        return Ok(format!(
            "recording {}; frames arrive continuously, ask profiler_summary after a few seconds",
            conn.game.as_ref().map_or("", |g| g.name.as_str())
        ));
    }
    let found: Vec<_> = games.found.values().map(|(g, _)| g).collect();
    let pick = match &wanted {
        Some(want) => found.iter().find(|g| g.name.to_lowercase().contains(want.as_str())),
        // Nothing named: the editor, the one game always there.
        None => found
            .iter()
            .find(|g| g.name == "Editor")
            .or_else(|| found.first()),
    };
    let Some(game) = pick else {
        let names: Vec<&str> = found.iter().map(|g| g.name.as_str()).collect();
        return Err(format!("no game matches; these can be profiled: {}", names.join(", ")));
    };
    requests.write(ConnRequest::Connect((*game).clone()));
    Ok(format!(
        "connecting to {} ({}); recording starts when it answers, usually within a second",
        game.name,
        game.place()
    ))
}

fn status(capture: &Capture, conn: &Connection, games: &Games) -> Value {
    let seconds = match (capture.frames.front(), capture.frames.back()) {
        (Some(a), Some(b)) => b.end_ns.saturating_sub(a.start_ns) as f64 / 1e9,
        _ => 0.0,
    };
    json!({
        "profiling": conn.game.as_ref().map(|g| g.name.clone()),
        "connection": match &conn.state {
            ConnState::Idle => "idle".to_string(),
            ConnState::Connecting => "connecting".to_string(),
            ConnState::Connected => "connected".to_string(),
            ConnState::Failed(why) => format!("failed: {why}"),
        },
        "recording": conn.recording,
        "frames_captured": capture.frames.len(),
        "seconds_captured": round(seconds),
        "games_available": games.found.values().map(|(g, _)| json!({"name": g.name, "where": g.place()})).collect::<Vec<_>>(),
    })
}

/// The frames that ended within the last `seconds` of the capture.
fn window(capture: &Capture, seconds: f32) -> Vec<&ProfileFrame> {
    let Some(newest) = capture.frames.back() else { return Vec::new() };
    let from = newest.end_ns.saturating_sub((seconds.max(0.0) as f64 * 1e9) as u64);
    capture.frames.iter().filter(|f| f.end_ns >= from).collect()
}

fn round(ms: f64) -> f64 {
    (ms * 1000.0).round() / 1000.0
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let at = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[at.min(sorted.len() - 1)]
}

/// Per span name across `frames`: total ms, runs, and the longest single run.
#[derive(Default, Clone, Copy)]
struct Tally {
    total_ms: f64,
    runs: u32,
    max_ms: f64,
}

fn tally<'a>(frames: impl IntoIterator<Item = &'a ProfileFrame>) -> HashMap<u32, Tally> {
    let mut out: HashMap<u32, Tally> = HashMap::new();
    for frame in frames {
        for span in &frame.spans {
            let ms = span.end_ns.saturating_sub(span.start_ns) as f64 / 1e6;
            let t = out.entry(span.name).or_default();
            t.total_ms += ms;
            t.runs += 1;
            t.max_ms = t.max_ms.max(ms);
        }
    }
    out
}

/// What the frames cost and where it went.
fn summary(capture: &Capture, frames: &[&ProfileFrame], limit: usize) -> Value {
    let n = frames.len() as f64;
    let mut times: Vec<f64> = frames.iter().map(|f| frame_ms(f) as f64).collect();
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / n;

    let mut systems: Vec<(u32, Tally)> = tally(frames.iter().copied())
        .into_iter()
        .filter(|(name, _)| !is_wrapper(capture.name(*name)))
        .collect();
    systems.sort_by(|a, b| b.1.total_ms.total_cmp(&a.1.total_ms));
    let systems: Vec<Value> = systems
        .iter()
        .take(limit)
        .map(|(name, t)| {
            json!({
                "name": capture.name(*name),
                "ms_per_frame": round(t.total_ms / n),
                "share_of_frame": format!("{:.1}%", t.total_ms / n / mean * 100.0),
                "longest_run_ms": round(t.max_ms),
                "runs_per_frame": round(t.runs as f64 / n),
            })
        })
        .collect();

    let mut gpu: HashMap<u32, (f64, u32, u8)> = HashMap::new();
    for frame in frames {
        for pass in &frame.gpu {
            let e = gpu.entry(pass.name).or_insert((0.0, 0, pass.depth));
            e.0 += pass.ms as f64;
            e.1 += 1;
        }
    }
    let gpu_total: f64 = gpu.values().filter(|(_, _, d)| *d == 0).map(|(ms, runs, _)| ms / *runs as f64).sum();
    let mut gpu: Vec<(u32, f64)> = gpu.into_iter().map(|(name, (ms, runs, _))| (name, ms / runs as f64)).collect();
    gpu.sort_by(|a, b| b.1.total_cmp(&a.1));

    let counters: Vec<Value> = frames
        .last()
        .map(|f| f.counters.iter().map(|c| json!({"name": capture.name(c.name), "value": c.value})).collect())
        .unwrap_or_default();

    let overlapped = frames.iter().filter(|f| capture.overlapped.contains(&f.index)).count();
    json!({
        "game": capture.source,
        "frames": frames.len(),
        "frames_overlapping_companion_work": overlapped,
        "overlap_note": if overlapped > 0 {
            "Some frames were recorded while the companion ran a job someone started anyway; their times include that competition for the machine."
        } else {
            "No companion work ran during these frames."
        },
        "seconds": round(frames.last().map_or(0, |l| l.end_ns.saturating_sub(frames[0].start_ns)) as f64 / 1e9),
        "fps": round(1000.0 / mean),
        "frame_ms": {
            "mean": round(mean),
            "p50": round(percentile(&times, 0.5)),
            "p95": round(percentile(&times, 0.95)),
            "p99": round(percentile(&times, 0.99)),
            "max": round(times.last().copied().unwrap_or(0.0)),
            "over_16_6_ms": times.iter().filter(|t| **t > BUDGET_MS).count(),
            "over_33_3_ms": times.iter().filter(|t| **t > BUDGET_MS * 2.0).count(),
        },
        "note": "System times are inclusive (a span's time includes spans inside it) and summed over every thread, so they can add up to more than a frame.",
        "slowest_systems": systems,
        "gpu_ms_per_frame": round(gpu_total),
        "gpu_passes": gpu.iter().take(limit).map(|(name, ms)| json!({"pass": capture.name(*name), "ms": round(*ms)})).collect::<Vec<_>>(),
        "counters": counters,
    })
}

/// The slow frames, and for each, what took longer than it usually does.
///
/// "Usually" is the mean over the whole window, so a system that is always
/// slow does not crowd out the one that spiked: the list is ordered by how
/// much MORE each name took in this frame than in an average one.
fn spikes(capture: &Capture, frames: &[&ProfileFrame], limit: usize, over_ms: Option<f32>) -> Value {
    let n = frames.len() as f64;
    let usual: HashMap<u32, f64> = tally(frames.iter().copied())
        .into_iter()
        .map(|(name, t)| (name, t.total_ms / n))
        .collect();
    let mut times: Vec<f64> = frames.iter().map(|f| frame_ms(f) as f64).collect();
    times.sort_by(f64::total_cmp);
    let median = percentile(&times, 0.5);

    let mut slow: Vec<&&ProfileFrame> = match over_ms {
        Some(over) => frames.iter().filter(|f| frame_ms(f) > over).collect(),
        None => frames.iter().collect(),
    };
    let matching = slow.len();
    slow.sort_by(|a, b| frame_ms(b).total_cmp(&frame_ms(a)));
    slow.truncate(limit);

    let detail: Vec<Value> = slow
        .iter()
        .map(|frame| {
            let mut extra: Vec<(u32, f64, f64)> = tally([**frame])
                .into_iter()
                .filter(|(name, _)| !is_wrapper(capture.name(*name)))
                .map(|(name, t)| (name, t.total_ms, t.total_ms - usual.get(&name).copied().unwrap_or(0.0)))
                .filter(|(_, _, extra)| *extra > 0.05)
                .collect();
            extra.sort_by(|a, b| b.2.total_cmp(&a.2));
            json!({
                "frame": frame.index,
                "overlapped_companion_work": capture.overlapped.contains(&frame.index),
                "ms": round(frame_ms(frame) as f64),
                "ms_over_median": round(frame_ms(frame) as f64 - median),
                "took_longer_than_usual": extra.iter().take(8).map(|(name, ms, more)| json!({
                    "name": capture.name(*name),
                    "ms": round(*ms),
                    "usual_ms": round(ms - more),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    #[derive(Serialize)]
    struct Spikes {
        median_frame_ms: f64,
        frames_in_window: usize,
        frames_matching: usize,
        slowest: Vec<Value>,
    }
    serde_json::to_value(Spikes {
        median_frame_ms: round(median),
        frames_in_window: frames.len(),
        frames_matching: matching,
        slowest: detail,
    })
    .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_profiler::wire::GamePacket;
    use renzora_profiler::Span;

    fn capture() -> Capture {
        let mut capture = Capture::default();
        capture.receive(GamePacket::Names(vec![
            (1, "game::physics".into()),
            (2, "game::ai".into()),
            (3, "schedule: Update".into()),
        ]));
        // Ten frames of 10 ms: physics 4 ms, ai 1 ms; frame 7 has ai at 9 ms.
        for i in 0..10u64 {
            let start = i * 10_000_000;
            let ai = if i == 7 { 9_000_000 } else { 1_000_000 };
            capture.receive(GamePacket::Frame(ProfileFrame {
                index: i,
                start_ns: start,
                end_ns: start + if i == 7 { 18_000_000 } else { 10_000_000 },
                spans: vec![
                    Span { thread: 0, name: 3, start_ns: start, end_ns: start + 9_000_000, depth: 0 },
                    Span { thread: 0, name: 1, start_ns: start, end_ns: start + 4_000_000, depth: 1 },
                    Span { thread: 0, name: 2, start_ns: start + 4_000_000, end_ns: start + 4_000_000 + ai, depth: 1 },
                ],
                ..Default::default()
            }));
        }
        capture
    }

    #[test]
    fn the_summary_ranks_systems_and_leaves_out_wrappers() {
        let capture = capture();
        let frames: Vec<&ProfileFrame> = capture.frames.iter().collect();
        let s = summary(&capture, &frames, 10);
        let names: Vec<&str> = s["slowest_systems"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["game::physics", "game::ai"]);
        assert_eq!(s["slowest_systems"][0]["ms_per_frame"], 4.0);
        assert_eq!(s["frame_ms"]["max"], 18.0);
        assert_eq!(s["frame_ms"]["over_16_6_ms"], 1);
    }

    #[test]
    fn a_spike_names_what_grew_not_what_is_always_slow() {
        let capture = capture();
        let frames: Vec<&ProfileFrame> = capture.frames.iter().collect();
        let s = spikes(&capture, &frames, 1, Some(15.0));
        assert_eq!(s["frames_matching"], 1);
        let slow = &s["slowest"][0];
        assert_eq!(slow["frame"], 7);
        // Physics costs more every frame, but it is ai that changed.
        assert_eq!(slow["took_longer_than_usual"][0]["name"], "game::ai");
        assert_eq!(slow["took_longer_than_usual"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn the_window_keeps_only_recent_frames() {
        let capture = capture();
        // The newest frame ends at 100 ms; 11 ms back reaches frames 8 and 9
        // (frame 7 is the long one, ending at 88 ms).
        assert_eq!(window(&capture, 0.011).iter().map(|f| f.index).collect::<Vec<_>>(), [8, 9]);
    }
}
