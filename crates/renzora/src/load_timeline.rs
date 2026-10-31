//! A timestamped record of the last project load.
//!
//! Loading a project is several pieces of work that finish at different times
//! and live in different crates: the scene file, the models it names, the
//! viewport cameras, the textures, virtual geometry, the environment lighting.
//! From the outside it is one black screen that fills in bit by bit, and "it
//! got slower" could be any of them. This is the one place they all write to,
//! so a load can be read back as a list of stages with times.
//!
//! It lives in the contract crate because nearly every writer is a different
//! crate, and because the MCP plugin reads it: a native plugin can link this
//! crate and nothing else of the engine's.
//!
//! A stage is recorded once, the first time it is reached, and each one is
//! also a log line, so the timeline is in the terminal of every launch without
//! anything asking for it.

use std::time::{Duration, Instant};

use bevy::prelude::*;

/// A frame this long during a load is counted as a stall: the main thread was
/// blocked by something, so nothing on screen moved for that long.
pub const STALL: Duration = Duration::from_millis(250);

/// One stage of a load: what was reached, when, and a few words about it.
#[derive(Clone, Debug)]
pub struct LoadMark {
    pub stage: String,
    pub at: Duration,
    pub detail: String,
    /// The editor's memory when the stage was reached, where the platform says.
    pub memory: Option<ProcessMemory>,
}

/// How much memory the editor process holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessMemory {
    /// In physical memory now: what Task Manager shows.
    pub resident: u64,
    /// Committed to the process, resident or not: what it has asked for.
    pub committed: u64,
    /// What the GPU allocator held at the same moment, when a probe is set
    /// (see [`LoadTimeline::set_gpu_probe`]).
    pub gpu: Option<GpuMemory>,
}

/// What the GPU allocator holds: the bytes handed out to buffers and textures,
/// and the memory blocks reserved behind them.
///
/// Recorded beside the process figures at every stage because a stage that
/// commits gigabytes may be doing it on the GPU's behalf (render targets,
/// staging, the blocks the allocator grows in) or on its own heap, and the
/// process figures alone cannot tell the two apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuMemory {
    pub allocated: u64,
    pub reserved: u64,
}

/// Reads the GPU allocator's totals. Set by the crate that owns the render
/// device, since this crate cannot reach it before the renderer exists.
pub type GpuMemoryProbe = Box<dyn Fn() -> Option<GpuMemory> + Send + Sync>;

/// The editor's memory right now. Recorded at every stage, so a load says which
/// stage allocated what, not only how long each took: one that ends holding
/// many gigabytes more than its files can explain points at the stage that did.
#[cfg(target_os = "windows")]
pub fn process_memory() -> Option<ProcessMemory> {
    /// `PROCESS_MEMORY_COUNTERS_EX`.
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_non_paged_pool: usize,
        quota_non_paged_pool: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        // The same signature `core::shell` declares it with: two differing
        // declarations of one symbol is a lint, and a real hazard.
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut core::ffi::c_void,
            counters: *mut Counters,
            cb: u32,
        ) -> i32;
    }
    let mut counters = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        ..Default::default()
    };
    // SAFETY: the pseudo-handle from GetCurrentProcess needs no closing, and
    // `counters` is a correctly laid out PROCESS_MEMORY_COUNTERS_EX whose `cb`
    // says so, alive for the whole call.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    (ok != 0).then_some(ProcessMemory {
        resident: counters.working_set as u64,
        committed: counters.private_usage as u64,
        gpu: None,
    })
}

