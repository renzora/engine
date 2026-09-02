//! The **Files** tab: every file in the project as a folder tree, ticked.
//!
//! The ticks start from what the automatic crawl found, and from that moment the
//! list is authoritative — the archive holds exactly what is ticked. That is the
//! point of it: the crawl follows quoted asset paths out of files it packed, so
//! it cannot find a path a script assembles at runtime, and until recently could
//! not see a Rust script's assets at all (those are compiled into the binary, so
//! nothing packed them and nothing read them). A file it misses is a game that
//! runs and logs `Path not found` once a frame, and the fix should be a tick
//! rather than a restructure.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::tracked::{bind_2way, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{section, tree_node_with};

use crate::overlay::ExportOverlayState;

use super::settings::{finish_tab, tab_panel};
use super::widgets::{pill_button, txt};
use super::{FilesAction, FilesBulk, FilesPanel};

/// A checkbox sized for a tree row.
///
/// A checkbox rather than the toggle switch the Features tab uses: a file tree
/// is dense and long, and a row of switches reads as a control panel where what
/// is wanted is a list of ticks. `FocusPolicy::Block` so a click lands on the
/// box instead of falling through to the tree row and folding the folder.
fn file_check(commands: &mut Commands) -> Entity {
    let cb = renzora_ember::widgets::checkbox(commands, false);
    commands.entity(cb).insert(FocusPolicy::Block);
    cb
}

/// One level of the file tree: the folders at `prefix`, then the files.
///
/// Recursive over path segments rather than over the filesystem — the input is
/// already the flat, sorted list of keys the scan collected, so this never
/// touches the disk. Folders come first at each level and both halves stay in
/// the sorted order they arrived in, which is what makes the tree stable between
/// openings.
///
/// A folder's tick is not stored anywhere: it reads as on when every file under
/// it is on, and toggling it writes that answer down to each of them. There is
/// no third state to represent a partial folder — the row shows off, and the
/// files below it tell the truth.
fn file_tree_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    files: &[String],
    prefix: &str,
    depth: usize,
) -> Vec<Entity> {
    let mut folders: Vec<String> = Vec::new();
    let mut leaves: Vec<&String> = Vec::new();
    for key in files {
        let Some(rest) = key.strip_prefix(prefix) else { continue };
        match rest.split_once('/') {
            Some((dir, _)) => {
                if !folders.iter().any(|f| f == dir) {
                    folders.push(dir.to_string());
                }
            }
            None => leaves.push(key),
        }
    }

    let mut out = Vec::new();
    for dir in folders {
        let sub_prefix = format!("{prefix}{dir}/");
        let children = file_tree_rows(commands, fonts, files, &sub_prefix, depth + 1);
        // The keys this folder governs, captured for its own tick.
        let owned: Vec<String> = files
            .iter()
            .filter(|k| k.starts_with(&sub_prefix))
            .cloned()
            .collect();
        let cb = file_check(commands);
        let get_keys = owned.clone();
        let set_keys = owned;
        bind_2way(
            commands,
            cb,
            move |w| {
                w.get_resource::<ExportOverlayState>()
                    .and_then(|s| s.included_files.as_ref().map(|inc| {
                        !get_keys.is_empty() && get_keys.iter().all(|k| inc.contains(k))
                    }))
                    .unwrap_or(false)
            },
            move |w, v: &bool| {
                if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                    if let Some(inc) = s.included_files.as_mut() {
                        for k in &set_keys {
                            if *v {
                                inc.insert(k.clone());
                            } else {
                                inc.remove(k);
                            }
                        }
                    }
                }
            },
        );
        out.push(tree_node_with(commands, fonts, &dir, depth, children, false, vec![cb], None));
    }
    for key in leaves {
        let name = key.rsplit_once('/').map_or(key.as_str(), |(_, n)| n).to_string();
        let cb = file_check(commands);
        let get_key = key.clone();
        let set_key = key.clone();
        bind_2way(
            commands,
            cb,
            move |w| {
                w.get_resource::<ExportOverlayState>()
                    .and_then(|s| s.included_files.as_ref().map(|inc| inc.contains(&get_key)))
                    .unwrap_or(false)
            },
            move |w, v: &bool| {
                if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                    if let Some(inc) = s.included_files.as_mut() {
                        if *v {
                            inc.insert(set_key.clone());
                        } else {
                            inc.remove(&set_key);
                        }
                    }
                }
            },
        );
        // The same icon table the asset browser draws from, so a `.bsn` here is
        // the same film slate it is over there.
        let p = std::path::Path::new(key.as_str());
        let icon = renzora_ember::file_kind::icon_for(p, false);
        let colour = renzora_ember::file_kind::color_for(p);
        out.push(tree_node_with(
            commands,
            fonts,
            &name,
            depth,
            Vec::new(),
            false,
            vec![cb],
            Some((icon, colour)),
        ));
    }
    out
}

