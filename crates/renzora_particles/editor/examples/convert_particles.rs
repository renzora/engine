//! Convert every `.particle` under a directory to a sibling `.vfx`.
//!
//! ```sh
//! cargo run --profile dist -p renzora_particles_editor --example convert_particles -- <dir>
//! ```
//!
//! Walks `<dir>` and its subfolders. A `.particle` whose `<name>.vfx` already
//! exists is skipped, and the summary says so: a batch run over a whole project
//! must never touch an effect someone may have edited since, and unlike the
//! editor's single-file convert there is nobody to show a second copy to. The
//! `.particle` files are left exactly as they are.
//!
//! Exits non-zero when any file failed to convert.

use std::path::{Path, PathBuf};

use renzora_particles_editor::convert;

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out)?;
        } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("particle")) {
            out.push(p);
        }
    }
    Ok(())
}

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: convert_particles <dir>");
        std::process::exit(2);
    };
    let mut files = Vec::new();
    if let Err(e) = walk(&dir, &mut files) {
        eprintln!("cannot read {}: {e}", dir.display());
        std::process::exit(2);
    }
    let rel = |p: &Path| p.strip_prefix(&dir).unwrap_or(p).display().to_string();

    let (mut converted, mut skipped, mut failed, mut warned) = (0usize, 0usize, 0usize, 0usize);
    for particle in &files {
        let target = convert::sibling_vfx(particle);
        if target.exists() {
            println!("skipped    {}  ({} already exists)", rel(particle), rel(&target));
            skipped += 1;
            continue;
        }
        let result = convert::convert_file(particle).and_then(|c| {
            let name = particle.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let text = convert::vfx_text(&c, &name)?;
            std::fs::write(&target, text).map_err(|e| format!("could not write {}: {e}", target.display()))?;
            Ok(c)
        });
        match result {
            Ok(c) => {
                converted += 1;
                if c.warnings.is_empty() {
                    println!("converted  {} -> {}", rel(particle), rel(&target));
                } else {
                    warned += 1;
                    println!("converted  {} -> {}  ({} warning(s))", rel(particle), rel(&target), c.warnings.len());
                    for w in &c.warnings {
                        println!("             - {w}");
                    }
                }
            }
            Err(e) => {
                failed += 1;
                println!("FAILED     {}: {e}", rel(particle));
            }
        }
    }
    println!();
    println!(
        "{} .particle file(s): {converted} converted ({warned} with warnings), {skipped} skipped, {failed} failed",
        files.len()
    );
    if failed > 0 {
        std::process::exit(1);
    }
}
