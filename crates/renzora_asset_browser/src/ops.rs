//! File operations and the small lookups the widgets ask for: create, delete,
//! duplicate, favorite, reveal-in-explorer, plus the folder/type colour tables
//! and the two resources this panel publishes for other crates.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora_editor_framework::EditorCommands;
use renzora_ember::reactive::Rx;

use crate::interact::open_file;
use crate::state::{
    save_list, unique_path, CrumbNav, NativeAssets, NewAsset, ShortcutClick,
};

/// Create a new asset (folder or file) in the current folder + select it.
pub(crate) fn create_asset(world: &mut World, kind: NewAsset) {
    let folder = world
        .get_resource::<NativeAssets>()
        .and_then(|s| s.current.clone())
        .or_else(|| {
            world
                .get_resource::<renzora::core::CurrentProject>()
                .map(|p| p.path.clone())
        });
    let Some(folder) = folder else {
        return;
    };
    let boilerplate = world
        .get_resource::<renzora_editor_framework::EditorSettings>()
        .is_none_or(|s| s.new_file_boilerplate);
    let path = unique_path(&folder, kind.filename(), kind.is_folder());
    let ok = if kind.is_folder() {
        std::fs::create_dir_all(&path).is_ok()
    } else {
        std::fs::write(&path, kind.content(boilerplate)).is_ok()
    };
    if ok {
        if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
            s.selected = Some(path);
            s.listing_dirty = true;
        }
    }
}

pub(crate) fn toggle_favorite(world: &mut World, path: &Path) {
    let root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.path.clone());
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        if let Some(i) = s.favorites.iter().position(|f| f == path) {
            s.favorites.remove(i);
        } else {
            s.favorites.push(path.to_path_buf());
        }
        if let Some(root) = root {
            save_list(&root, "favorites", &s.favorites);
        }
    }
}

pub(crate) fn delete_asset(world: &mut World, path: &Path) {
    let _ = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        s.listing_dirty = true;
        s.selection.remove(path);
        if s.selected.as_deref() == Some(path) {
            s.selected = None;
        }
    }
}

/// Copy a file (or directory tree) next to itself with a " copy" suffix.
pub(crate) fn duplicate_asset(path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("copy");
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let is_dir = path.is_dir();
    let dest = unique_path(parent, &format!("{stem} copy{ext}"), is_dir);
    if is_dir {
        let _ = copy_dir_recursive(path, &dest);
    } else {
        let _ = std::fs::copy(path, &dest);
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Open the OS file manager at `path` (selecting it where supported).
pub(crate) fn reveal_in_explorer(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg("/select,").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = path.parent().unwrap_or(path);
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

pub(crate) fn project_root(w: &Rx) -> Option<PathBuf> {
    w.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone())
}

/// Republish the current folder as a project-relative, forward-slashed path
/// (`""` = project root) into [`AssetBrowserCwd`](renzora::core::AssetBrowserCwd)
/// so the importer's drop handler targets it. Mirrors the relative-path
/// computation in `import_click`.
pub(crate) fn publish_cwd(
    mut cwd: ResMut<renzora::core::AssetBrowserCwd>,
    state: Res<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let val = project.as_ref().map(|project| {
        let folder = state.current.clone().unwrap_or_else(|| project.path.clone());
        folder
            .strip_prefix(&project.path)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default()
    });
    if cwd.0 != val {
        cwd.0 = val;
    }
}

/// The folder being shown (the explicit nav target, else the project root).
pub(crate) fn current_folder(w: &Rx) -> Option<PathBuf> {
    w.get_resource::<NativeAssets>()
        .and_then(|s| s.current.clone())
        .or_else(|| project_root(w))
}

/// Accent color for a folder's icon, by well-known name (ported from the egui
/// browser's `folder_icon_color`).
pub(crate) fn folder_color(name: &str) -> (u8, u8, u8) {
    match name.to_lowercase().as_str() {
        "assets" => (255, 210, 100),
        "scenes" | "blueprints" => (100, 180, 255),
        "scripts" => (130, 230, 180),
        "materials" => (255, 130, 200),
        "textures" | "images" => (150, 230, 130),
        "models" | "meshes" => (255, 170, 100),
        "audio" | "sounds" | "music" => (200, 130, 230),
        "prefabs" => (130, 180, 255),
        "src" => (255, 130, 80),
        "shaders" => (180, 130, 255),
        _ => (170, 175, 190),
    }
}

/// Accent color + human-readable type label for a file, by extension. Drives the
/// tile's type subtitle and bottom accent strip. Folders are handled separately.
///
/// The table itself lives in `renzora_ember::file_kind` — the folder picker
/// lists files now and needs the same answers, and it cannot reach a crate that
/// depends on it. Two tables would have drifted the first time an extension was
/// added to one of them.
pub(crate) fn asset_type_info(path: &Path) -> ((u8, u8, u8), &'static str) {
    renzora_ember::file_kind::type_info(path)
}

pub(crate) fn icon_for(path: &Path, is_dir: bool) -> &'static str {
    renzora_ember::file_kind::icon_for(path, is_dir)
}

/// A favorites/recent shortcut row: navigate (folder) or open (file).
pub(crate) fn shortcut_click(
    q: Query<(&Interaction, &ShortcutClick), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
    cmds: Option<Res<EditorCommands>>,
) {
    for (interaction, shortcut) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if shortcut.is_dir {
            state.current = Some(shortcut.path.clone());
            state.selected = None;
        } else {
            open_file(&cmds, &shortcut.path);
            let root = project.as_ref().map(|p| p.path.clone());
            track_recent(&mut state, &shortcut.path, root.as_deref());
        }
    }
}

/// Move `path` to the front of the recent list (max 20) + persist.
pub(crate) fn track_recent(state: &mut NativeAssets, path: &Path, root: Option<&Path>) {
    state.recent.retain(|p| p != path);
    state.recent.insert(0, path.to_path_buf());
    state.recent.truncate(20);
    if let Some(root) = root {
        save_list(root, "recent", &state.recent);
    }
}

pub(crate) fn crumb_click(
    q: Query<(&Interaction, &CrumbNav), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, nav) in &q {
        if *interaction == Interaction::Pressed {
            state.current = Some(nav.0.clone());
            state.selected = None;
        }
    }
}
