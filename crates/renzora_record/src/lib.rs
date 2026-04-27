//! Record panel — captures the viewport render target and encodes it to an
//! H.264 MP4 via a backgrounded ffmpeg process.
//!
//! Output frames are read back from the viewport's offscreen texture using
//! Bevy's [`Screenshot`] API and piped to ffmpeg over stdin. Encoding runs on
//! a worker thread (see [`encoder`]) so the editor doesn't stall.
//!
//! Quality defaults aim for "visually lossless" — libx264 CRF 17, preset
//! slow, yuv420p. The output file lives under `<project>/recordings/` with a
//! timestamped name unless a custom path is set in the panel.

mod encoder;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::math::UVec2;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;
use bevy_egui::egui::{self, Color32};
use egui_phosphor::regular;

use renzora::core::{CurrentProject, ViewportRenderTarget};
use renzora_editor::{AppEditorExt, EditorCommands};
use renzora_theme::{Theme, ThemeManager};
use renzora_ui::{
    alert, enum_combobox, inline_property, section_header, spinner, AlertKind, EditorPanel,
    PanelLocation, Toasts,
};

use crate::encoder::{
    ensure_ffmpeg_available, start as start_encoder, EncoderHandle, EncoderResult, EncoderSettings,
    Frame,
};

// ─── Resources ──────────────────────────────────────────────────────────────

/// Top-level recording state. Behind a `Mutex` because the panel `ui` runs
/// against `&World` while the start/stop commands run with `&mut World` —
/// the lock keeps both sides honest.
#[derive(Resource, Default)]
pub struct RecordingState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
enum Inner {
    #[default]
    Idle,
    Recording(Active),
    /// Stop was requested; encoder is still flushing. Held so we can surface
    /// the final result in the UI when the worker exits.
    Stopping {
        worker: std::thread::JoinHandle<EncoderResult>,
        output_path: PathBuf,
    },
    /// Last-result toast shown until the user starts another recording.
    Done(String),
}

struct Active {
    handle: EncoderHandle,
    /// What we're recording — fixed for the duration of one recording.
    target: RecordTarget,
    /// Captured at record-start so the texture's CPU-side size can't drift
    /// from what ffmpeg was configured to expect.
    locked_size: UVec2,
    started_at: Instant,
    frame_index: u64,
    output_path: PathBuf,
    /// Set true when stop is requested; the next frame will tear down.
    stop_requested: bool,
}

/// What surface to capture into the video.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum RecordTarget {
    /// The 3D viewport's offscreen render texture — just the scene, no
    /// editor chrome.
    #[default]
    Viewport,
    /// The whole primary window, including egui chrome and panels.
    Window,
}

/// Tracks the background ffmpeg-binary prefetch. ffmpeg-sidecar downloads
/// ~30MB on first use; doing it inline on the first Record click freezes the
/// editor for seconds, so the prefetch runs on a background thread at plugin
/// startup and the panel disables the Record button until it's ready.
#[derive(Resource, Clone)]
pub struct FfmpegReadiness {
    state: Arc<Mutex<FfmpegState>>,
}

#[derive(Clone)]
enum FfmpegState {
    Preparing,
    Ready,
    Failed(String),
}

impl FfmpegReadiness {
    fn snapshot(&self) -> FfmpegState {
        self.state
            .lock()
            .map(|g| (*g).clone())
            .unwrap_or(FfmpegState::Failed("readiness lock poisoned".into()))
    }
}

/// User-facing encoder settings, edited from the panel UI.
#[derive(Resource, Clone)]
pub struct RecordingConfig {
    pub target: RecordTarget,
    pub fps: u32,
    pub crf: u8,
    pub preset: String,
    /// Custom output directory. `None` ⇒ `<project>/recordings/`.
    pub output_dir: Option<PathBuf>,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            target: RecordTarget::Viewport,
            fps: 60,
            crf: 17,
            preset: "slow".to_string(),
            output_dir: None,
        }
    }
}

// ─── Panel ──────────────────────────────────────────────────────────────────

pub struct RecordPanel;

impl Default for RecordPanel {
    fn default() -> Self {
        Self
    }
}

impl EditorPanel for RecordPanel {
    fn id(&self) -> &str {
        "record"
    }

