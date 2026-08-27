//! Unpacking the `sdk.tar.xz` that ships inside the engine download.
//!
//! A release carries the SDK compressed and unextracted — one ~555 MB file that
//! costs a user who never writes a plugin nothing but disk. The first time
//! someone installs one, it becomes the ~3.6 GB `sdk/` tree the compiler reads.
//!
//! # Why the extraction is staged and renamed
//!
//! [`Sdk::load`](crate::Sdk::load) decides an SDK is present by finding
//! `manifest.json`. That file is small and lands early, so an extraction
//! interrupted a third of the way through — a full disk, a closed lid, a killed
//! process — would leave a directory that *loads* and then fails at compile time
//! with a missing-crate error pointing at nothing.
//!
//! So the tree is built under a `.partial` name and renamed into place only once
//! it is complete. A rename is atomic on every filesystem this runs on, so `sdk/`
//! either does not exist or is whole.
//!
//! # Why it decompresses to a temporary file first
//!
//! `lzma-rs` decodes a whole xz stream into a writer; it has no `Read` adapter to
//! chain a tar reader onto. Decompressing into memory would mean holding 3.6 GB,
//! so it goes to a temporary file and is untarred from there. That is ~7 GB of
//! transient disk during the one extraction, released as soon as it finishes.
//!
//! Streaming would be possible with zstd, whose `ruzstd` decoder does implement
//! `Read` — worth revisiting if the transient disk ever matters more than xz's
//! better ratio.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

/// What the SDK looks like on disk right now.
#[derive(Debug, Clone)]
pub enum SdkState {
    /// Extracted and usable.
    Ready,
    /// Shipped but not yet unpacked. The first plugin install does this.
    Packed { archive: PathBuf, bytes: u64 },
    /// Neither a tree nor an archive — a build that shipped without one.
    Absent,
}

/// Inspect `root` (the directory holding the executables) for an SDK.
///
/// Prefers an extracted tree, so a re-run after unpacking is a cheap stat rather
/// than a repeated extraction.
pub fn sdk_state(root: &Path) -> SdkState {
    if root.join("sdk").join("manifest.json").is_file() {
        return SdkState::Ready;
    }
    let archive = root.join("sdk.tar.xz");
    match std::fs::metadata(&archive) {
        Ok(m) if m.is_file() => SdkState::Packed { archive, bytes: m.len() },
        _ => SdkState::Absent,
    }
}

/// Unpack `archive` into `<root>/sdk/`.
///
/// `progress` is called with bytes written so far, for a UI that has a user
/// waiting on it. It is called during decompression only — the tar pass is
/// comparatively quick, and a bar that stalls at 100% is better than one that
/// restarts.
pub fn extract(
    archive: &Path,
    root: &Path,
    mut progress: impl FnMut(u64),
) -> Result<PathBuf, String> {
    let final_dir = root.join("sdk");
    if final_dir.join("manifest.json").is_file() {
        return Ok(final_dir);
    }

    // Both scratch paths sit beside the destination rather than in the system
    // temp directory: this is gigabytes, and `/tmp` is a ramdisk on plenty of
    // Linux installs. Next to the target is also guaranteed to be the same
    // filesystem, which is what makes the final rename atomic rather than a copy.
    let staging = root.join("sdk.partial");
    let tar_path = root.join("sdk.tar.partial");
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_file(&tar_path);

    let result = (|| -> Result<(), String> {
        decompress(archive, &tar_path, &mut progress)?;
        unpack_tar(&tar_path, &staging)?;
        Ok(())
    })();

    let _ = std::fs::remove_file(&tar_path);
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    // The archive holds a top-level `sdk/`, so the staged tree is
    // `sdk.partial/sdk/…`. Rename that inner directory, not its wrapper.
    let inner = staging.join("sdk");
    let src = if inner.is_dir() { inner } else { staging.clone() };
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&src, &final_dir).map_err(|e| {
        format!("could not move the unpacked SDK into place: {e}")
    })?;
    let _ = std::fs::remove_dir_all(&staging);

    if !final_dir.join("manifest.json").is_file() {
        return Err("the SDK archive unpacked without a manifest.json".to_string());
    }
    Ok(final_dir)
}

fn decompress(archive: &Path, out: &Path, progress: &mut impl FnMut(u64)) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| format!("could not open {}: {e}", archive.display()))?;
    let mut reader = BufReader::new(file);
    let out_file =
        File::create(out).map_err(|e| format!("could not write {}: {e}", out.display()))?;
    let mut writer = Counting { inner: BufWriter::new(out_file), written: 0, progress };
    lzma_rs::xz_decompress(&mut reader, &mut writer)
        .map_err(|e| format!("the SDK archive is corrupt or truncated: {e:?}"))
}

fn unpack_tar(tar_path: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = File::open(tar_path).map_err(|e| e.to_string())?;
    tar::Archive::new(BufReader::new(file))
        .unpack(dest)
        .map_err(|e| format!("could not unpack the SDK: {e}"))
}

/// A writer that reports how much has gone through it.
struct Counting<'a, W, F> {
    inner: W,
    written: u64,
    progress: &'a mut F,
}

impl<W: std::io::Write, F: FnMut(u64)> std::io::Write for Counting<'_, W, F> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.written += n as u64;
        (self.progress)(self.written);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
