//! `sdk/shared.tsv`: the crates inside the engine's shared images.
//!
//! A plugin may depend on a crate that is built on Bevy (avian, an ecosystem
//! crate). When its dependencies are compiled on the user's machine, every crate
//! the shared images already contain is taken from the SDK instead of compiled
//! again, so the dependency links against the engine's own Bevy and `glam`
//! rather than a second copy with different types. The wrapper that does it is
//! `crates/renzora_native_build/src/shim.rs`, which also documents the format
//! written here: `package, version, crate, proc_macro, features, path`, tab
//! separated, path relative to the SDK root.
//!
//! # Why the unit graph comes from cargo's fingerprints
//!
//! The SDK holds several builds of some crates: `glam 0.33.12` four times, built
//! for different feature sets and for the host. Only one of each is inside the
//! shared images, and substituting any other compiles, then fails on a type
//! mismatch between two `glam`s with the same name. Nothing in cargo's JSON says
//! which one: it lists artifacts, not the edges between them. A first attempt
//! that kept "the build with the most features" picked wrong.
//!
//! Cargo's fingerprints do record the edges, exactly. Each unit's
//! `.fingerprint/<package>-<hash>/lib-<crate>.json` lists the fingerprint of
//! every unit it depends on, and `lib-<crate>` beside it holds that unit's own
//! fingerprint, as the hex of the `u64`'s little-endian bytes. Walking those from
//! the three images gives the exact set.
//!
//! The walk stops at proc macros. Their dependencies are compiled into the
//! macro, which runs inside the compiler, and are not in any image.
//!
//! Only crates the walk reaches are listed. The SDK also has metadata for crates
//! the engine linked statically into an executable, and a plugin cannot reach
//! their code: substituting one would fail the final link with `E0461`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// One crate the SDK staged, as cargo described it.
pub struct Unit {
    /// The target's crate name, e.g. `glam`.
    pub crate_name: String,
    /// Cargo's package id, which carries the package name and version.
    pub package_id: String,
    pub features: Vec<String>,
    pub proc_macro: bool,
    /// SDK-relative path of the staged file, e.g. `deps/libglam-<hash>.rmeta`.
    pub staged: String,
}

/// The shared images, which the walk starts from.
const ROOTS: [&str; 3] = ["bevy_dylib", "renzora_dylib", "renzora_ember_dylib"];

