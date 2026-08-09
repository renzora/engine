//! Folder picker — a project's own directory tree as a bordered, scrollable
//! list of rows with exactly one selected. For every "where should this land?"
//! prompt in the editor: the marketplace's install-into confirmation, the
//! hierarchy's Create-asset overlay.
//!
//! The pick lives in one [`FolderPick`] resource rather than in each caller's
//! own state. That's deliberate: a picker only ever appears inside a modal
//! overlay, so at most one is on screen, and a shared resource is what lets the
//! selected-row highlight be a plain reactive binding instead of click plumbing
//! re-written per caller.
//!
//! The walk is depth-bounded and count-capped, and skips hidden / build /
//! dependency directories — a huge project shouldn't stall the overlay opening,
//! and `target/` is never a place an asset belongs.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::tracked::bind_bg;
use crate::theme::*;

use super::scroll_view;

/// How many rows the walk will produce before it gives up descending.
const MAX_ROWS: usize = 300;

/// The folder the open picker currently targets. [`folder_picker`] seeds it;
/// the caller reads it when its overlay is confirmed.
#[derive(Resource, Default)]
pub struct FolderPick(pub Option<PathBuf>);

impl FolderPick {
    pub fn path(&self) -> Option<&Path> {
        self.0.as_deref()
    }
}

/// One row of the tree, carrying the directory it selects.
#[derive(Component)]
pub(crate) struct FolderPickRow(PathBuf);

/// A bordered, scrollable folder tree rooted at `root` with `selected`
/// pre-picked. Returns the box entity: drop it straight into an overlay body.
/// It flex-grows, so put it between the fixed content above and the buttons
/// below and it fills whatever height is left instead of leaving dead space.
///
/// `max_depth` bounds the walk below `root` (`0` = the root's own children
/// only). `root` itself is always the first row, so "put it at the top level"
/// is reachable without scrolling.
pub fn folder_picker(
    commands: &mut Commands,
    fonts: &EmberFonts,
    root: &Path,
    selected: &Path,
    max_depth: usize,
) -> Entity {
    commands.insert_resource(FolderPick(Some(selected.to_path_buf())));

    let tree = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            },
            Name::new("folder-picker-tree"),
        ))
        .id();

    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let mut rows = vec![folder_row(commands, fonts, root.to_path_buf(), 0, &root_name)];
    for (path, depth, name) in folder_dirs(root, max_depth) {
        rows.push(folder_row(commands, fonts, path, depth + 1, &name));
    }
    commands.entity(tree).add_children(&rows);

    let scroll = scroll_view(commands, tree);
    let boxed = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(60.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Name::new("folder-picker"),
        ))
        .id();
    commands.entity(boxed).add_child(scroll);
    boxed
}

/// The directories under `root`, depth-first and alphabetical, as
/// `(path, depth, name)` — `depth` counted from `root`'s children at 0. Public
/// so a caller that wants its own row rendering can still share the walk (and
/// its skip rules) rather than re-deriving them.
pub fn folder_dirs(root: &Path, max_depth: usize) -> Vec<(PathBuf, usize, String)> {
    fn rec(dir: &Path, depth: usize, max: usize, out: &mut Vec<(PathBuf, usize, String)>) {
        if depth > max || out.len() > MAX_ROWS {
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        entries.sort();
        for path in entries {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Hidden dotfolders, cargo's build dir and npm's dependency dir are
            // never asset destinations, and `target/` alone would blow the row
            // cap on its own.
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            out.push((path.clone(), depth, name));
            rec(&path, depth + 1, max, out);
        }
    }
    let mut out = Vec::new();
    rec(root, 0, max_depth, &mut out);
    out
}

fn folder_row(commands: &mut Commands, fonts: &EmberFonts, path: PathBuf, depth: usize, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::left(Val::Px(8.0 + depth as f32 * 14.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            FolderPickRow(path.clone()),
            Name::new("folder-picker-row"),
        ))
        .id();
    let p = path.clone();
    bind_bg(commands, row, move |w| {
        if w.get_resource::<FolderPick>().is_some_and(|f| f.0.as_deref() == Some(p.as_path())) {
            rgb(accent()).with_alpha(0.20)
        } else if matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "folder", text_muted(), 12.0);
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(row).add_children(&[icon, label]);
    row
}

/// Click a row → it becomes the pick.
pub(crate) fn folder_pick_click(
    q: Query<(&Interaction, &FolderPickRow), Changed<Interaction>>,
    mut pick: ResMut<FolderPick>,
) {
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed && pick.0.as_deref() != Some(row.0.as_path()) {
            pick.0 = Some(row.0.clone());
        }
    }
}
