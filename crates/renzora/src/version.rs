//! The engine's identity — one version string, one release-tag resolver.
//!
//! This used to be four independent literals: `renzora_shell::about` said
//! `r1-alpha7`, `renzora_splash` still said `r1-alpha6`, the command palette
//! hardcoded `r1-alpha6` as its docs fallback, and `build.rs` exported the crate's
//! semver (`0.2.0`) as `RENZORA_ENGINE_VERSION`. Nothing agreed, and nothing could
//! answer the one question the export downloader actually needs — *which release
//! tag matches the binary I am running?* — so it asked for `releases/latest` and
//! got whatever was newest, regardless of ABI.
//!
//! Everything user-facing reads [`ENGINE_VERSION`] from here. The semver in
//! `Cargo.toml` is left alone: it is the crate's version for cargo's benefit, not
//! the engine's version for a human's.
//!
//! # Channels
//!
//! A binary is one of three things, distinguished by two env vars CI stamps in at
//! compile time:
//!
//! * **Dev** — neither var set. Built from a checkout; has no release of its own.
//! * **Nightly** — `RENZORA_RELEASE_TAG=r1-alpha7-nightly-16aug26`. One release per
//!   night, tagged `<version>-nightly-<ddmonyy>`, published as a prerelease.
//! * **Release** — `RENZORA_RELEASE_TAG=r1-alpha7`, i.e. the tag *is* the version.
//!
//! `option_env!` bakes these in when the `renzora` crate compiles, so they are
//! only picked up by a cold build. CI builds cold every run; a warm local tree
//! will not notice the vars changing, which is fine because a local tree is always
//! Dev anyway.

/// The engine version, in the `r1-alphaN` scheme used for docs directories,
/// release tags and everything shown to a user. **Bump this and `docs/` together**
/// (see CLAUDE.md §4) — it is what the export downloader asks GitHub for.
pub const ENGINE_VERSION: &str = "r1-alpha7";

/// Which kind of build this is. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildChannel {
    /// Built from a source checkout — no release of its own.
    Dev,
    /// A dated prerelease built by the nightly schedule.
    Nightly,
    /// A tagged release build.
    Release,
}

/// The exact GitHub release tag this binary was published under, or `None` for a
/// build from source.
///
/// A published binary knows its own tag exactly and never has to guess; only a
/// dev build has to *resolve* one (see [`fallback_tag_prefix`]).
pub fn release_tag() -> Option<&'static str> {
    option_env!("RENZORA_RELEASE_TAG").filter(|t| !t.is_empty())
}

/// The commit this binary was built from, when CI stamped it in.
pub fn build_commit() -> Option<&'static str> {
    option_env!("RENZORA_BUILD_COMMIT").filter(|c| !c.is_empty())
}

/// Classify this build. A tag equal to [`ENGINE_VERSION`] is a release; any other
/// tag is a nightly (they are all `<version>-nightly-<date>`); no tag is dev.
pub fn channel() -> BuildChannel {
    match release_tag() {
        Some(t) if t == ENGINE_VERSION => BuildChannel::Release,
        Some(_) => BuildChannel::Nightly,
        None => BuildChannel::Dev,
    }
}

/// Tag prefix a dev build falls back to when no release exists for its exact
/// version — i.e. `"r1-alpha7-nightly-"`. The newest release whose tag starts with
/// this is the nightly that matches an in-development editor.
///
/// Deliberately NOT "latest stable": an `r1-alpha7` editor paired with `r1-alpha6`
/// runtime templates is an ABI mismatch waiting to surface as a scene that loads
/// in the editor and not in the export.
pub fn fallback_tag_prefix() -> String {
    format!("{ENGINE_VERSION}-nightly-")
}

/// The platform key this binary was built for — `windows-x64`, `linux-arm64`,
/// `macos-arm64`, … — or `None` on a target the engine does not publish builds
/// for.
///
/// This one string names three things that have to agree: the `dist/<key>/`
/// directory a build stages into, the `<key>.zip` engine asset on a release, and
/// the `renzora-runtime-<key>.zip` export template beside it. It lives in the
/// contract crate because both the updater (which wants the engine asset for the
/// host) and the exporter (which wants a template for some *other* platform)
/// need it, and a second copy of this table is exactly how the download feature
/// came to ask for an asset name nothing published.
pub fn host_platform_key() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Some("windows-x64");
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        return Some("windows-arm64");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Some("linux-x64");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Some("linux-arm64");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Some("macos-x64");
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Some("macos-arm64");
    }
    #[allow(unreachable_code)]
    None
}

/// Version string for display — `r1-alpha7`, `r1-alpha7-nightly-16aug26`, or
/// `r1-alpha7 (dev)`.
pub fn display() -> String {
    match release_tag() {
        Some(t) => t.to_string(),
        None => format!("{ENGINE_VERSION} (dev)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_follows_the_docs_scheme() {
        // The downloader builds release tags out of this and `docs/<version>/` is
        // named after it, so a stray `v` prefix or a semver here breaks both.
        assert!(ENGINE_VERSION.starts_with("r1-alpha"));
        assert!(!ENGINE_VERSION.contains(char::is_whitespace));
    }

    #[test]
    fn fallback_prefix_matches_the_nightly_tag_shape() {
        let prefix = fallback_tag_prefix();
        assert_eq!(prefix, format!("{ENGINE_VERSION}-nightly-"));
        // A real nightly tag must both start with the prefix and NOT equal the
        // version, or `channel()` would misclassify it as a release.
        let tag = format!("{prefix}16aug26");
        assert!(tag.starts_with(&prefix));
        assert_ne!(tag, ENGINE_VERSION);
    }

    /// A local `cargo test` has no CI env vars, so this is the dev case.
    #[test]
    fn an_unstamped_build_is_dev() {
        if release_tag().is_none() {
            assert_eq!(channel(), BuildChannel::Dev);
            assert_eq!(display(), format!("{ENGINE_VERSION} (dev)"));
        }
    }
}
