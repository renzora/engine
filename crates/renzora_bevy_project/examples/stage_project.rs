//! Stage a Bevy project's generated crate root, and say what was read out of it.
//!
//! ```sh
//! cargo run --profile dist -p renzora_bevy_project --example stage_project -- <path>
//! cargo run --profile dist -p renzora_bevy_project --example stage_project -- <path> --sdk dist/windows-x64/sdk
//! ```
//!
//! Without `--sdk` it stops after writing `.renzora/bevy/`. That is the half of
//! the loader that had to *guess*: which file is the crate root, which `mod`
//! declarations to redirect, which part of `fn main` builds the `App`. It
//! is therefore the half worth looking at first when a project does not load.
//! Everything after it is the same `rustc` invocation a native plugin takes.
//!
//! With `--sdk` it compiles too, which is the end-to-end check: a project that
//! gets through this will load in the editor, because the editor does exactly
//! this and then `dlopen`s the result.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(arg) = args.first() else {
        eprintln!("usage: stage_project <path to a bevy crate> [--sdk <dir>]");
        std::process::exit(2);
    };
    let sdk_dir = args
        .iter()
        .position(|a| a == "--sdk")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);
    let dir = std::path::PathBuf::from(arg);

    let Some(krate) = renzora::core::bevy_project::inspect(&dir) else {
        eprintln!(
            "{} is not a Bevy crate: no Cargo.toml with a `bevy` dependency (or a workspace \
             whose game member could not be picked; see `[package.metadata.renzora] member`)",
            dir.display()
        );
        std::process::exit(1);
    };

    println!("package      {}", krate.package);
    println!("crate name   {}", krate.crate_name());
    println!("edition      {}", krate.edition);
    println!("bevy         {}", krate.bevy_req.as_deref().unwrap_or("(path or git)"));
    println!(
        "crate root   {} ({})",
        krate.root.display(),
        if krate.root_is_lib { "library" } else { "binary" }
    );
    if let Some(bin) = &krate.bin {
        println!("binary       {}", bin.display());
    }
    println!("build.rs     {}", if krate.has_build_script { "yes" } else { "no" });
    println!("deps         {}", krate.dependencies.len());

    let entry = match renzora_bevy_project::entry::stage(&krate, &dir) {
        Ok(entry) => entry,
        Err(e) => {
            eprintln!("\ncould not stage this project:\n{e}");
            std::process::exit(1);
        }
    };
    println!("\nstaged       {}", entry.dir.display());
    for note in &entry.notes {
        println!("note         {note}");
    }

    let Some(sdk_dir) = sdk_dir else {
        println!("\nPass --sdk <dir> to compile it as well.");
        return;
    };

    let sdk = match renzora_plugin_build::Sdk::load(&sdk_dir) {
        Ok(sdk) => sdk,
        Err(e) => {
            eprintln!("\nno usable SDK at {}: {e}", sdk_dir.display());
            std::process::exit(1);
        }
    };
    let out = entry.dir.join(format!("{}.{}", krate.crate_name(), sdk.manifest().lib_ext));
    println!("\ncompiling    {} -> {}", krate.package, out.display());

    // The editor's own compile path, not a second one that might behave
    // differently. This tool exists to explain why a project will not load, so
    // it has to fail the same way the editor does, including the retry that
    // drops the component-label table.
    //
    // Streamed rather than collected: a Bevy crate's compile is long enough that
    // a silent wait reads as a hang, and the first error usually arrives well
    // before the end.
    let mut notes = Vec::new();
    match renzora_bevy_project::compile(
        &sdk,
        &krate,
        &dir,
        &entry.dir,
        &out,
        &mut notes,
        &mut |line| eprintln!("  {line}"),
    ) {
        Ok(stamp) => {
            for note in &notes {
                println!("note         {note}");
            }
            println!("\nbuilt        {} (stamp {stamp})", out.display());
        }
        Err(e) => {
            eprintln!("\nrustc rejected this project:\n{e}");
            std::process::exit(1);
        }
    }
}
