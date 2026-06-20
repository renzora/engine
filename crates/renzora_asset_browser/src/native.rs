//! Bevy-native (ember) Asset Browser — the sole asset browser implementation.
//!
//! Toolbar (back + breadcrumb + search) over a wrapping grid of folder/file
//! tiles read from the current folder. Click a folder to navigate in, a file to
//! select. Includes thumbnails, the folder tree, drag-drop, rename/delete and
//! the context menu. The former egui `AssetBrowserPanel` has been removed.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::picking::Pickable;
use bevy::prelude::*;

use renzora_editor_framework::{EditorCommands, MaterialThumbnailRegistry, ModelThumbnailRegistry, SplashState};
use renzora_ember::dock::panel_active;
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::virtual_scroll::virtual_scroll;
use renzora_ember::theme::{accent, panel_bg, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{
    icon_label_button, menu_item, menu_item_styled, menu_sep, screen_menu, scroll_view, slider,
    text_input, EmberScroll, EmberTextInput,
};

use crate::thumbnails::{
    supports_material_thumbnail, supports_model_thumbnail, supports_thumbnail, ThumbnailCache,
};

/// Which thumbnail source a file uses.
#[derive(Clone, Copy, PartialEq)]
enum ThumbKind {
    Image,
    Model,
    Material,
}

fn thumb_kind(name: &str) -> Option<ThumbKind> {
    if supports_thumbnail(name) {
        Some(ThumbKind::Image)
    } else if supports_model_thumbnail(name) {
        Some(ThumbKind::Model)
    } else if supports_material_thumbnail(name) {
        Some(ThumbKind::Material)
    } else {
        None
    }
}

/// The ready thumbnail handle for `path`, from whichever registry owns `kind`.
fn handle_for(w: &World, kind: ThumbKind, path: &PathBuf) -> Option<Handle<Image>> {
    match kind {
        ThumbKind::Image => w.get_resource::<ThumbnailCache>().and_then(|c| c.handle(path)),
        ThumbKind::Model => w.get_resource::<ModelThumbnailRegistry>().and_then(|r| r.handle(path)),
        ThumbKind::Material => w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(path)),
    }
}

const TILE_W: f32 = 96.0;

/// How the current folder's entries are ordered (folders always sort first).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SortMode {
    Name,
    Type,
    Size,
    Modified,
}

impl SortMode {
    const ALL: [SortMode; 4] = [SortMode::Name, SortMode::Type, SortMode::Size, SortMode::Modified];
    fn label(self) -> &'static str {
        match self {
            SortMode::Name => "Name",
            SortMode::Type => "Type",
            SortMode::Size => "Size",
            SortMode::Modified => "Date Modified",
        }
    }
}

// Content-area surfaces (Unreal-style: flat, dark).

/// Lean native state for the browser (independent of the egui panel's state).
#[derive(Resource)]
pub(crate) struct NativeAssets {
    current: Option<PathBuf>,
    selected: Option<PathBuf>,
    search: String,
    /// Expanded folders in the left tree.
    expanded: HashSet<PathBuf>,
    /// The grid tile the cursor is currently over (for right-click targeting).
    hovered: Option<PathBuf>,
    /// Last tile click (path, time) for double-click detection.
    last_click: Option<(PathBuf, f64)>,
    /// Grid tile zoom (0.5–1.5).
    zoom: f32,
    /// Folder-tree pane width (px).
    tree_width: f32,
    /// Active divider drag: `(start cursor x, start tree width)`. Persists the
    /// drag even when the cursor leaves the thin splitter (bevy_ui drops
    /// `Pressed` off-element).
    divider_drag: Option<(f32, f32)>,
    /// Favorited folders (persisted to `<root>/.editor/favorites`).
    favorites: Vec<PathBuf>,
    /// Recently opened files (persisted to `<root>/.editor/recent`).
    recent: Vec<PathBuf>,
    /// Whether favorites/recent have been loaded from disk this session.
    loaded: bool,
    /// A pending tile press `(path, is_dir, origin)` — promoted to a drag once
    /// the cursor moves >5px (for drag-to-viewport).
    drag_press: Option<(PathBuf, bool, Vec2)>,
    /// A plain press on a tile that's already part of a multi-selection. We must
    /// NOT collapse the selection to it on press — that would drop the other
    /// selected items before a drag can carry them. Instead the collapse is
    /// deferred to mouse-release and only applied if no drag occurred (a click).
    pending_single_select: Option<PathBuf>,
    /// Cached directory listing for the current folder. `read_dir` + a
    /// `metadata()` syscall per file is far too expensive to run every frame, so
    /// `refresh_listing` rescans only when the folder/search/sort changes, after
    /// an edit (`listing_dirty`), or on a slow throttle to catch external
    /// changes. The grid snapshot and `visible_order` both read this — shared in
    /// an `Arc` so neither clones the `Entry` data per frame.
    listing: std::sync::Arc<Vec<Entry>>,
    /// Hash of the inputs the cached `listing` was built from (folder, search,
    /// sort, direction). A mismatch forces an immediate rescan.
    listing_sig: u64,
    /// Seconds since the last rescan — drives the slow external-change throttle.
    listing_timer: f32,
    /// Set by edits (create / rename / delete / move) to force a rescan next
    /// frame without waiting for the throttle.
    listing_dirty: bool,
    /// Whether the collapsible FAVORITES / RECENT tree sections are expanded
    /// (both collapsed by default).
    fav_open: bool,
    recent_open: bool,
    /// True while a tile is being dragged out (drives the cursor ghost).
    dragging: bool,
    /// Entry sort order + direction.
    sort: SortMode,
    sort_desc: bool,
    /// List view (rows) instead of the tile grid.
    list_view: bool,
    /// The panel is too narrow for a usable grid alongside the tree: collapse to
    /// a tree-only file browser (grid + splitter hidden, tree shows files). Set
    /// by `responsive_layout` from the panel's measured width.
    narrow: bool,
    /// Multi-selection (marquee + ctrl/shift click). `selected` stays the
    /// primary/anchor (rename + context-menu target).
    selection: HashSet<PathBuf>,
    selection_anchor: Option<PathBuf>,
    /// Active marquee rubber-band, in window logical px: press origin + current.
    marquee_start: Option<Vec2>,
    marquee_current: Option<Vec2>,
    /// Selection captured when the marquee began (swept tiles add to it).
    pre_marquee: HashSet<PathBuf>,
    /// The current folder's entries in display order — for shift-range select.
    visible_order: Vec<PathBuf>,
    /// The asset being inline-renamed (its grid tile / list row shows a text
    /// field instead of the name label). `None` = no active rename.
    renaming: Option<PathBuf>,
    /// A pending name-click rename `(path, click time)`: set when the name of the
    /// already-sole-selected item is clicked, fired by `rename_arm_fire` after a
    /// short delay — unless a double-click opens the item first (which clears it).
    rename_arm: Option<(PathBuf, f64)>,
}

impl Default for NativeAssets {
    fn default() -> Self {
        Self {
            current: None,
            selected: None,
            search: String::new(),
            expanded: HashSet::new(),
            hovered: None,
            last_click: None,
            zoom: 1.0,
            tree_width: 180.0,
            divider_drag: None,
            favorites: Vec::new(),
            recent: Vec::new(),
            loaded: false,
            drag_press: None,
            pending_single_select: None,
            listing: std::sync::Arc::new(Vec::new()),
            listing_sig: 0,
            listing_timer: 0.0,
            listing_dirty: false,
            fav_open: false,
            recent_open: false,
            dragging: false,
            sort: SortMode::Name,
            sort_desc: false,
            list_view: false,
            narrow: false,
            selection: HashSet::new(),
            selection_anchor: None,
            marquee_start: None,
            marquee_current: None,
            pre_marquee: HashSet::new(),
            visible_order: Vec::new(),
            renaming: None,
            rename_arm: None,
        }
    }
}

