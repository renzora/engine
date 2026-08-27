//! Unpacking a shipped `sdk.tar.xz`.
//!
//! Builds a miniature SDK archive with the same tools the release script uses
//! (`tar` + `xz`), so the round trip covers the real format rather than a
//! hand-rolled stand-in. Skipped where those tools are absent.

use std::path::{Path, PathBuf};
use std::process::Command;

use renzora_plugin_build::unpack::{extract, sdk_state, SdkState};

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").output().is_ok_and(|o| o.status.success())
}

/// A `<root>/sdk.tar.xz` holding a plausible SDK, made the way a release is.
fn build_release_layout(root: &Path) -> bool {
    if !have("tar") || !have("xz") {
        eprintln!("tar/xz unavailable — skipping");
        return false;
    }
    let sdk = root.join("sdk");
    std::fs::create_dir_all(sdk.join("deps")).unwrap();
    std::fs::write(sdk.join("manifest.json"), r#"{"triple":"test"}"#).unwrap();
    // Big enough that decompression reports progress more than once.
    std::fs::write(sdk.join("deps").join("libfake.rlib"), vec![7u8; 512 * 1024]).unwrap();

    let ok = Command::new("sh")
        .arg("-c")
        .arg("tar -cf - sdk | xz -0 -T0 > sdk.tar.xz")
        .current_dir(root)
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "could not build the test archive");
    std::fs::remove_dir_all(&sdk).unwrap();
    true
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
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
    let dir = extract(&root.join("sdk.tar.xz"), &root, |_| ticks += 1).expect("extract");

    assert!(dir.join("manifest.json").is_file(), "manifest landed");
    assert!(dir.join("deps").join("libfake.rlib").is_file(), "contents landed");
    assert!(ticks > 0, "progress should be reported for a UI to show");
    assert!(matches!(sdk_state(&root), SdkState::Ready), "now reads as Ready");

    // Scratch paths must not survive — they sit beside the destination, so
    // leaving them behind means gigabytes of litter in the install directory.
    assert!(!root.join("sdk.partial").exists(), "staging dir cleaned up");
    assert!(!root.join("sdk.tar.partial").exists(), "temp tarball cleaned up");

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

    let archive = root.join("sdk.tar.xz");
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
    assert!(!root.join("sdk.tar.partial").exists(), "temp tarball cleaned up on failure");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn extracting_twice_is_a_no_op() {
    let root = tmp("renzora_unpack_twice");
    if !build_release_layout(&root) {
        return;
    }
    let archive = root.join("sdk.tar.xz");
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
