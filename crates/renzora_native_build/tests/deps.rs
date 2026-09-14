//! A native plugin's third-party dependencies.
//!
//! The case worth testing is the **refusal**: a dependency that drags Bevy back
//! in is the one failure this whole mechanism exists to prevent, and it is the
//! one that cannot be allowed to fail quietly — a second Bevy compilation gives
//! the plugin different `TypeId`s, so it builds, loads, and then reads the
//! engine's `World` through the wrong layouts.
//!
//! # Why the fixtures use a local stub rather than a real crates.io crate
//!
//! The obvious test is to depend on something that genuinely pulls Bevy. It also
//! takes minutes: cargo has to fetch the registry index and resolve Bevy's whole
//! tree before it can answer, and a test that needs the network is a test that
//! gets skipped.
//!
//! The guard matches on the crate NAMES in the resolved graph, so a local stub
//! crate named `bevy_ecs` presents cargo with exactly what the real case would.
//! Path dependencies resolve offline and instantly. Same code path, same answer,
//! no network.

use std::path::{Path, PathBuf};

use renzora_native_build::deps;

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The version to pin these builds to: whatever is compiling the test.
///
/// A real SDK passes its manifest's `rustc`, which is a version the machine was
/// made to have. A test has no SDK, so it names the one compiler that is
/// certainly present — and by doing so still exercises the pinned path rather
/// than a bypass, on a machine of any toolchain layout.
fn host_toolchain() -> String {
    let out = std::process::Command::new(renzora_native_build::tool("rustc"))
        .arg("-vV")
        .output()
        .expect("rustc must run to compile this test");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("release: "))
        .map(|s| s.trim().to_string())
        .expect("`rustc -vV` always has a release line")
}

/// A do-nothing crate at `root/<name>`, for use as a path dependency.
///
/// Its name is the entire point — the guard reads names out of the resolved
/// graph, so this is a faithful stand-in for any real crate of that name.
fn stub_crate(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
    )
    .unwrap();
    std::fs::write(dir.join("src").join("lib.rs"), "").unwrap();
    dir
}

/// A plugin directory whose `Cargo.toml` carries `deps` in `[dependencies]`.
fn plugin(root: &Path, deps_block: &str) -> PathBuf {
    let dir = root.join("plug");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"plug\"\nversion = \"0.1.0\"\n\
             edition = \"2021\"\n\n[lib]\ncrate-type = [\"dylib\"]\n\n\
             [dependencies]\n{deps_block}"
        ),
    )
    .unwrap();
    std::fs::write(dir.join("src").join("lib.rs"), "").unwrap();
    dir
}

/// The common case, and the one that must stay fast and offline: a plugin that
/// asks for nothing beyond what the SDK already hands it.
///
/// `deps::build` has to return empty *without invoking cargo at all* — that is
/// what keeps an ordinary plugin building in about a second with no network.
#[test]
fn a_plugin_with_no_third_party_crates_never_runs_cargo() {
    let root = tmp("renzora_deps_none");
    let dir = plugin(
        &root,
        "bevy = \"0.19\"\nrenzora = { path = \"../../crates/renzora\" }\n\
         renzora_ember = { path = \"../../crates/renzora_ember\" }\n",
    );

    let out = deps::build(&dir, &root.join("build"), &host_toolchain())
        .expect("no deps is not an error");
    assert!(out.externs.is_empty(), "nothing to pass to rustc");
    assert!(out.search.is_none(), "no search path either");
    // The tell that cargo never ran: nothing was even written. Those path
    // dependencies point at directories that do not exist from here, so a build
    // would have failed rather than returned Ok.
    assert!(
        !root.join("build").join("deps").join("Cargo.toml").exists(),
        "no manifest should be synthesized when there is nothing to build"
    );
}

/// The refusal, on a DIRECT dependency.
///
/// `bevy_ecs` is not stripped the way `bevy` is, on purpose: writing it is a
/// mistake (the SDK's Bevy is reached through the `bevy` facade), and being told
/// so beats being silently given something else.
#[test]
fn a_bevy_subcrate_dependency_is_refused_by_name() {
    let root = tmp("renzora_deps_subcrate");
    let stub = stub_crate(&root, "bevy_ecs");
    let dir = plugin(
        &root,
        &format!(
            "bevy = \"0.19\"\nbevy_ecs = {{ path = \"{}\" }}\n",
            stub.display().to_string().replace('\\', "/")
        ),
    );

    let msg = deps::build(&dir, &root.join("build"), &host_toolchain())
        .expect_err("must be refused");
    assert!(msg.contains("bevy_ecs"), "the message must name the offender: {msg}");
    assert!(
        msg.contains("TypeId"),
        "and say why it matters, not just that it is banned: {msg}"
    );
}

