//! The folder tree in the left pane — and, in the narrow tree-only layout, the
//! whole browser: there it lists files as well as folders, and those file rows
//! carry an `AssetTile` so they select, open, drag and right-click through
//! exactly the same systems as the grid's.
//!
//! Indent guides match the hierarchy panel: 1.5px absolute line nodes, the same
//! INDENT / LINE_OFFSET geometry, and `inspector_stripe` odd/even row bands.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::picking::Pickable;
use bevy::prelude::*;

use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::reactive::tracked::bind_bg;
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};

use crate::grid::{display_name, rename_field_for};
use crate::ops::{asset_type_info, current_folder, folder_color, icon_for, project_root};
use crate::state::{
    file_name_of, hash_path_set, AssetNameLabel, AssetTile, NativeAssets, ShortcutClick, TreeNav,
    TreeTab, TreeToggle,
};

const TREE_INDENT: f32 = 12.0;
const TREE_ROW_H: f32 = 20.0;
const TREE_BASE_X: f32 = 4.0;
const TREE_LINE_OFFSET: f32 = TREE_INDENT / 2.0 - 1.0; // 5.0
const TREE_CENTER_Y: f32 = TREE_ROW_H / 2.0;

struct TreeRow {
    path: PathBuf,
    name: String,
    depth: usize,
    expanded: bool,
    has_children: bool,
    /// Whether this is the last child of its parent (the elbow vs. tee join).
    is_last: bool,
    /// For each ancestor level, whether that ancestor has more siblings below
    /// (i.e. draw a pass-through vertical line at that level).
    parent_lines: Vec<bool>,
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
            if name.starts_with('.') {
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
        .filter(|e| !e.name.starts_with('.'))
        .map(|e| (dir.join(&e.name), e.name, e.is_dir))
        .collect()
}

fn has_subdirs(dir: &Path) -> bool {
    dir_kinds(dir).iter().any(|(_, _, is_dir)| *is_dir)
}

/// A 1.5px vertical guide line. `full` runs the whole row height; otherwise it
/// stops at `height` (used for the elbow on a last child).
/// Whether `dir` contains any non-hidden file (used to decide if a folder is
/// expandable in tree-only file mode).
fn has_visible_files(dir: &Path) -> bool {
    dir_kinds(dir).iter().any(|(_, _, is_dir)| !*is_dir)
}

/// The tree's flattened visible order, as bare paths — what a shift-range
/// select in the tree walks. Computed on demand (only on the click that needs
/// it, never per frame), which is why this hands back paths rather than exposing
/// [`TreeRow`] to the click systems.
pub(crate) fn flat_folder_order(
    root: &Path,
    expanded: &HashSet<PathBuf>,
    show_files: bool,
) -> Vec<PathBuf> {
    let mut rows = Vec::new();
    flatten_dirs(root, 0, expanded, show_files, &mut Vec::new(), &mut rows);
    rows.into_iter().map(|r| r.path).collect()
}

fn flatten_dirs(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    show_files: bool,
    ancestors: &mut Vec<bool>,
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
    let has_files = !files.is_empty();
    let last = subs.len().saturating_sub(1);
    for (idx, (path, name)) in subs.into_iter().enumerate() {
        // A folder isn't truly last if files follow it below.
        let more_after = idx != last || has_files;
        let is_exp = expanded.contains(&path);
        let has = has_subdirs(&path) || (show_files && has_visible_files(&path));
        out.push(TreeRow {
            path: path.clone(),
            name,
            depth,
            expanded: is_exp,
            has_children: has,
            is_last: !more_after,
            parent_lines: ancestors.clone(),
            is_file: false,
        });
        if is_exp && has {
            // Descendants draw a pass-through line at this level iff this node
            // has a sibling (folder or file) after it.
            ancestors.push(more_after);
            flatten_dirs(&path, depth + 1, expanded, show_files, ancestors, out);
            ancestors.pop();
        }
    }
    // Files of this folder sort after its subfolders, at the same depth.
    if show_files {
        let last_f = files.len().saturating_sub(1);
        for (idx, (path, name)) in files.into_iter().enumerate() {
            out.push(TreeRow {
                path,
                name,
                depth,
                expanded: false,
                has_children: false,
                is_last: idx == last_f,
                parent_lines: ancestors.clone(),
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
            if name.starts_with('.') {
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
                is_last: true,
                parent_lines: Vec::new(),
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
    let renaming = st.and_then(|s| s.renaming.clone());
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
                // Files appear in the tree only in the narrow layout, where the
                // tree IS the browser; the wide sidebar stays folders-only.
                let mut rows = Vec::new();
                flatten_dirs(&root, 0, &expanded, narrow, &mut Vec::new(), &mut rows);
                items.extend(rows.into_iter().map(TreeItem::Folder));
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

    let keyed: Vec<(u64, u64)> = items
        .iter()
        .enumerate()
        .map(|(idx, it)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match it {
                TreeItem::Header { label } => {
                    (0u8, label).hash(&mut k);
                    label.hash(&mut h);
                }
                TreeItem::Shortcut { name, path, is_dir } => {
                    (1u8, path).hash(&mut k);
                    // The stripe is baked in at build time from the row index, so
                    // a row whose position shifts must rebuild even when its own
                    // content didn't change — fold the index into the hash.
                    (name, is_dir, idx).hash(&mut h);
                }
                TreeItem::Folder(r) => {
                    (2u8, &r.path).hash(&mut k);
                    (r.depth, r.expanded, r.has_children, &r.name).hash(&mut h);
                    (r.is_last, &r.parent_lines, r.is_file, idx).hash(&mut h);
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
            TreeItem::Shortcut { name, path, is_dir } => shortcut_row(c, f, name, path, *is_dir, i),
            TreeItem::Folder(r) => {
                tree_row(c, f, r, i, renaming.as_deref() == Some(r.path.as_path()))
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
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_child(label);
    row
}

/// A Recent/Favorites row: zebra-striped like the folder tree, icon carrying the
/// asset type's accent color (folders keep their name-derived tint).
fn shortcut_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    path: &Path,
    is_dir: bool,
    row_index: usize,
) -> Entity {
    let stripe = inspector_stripe(row_index);
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(TREE_ROW_H),
                padding: UiRect::left(Val::Px(8.0)),
                column_gap: Val::Px(4.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(stripe),
            Interaction::default(),
            ShortcutClick {
                path: path.to_path_buf(),
                is_dir,
            },
            Name::new("tree-shortcut"),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::border()),
        _ => stripe,
    });
    let (icon_name, icon_color) = if is_dir {
        ("folder", folder_color(name))
    } else {
        (icon_for(path, false), asset_type_info(path).0)
    };
    let ic = icon_text(commands, &fonts.phosphor, icon_name, icon_color, 12.0);
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[ic, label]);
    row
}

fn tree_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    r: &TreeRow,
    row_index: usize,
    is_renaming: bool,
) -> Entity {
    let content_x = TREE_BASE_X + r.depth as f32 * TREE_INDENT;
    let stripe = inspector_stripe(row_index);

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
        commands.entity(caret).insert((Interaction::default(), TreeToggle(r.path.clone())));
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
                overflow: Overflow::clip(),
                ..default()
            },
            Interaction::default(),
            Name::new(if r.is_file { "tree-file" } else { "tree-nav" }),
        ))
        .id();
    if r.is_file {
        commands.entity(nav).insert(AssetTile { path: r.path.clone(), is_dir: false });
    } else {
        commands.entity(nav).insert(TreeNav(r.path.clone()));
    }
    let folder_icon = if r.is_file {
        icon_text(commands, &fonts.phosphor, icon_for(&r.path, false), asset_type_info(&r.path).0, 13.0)
    } else {
        icon_text(
            commands,
            &fonts.phosphor,
            if r.expanded { "folder-open" } else { "folder" },
            folder_color(&r.name),
            13.0,
        )
    };
    let name = if is_renaming {
        // Inline rename field (same widget the grid uses), laid out to grow in
        // the row rather than force a fixed width.
        let f = rename_field_for(commands, fonts, &r.path, display_name(&r.name, !r.is_file));
        commands.entity(f).insert(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        });
        f
    } else if r.is_file {
        // File rows rename with the same gesture as the grid: an interactive
        // `AssetNameLabel` that passes the click through to the row's `AssetTile`
        // for selection while still registering the name press that arms rename.
        commands
            .spawn((
                Text::new(display_name(&r.name, false).to_string()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_primary())),
                bevy::text::TextLayout::no_wrap(),
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Interaction::default(),
                bevy::ui::FocusPolicy::Pass,
                AssetNameLabel(r.path.clone()),
            ))
            .id()
    } else {
        commands
            .spawn((
                Text::new(r.name.clone()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_primary())),
                bevy::text::TextLayout::no_wrap(),
                Pickable::IGNORE,
                Node {
                    min_width: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .id()
    };
    commands.entity(nav).add_children(&[folder_icon, name]);

    // Full-row background (odd/even stripe + selection + hover), lowest z.
    let bg_visual = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(stripe),
            Pickable::IGNORE,
            Name::new("tree-row-bg"),
        ))
        .id();
    let sel_path = r.path.clone();
    let is_file = r.is_file;
    bind_bg(commands, bg_visual, move |w| {
        let active = if is_file {
            w.get_resource::<NativeAssets>()
                .map(|s| s.is_selected(&sel_path))
                .unwrap_or(false)
        } else {
            current_folder(w).as_deref() == Some(sel_path.as_path())
        };
        if active {
            return rgb(accent()).with_alpha(0.30);
        }
        if matches!(
            w.get::<Interaction>(nav),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            return rgb(renzora_ember::theme::border());
        }
        stripe
    });

    // bg first (behind), then guide lines, then the indent spacer + content.
    let mut kids = vec![bg_visual];
    for (level, &has_more) in r.parent_lines.iter().enumerate() {
        if has_more {
            let x = TREE_BASE_X + level as f32 * TREE_INDENT + TREE_LINE_OFFSET;
            kids.push(renzora_ember::widgets::tree_vline(commands, x, 0.0, true, 0.0));
        }
    }
    if r.depth > 0 {
        let x = TREE_BASE_X + (r.depth - 1) as f32 * TREE_INDENT + TREE_LINE_OFFSET;
        kids.push(renzora_ember::widgets::tree_vline(commands, x, 0.0, !r.is_last, TREE_CENTER_Y));
        kids.push(renzora_ember::widgets::tree_hline(commands, x, TREE_CENTER_Y, 5.0));
    }
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
