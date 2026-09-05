//! Click handling and file opening: the grid tile, the tree row, the tree tabs,
//! the back button, the two search boxes, the thumbnail requests for whatever is
//! on screen, and the routing that decides whether a double-clicked file belongs
//! to an in-editor editor or to the OS default app.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora_editor_framework::{
    EditorCommands, MaterialThumbnailRegistry, ModelThumbnailRegistry, SceneThumbnailRegistry,
};
use renzora_ember::widgets::EmberTextInput;

use crate::ops::track_recent;
use crate::state::{
    thumb_kind, AssetBack, AssetNameLabel, AssetSearch, AssetTile, NativeAssets, ThumbKind,
    TreeNav, TreeSearch, TreeTabBtn, TreeToggle,
};
use crate::tree::flat_folder_order;
use crate::thumbnails::ThumbnailCache;

pub(crate) fn tree_toggle_click(
    q: Query<(&Interaction, &TreeToggle), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, toggle) in &q {
        if *interaction == Interaction::Pressed {
            if state.expanded.contains(&toggle.0) {
                state.expanded.remove(&toggle.0);
            } else {
                state.expanded.insert(toggle.0.clone());
            }
        }
    }
}

pub(crate) fn tree_nav_click(
    q: Query<(&Interaction, &TreeNav)>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    windows: Query<&Window>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut state: ResMut<NativeAssets>,
    mut press: Local<Option<(PathBuf, Vec2)>>,
    mut last_click: Local<Option<(PathBuf, f64)>>,
) {
    // While a rename field is open, the field owns clicks — don't navigate/re-arm.
    if state.renaming.is_some() {
        *press = None;
        return;
    }
    let cursor = windows.iter().find_map(|w| w.cursor_position());
    if mouse.just_pressed(MouseButton::Left) {
        if let (Some((_, nav)), Some(c)) =
            (q.iter().find(|(i, _)| **i == Interaction::Pressed), cursor)
        {
            *press = Some((nav.0.clone(), c));
        }
    }
    // Navigate on release only if it was a click (no drag) — a press that moved
    // >5px is a folder drag (handled by `asset_drag`), not a navigation.
    if mouse.just_released(MouseButton::Left) {
        if let Some((path, origin)) = press.take() {
            let moved = cursor.map(|c| c.distance(origin) > 5.0).unwrap_or(false);
            if moved {
                return;
            }
            let ctrl = keys.any_pressed([
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::SuperLeft,
                KeyCode::SuperRight,
            ]);
            let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
            if ctrl || shift {
                // Ctrl/Shift click multi-selects tree folders (for a batch delete)
                // instead of navigating. Shift-range walks the tree's flattened
                // visible order, computed on demand (only on this click, not per
                // frame).
                let order: Vec<PathBuf> = project
                    .as_ref()
                    .map(|p| flat_folder_order(&p.path, &state.expanded, state.narrow))
                    .unwrap_or_default();
                state.click_select_in(&path, ctrl, shift, &order);
            } else {
                let now = time.elapsed_secs_f64();
                let double = last_click
                    .as_ref()
                    .is_some_and(|(p, t)| *p == path && now - t < 0.4);
                let was_sole = state.selection.len() == 1
                    && state.selected.as_deref() == Some(path.as_path());
                if double {
                    // Double-click = open: (re)navigate, cancel any armed rename.
                    *last_click = None;
                    state.rename_arm = None;
                    state.current = Some(path.clone());
                } else if was_sole {
                    // Slow second click on the already-selected folder arms a
                    // rename (fired by `rename_arm_fire` after the double-click
                    // window) — the same gesture the grid uses.
                    state.rename_arm = Some((path.clone(), now));
                    *last_click = Some((path, now));
                } else {
                    // First click: navigate, select this folder, toggle expansion.
                    state.current = Some(path.clone());
                    state.selection.clear();
                    state.selection.insert(path.clone());
                    state.selected = Some(path.clone());
                    state.selection_anchor = Some(path.clone());
                    if state.expanded.contains(&path) {
                        state.expanded.remove(&path);
                    } else {
                        state.expanded.insert(path.clone());
                    }
                    *last_click = Some((path, now));
                }
            }
        }
    }
}

pub(crate) fn tree_tab_click(
    q: Query<(&Interaction, &TreeTabBtn), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed && state.tree_tab != btn.0 {
            state.tree_tab = btn.0;
        }
    }
}

/// Mirror the narrow-mode search field into `tree_search` (same shape as
/// `search_sync` for the toolbar box, kept separate — see [`TreeSearch`]).
pub(crate) fn tree_search_sync(
    input: Query<&EmberTextInput, With<TreeSearch>>,
    mut state: ResMut<NativeAssets>,
) {
    for inp in &input {
        if state.tree_search != inp.value {
            state.tree_search = inp.value.clone();
        }
    }
}

