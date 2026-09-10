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

use renzora_native_build::install;

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

/// The archive an extracted tree came from, written inside the tree itself.
///
/// See [`sdk_state`] for why it exists at all.
const STAMP: &str = ".archive-stamp";

/// Inspect `root` (the directory holding the executables) for an SDK.
///
/// Prefers an extracted tree, so a re-run after unpacking is a cheap stat rather
/// than a repeated extraction.
///
/// # Why the tree is checked against the archive rather than just found
///
/// On every platform but macOS the two are co-located: `sdk/` is unpacked beside
/// the executable and an update replaces that whole directory, so a tree that is
/// there at all is by construction the right one, and the archive is deleted
/// once it has been used.
///
/// A macOS install breaks that. The tree lives under Application Support (see
/// [`install::sdk_dir`]) while the archive stays inside the `.app`, so updating
/// the engine replaces the archive and leaves the old tree exactly where it was
/// — and every metadata filename in an SDK hashes the build configuration, so a
/// tree from the previous engine does not merely produce a stale plugin, it
/// fails to link one at all.
///
/// So an extracted tree records which archive produced it, and is believed only
/// while that still matches. A mismatch reports `Packed`, which is already the
/// "unpack before you can build" path the setup window knows how to run.
///
/// When there is no archive to compare against — a source checkout, or a
/// platform that deleted it after unpacking — the tree is taken at face value,
/// which is the pre-existing behaviour on all of them.
pub fn sdk_state(root: &Path) -> SdkState {
    let tree = install::sdk_dir(root);
    let archive = root.join("sdk.tar.zst");
    let packed = match std::fs::metadata(&archive) {
        Ok(m) if m.is_file() => Some(SdkState::Packed { archive: archive.clone(), bytes: m.len() }),
        _ => None,
    };

    if tree.join("manifest.json").is_file() {
        let stale = packed.is_some() && !stamp_matches(&tree, &archive);
        if !stale {
            return SdkState::Ready;
        }
    }
    packed.unwrap_or(SdkState::Absent)
}

/// Unpack `archive` into the tree [`install::sdk_dir`] names for `root`.
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
    let final_dir = install::sdk_dir(root);
    if final_dir.join("manifest.json").is_file() && stamp_matches(&final_dir, archive) {
        return Ok(final_dir);
    }

    // The destination is not always under `root` any more — on macOS it is in
    // Application Support — so make sure the tree above it exists before
    // anything tries to write there.
    let parent = final_dir
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", final_dir.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("could not create {}: {e}", parent.display()))?;

    // Scratch sits beside the DESTINATION rather than in the system temp
    // directory: this is gigabytes, and `/tmp` is a ramdisk on plenty of Linux
    // installs. Beside the target is also guaranteed to be the same filesystem,
    // which is what makes the final rename atomic rather than a copy — and that
    // is why it follows `final_dir` rather than staying under `root`.
    //
    // Suffixed with the process id because the destination is now shared: two
    // editors launching together resolve the same Application Support path,
    // where before they would each have been unpacking inside their own install
    // directory. They still race for the final rename, which is atomic, so the
    // loser simply does redundant work rather than corrupting the winner's tree.
    let staging = parent.join(format!("sdk.partial.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);

    if let Err(e) = unpack_stream(archive, &staging, progress) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    // The archive holds a top-level `sdk/`, so the staged tree is
    // `sdk.partial.<pid>/sdk/…`. Rename that inner directory, not its wrapper.
    let inner = staging.join("sdk");
    let src = if inner.is_dir() { inner } else { staging.clone() };

    // Stamped before the rename, so the tree that lands is already self-
    // describing. Writing it afterwards would leave a window where a tree is
    // complete but unattributed, which `sdk_state` would read as stale and
    // unpack all over again.
    if let Some(stamp) = fingerprint(archive) {
        let _ = std::fs::write(src.join(STAMP), stamp);
    }

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

/// Was `tree` unpacked from `archive`?
///
/// True when the stamp is absent, deliberately: a tree staged by
/// `cargo renzora` has never been through [`extract`] and has no stamp, and
/// refusing to use it would break every contributor's build to catch a case
/// that only exists in a shipped macOS install.
fn stamp_matches(tree: &Path, archive: &Path) -> bool {
    let Ok(recorded) = std::fs::read_to_string(tree.join(STAMP)) else {
        return true;
    };
    fingerprint(archive).is_some_and(|current| current == recorded.trim())
}

/// Identify an archive cheaply enough to check on every launch.
///
/// Length plus an FNV-1a hash of the first mebibyte. Length alone is a weak
/// discriminator — two engine builds differing in one crate compress to
/// similar sizes and could collide — and hashing 457 MB on every startup to
/// rule that out would cost more than the unpack it is trying to avoid. The
/// head of a zstd frame diverges as soon as any input byte does, so the pair
/// separates two builds for about a millisecond of I/O.
///
/// Hand-rolled rather than pulled from a crate for the same reason `xtask`'s
/// `build_id` is: nothing here needs to resist an adversary, only two different
/// SDKs accidentally agreeing.
fn fingerprint(archive: &Path) -> Option<String> {
    use std::io::Read;

    let mut file = File::open(archive).ok()?;
    let len = file.metadata().ok()?.len();

    let mut head = vec![0u8; 1 << 20];
    let mut filled = 0;
    while filled < head.len() {
        match file.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }

    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for b in len.to_le_bytes().iter().chain(&head[..filled]) {
        h ^= *b as u64;
        h = h.wrapping_mul(PRIME);
    }
    Some(format!("{len:x}-{h:016x}"))
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
    // `Decoder::new` parses the frame header eagerly, so a truncated archive
    // fails here rather than partway through the untar.
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
