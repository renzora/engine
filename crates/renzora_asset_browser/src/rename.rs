//! Inline rename (F2, the context menu, and the OS-explorer "slow second click"
//! on a name label), plus the Delete shortcut — which lives here because both
//! are keyboard edits that have to agree about when a rename is in flight.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora_ember::widgets::EmberTextInput;

use crate::grid::display_name;
use crate::ops::delete_asset;
use crate::state::{file_name_of, AssetRenameInput, AssetRoot, NativeAssets};

/// Begin an inline rename of `path`: select it (so it's the visible target) and
/// arm `renaming`, which makes the keyed-list rebuild that tile with a focused
/// text field (`AssetRenameInput`).
pub(crate) fn start_rename(world: &mut World, path: &Path) {
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        s.selection.clear();
        s.selection.insert(path.to_path_buf());
        s.selected = Some(path.to_path_buf());
        s.selection_anchor = Some(path.to_path_buf());
        s.renaming = Some(path.to_path_buf());
    }
}

/// Fire a name-click-armed rename once it has survived the double-click window —
/// the delay is what lets a double-click open the item instead (it clears the
/// arm). Cancels the arm if the selection has since moved off the item.
pub(crate) fn rename_arm_fire(time: Res<Time>, mut state: ResMut<NativeAssets>) {
    // Just over the 0.4s double-click window, so an open always cancels first.
    const DELAY: f64 = 0.45;
    let Some((path, t)) = state.rename_arm.clone() else {
        return;
    };
    let sole = state.selection.len() == 1 && state.selected.as_deref() == Some(path.as_path());
    if state.renaming.is_some() || !sole {
        state.rename_arm = None;
        return;
    }
    if time.elapsed_secs_f64() - t >= DELAY {
        state.rename_arm = None;
        state.renaming = Some(path);
    }
}

/// F2 starts an inline rename of the primary selection (folder or file), unless
/// one is already in progress.
pub(crate) fn rename_shortcut(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<NativeAssets>) {
    if !keys.just_pressed(KeyCode::F2) || state.renaming.is_some() {
        return;
    }
    if let Some(path) = state.selected.clone() {
        state.renaming = Some(path);
    }
}

/// Delete the selected files/folders on the Delete key while the asset grid is
/// hovered with a selection.
///
/// Runs in `PreUpdate` and **consumes** the key (`clear_just_pressed`) so no
/// `Update` consumer — the gizmo's entity-delete, the timeline's keyframe
/// delete, etc. — also fires on the same press. The shared
/// `InputFocusState::suppress_entity_delete` bool is unusable for this: several
/// panel guards write it every frame and clobber one another (the timeline guard
/// forces it `false` whenever its own cursor isn't over the timeline), so a
/// raised flag here would be overwritten before the gizmo reads it. Consuming
/// the key in an earlier schedule sidesteps the ordering entirely.
pub(crate) fn asset_delete_shortcut(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    roots: Query<&bevy::ui::RelativeCursorPosition, With<AssetRoot>>,
    state: Res<NativeAssets>,
    mut commands: Commands,
) {
    // Claim Delete anywhere over the asset panel (grid *or* folder tree), not just
    // the grid, so selected tree folders can be deleted with the key too.
    let claim = roots.iter().any(|r| r.cursor_over)
        && state.renaming.is_none()
        && (!state.selection.is_empty() || state.selected.is_some());
    if !claim || !keys.just_pressed(KeyCode::Delete) {
        return;
    }
    keys.clear_just_pressed(KeyCode::Delete);
    let mut paths: Vec<PathBuf> = state.selection.iter().cloned().collect();
    if paths.is_empty() {
        paths.extend(state.selected.clone());
    }
    commands.queue(move |world: &mut World| {
        for p in &paths {
            delete_asset(world, p);
        }
    });
}

/// Auto-focus the rename field the frame it appears — it's spawned by the keyed
/// list (not a click), so `text_input` wouldn't focus it otherwise.
pub(crate) fn focus_asset_rename(mut q: Query<&mut EmberTextInput, Added<AssetRenameInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
        // Start with the whole name selected (like an OS rename) so typing or
        // Delete replaces it outright.
        inp.select_all = true;
    }
}