impl NativeAssets {
    /// Apply a single-tile click with modifiers: ctrl toggles, shift selects the
    /// range from the anchor (using `visible_order`), plain replaces. `selected`
    /// tracks the primary item for rename / context-menu targeting.
    fn click_select(&mut self, path: &Path, ctrl: bool, shift: bool) {
        let p = path.to_path_buf();
        if ctrl {
            if self.selection.contains(&p) {
                self.selection.remove(&p);
                self.selected = self.selection.iter().next().cloned();
            } else {
                self.selection.insert(p.clone());
                self.selected = Some(p.clone());
                self.selection_anchor = Some(p);
            }
        } else if shift && self.selection_anchor.is_some() {
            let anchor = self.selection_anchor.clone().unwrap();
            let ai = self.visible_order.iter().position(|q| *q == anchor);
            let ci = self.visible_order.iter().position(|q| *q == p);
            if let (Some(a), Some(c)) = (ai, ci) {
                let (s, e) = if a <= c { (a, c) } else { (c, a) };
                self.selection.clear();
                for q in &self.visible_order[s..=e] {
                    self.selection.insert(q.clone());
                }
                self.selected = Some(p);
            } else {
                // Anchor/target not in view — fall back to a plain select.
                self.selection.clear();
                self.selection.insert(p.clone());
                self.selected = Some(p.clone());
                self.selection_anchor = Some(p);
            }
        } else {
            self.selection.clear();
            self.selection.insert(p.clone());
            self.selected = Some(p.clone());
            self.selection_anchor = Some(p);
        }
    }

    /// True if `path` is part of the current (multi-)selection.
    fn is_selected(&self, path: &Path) -> bool {
        self.selection.contains(path) || self.selected.as_deref() == Some(path)
    }
}

// ── Favorites / recent persistence (<root>/.editor/{favorites,recent}) ─────────

fn load_list(root: &Path, file: &str) -> Vec<PathBuf> {
    let path = root.join(".editor").join(file);
    std::fs::read_to_string(&path)
        .ok()
        .map(|c| {
            c.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| root.join(l.trim()))
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

fn save_list(root: &Path, file: &str, list: &[PathBuf]) {
    let dir = root.join(".editor");
    let _ = std::fs::create_dir_all(&dir);
    let content: String = list
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok().map(|r| r.to_string_lossy().replace('\\', "/")))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(dir.join(file), content);
}

#[derive(Component)]
struct AssetRoot;
#[derive(Component)]
struct DragGhost;
/// The toolbar "Add" button — clicking it opens the new-asset menu.
#[derive(Component)]
struct AddMenuBtn;
#[derive(Component)]
struct Splitter;

#[derive(Component)]
struct AssetTile {
    path: PathBuf,
    is_dir: bool,
}
/// Marks the inline rename text field in a grid tile / list row, carrying the
/// asset path it renames. The keyed-list rebuild spawns it when `renaming` is set.
#[derive(Component)]
struct AssetRenameInput(PathBuf);
/// The clickable name label of a tile/row. Clicking it while its asset is already
/// the sole selection arms an inline rename (OS-explorer "slow second click").
/// Uses `FocusPolicy::Pass` so the click still reaches the tile beneath it
/// (selection / double-click-open keep working).
#[derive(Component)]
struct AssetNameLabel(PathBuf);
#[derive(Component)]
struct AssetBack;
#[derive(Component)]
struct AssetSearch;
/// A collapsible tree section header (FAVORITES / RECENT).
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Favorites,
    Recent,
}

#[derive(Component)]
struct SectionToggle(Section);

#[derive(Component)]
struct TreeToggle(PathBuf);
#[derive(Component)]
struct TreeNav(PathBuf);
#[derive(Component)]
struct NewAssetBtn(NewAsset);
#[derive(Component)]
struct ImportBtn;
#[derive(Component)]
struct CrumbNav(PathBuf);
#[derive(Component)]
struct ShortcutClick {
    path: PathBuf,
    is_dir: bool,
}

/// What the New-Folder / Add menu creates.
#[derive(Clone, Copy)]
enum NewAsset {
    Folder,
    Material,
    Blueprint,
    Lua,
    Rhai,
    Particle,
}

impl NewAsset {
    fn filename(self) -> &'static str {
        match self {
            NewAsset::Folder => "New Folder",
            NewAsset::Material => "NewMaterial.material",
            NewAsset::Blueprint => "NewBlueprint.blueprint",
            NewAsset::Lua => "new_script.lua",
            NewAsset::Rhai => "new_script.rhai",
            NewAsset::Particle => "NewParticle.particle",
        }
    }
    fn content(self) -> &'static str {
        match self {
            NewAsset::Folder => "",
            NewAsset::Material | NewAsset::Blueprint => "{}",
            NewAsset::Lua => "-- New Lua script\n",
            NewAsset::Rhai => "// New Rhai script\n",
            NewAsset::Particle => "(name: \"New Particle\")",
        }
    }
    fn is_folder(self) -> bool {
        matches!(self, NewAsset::Folder)
    }
}

/// A free path in `folder` for `filename`, suffixing " 2", " 3"… on collision.
fn unique_path(folder: &Path, filename: &str, is_folder: bool) -> PathBuf {
    let candidate = folder.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = if is_folder {
        (filename.to_string(), String::new())
    } else {
        let p = Path::new(filename);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(filename).to_string();
        let ext = p.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
        (stem, ext)
    };
    for n in 2..1000 {
        let cand = folder.join(format!("{stem} {n}{ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    candidate
}

pub fn register_native_asset_browser(app: &mut App) {
    app.init_resource::<NativeAssets>();
    app.init_resource::<renzora::core::SelectAllClaimed>();
    app.register_panel_content("assets", false, build);
    // View systems — gated on the panel being the active (visible) tab. While
    // the Assets tab is a hidden background tab these stand down, so directory
    // scans, thumbnail loading and per-tile layout stop churning over entities
    // nobody can see (the bug where a backgrounded Assets tab held FPS at ~42).
    app.add_systems(
        Update,
        (
            tile_click,
            back_click,
            search_sync,
            request_thumbnails,
            tree_toggle_click,
            tree_nav_click,
            section_toggle_click,
            create_asset_click,
            import_click,
            crumb_click,
            load_persisted,
            shortcut_click,
            asset_drag,
            drag_ghost,
            add_menu_open,
            sort_menu_open,
            view_toggle_click,
            update_grid_layout,
            responsive_layout,
        )
            .run_if(in_state(SplashState::Editor))
            .run_if(panel_active("assets")),
    );
    app.add_systems(
        Update,
        (track_hover, asset_context_menu)
            .chain()
            .run_if(in_state(SplashState::Editor))
            .run_if(panel_active("assets")),
    );
    app.add_systems(
        Update,
        splitter_drag
            .run_if(in_state(SplashState::Editor))
            .run_if(panel_active("assets")),
    );
    app.add_systems(
        Update,
        (
            (refresh_listing, track_visible_order).chain(),
            marquee_select,
            marquee_overlay,
            marquee_autoscroll,
        )
            .run_if(in_state(SplashState::Editor))
            .run_if(panel_active("assets")),
    );
    app.add_systems(
        Update,
        (rename_shortcut, rename_arm_fire, focus_asset_rename, asset_rename_commit)
            .run_if(in_state(SplashState::Editor))
            .run_if(panel_active("assets")),
    );
    // Kept ungated: it publishes `SelectAllClaimed` every frame from grid-hover,
    // and the hierarchy reads that flag to decide whether to handle Ctrl+A. If we
    // froze it while the panel was hidden, the flag could stay stuck "claimed"
    // and swallow the hierarchy's select-all. It's a single cheap query.
    app.add_systems(
        Update,
        select_all_shortcut.run_if(in_state(SplashState::Editor)),
    );
    // PreUpdate (after input is collected) so it can consume Delete before the
    // gizmo's Update entity-delete sees the press. After `UiSystems::Focus` so
    // the grid's `cursor_over` is fresh this frame.
    app.add_systems(
        PreUpdate,
        asset_delete_shortcut
            .after(bevy::input::InputSystems)
            .after(bevy::ui::UiSystems::Focus)
            .run_if(in_state(SplashState::Editor)),
    );
    // Force the resize cursor while hovering/dragging the divider. In PostUpdate
    // so it wins over renzora_hui's Update cursor system (which would otherwise
    // reset to Default once the cursor leaves the thin splitter mid-drag).
    app.add_systems(
        PostUpdate,
        divider_cursor
            .run_if(in_state(SplashState::Editor))
            .run_if(panel_active("assets")),
    );
}

/// Drag the tree/content divider to resize the tree pane. The drag persists via
/// `divider_drag` (captured on press) so it keeps tracking even when the cursor
/// moves off the thin splitter — mirrors the dock's divider.
fn splitter_drag(
    splitter: Query<&Interaction, With<Splitter>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut state: ResMut<NativeAssets>,
) {
    if mouse.just_released(MouseButton::Left) {
        state.divider_drag = None;
    }
    let Some(cx) = windows.iter().next().and_then(|w| w.cursor_position()).map(|p| p.x) else {
        return;
    };
    if state.divider_drag.is_none()
        && mouse.just_pressed(MouseButton::Left)
        && splitter.iter().any(|i| *i == Interaction::Pressed)
    {
        state.divider_drag = Some((cx, state.tree_width));
    }
    if let Some((start_x, start_w)) = state.divider_drag {
        state.tree_width = (start_w + (cx - start_x)).clamp(120.0, 420.0);
    }
}

/// Show the ew-resize cursor whenever the divider is hovered or being dragged.
fn divider_cursor(
    splitter: Query<&Interaction, With<Splitter>>,
    state: Res<NativeAssets>,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut commands: Commands,
    mut forcing: Local<bool>,
) {
    let want = state.divider_drag.is_some()
        || splitter.iter().any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
    let Ok(win) = windows.single() else {
        return;
    };
    if want {
        commands
            .entity(win)
            .insert(bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::EwResize));
        *forcing = true;
    } else if *forcing {
        *forcing = false;
        commands
            .entity(win)
            .insert(bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::Default));
    }
}

fn create_asset_click(
    q: Query<(&Interaction, &NewAssetBtn), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(folder) = state.current.clone().or_else(|| project.as_ref().map(|p| p.path.clone())) else {
            continue;
        };
        let kind = btn.0;
        let path = unique_path(&folder, kind.filename(), kind.is_folder());
        let ok = if kind.is_folder() {
            std::fs::create_dir_all(&path).is_ok()
        } else {
            std::fs::write(&path, kind.content()).is_ok()
        };
        if ok {
            state.selected = Some(path);
        }
    }
}

