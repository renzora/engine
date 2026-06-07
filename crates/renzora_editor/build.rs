//! Dylib keepalive for the editor bundle. Mirrors `renzora_runtime/build.rs`:
//! emits a `pub use renzora_<crate>;` for every `renzora_*` dependency so the
//! linker keeps each rlib in the cdylib's link graph and its life-before-main
//! `inventory::submit!` ctor actually runs (instead of being dead-stripped as
//! unreferenced, which would leave the bundle replaying an empty inventory).
//!
//! The list is generated from this crate's own Cargo.toml — adding a bundled
//! editor crate only requires editing Cargo.toml, never lib.rs. The generated
//! `pub use`s cover BOTH the optional editor-only crates (active under the
//! `editor` feature) and the non-optional dual-mode crates whose `/editor`
//! code carries Editor-scope plugins. lib.rs gates the `include!` on the
//! `editor` feature, so the optional-crate `pub use`s only compile when those
//! deps are actually active.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");

    let manifest = fs::read_to_string("Cargo.toml").expect("read Cargo.toml");
    let toml: toml::Value = toml::from_str(&manifest).expect("parse Cargo.toml");

    let deps = toml
        .get("dependencies")
        .and_then(|v| v.as_table())
        .expect("[dependencies] table");

    // Every `renzora_*` dependency is a crate whose `inventory::submit!` ctors
    // must be kept alive. `starts_with("renzora_")` already excludes the plain
    // `renzora` shared dylib (the macro host, not a plugin). The self-exclusion
    // is belt-and-suspenders — the bundle never depends on itself.
    let mut crates: Vec<&str> = deps
        .keys()
        .map(String::as_str)
        .filter(|name| name.starts_with("renzora_") && *name != "renzora_editor")
        .collect();
    crates.sort();
    crates.dedup();

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let src = crates
        .iter()
        .map(|n| format!("pub use {};\n", n))
        .collect::<String>();
    fs::write(Path::new(&out_dir).join("bundle_reexports.rs"), src)
        .expect("write bundle_reexports.rs");
}
