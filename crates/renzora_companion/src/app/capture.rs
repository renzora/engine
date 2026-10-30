//! What the profiled game sent: names, threads and frames, kept for viewing.
//!
//! A capture is the stream itself, so saving one is writing the packets back
//! out (`.rzprof`, the wire format end to end) and opening one is reading them
//! in. No second file format to keep in step with the first.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use renzora_profiler::wire::GamePacket;
use renzora_profiler::ProfileFrame;

use super::games::GameInfo;

/// Frames kept: two minutes at 60 fps. Older ones drop off the front.
pub(crate) const MAX_FRAMES: usize = 7_200;

#[derive(Resource, Default)]
pub(crate) struct Capture {
    /// Who it came from, for titles and file names.
    pub source: String,
    pub names: HashMap<u32, String>,
    pub threads: BTreeMap<u32, String>,
    pub frames: VecDeque<ProfileFrame>,
    /// Bumped by every change, so views redraw only when there is something new.
    pub revision: u64,
    /// Opened from a file rather than streaming.
    pub from_file: Option<PathBuf>,
    /// Frames recorded while the companion was running a job: their times
    /// include the competition for the machine, so views mark them.
    pub overlapped: std::collections::HashSet<u64>,
    /// Whether a job is running as packets arrive; set by the connection.
    pub companion_busy: bool,
    /// The probes the game offers (see `ui::debug_page`).
    pub probes: Vec<renzora_profiler::wire::ProbeInfo>,
    /// Each watched probe's latest report, with a hash of its text so the
    /// Debug screen redraws a card only when it changed.
    pub reports: HashMap<String, (renzora::probes::ProbeReport, u64)>,
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Capture>();
}

impl Capture {
    /// Start over for a new game.
    pub fn start(&mut self, game: &GameInfo) {
        *self = Capture {
            source: game.name.clone(),
            revision: self.revision + 1,
            ..Default::default()
        };
    }

    pub fn receive(&mut self, packet: GamePacket) {
        match packet {
            GamePacket::Hello { name, .. } => self.source = name,
            GamePacket::Names(names) => self.names.extend(names),
            GamePacket::Threads(threads) => self.threads.extend(threads),
            GamePacket::Frame(frame) => {
                if self.frames.len() == MAX_FRAMES {
                    if let Some(old) = self.frames.pop_front() {
                        self.overlapped.remove(&old.index);
                    }
                }
                if self.companion_busy {
                    self.overlapped.insert(frame.index);
                }
                self.frames.push_back(frame);
            }
            GamePacket::Probes(probes) => self.probes = probes,
            GamePacket::ProbeReport { id, json } => {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                json.hash(&mut hasher);
                let hash = hasher.finish();
                // An unchanged report is not a change: nothing redraws.
                if self.reports.get(&id).is_some_and(|(_, h)| *h == hash) {
                    return;
                }
                match serde_json::from_str::<renzora::probes::ProbeReport>(&json) {
                    Ok(report) => {
                        self.reports.insert(id, (report, hash));
                    }
                    Err(e) => warn!("[companion] probe {id} sent a report that could not be read: {e}"),
                }
            }
        }
        self.revision += 1;
    }

    /// An interned name's text, or a placeholder for one not received.
    pub fn name(&self, id: u32) -> &str {
        self.names.get(&id).map(String::as_str).unwrap_or("?")
    }

    pub fn thread_name(&self, id: u32) -> &str {
        self.threads.get(&id).map(String::as_str).unwrap_or("thread")
    }

    pub fn frame(&self, index: u64) -> Option<&ProfileFrame> {
        // Frames arrive in order, so the position follows from the first index.
        let first = self.frames.front()?.index;
        self.frames.get(index.checked_sub(first)? as usize).filter(|f| f.index == index)
    }

    /// Write the capture as a stream of packets.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
        out.write_all(
            &GamePacket::Hello {
                version: renzora_profiler::wire::VERSION,
                name: self.source.clone(),
                platform: String::new(),
                build: renzora::version::ENGINE_VERSION.to_string(),
            }
            .encode(),
        )?;
        let mut names: Vec<(u32, String)> = self.names.iter().map(|(k, v)| (*k, v.clone())).collect();
        names.sort_by_key(|(k, _)| *k);
        out.write_all(&GamePacket::Names(names).encode())?;
        let threads = self.threads.iter().map(|(k, v)| (*k, v.clone())).collect();
        out.write_all(&GamePacket::Threads(threads).encode())?;
        for frame in &self.frames {
            out.write_all(&GamePacket::Frame(frame.clone()).encode())?;
        }
        out.flush()
    }

    /// Read a capture written by [`Capture::save`].
    pub fn open(path: &Path) -> std::io::Result<Capture> {
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut capture = Capture {
            from_file: Some(path.to_path_buf()),
            ..Default::default()
        };
        while let Some(packet) = GamePacket::read(&mut reader)? {
            capture.receive(packet);
        }
        Ok(capture)
    }

    /// Where saved captures go: `~/.renzora/captures`.
    pub fn folder() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| PathBuf::from(home).join(".renzora").join("captures"))
    }
}

