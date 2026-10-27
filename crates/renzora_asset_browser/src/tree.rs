//! The folder tree in the left pane — and, in the narrow tree-only layout, the
//! whole browser: there it lists files as well as folders, and those file rows
//! carry an `AssetTile` so they select, open, drag and right-click through
//! exactly the same systems as the grid's.
//!
//! Rows match the hierarchy panel: the same indent, row height and label size,
//! and the same inset rounded highlight on otherwise flat rows.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::picking::Pickable;
use bevy::prelude::*;

use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_bg;
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::UiInteraction;

use crate::grid::{display_name, rename_field_for};
use crate::ops::{asset_type_info, current_folder, folder_color, icon_for, project_root};
use crate::state::{
    file_name_of, hash_path_set, is_hidden_name, AssetNameLabel, AssetTile, NativeAssets,
    RenameSurface, ShortcutClick, TreeHit, TreeNav, TreeTab, TreeToggle,
};

/// Row label size: the hierarchy's, so the two tree lists in the editor match.
/// One constant because the folder rows and the Recent/Favorites rows have to
/// agree.
const TREE_TEXT: f32 = renzora_ember::inspector::LIST_ROW_TEXT;
const TREE_ICON: f32 = renzora_ember::inspector::LIST_ROW_ICON;
/// One caret width per level, as in the hierarchy (see its `INDENT`): a
/// child's caret sits under its parent's icon.
const TREE_INDENT: f32 = 16.0;
const TREE_ROW_H: f32 = 26.0;
const TREE_BASE_X: f32 = 4.0;

struct TreeRow {
    path: PathBuf,
    name: String,
    depth: usize,
    expanded: bool,
    has_children: bool,
    /// A file row (tree-only narrow mode) rather than a folder — rendered with a
    /// file icon + `AssetTile` so it selects/opens/drags like a grid tile.
    is_file: bool,
}

/// A folder's non-hidden children as `(path, name, is_dir)`.
///
/// The tree's three walks share this so only one of them has to know where
/// listings come from. Deliberately does NOT stat each entry — the grid needs
/// size and mtime, the tree needs neither, and a `metadata()` per file was
/// previously the dominant cost on folders of hundreds of meshes.
#[cfg(not(target_arch = "wasm32"))]
fn dir_kinds(dir: &Path) -> Vec<(PathBuf, String, bool)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if is_hidden_name(&name) {
                return None;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            Some((e.path(), name, is_dir))
        })
        .collect()
}

/// Web: out of the directory-handle cache. An unread folder reports empty for
/// now and fills in once the read lands, same as the grid.
#[cfg(target_arch = "wasm32")]
fn dir_kinds(dir: &Path) -> Vec<(PathBuf, String, bool)> {
    renzora_webfs::list_dir(dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| !is_hidden_name(&e.name))
        .map(|e| (dir.join(&e.name), e.name, e.is_dir))
        .collect()
}

fn has_subdirs(dir: &Path) -> bool {
    dir_kinds(dir).iter().any(|(_, _, is_dir)| *is_dir)
}

/// Whether `dir` contains any non-hidden file (used to decide if a folder is
/// expandable in tree-only file mode).
fn has_visible_files(dir: &Path) -> bool {
    dir_kinds(dir).iter().any(|(_, _, is_dir)| !*is_dir)
}

/// Every file and folder under `dir`, at any depth, for shift-click on a tree
/// folder. Walked on the click that needs it, never per frame, and capped so a
/// shift-click on the project root of a huge project cannot stall the editor.
pub(crate) fn descendants(dir: &Path) -> Vec<PathBuf> {
    const MAX: usize = 5_000;
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for (path, _, is_dir) in dir_kinds(&d) {
            if out.len() >= MAX {
                return out;
            }
            if is_dir {
                stack.push(path.clone());
            }
            out.push(path);
        }
    }
    out
}

