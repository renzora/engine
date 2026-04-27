//! Background waveform peak generation + cache.
//!
//! For each unique audio file we encounter, we kick off a worker thread that
//! decodes the file via Kira's `StaticSoundData` (so we get the same codec
//! support the playback path has) and downsamples it to a fixed bucket count
//! of `(min, max)` peak pairs. The result is stored in the cache under the
//! file's path and a coarse mtime+size fingerprint so re-imports invalidate
//! cleanly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use bevy::prelude::*;
use kira::sound::static_sound::StaticSoundData;

/// Number of peak buckets we render per clip. ~512 looks crisp on
/// arrangement-view-sized clip rectangles up to ~1500px wide; cheap to keep
/// in memory (4 bytes × 2 × 512 × N clips is negligible).
pub const PEAK_BUCKETS: usize = 512;

#[derive(Clone, Debug)]
pub struct WaveformPeaks {
    /// `(min, max)` per bucket, both in `-1.0..=1.0`.
    pub peaks: Vec<(f32, f32)>,
    /// Source file's natural duration in seconds, useful for the panel
    /// (length of the clip in time, regardless of how many buckets we used).
    pub duration_secs: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct FileFingerprint {
    size: u64,
    mtime: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug)]
enum Entry {
    Pending(FileFingerprint),
    Done(FileFingerprint, WaveformPeaks),
    Failed(FileFingerprint),
}

/// Resource holding the cache and an mpsc-style worker channel.
#[derive(Resource, Clone, Default)]
pub struct WaveformCache {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<PathBuf, Entry>,
}

impl WaveformCache {
    /// Look up peaks for a path. Returns `None` while the file is loading,
    /// failed to load, or hasn't been requested yet (in the latter case
    /// `request` should be called).
    pub fn get(&self, path: &Path) -> Option<WaveformPeaks> {
        let inner = self.inner.lock().ok()?;
        match inner.entries.get(path) {
            Some(Entry::Done(_, peaks)) => Some(peaks.clone()),
            _ => None,
        }
    }

    /// Returns true if a fresh decode should be kicked off for `path` —
    /// either no entry exists, or the existing one is stale per fingerprint.
    pub fn needs_request(&self, path: &Path) -> bool {
        let Some(fp) = fingerprint(path) else { return false };
        let Ok(inner) = self.inner.lock() else { return false };
        match inner.entries.get(path) {
            None => true,
            Some(Entry::Pending(prev)) | Some(Entry::Done(prev, _)) | Some(Entry::Failed(prev)) => {
                *prev != fp
            }
        }
    }

    /// Spawn a worker that decodes `path` and stores peaks in the cache when
    /// done. Idempotent — if the cache already has fresh peaks or a pending
    /// task, this is a no-op.
    pub fn request(&self, path: PathBuf) {
        let Some(fp) = fingerprint(&path) else { return };
        {
            let mut inner = match self.inner.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            // Skip if the existing entry is already current.
            if let Some(existing) = inner.entries.get(&path) {
                let existing_fp = match existing {
                    Entry::Pending(f) | Entry::Done(f, _) | Entry::Failed(f) => f,
                };
                if existing_fp == &fp && !matches!(existing, Entry::Failed(_)) {
                    return;
                }
            }
            inner.entries.insert(path.clone(), Entry::Pending(fp.clone()));
        }

        let cache = self.clone();
        thread::Builder::new()
            .name("waveform-decode".into())
            .spawn(move || {
                let result = decode_to_peaks(&path);
                if let Ok(mut inner) = cache.inner.lock() {
                    match result {
                        Ok(peaks) => {
                            inner.entries.insert(path, Entry::Done(fp, peaks));
                        }
                        Err(e) => {
                            warn!("[Waveform] Failed to decode {}: {}", path.display(), e);
                            inner.entries.insert(path, Entry::Failed(fp));
                        }
                    }
                }
            })
            .ok(); // If the OS refuses to spawn, we just give up on the waveform.
    }
}

fn fingerprint(path: &Path) -> Option<FileFingerprint> {
    let meta = std::fs::metadata(path).ok()?;
    Some(FileFingerprint {
        size: meta.len(),
        mtime: meta.modified().ok(),
    })
}

fn decode_to_peaks(path: &Path) -> Result<WaveformPeaks, String> {
    let data = StaticSoundData::from_file(path).map_err(|e| e.to_string())?;
    let duration_secs = data.duration().as_secs_f64();

    // Kira's `Frame` is `{ left, right }` 32-bit floats in -1..1.
    let frames: &[kira::Frame] = &data.frames;
    let total = frames.len();
    if total == 0 {
        return Ok(WaveformPeaks {
            peaks: Vec::new(),
            duration_secs,
        });
    }

    let buckets = PEAK_BUCKETS.min(total);
    let mut out = Vec::with_capacity(buckets);
    let frames_per = (total as f64 / buckets as f64).max(1.0);
    for b in 0..buckets {
        let lo = (b as f64 * frames_per).floor() as usize;
        let hi = (((b as f64 + 1.0) * frames_per).floor() as usize).min(total);
        let slice = &frames[lo..hi.max(lo + 1)];
        let mut min = 0.0f32;
        let mut max = 0.0f32;
        for f in slice {
            // Mono mix of L/R for visual purposes.
            let s = (f.left + f.right) * 0.5;
            if s < min { min = s; }
            if s > max { max = s; }
        }
        out.push((min, max));
    }

    Ok(WaveformPeaks {
        peaks: out,
        duration_secs,
    })
}