fn import_click(
    q: Query<&Interaction, (With<ImportBtn>, Changed<Interaction>)>,
    mut commands: Commands,
    state: Res<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    commands.insert_resource(renzora::core::ImportRequested);
    // Pass a PROJECT-RELATIVE target (e.g. "models/foo"), not an absolute path —
    // the overlay prefixes it with "assets/". Empty (project root) → use default.
    let Some(project) = project else { return };
    let folder = state.current.clone().unwrap_or_else(|| project.path.clone());
    let target_dir = folder
        .strip_prefix(&project.path)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    if !target_dir.is_empty() {
        commands.insert_resource(renzora::core::ImportTargetDir(target_dir));
    }
}

/// Load favorites + recent from disk once the project is known.
fn load_persisted(mut state: ResMut<NativeAssets>, project: Option<Res<renzora::core::CurrentProject>>) {
    if state.loaded {
        return;
    }
    let Some(root) = project.map(|p| p.path.clone()) else {
        return;
    };
    state.favorites = load_list(&root, "favorites");
    state.recent = load_list(&root, "recent");
    state.loaded = true;
}

/// Drag a tile out toward the viewport: records the press, and once the cursor
/// moves >5px inserts an `AssetDragPayload` (the viewport shows a live preview
/// while it exists and commits the spawn when it's removed on release). Mirrors
/// the egui drag lifecycle, which only runs in the egui pass.
fn asset_drag(
    tiles: Query<(&Interaction, &AssetTile)>,
    tree: Query<(&Interaction, &TreeNav)>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut state: ResMut<NativeAssets>,
    payload: Option<Res<renzora_editor_framework::AssetDragPayload>>,
    mut commands: Commands,
) {
    if mouse.just_released(MouseButton::Left) {
        // Dropped over a folder (a grid folder tile or a tree row) → move the
        // dragged file(s) into it instead of spawning into the viewport.
        if state.dragging {
            if let Some(payload) = payload.as_ref() {
                if let Some(target) = drop_folder(&tiles, &tree) {
                    let sources = payload.paths.clone();
                    commands.queue(move |w: &mut World| move_assets(w, &sources, &target));
                }
            }
        }
        // A plain click (no drag) on an already-multi-selected item collapses
        // the selection to just that item — applied here, on release, so the
        // multi-selection survived in case this had become a drag instead.
        if let Some(p) = state.pending_single_select.take() {
            if !state.dragging {
                state.selection.clear();
                state.selection.insert(p.clone());
                state.selected = Some(p.clone());
                state.selection_anchor = Some(p);
            }
        }
        state.drag_press = None;
        state.dragging = false;
        if payload.is_some() {
            commands.remove_resource::<renzora_editor_framework::AssetDragPayload>();
        }
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(c) = cursor {
            // A pressed grid/list tile (file or folder)…
            if let Some((_, tile)) = tiles.iter().find(|(i, _)| matches!(i, Interaction::Pressed)) {
                state.drag_press = Some((tile.path.clone(), tile.is_dir, c));
            // …or a pressed tree folder row (TreeNav targets are always folders).
            } else if let Some((_, nav)) = tree.iter().find(|(i, _)| matches!(i, Interaction::Pressed)) {
                state.drag_press = Some((nav.0.clone(), true, c));
            }
        }
    }
    if payload.is_none() {
        if let (Some((path, _is_dir, origin)), Some(c)) = (state.drag_press.clone(), cursor) {
            // Files *and* folders are draggable into folders.
            if c.distance(origin) > 5.0 {
                // Multi-drag: carry the whole selection if the dragged tile is
                // part of it, else just the dragged file.
                let paths: Vec<PathBuf> = if state.selection.contains(&path) && state.selection.len() > 1 {
                    state.selection.iter().cloned().collect()
                } else {
                    vec![path.clone()]
                };
                let count = paths.len();
                commands.insert_resource(renzora_editor_framework::AssetDragPayload {
                    name: file_name_of(&path),
                    paths,
                    icon: String::new(),
                    color: [170, 175, 190],
                    origin: Vec2::new(origin.x, origin.y),
                    is_detached: true,
                    drag_count: count,
                    path,
                });
                state.dragging = true;
            }
        }
    }
}

/// The folder under the cursor to drop onto — a hovered grid folder tile, else a
/// hovered tree folder row.
fn drop_folder(
    tiles: &Query<(&Interaction, &AssetTile)>,
    tree: &Query<(&Interaction, &TreeNav)>,
) -> Option<PathBuf> {
    // Accept Hovered *or* Pressed: on the release frame the folder under the
    // cursor may still read Pressed (the button was down through the drag).
    let over = |i: &Interaction| matches!(i, Interaction::Hovered | Interaction::Pressed);
    if let Some((_, tile)) = tiles.iter().find(|(i, t)| t.is_dir && over(i)) {
        return Some(tile.path.clone());
    }
    tree.iter().find(|(i, _)| over(i)).map(|(_, nav)| nav.0.clone())
}

/// Move `sources` into `target` (drag-to-folder). Skips no-op / into-itself
/// moves and updates asset references via `emit_asset_path_change`, mirroring the
/// egui browser's `pending_move`. Navigates to the target on success.
fn move_assets(world: &mut World, sources: &[PathBuf], target: &Path) {
    let mut moved = 0usize;
    for source in sources {
        let Some(file_name) = source.file_name() else { continue };
        let dest = target.join(file_name);
        if *source == dest || source.as_path() == target {
            continue;
        }
        // Don't move a folder into itself / a descendant.
        if dest.starts_with(source) {
            continue;
        }
        let is_dir = source.is_dir();
        if std::fs::rename(source, &dest).is_ok() {
            moved += 1;
            crate::emit_asset_path_change(world, source, &dest, is_dir);
        }
    }
    if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
        s.selection.clear();
        s.selected = None;
        s.listing_dirty = true;
        if moved > 0 {
            s.current = Some(target.to_path_buf());
            s.expanded.insert(target.to_path_buf());
        }
    }
}

