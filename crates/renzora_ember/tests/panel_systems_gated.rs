//! Guard: a panel's systems must be registered through [`PanelScope`], so they
//! stop running when the panel isn't visible.
//!
//! # Why this is a test and not a convention
//!
//! It was a convention, and the convention lost. Surveying the editor found some
//! panel crates fully gated, several with *zero* gating, and no pattern beyond
//! who happened to think of it — 1283 of 1535 per-frame system registrations ran
//! unconditionally in an idle editor. Nobody skipped the gate on purpose; most
//! authors never knew it existed.
//!
//! `PanelScope` fixed the ergonomics (the gated path is the shortest one, and the
//! panel id is written once so it can't drift). This fixes the *drift*: a panel
//! added next year with a bare `app.add_systems` fails here, loudly, with the file
//! and line named — instead of quietly adding a few hundred µs nobody attributes
//! to it. Small per-panel costs are exactly the kind that accumulate invisibly
//! until the frame budget is gone.
//!
//! # What it checks
//!
//! In any file that calls `register_panel_content(`, every `app.add_systems(`
//! must be gated *somehow*: either chained off the registration as
//! `.systems(..)`, or carrying its own `panel_active(..)` run condition, or
//! marked with [`MARKER`]. It checks for **gating, not for a particular style** —
//! several panels correctly gate per-system with `panel_active` and should not be
//! churned into a different shape just to satisfy a lint.
//!
//! # Opting out
//!
//! The exemption lives **at the call site**, not in a list here:
//!
//! ```ignore
//! // panel-systems-ungated: poll_store drains in-flight async requests
//! app.add_systems(Update, (poll_store, ..));
//! ```
//!
//! Deliberately not a path list — a list rots the moment a file is renamed, and
//! it puts the justification where nobody looks while editing the code it
//! excuses. At the call site the reason travels with the code, is visible to
//! whoever is about to change it, and cannot go stale: delete the systems and the
//! marker goes with them.
//!
//! # What it does NOT check
//!
//! This is a text scan, so it can only see whether the author went through
//! `PanelScope` — not whether a given system actually needs gating, and not
//! systems registered elsewhere in the crate rather than beside the panel
//! registration. It guards the pattern against regressing; it is not a
//! completeness check for ungated work.

use std::path::{Path, PathBuf};

/// Call-site opt-out. Put it in the comment block directly above the
/// `app.add_systems(` call, followed by the reason.
const MARKER: &str = "panel-systems-ungated:";

/// Vendored third-party crates — never scanned.
const SKIP_DIRS: &[&str] = &[
    "bevy_firefly",
    "bevy_gaussian_splatting",
    "bevy_hanabi",
    "bevy_heavy",
    "bevy_hui",
    "bevy_silk",
    "bevy_oxr",
    "bevy_transform_interpolation",
    "avian",
    "polyanya",
    "bvh2d",
    "vleue_navigator",
    "glam_matrix_extras",
    "target",
    // This guard's own doc comments quote the very strings it searches for.
    "tests",
];

fn crates_root() -> PathBuf {
    // <repo>/crates/renzora_ember -> <repo>/crates
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("renzora_ember lives under <repo>/crates")
        .to_path_buf()
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_ref()) || name.starts_with('.') {
                continue;
            }
            collect_rs(&path, out);
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
}

/// The paren-balanced text of the `app.add_systems(` call starting at `start`,
/// so a per-system `panel_active(..)` inside the call counts as gating.
fn call_text(lines: &[&str], start: usize) -> String {
    let mut depth = 0i32;
    let mut out = String::new();
    for (n, line) in lines.iter().enumerate().skip(start).take(80) {
        out.push_str(line);
        out.push('\n');
        depth += line.matches('(').count() as i32 - line.matches(')').count() as i32;
        if n > start && depth <= 0 {
            break;
        }
    }
    out
}

/// Is `line_idx` preceded by a comment block containing [`MARKER`]?
///
/// Walks back over contiguous comment and blank lines only, so the marker has to
/// belong to *this* call — a comment further up, attached to something else,
/// can't accidentally license it.
fn has_marker_above(lines: &[&str], line_idx: usize) -> bool {
    for i in (0..line_idx).rev() {
        let t = lines[i].trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("//") {
            if rest.contains(MARKER) {
                return true;
            }
            continue;
        }
        break; // hit real code — the comment block is over
    }
    false
}

#[test]
fn panel_systems_go_through_panel_scope() {
    let root = crates_root();
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    assert!(
        !files.is_empty(),
        "found no .rs files under {} — the path logic is wrong, not the codebase",
        root.display()
    );

    let mut offenders: Vec<String> = Vec::new();
    for path in &files {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        if !src.contains("register_panel_content(") {
            continue;
        }
        // The framework file that *defines* the API isn't a panel.
        if src.contains("fn register_panel_content<F>") {
            continue;
        }
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains("app.add_systems(") || has_marker_above(&lines, i) {
                continue;
            }
            // Already gated per-system in the older style — correct, just not
            // `PanelScope`. Not worth churning.
            if call_text(&lines, i).contains("panel_active") {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            offenders.push(format!("  {rel}:{}", i + 1));
        }
    }

    assert!(
        offenders.is_empty(),
        "\n\nThese panels add systems with a bare `app.add_systems`, so those systems \
         keep running while the panel is hidden:\n\n{}\n\n\
         Chain off the registration instead — the id is then written once and can't \
         drift:\n\n\
         \x20   app.register_panel_content(ID, scroll, build)\n\
         \x20       .systems(Update, (..));   // skipped while the panel isn't visible\n\n\
         If they genuinely must keep running while it's hidden (background loads, \
         autosave, cleanup that has to observe a despawn), use `.always(..)`, or \
         mark the call:\n\n\
         \x20   // {MARKER} <why>\n\
         \x20   app.add_systems(..);\n",
        offenders.join("\n"),
    );
}