fn flatten_dirs(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    show_files: bool,
    out: &mut Vec<TreeRow>,
) {
    let mut subs: Vec<(PathBuf, String)> = Vec::new();
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    for (path, name, is_dir) in dir_kinds(dir) {
        if is_dir {
            subs.push((path, name));
        } else if show_files {
            files.push((path, name));
        }
    }
    subs.sort_by_key(|a| a.1.to_lowercase());
    files.sort_by_key(|a| a.1.to_lowercase());
    for (path, name) in subs {
        let is_exp = expanded.contains(&path);
        let has = has_subdirs(&path) || (show_files && has_visible_files(&path));
        out.push(TreeRow {
            path: path.clone(),
            name,
            depth,
            expanded: is_exp,
            has_children: has,
            is_file: false,
        });
        if is_exp && has {
            flatten_dirs(&path, depth + 1, expanded, show_files, out);
        }
    }
    // Files of this folder sort after its subfolders, at the same depth.
    if show_files {
        for (path, name) in files {
            out.push(TreeRow {
                path,
                name,
                depth,
                expanded: false,
                has_children: false,
                is_file: true,
            });
        }
    }
}

/// Recursive filename search for the narrow browser's search box: every
/// file/folder under `dir` whose lowercase name contains `query`, as flat
/// (depth-0) rows so matches from any nesting level read as one result list.
/// Capped (results + directories visited) so typing one letter in a huge
/// project can't stall the frame — the tree snapshot runs on the UI thread.
fn search_tree(dir: &Path, query: &str, visited: &mut usize, out: &mut Vec<TreeRow>) {
    const MAX_RESULTS: usize = 200;
    const MAX_DIRS: usize = 1000;
    *visited += 1;
    if out.len() >= MAX_RESULTS || *visited > MAX_DIRS {
        return;
    }
    let mut entries: Vec<(PathBuf, String, bool)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if is_hidden_name(&name) || crate::state::is_sidecar(&name, |sibling| dir.join(sibling).exists()) {
                continue;
            }
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            entries.push((e.path(), name, is_dir));
        }
    }
    entries.sort_by_key(|e| e.1.to_lowercase());
    for (path, name, is_dir) in entries {
        if name.to_lowercase().contains(query) && out.len() < MAX_RESULTS {
            out.push(TreeRow {
                path: path.clone(),
                name,
                depth: 0,
                expanded: false,
                has_children: false,
                is_file: !is_dir,
            });
        }
        if is_dir {
            search_tree(&path, query, visited, out);
        }
    }
}

/// A row in the tree: an empty-state label, a recent/favorites shortcut, or a
/// folder-tree row.
enum TreeItem {
    /// A plain muted label ("NO RECENT FILES" and friends) shown when the active
    /// tab has nothing to list.
    Header { label: &'static str },
    Shortcut { name: String, path: PathBuf, is_dir: bool },
    Folder(TreeRow),
}

/// Dirty token for the folder tree. The tree's shape is decided by the expansion
/// set, the favorites/recent shortcuts and the file/folder mode; folding those in
/// lets the (filesystem-walking) snapshot be skipped on frames where none of them
/// changed. A whole-second bucket is mixed in so folders created on disk while the
/// panel is open still appear within a second.
pub(crate) fn tree_token(world: &Rx) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(st) = world.get_resource::<NativeAssets>() {
        hash_path_set(st.expanded.iter()).hash(&mut h);
        hash_path_set(st.favorites.iter()).hash(&mut h);
        hash_path_set(st.recent.iter()).hash(&mut h);
        st.narrow.hash(&mut h);
        st.tree_tab.hash(&mut h);
        st.tree_search.hash(&mut h);
        st.renaming.hash(&mut h);
        st.rename_surface.hash(&mut h);
    }
    if let Some(root) = project_root(world) {
        root.hash(&mut h);
    }
    if let Some(time) = world.get_resource::<Time>() {
        (time.elapsed_secs() as u64).hash(&mut h);
    }
    h.finish()
}

