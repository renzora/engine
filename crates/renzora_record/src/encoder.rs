//! Encoder worker — pipes raw BGRA frames to ffmpeg via stdin.
//!
//! Runs on a background thread so the editor stays responsive while ffmpeg
//! encodes. Uses `ffmpeg-sidecar` so the ffmpeg binary is auto-downloaded the
//! first time recording is started instead of being a build-time dep.

use std::io::Write;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};
use ffmpeg_sidecar::command::FfmpegCommand;

/// One captured viewport frame, ready for the encoder. Frames are sent through
/// the channel in submission order — we trust the screenshot pipeline to
/// preserve that ordering since each capture is queued through Bevy's render
/// graph one frame after the previous.
pub struct Frame {
    pub bytes: Vec<u8>,
}

/// Encoder settings exposed to the panel UI. Defaults aim for "visually
/// lossless" quality at moderate file size.
#[derive(Clone, Debug)]
pub struct EncoderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// libx264 constant-rate-factor: 0 = lossless, 17 = visually lossless,
    /// 23 = ffmpeg default. Lower = better quality + larger file.
    pub crf: u8,
    /// libx264 preset — "slow" gives best size/quality tradeoff. "veryslow"
    /// is marginally better but much slower.
    pub preset: String,
    pub output_path: PathBuf,
}

/// Result reported back to the main thread when the worker exits.
#[derive(Debug)]
pub enum EncoderResult {
    Ok { frames_written: u64, output_path: PathBuf },
    Err(String),
}

/// Spawned by the panel when recording begins. Returns the sender used to
/// push frames + the worker thread handle.
pub struct EncoderHandle {
    pub tx: Sender<Frame>,
    pub worker: JoinHandle<EncoderResult>,
}

/// Hard cap on the in-flight frame queue. If encoding falls behind, the
/// capture system blocks (frames stay in lockstep with rendering rather than
/// silently dropping). 8 frames at 1080p ≈ 64 MB.
const FRAME_QUEUE_DEPTH: usize = 8;

pub fn start(settings: EncoderSettings) -> Result<EncoderHandle, String> {
    if let Some(parent) = settings.output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create output dir: {}", e))?;
    }

    let output_str = settings
        .output_path
        .to_str()
        .ok_or_else(|| "output path is not valid UTF-8".to_string())?
        .to_string();

    let size_arg = format!("{}x{}", settings.width, settings.height);
    let fps_arg = settings.fps.to_string();
    let crf_arg = settings.crf.to_string();

    // yuv420p needs even dimensions on both axes — viewport sizes are
    // arbitrary user-driven values, so crop down to the nearest even rect
    // before encoding. Costs at most one pixel column/row.
    let crop_filter = "crop=trunc(iw/2)*2:trunc(ih/2)*2";

    let mut child = FfmpegCommand::new()
        .hide_banner()
        .overwrite()
        .args(["-f", "rawvideo"])
        // Frames arrive as RGBA8 — viewport captures are converted from
        // BGRA8UnormSrgb in the observer, window captures are
        // platform-dependent and likewise normalised before the channel.
        .args(["-pix_fmt", "rgba"])
        .args(["-s:v", &size_arg])
        .args(["-r", &fps_arg])
        .args(["-i", "-"])
        .args(["-vf", crop_filter])
        .args(["-c:v", "libx264"])
        .args(["-preset", &settings.preset])
        .args(["-crf", &crf_arg])
        .args(["-pix_fmt", "yuv420p"])
        .args(["-movflags", "+faststart"])
        .output(&output_str)
        .spawn()
        .map_err(|e| format!("failed to spawn ffmpeg: {}", e))?;

    let mut stdin = child
        .take_stdin()
        .ok_or_else(|| "ffmpeg child has no stdin".to_string())?;

    let (tx, rx): (Sender<Frame>, Receiver<Frame>) = bounded(FRAME_QUEUE_DEPTH);
    let output_path = settings.output_path.clone();

    let worker = thread::Builder::new()
        .name("renzora-record-encoder".into())
        .spawn(move || {
            let mut frames_written = 0u64;
            while let Ok(frame) = rx.recv() {
                if let Err(e) = stdin.write_all(&frame.bytes) {
                    let _ = child.kill();
                    return EncoderResult::Err(format!("write to ffmpeg stdin: {}", e));
                }
                frames_written += 1;
            }

            // Channel closed → finalize. Dropping stdin signals EOF to ffmpeg
            // so it flushes the muxer and exits cleanly.
            drop(stdin);

            match child.wait() {
                Ok(status) if status.success() => EncoderResult::Ok {
                    frames_written,
                    output_path,
                },
                Ok(status) => EncoderResult::Err(format!(
                    "ffmpeg exited with status {:?}",
                    status.code()
                )),
                Err(e) => EncoderResult::Err(format!("waiting on ffmpeg: {}", e)),
            }
        })
        .map_err(|e| format!("failed to spawn encoder thread: {}", e))?;

    Ok(EncoderHandle { tx, worker })
}

/// Best-effort prefetch of the ffmpeg binary so the first record click isn't
/// blocked by a download. Idempotent.
pub fn ensure_ffmpeg_available() -> Result<(), String> {
    use ffmpeg_sidecar::{command::ffmpeg_is_installed, download::auto_download};
    if ffmpeg_is_installed() {
        return Ok(());
    }
    auto_download().map_err(|e| format!("ffmpeg auto-download failed: {}", e))
}