pub(crate) fn tile_click(
    q: Query<(&Interaction, &AssetTile), Changed<Interaction>>,
    names: Query<(&Interaction, &AssetNameLabel), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    cmds: Option<Res<EditorCommands>>,
) {
    // While an inline rename is active, let clicks fall through to its text field
    // (focus/caret) instead of re-selecting or navigating into the folder.
    if state.renaming.is_some() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let root = project.as_ref().map(|p| p.path.clone());
    // Whether this item was *already* the sole selection before this click — the
    // gate for explorer-style click-the-name-to-rename (so the click that first
    // selects an item never renames). Captured before the loop mutates selection.
    let prev_sole = (state.selection.len() == 1).then(|| state.selected.clone()).flatten();
    // The name label pressed this frame (its tile is also Pressed via FocusPolicy::Pass).
    let name_pressed = names.iter().find(|(i, _)| **i == Interaction::Pressed).map(|(_, n)| n.0.clone());
    for (interaction, tile) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let double = state
            .last_click
            .as_ref()
            .is_some_and(|(p, t)| p == &tile.path && now - t < 0.4);
        if double {
            state.last_click = None;
            // A double-click opens/navigates — cancel any rename armed by its
            // first click so the field doesn't pop up after we've opened the item,
            // and drop the deferred single-select so release doesn't re-add it.
            state.rename_arm = None;
            state.pending_single_select = None;
            if tile.is_dir {
                state.current = Some(tile.path.clone());
                state.selected = None;
                state.selection.clear();
            } else {
                open_file(&cmds, &tile.path);
                track_recent(&mut state, &tile.path, root.as_deref());
            }
        } else if !ctrl
            && !shift
            && state.selection.len() > 1
            && state.selection.contains(&tile.path)
        {
            // Pressing an item that's already part of a multi-selection: keep the
            // whole selection intact so a drag can carry it (drag-to-viewport),
            // and defer the collapse-to-single to release if this turns out to be
            // a plain click rather than a drag.
            state.pending_single_select = Some(tile.path.clone());
            state.selected = Some(tile.path.clone());
            state.last_click = Some((tile.path.clone(), now));
        } else {
            // Single click selects (ctrl toggles, shift range-selects); a second
            // click within 0.4s opens / navigates.
            state.click_select(&tile.path, ctrl, shift);
            state.pending_single_select = None;
            state.last_click = Some((tile.path.clone(), now));
        }
    }
    // Clicking the name label of the already-sole-selected item arms a rename.
    // `last_click == Some(path)` confirms this was a single click (a double-click
    // cleared it to None above), so double-click-to-open still wins.
    if let Some(p) = name_pressed {
        let single = state.last_click.as_ref().is_some_and(|(lp, _)| lp == &p);
        if single && prev_sole.as_deref() == Some(p.as_path()) && state.rename_arm.is_none() {
            state.rename_arm = Some((p, now));
        }
    }
}

/// Open a non-folder asset. Editor-backed kinds (scripts, templates, materials,
/// blueprints, particles, scenes, shaders, plain text) route to their in-editor
/// editor/layout via [`crate::open_double_clicked`]; everything else (textures,
/// audio, …) falls back to the OS default app. Deferred through `EditorCommands`
/// because the routing needs `&mut World`.
pub(crate) fn open_file(cmds: &Option<Res<EditorCommands>>, path: &Path) {
    let Some(cmds) = cmds else {
        if !opens_in_editor(path) {
            os_open(path);
        }
        return;
    };
    if is_code_kind(path) {
        let p = path.to_path_buf();
        cmds.push(move |w: &mut World| open_in_code_editor(w, p));
    } else if opens_in_editor(path) {
        let p = path.to_path_buf();
        cmds.push(move |w: &mut World| crate::open_double_clicked(w, p));
    } else {
        os_open(path);
    }
}

/// Whether double-clicking the file should open it inside the editor (vs the OS
/// default app). Mirrors the egui browser's `open_double_clicked` routing.
fn opens_in_editor(path: &Path) -> bool {
    renzora_editor_framework::doc_kind_for_path(path).is_some() || is_editable_text(path)
}

/// Code-editor-backed kinds: scripts, shaders and plain text. These open
/// straight into `CodeEditorState` (no layout switch).
///
/// A `.html` UI template is deliberately **not** one of them any more. It is
/// `DocTabKind::Ui`, which opens the canvas — the code editor is tabbed beside
/// it in the UI workspace for whoever wants the markup.
fn is_code_kind(path: &Path) -> bool {
    use renzora_editor_framework::DocTabKind;
    matches!(
        renzora_editor_framework::doc_kind_for_path(path),
        Some(DocTabKind::Script | DocTabKind::Shader)
    ) || is_editable_text(path)
}

/// Text formats the code editor opens that aren't covered by `doc_kind_for_path`.
fn is_editable_text(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str(),
        "rs" | "json" | "toml" | "yaml" | "yml" | "txt" | "md" | "css"
    )
}