pub(crate) fn tree_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(root) = project_root(world) else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let st = world.get_resource::<NativeAssets>();
    let expanded = st.map(|s| s.expanded.clone()).unwrap_or_default();
    let favorites = st.map(|s| s.favorites.clone()).unwrap_or_default();
    let recent = st.map(|s| s.recent.clone()).unwrap_or_default();
    let narrow = st.map(|s| s.narrow).unwrap_or(false);
    let tree_tab = st.map(|s| s.tree_tab).unwrap_or(TreeTab::Folders);
    // A rename started on the grid draws its field there, not here: both
    // surfaces show the same folder, and two fields fight over the focus. See
    // `state::RenameSurface`.
    let renaming = st.and_then(|s| {
        (s.active_rename_surface() == RenameSurface::Tree)
            .then(|| s.renaming.clone())
            .flatten()
    });
    // The search box only exists in the narrow layout — ignore its (possibly
    // stale) value in the wide sidebar so it can't invisibly filter the tree.
    let query = if narrow {
        st.map(|s| s.tree_search.trim().to_lowercase()).unwrap_or_default()
    } else {
        String::new()
    };

    let mut items: Vec<TreeItem> = Vec::new();
    match tree_tab {
        TreeTab::Recent => {
            for p in recent.iter().take(20) {
                let name = file_name_of(p);
                if query.is_empty() || name.to_lowercase().contains(&query) {
                    items.push(TreeItem::Shortcut { name, path: p.clone(), is_dir: false });
                }
            }
            if items.is_empty() {
                items.push(TreeItem::Header { label: "NO RECENT FILES" });
            }
        }
        TreeTab::Favorites => {
            for p in &favorites {
                let name = file_name_of(p);
                if query.is_empty() || name.to_lowercase().contains(&query) {
                    items.push(TreeItem::Shortcut { name, path: p.clone(), is_dir: p.is_dir() });
                }
            }
            if items.is_empty() {
                items.push(TreeItem::Header { label: "NO FAVORITES" });
            }
        }
        TreeTab::Folders => {
            if query.is_empty() {
                // The project itself is the first row, and everything else
                // hangs off it.
                //
                // It was implicit before: the tree began with the project's
                // *children*, so the folder you were actually in had no visible
                // parent and there was nowhere to drop a file to put it at the
                // top level. Making the root a row solves both at once, because
                // a row carries `TreeNav` and `TreeNav` is already a drop target.
                let root_open = expanded.contains(&root);
                items.push(TreeItem::Folder(TreeRow {
                    path: root.clone(),
                    name: file_name_of(&root),
                    depth: 0,
                    expanded: root_open,
                    has_children: true,
                    is_file: false,
                }));
                // Files appear in the tree only in the narrow layout, where the
                // tree IS the browser; the wide sidebar stays folders-only.
                if root_open {
                    let mut rows = Vec::new();
                    flatten_dirs(&root, 1, &expanded, narrow, &mut rows);
                    items.extend(rows.into_iter().map(TreeItem::Folder));
                }
            } else {
                // Searching: a flat list of every matching file/folder under
                // the project root, regardless of tree expansion.
                let mut rows = Vec::new();
                search_tree(&root, &query, &mut 0, &mut rows);
                if rows.is_empty() {
                    items.push(TreeItem::Header { label: "NO MATCHES" });
                }
                items.extend(rows.into_iter().map(TreeItem::Folder));
            }
        }
    }

    // A row's hash is its own content only, not its position. Rows used to fold
    // in their index for the odd/even stripe, so expanding one folder rebuilt
    // every row below it.
    let keyed: Vec<(u64, u64)> = items
        .iter()
        .map(|it| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match it {
                TreeItem::Header { label } => {
                    (0u8, label).hash(&mut k);
                    label.hash(&mut h);
                }
                TreeItem::Shortcut { name, path, is_dir } => {
                    (1u8, path).hash(&mut k);
                    (name, is_dir).hash(&mut h);
                }
                TreeItem::Folder(r) => {
                    (2u8, &r.path).hash(&mut k);
                    (r.depth, r.expanded, r.has_children, &r.name).hash(&mut h);
                    r.is_file.hash(&mut h);
                    // Rebuild this row when it enters/leaves inline-rename.
                    (renaming.as_deref() == Some(r.path.as_path())).hash(&mut h);
                }
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items: keyed,
        build: Box::new(move |c, f, i| match &items[i] {
            TreeItem::Header { label } => tree_header(c, f, label),
            TreeItem::Shortcut { name, path, is_dir } => shortcut_row(c, f, name, path, *is_dir),
            TreeItem::Folder(r) => {
                tree_row(c, f, r, renaming.as_deref() == Some(r.path.as_path()))
            }
        }),
    }
}