/// The tree is built once, when the modal spawns, from the file list the project
/// scan already collected. The ticks are reactive, so the detected set can
/// arrive a frame later — see
/// [`lifecycle::resolve_included_files`](super::lifecycle), which does the
/// reading only when this tab is actually on screen.
pub(super) fn build_files_tab(commands: &mut Commands, fonts: &EmberFonts, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    commands.entity(panel).insert(FilesPanel);
    let (sec, body) =
        section(commands, fonts, "folders", &renzora::lang::t("export.section.files"), accent());

    let note = txt(commands, fonts, &renzora::lang::t("export.files.note"), 11.0, text_muted());
    commands.entity(body).add_child(note);

    // Bulk actions. "Reset to detected" is the important one — it is the way
    // back from a mistake, and it re-runs the crawl rather than restoring a
    // snapshot, so it also picks up anything added since the dialog opened.
    let actions = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), margin: UiRect::vertical(Val::Px(4.0)), ..default() }).id();
    let all = pill_button(commands, fonts, "check-square", &renzora::lang::t("export.files.all"));
    commands.entity(all).insert(FilesBulk(FilesAction::All));
    let none = pill_button(commands, fonts, "square", &renzora::lang::t("export.files.none"));
    commands.entity(none).insert(FilesBulk(FilesAction::None));
    let reset = pill_button(commands, fonts, "arrow-counter-clockwise", &renzora::lang::t("export.files.reset"));
    commands.entity(reset).insert(FilesBulk(FilesAction::Detected));
    commands.entity(actions).add_children(&[all, none, reset]);
    commands.entity(body).add_child(actions);

    let count = txt(commands, fonts, "", 11.0, text_muted());
    bind_text(commands, count, |w| {
        let Some(s) = w.get_resource::<ExportOverlayState>() else {
            return String::new();
        };
        match &s.included_files {
            None => renzora::lang::t("export.files.auto"),
            Some(set) => renzora::lang::t("export.files.count")
                .replace("{n}", &set.len().to_string())
                .replace("{total}", &s.project_files.len().to_string()),
        }
    });
    commands.entity(body).add_child(count);

    // The tree, added straight into the section body. `finish_tab` puts ONE
    // scroll area around the whole panel, exactly as the other four tabs do —
    // an inner scroll area here nested a `tab_max`-tall viewport inside a panel
    // nothing capped, so the tree ran off the bottom of the dialog.
    //
    // Filled by a command that can read the world, the same way the plugin cards
    // are: the file list is settled by the scan that ran a moment ago, but this
    // builder only has `Commands`.
    let tree = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    commands.entity(body).add_child(tree);
    commands.queue(move |world: &mut World| {
        let files: Vec<String> = world
            .get_resource::<ExportOverlayState>()
            .map(|s| s.project_files.clone())
            .unwrap_or_default();
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut c = Commands::new(&mut queue, world);
            let rows = file_tree_rows(&mut c, &fonts, &files, "", 0);
            c.entity(tree).add_children(&rows);
        }
        queue.apply(world);
    });

    finish_tab(commands, panel, &[sec], tab_max);
    panel
}
