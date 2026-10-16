//! Keep the paths stored inside project files pointing at an asset after it
//! moves.
//!
//! [`renzora::core::AssetPathChanged`] already had subscribers, but every one
//! of them patches something live: a component on a scene entity, the
//! project's scene list, an open tab. Nothing followed the paths written into
//! files on disk, and that is where most references live. An imported model's
//! `.material` files name each texture by its full project path, so importing
//! into the wrong folder and dragging the model out left every material
//! pointing at textures in the folder it had just left. The model then loaded
//! untextured, from a fresh drop or from any scene, with nothing on screen to
//! say why, only a wall of "Path not found" in the log.
//!
//! This observer rewrites those files, whoever fired the event: the asset
//! browser's drag-to-folder and rename, or a scene renamed from its tab.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use renzora::core::project_files::{is_ignored, SelfWrites};
use renzora::core::{AssetPathChanged, CurrentProject};

/// How deep the walk goes. Far past any real project, and a stop for a
/// symlink loop.
const MAX_DEPTH: usize = 32;

/// Extensions of the text formats that store project paths.
///
/// Scripts are deliberately absent. A path in code may be built, compared or
/// commented on, and rewriting code behind its author is a different promise
/// from rewriting a data file the editor itself wrote.
const REFERENCING_EXTENSIONS: &[&str] = &[
    "material",
    "material_bp",
    "material_function",
    "bsn",
    "particle",
    "html",
    "anim",
    "blueprint",
    "bp",
];

pub(crate) fn rewrite_project_references(
    trigger: On<AssetPathChanged>,
    project: Option<Res<CurrentProject>>,
    mut commands: Commands,
) {
    let Some(project) = project else {
        return;
    };
    let ev = trigger.event();
    let mut rewritten: Vec<(PathBuf, String)> = Vec::new();

    for path in referencing_files(&project.path) {
        // Not valid UTF-8 is a binary file wearing a text extension (an `.anim`
        // can be either), and nothing in it is ours to rewrite.
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(updated) = ev.rewrite_text(&text) else {
            continue;
        };
        if let Err(e) = std::fs::write(&path, &updated) {
            warn!("[asset-move] could not update {}: {e}", path.display());
            continue;
        }
        rewritten.push((path, updated));
    }

    if rewritten.is_empty() {
        return;
    }
    info!(
        "[asset-move] '{}' -> '{}': updated references in {} file(s)",
        ev.old,
        ev.new,
        rewritten.len()
    );
    renzora::console_log::console_info(
        "Assets",
        format!(
            "Moved {}: updated the paths in {} file(s) that referenced it",
            ev.new,
            rewritten.len()
        ),
    );

    // Claim the scene writes. The open scene's entities were already patched in
    // memory by the live subscribers, so the watcher event this write causes is
    // our own echo and must not reload the scene (or, with unsaved changes,
    // raise a conflict over it). Everything else is left to look external on
    // purpose: a rewritten `.material` or `.particle` should reload.
    commands.queue(move |world: &mut World| {
        let mut self_writes = world.get_resource_or_insert_with(SelfWrites::default);
        for (path, text) in &rewritten {
            if path.extension().and_then(|e| e.to_str()) == Some("bsn") {
                self_writes.record(path, text.as_bytes());
            }
        }
    });
}

/// Every file under `root` in one of [`REFERENCING_EXTENSIONS`], skipping the
/// directories the project watcher ignores.
fn referencing_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if is_ignored(&relative) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if depth + 1 < MAX_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if file_type.is_file() && has_referencing_extension(&path) {
                out.push(path);
            }
        }
    }
    out
}

fn has_referencing_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| REFERENCING_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_finds_nested_materials_and_skips_ignored_dirs() {
        let root = std::env::temp_dir().join(format!("renzora_refs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("models/bistro/materials")).unwrap();
        std::fs::create_dir_all(root.join(".renzora/cache")).unwrap();
        std::fs::write(root.join("models/bistro/materials/a.material"), "{}").unwrap();
        std::fs::write(root.join("models/bistro/bistro.glb"), "").unwrap();
        std::fs::write(root.join(".renzora/cache/b.material"), "{}").unwrap();
        std::fs::write(root.join("main.BSN"), "").unwrap();

        let mut found: Vec<String> = referencing_files(&root)
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        found.sort();
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(found, ["main.BSN", "models/bistro/materials/a.material"]);
    }
}