#[cfg(target_os = "linux")]
pub fn process_memory() -> Option<ProcessMemory> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kilobytes = |key: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|rest| rest.split_whitespace().next()?.parse::<u64>().ok())
            .map(|kb| kb * 1024)
    };
    Some(ProcessMemory {
        resident: kilobytes("VmRSS:")?,
        committed: kilobytes("VmSize:")?,
        gpu: None,
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn process_memory() -> Option<ProcessMemory> {
    None
}

/// One kind of asset and the memory it holds.
#[derive(Clone, Debug, Default)]
pub struct AssetMemoryRow {
    /// What it is, e.g. `"meshes (CPU copies)"`.
    pub kind: &'static str,
    pub count: usize,
    pub bytes: u64,
    /// Whether these bytes are in this process's own memory. GPU memory is
    /// reported, estimated, beside it but kept out of the total, because the
    /// process figures do not include it.
    pub in_process: bool,
    pub note: String,
}

/// The memory each kind of asset holds, by kind.
///
/// The process figures say how much the editor holds, not what in. This is
/// the other half: each crate that owns a kind of asset reports what its own
/// assets hold, and what is left over between their total and the process's
/// committed memory is everything that is not an asset (the GPU driver's
/// copies and staging, the render world's buffers, compiled pipelines, the
/// allocator's reserve). A load that ends gigabytes heavier than its files
/// says, with this, whether the weight is in copies of the assets or outside
/// them.
#[derive(Resource, Default)]
pub struct AssetMemory {
    rows: Vec<AssetMemoryRow>,
}

impl AssetMemory {
    /// Replace the row for `row.kind`.
    pub fn set(&mut self, row: AssetMemoryRow) {
        match self.rows.iter_mut().find(|r| r.kind == row.kind) {
            Some(existing) => *existing = row,
            None => self.rows.push(row),
        }
    }

    pub fn rows(&self) -> &[AssetMemoryRow] {
        &self.rows
    }

    /// The rows, largest first, against the process's committed memory.
    pub fn report(&self) -> String {
        let mut rows: Vec<&AssetMemoryRow> = self.rows.iter().collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.bytes));
        let line = |r: &AssetMemoryRow| {
            let note = if r.note.is_empty() { String::new() } else { format!("  ({})", r.note) };
            format!("  {:<36} {:>7}  {:>10}{note}\n", r.kind, r.count, megabytes(r.bytes))
        };

        let mut out = String::from("Memory held by assets:\n");
        let in_process: u64 = rows.iter().filter(|r| r.in_process).map(|r| r.bytes).sum();
        for r in rows.iter().filter(|r| r.in_process) {
            out.push_str(&line(r));
        }
        out.push_str(&format!("  {:<36} {:>7}  {:>10}\n", "total in the assets", "", megabytes(in_process)));
        if let Some(process) = process_memory() {
            out.push_str(&format!(
                "  the editor holds {} committed ({} resident), so {} is outside the assets: \
                 GPU driver copies and staging, render-world buffers, compiled pipelines, \
                 the allocator's reserve, and everything else\n",
                gigabytes(process.committed),
                gigabytes(process.resident),
                gigabytes(process.committed.saturating_sub(in_process)),
            ));
        }
        let gpu: Vec<&&AssetMemoryRow> = rows.iter().filter(|r| !r.in_process).collect();
        if !gpu.is_empty() {
            out.push_str("On the GPU, estimated (not in the figures above):\n");
            for r in gpu {
                out.push_str(&line(r));
            }
        }
        out.trim_end().to_string()
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// The current or most recent project load. See the module docs.
#[derive(Resource, Default)]
pub struct LoadTimeline {
    project: String,
    started: Option<Instant>,
    marks: Vec<LoadMark>,
    finished: Option<Duration>,
    /// What was still outstanding when the load gave up, if it did.
    gave_up_waiting_on: Option<String>,
    longest_frame: Duration,
    stalls: u32,
    stalled_for: Duration,
    memory_at_start: Option<ProcessMemory>,
    memory_at_end: Option<ProcessMemory>,
    /// Appended to the report: the asset memory breakdown, when the finisher
    /// has one.
    appendix: String,
    /// Bumped by every [`begin`](Self::begin), so a reader can tell a new load
    /// from the one it last saw without comparing times.
    generation: u32,
    /// Kept across loads: it is set once, when the renderer exists.
    gpu_probe: Option<GpuMemoryProbe>,
}

impl LoadTimeline {
    /// Start recording a load of `project`, discarding the previous one.
    pub fn begin(&mut self, project: impl Into<String>) {
        let generation = self.generation.wrapping_add(1);
        *self = Self {
            project: project.into(),
            started: Some(Instant::now()),
            generation,
            gpu_probe: self.gpu_probe.take(),
            ..default()
        };
        self.memory_at_start = self.memory();
        info!("[loading] {}: load started{}", self.project, memory_text(self.memory_at_start, None));
    }

    /// Have every stage also record the GPU allocator's totals.
    pub fn set_gpu_probe(&mut self, probe: GpuMemoryProbe) {
        self.gpu_probe = Some(probe);
    }

    pub fn has_gpu_probe(&self) -> bool {
        self.gpu_probe.is_some()
    }

    /// The process figures, with the GPU allocator's beside them when a probe
    /// is set.
    fn memory(&self) -> Option<ProcessMemory> {
        let mut memory = process_memory()?;
        memory.gpu = self.gpu_probe.as_ref().and_then(|probe| probe());
        Some(memory)
    }

    /// Record that `stage` was reached, the first time only. Ignored when no
    /// load is running, so a writer never has to check first.
    pub fn mark(&mut self, stage: &str, detail: impl Into<String>) {
        let Some(at) = self.running_for() else { return };
        if self.has(stage) {
            return;
        }
        let detail = detail.into();
        let memory = self.memory();
        let before = self.marks.last().and_then(|m| m.memory).or(self.memory_at_start);
        info!(
            "[loading] {:>7.0} ms  {stage}{}{}",
            at.as_secs_f64() * 1000.0,
            bracket(&detail),
            memory_text(memory, before)
        );
        self.marks.push(LoadMark { stage: stage.to_string(), at, detail, memory });
    }

    /// Account for one frame of the load, so a blocked main thread shows up
    /// even when every stage eventually arrives.
    pub fn note_frame(&mut self, frame: Duration) {
        if !self.is_running() {
            return;
        }
        self.longest_frame = self.longest_frame.max(frame);
        if frame >= STALL {
            self.stalls += 1;
            self.stalled_for += frame;
        }
    }

    /// End the load and log the whole timeline.
    pub fn finish(&mut self) {
        let Some(total) = self.running_for() else { return };
        self.finished = Some(total);
        self.memory_at_end = self.memory();
        info!("{}", self.report());
    }

    /// Text added under the timeline in its report, such as the asset memory
    /// breakdown. Set before [`finish`](Self::finish) so the logged report
    /// carries it.
    pub fn set_appendix(&mut self, text: impl Into<String>) {
        self.appendix = text.into();
    }

    /// End the load without everything having arrived, naming what had not.
    pub fn give_up(&mut self, waiting_on: impl Into<String>) {
        self.gave_up_waiting_on = Some(waiting_on.into());
        self.finish();
    }

    pub fn has(&self, stage: &str) -> bool {
        self.marks.iter().any(|m| m.stage == stage)
    }

    pub fn is_running(&self) -> bool {
        self.started.is_some() && self.finished.is_none()
    }

    /// How long the current load has been running; `None` when none is.
    pub fn running_for(&self) -> Option<Duration> {
        self.started.filter(|_| self.finished.is_none()).map(|s| s.elapsed())
    }

    /// The whole load's length, once it has finished.
    pub fn finished(&self) -> Option<Duration> {
        self.finished
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn marks(&self) -> &[LoadMark] {
        &self.marks
    }

    /// The timeline as text: one line per stage with the gap since the stage
    /// before it, which is the number that says where the time went.
    pub fn report(&self) -> String {
        if self.started.is_none() {
            return "No project load recorded since the editor started.".to_string();
        }
        let mut out = format!("[loading] {}: ", self.project);
        match (self.finished, &self.gave_up_waiting_on) {
            (Some(total), None) => out.push_str(&format!("loaded in {}\n", ms(total))),
            (Some(total), Some(what)) => {
                out.push_str(&format!("gave up after {}, still waiting on {what}\n", ms(total)))
            }
            (None, _) => out.push_str(&format!(
                "still loading, {} so far\n",
                ms(self.running_for().unwrap_or_default())
            )),
        }
        if self.memory_at_start.is_some() {
            out.push_str(&format!("  at the start{}\n", memory_text(self.memory_at_start, None)));
        }
        let mut previous = Duration::ZERO;
        let mut previous_memory = self.memory_at_start;
        for mark in &self.marks {
            out.push_str(&format!(
                "  {:>9}  (+{:>8})  {}{}{}\n",
                ms(mark.at),
                ms(mark.at.saturating_sub(previous)),
                mark.stage,
                bracket(&mark.detail),
                memory_text(mark.memory, previous_memory)
            ));
            previous = mark.at;
            previous_memory = mark.memory.or(previous_memory);
        }
        if self.memory_at_end.is_some() {
            out.push_str(&format!("  at the end{}\n", memory_text(self.memory_at_end, previous_memory)));
        }
        out.push_str(&format!("  longest frame {}", ms(self.longest_frame)));
        if self.stalls > 0 {
            out.push_str(&format!(
                "; {} frame(s) over {} stalled the editor for {} in all",
                self.stalls,
                ms(STALL),
                ms(self.stalled_for)
            ));
        }
        if !self.appendix.is_empty() {
            out.push('\n');
            out.push_str(&self.appendix);
        }
        out
    }
}

fn ms(d: Duration) -> String {
    format!("{:.0} ms", d.as_secs_f64() * 1000.0)
}

fn gigabytes(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

/// `  [resident 12.63 GB, committed 18.65 GB, +9.10 GB; GPU 1.20 GB in 1.50 GB
/// reserved, +0.40 GB]`: each change is since `before`, which is the part a
/// stage allocated. The GPU's is in what it reserved, since reserved blocks
/// are what memory is actually taken for.
fn memory_text(now: Option<ProcessMemory>, before: Option<ProcessMemory>) -> String {
    let Some(now) = now else { return String::new() };
    let change = |now: u64, before: Option<u64>| {
        before.map_or(String::new(), |before| {
            let delta = now as i128 - before as i128;
            let sign = if delta < 0 { "-" } else { "+" };
            format!(", {sign}{}", gigabytes(delta.unsigned_abs() as u64))
        })
    };
    let gpu = now.gpu.map_or(String::new(), |gpu| {
        format!(
            "; GPU {} in {} reserved{}",
            gigabytes(gpu.allocated),
            gigabytes(gpu.reserved),
            change(gpu.reserved, before.and_then(|b| b.gpu).map(|g| g.reserved))
        )
    });
    format!(
        "  [resident {}, committed {}{}{gpu}]",
        gigabytes(now.resident),
        gigabytes(now.committed),
        change(now.committed, before.map(|b| b.committed))
    )
}

fn bracket(detail: &str) -> String {
    if detail.is_empty() {
        String::new()
    } else {
        format!("  ({detail})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_is_recorded_once_and_only_while_loading() {
        let mut timeline = LoadTimeline::default();
        timeline.mark("scene parsed", "");
        assert!(timeline.marks().is_empty(), "nothing is recorded before a load begins");

        timeline.begin("demo");
        timeline.mark("scene parsed", "first");
        timeline.mark("scene parsed", "second");
        assert_eq!(timeline.marks().len(), 1);
        assert_eq!(timeline.marks()[0].detail, "first");

        timeline.finish();
        timeline.mark("textures loaded", "");
        assert_eq!(timeline.marks().len(), 1, "nothing is recorded after a load ends");
    }

    #[test]
    fn begin_starts_a_new_generation_and_forgets_the_old_load() {
        let mut timeline = LoadTimeline::default();
        timeline.begin("a");
        timeline.mark("scene parsed", "");
        timeline.finish();
        let first = timeline.generation();

        timeline.begin("b");
        assert!(timeline.generation() > first);
        assert!(timeline.marks().is_empty());
        assert!(timeline.is_running());
    }

    #[test]
    fn stalls_are_counted_only_while_loading() {
        let mut timeline = LoadTimeline::default();
        timeline.note_frame(Duration::from_secs(1));
        timeline.begin("demo");
        timeline.note_frame(Duration::from_millis(16));
        timeline.note_frame(Duration::from_millis(400));
        timeline.finish();
        timeline.note_frame(Duration::from_secs(2));
        let report = timeline.report();
        assert!(report.contains("1 frame(s)"), "{report}");
        assert!(report.contains("longest frame 400 ms"), "{report}");
    }

    #[test]
    fn asset_memory_keeps_one_row_per_kind_and_totals_only_process_memory() {
        let mut memory = AssetMemory::default();
        let row = |kind, bytes, in_process| AssetMemoryRow { kind, count: 1, bytes, in_process, note: String::new() };
        memory.set(row("meshes", 10 << 20, true));
        memory.set(row("meshes", 30 << 20, true));
        memory.set(row("images", 20 << 20, true));
        memory.set(row("images on the GPU", 500 << 20, false));
        assert_eq!(memory.rows().len(), 3, "a kind set twice keeps one row");
        let report = memory.report();
        assert!(report.contains("50.0 MB"), "total is meshes + images only: {report}");
        assert!(report.find("meshes").unwrap() < report.find("images").unwrap(), "largest first: {report}");
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn this_process_reports_its_memory() {
        let memory = process_memory().expect("the platform reports process memory");
        assert!(memory.resident > 0 && memory.committed > 0, "{memory:?}");
    }

    #[test]
    fn a_stage_says_what_the_gpu_reserved_beside_the_process() {
        let gib = 1u64 << 30;
        let at = |committed, reserved| ProcessMemory {
            resident: gib,
            committed,
            gpu: Some(GpuMemory { allocated: reserved / 2, reserved }),
        };
        let text = memory_text(Some(at(4 * gib, 3 * gib)), Some(at(2 * gib, gib)));
        assert!(text.contains("committed 4.00 GB, +2.00 GB"), "{text}");
        assert!(text.contains("GPU 1.50 GB in 3.00 GB reserved, +2.00 GB"), "{text}");

        let mut timeline = LoadTimeline::default();
        timeline.set_gpu_probe(Box::new(move || Some(GpuMemory { allocated: gib, reserved: 2 * gib })));
        timeline.begin("demo");
        assert!(timeline.has_gpu_probe(), "the probe outlives a new load");
        timeline.mark("scene parsed", "");
        if let Some(memory) = timeline.marks()[0].memory {
            assert_eq!(memory.gpu, Some(GpuMemory { allocated: gib, reserved: 2 * gib }));
        }
    }

    #[test]
    fn a_load_that_gave_up_says_what_it_was_waiting_on() {
        let mut timeline = LoadTimeline::default();
        timeline.begin("demo");
        timeline.give_up("textures (3 of 10)");
        assert!(timeline.report().contains("still waiting on textures (3 of 10)"));
    }
}
