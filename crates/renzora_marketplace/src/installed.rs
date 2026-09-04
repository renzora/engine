//! What is already installed, and what should replace it.
//!
//! # Why the directory name is not simply the crate name
//!
//! Two different marketplace listings can ship a crate called `vignette`.
//! Installing the second under that name would overwrite the first — same
//! directory, same artefact path — and the user would have paid for a plugin
//! that silently replaced another.
//!
//! Identity is therefore the **asset id**, recorded in the `plugin.toml`
//! sidecar written beside the source. The directory name stays derived from the
//! crate name, because `renzora_plugin_build::crate_name` reads it back off the
//! directory and `layout` derives the artefact path from it — but it is
//! disambiguated (`vignette_2`) when a *different* asset already holds the
//! name. That also keeps the two crates' symbols apart, since the directory
//! name is what reaches `rustc --crate-name`.
//!
//! Reinstalling the *same* asset is an update, not a clash: it keeps its
//! directory, so the artefact and stamp it already has stay meaningful.

use std::path::{Path, PathBuf};

use crate::install::PluginSidecar;

/// One installed plugin, as read back from its sidecar.
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    /// Directory under `plugins/`, which is also the crate name it builds as.
    pub dir_name: String,
    pub path: PathBuf,
    pub asset_id: String,
    pub name: String,
    pub slug: String,
    /// The version recorded at install time.
    pub version: String,
}

/// Every marketplace-installed plugin found beside the editor.
///
/// Only ones with a sidecar are listed: a plugin the user built into `plugins/`
/// themselves is not ours to track, update, or count as a clash.
#[cfg(not(target_arch = "wasm32"))]
pub fn scan() -> Vec<InstalledPlugin> {
    let Ok(dir) = crate::install::engine_plugins_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for path in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let Some(meta) = read_sidecar(&path) else { continue };
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        out.push(InstalledPlugin {
            dir_name,
            path,
            asset_id: meta.asset_id,
            name: meta.name,
            slug: meta.slug,
            version: meta.version,
        });
    }
    out.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    out
}

/// Read a directory's `plugin.toml`, if it has a usable one.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_sidecar(dir: &Path) -> Option<PluginSidecar> {
    let text = std::fs::read_to_string(dir.join("plugin.toml")).ok()?;
    let meta: PluginSidecar = toml::from_str(&text).ok()?;
    (!meta.asset_id.is_empty()).then_some(meta)
}

/// The installed entry for `asset_id`, if that asset is installed.
#[cfg(not(target_arch = "wasm32"))]
pub fn find_by_asset(asset_id: &str) -> Option<InstalledPlugin> {
    scan().into_iter().find(|p| p.asset_id == asset_id)
}

/// Where a plugin from `asset_id` whose crate is called `crate_name` should be
/// installed.
///
/// * the same asset again → its existing directory, so this is an update
/// * a free name → that name
/// * taken by a *different* asset → the first free `name_2`, `name_3`, …
///
/// Returns the directory name and whether it replaces an existing install of
/// the same asset, which is the difference between "Installed" and "Updated" in
/// the message the user reads.
#[cfg(not(target_arch = "wasm32"))]
pub fn destination_for(asset_id: &str, crate_name: &str) -> (String, bool) {
    let installed = scan();
    if let Some(existing) = installed.iter().find(|p| p.asset_id == asset_id) {
        return (existing.dir_name.clone(), true);
    }

    let taken = |name: &str| -> bool {
        // Taken by anything on disk, sidecar or not: a directory that is not
        // ours is still a directory we must not write over.
        crate::install::engine_plugins_dir()
            .map(|d| d.join(name).exists())
            .unwrap_or(false)
    };

    if !taken(crate_name) {
        return (crate_name.to_string(), false);
    }
    // `_2` rather than `-2`: the directory name becomes a crate name, and a
    // hyphen there would be rewritten to an underscore anyway.
    for n in 2..100u32 {
        let candidate = format!("{crate_name}_{n}");
        if !taken(&candidate) {
            return (candidate, false);
        }
    }
    (format!("{crate_name}_{}", &asset_id.replace('-', "")[..8.min(asset_id.len())]), false)
}

// ── Updates ─────────────────────────────────────────────────────────────────

/// Why an installed plugin is, or is not, offered an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    /// Nothing newer published.
    UpToDate,
    /// A newer version is published and this engine can run it.
    Available { version: String },
    /// A newer version exists but needs an engine this old install cannot
    /// satisfy. Shown, not hidden: "update the editor" is the actionable
    /// answer, and silence would look like no update at all.
    NeedsNewerEngine { version: String, requires: String },
    /// The listing is gone or unpublished — keep what is installed.
    Unavailable,
}

