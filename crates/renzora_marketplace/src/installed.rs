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
///
/// A nightly orders as the release it is a nightly *of*. `version::nightly_prefix`
/// builds these as `<release>-nightly-<ddmonyy>`, and the date on the end is the
/// problem: the last run of digits is the year, which made the whole tag parse as
/// `("r1-alpha8-nightly-16aug", 26)` — a prefix matching no release, so every
/// comparison fell through to "cannot tell" and a nightly satisfied *every*
/// floor, including ones it could not actually meet. Cutting the suffix first
/// means an alpha8 nightly clears an alpha8 floor because it genuinely is alpha8,
/// and an alpha6 nightly is correctly told to update.
pub fn release_order(tag: &str) -> Option<(String, u32)> {
    let tag = tag.trim();
    // Case-insensitive, and safe to index with: `to_ascii_lowercase` maps byte
    // for byte, so the offset it finds is the same offset in the original and
    // always lands on an ASCII boundary.
    let tag = match tag.to_ascii_lowercase().find("-nightly-") {
        Some(at) => &tag[..at],
        None => tag,
    };
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

/// A plugin version as numbers, for deciding which of two is newer.
///
/// Leading digit groups only, so `1.2.0-beta.1` reads as `1.2.0`. A creator's
/// version is their own scheme and this is deliberately a weak reading of it:
/// the one thing it has to get right is that `1.0.11` is newer than `1.0.10`,
/// where a string compare says the opposite. Anything it cannot parse yields an
/// empty key, which compares equal to every other empty one and so decides
/// nothing, leaving the caller on its string comparison.
///
/// Matches `semver_key` in the marketplace's migration 057, so the two sides
/// agree about which release is the newest.
fn version_key(v: &str) -> Vec<u32> {
    v.trim()
        .split('.')
        .map(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u32>().ok()
        })
        .take_while(Option::is_some)
        .flatten()
        .collect()
}

