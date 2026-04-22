//! State types for the docker builder panel.
//!
//! A Bevy resource owns the authoritative state. The panel reads it via `&World`
//! and pushes `UiAction`s through an `Arc<Mutex<Vec<_>>>` bridge; a system
//! consumes those actions and drains the worker-thread channel each frame.

use bevy::prelude::Resource;
use std::collections::VecDeque;
use std::sync::{atomic::AtomicBool, mpsc, Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStatus {
    Pending,
    InProgress,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct BuildTarget {
    pub platform: String,
    pub feature: String,
    pub status: TargetStatus,
    pub crates_compiled: u32,
    pub last_line: String,
    pub error: Option<String>,
}

impl BuildTarget {
    pub fn new(platform: &str, feature: &str) -> Self {
        Self {
            platform: platform.to_string(),
            feature: feature.to_string(),
            status: TargetStatus::Pending,
            crates_compiled: 0,
            last_line: String::new(),
            error: None,
        }
    }

    pub fn progress(&self) -> f32 {
        match self.status {
            TargetStatus::Pending => 0.0,
            TargetStatus::InProgress => {
                let est = 250.0_f32;
                (self.crates_compiled as f32 / est).clamp(0.02, 0.98)
            }
            TargetStatus::Done => 1.0,
            TargetStatus::Failed => 1.0,
        }
    }
}

pub fn default_targets() -> Vec<BuildTarget> {
    let mut v = Vec::new();
    for p in &["linux-x64", "windows-x64", "macos-x64", "macos-arm64"] {
        for f in &["editor", "runtime", "server"] {
            v.push(BuildTarget::new(p, f));
        }
    }
    v.push(BuildTarget::new("web-wasm32", "runtime"));
    v.push(BuildTarget::new("android-arm64", "runtime"));
    v.push(BuildTarget::new("android-x86", "runtime"));
    v.push(BuildTarget::new("ios-arm64", "runtime"));
    v
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Idle,
    BuildingImage,
    StartingContainer,
    Building,
    Cleaning,
    Done,
    Failed(String),
}

/// Messages produced by worker threads.
pub enum WorkerMsg {
    Stage(Stage),
    Log(String),
    TargetStart(String, String),
    TargetCompileTick(String, String),
    TargetDone(String, String),
    TargetFailed(String, String, String),
    Finished,
}

#[derive(Debug, Clone)]
pub enum UiAction {
    BuildImage,
    BuildAll,
    Stop,
    CleanCache,
    ClearLogs,
}

/// Authoritative state — owned by the drain system, read-only from the panel.
#[derive(Resource)]
pub struct DockerBuilderState {
    pub targets: Vec<BuildTarget>,
    pub logs: VecDeque<String>,
    pub log_cap: usize,
    pub stage: Stage,
    pub running: bool,
    /// Channel to drain this frame (None if nothing active).
    pub rx: Option<Mutex<mpsc::Receiver<WorkerMsg>>>,
    /// Flag the worker polls to abort early.
    pub stop_flag: Option<Arc<AtomicBool>>,
}

impl Default for DockerBuilderState {
    fn default() -> Self {
        Self {
            targets: default_targets(),
            logs: VecDeque::new(),
            log_cap: 4000,
            stage: Stage::Idle,
            running: false,
            rx: None,
            stop_flag: None,
        }
    }
}

impl DockerBuilderState {
    pub fn push_log(&mut self, line: String) {
        if self.logs.len() >= self.log_cap {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }

    pub fn reset_targets(&mut self) {
        for t in &mut self.targets {
            t.status = TargetStatus::Pending;
            t.crates_compiled = 0;
            t.last_line.clear();
            t.error = None;
        }
    }

    pub fn target_mut(&mut self, platform: &str, feature: &str) -> Option<&mut BuildTarget> {
        self.targets
            .iter_mut()
            .find(|t| t.platform == platform && t.feature == feature)
    }
}

/// Action bridge — panel writes, system drains.
#[derive(Resource, Clone, Default)]
pub struct ActionBridge {
    pub actions: Arc<Mutex<Vec<UiAction>>>,
    pub settings: Arc<Mutex<PanelSettings>>,
}

#[derive(Debug, Clone)]
pub struct PanelSettings {
    pub auto_scroll: bool,
    pub search_filter: String,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            auto_scroll: true,
            search_filter: String::new(),
        }
    }
}
