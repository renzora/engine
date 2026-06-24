//! The Renzora plugin ABI hash, computed *explicitly* from its only three
//! semantic inputs — **bevy version**, **rust toolchain**, and the **curated
//! bevy feature set**.
//!
//! ## Why this exists
//!
//! Distribution plugins are `dlopen`'d and share one compiled `bevy_dylib` with
//! the host. The loader must reject a plugin built against an *incompatible*
//! bevy before it touches the `App`. The old guard derived that compatibility
//! tag implicitly from `TypeId::of::<bevy::ecs::world::World>()`. A Rust
//! `TypeId` folds in the crate's `-Cmetadata` / Strict-Version-Hash, which
//! captures far more than the bevy ABI: the dependency *source* (git rev vs
//! crates.io registry), unrelated `Cargo.lock` churn, the build profile,
//! `RUSTFLAGS`, and the target triple. So the tag drifted on changes that left
//! the actual bevy ABI byte-identical — every drift forcing a re-pin and a
//! rebuild of every distribution plugin for no real reason.
//!
//! This crate replaces that implicit tag with an explicit one: hash exactly the
//! three things that genuinely change the bevy a plugin links against, and
//! nothing else. Two builds with the same bevy version, the same rustc, and the
//! same bevy feature set produce the *same* hash regardless of dependency
//! source, lockfile noise, profile, flags, or target — killing the spurious
//! drift while still moving the moment a real ABI input changes.
//!
//! ## The feature input is *only* bevy's
//!
//! A plugin just needs to be layout-compatible with the shared `bevy_dylib`, so
//! the only features that matter are bevy's own — the curated
//! `[workspace.dependencies] bevy.features` list, which is the single source of
//! truth for "the bevy surface every plugin can reach." A plugin enabling its
//! *own* unrelated cargo features does not change bevy's compilation and must
//! not change the ABI hash, so those are intentionally ignored here.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The three (and only three) semantic inputs to the plugin ABI hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiInputs {
    /// Locked bevy version, e.g. `0.19.3`. Read from `Cargo.lock` so it is the
    /// exact resolved version, not the `^0.19` requirement.
    pub bevy_version: String,
    /// Rust toolchain identity: `release` + `commit-hash`, e.g.
    /// `1.95.0 9b00956e5`. Deliberately *excludes* the host triple so the hash
    /// is target-independent — a plugin is only ever `dlopen`'d by a host of its
    /// own platform anyway (the file extension enforces that), so folding the
    /// triple in would only add spurious per-platform divergence.
    pub rustc: String,
    /// The curated bevy feature set, sorted + de-duplicated. From the workspace
    /// `[workspace.dependencies] bevy.features` list.
    pub features: Vec<String>,
}

impl AbiInputs {
    /// The canonical, stable byte representation that gets hashed. Stable across
    /// runs and machines: fixed field order, features sorted. Also exactly what
    /// the CLI prints and what gets embedded for runtime diagnostics, so a
    /// rejected plugin can be diffed field-by-field against the host.
    pub fn canonical(&self) -> String {
        format!(
            "bevy {}\nrustc {}\nfeatures [{}]\n",
            self.bevy_version,
            self.rustc,
            self.features.join(",")
        )
    }

    /// The 128-bit ABI hash as `[u64; 2]` (the shape the FFI `plugin_bevy_hash`
    /// returns and the loader compares).
    pub fn hash(&self) -> [u64; 2] {
        let bytes = self.canonical();
        let bytes = bytes.as_bytes();
        // Two FNV-1a 64 lanes with distinct offset bases. FNV-1a is chosen over
        // `std::hash::DefaultHasher` precisely because it is a *fixed* algorithm
        // — its output depends only on the bytes, never on the Rust version that
        // compiled this code. (rustc is already an input we fold in deliberately;
        // we don't want a second, accidental dependence on it via the hasher.)
        [fnv1a64(0xcbf2_9ce4_8422_2325, bytes), fnv1a64(0x8422_2325_cbf2_9ce4, bytes)]
    }

    /// The ABI hash formatted as a 32-char lowercase hex string (the form stored
    /// in `abi.lock` and shown to users).
    pub fn hash_hex(&self) -> String {
        let h = self.hash();
        format!("{:016x}{:016x}", h[0], h[1])
    }
}