/// Write `out/shared.tsv` and copy the engine's lockfile beside it.
///
/// Returns how many crates were listed. `profile_dirs` holds every directory
/// whose `.fingerprint/` may hold a unit of this build: the target's, and the
/// host's when cross compiling.
pub fn write(
    out: &Path,
    repo: &Path,
    profile_dirs: &[PathBuf],
    lib_ext: &str,
    units: &[Unit],
) -> Result<usize, String> {
    let fingerprints = index(profile_dirs);
    let by_stem: HashMap<String, &Unit> = units.iter().map(|u| (stem_of(&u.staged), u)).collect();

    let mut queue: VecDeque<String> = VecDeque::new();
    for root in ROOTS {
        match root_unit(profile_dirs, root, lib_ext) {
            Some(key) => queue.push_back(key),
            None => return Err(format!("no build of `{root}` found to start the walk from")),
        }
    }
    let mut closure: HashSet<String> = HashSet::new();
    while let Some(key) = queue.pop_front() {
        if !closure.insert(key.clone()) {
            continue;
        }
        if by_stem.get(&key).is_some_and(|u| u.proc_macro) {
            continue;
        }
        let Some(unit) = fingerprints.units.get(&key) else { continue };
        for dep in &unit.deps {
            if let Some(next) = fingerprints.by_hash.get(dep) {
                queue.push_back(next.clone());
            }
        }
    }

    let mut lines: Vec<String> = Vec::new();
    for key in &closure {
        let Some(unit) = by_stem.get(key) else { continue };
        let Some((package, version)) = package_of(&unit.package_id) else { continue };
        let mut features = unit.features.clone();
        features.sort();
        lines.push(format!(
            "{package}\t{version}\t{}\t{}\t{}\t{}",
            unit.crate_name,
            if unit.proc_macro { "1" } else { "0" },
            features.join(","),
            unit.staged
        ));
    }
    lines.sort();
    let text = format!(
        "# The crates inside this engine's shared images, which a plugin dependency\n\
         # built on Bevy takes from here instead of compiling. Written by xtask's\n\
         # SDK staging; read by renzora_native_build's compiler wrapper.\n\
         # package\tversion\tcrate\tproc_macro\tfeatures\tpath\n{}\n",
        lines.join("\n")
    );
    std::fs::write(out.join("shared.tsv"), text).map_err(|e| e.to_string())?;
    std::fs::copy(repo.join("Cargo.lock"), out.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    Ok(lines.len())
}

/// The `<crate>-<hash>` key of a staged file, the same key a fingerprint
/// directory gives a unit.
fn stem_of(staged: &str) -> String {
    let path = Path::new(staged);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    // Rlibs and Unix shared libraries carry a `lib` prefix; a Windows DLL does
    // not, and a crate whose own name starts with `lib` must keep it there.
    let is_dll = path.extension().is_some_and(|e| e == "dll");
    if is_dll { stem.to_string() } else { stem.strip_prefix("lib").unwrap_or(stem).to_string() }
}

struct FingerprintUnit {
    /// The fingerprints of the units this one depends on, as stored in their
    /// hash files.
    deps: Vec<String>,
}

#[derive(Default)]
struct Index {
    /// `<crate>-<hash>` to the unit.
    units: HashMap<String, FingerprintUnit>,
    /// A unit's own fingerprint to its `<crate>-<hash>`.
    by_hash: HashMap<String, String>,
}

fn index(profile_dirs: &[PathBuf]) -> Index {
    let mut index = Index::default();
    for dir in profile_dirs {
        let Ok(entries) = std::fs::read_dir(dir.join(".fingerprint")) else { continue };
        for entry in entries.flatten() {
            let unit_dir = entry.path();
            let Some(dir_name) = unit_dir.file_name().and_then(|n| n.to_str()) else { continue };
            let Some((_, hash)) = dir_name.rsplit_once('-') else { continue };
            let Ok(files) = std::fs::read_dir(&unit_dir) else { continue };
            for file in files.flatten() {
                let name = file.file_name().to_string_lossy().to_string();
                let Some(crate_name) = name.strip_prefix("lib-") else { continue };
                if crate_name.ends_with(".json") {
                    continue;
                }
                let Ok(own) = std::fs::read_to_string(file.path()) else { continue };
                let Ok(json) = std::fs::read_to_string(unit_dir.join(format!("{name}.json"))) else { continue };
                let key = format!("{crate_name}-{hash}");
                index.by_hash.insert(own.trim().to_string(), key.clone());
                index.units.insert(key, FingerprintUnit { deps: dep_hashes(&json) });
            }
        }
    }
    index
}

/// The fingerprints in a fingerprint json's `"deps"`, converted to the form the
/// hash files hold. Each entry is `[package hash, "crate", public, fingerprint]`.
fn dep_hashes(json: &str) -> Vec<String> {
    let Some(start) = json.find("\"deps\":[") else { return Vec::new() };
    let mut rest = &json[start + "\"deps\":[".len()..];
    let mut out = Vec::new();
    loop {
        rest = rest.trim_start_matches([',', ' ']);
        let Some(body) = rest.strip_prefix('[') else { break };
        let Some(end) = body.find(']') else { break };
        if let Some(fp) = body[..end].rsplit(',').next().and_then(|n| n.trim().parse::<u64>().ok()) {
            out.push(le_hex(fp));
        }
        rest = &body[end + 1..];
    }
    out
}

/// A fingerprint as cargo writes it to a unit's hash file.
fn le_hex(value: u64) -> String {
    value.to_le_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// The `<crate>-<hash>` of the build of `name` that this staging's image is.
///
/// The image cargo reports is the uplifted copy, `target/<profile>/<name>.dll`,
/// which carries no hash. It is a hard link to `deps/<name>-<hash>.dll`, so the
/// hashed copy with the same length and modification time is that unit. Other
/// builds of the same crate (a `cargo test -p` of something, an older feature
/// set) sit in `deps/` beside it and differ in both.
fn root_unit(profile_dirs: &[PathBuf], name: &str, lib_ext: &str) -> Option<String> {
    let prefix = if lib_ext == "dll" { "" } else { "lib" };
    for dir in profile_dirs {
        let uplifted = dir.join(format!("{prefix}{name}.{lib_ext}"));
        let Ok(want) = std::fs::metadata(&uplifted) else { continue };
        let Ok(entries) = std::fs::read_dir(dir.join("deps")) else { continue };
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            let Some(rest) = file.strip_prefix(&format!("{prefix}{name}-")) else { continue };
            let Some(hash) = rest.strip_suffix(&format!(".{lib_ext}")) else { continue };
            let Ok(meta) = entry.metadata() else { continue };
            if meta.len() == want.len() && meta.modified().ok() == want.modified().ok() {
                return Some(format!("{name}-{hash}"));
            }
        }
    }
    // `renzora_dylib` and `renzora_ember_dylib` are built under a fixed name, so
    // `deps/` holds them unhashed and the search above finds nothing (which left
    // every SDK without a table). A fixed name is overwritten by whichever build
    // of the crate ran last, so the newest fingerprint is the build on disk.
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for dir in profile_dirs {
        if !dir.join("deps").join(format!("{prefix}{name}.{lib_ext}")).is_file() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir.join(".fingerprint")) else { continue };
        for entry in entries.flatten() {
            let unit = entry.file_name().to_string_lossy().to_string();
            let Some(hash) = unit.strip_prefix(&format!("{name}-")) else { continue };
            let Ok(meta) = std::fs::metadata(entry.path().join(format!("lib-{name}"))) else { continue };
            let Ok(modified) = meta.modified() else { continue };
            if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
                newest = Some((modified, format!("{name}-{hash}")));
            }
        }
    }
    newest.map(|(_, key)| key)
}