    fn title(&self) -> &str {
        "Record"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::RECORD)
    }

    fn min_size(&self) -> [f32; 2] {
        [220.0, 200.0]
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let Some(tm) = world.get_resource::<ThemeManager>() else {
            ui.label("Theme not loaded yet.");
            return;
        };
        let theme = tm.active_theme.clone();
        let theme = &theme;

        let cmds = world.get_resource::<EditorCommands>();
        let state = world.get_resource::<RecordingState>();
        let config = world.get_resource::<RecordingConfig>();
        let readiness = world.get_resource::<FfmpegReadiness>();
        let (Some(state), Some(config), Some(cmds), Some(readiness)) =
            (state, config, cmds, readiness)
        else {
            ui.label("Recording resources not initialised yet.");
            return;
        };

        let snapshot = state
            .inner
            .lock()
            .map(|g| InnerSnapshot::from(&*g))
            .unwrap_or(InnerSnapshot::Idle);
        let ffmpeg = readiness.snapshot();

        ui.add_space(8.0);
        draw_top_row(ui, &snapshot, &ffmpeg, theme, cmds);
        ui.add_space(8.0);

        // Surface readiness / failure as themed alerts. Successful saves are
        // announced via a toast (see `drive_finalise`) so the panel doesn't
        // hold a stale "Saved" banner.
        match (&snapshot, &ffmpeg) {
            (InnerSnapshot::Idle | InnerSnapshot::Done(_), FfmpegState::Preparing) => {
                alert(
                    ui,
                    AlertKind::Info,
                    "Preparing ffmpeg",
                    "Downloading the ffmpeg binary — record will be available shortly.",
                    theme,
                );
            }
            (_, FfmpegState::Failed(e)) => {
                alert(ui, AlertKind::Error, "ffmpeg unavailable", e, theme);
            }
            _ => {}
        }

        ui.add_space(8.0);
        section_header(ui, "Settings", theme);

        let editable = matches!(snapshot, InnerSnapshot::Idle | InnerSnapshot::Done(_));
        ui.add_enabled_ui(editable, |ui| {
            draw_settings_rows(ui, theme, config, cmds);
        });

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Saved as MP4 (H.264) under <project>/recordings/")
                .small()
                .color(theme.text.muted.to_color32()),
        );
    }
}

/// Top row: record/stop button on the left, status text on the right.
fn draw_top_row(
    ui: &mut egui::Ui,
    snapshot: &InnerSnapshot,
    ffmpeg: &FfmpegState,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        match snapshot {
            InnerSnapshot::Idle | InnerSnapshot::Done(_) => match ffmpeg {
                FfmpegState::Ready => {
                    if record_button(ui, false) {
                        cmds.push(|world: &mut World| start_recording(world));
                    }
                }
                FfmpegState::Preparing => {
                    ui.add_enabled(false, egui::Button::new("Preparing ffmpeg…"));
                    spinner(ui, 14.0, theme);
                }
                FfmpegState::Failed(_) => {
                    ui.add_enabled(false, egui::Button::new("ffmpeg unavailable"));
                }
            },
            InnerSnapshot::Recording { .. } => {
                if record_button(ui, true) {
                    cmds.push(|world: &mut World| request_stop(world));
                }
            }
            InnerSnapshot::Stopping => {
                ui.add_enabled(false, egui::Button::new("Encoding…"));
                spinner(ui, 14.0, theme);
            }
        }
        ui.add_space(8.0);

        match snapshot {
            InnerSnapshot::Idle if matches!(ffmpeg, FfmpegState::Ready) => {
                ui.label(
                    egui::RichText::new("Idle").color(theme.text.muted.to_color32()),
                );
            }
            InnerSnapshot::Recording { elapsed_secs, frame_index, size } => {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!("● REC  {}", fmt_elapsed(*elapsed_secs)))
                            .color(Color32::from_rgb(220, 60, 60))
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} frames  ·  {}×{}",
                            frame_index, size.x, size.y
                        ))
                        .small()
                        .color(theme.text.muted.to_color32()),
                    );
                });
            }
            InnerSnapshot::Stopping => {
                ui.label(
                    egui::RichText::new("Finalising file…")
                        .italics()
                        .color(theme.text.muted.to_color32()),
                );
            }
            _ => {}
        }
    });
}