/// A muted empty-state label row ("NO RECENT FILES" and friends).
fn tree_header(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                padding: UiRect::new(Val::Px(6.0), Val::Px(0.0), Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            Name::new("tree-header"),
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_child(label);
    row
}

/// A Recent/Favorites row: flat like the folder tree, icon carrying the asset
/// type's accent color (folders keep their name-derived tint).
fn shortcut_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    path: &Path,
    is_dir: bool,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(TREE_ROW_H),
                // The right side keeps a shortened name's "…" off the panel edge.
                padding: UiRect::horizontal(Val::Px(8.0)),
                column_gap: Val::Px(4.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            UiInteraction::default(),
            ShortcutClick {
                path: path.to_path_buf(),
                is_dir,
            },
            Name::new("tree-shortcut"),
        ))
        .id();
    // The hover highlight is the same inset pill as the folder tree's.
    let highlight = commands
        .spawn((
            renzora_ember::inspector::list_row_highlight_node(),
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
        ))
        .id();
    bind_bg(commands, highlight, move |w| {
        if let Some(tint) = crate::drag_drop::drop_target_tint(w, row, is_dir) {
            return tint;
        }
        match w.get::<UiInteraction>(row) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => {
                renzora_ember::inspector::list_row_hover()
            }
            _ => Color::NONE,
        }
    });
    commands.entity(row).add_child(highlight);
    let (icon_name, icon_color) = if is_dir {
        ("folder", folder_color(name))
    } else {
        (icon_for(path, false), asset_type_info(path).0)
    };
    let ic = icon_text(commands, &fonts.phosphor, icon_name, icon_color, TREE_ICON);
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, TREE_TEXT),
            TextColor(rgb(text_primary())),
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    renzora_ember::widgets::ellipsize(commands, label, name, Some(row));
    commands.entity(row).add_children(&[ic, label]);
    row
}

