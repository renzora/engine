//! Does a plugin-to-plugin `--extern` actually work? Build two and find out.
//!
//! ```sh
//! cargo run --profile dist -p renzora_plugin_build --example plugin_interop -- --sdk dist/windows-x64/sdk
//! ```
//!
//! The claim under test: plugin A can `use plugin_b::SomeType` if A's `rustc`
//! invocation is handed `--extern plugin_b=<B's dylib>`, and the two will then
//! agree about that type at runtime because both link the same shared Bevy.
//!
//! This builds exactly that: `handshake_b` defines a resource of **its own**
//! (deliberately not a contract-crate type, which would prove nothing, since every
//! plugin already shares those), and `handshake_a` imports it by name and reads
//! it out of the world.
//!
//! # What this can and cannot decide
//!
//! It settles the **compile and link** half on its own: whether `rustc` accepts
//! the edge, and whether A's image ends up importing B's. The **runtime** half
//! needs the editor, so the plugins are written where the loader will find them
//! and the log lines they print are the proof.
//!
//! # Two obstacles this exists to surface
//!
//! **The build empties `.rustc`.** `prune_byproducts` blanks the crate-metadata
//! section out of every finished plugin, saving 41% of the file on the measured
//! case, and its doc-comment says why that is safe: "Nothing does that to a
//! plugin; the host opens it by symbol name at runtime." Making B
//! `--extern`-able falsifies that sentence, so B is built here **without** the
//! prune. A plugin that is also a library cannot be pruned.
//!
//! **Windows resolves imports by name, not by path.** A's image will import
//! `handshake_b.dll`, and the loader searches the executable's directory, the
//! system directories and `PATH`, never a sibling plugin's `build/` folder. The
//! saving grace is that a module already loaded under that name resolves without
//! any search, so **B must be loaded before A**. That is the dependency graph
//! showing up as a load-order constraint.

use std::path::{Path, PathBuf};
use std::process::Command;

use renzora_plugin_build::{install, rustc, Sdk};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sdk_dir = args
        .iter()
        .position(|a| a == "--sdk")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .or_else(|| install::root().map(|r| install::sdk_dir(&r)));
    let Some(sdk_dir) = sdk_dir else {
        eprintln!("usage: plugin_interop --sdk <dir>");
        std::process::exit(2);
    };

    let sdk = match Sdk::load(&sdk_dir) {
        Ok(sdk) => sdk,
        Err(e) => {
            eprintln!("no usable SDK at {}: {e}", sdk_dir.display());
            std::process::exit(1);
        }
    };
    let rustc = match sdk.toolchain().rustc().cloned() {
        Some(rustc) => rustc,
        None => {
            eprintln!("the SDK's pinned rustc ({}) is not installed", sdk.manifest().rustc);
            std::process::exit(1);
        }
    };

    let scratch = std::env::temp_dir().join("renzora-plugin-interop");
    let _ = std::fs::remove_dir_all(&scratch);
    write_sources(&scratch);

    let ext = &sdk.manifest().lib_ext;
    let b_out = scratch.join("handshake_b").join(format!("handshake_b.{ext}"));
    let a_out = scratch.join("handshake_a").join(format!("handshake_a.{ext}"));

    // ── B first, and without the prune ──────────────────────────────────────
    println!("[1/3] building handshake_b (metadata kept, so it can be linked)");
    if let Err(e) = build(&rustc, &sdk, &sdk_dir, &scratch.join("handshake_b"), &b_out, &[]) {
        eprintln!("\nhandshake_b did not build:\n{e}");
        std::process::exit(1);
    }
    println!("      -> {}", b_out.display());

    // ── A, handed B as an extern ────────────────────────────────────────────
    println!("[2/3] building handshake_a with --extern handshake_b");
    match build(
        &rustc,
        &sdk,
        &sdk_dir,
        &scratch.join("handshake_a"),
        &a_out,
        &[("handshake_b".to_string(), b_out.clone())],
    ) {
        Ok(()) => println!("      -> {}", a_out.display()),
        Err(e) => {
            eprintln!("\nhandshake_a did not build against handshake_b:\n{e}");
            eprintln!(
                "\nTHEORY FAILED at the compile step: rustc would not accept a plugin dylib as \
                 an --extern for another plugin."
            );
            std::process::exit(1);
        }
    }

    // ── What the artifacts say ──────────────────────────────────────────────
    println!("[3/3] inspecting the result");
    let a_bytes = std::fs::read(&a_out).unwrap_or_default();
    let imports_b = contains(&a_bytes, b"handshake_b");
    let a_ctor = contains(&a_bytes, b"renzora_native_plugin_ctor");
    let b_bytes = std::fs::read(&b_out).unwrap_or_default();
    let b_ctor = contains(&b_bytes, b"renzora_native_plugin_ctor");

    println!("      handshake_a names handshake_b       {}", yes_no(imports_b));
    println!("      handshake_a exports the ctor        {}", yes_no(a_ctor));
    println!("      handshake_b exports the ctor        {}", yes_no(b_ctor));
    // What keeping the metadata costs. `prune_byproducts` is what the real
    // build runs and what a linkable plugin has to skip, so the difference is
    // the price of the whole idea.
    let pruned = scratch.join("handshake_b").join(format!("pruned.{ext}"));
    let _ = std::fs::copy(&b_out, &pruned);
    rustc::prune_byproducts(&pruned);
    let pruned_len = std::fs::metadata(&pruned).map(|m| m.len() as usize).unwrap_or(0);
    println!(
        "      handshake_b linkable / pruned       {} KB / {} KB  ({}% larger)",
        b_bytes.len() / 1024,
        pruned_len / 1024,
        if pruned_len > 0 { (b_bytes.len() * 100 / pruned_len).saturating_sub(100) } else { 0 }
    );

    println!(
        "\nCOMPILE + LINK: {}",
        if imports_b && a_ctor && b_ctor {
            "PASSED: a plugin can be used as a library by another plugin."
        } else {
            "INCONCLUSIVE: see the flags above."
        }
    );
    println!(
        "\nTo test the runtime half, copy both directories into <editor>/plugins/ and launch.\n\
         handshake_b must load first; see this example's module docs for why."
    );
    println!("sources + artifacts: {}", scratch.display());
}

