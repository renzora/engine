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
    assert!(out.search.is_empty(), "no search path either");
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

/// The host triple, which a real SDK records and a test has to ask for.
fn host_triple() -> String {
    let out = std::process::Command::new(renzora_native_build::tool("rustc"))
        .arg("-vV")
        .output()
        .expect("rustc must run to compile this test");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(|s| s.trim().to_string())
        .expect("`rustc -vV` always has a host line")
}

/// An SDK root holding only a `shared.tsv` with `line` in it.
fn sdk_with(root: &Path, line: &str) -> PathBuf {
    let sdk = root.join("sdk");
    std::fs::create_dir_all(&sdk).unwrap();
    std::fs::write(sdk.join("shared.tsv"), format!("# test table\n{line}\n")).unwrap();
    sdk
}

/// With the SDK sharing it at the same version, an engine crate is not an
/// offender: it is never compiled, and the plugin is handed the SDK's copy.
///
/// The stub is named `bevy_ecs` so the guard sees exactly what a dependency on
/// a real Bevy crate would show it. The recorded path does not have to exist,
/// because nothing in the dependency build reads the substituted crate.
#[test]
fn an_engine_crate_the_sdk_shares_is_taken_from_the_sdk() {
    let root = tmp("renzora_deps_shared");
    let stub = stub_crate(&root, "bevy_ecs");
    let dir = plugin(
        &root,
        &format!("bevy_ecs = {{ path = \"{}\" }}\n", stub.display().to_string().replace('\\', "/")),
    );
    let sdk = sdk_with(&root, "bevy_ecs\t0.1.0\tbevy_ecs\t0\t\tdeps/libbevy_ecs-0000.rmeta");
    let triple = host_triple();
    let share = deps::Share {
        table: &sdk.join("shared.tsv"),
        lockfile: None,
        search: &[],
        native: &[],
        triple: &triple,
    };

    let out = deps::build_with(&dir, &root.join("build"), &host_toolchain(), Some(&share))
        .expect("a crate the SDK shares is allowed");
    let Some((_, path)) = out.externs.iter().find(|(n, _)| n == "bevy_ecs") else {
        panic!("bevy_ecs must be handed over; got {:?}", out.externs);
    };
    assert_eq!(path, &sdk.join("deps/libbevy_ecs-0000.rmeta"), "the SDK's copy, not a compiled one");
    assert!(
        !out.search.iter().any(|d| d.join("libbevy_ecs.rlib").exists()),
        "nothing named bevy_ecs may have been compiled"
    );
}

/// The same crate at a version the SDK does not share would be a second copy,
/// and is refused with both versions named.
#[test]
fn an_engine_crate_at_another_version_is_still_refused() {
    let root = tmp("renzora_deps_shared_mismatch");
    let stub = stub_crate(&root, "bevy_ecs");
    let dir = plugin(
        &root,
        &format!("bevy_ecs = {{ path = \"{}\" }}\n", stub.display().to_string().replace('\\', "/")),
    );
    let sdk = sdk_with(&root, "bevy_ecs\t0.2.0\tbevy_ecs\t0\t\tdeps/libbevy_ecs-0000.rmeta");
    let triple = host_triple();
    let share = deps::Share {
        table: &sdk.join("shared.tsv"),
        lockfile: None,
        search: &[],
        native: &[],
        triple: &triple,
    };

    let msg = deps::build_with(&dir, &root.join("build"), &host_toolchain(), Some(&share))
        .expect_err("a different version is a second copy");
    assert!(msg.contains("bevy_ecs 0.1.0") && msg.contains("0.2.0"), "both versions named: {msg}");
}

#[test]
fn package_ids_of_every_form_parse() {
    let id = "registry+https://github.com/rust-lang/crates.io-index#glam@0.33.12";
    assert_eq!(deps::package_of(id), Some(("glam".into(), "0.33.12".into())));
    let id = "path+file:///C:/plugins/stub/bevy_ecs#0.1.0";
    assert_eq!(deps::package_of(id), Some(("bevy_ecs".into(), "0.1.0".into())));
    let id = "glam 0.33.12 (registry+https://github.com/rust-lang/crates.io-index)";
    assert_eq!(deps::package_of(id), Some(("glam".into(), "0.33.12".into())));
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

    // A version that cannot exist, so no machine can satisfy it.
    //
    // This said `1.0.0` first, on the reasoning that nothing would have it
    // installed. Rust 1.0.0 is a real release: CI named it, `rustup which`
    // **fetched it**, and the build got far enough to run a 2015 cargo that has
    // no `metadata` subcommand — failing with `No such subcommand` instead of
    // the refusal being tested. `rustup_which` no longer installs anything (see
    // its note), and the version here is one the CDN cannot serve either way.
    let msg = deps::build(&dir, &root.join("build"), "9999.0.0").expect_err("must be refused");
    assert!(msg.contains("9999.0.0"), "must name the version the SDK needs: {msg}");
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

/// Resolving a toolchain must never **install** one.
///
/// `rustup which --toolchain <v>` treats naming a toolchain as asking for it, so
/// a lookup meant to answer "is this here?" downloads it instead. CI proved it:
/// a test naming `1.0.0` fetched Rust 1.0.0 and then failed inside a cargo older
/// than `cargo metadata`.
///
/// Two costs, and the second outlives the test. A question that was supposed to
/// be free becomes a few hundred megabytes, and installing a toolchain is a
/// decision `renzora_plugin_build::toolchain` deliberately puts behind a prompt
/// that says how large it is — which a silent install on this path defeats.
///
/// Checked by the clock rather than by looking in `~/.rustup`: an install cannot
/// happen in well under a second, and the assertion then holds without the test
/// knowing how rustup lays its directory out. `1.0.0` specifically, because that
/// is the one that actually got fetched.
#[test]
fn resolving_a_toolchain_never_downloads_one() {
    let root = tmp("renzora_deps_noinstall");
    let stub = stub_crate(&root, "ordinary");
    let dir = plugin(
        &root,
        &format!(
            "ordinary = {{ path = \"{}\" }}\n",
            stub.display().to_string().replace('\\', "/")
        ),
    );

    let started = std::time::Instant::now();
    let msg = deps::build(&dir, &root.join("build"), "1.0.0").expect_err("must be refused");
    let took = started.elapsed();

    assert!(
        msg.contains("1.0.0") && msg.contains("rustup toolchain install"),
        "a real but uninstalled version must be refused, not fetched: {msg}"
    );
    assert!(
        took < std::time::Duration::from_secs(20),
        "took {took:?} — long enough that a toolchain was downloaded"
    );
}