/// Compare a release tag like `r1-alpha7` against another.
///
/// Ordering is by the numeric suffix within a matching prefix, so `r1-alpha10`
/// sorts after `r1-alpha7` — which a plain string compare gets wrong. An
/// unparseable tag returns `None`, and callers treat that as "cannot tell"
/// rather than as a failure to satisfy.
pub fn release_order(tag: &str) -> Option<(String, u32)> {
    let tag = tag.trim();
    if tag.is_empty() {
        return None;
    }
    // Split at the last run of digits: "r1-alpha7" -> ("r1-alpha", 7).
    let digits_start = tag
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .map(|(i, _)| i)
        .last()?;
    let (prefix, num) = tag.split_at(digits_start);
    num.parse::<u32>().ok().map(|n| (prefix.to_string(), n))
}

/// Does `engine` satisfy a `requires` floor?
///
/// True when the floor is empty (no constraint), when the two cannot be
/// compared (never block on a tag we do not understand), or when the engine is
/// at or past the floor.
pub fn engine_satisfies(engine: &str, requires: &str) -> bool {
    if requires.trim().is_empty() {
        return true;
    }
    match (release_order(engine), release_order(requires)) {
        (Some((ep, en)), Some((rp, rn))) if ep == rp => en >= rn,
        _ => true,
    }
}

/// Decide what to offer for one installed plugin.
///
/// Versions are compared as strings for equality only: a creator's version is
/// their own scheme, and "different from what is installed" is the honest
/// reading of it. Ordering them would mean guessing at semver they never
/// promised.
pub fn update_state(
    installed_version: &str,
    published: bool,
    latest_version: &str,
    min_engine_version: &str,
    engine_version: &str,
) -> UpdateState {
    if !published {
        return UpdateState::Unavailable;
    }
    if latest_version.trim() == installed_version.trim() {
        return UpdateState::UpToDate;
    }
    if !engine_satisfies(engine_version, min_engine_version) {
        return UpdateState::NeedsNewerEngine {
            version: latest_version.to_string(),
            requires: min_engine_version.to_string(),
        };
    }
    UpdateState::Available { version: latest_version.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tags_order_numerically_not_lexically() {
        assert_eq!(release_order("r1-alpha7"), Some(("r1-alpha".into(), 7)));
        // The case a string compare gets wrong.
        let a = release_order("r1-alpha10").unwrap();
        let b = release_order("r1-alpha7").unwrap();
        assert!(a.1 > b.1, "alpha10 is newer than alpha7");
        assert!("r1-alpha10" < "r1-alpha7", "…which string ordering disagrees with");
    }

    #[test]
    fn an_empty_floor_is_no_constraint() {
        assert!(engine_satisfies("r1-alpha7", ""));
        assert!(engine_satisfies("r1-alpha7", "   "));
    }

    #[test]
    fn the_engine_must_reach_the_floor() {
        assert!(engine_satisfies("r1-alpha7", "r1-alpha7"));
        assert!(engine_satisfies("r1-alpha8", "r1-alpha7"));
        assert!(!engine_satisfies("r1-alpha6", "r1-alpha7"));
    }

    /// Never block on something unparseable — a plugin that names its floor in
    /// a scheme we do not know is still installable.
    #[test]
    fn an_uncomparable_floor_does_not_block() {
        assert!(engine_satisfies("r1-alpha7", "whatever-2"));
        assert!(engine_satisfies("r1-alpha7", "2.0"));
    }

    #[test]
    fn a_matching_version_is_up_to_date() {
        assert_eq!(
            update_state("1.0.0", true, "1.0.0", "", "r1-alpha7"),
            UpdateState::UpToDate
        );
    }

    #[test]
    fn a_different_version_is_an_update() {
        assert_eq!(
            update_state("1.0.0", true, "1.1.0", "", "r1-alpha7"),
            UpdateState::Available { version: "1.1.0".into() }
        );
    }

    /// The update exists but this editor is too old — say so rather than
    /// showing nothing, which reads as "no update".
    #[test]
    fn an_update_needing_a_newer_engine_says_so() {
        assert_eq!(
            update_state("1.0.0", true, "2.0.0", "r1-alpha9", "r1-alpha7"),
            UpdateState::NeedsNewerEngine { version: "2.0.0".into(), requires: "r1-alpha9".into() }
        );
    }

    #[test]
    fn an_unpublished_listing_offers_nothing() {
        assert_eq!(
            update_state("1.0.0", false, "2.0.0", "", "r1-alpha7"),
            UpdateState::Unavailable
        );
    }
}
