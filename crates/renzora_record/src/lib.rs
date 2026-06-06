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
mod native;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::math::UVec2;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;

use renzora::core::{CurrentProject, ViewportRenderTarget};
use renzora_ui::Toasts;

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
    /// The whole primary window, including editor chrome and panels.
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

// ─── Panel state snapshot ─────────────────────────────────────────────────────

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
    // String payload (output path) kept for future status display
    Done(#[allow(dead_code)] String),
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

    let project = world
        .get_resource::<CurrentProject>()
        .map(|p| p.path.clone());

    let output_dir = config
        .output_dir
        .clone()
        .or_else(|| project.map(|p| p.join("recordings")));
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
    let Ok(mut guard) = state.inner.lock() else {
        return;
    };
    let Inner::Recording(active) = &mut *guard else {
        return;
    };

    if active.stop_requested {
        return;
    }

    let screenshot = match active.target {
        RecordTarget::Viewport => {
            let Some(handle) = viewport_target.image.clone() else {
                return;
            };
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
                    warn!(
                        "[record] frame {} format conversion failed: {}",
                        frame_idx, e
                    );
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
            let _ = tx.send(Frame {
                bytes: rgba.into_raw(),
            });
        });
}

/// System: when the user has requested stop, transition Recording → Stopping,
/// dropping the encoder sender so the worker drains and finalises the file.
fn drive_stop_transition(state: Res<RecordingState>) {
    let Ok(mut guard) = state.inner.lock() else {
        return;
    };
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
    let Ok(mut guard) = state.inner.lock() else {
        return;
    };
    let take_finished = matches!(&*guard, Inner::Stopping { worker, .. } if worker.is_finished());
    if !take_finished {
        return;
    }

    let Inner::Stopping {
        worker,
        output_path,
    } = std::mem::replace(&mut *guard, Inner::Idle)
    else {
        return;
    };

    match worker.join() {
        Ok(EncoderResult::Ok {
            frames_written,
            output_path,
        }) => {
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

        app.add_plugins(native::NativeRecordPanel);
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