fn fnv1a64(offset_basis: u64, bytes: &[u8]) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = offset_basis;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Walk up from `start` until a directory holding both `Cargo.lock` and a
/// `[workspace]`-bearing `Cargo.toml` is found — the engine workspace root.
///
/// `build.rs` calls this from `CARGO_MANIFEST_DIR` (`crates/renzora`); the CLI
/// calls it from the current dir. Distribution plugins are scaffolded *inside*
/// this workspace (`renzora add`), so the root — and thus the same `Cargo.lock`
/// and feature list — is always reachable for both the host and every plugin.
pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let lock = dir.join("Cargo.lock");
        let manifest = dir.join("Cargo.toml");
        if lock.is_file() && manifest.is_file() {
            if let Ok(text) = std::fs::read_to_string(&manifest) {
                if text.contains("[workspace]") {
                    return Some(dir.to_path_buf());
                }
            }
        }
    }
    None
}

/// Gather the three ABI inputs for the workspace rooted at `root`.
pub fn gather(root: &Path) -> Result<AbiInputs, String> {
    Ok(AbiInputs {
        bevy_version: bevy_version_from_lock(&root.join("Cargo.lock"))?,
        rustc: rustc_identity()?,
        features: bevy_features_from_manifest(&root.join("Cargo.toml"))?,
    })
}

/// Read the resolved `bevy` version out of `Cargo.lock`.
fn bevy_version_from_lock(lock_path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(lock_path)
        .map_err(|e| format!("reading {}: {e}", lock_path.display()))?;
    let doc: toml::Value =
        toml::from_str(&text).map_err(|e| format!("parsing {}: {e}", lock_path.display()))?;
    let packages = doc
        .get("package")
        .and_then(|p| p.as_array())
        .ok_or("Cargo.lock has no [[package]] entries")?;
    for pkg in packages {
        if pkg.get("name").and_then(|n| n.as_str()) == Some("bevy") {
            return pkg
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .ok_or_else(|| "bevy package in Cargo.lock has no version".to_string());
        }
    }
    Err("no `bevy` package found in Cargo.lock".to_string())
}

/// Read the curated bevy feature list from `[workspace.dependencies] bevy`.
/// Sorted + de-duplicated so reordering the list never moves the hash.
fn bevy_features_from_manifest(manifest_path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("reading {}: {e}", manifest_path.display()))?;
    let doc: toml::Value =
        toml::from_str(&text).map_err(|e| format!("parsing {}: {e}", manifest_path.display()))?;
    let bevy = doc
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.get("bevy"))
        .ok_or("Cargo.toml has no [workspace.dependencies] bevy")?;
    let mut features: Vec<String> = bevy
        .get("features")
        .and_then(|f| f.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    features.sort();
    features.dedup();
    Ok(features)
}

/// `release` + `commit-hash` from `rustc --version --verbose`. Uses the `RUSTC`
/// env var when set (cargo points it at the active toolchain) and falls back to
/// `rustc` on PATH.
fn rustc_identity() -> Result<String, String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let out = Command::new(&rustc)
        .args(["--version", "--verbose"])
        .output()
        .map_err(|e| format!("running `{rustc} --version --verbose`: {e}"))?;
    if !out.status.success() {
        return Err(format!("`{rustc} --version --verbose` exited with {}", out.status));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut release = None;
    let mut commit = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("release:") {
            release = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("commit-hash:") {
            commit = Some(v.trim().to_string());
        }
    }
    let release = release.ok_or("no `release:` line in rustc --version --verbose")?;
    // `commit-hash` is absent on some distro-built toolchains; the release alone
    // still identifies the ABI-relevant toolchain there.
    match commit {
        Some(c) if c != "unknown" => Ok(format!("{release} {c}")),
        _ => Ok(release),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(feat: &[&str]) -> AbiInputs {
        AbiInputs {
            bevy_version: "0.19.0".into(),
            rustc: "1.95.0 deadbeef".into(),
            features: feat.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn hash_is_deterministic_and_32_hex() {
        let h = inputs(&["a", "b"]).hash_hex();
        assert_eq!(h.len(), 32);
        assert_eq!(h, inputs(&["a", "b"]).hash_hex());
        assert!(h.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn feature_change_moves_the_hash() {
        assert_ne!(inputs(&["x"]).hash(), inputs(&["x", "y"]).hash());
    }

    #[test]
    fn feature_order_does_not_matter_via_sort() {
        // gather() sorts features; equal sets hash equally regardless of input order.
        let mut a = inputs(&["b", "a"]);
        a.features.sort();
        let mut b = inputs(&["a", "b"]);
        b.features.sort();
        assert_eq!(a.hash(), b.hash());
    }
}