/// Decide what to offer for one installed plugin.
///
/// `resolved_version` is what the marketplace says THIS engine can run, and
/// `newest_version` is what exists for any engine. The engine no longer works
/// either out for itself: compatibility lives on the release, so only the
/// marketplace can see the whole line of them.
///
/// The gap between the two is the interesting case. Being current for your
/// engine while something newer exists is not "up to date", and it is not an
/// update either. It is "update the editor", and saying nothing would make a
/// maintained plugin look abandoned.
///
/// `newest_version` empty means a marketplace that predates per-release
/// compatibility. The floor comparison is kept for exactly that case, because
/// the engine and the website deploy separately and one can be old while the
/// other is new.
pub fn update_state(
    installed_version: &str,
    published: bool,
    resolved_version: &str,
    min_engine_version: &str,
    newest_version: &str,
    newest_requires: &str,
    engine_version: &str,
) -> UpdateState {
    if !published {
        return UpdateState::Unavailable;
    }

    // An older marketplace answers with the newest release and its floor, with
    // no per-engine resolution behind it, so the floor still has to be checked
    // here.
    if newest_version.trim().is_empty() {
        if resolved_version.trim() == installed_version.trim() {
            return UpdateState::UpToDate;
        }
        if !engine_satisfies(engine_version, min_engine_version) {
            return UpdateState::NeedsNewerEngine {
                version: resolved_version.to_string(),
                requires: min_engine_version.to_string(),
            };
        }
        return UpdateState::Available { version: resolved_version.to_string() };
    }

    let installed = installed_version.trim();
    let resolved = resolved_version.trim();

    // Never offer a move backwards as an update. Downgrading the editor after
    // installing a plugin leaves the resolved version genuinely older than what
    // is on disk, and "Update to 1.0.11" over an installed 2.0.0 is a worse
    // answer than saying nothing.
    let newer = |candidate: &str| -> bool {
        if candidate.is_empty() || candidate == installed {
            return false;
        }
        match (version_key(candidate), version_key(installed)) {
            (c, i) if c.is_empty() || i.is_empty() => true,
            (c, i) => c > i,
        }
    };

    if newer(resolved) {
        return UpdateState::Available { version: resolved.to_string() };
    }

    // Current for this engine, but not for every engine.
    let newest = newest_version.trim();
    if newest != resolved && newer(newest) {
        return UpdateState::NeedsNewerEngine {
            version: newest.to_string(),
            requires: newest_requires.to_string(),
        };
    }

    UpdateState::UpToDate
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The marketplace has resolved for this engine, so these only exercise
    /// what the engine still decides: newer, older, or nothing to say.
    fn state(installed: &str, resolved: &str, newest: &str, requires: &str) -> UpdateState {
        update_state(installed, true, resolved, "", newest, requires, "r1-alpha7")
    }

    #[test]
    fn a_newer_resolved_release_is_an_update() {
        assert!(matches!(
            state("1.0.2", "1.0.11", "1.0.11", ""),
            UpdateState::Available { version } if version == "1.0.11"
        ));
    }

    /// The case this whole per-release scheme exists for: current for your
    /// engine, with something newer sitting behind an engine upgrade. Neither
    /// "up to date" nor an update, and silence here makes a maintained plugin
    /// look abandoned.
    #[test]
    fn current_for_this_engine_but_not_for_every_engine() {
        assert!(matches!(
            state("1.0.11", "1.0.11", "2.0.0", "r1-alpha8"),
            UpdateState::NeedsNewerEngine { version, requires }
                if version == "2.0.0" && requires == "r1-alpha8"
        ));
    }

    #[test]
    fn nothing_newer_anywhere_is_up_to_date() {
        assert!(matches!(
            state("2.0.0", "2.0.0", "2.0.0", ""),
            UpdateState::UpToDate
        ));
    }

    /// Downgrading the editor after installing leaves the resolved release
    /// genuinely older than what is on disk. "Update to 1.0.11" over an
    /// installed 2.0.0 is worse than saying nothing.
    #[test]
    fn an_older_resolved_release_is_not_offered_as_an_update() {
        assert!(matches!(
            state("2.0.0", "1.0.11", "2.0.0", "r1-alpha8"),
            UpdateState::UpToDate
        ));
    }

    /// The ordering a string compare gets backwards, and the reason this needs
    /// a numeric key at all. Two digits against one is where it breaks: `1.0.11`
    /// against `1.0.10` happens to come out right either way, so it proves
    /// nothing.
    #[test]
    fn patch_versions_order_as_numbers() {
        assert!(version_key("1.0.10") > version_key("1.0.9"));
        assert!("1.0.10" < "1.0.9", "...which string ordering disagrees with");
        assert!(matches!(
            state("1.0.9", "1.0.10", "1.0.10", ""),
            UpdateState::Available { .. }
        ));
        assert!(matches!(
            state("1.0.10", "1.0.9", "1.0.10", ""),
            UpdateState::UpToDate
        ));
    }

    /// A version this cannot read is not a reason to refuse an update: the
    /// creator's scheme is theirs, and "different" is the honest reading.
    #[test]
    fn an_unparseable_version_falls_back_to_difference() {
        assert!(version_key("nightly").is_empty());
        assert!(matches!(
            state("nightly", "2024-06-01", "2024-06-01", ""),
            UpdateState::Available { .. }
        ));
    }

    /// A marketplace that predates per-release compatibility sends no
    /// `latest_version`, and the floor comparison has to carry the decision.
    /// The engine and the website deploy separately, so this is a real state.
    #[test]
    fn an_older_marketplace_still_gets_the_floor_checked() {
        assert!(matches!(
            update_state("1.0.0", true, "2.0.0", "r1-alpha8", "", "", "r1-alpha7"),
            UpdateState::NeedsNewerEngine { .. }
        ));
        assert!(matches!(
            update_state("1.0.0", true, "2.0.0", "r1-alpha7", "", "", "r1-alpha7"),
            UpdateState::Available { .. }
        ));
    }

    #[test]
    fn an_unpublished_listing_is_left_alone() {
        assert!(matches!(
            update_state("1.0.0", false, "2.0.0", "", "2.0.0", "", "r1-alpha7"),
            UpdateState::Unavailable
        ));
    }

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

    /// A nightly is a build of a release, and orders as that release. Before
    /// the suffix was stripped, the trailing date parsed as the version number
    /// and every one of these was "cannot tell" — so a nightly cleared every
    /// floor ever set, including the last case here.
    #[test]
    fn a_nightly_orders_as_the_release_it_is() {
        assert_eq!(
            release_order("r1-alpha8-nightly-16aug26"),
            Some(("r1-alpha".into(), 8)),
            "the trailing date must not be read as the release number"
        );

        // A dev on a nightly gets what that release gets.
        assert!(engine_satisfies("r1-alpha8-nightly-16aug26", "r1-alpha8"));
        assert!(engine_satisfies("r1-alpha8-nightly-16aug26", "r1-alpha7"));

        // And is held to the same floor as that release, rather than sailing
        // past it and failing later at build or load time.
        assert!(!engine_satisfies("r1-alpha6-nightly-01jan26", "r1-alpha8"));
    }

    /// A floor someone wrote as a nightly still names a release.
    #[test]
    fn a_nightly_floor_reads_as_its_release() {
        assert!(engine_satisfies("r1-alpha8", "r1-alpha8-nightly-16aug26"));
        assert!(!engine_satisfies("r1-alpha7", "r1-alpha8-nightly-16aug26"));
    }

    /// Nothing but a suffix is still nothing to compare.
    #[test]
    fn a_bare_nightly_suffix_is_uncomparable() {
        assert_eq!(release_order("-nightly-16aug26"), None);
        assert!(engine_satisfies("r1-alpha7", "-nightly-16aug26"));
    }

}