/// Source / FPS / Quality / Preset rows, each as an `inline_property`.
fn draw_settings_rows(
    ui: &mut egui::Ui,
    theme: &Theme,
    config: &RecordingConfig,
    cmds: &EditorCommands,
) {
    const TARGET_VALUES: [RecordTarget; 2] = [RecordTarget::Viewport, RecordTarget::Window];
    const TARGET_LABELS: &[&str] = &[
        "Viewport (3D scene only)",
        "Entire window (full editor)",
    ];
    const FPS_VALUES: [u32; 4] = [24, 30, 60, 120];
    const FPS_LABELS: &[&str] = &["24", "30", "60", "120"];
    const CRF_VALUES: [u8; 5] = [0, 12, 17, 20, 23];
    const CRF_LABELS: &[&str] = &[
        "Lossless (huge files)",
        "Archival",
        "Visually lossless",
        "High",
        "Default",
    ];
    const PRESET_OPTIONS: &[&str] = &["ultrafast", "fast", "medium", "slow", "veryslow"];

    let target_idx = TARGET_VALUES
        .iter()
        .position(|t| *t == config.target)
        .unwrap_or(0);
    inline_property(ui, 0, "Source", theme, |ui| {
        if let Some(idx) =
            enum_combobox(ui, egui::Id::new("record_target"), target_idx, TARGET_LABELS)
        {
            let new_target = TARGET_VALUES[idx];
            cmds.push(move |w: &mut World| {
                if let Some(mut cfg) = w.get_resource_mut::<RecordingConfig>() {
                    cfg.target = new_target;
                }
            });
        }
    });

    let fps_idx = FPS_VALUES
        .iter()
        .position(|f| *f == config.fps)
        .unwrap_or(2);
    inline_property(ui, 1, "FPS", theme, |ui| {
        if let Some(idx) = enum_combobox(ui, egui::Id::new("record_fps"), fps_idx, FPS_LABELS) {
            let new_fps = FPS_VALUES[idx];
            cmds.push(move |w: &mut World| {
                if let Some(mut cfg) = w.get_resource_mut::<RecordingConfig>() {
                    cfg.fps = new_fps;
                }
            });
        }
    });

    let crf_idx = CRF_VALUES
        .iter()
        .position(|c| *c == config.crf)
        .unwrap_or(2);
    inline_property(ui, 2, "Quality", theme, |ui| {
        if let Some(idx) = enum_combobox(ui, egui::Id::new("record_crf"), crf_idx, CRF_LABELS) {
            let new_crf = CRF_VALUES[idx];
            cmds.push(move |w: &mut World| {
                if let Some(mut cfg) = w.get_resource_mut::<RecordingConfig>() {
                    cfg.crf = new_crf;
                }
            });
        }
    });

    let preset_idx = PRESET_OPTIONS
        .iter()
        .position(|p| *p == config.preset)
        .unwrap_or(3);
    inline_property(ui, 3, "Preset", theme, |ui| {
        if let Some(idx) = enum_combobox(
            ui,
            egui::Id::new("record_preset"),
            preset_idx,
            PRESET_OPTIONS,
        ) {
            let new_preset = PRESET_OPTIONS[idx].to_string();
            cmds.push(move |w: &mut World| {
                if let Some(mut cfg) = w.get_resource_mut::<RecordingConfig>() {
                    cfg.preset = new_preset;
                }
            });
        }
    });
}

/// Read-only snapshot of the recording state for the UI. The lock is dropped
/// before drawing.
enum InnerSnapshot {
    Idle,
    Recording {
        elapsed_secs: f32,
        frame_index: u64,
        size: UVec2,
    },
    Stopping,
    Done(String),
}

impl From<&Inner> for InnerSnapshot {
    fn from(inner: &Inner) -> Self {
        match inner {
            Inner::Idle => InnerSnapshot::Idle,
            Inner::Recording(active) => InnerSnapshot::Recording {
                elapsed_secs: active.started_at.elapsed().as_secs_f32(),
                frame_index: active.frame_index,
                size: active.locked_size,
            },
            Inner::Stopping { .. } => InnerSnapshot::Stopping,
            Inner::Done(msg) => InnerSnapshot::Done(msg.clone()),
        }
    }
}