/// A floating ghost (icon + name) that follows the cursor while a tile is being
/// dragged out of the browser. Spawned as a top-level overlay so it isn't
/// clipped by the panel, and despawned the moment the drag ends.
fn drag_ghost(
    mut commands: Commands,
    state: Res<NativeAssets>,
    payload: Option<Res<renzora_editor_framework::AssetDragPayload>>,
    fonts: Option<Res<EmberFonts>>,
    windows: Query<&Window>,
    mut ghosts: Query<(Entity, &mut Node), With<DragGhost>>,
) {
    let Some(payload) = payload.filter(|_| state.dragging) else {
        for (e, _) in &ghosts {
            commands.entity(e).despawn();
        }
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    // Reposition an existing ghost.
    if let Some((_, mut node)) = ghosts.iter_mut().next() {
        node.left = Val::Px(cursor.x + 12.0);
        node.top = Val::Px(cursor.y + 14.0);
        return;
    }
    // Otherwise spawn one (fonts may not be ready on the very first drag frame).
    let Some(fonts) = fonts else {
        return;
    };
    let color = asset_type_info(&payload.path).0;
    let ghost = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 12.0),
                top: Val::Px(cursor.y + 14.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                max_width: Val::Px(240.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg()).with_alpha(0.92)),
            BorderColor::all(rgb(accent())),
            GlobalZIndex(10_000),
            Pickable::IGNORE,
            DragGhost,
            Name::new("asset-drag-ghost"),
        ))
        .id();
    let ic = icon_text(&mut commands, &fonts.phosphor, icon_for(&payload.path, false), color, 14.0);
    let lbl = commands
        .spawn((
            Text::new(payload.name.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(ghost).add_children(&[ic, lbl]);
}

/// A favorites/recent shortcut row: navigate (folder) or open (file).
fn shortcut_click(
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
fn track_recent(state: &mut NativeAssets, path: &Path, root: Option<&Path>) {
    state.recent.retain(|p| p != path);
    state.recent.insert(0, path.to_path_buf());
    state.recent.truncate(20);
    if let Some(root) = root {
        save_list(root, "recent", &state.recent);
    }
}

fn crumb_click(
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

// ── Context menu ─────────────────────────────────────────────────────────────

fn track_hover(tiles: Query<(&Interaction, &AssetTile)>, mut state: ResMut<NativeAssets>) {
    for (interaction, tile) in &tiles {
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed)
            && state.hovered.as_deref() != Some(tile.path.as_path())
        {
            state.hovered = Some(tile.path.clone());
        }
    }
}

/// Right-click a tile → open the shared ember menu (Favorite / Duplicate /
/// Reveal / Delete) for the hovered asset.
fn asset_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Res<NativeAssets>,
    roots: Query<&bevy::ui::RelativeCursorPosition, With<AssetRoot>>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    if !roots.iter().any(|rcp| rcp.cursor_over) {
        return;
    }
    let Some(path) = state.hovered.clone() else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let fav_label = if state.favorites.contains(&path) {
        "Unfavorite"
    } else {
        "Favorite"
    };
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let mut kids = Vec::new();
    // "Open in <Editor>" routes editor-backed assets to their panel/layout.
    if let Some((icon, label)) = open_action(&path) {
        kids.push(menu_item(&mut commands, &fonts, icon, label, {
            let path = path.clone();
            move |w| open_from_menu(w, &path)
        }));
        kids.push(menu_sep(&mut commands));
    }
    kids.extend([
        menu_item(&mut commands, &fonts, "star", fav_label, {
            let path = path.clone();
            move |w| toggle_favorite(w, &path)
        }),
        menu_item(&mut commands, &fonts, "pencil-simple", "Rename", {
            let path = path.clone();
            move |w| start_rename(w, &path)
        }),
        menu_item(&mut commands, &fonts, "copy", "Duplicate", {
            let path = path.clone();
            move |_| duplicate_asset(&path)
        }),
        menu_item(&mut commands, &fonts, "folder-open", "Reveal in Explorer", {
            let path = path.clone();
            move |_| reveal_in_explorer(&path)
        }),
        menu_sep(&mut commands),
        menu_item_styled(&mut commands, &fonts, "trash", "Delete", (224, 96, 88), (224, 96, 88), {
            let path = path.clone();
            move |w| delete_asset(w, &path)
        }),
    ]);
    commands.entity(menu).add_children(&kids);
}

/// Click the toolbar "Add" button → open the shared ember menu of new-asset
/// types at the cursor.
fn add_menu_open(
    q: Query<
        (
            &Interaction,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        (With<AddMenuBtn>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    // Anchor the menu to the button's bottom-left (stable, cursor-independent).
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let items = [
        ("palette", "Material", NewAsset::Material),
        ("blueprint", "Blueprint", NewAsset::Blueprint),
        ("code", "Lua Script", NewAsset::Lua),
        ("code", "Rhai Script", NewAsset::Rhai),
        ("sparkle", "Particle", NewAsset::Particle),
    ];
    let kids: Vec<Entity> = items
        .iter()
        .map(|&(icon, label, kind)| {
            menu_item(&mut commands, &fonts, icon, label, move |w| create_asset(w, kind))
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

#[derive(Component)]
struct SortMenuBtn;
#[derive(Component)]
struct ViewToggleBtn;
#[derive(Component)]
struct AssetGrid;
/// The grid viewport — a marquee starts on an empty press here.
#[derive(Component)]
struct GridArea;
/// The rubber-band selection rectangle overlay (top-level, unclipped).
#[derive(Component)]
struct MarqueeRect;

/// Open the sort menu (modes + ascending/descending) anchored under the button.
fn sort_menu_open(
    q: Query<
        (
            &Interaction,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        (With<SortMenuBtn>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<NativeAssets>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let (cur_sort, cur_desc) = state.map(|s| (s.sort, s.sort_desc)).unwrap_or((SortMode::Name, false));
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let mut kids: Vec<Entity> = SortMode::ALL
        .iter()
        .map(|&mode| {
            let icon = if mode == cur_sort { "check" } else { "dot" };
            menu_item(&mut commands, &fonts, icon, mode.label(), move |w| {
                if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                    s.sort = mode;
                }
            })
        })
        .collect();
    kids.push(menu_sep(&mut commands));
    kids.push(menu_item(
        &mut commands,
        &fonts,
        if cur_desc { "arrow-up" } else { "check" },
        "Ascending",
        |w| {
            if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                s.sort_desc = false;
            }
        },
    ));
    kids.push(menu_item(
        &mut commands,
        &fonts,
        if cur_desc { "check" } else { "arrow-down" },
        "Descending",
        |w| {
            if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                s.sort_desc = true;
            }
        },
    ));
    commands.entity(menu).add_children(&kids);
}

/// Keep `visible_order` in sync with the grid's current folder + sort so
/// shift-range selection knows the display order.
fn track_visible_order(mut state: ResMut<NativeAssets>) {
    // Reads the shared cache (refreshed by `refresh_listing`) — no filesystem.
    let order: Vec<PathBuf> = state.listing.iter().map(|e| e.path.clone()).collect();
    if state.visible_order != order {
        state.visible_order = order;
    }
}

/// Rubber-band selection: pressing in empty grid space and dragging selects
/// every tile the rectangle touches. Ctrl/Shift keep the prior selection
/// (sweeping adds to it). Mirrors the egui browser's marquee.
fn marquee_select(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    grid: Query<&bevy::ui::RelativeCursorPosition, With<GridArea>>,
    tiles: Query<(&AssetTile, &Interaction, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
    mut state: ResMut<NativeAssets>,
) {
    if mouse.just_released(MouseButton::Left) {
        state.marquee_start = None;
        state.marquee_current = None;
        state.pre_marquee.clear();
        return;
    }
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };

    // Begin on a press over the grid that didn't land on a tile. Suppressed while
    // an inline rename is active so clicking into its field doesn't start a sweep.
    if mouse.just_pressed(MouseButton::Left) && state.marquee_start.is_none() && state.renaming.is_none() {
        let over_grid = grid.iter().any(|r| r.cursor_over);
        let on_tile = tiles.iter().any(|(_, i, _, _)| *i == Interaction::Pressed);
        if over_grid && !on_tile {
            let keep = keyboard.pressed(KeyCode::ControlLeft)
                || keyboard.pressed(KeyCode::ControlRight)
                || keyboard.pressed(KeyCode::ShiftLeft)
                || keyboard.pressed(KeyCode::ShiftRight);
            state.marquee_start = Some(cursor);
            state.marquee_current = Some(cursor);
            if keep {
                state.pre_marquee = state.selection.clone();
            } else {
                state.selection.clear();
                state.pre_marquee.clear();
            }
        }
    }

    // Update the sweep while held.
    if mouse.pressed(MouseButton::Left) {
        if let Some(start) = state.marquee_start {
            state.marquee_current = Some(cursor);
            let (min, max) = (start.min(cursor), start.max(cursor));
            let mut sel = state.pre_marquee.clone();
            for (tile, _, cn, ugt) in &tiles {
                let scale = cn.inverse_scale_factor();
                let half = cn.size() * scale * 0.5;
                let center = ugt.translation * scale;
                let (tmin, tmax) = (center - half, center + half);
                // AABB overlap test against the marquee rect.
                if tmin.x <= max.x && tmax.x >= min.x && tmin.y <= max.y && tmax.y >= min.y {
                    sel.insert(tile.path.clone());
                }
            }
            state.selection = sel;
            state.selected = state.selection.iter().next().cloned();
        }
    }
}

/// Draws/updates the marquee rectangle as a top-level overlay (unclipped by the
/// panel), and despawns it when the marquee ends.
fn marquee_overlay(
    mut commands: Commands,
    state: Res<NativeAssets>,
    mut rects: Query<(Entity, &mut Node), With<MarqueeRect>>,
) {
    if let (Some(a), Some(b)) = (state.marquee_start, state.marquee_current) {
        let min = a.min(b);
        let size = (a.max(b) - min).max(Vec2::ZERO);
        if let Some((_, mut n)) = rects.iter_mut().next() {
            n.left = Val::Px(min.x);
            n.top = Val::Px(min.y);
            n.width = Val::Px(size.x);
            n.height = Val::Px(size.y);
        } else {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(min.x),
                    top: Val::Px(min.y),
                    width: Val::Px(size.x),
                    height: Val::Px(size.y),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(accent()).with_alpha(0.15)),
                BorderColor::all(rgb(accent())),
                GlobalZIndex(9_000),
                Pickable::IGNORE,
                MarqueeRect,
                Name::new("asset-marquee"),
            ));
        }
    } else {
        for (e, _) in &rects {
            commands.entity(e).despawn();
        }
    }
}

/// While a marquee is being dragged, scroll the grid when the cursor nears the
/// viewport's top/bottom edge — so a rubber-band can reach off-screen tiles.
/// Speed ramps with how deep into the edge band the cursor is.
fn marquee_autoscroll(
    state: Res<NativeAssets>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    grid_area: Query<&Children, With<GridArea>>,
    mut scrolls: Query<(&mut EmberScroll, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
) {
    // How close (px) to an edge triggers scrolling, and the max px/frame at the
    // very edge.
    const EDGE: f32 = 34.0;
    const MAX_SPEED: f32 = 16.0;

    if state.marquee_start.is_none() || !mouse.pressed(MouseButton::Left) {
        return;
    }
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    // The grid's `scroll_view` outer (GridArea) wraps the `EmberScroll` viewport
    // as its first child.
    let Ok(kids) = grid_area.single() else {
        return;
    };
    let Some(viewport) = kids.iter().find(|&e| scrolls.contains(e)) else {
        return;
    };
    let Ok((mut s, cn, ugt)) = scrolls.get_mut(viewport) else {
        return;
    };
    let inv = cn.inverse_scale_factor();
    let half_h = cn.size().y * inv * 0.5;
    let center_y = ugt.translation.y * inv;
    let (top, bottom) = (center_y - half_h, center_y + half_h);
    if cursor.y < top + EDGE {
        let t = ((top + EDGE - cursor.y) / EDGE).clamp(0.0, 1.0);
        s.nudge(-t * MAX_SPEED);
    } else if cursor.y > bottom - EDGE {
        let t = ((cursor.y - (bottom - EDGE)) / EDGE).clamp(0.0, 1.0);
        s.nudge(t * MAX_SPEED);
    }
}

// ── Inline rename ────────────────────────────────────────────────────────────

/// Begin an inline rename of `path`: select it (so it's the visible target) and
/// arm `renaming`, which makes the keyed-list rebuild that tile with a focused
/// text field (`AssetRenameInput`).
fn start_rename(world: &mut World, path: &Path) {
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
fn rename_arm_fire(time: Res<Time>, mut state: ResMut<NativeAssets>) {
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
fn rename_shortcut(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<NativeAssets>) {
    if !keys.just_pressed(KeyCode::F2) || state.renaming.is_some() {
        return;
    }
    if let Some(path) = state.selected.clone() {
        state.renaming = Some(path);
    }
}

/// Ctrl/Cmd+A selects every visible entry in the current folder. Gated on the
/// grid being hovered so it doesn't hijack select-all from the viewport,
/// hierarchy, or a focused text field elsewhere in the editor.
///
/// It also publishes [`SelectAllClaimed`] every frame from the grid-hover state,
/// so the hierarchy's "select all entities" stands down while the cursor is over
/// the file grid — otherwise both fire on the same `Ctrl+A`.
fn select_all_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    grid: Query<&bevy::ui::RelativeCursorPosition, With<GridArea>>,
    mut state: ResMut<NativeAssets>,
    mut claimed: ResMut<renzora::core::SelectAllClaimed>,
) {
    let over_grid = grid.iter().any(|r| r.cursor_over) && state.renaming.is_none();
    claimed.0 = over_grid;

    let ctrl = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    if !ctrl || !keys.just_pressed(KeyCode::KeyA) || !over_grid {
        return;
    }
    if state.visible_order.is_empty() {
        return;
    }
    state.selection = state.visible_order.iter().cloned().collect();
    state.selected = state.visible_order.first().cloned();
    state.selection_anchor = state.visible_order.first().cloned();
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
fn asset_delete_shortcut(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    grid: Query<&bevy::ui::RelativeCursorPosition, With<GridArea>>,
    state: Res<NativeAssets>,
    mut commands: Commands,
) {
    let claim = grid.iter().any(|r| r.cursor_over)
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
fn focus_asset_rename(mut q: Query<&mut EmberTextInput, Added<AssetRenameInput>>) {
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
fn asset_rename_commit(
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
    if new_name.is_empty() || new_name == file_name_of(&path) {
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
    let dest = parent.join(new_name);
    if dest == old || dest.exists() {
        return;
    }
    let is_dir = old.is_dir();
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

/// Collapse to a tree-only file browser when the panel is too narrow for a
/// usable grid (mirrors the egui browser's `TREE_ONLY_WIDTH` behaviour).
fn responsive_layout(
    root: Query<&bevy::ui::ComputedNode, With<AssetRoot>>,
    mut state: ResMut<NativeAssets>,
) {
    const TREE_ONLY_WIDTH: f32 = 310.0;
    let Ok(cn) = root.single() else {
        return;
    };
    let width = cn.size().x * cn.inverse_scale_factor();
    if width <= 0.0 {
        return;
    }
    let narrow = width < TREE_ONLY_WIDTH;
    if state.narrow != narrow {
        state.narrow = narrow;
    }
}

/// Toggle grid/list view.
fn view_toggle_click(
    q: Query<&Interaction, (With<ViewToggleBtn>, Changed<Interaction>)>,
    mut state: ResMut<NativeAssets>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.list_view = !state.list_view;
    }
}

/// Retune the grid container's gaps/padding when the view mode changes (tight
/// rows for the list, roomy cells for the grid).
fn update_grid_layout(
    state: Res<NativeAssets>,
    mut last: Local<Option<bool>>,
    mut q: Query<&mut Node, With<AssetGrid>>,
) {
    if *last == Some(state.list_view) {
        return;
    }
    *last = Some(state.list_view);
    for mut n in &mut q {
        if state.list_view {
            n.row_gap = Val::Px(1.0);
            n.column_gap = Val::Px(0.0);
            n.padding = UiRect::axes(Val::Px(4.0), Val::Px(4.0));
        } else {
            n.row_gap = Val::Px(12.0);
            n.column_gap = Val::Px(10.0);
            n.padding = UiRect::all(Val::Px(10.0));
        }
    }
}

/// Create a new asset (folder or file) in the current folder + select it.
fn create_asset(world: &mut World, kind: NewAsset) {
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
    let path = unique_path(&folder, kind.filename(), kind.is_folder());
    let ok = if kind.is_folder() {
        std::fs::create_dir_all(&path).is_ok()
    } else {
        std::fs::write(&path, kind.content()).is_ok()
    };
    if ok {
        if let Some(mut s) = world.get_resource_mut::<NativeAssets>() {
            s.selected = Some(path);
            s.listing_dirty = true;
        }
    }
}

fn toggle_favorite(world: &mut World, path: &Path) {
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

fn delete_asset(world: &mut World, path: &Path) {
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
fn duplicate_asset(path: &Path) {
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
fn reveal_in_explorer(path: &Path) {
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

fn project_root(w: &World) -> Option<PathBuf> {
    w.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone())
}

/// The folder being shown (the explicit nav target, else the project root).
fn current_folder(w: &World) -> Option<PathBuf> {
    w.get_resource::<NativeAssets>()
        .and_then(|s| s.current.clone())
        .or_else(|| project_root(w))
}

/// Accent color for a folder's icon, by well-known name (ported from the egui
/// browser's `folder_icon_color`).
fn folder_color(name: &str) -> (u8, u8, u8) {
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
/// tile's type subtitle and bottom accent strip (mirrors Unreal's "Blueprint
/// Class" subtitle + colored underline). Folders are handled separately.
fn asset_type_info(path: &Path) -> ((u8, u8, u8), &'static str) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "material" => ((0, 200, 130), "Material"),
        "blueprint" | "bp" => ((100, 180, 255), "Blueprint"),
        "lua" => ((120, 170, 255), "Lua Script"),
        "rhai" => ((130, 230, 180), "Rhai Script"),
        "wgsl" | "glsl" | "vert" | "frag" => ((220, 120, 255), "Shader"),
        "rs" => ((230, 140, 90), "Rust Source"),
        "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "dds" | "bmp" | "tga" => {
            ((150, 210, 120), "Texture")
        }
        "glb" | "gltf" | "obj" | "fbx" => ((255, 170, 100), "Model"),
        "bsn" | "ron" | "scn" | "scene" => ((115, 191, 242), "Scene"),
        "particle" => ((230, 160, 90), "Particle"),
        "wav" | "ogg" | "mp3" | "flac" => ((200, 130, 230), "Audio"),
        "html" => ((230, 120, 90), "Template"),
        "" => ((150, 155, 170), "File"),
        other => ((150, 155, 170), uppercase_ext(other)),
    }
}

/// Leak a small set of uppercased extension labels for unknown types so the
/// subtitle can read e.g. "TXT" / "JSON". Bounded: only the handful of distinct
/// unknown extensions actually present in a project are ever leaked.
fn uppercase_ext(ext: &str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut map = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    if let Some(s) = map.get(ext) {
        return s;
    }
    let leaked: &'static str = Box::leak(ext.to_uppercase().into_boxed_str());
    map.insert(ext.to_string(), leaked);
    leaked
}

fn icon_for(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "dds" | "bmp" | "tga" => "image",
        "glb" | "gltf" | "obj" | "fbx" => "cube",
        "material" => "palette",
        "wgsl" | "glsl" | "vert" | "frag" => "graphics-card",
        "lua" | "rhai" | "rs" | "py" | "js" | "ts" => "code",
        "scene" | "bsn" | "ron" | "scn" => "film-slate",
        "wav" | "ogg" | "mp3" | "flac" => "speaker-high",
        "particle" => "sparkle",
        "blueprint" | "bp" => "blueprint",
        "html" => "browser",
        "toml" => "brackets-curly",
        _ => "file",
    }
}

// ── Panel ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            bevy::ui::RelativeCursorPosition::default(),
            AssetRoot,
        ))
        .id();

    // ── Folder tree (left pane, own scroll) ──
    let tree_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::vertical(Val::Px(4.0)),
            ..default()
        })
        .id();
    keyed_list(commands, tree_list, tree_snapshot);
    let tree_scroll = scroll_view(commands, tree_list);
    let tree_pane = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                ..default()
            },
            // Match the hierarchy panel base so the shared odd/even stripe
            // (inspector_stripe) renders the exact same colors.
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    commands.entity(tree_pane).add_child(tree_scroll);
    bind_with(
        commands,
        tree_pane,
        |w| {
            let s = w.get_resource::<NativeAssets>();
            (s.map(|s| s.narrow).unwrap_or(false), s.map(|s| s.tree_width).unwrap_or(180.0))
        },
        |w, e, (narrow, width): &(bool, f32)| {
            if let Some(mut n) = w.get_mut::<Node>(e) {
                // Tree-only mode: the tree fills the panel; otherwise its fixed width.
                n.width = if *narrow { Val::Percent(100.0) } else { Val::Px(*width) };
            }
        },
    );

    // Draggable divider (highlights on hover/drag).
    let splitter = commands
        .spawn((
            Node {
                width: Val::Px(4.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::border())),
            Interaction::default(),
            Splitter,
            Name::new("assets-splitter"),
        ))
        .id();
    bind_bg(commands, splitter, move |w| {
        let dragging = w.get_resource::<NativeAssets>().is_some_and(|s| s.divider_drag.is_some());
        let hovered = matches!(
            w.get::<Interaction>(splitter),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        if dragging || hovered {
            rgb(accent())
        } else {
            rgb(renzora_ember::theme::border())
        }
    });

    // ── Content (toolbar + search + grid + footer) ──
    let content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::panel_bg())),
        ))
        .id();

    // Toolbar.
    let toolbar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::header_bg())),
        ))
        .id();
    let back = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::hover_bg())),
            Interaction::default(),
            AssetBack,
            Name::new("assets-back"),
        ))
        .id();
    let back_icon = icon_text(commands, &fonts.phosphor, "arrow-left", text_primary(), 13.0);
    commands.entity(back).add_child(back_icon);
    // Invisible at the project root (nowhere to go up to) but still occupies its
    // slot via `Visibility::Hidden`, so the breadcrumb keeps a constant left
    // position whether or not the back button shows.
    bind_with(
        commands,
        back,
        |w| current_folder(w) != project_root(w),
        |w, e, show: &bool| {
            if let Some(mut v) = w.get_mut::<Visibility>(e) {
                let want = if *show { Visibility::Inherited } else { Visibility::Hidden };
                if *v != want {
                    *v = want;
                }
            }
        },
    );

    let new_folder = icon_label_button(commands, fonts, "folder-plus", "New Folder");
    commands.entity(new_folder).insert(NewAssetBtn(NewAsset::Folder));
    let add = icon_label_button(commands, fonts, "plus", "Add");
    commands
        .entity(add)
        .insert((AddMenuBtn, bevy::ui::RelativeCursorPosition::default()));
    let import = icon_label_button(commands, fonts, "download-simple", "Import");
    commands.entity(import).insert(ImportBtn);

    // Sort dropdown (opens a screen_menu of sort modes + direction).
    let sort_btn = icon_label_button(commands, fonts, "sort-ascending", "Sort");
    commands
        .entity(sort_btn)
        .insert((SortMenuBtn, bevy::ui::RelativeCursorPosition::default()));

    // View toggle (grid <-> list); icon reflects the view a click switches *to*.
    let view_icon = icon_text(commands, &fonts.phosphor, "list", text_primary(), 15.0);
    bind_with(
        commands,
        view_icon,
        |w| w.get_resource::<NativeAssets>().map(|s| s.list_view).unwrap_or(false),
        |w, e, list_view: &bool| {
            let name = if *list_view { "squares-four" } else { "list" };
            if let (Some(ch), Some(mut t)) = (renzora_ember::font::icon_glyph(name), w.get_mut::<Text>(e)) {
                t.0 = ch.to_string();
            }
        },
    );
    let view_btn = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::hover_bg())),
            Interaction::default(),
            ViewToggleBtn,
            Name::new("assets-view-toggle"),
        ))
        .id();
    commands.entity(view_btn).add_child(view_icon);

    // Breadcrumb in a dark inset box, with the back button to its left.
    let crumbs = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, crumbs, crumb_snapshot);

    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    // Zoom control (maps 0..1 → 0.5..1.5 tile scale) in a small framed box with
    // a magnifier glyph.
    let zoom = slider(commands, 0.5);
    bind_2way(
        commands,
        zoom,
        |w| w.get_resource::<NativeAssets>().map(|s| (s.zoom - 0.5).clamp(0.0, 1.0)).unwrap_or(0.5),
        |w, v| {
            if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                s.zoom = 0.5 + *v;
            }
        },
    );
    commands.entity(zoom).insert(Node {
        width: Val::Px(70.0),
        height: Val::Px(14.0),
        position_type: PositionType::Relative,
        align_items: AlignItems::Center,
        ..default()
    });
    let zoom_icon = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 11.0);
    let zoom_box = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::row_even())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("zoom-box"),
        ))
        .id();
    commands.entity(zoom_box).add_children(&[zoom_icon, zoom]);

    // Compact search field, placed just left of the zoom control.
    let search = text_input(commands, &fonts.ui, "Search...", "");
    commands.entity(search).insert((
        AssetSearch,
        Node {
            width: Val::Px(160.0),
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));

    commands
        .entity(toolbar)
        .add_children(&[add, import, new_folder, spacer, sort_btn, view_btn, search, zoom_box]);

    // Grid (also hosts list-view rows: a 100%-wide row wraps to its own line, so
    // the same wrapping container stacks them vertically). `update_grid_layout`
    // retunes the gaps/padding per view.
    let grid = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                width: Val::Percent(100.0),
                align_items: AlignItems::FlexStart,
                align_content: AlignContent::FlexStart,
                column_gap: Val::Px(10.0),
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            AssetGrid,
        ))
        .id();
    // Virtualized: only the tiles in (or near) the viewport are built, so a
    // folder of hundreds of split meshes stays cheap. `grid_snapshot` returns the
    // full list; `virtual_scroll` windows it.
    virtual_scroll(commands, grid, 6, grid_snapshot);
    let grid_scroll = scroll_view(commands, grid);
    // Mark the grid viewport so the marquee knows when a press lands in empty
    // grid space (vs. on a tile or the tree).
    commands.entity(grid_scroll).insert((GridArea, bevy::ui::RelativeCursorPosition::default()));

    // Live item count — sits at the right of the breadcrumb bar.
    let count = commands
        .spawn((
            Text::new("0 items"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_with(
        commands,
        count,
        |w| list_entries(w).len(),
        |w, e, n: &usize| {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                let s = if *n == 1 {
                    "1 item".to_string()
                } else {
                    format!("{n} items")
                };
                if t.0 != s {
                    t.0 = s;
                }
            }
        },
    );

    // Breadcrumb bar (back + path on the left, item count on the right),
    // directly under the top toolbar.
    let crumb_spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let crumb_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::window_bg())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("assets-crumb-bar"),
        ))
        .id();
    commands.entity(crumb_bar).add_children(&[back, crumbs, crumb_spacer, count]);

    commands
        .entity(content)
        .add_children(&[toolbar, crumb_bar, grid_scroll]);

    // Responsive: when the panel is too narrow, hide the grid + splitter so the
    // tree fills it as a file browser (see `responsive_layout` / `narrow`).
    bind_display(commands, content, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow));
    bind_display(commands, splitter, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow));

    commands.entity(root).add_children(&[tree_pane, splitter, content]);
    root
}

