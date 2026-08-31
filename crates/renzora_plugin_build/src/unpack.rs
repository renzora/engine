//! Unpacking the `sdk.tar.zst` that ships inside the engine download.
//!
//! A release carries the SDK compressed and unextracted — one ~444 MB file that
//! becomes the ~1.9 GB `sdk/` tree the compiler reads. Rust scripts and native
//! plugins both need it, so unpacking is part of setting the engine up rather
//! than an optional extra.
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
//! # Why zstd and not xz
//!
//! xz compresses this tree better — 341 MB against 444 MB at zstd's level 19 —
//! and for a long time that looked like the right trade for a component most
//! people would never touch. It stopped being right once Rust scripts started
//! needing the SDK too: unpacking is now on the path of anyone using the engine,
//! so its cost is paid by everyone and the download's is paid once.
//!
//! Measured on the real archive:
//!
//! | | archive | unpack |
//! |---|---|---|
//! | xz, via `lzma-rs` | 341 MB | **29.8 s** |
//! | zstd -19 | 444 MB | **2.1 s** decode |
//!
//! The obvious fix — swapping `lzma-rs` for the C `xz2` — does not work, and it
//! is worth recording why so nobody tries it again. Single-threaded liblzma
//! decodes this archive in **34.6 s**, *slower* than the pure-Rust decoder doing
//! strictly more work. The 1.6 s that `xz -T0` achieves is entirely 32-way
//! parallelism, and that is unreachable here: `lzma-sys 0.1.20` bundles liblzma
//! **5.2**, `lzma_stream_decoder_mt` arrived in **5.4**, and only the *encoder*
//! MT entry points are bound.
//!
//! zstd also removes a whole pass. `lzma-rs` decodes into a `Write` with no
//! `Read` adapter, so the previous version landed a ~1.9 GB tarball on disk and
//! read it back — ~3.4 GB of transient disk, plus an extra write and read of the
//! entire SDK. `zstd::Decoder` implements `Read` and chains straight into the tar
//! reader, so the intermediate file is gone and the only bytes written are the
//! tree itself.

use std::fs::File;
use std::io::BufReader;
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
    let archive = root.join("sdk.tar.zst");
    match std::fs::metadata(&archive) {
        Ok(m) if m.is_file() => SdkState::Packed { archive, bytes: m.len() },
        _ => SdkState::Absent,
    }
}

/// Unpack `archive` into `<root>/sdk/`.
///
/// `progress` is called with compressed bytes consumed so far, for a UI that has
/// a user waiting on it. Compressed rather than decompressed, because the
/// decoder now feeds the tar reader directly: there is no intermediate size to
/// count, and the archive's own length is a total the caller already has from
/// [`SdkState::Packed`].
pub fn extract(
    archive: &Path,
    root: &Path,
    progress: impl FnMut(u64),
) -> Result<PathBuf, String> {
    let final_dir = root.join("sdk");
    if final_dir.join("manifest.json").is_file() {
        return Ok(final_dir);
    }

    // Scratch sits beside the destination rather than in the system temp
    // directory: this is gigabytes, and `/tmp` is a ramdisk on plenty of Linux
    // installs. Next to the target is also guaranteed to be the same filesystem,
    // which is what makes the final rename atomic rather than a copy.
    let staging = root.join("sdk.partial");
    let _ = std::fs::remove_dir_all(&staging);

    if let Err(e) = unpack_stream(archive, &staging, progress) {
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

/// Decompress and untar in one pass, writing each file as it arrives.
///
/// The single pass is the whole reason for zstd. `lzma-rs` decodes a complete xz
/// stream into a `Write` and offers no `Read` adapter, so the previous version
/// had to land a ~1.9 GB tarball on disk and read it back — an extra write and
/// an extra read of the entire SDK, on top of a decoder that could not use more
/// than one core. `zstd::Decoder` implements `Read`, so it chains straight into
/// `tar::Archive` and the intermediate file stops existing.
fn unpack_stream(
    archive: &Path,
    dest: &Path,
    progress: impl FnMut(u64),
) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file =
        File::open(archive).map_err(|e| format!("could not open {}: {e}", archive.display()))?;
    // Progress is counted on the COMPRESSED side, before the decoder, because
    // that is the only place a byte count corresponds to a known total.
    let counted = Counting { inner: BufReader::with_capacity(1 << 20, file), read: 0, progress };
    let decoder = zstd::stream::Decoder::new(counted)
        .map_err(|e| format!("the SDK archive is corrupt or truncated: {e}"))?;
    tar::Archive::new(decoder)
        .unpack(dest)
        .map_err(|e| format!("could not unpack the SDK: {e}"))
}

/// A reader that reports how much has gone through it.
struct Counting<R, F> {
    inner: R,
    read: u64,
    progress: F,
}

impl<R: std::io::Read, F: FnMut(u64)> std::io::Read for Counting<R, F> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        (self.progress)(self.read);
        Ok(n)
    }
}