/// `(name, version)` from a cargo package id. Kept in step with
/// `renzora_native_build::deps::package_of`, which xtask cannot depend on.
fn package_of(id: &str) -> Option<(String, String)> {
    if let Some((url, fragment)) = id.split_once('#') {
        if let Some((name, version)) = fragment.split_once('@') {
            return Some((name.to_string(), version.to_string()));
        }
        let path = url.split('?').next().unwrap_or(url).trim_end_matches('/');
        let name = path.rsplit('/').next()?.trim_end_matches(".git");
        return Some((name.to_string(), fragment.to_string()));
    }
    let mut parts = id.split_whitespace();
    let name = parts.next()?;
    let version = parts.next()?;
    parts.next()?.starts_with('(').then(|| (name.to_string(), version.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of a real entry, taken from `bevy_dylib`'s fingerprint. The
    /// conversion is what made the walk work: the decimal written as plain hex
    /// matches no hash file, its little-endian bytes match the dependency's.
    #[test]
    fn a_dependency_fingerprint_is_converted_to_its_hash_file_form() {
        let json = r#"{"rustc":1,"deps":[[149,"bevy_internal",false,11884317706700096608]],"local":[]}"#;
        assert_eq!(dep_hashes(json), vec![le_hex(11884317706700096608)]);
        assert_eq!(le_hex(0x0102_0304_0506_0708), "0807060504030201");
    }

    #[test]
    fn several_and_no_dependencies_both_parse() {
        let many = r#"{"deps":[[1,"a",false,2],[3,"b",true,4]],"local":[]}"#;
        assert_eq!(dep_hashes(many), vec![le_hex(2), le_hex(4)]);
        assert!(dep_hashes(r#"{"deps":[],"local":[]}"#).is_empty());
    }

    #[test]
    fn staged_names_map_to_unit_keys() {
        assert_eq!(stem_of("deps/libglam-01ab.rmeta"), "glam-01ab");
        assert_eq!(stem_of("deps/libbevy-77.rlib"), "bevy-77");
        assert_eq!(stem_of("host-deps/num_derive-9.dll"), "num_derive-9");
        assert_eq!(stem_of("host-deps/libnum_derive-9.so"), "num_derive-9");
        assert_eq!(stem_of("deps/liblibm-3.rmeta"), "libm-3");
    }

    #[test]
    fn package_ids_of_every_form_parse() {
        let id = "registry+https://github.com/rust-lang/crates.io-index#glam@0.33.12";
        assert_eq!(package_of(id), Some(("glam".into(), "0.33.12".into())));
        let id = "git+https://github.com/x/avian.git?rev=34c9#avian3d@0.8.0-dev";
        assert_eq!(package_of(id), Some(("avian3d".into(), "0.8.0-dev".into())));
        let id = "path+file:///C:/engine/crates/renzora#0.1.0";
        assert_eq!(package_of(id), Some(("renzora".into(), "0.1.0".into())));
        let id = "glam 0.33.12 (registry+https://github.com/rust-lang/crates.io-index)";
        assert_eq!(package_of(id), Some(("glam".into(), "0.33.12".into())));
    }
}