/// Milliseconds a frame took, mark to mark.
pub(crate) fn frame_ms(frame: &ProfileFrame) -> f32 {
    frame.end_ns.saturating_sub(frame.start_ns) as f32 / 1e6
}

/// Total time per span name within `[start_ns, end_ns)`, slowest first: each
/// span counts only the part of it inside the range.
pub(crate) fn slowest_in(capture: &Capture, start_ns: u64, end_ns: u64, limit: usize) -> Vec<(u32, f64, u32)> {
    // A span is stored in the frame it FINISHED in, which can be after the
    // range for one that started inside it; a second of slack covers that.
    const SLACK_NS: u64 = 1_000_000_000;
    let mut totals: HashMap<u32, (f64, u32)> = HashMap::new();
    for frame in capture
        .frames
        .iter()
        .filter(|f| f.end_ns > start_ns && f.start_ns < end_ns.saturating_add(SLACK_NS))
    {
        for span in &frame.spans {
            let from = span.start_ns.max(start_ns);
            let to = span.end_ns.min(end_ns);
            if to <= from {
                continue;
            }
            let entry = totals.entry(span.name).or_default();
            entry.0 += (to - from) as f64 / 1e6;
            entry.1 += 1;
        }
    }
    let mut out: Vec<(u32, f64, u32)> = totals.into_iter().map(|(n, (ms, runs))| (n, ms, runs)).collect();
    // Outer spans (the whole schedule, the app update) would always win and say
    // nothing, so names that wrap the frame are left out.
    out.retain(|(n, _, _)| !is_wrapper(capture.name(*n)));
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.truncate(limit);
    out
}

/// Spans that contain the whole frame rather than doing work of their own.
pub(crate) fn is_wrapper(name: &str) -> bool {
    name == "main app"
        || name == "update"
        || name.starts_with("schedule:")
        // "sub app: RenderApp", "sub app: RenderExtractApp": the whole render
        // world's frame, which outranked every real system in a live editor.
        || name.starts_with("sub app:")
        || name.contains("executor")
        || name == "main_render_schedule"
        || name == "bevy_render::run_render_schedule"
        // Bevy 0.20 runs each camera's render graph as a schedule from here.
        || name == "bevy_core_pipeline::schedule::camera_driver"
        || name == "camera_schedule"
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_profiler::Span;

    fn frame(index: u64, start: u64, spans: Vec<Span>) -> ProfileFrame {
        ProfileFrame { index, start_ns: start, end_ns: start + 16_000_000, spans, ..Default::default() }
    }

    #[test]
    fn a_saved_capture_opens_as_it_was() {
        let mut capture = Capture::default();
        capture.receive(GamePacket::Names(vec![(0, "physics::step".into())]));
        capture.receive(GamePacket::Threads(vec![(0, "main".into())]));
        capture.receive(GamePacket::Frame(frame(5, 0, vec![Span { thread: 0, name: 0, start_ns: 10, end_ns: 900, depth: 0 }])));
        let path = std::env::temp_dir().join(format!("rz-capture-test-{}.rzprof", std::process::id()));
        capture.save(&path).unwrap();
        let opened = Capture::open(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(opened.name(0), "physics::step");
        assert_eq!(opened.thread_name(0), "main");
        assert_eq!(opened.frames, capture.frames);
        assert_eq!(opened.frame(5).map(|f| f.index), Some(5));
        assert!(opened.frame(6).is_none());
    }

    #[test]
    fn slowest_counts_only_the_part_in_range_and_skips_wrappers() {
        let mut capture = Capture::default();
        capture.receive(GamePacket::Names(vec![(0, "a".into()), (1, "b".into()), (2, "schedule: Update".into())]));
        capture.receive(GamePacket::Frame(frame(
            0,
            0,
            vec![
                Span { thread: 0, name: 2, start_ns: 0, end_ns: 16_000_000, depth: 0 },
                Span { thread: 0, name: 0, start_ns: 0, end_ns: 4_000_000, depth: 1 },
                Span { thread: 0, name: 1, start_ns: 4_000_000, end_ns: 5_000_000, depth: 1 },
            ],
        )));
        // Half of `a` is outside the range.
        let top = slowest_in(&capture, 2_000_000, 16_000_000, 10);
        let names: Vec<&str> = top.iter().map(|(n, _, _)| capture.name(*n)).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert!((top[0].1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn history_is_bounded() {
        let mut capture = Capture::default();
        for i in 0..(MAX_FRAMES as u64 + 3) {
            capture.receive(GamePacket::Frame(frame(i, i * 16_000_000, Vec::new())));
        }
        assert_eq!(capture.frames.len(), MAX_FRAMES);
        assert_eq!(capture.frames.front().map(|f| f.index), Some(3));
    }
}
