//! Parse every shader effect under a directory, the way the engine reads one,
//! and report each: its name, its sliders, or why it would not load.
//!
//! ```text
//! cargo run --profile dist -p renzora_postprocess --example check_effects -- <dir>
//! ```
//!
//! For the shader effects published to the marketplace: an effect that fails
//! here shows a red error in the Post Process inspector instead of drawing,
//! and this is the check that catches it before anyone installs it. It parses
//! only; a shader that parses and then renders wrongly needs to be looked at.
//! Exits non-zero when any file fails.

use std::path::{Path, PathBuf};

use renzora_postprocess::effect_file::{self, FieldRole};

fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("usage: check_effects <dir>");
        std::process::exit(2);
    };
    let mut files = Vec::new();
    collect(Path::new(&root), &mut files);
    files.sort();

    let mut failed = 0;
    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                println!("FAIL {}: {e}", path.display());
                failed += 1;
                continue;
            }
        };
        if !effect_file::is_effect(&source) {
            println!("FAIL {}: no // @effect header", path.display());
            failed += 1;
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        match effect_file::parse(&source, &effect_file::title_case(stem)) {
            Ok(info) => {
                let sliders: Vec<String> = info
                    .fields
                    .iter()
                    .map(|f| match f.role {
                        FieldRole::Slider { min, max, .. } => {
                            format!("{} [{min}..{max}] = {}", f.label, f.default)
                        }
                        FieldRole::Hidden => format!("({} = {})", f.name, f.default),
                        FieldRole::Time => "(time)".into(),
                        FieldRole::Enabled => "(enabled)".into(),
                    })
                    .collect();
                let snapshot = if info.snapshot { " snapshot" } else { "" };
                println!(
                    "ok   {:<28} {}{snapshot}: {}",
                    info.name,
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    sliders.join(", ")
                );
            }
            Err(e) => {
                println!("FAIL {}: {e}", path.display());
                failed += 1;
            }
        }
    }
    println!("\n{} of {} effects parse", files.len() - failed, files.len());
    if failed > 0 {
        std::process::exit(1);
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e == "wesl" || e == "wgsl")
        {
            out.push(path);
        }
    }
}
