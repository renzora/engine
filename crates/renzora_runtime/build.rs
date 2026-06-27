//! Auto-generates the dylib keepalive `pub use` list from this crate's own
//! Cargo.toml. Each `pub use renzora_foo;` in lib.rs causes cargo to emit a
//! real dep edge so the plugin dylib ends up in the final binary's DT_NEEDED —
//! and therefore gets loaded at startup, which is when its `inventory::submit!`
//! ctors run and register entries into the shared plugin registry.
//!
//! Engine (Runtime-scope) plugins only: every non-optional `renzora_*`
//! dependency. The editor is the separate `renzora_editor` bundle dll, so there
//! is no editor keepalive here.

use std::collections::BTreeMap;
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

    // Map each optional dep to the feature that enables it (`dep:renzora_x` in a
    // feature's list). The lean exporter strips a subsystem by removing its
    // feature from `default`; gating its keepalive `pub use` on that feature is
    // what makes the subsystem drop out of the build cleanly.
    let mut dep_feature: BTreeMap<&str, &str> = BTreeMap::new();
    if let Some(features) = toml.get("features").and_then(|v| v.as_table()) {
        for (fname, list) in features {
            if let Some(arr) = list.as_array() {
                for item in arr.iter().filter_map(|v| v.as_str()) {
                    if let Some(dep) = item.strip_prefix("dep:") {
                        dep_feature.insert(dep, fname.as_str());
                    }
                }
            }
        }
    }

    // Engine plugins = `renzora_*` deps. Each `pub use renzora_foo;` emits a real
    // dep edge so the plugin ends up in the binary's link (DT_NEEDED), which is
    // when its `inventory::submit!` ctors run. Non-optional deps are always
    // re-exported; optional ones only when their feature is enabled.
    let mut entries: BTreeMap<&str, String> = BTreeMap::new();
    for (name, v) in deps {
        if !name.starts_with("renzora_") || name == "renzora_runtime" {
            continue;
        }
        let optional = v
            .as_table()
            .and_then(|t| t.get("optional"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let line = if optional {
            match dep_feature.get(name.as_str()) {
                Some(feat) => format!("#[cfg(feature = \"{feat}\")]\npub use {name};\n"),
                // Optional but no feature gates it — can't be enabled; skip.
                None => continue,
            }
        } else {
            format!("pub use {name};\n")
        };
        entries.insert(name.as_str(), line);
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let src: String = entries.into_values().collect();
    fs::write(Path::new(&out_dir).join("engine_reexports.rs"), src)
        .expect("write engine_reexports.rs");
}
