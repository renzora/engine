//! End-to-end check that a plugin compiles against a real staged SDK.
//!
//! Skipped when `dist/<platform>/sdk/` is absent — CI's test lane does not run
//! `cargo renzora dist`, and a test that fails for want of a build artefact is a
//! test people learn to ignore. Run `cargo renzora dist` first to exercise it.

use std::path::PathBuf;

use renzora_plugin_build::{Error, Sdk};

/// `crates/renzora_plugin_build` → repo root → the staged SDK for this host.
fn staged_sdk() -> Option<PathBuf> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?.to_path_buf();
    let dir = if cfg!(target_os = "windows") {
        "windows-x64"
    } else if cfg!(target_os = "macos") {
        "macos-x64"
    } else {
        "linux-x64"
    };
    let sdk = repo.join("dist").join(dir).join("sdk");
    sdk.join("manifest.json").is_file().then_some(sdk)
}

const PLUGIN: &str = r#"
use bevy::prelude::*;

pub struct P;
impl Plugin for P {
    fn build(&self, app: &mut App) { app.add_systems(Update, s); }
}

// Exclusive `&mut World` — the access the whole shared-dylib arrangement exists
// to make possible.
fn s(world: &mut World) {
    let dt = world.resource::<Time>().delta_secs();
    let mut q = world.query::<&mut Transform>();
    for mut t in q.iter_mut(world) { t.rotate_y(dt); }
}

// A real function body from the contract crate, reading a process-global static.
// If `renzora` were linked statically this would resolve against a private copy.
fn _translated() -> String { renzora::lang::t("menu.file") }

renzora::plugin!(P);
"#;

#[test]
fn compiles_a_plugin_against_the_staged_sdk() {
    let Some(root) = staged_sdk() else {
        eprintln!("no staged SDK — run `cargo renzora dist` to exercise this test");
        return;
    };

    let sdk = Sdk::load(&root).expect("load manifest");
    if let Err(Error::Toolchain { needs, found, .. }) = sdk.check_toolchain() {
        eprintln!("SDK needs rustc {needs}, this is {found} — skipping");
        return;
    }

    // `compile` takes the plugin's ROOT and derives `src/lib.rs` from it, so the
    // layout here is the shipped one: source only, nothing else.
    let tmp = std::env::temp_dir().join("renzora_plugin_build_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(tmp.join("src").join("lib.rs"), PLUGIN).unwrap();
    let out = tmp.join(format!("testplug.{}", sdk.manifest().lib_ext));

    let stamp = match sdk.compile(&tmp, &out) {
        Ok(s) => s,
        Err(e) => panic!("compile failed:\n{e}"),
    };

    let bytes = std::fs::metadata(&out).expect("plugin written").len();
    // Sharing is the point, so assert on it rather than on "a file appeared".
    // Statically linking the contract crate alone adds ~4.8 MB; anything in that
    // range means the plugin embedded what it was supposed to import.
    assert!(
        bytes < 2 * 1024 * 1024,
        "plugin is {bytes} bytes — it embedded Bevy or the contract crate \
         instead of importing them"
    );
    assert!(stamp.contains("rustc-"), "stamp records the toolchain: {stamp}");

    let _ = std::fs::remove_dir_all(&tmp);
}