/// Clickable breadcrumb segments (project root + each path component).
fn crumb_snapshot(world: &World) -> KeyedSnapshot {
    let Some(root) = project_root(world) else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let cur = current_folder(world).unwrap_or_else(|| root.clone());
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project")
        .to_string();
    let mut segs: Vec<(String, PathBuf)> = vec![(root_name, root.clone())];
    if let Ok(rel) = cur.strip_prefix(&root) {
        let mut acc = root.clone();
        for comp in rel.components() {
            acc = acc.join(comp);
            segs.push((comp.as_os_str().to_string_lossy().to_string(), acc.clone()));
        }
    }
    let items: Vec<(u64, u64)> = segs
        .iter()
        .enumerate()
        .map(|(i, (name, path))| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            (i, path).hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| crumb_seg(c, f, i, &segs[i].0, &segs[i].1)),
    }
}

fn crumb_seg(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, path: &Path) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let mut kids = Vec::new();
    if idx > 0 {
        kids.push(icon_text(commands, &fonts.phosphor, "caret-right", text_muted(), 9.0));
    }
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            Interaction::default(),
            CrumbNav(path.to_path_buf()),
            Name::new("crumb"),
        ))
        .id();
    kids.push(label);
    commands.entity(row).add_children(&kids);
    row
}

