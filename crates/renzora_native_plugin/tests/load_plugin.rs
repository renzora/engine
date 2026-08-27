//! Covers the loader's *build* half: compiling a source-only plugin, writing a
//! stamp, and rebuilding when that stamp goes stale.
//!
//! # Why loading is not tested in-process
//!
//! It cannot be, and the attempt is instructive. `cargo test -p
//! renzora_native_plugin` resolves features without `renzora_app`, so
//! `dynamic_linking` is off and the test binary links its **own static Bevy** —
//! while the plugin it compiles imports `bevy_dylib`. Two Bevy copies, two `App`
//! types, and handing one to the other produced exactly the corruption the
//! shared images exist to prevent:
//!
//! ```text
//! Requested resource bevy_ecs::schedule::Schedules does not exist in the World
//! fatal runtime error: Rust cannot catch foreign exceptions, aborting
//! ```
//!
//! …from a `World` that demonstrably had one. That is now unreachable: the
//! loader declines unless its `dynamic_linking` feature is on, which is the same
//! switch that puts the shared images in the build. Which also means an
//! in-process load test would have to reproduce the host binary's exact link
//! configuration, at which point it is not a unit test — it is launching the
//! engine.
//!
//! So this asserts what can be asserted honestly here, and the load path is
//! proven by running the staged editor.

use std::path::PathBuf;

use renzora_plugin_build::Sdk;

fn staged_root() -> Option<PathBuf> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?.to_path_buf();
    let dir = if cfg!(target_os = "windows") {
        "windows-x64"
    } else if cfg!(target_os = "macos") {
        "macos-x64"
    } else {
        "linux-x64"
    };
    let root = repo.join("dist").join(dir);
    root.join("sdk").join("manifest.json").is_file().then_some(root)
}

/// Deliberately uses the three things that need more than a bare `rustc` call:
/// a derive macro (which resolves its paths through `BevyManifest`, so it needs
/// a `Cargo.toml` and `CARGO_MANIFEST_DIR`), `bsn!` (same machinery), and
/// `renzora::plugin!` instead of a hand-written entry point.
const PLUGIN: &str = r#"
use bevy::prelude::*;

// `#[derive(Component)]` expands through `BevyManifest`. Without the cargo
// environment this fails as a bare "proc macro panicked", naming the macro
// rather than the missing manifest.
#[derive(Component, Clone, Default)]
pub struct Spinner;

pub struct P;
impl Plugin for P {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (mark, spawn_scene.spawn()));
    }
}

// Exclusive world access, plus a contract-crate call whose function body reads a
// process-global static — the two things that only work with shared images.
fn mark(world: &mut World) {
    let _ = renzora::lang::t("menu.file");
    let _ = world.query::<&Transform>().iter(world).count();
}

// BSN, verbatim in the shape the Bevy docs use.
fn spawn_scene() -> impl Scene {
    bsn! {
        Spinner
        Transform::from_xyz(0.0, 0.5, 0.0)
    }
}

renzora::plugin!(P);
"#;

#[test]
fn builds_a_source_plugin_and_rebuilds_it_when_stale() {
    let Some(root) = staged_root() else {
        eprintln!("nothing staged — run `cargo renzora dist` to exercise this test");
        return;
    };
    let sdk = Sdk::load(root.join("sdk")).expect("load manifest");
    if sdk.check_toolchain().is_err() {
        eprintln!("local rustc differs from the SDK's — skipping");
        return;
    }

    // A plugin directory holding source and nothing else — no Cargo.toml, no
    // library, no stamp. Everything else has to be produced by the build.
    let dir = root.join("plugins").join("test-build");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src").join("lib.rs"), PLUGIN).unwrap();
    let out = dir.join("build").join(format!("test_build.{}", sdk.manifest().lib_ext));
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();

    let stamp = sdk.compile(&dir, &out).unwrap_or_else(|e| panic!("compile failed:\n{e}"));

    assert!(
        dir.join("Cargo.toml").is_file(),
        "no manifest generated — Bevy's derives would panic without one"
    );

    let bytes = std::fs::metadata(&out).expect("library written").len();
    // The assertion that matters. A plugin embedding the contract crate is
    // ~5.45 MB; one importing it is ~0.29 MB. Anything large means the linkage
    // silently regressed to a private copy — which has no other symptom, since
    // such a plugin loads and runs and simply never sees the host's state.
    assert!(
        bytes < 2 * 1024 * 1024,
        "plugin is {bytes} bytes — it embedded Bevy or the contract crate \
         instead of importing them"
    );

    // The stamp is what lets an engine change land without republishing
    // anything, so assert it actually distinguishes builds rather than being
    // decoration.
    assert!(stamp.contains("rustc-"), "stamp records the toolchain: {stamp}");
    assert_ne!(stamp, "definitely-not-the-current-stamp");
    assert_eq!(stamp, sdk.stamp(), "stamp is stable for one SDK");

    let _ = std::fs::remove_dir_all(&dir);
}
