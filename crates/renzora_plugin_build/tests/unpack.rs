//! Unpacking a shipped `sdk.tar.zst`.
//!
//! Builds a miniature SDK archive in the real format, so the round trip covers
//! what a release actually produces rather than a hand-rolled stand-in.
//!
//! # Why the fixture is built in Rust rather than by shelling out
//!
//! It used to run `tar -cf - sdk | xz …` through `sh`, and skip itself wherever
//! those tools were missing — which on a Windows developer's machine is most of
//! the time, so the test quietly did not run for the people most likely to break
//! it. `tar` and `zstd` are both ordinary dependencies of this crate now, so the
//! same archive can be written directly with no external commands and no skip.

use std::path::{Path, PathBuf};

use renzora_plugin_build::unpack::{extract, sdk_state, SdkState};

/// A `<root>/sdk.tar.zst` holding a plausible SDK, made the way a release is.
fn build_release_layout(root: &Path) -> bool {
    let sdk = root.join("sdk");
    std::fs::create_dir_all(sdk.join("deps")).unwrap();
    std::fs::write(sdk.join("manifest.json"), r#"{"triple":"test"}"#).unwrap();
    // Big enough that unpacking reports progress more than once. Incompressible
    // on purpose — a run of one byte would compress to almost nothing, and the
    // reader would then deliver it in a single chunk with no progress to show.
    let filler: Vec<u8> = (0..512 * 1024).map(|i| (i * 2654435761u64 >> 13) as u8).collect();
    std::fs::write(sdk.join("deps").join("libfake.rlib"), filler).unwrap();

    let archive = std::fs::File::create(root.join("sdk.tar.zst")).unwrap();
    // Level 1: the fixture is throwaway and its size is irrelevant, where the
    // time is paid on every test run.
    let mut encoder = zstd::stream::Encoder::new(archive, 1).unwrap();
    {
        let mut builder = tar::Builder::new(&mut encoder);
        // `sdk` as the archive root, matching `package-release.sh` — `extract`
        // renames that inner directory into place, so a flat archive would be
        // unpacked to the wrong shape.
        builder.append_dir_all("sdk", &sdk).unwrap();
        builder.finish().unwrap();
    }
    encoder.finish().unwrap();

    std::fs::remove_dir_all(&sdk).unwrap();
    true
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// How long unpacking a REAL SDK actually takes, on the code path the editor
/// runs.
///
/// Worth being able to measure rather than reason about — the guesses that led
/// here were wrong twice. This measurement is what decided the archive format:
/// under xz it read **29.8 s**, and under zstd **4.4 s** for the same tree, which
/// is why the SDK ships as `.zst`. See `unpack.rs` for the rest of that
/// comparison.
///
/// Note what it does NOT measure: decode speed alone. Most of the remaining time
/// is writing ~1.9 GB of files, so this number is disk-bound and will differ on
/// slower storage. That is the honest figure for "how long does the setup window
/// stay up", which is the only reason to care.
///
/// Skipped unless `RENZORA_SDK_ARCHIVE` points at an `sdk.tar.zst`, because the
/// archive is ~444 MB and cannot live in the repo. Run it as:
///
/// ```text
/// RENZORA_SDK_ARCHIVE=dist/windows-x64/sdk.tar.zst \
///   cargo test --profile dist -p renzora_plugin_build --test unpack -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a real SDK archive via RENZORA_SDK_ARCHIVE"]
fn how_long_a_real_sdk_takes_to_unpack() {
    let Ok(archive) = std::env::var("RENZORA_SDK_ARCHIVE") else {
        eprintln!("RENZORA_SDK_ARCHIVE unset — skipping");
        return;
    };
    let archive = PathBuf::from(archive);
    let compressed = std::fs::metadata(&archive).expect("archive exists").len();
    let root = tmp("renzora_unpack_timing");
    std::fs::copy(&archive, root.join("sdk.tar.zst")).expect("stage archive");

    let start = std::time::Instant::now();
    let mut last = 0u64;
    let dir = extract(&root.join("sdk.tar.zst"), &root, |done| last = done).expect("extract");
    let secs = start.elapsed().as_secs_f64();

    let extracted: u64 = walk_size(&dir);
    eprintln!(
        "unpacked {:.0} MB -> {:.0} MB in {secs:.1}s ({:.0} MB/s of output)",
        compressed as f64 / 1e6,
        extracted as f64 / 1e6,
        extracted as f64 / 1e6 / secs,
    );
    assert!(last > 0, "progress was reported");
    let _ = std::fs::remove_dir_all(&root);
}

/// Total bytes of every file under `dir`.
fn walk_size(dir: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    for e in entries.flatten() {
        let p = e.path();
        total += if p.is_dir() { walk_size(&p) } else { e.metadata().map(|m| m.len()).unwrap_or(0) };
    }
    total
}

#[test]
fn a_shipped_archive_unpacks_into_a_usable_sdk() {
    let root = tmp("renzora_unpack_ok");
    if !build_release_layout(&root) {
        return;
    }

    match sdk_state(&root) {
        SdkState::Packed { bytes, .. } => assert!(bytes > 0, "archive should have size"),
        other => panic!("a shipped-but-unextracted SDK should read as Packed, got {other:?}"),
    }

    let mut ticks = 0;
    let dir = extract(&root.join("sdk.tar.zst"), &root, |_| ticks += 1).expect("extract");

    assert!(dir.join("manifest.json").is_file(), "manifest landed");
    assert!(dir.join("deps").join("libfake.rlib").is_file(), "contents landed");
    assert!(ticks > 0, "progress should be reported for a UI to show");
    assert!(matches!(sdk_state(&root), SdkState::Ready), "now reads as Ready");

    // Scratch paths must not survive — they sit beside the destination, so
    // leaving them behind means gigabytes of litter in the install directory.
    assert!(!root.join("sdk.partial").exists(), "staging dir cleaned up");
    // No intermediate tarball at any point. This is the streaming property, not
    // just cleanup: the decoder feeds the tar reader directly, and reintroducing
    // a temporary file would silently double the disk written on every install.
    assert!(
        !root.join("sdk.tar.partial").exists(),
        "unpacking must stream, never land an intermediate tarball"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The case that would otherwise corrupt an install.
///
/// `Sdk::load` decides an SDK exists by finding `manifest.json`, which is small
/// and lands early. A truncated archive must therefore leave NO `sdk/` at all —
/// not a partial one that loads and then fails at compile time with a
/// missing-crate error pointing at nothing.
#[test]
fn a_truncated_archive_leaves_no_half_sdk() {
    let root = tmp("renzora_unpack_truncated");
    if !build_release_layout(&root) {
        return;
    }

    let archive = root.join("sdk.tar.zst");
    let whole = std::fs::read(&archive).unwrap();
    std::fs::write(&archive, &whole[..whole.len() / 2]).unwrap();

    let err = extract(&archive, &root, |_| {}).expect_err("a truncated archive must fail");
    assert!(
        err.contains("corrupt") || err.contains("truncated"),
        "the error should say the archive is bad: {err}"
    );

    assert!(!root.join("sdk").exists(), "no SDK directory may be left behind");
    assert!(matches!(sdk_state(&root), SdkState::Packed { .. }), "still just an archive");
    assert!(!root.join("sdk.partial").exists(), "staging dir cleaned up on failure");
    assert!(
        !root.join("sdk.tar.partial").exists(),
        "a failed unpack must leave no intermediate tarball either"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn extracting_twice_is_a_no_op() {
    let root = tmp("renzora_unpack_twice");
    if !build_release_layout(&root) {
        return;
    }
    let archive = root.join("sdk.tar.zst");
    extract(&archive, &root, |_| {}).expect("first");
    let mut ticks = 0;
    extract(&archive, &root, |_| ticks += 1).expect("second");
    assert_eq!(ticks, 0, "an already-extracted SDK must not be unpacked again");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_build_with_no_sdk_at_all_reads_as_absent() {
    let root = tmp("renzora_unpack_absent");
    assert!(matches!(sdk_state(&root), SdkState::Absent));
    let _ = std::fs::remove_dir_all(&root);
}