/// One `rustc` invocation, with the SDK's flags plus any extra externs.
///
/// Deliberately **not** `Sdk::compile`: that one prunes `.rustc` afterwards, and
/// pruning B is what would make B unusable as a library. Everything else is the
/// same command line, reached through the same shared builder so this cannot
/// drift from what the engine really does.
fn build(
    rustc: &Path,
    sdk: &Sdk,
    sdk_root: &Path,
    dir: &Path,
    out: &Path,
    extra: &[(String, PathBuf)],
) -> Result<(), String> {
    let manifest = sdk.manifest();
    let name = dir.file_name().unwrap_or_default().to_string_lossy().replace('-', "_");
    let src = dir.join("src").join("lib.rs");
    let bevy = sdk_root.join(&manifest.r#extern.bevy);
    let renzora = sdk_root.join(&manifest.r#extern.renzora);
    let ember = manifest.r#extern.renzora_ember.as_ref().map(|e| sdk_root.join(e));
    let dependency: Vec<PathBuf> =
        manifest.link_search.dependency.iter().map(|d| sdk_root.join(d)).collect();
    let native: Vec<PathBuf> =
        manifest.link_search.native.iter().map(|n| sdk_root.join(n)).collect();

    let target = rustc::Target {
        triple: &manifest.triple,
        toolchain: &manifest.rustc,
        crate_name: &name,
        edition: "2021",
        extern_bevy: &bevy,
        extern_renzora: &renzora,
        extern_ember: ember.as_deref(),
        dependency: &dependency,
        native: &native,
        plugin_dir: dir,
        build_dir: dir,
        src: &src,
        out,
    };
    let mut args = rustc::args(&target)?;
    for (extern_name, path) in extra {
        args.push("--extern".to_string());
        args.push(format!("{extern_name}={}", path.display()));
    }

    let mut cmd = Command::new(rustc);
    cmd.env("CARGO_MANIFEST_DIR", dir);
    cmd.env("CARGO_PKG_NAME", &name);
    cmd.args(&args);
    let output = cmd.output().map_err(|e| format!("could not run rustc: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).to_string())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "NO" }
}

/// The two plugins, written out fresh each run.
fn write_sources(scratch: &Path) {
    // B owns a type of its own. Using a contract-crate type here would prove
    // nothing: every plugin already shares those through `renzora_dylib`.
    let b_lib = r#"//! Plugin B: owns a resource of its own and fills it in.
use bevy::prelude::*;

/// B's own type. NOT from the contract crate, which is the whole point:
/// if A can see this, plugins can share their own types.
#[derive(Resource, Default, Debug)]
pub struct Handshake {
    pub greetings: Vec<String>,
}

#[derive(Default)]
pub struct HandshakeBPlugin;

impl Plugin for HandshakeBPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Handshake>();
        app.add_systems(Startup, seed);
        info!("[handshake-b] plugin built");
    }
}

fn seed(mut handshake: ResMut<Handshake>) {
    handshake.greetings.push("hello from B".to_string());
    info!("[handshake-b] seeded the resource");
}

renzora::plugin!(HandshakeBPlugin, Runtime);
"#;

    // A names B's type at compile time. This line is the experiment.
    let a_lib = r#"//! Plugin A: reads a resource defined by plugin B.
use bevy::prelude::*;

// THE TEST. Without `--extern handshake_b`, this line does not compile.
use handshake_b::Handshake;

#[derive(Default)]
pub struct HandshakeAPlugin;

impl Plugin for HandshakeAPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, report);
        info!("[handshake-a] plugin built");
    }
}

/// Reads B's resource. Succeeding proves three things at once: A compiled
/// against B's type, the two agree on its `TypeId` at runtime, and both are
/// looking at one shared `World`.
fn report(handshake: Option<Res<Handshake>>, mut said: Local<bool>) {
    if *said {
        return;
    }
    let Some(handshake) = handshake else { return };
    info!("[handshake-a] read B's resource: {:?}", handshake.greetings);
    *said = true;
}

renzora::plugin!(HandshakeAPlugin, Runtime);
"#;

    for (name, lib) in [("handshake_b", b_lib), ("handshake_a", a_lib)] {
        let dir = scratch.join(name);
        std::fs::create_dir_all(dir.join("src")).expect("create plugin dir");
        std::fs::write(dir.join("src").join("lib.rs"), lib).expect("write lib.rs");
        // Bevy's derives read this to resolve their own crate paths.
        let deps = if name == "handshake_a" {
            // Declared so the intent is readable, and stripped before cargo ever
            // sees it: `handshake_b` is supplied by `--extern`, not resolved.
            "bevy = \"0.19\"\nrenzora = \"*\"\n"
        } else {
            "bevy = \"0.19\"\nrenzora = \"*\"\n"
        };
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
                 [lib]\ncrate-type = [\"dylib\"]\n\n[dependencies]\n{deps}"
            ),
        )
        .expect("write Cargo.toml");
    }
}