/// Commit (Enter / click-away blur) or cancel (Escape) the active rename. Mirrors
/// the hierarchy's `rename_commit`: wait for the keyed-list-spawned field, and
/// only commit-on-blur once it has actually held focus.
pub(crate) fn asset_rename_commit(
    mut state: ResMut<NativeAssets>,
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<(&EmberTextInput, &AssetRenameInput)>,
    mut commands: Commands,
    mut had_focus: Local<bool>,
) {
    let Some(path) = state.renaming.clone() else {
        *had_focus = false;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        state.renaming = None;
        *had_focus = false;
        return;
    }
    // Wait for the rename field to spawn (don't cancel in the meantime).
    let Some((inp, _)) = inputs.iter().find(|(_, r)| r.0 == path) else {
        return;
    };
    if inp.focused {
        *had_focus = true;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let blurred = *had_focus && !inp.focused;
    if !enter && !blurred {
        return;
    }
    let new_name: String = inp.value.replace('\n', "").trim().to_string();
    state.renaming = None;
    *had_focus = false;
    // The field was seeded with the *displayed* name, so an untouched stem is a
    // no-op even though it isn't the on-disk file name.
    let current = file_name_of(&path);
    if new_name.is_empty() || new_name == display_name(&current, path.is_dir()) {
        return;
    }
    commands.queue(move |world: &mut World| finish_rename(world, &path, &new_name));
}

/// Rename `old` to `new_name` in place (same parent). Skips no-op / colliding
/// names, updates asset references (`emit_asset_path_change`) and the selection /
/// current-folder so they track the new path.
fn finish_rename(world: &mut World, old: &Path, new_name: &str) {
    let Some(parent) = old.parent() else {
        return;
    };
    let is_dir = old.is_dir();
    let dest = parent.join(keep_extension(old, new_name, is_dir));
    if dest == old || dest.exists() {
        return;
    }
    if std::fs::rename(old, &dest).is_err() {
        return;
    }
    crate::emit_asset_path_change(world, old, &dest, is_dir);
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        s.listing_dirty = true;
        if s.selection.remove(old) {
            s.selection.insert(dest.clone());
        }
        if s.selected.as_deref() == Some(old) {
            s.selected = Some(dest.clone());
        }
        if s.selection_anchor.as_deref() == Some(old) {
            s.selection_anchor = Some(dest.clone());
        }
        // Renaming the open folder (e.g. via F2 in tree-only mode) follows it.
        if s.current.as_deref() == Some(old) {
            s.current = Some(dest);
        }
    }
}

/// The file name a rename of `old` to `new_name` should actually produce.
///
/// Typing just the stem keeps the old extension, so renaming `rock.png` to
/// `boulder` yields `boulder.png` rather than an extension-less file the
/// importers no longer recognise. This matches the OS file browsers and the
/// scene-tab rename in `renzora_shell`. Writing an extension explicitly still
/// wins (`boulder.jpg` stays `.jpg`), and a trailing dot (`boulder.`) is the
/// escape hatch for genuinely dropping one. Folders are left alone — a dot in
/// a folder name is part of the name, not a type.
fn keep_extension(old: &Path, new_name: &str, is_dir: bool) -> String {
    if is_dir {
        return new_name.to_string();
    }
    // Windows strips a trailing dot from the real file name, so trim it here and
    // Linux drops the extension the same way instead of keeping a literal
    // `boulder.` on disk.
    if let Some(stem) = new_name.strip_suffix('.') {
        return stem.to_string();
    }
    // "No dot at all" rather than `Path::extension`, so a dotfile name like
    // `.gitignore` reads as a deliberate choice and doesn't collect the old
    // extension on the end.
    match old.extension().and_then(|e| e.to_str()) {
        Some(ext) if !new_name.contains('.') => format!("{new_name}.{ext}"),
        _ => new_name.to_string(),
    }
}
