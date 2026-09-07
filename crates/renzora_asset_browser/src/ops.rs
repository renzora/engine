//! File operations and the small lookups the widgets ask for: create, delete,
//! duplicate, favorite, reveal-in-explorer, plus the folder/type colour tables
//! and the two resources this panel publishes for other crates.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora_editor_framework::EditorCommands;
use renzora_ember::reactive::Rx;

use crate::interact::open_file;
use crate::state::{
    save_list, unique_path, AssetTile, CrumbNav, NativeAssets, NewAsset, ShortcutClick, TreeNav,
};

/// Create a new asset (folder or file) in the current folder + select it.
///
/// A new **folder** opens its rename field immediately, with the placeholder
/// name selected. Every OS file manager does this, and the reason is that
/// `New Folder` never produces the folder you wanted: the name is the entire
/// point of the thing you just made, so leaving `New Folder` sitting there just
/// means a second gesture to fix it. The other kinds don't, because a `.lua` or
/// a `.material` is opened and edited straight after creating it and a rename
/// field in the way is one more thing to dismiss.
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
        // A new *folder* goes straight into its rename; a new file does not.
        // The name is the whole point of a folder, where a file arrives with an
        // extension the field would have to be careful of and a template that
        // already says what it is.
        let start_naming = kind.is_folder();
        if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
            s.selected = Some(path.clone());
            s.listing_dirty = true;
            if start_naming {
                // The narrow layout has no grid to draw the field in.
                let surface = if s.narrow {
                    crate::state::RenameSurface::Tree
                } else {
                    crate::state::RenameSurface::Grid
                };
                s.begin_rename(&path, surface);
            }
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
/// Show `path` in the OS file manager.
///
/// **A folder opens; a file is revealed inside its folder.** The two are not the
/// same gesture, and treating them the same is what made this wrong: every
/// platform arm reached for the *parent*, so right-clicking `plugins` in a
/// project at `~/Documents/hello` opened `hello`, and Reveal on the empty grid
/// (which passes the folder you are looking at) opened `~/Documents` — one level
/// above the project, every time.
///
/// Selecting a folder inside its parent is technically "revealing" it, but
/// nobody asking to see a folder in their file manager means "show me the folder
/// next to its siblings". They mean open it.
pub(crate) fn reveal_in_explorer(path: &Path) {
    let is_dir = path.is_dir();
    #[cfg(target_os = "windows")]
    {
        if is_dir {
            let _ = std::process::Command::new("explorer").arg(path).spawn();
        } else {
            // No space after the comma, and one argument: `explorer` parses
            // `/select,<path>` as a single token.
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn();
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if !is_dir {
            cmd.arg("-R");
        }
        let _ = cmd.arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // `xdg-open` has no "select this file" mode: handed a file it *launches*
        // it in the default app, which is the one thing Reveal must not do. So a
        // file opens its containing folder, without the file selected in it.
        let target = if is_dir {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
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
    hovering: Option<Res<renzora::core::FileDragHovering>>,
    tiles: Query<(&Interaction, &AssetTile)>,
    tree: Query<(&Interaction, &TreeNav)>,
    crumbs: Query<(&Interaction, &CrumbNav)>,
) {
    // While an OS file drag is over the window, the drop should land wherever
    // the cursor is -- a tree row, a breadcrumb, a folder tile -- rather than
    // always in the folder that happens to be open. Falling back to the open
    // folder is what makes "drop on the empty grid" mean "drop in here".
    //
    // This is the same set of targets an *internal* drag uses (`drop_folder` in
    // `drag_drop`), deliberately: one gesture, one set of places it can land.
    let dragging = hovering.is_some_and(|h| h.0);
    let hovered = dragging.then(|| hovered_folder(&tiles, &tree, &crumbs)).flatten();

    let val = project.as_ref().map(|project| {
        let folder = hovered
            .or_else(|| state.current.clone())
            .unwrap_or_else(|| project.path.clone());
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

/// The folder under the cursor, from any of the three places one is shown.
///
/// A copy of `drag_drop::drop_folder` in spirit but not in code, because that
/// one runs on the release frame of an internal drag and accepts `Pressed` for
/// it; an OS drag never presses anything, so this takes `Hovered` alone.
fn hovered_folder(
    tiles: &Query<(&Interaction, &AssetTile)>,
    tree: &Query<(&Interaction, &TreeNav)>,
    crumbs: &Query<(&Interaction, &CrumbNav)>,
) -> Option<PathBuf> {
    if let Some((_, tile)) = tiles.iter().find(|(i, t)| t.is_dir && **i == Interaction::Hovered) {
        return Some(tile.path.clone());
    }
    if let Some((_, nav)) = tree.iter().find(|(i, _)| **i == Interaction::Hovered) {
        return Some(nav.0.clone());
    }
    crumbs
        .iter()
        .find(|(i, _)| **i == Interaction::Hovered)
        .map(|(_, c)| c.0.clone())
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
        // Matches the `.anim` accent in `renzora_ember::file_kind`, so a folder
        // of clips is the same teal as the clips inside it.
        "animations" | "animation" | "anims" => (90, 215, 205),
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

// ── Paste from the system clipboard ─────────────────────────────────────────

/// Copy files and folders that were copied in the OS file manager into the
/// folder the browser is showing.
///
/// # What the clipboard actually gives us
///
/// A file manager advertises a copy under several targets at once:
/// `text/uri-list`, a desktop-specific one (`x-special/gnome-copied-files`), and
/// usually `text/plain` carrying the same URIs. `arboard` reads text, so this
/// takes the text target and parses it. That covers the common case and needs no
/// new dependency, at the cost of the one case it cannot cover: a file manager
/// that publishes *only* `text/uri-list` and no plain text. There is nothing to
/// paste there, and this reports that rather than failing silently.
///
/// Paths are copied, never moved. The clipboard says nothing about whether the
/// user chose copy or cut, and guessing wrong destroys the original.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn paste_from_clipboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
    input_focus: Option<Res<renzora::core::InputFocusState>>,
) {
    let ctrl = keyboard.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    if !ctrl || !keyboard.just_pressed(KeyCode::KeyV) {
        return;
    }
    // A rename field or any other text input owns Ctrl+V while it has focus.
    if state.renaming.is_some() || input_focus.is_some_and(|f| f.ui_wants_keyboard) {
        return;
    }

    let Some(root) = project.map(|p| p.path.clone()) else {
        return;
    };
    let dest = state.current.clone().unwrap_or_else(|| root.clone());

    let Some(text) = renzora_ember::widgets::clipboard::get_text() else {
        return;
    };
    let sources = clipboard_paths(&text);
    if sources.is_empty() {
        return;
    }

    let mut pasted = 0usize;
    for source in &sources {
        let Some(name) = source.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let is_dir = source.is_dir();
        // Copying a folder into itself or its own descendant would recurse until
        // the disk filled.
        if is_dir && dest.starts_with(source) {
            continue;
        }
        let target = unique_path(&dest, name, is_dir);
        let ok = if is_dir {
            copy_dir_recursive(source, &target).is_ok()
        } else {
            std::fs::copy(source, &target).is_ok()
        };
        if ok {
            pasted += 1;
        }
    }
    if pasted > 0 {
        state.listing_dirty = true;
    }
}

/// The existing paths named by a clipboard's text payload.
///
/// Accepts `file://` URIs and bare absolute paths, one per line, and ignores the
/// `copy`/`cut` verb some desktops put on the first line. Anything that does not
/// resolve to something on disk is dropped, which is what makes this safe to run
/// against ordinary copied text: a paste of prose simply finds no paths.
#[cfg(not(target_arch = "wasm32"))]
fn clipboard_paths(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "copy" && *l != "cut")
        .filter_map(|line| {
            let raw = line.strip_prefix("file://").unwrap_or(line);
            let decoded = percent_decode(raw);
            let path = PathBuf::from(decoded);
            path.is_absolute().then_some(path)
        })
        .filter(|p| p.exists())
        .collect()
}

/// Decode `%XX` escapes. A URI from a file manager percent-encodes spaces and
/// anything non-ASCII, so without this every path with a space in it is dropped
/// as "does not exist".
#[cfg(not(target_arch = "wasm32"))]
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    #[test]
    fn a_space_in_a_path_survives_the_uri_encoding() {
        assert_eq!(percent_decode("/home/a%20b/c%2Ed"), "/home/a b/c.d");
    }

    /// The desktop-specific payload leads with the verb, which is not a path.
    #[test]
    fn the_copy_verb_is_not_mistaken_for_a_path() {
        let paths = clipboard_paths("copy\nfile:///definitely/not/here");
        assert!(paths.is_empty());
    }

    /// Ordinary copied text must not be read as a paste of files.
    #[test]
    fn prose_yields_no_paths() {
        assert!(clipboard_paths("the quick brown fox\njumped over").is_empty());
    }

    /// Both the URI form and a bare absolute path resolve, and only if they
    /// exist: this is what keeps a stale clipboard from creating empty files.
    #[test]
    fn only_existing_absolute_paths_are_accepted() {
        let dir = std::env::temp_dir();
        let probe = dir.join("renzora-paste-probe.txt");
        std::fs::write(&probe, b"x").unwrap();
        let uri = format!("file://{}", probe.display());
        assert_eq!(clipboard_paths(&uri), vec![probe.clone()]);
        assert_eq!(clipboard_paths(&probe.display().to_string()), vec![probe.clone()]);
        assert!(clipboard_paths("relative/path").is_empty());
        let _ = std::fs::remove_file(&probe);
    }
}
