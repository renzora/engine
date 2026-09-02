//! The file grid: the cached directory listing behind it, and the two ways an
//! entry is drawn — a zoomable card [`tile`] or a compact [`list_row`].
//!
//! The listing is *cached*, not read per frame. `read_dir` plus a `metadata()`
//! syscall per file was the dominant cost pinning the visible panel's frame rate
//! on a folder of hundreds of split meshes; [`refresh_listing`] rescans only when
//! the folder, search or sort changed, an edit marked it dirty, or a slow
//! throttle elapsed to catch changes made by other tools.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_with};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::widgets::text_input;

use crate::ops::{asset_type_info, folder_color, icon_for};
use crate::state::{
    handle_for, hash_path_set, thumb_kind, AssetNameLabel, AssetRenameInput, AssetTile,
    NativeAssets, SortMode, TILE_W,
};
use crate::thumbnails::ThumbnailCache;

pub(crate) struct Entry {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) size: u64,
    pub(crate) modified: u64,
}

/// What the browser *shows* for an entry: a file drops its extension, a folder
/// keeps its whole name (a dot in a folder name is part of the name, not a
/// type). The type icon, its accent colour and the type label under the tile
/// already say what the file is, so the extension is noise on the label — and
/// the rename field seeds from this too, so you edit exactly the text you were
/// reading and `keep_extension` puts the extension back on commit.
///
/// Sorting, searching and thumbnail routing all still work off the real
/// `Entry::name`; this is presentation only.
pub(crate) fn display_name(name: &str, is_dir: bool) -> &str {
    if is_dir {
        return name;
    }
    match name.rsplit_once('.') {
        // A leading-dot name (`.gitignore`) is all name and no extension.
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

/// Lowercase file extension (`""` for none / folders).
fn ext_of(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

/// Human-readable byte size (e.g. `1.5 MB`).
fn human_size(bytes: u64) -> String {
    const U: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut s = bytes as f64;
    let mut i = 0;
    while s >= 1024.0 && i < U.len() - 1 {
        s /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{s:.1} {}", U[i])
    }
}

/// The current folder's cached listing (see [`refresh_listing`]). Cheap — clones
/// an `Arc`, never touches the filesystem.
pub(crate) fn list_entries(w: &Rx) -> std::sync::Arc<Vec<Entry>> {
    w.get_resource::<NativeAssets>()
        .map(|s| s.listing.clone())
        .unwrap_or_default()
}

/// How often (seconds) to rescan the folder even when nothing the editor did
/// changed — the safety net for files added/removed by other tools.
const LISTING_THROTTLE: f32 = 0.5;

/// Rescan the current folder into [`NativeAssets::listing`], but only when the
/// folder/search/sort changed, an edit marked it dirty, or the slow throttle
/// elapsed. This replaces a per-frame `read_dir` + a `metadata()` syscall per
/// file — which, on a folder of hundreds of split meshes, was the dominant cost
/// pinning the visible Assets panel's frame rate.
pub(crate) fn refresh_listing(
    time: Res<Time>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let folder = state
        .current
        .clone()
        .or_else(|| project.as_ref().map(|p| p.path.clone()));
    let Some(folder) = folder else {
        if !state.listing.is_empty() {
            state.listing = std::sync::Arc::new(Vec::new());
        }
        return;
    };

    let mut h = std::collections::hash_map::DefaultHasher::new();
    folder.hash(&mut h);
    state.search.to_lowercase().hash(&mut h);
    (state.sort as u8).hash(&mut h);
    state.sort_desc.hash(&mut h);
    let sig = h.finish();

    state.listing_timer += time.delta_secs();
    let stale =
        sig != state.listing_sig || state.listing_dirty || state.listing_timer >= LISTING_THROTTLE;
    if !stale {
        return;
    }

    let search = state.search.to_lowercase();
    let entries = read_sorted_entries(&folder, &search, state.sort, state.sort_desc);
    state.listing = std::sync::Arc::new(entries);
    state.listing_sig = sig;
    state.listing_timer = 0.0;
    state.listing_dirty = false;
}

/// Read + sort a folder's non-hidden entries (folders first). Shared by the grid
/// snapshot and the shift-range `visible_order` tracker.
fn read_sorted_entries(folder: &Path, search: &str, sort: SortMode, desc: bool) -> Vec<Entry> {
    let mut entries = read_entries(folder, search);
    sort_entries(&mut entries, sort, desc);
    entries
}

/// Web: the same listing, out of the browser's directory handle.
///
/// A cache miss yields nothing for this frame and starts the read; the panel
/// already rescans on a throttle, so the folder fills in a beat later.
/// (`list_dir` handles converting the editor's project-prefixed path into one
/// relative to the picked directory.)
#[cfg(target_arch = "wasm32")]
fn read_entries(folder: &Path, search: &str) -> Vec<Entry> {
    let Some(list) = renzora_webfs::list_dir(folder) else {
        return Vec::new();
    };
    list.into_iter()
        .filter(|e| !e.name.starts_with('.'))
        .filter(|e| search.is_empty() || e.name.to_lowercase().contains(search))
        .map(|e| Entry {
            path: folder.join(&e.name),
            name: e.name,
            is_dir: e.is_dir,
            size: e.size,
            modified: e.modified,
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_entries(folder: &Path, search: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(folder) {
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if !search.is_empty() && !name.to_lowercase().contains(search) {
                continue;
            }
            let meta = e.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = if is_dir { 0 } else { meta.as_ref().map(|m| m.len()).unwrap_or(0) };
            let modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            entries.push(Entry { path, name, is_dir, size, modified });
        }
    }
    entries
}

/// Folders always first; then the chosen key (reversed when descending).
/// Shared by both targets — only the reading differs.
fn sort_entries(entries: &mut [Entry], sort: SortMode, desc: bool) {
    entries.sort_by(|a, b| {
        let dir = b.is_dir.cmp(&a.is_dir);
        if dir != std::cmp::Ordering::Equal {
            return dir;
        }
        let by_name = || a.name.to_lowercase().cmp(&b.name.to_lowercase());
        let ord = match sort {
            SortMode::Name => by_name(),
            SortMode::Type => ext_of(&a.name).cmp(&ext_of(&b.name)).then_with(by_name),
            SortMode::Size => a.size.cmp(&b.size).then_with(by_name),
            SortMode::Modified => a.modified.cmp(&b.modified).then_with(by_name),
        };
        if desc {
            ord.reverse()
        } else {
            ord
        }
    });
}

/// Dirty token for the asset grid. The listing `Arc` is rebuilt by
/// `refresh_listing` whenever the folder, search, sort, or contents change (it's
/// pre-sorted, so its pointer identity captures item set *and* order). The rest
/// are per-render overlays the snapshot folds into each item's hash. Combined
/// with `virtual_scroll_versioned`'s scroll-window term, this skips the per-entry
/// hashing on frames where nothing changed.
pub(crate) fn grid_token(world: &Rx) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(st) = world.get_resource::<NativeAssets>() {
        (std::sync::Arc::as_ptr(&st.listing) as usize as u64).hash(&mut h);
        ((st.zoom * 20.0).round() as i64).hash(&mut h);
        st.list_view.hash(&mut h);
        st.renaming.hash(&mut h);
        hash_path_set(st.favorites.iter()).hash(&mut h);
    }
    h.finish()
}

pub(crate) fn grid_snapshot(world: &Rx) -> KeyedSnapshot {
    let entries = list_entries(world);
    if entries.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new(renzora::lang::t("assets.empty_folder")),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                    Node { padding: UiRect::all(Val::Px(8.0)), ..default() },
                ))
                .id()
            }),
        };
    }
    let (zoom, list_view, renaming) = world
        .get_resource::<NativeAssets>()
        .map(|s| (s.zoom, s.list_view, s.renaming.clone()))
        .unwrap_or((1.0, false, None));
    let zoom_q = (zoom * 20.0).round() as u64;
    let favs: HashSet<PathBuf> = world
        .get_resource::<NativeAssets>()
        .map(|s| s.favorites.iter().cloned().collect())
        .unwrap_or_default();
    let items: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| {
            let editing = renaming.as_deref() == Some(e.path.as_path());
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&e.name, e.is_dir, zoom_q, favs.contains(&e.path), list_view, e.size, editing).hash(&mut h);
            let mut k = std::collections::hash_map::DefaultHasher::new();
            e.path.hash(&mut k);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let e = &entries[i];
            let fav = favs.contains(&e.path);
            let editing = renaming.as_deref() == Some(e.path.as_path());
            if list_view {
                list_row(c, f, e, fav, editing)
            } else {
                tile(c, f, e, zoom, fav, editing)
            }
        }),
    }
}

