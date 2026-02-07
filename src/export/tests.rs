//! Tests for the export module
//!
//! Covers ExportTarget metadata and BuildType configuration.

use super::build::{ExportTarget, BuildType};

// =============================================================================
// A. ExportTarget
// =============================================================================

#[test]
fn target_triple_windows() {
    assert_eq!(ExportTarget::Windows.target_triple(), "x86_64-pc-windows-msvc");
}

#[test]
fn target_triple_linux() {
    assert_eq!(ExportTarget::Linux.target_triple(), "x86_64-unknown-linux-gnu");
}

#[test]
fn target_triple_macos() {
    assert_eq!(ExportTarget::MacOS.target_triple(), "x86_64-apple-darwin");
}

#[test]
fn target_triple_macos_arm() {
    assert_eq!(ExportTarget::MacOSArm.target_triple(), "aarch64-apple-darwin");
}

#[test]
fn exe_extension_windows_has_exe() {
    assert_eq!(ExportTarget::Windows.exe_extension(), ".exe");
}

#[test]
fn exe_extension_unix_empty() {
    assert_eq!(ExportTarget::Linux.exe_extension(), "");
    assert_eq!(ExportTarget::MacOS.exe_extension(), "");
    assert_eq!(ExportTarget::MacOSArm.exe_extension(), "");
}

#[test]
fn display_name_non_empty() {
    let targets = [ExportTarget::Windows, ExportTarget::Linux, ExportTarget::MacOS, ExportTarget::MacOSArm];
    for t in &targets {
        assert!(!t.display_name().is_empty(), "{:?} should have a display name", t);
    }
}

#[test]
fn all_targets_unique_triples() {
    let targets = [ExportTarget::Windows, ExportTarget::Linux, ExportTarget::MacOS, ExportTarget::MacOSArm];
    let triples: Vec<_> = targets.iter().map(|t| t.target_triple()).collect();
    for i in 0..triples.len() {
        for j in (i + 1)..triples.len() {
            assert_ne!(triples[i], triples[j], "Duplicate target triple");
        }
    }
}

// =============================================================================
// B. BuildType
// =============================================================================

#[test]
fn debug_cargo_flag_empty() {
    assert_eq!(BuildType::Debug.cargo_flag(), "");
}

#[test]
fn release_cargo_flag() {
    assert_eq!(BuildType::Release.cargo_flag(), "--release");
}

#[test]
fn debug_profile_name() {
    assert_eq!(BuildType::Debug.profile_name(), "debug");
}

#[test]
fn release_profile_name() {
    assert_eq!(BuildType::Release.profile_name(), "release");
}