/// The refusal, on an INDIRECT dependency — the case the guard actually exists
/// for, since nobody writes `bevy_ecs` in a plugin manifest by accident but
/// plenty of ordinary crates depend on Bevy.
///
/// `harmless` is a perfectly reasonable-looking dependency; it just happens to
/// pull Bevy in underneath.
#[test]
fn a_dependency_that_itself_pulls_bevy_is_refused() {
    let root = tmp("renzora_deps_indirect");
    let bevy = stub_crate(&root, "bevy_transform");
    let mid = stub_crate(&root, "harmless");
    // Make `harmless` depend on the Bevy stub, so the offender is one level down
    // and invisible in the plugin's own manifest.
    let manifest = std::fs::read_to_string(mid.join("Cargo.toml")).unwrap();
    std::fs::write(
        mid.join("Cargo.toml"),
        format!(
            "{manifest}\n[dependencies]\nbevy_transform = {{ path = \"{}\" }}\n",
            bevy.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();

    let dir = plugin(
        &root,
        &format!(
            "bevy = \"0.19\"\nharmless = {{ path = \"{}\" }}\n",
            mid.display().to_string().replace('\\', "/")
        ),
    );

    let msg = deps::build(&dir, &root.join("build"), &host_toolchain())
        .expect_err("must be refused");
    assert!(
        msg.contains("bevy_transform"),
        "must name the transitive offender, not just the direct dependency: {msg}"
    );
}

/// The sub-table form is refused rather than silently ignored.
///
/// Dropping it without a word would surface much later as "cannot find crate",
/// pointing at the plugin's source instead of at its manifest.
#[test]
fn a_dependency_sub_table_is_refused_with_a_useful_message() {
    let root = tmp("renzora_deps_subtable");
    let dir = root.join("plug");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"plug\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [dependencies.noise]\nversion = \"0.9\"\n",
    )
    .unwrap();

    let msg = deps::build(&dir, &root.join("build"), &host_toolchain())
        .expect_err("must be refused");
    assert!(msg.contains("one per line"), "must say what to do instead: {msg}");
}

/// A toolchain the machine does not have must be refused **before** anything is
/// built, and the message must name the version and how to get it.
///
/// This is the failure that used to arrive as `error[E0514]: found crate
/// 'sysinfo' compiled by an incompatible version of rustc` — accurate, but
/// pointing at a dependency the author never chose a version of, after cargo had
/// already compiled the whole tree with the wrong compiler.
#[test]
fn a_toolchain_the_machine_lacks_is_refused_before_cargo_runs() {
    let root = tmp("renzora_deps_toolchain");
    let stub = stub_crate(&root, "ordinary");
    let dir = plugin(
        &root,
        &format!(
            "ordinary = {{ path = \"{}\" }}\n",
            stub.display().to_string().replace('\\', "/")
        ),
    );

    // A version that has never existed, so neither rustup nor a `PATH` compiler
    // can satisfy it on any machine this test runs on.
    let msg = deps::build(&dir, &root.join("build"), "1.0.0").expect_err("must be refused");
    assert!(msg.contains("1.0.0"), "must name the version the SDK needs: {msg}");
    assert!(
        msg.contains("rustup toolchain install"),
        "must say how to get it, not just that it is missing: {msg}"
    );
    assert!(
        !root.join("build").join("deps").join("target").exists(),
        "nothing should have been compiled before the toolchain was checked"
    );
}

/// A `target/` left by a different toolchain is discarded rather than reused.
///
/// A successful build deletes `deps/` wholesale, so the only tree this ever
/// meets is one a FAILED build left — which is exactly when it is most likely to
/// have been produced by the wrong compiler. Reusing it would make a corrected
/// toolchain fail identically to the uncorrected one, which reads as the fix not
/// having worked.
#[test]
fn a_dependency_tree_from_another_toolchain_is_discarded() {
    let root = tmp("renzora_deps_stale");
    let stub = stub_crate(&root, "ordinary");
    let dir = plugin(
        &root,
        &format!(
            "ordinary = {{ path = \"{}\" }}\n",
            stub.display().to_string().replace('\\', "/")
        ),
    );

    // Stand in for what a failed build under an older compiler leaves behind.
    let deps_dir = root.join("build").join("deps");
    std::fs::create_dir_all(deps_dir.join("target").join("release")).unwrap();
    std::fs::write(deps_dir.join(".toolchain"), "1.94.0\n").unwrap();
    let stale = deps_dir.join("target").join("release").join("libordinary.rlib");
    std::fs::write(&stale, b"metadata from the wrong rustc").unwrap();

    deps::build(&dir, &root.join("build"), &host_toolchain()).expect("an ordinary crate builds");

    assert!(!stale.exists(), "the other toolchain's artefacts must not survive");
    assert_eq!(
        std::fs::read_to_string(deps_dir.join(".toolchain")).unwrap().trim(),
        host_toolchain(),
        "the tree must be stamped with the toolchain that actually built it"
    );
}