// ── Grid ─────────────────────────────────────────────────────────────────────

struct Entry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    size: u64,
    modified: u64,
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
fn list_entries(w: &World) -> std::sync::Arc<Vec<Entry>> {
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
fn refresh_listing(
    time: Res<Time>,
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    use std::hash::{Hash, Hasher};

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
    // Folders always first; then the chosen key (reversed when descending).
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
    entries
}

fn grid_snapshot(world: &World) -> KeyedSnapshot {
    let entries = list_entries(world);
    if entries.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new("Empty folder"),
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

/// The inline rename text field for a grid/list entry, seeded with its current
/// name and tagged so `asset_rename_commit` can find it.
fn rename_field(commands: &mut Commands, fonts: &EmberFonts, entry: &Entry) -> Entity {
    let input = text_input(commands, &fonts.ui, "Name", &entry.name);
    commands.entity(input).insert((
        AssetRenameInput(entry.path.clone()),
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            height: Val::Px(20.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
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
                Text::new(entry.name.clone()),
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
        renzora_ember::reactive::bind_text_color(commands, star, |w| {
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
                Text::new(entry.name.clone()),
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

// ── Folder tree ──────────────────────────────────────────────────────────────
//
// Tree indent guides match the hierarchy panel: 1.5px absolute line nodes, the
// same INDENT / LINE_OFFSET geometry, and `inspector_stripe` odd/even row bands.

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

fn has_subdirs(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten().any(|e| {
                !e.file_name().to_string_lossy().starts_with('.')
                    && e.file_type().map(|t| t.is_dir()).unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// A 1.5px vertical guide line. `full` runs the whole row height; otherwise it
/// stops at `height` (used for the elbow on a last child).
/// Whether `dir` contains any non-hidden file (used to decide if a folder is
/// expandable in tree-only file mode).
fn has_visible_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten().any(|e| {
                !e.file_name().to_string_lossy().starts_with('.')
                    && e.file_type().map(|t| t.is_file()).unwrap_or(false)
            })
        })
        .unwrap_or(false)
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
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                subs.push((e.path(), name));
            } else if show_files {
                files.push((e.path(), name));
            }
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

/// A row in the tree: a section header, a favorites/recent shortcut, or a folder.
enum TreeItem {
    /// A section header. `section` is `Some` for collapsible groups
    /// (FAVORITES / RECENT); `None` for the always-open FOLDERS header.
    Header {
        label: &'static str,
        section: Option<Section>,
        open: bool,
    },
    Shortcut { name: String, path: PathBuf, is_dir: bool },
    Folder(TreeRow),
}

fn file_name_of(p: &Path) -> String {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string()
}

fn tree_snapshot(world: &World) -> KeyedSnapshot {
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
    let fav_open = st.map(|s| s.fav_open).unwrap_or(false);
    let recent_open = st.map(|s| s.recent_open).unwrap_or(false);
    let show_files = st.map(|s| s.narrow).unwrap_or(false);

    let mut items: Vec<TreeItem> = Vec::new();
    if !favorites.is_empty() {
        items.push(TreeItem::Header {
            label: "FAVORITES",
            section: Some(Section::Favorites),
            open: fav_open,
        });
        if fav_open {
            for p in &favorites {
                items.push(TreeItem::Shortcut {
                    name: file_name_of(p),
                    path: p.clone(),
                    is_dir: p.is_dir(),
                });
            }
        }
    }
    if !recent.is_empty() {
        items.push(TreeItem::Header {
            label: "RECENT",
            section: Some(Section::Recent),
            open: recent_open,
        });
        if recent_open {
            for p in recent.iter().take(20) {
                items.push(TreeItem::Shortcut {
                    name: file_name_of(p),
                    path: p.clone(),
                    is_dir: false,
                });
            }
        }
    }
    items.push(TreeItem::Header {
        label: "FOLDERS",
        section: None,
        open: true,
    });
    let mut rows = Vec::new();
    flatten_dirs(&root, 0, &expanded, show_files, &mut Vec::new(), &mut rows);
    items.extend(rows.into_iter().map(TreeItem::Folder));

    let keyed: Vec<(u64, u64)> = items
        .iter()
        .map(|it| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match it {
                TreeItem::Header { label, open, .. } => {
                    (0u8, label).hash(&mut k);
                    (label, open).hash(&mut h);
                }
                TreeItem::Shortcut { name, path, is_dir } => {
                    (1u8, path).hash(&mut k);
                    (name, is_dir).hash(&mut h);
                }
                TreeItem::Folder(r) => {
                    (2u8, &r.path).hash(&mut k);
                    (r.depth, r.expanded, r.has_children, &r.name).hash(&mut h);
                    (r.is_last, &r.parent_lines, r.is_file).hash(&mut h);
                }
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items: keyed,
        build: Box::new(move |c, f, i| match &items[i] {
            TreeItem::Header { label, section, open } => tree_header(c, f, label, *section, *open),
            TreeItem::Shortcut { name, path, is_dir } => shortcut_row(c, f, name, path, *is_dir),
            TreeItem::Folder(r) => tree_row(c, f, r, i),
        }),
    }
}

/// A section header. Collapsible groups (FAVORITES / RECENT) get a caret and a
/// click target; the FOLDERS header is a plain label.
fn tree_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    text: &str,
    section: Option<Section>,
    open: bool,
) -> Entity {
    // Collapsible sections (FAVORITES / RECENT) sit in a subtle inset box; the
    // plain FOLDERS header is borderless.
    let boxed = section.is_some();
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(3.0),
                margin: if boxed {
                    UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(4.0), Val::Px(0.0))
                } else {
                    UiRect::ZERO
                },
                padding: if boxed {
                    UiRect::axes(Val::Px(6.0), Val::Px(4.0))
                } else {
                    UiRect::new(Val::Px(6.0), Val::Px(0.0), Val::Px(6.0), Val::Px(2.0))
                },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(if boxed { rgb(renzora_ember::theme::popup_bg()) } else { Color::NONE }),
            Name::new("tree-header"),
        ))
        .id();
    let mut kids = Vec::new();
    if section.is_some() {
        let caret = icon_text(
            commands,
            &fonts.phosphor,
            if open { "caret-down" } else { "caret-right" },
            text_muted(),
            9.0,
        );
        kids.push(caret);
    }
    let label = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    kids.push(label);
    commands.entity(row).add_children(&kids);
    if let Some(sec) = section {
        commands
            .entity(row)
            .insert((Interaction::default(), SectionToggle(sec)));
    }
    row
}

fn shortcut_row(commands: &mut Commands, fonts: &EmberFonts, name: &str, path: &Path, is_dir: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(20.0),
                padding: UiRect::left(Val::Px(8.0)),
                column_gap: Val::Px(4.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ShortcutClick {
                path: path.to_path_buf(),
                is_dir,
            },
            Name::new("tree-shortcut"),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    let (icon_name, icon_color) = if is_dir {
        ("folder", folder_color(name))
    } else {
        (icon_for(path, false), text_muted())
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

fn tree_row(commands: &mut Commands, fonts: &EmberFonts, r: &TreeRow, row_index: usize) -> Entity {
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
                ui_font(&fonts.phosphor, 10.0),
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
    let name = commands
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
        .id();
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

// ── Click systems ────────────────────────────────────────────────────────────

fn tree_toggle_click(
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

fn tree_nav_click(
    q: Query<(&Interaction, &TreeNav)>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut state: ResMut<NativeAssets>,
    mut press: Local<Option<(PathBuf, Vec2)>>,
) {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
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
            if !moved {
                state.current = Some(path.clone());
                state.selected = None;
                if state.expanded.contains(&path) {
                    state.expanded.remove(&path);
                } else {
                    state.expanded.insert(path);
                }
            }
        }
    }
}

fn section_toggle_click(
    q: Query<(&Interaction, &SectionToggle), Changed<Interaction>>,
    mut state: ResMut<NativeAssets>,
) {
    for (interaction, toggle) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match toggle.0 {
            Section::Favorites => state.fav_open = !state.fav_open,
            Section::Recent => state.recent_open = !state.recent_open,
        }
    }
}

fn tile_click(
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
fn open_file(cmds: &Option<Res<EditorCommands>>, path: &Path) {
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

/// Code-editor-backed kinds: scripts, shaders, HTML templates, and plain text.
/// These open straight into `CodeEditorState` (no layout switch).
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
fn open_action(path: &Path) -> Option<(&'static str, &'static str)> {
    use renzora_editor_framework::DocTabKind;
    if path.is_dir() {
        return Some(("folder-open", "Open"));
    }
    match renzora_editor_framework::doc_kind_for_path(path) {
        Some(DocTabKind::Material) => Some(("palette", "Open in Material Editor")),
        Some(DocTabKind::Particle) => Some(("sparkle", "Open in Particle Editor")),
        Some(DocTabKind::Blueprint) => Some(("blueprint", "Open in Blueprint Editor")),
        Some(DocTabKind::Scene) => Some(("film-slate", "Open Scene")),
        Some(DocTabKind::Script) | Some(DocTabKind::Shader) => Some(("code", "Open in Code Editor")),
        _ if is_editable_text(path) => Some(("code", "Open in Code Editor")),
        _ => None,
    }
}

/// Context-menu open: navigate into folders, route files to their editor.
fn open_from_menu(world: &mut World, path: &Path) {
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

/// Open a script/shader/template/text file in the code editor. Always loads it
/// into `CodeEditorState` (no blank-prone document-tab layout switch); then makes
/// the editor visible per the `code_open_switch_layout` setting — either adding a
/// Code Editor panel to the current dock layout (default) or switching to the
/// dedicated "Scripting" layout.
fn open_in_code_editor(world: &mut World, path: PathBuf) {
    world.insert_resource(renzora::core::OpenCodeEditorFile { path });
    let switch = world
        .get_resource::<renzora_editor_framework::EditorSettings>()
        .map(|s| s.code_open_switch_layout)
        .unwrap_or(false);

    // egui dock model (legacy backend): focus/add the panel, or switch layout.
    if switch {
        renzora_editor_framework::switch_layout_by_name(world, "Scripting");
    } else if let Some(mut docking) = world.get_resource_mut::<renzora_editor_framework::DockingState>() {
        docking.tree.focus_or_add_panel("code_editor");
    }

    // bevy_ui shell dock model: add/focus the code-editor panel in the live dock
    // and flag a rebuild. The shell renders from its own `renzora_ember::dock`
    // model, not `DockingState`, so this is what makes the panel appear there.
    // (Switching the ember workspace needs the shell's `ShellLayouts`, which
    // isn't reachable here, so on the bevy_ui backend both modes reveal in place.)
    if let Some(mut dock) = world.get_resource_mut::<renzora_ember::dock::Dock>() {
        dock.tree.focus_or_add_panel("code_editor");
    }
    if let Some(mut dirty) = world.get_resource_mut::<renzora_ember::dock::DockDirty>() {
        dirty.0 = true;
    }
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

fn back_click(
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
fn request_thumbnails(
    tiles: Query<&AssetTile>,
    mut cache: ResMut<ThumbnailCache>,
    mut model: Option<ResMut<ModelThumbnailRegistry>>,
    mut material: Option<ResMut<MaterialThumbnailRegistry>>,
    asset_server: Res<AssetServer>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let project = project.as_deref();
    for tile in &tiles {
        if tile.is_dir {
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
            None => {}
        }
    }
}

fn search_sync(input: Query<&EmberTextInput, With<AssetSearch>>, mut state: ResMut<NativeAssets>) {
    for inp in &input {
        if state.search != inp.value {
            state.search = inp.value.clone();
        }
    }
}
