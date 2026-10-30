//! The check that nothing built on Bevy comes from crates.io (CLAUDE.md §8).
//!
//! Bevy is `bevy/`, a subtree of Bevy's `main`, and every crate built on it
//! lives in `vendor/` pointing there by path. A crates.io dependency that pulls
//! in a Bevy crate breaks that silently: cargo resolves the registry copy
//! beside ours, the build succeeds, and the program carries two Bevys, whose
//! `World`s, `TypeId`s and plugin images disagree. Nothing fails until something
//! crosses between them.
//!
//! So the build graph is read before every build, and a Bevy package from
//! anywhere but this tree stops it, naming the crate that pulled it in.
//!
//! It reads `cargo tree`, which resolves features, rather than `Cargo.lock`. The
//! lockfile also records optional dependencies nobody enables (polyanya's
//! `rerecast` lists a registry `bevy_reflect` behind a feature that is off), and
//! failing on those would make the check cry wolf.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// Fails, with a message naming each offender and what pulled it in, if the
/// build graph holds a Bevy package from outside `bevy/`.
pub fn check(repo: &Path) -> Result<(), String> {
    let bevy_packages = bevy_package_names(&repo.join("bevy"))?;
    if bevy_packages.is_empty() {
        return Err("found no packages under bevy/, so the check cannot run".into());
    }

    let tree = cargo_tree(repo, &["--workspace"])?;
    let mut foreign: BTreeSet<String> = BTreeSet::new();
    for line in tree.lines() {
        let line = line.trim();
        let Some((name, rest)) = line.split_once(' ') else { continue };
        if bevy_packages.contains(name) && !is_local(rest) {
            // `name vX.Y.Z`, the spec `cargo tree -i` takes to find the culprit.
            let version = rest.split_whitespace().next().unwrap_or("").trim_start_matches('v');
            foreign.insert(format!("{name}@{version}"));
        }
    }
    if foreign.is_empty() {
        return Ok(());
    }

    let mut message = String::from(
        "a Bevy crate comes from outside bevy/ (CLAUDE.md §8: nothing built on Bevy \
         may come from crates.io). Vendor the crate that pulls it in under vendor/ \
         and point it at bevy/ by path.\n",
    );
    for spec in &foreign {
        message.push_str(&format!("\n  {spec}, pulled in by:\n"));
        let inverted = cargo_tree(repo, &["--workspace", "-i", spec])
            .unwrap_or_else(|e| format!("    (could not trace it: {e})\n"));
        for path in inverted.lines().take(12) {
            message.push_str(&format!("    {path}\n"));
        }
    }
    Err(message)
}

/// `cargo tree` over every target platform, normal and build edges, one package
/// per line, no tree drawing.
fn cargo_tree(repo: &Path, extra: &[&str]) -> Result<String, String> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(repo)
        .args(["tree", "--target", "all", "-e", "normal,build", "--prefix", "none", "--format", "{p}"])
        .args(extra)
        .output()
        .map_err(|e| format!("could not run cargo tree: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo tree failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Whether the rest of a `cargo tree` line (`vX.Y.Z (source)`) names a path on
/// disk. A registry package prints no source at all, and a git one a URL.
fn is_local(rest: &str) -> bool {
    rest.split('(')
        .skip(1)
        .filter_map(|part| part.split(')').next())
        .any(|source| !source.contains("://") && (source.contains('/') || source.contains('\\')))
}

/// Every package name declared under `dir`, read from the `[package]` table of
/// each `Cargo.toml`. A plain scan: xtask has no dependencies, TOML parser
/// included, and a package's `name` line is all that is needed.
fn bevy_package_names(dir: &Path) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name();
            if path.is_dir() {
                if file_name != "target" && file_name != ".git" {
                    pending.push(path);
                }
            } else if file_name == "Cargo.toml" {
                if let Some(name) = std::fs::read_to_string(&path).ok().and_then(|text| package_name(&text)) {
                    names.insert(name);
                }
            }
        }
    }
    Ok(names)
}

/// The `name` in a manifest's `[package]` table.
fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(value) = line.strip_prefix("name") {
            let value = value.trim_start().strip_prefix('=')?.trim();
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_source_is_local_and_a_registry_or_git_one_is_not() {
        assert!(is_local(r"v0.20.0-dev (C:\work\engine\bevy\crates\bevy_ecs)"));
        assert!(is_local("v0.20.0-dev (/home/me/engine/bevy/crates/bevy_ecs)"));
        assert!(!is_local("v0.20.0"));
        assert!(!is_local("v0.20.0 (*)"));
        assert!(!is_local("v0.20.0 (proc-macro)"));
        assert!(!is_local("v0.20.0 (https://github.com/bevyengine/bevy#ae615215)"));
    }

    #[test]
    fn the_package_name_is_read_from_the_package_table_only() {
        let manifest = "[workspace]\nname = \"not-this\"\n\n[package]\n# a comment\nname = \"bevy-settings\"\nversion = \"0.20.0-dev\"\n";
        assert_eq!(package_name(manifest).as_deref(), Some("bevy-settings"));
        assert_eq!(package_name("[workspace]\nmembers = []\n"), None);
    }
}