/// The inline rename text field for a grid/list entry, seeded with the name as
/// displayed (extension-less for files) and tagged so `asset_rename_commit` can
/// find it.
fn rename_field(commands: &mut Commands, fonts: &EmberFonts, entry: &Entry) -> Entity {
    rename_field_for(commands, fonts, &entry.path, display_name(&entry.name, entry.is_dir))
}

/// The inline rename field from a bare path + name — shared by the grid tiles and
/// the folder tree so both rename identically.
pub(crate) fn rename_field_for(
    commands: &mut Commands,
    fonts: &EmberFonts,
    path: &Path,
    name: &str,
) -> Entity {
    let input = text_input(commands, &fonts.ui, &renzora::lang::t("common.name"), name);
    commands.entity(input).insert((
        AssetRenameInput(path.to_path_buf()),
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            // 22px (not 20) so the text caret isn't jammed against the bottom.
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
    ));
    input
}

/// One compact list-view row: type icon + name + type + size, sharing the same
/// `AssetTile` selection/click/drag wiring as the grid tile.
fn list_row(commands: &mut Commands, fonts: &EmberFonts, entry: &Entry, fav: bool, editing: bool) -> Entity {
    let is_dir = entry.is_dir;
    let (type_color, type_label) = if is_dir {
        (folder_color(&entry.name), "Folder")
    } else {
        asset_type_info(&entry.path)
    };
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            AssetTile {
                path: entry.path.clone(),
                is_dir,
            },
            Name::new("asset-row"),
        ))
        .id();
    let path_bg = entry.path.clone();
    bind_bg(commands, row, move |w| {
        let selected = w
            .get_resource::<NativeAssets>()
            .map(|s| s.is_selected(&path_bg))
            .unwrap_or(false);
        let hovered = matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        let st = &w.resource::<renzora_ember::style::Theme>().asset_tile;
        if selected {
            st.card_selected.color()
        } else if hovered {
            st.card_hover.color()
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, icon_for(&entry.path, is_dir), type_color, 15.0);
    let name = if editing {
        rename_field(commands, fonts, entry)
    } else {
        commands
            .spawn((
                Text::new(display_name(&entry.name, is_dir).to_string()),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_primary())),
                bevy::text::TextLayout::no_wrap(),
                Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
                Interaction::default(),
                bevy::ui::FocusPolicy::Pass,
                AssetNameLabel(entry.path.clone()),
            ))
            .id()
    };
    let ty = commands
        .spawn((
            Text::new(type_label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node { width: Val::Px(96.0), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
        ))
        .id();
    let size = commands
        .spawn((
            Text::new(if is_dir { String::new() } else { human_size(entry.size) }),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node { width: Val::Px(64.0), flex_shrink: 0.0, ..default() },
        ))
        .id();
    let mut kids = vec![icon, name, ty, size];
    if fav {
        let star = icon_text(commands, &fonts.phosphor, "star", (255, 200, 70), 11.0);
        kids.insert(1, star);
    }
    commands.entity(row).add_children(&kids);
    row
}

fn tile(commands: &mut Commands, fonts: &EmberFonts, entry: &Entry, zoom: f32, fav: bool, editing: bool) -> Entity {
    let card_w = (TILE_W * zoom).round();
    let thumb_h = card_w; // square preview, Unreal-style
    let icon_sz = (card_w * 0.42).round();
    let is_dir = entry.is_dir;
    let (type_color, type_label) = if is_dir {
        (folder_color(&entry.name), "Folder")
    } else {
        asset_type_info(&entry.path)
    };

    // ── Card shell (folders and files share the same card) ──
    let col = commands
        .spawn((
            Node {
                width: Val::Px(card_w),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Interaction::default(),
            AssetTile {
                path: entry.path.clone(),
                is_dir,
            },
            Name::new("asset-tile"),
        ))
        .id();
    let path_bg = entry.path.clone();
    bind_bg(commands, col, move |w| {
        let selected = w
            .get_resource::<NativeAssets>()
            .map(|s| s.is_selected(&path_bg))
            .unwrap_or(false);
        let hovered = matches!(
            w.get::<Interaction>(col),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        let st = &w.resource::<renzora_ember::style::Theme>().asset_tile;
        if selected {
            st.card_selected.color()
        } else if hovered {
            st.card_hover.color()
        } else {
            st.card_bg.color()
        }
    });
    // Themed border — accent (border_selected) on select, else border.
    let path_bd = entry.path.clone();
    bind_with(
        commands,
        col,
        move |w| {
            w.get_resource::<NativeAssets>()
                .map(|s| s.is_selected(&path_bd))
                .unwrap_or(false)
        },
        move |w, e, selected: &bool| {
            let (sel, norm) = {
                let st = &w.resource::<renzora_ember::style::Theme>().asset_tile;
                (st.border_selected.color(), st.border.color())
            };
            if let Some(mut bc) = w.get_mut::<BorderColor>(e) {
                *bc = BorderColor::all(if *selected { sel } else { norm });
            }
        },
    );

    // ── Thumbnail area (icon + optional rendered thumbnail overlay) ──
    let thumb = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(thumb_h),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
        ))
        .id();
    bind_bg(commands, thumb, |w| {
        w.resource::<renzora_ember::style::Theme>().asset_tile.thumb_bg.color()
    });
    let icon = icon_text(commands, &fonts.phosphor, icon_for(&entry.path, is_dir), type_color, icon_sz);
    commands.entity(thumb).add_child(icon);

    // Star badge on favorited assets.
    if fav {
        let star = icon_text(commands, &fonts.phosphor, "star", (255, 200, 70), 12.0);
        renzora_ember::reactive::tracked::bind_text_color(commands, star, |w| {
            w.resource::<renzora_ember::style::Theme>().asset_tile.star.color()
        });
        commands.entity(star).insert(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(3.0),
            right: Val::Px(3.0),
            ..default()
        });
        commands.entity(thumb).add_child(star);
    }

    // Folders preview their contents: a mosaic of up to four images found at or
    // just below them, so a texture library doesn't render as forty identical
    // glyphs.
    //
    // The cells are spawned unconditionally and everything about them — how
    // many are visible, how big they are, which image each shows — is decided
    // by the binding below. Baking the scanned image list into the tile's
    // structure at build time does NOT work: `scan_folder_previews` runs after
    // the tile already exists, so the tile would have to be torn down and
    // rebuilt to gain a mosaic, and the grid only rebuilds a tile when its
    // content hash changes. Making the mosaic runtime state instead of build
    // state takes the rebuild out of the picture entirely.
    if is_dir {
        // Geometry in px rather than percentages, because the mosaic needs a
        // margin off the tile edge and gutters between the cells — and neither
        // can be expressed as a percentage of a box the cells also have to fill.
        // `card_w` is already the zoom-scaled tile width, so this tracks zoom.
        let inset = (card_w * 0.075).round().max(3.0);
        let gutter = (card_w * 0.025).round().max(2.0);
        // The thumb box is the card minus its 1px border, so it isn't square —
        // fit a square mosaic to the smaller side and centre it by hand rather
        // than insetting all four edges, which would leave the cells' fixed
        // sizes guessing at a box width they'd overflow and wrap out of.
        let box_w = (card_w - 2.0).max(1.0);
        let span = (box_w.min(thumb_h) - inset * 2.0).max(8.0);
        let grid = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(((box_w - span) / 2.0).round()),
                    top: Val::Px(((thumb_h - span) / 2.0).round()),
                    width: Val::Px(span),
                    height: Val::Px(span),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(gutter),
                    row_gap: Val::Px(gutter),
                    // Wrapped rows sit at the top at their own height; the
                    // default stretches the two lines apart.
                    align_content: AlignContent::FlexStart,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    overflow: Overflow::clip(),
                    // Hidden until the scan finds something, so a folder with no
                    // images keeps its glyph rather than showing an empty box.
                    display: Display::None,
                    ..default()
                },
                Name::new("asset-folder-mosaic"),
            ))
            .id();
        commands.entity(thumb).add_child(grid);
        let cells: Vec<Entity> = (0..crate::thumbnails::FOLDER_PREVIEW_MAX)
            .map(|_| {
                let cell = commands
                    .spawn((
                        // `Stretch` fills the cell; the default `Auto`
                        // letterboxes, which would leave ragged gaps inside the
                        // gutters and turn a tidy contact sheet into rubble. At
                        // quadrant size the aspect distortion is invisible.
                        ImageNode {
                            image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                            ..default()
                        },
                        Node {
                            display: Display::None,
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            ..default()
                        },
                        Name::new("asset-folder-mosaic-cell"),
                    ))
                    .id();
                commands.entity(grid).add_child(cell);
                cell
            })
            .collect();

        // A folder showing its contents must still read as a folder — without
        // this, a tile of four textures is indistinguishable from a texture
        // asset at a glance. The chip keeps the glyph legible over a bright
        // image, and keeps the per-folder accent colour the plain tiles use.
        let badge_sz = (card_w * 0.2).round().max(9.0);
        let badge_icon = icon_text(
            commands,
            &fonts.phosphor,
            icon_for(&entry.path, is_dir),
            type_color,
            badge_sz,
        );
        let badge = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(3.0),
                    bottom: Val::Px(3.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.78)),
                Name::new("asset-folder-badge"),
            ))
            .id();
        commands.entity(badge).add_child(badge_icon);
        commands.entity(thumb).add_child(badge);

        let folder = entry.path.clone();
        bind_with(
            commands,
            grid,
            move |w| {
                let images = w
                    .get_resource::<crate::thumbnails::FolderPreviews>()
                    .and_then(|p| p.images(&folder))
                    .unwrap_or_default();
                images
                    .iter()
                    .take(crate::thumbnails::FOLDER_PREVIEW_MAX)
                    .map(|path| {
                        w.get_resource::<ThumbnailCache>()
                            .and_then(|c| c.handle(path))
                    })
                    .collect::<Vec<Option<Handle<Image>>>>()
            },
            move |w, e, handles: &Vec<Option<Handle<Image>>>| {
                // Sized from the *scanned* count, not the count that has finished
                // loading, so cells fill in without the mosaic reflowing.
                // 4 images → 2×2; fewer → equal columns, because one image parked
                // in a quadrant of an otherwise empty box reads as a broken tile.
                let (cell_w, cell_h) = if handles.len() >= 4 {
                    let half = ((span - gutter) / 2.0).max(1.0);
                    (half, half)
                } else {
                    let cols = handles.len().max(1) as f32;
                    (
                        ((span - gutter * (cols - 1.0)) / cols).max(1.0),
                        span,
                    )
                };
                let mut shown = 0;
                for (i, &cell) in cells.iter().enumerate() {
                    let handle = handles.get(i).and_then(|h| h.clone());
                    if let Some(handle) = handle.clone() {
                        if let Some(mut image) = w.get_mut::<ImageNode>(cell) {
                            image.image = handle;
                        }
                        shown += 1;
                    }
                    if let Some(mut node) = w.get_mut::<Node>(cell) {
                        node.width = Val::Px(cell_w);
                        node.height = Val::Px(cell_h);
                        node.display = if handle.is_some() {
                            Display::Flex
                        } else {
                            Display::None
                        };
                    }
                }
                // Glyph and mosaic swap both ways: a folder whose images are
                // deleted goes back to its plain centred icon, badge and all.
                let previewing = shown > 0;
                for (target, visible) in [(e, previewing), (badge, previewing), (icon, !previewing)]
                {
                    if let Some(mut node) = w.get_mut::<Node>(target) {
                        node.display = if visible { Display::Flex } else { Display::None };
                    }
                }
            },
        );
    }

    if let Some(kind) = (!is_dir).then(|| thumb_kind(&entry.name)).flatten() {
        let img = commands
            .spawn((
                ImageNode::default(),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
                Name::new("asset-thumb"),
            ))
            .id();
        commands.entity(thumb).add_child(img);
        let p = entry.path.clone();
        bind_with(
            commands,
            img,
            move |w| handle_for(w, kind, &p),
            move |w, e, h: &Option<Handle<Image>>| {
                let Some(handle) = h else { return };
                if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                    n.image = handle.clone();
                }
                if let Some(mut node) = w.get_mut::<Node>(e) {
                    node.display = Display::Flex;
                }
                if let Some(mut node) = w.get_mut::<Node>(icon) {
                    node.display = Display::None;
                }
            },
        );
    }

    // ── Label area: name + type subtitle ──
    let info = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: Val::Px(1.0),
            padding: UiRect::axes(Val::Px(5.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let name = if editing {
        rename_field(commands, fonts, entry)
    } else {
        commands
            .spawn((
                Text::new(display_name(&entry.name, is_dir).to_string()),
                ui_font(&fonts.ui, 10.0),
                TextColor(rgb(text_primary())),
                Node {
                    width: Val::Percent(100.0),
                    max_height: Val::Px(26.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Interaction::default(),
                bevy::ui::FocusPolicy::Pass,
                AssetNameLabel(entry.path.clone()),
            ))
            .id()
    };
    let ty = commands
        .spawn((
            Text::new(type_label),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(renzora_ember::theme::text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(info).add_children(&[name, ty]);

    // Colored bottom accent strip (the Unreal "type underline").
    let accent = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(rgb(type_color)),
        ))
        .id();
    commands.entity(col).add_children(&[thumb, info, accent]);
    col
}