/// Context-menu "Open …" label + icon for an asset, or `None` if it has no
/// in-editor opener (a folder, texture, audio clip, …).
pub(crate) fn open_action(path: &Path) -> Option<(&'static str, String)> {
    use renzora_editor_framework::DocTabKind;
    if path.is_dir() {
        return Some(("folder-open", renzora::lang::t("common.open")));
    }
    match renzora_editor_framework::doc_kind_for_path(path) {
        Some(DocTabKind::Material) => Some(("palette", renzora::lang::t("assets.open_in.material_editor"))),
        Some(DocTabKind::Particle) => Some(("sparkle", renzora::lang::t("assets.open_in.particle_editor"))),
        Some(DocTabKind::Blueprint) => Some(("blueprint", renzora::lang::t("assets.open_in.blueprint_editor"))),
        Some(DocTabKind::Scene) => Some(("film-slate", renzora::lang::t("assets.open_in.scene"))),
        Some(DocTabKind::Script) | Some(DocTabKind::Shader) => Some(("code", renzora::lang::t("assets.open_in.code_editor"))),
        // Without this arm a `.html` fell through to `None` and lost its
        // right-click **Open** entirely the moment it stopped being a `Script`.
        Some(DocTabKind::Ui) => Some(("browser", renzora::lang::t_or("assets.open_in.ui_editor", "Open in UI Editor"))),
        _ if is_editable_text(path) => Some(("code", renzora::lang::t("assets.open_in.code_editor"))),
        _ => None,
    }
}

/// Context-menu open: navigate into folders, route files to their editor.
pub(crate) fn open_from_menu(world: &mut World, path: &Path) {
    if path.is_dir() {
        if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
            s.current = Some(path.to_path_buf());
            s.selected = None;
        }
    } else if is_code_kind(path) {
        open_in_code_editor(world, path.to_path_buf());
    } else {
        crate::open_double_clicked(world, path.to_path_buf());
    }
}

/// Open a script/shader/template/text file in the code editor — as a **document
/// tab**, like every other kind the browser opens.
///
/// It used to insert `OpenCodeEditorFile` and stop there, which is why a
/// double-clicked script was the one asset that opened without appearing in the
/// tab strip: nothing had told `DocumentTabState` about it. `open_asset_tab`
/// does that, focuses an already-open tab instead of opening a second one, and
/// inserts the `OpenCodeEditorFile` request itself for script/shader kinds — so
/// the file still lands in `CodeEditorState` exactly as before.
///
/// Text formats the kind table doesn't know (`.rs`, `.toml`, `.md`, `.css`) open
/// as `Script` tabs: the code editor is what holds them, and `Script` is the kind
/// that routes there.
fn open_in_code_editor(world: &mut World, path: PathBuf) {
    use renzora_editor_framework::DocTabKind;
    let kind = renzora_editor_framework::doc_kind_for_path(&path).unwrap_or(DocTabKind::Script);
    renzora_editor_framework::open_asset_tab(world, &path, kind);

    // Nothing here for the bevy_ui shell's dock: revealing the code-editor panel
    // there is `sync_workspace_to_active_doc`'s job now, because it runs *after*
    // the workspace switch the new tab triggers. Adding the panel from here put
    // it into the layout we were about to leave.
}

/// Open a file with its OS default application.
fn os_open(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

pub(crate) fn back_click(
    q: Query<&Interaction, (With<AssetBack>, Changed<Interaction>)>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(root) = project.map(|p| p.path.clone()) else {
        return;
    };
    let cur = state.current.clone().unwrap_or_else(|| root.clone());
    if cur == root {
        return;
    }
    if let Some(parent) = cur.parent() {
        // Don't navigate above the project root.
        if parent.starts_with(&root) || parent == root {
            state.current = Some(parent.to_path_buf());
            state.selected = None;
        }
    }
}

/// Kick off thumbnail loads for visible tiles (each registry de-dupes).
pub(crate) fn request_thumbnails(
    tiles: Query<&AssetTile>,
    mut cache: ResMut<ThumbnailCache>,
    mut model: Option<ResMut<ModelThumbnailRegistry>>,
    mut material: Option<ResMut<MaterialThumbnailRegistry>>,
    mut scene: Option<ResMut<SceneThumbnailRegistry>>,
    folders: Res<crate::thumbnails::FolderPreviews>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let project = project.as_deref();
    for tile in &tiles {
        if tile.is_dir {
            // A folder's tile draws a mosaic of images found inside it — those
            // are ordinary image thumbnails and load through the same cache.
            if let Some(images) = folders.images(&tile.path) {
                for image in images.iter() {
                    cache.request(image.clone(), &asset_server, project);
                }
            }
            continue;
        }
        let Some(name) = tile.path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        match thumb_kind(name) {
            Some(ThumbKind::Image) => {
                cache.request(tile.path.clone(), &asset_server, project);
            }
            Some(ThumbKind::Model) => {
                if let Some(model) = model.as_mut() {
                    model.request(tile.path.clone());
                }
            }
            Some(ThumbKind::Material) => {
                if let Some(material) = material.as_mut() {
                    material.request(tile.path.clone());
                }
            }
            Some(ThumbKind::Scene) => {
                if let Some(scene) = scene.as_mut() {
                    scene.request(tile.path.clone(), &asset_server, project);
                }
            }
            None => {}
        }
    }
}

pub(crate) fn search_sync(
    input: Query<&EmberTextInput, With<AssetSearch>>,
    mut state: ResMut<NativeAssets>,
) {
    for inp in &input {
        if state.search != inp.value {
            state.search = inp.value.clone();
        }
    }
}