fn record_button(ui: &mut egui::Ui, active: bool) -> bool {
    let (label, color) = if active {
        (format!("{}  Stop", regular::STOP), Color32::from_rgb(220, 60, 60))
    } else {
        (format!("{}  Record", regular::RECORD), Color32::from_rgb(220, 60, 60))
    };
    let btn = egui::Button::new(
        egui::RichText::new(label)
            .size(13.0)
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(color)
    .min_size(egui::vec2(110.0, 28.0));
    ui.add(btn).clicked()
}

fn fmt_elapsed(secs: f32) -> String {
    let total = secs as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{:02}:{:02}", m, s)
}

// ─── Start / stop / capture ─────────────────────────────────────────────────

fn start_recording(world: &mut World) {
    let config = world
        .get_resource::<RecordingConfig>()
        .cloned()
        .unwrap_or_default();

    let Some(size) = resolve_target_size(world, config.target) else {
        warn!(
            "[record] Could not determine size for target {:?} — nothing to record",
            config.target
        );
        return;
    };

    let project = world.get_resource::<CurrentProject>().map(|p| p.path.clone());

    let output_dir = config.output_dir.clone().or_else(|| {
        project.map(|p| p.join("recordings"))
    });
    let Some(output_dir) = output_dir else {
        warn!("[record] No project open and no output_dir set — refusing to record");
        return;
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let output_path = output_dir.join(format!("recording_{}.mp4", timestamp));

    // ffmpeg readiness is prefetched in the background at plugin startup —
    // the panel UI gates the Record button on `FfmpegState::Ready`, so we
    // can spawn the encoder directly here without blocking on a download.

    let settings = EncoderSettings {
        width: size.x,
        height: size.y,
        fps: config.fps,
        crf: config.crf,
        preset: config.preset.clone(),
        output_path: output_path.clone(),
    };

    let handle_enc = match start_encoder(settings) {
        Ok(h) => h,
        Err(e) => {
            warn!("[record] Failed to start encoder: {}", e);
            store_done_message(world, format!("Encoder failed to start: {}", e));
            return;
        }
    };

    info!(
        "[record] Recording started → {} (target: {:?}, {}×{} @ {} fps, crf {}, {})",
        output_path.display(),
        config.target,
        size.x,
        size.y,
        config.fps,
        config.crf,
        config.preset
    );

    let state = world.resource::<RecordingState>();
    if let Ok(mut guard) = state.inner.lock() {
        *guard = Inner::Recording(Active {
            handle: handle_enc,
            target: config.target,
            locked_size: size,
            started_at: Instant::now(),
            frame_index: 0,
            output_path,
            stop_requested: false,
        });
    }
}

/// Compute the pixel dimensions we'll lock the encoder to, based on what
/// surface the user wants to capture.
fn resolve_target_size(world: &mut World, target: RecordTarget) -> Option<UVec2> {
    match target {
        RecordTarget::Viewport => {
            let handle = world
                .get_resource::<ViewportRenderTarget>()
                .and_then(|t| t.image.clone())?;
            world
                .get_resource::<Assets<Image>>()
                .and_then(|imgs| imgs.get(&handle))
                .map(|img| {
                    let s = img.texture_descriptor.size;
                    UVec2::new(s.width, s.height)
                })
        }
        RecordTarget::Window => {
            let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
            let win = q.iter(world).next()?;
            let w = win.resolution.physical_width();
            let h = win.resolution.physical_height();
            if w == 0 || h == 0 {
                None
            } else {
                Some(UVec2::new(w, h))
            }
        }
    }
}

fn request_stop(world: &mut World) {
    let state = world.resource::<RecordingState>();
    if let Ok(mut guard) = state.inner.lock() {
        if let Inner::Recording(active) = &mut *guard {
            active.stop_requested = true;
        }
    }
}

fn store_done_message(world: &mut World, msg: String) {
    let state = world.resource::<RecordingState>();
    if let Ok(mut guard) = state.inner.lock() {
        *guard = Inner::Done(msg);
    }
}

/// System: when recording, queue a Screenshot of the configured target each
/// frame. The screenshot capture is asynchronous — its observer extracts the
/// pixel data and forwards it to the encoder thread.
///
/// This runs in `Last` so it's the final thing we ask Bevy to do for the
/// frame, after viewport rendering has been queued.
fn capture_frame_system(
    mut commands: Commands,
    state: Res<RecordingState>,
    viewport_target: Res<ViewportRenderTarget>,
) {
    let Ok(mut guard) = state.inner.lock() else { return };
    let Inner::Recording(active) = &mut *guard else { return };

    if active.stop_requested {
        return;
    }

    let screenshot = match active.target {
        RecordTarget::Viewport => {
            let Some(handle) = viewport_target.image.clone() else { return };
            Screenshot::image(handle)
        }
        RecordTarget::Window => Screenshot::primary_window(),
    };

    let tx = active.handle.tx.clone();
    let frame_idx = active.frame_index;
    active.frame_index = active.frame_index.saturating_add(1);
    let expected_size = active.locked_size;

    commands
        .spawn(screenshot)
        .observe(move |trigger: On<ScreenshotCaptured>| {
            // Normalise to RGBA8 regardless of the source texture format.
            // Cheap byte swap when the source is already 8-bit RGBA/BGRA;
            // handles platform differences in the window swapchain format.
            let dyn_img = match trigger.image.clone().try_into_dynamic() {
                Ok(d) => d,
                Err(e) => {
                    warn!("[record] frame {} format conversion failed: {}", frame_idx, e);
                    return;
                }
            };
            let rgba = dyn_img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if w != expected_size.x || h != expected_size.y {
                // Source surface was resized mid-recording. Drop the frame
                // rather than send mismatched bytes.
                warn!(
                    "[record] dropped frame {} — source resized to {}×{}, encoder expects {}×{}",
                    frame_idx, w, h, expected_size.x, expected_size.y
                );
                return;
            }
            let _ = tx.send(Frame { bytes: rgba.into_raw() });
        });
}

/// System: when the user has requested stop, transition Recording → Stopping,
/// dropping the encoder sender so the worker drains and finalises the file.
fn drive_stop_transition(state: Res<RecordingState>) {
    let Ok(mut guard) = state.inner.lock() else { return };
    let needs_transition = matches!(&*guard, Inner::Recording(a) if a.stop_requested);
    if !needs_transition {
        return;
    }
    if let Inner::Recording(active) = std::mem::replace(&mut *guard, Inner::Idle) {
        // Drop the sender so the worker's `recv` returns Err and it finalises
        // ffmpeg.
        let EncoderHandle { tx, worker } = active.handle;
        drop(tx);
        *guard = Inner::Stopping {
            worker,
            output_path: active.output_path,
        };
    }
}

/// System: when in Stopping, poll the worker. Once it exits, transition to
/// Done with a status message and fire a toast on success.
fn drive_finalise(state: Res<RecordingState>, mut toasts: Option<ResMut<Toasts>>) {
    let Ok(mut guard) = state.inner.lock() else { return };
    let take_finished = matches!(&*guard, Inner::Stopping { worker, .. } if worker.is_finished());
    if !take_finished {
        return;
    }

    let Inner::Stopping { worker, output_path } = std::mem::replace(&mut *guard, Inner::Idle) else {
        return;
    };

    match worker.join() {
        Ok(EncoderResult::Ok { frames_written, output_path }) => {
            info!(
                "[record] Saved {} frames → {}",
                frames_written,
                output_path.display()
            );
            let file_name = output_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("recording.mp4");
            if let Some(toasts) = toasts.as_mut() {
                toasts.success(format!("Recording saved · {}", file_name));
            }
            *guard = Inner::Done(file_name.to_string());
        }
        Ok(EncoderResult::Err(e)) => {
            warn!("[record] Encoder error: {}", e);
            if let Some(toasts) = toasts.as_mut() {
                toasts.error(format!("Recording failed: {}", e));
            }
            *guard = Inner::Done(format!("Encoder error: {} ({})", e, output_path.display()));
        }
        Err(_) => {
            if let Some(toasts) = toasts.as_mut() {
                toasts.error("Recording failed: encoder thread panicked");
            }
            *guard = Inner::Done("Encoder thread panicked".to_string());
        }
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct RecordPlugin;

impl Plugin for RecordPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] RecordPlugin");

        app.init_resource::<RecordingState>();
        app.init_resource::<RecordingConfig>();
        app.insert_resource(spawn_ffmpeg_prefetch());

        app.add_systems(Last, capture_frame_system);
        app.add_systems(Update, (drive_stop_transition, drive_finalise).chain());

        app.register_panel(RecordPanel::default());
    }
}

/// Kick off ffmpeg auto-download on a background thread so the first Record
/// click doesn't freeze the editor for the duration of the network fetch.
fn spawn_ffmpeg_prefetch() -> FfmpegReadiness {
    let state = Arc::new(Mutex::new(FfmpegState::Preparing));
    let writer = state.clone();
    std::thread::Builder::new()
        .name("renzora-record-ffmpeg-prefetch".into())
        .spawn(move || {
            let result = ensure_ffmpeg_available();
            if let Ok(mut guard) = writer.lock() {
                *guard = match result {
                    Ok(()) => FfmpegState::Ready,
                    Err(e) => FfmpegState::Failed(e),
                };
            }
        })
        .expect("failed to spawn ffmpeg prefetch thread");
    FfmpegReadiness { state }
}

renzora::add!(RecordPlugin, Editor);

