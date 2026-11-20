//! Building one Rust script: a native plugin made from a single `.rs` file.
//!
//! Here rather than in `renzora_rust_script`, which loads and runs scripts and
//! is built on Bevy, because building one is only file work and a `rustc` run:
//! the standalone companion compiles scripts too, and links no engine. The
//! script crate re-exports [`build_to_path`], so its callers did not change.

use std::path::{Path, PathBuf};

use crate::Sdk;

/// Compile one `.rs` into `<project>/.renzora/scripts/`, returning the library.
///
/// Split from loading the library so the compile (the second that matters) can
/// run on a task pool, or in the companion, while the load stays on the
/// editor's main thread. Nothing here touches a `World`, which is what makes
/// that possible.
///
/// The build directory is hidden inside the project because these are derived:
/// they belong with the project, but nobody should be asked to look at or commit
/// them.
pub fn build_to_path(sdk: &Sdk, project: &Path, src: &Path) -> Result<PathBuf, String> {
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("script");
    let build = project.join(".renzora").join("scripts").join(stem);
    let source = std::fs::read(src).map_err(|e| e.to_string())?;

    // The library is named for what it was built from: this source, against this
    // SDK. A script that has not changed since the last launch therefore finds
    // its library already on disk and skips rustc, which used to cost about a
    // second per script on every project open.
    //
    // It also keeps the old rule that a recompile writes a NEW file rather than
    // overwriting the loaded one: on Windows the previous library is still
    // mapped and cannot be replaced, and on every platform overwriting a mapped
    // image is a way to crash later rather than fail now. Changed source is a
    // different name; unchanged source is the same, already-correct file.
    let key = build_key(&sdk.stamp(), &source);
    let ext = &sdk.manifest().lib_ext;
    let out = build.join(format!("{stem}-{key:016x}.{ext}"));
    if out.is_file() {
        return Ok(out);
    }

    // `Sdk::compile` takes a plugin-shaped DIRECTORY (`<dir>/src/lib.rs`): it
    // generates the Cargo.toml Bevy's derives need there and takes the crate name
    // from it. A script is one loose file, so it is staged into that shape rather
    // than teaching the compiler a second layout.
    std::fs::create_dir_all(build.join("src")).map_err(|e| e.to_string())?;
    std::fs::write(build.join("src").join("lib.rs"), &source).map_err(|e| e.to_string())?;

    // Built under a temporary name and renamed into place, so a build that dies
    // halfway can never leave a file the cache check above would trust.
    let partial = build.join(format!("{stem}-{key:016x}.partial.{ext}"));
    sdk.compile(&build, &partial).map_err(|e| {
        // Point at the file the author edits, not the staged copy they have never
        // seen: a diagnostic naming `.renzora/scripts/spin/src/lib.rs` sends
        // them to a derived file that is overwritten on every build.
        e.to_string()
            .replace(&build.join("src").join("lib.rs").to_string_lossy().to_string(), &src.to_string_lossy())
    })?;
    std::fs::rename(&partial, &out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// FNV-1a over the SDK stamp and the script source.
///
/// Spelled out rather than taken from `std`, whose hasher is not promised to
/// give the same answer from one Rust release to the next; this key names files
/// that have to be found again on the next launch.
pub fn build_key(stamp: &str, source: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in stamp.as_bytes().iter().chain([0u8].iter()).chain(source) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key names files found again on the next launch: the same inputs
    /// must give the same key, and a change to either must give another.
    #[test]
    fn the_key_is_stable_and_depends_on_both_inputs() {
        let a = build_key("stamp", b"fn main() {}");
        assert_eq!(a, build_key("stamp", b"fn main() {}"));
        assert_ne!(a, build_key("stamp2", b"fn main() {}"));
        assert_ne!(a, build_key("stamp", b"fn main() { }"));
        // The separator keeps "ab"+"c" apart from "a"+"bc".
        assert_ne!(build_key("ab", b"c"), build_key("a", b"bc"));
    }
}
