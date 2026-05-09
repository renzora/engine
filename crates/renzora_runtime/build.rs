//! Auto-generates the dylib keepalive `pub use` lists from this crate's
//! own Cargo.toml. Each `pub use renzora_foo;` in lib.rs causes cargo to
//! emit a real dep edge so the plugin dylib ends up in the final
//! binary's DT_NEEDED — and therefore gets loaded at startup, which is
//! when its `inventory::submit!` ctors run and register entries into
//! the shared plugin registry.
//!
//! Engine plugins: every non-optional `renzora_*` dependency.
//! Editor plugins: every `dep:renzora_*` entry under the `editor` feature.

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

    // Engine plugins: non-optional deps named `renzora_*`. Skip the runtime
    // crate itself if it's somehow listed.
    let mut engine: Vec<&str> = deps
        .iter()
        .filter(|(name, _)| name.starts_with("renzora_") && name.as_str() != "renzora_runtime")
        .filter(|(_, v)| {
            !v.as_table()
                .and_then(|t| t.get("optional"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .map(|(name, _)| name.as_str())
        .collect();
    engine.sort();

    // Editor plugins: walk the `features.editor` array, pick up `dep:foo`
    // entries that target a `renzora_*` crate.
    let editor_feat = toml
        .get("features")
        .and_then(|f| f.get("editor"))
        .and_then(|e| e.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let mut editor: Vec<&str> = editor_feat
        .iter()
        .filter_map(|v| v.as_str())
        .filter_map(|s| s.strip_prefix("dep:"))
        .filter(|s| s.starts_with("renzora_"))
        .collect();
    editor.sort();
    editor.dedup();

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");

    let engine_src = engine
        .iter()
        .map(|n| format!("pub use {};\n", n))
        .collect::<String>();
    fs::write(Path::new(&out_dir).join("engine_reexports.rs"), engine_src)
        .expect("write engine_reexports.rs");

    let editor_src = editor
        .iter()
        .map(|n| format!("pub use {};\n", n))
        .collect::<String>();
    fs::write(Path::new(&out_dir).join("editor_reexports.rs"), editor_src)
        .expect("write editor_reexports.rs");
}
