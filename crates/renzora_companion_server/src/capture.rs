//! What the profiled game sent: names, threads, frames and probe reports.
//!
//! Both processes keep one. The companion's holds the whole recording (two
//! minutes) for saving and for the editor's questions; the window's holds the
//! last few seconds it draws from, fed by the companion as packets arrive (see
//! `crate::window`). They are the same type with a different [`Capture::limit`].
//!
//! A capture is the stream itself, so saving one is writing the packets back
//! out (`.rzprof`, the wire format end to end) and opening one is reading them
//! in. No second file format to keep in step with the first.
//!
//! Ported from the engine companion's `app::capture`.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use renzora_companion_proto::frame::ProfileFrame;
use renzora_companion_proto::probes::ProbeReport;
use renzora_companion_proto::wire::{self, GamePacket, ProbeInfo};

use crate::log;

/// Frames the companion keeps: two minutes at 60 fps. Older ones drop off the
/// front.
pub(crate) const MAX_FRAMES: usize = 7_200;

pub(crate) struct Capture {
    /// Who it came from, for titles and file names.
    pub source: String,
    /// The engine version the game was built from, from its hello.
    pub build: String,
    pub names: HashMap<u32, String>,
    pub threads: BTreeMap<u32, String>,
    pub frames: VecDeque<ProfileFrame>,
    /// Frames kept before the oldest drops off.
    pub limit: usize,
    /// Opened from a file rather than streaming.
    pub from_file: Option<PathBuf>,
    /// Frames recorded while the companion was running a job: their times
    /// include the competition for the machine, so views mark them.
    pub overlapped: HashSet<u64>,
    /// The probes the game offers (see the Debug screen).
    pub probes: Vec<ProbeInfo>,
    /// Each watched probe's latest report, with a hash of its text (so an
    /// unchanged report changes nothing) and the text itself (so it can be
    /// passed on to the window as it came).
    pub reports: HashMap<String, (ProbeReport, u64, String)>,
    /// Counts every change, so a view can tell whether there is anything new.
    pub revision: u64,
}

impl Default for Capture {
    fn default() -> Self {
        Self::with_limit(MAX_FRAMES)
    }
}

impl Capture {
    pub(crate) fn with_limit(limit: usize) -> Self {
        Self {
            source: String::new(),
            build: String::new(),
            names: HashMap::new(),
            threads: BTreeMap::new(),
            frames: VecDeque::new(),
            limit: limit.max(1),
            from_file: None,
            overlapped: HashSet::new(),
            probes: Vec::new(),
            reports: HashMap::new(),
            revision: 0,
        }
    }

    /// Start over, for a new game or file, keeping the limit and counting on
    /// from the old revision.
    pub(crate) fn reset(&mut self, source: &str, from_file: Option<PathBuf>) {
        let (limit, revision) = (self.limit, self.revision + 1);
        *self = Self::with_limit(limit);
        self.source = source.to_string();
        self.from_file = from_file;
        self.revision = revision;
    }