fn tree_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    r: &TreeRow,
    is_renaming: bool,
) -> Entity {
    let content_x = TREE_BASE_X + r.depth as f32 * TREE_INDENT;

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(TREE_ROW_H),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("tree-row"),
        ))
        .id();

    // Caret (only when there are subfolders).
    let caret = commands
        .spawn(Node {
            width: Val::Px(16.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    if r.has_children {
        let glyph = icon_glyph(if r.expanded { "caret-down" } else { "caret-right" }).unwrap_or(' ');
        let g = commands
            .spawn((
                Text::new(glyph.to_string()),
                ui_font(&bevy::text::FontSource::Handle(fonts.phosphor.clone()), 10.0),
                TextColor(rgb(text_muted())),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(caret).insert((UiInteraction::default(), TreeToggle(r.path.clone())));
        commands.entity(caret).add_child(g);
    }

    // Nav zone (icon + name). Folder rows navigate via TreeNav; file rows (shown
    // in tree-only narrow mode) carry an AssetTile so they select/open/drag/
    // right-click through the same systems as the grid tiles.
    let nav = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                height: Val::Percent(100.0),
                column_gap: Val::Px(4.0),
                // Keeps a shortened name's "…" off the panel edge.
                padding: UiRect::right(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            UiInteraction::default(),
            Name::new(if r.is_file { "tree-file" } else { "tree-nav" }),
        ))
        .id();
    if r.is_file {
        commands.entity(nav).insert(AssetTile { path: r.path.clone(), is_dir: false });
    } else {
        commands.entity(nav).insert(TreeNav(r.path.clone()));
    }
    commands.entity(nav).insert(TreeHit(r.path.clone()));
    let folder_icon = if r.is_file {
        icon_text(commands, &fonts.phosphor, icon_for(&r.path, false), asset_type_info(&r.path).0, TREE_ICON)
    } else {
        icon_text(
            commands,
            &fonts.phosphor,
            if r.expanded { "folder-open" } else { "folder" },
            folder_color(&r.name),
            TREE_ICON,
        )
    };
    let name = if is_renaming {
        // Inline rename field (same widget the grid uses), laid out to grow in
        // the row rather than force a fixed width.
        let f = rename_field_for(commands, fonts, &r.path, display_name(&r.name, !r.is_file));
        commands.entity(f).insert(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            height: Val::Px(26.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        });
        f
    } else {
        // Both kinds of row rename with the same gesture as the grid: an
        // interactive `AssetNameLabel` that passes the click through to the row
        // beneath it (an `AssetTile` on a file row, a `TreeNav` on a folder) for
        // selection while still registering the name press that arms rename.
        //
        // The label is sized to the text, never stretched across the row: the
        // rest of a folder row has to stay a plain click that navigates and folds
        // the branch open, and only the name itself may open the field. A file
        // row's rename is a `Tree` rename even though the row carries an
        // `AssetTile`, because the tree is where it is drawn -- the grid is
        // hidden in the narrow layout that shows files here at all.
        let shown = display_name(&r.name, !r.is_file).to_string();
        let label = commands
            .spawn((
                Text::new(shown.clone()),
                ui_font(&fonts.ui, TREE_TEXT),
                TextColor(rgb(text_primary())),
                Node {
                    min_width: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                UiInteraction::default(),
                PASS_THROUGH,
                AssetNameLabel { path: r.path.clone(), surface: RenameSurface::Tree },
            ))
            .id();
        // A long name (a model's generated folder names run well past the
        // panel) ends in "…" instead of being cut mid-letter at the panel's
        // edge, and shows in full as the label's tooltip while shortened.
        renzora_ember::widgets::ellipsize(commands, label, shown, Some(label));
        label
    };
    commands.entity(nav).add_children(&[folder_icon, name]);

    // The row's highlight (selection + hover), lowest z: a rounded pill inset
    // from the panel's sides, on an otherwise flat row.
    let bg_visual = commands
        .spawn((
            renzora_ember::inspector::list_row_highlight_node(),
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
            Name::new("tree-row-bg"),
        ))
        .id();
    let sel_path = r.path.clone();
    let is_file = r.is_file;
    bind_bg(commands, bg_visual, move |w| {
        // Hover is read on `nav`, not on the background: `nav` is the row's
        // interactive zone and the one `drop_folder` resolves against, so the
        // highlight lights exactly when a release would land here.
        if let Some(tint) = crate::drag_drop::drop_target_tint(w, nav, !is_file) {
            return tint;
        }
        // A folder is lit while it is the open folder or part of a
        // multi-selection (a marquee, a ctrl-click, a shift-click on its
        // parent): a selection you cannot see is one you cannot trust to delete.
        let selected = w
            .get_resource::<NativeAssets>()
            .map(|s| s.is_selected(&sel_path) && (is_file || s.selection.len() > 1))
            .unwrap_or(false);
        let active = selected
            || (!is_file && current_folder(w).as_deref() == Some(sel_path.as_path()));
        if active {
            return renzora_ember::inspector::list_row_selected();
        }
        if matches!(
            w.get::<UiInteraction>(nav),
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
        ) {
            return renzora_ember::inspector::list_row_hover();
        }
        Color::NONE
    });

    // bg first (behind), then the indent spacer + content. No guide lines: the
    // indent alone shows nesting, and the lines were most of what made the tree
    // look deep and busy. (The hierarchy keeps its lines for one job, showing
    // which group a drag would drop into; nothing is dragged into a folder
    // here by its row's position, so they would have no job at all.)
    let mut kids = vec![bg_visual];
    // Indent spacer up to the caret column.
    kids.push(
        commands
            .spawn((
                Node {
                    width: Val::Px(content_x),
                    height: Val::Percent(100.0),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id(),
    );
    kids.push(caret);
    kids.push(nav);
    commands.entity(row).add_children(&kids);
    row
}