    /// Take in one packet. `overlapped` marks a frame recorded while a job
    /// ran.
    pub(crate) fn receive(&mut self, packet: GamePacket, overlapped: bool) {
        match packet {
            GamePacket::Hello { name, build, .. } => {
                self.source = name;
                self.build = build;
            }
            GamePacket::Names(names) => self.names.extend(names),
            GamePacket::Threads(threads) => self.threads.extend(threads),
            GamePacket::Frame(frame) => {
                while self.frames.len() >= self.limit {
                    match self.frames.pop_front() {
                        Some(old) => {
                            self.overlapped.remove(&old.index);
                        }
                        None => break,
                    }
                }
                if overlapped {
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
                if self.reports.get(&id).is_some_and(|(_, h, _)| *h == hash) {
                    return;
                }
                match serde_json::from_str::<ProbeReport>(&json) {
                    Ok(report) => {
                        self.reports.insert(id, (report, hash, json));
                    }
                    Err(e) => log::warn(format!("probe {id} sent a report that could not be read: {e}")),
                }
            }
        }
        self.revision += 1;
    }

    /// An interned name's text, or a placeholder for one not received.
    pub(crate) fn name(&self, id: u32) -> &str {
        self.names.get(&id).map(String::as_str).unwrap_or("?")
    }

    pub(crate) fn thread_name(&self, id: u32) -> &str {
        self.threads.get(&id).map(String::as_str).unwrap_or("thread")
    }

    pub(crate) fn frame(&self, index: u64) -> Option<&ProfileFrame> {
        // Frames arrive in order, so the position follows from the first index.
        let first = self.frames.front()?.index;
        self.frames.get(usize::try_from(index.checked_sub(first)?).ok()?).filter(|f| f.index == index)
    }

    /// The packets that rebuild this capture elsewhere, with the newest `last`
    /// frames: how the window is brought up to date when it opens, and what a
    /// saved file holds (`last` = everything). A frame's flag says whether it
    /// was recorded while a job ran.
    pub(crate) fn packets(&self, last: usize) -> Vec<(GamePacket, bool)> {
        let mut out = Vec::new();
        out.push((
            GamePacket::Hello {
                version: wire::VERSION,
                name: self.source.clone(),
                platform: String::new(),
                build: self.build.clone(),
            },
            false,
        ));
        let mut names: Vec<(u32, String)> = self.names.iter().map(|(k, v)| (*k, v.clone())).collect();
        names.sort_by_key(|(k, _)| *k);
        out.push((GamePacket::Names(names), false));
        out.push((GamePacket::Threads(self.threads.iter().map(|(k, v)| (*k, v.clone())).collect()), false));
        let skip = self.frames.len().saturating_sub(last);
        for frame in self.frames.iter().skip(skip) {
            out.push((GamePacket::Frame(frame.clone()), self.overlapped.contains(&frame.index)));
        }
        if !self.probes.is_empty() {
            out.push((GamePacket::Probes(self.probes.clone()), false));
        }
        for (id, (_, _, json)) in &self.reports {
            out.push((GamePacket::ProbeReport { id: id.clone(), json: json.clone() }, false));
        }
        out
    }

    /// Write the capture as a stream of packets.
    pub(crate) fn save(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
        for (packet, _) in self.packets(usize::MAX) {
            // Probe reports describe the moment, not the recording.
            if matches!(packet, GamePacket::Probes(_) | GamePacket::ProbeReport { .. }) {
                continue;
            }
            out.write_all(&packet.encode())?;
        }
        out.flush()
    }

    /// Read a capture written by [`Capture::save`] (or by the engine
    /// companion, which wrote the same packets).
    pub(crate) fn open(path: &Path) -> std::io::Result<Capture> {
        let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let mut capture = Capture { from_file: Some(path.to_path_buf()), ..Default::default() };
        while let Some(packet) = GamePacket::read(&mut reader)? {
            capture.receive(packet, false);
        }
        Ok(capture)
    }

    /// Where saved captures go: `~/.renzora/captures`.
    pub(crate) fn folder() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| PathBuf::from(home).join(".renzora").join("captures"))
    }

    /// Save into [`Capture::folder`] under the game's name and the time, and
    /// say what happened in words for the window.
    pub(crate) fn save_to_folder(&self) -> String {
        if self.frames.is_empty() {
            return "Nothing recorded yet to save".to_string();
        }
        let Some(folder) = Self::folder() else {
            return "No home folder to save captures in".to_string();
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let safe: String = self
            .source
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let path = folder.join(format!("{}-{stamp}.rzprof", if safe.is_empty() { "capture" } else { &safe }));
        match self.save(&path) {
            Ok(()) => format!("Saved {} frames to {}", self.frames.len(), path.display()),
            Err(e) => format!("Could not save the capture: {e}"),
        }
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
        // The system that runs a schedule the profiler wraps so it is timed
        // (`renzora_profiler::Timed`): the whole schedule, not work of its own.
        || name.contains("time_unspanned_schedules")
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
    use renzora_companion_proto::frame::Span;

    fn frame(index: u64, start: u64, spans: Vec<Span>) -> ProfileFrame {
        ProfileFrame { index, start_ns: start, end_ns: start + 16_000_000, spans, ..Default::default() }
    }

    #[test]
    fn a_saved_capture_opens_as_it_was() {
        let mut capture = Capture::default();
        capture.receive(GamePacket::Names(vec![(0, "physics::step".into())]), false);
        capture.receive(GamePacket::Threads(vec![(0, "main".into())]), false);
        capture.receive(
            GamePacket::Frame(frame(5, 0, vec![Span { thread: 0, name: 0, start_ns: 10, end_ns: 900, depth: 0 }])),
            false,
        );
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
        capture.receive(GamePacket::Names(vec![(0, "a".into()), (1, "b".into()), (2, "schedule: Update".into())]), false);
        capture.receive(
            GamePacket::Frame(frame(
                0,
                0,
                vec![
                    Span { thread: 0, name: 2, start_ns: 0, end_ns: 16_000_000, depth: 0 },
                    Span { thread: 0, name: 0, start_ns: 0, end_ns: 4_000_000, depth: 1 },
                    Span { thread: 0, name: 1, start_ns: 4_000_000, end_ns: 5_000_000, depth: 1 },
                ],
            )),
            false,
        );
        // Half of `a` is outside the range.
        let top = slowest_in(&capture, 2_000_000, 16_000_000, 10);
        let names: Vec<&str> = top.iter().map(|(n, _, _)| capture.name(*n)).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert!((top[0].1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn history_is_bounded_and_overlap_marks_go_with_their_frames() {
        let mut capture = Capture::with_limit(10);
        for i in 0..13u64 {
            capture.receive(GamePacket::Frame(frame(i, i * 16_000_000, Vec::new())), i == 1 || i == 12);
        }
        assert_eq!(capture.frames.len(), 10);
        assert_eq!(capture.frames.front().map(|f| f.index), Some(3));
        assert_eq!(capture.overlapped, HashSet::from([12]));
    }

    /// What the window is sent rebuilds the newest part of the capture,
    /// marks and all.
    #[test]
    fn packets_rebuild_the_newest_frames() {
        let mut capture = Capture::default();
        capture.receive(GamePacket::Names(vec![(0, "a".into())]), false);
        for i in 0..5u64 {
            capture.receive(GamePacket::Frame(frame(i, i * 16_000_000, Vec::new())), i == 4);
        }
        let mut copy = Capture::with_limit(3);
        for (packet, overlapped) in capture.packets(3) {
            copy.receive(packet, overlapped);
        }
        assert_eq!(copy.frames.iter().map(|f| f.index).collect::<Vec<_>>(), [2, 3, 4]);
        assert_eq!(copy.name(0), "a");
        assert!(copy.overlapped.contains(&4));
    }
}
